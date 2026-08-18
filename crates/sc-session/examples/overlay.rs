//! Dump one composed sheet as a PPM, for looking at.
//!
//! `cargo run -p sc-session --example overlay -- A.pdf B.pdf 2 out.ppm`
//!
//! PPM because it is fifteen lines and no dependency; anything can convert it.

use sc_render::Tile;
use sc_session::Session;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: overlay <A.pdf> <B.pdf> <sheet> <out.ppm> [dpi]");
        std::process::exit(2);
    }
    let dpi: f32 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(150.0);
    let page: i32 = a[3].parse()?;

    let s = Session::open(&a[1], &a[2])?;
    let zoom = dpi / 72.0;
    let (w, h) = s.page_device_size(page, zoom)?;
    let t = s.compose_tile(page, zoom, Tile::whole(w, h))?;

    let mut out = std::io::BufWriter::new(std::fs::File::create(&a[4])?);
    write!(out, "P6\n{w} {h}\n255\n")?;
    for y in 0..h {
        for x in 0..w {
            let [b, g, r] = t.bgr_at(x, y);
            out.write_all(&[r, g, b])?;
        }
    }
    let found = s.scan_page(page)?;
    eprintln!("sheet {page}: {} changes, {} px", found.changes.len(), found.pixels);
    Ok(())
}
