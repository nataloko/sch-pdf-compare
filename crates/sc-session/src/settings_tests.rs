// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::settings::Settings;
use sc_diff::{Options, RectF, Rgb};
use std::path::PathBuf;

/// A file of our own, named after the test, so nothing races.
fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sch-pdf-compare-test-{name}.json"));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn excluded_regions_survive_a_restart() {
    // The whole reason this exists: working out where a set's title block is
    // costs a reviewer a minute, and losing it every session makes the feature
    // not worth using.
    let path = scratch("regions");
    let mut s = Settings::at(path.clone());
    let rects = [
        RectF::new(600.0, 570.0, 200.0, 25.0),
        RectF::new(10.0, 10.0, 30.0, 30.0),
    ];
    s.set_ignore_rects("/docs/A.pdf", "/docs/B.pdf", &rects);
    s.save().expect("saves");

    let again = Settings::at(path.clone());
    let back = again.ignore_rects("/docs/A.pdf", "/docs/B.pdf");
    assert_eq!(back.len(), 2);
    assert_eq!(back[0], rects[0]);
    assert_eq!(back[1], rects[1]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn regions_belong_to_the_pair_that_defined_them() {
    // A title block excluded for one drawing set must not silently apply to a
    // different one, whose frame is somewhere else entirely.
    let path = scratch("per-pair");
    let mut s = Settings::at(path.clone());
    s.set_ignore_rects("/A.pdf", "/B.pdf", &[RectF::new(1.0, 2.0, 3.0, 4.0)]);
    s.save().expect("saves");

    let again = Settings::at(path.clone());
    assert_eq!(again.ignore_rects("/A.pdf", "/B.pdf").len(), 1);
    assert!(again.ignore_rects("/A.pdf", "/OTHER.pdf").is_empty());
    assert!(again.ignore_rects("/OTHER.pdf", "/B.pdf").is_empty());
    // and the order matters: comparing B against A is a different comparison
    assert!(again.ignore_rects("/B.pdf", "/A.pdf").is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn options_round_trip_including_the_colours() {
    let path = scratch("options");
    let mut s = Settings::at(path.clone());
    let o = Options {
        only_a: Rgb::new(0x00, 0x40, 0xff),
        only_b: Rgb::new(0xff, 0x80, 0x00),
        tolerance: 2,
        shared_ink: 30,
    };
    s.set_options(o);
    s.save().expect("saves");
    assert_eq!(Settings::at(path.clone()).options(), o);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_tolerance_from_a_hand_edited_file_is_clamped() {
    // The file is meant to be hand-editable, so it will be hand-edited badly.
    let path = scratch("clamp");
    std::fs::write(&path, r#"{"version":1,"tolerance":99,"shared_ink":-5}"#).expect("writes");
    let o = Settings::at(path.clone()).options();
    assert_eq!(o.tolerance, sc_diff::MAX_TOLERANCE);
    assert_eq!(o.shared_ink, 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_broken_file_falls_back_rather_than_refusing_to_start() {
    let path = scratch("broken");
    std::fs::write(&path, "this is not json at all {{{").expect("writes");
    let s = Settings::at(path.clone());
    assert_eq!(s.options(), Options::default());
    assert!(s.ignore_rects("/A.pdf", "/B.pdf").is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_file_from_a_newer_version_is_not_read_or_clobbered() {
    // Someone running two builds should not have the newer one's settings
    // quietly replaced by the older one's idea of them.
    let path = scratch("future");
    // `r##` because the content itself contains `"#`, which would close `r#`.
    let future = r##"{"version":99,"tolerance":3,"only_a":"#000000","only_b":"#ffffff"}"##;
    std::fs::write(&path, future).expect("writes");
    let s = Settings::at(path.clone());
    assert_eq!(s.options(), Options::default(), "not read");
    assert_eq!(
        std::fs::read_to_string(&path).expect("still there"),
        future,
        "not clobbered"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_last_pair_is_remembered() {
    let path = scratch("last");
    let mut s = Settings::at(path.clone());
    assert!(s.last_pair().is_none());
    s.set_ignore_rects("/one/A.pdf", "/one/B.pdf", &[]);
    s.set_ignore_rects("/two/A.pdf", "/two/B.pdf", &[]);
    s.save().expect("saves");
    let again = Settings::at(path.clone());
    assert_eq!(again.last_pair(), Some(("/two/A.pdf", "/two/B.pdf")));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn only_so_many_pairs_are_kept() {
    let path = scratch("cap");
    let mut s = Settings::at(path.clone());
    for i in 0..60 {
        s.set_ignore_rects(
            &format!("/a{i}.pdf"),
            "/b.pdf",
            &[RectF::new(1.0, 1.0, 1.0, 1.0)],
        );
    }
    s.save().expect("saves");
    let again = Settings::at(path.clone());
    // The oldest went, the newest stayed.
    assert!(again.ignore_rects("/a0.pdf", "/b.pdf").is_empty());
    assert_eq!(again.ignore_rects("/a59.pdf", "/b.pdf").len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn saving_twice_leaves_one_entry_not_two() {
    let path = scratch("dedupe");
    let mut s = Settings::at(path.clone());
    s.set_ignore_rects("/A.pdf", "/B.pdf", &[RectF::new(1.0, 1.0, 1.0, 1.0)]);
    s.set_ignore_rects("/A.pdf", "/B.pdf", &[RectF::new(2.0, 2.0, 2.0, 2.0)]);
    s.save().expect("saves");
    let again = Settings::at(path.clone());
    let back = again.ignore_rects("/A.pdf", "/B.pdf");
    assert_eq!(back.len(), 1, "the later save replaces the earlier one");
    assert_eq!(back[0].x, 2.0);
    let _ = std::fs::remove_file(&path);
}
