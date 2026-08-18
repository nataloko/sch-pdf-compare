// Choosing the two overlay colours.
//
// Not a preference. Everything this tool shows is carried by two colours, and
// the default pair is red and green — which a reader with red-green colour
// blindness cannot tell apart. Until this existed, changing them meant editing
// the settings file by hand.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include <QColor>
#include <QDialog>

class QPushButton;

class ColourDialog : public QDialog {
    Q_OBJECT

  public:
    ColourDialog(const QColor &onlyA, const QColor &onlyB, QWidget *parent = nullptr);

    QColor onlyA() const { return m_a; }
    QColor onlyB() const { return m_b; }

    // The pair a reader with red-green colour blindness can use. Blue and
    // orange stay distinct under every common form of it, which red and green
    // do not.
    static QColor accessibleA() { return QColor(0x00, 0x40, 0xff); }
    static QColor accessibleB() { return QColor(0xff, 0x80, 0x00); }
    static QColor defaultA() { return QColor(0xd8, 0x10, 0x10); }
    static QColor defaultB() { return QColor(0x00, 0x96, 0x28); }

  private:
    void choose(QColor *slot, QPushButton *button, const QString &title);
    void restyle();

    QColor m_a;
    QColor m_b;
    QPushButton *m_buttonA = nullptr;
    QPushButton *m_buttonB = nullptr;
};
