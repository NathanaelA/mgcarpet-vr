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
    // Re-pinned 2026-07-16 (DELIBERATE, BEHAVIORAL) — the LEVEL-END
    // marker law (checkpoint D only): dis-gated (14,3)/(14,4) ending
    // fly-to markers now spawn HIDDEN (flags 0x20) until their
    // (11,12)/(11,31) trigger trips and reveals them — player-
    // verified retail shows the portal only on trip (mc2:00 ending,
    // 2026-07-16). This level's exit cluster ((11,12) THING 465 +
    // (14,3) THING 466, dis 11) materializes in checkpoint C's
    // disposition storm, so D's hash gains the hidden bit; verified
    // by instrument: the flag is the ONLY delta (no endseq installs
    // — the parked pose never trips the (117,160) switch; won stays
    // false). A/B/C and every other fixture unmoved.
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
    // Re-pinned 2026-07-17 (DELIBERATE, BEHAVIORAL) — the (10,82)
    // room carve now consumes its authored THING pars on the load
    // path (PrepareEvents case 0x52, EV:373-379: par1/par2 = box
    // half-extents, par3 = depth multiplier); the port had left the
    // ctor's 3/3/2 defaults on every authored record, so entry
    // caverns carved as 6×6 closets. Level-014 authors two records
    // at (52,95) — pars (16,10,3) and (9,6,7) — so the load-settle
    // terrain (and everything downstream, checkpoint A on) moves.
    // Player-reported as the mc2:23 spawn-embedded-in-rock bug
    // (2026-07-17; that level's start chamber authors (58,42,9)).
    // Re-pinned 2026-07-17 (DELIBERATE, BEHAVIORAL) — the m21 devil
    // frog-jump port (checkpoint C only): the verbatim ctor zeroes
    // the jump impulse `word_0x2C_44` (f44); the old spawn misfiled
    // retail's `subSpellIndex_0x2A_42 = 400` into f44 (the bolt
    // thunk hard-sets 500, so the 400 was never read — pure spawn
    // state). This level's m21s materialize in C's disposition storm
    // STAGE-HELD, so the ctor field is their ONLY hash contribution —
    // verified by bisect: f44 400→0 alone reproduces the move; the
    // whole jump-cycle rewrite (sub_265A0 verbatim, replacing the
    // hover APPROX) and the m17 verbatim dive-step move NOTHING here
    // (held creatures never run their model tick; no manticore dives
    // in this window). A/B/load and every other fixture unmoved.
    // Re-pinned 2026-07-18 (DELIBERATE, BEHAVIORAL) — held devils
    // RUN the jump cycle (checkpoint C again): retail's m21 phase-7
    // wrapper (`sub_26470` EF:16938-61) calls `sub_265A0` after the
    // 1D5D0 legs for hold kinds 1-10, so this level's storm-
    // materialized held m21s now hop/settle/cackle (entity-LCG
    // draws + z motion + speed writes join the hash). Fixes the
    // player-reported floating devil (the held walk only ever
    // lifted z — nothing settled it back down) and the silent
    // mc2:08 basin. Unit-pinned by mc2_held_devil_settles_and_hops
    // (red-proven against the disabled call).
    // Re-pinned 2026-07-18 (DELIBERATE, BEHAVIORAL) — held DRAGONS
    // bob (checkpoint C again, the same held-seam family): retail's
    // m0 phase-7 wrapper (`sub_1F300`, m0-m3-gaps trace §2) runs the
    // vertical bob `sub_1F040` for hold kinds 1-10, so this level's
    // held m0s now arc off the terrain (f26 velocity + z motion join
    // the hash). Fixes the player-reported flat-flying "crippled
    // dragons" (mc2:08). Unit-pinned by
    // mc2_held_dragon_bobs_from_the_ground (red-proven).
    assert_eq!(
        got,
        vec![
            0xb9ef2aab49926cbcu64,
            0x7a89b38d106e4b85,
            0xda67b7efcb54c962,
            0x638e8fb0b8dc2512,
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
    // Re-pinned 2026-07-17 with the (10,82) authored-extents carve
    // (see the hash ledger above): the load-settle terrain around
    // (52,95) opens to the authored sizes, which moves the drawable
    // terrain, the parked open_spot AND the creature trajectories —
    // the projection move IS the intended observable change.
    // Re-pinned 2026-07-18 with the held-devil jump cycle (see the
    // hash ledger above): held m21s now visibly hop and settle —
    // checkpoint D's projection moves WITH the state hash, proving
    // the change is behavioral (z motion + sprite phases), exactly
    // what the fix intends.
    // Re-pinned 2026-07-18 again with the held-dragon bob (same
    // family, same checkpoint): held m0s visibly arc.
    const OBSERVABLE: [u64; 4] = [
        0xb0299049353c6c29,
        0x2d60a54a359da557,
        0x1ada7615a38d2848,
        0xc05c627a6f66b3ba,
    ];
    assert_eq!(
        obs, OBSERVABLE,
        "the OBSERVABLE projection diverged — this is a behavior \
         change, never a layout-only one"
    );
}

/// The wall-hug eye band (cave-peek diagnosis 2026-07-17): drive the
/// faithful MC2 mover straight into a sealed cave wall and pin that
/// the eye NEVER leaves the mover's clamp band — >= floor+256 and
/// <= ceiling-384 against the INTERPOLATED surfaces the renderer
/// draws (mesh == collision: same corner heights, same parity
/// diagonals). This exonerates the vertical clamps for the wall-peek
/// x-ray: the residual vector is the near plane cutting a hugged
/// steep face LATERALLY, which the terrain shader's backface-black
/// arm now paints as rock instead of x-raying the far chamber.
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

/// The ENHANCED-mover funnel squeeze (player repro 2026-07-17,
/// mc2:03 main cavern): the deviation mover had no cave narrow-space
/// law — nothing refused entry into the seam where floor meets
/// ceiling, and the old floor-wins pinch clamp then hoisted the head
/// THROUGH the diving ceiling ("squeezed further and further, and at
/// the end push your head through"). With the squeeze gate (the
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

/// The mc2:23 spawn-embedded-in-rock report (player, 2026-07-17):
/// level-023's (3,4) wizard start at (134,47) lies in baked-sealed
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
