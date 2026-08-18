#!/usr/bin/env bash
# Install the built installer under Wine, start what it installed, and
# uninstall it again.
#
#   ./verify.sh [path/to/setup.exe]
#
# Wine cannot tell you how this behaves on Windows. What it can answer is the
# question deployment actually fails on — did every DLL resolve, and does Qt
# find its platform plugin — and it answers it in a few seconds. A Qt program
# missing either dies in milliseconds, so *surviving* is the signal: the run
# below is expected to be killed by its own timeout.
#
# Run it in the same container as `build.sh`; `./installer.sh --verify` does
# both in one go.
set -uo pipefail

cd "$(dirname "$0")"
here=$PWD

setup=${1:-}
if [ -z "$setup" ]; then
    setup=$(ls -t build/sch-pdf-compare-*-x86_64-setup.exe 2>/dev/null | head -1)
fi
[ -n "$setup" ] && [ -f "$setup" ] || {
    echo "verify: no installer — run ./build.sh first" >&2; exit 2; }
setup=$(cd "$(dirname "$setup")" && pwd)/$(basename "$setup")

command -v wine >/dev/null || { echo "verify: no wine" >&2; exit 2; }

# A prefix of its own, thrown away at the end: this both keeps the check
# repeatable and makes "the uninstaller removed everything" a question that can
# actually be asked.
export WINEPREFIX=${WINEPREFIX:-$(mktemp -d)/prefix}
export WINEDEBUG=${WINEDEBUG:--all}
export WINEDLLOVERRIDES=${WINEDLLOVERRIDES:-mscoree=d}
fail=0
say() { printf '%-6s %s\n' "$1" "$2"; }
check() { if [ "$1" = 0 ]; then say ok "$2"; else say FAIL "$2"; fail=$((fail + 1)); fi; }

echo "verify: $(basename "$setup")"
timeout 300 wineboot -i >/dev/null 2>&1

# --- it installs, silently ---------------------------------------------------
timeout 300 wine "$setup" /S >/dev/null 2>&1
prog="$WINEPREFIX/drive_c/Program Files/sch-pdf-compare"
[ -d "$prog" ]; check $? "the silent install made the program folder"
n=$(find "$prog" -type f 2>/dev/null | wc -l)
[ "$n" -gt 20 ]; check $? "it holds $n files"
for f in sch-pdf-compare.exe schcompare.dll Qt6Widgets.dll platforms/qwindows.dll \
         doc/LICENSE.txt doc/QT-LGPL-NOTICE.md uninstall.exe; do
    [ -f "$prog/$f" ]; check $? "  $f"
done

# --- and what it installed starts -------------------------------------------
#
# The one failure this whole exercise is about: a Qt program with a DLL missing
# or no platform plugin exits at once. It has no `--version` that proves as
# much, because a GUI-subsystem program answering `--version` never constructs
# a window — so the check is that it is still running when the timeout fires.
#
# Nothing below is meaningful if the install did not happen, and it does not
# fail honestly either: with no directory to change into, the run below took
# place in the staging tree and passed. Stop here instead.
[ -d "$prog" ] || {
    echo
    echo "verify: nothing was installed, so the rest was not attempted" >&2
    exit 1
}
cd "$prog" || exit 1
timeout 15 wine sch-pdf-compare.exe -platform offscreen >/dev/null 2>&1
rc=$?
[ "$rc" = 124 ]; check $? "it starts and stays up (exit $rc, 124 is the timeout)"
wineserver -k 2>/dev/null

# --- the uninstaller takes what it put there, and nothing else ---------------
echo "a file the user left here" > "$prog/notes.txt"
timeout 300 wine "$prog/uninstall.exe" /S _?="$(winepath -w "$prog" 2>/dev/null || echo "$prog")" \
    >/dev/null 2>&1
sleep 2
left=$(find "$prog" -type f 2>/dev/null | sed "s|$prog/||" | sort | tr '\n' ' ')
[ "$left" = "notes.txt " ]; check $? "the uninstaller left only what it did not install: [$left]"
rm -rf "$prog"

echo
[ "$fail" = 0 ] && echo "verify: all good" || echo "verify: $fail failed"
exit $((fail > 0))
