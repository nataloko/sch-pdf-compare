// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::*;

fn sigs(pages: &[&str]) -> Vec<Signature> {
    pages.iter().map(|t| Signature::from_text(t)).collect()
}

/// Sheets that look like schematic sheets: a few shared frame words and some
/// net names of their own.
fn sheet(unique: &str) -> String {
    format!("Example Project Sheet Size Date Rev {unique}")
}

#[test]
fn identical_sets_match_one_to_one() {
    let pages: Vec<String> = (0..8)
        .map(|i| sheet(&format!("NET_{i}A NET_{i}B DEV_{i}")))
        .collect();
    let refs: Vec<&str> = pages.iter().map(|s| s.as_str()).collect();
    let a = sigs(&refs);
    let pairs = match_sheets(&a, &a);
    assert_eq!(pairs.len(), 8);
    for (i, p) in pairs.iter().enumerate() {
        assert_eq!((p.page_a, p.page_b), (i as i32 + 1, i as i32 + 1));
    }
}

#[test]
fn a_sheet_inserted_at_the_front_shows_as_added() {
    let base: Vec<String> = (0..5).map(|i| sheet(&format!("NET_{i} DEV_{i}"))).collect();
    let a: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
    let extra = sheet("BRAND_NEW_SHEET WITH_ITS_OWN NETS_HERE");
    let mut b: Vec<&str> = vec![extra.as_str()];
    b.extend(a.iter().copied());

    let pairs = match_sheets(&sigs(&a), &sigs(&b));
    assert_eq!(pairs.len(), 6);
    assert_eq!(
        (pairs[0].page_a, pairs[0].page_b),
        (0, 1),
        "the new sheet is added"
    );
    for (i, p) in pairs.iter().enumerate().skip(1) {
        assert_eq!((p.page_a, p.page_b), (i as i32, i as i32 + 1));
    }
}

#[test]
fn a_removed_sheet_shows_as_removed() {
    let base: Vec<String> = (0..5).map(|i| sheet(&format!("NET_{i} DEV_{i}"))).collect();
    let a: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
    let b: Vec<&str> = a.iter().copied().skip(2).collect();

    let pairs = match_sheets(&sigs(&a), &sigs(&b));
    assert_eq!((pairs[0].page_a, pairs[0].page_b), (1, 0));
    assert_eq!((pairs[1].page_a, pairs[1].page_b), (2, 0));
    assert_eq!((pairs[2].page_a, pairs[2].page_b), (3, 1));
}

#[test]
fn reordered_sheets_are_matched_by_content_where_order_allows() {
    // The case a uniform offset cannot express at all: one sheet moved to the
    // end. Alignment cannot cross sheets over, so it reports the move as a
    // removal and an addition — which is the truthful answer, and puts both
    // ends of the move in front of the reader.
    let base: Vec<String> = (0..5).map(|i| sheet(&format!("NET_{i} DEV_{i}"))).collect();
    let a: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
    let mut b: Vec<&str> = a.clone();
    let moved = b.remove(1);
    b.push(moved);

    let pairs = match_sheets(&sigs(&a), &sigs(&b));
    let matched: Vec<_> = pairs
        .iter()
        .filter(|p| p.page_a != 0 && p.page_b != 0)
        .collect();
    assert_eq!(
        matched.len(),
        4,
        "the four sheets that stayed put are matched"
    );
    assert!(
        pairs.iter().any(|p| p.page_a == 2 && p.page_b == 0),
        "sheet 2 left its place"
    );
    assert!(
        pairs.iter().any(|p| p.page_a == 0 && p.page_b == 5),
        "and turned up at the end"
    );
}

#[test]
fn near_duplicate_sheets_are_kept_in_order() {
    // The hard case, and the reason this is an alignment and not a best-score
    // search: eight channel sheets that differ only in one number. Greedy
    // matching pairs sheet 3 with whichever scored highest and can never take it
    // back; ordering is what actually separates them.
    let pages: Vec<String> = (0..8)
        .map(|i| sheet(&format!("ETHERNET PORT CHANNEL MAGJACK RJ45 PORT_{i}")))
        .collect();
    let refs: Vec<&str> = pages.iter().map(|s| s.as_str()).collect();
    let a = sigs(&refs);
    let pairs = match_sheets(&a, &a);
    for (i, p) in pairs.iter().enumerate() {
        assert_eq!(
            (p.page_a, p.page_b),
            (i as i32 + 1, i as i32 + 1),
            "near-duplicate sheet {} paired with the wrong one",
            i + 1
        );
    }
}

#[test]
fn sheets_with_nothing_in_common_are_not_forced_together() {
    let a = sigs(&["ALPHA BRAVO CHARLIE DELTA"]);
    let b = sigs(&["ZULU YANKEE XRAY WHISKEY"]);
    let pairs = match_sheets(&a, &b);
    assert_eq!(
        pairs.len(),
        2,
        "one removed and one added, not one bad pair"
    );
    assert!(pairs.iter().any(|p| p.page_a == 1 && p.page_b == 0));
    assert!(pairs.iter().any(|p| p.page_a == 0 && p.page_b == 1));
}

#[test]
fn a_sheet_with_no_text_is_unknown_not_identical() {
    // Two blank signatures must not score 1.0, or every text-less sheet pairs
    // with the first one it meets.
    let blank = Signature::from_text("   \n  ");
    assert!(blank.is_empty());
    assert_eq!(blank.similarity(&blank), 0.0);
}

#[test]
fn frame_letters_do_not_become_the_signature() {
    // Every drawing frame is edged with single characters. If those counted,
    // every sheet in the set would look like every other.
    let s = Signature::from_text("A B C D 1 2 3 4 5 NET_RESET# BUS1[0..7]");
    assert!(s.tokens.contains("NET_RESET#"));
    assert!(s.tokens.contains("BUS1[0..7]"));
    assert!(!s.tokens.contains("A"));
    assert!(!s.tokens.contains("1"));
}

#[test]
fn empty_documents_produce_no_pairs() {
    assert!(match_sheets(&[], &[]).is_empty());
    assert_eq!(match_sheets(&sigs(&["ONE TWO THREE"]), &[]).len(), 1);
}
