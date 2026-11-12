//! Data section: rows / points navigator, edit actions (insert/delete row, delete point,
//! delete all with confirmation) and min/max of the current row.

use chrono::TimeZone;
use egui::{Button, RichText, Ui, Vec2};

use super::{err_color, icard, info_row, mem, mem_set, ok_color, tr, trf, IconButton};
use crate::state::{AppState, Command};

/// `MessPunkt.timeStamp` (ms since epoch) as local time.
pub(crate) fn format_time(ms: i64) -> String {
    if ms <= 0 {
        return "–".into();
    }
    match chrono::Local.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => t.format("%d.%m.%Y %H:%M:%S").to_string(),
        chrono::LocalResult::None => "–".into(),
    }
}

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    rows_card(ui, state);
    points_card(ui, state);
    edit_card(ui, state);
    minmax_card(ui, state);
}

fn rows_card(ui: &mut Ui, state: &mut AppState) {
    let title = format!("{} ({})", tr(state, "App.Data.Rows"), state.survey.rows.len());
    icard(ui, "rows-3", &title, |ui| {
        let n = state.survey.rows.len();
        if n == 0 {
            ui.label(RichText::new(tr(state, "App.Data.NoRows")).weak());
            return;
        }
        let active = state.survey.active_row;
        let row_word = tr(state, "App.Data.Row");
        let pts_word = tr(state, "App.Data.PointsShort");
        let mut select = None;
        let row_h = 22.0;
        egui::ScrollArea::vertical()
            .id_salt("app-panels-rows")
            .max_height(row_h * 8.0)
            .auto_shrink([false, true])
            .show_rows(ui, row_h, n, |ui, range| {
                for i in range {
                    let count = state.survey.rows[i].count_points();
                    let sel = i == active;
                    let text = RichText::new(format!("{row_word} {}", i + 1));
                    let b = Button::selectable(sel, if sel { text.strong() } else { text })
                        .right_text(RichText::new(format!("{count} {pts_word}")).weak())
                        .min_size(Vec2::new(ui.available_width(), row_h - 2.0));
                    let r = ui.add(b);
                    // keep the active row visible after keyboard navigation
                    if sel && mem::<usize>(ui, "rows_scrolled_to") != active + 1 {
                        r.scroll_to_me(None);
                        mem_set(ui, "rows_scrolled_to", active + 1);
                    }
                    if r.clicked() && !sel {
                        select = Some(i);
                    }
                }
            });
        if let Some(i) = select {
            state.push(Command::SelectRow(i));
        }
    });
}

fn points_card(ui: &mut Ui, state: &mut AppState) {
    let Some(row) = state.survey.current_row() else { return };
    let n = row.count_points();
    let title = format!(
        "{} ({}) – {} {}",
        tr(state, "App.Data.Points"),
        n,
        tr(state, "App.Data.Row"),
        state.survey.active_row + 1
    );
    icard(ui, "chart-column", &title, |ui| {
        let Some(row) = state.survey.current_row() else { return };
        if n == 0 {
            ui.label(RichText::new(tr(state, "App.Data.NoPoints")).weak());
            return;
        }
        let active = row.active_point;
        let pt_word = tr(state, "App.Data.Point");
        let tip_valid = tr(state, "App.Measure.DataValid");
        let tip_invalid = tr(state, "OptionPanel.point_data_not_valid");
        let tip_empty = tr(state, "App.Measure.NotRecorded");
        let (c_ok, c_err, c_weak) = (ok_color(ui), err_color(ui), ui.visuals().weak_text_color());
        let mut select = None;
        let row_h = 22.0;
        egui::ScrollArea::vertical()
            .id_salt("app-panels-points")
            .max_height(row_h * 10.0)
            .auto_shrink([false, true])
            .show_rows(ui, row_h, n, |ui, range| {
                for i in range {
                    let p = &row.points[i];
                    let sel = i == active;
                    let (flag, color, tip) = if !p.acquired {
                        (None, c_weak, &tip_empty)
                    } else if p.is_data_valid() {
                        (Some("check"), c_ok, &tip_valid)
                    } else {
                        (Some("triangle-alert"), c_err, &tip_invalid)
                    };
                    let time = if p.acquired { format_time(p.time_stamp) } else { String::new() };
                    let text = RichText::new(format!("{pt_word} {}", i + 1));
                    let b = Button::selectable(sel, if sel { text.strong() } else { text })
                        .right_text(RichText::new(format!("{time}      ")).color(color).small())
                        .min_size(Vec2::new(ui.available_width(), row_h - 2.0));
                    let r = ui.add(b).on_hover_text(tip.as_str());
                    // Status icon at the right end of the row.
                    let ir = egui::Rect::from_center_size(
                        egui::pos2(r.rect.right() - 13.0, r.rect.center().y),
                        Vec2::splat(13.0),
                    );
                    match flag {
                        Some(icon) => super::icons::paint(ui, icon, ir, color, 1.0),
                        None => {
                            ui.painter().circle_stroke(ir.center(), 4.0, egui::Stroke::new(1.2, color));
                        }
                    }
                    if sel && mem::<usize>(ui, "points_scrolled_to") != active + 1 + 100_000 * state.survey.active_row {
                        r.scroll_to_me(None);
                        mem_set(ui, "points_scrolled_to", active + 1 + 100_000 * state.survey.active_row);
                    }
                    if r.clicked() && !sel {
                        select = Some(i);
                    }
                }
            });
        if let Some(i) = select {
            state.push(Command::SelectPoint(i));
        }
    });
}

#[derive(Clone, Default)]
struct ConfirmDeleteAll(bool);

fn edit_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "sliders-horizontal", &tr(state, "MainFrame.Menu.Edit.Caption"), |ui| {
        let gap = ui.spacing().item_spacing.x;
        let half = ((ui.available_width() - gap) / 2.0).floor();
        let has_row = state.survey.current_row().is_some();
        let has_point = state.survey.current_row().map(|r| r.count_points() > 0).unwrap_or(false);

        ui.horizontal(|ui| {
            if ui
                .add(IconButton::new("plus", tr(state, "MainFrame.Menu.Edit.newRow.Caption")).align_left().min_size(Vec2::new(half, 32.0)))
                .on_hover_text(tr(state, "MainFrame.Menu.Edit.newRow.Caption"))
                .clicked()
            {
                state.push(Command::NewRow);
            }
            if ui
                .add_enabled(has_row, IconButton::new("minus", tr(state, "MainFrame.Menu.Edit.DelRow.Caption")).align_left().min_size(Vec2::new(half, 32.0)))
                .on_hover_text(tr(state, "MainFrame.Menu.Edit.DelRow.Caption"))
                .clicked()
            {
                state.push(Command::DeleteRow);
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(has_point, IconButton::new("minus", tr(state, "MainFrame.Menu.Edit.DelPoint.Caption")).align_left().min_size(Vec2::new(half, 32.0)))
                .on_hover_text(tr(state, "MainFrame.Menu.Edit.DelPoint.Caption"))
                .clicked()
            {
                state.push(Command::DeletePoint);
            }
            let confirm: ConfirmDeleteAll = mem(ui, "confirm_delete_all");
            if ui
                .add_enabled(
                    has_row && !confirm.0,
                    IconButton::new("trash", tr(state, "MainFrame.Menu.Edit.DelAllRows.Caption"))
                        .danger()
                        .align_left()
                        .min_size(Vec2::new(half, 32.0)),
                )
                .on_hover_text(tr(state, "MainFrame.Menu.Edit.DelAllRows.Caption"))
                .clicked()
            {
                mem_set(ui, "confirm_delete_all", ConfirmDeleteAll(true));
            }
        });

        let confirm: ConfirmDeleteAll = mem(ui, "confirm_delete_all");
        if confirm.0 {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(err_color(ui).gamma_multiply(0.15))
                .corner_radius(6)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let rows = state.survey.rows.len();
                    let pts: usize = state.survey.rows.iter().map(|r| r.count_points()).sum();
                    ui.label(RichText::new(trf(state, "App.Data.ConfirmDeleteAll", &[&rows, &pts])).color(err_color(ui)));
                    ui.horizontal(|ui| {
                        if ui.add(IconButton::new("trash", tr(state, "MainFrame.Menu.Edit.DelAllRows.Caption")).danger()).clicked() {
                            state.push(Command::DeleteAllRows);
                            mem_set(ui, "confirm_delete_all", ConfirmDeleteAll(false));
                        }
                        if ui.button(tr(state, "App.Common.Cancel")).clicked() {
                            mem_set(ui, "confirm_delete_all", ConfirmDeleteAll(false));
                        }
                    });
                });
        }
    });
}

fn minmax_card(ui: &mut Ui, state: &mut AppState) {
    icard(ui, "gauge", &tr(state, "App.Data.MinMax"), |ui| {
        match state.survey.current_row() {
            Some(r) => {
                info_row(ui, &tr(state, "App.Data.Minimum"), r.minimum.to_string());
                info_row(ui, &tr(state, "App.Data.Maximum"), r.maximum.to_string());
            }
            None => {
                ui.label(RichText::new(tr(state, "App.Data.NoRows")).weak());
            }
        }
        ui.label(RichText::new(tr(state, "App.Data.MinMax.Hint")).small().weak());
        let gap = ui.spacing().item_spacing.x;
        let half = ((ui.available_width() - gap) / 2.0).floor();
        ui.horizontal(|ui| {
            if ui
                .add(IconButton::new("search", tr(state, "MainFrame.Menu.Edit.findMinMax.Caption")).align_left().min_size(Vec2::new(half, 32.0)))
                .clicked()
            {
                state.push(Command::FindMinMax);
            }
            if ui
                .add(IconButton::new("refresh-cw", tr(state, "MainFrame.Menu.Edit.resetMinMax.Caption")).align_left().min_size(Vec2::new(half, 32.0)))
                .clicked()
            {
                state.push(Command::ResetMinMax);
            }
        });
    });
}
