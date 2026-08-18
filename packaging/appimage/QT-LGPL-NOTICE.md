# Qt in this AppImage

This image bundles Qt 6 under the **GNU Lesser General Public License v3**
(`LGPL-3.0.txt`, beside this file). The application itself is
AGPL-3.0-or-later; the two are separate works, dynamically linked.

What the LGPL asks of a distributor who bundles Qt, and how this image answers:

**Qt is never static.** Every Qt module in `usr/lib` is a shared library, and
`AppRun` finds them through `LD_LIBRARY_PATH` rather than through a run path
baked into the binaries. Nothing here has been patched: the files are byte for
byte what the build produced. So the substitution the licence exists to
protect works — extract the image, replace a Qt library with your own build of
the same version, and run it.

```sh
./sch-pdf-compare-x86_64.AppImage --appimage-extract
cp /your/libQt6Widgets.so.6 squashfs-root/usr/lib/
./squashfs-root/AppRun
```

**The Qt source.** These are Qt's own release archives, unmodified, built with
the recipe in `build-qt.sh`. `BUILD-INFO.txt` names the exact version and the
base image it was compiled on.

<https://download.qt.io/official_releases/qt/>

**The application's source**, and the scripts that built this image:

<https://github.com/nataloko/sch-pdf-compare>
