//! 2-D measurement views. OWNER: agent "plots".
//!
//! * `PointView`  – port of `MessPunkt` + `MessPunkt$ViewArea` (single point spectrum/signal, FFT
//!   interval dragging, zoom, horizontal-cut marker).
//! * `RowView`    – port of `MessReihe` + `$ViewArea/$ColHeader/$RowHeader` (colour-coded section of
//!   one row: X = point, Y = frequency or depth, valid base/step, interpolation, block average, grid).
//! * `ScaleBar`   – port of `ScalePanel` (colour legend, drag to shift `color_model_base`, colour-model slider).
//! * `SignalPlot` – port of `AudionInputPanel$ViewArea` (signal + spectrum plot used by Soundtest tabs).
//!
//! All widgets are immediate-mode: `fn ui(&mut self, ui: &mut egui::Ui, ...) -> ViewResponse`.
//! They never own the Survey; they read/modify it through `&mut Survey` and report user
//! intents back through `ViewResponse` so the app can update the status bar.
//!
//! CONTRACT: names below are used by stoneage-app while being written in parallel.
//! Add freely; do not rename/remove.

pub mod point_view;
pub mod row_view;
pub mod scale_bar;
pub mod signal_plot;
pub mod theme;

pub use point_view::PointView;
pub use row_view::RowView;
pub use scale_bar::ScaleBar;
pub use signal_plot::SignalPlot;

/// What happened inside a view this frame.
#[derive(Default, Debug, Clone)]
pub struct ViewResponse {
    /// Survey data or display parameters were changed by the view (mark document dirty).
    pub changed: bool,
    /// Text for the status bar describing what is under the mouse (value, freq/depth, position).
    pub hover_info: Option<String>,
    /// The user selected another row / point by clicking in the view.
    pub select_row: Option<usize>,
    pub select_point: Option<usize>,
}
