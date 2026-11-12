//! Shared application state. Used by both app agents ("shell" owns main.rs/app.rs/shell/*,
//! "panels" owns panels/* and help/*). Add fields freely; do not rename/remove.

use std::path::PathBuf;
use std::time::Instant;

use stoneage_core::{AppConfig, I18n, Survey};
use stoneage_device::{AudioCapture, Oscilloscope};

/// Central tabs (original: Map, 3d, Areal, Row, Point, Help, Messages, Soundtest, Soundtest2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Map,
    View3d,
    Areal,
    Row,
    Point,
    Soundtest,
    Soundtest2,
}

/// Sections of the left sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SidebarSection {
    /// Files: new/open/save/save as, recent files, import/export CSV, load map, export image, print, save defaults.
    Files,
    /// Recording: oscilloscope start/stop/show, Record, Record delayed (countdown), Rec + new row, delayed variant.
    Measure,
    /// Survey parameters: title, date, description, Kb, fRange, dfh, point/row distance, Apply.
    Survey,
    /// Display: zoom X/Y/Z, block size, horizontal cut, interpolate, show depth, grid, block average,
    /// valid base/step, colour model, reverse colours.
    Display,
    /// Data: rows/points navigator (list), delete point/row/all, insert row, find/reset min-max.
    Data,
    /// Help center: user guide, keyboard shortcuts, about, license, language.
    Help,
    /// Messages / tracer log (original "Messages" tab, toggle Tracer).
    Messages,
    /// App updates from GitHub Releases (new in StoneAge 2).
    Updates,
}

/// Phase of the self-updater (`stoneage-update`).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateStatus {
    /// Nothing checked yet in this session.
    #[default]
    Idle,
    Checking,
    /// Running version is current (`latest` = newest published tag, if known).
    UpToDate { latest: Option<String> },
    /// `UpdateState::available` holds the release.
    Available,
    Downloading,
    /// Verified package at `UpdateState::package`.
    Ready,
    /// Last check / download failed (message for the user).
    Error(String),
}

/// Update settings and progress (settings are persisted by `app.rs`).
pub struct UpdateState {
    /// "Check automatically on startup" (default on).
    pub auto_check: bool,
    /// "Include pre-releases" (default off).
    pub include_prerelease: bool,
    /// Version the user chose to skip (no startup notification for it).
    pub skipped_version: Option<String>,
    pub last_checked: Option<chrono::DateTime<chrono::Local>>,
    pub status: UpdateStatus,
    pub available: Option<stoneage_update::ReleaseInfo>,
    pub progress: Option<stoneage_update::Progress>,
    /// Verified, downloaded package (zip).
    pub package: Option<PathBuf>,
    /// Running background job (check or download).
    pub job: Option<stoneage_update::Job>,
    /// The running check was started automatically (no "up to date"/error toasts).
    pub silent: bool,
    /// Badge on the Updates rail icon (new version not yet looked at).
    pub badge: bool,
    /// The startup check has been considered.
    pub startup_done: bool,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            auto_check: true,
            include_prerelease: false,
            skipped_version: None,
            last_checked: None,
            status: UpdateStatus::Idle,
            available: None,
            progress: None,
            package: None,
            job: None,
            silent: false,
            badge: false,
            startup_done: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub time: chrono::DateTime<chrono::Local>,
    pub level: log::Level,
    pub text: String,
}

/// Pending "Record delayed" countdown.
#[derive(Clone, Debug)]
pub struct PendingRecord {
    pub fire_at: Instant,
    /// true = "Rec + new row" variant.
    pub new_row: bool,
}

pub struct AppState {
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub i18n: I18n,
    pub survey: Survey,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub recent_files: Vec<PathBuf>,
    pub tab: Tab,
    pub sidebar: SidebarSection,
    pub sidebar_open: bool,
    pub scope: Box<dyn Oscilloscope>,
    pub scope_running: bool,
    pub audio: Option<AudioCapture>,
    pub demo_mode: bool,
    pub pending_record: Option<PendingRecord>,
    pub log: Vec<LogEntry>,
    /// Debug tracing (original Tracer "debug" checkbox).
    pub trace_debug: bool,
    pub status_hover: Option<String>,
    pub toasts: Vec<(Instant, String)>,
    /// Set by panels to ask the shell to run a command on the next frame.
    pub commands: Vec<Command>,
    /// Self-updater state (Updates section).
    pub update: UpdateState,
}

/// Every user action. Panels/menus/shortcuts push these; the shell executes them in one place
/// (`shell::commands::execute`), so menu, sidebar buttons and shortcuts behave identically.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    NewSurvey,
    Open,
    OpenPath(PathBuf),
    Save,
    SaveAs,
    ExportCsv,
    ImportCsv,
    LoadMap,
    ClearMap,
    ExportImage,
    Print,
    SaveDefaults,
    Exit,
    DeleteRow,
    DeletePoint,
    DeleteAllRows,
    NewRow,
    NextRow,
    PrevRow,
    NextPoint,
    PrevPoint,
    SelectRow(usize),
    SelectPoint(usize),
    FindMinMax,
    ResetMinMax,
    ToggleTracer,
    RecreateScenegraph,
    SetLanguage(stoneage_core::Language),
    StartScope,
    StopScope,
    ShowScopeGui(bool),
    /// Record current point and append a new one (`jbPunktNeu`).
    Record { delayed: bool },
    /// Record current point and create a new row (`jbReiheNeu`).
    RecordNewRow { delayed: bool },
    CancelDelayedRecord,
    SwitchTab(Tab),
    ShowSection(SidebarSection),
    /// Ask GitHub for a newer release (`silent`: automatic startup check, no "up to date" toast).
    CheckForUpdates { silent: bool },
    /// Download + verify the available release in the background.
    DownloadUpdate,
    /// Install the downloaded package and restart (asks about unsaved changes first).
    InstallUpdate,
    /// Cancel the running check / download.
    CancelUpdate,
    /// Do not notify about the available version again.
    SkipVersion,
    /// Open the GitHub releases page in the browser.
    OpenReleasesPage,
    /// File → Diagnostics → Export diagnostics… (logs, crash reports, device and settings as text).
    ExportDiagnostics,
    /// File → Diagnostics → Open log folder.
    OpenLogFolder,
}

impl AppState {
    pub fn push(&mut self, c: Command) {
        self.commands.push(c);
    }
    pub fn log(&mut self, level: log::Level, text: impl Into<String>) {
        let text = text.into();
        // Also into the daily log file (shell::diagnostics).
        log::log!(target: "stoneage::ui", level, "{text}");
        self.log.push(LogEntry { time: chrono::Local::now(), level, text });
    }
    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push((Instant::now(), text.into()));
    }
}
