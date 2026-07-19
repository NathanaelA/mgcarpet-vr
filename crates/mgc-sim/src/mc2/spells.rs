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
//! GenerateEvents pass EV:367-371) and the class-15 cast costs /
//! GetSpellManaCost. LevelInit.cpp:12-21 patches rows 4 and 19 at
//! level init, keyed to MapType (Day vs non-Day; tier-0 life +
//! hintText only — the open-closure trace §4) — see
//! [`level_init_patch`].

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

/// `LevelInit_56C00`'s SPELLS patch (LevelInit.cpp:12-21, verbatim):
/// every level init re-writes rows 4 and 19, tier 0 only — life +
/// hintText — keyed to MapType. Non-Day (Night/Cave) is the default
/// arm (life 19, hints 199/245); Day overrides it (life 2, hints
/// 198/244). Runs over the freshly-loaded DAT in retail; here over
/// the bundle-parsed table, re-applied whenever the environment is
/// (re)declared — idempotent per `day` value, like retail's
/// unconditional writes (docs/traces/mc2-class10-m9-dome-open-closure
/// .md §4.5).
pub fn level_init_patch(rows: &mut [Mc2SpellRow], day: bool) {
    let (life, h4, h19) = if day { (2, 198, 244) } else { (19, 199, 245) };
    if let Some(r) = rows.get_mut(4) {
        r.tiers[0].life = life;
        r.tiers[0].hint_text = h4;
    }
    if let Some(r) = rows.get_mut(19) {
        r.tiers[0].life = life;
        r.tiers[0].hint_text = h19;
    }
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

/// The retail `LANGUAGE/L1.TXT` spell-name strings (English), lang
/// indices **159..=265**, extracted verbatim from `game.gog` and
/// cross-verified 1:1 against the SPELLS.DAT `hintText` indices
/// (docs/spell-audit/spell-names.md §2). Index 159 = the level-up
/// format string; 160..185 = the UPPERCASE base names (one per spell
/// row); 186..265 = the mixed-case per-tier hint names the hover /
/// spell-change surfaces show. There is NO roman-numeral generator —
/// "Crater / Crater II / Crater III" are three literal strings; the
/// engine indexes one string per (spell,tier) via `hintText`. The
/// rows-4/19 tier-0 Day/non-Day alternates (198/199, 244/245) are
/// applied at runtime by [`level_init_patch`], so resolving the LIVE
/// `hint_text` yields the correct environment name automatically.
/// (CD import for other locales / a bundle bake are deferred; the
/// shipped English game is byte-identical to this table.)
const LANG_BASE: usize = 159;
static MC2_LANG: [&str; 107] = [
    "Your ability to cast %s has improved.", // 159
    "FIREBALL",                              // 160
    "POSSESS",                               // 161
    "CASTLE",                                // 162
    "SPEED UP",                              // 163
    "MORPH",                                 // 164
    "HEAL",                                  // 165
    "SHIELD",                                // 166
    "LIGHTNING",                             // 167
    "REBOUND",                               // 168
    "METEOR",                                // 169
    "TELEPORT",                              // 170
    "INVISIBLE",                             // 171
    "BEYOND SIGHT",                          // 172
    "STEAL MANA",                            // 173
    "DUEL",                                  // 174
    "TREMOR",                                // 175
    "CRATER",                                // 176
    "EARTHQUAKE",                            // 177
    "VOLCANO",                               // 178
    "SUMMON ARMY",                           // 179
    "GRAVITY WELL",                          // 180
    "WHIRLWIND",                             // 181
    "FOOL'S MANA",                           // 182
    "MAGIC MINE",                            // 183
    "ALLIANCE",                              // 184
    "CAVE IN",                               // 185
    "FireBall",                              // 186 spell 0
    "Rapid Fire",                            // 187
    "Fire Storm",                            // 188
    "Possession",                            // 189 spell 1
    "Mana Magnet",                           // 190
    "Mana Lock",                             // 191
    "Castle",                                // 192 spell 2
    "Fire Tower",                            // 193
    "Lightning Tower",                       // 194
    "Speed Up",                              // 195 spell 3
    "Super Speed",                           // 196
    "Super Speed II",                        // 197
    "Bee Morph",                             // 198 spell 4 (Day tier 0)
    "Firefly Morph",                         // 199 spell 4 (non-Day tier 0)
    "Cymmerian Morph",                       // 200
    "Wyvern Morph",                          // 201
    "Heal",                                  // 202 spell 5
    "Aid",                                   // 203
    "Constitution",                          // 204
    "Shield",                                // 205 spell 6
    "Shield II",                             // 206
    "Invulnerable",                          // 207
    "Lightning",                             // 208 spell 7
    "Thunderbolt",                           // 209
    "Thunderstorm",                          // 210
    "Rebound",                               // 211 spell 8
    "Rebound II",                            // 212
    "Amplify",                               // 213
    "Meteor",                                // 214 spell 9
    "Meteor II",                             // 215
    "Meteor III",                            // 216
    "Teleport",                              // 217 spell 10
    "Teleport II",                           // 218
    "Castle Port",                           // 219
    "Invisible",                             // 220 spell 11
    "Possess Invisible",                     // 221
    "Attack Invisible",                      // 222
    "Beyond Sight",                          // 223 spell 12
    "See Invisible",                         // 224
    "See All",                               // 225
    "Steal Mana",                            // 226 spell 13
    "Double Steal",                          // 227
    "Burgle",                                // 228
    "Duel",                                  // 229 spell 14
    "Mana Drain",                            // 230
    "Health Drain",                          // 231
    "Tremor",                                // 232 spell 15
    "Tremor II",                             // 233
    "Tremor III",                            // 234
    "Crater",                                // 235 spell 16
    "Crater II",                             // 236
    "Crater III",                            // 237
    "Earthquake",                            // 238 spell 17
    "Earthquake II",                         // 239
    "Earthquake III",                        // 240
    "Volcano",                               // 241 spell 18
    "Volcano II",                            // 242
    "Volcano III",                           // 243
    "Bee Army",                              // 244 spell 19 (Day tier 0)
    "Firefly Army",                          // 245 spell 19 (non-Day tier 0)
    "Cymmerian Army",                        // 246
    "Wyvern Army",                           // 247
    "Gravity Well",                          // 248 spell 20
    "Gravity Well II",                       // 249
    "Gravity Well III",                      // 250
    "Whirlwind",                             // 251 spell 21
    "Whirlwind II",                          // 252
    "Whirlwind III",                         // 253
    "Fool's Mana",                           // 254 spell 22
    "Rapid Fire",                            // 255
    "Lightning Fire",                        // 256
    "Magic Mine",                            // 257 spell 23
    "Magic Mine II",                         // 258
    "Magic Mine III",                        // 259
    "Alliance",                              // 260 spell 24
    "Alliance II",                           // 261
    "Alliance III",                          // 262
    "Cave In",                               // 263 spell 25
    "Cave In II",                            // 264
    "Cave In III",                           // 265
];

/// Resolve a `LANGUAGE/L1.TXT` lang-string index (159..=265) to its
/// verbatim English string. Out-of-range → `""`.
pub fn lang(idx: i16) -> &'static str {
    (idx as usize)
        .checked_sub(LANG_BASE)
        .and_then(|i| MC2_LANG.get(i))
        .copied()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_resolves_per_tier_names() {
        // The three surfaces read distinct strings per tier — no numeral
        // generator (docs/spell-audit/spell-names.md).
        assert_eq!(lang(159), "Your ability to cast %s has improved.");
        assert_eq!(lang(160), "FIREBALL"); // base name (level-up banner)
        assert_eq!(lang(189), "Possession"); // spell 1 tier 0
        assert_eq!(lang(190), "Mana Magnet"); // spell 1 tier 1
        assert_eq!(lang(191), "Mana Lock"); // spell 1 tier 2
        assert_eq!(lang(199), "Firefly Morph"); // spell 4 non-Day tier 0
        assert_eq!(lang(265), "Cave In III"); // last entry
        assert_eq!(lang(158), ""); // below range
        assert_eq!(lang(266), ""); // above range
    }

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
    fn level_init_patch_rows_4_19() {
        // LevelInit.cpp:12-21 verbatim: non-Day default arm, Day
        // override; tier 0 only, other tiers/rows untouched.
        let mut rows = vec![Mc2SpellRow::default(); MC2_SPELL_ROWS];
        rows[4].tiers[1].life = 7;
        level_init_patch(&mut rows, false);
        assert_eq!(
            (rows[4].tiers[0].life, rows[4].tiers[0].hint_text),
            (19, 199)
        );
        assert_eq!(
            (rows[19].tiers[0].life, rows[19].tiers[0].hint_text),
            (19, 245)
        );
        level_init_patch(&mut rows, true);
        assert_eq!(
            (rows[4].tiers[0].life, rows[4].tiers[0].hint_text),
            (2, 198)
        );
        assert_eq!(
            (rows[19].tiers[0].life, rows[19].tiers[0].hint_text),
            (2, 244)
        );
        assert_eq!(rows[4].tiers[1].life, 7);
        assert_eq!(rows[5], Mc2SpellRow::default());
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
