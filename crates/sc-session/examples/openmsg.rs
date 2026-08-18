//! What a person is told when a file will not open.
use sc_session::Session;
fn main() {
    for p in std::env::args().skip(1) {
        match Session::open(&p, &p) {
            Ok(s) => println!("  ok    {} ({} sheets)", p, s.page_count()),
            Err(e) => println!("  fail  {e}"),
        }
    }
}
