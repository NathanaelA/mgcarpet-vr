//! Decoder for the standalone Bullfrog FMV streams (`GLOBE.DAT`,
//! `TIMER.DAT`, `SCROLL.DAT`, the INTRO `.DAT` movies): a 12-byte
//! header `{u32 header_size=12, u16 magic=0xAF12, u16 frame_count,
//! u16 width, u16 height}` followed by Autodesk Animator FRAME
//! chunks — the same encoding the animated TMAPS tails use
//! (`crate::flc`), but with the fuller chunk repertoire of a
//! standalone movie: BRUN keyframes, LC (FLI byte-delta), BLACK,
//! COLOR palettes, prefix frames. Retail decoder:
//! `PlayInfoFmv`/`ReadFrame_75DB0`/`DrawFrame_75E70` (remc2
//! Animation.cpp:41-77; remc1 `PlayInfoFmv_107C0`/`sub_1002D` is the
//! same code).
//!
//! The caller seeds the canvas: the menu mini-movies (GLOBE/TIMER)
//! are pure delta streams drawn OVER the live menu screen — their
//! frames only touch their own pixels, so decoding against the menu
//! background reproduces the retail composite.

use crate::flc;

#[derive(Debug)]
pub struct Fmv {
    pub width: usize,
    pub height: usize,
    /// One full canvas per frame, in play order.
    pub frames: Vec<Vec<u8>>,
    /// The stream's embedded palette (first COLOR chunk), 256×RGB —
    /// these files store 6-bit VGA components (GLOBE's is
    /// byte-identical to MAINMENU.PAL; TIMER/SCROLL differ in a few
    /// entries, so frames blitted under a foreign palette need an
    /// index remap).
    pub palette: Option<[u8; 768]>,
}

const FRAME_MAGIC: u16 = 0xF1FA;
const PREFIX_MAGIC: u16 = 0xF100;

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Decode a whole FMV file. `seed` pre-fills the canvas (pass the
/// target screen for the delta-only menu movies; None = black).
pub fn decode(file: &[u8], seed: Option<&[u8]>) -> Result<Fmv, String> {
    let hdr_len = u32le(file, 0).ok_or("fmv: truncated header")? as usize;
    let magic = u16le(file, 4).ok_or("fmv: truncated header")?;
    if hdr_len != 12 || magic != 0xAF12 {
        return Err(format!("fmv: bad header ({hdr_len}, {magic:#x})"));
    }
    let frame_count = u16le(file, 6).unwrap() as usize;
    let width = u16le(file, 8).ok_or("fmv: truncated header")? as usize;
    let height = u16le(file, 10).ok_or("fmv: truncated header")? as usize;
    if width == 0 || height == 0 || width * height > 1 << 20 {
        return Err(format!("fmv: implausible {width}x{height}"));
    }

    let mut canvas = match seed {
        Some(s) if s.len() == width * height => s.to_vec(),
        Some(_) => return Err("fmv: seed does not match dimensions".into()),
        None => vec![0u8; width * height],
    };
    let mut frames = Vec::with_capacity(frame_count);
    let mut palette: Option<[u8; 768]> = None;
    let mut off = 12usize;
    while frames.len() < frame_count && off + 16 <= file.len() {
        // NOTE: the declared frame size is UNRELIABLE in the retail
        // SCREENS movies (GLOBE.DAT frame 0 declares 8400 bytes but
        // its chunks span 7716 — the next frame header sits right
        // after the chunks). The real extent is the 16-byte header
        // plus the sum of its sub-chunk sizes.
        let ftype = u16le(file, off + 4).ok_or("fmv: truncated frame")?;
        if ftype != FRAME_MAGIC && ftype != PREFIX_MAGIC {
            return Err(format!("fmv: unknown frame type {ftype:#x}"));
        }
        let chunks = u16le(file, off + 6).unwrap() as usize;
        let mut coff = off + 16;
        for _ in 0..chunks {
            let csize = u32le(file, coff).ok_or("fmv: truncated chunk")? as usize;
            let ctype = u16le(file, coff + 4).ok_or("fmv: truncated chunk")?;
            if csize < 6 || coff + csize > file.len() {
                return Err("fmv: chunk overruns file".into());
            }
            let data = &file[coff + 6..coff + csize];
            // Prefix frames (0xF100: settings/celdata) carry no
            // pixels — retail skips them (DrawFrame's first arm).
            if ftype == FRAME_MAGIC {
                match ctype {
                    // COLOR_256 / COLOR_64: the canvas stays 8bpp
                    // indices (retail blits indices under the
                    // screen's installed palette), but the FIRST
                    // palette is captured for bake-time remapping.
                    4 | 11 => {
                        if palette.is_none() {
                            palette = parse_color_chunk(data);
                        }
                    }
                    // PSTAMP: preview thumbnail, no canvas effect.
                    0x12 => {}
                    7 => flc::delta_flc(&mut canvas, width, height, data)
                        .map_err(|e| format!("fmv: SS2: {e}"))?,
                    0xC => delta_fli(&mut canvas, width, height, data)?,
                    0xD => canvas.fill(0),
                    0xF => brun(&mut canvas, width, height, data)?,
                    0x10 => {
                        if data.len() < width * height {
                            return Err("fmv: COPY short".into());
                        }
                        canvas.copy_from_slice(&data[..width * height]);
                    }
                    other => return Err(format!("fmv: unknown chunk {other:#x}")),
                }
            }
            coff += csize;
        }
        if ftype == FRAME_MAGIC {
            frames.push(canvas.clone());
        }
        off = coff;
    }
    Ok(Fmv {
        width,
        height,
        frames,
        palette,
    })
}

/// `COLOR_256`/`COLOR_64` payload: `u16 packets`, each
/// `{u8 skip, u8 count (0 = 256)}` + `count×3` RGB bytes.
fn parse_color_chunk(data: &[u8]) -> Option<[u8; 768]> {
    let packets = u16le(data, 0)? as usize;
    let mut pal = [0u8; 768];
    let mut off = 2usize;
    let mut index = 0usize;
    for _ in 0..packets {
        index += *data.get(off)? as usize;
        let count = match *data.get(off + 1)? as usize {
            0 => 256,
            n => n,
        };
        off += 2;
        for _ in 0..count {
            if index >= 256 {
                return None;
            }
            pal[index * 3..index * 3 + 3].copy_from_slice(data.get(off..off + 3)?);
            index += 1;
            off += 3;
        }
    }
    Some(pal)
}

/// `FLI_BRUN` (0xF): full-frame keyframe. Per line: a (legacy) packet
/// count byte, then packets `{i8 count}`: positive = replicate the
/// next byte `count` times, negative = copy `-count` literal bytes.
/// Decoded by width, not by packet count (the count byte overflows at
/// >255 packets and players ignore it).
fn brun(canvas: &mut [u8], w: usize, h: usize, data: &[u8]) -> Result<(), String> {
    let mut off = 0usize;
    for y in 0..h {
        off += 1; // legacy packet count
        let mut x = 0usize;
        while x < w {
            let n = *data.get(off).ok_or("fmv: BRUN truncated")? as i8;
            off += 1;
            if n >= 0 {
                let count = (n as usize).min(w - x);
                let v = *data.get(off).ok_or("fmv: BRUN truncated")?;
                off += 1;
                canvas[y * w + x..y * w + x + count].fill(v);
                x += count;
            } else {
                let count = ((-(n as i32)) as usize).min(w - x);
                let src = data.get(off..off + count).ok_or("fmv: BRUN truncated")?;
                canvas[y * w + x..y * w + x + count].copy_from_slice(src);
                off += count;
                x += count;
            }
        }
    }
    Ok(())
}

/// `FLI_LC` (0xC): byte-oriented delta. `u16 skip_lines`, `u16
/// line_count`; per line `u8 packets`, packets `{u8 column_skip, i8
/// count}`: positive = literal copy, negative = replicate one byte
/// (the sign convention is the INVERSE of BRUN's).
fn delta_fli(canvas: &mut [u8], w: usize, h: usize, data: &[u8]) -> Result<(), String> {
    let skip = u16le(data, 0).ok_or("fmv: LC truncated")? as usize;
    let lines = u16le(data, 2).ok_or("fmv: LC truncated")? as usize;
    let mut off = 4usize;
    for y in skip..(skip + lines).min(h) {
        let packets = *data.get(off).ok_or("fmv: LC truncated")? as usize;
        off += 1;
        let mut x = 0usize;
        for _ in 0..packets {
            x += *data.get(off).ok_or("fmv: LC truncated")? as usize;
            let n = *data.get(off + 1).ok_or("fmv: LC truncated")? as i8;
            off += 2;
            if n >= 0 {
                let count = (n as usize).min(w.saturating_sub(x));
                let src = data.get(off..off + n as usize).ok_or("fmv: LC truncated")?;
                canvas[y * w + x..y * w + x + count].copy_from_slice(&src[..count]);
                off += n as usize;
                x += count;
            } else {
                let count = ((-(n as i32)) as usize).min(w.saturating_sub(x));
                let v = *data.get(off).ok_or("fmv: LC truncated")?;
                off += 1;
                canvas[y * w + x..y * w + x + count].fill(v);
                x += count;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the menu mini-movies against the pristine install: GLOBE =
    /// 30 frames of 320×200, TIMER = 3, SCROLL = 26 (headers verified
    /// against the retail 12-byte layout).
    #[test]
    fn menu_movies_decode() {
        let found = crate::gamedata::Gamedata::locate(std::path::Path::new("../../gamedata"));
        let Some(src) = found.mc1 else { return };
        let bg = src
            .read("DATA/SCREENS/MAINMENU.DAT")
            .expect("MAINMENU.DAT readable");
        for (name, want_frames) in [
            ("DATA/SCREENS/GLOBE.DAT", 30usize),
            ("DATA/SCREENS/TIMER.DAT", 3),
            ("DATA/SCREENS/SCROLL.DAT", 26),
        ] {
            let file = src.read(name).expect("movie readable");
            let fmv = decode(&file, Some(&bg)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((fmv.width, fmv.height), (320, 200), "{name} dims");
            // Retail plays frames 0..count-1 (the loop breaks at
            // count-1); the file may terminate before the nominal
            // last frame.
            assert!(
                fmv.frames.len() + 1 >= want_frames,
                "{name}: {} frames (want ~{want_frames})",
                fmv.frames.len()
            );
            // The deltas must actually touch pixels — a frame differs
            // from the seed.
            assert!(
                fmv.frames.iter().any(|f| f != &bg),
                "{name}: frames never differ from the seed"
            );
        }
    }
}
