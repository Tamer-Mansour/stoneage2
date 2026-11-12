//! Remote diagnostics: everything the app does is written to a daily log file, crashes (panics)
//! are written to crash reports, and File → Diagnostics → "Export diagnostics…" bundles all of it
//! into one text file that can be sent to the developer.
//!
//! Files live in `%LOCALAPPDATA%\StoneAge2\logs\` (survive updates and reinstalls):
//! * `stoneage-YYYY-MM-DD.log` – one per day, the newest `KEEP_DAYS` are kept
//! * `crash-YYYYMMDD-HHMMSS.txt` – one per crash, with location and backtrace
//! * `crash-seen` – marker: crashes older than this were already reported to the user

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::state::AppState;

const KEEP_DAYS: u64 = 30;
/// Lines of log files included in an export (newest last).
const EXPORT_LOG_LINES: usize = 5000;

pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("StoneAge2").join("logs")
}

struct FileLogger {
    file: Mutex<Option<File>>,
}

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        // Everything from our crates; only warnings and errors from libraries (egui, wgpu, …).
        if m.target().starts_with("stoneage") {
            m.level() <= Level::Debug
        } else {
            m.level() <= Level::Warn
        }
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} {:5} [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            r.level(),
            r.target(),
            r.args()
        );
        if let Ok(mut g) = self.file.lock() {
            if let Some(f) = g.as_mut() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut g) = self.file.lock() {
            if let Some(f) = g.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

/// Installs the file logger and the crash (panic) hook. Call first thing in `main`.
pub fn init() {
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    prune(&dir);
    let path = dir.join(format!("stoneage-{}.log", chrono::Local::now().format("%Y-%m-%d")));
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let logger = Box::leak(Box::new(FileLogger { file: Mutex::new(file) }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let report = format!(
            "StoneAge 2 crash report\n\
             time:     {}\n\
             version:  {} ({})\n\
             thread:   {thread}\n\
             panic:    {info}\n\n\
             backtrace:\n{bt}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            env!("CARGO_PKG_VERSION"),
            arch(),
        );
        log::error!(target: "stoneage::crash", "PANIC in thread '{thread}': {info}");
        log::logger().flush();
        let name = format!("crash-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let _ = fs::write(log_dir().join(name), report);
        default_hook(info);
    }));

    log::info!(target: "stoneage::app", "==== StoneAge 2 {} ({}) started ====", env!("CARGO_PKG_VERSION"), arch());
    log::info!(target: "stoneage::app", "exe: {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default());
    log::info!(target: "stoneage::app", "os: {}", os_version());
}

/// Deletes log and crash files older than `KEEP_DAYS`.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let limit = Duration::from_secs(KEEP_DAYS * 24 * 3600);
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("stoneage-") || name.starts_with("crash-")) || name == "crash-seen" {
            continue;
        }
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > limit);
        if old {
            let _ = fs::remove_file(e.path());
        }
    }
}

pub fn arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86, 32-bit"
    } else if cfg!(target_arch = "x86_64") {
        "x64, 64-bit"
    } else {
        std::env::consts::ARCH
    }
}

/// Windows version string (`ver`), empty if unavailable.
pub fn os_version() -> String {
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", "ver"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| std::env::consts::OS.to_string())
}

/// Files in the log folder matching `prefix`, oldest first.
fn files(prefix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(log_dir())
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    n.starts_with(prefix) && n != "crash-seen"
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Crash reports written since the user was last told about crashes.
pub fn unseen_crashes() -> Vec<PathBuf> {
    let seen = fs::metadata(log_dir().join("crash-seen")).and_then(|m| m.modified()).ok();
    files("crash-")
        .into_iter()
        .filter(|p| match (seen, fs::metadata(p).and_then(|m| m.modified()).ok()) {
            (Some(s), Some(m)) => m > s,
            _ => true,
        })
        .collect()
}

pub fn mark_crashes_seen() {
    let _ = fs::write(log_dir().join("crash-seen"), chrono::Local::now().to_rfc3339());
}

/// The complete diagnostics report as text.
pub fn report(state: &AppState) -> String {
    let mut s = String::new();
    let now = chrono::Local::now();
    let sec = |s: &mut String, title: &str| {
        s.push_str(&format!("\n==================== {title} ====================\n"));
    };
    s.push_str("StoneAge 2 – diagnostics report\n");
    s.push_str(&format!("created:      {}\n", now.format("%Y-%m-%d %H:%M:%S %:z")));
    s.push_str(&format!("version:      {} ({})\n", env!("CARGO_PKG_VERSION"), arch()));
    s.push_str(&format!("assets:       {}\n", if stoneage_core::assets::ORIGINAL_ASSETS { "original" } else { "public" }));
    s.push_str(&format!("os:           {}\n", os_version()));
    s.push_str(&format!(
        "exe:          {}\n",
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
    ));
    s.push_str(&format!("config file:  {} (exists: {})\n", state.config_path.display(), state.config_path.is_file()));
    s.push_str(&format!("log folder:   {}\n", log_dir().display()));
    s.push_str(&format!("language:     {}\n", state.i18n.lang.code()));

    sec(&mut s, "DEVICE");
    s.push_str(&format!("scope kind:   {:?}\n", state.scope.kind()));
    s.push_str(&format!("scope ready:  {}\n", state.scope.is_ready()));
    s.push_str(&format!("scope run:    {}\n", state.scope_running));
    s.push_str(&format!("demo mode:    {}\n", state.demo_mode));
    s.push_str(&format!("audio open:   {}\n", state.audio.is_some()));
    if let Ok(dir) = std::env::current_exe().map(|p| p.parent().map(Path::to_path_buf).unwrap_or_default()) {
        for f in ["PCSGU250.dll", "PcLab2000LT.exe", "pcgu250.bit", "mpusbapi.dll", "FASTTime32.dll"] {
            s.push_str(&format!("next to exe:  {f:<16} {}\n", if dir.join(f).is_file() { "found" } else { "missing" }));
        }
    }

    sec(&mut s, "SURVEY");
    let sv = &state.survey;
    s.push_str(&format!(
        "file: {}\ndirty: {}\nrows: {}  active row: {}  max points: {}\n",
        state.file_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(untitled)".into()),
        state.dirty,
        sv.rows.len(),
        sv.active_row + 1,
        sv.max_points()
    ));
    for (i, r) in sv.rows.iter().enumerate() {
        let valid = r.points.iter().filter(|p| p.is_data_valid()).count();
        let acquired = r.points.iter().filter(|p| p.acquired).count();
        s.push_str(&format!(
            "  row {:>3}: {:>4} points, {:>4} acquired, {:>4} valid, min {} max {}\n",
            i + 1,
            r.points.len(),
            acquired,
            valid,
            r.minimum,
            r.maximum
        ));
    }
    s.push_str(&format!(
        "params: kb={} fRange={} dfh={} tm={} pointDist={} rowDist={} cut={} blockSize={} avg={} interp={} depth={}\n",
        sv.kb,
        sv.f_range,
        sv.dfh,
        sv.tm,
        sv.point_distance,
        sv.row_distance,
        sv.horizontal_cut,
        sv.block_size,
        sv.use_block_average,
        sv.interpolate,
        sv.show_depth
    ));

    sec(&mut s, "SETTINGS (StoneAge.ini)");
    s.push_str(&state.config.to_ini_string(&now.format("%d.%m.%Y, %H:%M").to_string()));

    sec(&mut s, "CRASH REPORTS");
    let crashes = files("crash-");
    if crashes.is_empty() {
        s.push_str("(none)\n");
    }
    for p in crashes.iter().rev().take(10) {
        s.push_str(&format!("--- {} ---\n", p.display()));
        s.push_str(&fs::read_to_string(p).unwrap_or_default());
    }

    sec(&mut s, "MESSAGES (this session)");
    for e in &state.log {
        s.push_str(&format!("{} {:5} {}\n", e.time.format("%H:%M:%S%.3f"), e.level, e.text));
    }

    sec(&mut s, "LOG FILES (newest last)");
    log::logger().flush();
    let mut lines: Vec<String> = Vec::new();
    for p in files("stoneage-") {
        if let Ok(text) = fs::read_to_string(&p) {
            lines.push(format!("--- {} ---", p.display()));
            lines.extend(text.lines().map(str::to_string));
        }
    }
    let skip = lines.len().saturating_sub(EXPORT_LOG_LINES);
    if skip > 0 {
        s.push_str(&format!("({skip} older lines omitted)\n"));
    }
    for l in &lines[skip..] {
        s.push_str(l);
        s.push('\n');
    }
    s
}

/// Opens the log folder in Explorer.
pub fn open_log_folder() {
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}
