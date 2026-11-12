//! Custom window title bar (the window is undecorated): app icon + name, the menus and the
//! view tabs right next to it, and the theme toggle + window buttons on the right – everything
//! in one row. Empty space drags the window, double-click maximises/restores, the window edges
//! resize it.

use egui::viewport::ResizeDirection;
use egui::{Align, Color32, CursorIcon, Layout, Rect, RichText, Sense, Stroke, Vec2, ViewportCommand};

use super::icons::{self, IconButton};
use super::theme::{semibold, Palette};
use crate::app::StoneAgeApp;

pub const HEIGHT: f32 = 44.0;
const BTN_W: f32 = 46.0;
/// Width kept free on the right for the theme toggle and the window buttons.
const RIGHT_RESERVE: f32 = 3.0 * BTN_W + 56.0;

pub fn title_bar(ui: &mut egui::Ui, app: &mut StoneAgeApp) {
    let ctx = ui.ctx().clone();
    let p = Palette::of(ui.visuals());
    egui::Panel::top("stoneage_titlebar")
        .resizable(false)
        .exact_size(HEIGHT)
        .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin { left: 14, right: 0, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let bar = ui.max_rect();
            // Background drag area first, so widgets placed on top take precedence.
            let drag = ui.interact(bar, ui.scope_id().with("titlebar_drag"), Sense::click_and_drag());
            if drag.double_clicked() {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            } else if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // App icon + name + document.
                if let Some(tex) = super::splash::icon_texture(ui.ctx()) {
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(22.0)).corner_radius(5));
                }
                ui.label(RichText::new("StoneAge").font(semibold(14.5)).color(p.text));
                ui.add_space(-6.0);
                ui.label(RichText::new("2").font(semibold(14.5)).color(p.accent));
                let doc = super::document_name(&app.state);
                ui.add(egui::Label::new(RichText::new(doc).size(12.5).color(p.text_weak)).truncate().selectable(false));
                if app.state.dirty {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, super::theme::WARN_AMBER);
                }
                vline(ui, &p);
                // Menus next to the name.
                // MenuBar takes all remaining width: give it a child ui exactly as wide as the
                // menu buttons measured last frame.
                let wid = ui.scope_id().with("menu_w");
                let w: f32 = ui.ctx().data(|d| d.get_temp(wid)).unwrap_or(260.0);
                ui.allocate_ui_with_layout(Vec2::new(w, HEIGHT), Layout::left_to_right(Align::Center), |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        let x0 = ui.cursor().left();
                        super::menu::menus(ui, app);
                        let used = (ui.cursor().left() - x0 - ui.spacing().item_spacing.x).max(10.0).ceil();
                        if (used - w).abs() > 0.5 {
                            ui.ctx().data_mut(|d| d.insert_temp(wid, used));
                            ui.ctx().request_repaint();
                        }
                    });
                });
                vline(ui, &p);
                // View tabs in the same row.
                super::tabs::segmented(ui, &mut app.state, RIGHT_RESERVE);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    window_buttons(ui, &ctx, &p);
                    ui.add_space(6.0);
                    theme_toggle(ui, app);
                });
            });
        });
    resize_edges(&ctx);
}

fn vline(ui: &mut egui::Ui, p: &Palette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 20.0), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, p.hairline));
}

fn theme_toggle(ui: &mut egui::Ui, app: &mut StoneAgeApp) {
    let dark = app.shell.dark;
    let (icon, tip) = if dark {
        ("sun", super::t(&app.state.i18n, "App.Menu.View.Light", "Light theme"))
    } else {
        ("moon", super::t(&app.state.i18n, "App.Menu.View.Dark", "Dark theme"))
    };
    if ui.add(IconButton::icon_only(icon, 32.0).corner(8)).on_hover_text(tip).clicked() {
        app.shell.dark = !dark;
        super::theme::set_dark(ui.ctx(), !dark);
    }
}

fn window_buttons(ui: &mut egui::Ui, ctx: &egui::Context, p: &Palette) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if caption_button(ui, "x", true, p).on_hover_text("Close").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    let (icon, tip) = if maximized { ("copy", "Restore") } else { ("square", "Maximize") };
    if caption_button(ui, icon, false, p).on_hover_text(tip).clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if caption_button(ui, "minus", false, p).on_hover_text("Minimize").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

fn caption_button(ui: &mut egui::Ui, icon: &str, close: bool, p: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(BTN_W, HEIGHT), Sense::click());
    let ctx = ui.ctx().clone();
    let h = ctx.animate_bool_with_time(resp.id.with("h"), resp.hovered(), icons::transition_secs(&ctx, 0.10));
    if h > 0.0 {
        let fill = if close { Color32::from_rgb(0xD9, 0x34, 0x2B) } else { p.hover };
        let fill = if resp.is_pointer_button_down_on() { icons::darken(fill, 0.1) } else { fill };
        ui.painter().rect_filled(rect, 0.0, fill.gamma_multiply(h));
    }
    let color = if close { icons::lerp(p.text_weak, Color32::WHITE, h) } else { icons::lerp(p.text_weak, p.text, h) };
    // Square / restore glyphs read better slightly smaller than the others.
    let size = if icon == "x" || icon == "minus" { 16.0 } else { 13.0 };
    let t = icons::hover_progress(&ctx, resp.id, resp.hovered());
    icons::paint(ui, icon, Rect::from_center_size(rect.center(), Vec2::splat(size)), color, t);
    resp
}

/// Undecorated windows have no native resize border: emulate one along the window edges.
fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    const M: f32 = 5.0;
    let (l, r) = (pos.x < screen.left() + M, pos.x > screen.right() - M);
    let (t, b) = (pos.y < screen.top() + M, pos.y > screen.bottom() - M);
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some((ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest)),
        (_, true, true, _) => Some((ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast)),
        (true, _, _, true) => Some((ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest)),
        (_, true, _, true) => Some((ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast)),
        (true, ..) => Some((ResizeDirection::West, CursorIcon::ResizeWest)),
        (_, true, ..) => Some((ResizeDirection::East, CursorIcon::ResizeEast)),
        (_, _, true, _) => Some((ResizeDirection::North, CursorIcon::ResizeNorth)),
        (_, _, _, true) => Some((ResizeDirection::South, CursorIcon::ResizeSouth)),
        _ => None,
    };
    if let Some((dir, cursor)) = dir {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
