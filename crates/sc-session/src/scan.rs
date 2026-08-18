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
    /// The two sheets are not the same size on paper.
    ///
    /// This is not a detail. The comparison lays both sides out to the first
    /// document's geometry and crops the second, so an A3 sheet compared against
    /// its A4 reissue is compared against its own top-left corner. The ink then
    /// has nothing to do with the ink it is being measured against, and the
    /// count that comes back is meaningless — which is far worse than a large
    /// count, because it reads as a nearly unchanged sheet.
    pub size_mismatch: bool,
    /// How much of the sheet the change regions cover, from 0 to 1.
    ///
    /// The count of regions alone cannot tell "seven small edits" from "the
    /// whole sheet is different": the clustering deliberately bridges
    /// neighbouring cells, so a sheet that differs everywhere collapses into a
    /// handful of very large regions. Measured on a real mismatched pair, 64000
    /// unmatched pixels came back as 7 regions.
    pub coverage: f32,
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
        let mut out = self.to_page_space(page_no, &found, zoom);
        out.size_mismatch = self.sheet_sizes_differ(page_no);
        // Area of the regions against the area of the sheet. Overlapping boxes
        // are counted twice, which can only make this larger, so it is capped —
        // it is a signal that a sheet is substantially different, not a
        // measurement.
        let sheet = (uw * uh) as f32;
        let covered: f32 = out
            .changes
            .iter()
            .map(|r| (r.dx * zoom) * (r.dy * zoom))
            .sum();
        out.coverage = (covered / sheet).clamp(0.0, 1.0);
        Ok(out)
    }

    /// True when the two sheets of a pair are not the same size on paper.
    ///
    /// A per-cent of tolerance, because a sheet that went through two PDF
    /// producers can come back a fraction of a point different, and that is not
    /// what this is for.
    pub fn sheet_sizes_differ(&self, page_no: i32) -> bool {
        let pair = self.pair(page_no);
        if pair.page_a == 0 || pair.page_b == 0 {
            return false;
        }
        let (doc_a, doc_b) = self.docs();
        let (Ok((aw, ah)), Ok((bw, bh))) =
            (doc_a.page_size(pair.page_a), doc_b.page_size(pair.page_b))
        else {
            return false;
        };
        let off = |x: f32, y: f32| (x - y).abs() > 0.01 * x.max(y).max(1.0);
        off(aw, bw) || off(ah, bh)
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
