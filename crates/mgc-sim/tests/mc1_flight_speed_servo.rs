//! MC1's class-9 flight speed servo is a **SIGN TIMES TWO**, and it
//! NEVER clamps: `CARPET.EXE` 0x539DA (file 0x6C1D2) computes
//! `gap / |gap|` with an `idiv` and scales it with `lea 0x0(,%eax,2)`.
//! The port carried `+126 += (+128 - +126).clamp(-2, 2)` at seven
//! sites. See `mc1::combat::no_mc1_flight_speed_step` for the five
//! retail sites and the class-9 dispatch table that names them.
//!
//! ⚠⚠ **THIS LAW IS LATENT ON ALL SHIPPED DATA, WHICH IS WHY IT NEEDS
//! A UNIT TEST AND CANNOT TAKE A FIXTURE.** The two forms agree
//! whenever the gap is 0 or |gap| >= 2 and differ ONLY at |gap| == 1,
//! and BOTH forms step by 2 above that — so they preserve the gap's
//! PARITY and an even gap can never reach 1. A class-9 record's gap
//! is born as exactly `-carpet_speed` (the ctor writes +126 == +128
//! and only the cast adds, `world.rs::cast_projectile` :65060), and
//! MC1's carpet speed is quantised to the +-16 servo step, so every
//! gap in the corpus is even. Measured directly in RETAIL'S OWN
//! MEMORY (the `.mgcr` `state.struct_b64` pool, class @+64, +126 and
//! +128): **346,818 class-9 records across four takes, ZERO odd
//! gaps** — the gap histogram is multiples of 16 plus the servo's own
//! even walk-down (80, 78, 76 ...).
//!
//! These tests therefore CONSTRUCT the differing case directly, by
//! casting with an ODD carpet speed.

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc1::spells::SpellId;

fn synthetic_assets() -> FeatureAssets {
    let mut grid = vec![31u8; 1024];
    for y in 0..32i32 {
        for x in 0..32i32 {
            let (dx, dy) = (x - 15, y - 15);
            let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
            grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
        }
    }
    let tab: Vec<u8> = (0..24u32)
        .flat_map(|_| {
            let mut e = 0u32.to_le_bytes().to_vec();
            e.extend_from_slice(&[4, 4]);
            e
        })
        .collect();
    let mut dat = Vec::new();
    for row in 0..4 {
        dat.push(4u8);
        if row == 1 || row == 2 {
            dat.extend_from_slice(&[0x10, 7, 7, 0x10]);
        } else {
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
        }
        dat.push(0);
    }
    FeatureAssets::parse(&grid, &tab, &dat).unwrap()
}

fn armed_world() -> (World, PlayerPose) {
    let planes = Planes {
        height: vec![100; 0x10000],
        tile_type: vec![5; 0x10000],
        shading: vec![32; 0x10000],
        angle: vec![5; 0x10000],
        ceiling: Vec::new(),
    };
    let mut w = World::new(planes, &[], 1, synthetic_assets());
    w.set_dev_spells(true);
    w.grant_all_spells();
    let pose = PlayerPose::from_tiles(16.0, 40.0, 16.0, 0.0, 0.0, 0.0);
    (w, pose)
}

#[test]
fn an_odd_gap_overshoots_and_oscillates_forever() {
    // Meteor (spell 7) is a state-3 projectile -> `proj_generic_tick`,
    // and its cast inherits the carpet speed (`+126 += p.speed`). An
    // ODD pose speed gives the ctor's 384 a +126 of 385: a gap of
    // exactly -1, the only place the two forms disagree.
    let speeds = meteor_speed_series(1);
    assert_eq!(
        speeds,
        vec![383, 385, 383, 385, 383, 385, 383, 385, 383],
        "retail steps 2 * SIGN(gap): from 385 against minSpeed 384 it \
         OVERSHOOTS to 383, then oscillates about 384 forever. A \
         `.clamp(-2, 2)` would step ONE and pin at 384."
    );
    // The point of the law, stated as the observable: the servo never
    // settles, and in particular never reads the clamped fixed point.
    assert!(
        !speeds.contains(&384),
        "the servo must never reach minSpeed on an odd gap; got {speeds:?}"
    );
}

#[test]
fn an_even_gap_is_identical_under_both_forms() {
    // The latency proof, as a test: a gap of -16 (one carpet servo
    // step, the corpus's most common value by far) walks down in 2s
    // and lands exactly on 384 under EITHER form. This test passes
    // with the law on OR off -- it is here to document the scope of
    // the change, and it is deliberately not the law's pin.
    let speeds = meteor_speed_series(16);
    assert_eq!(
        speeds,
        vec![398, 396, 394, 392, 390, 388, 386, 384, 384],
        "an even gap preserves parity under a step of 2 and settles"
    );
}

/// Cast a meteor with `carpet_speed` and return `+126` for the nine
/// ticks after the shot.
fn meteor_speed_series(carpet_speed: i16) -> Vec<i16> {
    let (mut w, mut pose) = armed_world();
    pose.speed = carpet_speed;
    w.tick(
        pose,
        PlayerCommand {
            equip_left: Some(SpellId(7)),
            ..Default::default()
        },
    );
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    let mut out = Vec::new();
    for _ in 0..9 {
        w.tick(pose, PlayerCommand::default());
        let s = w.debug_creature_speeds(9, 3);
        assert_eq!(s.len(), 1, "exactly one live meteor expected, got {s:?}");
        out.push(s[0]);
    }
    out
}
