// End-to-end tests that need no customer drawings.
//
// The real sets live in `samples/`, are never committed, and carry most of this
// project's evidence — but a clone without them would otherwise exercise almost
// nothing above the pixel kernel. These build their own documents, so what they
// check is exact: one label changed, and one label is what comes back.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::*;
use sc_diff::TextChangeKind;
use sc_fixture::{write, Sheet};
use std::path::PathBuf;

/// A pair of files named after the test, so nothing races.
fn pair(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir();
    let a = dir.join(format!("sch-fx-{name}-a.pdf"));
    let b = dir.join(format!("sch-fx-{name}-b.pdf"));
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    (a, b)
}

fn open(a: &PathBuf, b: &PathBuf) -> Session {
    Session::open(&a.to_string_lossy(), &b.to_string_lossy()).expect("opens")
}

fn cleanup(a: &PathBuf, b: &PathBuf) {
    let _ = std::fs::remove_file(a);
    let _ = std::fs::remove_file(b);
}

/// Three sheets that look a little like a drawing set: a shared frame, a title
/// block in the bottom right, and something different on each.
fn set(rev: &str, values: &[&str]) -> Vec<Sheet> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Sheet::a4_landscape()
                .line(20.0, 20.0, 822.0, 20.0)
                .line(20.0, 575.0, 822.0, 575.0)
                .line(20.0, 20.0, 20.0, 575.0)
                .line(822.0, 20.0, 822.0, 575.0)
                // The title block, in the same place on every sheet, carrying
                // the revision — the thing that changes set-wide.
                .text(700.0, 35.0, &format!("SHEET {} REV {rev}", i + 1))
                .text(200.0, 400.0, &format!("NET_SHEET_{}", i + 1))
                .text(300.0, 300.0, v)
        })
        .collect()
}

#[test]
fn one_changed_value_is_found_and_named() {
    let (a, b) = pair("one-value");
    write(&a, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    write(&b, &set("A", &["12k", "4k7", "100nF"])).expect("writes");
    let s = open(&a, &b);

    assert_eq!(s.page_count(), 3);
    // The drawing differs, and only on the sheet that changed.
    assert!(!s.scan_page(1).expect("scans").changes.is_empty());
    assert!(s.scan_page(2).expect("scans").changes.is_empty());
    assert!(s.scan_page(3).expect("scans").changes.is_empty());

    // And it can be said in words, which is the answer worth having.
    let text = s.page_text_changes(1).expect("reads");
    assert_eq!(text.len(), 1, "one change, got {text:?}");
    assert_eq!(text[0].kind, TextChangeKind::Changed);
    assert_eq!(
        (text[0].before.as_str(), text[0].after.as_str()),
        ("10k", "12k")
    );
    cleanup(&a, &b);
}

#[test]
fn identical_sets_report_nothing_at_all() {
    let (a, b) = pair("identical");
    write(&a, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    write(&b, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    let s = open(&a, &b);
    for p in 1..=3 {
        assert!(
            s.scan_page(p).expect("scans").changes.is_empty(),
            "sheet {p}"
        );
        assert!(
            s.page_text_changes(p).expect("reads").is_empty(),
            "sheet {p}"
        );
    }
    cleanup(&a, &b);
}

#[test]
fn a_revision_that_changed_the_title_block_marks_every_sheet() {
    // The problem excluded regions exist for, reproduced exactly: the only
    // difference is the revision letter, and it is on all three sheets.
    let (a, b) = pair("titleblock");
    write(&a, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    write(&b, &set("B", &["10k", "4k7", "100nF"])).expect("writes");
    let mut s = open(&a, &b);

    let all: Vec<_> = (1..=3).map(|p| s.scan_page(p).expect("scans")).collect();
    assert!(
        all.iter().all(|r| !r.changes.is_empty()),
        "every sheet is marked"
    );

    // Excluding the title block clears all three, and says it excluded them.
    let (w, h) = s.page_size(1).expect("has a sheet 1");
    s.add_ignore_rect(RectF::new(w * 0.75, h * 0.85, w * 0.25, h * 0.15));
    for p in 1..=3 {
        let r = s.scan_page(p).expect("scans");
        assert!(r.changes.is_empty(), "sheet {p} still marked: {r:?}");
        assert!(r.ignored > 0, "sheet {p} does not say it excluded anything");
    }
    cleanup(&a, &b);
}

#[test]
fn a_sheet_inserted_in_the_middle_is_matched_by_content() {
    // What a uniform offset cannot express: the insert is in the middle, so the
    // sheets before it line up and the ones after it are pushed along by one.
    let (a, b) = pair("inserted");
    write(&a, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    let mut later = set("A", &["10k", "4k7", "100nF"]);
    later.insert(
        1,
        Sheet::a4_landscape()
            .text(700.0, 35.0, "SHEET NEW REV A")
            .text(200.0, 400.0, "BRAND_NEW_NET_HERE")
            .text(300.0, 300.0, "47uH"),
    );
    write(&b, &later).expect("writes");

    let mut s = open(&a, &b);
    s.auto_match().expect("matches");
    assert!(s.pairing_is_automatic());
    assert_eq!(s.page_count(), 4, "three sheets and the new one");

    let pairs: Vec<_> = (1..=4).map(|p| s.pair(p)).collect();
    let added: Vec<_> = pairs.iter().filter(|p| p.page_a == 0).collect();
    assert_eq!(added.len(), 1, "exactly one sheet is new: {pairs:?}");
    assert_eq!(added[0].page_b, 2, "and it is the one that was inserted");
    // The rest line up one to one despite the shift.
    assert!(pairs.iter().any(|p| p.page_a == 2 && p.page_b == 3));
    assert!(pairs.iter().any(|p| p.page_a == 3 && p.page_b == 4));
    cleanup(&a, &b);
}

#[test]
fn the_report_of_a_known_change_says_the_known_thing() {
    let (a, b) = pair("report");
    write(&a, &set("A", &["10k", "4k7", "100nF"])).expect("writes");
    write(&b, &set("A", &["10k", "4k7", "220nF"])).expect("writes");
    let s = open(&a, &b);
    let scanned: Vec<_> = (1..=3).map(|p| s.scan_page(p).expect("scans")).collect();
    let r = s.report(&scanned);

    assert!(r.contains("## Sheet 3"), "the sheet that changed is in it");
    assert!(
        !r.contains("## Sheet 1"),
        "and the ones that did not are not"
    );
    assert!(
        r.contains("`100nF`") && r.contains("`220nF`"),
        "with both readings:\n{r}"
    );
    cleanup(&a, &b);
}
