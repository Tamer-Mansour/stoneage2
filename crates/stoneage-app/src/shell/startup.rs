//! Startup sequence: port of `Start.<clinit>`, `MainFrame.<clinit>`, `MainFrame.<init>` and `Start.main`.
//!
//! Order (as in the original):
//! 1. `MainFrame.<clinit>`: load `StoneAge.ini` (exe directory; else built-in defaults), set locale,
//!    create the survey (`MessAreal`), "Initialisiere GUI (Phase 1/2) ...".
//! 2. `Start.<clinit>`: "loading Library ...", create the lock file `stoneage.lock`
//!    (already present -> `panicExit("lock file present - stoneage allready running?")`).
//! 3. `Start.main`: optional `debug` argument, load the scope library (failure -> demo mode),
//!    "constructing MainFrame", show the frame, "deleting Splash".
//!
//! `MainFrame.demoMode` is `true` after startup in every case; it becomes `false` only when the
//! oscilloscope is started (`OptionPanel$22`).

use std::path::{Path, PathBuf};

use stoneage_core::{AppConfig, I18n, Language, Survey};
use stoneage_device::open_scope;

use crate::state::{AppState, SidebarSection, Tab};

pub const INI_NAME: &str = "StoneAge.ini";
pub const LOCK_NAME: &str = "stoneage.lock";

pub struct Startup {
    pub state: AppState,
    /// Splash status lines in order (`Splash.setStatus`).
    pub splash_lines: Vec<String>,
    /// `Start.panicExit(msg)` was called: the splash stays, a click exits.
    pub panic: Option<String>,
    /// Lock file created by us (removed on exit, `File.deleteOnExit`).
    pub lock_path: Option<PathBuf>,
}

/// Directory of the executable (where `StoneAge.ini`, `PCSGU250.dll` are looked up).
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Path of the lock file. The original used `user.dir + "\\stoneage.lock"`; the temp directory
/// is used here so a read-only install directory does not trigger "lock file error".
pub fn lock_file_path() -> PathBuf {
    std::env::temp_dir().join(LOCK_NAME)
}

/// Creates the lock file containing our PID. A lock left behind by a crashed or killed instance
/// (its PID is no longer a running StoneAge process) is removed instead of blocking startup –
/// the original would show "allready running" forever after a crash.
fn acquire_lock(lock: &Path) -> std::io::Result<()> {
    use std::io::Write;
    for _ in 0..2 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(lock) {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let owner = std::fs::read_to_string(lock).ok().and_then(|s| s.trim().parse::<u32>().ok());
                match owner {
                    Some(pid) if pid != std::process::id() && process_alive(pid) => return Err(e),
                    _ => {
                        // Stale (or unreadable, from an old version): take it over.
                        std::fs::remove_file(lock)?;
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "lock file present"))
}

/// True if a process with this PID is running a `stoneage` executable.
fn process_alive(pid: u32) -> bool {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    match cmd.output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_ascii_lowercase().contains("stoneage"),
        // Can't tell: be safe and treat it as running.
        Err(_) => true,
    }
}

pub fn remove_lock(path: Option<&Path>) {
    if let Some(p) = path {
        let _ = std::fs::remove_file(p);
    }
}

/// Splits "de_DE" into ("de", "DE").
pub fn lang_parts(lang: Language) -> (String, String) {
    let code = lang.code();
    let mut it = code.split('_');
    (it.next().unwrap_or("en").to_string(), it.next().unwrap_or("US").to_string())
}

pub fn run() -> Startup {
    let mut lines: Vec<String> = Vec::new();
    let mut panic: Option<String> = None;
    let debug = std::env::args().nth(1).is_some_and(|a| a == "debug");

    // --- MainFrame.<clinit> --------------------------------------------------------------
    let dir = exe_dir();
    let config_path = dir.join(INI_NAME);
    let mut early_log: Vec<(log::Level, String)> = vec![(log::Level::Debug, "MainFrame : loading ini file...".into())];
    let config = if config_path.is_file() {
        match AppConfig::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                early_log.push((log::Level::Warn, format!("MainFrame : {}: {e}", config_path.display())));
                early_log.push((log::Level::Debug, "MainFrame : No resource ini file - using default ...".into()));
                AppConfig::default()
            }
        }
    } else {
        early_log.push((log::Level::Debug, "MainFrame : No local ini file - trying built-in ...".into()));
        AppConfig::default()
    };
    lines.push("Initialisiere GUI (Phase 1) ...".into());
    lines.push(" setting locale ...".into());
    let lang = Language::from_ini(&config.language, &config.country);
    let i18n = I18n::new(lang);
    lines.push("Initialisiere GUI (Phase 2) ...".into());
    let survey = Survey::new(&config);
    lines.push("Initialisiere GUI (Phase 1) ... ok".into());

    // --- Start.<clinit> ------------------------------------------------------------------
    lines.push(i18n.tr("Splash.Status.loadingLibrary"));
    let lock = lock_file_path();
    let mut lock_path = None;
    match acquire_lock(&lock) {
        Ok(()) => {
            lines.push(format!("lockfile: {}", lock.display()));
            lock_path = Some(lock.clone());
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            panic = Some("lock file present - stoneage allready running?".into());
        }
        Err(e) => {
            lines.push(e.to_string());
            panic = Some("lock file error - check permissions!".into());
        }
    }

    // --- Start.main ----------------------------------------------------------------------
    let (scope, scope_err) = open_scope(Some(&dir));
    match &scope_err {
        None => lines.push("loading stoneage.dll ...".into()),
        Some(e) => {
            lines.push(e.to_string());
            lines.push("loading Library ... failed. switched to demoMode".into());
        }
    }
    lines.push("constructing MainFrame".into());
    lines.push("constructing MainFrame ... ok".into());
    lines.push("deleting Splash".into());
    if let Some(p) = &panic {
        lines.push(format!("panic: {p}"));
    }

    let mut state = AppState {
        config,
        config_path,
        i18n,
        survey,
        file_path: None,
        dirty: false,
        recent_files: Vec::new(),
        tab: Tab::Map,
        sidebar: SidebarSection::Measure,
        sidebar_open: true,
        scope,
        scope_running: false,
        audio: None,
        demo_mode: true,
        pending_record: None,
        log: Vec::new(),
        trace_debug: debug,
        status_hover: None,
        toasts: Vec::new(),
        commands: Vec::new(),
        update: Default::default(),
    };
    for (level, text) in early_log {
        state.log(level, text);
    }
    for (i, l) in lines.iter().enumerate() {
        state.log(log::Level::Debug, format!("Splash :  {} {l}", i + 1));
    }
    state.log(log::Level::Info, format!("MainFrame : {}", stoneage_core::ORIGINAL_VERSION));
    if scope_err.is_some() {
        state.log(log::Level::Warn, "Start : loading Library ... failed. switched to demoMode");
    }

    Startup { state, splash_lines: lines, panic, lock_path }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_parts() {
        assert_eq!(lang_parts(Language::German), ("de".into(), "DE".into()));
        assert_eq!(lang_parts(Language::Turkish), ("tr".into(), "TR".into()));
    }
}
