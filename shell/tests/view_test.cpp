// Drives the real window and asserts what a reader would see.
//
// A plain executable, not a framework. Runs on Qt's `offscreen` platform, so it
// needs no X server, no window manager and no synthetic X events — the last of
// which is what makes an xdotool-driven test flaky against Qt, since Qt ignores
// XSendEvent and a bare Xvfb has no window manager to focus anything.
//
// `--write <dir>` dumps what it drew as PNGs, because a failing assertion about
// a picture is far easier to understand next to the picture.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "ColourDialog.h"
#include "CompareView.h"
#include "MainWindow.h"
#include "Session.h"

#include <QAction>
#include <QApplication>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QFile>
#include <QPrinter>
#include <QCheckBox>
#include <QPushButton>
#include <QFileInfo>
#include <QFile>
#include <QPrinter>
#include <QCheckBox>
#include <QPushButton>
#include <QFileInfo>
#include <QLabel>
#include <QTest>
#include <QScrollBar>
#include <QSlider>
#include <QSpinBox>
#include <QToolBar>
#include <QWheelEvent>
#include <QTreeWidget>

static int failures = 0;
static QString writeDir;

static void check(bool ok, const QString &what) {
    if (!ok) {
        fprintf(stderr, "FAIL %s\n", qPrintable(what));
        failures++;
    }
}

static bool actionChecked(const QWidget &win, const char *name) {
    auto *a = win.findChild<QAction *>(QString::fromLatin1(name));
    return a && a->isChecked();
}

static void shot(QWidget *w, const QString &name) {
    if (writeDir.isEmpty()) {
        return;
    }
    w->grab().save(QDir(writeDir).filePath(name + ".png"));
}

// Spins the event loop until the sweep has finished *and* the window has
// collected it. The sweep is driven by a wakeup handle the event loop watches,
// so this is a real wait on real machinery, not a sleep.
//
// Waiting on `finished` alone is a race, and it was one: the worker sets that
// flag before the event loop has delivered the wakeup that rebuilds the
// sidebar, so two runs in three found 20 sheets listed and a status line still
// reading "scanning 20 of 21".
static bool waitForSweep(Session *s) {
    for (int i = 0; i < 600; i++) {
        if (s->sweepCollected()) {
            return true;
        }
        QTest::qWait(50);
    }
    return false;
}

// One side of a named pair from `samples/sets.json`, or empty when the drawings
// are not on this machine. The sets are addressed by the role they play rather
// than by name: this repository is public and a customer's board codes are as
// much theirs as the drawings.
static QString sample(const QString &set, const QString &side) {
    const QDir root(QStringLiteral(SC_SOURCE_DIR "/../../samples"));
    QFile manifest(root.filePath(QStringLiteral("sets.json")));
    if (!manifest.open(QIODevice::ReadOnly)) {
        return {};
    }
    const QJsonObject all = QJsonDocument::fromJson(manifest.readAll()).object();
    const QString name = all.value(set).toObject().value(side).toString();
    if (name.isEmpty()) {
        return {};
    }
    const QString p = QFileInfo(root.filePath(name)).absoluteFilePath();
    return QFileInfo::exists(p) ? p : QString();
}

/// A number the manifest says this pair should produce, or -1.
static int expected(const QString &set, const QString &key) {
    const QDir root(QStringLiteral(SC_SOURCE_DIR "/../../samples"));
    QFile manifest(root.filePath(QStringLiteral("sets.json")));
    if (!manifest.open(QIODevice::ReadOnly)) {
        return -1;
    }
    const QJsonObject all = QJsonDocument::fromJson(manifest.readAll()).object();
    return all.value(set).toObject().value(key).toInt(-1);
}

int main(int argc, char **argv) {
    // Point the settings at a scratch directory: the real one belongs to whoever
    // is running the tests and is not ours to write into.
    //
    // Named outright rather than set through the environment. The first attempt
    // set `XDG_CONFIG_HOME`, which passed on Linux and quietly wrote to the real
    // location on Windows, where the core reads `APPDATA` — and it is the core's
    // own answer, below, that the assertions use, so the two cannot drift apart
    // again.
    const QString cfg = QDir::temp().filePath(QStringLiteral("sch-pdf-compare-viewtest"));
    QDir(cfg).removeRecursively();
    sc_settings_set_dir(cfg.toUtf8().constData());
    const QString settingsFile = QString::fromUtf8(sc_settings_path());

    QApplication app(argc, argv);
    for (int i = 1; i + 1 < argc; i++) {
        if (QString::fromLatin1(argv[i]) == QLatin1String("--write")) {
            writeDir = QString::fromLocal8Bit(argv[i + 1]);
        }
    }

    MainWindow win;
    win.setForTesting(true);
    win.resize(1400, 950);
    win.show();

    auto *status = win.findChild<QLabel *>(QStringLiteral("status"));
    auto *sheets = win.findChild<QTreeWidget *>(QStringLiteral("sheets"));
    auto *view = win.findChild<CompareView *>();
    check(status && sheets && view, QStringLiteral("the window has its parts"));
    if (!status || !sheets || !view) {
        return 1;
    }

    check(status->text().contains(QStringLiteral("Open two")),
          QStringLiteral("with nothing open it says so"));
    // Nothing that needs a comparison may look available before there is one:
    // a key or a button that quietly does nothing reads as broken, and that is
    // what "the keys don't work" turned out to mean.
    for (const char *name : {"overlay", "onlyA", "onlyB", "flip", "sideBySide", "singlePage",
                             "next", "prev", "excludeRegion", "clearRegions", "scanAll",
                             "autoMatch"}) {
        auto *a = win.findChild<QAction *>(QString::fromLatin1(name));
        check(a && !a->isEnabled(),
              QStringLiteral("%1 is off until a pair is open").arg(QString::fromLatin1(name)));
    }
    // Every button in the bar has a picture and keeps its words. Half a toolbar
    // in icons and half in text reads as unfinished, and an icon nobody drew is
    // a blank square on the two platforms with no icon theme.
    {
        auto *bar = win.findChild<QToolBar *>(QStringLiteral("toolbar"));
        check(bar != nullptr, QStringLiteral("the window has a toolbar"));
        for (QAction *a : bar ? bar->actions() : QList<QAction *>()) {
            if (a->isSeparator() || a->text().isEmpty()) {
                continue; // a separator, or a control rather than a button
            }
            check(!a->icon().isNull(),
                  QStringLiteral("%1 has an icon").arg(a->text()));
            check(!a->icon().pixmap(24, 24).isNull(),
                  QStringLiteral("%1's icon is drawn, not empty").arg(a->text()));
        }
    }
    shot(&win, QStringLiteral("empty"));

    // Written into the build tree by `gen-fixtures`, so everything below runs
    // on a clone with no customer drawings in it. Three sheets: the later
    // revision changes one component value on sheet 1 and the revision letter
    // in every title block.
    const QString a = QStringLiteral(SC_FIXTURE_DIR "/a.pdf");
    const QString b = QStringLiteral(SC_FIXTURE_DIR "/b.pdf");
    check(QFileInfo::exists(a) && QFileInfo::exists(b),
          QStringLiteral("the fixture documents were written"));

    check(win.openPair(a, b), QStringLiteral("the pair opens"));
    QTest::qWait(50);
    check(status->text().contains(QStringLiteral("of 6")),
          QStringLiteral("6 virtual sheets: '%1'").arg(status->text()));
    check(status->text().contains(QStringLiteral("Overlay")),
          QStringLiteral("overlay is the default view"));
    shot(&win, QStringLiteral("opened"));

    // Opening starts the sweep by itself; it fills the sidebar as it goes and
    // the window stays responsive throughout. This set shares a title block
    // whose date changed, so every sheet reports something — which is exactly
    // why excluded regions exist.
    check(view->session()->sweepStatus().running || view->session()->sweepStatus().finished,
          QStringLiteral("opening a pair starts the sweep"));
    check(waitForSweep(view->session()), QStringLiteral("the sweep finishes"));
    check(sheets->topLevelItemCount() == 6,
          QStringLiteral("every sheet is listed, got %1").arg(sheets->topLevelItemCount()));
    // The count is next to how much of the sheet it covers, because a count on
    // its own cannot separate a few edits from a redrawn sheet.
    check(sheets->headerItem()->text(2).contains(QStringLiteral("sheet")),
          QStringLiteral("the list has a coverage column"));
    check(!sheets->topLevelItem(0)->text(2).isEmpty(),
          QStringLiteral("and fills it in, got '%1'").arg(sheets->topLevelItem(0)->text(2)));
    check(status->text().contains(QStringLiteral("6 sheets changed")),
          QStringLiteral("and the status line says so: '%1'").arg(status->text()));
    shot(&win, QStringLiteral("scanned"));

    // Stepping through changes moves the reader without touching their zoom.
    const double zoomBefore = view->zoom();
    win.findChild<QAction *>(QStringLiteral("next"))->trigger();
    QTest::qWait(50);
    check(qFuzzyCompare(view->zoom(), zoomBefore),
          QStringLiteral("stepping to a change leaves the zoom alone"));
    check(status->text().contains(QStringLiteral("at change 1")),
          QStringLiteral("and says where it is: '%1'").arg(status->text()));
    shot(&win, QStringLiteral("first-change"));

    // Flipping between the documents must not move the view either: it is a
    // blink comparator and the eye catches what jumps.
    const int pageBefore = view->currentPage();
    win.findChild<QAction *>(QStringLiteral("onlyA"))->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("A only")), QStringLiteral("A only"));
    check(view->currentPage() == pageBefore && qFuzzyCompare(view->zoom(), zoomBefore),
          QStringLiteral("A only does not move the view"));
    shot(&win, QStringLiteral("only-a"));
    const QImage drawnA = view->viewport()->grab().toImage();
    win.findChild<QAction *>(QStringLiteral("onlyB"))->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("B only")), QStringLiteral("B only"));
    shot(&win, QStringLiteral("only-b"));
    const QImage drawnB = view->viewport()->grab().toImage();
    win.findChild<QAction *>(QStringLiteral("overlay"))->trigger();
    QTest::qWait(20);
    const QImage drawnOverlay = view->viewport()->grab().toImage();

    // The pixels, not the status line. This check is here because its absence
    // shipped: the mode was set, the cache was emptied and the status line read
    // "A only", while the viewport went on drawing the overlay, because the
    // layout that carries which document each sheet shows was never rebuilt.
    // Asserting on the words the window says about itself proved nothing.
    check(drawnA != drawnOverlay, QStringLiteral("A only really draws something else"));
    check(drawnB != drawnOverlay, QStringLiteral("B only really draws something else"));
    check(drawnA != drawnB, QStringLiteral("and the two revisions differ from each other"));

    // `Tab` blinks between the two revisions and never shows the overlay: a
    // third picture between them would break the effect the key exists for.
    // The window is on the overlay here, so the first one enters the blink.
    auto *tab = win.findChild<QAction *>(QStringLiteral("flip"));
    tab->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("A only")), QStringLiteral("Tab: overlay -> A"));
    tab->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("B only")), QStringLiteral("Tab: A -> B"));
    tab->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("A only")), QStringLiteral("Tab: B -> A"));
    tab->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("B only")),
          QStringLiteral("and never lands on the overlay: '%1'").arg(status->text()));
    check(view->currentPage() == pageBefore && qFuzzyCompare(view->zoom(), zoomBefore),
          QStringLiteral("and blinking leaves the view where it was"));
    win.findChild<QAction *>(QStringLiteral("overlay"))->trigger();
    QTest::qWait(20);

    // The finished sweep should have spotted the title block and be offering
    // it — offering, with the menu item enabled, not applying it.
    Session *s = view->session();
    auto *accept = win.findChild<QAction *>(QStringLiteral("acceptSuggestions"));
    check(accept->isEnabled(), QStringLiteral("a repeating region is offered"));
    check(s->ignoreRects().isEmpty(), QStringLiteral("and nothing is excluded yet"));
    check(status->text().contains(QStringLiteral("repeat")),
          QStringLiteral("the status line mentions it: '%1'").arg(status->text()));

    // Accepting it has to take sheets off the list, and say so.
    win.applySuggestions();
    check(waitForSweep(s), QStringLiteral("the sweep runs again after excluding"));
    check(sheets->topLevelItemCount() < 6,
          QStringLiteral("excluding the title block clears sheets, %1 left")
              .arg(sheets->topLevelItemCount()));
    check(status->text().contains(QStringLiteral("excluded")),
          QStringLiteral("and the exclusion is on show: '%1'").arg(status->text()));
    shot(&win, QStringLiteral("title-block-excluded"));

    win.findChild<QAction *>(QStringLiteral("clearRegions"))->trigger();
    QTest::qWait(20);
    check(s->ignoreRects().isEmpty(), QStringLiteral("regions clear"));

    // Excluding a region by hand. Ctrl+drag has always done it and nobody
    // found it, so the menu entry arms the next plain drag instead; this is
    // that path, end to end.
    auto *exclude = win.findChild<QAction *>(QStringLiteral("excludeRegion"));
    check(exclude && exclude->isEnabled(), QStringLiteral("excluding a region is offered"));
    exclude->trigger();
    check(view->regionArmed(), QStringLiteral("and arms the next drag"));
    check(status->text().contains(QStringLiteral("Drag a rectangle")),
          QStringLiteral("and says what to do: '%1'").arg(status->text()));
    QWidget *vp = view->viewport();
    QTest::mousePress(vp, Qt::LeftButton, Qt::NoModifier, QPoint(60, 60));
    QTest::mouseMove(vp, QPoint(160, 140));
    QTest::mouseRelease(vp, Qt::LeftButton, Qt::NoModifier, QPoint(160, 140));
    QTest::qWait(20);
    check(s->ignoreRects().size() == 1,
          QStringLiteral("a plain drag excludes a region, got %1").arg(s->ignoreRects().size()));
    check(!view->regionArmed(), QStringLiteral("and disarms, so the next drag scrolls"));
    shot(&win, QStringLiteral("region-excluded-by-hand"));
    win.findChild<QAction *>(QStringLiteral("clearRegions"))->trigger();
    QTest::qWait(20);

    // Nudging the pairing changes which sheets face each other.
    const QImage drawnPaired = view->viewport()->grab().toImage();
    win.findChild<QAction *>(QStringLiteral("shiftRight"))->trigger();
    QTest::qWait(50);
    check(s->pageDelta() == 1, QStringLiteral("pairing shifted"));
    check(s->pair(1).first == 0, QStringLiteral("and sheet 1 of A now has no counterpart"));
    // Same reason as the view modes: the page count and what each sheet shows
    // both live in the layout, so a shift that is not drawn is a shift that did
    // not happen as far as the reader is concerned.
    check(view->viewport()->grab().toImage() != drawnPaired,
          QStringLiteral("and the viewport draws the shift"));
    win.findChild<QAction *>(QStringLiteral("shiftLeft"))->trigger();
    QTest::qWait(20);
    check(s->pageDelta() == 0, QStringLiteral("and shifts back"));

    // Matching by content lines these sets up one to one, and says it did.
    win.findChild<QAction *>(QStringLiteral("autoMatch"))->trigger();
    QTest::qWait(50);
    check(s->pairingIsAutomatic(), QStringLiteral("the pairing is a content match"));
    check(s->pair(2).first == 2 && s->pair(2).second == 2,
          QStringLiteral("and these sets match one to one"));
    check(status->text().contains(QStringLiteral("matched by content")),
          QStringLiteral("and the status line says which pairing is in force: '%1'")
              .arg(status->text()));
    check(waitForSweep(s), QStringLiteral("and the sweep runs again on the new pairing"));
    shot(&win, QStringLiteral("matched"));

    // The text panel says what a sheet reads differently. Sheet 2 of this pair
    // renamed a run of UART nets, which is the kind of thing a reviewer is
    // actually hunting for and cannot see in a red blob.
    auto *textList = win.findChild<QTreeWidget *>(QStringLiteral("textChanges"));
    check(textList != nullptr, QStringLiteral("the window has a text panel"));
    view->goToPage(1);
    QTest::qWait(50);
    check(textList->topLevelItemCount() > 0,
          QStringLiteral("sheet 1 has text changes, got %1").arg(textList->topLevelItemCount()));
    bool renamed = false;
    for (int i = 0; i < textList->topLevelItemCount(); i++) {
        if (textList->topLevelItem(i)->text(0) == QLatin1String("NET_ALPHA") &&
            textList->topLevelItem(i)->text(1) == QLatin1String("NET_BRAVO")) {
            renamed = true;
        }
    }
    check(renamed, QStringLiteral("and it spells out the rename"));

    // Text that only moved is counted rather than listed, so the few rows that
    // say something different are not buried under hundreds that do not.
    auto *showMoved = win.findChild<QCheckBox *>(QStringLiteral("showMoved"));
    check(showMoved != nullptr, QStringLiteral("there is a control for moved text"));
    check(!showMoved->isChecked(), QStringLiteral("moved text is out of the list by default"));
    check(showMoved->text().contains(QStringLiteral("moved")),
          QStringLiteral("and the control says how many there are: '%1'").arg(showMoved->text()));
    const int listed = textList->topLevelItemCount();
    showMoved->setChecked(true);
    QTest::qWait(20);
    check(textList->topLevelItemCount() >= listed,
          QStringLiteral("turning them on can only add rows"));
    showMoved->setChecked(false);
    QTest::qWait(20);
    check(textList->topLevelItemCount() == listed, QStringLiteral("and turning them off restores"));
    shot(&win, QStringLiteral("text-changes"));

    // The report is the thing that leaves the application. Written from what
    // has already been scanned, so it does not re-render eighty-five sheets.
    const QString md = s->report();
    check(md.contains(QStringLiteral("# What changed")), QStringLiteral("the report has a title"));
    check(md.contains(QStringLiteral("`NET_ALPHA`")),
          QStringLiteral("and carries the net renames into it"));
    check(md.contains(QStringLiteral("## Sheet 1")), QStringLiteral("sheet by sheet"));
    if (!writeDir.isEmpty()) {
        QFile out(QDir(writeDir).filePath(QStringLiteral("report.md")));
        if (out.open(QIODevice::WriteOnly | QIODevice::Text)) {
            out.write(md.toUtf8());
        }
    }

    // The overlay colours. The default pair is red and green, which a reader
    // with red-green colour blindness cannot tell apart — and everything this
    // tool shows is carried by those two colours, so this has to be reachable
    // without editing a settings file.
    {
        check(win.findChild<QAction *>(QStringLiteral("overlayColours")) != nullptr,
              QStringLiteral("there is a menu entry for the colours"));
        const QColor wasA = s->colourOnlyA();
        check(wasA == ColourDialog::defaultA(), QStringLiteral("the default is red"));

        ColourDialog d(s->colourOnlyA(), s->colourOnlyB());
        d.findChild<QPushButton *>(QStringLiteral("accessiblePreset"))->click();
        check(d.onlyA() == ColourDialog::accessibleA() && d.onlyB() == ColourDialog::accessibleB(),
              QStringLiteral("the preset picks a pair that survives colour blindness"));
        shot(&d, QStringLiteral("colours"));

        s->setColours(d.onlyA(), d.onlyB());
        QTest::qWait(20);
        check(s->colourOnlyA() == ColourDialog::accessibleA(),
              QStringLiteral("and the core takes them"));

        // The overlay has to actually come out in the new colours.
        const QSize dev = s->pageDeviceSize(1, 1.0);
        const QImage tile = s->tile(1, 1.0, QRect(QPoint(0, 0), dev), SC_VIEW_MODE_OVERLAY);
        bool blueish = false;
        for (int y = 0; y < tile.height() && !blueish; y += 2) {
            for (int x = 0; x < tile.width(); x += 2) {
                const QColor c = tile.pixelColor(x, y);
                if (c.blue() > c.red() + 60 && c.blue() > c.green() + 40) {
                    blueish = true;
                    break;
                }
            }
        }
        check(blueish, QStringLiteral("the overlay is drawn in them"));

        s->setColours(ColourDialog::defaultA(), ColourDialog::defaultB());
        QTest::qWait(20);
    }

    // Side by side. Where the overlay is at its worst — text that changed is
    // drawn twice on top of itself — this is what makes both readings legible.
    {
        auto *sbs = win.findChild<QAction *>(QStringLiteral("sideBySide"));
        check(sbs != nullptr, QStringLiteral("there is a side-by-side action"));
        const double before = view->zoom();
        sbs->setChecked(true);
        QTest::qWait(50);
        check(view->layout() == CompareView::Layout::SideBySide,
              QStringLiteral("the viewport is side by side"));
        check(status->text().contains(QStringLiteral("Side by side")),
              QStringLiteral("and says so: '%1'").arg(status->text()));
        // Two sheets across means each is drawn smaller, so a fitted zoom has
        // to come down. Leaving it alone would push half the comparison off
        // screen, which is the whole thing this view exists to avoid.
        check(view->zoom() < before,
              QStringLiteral("fitting the width accounts for both sheets: %1 -> %2")
                  .arg(before)
                  .arg(view->zoom()));
        shot(&win, QStringLiteral("side-by-side"));

        // The four are one exclusive choice, so side by side is on only while
        // none of the single-sheet views is, and picking one is how a reader
        // leaves it. Two of them checked at once would say the window is
        // showing two things at once.
        check(!actionChecked(win, "overlay") && !actionChecked(win, "onlyA") && !actionChecked(win, "onlyB"),
              QStringLiteral("side by side is checked instead of a single view"));
        win.findChild<QAction *>(QStringLiteral("overlay"))->trigger();
        QTest::qWait(50);
        check(view->layout() == CompareView::Layout::Single,
              QStringLiteral("and choosing a single view goes back to one sheet"));
        check(!sbs->isChecked(), QStringLiteral("and unchecks side by side"));
    }

    // Fading the drawing the two revisions agree on, until only the changes are
    // left. The assertions are on the pixels: this is a control whose entire
    // job is what the sheet looks like, and the status line saying "40%" would
    // prove nothing about that.
    {
        auto *slider = win.findChild<QSlider *>(QStringLiteral("fadeSlider"));
        auto *fade = win.findChild<QAction *>(QStringLiteral("fade"));
        check(slider && fade, QStringLiteral("there is a control for the unchanged drawing"));

        // Neutral dark ink is the drawing both revisions share; anything with
        // chroma in it is a difference.
        auto counts = [](const QImage &img, int *neutral, int *coloured) {
            *neutral = 0;
            *coloured = 0;
            for (int y = 0; y < img.height(); y++) {
                for (int x = 0; x < img.width(); x++) {
                    const QColor c = img.pixelColor(x, y);
                    const int lo = qMin(qMin(c.red(), c.green()), c.blue());
                    const int hi = qMax(qMax(c.red(), c.green()), c.blue());
                    if (hi - lo > 40) {
                        (*coloured)++;
                    } else if (lo < 128) {
                        (*neutral)++;
                    }
                }
            }
        };
        const QSize dev = s->pageDeviceSize(1, 1.0);
        const QRect whole(QPoint(0, 0), dev);
        int neutralFull = 0;
        int colouredFull = 0;
        counts(s->tile(1, 1.0, whole, SC_VIEW_MODE_OVERLAY), &neutralFull, &colouredFull);
        check(neutralFull > 0 && colouredFull > 0,
              QStringLiteral("the overlay starts with artwork and changes on it: %1 / %2")
                  .arg(neutralFull)
                  .arg(colouredFull));

        slider->setValue(0);
        QTest::qWait(50);
        check(s->sharedInk() == 0, QStringLiteral("the slider empties the drawing"));
        int neutralGone = 0;
        int colouredGone = 0;
        counts(s->tile(1, 1.0, whole, SC_VIEW_MODE_OVERLAY), &neutralGone, &colouredGone);
        check(neutralGone * 20 < neutralFull,
              QStringLiteral("the shared artwork goes: %1 of %2 left")
                  .arg(neutralGone)
                  .arg(neutralFull));
        // Never fewer. It can be more: where a difference sat on top of ink
        // both revisions drew, the pixel was dark enough to read as artwork
        // and comes out plainly coloured once the artwork under it is gone.
        // Which is the point of the control.
        check(colouredGone >= colouredFull,
              QStringLiteral("and no difference is faded with it: %1 against %2")
                  .arg(colouredGone)
                  .arg(colouredFull));
        check(status->text().contains(QStringLiteral("unchanged drawing at 0%")),
              QStringLiteral("and the window says why the sheet is empty: '%1'")
                  .arg(status->text()));
        shot(&win, QStringLiteral("faded"));

        // The button is the one a reader uses: a quarter at a time, and from
        // nothing back to the whole drawing.
        fade->trigger();
        QTest::qWait(20);
        check(s->sharedInk() == 100, QStringLiteral("from nothing it comes back whole"));
        fade->trigger();
        QTest::qWait(20);
        check(s->sharedInk() == 75, QStringLiteral("then a quarter at a time, got %1")
                                        .arg(s->sharedInk()));
        fade->trigger();
        fade->trigger();
        QTest::qWait(20);
        check(s->sharedInk() == 25, QStringLiteral("and again, got %1").arg(s->sharedInk()));
        check(slider->value() == 25, QStringLiteral("with the slider following the button"));

        // It only means anything on the overlay, so it is not live anywhere
        // else. A control that is enabled and does nothing is exactly the
        // complaint that started all this.
        win.findChild<QAction *>(QStringLiteral("onlyA"))->trigger();
        QTest::qWait(20);
        check(!slider->isEnabled() && !fade->isEnabled(),
              QStringLiteral("a single revision has nothing to fade"));
        win.findChild<QAction *>(QStringLiteral("sideBySide"))->setChecked(true);
        QTest::qWait(20);
        check(!slider->isEnabled(), QStringLiteral("nor has side by side"));
        win.findChild<QAction *>(QStringLiteral("overlay"))->trigger();
        QTest::qWait(20);
        check(slider->isEnabled() && fade->isEnabled(),
              QStringLiteral("and it comes back with the overlay"));

        slider->setValue(100);
        QTest::qWait(20);
    }

    // Tolerance, on the bar. It is the one setting that changes every answer
    // this tool gives, and the right value is found by turning it up until the
    // fringe goes — which is not something to do through a menu.
    {
        auto *tol = win.findChild<QSpinBox *>(QStringLiteral("toleranceBox"));
        check(tol != nullptr, QStringLiteral("the toolbar has a tolerance control"));
        check(tol->maximum() == Session::maxTolerance(),
              QStringLiteral("reaching as far as the core allows, %1 of %2")
                  .arg(tol->maximum())
                  .arg(Session::maxTolerance()));
        check(tol->value() == s->tolerance(),
              QStringLiteral("and showing what is in force, %1 of %2")
                  .arg(tol->value())
                  .arg(s->tolerance()));

        const int above = Session::toleranceHidesMovement() + 1;
        tol->setValue(above);
        QTest::qWait(50);
        check(s->tolerance() == above,
              QStringLiteral("the control sets it, got %1").arg(s->tolerance()));
        // Above the line it hides small movements, and the reader is told so
        // next to the counts it produced rather than left to remember it.
        check(status->text().contains(QStringLiteral("only moved")),
              QStringLiteral("and warns what that costs: '%1'").arg(status->text()));
        check(s->report().contains(QStringLiteral("merely moved")),
              QStringLiteral("as does the report, which is read without the window"));

        // Changing it throws every scanned answer away, so the set is swept
        // again by itself. Leaving that to the reader empties the sidebar and
        // says nothing about why.
        QTest::qWait(600);
        check(waitForSweep(s), QStringLiteral("changing the tolerance sweeps the set again"));
        check(status->text().contains(QStringLiteral("sheets changed")),
              QStringLiteral("without the reader asking for it: '%1'").arg(status->text()));

        // Asking for more than the ceiling must not throw a finished sweep away
        // to arrive at the number it was already at.
        tol->setValue(Session::maxTolerance());
        QTest::qWait(600);
        check(waitForSweep(s), QStringLiteral("the sweep at the ceiling finishes"));
        check(s->changeCount(1) >= 0, QStringLiteral("sheet 1 has been scanned"));
        s->setTolerance(Session::maxTolerance() + 5);
        QTest::qWait(100);
        check(s->tolerance() == Session::maxTolerance(),
              QStringLiteral("asking for more than the ceiling is clamped"));
        check(s->changeCount(1) >= 0,
              QStringLiteral("and keeps the scan it already had, got %1").arg(s->changeCount(1)));

        // And coming back down finds everything again, by itself.
        tol->setValue(1);
        QTest::qWait(600);
        check(waitForSweep(s), QStringLiteral("and back down again"));
        check(sheets->topLevelItemCount() == 6,
              QStringLiteral("the sidebar fills back in, got %1")
                  .arg(sheets->topLevelItemCount()));
        check(!status->text().contains(QStringLiteral("only moved")),
              QStringLiteral("and the warning goes with it: '%1'").arg(status->text()));
    }

    // One sheet at a time: the whole sheet on screen and nothing to scroll.
    // A set is flipped through as often as it is read, and scrolling is the
    // thing in the way when you are looking for the sheet that changed.
    {
        auto *one = win.findChild<QAction *>(QStringLiteral("singlePage"));
        check(one != nullptr, QStringLiteral("there is a single-page action"));
        view->goToPage(3);
        QTest::qWait(50);
        one->setChecked(true);
        QTest::qWait(50);
        check(view->flow() == CompareView::Flow::SinglePage,
              QStringLiteral("the viewport shows one sheet"));
        check(view->currentPage() == 3,
              QStringLiteral("and keeps the sheet the reader was on, got %1")
                  .arg(view->currentPage()));
        check(status->text().contains(QStringLiteral("one sheet")),
              QStringLiteral("and says so: '%1'").arg(status->text()));

        // The two halves of what this view is. Nothing scrolls...
        check(view->verticalScrollBar()->maximum() == 0 &&
                  view->horizontalScrollBar()->maximum() == 0,
              QStringLiteral("there is nothing to scroll, %1 x %2")
                  .arg(view->horizontalScrollBar()->maximum())
                  .arg(view->verticalScrollBar()->maximum()));
        // ...because the whole sheet is on screen. Measured against the
        // viewport rather than taken from the fit: a fit that stopped fitting
        // is exactly the failure this would have to catch.
        auto sheetFits = [&] {
            const QSize sheet = view->session()->pageDeviceSize(view->currentPage(), view->zoom());
            const QSize vp = view->viewport()->size();
            return sheet.width() <= vp.width() && sheet.height() <= vp.height();
        };
        check(sheetFits(), QStringLiteral("the whole sheet is on screen"));
        shot(&win, QStringLiteral("single-page"));

        // Stepping through the set, by every route a hand takes.
        QTest::keyClick(view, Qt::Key_PageDown);
        QTest::qWait(50);
        check(view->currentPage() == 4,
              QStringLiteral("PageDown steps a sheet, got %1").arg(view->currentPage()));
        check(sheetFits(), QStringLiteral("and it is whole too"));
        QTest::keyClick(view, Qt::Key_PageUp);
        QTest::qWait(50);
        check(view->currentPage() == 3, QStringLiteral("PageUp comes back"));
        QTest::keyClick(view, Qt::Key_End);
        QTest::qWait(50);
        check(view->currentPage() == view->session()->pageCount(),
              QStringLiteral("End goes to the last sheet, got %1").arg(view->currentPage()));
        QTest::keyClick(view, Qt::Key_Home);
        QTest::qWait(50);
        check(view->currentPage() == 1, QStringLiteral("and Home to the first"));

        // The wheel has nothing to scroll, so it turns sheets. One notch, one
        // sheet — a touchpad sends fractions of a notch and would otherwise
        // send a whole set past in a flick.
        auto spin = [&](int notches) {
            QWheelEvent w(QPointF(200, 200), view->viewport()->mapToGlobal(QPoint(200, 200)),
                          QPoint(0, 0), QPoint(0, notches * 120), Qt::NoButton, Qt::NoModifier,
                          Qt::NoScrollPhase, false);
            QApplication::sendEvent(view->viewport(), &w);
            QTest::qWait(30);
        };
        spin(-1);
        check(view->currentPage() == 2,
              QStringLiteral("a notch down turns to the next sheet, got %1")
                  .arg(view->currentPage()));
        spin(1);
        check(view->currentPage() == 1, QStringLiteral("and back up again"));

        // A zoom is a request this view cannot honour, so it leaves — visibly,
        // with the button unchecking itself. The alternative is a single sheet
        // that has to be scrolled around, which is what it exists to remove.
        view->setZoom(view->zoom() * 2.0);
        QTest::qWait(50);
        check(view->flow() == CompareView::Flow::Continuous,
              QStringLiteral("zooming in leaves the one-sheet view"));
        check(!one->isChecked(), QStringLiteral("and the button says so"));

        // One sheet at a time and side by side are different questions, so
        // asking one must not answer the other.
        one->setChecked(true);
        QTest::qWait(50);
        win.findChild<QAction *>(QStringLiteral("sideBySide"))->setChecked(true);
        QTest::qWait(50);
        check(view->flow() == CompareView::Flow::SinglePage && one->isChecked(),
              QStringLiteral("side by side leaves the single sheet alone"));
        check(view->layout() == CompareView::Layout::SideBySide,
              QStringLiteral("and both are in force at once"));
        check(view->verticalScrollBar()->maximum() == 0,
              QStringLiteral("with both sheets whole and still nothing to scroll"));
        shot(&win, QStringLiteral("single-page-side-by-side"));

        // Stepping through the changes has to cross sheets, and the sheet it is
        // going to is not laid out yet. This shipped broken once for the same
        // reason in another guise: the answer was found and there was nowhere
        // on screen to point at it.
        win.findChild<QAction *>(QStringLiteral("overlay"))->trigger();
        QTest::qWait(50);
        one->setChecked(true);
        view->goToPage(1);
        QTest::qWait(50);
        const int wasOn = view->currentPage();
        auto *step = win.findChild<QAction *>(QStringLiteral("next"));
        for (int i = 0; i < 40 && view->currentPage() == wasOn; i++) {
            step->trigger();
            QTest::qWait(10);
        }
        check(view->currentPage() != wasOn,
              QStringLiteral("stepping to a change on another sheet brings that sheet up"));
        check(view->flow() == CompareView::Flow::SinglePage,
              QStringLiteral("without leaving the view, since the zoom never moved"));

        one->setChecked(false);
        QTest::qWait(50);
        check(view->flow() == CompareView::Flow::Continuous,
              QStringLiteral("and the scroll runs through the set again"));
        check(view->verticalScrollBar()->maximum() > 0,
              QStringLiteral("with the scrollbar back"));
    }

    // Printing, checked by printing to a PDF and reading back what came out.
    // A reviewer's other output is paper: the overlay of the sheets that
    // changed, to mark up by hand.
    {
        const QString pdf = QDir::temp().filePath(QStringLiteral("sch-print-test.pdf"));
        QFile::remove(pdf);
        QPrinter printer(QPrinter::HighResolution);
        printer.setOutputFormat(QPrinter::PdfFormat);
        printer.setOutputFileName(pdf);
        const QVector<int> changed = win.changedSheetList();
        check(!changed.isEmpty(), QStringLiteral("there are changed sheets to print"));
        // Two of them is enough to prove the page break works without spending
        // twenty seconds rendering the whole set at print resolution.
        const QVector<int> few = {changed.first(), changed.last()};
        check(win.printTo(printer, few) == 2, QStringLiteral("both sheets printed"));
        check(QFileInfo(pdf).size() > 0, QStringLiteral("and produced a file"));

        // Read it back with our own engine: two pages, and they are not blank.
        const QByteArray p8 = pdf.toUtf8();
        ScSession *check_doc = sc_session_open(p8.constData(), p8.constData());
        check(check_doc != nullptr, QStringLiteral("the printed PDF opens"));
        if (check_doc) {
            check(sc_session_page_count(check_doc) == 2,
                  QStringLiteral("with one page per sheet, got %1")
                      .arg(sc_session_page_count(check_doc)));
            sc_session_free(check_doc);
        }
        if (!writeDir.isEmpty()) {
            QFile::copy(pdf, QDir(writeDir).filePath(QStringLiteral("printed.pdf")));
        }
        QFile::remove(pdf);
    }

    // `--for-testing` must not have written anything, whatever else happened
    // above — and plenty above excluded regions and changed the tolerance.
    check(!settingsFile.isEmpty(), QStringLiteral("the core says where its settings go"));
    check(!QFileInfo::exists(settingsFile),
          QStringLiteral("--for-testing wrote no settings"));

    // And a normal run must. Excluded regions are the reason any of this
    // exists: working out where a set's title block is costs a reviewer a
    // minute, and losing it every session makes the feature not worth using.
    {
        MainWindow real;
        real.resize(900, 700);
        real.show();
        check(real.openPair(a, b), QStringLiteral("a normal run opens the pair"));
        Session *rs = real.findChild<CompareView *>()->session();
        const QRectF block(600.0, 570.0, 200.0, 25.0);
        rs->addIgnoreRect(block);
        QTest::qWait(50);
        real.close();
        QTest::qWait(50);
        check(QFileInfo::exists(settingsFile),
              QStringLiteral("a normal run saves them, at %1").arg(settingsFile));
    }
    {
        MainWindow again;
        again.resize(900, 700);
        again.show();
        check(again.openPair(a, b), QStringLiteral("and reopening the pair"));
        Session *rs = again.findChild<CompareView *>()->session();
        check(rs->ignoreRects().size() == 1,
              QStringLiteral("brings the excluded region back, got %1")
                  .arg(rs->ignoreRects().size()));

        QString la;
        QString lb;
        check(Session::lastPair(&la, &lb) && la == a && lb == b,
              QStringLiteral("and the pair itself is remembered for reopening"));
        again.close();
    }
    QDir(cfg).removeRecursively();

    // And the real drawing sets, when this machine has them. Everything above
    // has already run; this is the check that it also holds at 21 sheets of
    // dense schematic rather than 3 sheets of fixture.
    const QString realA = sample(QStringLiteral("same_producer"), QStringLiteral("a"));
    const QString realB = sample(QStringLiteral("same_producer"), QStringLiteral("b"));
    if (!realA.isEmpty() && !realB.isEmpty()) {
        MainWindow real;
        real.setForTesting(true);
        real.resize(1400, 950);
        real.show();
        check(real.openPair(realA, realB), QStringLiteral("the real pair opens"));
        auto *rv = real.findChild<CompareView *>();
        auto *rs = real.findChild<QTreeWidget *>(QStringLiteral("sheets"));
        check(waitForSweep(rv->session()), QStringLiteral("its sweep finishes"));
        const int sheets = expected(QStringLiteral("same_producer"), QStringLiteral("sheets"));
        check(rs->topLevelItemCount() == sheets,
              QStringLiteral("all %1 sheets are listed, got %2")
                  .arg(sheets)
                  .arg(rs->topLevelItemCount()));
        const QString rmd = rv->session()->report();
        check(!rmd.isEmpty() && rmd.contains(QStringLiteral("## Sheet")),
              QStringLiteral("and the report has the real set's sheets in it"));

        // The fade, on a sheet of real line work rather than a fixture with
        // four strings on it. This is the case it exists for: the drawing is
        // 2.6% of the sheet and the change is a tenth of a per cent of it.
        Session *rsn = rv->session();
        const QSize dev = rsn->pageDeviceSize(2, 1.0);
        auto marks = [](const QImage &img, int *neutral, int *coloured) {
            *neutral = 0;
            *coloured = 0;
            for (int y = 0; y < img.height(); y++) {
                for (int x = 0; x < img.width(); x++) {
                    const QColor c = img.pixelColor(x, y);
                    const int lo = qMin(qMin(c.red(), c.green()), c.blue());
                    const int hi = qMax(qMax(c.red(), c.green()), c.blue());
                    if (hi - lo > 40) {
                        (*coloured)++;
                    } else if (lo < 128) {
                        (*neutral)++;
                    }
                }
            }
        };
        int drawnFull = 0;
        int changedFull = 0;
        marks(rsn->tile(2, 1.0, QRect(QPoint(0, 0), dev), SC_VIEW_MODE_OVERLAY), &drawnFull,
              &changedFull);
        rsn->setSharedInk(0);
        int drawnGone = 0;
        int changedGone = 0;
        marks(rsn->tile(2, 1.0, QRect(QPoint(0, 0), dev), SC_VIEW_MODE_OVERLAY), &drawnGone,
              &changedGone);
        rsn->setSharedInk(100);
        check(drawnFull > 1000 && changedFull > 0,
              QStringLiteral("the real sheet is dense and has changes on it: %1 / %2")
                  .arg(drawnFull)
                  .arg(changedFull));
        check(drawnGone * 50 < drawnFull,
              QStringLiteral("fading empties it: %1 of %2 left").arg(drawnGone).arg(drawnFull));
        check(changedGone >= changedFull,
              QStringLiteral("and every change is still there: %1 against %2")
                  .arg(changedGone)
                  .arg(changedFull));
        shot(&real, QStringLiteral("real-set"));
        real.close();
    } else {
        printf("view: (samples absent; the real-set pass was skipped)\n");
    }

    if (failures == 0) {
        printf("view: ok\n");
    }
    return failures == 0 ? 0 : 1;
}
