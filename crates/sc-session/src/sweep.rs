// Copyright (c) the sch-pdf-compare authors. AGPL-3.0-or-later; see LICENSE.

//! Scanning the whole set in the background.
//!
//! The reader should not wait on 85 sheets to find out that three of them
//! changed, so the sweep runs on its own thread and the frontend watches a
//! [`Wakeup`] for "there is more to read".
//!
//! It opens its **own** copies of the two documents rather than sharing the
//! session's. MuPDF's context is per-thread; handing one thread's `Document` to
//! another is not something the type system stops here, and it is worth the
//! second open to make it impossible.

use crate::scan::SheetChanges;
use crate::wakeup::{Wakeup, WakeupHandle};
use crate::Session;
use sc_diff::{Options, RectF};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Everything the sweep needs, in a form that can cross a thread boundary.
/// Deliberately a snapshot: the sweep answers the question that was asked when
/// it started, and changing the tolerance mid-sweep restarts it rather than
/// producing a result half in each world.
#[derive(Clone, Debug)]
struct Job {
    path_a: String,
    path_b: String,
    page_delta: i32,
    options: Options,
    ignore_rects: Vec<RectF>,
}

/// What the frontend reads. Every field is meaningless until `finished`, except
/// `scanned` and `total`, which are there to fill a progress line.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SweepStatus {
    pub running: bool,
    /// The sweep reached the end. The summary below only means anything once
    /// this is true.
    pub finished: bool,
    pub scanned: i32,
    pub total: i32,
    pub changed_sheets: i32,
}

#[derive(Default)]
struct Shared {
    status: SweepStatus,
    /// Sheets scanned since the frontend last collected. Drained by
    /// [`Sweep::take_results`], so the lock is held only for the move.
    fresh: Vec<SheetChanges>,
}

pub struct Sweep {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    wakeup: Wakeup,
    handle: Option<JoinHandle<()>>,
}

impl Sweep {
    /// The handle the frontend's event loop watches.
    pub fn wakeup_handle(&self) -> WakeupHandle {
        self.wakeup.handle()
    }

    pub fn status(&self) -> SweepStatus {
        self.shared.lock().map(|s| s.status).unwrap_or_default()
    }

    /// Takes the sheets scanned since the last call, and clears the wakeup.
    ///
    /// Draining before reading, not after: a sheet finished between the two
    /// would otherwise clear a signal that was about work the frontend has not
    /// collected, and the result would sit there until something else happened.
    pub fn take_results(&self) -> Vec<SheetChanges> {
        self.wakeup.drain();
        match self.shared.lock() {
            Ok(mut s) => std::mem::take(&mut s.fresh),
            Err(_) => Vec::new(),
        }
    }

    /// Asks the sweep to stop and waits for it.
    ///
    /// Waiting rather than detaching: the thread holds its own MuPDF documents,
    /// and a caller that is about to reopen the pair should not race a scan of
    /// the old one.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Sweep {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Session {
    /// Starts scanning every sheet on a worker thread.
    ///
    /// Returns `None` only if the platform would not give us a wakeup object.
    pub fn start_sweep(&self) -> Option<Sweep> {
        let job = Job {
            path_a: self.path_a().to_owned(),
            path_b: self.path_b().to_owned(),
            page_delta: self.page_delta(),
            options: self.options(),
            ignore_rects: self.ignore_rects().to_vec(),
        };
        let total = self.page_count();
        let wakeup = Wakeup::new()?;
        let shared = Arc::new(Mutex::new(Shared {
            status: SweepStatus {
                running: true,
                total,
                ..Default::default()
            },
            fresh: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let handle = std::thread::Builder::new()
            .name("sch-sweep".into())
            .spawn({
                let shared = Arc::clone(&shared);
                let stop = Arc::clone(&stop);
                let wakeup = wakeup.clone();
                move || run(job, total, shared, stop, wakeup)
            })
            .ok()?;

        Some(Sweep {
            shared,
            stop,
            wakeup,
            handle: Some(handle),
        })
    }
}

fn run(job: Job, total: i32, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>, wakeup: Wakeup) {
    let finish = |changed: i32, scanned: i32| {
        if let Ok(mut s) = shared.lock() {
            // `finished` and `running` move together and before the last poke,
            // so a frontend that checks "finished" on the wakeup that announces
            // it actually sees it. Setting them after would mean the last
            // notification says "still going" and nothing ever fires again.
            s.status.finished = true;
            s.status.running = false;
            s.status.changed_sheets = changed;
            s.status.scanned = scanned;
        }
        wakeup.poke();
    };

    // Its own handles, on its own thread. See the module comment.
    let Ok(worker) = Session::open(&job.path_a, &job.path_b) else {
        finish(0, 0);
        return;
    };
    let mut worker = worker;
    worker.set_page_delta(job.page_delta);
    worker.set_options(job.options);
    for r in &job.ignore_rects {
        worker.add_ignore_rect(*r);
    }

    let mut changed = 0;
    let mut scanned = 0;
    for page in 1..=total {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(result) = worker.scan_page(page) else {
            continue;
        };
        scanned += 1;
        if !result.changes.is_empty() {
            changed += 1;
        }
        if let Ok(mut s) = shared.lock() {
            s.status.scanned = scanned;
            s.status.changed_sheets = changed;
            s.fresh.push(result);
        }
        wakeup.poke();
    }
    finish(changed, scanned);
}

/// Regions whose changes recur across the set, offered as something to exclude.
///
/// A drawing set shares a title block, so a changed date colours every sheet.
/// This finds that — but it only ever *offers*. A net renamed across the whole
/// set looks identical to a heuristic, and silently discounting it would be the
/// worst failure this tool could have.
///
/// Matching is on how close two boxes' corners are, not on which cell of a grid
/// they fall in. A grid splits neighbours that happen to straddle a boundary,
/// and measured on the sample set that is not a rare case: the title-block
/// change lands at x = 616 on one sheet, 685 on another and 725 on a third, so
/// grid bucketing scattered one region across four cells and found nothing.
pub fn suggest_ignores(results: &[SheetChanges], sheets: i32) -> Vec<RectF> {
    // Too few sheets for "recurring" to mean anything, or a sweep that has not
    // finished — a partial answer here would offer to hide whatever happened to
    // be scanned first.
    if sheets < 4 || (results.len() as i32) < sheets {
        return Vec::new();
    }
    // Half the set, which is the fork's threshold and was settled against these
    // documents. Two thirds sounds more careful and is worse: it misses a title
    // block whose field drifts enough that only some sheets agree closely.
    let threshold = 3.max((sheets + 1) / 2);
    // How near two boxes have to be to count as the same place — and the slack
    // is deliberately not the same on both axes.
    //
    // A drawing frame is built in rows: a title-block field sits at a fixed y on
    // every sheet, and what varies is how far along that row the characters that
    // differ happen to fall. Measured across both sample sets, the recurring
    // change sits at y = 581.76 on every single sheet of both, while x ranges
    // over 616–725. Tight in y, loose in x, is what that shape asks for.
    //
    // Symmetric 8pt — the fork's rule — finds it on the 85-sheet set (73 of 84
    // sheets agree) and misses it by one sheet on the 21-sheet set (10 against a
    // threshold of 11), which is the wrong answer for a set where every sheet
    // visibly has the same title-block change.
    // Enough for a title block's fields; past this the reader is being asked to
    // review a list rather than accept a suggestion.
    const MAX_SUGGESTIONS: usize = 8;

    let mut out: Vec<RectF> = Vec::new();
    for (i, sheet) in results.iter().enumerate() {
        for box_ in &sheet.changes {
            let on_others = results
                .iter()
                .enumerate()
                .filter(|(j, other)| *j != i && other.changes.iter().any(|o| same_place(*o, *box_)))
                .count() as i32;
            if on_others < threshold {
                continue;
            }
            if out.iter().any(|s| same_place(*s, *box_)) {
                continue;
            }
            // A little slack, so the suggestion covers the whole field rather
            // than only the characters that happened to differ on this sheet.
            out.push(RectF::new(
                box_.x - 4.0,
                box_.y - 4.0,
                box_.dx + 8.0,
                box_.dy + 8.0,
            ));
            if out.len() >= MAX_SUGGESTIONS {
                return out;
            }
        }
    }
    out
}

/// How near two boxes have to be to count as the same place — and the slack is
/// deliberately not the same on both axes.
///
/// A drawing frame is built in rows: a title-block field sits at a fixed y on
/// every sheet, and what varies is how far along that row the characters that
/// differ happen to fall. Measured across both sample sets, the recurring change
/// sits at y = 581.76 on every single sheet of both, while x ranges over
/// 616–725. Tight in y, loose in x, is what that shape asks for.
///
/// Symmetric 8pt — the fork's rule — finds it on the 85-sheet set (73 of 84
/// sheets agree) but misses it by one sheet on the 21-sheet set (10 against a
/// threshold of 11), which is the wrong answer for a set where every sheet
/// visibly carries the same title-block change.
const SAME_ROW_PT: f32 = 8.0;
const SAME_AREA_PT: f32 = 32.0;

fn same_place(a: RectF, b: RectF) -> bool {
    (a.x - b.x).abs() <= SAME_AREA_PT && (a.y - b.y).abs() <= SAME_ROW_PT
}
