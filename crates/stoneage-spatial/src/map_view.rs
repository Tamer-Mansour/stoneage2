//! Map view: background map with the measurement area (areal depth cut) overlaid.
//!
//! Port of `MessKarte` (+ empty `$ColHeader`/`$RowHeader`) and `MessKarte$ViewArea`:
//! the overlay lives in a local coordinate system where 200 px = 50 m (4 px per metre). It is
//! placed with `AffineTransform = translate(kartePosX, kartePosY) * rotate(karteRotation + dragalpha)
//! * scale(karteScale)` and edited through three handles: `moveArea` (origin), `scaleArea`
//! (top-right corner) and `rotateArea` (bottom-right corner).

use std::f64::consts::TAU;

use egui::epaint::{Mesh, TextShape, Vertex};
use egui::{
    Align, Color32, CornerRadius, CursorIcon, FontId, Layout, Pos2, Rect, ScrollArea, Sense, Shape,
    Stroke, TextureHandle, TextureOptions, UiBuilder, Vec2,
};
use stoneage_core::{I18n, MapPlacement, Survey};

use crate::areal_view::{message_format_number, tr_or, ArealGeometry, ArealTexture};
use crate::ViewResponse;

/// `MessKarte$ViewArea.ROTATE`
pub const ROTATE: i32 = 1;
/// `MessKarte$ViewArea.MOVE`
pub const MOVE: i32 = 2;
/// `MessKarte$ViewArea.SCALE`
pub const SCALE: i32 = 3;

/// `mpx` / `mpy` – move handle around the origin.
pub const MOVE_AREA: ([i32; 4], [i32; 4]) = ([-10, 10, 10, -10], [-10, -10, 10, 10]);
/// `spx` / `spy` – scale handle at the top-right corner.
pub const SCALE_AREA: ([i32; 4], [i32; 4]) = ([190, 210, 210, 190], [-10, -10, 10, 10]);
/// `rpx` / `rpy` – rotate handle at the bottom-right corner.
pub const ROTATE_AREA: ([i32; 4], [i32; 4]) = ([190, 210, 210, 190], [190, 190, 210, 210]);

/// `kartenBreite` / `kartenHoehe` – size of the default (white) map image.
pub const DEFAULT_MAP_SIZE: [usize; 2] = [800, 600];
/// Side length of the overlay square in local pixels (= 50 m).
pub const FRAME: f64 = 200.0;
/// Local pixels per metre (200 px / 50 m).
pub const PX_PER_M: f64 = FRAME / 50.0;

// ---------------------------------------------------------------------------------------------
// Pure transform maths (unit tested)
// ---------------------------------------------------------------------------------------------

/// The overlay transform `translate(pos) * rotate(angle) * scale(s, s)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapTransform {
    pub pos_x: f64,
    pub pos_y: f64,
    pub angle: f64,
    pub scale: f64,
}

impl MapTransform {
    /// Transform currently shown (`karteRotation + dragalpha`).
    pub fn new(map: &MapPlacement, drag_alpha: f64) -> Self {
        Self { pos_x: map.pos_x as f64, pos_y: map.pos_y as f64, angle: map.rotation + drag_alpha, scale: map.scale }
    }

    /// Local overlay coordinates -> view coordinates.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let (s, c) = self.angle.sin_cos();
        let (x, y) = (x * self.scale, y * self.scale);
        (self.pos_x + c * x - s * y, self.pos_y + s * x + c * y)
    }

    /// View coordinates -> local overlay coordinates (`createInverse`); `None` if not invertible.
    pub fn inverse(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if self.scale == 0.0 || !self.scale.is_finite() {
            return None;
        }
        let (s, c) = self.angle.sin_cos();
        let (dx, dy) = (x - self.pos_x, y - self.pos_y);
        Some(((c * dx + s * dy) / self.scale, (-s * dx + c * dy) / self.scale))
    }
}

/// `java.awt.Point.setLocation(double, double)`: rounds to the nearest int.
pub fn to_awt_point(x: f64, y: f64) -> (i32, i32) {
    ((x + 0.5).floor() as i32, (y + 0.5).floor() as i32)
}

/// `java.awt.Polygon.contains(double, double)` (even-odd rule, half-open edges).
pub fn polygon_contains(xs: &[i32], ys: &[i32], x: f64, y: f64) -> bool {
    let n = xs.len().min(ys.len());
    if n <= 2 {
        return false;
    }
    let (bx0, bx1) = (*xs[..n].iter().min().unwrap(), *xs[..n].iter().max().unwrap());
    let (by0, by1) = (*ys[..n].iter().min().unwrap(), *ys[..n].iter().max().unwrap());
    if !(x >= bx0 as f64 && y >= by0 as f64 && x < bx1 as f64 && y < by1 as f64) {
        return false;
    }
    let mut hits = 0;
    let mut lastx = xs[n - 1];
    let mut lasty = ys[n - 1];
    for i in 0..n {
        let curx = xs[i];
        let cury = ys[i];
        'edge: {
            if cury == lasty {
                break 'edge;
            }
            let leftx;
            if curx < lastx {
                if x >= lastx as f64 {
                    break 'edge;
                }
                leftx = curx;
            } else {
                if x >= curx as f64 {
                    break 'edge;
                }
                leftx = lastx;
            }
            let (test1, test2);
            if cury < lasty {
                if y < cury as f64 || y >= lasty as f64 {
                    break 'edge;
                }
                if x < leftx as f64 {
                    hits += 1;
                    break 'edge;
                }
                test1 = x - curx as f64;
                test2 = y - cury as f64;
            } else {
                if y < lasty as f64 || y >= cury as f64 {
                    break 'edge;
                }
                if x < leftx as f64 {
                    hits += 1;
                    break 'edge;
                }
                test1 = x - lastx as f64;
                test2 = y - lasty as f64;
            }
            if test1 < test2 / (lasty - cury) as f64 * (lastx - curx) as f64 {
                hits += 1;
            }
        }
        lastx = curx;
        lasty = cury;
    }
    hits & 1 != 0
}

/// `mousePressed`: which handle (`MOVE`/`SCALE`/`ROTATE`, 0 = none) is under the view point.
/// Checked in the original order; a later match wins.
pub fn hit_handle(t: &MapTransform, x: f64, y: f64) -> i32 {
    let Some((lx, ly)) = t.inverse(x, y) else { return 0 };
    let (px, py) = to_awt_point(lx, ly);
    let (px, py) = (px as f64, py as f64);
    let mut movement = 0;
    if polygon_contains(&MOVE_AREA.0, &MOVE_AREA.1, px, py) {
        movement = MOVE;
    }
    if polygon_contains(&SCALE_AREA.0, &SCALE_AREA.1, px, py) {
        movement = SCALE;
    }
    if polygon_contains(&ROTATE_AREA.0, &ROTATE_AREA.1, px, py) {
        movement = ROTATE;
    }
    movement
}

/// `mouseDragged` for `ROTATE`: the angle between (start - pos) and (curr - pos), in `[0, 2pi)`.
/// `None` when either vector is (almost) zero. The `acos` argument is clamped to `[-1, 1]`
/// (the original could produce NaN from rounding).
pub fn drag_rotation(start: (f64, f64), curr: (f64, f64), pos: (f64, f64)) -> Option<f64> {
    let (x1, y1) = (start.0 - pos.0, start.1 - pos.1);
    let (x2, y2) = (curr.0 - pos.0, curr.1 - pos.1);
    let l1 = (x1 * x1 + y1 * y1).sqrt();
    let l2 = (x2 * x2 + y2 * y2).sqrt();
    if !(0.01 < l1 * l2) {
        return None;
    }
    let mut a = ((x1 * x2 + y1 * y2) / (l1 * l2)).clamp(-1.0, 1.0).acos();
    if x1 * y2 - y1 * x2 < 0.0 {
        a = TAU - a;
    }
    Some(a)
}

/// `mouseDragged` for `SCALE`: distance from the origin / 200, accepted only inside (0.25, 4).
pub fn drag_scale(curr: (f64, f64), pos: (f64, f64)) -> Option<f64> {
    let (x2, y2) = (curr.0 - pos.0, curr.1 - pos.1);
    let s = (x2 * x2 + y2 * y2).sqrt() / FRAME;
    (s > 0.25 && s < 4.0).then_some(s)
}

/// `mouseDragged` for `MOVE`: the new origin, each coordinate accepted only inside the view.
pub fn drag_move(map: &mut MapPlacement, curr: (i32, i32), view_w: i32, view_h: i32) {
    if curr.0 > 0 && curr.0 < view_w {
        map.pos_x = curr.0;
    }
    if curr.1 > 0 && curr.1 < view_h {
        map.pos_y = curr.1;
    }
}

/// `mouseReleased`: `karteRotation = (karteRotation + dragalpha) % 2pi`.
pub fn commit_rotation(rotation: f64, drag_alpha: f64) -> f64 {
    (rotation + drag_alpha) % TAU
}

/// Grid line positions of the overlay (`round(i * step)` while `<= 200`, as in the original loop).
pub fn overlay_grid(step: f64) -> Vec<f64> {
    let mut out = Vec::new();
    if step.is_nan() || step <= 0.0 || !step.is_finite() {
        return out;
    }
    let mut i = 0i64;
    let mut v = 0i64;
    while v <= FRAME as i64 {
        if out.last() != Some(&(v as f64)) {
            out.push(v as f64);
        }
        v = (i as f64 * step + 0.5).floor() as i64;
        i += 1;
    }
    out
}

/// Size of the areal image in local overlay pixels: `4 * max((maxPoints-1)*pd, pd)` by
/// `4 * max((rows-1)*rd, rd)` (the image of `MessAreal$ViewArea` scaled by `4*w_m / width`).
pub fn overlay_extent(max_points: usize, rows: usize, point_distance: f64, row_distance: f64) -> (f64, f64) {
    let mut w = (max_points as f64 - 1.0) * point_distance;
    let mut h = (rows as f64 - 1.0) * row_distance;
    if w < point_distance {
        w = point_distance;
    }
    if h < row_distance {
        h = row_distance;
    }
    (PX_PER_M * w, PX_PER_M * h)
}

// ---------------------------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct DragState {
    start: (f64, f64),
    drag_alpha: f64,
    movement: i32,
}

#[derive(Default)]
pub struct MapView {
    map_tex: Option<TextureHandle>,
    map_key: Option<(usize, u32, u32, u64)>,
    areal: ArealTexture,
    drag: DragState,
}

fn image_key(img: &image::RgbaImage) -> (usize, u32, u32, u64) {
    let raw = img.as_raw();
    // Cheap fingerprint: buffer address, size and a sparse sample of the bytes.
    let mut h: u64 = 0xcbf29ce484222325;
    let step = (raw.len() / 4096).max(1);
    for b in raw.iter().step_by(step) {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    (raw.as_ptr() as usize, img.width(), img.height(), h)
}

fn handle_colors(kind: i32) -> Color32 {
    match kind {
        MOVE => Color32::from_rgb(0, 200, 0),
        ROTATE => Color32::from_rgb(40, 110, 255),
        _ => Color32::from_rgb(230, 40, 40),
    }
}

impl MapView {
    /// Shows `survey.map_image` (if any) with the measurement area overlay.
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &mut Survey, i18n: &I18n) -> ViewResponse {
        let mut resp = ViewResponse::default();
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let visuals = ui.visuals().clone();
        ui.painter_at(full).rect_filled(full, 0.0, visuals.extreme_bg_color);

        // Map texture (re-uploaded only when the image changes).
        let map_size = match &survey.map_image {
            Some(img) if img.width() > 0 && img.height() > 0 => {
                let key = image_key(img);
                if self.map_key != Some(key) || self.map_tex.is_none() {
                    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                    match &mut self.map_tex {
                        Some(t) => t.set(ci, TextureOptions::LINEAR),
                        None => self.map_tex = Some(ui.ctx().load_texture("stoneage_map", ci, TextureOptions::LINEAR)),
                    }
                    self.map_key = Some(key);
                }
                [img.width() as usize, img.height() as usize]
            }
            _ => {
                self.map_tex = None;
                self.map_key = None;
                DEFAULT_MAP_SIZE
            }
        };
        let has_map = self.map_tex.is_some();
        let areal_tex = self.areal.update(ui.ctx(), survey, true, "stoneage_map_areal").map(|t| t.id());

        let mut child = ui.new_child(UiBuilder::new().max_rect(full).layout(Layout::top_down(Align::Min)));
        let out = ScrollArea::both()
            .id_salt("stoneage_map_scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                let avail = ui.available_size();
                let size = Vec2::new((map_size[0] as f32).max(avail.x), (map_size[1] as f32).max(avail.y));
                ui.allocate_exact_size(size, Sense::click_and_drag())
            });
        let (rect, r) = out.inner;
        let view_rect = out.inner_rect;
        let p = ui.painter_at(view_rect);
        let origin = rect.min.round();
        let to_screen = |(x, y): (f64, f64)| origin + Vec2::new(x as f32, y as f32);
        let to_view = |q: Pos2| ((q.x - origin.x) as f64, (q.y - origin.y) as f64);

        // --- interaction (MessKarte$ViewArea mouse listeners) ---
        let view_w = rect.width() as i32;
        let view_h = rect.height() as i32;
        if r.drag_started() {
            let start = ui.input(|i| i.pointer.press_origin()).or(r.interact_pointer_pos()).map(to_view);
            if let Some(start) = start {
                let start = (start.0.round(), start.1.round());
                self.drag.start = start;
                self.drag.drag_alpha = 0.0;
                self.drag.movement = hit_handle(&MapTransform::new(&survey.map, 0.0), start.0, start.1);
            }
        }
        if r.dragged() && self.drag.movement != 0 {
            if let Some(cur) = r.interact_pointer_pos().map(to_view) {
                let cur = (cur.0.round(), cur.1.round());
                let pos = (survey.map.pos_x as f64, survey.map.pos_y as f64);
                match self.drag.movement {
                    ROTATE => {
                        if let Some(a) = drag_rotation(self.drag.start, cur, pos) {
                            self.drag.drag_alpha = a;
                        }
                    }
                    SCALE => {
                        if let Some(s) = drag_scale(cur, pos) {
                            if s != survey.map.scale {
                                survey.map.scale = s;
                                resp.changed = true;
                            }
                        }
                    }
                    MOVE => {
                        let before = (survey.map.pos_x, survey.map.pos_y);
                        drag_move(&mut survey.map, (cur.0 as i32, cur.1 as i32), view_w, view_h);
                        resp.changed |= before != (survey.map.pos_x, survey.map.pos_y);
                    }
                    _ => {}
                }
            }
        }
        if r.drag_stopped() {
            if self.drag.drag_alpha != 0.0 {
                survey.map.rotation = commit_rotation(survey.map.rotation, self.drag.drag_alpha);
                resp.changed = true;
            }
            self.drag.drag_alpha = 0.0;
            self.drag.movement = 0;
        }

        let t = MapTransform::new(&survey.map, self.drag.drag_alpha);
        let local = |x: f64, y: f64| to_screen(t.apply(x, y));

        // --- map image (or the original's white 800x600 canvas) ---
        let map_rect = Rect::from_min_size(origin, Vec2::new(map_size[0] as f32, map_size[1] as f32));
        match &self.map_tex {
            Some(tex) => {
                p.image(tex.id(), map_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                p.rect_filled(map_rect, CornerRadius::same(4), Color32::WHITE);
            }
        }

        // --- overlay in local coordinates ---
        let quad = |x0: f64, y0: f64, x1: f64, y1: f64| vec![local(x0, y0), local(x1, y0), local(x1, y1), local(x0, y1)];
        let grid_col = Color32::from_rgba_unmultiplied(0, 255, 0, 51);
        let gx = overlay_grid(FRAME * survey.point_distance / 50.0);
        let gz = overlay_grid(FRAME * survey.row_distance / 50.0);
        for &x in &gx {
            p.line_segment([local(x, 0.0), local(x, FRAME)], Stroke::new(1.0, grid_col));
        }
        for &z in &gz {
            p.line_segment([local(0.0, z), local(FRAME, z)], Stroke::new(1.0, grid_col));
        }

        // Areal depth-cut image.
        let geom = ArealGeometry::from_survey(survey);
        if let Some(id) = areal_tex {
            if geom.pref_width() > 0 && geom.pref_height() > 0 {
                let (ew, eh) = overlay_extent(geom.max_points, geom.rows, survey.point_distance, survey.row_distance);
                let mut mesh = Mesh::with_texture(id);
                let corners = [(0.0, 0.0, 0.0, 0.0), (ew, 0.0, 1.0, 0.0), (ew, eh, 1.0, 1.0), (0.0, eh, 0.0, 1.0)];
                for (x, y, u, v) in corners {
                    mesh.vertices.push(Vertex { pos: local(x, y), uv: Pos2::new(u, v), color: Color32::WHITE });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                p.add(Shape::mesh(mesh));
            }
        }

        // Frames (red): drawRect(-10,-10,220,220) and drawRect(0,0,200,200), label "50 m".
        let red = Color32::from_rgb(220, 30, 30);
        p.add(Shape::closed_line(quad(-10.0, -10.0, 210.0, 210.0), Stroke::new(1.0, red.gamma_multiply(0.6))));
        p.add(Shape::closed_line(quad(0.0, 0.0, FRAME, FRAME), Stroke::new(1.5, red)));
        {
            let size = (12.0 * t.scale as f32).clamp(8.0, 28.0);
            let galley = p.layout_no_wrap("50 m".to_string(), FontId::proportional(size), red);
            let pos = local(90.0, -(size as f64) * 1.05);
            p.add(TextShape::new(pos, galley, red).with_angle(t.angle as f32));
        }

        // Handles (fill alpha 0.2 like the original, plus a crisp outline and hover highlight).
        let hover_kind = r
            .hover_pos()
            .map(to_view)
            .map(|(x, y)| hit_handle(&MapTransform::new(&survey.map, 0.0), x.round(), y.round()))
            .unwrap_or(0);
        for (kind, (xs, ys)) in [(MOVE, MOVE_AREA), (ROTATE, ROTATE_AREA), (SCALE, SCALE_AREA)] {
            let pts: Vec<Pos2> = (0..4).map(|i| local(xs[i] as f64, ys[i] as f64)).collect();
            let c = handle_colors(kind);
            let active = self.drag.movement == kind || (self.drag.movement == 0 && hover_kind == kind);
            let fill = if active { c.gamma_multiply(0.45) } else { c.gamma_multiply(0.2) };
            p.add(Shape::convex_polygon(pts.clone(), fill, Stroke::new(if active { 2.0 } else { 1.2 }, c)));
            // Small glyph in the handle centre.
            let centre = local((xs[0] + xs[1]) as f64 / 2.0, (ys[0] + ys[2]) as f64 / 2.0);
            p.circle_filled(centre, 2.0, c);
        }

        let cursor_kind = if self.drag.movement != 0 { self.drag.movement } else { hover_kind };
        match cursor_kind {
            MOVE => ui.ctx().set_cursor_icon(if self.drag.movement != 0 { CursorIcon::Grabbing } else { CursorIcon::Move }),
            SCALE => ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw),
            ROTATE => ui.ctx().set_cursor_icon(if self.drag.movement != 0 { CursorIcon::Grabbing } else { CursorIcon::Alias }),
            _ => {}
        }

        // Hover read-out: position inside the measurement area in metres.
        if let Some(hp) = r.hover_pos() {
            let (vx, vy) = to_view(hp);
            if let Some((lx, ly)) = t.inverse(vx, vy) {
                let text = match hover_kind {
                    MOVE => tr_or(i18n, "App.Map.HandleMove", "Drag to move the measurement area", &[]),
                    SCALE => tr_or(i18n, "App.Map.HandleScale", "Drag to scale the measurement area", &[]),
                    ROTATE => tr_or(i18n, "App.Map.HandleRotate", "Drag to rotate the measurement area", &[]),
                    _ => tr_or(
                        i18n,
                        "App.Map.Hover",
                        "x = {0} m, y = {1} m",
                        &[&format!("{:.2}", lx / PX_PER_M), &format!("{:.2}", ly / PX_PER_M)],
                    ),
                };
                resp.hover_info = Some(text);
            }
        }

        // Caption (`MessKarte.strHorizontalSchnitt` at (10,10)), drawn as a pill.
        let depth = survey.depth(survey.horizontal_cut.max(0) as usize);
        let caption = tr_or(
            i18n,
            "MessKarte.strHorizontalSchnitt",
            "Depth-cut at {0} m",
            &[&message_format_number(depth, i18n.lang)],
        );
        let cap_font = FontId::proportional(13.0);
        let galley = p.layout_no_wrap(caption, cap_font, Color32::from_gray(20));
        let cap_pos = view_rect.min + Vec2::new(10.0, 10.0);
        let bg = Rect::from_min_size(cap_pos, galley.size() + Vec2::new(16.0, 8.0));
        p.rect_filled(bg, CornerRadius::same(6), Color32::from_white_alpha(225));
        p.galley(cap_pos + Vec2::new(8.0, 4.0), galley, Color32::from_gray(20));

        // Empty state: no map loaded.
        if !has_map {
            let hint = tr_or(
                i18n,
                "App.Map.NoMap",
                "No map loaded \u{2013} use {0} \u{2192} {1} to show a background map.",
                &[
                    &tr_or(i18n, "MainFrame.Menu.File.Caption", "File", &[]),
                    &tr_or(i18n, "MainFrame.Menu.File.LoadMap.Caption", "Load map", &[]),
                ],
            );
            let g = p.layout_no_wrap(hint, FontId::proportional(13.0), visuals.text_color());
            let pos = Pos2::new(view_rect.center().x - g.size().x / 2.0, view_rect.bottom() - g.size().y - 22.0);
            let bg = Rect::from_min_size(pos - Vec2::new(12.0, 7.0), g.size() + Vec2::new(24.0, 14.0));
            p.rect_filled(bg, CornerRadius::same(8), visuals.window_fill);
            p.rect_stroke(bg, CornerRadius::same(8), visuals.window_stroke, egui::StrokeKind::Inside);
            p.galley(pos, g, visuals.text_color());
        }

        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn transform_roundtrip() {
        let t = MapTransform { pos_x: 50.0, pos_y: 25.0, angle: 0.7, scale: 1.5 };
        let v = t.apply(200.0, 0.0);
        assert!(close(t.inverse(v.0, v.1).unwrap(), (200.0, 0.0)));
        let id = MapTransform { pos_x: 50.0, pos_y: 25.0, angle: 0.0, scale: 1.0 };
        assert!(close(id.apply(200.0, 200.0), (250.0, 225.0)));
        // 90 degrees clockwise on screen (y down): local x axis points down.
        let r = MapTransform { pos_x: 0.0, pos_y: 0.0, angle: std::f64::consts::FRAC_PI_2, scale: 2.0 };
        assert!(close(r.apply(10.0, 0.0), (0.0, 20.0)));
        assert!(MapTransform { scale: 0.0, ..t }.inverse(1.0, 1.0).is_none());
    }

    #[test]
    fn polygon_half_open() {
        let (xs, ys) = MOVE_AREA;
        assert!(polygon_contains(&xs, &ys, -10.0, -10.0));
        assert!(polygon_contains(&xs, &ys, 0.0, 0.0));
        assert!(polygon_contains(&xs, &ys, 9.0, 9.0));
        assert!(!polygon_contains(&xs, &ys, 10.0, 0.0));
        assert!(!polygon_contains(&xs, &ys, 0.0, 10.0));
        let tri = ([0, 10, 0], [0, 0, 10]);
        assert!(polygon_contains(&tri.0, &tri.1, 2.0, 2.0));
        assert!(!polygon_contains(&tri.0, &tri.1, 8.0, 8.0));
    }

    #[test]
    fn handles_hit() {
        let t = MapTransform { pos_x: 50.0, pos_y: 25.0, angle: 0.0, scale: 1.0 };
        assert_eq!(hit_handle(&t, 50.0, 25.0), MOVE);
        assert_eq!(hit_handle(&t, 250.0, 25.0), SCALE);
        assert_eq!(hit_handle(&t, 250.0, 225.0), ROTATE);
        assert_eq!(hit_handle(&t, 150.0, 125.0), 0);
        // rotated by 90 deg and scaled 0.5: local (200,0) -> view (50, 125)
        let t = MapTransform { pos_x: 50.0, pos_y: 25.0, angle: std::f64::consts::FRAC_PI_2, scale: 0.5 };
        assert_eq!(hit_handle(&t, 50.0, 125.0), SCALE);
        // local (200,200) -> view (-50, 125)
        assert_eq!(hit_handle(&t, -50.0, 125.0), ROTATE);
    }

    #[test]
    fn rotation_drag() {
        let pos = (100.0, 100.0);
        let a = drag_rotation((200.0, 100.0), (100.0, 200.0), pos).unwrap();
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let a = drag_rotation((200.0, 100.0), (100.0, 0.0), pos).unwrap();
        assert!((a - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(drag_rotation((100.0, 100.0), (150.0, 100.0), pos).is_none());
        let a = drag_rotation((200.0, 100.0), (300.0, 100.0), pos).unwrap();
        assert!(a.abs() < 1e-12);
        assert!((commit_rotation(6.0, 1.0) - (7.0 - TAU)).abs() < 1e-12);
        assert!((commit_rotation(-7.0, 0.0) - (-7.0 + TAU)).abs() < 1e-12);
    }

    #[test]
    fn scale_drag() {
        assert_eq!(drag_scale((300.0, 0.0), (0.0, 0.0)), Some(1.5));
        assert_eq!(drag_scale((50.0, 0.0), (0.0, 0.0)), None);
        assert_eq!(drag_scale((800.0, 0.0), (0.0, 0.0)), None);
    }

    #[test]
    fn move_drag() {
        let mut m = MapPlacement { rotation: 0.0, scale: 1.0, pos_x: 50, pos_y: 25 };
        drag_move(&mut m, (300, 200), 800, 600);
        assert_eq!((m.pos_x, m.pos_y), (300, 200));
        drag_move(&mut m, (0, 700), 800, 600);
        assert_eq!((m.pos_x, m.pos_y), (300, 200));
        drag_move(&mut m, (799, 1), 800, 600);
        assert_eq!((m.pos_x, m.pos_y), (799, 1));
    }

    #[test]
    fn grid_and_extent() {
        assert_eq!(overlay_grid(20.0), (0..=10).map(|i| i as f64 * 20.0).collect::<Vec<_>>());
        assert_eq!(overlay_grid(30.0).last(), Some(&180.0));
        assert!(overlay_grid(0.0).is_empty());
        assert_eq!(overlay_extent(11, 5, 5.0, 2.5), (200.0, 40.0));
        assert_eq!(overlay_extent(1, 1, 5.0, 2.5), (20.0, 10.0));
    }
}
