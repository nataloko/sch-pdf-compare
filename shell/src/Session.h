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

    QString lastError() const;

  signals:
    // The model changed and everything drawn from it is stale.
    void invalidated();

  private:
    explicit Session(ScSession *s, QString pathA, QString pathB, QObject *parent);

    ScSession *m_s = nullptr;
    QString m_pathA;
    QString m_pathB;
};
