//! Output backend: a cpal stream rendering 32 sample channels plus
//! one music stream. The channels are deliberately dumb — volume and
//! pan arrive as absolute values from the mixer (which runs the
//! original's per-tick fade ramps itself), samples are 8-bit unsigned
//! mono PCM resampled by linear interpolation, and the music stream
//! is decoded PCM handed over whole.
//!
//! Everything crosses the realtime boundary through a lock-free-ish
//! mpsc channel; the callback never allocates or blocks (Arc drops of
//! replaced buffers are the one small exception, accepted for
//! simplicity at this scale).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

/// Number of driver sample channels in the original (remc1's
/// word_CBFF0 table).
pub const CHANNELS: usize = 32;

/// Commands into the audio callback.
pub enum Cmd {
    /// Start `pcm` on channel `ch` (replacing whatever runs there).
    Play {
        ch: usize,
        pcm: Arc<Vec<u8>>,
        sample_rate: u32,
        /// 0..=0x7FFF linear.
        vol: u16,
        /// 0..=0xFFFF, 0x7FFF center.
        pan: u16,
        looped: bool,
    },
    Stop {
        ch: usize,
    },
    SetVol {
        ch: usize,
        vol: u16,
    },
    /// Replace the music stream (interleaved i16, `channels` wide).
    /// `overlay` is the sample-aligned danger stem, mixed on top at
    /// [`Cmd::MusicOverlayGain`]'s level from the same play position.
    Music {
        pcm: Arc<Vec<i16>>,
        overlay: Option<Arc<Vec<i16>>>,
        channels: u16,
        sample_rate: u32,
        looped: bool,
    },
    /// Danger-stem gain, 0..=1 (the mixer runs the original's CC7
    /// ramp and sends the result here).
    MusicOverlayGain {
        gain: f32,
    },
    StopMusic,
    /// Master gains, 0..=1 linear.
    MasterVol {
        sfx: f32,
        music: f32,
    },
    /// Freeze the whole output (game pause: retail suspends ALL
    /// sound). Channels and music hold their positions and the
    /// device streams silence until resumed.
    Suspend {
        on: bool,
    },
}

struct Channel {
    pcm: Option<Arc<Vec<u8>>>,
    /// Fixed-point position/step, 32.32.
    pos: u64,
    step: u64,
    vol: f32,
    pan: f32,
    looped: bool,
}

struct MusicState {
    pcm: Option<Arc<Vec<i16>>>,
    overlay: Option<Arc<Vec<i16>>>,
    overlay_gain: f32,
    channels: u16,
    pos: u64,
    step: u64,
    looped: bool,
}

pub struct Renderer {
    rx: Receiver<Cmd>,
    channels: Vec<Channel>,
    music: MusicState,
    sfx_gain: f32,
    music_gain: f32,
    out_rate: f64,
    /// Game pause: stream silence, hold every play position.
    suspended: bool,
}

impl Renderer {
    pub fn new(rx: Receiver<Cmd>, out_rate: u32) -> Self {
        Renderer {
            rx,
            channels: (0..CHANNELS)
                .map(|_| Channel {
                    pcm: None,
                    pos: 0,
                    step: 0,
                    vol: 0.0,
                    pan: 0.5,
                    looped: false,
                })
                .collect(),
            music: MusicState {
                pcm: None,
                overlay: None,
                overlay_gain: 0.0,
                channels: 2,
                pos: 0,
                step: 0,
                looped: false,
            },
            sfx_gain: 1.0,
            music_gain: 1.0,
            out_rate: f64::from(out_rate),
            suspended: false,
        }
    }

    /// True while `ch` still has samples to play.
    fn channel_live(ch: &Channel) -> bool {
        ch.pcm
            .as_ref()
            .is_some_and(|p| ch.looped || (ch.pos >> 32) < p.len() as u64)
    }

    fn drain_cmds(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(cmd) => self.apply(cmd),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Play {
                ch,
                pcm,
                sample_rate,
                vol,
                pan,
                looped,
            } => {
                let c = &mut self.channels[ch];
                c.step = ((f64::from(sample_rate) / self.out_rate) * (1u64 << 32) as f64) as u64;
                c.pcm = Some(pcm);
                c.pos = 0;
                c.vol = f32::from(vol) / 32767.0;
                c.pan = f32::from(pan) / 65535.0;
                c.looped = looped;
            }
            Cmd::Stop { ch } => self.channels[ch].pcm = None,
            Cmd::SetVol { ch, vol } => self.channels[ch].vol = f32::from(vol) / 32767.0,
            Cmd::Music {
                pcm,
                overlay,
                channels,
                sample_rate,
                looped,
            } => {
                self.music.step =
                    ((f64::from(sample_rate) / self.out_rate) * (1u64 << 32) as f64) as u64;
                self.music.pcm = Some(pcm);
                self.music.overlay = overlay;
                self.music.channels = channels.max(1);
                self.music.pos = 0;
                self.music.looped = looped;
            }
            Cmd::MusicOverlayGain { gain } => self.music.overlay_gain = gain,
            Cmd::StopMusic => {
                self.music.pcm = None;
                self.music.overlay = None;
            }
            Cmd::MasterVol { sfx, music } => {
                self.sfx_gain = sfx;
                self.music_gain = music;
            }
            Cmd::Suspend { on } => self.suspended = on,
        }
    }

    /// Fill an interleaved stereo f32 buffer.
    pub fn render(&mut self, out: &mut [f32]) {
        self.drain_cmds();
        if self.suspended {
            // Game pause: silence, positions held (retail suspends
            // ALL sound and resumes where it left off).
            out.fill(0.0);
            return;
        }
        for frame in out.chunks_exact_mut(2) {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for ch in &mut self.channels {
                let Some(pcm) = ch.pcm.as_ref() else { continue };
                let len = pcm.len() as u64;
                if len == 0 {
                    continue;
                }
                let mut idx = ch.pos >> 32;
                if idx >= len {
                    if ch.looped {
                        ch.pos %= len << 32;
                        idx = ch.pos >> 32;
                    } else {
                        ch.pcm = None;
                        continue;
                    }
                }
                let frac = (ch.pos & 0xFFFF_FFFF) as f32 / 4294967296.0;
                let s0 = f32::from(pcm[idx as usize]) - 128.0;
                let s1 = f32::from(
                    pcm[if idx + 1 < len {
                        idx as usize + 1
                    } else if ch.looped {
                        0
                    } else {
                        idx as usize
                    }],
                ) - 128.0;
                let s = (s0 + (s1 - s0) * frac) / 128.0 * ch.vol * self.sfx_gain;
                l += s * (1.0 - ch.pan);
                r += s * ch.pan;
                ch.pos += ch.step;
            }
            let mut music_done = false;
            if let Some(pcm) = self.music.pcm.as_ref() {
                let chans = self.music.channels as u64;
                let frames = pcm.len() as u64 / chans;
                let mut idx = self.music.pos >> 32;
                if idx >= frames {
                    if self.music.looped && frames > 0 {
                        self.music.pos %= frames << 32;
                        idx = self.music.pos >> 32;
                    } else {
                        music_done = true;
                    }
                }
                if !music_done {
                    let at = (idx * chans) as usize;
                    let (mut ml, mut mr) = if chans >= 2 {
                        (f32::from(pcm[at]), f32::from(pcm[at + 1]))
                    } else {
                        (f32::from(pcm[at]), f32::from(pcm[at]))
                    };
                    if let Some(ov) = self.music.overlay.as_ref() {
                        if self.music.overlay_gain > 0.0 && at + 1 < ov.len().max(1) {
                            let (ol, or_) = if chans >= 2 {
                                (f32::from(ov[at]), f32::from(ov[at + 1]))
                            } else {
                                (f32::from(ov[at]), f32::from(ov[at]))
                            };
                            ml += ol * self.music.overlay_gain;
                            mr += or_ * self.music.overlay_gain;
                        }
                    }
                    l += ml / 32768.0 * self.music_gain;
                    r += mr / 32768.0 * self.music_gain;
                    self.music.pos += self.music.step;
                }
            }
            if music_done {
                self.music.pcm = None;
            }
            frame[0] = l.clamp(-1.0, 1.0);
            frame[1] = r.clamp(-1.0, 1.0);
        }
    }

    /// Channel-liveness snapshot for the mixer (best-effort; the
    /// mixer keeps its own bookkeeping and only needs "has this
    /// one-shot finished" style answers at tick granularity).
    pub fn live_mask(&self) -> u32 {
        let mut mask = 0u32;
        for (i, ch) in self.channels.iter().enumerate() {
            if Self::channel_live(ch) {
                mask |= 1 << i;
            }
        }
        mask
    }
}

/// Handle used by the game thread.
pub struct Output {
    pub tx: Sender<Cmd>,
    /// Kept alive for the stream's lifetime.
    _stream: Option<cpal::Stream>,
    live: Arc<std::sync::atomic::AtomicU32>,
}

impl Output {
    /// Open the default output device. Returns a silent stub when no
    /// device is available (headless runs must not fail).
    pub fn open() -> Output {
        use cpal::traits::{DeviceTrait, HostTrait};
        let (tx, rx) = std::sync::mpsc::channel();
        let live = Arc::new(std::sync::atomic::AtomicU32::new(0));

        let stream = (|| {
            let host = cpal::default_host();
            let device = host.default_output_device()?;
            let config = device.default_output_config().ok()?;
            let rate = config.sample_rate();
            let mut renderer = Renderer::new(rx, rate);
            let live_w = live.clone();
            let stream = device
                .build_output_stream(
                    cpal::StreamConfig {
                        channels: 2,
                        sample_rate: rate,
                        buffer_size: cpal::BufferSize::Default,
                    },
                    move |data: &mut [f32], _| {
                        renderer.render(data);
                        live_w.store(
                            renderer.live_mask(),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    },
                    |e| eprintln!("audio stream error: {e}"),
                    None,
                )
                .ok()?;
            use cpal::traits::StreamTrait;
            stream.play().ok()?;
            Some(stream)
        })();
        if stream.is_none() {
            eprintln!("note: no audio output device — sound disabled");
        }
        Output {
            tx,
            _stream: stream,
            live,
        }
    }

    pub fn live_mask(&self) -> u32 {
        self.live.load(std::sync::atomic::Ordering::Relaxed)
    }
}
