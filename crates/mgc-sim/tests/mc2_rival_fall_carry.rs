//! ROUND 98 — ⭐⭐⭐ THE FALL VELOCITY IS NEVER RESET: it accumulates
//! across a rival wizard's LIVES.
//!
//! `sub_12A70`'s lethal branch (EF:5416-19) is EXACTLY two
//! instructions in the shipped `NETHERW.EXE` — file 0x3740A-0x37415,
//! linear 0x12BAA:
//!
//! ```text
//!   3740a: 83 f8 02        cmp  $0x2,%eax          ; sub_5EFA0(a1x) == 2
//!   3740d: 75 30           jne  0x3743f
//!   3740f: c6 43 45 02     movb $0x2,0x45(%ebx)    ; actionIndex_0x45_69 = 2
//!   37413: 31 d2           xor  %edx,%edx          ; return 0
//!   37415: e9 49 02 00 00  jmp  <epilogue>
//! ```
//!
//! — it stamps the action byte and returns. **There is no write to
//! `word_0x2C_44` on it.** `sub_5E310`'s gravity leg (EF:60081-90)
//! then reads whatever @0x2C already held, so every death RESUMES the
//! previous death's terminal velocity, and a rival that has died
//! before drops out of the sky the instant it dies again.
//!
//! ⭐⭐⭐ AND IT IS A SPLIT IN A SIBLING PAIR, PROVEN BYTE-FOR-BYTE ON
//! BOTH ARMS OF THE SHIPPED BINARY. The HUMAN's lethal transition,
//! `AddPlayer03_00_5E010` (EF:60036-40, file **0x82949-0x82953**),
//! does clear it:
//!
//! ```text
//!   82949: c6 43 45 02          movb $0x2,0x45(%ebx)  ; actionIndex = 2
//!   8294d: 81 c2 8e 6e 00 00    add  $0x6e8e,%edx
//!   82953: 66 c7 43 2c 00 00    movw $0x0,0x2c(%ebx)  ; word_0x2C_44 = 0
//! ```
//!
//! The RIVAL's (0x3740F) carries only the first of those two stores.
//! The port had copied the human arm's reset onto the rival arm and
//! made the pair uniform — the classic "an absence in an enumerated
//! list is the law", here with both halves of the list disassembled.
//! The MC2 human column (`world.rs`, `player.fall_speed = 0`) is
//! therefore CORRECT as written and must not be "fixed" to match.
//!
//! Recorded witness (`state.struct_b64` @0x2C on rival 378 of
//! `recordings/mc2l6-rival-spells-galore.mgcr`): 0 while alive, the
//! t=1846 fall walks it to −76, it HOLDS −76 through the whole next
//! life (respawn t=3084), the t=14996 death RESUMES at −76 and steps
//! −78/−80/−82/−84, holds −84 to t=20747, and so on through
//! −86/−90/−104/−112 over six deaths. The port stamped `f46 = 0`
//! alongside the action byte, so every fall but the first started
//! from rest: retail steps 378's z 1114 → 1034 at t=14997 where the
//! port stepped 1114 → 1110. That was the take's HORIZON and the
//! single biggest wall family in it (15,229 gated ticks, 4 segments).
//!
//! The lane (`word_0x2C_44`, the port's `f46`) is NOT in `EntObsMc2`,
//! and the conformance PAIR re-imports it every tick, so no fixture
//! can see this law — the whole-take pair census is byte-identical
//! with it on and off. Per docs/CONFORMANCE.md that makes a unit test
//! the correct lane. Kill switch: `MGC_NO_RIVAL_FALL_CARRY=1`.
//!
//! Self-skips without baked mc2 data (game data is optional).

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

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

/// The rival's carpet altitude, or `None` while it has no live
/// class-3 record (the grave/respawn window).
fn wizard_z(w: &World, slot: usize) -> Option<i16> {
    w.debug_flock_probe(3, 1)
        .into_iter()
        .find(|r| r.slot == slot)
        .map(|r| r.z)
}

fn wizard_slot(w: &World) -> Option<usize> {
    w.debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 3 && e.model == 1 && e.life >= 0)
        .map(|e| e.slot)
}

/// The record's `actionIndex_0x45_69` (1 alive, 2 death fall,
/// 3 dead-wait), or `None` if the slot is not a live wizard record.
fn state_of(w: &World, slot: usize) -> Option<u8> {
    w.debug_pool()
        .1
        .into_iter()
        .find(|e| e.slot == slot && e.class == 3 && e.model == 1)
        .map(|e| e.state)
}

/// ⭐⭐⭐ A rival's SECOND death falls faster than its first, because
/// retail never clears `word_0x2C_44`.
///
/// The test drives one rival through two complete deaths on the same
/// pool record and compares the z drop on the FIRST tick of each fall
/// (`actionIndex` 2, `sub_5E310`'s gravity leg). Retail's first fall
/// starts from rest; the second starts from whatever velocity the
/// first left behind, so the second opening step is strictly the
/// larger drop. With `MGC_NO_RIVAL_FALL_CARRY=1` (the pre-dig
/// invented `f46 = 0`) the two openings are identical and this test
/// fails.
#[test]
fn a_rival_second_death_resumes_the_first_death_s_terminal_velocity() {
    let Some(mut w) = load("level-004") else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let views = w.rival_views();
    assert!(!views.is_empty(), "level-004 spawns rivals");
    let color = views[0].slot;
    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);
    let slot = wizard_slot(&w).expect("a live rival wizard record");

    let mut openings: Vec<i32> = Vec::new();
    let mut falls: Vec<usize> = Vec::new();
    for cycle in 0..2 {
        // Keep the letters coming until the record actually enters the
        // death fall (`tick70` = 2). A rival heals, so one letter is
        // not always lethal.
        let mut entered = false;
        for _ in 0..6000 {
            if state_of(&w, slot) == Some(2) {
                entered = true;
                break;
            }
            w.debug_kill_mc2_rival(color);
            w.tick(pose, idle);
        }
        assert!(entered, "cycle {cycle}: the rival reaches its death fall");
        // The opening step of THIS fall.
        let a = wizard_z(&w, slot).expect("z at the top of the fall");
        w.tick(pose, idle);
        let b = wizard_z(&w, slot).expect("z one fall tick later");
        openings.push(a as i32 - b as i32);
        // Ride the fall out and wait for the respawn, so the next
        // cycle starts on a live wizard on the SAME record.
        let mut n = 0;
        for k in 0..4000 {
            w.tick(pose, idle);
            n = k;
            if state_of(&w, slot) == Some(1) {
                break;
            }
        }
        falls.push(n);
        assert_eq!(
            state_of(&w, slot),
            Some(1),
            "cycle {cycle}: the rival respawns on the same record"
        );
    }

    assert_eq!(openings.len(), 2, "two death cycles ran");
    assert!(
        openings[1] > openings[0],
        "the SECOND fall resumes the FIRST fall's terminal velocity — \
         opening z drops {openings:?} over falls of {falls:?} ticks. \
         Equal openings mean the lethal branch cleared word_0x2C_44, \
         which the shipped NETHERW.EXE does not do (0x3740A: cmp/jne/\
         movb $0x2,0x45(%ebx)/xor/jmp, and nothing else)."
    );
}
