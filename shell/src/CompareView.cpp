// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "CompareView.h"

#include "Session.h"

#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QScrollBar>
#include <QWheelEvent>
#include <cmath>

namespace {
// Big enough that a viewport is a dozen of them, small enough that one costs
// about 8 ms to compose — measured, and the reason this can render on demand in
// paintEvent rather than needing a background thread yet.
constexpr int kTile = 512;
// Space between sheets, and around the whole run of them.
constexpr int kGap = 16;
constexpr double kMinZoom = 0.05;
constexpr double kMaxZoom = 12.0;

quint64 tileKey(int page, int tx, int ty) {
    return (quint64(quint32(page)) << 40) ^ (quint64(quint32(tx)) << 20) ^ quint64(quint32(ty));
}
} // namespace

CompareView::CompareView(QWidget *parent) : QAbstractScrollArea(parent) {
    viewport()->setBackgroundRole(QPalette::Dark);
    setFocusPolicy(Qt::StrongFocus);
    horizontalScrollBar()->setSingleStep(32);
    verticalScrollBar()->setSingleStep(32);
}

void CompareView::setSession(Session *s) {
    m_session = s;
    m_tiles.clear();
    m_page = 1;
    m_fit = Fit::Width;
    applyFit();
    verticalScrollBar()->setValue(0);
    emit currentPageChanged(currentPage());
}

void CompareView::armRegion() {
    if (!m_session || m_armed) {
        return;
    }
    m_armed = true;
    viewport()->setCursor(Qt::CrossCursor);
    setFocus(Qt::OtherFocusReason);
    emit regionArmedChanged(true);
}

void CompareView::disarm() {
    if (!m_armed) {
        return;
    }
    m_armed = false;
    viewport()->unsetCursor();
    emit regionArmedChanged(false);
}

void CompareView::invalidate() {
    m_tiles.clear();
    // The layout, not just the tiles. Each placed sheet carries which document
    // it shows and how big it is, and both are settled here: switching to
    // A-only changes the first, shifting the pairing changes the page count and
    // so the second. Repainting without rebuilding it drew the old answer from
    // a correctly emptied cache — the status line said "A only" over an overlay,
    // for as long as this method did not do this.
    if (m_fit != Fit::None) {
        applyFit();
    } else {
        relayout();
    }
    viewport()->update();
}

void CompareView::relayout() {
    m_layout.clear();
    m_content = QSize();
    if (!m_session) {
        horizontalScrollBar()->setRange(0, 0);
        verticalScrollBar()->setRange(0, 0);
        return;
    }
    const int n = m_session->pageCount();
    // Which sheets get placed at all. One sheet at a time is not a scroll that
    // stops early: the other sheets are not laid out, so nothing can scroll on
    // to them and a fit to the page is a fit to this page.
    int first = 1;
    int last = n;
    if (m_flow == Flow::SinglePage) {
        m_page = qBound(1, m_page, qMax(1, n));
        first = m_page;
        last = m_page;
    }
    int widest = 0;
    QVector<QSize> sizes;
    sizes.reserve(last - first + 1);
    for (int p = first; p <= last; p++) {
        const QSize s = m_session->pageDeviceSize(p, m_zoom);
        sizes.append(s);
        widest = qMax(widest, s.width());
    }
    const bool twoUp = m_layout_mode == Layout::SideBySide;
    // Side by side needs room for two of the widest sheet and a gutter between.
    const int band = twoUp ? widest * 2 + kGap : widest;
    int y = kGap;
    m_layout.reserve(twoUp ? sizes.size() * 2 : sizes.size());
    const ScViewMode single = m_session ? m_session->viewMode() : SC_VIEW_MODE_OVERLAY;
    for (int i = 0; i < sizes.size(); i++) {
        const int page = first + i;
        const QSize s = sizes[i];
        // Sheets are centred on the widest one, so a set whose pages differ in
        // size does not jitter left and right as it scrolls.
        if (twoUp) {
            const int lx = kGap + (widest - s.width()) / 2;
            const int rx = kGap + widest + kGap + (widest - s.width()) / 2;
            m_layout.append({page, QRect(QPoint(lx, y), s), SC_VIEW_MODE_ONLY_A});
            m_layout.append({page, QRect(QPoint(rx, y), s), SC_VIEW_MODE_ONLY_B});
        } else {
            const int x = kGap + (widest - s.width()) / 2;
            m_layout.append({page, QRect(QPoint(x, y), s), single});
        }
        y += s.height() + kGap;
    }
    m_content = QSize(band + 2 * kGap, y);

    const QSize vp = viewport()->size();
    horizontalScrollBar()->setRange(0, qMax(0, m_content.width() - vp.width()));
    horizontalScrollBar()->setPageStep(vp.width());
    verticalScrollBar()->setRange(0, qMax(0, m_content.height() - vp.height()));
    verticalScrollBar()->setPageStep(vp.height());
}

void CompareView::applyFit() {
    if (!m_session || m_session->pageCount() < 1) {
        relayout();
        return;
    }
    if (m_fit == Fit::None) {
        relayout();
        return;
    }
    const QSizeF pt = m_session->pageSize(qMax(1, currentPage()));
    if (pt.isEmpty()) {
        relayout();
        return;
    }
    const QSize vp = viewport()->size();
    const bool twoUp = m_layout_mode == Layout::SideBySide;
    // Two sheets and a gutter have to fit across, or fitting the width would
    // put half the comparison off screen.
    const double usableW = qMax(1, vp.width() - (twoUp ? 3 : 2) * kGap) / (twoUp ? 2.0 : 1.0);
    const double usableH = qMax(1, vp.height() - 2 * kGap);
    double z = usableW / pt.width();
    if (m_fit == Fit::Page) {
        z = qMin(z, usableH / pt.height());
    }
    m_zoom = qBound(kMinZoom, z, kMaxZoom);
    m_tiles.clear();
    relayout();
    emit zoomChanged(m_zoom);
}

void CompareView::setZoom(double z, const QPoint &anchor) {
    z = qBound(kMinZoom, z, kMaxZoom);
    if (qFuzzyCompare(z, m_zoom)) {
        return;
    }
    // Zoom about a point: keep whatever is under `anchor` under it afterwards.
    // Without this the page slides away from wherever the reader was looking,
    // which at 400% means losing the component they were examining.
    const QPoint a = anchor.x() >= 0 ? anchor : QPoint(viewport()->width() / 2, viewport()->height() / 2);
    const QPoint before = a + QPoint(horizontalScrollBar()->value(), verticalScrollBar()->value());
    const double ratio = z / m_zoom;

    m_fit = Fit::None;
    m_zoom = z;
    m_tiles.clear();
    relayout();

    const QPoint after(int(std::lround(before.x() * ratio)), int(std::lround(before.y() * ratio)));
    horizontalScrollBar()->setValue(after.x() - a.x());
    verticalScrollBar()->setValue(after.y() - a.y());
    viewport()->update();
    emit zoomChanged(m_zoom);
    emit currentPageChanged(currentPage());
}

void CompareView::setLayout(Layout l) {
    if (l == m_layout_mode) {
        return;
    }
    m_layout_mode = l;
    m_tiles.clear();
    // The sheets are a different size on screen now, so a fit has to be redone
    // rather than kept; an explicit zoom is the reader's and stays.
    if (m_fit != Fit::None) {
        applyFit();
    } else {
        relayout();
    }
    viewport()->update();
    emit currentPageChanged(currentPage());
}

void CompareView::setFlow(Flow f) {
    if (f == m_flow) {
        return;
    }
    // Whichever sheet the reader was looking at is the one they keep. Read out
    // before the flow changes, because `currentPage` answers differently on
    // either side of it.
    const int was = currentPage();
    m_flow = f;
    m_page = qMax(1, was);
    if (m_fit != Fit::None) {
        applyFit();
    } else {
        relayout();
    }
    if (f == Flow::SinglePage) {
        verticalScrollBar()->setValue(0);
    } else {
        goToPage(m_page);
    }
    viewport()->update();
    emit currentPageChanged(currentPage());
}

void CompareView::setFit(Fit f) {
    m_fit = f;
    applyFit();
    viewport()->update();
}

QPoint CompareView::contentOrigin() const {
    // When the content is smaller than the viewport it is centred rather than
    // pinned to a corner; a single A4 sheet against the top-left of a wide
    // window looks like a bug. Vertically this only ever applies to a sheet
    // with nothing above or below it — which is precisely the single-page
    // flow, where the alternative was a band of grey under the sheet that read
    // as a layout that had gone wrong.
    const int extraX = viewport()->width() - m_content.width();
    const int extraY = viewport()->height() - m_content.height();
    const int x = extraX > 0 ? -extraX / 2 : horizontalScrollBar()->value();
    const int y = extraY > 0 ? -extraY / 2 : verticalScrollBar()->value();
    return QPoint(x, y);
}

int CompareView::currentPage() const {
    if (m_layout.isEmpty()) {
        return 0;
    }
    if (m_flow == Flow::SinglePage) {
        return m_page;
    }
    const QPoint o = contentOrigin();
    const QRect vis(o, viewport()->size());
    int best = m_layout.first().page;
    int bestArea = -1;
    for (const Placed &p : m_layout) {
        const QRect i = p.rect.intersected(vis);
        const int area = i.width() * i.height();
        if (area > bestArea) {
            bestArea = area;
            best = p.page;
        }
    }
    return best;
}

void CompareView::goToPage(int page) {
    if (m_flow == Flow::SinglePage) {
        const int n = m_session ? m_session->pageCount() : 0;
        page = qBound(1, page, qMax(1, n));
        if (page != m_page) {
            m_page = page;
            // The sheets can differ in size, so a fit belongs to the sheet it
            // was worked out for. The tiles are keyed by page and survive.
            if (m_fit != Fit::None) {
                applyFit();
            } else {
                relayout();
            }
        }
        verticalScrollBar()->setValue(0);
        viewport()->update();
        emit currentPageChanged(currentPage());
        return;
    }
    for (const Placed &p : m_layout) {
        if (p.page == page) {
            verticalScrollBar()->setValue(p.rect.top() - kGap);
            viewport()->update();
            emit currentPageChanged(currentPage());
            return;
        }
    }
}

QRect CompareView::pageRectToContent(int page, const QRectF &pageRect) const {
    for (const Placed &p : m_layout) {
        // The first entry for a sheet is the earlier revision, which is where a
        // change is anchored; side by side leaves the later one alongside it.
        if (p.page != page) {
            continue;
        }
        const QRect r(int(pageRect.x() * m_zoom), int(pageRect.y() * m_zoom),
                      qMax(1, int(std::ceil(pageRect.width() * m_zoom))),
                      qMax(1, int(std::ceil(pageRect.height() * m_zoom))));
        return r.translated(p.rect.topLeft());
    }
    return {};
}

void CompareView::showRect(int page, const QRectF &pageRect) {
    // Stepping through the changes crosses sheets, and one sheet at a time
    // means the next one is not laid out yet. Without this, `Ctrl+.` walked off
    // the end of the sheet and then did nothing at all — the change was found,
    // there was simply nowhere on screen to point at.
    if (m_flow == Flow::SinglePage && page != m_page) {
        goToPage(page);
    }
    const QRect target = pageRectToContent(page, pageRect);
    if (target.isNull()) {
        return;
    }
    // Centre it if it is off screen, and leave the scroll alone if it is not.
    // Snapping to a change that was already visible is disorienting when the
    // reader is stepping through a list of them.
    const QRect vis(contentOrigin(), viewport()->size());
    const QRect margin = target.adjusted(-kGap, -kGap, kGap, kGap);
    if (!vis.contains(margin)) {
        horizontalScrollBar()->setValue(target.center().x() - viewport()->width() / 2);
        verticalScrollBar()->setValue(target.center().y() - viewport()->height() / 2);
    }
    viewport()->update();
    emit currentPageChanged(currentPage());
}

const QImage &CompareView::tileAt(int page, const QRect &tile, ScViewMode mode) {
    // The mode is part of the key: side by side keeps both documents' tiles for
    // the same sheet at the same place, and without it they overwrite each
    // other and the two panes show the same picture.
    const quint64 key =
        tileKey(page, tile.x() / kTile, tile.y() / kTile) ^ (quint64(mode) << 60);
    auto it = m_tiles.find(key);
    if (it != m_tiles.end()) {
        return it.value();
    }
    QImage img = m_session ? m_session->tile(page, m_zoom, tile, mode) : QImage();
    if (img.isNull()) {
        if (m_blank.size() != tile.size()) {
            m_blank = QImage(tile.size(), QImage::Format_RGB32);
            m_blank.fill(Qt::white);
        }
        return m_blank;
    }
    return *m_tiles.insert(key, img);
}

void CompareView::paintEvent(QPaintEvent *e) {
    QPainter g(viewport());
    g.fillRect(e->rect(), palette().dark());
    if (!m_session || m_layout.isEmpty()) {
        // An empty grey rectangle looks like a comparison that found nothing.
        // Say which it is, how to start one, and — because the first thing the
        // first reader did was click the switched-off buttons and report them
        // broken — that the rest of the window waits on this.
        QFont f = font();
        f.setPointSizeF(f.pointSizeF() * 1.3);
        g.setFont(f);
        g.setPen(palette().brightText().color());
        g.drawText(viewport()->rect(), Qt::AlignCenter,
                   tr("Open two revisions of a drawing to compare them.\n\n"
                      "File \u25b8 Compare Two Files\u2026   (Ctrl+O)\n\n"
                      "Everything else stays switched off until then."));
        return;
    }
    const QPoint o = contentOrigin();
    const QRect vis(o, viewport()->size());

    for (const Placed &p : m_layout) {
        const QRect onScreen = p.rect.intersected(vis);
        if (onScreen.isEmpty()) {
            continue;
        }
        // The part of this sheet that needs drawing, in the sheet's own device
        // pixels.
        const QRect want = onScreen.translated(-p.rect.topLeft());
        const int tx0 = want.left() / kTile;
        const int ty0 = want.top() / kTile;
        const int tx1 = want.right() / kTile;
        const int ty1 = want.bottom() / kTile;
        for (int ty = ty0; ty <= ty1; ty++) {
            for (int tx = tx0; tx <= tx1; tx++) {
                QRect tile(tx * kTile, ty * kTile, kTile, kTile);
                tile = tile.intersected(QRect(QPoint(0, 0), p.rect.size()));
                if (tile.isEmpty()) {
                    continue;
                }
                const QImage &img = tileAt(p.page, tile, p.mode);
                g.drawImage(p.rect.topLeft() + tile.topLeft() - o, img);
            }
        }
        g.setPen(QPen(palette().shadow().color(), 1));
        g.drawRect(p.rect.translated(-o).adjusted(-1, -1, 0, 0));
    }

    if (m_dragging) {
        const QRect r = QRect(m_dragStart, m_dragNow).normalized().translated(-o);
        g.setPen(QPen(QColor(0x60, 0x84, 0xb0), 1, Qt::DashLine));
        g.setBrush(QColor(0x60, 0x84, 0xb0, 40));
        g.drawRect(r);
    }
}

void CompareView::resizeEvent(QResizeEvent *e) {
    QAbstractScrollArea::resizeEvent(e);
    if (m_fit != Fit::None) {
        applyFit();
    } else {
        relayout();
    }
}

void CompareView::keyPressEvent(QKeyEvent *e) {
    if (m_armed && e->key() == Qt::Key_Escape) {
        disarm();
        e->accept();
        return;
    }
    // One sheet at a time still has to be readable with one hand. `PageDown`
    // scrolls down this sheet, and only once there is no more of it does it
    // step to the next — which is what every document reader does and what the
    // key is for. Coming back lands at the foot of the previous sheet, because
    // that is where the reader was.
    if (m_flow == Flow::SinglePage &&
        (e->key() == Qt::Key_PageDown || e->key() == Qt::Key_PageUp)) {
        const bool forward = e->key() == Qt::Key_PageDown;
        QScrollBar *sb = verticalScrollBar();
        const bool atEnd = forward ? sb->value() >= sb->maximum() : sb->value() <= sb->minimum();
        const int want = m_page + (forward ? 1 : -1);
        const int n = m_session ? m_session->pageCount() : 0;
        if (atEnd && want >= 1 && want <= n) {
            goToPage(want);
            if (!forward) {
                verticalScrollBar()->setValue(verticalScrollBar()->maximum());
            }
            e->accept();
            return;
        }
    }
    QAbstractScrollArea::keyPressEvent(e);
}

void CompareView::scrollContentsBy(int dx, int dy) {
    QAbstractScrollArea::scrollContentsBy(dx, dy);
    viewport()->update();
    emit currentPageChanged(currentPage());
}

bool CompareView::focusNextPrevChild(bool) {
    return false;
}

void CompareView::wheelEvent(QWheelEvent *e) {
    if (e->modifiers() & Qt::ControlModifier) {
        const double steps = e->angleDelta().y() / 120.0;
        setZoom(m_zoom * std::pow(1.2, steps), e->position().toPoint());
        e->accept();
        return;
    }
    QAbstractScrollArea::wheelEvent(e);
}

bool CompareView::contentToPage(const QPoint &content, int *page, QPointF *pagePt) const {
    for (const Placed &p : m_layout) {
        if (!p.rect.contains(content)) {
            continue;
        }
        const QPoint d = content - p.rect.topLeft();
        *page = p.page;
        *pagePt = QPointF(d.x() / m_zoom, d.y() / m_zoom);
        return true;
    }
    return false;
}

void CompareView::mousePressEvent(QMouseEvent *e) {
    if (e->button() == Qt::LeftButton && (m_armed || (e->modifiers() & Qt::ControlModifier))) {
        m_dragging = true;
        m_dragStart = e->pos() + contentOrigin();
        m_dragNow = m_dragStart;
        viewport()->update();
        return;
    }
    QAbstractScrollArea::mousePressEvent(e);
}

void CompareView::mouseMoveEvent(QMouseEvent *e) {
    if (m_dragging) {
        m_dragNow = e->pos() + contentOrigin();
        viewport()->update();
        return;
    }
    QAbstractScrollArea::mouseMoveEvent(e);
}

void CompareView::mouseReleaseEvent(QMouseEvent *e) {
    if (!m_dragging) {
        QAbstractScrollArea::mouseReleaseEvent(e);
        return;
    }
    m_dragging = false;
    const QRect r = QRect(m_dragStart, m_dragNow).normalized();
    viewport()->update();
    if (r.width() < 4 || r.height() < 4) {
        return; // a click, not a drag; still armed, so try again
    }
    disarm();
    int page = 0;
    QPointF a, b;
    if (contentToPage(r.topLeft(), &page, &a)) {
        int page2 = 0;
        if (!contentToPage(r.bottomRight(), &page2, &b) || page2 != page) {
            // A drag that ran off the sheet is clamped to it rather than
            // refused: the reader meant "this corner of every sheet".
            const QSizeF pt = m_session ? m_session->pageSize(page) : QSizeF();
            b = QPointF(qMin(qreal(pt.width()), r.right() / m_zoom),
                        qMin(qreal(pt.height()), r.bottom() / m_zoom));
        }
        emit regionSelected(page, QRectF(a, b).normalized());
    }
}
