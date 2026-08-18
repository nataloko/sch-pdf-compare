// The toolbar's pictures, painted here rather than loaded from anywhere.
//
// Three reasons, and the first two are the ones that settle it. There is no
// icon theme inside the AppImage and none on Windows, so a themed icon is a
// blank square on two of the three targets this ships to. And the four things
// this toolbar says most often — this revision, that revision, both at once,
// both alongside — have no icon in any theme that means them; the previous
// version of the toolbar was text-only for exactly that reason, and said so.
//
// The third is that the two overlay colours belong to the reader. A reviewer
// who moved them to blue and orange because red and green are the same colour
// to them would otherwise be left looking at a red button and a green one.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include <QColor>
#include <QIcon>

namespace Icons {

// What the pictures are drawn with: the toolbar's own foreground, the paper a
// drawing is on, and the reader's two overlay colours.
struct Look {
    QColor ink;
    QColor paper = QColor(Qt::white);
    QColor onlyA;
    QColor onlyB;
};

QIcon open(const Look &l);
QIcon onlyA(const Look &l);
QIcon onlyB(const Look &l);
QIcon overlay(const Look &l);
QIcon sideBySide(const Look &l);
QIcon singlePage(const Look &l);
QIcon previousChange(const Look &l);
QIcon nextChange(const Look &l);
QIcon excludeRegion(const Look &l);
QIcon fadeShared(const Look &l);

} // namespace Icons
