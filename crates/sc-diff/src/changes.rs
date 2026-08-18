// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::ink::dilate_ink;
use crate::{Options, Rect, CHANGE_CELL, CHANGE_MIN_PIXELS, MAX_TOLERANCE};

/// One region where the two documents disagree, in the scanned image's pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Change {
    pub box_: Rect,
    pub pixels: i32,
}

/// Groups the ink the two documents do not share into regions a reader can be
/// taken to one at a time.
///
/// Works on a coarse cell grid rather than single pixels: a changed component is
/// a blob a few millimetres across, and per-pixel connected components would
/// report every stroke of it as its own destination — forty stops to walk one
/// edit.
pub fn find_changes(ink_a: &[u8], ink_b: &[u8], w: usize, h: usize, opts: &Options) -> Vec<Change> {
    let mut out = Vec::new();
    if w == 0 || h == 0 {
        return out;
    }
    let n = w * h;
    if ink_a.len() < n || ink_b.len() < n {
        return out;
    }
    let r = opts.tolerance.clamp(0, MAX_TOLERANCE);
    let mut dil_a = vec![0u8; n];
    let mut dil_b = vec![0u8; n];
    let mut scratch = vec![0u8; n];
    dilate_ink(ink_a, &mut dil_a, w, h, r, &mut scratch);
    dilate_ink(ink_b, &mut dil_b, w, h, r, &mut scratch);

    let cell = CHANGE_CELL as usize;
    let cw = w.div_ceil(cell);
    let ch = h.div_ceil(cell);
    let mut cell_pixels = vec![0i32; cw * ch];
    const THR: u8 = 96;
    for y in 0..h {
        let row = y * w;
        let cy = y / cell;
        for x in 0..w {
            let a = ink_a[row + x] > THR;
            let b = ink_b[row + x] > THR;
            let unmatched = (a && dil_b[row + x] <= THR) || (b && dil_a[row + x] <= THR);
            if unmatched {
                cell_pixels[cy * cw + x / cell] += 1;
            }
        }
    }

    // Bridge neighbouring cells so one edit does not come back as a handful of
    // separate destinations. A changed component — a resistor and its value, a
    // re-routed wire — spans several cells with gaps between its strokes, so
    // grow the occupied set by one cell before labelling, then measure only the
    // cells that actually held something.
    let mut grown = vec![false; cw * ch];
    for cy in 0..ch {
        for cx in 0..cw {
            if cell_pixels[cy * cw + cx] == 0 {
                continue;
            }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < cw && (ny as usize) < ch {
                        grown[ny as usize * cw + nx as usize] = true;
                    }
                }
            }
        }
    }

    let mut label = vec![false; cw * ch];
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..cw * ch {
        if label[start] || !grown[start] {
            continue;
        }
        stack.clear();
        stack.push(start);
        label[start] = true;
        let (mut min_x, mut min_y) = (cw, ch);
        let (mut max_x, mut max_y) = (0isize - 1, 0isize - 1);
        let mut pixels = 0i32;
        while let Some(idx) = stack.pop() {
            let cx = idx % cw;
            let cy = idx / cw;
            if cell_pixels[idx] > 0 {
                pixels += cell_pixels[idx];
                min_x = min_x.min(cx);
                max_x = max_x.max(cx as isize);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy as isize);
            }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= cw || ny as usize >= ch {
                        continue;
                    }
                    let n_idx = ny as usize * cw + nx as usize;
                    if !label[n_idx] && grown[n_idx] {
                        label[n_idx] = true;
                        stack.push(n_idx);
                    }
                }
            }
        }
        if pixels >= CHANGE_MIN_PIXELS && max_x >= min_x as isize {
            let (min_x, min_y) = (min_x as i32, min_y as i32);
            let (max_x, max_y) = (max_x as i32, max_y as i32);
            out.push(Change {
                box_: Rect::new(
                    min_x * CHANGE_CELL,
                    min_y * CHANGE_CELL,
                    (max_x - min_x + 1) * CHANGE_CELL,
                    (max_y - min_y + 1) * CHANGE_CELL,
                ),
                pixels,
            });
        }
    }
    out
}
