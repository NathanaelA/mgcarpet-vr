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

mod combat;
pub mod features;
pub mod mc1_behavior;
pub mod mc1_entities;
pub mod mc1_sprite_stats;
pub mod mc1_tables;
mod mobs;
pub mod spells;
pub mod world;
mod tables;

/// Fixed simulation tick rate.
///
/// Placeholder value. The original advanced one "game turn" per rendered
/// frame, capped by hardware and later by remc2's 24 FPS limiter; the
/// authentic cadence needs to be measured against the reference before
/// gameplay logic lands here.
pub const TICK_RATE_HZ: u32 = 30;

/// Seconds per tick (render-side interpolation uses the same constant,
/// so keep a single definition).
pub const TICK_DT: f32 = 1.0 / TICK_RATE_HZ as f32;

/// Map side length in tiles; coordinates wrap modulo this.
pub const MAP_TILES: usize = 256;

/// Altitude of one height-byte step in tile units (engine: 32/256).
pub const HEIGHT_SCALE: f32 = 1.0 / 8.0;

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
    /// Left-hand cast held (the original's dw_0 bit 0x10; LMB).
    pub fire_left: bool,
    /// Right-hand cast held (dw_0 bit 0x20; RMB).
    pub fire_right: bool,
    /// Equip a spell to the left/right hand this tick (from the book
    /// screen or a quick key) — the original's commands 0x15/0x16.
    pub equip_left: Option<spells::SpellId>,
    pub equip_right: Option<spells::SpellId>,
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
        }
    }
}

/// Flight tuning. Placeholder feel, to be eyeballed against remc2
/// side-by-side before habits form (see docs/ROADMAP.md).
const ACCEL: f32 = 40.0; // tiles/s^2 at full thrust
const DRAG_PER_TICK: f32 = 0.90; // velocity retained per tick
const MAX_PITCH: f32 = 1.45; // radians
const MIN_CLEARANCE: f32 = 0.75; // tiles above ground
const CEILING: f32 = 40.0; // tiles

/// The whole game state and its single mutation entry point.
#[derive(Default)]
pub struct Simulation {
    /// Monotonic tick counter since level start. One tick = one of the
    /// original's game turns (events, water phase, sprite frames).
    pub tick: u64,
    pub flyer: Flyer,
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
        Self {
            terrain_height,
            ..Self::default()
        }
    }

    /// A sim over a living world; the flight clamp follows the world's
    /// mutating height plane.
    pub fn with_world(world: world::World) -> Self {
        Self {
            world: Some(world),
            ..Self::default()
        }
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

    /// Advance exactly one fixed tick.
    pub fn step(&mut self, input: &FlightInput) {
        self.tick += 1;

        // The Accelerate brake-cancel reads the tick's raw thrust
        // input BEFORE anything moves (manual: "press the down cursor
        // to cancel"; symmetric for backward — the resisting input is
        // the one control that works against the spell).
        if let Some(w) = &mut self.world {
            w.thrust_cancel(input.thrust);
        }

        let f = &mut self.flyer;

        f.yaw += input.yaw_delta;
        f.pitch = (f.pitch + input.pitch_delta).clamp(-MAX_PITCH, MAX_PITCH);

        // View basis: yaw 0 faces -Z; right-handed Y-up.
        let (sy, cy) = f.yaw.sin_cos();
        let (sp, cp) = f.pitch.sin_cos();
        let fwd = [sy * cp, sp, -cy * cp];
        let right = [cy, 0.0, sy];

        // The Accelerate override (types 2/21): while channeling, the
        // spell REPLACES the thrust model — normal thrust input is
        // IGNORED (strafe/lift/turn stay live) and velocity is driven
        // toward facing × factor × the normal full-thrust terminal
        // speed. Deliberately tier-independent: this must behave the
        // same under the future faithful MC1 thrust model (Phase 5),
        // because the original also bypasses its own control scheme
        // here — it writes the carpet speed directly.
        let over = self.world.as_ref().and_then(|w| w.accel_override());
        let thrust = if over.is_some() { 0.0 } else { input.thrust };
        let ax = fwd[0] * thrust + right[0] * input.strafe;
        let ay = fwd[1] * thrust + input.lift;
        let az = fwd[2] * thrust + right[2] * input.strafe;
        f.vx += ax * ACCEL * TICK_DT;
        f.vy += ay * ACCEL * TICK_DT;
        f.vz += az * ACCEL * TICK_DT;
        f.vx *= DRAG_PER_TICK;
        f.vy *= DRAG_PER_TICK;
        f.vz *= DRAG_PER_TICK;
        if let Some(k) = over {
            // The placeholder model's full-thrust terminal speed:
            // v = a·dt·d/(1-d) (12 tiles/s at current tuning).
            let vmax = ACCEL * TICK_DT * DRAG_PER_TICK / (1.0 - DRAG_PER_TICK);
            let tv = [fwd[0] * k * vmax, fwd[1] * k * vmax, fwd[2] * k * vmax];
            // Snappy approach: "propelled", not "accelerating".
            f.vx += (tv[0] - f.vx) * 0.5;
            f.vy += (tv[1] - f.vy) * 0.5;
            f.vz += (tv[2] - f.vz) * 0.5;
        }

        let from = (f.x, f.z, f.y);
        f.x += f.vx * TICK_DT;
        f.y += f.vy * TICK_DT;
        f.z += f.vz * TICK_DT;

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
        }

        let floor = self.ground_height(self.flyer.x, self.flyer.z) + MIN_CLEARANCE;
        let f = &mut self.flyer;
        if f.y < floor {
            f.y = floor;
            f.vy = f.vy.max(0.0);
        }
        if f.y > CEILING {
            f.y = CEILING;
            f.vy = f.vy.min(0.0);
        }

        // The world turn: triggers/portals probe the flyer, events tick.
        if let Some(w) = &mut self.world {
            let f = self.flyer;
            // Horizontal speed in tiles/tick — the cast inherits it
            // onto the projectile's base speed like the carpet's +126.
            let speed = (f.vx * f.vx + f.vy * f.vy + f.vz * f.vz).sqrt() * TICK_DT;
            w.tick(
                world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, speed),
                world::PlayerCommand {
                    fire_left: input.fire_left,
                    fire_right: input.fire_right,
                    equip_left: input.equip_left,
                    equip_right: input.equip_right,
                },
            );
            if let Some((x, z)) = w.take_teleport() {
                // Portal arrival: the original moves the entity to the
                // destination point; altitude snaps above the ground
                // there (velocity carries over).
                let ground = w.ground_height_tiles(x, z);
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                f.y = f.y.max(ground + MIN_CLEARANCE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_counted() {
        let mut sim = Simulation::new();
        for _ in 0..10 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.tick, 10);
    }

    #[test]
    fn thrust_moves_and_drag_stops() {
        let mut sim = Simulation::new();
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

    #[test]
    fn world_wraps() {
        let mut sim = Simulation::new();
        sim.flyer.x = 255.9;
        sim.flyer.vx = 12.0;
        sim.step(&FlightInput::default());
        assert!(sim.flyer.x < 256.0);
    }
}
