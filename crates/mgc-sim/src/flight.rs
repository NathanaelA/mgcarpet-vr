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
//!   (3×80 held / 2×80 released, :65171-78), bypassing the chase; on
//!   expiry it resets the target to +80 max forward (:65191-97) — an
//!   authentic quirk. Any Up/Down press cancels it (:55144-51 in the
//!   handler via the v_14 speed-touched flag).
//! - One RNG draw exists in the move: every 64th tick a private-LCG
//!   roll (`9377·r + 9439`, :55294-99) fires the wind-gust FLUTTER
//!   (sound 46) on `r % 11 == 0` — sound-only, but the draw mutates
//!   state, so it is replicated for fidelity.
//!
//! The enhanced mover (hold-to-fly) stays float-based in `lib.rs` as a
//! deliberate deviation; both obey the player-directed rule that aim
//! pitch never steals meaningful mobility (the faithful model's cos
//! shrink maxes at ~29%, and thrust stays fully live while aiming).
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
//! Deliberately unported here (cited, banked): the `sub_5DE30`
//! possess/tornado leash (worklist item 7 — needs the grab spells) and
//! the trailing cave-ambient/water-loop sound block (EF:59776-59850,
//! presentation). MC2's tick makes NO flutter roll (MC1's :55294-99
//! LCG is replaced by that sound block; the draw is carpet-private
//! state, so omitting it moves no world golden).

use crate::mc1::features::Gen;

/// Faithful carpet state (the human entity + Type_160 fields we use).
#[derive(Debug, Clone, Copy, Default)]
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
}

/// What the move reports back to the sim boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mc1Moved {
    /// The wind-gust flutter roll fired (sound 46, player-anchored).
    pub flutter: bool,
}

/// The faithful human move: sub_46840's command integration followed
/// by sub_455D0's move, in the original's statement order. `ground`
/// returns terrain height in engine units at an 8.8 position; `gate`
/// is the sub_45410 wall gate minus its z-floor (the floor is applied
/// here, :55103-05) — `None` discards the whole move (x, y AND z
/// freeze; the sink and any slide are lost with it, verbatim).
/// `accel_over` = the Accelerate spell's signed factor (±3 held / ±2
/// released); `knock` = this tick's buffet displacement (direction,
/// magnitude), already decayed by the caller.
pub fn mc1_move(
    st: &mut Mc1State,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    ground: &dyn Fn(u16, u16) -> i16,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> Option<(u16, u16, i16)>,
) -> Mc1Moved {
    // ---- sub_46840 (:55760-:55821): command integration, pre-move ----
    // Up/Down step the target ±16/tick held, clamp ±80 (:55766-80).
    let mut dir: i16 = 0;
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
    // 4/tick toward 0 with a sign-flip snap (:55800-19).
    let sdir: i16 = match (inp.strafe_left, inp.strafe_right) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    if sdir != 0 {
        st.strafe = (st.strafe + 16 * sdir).clamp(-80, 80);
    } else if st.strafe != 0 {
        let s = st.strafe.signum();
        st.strafe -= 4 * s;
        if st.strafe.signum() != s {
            st.strafe = 0;
        }
    }

    // The Accelerate override writes BOTH the target and the actual
    // speed (:65171-78) — the chase below then steps by zero.
    if let Some(k) = accel_over {
        let v = (k * 80.0) as i16; // 3×80 held / 2×80 released, signed
        st.tgt_speed = v;
        st.act_speed = v;
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
    let g = ground(st.x, st.y) as i32; // :55151, at the pre-move position
    // v5 = z − ground − 1024 clamped ±256: −256 = full climb
    // authority, 0 at the soft ceiling, +256 = fully inverted.
    let v5 = (st.z as i32 - g - 1024).clamp(-256, 256);
    st.aim_pitch = (st.pitch_f as u16) & 0x7FF; // published +32 (:55158-60)
    let mut v6 = st.aim_pitch as i32;
    if v6 > 1024 {
        v6 -= 2048;
    }
    let s = st.act_speed;
    if s != 0 && v6 != 0 {
        let dive = (s > 0 && v6 > 0) || (s < 0 && v6 < 0);
        st.eff_pitch = if dive {
            // Descent passes the raw aim (:55176/:55186).
            st.aim_pitch
        } else {
            // Climb scaled by authority (−v5)/256, truncating
            // (:55181/:55191), re-masked to 11 bits (:55193-95).
            ((((v6 * -v5) / 256) as i16) as u16) & 0x7FF
        };
    } else if s == 0 && st.z as i32 > g + 1024 {
        // Speed-0 sink above the soft ceiling (:55171-72);
        // eff_pitch deliberately left stale.
        cand.2 = st.z - 8;
    }
    // The polar step (:55196): horizontal = s·cos(eff), z −= s·sin(eff).
    Gen::polar_step(&mut cand, st.yaw, st.eff_pitch, st.act_speed);

    // (d) strafe: second polar step at yaw+512, pitch 0 (:55197-203).
    if st.strafe != 0 {
        Gen::polar_step(&mut cand, st.yaw.wrapping_add(512) & 0x7FF, 0, st.strafe);
    }

    // (e) knock displacement (v_22/v_24, :55204-19; decay lives with
    // the caller's Type_160 emulation).
    if let Some((kdir, kmag)) = knock {
        Gen::polar_step(&mut cand, kdir, 0, kmag);
    }

    // (f) commit gate + unconditional z-floor ground+128 (row v_12)
    // at the FINAL candidate (:55250-52, :55103-05). A fully blocked
    // move commits nothing — not even the sink.
    if let Some(mut p) = gate((st.x, st.y, st.z), cand) {
        let floor = ground(p.0, p.1).saturating_add(128);
        if p.2 < floor {
            p.2 = floor;
        }
        st.x = p.0;
        st.y = p.1;
        st.z = p.2;
    }

    // (g) the every-64th-tick flutter roll on the entity's private
    // LCG (:55294-99) — sound-only, but the draw is state.
    st.tick_ctr = st.tick_ctr.wrapping_add(1);
    let mut flutter = false;
    if st.tick_ctr & 0x3F == 0 {
        st.rand = st.rand.wrapping_mul(9377).wrapping_add(9439);
        flutter = st.rand % 0xB == 0;
    }
    Mc1Moved { flutter }
}

/// The MC2 carpet's `str_D7BD6` tuning row — `AddPlayer_4A920`
/// explicitly overwrites the generic default with row 104 on cave
/// maps, row 66 otherwise (EF:33329-32; trace §0.1 — row 59 is the
/// pre-overwrite default and must NOT be used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// `moveSpeed_0x14C_332` (0..3): the spider-web SLOW — scales the
    /// pose delta, forward and strafe speed by (4−n)/4. Drives the
    /// red screen tint (presentation reads it).
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
}

/// The faithful MC2 human move — `sub_5F380`'s command integration
/// followed by `sub_5D530` in the original's statement order (trace
/// docs/traces/mc2-flight-model.md). `ground` is `getTerrainAlt`;
/// `ceiling` returns the cave clamp target `ceiling − 384` (None
/// off-cave — [`crate::mc1::world::World::player_cave_ceiling`]);
/// `gate` is `moveTest_5D0A0` (water slide + cave steer, world-side);
/// `stuck` is `sub_5DD50`'s wedged test at the CURRENT position
/// (water / sealed / latched-and-colliding). `accel_over` and `knock`
/// ride the same channels as [`mc1_move`] — the MC2 knock constants
/// (cap 128, decay −4, snap <4; EF:59695-711) equal the MC1 channel's,
/// and `moveBoost` IS that channel's retail home.
#[allow(clippy::too_many_arguments)]
pub fn mc2_move(
    st: &mut Mc1State,
    ext: &mut Mc2Ext,
    inp: &Mc1Input,
    accel_over: Option<f32>,
    knock: Option<(u16, i16)>,
    ground: &dyn Fn(u16, u16) -> i16,
    ceiling: &dyn Fn(u16, u16) -> Option<i16>,
    gate: &dyn Fn((u16, u16, i16), (u16, u16, i16)) -> Mc2GateOut,
    stuck: &dyn Fn((u16, u16, i16), bool) -> bool,
) -> Mc2Moved {
    let mut moved = Mc2Moved::default();

    // ---- sub_5F380 (EF:60748): command integration, pre-move ----
    // Identical numbers to MC1's sub_46840 (trace §4c: the D4B8x
    // constants match 16/±80/−4 exactly).
    let mut dir: i16 = 0;
    if inp.speed_up && st.tgt_speed < 80 {
        dir = 1;
    }
    if inp.speed_down && st.tgt_speed > -80 {
        dir = -1;
    }
    if dir != 0 {
        st.tgt_speed = (st.tgt_speed + 16 * dir).clamp(-80, 80);
    }
    let sdir: i16 = match (inp.strafe_left, inp.strafe_right) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    if sdir != 0 {
        st.strafe = (st.strafe + 16 * sdir).clamp(-80, 80);
    } else if st.strafe != 0 {
        let s = st.strafe.signum();
        st.strafe -= 4 * s;
        if st.strafe.signum() != s {
            st.strafe = 0;
        }
    }

    // The speed-up spell override (EF:56189 arms it; the channel's
    // shape is shared with MC1's Accelerate).
    if let Some(k) = accel_over {
        let v = (k * 80.0) as i16;
        st.tgt_speed = v;
        st.act_speed = v;
    }

    // ---- sub_5D530 (EF:59610), statement order ----
    // (0) pose: the filtered delta (EF:38060-66, ÷4 toward zero),
    // slow-scaled while the web slow is active (EF:59622-30), then
    // yaw as a RATE and the published absolute aim pitch.
    let dr = ((2 * inp.stick_x as i32 - st.roll_f as i32) / 4) as i16;
    let dp = ((2 * inp.stick_y as i32 - st.pitch_f as i32) / 4) as i16;
    if ext.move_speed > 0 {
        st.roll_f += ext.slow_scale(dr as i32) as i16;
        st.pitch_f += ext.slow_scale(dp as i32) as i16;
    } else {
        st.roll_f += dr;
        st.pitch_f += dp;
    }
    st.yaw = ((st.yaw as i32 + st.roll_f as i32 / 8) & 0x7FF) as u16; // EF:59635

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
    let g = ground(st.x, st.y) as i32;
    let band = ext.row.band as i32;
    let alt_diff = (((st.z as i32 - g - band) << 10) / band).clamp(-256, 256);
    st.aim_pitch = (st.pitch_f as u16) & 0x7FF; // published (EF:59651-52)
    let mut v6 = st.aim_pitch as i32;
    if v6 > 1024 {
        v6 -= 2048;
    }
    let s = st.act_speed;
    if s != 0 && v6 != 0 {
        let dive = (s > 0 && v6 > 0) || (s < 0 && v6 < 0);
        st.eff_pitch = if dive {
            st.aim_pitch
        } else {
            // Round-toward-zero fold (the −sign·255 >> 8 idiom).
            ((((v6 * -alt_diff) / 256) as i16) as u16) & 0x7FF
        };
    }
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
    }

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

    // (7) the sub_5DE30 possess/tornado leash — UNPORTED (banked,
    // trace §6/worklist 7: needs the grab-spell machinery).

    // (8) slow/mobilize decay (EF:59722-43).
    ext.tick_debuffs();

    // (9) the commit gate + vertical resolution (EF:59745-69).
    let out = gate((st.x, st.y, st.z), cand);
    if out.wet {
        ext.water_ctr += 1;
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
                // bounce, no damage — EF:59757-63). The floor+
                // clearance max is the PLAYTEST-CAVE round-2 guard:
                // where headroom pinches below 384+clearance (door
                // slopes) the raw retail clamp would pin the carpet
                // under the terrain.
                if let Some(c) = ceiling(p.0, p.1) {
                    z = z.min((c as i32).max(g + clr));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_ground(_: u16, _: u16) -> i16 {
        0
    }
    fn open_gate(_: (u16, u16, i16), p: (u16, u16, i16)) -> Option<(u16, u16, i16)> {
        Some(p)
    }

    fn step(st: &mut Mc1State, inp: &Mc1Input) -> Mc1Moved {
        mc1_move(st, inp, None, None, &flat_ground, &open_gate)
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
        // 0.71, never worse (the player's "flat plane" holds within
        // 29% — load-bearing for combat dodging).
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
            &flat_ground,
            &no_ceiling,
            &open_gate2,
            &never_stuck,
        )
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
            &flat_ground,
            &low_roof,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.z, 500, "clamped to the roof");
        // A roof target below the clearance floor: the floor wins.
        let pinch = |_: u16, _: u16| -> Option<i16> { Some(100) };
        mc2_move(
            &mut st,
            &mut ext,
            &idle,
            None,
            None,
            &flat_ground,
            &pinch,
            &open_gate2,
            &never_stuck,
        );
        assert_eq!(st.z, 256, "the clearance floor beats the roof");
    }
}
