// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include <QApplication>
#include <QLabel>
#include <QMenuBar>
#include <QStatusBar>

extern "C" {
#include "schcompare.h"
}

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    setWindowTitle(QStringLiteral("sch-pdf-compare"));
    resize(1280, 900);

    m_placeholder = new QLabel(
        tr("Open two revisions of a schematic to compare them."), this);
    m_placeholder->setAlignment(Qt::AlignCenter);
    setCentralWidget(m_placeholder);

    buildMenus();

    // Reading the version through the ABI rather than from a C++ constant is
    // the cheapest possible proof that the shell is talking to the core it was
    // built against, and it fails loudly at startup rather than at first use.
    statusBar()->showMessage(
        tr("core %1").arg(QString::fromUtf8(sc_version())));
}

void MainWindow::buildMenus() {
    QMenu *file = menuBar()->addMenu(tr("&File"));
    file->addSeparator();
    QAction *quit = file->addAction(tr("&Quit"));
    quit->setShortcut(QKeySequence::Quit);
    connect(quit, &QAction::triggered, qApp, &QApplication::quit);
}
