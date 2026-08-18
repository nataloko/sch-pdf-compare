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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pairing {
    pub n_a: i32,
    pub n_b: i32,
    pub page_delta: i32,
    pub first_a_page: i32,
    pub page_count: i32,
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
        }
    }

    pub fn at(&self, page_no: i32) -> Pair {
        if page_no < 1 || page_no > self.page_count {
            return Pair::default();
        }
        let a = self.first_a_page + page_no - 1;
        let b = a + self.page_delta;
        Pair {
            page_a: if a >= 1 && a <= self.n_a { a } else { 0 },
            page_b: if b >= 1 && b <= self.n_b { b } else { 0 },
        }
    }
}
