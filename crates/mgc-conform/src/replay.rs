//! `replay` — PURE INPUT REPLAY (docs/RECORDING.md "Consumers",
//! docs/CONFORMANCE.md "The replay verifier"): seed the world ONCE
//! from the recording's first closure, then free-run, feeding only
//! the per-tick input recovered from the recording — the mover steps
//! OUTSIDE the world tick exactly like the app (`Simulation::step`'s
//! faithful path, integer-only), `World::tick(pose, cmd)` after.
//! Nothing is pinned, nothing re-imports, and divergence is REPORTED
//! at every recorded boundary, never corrected — the instrument's
//! whole point is where and how the free run leaves the recording.
//!
//! A gap in the recording re-anchors a fresh SEGMENT (a capture
//! artifact, not a resync); within a segment the only recording data
//! that reaches the sim is the input stream itself.
//!
//! `--pose-only` is the tier-2 chain: the FLIGHT state chains while
//! the world context is re-imported per pair (retail's own world at
//! N, the pose channel's shape minus the reseed) — it isolates the
//! mover + input-recovery chain from world fidelity. Gated pairs
//! (death/warp/accel/debuff, world-driven poses a bare mover cannot
//! own) re-seed the chain silently and are counted, not graded.
//!
//! Input recovery is the pose channel's (docs/CONFORMANCE.md):
//! move/fire byte `Type_160/164 dw_0` (MC1 stamped post-pass — read
//! at N; MC2 in PlayerEvents — read at N+1), stick by inverting the
//! low-pass filter across the recorded accumulator pair, MC2 casts by
//! the press-latch alignment law, respawn by the SPACE lane. MC1
//! casts read dw_0 bits 0x10/0x20 — the CONSUMED fire levels, same
//! stamp as the move bits, so the edge needs no `--input-delay`
//! model.

use crate::Args;
use crate::verify::{
    PairDiff, PairPose, append_hand_diffs, append_sprite_diffs, capture_clean, compare, exec_pair,
    fire_bits_mc1, measured_planes,
};
use crate::verify_mc2::{capture_clean_mc2, compare_mc2_gated, torn_slots};
use mgc_formats::mgcr::{
    ObsMc1, ObsMc2, Recording, RetailEntMc1, RetailEntMc2, RetailMc1, RetailMc2, RetailPlayerMc2,
    RetailWizardMc1, decode_retail_mc1, decode_retail_mc2,
};
use mgc_formats::recover::{self, consumed_knock};
use mgc_sim::engine::world::conformance::{
    PinnedMc1, PinnedMc2, emit_pose_window, integer_pose, mc1_state_from_retail,
    mc2_state_from_retail, pose_all_mc1, pose_all_mc2, pose_lanes_mc1, pose_lanes_mc2, pose_window,
};
use mgc_sim::engine::world::{FlightDrive, Mc2Drive, PlayerCommand, PlayerPose, World};
use mgc_sim::flight::{self, Mc1Input, Mc1State, Mc2Ext};
use mgc_sim::mc1::spells::SpellId;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(crate) fn replay(path: &std::path::Path, args: &Args) -> i32 {
    // The horizon shortcut answers "where does the free run first
    // part?" and nothing else. `--segmented` exists precisely to keep
    // measuring past that point, and `--pose-only` grades on a
    // different lane, so both combinations are refused rather than
    // silently reporting a truncated sweep as a whole one.
    if args.stop_at_div && (args.segmented || args.pose_only) {
        eprintln!(
            "replay: --stop-at-divergence is the horizon query and cannot be combined \
             with --segmented or --pose-only"
        );
        return 2;
    }
    let family = match Recording::open(path).and_then(|r| r.header.family()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let res = match family {
        mgc_formats::mgcr::Family::Mc1 => run_mc1(path, args, None),
        mgc_formats::mgcr::Family::Mc2 => run_mc2(path, args, None),
    };
    match res {
        Ok(clean) => {
            if clean {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            2
        }
    }
}

// ------------------------------------------------------------ the chain

/// ⭐⭐⭐ THE KNOWN-DEVIATION ROSTER IN `replay` (player-ruled
/// 2026-09-04, and owed since round 99): decide whether one divergent
/// boundary is EXCUSED — every row on it matched by a rule whose
/// `status` is `deviation`, i.e. retail behaviour the register says
/// the port deliberately does not reproduce.
///
/// The rule is deliberately ALL-OR-NOTHING, and that is what makes a
/// coarse rule safe: `mc2l22 (5,27) heading` can excuse the hydra's
/// uninitialised-parity family without ever hiding a compound break,
/// because one unexplained row on the same boundary demotes the whole
/// thing back to `SegOpen::Deviation`.
///
/// ⭐⭐⭐ THE POSE CHANNEL *IS* SCOPEABLE — THE PLAYER IS JUST ANOTHER
/// POOL RECORD (round 103, re-measured round 119 dig W119-8). This
/// function used to refuse every pose row outright on the stated
/// grounds that "its rows carry no (class, model, slot) for a rule to
/// scope against". That is factually wrong: the human is a live pool
/// record — class 3, model 0, at `human_slot`, with a position — and
/// [`PoseLane::note`] already writes exactly that tuple into its own
/// CSV. The refusal cost mc1l48 its horizon: at t=12848 the carpet is
/// at tile (143.95, 61.57), standing on cells (143,61)/(144,61) of the
/// REGISTERED `mc1l48-reload-painter-dat-damage-top-wall` wound
/// (`MGC_CELL_TRACE` from the t=12424 reload: port height 16, retail
/// 24, drifting from t=12435 and healing only at the re-anchor), so
/// `pose.z` and `pose.eff_pitch` — the two ground-derived lanes —
/// part by 2 and 5 while the one-tick pair lane, which reads the
/// MEASURED plane, is bit-exact. A registered deviation the excuse
/// machinery structurally could not see.
///
/// A pose row is therefore classified like any other, with the
/// human's own `(class, model, slot, pos)`, under its own
/// [`RowKind::Pose`] so no existing rule can reach one by accident: a
/// pose rule must say `"kind": "pose"` and name `pose.*` fields out
/// loud. `MGC_NO_POSE_ROSTER_EXCUSE=1` restores the old blanket
/// refusal.
///
/// Two lanes can still NEVER be excused, whatever the roster says:
///   * an RNG boundary — a parted LCG stream is not a per-row fact
///     about one entity, it is the whole world's future;
///   * `status: capture` and `status: open` rows. Capture rows are the
///     recording's limitation rather than the port's, but they are not
///     a RULING; `open` rows are leads awaiting a fix round. Only
///     `deviation` is a thing the player has decided stays.
///
/// Hits fold into [`RStats::roster_hits`] as (rows, boundaries) so the
/// report can print the same visible per-rule table `verify-deltas`
/// does — a rule that starts excusing an order of magnitude more is a
/// signal, never a silent mask.
fn no_pose_roster_excuse() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_POSE_ROSTER_EXCUSE").is_some())
}

fn roster_excuse(
    stats: &mut RStats,
    roster: Option<&crate::roster::Roster>,
    take: &str,
    t: u64,
    pose: &[(&'static str, i64, i64)],
    pd: &PairDiff,
    human_slot: u16,
    ctx: &dyn Fn(u16) -> Option<(u8, u8, f64, f64)>,
) -> bool {
    use crate::roster::{RowCtx, RowKind, RuleStatus, Tag};
    let Some(r) = roster else { return false };
    if pd.rng_want != pd.rng_got {
        return false;
    }
    let mut idxs: Vec<usize> = Vec::new();
    // THE POSE PASS. All-or-nothing like the world pass: one pose row
    // that no `deviation` rule claims demotes the whole boundary.
    if !pose.is_empty() {
        if no_pose_roster_excuse() {
            return false;
        }
        // No context for the carpet ⇒ nothing to scope against ⇒ the
        // old refusal, which is the conservative answer.
        let Some((class, model, x, y)) = ctx(human_slot) else {
            return false;
        };
        for (name, ..) in pose {
            let row = RowCtx {
                kind: RowKind::Pose,
                slot: Some(human_slot),
                class,
                model,
                field: Some(name),
                pos: Some((x, y)),
            };
            match r.classify(take, t.saturating_sub(1), &row) {
                Some(i) if r.rules[i].status == RuleStatus::Deviation => idxs.push(i),
                _ => return false,
            }
        }
    }
    // The boundary t grades the pair (t-1 → t) — rules scope on the
    // pair tick, the same key `verify-deltas` classifies under.
    let tags = crate::verify::classify_pair(Some(r), take, t.saturating_sub(1), pd, ctx);
    for tag in tags
        .missing
        .iter()
        .chain(tags.extra.iter())
        .chain(tags.fields.iter())
    {
        match tag {
            Tag::Rule(i) if r.rules[*i].status == RuleStatus::Deviation => idxs.push(*i),
            _ => return false,
        }
    }
    if idxs.is_empty() {
        return false;
    }
    let mut counted = std::collections::BTreeSet::new();
    for i in idxs {
        let e = stats
            .roster_hits
            .entry(r.rules[i].id.clone())
            .or_insert((0, 0));
        e.0 += 1;
        if counted.insert(i) {
            e.1 += 1;
        }
    }
    stats.roster_ticks.push(t);
    true
}

/// The chained human flight state — the driver's copy of what
/// `Simulation` owns in the app (integer carpet + MC2 channels + the
/// Accelerate expiry edge).
#[derive(Default, Clone)]
struct Chain {
    s: Mc1State,
    ext: Mc2Ext,
    accel_was_active: bool,
}

impl Chain {
    /// Seed from the recorded closure at the anchor (the shared
    /// seeding law — mgc_sim conformance).
    fn seed_mc1(st: &RetailMc1, slot: u16) -> Self {
        Chain {
            s: mc1_state_from_retail(st, slot),
            ext: Mc2Ext::default(),
            accel_was_active: false,
        }
    }

    /// The MC2 twin — plus the debuff ladders and water/nudge
    /// channels the pose channel gates instead of seeding.
    fn seed_mc2(st: &RetailMc2, slot: u16, row: flight::Mc2Row) -> Self {
        let (s, ext) = mc2_state_from_retail(st, slot, row);
        Chain {
            s,
            ext,
            accel_was_active: false,
        }
    }

    fn pose(&self) -> PlayerPose {
        integer_pose(&self.s)
    }
}

/// `MGC_REPLAY_BUILDING_PATCHES=1` (round 157): turn the three BUILD
/// patches (`mc1_crushed_site_collapse`, `mc1_building_pad_saturate`,
/// `mc2_building_pad_saturate`) ON over the replay's retail set, to
/// watch what a witness take does with them live. Not a conformance
/// arm: the graded rows after the first patched tick are the patch's
/// effect, not port defects. Re-applied every tick because every
/// re-anchor rebuilds the world.
fn building_patches_probe(world: &mut World) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // The replay world is `strict_retail`, which pins every patch to
    // retail; the sim's `MGC_FORCE_BUILDING_PATCHES` lets these three
    // through (set BOTH env vars).
    if *ON.get_or_init(|| std::env::var_os("MGC_REPLAY_BUILDING_PATCHES").is_some()) {
        let mut p = world.patches();
        if !p.mc1_crushed_site_collapse || !p.mc1_building_pad_saturate || !p.mc2_building_pad_saturate {
            p.mc1_crushed_site_collapse = true;
            p.mc1_building_pad_saturate = true;
            p.mc2_building_pad_saturate = true;
            world.set_patches(p);
        }
    }
}

/// One free-run tick, MC1/HW — `Simulation::step`'s faithful path in
/// integer space: dead/falling input override, Accelerate expiry
/// edge, knock drain at the tick head, then `World::tick_flight` —
/// the MOVER (with the death fall + dead-camera turn) runs INSIDE the
/// walk at the carpet's slot, so its ground probe reads this tick's
/// painted terrain and the walkers below the slot read the record's
/// pre-move pose (the t=563 replay-wall law) — then the
/// respawn/teleport/speed-zero mailboxes back into the carpet.
fn step_mc1(world: &mut World, ch: &mut Chain, inp: Mc1Input, cmd: PlayerCommand) {
    building_patches_probe(world);
    let falling = world.player_falling();
    let dead = world.player_dead();
    // Only the COMMAND handler stops at death (sub_46840 is skipped
    // from state 2 on): casts, thrust and strafe die with it, but the
    // STICK lives in the input pass and keeps feeding the filters all
    // the way down — `World::step_player_flight` carries the law and
    // the freeze that goes with it (`Mc1Input::no_command`). The dead
    // arm needs no zeroed registers here: retail's state-3 dispatch
    // simply never moves.
    let (inp, cmd) = if falling || dead {
        (
            Mc1Input {
                stick_x: inp.stick_x,
                stick_y: inp.stick_y,
                ..Mc1Input::default()
            },
            PlayerCommand {
                respawn: cmd.respawn,
                ..PlayerCommand::default()
            },
        )
    } else {
        (inp, cmd)
    };
    let thrust = if inp.speed_up {
        1.0
    } else if inp.speed_down {
        -1.0
    } else {
        0.0
    };
    world.thrust_cancel(thrust);
    // The Accelerate token writes the speed columns ITSELF at its
    // own walk slot (sub_56380 :65167-78) — a BELOW-carpet token's
    // mail is consumed by step_player_flight before the mover, an
    // ABOVE-carpet token's lands post-move and the driver takes it
    // here after the turn, exactly like the MC2 driver below. (The
    // old "token-below-carpet order" comment was the false premise
    // that stranded the above-carpet mail a full tick: mc1l48 t=51 /
    // mc1l6 t=1618 / mc1l32-quick t=17696.)
    let mut drive = FlightDrive {
        s: &mut ch.s,
        inp,
        over: None,
        falling,
        dead,
        mc2: None,
    };
    world.tick_flight(&mut drive, cmd);
    if let Some(base) = world.take_speed_base() {
        ch.s.tgt_speed = base;
        ch.s.act_speed = base;
    }
    // Respawn (sub_44D30 :54868-83): position at the castle one tile
    // up, then EXACTLY THREE flight registers cleared — `v_12` (target
    // speed), `v_16` (strafe) and the knock triple `v_22/24/26`. The
    // actual speed `+126`, the entity's `+63` tick counter and its
    // private LCG are NOT touched, and the heading is kept: mc1l42
    // t=17398 respawns at `tgt 0 / act -80 / strafe 0`, then servos
    // −64, −48, −32, −16 over the next four ticks with `f63` running
    // 90, 91, 92 straight through. A full `from_tiles` reset restarted
    // the counter and the LCG (retail 810782015, ours 0) and snapped
    // the speed to zero a tick early.
    if let Some((x, z, alt)) = world.take_respawn() {
        ch.s.x = (x.rem_euclid(256.0) * 256.0) as u16;
        ch.s.y = (z.rem_euclid(256.0) * 256.0) as u16;
        // ...AT THE SEAT'S OWN Z, not a tile above it. mc1l42 t=17398
        // respawns on z = 3776 with the site's own terrain reading
        // exactly 3776 (`MGC_CELL_TRACE` (123,13): height 118), so the
        // `+256` read off `tempZ._axis_2d.y++` (:54848) is not what
        // the engine lands on. The z now comes from the SEAT the sim
        // teleported to (:54858-61 copies the castle's whole
        // position), rather than being re-derived from the ground
        // here — one implementation, shared with the app, which used
        // to derive `ground + 1.0` and land 256 units high.
        ch.s.z = (alt * 256.0) as i16;
        ch.s.tgt_speed = 0;
        ch.s.strafe = 0;
    }
    if let Some((x, z, alt)) = world.take_teleport() {
        ch.s.x = (x.rem_euclid(256.0) * 256.0) as u16;
        ch.s.y = (z.rem_euclid(256.0) * 256.0) as u16;
        if let Some(alt) = alt {
            ch.s.z = (alt * 256.0) as i16;
        }
    }
    if world.take_speed_zero() {
        ch.s.tgt_speed = 0;
    }
}

/// The MC2 twin (`Simulation::move_mc2` + step tail): row refresh,
/// signed Accelerate restore, debuff drain, the mover with the cave
/// closures, cave accel-cancel, end-pose seizure.
/// Book a recorded cheat for the tick and count it. The application
/// itself happens inside the tick at retail's phase (`World::tick`);
/// a cheat this build has no handler for is REPORTED, since it
/// guarantees divergence from that tick on.
fn book_cheat(world: &World, cheat: Option<recover::Cheat>, stats: &mut RStats) {
    let Some(c) = cheat else { return };
    if world.cheat_supported(c) {
        *stats.cheats.entry(c.name()).or_default() += 1;
    } else {
        *stats.cheats_unimpl.entry(c.name()).or_default() += 1;
    }
}

fn step_mc2(world: &mut World, ch: &mut Chain, inp: Mc1Input, cmd: PlayerCommand) {
    building_patches_probe(world);
    let falling = world.player_falling();
    let dead = world.player_dead();
    let end_seized = world.mc2_end_pose().is_some();
    // ⭐ THE STICK OUTLIVES THE COMMAND HANDLER ON THE DEATH FALL —
    // MC2's `sub_5F380`/`sub_5D530` split is MC1's `sub_46840`/
    // `sub_455D0` split (see `step_mc1`), and this arm had asserted the
    // opposite. `PlayerEvents_51BB0` recomputes `rollDelta_0x4_4` /
    // `pitchDelta_0x6_6` from the raw cursor every frame (EF:38060-63)
    // with NO life or actionIndex guard anywhere on the path, and
    // state 2's `sub_5E310` opens on `sub_5D530`, so the filters keep
    // integrating all the way down. mc2l0 t=11191-11192: retail holds
    // roll_acc 7 / pitch_acc −135 across both fall ticks because the
    // player's cursor is parked at a FIXED POINT of the filter, where
    // a zeroed stick decayed pitch_acc to −102 and dragged the whole
    // aim_pitch → eff_pitch → polar-step cascade with it.
    // The ending sequence keeps the full seizure: `sub_5E8C0` drives
    // its own exponential decay on the accumulators (EF:60577-87).
    let (inp, cmd) = if falling || dead || end_seized {
        (
            Mc1Input {
                stick_x: if end_seized { 0 } else { inp.stick_x },
                stick_y: if end_seized { 0 } else { inp.stick_y },
                no_command: true,
                mc2_stop: false,
                ..Mc1Input::default()
            },
            PlayerCommand {
                respawn: cmd.respawn,
                ..PlayerCommand::default()
            },
        )
    } else {
        (inp, cmd)
    };
    // ⚠ The dead arm used to zero act/tgt/strafe here. Retail's state-3
    // dispatch never reaches `sub_5F380` OR `sub_5D530`, so it touches
    // none of them: mc2l0 records `16 / 16 / 0` unchanged from the
    // landing tick t=11193 onward. `World::step_player_flight_mc2`
    // carries the whole arm now.
    ch.ext.row = world.mc2_carpet_row();
    let thrust = if inp.speed_up {
        1.0
    } else if inp.speed_down {
        -1.0
    } else {
        0.0
    };
    world.thrust_cancel(thrust);
    // The whole move — Accelerate restore edge, knock, debuff drain,
    // `sub_5D530`, the death fall and the grey-screen turn — runs
    // INSIDE the turn now, at the carpet's own walk slot, exactly as
    // the MC1 driver above does (`World::step_player_flight_mc2`).
    // The tick-head sample here is only the input gate and the row.
    let over = world.accel_override();
    let mut drive = FlightDrive {
        s: &mut ch.s,
        inp,
        over,
        falling,
        dead,
        mc2: Some(Mc2Drive {
            ext: &mut ch.ext,
            accel_was_active: &mut ch.accel_was_active,
        }),
    };
    world.tick_flight(&mut drive, cmd);
    // The SPEED token's register write when its walk slot is ABOVE the
    // carpet's — retail's write lands after `sub_5D530`, so the boost
    // is recorded at this boundary but first MOVES the carpet next
    // tick. (Below-carpet tokens are consumed inside the walk, at the
    // carpet's own dispatch, and this take finds nothing.)
    if let Some(base) = world.take_speed_base() {
        ch.s.tgt_speed = base;
        ch.s.act_speed = base;
    }
    if let Some((x, z, alt)) = world.take_respawn() {
        // ⭐ A RESPAWN MOVES THE CARPET, IT DOES NOT REBUILD IT — the
        // MC1 arm's law (`step_mc1`: position, then EXACTLY the target
        // speed and strafe cleared) one game over. Rebuilding through
        // `from_tiles` zeroed lanes retail carries straight across the
        // death: mc2l0 t=11219 resumes with `eff_pitch` STILL 1913,
        // the value stranded before the fall because only `sub_5D530`
        // writes it and the corpse never ran one, and with roll/pitch
        // integrating out of the state-3 wipe on the live stick
        // (1 and −33 = the cursor's own delta from 0).
        ch.s.x = (x.rem_euclid(256.0) * 256.0) as u16;
        ch.s.y = (z.rem_euclid(256.0) * 256.0) as u16;
        ch.s.z = (alt * 256.0) as i16;
        ch.s.tgt_speed = 0;
        ch.s.strafe = 0;
    }
    if let Some((x, z, alt)) = world.take_teleport() {
        ch.s.x = (x.rem_euclid(256.0) * 256.0) as u16;
        ch.s.y = (z.rem_euclid(256.0) * 256.0) as u16;
        if let Some(alt) = alt {
            ch.s.z = (alt * 256.0) as i16;
        }
    }
    if world.take_speed_zero() {
        ch.s.tgt_speed = 0;
    }
    // The ending sequence used to be mirrored onto the carpet HERE,
    // from the float `mc2_end_pose`, with both speed registers forced
    // to 0. It is written at the carpet's own walk slot now
    // (`World::mc2_carpet_dispatch`, the actionIndex-11/12 table
    // entry) in engine units and with the scripted `actSpeed` intact —
    // retail's sequence bleeds and re-accelerates that register (4/tick
    // down, +8/tick up to 200) and the recorder captures every value,
    // while `speed_0xc_12` it never touches at all.
}

// ------------------------------------------------------- input recovery
//
// The recovery laws live in the shared home (mgc_formats::recover);
// this driver only widens the recovered pair into the mover's input.

fn mc1_mover_input(mb: u32, stick: (i16, i16)) -> Mc1Input {
    Mc1Input {
        stick_x: stick.0,
        stick_y: stick.1,
        speed_up: mb & 1 != 0,
        speed_down: mb & 2 != 0,
        strafe_left: mb & 4 != 0,
        strafe_right: mb & 8 != 0,
        // Cleared at the carpet's dispatch on the death fall
        // (`World::step_player_flight`).
        no_command: false,
        mc2_stop: false,
        mc2_park: false,
    }
}

// ------------------------------------------------------------ traces
//
// Game-agnostic instruments, shared by both replay arms. They were
// MC1-only until the MC2 phase dig needed them (the carpet lagging a
// mid-walk terrain raise, mc2l3 t=244) — nothing in either is MC1
// specific, and an instrument that exists for one game and silently
// no-ops for the other reads as "the port is clean here".

/// `MGC_POSE_WINDOW=<t0>-<t1>` at an MC1 grade site — see
/// [`emit_pose_window`]. The window is deliberately shared with the
/// app's `--replay-check`: its whole reason for existing is putting the
/// two retail drivers' carpets side by side on the same ticks.
fn pose_window_mc1(t: u64, s: &Mc1State, e: &RetailEntMc1, w: &RetailWizardMc1) {
    if pose_window().is_none() {
        return;
    }
    emit_pose_window(t, &pose_all_mc1(s, e, w), &[]);
}

/// The MC2 twin. `extras` are the death column's two retail-only tells.
fn pose_window_mc2(t: u64, s: &Mc1State, ext: &Mc2Ext, e: &RetailEntMc2, p: &RetailPlayerMc2) {
    if pose_window().is_none() {
        return;
    }
    emit_pose_window(
        t,
        &pose_all_mc2(s, ext, e, p),
        &[("f2c", e.f2c as i64), ("action45", e.action45 as i64)],
    );
}

/// `MGC_CELL_TRACE=<x>,<y>[;<x>,<y>…]:<t0>:<t1>` — the terrain-drift
/// microscope: the port's live height/type/angle planes beside the
/// truth channel at the watched cells, printed on change. Terrain is
/// invisible to grading until something stands on it — this is how a
/// plant tick is found (the spurious castle-paint apron dip, mc1l0
/// t=3856; the fire-cell angle split, t=4290).
#[derive(Default)]
struct CellTrace {
    cells: Vec<(u8, u8)>,
    window: (u64, u64),
    last: Vec<Option<((u8, u8, u8), (u8, u8, u8))>>,
}

impl CellTrace {
    fn from_env() -> Self {
        let parsed = std::env::var("MGC_CELL_TRACE").ok().and_then(|v| {
            let (cells, ts) = v.split_once(':')?;
            let (a, b) = ts.split_once(':')?;
            let cells: Vec<(u8, u8)> = cells
                .split(';')
                .filter_map(|c| {
                    let (x, y) = c.split_once(',')?;
                    Some((x.parse::<u8>().ok()?, y.parse::<u8>().ok()?))
                })
                .collect();
            Some((cells, a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
        });
        let Some((cells, t0, t1)) = parsed else {
            return Self::default();
        };
        let last = vec![None; cells.len()];
        Self {
            cells,
            window: (t0, t1),
            last,
        }
    }

    fn emit(&mut self, world: &World, timg: &Option<mgc_formats::mgcr::TerrainImage>, t: u64) {
        if self.cells.is_empty() || t < self.window.0 || t > self.window.1 {
            return;
        }
        for (k, &(cx, cy)) in self.cells.iter().enumerate() {
            let idx = ((cy as usize) << 8) | cx as usize;
            let p = world.planes();
            let port = (p.height[idx], p.tile_type[idx], p.angle[idx]);
            let plane = |name: &str| {
                timg.as_ref()
                    .and_then(|i| i.plane(name))
                    .map_or(0, |v| v[idx])
            };
            let truth = (plane("height"), plane("type"), plane("angle"));
            if self.last[k] == Some((port, truth)) {
                continue;
            }
            self.last[k] = Some((port, truth));
            println!(
                "CELL t={t} ({cx},{cy}) port h/ty/an={}/{}/{:#04x} truth={}/{}/{:#04x}{}",
                port.0,
                port.1,
                port.2,
                truth.0,
                truth.1,
                truth.2,
                if port != truth { "  <-- DRIFT" } else { "" }
            );
        }
    }
}

/// `MGC_PLANE_CENSUS=<t0>:<t1>` — the CARVING comparator: all four
/// terrain planes, whole map, every tick of the window, port beside
/// the take's measured truth channel.
///
/// This is the ONLY instrument that grades terrain over TIME.
/// `terrain-check`/`terrain-diff` grade **record 0 only** (the
/// stock-bake validator), and `replay --segmented` INSTALLS the
/// measured planes at every anchor without ever comparing them — so a
/// port that carved a cave differently from tick 1 onward scored a
/// clean sweep. Round 131 (mc2l5, the castle-through-rock take) built
/// this to close that hole and measured the port's cave carving
/// BIT-PERFECT over 96,213 ticks on height/type/ceiling, with only
/// `angle` bits 4-6 — the texture-rotation nibble — drifting.
///
/// A line is printed only when the census CHANGES (a steady-state
/// mismatch prints once), and `anglebits` is the OR of every
/// `port ^ truth` byte on the angle plane, so the cosmetic nibble
/// (0x70) is distinguishable at a glance from a load-bearing bit:
/// bit 3 = SEALED on a cave level, bit 7 = BUILT — the two the
/// castle-site rule reads (`sub_11CB0`).
///
/// ⭐⭐ **`anglebits=0x70` IS THE EXPECTED FLOOR, NOT A DEFECT.** Bits
/// 4-6 hold `16 * (pseudo % 7)`, the retile pass's texture-rotation
/// draw (`terrain_paint.rs`, `pseudo = pseudo*9377 + 9439`), and the
/// retile LCG **has no capture** — the importer forces
/// `Gen::pseudo = 0` at every anchor (`conformance.rs`), so the port's
/// stream is re-phased against retail's on each re-seed by
/// construction. No gameplay predicate reads bits 4-6 (the one site
/// that touches them, `castle.rs`'s `(angle & 0x70) | 1`, only
/// PRESERVES them). Round 132 measured the corpus floor: mc2l5 96,213
/// ticks / mc2l22 65,556 / mc2l24 54,057 / mc2l15 43,187 / mc2l31
/// 29,465 — **288,478 ticks, height/type/ceiling diff ZERO on every
/// one, angle drift confined to 0x70 throughout.** Treat any bit
/// OUTSIDE 0x70, or any height/type/ceiling count at all, as the real
/// signal.
///
/// A SUMMARY line lands when the window closes (or the run ends), so
/// a sweep can be graded from the tail alone.
struct PlaneCensus {
    window: Option<(u64, u64)>,
    /// The last printed (height, type, ceiling, angle) counts —
    /// change-gated output.
    last: Option<[usize; 4]>,
    /// Worst per-plane count seen, the OR of every angle-diff byte,
    /// the first dirty tick and the tick count, for the summary.
    worst: [usize; 4],
    angle_bits: u8,
    /// Which of the four planes the take actually DECLARES — an
    /// absent truth plane is not a zero plane, and a summary that did
    /// not say so would read "ceiling bit-perfect" on a level that
    /// has no ceiling.
    graded: Vec<&'static str>,
    first_dirty: Option<u64>,
    ticks: u64,
    dirty_ticks: u64,
    done: bool,
}

impl PlaneCensus {
    fn from_env() -> Self {
        Self {
            window: std::env::var("MGC_PLANE_CENSUS").ok().and_then(|v| {
                let (a, b) = v.split_once(':')?;
                Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
            }),
            last: None,
            worst: [0; 4],
            angle_bits: 0,
            graded: Vec::new(),
            first_dirty: None,
            ticks: 0,
            dirty_ticks: 0,
            done: false,
        }
    }

    fn emit(&mut self, world: &World, timg: &Option<mgc_formats::mgcr::TerrainImage>, t: u64) {
        let Some((t0, t1)) = self.window else { return };
        if t > t1 {
            self.summary();
            return;
        }
        if t < t0 {
            return;
        }
        let Some(img) = timg.as_ref() else {
            if !self.done {
                self.done = true;
                println!("PLANECENSUS t={t} NO MEASURED TERRAIN CHANNEL — nothing to grade");
            }
            return;
        };
        let p = world.planes();
        // The port keeps `ceiling`/`angle` EMPTY where the level has
        // none; a plane the take does not declare is not graded (an
        // absent truth plane is not a zero plane).
        let port: [&[u8]; 4] = [&p.height, &p.tile_type, &p.ceiling, &p.angle];
        let mut n = [0usize; 4];
        let mut bits = 0u8;
        let mut first: Vec<(usize, usize, u8, u8)> = Vec::new();
        for (k, name) in ["height", "type", "ceiling", "angle"].iter().enumerate() {
            let Some(truth) = img.plane(name) else { continue };
            let pk = port[k];
            if pk.is_empty() {
                continue;
            }
            if !self.graded.contains(name) {
                self.graded.push(name);
            }
            for i in 0..truth.len().min(pk.len()) {
                if truth[i] != pk[i] {
                    n[k] += 1;
                    if k == 3 {
                        bits |= truth[i] ^ pk[i];
                    } else if first.len() < 8 {
                        first.push((i & 0xFF, i >> 8, pk[i], truth[i]));
                    }
                }
            }
        }
        self.ticks += 1;
        for k in 0..4 {
            self.worst[k] = self.worst[k].max(n[k]);
        }
        self.angle_bits |= bits;
        if n.iter().any(|&c| c > 0) {
            self.dirty_ticks += 1;
            if self.first_dirty.is_none() {
                self.first_dirty = Some(t);
            }
        }
        if self.last == Some(n) {
            return;
        }
        self.last = Some(n);
        println!(
            "PLANECENSUS t={t} height={} type={} ceiling={} angle={} anglebits={:#04x}{}{}",
            n[0],
            n[1],
            n[2],
            n[3],
            bits,
            if first.is_empty() {
                String::new()
            } else {
                format!(" first={first:?}")
            },
            // The two angle bits the castle-site rule reads. Bits 4-6
            // are the texture-rotation nibble (cosmetic).
            if bits & 0x88 != 0 {
                "  <-- LOAD-BEARING (bit3 SEALED / bit7 BUILT)"
            } else {
                ""
            }
        );
    }

    /// One machine-readable verdict for the whole window.
    fn summary(&mut self) {
        if self.done || self.window.is_none() {
            return;
        }
        self.done = true;
        let (t0, t1) = self.window.unwrap();
        println!(
            "PLANECENSUS SUMMARY window={t0}:{t1} ticks={} planes={} dirty={} worst height={} \
             type={} ceiling={} angle={} anglebits={:#04x} first_dirty={} VERDICT={}",
            self.ticks,
            self.graded.join(","),
            self.dirty_ticks,
            self.worst[0],
            self.worst[1],
            self.worst[2],
            self.worst[3],
            self.angle_bits,
            self.first_dirty
                .map_or_else(|| "none".to_string(), |t| t.to_string()),
            if self.worst[..3].iter().all(|&c| c == 0) && self.angle_bits & 0x88 == 0 {
                if self.worst[3] == 0 {
                    "BIT-PERFECT"
                } else {
                    "COSMETIC-ONLY (angle rotation nibble)"
                }
            } else {
                "DRIFT"
            }
        );
    }
}

impl Drop for PlaneCensus {
    fn drop(&mut self) {
        self.summary();
    }
}

/// `MGC_PACE_TRACE=<t0>:<t1>` — **THE DIFFICULTY-PACE TRANSITION-LIST
/// DIFF**, the instrument round 131's W8 dig named and did not build.
///
/// W8 answered the player's *"later in the level they are more
/// difficult to kill"* out of retail's own recorded bytes: the ramp is
/// **three interleaved DISCRETE schedules** — each rival's one-time
/// life ratchet (`sub_5C950` resets `word_0x24A_586` to 256 on its
/// first death, permanently discarding the authored handicap), rebuild
/// churn at the ratcheted value, and the authored stage roster
/// escalating in waves. Every piece of the ARITHMETIC is graded.
/// **The RATE is not**: `replay` re-imports rival mana, `mana_max`,
/// castle caps and the live StageVar table at every anchor, so a port
/// whose rivals banked twice as fast, or whose stage script released
/// the 60,000-HP wave 10,000 ticks early or never at all, scores
/// **zero divergences corpus-wide**.
///
/// What is graded here is not a value but a **TICK LIST**: the tick at
/// which each transition first fires, on retail's side out of the
/// recorded closure and on the port's out of the live world, compared
/// key by key. Three transition families —
///
/// * `rung p<i>=<n>` — player i's ESTABLISHED-castle register naming a
///   castle at rung n. Round 131's 131-5 proved the register
///   (`CastleEntityIndex_0x3A_58`) has exactly one writer, the level-up
///   commit `sub_60480`, so this edge IS the rung commit.
/// * `ratchet p<i>` — player i's `word_0x24A_586` stepping to 256 off
///   an authored handicap: the 5.02× HP-and-healing jump, once per
///   rival per level, and the single biggest term in what the player
///   feels.
/// * `model (c,m)` — the first tick a (class, model) is on the roster,
///   which is how the authored wave schedule shows itself.
///
/// ⚠⚠ **RUN IT ON A PLAIN `replay`, NEVER `--segmented`.** A
/// re-anchor imports retail's own castle register, life scalars and
/// StageVar table, which is precisely the contamination that made the
/// rate ungraded in the first place — under `--segmented` this
/// instrument would grade retail against itself and always read clean.
/// A plain free run diverges, and that is the point: keys are player
/// index and (class, model), never slot, so the lists stay comparable
/// past the horizon.
///
/// A `d=` of zero on every key over a long take is the strong result;
/// a systematically SIGNED drift on the `ratchet` or `rung` families
/// is a pace defect, and a key present on one side only is a schedule
/// the port never runs (or invents).
///
/// ⭐⭐⭐ **WHAT ROUND 132 MEASURED, AND WHAT IT MEANS FOR W8's
/// "THE RATE IS UNGRADED".** Six MC2 takes, **490 graded pace keys,
/// every single one exact** — mc2l6-rsg 120/120 to a horizon of
/// 34,604, mc2l24 104/104 to 35,669, mc2l31 56/56 to END, plus mc2l6
/// 79/79, mc2l22 85/85 and mc2l5 46/46. Not one rung commit, rival
/// ratchet or creature first-appearance is off by a tick.
///
/// The honest reading is narrower than that sounds, and it is the
/// point: inside a bit-exact horizon the port's world IS retail's, so
/// agreement there is implied by the horizon rather than discovered by
/// this instrument. What round 132 actually established is that **W8's
/// worry was scoped to the SEGMENTED lane.** `--segmented` re-imports
/// rival mana, `mana_max`, castle caps and the StageVar table at every
/// anchor, and it is that lane which cannot see a pace defect. A PLAIN
/// free run re-imports nothing, so it grades the rate for free — as far
/// as its horizon reaches, which on the two longest MC2 free runs is
/// the whole difficulty schedule of the level. This instrument's job is
/// to make that legible without reading 35,000 boundaries, and to say
/// exactly where the grading stops.
///
/// ⏭ What is still ungraded: pace BEYOND each take's horizon — mc2l22
/// past 4,662 and mc2l5 past 3,975, where the authored 36,000/60,000
/// and 36,000/300,000 HP waves actually land. Those need the horizons
/// moved, not a better instrument.
struct PaceTrace {
    window: Option<(u64, u64)>,
    /// key → (retail first tick, port first tick).
    seen: std::collections::BTreeMap<String, (Option<u64>, Option<u64>)>,
    prev_scale: Option<([u16; 8], [u16; 8])>,
    /// The run's first divergent boundary. Past it the port's world is
    /// its OWN, not a measurement of retail's schedule, so keys first
    /// seen beyond it are reported and NOT scored.
    horizon: Option<u64>,
    done: bool,
}

impl PaceTrace {
    fn from_env() -> Self {
        Self {
            window: std::env::var("MGC_PACE_TRACE").ok().and_then(|v| {
                let (a, b) = v.split_once(':')?;
                Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
            }),
            seen: std::collections::BTreeMap::new(),
            prev_scale: None,
            horizon: None,
            done: false,
        }
    }

    /// `side` 0 = retail, 1 = port.
    fn mark(&mut self, key: String, side: usize, t: u64) {
        let e = self.seen.entry(key).or_insert((None, None));
        let slot = if side == 0 { &mut e.0 } else { &mut e.1 };
        if slot.is_none() {
            *slot = Some(t);
        }
    }

    fn emit(&mut self, world: &World, st: &mgc_formats::mgcr::RetailMc2, t: u64) {
        let Some((t0, t1)) = self.window else { return };
        if t > t1 {
            self.summary();
            return;
        }
        if t < t0 {
            return;
        }
        // ---- retail, straight out of the recorded closure
        let mut r_rungs = [0i16; 8];
        let mut r_scale = [0u16; 8];
        for (i, p) in st.players.iter().enumerate().take(8) {
            r_scale[i] = p.life_scale as u16;
            let c = p.castle_ent as usize;
            if p.castle_ent > 0
                && c < st.ents.len()
                && st.ents[c].class3f == 3
                && st.ents[c].model40 == 2
                && st.ents[c].flags & 0x400 == 0
            {
                // ⚠ A CASTLE'S LEVEL IS `dword_0x10_16`, NOT `+0x26`.
                // Retail's `+0x26` on a (3,2) is the generic hit
                // source and reads 0 forever; the rung the level-up
                // commit increments (`sub_60480`, VA 0x6047F
                // `dword_0x10_16++`, in the same straight-line block
                // as the register stamp at VA 0x60534) is @0x10 —
                // which is exactly what the importer's catch-all
                // `_ => r.scratch10 as i16` puts in the port's `f26`.
                // Reading the same-named field on both sides was
                // wrong on the retail side, and it read "the port
                // INVENTS every rung".
                r_rungs[i] = (st.ents[c].scratch10.clamp(0, 7)) as i16;
            }
        }
        for e in st.ents.iter().skip(1) {
            if e.class3f == 0 || e.flags & 0x400 != 0 {
                continue;
            }
            self.mark(format!("model ({:2},{:2})", e.class3f, e.model40), 0, t);
        }
        // ---- port
        let (p_rungs, p_scale, p_models) = world.debug_mc2_pace_census();
        for (c, m) in p_models {
            self.mark(format!("model ({c:2},{m:2})"), 1, t);
        }
        for i in 0..8 {
            if r_rungs[i] > 0 {
                self.mark(format!("rung  p{i}={}", r_rungs[i]), 0, t);
            }
            if p_rungs[i] > 0 {
                self.mark(format!("rung  p{i}={}", p_rungs[i]), 1, t);
            }
        }
        // The ratchet is an EDGE, not a level: 256 is also the
        // un-handicapped default, so only a step UP off an authored
        // value below it counts.
        if let Some((pr, pp)) = self.prev_scale {
            for i in 0..8 {
                if r_scale[i] == 256 && pr[i] != 0 && pr[i] < 256 {
                    self.mark(format!("ratchet p{i} ({}->256)", pr[i]), 0, t);
                }
                if p_scale[i] == 256 && pp[i] != 0 && pp[i] < 256 {
                    self.mark(format!("ratchet p{i} ({}->256)", pp[i]), 1, t);
                }
            }
        }
        self.prev_scale = Some((r_scale, p_scale));
    }

    /// Called once the run is over, with the horizon it reached.
    fn finish(&mut self, horizon: Option<u64>) {
        self.horizon = horizon;
        self.summary();
    }

    fn summary(&mut self) {
        if self.done || self.window.is_none() {
            return;
        }
        self.done = true;
        // ⚠⚠ **A PACE KEY IS ONLY GRADED INSIDE THE HORIZON.** A plain
        // free run keeps going past its first divergence, and from
        // there on the port is simulating its OWN world — a rung it
        // never reaches, or reaches 7,000 ticks late, is the
        // divergence talking, not the schedule. Round 132 built this
        // instrument without the split and it read "SCHEDULE MISMATCH"
        // on mc2l5 and mc2l22, whose free-run horizons are 551 and
        // 4,661: every one of those rows was post-horizon noise. The
        // takes that answer the pace question are the ones that
        // free-run FAR — mc2l6-rsg (34,603 bit-exact boundaries) is
        // this instrument's real subject.
        let hz = self.horizon.unwrap_or(u64::MAX);
        let (mut both, mut only_r, mut only_p, mut drift) = (0usize, 0usize, 0usize, 0i64);
        let mut ungraded = 0usize;
        let mut worst = (0i64, String::new());
        for (k, &(r, p)) in &self.seen {
            // (3,0) is the HUMAN'S CARPET. Retail keeps it as an
            // ordinary pool record; the port's replay seat drives the
            // human out-of-pool behind the `PLAYER_TARGET` sentinel,
            // so this key is retail-only on every MC2 take by
            // construction. A known structural difference, not a
            // schedule the port fails to run — reported, never scored.
            if k == "model ( 3, 0)" {
                println!("PACE {k}  retail_t={r:?} port_t={p:?}  [structural: out-of-pool human]");
                continue;
            }
            // A key whose EARLIEST sighting on either side already
            // lies past the horizon was never graded by anything.
            let earliest = r.into_iter().chain(p).min().unwrap_or(u64::MAX);
            if earliest > hz {
                ungraded += 1;
                println!("PACE {k}  retail_t={r:?} port_t={p:?}  [past horizon {hz} — UNGRADED]");
                continue;
            }
            match (r, p) {
                (Some(a), Some(b)) => {
                    both += 1;
                    let d = b as i64 - a as i64;
                    if d != 0 {
                        drift += 1;
                        if d.abs() > worst.0.abs() {
                            worst = (d, k.clone());
                        }
                    }
                    println!("PACE {k}  retail_t={a} port_t={b} d={d:+}");
                }
                (Some(a), None) => {
                    only_r += 1;
                    println!("PACE {k}  retail_t={a} port_t=NEVER  <-- the port never runs this");
                }
                (None, Some(b)) => {
                    only_p += 1;
                    println!("PACE {k}  retail_t=NEVER port_t={b}  <-- the port INVENTS this");
                }
                (None, None) => {}
            }
        }
        println!(
            "PACE SUMMARY horizon={} keys={} graded={} ungraded={ungraded} matched={both} \
             drifted={drift} retail_only={only_r} port_only={only_p} worst={:+} ({}) VERDICT={}",
            self.horizon
                .map_or_else(|| "END".to_string(), |h| h.to_string()),
            self.seen.len() - usize::from(self.seen.contains_key("model ( 3, 0)")),
            both + only_r + only_p,
            worst.0,
            if worst.1.is_empty() { "-" } else { &worst.1 },
            if only_r + only_p > 0 {
                "SCHEDULE MISMATCH"
            } else if drift > 0 {
                "PACE DRIFT"
            } else {
                "IN STEP"
            }
        );
    }
}

impl Drop for PaceTrace {
    fn drop(&mut self) {
        self.summary();
    }
}

/// `MGC_KNOCK_TRACE=<t0>:<t1>` — the knock PHASE probe. Prints what
/// the mover consumed this tick vs what the world tick armed for the
/// next, beside retail's own recorded `+22`/`+24` pair. The whole
/// question the MC2 in-walk mover answers ("does a hit posted at a
/// LOWER slot shove THIS tick's move?") is one line of this trace.
struct KnockTrace {
    window: Option<(u64, u64)>,
    pre: (u16, i16),
}

impl KnockTrace {
    fn from_env() -> Self {
        Self {
            window: std::env::var("MGC_KNOCK_TRACE").ok().and_then(|v| {
                let (a, b) = v.split_once(':')?;
                Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
            }),
            pre: (0, 0),
        }
    }

    /// Sample the armed knock BEFORE the driver steps the pair.
    fn arm(&mut self, world: &World) {
        if self.window.is_some() {
            self.pre = world.debug_player_knock();
        }
    }

    /// `pt`/`t` are the pair's endpoints; `rec` is retail's recorded
    /// (dir, mag) at each of them.
    fn emit(&self, world: &World, pt: u64, t: u64, rec_pt: (u16, i16), rec_t: (u16, i16)) {
        let Some((t0, t1)) = self.window else { return };
        if pt < t0 || pt > t1 {
            return;
        }
        let post = world.debug_player_knock();
        println!(
            "KNOCK pair {pt}->{t}  port consumed=({},{}) armed=({},{})  \
             retail rec@{pt}=({},{}) @{t}=({},{})",
            self.pre.0, self.pre.1, post.0, post.1, rec_pt.0, rec_pt.1, rec_t.0, rec_t.1
        );
    }
}

// ---------------------------------------------------------- aggregation

/// (Re-)anchor the free run on the recording at `t`: pristine planes,
/// the recorded closure imported, the measured image installed OVER
/// the import (the importer's terrain-replay pass reconstructs
/// state-derived edits for measurement-less runs and would
/// DOUBLE-APPLY them on already-measured planes), the fire latch
/// re-armed and the flight chain re-seeded.
///
/// The gap path and `--segmented`'s deviation reset are the SAME
/// operation — that is the whole content of the segmented design: a
/// detected deviation re-anchors exactly the way a capture gap
/// already did.
fn anchor_mc1(
    world: &mut World,
    pristine: &mgc_sim::engine::features::Planes,
    timg: &Option<mgc_formats::mgcr::TerrainImage>,
    st: &RetailMc1,
    t: u64,
) -> Result<(Chain, u16, usize), String> {
    world.restore_planes(pristine);
    let report = world
        .retail_import_mc1(st)
        .map_err(|e| format!("t={t}: import: {e}"))?;
    if let Some((h, ty, ceil, an)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil, an)
            .map_err(|e| format!("t={t}: terrain: {e}"))?;
    }
    let (fl, fr) = recover::mc1_fire(st.wizards[st.local_player as usize].move_bits);
    world.set_prev_fire(fl, fr);
    Ok((
        Chain::seed_mc1(st, report.human_slot),
        report.human_slot,
        report.active,
    ))
}

/// The MC2 twin: same contract, plus the THING table (MC2 ctors read
/// it) and the carpet's tuning row for the chain seed.
///
/// ⚠ The install order is the OPPOSITE of [`anchor_mc1`]'s, and both
/// are load-bearing: the MC2 importer's terrain-replay reconstruct
/// pass is gated on `measured_terrain` and DRAWS PSEUDO, so terrain
/// must install FIRST (the pair path's order, verify_mc2.rs) or every
/// anchor burns phantom pseudo draws against a gate that was meant to
/// be closed; MC1's importer reconstructs unconditionally and its
/// edits must be measured OVER (see anchor_mc1's doc).
fn anchor_mc2(
    world: &mut World,
    pristine: &mgc_sim::engine::features::Planes,
    things: &mgc_sim::engine::world::conformance::ThingTable,
    timg: &Option<mgc_formats::mgcr::TerrainImage>,
    st: &RetailMc2,
    t: u64,
) -> Result<(Chain, u16), String> {
    world.restore_planes(pristine);
    if let Some((h, ty, ceil, an)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil, an)
            .map_err(|e| format!("t={t}: terrain: {e}"))?;
    }
    world.restore_thing_table(things);
    let report = world
        .retail_import_mc2(st)
        .map_err(|e| format!("t={t}: import: {e}"))?;
    // ⚠⚠⚠ A ROW THAT DOES NOT DECODE IS A SILENT WHOLE-BEHAVIOUR
    // SUBSTITUTION, AND THE WORLD-MODE BANNER NEVER SAID SO. The pair
    // path has printed `behavior base … N bad rows` since the importer
    // was written (verify.rs); this path printed neither, so
    // `mc2l22-new`'s seed imported all 645 live records on the
    // stand-in row 59 (`base160` = 0 in its record 0 — see
    // `mgcr::mc2_base160`) and the only evidence was the divergence
    // itself. stderr, so `--brief`'s stdout stays machine-readable.
    if report.bad_rows > 0 {
        eprintln!(
            "   ⚠ t={t}: {} of {} imported records have an UNDECODABLE behaviour row \
             (base {:#x}) — every one of them falls back to the stand-in row 59",
            report.bad_rows, report.active, report.behavior_base
        );
    }
    let (fl, fr) = recover::mc1_fire(st.players[st.local_player as usize].move_bits);
    world.set_prev_fire(fl, fr);
    let row = world.mc2_carpet_row();
    Ok((
        Chain::seed_mc2(st, report.human_slot, row),
        report.human_slot,
    ))
}

/// A recorded boundary's verdict, folded per segment. The headline is
/// the HORIZON — graded boundaries bit-exact from the anchor before
/// the first divergence; after it, traffic is tallied but the run
/// never reseeds (pure replay).
#[derive(Default)]
struct Segment {
    t0: u64,
    end: u64,
    stepped: u64,
    graded: u64,
    ungraded: u64,
    clean: u64,
    horizon: Option<u64>,
    first_render: String,
    /// Compact first-divergence signature for `--brief`
    /// (`(9,0)slot399:id,x,y` / `pose:vx` / `rng` / `missing(5,9)`).
    sig: String,
    firsts: BTreeMap<&'static str, u64>,
    pose_rows: u64,
    rng_bad: u64,
    missing: u64,
    extra: u64,
    field_rows: u64,
    /// Why this segment had to open. The FIRST segment and every one
    /// behind a capture gap are free; a `Deviation` is a port failure
    /// and the only kind that counts against certification.
    opened_by: SegOpen,
}

/// What forced a segment to open — the certification arithmetic.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SegOpen {
    /// The take's first anchor.
    #[default]
    Seed,
    /// A hole in the capture: the recording could not be paired across
    /// it, so the reset is a property of the RECORDING, not the port.
    Gap,
    /// A true incremental deviation (`--segmented` only).
    Deviation,
    /// ⭐ A boundary whose EVERY row is a registered DEVIATION
    /// (`conformance/known-deviations.json`, `status: deviation`) —
    /// PLAYER-RULED retail behaviour the port deliberately does not
    /// reproduce, so the reset is a property of the RULING, not a port
    /// failure, and it does not count against certification.
    ///
    /// The re-anchor still HAPPENS, and that is the whole point: a
    /// registered row is excused as a DEFECT, never as a divergence.
    /// Its value propagates (a fitted RNG draw, a painted tile) and no
    /// amount of ruling makes the port's downstream state retail's, so
    /// the only sound way to keep measuring past one is to re-import
    /// retail's state — which is exactly what a segmented reset is.
    /// `--segmented` therefore needs no flag; a plain free run opts in
    /// with `--resync-deviations`.
    ///
    /// ⚠ A boundary is excused only when EVERY row on it matches a
    /// `deviation` rule. One unexplained row and the whole boundary is
    /// a `Deviation` again — which is what stops a field-scoped rule
    /// from ever hiding a compound break.
    Roster,
    /// The PORT signalled a level restart (`World::take_restart` — the
    /// castle-less PERMADEATH respawn, MC1 case 0xF :48628-31 / MC2
    /// EF:37671-75). Retail's reload runs machinery whose inputs are
    /// not in the recording — MC1 re-reads the level (`sub_408D0`
    /// :51596 → LoadLevel) OVER ITS OWN LIVE HEAP RESIDUE; MC2
    /// re-reads a PRE-CAPTURE disk checkpoint (SaveLevel slot 1,
    /// EF:39894-921) — so the reload boundary is re-anchored like a
    /// gap: a property of the mechanism, not a port failure, and it
    /// does not count against certification.
    ///
    /// ⛔ THE OLD JUSTIFICATION HERE WAS FALSE (round 116). It claimed
    /// MC1 "re-runs LoadLevel+GenerateFeatures with its own reseed
    /// over its own heap residue (:51592-609)". There is no `srand` in
    /// `sub_408D0_40C10` and none in the shipped binary: the reload
    /// regenerates MID-STREAM, and the port reaches the seam bit-exact
    /// INCLUDING rng. What is actually unrecoverable is the RESIDUE —
    /// the previous life's heap is an input to the reload and the
    /// capture does not hold it. The conclusion survives; the reason
    /// did not.
    ///
    /// A plain free run opts in with `--resync-restarts`.
    Restart,
}

#[derive(Default)]
struct RStats {
    segs: Vec<Segment>,
    gates: BTreeMap<&'static str, u64>,
    stick_unrec: u64,
    /// Pairs whose stick was recovered a SECOND time off the trial
    /// step's true crank count — see the trial step in the MC2 loop.
    /// ⚠ RELABELLED round 140: this used to count only "the port
    /// counted a total other than 1" (the historical arity
    /// assumption). It now ALSO counts sim-witness repasses — pairs
    /// where the RECORDED witness is dark (no `word_0x30_48` edge, no
    /// grab latch) but the port's own walk cranked, which the pair
    /// predicate cannot see (`MGC_NO_MC2_WW_CRANK_SIM_WITNESS`). So a
    /// rise here is not by itself a regression: landing the crank
    /// accumulation law took mc2l16 from 114 to 127 while removing 50
    /// divergences.
    crank_repass: u64,
    respawns: u64,
    suicides: u64,
    equips: u64,
    rebind_dropped: u64,
    /// Boundaries the RECORDING spent PAUSED — its own category, not a
    /// tear and not a graded turn. Retail's paused frame draws the
    /// global LCG and returns, so the port reproduces it with
    /// `World::tick_paused` and there is nothing to grade: the pool is
    /// frozen on both sides by construction. Counting these as torn
    /// (what the tear heuristic did) hid a 66.8-second pause at the
    /// head of mc1l6 behind a 1,602-tick "horizon" that graded
    /// nothing. See `recover::paused_turn_mc1`.
    paused: u64,
    /// Recorded retail cheats replayed into the world, by name, and
    /// the ones this build has no handler for — an UNAPPLIED cheat is
    /// a guaranteed divergence from that tick on, so it is reported
    /// rather than swallowed.
    cheats: BTreeMap<&'static str, u64>,
    cheats_unimpl: BTreeMap<&'static str, u64>,
    /// `--classify` verdict per classified reset-cluster head:
    /// true = LOCAL (the pair at t-1 is itself dirty ⇒ fixture
    /// candidate), false = INHERITED (the pair is clean ⇒ the break
    /// rides earlier state — unit test / upstream dig).
    class_tags: BTreeMap<u64, bool>,
    /// `--stop-at-divergence`: the boundary the run stopped on. Every
    /// count in the report is truncated there, and both renderers say
    /// so — the flag must never be mistakable for a full sweep.
    truncated: Option<u64>,
    /// ⭐ THE ROSTER LEDGER — per-rule hit counts for the boundaries
    /// `SegOpen::Roster` excused, keyed by rule index. The roster's own
    /// doctrine (roster.rs) is that a masking rule must be VISIBLE: a
    /// rule that suddenly excuses an order of magnitude more boundaries
    /// is a signal, so `replay` prints this table exactly the way
    /// `verify-deltas` prints its own.
    roster_hits: BTreeMap<String, (u64, u64)>,
    /// Boundaries excused by the roster, in order — the companion list
    /// to `reset clusters`, and never folded into it.
    roster_ticks: Vec<u64>,
}

impl RStats {
    fn seg(&mut self) -> &mut Segment {
        self.segs.last_mut().expect("segment open")
    }

    /// Has ANY segment diverged yet? The gate on `--resync-restarts`
    /// (see [`SegOpen::Restart`]): before the first divergence the
    /// port's restart signal is corroborated — it is running retail's
    /// own state, so it permadeaths where retail permadeathed. AFTER
    /// it the port is wild, and a `take_restart` there is evidence of
    /// nothing at all. Resyncing on it would launder the port's own
    /// noise back into retail's state, which is exactly the purity
    /// `--segmented` buys honestly (it re-anchors at EVERY break, so
    /// its restarts are never load-bearing) and a plain run must not
    /// buy on credit. Measured on mc1l48: ungated, the tail books 8
    /// restart segments against retail's 5.
    fn any_diverged(&self) -> bool {
        self.segs.iter().any(|s| s.horizon.is_some())
    }

    fn open(&mut self, t0: u64, opened_by: SegOpen) {
        self.segs.push(Segment {
            t0,
            end: t0,
            opened_by,
            ..Segment::default()
        });
    }

    /// Fold one graded boundary. `pose` rows are (lane, want, got);
    /// `pd` is the world diff at the boundary. Returns whether the
    /// boundary was CLEAN — `--segmented` re-anchors on a false.
    fn grade(
        &mut self,
        t: u64,
        pose: &[(&'static str, i64, i64)],
        pd: &PairDiff,
        args: &Args,
        dump: bool,
    ) -> bool {
        let seg = self.segs.last_mut().expect("segment open");
        seg.graded += 1;
        let clean = pose.is_empty() && pd.clean();
        if clean {
            seg.clean += 1;
        } else {
            if !pose.is_empty() {
                seg.firsts.entry("pose").or_insert(t);
                seg.pose_rows += pose.len() as u64;
            }
            if pd.rng_want != pd.rng_got {
                seg.firsts.entry("rng").or_insert(t);
                seg.rng_bad += 1;
            }
            if !pd.missing.is_empty() || !pd.extra.is_empty() {
                seg.firsts.entry("entity-set").or_insert(t);
                seg.missing += pd.missing.len() as u64;
                seg.extra += pd.extra.len() as u64;
            }
            if !pd.fields.is_empty() {
                seg.firsts.entry("fields").or_insert(t);
                seg.field_rows += pd.fields.len() as u64;
            }
            if seg.horizon.is_none() {
                seg.horizon = Some(t);
                let mut s = String::new();
                for (name, want, got) in pose.iter().take(args.max_diffs) {
                    let _ = writeln!(s, "    {name}: retail {want} port {got}");
                }
                if !pd.clean() {
                    // The boundary t grades the pair (t-1 → t).
                    let _ = write!(s, "{}", pd.render(t.saturating_sub(1), args.max_diffs));
                }
                seg.first_render = s;
            }
            if dump {
                for (name, want, got) in pose {
                    println!("    {name}: retail {want} port {got}");
                }
                print!("{}", pd.render(t, usize::MAX));
            }
        }
        clean
    }

    fn render(&self, mode: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "   mode: {mode}");
        if let Some(t) = self.truncated {
            let _ = writeln!(
                out,
                "   ⚠ STOPPED at the first divergence (--stop-at-divergence): \
                 boundary {t} is the horizon, every COUNT below is truncated there"
            );
        }
        // THE CERTIFICATION LINE (segmented runs). A take certifies
        // when it free-runs as ONE segment, so the figure that matters
        // is resets the PORT forced — gap resets are the capture's
        // property and can never be driven to zero by fixing the port.
        let devs: Vec<u64> = self
            .segs
            .iter()
            .filter(|s| s.opened_by == SegOpen::Deviation)
            .map(|s| s.t0)
            .collect();
        if !devs.is_empty()
            || self.segs.iter().any(|s| {
                matches!(
                    s.opened_by,
                    SegOpen::Gap | SegOpen::Restart | SegOpen::Roster
                )
            })
        {
            let gaps = self
                .segs
                .iter()
                .filter(|s| s.opened_by == SegOpen::Gap)
                .count();
            let restarts = self
                .segs
                .iter()
                .filter(|s| s.opened_by == SegOpen::Restart)
                .count();
            // The roster-excused count rides BESIDE the certification
            // arithmetic and never inside it: `excess resets` stays the
            // number of resets the PORT forced, which is what a take
            // has to drive to zero. A registered deviation is not one.
            let excused = self.roster_segs();
            let excused_txt = if excused == 0 {
                String::new()
            } else {
                format!(", {excused} roster-excused")
            };
            let _ = writeln!(
                out,
                "   segments: {} total, {} gap-forced, {} restart-forced, {} DEVIATION-forced\
                 {excused_txt} (excess resets: {})",
                self.segs.len(),
                gaps,
                restarts,
                devs.len(),
                devs.len()
            );
            if !devs.is_empty() {
                // Every reset tick is a self-naming fixture candidate,
                // but a wrong law usually fails on a RUN of adjacent
                // ticks (one carcass, one respawn, one clash) — so
                // collapse the runs and let the cluster count, not the
                // reset count, be what the reader triages.
                let mut runs: Vec<(u64, u64)> = Vec::new();
                for &t in &devs {
                    match runs.last_mut() {
                        Some(r) if t <= r.1 + 1 => r.1 = t,
                        _ => runs.push((t, t)),
                    }
                }
                let shown: Vec<String> = runs
                    .iter()
                    .take(24)
                    .map(|(a, b)| {
                        // `--classify` tags the cluster HEAD: LOCAL =
                        // the pair at t-1 is itself dirty (fixture
                        // candidate); INHERITED = the pair is clean,
                        // so the break rides earlier state (unit
                        // test / upstream dig).
                        let tag = match self.class_tags.get(a) {
                            Some(true) => "[LOCAL]",
                            Some(false) => "[INHERITED]",
                            None => "",
                        };
                        if a == b {
                            format!("{a}{tag}")
                        } else {
                            format!("{a}-{b}({}){tag}", b - a + 1)
                        }
                    })
                    .collect();
                let _ = writeln!(
                    out,
                    "   reset clusters (fixture candidates): {} in {} run(s): {}{}",
                    devs.len(),
                    runs.len(),
                    shown.join(", "),
                    if runs.len() > shown.len() {
                        format!(", … (+{} more)", runs.len() - shown.len())
                    } else {
                        String::new()
                    }
                );
                if !self.class_tags.is_empty() {
                    let local = self.class_tags.values().filter(|&&v| v).count();
                    let _ = writeln!(
                        out,
                        "   classified heads: {} LOCAL (pair dirty ⇒ fixture), {} INHERITED \
                         (pair clean ⇒ unit test / upstream)",
                        local,
                        self.class_tags.len() - local
                    );
                }
            }
            // THE ROSTER LEDGER — always visible when a rule fired, and
            // itemised by rule, so a rule that starts excusing far more
            // than its note claims is caught by reading the report
            // rather than by noticing a number that quietly fell.
            if !self.roster_hits.is_empty() {
                let shown: Vec<String> = self
                    .roster_ticks
                    .iter()
                    .take(24)
                    .map(|t| t.to_string())
                    .collect();
                let _ = writeln!(
                    out,
                    "   roster-excused boundaries (registered DEVIATIONS, not defects): {}: {}{}",
                    self.roster_ticks.len(),
                    shown.join(", "),
                    if self.roster_ticks.len() > shown.len() {
                        format!(", … (+{} more)", self.roster_ticks.len() - shown.len())
                    } else {
                        String::new()
                    }
                );
                for (id, (rows, bounds)) in &self.roster_hits {
                    let _ = writeln!(out, "     {id}: {rows} row(s) over {bounds} boundary/ies");
                }
            }
        }
        for (i, seg) in self.segs.iter().enumerate() {
            let _ = writeln!(
                out,
                "   segment {i} [{}]: t={}..{} — {} stepped, {} graded ({} capture-skipped), {} clean",
                match seg.opened_by {
                    SegOpen::Seed => "seed",
                    SegOpen::Gap => "gap",
                    SegOpen::Deviation => "reset",
                    SegOpen::Roster => "roster",
                    SegOpen::Restart => "restart",
                },
                seg.t0,
                seg.end,
                seg.stepped,
                seg.graded,
                seg.ungraded,
                seg.clean
            );
            if i == 0 && self.paused > 0 {
                let _ = writeln!(
                    out,
                    "     PAUSED: {} boundaries reproduced as `tick_paused` (one LCG \
                     draw, frozen pool) — retail's P, not a torn capture",
                    self.paused
                );
            }
            match seg.horizon {
                Some(h) => {
                    let _ = writeln!(
                        out,
                        "     BIT-EXACT HORIZON: {} boundaries (t={}..{})",
                        h.saturating_sub(seg.t0 + 1),
                        seg.t0 + 1,
                        h
                    );
                    let firsts: Vec<String> = seg
                        .firsts
                        .iter()
                        .map(|(k, t)| format!("{k} t={t}"))
                        .collect();
                    let _ = writeln!(out, "     channel firsts: {}", firsts.join(", "));
                    let _ = writeln!(out, "     first divergence (t={h}):");
                    let _ = write!(out, "{}", seg.first_render);
                    let _ = writeln!(
                        out,
                        "     post-divergence traffic (NOT a defect count): pose {} rows, \
                         rng {}/{} boundaries, sets {}/{} \
                         missing/extra, fields {} rows",
                        seg.pose_rows,
                        seg.rng_bad,
                        seg.graded,
                        seg.missing,
                        seg.extra,
                        seg.field_rows
                    );
                }
                None => {
                    let _ = writeln!(out, "     BIT-EXACT to the segment end — zero divergence");
                }
            }
        }
        if !self.gates.is_empty() {
            let gates: Vec<String> = self.gates.iter().map(|(k, v)| format!("{k} {v}")).collect();
            let _ = writeln!(out, "   pose-only reseeds: {}", gates.join(", "));
        }
        if self.stick_unrec > 0 {
            let _ = writeln!(
                out,
                "   stick-unrecoverable pairs (centered stick fed): {}",
                self.stick_unrec
            );
        }
        if self.crank_repass > 0 {
            let _ = writeln!(
                out,
                "   whirlwind crank re-recoveries (trial-step count or dark witness): {}",
                self.crank_repass
            );
        }
        let _ = writeln!(
            out,
            "   input events: {} respawn(s), {} suicide(s), {} equip/rebind(s){}",
            self.respawns,
            self.suicides,
            self.equips,
            if self.rebind_dropped > 0 {
                format!(
                    ", {} rebind(s) DROPPED (both hands in one pair)",
                    self.rebind_dropped
                )
            } else {
                String::new()
            }
        );
        if !self.cheats.is_empty() || !self.cheats_unimpl.is_empty() {
            let fmt = |m: &BTreeMap<&'static str, u64>| {
                m.iter()
                    .map(|(k, v)| format!("{v}x {k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(out, "   retail cheats replayed: {}", fmt(&self.cheats));
            if !self.cheats_unimpl.is_empty() {
                let _ = writeln!(
                    out,
                    "   ⚠ cheats with NO port handler (divergence from that tick on): {}",
                    fmt(&self.cheats_unimpl)
                );
            }
        }
        out
    }

    /// The earliest divergent boundary of the whole run, or `None`
    /// while every segment is still bit-exact. `MGC_PACE_TRACE` reads
    /// it to say which of its keys are actually GRADED.
    fn first_horizon(&self) -> Option<u64> {
        self.segs.iter().filter_map(|s| s.horizon).min()
    }

    /// Was segment `i`'s horizon a ROSTER-EXCUSED boundary the run
    /// then RE-ANCHORED on? Both halves matter: excusing a row is a
    /// statement about blame, and re-anchoring is what makes the
    /// measurement past it mean anything. A plain free run that
    /// declines to resync (no `--resync-deviations`) keeps its horizon
    /// — the deviating value has propagated and nothing downstream is
    /// the port's own state any more.
    fn excused_at(&self, i: usize) -> bool {
        self.segs[i].horizon.is_some()
            && self.segs.get(i + 1).map(|n| n.opened_by) == Some(SegOpen::Roster)
    }

    fn roster_segs(&self) -> usize {
        self.segs
            .iter()
            .filter(|s| s.opened_by == SegOpen::Roster)
            .count()
    }

    fn clean(&self) -> bool {
        (0..self.segs.len()).all(|i| self.segs[i].horizon.is_none() || self.excused_at(i))
    }

    /// `--brief` — ONE machine-readable line per take: the corpus
    /// regression sweep that used to be hand-rolled shell loops.
    /// `horizon` = the last bit-exact boundary before the take's
    /// first divergence (`END` when nothing diverged), `first` = the
    /// divergence tick itself, `sig` its compact signature. A whole
    /// corpus's `--brief` output diffs against a saved baseline.
    fn render_brief(&self, take: &str, mode: &str, terrain: &str) -> String {
        let gaps = self
            .segs
            .iter()
            .filter(|s| s.opened_by == SegOpen::Gap)
            .count();
        let devs = self
            .segs
            .iter()
            .filter(|s| s.opened_by == SegOpen::Deviation)
            .count();
        let graded: u64 = self.segs.iter().map(|s| s.graded).sum();
        let clean: u64 = self.segs.iter().map(|s| s.clean).sum();
        // ⭐ `horizon`/`first` are the first UNEXCUSED divergence — a
        // boundary the roster excused AND the run re-anchored on is not
        // one ([`RStats::excused_at`]). That is the ruling in one
        // field: a take whose only breaks are registered deviations
        // reads `horizon=END`, which is what "certified except the
        // registered rows" has to look like on the sweep line. Nothing
        // is hidden — `roster=N` says how many were excused, and the
        // full report itemises them by rule and by tick.
        // Byte-stable for every take with no excused boundary, since
        // `excused_at` is false throughout one.
        let first = (0..self.segs.len())
            .filter(|&i| !self.excused_at(i))
            .filter_map(|i| self.segs[i].horizon)
            .min();
        let sig = first
            .and_then(|t| self.segs.iter().find(|s| s.horizon == Some(t)))
            .map(|s| s.sig.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into());
        let end = self.segs.last().map_or(0, |s| s.end);
        let tags = if self.class_tags.is_empty() {
            String::new()
        } else {
            let local = self.class_tags.values().filter(|&&v| v).count();
            format!(" local={local} inherited={}", self.class_tags.len() - local)
        };
        // `paused` is reported BESIDE `graded`, never folded into it:
        // a paused boundary is reproduced (`tick_paused`) but grades
        // nothing, so counting it either way would lie. Only a take
        // whose player pressed P carries a non-zero value.
        let paused = if self.paused == 0 {
            String::new()
        } else {
            format!(" paused={}", self.paused)
        };
        // Conditional like `paused` so the certified corpus's baseline
        // lines are byte-stable: only a take with in-band level
        // restarts (permadeath) carries the field.
        let restarts = self
            .segs
            .iter()
            .filter(|s| s.opened_by == SegOpen::Restart)
            .count();
        let restarts = if restarts == 0 {
            String::new()
        } else {
            format!(" restarts={restarts}")
        };
        // A deviation-forced re-anchor on a PRISTINE take restores
        // un-dug planes (restore_planes), so from the first reset in
        // excavated territory the sweep degenerates to one reset per
        // tick — the count measures the capture, not the port
        // (session 66: mc1l32-terrainless "31,178 segments" was this;
        // the non-segmented instrument still read 16/15/0). Tag the
        // line so a baseline diff can never book the artifact as a
        // regression.
        let artifact = if terrain == "pristine" && devs > 0 {
            " ⚠pristine-reset-artifact"
        } else {
            ""
        };
        let stopped = match self.truncated {
            None => String::new(),
            Some(t) => format!(" stopped={t}"),
        };
        // Conditional like `paused` and `restarts`: a take with no
        // registered-deviation boundary carries no field at all, so
        // every baseline line in the corpus stays byte-stable.
        let roster = self.roster_ticks.len();
        let roster = if roster == 0 {
            String::new()
        } else {
            format!(" roster={roster}")
        };
        format!(
            "BRIEF {take} mode={mode} terrain={terrain}{stopped} end={end} segments={} gaps={gaps}{restarts}{roster} \
             devs={devs} graded={graded}{paused} clean={clean} horizon={} first={} sig={sig}{tags}{artifact}\n",
            self.segs.len(),
            first.map_or_else(|| "END".to_string(), |t| t.saturating_sub(1).to_string()),
            first.map_or_else(|| "-".to_string(), |t| t.to_string()),
        )
    }

    /// Fold one stepped pose-only boundary (tier-2 has no world diff)
    /// and emit its CSV rows.
    fn fold_pose_only(
        &mut self,
        t: u64,
        pose: &[(&'static str, i64, i64)],
        csv: &mut Option<std::io::BufWriter<std::fs::File>>,
    ) -> Result<(), String> {
        let seg = self.seg();
        seg.stepped += 1;
        seg.graded += 1;
        if pose.is_empty() {
            seg.clean += 1;
            return Ok(());
        }
        seg.pose_rows += pose.len() as u64;
        seg.firsts.entry("pose").or_insert(t);
        if seg.horizon.is_none() {
            seg.horizon = Some(t);
            let mut s = String::new();
            for (name, want, got) in pose {
                let _ = writeln!(s, "    {name}: retail {want} port {got}");
            }
            seg.first_render = s;
            let names: Vec<&str> = pose.iter().map(|(n, ..)| *n).take(4).collect();
            seg.sig = format!("pose:{}", names.join(","));
        }
        emit_replay_csv(csv, t.saturating_sub(1), pose, &PairDiff::default())
    }
}

// Pose lanes (the chained carpet vs the recorded pose at a graded
// boundary) live in the shared seeding home — mgc_sim conformance
// `pose_lanes_mc1`/`pose_lanes_mc2`.

// --------------------------------------------------------------- MC1 run

fn run_mc1(
    path: &std::path::Path,
    args: &Args,
    port_dump: Option<&PortDump>,
) -> Result<bool, String> {
    let mut rec = Recording::open(path)?;
    let game = rec.header.game.clone();
    let level = rec.header.level.ok_or("recording has no level number")?;
    if !args.brief {
        println!(
            "== replay {} (game {game}, level {level}{})",
            path.display(),
            if args.pose_only { ", pose-only" } else { "" }
        );
    }
    let (mut world, pristine) = crate::verify::build_world(&args.baked, &game, level)?;
    // The known-deviation roster, the same file and the same `--no-roster`
    // switch `verify-deltas` uses ([`roster_excuse`]).
    let roster = crate::verify::load_roster(args)?;
    let take = crate::verify::take_stem(path);
    let mut csv = open_csv(args)?;
    let mut shadow = crate::shadow::Shadow::from_env()?;
    let state_dump: Option<(u64, String)> = std::env::var("MGC_STATE_DUMP").ok().and_then(|s| {
        let (t, path) = s.split_once(':')?;
        Some((t.parse().ok()?, path.to_string()))
    });
    // `MGC_STATE_DUMP=<t>:<path>` writes the sectioned whole-world dump
    // once, at the first tick at or after `t` — an ANCHOR tick counts,
    // so a run seeded at `t` dumps retail's own imported state and a
    // run that walked there dumps the port's. Diffing the two is how a
    // free-run break gets attributed when the entity pool, the free
    // list and every graded field are already bit-identical.
    let tear_trace: Option<(u64, u64)> = std::env::var("MGC_TEAR_TRACE").ok().and_then(|s| {
        let (a, b) = s.split_once(':')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    });
    let mut state_dumped = false;
    let mut dump_state = |world: &World, t: u64| -> Result<(), String> {
        let Some(spec) = state_dump.as_ref() else {
            return Ok(());
        };
        if t < spec.0 || state_dumped {
            return Ok(());
        }
        state_dumped = true;
        println!("  STATE DUMP at t={t} -> {}", spec.1);
        let mut out = String::new();
        for (name, bytes) in world.debug_state_sections() {
            let _ = write!(out, "{name}\t{}\t", bytes.len());
            for b in &bytes {
                let _ = write!(out, "{b:02x}");
            }
            out.push('\n');
        }
        std::fs::write(&spec.1, out).map_err(|e| format!("state dump: {e}"))
    };
    let mut timg = (!args.no_terrain)
        .then(|| {
            rec.header
                .channels
                .terrain
                .as_ref()
                .map(mgc_formats::mgcr::TerrainImage::new)
        })
        .flatten();
    // `--start <t>`: every record before t is skipped WITHOUT decoding
    // — only its terrain delta is folded, which is exactly what the
    // loop below did with them (docs/PERF-CONFORM.md).
    if !args.brief {
        crate::slice_banner(&rec);
    }
    if let Some(s) = args.start {
        crate::late_tick_hint(&rec, path, s);
        rec.skip_to(s, timg.as_mut())?;
    }

    let mut stats = RStats::default();
    let mut st_prev: Option<(u64, RetailMc1)> = None;
    let mut chain: Option<(Chain, u16)> = None; // (flight chain, human slot)
    // `MGC_OVERRUN=<n>`: after the last record, re-anchor the port ON
    // that record and keep stepping n ticks with the last recovered
    // input, one heartbeat line per tick — the instrument for a take
    // whose retail side FROZE at its end (mc1l26-froze, mc1l49): does
    // the port, holding retail's exact final state, hang the same way?
    let overrun: Option<u64> = std::env::var("MGC_OVERRUN").ok().and_then(|v| v.parse().ok());
    let mut overrun_input: Option<(Mc1Input, PlayerCommand)> = None;
    let mut printed_import = false;
    // MGC_CASTLE_TRACE=<t0>:<t1> — the replay-mode castle-story probe:
    // at every boundary in range, print the retail (3,2) rows beside
    // the port's live castles — f70 / case machine / f50 shake / level
    // / life / ch0 mail — the free-run state-drift microscope the
    // pair-mode probes can't see.
    let ctrace = std::env::var("MGC_CASTLE_TRACE").ok().and_then(|v| {
        let (a, b) = v.split_once(':')?;
        Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
    });
    // MGC_MOB_TRACE=<slot>[;<slot>…]:<t0>:<t1> — the creature-machine
    // microscope: retail's state byte / pack link / phase clock beside
    // the port's live ones. The obs schema grades neither +70 nor +52,
    // so a creature that entered the WRONG STATE reports only as the
    // downstream yaw and rand rows — this is how the state itself is
    // read.
    let mtrace = std::env::var("MGC_MOB_TRACE").ok().and_then(|v| {
        let (slots, ts) = v.split_once(':')?;
        let (a, b) = ts.split_once(':')?;
        let slots: Vec<usize> = slots.split(';').filter_map(|s| s.parse().ok()).collect();
        Some((slots, a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
    });
    // MGC_MANA_TRACE=<t0>:<t1> — THE MANA-LEDGER microscope. The
    // delta register +132 is UNGRADED by the obs schema, so a
    // divergent regen accumulator drifts invisibly and only
    // materializes ticks later as a graded +140 row (mc1l42's free
    // run: clean through t=349, wrong at 350). Prints retail's
    // +132/+136/+140 for the human carpet beside the port's pool, plus
    // every live class-12 burst counter (+48 → f26) on both sides —
    // the pin's own clock, since `sub_55E80`'s mid-burst arm is what
    // zeroes the delta.
    let manatrace = std::env::var("MGC_MANA_TRACE").ok().and_then(|v| {
        let (a, b) = v.split_once(':')?;
        Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
    });
    // MGC_SITE_TRACE=<x>,<y>:<t0>:<t1> — the site-roster companion:
    // every non-castle entity within 8 tiles of the site, both sides,
    // compact — the crush/effect-lifetime microscope.
    let strace = std::env::var("MGC_SITE_TRACE").ok().and_then(|v| {
        let (xy, ts) = v.split_once(':')?;
        let (x, y) = xy.split_once(',')?;
        let (a, b) = ts.split_once(':')?;
        Some((
            x.parse::<f64>().ok()?,
            y.parse::<f64>().ok()?,
            a.parse::<u64>().ok()?,
            b.parse::<u64>().ok()?,
        ))
    });
    let mut celltrace = CellTrace::from_env();
    let mut pcensus = PlaneCensus::from_env();
    let mut ktrace = KnockTrace::from_env();
    // `--segmented`: the boundary grade sets this, and the re-anchor
    // runs after the tick body so the break's own diagnostics (traces,
    // CSV) still see the DIVERGED state that produced them.
    let mut reset_at: Option<(u64, SegOpen)> = None;
    // The PORT's own restart signal (`take_restart`): the tick the
    // castle-less respawn fired. Retail's reload lands 1-2 boundaries
    // later (the reload frame is capture-skipped), so any boundary in
    // the short window behind the signal is the SEAM — re-anchored as
    // `SegOpen::Restart`, never graded (see the enum doc).
    let mut restart_at: Option<u64> = None;
    // ---- `--classify` state (the segmented-residue doctrine run
    // inline): a SCRATCH world for the pair check (never the free-run
    // world — the pair import would wipe the state under
    // measurement), the measured planes AS OF t-1 (the image below
    // tracks t), and the verify-style pair commands (fire = dw_0@N,
    // prev pair's command feeds the fire edge).
    let mut classify_world: Option<World> = None;
    #[allow(clippy::type_complexity)]
    let mut prev_measured: Option<(Vec<u8>, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)> = None;
    let mut pair_cmd_prev = PlayerCommand::default();
    let mut last_dev: Option<u64> = None;
    // `--stop-at-divergence`: armed at the boundary that broke, read at
    // the END of that tick's body so the break's own bookkeeping (and
    // its diagnostics) still run exactly as they do in a full sweep.
    let mut stop_at: Option<u64> = None;
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        // The terrain image tracks the take continuously (self-healing
        // deltas) — installed into the world only at anchors
        // (world mode) or per pair (pose-only, terrain@N+1).
        // `--classify` keeps the PRE-apply planes: the pair check at
        // boundary t must run on terrain@t-1, and after this apply
        // the image holds t.
        if args.classify
            && args.segmented
            && tick.terrain.is_some()
            && let Some((h, ty, ceil, an)) = measured_planes(&timg)
        {
            prev_measured = Some((
                h.to_vec(),
                ty.to_vec(),
                ceil.map(|c| c.to_vec()),
                an.map(|a| a.to_vec()),
            ));
        }
        if let (Some(img), Some(block)) = (timg.as_mut(), &tick.terrain) {
            img.dump_delta(block, tick.t);
            img.apply(block)
                .map_err(|e| format!("t={}: terrain: {e}", tick.t))?;
        }
        let Some(state) = &tick.state else {
            st_prev = None;
            continue;
        };
        if args.start.is_some_and(|s| tick.t < s) {
            continue;
        }
        let st = decode_retail_mc1(state)?;
        let obs: ObsMc1 = match &tick.obs {
            Some(v) => serde_json::from_str(v.get()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        let anchor = !matches!((&st_prev, &chain), (Some((pt, _)), Some(_)) if tick.t == pt + 1);
        if anchor {
            // The ONLY moments recording state touches the sim: the
            // take's seed, and a capture gap (never a deviation —
            // that reset lives at the boundary grade, below).
            let (ch, human_slot, active) = anchor_mc1(&mut world, &pristine, &timg, &st, tick.t)?;
            chain = Some((ch, human_slot));
            stats.open(
                tick.t,
                if stats.segs.is_empty() {
                    SegOpen::Seed
                } else {
                    SegOpen::Gap
                },
            );
            if !printed_import {
                printed_import = true;
                let measured = measured_planes(&timg).is_some();
                if !args.brief {
                    println!(
                        "   import: {active} active entities, human slot {human_slot}, terrain {}",
                        if measured { "MEASURED" } else { "pristine" }
                    );
                }
                // ⚠ A re-anchor restores the ENTITY state from the
                // recording but the TERRAIN from the measured channel —
                // and a format-1 take has none, so the reset drops the
                // port's own terraforming back to pristine planes and
                // the next tick breaks on ground it no longer shares.
                // Excess resets are then a property of the CAPTURE, and
                // the count means nothing. Such a take wants a v2
                // re-record, not a dig.
                if args.segmented && !measured && !args.brief {
                    println!(
                        "   ⚠ --segmented WITHOUT a measured terrain channel: every reset \
                         restores PRISTINE planes, so the reset count is a capture artifact, \
                         not a port score (re-record this take with terrain)"
                    );
                }
            }
            dump_state(&world, tick.t)?;
            // A `--port` dump landing ON an anchor: the port state IS
            // the retail import — still printed (identity modulo
            // representation is itself a useful calibration), with
            // the caveat named.
            if let Some(spec) = port_dump
                && tick.t >= spec.t
            {
                render_port_dump(&world, &st, human_slot, spec, tick.t, true);
                return Ok(true);
            }
            // The verify-command law at an anchor: the previous
            // pair's consumed fire is unknowable, so seed it from the
            // anchor's own consumed byte — the same approximation
            // `anchor_mc1`'s `set_prev_fire` already applies.
            pair_cmd_prev = fire_bits_mc1(&st);
            st_prev = Some((tick.t, st));
            continue;
        }
        let (pt, pst) = st_prev.take().expect("anchored");
        let (ch, human_slot) = chain.as_mut().expect("anchored");
        let slot = *human_slot;
        let cw = &st.wizards[st.local_player as usize];

        // ---- input for the pair pt → t, all from the recording
        // (the shared recovery laws — mgc_formats::recover) ----
        let rec = recover::recover_pair_mc1(&pst, &st, tick.input.as_ref());
        let stick_ok = rec.stick_ok();
        if !stick_ok {
            stats.stick_unrec += 1;
        }
        let inp = mc1_mover_input(rec.move_byte, rec.stick());
        if rec.equip_left.is_some() || rec.equip_right.is_some() {
            stats.equips += 1;
        }
        if rec.respawn {
            stats.respawns += 1;
        }
        if rec.suicide {
            stats.suicides += 1;
        }
        let cmd = PlayerCommand {
            fire_left: rec.fire_left,
            fire_right: rec.fire_right,
            equip_left: rec.equip_left.map(SpellId),
            equip_right: rec.equip_right.map(SpellId),
            respawn: rec.respawn,
            suicide: rec.suicide,
            demolish: rec.demolish,
            cheat: rec.cheat,
            ..PlayerCommand::default()
        };
        // The dw==48 strafe-freeze emulation (law in RecoveredPair):
        // pre-feed one decay quantum, the mover's decay lands back on
        // the frozen value.
        if rec.mc1_strafe_freeze() && ch.s.strafe != 0 {
            ch.s.strafe += 4 * ch.s.strafe.signum();
        }
        overrun_input = Some((inp, cmd));

        if args.pose_only {
            // Tier-2: fresh retail world context at N, chained flight.
            pose_only_pair_mc1(
                &mut world, &pristine, &timg, &pst, &st, ch, slot, pt, inp, stick_ok, &mut stats,
                &mut csv,
            )?;
        } else {
            // MGC_KNOCK_TRACE=<t0>:<t1> — the knock PHASE probe. Prints
            // what the mover consumed this tick vs what the world tick
            // armed for the next, beside retail's recorded +22/+24.
            ktrace.arm(&world);
            mgc_sim::DEBUG_TICK.store(tick.t, std::sync::atomic::Ordering::Relaxed);
            // `--port --at-slot <n>`: arm the mid-walk pool snapshot
            // for the tick INTO the dump boundary.
            if let Some(spec) = port_dump
                && tick.t == spec.t
                && let Some(n) = spec.at_slot
            {
                world.arm_walk_probe(n);
            }
            book_cheat(&world, rec.cheat, &mut stats);
            // ⭐⭐⭐ A PAUSED TURN IS ITS OWN STEP, NOT A SKIPPED ONE
            // AND NOT A FULL ONE. Retail draws the global LCG and
            // returns (`sub_41780_41AC0` :52197 — draw first, pause
            // test second), so the faithful reproduction is
            // `tick_paused`: no mover, no walk, one draw. Running the
            // FULL step here was doubly wrong — it advanced a world
            // retail had frozen AND consumed a different number of
            // draws — and it is why mc1l6's opening pause read as a
            // 1,602-tick "horizon" that graded nothing and left the
            // port 66 phase steps ahead by t=1603.
            //
            // The chain (`ch`) is deliberately untouched: retail's
            // paused frame runs no mover, so the pose must not move
            // either.
            // The HUD-drawn gate is a presentation INPUT (the map key,
            // the level fly-in) the recovery cannot derive from the
            // key stream: re-seed it per pair from the record's view
            // byte, the way the pair importer does. The frame clock
            // itself free-runs (one frame per tick at the default
            // game speed).
            world.set_mc1_hud_drawn(matches!(
                pst.wizards[pst.local_player as usize].view,
                0 | 3
            ));
            // THE SCRATCH LANE's port half (round 160). Sampled
            // BEFORE the step so the interval matches retail's
            // `pst` -> `st` exactly one tick, which is what makes the
            // draw count comparable in a free run: the port has been
            // carrying slot 0's `+4` since the anchor, so the VALUES
            // drift, but the per-tick distance does not.
            let port_scratch_pre = world.mc1_scratch_rand();
            if recover::paused_turn_mc1(&pst, &obs) {
                // ⭐⭐ THE PAUSE SCREEN IS INTERACTIVE. A paused turn
                // runs no sim, but the BIG MAP / spellbook is still
                // live under it and a click there re-equips: mc1l6
                // t=1579 is a bare left-mouse press — `keys_down` is
                // EMPTY, the Enter that opened the map was 31 ticks
                // earlier at t=1546-48 — and retail's `hand_left`
                // goes 0 -> 2 (Accelerate) on exactly that tick, then
                // survives the unpause at 1603. Dropping input on a
                // paused boundary lost it, and the whole 39,754-tick
                // take then differed by one lane forever.
                //
                // The equip needs no mouse model: recovery replays
                // the recorded hand change itself
                // (`recover.rs`: `equip(pw.hand_left, cw.hand_left)`),
                // so applying it here is exactly the app's own
                // `flush_equip_if_paused`. Everything else the pause
                // screen can do is still unmodelled — this is the arm
                // the corpus witnesses.
                world.tick_paused();
                if rec.equip_left.is_some() || rec.equip_right.is_some() {
                    world.equip_hands(rec.equip_left.map(SpellId), rec.equip_right.map(SpellId));
                }
                stats.paused += 1;
            } else {
                step_mc1(&mut world, ch, inp, cmd);
            }
            if world.take_restart() {
                restart_at = Some(tick.t);
            }
            if let Some(spec) = port_dump
                && tick.t >= spec.t
            {
                render_port_dump(&world, &st, slot, spec, tick.t, false);
                return Ok(true);
            }
            let kw = &pst.wizards[pst.local_player as usize];
            ktrace.emit(
                &world,
                pt,
                tick.t,
                (kw.knock_dir, kw.knock_mag),
                (cw.knock_dir, cw.knock_mag),
            );
            stats.seg().stepped += 1;
            if let Ok(v) = std::env::var("MGC_PLANE_DIFF") {
                if let Some((a, b)) = v.split_once(':')
                    && let (Ok(t0), Ok(t1)) = (a.parse::<u64>(), b.parse::<u64>())
                    && tick.t >= t0
                    && tick.t <= t1
                    && let Some(img) = timg.as_ref()
                    && let Some(th) = img.plane("height")
                {
                    let ph = &world.planes().height;
                    let mut n = 0usize;
                    let mut first = Vec::new();
                    for i in 0..th.len().min(ph.len()) {
                        if th[i] != ph[i] {
                            n += 1;
                            if first.len() < 4000 {
                                first.push((i & 0xFF, i >> 8, ph[i], th[i]));
                            }
                        }
                    }
                    println!("PLANEDIFF t={} hdiff={n} {first:?}", tick.t);
                }
            }
            celltrace.emit(&world, &timg, tick.t);
            pcensus.emit(&world, &timg, tick.t);
            if let Some((t0, t1)) = ctrace
                && tick.t >= t0
                && tick.t <= t1
            {
                for (s, re) in st.ents.iter().enumerate() {
                    if re.class64 == 3 && re.model65 == 2 {
                        println!(
                            "CASTLE t={} retail slot {s} own={} f70={} f48={} f50={} lvl={} \
                             life={} mail={:?} flags={:#x} at ({:.1},{:.1})",
                            tick.t,
                            re.id24,
                            re.f70,
                            re.f48,
                            re.f50,
                            re.f26,
                            re.act_life,
                            re.mail,
                            re.flags,
                            re.x as f64 / 256.0,
                            re.y as f64 / 256.0
                        );
                    }
                }
                let (_, ev) = world.debug_pool();
                for d in ev.iter().filter(|d| d.class == 3 && d.model == 2) {
                    let (t70, f59, f50, f26, life, flags) =
                        world.debug_castle_machine(d.slot).expect("live slot");
                    println!(
                        "CASTLE t={}   port slot {} own={} f70={} f59={} f50={} lvl={} \
                         life={} mail={:?} flags={:#x} at ({},{})",
                        tick.t,
                        d.slot,
                        d.id24,
                        t70,
                        f59,
                        f50,
                        f26,
                        life,
                        world.debug_mail(d.slot),
                        flags,
                        d.tx,
                        d.ty
                    );
                }
            }
            if let Some((slots, t0, t1)) = mtrace.as_ref()
                && tick.t >= *t0
                && tick.t <= *t1
            {
                for &s in slots {
                    let re = &st.ents[s];
                    let p = world.debug_mob_machine(s);
                    let pf = p.map_or_else(
                        || "  port <none>".to_string(),
                        |(t70, f52, f63, f34, f146, f126, rand, f71)| {
                            format!(
                                "  port f70={t70} f52={f52} f63={f63} f34={f34} \
                                 f146={f146} f126={f126} rand={rand} f71={f71}"
                            )
                        },
                    );
                    println!(
                        "MOB t={} slot {s} ({},{}) retail f70={} f52={} f63={} f34={} \
                         f146={} f126={} rand={}\nMOB t={} slot {s}{pf}",
                        tick.t,
                        re.class64,
                        re.model65,
                        re.f70,
                        re.f52,
                        re.f63,
                        re.f34,
                        re.f146,
                        re.f126,
                        re.rand,
                        tick.t,
                    );
                }
            }
            if let Some((t0, t1)) = manatrace
                && tick.t >= t0
                && tick.t <= t1
            {
                let rw = &st.ents[slot as usize];
                let pin = PinnedMc1 {
                    slot,
                    local: pst.local_player,
                    player_count: pst.player_count,
                    pose: ch.pose(),
                };
                let pp = world.obs_project_mc1(&pin);
                let pmana = pp.player.as_ref().map_or((0, 0), |p| (p.mana, p.mana_max));
                let mut rtok = String::new();
                for (s, re) in st.ents.iter().enumerate() {
                    if re.class64 == 12
                        && (re.f48 != 0
                            || re.f26 != 0
                            || std::env::var_os("MGC_MANA_TRACE_ALL").is_some())
                    {
                        let _ = write!(
                            rtok,
                            " [{s}]m{}+48={}+50={}+26={}",
                            re.model65, re.f48, re.f50, re.f26
                        );
                    }
                }
                let mut ptok = String::new();
                let (_, ev) = world.debug_pool();
                for d in ev.iter().filter(|d| d.class == 12 && d.f26 != 0) {
                    let _ = write!(ptok, " [{}]m{}f26={}", d.slot, d.model, d.f26);
                }
                println!(
                    "MANA t={} retail life={} +132={} +136={} +140={} dw0={} chg={} tok:{}\n\
                     MANA t={}   port                    +136={} +140={} tok:{}",
                    tick.t,
                    rw.act_life,
                    rw.f132,
                    rw.f136,
                    rw.f140,
                    cw.move_bits,
                    cw.charge,
                    rtok,
                    tick.t,
                    pmana.1,
                    pmana.0,
                    ptok
                );
            }
            if let Some((sx, sy, t0, t1)) = strace
                && tick.t >= t0
                && tick.t <= t1
            {
                let mut line = format!("SITE t={} retail:", tick.t);
                for (s, re) in st.ents.iter().enumerate() {
                    let (ex, ey) = (re.x as f64 / 256.0, re.y as f64 / 256.0);
                    if re.class64 != 0
                        && !(re.class64 == 3 && re.model65 == 2)
                        && (ex - sx).abs() < 8.0
                        && (ey - sy).abs() < 8.0
                    {
                        if re.class64 == 10 && re.model65 == 39 {
                            let _ = write!(
                                line,
                                " [{s}]BALL L{} o{} m{} @({ex:.2},{ey:.2},{})",
                                re.act_life, re.f144, re.f140, re.z
                            );
                        } else {
                            let _ = write!(
                                line,
                                " [{s}]({},{})L{}f26={}f70={}",
                                re.class64, re.model65, re.act_life, re.f26, re.f70
                            );
                        }
                    }
                }
                println!("{line}");
                let (_, ev) = world.debug_pool();
                let mut line = format!("SITE t={}   port:", tick.t);
                for d in &ev {
                    if (d.class == 3 && d.model == 2)
                        || ((d.tx as f64) - sx).abs() >= 8.0
                        || ((d.ty as f64) - sy).abs() >= 8.0
                    {
                        continue;
                    }
                    if d.class == 10 && d.model == 39 {
                        let l = world.debug_launch(d.slot).expect("live");
                        let _ = write!(
                            line,
                            " [{}]BALL L{} o{} m{} @({:.2},{:.2},{})",
                            d.slot,
                            d.life,
                            d.owner,
                            d.cargo,
                            l.0 as f64 / 256.0,
                            l.1 as f64 / 256.0,
                            l.2
                        );
                    } else {
                        let _ = write!(
                            line,
                            " [{}]({},{})L{}f26={}f70={}",
                            d.slot, d.class, d.model, d.life, d.f26, d.state
                        );
                    }
                }
                println!("{line}");
            }
            // `MGC_TEAR_TRACE=<t0>:<t1>` — WHY a boundary is called
            // torn. `capture_clean_mc1` is a HEURISTIC (a `+63` step
            // census plus the one-step LCG test), not a record of
            // missing data: a gapless recording can still be declared
            // ungradeable. This splits the verdict into its two
            // clauses and names the suspects, so a false tear can be
            // told from a real one.
            if let Some((t0, t1)) = tear_trace {
                if tick.t >= t0 && tick.t <= t1 {
                    let mut suspects: Vec<(u16, u8, u8, u8, u8, bool)> = Vec::new();
                    for re in &obs.entities {
                        let prev = &pst.ents[re.slot as usize];
                        if prev.class64 == 0 || prev.class64 != re.class || prev.model65 != re.model
                        {
                            continue;
                        }
                        if matches!(re.tick_byte.wrapping_sub(prev.f63), 0 | 2) {
                            // A slot REAPED AND RE-MINTED as the same
                            // (class, model) is a different entity, and
                            // its `+63` is the fresh alloc value — the
                            // per-entity LCG says which.
                            suspects.push((
                                re.slot,
                                re.class,
                                re.model,
                                prev.f63,
                                re.tick_byte,
                                re.rand != prev.rand,
                            ));
                        }
                    }
                    let lcg_ok = pst.rand.wrapping_mul(9377).wrapping_add(9439) == obs.rng;
                    let reminted = suspects.iter().filter(|s| s.5).count();
                    println!(
                        "  TEAR t={} verdict={} suspects={} (re-minted {}) lcg_one_step={} {:?}",
                        tick.t,
                        if capture_clean(&pst, &obs) {
                            "GRADED"
                        } else {
                            "TORN"
                        },
                        suspects.len(),
                        reminted,
                        lcg_ok,
                        &suspects[..suspects.len().min(6)]
                    );
                }
            }
            // Grade at the boundary (capture-clean pairs only — a torn
            // snapshot grades nothing, the chain runs on regardless).
            if (args.segmented || (args.resync_restarts && !stats.any_diverged()))
                && restart_at.is_some_and(|a| tick.t > a && tick.t - a <= 4)
            {
                // THE RELOAD BOUNDARY (Seam B): the port signalled the
                // restart 1-2 boundaries ago and retail has now
                // rebuilt the level from inputs outside the recording
                // (its own reseed over its own heap residue; MC2's
                // pre-capture disk checkpoint) — nothing is gradable.
                // Re-anchor like a gap, tagged `restart`. Segmented,
                // or a plain run that asked with `--resync-restarts`:
                // by default the plain run keeps its "never correct"
                // purity (a WILD post-horizon run can trip
                // `take_restart` on state retail never held — the
                // mc1hwl0 guard row must not re-anchor on it).
                stats.seg().ungraded += 1;
                reset_at = Some((tick.t, SegOpen::Restart));
            } else if capture_clean(&pst, &obs) {
                let pose = pose_lanes_mc1(&ch.s, &st.ents[slot as usize], cw);
                pose_window_mc1(tick.t, &ch.s, &st.ents[slot as usize], cw);
                let pin = PinnedMc1 {
                    slot,
                    local: pst.local_player,
                    player_count: pst.player_count,
                    pose: ch.pose(),
                };
                let port = world.obs_project_mc1(&pin);
                // THE RAW SHADOW IN A FREE RUN. Pair mode's copy of
                // this catches a one-tick WRITE bug; here the port has
                // been carrying its own state since the anchor, so the
                // first tick a lane parts is the first tick the port's
                // HISTORY parts from retail's — the only instrument
                // that can explain a `--segmented` break whose pair
                // diff at the same tick is CLEAN.
                if let Some(sh) = shadow.as_mut() {
                    sh.compare_ents_mc1(&world, &st, slot, tick.t);
                    sh.compare_wiz_mc1(&world, &st, tick.t);
                    sh.compare_globals_mc1(&world, &st, tick.t);
                    sh.compare_chains_mc1(&world, &st, slot, tick.t);
                    sh.compare_free_mc1(&world, &st, slot, tick.t);
                }
                let mut pd = compare(&obs, &port, slot);
                append_hand_diffs(&mut pd, &st, &port, pst.local_player as usize);
                // The SPRITE lane grades the free run too: the port
                // carries its own type86 history from the anchor, so
                // a wrong row here is the recolor bug class showing
                // its face — exactly what the lane exists to catch.
                append_sprite_diffs(&mut pd, &st, &world, slot);
                crate::verify::append_scratch_diffs(
                    &mut pd,
                    pst.ents[0].rand,
                    st.ents[0].rand,
                    port_scratch_pre,
                    world.mc1_scratch_rand(),
                );
                let dump = args.dump == Some(pt)
                    || (args.dump_first
                        && stats.seg().horizon.is_none()
                        && !(pose.is_empty() && pd.clean()));
                emit_replay_csv(&mut csv, pt, &pose, &pd)?;
                let boundary_clean = stats.grade(tick.t, &pose, &pd, args, dump);
                // THE ROSTER PASS — the MC2 arm's twin
                // ([`roster_excuse`]). MC1/HW carries registered
                // deviations of its own (`mc1l32-stuck-explosion-wedge`).
                let excused = !boundary_clean && {
                    let rmap: BTreeMap<u16, &mgc_formats::mgcr::EntObsMc1> =
                        obs.entities.iter().map(|e| (e.slot, e)).collect();
                    let pmap: BTreeMap<u16, &mgc_formats::mgcr::EntObsMc1> =
                        port.entities.iter().map(|e| (e.slot, e)).collect();
                    let rctx = |slot: u16| {
                        rmap.get(&slot)
                            .or_else(|| pmap.get(&slot))
                            .map(|e| (e.class, e.model, e.x, e.y))
                    };
                    roster_excuse(
                        &mut stats,
                        roster.as_ref(),
                        &take,
                        tick.t,
                        &pose,
                        &pd,
                        slot,
                        &rctx,
                    )
                };
                if args.stop_at_div && !boundary_clean && !(excused && args.resync_deviations) {
                    stop_at = Some(tick.t);
                }
                // `--brief`'s first-divergence signature, captured the
                // moment a segment's horizon lands.
                if stats.seg().horizon == Some(tick.t) && stats.seg().sig.is_empty() {
                    let cm = |s: u16| {
                        obs.entities
                            .iter()
                            .find(|e| e.slot == s)
                            .map(|e| (e.class, e.model))
                    };
                    let sig = brief_sig(&pose, &pd, &cm);
                    stats.seg().sig = sig;
                }
                // ---- THE SEGMENTED DOCTRINE ----
                // A true incremental deviation closes the segment and
                // re-anchors, exactly the way a capture gap does. The
                // run keeps MEASURING past the first break instead of
                // running wild, and every reset tick names itself as a
                // fixture candidate.
                if (args.segmented || (excused && args.resync_deviations)) && !boundary_clean {
                    // A dirty boundary in the restart window IS the
                    // seam (MC2's checkpoint restore lands a tick
                    // BEFORE the witnessed respawn key) — same
                    // re-anchor, restart-tagged, never a candidate.
                    let kind = if excused {
                        SegOpen::Roster
                    } else if restart_at.is_some_and(|a| tick.t >= a && tick.t - a <= 4) {
                        SegOpen::Restart
                    } else {
                        SegOpen::Deviation
                    };
                    reset_at = Some((tick.t, kind));
                    // `--classify`: run the PAIR at the cluster HEAD
                    // (adjacent resets are one story). Pair DIRTY at
                    // t-1 ⇒ the one-tick law itself is wrong here —
                    // LOCAL, a fixture candidate. Pair CLEAN ⇒ the
                    // law is right and the break rides earlier state
                    // — INHERITED, a unit test / upstream dig. The
                    // doctrine of [segmented-residue], automated.
                    if args.classify && kind == SegOpen::Deviation && last_dev != Some(tick.t - 1) {
                        let cw = match classify_world.as_mut() {
                            Some(w) => w,
                            None => {
                                let (w, _) = crate::verify::build_world(&args.baked, &game, level)?;
                                classify_world = Some(w);
                                classify_world.as_mut().expect("just built")
                            }
                        };
                        // Terrain@t-1: the pre-apply snapshot when
                        // this tick carried a block, else the image
                        // as it stands (unchanged since t-1).
                        let cur;
                        let measured = if tick.terrain.is_some() && prev_measured.is_some() {
                            prev_measured.as_ref().map(|(h, ty, c, a)| {
                                (h.as_slice(), ty.as_slice(), c.as_deref(), a.as_deref())
                            })
                        } else {
                            cur = measured_planes(&timg);
                            cur
                        };
                        let pair_cmd = {
                            let mut c = fire_bits_mc1(&pst);
                            c.equip_left = rec.equip_left.map(SpellId);
                            c.equip_right = rec.equip_right.map(SpellId);
                            c.demolish = rec.demolish;
                            c.respawn = rec.respawn;
                            c.suicide = rec.suicide;
                            c
                        };
                        match exec_pair(
                            cw,
                            &pristine,
                            measured,
                            &pst,
                            &st,
                            &obs,
                            pair_cmd,
                            pair_cmd_prev,
                            PairPose::Pair,
                        ) {
                            Ok((pdp, _, _)) => {
                                stats.class_tags.insert(tick.t, !pdp.clean());
                            }
                            Err(e) => eprintln!("  classify t={}: {e}", tick.t),
                        }
                    }
                    last_dev = Some(tick.t);
                }
            } else {
                stats.seg().ungraded += 1;
            }
        }
        stats.seg().end = tick.t;
        dump_state(&world, tick.t)?;
        if let Some((t, kind)) = reset_at.take() {
            if kind == SegOpen::Restart {
                // Retail's reload re-read the level: un-consume the
                // one-shot dispositions (`World::reload_thing_table`).
                world.reload_thing_table();
            }
            let (ch, human_slot, _) = anchor_mc1(&mut world, &pristine, &timg, &st, t)?;
            chain = Some((ch, human_slot));
            stats.open(t, kind);
            restart_at = None;
        }
        if restart_at.is_some_and(|a| tick.t > a + 4) {
            restart_at = None;
        }
        // This pair's verify-law command becomes the next pair's
        // predecessor (only the fire bits matter downstream — the
        // classify pair's `set_prev_fire`).
        pair_cmd_prev = fire_bits_mc1(&pst);
        st_prev = Some((tick.t, st));
        if stop_at.is_some() {
            stats.truncated = stop_at;
            break;
        }
        if let Some(limit) = args.limit {
            if stats.segs.iter().map(|s| s.stepped).sum::<u64>() >= limit {
                break;
            }
        }
    }
    if let Some(spec) = port_dump {
        return Err(format!(
            "dump-state --port: t={} never reached (last boundary {})",
            spec.t,
            stats.segs.last().map_or(0, |s| s.end)
        ));
    }
    if let (Some(n), Some((t_last, st)), Some((inp, cmd))) =
        (overrun, st_prev.as_ref(), overrun_input)
    {
        let (mut ch, human_slot, active) = anchor_mc1(&mut world, &pristine, &timg, st, *t_last)?;
        println!(
            "== OVERRUN: anchored on the LAST record t={t_last} ({active} active, human slot \
             {human_slot}); stepping {n} ticks with the last recovered input"
        );
        for i in 1..=n {
            let t = t_last + i;
            let started = std::time::Instant::now();
            step_mc1(&mut world, &mut ch, inp, cmd);
            let (free, ev) = world.debug_pool();
            let pose = ch.pose();
            println!(
                "OVERRUN t={t} {:.1}ms live={} free={free} pose=({:.2},{:.2},{})",
                started.elapsed().as_secs_f64() * 1e3,
                ev.len(),
                pose.x as f64 / 256.0,
                pose.y as f64 / 256.0,
                pose.z,
            );
        }
        return Ok(stats.clean());
    }
    // The castle-transform watchdog's predicate is counted in the
    // retail arm too (features.rs `castle_watchdog_fired`): a free run
    // that reports it names a castle retail parked forever. Printed
    // only when nonzero, to stderr under --brief (the brief line is a
    // baseline artifact).
    {
        let ((n, first), (seized, stale)) = world.debug_castle_watchdog();
        if n != 0 {
            let line = format!(
                "CASTLE-WATCHDOG orphaned-wait ticks={n} first t={first} (recycle victims \
                 seized={seized}, stale skipped={stale})"
            );
            if args.brief {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
    }
    let mode = if args.pose_only { "pose-only" } else { "world" };
    // ⭐ A RESYNC MUST NEVER BE INVISIBLE IN A BASELINE DIFF. A plain
    // run that crossed a permadeath seam is holding retail's state
    // from that boundary on, so its numbers are not comparable with a
    // pure free run's — the mode says which instrument produced them.
    let mode = if args.resync_restarts && !args.segmented {
        format!("{mode}+resync-restarts")
    } else {
        mode.to_string()
    };
    let mode = mode.as_str();
    if args.brief {
        let terrain = if measured_planes(&timg).is_some() {
            "measured"
        } else {
            "pristine"
        };
        print!(
            "{}",
            stats.render_brief(&crate::verify::take_stem(path), mode, terrain)
        );
    } else {
        print!("{}", stats.render(mode));
    }
    if let Some(sh) = shadow.as_ref() {
        // The free run's question is "what broke FIRST", so the lanes
        // are ordered by the tick they part, not by family.
        print!("{}", sh.render(true));
    }
    Ok(stats.clean())
}

/// Tier-2 pair: world context re-imported at N (retail's own world),
/// the flight chain stepped on. World-driven pose domains (death,
/// warp, accel, unrecoverable stick wipes) re-seed the chain and are
/// counted as gates, not divergence.
#[allow(clippy::too_many_arguments)]
fn pose_only_pair_mc1(
    world: &mut World,
    pristine: &mgc_sim::engine::features::Planes,
    timg: &Option<mgc_formats::mgcr::TerrainImage>,
    pst: &RetailMc1,
    st: &RetailMc1,
    ch: &mut Chain,
    slot: u16,
    pt: u64,
    inp: Mc1Input,
    stick_ok: bool,
    stats: &mut RStats,
    csv: &mut Option<std::io::BufWriter<std::fs::File>>,
) -> Result<(), String> {
    let e0 = &pst.ents[slot as usize];
    let e1 = &st.ents[slot as usize];
    let (w0, w1) = (
        &pst.wizards[pst.local_player as usize],
        &st.wizards[st.local_player as usize],
    );
    let mut gate = |why: &'static str, stats: &mut RStats| {
        *stats.gates.entry(why).or_default() += 1;
        *ch = Chain::seed_mc1(st, slot);
    };
    if matches!(e0.f70, 2 | 3) || matches!(e1.f70, 2 | 3) {
        gate("death/respawn", stats);
        return Ok(());
    }
    if !stick_ok {
        gate("stick-unrecoverable", stats);
        return Ok(());
    }
    if (e1.x.wrapping_sub(e0.x) as i16).unsigned_abs() > 2048
        || (e1.y.wrapping_sub(e0.y) as i16).unsigned_abs() > 2048
    {
        gate("warp", stats);
        return Ok(());
    }
    if e0.f126.abs() > 80
        || w0.cmd_speed.abs() > 80
        || e1.f126.abs() > 80
        || w1.cmd_speed.abs() > 80
    {
        gate("accel-domain", stats);
        return Ok(());
    }
    // Fresh retail context at N: measured terrain@N+1 (the pose
    // channel's phase law — the image already holds N+1 here), the
    // imported world for walls/ground.
    world.restore_planes(pristine);
    world
        .retail_import_mc1(pst)
        .map_err(|e| format!("t={pt}: import: {e}"))?;
    // Measured@N+1 AFTER the import — the pose channel's order (the
    // importer's terrain replay double-applies on measured planes).
    if let Some((h, ty, ceil, an)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil, an)
            .map_err(|e| format!("t={pt}: terrain: {e}"))?;
    }
    // The chained mover consumes the recorded knock reconstruction
    // (no world tick runs to arm the channel).
    let knock = consumed_knock(w0.knock_mag, w0.knock_dir, w1.knock_mag, w1.knock_dir);
    let w: &World = world;
    flight::mc1_move(
        &mut ch.s,
        &inp,
        None,
        knock,
        &|x, y| w.ground_z_engine(x, y),
        &|cur, prop| w.player_wall_gate_fixed(cur, prop),
    );
    let pose = pose_lanes_mc1(&ch.s, e1, w1);
    pose_window_mc1(pt + 1, &ch.s, e1, w1);
    stats.fold_pose_only(pt + 1, &pose, csv)
}

// --------------------------------------------------------------- MC2 run

fn run_mc2(
    path: &std::path::Path,
    args: &Args,
    port_dump: Option<&PortDump>,
) -> Result<bool, String> {
    let mut rec = Recording::open(path)?;
    let level = rec.header.level.ok_or("recording has no level number")?;
    if !args.brief {
        println!(
            "== replay {} (game mc2, level {level}{})",
            path.display(),
            if args.pose_only { ", pose-only" } else { "" }
        );
    }
    let replayed = crate::verify_mc2::mc2_take_replayed(path)?;
    let (mut world, pristine, things) =
        crate::verify_mc2::build_world_mc2(&args.baked, level, replayed)?;
    // The known-deviation roster — see the MC1 arm and [`roster_excuse`].
    let roster = crate::verify::load_roster(args)?;
    let take = crate::verify::take_stem(path);
    let mut csv = open_csv(args)?;
    let mut shadow = crate::shadow::Shadow::from_env()?;
    // `MGC_STATE_DUMP=<t>:<path>` — the MC1 arm's INHERITED-head
    // instrument, same semantics (see run_mc1): one sectioned
    // whole-world dump at the first tick at or after `t`; an anchored
    // run dumps retail's import, a walked run dumps the port's, and
    // the diff attributes a pair-clean free-run break.
    let state_dump: Option<(u64, String)> = std::env::var("MGC_STATE_DUMP").ok().and_then(|s| {
        let (t, path) = s.split_once(':')?;
        Some((t.parse().ok()?, path.to_string()))
    });
    let mut state_dumped = false;
    let mut dump_state = |world: &World, t: u64| -> Result<(), String> {
        let Some(spec) = state_dump.as_ref() else {
            return Ok(());
        };
        if t < spec.0 || state_dumped {
            return Ok(());
        }
        state_dumped = true;
        println!("  STATE DUMP at t={t} -> {}", spec.1);
        let mut out = String::new();
        for (name, bytes) in world.debug_state_sections() {
            let _ = write!(out, "{name}\t{}\t", bytes.len());
            for b in &bytes {
                let _ = write!(out, "{b:02x}");
            }
            out.push('\n');
        }
        std::fs::write(&spec.1, out).map_err(|e| format!("state dump: {e}"))
    };
    let mut timg = (!args.no_terrain)
        .then(|| {
            rec.header
                .channels
                .terrain
                .as_ref()
                .map(mgc_formats::mgcr::TerrainImage::new)
        })
        .flatten();
    // `--start <t>`: every record before t is skipped WITHOUT decoding
    // — only its terrain delta is folded, which is exactly what the
    // loop below did with them (docs/PERF-CONFORM.md).
    if !args.brief {
        crate::slice_banner(&rec);
    }
    if let Some(s) = args.start {
        crate::late_tick_hint(&rec, path, s);
        rec.skip_to(s, timg.as_mut())?;
    }

    let mut stats = RStats::default();
    let mut celltrace = CellTrace::from_env();
    let mut pcensus = PlaneCensus::from_env();
    let mut pace = PaceTrace::from_env();
    let mut ktrace = KnockTrace::from_env();
    // MGC_ALLOC_TRACE — the pool-allocator microscope (dig 98-Q13).
    let alloctrace = crate::alloc_trace::AllocTrace::from_env();
    let mut stagetrace = crate::alloc_trace::StageTrace::from_env();
    // MGC_MOB_TRACE=<slot>[;<slot>…]:<t0>:<t1> — see the emission site
    // below; same spelling as the MC1 arm's.
    let mtrace = std::env::var("MGC_MOB_TRACE").ok().and_then(|v| {
        let (slots, ts) = v.split_once(':')?;
        let (a, b) = ts.split_once(':')?;
        let slots: Vec<usize> = slots.split(';').filter_map(|s| s.parse().ok()).collect();
        Some((slots, a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
    });
    let mut st_prev: Option<(u64, RetailMc2)> = None;
    let mut chain: Option<(Chain, u16)> = None;
    // The respawn-press dating witness (mgc_formats::recover law).
    let mut witness = recover::Mc2RespawnWitness::default();
    let mut printed_import = false;
    // `--segmented` / `--classify` state — the MC1 arm's shape exactly
    // (see run_mc1): the re-anchor runs AFTER the tick body so the
    // break's own diagnostics still see the diverged state, and the
    // classify pair runs on a SCRATCH world with terrain@t-1.
    let mut reset_at: Option<(u64, SegOpen)> = None;
    // The port's restart signal — see the MC1 loop's `restart_at`.
    let mut restart_at: Option<u64> = None;
    let mut classify_world: Option<World> = None;
    #[allow(clippy::type_complexity)]
    let mut prev_measured: Option<(Vec<u8>, Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)> = None;
    let mut last_dev: Option<u64> = None;
    // `--stop-at-divergence`: armed at the boundary that broke, read at
    // the END of that tick's body so the break's own bookkeeping (and
    // its diagnostics) still run exactly as they do in a full sweep.
    let mut stop_at: Option<u64> = None;
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        // `--classify` keeps the PRE-apply planes: the pair check at
        // boundary t must run on terrain@t-1, and after this apply
        // the image holds t.
        if args.classify
            && args.segmented
            && tick.terrain.is_some()
            && let Some((h, ty, ceil, an)) = measured_planes(&timg)
        {
            prev_measured = Some((
                h.to_vec(),
                ty.to_vec(),
                ceil.map(|c| c.to_vec()),
                an.map(|a| a.to_vec()),
            ));
        }
        if let (Some(img), Some(block)) = (timg.as_mut(), &tick.terrain) {
            img.dump_delta(block, tick.t);
            img.apply(block)
                .map_err(|e| format!("t={}: terrain: {e}", tick.t))?;
        }
        let Some(state) = &tick.state else {
            st_prev = None;
            continue;
        };
        if args.start.is_some_and(|s| tick.t < s) {
            continue;
        }
        let st = decode_retail_mc2(state)?;
        let obs: ObsMc2 = match &tick.obs {
            Some(v) => serde_json::from_str(v.get()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        // The respawn witness folds EVERY record in stream order
        // (dating law on [`recover::Mc2RespawnWitness`]); fire rides
        // the CONSUMED move/fire byte on the pair's END record —
        // both laws live in the shared recovery home.
        let respawn = witness.observe(tick.input.as_ref());

        let anchor = !matches!((&st_prev, &chain), (Some((pt, _)), Some(_)) if tick.t == pt + 1);
        if anchor {
            let (ch, human_slot) = anchor_mc2(&mut world, &pristine, &things, &timg, &st, tick.t)?;
            chain = Some((ch, human_slot));
            stats.open(
                tick.t,
                if stats.segs.is_empty() {
                    SegOpen::Seed
                } else {
                    SegOpen::Gap
                },
            );
            if !printed_import {
                printed_import = true;
                let measured = measured_planes(&timg).is_some();
                if !args.brief {
                    println!(
                        "   import: human slot {human_slot}, terrain {}",
                        if measured { "MEASURED" } else { "pristine" }
                    );
                }
                // The recorder captures the cave CEILING on every map
                // type; the port models the plane only on caves, where
                // its presence IS the cave signal. Naming the drop
                // keeps the off-cave ceiling an ANNOUNCED ungraded
                // lane instead of a silent one.
                if !args.brief
                    && measured_planes(&timg).is_some_and(|(_, _, c, _)| c.is_some())
                    && !world.has_ceiling_plane()
                {
                    println!(
                        "   ⚠ measured CEILING dropped: this level carries no ceiling plane                          (off-cave) — the capture's ceiling is an UNGRADED lane here"
                    );
                }
                // Same law as the MC1 arm: a reset restores terrain
                // from the measured channel, so without one the count
                // is a capture artifact.
                if args.segmented && !measured && !args.brief {
                    println!(
                        "   ⚠ --segmented WITHOUT a measured terrain channel: every reset \
                         restores PRISTINE planes, so the reset count is a capture artifact, \
                         not a port score (re-record this take with terrain)"
                    );
                }
            }
            dump_state(&world, tick.t)?;
            // A `--port` dump landing ON an anchor: the port state IS
            // the retail import — still printed (identity modulo
            // representation is itself a useful calibration), with
            // the caveat named.
            if let Some(spec) = port_dump
                && tick.t >= spec.t
            {
                render_port_dump_mc2(&world, &st, human_slot, spec, tick.t, true);
                return Ok(true);
            }
            st_prev = Some((tick.t, st));
            continue;
        }
        let (pt, pst) = st_prev.take().expect("anchored");
        let (ch, human_slot) = chain.as_mut().expect("anchored");
        let slot = *human_slot;
        let cp = &st.players[st.local_player as usize];

        // ---- input for the pair pt → t, all from the recording
        // (the shared recovery laws — mgc_formats::recover) ----
        let mut rec = recover::recover_pair_mc2(&pst, &st, respawn, tick.input.as_ref());
        let mut inp = mc1_mover_input(rec.move_byte, rec.stick());
        inp.mc2_park = rec.mc2_park;
        // ⭐⭐ THE SPEED COMMAND IS NOT AN INPUT, IT IS A REGISTER —
        // AND IN A PURE INPUT REPLAY IT CARRIES ITSELF. This arm used
        // to pin `tgt_speed` from the recovered `cmd_speed` lane every
        // tick and hold the key bits out of the integrator, on the
        // reading that MC2's command is "mouse-proportional, not the
        // ±16 key servo". Measured: mc2l3's entire take takes exactly
        // eleven values, every multiple of 16 in ±80 and nothing else
        // — the key servo's own output set. The port ports `sub_5F380`
        // verbatim, so replaying the KEYS reproduces the register, and
        // that is the only form that also reproduces what the register
        // does NOT hold.
        //
        // The pin cost two whole families. It fed the mover values
        // retail's mover never saw, because a recorded command is the
        // SETTLED one: the wall dead-stop zeroes it after the servo ran
        // (which is why `recover::mc2_pair_cmd_speed` has to un-do it
        // for PAIR mode, where N+1 is all there is), and the SPEED
        // token writes it from a walk slot above the carpet — galore
        // t=5890's 240 landed after `sub_5D530`, and pinning it at the
        // tick head sent the port flying while retail stood still. And
        // suppressing the keys erased `word_0xe_14`, so the brake
        // could not cancel a boost at all (t=6278: the down key clamps
        // 160 → 80 in `sub_5F380` and the token restores on the same
        // tick).
        //
        // ⭐ SO THE FREE RUN SEEDS THE REGISTER FROM THE PREVIOUS
        // BOUNDARY AND LETS THE KEYS INTEGRATE IT. The record at N is
        // by definition the register as tick N+1 STARTS — after the
        // dead-stop, after the token's write — which is precisely the
        // value `sub_5F380` opens on. Every family then falls out of
        // one line: an ordinary tick integrates to the value N+1
        // records; a blocked tick integrates and the port's own
        // dead-stop re-zeroes it; a boost tick integrates nothing
        // (`move_bits 0x10` holds no speed key) and the token writes
        // afterwards; and a brake tick clamps 160 → 80 and arms
        // `word_0xe_14` on the way past.
        //
        // Pair mode keeps the reconstruction: it re-imports both
        // endpoints every tick and never carries a register across, so
        // N+1 is all it has.
        ch.s.tgt_speed = pst.players[pst.local_player as usize].cmd_speed;
        if rec.rebind_dropped {
            stats.rebind_dropped += 1;
        }
        if rec.mc2_select.is_some() {
            stats.equips += 1;
        }
        if rec.respawn {
            stats.respawns += 1;
        }
        // ⭐⭐ **SHIFT+K WAS MISSING FROM THE MC2 LOOP'S COMMAND.** The
        // MC1 arm above counts and forwards `rec.suicide`; this one
        // built its `PlayerCommand` without the field, so even a
        // recovered MC2 self-kill could not have reached
        // `World::tick`'s MC2 suicide arm. Both halves of the lane —
        // the state witness in `recover_pair_mc2_k` and this
        // forwarding — are new; see the witness for the mc2l5 evidence.
        if rec.suicide {
            stats.suicides += 1;
        }
        let cmd = PlayerCommand {
            fire_left: rec.fire_left,
            fire_right: rec.fire_right,
            mc2_select: rec.mc2_select,
            mc2_ring_cast: rec.mc2_ring_cast,
            // ⭐ THE RING BIND (`PlayerAction 0x26`) — the free
            // runner's own lane. See `recover::mc2_ring_bind`.
            spell_ring: rec.spell_ring,
            respawn: rec.respawn,
            suicide: rec.suicide,
            demolish: rec.demolish,
            cheat: rec.cheat,
            ..PlayerCommand::default()
        };

        // ⭐⭐⭐ THE CRANK COUNT COMES FROM THE SIMULATION, NOT FROM A
        // GUESS. `sub_33340` adds 28 to `roll_0x155_341` once per
        // visit that reaches the `v40` block (`NETHERW.EXE`
        // 0x57c67..0x57c8d), and its disc walk reaches the same victim
        // more than once whenever `CopyEntityPosition_57CF0` carries
        // it into a cell the walk has not got to yet. The recovery has
        // to undo exactly those 28s before inverting the stick filter,
        // and the recorded pair CANNOT count them — mc2l24 t=7913
        // inverts to a legal signed-byte cursor for k=1 (72) and for
        // k=2 (14) alike, and to none at all for k=0. So: trial-step a
        // THROWAWAY CLONE with k=1, ask the port how many cranks it
        // actually handed the carpet, and if that is not 1, recover
        // again with the true count and run the REAL step on it.
        // Deterministic, and rare — the trial only fires on a pair
        // whose own witness (`whirl_crank`: the funnel's write to the
        // human's `word_0x30_48`) says a funnel held the wizard.
        //
        // ⚠ THE CLONE IS THE WHOLE POINT: the first step never touches
        // the live world, so nothing has to be undone and no side
        // effect (restart latch, cheat book, ktrace arm, sound) is
        // double-counted.
        // ⭐⭐⭐ AND THE TRIAL IS ALSO THE WITNESS, NOT ONLY THE COUNT.
        // `recover::mc2_crank_witness` is a pair predicate and both of
        // its disjuncts are EDGES, so a mid-ring visit that recomputes
        // the SAME `word_0x30_48` and leaves the victim ungrabbed is
        // invisible to it (mc2l12 t=9584 — see
        // `recover_pair_mc2_kw`). Whenever a (10,22) head is live the
        // trial runs anyway and the PORT supplies the witness; the
        // recovery then reads the capture's own `rollDelta_0x4_4`,
        // which is count-free and retail's own datum.
        // `MGC_NO_MC2_WW_CRANK_SIM_WITNESS` restores the pair-only
        // witness (and the trial's old `rec.whirl_crank` gate).
        let sim_witness = !std::env::var_os("MGC_NO_MC2_WW_CRANK_SIM_WITNESS").is_some();
        if (rec.whirl_crank || (sim_witness && world.mc2_whirlwind_alive()))
            && !args.pose_only
            && mgc_sim::mc2_ww_crank_count_law()
            && !recover::paused_turn_mc2(&pst, &st)
        {
            let mut trial_world = world.clone();
            let mut trial_ch = ch.clone();
            mgc_sim::reset_whirl_cranks();
            step_mc2(&mut trial_world, &mut trial_ch, inp, cmd);
            let k = mgc_sim::whirl_cranks();
            // k == 0 with no pair witness is the ordinary
            // no-funnel-reached-him tick: the alt recovery would be
            // byte-identical, so do not spend it (or count it).
            if (rec.whirl_crank && k != 1) || (!rec.whirl_crank && k >= 1) {
                let alt = recover::recover_pair_mc2_kw(
                    &pst,
                    &st,
                    respawn,
                    tick.input.as_ref(),
                    k,
                    sim_witness && k >= 1,
                );
                // ⚠ NEVER TRADE A RECOVERABLE CURSOR FOR AN
                // UNRECOVERABLE ONE. A count the port got WRONG (its
                // funnel missed a visit retail made) must not also
                // cost the pair its whole stick — mc2l1 t=283 is
                // exactly that pair: retail cranked, the port's ring
                // walk reached the carpet zero times, and k=0 inverts
                // to a cursor of 148 (out of the signed byte).
                if alt.stick_ok() || !rec.stick_ok() {
                    rec = alt;
                    inp = mc1_mover_input(rec.move_byte, rec.stick());
                    inp.mc2_park = rec.mc2_park;
                    stats.crank_repass += 1;
                }
            }
            mgc_sim::reset_whirl_cranks();
        }
        let stick_ok = rec.stick_ok();
        if !stick_ok {
            stats.stick_unrec += 1;
        }
        mgc_sim::DEBUG_TICK.store(tick.t, std::sync::atomic::Ordering::Relaxed);
        if args.pose_only {
            pose_only_pair_mc2(
                &mut world, &pristine, &things, &timg, &pst, &st, ch, slot, pt, inp, stick_ok,
                &mut stats, &mut csv,
            )?;
        } else {
            // `--port --at-slot <n>`: arm the mid-walk pool snapshot
            // for the tick INTO the dump boundary.
            if let Some(spec) = port_dump
                && tick.t == spec.t
                && let Some(n) = spec.at_slot
            {
                world.arm_walk_probe(n);
            }
            book_cheat(&world, rec.cheat, &mut stats);
            ktrace.arm(&world);
            // The MC1 law's twin — see the `paused_turn_mc1` branch in
            // the MC1 loop. MC2's frame function draws at its top
            // (EF:39947) and gates the body below, measured on
            // `mc2l0-test`'s three deliberate pause cycles: one LCG
            // step, zero pool changes.
            // The barrel roll's homing-lock break (`sub_55EB0`),
            // retail's PLAYER FRAME, before `UpdateEntities_57730` —
            // see [`World::mc2_broll_lock_break`]. The free-run lane
            // pins the carpet and never runs `sub_55C60`, so the edge
            // comes off the imported `byte_0x846_2BDE` phase.
            world.mc2_broll_lock_break(
                pst.players[pst.local_player as usize].broll_phase,
                st.players[st.local_player as usize].broll_phase,
            );
            if recover::paused_turn_mc2(&pst, &st) {
                world.tick_paused();
                stats.paused += 1;
            } else {
                step_mc2(&mut world, ch, inp, cmd);
            }
            if world.take_restart() {
                restart_at = Some(tick.t);
            }
            if let Some(spec) = port_dump
                && tick.t >= spec.t
            {
                render_port_dump_mc2(&world, &st, slot, spec, tick.t, false);
                return Ok(true);
            }
            let kp = &pst.players[pst.local_player as usize];
            ktrace.emit(
                &world,
                pt,
                tick.t,
                (kp.knock_dir, kp.knock_mag),
                (cp.knock_dir, cp.knock_mag),
            );
            if let Ok(v) = std::env::var("MGC_PLANE_DIFF") {
                if let Some((a, b)) = v.split_once(':')
                    && let (Ok(t0), Ok(t1)) = (a.parse::<u64>(), b.parse::<u64>())
                    && tick.t >= t0
                    && tick.t <= t1
                    && let Some(img) = timg.as_ref()
                    && let Some(th) = img.plane("height")
                {
                    let ph = &world.planes().height;
                    let mut n = 0usize;
                    let mut first = Vec::new();
                    for i in 0..th.len().min(ph.len()) {
                        if th[i] != ph[i] {
                            n += 1;
                            if first.len() < 4000 {
                                first.push((i & 0xFF, i >> 8, ph[i], th[i]));
                            }
                        }
                    }
                    println!("PLANEDIFF t={} hdiff={n} {first:?}", tick.t);
                }
            }
            celltrace.emit(&world, &timg, tick.t);
            pcensus.emit(&world, &timg, tick.t);
            pace.emit(&world, &st, tick.t);
            // `MGC_MOB_TRACE` — the MC1 arm has carried the creature-
            // machine microscope since 19c, and on MC2 it silently
            // no-opped, which reads as "this take has no state drift"
            // (the instrument-asymmetry trap, 4th occurrence). The
            // homes are re-mapped, not renamed: retail's TARGET-YAW
            // channel is `roll` @0x20 and the port keeps it in `f34`,
            // so a raw `f34`-vs-`f34` print would have compared
            // @0x34 (the subentity chain) against it.
            if let Some((slots, t0, t1)) = mtrace.as_ref()
                && tick.t >= *t0
                && tick.t <= *t1
            {
                for &s in slots {
                    let re = &st.ents[s];
                    let p = world.debug_mob_machine(s);
                    let pf = p.map_or_else(
                        || "  port <none>".to_string(),
                        |(t70, f52, f63, f34, f146, f126, rand, f71)| {
                            format!(
                                "  port a45={t70} f32={f52} f3e={f63} roll={f34} \
                                 t96={f146} speed={f126} rand={rand} f46={f71}"
                            )
                        },
                    );
                    println!(
                        "MOB t={} slot {s} ({},{}) retail a45={} f32={} f3e={} roll={} \
                         t96={} speed={} rand={} f46={}\nMOB t={} slot {s}{pf}",
                        tick.t,
                        re.class3f,
                        re.model40,
                        re.action45,
                        re.f32,
                        re.phase3e,
                        re.roll,
                        re.target96,
                        re.speed,
                        re.rand,
                        re.b46,
                        tick.t,
                    );
                }
            }
            stats.seg().stepped += 1;
            if (args.segmented || (args.resync_restarts && !stats.any_diverged()))
                && restart_at.is_some_and(|a| tick.t > a && tick.t - a <= 4)
            {
                // The reload boundary — see the MC1 arm (segmented, or
                // `--resync-restarts`; same plain-mode purity rule).
                // MC2's restore re-reads a PRE-CAPTURE disk checkpoint
                // (SaveLevel slot 1), so the seam is ungradable by
                // construction — and re-importing retail's closure is
                // the ONLY way an MC2 take can carry a permadeath at
                // all, which is why this arm is not MC1-only.
                stats.seg().ungraded += 1;
                reset_at = Some((tick.t, SegOpen::Restart));
            } else if capture_clean_mc2(&pst, &st) {
                let pose = pose_lanes_mc2(&ch.s, &ch.ext, &st.ents[slot as usize], cp);
                pose_window_mc2(tick.t, &ch.s, &ch.ext, &st.ents[slot as usize], cp);
                let mut castles = [0i16; 8];
                for (i, p) in pst.players.iter().take(8).enumerate() {
                    castles[i] = p.castle;
                }
                let pin = PinnedMc2 {
                    slot,
                    local: pst.local_player,
                    player_count: pst.player_count,
                    pose: ch.pose(),
                    castles,
                };
                let port = world.obs_project_mc2(&pin);
                // Two scopes: the census gets the full cadence law, the
                // graded diff below keeps the narrow one — see
                // `verify_mc2::TearScope`.
                let torn = torn_slots(&pst, &st);
                let torn_graded =
                    crate::verify_mc2::torn_slots_scoped(&pst, &st, crate::verify_mc2::TearScope::Graded);
                // THE RAW SHADOW, free-run half. Unlike pair mode the
                // port has been carrying its OWN state since the
                // anchor, so the first tick a raw lane parts is the
                // first tick the port's HISTORY parts from retail's —
                // the only instrument that can explain a `--segmented`
                // break whose pair diff at the same tick is CLEAN.
                if let Some(at) = alloctrace.as_ref() {
                    at.emit(&world, &st, slot, tick.t);
                }
                if let Some(sg) = stagetrace.as_mut() {
                    sg.emit(&world, &st, tick.t);
                }
                if let Some(sh) = shadow.as_mut() {
                    sh.compare_ents_mc2(&world, &st, slot, &torn, tick.t);
                    sh.compare_map_heads_mc2(&world, &st, slot, &torn, tick.t);
                    sh.compare_wiz_mc2(&world, &st, tick.t);
                    sh.compare_free_mc2(&world, &st, slot, tick.t);
                    // THE OBJECTIVE BOARD, free-run half: the port has
                    // carried its own since the anchor, so the first
                    // row is the tick its objective history parts —
                    // and a stage-gated disposition rides that latch.
                    sh.compare_board_mc2(&world, &st, tick.t);
                }
                let mut pd = compare_mc2_gated(&obs, &port, slot, &torn_graded);
                // The MC2 sprite lane, free-run half (see the MC1
                // boundary above).
                crate::verify_mc2::append_sprite_diffs_mc2(&mut pd, &st, &world, slot, &torn_graded);
                let pd = pd;
                let dump = args.dump == Some(pt)
                    || (args.dump_first
                        && stats.seg().horizon.is_none()
                        && !(pose.is_empty() && pd.clean()));
                emit_replay_csv(&mut csv, pt, &pose, &pd)?;
                let boundary_clean = stats.grade(tick.t, &pose, &pd, args, dump);
                // THE ROSTER PASS — is every row on this boundary a
                // registered DEVIATION? (see [`roster_excuse`]).
                let excused = !boundary_clean && {
                    let rmap: BTreeMap<u16, &mgc_formats::mgcr::EntObsMc2> =
                        obs.entities.iter().map(|e| (e.slot, e)).collect();
                    let pmap: BTreeMap<u16, &mgc_formats::mgcr::EntObsMc2> =
                        port.entities.iter().map(|e| (e.slot, e)).collect();
                    let rctx = |slot: u16| {
                        rmap.get(&slot)
                            .or_else(|| pmap.get(&slot))
                            .map(|e| (e.class, e.model, e.x, e.y))
                    };
                    roster_excuse(
                        &mut stats,
                        roster.as_ref(),
                        &take,
                        tick.t,
                        &pose,
                        &pd,
                        slot,
                        &rctx,
                    )
                };
                if args.stop_at_div && !boundary_clean && !(excused && args.resync_deviations) {
                    stop_at = Some(tick.t);
                }
                if stats.seg().horizon == Some(tick.t) && stats.seg().sig.is_empty() {
                    let cm = |s: u16| {
                        obs.entities
                            .iter()
                            .find(|e| e.slot == s)
                            .map(|e| (e.class, e.model))
                    };
                    let sig = brief_sig(&pose, &pd, &cm);
                    stats.seg().sig = sig;
                }
                // ---- THE SEGMENTED DOCTRINE (the MC1 arm's twin) ----
                // `--resync-deviations` extends the re-anchor to a
                // PLAIN free run, but only for an excused boundary:
                // that is the whole instrument, and it is why the flag
                // is not simply "ignore the roster rows".
                if (args.segmented || (excused && args.resync_deviations)) && !boundary_clean {
                    // A dirty boundary in the restart window IS the
                    // seam (the checkpoint restore lands a tick
                    // BEFORE the witnessed respawn key).
                    let kind = if excused {
                        SegOpen::Roster
                    } else if restart_at.is_some_and(|a| tick.t >= a && tick.t - a <= 4) {
                        SegOpen::Restart
                    } else {
                        SegOpen::Deviation
                    };
                    reset_at = Some((tick.t, kind));
                    // `--classify`: the pair at the cluster HEAD.
                    // Pair DIRTY at t-1 ⇒ LOCAL (fixture candidate),
                    // CLEAN ⇒ INHERITED (unit test / upstream dig).
                    if args.classify && kind == SegOpen::Deviation && last_dev != Some(tick.t - 1) {
                        let cw = match classify_world.as_mut() {
                            Some(w) => w,
                            None => {
                                let (w, _, _) = crate::verify_mc2::build_world_mc2(
                                    &args.baked,
                                    level,
                                    replayed,
                                )?;
                                classify_world = Some(w);
                                classify_world.as_mut().expect("just built")
                            }
                        };
                        // Terrain@t-1: the pre-apply snapshot when
                        // this tick carried a block, else the image
                        // as it stands (unchanged since t-1).
                        let cur;
                        let measured = if tick.terrain.is_some() && prev_measured.is_some() {
                            prev_measured.as_ref().map(|(h, ty, c, a)| {
                                (h.as_slice(), ty.as_slice(), c.as_deref(), a.as_deref())
                            })
                        } else {
                            cur = measured_planes(&timg);
                            cur
                        };
                        // The previous pair's command is unknowable
                        // here — seed its fire from the START record's
                        // consumed byte, the anchor's own
                        // approximation. The pair command is the
                        // recovered one the free step just consumed.
                        let (pfl, pfr) =
                            recover::mc1_fire(pst.players[pst.local_player as usize].move_bits);
                        let prev_cmd = PlayerCommand {
                            fire_left: pfl,
                            fire_right: pfr,
                            ..PlayerCommand::default()
                        };
                        match crate::verify_mc2::exec_pair_mc2(
                            cw,
                            &pristine,
                            measured,
                            &things,
                            &pst,
                            &st,
                            &obs,
                            cmd,
                            prev_cmd,
                            if crate::verify_mc2::mc2_pose_pair() {
                                crate::verify::PairPose::Pair
                            } else {
                                crate::verify::PairPose::PinN1
                            },
                            rec.mc2_park,
                        ) {
                            Ok((pdp, _, _)) => {
                                stats.class_tags.insert(tick.t, !pdp.clean());
                            }
                            Err(e) => eprintln!("  classify t={}: {e}", tick.t),
                        }
                    }
                    last_dev = Some(tick.t);
                }
            } else {
                stats.seg().ungraded += 1;
            }
        }
        stats.seg().end = tick.t;
        dump_state(&world, tick.t)?;
        if let Some((t, kind)) = reset_at.take() {
            if kind == SegOpen::Restart {
                // The MC1 arm's twin — MC2's checkpoint restore
                // re-reads the level image (SLEV), table included.
                world.reload_thing_table();
            }
            let (ch, human_slot) = anchor_mc2(&mut world, &pristine, &things, &timg, &st, t)?;
            chain = Some((ch, human_slot));
            stats.open(t, kind);
            restart_at = None;
        }
        if restart_at.is_some_and(|a| tick.t > a + 4) {
            restart_at = None;
        }
        st_prev = Some((tick.t, st));
        if stop_at.is_some() {
            stats.truncated = stop_at;
            break;
        }
        if let Some(limit) = args.limit {
            if stats.segs.iter().map(|s| s.stepped).sum::<u64>() >= limit {
                break;
            }
        }
    }
    if let Some(spec) = port_dump {
        return Err(format!(
            "dump-state --port: t={} never reached (last boundary {})",
            spec.t,
            stats.segs.last().map_or(0, |s| s.end)
        ));
    }
    let mode = if args.pose_only { "pose-only" } else { "world" };
    // ⭐ A RESYNC MUST NEVER BE INVISIBLE IN A BASELINE DIFF. A plain
    // run that crossed a permadeath seam is holding retail's state
    // from that boundary on, so its numbers are not comparable with a
    // pure free run's — the mode says which instrument produced them.
    let mode = if args.resync_restarts && !args.segmented {
        format!("{mode}+resync-restarts")
    } else {
        mode.to_string()
    };
    // The horizon is only final once the run is over.
    pace.finish(stats.first_horizon());
    let mode = mode.as_str();
    if args.brief {
        let terrain = if measured_planes(&timg).is_some() {
            "measured"
        } else {
            "pristine"
        };
        print!(
            "{}",
            stats.render_brief(&crate::verify::take_stem(path), mode, terrain)
        );
    } else {
        print!("{}", stats.render(mode));
    }
    if let Some(sh) = shadow.as_ref() {
        // The free run's question is "what broke FIRST", so the lanes
        // are ordered by the tick they part, not by family.
        print!("{}", sh.render(true));
    }
    Ok(stats.clean())
}

/// Tier-2 MC2 pair — the MC1 twin's shape with the MC2 gates
/// (debuffs included: the ladder phase story is unmeasured, the pose
/// channel's own gate).
#[allow(clippy::too_many_arguments)]
fn pose_only_pair_mc2(
    world: &mut World,
    pristine: &mgc_sim::engine::features::Planes,
    things: &mgc_sim::engine::world::conformance::ThingTable,
    timg: &Option<mgc_formats::mgcr::TerrainImage>,
    pst: &RetailMc2,
    st: &RetailMc2,
    ch: &mut Chain,
    slot: u16,
    pt: u64,
    inp: Mc1Input,
    stick_ok: bool,
    stats: &mut RStats,
    csv: &mut Option<std::io::BufWriter<std::fs::File>>,
) -> Result<(), String> {
    let e0 = &pst.ents[slot as usize];
    let e1 = &st.ents[slot as usize];
    let (p0, p1) = (
        &pst.players[pst.local_player as usize],
        &st.players[st.local_player as usize],
    );
    let row = world.mc2_carpet_row();
    let mut gate = |why: &'static str, stats: &mut RStats| {
        *stats.gates.entry(why).or_default() += 1;
        *ch = Chain::seed_mc2(st, slot, row);
    };
    if e0.action45 != 0 || e1.action45 != 0 {
        gate("death/respawn", stats);
        return Ok(());
    }
    if !stick_ok {
        gate("stick-unrecoverable", stats);
        return Ok(());
    }
    if (e1.x.wrapping_sub(e0.x) as i16).unsigned_abs() > 2048
        || (e1.y.wrapping_sub(e0.y) as i16).unsigned_abs() > 2048
    {
        gate("warp", stats);
        return Ok(());
    }
    if e0.speed.abs() > 80
        || p0.cmd_speed.abs() > 80
        || e1.speed.abs() > 80
        || p1.cmd_speed.abs() > 80
    {
        gate("accel-domain", stats);
        return Ok(());
    }
    if p0.move_speed != 0 || p1.move_speed != 0 || p0.mobilize != 0 || p1.mobilize != 0 {
        gate("debuff (web-slow/paralyze)", stats);
        return Ok(());
    }
    world.restore_planes(pristine);
    world.restore_thing_table(things);
    world
        .retail_import_mc2(pst)
        .map_err(|e| format!("t={pt}: import: {e}"))?;
    // Measured@N+1 AFTER the import — the pose channel's order.
    if let Some((h, ty, ceil, an)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil, an)
            .map_err(|e| format!("t={pt}: terrain: {e}"))?;
    }
    let knock = consumed_knock(p0.knock_mag, p0.knock_dir, p1.knock_mag, p1.knock_dir);
    ch.ext.row = world.mc2_carpet_row();
    let w: &World = world;
    // THE DUEL LEASH, off retail's own lock register — the three words
    // `dword_0x142_322` / `word_0x146_326` / `word_0x14A_330` were in
    // the capture all along; only the decoder was missing them. Armed
    // while the register stands at BOTH ends of the pair, and read at
    // the carpet's PRE-MOVE position like `sub_5DE30` does. See the
    // twin note in `pose_lane.rs`.
    let carpet_pre = (ch.s.x, ch.s.y, ch.s.z);
    let leash = (p0.duel_target != 0 && p1.duel_target != 0)
        .then(|| w.mc2_duel_leash_recorded(carpet_pre, p0.duel_target, p0.duel_hold))
        .flatten();
    flight::mc2_move(
        &mut ch.s,
        &mut ch.ext,
        &inp,
        None,
        knock,
        leash,
        &|x, y| w.ground_z_engine(x, y),
        &|x, y| w.player_cave_ceiling(x, y),
        &|cur, prop| w.player_mc2_gate(cur, prop),
        &|pos, latched| w.player_mc2_stuck(pos, latched),
    );
    let pose = pose_lanes_mc2(&ch.s, &ch.ext, e1, p1);
    pose_window_mc2(pt + 1, &ch.s, &ch.ext, e1, p1);
    stats.fold_pose_only(pt + 1, &pose, csv)
}

// ---------------------------------------------------- the port-side dump

/// `dump-state --port` — the instrument the whole wishlist ranked
/// first: every other tool either READS THE RECORDING or COMPARES
/// projections; nothing printed what the PORT holds at tick T. This
/// free-runs (or, with `--start t-1`, pair-imports) to T and prints
/// the port's record lane-for-lane beside retail's.
pub(crate) struct PortDump {
    pub(crate) t: u64,
    pub(crate) slots: Vec<u16>,
    /// Sample MID-WALK: snapshot the pool as the tick into `t`
    /// reaches this slot, before it dispatches.
    pub(crate) at_slot: Option<u16>,
}

/// Entry from `mgc-conform dump-state <file> <t> <slot>… --port` —
/// dispatches on the take's family (the MC2 arm is `run_mc2`'s twin
/// hook; the walk loop is game-shared, so `--at-slot` carries over).
pub(crate) fn port_dump(path: &std::path::Path, t: u64, slots: &[u16], args: &Args) -> i32 {
    let family = match Recording::open(path).and_then(|r| r.header.family()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    if args.pose_only {
        eprintln!("dump-state --port drives the full world (drop --pose-only)");
        return 2;
    }
    let spec = PortDump {
        t,
        slots: slots.to_vec(),
        at_slot: args.at_slot,
    };
    let run = match family {
        mgc_formats::mgcr::Family::Mc1 => run_mc1(path, args, Some(&spec)),
        mgc_formats::mgcr::Family::Mc2 => run_mc2(path, args, Some(&spec)),
    };
    match run {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            2
        }
    }
}

/// The side-by-side: retail's record at the boundary vs the port's
/// live pool (or the mid-walk probe), joined BY LANE NAME, ≠-marked,
/// `—` where the port does not model the lane. ALL fields print —
/// the graded ones exactly because "graded and ungraded alike" is
/// what makes this a state dump rather than another diff.
fn render_port_dump(
    world: &World,
    st: &RetailMc1,
    human_slot: u16,
    spec: &PortDump,
    t: u64,
    at_anchor: bool,
) {
    use mgc_sim::engine::world::conformance::retail_ent_lanes_mc1;
    println!(
        "== dump-state --port t={t}{}",
        if t != spec.t {
            format!(
                " (requested t={} has no graded boundary — nearest after)",
                spec.t
            )
        } else {
            String::new()
        }
    );
    if at_anchor {
        println!(
            "   ⚠ t is an ANCHOR (seed/gap/--start): the port state IS the retail \
             import — expect identity modulo representation"
        );
    } else {
        // ⭐ NAME THE PAIR. The port column is the state AFTER the
        // tick `t-1 -> t`; the retail column is the record AT t. Both
        // the free-run shadow (`compare_ents_*` below in this loop)
        // and `verify-deltas`' shadow now stamp that boundary `t` —
        // but `verify-deltas`' GRADED rows, `--dump` and the fixture
        // manifests name the same pair by its START tick `t-1`, and a
        // dig that mixed the two lost a day in round 148.
        println!(
            "   pair {} → {t}: the port column is POST-tick, the retail column is the \
             record AT t (pair-scoped output — graded rows, --dump, fixtures — names \
             this pair {})",
            t.saturating_sub(1),
            t.saturating_sub(1)
        );
    }
    let mut from_probe = false;
    if let Some(n) = spec.at_slot {
        if at_anchor {
            println!("   ⚠ --at-slot {n} ignored: no tick ran into an anchor");
        } else if world.walk_probe_hit() {
            from_probe = true;
            println!(
                "   pool sampled MID-WALK as slot {n} was reached (the retail column \
                 stays the BOUNDARY state at t={t} — retail has no mid-walk sample)"
            );
        } else {
            println!(
                "   ⚠ the walk never reached slot {n} this tick — showing the \
                 POST-TICK pool instead"
            );
        }
    }
    for &slot in &spec.slots {
        let Some(re) = st.ents.get(slot as usize) else {
            println!("  slot {slot}: out of range");
            continue;
        };
        if slot == human_slot {
            println!(
                "  ⚠ slot {slot} is the HUMAN CARPET: the port carries it out-of-pool, \
                 so the port column below is the reserved hole, not the player"
            );
        }
        // The MC2 twin's law, same shape (see `render_port_dump_mc2`):
        // a class-0 slot is RESIDUE. `verify.rs`'s pair diff skips it
        // (`if e.class64 == 0 … continue`) and the lane table homes
        // every offset by the CURRENT class byte, which retail's free
        // path has zeroed — so nothing below is comparable.
        let freed = re.class64 == 0;
        if freed && slot != human_slot {
            let on_free = st.free_stack.contains(&slot);
            let on_rec = st.recycle_stack.contains(&slot);
            println!(
                "  ⚠ slot {slot} is FREE in retail (class byte 0{}{}): the pair diff never \
                 compares a class-0 slot and the lane table homes every offset by the \
                 CURRENT class, so the rows below are RESIDUE read through the wrong \
                 field: they are marked with a middle dot, NOT the difference glyph, and \
                 no grade sees them. Dump the tick the slot was last LIVE to compare it.",
                if on_free { ", on the free stack" } else { "" },
                if on_rec { ", on the recycle stack" } else { "" },
            );
        }
        let mark = if freed { "  ·" } else { "  ≠" };
        let retail = retail_ent_lanes_mc1(re);
        let port: BTreeMap<&'static str, Option<i64>> = world
            .port_ent_lanes_mc1(slot, human_slot, from_probe)
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();
        println!(
            "  slot {slot}: retail ({},{}) f70={} life={}  {:24} {:>12} {:>12}",
            re.class64, re.model65, re.f70, re.act_life, "lane", "retail", "port"
        );
        for (name, rv) in retail {
            match port.get(name) {
                Some(Some(pv)) => {
                    println!(
                        "    {:24} {:>12} {:>12}{}",
                        name,
                        rv,
                        pv,
                        if *pv != rv { mark } else { "" }
                    );
                }
                Some(None) => println!("    {:24} {:>12} {:>12}", name, rv, "—"),
                None => println!("    {:24} {:>12} {:>12}", name, rv, "?"),
            }
        }
    }
    // The allocator context — the same tails the recording-side
    // dump-state prints (next pop LAST). BOTH halves: once the free
    // stack is dry MC1 seizes recycle victims, and a one-cell
    // disagreement there shifts every seizure that follows
    // (mc1hwl0 t=31888). The MC2 twin has printed both since it was
    // written; the MC1 arm printed only `free` and made that dig a
    // multi-command detour.
    let tail = |v: &[u16]| v[v.len().saturating_sub(8)..].to_vec();
    let live: Vec<u16> = st
        .free_stack
        .iter()
        .copied()
        .filter(|&s| (s as usize) < st.ents.len() && s != human_slot)
        .collect();
    let got = world.free_stack_mc1();
    println!(
        "  free stack: retail len {} tail {:?}  port len {} tail {:?}",
        live.len(),
        tail(&live),
        got.len(),
        tail(got),
    );
    let want_rec: Vec<u16> = st
        .recycle_stack
        .iter()
        .copied()
        .filter(|&s| (s as usize) < st.ents.len() && s != human_slot)
        .collect();
    let got_rec = world.recycle_stack_mc1();
    println!(
        "  recycle stack: retail len {} tail {:?}  port len {} tail {:?}",
        want_rec.len(),
        tail(&want_rec),
        got_rec.len(),
        tail(got_rec),
    );
}

/// The MC2 side-by-side — [`render_port_dump`]'s twin joined through
/// [`retail_ent_lanes_mc2`]/[`World::port_ent_lanes_mc2`]. Same
/// conventions: `≠`-marked, `—` = the port does not model the lane
/// for this record's class, `?` = a table mismatch (extend BOTH).
/// The raw `flags` lane always prints `—` — compare the translated
/// bit sub-lanes below it.
fn render_port_dump_mc2(
    world: &World,
    st: &RetailMc2,
    human_slot: u16,
    spec: &PortDump,
    t: u64,
    at_anchor: bool,
) {
    use mgc_sim::engine::world::conformance::retail_ent_lanes_mc2;
    println!(
        "== dump-state --port t={t}{}",
        if t != spec.t {
            format!(
                " (requested t={} has no graded boundary — nearest after)",
                spec.t
            )
        } else {
            String::new()
        }
    );
    if at_anchor {
        println!(
            "   ⚠ t is an ANCHOR (seed/gap/--start): the port state IS the retail \
             import — expect identity modulo representation"
        );
    } else {
        // ⭐ NAME THE PAIR. The port column is the state AFTER the
        // tick `t-1 -> t`; the retail column is the record AT t. Both
        // the free-run shadow (`compare_ents_*` below in this loop)
        // and `verify-deltas`' shadow now stamp that boundary `t` —
        // but `verify-deltas`' GRADED rows, `--dump` and the fixture
        // manifests name the same pair by its START tick `t-1`, and a
        // dig that mixed the two lost a day in round 148.
        println!(
            "   pair {} → {t}: the port column is POST-tick, the retail column is the \
             record AT t (pair-scoped output — graded rows, --dump, fixtures — names \
             this pair {})",
            t.saturating_sub(1),
            t.saturating_sub(1)
        );
    }
    let mut from_probe = false;
    if let Some(n) = spec.at_slot {
        if at_anchor {
            println!("   ⚠ --at-slot {n} ignored: no tick ran into an anchor");
        } else if world.walk_probe_hit() {
            from_probe = true;
            println!(
                "   pool sampled MID-WALK as slot {n} was reached (the retail column \
                 stays the BOUNDARY state at t={t} — retail has no mid-walk sample)"
            );
        } else {
            println!(
                "   ⚠ the walk never reached slot {n} this tick — showing the \
                 POST-TICK pool instead"
            );
        }
    }
    for &slot in &spec.slots {
        let Some(re) = st.ents.get(slot as usize) else {
            println!("  slot {slot}: out of range");
            continue;
        };
        if slot == human_slot {
            println!(
                "  ⚠ slot {slot} is the HUMAN CARPET: the port carries it out-of-pool, \
                 so the port column below is the reserved hole, not the player"
            );
        }
        // ⭐⭐⭐ A CLASS-0 SLOT IS RESIDUE, AND RESIDUE IS NOT A
        // DIVERGENCE. Round 149 banked "`--port --start <t>` returns a
        // divergent port column while the graded replay from the same
        // anchor is bit-exact" as a broken instrument arm; round 150
        // measured it and the arm is fine — every one of the 113 `≠`
        // rows mc2l22 t=63665 printed sat on a slot retail holds FREE.
        // Two reasons they are not comparable and neither is a defect:
        //   * the pair diff SKIPS a class-0 slot outright (the importer
        //     still carries its stale bytes — `import_ent_mc2`'s
        //     "a freed slot is not an EMPTY slot" arm), so no grade
        //     ever looks at these lanes;
        //   * the lane table dispatches every `@offset` on the CURRENT
        //     class byte, which retail's free path has already zeroed
        //     — so residue written under the slot's PREVIOUS class
        //     reads through a DIFFERENT port field. mc2l22 slot 5: the
        //     port's `f140` still holds the (10,m) `@0x2A` amount 400
        //     exactly as retail's record does, but class 0 homes `f2a`
        //     in `f44` and `mana` in `f140`, so one identical residue
        //     byte printed as TWO `≠` rows.
        // The free-list link lanes (`next16`/`prev18`) are the same
        // story from the other side: the port keeps its free list in a
        // Vec, so it prints 0 where retail's record still holds the
        // in-slot chain.
        let freed = re.class3f == 0;
        if freed && slot != human_slot {
            let on_free = st.free_stack.contains(&slot);
            let on_rec = st.recycle_stack.contains(&slot);
            println!(
                "  ⚠ slot {slot} is FREE in retail (class byte 0{}{}): the pair diff never \
                 compares a class-0 slot and the lane table homes every offset by the \
                 CURRENT class, so the rows below are RESIDUE read through the wrong \
                 field: they are marked with a middle dot, NOT the difference glyph, and \
                 no grade sees them. Dump the tick the slot was last LIVE to compare it.",
                if on_free { ", on the free stack" } else { "" },
                if on_rec { ", on the recycle stack" } else { "" },
            );
        }
        let mark = if freed { "  ·" } else { "  ≠" };
        let retail = retail_ent_lanes_mc2(re);
        let port: BTreeMap<&'static str, Option<i64>> = world
            .port_ent_lanes_mc2(slot, human_slot, from_probe)
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();
        println!(
            "  slot {slot}: retail ({},{}) action={} life={}  {:24} {:>12} {:>12}",
            re.class3f, re.model40, re.action45, re.life, "lane", "retail", "port"
        );
        for (name, rv) in retail {
            match port.get(name) {
                Some(Some(pv)) => {
                    println!(
                        "    {:24} {:>12} {:>12}{}",
                        name,
                        rv,
                        pv,
                        if *pv != rv { mark } else { "" }
                    );
                }
                Some(None) => println!("    {:24} {:>12} {:>12}", name, rv, "—"),
                None => println!("    {:24} {:>12} {:>12}", name, rv, "?"),
            }
        }
    }
    // The allocator context — free FIRST, recycle when dry (the MC2
    // pop order), next pop LAST, the human slot filtered like the
    // importer does.
    let want_free: Vec<u16> = st
        .free_stack
        .iter()
        .copied()
        .filter(|&s| (s as usize) < st.ents.len() && s != human_slot)
        .collect();
    let want_rec: Vec<u16> = st
        .recycle_stack
        .iter()
        .copied()
        .filter(|&s| (s as usize) < st.ents.len() && s != human_slot)
        .collect();
    let (got_free, got_rec) = world.free_stacks_mc2();
    println!(
        "  free stack: retail len {} tail {:?}  port len {} tail {:?}",
        want_free.len(),
        &want_free[want_free.len().saturating_sub(8)..],
        got_free.len(),
        &got_free[got_free.len().saturating_sub(8)..],
    );
    println!(
        "  recycle stack: retail len {} tail {:?}  port len {} tail {:?}",
        want_rec.len(),
        &want_rec[want_rec.len().saturating_sub(8)..],
        got_rec.len(),
        &got_rec[got_rec.len().saturating_sub(8)..],
    );
}

// -------------------------------------------------------------- plumbing

/// Compact first-divergence signature for `--brief`:
/// `(9,0)slot399:id,x,y` / `pose:vx` / `missing(5,9)slot123x4` /
/// `rng`. `cm` resolves a slot to its recorded (class, model).
fn brief_sig(
    pose: &[(&'static str, i64, i64)],
    pd: &PairDiff,
    cm: &dyn Fn(u16) -> Option<(u8, u8)>,
) -> String {
    if !pose.is_empty() {
        let names: Vec<&str> = pose.iter().map(|(n, ..)| *n).take(4).collect();
        return format!("pose:{}", names.join(","));
    }
    if let Some((slot, c, m)) = pd.missing.first() {
        return format!("missing({c},{m})slot{slot}x{}", pd.missing.len());
    }
    if let Some((slot, c, m)) = pd.extra.first() {
        return format!("extra({c},{m})slot{slot}x{}", pd.extra.len());
    }
    if let Some(d) = pd.fields.first() {
        return match d.slot {
            Some(slot) => {
                let mut fields: Vec<&str> = pd
                    .fields
                    .iter()
                    .filter(|f| f.slot == Some(slot))
                    .map(|f| f.field)
                    .take(6)
                    .collect();
                fields.dedup();
                match cm(slot) {
                    Some((c, m)) => format!("({c},{m})slot{slot}:{}", fields.join(",")),
                    None => format!("slot{slot}:{}", fields.join(",")),
                }
            }
            None => d.field.to_string(),
        };
    }
    if pd.rng_want != pd.rng_got {
        return "rng".into();
    }
    "-".into()
}

fn open_csv(args: &Args) -> Result<Option<std::io::BufWriter<std::fs::File>>, String> {
    match &args.csv {
        Some(p) => {
            let f = std::fs::File::create(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut w = std::io::BufWriter::new(f);
            use std::io::Write as _;
            writeln!(
                w,
                "t\tkind\tslot\tclass\tmodel\tfield\twant\tgot\tx\ty\tz\trule"
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(w))
        }
        None => Ok(None),
    }
}

/// TSV rows at a graded boundary (pose lanes + the world diff, the
/// verify-deltas column shape) for offline triage.
fn emit_replay_csv(
    csv: &mut Option<std::io::BufWriter<std::fs::File>>,
    t: u64,
    pose: &[(&'static str, i64, i64)],
    pd: &PairDiff,
) -> Result<(), String> {
    let Some(w) = csv.as_mut() else {
        return Ok(());
    };
    use std::io::Write as _;
    let mut go = || -> std::io::Result<()> {
        for (name, want, got) in pose {
            writeln!(w, "{t}\tpose\t\t\t\t{name}\t{want}\t{got}\t\t\t\t")?;
        }
        if pd.rng_want != pd.rng_got {
            writeln!(
                w,
                "{t}\trng\t\t\t\t\t{}\t{}\t\t\t\t",
                pd.rng_want, pd.rng_got
            )?;
        }
        for (slot, c, m) in &pd.missing {
            writeln!(w, "{t}\tmissing\t{slot}\t{c}\t{m}\t\t\t\t\t\t\t")?;
        }
        for (slot, c, m) in &pd.extra {
            writeln!(w, "{t}\textra\t{slot}\t{c}\t{m}\t\t\t\t\t\t\t")?;
        }
        for d in &pd.fields {
            match d.slot {
                Some(slot) => writeln!(
                    w,
                    "{t}\tfield\t{slot}\t\t\t{}\t{}\t{}\t\t\t\t",
                    d.field, d.want, d.got
                )?,
                None => writeln!(
                    w,
                    "{t}\tfield\t\t\t\t{}\t{}\t{}\t\t\t\t",
                    d.field, d.want, d.got
                )?,
            }
        }
        Ok(())
    };
    go().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE PIN for the pose-channel roster pass (round 119, dig
    /// W119-8). The law's only observable is the GRADING VERDICT, not
    /// a sim lane — `pose.z` and `pose.eff_pitch` are not `ObsMc1`
    /// entity fields and the fixture suite deliberately grades raw
    /// signatures with the roster switched out (roster.rs' own
    /// doctrine), so a recording fixture cannot pin this. Sweep
    /// evidence instead: mc1l48 12847 → 13895, mc1l49 / mc1l0 /
    /// mc1l32-quick / mc2l22 / mc1hwl0 / mc1l0-spells-galore
    /// byte-identical, 477 fixtures 0 regressions.
    ///
    /// UNCONDITIONAL: the assertion is `true`, so the test FAILS
    /// under `MGC_NO_POSE_ROSTER_EXCUSE=1`.
    fn painter_pose_roster() -> crate::roster::Roster {
        serde_json::from_str(
            r#"{"rules":[{
                 "id":"mc1l48-reload-painter-dat-damage-top-wall-pose",
                 "status":"deviation","note":"test copy of the shipped rule",
                 "takes":["mc1l48"],"kind":"pose","class":3,"model":0,
                 "fields":["pose.z","pose.eff_pitch"],"slots":[681],
                 "ticks":[12847],"rect":[142.0,59.5,163.0,62.5]}]}"#,
        )
        .expect("rule parses")
    }

    /// The measured witness: mc1l48 boundary t=12848, carpet slot 681
    /// (class 3 model 0) at tile (143.95, 61.57) — inside the
    /// registered top-wall rect — with retail 1446/21 against port
    /// 1444/16 on the two ground-derived lanes.
    #[test]
    fn pose_rows_on_the_painter_wound_are_roster_excused() {
        let r = painter_pose_roster();
        let mut stats = RStats::default();
        let pose = [("pose.z", 1446_i64, 1444_i64), ("pose.eff_pitch", 21, 16)];
        let ctx = |s: u16| (s == 681).then_some((3_u8, 0_u8, 143.953_125, 61.574_218_75));
        assert!(
            roster_excuse(
                &mut stats,
                Some(&r),
                "mc1l48",
                12848,
                &pose,
                &PairDiff::default(),
                681,
                &ctx,
            ),
            "the pose channel must be scopeable — the player is just another pool record"
        );
        assert_eq!(stats.roster_ticks, vec![12848]);
        assert_eq!(
            stats.roster_hits["mc1l48-reload-painter-dat-damage-top-wall-pose"],
            (2, 1)
        );
    }

    /// The scope is real, not a blanket pass: the same two lanes one
    /// tick later, or off the rect, or on a lane the rule does not
    /// name, stay UNEXPLAINED. All-or-nothing survives too — one
    /// unclaimed pose row demotes the whole boundary.
    #[test]
    fn the_pose_rule_stays_scoped() {
        let r = painter_pose_roster();
        let ctx = |s: u16| (s == 681).then_some((3_u8, 0_u8, 143.953_125, 61.574_218_75));
        let off_rect = |s: u16| (s == 681).then_some((3_u8, 0_u8, 32.2, 87.1));
        let z = [("pose.z", 1446_i64, 1444_i64)];
        let mut st = RStats::default();
        // Wrong boundary (the floor is a finite tick list).
        assert!(!roster_excuse(
            &mut st,
            Some(&r),
            "mc1l48",
            12849,
            &z,
            &PairDiff::default(),
            681,
            &ctx
        ));
        // Off the wound's rect (t=42813's pose head, 100 tiles away).
        assert!(!roster_excuse(
            &mut st,
            Some(&r),
            "mc1l48",
            12848,
            &z,
            &PairDiff::default(),
            681,
            &off_rect
        ));
        // A lane the rule does not name.
        let unnamed = [("pose.act_speed", 80_i64, -16_i64)];
        assert!(!roster_excuse(
            &mut st,
            Some(&r),
            "mc1l48",
            12848,
            &unnamed,
            &PairDiff::default(),
            681,
            &ctx
        ));
        // ALL-OR-NOTHING: a claimed lane beside an unclaimed one.
        let mixed = [("pose.z", 1446_i64, 1444_i64), ("pose.x", 0, 1)];
        assert!(!roster_excuse(
            &mut st,
            Some(&r),
            "mc1l48",
            12848,
            &mixed,
            &PairDiff::default(),
            681,
            &ctx
        ));
        // A different take.
        assert!(!roster_excuse(
            &mut st,
            Some(&r),
            "mc1l49",
            12848,
            &z,
            &PairDiff::default(),
            681,
            &ctx
        ));
        assert!(st.roster_ticks.is_empty(), "nothing may be booked");
    }

    /// `RowKind::Pose` is its own kind so no `field`-scoped rule — nor
    /// a kind-less one — can reach a pose row by accident.
    #[test]
    fn a_field_rule_never_reaches_a_pose_row() {
        use crate::roster::{Roster, RowCtx, RowKind};
        let r: Roster = serde_json::from_str(
            r#"{"rules":[
                {"id":"field-scoped","status":"deviation","note":"",
                 "kind":"field","class":3,"model":0,"fields":["pose.z","z"]},
                {"id":"kindless","status":"deviation","note":"",
                 "class":3,"model":0}]}"#,
        )
        .expect("rules parse");
        let pose_row = RowCtx {
            kind: RowKind::Pose,
            slot: Some(681),
            class: 3,
            model: 0,
            field: Some("pose.z"),
            pos: Some((143.95, 61.57)),
        };
        // The field-scoped rule is skipped; only the kind-less one can
        // see a pose row at all, and `roster_excuse` still needs every
        // row claimed by a `deviation` rule.
        assert_eq!(r.classify("mc1l48", 12847, &pose_row), Some(1));
        let field_only: Roster = serde_json::from_str(
            r#"{"rules":[{"id":"field-scoped","status":"deviation","note":"",
                 "kind":"field","class":3,"model":0,"fields":["pose.z"]}]}"#,
        )
        .expect("rule parses");
        assert_eq!(field_only.classify("mc1l48", 12847, &pose_row), None);
    }
}
