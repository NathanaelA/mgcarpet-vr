//! ROUND 148 (dig w148b) — THE THREE MC2 TERRAIN-AUTHORING LAWS that
//! closed the last five `terrain-check` residues in the corpus
//! (mc2l9, mc2l11, mc2l13, mc2l16, mc2l31).
//!
//! The GENERATE axis is invisible to `replay` (every pair re-imports
//! the recorded terrain), so no recording fixture can witness any of
//! these — `terrain-check` and a unit test are the only lanes
//! (docs/CONFORMANCE.md, "an ungraded-lane law can only take a unit
//! test"). Each test below rebuilds the level exactly as
//! `mgc-conform`'s `native_settled_world` does — the port's own
//! generator, settled by the take's recorder phase with the carpet
//! idle at the authored start — and asserts the CELLS the recording's
//! record 0 holds.
//!
//! 1. **`MGC_NO_MC2_PATH_POINT_DEFERRED_STAMP`** — the (10,30)
//!    waterpath point lays its run at ITS OWN dispatch
//!    (`ApplyPointToPath_343F0`, EventsFunctions.cpp:25050), in the
//!    pass's load settle, not synchronously inside `sub_48690`. The
//!    stamp draws the shared `pseudoRand` retile stream, so the
//!    collapse moved every draw in between.
//! 2. **`MGC_NO_MC2_ROAD_STRIP_SEAM_CARRY`** — the (10,27) road-strip
//!    walkers index the heightmap through a 16-bit word and advance
//!    it with `inc %ebx` (`sub_34210` / `sub_34000` / `sub_34110`), so
//!    a run that reaches x = 255 carries into the y byte and
//!    continues ON THE NEXT ROW.
//! 3. **`MGC_NO_MC2_RISER_SHADE_BYTE_FOLD`** — the class-14 model-1
//!    riser folds its NW-SE relief in AL with SIGNED-BYTE compares
//!    (`sub_59F60`), so a +96 relief reads −128 and takes the dark
//!    arm where an `i32` took the bright one.
//!
//! Each test FAILS with its own switch set. Self-skips without baked
//! mc2 data (game data is optional).

#[path = "common/mod.rs"]
mod common;

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::{Path, PathBuf};

fn baked_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    if !root.join("mc2").exists() {
        common::golden_skip("no baked/mc2 tree");
        return None;
    }
    if common::modded_bake(&root) {
        return None;
    }
    Some(root)
}

/// `mgc-conform`'s `native_settled_world` for MC2, minus the human's
/// carried book (no test here reads a book lane): build the level the
/// way the port does and tick it `settle` times with the carpet idle
/// at the authored start.
fn settled(level: u32, settle: u32) -> Option<Planes> {
    let root = baked_root()?;
    let file = std::fs::File::open(root.join("mc2").join(format!("level-{level:03}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    if pkg.meta.overlay.is_some() {
        common::golden_skip("MODDED level package");
        return None;
    }
    let header = pkg.header.as_ref();
    let variant = match header.map(|h| (h.map_type, h.gfx_type)) {
        Some((mgc_formats::MapType::Night, g)) if g & 2 != 0 => "mc2-night-fog",
        Some((mgc_formats::MapType::Night, _)) => "mc2-night",
        Some((mgc_formats::MapType::Cave, _)) => "mc2-cave",
        _ => "mc2-day",
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join(variant)).ok()?;
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    if let Some(dims) = bundle.mc2_extent_dims(&root.join("assets")) {
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp).ok()?;
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let night = matches!(
        header.map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    );
    let mut w = World::new_for_game_env(
        planes,
        &pkg.things.things,
        seed,
        assets,
        GameId::Mc2,
        night,
    );
    w.set_placeholders(true);
    w.set_mc2_night_shade(night);
    w.set_mc2_doom_level(header.is_some_and(|h| h.gfx_type & 2 != 0));
    w.set_mc2_castle_purge_level(header.is_some_and(|h| h.gfx_type & 4 != 0));
    if let Some(stages) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = stages
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        if !rows.is_empty() {
            w.set_mc2_stages(&rows);
        }
        let vars: Vec<(i8, i8, u8, u8, u32)> = stages
            .variables
            .iter()
            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
            .collect();
        if !vars.is_empty() {
            w.set_mc2_stagevars(&vars);
        }
    }
    let (wizards, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&wizards, count);
    // The authored player start (the class-3 model-4 THING, tile
    // centre) — the settle flies an IDLE carpet there.
    let (px, pz) = pkg
        .things
        .things
        .iter()
        .find(|t| t.kind == mgc_formats::ThingKind::Entity && t.class == 3 && t.model == 4)
        .map(|t| (t.x as f32 + 0.5, t.y as f32 + 0.5))
        .unwrap_or((128.5, 128.5));
    let idle = PlayerCommand::default();
    for _ in 0..settle {
        let alt = w.ground_height_tiles(px, pz) + 2.0;
        let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);
        w.tick(pose, idle);
    }
    Some(w.planes_clone())
}

fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(wz), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in wz.wizards.iter().enumerate().take(8).skip(1) {
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

fn cell(x: u8, y: u8) -> usize {
    (y as usize) << 8 | x as usize
}

/// ⭐ LAW 1 — THE WATERPATH POINT STAMPS AT ITS OWN DISPATCH.
///
/// mc2:11's (10,67) → (10,75) leg is a pure +Y run of 8 cells, and
/// exactly two of them are still type 1 when it lands: (10,73) and
/// (10,74). The run makes three `pseudoRand` draws there, and the
/// order it makes them in decides both angles' high nibbles. Retail's
/// record 0 holds `17` / `33`; the pre-dig port, which stamped the
/// run inside `mc2_stamp_path_leg`, held `33` / `17`.
///
/// With `MGC_NO_MC2_PATH_POINT_DEFERRED_STAMP=1` this test fails on
/// the first assert.
#[test]
fn waterpath_point_stamps_at_its_own_settle_dispatch() {
    let Some(p) = settled(11, 8) else { return };
    assert_eq!(
        p.angle[cell(10, 73)],
        17,
        "mc2:11 angle(10,73) — the LAST of the run's three retile draws"
    );
    assert_eq!(
        p.angle[cell(10, 74)],
        33,
        "mc2:11 angle(10,74) — the SECOND of the run's three retile draws"
    );
}

/// ⭐ LAW 2 — THE ROAD STRIP CARRIES ACROSS THE MAP SEAM.
///
/// mc2:31's (8,67) → (245,67) road leg lays its second X run from
/// (255,67) with `dword_0x10_16 = 9`. Retail's stamped rows are
/// `(255,67) + (0..7, 68)` and `(255,68) + (0..7, 69)` — the run
/// steps onto the next row when the 16-bit index carries out of the
/// x byte. The pre-dig port wrapped x inside the row and laid
/// `(255..7, 67)` / `(255..7, 68)` instead, which is a +48 raise and
/// a type-8 stamp on the wrong rows.
///
/// With `MGC_NO_MC2_ROAD_STRIP_SEAM_CARRY=1` this test fails on the
/// first assert.
#[test]
fn road_strip_carries_into_the_next_row_at_the_map_seam() {
    let Some(p) = settled(31, 7) else { return };
    // Row 67 east of the seam is NOT part of the second run.
    assert_eq!(p.height[cell(0, 67)], 139, "mc2:31 height(0,67)");
    // Rows 68/69 are — 69 is the carried row the port used to miss.
    assert_eq!(p.height[cell(0, 69)], 183, "mc2:31 height(0,69)");
    assert_eq!(p.height[cell(7, 69)], 172, "mc2:31 height(7,69)");
    assert_eq!(p.tile_type[cell(0, 69)], 8, "mc2:31 type(0,69) = road");
    assert_eq!(p.tile_type[cell(0, 66)], 6, "mc2:31 type(0,66) — NOT road");
}

/// ⭐ LAW 3 — THE RISER FOLDS ITS SHADE IN A BYTE.
///
/// mc2:13 cell (148,177) sits where two risers overlap: slot 43
/// (orientation 1, base (146,159)) raises (147,176) to 48 and slot
/// 323 (orientation 0, base (145,176), L = 42) raises it again to 96
/// before running its own shading pass. The relief is
/// `96 - 0 + 32 = 128`, i.e. −128 as a signed byte, so retail folds
/// DARK — `(128 & 3) + 28 = 28` — and the cave's non-Day inversion
/// stores `64 − 28 = 36`. Folding in `i32` reads 128 as `> 40` and
/// stores `64 − ((128 & 7) + 40) = 24`.
///
/// With `MGC_NO_MC2_RISER_SHADE_BYTE_FOLD=1` this test fails.
#[test]
fn riser_shade_fold_is_a_signed_byte() {
    let Some(p) = settled(13, 11) else { return };
    // The relief that forces the fold: two stacked riser raises.
    assert_eq!(p.height[cell(147, 176)], 96, "mc2:13 height(147,176)");
    assert_eq!(p.height[cell(149, 178)], 0, "mc2:13 height(149,178)");
    assert_eq!(
        p.shading[cell(148, 177)],
        36,
        "mc2:13 shading(148,177) — the dark arm, 64 - ((128 & 3) + 28)"
    );
}
