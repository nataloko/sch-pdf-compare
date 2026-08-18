// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "ColourDialog.h"

#include <QColorDialog>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

ColourDialog::ColourDialog(const QColor &onlyA, const QColor &onlyB, QWidget *parent)
    : QDialog(parent), m_a(onlyA), m_b(onlyB) {
    setWindowTitle(tr("Overlay colours"));

    auto *top = new QVBoxLayout(this);
    auto *why = new QLabel(
        tr("The overlay shows content that is only in one revision in these two "
           "colours. Shared content stays black."),
        this);
    why->setWordWrap(true);
    top->addWidget(why);

    auto *form = new QFormLayout;
    m_buttonA = new QPushButton(this);
    m_buttonA->setObjectName(QStringLiteral("colourA"));
    m_buttonB = new QPushButton(this);
    m_buttonB->setObjectName(QStringLiteral("colourB"));
    form->addRow(tr("Only in the earlier revision:"), m_buttonA);
    form->addRow(tr("Only in the later revision:"), m_buttonB);
    top->addLayout(form);

    connect(m_buttonA, &QPushButton::clicked, this, [this] {
        choose(&m_a, m_buttonA, tr("Colour for the earlier revision"));
    });
    connect(m_buttonB, &QPushButton::clicked, this, [this] {
        choose(&m_b, m_buttonB, tr("Colour for the later revision"));
    });

    auto *presets = new QDialogButtonBox(this);
    QPushButton *accessible = presets->addButton(tr("Blue and orange"),
                                                 QDialogButtonBox::ActionRole);
    accessible->setObjectName(QStringLiteral("accessiblePreset"));
    accessible->setToolTip(
        tr("Distinct under red-green colour blindness, which the default pair is not."));
    QPushButton *standard =
        presets->addButton(tr("Red and green"), QDialogButtonBox::ActionRole);
    standard->setObjectName(QStringLiteral("defaultPreset"));
    connect(accessible, &QPushButton::clicked, this, [this] {
        m_a = accessibleA();
        m_b = accessibleB();
        restyle();
    });
    connect(standard, &QPushButton::clicked, this, [this] {
        m_a = defaultA();
        m_b = defaultB();
        restyle();
    });
    top->addWidget(presets);

    auto *box = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    connect(box, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(box, &QDialogButtonBox::rejected, this, &QDialog::reject);
    top->addWidget(box);

    restyle();
}

void ColourDialog::choose(QColor *slot, QPushButton *button, const QString &title) {
    const QColor picked = QColorDialog::getColor(*slot, this, title);
    if (picked.isValid()) {
        *slot = picked;
        restyle();
    }
    Q_UNUSED(button);
}

/// Shows each colour on its own button, with the text in black or white
/// according to how dark the colour is — a label the reader cannot read is a
/// poor way to present a colour choice.
void ColourDialog::restyle() {
    const auto style = [](const QColor &c) {
        const int luma = (77 * c.red() + 150 * c.green() + 29 * c.blue()) >> 8;
        return QStringLiteral("background-color: %1; color: %2; padding: 6px;")
            .arg(c.name(), luma > 128 ? QStringLiteral("black") : QStringLiteral("white"));
    };
    m_buttonA->setText(m_a.name());
    m_buttonA->setStyleSheet(style(m_a));
    m_buttonB->setText(m_b.name());
    m_buttonB->setStyleSheet(style(m_b));
}
