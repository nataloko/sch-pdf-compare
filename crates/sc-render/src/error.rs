// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

use std::fmt;

/// Hand-written rather than derived, so the distinctions the caller acts on
/// stay distinct: "this file is not a PDF" and "this file is not readable" want
/// different sentences in front of a person, and a single stringly-typed error
/// loses that on the first refactor.
#[derive(Debug)]
pub enum Error {
    /// The file could not be opened or read at all.
    Io(String),
    /// The file opened but is not a document this tool can read.
    Format(String),
    /// The file is a PDF, but encrypted, and we have no password for it.
    Locked(String),
    /// The file opened and has no pages.
    ///
    /// Its own case because MuPDF will happily open a badly damaged file,
    /// rebuild what it can and hand back nothing — which without this check
    /// reads as a comparison of two empty documents rather than as a failure.
    Empty(String),
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
            // Every one of these names the file. The shell opens two at once
            // and reports one failure; "cannot read the file" leaves the reader
            // to guess which of them it meant.
            Error::Io(s) => write!(f, "{s}"),
            Error::Format(s) => {
                write!(f, "{s} is not a PDF, or is too damaged to read.")
            }
            Error::Locked(s) => write!(
                f,
                "{s} is password-protected. Save a copy without the password and \
                 compare that."
            ),
            Error::Empty(s) => write!(
                f,
                "{s} has no pages. It may have been truncated or only partly \
                 downloaded."
            ),
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
