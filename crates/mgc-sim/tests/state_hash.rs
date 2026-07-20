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
    const GOLDEN: [u64; 6] = [
        0x795499327cc36b28, // post-init (feature pass + disposition 0)
        0xe37dd14011ee7d15, // A: 32 idle ticks far afield
        0xd586b0f8e4e7a45a, // B: crater trigger fired + 120 dig ticks
        0x33a250c42d61569b, // C: ambush disposition fired
        0x9f6a5fd47305a944, // D: 64 ticks of two-hand fireball combat
        0xd81dccfbd92bcbd9, // E: 100 aftermath ticks
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
    const OBSERVABLE: [u64; 6] = [
        0x09a4bbee6ed601d4,
        0x797dd4817a1d1f11,
        0xbb23b68555315fd5,
        0xfa89ab230f971f40,
        0x5cf85ee7f75b41d2,
        0x8e471bd2f137dbb4,
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}
