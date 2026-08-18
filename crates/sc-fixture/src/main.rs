//! Writes the pair of documents the window test compares.
//!
//! A separate binary because the test is C++ and the fixtures are Rust. CMake
//! runs this once into the build tree, so the window's own behaviour — the
//! sidebar filling, the sweep finishing, the text panel, printing, settings —
//! is exercised on a clone with no customer drawings in it.
//!
//! `gen-fixtures <directory>` writes `a.pdf` and `b.pdf`.

use sc_fixture::{write, Sheet};
use std::path::Path;

/// Three sheets with a shared frame and a title block carrying the revision.
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
                .text(700.0, 35.0, &format!("SHEET {} REV {rev}", i + 1))
                .text(200.0, 400.0, &format!("NET_SHEET_{}", i + 1))
                .text(300.0, 300.0, v)
        })
        .collect()
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    // Six sheets, not three: the repeat detector will not offer anything below
    // four, on the grounds that "recurring" means nothing across two sheets, and
    // a fixture that cannot reach the threshold cannot test the feature.
    //
    // The later revision changes one label on sheet 1 and the revision letter in
    // every title block — so the test has both a single real change to find and
    // the set-wide repeat that excluded regions exist for.
    let a = ["NET_ALPHA", "4k7", "100nF", "10k", "33R", "0R"];
    let b = ["NET_BRAVO", "4k7", "100nF", "10k", "33R", "0R"];
    write(&dir.join("a.pdf"), &set("A", &a))?;
    write(&dir.join("b.pdf"), &set("B", &b))?;
    Ok(())
}
