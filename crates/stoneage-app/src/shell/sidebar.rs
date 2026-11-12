//! Left icon rail + collapsible sidebar panel (default 320 px, resizable 260–480). The section
//! contents come from `panels::sidebar_section` (agent "app-panels"); this replaces the original
//! `OptionPanel` split pane (top) and the "Help" / "Messages" tabs.
//!
//! The rail shows the work sections at the top and the utility sections (Help, Messages, and
//! anything added after them) pinned to the bottom.

use egui::{Align, Layout, Rect, Sense, Stroke, Vec2};

use super::icons::{self, IconButton};
use super::t;
use super::theme::{semibold, Palette};
use crate::panels;
use crate::state::{AppState, Command, SidebarSection};

/// (section, tlp-icon name, i18n key, fallback)
pub const SECTIONS: [(SidebarSection, &str, &str, &str); 8] = [
    (SidebarSection::Files, "folder-open", "App.Section.Files", "Files"),
    (SidebarSection::Measure, "activity", "App.Section.Measure", "Measure"),
    (SidebarSection::Survey, "clipboard-list", "App.Section.Survey", "Survey"),
    (SidebarSection::Display, "palette", "App.Section.Display", "Display"),
    (SidebarSection::Data, "table-2", "App.Section.Data", "Data"),
    (SidebarSection::Help, "help-circle", "App.Section.Help", "Help"),
    (SidebarSection::Messages, "message-square", "App.Section.Messages", "Messages"),
    (SidebarSection::Updates, "cloud-download", "App.Section.Updates", "Updates"),
];

pub const RAIL_WIDTH: f32 = 60.0;
pub const PANEL_WIDTH: f32 = 320.0;
pub const PANEL_MIN: f32 = 260.0;
pub const PANEL_MAX: f32 = 480.0;

pub fn section_title(state: &AppState, s: SidebarSection) -> String {
    SECTIONS
        .iter()
        .find(|x| x.0 == s)
        .map(|(_, _, k, f)| t(&state.i18n, k, f))
        .unwrap_or_default()
}

fn section_icon(s: SidebarSection) -> &'static str {
    SECTIONS.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or("layers")
}

/// Work sections sit at the top of the rail, everything else (Help, Messages, …) at the bottom.
fn is_work_section(s: SidebarSection) -> bool {
    matches!(
        s,
        SidebarSection::Files
            | SidebarSection::Measure
            | SidebarSection::Survey
            | SidebarSection::Display
            | SidebarSection::Data
    )
}

pub fn sidebar(ui: &mut egui::Ui, state: &mut AppState) {
    rail(ui, state);

    let section = state.sidebar;
    let mut open = state.sidebar_open;
    let mut hide = false;
    let p = Palette::of(ui.visuals());
    egui::Panel::left("stoneage_sidebar_v2")
        .resizable(true)
        .default_size(PANEL_WIDTH)
        .size_range(PANEL_MIN..=PANEL_MAX)
        .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin { left: 14, right: 12, top: 12, bottom: 4 }))
        .show_collapsible(ui, &mut open, |ui| {
            // Header: icon tile, section title, hide button.
            ui.horizontal(|ui| {
                let (tile, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                ui.painter().rect_filled(tile, 8.0, p.accent_soft);
                ui.painter().rect_stroke(tile, 8.0, Stroke::new(1.0, p.accent.gamma_multiply(0.35)), egui::StrokeKind::Inside);
                icons::paint(ui, section_icon(section), Rect::from_center_size(tile.center(), Vec2::splat(17.0)), p.accent, 1.0);
                ui.add_space(4.0);
                ui.label(egui::RichText::new(section_title(state, section)).font(semibold(16.0)).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let hide_tip = t(&state.i18n, "App.Sidebar.Hide", "Hide panel");
                    if ui.add(IconButton::icon_only("panel-left-close", 28.0)).on_hover_text(hide_tip).clicked() {
                        hide = true;
                    }
                });
            });
            ui.add_space(8.0);
            let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, p.hairline));
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .id_salt(("sidebar_scroll", section_title(state, section)))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    // Lay the section out in a child ui of fixed width and advance by its height
                    // only, so a control that is a few pixels too wide can never push the
                    // resizable panel wider frame after frame.
                    let rect = ui.available_rect_before_wrap();
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()));
                    panels::sidebar_section(&mut child, state, section);
                    let used = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), child.min_rect().height()));
                    ui.advance_cursor_after_rect(used);
                });
        });
    // The panel may have been collapsed/expanded by dragging, or hidden by the hide button.
    state.sidebar_open = open && !hide;
}

fn rail(ui: &mut egui::Ui, state: &mut AppState) {
    let p = Palette::of(ui.visuals());
    egui::Panel::left("stoneage_rail")
        .resizable(false)
        .exact_size(RAIL_WIDTH)
        .show_separator_line(true)
        .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(8, 12)))
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (sec, icon, key, fallback) in SECTIONS.iter().filter(|s| is_work_section(s.0)) {
                    rail_button(ui, state, *sec, icon, &t(&state.i18n, key, fallback));
                }
            });
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (sec, icon, key, fallback) in SECTIONS.iter().rev().filter(|s| !is_work_section(s.0)) {
                    rail_button(ui, state, *sec, icon, &t(&state.i18n, key, fallback));
                }
                ui.add_space(2.0);
                let (line, _) = ui.allocate_exact_size(Vec2::new(28.0, 1.0), Sense::hover());
                ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, p.hairline));
                ui.add_space(2.0);
            });
        });
}

/// One rail entry: 44×40, accent indicator bar + soft accent fill when active, hover fill.
fn rail_button(ui: &mut egui::Ui, state: &mut AppState, sec: SidebarSection, icon: &str, label: &str) {
    let p = Palette::of(ui.visuals());
    let active = state.sidebar == sec && state.sidebar_open;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(44.0, 40.0), Sense::click());
    let resp = resp.on_hover_text(label);
    let ctx = ui.ctx().clone();
    let dur = icons::transition_secs(&ctx, 0.16);
    let a = ctx.animate_bool_with_time(resp.id.with("active"), active, dur);
    let h = ctx.animate_bool_with_time(resp.id.with("hover"), resp.hovered(), icons::transition_secs(&ctx, 0.10));

    let painter = ui.painter();
    if h > 0.0 && a < 1.0 {
        painter.rect_filled(rect, 10.0, p.hover.gamma_multiply(h * (1.0 - a)));
    }
    if a > 0.0 {
        painter.rect_filled(rect, 10.0, p.accent_soft.gamma_multiply(a));
        // Indicator bar on the rail's left edge, growing from the centre.
        let bar_h = 20.0 * a;
        let x = ui.max_rect().left() - 6.0;
        let bar = Rect::from_center_size(egui::pos2(x, rect.center().y), Vec2::new(3.0, bar_h));
        painter.rect_filled(bar, 2.0, p.accent);
    }
    let color = icons::lerp(icons::lerp(p.text_weak, p.text, h), p.accent, a);
    let t = icons::hover_progress(&ctx, resp.id, resp.hovered());
    icons::paint(ui, icon, Rect::from_center_size(rect.center(), Vec2::splat(20.0)), color, t);
    if sec == SidebarSection::Updates && state.update.badge {
        super::updates::paint_badge(ui, rect);
    }
    if resp.clicked() {
        state.push(Command::ShowSection(sec));
    }
}
