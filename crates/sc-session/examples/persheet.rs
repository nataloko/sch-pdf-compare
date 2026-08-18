//! Pixel regions per sheet, to see which sheets are noisy and why.
use sc_session::Session;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    for p in 1..=s.page_count() {
        let r = s.scan_page(p)?;
        let t = s.page_text_changes(p)?.len();
        if r.changes.len() >= 10 || t >= 10 {
            println!(
                "  sheet {p:2}: {:4} pixel regions, {t:5} text changes",
                r.changes.len()
            );
        }
    }
    Ok(())
}
