//! The C ABI. The only seam between the Rust core and the Qt shell.
//!
//! Three rules, and they are worth more than any convenience:
//!
//! 1. Every fallible call returns an [`ScStatus`] — 0 for success, negative for
//!    failure. [`sc_last_error`] gives the sentence to put in front of a person.
//! 2. Everything handed back is *borrowed*, and every function says until when.
//!    The count of `_free` functions should stay near zero.
//! 3. Nothing spawns a thread the frontend does not know about.
//!
//! Nothing Qt-shaped crosses: no widgets, no window handles, no fonts, no
//! glyphs. Every entry point tolerates a null handle and says so, and
//! `tests/abi.c` proves it — a frontend should need no null guards of its own.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};

use sc_diff::{RectF, ViewMode};
use sc_render::Tile as RenderTile;
use sc_session::{Session, SheetChanges};

/// 0 is success. Every failure is negative, so `if (status < 0)` is the whole
/// check a caller needs.
pub type ScStatus = i32;

pub const SC_OK: ScStatus = 0;
/// The render or scan has been started but has not finished; ask again when the
/// wakeup handle fires.
pub const SC_PENDING: ScStatus = 1;
pub const SC_ERR_INVALID: ScStatus = -1;
pub const SC_ERR_IO: ScStatus = -2;
pub const SC_ERR_FORMAT: ScStatus = -3;
pub const SC_ERR_NO_PAGE: ScStatus = -4;
pub const SC_ERR_GEOMETRY: ScStatus = -5;

thread_local! {
    /// The last failure's sentence, per thread.
    ///
    /// Per thread because two threads failing at once must not overwrite each
    /// other's message, and because it means no lock on a path that only exists
    /// to explain something to a person.
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(msg: impl std::fmt::Display) {
    let s = msg.to_string().replace('\0', " ");
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::new(s).unwrap_or_default());
}

fn status_of(e: &sc_render::Error) -> ScStatus {
    match e {
        sc_render::Error::Io(_) => SC_ERR_IO,
        sc_render::Error::Format(_) => SC_ERR_FORMAT,
        sc_render::Error::NoSuchPage(_) => SC_ERR_NO_PAGE,
        sc_render::Error::BadGeometry => SC_ERR_GEOMETRY,
        sc_render::Error::Mupdf(_) => SC_ERR_FORMAT,
    }
}

fn fail(e: sc_render::Error) -> ScStatus {
    let s = status_of(&e);
    set_error(e);
    s
}

fn invalid(msg: &str) -> ScStatus {
    set_error(msg);
    SC_ERR_INVALID
}

/// The last failure on this thread, as a NUL-terminated string.
///
/// # Safety
/// The pointer is valid until the next failing call on the same thread. Copy it
/// if you intend to keep it. Never null; an empty string means nothing has
/// failed yet.
#[no_mangle]
pub extern "C" fn sc_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

/// The library version, as a NUL-terminated string that lives forever.
///
/// # Safety
/// Callable with no preconditions. The returned pointer is valid for the
/// lifetime of the process and must not be freed.
#[no_mangle]
pub extern "C" fn sc_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Which of the three views a tile is composed for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ScViewMode {
    Overlay = 0,
    OnlyA = 1,
    OnlyB = 2,
}

impl From<ScViewMode> for ViewMode {
    fn from(m: ScViewMode) -> Self {
        match m {
            ScViewMode::OnlyA => ViewMode::OnlyA,
            ScViewMode::OnlyB => ViewMode::OnlyB,
            ScViewMode::Overlay => ViewMode::Overlay,
        }
    }
}

impl From<ViewMode> for ScViewMode {
    fn from(m: ViewMode) -> Self {
        match m {
            ViewMode::OnlyA => ScViewMode::OnlyA,
            ViewMode::OnlyB => ScViewMode::OnlyB,
            ViewMode::Overlay => ScViewMode::Overlay,
        }
    }
}

/// A rectangle in page space, in points.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct ScRectF {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
}

impl From<RectF> for ScRectF {
    fn from(r: RectF) -> Self {
        Self { x: r.x, y: r.y, dx: r.dx, dy: r.dy }
    }
}

/// Which sheet of each document a virtual page stands for. 0 means the document
/// has no matching sheet, and that page is drawn as entirely added or removed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct ScPair {
    pub page_a: i32,
    pub page_b: i32,
}

/// A composed tile, 32bpp BGRA, tightly packed.
///
/// On a little-endian host this is exactly `QImage::Format_RGB32`, so the shell
/// wraps `pixels` without copying anything.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ScTile {
    pub pixels: *const u8,
    pub width: i32,
    pub height: i32,
    pub stride: usize,
}

/// One open comparison. Opaque; every field is this crate's business.
pub struct ScSession {
    inner: Session,
    /// The tile handed out by the last [`sc_session_tile`], kept alive so the
    /// pointer the shell is holding stays valid. This is why that function
    /// documents its result as good only until the next call.
    last_tile: Option<sc_diff::Tile>,
    scans: HashMap<i32, SheetChanges>,
}

/// Opens two revisions for comparison.
///
/// # Safety
/// `path_a` and `path_b` must be NUL-terminated UTF-8. Returns null on failure,
/// with the reason in [`sc_last_error`]. Free with [`sc_session_free`].
#[no_mangle]
pub unsafe extern "C" fn sc_session_open(
    path_a: *const c_char,
    path_b: *const c_char,
) -> *mut ScSession {
    let (Some(a), Some(b)) = (cstr(path_a), cstr(path_b)) else {
        invalid("both file paths are required, as UTF-8");
        return std::ptr::null_mut();
    };
    match Session::open(a, b) {
        Ok(inner) => Box::into_raw(Box::new(ScSession {
            inner,
            last_tile: None,
            scans: HashMap::new(),
        })),
        Err(e) => {
            fail(e);
            std::ptr::null_mut()
        }
    }
}

/// Closes a comparison. Null is a no-op, and every pointer this session ever
/// handed out is dangling afterwards.
///
/// # Safety
/// `s` must be a pointer from [`sc_session_open`] that has not already been
/// freed.
#[no_mangle]
pub unsafe extern "C" fn sc_session_free(s: *mut ScSession) {
    if !s.is_null() {
        drop(Box::from_raw(s));
    }
}

/// How many virtual sheets the comparison has.
///
/// Not either document's own count: the range has to cover both, so a set that
/// gained sheets at the front is longer than either. 0 for a null session.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_page_count(s: *const ScSession) -> i32 {
    match s.as_ref() {
        Some(s) => s.inner.page_count(),
        None => 0,
    }
}

/// Which sheet of each document virtual page `page_no` stands for.
///
/// # Safety
/// `s` must be null or a live session. A null session or an out-of-range page
/// gives `{0, 0}`.
#[no_mangle]
pub unsafe extern "C" fn sc_session_pair(s: *const ScSession, page_no: i32) -> ScPair {
    match s.as_ref() {
        Some(s) => {
            let p = s.inner.pair(page_no);
            ScPair { page_a: p.page_a, page_b: p.page_b }
        }
        None => ScPair::default(),
    }
}

/// The sheet's size in points, after any rotation the file carries.
///
/// # Safety
/// `s` must be null or a live session; `w_pt` and `h_pt` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_page_size(
    s: *const ScSession,
    page_no: i32,
    w_pt: *mut f32,
    h_pt: *mut f32,
) -> ScStatus {
    let (Some(s), false, false) = (s.as_ref(), w_pt.is_null(), h_pt.is_null()) else {
        return invalid("a live session and two writable floats are required");
    };
    match s.inner.page_size(page_no) {
        Ok((w, h)) => {
            *w_pt = w;
            *h_pt = h;
            SC_OK
        }
        Err(e) => fail(e),
    }
}

/// The sheet's size in device pixels at `zoom`, as the renderer will round it.
///
/// The viewer lays pages out with this before it has rendered any of them.
///
/// # Safety
/// `s` must be null or a live session; `w` and `h` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_page_device_size(
    s: *const ScSession,
    page_no: i32,
    zoom: f32,
    w: *mut i32,
    h: *mut i32,
) -> ScStatus {
    let (Some(s), false, false) = (s.as_ref(), w.is_null(), h.is_null()) else {
        return invalid("a live session and two writable ints are required");
    };
    match s.inner.page_device_size(page_no, zoom) {
        Ok((dw, dh)) => {
            *w = dw;
            *h = dh;
            SC_OK
        }
        Err(e) => fail(e),
    }
}

/// Composes a tile of the comparison.
///
/// `x` and `y` are device pixels at `zoom` with their origin at the sheet's
/// top-left.
///
/// # Safety
/// `s` must be a live session and `out` writable. **The pixels `out` points at
/// are borrowed and stay valid only until the next `sc_session_tile` call on
/// this session, or until it is freed.** Draw from them or copy them; do not
/// keep the pointer.
#[no_mangle]
pub unsafe extern "C" fn sc_session_tile(
    s: *mut ScSession,
    page_no: i32,
    zoom: f32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    out: *mut ScTile,
) -> ScStatus {
    let (Some(s), false) = (s.as_mut(), out.is_null()) else {
        return invalid("a live session and a writable tile are required");
    };
    match s.inner.compose_tile(page_no, zoom, RenderTile::new(x, y, width, height)) {
        Ok(tile) => {
            let t = s.last_tile.insert(tile);
            *out = ScTile {
                pixels: t.data.as_ptr(),
                width: t.width,
                height: t.height,
                stride: t.stride(),
            };
            SC_OK
        }
        Err(e) => fail(e),
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_view_mode(s: *const ScSession) -> ScViewMode {
    match s.as_ref() {
        Some(s) => s.inner.view_mode().into(),
        None => ScViewMode::Overlay,
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_set_view_mode(s: *mut ScSession, mode: ScViewMode) {
    if let Some(s) = s.as_mut() {
        s.inner.set_view_mode(mode.into());
    }
}

/// How far, in device pixels, ink may sit from its counterpart and still count
/// as the same artwork. Clamped to 0..=3.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_tolerance(s: *const ScSession) -> i32 {
    match s.as_ref() {
        Some(s) => s.inner.options().tolerance,
        None => 0,
    }
}

/// Changing the tolerance changes every answer, so the scan cache is dropped.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_set_tolerance(s: *mut ScSession, tolerance: i32) {
    if let Some(s) = s.as_mut() {
        let mut o = s.inner.options();
        o.tolerance = tolerance.clamp(0, sc_diff::MAX_TOLERANCE);
        s.inner.set_options(o);
        s.scans.clear();
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_page_delta(s: *const ScSession) -> i32 {
    match s.as_ref() {
        Some(s) => s.inner.page_delta(),
        None => 0,
    }
}

/// Nudges which sheet of B lines up with which sheet of A.
///
/// Every cached scan is about the old pairing, so they all go.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_set_page_delta(s: *mut ScSession, delta: i32) {
    if let Some(s) = s.as_mut() {
        s.inner.set_page_delta(delta);
        s.scans.clear();
    }
}

/// Excludes a region, in page points, from the comparison on **every** sheet —
/// which is what a shared title block needs.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_add_ignore_rect(
    s: *mut ScSession,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
) {
    if let Some(s) = s.as_mut() {
        s.inner.add_ignore_rect(RectF::new(x, y, dx, dy));
        s.scans.clear();
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_clear_ignore_rects(s: *mut ScSession) {
    if let Some(s) = s.as_mut() {
        s.inner.clear_ignore_rects();
        s.scans.clear();
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_ignore_rect_count(s: *const ScSession) -> usize {
    match s.as_ref() {
        Some(s) => s.inner.ignore_rects().len(),
        None => 0,
    }
}

/// # Safety
/// `s` must be null or a live session; `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_ignore_rect(
    s: *const ScSession,
    index: usize,
    out: *mut ScRectF,
) -> ScStatus {
    let (Some(s), false) = (s.as_ref(), out.is_null()) else {
        return invalid("a live session and a writable rectangle are required");
    };
    match s.inner.ignore_rects().get(index) {
        Some(r) => {
            *out = (*r).into();
            SC_OK
        }
        None => invalid("no excluded region with that index"),
    }
}

/// Scans one sheet for regions where the two documents disagree, and caches it.
///
/// Costs roughly a third of a second the first time and nothing afterwards.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_scan_page(s: *mut ScSession, page_no: i32) -> ScStatus {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    if s.scans.contains_key(&page_no) {
        return SC_OK;
    }
    match s.inner.scan_page(page_no) {
        Ok(r) => {
            s.scans.insert(page_no, r);
            SC_OK
        }
        Err(e) => fail(e),
    }
}

/// How many change regions the sheet's scan found. −1 if it has not been
/// scanned yet, which is not the same as 0.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_change_count(s: *const ScSession, page_no: i32) -> i32 {
    match s.as_ref().and_then(|s| s.scans.get(&page_no)) {
        Some(r) => r.changes.len() as i32,
        None => -1,
    }
}

/// How many of the sheet's regions fell inside an excluded rectangle.
///
/// Reported rather than dropped: "not compared" must never read as "nothing
/// changed here".
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_ignored_count(s: *const ScSession, page_no: i32) -> i32 {
    match s.as_ref().and_then(|s| s.scans.get(&page_no)) {
        Some(r) => r.ignored,
        None => -1,
    }
}

/// One change region, in page points.
///
/// # Safety
/// `s` must be null or a live session; `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_change(
    s: *const ScSession,
    page_no: i32,
    index: usize,
    out: *mut ScRectF,
) -> ScStatus {
    let (Some(s), false) = (s.as_ref(), out.is_null()) else {
        return invalid("a live session and a writable rectangle are required");
    };
    match s.scans.get(&page_no).and_then(|r| r.changes.get(index)) {
        Some(r) => {
            *out = (*r).into();
            SC_OK
        }
        None => invalid("that sheet has no such change, or has not been scanned"),
    }
}

/// # Safety
/// `p` must be null or a NUL-terminated string.
unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}
