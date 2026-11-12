//! Print and "Export image".
//!
//! Original "Print" (`actionCommand "printData"`) printed the current row (`MessReihe.print`) through
//! the Java print dialog. Here the current view (Areal view for the Map/3D/Areal tabs, otherwise the
//! row section) is rendered with `RowView::render_image` / `ArealView::render_image`, written to a PNG
//! in the temp directory and handed to the Windows shell "print" verb.

use std::path::{Path, PathBuf};

use stoneage_plots::RowView;
use stoneage_spatial::ArealView;

use crate::state::{AppState, Tab};

pub const PRINT_WIDTH: u32 = 2000;
pub const PRINT_HEIGHT: u32 = 1400;

/// Renders the image that belongs to the current tab.
pub fn render(state: &AppState, width: u32, height: u32) -> egui::ColorImage {
    match state.tab {
        Tab::Map | Tab::View3d | Tab::Areal => ArealView::render_image(&state.survey, width, height),
        _ => RowView::render_image(&state.survey, &state.i18n, width, height),
    }
}

pub fn to_rgba(img: &egui::ColorImage) -> image::RgbaImage {
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        raw.extend_from_slice(&c.to_srgba_unmultiplied());
    }
    image::RgbaImage::from_raw(w as u32, h as u32, raw).unwrap_or_else(|| image::RgbaImage::new(1, 1))
}

/// Saves as PNG / GIF / JPEG depending on the extension (JPEG without alpha).
pub fn save_image(img: &egui::ColorImage, path: &Path) -> Result<(), String> {
    let rgba = to_rgba(img);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    let res = if ext == "jpg" || ext == "jpeg" {
        image::DynamicImage::ImageRgba8(rgba).to_rgb8().save(path)
    } else {
        rgba.save(path)
    };
    res.map_err(|e| e.to_string())
}

/// Temp file used for printing.
pub fn print_file() -> PathBuf {
    std::env::temp_dir().join(format!("stoneage-print-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S")))
}

/// Opens the Windows print dialog for `path` (shell verb "print").
pub fn print_path(path: &Path) -> Result<(), String> {
    let quoted = path.display().to_string().replace('\'', "''");
    let script = format!("Start-Process -FilePath '{quoted}' -Verb Print");
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match cmd.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(_) | Err(_) => {
            // Fallback: Paint's print switch (default printer).
            let status = std::process::Command::new("mspaint.exe")
                .arg("/p")
                .arg(path)
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("print failed ({status})"))
            }
        }
    }
}
