//! Shows what the sample manifest resolves to, for checking it by hand.
fn main() {
    match sc_fixture::Samples::load() {
        None => println!("no samples/sets.json — tests needing it will skip"),
        Some(s) => {
            for set in ["same_producer", "cross_producer", "rotated", "large"] {
                match s.both(set) {
                    Some(_) => println!("{set}: ok"),
                    None => println!("{set}: MISSING"),
                }
            }
            println!(
                "same_producer sheets      = {:?}",
                s.number("same_producer", "sheets")
            );
            println!(
                "same_producer regions     = {:?}",
                s.number("same_producer", "regions_on_probe_sheet")
            );
            println!(
                "same_producer total       = {:?}",
                s.number("same_producer", "total_regions")
            );
            println!(
                "cross_producer regions    = {:?}",
                s.number("cross_producer", "regions_on_probe_sheet")
            );
            println!(
                "large sheets              = {:?}",
                s.number("large", "sheets")
            );
            println!(
                "renamed                   = {:?} -> {:?}",
                s.text("same_producer", "renamed_from"),
                s.text("same_producer", "renamed_to")
            );
            println!("a_net_label               = {:?}", s.top("a_net_label"));
        }
    }
}
