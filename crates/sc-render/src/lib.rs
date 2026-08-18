//! MuPDF: open a document, ask a page its size, render a rect of it to pixels.
//!
//! Also owns the CAD-enhance pass-through device ported from the fork, which
//! darkens typical CAD-export grays and widens hairlines so a schematic stays
//! legible zoomed out. The decision to enable it is per *document*, which is
//! why `sc-session` forces it to agree across a pair: if one revision is
//! detected as an engineering drawing and the other is not, every stroke
//! differs and the whole sheet reads as changed.
