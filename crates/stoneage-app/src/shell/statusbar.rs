//! Bottom status bar – port of `MainFrame.jPanel1`: `lbl_Status` (last trace), `jbReiheDec`
//! `lbl_Reihe` `jbReiheInc`, `jbPunktDec` `lbl_Punkt` `jbPunktInc` ("Row {0} / {1}", "Point {0} / {1}"),
//! plus scope state (Demo / Connected), record countdown, hover info and file name + dirty dot.
//! Rendered as pill badges on the chrome surface.

use std::time::Instant;

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Vec2};

use super::icons::{self, IconButton};
use super::theme::{Palette, OK_GREEN, WARN_AMBER};
use super::{document_name, t, tf, trace};
use crate::state::{AppState, Command};

pub const HEIGHT: f32 = 32.0;

/// `lbl_Reihe`: MessageFormat("Row {0} / {1}", activeReihe + 1, reihen.size()).
pub fn row_label(state: &AppState) -> String {
    let s = &state.survey;
    tf(&state.i18n, "MainFrame.lbl_Reihe.Caption", "Row {0} / {1}", &[&(s.active_row + 1), &s.rows.len()])
}

/// `lbl_Punkt`: MessageFormat("Point {0} / {1}", activePunkt + 1, countPoints()).
pub fn point_label(state: &AppState) -> String {
    let (a, n) = state
        .survey
        .current_row()
        .map(|r| (r.active_point + 1, r.count_points()))
        .unwrap_or((0, 0));
    tf(&state.i18n, "MainFrame.lbl_Punkt.Caption", "Point {0} / {1}", &[&a, &n])
}

pub fn status_bar(ui: &mut egui::Ui, state: &mut AppState) {
    let p = Palette::of(ui.visuals());
    egui::Panel::bottom("stoneage_status")
        .resizable(false)
        .exact_size(HEIGHT)
        .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(10, 0)))
        .show(ui, |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                navigator(ui, state, &p);
                scope_chip(ui, state, &p);
                countdown(ui, state, &p);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    file_pill(ui, state, &p);
                    // Hover info from the views, else the last trace (lbl_Status).
                    let text = state
                        .status_hover
                        .clone()
                        .or_else(|| trace::last_message(state).map(str::to_string))
                        .unwrap_or_default();
                    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(p.text_weak)).truncate());
                });
            });
        });
}

/// Rounded pill frame used by every badge of the status bar.
fn pill<R>(ui: &mut egui::Ui, fill: Color32, stroke: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(11)
        .inner_margin(egui::Margin { left: 4, right: 4, top: 1, bottom: 1 })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(add).inner
        })
}

fn nav_pair(ui: &mut egui::Ui, p: &Palette, label: String, prev_tip: String, next_tip: String) -> (bool, bool) {
    let mut out = (false, false);
    pill(ui, p.panel, p.hairline, |ui| {
        out.0 = ui
            .add(IconButton::icon_only("chevron-left", 24.0).icon_size(13.0).corner(10))
            .on_hover_text(prev_tip)
            .clicked();
        ui.add_space(2.0);
        ui.add(egui::Label::new(RichText::new(label).monospace().size(12.0).color(p.text)).selectable(false));
        ui.add_space(2.0);
        out.1 = ui
            .add(IconButton::icon_only("chevron-right", 24.0).icon_size(13.0).corner(10))
            .on_hover_text(next_tip)
            .clicked();
    });
    out
}

fn navigator(ui: &mut egui::Ui, state: &mut AppState, p: &Palette) {
    // Original buttons `jbReiheDec` / `jbReiheInc` ("<<" / ">>"): their captions stay in the tooltips.
    let prev_row = t(&state.i18n, "MainFrame.Menu.Edit.prevRow.Caption", "Previous Row");
    let next_row = t(&state.i18n, "MainFrame.Menu.Edit.nextRow.Caption", "Next Row");
    let (prev, next) = nav_pair(ui, p, row_label(state), format!("{prev_row} (↑)"), format!("{next_row} (↓)"));
    if prev {
        state.push(Command::PrevRow);
    }
    if next {
        state.push(Command::NextRow);
    }
    let (prev, next) = nav_pair(ui, p, point_label(state), "←".into(), "→".into());
    if prev {
        state.push(Command::PrevPoint);
    }
    if next {
        state.push(Command::NextPoint);
    }
}

fn scope_chip(ui: &mut egui::Ui, state: &AppState, p: &Palette) {
    let (text, color) = if state.demo_mode {
        (t(&state.i18n, "App.Status.Demo", "Demo"), WARN_AMBER)
    } else {
        (t(&state.i18n, "App.Status.Connected", "Connected"), OK_GREEN)
    };
    let fill = if p.dark { color.gamma_multiply(0.12) } else { color.gamma_multiply(0.14) };
    pill(ui, fill, color.gamma_multiply(0.45), |ui| {
        ui.add_space(4.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 20.0), Sense::hover());
        // Live dot with a soft halo.
        ui.painter().circle_filled(r.center(), 5.0, color.gamma_multiply(0.25));
        ui.painter().circle_filled(r.center(), 3.0, color);
        ui.add_space(3.0);
        ui.label(RichText::new(text).size(12.0).color(if p.dark { color } else { icons::darken(color, 0.25) }));
        ui.add_space(4.0);
    });
}

fn countdown(ui: &mut egui::Ui, state: &mut AppState, p: &Palette) {
    let Some(pend) = &state.pending_record else { return };
    let left = pend.fire_at.saturating_duration_since(Instant::now());
    let secs = left.as_secs_f32();
    let text = tf(&state.i18n, "App.Status.Countdown", "Recording in {0} s", &[&format!("{secs:.1}")]);
    let cancel = t(&state.i18n, "App.Status.Cancel", "Cancel");
    let mut cancel_clicked = false;
    pill(ui, p.accent_soft, p.accent.gamma_multiply(0.6), |ui| {
        ui.add_space(4.0);
        icons::icon(ui, "timer", 14.0, p.accent);
        ui.add_space(3.0);
        ui.label(RichText::new(text).size(12.0).color(p.accent).strong());
        ui.add_space(4.0);
        cancel_clicked = ui
            .add(IconButton::icon_only("x", 20.0).icon_size(12.0).corner(10))
            .on_hover_text(format!("{cancel} (Esc)"))
            .clicked();
    });
    if cancel_clicked {
        state.push(Command::CancelDelayedRecord);
    }
}

fn file_pill(ui: &mut egui::Ui, state: &AppState, p: &Palette) {
    let name = document_name(state);
    let full = state.file_path.as_ref().map(|pth| pth.display().to_string()).unwrap_or_else(|| name.clone());
    let modified = t(&state.i18n, "App.Status.Modified", "Unsaved changes");
    pill(ui, p.panel, p.hairline, |ui| {
        ui.add_space(4.0);
        if state.dirty {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(8.0, 20.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.5, WARN_AMBER);
            resp.on_hover_text(&modified);
            ui.add_space(3.0);
        }
        ui.label(RichText::new(name).size(12.0).color(p.text));
        ui.add_space(3.0);
        icons::icon(ui, "file-text", 14.0, p.text_weak);
        ui.add_space(4.0);
    })
    .response
    .on_hover_text(full);
}
