//! All `Command`s are executed here – port of `MainFrame.actionPerformed`, the status-bar button
//! listeners `MainFrame$3..$6`, `MainFrame$1` (key 't'), `MainFrame.safeExit` and the
//! `OptionPanel` listeners for recording (`$20` jbPunktNeu, `$21` jbReiheNeu), the delayed variants
//! (jbPunktNeu1 / jbReiheNeu1, `RecordDelay` from StoneAge.ini) and the scope (`$22`, `$23`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stoneage_core::{csv, saf, I18n, Point, Row, Survey};

use super::files::{self, Kind};
use super::{print, startup, t, tf, trace, updates};
use crate::app::StoneAgeApp;
use crate::state::{AppState, Command, PendingRecord, SidebarSection};

const MF: &str = "MainFrame";

/// Result of the "unsaved changes" dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmChoice {
    Save,
    Discard,
    Cancel,
}

/// Commands that would discard the current survey and therefore ask first when it is dirty.
fn discards_document(cmd: &Command) -> bool {
    // InstallUpdate restarts the app, so it asks like Exit.
    matches!(cmd, Command::NewSurvey | Command::Open | Command::OpenPath(_) | Command::Exit | Command::InstallUpdate)
}

fn error_toast(state: &mut AppState, text: String) {
    state.toast(format!("⚠ {text}"));
}

impl StoneAgeApp {
    pub fn execute(&mut self, ctx: &egui::Context, cmd: Command) {
        // Every user action goes to the daily log (File → Diagnostics).
        log::info!(target: "stoneage::action", "{cmd:?}");
        if discards_document(&cmd) && self.state.dirty && !self.shell.force {
            self.shell.confirm = Some(cmd);
            return;
        }
        let st = &mut self.state;
        match cmd {
            // ---------------------------------------------------------------- File
            Command::NewSurvey => {
                st.survey = Survey::new(&st.config);
                st.file_path = None;
                st.dirty = false;
                st.pending_record = None;
                trace::info(st, MF, "new survey");
            }
            Command::Open => {
                if let Some(p) = files::open(&self.state.i18n, Kind::Saf, self.dir()) {
                    self.remember_dir(&p);
                    self.load_saf(p);
                }
            }
            Command::OpenPath(p) => self.load_saf(p),
            Command::Save => match self.state.file_path.clone() {
                Some(p) => self.save_saf(p),
                None => self.execute(ctx, Command::SaveAs),
            },
            Command::SaveAs => {
                let name = self.suggested_name("saf");
                if let Some(p) = files::save(&self.state.i18n, Kind::Saf, self.dir(), Some(&name)) {
                    self.remember_dir(&p);
                    self.save_saf(p);
                }
            }
            Command::ExportCsv => {
                let name = self.suggested_name("csv");
                if let Some(p) = files::save(&self.state.i18n, Kind::Csv, self.dir(), Some(&name)) {
                    self.remember_dir(&p);
                    let st = &mut self.state;
                    match csv::export(&st.survey, &p) {
                        Ok(()) => {
                            trace::info(st, MF, format!("Export: {} erfolgreich gespeichert!", p.display()));
                            let msg = tf(&st.i18n, "App.Toast.CsvExported", "CSV exported: {0}", &[&p.display()]);
                            st.toast(msg);
                        }
                        Err(e) => {
                            trace::error(st, MF, format!("Export: IOException: {e}"));
                            error_toast(st, e.to_string());
                        }
                    }
                }
            }
            Command::ImportCsv => {
                if let Some(p) = files::open(&self.state.i18n, Kind::Csv, self.dir()) {
                    self.remember_dir(&p);
                    let st = &mut self.state;
                    trace::trace(st, MF, format!("Import CSV: {}", p.display()));
                    match csv::import(&mut st.survey, &p) {
                        Ok(()) => {
                            st.dirty = true;
                            clamp_indices(&mut st.survey);
                            trace::info(st, MF, format!("Import CSV: {} ok", p.display()));
                            let msg = tf(&st.i18n, "App.Toast.CsvImported", "CSV imported: {0}", &[&p.display()]);
                            st.toast(msg);
                        }
                        Err(e) => {
                            trace::error(st, MF, format!("Import CSV: {e}"));
                            error_toast(st, e.to_string());
                        }
                    }
                }
            }
            Command::LoadMap => {
                if let Some(p) = files::open(&self.state.i18n, Kind::Image, self.dir()) {
                    self.remember_dir(&p);
                    let st = &mut self.state;
                    trace::trace(st, MF, format!("Lade Karte aus Datei: {}", p.display()));
                    match image::open(&p) {
                        Ok(img) => {
                            st.survey.map_image = Some(img.to_rgba8());
                            st.dirty = true;
                            let msg = tf(&st.i18n, "App.Toast.MapLoaded", "Map loaded: {0}", &[&p.display()]);
                            st.toast(msg);
                        }
                        Err(e) => {
                            trace::error(st, MF, "Lade Karte unterbrochen!");
                            error_toast(st, e.to_string());
                        }
                    }
                }
            }
            Command::ClearMap => {
                if st.survey.map_image.take().is_some() {
                    st.dirty = true;
                }
            }
            Command::ExportDiagnostics => {
                let name = format!(
                    "stoneage-diagnostics-{}.txt",
                    chrono::Local::now().format("%Y-%m-%d_%H-%M")
                );
                if let Some(p) = files::save(&self.state.i18n, Kind::Text, self.dir(), Some(&name)) {
                    self.remember_dir(&p);
                    let st = &mut self.state;
                    let text = super::diagnostics::report(st);
                    match std::fs::write(&p, text) {
                        Ok(()) => {
                            trace::info(st, MF, format!("Diagnostics exported: {}", p.display()));
                            let msg = tf(&st.i18n, "App.Toast.DiagnosticsExported", "Diagnostics saved: {0}", &[&p.display()]);
                            st.toast(msg);
                        }
                        Err(e) => {
                            trace::error(st, MF, format!("Export diagnostics: {e}"));
                            error_toast(st, e.to_string());
                        }
                    }
                }
            }
            Command::OpenLogFolder => super::diagnostics::open_log_folder(),
            Command::ExportImage => {
                let name = self.suggested_name("png");
                if let Some(p) = files::save(&self.state.i18n, Kind::Image, self.dir(), Some(&name)) {
                    self.remember_dir(&p);
                    let st = &mut self.state;
                    let img = print::render(st, print::PRINT_WIDTH, print::PRINT_HEIGHT);
                    match print::save_image(&img, &p) {
                        Ok(()) => {
                            trace::info(st, MF, format!("Export image: {}", p.display()));
                            let msg = tf(&st.i18n, "App.Toast.ImageExported", "Image exported: {0}", &[&p.display()]);
                            st.toast(msg);
                        }
                        Err(e) => {
                            trace::error(st, MF, format!("Export image: {e}"));
                            error_toast(st, e);
                        }
                    }
                }
            }
            Command::Print => {
                trace::trace(st, MF, "printData");
                let img = print::render(st, print::PRINT_WIDTH, print::PRINT_HEIGHT);
                let path = print::print_file();
                match print::save_image(&img, &path).and_then(|()| print::print_path(&path)) {
                    Ok(()) => {
                        let msg = t(&st.i18n, "App.Toast.PrintSent", "Sent to the print dialog");
                        st.toast(msg);
                    }
                    Err(e) => {
                        trace::error(st, MF, format!("print: {e}"));
                        error_toast(st, e);
                    }
                }
            }
            Command::SaveDefaults => save_defaults(st),
            Command::Exit => self.safe_exit(ctx),

            // ---------------------------------------------------------------- Edit
            Command::DeleteRow => {
                // "Delete current row": the last row cannot be deleted.
                let s = &mut st.survey;
                if s.rows.len() > 1 {
                    let idx = s.active_row.min(s.rows.len() - 1);
                    s.rows.remove(idx);
                    if s.active_row >= s.rows.len() {
                        s.active_row = s.rows.len() - 1;
                    }
                    let active = s.active_row;
                    st.dirty = true;
                    trace::info(st, MF, format!("Aktuelle Reihe gelöscht, aktiv={active}"));
                } else {
                    trace::warn(st, MF, "letzte Reihe kann nicht gelöscht werden!");
                    let msg = t(&st.i18n, "App.Toast.LastRow", "The last row cannot be deleted");
                    error_toast(st, msg);
                }
            }
            Command::DeletePoint => {
                // "Delete current Point": the last point of a row cannot be deleted.
                let deleted = st.survey.current_row_mut().and_then(|r| {
                    if r.points.len() > 1 {
                        let idx = r.active_point.min(r.points.len() - 1);
                        r.points.remove(idx);
                        if r.active_point >= r.points.len() {
                            r.active_point = r.points.len() - 1;
                        }
                        Some(r.active_point)
                    } else {
                        None
                    }
                });
                match deleted {
                    Some(active) => {
                        st.dirty = true;
                        trace::info(st, MF, format!("Aktueller Punkt gelöscht, aktiv={active}"));
                    }
                    None => {
                        trace::warn(st, MF, "letzter Punkt kann nicht gelöscht werden!");
                        let msg = t(&st.i18n, "App.Toast.LastPoint", "The last point cannot be deleted");
                        error_toast(st, msg);
                    }
                }
            }
            Command::DeleteAllRows => {
                // "Alle Reihen löschen": removeAllElements, add one new row, activeReihe = 0.
                st.survey.rows.clear();
                st.survey.rows.push(Row::new());
                st.survey.active_row = 0;
                st.dirty = true;
                trace::info(st, MF, "Alle Reihen gelöscht!");
            }
            Command::NewRow => {
                // "Neue Reihe": reihen.add(new MessReihe()) – appended, active row unchanged.
                st.survey.rows.push(Row::new());
                st.dirty = true;
                trace::trace(st, MF, "Neue Reihe");
            }
            Command::NextRow => {
                st.survey.next_row();
                trace_row(st);
            }
            Command::PrevRow => {
                st.survey.prev_row();
                trace_row(st);
            }
            Command::NextPoint => {
                if let Some(r) = st.survey.current_row_mut() {
                    r.next_point();
                }
            }
            Command::PrevPoint => {
                if let Some(r) = st.survey.current_row_mut() {
                    r.prev_point();
                }
            }
            Command::SelectRow(i) => {
                if !st.survey.rows.is_empty() {
                    st.survey.active_row = i.min(st.survey.rows.len() - 1);
                }
            }
            Command::SelectPoint(i) => {
                if let Some(r) = st.survey.current_row_mut() {
                    if !r.points.is_empty() {
                        r.active_point = i.min(r.points.len() - 1);
                    }
                }
            }
            Command::FindMinMax => {
                if let Some(r) = st.survey.current_row_mut() {
                    r.find_min_max();
                    st.dirty = true;
                }
            }
            Command::ResetMinMax => {
                if let Some(r) = st.survey.current_row_mut() {
                    r.reset_min_max();
                    st.dirty = true;
                }
            }
            Command::ToggleTracer => {
                // Tracer.toggleGuiVisible: show/hide the "Messages" tab -> the Messages section.
                let visible = !(st.sidebar_open && st.sidebar == SidebarSection::Messages);
                if visible {
                    st.sidebar = SidebarSection::Messages;
                    st.sidebar_open = true;
                } else {
                    st.sidebar_open = false;
                }
                trace::trace(st, MF, format!("switched tracer guiVisisble to {visible}"));
            }
            Command::RecreateScenegraph => {
                self.views.view3d.recreate();
                trace::trace(&mut self.state, MF, "recreate Scenegraph");
            }
            Command::SetLanguage(lang) => {
                trace::trace(st, MF, format!("switch to {}", lang.code()));
                st.i18n = I18n::new(lang);
            }

            // ---------------------------------------------------------------- Scope (OptionPanel$22/$23)
            Command::StartScope => {
                trace::trace(st, "optionPanel", "start Oszi ...");
                match st.scope.start() {
                    Ok(()) => {
                        st.demo_mode = false;
                        st.scope_running = true;
                        trace::info(st, "optionPanel", "start Oszi .. ok!");
                    }
                    Err(e) => {
                        trace::error(st, "optionPanel", format!("start Oszi: {e}"));
                        error_toast(st, e.to_string());
                    }
                }
            }
            Command::StopScope => {
                trace::trace(st, "optionPanel", "stop Oszi ...");
                st.scope.stop();
                st.scope_running = false;
                st.demo_mode = true;
                trace::info(st, "optionPanel", "stop Oszi .. ok!");
            }
            Command::ShowScopeGui(v) => st.scope.show_gui(v),

            // ---------------------------------------------------------------- Record
            Command::Record { delayed: true } => schedule(st, false),
            Command::RecordNewRow { delayed: true } => schedule(st, true),
            Command::Record { delayed: false } => record(st, false),
            Command::RecordNewRow { delayed: false } => record(st, true),
            Command::CancelDelayedRecord => {
                if st.pending_record.take().is_some() {
                    trace::trace(st, MF, "delayed record cancelled");
                    let msg = t(&st.i18n, "App.Toast.RecordCancelled", "Delayed recording cancelled");
                    st.toast(msg);
                }
            }

            // ---------------------------------------------------------------- Navigation
            Command::SwitchTab(tab) => st.tab = tab,
            Command::ShowSection(sec) => {
                if st.sidebar == sec && st.sidebar_open {
                    st.sidebar_open = false;
                } else {
                    st.sidebar = sec;
                    st.sidebar_open = true;
                }
            }

            // ---------------------------------------------------------------- Updates (shell/updates.rs)
            Command::CheckForUpdates { silent } => updates::start_check(ctx, st, silent),
            Command::DownloadUpdate => updates::start_download(ctx, st),
            Command::InstallUpdate => self.install_update(ctx),
            Command::CancelUpdate => updates::cancel(st),
            Command::SkipVersion => updates::skip(st),
            Command::OpenReleasesPage => {
                let url = updates::releases_page(st);
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
        }
    }

    /// Called by the "unsaved changes" dialog.
    pub fn resolve_confirm(&mut self, ctx: &egui::Context, choice: ConfirmChoice) {
        let Some(cmd) = self.shell.confirm.take() else { return };
        match choice {
            ConfirmChoice::Cancel => {}
            ConfirmChoice::Discard => self.execute_forced(ctx, cmd),
            ConfirmChoice::Save => {
                self.execute(ctx, Command::Save);
                if !self.state.dirty {
                    self.execute_forced(ctx, cmd);
                }
            }
        }
    }

    fn execute_forced(&mut self, ctx: &egui::Context, cmd: Command) {
        self.shell.force = true;
        self.execute(ctx, cmd);
        self.shell.force = false;
    }

    /// `MainFrame.safeExit`: stop the scope, "Bye Bye", close (the lock file is removed in `on_exit`).
    fn safe_exit(&mut self, ctx: &egui::Context) {
        let st = &mut self.state;
        st.pending_record = None;
        trace::trace(st, MF, "SafeExit(): Stopping Scope.");
        st.scope.stop();
        st.scope_running = false;
        if let Some(a) = st.audio.as_mut() {
            a.close();
        }
        st.audio = None;
        trace::trace(st, MF, "SafeExit(): Bye Bye.");
        startup::remove_lock(self.shell.lock_path.take().as_deref());
        self.shell.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn dir(&self) -> Option<&Path> {
        self.shell
            .last_dir
            .as_deref()
            .or_else(|| self.state.file_path.as_deref().and_then(Path::parent))
    }

    fn remember_dir(&mut self, p: &Path) {
        if let Some(d) = p.parent() {
            self.shell.last_dir = Some(d.to_path_buf());
        }
    }

    fn suggested_name(&self, ext: &str) -> String {
        let stem = self
            .state
            .file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                let n = self.state.survey.name.trim();
                if n.is_empty() { "stoneage".to_string() } else { sanitize(n) }
            });
        format!("{stem}.{ext}")
    }

    /// "Load": `.saf` -> survey. On failure the original re-initialised the survey and named it
    /// "Fehler beim Laden!".
    fn load_saf(&mut self, path: PathBuf) {
        let st = &mut self.state;
        match saf::load(&path) {
            Ok(s) => {
                st.survey = s;
                clamp_indices(&mut st.survey);
                st.file_path = Some(path.clone());
                st.dirty = false;
                st.pending_record = None;
                trace::info(st, MF, format!("Load: {} erfolgreich geladen!", path.display()));
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let msg = tf(&st.i18n, "App.Toast.Loaded", "Loaded {0}", &[&name]);
                st.toast(msg);
                self.add_recent(path);
            }
            Err(e) => {
                trace::error(st, MF, format!("Load: {} fehlgeschlagen! ({e})", path.display()));
                let mut s = Survey::new(&st.config);
                s.name = "Fehler beim Laden!".into();
                st.survey = s;
                st.file_path = None;
                st.dirty = false;
                let msg = tf(&st.i18n, "App.Toast.LoadFailed", "Could not load {0}: {1}", &[&path.display(), &e]);
                error_toast(st, msg);
                self.state.recent_files.retain(|p| p != &path);
            }
        }
    }

    /// "Save": writes the `.saf` (zip with data entry and map image).
    fn save_saf(&mut self, path: PathBuf) {
        let st = &mut self.state;
        match saf::save(&st.survey, &path) {
            Ok(()) => {
                st.file_path = Some(path.clone());
                st.dirty = false;
                trace::info(st, MF, format!("Save: {} erfolgreich gespeichert!", path.display()));
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let msg = tf(&st.i18n, "App.Toast.Saved", "Saved {0}", &[&name]);
                st.toast(msg);
                self.add_recent(path);
            }
            Err(e) => {
                trace::error(st, MF, format!("Save: IOException: {e}"));
                let msg = tf(&st.i18n, "App.Toast.SaveFailed", "Could not save {0}: {1}", &[&path.display(), &e]);
                error_toast(st, msg);
            }
        }
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect()
}

fn trace_row(st: &mut AppState) {
    let msg = format!("Reihe {} / {}", st.survey.active_row + 1, st.survey.rows.len());
    trace::trace(st, "MessAreal", msg);
}

/// Keeps the active row / point indices valid after data changes.
pub fn clamp_indices(s: &mut Survey) {
    if s.rows.is_empty() {
        s.active_row = 0;
        return;
    }
    s.active_row = s.active_row.min(s.rows.len() - 1);
    for r in &mut s.rows {
        r.active_point = r.active_point.min(r.points.len().saturating_sub(1));
    }
}

/// "Save Defaults" (`Save Ini`): survey parameters + locale -> StoneAge.ini.
fn save_defaults(st: &mut AppState) {
    trace::trace(st, MF, "Save Ini...");
    st.survey.store_defaults(&mut st.config);
    let (language, country) = startup::lang_parts(st.i18n.lang);
    st.config.language = language;
    st.config.country = country;
    let path = st.config_path.clone();
    match st.config.save(&path) {
        Ok(()) => {
            trace::info(st, MF, "Save Ini... ok");
            let msg = tf(&st.i18n, "App.Toast.DefaultsSaved", "Defaults saved to {0}", &[&path.display()]);
            st.toast(msg);
        }
        Err(e) => {
            trace::error(st, MF, format!("Save Ini: could not save settings! ({e})"));
            error_toast(st, format!("{}: {e}", path.display()));
        }
    }
}

/// Starts the "Record delayed" countdown (`RecordDelay` ms from StoneAge.ini).
fn schedule(st: &mut AppState, new_row: bool) {
    let delay = Duration::from_millis(st.config.record_delay_ms);
    st.pending_record = Some(PendingRecord { fire_at: Instant::now() + delay, new_row });
    trace::trace(st, "optionPanel", format!("delayed record in {} ms", delay.as_millis()));
    let secs = delay.as_secs_f32().ceil() as u64;
    let msg = tf(&st.i18n, "App.Toast.RecordScheduled", "Recording in {0} s", &[&secs]);
    st.toast(msg);
}

/// `MessPunkt.acquireData`: demo data in demo mode, otherwise channel 1 of the scope.
fn acquire(st: &mut AppState, row: usize, idx: usize) -> Result<Point, String> {
    if st.demo_mode {
        Ok(Point::demo(row, idx))
    } else {
        let ch = st.scope.read_ch1().map_err(|e| e.to_string())?;
        let mut p = Point::new();
        p.set_from_capture(&ch, st.survey.f_range);
        Ok(p)
    }
}

/// `OptionPanel$20` (jbPunktNeu, "Record") and `OptionPanel$21` (jbReiheNeu, "Rec + new row"):
///
/// ```text
/// last = currentRow.getLastPunkt()
/// if last.isDataValid():
///     if !last.acquired: last.acquireData(); last.acquired = true
///     Record:        p = new MessPunkt(); p.acquireData(); row.addPoint(p); row.activePunkt = count-1
///     Rec + new row: reihen.add(new MessReihe()); activeReihe = reihen.size()-1
/// else: Tracer.trace(button text, "OptionPanel.point_data_not_valid")
/// ```
fn record(st: &mut AppState, new_row: bool) {
    if st.survey.rows.is_empty() {
        st.survey.rows.push(Row::new());
        st.survey.active_row = 0;
    }
    clamp_indices(&mut st.survey);
    let row_idx = st.survey.active_row;
    let (last_idx, last_valid, last_acquired) = match st.survey.rows[row_idx].points.last() {
        Some(p) => (Some(st.survey.rows[row_idx].points.len() - 1), p.is_data_valid(), p.acquired),
        // The original threw a NullPointerException for a row without points; here we just add one.
        None => (None, true, true),
    };
    let button_key = if new_row { "OptionPanel.jbReiheNeu.Caption" } else { "OptionPanel.jbPunktNeu.Caption" };

    if !last_valid {
        let origin = st.i18n.tr(button_key);
        let msg = st.i18n.tr("OptionPanel.point_data_not_valid");
        trace::warn(st, &origin, &msg);
        error_toast(st, msg);
        return;
    }

    if let Some(li) = last_idx {
        if !last_acquired {
            match acquire(st, row_idx, li) {
                Ok(mut p) => {
                    p.acquired = true;
                    st.survey.rows[row_idx].points[li] = p;
                }
                Err(e) => {
                    trace::error(st, "MessPunkt", format!("acquireData: {e}"));
                    error_toast(st, e);
                    return;
                }
            }
        }
    }

    if new_row {
        st.survey.rows.push(Row::new());
        st.survey.active_row = st.survey.rows.len() - 1;
        trace::info(st, "optionPanel", format!("Rec + new row -> {}", st.survey.rows.len()));
    } else {
        let idx = st.survey.rows[row_idx].points.len();
        match acquire(st, row_idx, idx) {
            Ok(mut p) => {
                // The fresh point is not marked acquired (it is re-acquired on the next Record).
                p.acquired = false;
                let row = &mut st.survey.rows[row_idx];
                row.add_point(p);
                row.active_point = row.count_points().saturating_sub(1);
                let n = row.count_points();
                trace::info(st, "optionPanel", format!("Record -> point {n}"));
            }
            Err(e) => {
                trace::error(st, "MessPunkt", format!("acquireData: {e}"));
                error_toast(st, e);
                return;
            }
        }
    }
    st.dirty = true;
    let msg = t(&st.i18n, "App.Toast.Recorded", "Point recorded");
    st.toast(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discard_guard() {
        assert!(discards_document(&Command::Exit));
        assert!(discards_document(&Command::Open));
        assert!(!discards_document(&Command::Save));
        assert!(discards_document(&Command::InstallUpdate));
        assert!(!discards_document(&Command::DownloadUpdate));
    }

    #[test]
    fn sanitize_names() {
        assert_eq!(sanitize("a/b:c"), "a_b_c");
    }
}
