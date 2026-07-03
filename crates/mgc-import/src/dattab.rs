//! Bullfrog DAT/TAB archives: a `.DAT` blob indexed by a `.TAB` file of
//! offsets.
//!
//! This module handles the plain-offset variant: the TAB is an array of
//! little-endian u32 byte offsets into the DAT, entry `i` spanning
//! `tab[i]..tab[i+1]` (confirmed against remc2's `LevelDecompress_533B0`,
//! which does exactly that seek/read). Conventions observed in real data:
//!
//! - DATs may open with an 8-byte `BULLFROG` magic; the first offset
//!   simply starts past it. LEVELS.TAB is 4000 bytes = 1000 offsets.
//! - Unused trailing TAB entries are filled with the DAT's total size,
//!   so the delta rule uniformly yields zero-length (empty) entries.
//! - Either file may itself be whole-file RNC-compressed (MC1's SNDS
//!   sets): decompress both first, then index.
//! - Individual entries are often RNC containers in their own right
//!   (both games' LEVELS.DAT).
//!
//! Sprite-style TABs with wider entries (e.g. MC2's TMAPS, 5050 bytes)
//! are a different format and rejected here; they get their own parser
//! when sprites land.

use crate::rnc;

#[derive(Debug, PartialEq, Eq)]
pub enum TabError {
    /// TAB length is zero or not a multiple of 4.
    BadLength(usize),
    /// Offsets are not monotonically non-decreasing, or exceed the DAT.
    BadOffsets,
    /// Whole-file RNC decompression of the DAT or TAB failed.
    Rnc(rnc::RncError),
}

impl std::fmt::Display for TabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadLength(n) => write!(f, "TAB length {n} is not a non-zero multiple of 4"),
            Self::BadOffsets => write!(f, "TAB offsets not monotonic or beyond DAT end"),
            Self::Rnc(e) => write!(f, "archive decompression: {e}"),
        }
    }
}

impl std::error::Error for TabError {}

impl From<rnc::RncError> for TabError {
    fn from(e: rnc::RncError) -> Self {
        Self::Rnc(e)
    }
}

/// One archive member.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub index: usize,
    pub offset: u32,
    pub len: u32,
}

/// A DAT/TAB archive, fully resident in memory with any whole-file
/// compression already undone.
pub struct Archive {
    dat: Vec<u8>,
    entries: Vec<Entry>,
}

impl Archive {
    /// Open from raw file contents. Either input may be a whole-file RNC
    /// container; both are transparently decompressed before indexing.
    pub fn open(dat_raw: &[u8], tab_raw: &[u8]) -> Result<Self, TabError> {
        let dat = maybe_decompress(dat_raw)?;
        let tab = maybe_decompress(tab_raw)?;

        if tab.is_empty() || tab.len() % 4 != 0 {
            return Err(TabError::BadLength(tab.len()));
        }
        let offsets: Vec<u32> = tab
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();

        let dat_len = dat.len() as u32;
        let mut entries = Vec::with_capacity(offsets.len());
        for (index, pair) in offsets.windows(2).enumerate() {
            let (start, end) = (pair[0], pair[1]);
            if end < start || end > dat_len {
                return Err(TabError::BadOffsets);
            }
            entries.push(Entry {
                index,
                offset: start,
                len: end - start,
            });
        }
        // The last offset owns everything up to the end of the DAT
        // (zero-length when the TAB is sentinel-padded, as LEVELS.TAB is).
        let last = *offsets.last().unwrap();
        if last > dat_len {
            return Err(TabError::BadOffsets);
        }
        entries.push(Entry {
            index: offsets.len() - 1,
            offset: last,
            len: dat_len - last,
        });

        Ok(Self { dat, entries })
    }

    /// All entries, including empty ones (callers usually want
    /// [`Archive::non_empty`]).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn non_empty(&self) -> impl Iterator<Item = Entry> + '_ {
        self.entries.iter().copied().filter(|e| e.len > 0)
    }

    /// Raw bytes of an entry, still compressed if it is an RNC container.
    pub fn raw(&self, entry: Entry) -> &[u8] {
        &self.dat[entry.offset as usize..(entry.offset + entry.len) as usize]
    }

    /// Entry payload with per-entry RNC transparently undone.
    pub fn extract(&self, entry: Entry) -> Result<Vec<u8>, rnc::RncError> {
        let raw = self.raw(entry);
        if rnc::is_rnc(raw) {
            rnc::decompress(raw)
        } else {
            Ok(raw.to_vec())
        }
    }
}

fn maybe_decompress(data: &[u8]) -> Result<Vec<u8>, rnc::RncError> {
    if rnc::is_rnc(data) {
        rnc::decompress(data)
    } else {
        Ok(data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(offsets: &[u32]) -> Vec<u8> {
        offsets.iter().flat_map(|o| o.to_le_bytes()).collect()
    }

    #[test]
    fn slices_by_offset_deltas() {
        let dat = b"HEADERaaabbbbcc";
        let tab = le(&[6, 9, 13]);
        let a = Archive::open(dat, &tab).unwrap();
        assert_eq!(a.entries().len(), 3);
        assert_eq!(a.raw(a.entries()[0]), b"aaa");
        assert_eq!(a.raw(a.entries()[1]), b"bbbb");
        // Final entry runs to the end of the DAT.
        assert_eq!(a.raw(a.entries()[2]), b"cc");
    }

    #[test]
    fn sentinel_padding_yields_empty_entries() {
        let dat = b"HEADERaaa";
        let tab = le(&[6, 9, 9, 9]);
        let a = Archive::open(dat, &tab).unwrap();
        assert_eq!(a.entries().len(), 4);
        assert_eq!(a.non_empty().count(), 1);
        assert_eq!(a.raw(a.non_empty().next().unwrap()), b"aaa");
    }

    #[test]
    fn rejects_non_monotonic_offsets() {
        assert_eq!(
            Archive::open(b"0123456789", &le(&[4, 2])).err().unwrap(),
            TabError::BadOffsets
        );
    }

    #[test]
    fn rejects_offsets_beyond_dat() {
        assert_eq!(
            Archive::open(b"0123", &le(&[0, 99])).err().unwrap(),
            TabError::BadOffsets
        );
    }

    #[test]
    fn rejects_bad_tab_length() {
        assert_eq!(
            Archive::open(b"0123", &[0, 0, 0]).err().unwrap(),
            TabError::BadLength(3)
        );
        assert_eq!(
            Archive::open(b"0123", &[]).err().unwrap(),
            TabError::BadLength(0)
        );
    }
}
