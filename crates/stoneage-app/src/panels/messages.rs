//! Messages section: the tracer log (original "Messages" tab / `Tracer`) with level filter,
//! text filter, debug toggle, clear and copy-all.

use egui::{Color32, RichText, Ui};

use super::{err_color, mem, mem_set, tr, trf, warn_color, IconButton};
use crate::state::AppState;

#[derive(Clone)]
struct LogFilter {
    /// Show entries with level <= this (Error < Warn < Info < Debug < Trace).
    max_level: log::Level,
    search: String,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self { max_level: log::Level::Trace, search: String::new() }
    }
}

fn level_color(ui: &Ui, l: log::Level) -> Color32 {
    match l {
        log::Level::Error => err_color(ui),
        log::Level::Warn => warn_color(ui),
        log::Level::Info => ui.visuals().text_color(),
        log::Level::Debug | log::Level::Trace => ui.visuals().weak_text_color(),
    }
}

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    let mut f: LogFilter = mem(ui, "log_filter");

    ui.horizontal(|ui| {
        ui.label(tr(state, "App.Messages.Level"));
        egui::ComboBox::from_id_salt("app-panels-log-level")
            .selected_text(level_name(state, f.max_level))
            .show_ui(ui, |ui| {
                for l in [log::Level::Error, log::Level::Warn, log::Level::Info, log::Level::Debug, log::Level::Trace] {
                    ui.selectable_value(&mut f.max_level, l, level_name(state, l));
                }
            });
    });
    let w = ui.available_width();
    super::search_field(ui, &mut f.search, &tr(state, "App.Common.Search"), w);
    let (dbg_label, dbg_tip) = (tr(state, "App.Messages.Debug"), tr(state, "App.Messages.Debug.ToolTip"));
    ui.checkbox(&mut state.trace_debug, dbg_label).on_hover_text(dbg_tip);

    let needle = f.search.to_lowercase();
    let visible: Vec<usize> = state
        .log
        .iter()
        .enumerate()
        .filter(|(_, e)| e.level <= f.max_level && (needle.is_empty() || e.text.to_lowercase().contains(&needle)))
        .map(|(i, _)| i)
        .collect();

    ui.horizontal(|ui| {
        ui.label(RichText::new(trf(state, "App.Messages.Count", &[&visible.len(), &state.log.len()])).small().weak());
    });
    ui.horizontal(|ui| {
        if ui.add(IconButton::new("copy", tr(state, "App.Messages.CopyAll"))).clicked() {
            let text: String = visible.iter().map(|&i| format_entry(&state.log[i]) + "\n").collect();
            ui.ctx().copy_text(text);
            let msg = tr(state, "App.Messages.Copied");
            state.toast(msg);
        }
        if ui.add(IconButton::new("trash", tr(state, "App.Messages.Clear")).danger()).clicked() {
            state.log.clear();
        }
    });
    ui.separator();

    let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
    if visible.is_empty() {
        ui.label(RichText::new(tr(state, "App.Messages.Empty")).weak());
    }
    let h = ui.ctx().content_rect().height() - 300.0;
    egui::ScrollArea::vertical()
        .id_salt("app-panels-log")
        .max_height(h.max(220.0))
        .auto_shrink([false, true])
        .stick_to_bottom(true)
        .show_rows(ui, row_h, visible.len(), |ui, range| {
            for &i in &visible[range] {
                let e = &state.log[i];
                let c = level_color(ui, e.level);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(e.time.format("%H:%M:%S").to_string()).monospace().small().weak());
                    ui.label(RichText::new(level_tag(e.level)).monospace().small().color(c));
                    ui.add(egui::Label::new(RichText::new(&e.text).monospace().small().color(c)).truncate())
                        .on_hover_text(&e.text);
                });
            }
        });

    mem_set(ui, "log_filter", f);
}

fn level_tag(l: log::Level) -> &'static str {
    match l {
        log::Level::Error => "ERR",
        log::Level::Warn => "WRN",
        log::Level::Info => "INF",
        log::Level::Debug => "DBG",
        log::Level::Trace => "TRC",
    }
}

fn level_name(state: &AppState, l: log::Level) -> String {
    let key = match l {
        log::Level::Error => "App.Messages.Level.Error",
        log::Level::Warn => "App.Messages.Level.Warn",
        log::Level::Info => "App.Messages.Level.Info",
        log::Level::Debug => "App.Messages.Level.Debug",
        log::Level::Trace => "App.Messages.Level.Trace",
    };
    tr(state, key)
}

fn format_entry(e: &crate::state::LogEntry) -> String {
    format!("{} [{}] {}", e.time.format("%Y-%m-%d %H:%M:%S%.3f"), e.level, e.text)
}
