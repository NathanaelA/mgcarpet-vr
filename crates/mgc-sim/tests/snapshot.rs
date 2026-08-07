//! The save/load acceptance test (`docs/archive/DESIGN-SAVES.md`): snapshot a
//! played sim, apply it onto a freshly built one, and require the two
//! to be indistinguishable — then keep ticking both and require them
//! to STAY indistinguishable.
//!
//! The unit tests beside the codec (`engine::world::tests::snapshot_*`)
//! cover the mechanics on a micro-world and can reach the pool
//! internals the digests cannot see. This file covers what they
//! cannot: real baked levels, with real thing tables, real rival
//! columns, and — for MC2 — the stage engine, the spell book, and the
//! cave ceiling. A codec that only ever met the micro-world would
//! serialize a great many fields that are permanently zero there.
//!
//! Skips silently when the baked tree is absent; `MGC_REQUIRE_GOLDENS=1`
//! turns a skip into a failure.

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use mgc_sim::{FlightInput, Simulation, ThrustModel};
use std::path::{Path, PathBuf};

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc1/level-005.mgcl").exists() && !common::modded_bake(&p)).then_some(p)
}

fn planes_of(pkg: &LevelPackage) -> Option<Planes> {
    let terrain = pkg.terrain.as_ref()?;
    Some(Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    })
}

fn read_pkg(path: &Path) -> Option<LevelPackage> {
    mgc_formats::mgcl::read(std::fs::File::open(path).ok()?).ok()
}

/// MC1 level 005 — the fixture the other MC1 goldens use.
fn mc1_world(root: &Path) -> Option<World> {
    let pkg = read_pkg(&root.join("mc1/level-005.mgcl"))?;
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?;
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    Some(World::new(
        planes_of(&pkg)?,
        &pkg.things.things,
        seed,
        assets,
    ))
}

/// MC2 level 001, wired the way the app wires it: night shading, the
/// stage engine, and the stage-variable table. Without the stages a
/// large slice of the MC2 state this codec has to carry stays empty.
fn mc2_world(root: &Path) -> Option<World> {
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).ok()?,
        None => assets,
    };
    let pkg = read_pkg(&root.join("mc2/level-001.mgcl"))?;
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(
        planes_of(&pkg)?,
        &pkg.things.things,
        seed,
        assets,
        GameId::Mc2,
    );
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

fn sim_of(w: World) -> Simulation {
    let mut s = Simulation::with_world(w);
    s.thrust_model = ThrustModel::Mc1;
    s.sync_carpet_from_flyer();
    s
}

/// Fly a varied course so the snapshot is taken over state that has
/// actually moved: thrust, a banked turn, a coast, and a burst of
/// left-hand casting to stir the pool and the spell channels.
fn play(s: &mut Simulation) {
    let legs = [
        (
            60,
            FlightInput {
                thrust: 1.0,
                stick_y: -30,
                ..Default::default()
            },
        ),
        (
            60,
            FlightInput {
                thrust: 1.0,
                stick_x: 96,
                yaw_delta: 0.04,
                fire_left: true,
                ..Default::default()
            },
        ),
        (60, FlightInput::default()),
        // Arm the MC2 barrel roll and stop TEN ticks in, so the MC2
        // snapshot below is taken MID-ROLL and the divergence loop
        // proves the driver state survives the codec (MC1 worlds
        // ignore the command — the flight-verb gate).
        (
            1,
            FlightInput {
                barrel_roll: true,
                ..Default::default()
            },
        ),
        (10, FlightInput::default()),
    ];
    for (n, input) in legs {
        for _ in 0..n {
            s.step(&input);
        }
    }
}

/// snapshot → restore → identical → tick both → still identical.
fn acceptance(label: &str, build: impl Fn() -> World) {
    let mut live = sim_of(build());
    play(&mut live);
    let bytes = live.snapshot();

    let mut restored = sim_of(build());
    // The fixture only means something if the target starts DIFFERENT.
    assert_ne!(
        live.state_hash(),
        restored.state_hash(),
        "{label}: fresh world already matches the played one"
    );
    restored
        .restore(&bytes)
        .unwrap_or_else(|e| panic!("{label}: restore failed: {e}"));

    assert_eq!(
        live.state_hash(),
        restored.state_hash(),
        "{label}: state hash differs immediately after restore"
    );
    let (a, b) = (
        live.world.as_ref().unwrap(),
        restored.world.as_ref().unwrap(),
    );
    assert_eq!(
        a.observable_digest(),
        b.observable_digest(),
        "{label}: observable projection differs after restore"
    );

    // The divergence half: a field the codec dropped may not show up
    // until whatever reads it next runs. 600 ticks is the design's
    // number, and it is generous — most drops split within a handful.
    let cruise = FlightInput {
        thrust: 1.0,
        stick_x: -24,
        ..Default::default()
    };
    for i in 1..=600 {
        live.step(&cruise);
        restored.step(&cruise);
        assert_eq!(
            live.state_hash(),
            restored.state_hash(),
            "{label}: diverged {i} ticks after restore"
        );
    }
    assert_eq!(
        live.world.as_ref().unwrap().observable_digest(),
        restored.world.as_ref().unwrap().observable_digest(),
        "{label}: observable projection diverged over 600 ticks"
    );
}

#[test]
fn mc1_level_005_snapshot_round_trips() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let Some(_) = mc1_world(&root) else {
        common::golden_skip("mc1 bundle not present");
        return;
    };
    acceptance("mc1:005", || mc1_world(&root).unwrap());
}

#[test]
fn mc2_level_001_snapshot_round_trips() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let Some(_) = mc2_world(&root) else {
        common::golden_skip("mc2 bundle not present");
        return;
    };
    acceptance("mc2:001", || mc2_world(&root).unwrap());
}

/// A snapshot must not be applicable to a world it was not taken in.
/// The pool and table geometry decide what every slot handle in the
/// stream MEANS, so a mismatch has to be refused rather than
/// half-applied.
#[test]
fn snapshot_refuses_a_foreign_world() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let (Some(mc1), Some(mc2)) = (mc1_world(&root), mc2_world(&root)) else {
        common::golden_skip("both bundles needed");
        return;
    };
    let mut a = sim_of(mc1);
    play(&mut a);
    let bytes = a.snapshot();

    let mut b = sim_of(mc2);
    let before = b.state_hash();
    let err = b
        .restore(&bytes)
        .expect_err("an MC1 snapshot is not an MC2 save");
    assert!(
        matches!(err, mgc_sim::snapshot::SnapshotError::Identity { .. }),
        "expected an identity rejection, got {err:?}"
    );
    // Refused BEFORE anything was written: the sim is still playable.
    assert_eq!(
        before,
        b.state_hash(),
        "a refused snapshot must leave the world untouched"
    );
}

/// Informational: the uncompressed payload size, which the save
/// container's compression choice depends on. Not an assertion beyond
/// a sanity bound — it moves whenever the pool or a plane does.
#[test]
fn snapshot_size_is_sane() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    for (label, w) in [("mc1:005", mc1_world(&root)), ("mc2:001", mc2_world(&root))] {
        let Some(w) = w else { continue };
        let mut s = sim_of(w);
        play(&mut s);
        let n = s.snapshot().len();
        println!("{label}: {n} bytes ({} KiB)", n / 1024);
        assert!(
            (64 * 1024..4 * 1024 * 1024).contains(&n),
            "{label}: {n} bytes is outside any plausible range"
        );
    }
}
