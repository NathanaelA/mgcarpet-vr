//! Redbook audio rip: MC2's in-game music is CD audio — 27 short
//! tracks (~45 s each) following the data track inside `game.gog`
//! (a raw MODE1/2352 dump). The GOG install ships the cue sheet as
//! `game.ins` next to the image.
//!
//! Cue semantics used here: single-FILE sheet, `INDEX 01 mm:ss:ff`
//! positions are file-absolute MSF (75 frames/second, 2352 bytes per
//! frame); a `PREGAP` directive is virtual silence *not stored* in
//! the file, so it never shifts file offsets. Track N's audio runs
//! from its INDEX 01 to the next track's INDEX 01 (EOF for the last).
//! Audio frames are 588 stereo samples of little-endian s16 at
//! 44100 Hz.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const SECTOR: u64 = 2352;
pub const RATE: u32 = 44100;

/// One audio track: number and file-absolute sector range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioTrack {
    pub number: u32,
    pub start_sector: u64,
    /// Exclusive; patched to EOF for the last track at rip time.
    pub end_sector: u64,
}

/// Parse a cue sheet's AUDIO tracks. `image_sectors` bounds the last
/// track.
pub fn parse_cue(cue: &str, image_sectors: u64) -> Result<Vec<AudioTrack>, String> {
    let mut tracks: Vec<AudioTrack> = Vec::new();
    let mut current: Option<u32> = None;
    for line in cue.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("TRACK") => {
                let number: u32 = words
                    .next()
                    .ok_or("TRACK without number")?
                    .parse()
                    .map_err(|_| "bad TRACK number")?;
                let kind = words.next().ok_or("TRACK without type")?;
                current = (kind == "AUDIO").then_some(number);
            }
            Some("INDEX") => {
                let idx = words.next().ok_or("INDEX without number")?;
                let msf = words.next().ok_or("INDEX without MSF")?;
                if idx != "01" {
                    continue;
                }
                let sector = msf_to_sector(msf)?;
                if let Some(prev) = tracks.last_mut() {
                    if prev.end_sector == 0 {
                        prev.end_sector = sector;
                    }
                }
                if let Some(number) = current {
                    tracks.push(AudioTrack {
                        number,
                        start_sector: sector,
                        end_sector: 0,
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(last) = tracks.last_mut() {
        if last.end_sector == 0 {
            last.end_sector = image_sectors;
        }
    }
    for t in &tracks {
        if t.start_sector >= t.end_sector || t.end_sector > image_sectors {
            return Err(format!(
                "track {}: bad sector range {}..{} (image has {image_sectors})",
                t.number, t.start_sector, t.end_sector
            ));
        }
    }
    Ok(tracks)
}

fn msf_to_sector(msf: &str) -> Result<u64, String> {
    let parts: Vec<u64> = msf
        .split(':')
        .map(|p| p.parse::<u64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("bad MSF {msf}"))?;
    let [m, s, f] = parts[..] else {
        return Err(format!("bad MSF {msf}"));
    };
    Ok((m * 60 + s) * 75 + f)
}

/// Read one track's PCM (interleaved stereo s16) from the image.
pub fn read_track(image: &Path, track: AudioTrack) -> std::io::Result<Vec<i16>> {
    let mut f = std::fs::File::open(image)?;
    f.seek(SeekFrom::Start(track.start_sector * SECTOR))?;
    let bytes = (track.end_sector - track.start_sector) * SECTOR;
    let mut raw = vec![0u8; bytes as usize];
    f.read_exact(&mut raw)?;
    Ok(raw
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_mc2_cue_shape() {
        let cue = "FILE \"game.gog\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    PREGAP 00:02:00\n    INDEX 01 18:57:63\n  TRACK 03 AUDIO\n    INDEX 01 19:48:68\n";
        let tracks = parse_cue(cue, 100_000).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].number, 2);
        assert_eq!(tracks[0].start_sector, (18 * 60 + 57) * 75 + 63);
        assert_eq!(tracks[0].end_sector, (19 * 60 + 48) * 75 + 68);
        assert_eq!(tracks[1].end_sector, 100_000);
    }

    #[test]
    fn data_track_index_bounds_nothing() {
        // The data track's INDEX must not open a phantom track.
        let cue = "TRACK 01 MODE1/2352\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:10:00\n";
        let tracks = parse_cue(cue, 10_000).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].start_sector, 750);
    }
}
