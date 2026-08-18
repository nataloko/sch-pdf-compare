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

use sc_diff::{RectF, TextChange, TextChangeKind, ViewMode};
use sc_render::Tile as RenderTile;
use sc_session::{suggest_ignores, Session, Settings, SheetChanges, Sweep};

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
        Self {
            x: r.x,
            y: r.y,
            dx: r.dx,
            dy: r.dy,
        }
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
    sweep: Option<Sweep>,
    /// The text changes of whichever sheet was asked about last, with their
    /// strings kept alive. The ABI hands out borrowed `const char *`, and this
    /// is what they borrow from.
    text_changes: Vec<(TextChange, CString, CString)>,
    /// Everything the sweep has handed over, in the order it arrived. Kept
    /// alongside `scans` because the repeat detector wants the whole set, and
    /// asking it a question from half a sweep is a different question.
    swept: Vec<SheetChanges>,
    suggested: Vec<RectF>,
    /// Backing for the string [`sc_session_report`] hands out.
    report: CString,
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
            sweep: None,
            text_changes: Vec::new(),
            swept: Vec::new(),
            suggested: Vec::new(),
            report: CString::default(),
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
            ScPair {
                page_a: p.page_a,
                page_b: p.page_b,
            }
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
    match s
        .inner
        .compose_tile(page_no, zoom, RenderTile::new(x, y, width, height))
    {
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
        s.reset_scans();
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
        s.reset_scans();
    }
}

/// Works out which sheet of B each sheet of A corresponds to, from what is
/// written on them, and uses that as the pairing.
///
/// For a set that simply gained a sheet at the front, [`sc_session_set_page_delta`]
/// does the same job and is more predictable. This is for the case an offset
/// cannot express: sheets reordered, or inserted in the middle. It replaces any
/// delta, and every cached answer with it.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_auto_match(s: *mut ScSession) -> ScStatus {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    match s.inner.auto_match() {
        Ok(()) => {
            s.reset_scans();
            SC_OK
        }
        Err(e) => fail(e),
    }
}

/// True when the pairing came from the documents rather than from an offset.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_pairing_is_automatic(s: *const ScSession) -> bool {
    s.as_ref()
        .map(|s| s.inner.pairing_is_automatic())
        .unwrap_or(false)
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
        s.reset_scans();
    }
}

/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_clear_ignore_rects(s: *mut ScSession) {
    if let Some(s) = s.as_mut() {
        s.inner.clear_ignore_rects();
        s.reset_scans();
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

impl ScSession {
    /// Everything cached about the comparison is about the settings that were
    /// in force when it was computed, so a change to any of them throws the lot
    /// away — including a sweep that is still running and would otherwise
    /// deliver answers to a question nobody asked any more.
    fn reset_scans(&mut self) {
        if let Some(mut sweep) = self.sweep.take() {
            sweep.stop();
        }
        self.scans.clear();
        self.swept.clear();
        self.suggested.clear();
        self.text_changes.clear();
    }
}

/// How the sweep is getting on.
///
/// `finished` is the only field that makes `changed_sheets` mean anything;
/// before that it is a running total.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct ScSweepStatus {
    pub running: bool,
    pub finished: bool,
    pub scanned: i32,
    pub total: i32,
    pub changed_sheets: i32,
    /// Regions that recur across the set, waiting to be offered. Only ever
    /// meaningful once `finished`.
    pub suggested: i32,
}

/// Starts scanning every sheet on a worker thread.
///
/// The frontend should watch [`sc_session_wakeup_handle`] and call
/// [`sc_session_pump`] when it signals. Nothing is ever called back on the
/// worker thread.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_start_sweep(s: *mut ScSession) -> ScStatus {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    if let Some(mut old) = s.sweep.take() {
        old.stop();
    }
    s.swept.clear();
    s.suggested.clear();
    match s.inner.start_sweep() {
        Some(sw) => {
            s.sweep = Some(sw);
            SC_OK
        }
        None => invalid("this platform would not give us a wakeup object"),
    }
}

/// Stops the sweep and waits for its thread. Safe to call when none is running.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_stop_sweep(s: *mut ScSession) {
    if let Some(s) = s.as_mut() {
        if let Some(mut sw) = s.sweep.take() {
            sw.stop();
        }
    }
}

/// The handle to watch: a file descriptor on Unix, an event handle on Windows.
///
/// −1 when no sweep is running. Valid only until the sweep is stopped or the
/// session is freed, so a frontend must drop its notifier before either.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_wakeup_handle(s: *const ScSession) -> i64 {
    match s.as_ref().and_then(|s| s.sweep.as_ref()) {
        Some(sw) => sw.wakeup_handle() as i64,
        None => -1,
    }
}

/// Collects whatever the sweep has finished and clears the wakeup.
///
/// Call this when the handle signals, and once more after `finished` shows up
/// so the last sheets are collected. Doing the work here, on the caller's
/// thread, is the point: the sweep never touches the frontend's data.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_pump(s: *mut ScSession) -> ScStatus {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    let Some(sweep) = s.sweep.as_ref() else {
        return SC_OK;
    };
    for r in sweep.take_results() {
        s.scans.insert(r.page_no, r.clone());
        s.swept.push(r);
    }
    let status = sweep.status();
    if status.finished && s.suggested.is_empty() {
        s.suggested = suggest_ignores(&s.swept, s.inner.page_count());
    }
    if status.finished {
        SC_OK
    } else {
        SC_PENDING
    }
}

/// # Safety
/// `s` must be null or a live session; `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_sweep_status(
    s: *const ScSession,
    out: *mut ScSweepStatus,
) -> ScStatus {
    let (Some(s), false) = (s.as_ref(), out.is_null()) else {
        return invalid("a live session and a writable status are required");
    };
    *out = match s.sweep.as_ref() {
        Some(sw) => {
            let st = sw.status();
            ScSweepStatus {
                running: st.running,
                finished: st.finished,
                scanned: st.scanned,
                total: st.total,
                changed_sheets: st.changed_sheets,
                suggested: s.suggested.len() as i32,
            }
        }
        None => ScSweepStatus::default(),
    };
    SC_OK
}

/// How many recurring regions the finished sweep would offer to exclude.
///
/// Offered, never applied: a net renamed across the whole set looks exactly like
/// a changed title-block date, and hiding that silently is the worst failure
/// this tool could have.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_suggested_count(s: *const ScSession) -> i32 {
    match s.as_ref() {
        Some(s) => s.suggested.len() as i32,
        None => 0,
    }
}

/// # Safety
/// `s` must be null or a live session; `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn sc_session_suggested(
    s: *const ScSession,
    index: usize,
    out: *mut ScRectF,
) -> ScStatus {
    let (Some(s), false) = (s.as_ref(), out.is_null()) else {
        return invalid("a live session and a writable rectangle are required");
    };
    match s.suggested.get(index) {
        Some(r) => {
            *out = (*r).into();
            SC_OK
        }
        None => invalid("no suggested region with that index"),
    }
}

/// Loads this pair's saved state: the excluded regions worked out for it last
/// time, and the tolerance and colours in force.
///
/// The frontend chooses whether to call this. A run started `-for-testing`
/// simply does not, which is how persistence is exercised without it.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_load_settings(s: *mut ScSession) -> ScStatus {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    let saved = Settings::load();
    s.inner.set_options(saved.options());
    s.inner.clear_ignore_rects();
    let (a, b) = (s.inner.path_a().to_owned(), s.inner.path_b().to_owned());
    for r in saved.ignore_rects(&a, &b) {
        s.inner.add_ignore_rect(r);
    }
    s.reset_scans();
    SC_OK
}

/// Saves this pair's excluded regions, and the tolerance and colours, for next
/// time.
///
/// The file is re-read first, so a second window comparing a different pair does
/// not lose what it saved.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_save_settings(s: *const ScSession) -> ScStatus {
    let Some(s) = s.as_ref() else {
        return invalid("a live session is required");
    };
    let mut saved = Settings::load();
    saved.set_options(s.inner.options());
    saved.set_ignore_rects(s.inner.path_a(), s.inner.path_b(), s.inner.ignore_rects());
    match saved.save() {
        Ok(()) => SC_OK,
        Err(e) => {
            set_error(format_args!("cannot save settings: {e}"));
            SC_ERR_IO
        }
    }
}

thread_local! {
    /// Backing for [`sc_last_pair`]'s two borrowed strings.
    static LAST_PAIR: RefCell<(CString, CString)> =
        RefCell::new((CString::default(), CString::default()));
}

/// The pair compared most recently, for offering to reopen it.
///
/// False when there is none. The strings are borrowed and valid until the next
/// call to this function on the same thread.
///
/// # Safety
/// `path_a` and `path_b` must be writable, or null if that side is not wanted.
#[no_mangle]
pub unsafe extern "C" fn sc_last_pair(
    path_a: *mut *const c_char,
    path_b: *mut *const c_char,
) -> bool {
    let saved = Settings::load();
    let Some((a, b)) = saved.last_pair() else {
        return false;
    };
    LAST_PAIR.with(|slot| {
        let mut slot = slot.borrow_mut();
        *slot = (
            CString::new(a).unwrap_or_default(),
            CString::new(b).unwrap_or_default(),
        );
        if !path_a.is_null() {
            *path_a = slot.0.as_ptr();
        }
        if !path_b.is_null() {
            *path_b = slot.1.as_ptr();
        }
    });
    true
}

/// The comparison as a document, in Markdown.
///
/// Built from the sheets already scanned — it renders nothing, so a menu item
/// wired to this does not appear to hang. A sweep that has not finished is
/// reported honestly in the text rather than quietly producing a short answer.
///
/// # Safety
/// `s` must be null or a live session. **The string is borrowed and stays valid
/// only until the next `sc_session_report` call on this session, or until it is
/// freed.** Null on failure.
#[no_mangle]
pub unsafe extern "C" fn sc_session_report(s: *mut ScSession) -> *const c_char {
    let Some(s) = s.as_mut() else {
        invalid("a live session is required");
        return std::ptr::null();
    };
    // Whatever has been scanned, in sheet order. The sweep's own list is in the
    // order it finished, which is the same thing today and need not stay so.
    let mut scanned: Vec<SheetChanges> = (1..=s.inner.page_count())
        .filter_map(|p| s.scans.get(&p).cloned())
        .collect();
    scanned.sort_by_key(|r| r.page_no);
    let text = s.inner.report(&scanned);
    s.report = CString::new(text.replace('\0', " ")).unwrap_or_default();
    s.report.as_ptr()
}

/// Whether a piece of text was added, removed, or says something different.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ScTextChangeKind {
    /// In the later revision only.
    Added = 0,
    /// In the earlier revision only.
    Removed = 1,
    /// In the same place on both, saying something different.
    Changed = 2,
    /// The same text, elsewhere on the sheet. Told apart from an addition and a
    /// removal because a re-laid-out sheet moves dozens of identical labels and
    /// reporting each twice buries the real changes.
    Moved = 3,
}

/// One difference in what the two revisions of a sheet say.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ScTextChange {
    pub kind: ScTextChangeKind,
    /// What the earlier revision said; an empty string for an addition.
    pub before: *const c_char,
    /// What the later one says; an empty string for a removal.
    pub after: *const c_char,
    /// Where it is, in page points.
    pub rect: ScRectF,
}

/// Works out what the two revisions of this sheet say differently, and returns
/// how many differences there are.
///
/// Negative on failure. Read them with [`sc_session_text_change`].
///
/// This complements the overlay rather than replacing it — the overlay finds a
/// re-routed wire that carries no text at all. It is, though, the answer to the
/// case the overlay handles worst: when two revisions went through different PDF
/// producers every glyph is drawn differently and the overlay reports about
/// twenty-five regions a sheet, while the words report the two that changed.
///
/// # Safety
/// `s` must be null or a live session.
#[no_mangle]
pub unsafe extern "C" fn sc_session_text_changes(s: *mut ScSession, page_no: i32) -> i32 {
    let Some(s) = s.as_mut() else {
        return invalid("a live session is required");
    };
    match s.inner.page_text_changes(page_no) {
        Ok(changes) => {
            s.text_changes = changes
                .into_iter()
                .map(|c| {
                    let before = CString::new(c.before.replace('\0', " ")).unwrap_or_default();
                    let after = CString::new(c.after.replace('\0', " ")).unwrap_or_default();
                    (c, before, after)
                })
                .collect();
            s.text_changes.len() as i32
        }
        Err(e) => fail(e),
    }
}

/// One of the differences found by the last [`sc_session_text_changes`] call.
///
/// # Safety
/// `s` must be null or a live session and `out` writable. **The strings `out`
/// points at are borrowed and stay valid only until the next
/// `sc_session_text_changes` call on this session, or until it is freed.**
#[no_mangle]
pub unsafe extern "C" fn sc_session_text_change(
    s: *const ScSession,
    index: usize,
    out: *mut ScTextChange,
) -> ScStatus {
    let (Some(s), false) = (s.as_ref(), out.is_null()) else {
        return invalid("a live session and a writable change are required");
    };
    match s.text_changes.get(index) {
        Some((c, before, after)) => {
            *out = ScTextChange {
                kind: match c.kind {
                    TextChangeKind::Added => ScTextChangeKind::Added,
                    TextChangeKind::Removed => ScTextChangeKind::Removed,
                    TextChangeKind::Changed => ScTextChangeKind::Changed,
                    TextChangeKind::Moved => ScTextChangeKind::Moved,
                },
                before: before.as_ptr(),
                after: after.as_ptr(),
                rect: c.rect.into(),
            };
            SC_OK
        }
        None => invalid("no text change with that index"),
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
