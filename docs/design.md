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
registered, and what they needed instead was slack. Re-measured here through the
shipping algorithm — 100 dpi scan, 8 px cells, sheet 2 of SET-ONE:

| Pair | tol 0 | tol 1 | tol 2 |
| --- | --- | --- | --- |
| REV-P1 vs REV-P2, same producer | 10 regions / 2198 px | **6 / 1443** | 4 / 1251 |
| REV-P2 vs REV-P3, PDFCreator vs MS Print to PDF | 29 / 7809 px | **26 / 1969** | 8 / 1609 |
| REV-P1 vs itself | 0 | 0 | 0 |

Probing every whole-pixel offset within ±3 device pixels confirms the fork's
conclusion and sharpens it. For the same-producer pair there is a clear basin —
5–7 regions anywhere in ±1, jumping to 30+ beyond it — so the sheets are
registered and 1 px of tolerance is exactly the right amount. For the
cross-producer pair **there is no minimum at all**: the surface is flat at 14–26
regions everywhere in the probe. That residue is not a shifted page and no
alignment will remove it.

What it is: REV-P3 draws its text with CID TrueType fonts where the others use
subset Type1C, so the glyph shapes differ slightly everywhere there is text.
Tolerance still earns its place — it takes the unmatched ink from 7809 px to
1969, a 75% cut — but it cannot make two typefaces into one.

**So a cross-producer pair has a floor of roughly 25 reported regions a sheet,
and most of them are text.** That is why the text-level comparison exists, and
it settles the case: the same sheet reports 26 regions by pixels and **2** by
words, those two being the revision letter and the date.

Measured on the words themselves, 335 of that sheet's 341 words have an
identical twin in the other revision and the furthest any of them moved is
**0.985 pt** — so matching text by position is not merely possible across
producers, it is easy. The two comparisons answer different questions and the
tool shows both: only the pixels find a re-routed wire that carries no text, and
only the words can say `NET_ALPHA → NET_BRAVO`.

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

**Settings are per document pair, and hand-editable.** Working out where a set's
title block sits costs a reviewer a minute of attention; losing it when the
window closes makes the feature not worth using. They are JSON on purpose — the
regions worked out for one drawing set are usually right for the next one from
the same office, and copying them should not need the application. A malformed
file falls back to the defaults rather than refusing to start, and a file
written by a newer version is left alone rather than overwritten.

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
   ordering hazard cannot arise in that form — but see 6 and 7, which are the
   two shapes it took instead.

6. **"The worker finished" is not "the frontend collected".** The sweep sets
   `finished` and pokes; the event loop delivers that wakeup some time later,
   and only then is the sidebar rebuilt. Anything that waits on `finished` and
   immediately reads the UI sees the second-to-last answer — which is what two
   runs in three of the window test did, reporting 20 sheets of 21 and a status
   line still saying "scanning 20 of 21". `Session::sweepCollected()` is the
   flag that actually means what such a caller wants, and the distinction is
   worth keeping named rather than papered over with a wait.

7. **Do not `delete` a `QSocketNotifier` inside its own activated slot.**
   The sweep's last wakeup is where the frontend wants to stop watching, which
   is exactly the moment the notifier is emitting. Destroying it there cuts the
   emission short and the final update never lands. `setEnabled(false)` then
   `deleteLater()`.

8. **MuPDF's image scaler is not tile-invariant.** It chooses an image's
   subsample factor from the destination pixmap, so a tile that overlaps an
   embedded raster can differ from the same window of a full-page render by a
   few grey levels — measured at up to 17 on sheet 2 of the sample set, which
   carries a small logo. This is harmless *for the comparison*, because both
   documents are rendered with identical tile geometry and shift together, but
   it means a tile render is not byte-comparable to a full-page render and no
   test should assert that it is. Compare verdicts, not bytes.

9. **`-for-testing` deliberately does not save settings, and does not read
   them either.** Persistence has to be exercised without it, and a test that
   inherited a developer's real excluded regions would pass or fail depending on
   whose machine it ran on. The window test asserts that a `--for-testing` run
   leaves no settings file behind even after excluding regions and changing the
   tolerance.

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

## Comparing what the sheet says

Two tolerances, not one, and the reason is worth keeping.

Matching a word to an *identical* word can afford a generous radius — the text
matching exactly is already the evidence, and position only has to rule out a
different instance of the same string. Matching a word to a *different* one has
no such corroboration, so it stays tight or it pairs a label with its unrelated
neighbour and reports a change that never happened.

The first attempt used one tolerance of 2 pt, taken from the single sheet that
had been measured, where the largest displacement between producers was 0.985 pt.
Across the rest of the set it reaches 5.4 pt, and at 2 pt most of a cover sheet
came back as every word removed and the same word added. **A constant measured
on one sheet is a constant measured on nothing.**

Text that merely **moved** is its own kind of change. A sheet that was
re-laid-out moves dozens of identical labels, and reporting each as one thing
gone and another arrived doubles the noise and buries the handful that say
something different — sheet 8 of the sample set goes from 770 rows to 62 once
moves are counted rather than listed.

One more trap, met and fixed: a sheet with 47 labels reading `33R` will happily
match any of them to any other. An early diagnostic searched for the nearest
identical text anywhere on the sheet and reported a 9 pt median displacement,
which looked exactly like a 2% horizontal rescale between producers. It was not:
a distinctive token sat at the same coordinates in both files. **On a dense
sheet, "nearest word with the same text" is not evidence of anything.**

## Side by side, and printing

Side by side is one scroll and one zoom over a content area two sheets wide,
rather than two viewports kept in step. Synchronised panning is then not a
feature that can drift — there is only one thing to pan.

It earns its place where the overlay is at its worst. Where text changed, the
overlay draws both readings on top of each other and neither is legible; the
text panel says what they are, and this shows them where they sit.

The view mode is asked for **per tile** rather than read from the session,
because this view wants both single-document views on screen at once and neither
of them is "the" current mode. The tile cache keys on it too — without that, the
two panes ask for the same sheet at the same place and get each other's picture.

Printing turns the paper to match the drawing. A schematic set is landscape and
the default page is portrait, which prints the sheet at two-thirds the size it
could be. Every page carries a caption naming the two files, the view, the
tolerance, and — most of all — whether regions were excluded from the
comparison: a printout gets passed around without the application that made it,
and "part of this was not compared" has to travel with it.

## Windows

The core cross-compiles to `x86_64-pc-windows-gnu` with the MinGW toolchain, and
the resulting DLL loads and passes the whole ABI harness under Wine. MuPDF needed
no special handling, which is worth writing down because it was the one thing
about this plan that looked likely to go badly: the fork it grew out of builds
MuPDF through its own makefiles precisely to avoid that fight, and none of that
turned out to be necessary.

What is not solved is Qt. Ubuntu ships no MinGW build of Qt 6, so the shell
cannot be cross-built here; that wants Fedora's `mingw64-qt6-qtbase`, the route
the sibling Sterna project already takes, and its packaging can be copied
alongside.

## What is next

Ranked for the schematic-review workflow.

1. **Windows packaging.** The core cross-builds and passes its harness under
   Wine; the shell needs a distribution with a MinGW Qt 6, and then an installer.
2. **Alignment** (projection-profile registration). Designed in the fork,
   unbuilt, and now measured against: the one sample pair that looked like it
   might need it turns out to have no offset to find. Build it only if a
   document set turns up with a real basin the probe can see.
