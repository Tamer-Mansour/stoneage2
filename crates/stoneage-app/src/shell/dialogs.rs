//! Modal dialogs of the shell: "unsaved changes" before Exit / Open / New.
//! (The original `safeExit` exited without asking; the prompt is a modern addition.)

use egui::{Id, RichText};

use super::commands::ConfirmChoice;
use super::{document_name, t, tf};
use crate::state::AppState;

/// Shows the dialog while `open`; returns the user's choice once made.
pub fn unsaved_changes(ctx: &egui::Context, state: &AppState, open: bool) -> Option<ConfirmChoice> {
    if !open {
        return None;
    }
    let i18n = &state.i18n;
    let mut choice = None;
    let resp = egui::Modal::new(Id::unique("stoneage_unsaved_changes")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.label(RichText::new(t(i18n, "App.Unsaved.Title", "Unsaved changes")).heading());
        ui.add_space(8.0);
        ui.label(tf(
            i18n,
            "App.Unsaved.Text",
            "\"{0}\" has unsaved changes. Do you want to save them first?",
            &[&document_name(state)],
        ));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(RichText::new(t(i18n, "App.Unsaved.Save", "Save")).strong()).fill(stoneage_plots::theme::ACCENT))
                .clicked()
            {
                choice = Some(ConfirmChoice::Save);
            }
            if ui.button(t(i18n, "App.Unsaved.Discard", "Don't save")).clicked() {
                choice = Some(ConfirmChoice::Discard);
            }
            if ui.button(t(i18n, "App.Unsaved.Cancel", "Cancel")).clicked() {
                choice = Some(ConfirmChoice::Cancel);
            }
        });
    });
    if choice.is_none() && (resp.should_close() || ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
        choice = Some(ConfirmChoice::Cancel);
    }
    choice
}
