//! Port of `Splash` / `Splash$MyWindow`, shown inside the main window: the splash image with the
//! version label ("1.49, 2012/04/11, mm") and a progress bar that walks through the status lines
//! of the startup sequence for about one second.
//!
//! `Start.panicExit`: the progress bar turns red, shows "panic: <msg>", and a click removes the
//! lock file and exits with code -1 (`Start$1` / `Start$2`).

use std::path::Path;
use std::time::{Duration, Instant};

use egui::{Align, Color32, Layout, RichText, Sense};

/// How long the splash is shown.
pub const SPLASH_DURATION: Duration = Duration::from_millis(1000);
/// `Splash.maximum`
pub const SPLASH_MAXIMUM: usize = 38;
/// `Splash.labelText`
pub const LABEL_TEXT: &str = "1.49, 2012/04/11, mm";

pub struct Splash {
    started: Option<Instant>,
    lines: Vec<String>,
    panic: Option<String>,
    texture: Option<egui::TextureHandle>,
    finished: bool,
}

impl Splash {
    pub fn new(lines: Vec<String>, panic: Option<String>) -> Self {
        Self { started: None, lines, panic, texture: None, finished: false }
    }

    pub fn done(&self) -> bool {
        self.finished
    }
}

/// Decodes the embedded splash PNG.
fn splash_image() -> Option<image::RgbaImage> {
    image::load_from_memory(stoneage_core::SPLASH_PNG).ok().map(|i| i.to_rgba8())
}

/// Window icon derived from the splash image (centre square, 64x64).
/// Small icon texture for the custom title bar (same crop as the window icon), cached.
pub fn icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique("stoneage_title_icon");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let icon = window_icon();
    if icon.width == 0 {
        return None;
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let tex = ctx.load_texture("stoneage_title_icon", img, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

pub fn window_icon() -> egui::IconData {
    let Some(img) = splash_image() else { return egui::IconData::default() };
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    let crop = image::imageops::crop_imm(&img, x, y, side, side).to_image();
    let icon = image::imageops::resize(&crop, 64, 64, image::imageops::FilterType::Triangle);
    egui::IconData { rgba: icon.into_raw(), width: 64, height: 64 }
}

pub fn show(ui: &mut egui::Ui, splash: &mut Splash, lock_path: Option<&Path>) {
    let ctx = ui.ctx().clone();
    let started = *splash.started.get_or_insert_with(Instant::now);
    if splash.texture.is_none() {
        if let Some(img) = splash_image() {
            let size = [img.width() as usize, img.height() as usize];
            let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            splash.texture = Some(ctx.load_texture("stoneage-splash", ci, egui::TextureOptions::LINEAR));
        }
    }

    let elapsed = started.elapsed();
    let panic = splash.panic.clone();
    let (fraction, status) = if let Some(p) = &panic {
        (1.0, format!("panic: {p}"))
    } else {
        let f = (elapsed.as_secs_f32() / SPLASH_DURATION.as_secs_f32()).clamp(0.0, 1.0);
        let n = splash.lines.len().max(1);
        let idx = ((f * n as f32) as usize).min(n - 1);
        let status = splash.lines.get(idx).cloned().unwrap_or_else(|| "starting ...".into());
        // Progress like the JProgressBar: setStatus increments towards Splash.maximum.
        let steps = ((idx + 1) as f32 / n as f32) * splash.lines.len() as f32;
        (f.max(steps / SPLASH_MAXIMUM as f32).min(1.0), status)
    };

    let p = super::theme::Palette::of(ui.visuals());
    let bg = p.chrome;
    let resp = egui::CentralPanel::default_margins()
        .frame(egui::Frame::new().fill(bg))
        .show(ui, |ui| {
            // Soft accent glow behind the card.
            let screen = ui.max_rect();
            let c = screen.center();
            for (r, a) in [(420.0, 0.03), (300.0, 0.04), (200.0, 0.05)] {
                ui.painter().circle_filled(c, r, p.accent.gamma_multiply(a));
            }
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                let avail = ui.available_size();
                ui.add_space((avail.y * 0.5 - 230.0).max(16.0));
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, if p.dark { p.border_strong } else { p.hairline }))
                    .shadow(egui::epaint::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 36 }) })
                    .corner_radius(18)
                    .inner_margin(egui::Margin { left: 16, right: 16, top: 16, bottom: 20 })
                    .show(ui, |ui| {
                        ui.set_width(560.0);
                        ui.vertical_centered(|ui| {
                            if let Some(tex) = &splash.texture {
                                ui.add(egui::Image::from_texture(tex).max_size(egui::vec2(528.0, 300.0)).corner_radius(12));
                            }
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(RichText::new("StoneAge").font(super::theme::semibold(22.0)).color(p.text));
                                ui.add_space(-4.0);
                                ui.label(RichText::new("2").font(super::theme::semibold(22.0)).color(p.accent));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(LABEL_TEXT).size(11.5).color(p.text_weak));
                                });
                            });
                            ui.add_space(10.0);
                            // Slim progress bar with the status line above it.
                            let color = if panic.is_some() { Color32::from_rgb(0xD9, 0x3A, 0x3A) } else { p.accent };
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.add(egui::Label::new(RichText::new(status.clone()).size(12.0).color(if panic.is_some() { color } else { p.text_weak })).truncate());
                            });
                            ui.add_space(4.0);
                            let (r, _) = ui.allocate_exact_size(egui::vec2(512.0, 6.0), Sense::hover());
                            ui.painter().rect_filled(r, 3.0, if p.dark { p.raised } else { p.hover });
                            let fill = egui::Rect::from_min_size(r.min, egui::vec2(r.width() * fraction, r.height()));
                            ui.painter().rect_filled(fill, 3.0, color);
                            if panic.is_some() {
                                ui.add_space(10.0);
                                ui.label(RichText::new("Click to exit").size(12.0).color(p.text_weak));
                            }
                        });
                    });
            });
        })
        .response;

    if panic.is_some() {
        // Start$1.mouseClicked: delete lock file, "Panic Exit!", System.exit(-1).
        let clicked = resp.interact(Sense::click()).clicked() || ctx.input(|i| i.pointer.any_click());
        if clicked {
            let _ = std::fs::remove_file(lock_path.map(Path::to_path_buf).unwrap_or_else(super::startup::lock_file_path));
            log::error!("Start : remove lock file ...");
            log::error!("Start : Panic Exit!");
            std::process::exit(-1);
        }
        return;
    }

    if elapsed >= SPLASH_DURATION {
        splash.finished = true;
        ctx.request_repaint();
    } else {
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}
