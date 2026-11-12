//! Visual design system of the app: a refined dark theme (deep neutral surfaces, hairline
//! borders, soft elevation) and an equally polished light theme, one typography scale, the
//! 8px spacing grid and the accent colours (`stoneage_plots::theme::ACCENT` orange used
//! sparingly, `MARKER` as the cool secondary).
//!
//! Everything that paints chrome (title bar, rail, sidebar cards, status bar, toasts) takes its
//! colours from [`Palette::of`], so both themes stay consistent.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Theme, ThemePreference};
use stoneage_plots::theme::{ACCENT, MARKER};

/// Font family of the semibold UI face (titles, section headers, active tabs).
pub const SEMIBOLD: &str = "semibold";

// --- typography scale --------------------------------------------------------------------------
/// Window / dialog titles.
pub const TITLE_SIZE: f32 = 17.0;
/// Section headers (sidebar section, card titles).
pub const SECTION_SIZE: f32 = 13.5;
/// Body text, buttons.
pub const BODY_SIZE: f32 = 13.5;
/// Captions, hints, badges.
pub const CAPTION_SIZE: f32 = 11.5;

/// Semibold font of `size` (falls back to the regular face if Segoe UI Semibold is missing).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Colours of the chrome for one theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Title bar and icon rail (the darkest / most recessed surface).
    pub chrome: Color32,
    /// Sidebar panel background.
    pub panel: Color32,
    /// Card surface inside the sidebar.
    pub card: Color32,
    /// Card surface on hover / raised controls.
    pub raised: Color32,
    /// 1px hairline borders.
    pub hairline: Color32,
    /// Slightly stronger border (focused / hovered controls).
    pub border_strong: Color32,
    /// Main text.
    pub text: Color32,
    /// Secondary text.
    pub text_weak: Color32,
    /// Subtle hover fill on transparent controls.
    pub hover: Color32,
    /// Shadow colour for elevated surfaces.
    pub shadow: Color32,
    /// Accent (orange) – active elements, primary actions.
    pub accent: Color32,
    /// Accent tint for selected backgrounds.
    pub accent_soft: Color32,
    /// Cool secondary (blue).
    pub cool: Color32,
    pub dark: bool,
}

impl Palette {
    pub const DARK: Palette = Palette {
        chrome: Color32::from_rgb(0x0E, 0x10, 0x14),
        panel: Color32::from_rgb(0x14, 0x17, 0x1C),
        card: Color32::from_rgb(0x1A, 0x1E, 0x24),
        raised: Color32::from_rgb(0x22, 0x27, 0x2F),
        hairline: Color32::from_rgb(0x27, 0x2C, 0x34),
        border_strong: Color32::from_rgb(0x38, 0x3F, 0x4A),
        text: Color32::from_rgb(0xE4, 0xE7, 0xEC),
        text_weak: Color32::from_rgb(0x8A, 0x93, 0xA2),
        hover: Color32::from_rgb(0x1F, 0x24, 0x2B),
        shadow: Color32::from_rgba_premultiplied(0, 0, 0, 90),
        accent: ACCENT,
        accent_soft: Color32::from_rgba_premultiplied(0x2D, 0x1C, 0x09, 0x33),
        cool: MARKER,
        dark: true,
    };
    pub const LIGHT: Palette = Palette {
        chrome: Color32::from_rgb(0xE9, 0xEC, 0xF1),
        panel: Color32::from_rgb(0xF5, 0xF6, 0xF8),
        card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        raised: Color32::from_rgb(0xF1, 0xF3, 0xF6),
        hairline: Color32::from_rgb(0xDF, 0xE3, 0xE9),
        border_strong: Color32::from_rgb(0xC6, 0xCC, 0xD5),
        text: Color32::from_rgb(0x1A, 0x1F, 0x27),
        text_weak: Color32::from_rgb(0x66, 0x6F, 0x7D),
        hover: Color32::from_rgb(0xE2, 0xE6, 0xEC),
        shadow: Color32::from_rgba_premultiplied(0x10, 0x18, 0x28, 22),
        accent: ACCENT,
        accent_soft: Color32::from_rgba_premultiplied(0xF6, 0xE3, 0xCF, 0xFF),
        cool: MARKER,
        dark: false,
    };

    pub fn of(visuals: &egui::Visuals) -> Palette {
        if visuals.dark_mode { Self::DARK } else { Self::LIGHT }
    }
}

/// Soft drop shadow for cards / popups.
pub fn card_shadow(p: &Palette) -> Shadow {
    Shadow { offset: [0, 1], blur: if p.dark { 6 } else { 8 }, spread: 0, color: p.shadow }
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    super::icons::install(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(TITLE_SIZE, FontFamily::Name(SEMIBOLD.into()))),
            (TextStyle::Body, FontId::proportional(BODY_SIZE)),
            (TextStyle::Button, FontId::proportional(BODY_SIZE)),
            (TextStyle::Small, FontId::proportional(CAPTION_SIZE)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 6.0);
        s.button_padding = egui::vec2(10.0, 5.0);
        s.interact_size.y = 26.0;
        s.window_margin = Margin::same(14);
        s.menu_margin = Margin::same(6);
        s.indent = 16.0;
        s.combo_height = 280.0;
        s.scroll.bar_width = 8.0;
        s.scroll.floating = true;

        let v = &mut style.visuals;
        let r = CornerRadius::same(7);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = r;
        }
        v.widgets.hovered.expansion = 0.0;
        v.widgets.active.expansion = 0.0;
        v.hyperlink_color = MARKER;
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        v.indent_has_left_vline = false;
        v.striped = true;
        v.interact_cursor = Some(egui::CursorIcon::PointingHand);
        v.collapsing_header_frame = false;
        v.button_frame = true;
    });
    ctx.style_mut_of(Theme::Dark, |style| apply_palette(&mut style.visuals, &Palette::DARK));
    ctx.style_mut_of(Theme::Light, |style| apply_palette(&mut style.visuals, &Palette::LIGHT));
}

fn apply_palette(v: &mut egui::Visuals, p: &Palette) {
    v.panel_fill = p.panel;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.hairline);
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: p.shadow };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: p.shadow };
    v.extreme_bg_color = if p.dark { Color32::from_rgb(0x0B, 0x0D, 0x10) } else { Color32::WHITE };
    v.text_edit_bg_color = Some(if p.dark { Color32::from_rgb(0x10, 0x13, 0x17) } else { Color32::WHITE });
    v.faint_bg_color = if p.dark { Color32::from_rgb(0x1D, 0x21, 0x28) } else { Color32::from_rgb(0xF3, 0xF5, 0xF8) };
    v.code_bg_color = p.raised;
    v.override_text_color = None;
    v.weak_text_color = Some(p.text_weak);
    v.warn_fg_color = WARN_AMBER;
    v.error_fg_color = ERROR_RED;

    let w = &mut v.widgets;
    // Static frames (cards, separators).
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.weak_bg_fill = p.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    // Buttons at rest: quiet raised surface with a hairline.
    let (rest, hover, press) = if p.dark {
        (Color32::from_rgb(0x20, 0x25, 0x2C), Color32::from_rgb(0x29, 0x2F, 0x38), Color32::from_rgb(0x31, 0x38, 0x43))
    } else {
        (Color32::from_rgb(0xFF, 0xFF, 0xFF), Color32::from_rgb(0xF0, 0xF2, 0xF6), Color32::from_rgb(0xE4, 0xE8, 0xEE))
    };
    w.inactive.bg_fill = if p.dark { Color32::from_rgb(0x2A, 0x30, 0x39) } else { Color32::from_rgb(0xDD, 0xE2, 0xE9) };
    w.inactive.weak_bg_fill = rest;
    w.inactive.bg_stroke = Stroke::new(1.0, p.hairline);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.bg_fill = if p.dark { Color32::from_rgb(0x34, 0x3B, 0x46) } else { Color32::from_rgb(0xCF, 0xD5, 0xDE) };
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.active.bg_fill = press;
    w.active.weak_bg_fill = press;
    w.active.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.8));
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.open.bg_fill = press;
    w.open.weak_bg_fill = hover;
    w.open.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.open.fg_stroke = Stroke::new(1.0, p.text);

    v.selection.bg_fill = if p.dark { ACCENT.gamma_multiply(0.42) } else { Color32::from_rgb(0xF8, 0xD9, 0xB8) };
    v.selection.stroke = Stroke::new(1.5, if p.dark { Color32::from_rgb(0xFF, 0xC9, 0x92) } else { Color32::from_rgb(0x9A, 0x4E, 0x08) });
}

pub fn set_dark(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark { ThemePreference::Dark } else { ThemePreference::Light });
}

pub const OK_GREEN: Color32 = Color32::from_rgb(0x34, 0xC0, 0x7A);
pub const WARN_AMBER: Color32 = Color32::from_rgb(0xE8, 0xA8, 0x32);
pub const ERROR_RED: Color32 = Color32::from_rgb(0xE5, 0x4D, 0x4D);

/// Segoe UI (regular + semibold) and Cascadia Mono from the Windows font folder as the primary
/// faces, egui's built-in fonts and the Windows symbol/emoji fonts as fallbacks. Missing files
/// are skipped (the `semibold` family then falls back to the regular face).
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let load = |file: &str| std::fs::read(std::path::Path::new(&dir).join("Fonts").join(file)).ok();
    let add = |fonts: &mut egui::FontDefinitions, name: &str, file: &str| -> bool {
        match load(file) {
            Some(bytes) => {
                fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            None => false,
        }
    };
    let has_ui = add(&mut fonts, "segoe_ui", "segoeui.ttf");
    let has_sb = add(&mut fonts, "segoe_ui_sb", "seguisb.ttf");
    let has_mono = add(&mut fonts, "cascadia_mono", "CascadiaMono.ttf");
    let has_sym = add(&mut fonts, "segoe_symbol", "seguisym.ttf");
    let has_emoji = add(&mut fonts, "segoe_emoji", "seguiemj.ttf");

    let defaults_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let defaults_mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();

    let mut prop = Vec::new();
    if has_ui {
        prop.push("segoe_ui".to_owned());
    }
    prop.extend(defaults_prop.iter().cloned());
    let mut semibold = Vec::new();
    if has_sb {
        semibold.push("segoe_ui_sb".to_owned());
    }
    semibold.extend(prop.iter().cloned());
    let mut mono = Vec::new();
    if has_mono {
        mono.push("cascadia_mono".to_owned());
    }
    mono.extend(defaults_mono.iter().cloned());
    for (has, name) in [(has_sym, "segoe_symbol"), (has_emoji, "segoe_emoji")] {
        if has {
            for fam in [&mut prop, &mut semibold, &mut mono] {
                fam.push(name.to_owned());
            }
        }
    }
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Monospace, mono);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    ctx.set_fonts(fonts);
}
