// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::Session;
use sc_diff::{compose, PixelFormat, Pixels, Point, Rect, Size, Tile as Composed, TileMasks};
use sc_render::{Document, Result, Tile};

impl Session {
    /// Composes one tile of the comparison, ready for the screen.
    ///
    /// `tile` is in device pixels at `zoom` with its origin at the sheet's
    /// top-left, which is the same coordinate space [`sc_render::Document`]
    /// renders in.
    ///
    /// Both sides are rendered with `tolerance` extra pixels of context on every
    /// edge. Without that margin the dilation at a tile's border has no
    /// neighbours to look at and every seam in the viewport grows a line of
    /// changes that are not on the drawing.
    pub fn compose_tile(&self, page_no: i32, zoom: f32, tile: Tile) -> Result<Composed> {
        let pair = self.pair(page_no);
        let margin = self.options().tolerance.max(0);
        let grown = Tile::new(
            tile.x - margin,
            tile.y - margin,
            tile.width + 2 * margin,
            tile.height + 2 * margin,
        );
        let (doc_a, doc_b) = self.docs();
        let ra = render_side(doc_a, pair.page_a, zoom, grown)?;
        let rb = render_side(doc_b, pair.page_b, zoom, grown)?;

        let rects = self.tile_masks(zoom, tile);
        let masks = TileMasks {
            rects: &rects,
            device_origin: Point::new(tile.x, tile.y),
        };

        compose(
            as_pixels(ra.as_ref(), grown).as_ref(),
            as_pixels(rb.as_ref(), grown).as_ref(),
            Size::new(tile.width, tile.height),
            margin,
            self.view_mode(),
            &self.options(),
            (!rects.is_empty()).then_some(&masks),
        )
        .ok_or(sc_render::Error::BadGeometry)
    }

    /// The excluded regions as device pixels within this tile.
    ///
    /// Everything is offered to the compositor, including rectangles that miss
    /// the tile entirely — clipping is its job, and doing it twice is how the
    /// dash phase ends up disagreeing between two neighbouring tiles.
    fn tile_masks(&self, zoom: f32, tile: Tile) -> Vec<Rect> {
        self.ignore_rects()
            .iter()
            .map(|r| {
                let d = r.to_device(zoom);
                Rect::new(d.x - tile.x, d.y - tile.y, d.dx, d.dy)
            })
            .collect()
    }
}

fn render_side(
    doc: &Document,
    page_no: i32,
    zoom: f32,
    tile: Tile,
) -> Result<Option<sc_render::Raster>> {
    if page_no == 0 {
        return Ok(None);
    }
    Ok(Some(doc.render(page_no, zoom, tile)?))
}

fn as_pixels(r: Option<&sc_render::Raster>, tile: Tile) -> Option<Pixels<'_>> {
    let r = r?;
    Pixels::new(
        r.samples(),
        tile.width.min(r.width()),
        tile.height.min(r.height()),
        r.stride(),
        PixelFormat::Bgr8,
    )
}
