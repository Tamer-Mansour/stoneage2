//! StoneAge 2 – application shell. OWNER: agent "app-shell".
//!
//! Port of `Start` (startup sequence, splash, lock file, panicExit) and `MainFrame`
//! (menu, tabs, status bar, actionPerformed). See `app.rs` and `shell/`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod help;
mod panels;
mod shell;
mod state;

fn main() -> eframe::Result<()> {
    // Daily log file + crash reports (File → Diagnostics).
    shell::diagnostics::init();

    // After a self-update restart: wait for the old process, delete the replaced files.
    stoneage_update::after_restart(&shell::startup::lock_file_path());

    // Start.<clinit> / Start.main: ini, locale, lock file, scope library, survey.
    let startup = shell::startup::run();

    let viewport = egui::ViewportBuilder::default()
        .with_title(format!("StoneAge 2 – {}", stoneage_core::ORIGINAL_VERSION))
        // Original: MainFrame.jbInit setSize(1000, 700); Start.main then 800x600 (minimum here).
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 600.0])
        // Own title bar with the menus and tabs next to the name (shell::titlebar).
        .with_decorations(false)
        .with_resizable(true)
        .with_icon(shell::splash::window_icon());

    let options = eframe::NativeOptions {
        viewport,
        persist_window: true,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "StoneAge 2",
        options,
        Box::new(move |cc| Ok(Box::new(app::StoneAgeApp::new(cc, startup)))),
    )
}
