//! Point view – port of `MessPunkt$ViewArea` plus the display parts of `MessPunkt`.
//!
//! Shows the active point of the active row of the [`Survey`]: the `values` of the point as
//! colour-mapped bars (colour = `ColorMap.getMap(colorModel, reverseColor, colorModelBase)`),
//! a raw-value scale (0..256) and the dB scale derived from `dbMin`/`dbMax`, the frequency / depth
//! (spectrum source) or time (signal source) axis, the red horizontal-cut marker with its caption,
//! the original info texts (source, demo mode, sample rate, signal length, frequency range,
//! min/max, repaint time) and the draggable FFT interval (`FFTstartIdx`/`FFTendIdx`,
//! `DRAG_START`/`DRAG_END`/`DRAG_INTERVALL`).
//!
//! Modern additions: zoom (ctrl+wheel, buttons, double click resets), wheel panning when zoomed,
//! auto-scroll while dragging outside the plot (`scrollRectToVisible`), hover crosshair with
//! read-out (also returned in [`ViewResponse::hover_info`]), empty states.
//! See `docs/NOTES-plots-point.md` for the mapping and deviations.

use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Layout, PointerButton, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, Vec2,
};
use stoneage_core::{colormap, I18n, Language, Point, Survey, NUM_SAMPLES};

use crate::{theme, ViewResponse};

use logic::*;

/// `DRAG_NONE` / `DRAG_START` / `DRAG_END` / `DRAG_INTERVALL` of `MessPunkt$ViewArea`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DragType {
    #[default]
    None = 0,
    Start = 1,
    End = 2,
    Interval = 3,
}

/// `MessPunkt.FFTintervall` – minimum width of the FFT interval (in `signal_values` indices).
pub const FFT_INTERVAL_MIN: i32 = 64;
/// Upper bound of the FFT interval (`signal_values.length` = 4096).
pub const FFT_IDX_MAX: i32 = 4096;
/// Timer delay of `MessPunkt$ViewArea.t` (repaint every 25 ms).
pub const REPAINT_INTERVAL: Duration = Duration::from_millis(25);

/// Number of displayed sample indices (`MessPunkt.numSamples`).
const DOMAIN: f64 = NUM_SAMPLES as f64;
/// Value that maps to the top of the plot (the original plot is 256 px high, 1 px per unit).
const VALUE_TOP: f64 = 256.0;
/// Step of the original value/dB grid (`k * 32`, k = 0..=8).
const VALUE_GRID_STEP: i64 = 32;
const MAX_ZOOM: f64 = 64.0;

const CUT_COLOR: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);

// Plot margins inside the allocated rect (8 px grid).
const MARGIN_LEFT: f32 = 48.0;
const MARGIN_RIGHT: f32 = 64.0;
const MARGIN_TOP: f32 = 24.0;
const MARGIN_BOTTOM: f32 = 48.0;

pub struct PointView {
    /// `ViewArea.dragType`
    drag_type: DragType,
    /// `ViewArea.dragStart` (kept as FFT index instead of a pixel x).
    drag_start: i32,
    /// Magnification (1 = whole 2048 samples fit the width). Replaces the fixed `ViewArea.zoom = 4`.
    zoom: f64,
    /// First visible sample index (replaces the JScrollPane viewport position).
    view_start: f64,
}

impl Default for PointView {
    fn default() -> Self {
        Self {
            drag_type: DragType::None,
            drag_start: 0,
            zoom: 1.0,
            view_start: 0.0,
        }
    }
}

/// What the X axis shows.
#[derive(Clone, Copy, PartialEq)]
enum XKind {
    FrequencyKhz,
    DepthM,
    TimeMs,
    Sample,
}

#[derive(Clone, Copy)]
struct XAxis {
    kind: XKind,
    /// Axis unit per sample index.
    per_sample: f64,
}

impl XAxis {
    fn value(&self, s: f64) -> f64 {
        s * self.per_sample
    }
    fn sample(&self, u: f64) -> f64 {
        u / self.per_sample
    }
    fn unit(&self) -> &'static str {
        match self.kind {
            XKind::FrequencyKhz => "kHz",
            XKind::DepthM => "m",
            XKind::TimeMs => "ms",
            XKind::Sample => "",
        }
    }
}

/// Mapping between sample space and screen space of the plot rectangle.
#[derive(Clone, Copy)]
struct Frame {
    plot: Rect,
    view_start: f64,
    span: f64,
}

impl Frame {
    fn x_of(&self, s: f64) -> f32 {
        self.plot.left() + ((s - self.view_start) / self.span) as f32 * self.plot.width()
    }
    fn sample_at(&self, x: f32) -> f64 {
        self.view_start + ((x - self.plot.left()) / self.plot.width()) as f64 * self.span
    }
    fn y_of(&self, v: f64) -> f32 {
        self.plot.bottom() - (v / VALUE_TOP) as f32 * self.plot.height()
    }
    /// FFT interval index (`signal_values` index, 2 per sample) under screen x.
    fn fft_idx_at(&self, x: f32) -> i32 {
        (2.0 * self.sample_at(x)).round() as i32
    }
}

impl PointView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
        self.view_start = 0.0;
    }

    fn span(&self) -> f64 {
        DOMAIN / self.zoom
    }

    fn clamp_view(&mut self) {
        self.zoom = self.zoom.clamp(1.0, MAX_ZOOM);
        let max_start = (DOMAIN - self.span()).max(0.0);
        self.view_start = self.view_start.clamp(0.0, max_start);
    }

    /// Zoom by `factor` keeping sample `anchor` at the same screen position.
    fn zoom_about(&mut self, factor: f64, anchor: f64) {
        let old_span = self.span();
        let frac = if old_span > 0.0 { (anchor - self.view_start) / old_span } else { 0.5 };
        self.zoom = (self.zoom * factor).clamp(1.0, MAX_ZOOM);
        self.view_start = anchor - frac * self.span();
        self.clamp_view();
    }

    /// Shows the active point of the active row.
    pub fn ui(&mut self, ui: &mut egui::Ui, survey: &mut Survey, i18n: &I18n, demo_mode: bool) -> ViewResponse {
        let started = Instant::now();
        // FFTstartIdx/FFTendIdx live on the survey; edit a copy and write it back at the end.
        let mut fft = (survey.fft_interval.start_idx, survey.fft_interval.end_idx);
        let resp = self.ui_inner(ui, survey, i18n, demo_mode, started, &mut fft);
        if (survey.fft_interval.start_idx, survey.fft_interval.end_idx) != fft {
            survey.fft_interval.start_idx = fft.0;
            survey.fft_interval.end_idx = fft.1;
        }
        resp
    }

    #[allow(clippy::too_many_arguments)]
    fn ui_inner(
        &mut self,
        ui: &mut egui::Ui,
        survey: &Survey,
        i18n: &I18n,
        demo_mode: bool,
        started: Instant,
        fft: &mut (i32, i32),
    ) -> ViewResponse {
        let mut resp = ViewResponse::default();
        self.clamp_view();

        let Some(point) = survey.current_row().and_then(|r| r.current_point()) else {
            empty_state(ui, &t(i18n, "App.PointView.NoPoint", "No measurement point selected"), false);
            return resp;
        };
        if point.values.is_empty() {
            empty_state(ui, &t(i18n, "App.PointView.NoData", "This point contains no data"), false);
            return resp;
        }

        // Original timer: repaint every 25 ms; `paintComponent` acquires live data while !acquired.
        if !point.acquired {
            ui.ctx().request_repaint_after(REPAINT_INTERVAL);
        }

        let block = BlockParams { size: survey.block_size, average: survey.use_block_average };
        let vals: Vec<i64> = (0..NUM_SAMPLES).map(|i| get_value(&point.values, i, block)).collect();
        // second paint loop: min starts at 256, max at 0
        let vmin = vals.iter().copied().fold(256, i64::min);
        let vmax = vals.iter().copied().fold(0, i64::max);
        let valid = is_data_valid(&point.values);
        let spectrum = point.spectrum_is_source;
        let x_axis = self.x_axis(survey, point);

        self.header(ui, i18n, point, demo_mode, vmin, vmax, started);
        ui.add_space(4.0);

        let size = ui.available_size().max(Vec2::new(240.0, 180.0));
        let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = response.rect;
        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + MARGIN_LEFT, rect.top() + MARGIN_TOP),
            Pos2::new(rect.right() - MARGIN_RIGHT, rect.bottom() - MARGIN_BOTTOM),
        );
        if plot.width() < 16.0 || plot.height() < 16.0 {
            return resp;
        }

        // ---- input: zoom / pan --------------------------------------------------------------
        if response.hovered() {
            let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            let frame = self.frame(plot);
            let anchor = response.hover_pos().map_or(self.view_start + self.span() / 2.0, |p| frame.sample_at(p.x));
            if zoom_delta != 1.0 {
                self.zoom_about(zoom_delta as f64, anchor);
            } else if self.zoom > 1.0 && (scroll.x != 0.0 || scroll.y != 0.0) {
                let d = -(scroll.x + scroll.y) as f64 / plot.width() as f64 * self.span();
                self.view_start += d;
                self.clamp_view();
            }
        }
        if response.double_clicked() {
            self.reset_zoom();
        }

        // ---- input: FFT interval dragging (mousePressed / mouseDragged / mouseReleased) --------
        if !spectrum {
            let frame = self.frame(plot);
            if response.drag_started_by(PointerButton::Primary) {
                let origin = ui.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos());
                if let Some(p) = origin {
                    let idx = frame.fft_idx_at(p.x);
                    self.drag_start = idx;
                    self.drag_type = press_drag_type(idx, fft.0, fft.1);
                }
            }
            if response.dragged_by(PointerButton::Primary) && self.drag_type != DragType::None {
                if let Some(p) = response.interact_pointer_pos() {
                    // scrollRectToVisible: auto-scroll when dragging beyond the plot
                    let over = if p.x < plot.left() {
                        p.x - plot.left()
                    } else if p.x > plot.right() {
                        p.x - plot.right()
                    } else {
                        0.0
                    };
                    if over != 0.0 && self.zoom > 1.0 {
                        self.view_start += over as f64 / plot.width() as f64 * self.span() * 0.1;
                        self.clamp_view();
                        ui.ctx().request_repaint();
                    }
                    let idx = self.frame(plot).fft_idx_at(p.x);
                    let (s, e, ds) = drag_fft(self.drag_type, idx, fft.0, fft.1, self.drag_start);
                    *fft = (s, e);
                    self.drag_start = ds;
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.drag_type = DragType::None;
            }
            if let Some(p) = response.hover_pos() {
                let icon = match self.drag_type {
                    DragType::Interval => CursorIcon::Grabbing,
                    DragType::Start | DragType::End => CursorIcon::ResizeHorizontal,
                    DragType::None => {
                        let idx = frame.fft_idx_at(p.x);
                        if press_drag_type(idx, fft.0, fft.1) == DragType::Interval {
                            CursorIcon::Grab
                        } else {
                            CursorIcon::ResizeHorizontal
                        }
                    }
                };
                ui.ctx().set_cursor_icon(icon);
            }
        } else {
            self.drag_type = DragType::None;
        }

        // ---- paint ---------------------------------------------------------------------------
        let frame = self.frame(plot);
        let visuals = ui.visuals().clone();
        let text_color = visuals.text_color();
        let weak = visuals.weak_text_color();
        let grid = with_alpha(text_color, theme::GRID_ALPHA);
        let tick_font = FontId::proportional(11.0);
        let caption_font = FontId::proportional(12.0);

        painter.rect_filled(rect, CornerRadius::same(8), visuals.extreme_bg_color);
        painter.rect_filled(plot, CornerRadius::ZERO, visuals.faint_bg_color);
        let clip = painter.with_clip_rect(plot);

        // value / dB grid (original: 9 green lines at marginy + k*32)
        // Point::spectrum_factor/shift (dbMin/dbMax); fall back to the original defaults if unset.
        let (factor, shift) = if point.spectrum_factor.is_finite() && point.spectrum_factor != 0.0 {
            (point.spectrum_factor, point.spectrum_shift)
        } else {
            (spectrum_factor(-15.0, 40.0), spectrum_shift(-15.0, 40.0))
        };
        let mut v = 0;
        while v <= VALUE_TOP as i64 {
            let y = frame.y_of(v as f64);
            painter.hline(plot.x_range(), y, Stroke::new(1.0, grid));
            painter.text(Pos2::new(plot.left() - 6.0, y), Align2::RIGHT_CENTER, v.to_string(), tick_font.clone(), weak);
            painter.text(
                Pos2::new(plot.right() + 6.0, y),
                Align2::LEFT_CENTER,
                format!("{} dB", db_label(v, shift, factor)),
                tick_font.clone(),
                weak,
            );
            v += VALUE_GRID_STEP;
        }
        painter.text(
            Pos2::new(plot.left() - 6.0, plot.top() - 8.0),
            Align2::RIGHT_BOTTOM,
            t(i18n, "App.PointView.Axis.Value", "Value"),
            tick_font.clone(),
            weak,
        );
        painter.text(
            Pos2::new(plot.right() + 6.0, plot.top() - 8.0),
            Align2::LEFT_BOTTOM,
            t(i18n, "App.PointView.Axis.Level", "Level"),
            tick_font.clone(),
            weak,
        );

        // X ticks (original: 20 frequency ticks / 16 time ticks, green)
        let u0 = x_axis.value(frame.view_start);
        let u1 = x_axis.value(frame.view_start + frame.span);
        let target = (plot.width() / 96.0).max(2.0) as usize;
        let (ticks, step) = nice_ticks(u0, u1, target);
        let decimals = decimals_for(step);
        for u in ticks {
            let x = frame.x_of(x_axis.sample(u));
            if x < plot.left() - 0.5 || x > plot.right() + 0.5 {
                continue;
            }
            painter.vline(x, plot.y_range(), Stroke::new(1.0, grid));
            painter.text(
                Pos2::new(x, plot.bottom() + 6.0),
                Align2::CENTER_TOP,
                format!("{u:.decimals$}"),
                tick_font.clone(),
                weak,
            );
        }
        let x_title = match x_axis.kind {
            XKind::FrequencyKhz => format!("{} [kHz]", t(i18n, "App.PointView.Axis.Frequency", "Frequency")),
            XKind::DepthM => format!("{} [m]", t(i18n, "App.PointView.Axis.Depth", "Depth")),
            XKind::TimeMs => format!("{} [ms]", t(i18n, "App.PointView.Axis.Time", "Time")),
            XKind::Sample => t(i18n, "App.PointView.Axis.Sample", "Sample"),
        };
        painter.text(Pos2::new(plot.center().x, plot.bottom() + 24.0), Align2::CENTER_TOP, x_title, caption_font.clone(), text_color);

        // colour-mapped bars (second paint loop)
        let lut = colormap::lut(survey.color_model.max(0) as usize, survey.reverse_color, survey.color_model_base);
        let color_of = |avg: i64| {
            let c = lut[avg.clamp(0, 255) as usize];
            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
        };
        let y0 = frame.y_of(0.0);
        let bar = |x0: f32, x1: f32, avg: i64| {
            let y = frame.y_of(avg as f64).clamp(plot.top(), plot.bottom());
            let r = Rect::from_min_max(Pos2::new(x0, y.min(y0)), Pos2::new(x1, y.max(y0)));
            clip.rect_filled(r, CornerRadius::ZERO, color_of(avg));
        };
        let s_first = frame.view_start.floor().max(0.0) as usize;
        let s_last = ((frame.view_start + frame.span).ceil() as usize).min(NUM_SAMPLES);
        let spp = frame.span / plot.width() as f64;
        if spp >= 1.0 {
            // average `spp` samples per pixel column like the original `sum / zoom`
            let mut col: Option<i64> = None;
            let (mut sum, mut cnt) = (0i64, 0i64);
            let flush = |col: Option<i64>, sum: i64, cnt: i64| {
                if let (Some(c), true) = (col, cnt > 0) {
                    let x0 = plot.left() + c as f32;
                    bar(x0, x0 + 1.0, sum / cnt);
                }
            };
            for (s, &v) in vals.iter().enumerate().take(s_last).skip(s_first) {
                let c = ((s as f64 + 0.5 - frame.view_start) / spp).floor() as i64;
                if col != Some(c) {
                    flush(col, sum, cnt);
                    col = Some(c);
                    sum = 0;
                    cnt = 0;
                }
                sum += v;
                cnt += 1;
            }
            flush(col, sum, cnt);
        } else {
            for (s, &v) in vals.iter().enumerate().take(s_last).skip(s_first) {
                let x0 = frame.x_of(s as f64);
                let x1 = frame.x_of(s as f64 + 1.0);
                let gap = if x1 - x0 > 4.0 { 1.0 } else { 0.0 };
                bar(x0, x1 - gap, v);
            }
        }

        // FFT interval (signal source only – see notes)
        if !spectrum {
            let xa = frame.x_of(fft.0 as f64 / 2.0);
            let xb = frame.x_of(fft.1 as f64 / 2.0);
            let band = Rect::from_min_max(Pos2::new(xa, plot.top()), Pos2::new(xb, plot.bottom()));
            clip.rect_filled(band, CornerRadius::ZERO, with_alpha(theme::MARKER, 36));
            for x in [xa, xb] {
                clip.vline(x, plot.y_range(), Stroke::new(2.0, theme::MARKER));
                let grip = Rect::from_center_size(Pos2::new(x, plot.top() + 14.0), Vec2::new(6.0, 18.0));
                clip.rect_filled(grip, CornerRadius::same(3), theme::MARKER);
            }
            let label = format!(
                "{}  {} – {}",
                t(i18n, "App.PointView.FftInterval", "FFT interval"),
                fft.0,
                fft.1
            );
            clip.text(Pos2::new(xa.max(plot.left()) + 8.0, plot.bottom() - 6.0), Align2::LEFT_BOTTOM, label, tick_font.clone(), theme::MARKER);
        }

        // horizontal cut (first paint loop: red line + caption, i in 0..=2048)
        let cut = survey.horizontal_cut;
        if (0..=NUM_SAMPLES as i32).contains(&cut) {
            let x = frame.x_of(cut as f64 + 0.5).clamp(plot.left(), plot.right());
            if x > plot.left() - 1.0 && x < plot.right() + 1.0 {
                clip.vline(x, plot.y_range(), Stroke::new(1.5, CUT_COLOR));
                let caption = i18n.trf(
                    "MessPunkt.horizontalschnitt.Caption",
                    &[&java_msg_number(cut as f64, i18n.lang), &java_msg_number(survey.depth(cut as usize), i18n.lang)],
                );
                // original: right of the line for i < 1024, otherwise 210 px to the left
                let right = x < plot.center().x;
                let (pos, anchor) = if right {
                    (Pos2::new(x + 6.0, plot.top() + 6.0), Align2::LEFT_TOP)
                } else {
                    (Pos2::new(x - 6.0, plot.top() + 6.0), Align2::RIGHT_TOP)
                };
                let galley = clip.layout_no_wrap(caption, tick_font.clone(), CUT_COLOR);
                let r = anchor.anchor_size(pos, galley.size()).expand(3.0);
                clip.rect_filled(r, CornerRadius::same(4), with_alpha(visuals.extreme_bg_color, 220));
                clip.galley(r.min + Vec2::splat(3.0), galley, CUT_COLOR);
            }
        }

        painter.rect_stroke(plot, CornerRadius::ZERO, Stroke::new(1.0, with_alpha(text_color, 90)), StrokeKind::Outside);

        // invalid data message (OptionPanel.point_data_not_valid)
        if !valid {
            let msg = i18n.tr("OptionPanel.point_data_not_valid");
            let galley = painter.layout(msg, caption_font.clone(), visuals.warn_fg_color, plot.width() - 32.0);
            let r = Rect::from_center_size(plot.center(), galley.size()).expand(12.0);
            painter.rect_filled(r, CornerRadius::same(8), with_alpha(visuals.extreme_bg_color, 235));
            painter.rect_stroke(r, CornerRadius::same(8), Stroke::new(1.0, visuals.warn_fg_color), StrokeKind::Inside);
            painter.galley(r.min + Vec2::splat(12.0), galley, visuals.warn_fg_color);
        }

        // hover crosshair + read-out
        if let Some(p) = response.hover_pos().filter(|p| plot.contains(*p)) {
            let s = frame.sample_at(p.x).floor().clamp(0.0, DOMAIN - 1.0) as usize;
            let v = vals[s];
            let cross = with_alpha(text_color, 140);
            let yv = frame.y_of(v as f64).clamp(plot.top(), plot.bottom());
            clip.vline(p.x, plot.y_range(), Stroke::new(1.0, cross));
            clip.hline(plot.x_range(), yv, Stroke::new(1.0, cross));
            clip.circle_filled(Pos2::new(p.x, yv), 3.5, text_color);
            clip.circle_stroke(Pos2::new(p.x, yv), 3.5, Stroke::new(1.5, visuals.extreme_bg_color));

            let db = db_value(v, shift, factor);
            let x_txt = match x_axis.kind {
                XKind::Sample => format!("{s}"),
                _ => format!("{:.*} {}", decimals + 1, x_axis.value(s as f64), x_axis.unit()),
            };
            pill(&painter, Pos2::new(p.x, plot.bottom() + 2.0), Align2::CENTER_TOP, &x_txt, &tick_font, &visuals);
            pill(&painter, Pos2::new(plot.left() - 2.0, yv), Align2::RIGHT_CENTER, &v.to_string(), &tick_font, &visuals);
            pill(&painter, Pos2::new(plot.right() + 2.0, yv), Align2::LEFT_CENTER, &format!("{db:.2} dB"), &tick_font, &visuals);

            let mut parts = vec![format!("{} {s}", t(i18n, "App.PointView.Axis.Sample", "Sample"))];
            if spectrum {
                let khz = point.f_range / DOMAIN * s as f64 / 1000.0;
                parts.push(format!("{} {khz:.3} kHz", t(i18n, "App.PointView.Axis.Frequency", "Frequency")));
                parts.push(format!("{} {:.2} m", t(i18n, "App.PointView.Axis.Depth", "Depth"), survey.depth(s)));
            } else if point.sample_rate > 0 {
                let ms = 1000.0 * s as f64 / point.sample_rate as f64;
                parts.push(format!("{} {ms:.3} ms", t(i18n, "App.PointView.Axis.Time", "Time")));
            }
            parts.push(format!("{} {v} ({db:.2} dB)", t(i18n, "App.PointView.Axis.Value", "Value")));
            resp.hover_info = Some(parts.join("  ·  "));
        }

        resp
    }

    fn frame(&self, plot: Rect) -> Frame {
        Frame { plot, view_start: self.view_start, span: self.span() }
    }

    fn x_axis(&self, survey: &Survey, point: &Point) -> XAxis {
        let fallback = XAxis { kind: XKind::Sample, per_sample: 1.0 };
        let axis = if point.spectrum_is_source {
            if survey.show_depth {
                // MessReihe.tiefe(i) is linear in i (rounded to cm) – take the slope over the full range.
                XAxis { kind: XKind::DepthM, per_sample: survey.depth(NUM_SAMPLES) / DOMAIN }
            } else {
                // MessReihe.getfStep() = fRange / 2048 [Hz]
                XAxis { kind: XKind::FrequencyKhz, per_sample: point.f_range / DOMAIN / 1000.0 }
            }
        } else if point.sample_rate > 0 {
            XAxis { kind: XKind::TimeMs, per_sample: 1000.0 / point.sample_rate as f64 }
        } else {
            fallback
        };
        if axis.per_sample.is_finite() && axis.per_sample > 0.0 {
            axis
        } else {
            fallback
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn header(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        point: &Point,
        demo_mode: bool,
        vmin: i64,
        vmax: i64,
        started: Instant,
    ) {
        let visuals = ui.visuals().clone();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let source = if point.spectrum_is_source {
                i18n.tr("MessPunkt.spectrumIsSource.Caption")
            } else {
                i18n.tr("MessPunkt.signalIsSource.Caption")
            };
            chip(ui, &source, theme::MARKER);
            if demo_mode {
                chip(ui, &i18n.tr("MessPunkt.Demomode.Caption"), theme::ACCENT);
            }
            if !point.acquired {
                chip(ui, &t(i18n, "App.PointView.Live", "Live"), visuals.warn_fg_color);
            }
            let weak = visuals.weak_text_color();
            let info = |ui: &mut egui::Ui, s: String| {
                ui.label(RichText::new(s).small().color(weak));
            };
            info(ui, format!("{}: {}", t(i18n, "App.PointView.SampleRate", "Samplerate"), fmt_sample_rate(point.sample_rate)));
            info(ui, format!("{}: {}", i18n.tr("MessPunkt.signalLength.Caption"), fmt_signal_length_total(point.sample_rate)));
            if point.spectrum_is_source {
                info(ui, format!("{}: {}", t(i18n, "App.PointView.FrequencyRange", "Frequency range"), fmt_f_range(point.f_range)));
            } else {
                info(ui, format!("{}: {}", i18n.tr("MessPunkt.signalLength.Caption"), fmt_signal_caption(point.sample_rate)));
            }
            info(ui, format!("min = {vmin} max = {vmax}"));
            info(ui, format!("repaint: {} ms", started.elapsed().as_millis()));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .small_button("⟲")
                    .on_hover_text(t(i18n, "App.PointView.ZoomReset", "Reset zoom (double click)"))
                    .clicked()
                {
                    self.reset_zoom();
                }
                let center = self.view_start + self.span() / 2.0;
                if ui.small_button("+").on_hover_text(t(i18n, "App.PointView.ZoomIn", "Zoom in (Ctrl + wheel)")).clicked() {
                    self.zoom_about(2.0, center);
                }
                ui.label(RichText::new(format!("×{:.1}", self.zoom)).small().monospace());
                if ui.small_button("−").on_hover_text(t(i18n, "App.PointView.ZoomOut", "Zoom out (Ctrl + wheel)")).clicked() {
                    self.zoom_about(0.5, center);
                }
            });
        });
    }
}

// ---- small UI helpers ------------------------------------------------------------------------

/// `I18n::tr` with an English fallback for new `App.*` keys not yet in the property files.
fn t(i18n: &I18n, key: &str, fallback: &str) -> String {
    let s = i18n.tr(key);
    if s == key {
        fallback.to_owned()
    } else {
        s
    }
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(with_alpha(color, 40))
        .stroke(Stroke::new(1.0, with_alpha(color, 140)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().strong().color(color));
        });
}

fn pill(painter: &egui::Painter, pos: Pos2, anchor: Align2, text: &str, font: &FontId, visuals: &egui::Visuals) {
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), visuals.strong_text_color());
    let r = anchor.anchor_size(pos, galley.size() + Vec2::new(8.0, 4.0));
    painter.rect_filled(r, CornerRadius::same(4), visuals.widgets.active.bg_fill);
    painter.galley(r.min + Vec2::new(4.0, 2.0), galley, visuals.strong_text_color());
}

fn empty_state(ui: &mut egui::Ui, msg: &str, warn: bool) {
    let size = ui.available_size().max(Vec2::new(200.0, 120.0));
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let visuals = ui.visuals();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(8), visuals.extreme_bg_color);
    let color = if warn { visuals.warn_fg_color } else { visuals.weak_text_color() };
    painter.text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(14.0), color);
}

// ---- pure logic (ported from the bytecode) -----------------------------------------------------

mod logic {
    use super::{DragType, Language, FFT_IDX_MAX, FFT_INTERVAL_MIN, NUM_SAMPLES};

    #[derive(Clone, Copy)]
    pub struct BlockParams {
        /// `MessAreal.blockSize`
        pub size: i32,
        /// `MessAreal.useBlockAverage`
        pub average: bool,
    }

    /// `MessPunkt.getValue(int)` (including the ArrayIndexOutOfBounds fallbacks).
    pub fn get_value(values: &[i64], i: usize, block: BlockParams) -> i64 {
        let fallback = || {
            if i >= NUM_SAMPLES {
                values.get(NUM_SAMPLES - 1).copied().unwrap_or(0)
            } else {
                values.first().copied().unwrap_or(0)
            }
        };
        let bs = block.size as i64;
        if bs == 1 {
            return values.get(i).copied().unwrap_or_else(fallback);
        }
        if bs <= 0 {
            // Java: division by zero / empty loop – no sensible original behaviour; show raw value.
            return values.get(i).copied().unwrap_or_else(fallback);
        }
        let base = (jround_d(i as f64 / bs as f64) * bs) as i64;
        let (mut sum, mut max) = (0i64, 0i64);
        for k in base..base + bs {
            match usize::try_from(k).ok().and_then(|k| values.get(k)) {
                Some(&v) => {
                    sum += v;
                    max = max.max(v);
                }
                None => return fallback(),
            }
        }
        if block.average {
            sum / bs
        } else {
            max
        }
    }

    /// `MessPunkt.isDataValid()`: max - min > 10.
    pub fn is_data_valid(values: &[i64]) -> bool {
        let (mut min, mut max) = (100_000i64, -100_000i64);
        for &v in values {
            max = max.max(v);
            min = min.min(v);
        }
        max - min > 10
    }

    pub fn spectrum_factor(db_min: f32, db_max: f32) -> f32 {
        256.0 / (db_max - db_min)
    }

    pub fn spectrum_shift(db_min: f32, db_max: f32) -> f32 {
        -spectrum_factor(db_min, db_max) * db_min
    }

    /// dB grid label: `Math.round(100f * (v - spectrum_shift) / spectrum_factor) / 100.0`.
    pub fn db_label(v: i64, shift: f32, factor: f32) -> String {
        java_double(jround_f(100.0 * (v as f32 - shift) / factor) as f64 / 100.0)
    }

    pub fn db_value(v: i64, shift: f32, factor: f32) -> f64 {
        (v as f64 - shift as f64) / factor as f64
    }

    /// `mousePressed`: which edge / the whole interval is dragged.
    pub fn press_drag_type(idx: i32, start: i32, end: i32) -> DragType {
        if idx < start {
            DragType::Start
        } else if idx > end {
            DragType::End
        } else {
            DragType::Interval
        }
    }

    /// `mouseDragged`: returns new `(FFTstartIdx, FFTendIdx, dragStart)`.
    pub fn drag_fft(kind: DragType, idx: i32, start: i32, end: i32, drag_start: i32) -> (i32, i32, i32) {
        match kind {
            DragType::None => (start, end, drag_start),
            DragType::Start => {
                if idx >= 0 {
                    if end - idx >= FFT_INTERVAL_MIN {
                        (idx, end, drag_start)
                    } else {
                        (start, end, drag_start)
                    }
                } else {
                    (0, end, drag_start)
                }
            }
            DragType::End => {
                if idx <= FFT_IDX_MAX {
                    if idx - start >= FFT_INTERVAL_MIN {
                        (start, idx, drag_start)
                    } else {
                        (start, end, drag_start)
                    }
                } else {
                    (start, FFT_IDX_MAX, drag_start)
                }
            }
            DragType::Interval => {
                let d = idx - drag_start;
                if d + start >= 0 && d + end <= FFT_IDX_MAX {
                    (start + d, end + d, idx)
                } else {
                    (start, end, drag_start)
                }
            }
        }
    }

    /// `Math.round(float)`
    pub fn jround_f(x: f32) -> i32 {
        if x.is_nan() {
            0
        } else {
            (x + 0.5).floor() as i32
        }
    }

    /// `Math.round(double)`
    pub fn jround_d(x: f64) -> i64 {
        if x.is_nan() {
            0
        } else {
            (x + 0.5).floor() as i64
        }
    }

    fn java_fp(abs: f64, debug: String, sci: String) -> String {
        if abs == 0.0 || (1e-3..1e7).contains(&abs) {
            debug
        } else {
            // Java: "1.0E7", Rust: "1E7"
            match sci.split_once('E') {
                Some((m, e)) if !m.contains('.') => format!("{m}.0E{e}"),
                _ => sci,
            }
        }
    }

    /// `Float.toString`
    pub fn java_float(x: f32) -> String {
        if x.is_nan() {
            return "NaN".into();
        }
        if x.is_infinite() {
            return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
        }
        java_fp(x.abs() as f64, format!("{x:?}"), format!("{x:E}"))
    }

    /// `Double.toString`
    pub fn java_double(x: f64) -> String {
        if x.is_nan() {
            return "NaN".into();
        }
        if x.is_infinite() {
            return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
        }
        java_fp(x.abs(), format!("{x:?}"), format!("{x:E}"))
    }

    /// "Samplerate: " text.
    pub fn fmt_sample_rate(sr: i64) -> String {
        if sr < 3000 {
            format!("{sr} Hz")
        } else if sr < 900_000 {
            format!("{} KHz", jround_f(0.001f32 * sr as f32))
        } else {
            format!("{} MHz", jround_f(1.0e-6f32 * sr as f32))
        }
    }

    /// "Signallength: " text: `round(1000f / sampleRate * 2048 * 2 * 100) / 100.0 + " ms"`.
    pub fn fmt_signal_length_total(sr: i64) -> String {
        if sr <= 0 {
            return "–".into();
        }
        let dt = 1000.0f32 / sr as f32;
        format!("{} ms", java_double(jround_f(dt * 2048.0 * 2.0 * 100.0) as f64 / 100.0))
    }

    /// "[frequency:]" text of the spectrum source.
    pub fn fmt_f_range(f_range: f64) -> String {
        let f = f_range as f32;
        if f < 3000.0 {
            format!("{} Hz", java_float(f))
        } else if f < 900_000.0 {
            format!("{} KHz", jround_f(0.001f32 * f))
        } else {
            format!("{} MHz", jround_f(1.0e-6f32 * f))
        }
    }

    /// `MessPunkt.signalLength.Caption` value of the signal source: `2048000f / sampleRate`.
    pub fn fmt_signal_caption(sr: i64) -> String {
        if sr <= 0 {
            return "–".into();
        }
        let f = 2_048_000.0f32 / sr as f32;
        if (f as f64) < 0.3 {
            format!("{} µs", java_float(1000.0 * f))
        } else {
            format!("{} ms", java_float(f))
        }
    }

    /// `MessageFormat` rendering of a number argument (`NumberFormat.getInstance(locale)`:
    /// grouping, at most 3 fraction digits).
    pub fn java_msg_number(x: f64, lang: Language) -> String {
        let (group, dec) = match lang {
            Language::English => (',', '.'),
            Language::German | Language::Turkish => ('.', ','),
        };
        if !x.is_finite() {
            return if x.is_nan() { "\u{FFFD}".into() } else if x > 0.0 { "\u{221E}".into() } else { "-\u{221E}".into() };
        }
        let s = format!("{:.3}", x.abs());
        let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
        let frac = frac.trim_end_matches('0');
        let mut grouped = String::new();
        for (n, ch) in int.chars().enumerate() {
            if n > 0 && (int.len() - n) % 3 == 0 {
                grouped.push(group);
            }
            grouped.push(ch);
        }
        let neg = x < 0.0 && (int != "0" || !frac.is_empty());
        let mut out = String::new();
        if neg {
            out.push('-');
        }
        out.push_str(&grouped);
        if !frac.is_empty() {
            out.push(dec);
            out.push_str(frac);
        }
        out
    }

    /// "Nice" tick values (1/2/5 × 10^n) covering `[lo, hi]`; returns ticks and the step.
    pub fn nice_ticks(lo: f64, hi: f64, target: usize) -> (Vec<f64>, f64) {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let range = hi - lo;
        if !(range.is_finite() && range > 0.0) {
            return (vec![lo], 1.0);
        }
        let raw = range / target.max(1) as f64;
        let mag = 10f64.powf(raw.log10().floor());
        let norm = raw / mag;
        let step = mag
            * if norm <= 1.0 {
                1.0
            } else if norm <= 2.0 {
                2.0
            } else if norm <= 5.0 {
                5.0
            } else {
                10.0
            };
        let mut v = (lo / step).ceil() * step;
        let mut out = Vec::new();
        while v <= hi + step * 1e-9 && out.len() < 1000 {
            out.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
            v += step;
        }
        (out, step)
    }

    pub fn decimals_for(step: f64) -> usize {
        if step >= 1.0 || step <= 0.0 || !step.is_finite() {
            0
        } else {
            ((-step.log10()).ceil() as usize).min(6)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const B1: BlockParams = BlockParams { size: 1, average: true };

        #[test]
        fn get_value_block1_and_fallbacks() {
            let v: Vec<i64> = (0..2048).collect();
            assert_eq!(get_value(&v, 5, B1), 5);
            assert_eq!(get_value(&v, 2048, B1), 2047);
        }

        #[test]
        fn get_value_blocks() {
            let v: Vec<i64> = (0..2048).collect();
            let avg = BlockParams { size: 4, average: true };
            let max = BlockParams { size: 4, average: false };
            // round(5/4)=1 -> base 4: 4,5,6,7
            assert_eq!(get_value(&v, 5, avg), (4 + 5 + 6 + 7) / 4);
            assert_eq!(get_value(&v, 5, max), 7);
            // round(6/4 = 1.5) = 2 -> base 8
            assert_eq!(get_value(&v, 6, max), 11);
            // base 2048 is out of bounds and i < 2048 -> values[0] (Java AIOOBE fallback)
            assert_eq!(get_value(&v, 2047, max), 0);
            assert_eq!(get_value(&v, 2048, max), 2047);
            // max starts at 0
            let neg = vec![-5i64; 2048];
            assert_eq!(get_value(&neg, 0, max), 0);
        }

        #[test]
        fn data_valid() {
            assert!(!is_data_valid(&[5; 2048]));
            assert!(!is_data_valid(&[]));
            let mut v = vec![0i64; 2048];
            v[3] = 11;
            assert!(is_data_valid(&v));
            v[3] = 10;
            assert!(!is_data_valid(&v));
        }

        #[test]
        fn db_scale() {
            let f = spectrum_factor(-15.0, 40.0);
            let s = spectrum_shift(-15.0, 40.0);
            assert_eq!(db_label(256, s, f), "40.0");
            assert_eq!(db_label(0, s, f), "-15.0");
            assert_eq!(db_label(224, s, f), "33.13");
        }

        #[test]
        fn fft_dragging() {
            assert_eq!(press_drag_type(-1, 0, 4096), DragType::Start);
            assert_eq!(press_drag_type(100, 200, 4096), DragType::Start);
            assert_eq!(press_drag_type(300, 0, 200), DragType::End);
            assert_eq!(press_drag_type(100, 0, 200), DragType::Interval);
            // start edge keeps min width 64
            assert_eq!(drag_fft(DragType::Start, 100, 0, 200, 0), (100, 200, 0));
            assert_eq!(drag_fft(DragType::Start, 150, 0, 200, 0), (0, 200, 0));
            assert_eq!(drag_fft(DragType::Start, -3, 50, 200, 0), (0, 200, 0));
            // end edge
            assert_eq!(drag_fft(DragType::End, 1000, 0, 4096, 0), (0, 1000, 0));
            assert_eq!(drag_fft(DragType::End, 5000, 0, 1000, 0), (0, 4096, 0));
            assert_eq!(drag_fft(DragType::End, 30, 0, 1000, 0), (0, 1000, 0));
            // interval move
            assert_eq!(drag_fft(DragType::Interval, 120, 0, 200, 100), (20, 220, 120));
            assert_eq!(drag_fft(DragType::Interval, 50, 0, 200, 100), (0, 200, 100));
        }

        #[test]
        fn java_strings() {
            assert_eq!(java_float(8000.0), "8000.0");
            assert_eq!(java_float(0.0625), "0.0625");
            assert_eq!(java_float(1.0e7), "1.0E7");
            assert_eq!(fmt_sample_rate(16000), "16 KHz");
            assert_eq!(fmt_sample_rate(2000), "2000 Hz");
            assert_eq!(fmt_sample_rate(1_000_000), "1 MHz");
            assert_eq!(fmt_signal_length_total(16000), "256.0 ms");
            assert_eq!(fmt_f_range(8000.0), "8 KHz");
            assert_eq!(fmt_f_range(2000.0), "2000.0 Hz");
            assert_eq!(fmt_signal_caption(16000), "128.0 ms");
            assert_eq!(java_msg_number(1024.0, Language::English), "1,024");
            assert_eq!(java_msg_number(1234.5678, Language::German), "1.234,568");
            assert_eq!(java_msg_number(12.3, Language::English), "12.3");
            assert_eq!(java_msg_number(-0.5, Language::English), "-0.5");
        }

        #[test]
        fn ticks() {
            let (t, step) = nice_ticks(0.0, 8.0, 8);
            assert_eq!(step, 1.0);
            assert_eq!(t.len(), 9);
            assert_eq!(decimals_for(0.2), 1);
            assert_eq!(decimals_for(0.05), 2);
            assert_eq!(decimals_for(5.0), 0);
        }
    }
}
