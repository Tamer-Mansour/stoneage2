//! Help center content (user guide, shortcuts, about, license) in de/en/tr. OWNER: agent "app-panels".
//!
//! The original help pages (`StoneAge/help/<lang>/*.html`) only contained "To be written"
//! plus the (German) license agreement; this is a complete user guide written for the port.
//! Topic bodies use a tiny line-based markup rendered by [`render_body`]:
//! `# ` heading, `## ` sub-heading, `- ` bullet, `> ` note box, `$ ` formula (monospace),
//! `| a | b |` two-column table row, blank line = paragraph break, other lines = paragraph text.
//! Placeholders: `{version}` (original version), `{format}` (.saf format id), `{app_version}`.

mod de;
mod en;
mod tr;

use egui::{RichText, Ui};
use stoneage_core::Language;

use crate::panels::{mem, mem_set, tr as ttr};
use crate::state::{AppState, Command};

/// One help topic.
pub struct Topic {
    pub id: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

/// All topics of a language (the license topic body is filled from the original license page).
pub fn topics(lang: Language) -> &'static [Topic] {
    match lang {
        Language::German => de::TOPICS,
        Language::English => en::TOPICS,
        Language::Turkish => tr::TOPICS,
    }
}

// The license pages are stored in their original encoding (Windows-1252).
const LICENSE_DE: &[u8] = stoneage_core::assets::LICENSE_DE;
const LICENSE_EN: &[u8] = stoneage_core::assets::LICENSE_EN;
const LICENSE_TR: &[u8] = stoneage_core::assets::LICENSE_TR;

/// The original `help/<lang>/license.html`, rendered as plain text (menu removed).
pub fn license_text(lang: Language) -> String {
    let html = match lang {
        Language::German => LICENSE_DE,
        Language::English => LICENSE_EN,
        Language::Turkish => LICENSE_TR,
    };
    html_to_text(&decode_text(html))
}

/// UTF-8 if valid, otherwise Windows-1252.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    const CP1252: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}',
        '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| if (0x80..0xA0).contains(&b) { CP1252[(b - 0x80) as usize] } else { b as char })
        .collect()
}

/// Minimal HTML → text: drops `<head>`, the navigation `<div class="menu">`, tags; keeps line
/// structure (`<br>`, `<h1>`, `<p>` become line breaks), decodes the common entities.
pub(crate) fn html_to_text(html: &str) -> String {
    let mut s = html.replace("\r\n", "\n");
    // remove head and menu blocks
    for (start, end) in [("<head>", "</head>"), ("<div class=\"menu\">", "</div>")] {
        while let Some(a) = s.find(start) {
            match s[a..].find(end) {
                Some(b) => s.replace_range(a..a + b + end.len(), ""),
                None => break,
            }
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            rest = &rest[lt..];
            break;
        };
        let tag = rest[lt + 1..lt + gt].trim().to_ascii_lowercase();
        let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("");
        match name {
            "br" | "p" | "h1" | "h2" | "h3" | "li" | "tr" => out.push('\n'),
            _ => {}
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    let out = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    // the footer "(c) ..." follows the last <hr>; keep it, but trim excessive blank lines
    let mut lines: Vec<String> = Vec::new();
    let mut blank = 0;
    for l in out.lines() {
        let t = l.trim_end();
        if t.trim().is_empty() {
            blank += 1;
            if blank > 1 || lines.is_empty() {
                continue;
            }
        } else {
            blank = 0;
        }
        lines.push(t.to_string());
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

fn fill_placeholders(s: &str) -> String {
    s.replace("{version}", stoneage_core::ORIGINAL_VERSION)
        .replace("{format}", stoneage_core::FILE_FORMAT)
        .replace("{app_version}", env!("CARGO_PKG_VERSION"))
}

#[derive(Clone)]
struct HelpState {
    topic: String,
    search: String,
}

impl Default for HelpState {
    fn default() -> Self {
        Self { topic: "start".into(), search: String::new() }
    }
}

/// Help center (sidebar section "Help").
pub fn show(ui: &mut Ui, state: &mut AppState) {
    let mut hs: HelpState = mem(ui, "help_state");
    let lang = state.i18n.lang;

    // language switcher
    ui.horizontal_wrapped(|ui| {
        for l in Language::ALL {
            let caption = ttr(state, &format!("MainFrame.Menu.Edit.Languages.{}.Caption", l.code()));
            if ui.selectable_label(l == lang, caption).clicked() && l != lang {
                state.push(Command::SetLanguage(l));
            }
        }
    });
    ui.add_space(4.0);

    // search
    ui.horizontal(|ui| {
        let w = ui.available_width() - 34.0;
        crate::panels::search_field(ui, &mut hs.search, &ttr(state, "App.Help.Search"), w);
        if ui
            .add(crate::panels::IconButton::icon_only("x", 26.0).kind(crate::shell::icons::Kind::Normal).icon_size(13.0))
            .on_hover_text(ttr(state, "App.Common.Clear"))
            .clicked()
        {
            hs.search.clear();
        }
    });
    ui.add_space(4.0);

    // topic list
    let all = topics(lang);
    let needle = hs.search.trim().to_lowercase();
    let license = license_text(lang);
    let mut matches: Vec<(&Topic, usize)> = Vec::new();
    for t in all {
        if needle.is_empty() {
            matches.push((t, 0));
            continue;
        }
        let body = if t.id == "license" { license.to_lowercase() } else { fill_placeholders(t.body).to_lowercase() };
        let hits = body.matches(&needle).count() + t.title.to_lowercase().matches(&needle).count();
        if hits > 0 {
            matches.push((t, hits));
        }
    }
    let pal = crate::shell::theme::Palette::of(ui.visuals());
    egui::Frame::new()
        .fill(pal.card)
        .stroke(egui::Stroke::new(1.0, pal.hairline))
        .shadow(crate::shell::theme::card_shadow(&pal))
        .corner_radius(10)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if matches.is_empty() {
                ui.label(RichText::new(ttr(state, "App.Help.NoResults")).weak());
            }
            egui::ScrollArea::vertical()
                .id_salt("app-panels-help-topics")
                .max_height(if needle.is_empty() { 230.0 } else { 160.0 })
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (t, hits) in &matches {
                        let sel = hs.topic == t.id;
                        let mut b = egui::Button::selectable(sel, if sel { RichText::new(t.title).strong() } else { RichText::new(t.title) })
                            .min_size(egui::vec2(ui.available_width(), 20.0));
                        if *hits > 0 {
                            b = b.right_text(RichText::new(format!("{hits}")).small().weak());
                        }
                        if ui.add(b).clicked() {
                            hs.topic = t.id.to_string();
                        }
                    }
                });
        });
    ui.add_space(6.0);

    // topic body
    let topic = all.iter().find(|t| t.id == hs.topic).or_else(|| all.first());
    // topic body (the shell's sidebar scroll area scrolls it)
    if let Some(t) = topic {
        ui.add_space(4.0);
        ui.label(RichText::new(t.title).font(crate::shell::theme::semibold(crate::shell::theme::TITLE_SIZE)));
        ui.add_space(4.0);
        if t.id == "license" {
            ui.label(RichText::new(t.body).weak().small());
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(&license).small()).wrap());
        } else {
            render_body(ui, &fill_placeholders(t.body), &needle);
        }
        if t.id == "about" {
            icon_attribution(ui);
        }
    }

    mem_set(ui, "help_state", hs);
}

/// Attribution of the tlp-icons icon set (MIT) shown under "About".
fn icon_attribution(ui: &mut Ui) {
    let pal = crate::shell::theme::Palette::of(ui.visuals());
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(pal.card)
        .stroke(egui::Stroke::new(1.0, pal.hairline))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                crate::shell::icons::icon(ui, "sparkle", 15.0, pal.accent);
                ui.label(RichText::new("Icons: tlp-icons").font(crate::shell::theme::semibold(13.5)));
            });
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Icons by tlp-icons (MIT License), with geometry from Lucide (ISC) and Tabler Icons (MIT).",
                    )
                    .small(),
                )
                .wrap(),
            );
            ui.hyperlink_to("tlp-icons.netlify.app", "https://tlp-icons.netlify.app");
            egui::CollapsingHeader::new(RichText::new("MIT License").small())
                .id_salt("tlp-icons-license")
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(crate::shell::icons::LICENSE).small().weak()).wrap());
                });
        });
}

/// Render the topic markup (see module docs). Paragraphs containing `needle` are highlighted.
pub(crate) fn render_body(ui: &mut Ui, body: &str, needle: &str) {
    let mut para = String::new();
    let mut table: Vec<(String, String)> = Vec::new();
    let mut table_id = 0usize;

    let highlight = |ui: &Ui, text: &str| -> bool { !needle.is_empty() && text.to_lowercase().contains(needle) && ui.is_enabled() };

    let flush_para = |ui: &mut Ui, para: &mut String| {
        if !para.trim().is_empty() {
            let text = para.trim().to_string();
            let mut rt = RichText::new(&text);
            if highlight(ui, &text) {
                rt = rt.background_color(ui.visuals().selection.bg_fill.gamma_multiply(0.5));
            }
            ui.add(egui::Label::new(rt).wrap());
            ui.add_space(6.0);
        }
        para.clear();
    };
    let flush_table = |ui: &mut Ui, table: &mut Vec<(String, String)>, id: &mut usize| {
        if table.is_empty() {
            return;
        }
        *id += 1;
        egui::Grid::new(("help-table", *id)).num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for (a, b) in table.iter() {
                ui.label(RichText::new(a).monospace().strong());
                ui.add(egui::Label::new(b.as_str()).wrap());
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        table.clear();
    };

    for line in body.lines() {
        let l = line.trim_end();
        let t = l.trim_start();
        if t.starts_with('|') {
            flush_para(ui, &mut para);
            let cells: Vec<&str> = t.trim_matches('|').split('|').map(|c| c.trim()).collect();
            let a = cells.first().copied().unwrap_or("").to_string();
            let b = cells.get(1).copied().unwrap_or("").to_string();
            table.push((a, b));
            continue;
        }
        flush_table(ui, &mut table, &mut table_id);
        if t.is_empty() {
            flush_para(ui, &mut para);
        } else if let Some(h) = t.strip_prefix("## ") {
            flush_para(ui, &mut para);
            ui.add_space(4.0);
            ui.label(RichText::new(h).font(crate::shell::theme::semibold(14.0)));
            ui.add_space(2.0);
        } else if let Some(h) = t.strip_prefix("# ") {
            flush_para(ui, &mut para);
            ui.add_space(6.0);
            ui.label(RichText::new(h).font(crate::shell::theme::semibold(16.0)));
            ui.add_space(2.0);
        } else if let Some(b) = t.strip_prefix("- ") {
            flush_para(ui, &mut para);
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 2.5, stoneage_plots::theme::ACCENT);
                let mut rt = RichText::new(b);
                if highlight(ui, b) {
                    rt = rt.background_color(ui.visuals().selection.bg_fill.gamma_multiply(0.5));
                }
                ui.add(egui::Label::new(rt).wrap());
            });
        } else if let Some(n) = t.strip_prefix("> ") {
            flush_para(ui, &mut para);
            let cool = stoneage_plots::theme::MARKER;
            egui::Frame::new()
                .fill(cool.gamma_multiply(0.10))
                .stroke(egui::Stroke::new(1.0, cool.gamma_multiply(0.35)))
                .corner_radius(8)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        crate::shell::icons::icon(ui, "info", 15.0, cool);
                        ui.add(egui::Label::new(RichText::new(n)).wrap());
                    });
                });
            ui.add_space(6.0);
        } else if let Some(f) = t.strip_prefix("$ ") {
            flush_para(ui, &mut para);
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(6)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(f).monospace()).wrap());
                });
            ui.add_space(4.0);
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(t);
        }
    }
    flush_table(ui, &mut table, &mut table_id);
    flush_para(ui, &mut para);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_same_topics() {
        let ids = |l| topics(l).iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(Language::English), ids(Language::German));
        assert_eq!(ids(Language::English), ids(Language::Turkish));
        assert!(ids(Language::English).contains(&"license"));
    }

    #[test]
    fn license_is_plain_text() {
        for l in Language::ALL {
            let t = license_text(l);
            if stoneage_core::assets::ORIGINAL_ASSETS {
                assert!(t.contains("Lizenzvereinbarung"));
                assert!(t.contains("K-S Analysis, Waltershausen"));
            } else {
                assert!(t.contains("StoneAge 2"));
            }
            assert!(!t.contains('<'));
            assert!(!t.contains("Introduction"));
        }
    }

    #[test]
    fn placeholders_are_filled() {
        let about = topics(Language::English).iter().find(|t| t.id == "about").unwrap();
        let s = fill_placeholders(about.body);
        assert!(s.contains(stoneage_core::ORIGINAL_VERSION));
        assert!(s.contains(stoneage_core::FILE_FORMAT));
    }
}
