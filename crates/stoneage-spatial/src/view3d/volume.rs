//! Volume sampling and slice generation.
//!
//! * `sample_value`  – port of `Visual3d.getSampleValue(int,int,int)`.
//! * `build_volume`  – the volume loop of `Visual3d.createSceneGraph` (sample → colour → `double[x][y][z][4]`),
//!   followed by `ShapedTexture3D.findMinAndMaxValues` and the byte packing of `ShapedTexture3D.clamp`.
//! * `slice_positions` / `Volume::slice_image` – replacement for the 3-D texture + `X_PLANE/Y_PLANE/Z_PLANE`
//!   texgen of `generateGeometryX/Y/Z`: every quad of the original samples the 3-D texture on a plane; here that
//!   plane is pre-computed into a 2-D RGBA image (linear filtering along the slicing axis, clamp at the borders,
//!   like `Texture3D` with `BASE_LEVEL_LINEAR` / `CLAMP`).

use stoneage_core::{Survey, NUM_SAMPLES};

/// Initial value of `Visual3d.dimx/dimy/dimz` (the original volume is 4×4×4).
pub const ORIGINAL_DIM: usize = 4;
/// Upper bounds used to keep the volume responsive (the original fixed every dimension to 4).
pub const MAX_ROWS: usize = 96;
pub const MAX_POINTS: usize = 96;
pub const MAX_SAMPLES: usize = 128;
pub const MIN_SAMPLES: usize = 8;
/// Maximum number of voxels of the generated volume.
pub const VOXEL_BUDGET: usize = 1 << 20;
/// `getSampleValue` default / value returned when the sample does not exist (exception path).
pub const INVALID_SAMPLE: f64 = 128.0;
/// Channel 0 (alpha) written for every voxel in `createSceneGraph`.
pub const VOXEL_ALPHA: f64 = 128.0;
/// `new TransparencyAttributes(FASTEST, 0.1f)` in `ShapedTexture3D.prepareAppearance`.
pub const TRANSPARENCY: f32 = 0.1;

/// Geometry axes of the unit cube. X = points (texture S), Y = samples (texture T), Z = rows (texture R).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    pub fn from_index(i: usize) -> Axis {
        match i {
            0 => Axis::X,
            1 => Axis::Y,
            _ => Axis::Z,
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }

    /// In-plane axes (image columns, image rows) of a slice perpendicular to `self`.
    pub fn plane_axes(self) -> (Axis, Axis) {
        match self {
            Axis::X => (Axis::Z, Axis::Y),
            Axis::Y => (Axis::X, Axis::Z),
            Axis::Z => (Axis::X, Axis::Y),
        }
    }

    /// Texture coordinate of geometry coordinate `g` (0..1) along this axis. Sample index 0 (lowest frequency /
    /// shallowest depth) is at the top of the cube, i.e. the Y texture axis is flipped.
    pub fn tex_coord(self, g: f32) -> f32 {
        if self == Axis::Y {
            1.0 - g
        } else {
            g
        }
    }
}

/// `Visual3d.dimx/dimy/dimz` – note the original naming: x = rows, y = samples, z = points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl Dims {
    /// Volume size for a survey with `rows` rows and at most `max_points` points per row:
    /// one voxel per row/point (downsampled above `MAX_ROWS`/`MAX_POINTS`), as many samples as the voxel budget allows.
    pub fn for_survey(rows: usize, max_points: usize) -> Dims {
        let x = rows.clamp(1, MAX_ROWS);
        let z = max_points.clamp(1, MAX_POINTS);
        let y = (VOXEL_BUDGET / (x * z)).clamp(MIN_SAMPLES, MAX_SAMPLES);
        Dims { x, y, z }
    }

    pub fn len(&self) -> usize {
        self.x * self.y * self.z
    }

    /// Index into the flattened `[x][y][z]` array.
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (x * self.y + y) * self.z + z
    }
}

/// `Math.round(double)`.
pub fn java_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// `Math.round(count * i / (1.0 * dim))` – used for the row (x) and point (z) index in `getSampleValue`.
pub fn scaled_index(count: usize, i: usize, dim: usize) -> i64 {
    java_round((count * i) as f64 / dim as f64)
}

/// `Math.round((float)(2048 * y / (1 * dimy)))` – integer division, as in the original.
pub fn sample_index(y: usize, dimy: usize) -> usize {
    (NUM_SAMPLES * y) / dimy
}

/// Port of `Visual3d.getSampleValue(x, y, z)`; missing rows/points give `INVALID_SAMPLE` (the `catch` path).
pub fn sample_value(survey: &Survey, dims: Dims, x: usize, y: usize, z: usize) -> f64 {
    let row = usize::try_from(scaled_index(survey.rows.len(), x, dims.x))
        .ok()
        .and_then(|i| survey.rows.get(i));
    let Some(row) = row else { return INVALID_SAMPLE };
    let point = usize::try_from(scaled_index(row.points.len(), z, dims.z))
        .ok()
        .and_then(|i| row.points.get(i));
    let Some(point) = point else { return INVALID_SAMPLE };
    // `MessPunkt.getValue` honours the survey's block size / block average setting.
    survey.value(point, sample_index(y, dims.y) as i64) as f64
}

/// `(byte)(int)d` as used by `ShapedTexture3D.clamp` (d2i saturates like Rust `as`, i2b wraps).
pub fn to_byte(d: f64) -> u8 {
    (d as i32) as u8
}

/// The volume after `ShapedTexture3D.clamp`: RGBA bytes in the original `[x][y][z]` order.
pub struct Volume {
    pub dims: Dims,
    pub rgba: Vec<[u8; 4]>,
    /// `cal_min` / `cal_max` of `findMinAndMaxValues` (channel 0 = alpha, 1..3 = RGB). Unused by the original
    /// rendering; kept for fidelity and diagnostics.
    #[cfg_attr(not(test), allow(dead_code))]
    pub cal_min: [f64; 4],
    #[cfg_attr(not(test), allow(dead_code))]
    pub cal_max: [f64; 4],
}

/// `findMinAndMaxValues` including its quirky initial values (`Double.MAX_VALUE` / `Double.MIN_VALUE`).
pub fn find_min_max(data: &[[f64; 4]]) -> ([f64; 4], [f64; 4]) {
    let mut min = [f64::MAX; 4];
    let mut max = [f64::from_bits(1); 4]; // Double.MIN_VALUE = 4.9E-324
    for v in data {
        for c in 0..4 {
            if v[c] > max[c] {
                max[c] = v[c];
            }
            if v[c] < min[c] {
                min[c] = v[c];
            }
        }
    }
    (min, max)
}

/// Volume loop of `createSceneGraph` + `ShapedTexture3D` construction.
pub fn build_volume(
    dims: Dims,
    lut: &[[u8; 4]; 256],
    mut sampler: impl FnMut(usize, usize, usize) -> f64,
) -> Volume {
    let mut data = vec![[0.0f64; 4]; dims.len()];
    for x in 0..dims.x {
        for y in 0..dims.y {
            for z in 0..dims.z {
                let s = sampler(x, y, z);
                // `(int)Math.floor(s)` indexes the IndexColorModel; out-of-range indices are clamped here
                // (the original would throw and leave the scene empty).
                let ci = (s.floor() as i64).clamp(0, 255) as usize;
                let c = lut[ci];
                data[dims.idx(x, y, z)] = [VOXEL_ALPHA, c[0] as f64, c[1] as f64, c[2] as f64];
            }
        }
    }
    // double3d[0][0][0][0] = 0; double3d[0][0][1][0] = 255;
    data[dims.idx(0, 0, 0)][0] = 0.0;
    if dims.z > 1 {
        data[dims.idx(0, 0, 1)][0] = 255.0;
    }
    let (cal_min, cal_max) = find_min_max(&data);
    let rgba = data
        .iter()
        .map(|d| [to_byte(d[1]), to_byte(d[2]), to_byte(d[3]), to_byte(d[0])])
        .collect();
    Volume { dims, rgba, cal_min, cal_max }
}

/// Positions of the quads of `generateGeometryX/Y/Z(n)`: `for (f = 0; f <= 1; f += 1f/n)` in float arithmetic.
pub fn slice_positions(n: usize) -> Vec<f32> {
    let step = 1.0f32 / n.max(1) as f32;
    let mut out = Vec::new();
    let mut f = 0.0f32;
    while f <= 1.0 {
        out.push(f);
        f += step;
    }
    out
}

impl Volume {
    /// Number of voxels along a geometry axis.
    pub fn geom_len(&self, a: Axis) -> usize {
        match a {
            Axis::X => self.dims.z,
            Axis::Y => self.dims.y,
            Axis::Z => self.dims.x,
        }
    }

    /// Voxel at geometry indices `[point, sample, row]`.
    fn voxel(&self, g: [usize; 3]) -> [u8; 4] {
        self.rgba[self.dims.idx(g[2], g[1], g[0])]
    }

    /// RGBA image (unmultiplied) of the plane perpendicular to `a` at geometry coordinate `f` (0..1).
    /// Image columns run along `a.plane_axes().0`, rows along `.1`, both in voxel index order.
    pub fn slice_image(&self, a: Axis, f: f32) -> ([usize; 2], Vec<u8>) {
        let n = self.geom_len(a);
        let c = (a.tex_coord(f) * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let i0 = c.floor() as usize;
        let i1 = (i0 + 1).min(n - 1);
        let t = c - i0 as f32;
        let (ua, va) = a.plane_axes();
        let (w, h) = (self.geom_len(ua), self.geom_len(va));
        let mut out = Vec::with_capacity(w * h * 4);
        let mut g = [0usize; 3];
        for row in 0..h {
            for col in 0..w {
                g[ua.index()] = col;
                g[va.index()] = row;
                g[a.index()] = i0;
                let p0 = self.voxel(g);
                g[a.index()] = i1;
                let p1 = self.voxel(g);
                for k in 0..4 {
                    let v = p0[k] as f32 + (p1[k] as f32 - p0[k] as f32) * t;
                    out.push(v.round().clamp(0.0, 255.0) as u8);
                }
            }
        }
        ([w, h], out)
    }
}

/// Per-slice opacity factor (vertex tint, 0..1) for a stack of `slices` quads.
///
/// The original draws `n+1` quads per axis with texel alpha `128/255` and transparency `0.1`. To keep the
/// overall look independent of the slice count, the per-slice alpha is corrected so that a stack of `slices`
/// quads is as opaque as `opacity` × the original 4×4×4 stack (5 quads).
pub fn slice_tint(opacity: f32, slices: usize) -> f32 {
    let a_vox = (VOXEL_ALPHA as f32 / 255.0) * (1.0 - TRANSPARENCY);
    let reference = (ORIGINAL_DIM + 1) as f32;
    let exponent = opacity.max(0.0) * reference / slices.max(1) as f32;
    let target = 1.0 - (1.0 - a_vox).powf(exponent);
    // The texels already carry alpha 128/255; the tint scales it to `target`.
    (target / (VOXEL_ALPHA as f32 / 255.0)).clamp(0.0, 1.0)
}
