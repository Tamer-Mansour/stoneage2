//! Display section: zoom sliders, block size, horizontal cut with depth labels, check boxes,
//! valid base/step (only if `showValidBase`), colour model selector and reverse colours.
//! Every change calls `setValues()` like the original listeners `OptionPanel$2..$4, $12..$19`.

use egui::{Align2, Color32, FontId, RichText, Sense, Slider, Ui, Vec2};

use super::option_panel::{
    apply_now, BLOCK_SIZE_RANGE, HCUT_LABEL_STEP, HCUT_RANGE, VALID_STEP_RANGE, ZOOM_X_RANGE, ZOOM_Y_RANGE,
    ZOOM_Z_RANGE,
};
use super::{form_row, icard, java_double, tr, trf, LABEL_W};
use crate::state::AppState;

fn slider_row(ui: &mut Ui, label: &str, tooltip: &str, value: &mut i32, range: (i32, i32)) -> bool {
    form_row(ui, label, tooltip, |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(60.0);
        ui.add(Slider::new(value, range.0..=range.1).integer()).on_hover_text(tooltip).changed()
    })
}

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    let mut changed = false;

    // --- zoom ------------------------------------------------------------------------------
    icard(ui, "search", &tr(state, "App.Display.Zoom"), |ui| {
        let mut v = state.survey.disp_step_x;
        let tip = trf(state, "OptionPanel.jsldZoomX.ToolTip", &[&v]);
        if slider_row(ui, &tr(state, "App.Display.ZoomX"), &tip, &mut v, ZOOM_X_RANGE) {
            state.survey.disp_step_x = v;
            changed = true;
        }
        let mut v = state.survey.disp_step_y;
        let tip = trf(state, "OptionPanel.jsldZoomY.ToolTip", &[&v]);
        if slider_row(ui, &tr(state, "App.Display.ZoomY"), &tip, &mut v, ZOOM_Y_RANGE) {
            state.survey.disp_step_y = v;
            changed = true;
        }
        let mut v = state.survey.disp_step_z;
        let tip = trf(state, "OptionPanel.jsldZoomZ.ToolTip", &[&v]);
        if slider_row(ui, &tr(state, "App.Display.ZoomZ"), &tip, &mut v, ZOOM_Z_RANGE) {
            state.survey.disp_step_z = v;
            changed = true;
        }
    });

    // --- processing ------------------------------------------------------------------------
    icard(ui, "sliders-horizontal", &tr(state, "App.Display.Processing"), |ui| {
        let mut v = state.survey.block_size;
        let tip = trf(state, "OptionPanel.jsldBlockSize.ToolTip", &[&v]);
        if slider_row(ui, &tr(state, "App.Display.BlockSize"), &tip, &mut v, BLOCK_SIZE_RANGE) {
            state.survey.block_size = v;
            changed = true;
        }
        ui.add_space(2.0);
        let c_avg = tr(state, "OptionPanel.jcbUseBlockAverage.Caption");
        let c_int = tr(state, "OptionPanel.jcbInterpolation.Caption");
        let c_grid = tr(state, "OptionPanel.jcb_ZoomGrid.Caption");
        let c_depth = tr(state, "OptionPanel.jcbShowTiefe.Caption");
        let t_depth = tr(state, "OptionPanel.jcbShowTiefe.ToolTip");
        let s = &mut state.survey;
        changed |= ui.checkbox(&mut s.use_block_average, c_avg).changed(); // $4
        changed |= ui.checkbox(&mut s.interpolate, c_int).changed(); // $2
        changed |= ui.checkbox(&mut s.show_grid, c_grid).changed(); // $3
        changed |= ui.checkbox(&mut s.show_depth, c_depth).on_hover_text(t_depth).changed(); // $15
    });

    // --- horizontal cut ---------------------------------------------------------------------
    icard(ui, "layers", &tr(state, "App.Display.HorizontalCut"), |ui| {
        let mut v = state.survey.horizontal_cut;
        let depth = state.survey.depth(v.max(0) as usize);
        let tip = trf(state, "OptionPanel.sldHorizontalSchnitt.ToolTip", &[&java_double(depth)]);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(state, "App.Display.Sample")).weak());
            ui.label(RichText::new(format!("{v}")).monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{} m", java_double(depth))).strong());
            });
        });
        ui.spacing_mut().slider_width = ui.available_width() - 4.0;
        let r = ui
            .add(Slider::new(&mut v, HCUT_RANGE.0..=HCUT_RANGE.1).integer().show_value(false))
            .on_hover_text(&tip);
        if r.changed() {
            state.survey.horizontal_cut = v;
            changed = true;
        }
        // tiefenLabels: depth at every 512th sample (updateDynText: tiefe(i) + " m")
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        let font = FontId::proportional(10.5);
        let color = ui.visuals().weak_text_color();
        let pad = 8.0;
        let n = (HCUT_RANGE.1 / HCUT_LABEL_STEP) as usize;
        for k in 0..=n {
            let i = k as i32 * HCUT_LABEL_STEP;
            let frac = k as f32 / n as f32;
            let x = rect.left() + pad + frac * (rect.width() - 2.0 * pad);
            let anchor = if k == 0 {
                Align2::LEFT_TOP
            } else if k == n {
                Align2::RIGHT_TOP
            } else {
                Align2::CENTER_TOP
            };
            let x = if k == 0 { rect.left() } else if k == n { rect.right() } else { x };
            let label = format!("{} m", java_double(state.survey.depth(i as usize)));
            ui.painter().text(egui::pos2(x, rect.top()), anchor, label, font.clone(), color);
        }
    });

    // --- valid base / step (OptionPanel.showValidBase) -----------------------------------------
    if state.config.show_valid_base {
        icard(ui, "list-checks", &tr(state, "App.Display.ValidPoints"), |ui| {
            let (mut base, mut step) =
                state.survey.current_row().map(|r| (r.valid_base, r.valid_step)).unwrap_or((0, 1));
            let tip = trf(state, "OptionPanel.jsldValidStep.ToolTip", &[&step]);
            let step_changed = slider_row(ui, &tr(state, "App.Display.ValidStep"), &tip, &mut step, VALID_STEP_RANGE);
            let tip = trf(state, "OptionPanel.jsldValidBase.ToolTip", &[&base]);
            let base_changed = slider_row(ui, &tr(state, "App.Display.ValidBase"), &tip, &mut base, (0, (step - 1).max(0)));
            if step_changed || base_changed {
                if let Some(r) = state.survey.current_row_mut() {
                    r.valid_step = step;
                    r.valid_base = base.min(step - 1).max(0);
                }
                changed = true;
            }
        });
    }

    if changed {
        apply_now(ui, state);
    }

    // --- colours (ScalePanel) -----------------------------------------------------------------
    icard(ui, "palette", &tr(state, "App.Display.Colors"), |ui| {
        let n = stoneage_core::colormap::count();
        let reverse = state.survey.reverse_color;
        let base = state.survey.color_model_base;
        let cur = state.survey.color_model;
        let bar_w = (ui.available_width() - LABEL_W).max(60.0);
        for i in 0..n {
            let selected = cur == i as i32;
            let tip = trf(state, "ScalePanel.jsldColorMap.ToolTip", &[&i]);
            let clicked = ui
                .horizontal(|ui| {
                    let name_resp = ui
                        .add_sized(
                            Vec2::new(LABEL_W - 8.0, 18.0),
                            egui::Button::selectable(selected, stoneage_core::colormap::name(i)).truncate(),
                        )
                        .on_hover_text(&tip);
                    let (rect, bar_resp) = ui.allocate_exact_size(Vec2::new(bar_w, 16.0), Sense::click());
                    paint_gradient(ui, rect, &stoneage_core::colormap::lut(i, reverse, base), selected);
                    let bar_resp = bar_resp.on_hover_text(&tip).on_hover_cursor(egui::CursorIcon::PointingHand);
                    name_resp.clicked() || bar_resp.clicked()
                })
                .inner;
            if clicked && !selected {
                state.survey.color_model = i as i32;
                state.dirty = true;
            }
        }
        ui.add_space(4.0);
        let mut rev = state.survey.reverse_color;
        if ui.checkbox(&mut rev, tr(state, "App.Display.ReverseColors")).changed() {
            state.survey.reverse_color = rev;
            state.dirty = true;
        }
        if base != 0 {
            ui.horizontal(|ui| {
                ui.label(RichText::new(trf(state, "App.Display.ColorBase", &[&base])).weak());
                if ui.small_button(tr(state, "App.Display.ColorBaseReset")).clicked() {
                    state.survey.color_model_base = 0;
                    state.dirty = true;
                }
            });
        }
    });
}

fn paint_gradient(ui: &Ui, rect: egui::Rect, lut: &[[u8; 4]; 256], selected: bool) {
    let p = ui.painter();
    let steps = 64usize;
    let w = rect.width() / steps as f32;
    for s in 0..steps {
        let c = lut[s * 256 / steps];
        let r = egui::Rect::from_min_size(
            egui::pos2(rect.left() + s as f32 * w, rect.top()),
            Vec2::new(w + 0.5, rect.height()),
        );
        p.rect_filled(r, 0.0, Color32::from_rgb(c[0], c[1], c[2]));
    }
    let stroke = if selected {
        egui::Stroke::new(2.0, ui.visuals().selection.stroke.color)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    p.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);
}
