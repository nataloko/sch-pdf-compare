// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#include "MainWindow.h"

#include <QApplication>
#include <QCommandLineParser>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("sch-pdf-compare"));
    QApplication::setOrganizationName(QStringLiteral("sch-pdf-compare"));
    QApplication::setApplicationVersion(QStringLiteral("0.1.0"));

    QCommandLineParser parser;
    parser.setApplicationDescription(
        QStringLiteral("Compare two revisions of a schematic and show what changed."));
    parser.addHelpOption();
    parser.addVersionOption();
    // Deliberately does not save settings, so persistence can be exercised
    // without it.
    const QCommandLineOption forTesting(QStringLiteral("for-testing"),
                                        QStringLiteral("Do not read or write saved settings."));
    parser.addOption(forTesting);
    parser.addPositionalArgument(QStringLiteral("earlier"), QStringLiteral("The earlier revision."));
    parser.addPositionalArgument(QStringLiteral("later"), QStringLiteral("The later revision."));
    parser.process(app);

    MainWindow win;
    win.setForTesting(parser.isSet(forTesting));
    win.show();

    const QStringList files = parser.positionalArguments();
    if (files.size() == 2 && !win.openPair(files[0], files[1])) {
        return 1;
    }
    return QApplication::exec();
}
