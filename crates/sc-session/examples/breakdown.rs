use sc_diff::TextChangeKind::*;
use sc_session::Session;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    let page: i32 = a[3].parse()?;
    let c = s.page_text_changes(page)?;
    let n = |k| c.iter().filter(|x| x.kind == k).count();
    println!(
        "  changed {}  added {}  removed {}  moved {}  (total {})",
        n(Changed),
        n(Added),
        n(Removed),
        n(Moved),
        c.len()
    );
    Ok(())
}
