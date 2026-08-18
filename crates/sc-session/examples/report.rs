//! Writes the change report for a pair to stdout.
use sc_session::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: report <earlier.pdf> <later.pdf>");
        std::process::exit(2);
    }
    let s = Session::open(&a[1], &a[2])?;
    let scanned: Vec<_> = (1..=s.page_count())
        .filter_map(|p| s.scan_page(p).ok())
        .collect();
    print!("{}", s.report(&scanned));
    Ok(())
}
