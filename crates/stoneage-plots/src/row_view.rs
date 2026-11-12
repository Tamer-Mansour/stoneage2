//! Row view: port of `MessReihe` (display parts), `MessReihe$ViewArea`, `MessReihe$ColHeader`
//! and `MessReihe$RowHeader`.
//!
//! The active row is shown as a colour-coded section: X = measurement points
//! (`disp_step_x` px per point), Y = samples (`disp_step_y` px per sample block), colour = value
//! through the colour map scaled by the row minimum/maximum. All pixel math is in pure functions
//! (see the "pixel math" section) that reproduce the original bytecode exactly.

use std::hash::Hasher;

use egui::{pos2, vec2, Align2, Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, TextureOptions};
use stoneage_core::{colormap, I18n, Row, Survey};

use crate::{theme, ViewResponse};

/// `MessPunkt.numSamples`.
pub const SAMPLES: i32 = 2048;
/// `MessReihe$ColHeader.maxy` – height of the column header.
pub const COL_HEADER_HEIGHT: i32 = 25;
/// `MessReihe$RowHeader.sizex` – width of the row header.
pub const ROW_HEADER_WIDTH: i32 = 70;

// ------------------------------------------------------------------------------------------
// Strings
// ------------------------------------------------------------------------------------------

/// `i18n.tr(key)`, or `fallback` while the key is not (yet) present in the property files.
pub(crate) fn tr_or(i18n: &I18n, key: &str, fallback: &str) -> String {
    let s = i18n.tr(key);
    if s == key || s.is_empty() {
        fallback.to_owned()
    } else {
        s
    }
}

/// `MessageFormat`-like `{0}`, `{1}`, … substitution.
pub fn format_msg(pattern: &str, args: &[String]) -> String {
    let mut out = pattern.to_owned();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

/// `String.valueOf(double)` / `StringBuilder.append(double)` of Java.
pub fn java_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = v.abs();
    if (1e-3..1e7).contains(&a) {
        let s = format!("{v}");
        if s.contains('.') {
            s
        } else {
            format!("{s}.0")
        }
    } else {
        // scientific: Java prints "1.0E-5", "1.5E7"
        let s = format!("{v:e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') { m.to_owned() } else { format!("{m}.0") };
        format!("{m}E{e}")
    }
}

// ------------------------------------------------------------------------------------------
// Pixel math (pure, testable ports of the original bytecode)
// ------------------------------------------------------------------------------------------

/// `Math.round(double)` of Java (`floor(x + 0.5)`, saturating).
pub fn java_round(x: f64) -> i64 {
    if x.is_nan() {
        0
    } else {
        (x + 0.5).floor() as i64
    }
}

/// The display parameters used by the row section (snapshot of `MessAreal` + `MessReihe`
/// fields). All steps are sanitised to be >= 1 (the original would throw on 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SectionParams {
    pub disp_step_x: i32,
    pub disp_step_y: i32,
    pub block_size: i32,
    pub use_block_average: bool,
    pub interpolate: bool,
    pub valid_base: i32,
    pub valid_step: i32,
}

impl SectionParams {
    pub fn new(survey: &Survey, row: &Row) -> Self {
        Self {
            disp_step_x: survey.disp_step_x.max(1),
            disp_step_y: survey.disp_step_y.max(1),
            block_size: survey.block_size.max(1),
            use_block_average: survey.use_block_average,
            interpolate: survey.interpolate,
            valid_base: row.valid_base,
            valid_step: row.valid_step.max(1),
        }
    }
}

/// `MessPunkt.getValue(int)` including block handling (`blockSize`, `useBlockAverage`).
///
/// * block size 1: `values[idx]`
/// * otherwise the block starting at `round(idx / bs) * bs` is averaged (`sum / bs`, integer
///   division) or its maximum (starting at 0) is taken.
/// * any out-of-range access yields `values[2047]` if `idx >= 2048`, else `values[0]`.
pub fn block_value(values: &[i64], idx: i32, block_size: i32, use_block_average: bool) -> i64 {
    let oob = || -> i64 {
        if idx >= SAMPLES {
            values.get((SAMPLES - 1) as usize).copied().or(values.last().copied()).unwrap_or(0)
        } else {
            values.first().copied().unwrap_or(0)
        }
    };
    let bs = block_size.max(1) as i64;
    if bs == 1 {
        return if idx >= 0 { values.get(idx as usize).copied().unwrap_or_else(oob) } else { oob() };
    }
    let start = (java_round(idx as f64 / bs as f64) * bs) as i32 as i64;
    let mut sum: i64 = 0;
    let mut max: i64 = 0;
    let mut i = start;
    while i < start + bs {
        if i < 0 || i as usize >= values.len() {
            return oob();
        }
        let v = values[i as usize];
        sum = sum.wrapping_add(v);
        max = max.max(v);
        i += 1;
    }
    if use_block_average {
        sum / bs
    } else {
        max
    }
}

/// `MessReihe$ViewArea.getPreferredSize()` – size of the section image in pixels.
pub fn preferred_size(n_points: usize, p: &SectionParams) -> (i32, i32) {
    let h = SAMPLES / p.block_size * p.disp_step_y;
    let mut w = (n_points as i32 - 1) * p.disp_step_x;
    if w == 0 {
        w = p.disp_step_x;
    }
    (w.max(0), h)
}

/// `MessReihe$ColHeader.getPreferredSize().width`.
pub fn col_header_width(n_points: usize, disp_step_x: i32) -> i32 {
    disp_step_x * n_points as i32 + 50
}

/// `MessReihe$RowHeader.getPreferredSize().height`.
pub fn row_header_height(p: &SectionParams) -> i32 {
    SAMPLES / p.block_size * p.disp_step_y + 10
}

/// `MessReihe.getXPos(int)` – horizontal position [m] of pixel column `px` (2 decimals).
pub fn x_pos_for_pixel(px: i32, disp_step_x: i32, point_distance: f64) -> f64 {
    let d = px as f64 / disp_step_x.max(1) as f64;
    java_round(d * point_distance * 100.0) as f64 / 100.0
}

/// Last valid point index of a row (`((n-1-vb)/vs)*vs+vb`).
fn last_valid_index(n_points: usize, p: &SectionParams) -> i32 {
    ((n_points as i32 - 1 - p.valid_base) / p.valid_step) * p.valid_step + p.valid_base
}

/// `MessReihe.___getPointIndexForPixel(int)`.
pub fn point_index_for_pixel(x: i32, n_points: usize, p: &SectionParams) -> i32 {
    let mut idx;
    if p.interpolate {
        idx = (x as f64 / p.disp_step_x as f64).floor() as i32;
        while idx > 0 && (idx - p.valid_base) % p.valid_step != 0 {
            idx -= 1;
        }
    } else {
        let d = x as f64 / p.disp_step_x as f64;
        let d = (d - p.valid_base as f64) / p.valid_step as f64;
        idx = p.valid_base + p.valid_step * java_round(d) as i32;
    }
    let last = last_valid_index(n_points, p);
    if idx < p.valid_base {
        idx = p.valid_base;
    } else if idx > last {
        idx = last;
    }
    idx
}

/// Sample index shown at pixel row `y` (as used by `___getValueForPixel`, clamped 0..=2047).
pub fn sample_index_for_pixel(y: i32, p: &SectionParams) -> i32 {
    let d = y as f64 / p.disp_step_y as f64;
    let s = if p.interpolate { d.floor() as i32 } else { java_round(d) as i32 } * p.block_size;
    s.clamp(0, SAMPLES - 1)
}

fn get_point<'a>(points: &[&'a [i64]], idx: i32) -> Option<&'a [i64]> {
    if idx >= 0 {
        points.get(idx as usize).copied()
    } else {
        None
    }
}

/// `MessReihe.___getValueForPixel(int, int)` – the (interpolated) value under the mouse.
/// Returns `None` where the original would dereference a missing point.
pub fn value_for_pixel(points: &[&[i64]], p: &SectionParams, x: i32, y: i32) -> Option<i64> {
    let n = points.len();
    let i5 = point_index_for_pixel(x, n, p);
    let s6 = sample_index_for_pixel(y, p);
    let bs = p.block_size;
    let bv = |pi: i32, s: i32| -> Option<i64> {
        get_point(points, pi).map(|v| block_value(v, s, bs, p.use_block_average))
    };
    let mut v3 = bv(i5, s6)?;
    if p.interpolate {
        let mut i8 = i5 + p.valid_step;
        if i8 > n as i32 - 1 {
            i8 = i5;
        }
        let mut s9 = s6 + bs;
        if s9 > SAMPLES - bs {
            s9 = s6;
        }
        let dx = (i8 - i5) * p.disp_step_x;
        let v12 = bv(i5, s9)?;
        let v10 = bv(i8, s6)?;
        let v14 = bv(i8, s9)?;
        let d25 = (v10 - v3) as f64;
        let d27 = (v14 - v12) as f64;
        let (a, b) = if dx != 0 {
            let fx = (x % dx) as f64;
            (v3 as f64 + fx * d25 / dx as f64, v12 as f64 + fx * d27 / dx as f64)
        } else {
            (v3 as f64, v12 as f64)
        };
        let fy = (y % p.disp_step_y) as f64;
        v3 = java_round(a + fy * (b - a) / p.disp_step_y as f64);
    }
    Some(v3)
}

/// Raw value of one section pixel as computed in `MessReihe$ViewArea.paintComponent`
/// (before min/max scaling). 0 = no data; real zeros are shown as 1.
pub fn section_raw_value(points: &[&[i64]], p: &SectionParams, x: i32, y: i32) -> i64 {
    let dsx = p.disp_step_x;
    let dsy = p.disp_step_y;
    let vb = p.valid_base;
    let vs = p.valid_step;
    let bs = p.block_size;
    let avg = p.use_block_average;
    let step_x = (vs * dsx) as i64;
    let step_y = dsy as i64;
    if p.interpolate {
        let s0 = (y as f64 / dsy as f64).floor() as i32 * bs;
        let s1 = s0 + bs;
        let rel = (x as f64 - vb as f64 * dsx as f64) / dsx as f64 / vs as f64;
        let p0 = rel.floor() as i32 * vs + vb;
        match (get_point(points, p0), get_point(points, p0 + vs)) {
            (Some(a), Some(b)) if s1 < SAMPLES => {
                let v00 = block_value(a, s0, bs, avg);
                let v01 = block_value(a, s1, bs, avg);
                let v10 = block_value(b, s0, bs, avg);
                let v11 = block_value(b, s1, bs, avg);
                let wx = ((x - vb * dsx) as i64) % step_x;
                let wy = (y as i64) % step_y;
                let top = v00 as f64 + wx as f64 * (v10 - v00) as f64 / step_x as f64;
                let bottom = v01 as f64 + wx as f64 * (v11 - v01) as f64 / step_x as f64;
                let v = java_round(top + wy as f64 * (bottom - top) / step_y as f64);
                if v == 0 {
                    1
                } else {
                    v
                }
            }
            _ => 0,
        }
    } else {
        let s = java_round(y as f64 / dsy as f64) as i32 * bs;
        let rel = (x as f64 - vb as f64 * dsx as f64) / dsx as f64 / vs as f64;
        let pi = java_round(rel) as i32 * vs + vb;
        match get_point(points, pi) {
            Some(v) => {
                let v = block_value(v, s, bs, avg);
                if v == 0 {
                    1
                } else {
                    v
                }
            }
            None => 0,
        }
    }
}

/// Min/max scaling of `paintComponent`. Returns `None` if the scaled value leaves 0..=255
/// (the original then calls `resetMinMax()` and continues unscaled).
pub fn scale_value(v: i64, minimum: i64, maximum: i64) -> Option<i64> {
    if minimum != maximum && !(minimum == 0 && maximum == 255) {
        let s = java_round((v - minimum) as f64 * 255.0 / (maximum - minimum) as f64);
        if !(0..=255).contains(&s) {
            return None;
        }
        Some(s)
    } else {
        Some(v)
    }
}

/// Colour-map indices for the content rectangle `(x0, y0, w, h)` (iteration order x-outer,
/// y-inner like the original). Pixels outside the preferred size stay 0.
/// Returns the indices (row-major, `w*h`) and whether `resetMinMax()` was triggered.
pub fn render_indices(
    points: &[&[i64]],
    p: &SectionParams,
    minimum: i64,
    maximum: i64,
    rect: [i32; 4],
) -> (Vec<u8>, bool) {
    let [x0, y0, w, h] = rect;
    let (w, h) = (w.max(0), h.max(0));
    let mut out = vec![0u8; (w as usize) * (h as usize)];
    let (maxx, maxy) = preferred_size(points.len(), p);
    let (mut mn, mut mx) = (minimum, maximum);
    let mut reset = false;
    for x in x0..x0 + w {
        if x >= maxx {
            break;
        }
        for y in y0..y0 + h {
            if y >= maxy {
                break;
            }
            let raw = section_raw_value(points, p, x, y);
            let v = match scale_value(raw, mn, mx) {
                Some(v) => v,
                None => {
                    mn = 0;
                    mx = 255;
                    reset = true;
                    scale_value(raw, mn, mx).unwrap_or(raw)
                }
            };
            out[((y - y0) * w + (x - x0)) as usize] = v.clamp(0, 255) as u8;
        }
    }
    (out, reset)
}

/// x positions of the vertical grid lines for content columns `x0..x1` (`showGrid`).
pub fn grid_x_lines(x0: i32, x1: i32, p: &SectionParams) -> Vec<i32> {
    let dsx = p.disp_step_x;
    let step = dsx * p.valid_step;
    let mut v: Vec<i32> = Vec::new();
    let mut x = x0;
    while x < x1 {
        let lx = p.valid_base * dsx + (x / step) * step;
        if v.last() != Some(&lx) {
            v.push(lx);
        }
        x += dsx;
    }
    v
}

/// y positions of the horizontal grid lines for content rows `y0..y1`.
pub fn grid_y_lines(y0: i32, y1: i32, disp_step_y: i32) -> Vec<i32> {
    let dsy = disp_step_y.max(1);
    let mut v: Vec<i32> = Vec::new();
    let mut y = y0;
    while y < y1 {
        let ly = y / dsy * dsy;
        if v.last() != Some(&ly) {
            v.push(ly);
        }
        y += dsy;
    }
    v
}

/// Whether the grid is drawn (`showGrid && disp_stepx > 2 && disp_stepy > 2`).
pub fn grid_visible(show_grid: bool, p: &SectionParams) -> bool {
    show_grid && p.disp_step_x > 2 && p.disp_step_y > 2
}

/// One tick of the column header (`MessReihe$ColHeader.paintComponent`).
#[derive(Clone, Debug, PartialEq)]
pub struct ColTick {
    pub index: usize,
    /// Pixel position (`i * disp_step_x`).
    pub x: i32,
    /// `(i - validBase) % validStep == 0` → black tick, else light gray.
    pub valid: bool,
    pub active: bool,
    /// Major tick with a label (`getXPos(x)` metres), every > 50 px.
    pub labeled: bool,
}

pub fn col_header_ticks(n_points: usize, active: usize, p: &SectionParams) -> Vec<ColTick> {
    let mut last = -100;
    (0..n_points)
        .map(|i| {
            let x = i as i32 * p.disp_step_x;
            let labeled = x > last + 50;
            if labeled {
                last = x;
            }
            ColTick {
                index: i,
                x,
                valid: (i as i32 - p.valid_base) % p.valid_step == 0,
                active: i == active,
                labeled,
            }
        })
        .collect()
}

/// One tick of the row header (`MessReihe$RowHeader.paintComponent`).
#[derive(Clone, Debug, PartialEq)]
pub struct RowTick {
    /// Sample index.
    pub sample: i32,
    /// Pixel position `basey - dsy/2 + (i/bs)*dsy`.
    pub y: i32,
    /// Major tick with label (every > 27 px and the last sample).
    pub labeled: bool,
}

pub fn row_header_ticks(p: &SectionParams, base_y: i32) -> Vec<RowTick> {
    let mut last = -100;
    let mut v = Vec::with_capacity((SAMPLES / p.block_size) as usize + 1);
    let mut i = 0;
    while i < SAMPLES {
        let y = base_y - p.disp_step_y / 2 + i / p.block_size * p.disp_step_y;
        let labeled = y > last + 27 || i == SAMPLES - 1;
        if labeled {
            last = y;
        }
        v.push(RowTick { sample: i, y, labeled });
        i += p.block_size;
    }
    v
}

/// Pixel position of the horizontal-cut marker in the row header.
pub fn cut_marker_y(p: &SectionParams, base_y: i32, horizontal_cut: i32) -> i32 {
    base_y - p.disp_step_y / 2 + horizontal_cut / p.block_size * p.disp_step_y
}

/// Cheap fingerprint of the row data (replaces the original's unconditional repaint timer).
fn data_fingerprint(row: &Row) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write_usize(row.points.len());
    for pt in &row.points {
        h.write_u8(pt.acquired as u8);
        h.write_usize(pt.values.len());
        for v in &pt.values {
            h.write_i64(*v);
        }
    }
    h.finish()
}

fn y_label(survey: &Survey, i: usize) -> String {
    if survey.show_depth {
        format!("{} m", java_double(survey.depth(i)))
    } else {
        format!("{} kHz", java_double(survey.freq(i)))
    }
}

// ------------------------------------------------------------------------------------------
// Widget
// ------------------------------------------------------------------------------------------

/// Inputs of the section texture; mirrors the `old*` fields of `MessReihe$ViewArea`.
#[derive(Clone, Debug, PartialEq)]
struct CacheKey {
    color_model_in_use: i32,
    old_color_model_base: i32,
    old_reverse_color: bool,
    params: SectionParams,
    minimum: i64,
    maximum: i64,
    old_visible: [i32; 4],
    active_row: usize,
    data: u64,
}

#[derive(Default)]
pub struct RowView {
    texture: Option<TextureHandle>,
    cache: Option<CacheKey>,
    /// `framecnt` – number of section re-renders (diagnostics).
    framecnt: u64,
}

impl RowView {
    /// Number of times the section image was rebuilt (`framecnt`).
    pub fn frame_count(&self) -> u64 {
        self.framecnt
    }

    /// Shows the active row.
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &mut Survey, i18n: &I18n) -> ViewResponse {
        let mut resp = ViewResponse::default();
        let full = ui.available_rect_before_wrap();
        let visuals = ui.visuals().clone();
        let ppp = ui.ctx().pixels_per_point();

        let Some(row) = survey.current_row() else {
            ui.painter().text(
                full.center(),
                Align2::CENTER_CENTER,
                tr_or(i18n, "App.Row.Empty", "No row"),
                theme::label_font(),
                theme::weak_text(&visuals),
            );
            ui.advance_cursor_after_rect(full);
            return resp;
        };
        let p = SectionParams::new(survey, row);
        let n = row.points.len();
        let active = row.active_point;
        let base_y = row.base_y;
        let (cw, ch) = preferred_size(n, &p);

        let rhw = ROW_HEADER_WIDTH as f32;
        let chh = COL_HEADER_HEIGHT as f32;
        let corner = Rect::from_min_size(full.min, vec2(rhw, chh));
        let col_rect = Rect::from_min_max(pos2(full.left() + rhw, full.top()), pos2(full.right(), full.top() + chh));
        let row_rect = Rect::from_min_max(pos2(full.left(), full.top() + chh), pos2(full.left() + rhw, full.bottom()));
        let view_rect = Rect::from_min_max(pos2(full.left() + rhw, full.top() + chh), full.max);

        let mut reset_min_max = false;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view_rect));
        child.painter().rect_filled(view_rect, 0.0, theme::view_bg(&visuals));
        let out = egui::ScrollArea::both()
            .id_salt("stoneage_row_view")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                let (rect, response) = ui.allocate_exact_size(vec2(cw as f32, ch as f32), Sense::click());
                let vis = ui.clip_rect().intersect(rect);
                let row = survey.current_row().expect("row checked above");
                let points: Vec<&[i64]> = row.points.iter().map(|pt| pt.values.as_slice()).collect();
                if vis.is_positive() {
                    let vx0 = ((vis.left() - rect.left()).floor() as i32).max(0);
                    let vy0 = ((vis.top() - rect.top()).floor() as i32).max(0);
                    let vx1 = ((vis.right() - rect.left()).ceil() as i32).min(cw);
                    let vy1 = ((vis.bottom() - rect.top()).ceil() as i32).min(ch);
                    if vx1 > vx0 && vy1 > vy0 {
                        let visible = [vx0, vy0, vx1 - vx0, vy1 - vy0];
                        let key = CacheKey {
                            color_model_in_use: survey.color_model,
                            old_color_model_base: survey.color_model_base,
                            old_reverse_color: survey.reverse_color,
                            params: p,
                            minimum: row.minimum,
                            maximum: row.maximum,
                            old_visible: visible,
                            active_row: survey.active_row,
                            data: data_fingerprint(row),
                        };
                        if self.cache.as_ref() != Some(&key) || self.texture.is_none() {
                            let lut = colormap::lut(
                                survey.color_model.max(0) as usize,
                                survey.reverse_color,
                                survey.color_model_base,
                            );
                            let (idx, reset) = render_indices(&points, &p, row.minimum, row.maximum, visible);
                            reset_min_max = reset;
                            let img = indices_to_image(&idx, visible[2] as usize, visible[3] as usize, &lut);
                            match &mut self.texture {
                                Some(t) => t.set(img, TextureOptions::NEAREST),
                                None => {
                                    self.texture =
                                        Some(ui.ctx().load_texture("stoneage_row_view", img, TextureOptions::NEAREST))
                                }
                            }
                            self.cache = Some(key);
                            self.framecnt += 1;
                        }
                        if let Some(t) = &self.texture {
                            let r = Rect::from_min_size(
                                rect.min + vec2(vx0 as f32, vy0 as f32),
                                vec2((vx1 - vx0) as f32, (vy1 - vy0) as f32),
                            );
                            ui.painter().image(
                                t.id(),
                                r,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        let painter = ui.painter().with_clip_rect(vis);
                        // grid (nullColor)
                        if grid_visible(survey.show_grid, &p) {
                            let gs = Stroke::new(1.0, theme::rgba(colormap::NULL_COLOR));
                            for lx in grid_x_lines(vx0, vx1, &p) {
                                let x = theme::crisp(rect.left() + lx as f32, ppp);
                                painter.line_segment([pos2(x, vis.top()), pos2(x, vis.bottom())], gs);
                            }
                            for ly in grid_y_lines(vy0, vy1, p.disp_step_y) {
                                let y = theme::crisp(rect.top() + ly as f32, ppp);
                                painter.line_segment([pos2(vis.left(), y), pos2(vis.right(), y)], gs);
                            }
                        }
                        // active point column
                        if active < n {
                            let ax = rect.left() + (active as i32 * p.disp_step_x - p.disp_step_x / 2) as f32;
                            let ar = Rect::from_min_size(
                                pos2(ax, rect.top()),
                                vec2(p.disp_step_x.max(2) as f32, ch as f32),
                            );
                            painter.rect_stroke(ar, 0.0, theme::active_stroke(), egui::StrokeKind::Inside);
                        }
                        // horizontal cut
                        let cy = cut_marker_y(&p, base_y, survey.horizontal_cut) + p.disp_step_y / 2;
                        let y = theme::crisp(rect.top() + cy as f32, ppp);
                        painter.line_segment(
                            [pos2(vis.left(), y), pos2(vis.right(), y)],
                            Stroke::new(1.0, theme::with_alpha(theme::ACTIVE, 160)),
                        );
                    }
                }

                // hover read-out / click
                let mut hover = None;
                let mut select = None;
                if let Some(pos) = response.hover_pos() {
                    let x = (pos.x - rect.left()).floor() as i32;
                    let y = (pos.y - rect.top()).floor() as i32;
                    if !points.is_empty() {
                        let pi = point_index_for_pixel(x, n, &p);
                        let s = sample_index_for_pixel(y, &p);
                        let fd = if survey.show_depth {
                            format!("{} m", java_double(survey.depth(s as usize)))
                        } else {
                            format!("{} kHz", java_double(survey.freq(y.max(0) as usize)))
                        };
                        let value = value_for_pixel(&points, &p, x, y)
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into());
                        let pattern = tr_or(
                            i18n,
                            "App.Row.HoverInfo",
                            "Point {0}  |  x = {1} m  |  {2}  |  Value {3}",
                        );
                        hover = Some(format_msg(
                            &pattern,
                            &[
                                (pi + 1).to_string(),
                                java_double(x_pos_for_pixel(x, p.disp_step_x, survey.point_distance)),
                                fd,
                                value,
                            ],
                        ));
                        if response.clicked() {
                            select = Some(pi.max(0) as usize);
                        }
                    }
                }
                (hover, select)
            });
        let (hover, select) = out.inner;
        resp.hover_info = hover;
        resp.select_point = select;
        let offset = out.state.clamped_offset();

        if reset_min_max {
            // `MessReihe.resetMinMax()`
            if let Some(r) = survey.current_row_mut() {
                r.minimum = 0;
                r.maximum = 255;
            }
        }

        // ---- headers ----
        let painter = ui.painter();
        let text_c = theme::text(&visuals);
        let weak_c = theme::weak_text(&visuals);
        painter.rect_filled(col_rect, 0.0, theme::header_bg(&visuals));
        painter.rect_filled(row_rect, 0.0, theme::header_bg(&visuals));
        painter.rect_filled(corner, 0.0, theme::header_bg(&visuals));

        // column header
        {
            let cp = painter.with_clip_rect(col_rect);
            let maxy = COL_HEADER_HEIGHT as f32;
            let top = col_rect.top();
            for t in col_header_ticks(n, active, &p) {
                let x = col_rect.left() + t.x as f32 - offset.x;
                if x < col_rect.left() - 200.0 || x > col_rect.right() + 10.0 {
                    continue;
                }
                let xc = theme::crisp(x, ppp);
                if t.active {
                    cp.rect_filled(
                        Rect::from_min_size(
                            pos2(x - (p.disp_step_x / 2) as f32, top + maxy - 5.0),
                            vec2(p.disp_step_x.max(2) as f32, 5.0),
                        ),
                        1.0,
                        theme::ACTIVE,
                    );
                }
                cp.line_segment(
                    [pos2(xc, top + maxy - 5.0), pos2(xc, top + maxy - 2.0)],
                    Stroke::new(1.0, if t.valid { text_c } else { weak_c }),
                );
                if t.labeled {
                    cp.line_segment([pos2(xc, top + maxy - 12.0), pos2(xc, top + maxy - 6.0)], theme::axis_stroke(&visuals));
                    let label = format!(
                        "{} m",
                        java_double(x_pos_for_pixel(t.x, p.disp_step_x, survey.point_distance))
                    );
                    cp.text(pos2(x + 2.0, top + maxy - 10.0), Align2::LEFT_BOTTOM, label, theme::label_font(), text_c);
                }
            }
            cp.line_segment(
                [pos2(col_rect.left(), col_rect.bottom() - 0.5), pos2(col_rect.right(), col_rect.bottom() - 0.5)],
                theme::frame_stroke(&visuals),
            );
        }

        // row header
        {
            let rp = painter.with_clip_rect(row_rect);
            let sizex = ROW_HEADER_WIDTH as f32;
            let left = row_rect.left();
            for t in row_header_ticks(&p, base_y) {
                let y = row_rect.top() + t.y as f32 - offset.y;
                if y < row_rect.top() - 20.0 || y > row_rect.bottom() + 20.0 {
                    continue;
                }
                let yc = theme::crisp(y, ppp);
                rp.line_segment([pos2(left + sizex - 5.0, yc), pos2(left + sizex - 2.0, yc)], theme::minor_tick_stroke(&visuals));
                if t.labeled {
                    rp.line_segment([pos2(left + sizex - 12.0, yc), pos2(left + sizex - 2.0, yc)], theme::axis_stroke(&visuals));
                    rp.text(
                        pos2(left + 5.0, y + 10.0),
                        Align2::LEFT_BOTTOM,
                        y_label(survey, t.sample.max(0) as usize),
                        theme::label_font(),
                        text_c,
                    );
                }
            }
            let hc = survey.horizontal_cut;
            let y = row_rect.top() + cut_marker_y(&p, base_y, hc) as f32 - offset.y;
            rp.rect_filled(
                Rect::from_min_size(pos2(left + sizex - 5.0, y), vec2(5.0, p.disp_step_y.max(2) as f32)),
                1.0,
                theme::ACTIVE,
            );
            rp.text(
                pos2(left + 5.0, y + 10.0),
                Align2::LEFT_BOTTOM,
                format!("{} m", java_double(survey.depth(hc.max(0) as usize))),
                theme::label_font(),
                theme::ACTIVE,
            );
            rp.line_segment(
                [pos2(row_rect.right() - 0.5, row_rect.top()), pos2(row_rect.right() - 0.5, row_rect.bottom())],
                theme::frame_stroke(&visuals),
            );
        }

        // upper-left corner (`lblUpperLeft`: depth of the horizontal cut)
        painter.text(
            corner.center(),
            Align2::CENTER_CENTER,
            format!("{} m", java_double(survey.depth(survey.horizontal_cut.max(0) as usize))),
            theme::label_font(),
            theme::ACTIVE,
        );

        ui.advance_cursor_after_rect(full);
        resp
    }

    /// Render the row section to an RGBA image (used for Print / Export image).
    ///
    /// Port of `MessReihe.print()` / `printImage()`: `width`×`height` is the page; the layout
    /// constants of the original (120 dpi page, 2.5 cm left margin, 1 mm others) are scaled by
    /// `width / 992` (A4 portrait at 120 dpi).
    pub fn render_image(survey: &Survey, i18n: &I18n, width: u32, height: u32) -> egui::ColorImage {
        print::render_page(survey, i18n, width.max(1), height.max(1))
    }
}

/// Map colour-map indices to an image.
pub fn indices_to_image(idx: &[u8], w: usize, h: usize, lut: &[[u8; 4]; 256]) -> ColorImage {
    let pixels = idx.iter().map(|&i| theme::rgba(lut[i as usize])).collect();
    ColorImage::new([w, h], pixels)
}

// ------------------------------------------------------------------------------------------
// CPU raster helpers (print / export)
// ------------------------------------------------------------------------------------------

pub(crate) mod raster {
    use egui::epaint::text::{FontDefinitions, Fonts, TextOptions};
    use egui::{Color32, ColorImage, FontId};

    /// Clip rectangle in pixels `[x0, y0, x1, y1)`.
    pub type Clip = [i32; 4];

    pub fn full_clip(img: &ColorImage) -> Clip {
        [0, 0, img.size[0] as i32, img.size[1] as i32]
    }

    fn blend(dst: Color32, src: Color32, alpha: f32) -> Color32 {
        let a = alpha.clamp(0.0, 1.0);
        let mix = |d: u8, s: u8| (s as f32 * a + d as f32 * (1.0 - a)).round() as u8;
        Color32::from_rgb(mix(dst.r(), src.r()), mix(dst.g(), src.g()), mix(dst.b(), src.b()))
    }

    pub fn put(img: &mut ColorImage, clip: Clip, x: i32, y: i32, c: Color32, alpha: f32) {
        let [x0, y0, x1, y1] = clip;
        let (w, h) = (img.size[0] as i32, img.size[1] as i32);
        if x < x0.max(0) || y < y0.max(0) || x >= x1.min(w) || y >= y1.min(h) {
            return;
        }
        let i = (y * w + x) as usize;
        img.pixels[i] = if alpha >= 1.0 { c } else { blend(img.pixels[i], c, alpha) };
    }

    /// `Graphics.fillRect`.
    pub fn fill_rect(img: &mut ColorImage, clip: Clip, x: i32, y: i32, w: i32, h: i32, c: Color32) {
        for yy in y..y + h {
            for xx in x..x + w {
                put(img, clip, xx, yy, c, 1.0);
            }
        }
    }

    /// `Graphics.drawLine` (Bresenham), `t` = line thickness in pixels.
    pub fn draw_line(img: &mut ColorImage, clip: Clip, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: Color32) {
        let t = t.max(1);
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            fill_rect(img, clip, x, y, t, t, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// `Graphics.drawRect` (outline covering `w+1` × `h+1` pixels).
    pub fn draw_rect(img: &mut ColorImage, clip: Clip, x: i32, y: i32, w: i32, h: i32, t: i32, c: Color32) {
        draw_line(img, clip, x, y, x + w, y, t, c);
        draw_line(img, clip, x, y + h, x + w, y + h, t, c);
        draw_line(img, clip, x, y, x, y + h, t, c);
        draw_line(img, clip, x + w, y, x + w, y + h, t, c);
    }

    /// Draw `src` scaled into `(x, y, w, h)` with area averaging (`SCALE_AREA_AVERAGING`) and
    /// alpha blending.
    pub fn blit_scaled(dst: &mut ColorImage, clip: Clip, src: &ColorImage, x: i32, y: i32, w: i32, h: i32) {
        let (sw, sh) = (src.size[0] as i32, src.size[1] as i32);
        if w <= 0 || h <= 0 || sw <= 0 || sh <= 0 {
            return;
        }
        let (dw, dh) = (dst.size[0] as i32, dst.size[1] as i32);
        let cx0 = clip[0].max(0).max(x);
        let cy0 = clip[1].max(0).max(y);
        let cx1 = clip[2].min(dw).min(x + w);
        let cy1 = clip[3].min(dh).min(y + h);
        for dy in cy0..cy1 {
            let ry = (dy - y) as i64;
            let sy0 = (ry * sh as i64 / h as i64) as i32;
            let sy1 = (((ry + 1) * sh as i64 + h as i64 - 1) / h as i64) as i32;
            let sy1 = sy1.clamp(sy0 + 1, sh);
            for dx in cx0..cx1 {
                let rx = (dx - x) as i64;
                let sx0 = (rx * sw as i64 / w as i64) as i32;
                let sx1 = (((rx + 1) * sw as i64 + w as i64 - 1) / w as i64) as i32;
                let sx1 = sx1.clamp(sx0 + 1, sw);
                let (mut r, mut g, mut b, mut a, mut cnt) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for yy in sy0..sy1 {
                    for xx in sx0..sx1 {
                        let p = src.pixels[(yy * sw + xx) as usize];
                        let [pr, pg, pb, pa] = p.to_srgba_unmultiplied();
                        let pa = pa as f32 / 255.0;
                        r += pr as f32 * pa;
                        g += pg as f32 * pa;
                        b += pb as f32 * pa;
                        a += pa;
                        cnt += 1.0;
                    }
                }
                if a <= 0.0 {
                    continue;
                }
                let c = Color32::from_rgb((r / a).round() as u8, (g / a).round() as u8, (b / a).round() as u8);
                put(dst, clip, dx, dy, c, a / cnt);
            }
        }
    }

    /// CPU text rasterizer based on the default egui fonts (used for print / export images).
    pub struct TextRaster {
        fonts: Fonts,
    }

    impl TextRaster {
        pub fn new() -> Self {
            Self { fonts: Fonts::new(TextOptions::default(), FontDefinitions::default()) }
        }

        /// Width of `text` at pixel size `size`.
        pub fn width(&mut self, text: &str, size: f32) -> f32 {
            let g = self
                .fonts
                .with_pixels_per_point(1.0)
                .layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::BLACK);
            g.rect.width()
        }

        /// Line height (`FontMetrics.getHeight`) at pixel size `size`.
        pub fn line_height(&mut self, size: f32) -> f32 {
            self.fonts.with_pixels_per_point(1.0).row_height(&FontId::proportional(size))
        }

        /// `Graphics.drawString(text, x, baseline)`; `bold` is emulated by double striking.
        #[allow(clippy::too_many_arguments)]
        pub fn draw(
            &mut self,
            img: &mut ColorImage,
            clip: Clip,
            text: &str,
            x: f32,
            baseline: f32,
            size: f32,
            color: Color32,
            bold: bool,
        ) {
            if text.trim().is_empty() {
                return;
            }
            let galley = self
                .fonts
                .with_pixels_per_point(1.0)
                .layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
            let first_baseline = galley
                .rows
                .first()
                .map(|r| r.pos.y + r.row.glyphs.first().map(|g| g.pos.y).unwrap_or(size * 0.8))
                .unwrap_or(0.0);
            let top = baseline - first_baseline;
            let atlas = self.fonts.texture_atlas().image();
            let aw = atlas.size[0];
            let strikes: &[f32] = if bold { &[0.0, (size / 24.0).max(1.0)] } else { &[0.0] };
            for &dxs in strikes {
                for prow in &galley.rows {
                    for g in &prow.row.glyphs {
                        let uv = g.uv_rect;
                        if uv.is_nothing() {
                            continue;
                        }
                        let gx = (x + dxs + prow.pos.x + g.pos.x + uv.offset.x).round() as i32;
                        let gy = (top + prow.pos.y + g.pos.y + uv.offset.y).round() as i32;
                        let gw = (uv.max[0] - uv.min[0]) as i32;
                        let gh = (uv.max[1] - uv.min[1]) as i32;
                        for ty in 0..gh {
                            for tx in 0..gw {
                                let ax = uv.min[0] as usize + tx as usize;
                                let ay = uv.min[1] as usize + ty as usize;
                                let cov = atlas.pixels[ay * aw + ax].a();
                                if cov > 0 {
                                    put(img, clip, gx + tx, gy + ty, color, cov as f32 / 255.0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------
// Print / export layout (`MessReihe.print` + `printImage`)
// ------------------------------------------------------------------------------------------

/// Tokens of `new StringTokenizer(s, " \t\n\r", true)`.
pub fn tokenize_description(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Size of a component scaled by `printImage`: `w = min(clipRight - posx - 4, prefW)`,
/// `h = min(clipBottom - posy - 4, prefH)`, drawn at `(round(w*w/prefW), round(h*w/prefW))`.
pub fn print_image_size(pref: (i32, i32), pos: (i32, i32), clip_right: i32, clip_bottom: i32) -> Option<(i32, i32)> {
    let (pw, ph) = pref;
    if pw <= 0 || ph <= 0 {
        return None;
    }
    let w = (clip_right - pos.0 - 4).min(pw);
    let h = (clip_bottom - pos.1 - 4).min(ph);
    if w <= 0 || h <= 0 {
        return None;
    }
    let dw = java_round((w * w) as f64 / pw as f64) as i32;
    let dh = java_round((h * w) as f64 / pw as f64) as i32;
    Some((dw, dh))
}

mod print {
    use super::raster::{self, Clip, TextRaster};
    use super::*;

    /// Printer resolution of the original print job.
    const DPI: f64 = 120.0;
    /// A4 portrait width at 120 dpi – the page width the original layout was designed for.
    pub const PAGE_WIDTH_UNITS: f32 = 992.0;

    fn cm(v: f64) -> i32 {
        (v * DPI / 2.54 + 0.5) as i32
    }

    /// Complete section image (`ViewArea.paint` at preferred size on white).
    pub fn section_component(survey: &Survey, row: &Row) -> ColorImage {
        let p = SectionParams::new(survey, row);
        let (w, h) = preferred_size(row.points.len(), &p);
        let (w, h) = (w.max(1), h.max(1));
        let points: Vec<&[i64]> = row.points.iter().map(|pt| pt.values.as_slice()).collect();
        let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
        let (idx, _) = render_indices(&points, &p, row.minimum, row.maximum, [0, 0, w, h]);
        let mut img = indices_to_image(&idx, w as usize, h as usize, &lut);
        if grid_visible(survey.show_grid, &p) {
            let c = theme::rgba(colormap::NULL_COLOR);
            let clip = raster::full_clip(&img);
            for lx in grid_x_lines(0, w, &p) {
                raster::draw_line(&mut img, clip, lx, 0, lx, h, 1, c);
            }
            for ly in grid_y_lines(0, h, p.disp_step_y) {
                raster::draw_line(&mut img, clip, 0, ly, w, ly, 1, c);
            }
        }
        img
    }

    /// `ColHeader.paint` at preferred size on white.
    pub fn col_header_component(survey: &Survey, row: &Row, text: &mut TextRaster) -> ColorImage {
        let p = SectionParams::new(survey, row);
        let n = row.points.len();
        let w = col_header_width(n, p.disp_step_x).max(1);
        let maxy = COL_HEADER_HEIGHT;
        let mut img = ColorImage::filled([w as usize, maxy as usize], theme::PAGE_BG);
        let clip = raster::full_clip(&img);
        for t in col_header_ticks(n, row.active_point, &p) {
            if t.active {
                raster::fill_rect(&mut img, clip, t.x - p.disp_step_x / 2, maxy - 5, p.disp_step_x, 5, Color32::RED);
            }
            let c = if t.valid { theme::PAGE_INK } else { theme::PAGE_INK_LIGHT };
            raster::draw_line(&mut img, clip, t.x, maxy - 5, t.x, maxy - 3, 1, c);
            if t.labeled {
                raster::draw_line(&mut img, clip, t.x, maxy - 12, t.x, maxy - 7, 1, theme::PAGE_INK);
                let s = format!("{} m", java_double(x_pos_for_pixel(t.x, p.disp_step_x, survey.point_distance)));
                text.draw(&mut img, clip, &s, t.x as f32, (maxy - 10) as f32, 12.0, theme::PAGE_INK, false);
            }
        }
        img
    }

    /// `RowHeader.paint` at preferred size on white.
    pub fn row_header_component(survey: &Survey, row: &Row, text: &mut TextRaster) -> ColorImage {
        let p = SectionParams::new(survey, row);
        let sizex = ROW_HEADER_WIDTH;
        let h = row_header_height(&p).max(1);
        let mut img = ColorImage::filled([sizex as usize, h as usize], theme::PAGE_BG);
        let clip = raster::full_clip(&img);
        for t in row_header_ticks(&p, row.base_y) {
            raster::draw_line(&mut img, clip, sizex - 5, t.y, sizex - 3, t.y, 1, theme::PAGE_INK);
            if t.labeled {
                raster::draw_line(&mut img, clip, sizex - 12, t.y, sizex - 3, t.y, 1, theme::PAGE_INK);
                let s = y_label(survey, t.sample.max(0) as usize);
                text.draw(&mut img, clip, &s, 5.0, (t.y + 10) as f32, 12.0, theme::PAGE_INK, false);
            }
        }
        let hc = survey.horizontal_cut;
        let y = cut_marker_y(&p, row.base_y, hc);
        raster::fill_rect(&mut img, clip, sizex - 5, y, 5, p.disp_step_y, Color32::RED);
        let s = format!("{} m", java_double(survey.depth(hc.max(0) as usize)));
        text.draw(&mut img, clip, &s, 5.0, (y + 10) as f32, 12.0, Color32::RED, false);
        img
    }

    pub fn render_page(survey: &Survey, i18n: &I18n, width: u32, height: u32) -> ColorImage {
        let mut page = ColorImage::filled([width as usize, height as usize], theme::PAGE_BG);
        let s = width as f32 / PAGE_WIDTH_UNITS;
        let px = |u: i32| (u as f32 * s).round() as i32;
        let pxf = |u: f32| u * s;
        let line_t = s.round().max(1.0) as i32;
        let ink = theme::PAGE_INK;
        let mut text = TextRaster::new();

        // page geometry in 120-dpi units
        let page_w = PAGE_WIDTH_UNITS.round() as i32;
        let page_h = (height as f32 / s).round() as i32;
        let ml = cm(2.5); // iload 6
        let mt_rect = cm(0.1); // iload 7
        let mt = cm(0.1); // iload 8
        let mb = cm(0.1); // iload 9
        let mr = cm(0.1);
        let pw = page_w - ml - mr; // iload 14
        let ph = page_h - mt - mb; // iload 15
        let clip_units = (ml, mt, ml + pw, mt + ph);
        let page_clip: Clip = [px(clip_units.0), px(clip_units.1), px(clip_units.2) + line_t, px(clip_units.3) + line_t];
        let full: Clip = raster::full_clip(&page);

        // frame and header box
        raster::draw_rect(&mut page, full, px(ml), px(mt_rect), px(pw), px(ph), line_t, ink);
        raster::draw_rect(&mut page, full, px(ml), px(mt_rect), px(pw), px(110), line_t, ink);

        // title (serif bold 28)
        text.draw(&mut page, page_clip, &survey.name, pxf((ml + 10) as f32), pxf((mt + 33) as f32), pxf(28.0), ink, true);

        // description (serif 12) with the original word wrapping
        let fs = pxf(12.0);
        let lh = text.line_height(fs) / s; // in units
        let space_w = text.width(" ", fs) / s;
        let x0 = (ml + 10) as f32;
        let xmax = x0 + (pw - 390) as f32;
        let mut y = (mt + 33 + 15) as f32;
        let mut x = x0;
        for tok in tokenize_description(&survey.description) {
            match tok.as_str() {
                "\n" | "\r" => {
                    x = x0;
                    y += lh;
                }
                "\t" => x += space_w * 8.0,
                _ => {
                    text.draw(&mut page, page_clip, &tok, pxf(x), pxf(y), fs, ink, false);
                    x += text.width(&tok, fs) / s;
                    if x > xmax {
                        x = x0;
                        y += lh;
                    }
                }
            }
        }

        // date block
        let lhi = lh.round() as i32;
        raster::draw_rect(&mut page, page_clip, px(ml + pw - 360), px(mt), px(360), px(4 * lhi), line_t, ink);
        let dx = pxf((ml + pw - 350) as f32);
        let recorded = tr_or(i18n, "App.Row.Print.Recorded", "aufgenommen: ");
        let printed = tr_or(i18n, "App.Row.Print.Printed", "gedruckt: ");
        let now = chrono::Local::now().format("%d.%m.%Y, %H:%M Uhr").to_string();
        text.draw(&mut page, page_clip, &format!("{recorded}{}", survey.date), dx, pxf(mt as f32 + lh), fs, ink, false);
        text.draw(&mut page, page_clip, &format!("{printed}{now}"), dx, pxf(mt as f32 + 2.0 * lh), fs, ink, false);
        text.draw(&mut page, page_clip, stoneage_core::ORIGINAL_VERSION, dx, pxf(mt as f32 + 3.0 * lh), fs, ink, false);

        // components
        let place = |page: &mut ColorImage, img: &ColorImage, pos: (i32, i32)| {
            let pref = (img.size[0] as i32, img.size[1] as i32);
            if let Some((dw, dh)) = print_image_size(pref, pos, clip_units.2, clip_units.3) {
                raster::blit_scaled(page, page_clip, img, px(pos.0), px(pos.1), px(dw).max(1), px(dh).max(1));
            }
        };
        if let Some(row) = survey.current_row() {
            let view = section_component(survey, row);
            place(&mut page, &view, (ml + 80, mt + 150));
            let rh = row_header_component(survey, row, &mut text);
            place(&mut page, &rh, (ml + 4, mt + 150));
            let chd = col_header_component(survey, row, &mut text);
            place(&mut page, &chd, (ml + 80, mt + 114));
        }
        let scale = crate::scale_bar::render_legend(survey, &mut text);
        place(&mut page, &scale, (ml + pw - 80, mt + 150));

        // logo (bottom right)
        if let Ok(logo) = image::load_from_memory(stoneage_core::SPLASH_PNG) {
            let logo = logo.to_rgba8();
            let (iw, ih) = (logo.width() as i32, logo.height() as i32);
            let logo_img = ColorImage::from_rgba_unmultiplied([iw as usize, ih as usize], logo.as_raw());
            let (lx, ly) = (ml + pw - iw - 4, mt + ph - ih - 4);
            raster::blit_scaled(&mut page, page_clip, &logo_img, px(lx), px(ly), px(iw).max(1), px(ih).max(1));
        }
        page
    }
}

// ------------------------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> SectionParams {
        SectionParams {
            disp_step_x: 10,
            disp_step_y: 1,
            block_size: 1,
            use_block_average: false,
            interpolate: false,
            valid_base: 0,
            valid_step: 1,
        }
    }

    fn ramp(offset: i64) -> Vec<i64> {
        (0..SAMPLES as i64).map(|i| i + offset).collect()
    }

    #[test]
    fn java_round_matches_java() {
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(-2.6), -3);
        assert_eq!(java_round(0.49), 0);
    }

    #[test]
    fn java_double_format() {
        assert_eq!(java_double(2.0), "2.0");
        assert_eq!(java_double(1.25), "1.25");
        assert_eq!(java_double(0.01), "0.01");
        assert_eq!(java_double(0.0), "0.0");
        assert_eq!(java_double(1e7), "1.0E7");
        assert_eq!(java_double(0.0001), "1.0E-4");
    }

    #[test]
    fn block_value_rules() {
        let v = ramp(0);
        assert_eq!(block_value(&v, 5, 1, false), 5);
        assert_eq!(block_value(&v, 3000, 1, false), 2047);
        assert_eq!(block_value(&v, -1, 1, false), 0);
        // block 4 starting at round(5/4)*4 = 4 -> 4..8
        assert_eq!(block_value(&v, 5, 4, false), 7);
        assert_eq!(block_value(&v, 5, 4, true), (4 + 5 + 6 + 7) / 4);
        // round(6/4)=2 -> 8..12
        assert_eq!(block_value(&v, 6, 4, false), 11);
        // block runs past the end -> values[2047] for idx >= 2048, values[0] otherwise
        assert_eq!(block_value(&v, 2047, 4, false), 0);
        assert_eq!(block_value(&v, 2048, 4, false), 2047);
        // max starts at 0
        let neg = vec![-5i64; 2048];
        assert_eq!(block_value(&neg, 0, 2, false), 0);
        assert_eq!(block_value(&neg, 0, 2, true), -5);
    }

    #[test]
    fn sizes() {
        let p = params();
        assert_eq!(preferred_size(5, &p), (40, 2048));
        assert_eq!(preferred_size(1, &p), (10, 2048));
        let p2 = SectionParams { block_size: 4, disp_step_y: 3, ..p };
        assert_eq!(preferred_size(5, &p2), (40, 512 * 3));
        assert_eq!(row_header_height(&p2), 512 * 3 + 10);
        assert_eq!(col_header_width(5, 10), 100);
    }

    #[test]
    fn x_pos() {
        assert_eq!(x_pos_for_pixel(25, 10, 0.5), 1.25);
        assert_eq!(x_pos_for_pixel(10, 10, 1.0 / 3.0), 0.33);
    }

    #[test]
    fn point_index_mapping() {
        let p = params();
        assert_eq!(point_index_for_pixel(0, 5, &p), 0);
        assert_eq!(point_index_for_pixel(14, 5, &p), 1);
        assert_eq!(point_index_for_pixel(15, 5, &p), 2);
        assert_eq!(point_index_for_pixel(1000, 5, &p), 4);
        assert_eq!(point_index_for_pixel(-50, 5, &p), 0);
        // valid base/step: only 1,3,5,.. are valid
        let pv = SectionParams { valid_base: 1, valid_step: 2, ..p };
        assert_eq!(point_index_for_pixel(0, 6, &pv), 1);
        assert_eq!(point_index_for_pixel(29, 6, &pv), 3);
        assert_eq!(point_index_for_pixel(100, 6, &pv), 5);
        // interpolation walks down to the previous valid index
        let pi = SectionParams { interpolate: true, ..pv };
        assert_eq!(point_index_for_pixel(49, 6, &pi), 3);
        assert_eq!(point_index_for_pixel(50, 6, &pi), 5);
    }

    #[test]
    fn value_for_pixel_plain_and_interpolated() {
        let a = ramp(0);
        let b = ramp(100);
        let pts: Vec<&[i64]> = vec![&a, &b];
        let p = params();
        assert_eq!(value_for_pixel(&pts, &p, 0, 7), Some(7));
        assert_eq!(value_for_pixel(&pts, &p, 9, 7), Some(107));
        let pi = SectionParams { interpolate: true, ..p };
        // halfway between point 0 and 1 → 7 + 50
        assert_eq!(value_for_pixel(&pts, &pi, 5, 7), Some(57));
        let empty: Vec<&[i64]> = vec![];
        assert_eq!(value_for_pixel(&empty, &p, 0, 0), None);
    }

    #[test]
    fn section_pixels() {
        let a = ramp(0);
        let b = ramp(100);
        let pts: Vec<&[i64]> = vec![&a, &b];
        let p = params();
        // zero becomes 1
        assert_eq!(section_raw_value(&pts, &p, 0, 0), 1);
        assert_eq!(section_raw_value(&pts, &p, 4, 10), 10);
        assert_eq!(section_raw_value(&pts, &p, 5, 10), 110);
        // missing point → 0
        assert_eq!(section_raw_value(&pts, &p, 25, 10), 0);
        let pi = SectionParams { interpolate: true, ..p };
        assert_eq!(section_raw_value(&pts, &pi, 5, 10), 60);
        // p0+vs missing → 0
        assert_eq!(section_raw_value(&pts, &pi, 10, 10), 0);
        // s1 must be < 2048
        assert_eq!(section_raw_value(&pts, &pi, 5, 2047), 0);
        // vertical interpolation with dsy = 4
        let pv = SectionParams { disp_step_y: 4, ..pi };
        assert_eq!(section_raw_value(&pts, &pv, 0, 2), 1); // 0 + 2*(1-0)/4 = 0.5 → round 1
        assert_eq!(section_raw_value(&pts, &pv, 0, 6), 2); // 1 + 0.5 → 2 (round half up)
    }

    #[test]
    fn scaling() {
        assert_eq!(scale_value(5, 0, 255), Some(5));
        assert_eq!(scale_value(300, 7, 7), Some(300));
        assert_eq!(scale_value(10, 0, 10), Some(255));
        assert_eq!(scale_value(5, 0, 10), Some(128));
        assert_eq!(scale_value(11, 0, 10), None);
        let a = vec![5i64; 2048];
        let pts: Vec<&[i64]> = vec![&a, &a];
        let p = params();
        let (idx, reset) = render_indices(&pts, &p, 0, 10, [0, 0, 2, 2]);
        assert!(!reset);
        assert_eq!(idx, vec![128; 4]);
        let (idx, reset) = render_indices(&pts, &p, 6, 10, [0, 0, 2, 1]);
        assert!(reset);
        assert_eq!(idx, vec![5, 5]);
    }

    #[test]
    fn grid_lines() {
        let p = SectionParams { valid_step: 2, valid_base: 1, ..params() };
        assert_eq!(grid_x_lines(0, 60, &p), vec![10, 30, 50]);
        assert_eq!(grid_y_lines(0, 10, 4), vec![0, 4, 8]);
        assert!(!grid_visible(true, &params()));
        assert!(grid_visible(true, &SectionParams { disp_step_y: 3, ..params() }));
    }

    #[test]
    fn header_ticks() {
        let p = params();
        let t = col_header_ticks(12, 3, &p);
        let labeled: Vec<i32> = t.iter().filter(|t| t.labeled).map(|t| t.x).collect();
        assert_eq!(labeled, vec![0, 60]);
        assert!(t[3].active && !t[2].active);
        let r = row_header_ticks(&SectionParams { block_size: 1, disp_step_y: 1, ..p }, 0);
        assert_eq!(r.len(), 2048);
        assert_eq!(r[0].y, 0);
        assert!(r[0].labeled);
        assert!(!r[27].labeled);
        assert!(r[28].labeled);
        assert!(r[2047].labeled);
        assert_eq!(cut_marker_y(&SectionParams { block_size: 2, disp_step_y: 4, ..p }, 0, 10), -2 + 20);
    }

    #[test]
    fn description_tokens_and_print_scaling() {
        assert_eq!(tokenize_description("ab c\td\n"), vec!["ab", " ", "c", "\t", "d", "\n"]);
        assert_eq!(print_image_size((100, 50), (0, 0), 1000, 1000), Some((100, 50)));
        // width limited to 50 → factor 0.5
        assert_eq!(print_image_size((100, 50), (0, 0), 54, 1000), Some((25, 25)));
        assert_eq!(print_image_size((100, 50), (0, 0), 3, 1000), None);
    }

    /// Smoke test against the real core (colour maps, i18n, demo data). Run with
    /// `cargo test -p stoneage-plots -- --ignored`; writes `row_print_smoke.png` to the temp dir.
    #[test]
    #[ignore]
    fn smoke_render_and_ui() {
        let mut survey = Survey::default();
        survey.name = "Demo".into();
        survey.description = "Line one with some words
second	line".into();
        survey.show_grid = true;
        survey.disp_step_x = 8;
        survey.disp_step_y = 3;
        survey.block_size = 4;
        let mut row = Row::new();
        for i in 0..20 {
            row.add_point(stoneage_core::Point::demo(0, i));
        }
        row.find_min_max_with(survey.block_size, survey.use_block_average);
        survey.rows = vec![row];
        survey.active_row = 0;
        let i18n = I18n::new(stoneage_core::Language::English);
        let img = RowView::render_image(&survey, &i18n, 992, 1403);
        let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let out = std::env::temp_dir().join("row_print_smoke.png");
        image::save_buffer(&out, &raw, 992, 1403, image::ExtendedColorType::Rgba8).unwrap();
        let ctx = egui::Context::default();
        let mut view = RowView::default();
        let mut bar = crate::ScaleBar::default();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.horizontal(|ui| {
                    bar.ui(ui, &mut survey, &i18n);
                    view.ui(ui, &mut survey, &i18n);
                });
            });
            out.textures_delta.clear();
        }
        assert!(view.frame_count() >= 1);
    }

    #[test]
    fn message_format() {
        assert_eq!(format_msg("a {0} b {1}", &["x".into(), "y".into()]), "a x b y");
    }
}
