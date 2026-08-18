//! Coverage and counts for every sheet of a pair.
use sc_session::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    let mut hist = [0usize; 5];
    for p in 1..=s.page_count() {
        let r = s.scan_page(p)?;
        let t = s.page_text_changes(p)?.len();
        let band = ((r.coverage * 4.0) as usize).min(4);
        hist[band] += 1;
        if !r.changes.is_empty() && (r.coverage > 0.4 || r.changes.len() > 40) {
            println!(
                "    sheet {p:2}: {:3} regions, {:3.0}% covered, {t:4} text changes",
                r.changes.len(),
                r.coverage * 100.0
            );
        }
    }
    let total: usize = (1..=s.page_count())
        .map(|p| s.scan_page(p).map(|r| r.changes.len()).unwrap_or(0))
        .sum();
    println!("  coverage bands 0-25/25-50/50-75/75-100/100%: {hist:?}, {total} regions in total");
    Ok(())
}
