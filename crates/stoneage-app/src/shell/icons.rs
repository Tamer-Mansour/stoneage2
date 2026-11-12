//! tlp-icons (https://tlp-icons.netlify.app, MIT – see `assets/icons/LICENSE-tlp-icons.txt` and
//! `NOTICE-tlp-icons.md`) embedded as white-stroke 24×24 SVGs, rasterised by the egui_extras SVG
//! loader at the exact physical pixel size (crisp at any DPI) and tinted at paint time.
//!
//! tlp-icons animate on hover; [`hover_progress`] + [`paint`] emulate that: a short (~450 ms),
//! eased one-shot motion (pop / spin / wiggle / bounce / nudge, chosen per icon) that always ends
//! at rest. "Reduce motion" (View menu, persisted) turns all of it off.
//!
//! [`IconButton`] is the shared icon(+text) button used across the shell and the panels.

use std::borrow::Cow;
use std::f32::consts::{PI, TAU};

use egui::load::{Bytes, TexturePoll};
use egui::{
    Color32, CornerRadius, Id, Image, ImageSource, Rect, Response, Role, Sense, Stroke, StrokeKind, TextStyle,
    TextWrapMode, Ui, Vec2, Widget, WidgetInfo, WidgetText,
};

use super::theme::{Palette, ERROR_RED};

macro_rules! icon_set {
    ($($name:literal),* $(,)?) => {
        /// All embedded icon names.
        #[cfg_attr(not(test), allow(dead_code))]
        pub const NAMES: &[&str] = &[$($name),*];

        /// Image source of the icon `name` (`None` for unknown names).
        pub fn source(name: &str) -> Option<ImageSource<'static>> {
            match name {
                $($name => Some(ImageSource::Bytes {
                    uri: Cow::Borrowed(concat!("bytes://tlp-icons/", $name, ".svg")),
                    bytes: Bytes::Static(include_bytes!(concat!("../../assets/icons/", $name, ".svg"))),
                }),)*
                _ => None,
            }
        }
    };
}

icon_set!(
    "activity", "arrow-left", "arrow-right", "audio-waveform", "badge-check", "box", "brand-github",
    "chart-column", "check", "chevron-left", "chevron-right", "chevrons-left", "chevrons-right", "circle-dot",
    "circle-play", "circle-stop", "circle-x", "clipboard-list", "cloud-download", "copy", "crosshair",
    "download", "file-plus", "file-text", "folder-open", "folder", "gauge", "grid-3x3", "help-circle",
    "history", "image", "info", "layers", "list-checks", "map", "message-square", "mic", "minus", "moon",
    "mountain", "package", "palette", "panel-left-close", "play", "plus", "printer", "radar", "refresh-cw",
    "rocket", "rows-3", "ruler", "save", "search", "settings", "sliders-horizontal", "sparkle", "square", "sun",
    "table-2", "timer", "trash", "triangle-alert", "upload", "x",
);

/// The tlp-icons MIT license text (shown in Help → About).
pub const LICENSE: &str = include_str!("../../assets/icons/LICENSE-tlp-icons.txt");

/// Installs the egui_extras image loaders (SVG rasteriser). Called once at startup.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

// ---------------------------------------------------------------------------------------------
// motion

/// Duration of the hover motion.
pub const MOTION_SECS: f64 = 0.45;

/// Kind of hover motion of an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// Scale up and settle.
    Pop,
    /// One full eased turn.
    Spin,
    /// Damped rotation back and forth.
    Wiggle,
    /// Hop up and land.
    Bounce,
    /// Short push left and back.
    NudgeLeft,
    /// Short push right and back.
    NudgeRight,
}

/// The motion that fits an icon (mirrors the tlp-icons choreographies in spirit).
pub fn motion_for(name: &str) -> Motion {
    match name {
        "settings" | "refresh-cw" | "sun" | "radar" | "history" | "sparkle" | "crosshair" => Motion::Spin,
        "mic" | "trash" | "triangle-alert" | "message-square" | "help-circle" | "palette" | "moon" | "x"
        | "circle-x" | "timer" | "printer" | "clipboard-list" | "sliders-horizontal" => Motion::Wiggle,
        "download" | "upload" | "cloud-download" | "folder-open" | "folder" | "save" | "file-plus"
        | "file-text" | "rocket" | "package" | "box" | "mountain" | "image" | "map" => Motion::Bounce,
        "chevron-left" | "chevrons-left" | "arrow-left" | "panel-left-close" => Motion::NudgeLeft,
        "chevron-right" | "chevrons-right" | "arrow-right" => Motion::NudgeRight,
        _ => Motion::Pop,
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// Transform of `motion` at progress `t` (0 = start, 1 = rest): (scale, angle, offset).
/// Every motion returns exactly the identity at `t >= 1`.
pub fn transform(motion: Motion, t: f32, size: f32) -> (f32, f32, Vec2) {
    if t >= 1.0 || t <= 0.0 {
        return (1.0, 0.0, Vec2::ZERO);
    }
    let damp = 1.0 - ease_out_cubic(t);
    match motion {
        Motion::Pop => (1.0 + 0.22 * (PI * ease_out_cubic(t)).sin() * (1.0 - 0.4 * t), 0.0, Vec2::ZERO),
        Motion::Spin => (1.0, ease_in_out_cubic(t) * TAU, Vec2::ZERO),
        Motion::Wiggle => (1.0, 0.30 * (t * TAU * 2.0).sin() * damp, Vec2::ZERO),
        Motion::Bounce => {
            let hop = (t * PI * 2.0).sin().abs() * damp;
            (1.0, 0.0, Vec2::new(0.0, -0.16 * size * hop))
        }
        Motion::NudgeLeft => (1.0, 0.0, Vec2::new(-0.14 * size * (t * PI).sin(), 0.0)),
        Motion::NudgeRight => (1.0, 0.0, Vec2::new(0.14 * size * (t * PI).sin(), 0.0)),
    }
}

fn reduce_motion_id() -> Id {
    Id::unique("stoneage_reduce_motion")
}

/// "Reduce motion" setting (persisted in the egui memory).
pub fn reduce_motion(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.get_persisted::<bool>(reduce_motion_id())).unwrap_or(false)
}

pub fn set_reduce_motion(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_persisted(reduce_motion_id(), on));
}

/// Animation duration for state transitions (hover fades, sliding tab pill); 0 with reduce motion.
pub fn transition_secs(ctx: &egui::Context, secs: f32) -> f32 {
    if reduce_motion(ctx) { 0.0 } else { secs }
}

/// One-shot hover motion: returns the progress (0..1, 1 = at rest) of the motion that starts when
/// `hovered` becomes true. The motion always plays to the end, then waits for the next hover.
pub fn hover_progress(ctx: &egui::Context, id: Id, hovered: bool) -> f32 {
    if reduce_motion(ctx) {
        return 1.0;
    }
    let key = id.with("tlp_motion");
    let now = ctx.input(|i| i.time);
    let start: Option<f64> = ctx.data(|d| d.get_temp(key));
    let start = match (start, hovered) {
        (Some(s), _) => s,
        (None, true) => {
            ctx.data_mut(|d| d.insert_temp(key, now));
            now
        }
        (None, false) => return 1.0,
    };
    let t = ((now - start) / MOTION_SECS) as f32;
    if t >= 1.0 {
        if !hovered {
            ctx.data_mut(|d| d.remove::<f64>(key));
        }
        1.0
    } else {
        ctx.request_repaint();
        t.max(0.0)
    }
}

// ---------------------------------------------------------------------------------------------
// painting

/// Paints icon `name` into `rect` (square), tinted with `color`, at motion progress `t`
/// (1 = at rest). The SVG is rasterised once at the rest size, so the motion never re-rasterises.
pub fn paint(ui: &Ui, name: &str, rect: Rect, color: Color32, t: f32) {
    let Some(src) = source(name) else {
        debug_assert!(false, "unknown icon {name}");
        return;
    };
    let size = rect.size();
    let Ok(TexturePoll::Ready { texture }) = Image::new(src).fit_to_exact_size(size).load_for_size(ui.ctx(), size)
    else {
        return;
    };
    let (scale, angle, offset) = transform(motion_for(name), t, size.x);
    let target = Rect::from_center_size(rect.center() + offset, size * scale);
    let mut img = Image::from_texture(texture).tint(color);
    if angle != 0.0 {
        img = img.rotate(angle, Vec2::splat(0.5));
    }
    img.paint_at(ui, target);
}

/// A standalone icon of `size` points (animates when hovered).
pub fn icon(ui: &mut Ui, name: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let t = hover_progress(ui.ctx(), resp.id, resp.hovered());
        paint(ui, name, rect, color, t);
    }
    resp
}

// ---------------------------------------------------------------------------------------------
// IconButton

/// Visual flavour of an [`IconButton`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Quiet raised surface with a hairline.
    Normal,
    /// Accent-filled primary action.
    Primary,
    /// Red text/icon, red tint on hover.
    Danger,
    /// Transparent until hovered.
    Ghost,
}

/// Icon + optional text (+ optional right-aligned shortcut) button with hover/press transitions
/// and the hover motion of the icon.
pub struct IconButton<'a> {
    icon: &'a str,
    text: Option<WidgetText>,
    shortcut: Option<WidgetText>,
    tooltip_label: Option<String>,
    min_size: Vec2,
    icon_size: f32,
    kind: Kind,
    selected: bool,
    wrap: bool,
    align_left: bool,
    corner: u8,
    icon_color: Option<Color32>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: &'a str, text: impl Into<WidgetText>) -> Self {
        let text = text.into();
        let empty = text.text().is_empty();
        Self {
            icon,
            tooltip_label: Some(text.text().to_string()),
            text: if empty { None } else { Some(text) },
            shortcut: None,
            min_size: Vec2::ZERO,
            icon_size: 16.0,
            kind: Kind::Normal,
            selected: false,
            wrap: false,
            align_left: false,
            corner: 7,
            icon_color: None,
        }
    }

    /// Icon-only button (square, ghost by default).
    pub fn icon_only(icon: &'a str, size: f32) -> Self {
        Self {
            icon,
            text: None,
            shortcut: None,
            tooltip_label: None,
            min_size: Vec2::splat(size),
            icon_size: (size * 0.58).round(),
            kind: Kind::Ghost,
            selected: false,
            wrap: false,
            align_left: false,
            corner: 7,
            icon_color: None,
        }
    }

    #[allow(dead_code)] // API kept for shortcut hints inside buttons
    pub fn shortcut(mut self, s: impl Into<WidgetText>) -> Self {
        self.shortcut = Some(s.into());
        self.align_left = true;
        self
    }
    pub fn min_size(mut self, s: Vec2) -> Self {
        self.min_size = s;
        self
    }
    pub fn icon_size(mut self, s: f32) -> Self {
        self.icon_size = s;
        self
    }
    pub fn kind(mut self, k: Kind) -> Self {
        self.kind = k;
        self
    }
    pub fn primary(self) -> Self {
        self.kind(Kind::Primary)
    }
    pub fn danger(self) -> Self {
        self.kind(Kind::Danger)
    }
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
    pub fn align_left(mut self) -> Self {
        self.align_left = true;
        self
    }
    pub fn corner(mut self, c: u8) -> Self {
        self.corner = c;
        self
    }
    pub fn icon_color(mut self, c: Color32) -> Self {
        self.icon_color = Some(c);
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let pad = ui.spacing().button_padding;
        let gap = 7.0;
        let isz = self.icon_size;

        let shortcut = self.shortcut.map(|s| s.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Small));
        let short_w = shortcut.as_ref().map(|g| g.size().x + 10.0).unwrap_or(0.0);
        let outer_w = if self.min_size.x > 0.0 { self.min_size.x } else { ui.available_width() };
        let text_max = (outer_w - 2.0 * pad.x - isz - gap - short_w).max(20.0);
        let mode = if self.wrap { TextWrapMode::Wrap } else { TextWrapMode::Truncate };
        let galley = self.text.map(|t| t.into_galley(ui, Some(mode), text_max, TextStyle::Button));

        let content_w = isz + galley.as_ref().map(|g| gap + g.size().x).unwrap_or(0.0) + short_w;
        let content_h = galley.as_ref().map(|g| g.size().y).unwrap_or(0.0).max(isz);
        let desired = if galley.is_none() && shortcut.is_none() && self.min_size.x > 0.0 {
            // Icon-only: exactly the requested size (no padding added on top).
            self.min_size.max(Vec2::splat(isz + 4.0))
        } else {
            Vec2::new(content_w + 2.0 * pad.x, content_h + 2.0 * pad.y)
                .max(self.min_size)
                .max(Vec2::new(0.0, ui.spacing().interact_size.y))
        };
        let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
        let label = self.tooltip_label.clone().unwrap_or_default();
        let enabled = ui.is_enabled();
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &label));

        if ui.is_rect_visible(rect) {
            let ctx = ui.ctx();
            let p = Palette::of(ui.visuals());
            let dur = transition_secs(ctx, 0.12);
            let hov = ctx.animate_bool_with_time(resp.id.with("hov"), resp.hovered() && enabled, dur);
            let down = resp.is_pointer_button_down_on();
            let w = &ui.visuals().widgets;
            let (rest_fill, hover_fill, press_fill) = (w.inactive.weak_bg_fill, w.hovered.weak_bg_fill, w.active.weak_bg_fill);

            let (fill, stroke, fg, icon_fg) = match self.kind {
                Kind::Primary => {
                    let base = p.accent;
                    let hover = lighten(base, 0.12);
                    let fill = if down { darken(base, 0.08) } else { lerp(base, hover, hov) };
                    (fill, Stroke::NONE, Color32::WHITE, Color32::WHITE)
                }
                Kind::Danger => {
                    let red = ERROR_RED;
                    let fill = if down { red.gamma_multiply(0.30) } else { lerp(rest_fill, red.gamma_multiply(0.16), hov) };
                    (fill, Stroke::new(1.0, lerp(p.hairline, red.gamma_multiply(0.7), hov)), red, red)
                }
                Kind::Ghost => {
                    let fill = if down { press_fill } else { p.hover.gamma_multiply(hov) };
                    (fill, Stroke::NONE, p.text, p.text)
                }
                Kind::Normal => {
                    let fill = if down { press_fill } else { lerp(rest_fill, hover_fill, hov) };
                    (fill, Stroke::new(1.0, lerp(p.hairline, p.border_strong, hov)), p.text, p.text)
                }
            };
            let (fill, stroke, fg, icon_fg) = if self.selected {
                (p.accent_soft, Stroke::new(1.0, p.accent.gamma_multiply(0.75)), p.text, p.accent)
            } else {
                (fill, stroke, fg, icon_fg)
            };
            let icon_fg = self.icon_color.unwrap_or(icon_fg);
            let r = CornerRadius::same(self.corner);
            ui.painter().rect(rect, r, fill, stroke, StrokeKind::Inside);
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.expand(1.0), r, Stroke::new(1.5, p.cool), StrokeKind::Outside);
            }

            // content
            let left_aligned = self.align_left && galley.is_some();
            let group_w = isz + galley.as_ref().map(|g| gap + g.size().x).unwrap_or(0.0);
            let x0 = if left_aligned {
                rect.left() + pad.x
            } else {
                rect.center().x - (group_w + short_w) * 0.5
            };
            let icon_rect = Rect::from_min_size(egui::pos2(x0, rect.center().y - isz * 0.5), Vec2::splat(isz));
            let t = hover_progress(ctx, resp.id, resp.hovered() && enabled);
            paint(ui, self.icon, icon_rect, icon_fg, t);
            if let Some(g) = galley {
                let pos = egui::pos2(icon_rect.right() + gap, rect.center().y - g.size().y * 0.5);
                ui.painter().galley(pos, g, fg);
            }
            if let Some(s) = shortcut {
                let weak = if self.kind == Kind::Primary { Color32::from_white_alpha(200) } else { p.text_weak };
                let pos = egui::pos2(rect.right() - pad.x - s.size().x, rect.center().y - s.size().y * 0.5);
                ui.painter().galley(pos, s, weak);
            }
        }
        resp
    }
}

/// Linear blend `a` → `b`.
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_array();
    let [br, bg, bb, ba] = b.to_array();
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(m(ar, br), m(ag, bg), m(ab, bb), m(aa, ba))
}

pub fn lighten(c: Color32, f: f32) -> Color32 {
    lerp(c, Color32::WHITE, f)
}

pub fn darken(c: Color32, f: f32) -> Color32 {
    lerp(c, Color32::BLACK, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_embedded_and_white() {
        for n in NAMES {
            let Some(ImageSource::Bytes { bytes, .. }) = source(n) else { panic!("missing {n}") };
            let s = std::str::from_utf8(&bytes).unwrap();
            assert!(s.starts_with("<svg") && s.contains("stroke=\"white\""), "{n}");
        }
        assert!(source("no-such-icon").is_none());
    }

    #[test]
    fn motions_end_at_rest() {
        for m in [Motion::Pop, Motion::Spin, Motion::Wiggle, Motion::Bounce, Motion::NudgeLeft, Motion::NudgeRight] {
            assert_eq!(transform(m, 1.0, 16.0), (1.0, 0.0, Vec2::ZERO));
            let (s, a, o) = transform(m, 0.999, 16.0);
            let a = a.rem_euclid(TAU).min(TAU - a.rem_euclid(TAU));
            assert!((s - 1.0).abs() < 0.02 && a < 0.05 && o.length() < 0.3, "{m:?}");
        }
    }
}
