//! Menu bar – port of `MainFrame.jMenuBar1` (File, Edit with Languages) with all original items
//! (including the ones the original created but never added: Import CSV, Load map, recreate
//! Scenegraph), plus View / Measure / Help menus of the modern shell.

use egui::{Button, KeyboardShortcut};
use stoneage_core::Language;

use super::shortcuts as sc;
use super::{t, tabs::TABS};
use crate::app::StoneAgeApp;
use crate::state::{AppState, Command, SidebarSection};

fn item(ui: &mut egui::Ui, state: &mut AppState, key: &str, fallback: &str, cmd: Command) {
    if ui.button(t(&state.i18n, key, fallback)).clicked() {
        state.push(cmd);
    }
}

fn item_sc(ui: &mut egui::Ui, state: &mut AppState, key: &str, fallback: &str, s: &KeyboardShortcut, cmd: Command) {
    let text = t(&state.i18n, key, fallback);
    let sc_text = ui.ctx().format_shortcut(s);
    if ui.add(Button::new(text).shortcut_text(sc_text)).clicked() {
        state.push(cmd);
    }
}

fn item_txt(ui: &mut egui::Ui, state: &mut AppState, key: &str, fallback: &str, shortcut: &str, cmd: Command) {
    let text = t(&state.i18n, key, fallback);
    if ui.add(Button::new(text).shortcut_text(shortcut)).clicked() {
        state.push(cmd);
    }
}

/// The menus, placed inside the custom title bar next to the app name.
pub fn menus(ui: &mut egui::Ui, app: &mut StoneAgeApp) {
    file_menu(ui, &mut app.state);
    edit_menu(ui, &mut app.state);
    view_menu(ui, app);
    measure_menu(ui, &mut app.state);
    help_menu(ui, &mut app.state);
}

fn file_menu(ui: &mut egui::Ui, st: &mut AppState) {
    let title = t(&st.i18n, "MainFrame.Menu.File.Caption", "File");
    ui.menu_button(title, |ui| {
        item_sc(ui, st, "App.Menu.File.New", "New survey", &sc::NEW, Command::NewSurvey);
        item_sc(ui, st, "MainFrame.Menu.File.Load.Caption", "Load", &sc::OPEN, Command::Open);
        let recent_title = t(&st.i18n, "App.Menu.File.Recent", "Recent files");
        let recent = st.recent_files.clone();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button(recent_title, |ui| {
                for p in recent {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                        st.push(Command::OpenPath(p.clone()));
                    }
                }
                ui.separator();
                if ui.button(t(&st.i18n, "App.Menu.File.ClearRecent", "Clear list")).clicked() {
                    st.recent_files.clear();
                }
            });
        });
        ui.separator();
        item_sc(ui, st, "MainFrame.Menu.File.Save.Caption", "Save", &sc::SAVE, Command::Save);
        item_sc(ui, st, "App.Menu.File.SaveAs", "Save as…", &sc::SAVE_AS, Command::SaveAs);
        ui.separator();
        item(ui, st, "MainFrame.Menu.File.ImportCSV.Caption", "Import CSV", Command::ImportCsv);
        item(ui, st, "MainFrame.Menu.File.ExportCSV.Caption", "Export CSV", Command::ExportCsv);
        ui.separator();
        item(ui, st, "MainFrame.Menu.File.LoadMap.Caption", "Load map", Command::LoadMap);
        let has_map = st.survey.map_image.is_some();
        ui.add_enabled_ui(has_map, |ui| {
            item(ui, st, "App.Menu.File.ClearMap", "Remove map", Command::ClearMap);
        });
        ui.separator();
        item(ui, st, "App.Menu.File.ExportImage", "Export image…", Command::ExportImage);
        item_sc(ui, st, "MainFrame.Menu.File.Print.Caption", "Print", &sc::PRINT, Command::Print);
        ui.separator();
        item(ui, st, "MainFrame.Menu.File.SaveIni.Caption", "Save Defaults", Command::SaveDefaults);
        ui.separator();
        // Support: logs + crash reports for remote troubleshooting.
        let diag = t(&st.i18n, "App.Menu.File.Diagnostics", "Diagnostics");
        ui.menu_button(diag, |ui| {
            item_sc(ui, st, "App.Menu.File.ExportDiagnostics", "Export diagnostics…", &sc::DIAGNOSTICS, Command::ExportDiagnostics);
            item(ui, st, "App.Menu.File.OpenLogFolder", "Open log folder", Command::OpenLogFolder);
        });
        ui.separator();
        item_sc(ui, st, "MainFrame.Menu.File.Exit.Caption", "Exit", &sc::EXIT, Command::Exit);
    });
}

fn edit_menu(ui: &mut egui::Ui, st: &mut AppState) {
    let title = t(&st.i18n, "MainFrame.Menu.Edit.Caption", "Edit");
    ui.menu_button(title, |ui| {
        item(ui, st, "MainFrame.Menu.Edit.DelRow.Caption", "Delete current row", Command::DeleteRow);
        item(ui, st, "MainFrame.Menu.Edit.DelPoint.Caption", "Delete current Point", Command::DeletePoint);
        item(ui, st, "MainFrame.Menu.Edit.DelAllRows.Caption", "Delete all rows", Command::DeleteAllRows);
        ui.separator();
        item_txt(ui, st, "MainFrame.Menu.Edit.prevRow.Caption", "Previous Row", "↑", Command::PrevRow);
        item_txt(ui, st, "MainFrame.Menu.Edit.nextRow.Caption", "Next Row", "↓", Command::NextRow);
        item(ui, st, "MainFrame.Menu.Edit.newRow.Caption", "Insert new Row", Command::NewRow);
        ui.separator();
        item(ui, st, "MainFrame.Menu.Edit.findMinMax.Caption", "find Min/Max", Command::FindMinMax);
        item(ui, st, "MainFrame.Menu.Edit.resetMinMax.Caption", "reset Min/Max", Command::ResetMinMax);
        ui.separator();
        item_txt(ui, st, "MainFrame.Menu.Edit.toggleTracer.Caption", "toggle Tracer", "T", Command::ToggleTracer);
        // The original used the literal caption "recreate Scenegraph".
        item(ui, st, "App.Menu.Edit.RecreateScenegraph", "recreate Scenegraph", Command::RecreateScenegraph);
        ui.separator();
        let langs = t(&st.i18n, "MainFrame.Menu.Edit.Languages.Caption", "Languages");
        ui.menu_button(langs, |ui| {
            for (lang, key, fallback) in [
                (Language::German, "MainFrame.Menu.Edit.Languages.de_DE.Caption", "german (deutsch)"),
                (Language::English, "MainFrame.Menu.Edit.Languages.en_US.Caption", "english"),
                (Language::Turkish, "MainFrame.Menu.Edit.Languages.tr_TR.Caption", "turkish (türkçe)"),
            ] {
                let text = t(&st.i18n, key, fallback);
                if ui.add(Button::new(text).selected(st.i18n.lang == lang)).clicked() {
                    st.push(Command::SetLanguage(lang));
                }
            }
        });
    });
}

fn view_menu(ui: &mut egui::Ui, app: &mut StoneAgeApp) {
    let title = t(&app.state.i18n, "App.Menu.View", "View");
    ui.menu_button(title, |ui| {
        for (i, (tab, key, fallback)) in TABS.iter().enumerate() {
            let text = t(&app.state.i18n, key, fallback);
            let b = Button::new(text).selected(app.state.tab == *tab).shortcut_text(format!("Ctrl+{}", i + 1));
            if ui.add(b).clicked() {
                app.state.push(Command::SwitchTab(*tab));
            }
        }
        ui.separator();
        let sidebar = t(&app.state.i18n, "App.Menu.View.Sidebar", "Show sidebar");
        ui.checkbox(&mut app.state.sidebar_open, sidebar);
        ui.separator();
        let dark = t(&app.state.i18n, "App.Menu.View.Dark", "Dark theme");
        let light = t(&app.state.i18n, "App.Menu.View.Light", "Light theme");
        if ui.add(Button::new(dark).selected(app.shell.dark)).clicked() {
            app.shell.dark = true;
            super::theme::set_dark(ui.ctx(), true);
        }
        if ui.add(Button::new(light).selected(!app.shell.dark)).clicked() {
            app.shell.dark = false;
            super::theme::set_dark(ui.ctx(), false);
        }
        ui.separator();
        let mut reduce = super::icons::reduce_motion(ui.ctx());
        let label = t(&app.state.i18n, "App.Menu.View.ReduceMotion", "Reduce motion");
        if ui.checkbox(&mut reduce, label).changed() {
            super::icons::set_reduce_motion(ui.ctx(), reduce);
        }
    });
}

fn measure_menu(ui: &mut egui::Ui, st: &mut AppState) {
    let title = t(&st.i18n, "App.Menu.Measure", "Measure");
    ui.menu_button(title, |ui| {
        if st.scope_running {
            item(ui, st, "OptionPanel.jtbStartOszi.Stop.Caption", "stop Oszi", Command::StopScope);
        } else {
            item(ui, st, "OptionPanel.jtbStartOszi.Start.Caption", "start Oszi", Command::StartScope);
        }
        let mut gui = st.scope.gui_visible();
        let label = t(&st.i18n, "OptionPanel.jcbShowOsziGui.Caption", "show oszi");
        if ui.checkbox(&mut gui, label).changed() {
            st.push(Command::ShowScopeGui(gui));
        }
        ui.separator();
        item_sc(ui, st, "OptionPanel.jbPunktNeu.Caption", "Record", &sc::RECORD, Command::Record { delayed: false });
        item_sc(ui, st, "OptionPanel.jbPunktNeu1.Caption", "Record delayed", &sc::RECORD_DELAYED, Command::Record { delayed: true });
        item_sc(ui, st, "OptionPanel.jbReiheNeu.Caption", "Rec + new row", &sc::RECORD_NEW_ROW, Command::RecordNewRow { delayed: false });
        item(ui, st, "OptionPanel.jbReiheNeu1.Caption", "Delayed Record + new Row", Command::RecordNewRow { delayed: true });
        ui.add_enabled_ui(st.pending_record.is_some(), |ui| {
            item_txt(ui, st, "App.Menu.Measure.Cancel", "Cancel delayed record", "Esc", Command::CancelDelayedRecord);
        });
    });
}

fn help_menu(ui: &mut egui::Ui, st: &mut AppState) {
    let title = t(&st.i18n, "MainFrame.Tabs.Help.Caption", "Help");
    ui.menu_button(title, |ui| {
        item_sc(ui, st, "App.Menu.Help.Contents", "Help center", &sc::HELP, Command::ShowSection(SidebarSection::Help));
        item_txt(ui, st, "MainFrame.Tabs.Msg.Caption", "Messages", "T", Command::ShowSection(SidebarSection::Messages));
        ui.separator();
        ui.label(egui::RichText::new(format!("StoneAge 2 · v{}", env!("CARGO_PKG_VERSION"))).weak().small());
    });
}
