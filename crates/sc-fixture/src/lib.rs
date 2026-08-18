//! Minimal PDFs, written by hand, for tests.
//!
//! The real drawings are customer material: they live in `samples/`, they are
//! never committed, and a clone without them would otherwise have almost no
//! coverage of anything above the pixel kernel. These stand in — small, exact,
//! and made in the test that needs them, so there is no binary in the
//! repository and nothing to keep in step with the code.
//!
//! Deliberately hand-assembled rather than pulled from a PDF-writing crate. The
//! point is a file whose every byte is known, including the awkward ones a real
//! set will eventually contain.

#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::io;
use std::path::Path;

/// One sheet: some text, and some lines to stand in for the drawing.
#[derive(Clone, Debug, Default)]
pub struct Sheet {
    /// `(x, y, size, text)` in PDF user space, which has its origin at the
    /// bottom left — the opposite way up from everything else here.
    pub text: Vec<(f32, f32, f32, String)>,
    /// `(x0, y0, x1, y1)`.
    pub lines: Vec<(f32, f32, f32, f32)>,
    pub width: f32,
    pub height: f32,
}

impl Sheet {
    /// A landscape A4 sheet, the shape a schematic actually comes in.
    pub fn a4_landscape() -> Self {
        Self {
            width: 842.0,
            height: 595.0,
            ..Default::default()
        }
    }

    pub fn text(mut self, x: f32, y: f32, s: &str) -> Self {
        self.text.push((x, y, 10.0, s.to_owned()));
        self
    }

    pub fn line(mut self, x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        self.lines.push((x0, y0, x1, y1));
        self
    }

    fn content(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "1 w 0 G");
        for (x0, y0, x1, y1) in &self.lines {
            let _ = writeln!(s, "{x0} {y0} m {x1} {y1} l S");
        }
        for (x, y, size, t) in &self.text {
            let _ = writeln!(s, "BT /F1 {size} Tf {x} {y} Td ({}) Tj ET", escape(t));
        }
        s
    }
}

/// `(`, `)` and `\` end or escape a PDF string literal.
fn escape(s: &str) -> String {
    s.replace('\\', r"\\")
        .replace('(', r"\(")
        .replace(')', r"\)")
}

/// Writes a PDF of these sheets.
///
/// The cross-reference table's offsets have to be the real byte positions, so
/// the file is assembled as bytes throughout and each object's start is recorded
/// as it is written. MuPDF would rebuild a broken table without complaint, which
/// is exactly why it is worth getting right here — a fixture that is quietly
/// repaired on every open is not testing what it looks like it is testing.
pub fn write(path: &Path, sheets: &[Sheet]) -> io::Result<()> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str| {
        offsets.push(out.len());
        let n = offsets.len();
        out.extend_from_slice(format!("{n} 0 obj\n{body}\nendobj\n").as_bytes());
    };

    out.extend_from_slice(b"%PDF-1.4\n");
    // A binary comment, so anything transferring the file treats it as binary.
    out.extend_from_slice(b"%\xe2\xe3\xcf\xd3\n");

    // 1: catalog, 2: page tree, 3: the font every sheet uses.
    // Pages are 4, 6, 8 ... with their content streams alongside.
    let first_page_obj = 4;
    let kids: Vec<String> = (0..sheets.len())
        .map(|i| format!("{} 0 R", first_page_obj + i * 2))
        .collect();
    obj(&mut out, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>");
    obj(
        &mut out,
        &mut offsets,
        &format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            sheets.len()
        ),
    );
    obj(
        &mut out,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );

    for (i, sheet) in sheets.iter().enumerate() {
        let content = sheet.content();
        let stream_obj = first_page_obj + i * 2 + 1;
        obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {stream_obj} 0 R >>",
                sheet.width, sheet.height
            ),
        );
        obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Length {} >>\nstream\n{content}endstream",
                content.len()
            ),
        );
    }

    let xref_at = out.len();
    let count = offsets.len() + 1;
    out.extend_from_slice(format!("xref\n0 {count}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n")
            .as_bytes(),
    );
    std::fs::write(path, out)
}
