//! Refactor guard: golden state-hash fixtures over real baked data.
//! The scripted run exercises triggers, dispositions, crater digging,
//! creature spawns and movement, rival wizards, spell grants,
//! projectile combat and the economy loop; [`World::state_hash`]
//! digests the FULL persistent state (pool internals, LCG streams,
//! mailboxes), so ANY behavioral divergence — however internal — trips
//! the fixture.
//!
//! The goldens pin the CURRENT port's behavior, not retail's: they are
//! a refactoring invariant, not a fidelity oracle. Regenerate (run
//! with `--nocapture` and copy the printed array) only when a
//! DELIBERATE behavior change lands, and say so in the commit.
//!
//! Self-skips when the baked tree is absent (game data is optional).

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc1::rivals::RivalConfig;
use mgc_sim::mc1::spells::SpellId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    p.join("mc1/level-005.mgcl").exists().then_some(p)
}

/// Level 005 with its authored wizards (rival preplants), mirroring
/// the app's WorldInit path.
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
    let mut w = World::new(planes, &pkg.things.things, seed, assets);
    if let Some(f) = pkg.gen_params.as_ref().and_then(|g| g.footer) {
        w.set_win_pct(f[0]);
    }
    let (wizards, player_count) = rival_configs(pkg.wizards.as_ref());
    w.set_wizards(&wizards, player_count);
    w
}

/// wizards.json → per-slot rival configs (the mgc-app resolver,
/// duplicated here because the test crate can't reach it).
fn rival_configs(wizards: Option<&mgc_formats::Wizards>) -> ([Option<RivalConfig>; 8], u16) {
    let mut out: [Option<RivalConfig>; 8] = Default::default();
    let Some(w) = wizards else { return (out, 1) };
    let count = w.player_count.unwrap_or(1).min(8);
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(acc), Some(tempo), Some(allowed_mask)) =
            (cfg.accuracy, cfg.tempo, cfg.allowed_spells.as_ref())
        else {
            continue;
        };
        let mut book = [false; 24];
        let mut allowed = [false; 24];
        for s in 0..24 {
            let a = allowed_mask.get(s).copied().unwrap_or(0) != 0;
            allowed[s] = a;
            book[s] = a && cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            accuracy: acc.clamp(0, 255) as u8,
            tempo: tempo.clamp(0, 255) as u8,
            castle_level: cfg.castle_level.unwrap_or(0),
            book,
            allowed,
        });
    }
    (out, count)
}

/// Hover near the ground at (x, z) for `ticks` turns under `cmd`.
fn fly(w: &mut World, x: f32, z: f32, ticks: usize, cmd: PlayerCommand) {
    for _ in 0..ticks {
        let alt = w.ground_height_tiles(x, z) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), cmd);
    }
}

/// The scripted run; returns the checkpoint hashes.
fn run(root: &std::path::Path) -> (Vec<u64>, Vec<u64>) {
    let mut w = build_world(root);
    let idle = PlayerCommand::default();
    let mut hashes = vec![w.state_hash()];
    let mut obs = vec![w.observable_digest()]; // post-init, pre-tick

    // A: idle far from everything — ambient economy + rival brains.
    fly(&mut w, 20.0, 20.0, 32, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // B: the (99,115) proximity trigger → disposition 1 (crater +
    // follow-up trigger); back off while the crater digs.
    fly(&mut w, 101.5, 117.5, 16, idle);
    fly(&mut w, 20.0, 20.0, 120, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // C: the follow-up trigger → disposition 2 (8-creature ambush).
    fly(&mut w, 95.5, 109.5, 16, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // D: combat over the ambush — dev spells, fireballs both hands
    // (projectiles, mailboxes, deaths, corpse mana balls).
    w.set_dev_spells(true);
    let equip = PlayerCommand {
        equip_left: Some(SpellId(0)),
        equip_right: Some(SpellId(23)),
        ..Default::default()
    };
    fly(&mut w, 95.5, 109.5, 1, equip);
    let firing = PlayerCommand {
        fire_left: true,
        fire_right: true,
        ..Default::default()
    };
    fly(&mut w, 95.5, 109.5, 64, firing);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    // E: aftermath — regen, decay, wandering survivors.
    fly(&mut w, 20.0, 20.0, 100, idle);
    hashes.push(w.state_hash());
    obs.push(w.observable_digest());

    (hashes, obs)
}

/// The limit-removing property: a bumped pool is bit-identical to
/// pristine MC1 up to the first exhaustion event. Level 005's scripted
/// run never exhausts, so the OBSERVABLE state (terrain, population,
/// poses) must match exactly — the raw state hash legitimately differs
/// (pool length + chassis are hashed).
#[test]
fn bumped_pool_is_transparent_without_exhaustion() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let observe = |chassis: mgc_sim::chassis::ChassisParams| {
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
        let mut w = World::new_with_chassis(planes, &pkg.things.things, seed, assets, chassis);
        fly(&mut w, 101.5, 117.5, 16, PlayerCommand::default());
        fly(&mut w, 95.5, 109.5, 64, PlayerCommand::default());
        let poses: Vec<_> = w
            .live_poses()
            .iter()
            .map(|p| (p.type_index, (p.x * 256.0) as i32, (p.z * 256.0) as i32))
            .collect();
        (w.planes().height.clone(), w.live_things().len(), poses)
    };
    let pristine = observe(mgc_sim::chassis::ChassisParams::MC1);
    let bumped = observe(mgc_sim::chassis::ChassisParams {
        pool_slots: 2000,
        ..mgc_sim::chassis::ChassisParams::MC1
    });
    assert_eq!(pristine, bumped, "bumped pool must be transparent");
}

#[test]
fn level_005_golden_state_hashes() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let (got, obs) = run(&root);
    // Bit-identical across runs before anything else.
    assert_eq!(
        (got.clone(), obs.clone()),
        run(&root),
        "sim is not deterministic"
    );
    println!("state hashes: {got:#018x?}");

    // These goldens encode retail's house-emit gate: the EXACT
    // equality `f26 == f128` (:30819), not a `>=` (which would let
    // every over-full house emit villagers forever — the runaway-
    // ecology trap: unbounded peasants + loose mana until pool
    // saturation). The house-emit law affects checkpoints D/E (the
    // fixture's first house fills during the combat window); A-C hold.
    // Any behavioral re-pin here moves the OBSERVABLE projection below
    // at the same checkpoints — expected and REQUIRED.
    //
    // Re-pinned for the m4 militia movement-core fix: retail's idle
    // (sub_1B5D0 :22541) and chase (sub_1A120 :21654) handlers both run
    // sub_196E0 (`creature_move`) every alive tick — the altitude-clamp
    // carrier — which our port had dropped, freezing collapse-spawned
    // militia mid-air. Restoring it (plus the idle wander jitter) settles
    // and wanders them; the first evacuee militia appear once the crater
    // dig reaches a house at checkpoint B, so post-init + A hold while
    // B-E move in BOTH the layout hash and the OBSERVABLE projection.
    //
    // Re-pinned for the m12 settler transcription fixes (sub_1EED0
    // :25077-84, sub_1F120 :25165-70) — pre-decrement +26 tests in
    // WANDER and APPROACH, and the `(f63 % v_26) / 2` think gate. This
    // level has settlers, so their ent_rand phase shifts B-E.
    //
    // Re-pinned for the corpse-flame two-pass fix (sub_25130 :28142-58):
    // retail's life test reads the PRE-decrement value, and the `& 2`
    // latch guards ONLY the one-shot sound — so a life-1 corpse puff
    // spawns its fire ring on TWO ticks. Our port tested post-decrement
    // AND returned early on the latch, so it spawned one ring: every
    // creature death delivered HALF its fire damage. Measured on a
    // 17-part worm crushed under a fresh level-1 castle: 10,400 before,
    // 20,400 after, against a 20,000 ladder — i.e. retail's reported
    // "the crush destroys the castle outright, or leaves the bar at 0".
    // The ~50% per-cell spawn gate is FAITHFUL and stays (confirmed in
    // remc2's independent decompile of a different binary,
    // engine/EventsFunctions.cpp:22793) — it was never the halving.
    // B-E move: the crater dig at B is the first thing that kills.
    //
    // Re-pinned for the m4 behavior-ROW fix (row 0 -> row 16): remc1's
    // m4 ctor (sub_386DE) could not resolve its row symbol and wrote
    // unk_98F38[0]; the unresolved declaration survives commented out
    // as `//int unk_99138;//fix` directly above it, and unk_99138
    // self-identifies as row 16. Row 0 is the flyer row (v_14=-4,
    // v_20=0xFFFFFFFF), which is why militia never descended and
    // walked out over water; row 16 is the ground-walker row
    // (v_14=-128, v_20=0xFFF080FE).
    // Blast radius CONFIRMED by probe, not assumed: level 005 holds
    // ZERO live m4 through post-init/A/B/C, gains its first at D and a
    // second at E. Exactly D and E move, in both arrays. (The
    // "militia appear at B" claim in the note below is stale — that
    // was written before the crater/evacuation timing changed.)
    //
    // Re-pinned for the authored-castle footprint fix: retail stamps
    // one build pass per authored level with the row = the pass index
    // (:54983-91), i.e. rows 0..=level, and BUILD row 0 is EMPTY.
    // `spawn_starting_castle` was passing `level + 1`, so every
    // authored rival castle wore one build ring more terrain than it
    // owned — and since the demolish un-stamps the row matching the
    // LEVEL, that surplus ring outlived the castle as a flagless
    // stump. Level 005's rivals hold authored castles, so their
    // load-time footprint shrinks by a ring: EVERY layout hash moves.
    //
    // Re-pinned for the rest of sub_1F120's APPROACH shape (:25164-77):
    // the walk runs before the think gate on every tick, the re-aim and
    // the proximity promotion run only INSIDE it, the patience /
    // dead-anchor bail falls through instead of returning (so it can
    // still promote to BUILD the same tick), +146 is never cleared, and
    // the range test is the three-axis ROOTED distance (sub_42340_42680
    // :52721), not a 2-D squared one. Settlers therefore arrive later:
    // on the isolated settler fixture the build tile is UNCHANGED
    // (123,107) but the build tick moves 154 -> 241. Post-init and A
    // hold; B-E move, as with every settler re-pin above.
    //
    // Re-pinned for the class-10 effect PRE-decrement batch: retail's
    // whole class-10 family reads the PRE-decrement life (sub_24F60
    // :28068, sub_25410 :28285, sub_25760 :28433, sub_25A60 :28592,
    // sub_262D0 :28906, sub_26360 :28933, sub_263C0 :28956, sub_26D20
    // :29311, sub_25CE0 :28685) while the class-9 FLIGHT family is
    // genuinely post-decrement. Our port had it backwards at the
    // class-10 sites and right at the class-9 ones, so every fire,
    // splash, flash, tether and cloud ran one tick short. B-E move;
    // post-init and A hold (nothing has died yet at A).
    //
    // Re-pinned for the militia idle +26 re-zero (sub_1B5D0 :22482):
    // retail's FIRST statement in the m4 idle handler clears the
    // walk-in flag every tick, so the silent-absorb death gate only
    // ever sees +26 != 0 on the one-tick house hop. Our port kept the
    // spawn stagger (+26 = slot % 100) alive into combat, so once
    // mob_death's gate widened to m4 virtually every militia despawned
    // silently — no corpse, no 500-mana ball. Level 005 holds no live
    // m4 until D, so exactly D and E move — and OBSERVABLE holds,
    // because no militia dies inside the window: the moved hashes are
    // the re-zeroed +26 field itself, layout-only by construction.
    //
    // Re-pinned for the m13/m14 feeder-wander transcription fixes
    // (sub_1F640 :25382-25438 / sub_1FAC0 :25558-25614): door radius
    // BEFORE fullness on the rooted 3-axis distance (the village
    // leash — a full home keeps pulling its villager back), act-speed
    // swaps on anchor drop/acquire (+126 = +130 / +128), the m14
    // distant filter INSIDE the acquire loop, and one think gate
    // wrapping both arms. Villager walk/absorb streams shift, so B-E
    // move; post-init and A hold (no feeder has thought yet).
    //
    // Re-pinned for the class-2 static tick port (sub_49AA0/sub_49AD0/
    // sub_49B50): stones, dolmens and bad stones now run their retail
    // per-tick handlers — the terrain snap plus the +18 |= 2 static
    // draw stamp (and the dolmen's wizard shrine sweep). A-E move,
    // post-init holds (the stamp first lands on tick 1). Layout-only
    // by construction: the stamp is the whole delta (disabling it
    // alone restores the old pins — on this run's static terrain the
    // snap is an identity write), and OBSERVABLE holds.
    // Re-pinned for the per-rival village-wanted timers: `rival_wanted`
    // ([i16; 8]) joined the Gen hash so the m4 militia and m8 griffon
    // wanted-gates can turn on hostile RIVAL wizards, not only the human.
    // Layout-only by construction — OBSERVABLE holds byte-for-byte below:
    // level 005's scripted run flags no rival wanted, so the whole delta
    // is the new zeroed field entering the hash input.
    const GOLDEN: [u64; 6] = [
        0x63df455d85b8d5d4, // post-init (feature pass + disposition 0)
        0x0e41eba270a335d3, // A: 32 idle ticks far afield
        0x1be71bd427acc65a, // B: crater trigger fired + 120 dig ticks
        0xfe576d8ed06bcf60, // C: ambush disposition fired
        0x370c9aae47ddfe20, // D: 64 ticks of two-hand fireball combat
        0x72f8f8fda6510afb, // E: 100 aftermath ticks
    ];
    assert_eq!(
        got, GOLDEN,
        "state hash diverged from the golden fixture — if this change \
         in behavior is DELIBERATE, re-pin (run with --nocapture) and \
         say so in the commit"
    );

    // The layout-INDEPENDENT companion golden: the observable
    // projection (poses + terrain + population) at the same
    // checkpoints. It must SURVIVE hashed-layout re-pins — when GOLDEN
    // moves but OBSERVABLE holds, the re-pin is layout-only by
    // construction; if OBSERVABLE moves too, behavior moved and the
    // claim must say so.
    // The castle-footprint re-pin above moves post-init ONLY: the
    // authored castles stamp one ring less terrain at load, and
    // nothing downstream diverges — A-E hold byte-for-byte, which is
    // the evidence that the fix changed the castles' footprint and
    // not the way the level plays.
    // The m12 APPROACH re-pin and the class-10 PRE-decrement batch BOTH
    // move OBSERVABLE at B-E, and that is the correct signal: these are
    // behavior changes, not layout changes. Settlers arrive later, and
    // every fire, splash, flash, tether and cloud lives one tick longer,
    // so populations and poses at B-E genuinely differ. Post-init and A
    // hold — nothing has died and no settler has thought yet.
    // The walk-in silent-absorb fix (mob_death now vanishes militia and
    // retired settlers that enter a house, matching retail's per-model
    // death slots, instead of dropping them into the corpse path whose
    // 400-dmg flame destroyed the dwelling and churned the village) moves
    // B-E again: those creatures no longer corpse, so no flame, no house
    // damage, and the populations that survive differ. A still holds.
    //
    // The whole array then re-pins once more — including post-init and A
    // — for a PRESENTATION change, NOT a behavior one: `live_poses` now
    // keeps unclaimed MC1 dwellings in the pose set (as `map_only`, so no
    // billboard and no map dot) purely so the debug health-bar overlay
    // can cover them. `observable_digest` hashes the pose set, so the
    // extra (unclaimed, always-present) house poses shift every
    // checkpoint. The raw GOLDEN state hash above is UNCHANGED — proof
    // the sim itself did not move.
    // The feeder-wander leash fix moves OBSERVABLE at B-E as well —
    // a behavior change by design: villagers steer home instead of
    // diffusing, walk in the door in different ticks, and the act
    // speeds they wear differ. Post-init and A hold.
    const OBSERVABLE: [u64; 6] = [
        0x3b95f7fa279c099d, // post-init — + unclaimed-dwelling poses
        0x203eb5b24d0ab0e0, // A
        0x5fb716e8a43f6e32, // B — settler phase + feeder leash
        0xdea926f4c8033595, // C
        0x028118efca7bae5f, // D
        0x9f172a5c5b77dd57, // E
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}
