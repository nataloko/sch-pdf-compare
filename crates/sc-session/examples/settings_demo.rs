//! Writes a settings file so its shape can be eyeballed.
use sc_diff::RectF;
use sc_session::Settings;

fn main() {
    let mut s = Settings::load();
    s.set_ignore_rects(
        "/drawings/example REVA.pdf",
        "/drawings/example REVB.pdf",
        &[RectF::new(600.0, 570.0, 220.0, 25.0)],
    );
    s.save().expect("saves");
    println!(
        "{}",
        s.path()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
}
