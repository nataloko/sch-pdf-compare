// The application window.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include <QMainWindow>
#include <QVector>

class CompareView;
class QLabel;
class QPainter;
class QPrinter;
class QAction;
class QActionGroup;
class QCheckBox;
class QTreeWidget;
class Session;

class MainWindow : public QMainWindow {
    Q_OBJECT

  public:
    explicit MainWindow(QWidget *parent = nullptr);

    // Returns false and shows why when the pair cannot be opened.
    bool openPair(const QString &pathA, const QString &pathB);
    // Skips saving settings, so persistence can be exercised without it.
    void setForTesting(bool on) { m_forTesting = on; }

  private slots:
    void chooseAndOpen();
    void reopenLast();
    void exportReport();
    void printSheets();
    void printChangedSheets();
    void onCurrentPageChanged(int page);
    void onRegionSelected(int page, const QRectF &r);
    void scanEverySheet();
    void onSweepProgressed();
    void acceptSuggestions();
    void showViewMenu(const QPoint &at);

  public:
    // The half of the print path that does the work, without the dialog. Split
    // out so a test can print to a PDF and read back what came out, and so the
    // dialog stays purely a dialog.
    int printTo(QPrinter &printer, const QVector<int> &sheets);
    QVector<int> changedSheetList() const;

    // The half of `acceptSuggestions` that does the work, without the question.
    // Split out so a test exercises the real behaviour rather than a modal
    // dialog, and so the question stays purely a question.
    void applySuggestions();

  private slots:
    void stepChange(int direction);
    void nudgePairing(int by);
    void matchSheets();
    void nudgeTolerance(int by);

  protected:
    void closeEvent(QCloseEvent *e) override;
    // Watches for a palette change: the toolbar's pictures are drawn in the
    // toolbar's own colours.
    void changeEvent(QEvent *e) override;

  private:
    void buildMenus();
    void buildToolBar();
    void refreshIcons();
    // Everything that needs a comparison open is switched off until there is
    // one, so a key or a button that does nothing looks like it does nothing
    // rather than looking broken.
    void enableSessionActions(bool on);
    void syncViewActions();
    void persist();
    void printRange(const QVector<int> &sheets);
    void paintSheetForPrint(QPainter &g, QPrinter &printer, int sheet);
    QVector<int> changedSheets() const;
    void updateStatus();
    void rebuildSheetList();
    void rebuildTextChanges(int page);
    void setViewMode(int mode);
    void blink();

    CompareView *m_view = nullptr;
    QTreeWidget *m_sheets = nullptr;
    QTreeWidget *m_text = nullptr;
    QCheckBox *m_showMoved = nullptr;
    QLabel *m_status = nullptr;
    Session *m_session = nullptr;
    bool m_forTesting = false;
    // The change the reader is standing on, as (sheet, index).
    int m_atSheet = 0;
    int m_atIndex = -1;
    QAction *m_acceptSuggestions = nullptr;
    QAction *m_excludeRegion = nullptr;
    QAction *m_overlayAct = nullptr;
    QAction *m_onlyAAct = nullptr;
    QAction *m_onlyBAct = nullptr;
    QAction *m_sideBySideAct = nullptr;
    QAction *m_singlePageAct = nullptr;
    QActionGroup *m_modeGroup = nullptr;
    // Actions that need a comparison; see enableSessionActions.
    QVector<QAction *> m_needSession;
};
