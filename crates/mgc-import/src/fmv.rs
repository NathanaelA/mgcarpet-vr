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
//!
//! Two entry points. [`decode`] eagerly materialises every frame and
//! suits the 3-to-30-frame menu loops; [`FmvCursor`] walks the stream
//! one frame at a time against a single reusable canvas, which is what
//! the full-screen movies need — MC1's `INTRO.DAT` is 3165 frames, so
//! eager decoding would cost ~200 MB of canvases for a stream that is
//! 75 MB on disk and only ever shows one frame at a time.

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

/// A one-frame-at-a-time reader over a raw FMV stream. Retail plays
/// these movies exactly this way: one 320×200 buffer, decoded into in
/// place, blitted, repeat (`PlayInfoFmv_107C0`,
/// `ReadFrame_75DB0`/`DrawFrame_75E70`).
///
/// Generic over how the stream is held so a player can OWN it —
/// `FmvCursor<Vec<u8>>` for a movie read off disk and stepped across
/// frames, `FmvCursor<&[u8]>` to walk a buffer someone else keeps.
pub struct FmvCursor<B: AsRef<[u8]>> {
    file: B,
    width: usize,
    height: usize,
    /// Frame count from the header — the nominal length. The stream
    /// may end early (see [`FmvCursor::advance`]).
    frame_count: usize,
    canvas: Vec<u8>,
    palette: [u8; 768],
    have_palette: bool,
    /// Set when the frame just decoded carried a COLOR chunk, so a
    /// caller uploading the palette can skip untouched frames. The
    /// full-screen movies fade with live palette ramps, so this fires
    /// often, not just on frame 0.
    palette_changed: bool,
    off: usize,
    played: usize,
}

impl<B: AsRef<[u8]>> FmvCursor<B> {
    /// Parse the header and prime the canvas. `seed` pre-fills it (for
    /// the delta-only menu movies drawn over a live screen); `None`
    /// starts from black, which is what the full-screen movies want —
    /// they open on a BRUN keyframe.
    pub fn new(file: B, seed: Option<&[u8]>) -> Result<Self, String> {
        let (width, height, frame_count) = parse_header(file.as_ref())?;
        let canvas = match seed {
            Some(s) if s.len() == width * height => s.to_vec(),
            Some(_) => return Err("fmv: seed does not match dimensions".into()),
            None => vec![0u8; width * height],
        };
        Ok(FmvCursor {
            file,
            width,
            height,
            frame_count,
            canvas,
            palette: [0u8; 768],
            have_palette: false,
            palette_changed: false,
            off: 12,
            played: 0,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The header's nominal frame count. Retail's loop breaks at
    /// `count - 1`, and some streams stop short of even that, so treat
    /// this as an upper bound for progress reporting only — drive
    /// playback off [`FmvCursor::advance`] returning `false`.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Frames decoded so far.
    pub fn played(&self) -> usize {
        self.played
    }

    /// The current frame's pixels, as 8bpp palette indices.
    pub fn canvas(&self) -> &[u8] {
        &self.canvas
    }

    /// The palette in force, 256×RGB with 6-bit VGA components, or
    /// `None` before the stream's first COLOR chunk.
    pub fn palette(&self) -> Option<&[u8; 768]> {
        self.have_palette.then_some(&self.palette)
    }

    /// Whether the last [`FmvCursor::advance`] changed the palette.
    pub fn palette_changed(&self) -> bool {
        self.palette_changed
    }

    /// Decode the next frame into the canvas. `Ok(false)` means the
    /// stream is finished — either the header's count is exhausted or
    /// the file ran out of frames first, which several retail streams
    /// do. Chunk-level corruption is an `Err`; a clean short read is
    /// not.
    pub fn advance(&mut self) -> Result<bool, String> {
        self.palette_changed = false;
        let file = self.file.as_ref();
        loop {
            if self.played >= self.frame_count || self.off + 16 > file.len() {
                return Ok(false);
            }
            let mut sink = CursorSink {
                canvas: &mut self.canvas,
                palette: &mut self.palette,
                have_palette: &mut self.have_palette,
                palette_changed: &mut self.palette_changed,
            };
            let (next_off, was_pixel_frame) =
                decode_frame(file, self.off, self.width, self.height, &mut sink)?;
            self.off = next_off;
            // Prefix frames (0xF100) carry settings, not pixels; they
            // do not count as a frame, so keep going.
            if was_pixel_frame {
                self.played += 1;
                return Ok(true);
            }
        }
    }
}

/// `(width, height, frame_count)` if `file` opens with a Bullfrog FMV
/// header, else `None` — the cheap screen for "is this a movie?", used
/// to pick the streams out of a catalog without decoding them.
pub fn header(file: &[u8]) -> Option<(usize, usize, usize)> {
    parse_header(file).ok()
}

fn parse_header(file: &[u8]) -> Result<(usize, usize, usize), String> {
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
    Ok((width, height, frame_count))
}

/// Where a decoded frame's palette lands. [`decode`] keeps only the
/// first COLOR chunk (its callers blit under a fixed screen palette);
/// [`FmvCursor`] tracks the live one.
trait FrameSink {
    fn canvas(&mut self) -> &mut [u8];
    fn color(&mut self, data: &[u8]);
}

struct CursorSink<'a> {
    canvas: &'a mut [u8],
    palette: &'a mut [u8; 768],
    have_palette: &'a mut bool,
    palette_changed: &'a mut bool,
}

impl FrameSink for CursorSink<'_> {
    fn canvas(&mut self) -> &mut [u8] {
        self.canvas
    }
    fn color(&mut self, data: &[u8]) {
        // COLOR chunks are DELTAS over the installed palette (the
        // packet skips leave earlier entries alone), so ramp against
        // what is already there rather than starting from black.
        if apply_color_chunk(self.palette, data) {
            *self.have_palette = true;
            *self.palette_changed = true;
        }
    }
}

struct FirstColorSink<'a> {
    canvas: &'a mut [u8],
    palette: &'a mut Option<[u8; 768]>,
}

impl FrameSink for FirstColorSink<'_> {
    fn canvas(&mut self) -> &mut [u8] {
        self.canvas
    }
    fn color(&mut self, data: &[u8]) {
        if self.palette.is_none() {
            let mut pal = [0u8; 768];
            if apply_color_chunk(&mut pal, data) {
                *self.palette = Some(pal);
            }
        }
    }
}

/// Decode one FRAME chunk at `off`. Returns the offset just past it
/// and whether it was a pixel frame (0xF1FA) rather than a prefix.
fn decode_frame(
    file: &[u8],
    off: usize,
    width: usize,
    height: usize,
    sink: &mut dyn FrameSink,
) -> Result<(usize, bool), String> {
    // NOTE: the declared frame size is UNRELIABLE in the retail
    // SCREENS movies (GLOBE.DAT frame 0 declares 8400 bytes but its
    // chunks span 7716 — the next frame header sits right after the
    // chunks). The real extent is the 16-byte header plus the sum of
    // its sub-chunk sizes.
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
        // Prefix frames (0xF100: settings/celdata) carry no pixels —
        // retail skips them (DrawFrame's first arm).
        if ftype == FRAME_MAGIC {
            match ctype {
                // COLOR_256 / COLOR_64: the canvas stays 8bpp indices.
                4 | 11 => sink.color(data),
                // PSTAMP: preview thumbnail, no canvas effect.
                0x12 => {}
                7 => flc::delta_flc(sink.canvas(), width, height, data)
                    .map_err(|e| format!("fmv: SS2: {e}"))?,
                0xC => delta_fli(sink.canvas(), width, height, data)?,
                0xD => sink.canvas().fill(0),
                0xF => brun(sink.canvas(), width, height, data)?,
                0x10 => {
                    if data.len() < width * height {
                        return Err("fmv: COPY short".into());
                    }
                    sink.canvas()[..width * height].copy_from_slice(&data[..width * height]);
                }
                other => return Err(format!("fmv: unknown chunk {other:#x}")),
            }
        }
        coff += csize;
    }
    Ok((coff, ftype == FRAME_MAGIC))
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
    let (width, height, frame_count) = parse_header(file)?;
    let mut canvas = match seed {
        Some(s) if s.len() == width * height => s.to_vec(),
        Some(_) => return Err("fmv: seed does not match dimensions".into()),
        None => vec![0u8; width * height],
    };
    let mut frames = Vec::with_capacity(frame_count);
    let mut palette: Option<[u8; 768]> = None;
    let mut off = 12usize;
    while frames.len() < frame_count && off + 16 <= file.len() {
        let mut sink = FirstColorSink {
            canvas: &mut canvas,
            palette: &mut palette,
        };
        let (next_off, was_pixel_frame) = decode_frame(file, off, width, height, &mut sink)?;
        off = next_off;
        if was_pixel_frame {
            frames.push(canvas.clone());
        }
    }
    Ok(Fmv {
        width,
        height,
        frames,
        palette,
    })
}

/// Apply a `COLOR_256`/`COLOR_64` payload over `pal`: `u16 packets`,
/// each `{u8 skip, u8 count (0 = 256)}` + `count×3` RGB bytes. The
/// skips mean a packet only rewrites the entries it names, so this is
/// a delta over the installed palette — which is how the full-screen
/// movies fade. Returns false (leaving `pal` partly written) if the
/// payload is malformed.
fn apply_color_chunk(pal: &mut [u8; 768], data: &[u8]) -> bool {
    let Some(packets) = u16le(data, 0) else {
        return false;
    };
    let mut off = 2usize;
    let mut index = 0usize;
    for _ in 0..packets as usize {
        let (Some(skip), Some(raw_count)) = (data.get(off), data.get(off + 1)) else {
            return false;
        };
        index += *skip as usize;
        let count = match *raw_count as usize {
            0 => 256,
            n => n,
        };
        off += 2;
        for _ in 0..count {
            if index >= 256 {
                return false;
            }
            let Some(rgb) = data.get(off..off + 3) else {
                return false;
            };
            pal[index * 3..index * 3 + 3].copy_from_slice(rgb);
            index += 1;
            off += 3;
        }
    }
    true
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
            // The cursor must walk the same stream to the same place.
            let mut cur = FmvCursor::new(&file, Some(&bg)).expect("cursor opens");
            let mut n = 0usize;
            while cur.advance().expect("cursor advances") {
                assert_eq!(cur.canvas(), &fmv.frames[n][..], "{name} frame {n}");
                n += 1;
            }
            assert_eq!(n, fmv.frames.len(), "{name}: cursor frame count");
        }
    }

    /// Walk every full-screen movie in both installs with the incremental
    /// cursor. These are the streams the eager decoder cannot serve — MC1's
    /// `INTRO.DAT` alone would be ~200 MB of canvases — so the cursor is the
    /// only thing that ever reads them, and this is its only real exercise.
    /// Frame counts are header-verified against the pristine GOG ISOs.
    #[test]
    fn full_screen_movies_decode() {
        let found = crate::gamedata::Gamedata::locate(std::path::Path::new("../../gamedata"));
        let cases: [(_, &[(&str, usize)]); 2] = [
            (
                found.mc1,
                &[
                    ("INTRO/INTRO.DAT", 3165),
                    ("INTRO/OUTRO.DAT", 329),
                    ("INTRO/LEVELW1.DAT", 313),
                    ("INTRO/LEVELW2.DAT", 263),
                    ("INTRO/LEVELOSE.DAT", 160),
                    ("INTRO/LOGO.DAT", 91),
                    ("INTRO/INTEL.DAT", 41),
                    ("INTRO/TITLE-01.DAT", 150),
                    ("INTRO/TITLE-02.DAT", 4),
                    ("INTRO/TITLE-03.DAT", 150),
                    ("INTRO/TITLE-04.DAT", 4),
                ],
            ),
            (
                found.mc2,
                &[
                    ("INTRO/INTRO.DAT", 1276),
                    ("INTRO/INTRO2.DAT", 103),
                    ("INTRO/CUT1.DAT", 530),
                    ("INTRO/CUT2.DAT", 560),
                    ("INTRO/CUT3.DAT", 490),
                    ("INTRO/CUT4.DAT", 495),
                    ("INTRO/CUT5.DAT", 346),
                    ("INTRO/CUT6.DAT", 260),
                ],
            ),
        ];
        for (src, movies) in cases {
            let Some(src) = src else { continue };
            for (name, want_frames) in movies {
                let file = src.read(name).expect("movie readable");
                let mut cur = FmvCursor::new(&file, None).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!((cur.width(), cur.height()), (320, 200), "{name} dims");
                let mut lit = 0usize;
                while cur.advance().unwrap_or_else(|e| panic!("{name}: {e}")) {
                    if cur.canvas().iter().any(|&p| p != 0) {
                        lit += 1;
                    }
                }
                // Every retail stream runs to its full declared length —
                // none of them truncate.
                assert_eq!(cur.played(), *want_frames, "{name}: frames decoded");
                // ...and decodes to actual picture, not an all-black canvas
                // (a silently-wrong chunk handler would still "succeed").
                assert!(lit + 2 >= *want_frames, "{name}: only {lit} lit frames");
                // The movies carry their own palettes.
                assert!(cur.palette().is_some(), "{name}: no palette");
            }
        }
    }
}
