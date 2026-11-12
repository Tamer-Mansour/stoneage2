//! Sidebar section contents. OWNER: agent "app-panels".
//! Entry point used by the shell: `panels::sidebar_section(ui, state, section)`.
//!
//! The controls of the original `OptionPanel` (parameter dialog) are spread over the
//! Survey / Display / Measure sections; `option_panel.rs` holds the faithful port of its
//! logic (`validKb`, `validfRange`, `loadValues`, `setValues`, `updateDynText`).

mod data;
mod display;
mod files;
mod measure;
mod messages;
mod option_panel;
mod strings;
mod survey;
mod updates;

use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui, Vec2};

pub(crate) use crate::shell::icons::{self, IconButton};
use crate::shell::theme::{card_shadow, semibold, Palette, SECTION_SIZE};

use crate::state::{AppState, SidebarSection};

/// Width of the label column in every form row (consistent alignment across sections).
pub(crate) const LABEL_W: f32 = 118.0;
/// Height of a form row.
pub(crate) const ROW_H: f32 = 22.0;

/// Content of one sidebar section. The shell draws the section title and wraps this in a
/// vertical `ScrollArea`; nested lists (rows, points, log, help topics) have bounded heights.
pub fn sidebar_section(ui: &mut Ui, state: &mut AppState, section: SidebarSection) {
    ui.set_width(ui.available_width());
    ui.add_space(4.0);
    match section {
        SidebarSection::Files => files::show(ui, state),
        SidebarSection::Measure => measure::show(ui, state),
        SidebarSection::Survey => survey::show(ui, state),
        SidebarSection::Display => display::show(ui, state),
        SidebarSection::Data => data::show(ui, state),
        SidebarSection::Help => crate::help::show(ui, state),
        SidebarSection::Messages => messages::show(ui, state),
        SidebarSection::Updates => updates::show(ui, state),
    }
    ui.add_space(12.0);
}

// ---------------------------------------------------------------------------------------------
// i18n helpers

/// Translate `key`; for new `App.*` keys not yet present in the property files a built-in
/// de/en/tr fallback (see `strings.rs`, listed in docs/NOTES-app-panels.md) is used.
pub(crate) fn tr(state: &AppState, key: &str) -> String {
    let s = state.i18n.tr(key);
    if s == key {
        if let Some(f) = strings::fallback(state.i18n.lang, key) {
            return f.to_string();
        }
    }
    s
}

/// `MessageFormat.format(tr(key), args)` – `{0}`, `{1}`, ... substitution.
pub(crate) fn trf(state: &AppState, key: &str, args: &[&dyn std::fmt::Display]) -> String {
    format_args_java(&tr(state, key), args)
}

pub(crate) fn format_args_java(pattern: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = pattern.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &a.to_string());
    }
    out
}

/// `"" + double` in Java (`Double.toString`): always shows a fraction digit, switches to
/// scientific notation outside [1e-3, 1e7).
pub(crate) fn java_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = v.abs();
    if (1e-3..1e7).contains(&a) {
        let s = format!("{v}");
        if s.contains('.') {
            s
        } else {
            format!("{s}.0")
        }
    } else {
        let s = format!("{v:e}"); // e.g. 1.5e-4 / 1e7
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let mant = if mant.contains('.') { mant.to_string() } else { format!("{mant}.0") };
        format!("{mant}E{exp}")
    }
}

// ---------------------------------------------------------------------------------------------
// layout helpers shared by all sections

/// A titled card (group with heading).
pub(crate) fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_impl(ui, None, title, add)
}

/// A titled card with a tlp-icon in front of the title.
pub(crate) fn icard<R>(ui: &mut Ui, icon: &str, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_impl(ui, Some(icon), title, add)
}

fn card_impl<R>(ui: &mut Ui, icon: Option<&str>, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui.visuals());
    let r = Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.hairline))
        .shadow(card_shadow(&p))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin { left: 12, right: 12, top: 10, bottom: 12 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 7.0;
                    if let Some(icon) = icon {
                        icons::icon(ui, icon, 15.0, p.accent);
                    }
                    ui.add(egui::Label::new(RichText::new(title).font(semibold(SECTION_SIZE)).color(p.text)).truncate());
                });
                ui.add_space(6.0);
            }
            add(ui)
        })
        .inner;
    ui.add_space(10.0);
    r
}

/// One form row: fixed-width label (with tooltip) followed by the editor.
pub(crate) fn form_row<R>(ui: &mut Ui, label: &str, tooltip: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(LABEL_W, ROW_H), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(LABEL_W);
            let l = ui.add(egui::Label::new(RichText::new(label.trim()).color(ui.visuals().weak_text_color())).truncate());
            if !tooltip.is_empty() {
                l.on_hover_text(tooltip);
            }
        });
        add(ui)
    })
    .inner
}

/// A key/value read-out line used in info cards (value right-aligned).
pub(crate) fn info_row(ui: &mut Ui, label: &str, value: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(LABEL_W, 18.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(LABEL_W);
            ui.add(egui::Label::new(RichText::new(label).weak()).truncate());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::Label::new(value).truncate());
        });
    });
}

/// Full-width button.
pub(crate) fn wide_button(ui: &mut Ui, text: impl Into<egui::WidgetText>, height: f32) -> egui::Response {
    let w = ui.available_width();
    ui.add(egui::Button::new(text.into()).min_size(Vec2::new(w, height)))
}

/// Full-width button with a tlp-icon.
pub(crate) fn wide_icon_button(ui: &mut Ui, icon: &str, text: impl Into<egui::WidgetText>, height: f32) -> egui::Response {
    let w = ui.available_width();
    ui.add(IconButton::new(icon, text).min_size(Vec2::new(w, height)))
}

/// Single-line search field with a search icon inside on the left.
pub(crate) fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let resp = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .margin(Margin { left: 28, right: 8, top: 5, bottom: 5 })
            .desired_width(width),
    );
    let c = ui.visuals().weak_text_color();
    let r = egui::Rect::from_center_size(egui::pos2(resp.rect.left() + 15.0, resp.rect.center().y), Vec2::splat(14.0));
    icons::paint(ui, "search", r, c, 1.0);
    resp
}

/// Accent colours for status display.
pub(crate) fn ok_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(110, 200, 120)
    } else {
        Color32::from_rgb(30, 130, 50)
    }
}
pub(crate) fn warn_color(ui: &Ui) -> Color32 {
    ui.visuals().warn_fg_color
}
pub(crate) fn err_color(ui: &Ui) -> Color32 {
    ui.visuals().error_fg_color
}

/// Small coloured status dot followed by text.
pub(crate) fn status_dot(ui: &mut Ui, color: Color32, text: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, color);
        ui.label(text);
    });
}

/// Panel-local, per-frame-persistent UI state kept in egui memory.
pub(crate) fn mem<T: 'static + Clone + Send + Sync + Default>(ui: &Ui, name: &str) -> T {
    ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<T>(egui::Id::unique(("app-panels", name))).clone())
}
pub(crate) fn mem_set<T: 'static + Clone + Send + Sync + Default>(ui: &Ui, name: &str, v: T) {
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(("app-panels", name)), v));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_double_formatting() {
        assert_eq!(java_double(2.0), "2.0");
        assert_eq!(java_double(300.0), "300.0");
        assert_eq!(java_double(1.1), "1.1");
        assert_eq!(java_double(0.02), "0.02");
        assert_eq!(java_double(0.0), "0.0");
        assert_eq!(java_double(1100.0), "1100.0");
        assert_eq!(java_double(1.0e7), "1.0E7");
        assert_eq!(java_double(1.5e-4), "1.5E-4");
    }

    #[test]
    fn message_format() {
        assert_eq!(format_args_java("Zoom X = {0}", &[&5]), "Zoom X = 5");
        assert_eq!(format_args_java("{1}/{0}", &[&1, &2]), "2/1");
    }
}
