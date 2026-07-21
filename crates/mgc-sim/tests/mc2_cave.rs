//! CAVE FIXTURE over real level-014 data — the roster-richest cave (32
//! pillars, 61 brutes, 92 bees, 25 switches) — under the full MC2
//! profile. Positively exercised: the load settle (sculptors, pillar
//! MEASURE and the load-time arms) holds the floor↔ceiling invariant
//! over the whole map, the cave-only roster spawns
//! ((14,2)/(5,24)/(2,6)), the cave-EXCLUDED ctors spawn NOTHING, the
//! (10,86) drip spawner fires on its 8-turn cadence, and a NATIVE
//! Cave-In cast (spell 25, the one cave-only spell) flies, impacts and
//! collapses terrain through the (9,30) → (10,89) chain.
//!
//! Golden hashes pin the trajectory (the MC1 goldens in state_hash.rs
//! and the mc2_slice level-000 goldens are untouched — shared chassis,
//! separate fixtures). Self-skips without baked mc2 data.

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc2/level-014.mgcl").exists() && p.join("assets/mc2-cave/build.tab.bin").exists())
        .then_some(p)
}

fn build_world(root: &std::path::Path) -> Option<World> {
    build_world_level(root, "mc2/level-014.mgcl").map(|(w, _)| w)
}

fn build_world_level(
    root: &std::path::Path,
    level: &str,
) -> Option<(World, mgc_formats::LevelPackage)> {
    let file = std::fs::File::open(root.join(level)).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let terrain = pkg.terrain.as_ref()?;
    let ceiling = terrain.ceiling.clone().unwrap_or_default();
    if ceiling.is_empty() {
        // A bake without a ceiling plane — nothing to pin.
        return None;
    }
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling,
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-cave")).unwrap();
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
    // Caves are non-Day: runtime repaints invert relief shading
    // (sub_462A0's non-day arm) — the app sets the same.
    w.set_mc2_night_shade(true);
    if let Some(st) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = st
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        w.set_mc2_stages(&rows);
        let vars: Vec<(i8, i8, u8, u8, u32)> = st
            .variables
            .iter()
            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
            .collect();
        w.set_mc2_stagevars(&vars);
    }
    Some((w, pkg))
}

fn hover(w: &mut World, x: f32, z: f32, ticks: usize, cmd: PlayerCommand) {
    for _ in 0..ticks {
        let alt = w.ground_height_tiles(x, z) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), cmd);
    }
}

/// An OPEN cavern spot: a 3×3 unsealed neighborhood with > 40 height
/// units of headroom (most of a cave map is sealed rock — parking in
/// it squashes the player against the pinned ceiling and detonates
/// any cast on the spot, authentically).
fn open_spot(w: &World) -> (f32, f32) {
    let p = w.planes();
    let c = w.ceiling_plane();
    for y in 8..248usize {
        for x in 8..248usize {
            let ok = (0..3).all(|dy| {
                (0..3).all(|dx| {
                    let t = (y + dy - 1) * 256 + (x + dx - 1);
                    p.angle[t] & 8 == 0 && c[t] as i32 - p.height[t] as i32 > 40
                })
            });
            if ok {
                return (x as f32 + 0.5, y as f32 + 0.5);
            }
        }
    }
    (64.5, 64.5)
}

fn count(w: &World, class: u8, model: u8) -> usize {
    w.debug_pool()
        .1
        .into_iter()
        .filter(|e| e.class == class && e.model == model && e.life >= 0)
        .count()
}

/// THE invariant over the whole map: ceiling > floor ⇔ bit3 clear.
fn invariant_violations(w: &World) -> usize {
    let p = w.planes();
    let c = w.ceiling_plane();
    (0..c.len())
        .filter(|&t| {
            let open = c[t] > p.height[t];
            let sealed_bit = p.angle[t] & 8 != 0;
            open == sealed_bit
        })
        .count()
}

/// The scripted cave run; returns the checkpoint hashes.
fn run(root: &std::path::Path) -> Option<(Vec<u64>, Vec<u64>)> {
    let mut w = build_world(root)?;
    let (sx, sy) = open_spot(&w);
    let idle = PlayerCommand::default();
    let mut hashes = vec![w.state_hash()];
    let mut obs = vec![w.observable_digest()];

    // A: idle in an open cavern — the walker/bee cadences + the drip
    // spawner's 8-turn cadence in front of the parked pose.
    hover(&mut w, sx, sy, 64, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // B: a NATIVE Cave-In (spell 25, LEFT hand, tier 0) fired into
    // the cavern — the (9,30) manifestation detonates on the nearest
    // wall/ceiling and the (10,89) radial collapse runs to
    // completion (wave 227 → 1024 at +22/tick ≈ 37 ticks).
    w.set_dev_spells(true);
    let select = PlayerCommand {
        mc2_select: Some((25, 0, 0)),
        ..Default::default()
    };
    hover(&mut w, sx, sy, 1, select);
    let firing = PlayerCommand {
        fire_left: true,
        ..Default::default()
    };
    hover(&mut w, sx, sy, 2, firing);
    hover(&mut w, sx, sy, 96, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // C: the sweep's disposition storm (matches mc2sweep) — trips
    // the switch column, materializing the dis-gated brutes/bees.
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }
    hover(&mut w, sx, sy, 64, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // StageVar hold-gate: with the row table baked VERBATIM (byte0 is
    // a FLAG byte, not signed — reading it signed drops every flagged
    // row), level-014's full table loads: the kind-9 model-18 (THING
    // 334, gate = template-6 death — never fires in this run) PLUS the
    // flagged rows binding kind-1 walkers, kind-4 guardians and kind-6
    // timer spawns. Pin the census by kind and the kind-9 anchor.
    let held = w.debug_mc2_held();
    assert!(
        held.contains(&(447, 18, 9)),
        "level-014: the kind-9 model-18 hold survived the verbatim row bake"
    );
    let mut kinds = std::collections::BTreeMap::<u8, usize>::new();
    for &(_, _, k) in &held {
        *kinds.entry(k).or_insert(0) += 1;
    }
    assert_eq!(
        kinds,
        [(1u8, 7usize), (4, 26), (6, 26), (9, 1)]
            .into_iter()
            .collect(),
        "level-014 held census by kind (verbatim StageVar rows)"
    );

    Some((hashes, obs))
}

#[test]
fn mc2_cave_behaviors_and_goldens() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let Some((got, obs)) = run(&root) else {
        common::golden_skip("mc2 level-014 has no baked ceiling (pre-EPOCH-8 bake)");
        return;
    };
    assert_eq!(
        (got.clone(), obs.clone()),
        run(&root).unwrap(),
        "cave run is not deterministic"
    );
    println!("mc2 cave hashes: {got:#018x?}");

    // Behavior probes on a fresh world.
    let mut w = build_world(&root).unwrap();
    let idle = PlayerCommand::default();

    // The load settle held the invariant everywhere (probe = 0 on
    // all 47 caves; this pins the fixture level forever).
    assert_eq!(invariant_violations(&w), 0, "post-settle invariant");

    // The cave-only roster: pillars are authored load-time (DisId
    // −1) and measured in the settle; the level's brutes/bees are
    // ALL dis-gated (61 + 92 records) — fire the sweep's
    // disposition storm to materialize them, exactly like retail's
    // switch column would.
    let pillars = count(&w, 14, 2);
    assert!(pillars >= 20, "pillars measured + idle ({pillars}/32)");
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }
    let brutes = count(&w, 5, 24);
    assert!(brutes >= 10, "cave brutes spawned ({brutes}/61)");
    let bees = count(&w, 2, 6);
    assert!(bees >= 10, "cave bees spawned ({bees}/92)");

    // Cave-EXCLUDED: the flying-bee siblings and the m27 kraken
    // never spawn here whatever the level authors.
    assert_eq!(count(&w, 2, 7) + count(&w, 2, 8), 0, "no (2,7)/(2,8)");
    assert_eq!(count(&w, 5, 27), 0, "no (5,27) in caves");

    // The (10,86) drip spawner: every 8th turn, one drip lands on an
    // empty passable tile in the 20×20 window 10 tiles ahead of the
    // player (life 9 — sample every tick). Park mid-map, where the
    // window reaches carved type-0 floor (the open pocket's window
    // is all typed rock — the search finds nothing there,
    // authentically).
    let (sx, sy) = open_spot(&w);
    let mut saw_drip = false;
    for _ in 0..32 {
        hover(&mut w, 64.0, 64.0, 1, idle);
        saw_drip |= count(&w, 10, 86) > 0;
    }
    assert!(saw_drip, "the 8-turn drip cadence fired");

    // The native Cave-In: the (9,30) manifestation flies (often
    // sub-tick — 1.5 tiles/tick to the nearest wall detonates inside
    // the launch tick, authentically), the (10,89) collapse appears
    // and the ceiling under the burst moves (terrain is the weapon).
    let ceiling_before: Vec<u8> = w.ceiling_plane().to_vec();
    w.set_dev_spells(true);
    let select = PlayerCommand {
        mc2_select: Some((25, 0, 0)),
        ..Default::default()
    };
    hover(&mut w, sx, sy, 1, select);
    let firing = PlayerCommand {
        fire_left: true,
        ..Default::default()
    };
    hover(&mut w, sx, sy, 2, firing);
    let mut saw_collapse = count(&w, 10, 89) > 0;
    for _ in 0..96 {
        hover(&mut w, sx, sy, 1, idle);
        saw_collapse |= count(&w, 10, 89) > 0;
    }
    assert!(saw_collapse, "the (10,89) collapse ran");
    assert_ne!(
        ceiling_before,
        w.ceiling_plane(),
        "the collapse moved the ceiling"
    );
    // Every terrain writer re-ran the invariant.
    assert_eq!(invariant_violations(&w), 0, "post-collapse invariant");

    // GOLDEN: pin the checkpoint hashes. Re-pin deliberately when a
    // cave system lands a fidelity fix (document the move in git).
    //
    // Re-pinned for the m9 grounded-arm fix (`sub_20940` EF:12357-89):
    // the damage/death head now runs FIRST (a grounded hive was
    // unkillable), the stand-up counts UP and only the tick that READS
    // -1 stands the hive back up, an AWAKE hive arms the 50-tick
    // stand-up instead of scanning, and an ASLEEP hive parks at 0 and
    // feeds in place rather than cycling back to a 400-tick walk. This
    // level authors m9, so its hives move and eat on a different
    // schedule. The first two checkpoints hold; the last two move.
    // ATTRIBUTED by probe: the magic-mine teardown landed in the same
    // batch and moves NOTHING here (identical hashes before and after
    // it), so this re-pin is m9 alone.
    assert_eq!(
        got,
        vec![
            0xb9ef2aab49926cbcu64,
            0x7a89b38d106e4b85,
            0x8622703d123f88c1,
            0xdcb532dae4b6c65a,
        ],
        "cave goldens moved — re-pin ONLY for an intended fidelity change"
    );

    // The layout-INDEPENDENT companion golden — see state_hash.rs:
    // survives hashed-layout re-pins; moves ONLY with real behavior.
    //
    // The m9 grounded-arm fix moves the LAST checkpoint only, and that
    // is the correct signal: a hive that no wizard has approached now
    // squats and feeds in place instead of cycling back into a 400-tick
    // walk, so late-run hive positions genuinely differ. The first
    // three hold — the divergence needs ~400 asleep ticks to appear.
    const OBSERVABLE: [u64; 4] = [
        0xb0299049353c6c29,
        0x2d60a54a359da557,
        0x1ada7615a38d2848,
        0x367a0a11830499dc,
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}

/// The wall-hug eye band: drive the faithful MC2 mover straight into a
/// sealed cave wall and pin that the eye NEVER leaves the mover's
/// clamp band — >= floor+256 and <= ceiling-384 against the
/// INTERPOLATED surfaces the renderer draws (mesh == collision: same
/// corner heights, same parity diagonals). This exonerates the
/// vertical clamps for the wall-peek x-ray: the residual vector is the
/// near plane cutting a hugged steep face LATERALLY, which the terrain
/// shader's backface-black arm paints as rock instead of x-raying the
/// far chamber.
#[test]
fn mc2_cave_wall_hug_holds_the_clamp_band() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(w) = build_world(&root) else {
        eprintln!("skipping: no ceiling plane");
        return;
    };
    // A wall approach: sealed tile at (x, zw), 6 open roomy tiles
    // straight south of it.
    let (p, c) = (w.planes(), w.ceiling_plane().to_vec());
    let mut approach = None;
    'scan: for zw in 8..240usize {
        for x in 8..240usize {
            let sealed = p.angle[zw * 256 + x] & 8 != 0;
            if !sealed {
                continue;
            }
            let ok = (1..=6).all(|d| {
                let t = (zw + d) * 256 + x;
                p.angle[t] & 8 == 0 && c[t] as i32 - p.height[t] as i32 > 24
            });
            if ok {
                approach = Some((x, zw));
                break 'scan;
            }
        }
    }
    let Some((x, zw)) = approach else {
        eprintln!("no wall approach found");
        return;
    };
    eprintln!("approach: wall at ({x},{zw}), corridor south");

    let mut sim = mgc_sim::Simulation::with_world(w);
    let fx = x as f32 + 0.5;
    let fz = zw as f32 + 5.5;
    let g0 = sim.world.as_ref().unwrap().ground_height_tiles(fx, fz);
    sim.flyer.x = fx;
    sim.flyer.z = fz;
    sim.flyer.y = g0 + 1.5;
    sim.flyer.yaw = 0.0; // -Z: straight at the wall
    sim.flyer.pitch = 0.0;
    sim.sync_carpet_from_flyer();

    let mut worst_floor = f32::MAX;
    let mut worst_ceil = f32::MAX;
    for _ in 0..140 {
        sim.step(&mgc_sim::FlightInput {
            thrust: 1.0,
            ..Default::default()
        });
        let f = sim.flyer;
        let ex = ((f.x.rem_euclid(256.0)) * 256.0) as u16;
        let ez = ((f.z.rem_euclid(256.0)) * 256.0) as u16;
        let eye = f.y * 256.0;
        let w = sim.world.as_ref().unwrap();
        let floor = w.ground_z_engine(ex, ez) as f32;
        let ceil = w.player_cave_ceiling(ex, ez).unwrap() as f32 + 384.0;
        let (df, dc) = (eye - floor, ceil - eye);
        worst_floor = worst_floor.min(df);
        worst_ceil = worst_ceil.min(dc);
    }
    // The retail clamps: floor+256 (EF:59768) / ceiling-384
    // (EF:59758-63); 1.0 slop for the f32 round-trip.
    assert!(
        worst_floor >= 255.0,
        "eye dipped under floor+256 while wall-hugging (worst {worst_floor:.1})"
    );
    assert!(
        worst_ceil >= 383.0,
        "eye rose over ceiling-384 while wall-hugging (worst {worst_ceil:.1})"
    );
}

/// The ENHANCED-mover funnel squeeze: the deviation mover needs a cave
/// narrow-space law — without one, nothing refuses entry into the seam
/// where floor meets ceiling and a floor-wins pinch clamp hoists the
/// head THROUGH the diving ceiling. With the squeeze gate (the
/// faithful gate's sub_11E20 predicate) the eye must stay under the
/// interpolated ceiling for the whole approach.
#[test]
fn mc2_cave_enhanced_funnel_never_breaches_ceiling() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(w) = build_world(&root) else {
        eprintln!("skipping: no ceiling plane");
        return;
    };
    let (p, c) = (w.planes(), w.ceiling_plane().to_vec());
    // A FUNNEL: an OPEN (unsealed) pinch tile — air band a few height
    // bytes, far under the mover's 0.75-tile floor clearance — with a
    // roomy open corridor leading in. This is the mc2:03 shape: no
    // sealed tile ever stops the approach, the band just narrows.
    let mut approach = None;
    'scan: for zw in 8..240usize {
        for x in 8..240usize {
            let t0 = zw * 256 + x;
            let band0 = c[t0] as i32 - p.height[t0] as i32;
            if p.angle[t0] & 8 != 0 || band0 <= 0 || band0 > 5 {
                continue;
            }
            let ok = (1..=6).all(|d| {
                let t = (zw + d) * 256 + x;
                let band = c[t] as i32 - p.height[t] as i32;
                p.angle[t] & 8 == 0 && band > if d >= 3 { 20 } else { 4 }
            });
            if ok {
                approach = Some((x, zw));
                break 'scan;
            }
        }
    }
    let Some((x, zw)) = approach else {
        eprintln!("no funnel approach found on level-014");
        return;
    };
    eprintln!("funnel: pinch at ({x},{zw}), corridor south");

    let mut sim = mgc_sim::Simulation::with_world(w);
    sim.thrust_model = mgc_sim::ThrustModel::Enhanced;
    // Hug the pinch CORNER (the height/ceiling bytes live on tile
    // corners): the tile-center line interpolates away from the
    // narrowest point and misses the squeeze.
    let fx = x as f32 + 0.05;
    let fz = zw as f32 + 5.5;
    let g0 = sim.world.as_ref().unwrap().ground_height_tiles(fx, fz);
    sim.flyer.x = fx;
    sim.flyer.z = fz;
    sim.flyer.y = g0 + 1.5;
    sim.flyer.yaw = 0.0; // -Z: straight into the wall
    sim.flyer.pitch = 0.0;
    sim.sync_carpet_from_flyer();

    let mut worst: f32 = f32::MAX; // min (ceiling - eye), engine units
    for _ in 0..250 {
        sim.step(&mgc_sim::FlightInput {
            thrust: 1.0,
            ..Default::default()
        });
        let f = sim.flyer;
        let ex = ((f.x.rem_euclid(256.0)) * 256.0) as u16;
        let ez = ((f.z.rem_euclid(256.0)) * 256.0) as u16;
        let w = sim.world.as_ref().unwrap();
        // player_cave_ceiling = interpolated ceiling − 384.
        let ceil = w.player_cave_ceiling(ex, ez).unwrap() as f32 + 384.0;
        worst = worst.min(ceil - f.y * 256.0);
    }
    assert!(
        worst > 0.0,
        "the enhanced carpet's head breached the cave ceiling \
         (worst ceiling-eye = {worst:.1} engine units)"
    );
}

/// The mc2:23 spawn-embedded-in-rock case: level-023's (3,4) wizard
/// start at (134,47) lies in baked-sealed
/// rock — the entry cavern is carved at LOAD by an authored (10,82)
/// room at (127,47) with par extents (58,42) and depth par3 = 9
/// (PrepareEvents case 0x52, EV:373-379). Pin that the load settle
/// leaves the start tile (and a 3×3 ring around it) open cave with
/// real headroom, so the port never regresses to the ctor-default
/// 6×6 carve that left the player inside the wall.
#[test]
fn mc2_level_023_start_chamber_is_carved_open() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked mc2 data");
        return;
    };
    if !root.join("mc2/level-023.mgcl").exists() {
        eprintln!("skipping: no baked level-023");
        return;
    }
    let (w, pkg) = build_world_level(&root, "mc2/level-023.mgcl").unwrap();
    let start = pkg
        .things
        .things
        .iter()
        .find(|t| t.kind == mgc_formats::ThingKind::Entity && t.class == 3 && t.model == 4)
        .expect("level-023 authors the (3,4) start marker");
    let (sx, sy) = (start.x as usize, start.y as usize);
    let p = w.planes();
    let c = w.ceiling_plane();
    for dy in 0..3 {
        for dx in 0..3 {
            let t = (sy + dy - 1) * 256 + (sx + dx - 1);
            assert!(
                p.angle[t] & 8 == 0,
                "start ring tile ({},{}) still sealed",
                sx + dx - 1,
                sy + dy - 1
            );
            assert!(
                c[t] as i32 - p.height[t] as i32 > 20,
                "start ring tile ({},{}) has no headroom (floor {} ceiling {})",
                sx + dx - 1,
                sy + dy - 1,
                p.height[t],
                c[t]
            );
        }
    }
}
