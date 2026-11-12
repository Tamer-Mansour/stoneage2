//! Port of `ColorMap`: the `.act` palettes in assets/maps (what the original actually uses)
//! plus the built-in calculated maps (`maps` / `calcMaps` / `func`).
//!
//! Faithfulness notes (from the bytecode):
//! * The static initialiser only calls `readMaps()`; `calcMaps()` is never invoked, so all
//!   `numberOfMaps` (= `mapnames.length` = 10) selectable models come from the `.act` files.
//!   The calculated maps are still provided by [`calculated_maps`] for completeness.
//! * `getMap` writes `nullColor` into palette entry 0 (index 0 = "no data").
//! * `nullColor` is `new Color(0, 255, 0, 255)` (opaque green).

use std::sync::OnceLock;

/// `ColorMap.INTERPOLATE`
pub const INTERPOLATE: i32 = 0;
/// `ColorMap.STEP`
pub const STEP: i32 = 1;

/// `ColorMap.mapnames` – palette names, in the original order (also the `.act` file stems).
pub const MAP_NAMES: [&str; 10] = [
    "graustufen",
    "schwarzerkoerper",
    "spektrum",
    "system-win",
    "system-mac",
    "ei310ak2",
    "tet_duo",
    "jakub",
    "fox",
    "atari",
];

/// Raw `.act` files (256 × RGB), same order as [`MAP_NAMES`].
const ACT_FILES: [&[u8]; 10] = crate::assets::MAPS;

/// Number of selectable colour models (`ColorMap.numberOfMaps`).
pub fn count() -> usize {
    MAP_NAMES.len()
}

/// Display name of colour model `idx` (`getModelName`). Out-of-range indices are clamped
/// (the original would throw `ArrayIndexOutOfBoundsException`).
pub fn name(idx: usize) -> &'static str {
    MAP_NAMES[idx.min(MAP_NAMES.len() - 1)]
}

/// `colorValues[model][channel][i]` as read by `readMaps()`.
/// `InputStream.read()` returns -1 at EOF which is stored as byte 0xFF; replicated here.
fn color_values() -> &'static [[[u8; 256]; 3]; 10] {
    static CV: OnceLock<[[[u8; 256]; 3]; 10]> = OnceLock::new();
    CV.get_or_init(|| {
        let mut cv = [[[0u8; 256]; 3]; 10];
        for (m, bytes) in ACT_FILES.iter().enumerate() {
            let mut pos = 0usize;
            let mut next = || {
                let b = bytes.get(pos).copied().unwrap_or(0xFF);
                pos += 1;
                b
            };
            for i in 0..256 {
                cv[m][0][i] = next();
                cv[m][1][i] = next();
                cv[m][2][i] = next();
            }
        }
        cv
    })
}

/// Palette entry of the raw `.act` table (without base shift and null colour).
pub fn raw_color(model: usize, i: u8) -> [u8; 3] {
    let cv = color_values();
    let m = model.min(count() - 1);
    let i = i as usize;
    [cv[m][0][i], cv[m][1][i], cv[m][2][i]]
}

/// 256-entry RGBA lookup table (`getMap(model, reverse, base)`).
/// `base` is `colorModelBase`, the offset set by dragging the scale bar.
///
/// For every palette index `i`: `j = reverse ? 255 - (i + base) : i + base`,
/// `j = (j + 256) % 256`, colour = `colorValues[model][*][j]`, alpha 255;
/// finally entry 0 is replaced by [`NULL_COLOR`].
/// `model >= count()` uses the last model, like the original.
pub fn lut(model: usize, reverse: bool, base: i32) -> [[u8; 4]; 256] {
    let model = model.min(count() - 1);
    let cv = color_values();
    let mut out = [[0u8; 4]; 256];
    for (i, entry) in out.iter_mut().enumerate() {
        let i = i as i32;
        let j = if reverse { 255 - (i + base) } else { i + base };
        // Java: (j + 256) % 256 – only valid for j >= -256; rem_euclid is the safe superset.
        let j = (j + 256).rem_euclid(256) as usize;
        *entry = [cv[model][0][j], cv[model][1][j], cv[model][2][j], 255];
    }
    out[0] = NULL_COLOR;
    out
}

/// Same as [`lut`] but taking a (possibly negative / too large) Java `int` model index:
/// `model >= numberOfMaps` → last map, `model < 0` → map 0 (as `getMap` does).
pub fn lut_i32(model: i32, reverse: bool, base: i32) -> [[u8; 4]; 256] {
    lut(model.max(0) as usize, reverse, base)
}

/// Colour used for "no data" (`ColorMap.nullColor` = `new Color(0, 255, 0, 255)`).
pub const NULL_COLOR: [u8; 4] = [0, 255, 0, 255];

/// One calculated map definition from `ColorMap.maps`: mode (`INTERPOLATE`/`STEP`) and the
/// red/green/blue support points.
#[derive(Clone, Copy, Debug)]
pub struct CalcMapDef {
    pub mode: i32,
    pub r: &'static [i32],
    pub g: &'static [i32],
    pub b: &'static [i32],
}

const M0: [i32; 10] = [255, 0, 0, 127, 0, 127, 255, 204, 204, 255];
const M0G: [i32; 10] = [255, 0, 0, 255, 255, 127, 255, 153, 76, 0];
const M0B: [i32; 10] = [255, 127, 255, 127, 0, 0, 0, 0, 0, 0];
const M1R: [i32; 11] = [255, 0, 0, 127, 0, 127, 255, 204, 204, 255, 255];
const M1G: [i32; 11] = [255, 0, 0, 255, 255, 127, 255, 153, 76, 0, 0];
const M1B: [i32; 11] = [255, 127, 255, 127, 0, 0, 0, 0, 0, 0, 0];
const M2R: [i32; 7] = [255, 127, 0, 0, 255, 255, 255];
const M2G: [i32; 7] = [255, 127, 0, 255, 255, 0, 0];
const M2B: [i32; 7] = [255, 255, 255, 0, 0, 0, 0];
const M4: [i32; 4] = [255, 127, 0, 0];
const M5: [i32; 10] = [255, 223, 191, 159, 127, 95, 63, 31, 0, 0];
const DOWN: [i32; 2] = [255, 0];
const UP: [i32; 2] = [0, 255];

/// `ColorMap.maps` (9 definitions).
pub const CALC_MAPS: [CalcMapDef; 9] = [
    CalcMapDef { mode: INTERPOLATE, r: &M0, g: &M0G, b: &M0B },
    CalcMapDef { mode: STEP, r: &M1R, g: &M1G, b: &M1B },
    CalcMapDef { mode: STEP, r: &M2R, g: &M2G, b: &M2B },
    CalcMapDef { mode: INTERPOLATE, r: &M2R, g: &M2G, b: &M2B },
    CalcMapDef { mode: STEP, r: &M4, g: &M4, b: &M4 },
    CalcMapDef { mode: STEP, r: &M5, g: &M5, b: &M5 },
    CalcMapDef { mode: INTERPOLATE, r: &DOWN, g: &DOWN, b: &DOWN },
    CalcMapDef { mode: INTERPOLATE, r: &DOWN, g: &DOWN, b: &DOWN },
    CalcMapDef { mode: INTERPOLATE, r: &UP, g: &UP, b: &UP },
];

/// `ColorMap.func(idx, mode, arr)`: value of channel support points `arr` at palette index `idx`.
pub fn func(idx: i32, mode: i32, arr: &[i32]) -> u8 {
    let step = 255.0 / (arr.len() as f64 - 1.0);
    let i = (idx as f64 / step).floor() as i32;
    let erg = if i + 1 >= arr.len() as i32 {
        arr[arr.len() - 1] as f64
    } else if mode == INTERPOLATE {
        let i = i as usize;
        arr[i] as f64 + (idx as f64 - i as f64 * step) * (arr[i + 1] - arr[i]) as f64 / step
    } else {
        arr[i as usize] as f64
    };
    // values < 0 / > 255 are traced and clamped
    let erg = erg.clamp(0.0, 255.0);
    crate::javafmt::java_round(erg) as i32 as u8
}

/// `ColorMap.calcMaps()` – the calculated palettes (r, g, b per index). Unused by the original
/// at runtime (never called); note its loop bound was `numberOfMaps` (10) although only 9
/// definitions exist, so it would have thrown – here exactly the 9 definitions are computed.
pub fn calculated_maps() -> Vec<[[u8; 3]; 256]> {
    CALC_MAPS
        .iter()
        .map(|d| {
            let mut m = [[0u8; 3]; 256];
            for (i, e) in m.iter_mut().enumerate() {
                let i = i as i32;
                *e = [func(i, d.mode, d.r), func(i, d.mode, d.g), func(i, d.mode, d.b)];
            }
            m
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_count() {
        assert_eq!(count(), 10);
        assert_eq!(name(0), "graustufen");
        assert_eq!(name(5), "ei310ak2");
        assert_eq!(name(9), "atari");
        assert_eq!(name(99), "atari");
    }

    #[test]
    fn grey_lut() {
        let l = lut(0, false, 0);
        assert_eq!(l[0], NULL_COLOR);
        assert_eq!(l[1], [1, 1, 1, 255]);
        assert_eq!(l[128], [128, 128, 128, 255]);
        assert_eq!(l[255], [255, 255, 255, 255]);
    }

    #[test]
    fn base_and_reverse() {
        // base shifts the palette index: entry i shows colour (i + base) mod 256
        let l = lut(0, false, 10);
        assert_eq!(l[1], [11, 11, 11, 255]);
        assert_eq!(l[250], [4, 4, 4, 255]);
        let l = lut(0, true, 0);
        assert_eq!(l[0], NULL_COLOR);
        assert_eq!(l[1], [254, 254, 254, 255]);
        assert_eq!(l[255], [0, 0, 0, 255]);
        let l = lut(0, true, 5);
        assert_eq!(l[1], [249, 249, 249, 255]);
        let l = lut(0, false, -3);
        assert_eq!(l[1], [254, 254, 254, 255]);
        // out-of-range model → last
        assert_eq!(lut(42, false, 0), lut(9, false, 0));
        assert_eq!(lut_i32(-1, false, 0), lut(0, false, 0));
    }

    #[test]
    fn all_maps_load() {
        for m in 0..count() {
            let l = lut(m, false, 0);
            assert!(l.iter().all(|c| c[3] == 255));
        }
    }

    #[test]
    fn calc_func() {
        // interpolated grey ramp 255 -> 0
        assert_eq!(func(0, INTERPOLATE, &DOWN), 255);
        assert_eq!(func(255, INTERPOLATE, &DOWN), 0);
        assert_eq!(func(100, INTERPOLATE, &DOWN), 155);
        assert_eq!(func(100, INTERPOLATE, &UP), 100);
        // step map with 4 points: step = 85
        assert_eq!(func(84, STEP, &M4), 255);
        assert_eq!(func(85, STEP, &M4), 127);
        assert_eq!(func(255, STEP, &M4), 0);
        let maps = calculated_maps();
        assert_eq!(maps.len(), 9);
        assert_eq!(maps[8][200], [200, 200, 200]);
    }
}
