//! MC2 armed-window channel behaviors (the spell-verification track,
//! playtest follow-ups 2026-07-13):
//!
//! - **Invisibility (11) break-on-self-cast law** (`sub_5F7E0`
//!   EF:60987): T0 (any cast) breaks the cloak, T2 (nothing) survives.
//!   On break the invis window's burst `f26` is zeroed too, so the
//!   mana-regen block lifts with the cloak — observable here as
//!   `mc2_book_view().armed[11]` flipping false.
//! - **Speed (3) interruptible window** (`GetScroll_69DB0`,
//!   docs/spell-audit/speed.md): a BRAKE input cancels the window early
//!   (player 2026-07-14: Speed flies far past where you need). The
//!   interrupt zeroes the burst timer `f26` too, so the mana-regen
//!   suppression lifts with the boost — a forward press does not cancel.
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

/// A dry, unprotected tile to fly over (same scan as the castle test,
/// minus the 19×19 footprint — these spells need only a valid pose).
fn open_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    for cy in (24..222u16).step_by(3) {
        for cx in (24..232u16).step_by(3) {
            let t = (cy as usize % 256) * 256 + (cx as usize % 256);
            if p.angle[t] & 0x80 == 0 && p.angle[t] & 0xF != 0 {
                return (cx, cy);
            }
        }
    }
    panic!("no open spot on the level");
}

fn pose_at(w: &World, cx: u16, cy: u16) -> PlayerPose {
    let (px, pz) = (cx as f32 + 0.5, cy as f32 + 0.5);
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0)
}

fn count(w: &World, class: u8, model: u8) -> usize {
    w.debug_pool()
        .1
        .iter()
        .filter(|e| e.class == class && e.model == model && e.life >= 0)
        .count()
}

#[test]
fn mc2_possession_magnet_needs_a_mana_claim() {
    // Mana Magnet (Possession T1) must NOT drop a free-floating magnet
    // where the bolt happens to detonate in empty space — the magnet
    // rides a CLAIMED mana sphere, and a bolt that misses mana
    // "evaporates without trace" (player-confirmed 2026-07-13). Cast
    // over open terrain with no mana in the flight path → zero (10,54)
    // magnet auras exist after the bolt resolves.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    assert_eq!(count(&w, 10, 54), 0, "no magnet auras at level start");

    w.mc2_select_spell(1, 1, 0); // Possession tier 1 = Mana Magnet
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    // Let the bolt fly and detonate on terrain.
    for _ in 0..40 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(
        count(&w, 10, 54),
        0,
        "an empty-space possession bolt spawns NO magnet aura"
    );
}

#[test]
fn mc2_invisibility_break_law_per_tier() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    // --- Tier 0: any offensive cast BREAKS the cloak. -----------------
    // Bind invis (11) tier 0 to the left hand, fireball (0) to the right
    // (dev_spells self-grants both on select).
    w.mc2_select_spell(11, 0, 0);
    w.mc2_select_spell(0, 0, 1);
    // Cast invis: arms + runs the first effect tick (sets the flag and
    // the break strength). The window is now live.
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert!(
        w.mc2_book_view().armed[11],
        "T0 invisibility window is live right after casting"
    );
    // Cast fireball while cloaked at T0 → the arm-path break law fires
    // and zeroes the invis window (armed → false), lifting the regen
    // block with the cloak.
    w.tick(
        pose,
        PlayerCommand {
            fire_right: true,
            ..Default::default()
        },
    );
    assert!(
        !w.mc2_book_view().armed[11],
        "T0: casting fireball breaks invisibility (window cleared)"
    );

    // --- Tier 2: NOTHING breaks the cloak. ----------------------------
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    w.mc2_select_spell(11, 2, 0); // invis tier 2 (strength 3)
    w.mc2_select_spell(0, 0, 1);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert!(
        w.mc2_book_view().armed[11],
        "T2 invisibility window is live"
    );
    w.tick(
        pose,
        PlayerCommand {
            fire_right: true,
            ..Default::default()
        },
    );
    assert!(
        w.mc2_book_view().armed[11],
        "T2: casting fireball does NOT break invisibility (window survives)"
    );
}

#[test]
fn mc2_castle_cost_gate_tracks_live_level() {
    // The castle cast GATE must charge the OWN castle level's live
    // tier-scaled cost — not the stale SetSpell-time `max_life`. The
    // castle level rises via build with no re-select, so before the fix
    // the gate kept the level-0 base (1000) and you could recast below
    // the shown cost (player 2026-07-13). Retail re-syncs via the +1
    // castle XP on each upgrade; we re-sync at the gate.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);

    // A 19×19 clear footprint (the castle needs room to stamp).
    let p = w.planes();
    let mut spot = None;
    'outer: for cy in (24..222u16).step_by(3) {
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
            spot = Some((cx, cy));
            break 'outer;
        }
    }
    let (cx, cy) = spot.expect("a clear 19x19 spot");
    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 16.5;
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    // Build a level-1 castle.
    w.mc2_select_spell(2, 0, 0);
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
    let (_, _, lvl1) = w.loadout().castle.expect("castle raised");
    assert_eq!(lvl1, 1, "castle at level 1");

    // The live upgrade cost at level 1 = LADDER[1] = 10000 (tier 0, no
    // multiply). The pane already shows this; the gate must match it.
    assert_eq!(
        w.mc2_book_view().cost[2],
        10_000,
        "the live castle cost is the level-1 ladder rung"
    );

    // Now play for real (dev off). With mana ABOVE the stale base (1000)
    // but BELOW the live cost (10000), the recast must be REFUSED — no
    // new castle ball launches, the castle stays level 1.
    w.set_dev_spells(false);
    w.set_player_mana(5_000);
    let count_balls = |w: &World| {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 9 && e.model == 10 && e.life >= 0)
            .count()
    };
    w.tick(pose, PlayerCommand::default()); // release (fresh edge)
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(
        count_balls(&w),
        0,
        "5000 mana < the live 10000 cost → the recast is refused"
    );
    let (_, _, lvl_after) = w.loadout().castle.expect("castle survives");
    assert_eq!(
        lvl_after, 1,
        "the refused recast left the castle at level 1"
    );

    // Sanity: with dev spells back on (the gate is bypassed) the same
    // recast DOES launch — proving the refusal above was the mana gate,
    // not a broken binding.
    w.set_dev_spells(true);
    w.tick(pose, PlayerCommand::default());
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(
        count_balls(&w),
        1,
        "dev-spells bypasses the gate → the recast launches"
    );
}

/// The CASTLE spell (2) "active" window is an UPGRADE LOCK that tracks
/// the tower build, NOT a fixed 101-tick timer (the old bug: the port
/// armed `f26 = word_0x18 = 101` and counted it down, so the spell stayed
/// "active" ~2× the build animation, blocking the next upgrade — playtest
/// 2026-07-14). Retail (`sub_69AB0`/`sub_5F890`) never counts the timer
/// down; the castle build/upgrade entity holds it and clears it on
/// completion. Here: casting must raise the lock, and it must drop the
/// moment the build settles — well before 101 ticks.
#[test]
fn mc2_castle_spell_lock_tracks_the_build_not_a_fixed_timer() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);

    // A 19×19 clear footprint (same scan as the cost-gate test).
    let p = w.planes();
    let mut spot = None;
    'outer: for cy in (24..222u16).step_by(3) {
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
            spot = Some((cx, cy));
            break 'outer;
        }
    }
    let (cx, cy) = spot.expect("a clear 19x19 spot");
    let pose = PlayerPose::from_tiles(
        cx as f32 + 0.5,
        w.ground_height_tiles(cx as f32 + 0.5, cy as f32 + 16.5) + 2.0,
        cy as f32 + 16.5,
        0.0,
        0.0,
        0.0,
    );

    // Idle before casting: the lock is clear.
    assert_eq!(w.debug_mc2_spell_active(2), 0, "lock clear before casting");

    // Cast.
    w.mc2_select_spell(2, 0, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    // The lock engages while the ball flies + the castle builds.
    let mut active_ticks = 0usize;
    let mut cleared_at = None;
    for t in 0..101 {
        w.tick(pose, PlayerCommand::default());
        if w.debug_mc2_spell_active(2) > 0 {
            active_ticks += 1;
        } else if active_ticks > 0 && cleared_at.is_none() {
            cleared_at = Some(t);
            break;
        }
    }
    let (_, _, lvl) = w.loadout().castle.expect("castle raised");
    assert_eq!(lvl, 1, "the build finished");
    // The lock was engaged during the build...
    assert!(active_ticks > 0, "the castle spell locked during the build");
    // ...and CLEARED when the build settled, strictly before the old
    // fixed 101-tick window (the whole point of the fix).
    let cleared = cleared_at.expect("the lock cleared after the build");
    assert!(
        cleared < 101,
        "the lock cleared at tick {cleared} — with the build, not a 101-tick timer"
    );
    // NB the castle lock does NOT suppress mana regen: retail's
    // `sub_69AB0` touches the caster's mana only once (the cost debit on
    // the cast tick, `sub_68DE0` while `word_0x2E_46 == word_0x30_48`),
    // never on the held ticks — unlike a generic channelled spell whose
    // `sub_693F0` suppresses every tick. The port matches: the castle
    // spell is handled outside the generic effect loop and never calls
    // `suppress_regen` (the old fixed-101 path did, wrongly).
    // Once cleared, the lock stays clear (no phantom re-arm) while idle.
    for _ in 0..30 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(
        w.debug_mc2_spell_active(2),
        0,
        "the lock stays clear once the castle is standing idle"
    );
}

#[test]
fn mc2_lightning_l0_is_a_one_tick_beam() {
    // Lightning L0 (subtype 9) is a one-tick hitscan beam, not a
    // traveling ball — it must flash to its (10,23) blast and be gone,
    // NOT persist as a slow class-9 bolt (the old "stream of
    // projectiles"). docs/spell-audit/lightning.md §5.A.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    w.mc2_select_spell(7, 0, 0); // Lightning tier 0
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    // The VISIBLE flash: `sub_66750` lays a line of sprite-216 (9,9)
    // billboards from the muzzle to the impact THIS frame (the crackle).
    // Without them the one-tick beam despawned before it could render
    // (player 2026-07-13: "the visual flash is completely absent").
    let flash = count(&w, 9, 9);
    assert!(
        flash > 1,
        "the beam lays a sprite-216 trail flash ({flash} nodes)"
    );

    let mut saw_blast = false;
    let mut later_bolt = 0;
    for _ in 0..6 {
        w.tick(pose, PlayerCommand::default());
        saw_blast |= count(&w, 10, 23) > 0;
        later_bolt = later_bolt.max(count(&w, 9, 9));
    }
    assert!(saw_blast, "L0 detonates a (10,23) blast");
    assert_eq!(
        later_bolt, 0,
        "the flash is a 1-frame crackle — no (9,9) persists as a slow traveler"
    );
}

#[test]
fn mc2_lightning_storm_rains_beams() {
    // Lightning L1/L2 (subtype 12) detonates into the (10,38) STORM
    // cloud (`sub_4FFB0`), which hovers then RAINS (9,9) beams that
    // strike the ground as (10,23) impacts — NOT the old inert
    // stand-in that "did nothing" (player 2026-07-13). It must stay
    // pool-bounded. docs/spell-audit/lightning.md §5.C.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let live = |w: &World| w.debug_pool().1.iter().filter(|e| e.life >= 0).count();

    // Ambient baseline (level-000 has its own churn).
    let Some(mut w0) = build_world(&root) else {
        return;
    };
    w0.set_dev_spells(true);
    let (cx, cy) = open_spot(&w0);
    let pose = pose_at(&w0, cx, cy);
    let mut ambient_peak = live(&w0);
    for _ in 0..120 {
        w0.tick(pose, PlayerCommand::default());
        ambient_peak = ambient_peak.max(live(&w0));
    }

    for tier in 1u8..=2 {
        let mut w = build_world(&root).unwrap();
        w.set_dev_spells(true);
        w.mc2_select_spell(7, tier, 0);
        w.tick(
            pose,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        let (mut saw_cloud, mut saw_rain, mut peak) = (false, false, live(&w));
        for _ in 0..120 {
            w.tick(pose, PlayerCommand::default());
            saw_cloud |= count(&w, 10, 38) > 0;
            saw_rain |= count(&w, 10, 23) > 0; // a rained beam struck ground
            peak = peak.max(live(&w));
        }
        assert!(saw_cloud, "L{tier} spawns the (10,38) storm cloud");
        assert!(
            saw_rain,
            "L{tier} storm rains (9,9) beams → (10,23) strikes"
        );
        assert!(
            peak <= ambient_peak + 250,
            "L{tier} storm stays pool-bounded (ambient {ambient_peak}, peak {peak})"
        );
    }
}

#[test]
fn mc2_speed_window_interrupts_on_brake() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    // Cast Speed (3) tier 0 → arms the fixed-duration window and drives
    // the travel-speed override.
    w.mc2_select_spell(3, 0, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert!(w.mc2_book_view().armed[3], "the Speed window is live");
    assert!(
        w.accel_override().is_some(),
        "the Speed boost overrides travel speed"
    );

    // A FORWARD press does not cancel the boost (only a brake does).
    w.thrust_cancel(1.0);
    w.tick(pose, PlayerCommand::default());
    assert!(
        w.mc2_book_view().armed[3],
        "a forward thrust leaves the Speed window running"
    );

    // Braking INTERRUPTS the window (player 2026-07-14: Speed must be
    // stoppable — it flies far past where you need). The window clears,
    // the boost drops, and because the burst timer `f26` is zeroed the
    // mana-regen suppression lifts with it (armed==false ⇒ f26==0).
    w.thrust_cancel(-1.0);
    w.tick(pose, PlayerCommand::default());
    assert!(
        !w.mc2_book_view().armed[3],
        "braking cancels the MC2 Speed window"
    );
    assert!(
        w.accel_override().is_none(),
        "braking stops the MC2 Speed boost"
    );
}

#[test]
fn mc2_earthquake_carves_without_flooding_the_pool() {
    // Earthquake (17) lays a travelling trail of (10,11) SCORCH RINGS
    // (the earth-carve, like a moving Crater) — NOT (10,19) ground-fire
    // sprays. The spray is a fire effect that spews (10,14) smoke every
    // odd tick, so a trail dropping one per tick over its 128-life
    // FLOODED the entity pool (player-reported exhaustion 2026-07-13)
    // and rendered as explosions. Regression: the spell's entity
    // footprint stays near the ambient baseline, it lays scorch rings,
    // and it spawns NO fire spray.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let live = |w: &World| w.debug_pool().1.iter().filter(|e| e.life >= 0).count();

    // Ambient baseline: same pose, no cast (level-000 has its own smoke
    // emitters, so absolute counts include ambient churn).
    let Some(mut w0) = build_world(&root) else {
        return;
    };
    w0.set_dev_spells(true);
    let (cx, cy) = open_spot(&w0);
    let pose = pose_at(&w0, cx, cy);
    let mut ambient_peak = live(&w0);
    for _ in 0..141 {
        w0.tick(pose, PlayerCommand::default());
        ambient_peak = ambient_peak.max(live(&w0));
    }

    // Cast Earthquake tier 2 (the longest trail) and track the peak.
    let mut w = build_world(&root).unwrap();
    w.set_dev_spells(true);
    w.mc2_select_spell(17, 2, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    let mut peak = live(&w);
    let mut saw_scorch = false;
    let mut saw_spray = false;
    for _ in 0..140 {
        w.tick(pose, PlayerCommand::default());
        peak = peak.max(live(&w));
        saw_scorch |= count(&w, 10, 11) > 0;
        saw_spray |= count(&w, 10, 19) > 0;
    }
    assert!(saw_scorch, "the trail lays (10,11) scorch-ring carves");
    assert!(!saw_spray, "the trail must NOT spawn (10,19) fire sprays");
    assert!(
        peak <= ambient_peak + 60,
        "Earthquake stays near ambient ({ambient_peak}); no entity flood (peak {peak})"
    );
}

#[test]
fn mc2_fools_mana_throws_six_decoys_that_trap_the_possessor() {
    // Fool's Mana (22) is a SHOTGUN of six neutral fake-mana decoys,
    // not one real collectible sphere (the old port cast the inverse).
    // A non-owner possession claim springs the tier retaliation: tier 0
    // fires ONE fireball at the possessor and the decoy vanishes
    // (docs/spell-audit/fools-mana.md).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    let base = count(&w, 10, 39); // all mana spheres are model 39 in the port
    w.mc2_select_spell(22, 0, 0); // Fool's Mana tier 0, left hand
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(
        count(&w, 10, 39),
        base + 6,
        "the cast throws six fake-mana decoys, not one real sphere"
    );

    // A rival (a non-owner id) possession-claims one decoy → it springs.
    let slot = w.debug_mc2_claim_fool_sphere(12345);
    assert!(slot != 0, "a decoy is present to be claimed");
    let fb0 = count(&w, 9, 0); // class-9 subtype-0 = fireball
    w.tick(pose, PlayerCommand::default());
    assert_eq!(
        count(&w, 9, 0),
        fb0 + 1,
        "the claimed decoy fires exactly one fireball at the possessor"
    );
    assert_eq!(
        count(&w, 10, 39),
        base + 5,
        "the sprung (tier-0) decoy despawns after its single fireball"
    );
}

#[test]
fn mc2_fools_mana_tier2_retaliates_with_lightning() {
    // Tier 2/3 Fool's Mana answers a possession claim with a LIGHTNING
    // bolt (class-9 subtype 9), not a fireball (docs/spell-audit/
    // fools-mana.md §2b, `sub_36850`).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    w.mc2_select_spell(22, 2, 0); // Fool's Mana tier 2
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    let slot = w.debug_mc2_claim_fool_sphere(12345);
    assert!(slot != 0, "a decoy is present to be claimed");
    let (fb0, lb0) = (count(&w, 9, 0), count(&w, 9, 9));
    w.tick(pose, PlayerCommand::default());
    // The thunder bolt (subtype 9) fires the L0 beam, which flashes a
    // trail of (9,9) billboards — a large jump uniquely marks lightning.
    assert!(
        count(&w, 9, 9) > lb0,
        "the tier-2 decoy answers with a lightning bolt (flash), not silence"
    );
    assert_eq!(count(&w, 9, 0), fb0, "tier 2 does NOT fire a fireball");
}

#[test]
fn mc2_magic_mine_places_a_persistent_mine_not_a_fireball() {
    // Magic Mine (23) lands a persistent (10,78) proximity mine ahead of
    // the caster — not a fireball that bursts on first contact (the old
    // port's bug). With no enemy in range it arms and just sits there
    // (docs/spell-audit/magic-mine.md).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    assert_eq!(count(&w, 10, 78), 0, "no mines at level start");

    w.mc2_select_spell(23, 0, 0); // Magic Mine tier 0
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    // Let the carrier fly forward and land (~15-tile maxLife fuse).
    for _ in 0..30 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(
        count(&w, 10, 78),
        1,
        "the carrier placed exactly one persistent mine"
    );
    // It persists through the arm delay with no target in range.
    for _ in 0..120 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(
        count(&w, 10, 78),
        1,
        "the mine persists with no enemy nearby (no contact-detonate)"
    );
}

#[test]
fn mc2_magic_mine_detonates_when_a_target_approaches() {
    // The proximity trigger: a mine detonates (despawns + bursts) when a
    // wizard comes within 14 tiles after the arm delay. Placed as a
    // RIVAL-owned mine right where the human sits → it triggers on the
    // out-of-pool human (docs/spell-audit/magic-mine.md §2).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    let slot = w.debug_mc2_place_mine(cx, cy, 0, 7); // owner = rival id 7
    assert!(slot != 0, "the mine was placed");
    assert_eq!(count(&w, 10, 78), 1);

    let mut detonated = false;
    for _ in 0..90 {
        w.tick(pose, PlayerCommand::default());
        if count(&w, 10, 78) == 0 {
            detonated = true;
            break;
        }
    }
    assert!(
        detonated,
        "the mine detonates while the human sits inside its 14-tile trigger"
    );
}

#[test]
fn mc2_fools_mana_decoys_do_not_count_toward_world_mana() {
    // The fake decoys carry a random mana value for the disguise, but you
    // can never trip your OWN trap to reclaim them — so they must NOT
    // inflate the world-mana denominator, or their uncollectable share
    // would dilute the castle-share goal below reachability (player
    // 2026-07-13; docs/spell-audit/fools-mana.md).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    w.tick(pose, PlayerCommand::default()); // settle the mana census
    let before = w.loadout().world_mana;

    w.mc2_select_spell(22, 0, 0); // Fool's Mana tier 0
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    w.tick(pose, PlayerCommand::default()); // recompute the census
    assert_eq!(count(&w, 10, 39) >= 6, true, "six decoys exist");
    let after = w.loadout().world_mana;
    // Six decoys carry up to 6×1999 ≈ 12000 fake mana; excluded, the
    // denominator barely moves (a decoy would add thousands each).
    assert!(
        after <= before + 1999,
        "decoys must not inflate world-mana (before {before}, after {after})"
    );
}

#[test]
fn mc2_metamorph_transforms_and_reverts() {
    // Metamorph (4): the caster becomes a pooled class-5 creature (model
    // 19 on non-Day) slaved to the player pose, carpet hidden; the
    // transform reverts (creature despawns, carpet returns) at the cast
    // window expiry (docs/spell-audit/summon-creatures.md Part A).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    let base = count(&w, 5, 19);
    assert_eq!(w.mc2_metamorph_model(), 0, "not transformed at start");

    w.mc2_select_spell(4, 0, 0); // Metamorph tier 0 → model 19 (non-Day)
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(w.mc2_metamorph_model(), 19, "transformed into model 19");
    assert_eq!(
        count(&w, 5, 19),
        base + 1,
        "one metamorph creature spawned (the pose-puppet)"
    );

    // Ride out the cast window (tier-0 duration 201 ticks) → revert.
    for _ in 0..260 {
        w.tick(pose, PlayerCommand::default());
    }
    assert_eq!(w.mc2_metamorph_model(), 0, "reverted after the window");
    assert_eq!(count(&w, 5, 19), base, "the pose-puppet despawned");
}

#[test]
fn mc2_summon_army_spawns_an_allied_ring() {
    // Summon Army (19): the carrier lands and spawns a ring of allied
    // class-5 creatures (8 fireflies at tier 0 on non-Day), owned by the
    // caster (docs/spell-audit/summon-creatures.md Part B).
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);
    let base = count(&w, 5, 19);

    w.mc2_select_spell(19, 0, 0); // Summon Army tier 0 → firefly (19) ×8
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    // Let the carrier fly and land, then the ring appears.
    let mut peak = base;
    for _ in 0..40 {
        w.tick(pose, PlayerCommand::default());
        peak = peak.max(count(&w, 5, 19));
    }
    assert!(
        peak >= base + 2,
        "the carrier spawned an allied creature ring (peak {peak}, base {base})"
    );
}

#[test]
fn mc2_earthquake_travel_scales_with_tier() {
    // The earthquake trail's travel distance scales with the spell level
    // (~2× per tier): life_0x1A {16,32,64} → trail life 8× = {128,256,512}
    // ticks (player-confirmed 2026-07-14; docs/spell-audit/00-PLAN.md).
    // The (10,15) trail persists for its life as it travels, so its total
    // presence is a proxy for reach.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    // Read the trail's remaining LIFE the tick it first appears — the
    // travel is life × step, and reading life avoids the terrain
    // water-gate (`f26 > 8`) that can cut travel short in a wet spot.
    let trail_life = |tier: u8| -> i32 {
        let mut w = build_world(&root).unwrap();
        w.set_dev_spells(true);
        let (cx, cy) = open_spot(&w);
        let pose = pose_at(&w, cx, cy);
        w.mc2_select_spell(17, tier, 0); // Earthquake
        w.tick(
            pose,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        for _ in 0..60 {
            w.tick(pose, PlayerCommand::default());
            if let Some(e) = w
                .debug_pool()
                .1
                .iter()
                .find(|e| e.class == 10 && e.model == 15 && e.life >= 0)
            {
                return e.life;
            }
        }
        0
    };
    let (l0, l2) = (trail_life(0), trail_life(2));
    // life_0x1A {16,64} → trail life {128,512}: tier 2 lives ~4× longer.
    assert!(
        l0 > 0 && l2 >= l0 * 2,
        "tier-2 earthquake trail lives much longer than tier 0 (l0={l0}, l2={l2})"
    );
}

#[test]
fn mc2_spell_select_raises_notification_toast() {
    // The change-spell path (EF:37925) raises the top-of-screen
    // notification with the chosen TIER's own name, on a 20-tick life
    // (the presentation surface — hash-excluded, so the goldens never
    // see it). Selecting Possession tier 1 must toast "Mana Magnet"
    // (its distinct per-tier hint name), then decay to nothing.
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    assert!(w.notification().is_none(), "no toast at level start");

    w.mc2_select_spell(1, 1, 0); // Possession tier 1
    let want = w.mc2_spell_name(1, 1).to_string();
    assert!(!want.is_empty(), "the tier-1 name resolves from L1.TXT");
    let (text, color) = w.notification().expect("select raises a toast");
    assert_eq!(text, want, "the toast is the chosen tier's spell name");
    assert_eq!(color, [255, 0, 0], "plain toasts are red");

    // The 20-tick select life decays and clears (the toast never
    // perturbs the state hash — the countdown just runs on its own).
    for _ in 0..19 {
        w.tick(pose, PlayerCommand::default());
    }
    assert!(w.notification().is_some(), "toast still live before expiry");
    w.tick(pose, PlayerCommand::default());
    assert!(
        w.notification().is_none(),
        "toast cleared after its 20-tick life"
    );
}
