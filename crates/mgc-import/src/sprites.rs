//! World-sprite (billboard) baking: TMAPS archives in, one 8bpp atlas
//! plus a [`SpriteIndex`] out.
//!
//! TMAPS entries are the original engine's world sprite ids — creatures
//! (families of 8/16 rotation views, grouped by the TAB group field),
//! scenery, effects, water surfaces. Payload pixels are raw 8-bit
//! palette indices with **index 0 = transparent** (the engine's blitter
//! skips zero pixels); animated entries additionally carry an embedded
//! FLC delta stream ([`crate::flc`]), which we pre-decode into full
//! frames so the engine never sees the codec.
//!
//! Known retail data corruption: `TMAPS1-0` (MC1 arctic) entries 153
//! and 156 declare a 1x122 size but carry a neighbor's payload — an
//! original authoring bug. Such entries bake as frame-less placeholders
//! (ids must stay dense: they are the engine's sprite numbering).

use mgc_formats::bundle::{FramePos, SpriteEntry, SpriteIndex};

use crate::flc;
use crate::tmaps::{TmapsArchive, TmapsError};

/// One decoded sprite: all frames `width * height`, frame 0 the base.
pub struct DecodedSprite {
    pub index: usize,
    pub group: usize,
    pub flags: u16,
    pub width: u16,
    pub height: u16,
    pub frames: Vec<Vec<u8>>,
}

/// Decode every entry of a TMAPS archive. Undecodable entries (retail
/// corruption) become frame-less placeholders; a warning line per
/// occurrence is returned alongside.
pub fn decode_tmaps(
    archive: &TmapsArchive,
) -> Result<(Vec<DecodedSprite>, Vec<String>), TmapsError> {
    let mut sprites = Vec::with_capacity(archive.entries().len());
    let mut warnings = Vec::new();
    for entry in archive.entries() {
        let payload = archive.extract(*entry)?;
        // One-byte payloads are intentional placeholder slots for
        // sprites absent from an environment's set (MC2's night TMAPS
        // has runs of them) — ids must stay dense, so keep a frame-less
        // 0x0 entry without a warning.
        if payload.len() <= 1 {
            sprites.push(DecodedSprite {
                index: entry.index,
                group: entry.group,
                flags: 0,
                width: 0,
                height: 0,
                frames: Vec::new(),
            });
            continue;
        }
        if payload.len() < 6 {
            return Err(TmapsError::BadTexture(entry.index));
        }
        let flags = u16::from_le_bytes(payload[0..2].try_into().unwrap());
        let width = u16::from_le_bytes(payload[2..4].try_into().unwrap());
        let height = u16::from_le_bytes(payload[4..6].try_into().unwrap());
        let base_len = width as usize * height as usize;

        let mut sprite = DecodedSprite {
            index: entry.index,
            group: entry.group,
            flags,
            width,
            height,
            frames: Vec::new(),
        };

        let animated = flags & 1 != 0;
        if payload.len() == 6 + base_len && !animated {
            sprite.frames.push(payload[6..].to_vec());
        } else if animated && payload.len() > 6 + base_len {
            let base = &payload[6..6 + base_len];
            match flc::decode_frames(
                width as usize,
                height as usize,
                base,
                &payload[6 + base_len..],
            ) {
                Ok(frames) => sprite.frames = frames,
                Err(e) => {
                    warnings.push(format!(
                        "sprite {}: undecodable FLC stream ({e}) — baked frame-less",
                        entry.index
                    ));
                }
            }
        } else {
            // Known retail instances (EXPECTED on every full bake, not
            // a bake failure): MC1 TMAPS1-0 (arctic) entries 153 and
            // 156 ship with a mangled header (`flags 2, 1x122`) over a
            // byte-perfect duplicate of the PREVIOUS entry's pixels
            // (152's 90x65 / 155's 40x39) — an authoring bug in the
            // shipped data; the original engine read the same garbage
            // header, so frame-less is already generous.
            warnings.push(format!(
                "sprite {}: {}x{} with {}-byte payload (flags {:#06x}) — corrupt entry, baked frame-less",
                entry.index,
                width,
                height,
                payload.len(),
                flags
            ));
        }
        sprites.push(sprite);
    }
    Ok((sprites, warnings))
}

/// A packed sprite atlas: 8bpp palette indices, plus the index that
/// locates every frame.
pub struct PackedSprites {
    pub atlas: Vec<u8>,
    pub index: SpriteIndex,
}

/// Pack all frames into one atlas of the given width using shelf
/// packing in entry order (deterministic). Frames taller than a shelf
/// open a new shelf; frames wider than the atlas are rejected by
/// construction (retail maxima are far below 1024).
pub fn pack(sprites: &[DecodedSprite], atlas_width: u32) -> PackedSprites {
    struct Shelf {
        y: u32,
        height: u32,
        x: u32,
    }
    let mut shelves: Vec<Shelf> = Vec::new();
    let mut atlas_height = 0u32;
    let mut entries = Vec::with_capacity(sprites.len());
    let mut placements: Vec<(usize, u32, u32)> = Vec::new(); // (sprite idx, x, y) per frame, flattened

    for (si, sprite) in sprites.iter().enumerate() {
        let (w, h) = (sprite.width as u32, sprite.height as u32);
        assert!(w <= atlas_width, "sprite {} wider than atlas", sprite.index);
        let mut frames = Vec::with_capacity(sprite.frames.len());
        for _ in &sprite.frames {
            // Find the first shelf that fits; open a new one otherwise.
            let shelf = match shelves
                .iter_mut()
                .find(|s| s.height >= h && s.x + w <= atlas_width)
            {
                Some(s) => s,
                None => {
                    shelves.push(Shelf {
                        y: atlas_height,
                        height: h,
                        x: 0,
                    });
                    atlas_height += h;
                    shelves.last_mut().unwrap()
                }
            };
            let pos = FramePos {
                x: shelf.x,
                y: shelf.y,
            };
            placements.push((si, pos.x, pos.y));
            shelf.x += w;
            frames.push(pos);
        }
        entries.push(SpriteEntry {
            id: sprite.index as u32,
            group: sprite.group as u32,
            width: sprite.width,
            height: sprite.height,
            flags: sprite.flags,
            frames,
        });
    }

    let mut atlas = vec![0u8; (atlas_width * atlas_height) as usize];
    let mut frame_iter = placements.into_iter();
    for sprite in sprites {
        let w = sprite.width as usize;
        for frame in &sprite.frames {
            let (_, x, y) = frame_iter.next().unwrap();
            for (row_idx, row) in frame.chunks_exact(w.max(1)).enumerate() {
                let dst = (y as usize + row_idx) * atlas_width as usize + x as usize;
                atlas[dst..dst + w].copy_from_slice(row);
            }
        }
    }

    PackedSprites {
        atlas,
        index: SpriteIndex {
            atlas_width,
            atlas_height,
            sprites: entries,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(index: usize, w: u16, h: u16, frames: usize) -> DecodedSprite {
        DecodedSprite {
            index,
            group: index,
            flags: 0,
            width: w,
            height: h,
            frames: (0..frames)
                .map(|f| vec![(index * 16 + f) as u8; w as usize * h as usize])
                .collect(),
        }
    }

    #[test]
    fn packs_shelves_deterministically() {
        let sprites = vec![sprite(0, 4, 4, 2), sprite(1, 3, 2, 1), sprite(2, 6, 4, 1)];
        let packed = pack(&sprites, 8);
        let idx = &packed.index;
        // Sprite 0's two frames fill the first shelf.
        assert_eq!(
            idx.sprites[0].frames,
            vec![FramePos { x: 0, y: 0 }, FramePos { x: 4, y: 0 }]
        );
        // Sprite 1 fits on the same shelf? No: shelf 0 is full (x=8).
        // It opens shelf 1 (height 2).
        assert_eq!(idx.sprites[1].frames, vec![FramePos { x: 0, y: 4 }]);
        // Sprite 2 (6x4) needs a taller shelf.
        assert_eq!(idx.sprites[2].frames, vec![FramePos { x: 0, y: 6 }]);
        assert_eq!(idx.atlas_height, 10);
        // Pixel content lands where the index says.
        let at = |x: u32, y: u32| packed.atlas[(y * 8 + x) as usize];
        assert_eq!(at(0, 0), 0);
        assert_eq!(at(4, 0), 1); // sprite 0 frame 1
        assert_eq!(at(0, 4), 16); // sprite 1
        assert_eq!(at(5, 6), 32); // sprite 2
    }

    #[test]
    fn frameless_entries_keep_ids_dense() {
        let mut broken = sprite(1, 5, 5, 0);
        broken.frames.clear();
        let sprites = vec![sprite(0, 2, 2, 1), broken, sprite(2, 2, 2, 1)];
        let packed = pack(&sprites, 8);
        assert_eq!(packed.index.sprites.len(), 3);
        assert!(packed.index.sprites[1].frames.is_empty());
        assert_eq!(packed.index.sprites[2].id, 2);
    }
}
