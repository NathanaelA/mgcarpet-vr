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
use crate::pose_lane::{consumed_knock, recover_stick};
use crate::verify::{PairDiff, append_hand_diffs, capture_clean, compare, measured_planes};
use crate::verify_mc2::{
    capture_clean_mc2, compare_mc2_gated, mouse_pos_mc2, press_pos_mc2, respawn_key_mc2, torn_slots,
};
use mgc_formats::mgcr::{
    ObsMc1, ObsMc2, Recording, RetailMc1, RetailMc2, decode_retail_mc1, decode_retail_mc2,
};
use mgc_sim::engine::world::conformance::{PinnedMc1, PinnedMc2};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::flight::{self, Mc1Input, Mc1State, Mc2Ext};
use mgc_sim::mc1::spells::SpellId;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(crate) fn replay(path: &std::path::Path, args: &Args) -> i32 {
    let family = match Recording::open(path).and_then(|r| r.header.family()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let res = match family {
        mgc_formats::mgcr::Family::Mc1 => run_mc1(path, args),
        mgc_formats::mgcr::Family::Mc2 => run_mc2(path, args),
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

/// The chained human flight state — the driver's copy of what
/// `Simulation` owns in the app (integer carpet + MC2 channels + the
/// Accelerate expiry edge).
#[derive(Default)]
struct Chain {
    s: Mc1State,
    ext: Mc2Ext,
    accel_was_active: bool,
}

impl Chain {
    /// Seed from the recorded closure at the anchor (the pose
    /// channel's field map, pose_lane.rs).
    fn seed_mc1(st: &RetailMc1, slot: u16) -> Self {
        let e = &st.ents[slot as usize];
        let w = &st.wizards[st.local_player as usize];
        Chain {
            s: Mc1State {
                x: e.x,
                y: e.y,
                z: e.z,
                yaw: e.f30 & 0x7FF,
                roll_f: w.roll_acc as i16,
                pitch_f: w.pitch_acc as i16,
                aim_pitch: e.f32 & 0x7FF,
                eff_pitch: w.eff_pitch & 0x7FF,
                act_speed: e.f126,
                tgt_speed: w.cmd_speed,
                strafe: w.strafe,
                tick_ctr: e.f63,
                rand: e.rand,
            },
            ext: Mc2Ext::default(),
            accel_was_active: false,
        }
    }

    /// The MC2 twin — plus the debuff ladders and water/nudge
    /// channels the pose channel gates instead of seeding.
    fn seed_mc2(st: &RetailMc2, slot: u16, row: flight::Mc2Row) -> Self {
        let e = &st.ents[slot as usize];
        let p = &st.players[st.local_player as usize];
        Chain {
            s: Mc1State {
                x: e.x,
                y: e.y,
                z: e.z,
                yaw: e.yaw as u16 & 0x7FF,
                roll_f: p.roll_acc as i16,
                pitch_f: p.pitch_acc as i16,
                aim_pitch: e.pitch as u16 & 0x7FF,
                eff_pitch: p.eff_pitch & 0x7FF,
                act_speed: e.speed,
                tgt_speed: p.cmd_speed,
                strafe: p.strafe,
                tick_ctr: 0,
                rand: 0,
            },
            ext: Mc2Ext {
                move_speed: p.move_speed,
                move_speed_ctr: p.move_speed_ctr,
                mobilize: p.mobilize,
                mobilize_ctr: p.mobilize_ctr,
                add: (0, 0, 0),
                water_ctr: p.water_ctr as u16,
                nudge_latch: p.nudge_latch != 0,
                row,
            },
            accel_was_active: false,
        }
    }

    fn pose(&self) -> PlayerPose {
        PlayerPose {
            x: self.s.x,
            y: self.s.y,
            z: self.s.z,
            heading: self.s.yaw,
            pitch: self.s.aim_pitch,
            speed: self.s.act_speed,
        }
    }
}

/// One free-run tick, MC1/HW — `Simulation::step`'s faithful path
/// (mgc-sim lib.rs:556-897) in integer space: dead/falling input
/// override, Accelerate expiry edge, knock drain, the mover, the
/// death fall + dead-camera turn, `World::tick`, then the
/// respawn/teleport/speed-zero mailboxes back into the carpet.
fn step_mc1(world: &mut World, ch: &mut Chain, inp: Mc1Input, cmd: PlayerCommand) {
    let falling = world.player_falling();
    let dead = world.player_dead();
    let (inp, cmd) = if falling || dead {
        (
            Mc1Input::default(),
            PlayerCommand {
                respawn: cmd.respawn,
                ..PlayerCommand::default()
            },
        )
    } else {
        (inp, cmd)
    };
    if dead {
        ch.s.act_speed = 0;
        ch.s.tgt_speed = 0;
        ch.s.strafe = 0;
    }
    let thrust = if inp.speed_up {
        1.0
    } else if inp.speed_down {
        -1.0
    } else {
        0.0
    };
    world.thrust_cancel(thrust);
    // Accelerate expiry/cancel edge: MC1 restores +80 MAX FORWARD,
    // even out of backwards flight (:65191-97).
    let over = world.accel_override();
    if ch.accel_was_active && over.is_none() {
        ch.s.tgt_speed = 80;
        ch.s.act_speed = 80;
    }
    ch.accel_was_active = over.is_some();
    let knock = world.take_knock_step();
    let moved = {
        let w: &World = world;
        flight::mc1_move(
            &mut ch.s,
            &inp,
            over,
            knock,
            &|x, y| w.ground_z_engine(x, y),
            &|cur, prop| w.player_wall_gate_fixed(cur, prop),
        )
    };
    if moved.flutter {
        world.push_player_sound(46);
    }
    // Death fall (sub_45FC0): gravity on top of the drift, riding to
    // the ground+128 floor — integer form of the app's flyer-space
    // integration (the integer carpet is live under this driver).
    if falling {
        let dz = world.death_fall_step();
        let g = world.ground_z_engine(ch.s.x, ch.s.y);
        ch.s.z = (ch.s.z as i32 + dz as i32)
            .max(g as i32 + 128)
            .min(i16::MAX as i32) as i16;
    }
    // Dead (sub_463B0): the grey-screen camera turns toward the
    // killer while it waits for Space.
    if dead && let Some((kx, kz)) = world.killer_pos() {
        let tx = (kx.rem_euclid(256.0) * 256.0) as u16;
        let ty = (kz.rem_euclid(256.0) * 256.0) as u16;
        let target = flight::angle_between(ch.s.x, ch.s.y, tx, ty);
        let mut d = (target as i32 - ch.s.yaw as i32) & 0x7FF;
        if d > 1024 {
            d -= 2048;
        }
        ch.s.yaw = ((ch.s.yaw as i32 + d.clamp(-16, 16)) & 0x7FF) as u16;
    }
    world.tick(ch.pose(), cmd);
    // Respawn (sub_44D30): castle, one tile up, flight state re-armed,
    // heading preserved (the app's from_tiles reset — tick_ctr and the
    // private LCG restart with it).
    if let Some((x, z)) = world.take_respawn() {
        let g = world.ground_height_tiles(x, z);
        let yaw = ch.s.yaw;
        ch.s = Mc1State::from_tiles(x, z, g + 1.0, 0.0);
        ch.s.yaw = yaw;
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
fn step_mc2(world: &mut World, ch: &mut Chain, inp: Mc1Input, cmd: PlayerCommand) {
    let falling = world.player_falling();
    let dead = world.player_dead();
    let end_seized = world.mc2_end_pose().is_some();
    let (inp, cmd) = if falling || dead || end_seized {
        (
            Mc1Input::default(),
            PlayerCommand {
                respawn: cmd.respawn,
                ..PlayerCommand::default()
            },
        )
    } else {
        (inp, cmd)
    };
    if dead {
        ch.s.act_speed = 0;
        ch.s.tgt_speed = 0;
        ch.s.strafe = 0;
    }
    ch.ext.row = world.mc2_carpet_row();
    let thrust = if inp.speed_up {
        1.0
    } else if inp.speed_down {
        -1.0
    } else {
        0.0
    };
    world.thrust_cancel(thrust);
    // MC2's restore KEEPS the sign (GetScroll_69DB0 EF:56267-69).
    let over = world.accel_override();
    if ch.accel_was_active && over.is_none() {
        let sign = if ch.s.act_speed >= 0 { 1 } else { -1 };
        ch.s.tgt_speed = 80 * sign;
        ch.s.act_speed = 80 * sign;
    }
    ch.accel_was_active = over.is_some();
    let knock = world.take_knock_step();
    let (slow, stun) = world.take_mc2_debuffs();
    for _ in 0..slow {
        ch.ext.slow_hit();
    }
    for _ in 0..stun {
        ch.ext.stun_hit();
    }
    let moved = {
        let w: &World = world;
        flight::mc2_move(
            &mut ch.s,
            &mut ch.ext,
            &inp,
            over,
            knock,
            &|x, y| w.ground_z_engine(x, y),
            &|x, y| w.player_cave_ceiling(x, y),
            &|cur, prop| w.player_mc2_gate(cur, prop),
            &|pos, latched| w.player_mc2_stuck(pos, latched),
        )
    };
    if moved.accel_cancel {
        world.mc2_cancel_accel();
        ch.accel_was_active = false;
    }
    if falling {
        let dz = world.death_fall_step();
        let g = world.ground_z_engine(ch.s.x, ch.s.y);
        ch.s.z = (ch.s.z as i32 + dz as i32)
            .max(g as i32 + 128)
            .min(i16::MAX as i32) as i16;
    }
    if dead && let Some((kx, kz)) = world.killer_pos() {
        let tx = (kx.rem_euclid(256.0) * 256.0) as u16;
        let ty = (kz.rem_euclid(256.0) * 256.0) as u16;
        let target = flight::angle_between(ch.s.x, ch.s.y, tx, ty);
        let mut d = (target as i32 - ch.s.yaw as i32) & 0x7FF;
        if d > 1024 {
            d -= 2048;
        }
        ch.s.yaw = ((ch.s.yaw as i32 + d.clamp(-16, 16)) & 0x7FF) as u16;
    }
    world.tick(ch.pose(), cmd);
    if let Some((x, z)) = world.take_respawn() {
        let g = world.ground_height_tiles(x, z);
        let yaw = ch.s.yaw;
        ch.s = Mc1State::from_tiles(x, z, g + 1.0, 0.0);
        ch.s.yaw = yaw;
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
    // The ending sequence mirrors its scripted pose onto the carpet.
    if let Some((x, alt, z, yaw)) = world.mc2_end_pose() {
        ch.s.x = (x.rem_euclid(256.0) * 256.0) as u16;
        ch.s.y = (z.rem_euclid(256.0) * 256.0) as u16;
        ch.s.z = (alt * 256.0) as i16;
        ch.s.yaw = ((yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 2048.0)
            as u16)
            & 0x7FF;
        ch.s.act_speed = 0;
        ch.s.tgt_speed = 0;
    }
}

// ------------------------------------------------------- input recovery

/// MC1 dw_0 fire bits (0x10/0x20) — the CONSUMED per-tick fire levels,
/// stamped by the same consume loop as the move bits.
fn mc1_fire(mb: u32) -> (bool, bool) {
    (mb & 0x10 != 0, mb & 0x20 != 0)
}

fn mc1_mover_input(mb: u32, stick: (i16, i16)) -> Mc1Input {
    Mc1Input {
        stick_x: stick.0,
        stick_y: stick.1,
        speed_up: mb & 1 != 0,
        speed_down: mb & 2 != 0,
        strafe_left: mb & 4 != 0,
        strafe_right: mb & 8 != 0,
    }
}

// ---------------------------------------------------------- aggregation

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
    firsts: BTreeMap<&'static str, u64>,
    pose_rows: u64,
    rng_bad: u64,
    missing: u64,
    extra: u64,
    field_rows: u64,
}

#[derive(Default)]
struct RStats {
    segs: Vec<Segment>,
    gates: BTreeMap<&'static str, u64>,
    stick_unrec: u64,
    respawns: u64,
    equips: u64,
    rebind_dropped: u64,
}

impl RStats {
    fn seg(&mut self) -> &mut Segment {
        self.segs.last_mut().expect("segment open")
    }

    fn open(&mut self, t0: u64) {
        self.segs.push(Segment {
            t0,
            end: t0,
            ..Segment::default()
        });
    }

    /// Fold one graded boundary. `pose` rows are (lane, want, got);
    /// `pd` is the world diff at the boundary.
    fn grade(
        &mut self,
        t: u64,
        pose: &[(&'static str, i64, i64)],
        pd: &PairDiff,
        args: &Args,
        dump: bool,
    ) {
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
    }

    fn render(&self, mode: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "   mode: {mode}");
        for (i, seg) in self.segs.iter().enumerate() {
            let _ = writeln!(
                out,
                "   segment {i}: t={}..{} — {} stepped, {} graded ({} capture-skipped), {} clean",
                seg.t0, seg.end, seg.stepped, seg.graded, seg.ungraded, seg.clean
            );
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
                        "     traffic after: pose {} rows, rng {}/{} boundaries, sets {}/{} \
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
        let _ = writeln!(
            out,
            "   input events: {} respawn(s), {} equip/rebind(s){}",
            self.respawns,
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
        out
    }

    fn clean(&self) -> bool {
        self.segs.iter().all(|s| s.horizon.is_none())
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
        }
        emit_replay_csv(csv, t.saturating_sub(1), pose, &PairDiff::default())
    }
}

/// Pose lanes: the chained carpet vs the recorded pose at the graded
/// boundary (the pose channel's lane set).
fn pose_lanes_mc1(
    s: &Mc1State,
    e: &mgc_formats::mgcr::RetailEntMc1,
    w: &mgc_formats::mgcr::RetailWizardMc1,
) -> Vec<(&'static str, i64, i64)> {
    let mut rows = Vec::new();
    let mut lane = |name, want: i64, got: i64| {
        if want != got {
            rows.push((name, want, got));
        }
    };
    lane("pose.x", e.x as i64, s.x as i64);
    lane("pose.y", e.y as i64, s.y as i64);
    lane("pose.z", e.z as i64, s.z as i64);
    lane("pose.yaw", (e.f30 & 0x7FF) as i64, s.yaw as i64);
    lane("pose.aim_pitch", (e.f32 & 0x7FF) as i64, s.aim_pitch as i64);
    lane(
        "pose.eff_pitch",
        (w.eff_pitch & 0x7FF) as i64,
        (s.eff_pitch & 0x7FF) as i64,
    );
    lane("pose.act_speed", e.f126 as i64, s.act_speed as i64);
    lane("pose.tgt_speed", w.cmd_speed as i64, s.tgt_speed as i64);
    lane("pose.strafe", w.strafe as i64, s.strafe as i64);
    lane("pose.roll_f", w.roll_acc as i16 as i64, s.roll_f as i64);
    lane("pose.pitch_f", w.pitch_acc as i16 as i64, s.pitch_f as i64);
    lane("pose.tick_ctr", e.f63 as i64, s.tick_ctr as i64);
    lane("pose.rand", e.rand as i64, s.rand as i64);
    rows
}

fn pose_lanes_mc2(
    s: &Mc1State,
    e: &mgc_formats::mgcr::RetailEntMc2,
    p: &mgc_formats::mgcr::RetailPlayerMc2,
) -> Vec<(&'static str, i64, i64)> {
    let mut rows = Vec::new();
    let mut lane = |name, want: i64, got: i64| {
        if want != got {
            rows.push((name, want, got));
        }
    };
    lane("pose.x", e.x as i64, s.x as i64);
    lane("pose.y", e.y as i64, s.y as i64);
    lane("pose.z", e.z as i64, s.z as i64);
    lane("pose.yaw", (e.yaw as u16 & 0x7FF) as i64, s.yaw as i64);
    lane(
        "pose.aim_pitch",
        (e.pitch as u16 & 0x7FF) as i64,
        s.aim_pitch as i64,
    );
    lane(
        "pose.eff_pitch",
        (p.eff_pitch & 0x7FF) as i64,
        (s.eff_pitch & 0x7FF) as i64,
    );
    lane("pose.act_speed", e.speed as i64, s.act_speed as i64);
    lane("pose.tgt_speed", p.cmd_speed as i64, s.tgt_speed as i64);
    lane("pose.strafe", p.strafe as i64, s.strafe as i64);
    lane("pose.roll_f", p.roll_acc as i16 as i64, s.roll_f as i64);
    lane("pose.pitch_f", p.pitch_acc as i16 as i64, s.pitch_f as i64);
    // water_ctr is deliberately NOT a lane yet: it gates the
    // water-flight sound loop, not the pose. (Grading it here is what
    // EXPOSED the +610 u16-vs-int8 decode bug — the fixed byte read
    // makes it a candidate lane once its ++/−− law is verified against
    // a wet stretch.)
    rows
}

// --------------------------------------------------------------- MC1 run

fn run_mc1(path: &std::path::Path, args: &Args) -> Result<bool, String> {
    let mut rec = Recording::open(path)?;
    let game = rec.header.game.clone();
    let level = rec.header.level.ok_or("recording has no level number")?;
    println!(
        "== replay {} (game {game}, level {level}{})",
        path.display(),
        if args.pose_only { ", pose-only" } else { "" }
    );
    let (mut world, pristine) = crate::verify::build_world(&args.baked, &game, level)?;
    let mut csv = open_csv(args)?;
    let mut timg = (!args.no_terrain)
        .then(|| {
            rec.header
                .channels
                .terrain
                .as_ref()
                .map(mgc_formats::mgcr::TerrainImage::new)
        })
        .flatten();

    let mut stats = RStats::default();
    let mut st_prev: Option<(u64, RetailMc1)> = None;
    let mut chain: Option<(Chain, u16)> = None; // (flight chain, human slot)
    let mut printed_import = false;
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        // The terrain image tracks the take continuously (self-healing
        // deltas) — installed into the world only at anchors
        // (world mode) or per pair (pose-only, terrain@N+1).
        if let (Some(img), Some(block)) = (timg.as_mut(), &tick.terrain) {
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
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        let anchor = match (&st_prev, &chain) {
            (Some((pt, _)), Some(_)) if tick.t == pt + 1 => false,
            _ => true,
        };
        if anchor {
            // (Re-)anchor: restore planes to the measured image at
            // this record, import the closure, seed the chain. The
            // ONLY moments recording state touches the sim.
            world.restore_planes(&pristine);
            let report = world
                .retail_import_mc1(&st)
                .map_err(|e| format!("t={}: import: {e}", tick.t))?;
            // Measured planes AFTER the import: the importer's
            // terrain-replay pass reconstructs state-derived edits for
            // measurement-less runs — on already-measured planes it
            // DOUBLE-APPLIES them (the pose channel's proven order,
            // verify_mc2.rs pose-lane re-install).
            if let Some((h, ty, ceil)) = measured_planes(&timg) {
                world
                    .install_measured_terrain(h, ty, ceil)
                    .map_err(|e| format!("t={}: terrain: {e}", tick.t))?;
            }
            let (fl, fr) = mc1_fire(st.wizards[st.local_player as usize].move_bits);
            world.set_prev_fire(fl, fr);
            chain = Some((Chain::seed_mc1(&st, report.human_slot), report.human_slot));
            stats.open(tick.t);
            if !printed_import {
                printed_import = true;
                println!(
                    "   import: {} active entities, human slot {}, terrain {}",
                    report.active,
                    report.human_slot,
                    if measured_planes(&timg).is_some() {
                        "MEASURED"
                    } else {
                        "pristine"
                    }
                );
            }
            st_prev = Some((tick.t, st));
            continue;
        }
        let (pt, pst) = st_prev.take().expect("anchored");
        let (ch, human_slot) = chain.as_mut().expect("anchored");
        let slot = *human_slot;
        let pw = &pst.wizards[pst.local_player as usize];
        let cw = &st.wizards[st.local_player as usize];

        // ---- input for the pair pt → t, all from the recording ----
        let rec_stick = (
            recover_stick(pw.roll_acc as i16, cw.roll_acc as i16),
            recover_stick(pw.pitch_acc as i16, cw.pitch_acc as i16),
        );
        let stick_ok = matches!(rec_stick, (Some(_), Some(_)));
        let stick = match rec_stick {
            (Some(x), Some(y)) => (x, y),
            _ => {
                stats.stick_unrec += 1;
                (0, 0)
            }
        };
        let mb = pw.move_bits;
        let inp = mc1_mover_input(mb, stick);
        let (fire_left, fire_right) = mc1_fire(mb);
        // Equips: a recorded hand change across the pair replays as
        // the equip command (resolved to the internal spell id via the
        // acquisition list at N+1 — append_hand_diffs' convention).
        let equip = |prev_raw: u16, cur_raw: u16| -> Option<SpellId> {
            (prev_raw != cur_raw)
                .then(|| {
                    st.hand_spell(st.local_player as usize, cur_raw)
                        .map(SpellId)
                })
                .flatten()
        };
        let equip_left = equip(pw.hand_left, cw.hand_left);
        let equip_right = equip(pw.hand_right, cw.hand_right);
        if equip_left.is_some() || equip_right.is_some() {
            stats.equips += 1;
        }
        // Respawn: the SPACE lane on the END record (MC1 has no press
        // latch — ±1-tick dating caveat, RECORDING.md).
        let respawn = respawn_key_mc2(tick.input.as_ref());
        if respawn {
            stats.respawns += 1;
        }
        let cmd = PlayerCommand {
            fire_left,
            fire_right,
            equip_left,
            equip_right,
            respawn,
            ..PlayerCommand::default()
        };
        // Move byte exactly 48 (both fires, no move): retail
        // short-circuits sub_46840 WHOLE (:55759), freezing the held
        // strafe's decay. `Mc1Input` carries no fire state, so
        // pre-feed one decay quantum (the pose channel's emulation) —
        // the mover's decay lands back on the frozen value.
        if mb == 48 && ch.s.strafe != 0 {
            ch.s.strafe += 4 * ch.s.strafe.signum();
        }

        if args.pose_only {
            // Tier-2: fresh retail world context at N, chained flight.
            pose_only_pair_mc1(
                &mut world, &pristine, &timg, &pst, &st, ch, slot, pt, inp, stick_ok, &mut stats,
                &mut csv,
            )?;
        } else {
            step_mc1(&mut world, ch, inp, cmd);
            stats.seg().stepped += 1;
            // Grade at the boundary (capture-clean pairs only — a torn
            // snapshot grades nothing, the chain runs on regardless).
            if capture_clean(&pst, &obs) {
                let pose = pose_lanes_mc1(&ch.s, &st.ents[slot as usize], cw);
                let pin = PinnedMc1 {
                    slot,
                    local: pst.local_player,
                    player_count: pst.player_count,
                    pose: ch.pose(),
                };
                let port = world.obs_project_mc1(&pin);
                let mut pd = compare(&obs, &port, slot);
                append_hand_diffs(&mut pd, &st, &port, pst.local_player as usize);
                let dump = args.dump == Some(pt)
                    || (args.dump_first
                        && stats.seg().horizon.is_none()
                        && !(pose.is_empty() && pd.clean()));
                emit_replay_csv(&mut csv, pt, &pose, &pd)?;
                stats.grade(tick.t, &pose, &pd, args, dump);
            } else {
                stats.seg().ungraded += 1;
            }
        }
        stats.seg().end = tick.t;
        st_prev = Some((tick.t, st));
        if let Some(limit) = args.limit {
            if stats.segs.iter().map(|s| s.stepped).sum::<u64>() >= limit {
                break;
            }
        }
    }
    print!(
        "{}",
        stats.render(if args.pose_only { "pose-only" } else { "world" })
    );
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
    if let Some((h, ty, ceil)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil)
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
    stats.fold_pose_only(pt + 1, &pose, csv)
}

// --------------------------------------------------------------- MC2 run

fn run_mc2(path: &std::path::Path, args: &Args) -> Result<bool, String> {
    let mut rec = Recording::open(path)?;
    let level = rec.header.level.ok_or("recording has no level number")?;
    println!(
        "== replay {} (game mc2, level {level}{})",
        path.display(),
        if args.pose_only { ", pose-only" } else { "" }
    );
    let (mut world, pristine, things) = crate::verify_mc2::build_world_mc2(&args.baked, level)?;
    let mut csv = open_csv(args)?;
    let mut timg = (!args.no_terrain)
        .then(|| {
            rec.header
                .channels
                .terrain
                .as_ref()
                .map(mgc_formats::mgcr::TerrainImage::new)
        })
        .flatten();

    let mut stats = RStats::default();
    let mut st_prev: Option<(u64, RetailMc2)> = None;
    let mut chain: Option<(Chain, u16)> = None;
    // The respawn-key witnesses (verify_mc2::respawn_key_mc2 law).
    let mut prev_space = false;
    let mut prev_mouse: Option<(i16, i16)> = None;
    let mut printed_import = false;
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        if let (Some(img), Some(block)) = (timg.as_mut(), &tick.terrain) {
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
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        // Fire rides the CONSUMED move/fire byte on the pair's END
        // record (MC2 stamps it in PlayerEvents): measured strictly
        // stronger than the press-latch law — 560/560 retail arms
        // carry the bit on the same record, and the latch's extra
        // edges are UI clicks the byte correctly omits (ledger §THE
        // REPLAY VERIFIER). The respawn key still needs its witness
        // pair (no latch on the keyboard registers).
        let byte_fire = |p: &mgc_formats::mgcr::RetailPlayerMc2| {
            (p.move_bits & 0x10 != 0, p.move_bits & 0x20 != 0)
        };
        let press = press_pos_mc2(tick.input.as_ref());
        let space = respawn_key_mc2(tick.input.as_ref());
        let mouse = mouse_pos_mc2(tick.input.as_ref());
        let recentred = mouse.is_some() && mouse != prev_mouse && mouse == press;
        let respawn = space && (prev_space || recentred);
        prev_space = space;
        prev_mouse = mouse.or(prev_mouse);

        let anchor = match (&st_prev, &chain) {
            (Some((pt, _)), Some(_)) if tick.t == pt + 1 => false,
            _ => true,
        };
        if anchor {
            world.restore_planes(&pristine);
            world.restore_thing_table(&things);
            let report = world
                .retail_import_mc2(&st)
                .map_err(|e| format!("t={}: import: {e}", tick.t))?;
            // Measured planes AFTER the import (see the MC1 anchor).
            if let Some((h, ty, ceil)) = measured_planes(&timg) {
                world
                    .install_measured_terrain(h, ty, ceil)
                    .map_err(|e| format!("t={}: terrain: {e}", tick.t))?;
            }
            let (fl, fr) = byte_fire(&st.players[st.local_player as usize]);
            world.set_prev_fire(fl, fr);
            let row = world.mc2_carpet_row();
            chain = Some((
                Chain::seed_mc2(&st, report.human_slot, row),
                report.human_slot,
            ));
            stats.open(tick.t);
            if !printed_import {
                printed_import = true;
                println!(
                    "   import: human slot {}, terrain {}",
                    report.human_slot,
                    if measured_planes(&timg).is_some() {
                        "MEASURED"
                    } else {
                        "pristine"
                    }
                );
            }
            st_prev = Some((tick.t, st));
            continue;
        }
        let (pt, pst) = st_prev.take().expect("anchored");
        let (ch, human_slot) = chain.as_mut().expect("anchored");
        let slot = *human_slot;
        let pp = &pst.players[pst.local_player as usize];
        let cp = &st.players[st.local_player as usize];

        // ---- input for the pair pt → t ----
        let rec_stick = (
            recover_stick(pp.roll_acc as i16, cp.roll_acc as i16),
            recover_stick(pp.pitch_acc as i16, cp.pitch_acc as i16),
        );
        let stick_ok = matches!(rec_stick, (Some(_), Some(_)));
        let stick = match rec_stick {
            (Some(x), Some(y)) => (x, y),
            _ => {
                stats.stick_unrec += 1;
                (0, 0)
            }
        };
        // MC2 stamps the move byte in PlayerEvents — read the END
        // record.
        let inp = mc1_mover_input(cp.move_bits, stick);
        // Hand rebinds: a recorded hand change replays as the pane
        // select (tier = the recorded per-spell selection at N+1;
        // out-of-range spell = the unbind commit).
        let rebind = |hand: u8, prev: i16, cur: i16| -> Option<(u8, u8, u8)> {
            (prev != cur).then(|| {
                if (0..26i16).contains(&cur) {
                    (cur as u8, cp.sel[cur as usize], hand)
                } else {
                    (255, 0, hand)
                }
            })
        };
        let left = rebind(0, pp.hand_left, cp.hand_left);
        let right = rebind(1, pp.hand_right, cp.hand_right);
        let mc2_select = match (left, right) {
            (Some(l), Some(_)) => {
                stats.rebind_dropped += 1;
                Some(l)
            }
            (l, r) => l.or(r),
        };
        if mc2_select.is_some() {
            stats.equips += 1;
        }
        if respawn {
            stats.respawns += 1;
        }
        let (fire_left, fire_right) = byte_fire(cp);
        let cmd = PlayerCommand {
            fire_left,
            fire_right,
            mc2_select,
            respawn,
            ..PlayerCommand::default()
        };

        if args.pose_only {
            pose_only_pair_mc2(
                &mut world, &pristine, &things, &timg, &pst, &st, ch, slot, pt, inp, stick_ok,
                &mut stats, &mut csv,
            )?;
        } else {
            step_mc2(&mut world, ch, inp, cmd);
            stats.seg().stepped += 1;
            if capture_clean_mc2(&pst, &st) {
                let pose = pose_lanes_mc2(&ch.s, &st.ents[slot as usize], cp);
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
                let torn = torn_slots(&pst, &st);
                let pd = compare_mc2_gated(&obs, &port, slot, &torn);
                let dump = args.dump == Some(pt)
                    || (args.dump_first
                        && stats.seg().horizon.is_none()
                        && !(pose.is_empty() && pd.clean()));
                emit_replay_csv(&mut csv, pt, &pose, &pd)?;
                stats.grade(tick.t, &pose, &pd, args, dump);
            } else {
                stats.seg().ungraded += 1;
            }
        }
        stats.seg().end = tick.t;
        st_prev = Some((tick.t, st));
        if let Some(limit) = args.limit {
            if stats.segs.iter().map(|s| s.stepped).sum::<u64>() >= limit {
                break;
            }
        }
    }
    print!(
        "{}",
        stats.render(if args.pose_only { "pose-only" } else { "world" })
    );
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
    if let Some((h, ty, ceil)) = measured_planes(timg) {
        world
            .install_measured_terrain(h, ty, ceil)
            .map_err(|e| format!("t={pt}: terrain: {e}"))?;
    }
    let knock = consumed_knock(p0.knock_mag, p0.knock_dir, p1.knock_mag, p1.knock_dir);
    ch.ext.row = world.mc2_carpet_row();
    let w: &World = world;
    flight::mc2_move(
        &mut ch.s,
        &mut ch.ext,
        &inp,
        None,
        knock,
        &|x, y| w.ground_z_engine(x, y),
        &|x, y| w.player_cave_ceiling(x, y),
        &|cur, prop| w.player_mc2_gate(cur, prop),
        &|pos, latched| w.player_mc2_stuck(pos, latched),
    );
    let pose = pose_lanes_mc2(&ch.s, e1, p1);
    stats.fold_pose_only(pt + 1, &pose, csv)
}

// -------------------------------------------------------------- plumbing

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
