//! Create Castle (spell 16) over real baked data: casting on a
//! known-clear spot must spawn the class-10 model-45 build event and
//! visibly run the progressive flatten/paint build (terrain planes
//! change within a bounded number of ticks).
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

/// First tile whose 13x13 neighborhood is dry and free of the
/// building-protection bit (the placement scan's angle bit 7) —
/// a spot where the 8x8 scan must accept the cast.
fn clear_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    'outer: for cy in (20..236u16).step_by(3) {
        for cx in (20..236u16).step_by(3) {
            for dy in -6i32..=6 {
                for dx in -6i32..=6 {
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
    panic!("no clear 13x13 spot on the level");
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
    // Hover 4 tiles south of the target, facing north (heading 0 =
    // -y): cast_castle targets 4 tiles ahead.
    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 4.5;
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    // Snapshot the target region's planes.
    let region: Vec<usize> = (-6i32..=6)
        .flat_map(|dy| {
            (-6i32..=6).map(move |dx| {
                ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256)
            })
        })
        .collect();
    let snap: Vec<(u8, u8, u8)> = region
        .iter()
        .map(|&t| (w.planes().height[t], w.planes().tile_type[t], w.planes().angle[t]))
        .collect();

    // The level's villages own class-10 m45 houses of their own (and
    // settlers build more over time) — assert on DELTAS around the
    // cast tick, not absolute counts.
    let m45 = |w: &World| {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 10 && e.model == 45)
            .count()
    };

    // Equip + press-cast (edge-triggered).
    w.tick(pose, PlayerCommand {
        equip_left: Some(SpellId(16)),
        ..Default::default()
    });
    let before = m45(&w);
    w.tick(pose, PlayerCommand { fire_left: true, ..Default::default() });
    assert_eq!(m45(&w), before + 1, "the cast spawned the m45 build event");

    // The progressive build (30-tick life, paint every 5th tick)
    // must visibly touch the planes. Run past the 101-tick burst so
    // the second press below reaches the lockout gate, not the
    // burst-spacing gate.
    for _ in 0..110 {
        w.tick(pose, PlayerCommand::default());
    }
    let changed = region.iter().zip(&snap).any(|(&t, &(h, ty, a))| {
        w.planes().height[t] != h
            || w.planes().tile_type[t] != ty
            || w.planes().angle[t] != a
    });
    assert!(changed, "the build flattened/painted the target region");

    // Single-active lockout: a second press while the player castle
    // lives is a message-free skip (:65862) — no new m45 appears on
    // the cast tick.
    w.tick(pose, PlayerCommand::default()); // release the button
    let before2 = m45(&w);
    w.tick(pose, PlayerCommand { fire_left: true, ..Default::default() });
    assert_eq!(m45(&w), before2, "lockout: one player castle at a time");
}
