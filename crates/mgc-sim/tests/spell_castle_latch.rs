//! The `castle_latch_bug` patch arms, pinned against the certified
//! retail take `recordings/mc1l32-castle-bug.mgcr` (level 032, the
//! no-castle portal maze): parked at the recorded pose, retail builds
//! a castle ON the maze wall from one aim and fizzles a 10°-different
//! one — the hand-muzzle anchor + the NW-only window + the touchdown
//! short-circuit. The patched arm (native default) anchors at the
//! carpet and always re-scans the landing.
//!
//! Self-skips when the baked tree is absent.

use mgc_sim::WorldPatches;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc1::spells::SpellId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc1/level-032.mgcl").exists() && !common::modded_bake(&p)).then_some(p)
}

fn build_world(root: &std::path::Path) -> World {
    let file = std::fs::File::open(root.join("mc1/level-032.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: Vec::new(),
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    World::new(planes, &pkg.things.things, seed, assets)
}

/// The recorded carpet pose (raw engine units, t=376/391): parked
/// glued to the wall corner, ~4° pitch-up, castle in the LEFT hand.
fn recorded_pose(heading: u16) -> PlayerPose {
    PlayerPose {
        x: 3572,
        y: 60156,
        z: 7239,
        heading,
        pitch: 2023,
        speed: 80,
    }
}

fn castle_tile(w: &World) -> Option<(u8, u8)> {
    w.debug_pool()
        .1
        .iter()
        .find(|e| e.class == 3 && e.model == 2)
        .map(|e| (e.tx, e.ty))
}

/// Equip castle in the left hand, press-cast, run `ticks`.
fn cast_and_run(w: &mut World, pose: PlayerPose, ticks: usize) {
    w.tick(
        pose,
        PlayerCommand {
            equip_left: Some(SpellId(16)),
            ..Default::default()
        },
    );
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    for _ in 0..ticks {
        w.tick(pose, PlayerCommand::default());
    }
}

/// The recorded SUCCESS (t=392, aim yaw 530): the left-hand muzzle
/// crosses into tile (14,233) — the corner's one clean-window anchor
/// — the launch passes, the ball grounds into the wall face on its
/// first flight tick, and the touchdown short-circuit builds the
/// castle UNSCANNED at the snap tile (16,234), ON the wall; the
/// painter then carves the protected maze wall.
#[test]
fn castle_latch_retail_arm_reproduces_the_recorded_maze_castle() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let mut w = build_world(&root);
    w.set_dev_spells(true);
    let wall_h = w.planes().height[234 * 256 + 16];
    assert_eq!(wall_h, 244, "the target wall stands before the cast");

    cast_and_run(&mut w, recorded_pose(530), 110);
    assert_eq!(
        castle_tile(&w),
        Some((16, 234)),
        "the recorded cast raises the castle ON the wall tile"
    );
    assert_ne!(
        w.planes().height[234 * 256 + 16],
        244,
        "the painter carved the protected wall"
    );
}

/// The recorded FIZZLE (t=377, aim yaw 590): same parking spot, the
/// muzzle lands one tile south — tile (14,234), whose NW window
/// catches the protection skirt at (6,233)/(7,233) — and the launch
/// scan despawns the ball silently.
#[test]
fn castle_latch_retail_arm_fizzles_the_first_recorded_aim() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let mut w = build_world(&root);
    w.set_dev_spells(true);
    cast_and_run(&mut w, recorded_pose(590), 60);
    assert_eq!(castle_tile(&w), None, "the yaw-590 launch scan refuses");
}

/// The PATCHED arm (native default) anchors the launch scan at the
/// CARPET tile (13,234) — poisoned by the x=5 skirt column for every
/// aim — so the recorded cheese spot never launches.
#[test]
fn castle_latch_patched_arm_refuses_the_recorded_cast() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let mut w = build_world(&root);
    w.set_patches(WorldPatches {
        castle_latch_bug: true,
        ..WorldPatches::RETAIL
    });
    w.set_dev_spells(true);
    for heading in [530u16, 590] {
        cast_and_run(&mut w, recorded_pose(heading), 60);
        assert_eq!(
            castle_tile(&w),
            None,
            "the patched arm refuses the recorded cheese (yaw {heading})"
        );
    }
}

/// The patched arm's residual (documented, DEVIATIONS.md): parked
/// INSIDE tile (14,233) — the corner's clean-window anchor — the
/// carpet-anchored launch passes; the landing re-scan refuses the
/// wall and displaces the build one step back into the corridor at
/// (14,234). The retail arm from the SAME park anchors at the hand
/// muzzle one tile north instead — tile (14,232), whose window is
/// clean once the corridor statics' early protection churn clears
/// row 224 — and its ball crosses the wall face, so the castle
/// rises BEYOND the wall at the odd-parity snap (17,233): the two
/// arms build on opposite sides of the "impenetrable" wall.
#[test]
fn castle_latch_patched_arm_keeps_the_carpet_anchored_corridor_build() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let park = PlayerPose {
        x: 3648,
        y: 59776,
        z: 7239,
        heading: 530,
        pitch: 2023,
        speed: 80,
    };
    let mut w = build_world(&root);
    w.set_patches(WorldPatches {
        castle_latch_bug: true,
        ..WorldPatches::RETAIL
    });
    w.set_dev_spells(true);
    cast_and_run(&mut w, park, 110);
    assert_eq!(
        castle_tile(&w),
        Some((14, 234)),
        "the corridor park builds displaced into the corridor"
    );

    let mut w = build_world(&root);
    w.set_dev_spells(true);
    cast_and_run(&mut w, park, 60);
    assert_eq!(
        castle_tile(&w),
        Some((17, 233)),
        "the retail-arm hand anchor builds beyond the wall"
    );
}
