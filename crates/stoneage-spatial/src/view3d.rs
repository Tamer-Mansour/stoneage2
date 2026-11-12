//! 3-D volume view. Port of `StoneAge.Visual3d` + `StoneAge.ShapedTexture3D` without Java3D.
//!
//! The survey is sampled into a voxel volume exactly like `Visual3d.createSceneGraph` / `getSampleValue`
//! (x = rows, y = samples, z = points in the original naming), coloured with the areal colour map and packed
//! to RGBA like `ShapedTexture3D.clamp`. Instead of a hardware `Texture3D` with texgen planes, every slice quad
//! of `generateGeometryX/Y/Z` is pre-computed into a 2-D RGBA texture. Each frame only the slice stack whose
//! axis is most aligned with the view direction is drawn, back to front, through a software perspective
//! projection (classic 2-D texture-stack volume rendering).
//!
//! Geometry axes on screen: x = points, y = samples (frequency / depth, index 0 at the top), z = rows.

mod camera;
mod volume;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};

use egui::{
    Align2, Color32, ColorImage, FontId, Mesh, PointerButton, Pos2, Rect, Sense, Shape, Slider, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use stoneage_core::{colormap, I18n, Survey};

use crate::ViewResponse;
use camera::{back_to_front, choose_axis, focal_length, Camera};
use volume::{build_volume, sample_value, slice_positions, slice_tint, Axis, Dims, Volume};

/// Java3D's default (black) background.
const BACKGROUND: Color32 = Color32::from_rgb(8, 9, 12);
/// Grid subdivisions per slice quad (keeps the affine texture mapping close to perspective-correct).
const SUBDIV: usize = 6;
/// Default value of the opacity slider (1.0 = per-axis opacity of the original 4×4×4 stack).
const DEFAULT_OPACITY: f32 = 1.0;
const AXIS_COLORS: [Color32; 3] = [
    Color32::from_rgb(232, 96, 84),
    Color32::from_rgb(110, 200, 120),
    Color32::from_rgb(96, 150, 240),
];

/// One stack of slice textures (the quads of `generateGeometryX`, `Y` or `Z`).
struct SliceStack {
    /// Geometry coordinate (0..1) of each slice.
    positions: Vec<f32>,
    textures: Vec<TextureHandle>,
}

struct ValidScene {
    volume: Volume,
    /// Lazily built stacks, indexed by `Axis::index`.
    stacks: [Option<SliceStack>; 3],
}

enum Scene {
    /// `createInvalidSceneGraph`: a `ColorCube`.
    Invalid,
    /// `createSceneGraph`: the textured volume.
    Valid(Box<ValidScene>),
}

pub struct View3d {
    camera: Camera,
    opacity: f32,
    scene: Scene,
    /// Fingerprint of the survey data the current scene was built from.
    built_from: Option<u64>,
    /// `recreate()` was called: rebuild on the next frame even if the data looks unchanged.
    force_rebuild: bool,
    /// `invalidate()` was called: show the invalid scene until `recreate()`.
    invalidated: bool,
}

impl Default for View3d {
    fn default() -> Self {
        View3d {
            camera: Camera::default(),
            opacity: DEFAULT_OPACITY,
            scene: Scene::Invalid,
            built_from: None,
            force_rebuild: false,
            invalidated: false,
        }
    }
}

impl View3d {
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &Survey, i18n: &I18n) -> ViewResponse {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        let painter = painter.with_clip_rect(rect);
        let focal = focal_length(rect.size());

        // --- mouse: left orbit, right/middle pan, wheel zoom, double-click reset -------------------------
        if response.double_clicked() {
            self.camera = Camera::default();
        } else if response.dragged_by(PointerButton::Primary) {
            let d = response.drag_delta();
            self.camera.orbit(d.x, d.y);
        } else if response.dragged_by(PointerButton::Secondary) || response.dragged_by(PointerButton::Middle) {
            self.camera.pan_pixels(response.drag_delta(), focal);
        }
        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 {
                self.camera.zoom((-scroll * 0.002).exp());
            }
            if zoom != 1.0 {
                self.camera.zoom(1.0 / zoom);
            }
        }

        self.update_scene(survey);

        // --- scene ------------------------------------------------------------------------------------
        painter.rect_filled(rect, 0.0, BACKGROUND);
        let center = rect.center();
        let ctx = ui.ctx().clone();
        let mut slice_info = None;
        match &mut self.scene {
            Scene::Invalid => paint_color_cube(&painter, &self.camera, center, focal),
            Scene::Valid(scene) => {
                paint_box(&painter, &self.camera, center, focal);
                slice_info = Some(paint_volume(&ctx, &painter, scene, &self.camera, self.opacity, center, focal));
            }
        }

        // --- overlay ----------------------------------------------------------------------------------
        let text_color = Color32::from_gray(225);
        let weak = Color32::from_gray(150);
        let small = FontId::proportional(12.0);
        let y_label = if survey.show_depth { "App.View3d.AxisDepth" } else { "App.View3d.AxisFrequency" };
        let axis_names = [
            i18n.tr("App.View3d.AxisPoints"),
            i18n.tr(y_label),
            i18n.tr("App.View3d.AxisRows"),
        ];

        match slice_info {
            Some((axis, count)) => {
                let text = format!(
                    "{}: {} · {} ({})",
                    i18n.tr("App.View3d.SliceAxis"),
                    axis.letter(),
                    axis_names[axis.index()],
                    count
                );
                overlay_label(&painter, rect.left_top() + Vec2::new(12.0, 12.0), Align2::LEFT_TOP, &text, text_color);
            }
            None => {
                overlay_label(
                    &painter,
                    rect.center_top() + Vec2::new(0.0, 12.0),
                    Align2::CENTER_TOP,
                    &i18n.tr("App.View3d.NoData"),
                    text_color,
                );
            }
        }

        paint_gizmo(&painter, &self.camera, rect.left_bottom() + Vec2::new(56.0, -56.0), &axis_names, text_color);

        painter.text(
            rect.right_bottom() + Vec2::new(-12.0, -10.0),
            Align2::RIGHT_BOTTOM,
            i18n.tr("App.View3d.Hint"),
            small.clone(),
            weak,
        );

        if matches!(self.scene, Scene::Valid(_)) {
            let panel = Rect::from_min_size(rect.right_top() + Vec2::new(-236.0, 8.0), Vec2::new(228.0, 30.0));
            if rect.contains_rect(panel) {
                painter.rect_filled(panel, 8.0, Color32::from_black_alpha(170));
                painter.text(
                    panel.left_center() + Vec2::new(10.0, 0.0),
                    Align2::LEFT_CENTER,
                    i18n.tr("App.View3d.Opacity"),
                    small,
                    text_color,
                );
                let slider_rect = Rect::from_min_max(panel.min + Vec2::new(78.0, 4.0), panel.max - Vec2::new(8.0, 4.0));
                ui.put(
                    slider_rect,
                    Slider::new(&mut self.opacity, 0.05..=2.0)
                        .show_value(true)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
            }
        }

        ViewResponse::default()
    }

    /// Menu "Recreate scenegraph" (`Visual3d.recreateScenegraph`).
    pub fn recreate(&mut self) {
        // The original detaches the scene and builds a fresh one, which also resets the rotation.
        self.invalidated = false;
        self.force_rebuild = true;
        self.camera = Camera::default();
    }

    /// `Visual3d.invalidScenegraph`: replace the volume by the placeholder scene until `recreate()`.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
        self.scene = Scene::Invalid;
        self.built_from = None;
        self.camera = Camera::default();
    }

    /// True while the textured volume (not the placeholder cube) is shown.
    pub fn is_valid(&self) -> bool {
        matches!(self.scene, Scene::Valid(_))
    }

    fn update_scene(&mut self, survey: &Survey) {
        if self.invalidated {
            return;
        }
        let fp = fingerprint(survey);
        if !self.force_rebuild && self.built_from == Some(fp) {
            return;
        }
        self.force_rebuild = false;
        self.built_from = Some(fp);
        self.scene = match create_volume(survey) {
            Some(volume) => Scene::Valid(Box::new(ValidScene { volume, stacks: [None, None, None] })),
            None => Scene::Invalid,
        };
    }
}

/// `createSceneGraph` data part. `None` when there is nothing to show or the data could not be sampled.
fn create_volume(survey: &Survey) -> Option<Volume> {
    if !survey.rows.iter().any(|r| !r.points.is_empty()) {
        return None;
    }
    // Like the original (`try { ... } catch (Exception e)` in `recreateScenegraph`), a failure while sampling
    // leaves the view without a volume instead of taking the application down.
    catch_unwind(AssertUnwindSafe(|| {
        let dims = Dims::for_survey(survey.rows.len(), survey.max_points());
        let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
        build_volume(dims, &lut, |x, y, z| sample_value(survey, dims, x, y, z))
    }))
    .ok()
}

/// Cheap change detector for everything the volume depends on.
fn fingerprint(survey: &Survey) -> u64 {
    let mut h = DefaultHasher::new();
    survey.color_model.hash(&mut h);
    survey.color_model_base.hash(&mut h);
    survey.reverse_color.hash(&mut h);
    survey.block_size.hash(&mut h);
    survey.use_block_average.hash(&mut h);
    survey.rows.len().hash(&mut h);
    for row in &survey.rows {
        row.points.len().hash(&mut h);
        for p in &row.points {
            p.acquired.hash(&mut h);
            p.spectrum_is_source.hash(&mut h);
            p.time_stamp.hash(&mut h);
            p.values.len().hash(&mut h);
            for v in p.values.iter().step_by(128) {
                v.hash(&mut h);
            }
            p.values.last().hash(&mut h);
        }
    }
    h.finish()
}

fn overlay_label(painter: &egui::Painter, pos: Pos2, align: Align2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(13.0), color);
    let r = align.anchor_size(pos, galley.size()).expand2(Vec2::new(8.0, 4.0));
    painter.rect_filled(r, 6.0, Color32::from_black_alpha(160));
    painter.galley(r.min + Vec2::new(8.0, 4.0), galley, color);
}

/// Draws the slice stack most aligned with the view direction. Returns the slicing axis and slice count.
fn paint_volume(
    ctx: &egui::Context,
    painter: &egui::Painter,
    scene: &mut ValidScene,
    cam: &Camera,
    opacity: f32,
    center: Pos2,
    focal: f32,
) -> (Axis, usize) {
    let eye = cam.eye_object();
    let axis = Axis::from_index(choose_axis(eye));
    let volume = &scene.volume;
    let stack = scene.stacks[axis.index()].get_or_insert_with(|| build_stack(ctx, volume, axis));

    let object_pos: Vec<f32> = stack.positions.iter().map(|f| f - 0.5).collect();
    let tint = slice_tint(opacity, stack.positions.len());
    let t = (tint * 255.0).round() as u8;
    let tint = Color32::from_rgba_premultiplied(t, t, t, t);
    for i in back_to_front(&object_pos, eye[axis.index()]) {
        let mesh = slice_mesh(cam, axis, stack.positions[i], stack.textures[i].id(), tint, center, focal);
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
    (axis, stack.positions.len())
}

fn build_stack(ctx: &egui::Context, volume: &Volume, axis: Axis) -> SliceStack {
    let positions = slice_positions(volume.geom_len(axis));
    let textures = positions
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let (size, rgba) = volume.slice_image(axis, f);
            ctx.load_texture(
                format!("view3d-{}-{i}", axis.letter()),
                ColorImage::from_rgba_unmultiplied(size, &rgba),
                TextureOptions::LINEAR,
            )
        })
        .collect();
    SliceStack { positions, textures }
}

/// One textured quad of `generateGeometryX/Y/Z` at geometry coordinate `f`, subdivided into a grid.
fn slice_mesh(
    cam: &Camera,
    axis: Axis,
    f: f32,
    texture: egui::TextureId,
    tint: Color32,
    center: Pos2,
    focal: f32,
) -> Mesh {
    let (ua, va) = axis.plane_axes();
    let mut mesh = Mesh::with_texture(texture);
    let mut ok = Vec::with_capacity((SUBDIV + 1) * (SUBDIV + 1));
    for j in 0..=SUBDIV {
        for i in 0..=SUBDIV {
            let gu = i as f32 / SUBDIV as f32;
            let gv = j as f32 / SUBDIV as f32;
            let mut g = [0.0f32; 3];
            g[axis.index()] = f;
            g[ua.index()] = gu;
            g[va.index()] = gv;
            let p = cam.project([g[0] - 0.5, g[1] - 0.5, g[2] - 0.5], center, focal);
            ok.push(p.is_some());
            mesh.vertices.push(egui::epaint::Vertex {
                pos: p.unwrap_or(center),
                uv: Pos2::new(ua.tex_coord(gu), va.tex_coord(gv)),
                color: tint,
            });
        }
    }
    let w = SUBDIV + 1;
    for j in 0..SUBDIV {
        for i in 0..SUBDIV {
            let a = j * w + i;
            let (b, c, d) = (a + 1, a + w, a + w + 1);
            if ok[a] && ok[b] && ok[c] && ok[d] {
                mesh.add_triangle(a as u32, b as u32, d as u32);
                mesh.add_triangle(a as u32, d as u32, c as u32);
            }
        }
    }
    if mesh.indices.is_empty() {
        mesh.vertices.clear();
    }
    mesh
}

/// Faint outline of the volume cube (orientation aid; not in the original).
fn paint_box(painter: &egui::Painter, cam: &Camera, center: Pos2, focal: f32) {
    let stroke = Stroke::new(1.0, Color32::from_white_alpha(40));
    for a in 0..3 {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        for k in 0..4 {
            let mut p0 = [-0.5f32; 3];
            p0[b] = if k & 1 == 0 { -0.5 } else { 0.5 };
            p0[c] = if k & 2 == 0 { -0.5 } else { 0.5 };
            let mut p1 = p0;
            p1[a] = 0.5;
            if let (Some(s0), Some(s1)) = (cam.project(p0, center, focal), cam.project(p1, center, focal)) {
                painter.line_segment([s0, s1], stroke);
            }
        }
    }
}

/// `createInvalidSceneGraph`: Java3D `ColorCube` (±1) translated by (-0.5, -0.5, -0.5).
fn paint_color_cube(painter: &egui::Painter, cam: &Camera, center: Pos2, focal: f32) {
    // (normal axis, sign, colour) – ColorCube: front red, back green, right blue, left yellow, top magenta, bottom cyan.
    const FACES: [(usize, f32, Color32); 6] = [
        (2, 1.0, Color32::from_rgb(255, 0, 0)),
        (2, -1.0, Color32::from_rgb(0, 255, 0)),
        (0, 1.0, Color32::from_rgb(0, 0, 255)),
        (0, -1.0, Color32::from_rgb(255, 255, 0)),
        (1, 1.0, Color32::from_rgb(255, 0, 255)),
        (1, -1.0, Color32::from_rgb(0, 255, 255)),
    ];
    const OFFSET: f32 = -0.5;
    let eye = cam.eye_object();
    let mut mesh = Mesh::default();
    for (n, sign, color) in FACES {
        let mut fc = [OFFSET; 3];
        fc[n] += sign;
        let to_eye = [eye[0] - fc[0], eye[1] - fc[1], eye[2] - fc[2]];
        if to_eye[n] * sign <= 0.0 {
            continue; // back face (ColorCube culls back faces)
        }
        let (u, v) = ((n + 1) % 3, (n + 2) % 3);
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let projected: Option<Vec<Pos2>> = corners
            .iter()
            .map(|&(a, b)| {
                let mut p = fc;
                p[u] += a;
                p[v] += b;
                cam.project(p, center, focal)
            })
            .collect();
        let Some(pts) = projected else { continue };
        let base = mesh.vertices.len() as u32;
        for p in pts {
            mesh.colored_vertex(p, color);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    if !mesh.is_empty() {
        painter.add(Shape::mesh(mesh));
    }
}

/// Axis gizmo: rotation-only projection of the data axes (x = points, y = samples downwards, z = rows).
fn paint_gizmo(painter: &egui::Painter, cam: &Camera, origin: Pos2, names: &[String; 3], color: Color32) {
    const LEN: f32 = 34.0;
    let dirs: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]];
    painter.circle_filled(origin, LEN + 14.0, Color32::from_black_alpha(110));
    let mut items: Vec<(usize, [f32; 3])> = dirs.iter().enumerate().map(|(i, d)| (i, cam.rot.apply(*d))).collect();
    items.sort_by(|a, b| a.1[2].total_cmp(&b.1[2]));
    for (i, w) in items {
        let end = origin + Vec2::new(w[0], -w[1]) * LEN;
        let c = AXIS_COLORS[i];
        painter.line_segment([origin, end], Stroke::new(2.0, c));
        painter.circle_filled(end, 3.0, c);
        let dir = Vec2::new(w[0], -w[1]);
        let label_pos = end + if dir.length() > 1e-3 { dir.normalized() * 8.0 } else { Vec2::new(8.0, 0.0) };
        let align = Align2([
            if dir.x > 0.3 {
                egui::Align::Min
            } else if dir.x < -0.3 {
                egui::Align::Max
            } else {
                egui::Align::Center
            },
            if dir.y > 0.3 {
                egui::Align::Min
            } else if dir.y < -0.3 {
                egui::Align::Max
            } else {
                egui::Align::Center
            },
        ]);
        painter.text(
            label_pos,
            align,
            format!("{} {}", Axis::from_index(i).letter(), names[i]),
            FontId::proportional(11.0),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::camera::*;
    use super::volume::*;
    use egui::{Pos2, Vec2};

    #[test]
    fn java_round_matches_math_round() {
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(1.49), 1);
        assert_eq!(java_round(-0.5), 0);
        assert_eq!(java_round(2.5), 3);
    }

    #[test]
    fn sample_indices_follow_get_sample_value() {
        // reihen.size() = 10, dimx = 4: round(10*x/4)
        let idx: Vec<i64> = (0..4).map(|x| scaled_index(10, x, 4)).collect();
        assert_eq!(idx, vec![0, 3, 5, 8]);
        // 2048*y/dimy (integer division)
        assert_eq!((0..4).map(|y| sample_index(y, 4)).collect::<Vec<_>>(), vec![0, 512, 1024, 1536]);
        assert_eq!(sample_index(1, 3), 682);
        // identity when dim == count
        assert!((0..7).all(|i| scaled_index(7, i, 7) == i as i64));
    }

    #[test]
    fn dims_respect_budget() {
        let d = Dims::for_survey(3, 5);
        assert_eq!((d.x, d.y, d.z), (3, MAX_SAMPLES, 5));
        let d = Dims::for_survey(500, 500);
        assert_eq!((d.x, d.z), (MAX_ROWS, MAX_POINTS));
        assert!(d.len() <= VOXEL_BUDGET);
        let d = Dims::for_survey(0, 0);
        assert_eq!((d.x, d.z), (1, 1));
    }

    #[test]
    fn slice_positions_match_float_loop() {
        assert_eq!(slice_positions(4), vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(slice_positions(1), vec![0.0, 1.0]);
        let p = slice_positions(10);
        assert!(p.len() == 10 || p.len() == 11);
        assert!(p.iter().all(|f| *f <= 1.0));
    }

    fn test_lut() -> [[u8; 4]; 256] {
        let mut lut = [[0u8; 4]; 256];
        for (i, c) in lut.iter_mut().enumerate() {
            *c = [i as u8, 255 - i as u8, 7, 255];
        }
        lut
    }

    #[test]
    fn build_volume_packs_like_clamp() {
        let dims = Dims { x: 2, y: 3, z: 4 };
        let v = build_volume(dims, &test_lut(), |x, y, z| (x * 100 + y * 10 + z) as f64 + 0.7);
        assert_eq!(v.rgba.len(), 24);
        // RGBA = lut[floor(sample)] with alpha 128 ...
        assert_eq!(v.rgba[dims.idx(1, 2, 3)], [123, 132, 7, 128]);
        // ... except the two special voxels of createSceneGraph.
        assert_eq!(v.rgba[dims.idx(0, 0, 0)][3], 0);
        assert_eq!(v.rgba[dims.idx(0, 0, 1)][3], 255);
        assert_eq!(v.cal_min[0], 0.0);
        assert_eq!(v.cal_max[0], 255.0);
        assert_eq!(v.cal_min[3], 7.0);
        // out-of-range samples are clamped into the colour table
        let v = build_volume(dims, &test_lut(), |_, _, _| 1000.0);
        assert_eq!(v.rgba[5][0], 255);
        let v = build_volume(dims, &test_lut(), |_, _, _| -3.0);
        assert_eq!(v.rgba[5][0], 0);
    }

    #[test]
    fn find_min_max_initial_values() {
        let (min, max) = find_min_max(&[]);
        assert_eq!(min, [f64::MAX; 4]);
        assert_eq!(max[0], 4.9e-324);
        let (_, max) = find_min_max(&[[-1.0, 0.0, 0.0, 0.0]]);
        assert_eq!(max[0], 4.9e-324); // negative values never beat Double.MIN_VALUE
    }

    #[test]
    fn to_byte_is_java_cast() {
        assert_eq!(to_byte(128.0), 128);
        assert_eq!(to_byte(255.9), 255);
        assert_eq!(to_byte(256.0), 0);
        assert_eq!(to_byte(-1.0), 255);
    }

    #[test]
    fn slice_images_have_plane_layout_and_interpolate() {
        let dims = Dims { x: 2, y: 2, z: 3 }; // rows, samples, points
        let v = build_volume(dims, &test_lut(), |x, _, _| if x == 0 { 10.0 } else { 30.0 });
        // perpendicular to Z (rows): columns = points, rows = samples
        let (size, img) = v.slice_image(Axis::Z, 0.0);
        assert_eq!(size, [3, 2]);
        assert_eq!(img[4], 10); // (col 1,row 0) red channel = row 0 colour
        let (_, img) = v.slice_image(Axis::Z, 1.0);
        assert_eq!(img[4], 30);
        let (_, img) = v.slice_image(Axis::Z, 0.5);
        assert_eq!(img[4], 20); // halfway between the two row planes
        let (size, _) = v.slice_image(Axis::X, 0.5);
        assert_eq!(size, [2, 2]); // columns = rows, image rows = samples
        let (size, _) = v.slice_image(Axis::Y, 0.5);
        assert_eq!(size, [3, 2]); // columns = points, image rows = rows
    }

    #[test]
    fn slice_tint_reproduces_original_stack() {
        // 5 slices at opacity 1 → original appearance: texel alpha 128/255 × (1 − 0.1)
        assert!((slice_tint(1.0, 5) - 0.9).abs() < 1e-5);
        // more slices → each slice more transparent
        assert!(slice_tint(1.0, 100) < slice_tint(1.0, 5));
        assert_eq!(slice_tint(0.0, 10), 0.0);
        assert!(slice_tint(2.0, 1) <= 1.0);
    }

    #[test]
    fn nominal_projection_centres_origin() {
        let cam = Camera::default();
        assert!((cam.dist - 2.4142135).abs() < 1e-5);
        let c = Pos2::new(100.0, 50.0);
        let p = cam.project([0.0, 0.0, 0.0], c, 200.0).unwrap();
        assert!((p - c).length() < 1e-4);
        // +x right, +y up on screen
        let p = cam.project([0.5, 0.5, 0.0], c, 200.0).unwrap();
        assert!(p.x > c.x && p.y < c.y);
        // nearer points appear larger
        let near = cam.project([0.5, 0.0, 0.5], c, 200.0).unwrap();
        let far = cam.project([0.5, 0.0, -0.5], c, 200.0).unwrap();
        assert!(near.x > far.x);
        // behind the eye → clipped
        let mut close = cam;
        close.dist = 0.2;
        assert!(close.project([0.0, 0.0, 0.5], c, 200.0).is_none());
    }

    #[test]
    fn focal_uses_smaller_side() {
        let f = focal_length(Vec2::new(800.0, 400.0));
        assert!((f - 200.0 / (std::f32::consts::FRAC_PI_8).tan()).abs() < 1e-3);
    }

    #[test]
    fn orbit_keeps_rotation_orthonormal_and_axis_selection() {
        let mut cam = Camera::default();
        assert_eq!(choose_axis(cam.eye_object()), 2); // initially looking along the rows axis
        for _ in 0..200 {
            cam.orbit(3.0, -2.0);
        }
        let p = cam.rot.mul(&cam.rot.transpose());
        for i in 0..3 {
            for j in 0..3 {
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((p.0[i][j] - e).abs() < 1e-4);
            }
        }
        // 90° about Y → looking along X
        let mut cam = Camera::default();
        cam.orbit(std::f32::consts::FRAC_PI_2 / ROTATE_FACTOR, 0.0);
        assert_eq!(choose_axis(cam.eye_object()), 0);
        // 90° about X → looking along Y
        let mut cam = Camera::default();
        cam.orbit(0.0, std::f32::consts::FRAC_PI_2 / ROTATE_FACTOR);
        assert_eq!(choose_axis(cam.eye_object()), 1);
    }

    #[test]
    fn eye_object_inverts_camera_transform() {
        let mut cam = Camera::default();
        cam.orbit(40.0, 25.0);
        cam.pan_pixels(Vec2::new(30.0, -12.0), 300.0);
        let e = cam.eye_object();
        let c = cam.to_camera(e);
        assert!(c[0].abs() < 1e-4 && c[1].abs() < 1e-4);
        assert!((c[2] - cam.dist).abs() < 1e-4);
    }

    #[test]
    fn slices_sorted_back_to_front() {
        let pos = [-0.5, -0.25, 0.0, 0.25, 0.5];
        assert_eq!(back_to_front(&pos, 4.0), vec![0, 1, 2, 3, 4]);
        assert_eq!(back_to_front(&pos, -4.0), vec![4, 3, 2, 1, 0]);
        let inside = back_to_front(&pos, 0.1);
        assert_eq!(inside[0], 0);
        assert_eq!(*inside.last().unwrap(), 2);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut cam = Camera::default();
        cam.zoom(1e-6);
        assert_eq!(cam.dist, MIN_DIST);
        cam.zoom(1e9);
        assert_eq!(cam.dist, MAX_DIST);
        cam.zoom(f32::NAN);
        assert_eq!(cam.dist, MAX_DIST);
    }
}
