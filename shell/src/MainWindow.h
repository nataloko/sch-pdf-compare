// The application window.
//
// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.
#pragma once

#include <QMainWindow>
#include <QVector>

class CompareView;
class QLabel;
class QAction;
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
    void onCurrentPageChanged(int page);
    void onRegionSelected(int page, const QRectF &r);
    void scanEverySheet();
    void onSweepProgressed();
    void acceptSuggestions();

  public:
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

  private:
    void buildMenus();
    void persist();
    void updateStatus();
    void rebuildSheetList();
    void rebuildTextChanges(int page);
    void setViewMode(int mode);
    void blink();

    CompareView *m_view = nullptr;
    QTreeWidget *m_sheets = nullptr;
    QTreeWidget *m_text = nullptr;
    QLabel *m_status = nullptr;
    Session *m_session = nullptr;
    bool m_forTesting = false;
    // Where `Tab` came from, so it can go back.
    int m_blinkFrom = 0;
    // The change the reader is standing on, as (sheet, index).
    int m_atSheet = 0;
    int m_atIndex = -1;
    QAction *m_acceptSuggestions = nullptr;
};
