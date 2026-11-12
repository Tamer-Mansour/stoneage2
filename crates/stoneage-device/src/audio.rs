//! Port of `StoneAge.AudioCapture` (javax.sound `TargetDataLine`) using cpal.
//!
//! Original behaviour (reference/javap/AudioCapture.txt):
//! * `<clinit>` calls `openLine()` once.
//! * `openLine()` builds `AudioFormat(PCM_SIGNED, 44100 Hz, 16 bit, 1 channel, frameSize 2,
//!   frameRate 44100, bigEndian = true)`, checks `AudioSystem.isLineSupported`, gets the
//!   `TargetDataLine` and opens it with `line.getBufferSize()` (default 0.5 s = 44100 bytes).
//!   Failures are only traced ("ERR: Line matching ... not supported.", "ERR: Unable to open the
//!   line: ...", "ERR: ...").
//! * `closeLine()` only calls `line.stop()`.
//! * `readData()` (local `n` ends up as 4096): `frameSize = 2`, `frames = bufferSize / 8`,
//!   reads `frames * frameSize` bytes after `line.start()`, decodes
//!   `min(4096, bytes/2)` 16-bit samples (big/little endian, `(hi << 8) | (lo & 255)`) into a
//!   `float[]` WITHOUT scaling (values are raw integer sample values -32768..32767), then
//!   `line.stop()`. 8-bit formats would give `byte` (signed) or `byte - 128` (unsigned).
//!
//! Rust design: cpal captures continuously on its own stream thread into a ring buffer;
//! [`AudioCapture::read_data`] returns the most recent [`BLOCK_SIZE`] samples, scaled to the
//! same raw 16-bit integer range as the original.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

/// `AudioFormat` sample rate (`44100.0f`).
pub const SAMPLE_RATE: f32 = 44100.0;
/// `AudioFormat` sample size in bits.
pub const SAMPLE_SIZE_BITS: u32 = 16;
/// `AudioFormat` channel count.
pub const CHANNELS: u16 = 1;
/// `AudioFormat` encoding `PCM_SIGNED`.
pub const SIGNED: bool = true;
/// `AudioFormat` `bigEndian` flag (`iconst_1` in the constructor call).
pub const BIG_ENDIAN: bool = true;
/// `frameSize = sampleSizeInBits / 8 * channels`.
pub const FRAME_SIZE: usize = (SAMPLE_SIZE_BITS as usize / 8) * CHANNELS as usize;
/// Default `TargetDataLine` buffer in bytes (Java default: half a second of audio).
pub const LINE_BUFFER_BYTES: usize = (SAMPLE_RATE as usize / 2) * FRAME_SIZE;
/// Bytes read per `readData()` call: `bufferSize / 8 * frameSize`.
pub const READ_BYTES: usize = LINE_BUFFER_BYTES / 8 * FRAME_SIZE;
/// Samples returned per `readData()` call (`min(4096, READ_BYTES / 2)` = 4096).
pub const BLOCK_SIZE: usize = 4096;

/// Ring buffer length in samples (one line buffer, 0.5 s at 44.1 kHz).
const RING_CAPACITY: usize = LINE_BUFFER_BYTES / FRAME_SIZE;
/// How long `read_data` waits for a full block before zero-padding.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Decodes PCM bytes exactly like `AudioCapture.readData()` (no normalisation: the result holds the
/// integer sample values as `f32`). At most `max_samples` samples are returned.
pub fn decode_pcm(bytes: &[u8], bits: u32, signed: bool, big_endian: bool, max_samples: usize) -> Vec<f32> {
    match bits {
        16 => {
            let n = max_samples.min(bytes.len() / 2);
            (0..n)
                .map(|i| {
                    let b0 = bytes[2 * i] as i8 as i64;
                    let b1 = bytes[2 * i + 1] as i8 as i64;
                    let v = if big_endian { (b0 << 8) | (255 & b1) } else { (b1 << 8) | (255 & b0) };
                    v as f32
                })
                .collect()
        }
        8 => {
            let n = max_samples.min(bytes.len());
            bytes[..n]
                .iter()
                .map(|&b| if signed { b as i8 as f32 } else { (b as i8 as i32 - 128) as f32 })
                .collect()
        }
        // Original returns null for any other sample size.
        _ => Vec::new(),
    }
}

/// Converts a normalised cpal sample (-1.0..1.0) to the original's raw 16-bit value range.
fn to_raw16(x: f32) -> i16 {
    (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

struct Shared {
    ring: Mutex<Ring>,
    cond: Condvar,
}

struct Ring {
    data: VecDeque<i16>,
    /// Samples pushed since the last `reset` (used to wait for fresh data after (re)start).
    fresh: usize,
}

impl Shared {
    fn push(&self, samples: impl Iterator<Item = i16>) {
        if let Ok(mut r) = self.ring.lock() {
            for s in samples {
                if r.data.len() == RING_CAPACITY {
                    r.data.pop_front();
                }
                r.data.push_back(s);
                r.fresh += 1;
            }
            drop(r);
            self.cond.notify_all();
        }
    }
}

/// Live microphone / line-in capture used by the Soundtest views.
pub struct AudioCapture {
    stream: cpal::Stream,
    shared: Arc<Shared>,
    running: bool,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl AudioCapture {
    /// `openLine()` – opens the default input device with the original AudioFormat
    /// (44.1 kHz, 16 bit, mono, signed) or the closest supported one.
    /// Returns `Err` (instead of the original's trace + null line) when there is no usable input.
    pub fn open() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "ERR: no audio input device available".to_string())?;
        let device_name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "default input".to_string());

        let supported = choose_config(&device)?;
        let sample_format = supported.sample_format();
        let mut config: cpal::StreamConfig = supported.config();
        config.buffer_size = cpal::BufferSize::Default;
        let channels = config.channels.max(1);
        let sample_rate = config.sample_rate;

        let shared = Arc::new(Shared {
            ring: Mutex::new(Ring { data: VecDeque::with_capacity(RING_CAPACITY), fresh: 0 }),
            cond: Condvar::new(),
        });

        let stream = match sample_format {
            SampleFormat::I8 => build::<i8>(&device, &config, channels, &shared),
            SampleFormat::I16 => build::<i16>(&device, &config, channels, &shared),
            SampleFormat::I32 => build::<i32>(&device, &config, channels, &shared),
            SampleFormat::U8 => build::<u8>(&device, &config, channels, &shared),
            SampleFormat::U16 => build::<u16>(&device, &config, channels, &shared),
            SampleFormat::U32 => build::<u32>(&device, &config, channels, &shared),
            SampleFormat::F32 => build::<f32>(&device, &config, channels, &shared),
            SampleFormat::F64 => build::<f64>(&device, &config, channels, &shared),
            other => Err(format!("ERR: Line matching {other} not supported.")),
        }?;
        stream
            .play()
            .map_err(|e| format!("ERR: Unable to open the line: {e}"))?;
        log::info!(
            "AudioCapture: opened '{device_name}' {sample_rate} Hz, {channels} ch, {sample_format}"
        );
        Ok(Self { stream, shared, running: true, sample_rate, channels, device_name })
    }

    /// `closeLine()` – stops capturing (`line.stop()`). A later [`read_data`](Self::read_data)
    /// restarts the line, like `readData()` calling `line.start()` in the original.
    pub fn close(&mut self) {
        if self.running {
            let _ = self.stream.pause();
            self.running = false;
        }
        if let Ok(mut r) = self.shared.ring.lock() {
            r.data.clear();
            r.fresh = 0;
        }
    }

    /// `readData()` – one block of [`BLOCK_SIZE`] samples, as raw 16-bit integer values in `f32`
    /// (like the original: no normalisation). Returns the most recent samples; waits up to 0.5 s
    /// for a full block after (re)start and zero-pads the front if the device delivers too little.
    pub fn read_data(&mut self) -> Vec<f32> {
        if !self.running {
            if let Err(e) = self.stream.play() {
                log::warn!("AudioCapture: restart failed: {e}");
            }
            self.running = true;
        }
        let deadline = Instant::now() + READ_TIMEOUT;
        let Ok(mut r) = self.shared.ring.lock() else {
            return vec![0.0; BLOCK_SIZE];
        };
        while r.fresh < BLOCK_SIZE {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.shared.cond.wait_timeout(r, deadline - now) {
                Ok((g, _)) => r = g,
                Err(_) => return vec![0.0; BLOCK_SIZE],
            }
        }
        let avail = r.data.len().min(BLOCK_SIZE);
        let mut out = vec![0.0f32; BLOCK_SIZE - avail];
        out.extend(r.data.iter().skip(r.data.len() - avail).map(|&s| s as f32));
        out
    }

    /// Sample rate of the data returned by `read_data` – always 44100 Hz like the original
    /// `AudioFormat` (device data is resampled if the device runs at another rate).
    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE as u32
    }

    /// Native sample rate the device actually runs at.
    pub fn device_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Channels delivered by the device (only the first channel is used).
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Human readable name of the input device.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// True while the stream is capturing (between open/read and close).
    pub fn is_running(&self) -> bool {
        self.running
    }
}

/// Picks 44100 Hz / fewest channels / 16-bit integer if possible, otherwise the device default.
fn choose_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    let target = SAMPLE_RATE as u32;
    let mut best: Option<(u32, cpal::SupportedStreamConfig)> = None;
    if let Ok(ranges) = device.supported_input_configs() {
        for range in ranges {
            let fmt = range.sample_format();
            let fmt_rank = match fmt {
                SampleFormat::I16 => 0,
                SampleFormat::F32 => 1,
                SampleFormat::I32 => 2,
                SampleFormat::U16 => 3,
                SampleFormat::F64 => 4,
                SampleFormat::I8 | SampleFormat::U8 | SampleFormat::U32 => 5,
                _ => continue,
            };
            let ch = range.channels() as u32;
            if let Some(cfg) = range.try_with_sample_rate(target) {
                let score = ch * 10 + fmt_rank;
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, cfg));
                }
            }
        }
    }
    match best {
        Some((_, cfg)) => Ok(cfg),
        None => device
            .default_input_config()
            .map_err(|e| format!("ERR: Line matching {target} Hz input not supported: {e}")),
    }
}

/// Streaming linear-interpolation resampler (device rate -> 44100 Hz). Identity when equal.
pub struct Resampler {
    step: f64,
    pos: f64,
    prev: f32,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self { step: from_rate.max(1) as f64 / to_rate.max(1) as f64, pos: 1.0, prev: 0.0 }
    }

    /// Feeds input samples, appends the output samples to `out`.
    pub fn process(&mut self, input: impl Iterator<Item = f32>, out: &mut Vec<f32>) {
        if (self.step - 1.0).abs() < 1e-12 {
            out.extend(input);
            return;
        }
        for x in input {
            // output samples lying between prev (pos 0) and x (pos 1)
            while self.pos <= 1.0 {
                out.push(self.prev + (x - self.prev) * self.pos as f32);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = x;
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: u16,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let sh = Arc::clone(shared);
    let ch = channels as usize;
    let mut rs = Resampler::new(config.sample_rate, SAMPLE_RATE as u32);
    let mut out = Vec::with_capacity(4096);
    device
        .build_input_stream::<T, _, _>(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                out.clear();
                rs.process(data.iter().step_by(ch).map(|&s| f32::from_sample_(s)), &mut out);
                sh.push(out.iter().map(|&x| to_raw16(x)));
            },
            |err| log::warn!("AudioCapture: stream error: {err}"),
            None,
        )
        .map_err(|e| format!("ERR: Unable to open the line: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_constants_match_original() {
        assert_eq!(FRAME_SIZE, 2);
        assert_eq!(LINE_BUFFER_BYTES, 44100);
        assert_eq!(READ_BYTES, 11024);
        assert_eq!(BLOCK_SIZE.min(READ_BYTES / 2), 4096);
    }

    #[test]
    fn decode_16_bit_big_and_little_endian() {
        let be = [0x7F, 0xFF, 0x80, 0x00, 0xFF, 0xFF, 0x01, 0x02];
        assert_eq!(decode_pcm(&be, 16, true, true, 4096), vec![32767.0, -32768.0, -1.0, 258.0]);
        let le = [0xFF, 0x7F, 0x00, 0x80, 0xFF, 0xFF, 0x02, 0x01];
        assert_eq!(decode_pcm(&le, 16, true, false, 4096), vec![32767.0, -32768.0, -1.0, 258.0]);
        assert_eq!(decode_pcm(&be, 16, true, true, 2).len(), 2);
    }

    #[test]
    fn decode_8_bit() {
        let b = [0x00, 0x7F, 0x80, 0xFF];
        assert_eq!(decode_pcm(&b, 8, true, true, 10), vec![0.0, 127.0, -128.0, -1.0]);
        // unsigned branch uses the *signed* byte minus 128 (quirk of the original)
        assert_eq!(decode_pcm(&b, 8, false, true, 10), vec![-128.0, -1.0, -256.0, -129.0]);
        assert!(decode_pcm(&b, 24, true, true, 10).is_empty());
    }

    /// Needs real hardware: `cargo test -p stoneage-device -- --ignored live_capture`.
    #[test]
    #[ignore]
    fn live_capture() {
        match AudioCapture::open() {
            Ok(mut cap) => {
                let a = cap.read_data();
                assert_eq!(a.len(), BLOCK_SIZE);
                cap.close();
                assert!(!cap.is_running());
                assert_eq!(cap.read_data().len(), BLOCK_SIZE);
                assert!(cap.is_running());
                eprintln!("{} @ {} Hz, {} ch", cap.device_name(), cap.device_sample_rate(), cap.channels());
                let max = a.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                eprintln!("block max |s| = {max}");
            }
            Err(e) => eprintln!("no input: {e}"),
        }
    }

    #[test]
    fn resampler_rates() {
        let mut out = Vec::new();
        let mut r = Resampler::new(48000, 44100);
        r.process((0..48000).map(|i| i as f32), &mut out);
        assert!((out.len() as i64 - 44100).abs() <= 1, "{}", out.len());
        // linear ramp stays linear (slope 48000/44100 per output sample)
        let d = out[1001] - out[1000];
        assert!((d - 48000.0 / 44100.0).abs() < 1e-3);
        let mut same = Vec::new();
        Resampler::new(44100, 44100).process([1.0, 2.0].into_iter(), &mut same);
        assert_eq!(same, vec![1.0, 2.0]);
    }

    #[test]
    fn raw16_scaling() {
        assert_eq!(to_raw16(0.0), 0);
        assert_eq!(to_raw16(-1.0), -32768);
        assert_eq!(to_raw16(1.0), 32767);
        assert_eq!(to_raw16(f32::from_sample_(1234i16)), 1234);
    }
}
