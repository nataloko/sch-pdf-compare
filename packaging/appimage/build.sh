#!/usr/bin/env bash
# Build the AppImage. Run this inside the pinned container:
#
#   ./packaging/appimage/portable.sh          everything, from a clean machine
#   ./packaging/appimage/in-container.sh ./packaging/appimage/build.sh
#
# Everything that ends up in the image is compiled in there, against the Qt that
# `build-qt.sh` built on the same base — that is what makes the glibc floor
# 2.28 rather than the build machine's.
#
# What the licences oblige, since this bundles Qt rather than depending on the
# distribution's: never static-link Qt, keep it as separate shared libraries a
# user can substitute, and put the LGPL text and an offer of source inside the
# image. See QT-LGPL-NOTICE.md, which is copied in.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
. "$here/toolchain.env"

build=$here/build
appdir=$build/AppDir
tools=$here/toolchain/tools
out=$root/dist

mkdir -p "$tools" "$out"

# --- the tools ---------------------------------------------------------------
# Pinned to release tags rather than to "continuous": a build tool that changes
# under us turns the artefact red with no change on our side.
LD_URL=https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20250213-2/linuxdeploy-x86_64.AppImage
LDQT_URL=https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-qt-x86_64.AppImage
AT_URL=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage

fetch() {
    local url=$1 dest=$2
    [ -x "$dest" ] && return 0
    echo "appimage: fetching $(basename "$dest")" >&2
    curl -fsSL --retry 3 "$url" -o "$dest"
    chmod +x "$dest"
}
fetch "$LD_URL" "$tools/linuxdeploy"
fetch "$LDQT_URL" "$tools/linuxdeploy-plugin-qt"
fetch "$AT_URL" "$tools/appimagetool"

# Those three are themselves AppImages, and a rootless container has /dev/fuse
# but no working fusermount helper. Extract-and-run is the documented way out.
# It also settles the plugin-discovery problem for good: linuxdeploy finds
# `linuxdeploy-plugin-qt` beside itself on PATH rather than reaching for a
# FUSE-bound AppImage it cannot mount.
export APPIMAGE_EXTRACT_AND_RUN=1
# linuxdeploy carries its own binutils, older than the libraries it is pointed
# at; this switches off the half of that which fails loudly.
export NO_STRIP=1
export PATH="$tools:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

prefix=$here/toolchain/qt-$QT_VERSION
"$here/build-qt.sh" --check "$prefix" || {
    echo "appimage: no portable Qt — run build-qt.sh first" >&2
    exit 2
}
export PATH="$prefix/bin:$PATH"
for command in cargo cmake rsvg-convert readelf; do
    command -v "$command" >/dev/null || { echo "appimage: $command is required" >&2; exit 2; }
done

qt_plugins=$("$prefix/bin/qmake6" -query QT_INSTALL_PLUGINS)
qt_libs=$("$prefix/bin/qmake6" -query QT_INSTALL_LIBS)
# This Qt lives in a private prefix rather than the base image's linker cache.
# Make it visible to linuxdeploy's dependency resolver.
export LD_LIBRARY_PATH="$qt_libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' \
    "$root/crates/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "appimage: no version in crates/Cargo.toml" >&2; exit 2; }

# --- build -------------------------------------------------------------------
# Release, which also builds the Rust core with --release: CMakeLists drives
# cargo and takes the profile from CMAKE_BUILD_TYPE.
echo "appimage: building the application" >&2
cmake -S "$root/shell" -B "$build/cmake" -G "Unix Makefiles" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH="$prefix" >/dev/null
cmake --build "$build/cmake" --target sch-pdf-compare

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib" "$appdir/usr/plugins"
install -m755 "$build/cmake/sch-pdf-compare" "$appdir/usr/bin/"
# The core is loaded by SONAME from beside the application, so it has to travel.
install -m755 "$build/cmake/cargo/release/libschcompare.so" "$appdir/usr/lib/"

# --- the icon ----------------------------------------------------------------
# Rendered from the SVG rather than committed: `check.sh` refuses any tracked
# image, because a rendered crop of a customer drawing was committed once as a
# README illustration and a picture of a drawing is the drawing.
#
# One directory per size, all called `sch-pdf-compare.png`. linuxdeploy names
# the deployed icon after the *file* and then looks for the desktop entry's
# `Icon=` among those names, so handing it `icon-256.png` installs an icon
# called `icon-256` and fails with "could not find suitable icon".
icons=$build/icons
rm -rf "$icons"
icon_args=()
for size in 256 128 64 48 32; do
    mkdir -p "$icons/$size"
    rsvg-convert -w "$size" -h "$size" -o "$icons/$size/sch-pdf-compare.png" \
        "$here/sch-pdf-compare.svg"
    icon_args+=(--icon-file "$icons/$size/sch-pdf-compare.png")
done

# --- what the licences oblige ------------------------------------------------
docs=$appdir/usr/share/doc/sch-pdf-compare
mkdir -p "$docs"
cp "$root/LICENSE" "$docs/LICENSE"
cp "$here/QT-LGPL-NOTICE.md" "$here/LGPL-3.0.txt" "$docs/"
{
    echo "sch-pdf-compare AppImage build"
    echo
    echo "built:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "version:    $version"
    echo "commit:     $(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "base:       $(. /etc/os-release && echo "$PRETTY_NAME")"
    echo "glibc:      $(ldd --version | head -1)"
    echo "Qt:         $QT_VERSION, built from the official source on the base above"
    echo "Qt source:  https://download.qt.io/official_releases/qt/"
    echo
    echo "The glibc line is this image's floor: it will not start on a system"
    echo "with an older one."
} > "$docs/BUILD-INFO.txt"

# --- bundle ------------------------------------------------------------------
QMAKE=$prefix/bin/qmake6
export QMAKE
export EXTRA_QT_MODULES="widgets;printsupport;waylandclient"

# Asked for by what is on disk, not by name: upstream Qt splits the Wayland
# platform plugin into `libqwayland-generic.so` and `libqwayland-egl.so` and
# some distributions ship one `libqwayland.so`. Naming a plugin that is not
# there is a hard error from the plugin, after it has deployed everything else.
# `offscreen` is here because the window test runs headless.
want=
for p in libqwayland.so libqwayland-generic.so libqwayland-egl.so libqxcb.so libqoffscreen.so; do
    [ -e "$qt_plugins/platforms/$p" ] && want="$want${want:+;}$p"
done
[ -n "$want" ] || { echo "appimage: no usable platform plugin in $qt_plugins" >&2; exit 2; }
export EXTRA_PLATFORM_PLUGINS="$want"
# An empty LDAI_UPDATE_INFORMATION reads as a malformed update string rather
# than as no update string, and fails after the image is already built.
unset LDAI_UPDATE_INFORMATION

echo "appimage: bundling ($want)" >&2
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/sch-pdf-compare" \
    --library "$appdir/usr/lib/libschcompare.so" \
    --desktop-file "$here/sch-pdf-compare.desktop" \
    "${icon_args[@]}" \
    --plugin qt

# linuxdeploy classifies every OpenGL-shaped library as part of the host's
# driver stack. QtGui, though, links GLVND's driver-neutral ABI frontends even
# when the application only ever uses the raster painter. Bundle the dispatch
# ABI, never a Mesa or NVIDIA driver.
for so in libOpenGL.so.0 libEGL.so.1 libGLX.so.0 libGLdispatch.so.0; do
    found=
    for d in /usr/lib64 /usr/lib /lib64; do
        [ -e "$d/$so" ] && { cp -Lf "$d/$so" "$appdir/usr/lib/$so"; found=1; break; }
    done
    [ -n "$found" ] || { echo "appimage: GLVND frontend $so is missing" >&2; exit 2; }
done

# The Wayland platform plugin loads *more* plugins to do anything at all, and
# linuxdeploy only knows about some of them. Each of these is silent when it is
# missing, and each cost a round of "it works here" before it was found:
#
#   wayland-shell-integration       no xdg_toplevel: no window and no error
#   wayland-graphics-integration-client   software buffers — and, less
#                                   obviously, no title bar at all, because Qt
#                                   only draws a decoration when a client
#                                   buffer integration came up
#   wayland-decoration-client       the title bar itself, on any GNOME desktop
#   printsupport                    QPrinter finds no printers and says nothing
for d in wayland-shell-integration wayland-decoration-client \
         wayland-graphics-integration-client printsupport platformthemes; do
    [ -d "$qt_plugins/$d" ] && cp -r "$qt_plugins/$d" "$appdir/usr/plugins/"
done
for want in \
    wayland-shell-integration/libxdg-shell.so \
    wayland-graphics-integration-client/libqt-plugin-wayland-egl.so \
    wayland-decoration-client/libadwaita.so \
    printsupport/libcupsprintersupport.so; do
    [ -e "$appdir/usr/plugins/$want" ] || {
        echo "appimage: $want is not in the image" >&2
        exit 2
    }
done

# And every Qt library those plugins ask for has to come with them. Closed
# transitively rather than in one pass, because the plugins pull
# `libQt6WlShellIntegration`, which pulls more again.
qt_needed() {
    find "$appdir/usr/plugins" -name '*.so' -print0 |
        xargs -0 -r readelf -d 2>/dev/null |
        sed -n 's/.*NEEDED.*\[\(libQt6[^]]*\)\].*/\1/p'
    readelf -d "$appdir"/usr/lib/libQt6*.so.6 2>/dev/null |
        sed -n 's/.*NEEDED.*\[\(libQt6[^]]*\)\].*/\1/p'
}
for _ in 1 2 3 4 5; do
    missing=
    while read -r so; do
        [ -n "$so" ] || continue
        [ -e "$appdir/usr/lib/$so" ] && continue
        if [ -e "$qt_libs/$so" ]; then
            cp -f "$qt_libs/$so" "$appdir/usr/lib/$so"
        else
            missing="$missing $so"
        fi
    done < <(qt_needed | sort -u)
    [ -z "$missing" ] || {
        echo "appimage: plugins need Qt libraries that are not here:$missing" >&2
        exit 2
    }
    [ -z "$(qt_needed | sort -u | while read -r so; do
        [ -n "$so" ] && [ ! -e "$appdir/usr/lib/$so" ] && echo "$so"; done)" ] && break
done

# --- restore the source libraries -------------------------------------------
#
# Keep the original Qt files rather than linuxdeploy's run-path-rewritten
# copies; AppRun supplies the search path instead. This preserves the LGPL
# substitution seam — a user can drop in their own Qt — and it removes a whole
# class of bug: a hand-copied plugin whose run path still pointed at the build
# machine loaded there and nowhere else, twice, before this was written.
echo "appimage: restoring the libraries linuxdeploy rewrote" >&2
restored=0
for f in "$appdir"/usr/lib/*.so*; do
    [ -f "$f" ] || continue
    b=$(basename "$f")
    [ "$b" = "libschcompare.so" ] && continue   # ours, not on the system
    for d in "$qt_libs" /usr/lib64 /usr/lib /lib64; do
        [ -f "$d/$b" ] && { cp -f "$d/$b" "$f"; restored=$((restored + 1)); break; }
    done
done
while IFS= read -r p; do
    rel=${p#"$appdir"/usr/plugins/}
    [ -f "$qt_plugins/$rel" ] && { cp -f "$qt_plugins/$rel" "$p"; restored=$((restored + 1)); }
done < <(find "$appdir/usr/plugins" -name '*.so' 2>/dev/null)
echo "appimage: restored $restored" >&2

cat > "$appdir/AppRun" <<'APPRUN'
#!/usr/bin/env bash
# Resolution by LD_LIBRARY_PATH rather than by run path: the bundled libraries
# are byte for byte the build inputs, un-patched, which is what keeps the LGPL
# substitution seam open. Plugins are found through usr/bin/qt.conf.
here=$(readlink -f "$(dirname "$0")")
export APPDIR="${APPDIR:-$here}"
export LD_LIBRARY_PATH="$here/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
for hook in "$here"/apprun-hooks/*.sh; do
    [ -e "$hook" ] && . "$hook"
done
exec "$here/usr/bin/sch-pdf-compare" "$@"
APPRUN
chmod +x "$appdir/AppRun"
rm -f "$appdir/AppRun.wrapped"

# --- the floor ---------------------------------------------------------------
# Do not let a new Rust, C++ or Qt dependency silently undo the portable base.
# libstdc++ comes from the host, so its ABI ceiling matters as much as glibc's.
version_info=$(find "$appdir/usr" -type f -print0 | xargs -0 -r file | \
    sed -n 's/: .*ELF.*//p' | while read -r f; do
        readelf --version-info "$f" 2>/dev/null || true
    done)
glibc_floor=2.28
glibcxx_ceiling=3.4.25
cxxabi_ceiling=1.3.11
newest() { printf '%s\n' "$version_info" | grep -o "$1"'_[0-9][0-9.]*' | sed "s/$1//;s/^_//" | sort -Vu | tail -1; }
newest_glibc=$(newest GLIBC)
newest_glibcxx=$(newest GLIBCXX)
newest_cxxabi=$(newest CXXABI)
[ -n "$newest_glibc" ] && [ -n "$newest_glibcxx" ] && [ -n "$newest_cxxabi" ] || {
    echo "appimage: could not read what the packaged ELF requires" >&2
    exit 2
}
above() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -Vu | tail -1)" != "$1" ]; }
above "$glibc_floor" "$newest_glibc" && {
    echo "appimage: packaged ELF requires GLIBC_$newest_glibc, above $glibc_floor" >&2; exit 2; }
above "$glibcxx_ceiling" "$newest_glibcxx" && {
    echo "appimage: packaged ELF requires GLIBCXX_$newest_glibcxx, above $glibcxx_ceiling" >&2; exit 2; }
above "$cxxabi_ceiling" "$newest_cxxabi" && {
    echo "appimage: packaged ELF requires CXXABI_$newest_cxxabi, above $cxxabi_ceiling" >&2; exit 2; }
printf '\nverified max imports: GLIBC_%s, GLIBCXX_%s, CXXABI_%s\n' \
    "$newest_glibc" "$newest_glibcxx" "$newest_cxxabi" >> "$docs/BUILD-INFO.txt"

# --- pack --------------------------------------------------------------------
echo "appimage: packing" >&2
image=$out/sch-pdf-compare-x86_64.AppImage
rm -f "$image"
VERSION=$version "$tools/appimagetool" "$appdir" "$image" >/dev/null 2>&1 || {
    echo "appimage: appimagetool failed" >&2
    exit 2
}

echo
echo "appimage: $image"
echo "          $(du -h "$image" | cut -f1), glibc floor $newest_glibc"
