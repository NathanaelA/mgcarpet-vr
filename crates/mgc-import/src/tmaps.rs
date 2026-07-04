//! TMAPS archives: sprite/billboard/animated texture maps (water
//! surfaces, overlays, object textures — NOT the terrain ground tiles,
//! which come from the BLK/BLOCK atlases; see `bake::bake_mc1_assets`).
//!
//! Unlike the plain-offset TABs handled by [`crate::dattab`], TMAPS TABs
//! use 10-byte entries — remc2 `Basic.h` `type_TMAPS00TAB_BEGIN_BUFFER`,
//! reading confirmed against retail data (MC1 `TMAPS*.TAB` = 530
//! entries, MC2 `TMAPS*-0.TAB` = 505, each including the sentinel):
//!
//! ```text
//! offset  size  meaning
//!   0      u32  decompressed entry size (== the RNC header's unpacked
//!               length on every retail entry)
//!   4      u32  byte offset of the entry in the DAT
//!   8      u16  group id: index of the first entry of the animation
//!               group this entry belongs to (frames of one animated
//!               texture are consecutive entries sharing the value)
//! ```
//!
//! Entry `i`'s compressed size is `offset[i+1] - offset[i]` (remc2
//! `TextureMaps.cpp` `sub_70C60_decompress_tmap`); the final TAB entry
//! is a sentinel whose offset equals the DAT size. The DAT opens with
//! the 8-byte `BULLFROG` magic (first entry offset is 8) and entries are
//! individually RNC-compressed. Decompressed payload layout (remc2
//! `engine_support.h` `type_particle_str`): `{u16 flags, u16 width,
//! u16 height}` then `width*height` 8-bit palette indices, row-major.
//! Flag bit 0 = animated; remc2 additionally marks water/lava cycling
//! groups (MC2 ids 311, 481–487, 496) with bit 5 at load time.
//!
//! File-name suffix scheme: MC1 `TMAPS{0,1}-0` = world tileset 0
//! (temperate) / 1 (arctic) (`TMAPS.DAT` is a byte-identical copy of
//! set 0); MC2 `TMAPS{0,1,2}-0` = day/night/cave environments.

use crate::rnc;

/// Size of one TAB record in bytes.
const ENTRY_SIZE: usize = 10;

#[derive(Debug, PartialEq, Eq)]
pub enum TmapsError {
    /// TAB length is not a non-zero multiple of the 10-byte record size.
    BadLength(usize),
    /// Offsets not monotonic, beyond the DAT, or a bad sentinel.
    BadOffsets,
    /// A group-head field points at itself inconsistently or forward.
    BadGroup(usize),
    /// RNC decompression of an entry failed.
    Rnc(rnc::RncError),
    /// Decompressed size differs from the TAB's declared size.
    SizeMismatch { index: usize, tab: u32, got: usize },
    /// Payload too short for its header, or pixels != width * height.
    BadTexture(usize),
}

impl std::fmt::Display for TmapsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadLength(n) => write!(f, "TAB length {n} is not a multiple of 10"),
            Self::BadOffsets => write!(f, "TAB offsets not monotonic or beyond DAT end"),
            Self::BadGroup(i) => write!(f, "entry {i}: group head is not a preceding index"),
            Self::Rnc(e) => write!(f, "entry decompression: {e}"),
            Self::SizeMismatch { index, tab, got } => {
                write!(f, "entry {index}: TAB says {tab} bytes, RNC yields {got}")
            }
            Self::BadTexture(i) => write!(f, "entry {i}: malformed texture payload"),
        }
    }
}

impl std::error::Error for TmapsError {}

impl From<rnc::RncError> for TmapsError {
    fn from(e: rnc::RncError) -> Self {
        Self::Rnc(e)
    }
}

/// One TMAPS archive member (the sentinel record is not materialized).
#[derive(Debug, Clone, Copy)]
pub struct TmapsEntry {
    pub index: usize,
    pub offset: u32,
    /// Stored (compressed) length, from the offset delta to the next entry.
    pub len: u32,
    /// Decompressed size declared by the TAB.
    pub unpacked_len: u32,
    /// Index of the first entry of this entry's group (== `index` for a
    /// group head).
    pub group: usize,
}

pub struct TmapsArchive {
    dat: Vec<u8>,
    entries: Vec<TmapsEntry>,
}

impl TmapsArchive {
    pub fn open(dat: &[u8], tab: &[u8]) -> Result<Self, TmapsError> {
        if tab.is_empty() || tab.len() % ENTRY_SIZE != 0 {
            return Err(TmapsError::BadLength(tab.len()));
        }
        let records: Vec<(u32, u32, u16)> = tab
            .chunks_exact(ENTRY_SIZE)
            .map(|r| {
                (
                    u32::from_le_bytes(r[0..4].try_into().unwrap()),
                    u32::from_le_bytes(r[4..8].try_into().unwrap()),
                    u16::from_le_bytes(r[8..10].try_into().unwrap()),
                )
            })
            .collect();

        // The last record is a sentinel: offset == DAT size, size 0.
        let dat_len = dat.len() as u32;
        let mut entries = Vec::with_capacity(records.len().saturating_sub(1));
        for (index, pair) in records.windows(2).enumerate() {
            let (unpacked_len, offset, group) = pair[0];
            let next_offset = pair[1].1;
            if next_offset < offset || next_offset > dat_len {
                return Err(TmapsError::BadOffsets);
            }
            let group = group as usize;
            // A head points at itself; members point backward at their head.
            if group > index
                || (group < index
                    && entries[group..]
                        .iter()
                        .any(|e: &TmapsEntry| e.group != group))
            {
                return Err(TmapsError::BadGroup(index));
            }
            entries.push(TmapsEntry {
                index,
                offset,
                len: next_offset - offset,
                unpacked_len,
                group,
            });
        }
        if records.last().map(|r| r.1) != Some(dat_len) {
            return Err(TmapsError::BadOffsets);
        }

        Ok(Self {
            dat: dat.to_vec(),
            entries,
        })
    }

    pub fn entries(&self) -> &[TmapsEntry] {
        &self.entries
    }

    /// Decompressed payload of one entry, size-checked against the TAB.
    pub fn extract(&self, entry: TmapsEntry) -> Result<Vec<u8>, TmapsError> {
        let raw = &self.dat[entry.offset as usize..(entry.offset + entry.len) as usize];
        let out = if rnc::is_rnc(raw) {
            rnc::decompress(raw)?
        } else {
            raw.to_vec()
        };
        if out.len() != entry.unpacked_len as usize {
            return Err(TmapsError::SizeMismatch {
                index: entry.index,
                tab: entry.unpacked_len,
                got: out.len(),
            });
        }
        Ok(out)
    }

    /// Extract and decode one entry as a texture.
    pub fn texture(&self, entry: TmapsEntry) -> Result<TmapsTexture, TmapsError> {
        TmapsTexture::parse(entry.index, self.extract(entry)?)
    }
}

/// A decoded TMAPS texture: 8-bit palette-index pixels, row-major.
pub struct TmapsTexture {
    /// Bit 0 = animated (frames follow in the same TAB group).
    pub flags: u16,
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

impl TmapsTexture {
    fn parse(index: usize, payload: Vec<u8>) -> Result<Self, TmapsError> {
        if payload.len() < 6 {
            return Err(TmapsError::BadTexture(index));
        }
        let flags = u16::from_le_bytes(payload[0..2].try_into().unwrap());
        let width = u16::from_le_bytes(payload[2..4].try_into().unwrap());
        let height = u16::from_le_bytes(payload[4..6].try_into().unwrap());
        if payload.len() != 6 + width as usize * height as usize {
            return Err(TmapsError::BadTexture(index));
        }
        Ok(Self {
            flags,
            width,
            height,
            pixels: payload[6..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(unpacked: u32, offset: u32, group: u16) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&unpacked.to_le_bytes());
        v.extend_from_slice(&offset.to_le_bytes());
        v.extend_from_slice(&group.to_le_bytes());
        v
    }

    #[test]
    fn parses_entries_and_sentinel() {
        // "BULLFROG" + two raw entries of 3 and 2 bytes.
        let dat = b"BULLFROGaaabb";
        let tab: Vec<u8> = [rec(3, 8, 0), rec(2, 11, 0), rec(0, 13, 0)].concat();
        let a = TmapsArchive::open(dat, &tab).unwrap();
        assert_eq!(a.entries().len(), 2);
        assert_eq!(a.extract(a.entries()[0]).unwrap(), b"aaa");
        assert_eq!(a.extract(a.entries()[1]).unwrap(), b"bb");
    }

    #[test]
    fn group_heads_run_backward_only() {
        let dat = b"BULLFROGabcd";
        // Entries 1 and 2 belong to the group headed at 1.
        let tab: Vec<u8> = [
            rec(1, 8, 0),
            rec(1, 9, 1),
            rec(1, 10, 1),
            rec(1, 11, 3),
            rec(0, 12, 0),
        ]
        .concat();
        let a = TmapsArchive::open(dat, &tab).unwrap();
        assert_eq!(
            a.entries().iter().map(|e| e.group).collect::<Vec<_>>(),
            vec![0, 1, 1, 3]
        );
        // A forward-pointing group head is rejected.
        let bad: Vec<u8> = [rec(1, 8, 2), rec(0, 9, 0)].concat();
        assert_eq!(
            TmapsArchive::open(b"BULLFROGa", &bad).err().unwrap(),
            TmapsError::BadGroup(0)
        );
    }

    #[test]
    fn rejects_size_mismatch() {
        let dat = b"BULLFROGaaa";
        let tab: Vec<u8> = [rec(99, 8, 0), rec(0, 11, 0)].concat();
        let a = TmapsArchive::open(dat, &tab).unwrap();
        assert!(matches!(
            a.extract(a.entries()[0]),
            Err(TmapsError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn rejects_bad_lengths_and_offsets() {
        assert_eq!(
            TmapsArchive::open(b"x", &[0; 7]).err().unwrap(),
            TmapsError::BadLength(7)
        );
        // Sentinel offset must equal the DAT size.
        let tab: Vec<u8> = [rec(1, 8, 0), rec(0, 10, 0)].concat();
        assert_eq!(
            TmapsArchive::open(b"BULLFROGa", &tab).err().unwrap(),
            TmapsError::BadOffsets
        );
    }
}
