//! Port of `StoneAge.AudionInputPanel` + `AudionInputPanel$ViewArea` (Soundtest view).
//!
//! Original (reference/javap/AudionInputPanel*.txt):
//! * `aquireData()`: `signal = AudioCapture.readData()` (4096 raw 16-bit values),
//!   `spectrum = FastFourierTransform.fftMag(signal)` (2048 bins). Here the caller does the
//!   acquisition (see [`SignalPlot::set_acquire_time`]) and passes both arrays to [`SignalPlot::ui`].
//! * `ViewArea`: a `javax.swing.Timer(5 ms)` calls `repaint()` and restarts itself, and every
//!   `paintComponent` calls `aquireData()` first – i.e. a continuous live view.
//! * Signal (green): every 8th sample (`i += 4*2*1`), one pixel per column starting at x = 63,
//!   vertical line from baseline y = 64 to `64 - round(32 * s / max)` where `max` = largest
//!   *positive* sample (`Math.max` starting at 0). Text `"max: " + max + " "` at (32, 32).
//! * Spectrum (red): every 4th bin (`i += 4`), baseline y = 148, line up to
//!   `148 - round(20 + 10*log10(m))` – 1 px per dB, baseline = -20 dB.
//! * dB grid: for d = -40, -30 .. 30: dark-red (128,0,0) line x 63..256 at `y = 148 - 20 - d`,
//!   label `"" + d + " dB"` at x = 16.
//! * White 15 px strip at the bottom; texts `"aquire: N ms"` (10,10) and `"repaint: N ms"` (10,20).
//! * Fields `marginx = 32`, `marginy = 48`, `height = 352`, preferred size 400x400 – unused for
//!   painting.
//! * `DRAG_NONE/START/END/INTERVALL` constants, `dragType`, `dragStart` exist but **all mouse
//!   handlers are empty** in 1.49: dragging changes nothing. This port adds an optional,
//!   self-contained frequency-interval selection (no side effects outside this widget).

use std::time::{Duration, Instant};

use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::theme;
use crate::ViewResponse;

/// `ViewArea.DRAG_NONE`
pub const DRAG_NONE: i32 = 0;
/// `ViewArea.DRAG_START`
pub const DRAG_START: i32 = 1;
/// `ViewArea.DRAG_END`
pub const DRAG_END: i32 = 2;
/// `ViewArea.DRAG_INTERVALL`
pub const DRAG_INTERVALL: i32 = 3;

/// Original decimation of the signal (`4 * 2 * 1`).
pub const SIGNAL_STEP: usize = 8;
/// Original decimation of the spectrum (`4 * 1`).
pub const SPECTRUM_STEP: usize = 4;
/// Original repaint timer period.
pub const TIMER_PERIOD: Duration = Duration::from_millis(5);
/// Spectrum baseline in dB (`148 - 20` px with 1 px per dB, 0 dB at `148 - 20`).
pub const SPECTRUM_BASE_DB: f32 = -20.0;

/// Mouse drag mode (`ViewArea.dragType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragType {
    #[default]
    None = DRAG_NONE as isize,
    Start = DRAG_START as isize,
    End = DRAG_END as isize,
    Intervall = DRAG_INTERVALL as isize,
}

pub struct SignalPlot {
    /// Sample rate of the signal in Hz (for the time/frequency axes). Default 44100.
    pub sample_rate: f32,
    /// Selected frequency interval `(start, end)` in Hz on the spectrum (addition, see module doc).
    pub selection: Option<(f32, f32)>,
    /// Current drag mode (`dragType`).
    pub drag_type: DragType,
    /// Frequency where the current drag started (`dragStart`), in Hz.
    pub drag_start: f32,
    /// Duration of the last acquisition (`aquire: N ms`), set by the caller.
    pub acquire_ms: Option<f64>,
    /// Last hover read-out.
    pub hover_info: Option<String>,
    /// Keep the 5 ms refresh timer running (`javax.swing.Timer`). Default true.
    pub live: bool,
    acquire_started: Option<Instant>,
    sel_at_drag: Option<(f32, f32)>,
}

impl Default for SignalPlot {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            selection: None,
            drag_type: DragType::None,
            drag_start: 0.0,
            acquire_ms: None,
            hover_info: None,
            live: true,
            acquire_started: None,
            sel_at_drag: None,
        }
    }
}

/// Value of `"max: "` in the original: largest positive sample (0 if none).
pub fn signal_max(signal: &[f32]) -> f32 {
    signal.iter().fold(0.0f32, |m, &v| m.max(v))
}

/// Original signal stem height in pixels for a 32 px half-height: `round(32 * s / max)`.
pub fn signal_stem_px(s: f32, max: f32) -> i32 {
    java_round_f(32.0 * s / max)
}

/// Original spectrum stem height in pixels: `round(20 + 10*log10(m))` (1 px per dB above -20 dB).
pub fn spectrum_stem_px(m: f32) -> i64 {
    let v = 20.0 + 10.0 * (m as f64).log10();
    java_round_d(v)
}

/// Magnitude to dB as drawn by the original (`10*log10(m)`).
pub fn magnitude_db(m: f32) -> f32 {
    10.0 * m.log10()
}

/// Frequency of spectrum bin `k` for a spectrum of `bins` bins (`= signal_len / 2`).
pub fn bin_frequency(k: usize, bins: usize, sample_rate: f32) -> f32 {
    if bins == 0 {
        0.0
    } else {
        k as f32 * sample_rate / (2 * bins) as f32
    }
}

/// `Math.round(float)` incl. NaN -> 0 and saturation.
fn java_round_f(v: f32) -> i32 {
    if v.is_nan() {
        0
    } else {
        (v as f64 + 0.5).floor().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}

/// `Math.round(double)` incl. NaN -> 0 and saturation.
fn java_round_d(v: f64) -> i64 {
    if v.is_nan() {
        0
    } else {
        (v + 0.5).floor().clamp(i64::MIN as f64, i64::MAX as f64) as i64
    }
}

/// Java `Float.toString` for the usual range (`12.0`, `1234.5`, `1.0E7`).
fn java_float(v: f32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let a = v.abs();
    if a == 0.0 || (1e-3..1e7).contains(&a) {
        let s = format!("{v}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let s = format!("{v:E}");
        // Rust: 1.5E7 / 1E7 -> Java: 1.5E7 / 1.0E7
        match s.split_once('E') {
            Some((m, e)) if !m.contains('.') => format!("{m}.0E{e}"),
            _ => s,
        }
    }
}

/// "Nice" tick step for roughly `target` ticks over `span`.
fn nice_step(span: f32, target: f32) -> f32 {
    if span <= 0.0 || !span.is_finite() {
        return 1.0;
    }
    let raw = span / target.max(1.0);
    let mag = 10f32.powf(raw.log10().floor());
    let norm = raw / mag;
    let n = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    };
    n * mag
}

fn fmt_hz(f: f32) -> String {
    if f.abs() >= 1000.0 {
        format!("{:.2} kHz", f / 1000.0)
    } else {
        format!("{f:.0} Hz")
    }
}

const LEFT: f32 = 64.0;
const RIGHT: f32 = 16.0;
const TOP: f32 = 32.0;
const AXIS_H: f32 = 22.0;
const GAP: f32 = 16.0;
const EDGE_GRAB: f32 = 6.0;

impl SignalPlot {
    /// Call right before `AudioCapture::read_data` (start of `aquireData`).
    pub fn begin_acquire(&mut self) {
        self.acquire_started = Some(Instant::now());
    }

    /// Call right after acquisition + FFT (end of `aquireData`); shown as `aquire: N ms`.
    pub fn end_acquire(&mut self) {
        if let Some(t) = self.acquire_started.take() {
            self.acquire_ms = Some(t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// Sets the acquisition time explicitly (alternative to begin/end_acquire).
    pub fn set_acquire_time(&mut self, ms: f64) {
        self.acquire_ms = Some(ms);
    }

    /// Selected interval as spectrum bin indices `(start, end)` (inclusive), for `bins` bins.
    pub fn selected_bins(&self, bins: usize) -> Option<(usize, usize)> {
        let (a, b) = self.selection?;
        if bins == 0 || self.sample_rate <= 0.0 {
            return None;
        }
        let df = self.sample_rate / (2 * bins) as f32;
        let ia = ((a.min(b) / df).round().max(0.0) as usize).min(bins - 1);
        let ib = ((a.max(b) / df).round().max(0.0) as usize).min(bins - 1);
        Some((ia, ib))
    }

    /// Draws `signal` (top) and its magnitude `spectrum` (bottom), with the interval dragging
    /// of `AudionInputPanel$ViewArea`.
    pub fn ui(&mut self, ui: &mut egui::Ui, signal: &[f32], spectrum: &[f32]) -> ViewResponse {
        let paint_start = Instant::now();
        let mut resp_out = ViewResponse::default();

        let avail = ui.available_size();
        let size = Vec2::new(avail.x.max(400.0), avail.y.max(400.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let vis = ui.visuals();
        let text_col = vis.text_color();
        let weak_col = vis.weak_text_color();
        let grid_col = weak_col.gamma_multiply(theme::GRID_ALPHA as f32 / 255.0);
        let axis_col = weak_col.gamma_multiply(0.6);
        let bg = vis.extreme_bg_color;
        let small = FontId::proportional(11.0);
        let mono = FontId::monospace(11.0);

        painter.rect_filled(rect, 8.0, bg);
        painter.rect_stroke(rect, 8.0, Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);

        let sr = if self.sample_rate > 0.0 { self.sample_rate } else { 44100.0 };

        // ---- layout --------------------------------------------------------------------
        let plot_w = rect.width() - LEFT - RIGHT;
        let body_h = rect.height() - TOP - 2.0 * AXIS_H - GAP - 8.0;
        let sig_h = (body_h * 0.4).max(60.0);
        let spec_h = (body_h - sig_h).max(80.0);
        let sig_rect = Rect::from_min_size(rect.min + Vec2::new(LEFT, TOP), Vec2::new(plot_w, sig_h));
        let spec_rect = Rect::from_min_size(
            Pos2::new(sig_rect.left(), sig_rect.bottom() + AXIS_H + GAP),
            Vec2::new(plot_w, spec_h),
        );

        // ---- header: max / aquire / repaint texts -----------------------------------------
        let max = signal_max(signal);
        painter.text(
            rect.min + Vec2::new(12.0, 10.0),
            Align2::LEFT_TOP,
            format!("max: {} ", java_float(max)),
            mono.clone(),
            text_col,
        );
        if let Some(a) = self.acquire_ms {
            painter.text(
                Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
                Align2::RIGHT_TOP,
                format!("aquire: {:.0} ms", a),
                mono.clone(),
                weak_col,
            );
        }

        // ---- signal panel --------------------------------------------------------------------
        painter.rect_stroke(sig_rect, 4.0, Stroke::new(1.0, axis_col), StrokeKind::Inside);
        let n = signal.len();
        let dur_ms = n as f32 / sr * 1000.0;
        let mid_y = sig_rect.center().y;
        let half = sig_rect.height() * 0.5 - 2.0;
        // original: +-32 px for +-max in a 64 px half-height region -> +-50 % of the half height
        let amp_px = half * 0.5 * 2.0 * 0.9;
        let y_of_sig = |s: f32| -> f32 {
            let v = if max > 0.0 { s / max } else { 0.0 };
            (mid_y - v * amp_px).clamp(sig_rect.top() + 1.0, sig_rect.bottom() - 1.0)
        };
        // y grid (+-max, 0)
        painter.hline(sig_rect.x_range(), mid_y, Stroke::new(1.0, grid_col));
        for v in [max, -max] {
            if max > 0.0 {
                let y = y_of_sig(v);
                painter.hline(sig_rect.x_range(), y, Stroke::new(1.0, grid_col));
                painter.text(Pos2::new(sig_rect.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{v:.0}"), small.clone(), weak_col);
            }
        }
        painter.text(Pos2::new(sig_rect.left() - 6.0, mid_y), Align2::RIGHT_CENTER, "0", small.clone(), weak_col);
        // time ticks
        if n > 0 {
            let step = nice_step(dur_ms, plot_w / 90.0);
            let mut t = 0.0;
            while t <= dur_ms + 1e-3 {
                let x = sig_rect.left() + t / dur_ms * plot_w;
                painter.vline(x, sig_rect.y_range(), Stroke::new(1.0, grid_col));
                painter.text(Pos2::new(x, sig_rect.bottom() + 3.0), Align2::CENTER_TOP, format!("{t:.0}"), small.clone(), weak_col);
                t += step;
            }
            painter.text(Pos2::new(sig_rect.right(), sig_rect.bottom() + 3.0 + 12.0), Align2::RIGHT_TOP, "t [ms]", small.clone(), weak_col);
        }
        // stems: every 8th sample, from the zero line to the value
        let sig_stroke = Stroke::new(1.0, theme::MARKER);
        let mut i = 0;
        while i < n {
            let x = sig_rect.left() + i as f32 / n as f32 * plot_w;
            painter.line_segment([Pos2::new(x, mid_y), Pos2::new(x, y_of_sig(signal[i]))], sig_stroke);
            i += SIGNAL_STEP;
        }

        // ---- spectrum panel ---------------------------------------------------------------------
        painter.rect_stroke(spec_rect, 4.0, Stroke::new(1.0, axis_col), StrokeKind::Inside);
        let bins = spectrum.len();
        let f_max = bin_frequency(bins, bins, sr);
        let data_max_db = spectrum
            .iter()
            .step_by(SPECTRUM_STEP)
            .map(|&m| magnitude_db(m))
            .filter(|v| v.is_finite())
            .fold(f32::NEG_INFINITY, f32::max);
        let db_lo = -45.0f32;
        let db_hi = if data_max_db.is_finite() { ((data_max_db / 10.0).ceil() * 10.0 + 5.0).max(35.0) } else { 35.0 };
        let y_of_db = |d: f32| -> f32 {
            let t = (d - db_lo) / (db_hi - db_lo);
            (spec_rect.bottom() - t * spec_rect.height()).clamp(spec_rect.top() + 1.0, spec_rect.bottom() - 1.0)
        };
        let x_of_f = |f: f32| -> f32 {
            if f_max > 0.0 { spec_rect.left() + f / f_max * plot_w } else { spec_rect.left() }
        };
        let f_of_x = |x: f32| -> f32 {
            if plot_w > 0.0 { ((x - spec_rect.left()) / plot_w).clamp(0.0, 1.0) * f_max } else { 0.0 }
        };

        // selection band (below the data)
        if let Some((a, b)) = self.selection {
            let (lo, hi) = (a.min(b), a.max(b));
            let band = Rect::from_x_y_ranges(x_of_f(lo)..=x_of_f(hi), spec_rect.y_range());
            painter.rect_filled(band, 0.0, theme::MARKER.gamma_multiply(0.15));
            painter.vline(band.left(), spec_rect.y_range(), Stroke::new(1.5, theme::MARKER));
            painter.vline(band.right(), spec_rect.y_range(), Stroke::new(1.5, theme::MARKER));
        }

        // dB grid every 10 dB (original: -40 .. 30 dB, dark red)
        let mut d = ((db_lo / 10.0).ceil() * 10.0) as i32;
        while d as f32 <= db_hi {
            let y = y_of_db(d as f32);
            painter.hline(spec_rect.x_range(), y, Stroke::new(1.0, grid_col));
            painter.text(Pos2::new(spec_rect.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{d} dB"), small.clone(), weak_col);
            d += 10;
        }
        // frequency ticks
        if f_max > 0.0 {
            let step = nice_step(f_max, plot_w / 90.0);
            let mut f = 0.0;
            while f <= f_max + 1e-3 {
                let x = x_of_f(f);
                painter.vline(x, spec_rect.y_range(), Stroke::new(1.0, grid_col));
                let lbl = if step >= 1000.0 { format!("{:.0}k", f / 1000.0) } else if f_max >= 1000.0 { format!("{:.1}k", f / 1000.0) } else { format!("{f:.0}") };
                painter.text(Pos2::new(x, spec_rect.bottom() + 3.0), Align2::CENTER_TOP, lbl, small.clone(), weak_col);
                f += step;
            }
            painter.text(Pos2::new(spec_rect.right(), spec_rect.bottom() + 15.0), Align2::RIGHT_TOP, "f [Hz]", small.clone(), weak_col);
        }
        // stems: every 4th bin, from the -20 dB baseline (original baseline y = 148)
        let base_y = y_of_db(SPECTRUM_BASE_DB);
        let spec_stroke = Stroke::new(1.0, theme::ACCENT);
        let mut k = 0;
        while k < bins {
            let db = magnitude_db(spectrum[k]);
            if db.is_finite() || db == f32::NEG_INFINITY {
                let x = x_of_f(bin_frequency(k, bins, sr));
                let y = if db.is_finite() { y_of_db(db) } else { spec_rect.bottom() - 1.0 };
                painter.line_segment([Pos2::new(x, base_y), Pos2::new(x, y)], spec_stroke);
            }
            k += SPECTRUM_STEP;
        }

        // selection peak read-out
        if let Some((ia, ib)) = self.selected_bins(bins) {
            if let Some((pk, pm)) = spectrum[ia..=ib]
                .iter()
                .enumerate()
                .fold(None::<(usize, f32)>, |acc, (j, &m)| match acc {
                    Some((_, bm)) if bm >= m => acc,
                    _ => Some((ia + j, m)),
                })
            {
                let (a, b) = self.selection.unwrap_or_default();
                let txt = format!(
                    "{} – {}   peak {} @ {:.1} dB",
                    fmt_hz(a.min(b)),
                    fmt_hz(a.max(b)),
                    fmt_hz(bin_frequency(pk, bins, sr)),
                    magnitude_db(pm)
                );
                let pos = Pos2::new(spec_rect.left() + 8.0, spec_rect.top() + 6.0);
                let galley = painter.layout_no_wrap(txt, small.clone(), text_col);
                let r = Rect::from_min_size(pos, galley.size()).expand(4.0);
                painter.rect_filled(r, 4.0, bg.gamma_multiply(0.9));
                painter.galley(pos, galley, text_col);
            }
        }

        // ---- interaction: DRAG_START / DRAG_END / DRAG_INTERVALL --------------------------------
        let in_spec = |p: Pos2| spec_rect.expand2(Vec2::new(0.0, 4.0)).contains(p);
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if in_spec(p) {
                    self.selection = None;
                    resp_out.changed = true;
                }
            }
        }
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                if in_spec(p) {
                    let f = f_of_x(p.x);
                    self.drag_start = f;
                    self.sel_at_drag = self.selection;
                    self.drag_type = match self.selection {
                        Some((a, b)) => {
                            let (xa, xb) = (x_of_f(a), x_of_f(b));
                            if (p.x - xa).abs() <= EDGE_GRAB {
                                DragType::Start
                            } else if (p.x - xb).abs() <= EDGE_GRAB {
                                DragType::End
                            } else if p.x > xa.min(xb) && p.x < xa.max(xb) {
                                DragType::Intervall
                            } else {
                                self.selection = Some((f, f));
                                DragType::End
                            }
                        }
                        None => {
                            self.selection = Some((f, f));
                            DragType::End
                        }
                    };
                }
            }
        }
        if resp.dragged() && self.drag_type != DragType::None {
            if let (Some(p), Some((a, b))) = (resp.interact_pointer_pos(), self.selection) {
                let f = f_of_x(p.x);
                self.selection = Some(match self.drag_type {
                    DragType::Start => (f, b),
                    DragType::End => (a, f),
                    DragType::Intervall => {
                        let (a0, b0) = self.sel_at_drag.unwrap_or((a, b));
                        let (lo, hi) = (a0.min(b0), a0.max(b0));
                        let df = (f - self.drag_start).clamp(-lo, f_max - hi);
                        (lo + df, hi + df)
                    }
                    DragType::None => (a, b),
                });
                resp_out.changed = true;
            }
        }
        if resp.drag_stopped() && self.drag_type != DragType::None {
            if let Some((a, b)) = self.selection {
                self.selection = if (x_of_f(a) - x_of_f(b)).abs() < 2.0 { None } else { Some((a.min(b), a.max(b))) };
            }
            self.drag_type = DragType::None;
            self.sel_at_drag = None;
        }

        // ---- hover read-out -----------------------------------------------------------------
        self.hover_info = None;
        if let Some(p) = resp.hover_pos() {
            if sig_rect.contains(p) && n > 0 {
                let i = (((p.x - sig_rect.left()) / plot_w) * n as f32).round().clamp(0.0, (n - 1) as f32) as usize;
                painter.vline(p.x, sig_rect.y_range(), Stroke::new(1.0, text_col.gamma_multiply(0.4)));
                self.hover_info = Some(format!("t = {:.2} ms   value = {:.0}", i as f32 / sr * 1000.0, signal[i]));
            } else if spec_rect.contains(p) && bins > 0 {
                let f = f_of_x(p.x);
                let df = sr / (2 * bins) as f32;
                let k = ((f / df).round() as usize).min(bins - 1);
                painter.vline(p.x, spec_rect.y_range(), Stroke::new(1.0, text_col.gamma_multiply(0.4)));
                self.hover_info = Some(format!(
                    "f = {}   {:.1} dB   (bin {k})",
                    fmt_hz(bin_frequency(k, bins, sr)),
                    magnitude_db(spectrum[k])
                ));
                if self.drag_type == DragType::None {
                    if let Some((a, b)) = self.selection {
                        if (p.x - x_of_f(a)).abs() <= EDGE_GRAB || (p.x - x_of_f(b)).abs() <= EDGE_GRAB {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        } else if p.x > x_of_f(a.min(b)) && p.x < x_of_f(a.max(b)) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                    }
                }
            }
            if let Some(h) = &self.hover_info {
                let g = painter.layout_no_wrap(h.clone(), small.clone(), text_col);
                let pos = Pos2::new(rect.left() + 12.0, rect.bottom() - 6.0 - g.size().y);
                painter.galley(pos, g, text_col);
            }
        }
        resp_out.hover_info = self.hover_info.clone();

        // "repaint: N ms" = acquisition + painting time, like the original
        let repaint_ms = self.acquire_ms.unwrap_or(0.0) + paint_start.elapsed().as_secs_f64() * 1000.0;
        painter.text(
            Pos2::new(rect.right() - 12.0, rect.top() + 22.0),
            Align2::RIGHT_TOP,
            format!("repaint: {repaint_ms:.0} ms"),
            mono,
            weak_col,
        );

        // javax.swing.Timer(5 ms) -> continuous refresh
        if self.live {
            ui.ctx().request_repaint_after(TIMER_PERIOD);
        }
        resp_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_pixel_formulas() {
        assert_eq!(signal_stem_px(100.0, 100.0), 32);
        assert_eq!(signal_stem_px(-50.0, 100.0), -16);
        assert_eq!(signal_stem_px(0.0, 0.0), 0); // NaN -> 0
        assert_eq!(spectrum_stem_px(1.0), 20);
        assert_eq!(spectrum_stem_px(10.0), 30);
        assert_eq!(spectrum_stem_px(0.01), 0);
    }

    #[test]
    fn max_is_positive_only() {
        assert_eq!(signal_max(&[-500.0, -3.0]), 0.0);
        assert_eq!(signal_max(&[-500.0, 7.0, 3.0]), 7.0);
    }

    #[test]
    fn frequencies_and_bins() {
        assert_eq!(bin_frequency(0, 2048, 44100.0), 0.0);
        assert!((bin_frequency(2048, 2048, 44100.0) - 22050.0).abs() < 1e-3);
        let p = SignalPlot { selection: Some((1000.0, 500.0)), ..Default::default() };
        let (a, b) = p.selected_bins(2048).unwrap();
        assert!(a < b && b < 2048);
        assert_eq!(a, (500.0f32 / (44100.0 / 4096.0)).round() as usize);
    }

    #[test]
    fn java_float_format() {
        assert_eq!(java_float(0.0), "0.0");
        assert_eq!(java_float(12345.0), "12345.0");
        assert_eq!(java_float(1.5), "1.5");
        assert_eq!(java_float(1.0e7), "1.0E7");
    }

    #[test]
    fn drag_constants() {
        assert_eq!(DragType::Start as i32, DRAG_START);
        assert_eq!(DragType::End as i32, DRAG_END);
        assert_eq!(DragType::Intervall as i32, DRAG_INTERVALL);
        assert_eq!(DragType::None as i32, DRAG_NONE);
    }
}
