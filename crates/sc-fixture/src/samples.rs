// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! Finding the real drawing sets, without naming them.
//!
//! The sets this project was built against are customer drawings. They are not
//! in the repository and neither are their names — a public repository holding
//! a customer's board codes and net names is the same leak as holding the files.
//!
//! `samples/sets.json` says which file plays which part and what each test
//! should find. It lives inside `samples/`, so it is ignored along with the
//! drawings. Without it every test that needs it returns early, and the
//! `sc-fixture` tests cover the same ground on documents the test wrote itself.

use std::path::PathBuf;

pub struct Samples {
    root: PathBuf,
    json: String,
}

impl Samples {
    /// `None` when there is no manifest, which is the normal case for anyone
    /// who is not the person with the drawings.
    pub fn load() -> Option<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");
        let json = std::fs::read_to_string(root.join("sets.json")).ok()?;
        Some(Self { root, json })
    }

    /// The absolute path of one side of a named pair, e.g. `("large", "a")`.
    pub fn path(&self, set: &str, side: &str) -> Option<String> {
        let name = self.field(set, side)?;
        let p = self.root.join(name.trim_matches('"'));
        p.exists().then(|| p.to_string_lossy().into_owned())
    }

    pub fn both(&self, set: &str) -> Option<(String, String)> {
        Some((self.path(set, "a")?, self.path(set, "b")?))
    }

    /// A number the manifest says a test should find.
    pub fn number(&self, set: &str, key: &str) -> Option<i32> {
        self.field(set, key)?.parse().ok()
    }

    /// A string the manifest carries, such as a net label to look for.
    pub fn text(&self, set: &str, key: &str) -> Option<String> {
        Some(self.field(set, key)?.trim_matches('"').to_owned())
    }

    /// A top-level string, for the few things that belong to no pair.
    pub fn top(&self, key: &str) -> Option<String> {
        Some(scan(&self.json, key)?.trim_matches('"').to_owned())
    }

    /// A deliberately small reader rather than a JSON dependency in a test
    /// helper: the file is written by hand, read only here, and adding a parser
    /// to two crates' dev-dependencies to find four fields is not a trade worth
    /// making.
    fn field(&self, set: &str, key: &str) -> Option<String> {
        let at = self.json.find(&format!("\"{set}\""))?;
        let rest = &self.json[at..];
        let end = rest[1..]
            .find("\n  \"")
            .map(|e| e + 1)
            .unwrap_or(rest.len());
        scan(&rest[..end], key)
    }
}

/// The value following `"key":`, up to the next comma or closing brace.
fn scan(text: &str, key: &str) -> Option<String> {
    let at = text.find(&format!("\"{key}\""))?;
    let after = &text[at + key.len() + 2..];
    let colon = after.find(':')? + 1;
    let value = &after[colon..];
    let end = value.find([',', '\n', '}']).unwrap_or(value.len());
    let v = value[..end].trim();
    (!v.is_empty()).then(|| v.to_owned())
}
