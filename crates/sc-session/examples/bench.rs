//! How long a tile costs, which decides how the viewer has to cache.
use sc_render::Tile;
use sc_session::Session;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    for dpi in [96.0f32, 150.0, 300.0] {
        let zoom = dpi / 72.0;
        let (w, h) = s.page_device_size(2, zoom)?;
        let t0 = Instant::now();
        s.compose_tile(2, zoom, Tile::whole(w, h))?;
        let whole = t0.elapsed();
        let t0 = Instant::now();
        let n = 12;
        for i in 0..n {
            s.compose_tile(2, zoom, Tile::new((i % 4) * 512, (i / 4) * 512, 512, 512))?;
        }
        let tiles = t0.elapsed();
        println!(
            "{dpi:5.0} dpi  page {w}x{h}: whole {:?}, {n} tiles of 512 {:?} ({:?} each)",
            whole, tiles, tiles / n as u32
        );
    }
    Ok(())
}
