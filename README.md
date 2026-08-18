# sch-pdf-compare

This software compares two revisions of an electronics schematic in PDF format
and shows the differences.

A general PDF tool is not sufficient for this task. A drawing set contains 21 to
85 sheets of A4. A resistor value that changed is only a small number of pixels.
The two revisions usually come from different PDF producers. All the sheets
contain the same title block, and the date in it changes with each revision.

In the overlay, the content that is only in the previous revision is red. The
content that is only in the new revision is green. The content in the two
revisions is black, and the paper is white. Thus you can quickly find a net that
has a new route.

This file does not contain an example image. All the drawings for the tests are
customer material. The `samples/` directory stays out of the repository.

## What the software does

- **An overlay with a tolerance in device pixels.** Two PDF producers can put
  the same line a fraction of a pixel apart. The tolerance prevents a full sheet
  of colour. The values are 0 to 8, and the toolbar has a control for them. A
  large zoom makes the difference between the two producers wider on the screen.
  Thus a larger value is necessary at a large zoom. Above 3 pixels, the software
  does not report a line that only moved. The status line, the printed page and
  the report give this warning.
- **A view of one revision.** The `1`, `2` and `3` keys show revision A,
  revision B, and the overlay. The `Tab` key changes between the two revisions.
  The zoom and the position on the sheet do not move.
- **Movement between the changes.** `Ctrl+.` goes to the next change and
  `Ctrl+,` goes to the previous change. The movement continues on to the other
  sheets and does not change the zoom.
- **A list of the sheets that changed**, with a count for each sheet and how
  much of the sheet the changes cover. A count alone cannot show the difference
  between some small changes and a sheet with a new drawing on it. A sheet with a
  different paper size in the two revisions says that instead of a count.
  You can select a sheet in the list to go to it.
- **Regions that the software does not compare.** Hold `Ctrl` and move the
  pointer to make a rectangle around the title block. The software then ignores
  that region on all the sheets. It shows the artwork in the region with less
  ink, inside a dashed border, and gives a count in the status line. Thus a
  region that the software did not compare does not look like a region with no
  changes. The software scans all the sheets again when you add a region or
  remove a region, because each result was for a different comparison.
- **Two methods to pair the sheets.** `Alt+Shift+←/→` moves the pairing by one
  sheet. The **Match Sheets by Content** command finds the pairs from the text on
  the sheets. The software identifies a sheet that has no pair as an added sheet
  or a removed sheet.
- **A panel that shows the text differences**, for example `10k → 12k`. This is
  necessary when the two revisions come from different PDF producers. Text that
  only moved is counted, not listed, because a sheet with a new layout moves
  hundreds of labels and those rows conceal the few that read differently.
- **A control that changes the content in the two revisions from black to
  white.** The overlay shows this content in black. The **Fade** button and the
  `F` key change it in four steps. The slider gives the values between the two
  ends. The differences keep their colours. At the white end, the sheet shows
  only the changes. Use this control when a sheet has much content and the
  changes are small.
- **A choice of the two overlay colours.** The usual pair is red and green. A
  reader with red-green colour blindness can select blue and orange, because all
  the information in the overlay is in these two colours.
- **A background scan** of all the sheets when the software opens the two files.
  Thus you do not wait to know which sheets changed.
- **A side-by-side view** with the `4` key. The two revisions have one zoom and
  one position. This helps you where the overlay puts two different texts on top
  of each other.
- **A view of one full sheet.** The `5` key shows one sheet at the size of the
  window. There is no scroll. The mouse wheel, `PageUp` and `PageDown` move to
  the previous sheet and to the next sheet. `Home` and `End` go to the first
  sheet and to the last sheet. A zoom operation stops this view, because the
  sheet is then larger than the window. This view operates with the overlay,
  with one revision, and with the side-by-side view.
- **Movement on a sheet that is larger than the window.** Hold `Shift` and turn
  the wheel to move the sheet to the left and to the right.
- **A print function** in the same orientation as the drawing. Each page gives
  the two file names, the view, the tolerance, and the regions that the software
  did not compare.
- **A change report** from **File ▸ Export Change Report**. The report is a
  Markdown file with one section for each sheet and a table of the text changes.
- **A record of the regions to ignore, for each pair of files.** Thus you find
  the title block one time only.
- **Detection of the regions that change on all the sheets.** The software gives
  these regions to you but does not apply them. A net name that changed on all
  the sheets looks the same, and the software must not remove it from the view.

## Two revisions from different PDF producers

This condition controlled the design. The table gives the results for one sheet
of a drawing set. The software compared this sheet with the revision two steps
after it, which came from a different PDF producer.

| | regions found |
| --- | --- |
| pixels, no tolerance | 29 |
| pixels, 1 px tolerance | 26 |
| **text** | **2** |

The two text changes are the revision letter and the date. The other 24 regions
are only differences in the shapes of the characters. One file has CID TrueType
fonts, and the other file has subset Type1C fonts.

The software examined all the offsets in a range of 3 pixels in each direction.
The number of regions did not decrease to less than 14, and there was no
minimum. Thus the two sheets are aligned correctly, and an alignment procedure
cannot correct this condition.

When the software compares the text, this problem does not occur for the part of
the schematic that is text. The overlay also finds a wire that has a new route
and no text. The two panels give different information, thus the software shows
the two panels together.

## How to build the software

Qt 6.4 or a newer version, a Rust toolchain, CMake and Ninja are necessary.
Cargo makes MuPDF from its source code, thus the first build takes some minutes.

```sh
cmake -S shell -B shell/build -G Ninja
cmake --build shell/build
./shell/build/sch-pdf-compare earlier.pdf later.pdf
```

CMake controls Cargo, thus these commands make all the parts.

The same commands operate on Windows with the official Qt for Windows. The Qt
installer gives two versions, and this project can use each one. MinGW is not
necessary for this project.

`packaging/windows/build-core.sh` also makes the core for Windows on a Linux
machine and does a test of it with Wine. Only a cross-build of the shell on a
Linux machine must have a MinGW Qt 6, because the Qt tools operate on the build
machine.

## Layout

A Rust core does all the operations. `shell/` is a Qt 6 Widgets frontend that
shows what the core gives it. The two parts connect at one flat C ABI, which
cbindgen makes. `AGENTS.md` gives the rules for this connection. `docs/design.md`
gives the decisions and the errors that this project made before.

## Licence

AGPL-3.0-or-later, because the software includes MuPDF. Qt is LGPLv3 and has a
dynamic link.
