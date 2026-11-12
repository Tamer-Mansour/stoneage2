//! Central view area: Map / 3D / Areal / Row / Point views with the colour `ScaleBar` on the right
//! (original: `ScalePanel` at BorderLayout.EAST), and the two Soundtest tabs
//! (`AudionInputPanel`: `AudioCapture.readData` -> `FastFourierTransform.fftMag` -> plot).

use std::time::Duration;

use stoneage_core::fft::fft_mag;
use stoneage_device::AudioCapture;
use stoneage_plots::{SignalPlot, ViewResponse};

use super::icons::IconButton;
use super::{t, trace};
use crate::app::Views;
use crate::state::{AppState, Command, Tab};

pub const SCALE_BAR_WIDTH: f32 = 92.0;

pub fn central(ui: &mut egui::Ui, state: &mut AppState, views: &mut Views) {
    state.status_hover = None;
    let frame = egui::Frame::central_panel(ui.style()).inner_margin(8);
    let tab = state.tab;
    let resp = egui::CentralPanel::default_margins()
        .frame(frame)
        .show(ui, |ui| match tab {
            Tab::Soundtest => {
                views.sound1.ui(ui, state, false);
                ViewResponse::default()
            }
            Tab::Soundtest2 => {
                views.sound2.ui(ui, state, true);
                ViewResponse::default()
            }
            _ => survey_view(ui, state, views, tab),
        })
        .inner;
    apply(state, resp);
}

fn survey_view(ui: &mut egui::Ui, state: &mut AppState, views: &mut Views, tab: Tab) -> ViewResponse {
    let scale = egui::Panel::right("stoneage_scale_bar")
        .resizable(false)
        .exact_size(SCALE_BAR_WIDTH)
        .frame(egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 0, top: 0, bottom: 0 }))
        .show(ui, |ui| views.scale.ui(ui, &mut state.survey, &state.i18n))
        .inner;
    let main = egui::CentralPanel::no_frame()
        .show(ui, |ui| match tab {
            Tab::Map => views.map.ui(ui, &mut state.survey, &state.i18n),
            Tab::View3d => views.view3d.ui(ui, &state.survey, &state.i18n),
            Tab::Areal => views.areal.ui(ui, &mut state.survey, &state.i18n),
            Tab::Row => views.row.ui(ui, &mut state.survey, &state.i18n),
            _ => views.point.ui(ui, &mut state.survey, &state.i18n, state.demo_mode),
        })
        .inner;
    merge(main, scale)
}

fn merge(a: ViewResponse, b: ViewResponse) -> ViewResponse {
    ViewResponse {
        changed: a.changed || b.changed,
        hover_info: a.hover_info.or(b.hover_info),
        select_row: a.select_row.or(b.select_row),
        select_point: a.select_point.or(b.select_point),
    }
}

/// Applies what a view reported: dirty flag, status-bar hover text, row/point selection.
pub fn apply(state: &mut AppState, r: ViewResponse) {
    if r.changed {
        state.dirty = true;
    }
    if r.hover_info.is_some() {
        state.status_hover = r.hover_info;
    }
    if let Some(row) = r.select_row {
        state.push(Command::SelectRow(row));
    }
    if let Some(p) = r.select_point {
        state.push(Command::SelectPoint(p));
    }
}

/// Largest power of two `<= n` (the FFT needs a power-of-two length).
pub fn pow2_floor(n: usize) -> usize {
    if n == 0 { 0 } else { 1 << (usize::BITS - 1 - n.leading_zeros()) }
}

/// One Soundtest tab (`AudionInputPanel`).
#[derive(Default)]
pub struct SoundPane {
    pub signal: Vec<f32>,
    pub spectrum: Vec<f32>,
    pub live: bool,
    plot: SignalPlot,
}

impl SoundPane {
    /// `continuous == false`: one acquisition per click (`aquireData`); `true`: live acquisition.
    pub fn ui(&mut self, ui: &mut egui::Ui, state: &mut AppState, continuous: bool) {
        let mut acquire = false;
        ui.horizontal(|ui| {
            if continuous {
                let label = t(&state.i18n, "App.Sound.Live", "Live");
                let icon = if self.live { "circle-stop" } else { "circle-play" };
                if ui.add(IconButton::new(icon, label).selected(self.live)).clicked() {
                    self.live = !self.live;
                }
            } else if ui.add(IconButton::new("circle-dot", t(&state.i18n, "App.Sound.Acquire", "Acquire")).primary()).clicked() {
                acquire = true;
            }
            let open = state.audio.is_some();
            ui.add_enabled_ui(open, |ui| {
                if ui.add(IconButton::new("x", t(&state.i18n, "App.Sound.Close", "Close audio input"))).clicked() {
                    if let Some(a) = state.audio.as_mut() {
                        a.close();
                    }
                    state.audio = None;
                    self.live = false;
                }
            });
            ui.separator();
            let info = if self.signal.is_empty() {
                t(&state.i18n, "App.Sound.NoData", "No signal acquired yet")
            } else {
                format!("signal length = {} · FFT {}", self.signal.len(), self.spectrum.len())
            };
            ui.label(egui::RichText::new(info).weak());
        });
        ui.add_space(4.0);

        if acquire || (continuous && self.live) {
            self.acquire(state, !continuous);
        }
        if continuous && self.live {
            ui.ctx().request_repaint_after(Duration::from_millis(40));
        }
        self.plot.ui(ui, &self.signal, &self.spectrum);
    }

    /// `AudionInputPanel.aquireData`: signal = AudioCapture.readData(); spectrum = fftMag(signal).
    fn acquire(&mut self, state: &mut AppState, log_it: bool) {
        if state.audio.is_none() {
            match AudioCapture::open() {
                Ok(a) => state.audio = Some(a),
                Err(e) => {
                    trace::error(state, "audioPanel", format!("openLine: {e}"));
                    state.toast(format!("⚠ {e}"));
                    self.live = false;
                    return;
                }
            }
        }
        let Some(audio) = state.audio.as_mut() else { return };
        self.signal = audio.read_data();
        if log_it {
            let msg = format!("signal length = {}", self.signal.len());
            trace::trace(state, "audioPanel", msg);
        }
        let n = pow2_floor(self.signal.len());
        self.spectrum = if n >= 2 { fft_mag(&self.signal[..n]) } else { Vec::new() };
    }
}

#[cfg(test)]
mod tests {
    use super::pow2_floor;

    #[test]
    fn pow2() {
        assert_eq!(pow2_floor(0), 0);
        assert_eq!(pow2_floor(1), 1);
        assert_eq!(pow2_floor(2048), 2048);
        assert_eq!(pow2_floor(3000), 2048);
    }
}
