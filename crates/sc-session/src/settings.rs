// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! What is remembered between sessions.
//!
//! The excluded regions are the reason this exists. Working out that a set's
//! title block changes on every sheet, and where it is, is a minute of a
//! reviewer's attention; losing it every time the window closes makes the
//! feature not worth using.
//!
//! JSON, and deliberately readable: someone who has worked out the excluded
//! regions for one drawing set should be able to copy them to the next one with
//! a text editor.

use std::path::PathBuf;
use std::sync::Mutex;

/// A directory named by the caller, used instead of the platform's own.
///
/// A test has to put the settings somewhere harmless, and doing that with an
/// environment variable means knowing which one — `XDG_CONFIG_HOME` or
/// `APPDATA` — and trusting that the frontend's way of setting it reaches this
/// process's view of the environment. Neither held: the window test set the
/// Unix one and passed on Linux while writing to the real location on Windows.
///
/// Naming the directory outright removes both problems. It is also the shape a
/// portable installation wants, where the settings live beside the application.
static SETTINGS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Puts the settings in `dir` rather than in the platform's own place.
pub fn set_settings_dir(dir: Option<PathBuf>) {
    if let Ok(mut slot) = SETTINGS_DIR.lock() {
        *slot = dir;
    }
}

use sc_diff::{Options, RectF, Rgb};
use serde::{Deserialize, Serialize};

/// Bumped when the shape changes in a way an older file cannot be read as. A
/// file from the future is left alone rather than overwritten — the person
/// running two versions should not silently lose the newer one's settings.
const VERSION: u32 = 1;

/// How many document pairs to remember. Past this the oldest goes; the file is
/// a convenience, not an archive.
const MAX_PAIRS: usize = 50;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StoredRect {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StoredPair {
    a: String,
    b: String,
    #[serde(default)]
    ignore: Vec<StoredRect>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Data {
    version: u32,
    #[serde(default = "default_tolerance")]
    tolerance: i32,
    #[serde(default = "default_only_a")]
    only_a: String,
    #[serde(default = "default_only_b")]
    only_b: String,
    #[serde(default)]
    last_pair: Option<[String; 2]>,
    #[serde(default)]
    pairs: Vec<StoredPair>,
}

fn default_tolerance() -> i32 {
    Options::default().tolerance
}
fn default_only_a() -> String {
    hex(Options::default().only_a)
}
fn default_only_b() -> String {
    hex(Options::default().only_b)
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: VERSION,
            tolerance: default_tolerance(),
            only_a: default_only_a(),
            only_b: default_only_b(),
            last_pair: None,
            pairs: Vec::new(),
        }
    }
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn unhex(s: &str, fallback: Rgb) -> Rgb {
    let t = s.trim_start_matches('#');
    if t.len() != 6 {
        return fallback;
    }
    let p = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    match (p(0), p(2), p(4)) {
        (Some(r), Some(g), Some(b)) => Rgb::new(r, g, b),
        _ => fallback,
    }
}

pub struct Settings {
    path: Option<PathBuf>,
    data: Data,
}

impl Settings {
    /// Reads the settings file, or starts from the defaults.
    ///
    /// Never fails. A missing file is the normal first run, and an unreadable or
    /// malformed one is not worth refusing to start over — the worst case is a
    /// reviewer redoing a minute of work, and the alternative is a tool that
    /// will not open because of a stray character in a config file.
    pub fn load() -> Self {
        Self::from_path(settings_path())
    }

    /// The same, from a named file.
    ///
    /// Exists so tests can point at a temporary file instead of setting
    /// `XDG_CONFIG_HOME` — an environment variable is process-wide, and Rust
    /// runs tests in threads, so two of them doing that race each other.
    pub fn at(path: PathBuf) -> Self {
        Self::from_path(Some(path))
    }

    fn from_path(path: Option<PathBuf>) -> Self {
        let data = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Data>(&t).ok())
            .filter(|d| d.version <= VERSION)
            .unwrap_or_default();
        Self { path, data }
    }

    /// Where the settings live, for a message that has to name it.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Writes the file, creating its directory.
    ///
    /// Written beside and renamed, so an interrupted save leaves the previous
    /// settings intact rather than half a file.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn options(&self) -> Options {
        let d = Options::default();
        Options {
            only_a: unhex(&self.data.only_a, d.only_a),
            only_b: unhex(&self.data.only_b, d.only_b),
            tolerance: self.data.tolerance.clamp(0, sc_diff::MAX_TOLERANCE),
        }
    }

    pub fn set_options(&mut self, o: Options) {
        self.data.tolerance = o.tolerance;
        self.data.only_a = hex(o.only_a);
        self.data.only_b = hex(o.only_b);
    }

    /// The regions excluded for this document pair, if it has been seen before.
    pub fn ignore_rects(&self, a: &str, b: &str) -> Vec<RectF> {
        self.data
            .pairs
            .iter()
            .find(|p| p.a == a && p.b == b)
            .map(|p| {
                p.ignore
                    .iter()
                    .map(|r| RectF::new(r.x, r.y, r.dx, r.dy))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn set_ignore_rects(&mut self, a: &str, b: &str, rects: &[RectF]) {
        let ignore: Vec<StoredRect> = rects
            .iter()
            .map(|r| StoredRect {
                x: r.x,
                y: r.y,
                dx: r.dx,
                dy: r.dy,
            })
            .collect();
        // Most recently used last, so trimming drops the oldest.
        self.data.pairs.retain(|p| !(p.a == a && p.b == b));
        self.data.pairs.push(StoredPair {
            a: a.to_owned(),
            b: b.to_owned(),
            ignore,
        });
        let excess = self.data.pairs.len().saturating_sub(MAX_PAIRS);
        self.data.pairs.drain(..excess);
        self.data.last_pair = Some([a.to_owned(), b.to_owned()]);
    }

    /// The pair opened most recently, for offering to reopen it.
    pub fn last_pair(&self) -> Option<(&str, &str)> {
        self.data
            .last_pair
            .as_ref()
            .map(|p| (p[0].as_str(), p[1].as_str()))
    }
}

/// `$XDG_CONFIG_HOME/sch-pdf-compare/settings.json`, or the platform's
/// equivalent. `None` when the environment says nothing about where to put it,
/// in which case nothing is saved and nothing breaks.
pub fn settings_path() -> Option<PathBuf> {
    if let Ok(slot) = SETTINGS_DIR.lock() {
        if let Some(dir) = slot.as_ref() {
            return Some(dir.join("sch-pdf-compare").join("settings.json"));
        }
    }
    let dir = config_dir()?;
    Some(dir.join("sch-pdf-compare").join("settings.json"))
}

#[cfg(windows)]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(not(windows))]
fn config_dir() -> Option<PathBuf> {
    // Written out rather than pulling in a crate for it: this is the whole of
    // the XDG rule that applies here.
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return Some(PathBuf::from(x));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
}
