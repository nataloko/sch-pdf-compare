//! How long the whole-set sweep takes.
use sc_session::Session;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let s = Session::open(&a[1], &a[2])?;
    println!("{} sheets", s.page_count());

    let t0 = Instant::now();
    let sweep = s.start_sweep().expect("starts");
    let mut got = 0;
    while !sweep.status().finished {
        got += sweep.take_results().len();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    got += sweep.take_results().len();
    println!(
        "sweep: {:?} for {got} sheets ({:?}/sheet)",
        t0.elapsed(),
        t0.elapsed() / got.max(1) as u32
    );
    Ok(())
}
