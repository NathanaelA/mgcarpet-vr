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

/// The boom's length in tiles with nothing in the way. Three tiles
/// puts a carpet at roughly a sixth of the screen height — big enough
/// to read its heading, small enough to leave the flight visible.
pub const BOOM_REACH: f32 = 3.0;

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

    let mut eye = at(len);
    // The final clamp. The march can only refuse to EXTEND the boom;
    // at `BOOM_MIN` it has no shorter answer to give, and the subject
    // itself may be hugging the ground (a carpet landed on a slope),
    // so pin the eye into the gap here too. Floor last = the floor
    // wins a low-headroom pinch, the sim's own branch order
    // (mgc-sim/src/lib.rs, the MC1 mover's ceiling tail).
    let (gx, gz) = (wrap(eye[0]), wrap(eye[2]));
    if let Some(c) = ceiling(gx, gz) {
        eye[1] = eye[1].min(c - BOOM_CLEARANCE);
    }
    eye[1] = eye[1].max(ground(gx, gz) + BOOM_CLEARANCE);
    [gx, eye[1], gz]
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
    let theta = ((fov_y * 0.5).tan() * SUBJECT_DROP).atan();
    let (st, ct) = theta.sin_cos();
    std::array::from_fn(|i| -fwd[i] * ct + up[i] * st)
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
        assert!(eye[0] < 2.5, "eye x {} did not wrap past the seam", eye[0]);
    }
}
