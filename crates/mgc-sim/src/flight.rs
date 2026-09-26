//! The player carpet flight models — the Phase-5 fidelity port.
//!
//! [`Mc1State`] + [`mc1_move`] are a direct import of remc1's human
//! carpet movement — sub_455D0_45910 (:55110) with its sub_46840
//! command integration (:55760-:55821) and the sub_45410 commit gate's
//! trailing z-floor (:55103-05) — in the engine's own integer units
//! (positions 8.8 fixed-point tiles on wrapping 16-bit axes, altitude
//! 256 = one tile of height, 11-bit angles, speeds in units/tick).
//! Line citations are remc1 sub_main.cpp.
//!
//! Key facts the port preserves verbatim:
//! - ROLL is a RATE input (yaw += filtered/8 per tick — an airplane
//!   stick: deflection turns, recenter to fly straight) while PITCH is
//!   an ABSOLUTE aim (the filtered value IS the 11-bit pitch angle,
//!   max ±254 ≈ ±44.6°). Both run the same low-pass
//!   `s += (2·input − s)/4` (:49017-20 deltas, :55143-44 integration),
//!   converging on 2× the raw ±127 input. Signed 16-bit throughout —
//!   remc1's `uint16` filter fields are a transcription bug (the
//!   sign-division idioms prove the original sign-extended).
//!   NOTE for future reviewers: the decompile's
//!   `(env − (my_sign32(env)<<2) + my_sign32(env)) >> 2` (:49018,
//!   EF:38061) IS plain trunc-toward-zero `/4` — `my_sign32` returns
//!   −1 for negative and 0 otherwise (engine_support.cpp:2962,
//!   Basic.cpp:3), a negative-indicator, NOT a signum. Reading it as
//!   a true ±1 signum invents a phantom asymmetric rounding law;
//!   same for the `−7·sign >> 3` yaw feed.
//! - Speed is command-driven: Up/Down step the TARGET ±16/tick held,
//!   clamp ±80, and the target HOLDS on release (no stop key, no
//!   decay — the authentic quantum-hunting standstill); actual speed
//!   chases the target in pure ±16 sign steps.
//! - The move is a true polar rotation (sub_41EC0 :52523): horizontal
//!   = speed·cos(pitch_eff), vertical = −speed·sin(pitch_eff). DIVES
//!   pass the raw aim pitch; CLIMB authority scales by altitude —
//!   full below ground+768, zero at the ground+1024 soft ceiling,
//!   INVERTED above (pitching up pushes you down) — so lasting
//!   altitude comes only from terrain rising underneath (the
//!   wall-climb skill move). At speed 0 the polar step vanishes:
//!   hover holds altitude exactly, except the 8/tick sink above the
//!   soft ceiling. Level flight (pitch 0) holds any altitude.
//! - The effective pitch is PERSISTENT state: the s==0/v6==0 branch
//!   leaves it stale (:55163-92) and the step consumes it anyway.
//! - Strafe is its own ±80 speed at yaw+512: ±16/tick held, −4/tick
//!   decay on release with a snap to 0 on sign flip.
//! - The Accelerate spell writes BOTH the target and actual speed
//!   (3×80 held / 2×80 released, :65171-78) from its token's walk
//!   slot, BEFORE the command integration — a resisting press then
//!   steps the boosted target back into the ±80 band (one tick of
//!   act-speed chase from ±160) and arms the v_14 speed-touched
//!   latch (:55780); the token reads the latch on its NEXT pass and
//!   ends the burst (counter = 1, :65146-50), restoring target AND
//!   actual to +80 max forward — even out of backwards flight
//!   (:65191-97), an authentic quirk.
//! - One RNG draw exists in the move: every 64th tick a private-LCG
//!   roll (`9377·r + 9439`, :55294-99) fires the wind-gust FLUTTER
//!   (sound 46) on `r % 11 == 0` — sound-only, but the draw mutates
//!   state, so it is replicated for fidelity.
//!
//! The enhanced mover (hold-to-fly) stays float-based in `lib.rs` as a
//! deliberate deviation; both obey the rule that aim pitch never steals
//! meaningful mobility (the faithful model's cos shrink maxes at ~29%,
//! and thrust stays fully live while aiming).
//!
//! [`Mc2Ext`] + [`mc2_move`] are the Phase-4.4 `FlightVerb::Mc2` arm —
//! remc2's `sub_5D530` (EF:59610) with its `sub_5F380` command
//! integration (EF:60748) and `moveTest_5D0A0` commit gate (EF:59429,
//! supplied by the world through [`Mc2GateOut`]); the full trace is
//! docs/traces/mc2-flight-model.md. The speed/strafe/pose halves are
//! MC1's verbatim (same 16/±80/−4 constants — trace §4c); what
//! differs: the row-data climb ramp (band 1024 open / 3072 cave, row
//! 66/104 — NOT row 59, trace §0.1), the ground+256 clearance with the
//! always-on row-`0xe` buoyancy sink, the water/cave gate that zeroes
//! target speed on CAVE refusal, the `sub_5DD50` 128-unit nudge, and
//! the slow/mobilize debuff channels (the spider-web tint/stun).
//! Deliberately unported here (cited, banked): the trailing
//! cave-ambient/water-loop sound block (EF:59776-59850,
//! presentation). `sub_5DE30`'s duel leash (worklist item 7) landed
//! as block 7 — the world computes it, the mover applies it. MC2's tick makes NO flutter roll (MC1's :55294-99
//! LCG is replaced by that sound block; the draw is carpet-private
//! state, so omitting it moves no world golden).

use crate::engine::features::Gen;

/// Faithful carpet state (the human entity + Type_160 fields we use).
#[derive(Debug, Clone, Copy, Default, Hash)]
pub struct Mc1State {
    /// Position in engine units (8.8 tiles, wrapping like the
    /// original's 16-bit axes).
    pub x: u16,
    pub y: u16,
    /// Altitude in engine units (256 = one tile of height).
    pub z: i16,
    /// 11-bit heading (+30; 0 = north/-Z like the rest of the sim).
    pub yaw: u16,
    /// The low-passed roll/pitch stick pair (Type_160 +327/+329),
    /// SIGNED (see the module note on remc1's transcription bug).
    pub roll_f: i16,
    pub pitch_f: i16,
    /// Published aim pitch (+32): the filtered pitch masked to 11
    /// bits. This is what casts aim along; the camera renders HALF
    /// of it (:52434).
    pub aim_pitch: u16,
    /// Effective (authority-scaled) pitch fed to the polar step —
    /// persistent because the original leaves it stale on the
    /// speed-0/pitch-0 branch (:55163-92).
    pub eff_pitch: u16,
    /// Actual forward speed (+126) and the Up/Down target it chases
    /// (Type_160 v_12), units/tick.
    pub act_speed: i16,
    pub tgt_speed: i16,
    /// Strafe speed (Type_160 v_16), the second polar step at yaw+512.
    pub strafe: i16,
    /// Entity tick counter (var_u8_29858_63) + private LCG
    /// (rand_29799_4) — the every-64th-tick flutter roll.
    pub tick_ctr: u8,
    pub rand: u32,
}

/// Retail's 11-bit bearing between two engine-unit points (the Gen
/// atan helper), exposed for boundary drivers that mirror the app's
/// dead-camera turn outside this crate (the conformance replay).
pub fn angle_between(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
    crate::engine::features::Gen::angle_between(ax, ay, bx, by)
}

/// `MoveEntity_57FA0` — the engine's polar step, exposed for boundary
/// drivers that displace the human's pose OUTSIDE the mover. The
/// doomsday pyramid's hurl-away beam (`sub_21AB0` case 7, EF:13444)
/// is one: it runs at the PYRAMID's pool slot and rewrites the
/// player's position before `sub_5D530` ever sees it.
pub fn move_entity(pos: &mut (u16, u16, i16), yaw: u16, pitch: u16, dist: i16) {
    crate::engine::features::Gen::polar_step(pos, yaw, pitch, dist);
}

impl Mc1State {
    /// Seed the integer state from tile-space floats (spawn/level
    /// hand-off; the reverse mapping runs after every move).
    pub fn from_tiles(x: f32, z_map: f32, alt: f32, yaw: f32) -> Self {
        const TAU: f32 = std::f32::consts::TAU;
        Mc1State {
            x: (x.rem_euclid(256.0) * 256.0) as u16,
            y: (z_map.rem_euclid(256.0) * 256.0) as u16,
            z: (alt * 256.0) as i16,
            yaw: (yaw.rem_euclid(TAU) * (2048.0 / TAU)) as u16 & 0x7FF,
            ..Default::default()
        }
    }

    /// Signed aim pitch in engine angle units (positive = DOWN, the
    /// original's convention; mouse-forward dives, like a stick).
    pub fn aim_signed(&self) -> i16 {
        let v = self.aim_pitch as i32;
        (if v > 1024 { v - 2048 } else { v }) as i16
    }
}

/// A/B toggle for the whirlwind crank's PHASE against `sub_5D530`'s
/// stop veto: set `MGC_NO_MC2_WW_CRANK_BEFORE_VETO` to restore the
/// pre-dig placement, where the staged `Mc2Ext::whirl_bumps` drain sat
/// BELOW the `byte[1] & 8` early return (`NETHERW.EXE` 0x81d39) and a
/// vetoed tick therefore lost the crank the funnel had already stored
/// at 0x57c86.
fn no_mc2_ww_crank_before_veto() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_CRANK_BEFORE_VETO").is_some())
}

/// One tick of player commands for the faithful mover, mapped from
/// [`crate::FlightInput`] by the sim boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mc1Input {
    /// Raw stick, ±127 (the original's mouse offset from screen
    /// center; the filter targets 2× this).
    pub stick_x: i16,
    pub stick_y: i16,
    /// Up/Down = target-speed impulses (command bits 1/2).
    pub speed_up: bool,
    pub speed_down: bool,
    /// Left/Right strafe (command bits 4/8).
    pub strafe_left: bool,
    pub strafe_right: bool,
    /// THE DEATH-FALL DISPATCH SKIPS THE COMMAND HANDLER. Retail's
    /// carpet runs `sub_46840` + `sub_455D0` only from states 0/1; the
    /// falling arm `sub_45FC0` (:55463) calls `sub_455D0` ALONE, so the
    /// speed target and the strafe register FREEZE at their last values
    /// while the STICK keeps feeding the filters — the stick words live
    /// in the input pass, which never stops. mc1l42 t=17306-17344 reads
    /// exactly that: `tgt`/`act` pinned at −80 and `strafe` frozen at 36
    /// for the whole fall, with roll/pitch/yaw still moving.
    pub no_command: bool,
    /// MC2: a modal UI (big map / spell book) has the carpet PARKED —
    /// retail keeps playing but stops the carpet dead: speeds snap to
    /// 0, no translation, no buoyancy, while the pose filters keep
    /// integrating the re-centred cursor (mc2l0 t=598: speed 80→0 in
    /// one boundary, x/y/z pinned ~200 ticks, yaw −1/tick).
    pub mc2_park: bool,
    /// MC2 (dig Q7): `sub_5D530`'s FORCED-STOP one-shot, `byte[1] & 8`
    /// — shipped `NETHERW.EXE` 0x81d39 `mov ah,[ebx+0xd]` / `test
    /// ah,0x8` / `and dl,0xf7` / `<epilogue>`, i.e. EF:59951-55
    /// `if (byte[1] & 8) { byte[1] &= 0xF7; return; }`.
    ///
    /// ⚠ IT VETOES `sub_5D530` ONLY. `sub_5F380`'s command
    /// integration runs at EF:60302, a separate call TWENTY-SEVEN
    /// statements ahead of `sub_5D530(a1x)` at EF:60329, so the target
    /// speed and the strafe register keep stepping through a stop —
    /// which is exactly what mc2l30 records while the whirlwind holds
    /// the wizard (t=2986-2999: `pose.strafe` 48→44→40→56→72→80 and
    /// `pose.tgt_speed` 64→48→…→−64 both keep marching while x/y/z and
    /// `act_speed` are frozen). It is the mirror image of
    /// [`Self::no_command`], which skips the command pass and keeps
    /// the mover.
    pub mc2_stop: bool,
}

/// What the move reports back to the sim boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mc1Moved {
    /// The wind-gust flutter roll fired (sound 46).
    pub flutter: bool,
    /// A speed press MOVED the target this tick — retail's Type_160
    /// v_14 latch (:55762-80, armed only when the press steps v_12
    /// inside its bounds test). The speed-spell tokens read it the
    /// NEXT walk pass and end their burst on it (sub_56380 :65146-50).
    pub speed_touched: bool,
    /// The axis sub_45410 leaves in retail's global scratch
    /// `word_AE454_AE444`, floor-clamped (:55103-05) — EQUAL to the
    /// settled pose when the move committed, and the refused second
    /// cardinal when it did not. The death-fall trail spawns here
    /// (:55478), which is the only reason it has to survive the
    /// refusal; see [`crate::mc1::Gen::player_wall_slide`].
    pub scratch: (u16, u16, i16),
}

/// The faithful human move: sub_46840's command integration followed
/// by sub_455D0's move, in the original's statement order. `ground`
/// returns terrain height in engine units at an 8.8 position; `gate`
/// is the sub_45410 wall gate minus its z-floor (the floor is applied
/// here, :55103-05), returning `(commits, scratch)` — `false` discards
/// the whole move (x, y AND z freeze; the sink and any slide are lost
/// with it, verbatim) but the scratch it hands back still stands, and
/// the death trail spawns there.
/// `accel_over` = the Accelerate spell's signed factor (±3 held / ±2
/// released); `knock` = this tick's buffet displacement (direction,
/// magnitude), already decayed by the caller.
/// ⭐ THE WIZARD'S `+128` IS NOT A REGISTER — IT IS A GLOBAL, RE-STAMPED
/// EVERY TICK. `sub_45C90_45FD0`, the class-3 carpet dispatch, opens
/// with an UNCONDITIONAL `a1x->actSpeed_29923_128 = dword_93A90`
/// (`reference/remc1/sub_main.cpp:55343`, twin `remc1hw:51411`) before
/// it calls `sub_46840_46B80` and hence before the mover, and
/// `dword_93A90` is the file-scope `int dword_93A90 = 80` (:4209,
/// hw :3913) with **no store anywhere in either decompile** — the same
/// global that bounds the commanded speed `v_12` at :55766-79, which is
/// this file's own `±80` target clamp. The human's own constructor
/// stamps it too (`sub_37820_37BE0` :44189 `= 80`).
/// ⭐ MEASURED, not assumed: every `class64 == 3` record in the whole
/// `mc1l48` duel window (2,231 ticks × 1,000 slots) carries `+128 == 80`
/// for model 0 (the human, slot 681) and model 1 (rivals 706/712), and
/// `0` for the models that never reach this dispatch (2 = castle, 3).
/// So `3 * +128 / 2 = 120` is the duel grip's cap for every wizard.
pub const MC1_WIZ_SPEED_CAP: i16 = 80;

/// The duel grip's per-tick payload — the victim's settled position,
/// the armed hold distance (`Type_160 +318`) and the caster's `+128`
/// ([`MC1_WIZ_SPEED_CAP`]). `yaw_drag` carries D12's
/// `MGC_NO_MC1_DUEL_YAW_DRAG` kill switch into the mover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mc1DuelGrip {
    /// The victim record's `+72` triple, read BEFORE this move.
    pub vpos: (u16, u16, i16),
    /// `Type_160 +318` — `clamp(dist3d, 1024, 3072)` at the last arm.
    pub hold: u32,
    /// The caster's `+128`.
    pub max_speed: i16,
    /// Apply the heading servo (:55244-46) as well as the step.
    pub yaw_drag: bool,
}

/// ⭐ THE DUEL GRIP'S HEADING SERVO — `sub_455D0`'s lock tail
/// (`reference/remc1/sub_main.cpp:55244-46`), the half of the grip the
/// port never had:
///
/// ```text
///   v24 = sub_42150_42490(&self+72, &victim+72);            // bearing
///   v25 = +30 + sub_422A0_425E0(+30, v24, 5, 0x82u);        // cap 130
///   HIBYTE(v25) &= 7u;  a1x->var_u16_29825_30 = v25;
/// ```
///
/// The write lands on the ENTITY (`+30`), not on the move scratch, so
/// `sub_45410`'s commit gate can never refuse it; and it sits BELOW
/// every polar step, so the tick's own translation still uses the
/// PRE-drag yaw. Both positions are the pre-move `+72` values — the
/// entity's own position is only committed after this block.
/// `a3 = 5` is dead (`sub_422A0_425E0` :52689 ignores it).
pub fn mc1_duel_turn(yaw: u16, sx: u16, sy: u16, vx: u16, vy: u16) -> u16 {
    let bearing = Gen::angle_between(sx, sy, vx, vy);
    let turn = Gen::turn_step(yaw, bearing, 0x82);
    ((yaw as i32 + turn as i32) & 0x7FF) as u16
}

/// `sub_42340_42680` — the duel's 3-D separation, the operand of BOTH
/// the release test (:55232) and the closing rate (:55238). ⚠ NOT the
/// 2-D `Gen::dist2_sq` the port's pre-D18 block used.
pub fn mc1_duel_dist(spos: (u16, u16, i16), vpos: (u16, u16, i16)) -> i32 {
    let dx = (vpos.0 as i16).wrapping_sub(spos.0 as i16) as i32;
    let dy = (vpos.1 as i16).wrapping_sub(spos.1 as i16) as i32;
    let dz = vpos.2 as i32 - spos.2 as i32;
    Gen::isqrt((dx * dx + dy * dy + dz * dz) as u32) as i32
}

/// The duel grip's TRANSLATION (`sub_455D0` :55236-47) — the pose
/// channel's shadow step, and (through [`mc1_move_duel`]) the world's:
///
/// ```text
///   v21 = sub_42340_42680(&self+72, &victim+72);   // 3-D distance
///   v22 = 3 * a1x->actSpeed_29923_128 / 2;         // +128, not +126
///   v23 = (v21 - Type_160+318) / (1024 / v22);     // closing rate
///   clamp v23 to ±v22
///   sub_41EC0_42200(&scratch, v24, a1x->var_u16_29827_32, v23);
/// ```
///
/// ⚠ The rate is SIGNED — inside the hold distance retail PUSHES the
/// caster away — the speed register is `+128` (the commanded max, a
/// per-tick re-stamp of the global [`MC1_WIZ_SPEED_CAP`]), not the
/// live `+126`, and the step carries the caster's published AIM PITCH,
/// not 0. Returns the servoed yaw ([`mc1_duel_turn`]).
#[allow(clippy::too_many_arguments)]
pub fn mc1_duel_tail(
    cand: &mut (u16, u16, i16),
    yaw: u16,
    aim_pitch: u16,
    spos: (u16, u16, i16),
    vpos: (u16, u16, i16),
    hold: u32,
    max_speed: i16,
) -> u16 {
    let dist = mc1_duel_dist(spos, vpos);
    let v22 = 3 * max_speed as i32 / 2;
    if v22 != 0 {
        let denom = 1024 / v22;
        let mut v23 = if denom != 0 {
            (dist - hold as i32) / denom
        } else {
            0
        };
        v23 = v23.clamp(-v22, v22);
        let bearing = Gen::angle_between(spos.0, spos.1, vpos.0, vpos.1);
        Gen::polar_step(cand, bearing, aim_pitch, v23 as i16);
    }
    mc1_duel_turn(yaw, spos.0, spos.1, vpos.0, vpos.1)
}

pub fn mc1_move(
    st: &mut Mc1State,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    ground: &dyn Fn(u16, u16) -> i16,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> (bool, (u16, u16, i16)),
) -> Mc1Moved {
    mc1_move_duel(st, inp, accel_over, knock, None, ground, gate)
}

/// [`mc1_move`] with `sub_455D0`'s LOCK TAIL in its own seat — the
/// duel grip runs between the knock add (:55219-25) and the commit
/// gate (:55249), so the step is gated and z-floored with the rest of
/// the move exactly as retail's is. See [`mc1_duel_tail`].
#[allow(clippy::too_many_arguments)]
pub fn mc1_move_duel(
    st: &mut Mc1State,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    duel: Option<Mc1DuelGrip>,
    ground: &dyn Fn(u16, u16) -> i16,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> (bool, (u16, u16, i16)),
) -> Mc1Moved {
    mc1_move_duel_with(st, inp, accel_over, knock, duel, ground, gate, None)
}

/// MC1's climb ramp (:55151-67): authority `z − ground − 1024`
/// clamped ±256 folds into the effective pitch (climbs ramp, dives
/// pass raw); a carpet AT REST above the 1024 soft ceiling sinks 8 —
/// returned as the candidate altitude. Reads the published
/// `aim_pitch`; shared with the enhanced kernel.
pub(crate) fn mc1_climb_ramp(st: &mut Mc1State, ground: &dyn Fn(u16, u16) -> i16, s: i16) -> i16 {
    let g = ground(st.x, st.y) as i32; // :55151, at the pre-move position
    let v5 = (st.z as i32 - g - 1024).clamp(-256, 256);
    let mut v6 = st.aim_pitch as i32;
    if v6 > 1024 {
        v6 -= 2048;
    }
    if s != 0 && v6 != 0 {
        let dive = (s > 0 && v6 > 0) || (s < 0 && v6 < 0);
        st.eff_pitch = if dive {
            st.aim_pitch
        } else {
            ((((v6 * -v5) / 256) as i16) as u16) & 0x7FF
        };
    } else if s == 0 && st.z as i32 > g + 1024 {
        return st.z - 8;
    }
    st.z
}

/// [`mc1_move_duel`] with an optional [`CarpetPropel`] kernel in
/// place of the faithful propulsion (the speed keys, the pose filter,
/// the speed chase, the climb ramp and the forward/strafe steps).
#[allow(clippy::too_many_arguments)]
pub fn mc1_move_duel_with<'p>(
    st: &mut Mc1State,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    duel: Option<Mc1DuelGrip>,
    ground: &dyn Fn(u16, u16) -> i16,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> (bool, (u16, u16, i16)),
    propel: Option<&mut (dyn CarpetPropel + 'p)>,
) -> Mc1Moved {
    let mut dir: i16 = 0;
    let mut cand = if let Some(k) = propel {
        let climb = |st: &mut Mc1State, s: i16| {
            let z = mc1_climb_ramp(st, ground, s);
            let mut p = (st.x, st.y, z);
            Gen::polar_step(&mut p, st.yaw, st.eff_pitch, s);
            p.2
        };
        k.propel(st, (0, 0), false, &climb)
    } else {
        // The Accelerate override writes BOTH the target and the actual
        // speed (:65171-78) — and it lands BEFORE the command integration:
        // retail's write happens at the spell TOKEN's walk slot, below the
        // carpet dispatch, so the brake press below steps a boosted
        // ±160/±240 target (and clamps it back into the ±80 band) while
        // the actual speed starts its chase from the boosted value.
        if let Some(k) = accel_over {
            let v = (k * 80.0) as i16; // 3×80 held / 2×80 released, signed
            st.tgt_speed = v;
            st.act_speed = v;
        }

        // ---- sub_46840 (:55760-:55821): command integration, pre-move ----
        // ⚠ THE WHOLE BLOCK IS SKIPPED ON THE DEATH FALL (`no_command`):
        // retail dispatches state 2 to `sub_45FC0`, which calls `sub_455D0`
        // without it, so the target speed and the strafe register freeze
        // where the last live tick left them — including the strafe's
        // 4/tick release decay, which is why a carpet that dies mid-strafe
        // keeps sliding sideways all the way down.
        //
        // Up/Down step the target ±16/tick held, clamp ±80 (:55766-80).
        // A press that MOVES the target arms the v_14 latch (:55780) —
        // during a boost only the RESISTING press passes the bounds test
        // (the boosted target sits outside the ±80 band), and the latch is
        // what ends the burst at the token's next pass (:65146-50).
        if !inp.no_command {
            if inp.speed_up && st.tgt_speed < 80 {
                dir = 1;
            }
            if inp.speed_down && st.tgt_speed > -80 {
                dir = -1;
            }
            if dir != 0 {
                st.tgt_speed = (st.tgt_speed + 16 * dir).clamp(-80, 80);
            }
            // Strafe: ±16/tick held clamp ±80 (:55783-96); released, decay
            // 4/tick toward 0 with a sign-flip snap (:55800-19). The bit
            // tests are SEQUENTIAL (:55783-86) — both strafes held resolves
            // to RIGHT, never to release (pose-channel-measured on mc1l0
            // t=3189/3305: retail steps +16 under the 0xE move byte).
            let mut sdir: i16 = 0;
            if inp.strafe_left {
                sdir = -1;
            }
            if inp.strafe_right {
                sdir = 1;
            }
            if sdir != 0 {
                st.strafe = (st.strafe + 16 * sdir).clamp(-80, 80);
            } else if st.strafe != 0 {
                let s = st.strafe.signum();
                st.strafe -= 4 * s;
                if st.strafe.signum() != s {
                    st.strafe = 0;
                }
            }
        }

        // ---- sub_455D0 (:55110), statement order ----
        // (a) filter integration + yaw from filtered roll (:55143-46).
        st.roll_f += ((2 * inp.stick_x as i32 - st.roll_f as i32) / 4) as i16;
        st.pitch_f += ((2 * inp.stick_y as i32 - st.pitch_f as i32) / 4) as i16;
        st.yaw = ((st.yaw as i32 + st.roll_f as i32 / 8) & 0x7FF) as u16;

        // (b) actual speed chases the target in ±16 sign steps (:55147-50).
        let d = st.tgt_speed - st.act_speed;
        if d != 0 {
            st.act_speed += d.signum() * 16;
        }

        // (c) vertical: climb authority + aim publication (:55151-95).
        let mut cand = (st.x, st.y, st.z);
        st.aim_pitch = (st.pitch_f as u16) & 0x7FF; // published +32 (:55158-60)
        // v5 = z − ground − 1024 clamped ±256 folds the climb; the speed-0
        // sink above the soft ceiling (:55171-72) lands in the candidate.
        cand.2 = mc1_climb_ramp(st, ground, st.act_speed);
        // The polar step (:55196): horizontal = s·cos(eff), z −= s·sin(eff).
        Gen::polar_step(&mut cand, st.yaw, st.eff_pitch, st.act_speed);

        // (d) strafe: second polar step at yaw+512, pitch 0 (:55197-203).
        if st.strafe != 0 {
            Gen::polar_step(&mut cand, st.yaw.wrapping_add(512) & 0x7FF, 0, st.strafe);
        }
        cand
    };

    // (e) knock displacement (v_22/v_24, :55204-19; decay lives with
    // the caller's Type_160 emulation).
    if let Some((kdir, kmag)) = knock {
        Gen::polar_step(&mut cand, kdir, 0, kmag);
    }

    // (e2) THE DUEL GRIP (:55226-48) — retail's lock tail sits HERE,
    // after the knock mailbox is drained into the scratch and BEFORE
    // `sub_45410`'s commit gate, so its step is gated and z-floored
    // with the rest of the move. Both position operands are the
    // PRE-move `+72` values (the entity is only re-seated at
    // :55250-52), and the pitch is this tick's published `+32`.
    if let Some(g) = duel {
        let servoed = mc1_duel_tail(
            &mut cand,
            st.yaw,
            st.aim_pitch,
            (st.x, st.y, st.z),
            g.vpos,
            g.hold,
            g.max_speed,
        );
        if g.yaw_drag {
            st.yaw = servoed;
        }
    }

    // (f) commit gate + unconditional z-floor ground+128 (row v_12)
    // at the FINAL candidate (:55250-52, :55103-05). A fully blocked
    // move commits nothing — not even the sink.
    //
    // ⚠ THE Z-FLOOR IS NOT INSIDE THE GATE. :55103-05 sits after every
    // branch of sub_45410 and clamps the SCRATCH, committed or not,
    // against the ground under the SCRATCH — which on a refusal is the
    // blocking cell, and blocking cells are the ones that rise. Only
    // the three `st` writes below are conditional (:55251-52).
    let (commit, mut scratch) = gate((st.x, st.y, st.z), cand);
    let floor = ground(scratch.0, scratch.1).saturating_add(128);
    if scratch.2 < floor {
        scratch.2 = floor;
    }
    if commit {
        st.x = scratch.0;
        st.y = scratch.1;
        st.z = scratch.2;
    }

    // (g) the every-64th-tick flutter roll on the entity's private
    // LCG (:55294-99) — sound-only, but the draw is state. The test
    // reads the +63 clock BEFORE this tick's bump: retail's handler
    // has no increment (:55294 tests the settled value; the dispatch
    // bumps +63 after), pose-channel-measured on mc1l0 — every draw
    // landed one pair late under the post-increment order.
    let mut flutter = false;
    if st.tick_ctr & 0x3F == 0 {
        st.rand = st.rand.wrapping_mul(9377).wrapping_add(9439);
        flutter = st.rand % 0xB == 0;
    }
    st.tick_ctr = st.tick_ctr.wrapping_add(1);
    Mc1Moved {
        flutter,
        speed_touched: dir != 0,
        scratch,
    }
}

/// The MC2 carpet's `str_D7BD6` tuning row — `AddPlayer_4A920`
/// explicitly overwrites the generic default with row 104 on cave
/// maps, row 66 otherwise (EF:33329-32; trace §0.1 — row 59 is the
/// pre-overwrite default and must NOT be used).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mc2Row {
    /// `word_160_0xa_10`: the climb-ramp band / soft-ceiling offset.
    pub band: i16,
    /// `word_160_0xc_12`: ground clearance (the z-floor offset).
    pub clearance: i16,
    /// `word_160_0xe_14`: the always-on buoyancy step above the
    /// clearance band (negative = sink).
    pub buoyancy: i16,
}

impl Mc2Row {
    /// Row 66 (L:78): open (day/night) maps.
    pub const OPEN: Mc2Row = Mc2Row {
        band: 1024,
        clearance: 256,
        buoyancy: -16,
    };
    /// Row 104 (L:116): cave maps — triple climb band, gentler sink.
    pub const CAVE: Mc2Row = Mc2Row {
        band: 3072,
        clearance: 256,
        buoyancy: -8,
    };
}

impl Default for Mc2Row {
    fn default() -> Self {
        Mc2Row::OPEN
    }
}

/// The MC2-only carpet channels (trace §5) layered over [`Mc1State`]
/// — the shared pose/speed/strafe state stays in the MC1 struct so
/// the renderer/camera derivation is model-agnostic.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mc2Ext {
    /// `moveSpeed_0x14C_332` (0..3): the stagger/web SLOW — scales
    /// the pose delta, forward and strafe speed by (4−n)/4. Drives
    /// the GREEN screen tint (`SetPaletteModification_5C830` subMod
    /// 3: R/B darkened `56*count>>8`, count = `171*n/3+85` — the
    /// manticore-spit poison cast; presentation reads it).
    pub move_speed: u8,
    /// `moveSpeedCounter_0x14D_333`: 8-tick decay counter per level.
    pub move_speed_ctr: u8,
    /// `mobilizeCounter_0x14E_334`: the FULL-STOP web (stun) — all
    /// speed forced 0, −51/tick settle toward the ground.
    pub mobilize: u8,
    /// `mobilizeCounter2_0x150_336`: 10-tick decay counter.
    pub mobilize_ctr: u8,
    /// `xAdd/yAdd/zAdd_0x1A6/8/A`: one-shot world-space displacement
    /// mailbox — external systems write, the move applies once and
    /// clears (EF:59713-18).
    pub add: (i16, i16, i16),
    /// `waterCounter_0x262_610`: ++ on a wet predicted tile (in the
    /// gate), −− each tick; gates the water-flight sound loop.
    pub water_ctr: u16,
    /// `byte_0x261_609`: the "nudging out of a wall" latch
    /// (`sub_5DD50`).
    pub nudge_latch: bool,
    /// The tuning row (selected by map type at level hand-off).
    pub row: Mc2Row,
    /// ⭐ `sub_33340`'s camera-roll cranks owed to THIS tick's mover
    /// (EF:24344-46, `roll_0x155_341 += 28` while under 256), one per
    /// visit that reached the `v40` block. Retail lands them at the
    /// FUNNEL's slot, between the input pass's `rollDelta` (EF:38336-37
    /// / 0x77480, which reads the PRE-crank accumulator) and
    /// `sub_5D530`'s `roll += rollDelta` (0x81d6d) — so the mover's own
    /// filter delta must be computed from the un-cranked value and the
    /// crank folded in beside it. Set by the carpet's dispatch from
    /// [`PlayerWhirl::bumps`], drained here.
    pub whirl_bumps: u8,
    /// ⭐ `sub_3A200`'s class-3 model-0 pitch seizure owed to THIS
    /// tick's mover (`pitch = pitch_acc = 512`, `NETHERW.EXE` 0x5ea55 /
    /// 0x5ea5b) — `whirl_bumps`' phase and lifetime, on the pitch lane.
    /// See `engine::features::no_mc2_flood_human_spin`.
    pub flood_spin: bool,
}

/// ⚠ `whirl_bumps` IS HASH-TRANSPARENT, exactly like the snapshot
/// codec's exclusion of it: the carpet's own dispatch sets it and the
/// mover in the SAME call drains it, so it is always 0 at every
/// boundary a state hash is taken at — and hashing it would move every
/// existing `flight_tier_golden_state_hashes` pin for a field that can
/// never be observed there (measured: it does, on all eight). Every
/// other field hashes exactly as the old `derive(Hash)` did, in
/// declaration order. ⛔ DO NOT PUT `Hash` BACK IN THE DERIVE.
impl std::hash::Hash for Mc2Ext {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Mc2Ext {
            move_speed,
            move_speed_ctr,
            mobilize,
            mobilize_ctr,
            add,
            water_ctr,
            nudge_latch,
            row,
            whirl_bumps: _,
            flood_spin: _,
        } = self;
        move_speed.hash(state);
        move_speed_ctr.hash(state);
        mobilize.hash(state);
        mobilize_ctr.hash(state);
        add.hash(state);
        water_ctr.hash(state);
        nudge_latch.hash(state);
        row.hash(state);
    }
}

impl Mc2Ext {
    /// The slow/mobilize decay walk (`sub_5D530` EF:59722-43, block
    /// 8) — split out so the enhanced (deviation) mover can service
    /// the debuff channels on the same cadence.
    pub fn tick_debuffs(&mut self) {
        if self.move_speed > 0 {
            self.move_speed_ctr = self.move_speed_ctr.wrapping_sub(1);
            if self.move_speed_ctr == 0 {
                self.move_speed -= 1;
                if self.move_speed > 0 {
                    self.move_speed_ctr = 8;
                }
            }
        }
        if self.mobilize > 0 {
            self.mobilize_ctr = self.mobilize_ctr.wrapping_sub(1);
            if self.mobilize_ctr == 0 {
                self.mobilize -= 1;
            }
        }
    }

    /// A debuff-stamp SLOW hit (`sub_38E70` EF:28407-17): ramp one
    /// level to the cap 3, re-arm the 8-tick counter.
    pub fn slow_hit(&mut self) {
        if self.move_speed < 3 {
            self.move_speed += 1;
        }
        self.move_speed_ctr = 8;
    }

    /// A debuff-stamp PARALYZE hit (`sub_38F70` EF:28442-43): latch
    /// the full-stop with its 10-tick counter (the −80 backward kick
    /// rides the knock channel at the stamp).
    pub fn stun_hit(&mut self) {
        self.mobilize = 1;
        self.mobilize_ctr = 10;
    }

    /// The `(4−moveSpeed)/4` slow scale (round toward zero), applied
    /// to pose deltas and speeds while the web slow is active.
    fn slow_scale(&self, v: i32) -> i32 {
        (v * (4 - self.move_speed as i32)) / 4
    }
}

/// What the world's `moveTest_5D0A0` gate hands back to [`mc2_move`].
#[derive(Debug, Clone, Copy)]
pub struct Mc2GateOut {
    /// `Some((committed candidate, yaw turn))` — the candidate may
    /// have been slid along a water cardinal or steered around a cave
    /// wall (the yaw turn is the cave steer-assist's ±(17·i)/6,
    /// EF:59578-84; 0 otherwise). `None` = the move is refused.
    pub pass: Option<((u16, u16, i16), i16)>,
    /// The predicted tile was deep water (`waterCounter`++, EF:59480).
    pub wet: bool,
    /// The refusal happened in-cave: target speed zeroes and the
    /// speed-up spell cancels (EF:59599-605 — the block runs after
    /// the non-cave early return at EF:59513, so open-level water
    /// refusals do NOT zero speed).
    pub zero_speed: bool,
}

/// What [`mc2_move`] reports back to the sim boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mc2Moved {
    /// A cave refusal cancelled the speed-up spell (retail clears the
    /// `SpellEnabled[3]` manifestation's `word_0x2E_46`, EF:59603 —
    /// MC2 spell 3 = the accelerate channel).
    pub accel_cancel: bool,
    /// `word_0xe_14` — retail's "a speed KEY stepped the command this
    /// frame" flag, set by `sub_5F380` (EF:60790) whenever the guarded
    /// ±16 integration actually moved the target. MC1 has published it
    /// as `Mc1Moved::speed_touched` all along; MC2 computed `dir` and
    /// threw it away.
    ///
    /// ⭐⭐ IT IS THE SPEED SPELL'S BRAKE-CANCEL, AND THE GUARD IS WHAT
    /// MAKES IT DIRECTIONAL. `GetScroll_69DB0` collapses its window to
    /// one tick when this is set, and the integration only steps while
    /// the target is inside ±80 — so against a FORWARD boost (target
    /// +160/+240/+320) the up key cannot step and only the down key
    /// arms it, and against a BACKWARD boost only the up key does.
    /// That is the manual's "press the down cursor to cancel", falling
    /// out of the bounds test rather than a separate rule.
    pub speed_touched: bool,
}

/// DIG5 PROBE: `MGC_MC2_MOVE_TRACE=1` block-by-block mover trace.
fn mc2_move_trace(f: impl FnOnce() -> String) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var("MGC_MC2_MOVE_TRACE").is_ok()) {
        println!("{}", f());
    }
}

/// The faithful MC2 human move — `sub_5F380`'s command integration
/// followed by `sub_5D530` in the original's statement order (trace
/// docs/traces/mc2-flight-model.md). `ground` is `getTerrainAlt`;
/// `ceiling` returns the cave clamp target `ceiling − 384` (None
/// off-cave — [`crate::engine::world::World::player_cave_ceiling`]);
/// `gate` is `moveTest_5D0A0` (water slide + cave steer, world-side);
/// `stuck` is `sub_5DD50`'s wedged test at the CURRENT position
/// (water / sealed / latched-and-colliding). `accel_over` and `knock`
/// ride the same channels as [`mc1_move`] — the MC2 knock constants
/// (cap 128, decay −4, snap <4; EF:59695-711) equal the MC1 channel's,
/// and `moveBoost` IS that channel's retail home. `leash` is the DUEL
/// pull the world armed for this dispatch
/// ([`crate::engine::world::World::mc2_duel_enforce`] — `sub_5DE30`,
/// applied at block 7 below).
#[allow(clippy::too_many_arguments)]
pub fn mc2_move(
    st: &mut Mc1State,
    ext: &mut Mc2Ext,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    leash: Option<(u16, i16)>,
    ground: &dyn Fn(u16, u16) -> i16,
    ceiling: &dyn Fn(u16, u16) -> Option<i16>,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> Mc2GateOut,
    stuck: &dyn Fn((u16, u16, i16), bool) -> bool,
) -> Mc2Moved {
    mc2_move_with(
        st, ext, inp, accel_over, knock, leash, ground, ceiling, gate, stuck, None,
    )
}

/// The swappable PROPULSION stage of both faithful movers
/// ([`mc1_move_duel_with`], [`mc2_move_with`]): how the player's input
/// becomes this tick's candidate position. The faithful movers run
/// retail's blocks inline; the enhanced controls plug a kernel in
/// here and inherit EVERYTHING else — MC2's whirlwind crank, the
/// well's pitch seizure, the stop veto, the knock, the displacement
/// mailbox, the duel leash/grip, the web decay, the commit gate, the
/// vertical resolution, MC1's carpet flutter — so a world interaction
/// can never again reach one control mode and not the other (the
/// 2026-09-25 parity audit: webs, both duels, Speed cancel,
/// gravity-well pull, whirl spin, flutter were all missing).
pub trait CarpetPropel {
    /// Step the pose (yaw, pitch, speed) from the player's input and
    /// return the candidate position — `st.x/y/z` stay the CURRENT
    /// position for the gate.
    ///
    /// `web` = MC2's (slow level, paralyze) — (0, 0) in MC1.
    /// `seized_pitch`: MC2's gravity-well seizure wrote
    /// `st.pitch_f`/`aim_pitch` this tick. `climb(st, s)` is the
    /// game's own pitch→altitude law (retail's climb ramp, MC1's
    /// at-rest sink included) for forward speed `s`: it updates
    /// `st.eff_pitch` and returns the candidate altitude.
    fn propel(
        &mut self,
        st: &mut Mc1State,
        web: (u8, u8),
        seized_pitch: bool,
        climb: &dyn Fn(&mut Mc1State, i16) -> i16,
    ) -> (u16, u16, i16);

    /// The heading the WORLD sees is the hull yaw plus this offset
    /// (11-bit units): the enhanced controls cast along the crosshair,
    /// not the hull (player ruling).
    fn aim_heading_offset(&self) -> i16 {
        0
    }
}

/// Block (2)'s climb ramp (EF:59645-66): authority −256..+256
/// normalized by the row band, folded into the effective pitch with
/// the four-quadrant raw/ramped law (climb toward the band ramps,
/// dives pass raw). Reads the published `aim_pitch`; `s` is the
/// forward speed this tick. Shared with the enhanced kernel.
pub(crate) fn mc2_climb_ramp(
    st: &mut Mc1State,
    ext: &Mc2Ext,
    ground: &dyn Fn(u16, u16) -> i16,
    s: i16,
) {
    let g = ground(st.x, st.y) as i32;
    let band = ext.row.band as i32;
    let alt_diff = (((st.z as i32 - g - band) << 10) / band).clamp(-256, 256);
    let mut v6 = st.aim_pitch as i32;
    if v6 > 1024 {
        v6 -= 2048;
    }
    if s != 0 && v6 != 0 {
        let dive = (s > 0 && v6 > 0) || (s < 0 && v6 < 0);
        st.eff_pitch = if dive {
            st.aim_pitch
        } else {
            // Round-toward-zero fold (the −sign·255 >> 8 idiom).
            ((((v6 * -alt_diff) / 256) as i16) as u16) & 0x7FF
        };
    }
}

/// [`mc2_move`] with an optional [`CarpetPropel`] kernel in place of the
/// faithful propulsion blocks.
#[allow(clippy::too_many_arguments)]
pub fn mc2_move_with<'p>(
    st: &mut Mc1State,
    ext: &mut Mc2Ext,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    leash: Option<(u16, i16)>,
    ground: &dyn Fn(u16, u16) -> i16,
    ceiling: &dyn Fn(u16, u16) -> Option<i16>,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> Mc2GateOut,
    stuck: &dyn Fn((u16, u16, i16), bool) -> bool,
    propel: Option<&mut (dyn CarpetPropel + 'p)>,
) -> Mc2Moved {
    let mut moved = Mc2Moved::default();

    // The modal park (big map / spell book): the input pass zeroes
    // BOTH speed registers before the mover runs, and the mover then
    // runs normally — fwd 0 freezes x/y, the buoyancy sink settles z
    // (mc2l0 t=937→938: one −16 step, then constant), and the pose
    // filters keep integrating the re-centred cursor.
    if inp.mc2_park {
        st.act_speed = 0;
        st.tgt_speed = 0;
    }

    // ---- sub_5F380 (EF:60748): command integration, pre-move ----
    // Identical numbers to MC1's sub_46840 (trace §4c: the D4B8x
    // constants match 16/±80/−4 exactly).
    //
    // ⚠ THE WHOLE BLOCK IS SKIPPED ON THE DEATH FALL (`no_command`),
    // the exact MC1 law one column over. Retail dispatches class 3 by
    // `actionIndex`, and state 2 goes to `sub_5E310` (EF:60045) whose
    // FIRST statement is `sub_5D530` — the command handler is never
    // reached, so thrust, strafe and strafe's own 4/tick release decay
    // all freeze while the mover keeps running. Only `AddPlayer03_00`
    // (state 0) calls `sub_5F380`, at EF:59967, and the `life < 0`
    // flip to state 2 is that function's TAIL.
    if propel.is_none() && !inp.no_command {
        let mut dir: i16 = 0;
        if inp.speed_up && st.tgt_speed < 80 {
            dir = 1;
        }
        if inp.speed_down && st.tgt_speed > -80 {
            dir = -1;
        }
        if dir != 0 {
            st.tgt_speed = (st.tgt_speed + 16 * dir).clamp(-80, 80);
            moved.speed_touched = true; // word_0xe_14 = 1 (EF:60790)
        }
        // Sequential bit tests like MC1's (EF:60793-96) — both strafes
        // held resolves to RIGHT, never to release.
        let mut sdir: i16 = 0;
        if inp.strafe_left {
            sdir = -1;
        }
        if inp.strafe_right {
            sdir = 1;
        }
        if sdir != 0 {
            st.strafe = (st.strafe + 16 * sdir).clamp(-80, 80);
        } else if st.strafe != 0 {
            let s = st.strafe.signum();
            st.strafe -= 4 * s;
            if st.strafe.signum() != s {
                st.strafe = 0;
            }
        }
    }

    // The speed-up spell override (EF:56189 arms it; the channel's
    // shape is shared with MC1's Accelerate).
    if propel.is_none()
        && let Some(k) = accel_over
    {
        let v = (k * 80.0) as i16;
        st.tgt_speed = v;
        st.act_speed = v;
    }

    // ---- the FRAME-HEAD half, ABOVE `sub_5D530`'s early return ----
    // (0) pose: the filtered delta (EF:38060-66, ÷4 toward zero),
    // slow-scaled while the web slow is active (EF:59622-30), then
    // yaw as a RATE and the published absolute aim pitch.
    // A kernel steers by its own law, so the stick leaves roll alone:
    // the filter just relaxes it (the whirlwind crank's residual spin).
    let sx = if propel.is_some() {
        0
    } else {
        inp.stick_x as i32
    };
    let dr = ((2 * sx - st.roll_f as i32) / 4) as i16;
    let dp = if propel.is_some() {
        0 // the kernel owns pitch
    } else {
        ((2 * inp.stick_y as i32 - st.pitch_f as i32) / 4) as i16
    };
    // ⭐⭐⭐ THE FUNNEL'S CRANK LANDS BETWEEN THE INPUT PASS AND THE
    // MOVER — AND THE STOP VETO DOES NOT SWALLOW IT.
    // `rollDelta_0x4_4` is computed in `PlayerEvents_51BB0`
    // (EF:38336-37 / `NETHERW.EXE` 0x77480 `movsx eax,byte [eax+0x3]`
    // — the stick is a signed BYTE — then `movsx edx,[ecx+0x155]` and
    // `mov [ecx+0x4],ax`) from the accumulator AS IT STOOD AT THE
    // FRAME HEAD; `sub_33340` then cranks that accumulator by 28 per
    // seizure at the funnel's own slot (0x57c86) — a DIRECT STORE into
    // `[ebx+0xa4] -> +0x155`, nowhere near `sub_5D530`; `sub_5D530`
    // finally adds the stored delta (0x81d6d). The port fuses the
    // input pass into the mover, so the crank has to be folded in
    // HERE, after `dr` is taken off the un-cranked value.
    //
    // ⚠ AND *ABOVE* THE VETO. `sub_5D530`'s `byte[1] & 8` early exit
    // (0x81d39, the epilogue jump) skips `roll += rollDelta` — it
    // cannot skip a store the FUNNEL already made at its own slot. The
    // inner-lift arm raises BOTH on the same visit (`byte[1] |= 8` at
    // 0x57ca3 / 0x57ca6 and the crank at 0x57c86), so the vetoed tick
    // is exactly the tick that used to lose its crank. mc2l1 t=283 is
    // that tick to the unit: the recorded human record takes
    // `flags 525 -> 268435981` (`byte[3] |= 0x10`, the inner lift's
    // grab latch at 0x57d2c), `rand 50933 -> 47348` (the latch's own
    // `9377v + 9439` draw), `yaw 1522 -> 1578` (+56 = `v38`) and
    // `word_0x30_48 = 1578` (0x57d30-34) — a textbook inner lift — and
    // `roll_acc 183 -> 211` is the crank ALONE, with the vetoed
    // `rollDelta` contributing nothing. It was the take's last
    // deviation. Set `MGC_NO_MC2_WW_CRANK_BEFORE_VETO` to restore the
    // swallowed crank.
    if !no_mc2_ww_crank_before_veto() {
        let mut cranks = std::mem::take(&mut ext.whirl_bumps);
        while cranks > 0 {
            if st.roll_f < 256 {
                st.roll_f = st.roll_f.wrapping_add(28);
            }
            cranks -= 1;
        }
    }
    // ⭐⭐ THE QUAKE'S CLOSE BAND SEIZES THE PITCH, AT THE SAME PHASE.
    // `sub_3A200` stores 512 into the record pitch AND `pitch_acc`
    // (0x5ea55 / 0x5ea5b) at the quake's slot: after `dp` was taken off
    // the un-seized accumulator (0x774b2), before `sub_5D530` adds it
    // (0x81d7e), and a vetoed tick keeps both 512s.
    let seized_pitch = std::mem::take(&mut ext.flood_spin);
    if seized_pitch {
        st.pitch_f = 512;
        st.aim_pitch = 512;
    }

    // `sub_5D530`'s early return — everything from here down is
    // inside retail's `else` (see [`Mc1Input::mc2_stop`]). The
    // caller owns the `byte[1] &= 0xF7` clear.
    if inp.mc2_stop {
        return moved;
    }

    // ---- sub_5D530 (EF:59610), statement order ----
    if no_mc2_ww_crank_before_veto() {
        let mut cranks = std::mem::take(&mut ext.whirl_bumps);
        while cranks > 0 {
            if st.roll_f < 256 {
                st.roll_f = st.roll_f.wrapping_add(28);
            }
            cranks -= 1;
        }
    }
    if ext.move_speed > 0 {
        st.roll_f += ext.slow_scale(dr as i32) as i16;
        st.pitch_f += ext.slow_scale(dp as i32) as i16;
    } else {
        st.roll_f += dr;
        st.pitch_f += dp;
    }
    st.yaw = ((st.yaw as i32 + st.roll_f as i32 / 8) & 0x7FF) as u16; // EF:59635

    // Blocks (1)-(4) are PROPULSION — the one stage a swapped-in
    // [`CarpetPropel`] kernel replaces (the enhanced controls). Every
    // block before and after is shared by both control modes.
    let mut cand = if let Some(k) = propel {
        let row = ext.row;
        let climb = |st: &mut Mc1State, s: i16| {
            let ramp = Mc2Ext {
                row,
                ..Mc2Ext::default()
            };
            mc2_climb_ramp(st, &ramp, ground, s);
            let mut p = (st.x, st.y, st.z);
            Gen::polar_step(&mut p, st.yaw, st.eff_pitch, s);
            p.2
        };
        k.propel(st, (ext.move_speed, ext.mobilize), seized_pitch, &climb)
    } else {
        // (1) actual speed chases the target in ±16 sign steps
        // (EF:59636-44).
        let d = st.tgt_speed - st.act_speed;
        if d != 0 {
            st.act_speed += d.signum() * 16;
        }

        // (2) the climb ramp — the row-data band (EF:59645-66): authority
        // −256..+256 normalized by the band, folded into the effective
        // pitch with the same four-quadrant raw/ramped law as MC1 (climb
        // toward the band ramps, dives pass raw).
        let mut cand = (st.x, st.y, st.z);
        st.aim_pitch = (st.pitch_f as u16) & 0x7FF; // published (EF:59651-52)
        mc2_climb_ramp(st, ext, ground, st.act_speed);
        // (No speed-0 sink here — MC2's sink is the post-gate row-0xe
        // buoyancy; eff_pitch stays stale on the zero branches, verbatim.)

        // (3) forward polar step, slow/mobilize-scaled (EF:59668-80).
        let fwd = if ext.move_speed > 0 {
            ext.slow_scale(st.act_speed as i32) as i16
        } else if ext.mobilize > 0 {
            0
        } else {
            st.act_speed
        };
        Gen::polar_step(&mut cand, st.yaw, st.eff_pitch, fwd);
        mc2_move_trace(|| {
            format!(
                "  fwd yaw={} eff={} fwd={fwd} -> {cand:?}",
                st.yaw, st.eff_pitch
            )
        });

        // (4) strafe at yaw+512, same scaling (EF:59681-93).
        if st.strafe != 0 {
            let sf = if ext.move_speed > 0 {
                ext.slow_scale(st.strafe as i32) as i16
            } else if ext.mobilize > 0 {
                0
            } else {
                st.strafe
            };
            Gen::polar_step(&mut cand, st.yaw.wrapping_add(512) & 0x7FF, 0, sf);
            mc2_move_trace(|| format!("  strafe sf={sf} -> {cand:?}"));
        }
        cand
    };

    // (5) the moveBoost knockback impulse (EF:59695-711) — the cap
    // 128 / decay −4 / snap <4 law lives in the world's knock channel
    // (take_knock_step), identical math.
    if let Some((kdir, kmag)) = knock {
        Gen::polar_step(&mut cand, kdir, 0, kmag);
    }

    // (6) the one-shot displacement mailbox (EF:59713-18) + water
    // counter decay (EF:59719-20).
    cand.0 = cand.0.wrapping_add(ext.add.0 as u16);
    cand.1 = cand.1.wrapping_add(ext.add.1 as u16);
    cand.2 = cand.2.wrapping_add(ext.add.2);
    ext.add = (0, 0, 0);
    if ext.water_ctr > 0 {
        ext.water_ctr -= 1;
    }

    // (7) `sub_5DE30` (EF:59721) — THE DUEL LEASH, and it lives HERE
    // rather than on the world's knock channel for two reasons the
    // channel cannot honour. The yaw servo reads the yaw that block 0
    // ABOVE just integrated and writes it back — `moveBoost` has no
    // yaw authority at all — and the pull is a plain one-shot polar
    // step onto the CANDIDATE, so the commit gate can still refuse it
    // and nothing survives into the next tick (`moveBoost` caps at
    // 128, decays −4 and keeps pulling for ~20 ticks after the lock
    // breaks). Retail:
    //
    // ```text
    //   v9 = a1x->yaw + sub_58350(a1x->yaw, v7, 5, 0x82);
    //   HIBYTE(v9) &= 7u;  a1x->yaw = v9;
    //   MoveEntity_57FA0(&predictedAxis, v8, a1x->pitch_0x1E_30, v6);
    // ```
    //
    // — the step's heading is the RAW bearing `v8`, not the servoed
    // yaw (the carpet is dragged sideways while it slews to face its
    // opponent), and its pitch is the published aim pitch, so a duel
    // fought up a hill pulls in three dimensions.
    if let Some((bearing, pull)) = leash {
        let step = Gen::turn_step(st.yaw, bearing, 0x82) as i32;
        st.yaw = ((st.yaw as i32 + step) & 0x7FF) as u16;
        Gen::polar_step(&mut cand, bearing, st.aim_pitch, pull);
    }

    // (8) slow/mobilize decay (EF:59722-43).
    ext.tick_debuffs();

    // (9) the commit gate + vertical resolution (EF:59745-69).
    mc2_move_trace(|| format!("  pre-gate cand={cand:?} knock={knock:?} leash={leash:?}"));
    let out = gate((st.x, st.y, st.z), cand);
    mc2_move_trace(|| {
        format!(
            "  gate -> pass={:?} wet={} zero={}",
            out.pass, out.wet, out.zero_speed
        )
    });
    if out.wet {
        ext.water_ctr = ext.water_ctr.wrapping_add(1) & 0xFF;
    }
    // ⭐ EVERY REFUSED MOVE IN A CAVE BUMPS THE COUNTER, not only the
    // deep-water head branch above. `moveTest_5D0A0`'s refusal block
    // opens with `incb 0x262(%eax)` (NETHERW.EXE 0x81ccc), ahead of the
    // position rollback — and it is cave-only because 0x81a1c returns
    // before that block off-cave, which is why the lane only ever shows
    // on the map_type-2 takes. Found from the other side: remc2 lacks
    // the same instruction (`remc2-fix-watercounter-refused-move.patch`)
    // and the port had the identical hole, invisible because
    // `pose_lanes_mc2` does not grade `water_ctr`. The counter is a
    // BYTE in retail, so both bumps wrap at 256.
    if out.zero_speed {
        ext.water_ctr = ext.water_ctr.wrapping_add(1) & 0xFF;
    }
    match out.pass {
        Some((p, dyaw)) => {
            st.yaw = ((st.yaw as i32 + dyaw as i32) & 0x7FF) as u16;
            let g = ground(p.0, p.1) as i32;
            let clr = ext.row.clearance as i32;
            let mut z = p.2 as i32;
            if ext.mobilize > 0 {
                z -= 51; // settle while frozen (EF:59750)
            } else if z > g + clr {
                z += ext.row.buoyancy as i32; // the row-0xe sink (EF:59755)
            }
            if z >= g + clr {
                // Above the clearance band: the cave roof clamps (no
                // bounce, no damage — EF:59757-63), RAW like retail:
                // the commit gate guarantees every landed position
                // has an air band >= clearance+fov+384, so the clamp
                // interval never degenerates.
                if let Some(c) = ceiling(p.0, p.1) {
                    z = z.min(c as i32);
                }
            } else {
                z = g + clr; // floor clamp to ground+256 (EF:59768)
            }
            st.x = p.0;
            st.y = p.1;
            st.z = z.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
        None => {
            if out.zero_speed {
                st.tgt_speed = 0; // dead-stop into a cave wall (EF:59602)
                moved.accel_cancel = true;
            }
            // sub_5DD50 (EF:59854): the un-gated 128-unit forward
            // shove out of whatever the carpet is wedged in.
            if stuck((st.x, st.y, st.z), ext.nudge_latch) {
                ext.nudge_latch = true;
                let mut a = (st.x, st.y, st.z);
                Gen::polar_step(&mut a, st.yaw, 0, 128);
                st.x = a.0;
                st.y = a.1;
                st.z = a.2;
            } else {
                ext.nudge_latch = false;
            }
        }
    }
    moved
}

// ------------------------------------------------------------ snapshot

/// MC2's barrel roll (`sub_55C60`, remc2 EF:38879-969) — the
/// both-strafes dodge. A seven-phase spring-settle on the VIEW roll:
/// wind up through a full turn past the live bank, overshoot +68
/// units, swing back 35, settle forward at |bank|+2048 — plus the
/// move's one real mechanic, `sub_55EB0`'s homing-lock break, fired
/// once at phase 1 and once at the finish. The phase targets re-read
/// the LIVE |bank| every tick: the roll pins the bank stick centered
/// (retail zeroes `rollDelta` each tick), the carpet auto-levels
/// underneath, and the settle tracks it — which is also why the exit
/// is snap-free: the masked rest angle `(|bank|+2048) & 0x7FF` IS the
/// live bank. Angle units: 2048 = 360°.
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub struct BarrelRoll {
    /// `byte_0x846`: 0 = idle, 1..=7 = the phase walk, 8 = finish
    /// (never observed across ticks — 8 resolves the same tick).
    pub phase: u8,
    /// `byte_0x847`: spin direction, the bank's sign at phase 1.
    pub dir: i8,
    /// `word_0x848`: accumulated roll angle (unmasked; the view
    /// write masks to 0x7FF).
    pub angle: i16,
    /// `word_0x84A`: angular velocity, seeded 91, accel-clamped to
    /// [11, 68], halved at each phase crossing past 3 (unclamped —
    /// retail's shift runs outside the clamp block).
    pub vel: i16,
}

/// What one [`BarrelRoll::tick`] asks of the sim boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrollOut {
    /// Fire the homing-lock break (`sub_55EB0`) this tick.
    pub lock_break: bool,
    /// The view roll to publish, masked 0..2047; `None` on the
    /// finishing tick (retail skips the view write there — the
    /// normal bank-derived roll resumes).
    pub view: Option<u16>,
}

impl BarrelRoll {
    pub fn active(&self) -> bool {
        self.phase != 0
    }

    /// The case-6 start gate (EF:37628-29): arm only from idle — a
    /// roll in progress swallows the command.
    pub fn arm(&mut self) {
        if self.phase == 0 {
            self.phase = 1;
        }
    }

    /// One driver tick (`sub_55C60` body, statement order). `bank` =
    /// the carpet's live bank stick in angle units (retail
    /// `roll_0x155_341`); `raw_dx` = this tick's raw mouse-X counts —
    /// retail re-baselines `byteindex_220` every tick past phase 4,
    /// so its abort test is exactly a per-tick delta > 16.
    pub fn tick(&mut self, bank: i16, raw_dx: i16) -> BrollOut {
        let mut out = BrollOut::default();
        let n1 = (bank as i32).abs();
        let (p1, p2, p3): (i32, i16, i32) = match self.phase {
            1 | 2 => (1, -2, 1024),
            3 => (1, 1, n1 + 2048),
            4 => (1, -2, n1 + 2116),
            5 => (-1, 3, n1 + 2048),
            6 => (-1, -4, n1 + 2013),
            7 => (1, 5, n1 + 2048),
            _ => (0, 0, 0),
        };
        if self.phase == 1 {
            self.dir = if bank >= 0 { 1 } else { -1 };
            self.vel = 91;
            self.angle = n1 as i16;
            self.phase = 2;
            out.lock_break = true;
        }
        self.angle += (p1 * self.vel as i32) as i16;
        if p2 != 0 {
            self.vel = (self.vel + p2).clamp(11, 68);
        }
        let a = self.angle as i32;
        if (p1 > 0 && a >= p3) || (p1 < 0 && a <= p3) {
            self.phase += 1;
            if self.phase > 3 {
                self.vel >>= 1;
            }
        }
        if self.phase >= 4 && (raw_dx as i32).abs() > 16 {
            self.phase = 8;
        }
        if self.phase == 8 {
            out.lock_break = true;
            // Retail clears only the phase; the rest is dead state
            // until the next arm re-seeds it. Reset it all so idle
            // compares equal to pristine (the hash-quiet gate).
            *self = BarrelRoll::default();
        } else {
            out.view = Some(((self.angle as i32 * self.dir as i32) & 0x7FF) as u16);
        }
        out
    }
}

use crate::snapshot::{Reader, Snap, SnapshotError, Writer};

impl Snap for Mc1State {
    fn put(&self, w: &mut Writer) {
        let Mc1State {
            x,
            y,
            z,
            yaw,
            roll_f,
            pitch_f,
            aim_pitch,
            eff_pitch,
            act_speed,
            tgt_speed,
            strafe,
            tick_ctr,
            rand,
        } = self;
        w.put(x);
        w.put(y);
        w.put(z);
        w.put(yaw);
        w.put(roll_f);
        w.put(pitch_f);
        w.put(aim_pitch);
        w.put(eff_pitch);
        w.put(act_speed);
        w.put(tgt_speed);
        w.put(strafe);
        w.put(tick_ctr);
        w.put(rand);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Mc1State {
            x: r.get()?,
            y: r.get()?,
            z: r.get()?,
            yaw: r.get()?,
            roll_f: r.get()?,
            pitch_f: r.get()?,
            aim_pitch: r.get()?,
            eff_pitch: r.get()?,
            act_speed: r.get()?,
            tgt_speed: r.get()?,
            strafe: r.get()?,
            tick_ctr: r.get()?,
            rand: r.get()?,
        })
    }
}

impl Snap for Mc2Row {
    fn put(&self, w: &mut Writer) {
        let Mc2Row {
            band,
            clearance,
            buoyancy,
        } = self;
        w.put(band);
        w.put(clearance);
        w.put(buoyancy);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Mc2Row {
            band: r.get()?,
            clearance: r.get()?,
            buoyancy: r.get()?,
        })
    }
}

impl Snap for BarrelRoll {
    fn put(&self, w: &mut Writer) {
        let BarrelRoll {
            phase,
            dir,
            angle,
            vel,
        } = self;
        w.put(phase);
        w.put(dir);
        w.put(angle);
        w.put(vel);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(BarrelRoll {
            phase: r.get()?,
            dir: r.get()?,
            angle: r.get()?,
            vel: r.get()?,
        })
    }
}

impl Snap for Mc2Ext {
    fn put(&self, w: &mut Writer) {
        let Mc2Ext {
            move_speed,
            move_speed_ctr,
            mobilize,
            mobilize_ctr,
            add,
            water_ctr,
            nudge_latch,
            row,
            // Intra-tick only: the carpet's dispatch sets it and the
            // mover in the SAME call drains it, so it never crosses a
            // snapshot boundary and takes no SNAPSHOT_VERSION bump.
            whirl_bumps: _,
            flood_spin: _,
        } = self;
        w.put(move_speed);
        w.put(move_speed_ctr);
        w.put(mobilize);
        w.put(mobilize_ctr);
        w.put(add);
        w.put(water_ctr);
        w.put(nudge_latch);
        w.put(row);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Mc2Ext {
            move_speed: r.get()?,
            move_speed_ctr: r.get()?,
            mobilize: r.get()?,
            mobilize_ctr: r.get()?,
            add: r.get()?,
            water_ctr: r.get()?,
            nudge_latch: r.get()?,
            row: r.get()?,
            whirl_bumps: 0,
            flood_spin: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_ground(_: u16, _: u16) -> i16 {
        0
    }
    fn open_gate(_: (u16, u16, i16), p: (u16, u16, i16)) -> (bool, (u16, u16, i16)) {
        (true, p)
    }

    fn step(st: &mut Mc1State, inp: &Mc1Input) -> Mc1Moved {
        mc1_move(st, inp, None, None, &flat_ground, &open_gate)
    }

    /// THE DEATH FALL SKIPS THE COMMAND HANDLER (`no_command`).
    /// Retail dispatches a falling carpet to `sub_45FC0`, which calls
    /// `sub_455D0` ALONE — so the speed target and the strafe register
    /// freeze where the last live tick left them (no ±16 step, and no
    /// 4/tick release decay either) while the STICK, which lives in
    /// the input pass and never stops, keeps feeding the filters.
    /// mc1l42 t=17306-17344 measures both halves at once: `tgt`/`act`
    /// pinned at −80 and `strafe` frozen at 36 for the whole fall,
    /// with roll/pitch/yaw still moving under the player's hand.
    #[test]
    fn the_death_fall_freezes_the_speed_target_and_the_strafe() {
        let live = |no_command: bool| {
            let mut st = Mc1State {
                tgt_speed: -80,
                act_speed: -80,
                strafe: 36,
                ..Default::default()
            };
            // A stick that is still moving, and a speed press + no
            // strafe key: the command block would step the target and
            // decay the strafe, the move block would filter the stick.
            let inp = Mc1Input {
                stick_x: 40,
                stick_y: 40,
                speed_up: true,
                no_command,
                ..Default::default()
            };
            step(&mut st, &inp);
            st
        };
        let cmd = live(false);
        assert_eq!(cmd.tgt_speed, -64, "the live command steps the target");
        assert_eq!(cmd.strafe, 32, "and decays the released strafe 4/tick");

        let fall = live(true);
        assert_eq!(fall.tgt_speed, -80, "the falling target FREEZES");
        assert_eq!(fall.strafe, 36, "and so does the strafe register");
        assert_eq!(
            fall.act_speed, -80,
            "the actual speed still chases its (frozen) target"
        );
        assert_eq!(
            (fall.roll_f, fall.pitch_f),
            (cmd.roll_f, cmd.pitch_f),
            "the stick filters run either way — sub_455D0 always does"
        );
    }

    /// sub_45410's z-floor (:55103-05) sits AFTER every branch, so it
    /// clamps the scratch whether or not the move commits, and the
    /// scratch it clamps survives a refusal (:55092-100 leaves the
    /// second cardinal standing while `v10 = 0`). Only the three
    /// pose writes are conditional (:55251-52). The carpet's
    /// death-fall trail spawns at that scratch (:55478), so a refused
    /// slide is not a no-op — it relocates the trail.
    ///
    /// Reversion-probed: applying the floor inside the commit arm (as
    /// the port did) drops the refused z to 900 and regresses
    /// mc1l32-quick's t=19134 head.
    #[test]
    fn the_z_floor_clamps_the_scratch_even_when_the_slide_is_refused() {
        let ground_1000 = |_: u16, _: u16| -> i16 { 1000 };
        let entry = Mc1State {
            x: 5000,
            y: 6000,
            z: 900,
            ..Default::default()
        };
        let idle = Mc1Input {
            no_command: true,
            ..Default::default()
        };

        // REFUSED: the gate hands back a slide axis it did NOT commit.
        let refused = (4096u16, 7000u16, 900i16);
        let mut st = entry;
        let moved = mc1_move(&mut st, &idle, None, None, &ground_1000, &|_, _| {
            (false, refused)
        });
        assert_eq!(
            (st.x, st.y, st.z),
            (entry.x, entry.y, entry.z),
            "a refused move commits nothing — not even the sink"
        );
        assert_eq!(
            moved.scratch,
            (refused.0, refused.1, 1128),
            "but the scratch stands, floor-clamped to ground+128 at the \
             SCRATCH's own cell"
        );

        // COMMITTED: scratch and settled pose are the same value, so
        // the trail spawn is unchanged on every ordinary fall tick.
        let mut st = entry;
        let moved = mc1_move(&mut st, &idle, None, None, &ground_1000, &|_, p| (true, p));
        assert_eq!(
            moved.scratch,
            (st.x, st.y, st.z),
            "a committed move leaves the pose EQUAL to the scratch"
        );
        assert_eq!(st.z, 1128, "and the floor still applies on commit");
    }

    /// The barrel roll from level flight: two lock-break pulses (the
    /// arm tick and the finish), a full tumble through 180°, the
    /// spring-settle back to rest, and a clean idle state after —
    /// the retail phase walk end to end (sub_55C60).
    #[test]
    fn barrel_roll_full_tumble_and_two_lock_breaks() {
        let mut br = BarrelRoll::default();
        assert!(!br.active());
        br.arm();
        assert!(br.active());
        let (mut breaks, mut ticks, mut past_half) = (0u32, 0u32, false);
        while br.active() {
            let out = br.tick(0, 0);
            breaks += u32::from(out.lock_break);
            if let Some(v) = out.view {
                assert!(v < 2048, "view is masked to the 11-bit circle");
                // The tumble visits the inverted band (a real 360°,
                // not a wobble).
                past_half |= (900..1150).contains(&v);
            }
            ticks += 1;
            assert!(ticks < 400, "the phase machine must terminate");
        }
        assert_eq!(breaks, 2, "lock break at phase 1 AND at the finish");
        assert!(past_half, "the roll passes through inverted");
        // 24 Hz ticks: the settle takes a couple of seconds.
        assert!((20..200).contains(&ticks), "duration sane: {ticks}");
        assert_eq!(br, BarrelRoll::default(), "idle state is pristine");
        // Re-arming works from idle only (EF:37628).
        br.arm();
        assert_eq!(br.phase, 1);
        br.arm();
        assert_eq!(br.phase, 1, "a running roll swallows the command");
    }

    /// Direction follows the bank's sign at the arm tick; a hard
    /// mouse grab (>16 raw counts in a tick) past phase 4 aborts
    /// straight to the finish, still firing the second lock break.
    #[test]
    fn barrel_roll_direction_and_mouse_abort() {
        let mut br = BarrelRoll::default();
        br.arm();
        br.tick(-200, 0);
        assert_eq!(br.dir, -1, "negative bank spins negative");
        assert_eq!(br.vel, 68, "seed 91 accel-clamps to 68 immediately");

        let mut br = BarrelRoll::default();
        br.arm();
        let mut ticks = 0;
        while br.active() && br.phase < 4 {
            br.tick(0, 0);
            ticks += 1;
            assert!(ticks < 400);
        }
        assert!(br.active(), "phase 4 reached while still rolling");
        // Small drift never aborts; the grab does, on the spot.
        let out = br.tick(0, 16);
        assert!(br.active() && !out.lock_break, "16 is under the gate");
        let out = br.tick(0, 17);
        assert!(out.lock_break, "the abort fires the finish pulse");
        assert!(!br.active());
        assert!(out.view.is_none(), "the finish tick skips the view write");
    }

    #[test]
    fn speed_target_persists_after_release() {
        let mut st = Mc1State {
            z: 128,
            ..Default::default()
        };
        let up = Mc1Input {
            speed_up: true,
            ..Default::default()
        };
        for _ in 0..3 {
            step(&mut st, &up);
        }
        assert_eq!(st.tgt_speed, 48);
        let idle = Mc1Input::default();
        for _ in 0..50 {
            step(&mut st, &idle);
        }
        // No decay, no stop: the target (and the chased actual speed)
        // hold forever — the authentic no-stop-key handling.
        assert_eq!(st.tgt_speed, 48);
        assert_eq!(st.act_speed, 48);
    }

    #[test]
    fn stick_is_a_rate_not_a_position() {
        let mut st = Mc1State {
            z: 128,
            ..Default::default()
        };
        let left = Mc1Input {
            stick_x: -127,
            ..Default::default()
        };
        for _ in 0..20 {
            step(&mut st, &left);
        }
        let yaw_after_hold = st.yaw;
        assert_ne!(yaw_after_hold, 0, "deflection turns");
        // Recentering the stick decays the filter; the yaw settles at
        // SOME heading and stays (no snap back).
        let centered = Mc1Input::default();
        for _ in 0..60 {
            step(&mut st, &centered);
        }
        // The truncating decay authentically parks at |s| ≤ 3 — below
        // the yaw step's s/8 threshold, so turning still stops dead.
        assert!(
            st.roll_f.abs() <= 3,
            "filter parks near center: {}",
            st.roll_f
        );
        let settled = st.yaw;
        for _ in 0..20 {
            step(&mut st, &centered);
        }
        assert_eq!(st.yaw, settled, "turning stops once recentered");
    }

    #[test]
    fn strafe_decays_on_release() {
        let mut st = Mc1State {
            z: 128,
            ..Default::default()
        };
        let right = Mc1Input {
            strafe_right: true,
            ..Default::default()
        };
        for _ in 0..10 {
            step(&mut st, &right);
        }
        assert_eq!(st.strafe, 80);
        let idle = Mc1Input::default();
        for _ in 0..19 {
            step(&mut st, &idle);
        }
        assert_eq!(st.strafe, 4);
        step(&mut st, &idle);
        assert_eq!(st.strafe, 0, "sign-flip snap to rest");
    }

    /// Both strafes held resolves to RIGHT — retail's bit tests are
    /// sequential (:55783-86 / EF:60793-96), never a release. Caught
    /// by the pose channel on mc1l0 (t=3189/3305: the recorded strafe
    /// column steps +16 under move byte 0xE where the old match
    /// decayed it).
    #[test]
    fn both_strafes_held_resolve_right_not_release() {
        let both = Mc1Input {
            strafe_left: true,
            strafe_right: true,
            ..Default::default()
        };
        let mut st = Mc1State {
            z: 128,
            strafe: -40,
            ..Default::default()
        };
        step(&mut st, &both);
        assert_eq!(st.strafe, -24, "mc1: right wins from a left drift");
        let mut st2 = Mc1State {
            z: 128,
            strafe: -40,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        step2(&mut st2, &mut ext, &both);
        assert_eq!(st2.strafe, -24, "mc2: same sequential resolve");
    }

    #[test]
    fn hover_holds_below_ceiling_sinks_above() {
        // Below the soft ceiling (ground 0, z = 512): exact hover.
        let mut st = Mc1State {
            z: 512,
            ..Default::default()
        };
        let idle = Mc1Input::default();
        for _ in 0..30 {
            step(&mut st, &idle);
        }
        assert_eq!(st.z, 512);
        // Above it (z = 2048): 8/tick sink at speed 0.
        st.z = 2048;
        step(&mut st, &idle);
        assert_eq!(st.z, 2040);
    }

    #[test]
    fn climb_authority_inverts_above_soft_ceiling() {
        // Full authority low: aiming up (negative pitch) climbs.
        let mut st = Mc1State {
            z: 256,
            act_speed: 80,
            tgt_speed: 80,
            ..Default::default()
        };
        let aim_up = Mc1Input {
            stick_y: -127,
            ..Default::default()
        };
        for _ in 0..20 {
            step(&mut st, &aim_up);
        }
        assert!(st.z > 256, "climbs below the band, z = {}", st.z);
        // But never through the soft ceiling band: authority hits 0
        // at ground+1024 and inverts above.
        for _ in 0..300 {
            step(&mut st, &aim_up);
        }
        assert!(
            st.z <= 1024 + 80,
            "the soft ceiling is unescapable by pitch, z = {}",
            st.z
        );
        // Well above the band (a wall-climb dash-away): pitching UP
        // pushes DOWN (inverted authority).
        st.z = 2048;
        let before = st.z;
        for _ in 0..10 {
            step(&mut st, &aim_up);
        }
        assert!(st.z < before, "inverted climb sinks, z = {}", st.z);
    }

    #[test]
    fn level_flight_holds_any_altitude() {
        // Pitch exactly 0 while moving: no vertical term at all, even
        // far above the soft ceiling — the wall-climb dash-away.
        let mut st = Mc1State {
            z: 4096,
            act_speed: 80,
            tgt_speed: 80,
            ..Default::default()
        };
        let fwd = Mc1Input::default();
        for _ in 0..100 {
            step(&mut st, &fwd);
        }
        assert_eq!(st.z, 4096);
    }

    #[test]
    fn floor_rides_rising_ground() {
        // Ground staircase: the z-floor (ground+128) carries the
        // carpet up a wall face.
        let stair = |x: u16, _: u16| -> i16 { ((x >> 8) as i16) * 64 };
        let mut st = Mc1State {
            z: 128,
            act_speed: 80,
            tgt_speed: 80,
            yaw: 512,
            ..Default::default()
        };
        for _ in 0..200 {
            mc1_move(
                &mut st,
                &Mc1Input::default(),
                None,
                None,
                &stair,
                &open_gate,
            );
        }
        let g = stair(st.x, st.y);
        assert!(st.z >= g + 128, "rides the floor: z {} ground {}", st.z, g);
        assert!(st.z > 1024, "gained real altitude from terrain");
    }

    #[test]
    fn accelerate_override_bypasses_chase() {
        let mut st = Mc1State {
            z: 128,
            ..Default::default()
        };
        let idle = Mc1Input::default();
        mc1_move(&mut st, &idle, Some(3.0), None, &flat_ground, &open_gate);
        assert_eq!(st.act_speed, 240);
        assert_eq!(st.tgt_speed, 240);
        mc1_move(&mut st, &idle, Some(2.0), None, &flat_ground, &open_gate);
        assert_eq!(st.act_speed, 160);
        // The expiry reset to +80 max forward (:65191-97) is the sim
        // boundary's edge-detection job — see the lib.rs test.
    }

    #[test]
    fn aim_pitch_costs_bounded_mobility() {
        // Full dive aim: horizontal speed shrinks by cos(±44.6°) ≈
        // 0.71, never worse (the flat-plane rule holds within 29% —
        // load-bearing for combat dodging).
        let mut st = Mc1State {
            z: 20000,
            act_speed: 80,
            tgt_speed: 80,
            ..Default::default()
        };
        let dive = Mc1Input {
            stick_y: 127,
            ..Default::default()
        };
        // Let the filter converge (target 254).
        for _ in 0..40 {
            step(&mut st, &dive);
        }
        let y0 = st.y;
        step(&mut st, &dive);
        let dy = y0.wrapping_sub(st.y) as i16 as i32; // yaw 0 = -y
        assert!(dy >= 55, "horizontal survives full dive: {dy} of 80");
        assert!(dy <= 80);
    }

    // ---- the MC2 arm (sub_5D530 / moveTest_5D0A0) ----

    fn open_gate2(_: (u16, u16, i16), p: (u16, u16, i16)) -> Mc2GateOut {
        Mc2GateOut {
            pass: Some((p, 0)),
            wet: false,
            zero_speed: false,
        }
    }
    fn no_ceiling(_: u16, _: u16) -> Option<i16> {
        None
    }
    fn never_stuck(_: (u16, u16, i16), _: bool) -> bool {
        false
    }

    fn step2(st: &mut Mc1State, ext: &mut Mc2Ext, inp: &Mc1Input) -> Mc2Moved {
        mc2_move(
            st,
            ext,
            inp,
            None,
            None,
            None,
            &flat_ground,
            &no_ceiling,
            &open_gate2,
            &never_stuck,
        )
    }

    /// ⭐⭐⭐ THE CLIMB-BAND AUTHORITY IS A TERRAIN READ, SO ITS PHASE
    /// IS THE CARPET'S OWN WALK SLOT. `sub_5D530` probes
    /// `getTerrainAlt_10C40(&predictedAxis)` (`NETHERW.EXE` file
    /// 0x81e57 `push 0x1b398` / 0x81e5f `call 0x35440` = VA 0x10C40)
    /// and folds it straight into `altDiff` (0x81e6a `movsx edx,word
    /// [0x1b39c]` / 0x81e71 `cwde` / 0x81e76-7a `sub/sub/shl 10` /
    /// 0x81e82 `idiv ecx`), clamped ±256 (0x81e89-0x81ea2), then
    /// `eff_pitch = trunc(tempPitch·−altDiff / 256)` on the two climb
    /// quadrants (0x81ed6 / 0x81f13). One HEIGHT BYTE of terrain is 32
    /// engine units, so a same-tick terraform the carpet cannot yet
    /// see moves the published effective pitch by tens of units.
    ///
    /// RETAIL MEASUREMENT (recordings/mc2l24.mgcr pair 26209→26210,
    /// 2026-09-10). The human at (20753, 35039, 2342), `act_speed`
    /// −80, `pitch_acc` 105, stick_y 49, band 1024 — a nose-down
    /// reverse, i.e. the `actSpeed < 0 && tempPitch > 0` ramped arm.
    /// The (10,42) painter at slot 701 — ABOVE the human's slot 116 —
    /// lowers the cell (81,136) neighbourhood by 2 height bytes that
    /// tick: `getTerrainAlt` is 1550 at the carpet's slot and 1486
    /// once the painter has run. Retail published `eff_pitch` 94; the
    /// post-painter plane yields 68, which is exactly what the pose
    /// lane reported before it was phased to terrain@N.
    #[test]
    fn mc2_climb_band_authority_is_phased_at_the_carpets_own_slot() {
        let at = |g: i16| {
            let mut st = Mc1State {
                x: 20753,
                y: 35039,
                z: 2342,
                yaw: 577,
                roll_f: -4,
                pitch_f: 105,
                aim_pitch: 105,
                eff_pitch: 105,
                act_speed: -80,
                tgt_speed: -80,
                ..Default::default()
            };
            let mut ext = Mc2Ext::default();
            let inp = Mc1Input {
                stick_x: -8,
                stick_y: 49,
                ..Default::default()
            };
            mc2_move(
                &mut st,
                &mut ext,
                &inp,
                None,
                None,
                None,
                &|_, _| g,
                &no_ceiling,
                &open_gate2,
                &never_stuck,
            );
            (st.aim_pitch, st.eff_pitch)
        };
        // The published aim pitch is terrain-blind: 105 + trunc((2·49 −
        // 105)/4) = 104 either way. Only the AUTHORITY fold moves.
        assert_eq!(at(1550), (104, 94), "terrain@N (the carpet's slot)");
        assert_eq!(at(1486), (104, 68), "terrain@N+1 (after the painter)");
    }

    #[test]
    fn mc2_buoyancy_sinks_to_clearance_and_holds() {
        // The row-0xe sink runs whenever above ground+256 (unlike
        // MC1's speed-0-above-band-only sink) and parks AT the
        // clearance floor.
        let mut st = Mc1State {
            z: 512,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let idle = Mc1Input::default();
        for _ in 0..16 {
            step2(&mut st, &mut ext, &idle);
        }
        assert_eq!(st.z, 256, "sank 16/tick to ground+clearance");
        step2(&mut st, &mut ext, &idle);
        assert_eq!(st.z, 256, "the floor clamp holds");
    }

    /// THE DOCKED-AT-YOUR-CASTLE VIEW HEIGHT — the whole chain the
    /// player judges when the carpet parks on its own castle mound:
    /// pad top → carpet floor → EYE.
    ///
    /// RETAIL MEASUREMENT (recordings/mc2l0.mgcr, the human's own
    /// (3,2) castle at tile (48,34)): the BUILD00 painter walks the
    /// castle entity's z 1644 → 2336 over t=2564..2583 and it holds
    /// there for the rest of the take — the castle re-pins to live
    /// ground every tick, so 2336 = 73 height bytes × 32 IS the pad
    /// top. The mover then floors the carpet at `pad + clearance`
    /// (EF:59768) and the world draw is handed `carpet + 128`
    /// (EF:21575), so retail's eye over that mound is 2720. MC1 halves
    /// the clearance (:55151, row `v_12`) and keeps the same +128 eye
    /// (remc1 sub_main.cpp:26406) — 2592 over the same mound.
    ///
    /// The corpus pins both clearances independently: the mc2l0 spawn
    /// pose sits at z 5024 over a 149-byte cell (149·32 + 256) and the
    /// human parks at z 256 over sea-level ground through
    /// t=5683..5758; the mc1l0 spawn pose sits at z 2080 over a
    /// 61-byte cell (61·32 + 128).
    #[test]
    fn docked_on_a_castle_pad_floors_and_lifts_the_eye() {
        // The measured mc2l0 mound: 73 height bytes.
        const PAD: i16 = 73 * 32;
        let pad_ground = |_: u16, _: u16| -> i16 { PAD };
        let eye = |carpet: i16| (carpet as f32 / 256.0 + crate::EYE_LIFT) * 256.0;

        // MC2: clearance 256 (row 66). Approach from below — the floor
        // clamp is the unconditional else-arm, so one commit settles it.
        let mut st = Mc1State {
            z: 0,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let idle = Mc1Input::default();
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            None,
            &pad_ground,
            &no_ceiling,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.z, PAD + 256, "MC2 carpet floors on the pad top");
        assert_eq!(eye(st.z), 2720.0, "MC2 eye = pad + clearance + 128");
        // It HOLDS there — the row-0xe buoyancy must not sink through
        // the pad on the ticks that follow (a docked carpet is parked,
        // not settling).
        for _ in 0..32 {
            mc2_move(
                &mut st,
                &mut ext,
                &idle,
                None,
                None,
                None,
                &pad_ground,
                &no_ceiling,
                &open_gate2,
                &never_stuck,
            );
        }
        assert_eq!(st.z, PAD + 256, "the dock holds against the sink");

        // MC1: half the clearance, same eye lift.
        let mut st1 = Mc1State {
            z: 0,
            ..Default::default()
        };
        mc1_move(&mut st1, &idle, None, None, &pad_ground, &open_gate);
        assert_eq!(st1.z, PAD + 128, "MC1 carpet floors at half the band");
        assert_eq!(eye(st1.z), 2592.0, "MC1 eye = pad + clearance + 128");

        // NON-VACUITY: the eye is genuinely ABOVE the carpet plane —
        // a dropped lift would put both games back on the carpet.
        assert_eq!(eye(st.z) - st.z as f32, 128.0);
        assert_eq!(eye(st1.z) - st1.z as f32, 128.0);
        // …and the two games really do dock at different heights.
        assert_ne!(st.z, st1.z);
    }

    #[test]
    fn mc2_cave_band_triples_climb_ceiling() {
        let aim_up = Mc1Input {
            stick_y: -127,
            ..Default::default()
        };
        let climb = |row: Mc2Row| -> i16 {
            let mut st = Mc1State {
                z: 256,
                act_speed: 80,
                tgt_speed: 80,
                ..Default::default()
            };
            let mut ext = Mc2Ext {
                row,
                ..Default::default()
            };
            for _ in 0..600 {
                step2(&mut st, &mut ext, &aim_up);
            }
            st.z
        };
        let open = climb(Mc2Row::OPEN);
        let cave = climb(Mc2Row::CAVE);
        // The authority zero sits at ground+band; the buoyancy sink
        // fights the last ramp sliver, so the carpet parks near but
        // below the band.
        assert!(
            open <= 1024 && open > 700,
            "open band parks under 1024: {open}"
        );
        assert!(
            cave <= 3072 && cave > 2300,
            "cave band parks under 3072: {cave}"
        );
    }

    #[test]
    fn mc2_mobilize_full_stops_and_settles() {
        let mut st = Mc1State {
            z: 800,
            act_speed: 80,
            tgt_speed: 80,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        ext.stun_hit();
        let idle = Mc1Input::default();
        let (x0, y0) = (st.x, st.y);
        let z0 = st.z;
        step2(&mut st, &mut ext, &idle);
        assert_eq!((st.x, st.y), (x0, y0), "full stop: no horizontal step");
        assert_eq!(st.z, z0 - 51, "the −51 settle while frozen");
        // The 10-tick counter releases the stun.
        for _ in 0..9 {
            step2(&mut st, &mut ext, &idle);
        }
        assert_eq!(ext.mobilize, 0, "released after 10 ticks");
        let x1 = st.x;
        step2(&mut st, &mut ext, &idle);
        assert!(!(st.x == x1 && st.y == y0), "moving again");
    }

    #[test]
    fn mc2_slow_quarters_speed_and_decays() {
        let mut st = Mc1State {
            z: 256,
            act_speed: 80,
            tgt_speed: 80,
            yaw: 512,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        ext.slow_hit();
        ext.slow_hit();
        ext.slow_hit();
        assert_eq!(ext.move_speed, 3);
        let idle = Mc1Input::default();
        let x0 = st.x;
        step2(&mut st, &mut ext, &idle);
        let dx = st.x.wrapping_sub(x0) as i16;
        assert_eq!(dx, 20, "moveSpeed 3 = quarter speed (80/4)");
        // 8 ticks per level: fully clear after 24.
        for _ in 0..24 {
            step2(&mut st, &mut ext, &idle);
        }
        assert_eq!(ext.move_speed, 0, "slow decays 1 level / 8 ticks");
    }

    #[test]
    fn mc2_cave_block_zeroes_target_and_cancels_accel() {
        let mut st = Mc1State {
            z: 256,
            act_speed: 80,
            tgt_speed: 80,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let blocked = |_: (u16, u16, i16), _: (u16, u16, i16)| Mc2GateOut {
            pass: None,
            wet: false,
            zero_speed: true,
        };
        let moved = mc2_move(
            &mut st,
            &mut ext,
            &Mc1Input::default(),
            None,
            None,
            None,
            &flat_ground,
            &no_ceiling,
            &blocked,
            &never_stuck,
        );
        assert_eq!(st.tgt_speed, 0, "cave block zeroes the TARGET speed");
        assert!(moved.accel_cancel, "and cancels the speed-up spell");
        // actSpeed still slews down over the following ticks (the
        // carpet decelerates, it doesn't freeze).
        assert_eq!(st.act_speed, 80);
        // ⭐ AND THE REFUSAL BUMPS THE WATER COUNTER. `moveTest_5D0A0`'s
        // refusal block opens with `incb 0x262(%eax)` (NETHERW.EXE
        // 0x81ccc) ahead of the position rollback — every refused move
        // in a cave, not only a wet one. The counter is ungraded (it
        // gates the water-flight sound loop), so this law can only take
        // a unit test; the positive evidence is remc2's own corpus,
        // whose two cave takes grade bit-perfect once the same
        // instruction is restored there.
        assert_eq!(ext.water_ctr, 1, "a refused cave move bumps waterCounter");
    }

    #[test]
    fn mc2_dry_pass_does_not_touch_the_water_counter() {
        let mut st = Mc1State {
            z: 256,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let open = |_: (u16, u16, i16), c: (u16, u16, i16)| Mc2GateOut {
            pass: Some((c, 0)),
            wet: false,
            zero_speed: false,
        };
        mc2_move(
            &mut st,
            &mut ext,
            &Mc1Input::default(),
            None,
            None,
            None,
            &flat_ground,
            &no_ceiling,
            &open,
            &never_stuck,
        );
        assert_eq!(ext.water_ctr, 0);
    }

    /// ⭐⭐ THE SPEED SPELL'S BRAKE-CANCEL IS `sub_5F380`'s BOUNDS TEST.
    /// `word_0xe_14` (`Mc2Moved::speed_touched`) rises only when the
    /// guarded ±16 step actually MOVED the target, and a boosted target
    /// sits outside ±80 — so exactly one key can move it, and it is the
    /// resisting one. That is the manual's "press the down cursor to
    /// cancel" falling out of arithmetic rather than a separate rule,
    /// and it is why the registered `thrust_cancel` deviation (which
    /// asserted the decompile "hard-overrides speed every tick with no
    /// brake input") was retired.
    #[test]
    fn the_mc2_speed_brake_is_the_resisting_key_only() {
        let step = |tgt: i16, up: bool, down: bool| {
            let mut st = Mc1State {
                z: 256,
                act_speed: tgt,
                tgt_speed: tgt,
                ..Default::default()
            };
            let mut ext = Mc2Ext::default();
            let inp = Mc1Input {
                speed_up: up,
                speed_down: down,
                ..Default::default()
            };
            let moved = mc2_move(
                &mut st,
                &mut ext,
                &inp,
                None,
                None,
                None,
                &flat_ground,
                &no_ceiling,
                &open_gate2,
                &never_stuck,
            );
            (moved.speed_touched, st.tgt_speed)
        };
        // FORWARD boost (tier 0 sustained): up is inert, down clamps
        // 160 → 80 and arms the flag — galore t=6278 exactly.
        assert_eq!(step(160, true, false), (false, 160), "up cannot brake");
        assert_eq!(step(160, false, true), (true, 80), "down brakes");
        // BACKWARD boost: mirrored, and the manual's "down" is now the
        // one that rides along.
        assert_eq!(step(-160, false, true), (false, -160), "down cannot brake");
        assert_eq!(step(-160, true, false), (true, -80), "up brakes");
        // Unboosted flight arms the flag on EITHER key — the window is
        // what makes it directional, not the flag.
        assert_eq!(step(16, true, false), (true, 32));
        assert_eq!(step(16, false, true), (true, 0));
    }

    #[test]
    fn mc2_nudge_shoves_128_forward_when_wedged() {
        let mut st = Mc1State {
            z: 256,
            yaw: 512, // east = +x
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let blocked = |_: (u16, u16, i16), _: (u16, u16, i16)| Mc2GateOut {
            pass: None,
            wet: false,
            zero_speed: false,
        };
        let wedged = |_: (u16, u16, i16), _: bool| true;
        let x0 = st.x;
        mc2_move(
            &mut st,
            &mut ext,
            &Mc1Input::default(),
            None,
            None,
            None,
            &flat_ground,
            &no_ceiling,
            &blocked,
            &wedged,
        );
        assert!(ext.nudge_latch, "the nudge latches");
        let dx = st.x.wrapping_sub(x0) as i16;
        assert!((dx - 128).abs() <= 1, "128-unit forward shove: {dx}");
    }

    #[test]
    fn mc2_ceiling_clamp_only_above_clearance() {
        // The clamp target is ceiling−384 (the closure supplies it
        // pre-subtracted); a carpet under the clearance band is never
        // yanked — the floor wins (EF:59757 branch order).
        let mut st = Mc1State {
            z: 800,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let low_roof = |_: u16, _: u16| -> Option<i16> { Some(500) };
        let idle = Mc1Input::default();
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            None,
            &flat_ground,
            &low_roof,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.z, 500, "clamped to the roof");
        // A roof target below the clearance floor: RAW retail — the
        // roof wins (EF:59757-63 clamps unconditionally once z sits
        // above floor+clearance). Such a pinch is unreachable in play
        // (the commit gate refuses air bands under clearance+fov+384);
        // if numerics ever brush it, a brief under-floor frame renders
        // as solid rock (the terrain shader's backface arm).
        let pinch = |_: u16, _: u16| -> Option<i16> { Some(100) };
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            None,
            &flat_ground,
            &pinch,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.z, 100, "the roof clamps raw, floor notwithstanding");
    }

    /// MC1's duel grip in ITS OWN SEAT — `sub_455D0`'s lock tail
    /// (`reference/remc1/sub_main.cpp:55226-48`). Three laws the
    /// pre-D18 world block broke: the rate is SIGNED, the cap is
    /// `3 * +128 / 2 = 120` off the re-stamped global
    /// ([`MC1_WIZ_SPEED_CAP`]), and the step is INSIDE the move — a
    /// refusing `sub_45410` discards it with everything else, while
    /// the heading servo lands on the ENTITY and survives.
    #[test]
    fn the_mc1_duel_grip_rides_the_move_and_the_servo_rides_the_entity() {
        let idle = Mc1Input::default();
        // Victim 10 tiles due north (`polar_step`'s zero is −y), so
        // the 3-D separation is 2,560 and the bearing is 0.
        let vpos = (0x8000u16, 0x8000u16.wrapping_sub(2560), 512i16);
        let grip = |hold: u32, yaw_drag: bool| Mc1DuelGrip {
            vpos,
            hold,
            max_speed: MC1_WIZ_SPEED_CAP,
            yaw_drag,
        };
        let fresh = |yaw: u16| Mc1State {
            x: 0x8000,
            y: 0x8000,
            z: 512,
            yaw,
            ..Default::default()
        };

        // (a) OUTSIDE the hold: (2560 − 1024) / (1024 / 120) = 192,
        // clamped to the cap 120 — a full-cap pull, pure −y.
        let mut st = fresh(0);
        let (x0, y0, z0) = (st.x, st.y, st.z);
        mc1_move_duel(
            &mut st,
            &idle,
            None,
            None,
            Some(grip(1024, true)),
            &flat_ground,
            &open_gate,
        );
        assert_eq!((st.x, st.z), (x0, z0), "pitch 0 ⇒ the pull is planar");
        assert_eq!(st.y, y0.wrapping_sub(120), "the ±120 cap, not ±80");

        // (b) INSIDE the hold the rate goes NEGATIVE and shoves the
        // caster back out: (2560 − 3072) / 8 = −64.
        let mut st = fresh(0);
        let y0 = st.y;
        mc1_move_duel(
            &mut st,
            &idle,
            None,
            None,
            Some(grip(3072, true)),
            &flat_ground,
            &open_gate,
        );
        assert_eq!(st.y, y0.wrapping_add(64), "inside the hold retail PUSHES");

        // (c) the commit gate refuses: :55249 discards the whole
        // scratch, duel step included — but `+30` was written on the
        // entity at :55246 and stands. 1000 → 870 is the 0x82 cap.
        let mut st = fresh(1000);
        let (x0, y0, z0) = (st.x, st.y, st.z);
        mc1_move_duel(
            &mut st,
            &idle,
            None,
            None,
            Some(grip(1024, true)),
            &flat_ground,
            &|_, p| (false, p),
        );
        assert_eq!(
            (st.x, st.y, st.z),
            (x0, y0, z0),
            "a refused move keeps nothing, the grip's step included"
        );
        assert_eq!(st.yaw, 870, "the servo is an ENTITY write and survives");

        // (d) D12's kill switch still parks the heading half alone.
        let mut st = fresh(1000);
        let y0 = st.y;
        mc1_move_duel(
            &mut st,
            &idle,
            None,
            None,
            Some(grip(1024, false)),
            &flat_ground,
            &open_gate,
        );
        assert_eq!(st.yaw, 1000, "yaw_drag off ⇒ the servo is parked");
        assert_ne!(st.y, y0, "…and the translation still runs");
    }

    /// `sub_5DE30`'s TRANSPORT (EF:59924-29), block 7 — the half the
    /// world's knock channel could never carry.
    ///
    /// ⚠ THIS LAW IS INVISIBLE TO A CONFORMANCE PAIR. The lock lives
    /// in three Type_160 words (`word_0x146_326`, `dword_0x142_322`,
    /// `word_0x14A_330`) that the MC2 import does not restore, so an
    /// imported world never holds a duel and both pose samplers pass
    /// `None` here. It is graded on the FREE RUN
    /// (mc2l6-rival-spells-galore t=1377) and pinned here.
    #[test]
    fn the_duel_leash_servos_the_yaw_and_steps_the_candidate() {
        // Idle carpet: no thrust, no strafe, so every unit of motion
        // below belongs to the leash.
        let mut st = Mc1State {
            x: 0x8000,
            y: 0x8000,
            z: 256,
            yaw: 1000,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let idle = Mc1Input::default();
        let (x0, y0) = (st.x, st.y);
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            // Bearing 0 = due north (`polar_step`'s zero is −y), pull
            // 120 = the tier cap `3 * minSpeed / 2`.
            Some((0, 120)),
            &flat_ground,
            &no_ceiling,
            &open_gate2,
            &never_stuck,
        );
        // The servo is `sub_58350(yaw, bearing, 5, 0x82)` — capped at
        // 130 per tick, so a 1000-unit error walks, it does not snap.
        assert_eq!(st.yaw, 870, "yaw servoed 130 toward the bearing");
        // …and the step rides the RAW bearing, not the servoed yaw:
        // due north is pure −y with x untouched.
        assert_eq!(st.x, x0, "no sideways drift on a due-north leash");
        assert_eq!(st.y, y0.wrapping_sub(120), "the full pull, one tick");

        // The SIGNED half: inside the held distance the leash is
        // negative and shoves the caster back out.
        let mut st = Mc1State {
            x: 0x8000,
            y: 0x8000,
            z: 256,
            yaw: 0,
            ..Default::default()
        };
        let mut ext = Mc2Ext::default();
        let y0 = st.y;
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            Some((0, -120)),
            &flat_ground,
            &no_ceiling,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.yaw, 0, "already on the bearing: the servo is a no-op");
        assert_eq!(st.y, y0.wrapping_add(120), "shoved AWAY from the opponent");
    }

    /// ⭐⭐ THE QUAKE'S CLOSE BAND SEIZES `pitch_acc`, BETWEEN THE INPUT
    /// PASS AND THE ADD (`engine::features::no_mc2_flood_human_spin`).
    /// mc2l18 t=27,263→27,264: accumulator 284, stick −4 (`pitchDelta`
    /// −73 off the UN-seized 284), and retail records 439 = 512 − 73.
    #[test]
    fn mc2_flood_spin_seizes_pitch_before_the_add() {
        let run = |acc: i16, stick: i16, spin: bool, stop: bool| {
            let mut st = Mc1State {
                z: 256,
                pitch_f: acc,
                ..Default::default()
            };
            let mut ext = Mc2Ext {
                flood_spin: spin,
                ..Default::default()
            };
            let inp = Mc1Input {
                stick_y: stick,
                mc2_stop: stop,
                ..Default::default()
            };
            mc2_move(
                &mut st,
                &mut ext,
                &inp,
                None,
                None,
                None,
                &flat_ground,
                &no_ceiling,
                &open_gate2,
                &never_stuck,
            );
            assert!(!ext.flood_spin, "the seizure is a one-shot");
            (st.pitch_f, st.aim_pitch)
        };
        // POSITIVE CONTROL: the plain filter step the port took.
        assert_eq!(run(284, -4, false, false), (211, 211));
        // THE LAW: 512 + the delta taken off 284.
        assert_eq!(run(284, -4, true, false), (439, 439));
        // t=27,261: −7 with a zero delta lands on 512 exactly.
        assert_eq!(run(-7, -2, false, false), (-7, 2041));
        assert_eq!(run(-7, -2, true, false), (512, 512));
        // A vetoed tick keeps both stores (the add never runs).
        assert_eq!(run(284, -4, true, true), (512, 512));
    }
}
