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
for tool in rsvg-convert patchelf; do
    if ! command -v "$tool" >/dev/null; then
        echo "no $tool on PATH — install it (librsvg2-bin, patchelf)" >&2
        exit 1
    fi
done

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

# Rendered here rather than committed. `check.sh` refuses any tracked image,
# because a rendered crop of a customer drawing was committed once as a README
# illustration and a picture of a drawing is the drawing — so the guard stays
# absolute and the icon lives in the repository as the SVG it was drawn as.
icons=$out/icons
rm -rf "$icons"
mkdir -p "$icons"
for s in 256 128 64 48 32; do
    rsvg-convert -w "$s" -h "$s" -o "$icons/icon-${s}.png" "$here/sch-pdf-compare.svg"
    d="$out/AppDir/usr/share/icons/hicolor/${s}x${s}/apps"
    mkdir -p "$d"
    install -m644 "$icons/icon-${s}.png" "$d/sch-pdf-compare.png"
done

export QMAKE=${QMAKE:-$(command -v qmake6 || echo /usr/lib/qt6/bin/qmake)}

# Qt's Wayland client loads its EGL buffer integration from
# `wayland-graphics-integration-client`, and linuxdeploy-plugin-qt only deploys
# that directory for a Qt Wayland *compositor*, never for a client. Without it
# Qt falls back to shared-memory buffers — and, less obviously, draws no window
# decoration at all, so the window comes up on GNOME with no title bar and no
# close button. Deploy the one client plugin by hand, before linuxdeploy runs,
# so it deploys the libraries it needs along with everything else.
qtplugins=$("$QMAKE" -query QT_INSTALL_PLUGINS 2>/dev/null || echo /usr/lib/x86_64-linux-gnu/qt6/plugins)
gfx=$qtplugins/wayland-graphics-integration-client/libqt-plugin-wayland-egl.so
if [ -f "$gfx" ]; then
    d=$out/AppDir/usr/plugins/wayland-graphics-integration-client
    mkdir -p "$d"
    install -m644 "$gfx" "$d/"
    # linuxdeploy rewrites the run path of everything it deploys itself and
    # leaves anything already in the AppDir alone, so this copy has to be
    # pointed at the bundled Qt by hand. Without it the plugin still loads its
    # metadata — so the log says the integration is *available* — and then fails
    # to dlopen for want of libQt6WaylandEglClientHwIntegration.so.6. On a build
    # machine that has Qt installed it finds the system copy and everything
    # looks right; on the machine the image was made for, it does not. Test on
    # the target, not on the machine that built it.
    patchelf --set-rpath '$ORIGIN/../../lib:$ORIGIN' "$d/libqt-plugin-wayland-egl.so"
else
    echo "warning: no $gfx — the AppImage will have no window decorations on Wayland" >&2
fi

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
    --icon-file "$icons/icon-256.png" \
    --plugin qt \
    --output appimage

# The failure this catches took two rounds to find: a plugin whose run path
# still points at the build machine loads there and nowhere else, and the only
# symptom is a feature quietly missing. Every plugin must reach the bundled Qt
# through $ORIGIN, and no plugin may want a Qt library the image does not carry.
bad=0
for so in $(find "$out/AppDir/usr/plugins" -name '*.so'); do
    case $(patchelf --print-rpath "$so") in
        *'$ORIGIN'*) ;;
        *) echo "plugin has no \$ORIGIN run path: ${so#"$out/AppDir/"}" >&2; bad=1 ;;
    esac
    for need in $(patchelf --print-needed "$so" | grep '^libQt6'); do
        if [ ! -e "$out/AppDir/usr/lib/$need" ]; then
            echo "plugin wants $need, which the image does not carry: ${so#"$out/AppDir/"}" >&2
            bad=1
        fi
    done
done
if [ "$bad" -ne 0 ]; then
    echo "the image would work on this machine and fail on another" >&2
    exit 1
fi

echo
echo "wrote $OUTPUT"
