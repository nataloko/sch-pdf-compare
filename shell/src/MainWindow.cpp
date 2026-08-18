// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include "CompareView.h"
#include "Session.h"

#include <QApplication>
#include <QDockWidget>
#include <QFileDialog>
#include <QFileInfo>
#include <QLabel>
#include <QMenuBar>
#include <QMessageBox>
#include <QStatusBar>
#include <QTreeWidget>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    setWindowTitle(tr("sch-pdf-compare"));
    resize(1400, 950);

    m_view = new CompareView(this);
    setCentralWidget(m_view);
    connect(m_view, &CompareView::currentPageChanged, this, &MainWindow::onCurrentPageChanged);
    connect(m_view, &CompareView::regionSelected, this, &MainWindow::onRegionSelected);
    connect(m_view, &CompareView::zoomChanged, this, [this](double) { updateStatus(); });

    auto *dock = new QDockWidget(tr("Changed sheets"), this);
    dock->setAllowedAreas(Qt::LeftDockWidgetArea | Qt::RightDockWidgetArea);
    m_sheets = new QTreeWidget(dock);
    m_sheets->setObjectName(QStringLiteral("sheets"));
    m_sheets->setHeaderLabels({tr("Sheet"), tr("Changes")});
    m_sheets->setRootIsDecorated(false);
    m_sheets->setColumnWidth(0, 160);
    dock->setWidget(m_sheets);
    addDockWidget(Qt::LeftDockWidgetArea, dock);
    connect(m_sheets, &QTreeWidget::itemActivated, this, [this](QTreeWidgetItem *i, int) {
        if (i) {
            m_view->goToPage(i->data(0, Qt::UserRole).toInt());
        }
    });
    connect(m_sheets, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem *i, int) {
        if (i) {
            m_view->goToPage(i->data(0, Qt::UserRole).toInt());
        }
    });

    m_status = new QLabel(this);
    m_status->setObjectName(QStringLiteral("status"));
    statusBar()->addWidget(m_status);

    buildMenus();
    updateStatus();
}

void MainWindow::buildMenus() {
    QMenu *file = menuBar()->addMenu(tr("&File"));
    QAction *open = file->addAction(tr("&Compare Two Files…"), this, &MainWindow::chooseAndOpen);
    open->setObjectName(QStringLiteral("open"));
    open->setShortcut(QKeySequence::Open);
    file->addSeparator();
    QAction *quit = file->addAction(tr("&Quit"), qApp, &QApplication::quit);
    quit->setShortcut(QKeySequence::Quit);

    QMenu *view = menuBar()->addMenu(tr("&View"));
    // Bare 1/2/3 rather than a modifier: switching what you are looking at is
    // the most frequent thing a reader does here, and `Tab` next to them makes
    // the pair a blink comparator.
    QAction *overlay = view->addAction(tr("&Overlay"), this, [this] { setViewMode(0); });
    overlay->setObjectName(QStringLiteral("overlay"));
    overlay->setShortcut(Qt::Key_3);
    QAction *onlyA = view->addAction(tr("Only &A"), this, [this] { setViewMode(1); });
    onlyA->setObjectName(QStringLiteral("onlyA"));
    onlyA->setShortcut(Qt::Key_1);
    QAction *onlyB = view->addAction(tr("Only &B"), this, [this] { setViewMode(2); });
    onlyB->setObjectName(QStringLiteral("onlyB"));
    onlyB->setShortcut(Qt::Key_2);
    QAction *flip = view->addAction(tr("&Flip A / B"), this, &MainWindow::blink);
    flip->setObjectName(QStringLiteral("flip"));
    flip->setShortcut(Qt::Key_Tab);
    view->addSeparator();
    QAction *fw = view->addAction(tr("Fit &Width"), this,
                                  [this] { m_view->setFit(CompareView::Fit::Width); });
    fw->setShortcut(Qt::CTRL | Qt::Key_0);
    view->addAction(tr("Fit &Page"), this, [this] { m_view->setFit(CompareView::Fit::Page); });
    QAction *zi = view->addAction(tr("Zoom &In"), this,
                                  [this] { m_view->setZoom(m_view->zoom() * 1.25); });
    zi->setShortcut(QKeySequence::ZoomIn);
    QAction *zo = view->addAction(tr("Zoom &Out"), this,
                                  [this] { m_view->setZoom(m_view->zoom() / 1.25); });
    zo->setShortcut(QKeySequence::ZoomOut);

    QMenu *cmp = menuBar()->addMenu(tr("&Compare"));
    QAction *next = cmp->addAction(tr("&Next Change"), this, [this] { stepChange(1); });
    next->setObjectName(QStringLiteral("next"));
    next->setShortcut(Qt::CTRL | Qt::Key_Period);
    QAction *prev = cmp->addAction(tr("&Previous Change"), this, [this] { stepChange(-1); });
    prev->setObjectName(QStringLiteral("prev"));
    prev->setShortcut(Qt::CTRL | Qt::Key_Comma);
    cmp->addSeparator();
    cmp->addAction(tr("Scan &Every Sheet"), this, &MainWindow::scanEverySheet)
        ->setObjectName(QStringLiteral("scanAll"));
    m_acceptSuggestions =
        cmp->addAction(tr("Exclude &Suggested Regions"), this, &MainWindow::acceptSuggestions);
    m_acceptSuggestions->setObjectName(QStringLiteral("acceptSuggestions"));
    m_acceptSuggestions->setEnabled(false);
    cmp->addSeparator();
    QAction *sr = cmp->addAction(tr("Shift Pairing &Right"), this, [this] { nudgePairing(1); });
    sr->setObjectName(QStringLiteral("shiftRight"));
    sr->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Right);
    QAction *sl = cmp->addAction(tr("Shift Pairing &Left"), this, [this] { nudgePairing(-1); });
    sl->setObjectName(QStringLiteral("shiftLeft"));
    sl->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Left);
    QAction *s0 = cmp->addAction(tr("Reset Pairing"), this, [this] {
        if (m_session) {
            m_session->setPageDelta(0);
        }
    });
    s0->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Home);
    cmp->addSeparator();
    QAction *tp = cmp->addAction(tr("More &Tolerance"), this, [this] { nudgeTolerance(1); });
    tp->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Plus);
    QAction *tm = cmp->addAction(tr("Less Tolerance"), this, [this] { nudgeTolerance(-1); });
    tm->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Minus);
    cmp->addSeparator();
    QAction *clr = cmp->addAction(tr("&Clear Excluded Regions"), this, [this] {
        if (m_session) {
            m_session->clearIgnoreRects();
        }
    });
    clr->setObjectName(QStringLiteral("clearRegions"));
    clr->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_C);
}

bool MainWindow::openPair(const QString &pathA, const QString &pathB) {
    QString error;
    Session *s = Session::open(pathA, pathB, &error, this);
    if (!s) {
        QMessageBox::warning(this, tr("Cannot compare these files"), error);
        return false;
    }
    delete m_session;
    m_session = s;
    connect(m_session, &Session::invalidated, this, [this] {
        m_view->invalidate();
        rebuildSheetList();
        updateStatus();
    });
    connect(m_session, &Session::sweepProgressed, this, &MainWindow::onSweepProgressed);
    m_view->setSession(m_session);
    setWindowTitle(tr("%1 vs %2 — sch-pdf-compare")
                       .arg(QFileInfo(pathA).fileName(), QFileInfo(pathB).fileName()));
    m_atSheet = 0;
    m_atIndex = -1;
    m_acceptSuggestions->setEnabled(false);
    rebuildSheetList();
    updateStatus();
    // Nobody opens a comparison to look at sheet 1; they want to know which
    // sheets changed. Start finding out immediately.
    m_session->startSweep();
    return true;
}

void MainWindow::chooseAndOpen() {
    const QString a = QFileDialog::getOpenFileName(this, tr("The earlier revision"), QString(),
                                                   tr("PDF files (*.pdf)"));
    if (a.isEmpty()) {
        return;
    }
    const QString b = QFileDialog::getOpenFileName(this, tr("The later revision"),
                                                   QFileInfo(a).absolutePath(),
                                                   tr("PDF files (*.pdf)"));
    if (b.isEmpty()) {
        return;
    }
    openPair(a, b);
}

void MainWindow::setViewMode(int mode) {
    if (m_session) {
        m_session->setViewMode(ScViewMode(mode));
        updateStatus();
    }
}

void MainWindow::blink() {
    if (!m_session) {
        return;
    }
    // Straight from the overlay, `Tab` shows A and remembers to come back to the
    // overlay rather than to B. Zoom and scroll never move, which is the whole
    // point: the eye catches what jumps.
    switch (m_session->viewMode()) {
    case SC_VIEW_MODE_ONLY_A:
        m_session->setViewMode(SC_VIEW_MODE_ONLY_B);
        break;
    case SC_VIEW_MODE_ONLY_B:
        m_session->setViewMode(m_blinkFrom == 0 ? SC_VIEW_MODE_OVERLAY : SC_VIEW_MODE_ONLY_A);
        break;
    default:
        m_blinkFrom = 0;
        m_session->setViewMode(SC_VIEW_MODE_ONLY_A);
        break;
    }
    updateStatus();
}

void MainWindow::nudgePairing(int by) {
    if (m_session) {
        m_session->setPageDelta(m_session->pageDelta() + by);
    }
}

void MainWindow::nudgeTolerance(int by) {
    if (m_session) {
        m_session->setTolerance(m_session->tolerance() + by);
    }
}

void MainWindow::onCurrentPageChanged(int page) {
    if (m_session && page >= 1 && m_session->changeCount(page) < 0) {
        m_session->scanPage(page);
        rebuildSheetList();
    }
    updateStatus();
}

void MainWindow::onRegionSelected(int page, const QRectF &r) {
    if (!m_session) {
        return;
    }
    Q_UNUSED(page);
    // Excluded regions apply to every sheet, which is what a shared title block
    // needs — and why the sheet the rectangle was drawn on does not matter.
    m_session->addIgnoreRect(r);
}

void MainWindow::scanEverySheet() {
    if (m_session) {
        m_session->startSweep();
        updateStatus();
    }
}

void MainWindow::onSweepProgressed() {
    rebuildSheetList();
    const ScSweepStatus st = m_session ? m_session->sweepStatus() : ScSweepStatus{};
    // Only offer once the sweep has actually reached the end. A suggestion from
    // half a set is an offer to hide whatever was scanned first.
    m_acceptSuggestions->setEnabled(st.finished && st.suggested > 0);
    updateStatus();
}

void MainWindow::acceptSuggestions() {
    if (!m_session) {
        return;
    }
    const QVector<QRectF> offered = m_session->suggestedRegions();
    if (offered.isEmpty()) {
        return;
    }
    // Say what is about to stop being compared, and let the reader decline. The
    // whole reason this is a suggestion and not a rule is that a net renamed
    // across every sheet looks exactly like a title-block date that moved.
    const auto answer = QMessageBox::question(
        this, tr("Exclude repeating regions?"),
        tr("%1 change on most sheets, which usually means a title block. "
           "Excluding them stops them being compared on every sheet.\n\n"
           "If a net was renamed across the whole set it would look the same, so "
           "check before accepting.")
            .arg(offered.size() == 1 ? tr("One region") 
                                     : tr("%1 regions").arg(offered.size())),
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
    if (answer != QMessageBox::Yes) {
        return;
    }
    applySuggestions();
}

void MainWindow::applySuggestions() {
    if (!m_session) {
        return;
    }
    const QVector<QRectF> offered = m_session->suggestedRegions();
    for (const QRectF &r : offered) {
        m_session->addIgnoreRect(r);
    }
    // The exclusions changed every answer, so the sweep starts again.
    m_session->startSweep();
}

void MainWindow::rebuildSheetList() {
    m_sheets->clear();
    if (!m_session) {
        return;
    }
    for (int p = 1; p <= m_session->pageCount(); p++) {
        const int n = m_session->changeCount(p);
        if (n <= 0) {
            continue; // unscanned (-1) or clean (0)
        }
        auto *item = new QTreeWidgetItem(m_sheets);
        const QPair<int, int> pr = m_session->pair(p);
        QString label = tr("Sheet %1").arg(p);
        if (pr.first == 0) {
            label += tr(" (added)");
        } else if (pr.second == 0) {
            label += tr(" (removed)");
        }
        item->setText(0, label);
        item->setText(1, QString::number(n));
        item->setData(0, Qt::UserRole, p);
    }
}

void MainWindow::stepChange(int direction) {
    if (!m_session) {
        return;
    }
    const int n = m_session->pageCount();
    int sheet = m_atSheet >= 1 ? m_atSheet : m_view->currentPage();
    int index = m_atIndex;

    for (int guard = 0; guard <= n + 1; guard++) {
        if (m_session->changeCount(sheet) < 0) {
            m_session->scanPage(sheet);
            rebuildSheetList();
        }
        const int count = qMax(0, m_session->changeCount(sheet));
        const int next = index + direction;
        if (next >= 0 && next < count) {
            m_atSheet = sheet;
            m_atIndex = next;
            m_view->showRect(sheet, m_session->change(sheet, next));
            updateStatus();
            return;
        }
        // Off the end of this sheet: carry on to the next one in reading order.
        sheet += direction;
        if (sheet < 1 || sheet > n) {
            updateStatus();
            return;
        }
        if (m_session->changeCount(sheet) < 0) {
            m_session->scanPage(sheet);
        }
        index = direction > 0 ? -1 : qMax(0, m_session->changeCount(sheet));
    }
}

void MainWindow::updateStatus() {
    if (!m_session) {
        m_status->setText(tr("Open two revisions of a schematic to compare them."));
        return;
    }
    const int page = m_view->currentPage();
    const int n = m_session->changeCount(page);
    const int ignored = m_session->ignoredCount(page);

    QString mode;
    switch (m_session->viewMode()) {
    case SC_VIEW_MODE_ONLY_A:
        mode = tr("A only");
        break;
    case SC_VIEW_MODE_ONLY_B:
        mode = tr("B only");
        break;
    default:
        mode = tr("Overlay");
        break;
    }

    QString text = tr("Sheet %1 of %2   %3   %4%   tolerance %5")
                       .arg(page)
                       .arg(m_session->pageCount())
                       .arg(mode)
                       .arg(int(m_view->zoom() * 100))
                       .arg(m_session->tolerance());
    if (m_session->pageDelta() != 0) {
        text += tr("   pairing %1%2").arg(m_session->pageDelta() > 0 ? "+" : "").arg(
            m_session->pageDelta());
    }
    const ScSweepStatus sw = m_session->sweepStatus();
    if (sw.running) {
        text += tr("   scanning %1 of %2").arg(sw.scanned).arg(sw.total);
    } else if (sw.finished) {
        text += QStringLiteral("   ") + (sw.changed_sheets == 1
                                             ? tr("1 sheet changed")
                                             : tr("%1 sheets changed").arg(sw.changed_sheets));
        if (sw.suggested > 0) {
            text += QStringLiteral("   ") + (sw.suggested == 1
                                                 ? tr("1 region repeats")
                                                 : tr("%1 regions repeat").arg(sw.suggested));
        }
    }
    if (n < 0) {
        text += tr("   not scanned");
    } else {
        text += QStringLiteral("   ") +
                (n == 1 ? tr("1 change here") : tr("%1 changes here").arg(n));
    }
    if (ignored > 0) {
        text += QStringLiteral("   ") + tr("%1 excluded").arg(ignored);
    }
    const int rects = m_session->ignoreRects().size();
    if (rects > 0) {
        text += QStringLiteral("   ") + (rects == 1
                                             ? tr("1 excluded region")
                                             : tr("%1 excluded regions").arg(rects));
    }
    if (m_atIndex >= 0) {
        text += tr("   at change %1 of sheet %2").arg(m_atIndex + 1).arg(m_atSheet);
    }
    m_status->setText(text);
}
