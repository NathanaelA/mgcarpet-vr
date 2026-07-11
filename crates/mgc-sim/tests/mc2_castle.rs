//! The MC2-NATIVE CASTLE COLUMN over real baked level-000 data
//! (mc2::castle — the Phase-4 port of retail actions 4/5/6): the
//! NATIVE Create-Castle cast (the 4.2 book, spell 2) must raise a
//! class-3 m2 castle that runs the MC2 machinery — the 19-tick
//! (10,42) painter stamps the tower, the MC2 capacity ladder rungs
//! (8500/18000 — NOT MC1's 10000/20000) prove the game-keyed swap,
//! the (10,43) token recast upgrades one level, the (3,3) balloon
//! fleet spawns to quota, and demolish walks the level back down to
//! a barren, unprotected square.
//!
//! Self-skips without baked mc2 data (game data is optional).

use mgc_sim::ids::GameId;
use mgc_sim::mc1::features::{FeatureAssets, Planes};
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};
use std::path::PathBuf;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc2/level-000.mgcl").exists() && p.join("assets/mc2-night/build.tab.bin").exists())
        .then_some(p)
}

fn build_world(root: &std::path::Path) -> Option<World> {
    let file = std::fs::File::open(root.join("mc2/level-000.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp).unwrap();
    }
    if let Some((sidx, _)) = bundle.sprites.as_ref() {
        let dims: Vec<(u16, u16)> = sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    w.set_mc2_night_shade(true);
    Some(w)
}

/// First tile whose neighborhood — including the cast site 16 tiles
/// south — is dry and free of the building-protection bit (the same
/// scan as the MC1 castle test).
fn clear_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    for cy in (24..222u16).step_by(3) {
        'cand: for cx in (24..232u16).step_by(3) {
            for dy in -9i32..=25 {
                for dx in -9i32..=9 {
                    let t =
                        ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256);
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

fn count(w: &World, class: u8, model: u8) -> usize {
    w.debug_pool()
        .1
        .iter()
        .filter(|e| e.class == class && e.model == model && e.life >= 0)
        .count()
}

#[test]
fn mc2_castle_builds_upgrades_and_demolishes() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);

    let (cx, cy) = clear_spot(&w);
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

    // The NATIVE castle cast (4.2): bind the MC2 castle spell
    // (index 2, dev-granted above) — the retired MC1 equip bridge
    // no longer casts on the MC2 column (the playtest-13
    // ghost-fireball gate).
    w.mc2_select_spell(2, 0, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    w.tick(pose, PlayerCommand::default());
    assert_eq!(count(&w, 9, 10), 1, "the cast launched the castle ball");

    // Ball flight + level-up + the 19-tick MC2 painter + settle.
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
        "the MC2 (10,42) painter stamped the castle footprint"
    );

    // THE LADDER DISCRIMINATOR: MC2 level-1 capacity = 8500
    // (sub_60810 EF:61710) — MC1's rung is 10000. A bridge castle
    // still running the MC1 column would report 10000 here.
    let (_, cap1, lvl1) = w.loadout().castle.expect("castle panel data");
    assert_eq!(
        (lvl1, cap1),
        (1, 8_500),
        "level 1 with the MC2 capacity rung"
    );

    // The balloon fleet: level-1 quota = 1 (sub_60400 EF:61529) —
    // the roster pass spawns it from the standing tick.
    assert_eq!(count(&w, 3, 3), 1, "one (3,3) balloon at level 1");

    // RECAST = the upgrade: the ball morphs into the (10,43) token
    // at the castle, the token mails the upgrade-request channel
    // (retail word_0x80_128/word_0x7C_124 — sub_389F0 EF:28240),
    // the castle re-runs the level-up arm.
    w.tick(pose, PlayerCommand::default()); // release the button
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(count(&w, 9, 10), 1, "the recast launches the upgrade ball");
    for _ in 0..200 {
        w.tick(pose, PlayerCommand::default());
    }
    let (_, cap2, lvl2) = w.loadout().castle.expect("castle survives the upgrade");
    assert_eq!(
        (lvl2, cap2),
        (2, 18_000),
        "the token upgrade raised level 2 on the MC2 ladder"
    );

    // Climb to level 6 (the 48x48 stage) — four more recasts. The
    // even-frame/odd-row origin math is the PLAYTEST-11 round-3
    // regression (retail: origin = D/2 - d/2, EF:27798): the old
    // (D-d)/2 read shifted every interior ring one tile toward
    // -x/-y — offset walkways, a squashed center tower, and castle
    // guards spawning inside wall cells where the all-four-blocked
    // walker law killed them in a respawn loop.
    for expect in 3..=6i16 {
        w.tick(pose, PlayerCommand::default());
        w.tick(
            pose,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        for _ in 0..220 {
            w.tick(pose, PlayerCommand::default());
        }
        let (_, _, lvl) = w.loadout().castle.expect("castle survives the upgrade");
        assert_eq!(lvl, expect as u8, "recast raised level {expect}");
    }
    let (_, cap6, _) = w.loadout().castle.expect("castle at level 6");
    assert_eq!(cap6, 317_400, "the MC2 level-6 capacity rung");
    // The guard roster survives on the (correctly aligned) walkways
    // — with the one-tile ring offset they died as fast as they
    // spawned (blocked on all four sides).
    for _ in 0..200 {
        w.tick(pose, PlayerCommand::default());
    }
    assert!(
        count(&w, 5, 15) >= 3,
        "castle guards survive on the level-6 walkways (got {})",
        count(&w, 5, 15)
    );

    // Demolish walks ONE level down per press (life = -1 → intake 2
    // → action 6 → sub_605E0), all the way to a barren unprotected
    // square (RemoveCastleStage model-0 arm).
    let castle = w
        .debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 3 && e.model == 2)
        .expect("castle entity lives");
    let (tx, ty) = (castle.tx as i32, castle.ty as i32);
    w.tick(
        pose,
        PlayerCommand {
            demolish: true,
            ..Default::default()
        },
    );
    for _ in 0..60 {
        w.tick(pose, PlayerCommand::default());
    }
    let (_, cap_d, lvl_d) = w.loadout().castle.expect("castle survives one demolish");
    assert_eq!(
        (lvl_d, cap_d),
        (5, 158_200),
        "one demolish = one level down (the MC2 downgrade)"
    );
    for _ in 0..5 {
        w.tick(
            pose,
            PlayerCommand {
                demolish: true,
                ..Default::default()
            },
        );
        for _ in 0..60 {
            w.tick(pose, PlayerCommand::default());
        }
    }
    assert_eq!(count(&w, 3, 2), 0, "the level-1 demolish killed the castle");
    for dy in -4i32..=4 {
        for dx in -4i32..=4 {
            let t = ((ty + dy) as usize % 256) * 256 + ((tx + dx) as usize % 256);
            assert_eq!(
                w.planes().angle[t] & 0x80,
                0,
                "no protection bit lingers at ({dx},{dy})"
            );
        }
    }
}
