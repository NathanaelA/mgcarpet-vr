//! MC2's compiled CD-speech segment table `CdTracks_DB080[28]` —
//! the per-level objective-voiceover index (remc2
//! `Type_DB080_CdTrack.h:19`, trace docs/traces/mc2-voiceover-
//! triggers.md).
//!
//! One row per redbook track: row = 0-based level number, physical
//! CD track = row + 1 (`TrackIdx_0`). Each row holds 10 `(startPos,
//! length)` segment slots in CD FRAMES (75/s); `{0,0}` slots are
//! empty (retail no-ops on `length == 0`). Segment semantics:
//! - segment 0 = the level's map-screen intro line,
//! - segment N+1 = objective row N's spoken line,
//! - segment 9 = the level-completion line.
//!
//! Rows 25/26 = the two secret-level one-liners; row 27 is dead data
//! (its implied audio track does not exist — see the trace).
//!
//! Retail converts frames → ms with `× 13.33333333333` truncated to
//! int (EF:48001-02) and its own SDL backend seeks the PER-TRACK rip
//! by that ms alone — `TrackOffsets_180084` (drive TOC) applies only
//! to the dead MSCDEX path, so per-track rips need NO correction.
//! The port applies the conversion uniformly, including the secret
//! rows (deliberate: retail's secret path skips it — a latent bug
//! that would cut those clips 13× short; trace §1b).

pub struct CdTrack {
    /// Physical CD track number (1-based; rip member `track-NN`).
    pub track: u8,
    /// `(startPos, length)` in CD frames; `(_, 0)` = empty slot.
    pub segments: [(u16, u16); 10],
}

/// Frames (75/s) → milliseconds, retail's exact law: a double
/// multiply truncated to int32 (EF:48001-02).
pub fn frames_to_ms(frames: u16) -> u32 {
    (f64::from(frames) * 13.33333333333) as u32
}

pub const CD_TRACKS: [CdTrack; 28] = [
    CdTrack {
        track: 1,
        segments: [
            (0x0000, 0x02EE),
            (0x0339, 0x012C),
            (0x04B0, 0x01C2),
            (0x06BD, 0x01C2),
            (0x08CA, 0x0177),
            (0x0A8C, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0C4E, 0x020D),
        ],
    },
    CdTrack {
        track: 2,
        segments: [
            (0x0000, 0x0465),
            (0x04B0, 0x020D),
            (0x0708, 0x0258),
            (0x09AB, 0x012C),
            (0x0B22, 0x020D),
            (0x0D7A, 0x01C2),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0F87, 0x0258),
        ],
    },
    CdTrack {
        track: 3,
        segments: [
            (0x0000, 0x0384),
            (0x03CF, 0x02A3),
            (0x06BD, 0x0177),
            (0x087F, 0x01C2),
            (0x0A8C, 0x0177),
            (0x0C4E, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0DC5, 0x0177),
        ],
    },
    CdTrack {
        track: 4,
        segments: [
            (0x0000, 0x020D),
            (0x0258, 0x012C),
            (0x03CF, 0x01C2),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x05DC, 0x0258),
        ],
    },
    CdTrack {
        track: 5,
        segments: [
            (0x0000, 0x02EE),
            (0x0339, 0x0177),
            (0x04FB, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0672, 0x02A3),
        ],
    },
    CdTrack {
        track: 6,
        segments: [
            (0x0000, 0x0384),
            (0x03CF, 0x0177),
            (0x0591, 0x0177),
            (0x0753, 0x020D),
            (0x09AB, 0x01C2),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0BB8, 0x0177),
        ],
    },
    CdTrack {
        track: 7,
        segments: [
            (0x0000, 0x0465),
            (0x04B0, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0672, 0x020D),
        ],
    },
    CdTrack {
        track: 8,
        segments: [
            (0x0000, 0x0384),
            (0x03CF, 0x01C2),
            (0x05DC, 0x0177),
            (0x079E, 0x012C),
            (0x0915, 0x012C),
            (0x0A8C, 0x0258),
            (0x0D2F, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0F87, 0x012C),
        ],
    },
    CdTrack {
        track: 9,
        segments: [
            (0x0000, 0x03CF),
            (0x041A, 0x0177),
            (0x05DC, 0x0096),
            (0x06BD, 0x00E1),
            (0x07E9, 0x020D),
            (0x0A41, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0C99, 0x020D),
        ],
    },
    CdTrack {
        track: 10,
        segments: [
            (0x0000, 0x02A3),
            (0x02EE, 0x020D),
            (0x0546, 0x0177),
            (0x0708, 0x0258),
            (0x09AB, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0C03, 0x012C),
        ],
    },
    CdTrack {
        track: 11,
        segments: [
            (0x0000, 0x0258),
            (0x02A3, 0x0177),
            (0x0465, 0x020D),
            (0x06BD, 0x0177),
            (0x087F, 0x01C2),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0A8C, 0x012C),
        ],
    },
    CdTrack {
        track: 12,
        segments: [
            (0x0000, 0x0546),
            (0x0591, 0x0258),
            (0x0834, 0x01C2),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0A41, 0x012C),
        ],
    },
    CdTrack {
        track: 13,
        segments: [
            (0x0000, 0x03CF),
            (0x041A, 0x01C2),
            (0x0627, 0x0177),
            (0x07E9, 0x0177),
            (0x09AB, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0B22, 0x01C2),
        ],
    },
    CdTrack {
        track: 14,
        segments: [
            (0x0000, 0x02EE),
            (0x0339, 0x02A3),
            (0x0627, 0x020D),
            (0x087F, 0x01C2),
            (0x0A8C, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0C4E, 0x020D),
        ],
    },
    CdTrack {
        track: 15,
        segments: [
            (0x0000, 0x0465),
            (0x04B0, 0x00E1),
            (0x05DC, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0834, 0x020D),
        ],
    },
    CdTrack {
        track: 16,
        segments: [
            (0x0000, 0x0339),
            (0x0384, 0x020D),
            (0x05DC, 0x00E1),
            (0x0708, 0x00E1),
            (0x0834, 0x0177),
            (0x09F6, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0B6D, 0x0177),
        ],
    },
    CdTrack {
        track: 17,
        segments: [
            (0x0000, 0x0465),
            (0x04B0, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0672, 0x012C),
        ],
    },
    CdTrack {
        track: 18,
        segments: [
            (0x0000, 0x05DC),
            (0x0627, 0x01C2),
            (0x0834, 0x012C),
            (0x09AB, 0x01C2),
            (0x0BB8, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0D7A, 0x012C),
        ],
    },
    CdTrack {
        track: 19,
        segments: [
            (0x0000, 0x020D),
            (0x0258, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x03CF, 0x00E1),
        ],
    },
    CdTrack {
        track: 20,
        segments: [
            (0x0000, 0x0339),
            (0x0384, 0x020D),
            (0x05DC, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0834, 0x01C2),
        ],
    },
    CdTrack {
        track: 21,
        segments: [
            (0x0000, 0x0339),
            (0x0384, 0x01C2),
            (0x0591, 0x0177),
            (0x0753, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x09AB, 0x0177),
        ],
    },
    CdTrack {
        track: 22,
        segments: [
            (0x0000, 0x0465),
            (0x04B0, 0x00E1),
            (0x05DC, 0x020D),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0834, 0x01C2),
        ],
    },
    CdTrack {
        track: 23,
        segments: [
            (0x0000, 0x03CF),
            (0x041A, 0x012C),
            (0x0591, 0x0177),
            (0x0753, 0x020D),
            (0x09AB, 0x012C),
            (0x0B22, 0x012C),
            (0x0C99, 0x01C2),
            (0x0EA6, 0x0177),
            (0x0000, 0x0000),
            (0x1068, 0x012C),
        ],
    },
    CdTrack {
        track: 24,
        segments: [
            (0x0000, 0x0339),
            (0x0384, 0x0177),
            (0x0546, 0x020D),
            (0x079E, 0x012C),
            (0x0915, 0x012C),
            (0x0A8C, 0x012C),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0C03, 0x0177),
        ],
    },
    CdTrack {
        track: 25,
        segments: [
            (0x0000, 0x04B0),
            (0x0000, 0x0000),
            (0x04FB, 0x02A3),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x07E9, 0x0627),
        ],
    },
    CdTrack {
        track: 26,
        segments: [
            (0x0000, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
        ],
    },
    CdTrack {
        track: 27,
        segments: [
            (0x0000, 0x0177),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
            (0x0000, 0x0000),
        ],
    },
    CdTrack {
        track: 28,
        segments: [
            (0x5BD0, 0x0107),
            (0x5BE0, 0x0107),
            (0x5BF0, 0x0107),
            (0x5C00, 0x0107),
            (0x5C10, 0x0107),
            (0x5C24, 0x0107),
            (0x5C34, 0x0107),
            (0x5C44, 0x0107),
            (0x5C60, 0x0107),
            (0x5C70, 0x0107),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_shape_matches_retail() {
        // Physical track = row + 1, uniformly.
        for (row, t) in CD_TRACKS.iter().enumerate() {
            assert_eq!(t.track as usize, row + 1);
        }
        // Row 0 segment 0 = a 10.0 s clip (0x2EE = 750 frames);
        // retail's truncating double multiply lands on 9999 ms.
        assert_eq!(frames_to_ms(0x2EE), 9999);
        assert_eq!(frames_to_ms(0), 0);
        // The retail truncation law: 0x177 = 375 frames → 4999 ms.
        assert_eq!(frames_to_ms(0x177), 4999);
    }
}
