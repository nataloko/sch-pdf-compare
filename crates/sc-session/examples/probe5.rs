use sc_render::Document;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let page: i32 = a[3].parse()?;
    for path in [&a[1], &a[2]] {
        let w = Document::open(path)?.page_words(page)?;
        // same rule diff_words uses internally
        let mut kept: Vec<&sc_render::Word> = Vec::new();
        for x in &w {
            if !kept.iter().any(|o| {
                o.text == x.text
                    && (o.rect.x - x.rect.x).abs() <= 1.0
                    && (o.rect.y - x.rect.y).abs() <= 1.0
            }) {
                kept.push(x);
            }
        }
        println!(
            "  {}: {} words -> {} after collapsing stamps",
            path.rsplit('/').next().unwrap_or(path),
            w.len(),
            kept.len()
        );
    }
    Ok(())
}
