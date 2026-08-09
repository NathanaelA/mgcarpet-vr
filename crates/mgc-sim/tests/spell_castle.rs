//! Create Castle (spell 16) over real baked data: casting toward a
//! known-clear spot must launch the class-9 m10 castle ball, land it,
//! raise the class-3 m2 castle entity, and visibly run the m42
//! painter + m41 leveler (terrain planes change within a bounded
//! number of ticks).
//!
//! Self-skips when the baked tree is absent (game data is optional).

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc1::spells::SpellId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc1/level-005.mgcl").exists() && !common::modded_bake(&p)).then_some(p)
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

/// First tile whose neighborhood — including the cast site 16 tiles
/// south (the launch-tick scan runs at the CAST position) — is dry
/// and free of the building-protection bit, so the asymmetric 8x8
/// scans (tx-8..tx-1) accept both the launch and the landing.
fn clear_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    for cy in (24..222u16).step_by(3) {
        'cand: for cx in (24..232u16).step_by(3) {
            for dy in -9i32..=25 {
                for dx in -9i32..=9 {
                    let t =
                        ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256);
                    // Protected (bit 7) or water (angle nibble 0).
                    if p.angle[t] & 0x80 != 0 || p.angle[t] & 0xF == 0 {
                        continue 'cand;
                    }
                }
            }
            return (cx, cy);
        }
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
        .map(|&t| {
            (
                w.planes().height[t],
                w.planes().tile_type[t],
                w.planes().angle[t],
            )
        })
        .collect();

    let count = |w: &World, class: u8, model: u8| {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == class && e.model == model)
            .count()
    };

    // Equip + press-cast (edge-triggered).
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
    w.tick(pose, PlayerCommand::default()); // the token fires at arm+1
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
        w.planes().height[t] != h || w.planes().tile_type[t] != ty || w.planes().angle[t] != a
    });
    assert!(
        changed,
        "the m42 painter flattened/painted the target region"
    );

    // The leveler is a uniform TRANSLATION of the footprint
    // (sub_28200 adds the same per-tick step to every tile) — the
    // painted tower must survive it (not flattened to perimeter mean).
    let castle = w
        .debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 3 && e.model == 2)
        .expect("castle entity lives");
    let (tx, ty) = (castle.tx as i32, castle.ty as i32);
    let (mut hmin, mut hmax) = (255u8, 0u8);
    for dy in -4i32..=4 {
        for dx in -4i32..=4 {
            let t = ((ty + dy) as usize % 256) * 256 + ((tx + dx) as usize % 256);
            let h = w.planes().height[t];
            hmin = hmin.min(h);
            hmax = hmax.max(h);
        }
    }
    assert!(
        hmax - hmin >= 12,
        "the tower relief survives the leveler (min {hmin}, max {hmax})"
    );

    let (_, cap1, lvl1) = w.loadout().castle.expect("castle panel data");
    assert_eq!((lvl1, cap1), (1, 10_000), "level 1, capacity ladder rung 1");

    // The RECAST on a standing castle is the UPGRADE (:65904-08):
    // a new ball launches carrying the (10,43) token morph, the
    // token mails the castle's ch5 (:31033-34), the castle re-runs
    // the level-up arm — level 2, capacity 20000 (sub_47DD0).
    w.tick(pose, PlayerCommand::default()); // release the button
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    w.tick(pose, PlayerCommand::default()); // the token fires at arm+1
    assert_eq!(count(&w, 9, 10), 1, "the recast launches the upgrade ball");
    for _ in 0..200 {
        w.tick(pose, PlayerCommand::default());
    }
    let (_, cap2, lvl2) = w.loadout().castle.expect("castle survives the upgrade");
    assert_eq!(
        (lvl2, cap2),
        (2, 20_000),
        "the token upgrade raised level 2"
    );
}

/// The castle-spell UPGRADE LOCK (`f26`) tracks the castle TRANSFORM,
/// not a fixed `count` (101-tick) timer — the same law as MC2
/// (`mc2_castle_spell_tick`). The lock must engage during the build and
/// clear the moment the castle is ESTABLISHED (`f59 == 4`), well before
/// 101 ticks.
#[test]
fn mc1_castle_spell_lock_tracks_the_build_not_a_fixed_timer() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let mut w = build_world(&root);
    w.set_dev_spells(true);
    let (cx, cy) = clear_spot(&w);
    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 16.5;
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    assert_eq!(w.debug_castle_lock(), 0, "lock clear before casting");
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
    let mut active_ticks = 0usize;
    let mut cleared_at = None;
    for t in 0..101 {
        w.tick(pose, PlayerCommand::default());
        if w.debug_castle_lock() > 0 {
            active_ticks += 1;
        } else if active_ticks > 0 && cleared_at.is_none() {
            cleared_at = Some(t);
            break;
        }
    }
    assert!(active_ticks > 0, "the lock engaged during the build");
    let cleared = cleared_at.expect("the lock cleared after the build");
    assert!(
        cleared < 101,
        "the lock cleared at tick {cleared} with the build, not a 101-tick timer"
    );
    // Established → lock clear (the between-transformations window).
    for _ in 0..20 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(
        w.debug_castle_lock(),
        0,
        "the lock stays clear once the castle is established (f59 == 4)"
    );
}

/// The FINAL destruction (level 1 → 0) must leave a barren square —
/// the un-stamp collapse reverses the painted tower
/// and walls; no stump may survive (and the renderer is told via
/// terrain_dirty, asserted in the unit suite).
#[test]
fn final_destruction_flattens_the_tower_to_a_barren_square() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let mut w = build_world(&root);
    w.set_dev_spells(true);

    let (cx, cy) = clear_spot(&w);
    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 16.5;
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    let count = |w: &World, class: u8, model: u8| {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == class && e.model == model)
            .count()
    };

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
    for _ in 0..110 {
        w.tick(pose, PlayerCommand::default());
    }
    let castle = w
        .debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 3 && e.model == 2)
        .expect("the castle stands");
    let (tx, ty) = (castle.tx as i32, castle.ty as i32);
    let relief = |w: &World| {
        let (mut hmin, mut hmax) = (255u8, 0u8);
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                let t = ((ty + dy) as usize % 256) * 256 + ((tx + dx) as usize % 256);
                let h = w.planes().height[t];
                hmin = hmin.min(h);
                hmax = hmax.max(h);
            }
        }
        (hmin, hmax)
    };
    let (_, towered) = relief(&w);
    let (base_min, _) = relief(&w);
    assert!(
        towered - base_min >= 12,
        "the tower relief stands before demolish"
    );

    // One demolish at level 1 = total destruction.
    w.tick(
        pose,
        PlayerCommand {
            demolish: true,
            ..Default::default()
        },
    );
    for _ in 0..40 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(count(&w, 3, 2), 0, "the castle entity is gone");
    let (fmin, fmax) = relief(&w);
    assert!(
        fmax - fmin < 12,
        "barren square: no tower stump survives (min {fmin}, max {fmax})"
    );
    // No protection bits linger on the footprint (the square is
    // ordinary ground again).
    for dy in -4i32..=4 {
        for dx in -4i32..=4 {
            let t = ((ty + dy) as usize % 256) * 256 + ((tx + dx) as usize % 256);
            assert_eq!(w.planes().angle[t] & 0x80, 0, "unprotected at ({dx},{dy})");
        }
    }
}

/// An AUTHORED rival castle's terrain must belong to its LEVEL.
///
/// Retail stamps one build pass per authored level with the row = the
/// pass index (`+29866 = i`, i = 0..count-1, :54983-91), so the rows
/// raised are 0..=level — and BUILD row 0 is EMPTY (w = h = 0). A
/// `castle_level` of 1 is therefore level 0: a bare flag that owns no
/// terrain at all. The port used to stamp rows 1..=level+1, raising a
/// tower the castle did not own; the demolish walks the row that
/// matches the LEVEL, so the extra ring outlived the castle as a
/// flagless stump (glaring on the mc2:06 ocean site the player found,
/// and the same off-by-one drove the MC1 sightings).
#[test]
fn an_authored_castle_owns_only_its_own_levels_terrain() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    for castle_level in [1u8, 2, 3] {
        let mut w = build_world(&root);
        let before = w.planes().height.to_vec();
        let mut cfgs: [Option<mgc_sim::mc1::rivals::RivalConfig>; 8] = Default::default();
        let mut book = [false; 24];
        book[0] = true;
        book[16] = true; // Castle: the starting-castle gate
        // tempo 0, NOT 255: the castle-rebuild lockout is
        // 32*((255-tempo)/8)+32 ticks (rivals.rs, :55552-57), so a
        // fast rival raises a REPLACEMENT castle inside the settle
        // window below — and then both assertions measure the new
        // castle instead of a stump left by the old one. At tempo 0
        // the lockout is 1024 ticks, well past this fixture. The
        // preplant itself comes from `castle_level` + book[16] and
        // does not depend on tempo.
        cfgs[1] = Some(mgc_sim::mc1::rivals::RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 0,
            castle_level,
            book,
            allowed: book,
        });
        w.set_wizards(&cfgs, 2);
        let Some(castle) = w
            .debug_pool()
            .1
            .into_iter()
            .find(|e| e.class == 3 && e.model == 2)
        else {
            eprintln!("skipping: level 005 spawns no rival castle");
            return;
        };
        // The authored level is castle_level - 1, and a level-0
        // castle has raised nothing yet.
        if castle_level == 1 {
            assert_eq!(
                w.planes().height,
                before.as_slice(),
                "a bare-flag castle (castle_level 1 = level 0) stamps no terrain"
            );
        }
        // Settle the build, then destroy it outright.
        let pose = PlayerPose::from_tiles(8.5, 40.0, 8.5, 0.0, 0.0, 0.0);
        for _ in 0..200 {
            w.tick(pose, PlayerCommand::default());
        }
        // One lethal knocks ONE level off, and damage is only
        // processed from the established sub-state — so hit, let the
        // demolish + repaint settle, hit again.
        for _ in 0..40 {
            w.debug_mail_hit(castle.slot, 60000, 1);
            for _ in 0..40 {
                w.tick(pose, PlayerCommand::default());
            }
            if !w
                .debug_pool()
                .1
                .iter()
                .any(|e| e.slot == castle.slot && e.class == 3 && e.model == 2)
            {
                break;
            }
        }
        for _ in 0..60 {
            w.tick(pose, PlayerCommand::default());
        }
        assert!(
            !w.debug_pool()
                .1
                .iter()
                .any(|e| e.slot == castle.slot && e.class == 3 && e.model == 2),
            "castle_level {castle_level}: the castle died"
        );
        // Nothing the castle raised may outlive it. The collapse
        // leaves ordinary rubble, so allow a little settling noise —
        // a surviving tower ring is 40+ units, far above this.
        let (tx, ty) = (castle.tx as i32, castle.ty as i32);
        for dy in -12i32..=12 {
            for dx in -12i32..=12 {
                let t = ((ty + dy) as usize % 256) * 256 + ((tx + dx) as usize % 256);
                let (now, was) = (w.planes().height[t] as i32, before[t] as i32);
                assert!(
                    now - was < 12,
                    "castle_level {castle_level}: stump left at ({dx},{dy}) — {was} → {now}"
                );
            }
        }
    }
}
