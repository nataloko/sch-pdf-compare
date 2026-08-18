#!/usr/bin/env bash
# Cross-builds the Rust core for Windows and checks it against the ABI harness
# under Wine.
#
# The Qt shell is not built here, and that is a limitation of this machine
# rather than of the project. Building on Windows needs nothing but the official
# Qt and the same one-command CMake build; cross-building the shell from Linux
# needs a MinGW Qt 6 whose moc/rcc/uic run on the build host, which Ubuntu does
# not package and Fedora does. The core is the half that was in doubt, and it is
# the half this proves.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
target=x86_64-pc-windows-gnu
profile=${1:-debug}

flags=()
[ "$profile" = release ] && flags+=(--release)

cargo build --manifest-path "$root/crates/Cargo.toml" -p sc-ffi --target "$target" "${flags[@]}"

out="$root/crates/target/$target/$profile"
echo "built: $out/schcompare.dll"

if ! command -v x86_64-w64-mingw32-gcc >/dev/null; then
    echo "no mingw compiler; skipping the harness" >&2
    exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
x86_64-w64-mingw32-gcc -Wall -Wextra -Werror -pedantic \
    -I "$root/crates/sc-ffi/include" "$root/crates/sc-ffi/tests/abi.c" \
    -L "$out" -lschcompare -o "$work/abi.exe"
cp "$out/schcompare.dll" "$work/"

wine=${WINE:-/usr/lib/wine/wine64}
if [ ! -x "$wine" ]; then
    wine=$(command -v wine || true)
fi
if [ -z "$wine" ] || [ ! -x "$wine" ]; then
    echo "no wine; built but not run" >&2
    exit 0
fi

# A prefix under $HOME, not /tmp: wine refuses to create one in a directory it
# does not own, which on a shared machine /tmp is not.
export WINEPREFIX=${WINEPREFIX:-$HOME/.wineprefix-sch-pdf-compare}
export WINEDEBUG=${WINEDEBUG:--all}
cd "$work"
"$wine" abi.exe
