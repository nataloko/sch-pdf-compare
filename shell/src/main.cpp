// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include <QApplication>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("sch-pdf-compare"));
    QApplication::setOrganizationName(QStringLiteral("sch-pdf-compare"));

    MainWindow win;
    win.show();
    return QApplication::exec();
}
