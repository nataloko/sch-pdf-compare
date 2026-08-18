//! The model: a pair of documents and everything the frontend asks about it.
//!
//! Owns the pairing, the compare options, the ignored regions, the tile cache
//! and the background sweep that scans every sheet for changes. This is the
//! whole application minus its pixels-on-screen.
