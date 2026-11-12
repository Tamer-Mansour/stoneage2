//! File dialogs (rfd) with the filters of the original `FileFilter` classes:
//! `StoneAgeFileFilter` (.saf), `CSVFileFilter` (.csv), `ImageFileFilter` (.jpg, .gif, .png).

use std::path::{Path, PathBuf};

use stoneage_core::I18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Saf,
    Csv,
    Image,
}

impl Kind {
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::Saf => &["saf"],
            Kind::Csv => &["csv"],
            Kind::Image => &["jpg", "gif", "png"],
        }
    }

    fn description(self, i18n: &I18n) -> String {
        let (key, fallback) = match self {
            Kind::Saf => ("StoneAgeFileFilter.Description", "StoneAge Files (*.saf)"),
            Kind::Csv => ("CSVFileFilter.Description", "CSV Files (*.csv)"),
            Kind::Image => ("ImageFileFilter.Description", "Images (*.jpg,*.gif,*.png)"),
        };
        super::t(i18n, key, fallback)
    }
}

fn dialog(i18n: &I18n, kind: Kind, dir: Option<&Path>) -> rfd::FileDialog {
    let mut d = rfd::FileDialog::new().add_filter(kind.description(i18n), kind.extensions());
    if let Some(dir) = dir.filter(|d| d.is_dir()) {
        d = d.set_directory(dir);
    }
    d
}

/// `JFileChooser.showOpenDialog` with the given filter.
pub fn open(i18n: &I18n, kind: Kind, dir: Option<&Path>) -> Option<PathBuf> {
    dialog(i18n, kind, dir).pick_file()
}

/// `JFileChooser.showSaveDialog`; the default extension is appended if missing
/// (original: `if (!name.endsWith(".saf")) name += ".saf"`, same for ".csv").
pub fn save(i18n: &I18n, kind: Kind, dir: Option<&Path>, file_name: Option<&str>) -> Option<PathBuf> {
    let mut d = dialog(i18n, kind, dir);
    if let Some(n) = file_name {
        d = d.set_file_name(n);
    }
    d.save_file().map(|p| ensure_extension(p, kind))
}

/// Appends the default extension unless the path already ends with an accepted one.
pub fn ensure_extension(path: PathBuf, kind: Kind) -> PathBuf {
    let ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            kind.extensions().iter().any(|x| *x == e) || (kind == Kind::Image && e == "jpeg")
        })
        .unwrap_or(false);
    if ok {
        path
    } else {
        let default = match kind {
            Kind::Image => "png",
            k => k.extensions()[0],
        };
        let mut s = path.into_os_string();
        s.push(".");
        s.push(default);
        PathBuf::from(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_appended() {
        assert_eq!(ensure_extension("a/b".into(), Kind::Saf), PathBuf::from("a/b.saf"));
        assert_eq!(ensure_extension("a/b.saf".into(), Kind::Saf), PathBuf::from("a/b.saf"));
        assert_eq!(ensure_extension("x.txt".into(), Kind::Csv), PathBuf::from("x.txt.csv"));
        assert_eq!(ensure_extension("img.JPG".into(), Kind::Image), PathBuf::from("img.JPG"));
        assert_eq!(ensure_extension("img".into(), Kind::Image), PathBuf::from("img.png"));
    }
}
