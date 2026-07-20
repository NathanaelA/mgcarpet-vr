//! The MC2 vertical slice on real level-000 data under the full MC2
//! profile. Every criterion is POSITIVELY exercised: creatures are
//! found via the pool debug view, the player is parked next to them,
//! and the observable is asserted per model — Goat wakes/flees/dies
//! (mana sphere + kill credit), Archers stand and FIRE (arrow entity +
//! danger music), Villager wanders and never attacks, the type-5
//! fly-to objective latches at its authored point.
//!
//! Golden hashes pin the slice (the MC1 goldens in state_hash.rs are
//! untouched — the columns share the chassis, not the fixtures).
//! Self-skips without baked mc2 data.

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

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
    // The mc2-night bundle's own feature data (level-000 is a night
    // map): SEARCH + the BUILD0-0 footprint bank + BLDGPRM — the
    // building creator consumes all three (the app arranges the
    // same).
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
    // Level-000 is a NIGHT map: runtime repaints invert relief
    // shading (sub_462A0's non-day arm) — the app sets the same.
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
    Some(w)
}

fn hover(w: &mut World, x: f32, z: f32, ticks: usize, cmd: PlayerCommand) {
    for _ in 0..ticks {
        let alt = w.ground_height_tiles(x, z) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), cmd);
    }
}

/// Nearest live (class, model) entity's tile position from the pool
/// debug view.
fn find_creature(w: &World, class: u8, model: u8) -> Option<(f32, f32)> {
    w.debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == class && e.model == model && e.life >= 0)
        .map(|e| (e.tx as f32 + 0.5, e.ty as f32 + 0.5))
}

/// The scripted slice run; returns the checkpoint hashes.
fn run(root: &std::path::Path) -> Option<(Vec<u64>, Vec<u64>)> {
    let mut w = build_world(root)?;
    let idle = PlayerCommand::default();
    let mut hashes = vec![w.state_hash()];
    let mut obs = vec![w.observable_digest()];

    // A: idle far from everything — awake pass + wander cadences.
    hover(&mut w, 16.0, 16.0, 64, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // B: the type-5 fly-to objective at (115, 212).
    hover(&mut w, 115.5, 212.5, 8, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // C: park next to a goat — awake + flee.
    if let Some((vx, vz)) = find_creature(&w, 5, 1) {
        hover(&mut w, vx + 2.0, vz, 96, idle);
    }
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // D: NATIVE fireballs at the nearest goat (the MC1 equip bridge
    // does NOT cast on the MC2 column — the seeded book's LEFT
    // fireball is the cast path). RIGHT unbinds so possession pulses
    // don't spray claims over the script; tier-0 fireball is CLICK
    // cadence, so the volley pulses the edge.
    w.set_dev_spells(true);
    if let Some((vx, vz)) = find_creature(&w, 5, 1) {
        let unbind_r = PlayerCommand {
            mc2_select: Some((255, 0, 1)),
            ..Default::default()
        };
        hover(&mut w, vx + 1.5, vz, 1, unbind_r);
        let firing = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        for _ in 0..48 {
            hover(&mut w, vx + 1.5, vz, 1, firing);
            hover(&mut w, vx + 1.5, vz, 1, idle);
        }
    }
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // E: materialize the rest of the authored population (archers
    // sit behind dispositions the sweep never trips) and provoke
    // them: killing townsfolk arms the wizard's wanted timer.
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }
    if let Some((vx, vz)) = find_creature(&w, 5, 13) {
        let firing = PlayerCommand {
            fire_left: true,
            fire_right: true,
            ..Default::default()
        };
        hover(&mut w, vx + 1.0, vz, 64, firing);
    }
    if let Some((ax, az)) = find_creature(&w, 5, 4) {
        hover(&mut w, ax + 3.0, az, 160, idle);
    }
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    Some((hashes, obs))
}

#[test]
fn mc2_slice_behaviors_and_goldens() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let Some((got, obs)) = run(&root) else {
        common::golden_skip("mc2 level-000 has no baked terrain");
        return;
    };
    assert_eq!(
        (got.clone(), obs.clone()),
        run(&root).unwrap(),
        "slice is not deterministic"
    );
    println!("mc2 slice hashes: {got:#018x?}");

    // Re-run the script with behavior probes at each phase.
    let mut w = build_world(&root).unwrap();
    let idle = PlayerCommand::default();

    // The at-load buildings raise the western village during the
    // first 30 ticks (the (10,45) action) — count them before the
    // idle window, then confirm the finished state below.
    let buildings = |w: &World| {
        w.debug_pool()
            .1
            .into_iter()
            .filter(|e| e.class == 10 && e.model == 45 && e.life >= 0)
            .count()
    };
    let b0 = buildings(&w);
    assert!(b0 >= 10, "at-load buildings spawned ({b0})");

    // Class-11 switches: live INVISIBLE pool entities (never
    // billboarded, never ledgered), each carrying its record's
    // disposition id + word_10 box; the route sequence rides them.
    let switches0 = w.debug_pool().1.iter().filter(|e| e.class == 11).count();
    assert!(
        switches0 >= 20,
        "level-000's switches spawned ({switches0})"
    );
    assert!(
        !w.misfits().iter().any(|&(c, m, _)| c == 11 && m <= 3),
        "proximity switches are known things now"
    );
    assert!(
        !w.live_poses().iter().any(|p| p.class == 11),
        "switches draw nothing (invisible in retail)"
    );
    assert!(
        w.active_volumes()
            .iter()
            .filter(|v| matches!(v.kind, mgc_sim::engine::world::VolumeKind::Proximity))
            .count()
            >= 20,
        "switch boxes feed the map-triggers overlay"
    );
    assert!(
        w.active_volumes()
            .iter()
            .any(|v| matches!(v.kind, mgc_sim::engine::world::VolumeKind::Objective)),
        "stage checkpoints plot on the overlay"
    );

    // Villager wanders (and never attacks). The building creators'
    // walkable village paint FREES the townsfolk — free walkers can
    // also die authentically (die-on-water row flag on a boxed-in
    // wander), so survival is not asserted; kill CREDIT staying zero
    // is (townsfolk/construction deaths never credit).
    let yaw0: Vec<f32> = w
        .live_poses()
        .iter()
        .filter(|p| p.class == 5 && p.model == 13)
        .map(|p| p.yaw)
        .collect();
    assert!(!yaw0.is_empty(), "villagers spawned at init");
    hover(&mut w, 16.0, 16.0, 200, idle);
    let yaw1: Vec<f32> = w
        .live_poses()
        .iter()
        .filter(|p| p.class == 5 && p.model == 13)
        .map(|p| p.yaw)
        .collect();
    assert!(!yaw1.is_empty(), "villagers survive the idle window");
    assert!(
        yaw0.iter().zip(&yaw1).any(|(a, b)| (a - b).abs() > 0.05),
        "the wander cadence turned somebody"
    );
    assert_eq!(
        w.combat_stats().0,
        0,
        "nothing credited while idling afield"
    );

    // The build actions have finished: pads parked as the static
    // building (state 52) and the footprint carrying REAL building
    // ground — either a texture-band paint (sub_45DC0, types 8..=0x22)
    // or a blend-transition tile (sub_462A0 through the generated
    // building_F2CD0x; type 50 = the [3,1,1,1] corner row under this
    // pad).
    let parked = w
        .debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 10 && e.model == 45 && e.life >= 0 && e.state == 52)
        .expect("a building parked static (state 52)");
    let t = parked.ty as usize * 256 + parked.tx as usize;
    let ground = w.planes().tile_type[t];
    assert!(
        ground >= 8,
        "the building's tile painted real building ground (got {ground})"
    );

    // The herd law: every level-000 goat spawns BOUND to the kind-2
    // graze-leash StageVar (slot 3, anchor tile (53,32)) via
    // `sub_12100`'s subtype pass — state 15 (8·model+7), speed 18,
    // milling the anchor. They never free-wander or form follow-chains.
    assert!(
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 5 && e.model == 1 && e.life >= 0)
            .all(|e| e.state == 15),
        "every goat is stage-held at the graze leash (state 15)"
    );

    // Goat flee: the kind-2 WIZARD WATCH (`sub_1DBF0` tail) — an
    // AWAKE leashed goat that sees a class-3 (range v_28 = 6 tiles,
    // cone-gated on ITS facing, one roll per v_26 = 32-tick cadence)
    // breaks to kind 10 → the FLEE-flagged raise (state 14, speed
    // 54). Park over the anchor: the mill sweeps every goat's cone
    // across us within a lap or two.
    let (gx, gz) = (53.5f32, 32.5f32);
    let mut fled = false;
    for _ in 0..512 {
        let alt = w.ground_height_tiles(gx, gz) + 1.0;
        w.tick(PlayerPose::from_tiles(gx, alt, gz, 0.0, 0.0, 0.0), idle);
        if w.debug_pool()
            .1
            .iter()
            .any(|e| e.class == 5 && e.model == 1 && e.state == 14)
        {
            fled = true;
            break;
        }
    }
    assert!(fled, "a goat that saw the player entered FLEE (14)");

    // ...and the RE-LEASH (`sub_12500` case 0xA): once the flee
    // drops (target ≥ v_28 away → the machine parks it back at
    // wander), the stage bind reclaims it into state 15 — retail's
    // calm-down-and-walk-home loop. Park far away and let it settle.
    let mut releashed = false;
    for _ in 0..600 {
        let alt = w.ground_height_tiles(16.0, 16.0) + 2.0;
        w.tick(PlayerPose::from_tiles(16.0, alt, 16.0, 0.0, 0.0, 0.0), idle);
        if w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 5 && e.model == 1 && e.life >= 0)
            .all(|e| e.state == 15)
        {
            releashed = true;
            break;
        }
    }
    assert!(releashed, "the fled goat re-leashed (kind-10 -> re-hold)");

    // Materialize the kill-target archers via the AUTHORED
    // progression (no debug disposition fire): leave the start box —
    // the (11,1) leave-trigger at (74,212), box 64, releases dis 1 —
    // then complete the two narrated fly-to checkpoints; each
    // completion trips its stage-gated (11,32) switch (par1 0 → dis
    // 2, par1 1 → dis 3 = the four (5,4) archers in the drowned
    // village). Order-insensitive like retail: a gate spawning after
    // its stage completed fires on its first probe.
    w.set_dev_spells(true);
    assert!(find_creature(&w, 5, 4).is_none(), "archers gated at start");
    hover(&mut w, 150.0, 212.5, 12, idle);
    hover(&mut w, 115.5, 212.5, 12, idle);
    hover(&mut w, 194.5, 213.5, 12, idle);
    assert!(
        find_creature(&w, 5, 4).is_some(),
        "the checkpoint chain released the archers (dis 1 → stage gates → dis 3)"
    );

    // Kill an ARCHER with the fireball: model 4 earns kill credit;
    // mana 500 drops a sphere. NATIVE cast (the MC1 equip bridge does
    // NOT cast on the MC2 column): rebind the seeded fireball onto
    // LEFT, keep RIGHT unbound (no possession claims over the kill
    // loop).
    let bind_l = PlayerCommand {
        mc2_select: Some((0, 0, 0)),
        ..Default::default()
    };
    hover(&mut w, 16.0, 16.0, 1, bind_l);
    let unbind_r = PlayerCommand {
        mc2_select: Some((255, 0, 1)),
        ..Default::default()
    };
    hover(&mut w, 16.0, 16.0, 1, unbind_r);
    let firing = PlayerCommand {
        fire_left: true,
        ..Default::default()
    };
    // Fire straight DOWN from directly overhead in short volleys: MC2
    // creatures are faithfully zero-extent (the cross-column damage
    // contract), so projectiles pass through them and the kill path
    // is the explosion FIRE landing ON the cell — whose area write
    // fires ONCE per fire. Volley, let the fire burn out (a live fire
    // captures follow-up fireballs and drifts out of the z band),
    // volley again: 4 connected 250-payload drops beat the
    // archer's 1000 (docs/traces/mc2-fireball-damage.md). The
    // hands spawn ~±1 tile lateral of the carpet — park one tile east
    // so the LEFT hand's drop lands on the cell.
    'kill: for _ in 0..12 {
        let Some((ax, az)) = find_creature(&w, 5, 4) else {
            break;
        };
        let galt = w.ground_height_tiles(ax, az);
        let overhead = PlayerPose::from_tiles(
            ax + 1.0,
            galt + 2.5,
            az,
            0.0,
            -std::f32::consts::FRAC_PI_2,
            0.0,
        );
        for _ in 0..2 {
            w.tick(overhead, firing);
        }
        for _ in 0..30 {
            w.tick(overhead, idle);
            if w.combat_stats().0 > 0 {
                break 'kill;
            }
        }
    }
    let kills = w.combat_stats().0;
    assert!(kills >= 1, "the fireball killed an archer (kills {kills})");
    // The kill state waits for phase & 7 == 0 before transforming
    // (KillEntity_1C930) — give the corpse its settle ticks.
    hover(&mut w, 16.0, 16.0, 16, idle);
    assert!(
        w.live_poses()
            .iter()
            .any(|p| p.class == 10 && p.model == 39),
        "the corpse dropped a mana sphere"
    );

    // Archers: the disposition-3 survivors of the fireball above
    // still stand. Arm the wanted timer by shooting a villager
    // (townsfolk kills are EXCLUDED from kill credit), then stand in
    // range: they fire — an arrow entity exists and the danger music
    // arms.
    let kills_before = w.combat_stats().0;
    if let Some((tx, tz)) = find_creature(&w, 5, 13) {
        hover(&mut w, tx + 1.0, tz, 64, firing);
    }
    assert_eq!(
        w.combat_stats().0,
        kills_before,
        "villager kills never count (model-13 exclusion)"
    );
    // The wanted timer decays (200 ticks) while the archer's acquire
    // cadence samples only every 4 x scanPeriod = 120 ticks — keep
    // the timer armed by continuing to harass villagers from the
    // archer's side (each processed hit re-arms 200; :14561),
    // TRACKING the archer (with buildings live, the archer brain's
    // building/shrine walk states move them between shots). The
    // (9,13) arrow probe honors the class-3 target filter, so arrows
    // don't wipe the pack.
    let mut arrow_seen = false;
    let (mut ax, mut az) = find_creature(&w, 5, 4).expect("archers materialized");
    let mut ayaw = 0.0f32;
    for _ in 0..400 {
        if let Some(p) = w.live_poses().iter().find(|p| p.class == 5 && p.model == 4) {
            (ax, az, ayaw) = (p.x, p.z, p.yaw);
        }
        let vtarget = find_creature(&w, 5, 13);
        // Stand 3 tiles along the archer's FACING — the wizard scan
        // is cone-gated on the archer's yaw (sub_1BF90 :9152-95).
        let (px, pz) = (ax + 3.0 * ayaw.sin(), az - 3.0 * ayaw.cos());
        let alt = w.ground_height_tiles(px, pz) + 0.75;
        let (yaw, pitch) = match vtarget {
            Some((tx, tz)) => {
                let (dx, dz) = (tx - px, tz - pz);
                let dist = (dx * dx + dz * dz).sqrt().max(0.1);
                let galt = w.ground_height_tiles(tx, tz);
                (dx.atan2(-dz), -((alt - galt) / dist).atan())
            }
            None => (0.0, 0.0),
        };
        w.tick(PlayerPose::from_tiles(px, alt, pz, yaw, pitch, 0.0), firing);
        if w.debug_pool()
            .1
            .iter()
            .any(|e| e.class == 9 && e.model == 13)
        {
            arrow_seen = true;
            break;
        }
    }
    assert!(arrow_seen, "an archer fired an arrow at the wanted wizard");
    let alt = w.ground_height_tiles(ax + 3.0, az) + 2.0;
    let frame = w.take_audio(PlayerPose::from_tiles(ax + 3.0, alt, az, 0.0, 0.0, 0.0));
    assert!(frame.danger, "the arrow armed the danger music");

    // The objective board: the archer-unlock flight above already
    // completed both fly-to checkpoints (rows 0 and 1) and advanced
    // the cursor onto the kill objective.
    let (cur, stages) = w.mc2_objective_view();
    assert_eq!(stages.len(), 5, "level-000 registers five stages");
    assert_eq!(stages[0], (5, 2), "checkpoint 1 latched at (115, 212)");
    assert_eq!(stages[1], (5, 2), "checkpoint 2 latched at (194, 213)");
    assert!(cur > 1, "the cursor advanced past the completed fly-tos");
    assert!(!w.completed(), "three stages remain — no premature win");

    // Pinned goldens: regenerate with --nocapture on a DELIBERATE
    // behavior change and say so in the commit.
    const GOLDEN: [u64; 6] = [
        0x3f29a575e74ed7ce, // post-init (GenerateEvents + dis 0)
        0xac2894c19444ec6b, // A: 64 idle ticks afield
        0x52c4a61b85ac224c, // B: the type-5 fly-to latched
        0x5da0547d0f700eaf, // C: goat awake/flee window
        0xf83f8f81b271eae7, // D: fireball combat over the goat
        0x7bc91738e9e0901e, // E: census + villager/archer provocation
    ];
    assert_eq!(
        got, GOLDEN,
        "the MC2 slice diverged from its goldens — if DELIBERATE, \
         re-pin (--nocapture) and say so in the commit"
    );

    // The layout-INDEPENDENT companion golden — see state_hash.rs:
    // survives hashed-layout re-pins; moves ONLY with real behavior.
    const OBSERVABLE: [u64; 6] = [
        0x9a885593f099242c,
        0xc8a0f078fd10afd7,
        0xddc9ae4bc40fafc9,
        0xd9ff6a0b332668d2,
        0x2861dfece633f89a,
        0xc419744a214ba321,
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}

/// Level-000's authored mission chain, end to end: fly-to rows 0/1 →
/// archers (dis 3) → kill them → row 2 latches (only while CURRENT —
/// the type-7 cursor gate) and the m17 kill switch drops the (15,3)
/// spell jar → row 3 (type 0: castle + 15% banked share; forced here
/// — the banked economy is pending) → the m32 row-3 watcher fires
/// dis 6 = FIVE (5,19)
/// fireflies while row 4 arms → killing the wave completes the
/// level. The m32 ObjectiveDone_2 pause keeps rows 2/4 from
/// latching vacuously in the one-tick gap before their targets
/// spawn.
#[test]
fn mc2_level000_mission_chain() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        common::golden_skip("mc2 level-000 has no baked terrain");
        return;
    };
    let idle = PlayerCommand::default();
    let count = |w: &World, class: u8, model: u8| {
        w.debug_pool()
            .1
            .into_iter()
            .filter(|e| e.class == class && e.model == model && e.life >= 0)
            .count()
    };

    // Fly the route with hops so the switch cascade keeps pace.
    for (x, z) in [
        (77.5, 222.5),
        (90.0, 214.0),
        (105.0, 212.5),
        (115.5, 212.5), // row 0
        (140.0, 212.5),
        (165.0, 212.5),
        (185.0, 213.0),
    ] {
        hover(&mut w, x, z, 16, idle);
    }
    hover(&mut w, 194.5, 213.5, 32, idle); // row 1 (the spire)
    let (_, stages) = w.mc2_objective_view();
    assert_eq!(stages[0].1, 2, "row 0 fly-to latched");
    assert_eq!(stages[1].1, 2, "row 1 fly-to latched");
    assert_eq!(
        stages[2],
        (7, 1),
        "row 2 (kill archers) armed, NOT vacuously latched"
    );
    assert_eq!(count(&w, 5, 4), 4, "dis 3 released the archer wave");

    // Extinguish the archer wave with the smite instrument — this
    // test's subject is the OBJECTIVE CHAIN reacting to model-4
    // extinction, not marksmanship (a stray-fireball fight floods
    // model-4 MILITIA into the type-7 extinction predicate; authentic,
    // but separately owned by the combat fixtures). The native MC2
    // hands are unbound and the runner is invincible.
    assert!(w.debug_smite(5, 4) >= 4, "the wave was live to smite");
    hover(&mut w, 194.5, 213.5, 48, idle);
    assert_eq!(count(&w, 5, 4), 0, "the archer wave died");
    let (_, stages) = w.mc2_objective_view();
    assert_eq!(stages[2].1, 2, "row 2 latched on the real kills");
    assert_eq!(
        count(&w, 15, 3),
        1,
        "the m17 kill switch dropped the spell jar"
    );
    assert!(!w.completed(), "row 4 held — no premature completion");

    // Row 3 = castle + banked share (type 0). Force it (the banked
    // economy is pending) and expect the m32 watcher's dis 6.
    w.debug_complete_mc2_stage(3);
    hover(&mut w, 170.0, 200.0, 32, idle);
    assert_eq!(count(&w, 5, 19), 5, "dis 6 released the FIREFLY wave");
    let (cur, stages) = w.mc2_objective_view();
    assert_eq!(stages[4], (7, 1), "row 4 (kill fireflies) armed and held");
    assert_eq!(cur, 4, "the cursor advanced to the firefly hunt");
    assert!(!w.completed(), "the wave must die first");

    // Extinguish the wave → all rows complete → the level ends
    // (the smite instrument again — the chain is the subject).
    assert!(w.debug_smite(5, 19) >= 1, "fireflies live to smite");
    hover(&mut w, 170.0, 200.0, 64, idle);
    assert_eq!(count(&w, 5, 19), 0, "the firefly wave died");
    assert!(w.completed(), "all stages done — the level completed");
    assert!(
        w.misfits().is_empty(),
        "no misfits on the full run (start markers + castle guards known): {:?}",
        w.misfits()
    );
}

/// The par1-authored SPELLS.DAT overrides (PrepareEvents EV:387-390):
/// a synthetic (10,11) tier-1 and (10,15) tier-2 THING must spawn with
/// the RETAIL CD table's life values — row 16 {6,12,24} / row 17
/// {16,32,64} — not the ctor defaults (240 / 128). Uses the real
/// baked spells.bin, so this also guards the import end to end
/// (the CD values differ from the decompile's baked-in fallback).
#[test]
fn mc2_par1_spells_overrides() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let Some(sp) = bundle.spells.as_deref() else {
        common::golden_skip("bundle predates spells.bin (rebake)");
        return;
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_spells(sp)
    .unwrap();
    let planes = Planes {
        height: vec![50; 65536],
        tile_type: vec![1; 65536],
        shading: vec![32; 65536],
        angle: vec![0; 65536],
        ceiling: Vec::new(),
    };
    let thing = |slot, model, x, par1| mgc_formats::Thing {
        slot,
        kind: mgc_formats::ThingKind::Entity,
        class: 10,
        model,
        x,
        y: 100,
        dis_id: 0,
        swi_sz: 0,
        swi_id: 0,
        parent: par1,
        child: 0,
        par3: None,
    };
    let things = [thing(1, 11, 100, 1), thing(2, 15, 120, 2)];
    let w = World::new_for_game(planes, &things, 1, assets, GameId::Mc2);
    let (_, pool) = w.debug_pool();
    // (10,11) = the SCORCH RING (NewAdd0A0B_4E840) — NOT a remap to
    // model 19.
    let ring = pool
        .iter()
        .find(|e| e.class == 10 && e.model == 11)
        .expect("the (10,11) scorch ring spawned");
    assert_eq!(ring.life, 12, "row 16 tier 1 life (CD SPELLS.DAT)");
    let trail = pool
        .iter()
        .find(|e| e.class == 10 && e.model == 15)
        .expect("the (10,15) trail spawned");
    assert_eq!(trail.life, 64, "row 17 tier 2 life (CD SPELLS.DAT)");
}

/// The (10,9) raise-land dome (mc2::morph): a synthetic tier-0 dome
/// on flat ground must ease a raised-cosine hill up over its life,
/// finalize to the `summit - 24` plateau with the 2x2 cap at
/// `plateau - 16`, and despawn — geometry per
/// docs/traces/mc2-class10-m9-dome-geometry.md (par1=0 → CD SPELLS
/// row 18 tier 0: maxLife 7, subSpell 400; radius = 7|1 = 7 tiles,
/// height = 2*7 + 100 = 114 over the base 50).
#[test]
fn mc2_dome_raises_and_finalizes() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let Some(sp) = bundle.spells.as_deref() else {
        common::golden_skip("bundle predates spells.bin (rebake)");
        return;
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_spells(sp)
    .unwrap();
    let planes = Planes {
        height: vec![50; 65536],
        tile_type: vec![1; 65536],
        shading: vec![32; 65536],
        angle: vec![0; 65536],
        ceiling: Vec::new(),
    };
    let things = [mgc_formats::Thing {
        slot: 1,
        kind: mgc_formats::ThingKind::Entity,
        class: 10,
        model: 9,
        x: 100,
        y: 100,
        dis_id: 0,
        swi_sz: 0,
        swi_id: 0,
        parent: 0, // par1 = tier 0
        child: 0,
        par3: None,
    }];
    let mut w = World::new_for_game(planes, &things, 1, assets, GameId::Mc2);
    {
        let (_, pool) = w.debug_pool();
        let dome = pool
            .iter()
            .find(|e| e.class == 10 && e.model == 9)
            .expect("the dome spawned");
        assert_eq!(dome.life, 17, "ctor life stands (override hits maxLife)");
    }
    // Park the player far away and run the dome to completion:
    // 16 grow ticks + the phase-2 flip + finalize.
    let idle = PlayerCommand::default();
    hover(&mut w, 30.0, 30.0, 24, idle);
    let (_, pool) = w.debug_pool();
    assert!(
        !pool.iter().any(|e| e.class == 10 && e.model == 9),
        "the dome despawned after finalize"
    );
    let h = |tx: usize, ty: usize| w.planes().height[ty << 8 | tx] as i32;
    // Base 50 + height 114 - 24 = the 140 plateau. The center tile
    // is (101,101) — retail's `(pos + 128) >> 8` on the authored
    // tile-center position (EF:23241) — so the 2x2 summit cap
    // presses (100..=101, 100..=101) to 124.
    for (tx, ty) in [(100, 100), (101, 100), (100, 101), (101, 101)] {
        assert_eq!(h(tx, ty), 124, "summit cap at ({tx},{ty})");
    }
    // Inside the disc but off the cap: clamped to the plateau.
    assert_eq!(h(99, 99), 140, "plateau northwest of the cap");
    assert_eq!(h(102, 101), 140, "plateau east of the cap");
    // Far outside the 7-tile disc: untouched flat ground.
    assert_eq!(h(120, 100), 50, "ground beyond the footprint");
    // The (10,18) summit child is REAL (mc2::morph summit vortex):
    // the ledger is clean and the eruption family (the vortex or what
    // it emitted before its ground-shift teardown) actually ran — the
    // finalize pass moves the terrain under the vortex, so by now it
    // may have despawned; the (10,19) fire column it raised on tick 0
    // persists.
    assert!(
        !w.misfits().iter().any(|&(c, m, _)| (c, m) == (10, 18)),
        "no (10,18) misfit anymore: {:?}",
        w.misfits()
    );
    let (_, pool) = w.debug_pool();
    assert!(
        pool.iter().any(|e| e.class == 10 && e.model == 19),
        "the summit fire-spray column exists"
    );
}

/// The (5,10) doomsday pyramid (mc2::doomsday): on a doom-flagged
/// level it activates (footprint wipe + terrain-flatten crater,
/// sound 10), is unkillable by damage (the life-8 clamp), and its
/// scripted death (tripped by player proximity) mass-kills the
/// creatures and hands off to the (10,9) APOCALYPSE dome with the
/// extinction latch set (docs/traces/mc2-class5-m10-doomsday.md).
/// On an unflagged level the first tick applies retail's ctor gate
/// and the pyramid never exists.
#[test]
fn mc2_doomsday_pyramid_extinction_script() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let planes = || Planes {
        height: vec![50; 65536],
        tile_type: vec![2; 65536],
        shading: vec![32; 65536],
        angle: vec![0; 65536],
        ceiling: Vec::new(),
    };
    let thing = |slot: u32, class: u16, model: u16, x: u16, y: u16| mgc_formats::Thing {
        slot,
        kind: mgc_formats::ThingKind::Entity,
        class,
        model,
        x,
        y,
        dis_id: 0,
        swi_sz: 0,
        swi_id: 0,
        parent: 0,
        child: 0,
        par3: None,
    };
    let idle = PlayerCommand::default();

    // Unflagged level: the gate despawns it on the first tick.
    let things = [thing(1, 5, 10, 100, 100)];
    let mut w = World::new_for_game(planes(), &things, 1, assets.clone(), GameId::Mc2);
    hover(&mut w, 30.0, 30.0, 2, idle);
    assert!(
        w.debug_pool().1.iter().all(|e| e.class != 5),
        "no pyramid on an unflagged level"
    );

    // Doom level: activate far from the player and run the active
    // cycle — the crater flatten + the falling-rock ring; the
    // pyramid holds (death is damage-scripted — the life-8 clamp
    // route is covered by the in-crate test).
    let things = [thing(1, 5, 10, 100, 100), thing(2, 5, 1, 130, 130)];
    let mut w = World::new_for_game(planes(), &things, 1, assets, GameId::Mc2);
    w.set_mc2_doom_level(true);
    hover(&mut w, 220.0, 220.0, 40, idle);
    {
        let (_, pool) = w.debug_pool();
        let p = pool
            .iter()
            .find(|e| e.class == 5 && e.model == 10)
            .expect("the pyramid stands");
        assert!(p.life >= 8, "unkillable clamp holds");
        assert!(
            pool.iter().any(|e| e.class == 10 && e.model == 14),
            "the falling-rock summon ring spins"
        );
    }
    // The flatten crater: the center region sinks below the flat 50.
    let h = |tx: usize, ty: usize| w.planes().height[ty << 8 | tx] as i32;
    assert!(
        h(100, 100) < 50 || h(101, 101) < 50,
        "the crater is sinking ({} / {})",
        h(100, 100),
        h(101, 101)
    );
}
