# Design

Why this tool is shaped the way it is, and the mistakes already paid for.
Ported from `docs/compare-mode.md` in the SumatraPDF fork this grew out of, and
rewritten for a standalone application.

The motivating job is reviewing revisions of electronics schematics: 21- to
85-sheet A4 sets where a changed resistor value is a handful of pixels and
nobody can find it by eye.

## Why standalone, and what that changed

The first version was a fork of SumatraPDF. There, the comparison had to
*impersonate a document*: `EngineCompare` was an `EngineBase` wrapping two other
engines, because presenting the comparison as one synthetic document inherited
the whole viewer — tiled async rendering, zoom, scroll, fit modes, printing, the
command palette — without touching any of it. The alternative, a real two-pane
split, meant extracting a `DocumentPane` out of `MainWindow`: ~450 `hwndCanvas`
call sites, ~268 `win->AsFixed()` sites, ~70 per-view fields. That trade was
correct for a fork.

It is not a trade this project has to make. Owning the viewer means the
comparison can be an honest model, and six of the fork's thirteen recorded
gotchas simply stop existing — they were all consequences of pretending to be an
`EngineBase` inside somebody else's application. It also means side-by-side with
independent panning is a normal feature rather than a "2-up virtual page" trick.

## Shape

Rust core, Qt 6 Widgets shell, one flat C ABI between them. See `AGENTS.md` for
the rules that seam is held to, and `../Sterna` for the same arrangement proved
out on a larger application.

| Crate | What it is |
| --- | --- |
| `sc-diff` | The comparison kernel. No PDF, no threads, no toolkit. |
| `sc-render` | MuPDF, plus the CAD-enhance device. |
| `sc-match` | Pairing the two documents' sheets. |
| `sc-session` | The model: a document pair and everything asked about it. |
| `sc-ffi` | The C ABI, and the only crate the shell links. |

## Decisions, and why

Re-litigate these only with new evidence; each cost something to learn.

**Colour only the difference in coverage, not the coverage.** Composition splits
the two ink values into `shared = min(a, b)`, drawn neutral black, and
`diff = |a − b|`, drawn in that side's colour. An earlier version composed the
two inks directly; it tinted shared anti-aliased edges cyan and darkened shared
mid-tones. Identical at the four corners, different in the middle — which is
where line art lives.

**Tolerance mattered; alignment did not.** The brief expected shifted pages.
Measuring the real sample sets found every revision pair already pixel-
registered (offset 0,0; correlation 0.99–1.00) across three different PDF
producers — PDFCreator, Ghostscript and Microsoft Print to PDF. What they needed
was slack. Measured again on this project's own samples, at 150 dpi with 1 px of
tolerance:

| Pair | Raw clusters | With 1 px tolerance |
| --- | --- | --- |
| SET-ONE REV-P1 vs REV-P2, same producer | 14 | 7 real changes |
| SET-ONE REV-P2 vs REV-P3, PDFCreator vs MS Print to PDF | 102 | 1 |

Most of the raw pixel difference between two producers is sub-pixel
rasterisation fringe. Auto-alignment remains unimplemented and, on this
evidence, unneeded.

**Ignored regions are explicit, not inferred.** A drawing set shares a title
block, so a changed date colours all 85 sheets. Automatic repeat detection finds
it — but discounting it silently is wrong: a genuinely systematic change, a net
renamed across the set, looks identical to a heuristic, and hiding that is the
worst failure this tool could have. So the detection *offers*, and the reader
accepts, adjusts or declines. Excluded regions are drawn washed out inside a
dashed border and counted in the status line, because "not compared" must never
look like "nothing changed here".

**Page coordinates everywhere.** Ignored regions and change boxes are stored in
page space so they survive zoom and rotation, and so one rectangle covers the
same place on every sheet.

**PDF only, and not a viewer.** No epub, html, cbz, img or svg, and no
JavaScript engine — those MuPDF features are compiled out. This tool is not
trying to reach parity with the viewer it came from.

## Gotchas

From the fork. Every one of them produced *wrong output* rather than an obvious
failure, which is why they are written down.

### Still live here

1. **CAD-enhance must match across the two documents.** MuPDF's content
   detection decides per document whether a page is an engineering drawing and
   widens its hairlines accordingly. If one revision is detected as one and the
   other is not, every stroke differs and the whole sheet reads as changed.
   Decide once for the pair and apply the same answer to both.

2. **Tiles need margin for the tolerance dilation.** Each side must render with
   `tolerance` extra pixels of context, or every tile boundary grows a seam of
   false differences. `sc_diff::compose` takes that margin explicitly.

3. **Any offset applied to B must be whole device pixels.** Rendering rounds the
   transformed rect, and an arbitrary real translation can round differently at
   each corner and yield a B tile one pixel off A's. Only relevant if alignment
   is ever built, which is why it is recorded rather than fixed.

4. **Navigating to a change means page coordinates, not screen coordinates.**
   The next change is usually on a sheet that is not on screen, and screen
   coordinates for a page that is not laid out are meaningless. Scroll by
   (page, rect) and leave the zoom alone — the reader chose it.

5. **The sweep's final notification must come after the sweep is marked
   finished.** In the fork, per-sheet callbacks all ran while the sweep was
   still in progress, so a UI that checked "finished" on them never saw it and
   nothing fired again. Here the sweep does not call back at all: it sets its
   status and pokes a wakeup handle, and the shell reads the status. The
   ordering hazard cannot arise.

6. **`-for-testing` deliberately does not save settings.** Persistence has to be
   exercised without it.

### Dead, and why

- **"The composite must be a DIB, not a plain pixmap."** A Windows GDI printing
  constraint inside SumatraPDF. We own the print path.
- **"The controller takes ownership of the engine."** SumatraPDF's document
  lifecycle. Gone with `EngineBase`.
- **"`LoadArgs` normalises the path"** and **"use a synthetic path, not document
  A's"** — both were about smuggling a two-document comparison through an API
  that assumes one file. There is no such API here.
- **"Sub-renders come back as `PixmapFormat::Native`."** Those were the 8-bit
  palette DIBs MuPDF produces for line art on Windows, readable only through
  GDI. We choose the render format; it is BGRA8.
- **"`BuildPagesInfo()` asserts if called twice."** A SumatraPDF layout
  invariant.
- **"`TocTree`'s root is an invisible container."** A Win32 tree-view detail. The
  changed-sheet list is a `QTreeWidget` populated from plain data.

## What is next

Ranked for the schematic-review workflow. Items 1 and 2 are the fork's own
"what's next" list; both have been validated against the real samples.

1. **Automatic page matching** by content signature, for sets whose sheets were
   reordered or inserted. Text-token Jaccard signatures matched 21/21 pages
   correctly across all three revisions of SET-ONE and across three different PDF
   producers. Correct matches scored 0.75–0.98 against 0.06–0.63 for the
   runner-up — except sheets 10–17, which are near-duplicate channel sheets
   scoring 0.634 against each other, so sequence alignment is needed and greedy
   best-match is not enough.
2. **Text-level diff.** MuPDF's structured text gives text with positions.
   Extract, normalise and diff by position and string, and a changed component
   value becomes a table row rather than a red blob to squint at. Text extracts
   cleanly from all three sample producers, including one using Identity-H CID
   fonts where the others use WinAnsi Type1C, so this is producer-agnostic.
3. **Alignment** (projection-profile registration). Designed in the fork,
   unbuilt, and still no evidence any real document set needs it.
