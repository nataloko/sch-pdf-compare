// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use crate::Session;
use sc_diff::Pairing;
use sc_match::{match_sheets, Signature};
use sc_render::{Document, Result};

impl Session {
    /// Works out which sheet of B each sheet of A corresponds to, from what is
    /// written on them, and installs that as the pairing.
    ///
    /// For a set that simply gained a sheet at the front, nudging the delta by
    /// hand does the same job and is more predictable. This is for the case an
    /// offset cannot express: sheets reordered, or inserted in the middle.
    ///
    /// Costs one text extraction per sheet of both documents. Nothing is
    /// rendered, so it is far cheaper than a sweep.
    pub fn auto_match(&mut self) -> Result<()> {
        let (doc_a, doc_b) = self.docs();
        let sig_a = signatures(doc_a)?;
        let sig_b = signatures(doc_b)?;
        let pairs = match_sheets(&sig_a, &sig_b);
        self.set_pairing(Pairing::explicit(
            sig_a.len() as i32,
            sig_b.len() as i32,
            pairs,
        ));
        Ok(())
    }

    /// True when the pairing came from the documents rather than from an offset.
    pub fn pairing_is_automatic(&self) -> bool {
        self.pairing_ref().is_explicit()
    }
}

fn signatures(doc: &Document) -> Result<Vec<Signature>> {
    (1..=doc.page_count())
        .map(|p| doc.page_text(p).map(|t| Signature::from_text(&t)))
        .collect()
}
