# packaging/windows

Two things live here: `build-core.sh`, which cross-builds the Rust core and
runs its ABI harness under Wine, and everything else, which builds the
**Windows installer**. Both run on Linux and produce Windows binaries.

```sh
./packaging/windows/installer.sh            # → packaging/windows/build/…setup.exe
./packaging/windows/installer.sh --verify   # …then install it under Wine and
                                            #   take it apart again
```

That is the whole thing: a pinned Fedora container, the dependencies, the
cross-build, the staging and NSIS. Inside a Fedora that already has the tools,
`./build.sh` and `./verify.sh` do the two halves on their own.

Measured 2026-08-18: a **24 MB installer** puts **79 MB across 33 files** on
disk. Before `--strip-unneeded` the staged tree is 113 MB, because Fedora ships
its MinGW packages unstripped.

## Why a container, and why Fedora

Cross-building the shell needs a **MinGW Qt 6 whose `moc`, `rcc` and `uic` run
on the build machine**. Fedora packages one (`mingw64-qt6-qtbase`) and Ubuntu
does not, and that single fact is why this build happens in a container rather
than wherever the developer happens to be standing. Everything else — the
MinGW toolchain, NSIS's compiler, Wine, even the Rust standard library for
`x86_64-pc-windows-gnu` — is a package away on any distribution.

## Why NSIS

**Its compiler is a Linux binary.** `makensis` comes from Fedora's
`mingw-nsis-base`, so the release artefact is produced entirely by native tools
and Wine is nowhere near the path that makes it. Wine is used afterwards, by
`verify.sh`, to answer one narrow question — did every DLL resolve — which is
the question deployment actually fails on.

## What goes in

| | |
|---|---|
| `sch-pdf-compare.exe`, `schcompare.dll` | the shell and the Rust core |
| `platforms\`, `styles\` | the Qt plugins; see below |
| ~25 DLLs | Qt, what Qt needs, and the MinGW runtime |
| `doc\` | the licences, and `BUILD-INFO.txt` naming the exact Qt |

**The DLL set is closed out of the import tables, not from a list.** Qt's own
deployment tool is not available for this target — `windeployqt` is a Windows
program — so `build.sh` walks `objdump -p` output to a fixed point. The rule
for *ours to ship* against *Windows'* is whether the MinGW sysroot has the
file: that tree holds only what the cross toolchain provides, and none of
`kernel32`, `msvcrt`, `shell32`, `user32`, `advapi32` or `ole32` is among them.
Checked rather than assumed, because shipping a private copy of a system DLL is
worse than shipping none.

`platforms\qwindows.dll` is not optional: Qt with no platform plugin prints
"This application failed to start because no Qt platform plugin could be
initialized" and exits. `qoffscreen.dll` is in there because that is how the
shell's own tests run, and how `verify.sh` starts the program without a
desktop. `styles\` is optional in the sense that the window still opens without
it, wearing the Fusion look on a desktop where everything else is native —
which a user sees and a test does not.

Printing needs no plugin of its own: Qt 6 keeps the Windows print support
inside `Qt6PrintSupport.dll`, and the sysroot has no `printsupport` plugin
directory at all. Checked, because printing is not optional in this program and
a missing plugin would be invisible until somebody printed.

## Traps

- **`makensis` loads its default x86 stub before it reads the script**, so
  `mingw32-nsis` has to be installed even though this installer is amd64 and
  uses none of it. Without it the build stops at "error setting default stub"
  having never opened the `.nsi`.
- **`wine-core` alone is not enough to run an installer.** A silent install
  returns 0 and puts nothing on disk, which is worse than an error — the first
  `verify.sh` run to catch it reported every file missing. `installer.sh
  --verify` installs the whole `wine` package.
- **The fixture generator must be built for the host.** It is a tool run during
  the build, not part of the product; built for Windows from Linux, the first
  cross build stopped at `cannot execute binary file`. What it produces is
  portable, so what produces it need not be.
- **The finish page must not start the program itself.** The installer asks for
  administrator rights, so anything it runs inherits them — and this program
  keeps its settings, including the excluded regions worked out for each pair
  of drawings, under the *running user's* AppData. A first run as Administrator
  writes them into the administrator's profile and the reader's own later runs
  start from defaults, permanently and with nothing to see. `StartApp` goes
  through `explorer.exe`, which is already running as the user.
- **An upgrade in place leaves the previous version's files behind, and for a
  Qt DLL that is not inert.** The loader finds the stale one first and the
  program dies before `main` with a missing entry-point box naming a symbol
  nobody has heard of. `.onInit` runs the old uninstaller first; `_?=` is what
  keeps it in place long enough to be waited on.
- **`RMDir /r "$INSTDIR"` is a recursive delete of a path the user typed into
  the directory page.** So `build.sh` generates the uninstall list from the
  staging tree: every file by name, every directory with a plain `RMDir`, which
  refuses a directory that is not empty. `verify.sh` leaves a file in the
  program folder and checks it survives.
- **The licence page is a RichEdit control** and renders LF-only text as one
  unreadable line, so every text file a person reads gets CRLF on the way in.
- **A failed check must not pass.** The first `verify.sh` could not `cd` into
  an installation that was never made, ran the program from the staging tree
  instead, and reported that it started. It now stops when there is nothing
  installed.

## What running the tests under Wine did and did not settle

The shell's own binaries cross-build too, so `view_test.exe` — the window
driven the way a reader drives it — can be run against the same DLL set the
installer ships. It needs `Qt6Test.dll` copied in beside it, which the
installer has no reason to carry.

It gets through everything except the last three assertions, which are the ones
about **saving settings**: after a normal window closes, `settings.json` is not
there, and the directory it lives in was never made. Everything else — opening
the pair, the sweep, the views, the tolerance, the fade, one sheet at a time,
excluding a region, printing to a PDF and reading it back — behaves as it does
on Linux.

That failure is not evidence of a defect on Windows. The CI Windows job runs
this same test natively, with MSVC and the official Qt, and is green, and the
assertions are not guarded by anything platform-specific. What is left
unresolved is whether Wine is the difference or the MinGW build is — the
installer ships the MinGW one, so it is worth knowing, and it is not known yet.
Until it is, this run is a smoke test and not a gate: `verify.sh` checks what
Wine answers reliably, which is whether the program starts at all.

## The stub is amd64, which is not the convention

An x86 stub runs on any Windows and is what nearly every installer uses,
including for 64-bit programs. It costs two things here and buys nothing: a
32-bit process writing `HKLM\Software` lands in `Wow6432Node` unless every
write is wrapped in `SetRegView 64`, and the only Wine here is 64-bit with no
WOW64, so an x86 stub could not be started before it shipped. A release
artefact that cannot be run before release is the wrong trade for supporting a
32-bit Windows that could not run the 64-bit program inside it either.

## What the installer deliberately does not do

- **No file association.** This program opens two PDFs and compares them; it is
  not a PDF viewer. Offering it for `.pdf` would put it in the Open with list
  for every drawing, datasheet and invoice on the machine, and two revisions
  cannot be named by double-clicking one file anyway.
- **Nothing on `PATH`.** Editing the system `PATH` from NSIS is the classic way
  to truncate somebody's: the naive `ReadRegStr` into a 1024-byte buffer
  silently loses everything past it, and NSIS's own documentation says so.
- **It does not touch the settings.** They are under the user's AppData, one
  per user on a machine that may have several, and they hold the excluded
  regions for every pair of drawings the reader has opened.

## Not done yet

**Code signing.** The installer is unsigned, so SmartScreen warns on first run
and the UAC prompt says "Unknown publisher". That needs a certificate, which
needs a legal entity. `osslsigncode` runs on Linux, so the build does not have
to move when there is something to sign with.
