//! Does applying an estimated transform improve the comparison, or only look
//! like it should?
use sc_diff::{find_changes, ink_plane, Options, PixelFormat, Pixels};
use sc_render::{Document, Tile};

fn plane(d: &Document, page: i32, zoom: f32, t: Tile, w: i32, h: i32) -> Vec<u8> {
    let r = d.render(page, zoom, t).expect("renders");
    ink_plane(
        Pixels::new(
            r.samples(),
            w.min(r.width()),
            h.min(r.height()),
            r.stride(),
            PixelFormat::Bgr8,
        )
        .as_ref(),
        w as usize,
        h as usize,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let page: i32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    // The offset to try, in points, on each axis.
    let tx: f32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let ty: f32 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);

    let da = Document::open(&a[1])?;
    let db = Document::open(&a[2])?;
    let zoom = 100.0 / 72.0;
    let (w, h) = da.page_device_size(page, zoom)?;
    let ia = plane(&da, page, zoom, Tile::whole(w, h), w, h);
    let opts = Options {
        tolerance: 1,
        ..Default::default()
    };

    // Whole device pixels only. A real translation rounds differently at each
    // corner and yields a tile one pixel off the other's.
    let px = (tx * zoom).round() as i32;
    let py = (ty * zoom).round() as i32;
    for (label, dx, dy) in [("as-is", 0, 0), ("shifted", px, py)] {
        let ib = plane(&db, page, zoom, Tile::new(dx, dy, w, h), w, h);
        let n = find_changes(&ia, &ib, w as usize, h as usize, &opts);
        let px_sum: i32 = n.iter().map(|c| c.pixels).sum();
        println!(
            "  {label:8} (dx {dx:3}, dy {dy:3} px): {:3} regions, {px_sum:6} unmatched px",
            n.len()
        );
    }
    Ok(())
}
