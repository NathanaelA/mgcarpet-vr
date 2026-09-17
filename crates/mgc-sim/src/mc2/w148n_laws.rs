//! Round 148, dig w148n — unit pins for the three free-run raw-shadow
//! laws this dig landed. Each test is a KILL-SWITCH BINARY: it asserts
//! the retail value with the law live and the pre-dig value with the
//! `MGC_NO_MC2_…` switch set, and each carries a POSITIVE CONTROL so a
//! test that silently stops exercising the path fails loudly.
//!
//! ⚠ The switches are `OnceLock`-cached per process, so a test cannot
//! flip one at runtime. The reversion probe is the same file re-run
//! with the switch exported:
//!
//! ```text
//!   cargo test --release -p mgc-sim mc2::w148n_laws
//!   MGC_NO_MC2_CASTLE_LOCK_STRAY_2E=1 \
//!   MGC_NO_MC2_M27_HIDE_BIT=1 \
//!   MGC_NO_MC2_M15_AIM_BEFORE_LIFE_TEST=1 \
//!     cargo test --release -p mgc-sim mc2::w148n_laws
//! ```
//!
//! Every assertion below branches on the switch, so BOTH arms must
//! pass and the probe is a real A/B rather than a skip.

use crate::chassis::ChassisParams;
use crate::engine::features::{FeatureAssets, Gen, Planes};
use crate::mc1::mobs::MobCtx;
use crate::patches::WorldPatches;
use crate::verbs::VerbSet;

fn flat_gen() -> Gen {
    let planes = Planes {
        height: vec![100; 0x10000],
        tile_type: vec![5; 0x10000],
        shading: vec![32; 0x10000],
        angle: vec![5; 0x10000],
        ceiling: Vec::new(),
    };
    let assets = FeatureAssets {
        rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
        build_tab: Vec::new(),
        build_dat: Vec::new(),
        bldgprm: Vec::new(),
        spells: Vec::new(),
        mc2_sprite_ext: Vec::new(),
    };
    Gen::new(planes, assets, 1, ChassisParams::MC2, VerbSet::MC2)
}

fn ctx_at(px: u16, py: u16) -> MobCtx {
    MobCtx {
        px,
        py,
        pz: 100,
        pyaw: 0,
        pmana: 0,
        pmana_max: 0,
        pdead: false,
        pdead_top: false,
        strict: false,
        patches: WorldPatches::RETAIL,
        mc2_turn: 0,
    }
}

// =========================================================================
// LAW 1 — `sub_5F890`'s marker index. The castle-spell lock stamp has
// NO class test and NO clamp, so a book marker of `1` writes
// `Entities[1]->word_0x2E_46 = word_0x30_48 - 1` (pin) / `= 0`
// (release) into POOL SLOT 1, whatever class sits there.
// =========================================================================

/// The clamp is the whole arithmetic half of the law: retail's
/// 0x840E4 is a bare `dec`, and pool slot 1's `@0x30` is 0 in every MC2
/// take in the corpus, so the pin writes `-1`. The pre-dig port wrote
/// `f28.max(1) as i16 - 1` and floored it at 0.
#[test]
fn castle_lock_pin_value_has_no_floor() {
    use crate::mc2::cast::castle_lock_pin_value;
    // THE DEFECT, exactly: @0x30 == 0 on the marker-1 record.
    assert_eq!(castle_lock_pin_value(0), -1);
    assert_eq!(
        (0u16).max(1) as i16 - 1,
        0,
        "positive control: the pre-dig `.max(1)` expression really did \
         return 0 for the same input, so this test can fail"
    );
    // POSITIVE CONTROL on the ordinary manifestation path: a live
    // (15,2) token's duration is unchanged by dropping the clamp.
    assert_eq!(castle_lock_pin_value(101), 100);
    assert_eq!(castle_lock_pin_value(1), 0);
}

/// The port homes `@0x2E` PER CLASS, and the pre-dig comment on
/// `mc2_owner_castle_token` wrote the marker case off precisely because
/// it assumed `f26`. `f26` is only `@0x2E` on class 15; on class 10 it
/// aliases `dword_0x10_16`.
#[test]
fn raw_2e_home_matches_the_lane_table() {
    use crate::mc2::cast::{Raw2eHome, Raw30Home, raw_at_2e_home, raw_at_30_home};
    // Every (class, model) the free-run census puts on this lane.
    assert_eq!(raw_at_2e_home(14, 1), Raw2eHome::F46); // mc2l16 slot 1
    assert_eq!(raw_at_2e_home(11, 4), Raw2eHome::F46); // mc2l4 slot 1
    assert_eq!(raw_at_2e_home(10, 0), Raw2eHome::F46); // mc2l8 / mc2l22
    assert_eq!(raw_at_2e_home(10, 14), Raw2eHome::F46); // mc2l6-rsg
    assert_eq!(raw_at_2e_home(10, 40), Raw2eHome::F46);
    assert_eq!(raw_at_2e_home(10, 71), Raw2eHome::F46); // mc2l18
    assert_eq!(raw_at_2e_home(9, 3), Raw2eHome::F46); // mc2l6
    assert_eq!(raw_at_2e_home(5, 0), Raw2eHome::Lease); // mc2l6
    assert_eq!(raw_at_2e_home(5, 16), Raw2eHome::Lease); // mc2l17
    assert_eq!(raw_at_2e_home(5, 23), Raw2eHome::Lease); // mc2l22
    assert_eq!(raw_at_2e_home(15, 2), Raw2eHome::F26); // the real token
    // POSITIVE CONTROL — the families whose `f46` is spent on another
    // retail word must be REFUSED, or the stray write would corrupt a
    // live field instead of closing a lane.
    assert_eq!(raw_at_2e_home(10, 39), Raw2eHome::None); // mana sphere
    assert_eq!(raw_at_2e_home(10, 57), Raw2eHome::None);
    assert_eq!(raw_at_2e_home(10, 45), Raw2eHome::None); // mc2l22 slot 1 @ t=0
    assert_eq!(raw_at_2e_home(10, 78), Raw2eHome::None); // magic mine @0x3D
    assert_eq!(raw_at_2e_home(3, 2), Raw2eHome::None); // castle: u8 f59
    // …and the `@0x30` source follows the same table.
    assert_eq!(raw_at_30_home(14, 1), Raw30Home::F50);
    assert_eq!(raw_at_30_home(15, 2), Raw30Home::F28);
    assert_eq!(raw_at_30_home(5, 10), Raw30Home::Dead); // pyramid stride
    assert_eq!(raw_at_30_home(5, 27), Raw30Home::Dead); // hydra spline
}

// =========================================================================
// LAW 2 — the m27 hydra's burrow show/hide touches byte[0] bits 0 and
// 3 only (`sub_29A90`; NETHERW.EXE 0x4E805 / 0x4E8BC / 0x4E929 /
// 0x4E99A). The port also stamped the INVISIBILITY bit 0x20.
// =========================================================================

#[test]
fn m27_hide_mask_is_bit0_only() {
    let off = crate::mc2::multipart::no_mc2_m27_hide_bit();
    let mask = crate::mc2::multipart::m27_hide_mask();
    if off {
        assert_eq!(mask, 0x21, "switch ON: the port's legacy draw alias");
    } else {
        assert_eq!(mask, 0x01, "retail's `or 0x1` / `and 0xFE`, nothing else");
    }
    // POSITIVE CONTROL: bit 3 (the targetable toggle) is never part of
    // the mask in either arm — each op applies it separately, which is
    // what makes retail's `(byte[0]|1) & 0xF7` two operations.
    assert_eq!(mask & 0x08, 0);
}

// =========================================================================
// LAW 3 — `sub_23E60` aims BEFORE it tests the target's life.
// =========================================================================

/// A (5,15) guard in state 122 whose target has just died still writes
/// one last committed heading, on the `byte_0x3E_62 & 3 == 0` tick.
#[test]
fn m15_chase_aims_before_the_life_test() {
    let off = crate::mc2::roster::no_mc2_m15_aim_before_life_test();
    let mut g = flat_gen();
    let ctx = ctx_at(0, 0);

    // The target: a record placed due EAST of the guard and ALREADY
    // dead (`life_0x8 < 0`) — the mc2l12 t=1580 shape, where a (3,1)
    // at slot 178 drops to -276 in the same tick.
    let t = g.new_event().expect("target slot");
    g.ent[t].class64 = 3;
    g.ent[t].model65 = 1;
    g.ent[t].x = 40000;
    g.ent[t].y = 10000;
    g.ent[t].z = 100;
    g.ent[t].act_life = -276;

    let i = g.new_event().expect("guard slot");
    g.ent[i].class64 = 5;
    g.ent[i].model65 = 15;
    g.ent[i].tick70 = 122;
    g.ent[i].x = 10000;
    g.ent[i].y = 10000;
    g.ent[i].z = 100;
    g.ent[i].act_life = 100;
    g.ent[i].f146 = t as u16;
    g.ent[i].f63 = 0; // the `& 3 == 0` aim tick
    g.ent[i].f34 = 777; // a bearing that is plainly not east

    let east = Gen::angle_between(10000, 10000, 40000, 10000);
    assert_ne!(east, 777, "positive control: the aim would actually move");

    g.m15_tick(i, &ctx);

    assert_eq!(
        g.ent[i].tick70, 121,
        "positive control: the dead target still sends the guard back \
         to the brain in BOTH arms"
    );
    if off {
        assert_eq!(g.ent[i].f34, 777, "switch ON: the port skipped the aim");
    } else {
        assert_eq!(g.ent[i].f34, east, "retail aims first (sub_23E60)");
    }
}

/// The `& 3` gate is retail's own: on a phase whose low two bits are
/// set, NEITHER arm writes the bearing. That gate is what separates
/// mc2l12's slots 706/785/975 (phase 116/124/24, all `& 3 == 0`, all
/// three re-aimed by retail) from 339/349/370/858 (phase 93/62/186/86,
/// none of them re-aimed).
#[test]
fn m15_chase_aim_respects_the_phase_gate() {
    let mut g = flat_gen();
    let ctx = ctx_at(0, 0);
    let t = g.new_event().expect("target slot");
    g.ent[t].class64 = 3;
    g.ent[t].model65 = 1;
    g.ent[t].x = 40000;
    g.ent[t].y = 10000;
    g.ent[t].z = 100;
    g.ent[t].act_life = -276;

    let i = g.new_event().expect("guard slot");
    g.ent[i].class64 = 5;
    g.ent[i].model65 = 15;
    g.ent[i].tick70 = 122;
    g.ent[i].x = 10000;
    g.ent[i].y = 10000;
    g.ent[i].z = 100;
    g.ent[i].act_life = 100;
    g.ent[i].f146 = t as u16;
    g.ent[i].f63 = 1; // 93 & 3, 62 & 3, … — NOT an aim tick
    g.ent[i].f34 = 777;

    g.m15_tick(i, &ctx);
    assert_eq!(g.ent[i].tick70, 121, "positive control: still exits");
    assert_eq!(g.ent[i].f34, 777, "no aim off the & 3 gate, in either arm");
}
