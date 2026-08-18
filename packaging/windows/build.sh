#!/usr/bin/env bash
# Build the Windows installer. This runs on Linux and produces a Windows
# program: the shell is cross-compiled with `mingw64-cmake`, and NSIS is
# assembled by a native `makensis`.
#
#   ./build.sh              build it into build/
#   ./build.sh --clean      throw the build tree away first
#   ./build.sh --stage      stop after staging, and do not run makensis
#
# It needs a Fedora with the MinGW toolchain, `mingw64-qt6-qtbase` and
# `mingw32-nsis`. `./installer.sh` runs it in a pinned container that has them,
# which is the way to build it on a machine that does not.
#
# NSIS rather than the other installer builders for one reason: its compiler is
# a Linux binary, so the release artefact is produced by native tools and Wine
# is nowhere near the path that makes it. Wine is used *after* the fact, by
# `verify.sh`, to answer the one question deployment actually fails on.
#
# The recipe is Sterna's, which had already paid for it.
#
# What the licence asks of this script is what it asks of the AppImage: Qt is
# bundled rather than depended on, so it is never static, it stays as separate
# DLLs a user can substitute, and the LGPL text and the offer of source are
# installed with it. See QT-LGPL-NOTICE.md.
set -uo pipefail

cd "$(dirname "$0")"
here=$PWD
root=$(cd ../.. && pwd)

CLEAN=0
STAGE_ONLY=0
for a in "$@"; do
    case "$a" in
        --clean) CLEAN=1 ;;
        --stage) STAGE_ONLY=1 ;;
        -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "windows: unknown option '$a'" >&2; exit 2 ;;
    esac
done

build=$here/build
stage=$build/stage

[ "$CLEAN" = 1 ] && rm -rf "$build"
mkdir -p "$build"

need() {
    command -v "$1" >/dev/null || { echo "windows: $1 not found — $2" >&2; exit 2; }
}
need cargo "dnf install cargo rust-std-static-x86_64-pc-windows-gnu"
need mingw64-cmake "dnf install mingw64-qt6-qtbase mingw64-gcc-c++ — and is this the container?"
need x86_64-w64-mingw32-objdump "dnf install mingw64-binutils"
[ "$STAGE_ONLY" = 1 ] || need makensis "dnf install mingw32-nsis"

objdump=x86_64-w64-mingw32-objdump
sysroot=$(x86_64-w64-mingw32-gcc -print-sysroot)/mingw
mingw_bin=$sysroot/bin
qt_plugins=$sysroot/lib/qt6/plugins
[ -d "$qt_plugins/platforms" ] || {
    echo "windows: no Qt plugins under $qt_plugins — install mingw64-qt6-qtbase" >&2
    exit 2
}

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' \
    "$root/crates/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "windows: no version in crates/Cargo.toml" >&2; exit 2; }

# --- build -------------------------------------------------------------------
#
# Release, which also builds the Rust core with --release: CMakeLists drives
# cargo and takes the profile from CMAKE_BUILD_TYPE. It supplies `--target
# x86_64-pc-windows-gnu` by itself for a cross build, and puts cargo's output
# inside this tree rather than in the workspace's own `crates/target`.
#
# The application target only. Building everything would build the test
# binaries as well, which is `verify.sh`'s business and not the installer's.
echo "windows: building" >&2
mingw64-cmake -S "$root/shell" -B "$build/cmake" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release >/dev/null || exit 2
cmake --build "$build/cmake" --target sch-pdf-compare || exit 2

# --- lay it out the way Windows expects --------------------------------------
#
# A Windows program folder is flat: the executable at the root, because that is
# where the loader looks for its DLLs and where Qt looks for its plugin
# directories.
rm -rf "$stage"
mkdir -p "$stage"
cp "$build/cmake/sch-pdf-compare.exe" "$build/cmake/schcompare.dll" "$stage/" || exit 2

# `platforms` is not optional: Qt with no platform plugin prints "This
# application failed to start because no Qt platform plugin could be
# initialized" and exits, which is the commonest way a deployed Qt program
# fails. `qoffscreen` is in there because that is how the shell's own tests run
# and how `verify.sh` starts the installed program without a desktop.
# `styles` is optional in the sense that the window still opens without it —
# wearing the Fusion look on a desktop where everything else is native, which a
# user sees and a test does not.
mkdir -p "$stage/platforms" "$stage/styles"
cp "$qt_plugins/platforms/qwindows.dll" "$stage/platforms/" || exit 2
cp "$qt_plugins/platforms/qoffscreen.dll" "$stage/platforms/" || exit 2
cp "$qt_plugins/platforms/qminimal.dll" "$stage/platforms/" || exit 2
cp "$qt_plugins/styles"/*.dll "$stage/styles/" 2>/dev/null

# Printing needs no plugin of its own here: Qt 6 keeps the Windows print
# support inside Qt6PrintSupport, and the sysroot has no `printsupport`
# plugin directory at all. Checked, because printing is not optional in this
# program and a missing plugin is invisible until somebody prints.

# --- the DLLs Windows does not have ------------------------------------------
#
# Closed transitively out of the import tables rather than taken from a list,
# which would be wrong the first time Qt changed a dependency. Qt's own
# deployment tool is not available for this target — `windeployqt` is a Windows
# program — so this walks `objdump -p` to a fixed point.
#
# The rule for "ours to ship" against "Windows'" is whether the MinGW sysroot
# has the file. That tree holds only what the cross toolchain provides, and
# none of kernel32, msvcrt, shell32, user32, advapi32 or ole32 is among them —
# checked, because shipping a private copy of a system DLL is worse than
# shipping none.
imports() {
    "$objdump" -p "$1" 2>/dev/null | sed -n 's/^[[:space:]]*DLL Name: \(.*\)$/\1/p'
}

echo "windows: closing the DLL set" >&2
copied=1
while [ "$copied" != 0 ]; do
    copied=0
    while IFS= read -r f; do
        while IFS= read -r dll; do
            dll=${dll%$'\r'}
            [ -n "$dll" ] || continue
            [ -e "$stage/$dll" ] && continue
            [ -e "$mingw_bin/$dll" ] || continue
            cp "$mingw_bin/$dll" "$stage/$dll" || exit 2
            copied=$((copied + 1))
        done < <(imports "$f")
    done < <(find "$stage" \( -name '*.dll' -o -name '*.exe' \) | sort)
    [ "$copied" = 0 ] || echo "windows:   +$copied" >&2
done

# A plugin that cannot resolve its own imports is not reported as a missing
# DLL: Qt reports it as "no platform plugin", which is the same message an
# absent one gives, so the two failures cannot be told apart from the message.
for dll in Qt6Core.dll Qt6Gui.dll Qt6Widgets.dll Qt6PrintSupport.dll; do
    [ -e "$stage/$dll" ] || { echo "windows: $dll was not resolved" >&2; exit 2; }
done

# Fedora ships its MinGW packages unstripped, and so is everything cargo and
# this CMake tree produce. Safe on a PE file: the export table a DLL is loaded
# through is part of the image, not part of the symbol table, which is why
# `--strip-unneeded` can take the latter without touching the former.
before=$(du -sh "$stage" | cut -f1)
echo "windows: stripping" >&2
find "$stage" \( -name '*.dll' -o -name '*.exe' \) -exec \
    x86_64-w64-mingw32-strip --strip-unneeded {} + || exit 2

# --- what the licences oblige ------------------------------------------------
docs=$stage/doc
mkdir -p "$docs"
cp "$root/LICENSE" "$docs/LICENSE.txt" || exit 2
cp "$here/QT-LGPL-NOTICE.md" "$here/../appimage/LGPL-3.0.txt" "$docs/" || exit 2

qt_version=$(basename "$(ls -d "$sysroot"/include/qt6/QtCore/[0-9]* 2>/dev/null | tail -1)" \
    2>/dev/null || echo unknown)
{
    echo "sch-pdf-compare for Windows"
    echo
    echo "built:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "version:    $version"
    echo "commit:     $(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "built on:   $(. /etc/os-release && echo "$PRETTY_NAME"), cross-compiled"
    echo "toolchain:  $(x86_64-w64-mingw32-gcc -dumpversion) (x86_64-w64-mingw32)"
    echo "Qt:         $qt_version, as packaged for MinGW"
    echo "Qt source:  https://download.qt.io/official_releases/qt/"
    echo
    echo "Qt is bundled as separate DLLs and may be substituted: replace the"
    echo "Qt6*.dll files in this directory. See QT-LGPL-NOTICE.md."
} > "$docs/BUILD-INFO.txt"

# The licence page is a RichEdit control and renders LF-only text as one
# unreadable line. Everything a person reads in the installer or in Notepad
# gets CRLF on the way in.
for f in "$docs"/*.txt "$docs"/*.md; do
    [ -e "$f" ] || continue
    sed -i 's/\r*$/\r/' "$f"
done

# --- the file lists the installer and the uninstaller use --------------------
#
# Generated rather than written, for the uninstaller's sake. The alternative is
# `RMDir /r "$INSTDIR"` on a directory the *user* typed into the directory
# page, which is how installers delete a Program Files. Everything here is
# removed by name, and each directory with a plain `RMDir`, which refuses a
# directory that is not empty — so anything a user left in the program folder
# survives the uninstall, and so does the folder holding it.
files=$build/files.nsh
uninstall=$build/uninstall.nsh
: > "$files"
: > "$uninstall.tmp"

# Shallowest first for the installer, which does not care; deepest first for
# the uninstaller, so a directory is emptied before RMDir reaches it.
dirs=$( (echo .; cd "$stage" && find . -mindepth 1 -type d -printf '%P\n') | sort )
for d in $dirs; do
    if [ "$d" = "." ]; then
        out='$INSTDIR'
        src=$stage
    else
        out='$INSTDIR\'$(echo "$d" | tr '/' '\\')
        src=$stage/$d
    fi
    printf 'SetOutPath "%s"\n' "$out" >> "$files"
    for f in "$src"/*; do
        [ -f "$f" ] || continue
        printf '  File "%s"\n' "$f" >> "$files"
        printf 'Delete "%s\\%s"\n' "$out" "$(basename "$f")" >> "$uninstall.tmp"
    done
done
tac "$uninstall.tmp" > "$uninstall"
rm -f "$uninstall.tmp"
for d in $(echo "$dirs" | tac); do
    [ "$d" = "." ] && continue
    printf 'RMDir "$INSTDIR\\%s"\n' "$(echo "$d" | tr '/' '\\')" >> "$uninstall"
done

n=$(grep -c '^  File ' "$files")
size=$(du -sh "$stage" | cut -f1)
echo "windows: staged $n files, $size (was $before before stripping)" >&2

if [ "$STAGE_ONLY" = 1 ]; then
    echo
    echo "windows: $stage"
    exit 0
fi

# --- assemble ----------------------------------------------------------------
out=$build/sch-pdf-compare-$version-x86_64-setup.exe
rm -f "$out"
makensis -V2 \
    -DVERSION="$version" \
    -DSTAGE="$stage" \
    -DFILES_NSH="$files" \
    -DUNINSTALL_NSH="$uninstall" \
    -DOUTFILE="$out" \
    sch-pdf-compare.nsi || exit 2

echo
echo "windows: $out"
echo "         $(du -h "$out" | cut -f1), from $size staged"
