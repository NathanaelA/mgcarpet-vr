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

use mgc_formats::bundle::AudioBundle;
pub use mixer::{FaithfulMixer, Listener, Sounds, Source};

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
    /// Per-game danger ramp steps per 30 Hz sim tick on the 0..126
    /// counter. MC1: +4 / −1.33 (CC7 step 2 at 0x3C/0x14 Hz). MC2:
    /// ±3 both ways (cc11 step ±1 at 90 Hz — Sound.cpp:5877/6076).
    danger_up: f32,
    danger_down: f32,
    /// Prefer the General MIDI render (`gm_file`) when the bundle
    /// carries it; the FM render is the always-present fallback.
    prefer_gm: bool,
    /// Voiceover duck state: retail drops music+sfx to 1/3 the
    /// instant a line starts (FadeDownSoundVolume_59A50) and ramps
    /// them back when it ends (the 120 Hz FadeUpSoundVolume timer).
    duck_gain: f32,
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
            danger_up: 4.0,
            danger_down: -2.0 * 20.0 / 30.0,
            prefer_gm: true,
            duck_gain: 1.0,
        }
    }

    /// MC2's danger ramp: cc11 expression step ±1 at 90 Hz on the
    /// war channels (Sound.cpp:5877, timer 30×3 Hz) → ±3 per 30 Hz
    /// sim tick, both directions.
    pub fn set_mc2_danger_ramp(&mut self) {
        self.danger_up = 3.0;
        self.danger_down = -3.0;
    }

    /// Pick the music arrangement (config `audio.arrangement`): `true`
    /// prefers the GM render when baked, `false` forces FM. Applies
    /// from the next `play_music` — the playing track is not restarted.
    pub fn set_prefer_gm(&mut self, prefer_gm: bool) {
        self.prefer_gm = prefer_gm;
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
        // Danger-stem ramp on the 0..126 counter, per-game rates
        // (see `danger_up`/`danger_down`).
        let target = if self.danger { 126.0 } else { 0.0 };
        if (self.danger_level - target).abs() > f32::EPSILON {
            let step = if self.danger {
                self.danger_up
            } else {
                self.danger_down
            };
            self.danger_level = (self.danger_level + step).clamp(0.0, 126.0);
            let _ = self.out.tx.send(output::Cmd::MusicOverlayGain {
                gain: self.danger_level / 126.0,
            });
        }
        // Voiceover duck recovery: once the line ends, ramp music+sfx
        // back up (retail's 120 Hz FadeUpSoundVolume ≈ 0.7 s full
        // traverse — APPROX, the exact per-callback step is a
        // volume-scale detail).
        if self.duck_gain < 1.0 && !self.out.speech_live() {
            self.duck_gain = (self.duck_gain + (2.0 / 3.0) / 21.0).min(1.0);
            let _ = self.out.tx.send(output::Cmd::Duck {
                gain: self.duck_gain,
            });
        }
    }

    /// Play one voiceover clip (`CdTracks_DB080` address: table row =
    /// 0-based level number, segment slot). Ducks music+sfx to 1/3
    /// for the clip's duration; a new line interrupts the playing one
    /// (retail `PlayCDTrackSegment_86FF0` stops before starting).
    /// Missing clips (empty retail slots) are a quiet no-op.
    pub fn play_speech(&mut self, row: u32, segment: u32) -> Result<(), String> {
        let Some(bundle) = &self.bundle else {
            return Err("no audio bundle loaded".into());
        };
        let Some(index) = &bundle.speech else {
            return Err("bundle has no speech".into());
        };
        let Some(clip) = index
            .clips
            .iter()
            .find(|c| c.row == row && c.segment == segment)
        else {
            return Ok(()); // empty slot — retail no-ops on length 0
        };
        let decoded = music::decode_flac(&bundle.dir.join(&clip.file))?;
        let _ = self.out.tx.send(output::Cmd::Speech {
            pcm: decoded.pcm,
            channels: decoded.channels,
            sample_rate: decoded.sample_rate,
        });
        self.duck_gain = 1.0 / 3.0;
        let _ = self.out.tx.send(output::Cmd::Duck {
            gain: self.duck_gain,
        });
        Ok(())
    }

    /// Whether the bundle carries voiceover clips at all.
    pub fn has_speech(&self) -> bool {
        self.bundle.as_ref().is_some_and(|b| b.speech.is_some())
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
        let (file, danger_file) = match &track.gm_file {
            Some(gm) if self.prefer_gm => (gm, &track.gm_danger_file),
            _ => (&track.file, &track.danger_file),
        };
        let decoded = music::decode_flac(&bundle.dir.join(file))?;
        let overlay = match danger_file {
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
