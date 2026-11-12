//! Picks each embedded asset from `assets/original/` (the original StoneAge 1.49 files – kept
//! locally only, ignored by git) when present, otherwise from `assets/public/` (own replacements
//! used for public builds and GitHub releases).
//!
//! Shared assets that exist in one variant only (e.g. the `App.*` translation layer in
//! `assets/i18n/app_<lang>.properties`) are included directly by `src/assets.rs`.
use std::path::Path;

/// (env var, path under `assets/original/`, path under `assets/public/`)
const ASSETS: &[(&str, &str, &str)] = &[
    ("SA_SPLASH", "splash.png", "splash.png"),
    ("SA_MAP_0", "maps/graustufen.act", "maps/graustufen.act"),
    ("SA_MAP_1", "maps/schwarzerkoerper.act", "maps/schwarzerkoerper.act"),
    ("SA_MAP_2", "maps/spektrum.act", "maps/spektrum.act"),
    ("SA_MAP_3", "maps/system-win.act", "maps/system-win.act"),
    ("SA_MAP_4", "maps/system-mac.act", "maps/system-mac.act"),
    ("SA_MAP_5", "maps/ei310ak2.act", "maps/ei310ak2.act"),
    ("SA_MAP_6", "maps/tet_duo.act", "maps/tet_duo.act"),
    ("SA_MAP_7", "maps/jakub.act", "maps/jakub.act"),
    ("SA_MAP_8", "maps/fox.act", "maps/fox.act"),
    ("SA_MAP_9", "maps/atari.act", "maps/atari.act"),
    ("SA_LICENSE_DE", "help/de_DE/license.html", "help/de_DE/license.html"),
    ("SA_LICENSE_EN", "help/en_US/license.html", "help/en_US/license.html"),
    ("SA_LICENSE_TR", "help/tr_TR/license.html", "help/tr_TR/license.html"),
    // Base translation layer (all keys except `App.*`). The original Turkish bundle exists twice
    // (`tr_TR.properties` is a legacy ASCII-only copy); the UTF-8 one is used.
    ("SA_I18N_DE", "i18n/de_DE.properties", "i18n/de_DE.properties"),
    ("SA_I18N_EN", "i18n/en_US.properties", "i18n/en_US.properties"),
    ("SA_I18N_TR", "i18n/tr_TR.utf8.properties", "i18n/tr_TR.properties"),
];

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut all_original = true;
    for (var, original_rel, public_rel) in ASSETS {
        let original = root.join("original").join(original_rel);
        let public = root.join("public").join(public_rel);
        for p in [&original, &public] {
            println!("cargo:rerun-if-changed={}", p.display());
        }
        let chosen = if original.exists() {
            original
        } else {
            all_original = false;
            assert!(public.exists(), "missing asset: {} (neither original nor public)", public.display());
            public
        };
        println!("cargo:rustc-env={var}={}", chosen.display());
    }
    // Directory itself, so renaming/removing assets/original/ triggers a rebuild.
    println!("cargo:rerun-if-changed={}", root.join("original").display());
    println!("cargo:rustc-env=SA_ORIGINAL_ASSETS={}", if all_original { "1" } else { "0" });
}
