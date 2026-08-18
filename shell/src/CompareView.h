// The viewport: the comparison's virtual sheets, either as one continuous
// scroll or one sheet at a time.
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

    // Whether the viewport scrolls through the set at all.
    //
    // `Continuous` lays every sheet out in one column and scrolls through them,
    // which is right for reading a set the way it was drawn.
    //
    // `SinglePage` shows **one whole sheet and nothing else**: the sheet being
    // read, fitted to the viewport, with no scrolling anywhere. `PageUp`,
    // `PageDown` and the wheel step from sheet to sheet, `Home` and `End` go to
    // the ends of the set. It is the view for flipping through a set looking
    // for the sheet that changed, where scrolling is the thing in the way.
    //
    // Because "the whole sheet is on screen" is the entire definition, asking
    // for a closer look leaves it: any zoom, and any fit other than to the
    // page, puts the viewport back into the continuous scroll rather than
    // quietly becoming a single sheet you have to scroll around.
    //
    // Orthogonal to `Layout` and to the view mode. A reader can be on one sheet
    // at a time, side by side, at whichever tolerance — these are three
    // different questions and combining them into one list of choices is how a
    // control ends up switching off something unrelated.
    enum class Flow { Continuous, SinglePage };

    Layout layout() const { return m_layout_mode; }
    void setLayout(Layout l);

    Flow flow() const { return m_flow; }
    void setFlow(Flow f);

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

    // Arms the next drag to mark a region to exclude, so the reader can reach
    // the feature from a menu or a toolbar instead of having to know that
    // Ctrl+drag does it. One shot: it disarms as soon as a rectangle is drawn,
    // or when Escape is pressed.
    void armRegion();
    bool regionArmed() const { return m_armed; }

  signals:
    void currentPageChanged(int page);
    // The viewport left, or entered, the one-sheet flow. Emitted because it can
    // leave on its own: a zoom is a request the flow cannot honour.
    void flowChanged(bool singlePage);
    // Whether the next drag marks a region to exclude. The window says so.
    void regionArmedChanged(bool armed);
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
    void keyPressEvent(QKeyEvent *e) override;
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
    // Moves by whole sheets, for the keys and the wheel in the one-sheet flow.
    void stepSheet(int by);
    QPoint contentOrigin() const;
    const QImage &tileAt(int page, const QRect &tile, ScViewMode mode);
    // Content coordinates -> (page, point in page space)
    bool contentToPage(const QPoint &content, int *page, QPointF *pagePt) const;
    QRect pageRectToContent(int page, const QRectF &pageRect) const;

    Session *m_session = nullptr;
    double m_zoom = 1.0;
    Fit m_fit = Fit::Width;
    Layout m_layout_mode = Layout::Single;
    Flow m_flow = Flow::Continuous;
    // The sheet on show, and the only one laid out, when the flow is
    // `SinglePage`. Meaningless otherwise: `currentPage` works it out from the
    // scroll position there.
    int m_page = 1;
    QVector<Placed> m_layout;
    QSize m_content;

    // Tiles are keyed by page and grid position and thrown away wholesale
    // whenever anything that changes their content changes. A generation
    // counter rather than selective eviction: getting that wrong shows a stale
    // comparison, which is the one thing this tool must never do.
    QHash<quint64, QImage> m_tiles;
    QImage m_blank;

    void disarm();

    // Ctrl+drag, in content coordinates.
    bool m_dragging = false;
    // Armed from the menu: the next plain drag marks a region, no Ctrl needed.
    bool m_armed = false;
    QPoint m_dragStart;
    QPoint m_dragNow;

    // Wheel notches not yet spent on a sheet. A free-spinning wheel and a
    // touchpad both send fractions of a notch, and a sheet per fraction sends
    // an 85-sheet set past in a flick.
    int m_wheelSpin = 0;

    // Grabbing the sheet with the middle button and pulling it about. A drawing
    // at a zoom that makes a resistor value legible is several viewports wide,
    // and reaching the other end of it by two scrollbars is the slow way; the
    // left button is spoken for by the exclusion rectangle.
    bool m_panning = false;
    QPoint m_panFrom;   // where the grab started, in viewport coordinates
    QPoint m_panScroll; // the two scrollbars when it started
};
