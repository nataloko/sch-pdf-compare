// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

/// Which sheet of each document a virtual page stands for.
///
/// A pair with one side 0 is a sheet that exists in only one revision. It
/// renders as entirely added or entirely removed rather than as a plain page,
/// so an unmatched sheet cannot be mistaken for an unchanged one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pair {
    /// 1-based page in the first document; 0 means no matching sheet.
    pub page_a: i32,
    pub page_b: i32,
}

/// Lays the two documents' sheets side by side on one axis, offset by
/// `page_delta`: sheet *n* of the first document pairs with sheet *n + delta* of
/// the second.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Pairing {
    pub n_a: i32,
    pub n_b: i32,
    pub page_delta: i32,
    pub first_a_page: i32,
    pub page_count: i32,
    /// An explicit sheet-by-sheet match, when one has been worked out from the
    /// documents' contents. Empty means the uniform `page_delta` above.
    ///
    /// Two mechanisms rather than one because they answer different questions:
    /// a delta is what a reader nudges when a sheet was inserted at the front,
    /// and it stays predictable. A content match handles a set that was
    /// reordered, which no single offset can express.
    explicit: Vec<Pair>,
}

impl Pairing {
    /// The virtual page range has to cover *both* documents, so it can start
    /// before the first document's page 1 when the second one has extra sheets
    /// ahead of it. Leaving a sheet unreachable would silently hide a whole page
    /// from the comparison, which is the one failure this tool must not have.
    pub fn build(n_a: i32, n_b: i32, delta: i32) -> Self {
        let first = 0.min(-delta);
        let last = n_a.max(n_b - delta);
        Self {
            n_a,
            n_b,
            page_delta: delta,
            first_a_page: first + 1,
            page_count: 0.max(last - first),
            explicit: Vec::new(),
        }
    }

    /// A pairing worked out sheet by sheet, in reading order.
    ///
    /// The caller has already decided which sheets face each other; this only
    /// stores it. `page_delta` is left at 0 and means nothing here — nudging a
    /// content match with an offset would be nudging an answer, not a guess.
    pub fn explicit(n_a: i32, n_b: i32, pairs: Vec<Pair>) -> Self {
        Self {
            n_a,
            n_b,
            page_delta: 0,
            first_a_page: 1,
            page_count: pairs.len() as i32,
            explicit: pairs,
        }
    }

    /// True when this pairing came from the documents rather than from an
    /// offset, so the UI can say which it is showing.
    pub fn is_explicit(&self) -> bool {
        !self.explicit.is_empty()
    }

    pub fn at(&self, page_no: i32) -> Pair {
        if page_no < 1 || page_no > self.page_count {
            return Pair::default();
        }
        if let Some(p) = self.explicit.get((page_no - 1) as usize) {
            return *p;
        }
        let a = self.first_a_page + page_no - 1;
        let b = a + self.page_delta;
        Pair {
            page_a: if a >= 1 && a <= self.n_a { a } else { 0 },
            page_b: if b >= 1 && b <= self.n_b { b } else { 0 },
        }
    }
}
