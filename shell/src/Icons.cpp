// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "Icons.h"

#include <QFont>
#include <QGuiApplication>
#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <QRectF>

#include <functional>

namespace {

// Each icon is drawn once per size it might be asked for, in a square of its
// own, rather than drawn large and scaled down. A one-pixel outline stays a
// one-pixel outline that way; scaling a 48-pixel drawing to 16 turns every
// sheet edge into grey mush at exactly the size a toolbar uses.
constexpr int kSizes[] = {16, 20, 24, 32, 48};

// The colour the core edges an excluded region with. The button and the thing
// it does are drawn in the same blue on purpose.
const QColor kMaskEdge(0x60, 0x84, 0xb0);

QRectF unitRect(int size, qreal x, qreal y, qreal w, qreal h) {
    return QRectF(x * size, y * size, w * size, h * size);
}

qreal penWidth(int size) {
    return qMax(1.0, size / 16.0);
}

void sheet(QPainter &g, const QRectF &r, const QColor &edge, const QColor &paper, qreal pen) {
    g.setPen(QPen(edge, pen));
    g.setBrush(paper);
    g.drawRect(r);
}

void letter(QPainter &g, int size, const QRectF &r, const QString &ch, const QColor &c) {
    // The same letters the buttons and the status line use. Not language
    // neutral, and deliberately so: A and B are what this application calls the
    // two revisions everywhere else.
    QFont f = QGuiApplication::font();
    f.setBold(true);
    f.setPixelSize(qMax(6, int(size * 0.5)));
    g.setFont(f);
    g.setPen(c);
    g.drawText(r, Qt::AlignCenter, ch);
}

QIcon build(const std::function<void(QPainter &, int)> &draw) {
    QIcon icon;
    for (int s : kSizes) {
        QPixmap px(s, s);
        px.fill(Qt::transparent);
        QPainter g(&px);
        g.setRenderHint(QPainter::Antialiasing, true);
        draw(g, s);
        g.end();
        icon.addPixmap(px);
    }
    return icon;
}

} // namespace

QIcon Icons::open(const Look &l) {
    return build([l](QPainter &g, int s) {
        const qreal p = penWidth(s);
        sheet(g, unitRect(s, 0.08, 0.10, 0.54, 0.70), l.ink, l.paper, p);
        sheet(g, unitRect(s, 0.36, 0.24, 0.54, 0.70), l.ink, l.paper, p);
    });
}

QIcon Icons::onlyA(const Look &l) {
    return build([l](QPainter &g, int s) {
        const QRectF r = unitRect(s, 0.16, 0.08, 0.68, 0.84);
        sheet(g, r, l.onlyA, l.paper, penWidth(s));
        letter(g, s, r, QStringLiteral("A"), l.onlyA);
    });
}

QIcon Icons::onlyB(const Look &l) {
    return build([l](QPainter &g, int s) {
        const QRectF r = unitRect(s, 0.16, 0.08, 0.68, 0.84);
        sheet(g, r, l.onlyB, l.paper, penWidth(s));
        letter(g, s, r, QStringLiteral("B"), l.onlyB);
    });
}

QIcon Icons::overlay(const Look &l) {
    // The composition rule itself, in three blocks: A's colour where only A
    // draws, B's where only B does, black where they agree. It is a picture of
    // what the button produces rather than a symbol standing in for it.
    return build([l](QPainter &g, int s) {
        const QRectF ra = unitRect(s, 0.08, 0.16, 0.52, 0.66);
        const QRectF rb = unitRect(s, 0.40, 0.16, 0.52, 0.66);
        g.setPen(Qt::NoPen);
        g.setBrush(l.onlyA);
        g.drawRect(ra);
        g.setBrush(l.onlyB);
        g.drawRect(rb);
        g.setBrush(l.ink);
        g.drawRect(ra.intersected(rb));
    });
}

QIcon Icons::sideBySide(const Look &l) {
    return build([l](QPainter &g, int s) {
        const qreal p = penWidth(s);
        sheet(g, unitRect(s, 0.06, 0.16, 0.38, 0.66), l.onlyA, l.paper, p);
        sheet(g, unitRect(s, 0.56, 0.16, 0.38, 0.66), l.onlyB, l.paper, p);
    });
}

QIcon Icons::singlePage(const Look &l) {
    // One sheet with its corner turned down: a page, rather than the run of
    // them the continuous scroll shows.
    return build([l](QPainter &g, int s) {
        const QRectF r = unitRect(s, 0.24, 0.08, 0.52, 0.84);
        const qreal fold = 0.20 * s;
        QPainterPath path;
        path.moveTo(r.left(), r.top());
        path.lineTo(r.right() - fold, r.top());
        path.lineTo(r.right(), r.top() + fold);
        path.lineTo(r.right(), r.bottom());
        path.lineTo(r.left(), r.bottom());
        path.closeSubpath();
        g.setPen(QPen(l.ink, penWidth(s)));
        g.setBrush(l.paper);
        g.drawPath(path);
        g.drawLine(QPointF(r.right() - fold, r.top()), QPointF(r.right() - fold, r.top() + fold));
        g.drawLine(QPointF(r.right() - fold, r.top() + fold), QPointF(r.right(), r.top() + fold));
    });
}

namespace {
void stepIcon(QPainter &g, int s, bool forward, const QColor &c) {
    // A chevron running into the bar it stops at: step to the next one, not
    // scroll a bit further.
    g.setPen(QPen(c, qMax(1.5, s / 9.0), Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    g.setBrush(Qt::NoBrush);
    const qreal x0 = (forward ? 0.26 : 0.68) * s;
    const qreal x1 = (forward ? 0.58 : 0.36) * s;
    QPainterPath path;
    path.moveTo(x0, 0.24 * s);
    path.lineTo(x1, 0.50 * s);
    path.lineTo(x0, 0.76 * s);
    g.drawPath(path);
    const qreal bar = (forward ? 0.76 : 0.18) * s;
    g.drawLine(QPointF(bar, 0.24 * s), QPointF(bar, 0.76 * s));
}
} // namespace

QIcon Icons::previousChange(const Look &l) {
    return build([l](QPainter &g, int s) { stepIcon(g, s, false, l.ink); });
}

QIcon Icons::nextChange(const Look &l) {
    return build([l](QPainter &g, int s) { stepIcon(g, s, true, l.ink); });
}

QIcon Icons::excludeRegion(const Look &l) {
    return build([l](QPainter &g, int s) {
        sheet(g, unitRect(s, 0.10, 0.10, 0.80, 0.80), l.ink, l.paper, penWidth(s));
        QPen dash(kMaskEdge, penWidth(s), Qt::DashLine);
        dash.setDashPattern({2, 2});
        g.setPen(dash);
        g.setBrush(Qt::NoBrush);
        g.drawRect(unitRect(s, 0.26, 0.30, 0.48, 0.38));
    });
}

QIcon Icons::fadeShared(const Look &l) {
    // Half the disc inked, half blank: the unchanged drawing on its way from
    // black to nothing.
    return build([l](QPainter &g, int s) {
        const QRectF r = unitRect(s, 0.12, 0.12, 0.76, 0.76);
        g.setPen(QPen(l.ink, penWidth(s)));
        g.setBrush(l.paper);
        g.drawEllipse(r);
        QPainterPath half;
        half.moveTo(r.center());
        half.arcTo(r, 90, 180);
        half.closeSubpath();
        g.setPen(Qt::NoPen);
        g.setBrush(l.ink);
        g.drawPath(half);
    });
}
