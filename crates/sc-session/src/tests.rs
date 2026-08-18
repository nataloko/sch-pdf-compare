// Measured against the real revision sets. `samples/` holds customer documents
// and is never committed, so each test returns early when they are absent.
//
// The counts here are golden numbers: if one moves, the comparison's behaviour
// moved, and that wants understanding before the number is edited.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::*;
use sc_fixture::Samples;

/// The named pair from `samples/sets.json`, or `None` when the drawings are not
/// on this machine. The sets are addressed by the role they play rather than by
/// name — see `sc_fixture::samples` for why.
fn session(set: &str) -> Option<Session> {
    let (a, b) = Samples::load()?.both(set)?;
    Some(Session::open(&a, &b).expect("opens"))
}

/// A number the manifest says this pair should produce.
fn expect(set: &str, key: &str) -> Option<i32> {
    Samples::load()?.number(set, key)
}

#[test]
fn a_document_against_itself_has_nothing_to_report() {
    // The strongest invariant in the tool. If this ever fails, every other
    // number here is noise.
    let (Some(s), Some(n)) = (session("identical"), expect("identical", "sheets")) else {
        return;
    };
    assert_eq!(s.page_count(), n);
    for p in 1..=s.page_count() {
        let r = s.scan_page(p).expect("scans");
        assert!(r.changes.is_empty(), "sheet {p} differs from itself");
    }
}

#[test]
fn consecutive_revisions_of_the_same_producer() {
    let Some(s) = session("same_producer") else {
        return;
    };
    let Some(sheet) = expect("same_producer", "probe_sheet") else {
        return;
    };
    let Some(want) = expect("same_producer", "regions_on_probe_sheet") else {
        return;
    };
    let r = s.scan_page(sheet).expect("scans");
    assert_eq!(
        r.changes.len() as i32,
        want,
        "the probe sheet of the same-producer pair"
    );

    let mut changed = 0i32;
    let mut regions = 0usize;
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
    let want_sheets = expect("same_producer", "changed_sheets").unwrap_or(changed);
    let want_regions = expect("same_producer", "total_regions").unwrap_or(regions as i32);
    assert_eq!((changed, regions as i32), (want_sheets, want_regions));
}

#[test]
fn across_pdf_producers_the_floor_is_rendering_not_alignment() {
    // The two sides of this pair went through different PDF producers, one of
    // them writing CID TrueType fonts where the other uses subset Type1C. The
    // residue is
    // glyph rasterisation spread over the whole sheet, not a shifted page:
    // probing every offset in +/-3 device pixels never gets below 14 regions,
    // and there is no minimum to find — the surface is flat. Tolerance is still
    // worth having (it takes the unmatched ink from 7809 px to 1969), but it
    // cannot make two typefaces into one. This is the case the text-level diff
    // exists for.
    let Some(s) = session("cross_producer") else {
        return;
    };
    let Some(sheet) = expect("cross_producer", "probe_sheet") else {
        return;
    };
    let Some(want) = expect("cross_producer", "regions_on_probe_sheet") else {
        return;
    };
    let r = s.scan_page(sheet).expect("scans");
    assert_eq!(
        r.changes.len() as i32,
        want,
        "the probe sheet of the cross-producer pair"
    );
}

#[test]
fn tolerance_cuts_the_fringe() {
    let Some(mut s) = session("cross_producer") else {
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
    let Some(mut s) = session("same_producer") else {
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
    let Some(mut s) = session("same_producer") else {
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
    let Some(s) = session("same_producer") else {
        return;
    };
    let zoom = 100.0 / 72.0;
    let (w, h) = s.page_device_size(2, zoom).expect("has a sheet 2");

    let whole = s
        .compose_tile(2, zoom, Tile::whole(w, h), s.view_mode())
        .expect("composes");
    let t = Tile::new(320, 240, 200, 160);
    let part = s.compose_tile(2, zoom, t, s.view_mode()).expect("composes");

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
    let Some(mut s) = session("same_producer") else {
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
        .compose_tile(2, zoom, Tile::whole(w, h), s.view_mode())
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
    let Some(mut s) = session("same_producer") else {
        return;
    };
    let zoom = 1.0;
    let (w, h) = s.page_device_size(2, zoom).expect("has a sheet 2");
    s.set_view_mode(ViewMode::Overlay);
    let out = s
        .compose_tile(2, zoom, Tile::whole(w, h), s.view_mode())
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
    let Some(s) = session("same_producer") else {
        return;
    };
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
    let Some(s) = session("same_producer") else {
        return;
    };
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
    for set in ["same_producer", "large"] {
        let Some(s) = session(set) else { return };
        let all: Vec<_> = (1..=s.page_count())
            .map(|p| s.scan_page(p).expect("scans"))
            .collect();
        let offered = suggest_ignores(&all, s.page_count());
        assert!(
            !offered.is_empty(),
            "{set}: a change on every sheet should be offered"
        );

        let (w, h) = s.page_size(1).expect("has a sheet 1");
        assert!(
            offered.iter().any(|r| r.x > w * 0.5 && r.y > h * 0.8),
            "{set}: the offer should be the title block, got {offered:?}"
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
    let Some(s) = session("identical") else {
        return;
    };
    let all: Vec<_> = (1..=s.page_count())
        .map(|p| s.scan_page(p).expect("scans"))
        .collect();
    assert!(suggest_ignores(&all, s.page_count()).is_empty());
}

#[test]
fn a_partial_sweep_offers_nothing() {
    // Suggesting from half a sweep would offer to hide whatever happened to be
    // scanned first, which is not the same question at all.
    let Some(s) = session("same_producer") else {
        return;
    };
    let few: Vec<_> = (1..=5).map(|p| s.scan_page(p).expect("scans")).collect();
    assert!(suggest_ignores(&few, s.page_count()).is_empty());
}

#[test]
fn automatic_matching_lines_up_the_real_sets() {
    // Every sample pair is in order and the same length, so the right answer is
    // the identity — which is exactly what makes it a good check: anything
    // clever enough to reorder them is too clever.
    for set in ["same_producer", "rotated", "large"] {
        let (Some(mut s), Some(n)) = (session(set), expect(set, "sheets")) else {
            return;
        };
        let t0 = std::time::Instant::now();
        s.auto_match().expect("matches");
        let took = t0.elapsed();

        assert!(
            s.pairing_is_automatic(),
            "{set}: the pairing is a content match"
        );
        assert_eq!(s.page_count(), n, "{set}: no sheet invented or lost");
        for p in 1..=n {
            let pair = s.pair(p);
            assert_eq!(
                (pair.page_a, pair.page_b),
                (p, p),
                "{set}: sheet {p} matched to {pair:?}"
            );
        }
        // Text only, nothing rendered. If this ever costs seconds it has stopped
        // being something to run on open.
        assert!(took.as_secs_f32() < 10.0, "{set}: matching took {took:?}");
    }
}

#[test]
fn automatic_matching_survives_a_different_pdf_producer() {
    // These two went through different PDF producers, one writing CID TrueType
    // fonts where the other uses subset Type1C. If the signature depended on how
    // the text was encoded rather than what it says, this is where it would
    // show.
    let Some(mut s) = session("cross_producer") else {
        return;
    };
    s.auto_match().expect("matches");
    for p in 1..=21 {
        assert_eq!((s.pair(p).page_a, s.pair(p).page_b), (p, p), "sheet {p}");
    }
}

#[test]
fn nudging_the_delta_replaces_an_automatic_match() {
    // The two mechanisms must not silently compose: an offset on top of a
    // content match is nudging an answer, and the reader could not tell which
    // they were looking at.
    let Some(mut s) = session("same_producer") else {
        return;
    };
    s.auto_match().expect("matches");
    assert!(s.pairing_is_automatic());
    s.set_page_delta(1);
    assert!(
        !s.pairing_is_automatic(),
        "a manual nudge takes the pairing back"
    );
    assert_eq!(s.pair(1).page_a, 0, "and behaves like a plain offset again");
}

#[test]
fn the_text_diff_is_where_a_cross_producer_pair_becomes_readable() {
    // The measurement this feature exists for. On sheet 2 the pixel comparison
    // reports 26 regions for the cross-producer pair and 6 for the same-producer
    // one — the difference between them is glyph rasterisation, not content.
    // The words show what actually changed, and the cross-producer pair should
    // come out no worse than the same-producer one.
    let Some(same) = session("same_producer") else {
        return;
    };
    let Some(cross) = session("cross_producer") else {
        return;
    };

    let same_px = same.scan_page(2).expect("scans").changes.len();
    let cross_px = cross.scan_page(2).expect("scans").changes.len();
    let same_txt = same.page_text_changes(2).expect("reads").len();
    let cross_txt = cross.page_text_changes(2).expect("reads").len();

    assert!(
        cross_px > same_px * 3,
        "the pixel floor is real: {same_px} vs {cross_px}"
    );
    assert!(
        cross_txt <= same_txt,
        "text should not punish a change of producer: {same_txt} vs {cross_txt}"
    );
    assert!(
        cross_txt < cross_px / 2,
        "and should be far quieter: {cross_txt} vs {cross_px}"
    );
}

#[test]
fn a_document_against_itself_says_nothing_changed() {
    let Some(s) = session("identical") else {
        return;
    };
    for p in 1..=s.page_count() {
        assert!(
            s.page_text_changes(p).expect("reads").is_empty(),
            "sheet {p}"
        );
    }
}

#[test]
fn an_unmatched_sheet_reads_as_entirely_removed() {
    let Some(mut s) = session("same_producer") else {
        return;
    };
    s.set_page_delta(1);
    let changes = s.page_text_changes(1).expect("reads");
    assert!(!changes.is_empty());
    assert!(
        changes
            .iter()
            .all(|c| c.kind == sc_diff::TextChangeKind::Added),
        "virtual sheet 1 is B's orphaned first sheet, so all of it is new"
    );
}

/// Scans the whole set the plain way, for tests that want the sweep's output
/// without the sweep.
fn scan_all(s: &Session) -> Vec<SheetChanges> {
    (1..=s.page_count())
        .map(|p| s.scan_page(p).expect("scans"))
        .collect()
}

#[test]
fn the_report_summarises_moves_rather_than_listing_them() {
    // Sheet 8 of this pair moves 354 labels and changes 10 things. Listing the
    // moves buries the changes, so the report counts them in a sentence.
    let Some(s) = session("same_producer") else {
        return;
    };
    let all = scan_all(&s);
    let r = s.report(&all);
    assert!(r.contains("moved without changing"), "moves are summarised");
    // And a sheet that was substantially re-laid-out says so rather than
    // stopping its table without explanation.
    assert!(r.contains("not listed"), "a truncated table admits it");
    assert!(r.contains("re-laid-out"));
}

#[test]
fn the_report_says_what_changed_and_where() {
    let Some(s) = session("same_producer") else {
        return;
    };
    let all = scan_all(&s);
    let r = s.report(&all);

    let Some(m) = Samples::load() else { return };
    let (pa, pb) = m
        .both("same_producer")
        .expect("the pair is on this machine");
    let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_owned();
    assert!(r.contains(&name(&pa)), "names the earlier revision");
    assert!(r.contains(&name(&pb)), "names the later one");
    let sheets = expect("same_producer", "sheets").unwrap_or(0);
    assert!(r.contains(&format!("{sheets} sheets compared")));
    assert!(r.contains("## Sheet 2"), "has a section per changed sheet");
    // The net renames are the point of the whole thing.
    let (Some(from), Some(to)) = (
        m.text("same_producer", "renamed_from"),
        m.text("same_producer", "renamed_to"),
    ) else {
        return;
    };
    assert!(
        r.contains(&format!("`{from}`")) && r.contains(&format!("`{to}`")),
        "spells out the renames"
    );
    assert!(
        r.contains("| Was | Is now |"),
        "as a table somebody can paste"
    );
}

#[test]
fn the_report_says_when_part_of_the_sheet_was_not_compared() {
    // A report that quietly omitted this would be actively misleading: someone
    // reading it would take "nothing changed there" from "we did not look".
    let Some(mut s) = session("same_producer") else {
        return;
    };
    let (w, h) = s.page_size(1).expect("has a sheet 1");
    s.add_ignore_rect(RectF::new(w * 0.7, h * 0.85, w * 0.3, h * 0.15));
    let all = scan_all(&s);
    let r = s.report(&all);
    assert!(r.contains("excluded from the comparison on every sheet"));
    assert!(r.contains("not compared"));
}

#[test]
fn the_report_admits_a_half_finished_scan() {
    let Some(s) = session("same_producer") else {
        return;
    };
    let few: Vec<_> = (1..=4).map(|p| s.scan_page(p).expect("scans")).collect();
    let r = s.report(&few);
    assert!(
        r.contains("Only 4 of 21 sheets"),
        "says so rather than reading as complete"
    );
}

#[test]
fn a_report_on_two_identical_documents_says_nothing_changed() {
    let Some(s) = session("identical") else {
        return;
    };
    let all = scan_all(&s);
    let r = s.report(&all);
    assert!(r.contains("Nothing changed."));
    assert!(!r.contains("## Sheet"), "and lists no sheets");
}

#[test]
fn a_net_name_cannot_break_the_table() {
    // Schematic text is full of characters Markdown treats as syntax. A `|` in a
    // net name would split a row and silently move every later column.
    let Some(s) = session("same_producer") else {
        return;
    };
    let all = scan_all(&s);
    let r = s.report(&all);
    for line in r
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| ---"))
    {
        assert_eq!(
            line.matches(" | ").count(),
            1,
            "row has more than two cells: {line}"
        );
    }
}

#[test]
fn two_sheets_of_different_paper_sizes_are_reported_as_such() {
    // A drawing set reissued at a different paper size. The comparison lays both
    // sides out to the first document's geometry and crops the second, so the
    // second sheet is measured against its own top-left corner and the count
    // that comes back means nothing.
    //
    // Measured on this pair: 18216 unmatched pixels on one side and 45693 on the
    // other came back as 7 regions, because the clustering bridges neighbouring
    // cells and a sheet that differs everywhere collapses into a few very large
    // ones. A small count reading as a nearly unchanged sheet is the worst way
    // for this to fail, so the sheet has to say that it is not comparable.
    let Some(s) = session("different_paper") else {
        return;
    };
    assert!(s.sheet_sizes_differ(1), "the sizes do differ");
    let r = s.scan_page(1).expect("scans");
    assert!(r.size_mismatch, "and the scan says so");
    assert!(
        r.coverage > 0.9,
        "and reports that the changes cover the sheet, got {:.2}",
        r.coverage
    );
}

#[test]
fn a_pair_of_matching_sheets_reports_no_size_mismatch() {
    for set in ["same_producer", "native_ecad"] {
        let Some(s) = session(set) else { return };
        assert!(!s.sheet_sizes_differ(1), "{set}: the sizes agree");
        assert!(!s.scan_page(1).expect("scans").size_mismatch, "{set}");
    }
}

#[test]
fn coverage_separates_a_few_edits_from_a_redrawn_sheet() {
    // The count of regions cannot do this on its own.
    let Some(s) = session("same_producer") else {
        return;
    };
    let Some(sheet) = expect("same_producer", "probe_sheet") else {
        return;
    };
    let r = s.scan_page(sheet).expect("scans");
    assert!(
        r.coverage < 0.25,
        "a few edits cover little of the sheet: {:.2}",
        r.coverage
    );

    let Some(bad) = session("different_paper") else {
        return;
    };
    assert!(bad.scan_page(1).expect("scans").coverage > 0.9);
}

#[test]
fn a_native_ecad_export_compares_cleanly() {
    // The sample sets this was built against all went through a print-to-PDF
    // path. This pair comes straight out of the ECAD tool instead, which is a
    // different kind of PDF entirely — the comparison has to hold there too.
    let Some(s) = session("native_ecad") else {
        return;
    };
    let Some(n) = expect("native_ecad", "sheets") else {
        return;
    };
    assert_eq!(s.page_count(), n);
    // Sheet 1 changed, but it is a revision of the same drawing, not a redraw.
    let r = s.scan_page(1).expect("scans");
    assert!(!r.changes.is_empty(), "consecutive revisions do differ");
    assert!(
        r.coverage < 0.25,
        "but sheet 1 is not redrawn: {:.2}",
        r.coverage
    );
}

#[test]
fn the_report_leads_with_a_size_mismatch() {
    let Some(s) = session("different_paper") else {
        return;
    };
    let scanned: Vec<_> = (1..=3).map(|p| s.scan_page(p).expect("scans")).collect();
    let r = s.report(&scanned);
    assert!(
        r.contains("a different size in the two revisions"),
        "the report says so"
    );
    assert!(
        r.contains("same paper size"),
        "and says what to do about it"
    );
    // Before the first sheet's own account, so nobody reads the counts first.
    let warning = r.find("a different size").expect("present");
    let first_sheet = r.find("## Sheet").expect("present");
    assert!(warning < first_sheet, "the warning comes first");
}
