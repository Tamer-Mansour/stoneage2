//! Software camera replacing Java3D's `SimpleUniverse` + `setNominalViewingTransform`, the outer
//! `TransformGroup` (scale 0.5) and the `MouseRotate` / `MouseZoom` behaviours.

use egui::{Pos2, Vec2};

/// Scale of the outermost `TransformGroup` (`Transform3D.set(new Vector3d(0,0,0), 0.5)`).
pub const SCENE_SCALE: f32 = 0.5;
/// Java3D default field of view (45°).
pub const FOV: f32 = std::f32::consts::FRAC_PI_4;
/// Java3D default front clip distance.
pub const NEAR: f32 = 0.1;
/// Radians per pixel for left-drag orbit (`MouseRotate` uses 0.03, which is very fast on modern displays).
pub const ROTATE_FACTOR: f32 = 0.01;
pub const MIN_DIST: f32 = 0.35;
pub const MAX_DIST: f32 = 25.0;

/// Eye distance of `ViewingPlatform.setNominalViewingTransform` (`1 / tan(fov/2)`).
pub fn nominal_distance() -> f32 {
    1.0 / (FOV * 0.5).tan()
}

/// Focal length in pixels for a viewport of `size` (the smaller side spans the field of view).
pub fn focal_length(size: Vec2) -> f32 {
    0.5 * size.x.min(size.y).max(1.0) / (FOV * 0.5).tan()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f32; 3]; 3]);

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// `Transform3D.rotX`
    pub fn rot_x(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])
    }

    /// `Transform3D.rotY`
    pub fn rot_y(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])
    }

    pub fn mul(&self, o: &Mat3) -> Mat3 {
        let mut r = [[0.0f32; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        Mat3(r)
    }

    pub fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }

    pub fn transpose(&self) -> Mat3 {
        let m = &self.0;
        Mat3([[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]])
    }

    /// Gram-Schmidt on the rows, against drift from accumulated rotations.
    pub fn orthonormalize(&mut self) {
        fn norm(v: [f32; 3]) -> [f32; 3] {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            [v[0] / l, v[1] / l, v[2] / l]
        }
        fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }
        let r0 = norm(self.0[0]);
        let d = dot(self.0[1], r0);
        let r1 = norm([self.0[1][0] - d * r0[0], self.0[1][1] - d * r0[1], self.0[1][2] - d * r0[2]]);
        let r2 = [
            r0[1] * r1[2] - r0[2] * r1[1],
            r0[2] * r1[0] - r0[0] * r1[2],
            r0[0] * r1[1] - r0[1] * r1[0],
        ];
        self.0 = [r0, r1, r2];
    }
}

/// Orbit camera. Object coordinates are those inside the outer scale group (the volume cube spans -0.5..0.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Rotation of the `MouseRotate` transform group.
    pub rot: Mat3,
    /// Eye distance from the origin (looking along -z).
    pub dist: f32,
    /// Translation in camera space (world units).
    pub pan: [f32; 2],
}

impl Default for Camera {
    fn default() -> Self {
        Camera { rot: Mat3::IDENTITY, dist: nominal_distance(), pan: [0.0, 0.0] }
    }
}

impl Camera {
    /// `MouseRotate`: x-drag rotates about Y, y-drag about X, applied in world space (pre-multiplied).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let rx = Mat3::rot_x(dy * ROTATE_FACTOR);
        let ry = Mat3::rot_y(dx * ROTATE_FACTOR);
        self.rot = ry.mul(&rx.mul(&self.rot));
        self.rot.orthonormalize();
    }

    /// Multiply the eye distance by `factor` (< 1 zooms in).
    pub fn zoom(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.dist = (self.dist * factor).clamp(MIN_DIST, MAX_DIST);
        }
    }

    /// Pan so that the scene centre follows a mouse movement of `d` pixels.
    pub fn pan_pixels(&mut self, d: Vec2, focal: f32) {
        let s = self.dist / focal.max(1.0);
        self.pan[0] += d.x * s;
        self.pan[1] -= d.y * s;
    }

    /// Object → camera space (camera at the origin of xy, eye at z = `dist`).
    pub fn to_camera(&self, p: [f32; 3]) -> [f32; 3] {
        let w = self.rot.apply([p[0] * SCENE_SCALE, p[1] * SCENE_SCALE, p[2] * SCENE_SCALE]);
        [w[0] + self.pan[0], w[1] + self.pan[1], w[2]]
    }

    /// Perspective projection of an object-space point; `None` if it is in front of the near plane.
    pub fn project(&self, p: [f32; 3], center: Pos2, focal: f32) -> Option<Pos2> {
        let c = self.to_camera(p);
        let denom = self.dist - c[2];
        if denom < NEAR {
            return None;
        }
        Some(Pos2::new(center.x + focal * c[0] / denom, center.y - focal * c[1] / denom))
    }

    /// Eye position in object coordinates.
    pub fn eye_object(&self) -> [f32; 3] {
        let e = self.rot.transpose().apply([-self.pan[0], -self.pan[1], self.dist]);
        [e[0] / SCENE_SCALE, e[1] / SCENE_SCALE, e[2] / SCENE_SCALE]
    }
}

/// Index (0..3) of the coordinate axis most aligned with the direction from the volume centre to the eye.
pub fn choose_axis(eye: [f32; 3]) -> usize {
    let mut best = 0;
    for i in 1..3 {
        if eye[i].abs() > eye[best].abs() {
            best = i;
        }
    }
    best
}

/// Drawing order (indices into `positions`, object coordinates along the slicing axis): farthest first.
pub fn back_to_front(positions: &[f32], eye: f32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..positions.len()).collect();
    order.sort_by(|&a, &b| {
        let da = (positions[a] - eye).abs();
        let db = (positions[b] - eye).abs();
        db.total_cmp(&da)
    });
    order
}
