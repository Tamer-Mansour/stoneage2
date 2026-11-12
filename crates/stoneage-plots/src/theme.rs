//! Shared colours/strokes for all views (also used by stoneage-spatial and stoneage-app via
//! `stoneage_plots::theme`). Read the colours from `ui.visuals()` where possible so the views
//! follow light/dark mode.
//!
//! OWNER: plots-row. Everything here is additive; the original three constants are kept.

use egui::{Align2, Color32, FontId, Painter, Pos2, Stroke, Visuals};

/// Accent colour (selection, active elements).
pub const ACCENT: Color32 = Color32::from_rgb(0xE0, 0x8A, 0x2E);
/// Secondary marker colour (cuts, cursors).
pub const MARKER: Color32 = Color32::from_rgb(0x2E, 0xA8, 0xE0);
/// Alpha used for grid lines drawn over data.
pub const GRID_ALPHA: u8 = 60;

/// Replacement for the original `Color.red` markers (active point, horizontal cut).
pub const ACTIVE: Color32 = Color32::from_rgb(0xE5, 0x48, 0x3C);
/// Paper colour used for print / export images (the original printed on white).
pub const PAGE_BG: Color32 = Color32::WHITE;
/// Ink colour used for print / export images (the original used `Color.black`).
pub const PAGE_INK: Color32 = Color32::BLACK;
/// Ink for secondary ticks on paper (original `Color.lightGray`).
pub const PAGE_INK_LIGHT: Color32 = Color32::from_rgb(0xC0, 0xC0, 0xC0);

/// Base spacing unit of the 8px grid.
pub const SPACING: f32 = 8.0;
/// Corner radius for panels/frames.
pub const CORNER_RADIUS: u8 = 6;
/// Size of axis labels.
pub const LABEL_SIZE: f32 = 11.0;
/// Size of small annotations.
pub const SMALL_SIZE: f32 = 10.0;

/// Font for axis labels.
pub fn label_font() -> FontId {
    FontId::proportional(LABEL_SIZE)
}

/// Font for small annotations (legend percentages, ...).
pub fn small_font() -> FontId {
    FontId::proportional(SMALL_SIZE)
}

/// Monospace font for numeric read-outs.
pub fn mono_font() -> FontId {
    FontId::monospace(LABEL_SIZE)
}

/// Normal text colour.
pub fn text(v: &Visuals) -> Color32 {
    v.text_color()
}

/// De-emphasised text colour.
pub fn weak_text(v: &Visuals) -> Color32 {
    v.weak_text_color()
}

/// Background of plot areas (behind data images).
pub fn view_bg(v: &Visuals) -> Color32 {
    v.extreme_bg_color
}

/// Background of axis header strips.
pub fn header_bg(v: &Visuals) -> Color32 {
    v.faint_bg_color
}

/// Stroke of frames around plot areas.
pub fn frame_stroke(v: &Visuals) -> Stroke {
    v.widgets.noninteractive.bg_stroke
}

/// Stroke for major axis ticks.
pub fn axis_stroke(v: &Visuals) -> Stroke {
    Stroke::new(1.0, v.text_color())
}

/// Stroke for minor axis ticks.
pub fn minor_tick_stroke(v: &Visuals) -> Stroke {
    Stroke::new(1.0, v.weak_text_color())
}

/// Stroke for grid lines drawn on top of data of colour `base`.
pub fn grid_stroke(base: Color32) -> Stroke {
    Stroke::new(1.0, with_alpha(base, GRID_ALPHA.saturating_mul(3)))
}

/// Stroke used to highlight the active element.
pub fn active_stroke() -> Stroke {
    Stroke::new(1.5, ACTIVE)
}

/// `c` with its alpha replaced (unmultiplied semantics).
pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// RGBA entry of a colour map lookup table as `Color32`.
pub fn rgba(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Snap a coordinate to the centre of a physical pixel so 1px lines are crisp.
pub fn crisp(x: f32, pixels_per_point: f32) -> f32 {
    let ppp = if pixels_per_point > 0.0 { pixels_per_point } else { 1.0 };
    ((x * ppp).floor() + 0.5) / ppp
}

/// Text with a one-pixel shadow (the original legend drew white text offset by (1,1) under
/// black text so it stays readable on any colour).
pub fn shadow_text(
    painter: &Painter,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
    shadow: Color32,
) {
    painter.text(pos + egui::vec2(1.0, 1.0), align, text, font.clone(), shadow);
    painter.text(pos, align, text, font, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crisp_snaps_to_pixel_centres() {
        assert_eq!(crisp(10.2, 1.0), 10.5);
        assert_eq!(crisp(10.0, 2.0), 10.25);
    }

    #[test]
    fn alpha_replaced() {
        let c = with_alpha(Color32::from_rgb(10, 20, 30), 255);
        assert_eq!(c, Color32::from_rgb(10, 20, 30));
    }
}
