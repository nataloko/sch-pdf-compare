// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "Session.h"

Session::Session(ScSession *s, QString pathA, QString pathB, QObject *parent)
    : QObject(parent), m_s(s), m_pathA(std::move(pathA)), m_pathB(std::move(pathB)) {
}

Session::~Session() {
    sc_session_free(m_s);
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
