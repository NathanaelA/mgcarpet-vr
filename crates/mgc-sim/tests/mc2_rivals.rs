//! MC2 rival column regression (Phase 4.3b, docs/traces/
//! mc2-rivals-brain.md + mc2-rivals-spawn-mortality.md +
//! mc2-rivals-open-closure.md): rivals spawn from the level record
//! under the NumberOfPlayers bound, carry their authored books and
//! castles, run the brain deterministically, and elimination feeds
//! the staged objective engine's kill-player cases.
//!
//! Runs against the real bakes (`baked/mc2`); skips silently when the
//! player's gamedata bake is absent (CI without game assets).

use mgc_formats::LevelPackage;
use mgc_sim::ids::GameId;
use mgc_sim::mc1::features::{FeatureAssets, Planes};
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

fn load(level: &str) -> Option<(World, LevelPackage)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.sprites.as_ref() {
        Some((sidx, _)) => {
            let dims: Vec<(u16, u16)> = sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
            assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims))
        }
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).ok()?,
        None => assets,
    };
    let file = std::fs::File::open(root.join("mc2").join(format!("{level}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
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
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some((w, pkg))
}

/// The app's `mc2_rival_configs` resolution, replicated for the
/// sim-side fixture (wizards.json MC2 shape + the header's authored
/// castle levels + the unk09 NumberOfPlayers bound).
fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(w), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(reflexes), Some(perception)) = (cfg.reflexes, cfg.perception) else {
            continue;
        };
        let mut start = [false; 26];
        let mut start_level = [0u8; 26];
        let mut blocked = [false; 26];
        for s in 0..26 {
            start[s] = cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
            start_level[s] = cfg
                .starting_spell_levels
                .get(s)
                .copied()
                .unwrap_or(0)
                .min(2);
            blocked[s] = cfg.blocked_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(Mc2RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            perception: perception.clamp(0, 255) as u8,
            reflexes: reflexes.clamp(0, 255) as u8,
            life: cfg.life.unwrap_or(0).max(0) as u16,
            castle_level: h.players[slot].max(0) as u8,
            start,
            start_level,
            blocked,
        });
    }
    (out, count)
}

fn count(w: &World, class: u8, model: u8) -> usize {
    w.debug_pool()
        .1
        .into_iter()
        .filter(|e| e.class == class && e.model == model && e.life >= 0)
        .count()
}

/// Level 004 (n=3): colors 1..2 spawn as rivals, the brain runs
/// deterministically, and the two kill-player stages (authored
/// 1-based payloads 3/2 -> colors 2/1) complete on elimination —
/// ending the level.
#[test]
fn mc2_rivals_spawn_brain_objective() {
    let Some((mut w, _pkg)) = load("level-004") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    // Two AI carpets ((3,1)) spawned; the human is out-of-pool.
    assert_eq!(count(&w, 3, 1), 2, "colors 1..2 spawn as rivals");
    let views = w.rival_views();
    assert_eq!(views.len(), 2);
    assert!(views.iter().all(|v| v.alive));
    assert_eq!(views[0].name, "Nyphur");

    // Determinism: the same run twice = the same state hash.
    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);
    let Some((mut w2, _)) = load("level-004") else {
        return;
    };
    for _ in 0..600 {
        w.tick(pose, idle);
        w2.tick(pose, idle);
    }
    assert_eq!(
        w.state_hash(),
        w2.state_hash(),
        "the rival brain is deterministic"
    );
    assert!(!w.completed(), "kill-player stages still open");

    // Eliminate both rivals. They may have BUILT castles during the
    // run (castle rung 0 costs exactly the starting 1000 mana) — a
    // dead rival with a castle RESPAWNS, so keep the castles smitten
    // too; a castle-less dead rival is BANISHED, which the two
    // kill-player stages read.
    for t in 0..8000 {
        if w.completed() {
            break;
        }
        if t % 16 == 0 {
            w.debug_kill_mc2_rival(1);
            w.debug_kill_mc2_rival(2);
            w.debug_smite(3, 2);
        }
        w.tick(pose, idle);
    }
    let views = w.rival_views();
    assert!(
        views.iter().all(|v| v.eliminated),
        "castle-less dead rivals are banished"
    );
    assert!(
        w.completed(),
        "both kill-player stages completed -> level end"
    );
}

/// RIVALS-POLISH #2: a dead MC2 wizard leaves a POSSESSABLE grave.
/// The bug was an inert grave (targetable bit 8 cleared, no ch1 claim
/// channel, a no-op dispatch arm), so the corpse could never be hit or
/// claimed and its re-pointed mana was lost forever. After the fix the
/// grave mirrors MC1 `spawn_grave` (action 42 `grave_tick`, `f28 = 2`,
/// bit 8 kept): a wizard's possession claim inherits everything the
/// grave owns, then it despawns.
#[test]
fn mc2_dead_wizard_grave_is_possessable() {
    let Some((mut w, _pkg)) = load("level-004") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // Kill a rival and let the death fall run to the grave spawn.
    // A respawn-capable rival can revive, so keep re-killing until the
    // grave materializes (mirrors mc2_rivals_spawn_brain_objective).
    let mut grave = false;
    for t in 0..2000 {
        if t % 16 == 0 {
            w.debug_kill_mc2_rival(1);
        }
        w.tick(pose, idle);
        if count(&w, 10, 40) > 0 {
            grave = true;
            break;
        }
    }
    assert!(grave, "the dead wizard leaves a (10,40) grave");

    // The human (PLAYER_TARGET) possesses the grave: it must respond to
    // the ch1 claim (the fix) and despawn, transferring every entity it
    // owned. The hook's debug_asserts also pin bit 8 + f28 == 2.
    let (before, after, freed) = w
        .debug_mc2_possess_grave(0xFFFF)
        .expect("a live grave to possess");
    assert!(freed, "possessing the grave despawns it (no longer inert)");
    assert_eq!(
        before, after,
        "every sphere the grave owned transfers to the possessor"
    );
}

/// RIVALS-POLISH: level-001's FIFTH objective (`index=9 stage=153`) is
/// a type-9 "destroy building" — razing the two vaults by Pyahandra's
/// tower. It was unported (`_ => false`), so the level could never end.
/// This drives the real level: force-complete rows 0-3 (which fires the
/// m32 stage-gated switch → disposition 8 → the two `par1=21` vaults),
/// confirm the level does NOT complete vacuously while the vaults live,
/// then raze them and confirm the type-9 row completes the level.
#[test]
fn mc2_level001_destroy_building_objective_completes() {
    let Some((mut w, _pkg)) = load("level-001") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    // Five objective rows; row 4 is the type-9 destroy-building.
    let (_, board) = w.mc2_objective_view();
    assert_eq!(board.len(), 5, "level-001 has five objective rows");
    assert_eq!(board[4].0, 9, "row 4 is the destroy-building objective");

    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // Force the first four objectives; the row-3 completion fires the
    // m32 switch (par1=3 → disposition 8) that spawns the two vaults.
    for row in 0..4 {
        w.debug_complete_mc2_stage(row);
    }
    // Let the switch fire and the vaults build out and park.
    for _ in 0..120 {
        w.tick(pose, idle);
        if w.debug_mc2_count_buildings(21) >= 2 {
            break;
        }
    }
    for _ in 0..50 {
        w.tick(pose, idle);
    }
    assert_eq!(
        w.debug_mc2_count_buildings(21),
        2,
        "the two par1=21 vaults spawned by the tower"
    );
    let (cursor, board) = w.mc2_objective_view();
    assert_eq!(cursor, 4, "the destroy-building row is current");
    assert_eq!(board[4].1, 1, "row 4 still active");
    assert!(
        !w.completed(),
        "level must NOT complete vacuously while the vaults stand"
    );

    // Raze the tag-21 stage once. Each vault DEGRADES into its byte_3
    // successor (bldgprm[21].chain = 54) — a fresh tag-54 building — so
    // the objective must NOT complete yet: the chain still has a live
    // stage. (A par1-21-only test would wrongly finish here.)
    w.debug_smite(10, 45);
    for _ in 0..60 {
        w.tick(pose, idle);
    }
    assert_eq!(w.debug_mc2_count_buildings(21), 0, "tag-21 stage collapsed");
    assert_eq!(
        w.debug_mc2_count_buildings(54),
        2,
        "each vault degraded into its tag-54 successor stage"
    );
    assert!(
        !w.completed(),
        "the chain still stands (tag 54) — objective must wait"
    );

    // Raze the tag-54 stage (bldgprm[54].chain = 0 → collapses fully).
    // Now the whole chain is gone → the destroy-building row completes.
    w.debug_smite(10, 45);
    for _ in 0..60 {
        w.tick(pose, idle);
        if w.completed() {
            break;
        }
    }
    assert_eq!(
        w.debug_mc2_count_buildings(54),
        0,
        "the vaults are fully razed"
    );
    assert!(
        w.completed(),
        "razing the whole chain completes the type-9 row → level end"
    );
}

/// MC2-STAGE-ENGINE-GAPS §A: objective type 1 (kill a NAMED creature)
/// was unported (`_ => false`), so any of its 21 levels could soft-lock.
/// The port binds the row to the live entity its authored THING index
/// spawns (`sub_58DA0`, EF:40650-90) and completes when that bound
/// creature is gone. Level-008 row 1 names THING 111 = a class-5 model-17
/// diver spawned at dis 0 (i.e. at load, BEFORE the app registers the
/// stages — so this also exercises the retroactive bind in
/// `set_mc2_stages`). Type 1 is a background row (not current-gated): it
/// must stay active while the diver lives and latch the moment it dies —
/// never vacuously at load.
#[test]
fn mc2_level008_kill_named_creature_objective_completes() {
    let Some((mut w, _pkg)) = load("level-008") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let (_, board) = w.mc2_objective_view();
    assert_eq!(board[1].0, 1, "row 1 is a kill-named-creature (type 1)");
    assert_eq!(board[1].1, 1, "row 1 active at load (not vacuously done)");
    // The named diver (THING 111 -> class-5 model-17) spawned at load and
    // bound. It is persistent (max_life 10000), so it will not self-expire.
    assert!(count(&w, 5, 17) >= 1, "the named diver spawned");

    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // Idle: the diver lives, so the background row must NOT complete.
    for _ in 0..40 {
        w.tick(pose, idle);
    }
    let (_, board) = w.mc2_objective_view();
    assert_eq!(
        board[1].1, 1,
        "row 1 stays open while the bound creature lives"
    );

    // Kill the diver: the bound row latches on the next objective pass.
    assert!(w.debug_smite(5, 17) >= 1, "smote the diver");
    for _ in 0..8 {
        w.tick(pose, idle);
    }
    let (_, board) = w.mc2_objective_view();
    assert_eq!(
        board[1].1, 2,
        "killing the named creature completes the type-1 row"
    );
}

/// MC2-STAGE-ENGINE-GAPS §A: objective type 2 (kill NAMED target "for
/// real") shares type 1's bind seam; in the port it reduces to type 1
/// (no slot-swap creature succession exists — metamorph is a cosmetic
/// pose-puppet, so no death is ever a transform handoff, and the razed
/// building's collapse successor is a fresh slot the bound row does not
/// follow). EVERY shipped type-2 target is a NAMED BUILDING (class-10
/// model-45), not a plain creature. Level-008 row 3 names THING slot 63
/// = a building released by disposition 1 — so this exercises the
/// SPAWN-TIME bind hook in `spawn_from_thing` (vs the type-1 test's
/// retroactive load-time bind). The row must bind the named instance
/// specifically, not any model-45, and latch when that instance dies.
#[test]
fn mc2_level008_kill_named_building_type2_completes() {
    let Some((mut w, _pkg)) = load("level-008") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let (_, board) = w.mc2_objective_view();
    assert_eq!(board[3].0, 2, "row 3 is a kill-for-real (type 2)");
    assert_eq!(board[3].1, 1, "row 3 active at load");

    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // The named target (slot 63) is dis-1-gated — not yet live, so
    // row 3 is unbound. Smiting any load-time buildings must NOT complete
    // it (the bind is entity-specific, not by-model).
    w.debug_smite(10, 45);
    for _ in 0..8 {
        w.tick(pose, idle);
    }
    let (_, board) = w.mc2_objective_view();
    assert_eq!(
        board[3].1, 1,
        "row 3 unbound — its named target is not among any load buildings"
    );

    // Release it (disposition 1): the building spawns and binds through
    // the spawn seam.
    w.debug_fire_disposition(1);
    for _ in 0..30 {
        w.tick(pose, idle);
    }
    assert!(count(&w, 10, 45) >= 1, "dis 1 released the named building");
    let (_, board) = w.mc2_objective_view();
    assert_eq!(board[3].1, 1, "row 3 still open — the bound building lives");

    // Raze it: the bound type-2 row latches on the next objective pass.
    assert!(w.debug_smite(10, 45) >= 1, "razed the named building");
    for _ in 0..8 {
        w.tick(pose, idle);
    }
    let (_, board) = w.mc2_objective_view();
    assert_eq!(
        board[3].1, 2,
        "razing the named building completes the type-2 row"
    );
}

/// MC2-STAGE-ENGINE-GAPS §B: the StageVar hold-gate layer
/// (`crate::mc2::stagevars`). A gated creature spawns HELD (frozen at its
/// phase-7 wait) until its trigger fires; then it drops to its active
/// action. Level-019 holds four model-16 creatures on a KIND-3 gate
/// (release when a bound entity dies): they must stay dormant while it
/// lives and all release when it dies. This exercises the load-time
/// retroactive attach + the per-tick reaction + the death-watch scan.
#[test]
fn mc2_level019_stagevar_holds_until_bound_death() {
    let Some((mut w, _pkg)) = load("level-019") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    // Four model-16 creatures are held at load (kind 3).
    let held0 = w.debug_mc2_held();
    assert_eq!(held0.len(), 4, "level-019 holds four creatures at load");
    assert!(
        held0
            .iter()
            .all(|&(_, model, kind)| model == 16 && kind == 3),
        "all four are model-16 on a kind-3 (bound-death) gate: {held0:?}"
    );

    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // A kind-3 gate does not self-release: the creatures stay dormant.
    for _ in 0..60 {
        w.tick(pose, idle);
    }
    assert_eq!(
        w.debug_mc2_held().len(),
        4,
        "kind-3 holds do not release on their own"
    );

    // Kill the watched entity (smite every creature model): the gate
    // fires and all four release to their active action.
    for m in 0..30 {
        w.debug_smite(5, m);
    }
    for _ in 0..20 {
        w.tick(pose, idle);
    }
    assert!(
        w.debug_mc2_held().is_empty(),
        "the bound entity's death released every held creature: {:?}",
        w.debug_mc2_held()
    );
}

/// MC2-STAGE-ENGINE-GAPS §B: the KIND-6 (timer) gate — a held creature
/// releases after a fixed countdown, with no external trigger. Level-104
/// holds two model-16 creatures whose timers are 2020/2040 ticks; both
/// must still be held well before then and both released after.
#[test]
fn mc2_level104_stagevar_timer_releases() {
    let Some((mut w, _pkg)) = load("level-104") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let held0 = w.debug_mc2_held();
    assert_eq!(held0.len(), 2, "level-104 holds two creatures at load");
    assert!(
        held0
            .iter()
            .all(|&(_, model, kind)| model == 16 && kind == 6),
        "both are model-16 on a kind-6 (timer) gate: {held0:?}"
    );

    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // Still held at 1000 ticks (both timers are > 2000).
    for _ in 0..1000 {
        w.tick(pose, idle);
    }
    assert_eq!(w.debug_mc2_held().len(), 2, "held while the timer runs");

    // Past both countdowns → both released.
    for _ in 0..1100 {
        w.tick(pose, idle);
    }
    assert!(
        w.debug_mc2_held().is_empty(),
        "the timer expired and released both: {:?}",
        w.debug_mc2_held()
    );
}

/// Level 022 (n=8, seven authored rival castles): every configured
/// color gets its castle at load, Life-scaled, full of mana.
#[test]
fn mc2_rivals_authored_castles() {
    let Some((mut w, pkg)) = load("level-022") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let players = pkg.header.as_ref().unwrap().players;
    let expected = players[1..].iter().filter(|&&p| p > 0).count();
    assert_eq!(count(&w, 3, 1), 7, "colors 1..7 spawn");
    assert_eq!(
        count(&w, 3, 2),
        expected,
        "one authored castle per configured color"
    );
    // The castles stand (action 4) and survive the brain running.
    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);
    for _ in 0..300 {
        w.tick(pose, idle);
    }
    assert_eq!(count(&w, 3, 2), expected, "castles stand through play");
    assert_eq!(w.rival_views().len(), 7);
}

#[test]
fn mc2_steal_mana_casts_a_projectile_not_a_stub() {
    // Steal Mana (13) used to be a `note_misfit` stub that charged mana
    // for zero effect. It is now a class-9 subtype-8 homing bolt whose
    // (10,25) impact stamps the struck wizard's ch3 "steal" inbox (the
    // rival/human ch3 consumers already drain + credit). Deterministic
    // lock: casting it spawns a real (9,8) bolt. (The full drain is
    // exercised by the pre-existing ch3 consumers + manual playtest;
    // the economy is not cleanly observable headless — dev_spells masks
    // the caster pool and rivals self-spend their mana.)
    let Some((mut w, _pkg)) = load("level-004") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    w.set_dev_spells(true);
    let pose = PlayerPose::from_tiles(64.0, 40.0, 64.0, 0.0, 0.0, 0.0);
    for _ in 0..6 {
        w.tick(pose, PlayerCommand::default());
    }
    w.mc2_select_spell(13, 0, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    assert_eq!(
        count(&w, 9, 8),
        1,
        "casting Steal Mana launches the (9,8) homing bolt"
    );
}
