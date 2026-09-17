//! ROUND 147 — ⭐⭐⭐ THE POST-(RE)SPAWN TRUCE WALKS THE TICK-TOP
//! CLASS-3 ROSTER, NOT THE RIVAL VECTOR.
//!
//! Round 141 landed the truce loop's ROSTER semantics on the HUMAN
//! respawn only (`mc2_human_respawn_truce.rs`). The two RIVAL call
//! paths — `World::mc2_spawn_rival` (level-start seating) and the
//! `sub_5C950` reuse arm (a rival's respawn) — kept a flat
//! `for other in &mut self.mc2_rivals` loop that knows nothing about
//! the roster. §"A LAW ON ONE CALL PATH IS NOT LANDED", for the third
//! round running.
//!
//! Retail's loop reads its members from `dword_38519`, the CLASS-3
//! tick-top chain, and nothing else — shipped `NETHERW.EXE`
//! VA 0x5CE1A..0x5CE5C (file 0x8161A, = VA + 0x24800):
//!
//! ```text
//!   5ce1a: 8b 80 77 96 00 00     mov    0x9677(%eax),%eax  ; dword_38519 = class-3 head
//!   5ce20: eb 32                 jmp    0x5ce54
//!   5ce22: 66 8b 50 1a           mov    0x1a(%eax),%dx     ; kx->id_0x1A_26
//!   5ce26: 66 3b 53 1a           cmp    0x1a(%ebx),%dx     ; vs the respawner's
//!   5ce2a: 74 26                 je     0x5ce52
//!   5ce2c: 8a 50 40              mov    0x40(%eax),%dl     ; kx->model_0x40_64
//!   5ce2f: 84 d2                 test   %dl,%dl
//!   5ce31: 74 05                 je     0x5ce38            ; model 0 = HUMAN
//!   5ce33: 80 fa 01              cmp    $0x1,%dl
//!   5ce36: 75 1a                 jne    0x5ce52            ; model 1 = RIVAL
//!   5ce38: 8b 93 a4 00 00 00     mov    0xa4(%ebx),%edx
//!   5ce3e: 0f bf 4a 38           movswl 0x38(%edx),%ecx    ; playerColorIndex
//!   5ce42: 8b 90 a4 00 00 00     mov    0xa4(%eax),%edx
//!   5ce48: 66 c7 84 ca 04 02 00  movw   $0x9fdf,0x204(%edx,%ecx,8)
//!   5ce52: 8b 00                 mov    (%eax),%eax        ; kx = kx->next_0
//!   5ce54: 3b 05 e4 a3 01 00     cmp    0x1a3e4,%eax
//!   5ce5a: 77 c6                 ja     0x5ce22
//! ```
//!
//! `dword_38519` is rebuilt once per tick, BEFORE the input handlers,
//! and only takes `life_0x8 >= 0` records
//! (`reference/remc2/remc2/engine/EventsFunctions.cpp:40281-88`). Two
//! consequences the flat loop got wrong, both measured by round 147's
//! ungraded-lane census:
//!
//! 1. **LEVEL-START SEATING.** `sub_5C950` is the PlayerAction-1 arm
//!    of the per-tick input switch (EF:37887), so all eight wizards
//!    are seated inside ONE tick whose class-3 chain was built before
//!    any wizard record existed. The walk visits nobody. The port's
//!    flat loop instead stamped every already-seated rival, which is
//!    the TRIANGULAR `init-check` signature — wiz k wrong toward every
//!    colour > k: mc2l22-new 21 rows, mc2l17 15, mc2l12 6, port 40927
//!    minus the settle ticks' decay against retail's 24607.
//!
//! 2. **A RESPAWN WHILE OTHER WIZARDS ARE DEAD.** `recordings/mc2l17.mgcr`
//!    t=1500: wizards 1,2,3,4,6 are all in the dead-wait (action 3,
//!    `life` −3114/−2737/−2260/−2012/−670) and only wizard 5 and the
//!    human are alive. Colour 3's counter runs out (`scratch10 1 -> 0`)
//!    and it respawns at t=1501. Retail stamps wizard 5 alone; the port
//!    stamped 1,2,4,5,6 — the pair census's `wiz 2/4/6 [3]: retail
//!    24607 port 40927`, and 2,085 free-run rows behind them.
//!
//! `hate` is an UNGRADED lane (the conformance pair re-imports the
//! whole player block every tick), so **no recording fixture can
//! witness this law** — a unit test is the correct lane
//! (docs/CONFORMANCE.md).
//!
//! Kill switch: `MGC_NO_MC2_RIVAL_TRUCE_ROSTER=1` restores the flat
//! every-rival loop on both rival paths and BOTH tests below fail.
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

/// Every rival's `(slot, invuln, hate[0..8])` as the ungraded-lane
/// projection reports them — the same accessor the raw shadow grades
/// through.
fn ledgers(w: &World) -> Vec<(u8, i64, Vec<i64>)> {
    w.wiz_shadow_mc2()
        .into_iter()
        .filter(|ws| ws.wiz != 0)
        .map(|ws| {
            let grace = ws
                .scalars
                .iter()
                .find(|(n, _)| *n == "invuln")
                .map_or(0, |(_, v)| *v);
            let hate = ws
                .arrays
                .iter()
                .find(|(n, _)| *n == "hate")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            (ws.wiz, grace, hate)
        })
        .collect()
}

/// ⭐⭐⭐ LEVEL-START SEATING STAMPS NOBODY: the class-3 chain is
/// rebuilt before the input handlers, so the tick that seats all eight
/// wizards walks an EMPTY roster and every ledger starts — and stays —
/// at [`HATE_NEUTRAL`].
///
/// With `MGC_NO_MC2_RIVAL_TRUCE_ROSTER=1` (the pre-dig port) the flat
/// loop leaves the triangular residue `hate[j] = 40927` for every
/// `j > slot` and this test fails on the very first rival.
#[test]
fn level_start_seating_leaves_every_hate_ledger_neutral() {
    let Some(w) = load("level-022") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let seated = ledgers(&w);
    assert!(
        seated.len() >= 4,
        "level-022 seats seven rivals, got {}",
        seated.len()
    );
    for (slot, _, hate) in &seated {
        assert_eq!(
            hate,
            &vec![HATE_NEUTRAL; 8],
            "wizard {slot}'s ledger at seating: the truce loop walked an empty roster, \
             so nothing may be elevated (got {hate:?})"
        );
    }

    // …and the first ticks do not manufacture one either: the decay
    // only moves a value that is already off neutral.
    let mut sim = Simulation::with_world(w);
    for _ in 0..24 {
        sim.step(&FlightInput::default());
    }
    for (slot, _, hate) in ledgers(sim.world.as_ref().unwrap()) {
        assert!(
            hate.iter().all(|&h| h == HATE_NEUTRAL),
            "wizard {slot} after 24 settle ticks: {hate:?}"
        );
    }
}

/// ⭐⭐⭐ A WIZARD IN ITS DEAD-WAIT IS OFF `dword_38519` AND TAKES NO
/// TRUCE. Kill two rivals; when the first of them comes back, the one
/// still lying dead must keep its neutral ledger toward it, while a
/// rival that never died takes the full [`HATE_RESPAWN`].
///
/// This is `recordings/mc2l17.mgcr` t=1501 in miniature. With
/// `MGC_NO_MC2_RIVAL_TRUCE_ROSTER=1` the dead rival is stamped too and
/// this test fails.
#[test]
fn a_rival_in_its_dead_wait_takes_no_truce() {
    let Some(w) = load("level-022") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let mut sim = Simulation::with_world(w);
    // Settle so the tick-top class-3 roster is built and populated.
    for _ in 0..32 {
        sim.step(&FlightInput::default());
    }
    let slots: Vec<u8> = ledgers(sim.world.as_ref().unwrap())
        .iter()
        .map(|(s, _, _)| *s)
        .collect();
    assert!(slots.len() >= 3, "need three rivals, got {slots:?}");
    let (a, b, spectator) = (slots[0], slots[1], slots[2]);

    let w = sim.world.as_mut().unwrap();
    w.debug_kill_mc2_rival(a);
    w.debug_kill_mc2_rival(b);

    // Ride to the FIRST of the two respawns. `sub_5C950`'s reuse arm
    // is the ONLY thing that raises `grace` (`invuln`), so a RISING
    // EDGE on it is the respawn tick — robust against the countdown
    // that starts eating it in the same tick.
    let mut grace: [i64; 8] = [0; 8];
    for (s, g, _) in ledgers(sim.world.as_ref().unwrap()) {
        grace[s as usize] = g;
    }
    let mut first: Option<(u8, Vec<(u8, i64, Vec<i64>)>)> = None;
    for _ in 0..8_000 {
        sim.step(&FlightInput::default());
        let now = ledgers(sim.world.as_ref().unwrap());
        let edge = now
            .iter()
            .find(|(s, g, _)| (*s == a || *s == b) && *g > grace[*s as usize])
            .map(|(s, _, _)| *s);
        for (s, g, _) in &now {
            grace[*s as usize] = *g;
        }
        if let Some(slot) = edge {
            first = Some((slot, now));
            break;
        }
    }
    let Some((back, rows)) = first else {
        panic!("neither killed rival respawned inside 8,000 ticks");
    };
    let other = if back == a { b } else { a };
    let row = |s: u8| {
        rows.iter()
            .find(|(w, _, _)| *w == s)
            .cloned()
            .unwrap_or((s, 0, Vec::new()))
    };
    let (_, other_grace, dead) = row(other);
    assert_eq!(
        other_grace, 0,
        "wizard {other} must still be lying dead when {back} came back \
         (a respawn of its own would reset its whole ledger and make this vacuous)"
    );
    assert_eq!(
        dead.get(back as usize),
        Some(&HATE_NEUTRAL),
        "wizard {other} was still in its dead-wait when {back} respawned, so it is off \
         dword_38519 and takes no truce — ledger {dead:?}"
    );
    let (_, _, alive) = row(spectator);
    let got = alive.get(back as usize).copied().unwrap_or(0);
    // Sampled one tick after the stamp, so at most one decay step of
    // `256 - Aggression` has come off it.
    assert!(
        (HATE_RESPAWN - 256..=HATE_RESPAWN).contains(&got),
        "wizard {spectator} never died, so it IS on the roster and takes the full truce — \
         hate[{back}] = {got}, ledger {alive:?}"
    );
}
