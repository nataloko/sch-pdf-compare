//! A quick look at a pair: sizes, and what the two comparisons report.
use sc_session::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    println!("  sheets: {}", s.page_count());
    for p in 1..=3.min(s.page_count()) {
        let r = s.scan_page(p)?;
        let text = s.page_text_changes(p)?.len();
        println!(
            "  sheet {p}: {} regions covering {:.0}% of the sheet, {text} text changes{}",
            r.changes.len(),
            r.coverage * 100.0,
            if r.size_mismatch {
                "   *** the two sheets are different sizes ***"
            } else {
                ""
            }
        );
    }
    Ok(())
}
