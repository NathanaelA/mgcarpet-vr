//! The MC1 (Magic Carpet 1 / Hidden Worlds) game columns — the
//! MC1-specific tables, spawn dispatch, spells and rosters that plug
//! into the shared chassis. The game-agnostic runtime lives OUTSIDE
//! this namespace: [`crate::engine`] (the shared world/features
//! chassis), [`crate::chassis`] (shared-engine parameter sets) and
//! [`crate::flight`] (the flight-model seam, MC1 + enhanced).
//!
//! Hidden Worlds is NOT a separate namespace: retail ships it as a
//! sibling binary of the same engine, consuming this module with its
//! own asset bundles.

pub mod behavior;
pub(crate) mod combat;

/// The MC1 THING registry column ([`crate::ids::GameId::known_thing`]):
/// the `(class, model)` set this game's spawn dispatch understands —
/// including its AUTHENTIC no-spawns (class-10 null/stub creators,
/// class-3 start markers), which are known non-entities, not misfits.
/// Derived from the spawn guards in `mobs.rs` (`spawn_scenery`
/// model ≤ 5, `spawn_class3` model ≤ 11, `spawn_creature` model ≤ 16),
/// `engine/features.rs` (`spawn_creator` model ≤ 61) and
/// `engine/world.rs` (`spawn_from_thing`'s class dispatch,
/// `spawn_trigger` states).
pub(crate) fn known_thing(class: u16, model: u16) -> bool {
    match class {
        2 => model <= 5,
        3 => model <= 11,
        5 => model <= 16,
        // Spawner logic / authored spell effects / jars: any model —
        // classes 7/9 park inert, class-12 models are jar variants.
        7 | 9 | 12 => true,
        10 => model <= 61,
        // Trigger volumes: model = the trigger state machine. All 32
        // states are live table entries, but state 31's retail handler
        // is a bare `ret` (CARPET.EXE 0x5A080) — it is the map-"O"
        // marker family and nothing else; see `World::trigger_tick`.
        11 => model <= 31,
        _ => false,
    }
}
pub mod corners;
pub mod entities;
pub(crate) mod mobs;
pub mod rivals;
pub mod spells;
pub mod sprite_stats;
pub(crate) mod tables;

/// `MGC_NO_MC1_SPRITE_SHEET_EXTENTS=1` keeps the static sprite-stat
/// table on every MC1 / Hidden Worlds level (round 168). See
/// [`derive_sprite_stats`].
pub(crate) fn no_mc1_sprite_sheet_extents() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_SPRITE_SHEET_EXTENTS").is_some())
}

/// THE SPRITE-STAT WIDTHS ARE THE SPRITE SHEET'S, NOT THE BINARY'S
/// (round 168). Both `CARPET.EXE` (file 0x9F398) and `HIDDEN.EXE`
/// (0x9F598) ship `unk_99BA0x` with EVERY row's width 0 — the two
/// tables differ in nothing but the name pointer — and the loader
/// derives it from the row's first sprite (remc1 sub_main.cpp:66699,
/// `var_6 = var_8 * sprite_w / sprite_h`, a missing sprite reading
/// 255 x 255). [`sprite_stats::SPRITE_STATS`] is that derivation run
/// on the TEMPERATE bank (0 of 286 rows differ); Hidden Worlds loads
/// the ARCTIC bank, whose trees are other bitmaps, and four rows
/// derive differently: 61 (177 -> 1), 83 (355 -> 343), 84 (355 -> 2)
/// and 226 (352 -> 355). [`crate::engine::features::Gen::set_sprite`]
/// halves the pair into the collision box, so every HW tree stood in
/// a box 6 too wide (171 retail, 177 port) and every burnt one 1 too
/// narrow (177 / 176). WITNESS mc1hwl0 t=1640, tree slot 118 burning
/// down to row 226: `+80` / `+82` 171 -> 177; 1.8 million rows on six
/// HW takes, and round 165's `(2,0) f80 / f82` init rows.
///
/// `dims[sprite_id]` = (width, height) off the bundle's sprite index.
/// Returns one (width, height) per row — EMPTY when the sheet derives
/// the static table, so the temperate bank hashes as it always has.
pub fn derive_sprite_stats(dims: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let derived: Vec<(u16, u16)> = sprite_stats::SPRITE_STATS
        .iter()
        .map(|s| {
            let (mut w, mut h) = dims.get(s.sprite_base as usize).copied().unwrap_or((0, 0));
            if w == 0 || h == 0 {
                (w, h) = (255, 255);
            }
            let width = (s.height as u32 * w as u32 / h as u32).min(u16::MAX as u32) as u16;
            (width, s.height)
        })
        .collect();
    let same = derived
        .iter()
        .zip(sprite_stats::SPRITE_STATS.iter())
        .all(|(d, s)| *d == (s.width, s.height));
    if same || no_mc1_sprite_sheet_extents() {
        Vec::new()
    } else {
        derived
    }
}

#[cfg(test)]
mod sheet_tests {
    use super::*;

    /// Round 168 ([`derive_sprite_stats`]): the row widths are the
    /// sheet's. The arctic bank's four bitmaps — sprite 140 at 110x96,
    /// 299 at 122x103, 153 and 156 at 1x122 — derive rows 83, 226, 61
    /// and 84 as retail's Hidden Worlds holds them (mc1hwl0: a tree's
    /// `+80` 171, a burnt tree's 177), and a sheet that derives the
    /// static table answers empty.
    #[test]
    fn the_sprite_stat_widths_follow_the_sheet() {
        // A sheet whose every bitmap has its row's static aspect.
        let mut dims = vec![(0u16, 0u16); 600];
        for s in sprite_stats::SPRITE_STATS.iter() {
            dims[s.sprite_base as usize] = (s.width, s.height);
        }
        let same = sprite_stats::SPRITE_STATS
            .iter()
            .all(|s| dims[s.sprite_base as usize] == (s.width, s.height));
        if same {
            assert!(derive_sprite_stats(&dims).is_empty(), "the static table's own sheet");
        }
        dims[140] = (110, 96);
        dims[299] = (122, 103);
        dims[153] = (1, 122);
        dims[156] = (1, 122);
        let d = derive_sprite_stats(&dims);
        assert_eq!(d.len(), sprite_stats::SPRITE_STATS.len());
        assert_eq!(d[83], (343, 300), "the HW tree: +80 = 171");
        assert_eq!(d[226], (355, 300), "the burnt tree: +80 = 177");
        assert_eq!(d[84], (2, 300));
        assert_eq!(d[61], (1, 150));
        // A missing bitmap reads 255 x 255 (:66692-94): width = height.
        let bare = derive_sprite_stats(&[]);
        assert_eq!(bare[83], (300, 300));
    }
}
