//! Areal view: horizontal depth cut over all rows x points.
//!
//! Port of `MessAreal$ViewArea` (pixel raster, interpolation, grid), `MessAreal$ColHeader`
//! (point distances in metres, active point marker), `MessAreal$RowHeader` (row distances in
//! metres, active row marker) and the display parts of `MessAreal` (upper-left corner label with
//! the depth of the cut, header sizes 25 / 70 px).
//!
//! The raster is computed exactly like the original: one image pixel per screen pixel, point `i`
//! of row `r` sits at pixel `(i * disp_step_x, r * disp_step_z)`, the colour index of a pixel is the
//! (block averaged) value of the point(s) at `survey.horizontal_cut`.

use std::sync::Arc;

use egui::{
    Align, Align2, Color32, ColorImage, CornerRadius, CursorIcon, FontId, Layout, Pos2, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, UiBuilder, Vec2,
};
use stoneage_core::{colormap, I18n, Language, Survey};
use stoneage_plots::theme;

use crate::ViewResponse;

/// `MessAreal.colHeaderHeight`
pub const COL_HEADER_HEIGHT: f32 = 25.0;
/// `MessAreal.rowHeaderWidth`
pub const ROW_HEADER_WIDTH: f32 = 70.0;
/// Breathing room around the raster inside the scroll area (modern look; not in the original).
const PAD: f32 = 12.0;

// ---------------------------------------------------------------------------------------------
// Pure helpers (unit tested)
// ---------------------------------------------------------------------------------------------

/// Java `Math.round(double)` (round half up).
pub fn java_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// Java `Double.toString` for the values that occur in the headers/labels
/// (`5.0`, `12.5`, `0.35`, ...).
pub fn java_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == v.trunc() && v.abs() < 1e7 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// `java.text.MessageFormat` rendering of a `Double` argument (`NumberFormat.getInstance()`:
/// pattern `#,##0.###`, HALF_EVEN) for the given UI language.
pub fn message_format_number(v: f64, lang: Language) -> String {
    let (group, decimal) = match lang {
        Language::English => (',', '.'),
        Language::German | Language::Turkish => ('.', ','),
    };
    if !v.is_finite() {
        return if v.is_nan() { "\u{FFFD}".into() } else if v > 0.0 { "\u{221E}".into() } else { "-\u{221E}".into() };
    }
    // HALF_EVEN to 3 fraction digits.
    let scaled = v.abs() * 1000.0;
    let mut r = scaled.round();
    if (scaled - scaled.trunc() - 0.5).abs() < 1e-9 && (r as i64) % 2 != 0 {
        r -= 1.0;
    }
    let r = r as u64;
    let int_part = r / 1000;
    let frac = r % 1000;
    let mut s = String::new();
    if v < 0.0 && r != 0 {
        s.push('-');
    }
    let digits = int_part.to_string();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            s.push(group);
        }
        s.push(c);
    }
    if frac != 0 {
        s.push(decimal);
        let mut f = format!("{frac:03}");
        while f.ends_with('0') {
            f.pop();
        }
        s.push_str(&f);
    }
    s
}

/// Port of `MessPunkt.getValue(int)` including the block average / block maximum of the
/// survey's `blockSize` / `useBlockAverage` (the original reads them from `MainFrame.areal`).
///
/// A block size `<= 0` is treated like 1 (the original would divide by zero).
/// The views use `Survey::value` (core) at runtime; this local copy documents/tests the maths.
pub fn block_value(values: &[i64], idx: i32, block_size: i32, use_block_average: bool) -> i64 {
    let fallback = || -> i64 {
        if values.is_empty() {
            0
        } else if idx >= 2048 {
            *values.get(2047).unwrap_or(&values[values.len() - 1])
        } else {
            values[0]
        }
    };
    let bs = block_size.max(1) as i64;
    if bs == 1 {
        return if idx >= 0 { values.get(idx as usize).copied().unwrap_or_else(fallback) } else { fallback() };
    }
    let start = (java_round(idx as f64 / bs as f64) * bs) as i32;
    let mut sum: i64 = 0;
    let mut max: i64 = 0;
    let mut i = start as i64;
    while i < start as i64 + bs {
        if i < 0 || i as usize >= values.len() {
            return fallback();
        }
        let v = values[i as usize];
        sum += v;
        max = max.max(v);
        i += 1;
    }
    if use_block_average {
        sum / bs
    } else {
        max
    }
}

/// Size/position mapping of the areal raster (`MessAreal$ViewArea.getPreferredSize`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArealGeometry {
    pub rows: usize,
    pub max_points: usize,
    pub step_x: i32,
    pub step_z: i32,
}

impl ArealGeometry {
    pub fn new(rows: usize, max_points: usize, step_x: i32, step_z: i32) -> Self {
        Self { rows, max_points, step_x: step_x.max(1), step_z: step_z.max(1) }
    }

    pub fn from_survey(s: &Survey) -> Self {
        Self::new(s.rows.len(), s.max_points(), s.disp_step_x, s.disp_step_z)
    }

    /// `(maxPoints-1)*disp_stepx`, or `disp_stepx` when that is 0. Negative when there are no points.
    pub fn pref_width(&self) -> i32 {
        let w = (self.max_points as i32 - 1) * self.step_x;
        if w == 0 {
            self.step_x
        } else {
            w
        }
    }

    /// `(rows-1)*disp_stepz`, or `disp_stepz` when that is 0.
    pub fn pref_height(&self) -> i32 {
        let h = (self.rows as i32 - 1) * self.step_z;
        if h == 0 {
            self.step_z
        } else {
            h
        }
    }

    /// Raster size in pixels (0 when the preferred size is negative).
    pub fn pixel_size(&self) -> [usize; 2] {
        [self.pref_width().max(0) as usize, self.pref_height().max(0) as usize]
    }

    /// Pixel position of point `point` in row `row`.
    pub fn cell_to_pixel(&self, point: usize, row: usize) -> (i32, i32) {
        (point as i32 * self.step_x, row as i32 * self.step_z)
    }

    /// Nearest measurement cell `(point, row)` of a pixel (`Math.round(x / disp_stepx)`).
    pub fn pixel_to_cell(&self, x: f64, z: f64) -> (i64, i64) {
        (java_round(x / self.step_x as f64), java_round(z / self.step_z as f64))
    }
}

/// Values of all measurement cells at the depth cut, `None` where a row has fewer points.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CellGrid {
    pub rows: usize,
    pub cols: usize,
    pub values: Vec<Option<i64>>,
}

impl CellGrid {
    /// `MessAreal.getPunkt(point, row)` + `getValue(horizontalschnitttiefe)`.
    pub fn get(&self, point: i64, row: i64) -> Option<i64> {
        if point < 0 || row < 0 || point as usize >= self.cols || row as usize >= self.rows {
            return None;
        }
        self.values[row as usize * self.cols + point as usize]
    }

    pub fn from_survey(s: &Survey) -> Self {
        let rows = s.rows.len();
        let cols = s.max_points();
        let mut values = vec![None; rows * cols];
        for (r, row) in s.rows.iter().enumerate() {
            for (p, pt) in row.points.iter().enumerate() {
                values[r * cols + p] =
                    Some(s.value(pt, s.horizontal_cut as i64));
            }
        }
        Self { rows, cols, values }
    }
}

/// The pixel loop of `MessAreal$ViewArea.paintComponent`: returns the colour index value of
/// every pixel (row-major, `geom.pixel_size()`), exactly as the original computes it.
pub fn compute_raster(geom: &ArealGeometry, cells: &CellGrid, interpolate: bool) -> Vec<i64> {
    let [w, h] = geom.pixel_size();
    let sx = geom.step_x as i64;
    let sz = geom.step_z as i64;
    let mut out = vec![0i64; w * h];
    for z in 0..h as i64 {
        for x in 0..w as i64 {
            let v = if interpolate {
                let z0 = z.div_euclid(sz);
                let x0 = x.div_euclid(sx);
                match (cells.get(x0, z0), cells.get(x0 + 1, z0), cells.get(x0, z0 + 1), cells.get(x0 + 1, z0 + 1)) {
                    (Some(v00), Some(v10), Some(v01), Some(v11)) => {
                        let dx = x % sx;
                        let dz = z % sz;
                        let a = v00 as f64 + 1.0 * dx as f64 * (v10 - v00) as f64 / sx as f64;
                        let b = v01 as f64 + 1.0 * dx as f64 * (v11 - v01) as f64 / sx as f64;
                        let mut v = java_round(a + 1.0 * dz as f64 * (b - a) / sz as f64);
                        if v == 0 {
                            v = 1;
                        }
                        // The original leaves the exact measurement positions at index 0.
                        if dx == 0 && dz == 0 {
                            v = 0;
                        }
                        v
                    }
                    _ => 0,
                }
            } else {
                let zi = java_round(z as f64 / sz as f64);
                let xi = java_round(x as f64 / sx as f64);
                match cells.get(xi, zi) {
                    Some(0) => 1,
                    Some(v) => v,
                    None => 0,
                }
            };
            out[z as usize * w + x as usize] = v;
        }
    }
    out
}

/// Pixel columns/rows of the grid lines (`showGrid && disp_step > 2`): multiples of the step below `limit`.
pub fn grid_lines(step: i32, limit: usize) -> Vec<usize> {
    if step <= 0 {
        return Vec::new();
    }
    (0..limit).step_by(step as usize).collect()
}

/// `IndexColorModel.getRGB(pixel)` masks the pixel with `0xff` for an 8-bit model.
pub fn lut_index(v: i64) -> usize {
    (v & 0xff) as usize
}

/// Colourise a raster with a 256 entry LUT; optionally bake the grid (colour `ColorMap.nullColor`).
pub fn colorize(raster: &[i64], size: [usize; 2], lut: &[[u8; 4]; 256], grid: Option<(i32, i32)>) -> ColorImage {
    let [w, h] = size;
    let mut px: Vec<Color32> = raster
        .iter()
        .map(|&v| {
            let c = lut[lut_index(v)];
            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
        })
        .collect();
    if let Some((sx, sz)) = grid {
        let n = colormap::NULL_COLOR;
        let nc = Color32::from_rgb(n[0], n[1], n[2]);
        for x in grid_lines(sx, w) {
            for z in 0..h {
                px[z * w + x] = nc;
            }
        }
        for z in grid_lines(sz, h) {
            for x in 0..w {
                px[z * w + x] = nc;
            }
        }
    }
    ColorImage::new(size, px)
}

/// Whether the grid is shown (`showGrid && disp_stepx > 2 && disp_stepz > 2`).
pub fn grid_enabled(s: &Survey) -> bool {
    s.show_grid && s.disp_step_x > 2 && s.disp_step_z > 2
}

/// Look up a key, falling back to `fallback` when the key is missing, then substitute `{0}`, `{1}`...
pub(crate) fn tr_or(i18n: &I18n, key: &str, fallback: &str, args: &[&str]) -> String {
    let t = i18n.tr(key);
    let mut s = if t == key || t.is_empty() { fallback.to_string() } else { t };
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a);
    }
    s
}

// ---------------------------------------------------------------------------------------------
// Texture cache (shared with the Map view)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, PartialEq, Debug)]
struct RasterKey {
    geom: ArealGeometry,
    cells: CellGrid,
    interpolate: bool,
    color_model: i32,
    reverse: bool,
    base: i32,
    grid: Option<(i32, i32)>,
}

/// Cached raster + texture; re-rendered/re-uploaded only when an input changes
/// (the original's `oldnump`, `oldhorizontalschnitttiefe`, `olddisp_step*`, `oldinterpolate`,
/// `colorModelInUse`, `oldColorModelBase`, `oldReverseColor` checks; here additionally the data).
#[derive(Default)]
pub(crate) struct ArealTexture {
    key: Option<RasterKey>,
    raster: Arc<Vec<i64>>,
    size: [usize; 2],
    tex: Option<TextureHandle>,
}

impl ArealTexture {
    /// Updates (if needed) and returns the texture; `None` when there is nothing to draw.
    pub(crate) fn update(&mut self, ctx: &egui::Context, s: &Survey, bake_grid: bool, name: &str) -> Option<&TextureHandle> {
        let geom = ArealGeometry::from_survey(s);
        let size = geom.pixel_size();
        if size[0] == 0 || size[1] == 0 {
            return None;
        }
        let key = RasterKey {
            geom,
            cells: CellGrid::from_survey(s),
            interpolate: s.interpolate,
            color_model: s.color_model,
            reverse: s.reverse_color,
            base: s.color_model_base,
            grid: if bake_grid && grid_enabled(s) { Some((geom.step_x, geom.step_z)) } else { None },
        };
        if self.key.as_ref() != Some(&key) || self.tex.is_none() {
            let raster = compute_raster(&key.geom, &key.cells, key.interpolate);
            let lut = colormap::lut(key.color_model.max(0) as usize, key.reverse, key.base);
            let img = colorize(&raster, size, &lut, key.grid);
            match &mut self.tex {
                Some(t) => t.set(img, TextureOptions::NEAREST),
                None => self.tex = Some(ctx.load_texture(name, img, TextureOptions::NEAREST)),
            }
            self.raster = Arc::new(raster);
            self.size = size;
            self.key = Some(key);
        }
        self.tex.as_ref()
    }

    fn raster_value(&self, x: usize, z: usize) -> Option<i64> {
        if x < self.size[0] && z < self.size[1] {
            self.raster.get(z * self.size[0] + x).copied()
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct ArealView {
    tex: ArealTexture,
}

impl ArealView {
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &mut Survey, i18n: &I18n) -> ViewResponse {
        let mut resp = ViewResponse::default();
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, visuals.extreme_bg_color);

        let geom = ArealGeometry::from_survey(survey);
        let [w, h] = geom.pixel_size();
        if w == 0 || h == 0 || survey.rows.is_empty() {
            painter.text(
                full.center(),
                Align2::CENTER_CENTER,
                tr_or(i18n, "App.Areal.NoData", "No measurement data", &[]),
                FontId::proportional(14.0),
                visuals.weak_text_color(),
            );
            return resp;
        }

        let tex_id = self.tex.update(ui.ctx(), survey, false, "stoneage_areal").map(|t| t.id());

        // --- content (scrollable raster) ---
        let content_rect = Rect::from_min_max(full.min + Vec2::new(ROW_HEADER_WIDTH, COL_HEADER_HEIGHT), full.max);
        let mut child = ui.new_child(UiBuilder::new().max_rect(content_rect).layout(Layout::top_down(Align::Min)));
        let out = ScrollArea::both()
            .id_salt("stoneage_areal_scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                let (rect, r) = ui.allocate_exact_size(Vec2::new(w as f32 + 2.0 * PAD, h as f32 + 2.0 * PAD), Sense::click());
                ((rect.min + Vec2::splat(PAD)).round(), r)
            });
        let (origin, r) = out.inner;
        let view_rect = out.inner_rect;
        let vp = ui.painter_at(view_rect);
        let img_rect = Rect::from_min_size(origin, Vec2::new(w as f32, h as f32));
        if let Some(id) = tex_id {
            vp.image(id, img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        vp.rect_stroke(img_rect, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Outside);

        // Grid (drawn as crisp overlay lines in the original colour).
        if grid_enabled(survey) {
            let n = colormap::NULL_COLOR;
            let stroke = Stroke::new(1.0, Color32::from_rgb(n[0], n[1], n[2]));
            for x in grid_lines(geom.step_x, w) {
                let xx = origin.x + x as f32 + 0.5;
                vp.line_segment([Pos2::new(xx, img_rect.top()), Pos2::new(xx, img_rect.bottom())], stroke);
            }
            for z in grid_lines(geom.step_z, h) {
                let zz = origin.y + z as f32 + 0.5;
                vp.line_segment([Pos2::new(img_rect.left(), zz), Pos2::new(img_rect.right(), zz)], stroke);
            }
        }

        let active_row = survey.active_row;
        let active_point = survey.current_row().map(|r| r.active_point);

        // Active measurement cell marker.
        if let Some(ap) = active_point {
            if survey.current_row().is_some_and(|r| ap < r.points.len()) {
                let (px, pz) = geom.cell_to_pixel(ap, active_row);
                let c = origin + Vec2::new(px as f32 + 0.5, pz as f32 + 0.5);
                let rad = (geom.step_x.min(geom.step_z) as f32 * 0.35).clamp(3.0, 10.0);
                vp.circle_stroke(c, rad + 1.0, Stroke::new(3.0, Color32::from_black_alpha(140)));
                vp.circle_stroke(c, rad, Stroke::new(2.0, theme::ACCENT));
            }
        }

        // Hover read-out + click selection.
        let cell_at = |p: Pos2| -> Option<(usize, usize, usize, usize)> {
            let l = p - origin;
            if l.x < 0.0 || l.y < 0.0 || l.x >= w as f32 || l.y >= h as f32 {
                return None;
            }
            let (pt, row) = geom.pixel_to_cell(l.x.floor() as f64, l.y.floor() as f64);
            Some((pt.max(0) as usize, row.max(0) as usize, l.x as usize, l.y as usize))
        };
        if let Some(hp) = r.hover_pos() {
            if let Some((pt, row, ix, iz)) = cell_at(hp) {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                let exists = survey.point(row, pt).is_some();
                let (cx, cz) = geom.cell_to_pixel(pt, row);
                let cell = Rect::from_center_size(
                    origin + Vec2::new(cx as f32 + 0.5, cz as f32 + 0.5),
                    Vec2::new(geom.step_x as f32, geom.step_z as f32),
                );
                if exists {
                    vp.rect_stroke(cell, CornerRadius::same(2), Stroke::new(1.0, Color32::from_white_alpha(160)), StrokeKind::Inside);
                }
                let xm = format!("{:.2}", ix as f64 / geom.step_x as f64 * survey.point_distance);
                let ym = format!("{:.2}", iz as f64 / geom.step_z as f64 * survey.row_distance);
                let val = self.tex.raster_value(ix, iz).map(|v| v.to_string()).unwrap_or_default();
                let text = if exists {
                    tr_or(
                        i18n,
                        "App.Areal.Hover",
                        "Row {0}, point {1}  |  x = {2} m, y = {3} m  |  value {4}",
                        &[&(row + 1).to_string(), &(pt + 1).to_string(), &xm, &ym, &val],
                    )
                } else {
                    tr_or(i18n, "App.Areal.HoverNoPoint", "x = {0} m, y = {1} m  |  no point", &[&xm, &ym])
                };
                resp.hover_info = Some(text);
            }
        }
        if r.clicked() {
            if let Some((pt, row, _, _)) = r.interact_pointer_pos().and_then(cell_at) {
                if survey.point(row, pt).is_some() {
                    survey.active_row = row;
                    if let Some(rw) = survey.rows.get_mut(row) {
                        rw.active_point = pt;
                    }
                    resp.select_row = Some(row);
                    resp.select_point = Some(pt);
                }
            }
        }

        // --- headers ---
        let header_bg = visuals.panel_fill;
        let text_col = visuals.text_color();
        let tick_col = visuals.weak_text_color();
        let font = FontId::proportional(11.0);
        let border = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);

        // Column header (`MessAreal$ColHeader.paintComponent`).
        let col_strip = Rect::from_min_max(Pos2::new(view_rect.left(), full.top()), Pos2::new(full.right(), view_rect.top()));
        let cp = ui.painter_at(col_strip);
        cp.rect_filled(col_strip, 0.0, header_bg);
        cp.line_segment([col_strip.left_bottom(), col_strip.right_bottom()], border);
        let hb = col_strip.bottom();
        let half_x = (geom.step_x / 2) as f32;
        let mut last = -100i32;
        for i in 0..geom.max_points {
            let xi = i as i32 * geom.step_x;
            let x = origin.x + xi as f32 + 0.5;
            if active_point == Some(i) {
                cp.rect_filled(
                    Rect::from_min_size(Pos2::new(x - half_x, hb - 7.0), Vec2::new(geom.step_x as f32, 5.0)),
                    CornerRadius::same(2),
                    theme::ACCENT,
                );
            }
            cp.line_segment([Pos2::new(x, hb - 5.0), Pos2::new(x, hb - 3.0)], Stroke::new(1.0, tick_col));
            if xi > last + 50 {
                last = xi;
                cp.line_segment([Pos2::new(x, hb - 12.0), Pos2::new(x, hb - 3.0)], Stroke::new(1.0, text_col));
                let label = format!("{} m", java_double(java_round(i as f64 * survey.point_distance * 100.0) as f64 / 100.0));
                cp.text(Pos2::new(x + 3.0, hb - 8.0), Align2::LEFT_BOTTOM, label, font.clone(), text_col);
            }
        }

        // Row header (`MessAreal$RowHeader.paintComponent`).
        let row_strip = Rect::from_min_max(Pos2::new(full.left(), view_rect.top()), Pos2::new(view_rect.left(), full.bottom()));
        let rp = ui.painter_at(row_strip);
        rp.rect_filled(row_strip, 0.0, header_bg);
        rp.line_segment([row_strip.right_top(), row_strip.right_bottom()], border);
        let rw = row_strip.right();
        let half_z = (geom.step_z / 2) as f32;
        let mut last = -100i32;
        let nrows = survey.rows.len();
        for i in 0..nrows {
            let yi = i as i32 * geom.step_z;
            let y = origin.y + yi as f32 + 0.5;
            rp.line_segment([Pos2::new(rw - 5.0, y), Pos2::new(rw - 3.0, y)], Stroke::new(1.0, tick_col));
            if active_row == i {
                rp.rect_filled(
                    Rect::from_min_size(Pos2::new(rw - 7.0, y - half_z), Vec2::new(5.0, geom.step_z as f32)),
                    CornerRadius::same(2),
                    theme::ACCENT,
                );
            }
            if yi > last + 25 || i == nrows - 1 {
                last = yi;
                rp.line_segment([Pos2::new(rw - 12.0, y), Pos2::new(rw - 3.0, y)], Stroke::new(1.0, text_col));
                let label = format!("{} m ({i})", java_double(java_round(i as f64 * survey.row_distance * 100.0) as f64 / 100.0));
                rp.text(Pos2::new(rw - 14.0, y), Align2::RIGHT_CENTER, label, font.clone(), text_col);
            }
        }

        // Upper-left corner: depth of the cut (`lblUpperLeft`, centred).
        let corner = Rect::from_min_max(full.min, Pos2::new(view_rect.left(), view_rect.top()));
        let kp = ui.painter_at(corner);
        kp.rect_filled(corner, 0.0, header_bg);
        kp.line_segment([corner.left_bottom(), corner.right_bottom()], border);
        kp.line_segment([corner.right_top(), corner.right_bottom()], border);
        let depth = survey.depth(survey.horizontal_cut.max(0) as usize);
        let mut label = format!("{} m", java_double(depth));
        let gw = kp.layout_no_wrap(label.clone(), FontId::proportional(11.5), text_col).size().x;
        if gw > corner.width() - 4.0 {
            label = format!("{depth:.2} m");
        }
        kp.text(corner.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.5), theme::ACCENT);

        resp
    }

    /// Render to an image for Export image / Print: the raster exactly as the original paints it
    /// (including the grid), scaled (nearest neighbour) to `width` x `height` when both are non-zero.
    pub fn render_image(survey: &Survey, width: u32, height: u32) -> egui::ColorImage {
        let geom = ArealGeometry::from_survey(survey);
        let size = geom.pixel_size();
        if size[0] == 0 || size[1] == 0 {
            let s = [width.max(1) as usize, height.max(1) as usize];
            return ColorImage::filled(s, Color32::WHITE);
        }
        let cells = CellGrid::from_survey(survey);
        let raster = compute_raster(&geom, &cells, survey.interpolate);
        let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
        let grid = if grid_enabled(survey) { Some((geom.step_x, geom.step_z)) } else { None };
        let img = colorize(&raster, size, &lut, grid);
        if width == 0 || height == 0 || (width as usize == size[0] && height as usize == size[1]) {
            return img;
        }
        resample_nearest(&img, [width as usize, height as usize])
    }
}

/// Nearest-neighbour resampling.
pub fn resample_nearest(img: &ColorImage, size: [usize; 2]) -> ColorImage {
    let [sw, sh] = img.size;
    let [w, h] = size;
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        let sy = (y * sh / h.max(1)).min(sh.saturating_sub(1));
        for x in 0..w {
            let sx = (x * sw / w.max(1)).min(sw.saturating_sub(1));
            px.push(img.pixels[sy * sw + sx]);
        }
    }
    ColorImage::new(size, px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_round_matches() {
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(-0.5), 0);
        assert_eq!(java_round(1.49), 1);
        assert_eq!(java_round(-1.5), -1);
    }

    #[test]
    fn java_double_format() {
        assert_eq!(java_double(5.0), "5.0");
        assert_eq!(java_double(12.5), "12.5");
        assert_eq!(java_double(0.35), "0.35");
        assert_eq!(java_double(0.0), "0.0");
    }

    #[test]
    fn message_format() {
        assert_eq!(message_format_number(3.0, Language::English), "3");
        assert_eq!(message_format_number(3.14159, Language::English), "3.142");
        assert_eq!(message_format_number(3.14159, Language::German), "3,142");
        assert_eq!(message_format_number(1234.5, Language::English), "1,234.5");
        assert_eq!(message_format_number(1234.5, Language::Turkish), "1.234,5");
        assert_eq!(message_format_number(0.0005, Language::English), "0");
        assert_eq!(message_format_number(0.0015, Language::English), "0.002");
    }

    #[test]
    fn block_value_cases() {
        let v: Vec<i64> = (0..2048).map(|i| (i % 100) as i64).collect();
        assert_eq!(block_value(&v, 5, 1, false), 5);
        assert_eq!(block_value(&v, 3000, 1, false), v[2047]);
        assert_eq!(block_value(&v, -1, 1, false), v[0]);
        // block 10: idx 14 -> round(1.4)=1 -> 10..20
        assert_eq!(block_value(&v, 14, 10, false), 19);
        assert_eq!(block_value(&v, 14, 10, true), (10..20).sum::<i64>() / 10);
        // idx 15 -> round(1.5)=2 -> 20..30
        assert_eq!(block_value(&v, 15, 10, false), 29);
        // past the end -> AIOOBE fallback
        assert_eq!(block_value(&v, 2047, 10, true), v[0]); // idx < 2048 -> values[0]
        assert_eq!(block_value(&v, 2100, 10, true), v[2047]);
        assert_eq!(block_value(&v, 100, 0, false), 0);
    }

    #[test]
    fn geometry() {
        let g = ArealGeometry::new(3, 4, 32, 16);
        assert_eq!(g.pixel_size(), [96, 32]);
        assert_eq!(g.cell_to_pixel(2, 1), (64, 16));
        assert_eq!(g.pixel_to_cell(47.0, 8.0), (1, 1));
        assert_eq!(g.pixel_to_cell(48.0, 7.0), (2, 0));
        let g1 = ArealGeometry::new(1, 1, 32, 16);
        assert_eq!(g1.pixel_size(), [32, 16]);
        let g0 = ArealGeometry::new(1, 0, 32, 16);
        assert_eq!(g0.pixel_size(), [0, 16]);
    }

    fn grid(rows: usize, cols: usize, f: impl Fn(usize, usize) -> Option<i64>) -> CellGrid {
        let mut values = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                values.push(f(r, c));
            }
        }
        CellGrid { rows, cols, values }
    }

    #[test]
    fn raster_nearest() {
        let g = ArealGeometry::new(2, 2, 4, 4);
        let cells = grid(2, 2, |r, c| if r == 1 && c == 1 { None } else { Some((r * 2 + c) as i64) });
        let r = compute_raster(&g, &cells, false);
        assert_eq!(r.len(), 16);
        assert_eq!(r[0], 1); // value 0 -> 1
        assert_eq!(r[3], 1); // x=3 rounds to point 1, value 1
        assert_eq!(r[1], 1); // x=1 -> point 0 -> 0 -> 1
        assert_eq!(r[2 * 4 + 2], 0); // (2,2) -> point (1,1) missing -> 0
        assert_eq!(r[2 * 4], 2); // row 1 point 0
    }

    #[test]
    fn raster_interpolated() {
        let g = ArealGeometry::new(2, 2, 4, 4);
        let cells = grid(2, 2, |r, c| Some([[10, 50], [90, 130]][r][c]));
        let r = compute_raster(&g, &cells, true);
        assert_eq!(r[0], 0); // exact measurement position stays 0
        assert_eq!(r[2], 30); // halfway between 10 and 50
        assert_eq!(r[2 * 4], 50); // halfway between 10 and 90
        assert_eq!(r[2 * 4 + 2], 70);
    }

    #[test]
    fn grid_line_positions() {
        assert_eq!(grid_lines(32, 96), vec![0, 32, 64]);
        assert_eq!(grid_lines(32, 97), vec![0, 32, 64, 96]);
        assert!(grid_lines(0, 10).is_empty());
    }

    #[test]
    fn lut_index_masks() {
        assert_eq!(lut_index(255), 255);
        assert_eq!(lut_index(256), 0);
        assert_eq!(lut_index(-1), 255);
    }

    #[test]
    fn colorize_grid() {
        let mut lut = [[0u8; 4]; 256];
        lut[7] = [1, 2, 3, 255];
        let img = colorize(&[7; 16], [4, 4], &lut, Some((2, 2)));
        let n = colormap::NULL_COLOR;
        assert_eq!(img.pixels[0], Color32::from_rgb(n[0], n[1], n[2]));
        assert_eq!(img.pixels[4 + 1], Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn resample() {
        let img = ColorImage::new([2, 1], vec![Color32::RED, Color32::BLUE]);
        let r = resample_nearest(&img, [4, 2]);
        assert_eq!(r.pixels, vec![Color32::RED, Color32::RED, Color32::BLUE, Color32::BLUE, Color32::RED, Color32::RED, Color32::BLUE, Color32::BLUE]);
    }
}
