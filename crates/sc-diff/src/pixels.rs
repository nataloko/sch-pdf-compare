// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

/// The byte orders a rendered page can arrive in.
///
/// The fork this grew out of had a fourth, `Native`, for the 8-bit palette DIBs
/// MuPDF produces for line art on Windows — they were only readable through GDI
/// and had to be copied before they could be compared. Rendering our own pixels
/// means we choose the format, so that whole class of bug is gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    /// 32bpp B, G, R, A. What Qt calls `Format_RGB32` on a little-endian host,
    /// and therefore what the shell can wrap without a copy.
    Bgra8,
    /// 24bpp B, G, R.
    Bgr8,
    /// 32bpp R, G, B, A.
    Rgba8,
}

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Bgr8 => 3,
            _ => 4,
        }
    }
}

/// A borrowed view of somebody else's pixels.
///
/// Constructed only through [`Pixels::new`], which checks that the buffer is
/// actually as big as the geometry claims — the ABI hands these in from C, and
/// a stride that lies is otherwise an out-of-bounds read in the compose loop.
#[derive(Clone, Copy, Debug)]
pub struct Pixels<'a> {
    data: &'a [u8],
    width: i32,
    height: i32,
    stride: usize,
    format: PixelFormat,
}

impl<'a> Pixels<'a> {
    pub fn new(
        data: &'a [u8],
        width: i32,
        height: i32,
        stride: usize,
        format: PixelFormat,
    ) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let row_bytes = (width as usize).checked_mul(format.bytes_per_pixel())?;
        if stride < row_bytes {
            return None;
        }
        // The last row only needs its own bytes, not a full stride of them: a
        // tightly packed buffer is allowed to stop at the end of the picture.
        let needed = stride
            .checked_mul(height as usize - 1)?
            .checked_add(row_bytes)?;
        if data.len() < needed {
            return None;
        }
        Some(Self {
            data,
            width,
            height,
            stride,
            format,
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Row `y`, from its first pixel to the end of the buffer. Callers read
    /// forward from it and stop at `width`.
    pub(crate) fn row(&self, y: i32) -> &'a [u8] {
        &self.data[y as usize * self.stride..]
    }
}

/// A composed tile: 32bpp BGRA, tightly packed, owned.
///
/// Tightly packed because the shell wraps it in a `QImage` and every extra
/// degree of freedom there is one more thing to get wrong at a tile seam.
#[derive(Clone, Debug)]
pub struct Tile {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

impl Tile {
    pub(crate) fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let n = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        Some(Self {
            width,
            height,
            data: vec![0; n],
        })
    }

    pub const fn stride(&self) -> usize {
        self.width as usize * 4
    }

    pub(crate) fn row_mut(&mut self, y: i32) -> &mut [u8] {
        let s = self.stride();
        &mut self.data[y as usize * s..][..s]
    }

    /// The B, G, R of one pixel. For tests and for the shell's own assertions.
    pub fn bgr_at(&self, x: i32, y: i32) -> [u8; 3] {
        let i = y as usize * self.stride() + x as usize * 4;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}
