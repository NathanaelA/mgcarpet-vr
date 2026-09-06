//! Round 111 pin: the LOAD-TIME POOL tells retail's story. The chain
//! painters of GenerateEvents mint one-shot RECORDS — two (10,27)
//! segment-walkers per Bresenham step of a (10,28) road leg
//! (`sub_48400` EV:5365), two (10,30) path points per (10,29)
//! waterpath leg (`sub_48690` EV:5493) — that die on their first
//! load-settle sweep and are freed IN-SWEEP, in ascending slot order.
//! The next pass pops those slots LIFO, so the authored (10,45)
//! buildings of pass G sit in REVERSE THING order on the freed block,
//! and every later spawn of the same pass (mc2l4's five (10,11) scorch
//! rings, whose radius cadence is the slot-seeded `f63 % 3`) is
//! shifted by the records ahead of it. The port used to stamp the
//! strips without a record — mc2l22's 126 buildings at slots 1..=126
//! instead of retail's 184 → 59 (record 0 of mc2l22.mgcr), and the
//! pad-edge smoothing rings each building runs on completion (slot
//! order = sweep order) averaged different neighbours: 2,941 height /
//! 3,432 shading cells; mc2l4's rings dug one disc-tick too many /
//! two too few: 1,357 height cells. Retail's slots below are read
//! from the takes' record 0 (`mgc-conform dump-state <take> 0 …`).
mod common;

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use std::path::Path;

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
    let w = World::new_for_game_env(planes, &pkg.things.things, seed, assets, GameId::Mc2, night);
    Some((w, pkg))
}

/// The pool slot of every PARKED (10,45) building (state 52 — the
/// load-settled ones; the disposition-0 buildings are still raising
/// in state 51), keyed by its tile.
fn building_slots(w: &World) -> Vec<(usize, (u8, u8))> {
    let (_, pool) = w.debug_pool();
    pool.iter()
        .filter(|e| e.class == 10 && e.model == 45 && e.state == 52)
        .map(|e| (e.slot, (e.tx, e.ty)))
        .collect()
}

#[test]
fn mc2_level_022_road_walkers_put_the_buildings_on_slots_184_down_to_59() {
    let Some((w, _)) = load("level-022", "mc2-day") else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let slots = building_slots(&w);
    assert_eq!(slots.len(), 126, "the 126 load-time (10,45) buildings");
    let min = slots.iter().map(|s| s.0).min().unwrap();
    let max = slots.iter().map(|s| s.0).max().unwrap();
    assert_eq!((min, max), (59, 184), "21 road legs = 184 walker records ahead of them");
    // First THING (row 32 at (129,217)) → the HIGHEST slot; last THING
    // (row 26 at (83,2)) → the lowest: retail record 0, slots 184 / 59.
    let at = |tile: (u8, u8)| slots.iter().find(|s| s.1 == tile).map(|s| s.0);
    assert_eq!(at((129, 217)), Some(184));
    assert_eq!(at((83, 2)), Some(59));
    assert_eq!(at((74, 204)), Some(173), "the first row-50 wall piece");
}

#[test]
fn mc2_level_004_waterpath_points_shift_the_buildings_to_slots_13_down_to_11() {
    let Some((w, _)) = load("level-004", "mc2-day") else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let slots = building_slots(&w);
    let at = |tile: (u8, u8)| slots.iter().find(|s| s.1 == tile).map(|s| s.0);
    // Retail record 0 of mc2l4.mgcr: 11 (111,56) · 12 (121,48) · 13 (112,45).
    assert_eq!(at((112, 45)), Some(13));
    assert_eq!(at((121, 48)), Some(12));
    assert_eq!(at((111, 56)), Some(11));
}
