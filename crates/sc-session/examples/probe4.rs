//! What do the words on one sheet actually look like?
use sc_render::Document;
use std::collections::HashMap;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let page: i32 = a[3].parse()?;
    for path in [&a[1], &a[2]] {
        let d = Document::open(path)?;
        let w = d.page_words(page)?;
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for x in &w {
            *counts.entry(x.text.as_str()).or_default() += 1;
        }
        let mut top: Vec<_> = counts.iter().collect();
        top.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!("{}", path.rsplit('/').next().unwrap_or(path));
        println!("  {} words, {} distinct", w.len(), counts.len());
        println!("  most repeated: {:?}", &top[..top.len().min(6)]);
        // Where do the copies of one very common token sit?
        if let Some((tok, _)) = top.first() {
            let places: Vec<_> = w
                .iter()
                .filter(|x| x.text.as_str() == **tok)
                .take(6)
                .map(|x| (x.rect.x as i32, x.rect.y as i32))
                .collect();
            println!("  first few '{tok}': {places:?}");
        }
        // A distinctive token, to compare across the two files
        for probe in ["PIN_A0", "PIN_D0", "+3V3"] {
            let places: Vec<_> = w
                .iter()
                .filter(|x| x.text == probe)
                .map(|x| (x.rect.x as i32, x.rect.y as i32))
                .collect();
            if !places.is_empty() {
                println!(
                    "  '{probe}' x{}: {:?}",
                    places.len(),
                    &places[..places.len().min(4)]
                );
            }
        }
    }
    Ok(())
}
