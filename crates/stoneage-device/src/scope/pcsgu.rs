//! Real PCSGU250 backend (only compiled for 32-bit Windows).
//!
//! All calls into `PCSGU250.dll` run on one dedicated worker thread:
//! * the setters of the vendor DLL busy-wait until PcLab2000LT.exe has consumed the command
//!   (`while (cmd_slot[0] != 0)`), which would hang forever if PcLab2000LT died – the UI thread
//!   must never block on that;
//! * `start()` reproduces `pcsgu250$StarterClass` (StartPCSGU250, sleep 2000 ms, RunOn(true),
//!   readCh1, isReady = true, ShowPCSGU250(guiVisible)) in the background.
//!
//! Reads wait for the worker with a timeout and return `ScopeError::Other` instead of blocking.

use std::ffi::c_char;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use libloading::Library;

use super::{dll_candidates, jni_copy_to_java, Oscilloscope, ScopeError, ScopeKind, DEVICE_BUFFER_LEN};

// PCSGU250.dll is Delphi: all exports are stdcall (`ret 4` etc.); on x86 "system" == stdcall.
type FnVoid = unsafe extern "system" fn();
type FnStart = unsafe extern "system" fn() -> u8;
type FnBool = unsafe extern "system" fn(u8);
type FnInt = unsafe extern "system" fn(i32);
type FnRead = unsafe extern "system" fn(*mut i32);
type FnDataReady = unsafe extern "system" fn() -> u8;
type FnGetSettings = unsafe extern "system" fn(*mut i32) -> u8;
type FnFindWindowA = unsafe extern "system" fn(*const c_char, *const c_char) -> isize;

/// Window title PcLab2000LT registers (`FindWindowA(NULL, "PCSGU250")` in Start/Stop_PCSGU250).
const PCLAB_WINDOW: &[u8] = b"PCSGU250\0";
/// Program `Start_PCSGU250` launches with `WinExec(.., SW_SHOWNORMAL)`.
const PCLAB_EXE: &str = "PcLab2000LT.exe";
/// `StarterClass.run`: `Thread.sleep(2000)` after `StartPCSGU250`.
const START_SETTLE: Duration = Duration::from_millis(2000);
/// Max time to wait for the PcLab2000LT window after `Start_PCSGU250`.
const PCLAB_WAIT_MAX: Duration = Duration::from_secs(15);
/// Max time a read waits for the worker thread.
const READ_TIMEOUT: Duration = Duration::from_secs(3);

struct Api {
    start: FnStart,
    stop: FnVoid,
    show: FnBool,
    run_on: FnBool,
    single_on: FnBool,
    trg_on: FnBool,
    voltage1: FnInt,
    voltage2: FnInt,
    time: FnInt,
    trg_level: FnInt,
    trg_edge: FnInt,
    trg_source: FnInt,
    y_position1: FnInt,
    y_position2: FnInt,
    coupling1: FnInt,
    coupling2: FnInt,
    read_ch1: FnRead,
    read_ch2: FnRead,
    #[allow(dead_code)]
    data_ready: Option<FnDataReady>,
    #[allow(dead_code)]
    get_settings: Option<FnGetSettings>,
    /// Keeps the function pointers above valid.
    _lib: Library,
}

unsafe fn sym<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    lib.get::<T>(name).map(|s| *s).map_err(|e| format!("missing export {name}: {e}"))
}

impl Api {
    fn load(path: &Path) -> Result<Self, String> {
        // SAFETY: loading the vendor DLL runs its DllMain, which only creates three named
        // file mappings (ShareDSOData, PCG250Data, PCS250Data); no hardware access.
        let lib = unsafe { Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
        // SAFETY: the signatures were recovered from the DLL's disassembly (see NOTES).
        unsafe {
            Ok(Self {
                start: sym(&lib, "Start_PCSGU250")?,
                stop: sym(&lib, "Stop_PCSGU250")?,
                show: sym(&lib, "Show_PCSGU250")?,
                run_on: sym(&lib, "RunOn")?,
                single_on: sym(&lib, "SingleOn")?,
                trg_on: sym(&lib, "TrgOn")?,
                voltage1: sym(&lib, "Voltage1")?,
                voltage2: sym(&lib, "Voltage2")?,
                time: sym(&lib, "Time")?,
                trg_level: sym(&lib, "TrgLevel")?,
                trg_edge: sym(&lib, "TrgEdge")?,
                trg_source: sym(&lib, "TrgSource")?,
                y_position1: sym(&lib, "YPosition1")?,
                y_position2: sym(&lib, "YPosition2")?,
                coupling1: sym(&lib, "Coupling1")?,
                coupling2: sym(&lib, "Coupling2")?,
                read_ch1: sym(&lib, "ReadCh1")?,
                read_ch2: sym(&lib, "ReadCh2")?,
                data_ready: sym(&lib, "DataReady").ok(),
                get_settings: sym(&lib, "GetSettings").ok(),
                _lib: lib,
            })
        }
    }

    fn read(&self, ch: u8) -> Vec<i32> {
        // The DLL writes exactly 0x1003 dwords; the original JNI bridge only had room for 0x1000
        // (a 12-byte stack overrun). We give it the full size and truncate in jni_copy_to_java.
        let mut buf = vec![0i32; DEVICE_BUFFER_LEN];
        let f = if ch == 1 { self.read_ch1 } else { self.read_ch2 };
        // SAFETY: buf holds DEVICE_BUFFER_LEN i32, as many as ReadCh1/2 copy.
        unsafe { f(buf.as_mut_ptr()) };
        buf
    }
}

#[derive(Clone, Copy, Debug)]
enum BoolCmd {
    Show,
    RunOn,
    SingleOn,
    TrgOn,
}

#[derive(Clone, Copy, Debug)]
enum IntCmd {
    Voltage1,
    Voltage2,
    Time,
    TrgLevel,
    TrgEdge,
    TrgSource,
    YPosition1,
    YPosition2,
    Coupling1,
    Coupling2,
}

enum Cmd {
    Start,
    Stop,
    Bool(BoolCmd, bool),
    Int(IntCmd, i32),
    Read(u8, Sender<Vec<i32>>),
    Quit,
}

pub(super) struct Pcsgu250Scope {
    tx: Sender<Cmd>,
    ready: Arc<AtomicBool>,
    gui_visible: Arc<AtomicBool>,
}

impl Pcsgu250Scope {
    pub fn open(dll_dir: Option<&Path>) -> Result<Self, ScopeError> {
        let mut errors = Vec::new();
        let mut loaded = None;
        for cand in dll_candidates(dll_dir) {
            if cand.is_absolute() && !cand.is_file() {
                continue;
            }
            match Api::load(&cand) {
                Ok(api) => {
                    loaded = Some((api, cand));
                    break;
                }
                Err(e) => errors.push(e),
            }
        }
        let Some((api, path)) = loaded else {
            return Err(ScopeError::Load(if errors.is_empty() {
                "PCSGU250.dll not found".into()
            } else {
                errors.join("; ")
            }));
        };
        log::info!("scope: loaded {}", path.display());
        let pclab_dir = path.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf);

        let ready = Arc::new(AtomicBool::new(false));
        // pcsgu250.<clinit>: isReady = false, guiVisible = true
        let gui_visible = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel();
        let (r, g) = (Arc::clone(&ready), Arc::clone(&gui_visible));
        std::thread::Builder::new()
            .name("pcsgu250".into())
            .spawn(move || worker(api, pclab_dir, rx, r, g))
            .map_err(|e| ScopeError::Other(format!("cannot start scope thread: {e}")))?;
        Ok(Self { tx, ready, gui_visible })
    }

    fn send(&self, c: Cmd) {
        if self.tx.send(c).is_err() {
            log::warn!("scope: worker thread is gone");
        }
    }

    fn read(&self, ch: u8) -> Result<Vec<i64>, ScopeError> {
        let (rtx, rrx) = mpsc::channel();
        self.tx
            .send(Cmd::Read(ch, rtx))
            .map_err(|_| ScopeError::Other("scope worker thread is gone".into()))?;
        let dev = rrx
            .recv_timeout(READ_TIMEOUT)
            .map_err(|_| ScopeError::Other("oscilloscope does not respond (PcLab2000LT busy?)".into()))?;
        Ok(jni_copy_to_java(&dev))
    }
}

impl Drop for Pcsgu250Scope {
    fn drop(&mut self) {
        // Never join: the worker may be stuck inside a DLL busy-wait.
        let _ = self.tx.send(Cmd::Quit);
    }
}

fn worker(
    api: Api,
    pclab_dir: Option<PathBuf>,
    rx: Receiver<Cmd>,
    ready: Arc<AtomicBool>,
    gui_visible: Arc<AtomicBool>,
) {
    let b = |v: bool| u8::from(v);
    while let Ok(cmd) = rx.recv() {
        // SAFETY (all calls below): signatures recovered from PCSGU250.dll, see NOTES.
        unsafe {
            match cmd {
                Cmd::Start => {
                    log::info!("scope: Oscilloscope: init() ...");
                    ensure_pclab_running(pclab_dir.as_deref());
                    let _ = (api.start)();
                    std::thread::sleep(START_SETTLE);
                    // Deviation (robustness): the DLL's setters busy-wait for PcLab2000LT to
                    // consume each command once started; without its window that loop never
                    // ends. Wait a little longer, then give up and clear the DLL's started flag.
                    let mut waited = START_SETTLE;
                    while !pclab_window_exists() && waited < PCLAB_WAIT_MAX {
                        std::thread::sleep(Duration::from_millis(250));
                        waited += Duration::from_millis(250);
                    }
                    if !pclab_window_exists() {
                        log::warn!("scope: PcLab2000LT window did not appear; scope not ready");
                        (api.stop)();
                        continue;
                    }
                    log::info!("scope: Oscilloscope: RunOn() ...");
                    (api.run_on)(1);
                    log::info!("scope: Oscilloscope: testing ...");
                    let _ = api.read(1);
                    ready.store(true, Ordering::SeqCst);
                    (api.show)(b(gui_visible.load(Ordering::SeqCst)));
                    log::info!("scope: Oscilloscope: testing ... ok!");
                }
                Cmd::Stop => (api.stop)(),
                Cmd::Bool(c, v) => {
                    let f = match c {
                        BoolCmd::Show => api.show,
                        BoolCmd::RunOn => api.run_on,
                        BoolCmd::SingleOn => api.single_on,
                        BoolCmd::TrgOn => api.trg_on,
                    };
                    f(b(v));
                }
                Cmd::Int(c, v) => {
                    let f = match c {
                        IntCmd::Voltage1 => api.voltage1,
                        IntCmd::Voltage2 => api.voltage2,
                        IntCmd::Time => api.time,
                        IntCmd::TrgLevel => api.trg_level,
                        IntCmd::TrgEdge => api.trg_edge,
                        IntCmd::TrgSource => api.trg_source,
                        IntCmd::YPosition1 => api.y_position1,
                        IntCmd::YPosition2 => api.y_position2,
                        IntCmd::Coupling1 => api.coupling1,
                        IntCmd::Coupling2 => api.coupling2,
                    };
                    f(v);
                }
                Cmd::Read(ch, reply) => {
                    let _ = reply.send(api.read(ch));
                }
                Cmd::Quit => break,
            }
        }
    }
}

/// `Start_PCSGU250` launches `PcLab2000LT.exe` via `WinExec` with a bare file name, i.e. it is
/// only found next to our exe, in the current directory or on PATH. The original ran with
/// `bin/` as working directory. To get the same result without changing the process-wide
/// current directory, we launch PcLab2000LT.exe from the DLL's directory ourselves when it is
/// not running yet and would not be found by `WinExec`.
fn ensure_pclab_running(pclab_dir: Option<&Path>) {
    if pclab_window_exists() {
        return;
    }
    let in_search_path = |d: Option<PathBuf>| d.map(|d| d.join(PCLAB_EXE).is_file()).unwrap_or(false);
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    if in_search_path(exe_dir) || in_search_path(std::env::current_dir().ok()) {
        return; // WinExec will find it
    }
    let Some(dir) = pclab_dir else { return };
    let exe = dir.join(PCLAB_EXE);
    if !exe.is_file() {
        log::warn!("scope: {} not found next to PCSGU250.dll", PCLAB_EXE);
        return;
    }
    match std::process::Command::new(&exe).current_dir(dir).spawn() {
        Ok(_) => log::info!("scope: launched {}", exe.display()),
        Err(e) => log::warn!("scope: cannot launch {}: {e}", exe.display()),
    }
}

fn pclab_window_exists() -> bool {
    // SAFETY: user32 is a system DLL; FindWindowA(NULL, "PCSGU250") has no side effects.
    unsafe {
        let Ok(user32) = Library::new("user32.dll") else { return false };
        let Ok(find) = user32.get::<FnFindWindowA>(b"FindWindowA\0") else { return false };
        find(std::ptr::null(), PCLAB_WINDOW.as_ptr().cast()) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads the original PCSGU250.dll and calls only ReadCh1/ReadCh2 (they just copy from the
    /// shared memory block; no Start_PCSGU250, no hardware). Run with `--ignored` on i686.
    #[test]
    #[ignore]
    fn load_original_dll_and_read_shared_memory() {
        let p = Path::new(super::super::ORIGINAL_INSTALL_BIN).join(super::super::DLL_NAME);
        let api = Api::load(&p).expect("load");
        let a = api.read(1);
        let b = api.read(2);
        assert_eq!(a.len(), DEVICE_BUFFER_LEN);
        assert_eq!(b.len(), DEVICE_BUFFER_LEN);
        assert_eq!(jni_copy_to_java(&a).len(), super::super::JAVA_BUFFER_LEN);
    }
}

impl Oscilloscope for Pcsgu250Scope {
    fn kind(&self) -> ScopeKind {
        ScopeKind::Pcsgu250
    }

    fn start(&mut self) -> Result<(), ScopeError> {
        self.tx
            .send(Cmd::Start)
            .map_err(|_| ScopeError::Other("scope worker thread is gone".into()))
    }

    fn stop(&mut self) {
        self.send(Cmd::Stop);
        self.ready.store(false, Ordering::SeqCst);
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    fn show_gui(&mut self, visible: bool) {
        // pcsgu250.setGuiVisible: guiVisible = v; ShowPCSGU250(guiVisible)
        self.gui_visible.store(visible, Ordering::SeqCst);
        self.send(Cmd::Bool(BoolCmd::Show, visible));
    }

    fn gui_visible(&self) -> bool {
        self.gui_visible.load(Ordering::SeqCst)
    }

    fn run_on(&mut self, on: bool) {
        self.send(Cmd::Bool(BoolCmd::RunOn, on));
    }

    fn single_on(&mut self, on: bool) {
        self.send(Cmd::Bool(BoolCmd::SingleOn, on));
    }

    fn voltage1(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::Voltage1, v));
    }

    fn voltage2(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::Voltage2, v));
    }

    fn time(&mut self, t: i32) {
        self.send(Cmd::Int(IntCmd::Time, t));
    }

    fn trg_level(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::TrgLevel, v));
    }

    fn trg_edge(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::TrgEdge, v));
    }

    fn trg_on(&mut self, on: bool) {
        self.send(Cmd::Bool(BoolCmd::TrgOn, on));
    }

    fn trg_source(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::TrgSource, v));
    }

    fn y_position1(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::YPosition1, v));
    }

    fn y_position2(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::YPosition2, v));
    }

    fn coupling1(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::Coupling1, v));
    }

    fn coupling2(&mut self, v: i32) {
        self.send(Cmd::Int(IntCmd::Coupling2, v));
    }

    fn read_ch1(&mut self) -> Result<Vec<i64>, ScopeError> {
        self.read(1)
    }

    fn read_ch2(&mut self) -> Result<Vec<i64>, ScopeError> {
        self.read(2)
    }
}
