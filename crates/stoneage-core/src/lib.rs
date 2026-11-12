//! StoneAge core: data model, file formats, settings, i18n, colour maps and signal math.
//!
//! OWNER: agent "core". Everything here is a faithful port of the Java classes
//! `MessPunkt`, `MessReihe`, `MessAreal`, `ColorMap`, `FastFourierTransform`,
//! the `StoneAge.ini` handling in `MainFrame`/`Start`, and the `.saf` / CSV formats.
//!
//! CONTRACT: the public names declared in this skeleton are used by the other crates
//! while they are being written in parallel. You may ADD fields/functions freely.
//! Do not rename or remove a public item; if a signature is truly wrong, keep the old
//! one as a thin wrapper and document the change in docs/CHANGES-core.md.

pub mod assets;
pub mod colormap;
pub mod config;
pub mod csv;
pub mod error;
pub mod fft;
pub mod i18n;
pub mod javafmt;
pub mod model;
pub mod saf;

pub use config::AppConfig;
pub use error::{Error, Result};
pub use i18n::{I18n, Language};
pub use model::{MapPlacement, Point, Row, Survey, NUM_SAMPLES};

/// Version string of the original application this port reproduces.
pub const ORIGINAL_VERSION: &str = "1.49, 2012/04/11, mm";
/// `.saf` file format identifier written by the original (`MainFrame.fileFormat`).
pub const FILE_FORMAT: &str = "003 - 2006.12.28";
/// Splash / logo image (PNG bytes) shipped with the original.
pub const SPLASH_PNG: &[u8] = assets::SPLASH_PNG;
