//! The POSE CHANNEL, tier 1 (docs/CONFORMANCE.md "The pose channel"):
//! `verify-deltas` pins the human pose, so the player's own motion
//! column is the one lane the world diff never verifies — the pinned
//! slot's pose fields are runner INPUTS, tautologically clean. This
//! module steps the faithful mover ([`flight::mc1_move`]) beside the
//! pinned world tick: flight state seeded from the recorded closure
//! at N, input recovered from the recorded flight column, stepped
//! once, and diffed against the recorded pose at N+1 — bit-exact,
//! the movers being integer ports.
//!
//! Input needs no reconstruction guesswork on MC1:
//! - the move/fire byte (`Type_160 dw_0`) is stamped by the consume
//!   loop every tick — AFTER the entity pass, for the NEXT tick's
//!   mover — and survives to the settled snapshot: the pair's boolean
//!   inputs are read straight off record N;
//! - the stick enters the mover only through the low-pass filter
//!   (`acc += (2·stick − acc)/4`), whose accumulators are recorded at
//!   both ends of the pair, so the filter is inverted per pair
//!   ([`recover_stick`]) — exact, and any solution is equivalent
//!   downstream. The map screen needs no gate: retail zeroes the
//!   command there, the accumulators visibly decay, recovery returns
//!   a centered stick.
//!
//! The world lanes are untouched: the shadow step never mutates the
//! world (ground reads the pair tick's mid-walk height snapshot —
//! the mover's own probe phase — walls the settled world), and
//! fixture signatures cannot drift because `exec_pair` is not
//! involved.

use mgc_formats::mgcr::{RetailMc1, RetailMc2};
use mgc_sim::engine::world::World;
use mgc_sim::flight::{self, Mc1Input, Mc1State, Mc2Ext};
use std::collections::BTreeMap;

/// Why a pair was not stepped. Death/respawn poses are driven by the
/// world (fall integration, castle re-seed), warps by the pad/spell
/// consumers — sim-boundary machinery the one-tick mover shadow does
/// not own; accel-domain pairs wait on the importer seeding
/// `player.speed_boost` (registered follow-up).
const GATE_DEATH: &str = "death/respawn";
const GATE_WARP: &str = "warp";
const GATE_ACCEL: &str = "accel-domain";
const GATE_STICK: &str = "stick-unrecoverable";
const GATE_WIZARD: &str = "wizard-row-missing";
const GATE_DEBUFF: &str = "debuff (web-slow/paralyze)";

/// OPT-IN (`MGC_POSE_GRADE_DEBUFF=1`): drop [`GATE_DEBUFF`] on the
/// MC2 arm and SEED the shadow mover's web-slow/paralyze ladder from
/// the recorded player block instead.
///
/// ⭐⭐⭐ THE GATE WAS NOT CONSERVATISM, IT WAS A MISSING SEED: the
/// pair built its [`Mc2Ext`] with `..Default::default()`, so
/// `move_speed`/`mobilize` were ALWAYS 0 and a debuffed step could
/// not be reproduced even in principle. Half of mc2l22's t=405..416
/// window is gated this way, and the take's whole head (t=413) lived
/// inside the gate for two sessions.
///
/// Default OFF so the census stays comparable across the wave; the
/// main session decides whether it becomes the default.
fn pose_grade_debuff() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_POSE_GRADE_DEBUFF").is_some())
}

/// KILL SWITCH (`MGC_NO_POSE_MANIF_SPEED_EXPIRY=1`) for
/// [`PoseLane::manif_speed_expiry_mc1`] — the pre-mover manifestation
/// expiry write. Set it to restore the blind step.
fn no_manif_speed_expiry() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_POSE_MANIF_SPEED_EXPIRY").is_some())
}

#[derive(Default)]
struct LaneStat {
    rows: u64,
    max_abs: i64,
    first: Option<(u64, i64, i64)>,
}

/// The channel's tally across a run, rendered after the world stats.
#[derive(Default)]
pub struct PoseLane {
    /// Fixture-grade pairs offered.
    pub pairs: u64,
    /// Pairs actually stepped (offered minus gated).
    pub stepped: u64,
    /// Stepped pairs with EVERY lane bit-exact.
    pub exact: u64,
    gates: BTreeMap<&'static str, u64>,
    lanes: BTreeMap<&'static str, LaneStat>,
    /// Pairs whose consumed move byte was exactly 48 (both fires, no
    /// move): retail short-circuits sub_46840 whole (:55759), so the
    /// held-strafe decay does not run that tick. The step emulates
    /// the freeze (see `run_pair_mc1`); the counter keeps the family
    /// visible for triage. MC1-only — MC2's sub_5F380 has no such
    /// short-circuit.
    pub dw48: u64,
    /// Pairs whose step carried a manifestation EXPIRY speed write
    /// ([`PoseLane::manif_speed_expiry_mc1`]) from a token BELOW the
    /// carpet's pool slot — the write runs before the pair's command
    /// integration.
    pub manif_expiry: u64,
    /// The same expiry seen on a token ABOVE the carpet's slot, where
    /// retail's write lands AFTER the mover and the `v_14` the token
    /// reads is the one this tick's own `sub_46840` just stamped —
    /// not the recorded one. Counted, never applied; zero across the
    /// corpus as measured (round 119).
    pub manif_expiry_above: u64,
    /// Which mover the run exercised (report label).
    arm: &'static str,
}

// The stick-filter inversion and consumed-knock reconstruction laws
// moved to the shared recovery home (mgc_formats::recover) so the
// app's `--replay` shares one implementation with the harness.
pub(crate) use mgc_formats::recover::{consumed_knock, recover_stick};

/// ⭐⭐⭐ THE KNOCK THAT IS ARMED AND SPENT INSIDE ONE TICK IS ZERO IN
/// BOTH SNAPSHOTS — and `moveBoost` is a HIT REGISTER, not a rare
/// buffet.
///
/// `sub_5EFA0`'s damage block (EF:60697-702) stamps the victim's
/// knock pair on EVERY delivered ch0 letter:
///
/// ```text
///   a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;
///   yaw_0x1E_30   = tan2(&hitter->position, &victim->position);
///   fov_0x22_34   = radix_tan(&hitter->position, &victim->position);
///   moveBoost_0x1E_30 = a1x->str_0x5E_94.dword_0x5E_94 / 10;  // 0..80
/// ```
///
/// and `AddPlayer03_00_5E010` calls `sub_5EFA0` BEFORE `sub_5D530`
/// (EF:59994), so the impulse is spent by the SAME tick's mover —
/// block 5, EF:59695-711 — which then decays it `−4` and SNAPS IT TO
/// ZERO below 4. A damage of 10..79 therefore mints a magnitude of
/// 1..7, steps the carpet by it, and leaves 0 behind: BOTH recorded
/// endpoints read `moveBoost == 0` and [`consumed_knock`] — which can
/// only reconstruct from the magnitude lane — returns `None`. The
/// shadow mover then steps a carpet retail shoved, and the whole hit
/// window grades dirty.
///
/// The recovery is exact because retail's two operands are both
/// recorded: the DIRECTION is the stamp itself, landed in
/// `yaw_0x1E_30` at N+1, and the AMOUNT is the same
/// `dword_0x5E_94` the line above subtracted from life, so the pair's
/// own life delta IS the operand (the hit arms `dword_0x18D_397 = 16`
/// in the same block, so no regen competes with it on a hit tick, and
/// reading life rather than the ch0 mailbox is proof against the
/// shield absorb rewriting the amount in place, EF:60680-93).
///
/// mc2l6-rsg t=11297..11299 is the witness: 50 damage/tick from rival
/// 378 ⇒ magnitude 5 at bearings 1064/1055/1045, worth exactly
/// (−1, +5) per tick — the three dirty pose pairs that open the
/// take's largest census family.
///
/// Kill switch `MGC_NO_POSE_SPENT_KNOCK=1` (law ON by default).
fn no_spent_knock() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_POSE_SPENT_KNOCK").is_some())
}

/// The magnitude window a within-tick spend can occupy: retail's
/// `abs(moveBoost) < 4 → 0` tail leaves nothing behind only for
/// 1..=7. Anything 8 or over survives into the N+1 snapshot and is
/// [`consumed_knock`]'s business, so this arm REFUSES it rather than
/// double-counting.
fn spent_knock_mc2(
    mag0: i16,
    mag1: i16,
    dir1: u16,
    life0: i32,
    life1: i32,
) -> Option<(u16, i16)> {
    if no_spent_knock() || mag0 != 0 || mag1 != 0 {
        return None;
    }
    let dmg = life0.checked_sub(life1)?;
    if dmg <= 0 {
        return None;
    }
    let mag = (dmg / 10).clamp(0, 80) as i16;
    if !(1..=7).contains(&mag) {
        return None;
    }
    Some((dir1 & 0x7FF, mag))
}

/// `MGC_POSE_DUEL_LIVE=1` — the kill switch for the recorded-opponent
/// duel leash: restores the pre-fix read off the (already ticked)
/// conformance world.
fn duel_pose_live() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MGC_POSE_DUEL_LIVE").is_ok())
}

/// DIG5 PROBE: `MGC_POSE_TRACE=<t0>-<t1>` window for the MC2 shadow
/// step's own input/output line.
fn pose_trace_window() -> Option<(u64, u64)> {
    static W: std::sync::OnceLock<Option<(u64, u64)>> = std::sync::OnceLock::new();
    *W.get_or_init(|| {
        let v = std::env::var("MGC_POSE_TRACE").ok()?;
        let (a, b) = v.split_once('-')?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    })
}

/// Wrapped signed distance on a 16-bit axis (x/y are wrapping).
fn wrap16(want: u16, got: u16) -> i64 {
    got.wrapping_sub(want) as i16 as i64
}

/// Wrapped signed distance on the 11-bit angle lanes.
fn wrap11(want: u16, got: u16) -> i64 {
    let d = (got.wrapping_sub(want) & 0x7FF) as i64;
    if d > 1024 { d - 2048 } else { d }
}

impl PoseLane {
    fn gate(&mut self, why: &'static str) {
        *self.gates.entry(why).or_default() += 1;
    }

    fn note(
        &mut self,
        csv: &mut Option<&mut dyn std::io::Write>,
        t: u64,
        ctx: (u16, u8, u8, f64, f64, i16),
        name: &'static str,
        want: i64,
        got: i64,
        delta: i64,
    ) -> std::io::Result<bool> {
        if want == got {
            return Ok(false);
        }
        let lane = self.lanes.entry(name).or_default();
        lane.rows += 1;
        lane.max_abs = lane.max_abs.max(delta.abs());
        lane.first.get_or_insert((t, want, got));
        if let Some(w) = csv.as_mut() {
            let (slot, c, m, x, y, z) = ctx;
            writeln!(
                w,
                "{t}\tpose\t{slot}\t{c}\t{m}\t{name}\t{want}\t{got}\t{x}\t{y}\t{z}\t"
            )?;
        }
        Ok(true)
    }

    /// Step one fixture-grade pair through the shadow mover. `world`
    /// is the imported state@N (terrain installed) and is only read.
    /// `ground_mid` is the reconstructed MID-WALK height plane
    /// (verify.rs: measured endpoints phased per cell by the pair
    /// tick's own snapshot oracle) — retail's carpet probes ground
    /// at its own walk slot, after the tick's lower-slot terraform
    /// and before the higher-slot digs, an image neither record
    /// endpoint holds (t=567 vs t=1210, the two failure families).
    /// When absent, the world's settled planes.
    /// ⭐⭐⭐ THE MANIFESTATION EXPIRY WRITES THE CARPET'S SPEED
    /// REGISTERS, AND IT RUNS AT THE TOKEN'S OWN POOL SLOT.
    ///
    /// Every class-12 machine ends with the same tail — decrement the
    /// burst counter, and on the tick it reaches zero slam the
    /// CASTER's commanded speed (`Type_160 +12`):
    ///
    /// ```text
    ///   Teleport  (10)  sub_56E50_57380 :65613-14
    ///       v6 = *(a1+48) - 1; *(a1+48) = v6;
    ///       if ( !v6 ) *(_WORD *)(*(_DWORD *)(v1 + 160) + 12) = 0;
    ///   Accelerate (2)  sub_56380_568B0 :65191-96
    ///       if ( !v7 ) { mgr->v_12 = *(v1+128); *(v1+126) = v_12; }
    ///   Accel back (21) sub_57F00_58410 :66222-29
    ///       if ( !v5 ) { mgr->v_12 = -*(v1+128); *(v1+126) = v_12; }
    /// ```
    ///
    /// and the ACCELERATE pair reaches that tail through a second
    /// door: the resisting-thrust latch `v_14` (:65145-50 / :66185-90)
    /// clamps the counter to 1 first, so a burst that was at full
    /// 251 last tick expires on THIS one. `v_14` is stamped by
    /// `sub_46840` at the CARPET's slot and cleared at the top of the
    /// next one, so a token below the carpet reads the value the
    /// recording holds at N — direct capture, no reconstruction.
    ///
    /// ⭐ The phase is the whole law. The token dispatches at its own
    /// pool slot; the human's manifestations live at slots 18..255 in
    /// `mc1l48` against the carpet's 681, so the write lands BEFORE
    /// `sub_46840`'s ±16 integration and before `sub_455D0` — the
    /// pair must slam the registers, THEN integrate, THEN move. The
    /// port's world arm has had both laws since round 47
    /// (`World::mc1_manifestation_tick`, `pending_speed_zero` /
    /// `pending_speed_base`); only this shadow lane was blind, and
    /// 35 of `mc1l48`'s 36 dirty pose pairs are it.
    ///
    /// Measured witnesses (`mc1l48`): t=29133 slot 248 model 10
    /// `f48 == 1` — `cmd_speed 80 -> 0` with an EMPTY move byte, an
    /// 80-unit step no integrator can take; t=4723 slot 232 model 2
    /// `f48 == 251` with `v_14 == 1` — `cmd_speed -32 -> 80`.
    ///
    /// Returns the state edit as a closure over `s`; `Ok` is silent.
    fn manif_speed_expiry_mc1(
        &mut self,
        pst: &RetailMc1,
        human_slot: u16,
        base_speed: i16,
        s: &mut Mc1State,
    ) {
        if no_manif_speed_expiry() {
            return;
        }
        let Some(w0) = pst.wizards.get(pst.local_player as usize) else {
            return;
        };
        let mut hit = false;
        for (slot, tok) in pst.ents.iter().enumerate() {
            // `if ( *(__int16 *)(a1 + 48) > 0 )` — the machine's own
            // head guard, plus the caster test `*(a1 + 42)`.
            if tok.class64 != 12 || tok.f42 != human_slot || tok.f48 == 0 {
                continue;
            }
            // ⭐⭐⭐ THE SELECTOR IS `+70`, NOT THE MODEL BYTE. A
            // class-12 record's handler is `str_2563D8[+70]` and the
            // machine states run `3 × spell` + 0/1/2 (round 116), so
            // a token that has ADVANCED off its manifestation state
            // still carries the spell in `+65` while dispatching
            // somewhere else entirely. `mc1l48` slot 15 is the
            // witness: model 2, `f48` frozen at 234, `+70` moved
            // 6 → 7 at t≈31520 and the accelerate machine has not
            // run since — keying on `model65` mis-fires the restore
            // on t=31601..31603 and makes three clean pairs dirty.
            let restore = match tok.f70 {
                30 => Some(0i16),         // 10 Teleport, sub_56E50_57380
                6 => Some(base_speed),    //  2 Accelerate, sub_56380_568B0
                63 => Some(-base_speed),  // 21 Accel back, sub_57F00_58410
                _ => None,
            };
            let Some(restore) = restore else { continue };
            // Does the counter reach zero on the coming tick? Either
            // it is already 1, or — Accelerate only — the resisting
            // press latched `v_14` and the machine clamps it to 1.
            let expires =
                tok.f48 == 1 || (matches!(tok.f70, 6 | 63) && w0.v14 != 0);
            if !expires {
                continue;
            }
            if slot as u16 > human_slot {
                // ABOVE the carpet: retail's write lands after the
                // mover and reads THIS tick's own `v_14`. Not
                // reconstructable from the pair; counted for triage.
                self.manif_expiry_above += 1;
                continue;
            }
            s.tgt_speed = restore;
            // Teleport writes `v_12` ONLY (:65613-14); the two
            // Accelerate tails mirror it into `+126` as well.
            if tok.f70 != 30 {
                s.act_speed = restore;
            }
            hit = true;
        }
        if hit {
            self.manif_expiry += 1;
        }
    }

    pub fn run_pair_mc1(
        &mut self,
        world: &World,
        ground_mid: Option<&[u8]>,
        pst: &RetailMc1,
        st: &RetailMc1,
        human_slot: u16,
        t: u64,
        mut csv: Option<&mut dyn std::io::Write>,
    ) -> std::io::Result<()> {
        self.arm = "mc1 mover";
        self.pairs += 1;
        let e0 = &pst.ents[human_slot as usize];
        let e1 = &st.ents[human_slot as usize];
        let (Some(w0), Some(w1)) = (
            pst.wizards.get(pst.local_player as usize),
            st.wizards.get(st.local_player as usize),
        ) else {
            self.gate(GATE_WIZARD);
            return Ok(());
        };
        // Death/respawn ticks: the pose is world-driven (fall
        // integration, castle re-seed), not mover output.
        if matches!(e0.f70, 2 | 3) || matches!(e1.f70, 2 | 3) {
            self.gate(GATE_DEATH);
            return Ok(());
        }
        // Warp ticks: pads/teleport spells move the player outside
        // the mover (max mover reach ≈ 450 units/tick; 8 tiles is
        // far beyond it).
        if wrap16(e0.x, e1.x).abs() > 2048 || wrap16(e0.y, e1.y).abs() > 2048 {
            self.gate(GATE_WARP);
            return Ok(());
        }
        // Accelerate-domain: the importer does not seed
        // `player.speed_boost`, so `accel_override` reads None on the
        // conformance world and the ±160/±240 regime cannot step
        // faithfully yet. The N+1 side also gates: the mover's own
        // integration clamps at ±80, so any bigger recorded landing
        // is the spell ARM tick — spell-domain, not mover-domain.
        if e0.f126.abs() > 80
            || w0.cmd_speed.abs() > 80
            || e1.f126.abs() > 80
            || w1.cmd_speed.abs() > 80
        {
            self.gate(GATE_ACCEL);
            return Ok(());
        }
        let (Some(sx), Some(sy)) = (
            recover_stick(w0.roll_acc as i16, w1.roll_acc as i16),
            recover_stick(w0.pitch_acc as i16, w1.pitch_acc as i16),
        ) else {
            self.gate(GATE_STICK);
            return Ok(());
        };
        // The pair's boolean inputs are the move byte recorded AT N:
        // retail's consume loop stamps dw_0 AFTER the entity pass,
        // for the NEXT tick's mover (measured on mc1l0 — mb@56's
        // strafe bit moves the strafe column across 56→57, mb@59's
        // down bit drops tgt across 59→60). Direct capture, not
        // inference.
        let mb = w0.move_bits;
        let inp = Mc1Input {
            stick_x: sx,
            stick_y: sy,
            speed_up: mb & 1 != 0,
            speed_down: mb & 2 != 0,
            strafe_left: mb & 4 != 0,
            strafe_right: mb & 8 != 0,
            // The pair shadow never steps a death fall (the domain
            // is gated) — the command handler always runs here.
            no_command: false,
            mc2_park: false,
        };
        let mut s = Mc1State {
            x: e0.x,
            y: e0.y,
            z: e0.z,
            yaw: e0.f30 & 0x7FF,
            roll_f: w0.roll_acc as i16,
            pitch_f: w0.pitch_acc as i16,
            aim_pitch: e0.f32 & 0x7FF,
            eff_pitch: w0.eff_pitch & 0x7FF,
            act_speed: e0.f126,
            tgt_speed: w0.cmd_speed,
            strafe: w0.strafe,
            tick_ctr: e0.f63,
            rand: e0.rand,
        };
        // move byte 48 exactly: retail skips sub_46840 whole, so a
        // held strafe does NOT decay this tick. The mover cannot skip
        // its decay, so pre-feed one quantum — the decay lands back
        // on the recorded value and the polar step sees the frozen
        // strafe, matching retail's arithmetic exactly.
        if mb == 48 {
            self.dw48 += 1;
            if s.strafe != 0 {
                s.strafe += 4 * s.strafe.signum();
            }
        }
        // The manifestation expiry tail runs at the TOKEN's pool
        // slot, i.e. BEFORE this carpet's command integration when
        // the token sits below it (the corpus case).
        self.manif_speed_expiry_mc1(pst, human_slot, e0.f128, &mut s);
        let knock = consumed_knock(w0.knock_mag, w0.knock_dir, w1.knock_mag, w1.knock_dir);
        let ground = |x: u16, y: u16| match ground_mid {
            Some(h) => World::ground_z_on_plane(h, x, y),
            None => world.ground_z_engine(x, y),
        };
        // The DUEL GRIP (`sub_455D0`'s lock tail, :55226-48) — the
        // lock triple `Type_160` +314/+316/+318 is recorded state and
        // `+128` is on the record, so a duelled pair steps faithfully.
        // ⭐ IT RIDES INSIDE THE MOVER: retail's block sits between
        // the knock add and `sub_45410`'s commit gate, so its step is
        // gated and z-floored with the rest of the move. Applying it
        // AFTER `mc1_move` (D12's landing, now the
        // `MGC_NO_MC1_DUEL_GRIP_EXACT=1` arm) leaves the grip's own
        // displacement unfloored — that is `mc1l48`'s residual
        // `pose.z` family in the duel window.
        // `MGC_NO_POSE_DUEL_TURN=1` restores the old blind step.
        let duelling = w0.duel_victim != 0 && std::env::var_os("MGC_NO_POSE_DUEL_TURN").is_none();
        let post_gate_grip = std::env::var_os("MGC_NO_MC1_DUEL_GRIP_EXACT").is_some();
        let grip = (duelling && !post_gate_grip).then(|| {
            let ve = &pst.ents[w0.duel_victim as usize];
            flight::Mc1DuelGrip {
                vpos: (ve.x, ve.y, ve.z),
                hold: w0.duel_hold,
                max_speed: e0.f128,
                yaw_drag: true,
            }
        });
        flight::mc1_move_duel(&mut s, &inp, None, knock, grip, &ground, &|cur, prop| {
            world.player_wall_gate_fixed(cur, prop)
        });
        if duelling && post_gate_grip {
            let ve = &pst.ents[w0.duel_victim as usize];
            let mut cand = (s.x, s.y, s.z);
            s.yaw = flight::mc1_duel_tail(
                &mut cand,
                s.yaw,
                s.aim_pitch,
                (e0.x, e0.y, e0.z),
                (ve.x, ve.y, ve.z),
                w0.duel_hold,
                e0.f128,
            );
            s.x = cand.0;
            s.y = cand.1;
            s.z = cand.2;
        }
        self.stepped += 1;
        let ctx = (
            human_slot,
            e1.class64,
            e1.model65,
            e1.x as f64 / 256.0,
            e1.y as f64 / 256.0,
            e1.z,
        );
        let mut dirty = false;
        macro_rules! lane {
            ($name:literal, $want:expr, $got:expr, $delta:expr) => {
                dirty |= self.note(&mut csv, t, ctx, $name, $want, $got, $delta)?;
            };
            ($name:literal, $want:expr, $got:expr) => {
                lane!($name, $want, $got, $got - $want);
            };
        }
        lane!("pose.x", e1.x as i64, s.x as i64, wrap16(e1.x, s.x));
        lane!("pose.y", e1.y as i64, s.y as i64, wrap16(e1.y, s.y));
        lane!("pose.z", e1.z as i64, s.z as i64);
        lane!(
            "pose.yaw",
            (e1.f30 & 0x7FF) as i64,
            s.yaw as i64,
            wrap11(e1.f30 & 0x7FF, s.yaw)
        );
        lane!(
            "pose.aim_pitch",
            (e1.f32 & 0x7FF) as i64,
            s.aim_pitch as i64,
            wrap11(e1.f32 & 0x7FF, s.aim_pitch)
        );
        lane!(
            "pose.eff_pitch",
            (w1.eff_pitch & 0x7FF) as i64,
            (s.eff_pitch & 0x7FF) as i64,
            wrap11(w1.eff_pitch & 0x7FF, s.eff_pitch & 0x7FF)
        );
        lane!("pose.act_speed", e1.f126 as i64, s.act_speed as i64);
        lane!("pose.tgt_speed", w1.cmd_speed as i64, s.tgt_speed as i64);
        lane!("pose.strafe", w1.strafe as i64, s.strafe as i64);
        lane!("pose.roll_f", w1.roll_acc as i16 as i64, s.roll_f as i64);
        lane!("pose.pitch_f", w1.pitch_acc as i16 as i64, s.pitch_f as i64);
        lane!("pose.tick_ctr", e1.f63 as i64, s.tick_ctr as i64);
        lane!("pose.rand", e1.rand as i64, s.rand as i64);
        if !dirty {
            self.exact += 1;
        }
        Ok(())
    }

    /// The MC2 twin: same shadow-step shape over [`flight::mc2_move`]
    /// with the world's cave/gate/stuck closures. Phase difference:
    /// MC2 stamps the move byte in PlayerEvents BEFORE the entity
    /// pass, so the pair consumes the byte recorded at N+1 (MC1's
    /// post-pass stamp reads at N). No flutter LCG on the MC2 path;
    /// web-slow/paralyze pairs are gated until the debuff-phase story
    /// is measured.
    pub fn run_pair_mc2(
        &mut self,
        world: &World,
        ground_mid: Option<&[u8]>,
        pst: &RetailMc2,
        st: &RetailMc2,
        human_slot: u16,
        t: u64,
        full_stop: bool,
        mut csv: Option<&mut dyn std::io::Write>,
    ) -> std::io::Result<()> {
        self.arm = "mc2 mover";
        self.pairs += 1;
        let e0 = &pst.ents[human_slot as usize];
        let e1 = &st.ents[human_slot as usize];
        let (Some(p0), Some(p1)) = (
            pst.players.get(pst.local_player as usize),
            st.players.get(st.local_player as usize),
        ) else {
            self.gate(GATE_WIZARD);
            return Ok(());
        };
        if e0.action45 != 0 || e1.action45 != 0 {
            self.gate(GATE_DEATH);
            return Ok(());
        }
        if wrap16(e0.x, e1.x).abs() > 2048 || wrap16(e0.y, e1.y).abs() > 2048 {
            self.gate(GATE_WARP);
            return Ok(());
        }
        if e0.speed.abs() > 80
            || p0.cmd_speed.abs() > 80
            || e1.speed.abs() > 80
            || p1.cmd_speed.abs() > 80
        {
            self.gate(GATE_ACCEL);
            return Ok(());
        }
        if (p0.move_speed != 0 || p1.move_speed != 0 || p0.mobilize != 0 || p1.mobilize != 0)
            && !pose_grade_debuff()
        {
            self.gate(GATE_DEBUFF);
            return Ok(());
        }
        let (Some(sx), Some(sy)) = (
            recover_stick(p0.roll_acc as i16, p1.roll_acc as i16),
            recover_stick(p0.pitch_acc as i16, p1.pitch_acc as i16),
        ) else {
            self.gate(GATE_STICK);
            return Ok(());
        };
        let mb = p1.move_bits;
        let inp = Mc1Input {
            stick_x: sx,
            stick_y: sy,
            // The speed command is the recovered cmd_speed lane (fed
            // as tgt below, the free-run law) — the key bits stay out
            // of the integrator.
            speed_up: false,
            speed_down: false,
            strafe_left: mb & 4 != 0,
            strafe_right: mb & 8 != 0,
            // The pair shadow never steps a death fall (the domain
            // is gated) — the command handler always runs here.
            no_command: false,
            // The modal park (big map / spell book): command 0 and
            // the carpet pinned across the pair — plus the FULL-STOP
            // key (BACKSPACE, PlayerAction 0x27), which bypasses the
            // position clause (a same-tick knockback moves the parked
            // carpet: mc2l0-permadeath t=2229, the pose head) — and
            // the held-speed-key guard the shared recovery carries
            // (`recover::recover_pair_mc2`'s mb & 3, the mc2l3 t=605
            // zero-crossing discriminator).
            mc2_park: p1.cmd_speed == 0
                && mb & 3 == 0
                && e1.speed == 0
                && (full_stop || (e0.x == e1.x && e0.y == e1.y)),
        };
        let mut s = Mc1State {
            x: e0.x,
            y: e0.y,
            z: e0.z,
            yaw: e0.yaw as u16 & 0x7FF,
            roll_f: p0.roll_acc as i16,
            pitch_f: p0.pitch_acc as i16,
            aim_pitch: e0.pitch as u16 & 0x7FF,
            eff_pitch: p0.eff_pitch & 0x7FF,
            act_speed: e0.speed,
            // The consumed speed COMMAND (the recovered cmd_speed
            // lane at N+1 — mouse-proportional, not the key servo),
            // un-done across a WALL DEAD-STOP
            // ([`recover::mc2_pair_cmd_speed`], the shared law).
            tgt_speed: mgc_formats::recover::mc2_pair_cmd_speed(
                p0.cmd_speed,
                p1.cmd_speed,
                mb,
                e0.x == e1.x && e0.y == e1.y
                    || (e1.x.wrapping_sub(e0.x) as i16).unsigned_abs() > 2048
                    || (e1.y.wrapping_sub(e0.y) as i16).unsigned_abs() > 2048,
                e1.speed,
            ),
            strafe: p0.strafe,
            tick_ctr: 0,
            rand: 0,
        };
        let mut ext = Mc2Ext {
            water_ctr: p0.water_ctr as u16,
            nudge_latch: p0.nudge_latch != 0,
            row: world.mc2_carpet_row(),
            // The web-slow/paralyze ladder, off the recorded player
            // block (`moveSpeed_0x14C_332` / `mobilizeCounter_0x14E_334`
            // and their counters) — the seed [`GATE_DEBUFF`] existed
            // for the lack of.
            move_speed: p0.move_speed,
            move_speed_ctr: p0.move_speed_ctr,
            mobilize: p0.mobilize,
            mobilize_ctr: p0.mobilize_ctr,
            ..Default::default()
        };
        // The magnitude lane first; the WITHIN-TICK SPEND
        // ([`spent_knock_mc2`]) when it has nothing to read.
        let knock = consumed_knock(p0.knock_mag, p0.knock_dir, p1.knock_mag, p1.knock_dir).or_else(
            || spent_knock_mc2(p0.knock_mag, p1.knock_mag, p1.knock_dir, e0.life, e1.life),
        );
        flight::mc2_move(
            &mut s,
            &mut ext,
            &inp,
            None,
            knock,
            // THE DUEL LEASH, off retail's own lock register. The
            // pair reads the lock as it stood at N — the victim's slot
            // is ABOVE the carpet's, so his stamp reaches the mover a
            // tick later — and arms only while the register still
            // stands at N+1 (a lock that broke during the tick is
            // cleared by retail, which pulled nothing that tick).
            (p0.duel_target != 0 && p1.duel_target != 0)
                .then(|| {
                    // ⭐⭐⭐ THE OPPONENT'S POSITION IS A RECORDED
                    // OPERAND, NOT A LIVE ONE. This lane's world has
                    // ALREADY been ticked by `exec_pair_mc2` — its
                    // entity table holds the PORT's own N+1 — while
                    // `sub_5DE30` runs inside the carpet's dispatch
                    // and reads the opponent at HIS walk phase: a
                    // higher slot has not moved yet (state@N), a
                    // lower one already has (state@N+1). Reading it
                    // off the live world was the pose channel's one
                    // remaining un-recovered input, and it is the
                    // whole of mc2l6-rsg's x/y/yaw ±1 drift (t=1376
                    // pull 1 read as 0, t=1378 bearing 1147 read as
                    // 1146, …).
                    if duel_pose_live() {
                        return world.mc2_duel_leash_recorded(
                            (e0.x, e0.y, e0.z),
                            p0.duel_target,
                            p0.duel_hold,
                        );
                    }
                    let src = if p0.duel_target > human_slot { pst } else { st };
                    let o = src.ents.get(p0.duel_target as usize)?;
                    Some(World::mc2_duel_leash_at(
                        (e0.x, e0.y, e0.z),
                        (o.x, o.y, o.z),
                        p0.duel_hold,
                    ))
                })
                .flatten(),
            &|x, y| match ground_mid {
                Some(h) => World::ground_z_on_plane(h, x, y),
                None => world.ground_z_engine(x, y),
            },
            &|x, y| world.player_cave_ceiling(x, y),
            &|cur, prop| world.player_mc2_gate(cur, prop),
            &|pos, latched| world.player_mc2_stuck(pos, latched),
        );
        // DIG5 PROBE (`MGC_POSE_TRACE=<t0>-<t1>`): the shadow step's
        // own inputs and outputs beside the recorded landing.
        if let Some((a, b)) = pose_trace_window() {
            if t >= a && t <= b {
                println!(
                    "POSE t={t} in: pos=({},{},{}) yaw={} aim={} eff={} act={} tgt={} strafe={} rf={} pf={} stick=({},{}) mb={} park={} knock={:?} g0={}",
                    e0.x,
                    e0.y,
                    e0.z,
                    e0.yaw as u16 & 0x7FF,
                    e0.pitch as u16 & 0x7FF,
                    p0.eff_pitch & 0x7FF,
                    e0.speed,
                    p0.cmd_speed,
                    p0.strafe,
                    p0.roll_acc as i16,
                    p0.pitch_acc as i16,
                    inp.stick_x,
                    inp.stick_y,
                    mb,
                    inp.mc2_park,
                    knock,
                    match ground_mid {
                        Some(h) => World::ground_z_on_plane(h, e0.x, e0.y),
                        None => world.ground_z_engine(e0.x, e0.y),
                    },
                );
                let opp = pst.ents.get(p0.duel_target as usize);
                println!(
                    "     duel tgt={} hold={} opp={:?} dist={:?} leash={:?}",
                    p0.duel_target,
                    p0.duel_hold,
                    opp.map(|e| (e.x, e.y, e.z)),
                    opp.map(|e| {
                        let dx = (e.x.wrapping_sub(e0.x)) as i16 as i32;
                        let dy = (e.y.wrapping_sub(e0.y)) as i16 as i32;
                        let dz = e.z as i32 - e0.z as i32;
                        (dx * dx + dy * dy + dz * dz) as f64
                    }),
                    world.mc2_duel_leash_recorded((e0.x, e0.y, e0.z), p0.duel_target, p0.duel_hold),
                );
                if let Some(e) = opp {
                    let dx = (e.x.wrapping_sub(e0.x)) as i16 as i32;
                    let dy = (e.y.wrapping_sub(e0.y)) as i16 as i32;
                    let dz = e.z as i32 - e0.z as i32;
                    let r2 = (dx * dx + dy * dy + dz * dz) as f64;
                    let d = r2.sqrt().floor() as i32;
                    println!(
                        "     rec  bearing={} dist={} hold={} pull={}",
                        mgc_sim::flight::angle_between(e0.x, e0.y, e.x, e.y),
                        d,
                        p0.duel_hold,
                        ((d - p0.duel_hold) / 8).clamp(-120, 120),
                    );
                }
                println!(
                    "     got  pos=({},{},{}) yaw={} aim={} eff={} act={} | want pos=({},{},{}) yaw={} aim={} eff={} act={}",
                    s.x,
                    s.y,
                    s.z,
                    s.yaw,
                    s.aim_pitch,
                    s.eff_pitch & 0x7FF,
                    s.act_speed,
                    e1.x,
                    e1.y,
                    e1.z,
                    e1.yaw as u16 & 0x7FF,
                    e1.pitch as u16 & 0x7FF,
                    p1.eff_pitch & 0x7FF,
                    e1.speed,
                );
            }
        }
        self.stepped += 1;
        let ctx = (
            human_slot,
            e1.class3f,
            e1.model40,
            e1.x as f64 / 256.0,
            e1.y as f64 / 256.0,
            e1.z,
        );
        let mut dirty = false;
        macro_rules! lane {
            ($name:literal, $want:expr, $got:expr, $delta:expr) => {
                dirty |= self.note(&mut csv, t, ctx, $name, $want, $got, $delta)?;
            };
            ($name:literal, $want:expr, $got:expr) => {
                lane!($name, $want, $got, $got - $want);
            };
        }
        lane!("pose.x", e1.x as i64, s.x as i64, wrap16(e1.x, s.x));
        lane!("pose.y", e1.y as i64, s.y as i64, wrap16(e1.y, s.y));
        lane!("pose.z", e1.z as i64, s.z as i64);
        lane!(
            "pose.yaw",
            (e1.yaw as u16 & 0x7FF) as i64,
            s.yaw as i64,
            wrap11(e1.yaw as u16 & 0x7FF, s.yaw)
        );
        lane!(
            "pose.aim_pitch",
            (e1.pitch as u16 & 0x7FF) as i64,
            s.aim_pitch as i64,
            wrap11(e1.pitch as u16 & 0x7FF, s.aim_pitch)
        );
        lane!(
            "pose.eff_pitch",
            (p1.eff_pitch & 0x7FF) as i64,
            (s.eff_pitch & 0x7FF) as i64,
            wrap11(p1.eff_pitch & 0x7FF, s.eff_pitch & 0x7FF)
        );
        lane!("pose.act_speed", e1.speed as i64, s.act_speed as i64);
        lane!("pose.tgt_speed", p1.cmd_speed as i64, s.tgt_speed as i64);
        lane!("pose.strafe", p1.strafe as i64, s.strafe as i64);
        lane!("pose.roll_f", p1.roll_acc as i16 as i64, s.roll_f as i64);
        lane!("pose.pitch_f", p1.pitch_acc as i16 as i64, s.pitch_f as i64);
        if !dirty {
            self.exact += 1;
        }
        Ok(())
    }

    /// The report block (empty string when the channel never ran).
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        if self.pairs == 0 {
            return out;
        }
        let pct = |n: u64, d: u64| {
            if d == 0 {
                0.0
            } else {
                n as f64 * 100.0 / d as f64
            }
        };
        let _ = writeln!(
            out,
            "  POSE CHANNEL ({}): {} stepped / {} pairs — {} bit-exact ({:.1}% of stepped)",
            self.arm,
            self.stepped,
            self.pairs,
            self.exact,
            pct(self.exact, self.stepped),
        );
        if !self.gates.is_empty() {
            let gates: Vec<String> = self.gates.iter().map(|(k, v)| format!("{k} {v}")).collect();
            let _ = writeln!(out, "    gated: {}", gates.join(", "));
        }
        if self.manif_expiry > 0 || self.manif_expiry_above > 0 {
            let _ = writeln!(
                out,
                "    manifestation speed-expiry pairs: {} (above-carpet, unmodelled: {})",
                self.manif_expiry, self.manif_expiry_above
            );
        }
        if self.dw48 > 0 {
            let _ = writeln!(
                out,
                "    move-byte-48 pairs (sub_46840 skip): {}",
                self.dw48
            );
        }
        for (name, l) in &self.lanes {
            let (t, want, got) = l.first.unwrap_or_default();
            let _ = writeln!(
                out,
                "    {name}: {} rows, max |d| {}, first t={t} want {want} got {got}",
                l.rows, l.max_abs
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pool holding one human carpet at 681 and one class-12
    /// machine at `slot`, owned by him.
    fn manif_world(slot: usize, f70: u8, f48: u16, v14: i16) -> RetailMc1 {
        let mut st = RetailMc1 {
            local_player: 0,
            wizards: vec![mgc_formats::mgcr::RetailWizardMc1::default()],
            ents: vec![mgc_formats::mgcr::RetailEntMc1::default(); 1000],
            ..RetailMc1::default()
        };
        st.wizards[0].v14 = v14;
        st.ents[681].class64 = 3;
        st.ents[681].f128 = 80; // the base commanded max (:55343)
        let t = &mut st.ents[slot];
        t.class64 = 12;
        t.model65 = f70 / 3;
        t.f70 = f70;
        t.f42 = 681;
        t.f48 = f48;
        t.f50 = 251;
        st
    }

    fn manif_step(st: &RetailMc1, tgt: i16, act: i16) -> (i16, i16) {
        let mut lane = PoseLane::default();
        let mut s = Mc1State {
            tgt_speed: tgt,
            act_speed: act,
            ..Mc1State::default()
        };
        lane.manif_speed_expiry_mc1(st, 681, st.ents[681].f128, &mut s);
        (s.tgt_speed, s.act_speed)
    }

    /// ⭐ THE MANIFESTATION EXPIRY TAIL, from the shipped machines:
    /// Teleport zeroes the COMMANDED speed only (`sub_56E50_57380`
    /// :65613-14), the two Accelerate tails slam BOTH registers back
    /// to ±`+128` (`sub_56380_568B0` :65191-96 / `sub_57F00_58410`
    /// :66222-29), and the resisting-thrust latch `v_14` is a second
    /// door into the Accelerate tail (:65145-50) that Teleport does
    /// not have.
    ///
    /// Witnesses, all `mc1l48`: t=29133 slot 248 `f70 = 30`,
    /// `f48 == 1` — `cmd_speed 80 -> 0` on an EMPTY move byte;
    /// t=4723 slot 232 `f70 = 6`, `f48 == 251`, `v_14 == 1` —
    /// `cmd_speed -32 -> 80`.
    #[test]
    fn the_manifestation_expiry_writes_the_carpet_speed_registers() {
        // Teleport (10), last burst tick: v_12 := 0, +126 untouched.
        assert_eq!(manif_step(&manif_world(213, 30, 1, 0), -80, -80), (0, -80));
        // ... and NOT while the burst still has ticks to run.
        assert_eq!(
            manif_step(&manif_world(213, 30, 5, 0), -80, -80),
            (-80, -80)
        );
        // ... nor on the resisting-thrust latch: Teleport's machine
        // has no `v_14` door.
        assert_eq!(
            manif_step(&manif_world(213, 30, 5, 1), -80, -80),
            (-80, -80)
        );
        // Accelerate forward (2): both registers back to +128.
        assert_eq!(manif_step(&manif_world(232, 6, 1, 0), -32, -32), (80, 80));
        // ... reached from a FULL burst by the `v_14` latch alone.
        assert_eq!(manif_step(&manif_world(232, 6, 251, 1), -32, -32), (80, 80));
        // ... and a full burst with no latch is the SUSTAIN arm, not
        // the tail (the ±240 write; `GATE_ACCEL` owns those pairs).
        assert_eq!(
            manif_step(&manif_world(232, 6, 251, 0), -32, -32),
            (-32, -32)
        );
        // Accelerate backwards (21): −`+128`.
        assert_eq!(manif_step(&manif_world(255, 63, 1, 0), 32, 32), (-80, -80));
    }

    /// ⭐⭐⭐ THE SELECTOR IS `+70`, NOT THE MODEL BYTE — a machine
    /// that has advanced off its manifestation state still carries
    /// the spell in `+65`. `mc1l48` slot 15: model 2, `f48` frozen at
    /// 234, `+70` moved 6 → 7 at t≈31520 and the Accelerate machine
    /// has not run since. Keying on `model65` restores ±80 over
    /// t=31601..31603 and turns three bit-exact pairs dirty.
    #[test]
    fn a_machine_off_its_manifestation_state_writes_nothing() {
        let mut st = manif_world(15, 6, 234, 1);
        assert_eq!(manif_step(&st, 16, 16), (80, 80), "state 6 IS the machine");
        st.ents[15].f70 = 7; // the successor state; model65 still 2
        assert_eq!(manif_step(&st, 16, 16), (16, 16));
    }

    /// The machine only speaks for ITS OWN caster, and only from
    /// below the carpet's pool slot — above it retail's write lands
    /// after the mover and reads the `v_14` this tick's own
    /// `sub_46840` just stamped, which the pair cannot reconstruct.
    #[test]
    fn the_expiry_is_scoped_to_the_caster_and_to_the_slot_order() {
        let mut st = manif_world(213, 30, 1, 0);
        st.ents[213].f42 = 706; // a rival's token
        assert_eq!(manif_step(&st, -80, -80), (-80, -80));

        let st = manif_world(900, 30, 1, 0); // above the carpet
        let mut lane = PoseLane::default();
        let mut s = Mc1State {
            tgt_speed: -80,
            ..Mc1State::default()
        };
        lane.manif_speed_expiry_mc1(&st, 681, 80, &mut s);
        assert_eq!(s.tgt_speed, -80);
        assert_eq!(lane.manif_expiry_above, 1);
        assert_eq!(lane.manif_expiry, 0);
    }

    /// Every (acc, stick) transition the filter can produce must be
    /// invertible to a stick that reproduces the same landing value.
    #[test]
    fn stick_recovery_inverts_the_filter() {
        for acc in (-600i32..=600).step_by(7) {
            for stick in -128i32..=127 {
                let a = acc as i16;
                let next = a + ((2 * stick - acc) / 4) as i16;
                let rec = recover_stick(a, next)
                    .unwrap_or_else(|| panic!("unrecoverable acc {acc} stick {stick}"));
                let landed = a + ((2 * rec as i32 - acc) / 4) as i16;
                assert_eq!(landed, next, "acc {acc} stick {stick} rec {rec}");
            }
        }
    }

    /// A transition no command-range stick can explain (a respawn
    /// wipe) is refused, not approximated.
    #[test]
    fn impossible_transition_is_refused() {
        assert_eq!(recover_stick(0, 300), None);
        assert_eq!(recover_stick(500, 0), None);
    }

    /// ⭐⭐⭐ THE WITHIN-TICK SPEND. `sub_5EFA0` mints
    /// `moveBoost = damage/10` and `sub_5D530` spends it the same
    /// tick, decaying −4 and snapping to 0 below 4 — so a 10..79
    /// damage leaves BOTH snapshots at 0 and only the life delta and
    /// the landed `yaw_0x1E_30` survive to name it.
    ///
    /// mc2l6-rsg t=11297: 50 damage from rival 378, bearing 1064 ⇒
    /// magnitude 5, the exact impulse the shadow mover was missing.
    #[test]
    fn a_knock_spent_inside_its_own_tick_is_recovered_from_the_life_delta() {
        assert_eq!(
            consumed_knock(0, 1301, 0, 1064),
            None,
            "the magnitude lane cannot see a knock that decayed to 0"
        );
        assert_eq!(spent_knock_mc2(0, 0, 1064, 10_000, 9_950), Some((1064, 5)));
        // The two neighbours of the same window.
        assert_eq!(spent_knock_mc2(0, 0, 1055, 9_950, 9_900), Some((1055, 5)));
        assert_eq!(spent_knock_mc2(0, 0, 1045, 9_900, 9_850), Some((1045, 5)));
        // No life drop = no letter = no impulse (the tick after the
        // rival stops hitting: mail amount lingers, life holds).
        assert_eq!(spent_knock_mc2(0, 0, 1045, 9_850, 9_850), None);
        // A HEAL is not a hit.
        assert_eq!(spent_knock_mc2(0, 0, 1045, 9_850, 9_900), None);
        // Under 10 damage mints magnitude 0 — retail steps nothing.
        assert_eq!(spent_knock_mc2(0, 0, 1045, 9_850, 9_841), None);
        // 80 damage mints 8, which SURVIVES the decay into the N+1
        // snapshot: `consumed_knock`'s business, refused here.
        assert_eq!(spent_knock_mc2(0, 0, 1045, 9_850, 9_770), None);
        // A residue in either snapshot means the magnitude lane can
        // read it; this arm stands down.
        assert_eq!(spent_knock_mc2(0, 12, 1045, 10_000, 9_950), None);
        assert_eq!(spent_knock_mc2(9, 0, 1045, 10_000, 9_950), None);
    }

    /// The angle-lane wrap helpers measure the short way around.
    #[test]
    fn wrapped_deltas() {
        assert_eq!(wrap16(0xFFF0, 0x0010), 0x20);
        assert_eq!(wrap11(2040, 8), 16);
        assert_eq!(wrap11(8, 2040), -16);
    }
}
