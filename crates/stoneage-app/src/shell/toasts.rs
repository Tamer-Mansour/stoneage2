//! Toast notifications (bottom right) for success / error messages (`state.toast`).
//! Messages starting with "⚠" are shown as errors (triangle-alert icon) and stay longer; the
//! marker itself is not displayed. Toasts slide/fade in and fade out before they expire.

use std::time::{Duration, Instant};

use egui::{Align2, Id, Order, Rect, RichText, Sense, Stroke, Vec2};

use super::icons;
use super::theme::{Palette, ERROR_RED, OK_GREEN};
use crate::state::AppState;

pub const TOAST_TIME: Duration = Duration::from_millis(3500);
pub const ERROR_TIME: Duration = Duration::from_millis(7000);
pub const MAX_TOASTS: usize = 5;
const FADE: f32 = 0.22;

fn is_error(text: &str) -> bool {
    text.starts_with('⚠')
}

fn lifetime(text: &str) -> Duration {
    if is_error(text) { ERROR_TIME } else { TOAST_TIME }
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    let now = Instant::now();
    state.toasts.retain(|(t, text)| now.duration_since(*t) < lifetime(text));
    if state.toasts.len() > MAX_TOASTS {
        let n = state.toasts.len() - MAX_TOASTS;
        state.toasts.drain(..n);
    }
    if state.toasts.is_empty() {
        return;
    }
    let reduce = icons::reduce_motion(ctx);
    let mut dismiss = None;
    egui::Area::new(Id::unique("stoneage_toasts"))
        .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-18.0, -46.0))
        .order(Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_max_width(400.0);
            let p = Palette::of(ui.visuals());
            for (i, (born, text)) in state.toasts.iter().enumerate() {
                let err = is_error(text);
                let color = if err { ERROR_RED } else { OK_GREEN };
                let msg = text.trim_start_matches('⚠').trim_start();
                let age = now.duration_since(*born).as_secs_f32();
                let left = lifetime(text).as_secs_f32() - age;
                let alpha = if reduce { 1.0 } else { (age / FADE).min(left / FADE).clamp(0.0, 1.0) };
                ui.set_opacity(alpha);
                let slide = if reduce { 0.0 } else { (1.0 - (age / FADE).min(1.0)) * 16.0 };
                ui.add_space(slide);
                let resp = egui::Frame::new()
                    .fill(p.card)
                    .stroke(Stroke::new(1.0, if p.dark { p.border_strong } else { p.hairline }))
                    .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 20, spread: 0, color: p.shadow })
                    .corner_radius(10)
                    .inner_margin(egui::Margin { left: 12, right: 14, top: 10, bottom: 10 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Icon in a soft tinted circle.
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 13.0, color.gamma_multiply(0.16));
                            let icon = if err { "triangle-alert" } else { "check" };
                            let t = icons::hover_progress(ui.ctx(), ui.scope_id().with(("toast_icon", i)), true);
                            icons::paint(ui, icon, Rect::from_center_size(r.center(), Vec2::splat(15.0)), color, t);
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(msg).color(p.text)).wrap());
                        });
                    })
                    .response;
                if resp.interact(Sense::click()).clicked() {
                    dismiss = Some(i);
                }
                ui.add_space(8.0);
            }
        });
    if let Some(i) = dismiss {
        state.toasts.remove(i);
    }
    // Keep animating until all toasts expired.
    ctx.request_repaint_after(Duration::from_millis(if reduce { 250 } else { 16 }));
}
