// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include "CompareView.h"
#include "Session.h"

#include <QApplication>
#include <QDockWidget>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QLabel>
#include <QCloseEvent>
#include <QMenuBar>
#include <QPageLayout>
#include <QPainter>
#include <QPrintDialog>
#include <QPrinter>
#include <QProgressDialog>
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

    // What the sheet *says* differently, next to the list of where it *looks*
    // different. The two answer different questions and a reviewer wants both:
    // the overlay catches a re-routed wire that carries no text, this catches a
    // value that went from 10k to 12k and spells it out.
    auto *textDock = new QDockWidget(tr("Text on this sheet"), this);
    textDock->setAllowedAreas(Qt::LeftDockWidgetArea | Qt::RightDockWidgetArea);
    m_text = new QTreeWidget(textDock);
    m_text->setObjectName(QStringLiteral("textChanges"));
    m_text->setHeaderLabels({tr("Was"), tr("Is now")});
    m_text->setRootIsDecorated(false);
    m_text->setColumnWidth(0, 140);
    textDock->setWidget(m_text);
    addDockWidget(Qt::RightDockWidgetArea, textDock);
    connect(m_text, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem *i, int) {
        if (i && m_session) {
            m_view->showRect(m_view->currentPage(), i->data(0, Qt::UserRole).toRectF());
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
    QAction *reopen = file->addAction(tr("&Reopen Last Comparison"), this,
                                      &MainWindow::reopenLast);
    reopen->setObjectName(QStringLiteral("reopenLast"));
    file->addSeparator();
    QAction *rep = file->addAction(tr("&Export Change Report…"), this,
                                   &MainWindow::exportReport);
    rep->setObjectName(QStringLiteral("exportReport"));
    file->addSeparator();
    QAction *pr = file->addAction(tr("&Print…"), this, &MainWindow::printSheets);
    pr->setObjectName(QStringLiteral("print"));
    pr->setShortcut(QKeySequence::Print);
    QAction *prc = file->addAction(tr("Print &Changed Sheets…"), this,
                                   &MainWindow::printChangedSheets);
    prc->setObjectName(QStringLiteral("printChanged"));
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
    // A checkable pair rather than a third view mode: this changes how the
    // viewport is arranged, not what the core composes, and the A/B/overlay
    // choice still applies inside the single-sheet layout.
    QAction *sbs = view->addAction(tr("&Side by Side"));
    sbs->setObjectName(QStringLiteral("sideBySide"));
    sbs->setCheckable(true);
    sbs->setShortcut(Qt::Key_4);
    connect(sbs, &QAction::toggled, this, [this](bool on) {
        m_view->setLayout(on ? CompareView::Layout::SideBySide : CompareView::Layout::Single);
        updateStatus();
    });
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
    QAction *am = cmp->addAction(tr("&Match Sheets by Content"), this, &MainWindow::matchSheets);
    am->setObjectName(QStringLiteral("autoMatch"));
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
            persist();
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
    if (!m_forTesting) {
        // Whatever was worked out for this pair last time, before anything is
        // scanned against the wrong settings.
        m_session->loadSettings();
    }
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

void MainWindow::reopenLast() {
    QString a;
    QString b;
    if (!Session::lastPair(&a, &b)) {
        QMessageBox::information(this, tr("Nothing to reopen"),
                                 tr("No comparison has been saved yet."));
        return;
    }
    openPair(a, b);
}

void MainWindow::exportReport() {
    if (!m_session) {
        return;
    }
    const ScSweepStatus st = m_session->sweepStatus();
    if (!st.finished) {
        // Offered rather than refused: a report of the first few sheets is
        // sometimes exactly what somebody wants, and it says so in its own text.
        const auto go = QMessageBox::question(
            this, tr("The scan has not finished"),
            tr("Only %1 of %2 sheets have been scanned. A report written now "
               "covers those and says so.\n\nWrite it anyway?")
                .arg(st.scanned)
                .arg(st.total),
            QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
        if (go != QMessageBox::Yes) {
            return;
        }
    }
    const QString suggested =
        QFileInfo(m_session->pathB()).completeBaseName() + QStringLiteral(" changes.md");
    const QString path = QFileDialog::getSaveFileName(
        this, tr("Write the change report"),
        QFileInfo(m_session->pathA()).absolutePath() + QLatin1Char('/') + suggested,
        tr("Markdown (*.md);;All files (*)"));
    if (path.isEmpty()) {
        return;
    }
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Text)) {
        QMessageBox::warning(this, tr("Cannot write the report"), f.errorString());
        return;
    }
    const QByteArray text = m_session->report().toUtf8();
    if (f.write(text) != text.size() || !f.flush()) {
        QMessageBox::warning(this, tr("Cannot write the report"), f.errorString());
        return;
    }
    statusBar()->showMessage(tr("Report written to %1").arg(path), 5000);
}

QVector<int> MainWindow::changedSheets() const {
    QVector<int> out;
    if (!m_session) {
        return out;
    }
    for (int p = 1; p <= m_session->pageCount(); p++) {
        if (m_session->changeCount(p) > 0) {
            out.append(p);
        }
    }
    return out;
}

void MainWindow::printSheets() {
    if (!m_session) {
        return;
    }
    QVector<int> all;
    for (int p = 1; p <= m_session->pageCount(); p++) {
        all.append(p);
    }
    printRange(all);
}

void MainWindow::printChangedSheets() {
    if (!m_session) {
        return;
    }
    const QVector<int> sheets = changedSheets();
    if (sheets.isEmpty()) {
        QMessageBox::information(
            this, tr("Nothing to print"),
            m_session->sweepStatus().finished
                ? tr("No sheet has any changes on it.")
                : tr("No changes have been found yet. The scan is still running."));
        return;
    }
    printRange(sheets);
}

/// Prints the given virtual sheets as they are being viewed.
///
/// Deliberately not "print the two documents": what is worth putting on paper is
/// the comparison, in whichever view the reader has chosen, and their tolerance
/// and excluded regions with it.
void MainWindow::printRange(const QVector<int> &sheets) {
    if (!m_session || sheets.isEmpty()) {
        return;
    }
    QPrinter printer(QPrinter::HighResolution);
    printer.setDocName(QFileInfo(m_session->pathB()).completeBaseName());
    QPrintDialog dialog(&printer, this);
    dialog.setWindowTitle(tr("Print the comparison"));
    if (dialog.exec() != QDialog::Accepted) {
        return;
    }

    const int printed = printTo(printer, sheets);
    if (printed < 0) {
        QMessageBox::warning(this, tr("Cannot print"), tr("The printer would not start."));
        return;
    }
    statusBar()->showMessage(printed == 1 ? tr("Sent 1 sheet to the printer")
                                          : tr("Sent %1 sheets to the printer").arg(printed),
                             5000);
}

/// Paints the sheets onto a printer. Returns how many pages came out, or -1 if
/// the printer would not start.
int MainWindow::printTo(QPrinter &printer, const QVector<int> &sheets) {
    if (!m_session || sheets.isEmpty()) {
        return 0;
    }
    // Turn the paper to match the drawing. A schematic set is landscape and the
    // default page is portrait, which prints the sheet at two-thirds the size it
    // could be with a band of white above and below it. Set per sheet, because a
    // set can mix the two, and before the page starts — after it, the layout
    // applies to the next one.
    auto orientation = [this](int sheet) {
        const QSizeF pt = m_session->pageSize(sheet);
        return pt.width() >= pt.height() ? QPageLayout::Landscape : QPageLayout::Portrait;
    };
    const bool okOrient = printer.setPageOrientation(orientation(sheets.first()));
    if (qEnvironmentVariableIsSet("SC_DEBUG_PRINT")) {
        const QSizeF pt = m_session->pageSize(sheets.first());
        fprintf(stderr, "[print] sheet %d is %.0fx%.0f pt, orientation set=%d ok=%d\n",
                sheets.first(), pt.width(), pt.height(),
                int(printer.pageLayout().orientation()), int(okOrient));
    }

    QPainter g;
    if (!g.begin(&printer)) {
        return -1;
    }
    QProgressDialog progress(tr("Printing…"), tr("Stop"), 0, sheets.size(), this);
    progress.setWindowModality(Qt::WindowModal);
    // Shown only if it takes long enough to be worth interrupting; a two-sheet
    // print should not flash a dialog.
    progress.setMinimumDuration(1000);

    int done = 0;
    for (int i = 0; i < sheets.size(); i++) {
        progress.setValue(i);
        if (progress.wasCanceled()) {
            printer.abort();
            break;
        }
        if (done > 0) {
            printer.setPageOrientation(orientation(sheets[i]));
            if (!printer.newPage()) {
                break;
            }
        }
        paintSheetForPrint(g, printer, sheets[i]);
        done++;
    }
    progress.setValue(sheets.size());
    g.end();
    return done;
}

QVector<int> MainWindow::changedSheetList() const {
    return changedSheets();
}

/// Draws one sheet onto the printer page, with a caption saying what it is.
///
/// The caption is not decoration. A printed comparison gets passed around
/// without the application that made it, and it has to carry which two files it
/// is, which view, and — most of all — whether part of every sheet was excluded
/// from the comparison. A printout that quietly omitted that would be read as
/// "nothing changed there".
void MainWindow::paintSheetForPrint(QPainter &g, QPrinter &printer, int sheet) {
    const QRect page = printer.pageLayout().paintRectPixels(printer.resolution());
    const int dpi = qMax(72, printer.resolution());

    // A caption strip about a quarter of an inch tall, in two lines.
    QFont caption = g.font();
    caption.setPointSizeF(7.0);
    g.setFont(caption);
    const QFontMetrics fm(caption, &printer);
    const int captionH = fm.height() * 2 + dpi / 24;
    const QRect art(0, 0, page.width(), qMax(1, page.height() - captionH));

    const QSizeF pt = m_session->pageSize(sheet);
    if (pt.isEmpty() || art.isEmpty()) {
        return;
    }
    // Fit the sheet in the printable area, then cap the rendering resolution.
    // A full page at a laser printer's 1200 dpi is half a gigabyte of pixels;
    // 300 dpi is past what the eye resolves on paper and the printer's own
    // scaling covers the rest.
    const double fit = qMin(art.width() / pt.width(), art.height() / pt.height());
    const double zoom = qMin(fit, 300.0 / 72.0);
    const QSize deviceSize = m_session->pageDeviceSize(sheet, zoom);
    if (deviceSize.isEmpty()) {
        return;
    }
    const QImage img =
        m_session->tile(sheet, zoom, QRect(QPoint(0, 0), deviceSize), m_session->viewMode());
    if (img.isNull()) {
        return;
    }

    // Centred, aspect preserved.
    const QSize drawn = img.size().scaled(art.size(), Qt::KeepAspectRatio);
    const QRect target(art.x() + (art.width() - drawn.width()) / 2,
                       art.y() + (art.height() - drawn.height()) / 2, drawn.width(),
                       drawn.height());
    g.setRenderHint(QPainter::SmoothPixmapTransform, true);
    g.drawImage(target, img);

    // Printing always puts one sheet on a page in the session's own view; the
    // side-by-side arrangement is a property of the viewport, not of what gets
    // composed, so it has nothing to say here.
    QString mode;
    switch (m_session->viewMode()) {
    case SC_VIEW_MODE_ONLY_A:
        mode = tr("earlier revision only");
        break;
    case SC_VIEW_MODE_ONLY_B:
        mode = tr("later revision only");
        break;
    default:
        mode = tr("overlay: red was removed, green was added");
        break;
    }
    QString line1 = tr("Sheet %1 of %2 — %3 vs %4 — %5")
                        .arg(sheet)
                        .arg(m_session->pageCount())
                        .arg(QFileInfo(m_session->pathA()).fileName(),
                             QFileInfo(m_session->pathB()).fileName(), mode);
    QString line2 = tr("tolerance %1 px").arg(m_session->tolerance());
    const int n = m_session->changeCount(sheet);
    if (n >= 0) {
        line2 += QStringLiteral("   ·   ") +
                 (n == 1 ? tr("1 changed region on this sheet")
                         : tr("%1 changed regions on this sheet").arg(n));
    }
    const int excluded = m_session->ignoreRects().size();
    if (excluded > 0) {
        line2 += QStringLiteral("   ·   ") +
                 (excluded == 1 ? tr("1 region excluded from the comparison")
                                : tr("%1 regions excluded from the comparison").arg(excluded)) +
                 tr(" — anything inside was not compared");
    }

    g.setPen(Qt::black);
    const QRect strip(0, page.height() - captionH, page.width(), captionH);
    g.drawText(QRect(strip.x(), strip.y(), strip.width(), fm.height()),
               Qt::AlignLeft | Qt::AlignVCenter, line1);
    g.drawText(QRect(strip.x(), strip.y() + fm.height(), strip.width(), fm.height()),
               Qt::AlignLeft | Qt::AlignVCenter, line2);
}

void MainWindow::closeEvent(QCloseEvent *e) {
    persist();
    QMainWindow::closeEvent(e);
}

/// Writes the excluded regions and settings back, unless this run was told not
/// to. Called after anything worth keeping, and again on the way out, because a
/// reviewer who worked out a title block and then lost the window should not
/// have to do it twice.
void MainWindow::persist() {
    if (m_session && !m_forTesting) {
        m_session->saveSettings();
    }
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

void MainWindow::matchSheets() {
    if (!m_session) {
        return;
    }
    if (!m_session->autoMatch()) {
        QMessageBox::warning(this, tr("Cannot match these sheets"), m_session->lastError());
        return;
    }
    // The pairing changed, so every scanned answer was about different sheets.
    m_session->startSweep();
    updateStatus();
}

void MainWindow::nudgeTolerance(int by) {
    if (m_session) {
        m_session->setTolerance(m_session->tolerance() + by);
        persist();
    }
}

void MainWindow::onCurrentPageChanged(int page) {
    if (m_session && page >= 1 && m_session->changeCount(page) < 0) {
        m_session->scanPage(page);
        rebuildSheetList();
    }
    rebuildTextChanges(page);
    updateStatus();
}

void MainWindow::rebuildTextChanges(int page) {
    m_text->clear();
    if (!m_session || page < 1) {
        return;
    }
    for (const Session::TextChange &c : m_session->textChanges(page)) {
        auto *item = new QTreeWidgetItem(m_text);
        switch (c.kind) {
        case SC_TEXT_CHANGE_KIND_CHANGED:
            item->setText(0, c.before);
            item->setText(1, c.after);
            break;
        case SC_TEXT_CHANGE_KIND_REMOVED:
            item->setText(0, c.before);
            item->setText(1, tr("— removed"));
            break;
        case SC_TEXT_CHANGE_KIND_MOVED:
            item->setText(0, c.before);
            item->setText(1, tr("— moved"));
            break;
        default:
            item->setText(0, tr("— added"));
            item->setText(1, c.after);
            break;
        }
        item->setData(0, Qt::UserRole, c.rect);
    }
}

void MainWindow::onRegionSelected(int page, const QRectF &r) {
    if (!m_session) {
        return;
    }
    Q_UNUSED(page);
    // Excluded regions apply to every sheet, which is what a shared title block
    // needs — and why the sheet the rectangle was drawn on does not matter.
    m_session->addIgnoreRect(r);
    persist();
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
    persist();
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
    if (m_view->layout() == CompareView::Layout::SideBySide) {
        mode = tr("Side by side");
    } else {
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
    }

    QString text = tr("Sheet %1 of %2   %3   %4%   tolerance %5")
                       .arg(page)
                       .arg(m_session->pageCount())
                       .arg(mode)
                       .arg(int(m_view->zoom() * 100))
                       .arg(m_session->tolerance());
    if (m_session->pairingIsAutomatic()) {
        text += tr("   sheets matched by content");
    } else if (m_session->pageDelta() != 0) {
        text += tr("   pairing %1%2")
                    .arg(m_session->pageDelta() > 0 ? "+" : "")
                    .arg(m_session->pageDelta());
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
    // Ahead of the counts, because it says they cannot be trusted.
    if (m_session->sizeMismatch(page) == 1) {
        text += tr("   ⚠ the two sheets are different sizes — the counts below "
                   "are not reliable");
    }
    if (n < 0) {
        text += tr("   not scanned");
    } else {
        text += QStringLiteral("   ") +
                (n == 1 ? tr("1 change here") : tr("%1 changes here").arg(n));
    }
    const float covered = m_session->coverage(page);
    if (covered >= 0.25f) {
        // A count alone cannot separate a few edits from a sheet that differs
        // everywhere, because the clustering bridges neighbouring cells.
        text += QStringLiteral("   ") +
                tr("covering %1% of the sheet").arg(int(covered * 100.0f + 0.5f));
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
