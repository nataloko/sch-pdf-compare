//! How consistently do two revisions place their words? Decides whether a
//! text-level diff can match by position at all.
use sc_render::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let da = Document::open(&a[1])?;
    let db = Document::open(&a[2])?;
    let page: i32 = a[3].parse()?;
    let wa = da.page_words(page)?;
    let wb = db.page_words(page)?;
    println!("words: A={} B={}", wa.len(), wb.len());

    // For each word in A with an exact text twin in B, how far did it move?
    let mut moved: Vec<f32> = Vec::new();
    let mut exact_same_place = 0;
    let mut no_twin = 0;
    for w in &wa {
        let twin = wb.iter().filter(|o| o.text == w.text).min_by(|p, q| {
            let d = |r: &sc_render::Word| (r.x - w.x).abs() + (r.y - w.y).abs();
            d(p).partial_cmp(&d(q)).unwrap_or(std::cmp::Ordering::Equal)
        });
        match twin {
            Some(t) => {
                let d = (t.x - w.x).abs().max((t.y - w.y).abs());
                if d < 0.01 {
                    exact_same_place += 1;
                }
                moved.push(d);
            }
            None => no_twin += 1,
        }
    }
    moved.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f32| {
        moved
            .get(((moved.len() as f32 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    println!(
        "with an identical twin: {} ({} at the very same point), no twin at all: {}",
        moved.len(),
        exact_same_place,
        no_twin
    );
    println!(
        "displacement pt: median {:.3}  p90 {:.3}  p99 {:.3}  max {:.3}",
        pct(0.5),
        pct(0.9),
        pct(0.99),
        moved.last().copied().unwrap_or(0.0)
    );
    Ok(())
}
