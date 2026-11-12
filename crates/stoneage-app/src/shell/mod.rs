//! Window layout, menu bar, tabs, status bar, command execution. OWNER: agent "app-shell".
//!
//! * `startup`   – `Start.<clinit>`/`Start.main`/`MainFrame.<clinit>`: ini, locale, lock file, scope, panicExit
//! * `splash`    – `Splash` / `Splash$MyWindow` (inside the main window)
//! * `commands`  – `MainFrame.actionPerformed` + `OptionPanel` record/scope listeners, one place for all `Command`s
//! * `menu`, `tabs`, `sidebar`, `statusbar`, `central` – the `MainFrame` layout
//! * `files`, `print` – file dialogs (original FileFilter classes), print / export image
//! * `shortcuts`, `toasts`, `dialogs`, `theme`, `trace`
//! * `icons` – embedded tlp-icons (SVG), hover motion, `IconButton`

pub mod central;
pub mod commands;
pub mod diagnostics;
pub mod dialogs;
pub mod files;
pub mod icons;
pub mod menu;
pub mod print;
pub mod shortcuts;
pub mod sidebar;
pub mod splash;
pub mod startup;
pub mod statusbar;
pub mod tabs;
pub mod titlebar;
pub mod theme;
pub mod toasts;
pub mod trace;
pub mod updates;

use stoneage_core::I18n;

use crate::state::AppState;

/// Translate `key`; if the key is missing in all bundles (I18n returns the key), use `fallback`.
/// Used for the new `App.*` keys (listed in docs/NOTES-app-shell.md) until they are in the bundles.
pub fn t(i18n: &I18n, key: &str, fallback: &str) -> String {
    let s = i18n.tr(key);
    if s == key || s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// Like [`t`] with `MessageFormat`-style `{0}`, `{1}` substitution.
pub fn tf(i18n: &I18n, key: &str, fallback: &str, args: &[&dyn std::fmt::Display]) -> String {
    let s = i18n.trf(key, args);
    if s == key || s.is_empty() || s.starts_with(key) {
        format_args_msg(fallback, args)
    } else {
        s
    }
}

/// Minimal `{n}` substitution for fallback strings.
pub fn format_args_msg(pattern: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = pattern.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &a.to_string());
    }
    out
}

/// Display name of the current document (file name, else survey name, else "untitled").
pub fn document_name(state: &AppState) -> String {
    if let Some(name) = state.file_path.as_ref().and_then(|p| p.file_name()) {
        return name.to_string_lossy().into_owned();
    }
    if !state.survey.name.trim().is_empty() {
        return state.survey.name.clone();
    }
    t(&state.i18n, "App.Untitled", "untitled")
}

/// Window title "StoneAge 2 – <file>*".
pub fn window_title(state: &AppState) -> String {
    format!("StoneAge 2 – {}{}", document_name(state), if state.dirty { "*" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::format_args_msg;

    #[test]
    fn fallback_substitution() {
        assert_eq!(format_args_msg("Row {0} / {1}", &[&3, &7]), "Row 3 / 7");
        assert_eq!(format_args_msg("no args", &[]), "no args");
    }
}
