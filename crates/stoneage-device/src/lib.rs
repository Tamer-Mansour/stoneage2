//! Hardware access. OWNER: agent "device".
//!
//! * `Oscilloscope` – port of `StoneAge.pcsgu250` (JNI class) + `stoneage.dll` (JNI bridge)
//!   driving Velleman `PCSGU250.dll` directly (no Java, no JNI). PCSGU250.dll is 32-bit, so
//!   the real scope only works in the i686-pc-windows-msvc build; any other build (or a missing
//!   DLL / device) must fall back to `DemoScope` without crashing.
//! * `AudioCapture` – port of `StoneAge.AudioCapture` (javax.sound) used by the Soundtest tabs.
//!
//! CONTRACT: the public names below are used by stoneage-app while it is written in parallel.
//! Add freely; do not rename/remove. Document deviations in docs/CHANGES-device.md.

pub mod audio;
pub mod scope;

pub use audio::AudioCapture;
pub use scope::{open_scope, DemoScope, Oscilloscope, ScopeError, ScopeKind};
