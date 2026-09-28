//! Real-time audio. `Engine` mixes key sounds inside the output callback with
//! no locks and no allocations. The manager thread owns the Windows output
//! stream: it follows the default device and pauses the stream when idle so
//! TubbyKeys never keeps the PC awake.

use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;

use crate::keymap::{KeyClass, KeyPos};
use crate::packs::SoundBank;

pub const MAX_VOICES: usize = 48;
const KEY_QUEUE: usize = 256;
const IDLE_PAUSE: Duration = Duration::from_secs(20);
const DEVICE_POLL: Duration = Duration::from_secs(2);
const TONE_DB: f32 = 9.0;
const PITCH_SEMITONES: f32 = 5.0;

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Relaxed))
    }
    pub fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Relaxed)
    }
}

/// Live parameters, read by the audio thread on every buffer.
pub struct Params {
    pub enabled: AtomicBool,
    pub volume: AtomicF32,
    pub tone: AtomicF32,
    pub pitch: AtomicF32,
    pub randomize: AtomicBool,
    pub spatial: AtomicBool,
    /// Stereo width after headphone narrowing, 0..1.
    pub width: AtomicF32,
    /// Left/right balance, -1 (left only) .. 1 (right only).
    pub balance: AtomicF32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            volume: AtomicF32::new(0.7),
            tone: AtomicF32::new(0.0),
            pitch: AtomicF32::new(0.0),
            randomize: AtomicBool::new(true),
            spatial: AtomicBool::new(true),
            width: AtomicF32::new(0.8),
            balance: AtomicF32::new(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub pos: KeyPos,
    pub down: bool,
}

pub enum Command {
    /// Replace the active pack.
    SetBank(Arc<SoundBank>),
    /// Play one press sound from a pack without switching to it.
    Preview(Arc<SoundBank>),
}

#[derive(Clone, Copy, Default)]
struct Voice {
    live: bool,
    slot: u8,
    clip: u16,
    pos: f64,
    step: f64,
    gain_l: f32,
    gain_r: f32,
    born: u64,
}

/// The mixer. Lives inside the output callback.
pub struct Engine {
    keys: rtrb::Consumer<KeyEvent>,
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Arc<SoundBank>>,
    params: Arc<Params>,
    /// Slot 0 is the active pack, slot 1 the pack being previewed.
    banks: [Option<Arc<SoundBank>>; 2],
    voices: [Voice; MAX_VOICES],
    out_rate: f32,
    rng: u32,
    clock: u64,
    tone: Tone,
    active: Arc<AtomicBool>,
}

impl Engine {
    fn new(
        keys: rtrb::Consumer<KeyEvent>,
        commands: rtrb::Consumer<Command>,
        garbage: rtrb::Producer<Arc<SoundBank>>,
        params: Arc<Params>,
        active: Arc<AtomicBool>,
    ) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(1);
        Self {
            keys,
            commands,
            garbage,
            params,
            banks: [None, None],
            voices: [Voice::default(); MAX_VOICES],
            out_rate: 48_000.0,
            rng: seed | 1,
            clock: 0,
            tone: Tone::default(),
            active,
        }
    }

    fn set_output_rate(&mut self, rate: u32) {
        if rate as f32 != self.out_rate {
            self.out_rate = rate as f32;
            self.voices.iter_mut().for_each(|v| v.live = false);
        }
    }

    fn has_pending(&self) -> bool {
        self.keys.slots() > 0 || self.commands.slots() > 0
    }

    /// Fill one interleaved output buffer. `out` arrives zeroed.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        self.drain_commands();
        let enabled = self.params.enabled.load(Relaxed);
        while let Ok(event) = self.keys.pop() {
            if enabled {
                self.trigger_key(event);
            }
        }
        if !self.voices.iter().any(|v| v.live) || channels == 0 {
            self.tone.reset();
            return;
        }
        self.active.store(true, Relaxed);
        let volume = self.params.volume.get().clamp(0.0, 1.0).powi(2);
        // Balance only turns the far side down, so the middle keeps full level.
        let balance = self.params.balance.get().clamp(-1.0, 1.0);
        let gain_l = volume * (1.0 - balance.max(0.0));
        let gain_r = volume * (1.0 + balance.min(0.0));
        self.tone.update(self.params.tone.get(), self.out_rate);
        for frame in out.chunks_exact_mut(channels) {
            let (mut left, mut right) = (0.0f32, 0.0f32);
            for voice in self.voices.iter_mut().filter(|v| v.live) {
                let Some(bank) = &self.banks[voice.slot as usize] else {
                    voice.live = false;
                    continue;
                };
                let data = &bank.clips[voice.clip as usize].samples;
                if voice.pos >= data.len() as f64 {
                    voice.live = false;
                    continue;
                }
                let sample = cubic(data, voice.pos);
                left += sample * voice.gain_l;
                right += sample * voice.gain_r;
                voice.pos += voice.step;
            }
            let (left, right) = self.tone.process(left, right);
            let (left, right) = (soft_clip(left * gain_l), soft_clip(right * gain_r));
            if channels == 1 {
                frame[0] = 0.5 * (left + right);
            } else {
                frame[0] = left;
                frame[1] = right;
            }
        }
    }

    fn drain_commands(&mut self) {
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::SetBank(bank) => self.swap_bank(0, bank),
                Command::Preview(bank) => {
                    let same = matches!(&self.banks[1], Some(b) if Arc::ptr_eq(b, &bank));
                    if !same {
                        self.swap_bank(1, bank);
                    }
                    // When `same`, the incoming clone is dropped here. The slot still
                    // holds the bank, so this only decrements a counter.
                    let roll = self.next_rand();
                    let picked = self.banks[1]
                        .as_ref()
                        .and_then(|b| b.press.pick(KeyClass::Generic, 2, roll));
                    if let Some((clip, rate)) = picked {
                        let rate = rate * pitch_rate(self.params.pitch.get());
                        self.start_voice(1, clip, rate, 0.0, 1.0);
                    }
                }
            }
        }
    }

    fn swap_bank(&mut self, slot: usize, bank: Arc<SoundBank>) {
        for voice in self.voices.iter_mut().filter(|v| v.slot as usize == slot) {
            voice.live = false;
        }
        if let Some(old) = self.banks[slot].replace(bank) {
            // Freeing a bank allocates nothing but still takes time, so hand it
            // back to the manager thread. If that queue is somehow full, free it here.
            let _ = self.garbage.push(old);
        }
    }

    fn trigger_key(&mut self, event: KeyEvent) {
        let roll = self.next_rand();
        let picked = self.banks[0].as_ref().and_then(|bank| {
            let set = if event.down {
                &bank.press
            } else {
                &bank.release
            };
            set.pick(event.pos.class, event.pos.row, roll)
        });
        let Some((clip, rate)) = picked else { return };
        let pan = if self.params.spatial.load(Relaxed) {
            event.pos.pan * self.params.width.get().clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut rate = rate * pitch_rate(self.params.pitch.get());
        let mut gain = 1.0;
        if self.params.randomize.load(Relaxed) {
            rate *= 1.0 + (self.rand_unit() * 2.0 - 1.0) * 0.035;
            gain *= 1.0 + (self.rand_unit() * 2.0 - 1.0) * 0.08;
        }
        self.start_voice(0, clip, rate, pan, gain);
    }

    fn start_voice(&mut self, slot: u8, clip: u16, rate: f32, pan: f32, gain: f32) {
        let Some(clip_rate) = self.banks[slot as usize]
            .as_ref()
            .and_then(|b| b.clips.get(clip as usize))
            .map(|c| c.rate)
        else {
            return;
        };
        let index = match self.voices.iter().position(|v| !v.live) {
            Some(free) => free,
            None => (0..MAX_VOICES)
                .min_by_key(|&i| self.voices[i].born)
                .unwrap_or(0),
        };
        self.clock += 1;
        let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
        self.voices[index] = Voice {
            live: true,
            slot,
            clip,
            pos: 0.0,
            step: rate as f64 * clip_rate as f64 / self.out_rate as f64,
            gain_l: angle.cos() * SQRT_2 * gain,
            gain_r: angle.sin() * SQRT_2 * gain,
            born: self.clock,
        };
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn rand_unit(&mut self) -> f32 {
        (self.next_rand() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Map the pitch control (-1..1) to a playback rate, +-5 semitones.
fn pitch_rate(pitch: f32) -> f32 {
    (pitch.clamp(-1.0, 1.0) * PITCH_SEMITONES / 12.0).exp2()
}

/// Catmull-Rom interpolation. Handles pitch shifting and sample-rate conversion.
fn cubic(data: &[f32], pos: f64) -> f32 {
    let i = pos as usize;
    let t = (pos - i as f64) as f32;
    let at = |k: isize| -> f32 {
        let j = i as isize + k;
        if j < 0 {
            0.0
        } else {
            data.get(j as usize).copied().unwrap_or(0.0)
        }
    };
    let (y0, y1, y2, y3) = (at(-1), at(0), at(1), at(2));
    let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c = -0.5 * y0 + 0.5 * y2;
    ((a * t + b) * t + c) * t + y1
}

/// Transparent below 0.75, then rounds off smoothly towards 1.0.
fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.75;
    let magnitude = x.abs();
    if magnitude <= KNEE {
        x
    } else {
        x.signum() * (KNEE + (1.0 - KNEE) * ((magnitude - KNEE) / (1.0 - KNEE)).tanh())
    }
}

/// Stereo biquad, transposed direct form II.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Biquad {
    /// RBJ cookbook shelving filter with slope 1.
    fn shelf(high: bool, freq: f32, gain_db: f32, rate: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / SQRT_2;
        let k = 2.0 * a.sqrt() * alpha;
        let sign = if high { -1.0 } else { 1.0 };
        let b0 = a * ((a + 1.0) - sign * (a - 1.0) * cos + k);
        let b1 = sign * 2.0 * a * ((a - 1.0) - sign * (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) - sign * (a - 1.0) * cos - k);
        let a0 = (a + 1.0) + sign * (a - 1.0) * cos + k;
        let a1 = -sign * 2.0 * ((a - 1.0) + sign * (a + 1.0) * cos);
        let a2 = (a + 1.0) + sign * (a - 1.0) * cos - k;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    fn run(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = [0.0; 2];
        self.z2 = [0.0; 2];
    }
}

/// Tilt EQ: negative values boost lows and cut highs (thock), positive values
/// do the opposite (clack).
struct Tone {
    value: f32,
    rate: f32,
    low: Biquad,
    high: Biquad,
}

impl Default for Tone {
    fn default() -> Self {
        Self {
            value: 0.0,
            rate: 0.0,
            low: Biquad::default(),
            high: Biquad::default(),
        }
    }
}

impl Tone {
    fn update(&mut self, value: f32, rate: f32) {
        let value = value.clamp(-1.0, 1.0);
        if value == self.value && rate == self.rate {
            return;
        }
        let (z_low, z_high) = ((self.low.z1, self.low.z2), (self.high.z1, self.high.z2));
        self.value = value;
        self.rate = rate;
        self.low = Biquad::shelf(false, 400.0, -value * TONE_DB, rate);
        self.high = Biquad::shelf(true, 2500.0, value * TONE_DB, rate);
        (self.low.z1, self.low.z2) = z_low;
        (self.high.z1, self.high.z2) = z_high;
    }

    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.value.abs() < 0.01 {
            return (left, right);
        }
        let left = self.high.run(0, self.low.run(0, left));
        let right = self.high.run(1, self.low.run(1, right));
        (left, right)
    }

    fn reset(&mut self) {
        self.low.reset();
        self.high.reset();
    }
}

/// Current output device, reported to the UI.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputStatus {
    pub device: Option<String>,
    pub headphones: bool,
}

enum Signal {
    Wake,
}

/// Handle the rest of the app uses to talk to the audio thread.
pub struct AudioHandle {
    pub params: Arc<Params>,
    commands: Mutex<rtrb::Producer<Command>>,
    signal: mpsc::Sender<Signal>,
    paused: Arc<AtomicBool>,
}

impl AudioHandle {
    pub fn send(&self, command: Command) {
        if let Ok(mut producer) = self.commands.lock() {
            let _ = producer.push(command);
        }
        self.wake();
    }

    /// Resume the output stream if it was paused for being idle.
    pub fn wake(&self) {
        if self.paused.load(Relaxed) {
            let _ = self.signal.send(Signal::Wake);
        }
    }

    /// A cheap waker for the keyboard hook thread.
    pub fn waker(&self) -> Waker {
        Waker {
            signal: self.signal.clone(),
            paused: Arc::clone(&self.paused),
        }
    }
}

#[derive(Clone)]
pub struct Waker {
    signal: mpsc::Sender<Signal>,
    paused: Arc<AtomicBool>,
}

impl Waker {
    pub fn wake(&self) {
        if self.paused.load(Relaxed) {
            let _ = self.signal.send(Signal::Wake);
        }
    }
}

/// Start the audio thread. Returns the key-event producer for the keyboard
/// hook and the handle for everything else.
pub fn start(
    params: Arc<Params>,
    on_status: impl Fn(OutputStatus) + Send + 'static,
) -> (rtrb::Producer<KeyEvent>, AudioHandle) {
    let (key_tx, key_rx) = rtrb::RingBuffer::new(KEY_QUEUE);
    let (command_tx, command_rx) = rtrb::RingBuffer::new(64);
    let (garbage_tx, garbage_rx) = rtrb::RingBuffer::new(64);
    let (signal_tx, signal_rx) = mpsc::channel();
    let active = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(true));
    let engine = Engine::new(
        key_rx,
        command_rx,
        garbage_tx,
        Arc::clone(&params),
        Arc::clone(&active),
    );
    let manager = Manager {
        engine: Arc::new(Mutex::new(engine)),
        garbage: garbage_rx,
        signals: signal_rx,
        paused: Arc::clone(&paused),
        active,
    };
    std::thread::Builder::new()
        .name("tubbykeys-audio".into())
        .spawn(move || manager.run(on_status))
        .expect("failed to start the audio thread");
    let handle = AudioHandle {
        params,
        commands: Mutex::new(command_tx),
        signal: signal_tx,
        paused,
    };
    (key_tx, handle)
}

struct Manager {
    engine: Arc<Mutex<Engine>>,
    garbage: rtrb::Consumer<Arc<SoundBank>>,
    signals: mpsc::Receiver<Signal>,
    paused: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
}

impl Manager {
    fn run(mut self, on_status: impl Fn(OutputStatus)) {
        crate::system::init_com();
        let host = cpal::default_host();
        let failed = Arc::new(AtomicBool::new(false));
        let mut stream: Option<cpal::Stream> = None;
        let mut device: Option<cpal::Device> = None;
        let mut device_id: Option<cpal::DeviceId> = None;
        let mut last_error: Option<String> = None;
        let mut status = OutputStatus::default();
        let mut last_poll: Option<Instant> = None;
        let mut idle_since = Instant::now();
        loop {
            let woken = match self.signals.recv_timeout(Duration::from_millis(250)) {
                Ok(Signal::Wake) => true,
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            while self.garbage.pop().is_ok() {}

            if failed.swap(false, Relaxed) {
                stream = None;
                device_id = None;
            }
            if last_poll.is_none_or(|t| t.elapsed() >= DEVICE_POLL) || (woken && stream.is_none()) {
                last_poll = Some(Instant::now());
                let id = host.default_output_device().and_then(|d| d.id().ok());
                if stream.is_none() || id != device_id {
                    let was_running = !self.paused.load(Relaxed);
                    drop(stream.take()); // release the old device first
                    self.paused.store(true, Relaxed);
                    // Open the concrete endpoint, not cpal's virtual default device:
                    // re-activating the virtual device fails once the endpoint's format
                    // changes (a Bluetooth headset switching profiles, for example).
                    // This loop already follows default-device changes itself.
                    device = id.as_ref().and_then(|id| host.device_by_id(id));
                    device_id = id;
                    stream = match device.as_ref().map(|d| self.build_stream(d, &failed)) {
                        Some(Ok(s)) => {
                            last_error = None;
                            Some(s)
                        }
                        Some(Err(e)) => {
                            if last_error.as_ref() != Some(&e) {
                                eprintln!("audio output unavailable: {e}");
                                last_error = Some(e);
                            }
                            None
                        }
                        None => None,
                    };
                    if was_running {
                        self.resume(&stream);
                    }
                }
                let fresh = OutputStatus {
                    device: device.as_ref().map(|d| d.to_string()),
                    headphones: device.is_some() && crate::system::default_output_is_headphones(),
                };
                if fresh != status {
                    status = fresh;
                    on_status(status.clone());
                }
            }

            if woken {
                idle_since = Instant::now();
                self.resume(&stream);
            } else if !self.paused.load(Relaxed) {
                if self.active.swap(false, Relaxed) {
                    idle_since = Instant::now();
                } else if idle_since.elapsed() >= IDLE_PAUSE {
                    if let Some(s) = &stream {
                        let _ = s.pause();
                    }
                    self.paused.store(true, Relaxed);
                    // A key may have slipped in between the idle check and the flag.
                    if self.engine.lock().map(|e| e.has_pending()).unwrap_or(false) {
                        self.resume(&stream);
                    }
                }
            }
        }
    }

    fn resume(&self, stream: &Option<cpal::Stream>) {
        if let Some(s) = stream {
            if s.play().is_ok() {
                self.paused.store(false, Relaxed);
            }
        }
    }

    fn build_stream(
        &self,
        device: &cpal::Device,
        failed: &Arc<AtomicBool>,
    ) -> Result<cpal::Stream, String> {
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let channels = supported.channels() as usize;
        let config = cpal::StreamConfig {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        if let Ok(mut engine) = self.engine.lock() {
            engine.set_output_rate(supported.sample_rate());
        }
        let engine = Arc::clone(&self.engine);
        let failed = Arc::clone(failed);
        device
            .build_output_stream::<f32, _, _>(
                config,
                move |data: &mut [f32], _| {
                    // Uncontended: the manager only locks while no stream is running.
                    if let Ok(mut engine) = engine.try_lock() {
                        engine.process(data, channels);
                    }
                },
                move |err| {
                    if !matches!(
                        err.kind(),
                        cpal::ErrorKind::Xrun | cpal::ErrorKind::RealtimeDenied
                    ) {
                        eprintln!("audio stream error: {err}");
                        failed.store(true, Relaxed);
                    }
                },
                None,
            )
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap;
    use crate::packs::{ClassMap, Clip};

    fn bank(id: &str) -> Arc<SoundBank> {
        let burst: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.3).sin() * 0.5).collect();
        let clip = || Clip {
            samples: burst.clone().into_boxed_slice(),
            rate: 48_000,
        };
        Arc::new(SoundBank {
            id: id.into(),
            clips: vec![clip(), clip()],
            press: ClassMap {
                default: vec![0],
                ..Default::default()
            },
            release: ClassMap {
                default: vec![1],
                ..Default::default()
            },
        })
    }

    struct Rig {
        engine: Engine,
        keys: rtrb::Producer<KeyEvent>,
        commands: rtrb::Producer<Command>,
        garbage: rtrb::Consumer<Arc<SoundBank>>,
        params: Arc<Params>,
    }

    fn rig() -> Rig {
        let (keys, key_rx) = rtrb::RingBuffer::new(KEY_QUEUE);
        let (commands, command_rx) = rtrb::RingBuffer::new(8);
        let (garbage_tx, garbage) = rtrb::RingBuffer::new(8);
        let params = Arc::new(Params::default());
        params.randomize.store(false, Relaxed);
        let mut engine = Engine::new(
            key_rx,
            command_rx,
            garbage_tx,
            Arc::clone(&params),
            Arc::new(AtomicBool::new(false)),
        );
        engine.set_output_rate(48_000);
        Rig {
            engine,
            keys,
            commands,
            garbage,
            params,
        }
    }

    fn key(scan: u16, down: bool) -> KeyEvent {
        KeyEvent {
            pos: keymap::lookup(scan),
            down,
        }
    }

    fn render(engine: &mut Engine, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        engine.process(&mut out, 2);
        out
    }

    fn energy(buf: &[f32], channel: usize) -> f32 {
        buf.iter().skip(channel).step_by(2).map(|s| s * s).sum()
    }

    #[test]
    fn a_key_press_makes_sound() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.keys.push(key(0x1E, true)).unwrap();
        let out = render(&mut r.engine, 512);
        assert!(energy(&out, 0) > 0.1 && energy(&out, 1) > 0.1);
    }

    #[test]
    fn spatial_audio_pans_left_keys_left() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.params.width.set(1.0);
        r.keys.push(key(0x1E, true)).unwrap(); // A, left half
        let out = render(&mut r.engine, 512);
        assert!(energy(&out, 0) > energy(&out, 1) * 1.5);
    }

    #[test]
    fn balance_turns_down_the_far_side() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.params.spatial.store(false, Relaxed);
        r.params.balance.set(1.0);
        r.keys.push(key(0x39, true)).unwrap();
        let out = render(&mut r.engine, 512);
        assert_eq!(energy(&out, 0), 0.0);
        assert!(energy(&out, 1) > 0.1);

        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.params.spatial.store(false, Relaxed);
        r.params.balance.set(-0.5);
        r.keys.push(key(0x39, true)).unwrap();
        let out = render(&mut r.engine, 512);
        let (left, right) = (energy(&out, 0), energy(&out, 1));
        assert!(
            right > 0.0 && right < left * 0.5,
            "left {left}, right {right}"
        );
    }

    #[test]
    fn disabled_engine_is_silent() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.params.enabled.store(false, Relaxed);
        r.keys.push(key(0x1E, true)).unwrap();
        assert_eq!(energy(&render(&mut r.engine, 512), 0), 0.0);
    }

    #[test]
    fn preview_plays_even_when_disabled() {
        let mut r = rig();
        r.params.enabled.store(false, Relaxed);
        r.commands.push(Command::Preview(bank("b"))).ok();
        assert!(energy(&render(&mut r.engine, 512), 0) > 0.1);
    }

    #[test]
    fn output_stays_bounded_under_a_key_storm() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        r.params.volume.set(1.0);
        for _ in 0..3 {
            for i in 0..80u16 {
                let _ = r.keys.push(key(0x10 + (i % 12), i % 2 == 0));
            }
            let out = render(&mut r.engine, 1024);
            assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        }
        assert!(r.engine.voices.iter().filter(|v| v.live).count() <= MAX_VOICES);
    }

    #[test]
    fn replaced_banks_are_freed_off_the_audio_thread() {
        let mut r = rig();
        r.commands.push(Command::SetBank(bank("a"))).ok();
        render(&mut r.engine, 64);
        r.commands.push(Command::SetBank(bank("b"))).ok();
        render(&mut r.engine, 64);
        let old = r.garbage.pop().expect("old bank returned");
        assert_eq!(old.id, "a");
    }

    #[test]
    fn tone_extremes_stay_stable() {
        for tone in [-1.0, 1.0] {
            let mut r = rig();
            r.commands.push(Command::SetBank(bank("a"))).ok();
            r.params.tone.set(tone);
            r.keys.push(key(0x39, true)).unwrap();
            let out = render(&mut r.engine, 4096);
            assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(energy(&out, 0) > 0.01);
        }
    }

    #[test]
    fn pitch_maps_to_five_semitones() {
        assert!((pitch_rate(0.0) - 1.0).abs() < 1e-6);
        assert!((pitch_rate(1.0) - 2f32.powf(5.0 / 12.0)).abs() < 1e-4);
        assert!(pitch_rate(-1.0) < 1.0);
    }
}
