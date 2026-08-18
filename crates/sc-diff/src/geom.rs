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
