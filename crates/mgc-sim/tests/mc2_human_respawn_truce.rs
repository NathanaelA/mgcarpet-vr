//! ROUND 141 — ⭐⭐⭐ THE POST-DEATH TRUCE IS NOT A RIVAL-ONLY LAW:
//! the HUMAN's respawn pushes every rival's hate ledger toward
//! colour 0 up to 0x9FDF too.
//!
//! `sub_5C950` is ONE routine for every wizard (EF:44185-93 in
//! `reference/remc2/remc2/engine/EventsFunctions.cpp`). Its truce loop sits
//! BELOW the `IsAiPlayer` fork and is completely ungated — shipped
//! `NETHERW.EXE` VA 0x5CE22..0x5CE5A (file 0x81622, = VA + 0x24800),
//! and `df 9f` is the ONLY occurrence of that immediate in the whole
//! binary:
//!
//! ```text
//!   5ce22: 66 8b 50 1a           mov    0x1a(%eax),%dx   ; kx->id_0x1A_26
//!   5ce26: 66 3b 53 1a           cmp    0x1a(%ebx),%dx   ; vs the respawner's
//!   5ce2a: 74 26                 je     0x5ce52
//!   5ce2c: 8a 50 40              mov    0x40(%eax),%dl   ; kx->model_0x40_64
//!   5ce2f: 84 d2                 test   %dl,%dl
//!   5ce31: 74 05                 je     0x5ce38          ; model 0 = HUMAN
//!   5ce33: 80 fa 01              cmp    $0x1,%dl
//!   5ce36: 75 1a                 jne    0x5ce52          ; model 1 = RIVAL
//!   5ce38: 8b 93 a4 00 00 00     mov    0xa4(%ebx),%edx
//!   5ce3e: 0f bf 4a 38           movswl 0x38(%edx),%ecx  ; playerColorIndex
//!   5ce42: 8b 90 a4 00 00 00     mov    0xa4(%eax),%edx
//!   5ce48: 66 c7 84 ca 04 02 00  movw   $0x9fdf,0x204(%edx,%ecx,8)
//!   5ce52: 8b 00                 mov    (%eax),%eax      ; kx = kx->next_0
//!   5ce54: 3b 05 e4 a3 01 00     cmp    0x1a3e4,%eax
//!   5ce5a: 77 c6                 ja     0x5ce22
//! ```
//!
//! ⚠ DISCRIMINATED ON AN ARGUMENT, NOT SHAPE (DIG-PROTOCOL §3): the
//! near-identical sibling 0x51 bytes below — `movw $0x601f,
//! 0x1fc(%ecx,%eax,8)` at 0x5cea3 — is the RESPAWNER'S OWN ledger
//! going back to neutral, and THAT one IS gated (`cmpb $0x1,0x40(%ebx)`
//! at 0x5ce7a, model 1 only). The truce loop above it has no such test.
//! Both walk `array_0x1FC_508` at an 8-byte stride, the same eight
//! words (0x204 + 8·colour vs 0x1fc + 8·l with `l` PRE-incremented
//! 1..8).
//!
//! The port had the write on the two RIVAL call paths only
//! (`mc2_spawn_rival` and the `sub_5C950` reuse arm), so a human
//! respawn left every rival's `hate[0]` running wherever
//! `mc2_rival_hate_decay` had carried it — §5, a known-correct law
//! that never reached one call path.
//!
//! RECORDED WITNESS — `recordings/mc2l16.mgcr` t=7934, the human's
//! respawn (the rivals' own decay of −(256 − Aggression) runs later
//! in the same tick):
//!
//! | wiz | t=7933 | port, no truce  | retail t=7934      |
//! |-----|--------|-----------------|--------------------|
//! | 1   | 24607  | 24607           | 40914 = 40927 − 13 |
//! | 4   | 45303  | 45279 (= −24)   | 40903 = 40927 − 24 |
//! | 5   | 54505  | 54459 (= −46)   | 40881 = 40927 − 46 |
//!
//! `hate` is an UNGRADED lane, so the debt rides 1,131 bit-exact
//! boundaries before it surfaces at t=9066, where rival 372's target
//! election picks slot 452 where retail picks 389 and the wizard's
//! `z` steps −4 instead of −8. The head classifies INHERITED — the
//! conformance pair re-imports `hate` every tick — so **no fixture
//! can witness this law** and a unit test is the correct lane
//! (docs/CONFORMANCE.md).
//!
//! Kill switch: `MGC_NO_MC2_HUMAN_RESPAWN_TRUCE=1`.
//!
//! Self-skips without baked mc2 data (game data is optional).

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use mgc_sim::{FlightInput, Simulation};
use std::path::Path;

/// `HATE_NEUTRAL` (0x601F) — the ledger's resting value.
const HATE_NEUTRAL: i64 = 24607;
/// `HATE_RESPAWN` (0x9FDF, `-24609` signed) — the truce write.
const HATE_RESPAWN: i64 = 40927;

fn load(level: &str) -> Option<World> {
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
    let assets = match bundle.mc2_extent_dims(&root.join("assets")) {
        Some(dims) => assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims)),
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
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some(w)
}

fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(wz), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in wz.wizards.iter().enumerate().take(8).skip(1) {
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

/// Every non-human wizard's `hate[0]` as the ungraded-lane projection
/// reports it (`World::wiz_shadow_mc2`, the same accessor the raw
/// shadow grades through).
fn rival_hate_toward_human(w: &World) -> Vec<(u8, i64)> {
    w.wiz_shadow_mc2()
        .into_iter()
        .filter(|ws| ws.wiz != 0)
        .filter_map(|ws| {
            ws.arrays
                .iter()
                .find(|(n, _)| *n == "hate")
                .and_then(|(_, v)| v.first().copied())
                .map(|h| (ws.wiz, h))
        })
        .collect()
}

/// ⭐⭐⭐ The human's respawn raises every rival's `hate[0]` to the
/// truce value, exactly as a rival's respawn does.
///
/// With `MGC_NO_MC2_HUMAN_RESPAWN_TRUCE=1` (the pre-dig port) the
/// ledgers stay at [`HATE_NEUTRAL`] and this test fails.
#[test]
fn the_human_respawn_raises_every_rival_hate_toward_colour_zero() {
    let Some(w) = load("level-004") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    // The death FALL needs the flyer, so drive the world through a
    // `Simulation` exactly as the MC1 death tests do.
    let mut sim = Simulation::with_world(w);

    // Settle so the tick-top class-3 roster (`dword_38519`) is built —
    // the truce walks THAT, not the rival vector.
    for _ in 0..32 {
        sim.step(&FlightInput::default());
    }
    let before = rival_hate_toward_human(sim.world.as_ref().unwrap());
    assert!(!before.is_empty(), "level-004 spawns rivals");
    assert!(
        before.iter().all(|&(_, h)| h == HATE_NEUTRAL),
        "no combat yet, so every ledger should still rest at neutral: {before:?}"
    );

    // Kill the human and ride the death fall out — `player_dead()`
    // only latches once the corpse lands and `sub_5C950` can run.
    sim.world.as_mut().unwrap().debug_kill_player();
    let mut landed = false;
    for _ in 0..4000 {
        sim.step(&FlightInput::default());
        if sim.world.as_ref().unwrap().player_dead() {
            landed = true;
            break;
        }
    }
    assert!(landed, "the human corpse lands");
    // Neutral again just before the respawn: whatever the fall did,
    // the truce is the only thing that can raise these.
    let dead = rival_hate_toward_human(sim.world.as_ref().unwrap());
    sim.step(&FlightInput {
        respawn: true,
        ..FlightInput::default()
    });
    assert!(
        !sim.world.as_ref().unwrap().player_dead(),
        "the respawn key revived the human"
    );

    let after = rival_hate_toward_human(sim.world.as_ref().unwrap());
    assert!(
        dead.iter().all(|&(_, h)| h <= HATE_NEUTRAL + 256),
        "the ledgers were not already elevated before the respawn: {dead:?}"
    );
    assert_eq!(after.len(), before.len(), "the roster did not change");
    for &(wiz, h) in &after {
        // `HATE_RESPAWN` minus at most one tick of this rival's own
        // decay (`256 - Aggression`, never more than 256).
        assert!(
            h > HATE_NEUTRAL && h <= HATE_RESPAWN && HATE_RESPAWN - h <= 256,
            "wiz {wiz}: hate[0] = {h}, wanted the truce value {HATE_RESPAWN} \
             (less at most one tick of decay); all = {after:?}"
        );
    }
}
