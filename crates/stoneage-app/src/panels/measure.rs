//! Measure section: oscilloscope status/start/stop/show GUI (`jtbStartOszi` = OptionPanel$22,
//! `jcbShowOsziGui` = OptionPanel$23), the record buttons (`jbPunktNeu` = $20, `jbReiheNeu` = $21,
//! `jbPunktNeu1` / `jbReiheNeu1` = delayed variants), delayed-record countdown, position and
//! information about the last point of the current row.

use std::time::{Duration, Instant};

use egui::{RichText, Ui, Vec2};
use stoneage_device::ScopeKind;

use super::{err_color, icard, icons, info_row, ok_color, status_dot, tr, trf, warn_color, IconButton};
use crate::state::{AppState, Command};

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    scope_card(ui, state);
    record_card(ui, state);
    position_card(ui, state);
    last_point_card(ui, state);
}

fn scope_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "radar", &tr(state, "App.Measure.Scope"), |ui| {
        let kind = state.scope.kind();
        let ready = state.scope.is_ready();
        match kind {
            ScopeKind::Demo => {
                status_dot(ui, warn_color(ui), RichText::new(tr(state, "App.Measure.ScopeDemo")).strong());
                ui.label(RichText::new(tr(state, "MessPunkt.Demomode.Caption")).small().color(warn_color(ui)));
            }
            ScopeKind::Pcsgu250 if ready => {
                status_dot(ui, ok_color(ui), RichText::new(tr(state, "App.Measure.ScopeConnected")).strong());
            }
            ScopeKind::Pcsgu250 => {
                status_dot(ui, err_color(ui), RichText::new(tr(state, "App.Measure.ScopeNotReady")).strong());
            }
        }
        info_row(ui, &tr(state, "App.Measure.Device"), match kind {
            ScopeKind::Demo => "Demo".to_string(),
            ScopeKind::Pcsgu250 => "Velleman PCSGU250".to_string(),
        });
        let running = if state.scope_running { tr(state, "App.Measure.Running") } else { tr(state, "App.Measure.Stopped") };
        info_row(ui, &tr(state, "App.Measure.State"), running);
        ui.add_space(6.0);

        // jtbStartOszi – toggle button whose caption shows the action
        let (icon, label, cmd) = if state.scope_running {
            ("circle-stop", tr(state, "OptionPanel.jtbStartOszi.Stop.Caption"), Command::StopScope)
        } else {
            ("circle-play", tr(state, "OptionPanel.jtbStartOszi.Start.Caption"), Command::StartScope)
        };
        let w = ui.available_width();
        if ui
            .add(IconButton::new(icon, label).selected(state.scope_running).min_size(Vec2::new(w, 34.0)))
            .on_hover_text(tr(state, "App.Measure.StartStop.ToolTip"))
            .clicked()
        {
            state.push(cmd);
        }
        // jcbShowOsziGui
        let mut vis = state.scope.gui_visible();
        if ui
            .checkbox(&mut vis, tr(state, "OptionPanel.jcbShowOsziGui.Caption"))
            .on_hover_text(tr(state, "App.Measure.ShowGui.ToolTip"))
            .changed()
        {
            state.push(Command::ShowScopeGui(vis));
        }
    });
}

fn record_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "circle-dot", &tr(state, "App.Measure.Record"), |ui| {
        let pending = state.pending_record.clone();
        let w = ui.available_width();
        let gap = ui.spacing().item_spacing.x;
        let half = ((w - gap) / 2.0).floor();

        if !state.scope_running {
            ui.label(RichText::new(tr(state, "App.Measure.ScopeNotRunning")).small().color(warn_color(ui)));
            ui.add_space(2.0);
        }

        ui.add_enabled_ui(pending.is_none(), |ui| {
            ui.horizontal(|ui| {
                let b = IconButton::new("circle-dot", RichText::new(tr(state, "OptionPanel.jbPunktNeu.Caption")).font(crate::shell::theme::semibold(14.5)))
                    .primary()
                    .icon_size(18.0)
                    .wrap()
                    .min_size(Vec2::new(half, 48.0));
                // Shortcuts (Space / ⇧+Space) are shown in the tooltips to keep the captions readable.
                let tip = format!("{}\n{}", tr(state, "OptionPanel.jbPunktNeu.ToolTip"), tr(state, "App.Key.Space"));
                if ui.add(b).on_hover_text(tip).clicked() {
                    state.push(Command::Record { delayed: false });
                }
                let b = IconButton::new("rows-3", RichText::new(tr(state, "OptionPanel.jbReiheNeu.Caption")).font(crate::shell::theme::semibold(14.5)))
                    .icon_size(18.0)
                    .icon_color(stoneage_plots::theme::ACCENT)
                    .wrap()
                    .min_size(Vec2::new(half, 48.0));
                let tip = format!("{}\n⇧+Space", tr(state, "OptionPanel.jbReiheNeu.ToolTip"));
                if ui.add(b).on_hover_text(tip).clicked() {
                    state.push(Command::RecordNewRow { delayed: false });
                }
            });
            ui.horizontal(|ui| {
                let b = IconButton::new("timer", tr(state, "OptionPanel.jbPunktNeu1.Caption"))
                    .wrap()
                    .align_left()
                    .min_size(Vec2::new(half, 36.0));
                if ui.add(b).on_hover_text(format!("{}
Ctrl+Space", tr(state, "OptionPanel.jbPunktNeu1.ToolTip"))).clicked() {
                    state.push(Command::Record { delayed: true });
                }
                let b = IconButton::new("timer", tr(state, "OptionPanel.jbReiheNeu1.Caption"))
                    .wrap()
                    .align_left()
                    .min_size(Vec2::new(half, 36.0));
                if ui.add(b).on_hover_text(tr(state, "OptionPanel.jbReiheNeu1.ToolTip")).clicked() {
                    state.push(Command::RecordNewRow { delayed: true });
                }
            });
        });

        let delay = Duration::from_millis(state.config.record_delay_ms.max(1));
        ui.add_space(2.0);
        ui.label(
            RichText::new(trf(state, "App.Measure.DelayHint", &[&format!("{:.1}", delay.as_secs_f32())]))
                .small()
                .weak(),
        );

        if let Some(p) = pending {
            ui.add_space(6.0);
            let left = p.fire_at.saturating_duration_since(Instant::now());
            let frac = 1.0 - (left.as_secs_f32() / delay.as_secs_f32()).clamp(0.0, 1.0);
            let what = if p.new_row { tr(state, "OptionPanel.jbReiheNeu1.Caption") } else { tr(state, "OptionPanel.jbPunktNeu1.Caption") };
            ui.label(RichText::new(what).strong());
            ui.add(
                egui::ProgressBar::new(frac)
                    .text(trf(state, "App.Measure.Countdown", &[&format!("{:.1}", left.as_secs_f32())]))
                    .desired_height(22.0),
            );
            if super::wide_icon_button(ui, "x", format!("{}  (Esc)", tr(state, "App.Common.Cancel")), 30.0).clicked() {
                state.push(Command::CancelDelayedRecord);
            }
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    });
}

fn position_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "crosshair", &tr(state, "App.Measure.Position"), |ui| {
        let rows = state.survey.rows.len();
        let row = state.survey.active_row;
        let (pt, pts) = state.survey.current_row().map(|r| (r.active_point, r.count_points())).unwrap_or((0, 0));
        let row_txt = trf(state, "MainFrame.lbl_Reihe.Caption", &[&(row + 1).min(rows.max(1)), &rows]);
        let pt_txt = trf(state, "MainFrame.lbl_Punkt.Caption", &[&(pt + 1).min(pts.max(1)), &pts]);
        nav_line(ui, state, &row_txt, Command::PrevRow, Command::NextRow, "MainFrame.Menu.Edit.prevRow.Caption", "MainFrame.Menu.Edit.nextRow.Caption");
        nav_line(ui, state, &pt_txt, Command::PrevPoint, Command::NextPoint, "App.Data.PrevPoint", "App.Data.NextPoint");
    });
}

fn nav_line(ui: &mut Ui, state: &mut AppState, text: &str, prev: Command, next: Command, prev_tip: &str, next_tip: &str) {
    ui.horizontal(|ui| {
        // Original captions "<<" / ">>" (jbReiheDec / jbReiheInc) are kept in the tooltips.
        let prev_tip = format!("{}  ({})", tr(state, prev_tip), tr(state, "MainFrame.jbReiheDec.Caption"));
        let next_tip = format!("{}  ({})", tr(state, next_tip), tr(state, "MainFrame.jbReiheInc.Caption"));
        let nav = |icon| IconButton::icon_only(icon, 30.0).kind(super::icons::Kind::Normal).icon_size(15.0);
        if ui.add(nav("chevron-left")).on_hover_text(prev_tip).clicked() {
            state.push(prev);
        }
        let w = ui.available_width() - 30.0 - 2.0 * ui.spacing().item_spacing.x;
        ui.add_sized(Vec2::new(w.max(40.0), 22.0), egui::Label::new(RichText::new(text).font(crate::shell::theme::semibold(13.5))));
        if ui.add(nav("chevron-right")).on_hover_text(next_tip).clicked() {
            state.push(next);
        }
    });
}

fn last_point_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "activity", &tr(state, "App.Measure.LastPoint"), |ui| {
        // OptionPanel$20/$21 operate on MessReihe.getLastPunkt() of the current row.
        let Some(p) = state.survey.current_row().and_then(|r| r.points.last()) else {
            ui.label(RichText::new(tr(state, "App.Measure.NoPoint")).weak());
            return;
        };
        if !p.acquired {
            ui.label(RichText::new(tr(state, "App.Measure.NotRecorded")).weak());
            return;
        }
        let valid = p.is_data_valid();
        info_row(ui, &tr(state, "App.Measure.Time"), super::data::format_time(p.time_stamp));
        let src = if p.spectrum_is_source { "MessPunkt.spectrumIsSource.Caption" } else { "MessPunkt.signalIsSource.Caption" };
        info_row(ui, &tr(state, "App.Measure.Source"), tr(state, src));
        info_row(ui, &tr(state, "OptionPanel.lbl_fRange.Caption"), format!("{} kHz", super::java_double(p.f_range)));
        info_row(ui, &tr(state, "App.Measure.SampleRate"), p.sample_rate.to_string());
        if valid {
            status_dot(ui, ok_color(ui), tr(state, "App.Measure.DataValid"));
        } else {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(err_color(ui).gamma_multiply(0.15))
                .corner_radius(6)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icons::icon(ui, "triangle-alert", 15.0, err_color(ui));
                        ui.add(egui::Label::new(RichText::new(tr(state, "OptionPanel.point_data_not_valid")).color(err_color(ui))).wrap());
                    });
                });
        }
    });
}
