//! Synthetic PCSGU250 data for the demo scope.
//!
//! Produces raw device buffers (`DEVICE_BUFFER_LEN` `i32`) in the layout of
//! `PCSGU250.dll!ReadCh1`: `[sample_rate, full_scale, grnd_level, 4096 samples]`.
//! The samples look like the FFT display of PcLab2000LT used by StoneAge: 2048 spectrum bins,
//! each emitted twice (so `MessPunkt.spectrumIsSource` is true), 8-bit display values 0..=255.
//! The header uses the constants of the original Java demo branch of `MessPunkt.acquireData`
//! (sampleRate 16000, fullScale 255, grndLevel 0).

use super::{DEVICE_BUFFER_LEN, HEADER_LEN, SPECTRUM_VALUES};

pub(super) const DEMO_SAMPLE_RATE: i32 = 16000;
pub(super) const DEMO_FULL_SCALE: i32 = 255;
pub(super) const DEMO_GRND_LEVEL: i32 = 0;

/// Settings mirrored from the setter calls (the real device keeps them in PcLab2000LT).
#[derive(Clone, Debug, Default)]
pub(super) struct DemoSettings {
    pub run: bool,
    pub single_pending: bool,
    pub voltage1: i32,
    pub voltage2: i32,
    pub time: i32,
    pub trg_level: i32,
    pub trg_edge: i32,
    pub trg_on: bool,
    pub trg_source: i32,
    pub y_position1: i32,
    pub y_position2: i32,
    pub coupling1: i32,
    pub coupling2: i32,
}

/// Deterministic generator (same seed → same sequence of sweeps).
pub(super) struct DemoGenerator {
    seed: u64,
    frame: u64,
}

impl DemoGenerator {
    pub fn new(seed: u64) -> Self {
        Self { seed, frame: 0 }
    }

    /// One sweep: (channel 1, channel 2) raw device buffers.
    pub fn next_frames(&mut self, _settings: &DemoSettings) -> (Vec<i32>, Vec<i32>) {
        let f = self.frame;
        self.frame += 1;
        let mut rng = Lcg::new(self.seed ^ f.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let p = f as f64;

        // Channel 1: frequency-sounding spectrum. 1/f-like background falling off over the band,
        // plus a few resonance peaks that drift slowly from sweep to sweep (like moving between
        // measuring points over varying ground).
        let peaks = [
            (180.0 + 40.0 * (p * 0.37).sin(), 14.0, 38.0),
            (520.0 + 90.0 * (p * 0.21 + 1.0).sin(), 28.0, 26.0),
            (1100.0 + 150.0 * (p * 0.13 + 2.0).sin(), 45.0, 18.0),
            (1650.0 + 60.0 * (p * 0.29 + 0.5).sin(), 30.0, 10.0),
        ];
        let ch1 = frame(|k| {
            let kf = k as f64;
            let mut v = 205.0 - 58.0 * (1.0 + kf / 8.0).log10();
            for &(c, w, h) in &peaks {
                let d = (kf - c) / w;
                v += h * (-0.5 * d * d).exp();
            }
            v + rng.noise(3.0)
        });

        // Channel 2: reference channel – weaker, flatter background.
        let ch2 = frame(|k| {
            let kf = k as f64;
            150.0 - 40.0 * (1.0 + kf / 16.0).log10() + rng.noise(2.0)
        });
        (ch1, ch2)
    }
}

/// Builds a device buffer from a spectrum function of the bin index (0..2048).
fn frame(mut bin: impl FnMut(usize) -> f64) -> Vec<i32> {
    let mut buf = vec![0i32; DEVICE_BUFFER_LEN];
    buf[0] = DEMO_SAMPLE_RATE;
    buf[1] = DEMO_FULL_SCALE;
    buf[2] = DEMO_GRND_LEVEL;
    for k in 0..SPECTRUM_VALUES {
        let v = bin(k).round().clamp(0.0, 255.0) as i32;
        let i = HEADER_LEN + 2 * k;
        buf[i] = v;
        if i + 1 < DEVICE_BUFFER_LEN {
            buf[i + 1] = v;
        }
    }
    buf
}

/// Small deterministic PRNG (no external dependency).
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }

    /// Uniform noise in `[-amp, amp)`.
    fn noise(&mut self, amp: f64) -> f64 {
        (self.next_f64() * 2.0 - 1.0) * amp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_frame_layout() {
        let mut g = DemoGenerator::new(1);
        let (a, b) = g.next_frames(&DemoSettings::default());
        for buf in [&a, &b] {
            assert_eq!(buf.len(), DEVICE_BUFFER_LEN);
            assert_eq!(&buf[..3], &[16000, 255, 0]);
            assert!((0..SPECTRUM_VALUES).all(|k| buf[3 + 2 * k] == buf[4 + 2 * k]));
            assert!(buf[3..].iter().all(|v| (0..=255).contains(v)));
        }
    }
}
