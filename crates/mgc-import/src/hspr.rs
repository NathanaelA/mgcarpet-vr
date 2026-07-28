//! HSPR/MSPR UI-sprite libraries (2D screen-space sprites: spell
//! icons, HUD panel, mana-bar frames, level pips, map markers).
//!
//! Format (TAB/DAT pair is self-describing, no remc1 reader needed):
//! both files are whole-file RNC (decompressed by the caller); TAB =
//! 6-byte entries
//! `{u32 offset, u8 width, u8 height}` into the DAT; pixel payload is
//! signed-RLE per row — `n > 0` copy `n` palette-index bytes, `n < 0`
//! skip `-n` transparent pixels, `n == 0` end of row. Index 0 =
//! transparent, as everywhere else in the engine's sprite paths.
//!
//! HSPR is the 640x480 set (spell icons 62x34); MSPR is its half-size
//! 320x200 twin. Entry map (87 entries, MC1): 1/2 = LMB/RMB slot
//! highlights, 3/4 = slot backgrounds, 6..=29 = the 24 spell icons
//! keyed by INTERNAL spell type (remc1 begSprTab[type + 6], drawn at
//! sub_main.cpp:27700), 40 = HUD panel, 41/42 = mana-bar frames,
//! 43..=52 = level pips, 83/84 = the advertised-trigger map X-markers
//! (sub_main.cpp:57386).

use crate::sprites::DecodedSprite;

#[derive(Debug)]
pub enum HsprError {
    /// TAB length is not a multiple of 6.
    BadTab(usize),
    /// RLE run walked outside the row or the DAT (entry index).
    BadRle(usize),
}

impl std::fmt::Display for HsprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HsprError::BadTab(n) => write!(f, "HSPR TAB is {n} bytes (not 6-byte entries)"),
            HsprError::BadRle(i) => write!(f, "HSPR entry {i}: RLE overruns row or data"),
        }
    }
}

/// Decode a whole HSPR/MSPR library into the same shape the TMAPS
/// world sprites use, so [`crate::sprites::pack`] and the bundle
/// `SpriteIndex` schema apply unchanged (single frame per sprite,
/// `group == index`).
pub fn decode(dat: &[u8], tab: &[u8]) -> Result<Vec<DecodedSprite>, HsprError> {
    decode_opaque(dat, tab, 0)
}

/// [`decode`], but with the black-shape rescue: a sprite whose ONLY
/// drawn (literal) pixels are palette index 0 — a pure-black shape, the
/// advertised-trigger map X marker (HSPR 83) being the sole MC1 case —
/// has those literal-0 pixels remapped to `opaque_black`. The engine's
/// blit draws literal pixels regardless of value and takes transparency
/// only from the RLE's skip runs; our atlas composite instead keys
/// transparency off index 0, so a black-in-0 shape would vanish
/// entirely. Moving just the DRAWN index-0 pixels to an opaque black
/// index (the skip runs stay transparent) preserves the shape. Passing
/// `opaque_black == 0` keeps the raw decode (callers that don't need the
/// rescue, e.g. fonts, and games whose markers are pre-coloured).
pub fn decode_opaque(
    dat: &[u8],
    tab: &[u8],
    opaque_black: u8,
) -> Result<Vec<DecodedSprite>, HsprError> {
    if tab.len() % 6 != 0 {
        return Err(HsprError::BadTab(tab.len()));
    }
    let mut sprites = Vec::with_capacity(tab.len() / 6);
    for (i, e) in tab.chunks_exact(6).enumerate() {
        let offset = u32::from_le_bytes(e[0..4].try_into().unwrap()) as usize;
        let (w, h) = (e[4] as usize, e[5] as usize);
        if w == 0 || h == 0 {
            // Entry 0 is a null slot; keep ids dense like TMAPS bakes.
            sprites.push(DecodedSprite {
                index: i,
                group: i,
                flags: 0,
                width: 0,
                height: 0,
                frames: Vec::new(),
            });
            continue;
        }
        let mut pixels = vec![0u8; w * h];
        // The black-shape rescue (see `decode_opaque`): remember which
        // drawn pixels landed on index 0, and whether the sprite drew
        // any non-zero (coloured) pixel at all.
        let mut lit0_pos: Vec<usize> = Vec::new();
        let mut has_colour = false;
        let (mut p, mut row, mut col) = (offset, 0usize, 0usize);
        while row < h {
            let n = *dat.get(p).ok_or(HsprError::BadRle(i))? as i8;
            p += 1;
            if n == 0 {
                row += 1;
                col = 0;
            } else if n > 0 {
                let n = n as usize;
                if col + n > w || p + n > dat.len() {
                    return Err(HsprError::BadRle(i));
                }
                let base = row * w + col;
                pixels[base..base + n].copy_from_slice(&dat[p..p + n]);
                for (k, &v) in dat[p..p + n].iter().enumerate() {
                    if v == 0 {
                        lit0_pos.push(base + k);
                    } else {
                        has_colour = true;
                    }
                }
                p += n;
                col += n;
            } else {
                col += (-(n as isize)) as usize;
                if col > w {
                    return Err(HsprError::BadRle(i));
                }
            }
        }
        // A pure-black shape (drew index 0 and nothing else): move its
        // drawn pixels off the transparent index so the composite keeps
        // them. Skip-run pixels stayed 0 and remain transparent.
        if opaque_black != 0 && !has_colour && !lit0_pos.is_empty() {
            for pos in lit0_pos {
                pixels[pos] = opaque_black;
            }
        }
        sprites.push(DecodedSprite {
            index: i,
            group: i,
            flags: 0,
            width: w as u16,
            height: h as u16,
            frames: vec![pixels],
        });
    }
    Ok(sprites)
}
