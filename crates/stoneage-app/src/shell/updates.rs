//! Self-update integration (crate `stoneage-update`): startup check, background job polling,
//! command handlers, rail badge. The UI lives in `panels/updates.rs`.
//!
//! Nothing is ever installed without the user clicking "Install & restart".

use stoneage_core::I18n;
use stoneage_update::{CheckOutcome, Event, Job, UpdateError};

use super::{t, tf, trace};
use crate::app::StoneAgeApp;
use crate::state::{AppState, SidebarSection, UpdateStatus};

const ORIGIN: &str = "Updater";

/// Version of this build (workspace `version`).
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repository to check: `UpdateRepo` from StoneAge.ini, else `stoneage_update::UPDATE_REPO`.
pub fn repo(st: &AppState) -> String {
    let ov = st.config.extra.iter().find(|(k, _)| k.eq_ignore_ascii_case("UpdateRepo")).map(|(_, v)| v.as_str());
    stoneage_update::effective_repo(ov)
}

pub fn releases_page(st: &AppState) -> String {
    stoneage_update::releases_page(&repo(st))
}

/// User-facing text for an update error.
pub fn error_text(i18n: &I18n, e: &UpdateError) -> String {
    match e {
        UpdateError::Network(_) => t(i18n, "App.Update.Error.Offline", "GitHub is not reachable. Check the internet connection."),
        UpdateError::RateLimited { .. } => {
            t(i18n, "App.Update.Error.RateLimit", "GitHub rate limit reached. Please try again later.")
        }
        UpdateError::NoRelease(_) => t(i18n, "App.Update.Error.NoRelease", "No release has been published yet."),
        UpdateError::NoAsset(..) => t(i18n, "App.Update.Error.NoAsset", "The new release has no package for this build."),
        UpdateError::NoChecksum(_) | UpdateError::ChecksumMismatch { .. } => t(
            i18n,
            "App.Update.Error.Checksum",
            "The download could not be verified (checksum). Nothing was changed.",
        ),
        e => tf(i18n, "App.Update.Error.Generic", "Update failed: {0}", &[e]),
    }
}

fn error_toast(st: &mut AppState, text: String) {
    st.toast(format!("⚠ {text}"));
}

/// Once per frame (after the splash): starts the automatic startup check and processes events of
/// the running background job.
pub fn poll(ctx: &egui::Context, st: &mut AppState) {
    if !st.update.startup_done {
        st.update.startup_done = true;
        if st.update.auto_check {
            start_check(ctx, st, true);
        }
    }
    let Some(job) = st.update.job.as_mut() else { return };
    let events = job.poll();
    let finished = job.is_finished();
    for ev in events {
        handle(st, ev);
    }
    if finished {
        st.update.job = None;
    }
    // The section being looked at clears the badge.
    if st.update.badge && st.sidebar_open && st.sidebar == SidebarSection::Updates {
        st.update.badge = false;
    }
}

fn handle(st: &mut AppState, ev: Event) {
    let silent = st.update.silent;
    match ev {
        Event::Checked(r) => {
            st.update.last_checked = Some(chrono::Local::now());
            match r {
                Ok(CheckOutcome::Available(rel)) => {
                    let v = rel.version.to_string();
                    let skipped = st.update.skipped_version.as_deref() == Some(v.as_str());
                    trace::info(st, ORIGIN, format!("new version {} available ({})", rel.tag, rel.asset.name));
                    // Re-check of an already downloaded version keeps the verified package.
                    let same = st.update.available.as_ref().is_some_and(|a| a.version == rel.version);
                    if same && st.update.package.as_ref().is_some_and(|p| p.is_file()) {
                        st.update.status = UpdateStatus::Ready;
                    } else {
                        st.update.status = UpdateStatus::Available;
                        st.update.package = None;
                        st.update.progress = None;
                    }
                    st.update.available = Some(rel);
                    if !(silent && skipped) {
                        st.update.badge = !(st.sidebar_open && st.sidebar == SidebarSection::Updates);
                        let msg = tf(&st.i18n, "App.Update.Toast.Available", "Update available: StoneAge {0}", &[&v]);
                        st.toast(msg);
                    }
                }
                Ok(CheckOutcome::UpToDate { latest }) => {
                    trace::trace(st, ORIGIN, format!("up to date ({CURRENT_VERSION}, latest {latest:?})"));
                    st.update.status = UpdateStatus::UpToDate { latest };
                    st.update.available = None;
                    st.update.package = None;
                    if !silent {
                        let msg = t(&st.i18n, "App.Update.Toast.UpToDate", "StoneAge is up to date");
                        st.toast(msg);
                    }
                }
                Err(e) => {
                    trace::warn(st, ORIGIN, format!("check failed: {e}"));
                    let text = error_text(&st.i18n, &e);
                    st.update.status = UpdateStatus::Error(text.clone());
                    if !silent {
                        error_toast(st, text);
                    }
                }
            }
        }
        Event::Progress(p) => st.update.progress = Some(p),
        Event::Downloaded(Ok(path)) => {
            trace::info(st, ORIGIN, format!("package verified: {}", path.display()));
            st.update.package = Some(path);
            st.update.status = UpdateStatus::Ready;
            let msg = t(&st.i18n, "App.Update.Toast.Downloaded", "Update downloaded – click \"Install & restart\"");
            st.toast(msg);
        }
        Event::Downloaded(Err(UpdateError::Cancelled)) => {
            trace::trace(st, ORIGIN, "download cancelled");
            st.update.status = UpdateStatus::Available;
            st.update.progress = None;
            let msg = t(&st.i18n, "App.Update.Toast.Cancelled", "Update download cancelled");
            st.toast(msg);
        }
        Event::Downloaded(Err(e)) => {
            trace::error(st, ORIGIN, format!("download failed: {e}"));
            let text = error_text(&st.i18n, &e);
            st.update.status = UpdateStatus::Error(text.clone());
            st.update.progress = None;
            error_toast(st, text);
        }
    }
}

fn repaint_notifier(ctx: &egui::Context) -> stoneage_update::job::Notify {
    let ctx = ctx.clone();
    Box::new(move || ctx.request_repaint())
}

fn busy(st: &AppState) -> bool {
    st.update.job.is_some()
}

/// `Command::CheckForUpdates`.
pub fn start_check(ctx: &egui::Context, st: &mut AppState, silent: bool) {
    if busy(st) {
        return;
    }
    let repo = repo(st);
    trace::trace(st, ORIGIN, format!("checking {repo} (current {CURRENT_VERSION})"));
    st.update.silent = silent;
    st.update.status = UpdateStatus::Checking;
    st.update.job = Some(Job::check(repo, CURRENT_VERSION.to_string(), st.update.include_prerelease, repaint_notifier(ctx)));
}

/// `Command::DownloadUpdate`.
pub fn start_download(ctx: &egui::Context, st: &mut AppState) {
    if busy(st) {
        return;
    }
    let Some(rel) = st.update.available.clone() else { return };
    let dir = std::env::temp_dir().join("stoneage-update").join(rel.version.to_string());
    trace::info(st, ORIGIN, format!("downloading {}", rel.asset.url));
    st.update.silent = false;
    st.update.status = UpdateStatus::Downloading;
    st.update.package = None;
    st.update.progress = Some(stoneage_update::Progress { total: Some(rel.asset.size), ..Default::default() });
    st.update.job = Some(Job::download(rel, dir, repaint_notifier(ctx)));
}

/// `Command::CancelUpdate`.
pub fn cancel(st: &mut AppState) {
    let Some(job) = st.update.job.take() else { return };
    job.cancel();
    st.update.progress = None;
    st.update.status = match st.update.status {
        UpdateStatus::Downloading => UpdateStatus::Available,
        UpdateStatus::Checking => UpdateStatus::Idle,
        ref s => s.clone(),
    };
    trace::trace(st, ORIGIN, "cancelled");
}

/// `Command::SkipVersion`.
pub fn skip(st: &mut AppState) {
    let Some(rel) = st.update.available.as_ref() else { return };
    let v = rel.version.to_string();
    trace::info(st, ORIGIN, format!("skipping version {v}"));
    st.update.skipped_version = Some(v.clone());
    st.update.badge = false;
    let msg = tf(&st.i18n, "App.Update.Toast.Skipped", "Version {0} will be skipped", &[&v]);
    st.toast(msg);
}

/// Red dot at the top right of the rail button.
pub fn paint_badge(ui: &egui::Ui, rect: egui::Rect) {
    let c = rect.right_top() + egui::vec2(-7.0, 7.0);
    ui.painter().circle(c, 4.5, egui::Color32::from_rgb(230, 70, 60), egui::Stroke::new(1.5, ui.visuals().panel_fill));
}

impl StoneAgeApp {
    /// `Command::InstallUpdate` (the unsaved-changes guard ran in `execute`): install the verified
    /// package, start the new exe, close this window. The lock file is removed in `on_exit` (after
    /// the settings were saved); the new process waits for that (`stoneage_update::after_restart`).
    pub(crate) fn install_update(&mut self, ctx: &egui::Context) {
        let st = &mut self.state;
        let Some(pkg) = st.update.package.clone().filter(|_| st.update.status == UpdateStatus::Ready) else {
            return;
        };
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                error_toast(st, e.to_string());
                return;
            }
        };
        trace::info(st, ORIGIN, format!("installing {} over {}", pkg.display(), exe.display()));
        match stoneage_update::install(&pkg, &exe) {
            Ok(report) => {
                for w in &report.warnings {
                    trace::warn(st, ORIGIN, w);
                }
                trace::info(st, ORIGIN, format!("installed {} files", report.files));
                let _ = std::fs::remove_file(&pkg);
            }
            Err(e) => {
                trace::error(st, ORIGIN, format!("install failed: {e}"));
                let text = error_text(&st.i18n, &e);
                st.update.status = UpdateStatus::Error(text.clone());
                error_toast(st, text);
                return;
            }
        }

        // Like safeExit: release the hardware before the new instance starts.
        st.pending_record = None;
        st.scope.stop();
        st.scope_running = false;
        if let Some(a) = st.audio.as_mut() {
            a.close();
        }
        st.audio = None;
        match stoneage_update::restart(&exe) {
            Ok(_) => {
                trace::trace(st, ORIGIN, "restarting");
                self.shell.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => {
                trace::error(st, ORIGIN, format!("restart failed: {e}"));
                let msg = t(&st.i18n, "App.Update.Toast.RestartManually", "Update installed. Please restart StoneAge.");
                error_toast(st, msg);
                st.update.status = UpdateStatus::Idle;
                st.update.package = None;
            }
        }
    }
}
