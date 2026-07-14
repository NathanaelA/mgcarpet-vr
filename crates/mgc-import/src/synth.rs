//! Pure-Rust General-MIDI renderer: [`oxisynth`] (a SoundFont synth
//! with no C dependency) + a GM soundfont. Replaces the old external
//! `fluidsynth` CLI shell-out, so the GM music arrangement renders on
//! any host — Windows/macOS/bare-Linux — with no system binary, only a
//! soundfont file.
//!
//! The GM bake is still OPTIONAL: it upgrades the bundle when a GM
//! soundfont is found and is skipped (FM render only) when none is.
//! Discovery: `MGC_SOUNDFONT` override first, then the shipped
//! `GeneralUser-GS.sf2` next to the executable, then the in-repo copy
//! under `assets/static/`, then the usual distro soundfont locations.
//!
//! Rendering is driven off the same type-0 SMF bytes the FM/XMI paths
//! already produce (`crate::smf` / `crate::xmi`): the SMF is parsed with
//! [`midly`], its events are streamed into the synth on a tempo-aware
//! sample clock, and the interleaved-stereo f32 result is handed back
//! to the caller for the same peak-normalize + quantize + FLAC step the
//! fluidsynth path used (loudness normalization must scale a base/
//! danger-stem pair by ONE factor to keep the overlay mix valid).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use oxisynth::{MidiEvent, SoundFont, Synth, SynthDescriptor};

/// Soundfont locations tried in order (after the `MGC_SOUNDFONT`
/// override): the shipped GeneralUser GS first, then common distro GM
/// fonts as a dev-host fallback.
const SOUNDFONT_CANDIDATES: &[&str] = &[
    "/usr/share/sounds/sf2/FluidR3_GM.sf2",
    "/usr/share/soundfonts/FluidR3_GM.sf2",
    "/usr/share/soundfonts/default.sf2",
    "/usr/share/sounds/sf2/default-GM.sf2",
    "/usr/share/sounds/sf2/TimGM6mb.sf2",
];

/// The soundfont basename shipped with the game (see the release
/// packaging). Kept next to the executable, or in the repo's
/// `assets/static/` dir during development.
const SHIPPED_SOUNDFONT: &str = "GeneralUser-GS.sf2";

pub struct GmRenderer {
    /// The whole soundfont, read once and reloaded per song (a fresh
    /// [`Synth`] per render keeps songs independent).
    font_bytes: Vec<u8>,
    pub soundfont: PathBuf,
}

impl GmRenderer {
    /// Find and validate a GM soundfont; `Err` (with the reason) when
    /// the host has none — the caller then bakes the FM arrangement
    /// only, exactly as before.
    pub fn locate() -> Result<GmRenderer, String> {
        let soundfont = match std::env::var_os("MGC_SOUNDFONT") {
            Some(p) => PathBuf::from(p),
            None => shipped_or_system().ok_or(
                "no GM soundfont found (ship GeneralUser-GS.sf2 beside the game or set \
                 MGC_SOUNDFONT)",
            )?,
        };
        let font_bytes =
            std::fs::read(&soundfont).map_err(|e| format!("{}: {e}", soundfont.display()))?;
        // Validate up front so a corrupt/foreign file fails locate(),
        // not mid-bake.
        SoundFont::load(&mut Cursor::new(&font_bytes))
            .map_err(|e| format!("{}: not a usable soundfont ({e:?})", soundfont.display()))?;
        Ok(GmRenderer {
            font_bytes,
            soundfont,
        })
    }

    /// Render a type-0 SMF to interleaved-stereo f32 at `rate` Hz,
    /// unity gain (unnormalized — the caller normalizes). Deterministic
    /// in its inputs: a fresh synth per call, tempo honored from the
    /// file, one tail second past the last event so releases ring out.
    pub fn render(&self, midi: &[u8], rate: u32) -> Result<Vec<f32>, String> {
        let smf = Smf::parse(midi).map_err(|e| format!("SMF parse: {e}"))?;
        let ticks_per_quarter = match smf.header.timing {
            Timing::Metrical(t) => u32::from(t.as_int()).max(1),
            Timing::Timecode(..) => return Err("SMPTE-timed SMF unsupported".into()),
        };

        // Flatten every track to absolute-tick events (type-0 has one,
        // but merge defensively), stable-sorted so same-tick order is
        // preserved.
        let mut events: Vec<(u64, TrackEventKind)> = Vec::new();
        for track in &smf.tracks {
            let mut abs = 0u64;
            for ev in track {
                abs += u64::from(ev.delta.as_int());
                events.push((abs, ev.kind));
            }
        }
        events.sort_by_key(|(t, _)| *t);

        let mut synth = Synth::new(SynthDescriptor {
            sample_rate: rate as f32,
            gain: 1.0,
            audio_channels: 1,
            ..Default::default()
        })
        .map_err(|e| format!("oxisynth init: {e:?}"))?;
        let font = SoundFont::load(&mut Cursor::new(&self.font_bytes))
            .map_err(|e| format!("soundfont load: {e:?}"))?;
        synth.add_font(font, true);

        let mut out: Vec<f32> = Vec::new();
        let mut cur_tick = 0u64;
        // MIDI default tempo until the file sets one (ours pins 1e6 at
        // tick 0). samples-per-tick = rate · (µs/quarter / 1e6) / ppq.
        let mut tempo_us = 500_000f64;
        let spt =
            |tempo_us: f64| rate as f64 * tempo_us / 1_000_000.0 / f64::from(ticks_per_quarter);
        let mut carry = 0f64; // fractional-sample accumulator

        for (tick, kind) in events {
            // Advance the sample clock from cur_tick to this event.
            if tick > cur_tick {
                carry += (tick - cur_tick) as f64 * spt(tempo_us);
                let n = carry as u64;
                carry -= n as f64;
                render_into(&mut synth, &mut out, n);
                cur_tick = tick;
            }
            match kind {
                TrackEventKind::Meta(MetaMessage::Tempo(us)) => tempo_us = f64::from(us.as_int()),
                TrackEventKind::Midi { channel, message } => {
                    if let Some(ev) = to_oxi(channel.as_int(), message) {
                        let _ = synth.send_event(ev);
                    }
                }
                _ => {}
            }
        }
        // One tail second for release/reverb ring-out.
        render_into(&mut synth, &mut out, u64::from(rate));
        Ok(out)
    }
}

/// Push `n` interleaved-stereo frames of synth output.
fn render_into(synth: &mut Synth, out: &mut Vec<f32>, n: u64) {
    out.reserve(2 * n as usize);
    for _ in 0..n {
        let (l, r) = synth.read_next();
        out.push(l);
        out.push(r);
    }
}

/// Translate a parsed MIDI channel message to an oxisynth event. A
/// note-on with velocity 0 is the MIDI idiom for note-off. Messages the
/// GM arrangement never emits (aftertouch) still map through for
/// robustness.
fn to_oxi(channel: u8, message: MidiMessage) -> Option<MidiEvent> {
    Some(match message {
        MidiMessage::NoteOn { key, vel } if vel.as_int() == 0 => MidiEvent::NoteOff {
            channel,
            key: key.as_int(),
        },
        MidiMessage::NoteOn { key, vel } => MidiEvent::NoteOn {
            channel,
            key: key.as_int(),
            vel: vel.as_int(),
        },
        MidiMessage::NoteOff { key, .. } => MidiEvent::NoteOff {
            channel,
            key: key.as_int(),
        },
        MidiMessage::Controller { controller, value } => MidiEvent::ControlChange {
            channel,
            ctrl: controller.as_int(),
            value: value.as_int(),
        },
        MidiMessage::ProgramChange { program } => MidiEvent::ProgramChange {
            channel,
            program_id: program.as_int(),
        },
        MidiMessage::PitchBend { bend } => MidiEvent::PitchBend {
            channel,
            value: bend.0.as_int(),
        },
        MidiMessage::ChannelAftertouch { vel } => MidiEvent::ChannelPressure {
            channel,
            value: vel.as_int(),
        },
        MidiMessage::Aftertouch { key, vel } => MidiEvent::PolyphonicKeyPressure {
            channel,
            key: key.as_int(),
            value: vel.as_int(),
        },
    })
}

/// The shipped soundfont (next to the exe, then the vendored dev copy),
/// then the distro GM fonts.
fn shipped_or_system() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    {
        candidates.push(dir.join(SHIPPED_SOUNDFONT));
        candidates.push(dir.join("soundfonts").join(SHIPPED_SOUNDFONT));
    }
    // The in-repo copy under `assets/static/` (dev builds run from the
    // source tree). `CARGO_MANIFEST_DIR` is `<root>/crates/mgc-import`,
    // baked at compile time — for a shipped build it points at the CI
    // path, which doesn't exist on the player's machine and is skipped.
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/static")
            .join(SHIPPED_SOUNDFONT),
    );
    // Last resort: whatever GM font the host distro provides.
    candidates.extend(SOUNDFONT_CANDIDATES.iter().map(PathBuf::from));
    candidates.into_iter().find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmp::{Event, EventKind, Song};

    /// Renders a single note through the real synth when a GM soundfont
    /// is available on the host; a no-op skip otherwise (CI without a
    /// font still passes). Guards the whole SMF→oxisynth path end-to-end.
    #[test]
    fn renders_a_note_to_nonzero_audio() {
        let Ok(renderer) = GmRenderer::locate() else {
            eprintln!("skip: no GM soundfont on this host");
            return;
        };
        let ev = |tick, kind| Event {
            tick,
            track: 0,
            kind,
        };
        let song = Song {
            tick_rate: 120,
            events: vec![
                ev(0, EventKind::Program { ch: 0, prog: 0 }),
                ev(
                    0,
                    EventKind::NoteOn {
                        ch: 0,
                        note: 60,
                        vel: 100,
                    },
                ),
                ev(60, EventKind::NoteOff { ch: 0, note: 60 }),
            ],
            end_tick: 60,
        };
        let midi = crate::smf::encode(&song, &crate::adlib::MixSpec::full());
        let pcm = renderer.render(&midi, 44100).unwrap();
        // 60 ticks @ 120/s = 0.5s of note + 1s tail, interleaved stereo.
        assert!(pcm.len() > 2 * 44100, "too short: {} samples", pcm.len());
        let peak = pcm.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.0, "synth produced silence");
    }
}
