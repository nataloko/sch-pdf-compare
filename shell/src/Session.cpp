// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "Session.h"

#ifdef Q_OS_WIN
#include <QWinEventNotifier>
#else
#include <QSocketNotifier>
#endif

Session::Session(ScSession *s, QString pathA, QString pathB, QObject *parent)
    : QObject(parent), m_s(s), m_pathA(std::move(pathA)), m_pathB(std::move(pathB)) {
}

Session::~Session() {
    // Order matters: the notifier watches a handle the sweep owns, and the
    // sweep's thread holds its own documents. Tear down outside-in.
    stopSweep();
    sc_session_free(m_s);
}

void Session::dropNotifier() {
    if (!m_notifier) {
        return;
    }
    // Disabled first and deleted later, never `delete` outright: this is called
    // from inside the notifier's own activated slot when the sweep finishes,
    // and destroying the object that is currently emitting cuts the emission
    // short — which cost the last sheet of every sweep and left the status line
    // reading "scanning 20 of 21" forever.
    m_notifier->setEnabled(false);
    m_notifier->deleteLater();
    m_notifier = nullptr;
}

void Session::startSweep() {
    stopSweep();
    if (sc_session_start_sweep(m_s) < 0) {
        return;
    }
    const int64_t h = sc_session_wakeup_handle(m_s);
    if (h < 0) {
        return;
    }
#ifdef Q_OS_WIN
    auto *n = new QWinEventNotifier(reinterpret_cast<Qt::HANDLE>(h), this);
    connect(n, &QWinEventNotifier::activated, this, &Session::onWakeup);
#else
    auto *n = new QSocketNotifier(int(h), QSocketNotifier::Read, this);
    connect(n, &QSocketNotifier::activated, this, &Session::onWakeup);
#endif
    m_notifier = n;
    // The sweep may already have finished a sheet before the notifier existed,
    // and a wakeup delivered into that gap is simply gone. One read now covers
    // it; every later one is driven by the handle.
    onWakeup();
}

void Session::stopSweep() {
    dropNotifier();
    sc_session_stop_sweep(m_s);
}

void Session::onWakeup() {
    // Collect on this thread, then tell the window. The sweep never touches
    // anything the UI owns.
    const ScStatus st = sc_session_pump(m_s);
    if (qEnvironmentVariableIsSet("SC_DEBUG_SWEEP")) {
        const ScSweepStatus d = sweepStatus();
        fprintf(stderr, "[wake] pump=%d scanned=%d/%d running=%d finished=%d notifier=%p\n",
                st, d.scanned, d.total, int(d.running), int(d.finished), (void *)m_notifier);
    }
    if (st == SC_OK) {
        // Finished. Nothing more will signal, so stop watching a handle that is
        // about to go away.
        dropNotifier();
    }
    emit sweepProgressed();
}

void Session::pumpForTest() {
    onWakeup();
}

ScSweepStatus Session::sweepStatus() const {
    ScSweepStatus s{};
    sc_session_sweep_status(m_s, &s);
    return s;
}

QVector<Session::TextChange> Session::textChanges(int page) {
    QVector<TextChange> out;
    const int n = sc_session_text_changes(m_s, page);
    if (n <= 0) {
        return out;
    }
    out.reserve(n);
    for (int i = 0; i < n; i++) {
        ScTextChange c;
        if (sc_session_text_change(m_s, size_t(i), &c) != SC_OK) {
            continue;
        }
        // Copied out here, deliberately: the core lends these strings only
        // until the next call, and this vector outlives that.
        out.append({c.kind, QString::fromUtf8(c.before), QString::fromUtf8(c.after),
                    QRectF(c.rect.x, c.rect.y, c.rect.dx, c.rect.dy)});
    }
    return out;
}

bool Session::sweepCollected() const {
    // The notifier is dropped by `onWakeup` exactly when the pump reports that
    // it has taken the last results and the sweep is done, so its absence is
    // the collection having happened.
    return sweepStatus().finished && m_notifier == nullptr;
}

QVector<QRectF> Session::suggestedRegions() const {
    QVector<QRectF> out;
    const int n = sc_session_suggested_count(m_s);
    out.reserve(n);
    for (int i = 0; i < n; i++) {
        ScRectF r;
        if (sc_session_suggested(m_s, size_t(i), &r) == SC_OK) {
            out.append(QRectF(r.x, r.y, r.dx, r.dy));
        }
    }
    return out;
}

Session *Session::open(const QString &pathA, const QString &pathB, QString *error,
                       QObject *parent) {
    const QByteArray a = pathA.toUtf8();
    const QByteArray b = pathB.toUtf8();
    ScSession *s = sc_session_open(a.constData(), b.constData());
    if (!s) {
        if (error) {
            *error = QString::fromUtf8(sc_last_error());
        }
        return nullptr;
    }
    return new Session(s, pathA, pathB, parent);
}

QString Session::lastError() const {
    return QString::fromUtf8(sc_last_error());
}

int Session::pageCount() const {
    return sc_session_page_count(m_s);
}

QPair<int, int> Session::pair(int page) const {
    const ScPair p = sc_session_pair(m_s, page);
    return {p.page_a, p.page_b};
}

QSizeF Session::pageSize(int page) const {
    float w = 0, h = 0;
    if (sc_session_page_size(m_s, page, &w, &h) < 0) {
        return {};
    }
    return {w, h};
}

QSize Session::pageDeviceSize(int page, double zoom) const {
    int32_t w = 0, h = 0;
    if (sc_session_page_device_size(m_s, page, float(zoom), &w, &h) < 0) {
        return {};
    }
    return {w, h};
}

QImage Session::tile(int page, double zoom, const QRect &r) const {
    ScTile t;
    if (sc_session_tile(m_s, page, float(zoom), r.x(), r.y(), r.width(), r.height(), &t) < 0) {
        return {};
    }
    // Format_RGB32 is 0xffRRGGBB in a quint32, which on a little-endian host is
    // B, G, R, A in memory — exactly what the core composes. No swizzle, and the
    // wrap itself is free; only the copy below costs anything.
    const QImage borrowed(t.pixels, t.width, t.height, qsizetype(t.stride), QImage::Format_RGB32);
    return borrowed.copy();
}

ScViewMode Session::viewMode() const {
    return sc_session_view_mode(m_s);
}

void Session::setViewMode(ScViewMode m) {
    if (m == viewMode()) {
        return;
    }
    sc_session_set_view_mode(m_s, m);
    emit invalidated();
}

int Session::tolerance() const {
    return sc_session_tolerance(m_s);
}

void Session::setTolerance(int t) {
    if (t == tolerance()) {
        return;
    }
    sc_session_set_tolerance(m_s, t);
    emit invalidated();
}

int Session::pageDelta() const {
    return sc_session_page_delta(m_s);
}

void Session::setPageDelta(int d) {
    if (d == pageDelta()) {
        return;
    }
    sc_session_set_page_delta(m_s, d);
    emit invalidated();
}

bool Session::autoMatch() {
    if (sc_session_auto_match(m_s) < 0) {
        return false;
    }
    emit invalidated();
    return true;
}

bool Session::pairingIsAutomatic() const {
    return sc_session_pairing_is_automatic(m_s);
}

QVector<QRectF> Session::ignoreRects() const {
    QVector<QRectF> out;
    const size_t n = sc_session_ignore_rect_count(m_s);
    out.reserve(int(n));
    for (size_t i = 0; i < n; i++) {
        ScRectF r;
        if (sc_session_ignore_rect(m_s, i, &r) == SC_OK) {
            out.append(QRectF(r.x, r.y, r.dx, r.dy));
        }
    }
    return out;
}

void Session::addIgnoreRect(const QRectF &r) {
    sc_session_add_ignore_rect(m_s, float(r.x()), float(r.y()), float(r.width()),
                               float(r.height()));
    emit invalidated();
}

void Session::clearIgnoreRects() {
    sc_session_clear_ignore_rects(m_s);
    emit invalidated();
}

int Session::scanPage(int page) {
    if (sc_session_scan_page(m_s, page) < 0) {
        return -1;
    }
    return sc_session_change_count(m_s, page);
}

int Session::changeCount(int page) const {
    return sc_session_change_count(m_s, page);
}

int Session::ignoredCount(int page) const {
    return sc_session_ignored_count(m_s, page);
}

QRectF Session::change(int page, int index) const {
    ScRectF r;
    if (sc_session_change(m_s, page, size_t(index), &r) < 0) {
        return {};
    }
    return QRectF(r.x, r.y, r.dx, r.dy);
}
