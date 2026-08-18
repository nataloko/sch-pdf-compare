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
shipping algorithm — 100 dpi scan, 8 px cells, one sheet of the 21-sheet set:

| Pair | tol 0 | tol 1 | tol 2 |
| --- | --- | --- | --- |
| consecutive revisions, same producer | 10 regions / 2198 px | **6 / 1443** | 4 / 1251 |
| two revisions apart, different producer | 29 / 7809 px | **26 / 1969** | 8 / 1609 |
| a revision against itself | 0 | 0 | 0 |

Probing every whole-pixel offset within ±3 device pixels confirms the fork's
conclusion and sharpens it. For the same-producer pair there is a clear basin —
5–7 regions anywhere in ±1, jumping to 30+ beyond it — so the sheets are
registered and 1 px of tolerance is exactly the right amount. For the
cross-producer pair **there is no minimum at all**: the surface is flat at 14–26
regions everywhere in the probe. That residue is not a shifted page and no
alignment will remove it.

What it is: the later of those two draws its text with CID TrueType fonts where
the earlier uses subset Type1C, so the glyph shapes differ slightly everywhere
there is text.
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
only the words can say that a run of serial nets was renumbered.

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

## When a sheet is not comparable at all

Two sample sets added later found a failure that the first ones could not.

One set was **reissued at a different paper size** — the same drawing on A3 that
the previous revision had on A4. The comparison lays both sides out to the first
document's geometry and crops the second, so an A3 sheet is measured against its
own top-left corner. Nothing about the result means anything.

What made it dangerous is what it reported. On sheet 1 there were 18216
unmatched ink pixels on one side and 45693 on the other, and the scan came back
with **7 regions** — which reads as a nearly unchanged sheet. The cause is not
the matching but the clustering: it bridges neighbouring cells on purpose, so
that one edit is one destination rather than forty, and a sheet that differs
everywhere therefore collapses into a handful of very large regions.

Two things follow, and both are now reported.

**A paper-size mismatch is stated, not silently cropped.** The status line, the
sheet's own section of the report, and the head of the report all say it, and the
report says what to do about it. A count that cannot be trusted has to be labelled
as such before anybody reads it.

**More tolerance can mean more regions.** Counter-intuitive, and measured: on one
sheet the count went 31 → 47 as the tolerance went 0 → 1, while the unmatched ink
halved from 18434 pixels to 9137. Tolerance removes the fringe that *connects*
genuine differences to each other, and the clustering bridges neighbouring cells,
so taking the fringe away splits one large blob into several. The count and the
unmatched ink are a pair of numbers to read together. This is pinned by a test so
that nobody corrects it.

**A count of regions is not a measure of how much changed.** Each sheet now also
carries the fraction of itself that the regions cover. Six edits cover a few per
cent; the mismatched sheets cover 100%. On the other new set — a native ECAD
export rather than a print-to-PDF — the sheets came back spread across every
band, which is the difference between a revision and a redraw, and no count of
regions could have said it: each of those sheets reports 33 to 56 regions
whatever its coverage.

That set is also the evidence that the comparison holds outside the print-to-PDF
path it was built against. Every document in the corpus, compared against itself,
reports exactly zero regions on every sheet — including the native ECAD export
and a file whose cross-reference table other PDF readers complain about.

### Alignment and scaling are one problem

The first draft of this section called a paper-size mismatch "a different problem
with a different answer" from alignment. That is wrong, and the framing is what
let the case be filed as a new category instead of as a transform that is not the
identity. Translation and scale are both the same question — **what transform
maps sheet B onto sheet A** — and one estimator covers them.

Measured, to check that rather than assert it. Rendering B at the scale that
makes its sheet the same size as A's:

| Pair | estimated scale | regions before | regions after |
| --- | --- | --- | --- |
| same size on paper | 1.0000 × 1.0000 | 37 | 37 |
| A4 against A3 | 0.7070 × 0.7067 | 7 | 2 |

The control is the important row: a same-size pair estimates exactly the identity
and nothing changes. One mechanism, and it is a no-op when there is nothing to
correct — which is the argument for having one rather than two.

The second row is the one that teaches something. 0.7070 is 1/√2 to four figures,
so the estimate is geometrically right, and yet the comparison is no better
afterwards: B still lays down two and a half times A's ink. Rendered at a common
size the two sheets are visibly the **same circuit** — the same blocks in the same
arrangement — but re-laid out, with blocks moved, notes added and the title block
reworded. The sheet was redrawn at a new size, not reissued at one.

### Estimate it from the ink, and score it on the ink

Two proxies for the transform look obvious and both give confident wrong answers.

**Page size** corrects the sheet and leaves the content where it was. On the
rescaled pair the page ratio is 1.4142 and the content ratio, measured from the
text, is 1.54: the drawing was enlarged relative to its own frame as well as the
paper.

**Text anchors** look far better and are worse. Words whose text occurs once on
each sheet are reliable correspondences, and a least-squares fit of a scale and
an offset per axis over 59 of them, on the cross-producer pair, returns:

    scale 1.0000 x 1.0016, offset -0.10 x -2.64 pt, residual median 0.16 pt

A tight residual, from many anchors, on a pair known to have come out of two
different PDF producers. It is wrong. Applied to that sheet:

| | regions | unmatched ink |
| --- | --- | --- |
| as rendered | 1 | 7 px |
| shifted by the estimate | 17 | 44518 px |
| shifted by a deliberately wrong amount | 16 | 48585 px |

The estimate is no better than a wrong answer, because the sheets were already
registered to seven pixels. What the fit measured is not where the ink is: it is
where MuPDF puts a word's bounding box, and two producers writing CID TrueType
and subset Type1C put it at systematically different heights. The text moved by
2.64 pt; the drawing did not.

This is the same mistake as the phantom 2% rescale earlier, in a more convincing
disguise — a confident number from a signal that is not the one being corrected.

So the rule is not "estimate from the ink" but the stronger **estimate from the
ink and score on the ink**. A candidate transform is worth applying only when it
measurably reduces the unmatched ink that the comparison itself counts, and that
is cheap to check for any candidate from any source. The search the fork designed
— projection profiles for a first guess, then a local search — is the right
shape, provided the thing it maximises is the comparison's own metric and not a
correlation of something else.

`crates/sc-session/examples/applyfit.rs` is that check, and running it before
believing any estimate is the whole lesson.

**No transform rescues a redraw.** For this pair the honest answer is still that
the sheets are not comparable, which is what the size warning and the coverage
figure now say. Estimating a transform would not have changed that, and would
have hidden it behind a plausible-looking number.

So alignment and scaling collapse into one future item rather than two, and the
reporting is the part that had to exist first.

## Refusing a file, and saying why

Three things had to be refused that MuPDF will happily accept.

A **badly damaged file** opens: MuPDF rebuilds what it can and hands back zero
pages. Without a check that reads as a comparison of two empty documents — a
blank window and no explanation — which is squarely in this project's worst
category, wrong output rather than an obvious failure.

A **password-protected** drawing opens too, and renders nothing. Drawings arrive
from outside often enough that this is worth naming rather than leaving as an
empty overlay.

And **every message names its file**. The shell opens two documents and reports
one failure; "cannot read the file" leaves the reader to work out which. Nor does
a MuPDF error number belong in front of a person — `code: 7, message: no objects
found` was what "this is not a PDF" used to look like.

## Testing without the drawings

The real sets carry most of this project's evidence and none of them can be
committed. Left there, a clone would build and pass and test almost nothing above
the pixel kernel.

The drawings are also not *named*: the repository is public, and a customer's
board codes and net names are as much theirs as the files. The tests address the
sample sets by the role each plays and read the filenames — and the numbers each
should produce — from `samples/sets.json`, which is ignored along with the
drawings. `check.sh` fails if any word from a sample filename appears in a
tracked file, taking those words from the directory rather than from a list that
would itself be the leak.

`sc-fixture` writes minimal PDFs by hand — a frame, a title block carrying the
revision, a few labels — and both the Rust integration tests and the window test
compare a pair of them. The window test's fixtures are written into the build
tree by CMake, so the sidebar, the sweep, the text panel, printing and the
settings are all exercised on a machine with no customer material at all. Where
the real sets *are* present they get a second pass, at 21 sheets of dense
schematic rather than 6 of fixture.

The fixture writer assembles the cross-reference table with real byte offsets
rather than letting MuPDF repair it. A fixture quietly rebuilt on every open is
not testing what it appears to be testing.

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
something different — one sheet of the sample set goes from 770 rows to 62 once moves are counted
rather than listed.

One more trap, met and fixed: a sheet with 47 labels reading the same resistor
value will happily
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

## What a reader can reach

Two things were settled in the core long before they were reachable from the
window, and both had to be fixed rather than explained.

The **overlay colours** were configurable, persisted and honoured, and could be
changed only by editing the settings file. The default pair is red and green.
Every piece of information this tool produces is carried by those two colours, so
a reader with red-green colour blindness could not use it at all. There is now a
dialog with a blue-and-orange preset, which stays distinct under every common
form of it.

The **changed-sheet list** showed a count and nothing else, after the count had
already been shown to be unable to separate a few edits from a redrawn sheet. It
now carries the coverage next to the count, and a sheet whose two revisions are
different sizes on paper says so instead of showing a figure that means nothing.

The **text panel** listed moved text alongside real changes, which is what the
report had already stopped doing — one sample sheet has 354 moves against 10
changes. Moves are now counted on a control rather than listed, and the count is
on show, so nothing is hidden without saying so.

**Excluding a region was reachable only by Ctrl+drag**, and the first person
handed the built application could not find it. Nothing in any menu said the
feature existed; the only two entries about excluded regions were for accepting
the suggested ones and for clearing them all. The drag stays, because anyone who
knows it will keep using it, but there is now an entry that arms the next drag,
a toolbar button beside it, and the same pair on a right-click — and while it is
armed the status line says what to do and that Escape cancels.

The same report said the keys did nothing, and the reason was next to it:
**everything that needs a comparison did nothing, silently, when none was open**.
`1`, `2`, `3` and `Tab` all went through a `if (!m_session) return;` and left no
trace. The window now switches those actions off until a pair is open, and the
empty viewport says what to open and how, rather than showing the same flat grey
a comparison that found nothing would show. A control that is off reads as *not
yet*; a control that is on and does nothing reads as broken.

Everything reachable by a key is now also reachable by a button, and each button
names its key in its tooltip, so the toolbar teaches the keyboard rather than
replacing it.

And then, with the buttons finally on show, the first person to press one found
that **the view modes had never redrawn anything**. The mode was set on the
session, the tile cache was emptied, the status line read "A only" — and the
viewport went on drawing the overlay, because which document each sheet shows is
settled in `relayout()` and nothing rebuilt the layout. The pairing shift was
broken the same way and for the same reason, and it changes the page count as
well. `invalidate()` now rebuilds the layout, which is what "everything drawn is
stale" always meant.

Two things about that row of buttons were settled by using it. **`Tab` blinks
between the two revisions and never shows the overlay.** It had remembered which
view it started from and returned there, so the same key did different things
depending on history — but the fix is not to cycle all three. A blink comparator
works because the two pictures are the same drawing at the same place and only
what changed moves; the overlay differs from both of them everywhere there is
colour, so putting it between them breaks the effect the key exists for. `Tab`
is A, B, A, B, entering at A from anywhere else, and the overlay has a keystroke
of its own. **Side by side is the fourth member of the same exclusive choice**,
not a separate toggle beside it. It is a layout rather than a view mode, and the code
keeps that distinction, but from the reader's chair it is the fourth answer to
"what am I looking at" — and a checkable pair that can both be on says the window
is showing two things at once.

The reason it shipped is worth more than the fix. The window test drove the
action and then asserted on **the words the window says about itself**:

```cpp
win.findChild<QAction *>("onlyA")->trigger();
check(status->text().contains("A only"), "A only");
```

Both halves of that were true while the picture was wrong. The test now grabs
the viewport in each mode and asserts the three differ, and the same for the
pairing shift; putting the bug back makes all four fail. **For anything the
reader looks at, assert on what was drawn.**

## Finding the change on a sheet that is mostly unchanged

Everything above is about not reporting differences that are not there. This is
the other half of the same job: on a real sheet the change is a handful of
pixels and the sheet is not, and four controls came out of trying to find one by
eye.

**The tolerance ceiling moved from 3 to 8.** The old cap's reasoning was that
more slack than 3 stops distinguishing a stroke that moved from one that was
deleted, which is true and is why the default is still 1. What it missed is that
the tolerance is in *device* pixels, so it shrinks against the drawing as the
reader zooms in: a producer's fringe is about a pixel wide at 100% and about
three at 300%, and 300% is exactly where a reviewer goes to read a component
value. The ceiling ran out precisely when it was needed.

The cost was measured first. A 512-pixel tile of a real sheet costs 8.7 ms at
tolerance 1 and 11.0 ms at 8, so a viewport of a dozen goes from 105 ms to
135 ms; the dilation is sub-linear in its radius because the pass is memory
bound, and MuPDF's two renders dominate either way.

The old reasoning survives as a **line rather than a ceiling**. `MAX_TOLERANCE`
is 8, `TOLERANCE_HIDES_MOVEMENT` is 3, and above the second the status line, the
print caption and the exported report each say that a stroke which only moved is
no longer reported. Both numbers cross the ABI — `sc_max_tolerance()` and
`sc_tolerance_hides_movement()` — rather than being written into the frontend,
because a spin box that stops one short of what the core allows is a control
that lies, and this ceiling has moved once already.

Two things about the tolerance were broken and are worth recording. Nudging past
the ceiling **threw away a finished sweep of 85 sheets** to arrive at the number
it was already at, because setting it drops every scanned answer and nothing
noticed that the value had not changed. And changing it at all left the sidebar
empty until the reader found "Scan Every Sheet" for themselves — `sweep.rs` had
said since it was written that "changing the tolerance mid-sweep restarts it",
and the frontend had never honoured it. It now restarts, 400 ms after the last
change so that clicking the spin box four times starts one sweep rather than
four. That timer is not the idle timer the ground rules forbid: it fires once,
because of something the reader did, and never polls.

**Fading the drawing the two revisions agree on.** The composition rule splits
each pixel into `shared = min(a, b)`, drawn neutral black, and the leftover on
each side, drawn in that side's colour. `Options::shared_ink` scales the first
of those and nothing else, from 100 to 0. At 0 the sheet is blank except for
exactly what changed — the same trick as turning the room lights down to see one
LED, and the reason it is a slider and not a switch is that stopping halfway
keeps enough of the drawing to say *where* on the sheet the speck is.

Measured on a real sheet at 150 dpi, the drawing covers 2.65% of the paper and
the changes 0.13% of it. At 50 the artwork is still all there at half strength —
136,778 marks at a mean of 65 out of 255 rather than 120 — and at 0 there are
1,326 neutral pixels left on the whole sheet against 2,949 coloured ones. The
few that survive are places where *both* revisions have unmatched ink at the
same pixel, which composes dark: they are differences, not artwork.

Three things follow from "and nothing else". It is not in the scan: what changed
does not depend on how it is painted, and a reader who fades the drawing away
must still be told the same number of sheets changed. It does not touch the two
leftovers: fading the differences would be fading the answer. And only the
overlay has anything to fade — a single revision is meant to look exactly like
opening that file — so the control is switched off in every other view rather
than left live and inert, which is what "clicking Only A does nothing" turned
out to mean.

**One sheet at a time means one whole sheet and no scrolling.** It was built
first as "the scroll stops at the foot of this sheet", which is what most
document readers do and is not what was asked for or what the view is worth
having for: a set is flipped through to find the sheet that changed, and
scrolling is the thing in the way. The sheet is fitted to the viewport, both
scrollbars are taken away rather than left at an empty range, and `PageUp`,
`PageDown`, the arrow keys, `Home`, `End` and the wheel all move by whole
sheets. Wheel notches are accumulated to 120 before one counts, or a touchpad
sends an 85-sheet set past in a flick.

Because "the whole sheet is on screen" is the entire definition, **a zoom leaves
the flow** — visibly, with the button unchecking itself — rather than quietly
turning it into a single sheet that has to be scrolled around. Fitting the width
leaves it too: a portrait sheet fitted to the width of a wide window is taller
than the viewport, which is scrolling by another name.

It is a flow, not a view mode. A/B/overlay/side-by-side are one exclusive choice
because they answer "what am I looking at"; whether the viewport scrolls through
the set is a different question, and a reader wants one sheet of the overlay as
readily as one sheet side by side. Putting it in that group would make choosing a
single sheet switch the overlay off, which is the trap the group was made to
close. It broke `showRect` on the way in — stepping to a change crosses sheets,
and the sheet it was pointing at had not been laid out — which is the same class
of bug as the layout that never rebuilt, and is tested the same way.

**The toolbar has pictures now**, and the reason it did not is answered rather
than forgotten. It said: there is no icon set that says "only the earlier
revision", and a toolbar of guesses is worse than no toolbar. Both halves still
hold of every icon theme there is — so these are drawn, in `Icons.cpp`, from the
thing they stand for. The overlay button is a picture of the composition rule:
A's colour, B's colour, and black where they overlap. The two revision buttons
are sheets carrying the letter the rest of the window calls them by, in **the
reader's own two overlay colours**, so a reviewer who moved them to blue and
orange because red and green are the same colour to them is not left looking at
a red button and a green one. Painted rather than themed for a plainer reason as
well: there is no icon theme inside the AppImage and none on Windows, and a
themed icon is a blank square on two of the three targets this ships to. The
words stay next to the pictures.

## Windows

The core cross-compiles to `x86_64-pc-windows-gnu` with the MinGW toolchain, and
the resulting DLL loads and passes the whole ABI harness under Wine. MuPDF needed
no special handling, which is worth writing down because it was the one thing
about this plan that looked likely to go badly: the fork it grew out of builds
MuPDF through its own makefiles precisely to avoid that fight, and none of that
turned out to be necessary.

One difference between the two Windows routes is worth knowing before choosing.
`mupdf-sys` builds MuPDF with GNU make everywhere except MSVC, and only that path
maps cargo features onto the build. On MSVC it builds MuPDF's own Visual Studio
solution instead and ignores the features entirely, so the careful list of
formats this project does *not* want — epub, html, cbz, img, svg, the JavaScript
engine — has no effect there. The first Windows CI run linked Tesseract OCR into
the binary, which is how this came to light. A MinGW build honours the list; an
MSVC build does not.

Qt is not a problem so much as a choice of route, and it is worth being precise
about which, because it is easy to state the awkward case as though it were the
only one.

*Building on Windows* needs nothing special: the official Qt for Windows in
either flavour the installer offers, and the same one-command build.
`shell/CMakeLists.txt` already covers it — the MinGW triple is set only under
`CMAKE_CROSSCOMPILING`, so a native build lets cargo take the host toolchain,
and the MSVC and MinGW import-library names are both handled. `mupdf-sys` builds
MuPDF through MSBuild under MSVC.

*Cross-compiling the shell from Linux* is the awkward case, and only because Qt's
own tools have to run on the build machine: it wants a MinGW Qt 6 with native
`moc`, `rcc` and `uic`, which Ubuntu does not package and Fedora does — the route
the sibling Sterna project takes.

Neither has been run here; this machine has no Windows and no MinGW Qt. What is
verified is the core.

One thing that makes the choice less fraught than it looks: the seam is a flat C
ABI whose returns are all borrowed, so no allocation crosses it and there is no
allocator to mismatch. Matching the two halves' toolchains is still the sensible
default, but the design does not punish getting it wrong.

## The AppImage builds its own Qt

The Linux artefact bundles a Qt built here rather than the distribution's, for
two reasons that were each found the hard way.

**The floor.** Qt's own Linux binaries, and Ubuntu 24.04's packages, want glibc
2.39. An AppImage built against either runs on Ubuntu 24.04 and little else,
which is not what the word portable is for. Built on the maintained
`manylinux_2_28` image — AlmaLinux 8, pinned by digest — everything in the image
runs on glibc 2.28 and up. The packaging script reads back every versioned
symbol the packaged binaries import and refuses to finish if a new dependency
has raised the floor, because that is exactly the kind of regression that
otherwise arrives as a bug report from the one person with an older machine.

**The title bar.** GNOME advertises no `zxdg_decoration_manager_v1`, so on a
GNOME desktop the title bar is drawn by Qt, by whichever decoration plugin is
installed. Qt Base ships only `bradient`, which handles two gestures — a click
on a button and a drag to move — and has no clock in it, so it cannot recognise
a double click. The `adwaita` decoration matches the desktop's own and toggles
maximised on a double click; it lives in Qt Wayland, and its feature switches
itself off unless Qt Svg is already installed, so the build order is
load-bearing and its absence is invisible.

Both of those, and the shape of the scripts, come from `../Sterna`, which had
already paid for them.

Two things this replaced are worth recording as dead. The image used to be
built against the development machine's Qt, and **plugins were copied in by
hand and their run paths patched**; a copy whose run path still pointed at the
build machine loaded there and nowhere else, and the only symptom was a feature
quietly missing — twice, once as no window decoration. The bundled libraries are
now byte for byte the build's own output, with no run path rewritten at all, and
`AppRun` supplies the search path through `LD_LIBRARY_PATH` instead. That also
keeps Qt's LGPL substitution seam open, which a rewritten run path does not.

## What is next

Ranked for the schematic-review workflow.

1. **Windows packaging.** The core cross-builds and passes its harness under
   Wine. The shell has never been built on Windows here, and CI now attempts it
   on a Windows runner with the official Qt — until that job has been seen green,
   "the CMake handles it" is still an assertion. After that, an installer.
2. **Estimate the transform between two sheets** — one item, not two. Alignment
   and scaling are the same question, and one estimator covers both and is the
   identity when nothing needs correcting.

   Estimate it from the ink and score it on the ink: two proxies have now each
   produced a confident wrong answer, and the section above has the numbers. A
   candidate is worth applying only when it reduces the unmatched ink the
   comparison counts.

   It stays unbuilt because no pair in the corpus needs it. Every same-size pair
   is already registered — the closest thing to a counter-example scores 1 region
   and 7 unmatched pixels as it stands. Applying a scale would resample every
   line on the sheet and make its own fringe, so it wants measuring before it is
   switched on, in stages: report the estimate first, apply whole-pixel
   translation next, and only then consider scale.
