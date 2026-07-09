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

use crate::features::Gen;

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
}
