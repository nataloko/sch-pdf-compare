# Qt in this installation

This program bundles Qt 6 under the **GNU Lesser General Public License v3**
(`LGPL-3.0.txt`, beside this file). sch-pdf-compare itself is
AGPL-3.0-or-later; the two are separate works, dynamically linked.

What the LGPL asks of a distributor who bundles Qt, and how this installation
answers:

**Qt is never static.** Every Qt module in the program folder is a separate
DLL — `Qt6Core.dll`, `Qt6Gui.dll`, `Qt6Widgets.dll`, `Qt6PrintSupport.dll` and
the platform plugin under `platforms\` — and the loader finds them there
because they sit beside the executable. So the substitution the licence exists
to protect works: replace a Qt DLL in that folder with your own build of the
same version and run the program again.

**The Qt source.** These are Fedora's MinGW packages of Qt's own releases.
`BUILD-INFO.txt` names the exact version, the toolchain and the base they were
compiled on.

<https://download.qt.io/official_releases/qt/>

**The application's source**, and the scripts that built this installer:

<https://github.com/nataloko/sch-pdf-compare>

The program links MuPDF, which is AGPL-3.0-or-later, and so is this program.
`LICENSE.txt` is that licence, and the repository above is the corresponding
source it asks for.
