//! Keyboard shortcuts. Ctrl+O/S/Shift+S/N/P, Space = Record, Ctrl+Space = Record delayed,
//! Shift+Space = Rec + new row, arrows = point/row navigation, Esc = cancel delayed record,
//! F1 = help, Ctrl+1..7 = tabs, T = toggle tracer (original `MainFrame$1.keyTyped('t')`).

use egui::{Key, KeyboardShortcut, Modifiers};

use super::tabs::TABS;
use crate::state::{AppState, Command, SidebarSection};

pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
pub const SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
pub const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub const PRINT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);
pub const EXIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
pub const RECORD_DELAYED: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Space);
pub const RECORD_NEW_ROW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::Space);
pub const RECORD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Space);
pub const DIAGNOSTICS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::D);
pub const HELP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F1);

const TAB_KEYS: [Key; 7] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7];

/// Human readable shortcut text.
#[allow(dead_code)] // helper for the help center (app-panels)
pub fn label(ctx: &egui::Context, s: &KeyboardShortcut) -> String {
    ctx.format_shortcut(s)
}

pub fn handle(ctx: &egui::Context, state: &mut AppState) {
    let typing = ctx.egui_wants_keyboard_input();
    let modal_open = ctx.memory(|m| m.top_modal_layer().is_some());
    if modal_open {
        return;
    }
    let mut cmds = Vec::new();
    ctx.input_mut(|i| {
        // Order matters: the more specific shortcut first.
        if i.consume_shortcut(&SAVE_AS) {
            cmds.push(Command::SaveAs);
        }
        if i.consume_shortcut(&SAVE) {
            cmds.push(Command::Save);
        }
        if i.consume_shortcut(&OPEN) {
            cmds.push(Command::Open);
        }
        if i.consume_shortcut(&NEW) {
            cmds.push(Command::NewSurvey);
        }
        if i.consume_shortcut(&PRINT) {
            cmds.push(Command::Print);
        }
        if i.consume_shortcut(&EXIT) {
            cmds.push(Command::Exit);
        }
        if i.consume_shortcut(&DIAGNOSTICS) {
            cmds.push(Command::ExportDiagnostics);
        }
        if i.consume_shortcut(&HELP) {
            cmds.push(Command::ShowSection(SidebarSection::Help));
        }
        for (k, (tab, _, _)) in TAB_KEYS.iter().zip(TABS.iter()) {
            if i.consume_key(Modifiers::COMMAND, *k) {
                cmds.push(Command::SwitchTab(*tab));
            }
        }
        if typing {
            return;
        }
        if i.consume_shortcut(&RECORD_DELAYED) {
            cmds.push(Command::Record { delayed: true });
        } else if i.consume_shortcut(&RECORD_NEW_ROW) {
            cmds.push(Command::RecordNewRow { delayed: false });
        } else if i.consume_shortcut(&RECORD) {
            cmds.push(Command::Record { delayed: false });
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
            cmds.push(Command::PrevPoint);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
            cmds.push(Command::NextPoint);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            cmds.push(Command::PrevRow);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            cmds.push(Command::NextRow);
        }
        if state.pending_record.is_some() && i.consume_key(Modifiers::NONE, Key::Escape) {
            cmds.push(Command::CancelDelayedRecord);
        }
        if i.consume_key(Modifiers::NONE, Key::T) {
            cmds.push(Command::ToggleTracer);
        }
    });
    state.commands.extend(cmds);
}

/// (shortcut, description fallback) pairs for the help center.
#[allow(dead_code)] // helper for the help center (app-panels)
pub fn all(ctx: &egui::Context) -> Vec<(String, &'static str)> {
    let mut v = vec![
        (label(ctx, &NEW), "New survey"),
        (label(ctx, &OPEN), "Load"),
        (label(ctx, &SAVE), "Save"),
        (label(ctx, &SAVE_AS), "Save as"),
        (label(ctx, &PRINT), "Print"),
        (label(ctx, &RECORD), "Record"),
        (label(ctx, &RECORD_DELAYED), "Record delayed"),
        (label(ctx, &RECORD_NEW_ROW), "Rec + new row"),
        ("← / →".to_string(), "Previous / next point"),
        ("↑ / ↓".to_string(), "Previous / next row"),
        ("Esc".to_string(), "Cancel delayed record"),
        ("T".to_string(), "Toggle tracer"),
        (label(ctx, &HELP), "Help"),
    ];
    v.push((format!("{}1 … 7", if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" }), "Switch view"));
    v
}
