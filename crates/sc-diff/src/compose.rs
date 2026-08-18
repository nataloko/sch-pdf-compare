// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::ink::{dilate_ink, read_ink_plane, read_ink_row, shared_coverage, InkTable};
use crate::{
    Options, PixelFormat, Pixels, Point, Rect, Size, Tile, ViewMode, MASK_EDGE_COLOR,
    MASK_INK_PERCENT, MAX_TOLERANCE,
};

/// Regions the reader has excluded from the comparison, as device pixels within
/// the tile being composed.
///
/// `device_origin` is the tile's position on the page, so the dashes that mark
/// an excluded region line up from one tile to the next instead of restarting at
/// every seam.
#[derive(Clone, Copy, Debug)]
pub struct TileMasks<'a> {
    pub rects: &'a [Rect],
    pub device_origin: Point,
}

/// Marks the tile's pixels: 0 compared normally, 1 inside an excluded region,
/// 2 on that region's dashed edge.
fn build_mask_plane(target: Size, masks: Option<&TileMasks>) -> Option<Vec<u8>> {
    let masks = masks?;
    if masks.rects.is_empty() {
        return None;
    }
    let w = target.dx as usize;
    let mut plane = vec![0u8; w * target.dy as usize];
    const DASH: i32 = 5; // on for 5 device px, off for 5
    for r in masks.rects {
        let x0 = r.x.max(0);
        let y0 = r.y.max(0);
        let x1 = (r.x + r.dx).min(target.dx);
        let y1 = (r.y + r.dy).min(target.dy);
        for y in y0..y1 {
            let row = &mut plane[y as usize * w..][..w];
            let edge_row = y == r.y || y == r.y + r.dy - 1;
            for x in x0..x1 {
                let edge_col = x == r.x || x == r.x + r.dx - 1;
                if edge_row || edge_col {
                    let along = if edge_row {
                        masks.device_origin.x + x
                    } else {
                        masks.device_origin.y + y
                    };
                    row[x as usize] = if (along / DASH) & 1 != 0 { 1 } else { 2 };
                } else if row[x as usize] == 0 {
                    row[x as usize] = 1;
                }
            }
        }
    }
    Some(plane)
}

/// Writes one side into the tile unchanged, padding with white where the source
/// is smaller.
///
/// Used by the A-only and B-only view modes, which are meant to look exactly
/// like opening that file on its own — a reader flipping between them with `Tab`
/// is using them as a blink comparator, and any processing here would show up as
/// a flicker that means nothing.
fn copy_side_into(dst: &mut Tile, src: Option<&Pixels>, margin: i32) {
    let rgba = src
        .map(|s| s.format() == PixelFormat::Rgba8)
        .unwrap_or(false);
    let step = src.map(|s| s.format().bytes_per_pixel()).unwrap_or(4);
    for y in 0..dst.height {
        let width = dst.width;
        let sy = y + margin;
        let mut n = 0usize;
        let mut row: &[u8] = &[];
        if let Some(s) = src {
            if sy >= 0 && sy < s.height() {
                n = (width.min(s.width() - margin)).max(0) as usize;
                row = &s.row(sy)[margin as usize * step..];
            }
        }
        let out = dst.row_mut(y);
        for x in 0..n {
            let p = &row[x * step..];
            let o = &mut out[x * 4..][..4];
            o[0] = if rgba { p[2] } else { p[0] };
            o[1] = p[1];
            o[2] = if rgba { p[0] } else { p[2] };
            o[3] = 0xff;
        }
        out[n * 4..].fill(0xff);
    }
}

/// Composes one tile of the comparison.
///
/// `a` and `b` are the same sheet rendered from each document at the same scale,
/// each covering the target tile **plus `margin` device pixels of surrounding
/// context** so the tolerance dilation has neighbours to look at even at a tile
/// edge. Leave the margin out and every tile boundary grows a seam of changes
/// that are not there. Either side may be `None` when that document has no
/// matching page.
pub fn compose(
    a: Option<&Pixels>,
    b: Option<&Pixels>,
    target: Size,
    margin: i32,
    mode: ViewMode,
    opts: &Options,
    masks: Option<&TileMasks>,
) -> Option<Tile> {
    if target.dx <= 0 || target.dy <= 0 || margin < 0 {
        return None;
    }
    let mut dst = Tile::new(target.dx, target.dy)?;

    match mode {
        ViewMode::OnlyA => {
            copy_side_into(&mut dst, a, margin);
            return Some(dst);
        }
        ViewMode::OnlyB => {
            copy_side_into(&mut dst, b, margin);
            return Some(dst);
        }
        ViewMode::Overlay => {}
    }

    let r = opts.tolerance.clamp(0, MAX_TOLERANCE);
    let w = (target.dx + 2 * margin) as usize;
    let h = (target.dy + 2 * margin) as usize;
    let n = w.checked_mul(h)?;

    let mut ink_a = vec![0u8; n];
    let mut ink_b = vec![0u8; n];
    read_ink_plane(a, &mut ink_a, w, h);
    read_ink_plane(b, &mut ink_b, w, h);

    // `near_*` is "is there ink within the tolerance of here", which is the
    // question that decides shared from changed.
    let (near_a, near_b) = if r > 0 {
        let mut da = vec![0u8; n];
        let mut db = vec![0u8; n];
        let mut scratch = vec![0u8; n];
        dilate_ink(&ink_a, &mut da, w, h, r, &mut scratch);
        dilate_ink(&ink_b, &mut db, w, h, r, &mut scratch);
        (da, db)
    } else {
        (ink_a.clone(), ink_b.clone())
    };

    let table = InkTable::build(opts);
    let mask_plane = build_mask_plane(target, masks);
    let edge = MASK_EDGE_COLOR.bgr();
    let tw = target.dx as usize;

    for y in 0..target.dy {
        let src_row = (y + margin) as usize * w + margin as usize;
        let ia = &ink_a[src_row..];
        let ib = &ink_b[src_row..];
        let na = &near_a[src_row..];
        let nb = &near_b[src_row..];
        let mask = mask_plane.as_ref().map(|m| &m[y as usize * tw..][..tw]);
        let out = dst.row_mut(y);
        for x in 0..tw {
            let o = &mut out[x * 4..][..4];
            o[3] = 0xff;
            match mask.map(|m| m[x]).unwrap_or(0) {
                2 => {
                    o[..3].copy_from_slice(&edge);
                }
                1 => {
                    // Excluded: show the artwork but never colour it, washed out
                    // so "not compared" can never be mistaken for "nothing
                    // changed here".
                    let ink = ia[x].max(ib[x]) as i32;
                    let v = (255 - (ink * MASK_INK_PERCENT) / 100) as u8;
                    o[..3].fill(v);
                }
                _ => {
                    let matched_a = ia[x].min(nb[x]);
                    let matched_b = ib[x].min(na[x]);
                    let neutral = matched_a.max(matched_b);
                    let fa = table.from_a[(ia[x] - matched_a) as usize];
                    let fb = table.from_b[(ib[x] - matched_b) as usize];
                    // Only the agreed coverage is faded; the two leftovers keep
                    // their full strength, so turning this down empties the
                    // sheet of everything except what changed on it.
                    let base = 255 - shared_coverage(neutral, opts);
                    for i in 0..3 {
                        let v = ((base * fa[i] as i32 + 127) / 255) * fb[i] as i32;
                        o[i] = ((v + 127) / 255) as u8;
                    }
                }
            }
        }
    }
    Some(dst)
}

/// Reads a whole image's ink into a fresh `w * h` plane.
///
/// The sweep wants both planes at a fixed low resolution to hand to
/// [`crate::find_changes`], and does not want a composed tile at all.
pub fn ink_plane(px: Option<&Pixels>, w: usize, h: usize) -> Vec<u8> {
    let mut plane = vec![0u8; w * h];
    read_ink_plane(px, &mut plane, w, h);
    plane
}

/// One row of ink, for a caller that streams rather than holding a plane.
pub fn ink_row(px: Option<&Pixels>, y: i32, width: usize, out: &mut [u8]) -> bool {
    read_ink_row(px, y, width, out)
}
