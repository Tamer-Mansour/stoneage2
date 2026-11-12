//! "Updates" section: version info, update status, download progress, settings.
//! Logic lives in `shell/updates.rs` (commands) and the `stoneage-update` crate.

use egui::{RichText, Ui};

use super::{card, err_color, info_row, ok_color, status_dot, warn_color, wide_button};
use crate::shell::updates::{releases_page, CURRENT_VERSION};
use crate::shell::{t, tf};
use crate::state::{AppState, Command, UpdateStatus};

const MB: f64 = 1024.0 * 1024.0;

fn tt(state: &AppState, key: &str, fallback: &str) -> String {
    t(&state.i18n, key, fallback)
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / MB)
}

pub fn show(ui: &mut Ui, state: &mut AppState) {

    // ------------------------------------------------------------------ this version
    card(ui, &tt(state, "App.Update.ThisVersion", "This version"), |ui| {
        info_row(ui, &tt(state, "App.Update.Version", "Version"), RichText::new(CURRENT_VERSION).strong());
        info_row(ui, &tt(state, "App.Update.Build", "Build"), format!("Windows {}", stoneage_update::build_arch()));
    });

    // ------------------------------------------------------------------ status
    let busy = state.update.job.is_some();
    let status = state.update.status.clone();
    card(ui, &tt(state, "App.Update.Status", "Status"), |ui| {
        match &status {
            UpdateStatus::Idle => {
                ui.label(RichText::new(tt(state, "App.Update.NotChecked", "Not checked yet")).weak());
            }
            UpdateStatus::Checking => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(tt(state, "App.Update.Checking", "Checking for updates…"));
                });
            }
            UpdateStatus::UpToDate { latest } => {
                status_dot(ui, ok_color(ui), tt(state, "App.Update.UpToDate", "StoneAge is up to date"));
                if let Some(l) = latest {
                    info_row(ui, &tt(state, "App.Update.Latest", "Latest release"), l.as_str());
                }
            }
            UpdateStatus::Error(msg) => {
                status_dot(ui, err_color(ui), msg.as_str());
            }
            UpdateStatus::Available | UpdateStatus::Downloading | UpdateStatus::Ready => {}
        }

        if let (Some(rel), true) = (
            state.update.available.clone(),
            matches!(status, UpdateStatus::Available | UpdateStatus::Downloading | UpdateStatus::Ready),
        ) {
            let v = rel.version.to_string();
            let headline = tf(&state.i18n, "App.Update.Available", "New version {0} available", &[&v]);
            status_dot(ui, warn_color(ui), RichText::new(headline).strong());
            if state.update.skipped_version.as_deref() == Some(v.as_str()) {
                ui.label(RichText::new(tt(state, "App.Update.IsSkipped", "(this version is skipped)")).weak().small());
            }
            ui.add_space(4.0);
            if rel.name != rel.tag {
                info_row(ui, &tt(state, "App.Update.Name", "Release"), rel.name.as_str());
            }
            if let Some(d) = rel.date() {
                info_row(ui, &tt(state, "App.Update.Date", "Released"), d);
            }
            if rel.prerelease {
                info_row(ui, "", RichText::new(tt(state, "App.Update.Prerelease", "Pre-release")).color(warn_color(ui)));
            }
            info_row(ui, &tt(state, "App.Update.Size", "Download size"), mb(rel.asset.size));
            let notes = stoneage_update::notes_to_text(&rel.notes);
            if !notes.is_empty() {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(tt(state, "App.Update.Notes", "Release notes"))
                    .id_salt("update_release_notes")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("update_notes_scroll").max_height(220.0).show(ui, |ui| {
                            ui.add(egui::Label::new(notes).wrap());
                        });
                    });
            }
        }

        // Download progress.
        if status == UpdateStatus::Downloading {
            ui.add_space(6.0);
            let p = state.update.progress.unwrap_or_default();
            let speed = p.bytes_per_sec / MB;
            let text = match p.total {
                Some(total) if total > 0 => format!("{} / {} · {speed:.1} MB/s", mb(p.downloaded), mb(total)),
                _ => format!("{} · {speed:.1} MB/s", mb(p.downloaded)),
            };
            let bar = egui::ProgressBar::new(p.fraction().unwrap_or(0.0)).text(text).animate(p.fraction().is_none());
            ui.add(bar);
        }
        if status == UpdateStatus::Ready {
            ui.add_space(4.0);
            status_dot(ui, ok_color(ui), tt(state, "App.Update.ReadyText", "Downloaded and verified. Ready to install."));
        }
    });

    // ------------------------------------------------------------------ actions
    ui.add_enabled_ui(!busy, |ui| {
        if wide_button(ui, tt(state, "App.Update.CheckNow", "Check now"), 30.0).clicked() {
            state.push(Command::CheckForUpdates { silent: false });
        }
    });
    if busy {
        ui.add_space(4.0);
        if wide_button(ui, tt(state, "App.Update.Cancel", "Cancel"), 30.0).clicked() {
            state.push(Command::CancelUpdate);
        }
    }
    let has_release = state.update.available.is_some();
    if has_release && status == UpdateStatus::Available && !busy {
        ui.add_space(4.0);
        let text = RichText::new(tt(state, "App.Update.Download", "Download")).strong();
        if wide_button(ui, text, 30.0).clicked() {
            state.push(Command::DownloadUpdate);
        }
    }
    if status == UpdateStatus::Ready {
        ui.add_space(4.0);
        let text = RichText::new(tt(state, "App.Update.Install", "Install & restart")).strong();
        if wide_button(ui, text, 30.0)
            .on_hover_text(tt(state, "App.Update.InstallTip", "Replaces the program files and restarts StoneAge"))
            .clicked()
        {
            state.push(Command::InstallUpdate);
        }
    }
    let skippable = state
        .update
        .available
        .as_ref()
        .is_some_and(|r| state.update.skipped_version.as_deref() != Some(r.version.to_string().as_str()));
    if skippable && matches!(status, UpdateStatus::Available | UpdateStatus::Ready) {
        ui.add_space(4.0);
        if wide_button(ui, tt(state, "App.Update.Skip", "Skip this version"), 26.0).clicked() {
            state.push(Command::SkipVersion);
        }
    }
    ui.add_space(10.0);

    // ------------------------------------------------------------------ settings
    let auto_label = tt(state, "App.Update.AutoCheck", "Check automatically on startup");
    let pre_label = tt(state, "App.Update.IncludePre", "Include pre-releases");
    let settings_title = tt(state, "App.Update.Settings", "Settings");
    card(ui, &settings_title, |ui| {
        ui.checkbox(&mut state.update.auto_check, auto_label);
        ui.checkbox(&mut state.update.include_prerelease, pre_label);
        ui.add_space(4.0);
        let last = state
            .update
            .last_checked
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| tt(state, "App.Update.Never", "never"));
        info_row(ui, &tt(state, "App.Update.LastChecked", "Last checked"), last);
        ui.add_space(4.0);
        let url = releases_page(state);
        if ui
            .link(tt(state, "App.Update.ReleasesPage", "Releases page on GitHub"))
            .on_hover_text(url.as_str())
            .clicked()
        {
            state.push(Command::OpenReleasesPage);
        }
    });
}
