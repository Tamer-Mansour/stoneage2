//! View tabs (`MainFrame.jTabbedPane1`: Map, 3d, Areal, Row, Point, Soundtest, Soundtest2) as a
//! segmented control with icons and a sliding selection pill, shown in the custom title bar.
//! (Help and Messages live in the sidebar.)

use egui::{Rect, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Vec2, WidgetText};

use super::icons;
use super::t;
use super::theme::{card_shadow, semibold, Palette};
use crate::state::{AppState, Command, Tab};

/// (tab, i18n key, fallback)
pub const TABS: [(Tab, &str, &str); 7] = [
    (Tab::Map, "MainFrame.Tabs.Map.Caption", "Map"),
    (Tab::View3d, "MainFrame.Tabs.Java3d.Caption", "3d"),
    (Tab::Areal, "MainFrame.Tabs.Areal.Caption", "Areal"),
    (Tab::Row, "MainFrame.Tabs.Row.Caption", "Row"),
    (Tab::Point, "MainFrame.Tabs.Point.Caption", "Point"),
    (Tab::Soundtest, "MainFrame.Tabs.Soundtest.Caption", "Soundtest"),
    (Tab::Soundtest2, "MainFrame.Tabs.Soundtest2.Caption", "Soundtest 2"),
];

/// tlp-icon of a view tab.
pub fn tab_icon(tab: Tab) -> &'static str {
    match tab {
        Tab::Map => "map",
        Tab::View3d => "box",
        Tab::Areal => "grid-3x3",
        Tab::Row => "rows-3",
        Tab::Point => "chart-column",
        Tab::Soundtest => "mic",
        Tab::Soundtest2 => "audio-waveform",
    }
}

#[allow(dead_code)] // helper for panels / help
pub fn tab_name(state: &AppState, tab: Tab) -> String {
    TABS.iter()
        .find(|x| x.0 == tab)
        .map(|(_, k, f)| t(&state.i18n, k, f))
        .unwrap_or_default()
}

const SEG_H: f32 = 28.0;
const PAD: f32 = 3.0;
const ICON: f32 = 15.0;
const GAP: f32 = 6.0;
const SEG_PAD_X: f32 = 11.0;

/// Segmented control. `reserve` is the width that must stay free to the right of it (window
/// buttons); if the labels do not fit, the tabs collapse to icons only (names in the tooltips).
pub fn segmented(ui: &mut egui::Ui, state: &mut AppState, reserve: f32) {
    let p = Palette::of(ui.visuals());
    let ctx = ui.ctx().clone();
    let labels: Vec<_> = TABS
        .iter()
        .map(|(tab, key, fallback)| {
            let text = t(&state.i18n, key, fallback);
            let font = if state.tab == *tab { semibold(13.0) } else { egui::FontId::proportional(13.0) };
            let g = WidgetText::from(egui::RichText::new(text.clone()).font(font)).into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            );
            (*tab, text, g)
        })
        .collect();
    let full: f32 = labels.iter().map(|(_, _, g)| ICON + GAP + g.size().x + 2.0 * SEG_PAD_X).sum::<f32>() + 2.0 * PAD;
    let compact = full > ui.available_width() - reserve;
    let widths: Vec<f32> = labels
        .iter()
        .map(|(_, _, g)| if compact { ICON + 2.0 * 10.0 } else { ICON + GAP + g.size().x + 2.0 * SEG_PAD_X })
        .collect();
    let total = widths.iter().sum::<f32>() + 2.0 * PAD;

    let (outer, _) = ui.allocate_exact_size(Vec2::new(total, SEG_H + 2.0 * PAD), Sense::hover());
    let track = if p.dark { p.panel } else { p.hover };
    ui.painter().rect(outer, 10.0, track, Stroke::new(1.0, p.hairline), StrokeKind::Inside);

    // Segment rects.
    let mut x = outer.left() + PAD;
    let rects: Vec<Rect> = widths
        .iter()
        .map(|w| {
            let r = Rect::from_min_size(egui::pos2(x, outer.top() + PAD), Vec2::new(*w, SEG_H));
            x += w;
            r
        })
        .collect();

    // Sliding pill under the selected segment.
    if let Some(sel) = labels.iter().position(|(tab, _, _)| *tab == state.tab) {
        let id = ui.scope_id().with("tab_pill");
        let dur = icons::transition_secs(&ctx, 0.22);
        let target = rects[sel];
        let left = ctx.animate_value_with_time(id.with("l"), target.left(), dur);
        let right = ctx.animate_value_with_time(id.with("r"), target.right(), dur);
        let pill = Rect::from_x_y_ranges(left..=right, target.y_range());
        let fill = if p.dark { p.raised } else { p.card };
        ui.painter().add(card_shadow(&p).as_shape(pill, 8.0));
        ui.painter().rect(pill, 8.0, fill, Stroke::new(1.0, if p.dark { p.border_strong } else { p.hairline }), StrokeKind::Inside);
    }

    for (i, ((tab, text, galley), rect)) in labels.into_iter().zip(rects).enumerate() {
        let selected = state.tab == tab;
        let resp = ui.interact(rect, ui.scope_id().with(("tab", i)), Sense::click());
        let resp = if compact {
            resp.on_hover_text(format!("{text}   Ctrl+{}", i + 1))
        } else {
            resp.on_hover_text(format!("Ctrl+{}", i + 1))
        };
        let h = ctx.animate_bool_with_time(resp.id.with("h"), resp.hovered(), icons::transition_secs(&ctx, 0.10));
        let text_color = if selected { p.text } else { icons::lerp(p.text_weak, p.text, h) };
        let icon_color = if selected { p.accent } else { text_color };
        let tprog = icons::hover_progress(&ctx, resp.id, resp.hovered());
        let icon_x = if compact { rect.center().x - ICON * 0.5 } else { rect.left() + SEG_PAD_X };
        let icon_rect = Rect::from_min_size(egui::pos2(icon_x, rect.center().y - ICON * 0.5), Vec2::splat(ICON));
        icons::paint(ui, tab_icon(tab), icon_rect, icon_color, tprog);
        if !compact {
            let pos = egui::pos2(icon_rect.right() + GAP, rect.center().y - galley.size().y * 0.5);
            ui.painter().galley(pos, galley, text_color);
        }
        if resp.clicked() && !selected {
            state.push(Command::SwitchTab(tab));
        }
    }
}
