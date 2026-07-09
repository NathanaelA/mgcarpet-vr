//! Standard MIDI File (type 0) encoder for parsed HMP songs — the
//! bridge from `MUSIC<bank>-2` (the General MIDI arrangement, driver
//! digit 2 = the original's `GENERAL` target, remc1 :54029-30) to an
//! external GM renderer (fluidsynth).
//!
//! The HMP tick rate is `bpm` ticks/SECOND (see [`crate::hmp`]); the
//! SMF is written with division = tick_rate ticks per quarter and a
//! fixed 1 000 000 µs tempo, so one HMP tick = one SMF tick = 1/rate
//! seconds exactly.
//!
//! [`crate::adlib::MixSpec`] carries over: silenced channels (pin 0)
//! are dropped wholesale; pinned channels get the pin injected as a
//! tick-0 CC7 and their own CC7 stream dropped (the danger-stem solo
//! at the original fade ceiling). The end-of-track meta lands on the
//! song's `end_tick` in every mix, so ambient and stem renders stay
//! the same musical length.

use crate::adlib::MixSpec;
use crate::hmp::{EventKind, Song};

fn vlq(out: &mut Vec<u8>, mut v: u32) {
    let mut stack = [0u8; 5];
    let mut n = 0;
    loop {
        stack[n] = (v & 0x7F) as u8;
        v >>= 7;
        n += 1;
        if v == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        out.push(stack[i] | if i > 0 { 0x80 } else { 0 });
    }
}

/// Encode `song` as a type-0 SMF with the mix applied.
pub fn encode(song: &Song, mix: &MixSpec) -> Vec<u8> {
    let mut track = Vec::new();
    // Tempo: 1 000 000 µs per quarter → division ticks/second.
    track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x0F, 0x42, 0x40]);
    // Pinned channels: the override as a tick-0 CC7.
    for (ch, over) in mix.chan_override.iter().enumerate() {
        if let Some(v) = over {
            if *v > 0 {
                track.extend_from_slice(&[0x00, 0xB0 | ch as u8, 0x07, *v]);
            }
        }
    }
    let mut last_tick = 0u32;
    for ev in &song.events {
        let (ch, bytes): (u8, [u8; 3]) = match ev.kind {
            EventKind::NoteOn { ch, note, vel } => (ch, [0x90 | ch, note, vel]),
            EventKind::NoteOff { ch, note } => (ch, [0x90 | ch, note, 0]),
            EventKind::Program { ch, prog } => (ch, [0xC0 | ch, prog, 0xFF]),
            EventKind::Control { ch, ctrl, val } => (ch, [0xB0 | ch, ctrl, val]),
            EventKind::PitchBend { ch, value } => {
                (ch, [0xE0 | ch, (value & 0x7F) as u8, (value >> 7) as u8])
            }
        };
        match mix.chan_override[ch as usize] {
            Some(0) => continue, // silenced: the channel is gone
            Some(_) => {
                // Pinned: the song's own CC7 yields to the pin.
                if matches!(ev.kind, EventKind::Control { ctrl: 7, .. }) {
                    continue;
                }
            }
            None => {}
        }
        vlq(&mut track, ev.tick - last_tick);
        last_tick = ev.tick;
        if bytes[0] & 0xF0 == 0xC0 {
            track.extend_from_slice(&bytes[..2]);
        } else {
            track.extend_from_slice(&bytes);
        }
    }
    // End-of-track pinned to the song end, mix-independent.
    vlq(&mut track, song.end_tick.saturating_sub(last_tick));
    track.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    let mut out = Vec::with_capacity(track.len() + 22);
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // format 0
    out.extend_from_slice(&1u16.to_be_bytes()); // one track
    out.extend_from_slice(&(song.tick_rate as u16).to_be_bytes());
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(track.len() as u32).to_be_bytes());
    out.extend_from_slice(&track);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmp::Event;

    fn song(events: Vec<Event>, end: u32) -> Song {
        Song {
            tick_rate: 120,
            events,
            end_tick: end,
        }
    }

    #[test]
    fn vlq_matches_smf_spec() {
        let mut v = Vec::new();
        vlq(&mut v, 0);
        vlq(&mut v, 0x7F);
        vlq(&mut v, 0x80);
        vlq(&mut v, 0x3FFF);
        vlq(&mut v, 0x4000);
        assert_eq!(v, [0x00, 0x7F, 0x81, 0x00, 0xFF, 0x7F, 0x81, 0x80, 0x00]);
    }

    #[test]
    fn end_of_track_is_mix_independent() {
        let ev = |tick, ch| Event {
            tick,
            track: 0,
            kind: EventKind::NoteOn {
                ch,
                note: 60,
                vel: 100,
            },
        };
        let s = song(vec![ev(0, 1), ev(240, 3)], 480);
        let full = encode(&s, &MixSpec::full());
        let ambient = encode(&s, &MixSpec::ambient());
        // Both end with delta-to-480 then FF 2F 00.
        assert_eq!(&full[full.len() - 3..], &[0xFF, 0x2F, 0x00]);
        assert_eq!(&ambient[ambient.len() - 3..], &[0xFF, 0x2F, 0x00]);
        // The ambient mix dropped the danger-channel note.
        assert!(full.len() > ambient.len());
    }
}
