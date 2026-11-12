//! `StoneAge.ini` – program defaults ("Save Defaults" writes it back).
//!
//! Port of the `props` handling in `MainFrame.<clinit>` (load: local `StoneAge.ini`
//! (`MainFrame.propertyFilename`), else the built-in jar resource, else hard-coded values) and
//! of the "Save Ini" branch of `MainFrame.actionPerformed` (fixed header comments + key order).

use std::path::Path;

use crate::i18n::{escape_properties, parse_properties};
use crate::javafmt::java_double;

/// `MainFrame.propertyFilename` – relative to the working directory.
pub const PROPERTY_FILENAME: &str = "StoneAge.ini";

/// Keys handled explicitly (everything else lands in [`AppConfig::extra`]).
const KNOWN_KEYS: [&str; 14] = [
    "Language",
    "Country",
    "RecordDelay",
    "PointDistance",
    "RowDistance",
    "dfh",
    "Kb",
    "fRange",
    "disp_stepx",
    "disp_stepy",
    "disp_stepz",
    "colorModel",
    "colorModelBase",
    "showValidBase",
];

#[derive(Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub language: String,
    pub country: String,
    /// `RecordDelay` [ms] for "Record delayed".
    pub record_delay_ms: u64,
    pub point_distance: f64,
    pub row_distance: f64,
    pub dfh: f64,
    pub kb: f64,
    pub f_range: f64,
    pub disp_step_x: i32,
    pub disp_step_y: i32,
    pub disp_step_z: i32,
    pub color_model: i32,
    pub color_model_base: i32,
    pub show_valid_base: bool,
    /// Any other keys found in the file, preserved on save.
    pub extra: Vec<(String, String)>,
}

impl Default for AppConfig {
    /// The built-in `StoneAge.ini` resource of the jar (used by the original when no local
    /// file exists).
    fn default() -> Self {
        Self::parse_onto(Self::hardcoded(), include_str!("../assets/StoneAge.default.ini"))
    }
}

impl AppConfig {
    /// Values the original set when neither a local nor the built-in ini could be read
    /// ("No resource ini file - using default ...").
    pub fn hardcoded() -> Self {
        Self {
            language: "en".into(),
            country: "US".into(),
            record_delay_ms: 5000,
            point_distance: 5.0,
            row_distance: 5.0,
            dfh: 40.0,
            kb: 1.1,
            f_range: 30.0,
            disp_step_x: 32,
            disp_step_y: 3,
            disp_step_z: 32,
            color_model: 0,
            color_model_base: 0,
            show_valid_base: false,
            extra: Vec::new(),
        }
    }

    /// Parses ini text (Java `Properties` syntax). Keys missing or unparsable keep the values
    /// of the built-in default ini.
    pub fn parse(text: &str) -> Self {
        Self::parse_onto(Self::default(), text)
    }

    /// Parses `text` on top of `base`.
    pub fn parse_onto(base: Self, text: &str) -> Self {
        let mut c = base;
        c.extra.clear();
        for (k, v) in parse_properties(text) {
            let t = v.trim();
            match k.as_str() {
                "Language" => c.language = v.clone(),
                "Country" => c.country = v.clone(),
                "RecordDelay" => set(&mut c.record_delay_ms, t),
                "PointDistance" => set(&mut c.point_distance, t),
                "RowDistance" => set(&mut c.row_distance, t),
                "dfh" => set(&mut c.dfh, t),
                "Kb" => set(&mut c.kb, t),
                "fRange" => set(&mut c.f_range, t),
                "disp_stepx" => set(&mut c.disp_step_x, t),
                "disp_stepy" => set(&mut c.disp_step_y, t),
                "disp_stepz" => set(&mut c.disp_step_z, t),
                "colorModel" => set(&mut c.color_model, t),
                "colorModelBase" => set(&mut c.color_model_base, t),
                // Boolean.valueOf: true only for "true" (case-insensitive)
                "showValidBase" => c.show_valid_base = v.eq_ignore_ascii_case("true"),
                _ => {
                    if let Some(e) = c.extra.iter_mut().find(|(ek, _)| *ek == k) {
                        e.1 = v;
                    } else {
                        c.extra.push((k, v));
                    }
                }
            }
        }
        c
    }

    /// Reads an ini file. The original read it as ISO-8859-1 (`Properties.load(InputStream)`);
    /// UTF-8 files are accepted too.
    pub fn load(path: &Path) -> crate::Result<Self> {
        let bytes = std::fs::read(path)?;
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => encoding_rs::mem::decode_latin1(&bytes).into_owned(),
        };
        Ok(Self::parse(&text))
    }

    /// Like the original startup: local file if readable, otherwise the built-in defaults.
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// The file content "Save Defaults" writes, with the given timestamp text
    /// (`dd.MM.yyyy, HH:mm 'Uhr'`). Lines end with `\r\n` (Windows `PrintWriter.println`).
    pub fn to_ini_string(&self, saved_on: &str) -> String {
        let mut lines: Vec<String> = vec![
            format!("# StoneAge.ini - saved on {saved_on}"),
            "# Stoneage Properties".into(),
            "# courrently supported: de_DE, en_US".into(),
            "# Language: lowercase two-letter ISO-639 code.".into(),
            "# erlaubt: de, en, tr".into(),
            format!("Language = {}", self.language),
            "# Country: - uppercase two-letter ISO-3166 code.".into(),
            "# erlaubt: DE, US, TR".into(),
            format!("Country = {}", self.country),
            "# RecordDelay: how long (in milliseconds) do we wait before record in delayed record"
                .into(),
            format!("RecordDelay = {}", self.record_delay_ms),
            "# Point distance [m]".into(),
            format!("PointDistance = {}", java_double(self.point_distance)),
            "# Row distance [m]".into(),
            format!("RowDistance = {}", java_double(self.row_distance)),
            "# Delta fh []".into(),
            format!("dfh = {}", java_double(self.dfh)),
            "# Kb value is displayed by factor 1000".into(),
            format!("Kb = {}", java_double(self.kb)),
            "# fRange value of Oszilloscope".into(),
            format!("fRange = {}", java_double(self.f_range)),
            "# disp_stepx".into(),
            format!("disp_stepx = {}", self.disp_step_x),
            "# disp_stepy".into(),
            format!("disp_stepy = {}", self.disp_step_y),
            "# disp_stepz".into(),
            format!("disp_stepz = {}", self.disp_step_z),
            "# colorModel".into(),
            format!("colorModel = {}", self.color_model),
            "# colorModelBase".into(),
            format!("colorModelBase = {}", self.color_model_base),
            "# showValidBase (should the sliders for validBase be visible?)".into(),
            format!("showValidBase = {}", self.show_valid_base),
        ];
        // Deviation: the original dropped unknown keys; we keep them after the known block.
        let extras: Vec<_> =
            self.extra.iter().filter(|(k, _)| !KNOWN_KEYS.contains(&k.as_str())).collect();
        if !extras.is_empty() {
            lines.push("# other settings".into());
            for (k, v) in extras {
                lines.push(format!(
                    "{} = {}",
                    escape_properties(k, true),
                    escape_properties(v, false)
                ));
            }
        }
        let mut s = lines.join("\r\n");
        s.push_str("\r\n");
        s
    }

    /// Writes the same layout/comments as the original (`# StoneAge.ini - saved on ...`).
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        let now = chrono::Local::now().format("%d.%m.%Y, %H:%M Uhr").to_string();
        std::fs::write(path, self.to_ini_string(&now))?;
        Ok(())
    }
}

fn set<T: std::str::FromStr>(field: &mut T, v: &str) {
    if let Ok(x) = v.parse() {
        *field = x;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ini() {
        let c = AppConfig::default();
        assert_eq!(c.language, "en");
        assert_eq!(c.country, "US");
        assert_eq!(c.record_delay_ms, 5000);
        assert_eq!(c.point_distance, 2.0);
        assert_eq!(c.row_distance, 2.0);
        assert_eq!(c.dfh, 400.0);
        assert_eq!(c.kb, 1.1);
        assert_eq!(c.f_range, 300.0);
        assert_eq!((c.disp_step_x, c.disp_step_y, c.disp_step_z), (32, 3, 32));
        assert_eq!((c.color_model, c.color_model_base), (0, 0));
        assert!(!c.show_valid_base);
        assert!(c.extra.is_empty());
    }

    #[test]
    fn save_matches_original_layout() {
        let c = AppConfig::default();
        let s = c.to_ini_string("03.01.2017, 12:05 Uhr");
        let original = include_str!("../assets/StoneAge.default.ini");
        let norm = |t: &str| t.replace("\r\n", "\n").trim_end().to_string();
        assert_eq!(norm(&s), norm(original));
        assert!(s.contains("\r\n"));
    }

    #[test]
    fn roundtrip_with_unknown_keys() {
        let mut c = AppConfig::default();
        c.language = "de".into();
        c.country = "DE".into();
        c.kb = 0.7;
        c.f_range = 3000.0;
        c.color_model = 4;
        c.color_model_base = -17;
        c.show_valid_base = true;
        c.extra.push(("App.Theme".into(), "light".into()));
        c.extra.push(("Recent File 1".into(), "C:\\Daten\\Messung ü.saf".into()));
        let text = c.to_ini_string("01.01.2026, 00:00 Uhr");
        let back = AppConfig::parse(&text);
        assert_eq!(back, c);

        let dir = std::env::temp_dir().join(format!("stoneage-ini-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(PROPERTY_FILENAME);
        c.save(&p).unwrap();
        assert_eq!(AppConfig::load(&p).unwrap(), c);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_and_bad_values_fall_back() {
        let c = AppConfig::parse("Kb = abc\nshowValidBase = TRUE\nfoo=bar\n");
        assert_eq!(c.kb, 1.1);
        assert!(c.show_valid_base);
        assert_eq!(c.extra, vec![("foo".to_string(), "bar".to_string())]);
        assert_eq!(c.point_distance, 2.0);
    }
}
