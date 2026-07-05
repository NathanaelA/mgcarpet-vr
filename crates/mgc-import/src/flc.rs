//! Decoder for the Autodesk FLI/FLC animation streams embedded in
//! animated TMAPS entries.
//!
//! An animated TMAPS payload (flags bit 0) is the normal
//! `{u16 flags, u16 w, u16 h}` header and base image, followed by:
//!
//! ```text
//! u16 frame_count
//! u32 stream_len          (byte length of the FRAME chunks that follow)
//! frame_count times:
//!   u32 size  u16 magic=0xF1FA  u16 chunks  u8 pad[8]     FRAME chunk
//!   chunks times:
//!     u32 size  u16 type  <data>                          sub-chunk
//! ```
//!
//! This is Autodesk Animator's FLC frame encoding verbatim (Bullfrog
//! authored these animations in Animator); retail MC1 data uses exactly
//! two pixel sub-chunk types, verified over every entry of both
//! tilesets: type 7 (`DELTA_FLC`, word-oriented delta) and type 16
//! (`FLI_COPY`, raw full-frame refresh). A few MC2 streams additionally
//! carry palette sub-chunks (4 `COLOR_256` / 11 `COLOR_64`), skipped —
//! frames stay 8bpp indices against the environment's external palette.
//! Each frame patches the previous one; the decoder returns the full
//! frame sequence, base image first.

#[derive(Debug, PartialEq, Eq)]
pub enum FlcError {
    /// Payload shorter than its own header/declared lengths.
    Truncated,
    /// A FRAME chunk without the 0xF1FA magic.
    BadMagic(u16),
    /// A sub-chunk type outside the retail repertoire {4, 7, 11, 16}.
    UnknownChunk(u16),
    /// Delta data walked outside the frame canvas.
    OutOfBounds,
}

impl std::fmt::Display for FlcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated FLC stream"),
            Self::BadMagic(m) => write!(f, "FRAME chunk magic {m:#06x}, expected 0xf1fa"),
            Self::UnknownChunk(t) => write!(f, "unknown FLC sub-chunk type {t}"),
            Self::OutOfBounds => write!(f, "FLC delta writes outside the canvas"),
        }
    }
}

impl std::error::Error for FlcError {}

fn u16le(b: &[u8], at: usize) -> Result<u16, FlcError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(FlcError::Truncated)
}

fn u32le(b: &[u8], at: usize) -> Result<u32, FlcError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(FlcError::Truncated)
}

/// Decode the animation tail of an animated TMAPS entry. `base` is the
/// decoded base image (`w*h` bytes), `tail` everything after it in the
/// payload. Returns all frames including the base.
pub fn decode_frames(
    w: usize,
    h: usize,
    base: &[u8],
    tail: &[u8],
) -> Result<Vec<Vec<u8>>, FlcError> {
    debug_assert_eq!(base.len(), w * h);
    let frame_count = u16le(tail, 0)? as usize;
    let stream_len = u32le(tail, 2)? as usize;
    let stream = tail.get(6..6 + stream_len).ok_or(FlcError::Truncated)?;

    let mut frames = Vec::with_capacity(frame_count + 1);
    frames.push(base.to_vec());
    let mut canvas = base.to_vec();

    let mut off = 0usize;
    for _ in 0..frame_count {
        let size = u32le(stream, off)? as usize;
        let magic = u16le(stream, off + 4)?;
        if magic != 0xF1FA {
            return Err(FlcError::BadMagic(magic));
        }
        let chunks = u16le(stream, off + 6)? as usize;
        let frame_end = off.checked_add(size).ok_or(FlcError::Truncated)?;
        if frame_end > stream.len() || size < 16 {
            return Err(FlcError::Truncated);
        }
        let mut coff = off + 16;
        for _ in 0..chunks {
            let csize = u32le(stream, coff)? as usize;
            let ctype = u16le(stream, coff + 4)?;
            let cend = coff.checked_add(csize).ok_or(FlcError::Truncated)?;
            if cend > frame_end || csize < 6 {
                return Err(FlcError::Truncated);
            }
            let data = &stream[coff + 6..cend];
            match ctype {
                // COLOR_256 / COLOR_64: palette updates. Frames stay
                // 8bpp indices and the palette is the environment's
                // external PAL file, so these carry nothing for us
                // (present in a few MC2 water streams).
                4 | 11 => {}
                7 => delta_flc(&mut canvas, w, h, data)?,
                16 => {
                    // FLI_COPY: raw full-frame refresh.
                    if data.len() < w * h {
                        return Err(FlcError::Truncated);
                    }
                    canvas.copy_from_slice(&data[..w * h]);
                }
                other => return Err(FlcError::UnknownChunk(other)),
            }
            coff = cend;
        }
        frames.push(canvas.clone());
        off = frame_end;
    }
    Ok(frames)
}

/// `DELTA_FLC` (type 7): word-oriented delta against the previous
/// frame. Layout: `u16 lines`, then per encoded line, control words:
/// bits 15-14 = 11 -> negative line skip; 10 -> low byte is the line's
/// last pixel (odd widths); 00 -> packet count, followed by packets of
/// `{u8 column_skip, i8 count}` — positive count copies `count` literal
/// words, negative replicates one word `-count` times.
fn delta_flc(canvas: &mut [u8], w: usize, h: usize, data: &[u8]) -> Result<(), FlcError> {
    let mut off = 0usize;
    let next = |n: usize, off: &mut usize| -> Result<usize, FlcError> {
        let at = *off;
        *off += n;
        if *off > data.len() {
            return Err(FlcError::Truncated);
        }
        Ok(at)
    };
    let lines = {
        let at = next(2, &mut off)?;
        u16le(data, at)? as usize
    };
    let mut y = 0usize;
    for _ in 0..lines {
        // Control words prefixing this line.
        let packets = loop {
            let at = next(2, &mut off)?;
            let word = u16le(data, at)?;
            match word >> 14 {
                0b11 => {
                    // Negative line skip.
                    y = y
                        .checked_add((word as i16).unsigned_abs() as usize)
                        .ok_or(FlcError::OutOfBounds)?;
                }
                0b10 => {
                    // Odd-width tail pixel for the current line.
                    if y >= h || w == 0 {
                        return Err(FlcError::OutOfBounds);
                    }
                    canvas[y * w + (w - 1)] = word as u8;
                }
                _ => break word as usize,
            }
        };
        if y >= h {
            return Err(FlcError::OutOfBounds);
        }
        let mut x = 0usize;
        for _ in 0..packets {
            let at = next(2, &mut off)?;
            let skip = data[at] as usize;
            let count = data[at + 1] as i8;
            x += skip;
            if count >= 0 {
                let n = count as usize * 2;
                let at = next(n, &mut off)?;
                if x + n > w {
                    return Err(FlcError::OutOfBounds);
                }
                canvas[y * w + x..y * w + x + n].copy_from_slice(&data[at..at + n]);
                x += n;
            } else {
                let n = (-(count as i32)) as usize;
                let at = next(2, &mut off)?;
                let (a, b) = (data[at], data[at + 1]);
                if x + n * 2 > w {
                    return Err(FlcError::OutOfBounds);
                }
                for i in 0..n {
                    canvas[y * w + x + i * 2] = a;
                    canvas[y * w + x + i * 2 + 1] = b;
                }
                x += n * 2;
            }
        }
        y += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_chunk(sub: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(16 + sub.len() as u32).to_le_bytes());
        v.extend_from_slice(&0xF1FAu16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&[0u8; 8]);
        v.extend_from_slice(sub);
        v
    }

    fn sub_chunk(ctype: u16, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(6 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(&ctype.to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    fn tail(frames: &[Vec<u8>]) -> Vec<u8> {
        let stream: Vec<u8> = frames.concat();
        let mut v = Vec::new();
        v.extend_from_slice(&(frames.len() as u16).to_le_bytes());
        v.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        v.extend_from_slice(&stream);
        v
    }

    #[test]
    fn copy_chunk_replaces_frame() {
        let base = vec![1u8; 8]; // 4x2
        let refresh = vec![9u8; 8];
        let t = tail(&[frame_chunk(&sub_chunk(16, &refresh))]);
        let frames = decode_frames(4, 2, &base, &t).unwrap();
        assert_eq!(frames, vec![base, refresh]);
    }

    #[test]
    fn delta_literal_and_replicate() {
        let base = vec![0u8; 8]; // 4x2
        // 2 lines encoded: line 0: 1 packet, skip 1, copy 1 word [7,8];
        // line 1: 1 packet, skip 0, replicate word [5,6] once.
        let delta = [
            2, 0, // lines
            1, 0, // line 0: 1 packet
            1, 1, 7, 8, // skip 1, +1 word literal
            1, 0, // line 1: 1 packet
            0, 0xFF, 5, 6, // skip 0, -1 -> one word replicated
        ];
        let t = tail(&[frame_chunk(&sub_chunk(7, &delta))]);
        let frames = decode_frames(4, 2, &base, &t).unwrap();
        assert_eq!(frames[1], vec![0, 7, 8, 0, 5, 6, 0, 0]);
    }

    #[test]
    fn delta_line_skip_and_tail_pixel() {
        let base = vec![0u8; 6]; // 3x2
        let delta = [
            1, 0, // lines
            0xFF, 0xFF, // skip -1 lines -> y = 1
            0x21, 0x80, // tail pixel of line 1 = 0x21
            0, 0, // 0 packets
        ];
        let t = tail(&[frame_chunk(&sub_chunk(7, &delta))]);
        let frames = decode_frames(3, 2, &base, &t).unwrap();
        assert_eq!(frames[1], vec![0, 0, 0, 0, 0, 0x21]);
    }

    #[test]
    fn rejects_unknown_chunk_and_bad_magic() {
        let base = vec![0u8; 4];
        let t = tail(&[frame_chunk(&sub_chunk(12, &[0, 0]))]);
        assert_eq!(
            decode_frames(2, 2, &base, &t),
            Err(FlcError::UnknownChunk(12))
        );
        // Palette sub-chunks (4/11) are tolerated as no-ops.
        let t = tail(&[frame_chunk(&sub_chunk(11, &[0, 0]))]);
        assert_eq!(decode_frames(2, 2, &base, &t).unwrap().len(), 2);
        let mut bad = frame_chunk(&sub_chunk(16, &[0; 4]));
        bad[4] = 0;
        let t = tail(&[bad]);
        assert!(matches!(
            decode_frames(2, 2, &base, &t),
            Err(FlcError::BadMagic(_))
        ));
    }
}
