//! Measurement data model. Port of MessPunkt (Point), MessReihe (Row), MessAreal (Survey).
//! Java static state (`MessAreal.reihen`, `activeReihe`) becomes plain fields of `Survey`.
//!
//! The Java static `MessPunkt.FFTstartIdx/FFTendIdx/FFTintervall` (one FFT interval shared by
//! every point of the running program, never saved) becomes [`Survey::fft_interval`].
//!
//! Every method documents the Java method it ports. Formulas, constants and rounding follow
//! the bytecode exactly (see `docs/NOTES-core-model.md` for the quirks kept on purpose).

use std::time::{SystemTime, UNIX_EPOCH};

/// `MessPunkt.numSamples`
pub const NUM_SAMPLES: usize = 2048;
/// Length of `MessPunkt.signal_values` (`new long[4096]`).
pub const NUM_SIGNAL_SAMPLES: usize = 4096;
/// Length of the raw buffer handed to `pcsgu250.readCh1` (`new long[5000]`).
pub const CAPTURE_BUFFER_LEN: usize = 5000;
/// `MessPunkt.FFTintervall` – minimum width (in signal samples) of the FFT interval.
pub const FFT_MIN_INTERVAL: i32 = 64;
/// `MessPunkt.log10` (`Math.log(10.0)`).
pub const LOG10: f64 = std::f64::consts::LN_10;
/// Format of `MessAreal.datum` (`SimpleDateFormat("dd.MM.yyyy, HH:mm 'Uhr'")`) for chrono.
pub const DATE_FORMAT: &str = "%d.%m.%Y, %H:%M Uhr";

/// Java `Math.round(double)` (= `floor(x + 0.5)`, NaN -> 0, saturating).
pub(crate) fn java_round(x: f64) -> i64 {
    if x.is_nan() {
        return 0;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    r as i64 // `as` saturates like Java's (long) conversion
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Current local time formatted like `MessAreal.datum`.
pub fn format_date_now() -> String {
    chrono::Local::now().format(DATE_FORMAT).to_string()
}

/// One measurement point (`MessPunkt`).
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// `acquired`: data was recorded from the device (or loaded). A point with
    /// `acquired == false` is the "live" point that the views keep re-acquiring.
    pub acquired: bool,
    /// `spectrumIsSource`: true if `values` came from the scope spectrum, false if from the signal.
    pub spectrum_is_source: bool,
    /// `values`: processed values per sample index (what the views display), length 2048.
    pub values: Vec<i64>,
    /// `signal_values`: raw time-domain signal, length 4096.
    pub signal_values: Vec<i64>,
    /// `timeStamp` (ms since epoch).
    pub time_stamp: i64,
    /// `fRange` – frequency range of this point (default 4000.0, `0.5 * sampleRate` after acquisition).
    pub f_range: f64,
    pub sample_rate: i64,
    pub full_scale: i64,
    pub grnd_level: i64,
    /// `dbMax` (40.0f) – top of the dB scale for audio spectra.
    pub db_max: f32,
    /// `dbMin` (-15.0f) – bottom of the dB scale for audio spectra.
    pub db_min: f32,
    /// `spectrum_factor = 256 / (dbMax - dbMin)`.
    pub spectrum_factor: f32,
    /// `spectrum_shift = -spectrum_factor * dbMin`.
    pub spectrum_shift: f32,
    /// `fftready`: set by `acquireData` (false only for the synthetic test signal).
    pub fft_ready: bool,
}

impl Default for Point {
    fn default() -> Self {
        Self::new()
    }
}

impl Point {
    /// `MessPunkt.<init>`
    pub fn new() -> Self {
        let db_max = 40.0f32;
        let db_min = -15.0f32;
        let spectrum_factor = 256.0f32 / (db_max - db_min);
        let spectrum_shift = -spectrum_factor * db_min;
        Self {
            acquired: false,
            spectrum_is_source: true,
            values: vec![0; NUM_SAMPLES],
            signal_values: vec![0; NUM_SIGNAL_SAMPLES],
            time_stamp: 0,
            f_range: 4000.0,
            sample_rate: 0,
            full_scale: 0,
            grnd_level: 0,
            db_max,
            db_min,
            spectrum_factor,
            spectrum_shift,
            fft_ready: false,
        }
    }

    fn raw(&self, i: i64) -> Option<i64> {
        if i < 0 {
            None
        } else {
            self.values.get(i as usize).copied()
        }
    }

    /// Fallback of `getValue` when the index is out of range (`ArrayIndexOutOfBoundsException`).
    fn oob_value(&self, idx: i64) -> i64 {
        if idx >= NUM_SAMPLES as i64 {
            self.values.get(NUM_SAMPLES - 1).copied().unwrap_or(0)
        } else {
            self.values.first().copied().unwrap_or(0)
        }
    }

    /// `getValue(int)` with `blockSize == 1` (raw value, out-of-range indices clamp like the original).
    /// Use [`Point::value_at`] / [`Survey::value`] for the block-size aware version.
    pub fn value(&self, idx: usize) -> i64 {
        self.value_at(idx.min(i32::MAX as usize) as i64, 1, false)
    }

    /// Full port of `getValue(int)`: `block_size` = `MessAreal.blockSize`,
    /// `use_block_average` = `MessAreal.useBlockAverage`.
    ///
    /// For `block_size != 1` the block starting at `round(idx / block_size) * block_size` is
    /// reduced to its average (truncating long division) or its maximum (the maximum starts at 0,
    /// as in the original). If any index of the block is out of range the whole result is
    /// `values[2047]` (idx >= 2048) or `values[0]`.
    pub fn value_at(&self, idx: i64, block_size: i32, use_block_average: bool) -> i64 {
        let bs = block_size as i64;
        if bs == 1 {
            return self.raw(idx).unwrap_or_else(|| self.oob_value(idx));
        }
        let start = (java_round(idx as f64 / bs as f64).wrapping_mul(bs)) as i32 as i64;
        let mut sum: i64 = 0;
        let mut max: i64 = 0;
        let mut j = start;
        while j < start + bs {
            match self.raw(j) {
                Some(v) => {
                    sum = sum.wrapping_add(v);
                    max = max.max(v);
                }
                None => return self.oob_value(idx),
            }
            j += 1;
        }
        if use_block_average {
            if bs == 0 {
                // Java would throw ArithmeticException here; blockSize is never 0 in practice.
                return 0;
            }
            sum / bs
        } else {
            max
        }
    }

    /// `isDataValid()` – true if `max(values) - min(values) > 10`.
    pub fn is_data_valid(&self) -> bool {
        let mut sum: i64 = 0;
        let mut min: i64 = 100_000;
        let mut max: i64 = -100_000;
        for &v in &self.values {
            sum = sum.wrapping_add(v);
            max = max.max(v);
            min = min.min(v);
        }
        log::trace!("isDataValid: min={min} max={max} sum={sum}");
        max - min > 10
    }

    /// Shared part of `acquireData` cases 0..2: copy a raw device buffer
    /// (`[sampleRate, fullScale, grndLevel, data...]`) into `signal_values` / `values`
    /// and AND `spectrumIsSource` with "each value is doubled" for the first 1948 pairs.
    fn load_buffer(&mut self, buf: &[i64]) {
        let at = |k: usize| buf.get(k).copied().unwrap_or(0);
        self.values.resize(NUM_SAMPLES, 0);
        self.signal_values.resize(NUM_SIGNAL_SAMPLES, 0);
        for i in 0..NUM_SAMPLES {
            let j = 2 * i;
            self.signal_values[j] = at(3 + j);
            self.signal_values[j + 1] = at(4 + j);
            self.values[i] = at(3 + j);
            if i < 1948 {
                self.spectrum_is_source &= at(3 + j) == at(4 + j);
            }
        }
    }

    /// `fRange = 0.5f * (float) sampleRate`
    fn f_range_from_sample_rate(&mut self) {
        self.f_range = (0.5f32 * self.sample_rate as f32) as f64;
    }

    /// Fill this point from a raw scope capture (port of `acquireData` case 2, the scope).
    ///
    /// `ch` is the buffer filled by `pcsgu250.readCh1(long[5000])`:
    /// `ch[0] = sampleRate`, `ch[1] = fullScale`, `ch[2] = grndLevel`, `ch[3..]` = data.
    /// Missing entries count as 0 (the Java array is zero-initialised).
    ///
    /// `f_range` is ignored: like the original, the point's `fRange` is always
    /// `0.5 * sampleRate` from the capture header. Does not touch `acquired`.
    pub fn set_from_capture(&mut self, ch: &[i64], f_range: f64) {
        let _ = f_range;
        self.time_stamp = now_millis();
        self.spectrum_is_source = true;
        let at = |k: usize| ch.get(k).copied().unwrap_or(0);
        self.sample_rate = at(0);
        self.full_scale = at(1);
        self.grnd_level = at(2);
        self.load_buffer(ch);
        self.f_range_from_sample_rate();
        self.fft_ready = true;
    }

    /// `acquireData` case 0 – what the original does in demo mode (`MainFrame.demoMode`):
    /// a constant buffer of 5s (so `values` = 5 except `values[2047] = 0`, sampleRate 16000,
    /// fRange 8000). Note: such data is *not* `is_data_valid()` (range 5), exactly like the original.
    pub fn acquire_demo(&mut self) {
        self.time_stamp = now_millis();
        let mut buf = vec![0i64; CAPTURE_BUFFER_LEN];
        for v in buf.iter_mut().take(2 * NUM_SAMPLES) {
            *v = 5;
        }
        self.sample_rate = 16000;
        self.full_scale = 255;
        self.grnd_level = 0;
        self.load_buffer(&buf);
        self.f_range_from_sample_rate();
        self.fft_ready = true;
    }

    /// `acquireData` case 1 (unreachable in the original, kept for completeness): a sine test
    /// signal `round(125 * (1 + sin((phase + i) / 250)))`, every value doubled.
    /// The original uses `phase = System.currentTimeMillis() % 6280`.
    pub fn acquire_test_signal(&mut self, phase: i64) {
        self.time_stamp = now_millis();
        let mut buf = vec![0i64; CAPTURE_BUFFER_LEN];
        for i in 0..NUM_SAMPLES {
            let j = 2 * i;
            let v = java_round(125.0 * (1.0 + ((phase + i as i64) as f64 / 250.0).sin()));
            buf[j] = v;
            buf[j + 1] = v;
        }
        self.sample_rate = 16000;
        self.full_scale = 255;
        self.grnd_level = 0;
        self.load_buffer(&buf);
        self.f_range_from_sample_rate();
        self.fft_ready = false;
    }

    /// `acquireData` case 3 (sound card), after the FFT: `mag` is the output of
    /// `FastFourierTransform.fftMag`. `values[i] = round(shift + factor*10 * log10(mag[i]))`,
    /// sampleRate 44100, fRange 22050. At most 2048 bins are used.
    pub fn set_from_audio_spectrum(&mut self, mag: &[f32]) {
        self.time_stamp = now_millis();
        log::trace!("pkt: spectr length = {}", mag.len());
        self.values.resize(NUM_SAMPLES, 0);
        let shift = self.spectrum_shift as f64;
        let factor = (self.spectrum_factor * 10.0f32) as f64;
        for (i, &m) in mag.iter().take(NUM_SAMPLES).enumerate() {
            self.values[i] = java_round(shift + factor * (m as f64).log10());
        }
        self.sample_rate = 44100;
        self.full_scale = 255;
        self.grnd_level = 0;
        self.f_range_from_sample_rate();
        self.fft_ready = true;
    }

    /// `acquireData` case 3 (sound card): `AudioCapture.readData()` -> `fftMag` -> values.
    /// Uses [`crate::fft::fft_mag`].
    pub fn set_from_audio_signal(&mut self, signal: &[f32]) {
        log::trace!("pkt: signal length = {}", signal.len());
        let mag = crate::fft::fft_mag(signal);
        self.set_from_audio_spectrum(&mag);
    }

    /// Deterministic synthetic data for demo/testing, *not* the original demo data (that one is
    /// [`Point::acquire_demo`], a flat line). Uses the original's sine test signal
    /// ([`Point::acquire_test_signal`]) with a phase derived from `row`/`point`;
    /// `time_stamp` is 0 and the point is marked `acquired`.
    pub fn demo(row: usize, point: usize) -> Self {
        let mut p = Self::new();
        let phase = ((row as i64).wrapping_mul(1571) + (point as i64).wrapping_mul(97)).rem_euclid(6280);
        p.acquire_test_signal(phase);
        p.time_stamp = 0;
        p.acquired = true;
        p
    }

    /// `printData(PrintWriter)` – body of a `MessPunktStart` … `MessPunktEnde` block.
    pub fn print_data(&self, out: &mut String, nl: &str) {
        use std::fmt::Write as _;
        out.push_str("# Punktdaten:");
        out.push_str(nl);
        out.push_str("MessPunktStart");
        out.push_str(nl);
        let _ = write!(out, "fRange = {}{nl}", java_double(self.f_range));
        let _ = write!(out, "sampleRate = {}{nl}", self.sample_rate);
        let _ = write!(out, "fullScale = {}{nl}", self.full_scale);
        let _ = write!(out, "grndLevel = {}{nl}", self.grnd_level);
        let _ = write!(out, "timeStamp = {}{nl}", self.time_stamp);
        let _ = write!(out, "acquired = {}{nl}", self.acquired);
        let _ = write!(out, "spectrumIsSource = {}{nl}", self.spectrum_is_source);
        out.push_str("Values = ");
        for i in 0..NUM_SAMPLES {
            let _ = write!(out, "{} ", self.values.get(i).copied().unwrap_or(0));
        }
        out.push_str(nl);
        out.push_str("SignalValues = ");
        for i in 0..NUM_SIGNAL_SAMPLES {
            let _ = write!(out, "{} ", self.signal_values.get(i).copied().unwrap_or(0));
        }
        out.push_str(nl);
        out.push_str("MessPunktEnde");
        out.push_str(nl);
    }
}

/// Java `Double.toString(double)` / `StringBuilder.append(double)`.
pub fn java_double(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = x.abs();
    if (1e-3..1e7).contains(&a) {
        let s = format!("{x}");
        if s.contains('.') {
            s
        } else {
            s + ".0"
        }
    } else {
        let s = format!("{x:e}");
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let mant = if mant.contains('.') { mant.to_string() } else { format!("{mant}.0") };
        format!("{mant}E{exp}")
    }
}

/// Which handle of the FFT interval a drag grabbed (`MessPunkt.ViewArea.DRAG_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FftDrag {
    /// `DRAG_NONE`
    None,
    /// `DRAG_START` – pressed left of the interval.
    Start,
    /// `DRAG_END` – pressed right of the interval.
    End,
    /// `DRAG_INTERVALL` – pressed inside, moves the whole interval.
    Interval,
}

/// The FFT interval of the signal plot (Java statics `MessPunkt.FFTstartIdx/FFTendIdx`).
/// Units are signal-sample indices (0..=4096). In the original it is global for all points and
/// never saved, so it lives on [`Survey`] and is not part of the `.saf` file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FftInterval {
    /// `FFTstartIdx` (initially 0)
    pub start_idx: i32,
    /// `FFTendIdx` (initially 4096)
    pub end_idx: i32,
}

impl Default for FftInterval {
    fn default() -> Self {
        Self { start_idx: 0, end_idx: NUM_SIGNAL_SAMPLES as i32 }
    }
}

impl FftInterval {
    /// `mousePressed`: classify a press at signal index `idx` (= `4 * (x - marginx)` in the original).
    pub fn hit(&self, idx: i32) -> FftDrag {
        if idx < self.start_idx {
            FftDrag::Start
        } else if idx > self.end_idx {
            FftDrag::End
        } else {
            FftDrag::Interval
        }
    }
    /// `mouseDragged` with `DRAG_START`.
    pub fn drag_start_to(&mut self, idx: i32) {
        if idx >= 0 {
            if self.end_idx - idx >= FFT_MIN_INTERVAL {
                self.start_idx = idx;
            }
        } else {
            self.start_idx = 0;
        }
    }
    /// `mouseDragged` with `DRAG_END`.
    pub fn drag_end_to(&mut self, idx: i32) {
        if idx <= NUM_SIGNAL_SAMPLES as i32 {
            if idx - self.start_idx >= FFT_MIN_INTERVAL {
                self.end_idx = idx;
            }
        } else {
            self.end_idx = NUM_SIGNAL_SAMPLES as i32;
        }
    }
    /// `mouseDragged` with `DRAG_INTERVALL`: shift by `delta` samples (`4 * dx` in the original)
    /// if the interval stays inside 0..=4096.
    pub fn shift_by(&mut self, delta: i32) {
        if delta + self.start_idx >= 0 && delta + self.end_idx <= NUM_SIGNAL_SAMPLES as i32 {
            self.start_idx += delta;
            self.end_idx += delta;
        }
    }
    /// Apply a drag of kind `kind` to signal index `idx`; for `Interval`, `idx` is the delta.
    pub fn apply_drag(&mut self, kind: FftDrag, idx: i32) {
        match kind {
            FftDrag::None => {}
            FftDrag::Start => self.drag_start_to(idx),
            FftDrag::End => self.drag_end_to(idx),
            FftDrag::Interval => self.shift_by(idx),
        }
    }
}

/// One measurement row (`MessReihe`).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// `punkte`
    pub points: Vec<Point>,
    /// `activePunkt`
    pub active_point: usize,
    /// `validBase` – first valid point index (0)
    pub valid_base: i32,
    /// `validStep` – every `validStep`-th point is valid (1)
    pub valid_step: i32,
    /// `basey`
    pub base_y: i32,
    /// `minimum` (0) – lower bound of the colour scale
    pub minimum: i64,
    /// `maximum` (255) – upper bound of the colour scale
    pub maximum: i64,
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    /// `MessReihe.<init>` – contains one fresh (live, not acquired) point.
    pub fn new() -> Self {
        let mut r = Self {
            points: Vec::new(),
            active_point: 0,
            valid_base: 0,
            valid_step: 1,
            base_y: 0,
            minimum: 0,
            maximum: 255,
        };
        r.add_point(Point::new());
        r
    }
    /// `addPoint(MessPunkt)`: if the row only holds one point that was never acquired, that
    /// point is replaced; otherwise `p` is appended. `activePunkt` is not changed.
    pub fn add_point(&mut self, p: Point) {
        if self.points.len() == 1 && !self.points[0].acquired {
            self.points.clear();
        }
        self.points.push(p);
    }
    pub fn count_points(&self) -> usize {
        self.points.len()
    }
    /// `getPunkt(int)`
    pub fn point(&self, idx: usize) -> Option<&Point> {
        self.points.get(idx)
    }
    pub fn current_point(&self) -> Option<&Point> {
        self.points.get(self.active_point)
    }
    pub fn current_point_mut(&mut self) -> Option<&mut Point> {
        self.points.get_mut(self.active_point)
    }
    /// `getLastPunkt()` – the live point while measuring.
    pub fn last_point(&self) -> Option<&Point> {
        self.points.last()
    }
    pub fn last_point_mut(&mut self) -> Option<&mut Point> {
        self.points.last_mut()
    }
    /// `nextPunkt()`
    pub fn next_point(&mut self) {
        if (self.active_point as i64) < self.points.len() as i64 - 1 {
            self.active_point += 1;
        }
    }
    /// `prevPunkt()`
    pub fn prev_point(&mut self) {
        if self.active_point > 0 {
            self.active_point -= 1;
        }
    }
    /// `findMinMax()` with `blockSize == 1`. The original uses the survey's block size; prefer
    /// [`Row::find_min_max_with`] / [`Survey::find_min_max`].
    pub fn find_min_max(&mut self) {
        self.find_min_max_with(1, false);
    }
    /// `findMinMax()`: min/max of `getValue(i)` over all points and the 2048 sample indices.
    pub fn find_min_max_with(&mut self, block_size: i32, use_block_average: bool) {
        self.minimum = 255;
        self.maximum = -255;
        for p in &self.points {
            for i in 0..NUM_SAMPLES as i64 {
                let v = p.value_at(i, block_size, use_block_average);
                if v > self.maximum {
                    self.maximum = v;
                }
                if v < self.minimum {
                    self.minimum = v;
                }
            }
        }
        log::debug!("Reihe: MinValue = {}, MaxValue = {}", self.minimum, self.maximum);
    }
    /// `resetMinMax()` – back to 0..255.
    pub fn reset_min_max(&mut self) {
        self.minimum = 0;
        self.maximum = 255;
        log::debug!("Reihe: MinValue = {}, MaxValue = {}", self.minimum, self.maximum);
    }
    /// Position in metres of point index `idx`: `getXPos` with `disp_stepx == 1`,
    /// i.e. `round(idx * point_distance * 100) / 100`.
    pub fn x_pos(&self, idx: usize, point_distance: f64) -> f64 {
        x_pos_px(idx as f64, 1, point_distance)
    }
    /// `getfStep()` – `currentPoint.fRange / 2048`.
    pub fn f_step(&self) -> f64 {
        self.current_point().map(|p| p.f_range).unwrap_or(0.0) / 2048.0
    }
    /// Is point `idx` a "valid" point according to `validBase`/`validStep`
    /// (`(idx - validBase) % validStep == 0` and `idx >= validBase`)?
    pub fn is_valid_index(&self, idx: i32) -> bool {
        let step = self.valid_step.max(1);
        idx >= self.valid_base && (idx - self.valid_base) % step == 0
    }
    /// `___getPointIndexForPixel(int)`: point index under horizontal pixel `px`, snapped to the
    /// valid points (`validBase + k*validStep`) and clamped to the valid range.
    pub fn point_index_for_pixel(&self, px: i32, disp_step_x: i32, interpolate: bool) -> i32 {
        let base = self.valid_base;
        let step = if self.valid_step == 0 { 1 } else { self.valid_step };
        let mut idx: i32;
        if interpolate {
            idx = (px as f64 / disp_step_x as f64).floor() as i32;
            while idx > 0 && (idx - base) % step != 0 {
                idx -= 1;
            }
        } else {
            let d = (px as f64 / disp_step_x as f64 - base as f64) / step as f64;
            idx = base.wrapping_add(step.wrapping_mul(java_round(d) as i32));
        }
        let last = (self.points.len() as i32 - 1 - base) / step * step + base;
        if idx < base {
            idx = base;
        } else if idx > last {
            idx = last;
        }
        idx
    }
    /// `___getValueForPixel(int x, int y)`: value at pixel (x, y) of the row view, with optional
    /// bilinear interpolation between valid points / sample blocks. `s` supplies
    /// `disp_stepx`, `disp_stepy`, `blockSize`, `useBlockAverage`, `interpolate`.
    pub fn value_for_pixel(&self, x: i32, y: i32, s: &Survey) -> i64 {
        let pidx = self.point_index_for_pixel(x, s.disp_step_x, s.interpolate);
        let mut sidx = if s.interpolate {
            (y as f64 / s.disp_step_y as f64).floor() as i32
        } else {
            java_round(y as f64 / s.disp_step_y as f64) as i32
        };
        sidx = sidx.wrapping_mul(s.block_size);
        sidx = sidx.clamp(0, NUM_SAMPLES as i32 - 1);
        let bs = s.block_size;
        let get = |pi: i32, si: i32| -> i64 {
            if pi < 0 {
                return 0;
            }
            self.points
                .get(pi as usize)
                .map(|p| p.value_at(si as i64, bs, s.use_block_average))
                .unwrap_or(0)
        };
        let mut v = get(pidx, sidx);
        if s.interpolate {
            let mut p2 = pidx + self.valid_step;
            if p2 > self.points.len() as i32 - 1 {
                p2 = pidx;
            }
            let mut s2 = sidx + bs;
            if s2 > NUM_SAMPLES as i32 - bs {
                s2 = sidx;
            }
            let dx = (p2 - pidx).wrapping_mul(s.disp_step_x);
            let v12 = get(pidx, s2);
            let v21 = get(p2, sidx);
            let v22 = get(p2, s2);
            let d1 = (v21 - v) as f64;
            let d2 = (v22 - v12) as f64;
            let (a, b) = if dx != 0 {
                let fx = (x % dx) as f64;
                (v as f64 + fx * d1 / dx as f64, v12 as f64 + fx * d2 / dx as f64)
            } else {
                (v as f64, v12 as f64)
            };
            let dy = b - a;
            let fy = if s.disp_step_y != 0 { (y % s.disp_step_y) as f64 } else { 0.0 };
            v = java_round(a + fy * dy / s.disp_step_y as f64);
        }
        v
    }

    /// `printData(PrintWriter)` – a `ReiheStart` … `ReihenEnde` block.
    pub fn print_data(&self, out: &mut String, nl: &str) {
        use std::fmt::Write as _;
        out.push_str("ReiheStart");
        out.push_str(nl);
        let _ = write!(out, "validBase = {}{nl}", self.valid_base);
        let _ = write!(out, "validStep = {}{nl}", self.valid_step);
        let _ = write!(out, "activePunkt = {}{nl}", self.active_point);
        for p in &self.points {
            p.print_data(out, nl);
        }
        out.push_str("ReihenEnde");
        out.push_str(nl);
    }
}

/// `MessReihe.getXPos(int)`: `round((px / disp_stepx) * messpunktAbstand * 100) / 100` metres.
pub fn x_pos_px(px: f64, disp_step_x: i32, point_distance: f64) -> f64 {
    let d = px / disp_step_x as f64;
    java_round(d * point_distance * 100.0) as f64 / 100.0
}

/// Placement of the background map image in the Map view (`karteRotation`, `karteScale`, `kartePosX/Y`).
#[derive(Clone, Debug, PartialEq)]
pub struct MapPlacement {
    pub rotation: f64,
    pub scale: f64,
    pub pos_x: i32,
    pub pos_y: i32,
}

impl Default for MapPlacement {
    /// `MessAreal.<init>`: rotation 0, scale 1, position (50, 25).
    fn default() -> Self {
        Self { rotation: 0.0, scale: 1.0, pos_x: 50, pos_y: 25 }
    }
}

/// Size of the default (white) map image (`MessKarte.kartenBreite/kartenHoehe`).
pub const DEFAULT_MAP_SIZE: (u32, u32) = (800, 600);

/// A whole survey / document (`MessAreal`). This is what a `.saf` file contains.
#[derive(Clone, Debug)]
pub struct Survey {
    pub name: String,
    /// `bemerkung` (lines separated by `\n`; after loading every line ends with `\n`).
    pub description: String,
    /// `datum`, formatted like the original (`dd.MM.yyyy, HH:mm 'Uhr'`).
    pub date: String,
    /// `messpunktAbstand` [m]
    pub point_distance: f64,
    /// `messreihenAbstand` [m]
    pub row_distance: f64,
    pub kb: f64,
    pub tm: f64,
    pub dfh: f64,
    pub f_range: f64,
    pub disp_step_x: i32,
    pub disp_step_y: i32,
    pub disp_step_z: i32,
    /// `horizontalschnitttiefe` – sample index of the horizontal depth cut.
    pub horizontal_cut: i32,
    pub color_model: i32,
    pub color_model_base: i32,
    pub show_grid: bool,
    pub interpolate: bool,
    /// `showTiefe` – show depth instead of frequency on the Y axis.
    pub show_depth: bool,
    pub reverse_color: bool,
    pub block_size: i32,
    pub use_block_average: bool,
    pub active_row: usize,
    pub rows: Vec<Row>,
    pub map: MapPlacement,
    /// Background map image (Map view, `MessKarte.img`). Stored in the `.saf` zip as
    /// `StoneAgeData - karte.jpg`. `None` = the original's default white 800x600 image.
    pub map_image: Option<image::RgbaImage>,
    /// `MessPunkt.FFTstartIdx/FFTendIdx` (Java statics; not saved).
    pub fft_interval: FftInterval,
}

impl Default for Survey {
    fn default() -> Self {
        Self::new(&crate::AppConfig::default())
    }
}

impl Survey {
    /// New empty survey initialised from the defaults in `StoneAge.ini`
    /// (`MessAreal.<init>` + `MessAreal.init` + the `MainFrame.<init>` overrides).
    ///
    /// Like the original, only `PointDistance`, `RowDistance`, `dfh`, `Kb` and `fRange` are taken
    /// from the ini; `disp_step*` / `colorModel*` keep the `MessAreal` constructor defaults.
    pub fn new(cfg: &crate::AppConfig) -> Self {
        let mut s = Self::bare();
        s.row_distance = cfg.row_distance;
        s.dfh = cfg.dfh;
        s.kb = cfg.kb;
        s.f_range = cfg.f_range;
        s.point_distance = cfg.point_distance;
        s
    }

    /// `MessAreal.<init>` + `MessAreal.init()` without the ini overrides
    /// (pointDist 5, rowDist 5, Kb 1.1, Tm 0.02, dfh 40, fRange 30, steps 32/3/32, blockSize 10,
    /// showTiefe true, one empty row, date = now).
    pub fn bare() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            date: format_date_now(),
            point_distance: 5.0,
            row_distance: 5.0,
            kb: 1.1,
            tm: 0.02,
            dfh: 40.0,
            f_range: 30.0,
            disp_step_x: 32,
            disp_step_y: 3,
            disp_step_z: 32,
            horizontal_cut: 0,
            color_model: 0,
            color_model_base: 0,
            show_grid: false,
            interpolate: false,
            show_depth: true,
            reverse_color: false,
            block_size: 10,
            use_block_average: false,
            active_row: 0,
            rows: vec![Row::new()],
            map: MapPlacement::default(),
            map_image: None,
            fft_interval: FftInterval::default(),
        }
    }

    /// `MessAreal.init()` – one fresh row, active row 0.
    pub fn init_rows(&mut self) {
        self.rows.clear();
        self.rows.push(Row::new());
        self.active_row = 0;
    }

    pub fn current_row(&self) -> Option<&Row> {
        self.rows.get(self.active_row)
    }
    pub fn current_row_mut(&mut self) -> Option<&mut Row> {
        self.rows.get_mut(self.active_row)
    }
    /// `getCurrentReihe().getCurrentPunkt()`
    pub fn current_point(&self) -> Option<&Point> {
        self.current_row().and_then(|r| r.current_point())
    }
    /// `getPunkt(row, point)`
    pub fn point(&self, row: usize, point: usize) -> Option<&Point> {
        self.rows.get(row).and_then(|r| r.points.get(point))
    }
    /// `nextReihe()`
    pub fn next_row(&mut self) {
        if (self.active_row as i64) < self.rows.len() as i64 - 1 {
            self.active_row += 1;
        }
        log::debug!("Areal: Reihe: {}/{}", self.active_row + 1, self.rows.len());
    }
    /// `prevReihe()`
    pub fn prev_row(&mut self) {
        if self.active_row > 0 {
            self.active_row -= 1;
        }
        log::debug!("Areal: Reihe: {}/{}", self.active_row + 1, self.rows.len());
    }
    /// `getMaxPoints()` – the largest point count of all rows.
    pub fn max_points(&self) -> usize {
        self.rows.iter().map(|r| r.points.len()).max().unwrap_or(0)
    }
    /// Menu "Insert new Row" (`MainFrame` action "Neue Reihe"): appends a new row with one live
    /// point at the END of the survey. The active row is NOT changed (original behaviour).
    pub fn insert_row(&mut self) {
        self.rows.push(Row::new());
    }
    /// `MainFrame` "Delete current row" is allowed only if more than one row exists.
    pub fn can_delete_current_row(&self) -> bool {
        self.rows.len() > 1
    }
    /// `MainFrame` "Delete current row": removes the active row if more than one row exists and
    /// clamps `active_row`. Otherwise nothing happens ("letzte Reihe kann nicht gelöscht werden!").
    pub fn delete_current_row(&mut self) {
        if self.rows.len() > 1 {
            if self.active_row < self.rows.len() {
                self.rows.remove(self.active_row);
            }
            if self.active_row >= self.rows.len() {
                self.active_row = self.rows.len() - 1;
            }
            log::info!("Aktuelle Reihe gelöscht, aktiv={}", self.active_row);
        } else {
            log::info!("letzte Reihe kann nicht gelöscht werden!");
        }
    }
    /// `MainFrame` "Delete current Point" is allowed only if the current row has more than one point.
    pub fn can_delete_current_point(&self) -> bool {
        self.current_row().map(|r| r.points.len() > 1).unwrap_or(false)
    }
    /// `MainFrame` "Delete current Point": removes the active point of the current row if the row
    /// has more than one point and clamps `active_point`; otherwise nothing happens.
    pub fn delete_current_point(&mut self) {
        let Some(r) = self.current_row_mut() else { return };
        if r.points.len() > 1 {
            if r.active_point < r.points.len() {
                r.points.remove(r.active_point);
            }
            if r.active_point >= r.points.len() {
                r.active_point = r.points.len() - 1;
            }
            log::info!("Aktueller Punkt gelöscht, aktiv={}", r.active_point);
        } else {
            log::info!("letzter Punkt kann nicht gelöscht werden!");
        }
    }
    /// `MainFrame` "Alle Reihen löschen": one fresh row, active row 0.
    pub fn delete_all_rows(&mut self) {
        self.init_rows();
        log::info!("Alle Reihen gelöscht!");
    }
    /// `OptionPanel` "new point" button (`OptionPanel$20`): if the live (last) point of the current
    /// row holds valid data, freeze it (acquire once more if not yet acquired, mark acquired),
    /// then append a new point filled by `acquire` and make it active.
    /// Returns false (nothing changed) if the data is not valid (`OptionPanel.point_data_not_valid`).
    pub fn record_point(&mut self, mut acquire: impl FnMut(&mut Point)) -> bool {
        if !self.freeze_live_point(&mut acquire) {
            return false;
        }
        let mut p = Point::new();
        acquire(&mut p);
        if let Some(r) = self.current_row_mut() {
            r.add_point(p);
            r.active_point = r.points.len() - 1;
        }
        true
    }
    /// `OptionPanel` "new row" button (`OptionPanel$21`): like [`Survey::record_point`] it freezes
    /// the live point (returns false if its data is not valid), then appends a new row and makes
    /// it the active row.
    pub fn record_row(&mut self, mut acquire: impl FnMut(&mut Point)) -> bool {
        if !self.freeze_live_point(&mut acquire) {
            return false;
        }
        self.rows.push(Row::new());
        self.active_row = self.rows.len() - 1;
        true
    }
    fn freeze_live_point(&mut self, acquire: &mut impl FnMut(&mut Point)) -> bool {
        let Some(last) = self.current_row_mut().and_then(|r| r.last_point_mut()) else {
            return false;
        };
        if !last.is_data_valid() {
            log::info!("point data not valid");
            return false;
        }
        if !last.acquired {
            acquire(last);
            last.acquired = true;
        }
        true
    }
    /// `MessPunkt.getValue(int)` with this survey's `blockSize` / `useBlockAverage`.
    pub fn value(&self, p: &Point, idx: i64) -> i64 {
        p.value_at(idx, self.block_size, self.use_block_average)
    }
    /// Menu "find Min/Max" (`getCurrentReihe().findMinMax()`) with this survey's block settings.
    pub fn find_min_max(&mut self) {
        let (bs, avg) = (self.block_size, self.use_block_average);
        if let Some(r) = self.current_row_mut() {
            r.find_min_max_with(bs, avg);
        }
    }
    /// Menu "reset Min/Max" (`getCurrentReihe().resetMinMax()`).
    pub fn reset_min_max(&mut self) {
        if let Some(r) = self.current_row_mut() {
            r.reset_min_max();
        }
    }
    /// `MessReihe.getXPos(int)` for horizontal pixel `px` of the row view (uses `disp_stepx`).
    pub fn x_pos(&self, px: i32) -> f64 {
        x_pos_px(px as f64, self.disp_step_x, self.point_distance)
    }
    /// `MessReihe.getfStep()` – frequency step per sample index (current point's `fRange / 2048`).
    pub fn f_step(&self) -> f64 {
        self.current_row().map(|r| r.f_step()).unwrap_or(0.0)
    }
    /// `MessReihe.freq(int)` – frequency label [kHz, 2 decimals] for index `i`, exactly like the
    /// original: `round(0.1 * blockSize * (i / disp_stepy) * fRange / 2048) / 100`, where
    /// `fRange` is the *current point's* `fRange`. (The row header calls it with a sample index.)
    pub fn freq(&self, i: usize) -> f64 {
        let f_range = self.current_point().map(|p| p.f_range).unwrap_or(0.0);
        self.freq_with(i as f64, f_range)
    }
    /// `freq` with an explicit point `fRange`.
    pub fn freq_with(&self, i: f64, point_f_range: f64) -> f64 {
        let d = i / self.disp_step_y as f64;
        java_round(0.1 * (self.block_size as f64 * d * point_f_range) / 2048.0) as f64 / 100.0
    }
    /// `MessReihe.tiefe(int)` – calculated depth [m] of sample index `i`:
    /// `f = 500 * fRange / 2048 * i; depth = 100 * Kb * Tm * f / (2 * dfh)`, rounded to 0.01.
    pub fn depth(&self, i: usize) -> f64 {
        self.depth_f(i as f64)
    }
    /// `tiefe` for a fractional index.
    pub fn depth_f(&self, i: f64) -> f64 {
        let f = 500.0 * self.f_range / 2048.0 * i;
        let t = 100.0 * self.kb * self.tm * f / (2.0 * self.dfh);
        java_round(t * 100.0) as f64 / 100.0
    }
    /// Copy the survey parameters that are also defaults back into the config ("Save Defaults",
    /// `MainFrame` action "Save Ini"): PointDistance, RowDistance, dfh, Kb, fRange, disp_stepx/y/z,
    /// colorModel, colorModelBase. (Language, Country, RecordDelay, showValidBase come from
    /// elsewhere.)
    pub fn store_defaults(&self, cfg: &mut crate::AppConfig) {
        cfg.point_distance = self.point_distance;
        cfg.row_distance = self.row_distance;
        cfg.dfh = self.dfh;
        cfg.kb = self.kb;
        cfg.f_range = self.f_range;
        cfg.disp_step_x = self.disp_step_x;
        cfg.disp_step_y = self.disp_step_y;
        cfg.disp_step_z = self.disp_step_z;
        cfg.color_model = self.color_model;
        cfg.color_model_base = self.color_model_base;
    }

    /// `MessAreal.printData(PrintWriter)` – from `Dateiformat = …` to `#ArealEnde`.
    pub fn print_data(&self, out: &mut String, nl: &str) {
        use std::fmt::Write as _;
        let _ = write!(out, "Dateiformat = {}{nl}", crate::FILE_FORMAT);
        out.push_str("#ArealStart");
        out.push_str(nl);
        let _ = write!(out, "name = {}{nl}", self.name);
        // StringTokenizer(bemerkung, "\n", true): print a token, skip the following one.
        let mut skip = false;
        for tok in tokenize_keep_delims(&self.description, '\n') {
            if skip {
                skip = false;
            } else {
                let _ = write!(out, "bemerkung = {tok}{nl}");
                skip = true;
            }
        }
        let _ = write!(out, "datum = {}{nl}", self.date);
        let _ = write!(out, "messpunktAbstand = {}{nl}", java_double(self.point_distance));
        let _ = write!(out, "messreihenAbstand = {}{nl}", java_double(self.row_distance));
        let _ = write!(out, "Kb = {}{nl}", java_double(self.kb));
        let _ = write!(out, "Tm = {}{nl}", java_double(self.tm));
        let _ = write!(out, "dfh = {}{nl}", java_double(self.dfh));
        let _ = write!(out, "fRange = {}{nl}", java_double(self.f_range));
        let _ = write!(out, "disp_stepx = {}{nl}", self.disp_step_x);
        let _ = write!(out, "disp_stepy = {}{nl}", self.disp_step_y);
        let _ = write!(out, "disp_stepz = {}{nl}", self.disp_step_z);
        let _ = write!(out, "blockSize = {}{nl}", self.block_size);
        let _ = write!(out, "useBlockAverage = {}{nl}", self.use_block_average);
        let _ = write!(out, "showGrid = {}{nl}", self.show_grid);
        let _ = write!(out, "interpolate = {}{nl}", self.interpolate);
        let _ = write!(out, "showTiefe = {}{nl}", self.show_depth);
        let _ = write!(out, "colorModel = {}{nl}", self.color_model);
        let _ = write!(out, "colorModelBase = {}{nl}", self.color_model_base);
        let _ = write!(out, "reverseColor = {}{nl}", self.reverse_color);
        let _ = write!(out, "activeReihe = {}{nl}", self.active_row);
        let _ = write!(out, "kartePosX = {}{nl}", self.map.pos_x);
        let _ = write!(out, "kartePosY = {}{nl}", self.map.pos_y);
        let _ = write!(out, "karteScale = {}{nl}", java_double(self.map.scale));
        let _ = write!(out, "karteRotation = {}{nl}", java_double(self.map.rotation));
        for r in &self.rows {
            r.print_data(out, nl);
        }
        out.push_str("#ArealEnde");
        out.push_str(nl);
    }
}

/// `java.util.StringTokenizer(s, delim, returnDelims = true)` for a single delimiter char.
fn tokenize_keep_delims(s: &str, delim: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c == delim {
            if i > start {
                out.push(&s[start..i]);
            }
            out.push(&s[i..i + c.len_utf8()]);
            start = i + c.len_utf8();
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> crate::AppConfig {
        // Values of assets/StoneAge.default.ini (built by hand: AppConfig::parse belongs to core-misc).
        crate::AppConfig {
            language: "en".into(),
            country: "US".into(),
            record_delay_ms: 5000,
            point_distance: 2.0,
            row_distance: 2.0,
            dfh: 400.0,
            kb: 1.1,
            f_range: 300.0,
            disp_step_x: 32,
            disp_step_y: 3,
            disp_step_z: 32,
            color_model: 0,
            color_model_base: 0,
            show_valid_base: false,
            extra: vec![],
        }
    }

    #[test]
    fn java_round_matches_java() {
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(-2.6), -3);
        assert_eq!(java_round(f64::NAN), 0);
        assert_eq!(java_round(f64::NEG_INFINITY), i64::MIN);
    }

    #[test]
    fn java_double_format() {
        assert_eq!(java_double(2.0), "2.0");
        assert_eq!(java_double(1.1), "1.1");
        assert_eq!(java_double(0.02), "0.02");
        assert_eq!(java_double(400.0), "400.0");
        assert_eq!(java_double(-0.5), "-0.5");
        assert_eq!(java_double(1.0e7), "1.0E7");
        assert_eq!(java_double(1.5e-5), "1.5E-5");
        assert_eq!(java_double(0.001), "0.001");
        assert_eq!(java_double(0.0), "0.0");
    }

    #[test]
    fn new_survey_from_default_ini() {
        let s = Survey::new(&cfg());
        assert_eq!(s.point_distance, 2.0);
        assert_eq!(s.row_distance, 2.0);
        assert_eq!(s.kb, 1.1);
        assert_eq!(s.dfh, 400.0);
        assert_eq!(s.f_range, 300.0);
        assert_eq!(s.tm, 0.02);
        assert_eq!(s.block_size, 10);
        assert!(s.show_depth);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].points.len(), 1);
        assert_eq!(s.map, MapPlacement { rotation: 0.0, scale: 1.0, pos_x: 50, pos_y: 25 });
        assert_eq!(s.date.len(), "01.01.2026, 12:00 Uhr".len());
    }

    #[test]
    fn depth_formula_default_ini() {
        let s = Survey::new(&cfg());
        // f = 500*300/2048*i ; depth = 100*1.1*0.02*f/800 = 0.00275*f
        assert_eq!(s.depth(0), 0.0);
        assert_eq!(s.depth(2048), 412.5);
        assert_eq!(s.depth(1000), 201.42); // 201.416015625
        assert_eq!(s.depth(1), 0.2); // 0.2014...
        assert_eq!(s.depth(100), 20.14); // 20.1416
    }

    #[test]
    fn freq_formula_default_ini() {
        let mut s = Survey::new(&cfg());
        // current point fRange defaults to 4000.0 (MessPunkt.<init>)
        assert_eq!(s.f_step(), 4000.0 / 2048.0);
        // round(0.1*10*(i/3)*4000/2048)/100
        assert_eq!(s.freq(0), 0.0);
        assert_eq!(s.freq(2048), 13.33); // 1333.33 -> 1333
        assert_eq!(s.freq(300), 1.95); // 195.3125 -> 195
        s.rows[0].points[0].f_range = 8000.0;
        assert_eq!(s.freq(2048), 26.67);
        assert_eq!(s.f_step(), 8000.0 / 2048.0);
        // fRange from the ini is what freq_with gets when called with the survey fRange
        assert_eq!(s.freq_with(2048.0, 300.0), 1.0); // 100.0
    }

    #[test]
    fn x_pos_formula() {
        let s = Survey::new(&cfg());
        assert_eq!(s.x_pos(64), 4.0); // 64/32 * 2.0
        assert_eq!(s.x_pos(10), 0.63); // 0.3125*2 = 0.625 -> 0.63
        assert_eq!(s.rows[0].x_pos(3, 2.0), 6.0);
    }

    #[test]
    fn point_defaults_and_value() {
        let mut p = Point::new();
        assert_eq!(p.f_range, 4000.0);
        assert!(p.spectrum_is_source);
        assert!(!p.acquired);
        assert!((p.spectrum_factor - 256.0 / 55.0).abs() < 1e-6);
        assert!((p.spectrum_shift - 256.0 / 55.0 * 15.0).abs() < 1e-4);
        for (i, v) in p.values.iter_mut().enumerate() {
            *v = i as i64;
        }
        assert_eq!(p.value(5), 5);
        assert_eq!(p.value(5000), 2047);
        assert_eq!(p.value_at(-1, 1, false), 0);
        // blockSize 10: block start round(13/10)*10 = 10 -> max of 10..20 = 19, avg = 14
        assert_eq!(p.value_at(13, 10, false), 19);
        assert_eq!(p.value_at(13, 10, true), 14);
        // block 2050..2060 is out of range; original falls back by the *requested* index:
        // 2045 < 2048 -> values[0] (quirk kept)
        assert_eq!(p.value_at(2045, 10, false), 0);
        assert_eq!(p.value_at(2055, 10, false), 2047);
        assert!(p.is_data_valid());
        assert!(!Point::new().is_data_valid());
    }

    #[test]
    fn capture_processing() {
        let mut buf = vec![0i64; CAPTURE_BUFFER_LEN];
        buf[0] = 1_000_000;
        buf[1] = 255;
        buf[2] = 7;
        for i in 0..NUM_SAMPLES {
            buf[3 + 2 * i] = (i % 200) as i64;
            buf[4 + 2 * i] = (i % 200) as i64;
        }
        let mut p = Point::new();
        p.set_from_capture(&buf, 123.0);
        assert!(p.spectrum_is_source);
        assert_eq!(p.sample_rate, 1_000_000);
        assert_eq!(p.grnd_level, 7);
        assert_eq!(p.f_range, 500_000.0);
        assert_eq!(p.values[199], 199);
        assert_eq!(p.signal_values[5], 2);
        assert!(p.fft_ready);
        // signal capture: neighbours differ -> spectrumIsSource false
        for (k, v) in buf.iter_mut().enumerate().skip(3) {
            *v = k as i64;
        }
        p.set_from_capture(&buf, 0.0);
        assert!(!p.spectrum_is_source);
        assert_eq!(p.values[1], 5);
        assert_eq!(p.signal_values[0], 3);
    }

    #[test]
    fn demo_mode_data_is_flat_like_original() {
        let mut p = Point::new();
        p.acquire_demo();
        assert_eq!(p.values[0], 5);
        assert_eq!(p.values[2046], 5);
        assert_eq!(p.values[2047], 0);
        assert_eq!(p.f_range, 8000.0);
        assert!(p.spectrum_is_source);
        assert!(!p.is_data_valid());
        let d = Point::demo(1, 2);
        assert!(d.is_data_valid());
        assert_eq!(d, Point::demo(1, 2));
        assert!(d.acquired);
    }

    #[test]
    fn audio_spectrum() {
        let mut p = Point::new();
        p.set_from_audio_spectrum(&[1.0, 10.0, 0.0]);
        let f = 256.0f32 / 55.0;
        assert_eq!(p.values[0], java_round((f * 15.0) as f64));
        assert_eq!(p.values[1], java_round((-f * -15.0f32) as f64 + (f * 10.0) as f64));
        assert_eq!(p.values[2], i64::MIN);
        assert_eq!(p.f_range, 22050.0);
    }

    #[test]
    fn row_navigation_and_add_point() {
        let mut r = Row::new();
        assert_eq!(r.points.len(), 1);
        let mut p = Point::new();
        p.time_stamp = 1;
        r.add_point(p.clone()); // replaces the unacquired placeholder
        assert_eq!(r.points.len(), 1);
        assert_eq!(r.points[0].time_stamp, 1);
        r.add_point(Point::new()); // still unacquired -> replaced again
        assert_eq!(r.points.len(), 1);
        r.points[0].acquired = true;
        r.add_point(Point::new());
        assert_eq!(r.points.len(), 2);
        r.prev_point();
        assert_eq!(r.active_point, 0);
        r.next_point();
        r.next_point();
        assert_eq!(r.active_point, 1);
        r.find_min_max();
        assert_eq!((r.minimum, r.maximum), (0, 0));
        r.reset_min_max();
        assert_eq!((r.minimum, r.maximum), (0, 255));
    }

    #[test]
    fn survey_menu_actions() {
        let mut s = Survey::new(&cfg());
        s.insert_row();
        s.insert_row();
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.active_row, 0);
        s.next_row();
        s.next_row();
        s.next_row();
        assert_eq!(s.active_row, 2);
        s.delete_current_row();
        assert_eq!((s.rows.len(), s.active_row), (2, 1));
        s.prev_row();
        s.prev_row();
        assert_eq!(s.active_row, 0);
        s.delete_current_point(); // only one point -> nothing
        assert_eq!(s.rows[0].points.len(), 1);
        s.delete_all_rows();
        assert_eq!((s.rows.len(), s.active_row), (1, 0));
        s.delete_current_row(); // last row cannot be deleted
        assert_eq!(s.rows.len(), 1);
    }

    #[test]
    fn record_point_and_row() {
        let mut s = Survey::new(&cfg());
        // live point not valid -> refused
        assert!(!s.record_point(|p| p.acquire_demo()));
        let mut n = 0;
        let mut acq = |p: &mut Point| {
            n += 1;
            *p = Point { acquired: p.acquired, ..Point::demo(0, n) };
        };
        s.rows[0].points[0] = Point { acquired: false, ..Point::demo(0, 0) };
        assert!(s.record_point(&mut acq));
        assert_eq!(s.rows[0].points.len(), 2);
        assert!(s.rows[0].points[0].acquired);
        assert!(!s.rows[0].points[1].acquired);
        assert_eq!(s.rows[0].active_point, 1);
        assert!(s.record_row(&mut acq));
        assert_eq!((s.rows.len(), s.active_row), (2, 1));
        assert!(s.rows[0].points[1].acquired);
        s.active_row = 0;
        s.rows[0].active_point = 0;
        s.delete_current_point();
        assert_eq!((s.rows[0].points.len(), s.rows[0].active_point), (1, 0));
    }

    #[test]
    fn store_defaults_copies_fields() {
        let mut s = Survey::bare();
        s.disp_step_x = 7;
        s.color_model = 3;
        let mut c = cfg();
        s.store_defaults(&mut c);
        assert_eq!((c.point_distance, c.dfh, c.f_range, c.disp_step_x, c.color_model), (5.0, 40.0, 30.0, 7, 3));
    }

    #[test]
    fn fft_interval_drag() {
        let mut f = FftInterval::default();
        assert_eq!(f.hit(-4), FftDrag::Start);
        assert_eq!(f.hit(100), FftDrag::Interval);
        f.drag_start_to(4050); // would leave < 64
        assert_eq!(f.start_idx, 0);
        f.drag_start_to(1000);
        assert_eq!(f.start_idx, 1000);
        f.drag_end_to(1050);
        assert_eq!(f.end_idx, 4096);
        f.drag_end_to(2000);
        assert_eq!(f.end_idx, 2000);
        f.shift_by(-2000);
        assert_eq!((f.start_idx, f.end_idx), (1000, 2000));
        f.shift_by(500);
        assert_eq!((f.start_idx, f.end_idx), (1500, 2500));
        f.drag_end_to(5000);
        assert_eq!(f.end_idx, 4096);
        f.drag_start_to(-8);
        assert_eq!(f.start_idx, 0);
    }

    #[test]
    fn bemerkung_tokenizer() {
        assert_eq!(tokenize_keep_delims("a\nb\n", '\n'), vec!["a", "\n", "b", "\n"]);
        assert_eq!(tokenize_keep_delims("", '\n'), Vec::<&str>::new());
        assert_eq!(tokenize_keep_delims("\n\nx", '\n'), vec!["\n", "\n", "x"]);
    }
}
