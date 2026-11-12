//! Port of `Tracer.trace(Object, String)`: messages go to the log (Messages section) and the
//! last one is shown in the status bar (`MainFrame.lbl_Status`).
//!
//! The original only recorded traces while the "Debug Modus" checkbox was set. Here every
//! trace is kept; internal traces use `Level::Debug` (the Messages panel may hide them unless
//! `state.trace_debug`), user-relevant results use Info/Warn/Error.

use log::Level;

use crate::state::AppState;

/// `Tracer.trace(origin, msg)` – internal trace (debug level).
pub fn trace(state: &mut AppState, origin: &str, msg: impl AsRef<str>) {
    push(state, Level::Debug, origin, msg.as_ref());
}

/// User-relevant information (e.g. "Save: x erfolgreich gespeichert!").
pub fn info(state: &mut AppState, origin: &str, msg: impl AsRef<str>) {
    push(state, Level::Info, origin, msg.as_ref());
}

pub fn warn(state: &mut AppState, origin: &str, msg: impl AsRef<str>) {
    push(state, Level::Warn, origin, msg.as_ref());
}

pub fn error(state: &mut AppState, origin: &str, msg: impl AsRef<str>) {
    push(state, Level::Error, origin, msg.as_ref());
}

fn push(state: &mut AppState, level: Level, origin: &str, msg: &str) {
    // AppState::log also writes the daily log file.
    state.log(level, format!("{origin} : {msg}"));
}

/// Time stamp format of the original: `DateFormat.getTimeInstance()` + "." + milliseconds (3 digits).
#[allow(dead_code)] // helper for the Messages section (app-panels)
pub fn format_time(t: &chrono::DateTime<chrono::Local>) -> String {
    format!("{}.{:03}", t.format("%H:%M:%S"), t.timestamp_subsec_millis() % 1000)
}

/// Text for the status bar label (`lbl_Status`): the last trace message.
pub fn last_message(state: &AppState) -> Option<&str> {
    state.log.last().map(|e| e.text.as_str())
}
