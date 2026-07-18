//! HSCREEN0.DAT — MC2's monolithic frontend-screen blob
//! (`DATA/SCREENS/HSCREEN0.DAT`, the only file in SCREENS/): every
//! menu/map screen asset lives at a hard byte offset inside it,
//! fetched by `sub_7AA70_load_and_decompres_dat_file(path, dest,
//! position, length)` (remc2 EF:46290) — seek, read `length` bytes,
//! then RNC-decompress iff the chunk starts with `RNC\x01`, else use
//! raw. The world-map screen is `case 6` of `sub_7A110_load_hscreen`
//! (EF:46108-180); its chunk table is transcribed below
//! (docs/traces/mc2-campaign-save-menu.md, asset recon).
//!
//! The sprite bank inside it uses the same 6-byte TAB records as
//! HSPR (`{u32 offset, u8 w, u8 h}`, bitmap_pos_struct.h:27) — the
//! payload encoding is pinned by the `worldmap_chunks_decode` test
//! against the pristine install.

use crate::rnc;
use crate::sprites::DecodedSprite;

/// World-map background: 1280×960, 8bpp palette indices.
pub const WORLDMAP_W: usize = 1280;
pub const WORLDMAP_H: usize = 960;
/// Frontend screens (main menu bg, map border frame): 640×480.
pub const SCREEN_W: usize = 640;
pub const SCREEN_H: usize = 480;

// The `case 6` chunk table (EF:46108-180): (offset, on-disk length).
const BG: (usize, usize) = (0xB2C47, 0x87D83);
const PALETTE: (usize, usize) = (0x13A9CA, 768);
const SPRITE_POOL: (usize, usize) = (0x783BD, 103_577);
const SPRITE_INDEX: (usize, usize) = (0x91856, 1027);
/// The map-screen ornate border frame (`x_DWORD_17DE5C_border_bitmap`,
/// EF:46150) — a quadrant-RLE stream, decoded by [`border_frame`].
const BORDER: (usize, usize) = (0x141E85, 13_195);

// The `case 4` (main menu) chunk table: the loader's file cursor is
// `position + length` of the previous read (EF sub_7AA70), so the
// chunks are SEQUENTIAL from 0: palette (768 raw), background
// (RNC → 640×480), sprite pool, sprite TAB.
const MENU_PALETTE: (usize, usize) = (0, 768);
const MENU_BG: (usize, usize) = (768, 168_081);
const MENU_SPRITE_POOL: (usize, usize) = (168_849, 102_213);
const MENU_SPRITE_INDEX: (usize, usize) = (271_062, 411);

/// The decoded world-map screen assets.
pub struct WorldMap {
    /// 1280×960 8bpp row-major palette indices.
    pub bg: Vec<u8>,
    /// 256×RGB, 6-bit VGA components (0-63) as shipped — scale ×4
    /// for 8-bit channels.
    pub palette: [u8; 768],
    /// The 313-sprite frontend bank (portals, flags, cursor, logo),
    /// in the TMAPS `DecodedSprite` shape so `sprites::pack` applies.
    pub sprites: Vec<DecodedSprite>,
}

fn chunk(file: &[u8], (off, len): (usize, usize), what: &str) -> Result<Vec<u8>, String> {
    let raw = file
        .get(off..off + len)
        .ok_or_else(|| format!("HSCREEN0: {what} chunk {off:#x}+{len:#x} out of bounds"))?;
    if rnc::is_rnc(raw) {
        rnc::decompress(raw).map_err(|e| format!("HSCREEN0: {what}: RNC: {e:?}"))
    } else {
        Ok(raw.to_vec())
    }
}

/// The decoded main-menu screen (`case 4`): the 640×480 temple
/// background, its own palette, and the 111-sprite menu bank
/// (buttons grey/gold, fires/incense anims, cursor 39, scroll-dialog
/// parchment).
pub struct MainMenu {
    /// 640×480 8bpp row-major palette indices.
    pub bg: Vec<u8>,
    /// 256×RGB 6-bit components (case 4 has its OWN palette).
    pub palette: [u8; 768],
    pub sprites: Vec<DecodedSprite>,
}

/// Decode the main-menu screen (`case 4`) out of the raw HSCREEN0
/// file bytes.
pub fn mainmenu(file: &[u8]) -> Result<MainMenu, String> {
    let bg = chunk(file, MENU_BG, "main-menu background")?;
    if bg.len() != SCREEN_W * SCREEN_H {
        return Err(format!(
            "HSCREEN0: menu background decoded to {} bytes (want {})",
            bg.len(),
            SCREEN_W * SCREEN_H
        ));
    }
    let pal = chunk(file, MENU_PALETTE, "menu palette")?;
    let palette: [u8; 768] = pal
        .try_into()
        .map_err(|_| "HSCREEN0: menu palette chunk is not 768 bytes".to_string())?;
    let pool = chunk(file, MENU_SPRITE_POOL, "menu sprite pool")?;
    let index = chunk(file, MENU_SPRITE_INDEX, "menu sprite index")?;
    let sprites = crate::hspr::decode(&pool, &index)
        .map_err(|e| format!("HSCREEN0: menu sprite bank: {e}"))?;
    Ok(MainMenu {
        bg,
        palette,
        sprites,
    })
}

/// Decode the map screen's ornate border frame into a 640×480 8bpp
/// overlay (0 = transparent) — a verbatim port of retail
/// `sub_85CC3_draw_round_frame` (EF:47713) writing into a cleared
/// buffer instead of the live screen.
///
/// Stream law: i16 LE tokens describing the TOP-LEFT QUADRANT, 240
/// rows; each literal run is mirrored into all four quadrants
/// (horizontal mirror reversed in place, vertical mirror plain);
/// negative = transparent skip, zero = end of row. Rows while the
/// row counter exceeds 221 (the first 19) duplicate one trailing
/// byte (the retail `a2 > 221` arm). The retail routine's pointer
/// bookkeeping (including its quirky `v20` skip accounting) is kept
/// exactly; writes are bounds-checked because retail provably
/// overruns its buffer by design (the first BR mirror byte lands at
/// index 307200).
pub fn border_frame(file: &[u8]) -> Result<Vec<u8>, String> {
    let stream = chunk(file, BORDER, "border frame")?;
    let mut dest = vec![0u8; SCREEN_W * SCREEN_H];
    let put = |idx: isize, v: u8, dest: &mut Vec<u8>| {
        if let Ok(i) = usize::try_from(idx)
            && i < dest.len()
        {
            dest[i] = v;
        }
    };
    let mut s = 0usize; // stream byte cursor (v3)
    let read_i16 = |s: &mut usize| -> Result<i16, String> {
        let v = stream
            .get(*s..*s + 2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .ok_or("HSCREEN0: border stream truncated")?;
        *s += 2;
        Ok(v)
    };
    let mut v4: isize = 0; // top-left
    let mut v19: isize = (SCREEN_W * (SCREEN_H - 1)) as isize; // bottom-left
    let mut v17: isize = SCREEN_W as isize; // top-right (writes decrementing)
    let mut v18: isize = (SCREEN_W * SCREEN_H) as isize; // bottom-right
    let mut v20: isize = 0;
    let mut a1: isize = 0;
    let mut a2 = 240;
    loop {
        loop {
            let v5 = read_i16(&mut s)?;
            if v5 > 0 {
                a1 = v5 as isize;
                for k in 0..a1 {
                    let b = *stream
                        .get(s + k as usize)
                        .ok_or("HSCREEN0: border literal truncated")?;
                    put(v4 + k, b, &mut dest);
                    put(v19 + k, b, &mut dest);
                    put(v17 - k, b, &mut dest);
                    put(v18 - k, b, &mut dest);
                }
                s += a1 as usize;
                v4 += a1;
                v19 += a1;
                v17 -= a1;
                v18 -= a1;
                continue;
            }
            if v5 == 0 {
                break;
            }
            let skip = -(v5 as isize);
            v4 += skip;
            v20 += a1 + skip;
            v19 += skip;
            v17 -= skip;
            v18 -= skip;
        }
        if a2 > 221 {
            // The retail trailing-byte duplication (reads 3 bytes
            // back in the stream, no cursor advance).
            if s >= 3 {
                let b = stream[s - 3];
                put(v4, b, &mut dest);
                put(v19, b, &mut dest);
            }
        }
        v4 += -a1 + SCREEN_W as isize - v20;
        v19 += -a1 - SCREEN_W as isize - v20;
        v17 += v20 + a1 + SCREEN_W as isize;
        v18 += v20 + a1 - SCREEN_W as isize;
        v20 = 0;
        a2 -= 1;
        if a2 == 0 {
            break;
        }
    }
    Ok(dest)
}

/// Decode the world-map screen (`case 6`) out of the raw HSCREEN0
/// file bytes.
pub fn worldmap(file: &[u8]) -> Result<WorldMap, String> {
    let bg = chunk(file, BG, "world-map background")?;
    if bg.len() != WORLDMAP_W * WORLDMAP_H {
        return Err(format!(
            "HSCREEN0: background decoded to {} bytes (want {})",
            bg.len(),
            WORLDMAP_W * WORLDMAP_H
        ));
    }
    let pal = chunk(file, PALETTE, "palette")?;
    let palette: [u8; 768] = pal
        .try_into()
        .map_err(|_| "HSCREEN0: palette chunk is not 768 bytes".to_string())?;
    let pool = chunk(file, SPRITE_POOL, "sprite pool")?;
    let index = chunk(file, SPRITE_INDEX, "sprite index")?;
    // The payload is the HSPR signed-RLE row encoding (empirically
    // pinned by the test below: a raw w*h read overruns the pool at
    // the bank's tail, the RLE walk decodes all 313 cleanly).
    let sprites =
        crate::hspr::decode(&pool, &index).map_err(|e| format!("HSCREEN0: sprite bank: {e}"))?;
    Ok(WorldMap {
        bg,
        palette,
        sprites,
    })
}

/// The frontend language file (`LANGUAGE/L%d.TXT`): a 4785-byte
/// header, then NUL-separated strings the engine indexes 0..471
/// (retail `InitLanguage_76A40` MI:646-714 + `sub_5B870` EF:42829).
/// Level descriptions live at 23+level; "Empty" = 414.
pub const LANGUAGE_HEADER: usize = 4785;
pub const LANGUAGE_COUNT: usize = 471;

pub fn language_strings(raw: &[u8]) -> Vec<String> {
    let body = raw.get(LANGUAGE_HEADER..).unwrap_or(&[]);
    let mut out = Vec::with_capacity(LANGUAGE_COUNT);
    let mut start = 0usize;
    for (i, &b) in body.iter().enumerate() {
        if b == 0 {
            out.push(String::from_utf8_lossy(&body[start..i]).into_owned());
            start = i + 1;
            if out.len() >= LANGUAGE_COUNT {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the chunk table + payload encoding against the pristine
    /// install (skips silently when gamedata is absent, like the
    /// other source-gated tests).
    #[test]
    fn worldmap_chunks_decode() {
        let found = crate::gamedata::Gamedata::locate(std::path::Path::new("../../gamedata"));
        let Some(src) = found.mc2 else { return };
        let file = src
            .read("DATA/SCREENS/HSCREEN0.DAT")
            .expect("HSCREEN0.DAT readable");
        assert_eq!(file.len(), 1_557_528, "HSCREEN0.DAT size");
        let wm = worldmap(&file).expect("world-map chunks decode");
        assert_eq!(wm.bg.len(), 1280 * 960);
        // 6-bit VGA palette components.
        assert!(wm.palette.iter().all(|&c| c < 64), "palette is 6-bit");
        assert_eq!(wm.sprites.len(), 313, "map sprite bank entry count");
        // The known portal sprites exist and have sane sizes.
        for idx in [33, 37, 43, 70, 83, 270, 272, 305, 311, 39, 66] {
            let s = &wm.sprites[idx];
            assert!(
                s.width > 0 && s.height > 0,
                "sprite {idx} is non-empty ({}x{})",
                s.width,
                s.height
            );
        }
        // The corner-button pairs (grey/gold 246-253) too — the map
        // border overlay's art.
        for idx in 246..=253 {
            let s = &wm.sprites[idx];
            assert!(s.width > 0 && s.height > 0, "button sprite {idx} present");
        }
    }

    /// The main-menu (case 4) chunk table + the border-frame RLE
    /// against the pristine install.
    #[test]
    fn mainmenu_and_border_decode() {
        let found = crate::gamedata::Gamedata::locate(std::path::Path::new("../../gamedata"));
        let Some(src) = found.mc2 else { return };
        let file = src
            .read("DATA/SCREENS/HSCREEN0.DAT")
            .expect("HSCREEN0.DAT readable");
        let menu = mainmenu(&file).expect("main-menu chunks decode");
        assert_eq!(menu.bg.len(), 640 * 480);
        assert!(menu.palette.iter().all(|&c| c < 64), "palette is 6-bit");
        assert_eq!(menu.sprites.len(), 111, "menu sprite bank entry count");
        // Button art (grey 51-58 / gold 59-66) + cursor 39 present.
        for idx in [39, 51, 58, 59, 66, 106] {
            let s = &menu.sprites[idx];
            assert!(s.width > 0 && s.height > 0, "menu sprite {idx} present");
        }
        // The border frame: 640×480 with a frame shape — dense ink
        // along the screen edges, empty center (the map shows
        // through). Exact pixel symmetry is NOT asserted: the retail
        // routine's bookkeeping (trailing-byte arm, off-by-one
        // mirror axis) makes the quadrants near- but not perfectly
        // symmetric, faithfully reproduced.
        let b = border_frame(&file).expect("border decodes");
        assert_eq!(b.len(), 640 * 480);
        let nz = b.iter().filter(|&&p| p != 0).count();
        assert!(
            (80_000..120_000).contains(&nz),
            "frame ink density plausible ({nz})"
        );
        let row0 = b[..640].iter().filter(|&&p| p != 0).count();
        assert!(row0 > 600, "top edge is solid frame ({row0}/640)");
        let center = (200..280)
            .flat_map(|y| (220..420).map(move |x| y * 640 + x))
            .filter(|&i| b[i] != 0)
            .count();
        assert_eq!(center, 0, "frame center is transparent");
    }

    /// Language-file parsing: header skip + NUL-splitting, pinned on
    /// the real English file's known entries.
    #[test]
    fn language_strings_decode() {
        let found = crate::gamedata::Gamedata::locate(std::path::Path::new("../../gamedata"));
        let Some(src) = found.mc2 else { return };
        let raw = src.read("LANGUAGE/L2.TXT").expect("L2.TXT readable");
        let strings = language_strings(&raw);
        assert_eq!(strings.len(), LANGUAGE_COUNT);
        // Entry 414 = the empty-slot label (retail :2543).
        assert!(
            strings[414].to_uppercase().contains("EMPTY"),
            "entry 414 is the Empty label, got {:?}",
            strings[414]
        );
        // Level-description entries are long-form sentences.
        assert!(
            strings[23].len() > 40,
            "entry 23 (level 0 description) is prose, got {:?}",
            strings[23]
        );
    }
}
