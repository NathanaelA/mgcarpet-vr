//! FRANKENSTEIN smoke test: real MC2 level data pushed through the
//! superset seam under the MC2 profile (MC2 chassis: 1200-slot THING
//! table, u16 entity LCG, 29 buckets, instant win latch; MC2 verb
//! column). The world builds against the real mc2-night bundle
//! (search/build/bldgprm + sprites — see `build_world` below).
//!
//! Guards: level-000 loads through the seam, ticks with NO crash,
//! produces a deterministic state stream, and the fallback ledger
//! stays exactly as pinned at the bottom (damage still notes the
//! shared player-intake fallback; awake/movement/objective serve MC2
//! natively).

use mgc_sim::ids::GameId;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc2/level-000.mgcl").exists() && p.join("assets/mc1-temperate").exists()).then_some(p)
}

fn build_world(root: &std::path::Path) -> Option<World> {
    let file = std::fs::File::open(root.join("mc2/level-000.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    // MC2 terrain planes are generated natively at bake time; a bundle
    // predating that has none, and there is nothing to smoke.
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: Vec::new(),
    };
    // The mc2-night bundle's own feature data (level-000 is night):
    // SEARCH + the BUILD0-0 footprint bank + BLDGPRM — the building
    // creator consumes all three (the app arranges the same).
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.sprites.as_ref() {
        Some((sidx, _)) => {
            let dims: Vec<(u16, u16)> = sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
            assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims))
        }
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).unwrap(),
        None => assets,
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    // Night map: runtime repaints invert relief shading (the app
    // sets the same from the level header).
    w.set_mc2_night_shade(true);
    Some(w)
}

/// Sweep the player across the level (waking creatures, probing
/// triggers) and collect state hashes.
fn run(root: &std::path::Path) -> Option<Vec<u64>> {
    let mut w = build_world(root)?;
    let mut hashes = vec![w.state_hash()];
    let idle = PlayerCommand::default();
    for leg in 0..8 {
        // A diagonal sweep in 8 legs, hovering near the ground.
        let (x, z) = (32.0 * leg as f32 + 16.0, 32.0 * leg as f32 + 16.0);
        for t in 0..40 {
            let (x, z) = (x + t as f32 * 0.8, z + t as f32 * 0.8);
            let alt = w.ground_height_tiles(x, z) + 2.0;
            w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), idle);
        }
        hashes.push(w.state_hash());
    }
    Some(hashes)
}

#[test]
fn mc2_level_through_the_seam_no_crash_deterministic() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let Some(got) = run(&root) else {
        common::golden_skip("mc2 level-000 has no baked terrain (genlevel oracle absent)");
        return;
    };
    // Bit-identical across runs: the MC2 chassis (u16 entity rand,
    // 1200-slot table) must be as deterministic as MC1's.
    assert_eq!(
        got,
        run(&root).unwrap(),
        "frankenstein is not deterministic"
    );

    // The census pass: fire EVERY disposition so all 311 authored
    // things cross the spawn seam (the sweep only trips whatever
    // trigger volumes it happens to overlap), then run the world on.
    //
    // KNOWN COLLISION: MC2's class-0 (Conditional Spawn) collides with
    // the MC1 table's class-0 EMPTY-SLOT SENTINEL — the disposition
    // scan skips those records entirely, so they can never reach the
    // misfit ledger through this arm. OPEN: the MC2 spawn column must
    // key emptiness differently.
    let mut w = build_world(&root).unwrap();
    let idle = PlayerCommand::default();
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }
    // 160 ticks: the intake seam sits behind the 100-tick spawn
    // grace (the grace branch wipes the mailbox before dispatch).
    for t in 0..160 {
        let (x, z) = (16.0 + t as f32 * 1.2, 16.0 + t as f32 * 1.2);
        let alt = w.ground_height_tiles(x, z) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), idle);
    }

    // The report (visible under --nocapture).
    println!("frankenstein profile: game={:?}", w.game());
    println!("misfits (class, model, count): {:?}", w.misfits());
    println!("live things after census: {}", w.live_things().len());

    // The MC2 registry admits the full creature roster including the
    // MULTIPART subsystem (0/3/22/27) — the honest misfits left are
    // (5,10)/(5,15), the class-10 effect middle band and class-15
    // spell tokens. The SLICE creatures spawn as live MC2 class-5
    // entities, and BUILDINGS spawn live (never ledgered).
    assert!(
        !w.misfits().iter().any(|&(c, m, _)| c == 10 && m == 45),
        "buildings are ported — the ledger must not contain (10,45): {:?}",
        w.misfits()
    );
    // Pool view, not live_poses: the building billboard is the owner
    // FLAG, drawn only once claimed — the entities exist regardless.
    assert!(
        w.debug_pool()
            .1
            .iter()
            .any(|e| e.class == 10 && e.model == 45),
        "buildings spawned as live entities"
    );
    assert!(
        w.misfits()
            .iter()
            .all(|&(c, m, _)| c != 5 || matches!(m, 10 | 15)),
        "every creature including the multipart family is ported — \
         only (5,10) and (5,15) may be ledgered: {:?}",
        w.misfits()
    );
    let poses = w.live_poses();
    for model in [1u8, 13u8] {
        assert!(
            poses.iter().any(|p| p.class == 5 && p.model == model),
            "slice creature model {model} should be alive"
        );
    }
    // The (5,3) multipart flyers spawn LIVE now — a head plus its
    // 16 state-0xE8 children ride the pose list.
    assert!(
        poses.iter().any(|p| p.class == 5 && p.model == 3),
        "the (5,3) multipart flyer spawns live"
    );

    // The fallback ledger: awake/movement/objective serve MC2 —
    // damage still falls back, and targeting only for the player's
    // MC1 spells (none cast in this run).
    let fallbacks = w.verb_fallbacks();
    println!("verb fallbacks exercised: {fallbacks:?}");
    assert!(
        fallbacks.contains(&"damage"),
        "the damage seam still serves MC1 (got {fallbacks:?})"
    );
    for verb in ["awake", "movement", "objective"] {
        assert!(
            !fallbacks.contains(&verb),
            "the {verb} seam should serve MC2 natively now (got {fallbacks:?})"
        );
    }
}
