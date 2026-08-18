//! Does one similarity transform cover both cases?
//!
//! Renders B at a scale that makes its sheet the same size as A's, then compares
//! the ink the way the scan does. If translation and scale are the same problem,
//! the same estimate should leave a same-size pair alone and fix a rescaled one.
use sc_diff::{find_changes, ink_plane, Options, PixelFormat, Pixels};
use sc_render::{Document, Tile};

fn plane(d: &Document, page: i32, zoom: f32, w: i32, h: i32) -> Vec<u8> {
    let r = d.render(page, zoom, Tile::whole(w, h)).expect("renders");
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
    let da = Document::open(&a[1])?;
    let db = Document::open(&a[2])?;
    let zoom = 100.0 / 72.0;

    let (aw, ah) = da.page_size(page)?;
    let (bw, bh) = db.page_size(page)?;
    // The scale that makes B's sheet the same size on paper as A's.
    let sx = aw / bw;
    let sy = ah / bh;
    println!("  A {aw:.0}x{ah:.0} pt, B {bw:.0}x{bh:.0} pt  ->  scale {sx:.4} x {sy:.4}");

    let (w, h) = da.page_device_size(page, zoom)?;
    let ia = plane(&da, page, zoom, w, h);
    let opts = Options {
        tolerance: 1,
        ..Default::default()
    };
    let (uw, uh) = (w as usize, h as usize);

    for (label, z) in [("B as-is", zoom), ("B scaled to A", zoom * sx)] {
        let ib = plane(&db, page, z, w, h);
        let n = find_changes(&ia, &ib, uw, uh, &opts).len();
        let ink_a = ia.iter().filter(|&&v| v > 96).count();
        let ink_b = ib.iter().filter(|&&v| v > 96).count();
        println!("  {label:14}: ink A {ink_a:6}, ink B {ink_b:6}, regions {n}");
    }
    Ok(())
}
