#!/usr/bin/env bash
# What the Windows cross-build needs, inside the Fedora container.
#
# Fedora packages every piece of this, which is why the container is a Fedora:
#
#   mingw64-qt6-qtbase   Qt 6 for MinGW, with host-runnable moc, rcc and uic
#   mingw64-gcc-c++      the cross compiler, and the binutils that come with it
#   rust-std-static-...  the Rust standard library for x86_64-pc-windows-gnu,
#                        so no rustup download is needed
#   mingw64-nsis         the amd64 installer stubs; `mingw-nsis-base` under it
#                        owns makensis, which is a Linux binary — the reason
#                        NSIS is used at all
#   mingw32-nsis         the x86 stubs, which this installer does not use and
#                        cannot be built without: makensis loads its *default*
#                        stub as it starts, before it has read the `Target
#                        amd64-unicode` line that says which one is wanted. With
#                        only the amd64 package installed it stops at "error
#                        setting default stub" and never reads the script
#   clang                for the bindings mupdf-sys generates
#
# Wine is deliberately not here. It is `verify.sh`'s tool rather than the
# build's, and it is the biggest download of the lot — `installer.sh --verify`
# installs it when it is going to be used. It has to be the whole `wine`
# package: with `wine-core` alone a silent install of this installer returns 0
# and puts nothing on disk, which is worse than an error.
set -euo pipefail

dnf -y install --setopt=install_weak_deps=False \
    mingw64-gcc-c++ mingw64-qt6-qtbase mingw64-winpthreads \
    mingw64-nsis mingw32-nsis \
    cargo rust rust-std-static-x86_64-pc-windows-gnu \
    cmake ninja-build clang git findutils
