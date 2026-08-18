# AppImage

```sh
cmake -S shell -B shell/build-release -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build shell/build-release
./packaging/appimage/build.sh
```

The image is written to `dist/sch-pdf-compare-x86_64.AppImage`.

## The tools it needs

`linuxdeploy` and its Qt plugin, **extracted** rather than left as AppImages:

```sh
mkdir -p ~/.local/bin && cd ~/.local/bin
curl -sSLO https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
curl -sSLO https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage
chmod +x linuxdeploy*.AppImage
./linuxdeploy-x86_64.AppImage --appimage-extract && mv squashfs-root linuxdeploy.dir
./linuxdeploy-plugin-qt-x86_64.AppImage --appimage-extract && mv squashfs-root linuxdeploy-qt.dir
mkdir -p ~/.local/ldsrc && mv linuxdeploy*.AppImage ~/.local/ldsrc/
```

Extracted, because an AppImage mounts itself with FUSE and a build machine
frequently has none. The last line matters as much as the rest: `linuxdeploy`
looks for its plugins in its own directory as well as on `PATH`, so a plugin left
there as an AppImage is the one it finds, and it fails with exit code 127.

## What the image carries

Qt comes from the machine that built it, so **the image carries that machine's
glibc floor**. One built on Ubuntu 24.04 needs glibc 2.39 and will not start on
anything older. Building against an older Qt inside an old container is what
fixes that, and is a larger job than this script.

The platform plugins bundled are `xcb`, `wayland` and `offscreen`. Without the
Wayland one a current desktop goes through XWayland instead.

One more Qt plugin is copied in by hand, and the reason is worth reading before
anyone removes it. `linuxdeploy-plugin-qt` deploys
`wayland-graphics-integration-client` only for a Qt Wayland **compositor**, and
this is a Wayland **client**, so its EGL buffer integration was left out. The
visible result was not slower drawing — it was **a window with no title bar and
no close button on GNOME**. Qt draws its own decoration on Wayland, from
`wayland-decoration-client/libbradient.so`, and with no client buffer
integration it never asks for one. The proof either way is in the log:

```sh
QT_QPA_PLATFORM=wayland QT_LOGGING_RULES='qt.qpa.wayland=true' \
    ./dist/sch-pdf-compare-x86_64.AppImage 2>&1 | grep configure
```

`xdg_toplevel.configure` reporting the window's own size means no decoration;
the same size plus a frame — 1400x950 asked for, 1406x983 configured — means
bradient is drawing one.

The icon is rendered from `sch-pdf-compare.svg` by `rsvg-convert` at build time
rather than committed, because `check.sh` refuses any tracked image at all: a
rendered crop of a customer drawing was committed once as a README illustration,
and a picture of a drawing is the drawing.

## Running it

```sh
./dist/sch-pdf-compare-x86_64.AppImage earlier.pdf later.pdf
```

An AppImage mounts itself with FUSE 2. Where that is missing:

```sh
./dist/sch-pdf-compare-x86_64.AppImage --appimage-extract-and-run earlier.pdf later.pdf
```
