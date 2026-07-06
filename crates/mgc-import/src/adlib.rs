//! AdLib instrument banks (`DATA/INST.BNK` + `DATA/DRUM.BNK`) and the
//! import-time OPL3 song renderer.
//!
//! BNK is the classic Ad Lib Instrument Bank format (field order
//! cross-checked against libADLMIDI gen_adldata load_bnk.h): header
//! `{u8 ver_major, u8 ver_minor, "ADLIB-", u16 used, u16 total,
//! u32 names_off, u32 data_off}`; name entries are 12 bytes
//! `{u16 data_index, u8 usage_flag, char name[9]}`; instrument
//! records are 30 bytes `{u8 percussive, u8 voice_num,
//! u8 mod[13], u8 car[13], u8 mod_wave, u8 car_wave}` with per-op
//! fields `[ksl, mult, feedback, attack, sustain, sustain_flag,
//! decay, release, level, tremolo, vibrato, ksr, connection]`
//! (feedback/connection meaningful on the modulator only).
//!
//! MC1's banks are GM-ordered: `INST.BNK` entry N = MIDI program N
//! (entry 0 = `piano1`), `DRUM.BNK` entry N = percussion patch for
//! MIDI note N on channel 9 (35 = `bdc1` bass drum, 38 = snare — the
//! GM percussion map).
//!
//! The renderer plays a parsed [`crate::hmp::Song`] through the
//! nuked-opl3 emulator the way the era's HMI AdLib driver did:
//! 2-op patches on the 18 OPL3 channels, note pitch from the MIDI
//! note, drums as ordinary patches keyed by note. INTERIM (fidelity
//! pass owed, playtest is the oracle): note velocity and CC7 volume
//! are IGNORED — retail songs carry CC7=0 and velocities 1..9 on
//! busy melodic channels in *every* arrangement, which only renders
//! sensibly if the driver used raw patch levels; pitch-bend range is
//! assumed ±2 semitones.

use crate::hmp::{EventKind, Song};

/// One 2-op OPL patch, register-encoded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Patch {
    /// Register 0x20 family: AM|VIB|EG|KSR|MULT.
    pub op_20: [u8; 2],
    /// Register 0x40 family: KSL|TL.
    pub op_40: [u8; 2],
    /// Register 0x60 family: attack|decay.
    pub op_60: [u8; 2],
    /// Register 0x80 family: sustain|release.
    pub op_80: [u8; 2],
    /// Register 0xE0 family: waveform.
    pub op_e0: [u8; 2],
    /// Register 0xC0: feedback|connection (channel-wide).
    pub c0: u8,
}

/// A parsed BNK: 128 slots (GM programs or percussion notes); absent
/// slots stay all-zero (silent).
pub struct Bank {
    pub patches: [Patch; 128],
}

const BNK_SIG: &[u8; 6] = b"ADLIB-";
const NAME_ENTRY: usize = 12;
const RECORD: usize = 30;

pub fn parse_bnk(data: &[u8]) -> Result<Bank, String> {
    if data.len() < 20 || &data[2..8] != BNK_SIG {
        return Err("not an ADLIB- bank".into());
    }
    let used = u16::from_le_bytes(data[8..10].try_into().unwrap()) as usize;
    let names_off = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let data_off = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let mut patches = [Patch::default(); 128];
    for n in 0..used.min(128) {
        let ne = names_off + n * NAME_ENTRY;
        if ne + NAME_ENTRY > data.len() {
            return Err(format!("name entry {n} out of range"));
        }
        let index = u16::from_le_bytes(data[ne..ne + 2].try_into().unwrap()) as usize;
        let at = data_off + index * RECORD;
        if at + RECORD > data.len() {
            return Err(format!("instrument record {index} out of range"));
        }
        let rec = &data[at..at + RECORD];
        let op = |o: &[u8]| -> (u8, u8, u8, u8) {
            (
                (o[9] << 7) | (o[10] << 6) | (o[5] << 5) | (o[11] << 4) | (o[1] & 0x0F),
                ((o[0] & 3) << 6) | (o[8] & 0x3F),
                (o[3] << 4) | (o[6] & 0x0F),
                (o[4] << 4) | (o[7] & 0x0F),
            )
        };
        let m = &rec[2..15];
        let c = &rec[15..28];
        let (m20, m40, m60, m80) = op(m);
        let (c20, c40, c60, c80) = op(c);
        patches[n] = Patch {
            op_20: [m20, c20],
            op_40: [m40, c40],
            op_60: [m60, c60],
            op_80: [m80, c80],
            op_e0: [rec[28] & 7, rec[29] & 7],
            c0: ((m[2] & 7) << 1) | (m[12] & 1),
        };
    }
    Ok(Bank { patches })
}

/// Per-op register offsets of the 18 OPL3 2-op channels; channels
/// 0..8 live in register file 0, 9..17 in file 1 (reg | 0x100).
const SLOT_OFFSET: [u16; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x10, 0x11, 0x12];

/// MIDI percussion channel.
const DRUM_CH: u8 = 9;

struct Voice {
    /// MIDI (channel, note) sounding here; None = free.
    key: Option<(u8, u8)>,
    /// Allocation stamp for oldest-first stealing.
    age: u64,
    /// Block/fnum high bits as last written (for keyoff).
    b0: u8,
}

/// Per-MIDI-channel volume overrides for one rendered mix. `None` =
/// follow the song's own CC7; `Some(v)` = pin the channel to volume
/// `v` (0 silences it). This is how the danger-layer stems bake: the
/// original keeps the layers on channels 3/4/5 at CC7 0 and fades
/// them via runtime CC7 ramps (sub_20BD0 sends Bn 07 xx to 0xB3..0xB5;
/// sub_20D00 = the mode switch) — so the ambient mix pins 3/4/5 to 0
/// and the danger stem solos them at the fade ceiling 126.
#[derive(Clone, Copy)]
pub struct MixSpec {
    pub chan_override: [Option<u8>; 16],
}

impl MixSpec {
    /// Follow the song's CC7 everywhere.
    pub fn full() -> MixSpec {
        MixSpec {
            chan_override: [None; 16],
        }
    }

    /// The ambient mix: danger channels silenced.
    pub fn ambient() -> MixSpec {
        let mut m = MixSpec::full();
        for ch in DANGER_CHANNELS {
            m.chan_override[ch as usize] = Some(0);
        }
        m
    }

    /// The danger stem: ONLY the danger channels, at the original
    /// fade ceiling (126).
    pub fn danger_stem() -> MixSpec {
        let mut m = MixSpec {
            chan_override: [Some(0); 16],
        };
        for ch in DANGER_CHANNELS {
            m.chan_override[ch as usize] = Some(126);
        }
        m
    }
}

/// The engine's danger-layer channels (sub_20BD0 targets 0xB3/B4/B5).
pub const DANGER_CHANNELS: [u8; 3] = [3, 4, 5];

/// True when a song carries a danger layer: notes on a danger channel
/// whose first CC7 is 0 (the layer starts muted; menu/intro songs use
/// the channels normally and get no stem).
pub fn has_danger_layer(song: &Song) -> bool {
    let mut first_cc7: [Option<u8>; 16] = [None; 16];
    let mut muted_notes = false;
    for ev in &song.events {
        match ev.kind {
            EventKind::Control { ch, ctrl: 7, val } => {
                if first_cc7[ch as usize].is_none() {
                    first_cc7[ch as usize] = Some(val);
                }
            }
            EventKind::NoteOn { ch, .. }
                if DANGER_CHANNELS.contains(&ch) && first_cc7[ch as usize] == Some(0) =>
            {
                muted_notes = true;
            }
            _ => {}
        }
    }
    muted_notes
}

/// Renders `song` to mono i16 PCM at `sample_rate` Hz. Pure function
/// of its inputs — the OPL chip is fresh per call. CC7 is honored as
/// channel volume, applied at note-on (INTERIM: live CC7 ramps on
/// sounding notes are not re-applied; retail songs only set CC7 up
/// front). Velocity stays ignored (see module doc).
pub fn render(
    song: &Song,
    inst: &Bank,
    drum: &Bank,
    sample_rate: u32,
    mix: &MixSpec,
) -> Result<Vec<i16>, String> {
    let mut chip = nuked_opl3::Opl3Chip::new(sample_rate);
    // OPL3 mode on, waveform select on.
    chip.write_register(0x105, 0x01);
    chip.write_register(0x001, 0x20);

    let mut voices: Vec<Voice> = (0..18)
        .map(|_| Voice {
            key: None,
            age: 0,
            b0: 0,
        })
        .collect();
    let mut chan_prog = [0u8; 16];
    let mut chan_bend = [8192u16; 16];
    let mut chan_vol = [127u8; 16];
    let mut clock = 0u64;

    let chan_regs = |v: usize| -> (u16, u16, u16) {
        // (op1 slot base, op2 slot base, channel base) with file bit.
        let file = if v >= 9 { 0x100 } else { 0 };
        let c = v % 9;
        (
            file | SLOT_OFFSET[c],
            file | (SLOT_OFFSET[c] + 3),
            file | c as u16,
        )
    };

    let mut out = Vec::new();
    let samples_per_tick = f64::from(sample_rate) / f64::from(song.tick_rate);
    let mut next_event = 0usize;
    let mut emitted = 0u64;

    // One tail second past the last tick lets releases ring out.
    let total_ticks = u64::from(song.end_tick) + u64::from(song.tick_rate);

    for tick in 0..=total_ticks {
        while next_event < song.events.len()
            && u64::from(song.events[next_event].tick) <= tick
        {
            let ev = song.events[next_event];
            next_event += 1;
            clock += 1;
            match ev.kind {
                EventKind::Program { ch, prog } => chan_prog[ch as usize] = prog,
                EventKind::Control { ch, ctrl: 7, val } => chan_vol[ch as usize] = val,
                EventKind::Control { .. } => {} // CC10 pan etc: mono render
                EventKind::PitchBend { ch, value } => {
                    chan_bend[ch as usize] = value;
                    for (v, voice) in voices.iter_mut().enumerate() {
                        if let Some((vch, note)) = voice.key {
                            if vch == ch {
                                let (_, _, cb) = chan_regs(v);
                                let (fnum, block) =
                                    fnum_block(pitch(note, value));
                                voice.b0 = 0x20 | (block << 2) | (fnum >> 8) as u8;
                                chip.write_register(0xA0 + cb, (fnum & 0xFF) as u8);
                                chip.write_register(0xB0 + cb, voice.b0);
                            }
                        }
                    }
                }
                EventKind::NoteOff { ch, note } => {
                    for (v, voice) in voices.iter_mut().enumerate() {
                        if voice.key == Some((ch, note)) {
                            let (_, _, cb) = chan_regs(v);
                            chip.write_register(0xB0 + cb, voice.b0 & !0x20);
                            voice.key = None;
                        }
                    }
                }
                EventKind::NoteOn { ch, note, vel: _ } => {
                    let vol = mix.chan_override[ch as usize]
                        .unwrap_or(chan_vol[ch as usize])
                        .min(127);
                    if vol == 0 {
                        continue; // silenced channel: skip the voice
                    }
                    let mut patch = if ch == DRUM_CH {
                        drum.patches[note as usize & 127]
                    } else {
                        inst.patches[chan_prog[ch as usize] as usize & 127]
                    };
                    // Channel volume onto the carrier level (and the
                    // modulator too under additive connection):
                    // level' = 63 - (63 - level)·vol/127.
                    let scale = |op40: u8| -> u8 {
                        let ksl = op40 & 0xC0;
                        let tl = i32::from(op40 & 0x3F);
                        let out = 63 - (63 - tl) * i32::from(vol) / 127;
                        ksl | (out as u8 & 0x3F)
                    };
                    patch.op_40[1] = scale(patch.op_40[1]);
                    if patch.c0 & 1 != 0 {
                        patch.op_40[0] = scale(patch.op_40[0]);
                    }
                    // Voice allocation: free, else oldest.
                    let v = voices
                        .iter()
                        .position(|vc| vc.key.is_none())
                        .unwrap_or_else(|| {
                            voices
                                .iter()
                                .enumerate()
                                .min_by_key(|(_, vc)| vc.age)
                                .map(|(i, _)| i)
                                .unwrap()
                        });
                    let (o1, o2, cb) = chan_regs(v);
                    // Keyoff first if stealing.
                    chip.write_register(0xB0 + cb, voices[v].b0 & !0x20);
                    for (i, base) in [(0usize, o1), (1usize, o2)] {
                        chip.write_register(0x20 + base, patch.op_20[i]);
                        chip.write_register(0x40 + base, patch.op_40[i]);
                        chip.write_register(0x60 + base, patch.op_60[i]);
                        chip.write_register(0x80 + base, patch.op_80[i]);
                        chip.write_register(0xE0 + base, patch.op_e0[i]);
                    }
                    // Both stereo outputs on.
                    chip.write_register(0xC0 + cb, patch.c0 | 0x30);
                    let (fnum, block) =
                        fnum_block(pitch(note, chan_bend[ch as usize]));
                    let b0 = 0x20 | (block << 2) | (fnum >> 8) as u8;
                    chip.write_register(0xA0 + cb, (fnum & 0xFF) as u8);
                    chip.write_register(0xB0 + cb, b0);
                    voices[v] = Voice {
                        key: Some((ch, note)),
                        age: clock,
                        b0,
                    };
                }
            }
        }

        // Emit this tick's samples (mono = left channel).
        let due = ((tick + 1) as f64 * samples_per_tick) as u64;
        let mut frame = [0i16; 2];
        while emitted < due {
            chip.generate_resampled(&mut frame)
                .map_err(|e| e.to_string())?;
            out.push(frame[0]);
            emitted += 1;
        }
    }
    Ok(out)
}

/// MIDI note + 14-bit bend (center 8192, ±2 semitones) → frequency Hz.
fn pitch(note: u8, bend: u16) -> f64 {
    let n = f64::from(note) + (f64::from(bend) - 8192.0) / 4096.0;
    440.0 * ((n - 69.0) / 12.0).exp2()
}

/// Frequency → OPL (fnum, block) at the chip's native 49716 Hz clock.
fn fnum_block(freq: f64) -> (u16, u8) {
    let mut block = 0u8;
    let mut fnum = freq * f64::from(1 << 20) / 49716.0;
    while fnum > 1023.0 && block < 7 {
        fnum /= 2.0;
        block += 1;
    }
    (fnum.min(1023.0) as u16, block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmp::Event;

    #[test]
    fn fnum_block_covers_midi_range() {
        // A4 = 440 Hz lands mid-range with a legal fnum.
        let (f, b) = fnum_block(440.0);
        assert!(f <= 1023 && b <= 7, "fnum={f} block={b}");
        // Frequency doubling bumps the block, not the fnum, once high.
        let (f1, b1) = fnum_block(880.0);
        assert!(b1 >= b || f1 > f);
    }

    #[test]
    fn renders_a_note_to_nonzero_audio() {
        let mut inst = Bank {
            patches: [Patch::default(); 128],
        };
        // A crude organ: both ops sounding, additive, instant attack.
        inst.patches[0] = Patch {
            op_20: [0x01, 0x01],
            op_40: [0x10, 0x00],
            op_60: [0xF0, 0xF0],
            op_80: [0x77, 0x77],
            op_e0: [0, 0],
            c0: 0x01,
        };
        let drum = Bank {
            patches: [Patch::default(); 128],
        };
        let song = Song {
            tick_rate: 120,
            end_tick: 60,
            events: vec![
                Event {
                    tick: 0,
                    track: 0,
                    kind: EventKind::NoteOn {
                        ch: 0,
                        note: 69,
                        vel: 127,
                    },
                },
                Event {
                    tick: 48,
                    track: 0,
                    kind: EventKind::NoteOff { ch: 0, note: 69 },
                },
            ],
        };
        let pcm = render(&song, &inst, &drum, 22050, &MixSpec::full()).unwrap();
        // 60 ticks + 1s tail at 22050 → about 1.5s of audio.
        assert!(pcm.len() > 22050, "{} samples", pcm.len());
        let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 1000, "peak {peak} — chip stayed silent");
    }
}
