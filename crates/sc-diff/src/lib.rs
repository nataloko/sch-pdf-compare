//! The comparison kernel: ink extraction, tolerance, compositing, clustering.
//!
//! Ported from `CompareCore.{h,cpp}` in the SumatraPDF fork this tool grew out
//! of, assertion for assertion. Deliberately knows nothing about PDF, threading
//! or a toolkit — it takes pixels and gives back pixels and rectangles, so it
//! can be tested without any of them.
//!
//! The one decision worth re-reading before changing anything here: **colour
//! only the difference in coverage, not the coverage.** Composition splits the
//! two ink values into `shared = min(a, b)`, drawn neutral black, and
//! `diff = |a - b|`, drawn in that side's colour.

#![forbid(unsafe_code)]

mod changes;
mod compose;
mod geom;
mod ink;
mod pixels;

pub mod pairing;

pub use changes::{find_changes, Change};
pub use compose::{compose, ink_plane, ink_row, TileMasks};
pub use geom::{Point, Rect, Rgb, Size};
pub use ink::{compose_ink, dilate_ink, ink_from_rgb, read_ink_row};
pub use pairing::{Pair, Pairing};
pub use pixels::{PixelFormat, Pixels, Tile};

/// Which of the three views the tile is composed for.
///
/// `OnlyA` and `OnlyB` exist so `Tab` can flip between them as a blink
/// comparator without the zoom or the scroll position moving.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum ViewMode {
    #[default]
    Overlay = 0,
    OnlyA,
    OnlyB,
}

impl ViewMode {
    pub const fn name(self) -> &'static str {
        match self {
            ViewMode::OnlyA => "A only",
            ViewMode::OnlyB => "B only",
            ViewMode::Overlay => "Overlay",
        }
    }
}

/// More slack than this stops distinguishing a moved stroke from a deleted one.
pub const MAX_TOLERANCE: i32 = 3;

/// How an excluded region is drawn: its artwork kept at this strength, edged in
/// this colour, so it reads as "not compared" rather than "nothing changed".
pub const MASK_INK_PERCENT: i32 = 40;
pub const MASK_EDGE_COLOR: Rgb = Rgb::new(0x60, 0x84, 0xb0);

/// Grid coarseness and the smallest cluster worth reporting, in scan pixels.
pub const CHANGE_CELL: i32 = 8;
pub const CHANGE_MIN_PIXELS: i32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Options {
    /// Ink colours for content found in only one of the two documents.
    pub only_a: Rgb,
    pub only_b: Rgb,
    /// How far, in device pixels, ink may sit from its counterpart and still
    /// count as the same artwork.
    ///
    /// 1 is the default and it matters more than anything else here: on a pair
    /// exported by two different PDF producers, most of the raw pixel difference
    /// is sub-pixel rasterisation fringe, and without this the overlay is
    /// unreadable.
    pub tolerance: i32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            only_a: Rgb::new(0xd8, 0x10, 0x10),
            only_b: Rgb::new(0x00, 0x96, 0x28),
            tolerance: 1,
        }
    }
}

#[cfg(test)]
mod tests;
