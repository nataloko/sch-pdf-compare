// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::Session;
use sc_diff::{find_changes, ink_plane, Change, PixelFormat, Pixels, RectF};
use sc_render::{Document, Result, Tile};

/// The resolution every sheet is scanned at, whatever the reader is zoomed to.
///
/// Fixed on purpose. The change *list* must not depend on the view, or the same
/// document answers "how many sheets changed" differently depending on what the
/// reader happened to be looking at. 100 dpi is enough to find a changed
/// component and cheap enough to sweep 85 sheets.
pub const SCAN_DPI: f32 = 100.0;

/// What one sheet's scan found, in page points.
#[derive(Clone, Debug, Default)]
pub struct SheetChanges {
    pub page_no: i32,
    pub changes: Vec<RectF>,
    /// Unmatched ink, summed over the regions. A rough "how much changed".
    pub pixels: i32,
    /// Regions that were found but fall inside an excluded rectangle. Counted,
    /// never silently dropped: "not compared" must not read as "unchanged".
    pub ignored: i32,
}

impl Session {
    /// Scans one virtual sheet for regions where the two documents disagree.
    ///
    /// Renders both sides at [`SCAN_DPI`] and compares the ink. Costs roughly a
    /// third of a second a sheet, which is why the sweep runs in the background
    /// rather than on the way to drawing something.
    pub fn scan_page(&self, page_no: i32) -> Result<SheetChanges> {
        let zoom = SCAN_DPI / 72.0;
        let pair = self.pair(page_no);
        if pair.page_a == 0 && pair.page_b == 0 {
            return Ok(SheetChanges {
                page_no,
                ..Default::default()
            });
        }
        let (doc_a, doc_b) = self.docs();

        // One geometry for both sides. Whichever document has the sheet decides
        // it, A first; the other is cropped or padded to match.
        let (w, h) = if pair.page_a != 0 {
            doc_a.page_device_size(pair.page_a, zoom)?
        } else {
            doc_b.page_device_size(pair.page_b, zoom)?
        };
        if w <= 0 || h <= 0 {
            return Ok(SheetChanges {
                page_no,
                ..Default::default()
            });
        }

        let ra = render_or_blank(doc_a, pair.page_a, zoom, w, h)?;
        let rb = render_or_blank(doc_b, pair.page_b, zoom, w, h)?;
        let (uw, uh) = (w as usize, h as usize);
        let ink_a = ink_plane(as_pixels(ra.as_ref(), w, h).as_ref(), uw, uh);
        let ink_b = ink_plane(as_pixels(rb.as_ref(), w, h).as_ref(), uw, uh);

        let found = find_changes(&ink_a, &ink_b, uw, uh, &self.options());
        Ok(self.to_page_space(page_no, &found, zoom))
    }

    /// Converts a scan's boxes to page points and splits off the ones the reader
    /// has excluded.
    fn to_page_space(&self, page_no: i32, found: &[Change], zoom: f32) -> SheetChanges {
        let mut out = SheetChanges {
            page_no,
            ..Default::default()
        };
        for c in found {
            let r = RectF::from_device(c.box_, zoom);
            if self.ignore_rects().iter().any(|ig| ig.contains_rect(&r)) {
                out.ignored += 1;
                continue;
            }
            out.pixels += c.pixels;
            out.changes.push(r);
        }
        out
    }
}

/// Renders the sheet, or nothing at all when this document has no counterpart.
///
/// `None` rather than a white image: the comparison distinguishes "this sheet is
/// blank here" from "this document has no such sheet", and only the second one
/// should paint the whole page in the other side's colour.
fn render_or_blank(
    doc: &Document,
    page_no: i32,
    zoom: f32,
    w: i32,
    h: i32,
) -> Result<Option<sc_render::Raster>> {
    if page_no == 0 {
        return Ok(None);
    }
    Ok(Some(doc.render(page_no, zoom, Tile::whole(w, h))?))
}

/// Borrows a render as the comparison kernel's view of it. MuPDF's `DeviceBGR`
/// is already the kernel's `Bgr8`, so this is a description, not a conversion.
fn as_pixels(r: Option<&sc_render::Raster>, w: i32, h: i32) -> Option<Pixels<'_>> {
    let r = r?;
    Pixels::new(
        r.samples(),
        w.min(r.width()),
        h.min(r.height()),
        r.stride(),
        PixelFormat::Bgr8,
    )
}
