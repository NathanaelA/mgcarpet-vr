//! Round 132: **THE AUTHORED RIVAL SEED IS AN UNGRADED LANE.**
//!
//! Round 131's W8 dig answered the player's *"later in the level they
//! are more difficult to kill"* out of retail's OWN recorded bytes: a
//! rival's authored life handicap (`Life_0x3612F` → `word_0x24A_586`)
//! is **permanently discarded on its first death** — `sub_5C950`
//! resets the scalar to 256 and `maxLife` to 10000, and the authored
//! read sits inside the NEW-ENTITY `if (v9)` arm, skipped on reuse.
//! A rival authored at 51/256 therefore ratchets to **5.02× HP and
//! 5.02× healing at once** the moment the player first kills it, and
//! never moves again.
//!
//! ⚠⚠ **NO RECORDING IN THE CORPUS CAN SEE THE SEED.** `replay`
//! re-imports the rival-AI channel (`life_scale`, aggression,
//! perception, reflexes) and every entity's `max_life`/`life` at each
//! anchor, so a port that dropped the authored block entirely — every
//! rival starting at full strength in real play, on every level —
//! would give **ZERO divergences corpus-wide**. Same class as the
//! ungraded difficulty RATE ([[ungraded-lanes-register]]). This file
//! is the only thing standing under that lane.
//!
//! The expected values are **retail's own recorded `word_0x24A_586` /
//! `word_0x242..0x246` at t=1**, read off the takes with
//! `mgc-conform dump-state <take> 1 0`. They are written here as
//! constants on purpose: a recording is PROVENANCE, not identity, and
//! a test must never read `recordings/` (player-ruled, round 131).
//!
//! Provenance, verified this session:
//! ```text
//! mc2l6 / mc2l6-rival-spells-galore  (level-006)
//!   p1 life_scale=82  agg/per/refl=241/120/60
//!   p2 life_scale=51  agg/per/refl=188/108/48
//!   p3 life_scale=90  agg/per/refl=240/255/70
//! mc2l22                              (level-022)
//!   p1 254 250/254/255   p2 239 228/250/253   p3 252 240/255/255
//!   p4 252 245/255/255   p5 253 249/255/254   p6 254 239/255/254
//!   p7 255 245/255/255
//! ```
//! Note mc2l6's spread (82/51/90 = 0.32x/0.20x/0.35x) against
//! mc2l22's (all ≥ 239 = ~1.0x): the EARLY campaign hands the player
//! deliberately frail rivals, and the ratchet is what erases that. A
//! port that defaulted every rival to 256 would break level 6 and
//! leave level 22 looking perfect.
mod common;

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

/// The app's own load path (the `mc2_authored_castles` twin), minus
/// the settle: the seed is a LOAD-TIME fact and must hold before any
/// tick runs.
fn load(level: &str, bank: &str) -> Option<(World, LevelPackage)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join(bank)).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.mc2_extent_dims(&root.join("assets")) {
        Some(dims) => assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims)),
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).ok()?,
        None => assets,
    };
    let file = std::fs::File::open(root.join("mc2").join(format!("{level}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let night = matches!(
        pkg.header.as_ref().map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    );
    let mut w =
        World::new_for_game_env(planes, &pkg.things.things, seed, assets, GameId::Mc2, night);
    w.set_placeholders(true);
    w.set_mc2_night_shade(night);
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some((w, pkg))
}

/// The app's / harness's `mc2_rival_configs` resolution.
fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(w), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(reflexes), Some(perception)) = (cfg.reflexes, cfg.perception) else {
            continue;
        };
        let mut start = [false; 26];
        let mut start_level = [0u8; 26];
        let mut blocked = [false; 26];
        for s in 0..26 {
            start[s] = cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
            start_level[s] = cfg
                .starting_spell_levels
                .get(s)
                .copied()
                .unwrap_or(0)
                .min(2);
            blocked[s] = cfg.blocked_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(Mc2RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            perception: perception.clamp(0, 255) as u8,
            reflexes: reflexes.clamp(0, 255) as u8,
            life: cfg.life.unwrap_or(0).max(0) as u16,
            castle_level: h.players[slot].max(0) as u8,
            start,
            start_level,
            blocked,
        });
    }
    (out, count)
}

/// `(slot, life_scale, aggression, perception, reflexes)` as RETAIL
/// recorded them at t=1.
type Seed = (u8, u16, u16, u16, u16);

/// Grade one level's whole authored roster end to end: the `.mgcl`
/// byte, the live `Mc2Rival`, the `Gen::mc2_life_scale` mirror the
/// CASTLE-HP factor reads (`castle.rs` `mc2_life_scale.0[slot]`), and
/// the wizard entity's actual `max_life` — which is the only one of
/// the four the player can feel.
fn check(level: &str, bank: &str, want: &[Seed]) {
    let Some((w, pkg)) = load(level, bank) else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let wiz = pkg.wizards.as_ref().expect("level authors a wizard block");
    for &(slot, life, agg, per, refl) in want {
        // 1. The BAKED byte. `Life_0x3612F` is authored 0 for "no
        //    handicap"; retail's runtime word is 256 there, so the
        //    two spellings must be reconciled, never conflated.
        let baked = wiz.wizards[slot as usize].life.unwrap_or(0).max(0) as u16;
        let baked_runtime = if baked == 0 { 256 } else { baked };
        assert_eq!(
            baked_runtime, life,
            "{level} p{slot}: baked .mgcl Life_0x3612F ({baked}) != retail's recorded \
             word_0x24A_586 {life} — the authored handicap was lost in the BAKE"
        );

        let (scale, max_life, act_life, mirror, a, p, r) = w
            .debug_mc2_rival_life_seed(slot)
            .unwrap_or_else(|| panic!("{level}: no rival in slot {slot}"));

        // 2. The live AI record.
        assert_eq!(
            scale, life,
            "{level} p{slot}: Mc2Rival::life_scale — the seed never reached the brain"
        );
        // 3. The World mirror the castle-HP factor reads. Round 131
        //    found this one already: `reanchor_mc2_rival_ai` seeded
        //    the brain and left the mirror at 256
        //    (`no_mc2_life_scale_import`), so a rival's WIZARD was
        //    frail while its CASTLE was full-strength.
        assert_eq!(
            mirror, life,
            "{level} p{slot}: Gen::mc2_life_scale mirror — the castle-HP factor \
             disagrees with the wizard's"
        );
        // 4. The consequence. `10000 * scale >> 8`, floored at 1
        //    (EF:43768 / rivals.rs).
        let want_max = ((10_000u64 * life as u64) >> 8).max(1) as u32;
        assert_eq!(
            max_life, want_max,
            "{level} p{slot}: wizard entity max_life for scale {life}"
        );
        assert_eq!(
            act_life, want_max as i32,
            "{level} p{slot}: a freshly authored wizard starts at FULL life"
        );

        // 5. The rest of the authored personality block rides the
        //    same ungraded lane — `word_0x242/0x244/0x246`, which
        //    pace hate, aim cones and decision cadence.
        assert_eq!(
            (a, p, r),
            (agg, per, refl),
            "{level} p{slot}: authored aggression/perception/reflexes"
        );
    }
}

/// **The frail-rival level.** 82/51/90 is 0.32x / 0.20x / 0.35x — p2
/// starts on 1,992 HP against the 10,000 it will hold forever after
/// its first death. This is the take W8 measured the ratchet on
/// (mc2l6-rsg p2 steps 51 → 256 at t=3084).
#[test]
fn mc2_level_006_authored_rival_life_handicap_is_seeded() {
    check(
        "level-006",
        "mc2-day",
        &[
            (1, 82, 241, 120, 60),
            (2, 51, 188, 108, 48),
            (3, 90, 240, 255, 70),
        ],
    );
}

/// **The full-strength contrast.** Every one of mc2l22's seven rivals
/// is authored within 7% of 1.0x, so a port that hardcoded 256 would
/// pass here and fail level 6 — which is exactly why both levels are
/// pinned. Their personality spread is the discriminator that keeps
/// THIS row from going vacuous.
#[test]
fn mc2_level_022_authored_rival_roster_is_seeded_at_full_strength() {
    check(
        "level-022",
        "mc2-day",
        &[
            (1, 254, 250, 254, 255),
            (2, 239, 228, 250, 253),
            (3, 252, 240, 255, 255),
            (4, 252, 245, 255, 255),
            (5, 253, 249, 255, 254),
            (6, 254, 239, 255, 254),
            (7, 255, 245, 255, 255),
        ],
    );
}

// ---------------------------------------------------------------- round 147
// THE NATIVE RIVAL BOOK SEAT (`InitialiseSpells_54A50`).
//
// Round 147's `init-check` opened the native-construction channel and
// found three registers the port's `mc2_spawn_rival` never seated the
// way retail's level init does. Same ungraded class as the life
// handicap above: `reanchor_mc2_rival_ai` restores the whole player
// block at every anchor, so NO recording can fail these rows and this
// file is the only thing standing under them.
//
//  1. `SpellLevels_0x41D` — the AI arm of `InitialiseSpells_54A50` is
//     a bare `if (IsAiPlayer == 1)` reached BEFORE the
//     StartingSpells/BlockedSpells test (EF:39025-29; NETHERW.EXE
//     0x792e5 `cmpb $0x1,0x9(%edx)` / `je 0x7938f`, arm 0x793a8-0x793af).
//     The authored `byte_0x360FBx` level therefore lands on ALL 26
//     slots, blocked and un-granted ones included; the port wrote it
//     only inside the grant.
//  2. `SpellIndexLeft/Right_0x451/0x453` — cleared to -1 (EF:38998-99)
//     and then seated with the FIRST and SECOND granted spell in the
//     same 26-iteration walk (EF:39089-96; NETHERW.EXE 0x794d9-0x79500).
//     The port left both at `Mc2Spellbook::default()`'s -1.
//  3. `str_611_byte_0x45C_1116` (the weave direction) — memset 0 and
//     written ONLY by `sub_13890`'s tick-0 arm (EF:6022/6027). The
//     port's ctor invented 1.
//
// The expected numbers below are RETAIL's own record-0 player blocks,
// read off the takes with `mgc-conform init-check` — provenance, not
// identity (no test reads `recordings/`).

/// Pull one wizard's shadow lanes out of the public projection.
fn lane(w: &World, wiz: u8, name: &str) -> Vec<i64> {
    let sh = w.wiz_shadow_mc2();
    let row = sh
        .iter()
        .find(|r| r.wiz == wiz)
        .unwrap_or_else(|| panic!("wizard {wiz} has no shadow row"));
    if let Some((_, v)) = row.arrays.iter().find(|(n, _)| *n == name) {
        return v.clone();
    }
    let (_, v) = row
        .scalars
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("wizard {wiz} has no lane {name}"));
    vec![*v]
}

/// `(color, authored level, hand_left, hand_right)` as RETAIL recorded
/// them in the player block at record 0.
type BookSeat = (u8, u8, i64, i64);

fn check_book(level: &str, bank: &str, want: &[BookSeat]) {
    let Some((w, pkg)) = load(level, bank) else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let wiz = pkg.wizards.as_ref().expect("level authors a wizard block");
    for &(slot, lvl, left, right) in want {
        let cfg = &wiz.wizards[slot as usize];
        // The baked table must actually carry the level this row
        // pins, or the assertion below is vacuous.
        for s in 0..26usize {
            assert_eq!(
                cfg.starting_spell_levels.get(s).copied().unwrap_or(0).min(2),
                lvl,
                "{level} p{slot}: baked byte_0x360FBx[{s}] != the pinned authored level",
            );
        }
        // 1. EVERY slot carries it, not just the granted ones — and
        //    the level must be authored NON-ZERO or this says nothing.
        assert_ne!(lvl, 0, "{level} p{slot}: a 0 level cannot witness the law");
        assert_eq!(
            lane(&w, slot, "levels"),
            vec![lvl as i64; 26],
            "{level} p{slot}: SpellLevels_0x41D is written for all 26 spells",
        );
        // …and at least one of those slots is NOT granted, or the
        // pre-fix port would pass too.
        let ungranted = (0..26usize)
            .filter(|&s| {
                cfg.starting_spells.get(s).copied().unwrap_or(0) == 0
                    || cfg.blocked_spells.get(s).copied().unwrap_or(0) != 0
            })
            .count();
        assert!(
            ungranted > 0,
            "{level} p{slot}: every spell is granted — this row cannot witness the law",
        );
        // 2. The quick slots: first granted in the left hand, second
        //    in the right.
        let granted: Vec<i64> = (0..26usize)
            .filter(|&s| {
                cfg.starting_spells.get(s).copied().unwrap_or(0) != 0
                    && cfg.blocked_spells.get(s).copied().unwrap_or(0) == 0
            })
            .map(|s| s as i64)
            .collect();
        assert_eq!(granted.first().copied(), Some(left), "{level} p{slot}: left");
        assert_eq!(granted.get(1).copied(), Some(right), "{level} p{slot}: right");
        assert_eq!(lane(&w, slot, "hand_left"), vec![left], "{level} p{slot}");
        assert_eq!(lane(&w, slot, "hand_right"), vec![right], "{level} p{slot}");
        // 3. The weave direction starts at the memset 0.
        assert_eq!(
            lane(&w, slot, "weave_dir"),
            vec![0],
            "{level} p{slot}: str_611_byte_0x45C_1116 is 0 until the first weave",
        );
    }
}

/// mc2l17: six rivals, every one authored `starting_spell_levels =
/// [1; 26]` with between 14 and 24 spells actually granted — so the
/// 12 / 11 / 10 / 5 / 5 / 2 un-granted slots that retail still levels
/// are the whole witness.
#[test]
fn mc2_level_017_native_rivals_carry_retails_book_seat() {
    check_book(
        "level-017",
        "mc2-day",
        &[
            (1, 1, 0, 1),
            (2, 1, 0, 1),
            (3, 1, 0, 1),
            (4, 1, 0, 1),
            (5, 1, 0, 1),
            (6, 1, 0, 1),
        ],
    );
}

/// mc2l18: the TIER-2 contrast — p1/p5/p6 are authored `[2; 26]`, so
/// a port that wrote a flat 1 (or clamped to the granted set) fails
/// here even though level 17 would pass.
#[test]
fn mc2_level_018_native_rivals_carry_the_tier_two_book_seat() {
    check_book(
        "level-018",
        "mc2-day",
        &[(1, 2, 0, 1), (5, 2, 0, 1), (6, 2, 0, 1)],
    );
}
