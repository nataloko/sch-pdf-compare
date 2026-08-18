//! The comparison kernel: ink extraction, tolerance, compositing, clustering.
//!
//! Ported from `CompareCore.{h,cpp}` in the SumatraPDF fork this tool grew out
//! of. Deliberately knows nothing about PDF, threading or a toolkit — it takes
//! pixels and gives back pixels and rectangles, so it can be tested without any
//! of them.
