//! The model: a pair of documents and everything the frontend asks about it.
//!
//! Owns the pairing, the compare options, the excluded regions and the scan
//! cache. The shell asks this for tiles and for answers; it does no thinking of
//! its own.

#![forbid(unsafe_code)]

mod scan;

pub use scan::{SheetChanges, SCAN_DPI};

use sc_diff::{Options, Pair, Pairing, RectF, ViewMode};
use sc_render::{Document, Error, Result};

/// One open comparison: two revisions of the same drawing set.
pub struct Session {
    doc_a: Document,
    doc_b: Document,
    path_a: String,
    path_b: String,
    pairing: Pairing,
    options: Options,
    view_mode: ViewMode,
    /// Excluded regions, in page points, applied to **every** sheet — which is
    /// what a shared title block needs.
    ignore_rects: Vec<RectF>,
}

impl Session {
    pub fn open(path_a: &str, path_b: &str) -> Result<Self> {
        let doc_a = Document::open(path_a)?;
        let doc_b = Document::open(path_b)?;
        let pairing = Pairing::build(doc_a.page_count(), doc_b.page_count(), 0);
        Ok(Self {
            doc_a,
            doc_b,
            path_a: path_a.to_owned(),
            path_b: path_b.to_owned(),
            pairing,
            options: Options::default(),
            view_mode: ViewMode::default(),
            ignore_rects: Vec::new(),
        })
    }

    pub fn path_a(&self) -> &str {
        &self.path_a
    }

    pub fn path_b(&self) -> &str {
        &self.path_b
    }

    /// How many virtual sheets the comparison has. Not either document's own
    /// count: the range has to cover both, so a set that gained sheets at the
    /// front is longer than either.
    pub fn page_count(&self) -> i32 {
        self.pairing.page_count
    }

    pub fn pair(&self, page_no: i32) -> Pair {
        self.pairing.at(page_no)
    }

    pub fn page_delta(&self) -> i32 {
        self.pairing.page_delta
    }

    /// Nudges which sheet of B lines up with which sheet of A. The scan cache is
    /// the caller's to drop; every cached answer is about the old pairing.
    pub fn set_page_delta(&mut self, delta: i32) {
        self.pairing = Pairing::build(self.doc_a.page_count(), self.doc_b.page_count(), delta);
    }

    pub fn options(&self) -> Options {
        self.options
    }

    pub fn set_options(&mut self, o: Options) {
        self.options = o;
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, m: ViewMode) {
        self.view_mode = m;
    }

    pub fn ignore_rects(&self) -> &[RectF] {
        &self.ignore_rects
    }

    pub fn add_ignore_rect(&mut self, r: RectF) {
        self.ignore_rects.push(r);
    }

    pub fn clear_ignore_rects(&mut self) {
        self.ignore_rects.clear();
    }

    /// The virtual sheet's size in points.
    ///
    /// Taken from whichever document has the sheet, A first. A pair whose two
    /// sheets differ slightly in size — which happens when the revisions went
    /// through different PDF producers — is laid out to A's, and B is cropped or
    /// padded to match rather than being scaled to fit. Scaling would move every
    /// stroke on the sheet and turn a rounding difference into a page full of
    /// changes.
    pub fn page_size(&self, page_no: i32) -> Result<(f32, f32)> {
        let p = self.pairing.at(page_no);
        if p.page_a != 0 {
            self.doc_a.page_size(p.page_a)
        } else if p.page_b != 0 {
            self.doc_b.page_size(p.page_b)
        } else {
            Err(Error::NoSuchPage(page_no))
        }
    }

    pub(crate) fn docs(&self) -> (&Document, &Document) {
        (&self.doc_a, &self.doc_b)
    }
}

#[cfg(test)]
mod tests;
