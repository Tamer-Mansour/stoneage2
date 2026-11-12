//! Faithful port of the logic of `StoneAge.OptionPanel` (the original parameter dialog).
//!
//! * `VALID_KB` / `VALID_F_RANGE` – the `validKb` / `validfRange` arrays of the constructor.
//! * `OptionForm` – the text-field / combo-box contents (`tfName`, `tfDatum`, `taBemerkung`,
//!   `tf_dfh`, `tfMesspunktAbstand`, `tfMessreihenAbstand`, `jcb_kb`, `jcb_fRange`).
//!   Sliders and check boxes edit the survey directly (immediate mode: their value *is* the model).
//! * `load_values` = `loadValues()`, `set_values` = `setValues()`.
//! * `doNotSetValues` only suppressed the Swing change events fired by `loadValues`; in immediate
//!   mode loading never fires events, so it is not needed.

use crate::state::AppState;

use super::java_double;

/// `OptionPanel.validKb` (Kb, displayed in the ini/combo as factor 1000).
pub(crate) const VALID_KB: [f64; 15] =
    [0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8];
/// `OptionPanel.validfRange` [kHz].
pub(crate) const VALID_F_RANGE: [f64; 8] = [30.0, 60.0, 120.0, 300.0, 600.0, 1200.0, 3000.0, 6000.0];

/// Slider ranges from `jbInit`.
pub(crate) const ZOOM_X_RANGE: (i32, i32) = (1, 61);
pub(crate) const ZOOM_Y_RANGE: (i32, i32) = (1, 35);
pub(crate) const ZOOM_Z_RANGE: (i32, i32) = (1, 61);
pub(crate) const BLOCK_SIZE_RANGE: (i32, i32) = (1, 32);
/// `sldHorizontalSchnitt`: 0..2048, major ticks every 512 (labels = `tiefenLabels`).
pub(crate) const HCUT_RANGE: (i32, i32) = (0, 2048);
pub(crate) const HCUT_LABEL_STEP: i32 = 512;
/// `jsldValidStep`: 1..10. `jsldValidBase`: 0..validStep-1.
pub(crate) const VALID_STEP_RANGE: (i32, i32) = (1, 10);

/// The values shown by the form, as they were last loaded/applied from the survey.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Snapshot {
    pub name: String,
    pub date: String,
    pub description: String,
    pub dfh: f64,
    pub point_distance: f64,
    pub row_distance: f64,
    pub kb: f64,
    pub f_range: f64,
    pub tm: f64,
}

impl Snapshot {
    pub fn of(state: &AppState) -> Self {
        let s = &state.survey;
        Self {
            name: s.name.clone(),
            date: s.date.clone(),
            description: s.description.clone(),
            dfh: s.dfh,
            point_distance: s.point_distance,
            row_distance: s.row_distance,
            kb: s.kb,
            f_range: s.f_range,
            tm: s.tm,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct OptionForm {
    /// What the form was loaded from; when the survey changes behind our back (file loaded,
    /// new survey, ...) the form is reloaded (`MainFrame.loadValues` → `OptionPanel.loadValues`).
    pub loaded: Option<Snapshot>,
    pub name: String,
    pub date: String,
    pub description: String,
    pub dfh: String,
    pub point_distance: String,
    pub row_distance: String,
    pub tm: String,
    /// `jcb_kb.getSelectedIndex()` (`None` = -1, custom value typed into the editable combo).
    pub kb_index: Option<usize>,
    /// Editable text of `jcb_kb` (Kb × 1000).
    pub kb_text: String,
    /// `jcb_fRange.getSelectedIndex()`.
    pub fr_index: Option<usize>,
    pub fr_text: String,
}

const FORM_KEY: &str = "option_form";

impl OptionForm {
    /// Fetch the form from egui memory and reload it if the survey changed.
    pub fn get(ui: &egui::Ui, state: &AppState) -> Self {
        let mut f: OptionForm = super::mem(ui, FORM_KEY);
        let now = Snapshot::of(state);
        if f.loaded.as_ref() != Some(&now) {
            f.load_values(state);
        }
        f
    }
    pub fn store(self, ui: &egui::Ui) {
        super::mem_set(ui, FORM_KEY, self);
    }

    /// `loadValues()`
    pub fn load_values(&mut self, state: &AppState) {
        let a = &state.survey;
        self.name = a.name.clone();
        self.date = a.date.clone();
        self.dfh = java_double(a.dfh);
        // jcb_fRange.setSelectedItem(""+fRange) (no match in a non-editable combo),
        // then select the index whose value equals fRange.
        self.fr_text = java_double(a.f_range);
        if let Some(i) = VALID_F_RANGE.iter().position(|v| *v == a.f_range) {
            self.fr_index = Some(i);
        }
        // jcb_kb.setSelectedItem(""+round(Kb*1000*100)/100) (editable → becomes the text)
        self.kb_text = java_double((a.kb * 1000.0 * 100.0).round() / 100.0);
        self.kb_index = VALID_KB.iter().position(|v| *v == a.kb);
        self.point_distance = java_double(a.point_distance);
        self.row_distance = java_double(a.row_distance);
        self.description = a.description.clone();
        self.tm = java_double(a.tm);
        self.loaded = Some(Snapshot::of(state));
    }

    /// True if text fields contain edits that have not been applied yet.
    pub fn has_pending_edits(&self) -> bool {
        let Some(l) = &self.loaded else { return false };
        let num_differs = |text: &str, v: f64| text.trim().parse::<f64>().map(|p| p != v).unwrap_or(true);
        self.name != l.name
            || self.date != l.date
            || self.description != l.description
            || num_differs(&self.dfh, l.dfh)
            || num_differs(&self.point_distance, l.point_distance)
            || num_differs(&self.row_distance, l.row_distance)
            || num_differs(&self.tm, l.tm)
            || (self.kb_index.is_none() && num_differs(&self.kb_text, l.kb * 1000.0))
    }

    /// `setValues()` – copies every control into the survey (called by Apply, by Enter in the
    /// text fields, by the combo boxes, the check boxes and every slider change).
    pub fn set_values(&mut self, state: &mut AppState) {
        let mut log: Vec<String> = Vec::new();
        {
            let a = &mut state.survey;
            a.name = self.name.clone();
            a.description = self.description.clone();
            a.date = self.date.clone();
            match parse_double(&self.point_distance) {
                Some(v) => a.point_distance = v,
                None => {
                    log.push("Messpunktabstand: Ungültige Eingabe!".into());
                    self.point_distance = java_double(a.point_distance);
                }
            }
            match parse_double(&self.row_distance) {
                Some(v) => a.row_distance = v,
                None => {
                    // (sic) the original logs the point-distance message here as well
                    log.push("Messpunktabstand: Ungültige Eingabe!".into());
                    self.row_distance = java_double(a.row_distance);
                }
            }
            match self.fr_index.and_then(|i| VALID_F_RANGE.get(i)) {
                Some(v) => a.f_range = *v,
                None => {
                    log.push("fRange: not in List!".into());
                    match parse_double(&self.fr_text) {
                        Some(v) => a.f_range = v / 1000.0,
                        None => {
                            log.push("fRange: not a Number!".into());
                            self.fr_text = java_double(a.f_range);
                        }
                    }
                }
            }
            match self.kb_index.and_then(|i| VALID_KB.get(i)) {
                Some(v) => a.kb = *v,
                None => {
                    log.push("kb: not in List!".into());
                    match parse_double(&self.kb_text) {
                        Some(v) => a.kb = v / 1000.0,
                        None => {
                            log.push("kb: not a Number!".into());
                            self.kb_text = java_double((a.kb * 1000.0 * 100.0).round() / 100.0);
                        }
                    }
                }
            }
            match parse_double(&self.dfh) {
                Some(v) => a.dfh = v,
                None => {
                    log.push("dfh: Ungültige Eingabe!".into());
                    self.dfh = java_double(a.dfh);
                }
            }
            // Tm has no editor in the original panel (only an ini/.saf value); added here.
            match parse_double(&self.tm) {
                Some(v) => a.tm = v,
                None => {
                    log.push("Tm: Ungültige Eingabe!".into());
                    self.tm = java_double(a.tm);
                }
            }
            // Sliders: disp_stepx/y/z, blockSize, horizontalschnitttiefe are bound directly.
            // validBase / validStep of the current row, validBase max = validStep - 1.
            if let Some(r) = a.current_row_mut() {
                r.valid_step = r.valid_step.clamp(VALID_STEP_RANGE.0, VALID_STEP_RANGE.1);
                r.valid_base = r.valid_base.clamp(0, (r.valid_step - 1).max(0));
            }
        }
        for l in log {
            state.log(log::Level::Warn, format!("OptionPanel: {l}"));
        }
        state.dirty = true;
        self.loaded = Some(Snapshot::of(state));
    }
}

/// `Double.parseDouble` (accepts surrounding whitespace, a trailing d/f suffix like Java).
pub(crate) fn parse_double(s: &str) -> Option<f64> {
    let t = s.trim();
    let t = t.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(t);
    t.parse::<f64>().ok()
}

/// Apply all values (setValues) for a control that changed this frame.
pub(crate) fn apply_now(ui: &egui::Ui, state: &mut AppState) {
    let mut f = OptionForm::get(ui, state);
    f.set_values(state);
    f.store(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrays_match_original() {
        assert_eq!(VALID_KB.len(), 15);
        assert_eq!(VALID_KB[7], 1.1);
        assert_eq!(VALID_F_RANGE.len(), 8);
        assert_eq!(VALID_F_RANGE[3], 300.0);
    }

    #[test]
    fn parse_like_java() {
        assert_eq!(parse_double(" 2.5 "), Some(2.5));
        assert_eq!(parse_double("3d"), Some(3.0));
        assert_eq!(parse_double("abc"), None);
        assert_eq!(parse_double(""), None);
    }
}
