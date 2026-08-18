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

## Running it

```sh
./dist/sch-pdf-compare-x86_64.AppImage earlier.pdf later.pdf
```

An AppImage mounts itself with FUSE 2. Where that is missing:

```sh
./dist/sch-pdf-compare-x86_64.AppImage --appimage-extract-and-run earlier.pdf later.pdf
```
