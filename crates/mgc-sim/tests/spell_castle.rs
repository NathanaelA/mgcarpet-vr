//! Create Castle (spell 16) over real baked data: casting toward a
//! known-clear spot must launch the class-9 m10 castle ball, land it,
//! raise the class-3 m2 castle entity, and visibly run the m42
//! painter + m41 leveler (terrain planes change within a bounded
//! number of ticks). The traced chain replaced the old m45 house
//! approximation (playtest 3 — the castle never became visible).
//!
//! Self-skips when the baked tree is absent (game data is optional,
//! per the project rule).

use mgc_sim::features::{FeatureAssets, Planes};
use mgc_sim::spells::SpellId;
use mgc_sim::world::{PlayerCommand, PlayerPose, World};
use std::path::PathBuf;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    p.join("mc1/level-005.mgcl").exists().then_some(p)
}

fn build_world(root: &std::path::Path) -> World {
    let file = std::fs::File::open(root.join("mc1/level-005.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
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

/// First tile whose neighborhood — including the cast site 16 tiles
/// south (the launch-tick scan runs at the CAST position) — is dry
/// and free of the building-protection bit, so the asymmetric 8x8
/// scans (tx-8..tx-1) accept both the launch and the landing.
fn clear_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    'outer: for cy in (24..222u16).step_by(3) {
        for cx in (24..232u16).step_by(3) {
            for dy in -9i32..=25 {
                for dx in -9i32..=9 {
                    let t = ((cy as i32 + dy) as usize % 256) * 256
                        + ((cx as i32 + dx) as usize % 256);
                    // Protected (bit 7) or water (angle nibble 0).
                    if p.angle[t] & 0x80 != 0 || p.angle[t] & 0xF == 0 {
                        continue 'outer;
                    }
                }
            }
            return (cx, cy);
        }
        continue;
    }
    panic!("no clear 19x19 spot on the level");
}

#[test]
fn create_castle_builds_on_clear_ground() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let mut w = build_world(&root);
    w.set_dev_spells(true);

    let (cx, cy) = clear_spot(&w);
    // Hover 16 tiles south of the target, facing north (heading 0 =
    // -y): the castle ball targets 0x4000 units = 16 tiles ahead.
    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 16.5;
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    // Snapshot the target region's planes.
    let region: Vec<usize> = (-8i32..=8)
        .flat_map(|dy| {
            (-8i32..=8).map(move |dx| {
                ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256)
            })
        })
        .collect();
    let snap: Vec<(u8, u8, u8)> = region
        .iter()
        .map(|&t| (w.planes().height[t], w.planes().tile_type[t], w.planes().angle[t]))
        .collect();

    let count = |w: &World, class: u8, model: u8| {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == class && e.model == model)
            .count()
    };

    // Equip + press-cast (edge-triggered).
    w.tick(pose, PlayerCommand {
        equip_left: Some(SpellId(16)),
        ..Default::default()
    });
    w.tick(pose, PlayerCommand { fire_left: true, ..Default::default() });
    assert_eq!(count(&w, 9, 10), 1, "the cast launched the castle ball");

    // Ball flight (~11 ticks) + level-up + 20-tick painter + 10-tick
    // leveler; run past the 101-tick burst so the second press below
    // reaches the lockout gate, not the burst-spacing gate.
    let mut saw_castle = false;
    for _ in 0..110 {
        w.tick(pose, PlayerCommand::default());
        saw_castle |= count(&w, 3, 2) > 0;
    }
    assert!(saw_castle, "the ball landing raised the class-3 m2 castle");
    let changed = region.iter().zip(&snap).any(|(&t, &(h, ty, a))| {
        w.planes().height[t] != h
            || w.planes().tile_type[t] != ty
            || w.planes().angle[t] != a
    });
    assert!(changed, "the m42 painter flattened/painted the target region");

    // Single-active lockout: a second press while the player castle
    // lives is a message-free skip (:65862 surface behavior) — no
    // new ball appears on the cast tick.
    w.tick(pose, PlayerCommand::default()); // release the button
    w.tick(pose, PlayerCommand { fire_left: true, ..Default::default() });
    assert_eq!(count(&w, 9, 10), 0, "lockout: one player castle at a time");
}
