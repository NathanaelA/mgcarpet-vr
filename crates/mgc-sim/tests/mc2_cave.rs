//! The Phase-4.5 CAVE FIXTURE (ROADMAP "4.5 Cave levels", step (g)):
//! real level-014 data — the roster-richest cave (32 pillars, 61
//! brutes, 92 bees, 25 switches) — under the full MC2 profile.
//! Positively exercised: the load settle (sculptors, pillar MEASURE
//! and the load-time arms) holds the floor↔ceiling invariant over the
//! whole map, the cave-only roster spawns ((14,2)/(5,24)/(2,6)), the
//! cave-EXCLUDED ctors spawn NOTHING, the (10,86) drip spawner fires
//! on its 8-turn cadence, and a NATIVE Cave-In cast (spell 25, the
//! one cave-only spell) flies, impacts and collapses terrain through
//! the (9,30) → (10,89) chain.
//!
//! Golden hashes pin the trajectory (the MC1 goldens in
//! state_hash.rs and the mc2_slice level-000 goldens are untouched —
//! shared chassis, separate fixtures). Self-skips without baked mc2
//! data.

use mgc_sim::ids::GameId;
use mgc_sim::mc1::features::{FeatureAssets, Planes};
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc2/level-014.mgcl").exists() && p.join("assets/mc2-cave/build.tab.bin").exists())
        .then_some(p)
}

fn build_world(root: &std::path::Path) -> Option<World> {
    let file = std::fs::File::open(root.join("mc2/level-014.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let terrain = pkg.terrain.as_ref()?;
    let ceiling = terrain.ceiling.clone().unwrap_or_default();
    if ceiling.is_empty() {
        // A pre-EPOCH-8 bake has no ceiling plane — nothing to pin.
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
    Some(w)
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

/// THE invariant over the whole map (the tmp_caveprobe check):
/// ceiling > floor ⇔ bit3 clear.
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

    // StageVar hold-gate: with the row table baked VERBATIM (epoch
    // 13 — the 2026-07-16 flocking fix; the old bake read byte0 as
    // SIGNED and dropped every flagged row, then compacted the rest,
    // so only the kind-9 var survived here), level-014's full table
    // loads: the kind-9 model-18 (THING 334, gate = template-6 death
    // — never fires in this run) PLUS the previously-lost flagged
    // rows binding kind-1 walkers, kind-4 guardians and kind-6 timer
    // spawns. Pin the census by kind and the original kind-9 anchor.
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
    // Re-pinned 2026-07-11 (playtest-cave round 2): the (10,11)
    // scorch-ring trace correction — authored (10,11)s were riding
    // the (10,19) spray ctor (the m6-doc §0 numbering trap) and
    // flooding the pool; level-014 authors them, so the whole
    // trajectory legitimately moved.
    // Re-pinned 2026-07-12 (audio column): the objective-message
    // trigger ramp (`byte_0x36E02`, docs/traces/mc2-voiceover-
    // triggers.md §3) is new retail sim state hashed alongside the
    // stage board — every staged MC2 level's trajectory legitimately
    // moved with the level-load briefing arm.
    // Re-pinned 2026-07-13 (mana-magnet fidelity fix): the (10,54)
    // aura `sub_38D80` now DRAGS mana balls toward its eye (the retail
    // magnet) instead of the mis-ported creature grip. level-014 has
    // one aura + 30 balls, so the post-load checkpoints move as the
    // balls stream inward (the load-time checkpoint 0 is unchanged).
    // Re-pinned 2026-07-13 (mana downhill-roll): the ball tick now
    // applies the retail terrain gradient (`sub_58030`) to a grounded
    // ball's velocity, so balls roll downhill toward the low basin
    // (the level-001 transport the aura alone couldn't do). The 30
    // level-014 balls settle on real cave terrain, so the ticked
    // checkpoints (1 and 3) legitimately move; the load-time (0) and
    // pre-tick (2) checkpoints are unchanged.
    // Re-pinned 2026-07-13 (aura range/life from stageTag): the
    // disposition-spawned (10,54) aura now reads its RANGE and LIFE
    // from the THING's stageTag (`sub_4A310` EF:33095-33104) instead of
    // the ctor's fixed 14-tile/128 default. level-014's aura carries
    // stageTag 9 → a 9-tile reach (not 14), so the ball trajectory
    // (checkpoints 1 and 2) legitimately moves; the aura's f26 field
    // value also joins the hash.
    // Re-pinned 2026-07-14 (objective types 1/2 bind field): the
    // `Mc2Stage` struct gained a `bound: Option<u16>` slot for the
    // named-target entity binding. level-014 authors no type-1/2
    // objective, so no binding occurs and no behavior changed — the move
    // is purely the extra `None` joining each stage's hash.
    // Re-pinned 2026-07-14 (StageVar subsystem): the level's StageVar
    // table (`crate::mc2::stagevars`) now loads + hashes. level-014's
    // kind-9 var HOLDS the one model-18 (THING 334) at its phase-7
    // wait — its gate (template-6 death) never fires in this run, and
    // the two kind-1 vars (word=0) match nothing — so the move was
    // the StageVar table PLUS that one held binding joining the hash
    // (this note originally claimed "no creature held"; corrected
    // Session H9 to match the assertion above).
    // Re-pinned 2026-07-16 (DELIBERATE), Session E creature batch:
    // level-014's two (5,22) worms now carry the retail ctor
    // `f28 = 3` (E5 — the ch1 designation-mail admit; the whole
    // retarget→colorize machine was dead code behind f28=1), plus
    // the shared-mover/awake changes (E13 unconditional retry
    // terrain test, E15 hidden-skip + sphere pass) that shift any
    // creature trajectory from load. The file's behavioral asserts
    // (incl. count(5,27)==0) still pass. MC1 goldens untouched.
    // Re-pinned 2026-07-16 (DELIBERATE), Session G castle/cave
    // geometry — the review-fix batch (docs/REVIEW-FIX-PLAN-
    // 2026-07-15.md Session G) absorbed in ONE pin:
    //   G3  mesa floor writes now always retile (sub_570F0 keys the
    //       retile on a4, EF:39702-08) + clear the low nibble on
    //       h==0 (EF:39660) — tile_type/shading/angle move over
    //       mesa footprints;
    //   G7  the drip sprite roll (EF:37025) and pit/hill depth roll
    //       (EF:25639) draw the u16 entity rand, not raw lcg32 —
    //       the load-time cave-gen RNG stream shifts from the first
    //       draw;
    //   G8a dome/pit/hill measure to the tile CORNER (i<<8,
    //       EF:25496/25666 — no +128): every bowl/mound shifts half
    //       a tile;
    //   G8b the dome seal sync is bit3-only (EF:25522-25 — the old
    //       ceiling=floor−1 pin was the pit/hill law leaking in);
    //   G8c the tube wall ring covers side+1 dims (EF:25243);
    //   G9f cave_wall_ring is sub_34B00 VERBATIM: the SE corner is
    //       never stamped, the bottom row/right column stamp
    //       angle+retile without the type write;
    //   G9h the cave-in debris z reads retail's stale one-past-the-
    //       box neighbor cell (EF:23052-77) — this fixture's
    //       Cave-In cast moves with it.
    // All behavioral asserts above still pass; MC1 goldens and every
    // other fixture untouched (verified this session).
    // Re-pinned 2026-07-16 (DELIBERATE), Session J2 hash field tags —
    // LAYOUT-ONLY, zero behavior change: conditional hash-quiet fields
    // now write a distinct tag byte when (and only when) they
    // contribute, closing the adjacent-field aliasing class
    // (drain/scrolls/tokens, aura_claim/wanted, debuffs,
    // apocalypse/doom). This fixture legitimately exercises two of
    // them — mc2_spell_tokens is live from load (the fireball+possess
    // baseline, bitmask 3, moves checkpoint 0 too) and mc2_aura_claim
    // carries 10 live claims once ticked — so every checkpoint gains
    // tag bytes. Verified by instrumenting the tag writes: only tags
    // 3 (spell_tokens) and 4 (aura_claim) fire here. mc2_slice and
    // all MC1 goldens unmoved (their tagged fields never fire).
    // Re-pinned 2026-07-16 (DELIBERATE, BEHAVIORAL) — the FLOCKING
    // fix, two stacked corrections:
    //   (1) StageVar rows bake VERBATIM (epoch 13): the importer's
    //       signed-byte filter had dropped every FLAGGED row (byte0
    //       high bits 0x80/0x40) and compacted the rest, so most of
    //       this level's authored holds never existed — the held
    //       census above grows from 1 binding to 60;
    //   (2) stage-held creatures now RUN retail's `sub_1D5D0`
    //       movement legs (walk-to-point / graze-leash / shadow —
    //       EF:10171/10246/10111) instead of freezing at their spawn
    //       pose, and aggro-broken (kind-10) creatures re-leash via
    //       `sub_12500` case 0xA (EF:5054). Creature trajectories
    //       legitimately move from load. The OBSERVABLE projection
    //       moves too — this is real behavior, verified against the
    //       remc2 retail memimages (level-1 goat herd: all state 15,
    //       speed 18, leashed mill — the port now reproduces it).
    assert_eq!(
        got,
        vec![
            0xbe8e98cc7e7e8646u64,
            0x992ea558917fd24b,
            0xda0122efc0451fb3,
            0x28ee1084a6fdd590,
        ],
        "cave goldens moved — re-pin ONLY for an intended fidelity change"
    );

    // The layout-INDEPENDENT companion golden (review J3, pinned
    // 2026-07-16) — see state_hash.rs: survives hashed-layout
    // re-pins; moves ONLY with real behavior.
    // Moved 2026-07-16 WITH the flocking fix (BEHAVIORAL — the
    // stage-held cast now exists and MOVES): checkpoint 0 (load
    // time) is UNCHANGED — the world composition is identical at
    // t=0 — and the three ticked checkpoints move with the held
    // creatures' walk/graze trajectories, exactly the projection's
    // design (a layout-only change could not move it).
    const OBSERVABLE: [u64; 4] = [
        0x5fdfbe7cfbf8fc43,
        0x10cbd033029e9130,
        0x4c18438350ec3cb0,
        0x36cb50b763124752,
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}
