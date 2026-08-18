// Run against the real revision sets when they are present.
//
// `samples/` holds customer documents and is never committed, so every test
// here returns early when it is absent rather than failing — a clone without
// them still has a green suite, and the person who has them gets the coverage.
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

const DIGITAL_A10: &str = "SET-ONE - EXAMPLE DIGITAL REV-P1.pdf";
const DIGITAL_D02: &str = "SET-ONE - EXAMPLE DIGITAL REV-P3.pdf";
const SECOND_A07: &str = "SET-TWO - EXAMPLE SECOND REV-Q1.pdf";

#[test]
fn opens_a_real_set_and_counts_its_sheets() {
    let Some(p) = sample(DIGITAL_A10) else { return };
    let doc = Document::open(&p).expect("opens");
    assert_eq!(doc.page_count(), 21);

    let Some(p) = sample(SECOND_A07) else {
        return;
    };
    let doc = Document::open(&p).expect("opens");
    assert_eq!(doc.page_count(), 85);
}

#[test]
fn rotation_is_already_applied_to_the_page_size() {
    // REV-P3 was exported landscape where REV-P1 was portrait, and both draw
    // the same picture. If `/Rotate` were not applied here the two would never
    // line up and every sheet would read as entirely changed.
    let (Some(a), Some(d)) = (sample(DIGITAL_A10), sample(DIGITAL_D02)) else {
        return;
    };
    let a = Document::open(&a).expect("opens");
    let d = Document::open(&d).expect("opens");

    let (aw, ah) = a.page_size(2).expect("has a sheet 2");
    let (dw, dh) = d.page_size(2).expect("has a sheet 2");
    assert!(aw > ah, "A4 landscape once rotation is applied: {aw}x{ah}");
    assert!(dw > dh, "and the same for the landscape export: {dw}x{dh}");
    assert!(
        (aw - dw).abs() < 2.0 && (ah - dh).abs() < 2.0,
        "same sheet, same size"
    );
}

#[test]
fn renders_a_tile_that_matches_the_same_region_of_the_whole_sheet() {
    let Some(p) = sample(DIGITAL_A10) else { return };
    let doc = Document::open(&p).expect("opens");
    let zoom = 150.0 / 72.0;
    let (w, h) = doc.page_device_size(2, zoom).expect("has a sheet 2");
    assert!(
        w > 1000 && h > 700,
        "A4 at 150 dpi is about 1755x1240, got {w}x{h}"
    );

    let whole = doc.render(2, zoom, Tile::whole(w, h)).expect("renders");
    assert_eq!((whole.width(), whole.height()), (w, h));

    // A tile has to be byte-identical to the same window of the full render, or
    // every seam in the viewer is a lie.
    let t = Tile::new(300, 200, 128, 64);
    let tile = doc.render(2, zoom, t).expect("renders");
    for y in 0..t.height {
        let a = &tile.samples()[y as usize * tile.stride()..][..(t.width * 3) as usize];
        let b = &whole.samples()[(y + t.y) as usize * whole.stride() + (t.x * 3) as usize..]
            [..(t.width * 3) as usize];
        assert_eq!(a, b, "tile row {y} differs from the full render");
    }
}

#[test]
fn a_blank_margin_renders_as_paper_not_as_ink() {
    // Tiles routinely run past the end of a sheet. Those pixels must be white:
    // the comparison reads "not paper" as coverage, so anything else there is a
    // change that is not on the drawing.
    let Some(p) = sample(DIGITAL_A10) else { return };
    let doc = Document::open(&p).expect("opens");
    let zoom = 1.0;
    let (w, h) = doc.page_device_size(2, zoom).expect("has a sheet 2");
    let past = doc
        .render(2, zoom, Tile::new(w, h, 16, 16))
        .expect("renders");
    assert!(
        past.samples().iter().all(|&b| b == 0xff),
        "past the sheet is paper"
    );
}

#[test]
fn extracts_text_across_producers() {
    // REV-P1 is subset Type1C/WinAnsi, REV-P3 is CID TrueType/Identity-H. Sheet
    // matching leans on this, so it has to work for both.
    for name in [DIGITAL_A10, DIGITAL_D02] {
        let Some(p) = sample(name) else { return };
        let doc = Document::open(&p).expect("opens");
        let text = doc.page_text(2).expect("has a sheet 2");
        assert!(
            text.contains("NET_RESET#"),
            "{name}: expected a net label in the text"
        );
    }
}

#[test]
fn refuses_a_sheet_that_is_not_there() {
    let Some(p) = sample(DIGITAL_A10) else { return };
    let doc = Document::open(&p).expect("opens");
    assert!(matches!(doc.page_size(0), Err(Error::NoSuchPage(0))));
    assert!(matches!(doc.page_size(22), Err(Error::NoSuchPage(22))));
    assert!(matches!(
        doc.render(1, 1.0, Tile::new(0, 0, 0, 10)),
        Err(Error::BadGeometry)
    ));
}

#[test]
fn refuses_a_file_that_is_not_a_document() {
    assert!(Document::open("/definitely/not/here.pdf").is_err());
}
