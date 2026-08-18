// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! The comparison as a document somebody else can read.
//!
//! A reviewer's output is not a window. It is the list of what changed, going
//! into a change note, an email, or a review record — and until it can leave the
//! application, every one of those is retyped by hand.
//!
//! Markdown because it is readable as it stands, pastes into most things, and
//! renders as a table where that helps.

use std::fmt::Write;

use sc_diff::TextChangeKind;

use crate::{Session, SheetChanges};

/// The most table rows one sheet contributes before the rest are summarised.
///
/// A report is something a person reads. A sheet that was re-drawn can produce
/// hundreds of genuine differences, and printing all of them turns the document
/// into something nobody opens.
const MAX_ROWS: usize = 40;

/// What a sheet contributes to the report.
struct SheetReport {
    page_no: i32,
    size_mismatch: bool,
    coverage: f32,
    /// Which sheet of each document, for a set where they do not line up.
    page_a: i32,
    page_b: i32,
    regions: usize,
    ignored: i32,
    text: Vec<(TextChangeKind, String, String)>,
}

impl Session {
    /// Builds the report from scans already done.
    ///
    /// `scanned` is what the sweep collected. Taking it as an argument rather
    /// than re-scanning is the point: the caller has the results already, and a
    /// report that quietly re-rendered eighty-five sheets would be a menu item
    /// that appears to hang.
    ///
    /// The text differences *are* worked out here, because they cost a text
    /// extraction and no rendering at all.
    pub fn report(&self, scanned: &[SheetChanges]) -> String {
        let mut sheets = Vec::new();
        let mut changed = 0;
        for s in scanned {
            let text = self
                .page_text_changes(s.page_no)
                .unwrap_or_default()
                .into_iter()
                .map(|c| (c.kind, c.before, c.after))
                .collect::<Vec<_>>();
            if s.changes.is_empty() && text.is_empty() {
                continue;
            }
            changed += 1;
            let pair = self.pair(s.page_no);
            sheets.push(SheetReport {
                page_no: s.page_no,
                size_mismatch: s.size_mismatch,
                coverage: s.coverage,
                page_a: pair.page_a,
                page_b: pair.page_b,
                regions: s.changes.len(),
                ignored: s.ignored,
                text,
            });
        }

        let mut out = String::new();
        let _ = writeln!(out, "# What changed between these two revisions\n");
        let _ = writeln!(out, "- Earlier: `{}`", self.path_a());
        let _ = writeln!(out, "- Later: `{}`", self.path_b());
        let _ = writeln!(
            out,
            "- {} sheets compared, {changed} with something on them",
            self.page_count()
        );
        let tol = self.options().tolerance;
        let _ = writeln!(
            out,
            "- Tolerance: {tol} device pixel{}",
            if tol == 1 { "" } else { "s" }
        );
        if tol > sc_diff::TOLERANCE_HIDES_MOVEMENT {
            // The report leaves the application and is read without it. A
            // tolerance this wide is a legitimate choice and it changes what
            // "nothing changed here" means, so it travels with the numbers it
            // produced rather than being left behind on a status line.
            let _ = writeln!(
                out,
                "- **At this tolerance a stroke that merely moved is not \
                 reported as a change.**"
            );
        }
        if !self.ignore_rects().is_empty() {
            // Named, not buried: a reader of this report has to be able to see
            // that part of every sheet was deliberately not compared.
            let _ = writeln!(
                out,
                "- **{} region(s) excluded from the comparison on every sheet.** \
                 Anything inside them was not compared.",
                self.ignore_rects().len()
            );
        }
        if self.pairing_is_automatic() {
            let _ = writeln!(out, "- Sheets were paired by content, not by position");
        } else if self.page_delta() != 0 {
            let _ = writeln!(out, "- Sheet pairing offset by {}", self.page_delta());
        }
        if scanned.len() < self.page_count() as usize {
            let _ = writeln!(
                out,
                "\n> Only {} of {} sheets had been scanned when this was written.",
                scanned.len(),
                self.page_count()
            );
        }

        let mismatched = sheets.iter().filter(|s| s.size_mismatch).count();
        if mismatched > 0 {
            let _ = writeln!(
                out,
                "\n> **{} a different size in the two revisions.** \
                 Those sheets were compared at the first document's size with the other \
                 cropped. Reissue the two revisions at the same paper size to compare them.",
                if mismatched == 1 {
                    "One sheet is".to_string()
                } else {
                    format!("{mismatched} sheets are")
                }
            );
        }

        if sheets.is_empty() {
            let _ = writeln!(out, "\nNothing changed.");
            return out;
        }

        for s in &sheets {
            let _ = writeln!(out, "\n## {}\n", sheet_title(s));
            if s.size_mismatch {
                // First, and in bold. Everything below it on this sheet is
                // measured against the wrong thing.
                let _ = writeln!(
                    out,
                    "> **The two revisions of this sheet are different sizes on paper.** \
                     They were compared at the first document's size, with the other \
                     cropped, so nothing below is a reliable account of what changed.\n"
                );
            }
            let mut notes = Vec::new();
            if s.regions > 0 {
                notes.push(if s.regions == 1 {
                    "1 region of the drawing differs".to_string()
                } else {
                    format!("{} regions of the drawing differ", s.regions)
                });
            }
            if s.ignored > 0 {
                notes.push(format!(
                    "{} inside an excluded region, not compared",
                    s.ignored
                ));
            }
            if s.coverage >= 0.25 {
                notes.push(format!(
                    "they cover about {}% of the sheet, so it was substantially redrawn",
                    (s.coverage * 100.0).round() as i32
                ));
            }
            if !notes.is_empty() {
                let _ = writeln!(out, "{}.\n", notes.join("; "));
            }

            // Text that only moved is summarised, not listed. A sheet that was
            // re-laid-out moves dozens of identical labels, and forty rows
            // saying `+3V3` moved bury the three that say something different.
            let moved = s
                .text
                .iter()
                .filter(|(k, _, _)| *k == TextChangeKind::Moved)
                .count();
            let listed: Vec<_> = s
                .text
                .iter()
                .filter(|(k, _, _)| *k != TextChangeKind::Moved)
                .collect();
            if moved > 0 {
                let _ = writeln!(
                    out,
                    "{moved} piece{} of text moved without changing.\n",
                    if moved == 1 { "" } else { "s" }
                );
            }

            if listed.is_empty() {
                let _ = writeln!(
                    out,
                    "Nothing reads differently; the differences are in the drawing itself."
                );
                continue;
            }
            let _ = writeln!(out, "| Was | Is now |");
            let _ = writeln!(out, "| --- | --- |");
            for (kind, before, after) in listed.iter().take(MAX_ROWS) {
                let (l, r) = match kind {
                    TextChangeKind::Changed => (escape(before), escape(after)),
                    TextChangeKind::Removed => (escape(before), "_removed_".to_string()),
                    TextChangeKind::Added => ("_added_".to_string(), escape(after)),
                    // Summarised above; never reaches the table.
                    TextChangeKind::Moved => continue,
                };
                let _ = writeln!(out, "| {l} | {r} |");
            }
            if listed.len() > MAX_ROWS {
                // Said out loud. A table that stopped without saying so would
                // read as the complete list of what changed on this sheet.
                let _ = writeln!(
                    out,
                    "\n_{} more, not listed. This sheet was substantially \
                     re-laid-out; compare it in the application._",
                    listed.len() - MAX_ROWS
                );
            }
        }
        out
    }
}

fn sheet_title(s: &SheetReport) -> String {
    if s.page_a == 0 {
        format!("Sheet {} — added in the later revision", s.page_no)
    } else if s.page_b == 0 {
        format!("Sheet {} — removed", s.page_no)
    } else if s.page_a == s.page_b {
        format!("Sheet {}", s.page_no)
    } else {
        // The two documents call it different things, so say both.
        format!(
            "Sheet {} (earlier sheet {}, later sheet {})",
            s.page_no, s.page_a, s.page_b
        )
    }
}

/// Net names are full of characters a Markdown table would eat — `|` ends a
/// cell, and a schematic uses `#`, `[` and `_` freely.
fn escape(s: &str) -> String {
    let t = s.replace('\\', r"\\").replace('|', r"\|");
    if t.trim().is_empty() {
        "_(blank)_".to_string()
    } else {
        format!("`{t}`")
    }
}
