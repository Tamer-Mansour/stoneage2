//! Background jobs: check / download run on their own thread and report through a channel, so the
//! UI thread never blocks. `notify` is called after every event (e.g. `egui::Context::request_repaint`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

use crate::{CheckOutcome, Progress, ReleaseInfo, UpdateError};

#[derive(Debug)]
pub enum Event {
    Checked(Result<CheckOutcome, UpdateError>),
    Progress(Progress),
    /// Verified package path.
    Downloaded(Result<PathBuf, UpdateError>),
}

/// Handle of a running background job.
pub struct Job {
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    finished: bool,
}

pub type Notify = Box<dyn Fn() + Send + 'static>;

impl Job {
    /// Starts a release check of `repo` against `current`.
    pub fn check(repo: String, current: String, include_prerelease: bool, notify: Notify) -> Self {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        std::thread::Builder::new()
            .name("stoneage-update-check".into())
            .spawn(move || {
                let r = crate::check(&repo, &current, include_prerelease);
                let _ = tx.send(Event::Checked(r));
                notify();
            })
            .expect("spawn update thread");
        Self { rx, cancel, finished: false }
    }

    /// Starts downloading + verifying `release` into `dir`.
    pub fn download(release: ReleaseInfo, dir: PathBuf, notify: Notify) -> Self {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::Builder::new()
            .name("stoneage-update-download".into())
            .spawn(move || {
                let ptx = tx.clone();
                let mut on_progress = |p: Progress| {
                    let _ = ptx.send(Event::Progress(p));
                    notify();
                };
                let r = crate::download(&release, &dir, &c, &mut on_progress);
                let _ = tx.send(Event::Downloaded(r));
                notify();
            })
            .expect("spawn update thread");
        Self { rx, cancel, finished: false }
    }

    /// Requests cancellation (the download stops at the next chunk and deletes its partial file).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// All events received so far (non-blocking).
    pub fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => {
                    if matches!(e, Event::Checked(_) | Event::Downloaded(_)) {
                        self.finished = true;
                    }
                    out.push(e);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    break;
                }
            }
        }
        out
    }

    /// The job delivered its final event (or its thread ended).
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // A dropped (e.g. superseded) download must not keep running in the background.
        self.cancel();
    }
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job").field("finished", &self.finished).finish()
    }
}
