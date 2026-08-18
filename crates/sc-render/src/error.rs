// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use std::fmt;

/// Hand-written rather than derived, so the distinctions the caller acts on
/// stay distinct: "this file is not a PDF" and "this file is not readable" want
/// different sentences in front of a person, and a single stringly-typed error
/// loses that on the first refactor.
#[derive(Debug)]
pub enum Error {
    /// The file could not be opened or read.
    Io(String),
    /// The file opened but is not a document this tool can read.
    Format(String),
    /// A page number outside the document.
    NoSuchPage(i32),
    /// The requested tile has no area, or is so large it cannot be allocated.
    BadGeometry,
    /// MuPDF refused, and this is what it said.
    Mupdf(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(s) => write!(f, "cannot read the file: {s}"),
            Error::Format(s) => write!(f, "not a document this tool can read: {s}"),
            Error::NoSuchPage(n) => write!(f, "this document has no sheet {n}"),
            Error::BadGeometry => write!(f, "the requested area is empty or too large"),
            Error::Mupdf(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

impl From<mupdf::Error> for Error {
    fn from(e: mupdf::Error) -> Self {
        Error::Mupdf(e.to_string())
    }
}
