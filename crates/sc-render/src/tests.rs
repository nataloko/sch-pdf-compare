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

// --- awkward files -------------------------------------------------------
//
// These need no `samples/`: the fixtures are written by the test. A drawing
// arriving from outside is quite often protected, truncated by a half-finished
// download, or simply the wrong file, and each of those has to say which.

fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sch-render-test-{name}.pdf"));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn a_generated_fixture_opens_and_reads_back() {
    let p = scratch("basic");
    sc_fixture::write(
        &p,
        &[
            sc_fixture::Sheet::a4_landscape()
                .text(100.0, 500.0, "NET_RESET#")
                .line(0.0, 0.0, 100.0, 100.0),
            sc_fixture::Sheet::a4_landscape().text(120.0, 480.0, "BUS_MOSI"),
        ],
    )
    .expect("writes");

    let doc = Document::open(&p.to_string_lossy()).expect("opens");
    assert_eq!(doc.page_count(), 2);
    let (w, h) = doc.page_size(1).expect("has a sheet 1");
    assert!(w > h, "landscape, like a schematic: {w}x{h}");
    assert!(doc.page_text(1).expect("has text").contains("NET_RESET#"));
    assert!(doc.page_text(2).expect("has text").contains("BUS_MOSI"));

    // The words come back where they were put. PDF user space has its origin at
    // the bottom left and everything else here counts from the top, so this is
    // also the check that the flip is right.
    let words = doc.page_words(1).expect("has words");
    let w0 = words
        .iter()
        .find(|w| w.text == "NET_RESET#")
        .expect("the word is there");
    assert!((w0.rect.x - 100.0).abs() < 2.0, "x is {}", w0.rect.x);
    assert!(
        (w0.rect.y - (595.0 - 500.0 - 10.0)).abs() < 4.0,
        "y is {}",
        w0.rect.y
    );
    let _ = std::fs::remove_file(&p);
}

#[test]
fn a_file_that_is_not_a_pdf_says_so_and_names_itself() {
    let p = scratch("garbage");
    std::fs::write(&p, b"this is plainly not a PDF at all").expect("writes");
    let e = Document::open(&p.to_string_lossy()).expect_err("refused");
    assert!(matches!(e, Error::Format(_)));
    let said = e.to_string();
    assert!(said.contains("garbage"), "names the file: {said}");
    assert!(
        !said.contains("code:"),
        "no MuPDF error number in front of a person: {said}"
    );
    let _ = std::fs::remove_file(&p);
}

#[test]
fn a_document_with_no_pages_is_refused_rather_than_compared() {
    // MuPDF opens a badly damaged file, rebuilds what it can and hands back
    // nothing. Without this check that reads as a comparison of two empty
    // documents — a blank window and no explanation.
    let p = scratch("truncated");
    let full = scratch("truncated-source");
    sc_fixture::write(
        &full,
        &[sc_fixture::Sheet::a4_landscape().text(10.0, 10.0, "SOMETHING")],
    )
    .expect("writes");
    let data = std::fs::read(&full).expect("reads");
    std::fs::write(&p, &data[..data.len() / 3]).expect("writes");

    match Document::open(&p.to_string_lossy()) {
        Err(Error::Empty(_)) | Err(Error::Format(_)) => {}
        Err(other) => panic!("wrong complaint: {other}"),
        Ok(d) => panic!(
            "opened a third of a file and found {} pages",
            d.page_count()
        ),
    }
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(&full);
}

#[test]
fn a_missing_file_and_a_folder_are_told_apart() {
    let e = Document::open("/definitely/not/here/at/all.pdf").expect_err("refused");
    assert!(matches!(e, Error::Io(_)));
    assert!(e.to_string().contains("no such file"), "{e}");
    assert!(
        !e.to_string().contains("os error"),
        "the errno is for a log: {e}"
    );

    let e = Document::open(&std::env::temp_dir().to_string_lossy()).expect_err("refused");
    assert!(e.to_string().contains("folder"), "{e}");
}

#[test]
fn a_password_protected_drawing_says_so() {
    // Needs qpdf to make one; skipped where it is not installed rather than
    // carrying an encrypted binary in the repository.
    let src = scratch("locked-source");
    let out = scratch("locked");
    sc_fixture::write(
        &src,
        &[sc_fixture::Sheet::a4_landscape().text(50.0, 50.0, "SECRET_NET")],
    )
    .expect("writes");
    let made = std::process::Command::new("qpdf")
        .args([
            "--encrypt",
            "--user-password=secret",
            "--owner-password=owner",
            "--bits=256",
            "--",
        ])
        .arg(&src)
        .arg(&out)
        .status();
    let Ok(status) = made else { return };
    if !status.success() {
        return;
    }

    let e = Document::open(&out.to_string_lossy()).expect_err("refused");
    assert!(matches!(e, Error::Locked(_)), "got {e}");
    assert!(e.to_string().contains("password"), "{e}");
    // And it says what to do about it, since we cannot ask for the password.
    assert!(e.to_string().contains("without the password"), "{e}");
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&out);
}
