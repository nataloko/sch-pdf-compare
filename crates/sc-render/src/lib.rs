//! MuPDF: open a document, ask a sheet its size, render a rect of it to pixels.
//!
//! Deliberately narrow. The comparison needs three things from a PDF — how many
//! sheets, how big each one is, and what a given rectangle of one looks like as
//! pixels — plus its text, for matching sheets between two revisions. Everything
//! else MuPDF can do is somebody else's problem.
//!
//! Pixels come back as `Bgr8`, which is what MuPDF's `DeviceBGR` produces and
//! what both the comparison kernel and Qt read without a swizzle.

#![forbid(unsafe_code)]

mod error;

pub use error::{Error, Result};

pub use sc_diff::{RectF, Word};

use mupdf::{Colorspace, Device, IRect, Matrix, Pixmap};

/// One open document. Not `Sync`: MuPDF's context is per-thread, so the sweep
/// opens its own handles rather than sharing these.
pub struct Document {
    doc: mupdf::Document,
    page_count: i32,
}

impl Document {
    pub fn open(path: &str) -> Result<Self> {
        let doc = mupdf::Document::open(path).map_err(|e| Error::Io(e.to_string()))?;
        let page_count = doc.page_count()?;
        Ok(Self { doc, page_count })
    }

    /// Number of sheets. 1-based everywhere a page number is taken.
    pub fn page_count(&self) -> i32 {
        self.page_count
    }

    /// The sheet's size in points, **after** any `/Rotate` the file carries.
    ///
    /// This matters for real revision sets: one of the sample documents was
    /// exported landscape where its predecessors were portrait, and the two
    /// still draw the same picture. Asking MuPDF for the bound page rather than
    /// reading the media box is what makes that a non-event.
    pub fn page_size(&self, page_no: i32) -> Result<(f32, f32)> {
        let page = self.load(page_no)?;
        let b = page.bounds()?;
        Ok((b.x1 - b.x0, b.y1 - b.y0))
    }

    /// The sheet's text, in reading order. Used to match sheets between two
    /// revisions; never shown to anybody as-is.
    pub fn page_text(&self, page_no: i32) -> Result<String> {
        let page = self.load(page_no)?;
        Ok(page.text(mupdf::TextExtractOptions::default())?)
    }

    /// The sheet's words, each with where it sits on the page in points.
    ///
    /// This is what a text-level comparison works from. It is worth far more
    /// than it looks on a schematic: when two revisions came out of different
    /// PDF producers most of the *pixel* difference is glyph rasterisation, and
    /// comparing what the text says instead of how it was drawn sidesteps that
    /// entirely.
    pub fn page_words(&self, page_no: i32) -> Result<Vec<Word>> {
        let page = self.load(page_no)?;
        let words = page.words(mupdf::TextExtractOptions::default())?;
        Ok(words
            .into_iter()
            .map(|w| {
                Word::new(
                    w.text,
                    RectF::new(
                        w.bounds.x0,
                        w.bounds.y0,
                        w.bounds.x1 - w.bounds.x0,
                        w.bounds.y1 - w.bounds.y0,
                    ),
                )
            })
            .collect())
    }

    /// The whole sheet in device pixels at `zoom`, as MuPDF would round it.
    ///
    /// The viewer needs this to lay pages out before it has rendered any of
    /// them, and the tile coordinates passed to [`Document::render`] are
    /// relative to this rectangle's top-left.
    pub fn page_device_size(&self, page_no: i32, zoom: f32) -> Result<(i32, i32)> {
        let full = self.full_rect(page_no, zoom)?;
        Ok((full.x1 - full.x0, full.y1 - full.y0))
    }

    /// Renders `tile` — device pixels, origin at the sheet's top-left at this
    /// zoom — onto white.
    ///
    /// Onto white, never onto transparency: the comparison reads coverage as
    /// "how far from paper is this pixel", and an alpha channel would make every
    /// blank pixel ambiguous.
    pub fn render(&self, page_no: i32, zoom: f32, tile: Tile) -> Result<Raster> {
        if tile.width <= 0 || tile.height <= 0 {
            return Err(Error::BadGeometry);
        }
        let page = self.load(page_no)?;
        let full = self.full_rect(page_no, zoom)?;

        // Scale, then shift the sheet's top-left to the device origin, then shift
        // again by the tile's own position. Written out rather than composed
        // from helpers because the order is the whole content of it: a point
        // (x, y) lands at (zoom*x - full.x0 - tile.x, zoom*y - full.y0 - tile.y).
        let ctm = Matrix {
            a: zoom,
            b: 0.0,
            c: 0.0,
            d: zoom,
            e: -(full.x0 + tile.x) as f32,
            f: -(full.y0 + tile.y) as f32,
        };

        let bbox = IRect {
            x0: 0,
            y0: 0,
            x1: tile.width,
            y1: tile.height,
        };
        let cs = Colorspace::device_bgr();
        let mut pixmap = Pixmap::new_with_rect(&cs, bbox, false)?;
        // 0xff is white in every channel. Any pixel the page does not draw on has
        // to read as paper, including the margin past a short page.
        pixmap.clear_with(0xff)?;
        {
            let device = Device::from_pixmap(&pixmap)?;
            page.run(&device, &ctm)?;
        }
        Ok(Raster { pixmap })
    }

    fn load(&self, page_no: i32) -> Result<mupdf::Page> {
        if page_no < 1 || page_no > self.page_count {
            return Err(Error::NoSuchPage(page_no));
        }
        Ok(self.doc.load_page(page_no - 1)?)
    }

    fn full_rect(&self, page_no: i32, zoom: f32) -> Result<IRect> {
        let page = self.load(page_no)?;
        let ctm = Matrix::new_scale(zoom, zoom);
        Ok(page.bounds()?.transform(&ctm).round())
    }
}

/// A rectangle of a sheet to render, in device pixels at the requested zoom,
/// with its origin at the sheet's top-left.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Tile {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The whole sheet, given its device size.
    pub const fn whole(width: i32, height: i32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }
}

/// Rendered pixels, still owned by MuPDF.
///
/// Kept rather than copied out: the comparison reads these straight through a
/// borrowed view, and the shell wraps the composed result — not this — in a
/// `QImage`. One less copy on the path that runs 85 times a sweep.
pub struct Raster {
    pixmap: Pixmap,
}

impl Raster {
    pub fn width(&self) -> i32 {
        self.pixmap.width() as i32
    }

    pub fn height(&self) -> i32 {
        self.pixmap.height() as i32
    }

    /// Bytes per row. MuPDF pads rows, so this is not always `width * 3`.
    pub fn stride(&self) -> usize {
        self.pixmap.stride() as usize
    }

    /// B, G, R per pixel, `stride` bytes per row.
    pub fn samples(&self) -> &[u8] {
        self.pixmap.samples()
    }
}

#[cfg(test)]
mod tests;
