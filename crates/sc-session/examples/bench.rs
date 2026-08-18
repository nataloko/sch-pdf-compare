//! How long a tile costs, which decides how the viewer has to cache.
use sc_render::Tile;
use sc_session::Session;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut s = Session::open(&a[1], &a[2])?;
    // An optional third argument, because the tolerance is the one option that
    // costs anything: it is a dilation radius, and the ceiling was raised on
    // the strength of what this prints.
    if let Some(tol) = a.get(3).and_then(|t| t.parse().ok()) {
        let mut o = s.options();
        o.tolerance = tol;
        s.set_options(o);
    }
    println!("tolerance {}", s.options().tolerance);
    for dpi in [96.0f32, 150.0, 300.0] {
        let zoom = dpi / 72.0;
        let (w, h) = s.page_device_size(2, zoom)?;
        let t0 = Instant::now();
        s.compose_tile(2, zoom, Tile::whole(w, h), s.view_mode())?;
        let whole = t0.elapsed();
        let t0 = Instant::now();
        let n = 12;
        for i in 0..n {
            s.compose_tile(
                2,
                zoom,
                Tile::new((i % 4) * 512, (i / 4) * 512, 512, 512),
                s.view_mode(),
            )?;
        }
        let tiles = t0.elapsed();
        println!(
            "{dpi:5.0} dpi  page {w}x{h}: whole {:?}, {n} tiles of 512 {:?} ({:?} each)",
            whole,
            tiles,
            tiles / n as u32
        );
    }
    Ok(())
}
