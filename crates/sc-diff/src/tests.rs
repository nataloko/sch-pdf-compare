// Ported from `src/CompareCore_ut.cpp` in the SumatraPDF fork, assertion for
// assertion. These are the acceptance gate for the port: the kernel's behaviour
// was settled against real schematics and must not drift because it was
// rewritten in another language.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use super::*;

/// A grey image in the canonical BGRA8 layout, from one level per pixel.
fn make_gray(w: i32, h: i32, levels: &[u8]) -> Vec<u8> {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = levels[(y * w + x) as usize];
            let i = ((y * w + x) * 4) as usize;
            data[i] = v;
            data[i + 1] = v;
            data[i + 2] = v;
            data[i + 3] = 0xff;
        }
    }
    data
}

fn gray(data: &[u8], w: i32, h: i32) -> Pixels<'_> {
    Pixels::new(data, w, h, (w * 4) as usize, PixelFormat::Bgra8).expect("well formed")
}

/// Tolerance 0 on purpose: these cases pin the exact composition, and the
/// default 1px slack would match neighbouring strokes in the tiny test images.
fn red_green() -> Options {
    Options {
        only_a: Rgb::new(0xff, 0, 0),
        only_b: Rgb::new(0, 0xff, 0),
        tolerance: 0,
        shared_ink: SHARED_INK_FULL,
    }
}

#[test]
fn ink_from_rgb_ends_and_middle() {
    assert_eq!(ink_from_rgb(0xff, 0xff, 0xff), 0);
    assert_eq!(ink_from_rgb(0, 0, 0), 255);
    let mid = ink_from_rgb(0x80, 0x80, 0x80);
    assert!(mid > 100 && mid < 160);
    // a saturated colour still reads as ink, so coloured nets are not paper
    assert!(ink_from_rgb(0xff, 0, 0) > 128);
}

#[test]
fn compose_ink_corners() {
    let o = red_green();

    // neither document has ink -> paper white
    assert_eq!(compose_ink(0, 0, 0, &o), [0xff, 0xff, 0xff]);
    // only A -> red (blue and green fully absorbed)
    let bgr = compose_ink(0, 255, 0, &o);
    assert!(bgr[2] == 0xff && bgr[1] == 0 && bgr[0] == 0);
    // only B -> green
    let bgr = compose_ink(0, 0, 255, &o);
    assert!(bgr[1] == 0xff && bgr[2] == 0 && bgr[0] == 0);
    // shared -> black, i.e. unchanged content looks like the original drawing
    assert_eq!(compose_ink(255, 0, 0, &o), [0, 0, 0]);
}

#[test]
fn compose_ink_anti_aliased() {
    let o = red_green();

    // a half-covered edge present in both stays neutral grey, not coloured
    let bgr = compose_ink(128, 0, 0, &o);
    assert!(bgr[0] == bgr[1] && bgr[1] == bgr[2]);
    assert!(bgr[0] > 0 && bgr[0] < 0xff);

    // partial ink in A only fades toward red rather than jumping to it
    let partial = compose_ink(0, 64, 0, &o);
    let full = compose_ink(0, 255, 0, &o);
    assert_eq!(partial[2], 0xff); // red channel untouched by A's own ink
    assert!(partial[1] > full[1] && partial[1] < 0xff);
    assert_eq!(partial[0], partial[1]);
}

#[test]
fn compose_ink_proportional() {
    // Only the difference in coverage is coloured, so the same stroke exported
    // at slightly different line weights tints far more weakly than a stroke
    // genuinely present in one document only.
    let o = red_green();
    let slightly_heavier = compose_ink(180, 20, 0, &o);
    let only_in_a = compose_ink(0, 200, 0, &o);

    // Tint is chroma, not darkness: shared coverage darkens every channel
    // equally, and only the leftover difference pulls them apart.
    let weak = slightly_heavier[2] as i32 - slightly_heavier[1] as i32;
    let strong = only_in_a[2] as i32 - only_in_a[1] as i32;
    assert!(weak >= 0 && strong > 0);
    assert!(weak * 10 < strong);
    // the 180 of shared coverage still renders as near-black artwork
    assert!(slightly_heavier[2] < 0x60);
}

#[test]
fn compose_ink_custom_colors() {
    // the colours are configurable, e.g. a blue/orange pair for red-green
    // colour blindness
    let o = Options {
        only_a: Rgb::new(0x00, 0x40, 0xff),
        only_b: Rgb::new(0xff, 0x80, 0x00),
        ..Options::default()
    };
    assert_eq!(compose_ink(0, 255, 0, &o), [0xff, 0x40, 0x00]);
    assert_eq!(compose_ink(0, 0, 255, &o), [0x00, 0x80, 0xff]);
    assert_eq!(compose_ink(0, 0, 0, &o), [0xff, 0xff, 0xff]);
}

#[test]
fn read_ink_row_pads_and_refuses() {
    // 3x2, white and black
    let data = make_gray(3, 2, &[0xff, 0x00, 0xff, 0x00, 0x00, 0xff]);
    let px = gray(&data, 3, 2);
    let mut ink = [9u8; 5];

    assert!(read_ink_row(Some(&px), 0, 3, &mut ink));
    assert!(ink[0] == 0 && ink[1] == 255 && ink[2] == 0);

    assert!(read_ink_row(Some(&px), 1, 3, &mut ink));
    assert!(ink[0] == 255 && ink[1] == 255 && ink[2] == 0);

    // asking past the width pads with blank paper rather than reading junk
    assert!(read_ink_row(Some(&px), 0, 5, &mut ink));
    assert!(ink[3] == 0 && ink[4] == 0);

    // a row past the bottom is blank, not a failure: tiles outrun a short render
    assert!(read_ink_row(Some(&px), 7, 3, &mut ink));
    assert!(ink[0] == 0 && ink[1] == 0 && ink[2] == 0);

    assert!(!read_ink_row(None, 0, 3, &mut ink));
}

#[test]
fn compose_tile() {
    let o = red_green();
    // same stroke in both, one only in A, one only in B, one blank column
    let da = make_gray(4, 1, &[0x00, 0x00, 0xff, 0xff]);
    let db = make_gray(4, 1, &[0x00, 0xff, 0x00, 0xff]);
    let a = gray(&da, 4, 1);
    let b = gray(&db, 4, 1);

    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(4, 1),
        0,
        ViewMode::Overlay,
        &o,
        None,
    )
    .expect("composes");
    assert!(out.width == 4 && out.height == 1);
    assert_eq!(out.bgr_at(0, 0), [0, 0, 0]); // in both
    let p = out.bgr_at(1, 0); // only A
    assert!(p[2] == 0xff && p[1] == 0 && p[0] == 0);
    let p = out.bgr_at(2, 0); // only B
    assert!(p[1] == 0xff && p[2] == 0 && p[0] == 0);
    assert_eq!(out.bgr_at(3, 0), [0xff, 0xff, 0xff]); // neither

    // a sheet with no counterpart reads as entirely added / entirely removed
    let out = compose(
        Some(&a),
        None,
        Size::new(4, 1),
        0,
        ViewMode::Overlay,
        &o,
        None,
    )
    .expect("composes");
    let p = out.bgr_at(0, 0);
    assert!(p[2] == 0xff && p[1] == 0 && p[0] == 0);
    assert_eq!(out.bgr_at(3, 0), [0xff, 0xff, 0xff]);

    // A-only and B-only reproduce their side untouched
    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(4, 1),
        0,
        ViewMode::OnlyA,
        &o,
        None,
    )
    .expect("composes");
    assert_eq!(out.bgr_at(1, 0), [0, 0, 0]);
    assert_eq!(out.bgr_at(2, 0), [0xff, 0xff, 0xff]);

    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(4, 1),
        0,
        ViewMode::OnlyB,
        &o,
        None,
    )
    .expect("composes");
    assert_eq!(out.bgr_at(1, 0), [0xff, 0xff, 0xff]);
    assert_eq!(out.bgr_at(2, 0), [0, 0, 0]);

    // a tile larger than either render is padded with paper, not left uninitialised
    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(6, 2),
        0,
        ViewMode::Overlay,
        &o,
        None,
    )
    .expect("composes");
    assert!(out.width == 6 && out.height == 2);
    assert_eq!(out.bgr_at(5, 1), [0xff, 0xff, 0xff]);

    assert!(compose(
        Some(&a),
        Some(&b),
        Size::new(0, 1),
        0,
        ViewMode::Overlay,
        &o,
        None
    )
    .is_none());
    assert!(compose(
        Some(&a),
        Some(&b),
        Size::new(4, 1),
        -1,
        ViewMode::Overlay,
        &o,
        None
    )
    .is_none());
}

#[test]
fn dilate_spreads_by_the_radius_and_no_further() {
    let src = [0u8, 0, 255, 0, 0];
    let mut dst = [0u8; 5];
    let mut scratch = [0u8; 5];

    dilate_ink(&src, &mut dst, 5, 1, 0, &mut scratch);
    assert!(dst[1] == 0 && dst[2] == 255 && dst[3] == 0);

    dilate_ink(&src, &mut dst, 5, 1, 1, &mut scratch);
    assert_eq!(dst, [0, 255, 255, 255, 0]);

    // partial coverage dilates as its own value, not as full ink
    let partial = [0u8, 100, 0];
    let mut pd = [0u8; 3];
    let mut ps = [0u8; 3];
    dilate_ink(&partial, &mut pd, 3, 1, 1, &mut ps);
    assert_eq!(pd, [100, 100, 100]);
}

#[test]
fn compose_tolerance_absorbs_a_moved_stroke() {
    // The same stroke rasterised one pixel apart by two exporters: without
    // tolerance both positions light up as changes, with 1px of slack neither
    // does. This is the difference between a usable overlay and a sea of fringe
    // when the two revisions came out of different PDF producers.
    let da = make_gray(5, 1, &[0xff, 0x00, 0xff, 0xff, 0xff]);
    let db = make_gray(5, 1, &[0xff, 0xff, 0x00, 0xff, 0xff]);
    let a = gray(&da, 5, 1);
    let b = gray(&db, 5, 1);

    fn coloured(t: &Tile) -> usize {
        (0..t.width)
            .filter(|&x| {
                let p = t.bgr_at(x, 0);
                let lo = *p.iter().min().expect("three channels") as i32;
                let hi = *p.iter().max().expect("three channels") as i32;
                hi - lo > 40
            })
            .count()
    }

    let strict = red_green();
    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(5, 1),
        0,
        ViewMode::Overlay,
        &strict,
        None,
    )
    .expect("composes");
    assert_eq!(coloured(&out), 2); // both positions flagged

    let loose = Options {
        tolerance: 1,
        ..red_green()
    };
    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(5, 1),
        0,
        ViewMode::Overlay,
        &loose,
        None,
    )
    .expect("composes");
    assert_eq!(coloured(&out), 0); // recognised as the same stroke
                                   // and it still reads as artwork rather than being erased
    let p = out.bgr_at(1, 0);
    assert!(p[0] < 0x40 && p[1] < 0x40 && p[2] < 0x40);

    // tolerance must not swallow a stroke with no counterpart anywhere near
    let dblank = make_gray(5, 1, &[0xff; 5]);
    let blank = gray(&dblank, 5, 1);
    let out = compose(
        Some(&a),
        Some(&blank),
        Size::new(5, 1),
        0,
        ViewMode::Overlay,
        &loose,
        None,
    )
    .expect("composes");
    assert_eq!(coloured(&out), 1);
}

#[test]
fn the_ceiling_reaches_past_a_zoomed_in_fringe() {
    // Why the ceiling is 8 and not the 3 it started at. Tolerance is in device
    // pixels, so a fringe that is one pixel wide at 100% is five at 500% — and
    // 500% is where a reviewer goes to read a component value. At the old
    // ceiling the overlay of a cross-producer pair broke down exactly when it
    // was being looked at closely.
    let mut levels_a = [0xffu8; 13];
    levels_a[2] = 0x00;
    let mut levels_b = [0xffu8; 13];
    levels_b[7] = 0x00;
    let da = make_gray(13, 1, &levels_a);
    let db = make_gray(13, 1, &levels_b);
    let a = gray(&da, 13, 1);
    let b = gray(&db, 13, 1);

    let coloured = |tol: i32| {
        let out = compose(
            Some(&a),
            Some(&b),
            Size::new(13, 1),
            0,
            ViewMode::Overlay,
            &Options {
                tolerance: tol,
                ..red_green()
            },
            None,
        )
        .expect("composes");
        (0..out.width)
            .filter(|&x| {
                let p = out.bgr_at(x, 0);
                let lo = *p.iter().min().expect("three channels") as i32;
                let hi = *p.iter().max().expect("three channels") as i32;
                hi - lo > 40
            })
            .count()
    };

    assert_eq!(coloured(3), 2, "five pixels apart is past the old ceiling");
    assert_eq!(coloured(5), 0, "and within the new one");
    assert_eq!(
        coloured(MAX_TOLERANCE + 3),
        0,
        "asking for more than the ceiling is clamped, not refused"
    );
    // A warning line at or above the ceiling could never be crossed, and the
    // frontend would never say anything. Checked at compile time.
    const {
        assert!(TOLERANCE_HIDES_MOVEMENT < MAX_TOLERANCE);
    }
}

#[test]
fn fading_the_shared_ink_leaves_the_changes_alone() {
    // The whole point of the control: on a dense sheet three small edits are
    // three specks of colour in a page of black line work. Fading what the two
    // revisions agree on empties the page around them without touching them —
    // at 0 the sheet is blank except for exactly what changed.
    let da = make_gray(3, 1, &[0x00, 0x00, 0xff]);
    let db = make_gray(3, 1, &[0x00, 0xff, 0xff]);
    let a = gray(&da, 3, 1);
    let b = gray(&db, 3, 1);

    let at = |pct: i32| {
        compose(
            Some(&a),
            Some(&b),
            Size::new(3, 1),
            0,
            ViewMode::Overlay,
            &Options {
                shared_ink: pct,
                ..red_green()
            },
            None,
        )
        .expect("composes")
    };

    // Column 0 is drawn in both, column 1 only in A.
    let full = at(SHARED_INK_FULL);
    assert_eq!(full.bgr_at(0, 0), [0x00, 0x00, 0x00], "shared ink is black");
    assert_eq!(full.bgr_at(1, 0), [0x00, 0x00, 0xff], "A alone is red");

    let half = at(50);
    let shared = half.bgr_at(0, 0);
    assert!(
        shared.iter().all(|&v| (0x70..=0x90).contains(&v)),
        "half way it is mid grey, and still neutral: {shared:?}"
    );
    assert_eq!(
        half.bgr_at(1, 0),
        [0x00, 0x00, 0xff],
        "the change does not fade"
    );

    let none = at(0);
    assert_eq!(
        none.bgr_at(0, 0),
        [0xff, 0xff, 0xff],
        "at zero the drawing is gone"
    );
    assert_eq!(
        none.bgr_at(1, 0),
        [0x00, 0x00, 0xff],
        "and only the change is left"
    );
    assert_eq!(
        none.bgr_at(2, 0),
        [0xff, 0xff, 0xff],
        "blank paper stays blank"
    );

    // The two spellings of the rule must not drift: the tile loop inlines what
    // `compose_ink` documents, and only one of them is easy to read.
    for pct in [0, 25, 50, 100] {
        let o = Options {
            shared_ink: pct,
            ..red_green()
        };
        assert_eq!(
            compose_ink(0xc0, 0, 0, &o),
            {
                let d = make_gray(1, 1, &[0x3f]);
                let px = gray(&d, 1, 1);
                compose(
                    Some(&px),
                    Some(&px),
                    Size::new(1, 1),
                    0,
                    ViewMode::Overlay,
                    &o,
                    None,
                )
                .expect("composes")
                .bgr_at(0, 0)
            },
            "compose_ink and the tile loop agree at {pct}%"
        );
    }
}

#[test]
fn compose_masks_never_colour_and_never_hide() {
    // Inside an excluded region the artwork still shows but never picks up a
    // colour, and the region is edged so it cannot be mistaken for a part of the
    // sheet that simply did not change.
    let da = make_gray(8, 1, &[0xff, 0x00, 0xff, 0xff, 0xff, 0x00, 0xff, 0xff]);
    let db = make_gray(8, 1, &[0xff; 8]);
    let a = gray(&da, 8, 1);
    let b = gray(&db, 8, 1);
    let o = red_green();

    // without a mask both strokes are unmatched and get coloured
    let plain = compose(
        Some(&a),
        Some(&b),
        Size::new(8, 1),
        0,
        ViewMode::Overlay,
        &o,
        None,
    )
    .expect("composes");
    let p = plain.bgr_at(1, 0);
    assert!(p[2] as i32 > p[1] as i32 + 40); // red
    let p = plain.bgr_at(5, 0);
    assert!(p[2] as i32 > p[1] as i32 + 40);

    // exclude the right half; its stroke must stop being coloured
    let rects = [Rect::new(4, 0, 4, 1)];
    let masks = TileMasks {
        rects: &rects,
        device_origin: Point::default(),
    };
    let out = compose(
        Some(&a),
        Some(&b),
        Size::new(8, 1),
        0,
        ViewMode::Overlay,
        &o,
        Some(&masks),
    )
    .expect("composes");
    let p = out.bgr_at(1, 0);
    assert!(p[2] as i32 > p[1] as i32 + 40); // outside the mask: still reported
    let p = out.bgr_at(5, 0);
    assert!(p[0] == p[1] && p[1] == p[2]); // inside: neutral, never coloured
    assert!(p[0] < 0xff); // the artwork is still visible
    assert!(p[0] > 0x40); // but washed out
}

/// Draws a filled block of ink into a `w * h` plane.
fn plane_block(p: &mut [u8], w: usize, x0: usize, y0: usize, bw: usize, bh: usize) {
    for y in y0..y0 + bh {
        for x in x0..x0 + bw {
            p[y * w + x] = 255;
        }
    }
}

#[test]
fn find_changes_clusters_and_ignores_specks() {
    const W: usize = 128;
    const H: usize = 128;
    let mut a = vec![0u8; W * H];
    let mut b = vec![0u8; W * H];
    let o = Options {
        tolerance: 0,
        ..Options::default()
    };

    // identical sheets have nothing to report
    plane_block(&mut a, W, 10, 10, 20, 20);
    plane_block(&mut b, W, 10, 10, 20, 20);
    assert_eq!(find_changes(&a, &b, W, H, &o).len(), 0);

    // one block only in A and one only in B, far apart -> two destinations
    plane_block(&mut a, W, 60, 60, 12, 12);
    plane_block(&mut b, W, 100, 20, 12, 12);
    let changes = find_changes(&a, &b, W, H, &o);
    assert_eq!(changes.len(), 2);
    for c in &changes {
        assert!(c.pixels >= CHANGE_MIN_PIXELS);
        // the reported box has to actually contain the block it stands for
        let covers_a = c.box_.contains(Point::new(66, 66));
        let covers_b = c.box_.contains(Point::new(106, 26));
        assert!(covers_a || covers_b);
    }

    // a speck below the floor is noise, not a destination worth navigating to
    a.fill(0);
    b.fill(0);
    a[50 * W + 50] = 255;
    assert_eq!(find_changes(&a, &b, W, H, &o).len(), 0);

    // tolerance applies here too: a stroke that merely moved is not a change
    a.fill(0);
    b.fill(0);
    plane_block(&mut a, W, 40, 40, 10, 10);
    plane_block(&mut b, W, 41, 40, 10, 10);
    assert!(!find_changes(&a, &b, W, H, &o).is_empty()); // strict: edges disagree
    let loose = Options {
        tolerance: 1,
        ..Options::default()
    };
    assert_eq!(find_changes(&a, &b, W, H, &loose).len(), 0);

    // one edit split across neighbouring cells comes back as a single region
    a.fill(0);
    b.fill(0);
    plane_block(&mut a, W, 30, 30, 40, 3);
    let changes = find_changes(&a, &b, W, H, &o);
    assert_eq!(changes.len(), 1);
    assert!(changes[0].box_.dx >= 40);

    // a missing side is nothing to report, not a panic
    assert_eq!(find_changes(&[], &b, W, H, &o).len(), 0);
}

#[test]
fn pairing_equal_length() {
    let p = Pairing::build(3, 3, 0);
    assert!(p.page_count == 3 && p.first_a_page == 1);
    for i in 1..=3 {
        assert!(p.at(i).page_a == i && p.at(i).page_b == i);
    }
    // out of range is "no sheet on either side", not a crash or a wrapped index
    assert_eq!(p.at(0), Pair::default());
    assert_eq!(p.at(4), Pair::default());
}

#[test]
fn pairing_different_length() {
    // the second revision added two sheets at the end
    let p = Pairing::build(3, 5, 0);
    assert!(p.page_count == 5 && p.first_a_page == 1);
    assert!(p.at(3).page_a == 3 && p.at(3).page_b == 3);
    // pages 4 and 5 exist only in the second document -> entirely added
    assert!(p.at(4).page_a == 0 && p.at(4).page_b == 4);
    assert!(p.at(5).page_a == 0 && p.at(5).page_b == 5);

    // and the mirror case: sheets removed
    let p = Pairing::build(5, 3, 0);
    assert_eq!(p.page_count, 5);
    assert!(p.at(4).page_a == 4 && p.at(4).page_b == 0);
}

#[test]
fn pairing_positive_delta() {
    // a sheet was inserted at the front of the second document, so its page n+1
    // is the old page n; nudging the pairing by +1 lines them back up
    let p = Pairing::build(3, 4, 1);
    assert_eq!(p.first_a_page, 0); // virtual page 1 is B's orphaned first sheet
    assert_eq!(p.page_count, 4);
    assert!(p.at(1).page_a == 0 && p.at(1).page_b == 1);
    assert!(p.at(2).page_a == 1 && p.at(2).page_b == 2);
    assert!(p.at(3).page_a == 2 && p.at(3).page_b == 3);
    assert!(p.at(4).page_a == 3 && p.at(4).page_b == 4);
}

#[test]
fn pairing_negative_delta() {
    // the mirror: a sheet was dropped from the front of the second document
    let p = Pairing::build(4, 3, -1);
    assert_eq!(p.first_a_page, 1);
    assert_eq!(p.page_count, 4);
    assert!(p.at(1).page_a == 1 && p.at(1).page_b == 0);
    assert!(p.at(2).page_a == 2 && p.at(2).page_b == 1);
    assert!(p.at(4).page_a == 4 && p.at(4).page_b == 3);
}

#[test]
fn pairing_covers_every_sheet() {
    // whatever the offset, no sheet of either document may be left unreachable —
    // that would silently hide a whole page from the comparison
    for n_a in [1, 2, 5, 9] {
        for n_b in [1, 2, 5, 9] {
            for delta in [-4, -2, -1, 0, 1, 2, 7] {
                let p = Pairing::build(n_a, n_b, delta);
                let mut seen_a = 0;
                let mut seen_b = 0;
                for i in 1..=p.page_count {
                    let pair = p.at(i);
                    if pair.page_a != 0 {
                        seen_a += 1;
                    }
                    if pair.page_b != 0 {
                        seen_b += 1;
                    }
                }
                assert_eq!(seen_a, n_a, "nA={n_a} nB={n_b} delta={delta}");
                assert_eq!(seen_b, n_b, "nA={n_a} nB={n_b} delta={delta}");
            }
        }
    }
}

#[test]
fn pairing_empty() {
    let p = Pairing::build(0, 0, 0);
    assert_eq!(p.page_count, 0);
    assert_eq!(p.at(1), Pair::default());
}

#[test]
fn the_render_margin_is_what_stops_a_tile_edge_inventing_a_change() {
    // Gotcha, paid for once already: each side must be rendered with `tolerance`
    // extra pixels of context, or the dilation at a tile's border has no
    // neighbours to look at and every seam grows a line of changes that are not
    // on the drawing.
    //
    // The same stroke, one pixel apart in the two documents, sitting right at a
    // tile boundary. With context it is recognised as one stroke; without, the
    // tile can only see one half of the evidence.
    let one = Options {
        tolerance: 1,
        ..red_green()
    };

    // Five pixels of content: A's stroke at x=3, B's at x=4.
    let mut la = [0xffu8; 5];
    let mut lb = [0xffu8; 5];
    la[3] = 0x00;
    lb[4] = 0x00;
    let da = make_gray(5, 1, &la);
    let db = make_gray(5, 1, &lb);
    let a = gray(&da, 5, 1);
    let b = gray(&db, 5, 1);

    fn is_coloured(p: [u8; 3]) -> bool {
        let lo = *p.iter().min().expect("three channels") as i32;
        let hi = *p.iter().max().expect("three channels") as i32;
        hi - lo > 40
    }

    // A tile covering x=1..4, rendered with one pixel of context either side, so
    // the compositor is handed all five pixels and told the margin is 1.
    let with_context = compose(
        Some(&a),
        Some(&b),
        Size::new(3, 1),
        1,
        ViewMode::Overlay,
        &one,
        None,
    )
    .expect("composes");
    assert!(
        !is_coloured(with_context.bgr_at(2, 0)),
        "with a pixel of context the two strokes are recognised as one"
    );

    // The same tile rendered without context: only x=1..4 exists, so B's stroke
    // at x=4 is not in the evidence and A's at x=3 looks unmatched.
    let da_cut = make_gray(3, 1, &la[1..4]);
    let db_cut = make_gray(3, 1, &lb[1..4]);
    let a_cut = gray(&da_cut, 3, 1);
    let b_cut = gray(&db_cut, 3, 1);
    let no_context = compose(
        Some(&a_cut),
        Some(&b_cut),
        Size::new(3, 1),
        0,
        ViewMode::Overlay,
        &one,
        None,
    )
    .expect("composes");
    assert!(
        is_coloured(no_context.bgr_at(2, 0)),
        "without context the tile edge invents a change — this is the bug the margin prevents"
    );
}

fn word(text: &str, x: f32, y: f32) -> Word {
    Word::new(text, RectF::new(x, y, 20.0, 8.0))
}

#[test]
fn text_that_did_not_move_is_not_reported() {
    let a = vec![word("NET_RESET#", 100.0, 200.0), word("10k", 300.0, 400.0)];
    // The same sheet through a different PDF producer: everything shifted by
    // less than a point, which is the largest displacement measured on the real
    // sets. None of it is a change.
    let b = vec![word("NET_RESET#", 100.7, 200.4), word("10k", 300.9, 400.2)];
    assert!(diff_words(&a, &b).is_empty());
}

#[test]
fn a_value_that_changed_is_reported_with_both_readings() {
    let a = vec![word("R47", 100.0, 200.0), word("10k", 100.0, 210.0)];
    let b = vec![word("R47", 100.0, 200.0), word("12k", 100.0, 210.0)];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, TextChangeKind::Changed);
    assert_eq!(changes[0].before, "10k");
    assert_eq!(changes[0].after, "12k");
    // Reported where it is, so the reader can be taken there.
    assert_eq!(changes[0].rect.y, 210.0);
}

#[test]
fn added_and_removed_text_is_told_apart() {
    let a = vec![word("OLD_NET", 100.0, 200.0)];
    let b = vec![word("NEW_NET", 500.0, 300.0)];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 2, "far apart, so not one change but two");
    let removed = changes
        .iter()
        .find(|c| c.kind == TextChangeKind::Removed)
        .expect("removed");
    let added = changes
        .iter()
        .find(|c| c.kind == TextChangeKind::Added)
        .expect("added");
    assert_eq!(removed.before, "OLD_NET");
    assert_eq!(removed.after, "");
    assert_eq!(added.after, "NEW_NET");
    assert_eq!(added.before, "");
}

#[test]
fn a_repeated_label_pairs_with_its_own_copy() {
    // A bus draws the same label down a column. Matching identical text before
    // matching by position is what stops the first occurrence pairing with the
    // wrong copy and inventing two changes.
    let a = vec![
        word("SPI_CLK", 100.0, 100.0),
        word("SPI_CLK", 100.0, 120.0),
        word("SPI_CLK", 100.0, 140.0),
    ];
    let mut b = a.clone();
    b[1] = word("SPI_CS#", 100.0, 120.0);
    let changes = diff_words(&a, &b);
    assert_eq!(
        changes.len(),
        1,
        "only the middle one changed, got {changes:?}"
    );
    assert_eq!(changes[0].before, "SPI_CLK");
    assert_eq!(changes[0].after, "SPI_CS#");
    assert_eq!(changes[0].rect.y, 120.0);
}

#[test]
fn changes_come_back_in_reading_order() {
    let a = vec![word("A_ONE", 500.0, 400.0), word("B_TWO", 100.0, 100.0)];
    let b = vec![word("A_MOD", 500.0, 400.0), word("B_MOD", 100.0, 100.0)];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 2);
    assert!(
        changes[0].rect.y < changes[1].rect.y,
        "top of the sheet first"
    );
}

#[test]
fn an_empty_side_is_all_addition_or_all_removal() {
    let a = vec![word("ONLY_HERE", 10.0, 10.0)];
    let changes = diff_words(&a, &[]);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, TextChangeKind::Removed);

    let changes = diff_words(&[], &a);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, TextChangeKind::Added);

    assert!(diff_words(&[], &[]).is_empty());
}

#[test]
fn text_stamped_twice_in_one_place_is_one_thing() {
    // Sheet 2 of the sample set draws its whole content three times over. Without
    // collapsing that, every change on it is reported three times and a reader
    // stepping through the list visits each one three times.
    let a = vec![
        word("B03", 100.0, 200.0),
        word("B03", 100.0, 200.0),
        word("B03", 100.3, 200.2),
    ];
    let b = vec![
        word("D02", 100.0, 200.0),
        word("D02", 100.0, 200.0),
        word("D02", 100.3, 200.2),
    ];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 1, "one change, not three: {changes:?}");
    assert_eq!(
        (changes[0].before.as_str(), changes[0].after.as_str()),
        ("B03", "D02")
    );
}

#[test]
fn a_label_repeated_down_a_bus_is_not_collapsed() {
    // The other side of the same coin: these are genuinely separate labels a few
    // points apart, and collapsing them would hide a change to one of them.
    let a = vec![
        word("D0", 100.0, 100.0),
        word("D0", 100.0, 110.0),
        word("D0", 100.0, 120.0),
    ];
    let mut b = a.clone();
    b[2] = word("D7", 100.0, 120.0);
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].rect.y, 120.0,
        "and it is the third one that changed"
    );
}

#[test]
fn text_that_only_moved_is_told_apart_from_a_change() {
    // A sheet that was re-laid-out moves dozens of identical labels. Reported as
    // a removal plus an addition that is two rows each, and the handful of real
    // changes drown in them.
    let a = vec![word("+3V3", 100.0, 100.0), word("10k", 200.0, 200.0)];
    let b = vec![word("+3V3", 400.0, 300.0), word("12k", 200.0, 200.0)];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 2);
    let moved = changes
        .iter()
        .find(|c| c.kind == TextChangeKind::Moved)
        .expect("a move");
    assert_eq!(moved.before, "+3V3");
    assert_eq!(moved.after, "+3V3");
    assert_eq!(moved.rect.x, 100.0, "reported where it was");
    let changed = changes
        .iter()
        .find(|c| c.kind == TextChangeKind::Changed)
        .expect("a change");
    assert_eq!(
        (changed.before.as_str(), changed.after.as_str()),
        ("10k", "12k")
    );
}

#[test]
fn a_move_is_only_claimed_when_the_text_really_is_the_same() {
    // Otherwise every removal would find some unrelated addition to pair with.
    let a = vec![word("OLD_NET", 100.0, 100.0)];
    let b = vec![word("NEW_NET", 400.0, 300.0)];
    let changes = diff_words(&a, &b);
    assert!(changes.iter().all(|c| c.kind != TextChangeKind::Moved));
}

#[test]
fn a_move_never_outranks_a_match_in_place() {
    // Two copies of the same label, one of which stayed put: the one that stayed
    // must match where it is, leaving the other as the move.
    let a = vec![word("GND", 100.0, 100.0), word("GND", 200.0, 100.0)];
    let b = vec![word("GND", 100.0, 100.0), word("GND", 600.0, 500.0)];
    let changes = diff_words(&a, &b);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, TextChangeKind::Moved);
    assert_eq!(
        changes[0].rect.x, 200.0,
        "the one that stayed is not the one that moved"
    );
}
