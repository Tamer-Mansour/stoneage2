//! Embedded assets. `build.rs` selects the original StoneAge 1.49 files when they are present
//! locally (`assets/original/`, not in git) and the own replacements (`assets/public/`) otherwise.

pub const SPLASH_PNG: &[u8] = include_bytes!(env!("SA_SPLASH"));

/// Colour palettes (`.act`, 256 × RGB) in the original `ColorMap` order.
pub const MAPS: [&[u8]; 10] = [
    include_bytes!(env!("SA_MAP_0")),
    include_bytes!(env!("SA_MAP_1")),
    include_bytes!(env!("SA_MAP_2")),
    include_bytes!(env!("SA_MAP_3")),
    include_bytes!(env!("SA_MAP_4")),
    include_bytes!(env!("SA_MAP_5")),
    include_bytes!(env!("SA_MAP_6")),
    include_bytes!(env!("SA_MAP_7")),
    include_bytes!(env!("SA_MAP_8")),
    include_bytes!(env!("SA_MAP_9")),
];

/// License page shown in Help (HTML; the original pages are Windows-1252).
pub const LICENSE_DE: &[u8] = include_bytes!(env!("SA_LICENSE_DE"));
pub const LICENSE_EN: &[u8] = include_bytes!(env!("SA_LICENSE_EN"));
pub const LICENSE_TR: &[u8] = include_bytes!(env!("SA_LICENSE_TR"));

/// UI string bundles, base layer (Java `.properties`, UTF-8): every key except `App.*`,
/// from `assets/original/i18n/` or `assets/public/i18n/`.
pub const I18N_DE: &str = include_str!(env!("SA_I18N_DE"));
pub const I18N_EN: &str = include_str!(env!("SA_I18N_EN"));
pub const I18N_TR: &str = include_str!(env!("SA_I18N_TR"));

/// UI string bundles, app layer: the `App.*` keys of the new UI (`assets/i18n/app_<lang>`,
/// shared by all builds). Merged on top of the base layer; app keys win.
pub const APP_I18N_DE: &str = include_str!("../assets/i18n/app_de_DE.properties");
pub const APP_I18N_EN: &str = include_str!("../assets/i18n/app_en_US.properties");
pub const APP_I18N_TR: &str = include_str!("../assets/i18n/app_tr_TR.properties");

/// True when this build embeds the original StoneAge 1.49 assets.
pub const ORIGINAL_ASSETS: bool = matches!(env!("SA_ORIGINAL_ASSETS").as_bytes(), b"1");
