// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include "ColourDialog.h"
#include "CompareView.h"
#include "Icons.h"
#include "Session.h"

#include <QActionGroup>
#include <QApplication>
#include <QDockWidget>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QCheckBox>
#include <QLabel>
#include <QCloseEvent>
#include <QMenu>
#include <QMenuBar>
#include <QPageLayout>
#include <QPainter>
#include <QPrintDialog>
#include <QPrinter>
#include <QProgressDialog>
#include <QSlider>
#include <QSpinBox>
#include <QTimer>
#include <QMessageBox>
#include <QStatusBar>
#include <QToolBar>
#include <QTreeWidget>
#include <QVBoxLayout>

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
    // Three columns, not two. A count on its own cannot separate a few edits
    // from a sheet that was redrawn, because the clustering bridges neighbouring
    // cells on purpose — so how much of the sheet the changes cover goes next to
    // it, and a sheet that cannot be compared at all says so instead.
    m_sheets->setHeaderLabels({tr("Sheet"), tr("Changes"), tr("Of the sheet")});
    m_sheets->setRootIsDecorated(false);
    m_sheets->setColumnWidth(0, 150);
    m_sheets->setColumnWidth(1, 70);
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

    // Text that only moved is kept out of the list by default and counted on the
    // checkbox instead. A sheet that was re-laid-out moves hundreds of identical
    // labels, and those rows bury the few that say something different — one
    // sample sheet has 354 moves against 10 real changes. The count is on show,
    // so nothing is hidden without saying so, which is the same bargain the
    // report makes.
    m_showMoved = new QCheckBox(tr("Include text that only moved"), textDock);
    m_showMoved->setObjectName(QStringLiteral("showMoved"));
    connect(m_showMoved, &QCheckBox::toggled, this,
            [this] { rebuildTextChanges(m_view->currentPage()); });

    auto *textPane = new QWidget(textDock);
    auto *textLayout = new QVBoxLayout(textPane);
    textLayout->setContentsMargins(0, 0, 0, 0);
    textLayout->addWidget(m_text);
    textLayout->addWidget(m_showMoved);
    textDock->setWidget(textPane);
    addDockWidget(Qt::RightDockWidgetArea, textDock);
    connect(m_text, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem *i, int) {
        if (i && m_session) {
            m_view->showRect(m_view->currentPage(), i->data(0, Qt::UserRole).toRectF());
        }
    });

    m_status = new QLabel(this);
    m_status->setObjectName(QStringLiteral("status"));
    statusBar()->addWidget(m_status);

    m_rescan = new QTimer(this);
    m_rescan->setSingleShot(true);
    connect(m_rescan, &QTimer::timeout, this, [this] {
        if (m_session) {
            m_session->startSweep();
            updateStatus();
        }
    });

    buildMenus();
    buildToolBar();

    // The same controls on a right-click, because that is where a reader
    // reaches for "do something to this bit of the drawing".
    m_view->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(m_view, &QWidget::customContextMenuRequested, this, &MainWindow::showViewMenu);
    connect(m_view, &CompareView::regionArmedChanged, this, [this](bool) { updateStatus(); });

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
    m_needSession.append(rep);
    file->addSeparator();
    QAction *pr = file->addAction(tr("&Print…"), this, &MainWindow::printSheets);
    pr->setObjectName(QStringLiteral("print"));
    pr->setShortcut(QKeySequence::Print);
    m_needSession.append(pr);
    QAction *prc = file->addAction(tr("Print &Changed Sheets…"), this,
                                   &MainWindow::printChangedSheets);
    prc->setObjectName(QStringLiteral("printChanged"));
    m_needSession.append(prc);
    file->addSeparator();
    QAction *quit = file->addAction(tr("&Quit"), qApp, &QApplication::quit);
    quit->setShortcut(QKeySequence::Quit);

    QMenu *view = menuBar()->addMenu(tr("&View"));
    // Bare 1/2/3 rather than a modifier: switching what you are looking at is
    // the most frequent thing a reader does here, and `Tab` next to them makes
    // the pair a blink comparator.
    // Checkable and in one group of four: which of them you are looking at is
    // the one thing the window must never leave you guessing about, and a bare
    // key that changes it silently is exactly how you end up guessing. Side by
    // side belongs in the group even though it is a layout rather than a view
    // mode — from the reader's chair it is the fourth answer to "what am I
    // looking at", and a checkable pair that can both be on says otherwise.
    m_modeGroup = new QActionGroup(this);
    m_overlayAct = view->addAction(tr("&Overlay"), this, [this] { setViewMode(0); });
    m_overlayAct->setObjectName(QStringLiteral("overlay"));
    m_overlayAct->setShortcut(Qt::Key_3);
    m_onlyAAct = view->addAction(tr("Only &A"), this, [this] { setViewMode(1); });
    m_onlyAAct->setObjectName(QStringLiteral("onlyA"));
    m_onlyAAct->setShortcut(Qt::Key_1);
    m_onlyBAct = view->addAction(tr("Only &B"), this, [this] { setViewMode(2); });
    m_onlyBAct->setObjectName(QStringLiteral("onlyB"));
    m_onlyBAct->setShortcut(Qt::Key_2);
    for (QAction *a : {m_overlayAct, m_onlyAAct, m_onlyBAct}) {
        a->setCheckable(true);
        m_modeGroup->addAction(a);
        m_needSession.append(a);
    }
    m_overlayAct->setChecked(true);
    QAction *flip = view->addAction(tr("&Blink A / B"), this, &MainWindow::blink);
    flip->setObjectName(QStringLiteral("flip"));
    flip->setShortcut(Qt::Key_Tab);
    m_needSession.append(flip);
    view->addSeparator();
    // A checkable pair rather than a third view mode: this changes how the
    // viewport is arranged, not what the core composes, and the A/B/overlay
    // choice still applies inside the single-sheet layout.
    QAction *sbs = view->addAction(tr("&Side by Side"));
    m_sideBySideAct = sbs;
    sbs->setObjectName(QStringLiteral("sideBySide"));
    sbs->setCheckable(true);
    sbs->setShortcut(Qt::Key_4);
    m_needSession.append(sbs);
    m_modeGroup->addAction(sbs);
    connect(sbs, &QAction::toggled, this, [this](bool on) {
        m_view->setLayout(on ? CompareView::Layout::SideBySide : CompareView::Layout::Single);
        updateStatus();
    });
    // Deliberately *not* in that group. Which of the two revisions is on screen
    // and how many sheets the scroll runs through are different questions, and
    // a reader wants a single sheet of the overlay, or of the pair side by
    // side, as readily as either on its own. Putting it in the group would make
    // choosing one sheet switch the overlay off, which is the trap the group
    // was built to close in the first place.
    m_singlePageAct = view->addAction(tr("Single &Page"));
    m_singlePageAct->setObjectName(QStringLiteral("singlePage"));
    m_singlePageAct->setCheckable(true);
    m_singlePageAct->setShortcut(Qt::Key_5);
    m_needSession.append(m_singlePageAct);
    connect(m_singlePageAct, &QAction::toggled, this, [this](bool on) {
        m_view->setFlow(on ? CompareView::Flow::SinglePage : CompareView::Flow::Continuous);
        updateStatus();
    });
    // The viewport can leave this flow without being asked: showing the whole
    // sheet is what the flow *is*, so a zoom cannot be honoured inside it. The
    // button has to say so, or it claims a view the window is not in.
    connect(m_view, &CompareView::flowChanged, this, [this](bool one) {
        const QSignalBlocker block(m_singlePageAct);
        m_singlePageAct->setChecked(one);
        updateStatus();
    });
    view->addSeparator();
    QAction *fw = view->addAction(tr("Fit &Width"), this,
                                  [this] { m_view->setFit(CompareView::Fit::Width); });
    fw->setShortcut(Qt::CTRL | Qt::Key_0);
    m_needSession.append(fw);
    m_needSession.append(
        view->addAction(tr("Fit &Page"), this, [this] { m_view->setFit(CompareView::Fit::Page); }));
    QAction *zi = view->addAction(tr("Zoom &In"), this,
                                  [this] { m_view->setZoom(m_view->zoom() * 1.25); });
    zi->setShortcut(QKeySequence::ZoomIn);
    m_needSession.append(zi);
    view->addSeparator();
    QAction *col = view->addAction(tr("Overlay &Colours…"), this, [this] {
        if (!m_session) {
            return;
        }
        ColourDialog d(m_session->colourOnlyA(), m_session->colourOnlyB(), this);
        if (d.exec() == QDialog::Accepted) {
            m_session->setColours(d.onlyA(), d.onlyB());
            refreshIcons();
            persist();
        }
    });
    col->setObjectName(QStringLiteral("overlayColours"));
    m_needSession.append(col);
    // The other half of "make the change findable": the colours say what the
    // difference is, this takes away everything that is not one.
    QAction *fade = view->addAction(tr("Fade the &Unchanged Drawing"), this,
                                    &MainWindow::stepFade);
    fade->setObjectName(QStringLiteral("fade"));
    fade->setShortcut(Qt::Key_F);
    m_needSession.append(fade);
    view->addSeparator();
    QAction *zo = view->addAction(tr("Zoom &Out"), this,
                                  [this] { m_view->setZoom(m_view->zoom() / 1.25); });
    zo->setShortcut(QKeySequence::ZoomOut);
    m_needSession.append(zo);

    QMenu *cmp = menuBar()->addMenu(tr("&Compare"));
    QAction *next = cmp->addAction(tr("&Next Change"), this, [this] { stepChange(1); });
    next->setObjectName(QStringLiteral("next"));
    next->setShortcut(Qt::CTRL | Qt::Key_Period);
    m_needSession.append(next);
    QAction *prev = cmp->addAction(tr("&Previous Change"), this, [this] { stepChange(-1); });
    prev->setObjectName(QStringLiteral("prev"));
    prev->setShortcut(Qt::CTRL | Qt::Key_Comma);
    m_needSession.append(prev);
    cmp->addSeparator();
    QAction *scanAll = cmp->addAction(tr("Scan &Every Sheet"), this, &MainWindow::scanEverySheet);
    scanAll->setObjectName(QStringLiteral("scanAll"));
    m_needSession.append(scanAll);
    m_acceptSuggestions =
        cmp->addAction(tr("Exclude &Suggested Regions"), this, &MainWindow::acceptSuggestions);
    m_acceptSuggestions->setObjectName(QStringLiteral("acceptSuggestions"));
    m_acceptSuggestions->setEnabled(false);
    cmp->addSeparator();
    QAction *sr = cmp->addAction(tr("Shift Pairing &Right"), this, [this] { nudgePairing(1); });
    sr->setObjectName(QStringLiteral("shiftRight"));
    sr->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Right);
    m_needSession.append(sr);
    QAction *sl = cmp->addAction(tr("Shift Pairing &Left"), this, [this] { nudgePairing(-1); });
    sl->setObjectName(QStringLiteral("shiftLeft"));
    sl->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Left);
    m_needSession.append(sl);
    QAction *am = cmp->addAction(tr("&Match Sheets by Content"), this, &MainWindow::matchSheets);
    am->setObjectName(QStringLiteral("autoMatch"));
    m_needSession.append(am);
    QAction *s0 = cmp->addAction(tr("Reset Pairing"), this, [this] {
        if (m_session) {
            m_session->setPageDelta(0);
        }
    });
    s0->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Home);
    m_needSession.append(s0);
    cmp->addSeparator();
    QAction *tp = cmp->addAction(tr("More &Tolerance"), this, [this] { nudgeTolerance(1); });
    tp->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Plus);
    m_needSession.append(tp);
    QAction *tm = cmp->addAction(tr("Less Tolerance"), this, [this] { nudgeTolerance(-1); });
    tm->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_Minus);
    m_needSession.append(tm);
    cmp->addSeparator();
    // Ctrl+drag has always done this, and nobody found it. An entry that arms
    // the next drag puts the feature where a reader looks for it, and leaves
    // Ctrl+drag working for anyone who already knows.
    m_excludeRegion = cmp->addAction(tr("Exclude a &Region…"), this, [this] {
        m_view->armRegion();
    });
    m_excludeRegion->setObjectName(QStringLiteral("excludeRegion"));
    m_excludeRegion->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_I);
    m_needSession.append(m_excludeRegion);
    QAction *clr = cmp->addAction(tr("&Clear Excluded Regions"), this, [this] {
        if (m_session) {
            m_session->clearIgnoreRects();
            persist();
            // Same as adding one: what was excluded is compared again now, and
            // nothing knows that until the set has been swept.
            m_session->startSweep();
            updateStatus();
        }
    });
    clr->setObjectName(QStringLiteral("clearRegions"));
    clr->setShortcut(Qt::ALT | Qt::SHIFT | Qt::Key_C);
    m_needSession.append(clr);

    enableSessionActions(false);
}

void MainWindow::buildToolBar() {
    // Icons *and* text. The words were here first, on the grounds that no icon
    // set says "only the earlier revision" — which is still true of every icon
    // set there is, and is why these are drawn in `Icons.cpp` from the thing
    // they stand for: the overlay button is a picture of the composition rule,
    // in the reader's own two colours. Keeping the words next to them costs a
    // little width and settles the guessing the old comment was worried about.
    auto *bar = addToolBar(tr("Comparison"));
    QAction *fadeAct = findChild<QAction *>(QStringLiteral("fade"));
    bar->setObjectName(QStringLiteral("toolbar"));
    bar->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    bar->setMovable(false);
    bar->addAction(findChild<QAction *>(QStringLiteral("open")));
    bar->addSeparator();
    bar->addAction(m_onlyAAct);
    bar->addAction(m_onlyBAct);
    bar->addAction(m_overlayAct);
    bar->addAction(m_sideBySideAct);
    // One sheet at a time is not on the bar. It belongs to the same family as
    // the four buttons above it and sat with them, but it answers a different
    // question — those pick *what* is drawn, this picks how much of the set is
    // — and a fifth button in the row read as a fifth view mode. It keeps the
    // View menu, the context menu and `5`.
    bar->addSeparator();
    bar->addAction(findChild<QAction *>(QStringLiteral("prev")));
    bar->addAction(findChild<QAction *>(QStringLiteral("next")));
    bar->addSeparator();
    bar->addAction(m_excludeRegion);
    bar->addSeparator();

    // Tolerance on the bar rather than only on two keys nobody was told about.
    // It is the one setting that changes every answer this tool gives, and the
    // right value is not something a reader knows in advance — it is found by
    // turning it up until the fringe goes and stopping before the real changes
    // do. That is a control you want under your hand, not in a menu.
    auto *tolLabel = new QLabel(tr("Tolerance"), bar);
    tolLabel->setContentsMargins(8, 0, 4, 0);
    bar->addWidget(tolLabel);
    m_toleranceBox = new QSpinBox(bar);
    m_toleranceBox->setObjectName(QStringLiteral("toleranceBox"));
    m_toleranceBox->setRange(0, Session::maxTolerance());
    m_toleranceBox->setSuffix(tr(" px"));
    m_toleranceBox->setToolTip(
        tr("How far a stroke may sit from its counterpart and still count as the "
           "same artwork.\n\n"
           "1 absorbs the fringe two PDF producers leave around the same line, "
           "and it is why a cross-producer pair is readable at all. More is "
           "needed as you zoom in, because this is measured in screen pixels.\n\n"
           "Above %1, a stroke that merely moved stops being reported as a "
           "change at all.")
            .arg(Session::toleranceHidesMovement()));
    bar->addWidget(m_toleranceBox);
    m_needSessionWidgets.append(tolLabel);
    m_needSessionWidgets.append(m_toleranceBox);

    // Fading the drawing the two revisions agree on. Three small edits on a
    // dense sheet are three specks of colour in a page of black line work, and
    // no amount of staring finds them; take the agreed ink away and the sheet
    // is blank except for exactly what changed.
    //
    // A button *and* a slider for one setting, which is usually a mistake and
    // is not here: the button is the one a reader wants, four clicks from the
    // drawing to nothing and back, and the slider is where they stop halfway
    // because a speck of colour on an empty sheet does not say where on the
    // sheet it is.
    bar->addAction(fadeAct);
    m_fadeSlider = new QSlider(Qt::Horizontal, bar);
    m_fadeSlider->setObjectName(QStringLiteral("fadeSlider"));
    m_fadeSlider->setRange(0, 100);
    m_fadeSlider->setValue(100);
    m_fadeSlider->setFixedWidth(90);
    // The value lands when the handle is let go, not on every pixel of the
    // drag. Each step re-renders both revisions of every tile on screen
    // through MuPDF, and a hundred of those between one end and the other is a
    // slider that stutters. The button covers the quick sweep; this is for
    // stopping somewhere in particular.
    m_fadeSlider->setTracking(false);
    m_fadeSlider->setToolTip(
        tr("How strongly to draw what the two revisions agree on.\n\n"
           "Full is the drawing as it was drawn, with the changes coloured on "
           "top of it. Turned down, the unchanged artwork fades towards white "
           "and the differences stay exactly as they are — at nothing, the "
           "sheet is blank except for what changed.\n\n"
           "The overlay only; a single revision is shown as it is."));
    bar->addWidget(m_fadeSlider);
    m_needSessionWidgets.append(m_fadeSlider);
    connect(m_fadeSlider, &QSlider::valueChanged, this, [this](int v) {
        if (m_session) {
            m_session->setSharedInk(v);
            persist();
            updateStatus();
        }
    });
    connect(m_toleranceBox, &QSpinBox::valueChanged, this, &MainWindow::changeTolerance);

    // Shorter words on the buttons than in the menu, because the menu has room
    // to say "Compare Two Files…" and a toolbar with ten controls on it does
    // not. The menu entry stays the long one: that is where a reader goes to
    // find out what something is called.
    struct Short {
        const char *name;
        QString text;
    };
    for (const Short &s : {Short{"open", tr("Open")}, Short{"prev", tr("Previous")},
                           Short{"next", tr("Next")}, Short{"excludeRegion", tr("Exclude")},
                           Short{"fade", tr("Fade")}}) {
        if (QAction *a = findChild<QAction *>(QString::fromLatin1(s.name))) {
            a->setIconText(s.text);
        }
    }

    // The shortcut in the tooltip, so the toolbar teaches the keys rather than
    // replacing them.
    for (QAction *a : bar->actions()) {
        if (a && !a->shortcut().isEmpty()) {
            a->setToolTip(tr("%1  (%2)").arg(a->text().remove(QLatin1Char('&')),
                                             a->shortcut().toString(QKeySequence::NativeText)));
        }
    }

    refreshIcons();
}

/// Draws the toolbar's pictures for the colours in force.
///
/// Called again whenever those change, because two of the buttons stand for the
/// two overlay colours and a reader who moved them away from red and green did
/// so precisely because they cannot tell red from green.
void MainWindow::refreshIcons() {
    Icons::Look look;
    look.ink = palette().buttonText().color();
    look.onlyA = m_session ? m_session->colourOnlyA() : ColourDialog::defaultA();
    look.onlyB = m_session ? m_session->colourOnlyB() : ColourDialog::defaultB();

    auto set = [this](const char *name, const QIcon &icon) {
        if (QAction *a = findChild<QAction *>(QString::fromLatin1(name))) {
            a->setIcon(icon);
        }
    };
    set("open", Icons::open(look));
    set("onlyA", Icons::onlyA(look));
    set("onlyB", Icons::onlyB(look));
    set("overlay", Icons::overlay(look));
    set("sideBySide", Icons::sideBySide(look));
    set("singlePage", Icons::singlePage(look));
    set("prev", Icons::previousChange(look));
    set("next", Icons::nextChange(look));
    set("excludeRegion", Icons::excludeRegion(look));
    set("fade", Icons::fadeShared(look));
}

void MainWindow::changeEvent(QEvent *e) {
    QMainWindow::changeEvent(e);
    // The outlines are drawn in the toolbar's foreground colour, so a reader
    // who switches their desktop to a dark theme while this is open would
    // otherwise be left with black-on-black buttons.
    if (e->type() == QEvent::PaletteChange) {
        refreshIcons();
    }
}

void MainWindow::showViewMenu(const QPoint &at) {
    QMenu m(this);
    m.addAction(m_onlyAAct);
    m.addAction(m_onlyBAct);
    m.addAction(m_overlayAct);
    m.addAction(m_sideBySideAct);
    m.addAction(m_singlePageAct);
    m.addSeparator();
    m.addAction(m_excludeRegion);
    m.addAction(findChild<QAction *>(QStringLiteral("clearRegions")));
    m.exec(m_view->mapToGlobal(at));
}

void MainWindow::enableSessionActions(bool on) {
    for (QAction *a : m_needSession) {
        if (a) {
            a->setEnabled(on);
        }
    }
    for (QWidget *w : m_needSessionWidgets) {
        if (w) {
            w->setEnabled(on);
        }
    }
}

void MainWindow::syncViewActions() {
    if (!m_session) {
        return;
    }
    QAction *want = m_overlayAct;
    switch (m_session->viewMode()) {
    case SC_VIEW_MODE_ONLY_A:
        want = m_onlyAAct;
        break;
    case SC_VIEW_MODE_ONLY_B:
        want = m_onlyBAct;
        break;
    default:
        break;
    }
    // setChecked on a grouped action unchecks the others; the actions are
    // connected to triggered(), not toggled(), so this cannot loop.
    want->setChecked(true);
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
    enableSessionActions(true);
    syncViewActions();
    // The pair may have brought its own colours back with it.
    refreshIcons();
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
    if (m_session->tolerance() > Session::toleranceHidesMovement()) {
        // On the paper too. A printout is read away from the window that made
        // it, and this changes what an unmarked part of the sheet means.
        line2 += tr(" — a stroke that only moved is not reported");
    }
    if (m_session->viewMode() == SC_VIEW_MODE_OVERLAY && m_session->sharedInk() < 100) {
        // Likewise, and more so: this one is why the sheet came out nearly
        // empty, and nothing else on the page would say.
        line2 += QStringLiteral("   ·   ") +
                 tr("unchanged drawing faded to %1%").arg(m_session->sharedInk());
    }
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
        syncViewActions();
        updateStatus();
    }
}

void MainWindow::blink() {
    if (!m_session) {
        return;
    }
    // A, B, A, B. The overlay is deliberately not in the cycle: this is a blink
    // comparator, and it works because the two readings are the same drawing at
    // the same place and only what changed moves. Putting a third picture
    // between them — one that differs from both everywhere there is colour —
    // breaks exactly the effect the key exists for. From anywhere else, the
    // first `Tab` enters the blink at A; the overlay is a keystroke of its own.
    // Zoom and scroll never move: the eye catches what jumps.
    m_session->setViewMode(m_session->viewMode() == SC_VIEW_MODE_ONLY_A ? SC_VIEW_MODE_ONLY_B
                                                                        : SC_VIEW_MODE_ONLY_A);
    // Leaves side by side, because the cycle is over the single-sheet views and
    // `syncViewActions` checks one of them — which, in one exclusive group,
    // switches side by side off and the layout back with it.
    syncViewActions();
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
        changeTolerance(m_session->tolerance() + by);
    }
}

/// Changes the tolerance and arranges for the set to be scanned again.
///
/// Every scanned answer was about the old tolerance, and the core throws them
/// away when it changes — so without this the sidebar simply empties and stays
/// empty until the reader finds "Scan Every Sheet" for themselves. The sweep is
/// what the tolerance is usually being changed *for*.
///
/// It waits a moment first. The spin box goes up one step per click and a
/// reader hunting for the value that clears the fringe clicks it several times;
/// restarting on each one stops a worker mid-sheet for nothing. Not the idle
/// timer the ground rules forbid: this fires once, because of something the
/// reader did, and never polls.
void MainWindow::changeTolerance(int to) {
    if (!m_session) {
        return;
    }
    const int was = m_session->tolerance();
    m_session->setTolerance(to);
    if (m_session->tolerance() == was) {
        return;
    }
    persist();
    m_rescan->start(400);
}

/// Steps the unchanged drawing a quarter of the way towards white, and from
/// nothing back to the whole drawing.
///
/// A cycle rather than a one-way dimmer, because the reader is going to want
/// the drawing back: the fade is for finding the change, and the circuit around
/// it is for understanding the change once found.
void MainWindow::stepFade() {
    if (!m_session) {
        return;
    }
    const int now = m_session->sharedInk();
    const int next = now <= 0 ? 100 : qMax(0, ((now - 1) / 25) * 25);
    // Through the slider, so there is one path to the setting and the slider
    // cannot end up showing something other than what is drawn.
    m_fadeSlider->setValue(next);
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
    const bool showMoved = m_showMoved->isChecked();
    int moved = 0;
    for (const Session::TextChange &c : m_session->textChanges(page)) {
        if (c.kind == SC_TEXT_CHANGE_KIND_MOVED) {
            moved++;
            if (!showMoved) {
                continue;
            }
        }
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
    m_showMoved->setText(moved == 1 ? tr("Include the 1 piece of text that only moved")
                                    : tr("Include the %1 pieces of text that only moved")
                                          .arg(moved));
    m_showMoved->setEnabled(moved > 0);
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
    // And every sheet's answer was about the comparison before this rectangle,
    // so the core has thrown them all away. Sweeping again is the whole point
    // of drawing it: a reader excludes the title block precisely to find out
    // which sheets still have something on them. Without this the sidebar
    // emptied and stayed empty, which reads as "nothing changed anywhere".
    m_session->startSweep();
    updateStatus();
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
        if (m_session->sizeMismatch(p) == 1) {
            // Ahead of any figure, because no figure for this sheet means
            // anything: the two revisions are different sizes on paper and the
            // smaller one was compared against a crop of the larger.
            item->setText(2, tr("different size"));
            item->setToolTip(2, tr("The two revisions of this sheet are different sizes on "
                                   "paper. They were compared at the first document's size, "
                                   "with the other cropped, so the count is not reliable."));
        } else {
            const float covered = m_session->coverage(p);
            if (covered >= 0.0f) {
                item->setText(2, tr("%1%").arg(int(covered * 100.0f + 0.5f)));
            }
        }
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
        m_status->setText(tr("Open two revisions of a drawing to compare them."));
        return;
    }
    // The toolbar's settings say what the core says, whatever changed them —
    // the spin box, the two keys, or the pair's own saved settings on the way
    // in.
    if (m_toleranceBox) {
        const QSignalBlocker block(m_toleranceBox);
        m_toleranceBox->setValue(m_session->tolerance());
    }
    // The fade has nothing to fade unless the overlay is what is on screen: a
    // single revision is drawn exactly as it is, and side by side is two of
    // those. A control that is live but does nothing is what "clicking Only A
    // does nothing" turned out to mean, and it is not being repeated here.
    const bool overlayIsOn = m_view->layout() != CompareView::Layout::SideBySide &&
                             m_session->viewMode() == SC_VIEW_MODE_OVERLAY;
    if (m_fadeSlider) {
        const QSignalBlocker block(m_fadeSlider);
        m_fadeSlider->setValue(m_session->sharedInk());
        m_fadeSlider->setEnabled(overlayIsOn);
    }
    if (QAction *fade = findChild<QAction *>(QStringLiteral("fade"))) {
        fade->setEnabled(overlayIsOn);
    }
    if (m_view->regionArmed()) {
        m_status->setText(
            tr("Drag a rectangle over the part to leave out of the comparison.  "
               "Escape cancels."));
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
    // Said here rather than in a field of its own, because it is the reason the
    // scroll stops at the foot of the sheet and that question comes up while
    // the reader is looking at the sheet, not at the status bar.
    if (m_view->flow() == CompareView::Flow::SinglePage) {
        mode += tr(", one sheet");
    }

    QString text = tr("Sheet %1 of %2   %3   %4%   tolerance %5")
                       .arg(page)
                       .arg(m_session->pageCount())
                       .arg(mode)
                       .arg(int(m_view->zoom() * 100))
                       .arg(m_session->tolerance());
    // Next to the number it qualifies. A reader who turned the tolerance up to
    // clear the fringe has to be told what it costs, at the moment they are
    // reading the counts it produced.
    if (m_session->tolerance() > Session::toleranceHidesMovement()) {
        text += tr("   ⚠ a stroke that only moved is not reported");
    }
    // Only when it is not the whole drawing. A reader who left this down and
    // came back to a nearly blank sheet has to be able to find out why, and the
    // slider is small.
    if (overlayIsOn && m_session->sharedInk() < 100) {
        text += tr("   unchanged drawing at %1%").arg(m_session->sharedInk());
    }
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
