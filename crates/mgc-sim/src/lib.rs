 //! The simulation core: pure, headless, deterministic.
//!
//! Ground rules (enforced by review, not yet by tooling):
//! - No I/O, no rendering, no wall-clock time, no threads.
//! - Advances only via [`Simulation::step`] at a fixed tick rate;
//!   rendering interpolates between ticks and never influences state.
//! - Given the same level package and the same input sequence, the
//!   resulting state is bit-identical on every platform. This is what
//!   makes replay, testing, and (eventually) multiplayer possible.
//!
//! World units follow the original engine: 1.0 = one terrain tile
//! (256 fixed-point units in the original), the map is 256x256 tiles
//! and wraps around in both axes, and altitude is `height_byte / 8`
//! (the engine computes `32 * height_byte` in its own units).

pub mod chassis;
pub mod engine;
pub mod flight;
pub mod ids;
pub mod mc1;
pub mod mc2;
pub mod verbs;

use engine::{features, world};
use mc1::spells;

/// Fixed simulation tick rate.
///
/// 24 Hz — faithful to retail MC2, whose engine advanced one "game turn"
/// per rendered frame under remc2's 24 FPS limiter. MC1 has no single
/// "correct" tick rate (it ran uncapped, hardware-bound), so it borrows
/// the MC2 cadence as the best available estimate. Both games share this
/// constant.
pub const TICK_RATE_HZ: u32 = 24;

/// Seconds per tick (render-side interpolation uses the same constant,
/// so keep a single definition).
pub const TICK_DT: f32 = 1.0 / TICK_RATE_HZ as f32;

/// Map side length in tiles; coordinates wrap modulo this.
pub const MAP_TILES: usize = 256;

/// Altitude of one height-byte step in tile units (engine: 32/256).
pub const HEIGHT_SCALE: f32 = 1.0 / 8.0;

/// The thrust/steering model — the G-class flight-control tier
/// (ROADMAP "Flight-control tiers"). Selected once at the sim
/// boundary; replays must record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThrustModel {
    /// The faithful MC1 model (remc1 sub_455D0, ported in
    /// [`flight`]): rate-based stick steering, accelerate/decelerate
    /// impulses that persist until countered, thrust always in the
    /// level ground plane.
    #[default]
    Mc1,
    /// Hold-to-fly with automatic deceleration on release — a
    /// deliberate deviation, generalizing the original's own
    /// hold-to-move strafe to the forward axis. Keeps the authentic
    /// level-plane thrust rule (aim pitch never steals mobility).
    Enhanced,
}

/// The altitude model — the second G-class tier. `Faithful` =
/// terrain-follow only (the carpet floats up along rising ground and
/// settles by itself; no fly-up control exists). `ExtendedLift` adds
/// explicit float up/down keys — no original equivalent — capped at
/// the level's highest terrain tile and never bypassing wall blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AltitudeModel {
    #[default]
    Faithful,
    ExtendedLift,
}

/// Player intent for one tick, already normalized to [-1, 1] axes.
/// Angles are radians accumulated since the previous tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlightInput {
    /// Forward (+) / backward (-) along the view direction.
    pub thrust: f32,
    /// Right (+) / left (-) perpendicular to the view, horizontal.
    pub strafe: f32,
    /// Up (+) / down (-) in world space.
    pub lift: f32,
    pub yaw_delta: f32,
    pub pitch_delta: f32,
    /// The MC1 model's virtual stick, ±127 like the original's mouse
    /// offset from screen center (roll = x drives turn RATE, y is the
    /// aim pitch). The app's input mapper maintains it; the sim's
    /// low-pass filter lives in [`flight::Mc1State`] so replays stay
    /// deterministic. Ignored by the enhanced thrust model.
    pub stick_x: i16,
    pub stick_y: i16,
    /// Left-hand cast held (the original's dw_0 bit 0x10; LMB).
    pub fire_left: bool,
    /// Right-hand cast held (dw_0 bit 0x20; RMB).
    pub fire_right: bool,
    /// Equip a spell to the left/right hand this tick (from the book
    /// screen or a quick key) — the original's commands 0x15/0x16.
    pub equip_left: Option<spells::SpellId>,
    pub equip_right: Option<spells::SpellId>,
    /// MC2 spell selection (the CTRL-pane commit): (spell index
    /// 0..25, tier, hand 0 = left / 1 = right).
    pub mc2_select: Option<(u8, u8, u8)>,
    /// The Backspace full stop (retail MC2 action 0x27, EF:37954-66):
    /// zero the actual AND target speed, kill an active Speed/
    /// Accelerate channel, recenter the steering (the app resets the
    /// virtual stick — retail's SetCenterScreenForFlyAssistant).
    /// Enhancement-class in MC1/HW (deliberate: retail MC1's Backspace
    /// is text-entry only).
    pub full_stop: bool,
    /// The respawn key (Space; the original's command 15) — consumed
    /// only while dead.
    pub respawn: bool,
    /// The demolish key (Shift+L; the unique control word 48).
    pub demolish: bool,
}

/// The carpet: position in tile units, velocity in tiles/second.
#[derive(Debug, Clone, Copy)]
pub struct Flyer {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    /// Radians; 0 looks toward -Z, increasing turns right (clockwise
    /// viewed from above).
    pub yaw: f32,
    /// Radians; positive looks up. Clamped to just short of vertical.
    pub pitch: f32,
    /// Camera bank, radians; positive banks right (into a right
    /// turn). The faithful movers publish the filtered roll stick at
    /// FULL value — retail's render pose takes `u16_327` unhalved
    /// (:52432) while pitch is halved (:52433); MC2 identical
    /// (`rotation.roll = roll_0x155_341`, EF:40258). The enhanced
    /// mover leaves it 0 (deliberate: no tilt in mouse-look).
    pub roll: f32,
}

impl Default for Flyer {
    fn default() -> Self {
        Self {
            x: 128.0,
            y: 20.0,
            z: 160.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            yaw: 0.0,
            pitch: -0.2,
            roll: 0.0,
        }
    }
}

/// Flight tuning. Placeholder feel, to be eyeballed against remc2
/// side-by-side before habits form (see docs/ROADMAP.md).
const DRAG_PER_TICK: f32 = 0.90; // velocity retained per tick
/// The faithful carpet's base cruise, in tiles/sec — the reference
/// "how fast" for BOTH movers. `Mc1State` cruises at 80 engine
/// units/tick = 80/256 tiles/tick. The Enhanced deviation changes only
/// the CONTROL response (hold-to-fly vs. accelerate-buildup), never the
/// speed ceiling (deliberate), so its terminal is PINNED to this rather
/// than tuned independently — otherwise a fixed-size hazard (the kraken
/// buffet, 80/tick) is proportionally weak under the faster deviation
/// and you slip a tether you shouldn't.
const FAITHFUL_CRUISE_TPS: f32 = 80.0 / 256.0 * TICK_RATE_HZ as f32; // 7.5 @ 24 Hz
/// Enhanced float acceleration, DERIVED so the DRAG-governed terminal
/// (`ACCEL·dt·drag/(1−drag)`) equals [`FAITHFUL_CRUISE_TPS`]. DRAG alone
/// shapes the approach snappiness; the ceiling stays pinned regardless.
const ACCEL: f32 = FAITHFUL_CRUISE_TPS * (1.0 - DRAG_PER_TICK) / (TICK_DT * DRAG_PER_TICK);
const MAX_PITCH: f32 = 1.45; // radians
const MIN_CLEARANCE: f32 = 0.75; // tiles above ground
/// Extended-lift float rate, engine units/tick at full input (an
/// invented constant — the enhancement has no original equivalent).
const LIFT_STEP: f32 = 48.0;

/// The whole game state and its single mutation entry point.
#[derive(Default)]
pub struct Simulation {
    /// Monotonic tick counter since level start. One tick = one of the
    /// original's game turns (events, water phase, sprite frames).
    pub tick: u64,
    pub flyer: Flyer,
    /// The two G-class flight tiers; fixed per run (replay headers
    /// must record them once replays exist).
    pub thrust_model: ThrustModel,
    pub altitude_model: AltitudeModel,
    /// Faithful integer carpet state, authoritative under
    /// [`ThrustModel::Mc1`]; `flyer` is derived from it after each
    /// tick for the renderer/camera.
    pub carpet: flight::Mc1State,
    /// The MC2-only carpet channels (slow/stun webs, displacement
    /// mailbox, nudge latch, tuning row) — live under the
    /// [`verbs::FlightVerb::Mc2`] arm; the enhanced mover services
    /// the debuff channels too (the webs are gameplay).
    pub carpet_mc2: flight::Mc2Ext,
    /// The Accelerate override was live last tick (its expiry resets
    /// the speed target to +80 max forward, :65191-97).
    accel_was_active: bool,
    /// 256x256 height bytes, row-major `y * 256 + x`; empty means flat.
    /// The static fallback when no [`world::World`] is attached.
    terrain_height: Vec<u8>,
    /// The living level (MC1/HW): triggers, dispositions, runtime
    /// terrain events. None = static terrain (MC2 until its feature
    /// pass is ported, or bare test sims).
    pub world: Option<world::World>,
}

impl Simulation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_terrain(terrain_height: Vec<u8>) -> Self {
        debug_assert!(terrain_height.is_empty() || terrain_height.len() == MAP_TILES * MAP_TILES);
        let mut sim = Self {
            terrain_height,
            ..Self::default()
        };
        sim.sync_carpet_from_flyer();
        sim
    }

    /// A sim over a living world; the flight clamp follows the world's
    /// mutating height plane.
    pub fn with_world(world: world::World) -> Self {
        let mut sim = Self {
            world: Some(world),
            ..Self::default()
        };
        sim.sync_carpet_from_flyer();
        sim
    }

    /// Ground altitude in tile units at a world position (nearest tile;
    /// the engine interpolates across the tile's two triangles, which
    /// can wait until collision matters beyond a hover clamp).
    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        if let Some(w) = &self.world {
            return w.ground_height_tiles(x, z);
        }
        if self.terrain_height.is_empty() {
            return 0.0;
        }
        let tx = (x.floor() as i64).rem_euclid(MAP_TILES as i64) as usize;
        let tz = (z.floor() as i64).rem_euclid(MAP_TILES as i64) as usize;
        self.terrain_height[tz * MAP_TILES + tx] as f32 * HEIGHT_SCALE
    }

    /// Re-seed the faithful integer carpet from `flyer` (spawn, level
    /// hand-off, tests that set the flyer directly).
    pub fn sync_carpet_from_flyer(&mut self) {
        let f = &self.flyer;
        self.carpet = flight::Mc1State::from_tiles(f.x, f.z, f.y, f.yaw);
    }

    /// Advance exactly one fixed tick.
    pub fn step(&mut self, input: &FlightInput) {
        self.tick += 1;

        // The death fall / dead wait override the controls: the
        // original's dead wizard never reaches the command handler
        // (sub_46840 is skipped from state 2 on) — the stick filters
        // decay, the speed targets freeze, casts stop. Only the
        // respawn key passes through.
        let (falling, dead) = match &self.world {
            Some(w) => (w.player_falling(), w.player_dead()),
            None => (false, false),
        };
        let mut input = *input;
        if falling || dead {
            input = FlightInput {
                respawn: input.respawn,
                ..FlightInput::default()
            };
        }
        // The MC2 ending sequence seizes the flyer (retail swaps the
        // player's actionIndex to 11, sub_5E8C0 — every control
        // input dies with it; the scripted pose lands after the
        // world tick below).
        let end_seized = self
            .world
            .as_ref()
            .is_some_and(|w| w.mc2_end_pose().is_some());
        if end_seized {
            input = FlightInput::default();
        }
        let input = &input;

        // The Accelerate cancel reads the tick's raw thrust input
        // BEFORE anything moves. Retail cancels on the v_14
        // speed-TOUCHED flag (:65144-51), and v_14 arms only when
        // the press actually moves v_12 (:55766-80) — while boosted,
        // v_12 (±160/240) sits outside the ±80 input clamp, so the
        // aligned press is inert and only the RESISTING press bites
        // (manual: "press the down cursor to cancel"). Both thrust
        // models share the directional law: v_14 is the resisting-press
        // edge, NOT any-press (hold + re-cast must survive).
        if let Some(w) = &mut self.world {
            w.thrust_cancel(input.thrust);
        }

        // The Backspace full stop, applied before the move like
        // retail's action dispatch (EF:37954): actual + target speed
        // to 0, the live Speed/Accelerate channel killed. Retail does
        // NOT touch strafe (it decays on its own) or hard-zero the
        // steering filters (they decay once the stick recenters).
        // `accel_was_active` clears too: retail's zeroed counter
        // skips the spell's guarded drive block, so no +80/minSpeed
        // restore may fire (EF:56203 guard).
        if input.full_stop {
            self.carpet.act_speed = 0;
            self.carpet.tgt_speed = 0;
            // The enhanced mover's float velocities are its speed
            // state — the enhancement's analog of the same stop.
            self.flyer.vx = 0.0;
            self.flyer.vy = 0.0;
            self.flyer.vz = 0.0;
            if let Some(w) = &mut self.world {
                w.full_stop_cancel_accel();
            }
            self.accel_was_active = false;
        }

        match self.thrust_model {
            ThrustModel::Mc1 => {
                // The faithful mover is game-keyed by the world's
                // flight verb: MC2 worlds fly sub_5D530, everything
                // else (and world-less sims) the MC1 arm.
                let mc2 = self
                    .world
                    .as_ref()
                    .is_some_and(|w| w.verbs().flight == verbs::FlightVerb::Mc2);
                if mc2 {
                    self.move_mc2(input);
                } else {
                    self.move_mc1(input);
                }
            }
            ThrustModel::Enhanced => self.move_enhanced(input),
        }

        // The death fall (sub_45FC0 :55466-77): gravity −2/tick²
        // (clamped −256) on top of the still-drifting move, riding
        // down to the ground+128 floor — touchdown is detected by
        // the world tick below at that exact altitude. Integrated in
        // FLYER space at the FLYER's position: the integer carpet's
        // x/y are stale under the enhanced mover (never synced after
        // spawn), so clamping against ground THERE would suspend the
        // corpse mid-air wherever the local ground sits lower.
        if falling && let Some(w) = &mut self.world {
            let dz = w.death_fall_step() as f32 / 256.0;
            let g = w.ground_height_tiles(self.flyer.x, self.flyer.z);
            let y = (self.flyer.y + dz).max(g + 0.5);
            self.flyer.y = y;
            self.flyer.vy = 0.0;
            self.carpet.z = ((y * 256.0) as i32).min(i16::MAX as i32) as i16;
        }
        // Dead (sub_463B0 :55575-91): speeds zeroed, the camera
        // turns toward the killer while the grey screen waits for
        // Space.
        if dead {
            self.carpet.tgt_speed = 0;
            self.carpet.act_speed = 0;
            self.carpet.strafe = 0;
            if let Some(w) = &self.world
                && let Some((kx, kz)) = w.killer_pos()
            {
                const RAD: f32 = std::f32::consts::TAU / 2048.0;
                let px = (self.flyer.x.rem_euclid(256.0) * 256.0) as u16;
                let py = (self.flyer.z.rem_euclid(256.0) * 256.0) as u16;
                let tx = (kx.rem_euclid(256.0) * 256.0) as u16;
                let ty = (kz.rem_euclid(256.0) * 256.0) as u16;
                let target = features::Gen::angle_between(px, py, tx, ty);
                let mut d = (target as i32 - self.carpet.yaw as i32) & 0x7FF;
                if d > 1024 {
                    d -= 2048;
                }
                let step = d.clamp(-16, 16);
                self.carpet.yaw = ((self.carpet.yaw as i32 + step) & 0x7FF) as u16;
                self.flyer.yaw += step as f32 * RAD;
            }
        }

        // The world turn: triggers/portals probe the flyer, events tick.
        if let Some(w) = &mut self.world {
            let f = self.flyer;
            // Forward speed in tiles/tick — the cast inherits it onto
            // the projectile's base speed like the carpet's +126.
            // Faithful: +126 itself, sign included; enhanced: the
            // velocity magnitude.
            let speed = match self.thrust_model {
                ThrustModel::Mc1 => self.carpet.act_speed as f32 / 256.0,
                ThrustModel::Enhanced => (f.vx * f.vx + f.vy * f.vy + f.vz * f.vz).sqrt() * TICK_DT,
            };
            w.tick(
                world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, speed),
                world::PlayerCommand {
                    fire_left: input.fire_left,
                    fire_right: input.fire_right,
                    equip_left: input.equip_left,
                    equip_right: input.equip_right,
                    mc2_select: input.mc2_select,
                    respawn: input.respawn,
                    demolish: input.demolish,
                },
            );
            // Respawn (sub_44D30): reposition at the castle, one
            // tile up (:54845-63 z = ground+256), flight state
            // zeroed (thrust target, strafe, knock — :54878-83),
            // heading preserved.
            if let Some((x, z)) = w.take_respawn() {
                let ground = w.ground_height_tiles(x, z);
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                f.y = ground + 1.0;
                f.vx = 0.0;
                f.vy = 0.0;
                f.vz = 0.0;
                self.carpet = flight::Mc1State::from_tiles(f.x, f.z, f.y, f.yaw);
            }
            if let Some((x, z)) = w.take_teleport() {
                // Portal arrival: the original moves the entity to the
                // destination point; altitude snaps above the ground
                // there (velocity, speeds and steering carry over —
                // only the position hands off to the integer state).
                let ground = w.ground_height_tiles(x, z);
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                f.y = f.y.max(ground + MIN_CLEARANCE);
                self.carpet.x = (x.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.y = (z.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.z = (self.flyer.y * 256.0) as i16;
            }
            // The MC2 ending sequence: mirror the scripted pose onto
            // the flyer (position + heading; the tail's roll/pitch
            // auto-level EF:60577-87 decays the visual bank here).
            if let Some((x, alt, z, yaw)) = w.mc2_end_pose() {
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                f.y = alt;
                f.yaw = yaw;
                f.vx = 0.0;
                f.vy = 0.0;
                f.vz = 0.0;
                // roll −= (roll − sign·7)>>3 / pitch −= (pitch −
                // sign·3)>>2, in float space: geometric decay to 0.
                f.roll -= f.roll * 0.125;
                f.pitch -= f.pitch * 0.25;
                self.carpet.x = (x.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.y = (z.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.z = (alt * 256.0) as i16;
                self.carpet.yaw = ((yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU
                    * 2048.0) as u16)
                    & 0x7FF;
                self.carpet.act_speed = 0;
                self.carpet.tgt_speed = 0;
            }
        }
    }

    /// The faithful MC1 mover (remc1 sub_455D0, ported in [`flight`])
    /// over the integer carpet state; `flyer` is derived from it for
    /// the renderer/camera afterwards.
    fn move_mc1(&mut self, input: &FlightInput) {
        // Seam telemetry for the boundary verbs this mover consumes
        // (crate::verbs). The MC2 mover is live (`move_mc2`), so an
        // MC2 verb set reaching THIS mover is a wiring bug — the
        // fallback notes make it visible instead of silent.
        if let Some(w) = &mut self.world {
            if w.verbs().flight == verbs::FlightVerb::Mc2 {
                w.note_verb_fallback(verbs::VerbKind::Flight);
            }
            if w.verbs().commit_gate == verbs::CommitGateVerb::Mc2 {
                w.note_verb_fallback(verbs::VerbKind::CommitGate);
            }
        }
        // Accelerate expiry/cancel edge: the spell handler resets the
        // target AND actual speed to +80 — MAX FORWARD, even out of
        // backwards flight (:65191-97; an authentic quirk).
        let over = self.world.as_ref().and_then(|w| w.accel_override());
        if self.accel_was_active && over.is_none() {
            self.carpet.tgt_speed = 80;
            self.carpet.act_speed = 80;
        }
        self.accel_was_active = over.is_some();

        let knock = self.world.as_mut().and_then(|w| w.take_knock_step());
        let inp = flight::Mc1Input {
            stick_x: input.stick_x.clamp(-127, 127),
            stick_y: input.stick_y.clamp(-127, 127),
            speed_up: input.thrust > 0.0,
            speed_down: input.thrust < 0.0,
            strafe_left: input.strafe < 0.0,
            strafe_right: input.strafe > 0.0,
        };
        let prev = self.carpet;
        let moved = match &self.world {
            Some(w) => flight::mc1_move(
                &mut self.carpet,
                &inp,
                over,
                knock,
                &|x, y| w.ground_z_engine(x, y),
                &|cur, prop| w.player_wall_gate_fixed(cur, prop),
            ),
            None => {
                let th = &self.terrain_height;
                let ground = |x: u16, y: u16| -> i16 {
                    if th.is_empty() {
                        return 0;
                    }
                    let (tx, ty) = ((x >> 8) as usize, (y >> 8) as usize);
                    th[ty * MAP_TILES + tx] as i16 * 32
                };
                flight::mc1_move(&mut self.carpet, &inp, over, knock, &ground, &|_, p| {
                    Some(p)
                })
            }
        };
        if moved.flutter {
            if let Some(w) = &mut self.world {
                w.push_player_sound(46);
            }
        }

        // Extended lift: the deliberate deviation, layered OUTSIDE the
        // ported routine — vertical only (it cannot cross a wall), the
        // z-floor stays, and float-up caps at the level's highest
        // terrain + the soft-ceiling band (never a god's-eye view).
        if self.altitude_model == AltitudeModel::ExtendedLift {
            let g = match &self.world {
                Some(w) => w.ground_z_engine(self.carpet.x, self.carpet.y),
                None => {
                    (self.ground_height(self.carpet.x as f32 / 256.0, self.carpet.y as f32 / 256.0)
                        * 256.0) as i16
                }
            };
            let floor = g.saturating_add(128);
            if input.lift != 0.0 {
                let ceil = ((self.lift_ceiling() * 256.0) as i32).min(i16::MAX as i32) as i16;
                let dz = (input.lift * LIFT_STEP) as i16;
                let new_z = self.carpet.z.saturating_add(dz);
                // Rising: capped at the ceiling, but never yanked DOWN
                // from altitude already held above it (wall-climb gains
                // are legitimate). Descending: the z-floor holds.
                self.carpet.z = if dz > 0 {
                    new_z.min(ceil.max(self.carpet.z))
                } else {
                    new_z.max(floor)
                };
            } else if self.carpet.z > floor {
                // Hover keys idle: the carpet settles gently toward
                // the terrain-follow floor (deliberate — gameplay
                // assumes ground-contact pickups like spell jars;
                // holding altitude forever made you overfly them).
                // Rate = the faithful 8/tick passive sink.
                self.carpet.z = (self.carpet.z - 8).max(floor);
            }
        }

        // MC2 cave ceiling: the player clamps (no bounce, no damage)
        // at ceiling − 384 (sub_5D530 EF:59758-63). After extended
        // lift so the deviation can't pierce the roof either. The
        // floor band wins in a low-headroom pinch (retail's branch
        // order — the roof never pins the carpet under the terrain).
        if let Some(w) = &self.world {
            if let Some(c) = w.player_cave_ceiling(self.carpet.x, self.carpet.y) {
                let floor = w
                    .ground_z_engine(self.carpet.x, self.carpet.y)
                    .saturating_add(128);
                let c = c.max(floor);
                if self.carpet.z > c {
                    self.carpet.z = c;
                }
            }
        }

        self.derive_flyer(prev);
    }

    /// Derive the float flyer for the renderer from the integer
    /// carpet: yaw stays CONTINUOUS (accumulated radians) across the
    /// 11-bit wrap so the camera lerp never spins the long way.
    /// Shared tail of the two faithful movers.
    fn derive_flyer(&mut self, prev: flight::Mc1State) {
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        let mut dyaw = (self.carpet.yaw as i32 - prev.yaw as i32) & 0x7FF;
        if dyaw > 1024 {
            dyaw -= 2048;
        }
        let wrapd = |a: u16, b: u16| b.wrapping_sub(a) as i16 as f32 / 256.0;
        let c = self.carpet;
        let f = &mut self.flyer;
        f.yaw += dyaw as f32 * RAD;
        // Engine pitch is positive-DOWN; the flyer's is positive-up.
        // This is the FULL aim pitch (casts use it); the app camera
        // renders half of it under the mc1 model (:52434).
        f.pitch = -(c.aim_signed() as f32) * RAD;
        // The camera bank: the same filtered stick that drives the
        // yaw rate, published at FULL value (:52432 / EF:40258) so
        // the horizon telegraphs the turn. Positive roll_f = stick
        // right = turn right = bank right.
        f.roll = c.roll_f as f32 * RAD;
        f.vx = wrapd(prev.x, c.x) / TICK_DT;
        f.vz = wrapd(prev.y, c.y) / TICK_DT;
        f.vy = (c.z.wrapping_sub(prev.z) as f32 / 256.0) / TICK_DT;
        f.x = c.x as f32 / 256.0;
        f.z = c.y as f32 / 256.0;
        f.y = c.z as f32 / 256.0;
    }

    /// The faithful MC2 mover (remc2 sub_5D530, ported in
    /// [`flight::mc2_move`]) — the real [`verbs::FlightVerb::Mc2`]
    /// arm (Phase 4.4, docs/traces/mc2-flight-model.md). Same
    /// boundary contract as [`Self::move_mc1`]: integer carpet state
    /// is authoritative, the flyer derives after.
    fn move_mc2(&mut self, input: &FlightInput) {
        if self.world.is_none() {
            // World-less sims have no MC2 gate/ceiling data.
            return self.move_mc1(input);
        }

        // The tuning row per map type (spawn-time in retail via
        // AddPlayer_4A920; the map type never changes mid-level).
        if let Some(w) = &self.world {
            self.carpet_mc2.row = w.mc2_carpet_row();
        }

        // The speed-up (MC2 spell 3) rides the Accelerate channel —
        // same expiry edge as MC1 (:65191-97 shape).
        let over = self.world.as_ref().and_then(|w| w.accel_override());
        if self.accel_was_active && over.is_none() {
            self.carpet.tgt_speed = 80;
            self.carpet.act_speed = 80;
        }
        self.accel_was_active = over.is_some();

        let knock = self.world.as_mut().and_then(|w| w.take_knock_step());
        // Debuff-stamp hits → the slow/stun web channels (§5c/5d).
        if let Some(w) = &mut self.world {
            let (slow, stun) = w.take_mc2_debuffs();
            for _ in 0..slow {
                self.carpet_mc2.slow_hit();
            }
            for _ in 0..stun {
                self.carpet_mc2.stun_hit();
            }
        }

        let inp = flight::Mc1Input {
            stick_x: input.stick_x.clamp(-127, 127),
            stick_y: input.stick_y.clamp(-127, 127),
            speed_up: input.thrust > 0.0,
            speed_down: input.thrust < 0.0,
            strafe_left: input.strafe < 0.0,
            strafe_right: input.strafe > 0.0,
        };
        let prev = self.carpet;
        let w = self.world.as_ref().expect("checked above");
        let moved = flight::mc2_move(
            &mut self.carpet,
            &mut self.carpet_mc2,
            &inp,
            over,
            knock,
            &|x, y| w.ground_z_engine(x, y),
            &|x, y| w.player_cave_ceiling(x, y),
            &|cur, prop| w.player_mc2_gate(cur, prop),
            &|pos, latched| w.player_mc2_stuck(pos, latched),
        );
        if moved.accel_cancel
            && let Some(w) = &mut self.world
        {
            w.mc2_cancel_accel();
            // Retail's cave-wall cancel zeroes the same guarded
            // counter as Backspace (EF:59603) — the spell dies with
            // NO minSpeed restore, so the expiry edge must not fire
            // +80 next tick over the dead stop.
            self.accel_was_active = false;
        }

        // Extended lift (the deviation) — lift key only: the faithful
        // row-0xe buoyancy IS the idle settle on this arm, so the MC1
        // path's idle-sink branch is deliberately absent. The floor
        // is ground+256 (the MC2 clearance, trace §9), and the cave
        // roof re-clamps after so the deviation can't pierce it.
        if self.altitude_model == AltitudeModel::ExtendedLift && input.lift != 0.0 {
            let (cx, cy) = (self.carpet.x, self.carpet.y);
            let w = self.world.as_ref().expect("checked above");
            let g = w.ground_z_engine(cx, cy);
            let floor = g.saturating_add(self.carpet_mc2.row.clearance);
            let ceil = ((self.lift_ceiling() * 256.0) as i32).min(i16::MAX as i32) as i16;
            let dz = (input.lift * LIFT_STEP) as i16;
            let new_z = self.carpet.z.saturating_add(dz);
            self.carpet.z = if dz > 0 {
                new_z.min(ceil.max(self.carpet.z))
            } else {
                new_z.max(floor)
            };
            if let Some(c) = w.player_cave_ceiling(cx, cy) {
                let c = c.max(floor);
                if self.carpet.z > c {
                    self.carpet.z = c;
                }
            }
        }

        self.derive_flyer(prev);
    }

    /// The enhanced mover: hold-to-fly with automatic deceleration —
    /// a deliberate deviation from the original (see [`ThrustModel`]).
    /// Obeys the level-plane thrust rule: thrust and the Accelerate
    /// override act in the yaw ground plane at full magnitude however
    /// far you aim up or down (aim pitch must never bleed dodge
    /// mobility into vertical motion).
    fn move_enhanced(&mut self, input: &FlightInput) {
        // The MC2 debuff webs are gameplay, so the deviation mover
        // services their channels too: drain the stamp hits, tick
        // the decay, scale the applied step by the slow level and
        // full-stop + settle under the paralyze (the faithful laws
        // live in flight::mc2_move; this is their float analog).
        if let Some(w) = &mut self.world {
            let (slow, stun) = w.take_mc2_debuffs();
            for _ in 0..slow {
                self.carpet_mc2.slow_hit();
            }
            for _ in 0..stun {
                self.carpet_mc2.stun_hit();
            }
        }
        self.carpet_mc2.tick_debuffs();
        let web_stop = self.carpet_mc2.mobilize > 0;
        let web_scale = if web_stop {
            0.0
        } else {
            (4 - self.carpet_mc2.move_speed) as f32 / 4.0
        };

        let f = &mut self.flyer;

        f.yaw += input.yaw_delta;
        f.pitch = (f.pitch + input.pitch_delta).clamp(-MAX_PITCH, MAX_PITCH);

        // Movement basis: the yaw ground plane (yaw 0 faces -Z;
        // right-handed Y-up). Aim pitch is for shooting only.
        let (sy, cy) = f.yaw.sin_cos();
        let fwd = [sy, 0.0, -cy];
        let right = [cy, 0.0, sy];

        // Explicit float up/down = the extended-lift enhancement only
        // (the faithful altitude model has no vertical control).
        let lift = match self.altitude_model {
            AltitudeModel::ExtendedLift => input.lift,
            AltitudeModel::Faithful => 0.0,
        };

        // The Accelerate override (types 2/21): while channeling, the
        // spell REPLACES the thrust model — normal thrust input is
        // IGNORED (strafe/lift/turn stay live) and velocity is driven
        // toward facing × factor × the normal full-thrust terminal
        // speed. Deliberately tier-independent: the original also
        // bypasses its own control scheme here — it writes the carpet
        // speed (a horizontal quantity) directly.
        let over = self.world.as_ref().and_then(|w| w.accel_override());
        let thrust = if over.is_some() { 0.0 } else { input.thrust };
        let ax = fwd[0] * thrust + right[0] * input.strafe;
        let ay = lift;
        let az = fwd[2] * thrust + right[2] * input.strafe;
        f.vx += ax * ACCEL * TICK_DT;
        f.vy += ay * ACCEL * TICK_DT;
        f.vz += az * ACCEL * TICK_DT;
        f.vx *= DRAG_PER_TICK;
        f.vy *= DRAG_PER_TICK;
        f.vz *= DRAG_PER_TICK;
        if let Some(k) = over {
            // The enhanced model's full-thrust terminal speed:
            // v = a·dt·d/(1−d) — pinned to FAITHFUL_CRUISE_TPS (7.5
            // tiles/s), so accelerate here reaches k× the SAME ceiling
            // the faithful carpet does.
            let vmax = ACCEL * TICK_DT * DRAG_PER_TICK / (1.0 - DRAG_PER_TICK);
            let tv = [fwd[0] * k * vmax, fwd[2] * k * vmax];
            // Snappy approach: "propelled", not "accelerating".
            f.vx += (tv[0] - f.vx) * 0.5;
            f.vz += (tv[1] - f.vz) * 0.5;
        }

        let from = (f.x, f.z, f.y);
        f.x += f.vx * TICK_DT * web_scale;
        f.z += f.vz * TICK_DT * web_scale;
        if web_stop {
            // The paralyze settle (−51 engine units/tick, EF:59750).
            f.y -= 51.0 / 256.0;
        } else {
            f.y += f.vy * TICK_DT;
        }

        // Forced knock displacement (the kraken buffet, Type_160
        // v_22/v_24 — :55204-218): part of the move, BEFORE the wall
        // gate, so the drag cannot pull the carpet through a wall.
        if let Some(w) = &mut self.world {
            if let Some((dir, mag)) = w.take_knock_step() {
                let a = dir as f32 * std::f32::consts::TAU / 2048.0;
                let d = mag as f32 / 256.0; // engine units → tiles
                f.x += d * a.sin();
                f.z -= d * a.cos();
            }
        }

        // Wrap into [0, 256) like the original's 16-bit axes.
        f.x = f.x.rem_euclid(MAP_TILES as f32);
        f.z = f.z.rem_euclid(MAP_TILES as f32);

        // The human commit gate (sub_45410): type-8 walls are
        // horizontally impassable at any altitude — slide along the
        // nearer cardinal or discard the whole move. Blocking is the
        // explicit gate, not the height clamp; the burn-to-breach
        // castle exploit lives on the terrain side and is unaffected.
        if let Some(w) = &self.world {
            match w.player_wall_gate(from, (f.x, f.z, f.y)) {
                Some((x, z, alt)) => {
                    f.x = x;
                    f.z = z;
                    f.y = alt;
                }
                None => {
                    f.x = from.0;
                    f.z = from.1;
                    f.y = from.2;
                }
            }
            // The cave narrow-space refusal (moveTest_5D0A0's
            // sub_11E20 arm): retail never commits the carpet into
            // an air band tighter than clearance+fov+384 — the
            // funnel seams where floor meets ceiling are simply
            // unreachable, which is what keeps the retail eye clear
            // of the pinch line. The deviation mover needs the same
            // law; asymmetric (refuse ENTERING only) so a carpet
            // already in a tight spot can always fly back out.
            if w.player_cave_squeeze(f.x, f.z) && !w.player_cave_squeeze(from.0, from.1) {
                f.x = from.0;
                f.z = from.1;
                // The dead-stop analog (retail zeroes speed on the
                // cave refusal, EF:59602).
                f.vx = 0.0;
                f.vz = 0.0;
            }
        }

        let ground = self.ground_height(self.flyer.x, self.flyer.z);
        // The death fall must reach the ground+128 touchdown — the
        // living hover clearance sits ABOVE it and would hold the
        // corpse off the ground forever.
        let dead_fall = self.world.as_ref().is_some_and(|w| w.player_falling());
        let floor = ground + if dead_fall { 0.5 } else { MIN_CLEARANCE };
        let ceiling = self.lift_ceiling();
        // MC2 cave roof (sub_5D530 EF:59758-63): hard clamp at
        // ceiling − 384 on THIS model too — the enhanced thrust
        // must not fly through the cave ceiling either.
        let cave_ceiling = self.world.as_ref().and_then(|w| {
            let ex = (self.flyer.x.rem_euclid(256.0) * 256.0) as u16;
            let ez = (self.flyer.z.rem_euclid(256.0) * 256.0) as u16;
            w.player_cave_ceiling(ex, ez).map(|c| c as f32 / 256.0)
        });
        let f = &mut self.flyer;
        if f.y < floor {
            f.y = floor;
            f.vy = f.vy.max(0.0);
        }
        // The cap only stops further RISING past the ceiling — it
        // never pulls down altitude already held (the faithful model
        // has no hard ceiling; wall-climb altitude is legitimate).
        if f.y > ceiling && f.y > from.2 {
            f.y = from.2.max(ceiling);
            f.vy = f.vy.min(0.0);
        }
        // The cave roof is a HARD clamp (retail clamps every tick,
        // no altitude grandfathering under a rock ceiling) — RAW,
        // ceiling wins (sub_5D530 EF:59757-63). The squeeze gate
        // above keeps the pinch band unreachable; if numerics ever
        // brush it anyway, a brief under-floor frame renders as solid
        // rock (the terrain shader's backface arm), never the void.
        if let Some(c) = cave_ceiling {
            if f.y > c {
                f.y = c;
                f.vy = f.vy.min(0.0);
            }
        }
        // The faithful passive settle, inherited: at rest above the
        // soft-ceiling band, sink 8 engine units/tick (the original's
        // only downward drift) — without it, enhanced thrust under
        // the faithful altitude model would trap altitude forever
        // (level-plane thrust has no dive path).
        let speed = (f.vx * f.vx + f.vz * f.vz).sqrt();
        if speed < 0.05 && input.lift == 0.0 && f.y > ground + 4.0 {
            f.y -= 8.0 / 256.0;
        }
        // Extended lift with the hover keys idle: settle toward the
        // floor at any speed (deliberate — ground-contact pickups
        // assume the carpet comes down by itself).
        if self.altitude_model == AltitudeModel::ExtendedLift && input.lift == 0.0 && f.y > floor {
            f.y = (f.y - 8.0 / 256.0).max(floor);
        }
    }

    /// The altitude ceiling: the level's highest terrain tile plus the
    /// original's soft-ceiling band (ground+1024 = 4 tiles). Caps the
    /// extended-lift float-up so it never reaches a god's-eye view
    /// (deliberate); the faithful model can't climb past it anyway
    /// (climb authority inverts above the band).
    fn lift_ceiling(&self) -> f32 {
        let max_ground = match &self.world {
            Some(w) => w.max_ground_tiles(),
            None => self.terrain_height.iter().copied().max().unwrap_or(0) as f32 * HEIGHT_SCALE,
        };
        max_ground + 4.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal living world over flat height-100 terrain (the
    /// mortality boundary tests need World state, not just planes).
    fn flat_world(height: Vec<u8>) -> world::World {
        use crate::engine::features::{FeatureAssets, Planes};
        let planes = Planes {
            height,
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
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
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let assets = FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        world::World::new(planes, &[], 7, assets)
    }

    /// The death fall lands and the landing chain runs — faithful
    /// (integer) model.
    #[test]
    fn death_fall_lands_under_mc1() {
        let mut sim = Simulation::with_world(flat_world(vec![100; 0x10000]));
        sim.flyer.x = 112.5;
        sim.flyer.z = 116.5;
        sim.flyer.y = 100.0 / 8.0 + 3.0;
        sim.sync_carpet_from_flyer();
        sim.world.as_mut().unwrap().debug_kill_player();
        for _ in 0..200 {
            sim.step(&FlightInput::default());
            if sim.world.as_ref().unwrap().player_dead() {
                return;
            }
        }
        panic!("the corpse never landed (mc1)");
    }

    /// Under the ENHANCED mover the integer carpet's x/y are stale
    /// (spawn position) — the fall must integrate at the FLYER's
    /// position or the corpse suspends mid-air wherever the local
    /// ground is lower than the spawn's.
    #[test]
    fn death_fall_lands_under_enhanced_far_from_spawn() {
        // Spawn plateau high (200), death site low (40).
        let mut height = vec![40u8; MAP_TILES * MAP_TILES];
        for y in 0..MAP_TILES {
            for x in 120..140 {
                height[y * MAP_TILES + x] = 200;
            }
        }
        let mut sim = Simulation::with_world(flat_world(height));
        sim.thrust_model = ThrustModel::Enhanced;
        sim.altitude_model = AltitudeModel::ExtendedLift;
        // Carpet synced ON the plateau (level start)...
        sim.flyer.x = 128.0;
        sim.flyer.z = 128.0;
        sim.flyer.y = 200.0 / 8.0 + 2.0;
        sim.sync_carpet_from_flyer();
        // ...then the player flew to the lowland (enhanced never
        // re-syncs the integer carpet).
        sim.flyer.x = 60.0;
        sim.flyer.z = 60.0;
        sim.flyer.y = 40.0 / 8.0 + 3.0;
        sim.world.as_mut().unwrap().debug_kill_player();
        for _ in 0..300 {
            sim.step(&FlightInput::default());
            if sim.world.as_ref().unwrap().player_dead() {
                return;
            }
        }
        panic!("the corpse never landed (enhanced, far from spawn)");
    }

    /// Backspace full stop (retail action 0x27): both the actual and
    /// the TARGET speed zero — the carpet does not coast back up to a
    /// held target — and the stop sticks under idle input.
    #[test]
    fn full_stop_zeroes_actual_and_target_speed() {
        let mut sim = Simulation::new();
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..10 {
            sim.step(&fwd);
        }
        assert_eq!(sim.carpet.tgt_speed, 80);
        assert_eq!(sim.carpet.act_speed, 80);
        sim.step(&FlightInput {
            full_stop: true,
            ..Default::default()
        });
        assert_eq!(sim.carpet.tgt_speed, 0);
        assert_eq!(sim.carpet.act_speed, 0);
        for _ in 0..30 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.carpet.act_speed, 0, "no coast-back after the stop");
    }

    /// The full stop must also SUPPRESS the Accelerate expiry edge:
    /// retail's zeroed spell counter skips the guarded drive block,
    /// so no +80 restore fires over the stop (EF:56203 guard).
    #[test]
    fn full_stop_beats_the_accel_expiry_rebound() {
        let mut sim = Simulation::new();
        // A live boost last tick that vanished this tick (the shape
        // of a spell cancel): without full_stop this edge writes
        // tgt = act = +80 (:65191-97).
        sim.accel_was_active = true;
        sim.carpet.act_speed = 240;
        sim.carpet.tgt_speed = 240;
        sim.step(&FlightInput {
            full_stop: true,
            ..Default::default()
        });
        assert_eq!(sim.carpet.tgt_speed, 0, "no +80 rebound over the stop");
        assert_eq!(sim.carpet.act_speed, 0);
    }

    /// The enhanced mover's analog: the float velocities stop dead.
    #[test]
    fn full_stop_zeroes_enhanced_velocity() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..30 {
            sim.step(&fwd);
        }
        assert!(sim.flyer.vz.abs() > 0.1, "moving before the stop");
        sim.step(&FlightInput {
            full_stop: true,
            ..Default::default()
        });
        let v = sim.flyer.vx.abs() + sim.flyer.vy.abs() + sim.flyer.vz.abs();
        // One idle tick of drag may run after the zero; the stop
        // itself leaves at most that residue.
        assert!(v < 0.05, "stopped, |v| = {v}");
    }

    #[test]
    fn steps_are_counted() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        for _ in 0..10 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.tick, 10);
    }

    #[test]
    fn thrust_moves_and_drag_stops() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        let forward = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..30 {
            sim.step(&forward);
        }
        assert!(
            sim.flyer.z < 160.0,
            "forward thrust moves toward -Z, z = {}",
            sim.flyer.z
        );
        let coast = FlightInput::default();
        for _ in 0..300 {
            sim.step(&coast);
        }
        let speed = (sim.flyer.vx.powi(2) + sim.flyer.vy.powi(2) + sim.flyer.vz.powi(2)).sqrt();
        assert!(speed < 1e-3, "velocity decays to ~zero, got {speed}");
    }

    #[test]
    fn terrain_clamps_altitude() {
        let mut sim = Simulation::with_terrain(vec![80u8; MAP_TILES * MAP_TILES]);
        sim.thrust_model = ThrustModel::Enhanced;
        let dive = FlightInput {
            thrust: 1.0,
            pitch_delta: -1.0,
            ..Default::default()
        };
        for _ in 0..120 {
            sim.step(&dive);
        }
        // Ground is 80/8 = 10 tiles everywhere.
        assert!(sim.flyer.y >= 10.0 + MIN_CLEARANCE - 1e-4);
    }

    /// THE ALTITUDE ACCEPTANCE TEST (ROADMAP Phase 5): the authentic
    /// skill move — ride the ground-follow up a tall cliff face, dash
    /// away level, and the altitude HOLDS; only a full stop bleeds it
    /// at 8 engine units/tick. Runs the faithful model end to end
    /// (impulse thrust, floor ride, level-flight hold, speed-0 sink).
    #[test]
    fn wall_climb_skill_move() {
        // A 25-tile plateau spanning x tiles 130..200; lowland at 0.
        let mut th = vec![0u8; MAP_TILES * MAP_TILES];
        for y in 0..MAP_TILES {
            for x in 130..200 {
                th[y * MAP_TILES + x] = 200; // 200/8 = 25 tiles
            }
        }
        let mut sim = Simulation::with_terrain(th);
        sim.flyer.x = 120.0;
        sim.flyer.z = 128.0;
        sim.flyer.y = 0.5;
        sim.flyer.yaw = std::f32::consts::FRAC_PI_2; // east, toward the cliff
        sim.flyer.pitch = 0.0;
        sim.sync_carpet_from_flyer();

        // Phase 1 — ride the wall: hold accelerate into the cliff.
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..100 {
            sim.step(&fwd);
        }
        assert!(
            sim.flyer.x > 131.0,
            "reached the plateau, x={}",
            sim.flyer.x
        );
        assert!(
            (sim.flyer.y - 25.5).abs() < 0.1,
            "the floor carried the carpet up the face, y={}",
            sim.flyer.y
        );

        // Phase 2 — dash away level: decelerate through zero into
        // backward flight, off the cliff edge, pitch untouched.
        let back = FlightInput {
            thrust: -1.0,
            ..Default::default()
        };
        for _ in 0..110 {
            sim.step(&back);
        }
        assert!(
            sim.flyer.x < 129.0,
            "back over the lowland, x={}",
            sim.flyer.x
        );
        assert!(
            sim.flyer.y > 25.0,
            "level flight HOLDS the stolen altitude, y={}",
            sim.flyer.y
        );

        // Phase 3 — neutralize speed the authentic way (counter-
        // impulses; there is no stop key), then hover: 8/tick sink.
        while sim.carpet.tgt_speed < 0 {
            sim.step(&fwd);
        }
        while sim.carpet.act_speed != 0 {
            sim.step(&FlightInput::default());
        }
        let z0 = sim.carpet.z;
        for _ in 0..10 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.carpet.z, z0 - 80, "speed-0 hover bleeds 8/tick");
    }

    #[test]
    fn extended_lift_caps_at_highest_terrain() {
        let mut th = vec![0u8; MAP_TILES * MAP_TILES];
        th[0] = 80; // a lone 10-tile peak far away
        let mut sim = Simulation::with_terrain(th);
        sim.altitude_model = AltitudeModel::ExtendedLift;
        let rise = FlightInput {
            lift: 1.0,
            ..Default::default()
        };
        for _ in 0..500 {
            sim.step(&rise);
        }
        // Cap = highest terrain (10) + the soft-ceiling band (4).
        assert!(sim.flyer.y > 10.0, "lift works, y={}", sim.flyer.y);
        assert!(sim.flyer.y <= 14.01, "no god view, y={}", sim.flyer.y);
    }

    #[test]
    fn enhanced_thrust_stays_in_the_ground_plane() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        sim.flyer.y = 2.0; // inside the hover band (no passive settle)
        let y0 = sim.flyer.y;
        // Aim hard down, then thrust: motion must stay horizontal
        // (aim pitch is for shooting; it never steals mobility).
        let dive = FlightInput {
            pitch_delta: -1.4,
            ..Default::default()
        };
        sim.step(&dive);
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..60 {
            sim.step(&fwd);
        }
        assert!(
            (sim.flyer.y - y0).abs() < 1e-3,
            "no vertical bleed, y={}",
            sim.flyer.y
        );
        assert!(
            sim.flyer.z < 160.0,
            "full horizontal speed, z={}",
            sim.flyer.z
        );
    }

    #[test]
    fn world_wraps() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        sim.flyer.x = 255.9;
        sim.flyer.vx = 12.0;
        sim.step(&FlightInput::default());
        assert!(sim.flyer.x < 256.0);
    }
}
