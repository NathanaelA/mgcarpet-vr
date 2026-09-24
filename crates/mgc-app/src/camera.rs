//! The replay THIRD-PERSON BOOM (`--thirdperson`).
//!
//! `--replay` renders from the flyer's own eye, the way the game is
//! played, and paints the recorded pose as a translucent instrument
//! billboard sitting in the same place (the ghost, `replay.rs`). The
//! third-person arm moves the eye off the carpet so that ghost can be
//! WATCHED — the player's framing call (2026-09-20): *"watching what
//! the player is watching from behind, placing the player sprite
//! around the bottom 2/3 mark of the screen, slightly from above."*
//!
//! ⭐ **THE BOOM SITS ON A CONE AROUND THE REVERSED VIEW AXIS.** The
//! camera looks along the flyer's own view direction — parallel, never
//! toed in — so the horizon stays exactly where first person puts it
//! and the picture reads as the same flight seen from behind. The
//! subject then falls BELOW the view axis by the boom's rise over its
//! run, and that ratio is what the framing law fixes: put the camera
//! at angle `atan(tan(fov_y/2) · SUBJECT_DROP)` off the reversed axis
//! and the subject lands on the two-thirds mark at EVERY boom length.
//! Which matters because the length is not constant — the boom
//! shortens against terrain — and a framing that drifted as it
//! shortened would pump the subject up and down the screen every time
//! the carpet passed a hill.
//!
//! Nothing here touches the sim. The resolver is a pure function of a
//! pose and two terrain samplers (both `&self` reads on the world), so
//! it cannot perturb a graded replay — see `docs/CONFORMANCE.md` on
//! what the app is allowed to do while a take is live.

use mgc_sim::MAP_TILES;

/// How far below the view axis the subject sits, as a fraction of the
/// half-height: `1/3` down from the centre = the two-thirds mark of
/// the screen.
const SUBJECT_DROP: f32 = 1.0 / 3.0;

/// The boom's length in tiles with nothing in the way. Four tiles
/// (player-set 2026-09-23, up from three: "somewhat further back")
/// puts a carpet at roughly an eighth of the screen height — the
/// heading still reads, and the chase view's elastic trail adds its
/// own length at speed on top.
pub const BOOM_REACH: f32 = 4.0;

/// The shortest the boom may be pulled. Below this the sprite fills
/// the frame and the view is worse than first person; a camera this
/// close to a cliff takes the terrain shader's wall-peek black
/// instead (`terrain.wgsl`), which is the honest failure.
const BOOM_MIN: f32 = 0.75;

/// Clearance kept between the camera and the floor / cave ceiling, in
/// tiles — `EYE_LIFT`'s half tile, the same margin the eye rides at.
const BOOM_CLEARANCE: f32 = 0.5;

/// Samples taken along the boom when looking for the blocked stretch.
/// At `BOOM_REACH` = 3 tiles that is a sample every quarter tile.
const BOOM_STEPS: usize = 12;

/// Where the third-person eye goes.
///
/// `subject` is the pose being watched (the flyer's eye, already
/// interpolated), `yaw`/`pitch` its view direction, `fov_y` the
/// EFFECTIVE vertical field of view this frame (`flight_fov_y`, which
/// widens on a tall window — the framing law is in screen fractions,
/// so it has to be the fov actually rendered). `ground` and `ceiling`
/// sample the live planes in tile units; `ceiling` returns `None`
/// off-cave.
///
/// Returns the eye in tile units, x/z wrapped onto the torus.
pub fn boom_eye(
    subject: [f32; 3],
    yaw: f32,
    pitch: f32,
    fov_y: f32,
    ground: &dyn Fn(f32, f32) -> f32,
    ceiling: &dyn Fn(f32, f32) -> Option<f32>,
) -> [f32; 3] {
    let dir = boom_dir(yaw, pitch, fov_y);
    let at = |t: f32| {
        [
            subject[0] + dir[0] * t,
            subject[1] + dir[1] * t,
            subject[2] + dir[2] * t,
        ]
    };
    // Walk out along the boom and keep the last clear sample. Walking
    // OUT (rather than resolving the far end and marching back) is
    // what makes a hill between the carpet and the far end pull the
    // camera in: the first blocked sample ends the boom, whatever is
    // clear beyond it.
    let mut len = BOOM_MIN;
    for i in 0..=BOOM_STEPS {
        let t = BOOM_MIN + (BOOM_REACH - BOOM_MIN) * (i as f32 / BOOM_STEPS as f32);
        if blocked(at(t), ground, ceiling) {
            break;
        }
        len = t;
    }

    // The final clamp. The march can only refuse to EXTEND the boom;
    // at `BOOM_MIN` it has no shorter answer to give, and the subject
    // itself may be hugging the ground (a carpet landed on a slope),
    // so pin the eye into the gap here too. Floor last = the floor
    // wins a low-headroom pinch, the sim's own branch order
    // (mgc-sim/src/lib.rs, the MC1 mover's ceiling tail).
    clamp_vertical(at(len), ground, ceiling)
}

/// The unit vector from the subject to the eye: back along the view
/// axis, lifted onto the framing cone. No roll — a billboard carpet
/// cannot bank, so rolling the world around it would only swing the
/// subject across the frame for a tumble the sprite never shows.
fn boom_dir(yaw: f32, pitch: f32, fov_y: f32) -> [f32; 3] {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    // `camera_flat_basis`'s convention, unrolled (mgc-render).
    let fwd = [sy * cp, sp, -cy * cp];
    let up = [-sy * sp, cp, cy * sp];
    let (st, ct) = cone_theta(fov_y).sin_cos();
    std::array::from_fn(|i| -fwd[i] * ct + up[i] * st)
}

/// The cone half-angle that puts the subject on the two-thirds mark:
/// the subject sits this far BELOW the camera axis.
fn cone_theta(fov_y: f32) -> f32 {
    ((fov_y * 0.5).tan() * SUBJECT_DROP).atan()
}

// ---------------------------------------------------------------------
// The CHASE camera — the third-person view's feel layer (player-set
// 2026-09-23, the Gothic chase model): the camera does not ride the
// carpet rigidly but TRAILS it. Its heading follows the carpet's with
// a lag, so a turn shows the carpet rotated against the frame for a
// beat before the camera catches up; its position is elastic on the
// boom, so accelerations read as the carpet pulling away and settling
// back; it sits further back and looks down on the carpet from a
// little above. The framing law is still exact: whatever the lag, the
// camera LOOKS AT the subject with the cone offset, so the carpet
// stays on the two-thirds mark at every frame (the test measures it
// mid-lag). `--firstperson` is the correctness view; this one is the
// demo reel, and every number below is a feel knob.
// ---------------------------------------------------------------------

/// Heading lag: the camera yaw closes on the carpet's with this time
/// constant (s). Longer = more of the carpet's flank on show in turns.
const YAW_TAU: f32 = 0.30;
/// Pitch lag (s) — the camera's look-down follows the aim slowly.
const PITCH_TAU: f32 = 0.35;
/// Position elasticity (s): the eye trails its boom target. At cruise
/// (~7.5 tiles/s) the steady-state trail is speed × this.
const POS_TAU: f32 = 0.12;
/// Bank smoothing (s): the subject's roll follows the per-tick bank
/// law through this, so the quantised recorded yaw steps do not flick
/// the sprite.
const BANK_TAU: f32 = 0.12;
/// How much of the carpet's own aim pitch the camera takes on (0 =
/// always level, 1 = the first-person pitch).
const PITCH_FOLLOW: f32 = 0.5;
/// The look-down: the camera axis is tilted this far below the
/// carpet's (radians) — "slightly from above". The boom cone rides the
/// camera axis, so this also lifts the eye.
const ELEVATION: f32 = 12.0 * std::f32::consts::PI / 180.0;
/// A boom target further than this from the smoothed eye (tiles) is a
/// respawn/teleport, not motion: the camera re-seats instead of
/// gliding across the map.
const SNAP_DIST: f32 = 6.0;

/// The chase view's persistent state — one per replay session, stepped
/// once per rendered frame.
#[derive(Debug, Default, Clone)]
pub struct ChaseCam {
    /// The lagged camera heading/pitch (the boom's own axis).
    yaw: f32,
    pitch: f32,
    /// The smoothed eye; `None` until the first frame seats it.
    eye: Option<[f32; 3]>,
    /// The smoothed subject bank.
    bank: f32,
}

/// One frame's resolved chase view (roll is always 0: the world stays
/// level, the SUBJECT banks).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChaseView {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl ChaseCam {
    /// Step the chase view toward the carpet at `subject` (tile
    /// units) heading `yaw` / aiming `pitch`, over `dt` seconds.
    pub fn update(
        &mut self,
        subject: [f32; 3],
        yaw: f32,
        pitch: f32,
        fov_y: f32,
        dt: f32,
        ground: &dyn Fn(f32, f32) -> f32,
        ceiling: &dyn Fn(f32, f32) -> Option<f32>,
    ) -> ChaseView {
        let gain = |tau: f32| 1.0 - (-dt.max(0.0) / tau).exp();
        let target_pitch = pitch * PITCH_FOLLOW - ELEVATION;
        if self.eye.is_none() {
            self.yaw = yaw;
            self.pitch = target_pitch;
        }
        // Heading: shortest arc, so the seam at ±π never spins the
        // camera the long way round.
        self.yaw = wrap_angle(self.yaw + wrap_angle(yaw - self.yaw) * gain(YAW_TAU));
        self.pitch += (target_pitch - self.pitch) * gain(PITCH_TAU);

        // The boom target for the lagged axis, terrain-resolved.
        let target = boom_eye(subject, self.yaw, self.pitch, fov_y, ground, ceiling);
        let eye = match self.eye {
            None => target,
            Some(e) => {
                let d = [
                    wrap_delta(target[0] - e[0]),
                    target[1] - e[1],
                    wrap_delta(target[2] - e[2]),
                ];
                if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > SNAP_DIST * SNAP_DIST {
                    target
                } else {
                    let g = gain(POS_TAU);
                    [e[0] + d[0] * g, e[1] + d[1] * g, e[2] + d[2] * g]
                }
            }
        };
        // The glide can cut a corner into a slope the boom itself
        // stepped around: the floor/ceiling get the last word again.
        let eye = clamp_vertical(eye, ground, ceiling);
        self.eye = Some(eye);

        // Aim: look AT the subject, then lift the axis by the cone
        // angle so the subject lands on the two-thirds mark — the
        // framing law holds however far the eye is trailing.
        let v = [
            wrap_delta(subject[0] - eye[0]),
            subject[1] - eye[1],
            wrap_delta(subject[2] - eye[2]),
        ];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
        ChaseView {
            eye,
            yaw: v[0].atan2(-v[2]),
            pitch: (v[1] / len).clamp(-1.0, 1.0).asin() + cone_theta(fov_y),
        }
    }

    /// Step the subject's bank toward `target` (radians) and return
    /// the smoothed value.
    pub fn bank(&mut self, target: f32, dt: f32) -> f32 {
        self.bank += (target - self.bank) * (1.0 - (-dt.max(0.0) / BANK_TAU).exp());
        self.bank
    }

    /// The smoothed bank as of the last step (for the draw path that
    /// runs between camera steps).
    pub fn bank_now(&self) -> f32 {
        self.bank
    }
}

/// The enhanced mover's bank, re-derived from two consecutive tick
/// poses `(x, z, yaw)`: the yaw step gives the turn rate, the plan
/// step projected on the heading gives the signed forward speed, and
/// the sim's own law (`mgc_sim::enhanced_bank`) turns them into a
/// roll. A recorded take carries no velocity, so this is the only
/// honest source.
pub fn motion_bank(prev: (f32, f32, f32), cur: (f32, f32, f32), tick_dt: f32) -> f32 {
    let (px, pz, pyaw) = prev;
    let (cx, cz, cyaw) = cur;
    let turn_rate = wrap_angle(cyaw - pyaw) / tick_dt;
    let (sy, cy) = cyaw.sin_cos();
    // Heading in the plan: (sin yaw, -cos yaw), the renderer's frame.
    let fwd_speed = (wrap_delta(cx - px) * sy - wrap_delta(cz - pz) * cy) / tick_dt;
    mgc_sim::enhanced_bank(turn_rate, fwd_speed)
}

/// The filmed MC2 exit's float-out: the view `k` (0..1, clamped) of
/// the way from the first-person `eye` to the chase `boom`, eased at
/// both ends. Positions take the short way round the torus, angles
/// the short arc; the eye's bank unwinds to the boom's level world.
/// No terrain resolve: both ends are clear and the path is a boom's
/// length.
pub fn blend_view(
    eye: &mgc_render::CameraView,
    boom: &mgc_render::CameraView,
    k: f32,
) -> mgc_render::CameraView {
    let k = k.clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    mgc_render::CameraView {
        x: wrap(eye.x + wrap_delta(boom.x - eye.x) * k),
        y: eye.y + (boom.y - eye.y) * k,
        z: wrap(eye.z + wrap_delta(boom.z - eye.z) * k),
        yaw: eye.yaw + wrap_angle(boom.yaw - eye.yaw) * k,
        pitch: eye.pitch + (boom.pitch - eye.pitch) * k,
        roll: eye.roll + wrap_angle(boom.roll - eye.roll) * k,
        fov_y: eye.fov_y + (boom.fov_y - eye.fov_y) * k,
    }
}

/// The floor/ceiling clearance clamp the boom resolve ends on.
fn clamp_vertical(
    mut p: [f32; 3],
    ground: &dyn Fn(f32, f32) -> f32,
    ceiling: &dyn Fn(f32, f32) -> Option<f32>,
) -> [f32; 3] {
    let (gx, gz) = (wrap(p[0]), wrap(p[2]));
    if let Some(c) = ceiling(gx, gz) {
        p[1] = p[1].min(c - BOOM_CLEARANCE);
    }
    p[1] = p[1].max(ground(gx, gz) + BOOM_CLEARANCE);
    [gx, p[1], gz]
}

/// The shortest signed way round the torus for one axis (tiles).
fn wrap_delta(d: f32) -> f32 {
    let full = MAP_TILES as f32;
    (d + full / 2.0).rem_euclid(full) - full / 2.0
}

/// The shortest signed arc (radians).
fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

fn blocked(
    p: [f32; 3],
    ground: &dyn Fn(f32, f32) -> f32,
    ceiling: &dyn Fn(f32, f32) -> Option<f32>,
) -> bool {
    let (x, z) = (wrap(p[0]), wrap(p[2]));
    if p[1] < ground(x, z) + BOOM_CLEARANCE {
        return true;
    }
    matches!(ceiling(x, z), Some(c) if p[1] > c - BOOM_CLEARANCE)
}

fn wrap(v: f32) -> f32 {
    v.rem_euclid(MAP_TILES as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOV: f32 = 60.0 / 180.0 * std::f32::consts::PI;

    /// Sea level everywhere, no cave.
    fn flat(h: f32) -> impl Fn(f32, f32) -> f32 {
        move |_, _| h
    }
    fn open(_: f32, _: f32) -> Option<f32> {
        None
    }

    /// THE FRAMING LAW, measured through the renderer's own
    /// projection: the subject lands on the two-thirds mark.
    #[test]
    fn the_boom_frames_the_subject_at_the_two_thirds_mark() {
        let subject = [128.0, 20.0, 128.0];
        for &(yaw, pitch) in &[
            (0.0, 0.0),
            (1.2, 0.0),
            (-2.5, 0.25),
            (0.4, -0.3),
            (3.0, 0.1),
        ] {
            let eye = boom_eye(subject, yaw, pitch, FOV, &flat(0.0), &open);
            let cam = mgc_render::CameraView {
                x: eye[0],
                y: eye[1],
                z: eye[2],
                yaw,
                pitch,
                roll: 0.0,
                fov_y: FOV,
            };
            // 4:3 — the reference aspect, where `flight_fov_y` is the
            // identity, so the rendered fov is the one framed for.
            let (w, h) = (1280.0, 960.0);
            let (sx, sy) = mgc_render::world_to_screen(&cam, w, h, subject[0], subject[1], subject[2])
                .expect("the subject is in front of the camera");
            assert!(
                (sx - w / 2.0).abs() < 0.5,
                "yaw {yaw}: subject off-centre horizontally at {sx}"
            );
            assert!(
                (sy - h * 2.0 / 3.0).abs() < 0.5,
                "yaw {yaw} pitch {pitch}: subject at {sy}, want {}",
                h * 2.0 / 3.0
            );
        }
    }

    /// The boom reaches its full length over open ground.
    #[test]
    fn open_ground_pays_out_the_whole_boom() {
        let subject = [128.0, 20.0, 128.0];
        let eye = boom_eye(subject, 0.0, 0.0, FOV, &flat(0.0), &open);
        let d = ((eye[0] - subject[0]).powi(2)
            + (eye[1] - subject[1]).powi(2)
            + (eye[2] - subject[2]).powi(2))
        .sqrt();
        assert!((d - BOOM_REACH).abs() < 1e-3, "boom {d}, want {BOOM_REACH}");
        // Behind (yaw 0 looks toward -z, so the eye is at +z) and above.
        assert!(eye[2] > subject[2], "the eye sits behind the subject");
        assert!(eye[1] > subject[1], "the eye sits above the subject");
    }

    /// A hill BEHIND the carpet pulls the boom in — and the eye still
    /// clears it.
    #[test]
    fn terrain_behind_the_subject_shortens_the_boom() {
        let subject = [128.0, 20.0, 128.0];
        // Ground rises to a wall 1.5 tiles behind (yaw 0 ⇒ +z).
        let hill = |_x: f32, z: f32| if z > 129.5 { 25.0 } else { 0.0 };
        let eye = boom_eye(subject, 0.0, 0.0, FOV, &hill, &open);
        let run = eye[2] - subject[2];
        assert!(
            run < 1.5,
            "the boom should stop short of the wall, ran {run} tiles"
        );
        assert!(eye[1] >= hill(eye[0], eye[2]) + BOOM_CLEARANCE);
    }

    /// Flying low over flat ground: the eye never dips into it. The
    /// boom lifts anyway (it rises), so this pins the CLAMP by putting
    /// the subject under the floor — a carpet crushed into a slope.
    #[test]
    fn the_eye_never_sinks_into_the_floor() {
        let subject = [128.0, 1.0, 128.0];
        let eye = boom_eye(subject, 0.0, 0.4, FOV, &flat(4.0), &open);
        assert!(
            eye[1] >= 4.0 + BOOM_CLEARANCE - 1e-4,
            "eye at {} under a floor of 4.0",
            eye[1]
        );
    }

    /// In a cave the boom stays under the ceiling.
    #[test]
    fn a_cave_ceiling_caps_the_boom() {
        let subject = [128.0, 3.0, 128.0];
        let roof = |_: f32, _: f32| Some(3.6_f32);
        let eye = boom_eye(subject, 0.0, 0.0, FOV, &flat(0.0), &roof);
        assert!(
            eye[1] <= 3.6 - BOOM_CLEARANCE + 1e-4,
            "eye at {} through a 3.6 ceiling",
            eye[1]
        );
        assert!(eye[1] >= BOOM_CLEARANCE - 1e-4, "eye at {} under the floor", eye[1]);
    }

    /// A cave too tight for the clearance: the floor wins, the sim's
    /// own branch order.
    #[test]
    fn a_pinched_cave_gives_the_floor_the_last_word() {
        let subject = [128.0, 2.2, 128.0];
        let eye = boom_eye(subject, 0.0, 0.0, FOV, &flat(2.0), &|_, _| Some(2.4));
        assert!((eye[1] - 2.5).abs() < 1e-4, "eye at {}, want floor+0.5", eye[1]);
    }

    /// The torus seam: a boom that runs off the east edge comes back
    /// on the west one, and the samplers see the wrapped coordinate.
    #[test]
    fn the_boom_wraps_at_the_seam() {
        let subject = [255.0, 20.0, 128.0];
        // Looking west (yaw -π/2 ⇒ fwd.x = -1) puts the eye at +x.
        let eye = boom_eye(subject, -std::f32::consts::FRAC_PI_2, 0.0, FOV, &flat(0.0), &open);
        assert!(
            (0.0..MAP_TILES as f32).contains(&eye[0]),
            "eye x {} off the torus",
            eye[0]
        );
        assert!(eye[0] < BOOM_REACH, "eye x {} did not wrap past the seam", eye[0]);
    }
    fn screen_of(view: &ChaseView, subject: [f32; 3]) -> (f32, f32) {
        let cam = mgc_render::CameraView {
            x: view.eye[0],
            y: view.eye[1],
            z: view.eye[2],
            yaw: view.yaw,
            pitch: view.pitch,
            roll: 0.0,
            fov_y: FOV,
        };
        mgc_render::world_to_screen(&cam, 1280.0, 960.0, subject[0], subject[1], subject[2])
            .expect("the subject is in front of the chase camera")
    }

    /// The framing law survives the lag: a 90° step turn leaves the
    /// camera trailing for many frames, and on EVERY one of them the
    /// subject still sits centred on the two-thirds mark.
    #[test]
    fn the_chase_view_keeps_the_framing_while_it_lags() {
        let subject = [128.0, 20.0, 128.0];
        let mut cam = ChaseCam::default();
        cam.update(subject, 0.0, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        let turned = std::f32::consts::FRAC_PI_2;
        for frame in 0..40 {
            let view = cam.update(subject, turned, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
            let (sx, sy) = screen_of(&view, subject);
            assert!((sx - 640.0).abs() < 0.5, "frame {frame}: off-centre at {sx}");
            assert!((sy - 640.0).abs() < 0.5, "frame {frame}: subject at {sy}, want 640");
        }
    }

    /// The heading LAGS: right after a step turn the camera axis is
    /// still mostly the old heading, then it closes on the new one.
    #[test]
    fn the_chase_heading_trails_the_carpet_and_converges() {
        let subject = [128.0, 20.0, 128.0];
        let mut cam = ChaseCam::default();
        cam.update(subject, 0.0, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        let turned = 1.0;
        cam.update(subject, turned, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        let early = cam.yaw;
        assert!(early > 0.0 && early < 0.2, "one frame in, the axis moved {early} of 1.0");
        for _ in 0..180 {
            cam.update(subject, turned, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        }
        assert!((cam.yaw - turned).abs() < 0.02, "three seconds later it sits at {}", cam.yaw);
    }

    /// The eye is further back and higher than the rigid boom put it:
    /// the look-down lifts the cone, and level flight is watched from
    /// above.
    #[test]
    fn the_chase_eye_sits_behind_and_above() {
        let subject = [128.0, 20.0, 128.0];
        let mut cam = ChaseCam::default();
        let view = cam.update(subject, 0.0, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        assert!(view.eye[2] > subject[2] + 3.5, "eye z {} — not far enough back", view.eye[2]);
        let rigid = boom_eye(subject, 0.0, 0.0, FOV, &flat(0.0), &open);
        assert!(view.eye[1] > rigid[1] + 0.3, "eye y {} vs rigid {}", view.eye[1], rigid[1]);
        assert!(view.pitch < 0.0, "the camera looks down ({})", view.pitch);
    }

    /// A teleport re-seats the camera instead of gliding it across
    /// the map.
    #[test]
    fn a_teleport_snaps_the_chase_eye() {
        let mut cam = ChaseCam::default();
        cam.update([128.0, 20.0, 128.0], 0.0, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        let far = [40.0, 20.0, 200.0];
        let view = cam.update(far, 0.0, 0.0, FOV, 1.0 / 60.0, &flat(0.0), &open);
        let d = ((view.eye[0] - far[0]).powi(2) + (view.eye[2] - far[2]).powi(2)).sqrt();
        assert!(d < BOOM_REACH + 0.5, "eye {d} tiles from the new subject");
    }

    /// The bank law off recorded motion: a right turn at forward
    /// speed banks right (positive), a left turn left, and a turn in
    /// place banks nothing.
    #[test]
    fn motion_bank_follows_the_enhanced_law() {
        let dt = 1.0 / 24.0;
        // Heading 0 = toward -z; forward motion is -z.
        let right = motion_bank((128.0, 128.0, 0.0), (128.0, 127.7, 0.08), dt);
        let left = motion_bank((128.0, 128.0, 0.0), (128.0, 127.7, -0.08), dt);
        let still = motion_bank((128.0, 128.0, 0.0), (128.0, 128.0, 0.08), dt);
        assert!(right > 0.05, "right turn banked {right}");
        assert!((left + right).abs() < 1e-6, "left {left} vs right {right}");
        assert_eq!(still, 0.0, "a turn in place banks nothing");
        // The seam: a step across x = 0 is one tile, not 255.
        let seam = motion_bank((0.2, 128.0, 0.0), (255.9, 128.0, 0.0), dt);
        assert_eq!(seam, 0.0);
    }

    /// The filmed exit's float-out starts ON the eye, ends ON the
    /// boom, and crosses the seam and the ±π yaw cut the short way.
    #[test]
    fn blend_view_runs_eye_to_boom_the_short_way() {
        let v = |x: f32, yaw: f32, roll: f32| mgc_render::CameraView {
            x,
            y: 10.0,
            z: 128.0,
            yaw,
            pitch: 0.0,
            roll,
            fov_y: FOV,
        };
        let eye = v(255.5, 3.0, 0.3);
        let boom = v(1.5, -3.0, 0.0);
        let start = blend_view(&eye, &boom, 0.0);
        assert_eq!((start.x, start.yaw, start.roll), (eye.x, eye.yaw, eye.roll));
        let end = blend_view(&eye, &boom, 1.0);
        assert!((end.x - boom.x).abs() < 1e-4);
        assert!((wrap_angle(end.yaw - boom.yaw)).abs() < 1e-4);
        assert!(end.roll.abs() < 1e-6);
        // Midway: across the seam (x near 0.5), not back through 128;
        // across the yaw cut (near π), not through 0.
        let mid = blend_view(&eye, &boom, 0.5);
        assert!((mid.x - 0.5).abs() < 1e-3, "x {}", mid.x);
        assert!(mid.yaw.abs() > 3.0, "yaw {}", mid.yaw);
    }

}
