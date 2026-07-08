//! Audio runtime for mgcarpet.
//!
//! Three layers, matching the authenticity-matrix seam:
//! - [`output`]: the dumb device backend (cpal stream, 32 sample
//!   channels + music). Knows nothing about game rules.
//! - [`mixer`]: mixing POLICY. [`mixer::FaithfulMixer`] is the ported
//!   MC1 ruleset (per-id request slots, loudest-wins, tile-driven
//!   ambient loops, per-tick fades). An enhanced distance-weighted
//!   emitter mixer lands beside it later, feeding the same backend.
//! - [`music`]: FLAC track decoding for bundle music members.
//!
//! [`Audio`] bundles the pieces for the app: open device, load an
//! audio bundle, forward sim sound events, tick the mixer at sim
//! rate, start/stop music.

pub mod mixer;
pub mod music;
pub mod output;

use std::path::Path;

pub use mixer::{FaithfulMixer, Listener, Sounds, Source};
use mgc_formats::bundle::AudioBundle;

pub struct Audio {
    out: output::Output,
    pub mixer: FaithfulMixer,
    sounds: Option<Sounds>,
    bundle: Option<AudioBundle>,
    music_playing: Option<String>,
    /// Danger-music state: the original fades the danger layers
    /// (MIDI channels 3/4/5 of the playing song) in and out with CC7
    /// ramps of step 2 over 0..126 at rate 0x3C in / 0x14 out (remc1
    /// sub_20BD0/sub_20D00 — ~1.05 s up, ~3.15 s down). We run the
    /// same ramp at sim-tick granularity over the baked danger stem.
    danger: bool,
    danger_level: f32, // 0..126, the original's fade counter
}

impl Audio {
    /// Open the output device (silent stub when none) with no bundle
    /// loaded yet.
    pub fn open() -> Audio {
        Audio {
            out: output::Output::open(),
            mixer: FaithfulMixer::new(),
            sounds: None,
            bundle: None,
            music_playing: None,
            danger: false,
            danger_level: 0.0,
        }
    }

    /// The danger-mode wish for this tick (the original's wizard
    /// `v_46 > 0` state — armed by taking hits or being targeted).
    pub fn set_danger(&mut self, danger: bool) {
        self.danger = danger;
    }

    /// Game pause: freeze the whole output (channels + music hold
    /// their positions, the device streams silence). Retail suspends
    /// ALL sound while paused; mixer requests made meanwhile (the
    /// map-toggle ding) sit queued and flush on the first unpaused
    /// tick — the original's deferred-ding quirk (our per-id request
    /// slot plays it once even if the map toggled twice).
    pub fn set_paused(&mut self, on: bool) {
        let _ = self.out.tx.send(output::Cmd::Suspend { on });
    }

    /// Load an audio bundle directory (`baked/assets/<game>-audio`)
    /// and select a sample bank (0 = the gameplay bank).
    pub fn load_bundle(&mut self, dir: &Path, bank: u32) -> Result<(), String> {
        let bundle = AudioBundle::load(dir).map_err(|e| e.to_string())?;
        self.sounds = Sounds::from_bundle(&bundle, bank);
        if self.sounds.is_none() {
            return Err(format!("{}: no sample bank {bank}", dir.display()));
        }
        self.bundle = Some(bundle);
        Ok(())
    }

    pub fn has_sounds(&self) -> bool {
        self.sounds.is_some()
    }

    /// Forward one sim sound event into the faithful mixer.
    pub fn event(&mut self, id: u8, source: Source, listener: &Listener) {
        if self.sounds.is_some() {
            self.mixer.request(id, source, listener);
        }
    }

    /// Per-sim-tick flush (30 Hz — the fade ramps are per-tick).
    pub fn tick(&mut self) {
        if let Some(sounds) = &self.sounds {
            self.mixer.tick(sounds, &self.out.tx, self.out.live_mask());
        }
        // Danger-stem ramp: step 2 per driver callback, callback rate
        // 0x3C/s in danger, 0x14/s in calm (sub_20D00) → per 30 Hz
        // sim tick: +4 / -1.33 on the 0..126 counter.
        let target = if self.danger { 126.0 } else { 0.0 };
        if (self.danger_level - target).abs() > f32::EPSILON {
            let step = if self.danger { 4.0 } else { -2.0 * 20.0 / 30.0 };
            self.danger_level = (self.danger_level + step).clamp(0.0, 126.0);
            let _ = self.out.tx.send(output::Cmd::MusicOverlayGain {
                gain: self.danger_level / 126.0,
            });
        }
    }

    /// Play a bundle music track by name (`cgame1`, `track-02`),
    /// looped. No-op if it is already the one playing.
    pub fn play_music(&mut self, name: &str, looped: bool) -> Result<(), String> {
        if self.music_playing.as_deref() == Some(name) {
            return Ok(());
        }
        let Some(bundle) = &self.bundle else {
            return Err("no audio bundle loaded".into());
        };
        let Some(index) = &bundle.music else {
            return Err("bundle has no music".into());
        };
        let Some(track) = index.tracks.iter().find(|t| t.name == name) else {
            return Err(format!("no music track named {name}"));
        };
        let decoded = music::decode_flac(&bundle.dir.join(&track.file))?;
        let overlay = match &track.danger_file {
            Some(f) => Some(music::decode_flac(&bundle.dir.join(f))?.pcm),
            None => None,
        };
        let _ = self.out.tx.send(output::Cmd::Music {
            pcm: decoded.pcm,
            overlay,
            channels: decoded.channels,
            sample_rate: decoded.sample_rate,
            looped,
        });
        self.music_playing = Some(name.to_string());
        Ok(())
    }

    pub fn stop_music(&mut self) {
        let _ = self.out.tx.send(output::Cmd::StopMusic);
        self.music_playing = None;
    }

    /// Master gains, 0..=1.
    pub fn set_volumes(&mut self, sfx: f32, music: f32) {
        let _ = self.out.tx.send(output::Cmd::MasterVol { sfx, music });
    }
}
