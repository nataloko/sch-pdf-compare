//! Pairing the two documents' sheets.
//!
//! Two mechanisms, and the manual one is not a fallback for a broken automatic
//! one — they answer different questions. A uniform offset is what a reader
//! nudges when a revision inserted a sheet at the front, and it stays
//! predictable. A content match handles a set whose sheets were reordered,
//! which no single offset can express.
//!
//! The signature is the sheet's text. Measured across three revisions of the
//! sample set — and across three different PDF producers, one of them using
//! Identity-H CID fonts where the others use WinAnsi Type1C — text comes out
//! consistently enough that the right sheet scores 0.75–0.98 and the runner-up
//! 0.06–0.63.

#![forbid(unsafe_code)]

use std::collections::HashSet;

use sc_diff::Pair;

/// What one sheet looks like to the matcher.
#[derive(Clone, Debug, Default)]
pub struct Signature {
    tokens: HashSet<String>,
}

impl Signature {
    /// Builds a signature from a sheet's extracted text.
    ///
    /// Tokens of three characters or more, so the column and row letters that
    /// edge every drawing frame — "A", "B", "1", "5" — do not make every sheet
    /// look like every other. `#`, `_`, `.` and brackets are kept because net
    /// names are made of them: `BUS1[0..7]` and `NET_RESET#` are exactly the
    /// distinguishing marks worth having.
    pub fn from_text(text: &str) -> Self {
        let mut tokens = HashSet::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() || matches!(ch, '_' | '#' | '.' | '[' | ']') {
                cur.push(ch);
            } else {
                if cur.chars().count() >= 3 {
                    tokens.insert(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
        if cur.chars().count() >= 3 {
            tokens.insert(cur);
        }
        Self { tokens }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Jaccard: how much of everything either sheet says do they both say.
    ///
    /// Two sheets with no text at all are not "identical", they are unknown, and
    /// answering 1.0 there would confidently pair every blank sheet in the set
    /// with the first one it met.
    pub fn similarity(&self, other: &Signature) -> f32 {
        if self.tokens.is_empty() || other.tokens.is_empty() {
            return 0.0;
        }
        let shared = self.tokens.intersection(&other.tokens).count() as f32;
        let total = self.tokens.union(&other.tokens).count() as f32;
        shared / total
    }
}

/// Below this a pair is worse than leaving both sheets unmatched.
///
/// The runner-up on the sample sets scores up to 0.63: one of them repeats a
/// channel sheet eight times over, differing only in a number. So this cannot be
/// set by "the best score wins". It is the alignment below that separates them,
/// using the order the sheets come in.
const MATCH_FLOOR: f32 = 0.35;

/// What leaving a sheet unmatched costs.
///
/// Small, but not zero: a set that genuinely gained a sheet should show it as
/// added rather than dragging every later sheet out of step, and free gaps would
/// let the aligner do exactly that.
const GAP: f32 = -0.05;

/// Matches two documents' sheets, in reading order.
///
/// Needleman-Wunsch rather than "best score wins": on a set with repeated
/// channel sheets the top two candidates for a sheet are both plausible, and
/// what separates them is that sheets come in an order. Greedy matching pairs
/// sheet 12 with sheet 15 and is never able to reconsider.
///
/// Returns one [`Pair`] per virtual sheet. A pair with one side 0 is a sheet
/// that exists in only one revision.
pub fn match_sheets(a: &[Signature], b: &[Signature]) -> Vec<Pair> {
    let (n, m) = (a.len(), b.len());
    if n == 0 && m == 0 {
        return Vec::new();
    }

    // score[i][j] = best alignment of a[i..] against b[j..].
    let mut score = vec![vec![0.0f32; m + 1]; n + 1];
    for i in (0..=n).rev() {
        for j in (0..=m).rev() {
            if i == n && j == m {
                continue;
            }
            let take_a = if i < n {
                score[i + 1][j] + GAP
            } else {
                f32::NEG_INFINITY
            };
            let take_b = if j < m {
                score[i][j + 1] + GAP
            } else {
                f32::NEG_INFINITY
            };
            let both = if i < n && j < m {
                score[i + 1][j + 1] + a[i].similarity(&b[j]) - MATCH_FLOOR
            } else {
                f32::NEG_INFINITY
            };
            score[i][j] = both.max(take_a).max(take_b);
        }
    }

    // Walk the table back out into the pairing it stands for.
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n || j < m {
        let both = if i < n && j < m {
            score[i + 1][j + 1] + a[i].similarity(&b[j]) - MATCH_FLOOR
        } else {
            f32::NEG_INFINITY
        };
        let take_a = if i < n {
            score[i + 1][j] + GAP
        } else {
            f32::NEG_INFINITY
        };
        let take_b = if j < m {
            score[i][j + 1] + GAP
        } else {
            f32::NEG_INFINITY
        };

        if both >= take_a && both >= take_b {
            out.push(Pair {
                page_a: i as i32 + 1,
                page_b: j as i32 + 1,
            });
            i += 1;
            j += 1;
        } else if take_a >= take_b {
            out.push(Pair {
                page_a: i as i32 + 1,
                page_b: 0,
            });
            i += 1;
        } else {
            out.push(Pair {
                page_a: 0,
                page_b: j as i32 + 1,
            });
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests;
