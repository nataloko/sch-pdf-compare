// Measured against the real revision sets. `samples/` holds customer documents
// and is never committed, so each test returns early when they are absent.
//
// The counts here are golden numbers: if one moves, the comparison's behaviour
// moved, and that wants understanding before the number is edited.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::*;
use std::path::PathBuf;

fn sample(name: &str) -> Option<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name);
    p.exists().then(|| p.to_string_lossy().into_owned())
}

const A10: &str = "SET-ONE - EXAMPLE DIGITAL REV-P1.pdf";
const B03: &str = "SET-ONE - EXAMPLE DIGITAL REV-P2.pdf";
const D02: &str = "SET-ONE - EXAMPLE DIGITAL REV-P3.pdf";

fn session(a: &str, b: &str) -> Option<Session> {
    let (pa, pb) = (sample(a)?, sample(b)?);
    Some(Session::open(&pa, &pb).expect("opens"))
}

#[test]
fn a_document_against_itself_has_nothing_to_report() {
    // The strongest invariant in the tool. If this ever fails, every other
    // number here is noise.
    let Some(s) = session(A10, A10) else { return };
    assert_eq!(s.page_count(), 21);
    for p in 1..=s.page_count() {
        let r = s.scan_page(p).expect("scans");
        assert!(r.changes.is_empty(), "sheet {p} differs from itself");
    }
}

#[test]
fn consecutive_revisions_of_the_same_producer() {
    let Some(s) = session(A10, B03) else { return };
    let r = s.scan_page(2).expect("scans");
    assert_eq!(r.changes.len(), 6, "sheet 2 of REV-P1 vs REV-P2");

    let mut changed = 0;
    let mut regions = 0;
    for p in 1..=s.page_count() {
        let r = s.scan_page(p).expect("scans");
        if !r.changes.is_empty() {
            changed += 1;
            regions += r.changes.len();
        }
    }
    // Every sheet reports something, and that is the point of the excluded
    // regions: the set shares a title block whose revision and date changed, so
    // 21 of 21 sheets are "changed" until the reader says otherwise.
    assert_eq!((changed, regions), (21, 59));
}

#[test]
fn across_pdf_producers_the_floor_is_rendering_not_alignment() {
    // REV-P2 came out of Ghostscript, REV-P3 out of Microsoft Print to PDF with
    // CID TrueType fonts where the others use subset Type1C. The residue is
    // glyph rasterisation spread over the whole sheet, not a shifted page:
    // probing every offset in +/-3 device pixels never gets below 14 regions,
    // and there is no minimum to find — the surface is flat. Tolerance is still
    // worth having (it takes the unmatched ink from 7809 px to 1969), but it
    // cannot make two typefaces into one. This is the case the text-level diff
    // exists for.
    let Some(s) = session(B03, D02) else { return };
    let r = s.scan_page(2).expect("scans");
    assert_eq!(r.changes.len(), 26, "sheet 2 of REV-P2 vs REV-P3");
}

#[test]
fn tolerance_cuts_the_fringe() {
    let Some(mut s) = session(B03, D02) else {
        return;
    };
    let mut strict = s.options();
    strict.tolerance = 0;
    s.set_options(strict);
    let raw = s.scan_page(2).expect("scans").pixels;

    let mut loose = s.options();
    loose.tolerance = 1;
    s.set_options(loose);
    let slack = s.scan_page(2).expect("scans").pixels;

    assert!(
        slack * 3 < raw,
        "1px of slack should absorb most of the fringe: {raw} -> {slack}"
    );
}

#[test]
fn an_excluded_region_is_counted_never_dropped() {
    let Some(mut s) = session(A10, B03) else {
        return;
    };
    let before = s.scan_page(2).expect("scans");
    assert_eq!(before.ignored, 0);

    // The title block sits in the bottom right corner of an A4 landscape sheet.
    let (w, h) = s.page_size(2).expect("has a sheet 2");
    s.add_ignore_rect(RectF::new(w * 0.75, h * 0.9, w * 0.25, h * 0.1));

    let after = s.scan_page(2).expect("scans");
    assert!(
        after.changes.len() < before.changes.len(),
        "the excluded region stopped counting"
    );
    assert!(
        after.ignored > 0,
        "and it is reported as excluded, not silently gone"
    );
    assert_eq!(
        after.changes.len() + after.ignored as usize,
        before.changes.len()
    );

    s.clear_ignore_rects();
    assert_eq!(
        s.scan_page(2).expect("scans").changes.len(),
        before.changes.len()
    );
}

#[test]
fn an_unmatched_sheet_is_not_an_unchanged_one() {
    // Nudge the pairing so A's sheet 1 has no counterpart. It has to come back
    // as entirely removed, not as a quiet page with nothing on it.
    let Some(mut s) = session(A10, B03) else {
        return;
    };
    s.set_page_delta(1);
    assert_eq!(s.page_delta(), 1);
    let p = s.pair(1);
    assert_eq!((p.page_a, p.page_b), (0, 1));
    let r = s.scan_page(1).expect("scans");
    assert!(
        !r.changes.is_empty(),
        "a sheet present in only one revision is all change"
    );
}

#[test]
fn a_tile_reports_the_same_changes_as_the_whole_sheet() {
    // Compose the whole sheet, then compose a tile out of the middle of it: the
    // two must agree about which pixels are coloured, so what the reader sees
    // does not depend on how the viewport happened to cut the page up.
    //
    // Note this does *not* prove the render margin is doing its job — a real
    // sheet rarely has a stroke that shifted across the particular boundary this
    // tile draws, and the test still passes with the margin removed. The margin
    // is pinned by `the_render_margin_is_what_stops_a_tile_edge_inventing_a_change`
    // in `sc-diff`, where the geometry can be built to hit it.
    //
    // Agreement is on the *verdict*, not on the bytes, and that is deliberate:
    // MuPDF's image scaler picks its subsample factor from the destination, so a
    // tile overlapping an embedded raster can differ from the full-page render by
    // a few grey levels. Both documents are rendered with identical geometry so
    // they shift together and the comparison is unaffected — but a byte-for-byte
    // assertion here would be testing MuPDF's scaler, not the margin.
    use sc_render::Tile;
    let Some(s) = session(A10, B03) else { return };
    let zoom = 100.0 / 72.0;
    let (w, h) = s.page_device_size(2, zoom).expect("has a sheet 2");

    let whole = s
        .compose_tile(2, zoom, Tile::whole(w, h))
        .expect("composes");
    let t = Tile::new(320, 240, 200, 160);
    let part = s.compose_tile(2, zoom, t).expect("composes");

    for y in 0..t.height {
        for x in 0..t.width {
            assert_eq!(
                is_coloured(part.bgr_at(x, y)),
                is_coloured(whole.bgr_at(x + t.x, y + t.y)),
                "tile pixel ({x},{y}) disagrees with the full compose about being a change"
            );
        }
    }
}

/// Chroma, not darkness: shared artwork darkens every channel equally and only a
/// real difference pulls them apart.
fn is_coloured(p: [u8; 3]) -> bool {
    let lo = *p.iter().min().expect("three channels") as i32;
    let hi = *p.iter().max().expect("three channels") as i32;
    hi - lo > 40
}

#[test]
fn a_single_document_view_reproduces_that_document_untouched() {
    // `Tab` flips between the two as a blink comparator, so either side must be
    // exactly what opening that file on its own would show. Not "close" —
    // identical, or the flicker means something it should not.
    //
    // Note this cannot be phrased as "A-only has no colour": schematics draw
    // coloured nets, and the drawing's own colour is not the comparison's.
    use sc_diff::ViewMode;
    use sc_render::Tile;
    let Some(mut s) = session(A10, B03) else {
        return;
    };
    let zoom = 1.0;
    let (w, h) = s.page_device_size(2, zoom).expect("has a sheet 2");

    let pair = s.pair(2);
    let (doc_a, _) = s.docs();
    let raw = doc_a
        .render(pair.page_a, zoom, Tile::whole(w, h))
        .expect("renders");

    s.set_view_mode(ViewMode::OnlyA);
    let only_a = s
        .compose_tile(2, zoom, Tile::whole(w, h))
        .expect("composes");
    for y in 0..h {
        for x in 0..w {
            let i = y as usize * raw.stride() + (x * 3) as usize;
            let expect = [raw.samples()[i], raw.samples()[i + 1], raw.samples()[i + 2]];
            assert_eq!(only_a.bgr_at(x, y), expect, "A-only differs at ({x},{y})");
        }
    }
}

#[test]
fn the_overlay_colours_something_these_revisions_disagree_about() {
    use sc_diff::ViewMode;
    use sc_render::Tile;
    let Some(mut s) = session(A10, B03) else {
        return;
    };
    let zoom = 1.0;
    let (w, h) = s.page_device_size(2, zoom).expect("has a sheet 2");
    s.set_view_mode(ViewMode::Overlay);
    let out = s
        .compose_tile(2, zoom, Tile::whole(w, h))
        .expect("composes");
    let n = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| is_coloured(out.bgr_at(x, y)))
        .count();
    assert!(
        n > 100,
        "sheet 2 has six changes on it; the overlay showed {n} coloured pixels"
    );
}
