// The C ABI, wrapped in something Qt can connect to.
//
// One QObject owns one ScSession and is the only place in the shell that
// touches the core's C functions. Everything above it deals in QImage, QRectF
// and signals.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include <QImage>
#include <QObject>
#include <QRectF>
#include <QSizeF>
#include <QString>
#include <QVector>

extern "C" {
#include "schcompare.h"
}

#ifdef Q_OS_WIN
class QWinEventNotifier;
using WakeupNotifier = QWinEventNotifier;
#else
class QSocketNotifier;
using WakeupNotifier = QSocketNotifier;
#endif

class Session : public QObject {
    Q_OBJECT

  public:
    // Returns null and fills `error` when the pair cannot be opened.
    static Session *open(const QString &pathA, const QString &pathB, QString *error,
                         QObject *parent = nullptr);
    ~Session() override;

    QString pathA() const { return m_pathA; }
    QString pathB() const { return m_pathB; }

    // Virtual sheets, covering both documents. Not either one's own count.
    int pageCount() const;
    // Which sheet of each document a virtual page stands for; 0 for "no such
    // sheet in this revision".
    QPair<int, int> pair(int page) const;

    QSizeF pageSize(int page) const;              // points
    QSize pageDeviceSize(int page, double zoom) const;

    // A composed tile. The core lends its pixels only until the next call, so
    // this copies — the view caches tiles and outlives that promise. Drawing
    // straight through without caching would not need the copy.
    QImage tile(int page, double zoom, const QRect &r) const;

    ScViewMode viewMode() const;
    void setViewMode(ScViewMode m);

    int tolerance() const;
    void setTolerance(int t);

    int pageDelta() const;
    void setPageDelta(int d);
    // Matches the sheets by what is written on them. Replaces any delta.
    bool autoMatch();
    bool pairingIsAutomatic() const;

    // Reads back what was worked out for this pair last time, and stores it.
    // The caller decides whether to: a run started --for-testing does neither.
    void loadSettings();
    bool saveSettings();
    // The pair compared most recently, or false if there is none.
    static bool lastPair(QString *a, QString *b);

    QVector<QRectF> ignoreRects() const;
    void addIgnoreRect(const QRectF &r);
    void clearIgnoreRects();

    // Scans a sheet and caches it in the core. Returns the number of change
    // regions, or -1 on failure.
    int scanPage(int page);
    // -1 when the sheet has not been scanned yet, which is not 0.
    int changeCount(int page) const;
    int ignoredCount(int page) const;
    QRectF change(int page, int index) const;

    // One difference in what the two revisions of a sheet say.
    struct TextChange {
        ScTextChangeKind kind;
        QString before;
        QString after;
        QRectF rect;
    };
    // What this sheet says differently. Complements the overlay: the overlay
    // finds a re-routed wire, this finds a value that went from 10k to 12k.
    QVector<TextChange> textChanges(int page);

    // Starts scanning every sheet on a worker thread. Progress arrives as
    // signals, driven by the core's wakeup handle — there is no timer in this.
    void startSweep();
    void stopSweep();
    ScSweepStatus sweepStatus() const;
    // True once the sweep has finished *and* its last sheets have been
    // collected on this thread.
    //
    // Not the same as `sweepStatus().finished`, which is the worker's view.
    // Between the two the results exist but nothing on screen has been rebuilt
    // from them, and anything that reads the sidebar in that gap sees the
    // second-to-last answer.
    bool sweepCollected() const;
    // Regions that recur across the set. Offered, never applied.
    QVector<QRectF> suggestedRegions() const;

    // Drives one collection by hand, for tests that must not race the loop.
    void pumpForTest();

    QString lastError() const;

  signals:
    // A sheet's scan arrived, or the sweep finished. Cheap; fires per sheet.
    void sweepProgressed();
    // The model changed and everything drawn from it is stale.
    void invalidated();

  private:
    explicit Session(ScSession *s, QString pathA, QString pathB, QObject *parent);

  private slots:
    void onWakeup();

  private:
    void dropNotifier();

    ScSession *m_s = nullptr;
    QString m_pathA;
    QString m_pathB;
    // Watches the core's wakeup handle. Must not outlive the sweep that owns
    // the handle, which is why stopping the sweep drops this first.
    WakeupNotifier *m_notifier = nullptr;
};
