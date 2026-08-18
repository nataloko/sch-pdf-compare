// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::Session;
use sc_diff::{diff_words, TextChange};
use sc_render::Result;

impl Session {
    /// What the two revisions of this sheet *say* differently.
    ///
    /// Complements the overlay rather than replacing it: the overlay finds
    /// anything drawn, including a re-routed wire that carries no text at all,
    /// while this finds what a reader can be told in words — a component value,
    /// a net name, a revision letter.
    ///
    /// It is also the answer to the one case the overlay handles badly. When two
    /// revisions went through different PDF producers the pixels of every glyph
    /// differ and the overlay reports about twenty-five regions a sheet; the
    /// words do not differ, and this reports the handful that genuinely did.
    ///
    /// Costs a text extraction of both sheets and nothing else — no rendering.
    pub fn page_text_changes(&self, page_no: i32) -> Result<Vec<TextChange>> {
        let pair = self.pair(page_no);
        let (doc_a, doc_b) = self.docs();
        // A sheet with no counterpart is entirely added or entirely removed, and
        // an empty side gives exactly that.
        let wa = if pair.page_a != 0 {
            doc_a.page_words(pair.page_a)?
        } else {
            Vec::new()
        };
        let wb = if pair.page_b != 0 {
            doc_b.page_words(pair.page_b)?
        } else {
            Vec::new()
        };
        Ok(diff_words(&wa, &wb))
    }
}
