#!/usr/bin/env bash
# Build the small shared Qt this application's AppImage carries.
#
# Two reasons it is not the distribution's Qt.
#
# **The floor.** Qt's own Linux binaries, and Ubuntu 24.04's packages, want
# glibc 2.39, so an AppImage built against either runs on Ubuntu 24.04 and
# little else. Built on manylinux_2_28 it runs on glibc 2.28 and up.
#
# **The title bar.** GNOME advertises no `zxdg_decoration_manager_v1`, so on a
# GNOME desktop the title bar is drawn by Qt, by whichever decoration plugin is
# installed. Qt Base ships only `bradient`, which draws a title bar out of 1995
# and handles two gestures — a click on a button and a drag to move. It has no
# clock in it, so it cannot recognise a double click, and double-clicking that
# title bar does nothing. The `adwaita` decoration matches the desktop's own
# and toggles maximised on a double click. It lives in Qt Wayland rather than
# Qt Base, and `QT_FEATURE_wayland_decoration_adwaita` turns itself off unless
# Qt Svg is already installed — which is why the order below is load-bearing,
# and why a build without those two stages is silently the old title bar.
set -euo pipefail

cd "$(dirname "$0")"
. ./toolchain.env

resume=0
check=0
while :; do
    case "${1:-}" in
    --resume) resume=1; shift ;;
    # Ask the question this script already asks itself and answer it instead of
    # acting on it, so the packaging script can refuse early rather than build
    # an image against a half-built Qt.
    --check) check=1; shift ;;
    *) break ;;
    esac
done
[ "$#" -le 1 ] || { echo "usage: $0 [--resume] [--check] [PREFIX]" >&2; exit 2; }

version=$QT_VERSION
prefix=${1:-${SC_QT_PREFIX:-$PWD/toolchain/qt-$version}}
work=${SC_QT_BUILD_ROOT:-$PWD/toolchain/source-$version}
downloads=$work/downloads
src=$work/src
build=$work/build
export CMAKE_BUILD_PARALLEL_LEVEL=${CMAKE_BUILD_PARALLEL_LEVEL:-${BUILD_JOBS:-4}}

# Everything this application actually needs out of the prefix. Each one has
# been missing at least once in ../Sterna's history of this build, and every
# absence is silent: the window still opens.
wanted() {
    [ -e "$prefix/plugins/platforms/libqxcb.so" ] || return 1
    [ -e "$prefix/plugins/platforms/libqwayland.so" ] || return 1
    [ -e "$prefix/plugins/wayland-shell-integration/libxdg-shell.so" ] || return 1
    [ -e "$prefix/plugins/wayland-graphics-integration-client/libqt-plugin-wayland-egl.so" ] || return 1
    [ -e "$prefix/plugins/wayland-decoration-client/libadwaita.so" ] || return 1
    [ -e "$prefix/plugins/printsupport/libcupsprintersupport.so" ] || return 1
    return 0
}

qmake=$prefix/bin/qmake6
if [ -x "$qmake" ] && [ "$("$qmake" -query QT_VERSION)" = "$version" ] && wanted; then
    printf 'qt: using cached Qt %s in %s\n' "$version" "$prefix"
    exit 0
fi
if [ "$check" = 1 ]; then
    printf 'qt: no usable Qt %s in %s\n' "$version" "$prefix" >&2
    exit 1
fi

for command in cmake curl make sha256sum tar; do
    command -v "$command" >/dev/null || { echo "qt: $command is required" >&2; exit 2; }
done

mkdir -p "$downloads" "$src" "$build"

# The published SHA-256 of each source archive, checked before it is unpacked.
fetch_module() {
    local module=$1 sha=$2
    local archive=$downloads/$module-everywhere-src-$version.tar.xz
    local url=https://download.qt.io/official_releases/qt/${version%.*}/$version/submodules/$(basename "$archive")
    if [ ! -f "$archive" ] || ! echo "$sha  $archive" | sha256sum -c - >/dev/null 2>&1; then
        echo "qt: fetching $(basename "$archive")" >&2
        curl --fail --location --retry 3 "$url" -o "$archive"
    fi
    echo "$sha  $archive" | sha256sum -c - >/dev/null || {
        echo "qt: checksum failed for $archive" >&2
        exit 2
    }
    printf '%s\n' "$archive"
}

if [ "$resume" = 1 ]; then
    [ -f "$build/qtbase/CMakeCache.txt" ] || {
        echo "qt: no configured Qt build to resume in $build/qtbase" >&2
        exit 2
    }
    echo "qt: resuming Qt Base $version" >&2
else
    base_archive=$(fetch_module qtbase d9594a31228aa23ad6b531719a29b45f0f3989fe6c136d45767ea179f233c1ac)

    rm -rf "$src/qtbase" "$build/qtbase" "$prefix"
    mkdir -p "$src/qtbase" "$build/qtbase" "$prefix"
    tar -xf "$base_archive" -C "$src/qtbase" --strip-components=1

    echo "qt: configuring Qt Base $version" >&2
    (
        cd "$build/qtbase"
        # No OpenSSL and no Vulkan: this application opens two local files and
        # draws them, and every feature left in is a library the image has to
        # carry and a licence to account for. GTK3 off for the same reason —
        # the platform theme it provides would pull the whole GTK stack in to
        # colour a file dialog.
        "$src/qtbase/configure" \
            -prefix "$prefix" \
            -release \
            -opensource -confirm-license \
            -nomake examples -nomake tests \
            -no-openssl \
            -no-feature-vulkan \
            -no-feature-gtk3 \
            -- -G "Unix Makefiles"
    )
fi

echo "qt: building Qt Base" >&2
cmake --build "$build/qtbase" --parallel "$CMAKE_BUILD_PARALLEL_LEVEL"
cmake --install "$build/qtbase"

# Qt Svg first: Qt Wayland's adwaita decoration switches itself off if Svg is
# not already installed when it configures. Both build against the Qt Base just
# installed, through `qt-cmake` so they take its toolchain file, and both
# install into the same prefix.
for module in qtsvg qtwayland; do
    case $module in
    qtsvg) sha=7f3cf02f4824bf03c2c5859ea6db173bf1482a1daf24e6cdf7bc78cfa26a8a94 ;;
    qtwayland) sha=95788aa502f75441d4edf65932b235f76523084e13dbbb7b9ee2d207b32bd9b3 ;;
    esac
    archive=$(fetch_module "$module" "$sha")
    echo "qt: building $module $version" >&2
    rm -rf "${src:?}/$module" "${build:?}/$module"
    mkdir -p "$src/$module" "$build/$module"
    tar -xf "$archive" -C "$src/$module" --strip-components=1
    "$prefix/bin/qt-cmake" -S "$src/$module" -B "$build/$module" \
        -G "Unix Makefiles" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$prefix" \
        -DQT_BUILD_EXAMPLES=OFF \
        -DQT_BUILD_TESTS=OFF
    cmake --build "$build/$module" --parallel "$CMAKE_BUILD_PARALLEL_LEVEL"
    cmake --install "$build/$module"
done

# Name the missing one. "Qt did not build" is not a useful thing to read at
# this point, and each of these fails quietly at run time rather than loudly
# here: no title bar, no printer, or a Wayland window drawn through software.
for want in \
    plugins/platforms/libqxcb.so \
    plugins/platforms/libqwayland.so \
    plugins/wayland-shell-integration/libxdg-shell.so \
    plugins/wayland-graphics-integration-client/libqt-plugin-wayland-egl.so \
    plugins/wayland-decoration-client/libadwaita.so \
    plugins/printsupport/libcupsprintersupport.so; do
    [ -e "$prefix/$want" ] || { echo "qt: $want was not built" >&2; exit 2; }
done

echo "qt: installed Qt $("$qmake" -query QT_VERSION) in $prefix"
