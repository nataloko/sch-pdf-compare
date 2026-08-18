// The viewport: a continuous scroll through the comparison's virtual sheets.
//
// This is the part SumatraPDF used to provide. It lays every sheet out at the
// current zoom, draws whatever of them is on screen from a tile cache, and
// takes the reader to a change without disturbing the zoom they chose.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include "Session.h"

#include <QAbstractScrollArea>
#include <QHash>
#include <QImage>
#include <QRectF>
#include <QVector>

class Session;

class CompareView : public QAbstractScrollArea {
    Q_OBJECT

  public:
    explicit CompareView(QWidget *parent = nullptr);

    void setSession(Session *s);
    Session *session() const { return m_session; }

    enum class Fit { None, Width, Page };

    // How the sheets are arranged in the viewport.
    //
    // `Single` is one sheet at a time in whichever view the session is set to.
    // `SideBySide` puts the two revisions next to each other: the pair shares
    // one scroll and one zoom, so panning is synchronised by construction rather
    // than by keeping two viewports in step.
    //
    // It earns its place where the overlay is at its worst — text that changed
    // is drawn twice on top of itself and neither reading is legible.
    enum class Layout { Single, SideBySide };

    Layout layout() const { return m_layout_mode; }
    void setLayout(Layout l);

    double zoom() const { return m_zoom; }
    void setZoom(double z, const QPoint &anchor = QPoint(-1, -1));
    void setFit(Fit f);
    Fit fit() const { return m_fit; }

    // The sheet filling most of the viewport. 0 when there is nothing open.
    int currentPage() const;
    void goToPage(int page);
    // Scrolls until `pageRect` (page points) is on screen. Never changes zoom:
    // the reader picked it.
    void showRect(int page, const QRectF &pageRect);

    // Everything drawn is stale; drop the cache and repaint.
    void invalidate();

  signals:
    void currentPageChanged(int page);
    // The reader dragged out a rectangle with Ctrl held.
    void regionSelected(int page, const QRectF &pageRect);
    void zoomChanged(double zoom);

  protected:
    void paintEvent(QPaintEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;
    void wheelEvent(QWheelEvent *e) override;
    void mousePressEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void scrollContentsBy(int dx, int dy) override;
    // Tab is a comparison control here — the blink comparator — so it must not
    // be eaten by the focus chain before the shortcut sees it.
    bool focusNextPrevChild(bool next) override;

  private:
    struct Placed {
        int page = 0;
        QRect rect; // in content coordinates, device pixels at m_zoom
        // Which document this rectangle shows. In `Single` it is the session's
        // current view mode; side by side puts one of each.
        ScViewMode mode = SC_VIEW_MODE_OVERLAY;
    };

    void relayout();
    void applyFit();
    QPoint contentOrigin() const;
    const QImage &tileAt(int page, const QRect &tile, ScViewMode mode);
    // Content coordinates -> (page, point in page space)
    bool contentToPage(const QPoint &content, int *page, QPointF *pagePt) const;
    QRect pageRectToContent(int page, const QRectF &pageRect) const;

    Session *m_session = nullptr;
    double m_zoom = 1.0;
    Fit m_fit = Fit::Width;
    Layout m_layout_mode = Layout::Single;
    QVector<Placed> m_layout;
    QSize m_content;

    // Tiles are keyed by page and grid position and thrown away wholesale
    // whenever anything that changes their content changes. A generation
    // counter rather than selective eviction: getting that wrong shows a stale
    // comparison, which is the one thing this tool must never do.
    QHash<quint64, QImage> m_tiles;
    QImage m_blank;

    // Ctrl+drag, in content coordinates.
    bool m_dragging = false;
    QPoint m_dragStart;
    QPoint m_dragNow;
};
