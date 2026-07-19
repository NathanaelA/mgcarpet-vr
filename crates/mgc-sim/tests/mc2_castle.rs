//! The MC2-NATIVE CASTLE COLUMN over real baked level-000 data
//! (mc2::castle — retail actions 4/5/6): the NATIVE Create-Castle cast
//! (spell 2) must raise a class-3 m2 castle that runs the MC2
//! machinery — the 19-tick (10,42) painter stamps the tower, the MC2
//! capacity ladder rungs
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

/// A tier-1 castle cast must grow FIRE turrets — the (10,79) ring with
/// part-type 1 — via the cast-time research stamp; the dev-granted
/// spell must behave exactly like a legitimately leveled one.
#[test]
fn mc2_castle_tier1_cast_grows_fire_turrets() {
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

    // Bind the castle spell at TIER 1 (fire) and build.
    w.mc2_select_spell(2, 1, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    for _ in 0..120 {
        w.tick(pose, PlayerCommand::default());
    }
    let (_, _, lvl) = w.loadout().castle.expect("castle stands");
    assert_eq!(lvl, 1, "level-1 castle built");
    assert_eq!(
        count(&w, 10, 79),
        1,
        "the tier-1 build grows the stage-1 turret"
    );

    // Recast = upgrade to level 2: the 4-corner ring.
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
    let (_, _, lvl2) = w.loadout().castle.expect("castle survives");
    assert_eq!(lvl2, 2, "upgraded to level 2");
    assert_eq!(count(&w, 10, 79), 4, "level 2 grows the 4-turret ring");
}

/// The level's ending cluster is the CHECKPOINT variant — dis 4 spawns
/// the (11,12) X-marker trigger at (75,218) and the (14,3) fly-to
/// "X"/portal at (97,221) (there is NO (11,31)/(14,4) on level-000;
/// retail routes this through the same endGameSeq under actionIndex
/// 12). The marker must spawn HIDDEN, the trip must REVEAL it and
/// seize the flyer, and the fly-in must end in WON.
#[test]
fn mc2_level000_ending_end_to_end() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.debug_fire_disposition(4);
    w.tick(
        PlayerPose::from_tiles(10.0, 5.0, 10.0, 0.0, 0.0, 0.0),
        PlayerCommand::default(),
    );
    let pool = w.debug_pool().1;
    let trig = pool
        .iter()
        .find(|e| e.class == 11 && e.model == 12 && e.life >= 0)
        .expect("dis 4 spawns the (11,12) ending trigger");
    let marker = pool
        .iter()
        .find(|e| e.class == 14 && e.model == 3 && e.life >= 0)
        .expect("dis 4 spawns the (14,3) fly-to marker");
    assert!(
        !w.live_poses().iter().any(|p| p.class == 14 && p.model == 3),
        "the ending marker spawns HIDDEN (not drawable) until the trip"
    );
    eprintln!(
        "trigger at ({},{}), marker at ({},{})",
        trig.tx, trig.ty, marker.tx, marker.ty
    );
    // Park on the trigger; the 8-tick phase gate opens quickly.
    let (tx, ty) = (trig.tx as f32 + 0.5, trig.ty as f32 + 0.5);
    let alt = w.ground_height_tiles(tx, ty) + 1.0;
    let pose = PlayerPose::from_tiles(tx, alt, ty, 0.0, 0.0, 0.0);
    let mut seized_at = None;
    for t in 0..40 {
        w.tick(pose, PlayerCommand::default());
        if w.mc2_end_pose().is_some() {
            seized_at = Some(t);
            break;
        }
    }
    assert!(
        seized_at.is_some(),
        "the trigger trip seizes the flyer (endGameSeq installs)"
    );
    assert!(
        w.live_poses().iter().any(|p| p.class == 14 && p.model == 3),
        "the trip REVEALS the fly-to marker"
    );
    assert!(!w.won(), "the trip alone must not end the level");
    let mut won_at = None;
    for t in 0..2000 {
        w.tick(pose, PlayerCommand::default());
        if w.won() {
            won_at = Some(t);
            break;
        }
    }
    assert!(won_at.is_some(), "the fly-in ends the level (won)");
    let (ex, _, ez, _) = w.mc2_end_pose().expect("pose holds through the end");
    let d = ((ex - (marker.tx as f32)).powi(2) + (ez - (marker.ty as f32)).powi(2)).sqrt();
    assert!(
        d < 4.0,
        "the scripted carpet stopped at the marker (dist {d:.1} tiles)"
    );
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

    // The NATIVE castle cast: bind the MC2 castle spell (index 2,
    // dev-granted above) — the MC1 equip bridge does NOT cast on the
    // MC2 column (else ghost fireballs).
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
    // even-frame/odd-row origin math: retail origin = D/2 - d/2
    // (EF:27798), NOT (D-d)/2 — the wrong read shifts every interior
    // ring one tile toward -x/-y (offset walkways, a squashed center
    // tower, and castle guards spawning inside wall cells where the
    // all-four-blocked walker law kills them in a respawn loop).
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
    // Each upgrade awards +1 castle XP (`sub_6D8B0(owner,2,1)`
    // EF:61596) — five upgrades landed above (2..=6), so the ladder
    // that unlocks Fire/Lightning Tower tiers has climbed, and the XP
    // drain's spell-2 branch keeps the pane cost synced.
    let book = w.mc2_book_view();
    assert!(
        book.xp[2] >= 5,
        "castle upgrades awarded spell-2 XP (got {})",
        book.xp[2]
    );
    // The guard roster survives on the (correctly aligned) walkways —
    // a one-tile ring offset would block them on all four sides and
    // kill them as fast as they spawn.
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

/// The mana gate reads the manifestation's CACHED cost (`max_life`,
/// written by SetSpell_6D5E0), and retail refreshes that cache on
/// EVERY castle stat stamp — `sub_60780` (EF:61670) re-runs SetSpell
/// on the manifestation's own tier from both transform directions.
/// The upgrade path stays fresh via the +1 XP award's spell-2 branch,
/// but a DOWNGRADE (demolish / enemy razing) awards nothing, so the
/// cost cache must re-sync at the upgrade-lock release edge — else an
/// affordable rebuild dings (sound 29) against the stale higher rung
/// until the spell is re-selected.
///
/// The assert surface is the CACHE (`debug_spell_gate_cost`) against
/// the live law (`mc2_book_view().cost`) — the bug is exactly their
/// divergence. (A full end-to-end cast can't be driven here: the
/// per-tick mana census re-derives the pool ceiling from claimed
/// world mana, and this harness world has none to claim.)
#[test]
fn mc2_castle_cost_refreshes_on_downgrade() {
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

    // Build to level 2 under the dev instrument (gate bypassed).
    w.mc2_select_spell(2, 0, 0);
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
    let (_, _, lvl) = w.loadout().castle.expect("castle stands");
    assert_eq!(lvl, 2, "harness built to level 2");

    // Real-mana mode; the re-select recomputes the honest level-2
    // cache: the NEXT build (level 3) = ladder rung 20000.
    w.set_dev_spells(false);
    w.mc2_select_spell(2, 0, 0);
    assert_eq!(
        w.debug_spell_gate_cost(2),
        Some(20_000),
        "at level 2 the cached gate cost is the level-3 rung"
    );

    // A failed cast attempt (the ding) — it must not perturb the cache
    // or wedge any state.
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(count(&w, 9, 10), 0, "unaffordable: no castle ball");
    w.tick(pose, PlayerCommand::default()); // release the button

    // Demolish one level; the downgrade transform raises and then
    // releases the upgrade lock — the sub_60780 cost re-sync rides
    // the release edge.
    w.tick(
        pose,
        PlayerCommand {
            demolish: true,
            ..Default::default()
        },
    );
    for _ in 0..90 {
        w.tick(pose, PlayerCommand::default());
    }
    let (_, _, lvl) = w.loadout().castle.expect("castle survives the demolish");
    assert_eq!(lvl, 1, "one demolish = one level down");

    // The cache must track the live law back DOWN to the level-2 rung
    // with NO re-select in between (a stale higher rung would ding an
    // affordable rebuild).
    assert_eq!(
        w.mc2_book_view().cost[2],
        10_000,
        "the live law prices the level-2 rebuild at the level-1 rung"
    );
    assert_eq!(
        w.debug_spell_gate_cost(2),
        Some(10_000),
        "the cached gate cost re-synced on the downgrade (no re-select)"
    );
}

/// The pane grey-out law (`canSummon`/`canSubSummon`, EF:22503-08 /
/// EF:22602-08): a tier whose `maxManaLimit_A` castle-pool
/// prerequisite is nonzero must read NOT castable while no own castle
/// exists; requirement-free tiers (fireball) always read castable.
#[test]
fn mc2_pane_castable_reflects_castle_gate() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    let bv = w.mc2_book_view();
    // The CD table's own shape: the BASE fireball is requirement-free,
    // but its tier 2 carries a nonzero `maxManaLimit_A` — even
    // fireball's top tier greys castle-less.
    assert_eq!(
        bv.castable[0],
        [true, true, false],
        "fireball: base/repeat lit, tier 2 castle-gated (SPELLS.DAT)"
    );
    assert!(
        bv.castable.iter().flatten().any(|c| !c),
        "castle-less world: at least one castle-gated tier reads grey"
    );
}
