// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! Comparing what a sheet *says*, rather than how it was drawn.
//!
//! This exists because of a measurement. On a pair of revisions that went
//! through different PDF producers, the pixel comparison reports about 25
//! changed regions a sheet and no amount of alignment removes them: one file
//! draws its text with CID TrueType fonts where the other uses subset Type1C,
//! so every glyph differs slightly everywhere there is writing.
//!
//! The words themselves do not. Measured on that same sheet, 335 of 341 words
//! have an identical twin in the other revision and the furthest any of them
//! moved is **0.985 pt**. Comparing text to text turns twenty-six things to
//! squint at into six things to read.

use crate::RectF;

/// One word and where it sits on the page, in points.
#[derive(Clone, PartialEq, Debug)]
pub struct Word {
    pub text: String,
    pub rect: RectF,
}

impl Word {
    pub fn new(text: impl Into<String>, rect: RectF) -> Self {
        Self {
            text: text.into(),
            rect,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TextChangeKind {
    /// In the later revision only.
    Added = 0,
    /// In the earlier revision only.
    Removed = 1,
    /// In the same place on both, saying something different. The one a reader
    /// most wants: a component value that moved from 10k to 12k.
    Changed = 2,
    /// The same text, elsewhere on the sheet.
    ///
    /// Worth telling apart from an addition and a removal, which is what it
    /// otherwise looks like — twice. A sheet that was re-laid-out moves dozens
    /// of identical labels, and reporting each as one thing gone and another
    /// arrived buries the handful of real changes underneath.
    Moved = 3,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextChange {
    pub kind: TextChangeKind,
    /// What the earlier revision said. Empty for an addition.
    pub before: String,
    /// What the later one says. Empty for a removal.
    pub after: String,
    /// Where it is, in page points, taken from whichever side has it.
    pub rect: RectF,
}

/// How far a word may sit from an *identical* word and still be the same one.
///
/// Generous, because matching the text exactly is already strong evidence and
/// the position only has to rule out a different instance of the same string.
///
/// Sized from measurement, and the first attempt at it was wrong: two points,
/// taken from a single sheet where the largest displacement between producers
/// was 0.985 pt. Across the rest of the set it reaches **5.4 pt** — the shift is
/// systematic per sheet, with medians from 0.74 to 1.56 — and at two points most
/// of a cover sheet came back as every word removed and the same word added.
pub const SAME_WORD_PT: f32 = 6.0;

/// How far a word may sit from a *different* word and still be taken as a
/// change to it rather than one thing gone and another arrived.
///
/// Tight, and deliberately not the same number: this pass has no text to
/// corroborate the position, so a generous radius here pairs a label with its
/// unrelated neighbour and reports a change that did not happen. A value that
/// genuinely changed stays where it was.
pub const SAME_PLACE_PT: f32 = 2.0;

/// Compares two sheets' words.
///
/// Three passes, in order of how sure each is:
///
/// 1. Same text near the same place — unchanged, and said no more about.
/// 2. Of what is left, same place but different text — a change, reported with
///    both readings, which is the answer worth having.
/// 3. Of what is still left, the same text elsewhere on the sheet has moved.
/// 4. Whatever remains is in one revision only.
///
/// Passes 1 and 2 use different tolerances, for the reason given on each.
///
/// Doing 1 before 2 matters: a sheet with the same net label repeated down a
/// bus would otherwise pair the first occurrence with the wrong one and report
/// two spurious changes.
pub fn diff_words(before: &[Word], after: &[Word]) -> Vec<TextChange> {
    let before = dedupe_stamps(before);
    let after = dedupe_stamps(after);
    let (before, after) = (&before[..], &after[..]);
    let mut used_after = vec![false; after.len()];
    let mut matched_before = vec![false; before.len()];

    // 1. Identical text, nearest position.
    for (i, w) in before.iter().enumerate() {
        if let Some(j) = nearest(after, &used_after, w, SAME_WORD_PT, Some(&w.text)) {
            used_after[j] = true;
            matched_before[i] = true;
        }
    }

    let mut out = Vec::new();

    // 2. Same place, different text.
    for (i, w) in before.iter().enumerate() {
        if matched_before[i] {
            continue;
        }
        if let Some(j) = nearest(after, &used_after, w, SAME_PLACE_PT, None) {
            used_after[j] = true;
            matched_before[i] = true;
            out.push(TextChange {
                kind: TextChangeKind::Changed,
                before: w.text.clone(),
                after: after[j].text.clone(),
                rect: w.rect,
            });
        }
    }

    // 3. Of what is still unmatched, the same text somewhere else on the sheet
    // has moved rather than been replaced.
    for (i, w) in before.iter().enumerate() {
        if matched_before[i] {
            continue;
        }
        let Some(j) = after
            .iter()
            .enumerate()
            .find(|(j, o)| !used_after[*j] && o.text == w.text)
            .map(|(j, _)| j)
        else {
            continue;
        };
        used_after[j] = true;
        matched_before[i] = true;
        out.push(TextChange {
            kind: TextChangeKind::Moved,
            before: w.text.clone(),
            after: w.text.clone(),
            // Where it was. The reader is looking at the overlay, where this is
            // the mark in the earlier revision's colour.
            rect: w.rect,
        });
    }

    // 4. Whatever is left belongs to one side only.
    for (i, w) in before.iter().enumerate() {
        if !matched_before[i] {
            out.push(TextChange {
                kind: TextChangeKind::Removed,
                before: w.text.clone(),
                after: String::new(),
                rect: w.rect,
            });
        }
    }
    for (j, w) in after.iter().enumerate() {
        if !used_after[j] {
            out.push(TextChange {
                kind: TextChangeKind::Added,
                before: String::new(),
                after: w.text.clone(),
                rect: w.rect,
            });
        }
    }

    // Reading order, so stepping through them follows the sheet.
    out.sort_by(|a, b| {
        (a.rect.y, a.rect.x)
            .partial_cmp(&(b.rect.y, b.rect.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// How far apart two copies of the same text have to be before they are two
/// things rather than one thing drawn twice.
///
/// Deliberately much tighter than [`SAME_WORD_PT`]: a bus draws the same net
/// label down a column a few points apart and those are genuinely separate
/// labels, while a stamp drawn on top of itself is at the very same coordinates.
const SAME_STAMP_PT: f32 = 1.0;

/// Collapses text drawn more than once in the same place.
///
/// Sheet 2 of the sample set stamps its whole content three times over, so
/// without this every change on it is reported three times — and a reader
/// stepping through a change list would visit each one three times over.
fn dedupe_stamps(words: &[Word]) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for w in words {
        let dup = out.iter().any(|o| {
            o.text == w.text
                && (o.rect.x - w.rect.x).abs() <= SAME_STAMP_PT
                && (o.rect.y - w.rect.y).abs() <= SAME_STAMP_PT
        });
        if !dup {
            out.push(w.clone());
        }
    }
    out
}

/// The nearest unused word to `w` within `tolerance`, optionally requiring its
/// text to match exactly.
fn nearest(
    words: &[Word],
    used: &[bool],
    w: &Word,
    tolerance: f32,
    text: Option<&str>,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (j, o) in words.iter().enumerate() {
        if used[j] {
            continue;
        }
        if let Some(t) = text {
            if o.text != t {
                continue;
            }
        }
        let dx = (o.rect.x - w.rect.x).abs();
        let dy = (o.rect.y - w.rect.y).abs();
        if dx > tolerance || dy > tolerance {
            continue;
        }
        let d = dx + dy;
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, j));
        }
    }
    best.map(|(_, j)| j)
}
