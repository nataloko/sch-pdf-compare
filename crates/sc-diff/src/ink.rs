// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::{Options, PixelFormat, Pixels, SHARED_INK_FULL};

/// How much ink a pixel carries: 0 for paper white, 255 for solid black.
///
/// Schematics are line art on white, so luminance is a faithful stand-in for
/// coverage, and it keeps anti-aliased edges as partial values instead of
/// quantising them to on/off — which is the whole reason the overlay can tell a
/// stroke that moved from a stroke that appeared.
pub fn ink_from_rgb(r: u8, g: u8, b: u8) -> u8 {
    let lum = (77 * r as i32 + 150 * g as i32 + 29 * b as i32) >> 8;
    (255 - lum) as u8
}

/// What fraction of one channel is left after laying down `ink` of a colour
/// whose value in that channel is `col`: 255 for no ink, `col` for full ink.
fn ink_leaves(ink: u8, col: u8) -> u8 {
    (255 - (ink as i32 * (255 - col as i32) + 127) / 255) as u8
}

/// Lays down three inks subtractively: the coverage the two documents agree on
/// as neutral black, and whatever is left over on each side in that side's
/// colour.
///
/// White where nothing is drawn, A's colour where only A draws, B's where only
/// B does, black where both do. Colouring only the *unmatched* part is what
/// keeps a shared anti-aliased edge the same neutral grey it started as. An
/// earlier version composed the two inks directly and tinted every shared edge
/// cyan — identical at the four corners, wrong everywhere in between, which is
/// where line art lives.
///
/// `opts.shared_ink` fades the agreed coverage — and only that — towards white,
/// so a sheet can be reduced to nothing but what changed on it. The two
/// leftovers are never touched by it: fading the differences as well would be
/// fading the answer.
///
/// Returns the pixel in the output buffer's byte order: blue, green, red.
pub fn compose_ink(neutral: u8, only_a: u8, only_b: u8, opts: &Options) -> [u8; 3] {
    let base = 255 - shared_coverage(neutral, opts);
    let a_bgr = opts.only_a.bgr();
    let b_bgr = opts.only_b.bgr();
    let mut out = [0u8; 3];
    for i in 0..3 {
        let v = base * ink_leaves(only_a, a_bgr[i]) as i32;
        let v = ((v + 127) / 255) * ink_leaves(only_b, b_bgr[i]) as i32;
        out[i] = ((v + 127) / 255) as u8;
    }
    out
}

/// The agreed coverage after the fade, which is what actually gets drawn as
/// neutral ink. One definition, used by [`compose_ink`] and by the tile loop.
pub(crate) fn shared_coverage(neutral: u8, opts: &Options) -> i32 {
    let pct = opts.shared_ink.clamp(0, SHARED_INK_FULL);
    (neutral as i32 * pct + SHARED_INK_FULL / 2) / SHARED_INK_FULL
}

/// Per-coverage channel factors, so the compose loop does lookups and multiplies
/// instead of unpacking the colours again for every pixel.
pub(crate) struct InkTable {
    pub(crate) from_a: [[u8; 3]; 256],
    pub(crate) from_b: [[u8; 3]; 256],
}

impl InkTable {
    pub(crate) fn build(opts: &Options) -> Self {
        let a_bgr = opts.only_a.bgr();
        let b_bgr = opts.only_b.bgr();
        let mut t = InkTable {
            from_a: [[0; 3]; 256],
            from_b: [[0; 3]; 256],
        };
        for ink in 0..256usize {
            for i in 0..3 {
                t.from_a[ink][i] = ink_leaves(ink as u8, a_bgr[i]);
                t.from_b[ink][i] = ink_leaves(ink as u8, b_bgr[i]);
            }
        }
        t
    }
}

/// Fills `out[0..width)` with the ink coverage of row `y`.
///
/// Pixels past the source's own extent count as blank paper: a sub-render can
/// land a pixel short of the tile through rounding, and reading junk there would
/// paint a false change down the tile's edge. `None` for the pixels — an
/// unmatched sheet — is the caller's business and returns false.
pub fn read_ink_row(px: Option<&Pixels>, y: i32, width: usize, out: &mut [u8]) -> bool {
    let Some(px) = px else {
        return false;
    };
    if width == 0 || out.len() < width {
        return false;
    }
    let out = &mut out[..width];
    if y < 0 || y >= px.height() {
        out.fill(0);
        return true;
    }
    let n = width.min(px.width() as usize);
    let row = px.row(y);
    match px.format() {
        PixelFormat::Rgba8 => {
            for (x, o) in out[..n].iter_mut().enumerate() {
                let p = &row[x * 4..];
                *o = ink_from_rgb(p[0], p[1], p[2]);
            }
        }
        fmt => {
            let step = fmt.bytes_per_pixel();
            for (x, o) in out[..n].iter_mut().enumerate() {
                let p = &row[x * step..];
                *o = ink_from_rgb(p[2], p[1], p[0]);
            }
        }
    }
    out[n..].fill(0);
    true
}

/// Reads a whole image's ink into a `w * h` plane.
///
/// Returns false, leaving the plane blank, when the side is not there at all —
/// which is how an unmatched sheet ends up drawn entirely in the other
/// document's colour.
pub(crate) fn read_ink_plane(px: Option<&Pixels>, plane: &mut [u8], w: usize, h: usize) -> bool {
    let Some(px) = px else {
        plane[..w * h].fill(0);
        return false;
    };
    for y in 0..h {
        let row = &mut plane[y * w..][..w];
        if !read_ink_row(Some(px), y as i32, w, row) {
            row.fill(0);
        }
    }
    true
}

/// Greyscale dilation by a square of the given radius, as two 1-D passes.
///
/// The point is tolerance: comparing A against a dilated B asks "is there ink
/// *near* here in B", which is the right question when two exporters rasterise
/// the same stroke a fraction of a pixel apart. On a real cross-producer pair
/// this is the difference between one change and a hundred, so it is not a
/// nicety.
///
/// `scratch` holds the intermediate and must be `w * h` bytes like the rest.
pub fn dilate_ink(src: &[u8], dst: &mut [u8], w: usize, h: usize, radius: i32, scratch: &mut [u8]) {
    let n = w * h;
    assert!(
        src.len() >= n && dst.len() >= n && scratch.len() >= n,
        "planes are w*h"
    );
    if radius <= 0 {
        dst[..n].copy_from_slice(&src[..n]);
        return;
    }
    let r = radius as usize;
    for y in 0..h {
        let s = &src[y * w..][..w];
        let d = &mut scratch[y * w..][..w];
        for (x, o) in d.iter_mut().enumerate() {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r).min(w - 1);
            *o = *s[x0..=x1].iter().max().expect("window is never empty");
        }
    }
    for y in 0..h {
        let y0 = y.saturating_sub(r);
        let y1 = (y + r).min(h - 1);
        let d = &mut dst[y * w..][..w];
        d.copy_from_slice(&scratch[y0 * w..][..w]);
        for yy in y0 + 1..=y1 {
            let s = &scratch[yy * w..][..w];
            for (o, v) in d.iter_mut().zip(s) {
                *o = (*o).max(*v);
            }
        }
    }
}
