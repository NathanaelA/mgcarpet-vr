//! The MC2 spell table — `SPELLS.DAT` (26 rows x 80 bytes), carried
//! verbatim in the mc2-* asset bundles as `spells.bin` and parsed
//! here into [`Mc2SpellRow`]s.
//!
//! Layout: remc2 Spells.h (`#pragma pack(1)`) — per row `{i8 byte_0,
//! u8 isEnabled_1, 3 x 26-byte subspell tiers}`; per tier
//! `{i32 subSpellIndex_2, i32 manaCost_6, i32 maxManaLimit_A,
//! i32 xpos1_E, i32 xpos2_0x12, i16 hintText_0x16, i16 word_0x18,
//! i8 life_0x1A, u8 fontType_0x1B}`. Retail loads the file over a
//! baked-in fallback (Basic.cpp:334 Pathstruct → SPELLS_BEGIN_BUFFER)
//! — and the CD's values DIFFER from that fallback (e.g. row 18
//! subSpell {400,800,1200} vs the fallback's {120,240,480}), so the
//! imported file is the authority
//! (docs/traces/mc2-class10-m9-dome-geometry.md §3).
//!
//! Consumers: the par1-authored class-10 effect overrides (the
//! sub_4A310 case-0xA bottom block, EF:33148-33195, and the
//! GenerateEvents pass EV:367-371) and — with Phase 4.2 — the
//! class-15 cast costs / GetSpellManaCost. LevelInit.cpp:12-21
//! patches rows 4 and 19 at level init, keyed to MapType (Day vs
//! non-Day; tier-0 life + hintText only — the open-closure trace §4);
//! not yet ported, noted where the table loads.

/// One subspell tier (26 bytes on disk).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mc2SubSpell {
    /// `subSpellIndex_2` — copied into the entity's subSpell home
    /// (the area-damage amount for the class-10 effects).
    pub sub_spell: i32,
    /// `manaCost_6`.
    pub mana_cost: i32,
    /// `maxManaLimit_A` — the caster mana-pool gate for this tier.
    pub max_mana_limit: i32,
    /// `xpos1_E` / `xpos2_0x12` — the spell-XP ladder thresholds.
    pub xpos1: i32,
    pub xpos2: i32,
    /// `hintText_0x16x` — hint string id.
    pub hint_text: i16,
    /// `word_0x18`.
    pub word_0x18: i16,
    /// `life_0x1A` — copied into maxLife (model 9) or life (models
    /// 0x0B/0x0F) by the override sites.
    pub life: i8,
    /// `fontType_0x1B`.
    pub font_type: u8,
}

/// One spell row (80 bytes on disk).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mc2SpellRow {
    pub byte_0: i8,
    pub enabled: u8,
    /// The three tiers, indexed by the THING's `par1` (spell level).
    pub tiers: [Mc2SubSpell; 3],
}

/// Rows in the table.
pub const MC2_SPELL_ROWS: usize = 26;

/// Parse `spells.bin` (SPELLS.DAT verbatim). Anything but exactly
/// 26 x 80 bytes is a malformed bake.
pub fn parse(bytes: &[u8]) -> Result<Vec<Mc2SpellRow>, String> {
    if bytes.len() != MC2_SPELL_ROWS * 80 {
        return Err(format!(
            "spells.bin: expected {} bytes, got {}",
            MC2_SPELL_ROWS * 80,
            bytes.len()
        ));
    }
    let i32le = |b: &[u8], o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let i16le = |b: &[u8], o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
    Ok(bytes
        .chunks_exact(80)
        .map(|row| Mc2SpellRow {
            byte_0: row[0] as i8,
            enabled: row[1],
            tiers: std::array::from_fn(|k| {
                let t = &row[2 + 26 * k..2 + 26 * (k + 1)];
                Mc2SubSpell {
                    sub_spell: i32le(t, 0),
                    mana_cost: i32le(t, 4),
                    max_mana_limit: i32le(t, 8),
                    xpos1: i32le(t, 12),
                    xpos2: i32le(t, 16),
                    hint_text: i16le(t, 20),
                    word_0x18: i16le(t, 22),
                    life: t[24] as i8,
                    font_type: t[25],
                }
            }),
        })
        .collect())
}

/// `GetSpellIndex_6E020` (EF:44240) — class-10 effect model → spell
/// row. Everything unlisted resolves to row 0.
pub fn spell_index(model: u8) -> usize {
    match model {
        9 => 18,  // apocalypse dome / raise land
        11 => 16, // ground-fire spray
        15 => 17, // fire trail
        17 => 9,  // meteor
        22 => 21, // whirlwind
        67 => 20, // flood/quake
        71 => 15, // fissure
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_synthetic_row() {
        let mut bytes = vec![0u8; MC2_SPELL_ROWS * 80];
        // Row 18, tier 1: subSpell 800, life 9.
        let base = 18 * 80 + 2 + 26;
        bytes[18 * 80] = 3;
        bytes[18 * 80 + 1] = 8;
        bytes[base..base + 4].copy_from_slice(&800i32.to_le_bytes());
        bytes[base + 24] = 9;
        let t = parse(&bytes).unwrap();
        assert_eq!(t.len(), MC2_SPELL_ROWS);
        assert_eq!((t[18].byte_0, t[18].enabled), (3, 8));
        assert_eq!(t[18].tiers[1].sub_spell, 800);
        assert_eq!(t[18].tiers[1].life, 9);
        assert_eq!(t[18].tiers[0], Mc2SubSpell::default());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse(&[0u8; 80]).is_err());
    }

    #[test]
    fn model_to_row_map() {
        // EF:44243-44249 verbatim.
        for (m, r) in [
            (9, 18),
            (11, 16),
            (15, 17),
            (17, 9),
            (22, 21),
            (67, 20),
            (71, 15),
            (0, 0),
            (45, 0),
        ] {
            assert_eq!(spell_index(m), r);
        }
    }
}
