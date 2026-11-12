//! Survey section: title, date, description, geometry and the physical parameters
//! (Kb, fRange, dfh, Tm) of the original `OptionPanel`, plus Apply.

use egui::{Key, RichText, TextEdit, Ui};

use super::option_panel::{OptionForm, VALID_F_RANGE, VALID_KB};
use super::{form_row, icard, info_row, java_double, tr, IconButton, LABEL_W};
use crate::state::AppState;

pub(super) fn show(ui: &mut Ui, state: &mut AppState) {
    let mut form = OptionForm::get(ui, state);
    let mut apply = false;

    let enter = |ui: &Ui, r: &egui::Response| r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

    // --- general ---------------------------------------------------------------------------
    icard(ui, "file-text", &tr(state, "App.Survey.General"), |ui| {
        let field_w = ui.available_width() - LABEL_W - 8.0;
        form_row(ui, &tr(state, "OptionPanel.lbl_Name.Caption"), "", |ui| {
            let r = ui.add(TextEdit::singleline(&mut form.name).desired_width(field_w));
            apply |= enter(ui, &r); // OptionPanel$9
        });
        form_row(ui, &tr(state, "OptionPanel.lbl_Datum.Caption"), "", |ui| {
            let now_label = tr(state, "App.Survey.Now");
            let btn_w = 48.0;
            let r = ui.add(TextEdit::singleline(&mut form.date).desired_width((field_w - btn_w - ui.spacing().item_spacing.x - 2.0).max(40.0)));
            apply |= enter(ui, &r); // OptionPanel$10
            if ui
                .add(egui::Button::new(now_label).min_size(egui::vec2(btn_w, 0.0)))
                .on_hover_text(tr(state, "App.Survey.Now.ToolTip"))
                .clicked()
            {
                form.date = chrono::Local::now().format("%d.%m.%Y, %H:%M Uhr").to_string();
                apply = true;
            }
        });
        ui.label(tr(state, "OptionPanel.lbl_Bemerkung.Caption"));
        ui.add(
            TextEdit::multiline(&mut form.description)
                .desired_rows(4)
                .desired_width(ui.available_width()),
        );
    });

    // --- geometry --------------------------------------------------------------------------
    icard(ui, "ruler", &tr(state, "App.Survey.Geometry"), |ui| {
        let w = 90.0;
        let tip = tr(state, "OptionPanel.lbl_Punktabstand.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_Punktabstand.Caption"), &tip, |ui| {
            let r = ui.add(TextEdit::singleline(&mut form.point_distance).desired_width(w));
            let r = r.on_hover_text(tr(state, "OptionPanel.tfMesspunktAbstand.Caption"));
            apply |= enter(ui, &r); // OptionPanel$8
            ui.label(RichText::new("m").weak());
        });
        let tip = tr(state, "OptionPanel.lbl_Reihenabstand.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_Reihenabstand.Caption"), &tip, |ui| {
            let r = ui.add(TextEdit::singleline(&mut form.row_distance).desired_width(w)).on_hover_text(&tip);
            apply |= enter(ui, &r); // OptionPanel$11
            ui.label(RichText::new("m").weak());
        });
    });

    // --- measurement parameters --------------------------------------------------------------
    icard(ui, "sliders-horizontal", &tr(state, "App.Survey.Parameters"), |ui| {
        let combo_w = ui.available_width() - LABEL_W - 8.0;

        // Kb (jcb_kb, editable combo, OptionPanel$7)
        let kb_tip = tr(state, "OptionPanel.lbl_kb.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_kb.Caption"), &kb_tip, |ui| {
            let custom = tr(state, "App.Survey.Custom");
            let selected = match form.kb_index {
                Some(i) => tr(state, &format!("OptionPanel.validKbDescr.{i}")),
                None => format!("{custom} ({})", form.kb_text),
            };
            let before = form.kb_index;
            egui::ComboBox::from_id_salt("app-panels-kb")
                .width(combo_w)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for i in 0..VALID_KB.len() {
                        ui.selectable_value(&mut form.kb_index, Some(i), tr(state, &format!("OptionPanel.validKbDescr.{i}")));
                    }
                    ui.selectable_value(&mut form.kb_index, None, custom);
                })
                .response
                .on_hover_text(&kb_tip);
            if form.kb_index != before {
                if let Some(i) = form.kb_index {
                    form.kb_text = java_double((VALID_KB[i] * 1000.0 * 100.0).round() / 100.0);
                    apply = true;
                }
            }
        });
        if form.kb_index.is_none() {
            form_row(ui, "", &kb_tip, |ui| {
                let r = ui.add(TextEdit::singleline(&mut form.kb_text).desired_width(90.0)).on_hover_text(&kb_tip);
                apply |= enter(ui, &r);
                ui.label(RichText::new("× 1000").weak());
            });
        }

        // fRange (jcb_fRange, OptionPanel$5)
        let fr_tip = tr(state, "OptionPanel.lbl_fRange.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_fRange.Caption"), &fr_tip, |ui| {
            let selected = match form.fr_index {
                Some(i) => tr(state, &format!("OptionPanel.validfRangeDescr.{i}")),
                None => format!("{} kHz", form.fr_text),
            };
            let before = form.fr_index;
            egui::ComboBox::from_id_salt("app-panels-frange")
                .width(combo_w)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for i in 0..VALID_F_RANGE.len() {
                        ui.selectable_value(
                            &mut form.fr_index,
                            Some(i),
                            tr(state, &format!("OptionPanel.validfRangeDescr.{i}")),
                        );
                    }
                })
                .response
                .on_hover_text(&fr_tip);
            if form.fr_index != before {
                apply = true;
            }
        });

        // dfh (tf_dfh, OptionPanel$6)
        let dfh_tip = tr(state, "App.Survey.dfh.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_dfh.Caption"), &dfh_tip, |ui| {
            let r = ui.add(TextEdit::singleline(&mut form.dfh).desired_width(90.0)).on_hover_text(&dfh_tip);
            apply |= enter(ui, &r);
        });

        // Tm (only in ini/.saf in the original; keys OptionPanel.lbl_Tm.* exist)
        let tm_tip = tr(state, "OptionPanel.lbl_Tm.ToolTip");
        form_row(ui, &tr(state, "OptionPanel.lbl_Tm.Caption"), &tm_tip, |ui| {
            let r = ui.add(TextEdit::singleline(&mut form.tm).desired_width(90.0)).on_hover_text(&tm_tip);
            apply |= enter(ui, &r);
        });
    });

    // --- apply -------------------------------------------------------------------------------
    let pending = form.has_pending_edits();
    let w = ui.available_width();
    let mut btn = IconButton::new("check", tr(state, "OptionPanel.btnApply.Caption")).min_size(egui::vec2(w, 36.0));
    if pending {
        btn = btn.primary();
    }
    if ui.add(btn).on_hover_text(tr(state, "OptionPanel.btnApply.ToolTip")).clicked() {
        apply = true; // OptionPanel$1
    }
    if pending {
        ui.label(RichText::new(tr(state, "App.Survey.PendingEdits")).small().color(super::warn_color(ui)));
    }
    ui.add_space(8.0);

    if apply {
        form.set_values(state);
    }

    // --- derived values (read-only) ----------------------------------------------------------
    icard(ui, "mountain", &tr(state, "App.Survey.Derived"), |ui| {
        let s = &state.survey;
        let max_i = super::option_panel::HCUT_RANGE.1 as usize;
        info_row(ui, &tr(state, "App.Survey.DepthPerSample"), format!("{} m", java_double(s.depth(1))));
        info_row(ui, &tr(state, "App.Survey.MaxDepth"), format!("{} m", java_double(s.depth(max_i))));
        info_row(
            ui,
            &tr(state, "App.Survey.FreqStep"),
            format!("{} kHz", java_double(s.f_range / stoneage_core::NUM_SAMPLES as f64)),
        );
        ui.add_space(2.0);
        ui.label(RichText::new(tr(state, "App.Survey.DepthFormula")).small().weak());
    });

    form.store(ui);
}
