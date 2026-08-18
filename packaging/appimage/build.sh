#!/usr/bin/env bash
# Builds an AppImage of the application.
#
# The Qt it packages is this machine's, so the result carries this machine's
# glibc floor with it — an AppImage built on Ubuntu 24.04 will not start on a
# distribution older than that. Building against an old Qt in an old container
# is what fixes it, and is a larger job than this script; until then, say which
# machine built it.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
build=${1:-$root/shell/build-release}
out=$root/dist

if [ ! -x "$build/sch-pdf-compare" ]; then
    echo "no application at $build/sch-pdf-compare" >&2
    echo "build it first: cmake -S shell -B $build -G Ninja -DCMAKE_BUILD_TYPE=Release" >&2
    exit 1
fi

# The tools, extracted rather than run as AppImages: an AppImage needs FUSE to
# mount itself, and a build machine or a container frequently has none.
tools=${LINUXDEPLOY_DIR:-$HOME/.local/bin}
ld=$tools/linuxdeploy.dir/AppRun
qtdir=$tools/linuxdeploy-qt.dir
for t in "$ld" "$qtdir/AppRun"; do
    if [ ! -x "$t" ]; then
        echo "missing $t — see the README in this directory" >&2
        exit 1
    fi
done

# linuxdeploy finds its plugins by searching PATH for `linuxdeploy-plugin-*`,
# and will happily find the plugin's own .AppImage and try to run it — which
# needs FUSE to self-mount and fails with exit 127 where there is none. Give it
# a small wrapper onto the extracted copy, on a PATH of our own, so it cannot
# reach for the other.
shim=$(mktemp -d)
trap 'rm -rf "$shim"' EXIT
cat > "$shim/linuxdeploy-plugin-qt" <<WRAP
#!/bin/sh
exec "$qtdir/AppRun" "\$@"
WRAP
chmod +x "$shim/linuxdeploy-plugin-qt"
export PATH="$shim:$PATH"

rm -rf "$out/AppDir"
mkdir -p "$out/AppDir/usr/bin" "$out/AppDir/usr/lib"

install -m755 "$build/sch-pdf-compare" "$out/AppDir/usr/bin/"
# The core is loaded by SONAME from beside the application, so it has to travel.
install -m755 "$build/cargo/release/libschcompare.so" "$out/AppDir/usr/lib/"

for s in 256 128 64 48 32; do
    d="$out/AppDir/usr/share/icons/hicolor/${s}x${s}/apps"
    mkdir -p "$d"
    install -m644 "$here/icon-${s}.png" "$d/sch-pdf-compare.png"
done

export QMAKE=${QMAKE:-$(command -v qmake6 || echo /usr/lib/qt6/bin/qmake)}
# Widgets and PrintSupport are what the application links; the plugin works the
# rest out from the binary itself.
export EXTRA_QT_MODULES="widgets;printsupport"
# `xcb` comes by default and is all an X session needs. `wayland` matters
# because a current Linux desktop is frequently Wayland, and without it the
# application goes through XWayland instead. `offscreen` is what a test or a
# headless machine asks for.
export EXTRA_PLATFORM_PLUGINS="libqwayland-generic.so;libqwayland-egl.so;libqoffscreen.so"
# Deliberately not set: an empty LDAI_UPDATE_INFORMATION is read as a malformed
# update string rather than as no update string, and fails the last step after
# the image has already been built.
unset LDAI_UPDATE_INFORMATION
export OUTPUT="$out/sch-pdf-compare-x86_64.AppImage"

"$ld" --appdir "$out/AppDir" \
    --executable "$out/AppDir/usr/bin/sch-pdf-compare" \
    --library "$out/AppDir/usr/lib/libschcompare.so" \
    --desktop-file "$here/sch-pdf-compare.desktop" \
    --icon-file "$here/icon-256.png" \
    --plugin qt \
    --output appimage

echo
echo "wrote $OUTPUT"
