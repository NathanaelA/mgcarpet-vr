//! HMP (HMI MIDIPACK) song parser — the format inside MC1's
//! `DATA/MUSIC<bank>-<driver>.DAT` archives.
//!
//! Layout (probed byte-exact against retail MUSIC0/1; the old HMP
//! variant, no `013195` sub-signature):
//! - `+0x00` `"HMIMIDIP"` signature
//! - `+0x30` u32 track count
//! - `+0x38` u32 beats per minute (120 in every retail song); the
//!   stream tick rate is `bpm` ticks/second — retail songs decode to
//!   plausible song lengths (CGAME1 = 4:20) only at 120 ticks/s
//! - `+0x308` track chunks, back to back: 12-byte header
//!   `{u32 index, u32 len (header included), u32 midi_channel}`
//!   followed by the event stream.
//!
//! Event stream: delta time then a standard MIDI status byte (no
//! running status). Delta VLQ is little-endian 7-bit groups — bytes
//! with bit 7 CLEAR continue, the byte with bit 7 SET ends the
//! quantity (the reverse of SMF). Note-offs are note-ons with
//! velocity 0 (retail streams contain no 0x8n statuses). The only
//! meta in retail data is 0x2F end-of-track.

/// One MIDI-ish event at an absolute tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub tick: u32,
    /// Source track index — kept so the global merge is stable in the
    /// original driver's processing order.
    pub track: u32,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    NoteOn { ch: u8, note: u8, vel: u8 },
    NoteOff { ch: u8, note: u8 },
    Program { ch: u8, prog: u8 },
    Control { ch: u8, ctrl: u8, val: u8 },
    PitchBend { ch: u8, value: u16 },
}

pub struct Song {
    /// Ticks per second (`bpm` header field).
    pub tick_rate: u32,
    /// All tracks merged, sorted by `(tick, track)`.
    pub events: Vec<Event>,
    /// Last tick of the song (max end-of-track position).
    pub end_tick: u32,
}

const SIGNATURE: &[u8; 8] = b"HMIMIDIP";
const TRACK_COUNT_AT: usize = 0x30;
const BPM_AT: usize = 0x38;
const TRACKS_AT: usize = 0x308;
const TRACK_HEADER: usize = 12;

pub fn parse(data: &[u8]) -> Result<Song, String> {
    if data.len() < TRACKS_AT || &data[..8] != SIGNATURE {
        return Err("not an HMIMIDIP stream".into());
    }
    let read_u32 = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let track_count = read_u32(TRACK_COUNT_AT);
    let tick_rate = read_u32(BPM_AT);
    if tick_rate == 0 {
        return Err("zero tick rate".into());
    }

    let mut events = Vec::new();
    let mut end_tick = 0u32;
    let mut pos = TRACKS_AT;
    for t in 0..track_count {
        if pos + TRACK_HEADER > data.len() {
            return Err(format!("track {t}: truncated header at {pos}"));
        }
        let len = read_u32(pos + 4) as usize;
        let channel = read_u32(pos + 8);
        if len < TRACK_HEADER || pos + len > data.len() {
            return Err(format!("track {t}: bad length {len}"));
        }
        let stream = &data[pos + TRACK_HEADER..pos + len];
        let tick =
            parse_track(t, channel, stream, &mut events).map_err(|e| format!("track {t}: {e}"))?;
        end_tick = end_tick.max(tick);
        pos += len;
    }
    events.sort_by_key(|e| (e.tick, e.track));
    Ok(Song {
        tick_rate,
        events,
        end_tick,
    })
}

/// Walk one track stream; returns the end-of-track tick.
fn parse_track(
    track: u32,
    header_channel: u32,
    data: &[u8],
    out: &mut Vec<Event>,
) -> Result<u32, String> {
    let _ = header_channel; // channel comes from each status byte
    let mut p = 0usize;
    let mut tick = 0u32;
    loop {
        // Delta: LE 7-bit groups, terminator has bit 7 set.
        let mut delta = 0u32;
        let mut shift = 0u32;
        loop {
            let Some(&b) = data.get(p) else {
                return Err("truncated delta".into());
            };
            p += 1;
            delta |= u32::from(b & 0x7F) << shift;
            shift += 7;
            if b & 0x80 != 0 {
                break;
            }
            if shift > 28 {
                return Err("runaway delta".into());
            }
        }
        tick = tick.wrapping_add(delta);

        let Some(&status) = data.get(p) else {
            return Err("truncated status".into());
        };
        p += 1;
        if status == 0xFF {
            let (Some(&meta), Some(&len)) = (data.get(p), data.get(p + 1)) else {
                return Err("truncated meta".into());
            };
            p += 2 + len as usize;
            if meta == 0x2F {
                return Ok(tick);
            }
            continue;
        }
        let ch = status & 0x0F;
        let kind = status >> 4;
        let need = if matches!(kind, 0xC | 0xD) { 1 } else { 2 };
        if p + need > data.len() {
            return Err("truncated event data".into());
        }
        let d0 = data[p];
        let d1 = if need == 2 { data[p + 1] } else { 0 };
        p += need;
        let kind = match kind {
            0x9 if d1 > 0 => EventKind::NoteOn {
                ch,
                note: d0,
                vel: d1,
            },
            0x8 | 0x9 => EventKind::NoteOff { ch, note: d0 },
            0xB => EventKind::Control {
                ch,
                ctrl: d0,
                val: d1,
            },
            0xC => EventKind::Program { ch, prog: d0 },
            0xE => EventKind::PitchBend {
                ch,
                value: u16::from(d0) | (u16::from(d1) << 7),
            },
            // 0xA polyphonic aftertouch / 0xD channel pressure: absent
            // from retail data; skip without an event.
            _ => continue,
        };
        out.push(Event { tick, track, kind });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(tracks: &[(u32, &[u8])]) -> Vec<u8> {
        let mut d = vec![0u8; TRACKS_AT];
        d[..8].copy_from_slice(SIGNATURE);
        d[TRACK_COUNT_AT..TRACK_COUNT_AT + 4].copy_from_slice(&(tracks.len() as u32).to_le_bytes());
        d[BPM_AT..BPM_AT + 4].copy_from_slice(&120u32.to_le_bytes());
        for (i, (ch, stream)) in tracks.iter().enumerate() {
            d.extend_from_slice(&(i as u32).to_le_bytes());
            d.extend_from_slice(&((stream.len() + TRACK_HEADER) as u32).to_le_bytes());
            d.extend_from_slice(&ch.to_le_bytes());
            d.extend_from_slice(stream);
        }
        d
    }

    #[test]
    fn parses_notes_and_deltas() {
        // delta 0, prog; delta 30, note on; delta 0x83 (3 + 1<<7 =
        // 131), note off (vel 0); delta 0, EOT.
        let stream: &[u8] = &[
            0x80, 0xC3, 0x40, //
            0x9E, 0x93, 0x30, 0x50, //
            0x03, 0x81, 0x93, 0x30, 0x00, //
            0x80, 0xFF, 0x2F, 0x00,
        ];
        let s = parse(&song(&[(3, stream)])).unwrap();
        assert_eq!(s.tick_rate, 120);
        assert_eq!(s.end_tick, 161);
        assert_eq!(
            s.events,
            vec![
                Event {
                    tick: 0,
                    track: 0,
                    kind: EventKind::Program { ch: 3, prog: 0x40 }
                },
                Event {
                    tick: 30,
                    track: 0,
                    kind: EventKind::NoteOn {
                        ch: 3,
                        note: 0x30,
                        vel: 0x50
                    }
                },
                Event {
                    tick: 161,
                    track: 0,
                    kind: EventKind::NoteOff { ch: 3, note: 0x30 }
                },
            ]
        );
    }

    #[test]
    fn merge_is_stable_by_track() {
        let a: &[u8] = &[0x85, 0x90, 0x10, 0x7F, 0x80, 0xFF, 0x2F, 0x00];
        let b: &[u8] = &[0x85, 0x91, 0x20, 0x7F, 0x80, 0xFF, 0x2F, 0x00];
        let s = parse(&song(&[(0, a), (1, b)])).unwrap();
        assert_eq!(s.events.len(), 2);
        assert_eq!(s.events[0].track, 0);
        assert_eq!(s.events[1].track, 1);
    }
}
