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
#include "CompareView.h"
#include "MainWindow.h"
#include "Session.h"

#include <QAction>
#include <QApplication>
#include <QDir>
#include <QFileInfo>
#include <QLabel>
#include <QTest>
#include <QTreeWidget>

static int failures = 0;
static QString writeDir;

static void check(bool ok, const QString &what) {
    if (!ok) {
        fprintf(stderr, "FAIL %s\n", qPrintable(what));
        failures++;
    }
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

static QString sample(const QString &name) {
    const QString p = QFileInfo(QStringLiteral(SC_SOURCE_DIR "/../../samples/") + name).absoluteFilePath();
    return QFileInfo::exists(p) ? p : QString();
}

int main(int argc, char **argv) {
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
    shot(&win, QStringLiteral("empty"));

    const QString a = sample(QStringLiteral("SET-ONE - EXAMPLE DIGITAL REV-P1.pdf"));
    const QString b = sample(QStringLiteral("SET-ONE - EXAMPLE DIGITAL REV-P2.pdf"));
    if (a.isEmpty() || b.isEmpty()) {
        printf("view: ok (no samples; opened-document checks skipped)\n");
        return failures == 0 ? 0 : 1;
    }

    check(win.openPair(a, b), QStringLiteral("the pair opens"));
    QTest::qWait(50);
    check(status->text().contains(QStringLiteral("of 21")),
          QStringLiteral("21 virtual sheets: '%1'").arg(status->text()));
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
    check(sheets->topLevelItemCount() == 21,
          QStringLiteral("every sheet is listed, got %1").arg(sheets->topLevelItemCount()));
    check(status->text().contains(QStringLiteral("21 sheets changed")),
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
    win.findChild<QAction *>(QStringLiteral("onlyB"))->trigger();
    QTest::qWait(20);
    check(status->text().contains(QStringLiteral("B only")), QStringLiteral("B only"));
    shot(&win, QStringLiteral("only-b"));
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
    check(sheets->topLevelItemCount() < 21,
          QStringLiteral("excluding the title block clears sheets, %1 left")
              .arg(sheets->topLevelItemCount()));
    check(status->text().contains(QStringLiteral("excluded")),
          QStringLiteral("and the exclusion is on show: '%1'").arg(status->text()));
    shot(&win, QStringLiteral("title-block-excluded"));

    win.findChild<QAction *>(QStringLiteral("clearRegions"))->trigger();
    QTest::qWait(20);
    check(s->ignoreRects().isEmpty(), QStringLiteral("regions clear"));

    // Nudging the pairing changes which sheets face each other.
    win.findChild<QAction *>(QStringLiteral("shiftRight"))->trigger();
    QTest::qWait(20);
    check(s->pageDelta() == 1, QStringLiteral("pairing shifted"));
    check(s->pair(1).first == 0, QStringLiteral("and sheet 1 of A now has no counterpart"));
    win.findChild<QAction *>(QStringLiteral("shiftLeft"))->trigger();
    QTest::qWait(20);
    check(s->pageDelta() == 0, QStringLiteral("and shifts back"));

    // Matching by content lines these sets up one to one, and says it did.
    win.findChild<QAction *>(QStringLiteral("autoMatch"))->trigger();
    QTest::qWait(50);
    check(s->pairingIsAutomatic(), QStringLiteral("the pairing is a content match"));
    check(s->pair(7).first == 7 && s->pair(7).second == 7,
          QStringLiteral("and these sets match one to one"));
    check(status->text().contains(QStringLiteral("matched by content")),
          QStringLiteral("and the status line says which pairing is in force: '%1'")
              .arg(status->text()));
    check(waitForSweep(s), QStringLiteral("and the sweep runs again on the new pairing"));
    shot(&win, QStringLiteral("matched"));

    if (failures == 0) {
        printf("view: ok\n");
    }
    return failures == 0 ? 0 : 1;
}
