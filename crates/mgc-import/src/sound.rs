//! Sound-bank baking: MC1 `DATA/SNDS<bank>-<q>.DAT/.TAB` families →
//! the audio bundle's `sounds.bin` + `sounds.json`.
//!
//! Format (probed from retail data + remc1 sub_5D070/sub_5D138): both
//! files whole-file RNC. The TAB is a directory of 32-byte records:
//! `name[18]` (NUL-padded, `8.3` upper-case), `u32 offset` at +0x12
//! into the decompressed DAT, `u32 len` at +0x1A, `u16` at +0x1E
//! (constant 90 in every retail entry; purpose unknown, not baked).
//! Record 0 is a header pseudo-entry: empty name, len = whole DAT.
//! The DAT is raw unsigned 8-bit mono PCM.
//!
//! Bank digit = the engine's per-level sound-set selector (level
//! command case 4/36 → remc1 sub_5D070_5D580); quality digit = the
//! original's free-RAM tier (remc1 :51973 — `-1` ≥ 5 MB free, `-0`,
//! `-3` smallest). Per-sample sizes are exactly 2.00x between `-0`
//! and `-1`: the tiers are sample-rate halvings of one master, so we
//! bake only `-1` (22050 Hz) and the rate ladder dies here.
//!
//! Sound ids are the TAB record indices: the engine's per-tick request
//! slots index the loaded bank directly (remc1 sub_55100 flushes slots
//! 0..47 = bank 0's table). Bank 0 is the 47-entry gameplay bank
//! (WAVES2/FIREBAL1/...); banks 1..13 are small auxiliary sets (intro
//! voices, menu feedback, door sounds) loaded by other screens.

use std::collections::HashMap;

use mgc_formats::bundle::{SoundBankIndex, SoundEntry, SoundIndex};

/// Decompressed-TAB record size.
const TAB_RECORD: usize = 32;
const NAME_LEN: usize = 0x12;
const OFFSET_AT: usize = 0x12;
const LEN_AT: usize = 0x1A;

/// One parsed sample bank, PCM still referencing the source DAT.
pub struct ParsedBank<'a> {
    pub bank: u32,
    /// `(id, name, pcm)` per real entry (header pseudo-entry dropped).
    pub entries: Vec<(u32, String, &'a [u8])>,
}

/// Parse one decompressed TAB + DAT pair. `trim_tail` cuts the
/// 16-byte per-entry tail pad the SAMPLE driver never plays
/// (sub_48570/sub_52120 pass `size - 16`; the pad is garbage on some
/// samples and 0x00 = full-negative PCM on others — audible loop-seam
/// cracks if kept). Music banks must NOT trim: HMP payloads own their
/// full extent (CGAME2's last track runs into those bytes).
pub fn parse_bank<'a>(
    bank: u32,
    tab: &[u8],
    dat: &'a [u8],
    trim_tail: bool,
) -> Result<ParsedBank<'a>, String> {
    if tab.len() % TAB_RECORD != 0 {
        return Err(format!(
            "bank {bank}: TAB is {} bytes, not {TAB_RECORD}-byte records",
            tab.len()
        ));
    }
    let mut entries = Vec::new();
    for (i, rec) in tab.chunks_exact(TAB_RECORD).enumerate() {
        let name_raw = &rec[..NAME_LEN];
        let name_end = name_raw.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        let name = std::str::from_utf8(&name_raw[..name_end])
            .map_err(|_| format!("bank {bank} record {i}: non-ASCII name"))?;
        let offset =
            u32::from_le_bytes(rec[OFFSET_AT..OFFSET_AT + 4].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(rec[LEN_AT..LEN_AT + 4].try_into().unwrap()) as usize;
        if i == 0 {
            // Header pseudo-entry: empty name; its size field is a
            // load-allocation hint, not the DAT length (bank 0: says
            // 2028288, DAT is 1984208) — nothing to validate here.
            continue;
        }
        if offset + len > dat.len() {
            return Err(format!(
                "bank {bank} record {i} ({name}): {offset}+{len} spans past DAT ({})",
                dat.len()
            ));
        }
        let len = if trim_tail { len.saturating_sub(16) } else { len };
        let name = name
            .strip_suffix(".RAW")
            .unwrap_or(name)
            .to_ascii_lowercase();
        entries.push((i as u32, name, &dat[offset..offset + len]));
    }
    Ok(ParsedBank { bank, entries })
}

/// Concatenate parsed banks into one deduplicated PCM blob + index.
/// Samples dedupe by content (identical samples repeat within a bank —
/// FIRE at ids 5 and 45 — and across banks).
pub fn bake_blob(banks: &[ParsedBank<'_>], sample_rate: u32) -> (SoundIndex, Vec<u8>) {
    let mut blob = Vec::new();
    let mut seen: HashMap<&[u8], u32> = HashMap::new();
    let mut index = SoundIndex {
        sample_rate,
        encoding: "pcm8".into(),
        banks: Vec::new(),
    };
    for bank in banks {
        let mut entries = Vec::new();
        for &(id, ref name, pcm) in &bank.entries {
            let offset = *seen.entry(pcm).or_insert_with(|| {
                let at = blob.len() as u32;
                blob.extend_from_slice(pcm);
                at
            });
            entries.push(SoundEntry {
                id,
                name: name.clone(),
                offset,
                len: pcm.len() as u32,
            });
        }
        index.banks.push(SoundBankIndex {
            bank: bank.bank,
            entries,
        });
    }
    (index, blob)
}

/// MC2 `SOUND/SOUND.DAT`: one file holding every sample bank in up
/// to six quality tiers (remc2 LoadSounds_84300/ReadAndDecompressSound;
/// SoundNumber codes 1644/1622/1611/822/811/800 = 16- or 8-bit at
/// 44/22/11 kHz — the GOG release ships the two 8-bit tiers only).
///
/// Layout: `u32` at EOF-4 → directory: `i16 bank_count[6]` then per
/// bank a 96-byte record of six 16-byte tier slots
/// `{i32 index_off, i32 data_off, i32 index_size, i32 data_size}`
/// (-1 = tier absent). Each tier's index is the SAME 32-byte record
/// table as MC1's SNDS TAB (entry 0 = pseudo-header; engine sound id
/// = record index — OCEAN.WAV is id 1 = remc2 `Ocean_1`), except
/// offsets point into the tier's data region and each sample is a
/// full RIFF WAV (8-bit mono 22050 across the whole retail file);
/// the WAV container is stripped at bake so `sounds.bin` stays raw
/// PCM like MC1's.
pub fn parse_mc2_sound_dat(data: &[u8]) -> Result<Vec<ParsedBank<'_>>, String> {
    if data.len() < 16 {
        return Err("SOUND.DAT too short".into());
    }
    let u32_at = |at: usize| -> Result<usize, String> {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or_else(|| format!("read past EOF at {at}"))
    };
    let dir = u32_at(data.len() - 4)?;
    if dir + 12 > data.len() {
        return Err(format!("directory offset {dir} out of range"));
    }
    let counts: Vec<i16> = (0..6)
        .map(|q| i16::from_le_bytes(data[dir + q * 2..dir + q * 2 + 2].try_into().unwrap()))
        .collect();
    let bank_count = counts.iter().copied().max().unwrap_or(0).max(0) as usize;

    let mut banks = Vec::new();
    for bank in 0..bank_count {
        let rec = dir + 12 + 96 * bank;
        // Best available tier: lowest index = highest quality.
        let mut chosen = None;
        for q in 0..6 {
            let slot = rec + 16 * q;
            // Tier slot fields (remc2 type_v8): {index_off, data_off,
            // index_size, data_size}; all-ones = tier absent.
            let index_off = u32_at(slot)?;
            if index_off != u32::MAX as usize && (bank as i16) < counts[q] {
                let data_off = u32_at(slot + 4)?;
                let index_size = u32_at(slot + 8)?;
                chosen = Some((index_off, data_off, index_size));
                break;
            }
        }
        let Some((index_off, data_off, index_size)) = chosen else {
            continue;
        };
        let index = data
            .get(index_off..index_off + index_size)
            .ok_or_else(|| format!("bank {bank}: index out of range"))?;
        if index.starts_with(b"RNC") {
            return Err(format!(
                "bank {bank}: RNC-compressed index — unsupported (not seen in retail GOG data)"
            ));
        }
        let mut entries = Vec::new();
        for (i, rec) in index.chunks_exact(TAB_RECORD).enumerate() {
            if i == 0 {
                continue; // pseudo-header, as in the MC1 TAB
            }
            let name_end = rec[..NAME_LEN].iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
            let name = std::str::from_utf8(&rec[..name_end])
                .map_err(|_| format!("bank {bank} record {i}: non-ASCII name"))?;
            let offset =
                u32::from_le_bytes(rec[OFFSET_AT..OFFSET_AT + 4].try_into().unwrap()) as usize;
            let len = u32::from_le_bytes(rec[LEN_AT..LEN_AT + 4].try_into().unwrap()) as usize;
            let wav = data
                .get(data_off + offset..data_off + offset + len)
                .ok_or_else(|| format!("bank {bank} record {i} ({name}): data out of range"))?;
            let pcm = strip_wav(wav)
                .map_err(|e| format!("bank {bank} record {i} ({name}): {e}"))?;
            let name = name
                .strip_suffix(".WAV")
                .unwrap_or(name)
                .to_ascii_lowercase();
            entries.push((i as u32, name, pcm));
        }
        banks.push(ParsedBank {
            bank: bank as u32,
            entries,
        });
    }
    Ok(banks)
}

/// Return the `data` chunk of an 8-bit mono 22050 Hz RIFF WAV.
fn strip_wav(wav: &[u8]) -> Result<&[u8], String> {
    if wav.len() < 44 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("not a RIFF WAV".into());
    }
    let mut pos = 12;
    let mut fmt_ok = false;
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = wav
            .get(pos + 8..pos + 8 + size)
            .ok_or("chunk spans past EOF")?;
        match id {
            b"fmt " => {
                if body.len() < 16 {
                    return Err("short fmt chunk".into());
                }
                let channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
                let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                let bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
                if (channels, rate, bits) != (1, 22050, 8) {
                    return Err(format!(
                        "unexpected format: {channels}ch {rate}Hz {bits}bit (schema is pcm8/22050)"
                    ));
                }
                fmt_ok = true;
            }
            b"data" => {
                if !fmt_ok {
                    return Err("data chunk before fmt".into());
                }
                return Ok(body);
            }
            _ => {}
        }
        pos += 8 + size + (size & 1);
    }
    Err("no data chunk".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, offset: u32, len: u32) -> [u8; 32] {
        let mut rec = [0u8; 32];
        rec[..name.len()].copy_from_slice(name.as_bytes());
        rec[OFFSET_AT..OFFSET_AT + 4].copy_from_slice(&offset.to_le_bytes());
        rec[LEN_AT..LEN_AT + 4].copy_from_slice(&len.to_le_bytes());
        rec[0x1E..0x20].copy_from_slice(&90u16.to_le_bytes());
        rec
    }

    #[test]
    fn parses_dedupes_and_trims_tail_pad() {
        // Two samples of 16 + 8 payload bytes, each followed by the
        // 16-byte tail pad the driver never plays.
        let dat: Vec<u8> = (0..56u8).collect();
        let mut tab = Vec::new();
        tab.extend_from_slice(&record("", 0, 56)); // header
        tab.extend_from_slice(&record("FIRE.RAW", 0, 32));
        tab.extend_from_slice(&record("NULL.RAW", 32, 24));
        tab.extend_from_slice(&record("FIRE.RAW", 0, 32)); // dup of id 1
        let bank = parse_bank(0, &tab, &dat, true).unwrap();
        assert_eq!(bank.entries.len(), 3);
        assert_eq!(bank.entries[0].1, "fire");
        assert_eq!(bank.entries[0].2.len(), 16); // 32 - pad
        let (index, blob) = bake_blob(&[bank], 22050);
        assert_eq!(blob.len(), 24); // dup collapsed, pads gone
        let e = &index.banks[0].entries;
        assert_eq!((e[0].id, e[0].offset, e[0].len), (1, 0, 16));
        assert_eq!((e[1].id, e[1].offset, e[1].len), (2, 16, 8));
        assert_eq!((e[2].id, e[2].offset), (3, 0));
    }

    #[test]
    fn rejects_out_of_range_entries() {
        let dat = vec![0u8; 8];
        let mut tab = Vec::new();
        tab.extend_from_slice(&record("", 0, 8));
        tab.extend_from_slice(&record("X.RAW", 4, 8));
        assert!(parse_bank(0, &tab, &dat, true).is_err());
    }
}
