// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! A thing a worker thread can poke and an event loop can watch.
//!
//! This is the whole of the frontend's relationship with the background sweep.
//! The sweep never calls into the frontend: it writes its results down, pokes
//! this, and the frontend reads the results when its own event loop next gets
//! round to it. That asymmetry is deliberate — a callback arriving on a worker
//! thread is a callback that cannot raise a dialog, and the fork this tool grew
//! out of has a recorded bug where a "finished" notification arrived before the
//! thing it announced was true.

use std::sync::Arc;

/// The handle a frontend watches. On Unix a file descriptor for
/// `QSocketNotifier`; on Windows an event handle for `QWinEventNotifier`.
#[cfg(unix)]
pub type WakeupHandle = i32;
#[cfg(windows)]
pub type WakeupHandle = isize;

struct Inner {
    #[cfg(unix)]
    read: i32,
    #[cfg(unix)]
    write: i32,
    #[cfg(windows)]
    event: isize,
}

impl Drop for Inner {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::close(self.read);
            if self.write != self.read {
                libc::close(self.write);
            }
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.event as _);
        }
    }
}

/// Cloneable so the worker keeps one and the session keeps one; the underlying
/// object closes when the last of them goes.
#[derive(Clone)]
pub struct Wakeup {
    inner: Arc<Inner>,
}

impl Wakeup {
    pub fn new() -> Option<Self> {
        #[cfg(unix)]
        {
            // An eventfd would be one descriptor instead of two, but a pipe is
            // portable across every Unix and the frontend cannot tell the
            // difference — it watches a readable fd either way.
            let mut fds = [0i32; 2];
            // SAFETY: `fds` is two writable ints, which is what pipe() wants.
            let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
            if rc != 0 {
                return None;
            }
            // Non-blocking on both ends: the writer must never stall the sweep
            // because nobody has drained yet, and the reader must never block
            // the UI thread when there is nothing to take.
            for fd in fds {
                // SAFETY: `fd` is a descriptor pipe() just returned.
                unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                    libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            Some(Self {
                inner: Arc::new(Inner {
                    read: fds[0],
                    write: fds[1],
                }),
            })
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::CreateEventW;
            // Manual-reset, initially unset: the frontend drains and clears it
            // itself in `drain`, the same shape as reading a pipe empty.
            // SAFETY: null attributes and name are both valid arguments.
            let h = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if h.is_null() {
                return None;
            }
            Some(Self {
                inner: Arc::new(Inner { event: h as isize }),
            })
        }
    }

    /// What the frontend watches. Valid for as long as this `Wakeup` lives.
    pub fn handle(&self) -> WakeupHandle {
        #[cfg(unix)]
        {
            self.inner.read
        }
        #[cfg(windows)]
        {
            self.inner.event
        }
    }

    /// Called from the worker thread. Never blocks, and a poke that is lost
    /// because one is already pending is not a problem: the frontend re-reads
    /// the whole status, so one wakeup and ten say the same thing.
    pub fn poke(&self) {
        #[cfg(unix)]
        {
            let byte = 1u8;
            // SAFETY: writing one byte from a valid local to a descriptor we own.
            unsafe {
                libc::write(self.inner.write, std::ptr::addr_of!(byte).cast(), 1);
            }
        }
        #[cfg(windows)]
        {
            // SAFETY: `event` is a handle CreateEventW returned and we still own.
            unsafe {
                windows_sys::Win32::System::Threading::SetEvent(self.inner.event as _);
            }
        }
    }

    /// Called from the frontend after its event loop reports the handle ready.
    /// Clears the signal so it does not fire forever.
    pub fn drain(&self) {
        #[cfg(unix)]
        {
            let mut buf = [0u8; 64];
            // SAFETY: reading into a local buffer from a descriptor we own. The
            // fd is non-blocking, so this stops with EAGAIN rather than hanging.
            unsafe { while libc::read(self.inner.read, buf.as_mut_ptr().cast(), buf.len()) > 0 {} }
        }
        #[cfg(windows)]
        {
            // SAFETY: `event` is a handle CreateEventW returned and we still own.
            unsafe {
                windows_sys::Win32::System::Threading::ResetEvent(self.inner.event as _);
            }
        }
    }
}
