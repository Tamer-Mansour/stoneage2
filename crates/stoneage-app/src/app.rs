//! The eframe application: owns the shared `AppState`, the view widgets and the shell UI state.
//! Port of `MainFrame` (layout) – the pieces live in `crate::shell`.

use std::path::PathBuf;
use std::time::Instant;

use stoneage_plots::{PointView, RowView, ScaleBar};
use stoneage_spatial::{ArealView, MapView, View3d};

use crate::shell;
use crate::state::{AppState, Command, SidebarSection, Tab};

/// Key under which the shell stores its settings in the eframe storage.
const STORAGE_KEY: &str = "stoneage_shell";
/// Maximum number of remembered recent files.
pub const MAX_RECENT: usize = 10;

/// All view widgets (they keep zoom/drag/texture state between frames).
#[derive(Default)]
pub struct Views {
    pub point: PointView,
    pub row: RowView,
    pub scale: ScaleBar,
    pub areal: ArealView,
    pub map: MapView,
    pub view3d: View3d,
    /// "Soundtest" tab (single acquisition, `AudionInputPanel.aquireData`).
    pub sound1: shell::central::SoundPane,
    /// "Soundtest 2" tab (continuous acquisition).
    pub sound2: shell::central::SoundPane,
}

/// UI state that belongs to the shell only (not shared with the panels).
pub struct ShellUi {
    pub dark: bool,
    pub splash: shell::splash::Splash,
    /// Command waiting for the "unsaved changes" decision.
    pub confirm: Option<Command>,
    /// Set while a guarded command is executed after the user chose Save/Discard.
    pub force: bool,
    /// The window may close now (safeExit done).
    pub allow_close: bool,
    /// Lock file created at startup (`stoneage.lock`), removed on exit.
    pub lock_path: Option<PathBuf>,
    /// Last directory used in a file dialog (JFileChooser keeps its current directory).
    pub last_dir: Option<PathBuf>,
    /// Window title last sent to the viewport.
    pub title: String,
}

/// Settings remembered between sessions via eframe persistence.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Persisted {
    recent_files: Vec<PathBuf>,
    dark: bool,
    sidebar_open: bool,
    sidebar: usize,
    tab: usize,
    last_dir: Option<PathBuf>,
    /// Updates: "Check automatically on startup".
    update_auto_check: bool,
    /// Updates: "Include pre-releases".
    update_prerelease: bool,
    /// Updates: version the user chose to skip.
    update_skipped: Option<String>,
    /// Updates: last successful/attempted check (RFC 3339).
    update_last_checked: Option<String>,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            recent_files: Vec::new(),
            dark: true,
            sidebar_open: true,
            sidebar: 1,
            tab: 0,
            last_dir: None,
            update_auto_check: true,
            update_prerelease: false,
            update_skipped: None,
            update_last_checked: None,
        }
    }
}

pub struct StoneAgeApp {
    pub state: AppState,
    pub views: Views,
    pub shell: ShellUi,
}

impl StoneAgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: shell::startup::Startup) -> Self {
        let persisted: Persisted = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        shell::theme::install(&cc.egui_ctx);
        shell::theme::set_dark(&cc.egui_ctx, persisted.dark);

        let shell::startup::Startup { mut state, splash_lines, panic, lock_path } = startup;
        state.recent_files = persisted.recent_files;
        state.recent_files.retain(|p| p.exists());
        state.recent_files.truncate(MAX_RECENT);
        state.sidebar_open = persisted.sidebar_open;
        state.sidebar = shell::sidebar::SECTIONS
            .get(persisted.sidebar)
            .map(|s| s.0)
            .unwrap_or(SidebarSection::Measure);
        state.tab = shell::tabs::TABS.get(persisted.tab).map(|t| t.0).unwrap_or(Tab::Map);
        state.update.auto_check = persisted.update_auto_check;
        state.update.include_prerelease = persisted.update_prerelease;
        state.update.skipped_version = persisted.update_skipped;
        state.update.last_checked = persisted
            .update_last_checked
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Local));

        // A crash since the last start: tell the user how to send the report.
        let crashes = shell::diagnostics::unseen_crashes();
        if !crashes.is_empty() {
            let msg = shell::t(
                &state.i18n,
                "App.Toast.CrashLastTime",
                "StoneAge closed unexpectedly last time. Please send File → Diagnostics → Export diagnostics.",
            );
            state.log(log::Level::Warn, format!("{} crash report(s) since last start", crashes.len()));
            state.toast(format!("⚠ {msg}"));
            shell::diagnostics::mark_crashes_seen();
        }

        Self {
            state,
            views: Views::default(),
            shell: ShellUi {
                dark: persisted.dark,
                splash: shell::splash::Splash::new(splash_lines, panic),
                confirm: None,
                force: false,
                allow_close: false,
                lock_path,
                last_dir: persisted.last_dir,
                title: String::new(),
            },
        }
    }

    /// Adds `path` to the top of the recent-files list.
    pub fn add_recent(&mut self, path: PathBuf) {
        self.state.recent_files.retain(|p| p != &path);
        self.state.recent_files.insert(0, path);
        self.state.recent_files.truncate(MAX_RECENT);
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = shell::window_title(&self.state);
        if title != self.shell.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shell.title = title;
        }
    }

    /// "Record delayed" countdown (`RecordDelay` from StoneAge.ini).
    fn tick_pending_record(&mut self, ctx: &egui::Context) {
        let Some(p) = self.state.pending_record.clone() else { return };
        let now = Instant::now();
        if now >= p.fire_at {
            self.state.pending_record = None;
            let cmd = if p.new_row {
                Command::RecordNewRow { delayed: false }
            } else {
                Command::Record { delayed: false }
            };
            self.execute(ctx, cmd);
        } else {
            let left = p.fire_at - now;
            ctx.request_repaint_after(left.min(std::time::Duration::from_millis(100)));
        }
    }

    /// Executes all queued commands (menu, sidebar, shortcuts, views).
    pub fn run_commands(&mut self, ctx: &egui::Context) {
        // Commands may queue further commands; bound the loop to avoid ping-pong.
        for _ in 0..8 {
            let cmds = std::mem::take(&mut self.state.commands);
            if cmds.is_empty() {
                break;
            }
            for c in cmds {
                self.execute(ctx, c);
            }
        }
    }
}

impl eframe::App for StoneAgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Splash (Start / Splash$MyWindow) inside the window for ~1 s, or the panicExit screen.
        if !self.shell.splash.done() {
            shell::splash::show(ui, &mut self.shell.splash, self.shell.lock_path.as_deref());
            return;
        }

        // Window close button -> MainFrame.processWindowEvent(WINDOW_CLOSING) -> safeExit().
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.state.push(Command::Exit);
        }

        shell::shortcuts::handle(&ctx, &mut self.state);
        self.tick_pending_record(&ctx);
        shell::updates::poll(&ctx, &mut self.state);
        self.run_commands(&ctx);

        shell::titlebar::title_bar(ui, self);
        shell::statusbar::status_bar(ui, &mut self.state);
        shell::sidebar::sidebar(ui, &mut self.state);
        shell::central::central(ui, &mut self.state, &mut self.views);

        if let Some(choice) = shell::dialogs::unsaved_changes(&ctx, &self.state, self.shell.confirm.is_some()) {
            self.resolve_confirm(&ctx, choice);
        }
        shell::toasts::show(&ctx, &mut self.state);

        // Commands pushed by the UI this frame run right away (next frame would add latency).
        if !self.state.commands.is_empty() {
            self.run_commands(&ctx);
            ctx.request_repaint();
        }
        self.update_title(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let p = Persisted {
            recent_files: self.state.recent_files.clone(),
            dark: self.shell.dark,
            sidebar_open: self.state.sidebar_open,
            sidebar: shell::sidebar::SECTIONS
                .iter()
                .position(|s| s.0 == self.state.sidebar)
                .unwrap_or(1),
            tab: shell::tabs::TABS.iter().position(|t| t.0 == self.state.tab).unwrap_or(0),
            last_dir: self.shell.last_dir.clone(),
            update_auto_check: self.state.update.auto_check,
            update_prerelease: self.state.update.include_prerelease,
            update_skipped: self.state.update.skipped_version.clone(),
            update_last_checked: self.state.update.last_checked.map(|d| d.to_rfc3339()),
        };
        eframe::set_value(storage, STORAGE_KEY, &p);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Window closed without safeExit (e.g. OS shutdown): stop scope and remove the lock file.
        if !self.shell.allow_close {
            self.state.scope.stop();
        }
        shell::startup::remove_lock(self.shell.lock_path.take().as_deref());
    }
}
