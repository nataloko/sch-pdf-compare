# AppImage

```sh
./packaging/appimage/portable.sh
```

One command, and it writes `dist/sch-pdf-compare-x86_64.AppImage`. It needs
`podman` (or `docker`) and nothing else — every compiler, library and tool it
uses lives inside a container image pinned by digest in `toolchain.env`.

The first run takes about half an hour, almost all of it Qt. Qt lands in
`packaging/appimage/toolchain/`, which is ignored by git and reused, so later
runs take a couple of minutes.

## Why it builds its own Qt

**The floor.** Qt's own Linux binaries, and Ubuntu 24.04's packages, want glibc
2.39 — so an AppImage built against either runs on Ubuntu 24.04 and little else,
which is not what the word portable is for. Built on the maintained
`manylinux_2_28` image, everything in here runs on glibc 2.28 and up: RHEL 8,
Debian 10, Ubuntu 18.10, and anything since. `build.sh` reads back what the
packaged binaries actually import and refuses to finish if a new dependency has
raised the floor.

**The title bar.** GNOME advertises no `zxdg_decoration_manager_v1`, so on a
GNOME desktop the title bar above this window is drawn by Qt, by whichever
decoration plugin is installed. Qt Base ships only `bradient`, which draws a
title bar out of 1995 and handles two gestures — a click on a button and a drag
to move. It has no clock in it, so it cannot recognise a double click, and
double-clicking that title bar does nothing. The `adwaita` decoration matches
the desktop's own and toggles maximised on a double click. It lives in Qt
Wayland rather than Qt Base, and `QT_FEATURE_wayland_decoration_adwaita` turns
itself off unless Qt Svg is already installed — which is why `build-qt.sh`
builds Qt Svg first and checks for `libadwaita.so` afterwards.

Both are the same lesson as `../Sterna`, whose scripts this is adapted from.

## The pieces

| | |
| --- | --- |
| `toolchain.env` | The image digest, the Qt version, and how much of the machine the build may take. |
| `in-container.sh` | Runs a command in that image with the repository mounted. Falls back to the host's `podman` when the development container has none. |
| `install-build-deps.sh` | The distribution packages and Rust, inside the container. |
| `build-qt.sh` | Qt Base, Qt Svg, Qt Wayland, from their verified source archives. |
| `build.sh` | The application, the bundle, the floor check, the image. |
| `portable.sh` | All four, in one container. |

## Resources

`BUILD_JOBS` and `BUILD_CPUS` in `toolchain.env` bound the build to six of them.
Qt Base is by far the longest compile in this repository and a container given
every core makes the desktop unusable for the duration. Raise them on a machine
with nothing else to do.

## What is in the image, and what is not

Bundled: Qt as separate shared libraries, its platform plugins (`xcb`,
`wayland`, `offscreen`), the Wayland shell integration, the EGL client buffer
integration, the Adwaita decoration, the CUPS print plugin, and the GLVND
front-ends (`libEGL.so.1` and friends — the driver-neutral ABI, never a driver).

Not bundled: glibc, libstdc++, and the graphics driver, which come from the
machine it runs on.

Each of the Wayland plugins is silent when it is missing — the window still
opens — so `build.sh` checks for each by name and refuses rather than shipping
an image that is quietly worse:

| missing | what you see |
| --- | --- |
| `wayland-shell-integration/libxdg-shell.so` | no window at all, and no error |
| `wayland-graphics-integration-client/libqt-plugin-wayland-egl.so` | software buffers — and no title bar, because Qt only draws a decoration when a client buffer integration came up |
| `wayland-decoration-client/libadwaita.so` | `bradient`'s title bar from 1995 |
| `printsupport/libcupsprintersupport.so` | no printers, and "there are no printers" is a real answer |

## Qt's licence

Qt is LGPLv3 and is dynamically linked, never static. The bundled libraries are
byte for byte what the build produced — no run paths rewritten — and `AppRun`
finds them through `LD_LIBRARY_PATH`, so the substitution the licence protects
actually works. `QT-LGPL-NOTICE.md` says how, and travels inside the image
along with the licence text and a `BUILD-INFO.txt` naming the exact Qt.

## Running it

```sh
./dist/sch-pdf-compare-x86_64.AppImage earlier.pdf later.pdf
```

An AppImage mounts itself with FUSE 2. Where that is missing:

```sh
./dist/sch-pdf-compare-x86_64.AppImage --appimage-extract-and-run earlier.pdf later.pdf
```

## Checking the Wayland side without a Wayland desktop

```sh
QT_QPA_PLATFORM=wayland QT_LOGGING_RULES='qt.qpa.wayland=true' \
    ./dist/sch-pdf-compare-x86_64.AppImage 2>&1 | grep -E 'buffer integration|configure with'
```

`xdg_toplevel.configure` reporting the window's own size means no decoration is
being drawn; the same size plus a frame means one is.
