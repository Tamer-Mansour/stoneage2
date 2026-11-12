//! Files section: document actions, current file info, recent files, import/export, map.

use egui::{Button, RichText, Ui, Vec2};

use super::{icard, icons, IconButton};

use super::{info_row, tr, trf};
use crate::state::{AppState, Command};

/// Two equally wide icon buttons per row; the shortcut is shown in the tooltip.
fn button_grid(ui: &mut Ui, state: &mut AppState, items: &[(&str, String, String, Command, &str)]) {
    let gap = ui.spacing().item_spacing.x;
    let w = ((ui.available_width() - gap) / 2.0).floor();
    for pair in items.chunks(2) {
        ui.horizontal(|ui| {
            for (icon, label, tip, cmd, shortcut) in pair {
                let r = ui.add(IconButton::new(icon, label.as_str()).align_left().min_size(Vec2::new(w, 32.0)));
                let tip = match (tip.is_empty(), shortcut.is_empty()) {
                    (true, true) => String::new(),
                    (false, true) => tip.clone(),
                    (true, false) => shortcut.to_string(),
                    (false, false) => format!("{tip}
{shortcut}"),
                };
                let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
                if r.clicked() {
                    state.push(cmd.clone());
                }
            }
        });
    }
}

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    // --- current file --------------------------------------------------------------------
    icard(ui, "file-text", &tr(state, "App.Files.Current"), |ui| {
        let untitled = tr(state, "App.Files.Untitled");
        let (fname, full) = match &state.file_path {
            Some(p) => (
                p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| untitled.clone()),
                p.display().to_string(),
            ),
            None => (untitled.clone(), String::new()),
        };
        ui.horizontal(|ui| {
            icons::icon(ui, "file-text", 16.0, ui.visuals().weak_text_color());
            let r = ui.label(RichText::new(fname).font(crate::shell::theme::semibold(14.0)));
            if !full.is_empty() {
                r.on_hover_text(&full);
            }
            if state.dirty {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, super::warn_color(ui));
                resp.on_hover_text(tr(state, "App.Files.Modified"));
            }
        });
        if !full.is_empty() {
            ui.add(egui::Label::new(RichText::new(&full).small().weak()).truncate());
        }
        ui.add_space(4.0);
        let s = &state.survey;
        let title = if s.name.trim().is_empty() { "–".to_string() } else { s.name.clone() };
        info_row(ui, &tr(state, "OptionPanel.lbl_Name.Caption"), title);
        if !s.date.trim().is_empty() {
            info_row(ui, &tr(state, "OptionPanel.lbl_Datum.Caption"), s.date.clone());
        }
        let points: usize = s.rows.iter().map(|r| r.count_points()).sum();
        info_row(ui, &tr(state, "App.Files.Rows"), s.rows.len().to_string());
        info_row(ui, &tr(state, "App.Files.Points"), points.to_string());
        let modified = if state.dirty { tr(state, "App.Common.Yes") } else { tr(state, "App.Common.No") };
        info_row(ui, &tr(state, "App.Files.Modified"), modified);
        let map = if s.map_image.is_some() { tr(state, "App.Files.MapLoaded") } else { tr(state, "App.Files.NoMap") };
        info_row(ui, &tr(state, "App.Files.Map"), map);
    });

    // --- document ---------------------------------------------------------------------------
    icard(ui, "folder", &tr(state, "App.Files.Document"), |ui| {
        let items = vec![
            ("file-plus", tr(state, "App.Files.New"), String::new(), Command::NewSurvey, "Ctrl+N"),
            ("folder-open", tr(state, "MainFrame.Menu.File.Load.Caption"), String::new(), Command::Open, "Ctrl+O"),
            ("save", tr(state, "MainFrame.Menu.File.Save.Caption"), String::new(), Command::Save, "Ctrl+S"),
            ("copy", tr(state, "App.Files.SaveAs"), String::new(), Command::SaveAs, "Ctrl+Shift+S"),
        ];
        button_grid(ui, state, &items);
    });

    // --- recent files ---------------------------------------------------------------------------
    icard(ui, "history", &tr(state, "App.Files.Recent"), |ui| {
        if state.recent_files.is_empty() {
            ui.label(RichText::new(tr(state, "App.Files.NoRecent")).weak());
            return;
        }
        let missing_tip = tr(state, "App.Files.Missing");
        let mut open = None;
        for p in &state.recent_files {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
            let exists = p.exists();
            let current = state.file_path.as_ref() == Some(p);
            let mut text = RichText::new(&name);
            if !exists {
                text = text.strikethrough().weak();
            } else if current {
                text = text.strong();
            }
            let resp = ui
                .add_sized(
                    Vec2::new(ui.available_width(), 20.0),
                    Button::selectable(current, text).truncate(),
                )
                .on_hover_text(if exists { p.display().to_string() } else { format!("{}\n{missing_tip}", p.display()) });
            ui.add(egui::Label::new(RichText::new(dir).small().weak()).truncate());
            if resp.clicked() && exists {
                open = Some(p.clone());
            }
        }
        if let Some(p) = open {
            state.push(Command::OpenPath(p));
        }
    });

    // --- import / export ---------------------------------------------------------------------
    icard(ui, "layers", &tr(state, "App.Files.ImportExport"), |ui| {
        let items = vec![
            ("download", tr(state, "MainFrame.Menu.File.ImportCSV.Caption"), tr(state, "CSVFileFilter.Description"), Command::ImportCsv, ""),
            ("upload", tr(state, "MainFrame.Menu.File.ExportCSV.Caption"), tr(state, "CSVFileFilter.Description"), Command::ExportCsv, ""),
            ("image", tr(state, "App.Files.ExportImage"), tr(state, "App.Files.ExportImage.ToolTip"), Command::ExportImage, ""),
            ("printer", tr(state, "MainFrame.Menu.File.Print.Caption"), String::new(), Command::Print, "Ctrl+P"),
        ];
        button_grid(ui, state, &items);
    });

    // --- map ------------------------------------------------------------------------------------
    icard(ui, "map", &tr(state, "App.Files.Map"), |ui| {
        let has_map = state.survey.map_image.as_ref().map(|m| (m.width(), m.height()));
        match has_map {
            Some((w, h)) => {
                ui.label(trf(state, "App.Files.MapSize", &[&w, &h]));
            }
            None => {
                ui.label(RichText::new(tr(state, "App.Files.NoMap")).weak());
            }
        }
        let items = vec![
            ("map", tr(state, "MainFrame.Menu.File.LoadMap.Caption"), tr(state, "ImageFileFilter.Description"), Command::LoadMap, ""),
            ("circle-x", tr(state, "App.Files.ClearMap"), String::new(), Command::ClearMap, ""),
        ];
        button_grid(ui, state, &items);
    });

    // --- defaults -----------------------------------------------------------------------------
    icard(ui, "settings", &tr(state, "App.Files.Defaults"), |ui| {
        ui.label(RichText::new(tr(state, "App.Files.SaveDefaults.Hint")).small().weak());
        ui.add_space(2.0);
        if super::wide_icon_button(ui, "save", tr(state, "MainFrame.Menu.File.SaveIni.Caption"), 32.0)
            .on_hover_text(state.config_path.display().to_string())
            .clicked()
        {
            state.push(Command::SaveDefaults);
        }
    });
}
