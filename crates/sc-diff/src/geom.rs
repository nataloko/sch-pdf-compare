// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

/// A colour with no alpha. The comparison only ever lays ink on paper.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The channel order the output buffer is written in.
    pub const fn bgr(self) -> [u8; 3] {
        [self.b, self.g, self.r]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct Size {
    pub dx: i32,
    pub dy: i32,
}

impl Size {
    pub const fn new(dx: i32, dy: i32) -> Self {
        Self { dx, dy }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, dx: i32, dy: i32) -> Self {
        Self { x, y, dx, dy }
    }

    /// Half-open on both axes, so abutting rectangles do not both claim the
    /// pixel on their shared edge.
    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.dx && p.y >= self.y && p.y < self.y + self.dy
    }
}

/// A rectangle in page space, in points.
///
/// Change boxes and excluded regions are stored like this rather than in device
/// pixels so they survive zoom and rotation, and so one rectangle covers the
/// same place on every sheet — which is exactly what a shared title block needs.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, dx: f32, dy: f32) -> Self {
        Self { x, y, dx, dy }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.dx && y >= self.y && y < self.y + self.dy
    }

    /// True when every corner of `other` is inside this one.
    pub fn contains_rect(&self, other: &RectF) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.dx <= self.x + self.dx
            && other.y + other.dy <= self.y + self.dy
    }

    /// Device pixels at `zoom` back to points.
    pub fn from_device(r: Rect, zoom: f32) -> Self {
        Self {
            x: r.x as f32 / zoom,
            y: r.y as f32 / zoom,
            dx: r.dx as f32 / zoom,
            dy: r.dy as f32 / zoom,
        }
    }

    /// Points to device pixels at `zoom`, rounded outward so a rectangle never
    /// loses a pixel it partly covers.
    pub fn to_device(self, zoom: f32) -> Rect {
        let x0 = (self.x * zoom).floor() as i32;
        let y0 = (self.y * zoom).floor() as i32;
        let x1 = ((self.x + self.dx) * zoom).ceil() as i32;
        let y1 = ((self.y + self.dy) * zoom).ceil() as i32;
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}
