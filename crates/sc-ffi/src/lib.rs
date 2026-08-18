//! The C ABI. The only seam between the Rust core and the Qt shell.
//!
//! Three rules, and they are worth more than any convenience:
//!
//! 1. Every fallible call returns an [`ScStatus`] — 0 for success, negative for
//!    failure. `sc_last_error()` gives the sentence to put in front of a
//!    person.
//! 2. Everything handed back is *borrowed*, and every function says until when.
//!    The count of `_free` functions should stay near zero.
//! 3. Nothing spawns a thread the frontend does not know about. The background
//!    sweep is the one exception, and it is exposed the honest way: a wakeup
//!    handle to watch and a status to poll, never a callback fired from a
//!    worker thread onto code that wants to open a dialog.
//!
//! Nothing Qt-shaped crosses: no widgets, no window handles, no fonts, no
//! glyphs.

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

/// The library version, as a NUL-terminated string that lives forever.
///
/// # Safety
/// Callable with no preconditions. The returned pointer is valid for the
/// lifetime of the process and must not be freed.
#[no_mangle]
pub extern "C" fn sc_version() -> *const core::ffi::c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const core::ffi::c_char
}
