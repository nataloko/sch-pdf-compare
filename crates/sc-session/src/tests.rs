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
const SECOND_A07: &str = "SET-TWO - EXAMPLE SECOND REV-Q1.pdf";
const SECOND_B02: &str = "SET-TWO - EXAMPLE SECOND REV-Q2.pdf";

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

#[test]
fn the_sweep_finishes_and_agrees_with_scanning_by_hand() {
    // The whole point of the sweep is that it is the same answer, arrived at
    // without making the reader wait. If it ever disagrees with `scan_page`,
    // the number in the sidebar is a lie.
    let Some(s) = session(A10, B03) else { return };
    let mut sweep = s.start_sweep().expect("the platform gives us a wakeup");

    let mut collected: Vec<SheetChanges> = Vec::new();
    // Poll rather than block: this stands in for the frontend's event loop,
    // which wakes on the handle and reads the status.
    for _ in 0..600 {
        collected.extend(sweep.take_results());
        if sweep.status().finished {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    collected.extend(sweep.take_results());

    let st = sweep.status();
    assert!(st.finished, "the sweep finished");
    assert!(!st.running, "and says it is no longer running");
    assert_eq!(st.total, 21);
    assert_eq!(st.scanned, 21);
    assert_eq!(
        st.changed_sheets, 21,
        "this set's title block changed on every sheet"
    );
    assert_eq!(
        collected.len(),
        21,
        "and every sheet came back exactly once"
    );

    for r in &collected {
        let by_hand = s.scan_page(r.page_no).expect("scans");
        assert_eq!(
            r.changes.len(),
            by_hand.changes.len(),
            "sheet {} disagrees with a hand scan",
            r.page_no
        );
    }
    sweep.stop();
}

#[test]
fn a_sweep_can_be_stopped_before_it_finishes() {
    let Some(s) = session(A10, B03) else { return };
    let mut sweep = s.start_sweep().expect("starts");
    sweep.stop();
    // `stop` joins, so by the time it returns the thread is gone and nothing is
    // still holding a document open behind our back.
    assert!(!sweep.status().running);
}

#[test]
fn the_repeat_detector_finds_the_title_block_on_both_sets() {
    // Both sample sets share a title block whose date changed, so both should
    // be offered it — and the offer must land on the bottom-right of the sheet.
    for (a, b) in [(A10, B03), (SECOND_A07, SECOND_B02)] {
        let Some(s) = session(a, b) else { return };
        let all: Vec<_> = (1..=s.page_count())
            .map(|p| s.scan_page(p).expect("scans"))
            .collect();
        let offered = suggest_ignores(&all, s.page_count());
        assert!(
            !offered.is_empty(),
            "{a}: a change on every sheet should be offered"
        );

        let (w, h) = s.page_size(1).expect("has a sheet 1");
        assert!(
            offered.iter().any(|r| r.x > w * 0.5 && r.y > h * 0.8),
            "{a}: the offer should be the title block, got {offered:?}"
        );

        // Offered, never applied. Nothing is excluded until the reader says so,
        // because a net renamed across the whole set looks exactly like this and
        // hiding that is the worst thing this tool could do.
        assert!(
            s.ignore_rects().is_empty(),
            "suggesting must not exclude anything"
        );
        assert_eq!(s.scan_page(1).expect("scans").ignored, 0);
    }
}

#[test]
fn nothing_is_offered_when_nothing_repeats() {
    // A document against itself has no changes at all, so there is nothing to
    // suggest — and a detector that offered something here would be inventing.
    let Some(s) = session(A10, A10) else { return };
    let all: Vec<_> = (1..=s.page_count())
        .map(|p| s.scan_page(p).expect("scans"))
        .collect();
    assert!(suggest_ignores(&all, s.page_count()).is_empty());
}

#[test]
fn a_partial_sweep_offers_nothing() {
    // Suggesting from half a sweep would offer to hide whatever happened to be
    // scanned first, which is not the same question at all.
    let Some(s) = session(A10, B03) else { return };
    let few: Vec<_> = (1..=5).map(|p| s.scan_page(p).expect("scans")).collect();
    assert!(suggest_ignores(&few, s.page_count()).is_empty());
}
