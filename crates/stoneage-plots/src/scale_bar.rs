//! Colour legend: port of `ScalePanel` and `ScalePanel_jsldColorMap_changeAdapter`.
//!
//! * the legend shows the 256 colours of the current map (index 255 on top) with a black frame
//!   and percentage labels every 32 entries (white shadow + black text);
//! * pressing + dragging vertically shifts `colorModelBase` (`mouseDragged`);
//! * a click toggles `reverseColor` (`mouseClicked`);
//! * a vertical slider (1..=numberOfMaps) selects the colour model, tooltip
//!   `ScalePanel.jsldColorMap.ToolTip` with the model name.

use egui::{pos2, vec2, Align2, Color32, ColorImage, Rect, Sense, Stroke};
use stoneage_core::{colormap, I18n, Survey};

use crate::row_view::{format_msg, raster, tr_or};
use crate::{theme, ViewResponse};

/// `ScalePanel.basex`
pub const BASE_X: i32 = 5;
/// `ScalePanel.basey`
pub const BASE_Y: i32 = 5;
/// Height of the colour ramp (`paintComponent` local `256`).
pub const RAMP_HEIGHT: i32 = 256;
/// Width of the colour ramp.
pub const RAMP_WIDTH: i32 = 40;
/// Label spacing in colour entries (`paintComponent` local `32`).
pub const LABEL_STEP: i32 = 32;
/// `getPreferredSize()` = 50 × 266.
pub const WIDTH: i32 = 50;
pub const HEIGHT: i32 = 266;
/// Preferred size of the colour-model slider (29 × 100).
pub const SLIDER_LENGTH: f32 = 100.0;

/// `Math.round(float)` of Java.
fn java_round_f32(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

/// `mouseDragged`: new `colorModelBase` for mouse position `y` (component coordinates).
/// Values below 0 become 0, values above 256 wrap to 0 (as in the original).
pub fn drag_base(y: i32, drag_start: i32) -> i32 {
    let d = y - drag_start;
    if d < 0 || d > 256 {
        0
    } else {
        d
    }
}

/// `mousePressed`: drag anchor for mouse position `y`.
pub fn drag_anchor(y: i32, color_model_base: i32) -> i32 {
    y - color_model_base
}

/// Colour index painted in ramp row `k` (0 = top): the original paints from `mapSize-1` down.
pub fn ramp_index(k: i32, map_size: i32) -> i32 {
    map_size - 1 - k
}

/// Percentage labels: `(baseline y, percent)` for every `i % 32 == 0`, `i in 0..=256`.
pub fn legend_labels() -> Vec<(i32, i32)> {
    (0..=RAMP_HEIGHT)
        .filter(|i| i % LABEL_STEP == 0)
        .map(|i| {
            let y = 5 + BASE_Y + RAMP_HEIGHT - i;
            let pct = java_round_f32(i as f32 / RAMP_HEIGHT as f32 * 100.0);
            (y, pct)
        })
        .collect()
}

/// Percentage at component row `y` (same scale as the labels), clamped to 0..=100.
pub fn percent_at(y: i32) -> i32 {
    let i = (5 + BASE_Y + RAMP_HEIGHT - y).clamp(0, RAMP_HEIGHT);
    java_round_f32(i as f32 / RAMP_HEIGHT as f32 * 100.0)
}

/// `loadValues`: slider value for colour model `color_model`.
pub fn slider_from_model(color_model: i32) -> i32 {
    color_model + 1
}

/// `setValues`: colour model for slider value `v` (`None` while `doNotSetValues`).
pub fn model_from_slider(v: i32, do_not_set_values: bool) -> Option<i32> {
    if do_not_set_values {
        None
    } else {
        Some(v - 1)
    }
}

#[derive(Default)]
pub struct ScaleBar {
    /// `dragStart`
    drag_start: i32,
    /// `doNotSetValues`
    do_not_set_values: bool,
    /// `jsldColorMap.getValue()` (1-based colour model).
    slider_value: i32,
}

impl ScaleBar {
    /// `loadValues()`: slider ← `colorModel + 1`.
    pub fn load_values(&mut self, survey: &Survey) {
        self.do_not_set_values = true;
        self.slider_value = slider_from_model(survey.color_model);
        self.do_not_set_values = false;
    }

    /// `setValues()`: `colorModel` ← slider − 1. Returns true if applied.
    pub fn set_values(&mut self, survey: &mut Survey) -> bool {
        match model_from_slider(self.slider_value, self.do_not_set_values) {
            Some(m) => {
                survey.color_model = m;
                true
            }
            None => false,
        }
    }

    /// Slider tooltip (`ScalePanel.jsldColorMap.ToolTip`, `{0}` = model name).
    pub fn tooltip(i18n: &I18n, slider_value: i32) -> String {
        let name = colormap::name((slider_value - 1).max(0) as usize);
        let pattern = tr_or(i18n, "ScalePanel.jsldColorMap.ToolTip", "Colormodel = {0}");
        format_msg(&pattern, &[name.to_owned()])
    }

    /// Vertical colour legend; drag shifts `survey.color_model_base`, slider picks `survey.color_model`.
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &mut Survey, i18n: &I18n) -> ViewResponse {
        let mut resp = ViewResponse::default();
        ui.vertical(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(vec2(WIDTH as f32, HEIGHT as f32), Sense::click_and_drag());
            paint_legend(ui.painter(), rect, survey);

            let local_y = |pos: egui::Pos2| (pos.y - rect.top()).floor() as i32;
            // mousePressed
            if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                if let Some(pos) = response.interact_pointer_pos() {
                    self.drag_start = drag_anchor(local_y(pos), survey.color_model_base);
                }
            }
            // mouseDragged
            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let b = drag_base(local_y(pos), self.drag_start);
                    if b != survey.color_model_base {
                        survey.color_model_base = b;
                        resp.changed = true;
                    }
                }
            }
            // mouseClicked
            if response.clicked() {
                survey.reverse_color = !survey.reverse_color;
                resp.changed = true;
            }
            if let Some(pos) = response.hover_pos() {
                let pattern = tr_or(i18n, "App.Scale.HoverInfo", "Colour scale {0} %");
                resp.hover_info = Some(format_msg(&pattern, &[percent_at(local_y(pos)).to_string()]));
            }
            response.on_hover_text(tr_or(
                i18n,
                "App.Scale.Hint",
                "Drag to shift the colour scale, click to reverse the colours",
            ));

            // jsldColorMap
            self.load_values(survey);
            let n = colormap::count().max(1) as i32;
            let mut v = self.slider_value.clamp(1, n);
            let tip = Self::tooltip(i18n, v);
            let r = ui
                .scope(|ui| {
                    ui.spacing_mut().slider_width = SLIDER_LENGTH;
                    ui.add(egui::Slider::new(&mut v, 1..=n).vertical().show_value(false).step_by(1.0))
                })
                .inner
                .on_hover_text(tip);
            if r.changed() {
                self.slider_value = v;
                if self.set_values(survey) {
                    resp.changed = true;
                }
            }
        });
        resp
    }
}

/// Paint the legend into `rect` (component origin = `rect.min`).
pub fn paint_legend(painter: &egui::Painter, rect: Rect, survey: &Survey) {
    let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
    let map_size = lut.len() as i32;
    let step = (RAMP_HEIGHT / map_size).max(1);
    let o = rect.min;
    let mut y = BASE_Y;
    for k in 0..map_size {
        let c = theme::rgba(lut[ramp_index(k, map_size) as usize]);
        painter.rect_filled(
            Rect::from_min_size(pos2(o.x + BASE_X as f32, o.y + y as f32), vec2(RAMP_WIDTH as f32, step as f32)),
            0.0,
            c,
        );
        y += step;
    }
    painter.rect_stroke(
        Rect::from_min_size(
            pos2(o.x + BASE_X as f32, o.y + BASE_Y as f32),
            vec2(RAMP_WIDTH as f32, RAMP_HEIGHT as f32),
        ),
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        egui::StrokeKind::Outside,
    );
    for (ly, pct) in legend_labels() {
        theme::shadow_text(
            painter,
            pos2(o.x + (BASE_X + 10) as f32, o.y + ly as f32 + 2.0),
            Align2::LEFT_BOTTOM,
            &format!("{pct}%"),
            theme::small_font(),
            Color32::BLACK,
            Color32::WHITE,
        );
    }
}

/// The legend as printed (`printImage(scale)`): 50 × 266 on white.
pub(crate) fn render_legend(survey: &Survey, text: &mut raster::TextRaster) -> ColorImage {
    let mut img = ColorImage::filled([WIDTH as usize, HEIGHT as usize], theme::PAGE_BG);
    let clip = raster::full_clip(&img);
    let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
    let map_size = lut.len() as i32;
    let step = (RAMP_HEIGHT / map_size).max(1);
    let mut y = BASE_Y;
    for k in 0..map_size {
        let c = theme::rgba(lut[ramp_index(k, map_size) as usize]);
        raster::fill_rect(&mut img, clip, BASE_X, y, RAMP_WIDTH, step, c);
        y += step;
    }
    raster::draw_rect(&mut img, clip, BASE_X, BASE_Y, RAMP_WIDTH, RAMP_HEIGHT, 1, Color32::BLACK);
    for (ly, pct) in legend_labels() {
        let s = format!("{pct}%");
        text.draw(&mut img, clip, &s, (BASE_X + 11) as f32, (ly + 1) as f32, 12.0, Color32::WHITE, false);
        text.draw(&mut img, clip, &s, (BASE_X + 10) as f32, ly as f32, 12.0, Color32::BLACK, false);
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_rules() {
        assert_eq!(drag_anchor(100, 20), 80);
        assert_eq!(drag_base(130, 80), 50);
        assert_eq!(drag_base(70, 80), 0);
        assert_eq!(drag_base(80 + 256, 80), 256);
        assert_eq!(drag_base(80 + 257, 80), 0);
    }

    #[test]
    fn labels() {
        let l = legend_labels();
        assert_eq!(l.len(), 9);
        assert_eq!(l[0], (266, 0));
        assert_eq!(l[1], (234, 13));
        assert_eq!(l[4], (138, 50));
        assert_eq!(l[8], (10, 100));
        assert_eq!(percent_at(266), 0);
        assert_eq!(percent_at(10), 100);
        assert_eq!(percent_at(0), 100);
    }

    #[test]
    fn ramp_order() {
        assert_eq!(ramp_index(0, 256), 255);
        assert_eq!(ramp_index(255, 256), 0);
    }

    #[test]
    fn slider_values() {
        assert_eq!(slider_from_model(2), 3);
        assert_eq!(model_from_slider(5, false), Some(4));
        assert_eq!(model_from_slider(1, true), None);
    }
}
