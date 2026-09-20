//! MC1 Accelerate must survive ALIGNED thrust under the faithful
//! thrust model. Retail cancels on the v_14 speed-TOUCHED flag
//! (:65144-51), and v_14 arms only when the press actually moves v_12
//! (:55766-80) — while boosted, v_12 (±160/240) sits outside the ±80
//! input clamp, so the aligned press is inert and only the RESISTING
//! press cancels. Trap: do NOT fire both cancel directions on ANY
//! thrust — that kills hold + re-cast the moment the player flies
//! forward.
//!
//! This is the `Simulation`-level companion to the World-level law
//! test (`accelerate_directions_are_mutually_exclusive`): the input
//! plumbing under test lives in lib.rs, which only these steps drive.

use mgc_formats::{Thing, ThingKind};
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::mc1::spells::SpellId;
use mgc_sim::{FlightInput, Simulation, ThrustModel};

/// Synthetic diamond-ring SEARCH.DAT + a 4x4 building row (the same
/// shape as the sim's unit-test assets — no baked data needed).
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

fn flat_world() -> World {
    let planes = Planes {
        height: vec![100; 0x10000],
        tile_type: vec![5; 0x10000],
        shading: vec![32; 0x10000],
        angle: vec![5; 0x10000],
        ceiling: Vec::new(),
    };
    // ⚖ AUTHOR THE HUMAN'S START MARKER so this fixture's pool layout
    // does not depend on `MGC_NO_MC1_MARKERLESS_HUMAN_SEAT` (round
    // 162): carpet at slot 1, the granted book above it — retail's
    // own level-start order.
    let marker = Thing {
        slot: 0,
        kind: ThingKind::Entity,
        class: 3,
        model: 4,
        x: 16,
        y: 16,
        dis_id: 0,
        swi_sz: 0,
        swi_id: 0,
        parent: 0,
        child: 0,
        par3: None,
    };
    World::new(planes, &[marker], 1, synthetic_assets())
}

#[test]
fn mc1_thrust_model_keeps_accelerate_through_forward_hold() {
    let mut sim = Simulation::with_world(flat_world());
    sim.thrust_model = ThrustModel::Mc1;
    sim.world.as_mut().unwrap().set_dev_spells(true);
    sim.world.as_mut().unwrap().grant_all_spells();
    sim.step(&FlightInput {
        equip_left: Some(SpellId(2)),
        ..Default::default()
    });
    let boost = |sim: &Simulation| sim.world.as_ref().unwrap().accel_override();

    // CAST-AND-HOLD while flying forward: full boost every tick.
    //
    // ⚖ THE FIRST HELD TICK CARRIES NO BOOST YET, because the
    // accelerate token sits ABOVE the carpet here (carpet slot 1, the
    // granted book above it). The carpet's dispatch — mover included —
    // runs at slot 1, and the token writes its pending speed register
    // later in the SAME ascending pass, so the boost first moves the
    // carpet on the NEXT tick. That is the phase `World`'s MC2 sibling
    // already documents verbatim ("when the token's walk slot is ABOVE
    // the carpet's ... the boost then first moves the carpet on the
    // NEXT tick, which is exactly the phase the recordings show").
    //
    // ⚠⚠ THIS ASSERTION PINS THE PORT, AND RETAIL MAY NOT AGREE —
    // OPEN QUESTION, DO NOT TREAT AS SETTLED (round 162).
    //
    // I went looking for the above-carpet witness and found one:
    // mc1l48 carpet 681 with an Accelerate token (model 2) at slot
    // **684**, i.e. ABOVE it. Measured with `dump-state`:
    //
    //   slot 684  f48: 0 through t=50, then 250 at t=51
    //             (armed to 251 AND ticked in the same lap — the
    //             above-carpet signature; compare the BELOW-carpet
    //             shield mc1l3 slot 34, which sits at the full 251 on
    //             its own arm tick and only decrements at arm+1)
    //   carpet 681  f126: 80 at t=50 -> **240 at t=51** -> 240 at
    //             t=52 -> 160 at t=53   (80 x3 = the boost, then the
    //             x2 decay step), and f132 -1000 at t=51
    //
    // So RETAIL's carpet speed field is already tripled ON THE ARM
    // TICK, whereas the port reports no override until the next one.
    // The likely reconciliation is that the boosted target is written
    // by the COMMAND at the carpet's own walk slot (:55825-33, the
    // bounds-tested speed step) rather than by the token, which would
    // make the boost press-tick regardless of the token's slot — but
    // that is not established, and it would make this `None` wrong.
    //
    // ⭐ IT IS AN UNGRADED LANE, which is why no take reports it:
    // replay feeds the carpet's pose as INPUT, and the port keeps the
    // carpet OUT OF POOL as a class-0 pinned hole (`dump-state --port`
    // on slot 681 reads f126/f128 as 0 against retail's 240/80 — the
    // representation, not a divergence). A dedicated dig owes a
    // verdict here.
    let hold = FlightInput {
        fire_left: true,
        thrust: 1.0,
        ..Default::default()
    };
    sim.step(&hold);
    assert_eq!(
        boost(&sim),
        None,
        "PORT BEHAVIOUR, not a retail pin: no override on the press \
         tick — see the mc1l48 t=51 measurement above, which shows \
         retail's carpet speed already boosted at the arm tick"
    );
    for n in 1..5 {
        sim.step(&hold);
        assert_eq!(
            boost(&sim),
            Some(3.0),
            "held cast + forward thrust keeps max boost (tick {n})"
        );
    }

    // Button released, still pushing forward: the decay channel runs.
    sim.step(&FlightInput {
        thrust: 1.0,
        ..Default::default()
    });
    assert_eq!(
        boost(&sim),
        Some(2.0),
        "forward thrust alone must not cancel the decay channel"
    );

    // RE-CAST during the decay: allowed, back to full.
    sim.step(&hold);
    assert_eq!(boost(&sim), Some(3.0), "re-cast re-arms the full boost");

    // The resisting input is the one cancel (manual: the down cursor).
    // The mechanism is retail's two-phase one: the brake press moves
    // the boosted target and arms the mover's v_14 latch (:55766-80),
    // and the TOKEN reads that latch on its NEXT PASS and ends the
    // burst (counter = 1 → 0, :65146-50).
    //
    // ⚖ WHETHER THAT NEXT PASS IS THIS TICK OR THE NEXT ONE IS THE
    // SLOT-RELATIVE CAST PHASE AGAIN (round 162, w162a). The latch is
    // armed at the carpet's walk slot; this fixture's token is ABOVE
    // the carpet, so the walk reaches it later in the SAME tick and
    // the burst ends immediately. The TWO-TICK observable — brake tick
    // still boosting, burst ending one pass later — is the BELOW-
    // carpet expression of the identical mechanism, and that is the
    // one the corpus witnesses (mc1l48 t=4899, tokens 232/255 under
    // carpet 681). It stays pinned in
    // `engine::world::tests::accelerate_directions_are_mutually_exclusive`,
    // whose fixture is deliberately laid out below-carpet.
    sim.step(&FlightInput {
        thrust: -1.0,
        ..Default::default()
    });
    assert_eq!(
        boost(&sim),
        None,
        "token above the carpet: the latch is read later in the SAME \
         tick, so the burst ends at once"
    );

    // And the refire gate clears: a fresh cast works next tick.
    sim.step(&hold);
    assert_eq!(boost(&sim), Some(3.0), "fresh cast after the cancel");
}

/// THE OVERRIDE DIES WITH THE WIZARD (player report 2026-09-09, the
/// enhanced hold-to-fly throttle): killed under a held Accelerate,
/// the corpse kept its x3 propulsion after touchdown and the respawned
/// wizard flew off boosted with no throttle held. The landing ends the
/// port-side effect latches (`World::player_death_clear_effects`) and
/// the enhanced mover refuses the override on a corpse.
#[test]
fn enhanced_accelerate_does_not_survive_death_or_respawn() {
    let mut sim = Simulation::with_world(flat_world());
    sim.thrust_model = ThrustModel::Enhanced;
    sim.world.as_mut().unwrap().set_dev_spells(true);
    sim.world.as_mut().unwrap().grant_all_spells();
    sim.step(&FlightInput {
        equip_left: Some(SpellId(2)),
        ..Default::default()
    });
    let boost = |sim: &Simulation| sim.world.as_ref().unwrap().accel_override();
    let hold = FlightInput {
        fire_left: true,
        thrust: 1.0,
        ..Default::default()
    };
    for _ in 0..10 {
        sim.step(&hold);
    }
    assert_eq!(boost(&sim), Some(3.0), "held cast is boosting");
    let speed = |sim: &Simulation| (sim.flyer.vx.powi(2) + sim.flyer.vz.powi(2)).sqrt();
    assert!(speed(&sim) > 1.0, "the enhanced flyer is propelled");

    // Shift+K: the MC1 key pass lands `life = -1` post-walk and the
    // next tick's regen tail flips the fall.
    sim.step(&FlightInput {
        suicide: true,
        ..hold
    });
    let mut ticks = 0;
    while !sim.world.as_ref().unwrap().player_dead() {
        sim.step(&FlightInput::default());
        ticks += 1;
        assert!(ticks < 400, "the death fall must touch down");
    }
    assert_eq!(boost(&sim), None, "touchdown ends the override");
    for _ in 0..5 {
        sim.step(&FlightInput::default());
        assert_eq!(boost(&sim), None, "no override on the corpse");
        assert!(speed(&sim) < 1e-3, "the corpse does not move");
    }

    sim.step(&FlightInput {
        respawn: true,
        ..Default::default()
    });
    assert!(!sim.world.as_ref().unwrap().player_dead(), "respawned");
    for _ in 0..10 {
        sim.step(&FlightInput::default());
        assert_eq!(boost(&sim), None, "the respawn flies unboosted");
    }
    assert!(
        speed(&sim) < 0.05,
        "no throttle held, no spell live: the carpet stays put (v={})",
        speed(&sim)
    );
}

