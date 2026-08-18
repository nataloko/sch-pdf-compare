#!/usr/bin/env bash
# Prepare the maintained manylinux_2_28 image to build Qt, the core and the shell.
set -euo pipefail

if [ "$(id -u)" -ne 0 ] || ! grep -q '^PLATFORM_ID="platform:el8"' /etc/os-release; then
    echo "deps: this runs inside the manylinux_2_28 container" >&2
    exit 2
fi

dnf install -y -q epel-release
# Qt Base wants the X11, Wayland and font stacks; CUPS is not optional here
# because this application prints, and `mupdf-sys` needs a C toolchain and
# libclang for its bindgen step.
dnf install -y -q \
    autoconf automake binutils bzip2 cmake curl file gcc-c++ git gzip libtool \
    make ninja-build patchelf tar xz zstd \
    clang-devel llvm-devel \
    at-spi2-core-devel cups-devel dbus-devel fontconfig-devel freetype-devel \
    glib2-devel libdrm-devel libjpeg-turbo-devel librsvg2-tools libX11-devel \
    libXext-devel libXfixes-devel libXi-devel libXrender-devel libxcb-devel \
    libxkbcommon-devel libxkbcommon-x11-devel mesa-libGL-devel systemd-devel \
    wayland-devel wayland-protocols-devel xcb-util-cursor-devel \
    xcb-util-devel xcb-util-image-devel xcb-util-keysyms-devel \
    xcb-util-renderutil-devel xcb-util-wm-devel

# Rust, into the shared cargo home under the repository's toolchain directory
# so a second run reuses it and nothing lands in the image.
if ! command -v cargo >/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
fi
