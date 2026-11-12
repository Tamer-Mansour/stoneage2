//! Port of `StoneAge.pcsgu250` (JNI class), `pcsgu250$StarterClass` and the JNI bridge
//! `stoneage.dll`, talking to Velleman `PCSGU250.dll` directly.
//!
//! Recovered facts (see `docs/NOTES-device-scope.md` for the evidence):
//! * `PCSGU250.dll` exports are **stdcall** (Delphi), e.g. `ReadCh1` ends in `ret 4`.
//! * `ReadCh1/2(int *buf)` copy **4099** `i32` (`0x1003`) out of the shared memory block
//!   `ShareDSOData`: `buf[0]` = sample rate, `buf[1]` = full scale, `buf[2]` = ground level,
//!   `buf[3..4099]` = 4096 samples.
//! * The JNI bridge passes a 4096-`int` stack buffer and copies `min(len(java_array), 4096)`
//!   values into the Java `long[5000]`, sign-extending (`cdq`). Java elements `4096..5000`
//!   therefore stay `0` (the last three device samples are lost). We reproduce this exactly.
//! * `jlong` setting arguments are truncated to their low 32 bits, `jboolean` is zero-extended.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod demo;
#[cfg(all(windows, target_arch = "x86"))]
mod pcsgu;

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("PCSGU250.dll could not be loaded: {0}")]
    Load(String),
    #[error("device not ready")]
    NotReady,
    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    /// Real Velleman PCSGU250 via PCSGU250.dll.
    Pcsgu250,
    /// Synthetic data (original "demo mode").
    Demo,
}

/// Same operations as the Java `pcsgu250` class.
pub trait Oscilloscope: Send {
    fn kind(&self) -> ScopeKind;
    /// `start()` – the original starts the scope in a background thread (`StarterClass`).
    fn start(&mut self) -> Result<(), ScopeError>;
    fn stop(&mut self);
    fn is_ready(&self) -> bool;
    /// Show/hide the vendor oscilloscope window (`ShowPCSGU250`, "show oszi" checkbox).
    fn show_gui(&mut self, visible: bool);
    fn gui_visible(&self) -> bool;
    fn run_on(&mut self, on: bool);
    fn single_on(&mut self, on: bool);
    fn voltage1(&mut self, v: i32);
    fn voltage2(&mut self, v: i32);
    fn time(&mut self, t: i32);
    fn trg_level(&mut self, v: i32);
    fn trg_edge(&mut self, v: i32);
    fn trg_on(&mut self, on: bool);
    fn trg_source(&mut self, v: i32);
    fn y_position1(&mut self, v: i32);
    fn y_position2(&mut self, v: i32);
    fn coupling1(&mut self, v: i32);
    fn coupling2(&mut self, v: i32);
    /// `readCh1(long[])` – returns the buffer exactly as the Java side received it.
    fn read_ch1(&mut self) -> Result<Vec<i64>, ScopeError>;
    fn read_ch2(&mut self) -> Result<Vec<i64>, ScopeError>;
}

// ---------------------------------------------------------------------------------------------
// Buffer layout constants (shared by the real device and the demo).
// ---------------------------------------------------------------------------------------------

/// Number of `i32` values `PCSGU250.dll!ReadCh1/2` writes (`mov ecx,1003h; loop`).
pub const DEVICE_BUFFER_LEN: usize = 4099;
/// Header values in front of the samples: sample rate, full scale, ground level.
pub const HEADER_LEN: usize = 3;
/// Samples per channel delivered by the device.
pub const DEVICE_SAMPLES: usize = DEVICE_BUFFER_LEN - HEADER_LEN;
/// Size of the stack buffer in `stoneage.dll` (`0x4000` bytes) = max values copied to Java.
pub const JNI_COPY_LIMIT: usize = 4096;
/// Size of the `long[]` the Java side allocates (`sipush 5000; newarray long`).
pub const JAVA_BUFFER_LEN: usize = 5000;
/// Number of spectrum values `MessPunkt.acquireData` extracts (`values[i] = buf[3 + 2*i]`).
pub const SPECTRUM_VALUES: usize = 2048;

/// Directory of the original installation, used as the very last DLL search location.
pub const ORIGINAL_INSTALL_BIN: &str = r"C:\Users\tamer\Downloads\Stoneage\Stoneage\bin";
/// Vendor DLL file name.
pub const DLL_NAME: &str = "PCSGU250.dll";

/// Header of a `readCh1/2` buffer (`MessPunkt.sampleRate/fullScale/grndLevel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeHeader {
    pub sample_rate: i64,
    pub full_scale: i64,
    pub grnd_level: i64,
}

impl ScopeHeader {
    /// Reads `buf[0..3]` like `MessPunkt.acquireData` (case 2). Missing values read as 0.
    pub fn from_buffer(buf: &[i64]) -> Self {
        let g = |i: usize| buf.get(i).copied().unwrap_or(0);
        Self { sample_rate: g(0), full_scale: g(1), grnd_level: g(2) }
    }
}

/// Emulates `Java_StoneAge_pcsgu250_readCh1`: copy at most [`JNI_COPY_LIMIT`] device values
/// (sign-extended) into a zeroed `long[JAVA_BUFFER_LEN]`.
pub fn jni_copy_to_java(device: &[i32]) -> Vec<i64> {
    let mut out = vec![0i64; JAVA_BUFFER_LEN];
    let n = device.len().min(JNI_COPY_LIMIT).min(JAVA_BUFFER_LEN);
    for (o, &d) in out.iter_mut().zip(&device[..n]) {
        *o = i64::from(d);
    }
    out
}

/// Candidate locations for `PCSGU250.dll`, in search order:
/// `dll_dir`, the executable's directory, the plain name (Windows DLL search path / PATH),
/// and finally the original installation's `bin` directory.
pub fn dll_candidates(dll_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = dll_dir {
        v.push(d.join(DLL_NAME));
    }
    if let Some(d) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        v.push(d.join(DLL_NAME));
    }
    v.push(PathBuf::from(DLL_NAME));
    v.push(Path::new(ORIGINAL_INSTALL_BIN).join(DLL_NAME));
    v
}

/// Try to load `PCSGU250.dll` from `dll_dir` (then from the exe directory / PATH);
/// on any failure return a `DemoScope` and the reason.
pub fn open_scope(dll_dir: Option<&Path>) -> (Box<dyn Oscilloscope>, Option<ScopeError>) {
    #[cfg(all(windows, target_arch = "x86"))]
    {
        match pcsgu::Pcsgu250Scope::open(dll_dir) {
            Ok(s) => (Box::new(s), None),
            Err(e) => {
                log::warn!("scope: {e}; falling back to demo scope");
                (Box::new(DemoScope::new()), Some(e))
            }
        }
    }
    #[cfg(not(all(windows, target_arch = "x86")))]
    {
        let _ = dll_dir;
        let e = ScopeError::Load(format!(
            "PCSGU250.dll is a 32-bit Windows DLL; the real oscilloscope is only available in the \
             i686-pc-windows-msvc build (this build is {}-{})",
            std::env::consts::ARCH,
            std::env::consts::OS
        ));
        log::info!("scope: {e}; using demo scope");
        (Box::new(DemoScope::new()), Some(e))
    }
}

// ---------------------------------------------------------------------------------------------
// Demo scope
// ---------------------------------------------------------------------------------------------

/// Demo scope producing data in the same layout as the real device.
pub struct DemoScope {
    ready: Arc<AtomicBool>,
    gui_visible: bool,
    start_delay: Duration,
    settings: demo::DemoSettings,
    gen: demo::DemoGenerator,
    last_ch1: Option<Vec<i32>>,
    last_ch2: Option<Vec<i32>>,
}

impl DemoScope {
    pub fn new() -> Self {
        Self::with_start_delay(Duration::from_millis(300))
    }

    /// Like [`DemoScope::new`] but with a custom delay before `is_ready()` turns true after
    /// `start()` (the real `StarterClass` waits 2000 ms after `StartPCSGU250`).
    pub fn with_start_delay(start_delay: Duration) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            // `pcsgu250.<clinit>`: guiVisible = true
            gui_visible: true,
            start_delay,
            settings: demo::DemoSettings::default(),
            gen: demo::DemoGenerator::new(0x5EED_57A9_E250),
            last_ch1: None,
            last_ch2: None,
        }
    }

    /// Raw device buffer (4099 `i32`, before the JNI copy) for channel 1 or 2.
    fn device_frame(&mut self, ch: u8) -> Vec<i32> {
        let frozen = !self.settings.run && !self.settings.single_pending;
        let slot_has = if ch == 1 { self.last_ch1.is_some() } else { self.last_ch2.is_some() };
        if !(frozen && slot_has) {
            // Both channels are acquired together, like one scope sweep.
            let (a, b) = self.gen.next_frames(&self.settings);
            self.last_ch1 = Some(a);
            self.last_ch2 = Some(b);
            self.settings.single_pending = false;
        }
        if ch == 1 { self.last_ch1.clone() } else { self.last_ch2.clone() }.unwrap_or_default()
    }
}

impl Default for DemoScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Oscilloscope for DemoScope {
    fn kind(&self) -> ScopeKind {
        ScopeKind::Demo
    }

    fn start(&mut self) -> Result<(), ScopeError> {
        // StarterClass: StartPCSGU250, sleep, RunOn(true), readCh1, isReady = true, Show(guiVisible)
        self.settings.run = true;
        let ready = Arc::clone(&self.ready);
        let delay = self.start_delay;
        std::thread::Builder::new()
            .name("demo-scope-starter".into())
            .spawn(move || {
                std::thread::sleep(delay);
                ready.store(true, Ordering::SeqCst);
            })
            .map_err(|e| ScopeError::Other(e.to_string()))?;
        Ok(())
    }

    fn stop(&mut self) {
        self.settings.run = false;
        self.ready.store(false, Ordering::SeqCst);
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    fn show_gui(&mut self, visible: bool) {
        self.gui_visible = visible;
    }

    fn gui_visible(&self) -> bool {
        self.gui_visible
    }

    fn run_on(&mut self, on: bool) {
        self.settings.run = on;
    }

    fn single_on(&mut self, on: bool) {
        self.settings.single_pending = on;
    }

    fn voltage1(&mut self, v: i32) {
        self.settings.voltage1 = v;
    }

    fn voltage2(&mut self, v: i32) {
        self.settings.voltage2 = v;
    }

    fn time(&mut self, t: i32) {
        self.settings.time = t;
    }

    fn trg_level(&mut self, v: i32) {
        self.settings.trg_level = v;
    }

    fn trg_edge(&mut self, v: i32) {
        self.settings.trg_edge = v;
    }

    fn trg_on(&mut self, on: bool) {
        self.settings.trg_on = on;
    }

    fn trg_source(&mut self, v: i32) {
        self.settings.trg_source = v;
    }

    fn y_position1(&mut self, v: i32) {
        self.settings.y_position1 = v;
    }

    fn y_position2(&mut self, v: i32) {
        self.settings.y_position2 = v;
    }

    fn coupling1(&mut self, v: i32) {
        self.settings.coupling1 = v;
    }

    fn coupling2(&mut self, v: i32) {
        self.settings.coupling2 = v;
    }

    fn read_ch1(&mut self) -> Result<Vec<i64>, ScopeError> {
        let dev = self.device_frame(1);
        Ok(jni_copy_to_java(&dev))
    }

    fn read_ch2(&mut self) -> Result<Vec<i64>, ScopeError> {
        let dev = self.device_frame(2);
        Ok(jni_copy_to_java(&dev))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jni_copy_truncates_to_4096_and_zero_fills() {
        let dev: Vec<i32> = (0..DEVICE_BUFFER_LEN as i32).map(|i| i - 10).collect();
        let j = jni_copy_to_java(&dev);
        assert_eq!(j.len(), JAVA_BUFFER_LEN);
        assert_eq!(j[0], -10); // sign-extended (cdq)
        assert_eq!(j[4095], 4085);
        assert!(j[4096..].iter().all(|&v| v == 0));
    }

    #[test]
    fn demo_layout_matches_device() {
        let mut s = DemoScope::with_start_delay(Duration::ZERO);
        s.start().unwrap();
        let b = s.read_ch1().unwrap();
        assert_eq!(b.len(), JAVA_BUFFER_LEN);
        let h = ScopeHeader::from_buffer(&b);
        assert_eq!(h, ScopeHeader { sample_rate: 16000, full_scale: 255, grnd_level: 0 });
        // samples are 8-bit display values
        assert!(b[HEADER_LEN..JNI_COPY_LIMIT].iter().all(|&v| (0..=255).contains(&v)));
        // JNI truncation: everything from 4096 on is zero
        assert!(b[JNI_COPY_LIMIT..].iter().all(|&v| v == 0));
        // MessPunkt.spectrumIsSource: buf[3+2i] == buf[4+2i] for i < 1948
        assert!((0..1948).all(|i| b[3 + 2 * i] == b[4 + 2 * i]));
        // MessPunkt.isDataValid: max - min > 10 over the 2048 values
        let vals: Vec<i64> = (0..SPECTRUM_VALUES).map(|i| b[3 + 2 * i]).collect();
        let (mn, mx) = (vals.iter().min().unwrap(), vals.iter().max().unwrap());
        assert!(mx - mn > 10);
        // last value is lost by the JNI copy, exactly like the original
        assert_eq!(vals[2047], 0);
    }

    #[test]
    fn demo_is_deterministic_and_varies_between_sweeps() {
        let mut a = DemoScope::with_start_delay(Duration::ZERO);
        let mut b = DemoScope::with_start_delay(Duration::ZERO);
        a.start().unwrap();
        b.start().unwrap();
        let a1 = a.read_ch1().unwrap();
        let b1 = b.read_ch1().unwrap();
        assert_eq!(a1, b1);
        let a2 = a.read_ch1().unwrap();
        assert_ne!(a1, a2);
        // spectrum shape: low frequencies are stronger than high ones
        let lo: i64 = (0..64).map(|i| a1[3 + 2 * i]).sum();
        let hi: i64 = (1800..1864).map(|i| a1[3 + 2 * i]).sum();
        assert!(lo > hi);
    }

    #[test]
    fn demo_freezes_when_stopped_and_single_shot_works() {
        let mut s = DemoScope::with_start_delay(Duration::ZERO);
        s.start().unwrap();
        let _ = s.read_ch1().unwrap();
        s.run_on(false);
        let f1 = s.read_ch1().unwrap();
        let f2 = s.read_ch1().unwrap();
        assert_eq!(f1, f2);
        s.single_on(true);
        let f3 = s.read_ch1().unwrap();
        assert_ne!(f2, f3);
        assert_eq!(f3, s.read_ch1().unwrap());
    }

    #[test]
    fn demo_ready_after_start_and_gui_default_visible() {
        let mut s = DemoScope::with_start_delay(Duration::from_millis(5));
        assert!(!s.is_ready());
        assert!(s.gui_visible());
        s.start().unwrap();
        let t0 = std::time::Instant::now();
        while !s.is_ready() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(s.is_ready());
        s.stop();
        assert!(!s.is_ready());
        assert_eq!(s.kind(), ScopeKind::Demo);
    }

    #[test]
    fn ch2_has_same_layout() {
        let mut s = DemoScope::with_start_delay(Duration::ZERO);
        s.start().unwrap();
        let b = s.read_ch2().unwrap();
        assert_eq!(b.len(), JAVA_BUFFER_LEN);
        assert_eq!(ScopeHeader::from_buffer(&b).sample_rate, 16000);
        assert!(b[JNI_COPY_LIMIT..].iter().all(|&v| v == 0));
    }

    #[test]
    fn candidates_order() {
        let c = dll_candidates(Some(Path::new("X:/dir")));
        assert_eq!(c[0], Path::new("X:/dir").join(DLL_NAME));
        assert_eq!(c.last().unwrap(), &Path::new(ORIGINAL_INSTALL_BIN).join(DLL_NAME));
    }

    #[cfg(not(all(windows, target_arch = "x86")))]
    #[test]
    fn open_scope_falls_back_to_demo_on_64bit() {
        let (s, e) = open_scope(None);
        assert_eq!(s.kind(), ScopeKind::Demo);
        assert!(matches!(e, Some(ScopeError::Load(_))));
    }
}
