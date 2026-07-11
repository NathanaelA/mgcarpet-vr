//! The Phase-3 EXIT FIXTURE (ROADMAP "Phase 3", item 3.6): the MC2
//! vertical slice on real level-000 data under the full MC2 profile.
//! Every criterion is POSITIVELY exercised (the spec review's
//! anti-vacuous rule): creatures are found via the pool debug view,
//! the player is parked next to them, and the observable is asserted
//! per model — Goat wakes/flees/dies (mana sphere + kill credit),
//! Archers stand and FIRE (arrow entity + danger music), Villager
//! wanders and never attacks, the type-5 fly-to objective latches at
//! its authored point.
//!
//! Golden hashes pin the slice (the MC1 goldens in state_hash.rs are
//! untouched by all of this — the columns share the chassis, not the
//! fixtures). Self-skips without baked mc2 data.

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
fn run(root: &std::path::Path) -> Option<Vec<u64>> {
    let mut w = build_world(root)?;
    let idle = PlayerCommand::default();
    let mut hashes = vec![w.state_hash()];

    // A: idle far from everything — awake pass + wander cadences.
    hover(&mut w, 16.0, 16.0, 64, idle);
    hashes.push(w.state_hash());

    // B: the type-5 fly-to objective at (115, 212).
    hover(&mut w, 115.5, 212.5, 8, idle);
    hashes.push(w.state_hash());

    // C: park next to a goat — awake + flee.
    if let Some((vx, vz)) = find_creature(&w, 5, 1) {
        hover(&mut w, vx + 2.0, vz, 96, idle);
    }
    hashes.push(w.state_hash());

    // D: NATIVE fireballs at the nearest goat (playtest-13: the MC1
    // equip bridge no longer casts on the MC2 column — the seeded
    // book's LEFT fireball is the cast path now). RIGHT unbinds so
    // possession pulses don't spray claims over the script; tier-0
    // fireball is CLICK cadence, so the volley pulses the edge.
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

    Some(hashes)
}

#[test]
fn mc2_slice_behaviors_and_goldens() {
    let Some(root) = baked_root() else {
        eprintln!("skipped: baked mc2 data not present");
        return;
    };
    let Some(got) = run(&root) else {
        eprintln!("skipped: mc2 level-000 has no baked terrain");
        return;
    };
    assert_eq!(got, run(&root).unwrap(), "slice is not deterministic");
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
            .filter(|v| matches!(v.kind, mgc_sim::mc1::world::VolumeKind::Proximity))
            .count()
            >= 20,
        "switch boxes feed the map-triggers overlay"
    );
    assert!(
        w.active_volumes()
            .iter()
            .any(|v| matches!(v.kind, mgc_sim::mc1::world::VolumeKind::Objective)),
        "stage checkpoints plot on the overlay"
    );

    // Villager wanders (and never attacks). The building creators'
    // walkable village paint FREES the townsfolk (the Phase-3
    // terrain-imprisonment gap closed) — free walkers can now also
    // die authentically (die-on-water row flag on a boxed-in
    // wander), so survival is not asserted; kill CREDIT staying
    // zero is (townsfolk/construction deaths never credit).
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
    // pad). The bare type-1 village stand-in is gone.
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

    // The herd condensed into follow-chains during the idle window
    // (the walker trace's flock law: pack scans latch leaders, no
    // tether exists — the corrected v_2 turn keeps the mill tight).
    assert!(
        w.debug_pool()
            .1
            .iter()
            .any(|e| e.class == 5 && e.model == 1 && e.state == 11),
        "goats formed follow-chains (state 11)"
    );

    // Goat flee: ONLY wander/patrol run the wizard scan — followers
    // flee by COPYING their leader (sub_1C560's leader-state switch)
    // — so shadow a WANDERING chain head (state base+1 = 9),
    // re-acquiring each tick: a single static parking spot gets one
    // cone roll per cadence and can sit in the ±150° scan's blind
    // spot (or lose the head to the pack fallback) indefinitely.
    // Sampled DURING the hover — the goat drops back to idle once it
    // escapes the row's range, exactly the retail loop.
    let mut fled = false;
    for _ in 0..384 {
        let Some((vx, vz)) = w
            .debug_pool()
            .1
            .into_iter()
            .find(|e| e.class == 5 && e.model == 1 && e.life >= 0 && e.state == 9)
            .map(|e| (e.tx as f32 + 0.5, e.ty as f32 + 0.5))
        else {
            break; // whole herd chained; heads re-emerge next drop
        };
        let alt = w.ground_height_tiles(vx + 1.5, vz) + 2.0;
        w.tick(
            PlayerPose::from_tiles(vx + 1.5, alt, vz, 0.0, 0.0, 0.0),
            idle,
        );
        if w.debug_pool()
            .1
            .iter()
            .any(|e| e.class == 5 && e.model == 1 && e.state == 14)
        {
            fled = true;
            break;
        }
    }
    assert!(fled, "a goat near the player entered FLEE (14)");

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
    // mana 500 drops a sphere. NATIVE cast (playtest-13: the MC1
    // equip bridge no longer casts on the MC2 column): rebind the
    // seeded fireball onto LEFT, keep RIGHT unbound (no possession
    // claims over the kill loop).
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
    // arrows can't wipe the pack anymore — the (9,13) probe honors
    // the class-3 target filter now.
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

    // Pinned goldens (first pinned 2026-07-09, the Phase-3 landing;
    // re-pinned same day, DELIBERATE: the (10,45) building creator
    // landed — 47 at-load buildings raise/paint terrain at init, the
    // fixture feeds the mc2-night bundle's own SEARCH/BUILD/BLDGPRM,
    // and the (9,13) arrow probe honors the class-3 target filter.
    // Regenerate with --nocapture on DELIBERATE behavior change and
    // say so in the commit.
    const GOLDEN: [u64; 6] = [
        // Re-pinned 2026-07-09 (DELIBERATE), four landings in one
        // session: (1) the (10,45) texture-band pass
        // (sub_45DC0/sub_462A0 — real building ground + MC2-native
        // retile/shade, night inversion per the level header, fixture
        // sets set_mc2_night_shade like the app); (2) class-11
        // SWITCHES live ((11,0..=3) known + ticking) — the scripted
        // sweep genuinely crosses switch boxes and fires their
        // disposition chains; (3) the ROUTE-CHAIN entities native:
        // (2,0..=2) tree/stone/dolmen, (10,0) ground fire, (10,1)
        // big-explosion seeder — and the MC1-fallback spell chain's
        // spawn_effect resolves models 0/1 into the native MC2 ctors;
        // (4) the PROGRESSION GATES: (11,32) stage-gated + (11,4)
        // level-end switches (Mc2Stage carries its authored row —
        // the par1 key), and the behavior probes now unlock the
        // archers through the authored checkpoint chain.
        // Re-pinned 2026-07-10 (DELIBERATE), the Phase-4.3 FULL
        // ROSTER landing: the wave-A class-5 creatures (14 models),
        // the class-9 flyer core + creature attack thunks, the
        // class-2 band + tree burn ladder, the class-11 slot-switch
        // band + X-markers, class-14 map objects, and the (10,1)
        // corpse burst replacing its misfit note. The at-load hash
        // is UNCHANGED (level-000's initial population authored none
        // of the new models); the run diverges when the disposition
        // chain releases the previously-misfit (5,19) flyer wave and
        // the tree-burn/corpse handlers act.
        // Re-pinned 2026-07-10 (DELIBERATE), the MULTIPART landing:
        // class-5 models 0/3/22/27 leave the misfit ledger — ONLY
        // checkpoint E moved (level-000's dis chain releases its
        // four (5,3) flyers late in the census window; each now
        // spawns a live 17-entity chain instead of a misfit note).
        // Post-init through D are UNCHANGED.
        // Re-pinned 2026-07-10 (DELIBERATE), the PLAYTEST-2 batch:
        // (1) the (10,45) TEMPLATE FIX — disposition-authored MC2
        // buildings no longer run MC1's building_fixup(par1+16); the
        // id stays the ctor's raw par1 (remc2 EF:33089), par2 lands
        // in xtype/f66; (2) the building claim/damage column — f56 =
        // 33 (+2 productive), occupancy 2, the state-52 house tick
        // (claim + militia + death) and the state-53 teardown;
        // (3) the smoke-column family (10,59)/(10,60) emitters +
        // (10,13)/(10,14) particles live; (4) the (10,29)
        // waypoint-chain PATH STAMP at generate time (the causeway —
        // terrain changes from tick 0) + the one-tick stage marker;
        // (5) the real (10,5) splash replacing every misfit note.
        // Re-pinned 2026-07-10 (DELIBERATE), the class-15 TOKEN
        // landing: the stage chain's runtime (15,2) spell jar now
        // spawns live (sprite 77, pickup states) instead of a
        // misfit note — post-init through B are UNCHANGED (level-000
        // authors no load-time class-15/(10,39|58)/(14,1)/(10,63|64)
        // records); C/D/E move from the jar's spawn tick onward.
        // Re-pinned 2026-07-10 (DELIBERATE), the possession-delivery
        // fix: mc2_spawn_building now mirrors the intake bits into
        // f28 (=1, |=2 productive) — the shared writer gate
        // (area_write tests f28) finally sees MC2 buildings, so the
        // possess pulse's ch1 claim mail and ch0 area damage reach
        // the house tick (docs/traces/mc2-possession-delivery.md).
        // Every checkpoint moves: 47 at-load buildings hash the new
        // field from post-init on.
        // Re-pinned 2026-07-10 (DELIBERATE), the level-000 mission-
        // chain batch: MC2 census world seed 1 (retail sub_61F50 —
        // was MC1's intrinsic 1000), the type-7 kill objective's
        // current-cursor gate + the m32 ObjectiveDone_2 pause
        // (:40724/:54371 — rows no longer latch vacuously before
        // their targets spawn), the Mc2Stage force-complete flag in
        // the hash, and the (3,4..=11) start markers leaving the
        // misfit ledger. Every checkpoint moves (the census seed
        // shifts world_mana from post-init on).
        // Re-pinned 2026-07-10 (DELIBERATE), the SPELLS.DAT import:
        // FeatureAssets now carries the parsed spells.bin table
        // (hash-when-present, like bldgprm) — every checkpoint moves
        // by the asset hash alone; no behavior consumed it yet at
        // pin time.
        // Re-pinned 2026-07-11 (DELIBERATE), the worm link-length
        // floor (multipart.rs): zero-spacing chain children keep the
        // head's authored 96 — the PLAYTEST-11 "the whole worm is a
        // blob" fix. Only checkpoint E moved (the chain spawned in
        // its window carries the new f56).
        // Re-pinned 2026-07-11 (DELIBERATE), the objective-chime
        // correction (stage-engine trace): the advance chime is
        // retail's 61 (Success2), fired only when the CURRENT row
        // completes or the level ends and suppressed at cursor 0 —
        // was a 41 stand-in played on every completion. B onward
        // move (the fly-to latch tick emits the new sound event).
        // Re-pinned 2026-07-11 (DELIBERATE), the Phase-4.2 SPELL
        // COLUMN landing: (1) SetDefaultSpells_5C0A0 — every MC2
        // world seeds fireball + possess manifestations at init
        // (2 pool slots + the spell book hash from post-init on);
        // (2) the jar pickup KEEPS the collected entity as the
        // manifestation (retail's slot economy — the bank-and-
        // despawn interim is closed); (3) the D/E windows unbind
        // the native MC2 hands (MC1 dev methodology stays the
        // script's subject). Every checkpoint moves.
        // Re-pinned 2026-07-11 (DELIBERATE), the playtest-12 spell
        // fixes: the SetSpell cadence flag (`byte_0x3B_59` → f59 on
        // the seeded manifestations — post-init moves) and the
        // sub_67CB0 one-shot auto-aim (lock-less projectiles
        // acquire targets instead of flying straight — D/E move).
        // Re-pinned 2026-07-11 (DELIBERATE), the derived sprite
        // extents (the retail load-time pass EF:44870-44910 —
        // speed_6 from the bitmap aspect): every entity's collision
        // box changes from spawn on (the zero-box fireball/goat
        // tunneling fix + the PLAYTEST-11 worm-spacing provenance
        // closure).
        // Re-pinned 2026-07-11 (DELIBERATE), the playtest-13 ghost-
        // cast gate: the MC1 equip bridge no longer casts on the
        // MC2 column (grant_spell no-ops, the MC1 hand arm is
        // column-gated) — the D/E combat legs now fire the NATIVE
        // book's fireball (click-cadence edge pulses; payload 250
        // per docs/traces/mc2-fireball-damage.md). D/E move,
        // post-init through C are unchanged.
        // Re-pinned 2026-07-11 (DELIBERATE), the Phase-4.5 session-2
        // load-time cave arms: the shared chassis retile_and_shade
        // (dig_cell's tail recompute) gained MC2's twin arms — the
        // non-Day shade INVERSION (Terrain.cpp:2030-2033; retail's
        // sub_56F10 digs resolve through sub_462A0/46570, which
        // invert on night/cave maps — ours didn't) and the cave
        // floor↔ceiling invariant. Level-000 is a night level, so
        // every dig after load now writes inverted shades: A onward
        // move, post-init is unchanged (no dig in settle). MC1
        // goldens untouched (both arms are data-variant no-ops
        // there); cave levels get their first correct dig shading.
        // Re-pinned 2026-07-12 (DELIBERATE), the audio column: the
        // objective-message trigger ramp (`byte_0x36E02`,
        // docs/traces/mc2-voiceover-triggers.md §3) is new retail
        // sim state hashed with the stage board — set_mc2_stages
        // arms the briefing voiceover at load, so every checkpoint
        // including post-init moved. MC1 goldens untouched (the
        // ramp only ever arms through the MC2 stage machinery).
        // Re-pinned 2026-07-14 (objective types 1/2 bind field): the
        // `Mc2Stage` struct gained a `bound: Option<u16>` slot for the
        // named-target entity binding. level-000 authors no type-1/2
        // objective, so no binding occurs and no behavior changed — the
        // move is purely the extra `None` joining each stage's hash
        // (present from load, so every checkpoint moved uniformly).
        // Re-pinned 2026-07-14 (StageVar subsystem): the level's StageVar
        // table now loads + hashes. level-000's three kind-1 vars are
        // INERT (word=0, no matching THING), so nothing is held and no
        // behaviour changed — the move is purely the StageVar table
        // joining the hash (present from load → every checkpoint moved).
        0xdae4409d5a6168a8, // post-init (GenerateEvents + dis 0)
        0xd751a3f1cadddcef, // A: 64 idle ticks afield
        0x5d8778a11af07cb5, // B: the type-5 fly-to latched
        0x8ed6f81e715cee0e, // C: goat awake/flee window
        0x41bc756f3088cc9e, // D: fireball combat over the goat
        0x2de9f1b1aa9971e8, // E: census + villager/archer provocation
    ];
    assert_eq!(
        got, GOLDEN,
        "the MC2 slice diverged from its goldens — if DELIBERATE, \
         re-pin (--nocapture) and say so in the commit"
    );
}

/// Level-000's authored mission chain, end to end (the 2026-07-10
/// player report: the game stalled after the spell jar — the
/// castle-build stage never armed and the FIREFLY wave never came):
/// fly-to rows 0/1 → archers (dis 3) → kill them → row 2 latches
/// (only while CURRENT — the type-7 cursor gate) and the m17 kill
/// switch drops the (15,3) spell jar → row 3 (type 0: castle + 15%
/// banked share; forced here — the banked economy is the Phase-4.6
/// track) → the m32 row-3 watcher fires dis 6 = FIVE (5,19)
/// fireflies while row 4 arms → killing the wave completes the
/// level. The m32 ObjectiveDone_2 pause keeps rows 2/4 from
/// latching vacuously in the one-tick gap before their targets
/// spawn.
#[test]
fn mc2_level000_mission_chain() {
    let Some(root) = baked_root() else {
        eprintln!("skipped: baked mc2 data not present");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipped: mc2 level-000 has no baked terrain");
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

    // Kill the archers (dev fireballs, LEFT hand only — the
    // firehose sprays strays that kill villagers, and a village
    // offense floods model-4 MILITIA into the type-7 extinction
    // predicate; authentic, but not this test's subject). The
    // native MC2 hands are unbound (the default fireball/possess
    // book), and the runner is invincible — arrows aren't the
    // subject either.
    // Extinguish the wave with the smite instrument — this test's
    // subject is the OBJECTIVE CHAIN reacting to model-4 extinction,
    // not marksmanship (the old firehose loop was a marginal fight:
    // strays kill villagers → village offense → model-4 MILITIA
    // flood the extinction predicate; authentic, separately owned
    // by the combat fixtures).
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
        eprintln!("skipped: baked mc2 data not present");
        return;
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let Some(sp) = bundle.spells.as_deref() else {
        eprintln!("skipped: bundle predates spells.bin (rebake)");
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
    // (10,11) = the SCORCH RING (NewAdd0A0B_4E840 — the playtest-
    // cave round-2 trace correction; the old "remaps to model 19"
    // reading was the m6-doc numbering trap).
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
        eprintln!("skipped: baked mc2 data not present");
        return;
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let Some(sp) = bundle.spells.as_deref() else {
        eprintln!("skipped: bundle predates spells.bin (rebake)");
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
    // The (10,18) summit child is REAL since the 2026-07-11 misfit
    // sweep (mc2::morph summit vortex): the ledger is clean and the
    // eruption family (the vortex or what it emitted before its
    // ground-shift teardown) actually ran — the finalize pass moves
    // the terrain under the vortex, so by now it may have despawned;
    // the (10,19) fire column it raised on tick 0 persists.
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
        eprintln!("skipped: baked mc2 data not present");
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
