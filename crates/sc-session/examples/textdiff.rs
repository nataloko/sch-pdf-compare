//! What did these two revisions of a sheet say differently?
use sc_diff::TextChangeKind;
use sc_session::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    let page: i32 = a[3].parse()?;
    for c in s.page_text_changes(page)? {
        match c.kind {
            TextChangeKind::Changed => {
                println!(
                    "  {:6.0},{:6.0}  {} -> {}",
                    c.rect.x, c.rect.y, c.before, c.after
                )
            }
            TextChangeKind::Removed => {
                println!("  {:6.0},{:6.0}  removed {}", c.rect.x, c.rect.y, c.before)
            }
            TextChangeKind::Added => {
                println!("  {:6.0},{:6.0}  added   {}", c.rect.x, c.rect.y, c.after)
            }
        }
    }
    Ok(())
}
