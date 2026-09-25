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
pub mod patches;
pub mod snapshot;
pub mod verbs;

pub use patches::WorldPatches;

/// The whirlwind camera-roll CRANK COUNT channel — harness telemetry
/// for the replay driver's stick recovery (see
/// [`mc2::tail::mc2_ww_crank_count_law`]). Never read by simulation
/// logic; the same contract as [`DEBUG_TICK`].
pub use mc2::tail::{mc2_ww_crank_count_law, reset_whirl_cranks, whirl_cranks};

/// Debug-trace tick correlation for env-gated probes (MGC_*_TRACE):
/// harness-side consumers stamp the recording tick here so sim-side
/// eprintlns can label themselves. Never read by simulation logic.
pub static DEBUG_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// MGC_MAIL_TRACE gate: `Some(t)` (the current [`DEBUG_TICK`]) when the
/// mailbox trace is enabled and t falls inside the toggle's optional
/// `t0:t1` window (`MGC_MAIL_TRACE=1` = everywhere).
pub fn mail_trace() -> Option<u64> {
    let v = std::env::var("MGC_MAIL_TRACE").ok()?;
    if v.is_empty() {
        return None;
    }
    let t = DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed);
    if let Some((a, b)) = v.split_once(':')
        && let (Ok(a), Ok(b)) = (a.parse::<u64>(), b.parse::<u64>())
    {
        if t < a || t > b {
            return None;
        }
    }
    Some(t)
}

/// The pool slot [`Gen::mail_write`]/[`Gen::mail_write_single`] narrate
/// under `MGC_MAIL_TRACE` — `MGC_MAIL_SLOT=<n>`, default 486 (the
/// mc1l4 castle the trace was first cut for). The player's own mailbox
/// always narrates.
pub fn mail_trace_slot() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("MGC_MAIL_SLOT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(486)
    })
}

use engine::world;
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

/// THE EYE LIFT — the wizard's eye rides 128 engine units (half a
/// tile) above the carpet entity's own z. RETAIL LAW, and the same
/// literal in both games: the world draw is handed
/// `axis.z + 128`, never the raw carpet z —
/// MC2 `DrawWorld_411A0` (remc2 EventsFunctions.cpp:21575, mirrored
/// at :21606/:21868/:21899) and MC1 `DrawWorld_30D90_30DD0` (remc1
/// sub_main.cpp:26406, :26589). The per-frame view record itself is a
/// verbatim copy of the entity position (EF:40250-54 — it even calls
/// `getTerrainAlt_10C40` and discards the result), so 128 is the
/// entire head offset; nothing else lifts the camera.
///
/// It matters most where the ground is close: the faithful movers
/// floor the carpet at `ground + clearance` (MC2 256, MC1 128), so a
/// carpet parked on its castle pad renders from `pad_top + 384` in
/// MC2 and `pad_top + 256` in MC1 — the port rendering from the
/// carpet plane instead sat a full half-tile low (player report
/// 2026-08-05: "docked at your own castle the port is consistently
/// lower than retail"). [`Flyer::y`] stays the CARPET plane — it
/// round-trips through [`Simulation::sync_carpet_from_flyer`] and
/// feeds the world its pose — so the lift belongs to whoever builds
/// the camera, never to the pose.
pub const EYE_LIFT: f32 = 128.0 / 256.0;

/// The thrust/steering model — the G-class flight-control tier
/// (ROADMAP "Flight-control tiers"). Selected once at the sim
/// boundary; replays must record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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
    /// level-plane thrust rule (aim pitch never steals horizontal
    /// mobility; vertical belongs to the altitude model's law).
    /// Steering is chase-the-pointer: the mouse moves the aim
    /// crosshair (a desired heading, clamped on-screen) and the
    /// carpet turns to chase it with an ease-out curve; casts launch
    /// along the crosshair while the hull catches up. The camera
    /// banks ∝ turn_rate × forward speed — "an actual flyer, not a
    /// toy" (player ruling; retail's fixed velocity-independent bank
    /// is the departure point).
    Enhanced,
}

/// The altitude model — the second G-class tier. `Faithful` =
/// terrain-follow only (the carpet floats up along rising ground and
/// settles by itself; no fly-up control exists). `ExtendedLift` is
/// the desired-altitude law — no original equivalent: q/e pin a
/// GROUND-RELATIVE desired altitude the carpet drifts toward at the
/// game's standard descent speed, capped at the per-game climb band
/// (1024 over terrain, both games, caves included) and never bypassing wall
/// blocking or the cave roof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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
    /// Up (+) / down (-) — the enhanced-altitude q/e axis: steps the
    /// carpet at the classic full-pitch climb rate and pins the
    /// ground-relative DESIRED altitude (see [`lift_desired_law`]).
    /// Ignored under [`AltitudeModel::Faithful`].
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
    /// Cycle-ring membership write (the pane's SHIFT+click, retail
    /// cmd 0x26): (spell index, 0 = none / 1 = left ring / 2 =
    /// right). Both games' columns consume it.
    pub spell_ring: Option<(u8, u8)>,
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
    /// The suicide key (Shift+K — retail in BOTH games: MC1
    /// :20492-93, MC2 PlayerInput.cpp:239-41). Not a control word in
    /// retail (a direct life write in the key handler), so captures
    /// never carry it; live input only.
    pub suicide: bool,
    /// A retail CHEAT (control opcode 30 — ALT+F-key in both games),
    /// recovered from a recording's own cheat toast — or, since the
    /// win-level cheat landed, set LIVE by the app's Shift+C
    /// ([`mgc_formats::recover::Cheat::WinLevel`]); either way it
    /// rides the recorded port-input lane, so a live-cheated take
    /// replays its own cheats.
    pub cheat: Option<mgc_formats::recover::Cheat>,
    /// MC2's barrel roll trigger — both strafe keys pressed the same
    /// tick from neutral (the app's edge detect mirrors retail's
    /// prev-frame strafe byte, PlayerInput.cpp:2080-97 → command bit
    /// 0x80). Ignored off-MC2.
    pub barrel_roll: bool,
    /// Raw mouse-X counts accumulated this tick (unscaled device
    /// units) — the roll's abort sense: a per-tick |dx| > 16 means
    /// the player grabbed the stick (sub_55C60 EF:38951-56).
    pub raw_dx: i16,
    /// REPLAY-ONLY exact move byte (retail `dw_0` bits 1/2 speed,
    /// 4/8 strafe): when set, the faithful movers consume these bits
    /// verbatim instead of deriving them from the float axes. The
    /// float mapping cannot express retail's both-bits-held states
    /// (both speed keys, both strafes — the latter resolves RIGHT by
    /// the sequential-bit-test law), so a recovered retail stream
    /// needs the byte itself. Live play leaves it `None`.
    pub mc1_move_byte: Option<u8>,
    /// REPLAY-ONLY MC2 speed command: the recovered per-player
    /// `cmd_speed` lane (mouse-proportional). When set, it becomes
    /// the mover's target speed verbatim and the speed key bits stay
    /// out of the integrator. Live play leaves it `None`.
    pub mc2_cmd_speed: Option<i16>,
    /// REPLAY-ONLY MC2 modal park (big map / spell book open):
    /// retail keeps playing but stops the carpet dead — speeds snap
    /// to 0, no translation, pose filters keep integrating.
    pub mc2_park: bool,
}

/// The faithful movers' input view: the float axes' sign mapping, or
/// the replay exact byte when the caller supplies one.
fn mc1_input(input: &FlightInput) -> flight::Mc1Input {
    let (up, down, left, right) = match input.mc1_move_byte {
        Some(mb) => (mb & 1 != 0, mb & 2 != 0, mb & 4 != 0, mb & 8 != 0),
        None => (
            input.thrust > 0.0,
            input.thrust < 0.0,
            input.strafe < 0.0,
            input.strafe > 0.0,
        ),
    };
    flight::Mc1Input {
        stick_x: input.stick_x.clamp(-127, 127),
        stick_y: input.stick_y.clamp(-127, 127),
        // A replay-recovered MC2 speed command replaces the key
        // servo whole (fed as tgt_speed by `mc2_walk_prelude`).
        speed_up: up && input.mc2_cmd_speed.is_none(),
        speed_down: down && input.mc2_cmd_speed.is_none(),
        strafe_left: left,
        strafe_right: right,
        // Set world-side, at the carpet's dispatch — only the death
        // fall clears it (`World::step_player_flight`).
        no_command: false,
        // Likewise world-side: the whirlwind's grab arms arm
        // `byte[1] & 8` on the carpet record and
        // `World::step_player_flight_mc2` reads it (dig Q7).
        mc2_stop: false,
        mc2_park: input.mc2_park,
    }
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
    /// mover banks ∝ turn_rate × forward speed (deliberate — zero
    /// when standing, unlike retail's fixed bank).
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

// --- Enhanced-throttle steering: chase-the-pointer + proportional
// bank (deliberate deviations — docs/DEVIATIONS.md "enhanced flight").
// Retail turning is velocity-independent and banks a fixed amount even
// in place. The enhanced tier (player design, 2026-07-23, Gothic-3
// precedent): the mouse moves a DESIRED heading — the aim crosshair —
// and the carpet turns to chase it with an ease-out curve (rate ∝
// remaining error, capped). Yaw twin of the desired-altitude law.
// (Supersedes the one-session turn-rate damper, which collapsed into
// all-or-nothing: a mouse reports displacement, not velocity.)
/// Turn-rate cap, radians/sec — matched to the classic model's own max
/// (roll filter 254 → yaw += 254/8 per tick ≈ 2.33 rad/s at 24 Hz).
const TURN_RATE_MAX: f32 = 2.4;
/// Chase convergence gain, 1/sec: turn rate = lead × CHASE_GAIN
/// (capped), so the carpet closes on the pointer exponentially with a
/// ~200 ms time constant; leads beyond TURN_RATE_MAX/CHASE_GAIN
/// (≈27°) turn at the full capped rate.
const CHASE_GAIN: f32 = 5.0;
/// The crosshair lead clamp, radians (±34°): the desired heading
/// stays INSIDE the view at any aspect (half-hFOV at 4:3 ≈ 37.6°), so
/// the crosshair position is always the truth (player ruling). Public
/// so the app's frame-rate crosshair prediction clamps identically.
pub const LEAD_MAX: f32 = 0.6;
/// Bank = BANK_SCALE · turn_rate (rad/s) · signed forward speed
/// (tiles/s), clamped ±BANK_MAX — camera roll only, zero when
/// standing, gated off while strafing. Retuned 2026-07-23 (player:
/// 0.030 slammed every turn to the clamp): a max-rate turn at cruise
/// now banks ~11°, leaving the clamp for Accelerate-boosted turns.
const BANK_SCALE: f32 = 0.015;
const BANK_MAX: f32 = 0.6;

/// The enhanced mover's bank law in one place: `turn_rate` in rad/s,
/// `fwd_speed` the SIGNED forward speed in tiles/s. Public so the
/// app's `--thirdperson` chase view can bank the subject sprite off
/// the recorded motion with exactly the formula the camera uses.
pub fn enhanced_bank(turn_rate: f32, fwd_speed: f32) -> f32 {
    (BANK_SCALE * turn_rate * fwd_speed).clamp(-BANK_MAX, BANK_MAX)
}

// --- Enhanced-altitude: the desired-altitude law (deliberate
// deviation — docs/DEVIATIONS.md "enhanced flight"). q/e pin a
// GROUND-RELATIVE desired altitude; the carpet drifts toward it at
// the game's standard descent speed and q/e move at the classic
// model's own max climb rate.
/// The q/e step, engine units/tick — the classic carpet's max climb/
/// descent at full pitch and normal cruise (80·sin(254·τ/2048) ≈ 56;
/// speed-spell scaling deliberately ignored, player write-up).
const LIFT_QE_STEP: i16 = 56;
/// The MC1 drift rate toward the desired altitude, engine units/tick
/// (= MC1's standard 8/tick sink; the MC2 arms use the row's own
/// |buoyancy| so drift always matches the game's passive descent).
const LIFT_DRIFT_MC1: i16 = 8;
/// MC1's enhanced-lift ceiling over terrain: retail's soft ceiling,
/// ground+1024 (:55151). MC2 reads its carpet row's band instead —
/// 1024 open, 3072 in caves (row 104). Player ruling 2026-09-25:
/// retail's constants set the lift's range, floor to ceiling — the
/// lift only spares the pitch-and-fly micromanagement of reaching it.
const LIFT_BAND_MC1: i16 = 1024;

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
    /// Enhanced-throttle steering state (deliberate deviation).
    /// `aim_lead` = the chase-the-pointer desired-heading offset ahead
    /// of the carpet yaw, radians, clamped ±[`LEAD_MAX`] — the aim
    /// crosshair sits at yaw+lead and the carpet turns to close it.
    /// `turn_rate` = the ACTUAL rate the chase produced this tick,
    /// rad/s (feeds the proportional bank).
    aim_lead: f32,
    turn_rate: f32,
    /// Enhanced-altitude state (deliberate deviation): the pinned
    /// GROUND-RELATIVE desired altitude, engine units over terrain.
    /// PINNED through flings/pins/catapults (player ruling 6); reset
    /// to the spawn offset on respawn and re-seeded at level hand-off.
    lift_desired: i16,
    /// Weightless cheat (`gameplay.cheat.weightless`; `dev.lift_unclamped`
    /// until 2026-09-15; live-applied by the app): unclamp the
    /// desired-altitude band to the global lift
    /// ceiling (the level's highest terrain + the 4-tile soft-ceiling
    /// margin) instead of the per-game ground-relative band.
    pub weightless: bool,
    /// MC2's barrel roll driver state (`sub_55C60`) — see
    /// [`flight::BarrelRoll`]. Idle (all-default) off-MC2 and between
    /// rolls; hash-quiet at rest so pinned goldens stay unmoved.
    broll: flight::BarrelRoll,
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

    /// Deterministic digest of the FULL sim state: the world's own
    /// [`World::state_hash`](world::World::state_hash) folded together
    /// with the flight tier that lives out here.
    ///
    /// The world hash alone is blind to carpet momentum and aim —
    /// exactly where a snapshot/restore bug hides, since a restored
    /// world can be byte-perfect while the carpet resumes at the wrong
    /// speed (`docs/archive/DESIGN-SAVES.md`). This is the digest save/load
    /// fixtures compare; world-only goldens keep using the world's.
    ///
    /// The full destructure makes a new `Simulation` field a compile
    /// error here: extend the hash deliberately.
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let Simulation {
            tick,
            flyer,
            thrust_model,
            altitude_model,
            carpet,
            carpet_mc2,
            accel_was_active,
            turn_rate,
            aim_lead,
            lift_desired,
            weightless,
            broll,
            terrain_height,
            world: attached,
        } = self;
        let mut h = world::Fnv(0xcbf2_9ce4_8422_2325);
        tick.hash(&mut h);
        // Floats by BIT PATTERN (the `Player::speed_boost` precedent):
        // determinism here is bit-exact, and `-0.0` must not alias
        // `0.0`.
        for f in [
            flyer.x,
            flyer.y,
            flyer.z,
            flyer.vx,
            flyer.vy,
            flyer.vz,
            flyer.yaw,
            flyer.pitch,
            flyer.roll,
        ] {
            f.to_bits().hash(&mut h);
        }
        (thrust_model, altitude_model).hash(&mut h);
        carpet.hash(&mut h);
        carpet_mc2.hash(&mut h);
        accel_was_active.hash(&mut h);
        turn_rate.to_bits().hash(&mut h);
        aim_lead.to_bits().hash(&mut h);
        lift_desired.hash(&mut h);
        weightless.hash(&mut h);
        // Hash-quiet at rest (the transparent-at-pristine law): the
        // pinned goldens predate the barrel roll; a LIVE roll stamps a
        // tag byte (aliasing guard) + the driver state.
        if broll.active() {
            11u8.hash(&mut h);
            broll.hash(&mut h);
        }
        terrain_height.hash(&mut h);
        // Folded, not re-derived: the world keeps its own pinned
        // goldens and this tier adds to them.
        attached.as_ref().map(world::World::state_hash).hash(&mut h);
        h.finish()
    }

    /// The AIM heading, radians — where the crosshair points and casts
    /// launch. Under enhanced thrust this leads the carpet by the
    /// chase-steering lead (aim is instant, the hull catches up —
    /// player ruling 2026-07-23); under the faithful models aim IS the
    /// carpet heading.
    pub fn aim_yaw(&self) -> f32 {
        match self.thrust_model {
            ThrustModel::Enhanced => self.flyer.yaw + self.aim_lead,
            ThrustModel::Mc1 => self.flyer.yaw,
        }
    }

    /// Whether the MC2 barrel roll is running. The app recenters its
    /// virtual stick / mouse-yaw accumulator while it is (retail's
    /// `rollDelta` zero recenters the mouse stick the same way).
    pub fn barrel_rolling(&self) -> bool {
        self.broll.active()
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
    /// hand-off, tests that set the flyer directly). Also seeds the
    /// enhanced-altitude desired offset from the pose (ruling 6: the
    /// spawn altitude is the reset point).
    pub fn sync_carpet_from_flyer(&mut self) {
        let f = self.flyer;
        self.carpet = flight::Mc1State::from_tiles(f.x, f.z, f.y, f.yaw);
        self.seed_lift_desired();
    }

    /// Drop the Accelerate expiry edge (the replay RE-anchor: a fresh
    /// segment must not fire the +80 restore off the previous
    /// segment's stale arm).
    pub fn clear_accel_edge(&mut self) {
        self.accel_was_active = false;
    }

    /// The reverse hand-off: derive the float flyer WHOLESALE from
    /// the integer carpet (the replay anchor — the carpet was just
    /// seeded from a recorded closure and is the sole authority).
    /// Velocities zero; the continuous yaw accumulator re-bases here.
    pub fn sync_flyer_from_carpet(&mut self) {
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        let c = self.carpet;
        let f = &mut self.flyer;
        f.x = c.x as f32 / 256.0;
        f.z = c.y as f32 / 256.0;
        f.y = c.z as f32 / 256.0;
        f.yaw = c.yaw as f32 * RAD;
        f.pitch = -(c.aim_signed() as f32) * RAD;
        f.roll = c.roll_f as f32 * RAD;
        f.vx = 0.0;
        f.vy = 0.0;
        f.vz = 0.0;
        self.seed_lift_desired();
    }

    /// The desired-altitude offset band, engine units over terrain:
    /// retail's floor .. ceiling (MC1 128..[`LIFT_BAND_MC1`]; MC2 the
    /// carpet row — 256..1024 open, 256..3072 cave; the cave ROOF stays
    /// a separate dynamic clamp).
    fn lift_band(&self) -> (i16, i16) {
        match &self.world {
            Some(w) if w.verbs().flight == verbs::FlightVerb::Mc2 => {
                let row = w.mc2_carpet_row();
                (row.clearance, row.band)
            }
            _ => (128, LIFT_BAND_MC1),
        }
    }

    /// Seed the desired altitude from the current flyer pose (level
    /// hand-off / model switch), clamped into the game band.
    fn seed_lift_desired(&mut self) {
        let (x, z, y) = (self.flyer.x, self.flyer.z, self.flyer.y);
        let g = self.ground_height(x, z);
        let (lo, hi) = self.lift_band();
        self.lift_desired = (((y - g) * 256.0) as i32).clamp(lo as i32, hi as i32) as i16;
    }

    /// Switch the altitude model mid-run (the options-menu path):
    /// entering the enhanced tier re-seeds the desired altitude at
    /// the live pose so the carpet doesn't lunge for a stale target.
    pub fn set_altitude_model(&mut self, model: AltitudeModel) {
        if model == self.altitude_model {
            return;
        }
        self.altitude_model = model;
        if model == AltitudeModel::ExtendedLift {
            self.seed_lift_desired();
        }
    }

    /// Switch the thrust model mid-run WITH the mover-state hand-off
    /// (the options-menu path). A bare field assign leaks the inactive
    /// mover's stale state: the integer carpet resumes at its spawn
    /// seed (a phantom warp + inverted velocity), and the float flyer
    /// keeps velocities the faithful mover never writes.
    pub fn set_thrust_model(&mut self, model: ThrustModel) {
        if model == self.thrust_model {
            return;
        }
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        match model {
            ThrustModel::Mc1 => {
                // Enhanced → faithful: re-seed the integer carpet at
                // the live flyer pose, then carry momentum and aim
                // over — forward/strafe speed = the velocity projected
                // on the heading basis (engine units/tick, the ±80
                // cruise clamp), aim pitch seeded through the stick
                // filter so the published aim starts where the
                // enhanced camera was pointing.
                self.sync_carpet_from_flyer();
                let f = &self.flyer;
                let (sy, cy) = f.yaw.sin_cos();
                let fwd = f.vx * sy - f.vz * cy;
                let side = f.vx * cy + f.vz * sy;
                let sp = (fwd * TICK_DT * 256.0).clamp(-80.0, 80.0) as i16;
                self.carpet.act_speed = sp;
                self.carpet.tgt_speed = sp;
                self.carpet.strafe = (side * TICK_DT * 256.0).clamp(-80.0, 80.0) as i16;
                self.carpet.pitch_f = (-f.pitch / RAD).clamp(-254.0, 254.0) as i16;
                self.carpet.aim_pitch = (self.carpet.pitch_f as u16) & 0x7FF;
            }
            ThrustModel::Enhanced => {
                // Faithful → enhanced: the flyer pose is derived
                // fresh every tick; hand the carpet speeds over as
                // the float velocity (forward + strafe, tiles/sec).
                let c = self.carpet;
                let f = &mut self.flyer;
                let (sy, cy) = f.yaw.sin_cos();
                let fwd_v = c.act_speed as f32 / 256.0 / TICK_DT;
                let side_v = c.strafe as f32 / 256.0 / TICK_DT;
                f.vx = sy * fwd_v + cy * side_v;
                f.vz = -cy * fwd_v + sy * side_v;
                f.vy = 0.0;
            }
        }
        // The enhanced turn damper starts (and leaves) centered.
        self.turn_rate = 0.0;
        self.aim_lead = 0.0;
        // The v14 brake latch is the CLASSIC mover's (the enhanced
        // controls brake immediately and never write it): a stale
        // `true` carried across the switch would end every later
        // Accelerate/Speed cast on its first token pass.
        if let Some(w) = &mut self.world {
            w.mc1_v14 = false;
        }
        self.thrust_model = model;
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
            // ⚠ ONLY THE COMMAND DIES. `sub_46840` is skipped from
            // state 2 on, but the STICK lives in the input pass and
            // keeps steering a dying carpet all the way down — retail
            // aims and rolls through the whole fall (mc1l42
            // t=17307-17344). The freeze that goes with it (the speed
            // target and the strafe register) is the mover's own,
            // `Mc1Input::no_command`, set at the carpet's dispatch.
            input = FlightInput {
                respawn: input.respawn,
                stick_x: input.stick_x,
                stick_y: input.stick_y,
                ..FlightInput::default()
            };
        }
        // A DEAD wizard is PINNED at the grave. Retail's state-3
        // dispatch (`sub_463B0` :55575) runs NO MOVE AT ALL — that,
        // not a zeroed speed, is what pins the corpse, and it is why
        // the record still reads −80/−80/36 in the frozen registers
        // from the death tick to the respawn. The faithful walk models
        // it world-side ([`World::step_player_flight`]); the ENHANCED
        // tier has no such dispatch, so it keeps the momentum kill —
        // the float velocity above all, which is what used to slide
        // the camera along the terrain after touchdown.
        // FALLING is left alone either way: the death fall keeps its
        // horizontal glide down to touchdown.
        //
        // ⚠ BOTH GAMES. This carried an `!= FlightVerb::Mc2` clause —
        // true while MC2 had no state-3 arm of its own, stale the
        // moment one landed (`sub_5E7C0` → `sub_5E6C0`, which likewise
        // runs no move and likewise never zeroes the registers). The
        // clause is why the app's `--replay` diverged at mc2l0 t=11193
        // on `pose.act_speed` (retail 16, the pre-tick kill 0) while
        // the conformance replay — same walk, no pre-tick kill — read
        // the take bit-exact to its end.
        // The walk-slot movers: the classic AND the enhanced controls,
        // both games — the enhanced controls are a propulsion kernel
        // inside the ONE faithful dispatch (`flight::CarpetPropel`).
        // Only world-less sims keep the pre-tick `move_enhanced`.
        let walk_mc2 = self
            .world
            .as_ref()
            .is_some_and(|w| w.verbs().flight == verbs::FlightVerb::Mc2);
        let classic_walk = self.thrust_model == ThrustModel::Mc1 && self.world.is_some();
        let enhanced_walk = self.thrust_model == ThrustModel::Enhanced && self.world.is_some();
        let walk = classic_walk || enhanced_walk;
        if dead {
            self.flyer.vx = 0.0;
            self.flyer.vy = 0.0;
            self.flyer.vz = 0.0;
            // The enhanced turn damper is steering state — pinned too.
            self.turn_rate = 0.0;
            self.aim_lead = 0.0;
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

        // MC2's barrel roll: the case-6 start gate arms only from
        // idle (EF:37628-29) — a roll in progress swallows the
        // command — and only on an MC2 world (retail MC1 has no such
        // move). While a roll runs, the bank stick is pinned centered
        // (retail zeroes `rollDelta` every driver tick, EF:38957) —
        // X only, the pitch stick stays live. The START tick's move
        // still sees the live stick, like retail: the driver's zero
        // runs after the move.
        let rolling_before = self.broll.active();
        if input.barrel_roll
            && self
                .world
                .as_ref()
                .is_some_and(|w| w.verbs().flight == verbs::FlightVerb::Mc2)
        {
            self.broll.arm();
        }
        if rolling_before {
            input.stick_x = 0;
            input.yaw_delta = 0.0;
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
        // edge, NOT any-press (hold + re-cast must survive). Under the
        // faithful MC1 mover the kill itself rides the mover's v_14
        // latch (one token pass later, retail's phase); the enhanced
        // mover never runs that integration, so it keeps the
        // immediate resisting-press kill.
        if let Some(w) = &mut self.world {
            // Replay byte drive: the cancel's directional sense comes
            // off the byte (speed-up wins — the replay driver's law).
            let thrust = match input.mc1_move_byte {
                Some(mb) if mb & 1 != 0 => 1.0,
                Some(mb) if mb & 2 != 0 => -1.0,
                Some(_) => 0.0,
                None => input.thrust,
            };
            w.thrust_cancel(thrust);
            if self.thrust_model == ThrustModel::Enhanced {
                w.accel_brake_immediate(thrust);
            }
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
            // The enhanced turn damper recenters with the steering
            // (the stop's SetCenterScreenForFlyAssistant analog).
            self.turn_rate = 0.0;
            self.aim_lead = 0.0;
        }

        // The faithful mover on a live world steps INSIDE the world
        // turn, at the carpet's walk slot (World::tick_flight): its
        // ground probe must read terrain the lower-slot painters
        // stamped THIS tick (the t=563 replay-wall law) and the
        // damage mailbox that arms its knock has to run in the same
        // dispatch. BOTH games now — MC2 joined on the mc2l3 t=244 /
        // mc2l0 t=4104 heads, which are those two facts exactly.
        // World-less sims and the enhanced flyer keep the pre-tick
        // move.
        // The faithful mover is game-keyed by the world's flight
        // verb: MC2 worlds fly sub_5D530, everything else (and
        // world-less sims) the MC1 arm.
        // The enhanced kernel's reference pose: the carpet BEFORE the
        // prelude, so the prelude's spin reaches the flyer too.
        let pre_walk = self.carpet;
        match self.thrust_model {
            ThrustModel::Mc1 => {
                if !classic_walk {
                    // World-less: MC2's gate/ceiling data needs a world.
                    self.move_mc1(input);
                } else if walk_mc2 {
                    self.mc2_walk_prelude(input);
                }
            }
            ThrustModel::Enhanced if enhanced_walk => {
                if walk_mc2 {
                    self.mc2_walk_prelude(input);
                }
            }
            ThrustModel::Enhanced => self.move_enhanced(input),
        }

        // The death fall and the dead wait run world-side, at the
        // carpet's walk slot, for BOTH control modes.

        // The world turn: triggers/portals probe the flyer, events tick.
        let pcmd = world::PlayerCommand {
            fire_left: input.fire_left,
            fire_right: input.fire_right,
            equip_left: input.equip_left,
            equip_right: input.equip_right,
            mc2_select: input.mc2_select,
            // The cycle-ring cast shortcut is a REPLAY lane today: the
            // native shell has no ring pane to raise 0x40 from, so it
            // stays silent here (`GameInput` carries no cursor).
            mc2_ring_cast: None,
            spell_ring: input.spell_ring,
            respawn: input.respawn,
            demolish: input.demolish,
            suicide: input.suicide,
            cheat: input.cheat,
        };
        // Faithful MC1/HW on a live world: the pre-move channels are
        // sampled at the tick head (the conform replay driver's phase
        // law — Accelerate restore edge, armed knock), then the world
        // turn steps the carpet at its walk slot (tick_flight); the
        // flyer derives after.
        let mut walked_prev: Option<flight::Mc1State> = None;
        let mut kernel_refs = None;
        if walk && let Some(w) = &mut self.world {
            // The Accelerate kill and the speed writes resolve
            // INSIDE the walk at the token's own slot (below-carpet
            // mail via World::step_player_flight, above-carpet mail
            // taken after the turn below); this tick-head sample is
            // just the drive's initial value and the faithful MC1
            // world path forces the override off.
            let over = w.accel_override();
            let prev = self.carpet;
            let mut kernel = enhanced_walk.then(|| EnhancedKernel {
                flyer: &mut self.flyer,
                aim_lead: &mut self.aim_lead,
                turn_rate: &mut self.turn_rate,
                input: *input,
                over,
                pitch_climbs: self.altitude_model == AltitudeModel::Faithful,
                refs: KernelRefs::of(&pre_walk),
            });
            let mut drive = world::FlightDrive {
                propel: kernel
                    .as_mut()
                    .map(|k| k as &mut dyn flight::CarpetPropel),
                s: &mut self.carpet,
                inp: mc1_input(input),
                over,
                falling,
                dead,
                mc2: walk_mc2.then_some(world::Mc2Drive {
                    ext: &mut self.carpet_mc2,
                    accel_was_active: &mut self.accel_was_active,
                }),
            };
            w.tick_flight(&mut drive, pcmd);
            drop(drive);
            kernel_refs = kernel.map(|k| k.refs);
            walked_prev = Some(prev);
        }
        if let Some(prev) = walked_prev
            && walk_mc2
        {
            self.mc2_walk_lift(input, falling, dead);
            match kernel_refs {
                // The enhanced flyer follows whatever the shared
                // dispatch did to the carpet after its kernel ran
                // (knock, leash, gate, vertical, lift law).
                Some(refs) => refs.follow(&mut self.flyer, &self.carpet),
                None => self.derive_flyer(prev),
            }
        } else if let Some(prev) = walked_prev {
            // Enhanced altitude: the desired-altitude law (deliberate
            // deviation), after the in-walk move — vertical only, the
            // z-floor stays (see move_mc1's pre-tick twin). Skipped
            // during the death fall/dead wait.
            if self.altitude_model == AltitudeModel::ExtendedLift && !falling && !dead {
                let g = self
                    .world
                    .as_ref()
                    .expect("faithful walk has a world")
                    .ground_z_engine(self.carpet.x, self.carpet.y);
                let (hi, cap) = self.lift_caps(g, LIFT_BAND_MC1);
                self.carpet.z = lift_desired_law(
                    self.carpet.z,
                    g,
                    &mut self.lift_desired,
                    input.lift,
                    128,
                    hi,
                    cap,
                    0,
                    LIFT_DRIFT_MC1,
                );
            }
            match kernel_refs {
                Some(refs) => refs.follow(&mut self.flyer, &self.carpet),
                None => self.derive_flyer(prev),
            }
        }
        // The barrel-roll driver runs after the move AND after the
        // flyer derives from it, like retail's per-player frame tail
        // (sub_57B20 then sub_55C60, EF:38081-82) — under the
        // faithful walk the move is inside the world turn above, so
        // `derive_flyer` would otherwise republish the bank roll
        // straight over the tumble. The tumble overrides the
        // bank-derived flyer roll for the frame; the finishing tick skips the write
        // (retail's phase-8 arm), so the normal bank publish resumes
        // seam-free — the masked rest angle `(|bank|+2048) & 0x7FF`
        // IS the live bank.
        if self.broll.active() {
            const ROLL_RAD: f32 = std::f32::consts::TAU / 2048.0;
            let bank = match self.thrust_model {
                ThrustModel::Mc1 => self.carpet.roll_f,
                // The enhanced bank is a derived float — feed it back
                // in angle units so the phase targets track it the
                // same way.
                ThrustModel::Enhanced => (self.flyer.roll / ROLL_RAD) as i16,
            };
            let out = self.broll.tick(bank, input.raw_dx);
            if out.lock_break
                && let Some(w) = &mut self.world
            {
                w.mc2_break_player_locks();
            }
            if let Some(v) = out.view {
                self.flyer.roll = v as f32 * ROLL_RAD;
            }
        }

        if let Some(w) = &mut self.world {
            // ⭐⭐ THE SPEED TOKEN'S REGISTER WRITE when its walk slot
            // is ABOVE the carpet's. `GetScroll_69DB0` (EF:56230-40)
            // slams both `speed_0xc_12` and `actSpeed` from the
            // MANIFESTATION's dispatch, so the phase is pure walk
            // order: a token below the carpet lands before `sub_5D530`
            // and is consumed inside the walk
            // (`World::step_player_flight_mc2`); one above lands after,
            // is recorded at THIS boundary and first moves the carpet
            // next tick.
            //
            // ⚠ The conformance driver (`mgc-conform`'s `replay.rs`)
            // has always taken this mail here and the app never did —
            // the second half of the app/conform split that showed as
            // mc2l0 t=22551. The two drivers' post-turn tails are ONE
            // LAW (the respawn arm below says the same in its own
            // comment); a channel added to one belongs in the other.
            //
            // ⚠ MC2 ONLY, exactly as in the conform driver, where this
            // take lives in `step_mc2` and NOT in `step_mc1`. MC1's
            // Accelerate mails the same channel but its burst-end
            // restore is consumed at the carpet's own dispatch
            // (`World::step_player_flight`) — taking it a second time
            // out here re-slams ±80 a tick later, which is the very
            // "compensation outlives what it compensated for" shape
            // this seam already produced once on the MC2 side.
            // Faithful tier only besides: the enhanced mover's speed
            // state is its float velocity and it takes the boost
            // through `accel_override`/`speed_boost` instead.
            // MC1 too, not just MC2: on an IMPORTED MC1 world the
            // carpet has a real pool slot, so an ABOVE-carpet token
            // posts its mail after the step — the take is a no-op
            // when step_player_flight already consumed it (native
            // MC1 dispatches the carpet post-walk: always below).
            //
            // ⚠⚠ BUT THE MAIL IS DRAINED ON EVERY TIER. The enhanced
            // tier discards it — and must, because the MC2 cast
            // thunks read an undelivered mail as the caster's live
            // `actSpeed` (`Gen::mc2_caster_act_speed`). Left standing,
            // the Speed token's LAST write outlived the spell for the
            // rest of the session: a burst cancelled while reversing
            // at Speed III left −400 behind, and every later
            // possession bolt launched at 384 − 400 = −16, i.e.
            // hanging in the air even from a standstill
            // (player-reported 2026-09-19).
            if let Some(base) = w.take_speed_base()
                && self.thrust_model == ThrustModel::Mc1
            {
                self.carpet.tgt_speed = base;
                self.carpet.act_speed = base;
            }
            // Respawn (sub_44D30): reposition at the castle's FULL
            // position (:54858-61), flight state zeroed (thrust
            // target, strafe, knock — :54878-83), heading preserved.
            //
            // ⚠ The altitude is the SEAT'S OWN z, handed over by the
            // sim — not `ground + 1.0`. The `tempZ._axis_2d.y++` at
            // :54848 is not what the engine lands on: mc1l42 t=17398
            // respawns on 3776 with the site's terrain reading exactly
            // 3776, and the old re-derivation put the carpet 256 units
            // high. That was the last pose divergence in the take, and
            // `mgc-conform replay` could not see it because its pose
            // channel GATES the death/respawn domain — the app's own
            // `--replay-check` is what caught it.
            if let Some((x, z, alt)) = w.take_respawn() {
                let yaw_i = self.carpet.yaw;
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                f.y = alt;
                f.vx = 0.0;
                f.vy = 0.0;
                f.vz = 0.0;
                if self.thrust_model == ThrustModel::Mc1 {
                    // ⭐ THE RESPAWN CLEARS EXACTLY THREE REGISTERS
                    // (:54868-83) — `v_12` (target speed), `v_16`
                    // (strafe) and the knock triple, which the world
                    // side already zeroes. The actual speed `+126`,
                    // the `+63` tick counter, the private LCG, the
                    // stick filters and the STALE `v_28` effective
                    // pitch are NOT touched, and the heading is kept.
                    //
                    // A full `from_tiles` here restarted the counter
                    // and the LCG and wiped `v_28`: mc1l42 t=17398
                    // reads eff_pitch 7 where the port gave 0. This is
                    // the same surgical form the replay verifier's own
                    // driver runs (`replay::step_mc1`) — the two must
                    // stay one law, and they had drifted apart, which
                    // is exactly what let the app's `--replay-check`
                    // see a divergence `mgc-conform replay` could not.
                    let s = &mut self.carpet;
                    s.x = (f.x.rem_euclid(256.0) * 256.0) as u16;
                    s.y = (f.z.rem_euclid(256.0) * 256.0) as u16;
                    s.z = (alt * 256.0) as i16;
                    s.tgt_speed = 0;
                    s.strafe = 0;
                    s.yaw = yaw_i;
                } else {
                    self.carpet = flight::Mc1State::from_tiles(f.x, f.z, f.y, f.yaw);
                }
                // Ruling 6: death/respawn resets the pinned desired
                // altitude to the spawn offset (ground + one tile).
                let (lo, hi) = if w.verbs().flight == verbs::FlightVerb::Mc2 {
                    let row = w.mc2_carpet_row();
                    (row.clearance, row.band)
                } else {
                    (128, LIFT_BAND_MC1)
                };
                self.lift_desired = 256i16.clamp(lo, hi);
                self.turn_rate = 0.0;
                self.aim_lead = 0.0;
            }
            if let Some((x, z, alt)) = w.take_teleport() {
                // Teleport arrival: `CopyEntityPosition` hands the
                // FULL destination axis to the wizard where retail
                // authors one — the vortex/pad warp lands exactly on
                // the destination ground (row v_12 = 0, both games),
                // the spell's castle/return legs carry the castle's/
                // saved z. `None` = the pitch-0 random hop: retail
                // leaves z untouched (the mover's own z-floor digs it
                // out of higher terrain next move, :55103-05).
                // Velocity/steering carry over — position only.
                let f = &mut self.flyer;
                f.x = x;
                f.z = z;
                if let Some(alt) = alt {
                    f.y = alt;
                }
                self.carpet.x = (x.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.y = (z.rem_euclid(256.0) * 256.0) as u16;
                self.carpet.z = (self.flyer.y * 256.0) as i16;
                // The pinned ground-relative desired altitude is a
                // port-model quantity — re-seed it at the arrival
                // pose (the spawn-seed law, inlined like the respawn
                // arm's): retail has no absolute altitude hold, so a
                // ground-level emergence must not auto-climb back to
                // the departure altitude.
                let g = w.ground_height_tiles(self.flyer.x, self.flyer.z);
                let (lo, hi) = if w.verbs().flight == verbs::FlightVerb::Mc2 {
                    let row = w.mc2_carpet_row();
                    (row.clearance, row.band)
                } else {
                    (128, LIFT_BAND_MC1)
                };
                self.lift_desired =
                    (((self.flyer.y - g) * 256.0) as i32).clamp(lo as i32, hi as i32) as i16;
            }
            if w.take_speed_zero() {
                // The teleport spell's `Type_160 v_12 = 0`
                // (:65583/:65601/:65614; MC2 EF:57029/57046): the
                // TARGET speed alone — the actual then chases down
                // 16/tick, retail's short glide-out. The enhanced
                // mover's float velocity is its speed state; kill the
                // horizontal component as the same stop (vertical is
                // outside retail's word).
                self.carpet.tgt_speed = 0;
                if self.thrust_model == ThrustModel::Enhanced {
                    self.flyer.vx = 0.0;
                    self.flyer.vz = 0.0;
                }
            }
            // The MC2 ending sequence: mirror the scripted pose onto
            // the flyer (position + heading; the tail's roll/pitch
            // auto-level EF:60577-87 decays the visual bank here).
            //
            // ⚠ ALTERNATE-MOVER ONLY. `sub_5E8C0_endGameSeq` is a
            // class-3 dispatch entry in the same action table as
            // `AddPlayer03_00_5E010`, so under the faithful walk it is
            // written at the carpet's OWN walk slot
            // (`World::mc2_carpet_dispatch`, the actionIndex-11/12
            // arm), in engine units — including the scripted `actSpeed`
            // that retail bleeds down 4/tick and re-accelerates 8/tick
            // to 200. Re-mirroring the float pose over the carpet here
            // undoes it: it re-quantizes yaw through a float round-trip
            // (mc2l0 t=22595 read 450 for retail's 451) and forces both
            // speed registers to 0 where retail scripts one and never
            // touches the other. The conformance driver dropped this
            // same block for the same reason; the flyer follows from
            // `derive_flyer` above.
            if !classic_walk && let Some((x, alt, z, yaw)) = w.mc2_end_pose() {
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
        debug_assert!(self.world.is_none(), "a live world flies the walk");
        let th = &self.terrain_height;
        let ground = |x: u16, y: u16| -> i16 {
            if th.is_empty() {
                return 0;
            }
            let (tx, ty) = ((x >> 8) as usize, (y >> 8) as usize);
            th[ty * MAP_TILES + tx] as i16 * 32
        };
        let inp = mc1_input(input);
        let prev = self.carpet;
        flight::mc1_move(&mut self.carpet, &inp, None, None, &ground, &|_, p| (true, p));
        if self.altitude_model == AltitudeModel::ExtendedLift {
            let g = (self.ground_height(self.carpet.x as f32 / 256.0, self.carpet.y as f32 / 256.0)
                * 256.0) as i16;
            let (hi, cap) = self.lift_caps(g, LIFT_BAND_MC1);
            self.carpet.z = lift_desired_law(
                self.carpet.z,
                g,
                &mut self.lift_desired,
                input.lift,
                128,
                hi,
                cap,
                0,
                LIFT_DRIFT_MC1,
            );
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
    /// The tick-head half of the MC2 mover, for the FAITHFUL WALK:
    /// everything retail's input pass (`PlayerEvents`) settles before
    /// the frame function, with the move itself left to the carpet's
    /// own walk slot ([`world::World::step_player_flight_mc2`]). The
    /// Accelerate restore edge, the knock and the debuff drain are
    /// NOT here — those all belong to the dispatch.
    fn mc2_walk_prelude(&mut self, input: &FlightInput) {
        // The tuning row per map type (spawn-time in retail via
        // AddPlayer_4A920; the map type never changes mid-level).
        if let Some(w) = &self.world {
            self.carpet_mc2.row = w.mc2_carpet_row();
        }
        // The tornado's forced turn (`sub_33340`'s wizard arm writes
        // `yaw_0x1C_28` on every arm — see `Gen::player_spin`), ahead
        // of the move so this tick's flight rides the imposed heading.
        if let Some(w) = &mut self.world {
            let spin = w.take_player_spin();
            if spin != 0 {
                self.carpet.yaw = (self.carpet.yaw as i32 + spin as i32) as u16 & 0x7FF;
            }
        }
        // A replay-recovered speed command IS the target (the
        // cmd_speed lane, mouse-proportional — the key servo stays
        // out via `mc1_input`'s bit strip).
        if let Some(v) = input.mc2_cmd_speed {
            self.carpet.tgt_speed = v;
        }
    }

    /// The ExtendedLift deviation's MC2 tail, after the in-walk move
    /// (both control modes).
    fn mc2_walk_lift(&mut self, input: &FlightInput, falling: bool, dead: bool) {
        if self.altitude_model != AltitudeModel::ExtendedLift
            || self.carpet_mc2.mobilize != 0
            || falling
            || dead
        {
            return;
        }
        let (cx, cy) = (self.carpet.x, self.carpet.y);
        let row = self.carpet_mc2.row;
        let Some(w) = self.world.as_ref() else { return };
        let g = w.ground_z_engine(cx, cy);
        let sink = row.buoyancy.unsigned_abs() as i16;
        let (hi, cap) = self.lift_caps(g, row.band);
        self.carpet.z = lift_desired_law(
            self.carpet.z,
            g,
            &mut self.lift_desired,
            input.lift,
            row.clearance,
            hi,
            cap,
            sink,
            sink,
        );
        let w = self.world.as_ref().expect("checked above");
        if let Some(c) = w.player_cave_ceiling(cx, cy) {
            let c = c.max(g.saturating_add(row.clearance));
            if self.carpet.z > c {
                self.carpet.z = c;
            }
        }
    }

    /// The enhanced controls on a WORLD-LESS sim (unit tests, the
    /// terrain-only harness): hold-to-fly with automatic deceleration,
    /// a deliberate deviation (see [`ThrustModel`]), with nothing to
    /// interact with. Thrust acts in the yaw ground plane; vertical
    /// belongs to the altitude axis (the MC1-shaped float climb law
    /// under [`AltitudeModel::Faithful`], the lift keys under
    /// ExtendedLift).
    ///
    /// ⚠⚠ On a live world the enhanced controls are [`EnhancedKernel`]
    /// inside the ONE shared walk-slot dispatch. This function must
    /// never grow a world interaction again: its old world arms were a
    /// second, hand-kept copy of the dispatch, and every drift between
    /// the two was a player bug (the web latch, the duel leash and
    /// grip, the cave-wall Speed cancel, the gravity-well pull, the
    /// whirl spin, the flutter — parity audit 2026-09-25).
    fn move_enhanced(&mut self, input: &FlightInput) {
        debug_assert!(self.world.is_none(), "a live world flies the kernel");
        // Chase-the-pointer steering (player design 2026-07-23), as in
        // the kernel.
        self.aim_lead = (self.aim_lead + input.yaw_delta).clamp(-LEAD_MAX, LEAD_MAX);
        let step = if self.aim_lead.abs() < 1e-4 {
            self.aim_lead // snap the tail closed (floats never reach 0)
        } else {
            (self.aim_lead * CHASE_GAIN * TICK_DT)
                .clamp(-TURN_RATE_MAX * TICK_DT, TURN_RATE_MAX * TICK_DT)
        };
        self.aim_lead -= step;
        self.turn_rate = step / TICK_DT;

        let f = &mut self.flyer;
        f.yaw += step;
        f.pitch = (f.pitch + input.pitch_delta).clamp(-MAX_PITCH, MAX_PITCH);
        let (sy, cy) = f.yaw.sin_cos();
        let fwd = [sy, 0.0, -cy];
        let right = [cy, 0.0, sy];
        f.vx += (fwd[0] * input.thrust + right[0] * input.strafe) * ACCEL * TICK_DT;
        f.vz += (fwd[2] * input.thrust + right[2] * input.strafe) * ACCEL * TICK_DT;
        f.vx *= DRAG_PER_TICK;
        f.vy *= DRAG_PER_TICK;
        f.vz *= DRAG_PER_TICK;
        let from_y = f.y;
        f.x = (f.x + f.vx * TICK_DT).rem_euclid(MAP_TILES as f32);
        f.z = (f.z + f.vz * TICK_DT).rem_euclid(MAP_TILES as f32);
        f.y += f.vy * TICK_DT;

        let g = self.ground_height(self.flyer.x, self.flyer.z);
        if self.altitude_model == AltitudeModel::Faithful {
            // MC1's float climb law: a dive passes the raw aim, a climb
            // scales by the authority band (full at ground, zero at the
            // 4-tile soft ceiling, inverted above), plus the at-rest
            // 8/tick sink above the band.
            let f = &mut self.flyer;
            let s = (f.vx * fwd[0] + f.vz * fwd[2]) * TICK_DT;
            if s != 0.0 && f.pitch != 0.0 {
                let dive = (s > 0.0 && f.pitch < 0.0) || (s < 0.0 && f.pitch > 0.0);
                let eff = if dive {
                    f.pitch
                } else {
                    f.pitch * -(f.y - g - 4.0).clamp(-1.0, 1.0)
                };
                f.y += s * eff.sin();
            }
            let speed = (f.vx * f.vx + f.vz * f.vz).sqrt();
            if speed < 0.05 && f.y > g + 4.0 {
                f.y -= 8.0 / 256.0;
            }
        }
        {
            let f = &mut self.flyer;
            let floor = g + MIN_CLEARANCE;
            if f.y < floor {
                f.y = floor;
                f.vy = f.vy.max(0.0);
            }
        }
        if self.altitude_model == AltitudeModel::ExtendedLift {
            let clr = (MIN_CLEARANCE * 256.0) as i16;
            let g_e = ((g * 256.0) as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let z_e = ((self.flyer.y * 256.0).round() as i32)
                .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let (hi, cap) = self.lift_caps(g_e, LIFT_BAND_MC1);
            let nz = lift_desired_law(
                z_e,
                g_e,
                &mut self.lift_desired,
                input.lift,
                clr,
                hi,
                cap,
                0,
                LIFT_DRIFT_MC1,
            );
            self.flyer.y = nz as f32 / 256.0;
        }
        // The global lift ceiling only stops further RISING — it never
        // pulls down altitude already held.
        let ceiling = self.lift_ceiling();
        let f = &mut self.flyer;
        if f.y > ceiling && f.y > from_y {
            f.y = from_y.max(ceiling);
            f.vy = f.vy.min(0.0);
        }
        // The proportional bank (ruling 5 + the 2026-07-27 forward-
        // projection amendment).
        let fwd_sp = f.vx * fwd[0] + f.vz * fwd[2];
        f.roll = enhanced_bank(self.turn_rate, fwd_sp);
    }

    /// The enhanced-altitude band + cap for one arm: `(hi, cap_abs)` —
    /// the desired-offset ceiling and the absolute altitude cap. The
    /// debug unclamp swaps the ground-relative band for the global
    /// lift ceiling.
    fn lift_caps(&self, g: i16, band: i16) -> (i16, i16) {
        if self.weightless {
            let c = ((self.lift_ceiling() * 256.0) as i32).min(i16::MAX as i32) as i16;
            (i16::MAX / 2, c)
        } else {
            (band, g.saturating_add(band))
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

/// The enhanced controls as the movers' propulsion kernel
/// ([`flight::CarpetPropel`], both games): chase-the-pointer steering, float thrust
/// with drag, the Accelerate override — deliberate deviations, all of
/// them input→motion. Everything the WORLD does to the carpet (webs,
/// knock, whirl, hurl, the gravity well, the duel leash, the commit
/// gate, the vertical resolution, teleports, the death arms) runs in
/// the one shared dispatch around it, on the integer carpet.
///
/// The float flyer keeps the sub-engine-unit remainder and the
/// velocity; the integer carpet is the authority the world sees. The
/// flyer tracks every carpet write it did not make itself through
/// [`KernelRefs`].
struct EnhancedKernel<'a> {
    flyer: &'a mut Flyer,
    aim_lead: &'a mut f32,
    turn_rate: &'a mut f32,
    input: FlightInput,
    over: Option<f32>,
    /// ⚖ The enhanced-altitude split: under the faithful altitude
    /// model pitch climbs through the game's retail ramp; under
    /// ExtendedLift pitch is pure look and q/e own the vertical (the
    /// post-walk lift law).
    pitch_climbs: bool,
    refs: KernelRefs,
}

/// The integer carpet the flyer last matched — any difference is a
/// write by the shared dispatch, which the flyer adopts.
#[derive(Clone, Copy)]
struct KernelRefs {
    pos: (u16, u16, i16),
    yaw: u16,
    /// (target, actual) speed as the kernel last published them.
    speed: (i16, i16),
}

impl KernelRefs {
    fn of(c: &flight::Mc1State) -> Self {
        KernelRefs {
            pos: (c.x, c.y, c.z),
            yaw: c.yaw,
            speed: (c.tgt_speed, c.act_speed),
        }
    }

    /// Move the flyer by the carpet's change since the refs, keeping
    /// its sub-unit remainder; an outside write of speed ZERO (the
    /// teleport stop, the cave-wall dead stop) kills the momentum.
    fn follow(&self, f: &mut Flyer, c: &flight::Mc1State) {
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        let d16 = |now: u16, was: u16| now.wrapping_sub(was) as i16 as f32 / 256.0;
        f.x = (f.x + d16(c.x, self.pos.0)).rem_euclid(MAP_TILES as f32);
        f.z = (f.z + d16(c.y, self.pos.1)).rem_euclid(MAP_TILES as f32);
        f.y += (c.z as i32 - self.pos.2 as i32) as f32 / 256.0;
        let mut dyaw = (c.yaw as i32 - self.yaw as i32) & 0x7FF;
        if dyaw > 1024 {
            dyaw -= 2048;
        }
        f.yaw += dyaw as f32 * RAD;
        if c.tgt_speed == 0 && self.speed.0 != 0 {
            f.vx = 0.0;
            f.vz = 0.0;
        }
    }
}

impl flight::CarpetPropel for EnhancedKernel<'_> {
    fn propel(
        &mut self,
        st: &mut flight::Mc1State,
        web: (u8, u8),
        seized_pitch: bool,
        climb: &dyn Fn(&mut flight::Mc1State, i16) -> i16,
    ) -> (u16, u16, i16) {
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        let input = self.input;
        // Adopt the dispatch's writes so far (teleport, hurl, well,
        // whirl seizure, spin, the whirl crank's residual turn).
        self.refs.follow(self.flyer, st);
        let f = &mut *self.flyer;
        if st.act_speed != self.refs.speed.1 {
            // An outside SPEED write (the whirlwind's act 80): the
            // momentum becomes that speed along the heading.
            let v = st.act_speed as f32 / 256.0 / TICK_DT;
            let (sy, cy) = f.yaw.sin_cos();
            f.vx = sy * v;
            f.vz = -cy * v;
        }
        if seized_pitch {
            // The gravity well's pitch seizure owns the look this tick.
            f.pitch = (-(st.aim_signed() as f32) * RAD).clamp(-MAX_PITCH, MAX_PITCH);
        }

        // Chase-the-pointer steering (deliberate deviation — player
        // design 2026-07-23): mouse motion moves the DESIRED heading;
        // the carpet turns toward it at rate ∝ remaining lead, capped.
        *self.aim_lead = (*self.aim_lead + input.yaw_delta).clamp(-LEAD_MAX, LEAD_MAX);
        let step = if self.aim_lead.abs() < 1e-4 {
            *self.aim_lead
        } else {
            (*self.aim_lead * CHASE_GAIN * TICK_DT)
                .clamp(-TURN_RATE_MAX * TICK_DT, TURN_RATE_MAX * TICK_DT)
        };
        *self.aim_lead -= step;
        *self.turn_rate = step / TICK_DT;
        f.yaw += step;
        f.pitch = (f.pitch + input.pitch_delta).clamp(-MAX_PITCH, MAX_PITCH);

        // Float thrust + drag in the yaw ground plane; the Accelerate
        // override replaces the thrust (strafe stays live).
        let (sy, cy) = f.yaw.sin_cos();
        let fwd = [sy, -cy];
        let right = [cy, sy];
        let thrust = if self.over.is_some() { 0.0 } else { input.thrust };
        f.vx += (fwd[0] * thrust + right[0] * input.strafe) * ACCEL * TICK_DT;
        f.vz += (fwd[1] * thrust + right[1] * input.strafe) * ACCEL * TICK_DT;
        f.vx *= DRAG_PER_TICK;
        f.vz *= DRAG_PER_TICK;
        f.vy = 0.0;
        if let Some(k) = self.over {
            let vmax = ACCEL * TICK_DT * DRAG_PER_TICK / (1.0 - DRAG_PER_TICK);
            f.vx += (fwd[0] * k * vmax - f.vx) * 0.5;
            f.vz += (fwd[1] * k * vmax - f.vz) * 0.5;
        }

        // The web: the slow level scales the step, the paralyze
        // freezes it (retail's block-3/4 scaling, float form).
        let (slow, paralyze) = web;
        let web = if paralyze > 0 {
            0.0
        } else {
            (4 - slow) as f32 / 4.0
        };
        f.x = (f.x + f.vx * TICK_DT * web).rem_euclid(MAP_TILES as f32);
        f.z = (f.z + f.vz * TICK_DT * web).rem_euclid(MAP_TILES as f32);

        // Publish the pose and speed the world reads.
        st.yaw = ((f.yaw / RAD).round() as i64).rem_euclid(2048) as u16;
        let aim = (-f.pitch / RAD).round() as i32;
        st.pitch_f = aim as i16;
        st.aim_pitch = (aim & 0x7FF) as u16;
        // The published speed is the SIGNED forward component along
        // the hull (the retail-analog carpet speed a cast inherits and
        // MC2's Speed reads its direction from); the web only scales
        // the step, as retail's does.
        let fwd_sp = f.vx * fwd[0] + f.vz * fwd[1];
        let s = (fwd_sp * TICK_DT * 256.0).round().clamp(-32768.0, 32767.0) as i16;
        let s_step = (fwd_sp * web * TICK_DT * 256.0).round().clamp(-32768.0, 32767.0) as i16;
        st.act_speed = s;
        st.tgt_speed = s;
        st.strafe = 0;

        // Vertical: the game's retail climb law on the enhanced speed
        // (the faithful altitude model only).
        let z = if self.pitch_climbs { climb(st, s_step) } else { st.z };
        f.roll = enhanced_bank(*self.turn_rate, fwd_sp);
        let q = |t: f32| ((t * 256.0).floor() as i64).rem_euclid(65536) as u16;
        let cand = (q(f.x), q(f.z), z);
        f.y = z as f32 / 256.0;
        self.refs = KernelRefs {
            pos: cand,
            yaw: st.yaw,
            speed: (s, s),
        };
        cand
    }

    fn aim_heading_offset(&self) -> i16 {
        const RAD: f32 = std::f32::consts::TAU / 2048.0;
        (*self.aim_lead / RAD).round() as i16
    }
}

/// One tick of the enhanced-altitude desired-altitude law, in engine
/// units — shared by all three mover arms (deliberate deviation, see
/// docs/DEVIATIONS.md "enhanced flight"; player rulings 1/4/6 of
/// 2026-07-22).
///
/// `z` — the mover's committed altitude this tick; `g` — ground under
/// the carpet; `desired` — the pinned ground-relative offset (updated
/// in place); `lift` — the q/e axis; `lo..hi` — the offset band;
/// `cap_abs` — the absolute climb cap (band top, or the global lift
/// ceiling under the debug unclamp); `sink` — the passive sink the
/// faithful mover ALREADY applied this tick (the MC2 row buoyancy),
/// compensated out so the NET rates are uniform across games; `net` —
/// the net drift rate = the game's standard passive descent.
///
/// The unified q/e rule (ruling 4): the key always steps the CARPET
/// at the classic full-pitch climb rate; `desired` follows via
/// max/min, so a toward-key while lagging is a pure boost (desired
/// untouched) and an away-key (or pushing past) drags desired along.
fn lift_desired_law(
    z: i16,
    g: i16,
    desired: &mut i16,
    lift: f32,
    lo: i16,
    hi: i16,
    cap_abs: i16,
    sink: i16,
    net: i16,
) -> i16 {
    let (z, g) = (z as i32, g as i32);
    let (lo, hi, cap) = (lo as i32, hi as i32, cap_abs as i32);
    let (sink, net) = (sink as i32, net as i32);
    let floor = (g + lo).min(cap);
    let mut d = (*desired as i32).clamp(lo, hi);
    let mut nz = z;
    if lift > 0.0 {
        // Climb at the q/e rate (+ the sink already taken out). Never
        // yanked DOWN from altitude already held above the cap
        // (wall-climb gains stay legitimate, like classic lift did).
        nz = (z + LIFT_QE_STEP as i32 + sink).min(cap.max(z));
        d = d.max((nz - g).min(hi));
    } else if lift < 0.0 {
        nz = (z - (LIFT_QE_STEP as i32 - sink).max(0)).max(floor.min(z));
        d = d.min((nz - g).max(lo));
    } else {
        // Drift toward ground+desired at the standard descent speed:
        // climbing compensates the mover's own sink, descending RIDES
        // it (net = the game's passive descent either way).
        let target = (g + d).min(cap);
        if z < target {
            nz = (z + net + sink).min(target);
        } else if z > target {
            nz = (z - (net - sink).max(0)).max(target);
        }
    }
    *desired = d as i16;
    nz.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal living world over flat height-100 terrain (the
    /// mortality boundary tests need World state, not just planes).
    fn flat_world(height: Vec<u8>) -> world::World {
        use crate::engine::features::Planes;
        let planes = Planes {
            height,
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        world::World::new(planes, &[], 7, test_assets())
    }

    /// A bare flat MC2 world (no THINGs, so no start marker and no
    /// pinned human record — the funnel's `!human_law` arm).
    fn mc2_flat_world(height: u8) -> world::World {
        use crate::engine::features::Planes;
        let planes = Planes {
            height: vec![height; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        world::World::new_for_game(planes, &[], 7, test_assets(), crate::ids::GameId::Mc2)
    }

    fn test_assets() -> crate::engine::features::FeatureAssets {
        use crate::engine::features::FeatureAssets;
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
        FeatureAssets::parse(&grid, &tab, &dat).unwrap()
    }

    /// **THE ENHANCED MOVER IS SWEPT AND TURNED BY A WHIRLWIND.** The
    /// funnel's human arm rides `Gen::player_whirl` (a POSE seizure —
    /// heading + step on the mid ring, an absolute pose on the grab
    /// family), which the faithful walk drains at the carpet's own
    /// walk slot. The enhanced mover has no walk slot; it drains the
    /// channel itself, or the tornado arms a seizure every tick that
    /// nobody applies (player report 2026-09-10, level-001's arm
    /// tornado: "barely moves the flyer and does not rotate it").
    ///
    /// Non-vacuity: with the drain removed from `move_enhanced` the
    /// flyer here sits still at yaw 0 for the whole run.
    #[test]
    fn enhanced_mover_is_swept_and_turned_by_a_whirlwind() {
        let mut w = mc2_flat_world(100);
        let (wx, wy) = ((100u16 << 8) | 128, (100u16 << 8) | 128);
        let gz = w.ground_z_engine(wx, wy);
        let t = w.g.mc2_spawn_whirlwind(wx, wy, gz).expect("whirlwind");
        w.g.ent[t].id24 = 7; // someone ELSE's funnel (`sub_33810` case 1)
        let mut sim = Simulation::with_world(w);
        sim.thrust_model = ThrustModel::Enhanced;
        sim.altitude_model = AltitudeModel::ExtendedLift;
        // Four tiles east of the eye = the MID RING, facing north, no
        // input at all: every unit of motion below is the funnel's.
        sim.flyer.x = 104.5;
        sim.flyer.z = 100.5;
        sim.flyer.y = 100.0 / 8.0 + 2.0;
        sim.flyer.yaw = 0.0;
        sim.sync_carpet_from_flyer();
        let (x0, z0) = (sim.flyer.x, sim.flyer.z);
        // Tick 1 arms the channel at the funnel's slot; tick 2's move
        // drains it.
        for _ in 0..2 {
            sim.step(&FlightInput::default());
        }
        assert!(
            sim.flyer.yaw.abs() > 0.5,
            "the mid ring seizes the heading (tangent + 591): yaw {}",
            sim.flyer.yaw
        );
        let moved = ((sim.flyer.x - x0).powi(2) + (sim.flyer.z - z0).powi(2)).sqrt();
        assert!(
            moved > 0.3,
            "the 96-unit step landed on the float flyer: moved {moved} tiles"
        );
        // …and `actSpeed = 80` re-aims the cruise along the seized
        // heading, so the sweep keeps its retail violence: the next
        // ticks each cover the step PLUS the cruise.
        let (x1, z1) = (sim.flyer.x, sim.flyer.z);
        sim.step(&FlightInput::default());
        let per_tick = ((sim.flyer.x - x1).powi(2) + (sim.flyer.z - z1).powi(2)).sqrt();
        assert!(
            per_tick > (96.0 + 40.0) / 256.0,
            "step + cruise per tick, not the step alone: {per_tick} tiles"
        );
    }

    /// **THE SPEED MAIL DOES NOT OUTLIVE ITS TICK ON THE ENHANCED
    /// TIER.** The Speed token mails its carpet-speed write through
    /// `pending_speed_base`, and the MC2 cast thunks read an undelivered
    /// mail as the caster's live `actSpeed`. Only the faithful tier used
    /// to take it, so on the enhanced tier the last write stood for the
    /// rest of the session: a burst cancelled while reversing at Speed
    /// III left −400 behind and every later possession bolt launched at
    /// 384 − 400 (player report 2026-09-19).
    ///
    /// Non-vacuity: with the driver's take gated back to the faithful
    /// tier the mail is still `Some(-400)` after the step.
    #[test]
    fn enhanced_tier_drains_the_speed_mail() {
        let w = mc2_flat_world(100);
        let mut sim = Simulation::with_world(w);
        sim.thrust_model = ThrustModel::Enhanced;
        sim.flyer.x = 100.5;
        sim.flyer.z = 100.5;
        sim.flyer.y = 100.0 / 8.0 + 2.0;
        sim.sync_carpet_from_flyer();
        sim.world.as_mut().unwrap().pending_speed_base = Some(-400);
        sim.step(&FlightInput::default());
        let w = sim.world.as_ref().unwrap();
        assert_eq!(w.pending_speed_base, None, "the stale mail is drained");
        let p = world::PlayerPose::from_tiles(100.5, 14.5, 100.5, 0.0, 0.0, 0.0);
        assert_eq!(
            w.mc2_caster_act_speed(p),
            0,
            "a standstill caster casts at its own speed, not the dead burst's"
        );
    }

    /// The `human_law` column (a pinned human record: the app's own
    /// shape) publishes an ABSOLUTE pose on every visit — including a
    /// far-band visit that displaces nothing. Consumed one tick late,
    /// that pose must land as a delta: a flyer holding thrust nine
    /// tiles from the eye (inside the 12-tile ring, outside the
    /// 0x310000 band) keeps flying, and one four tiles out is swept
    /// AND turned.
    ///
    /// Non-vacuity: stamping `grab` as an absolute pose here freezes
    /// the far flyer at its first in-ring position (the level-001
    /// probe under the enhanced model sat at (9.6, 193.8) for seven
    /// ticks with 6 tiles/s of velocity).
    #[test]
    fn enhanced_mover_whirl_seizure_is_a_delta_not_a_rewind() {
        let rig = |tiles_out: f32| -> Simulation {
            let mut w = mc2_flat_world(100);
            let (wx, wy) = ((100u16 << 8) | 128, (100u16 << 8) | 128);
            let gz = w.ground_z_engine(wx, wy);
            let t = w.g.mc2_spawn_whirlwind(wx, wy, gz).expect("whirlwind");
            w.g.ent[t].id24 = 7;
            // The app's shape: a pinned class-0 human record.
            let pinned = w.g.new_event().expect("pinned seat");
            w.g.ent[pinned] = crate::engine::features::Ent::default();
            w.mc2_carpet_slot = pinned as u16;
            w.g.mc2_pinned = crate::engine::features::Mc2Pinned(pinned as u16);
            let mut sim = Simulation::with_world(w);
            sim.thrust_model = ThrustModel::Enhanced;
            sim.altitude_model = AltitudeModel::ExtendedLift;
            sim.flyer.x = 100.5 + tiles_out;
            sim.flyer.z = 100.5;
            sim.flyer.y = 100.0 / 8.0 + 2.0;
            sim.flyer.yaw = 0.0;
            sim.sync_carpet_from_flyer();
            sim
        };
        let thrust = FlightInput {
            thrust: 1.0,
            ..FlightInput::default()
        };
        // Far band: the tail visit publishes his own pose back. He
        // must keep making way north under thrust.
        let mut sim = rig(9.0);
        let z0 = sim.flyer.z;
        for _ in 0..12 {
            sim.step(&thrust);
        }
        assert!(
            z0 - sim.flyer.z > 1.0,
            "a far-band visit is not a rewind: advanced {} tiles",
            z0 - sim.flyer.z
        );
        assert!(
            sim.flyer.yaw.abs() < 1e-3,
            "the far band writes no heading: {}",
            sim.flyer.yaw
        );
        // Mid ring under the same column: swept and turned.
        let mut sim = rig(4.0);
        let (x0, z0) = (sim.flyer.x, sim.flyer.z);
        for _ in 0..3 {
            sim.step(&FlightInput::default());
        }
        assert!(
            sim.flyer.yaw.abs() > 0.5,
            "the mid ring seizes the heading: {}",
            sim.flyer.yaw
        );
        let moved = ((sim.flyer.x - x0).powi(2) + (sim.flyer.z - z0).powi(2)).sqrt();
        assert!(moved > 0.5, "the mid ring sweeps: moved {moved} tiles");
    }

    /// The retail suicide key (Shift+K, MC1 :20492-93 / MC2
    /// PlayerInput.cpp:239-41): the bare `life = -1` write enters the
    /// ordinary death chain with NO killer latched, and it bypasses
    /// invincibility (retail's write never enters the damage head).
    #[test]
    fn suicide_key_kills_with_no_killer_even_invincible() {
        let mut sim = Simulation::with_world(flat_world(vec![100; 0x10000]));
        sim.flyer.x = 112.5;
        sim.flyer.z = 116.5;
        sim.flyer.y = 100.0 / 8.0 + 3.0;
        sim.sync_carpet_from_flyer();
        sim.world.as_mut().unwrap().set_invincible(true);
        let input = FlightInput {
            suicide: true,
            ..Default::default()
        };
        sim.step(&input);
        {
            // The key pass is POST-walk (:20488-93): the write tick's
            // own dispatch ran the full live wizard tick (mc1l0-pd
            // t=2644 keeps f70 = 0) and only the bare -1 lands.
            let w = sim.world.as_ref().unwrap();
            assert_eq!(
                w.vitals().state,
                world::LifeState::Alive,
                "the write tick itself stays action 0"
            );
        }
        sim.step(&FlightInput::default());
        {
            // The NEXT tick's regen tail runs the death arm
            // (:55424-29): flip to the fall despite invincibility.
            let w = sim.world.as_ref().unwrap();
            assert_ne!(
                w.vitals().state,
                world::LifeState::Alive,
                "the death arm must flip the tick after the write"
            );
        }
        for _ in 0..200 {
            sim.step(&FlightInput::default());
            if sim.world.as_ref().unwrap().player_dead() {
                return;
            }
        }
        panic!("the suicide never landed");
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

    /// A DEAD wizard is pinned at the grave. Under the enhanced mover
    /// the camera rides the float velocity (`flyer.v*`), NOT the carpet
    /// speeds — so if only the carpet is zeroed the viewport keeps
    /// sliding along the terrain after touchdown. Once dead, the flyer
    /// must not drift.
    #[test]
    fn dead_wizard_pins_at_the_grave_under_enhanced() {
        let mut sim = Simulation::with_world(flat_world(vec![80; MAP_TILES * MAP_TILES]));
        sim.thrust_model = ThrustModel::Enhanced;
        sim.altitude_model = AltitudeModel::ExtendedLift;
        sim.flyer.x = 128.0;
        sim.flyer.z = 128.0;
        sim.flyer.y = 80.0 / 8.0 + 3.0;
        sim.sync_carpet_from_flyer();
        // Fly forward so the flyer carries real horizontal momentum.
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..20 {
            sim.step(&fwd);
        }
        assert!(
            sim.flyer.vx.abs() + sim.flyer.vz.abs() > 0.1,
            "the wizard is moving before the mortal blow"
        );
        sim.world.as_mut().unwrap().debug_kill_player();
        for _ in 0..300 {
            sim.step(&FlightInput::default());
            if sim.world.as_ref().unwrap().player_dead() {
                break;
            }
        }
        assert!(
            sim.world.as_ref().unwrap().player_dead(),
            "the corpse landed"
        );
        let (x0, z0) = (sim.flyer.x, sim.flyer.z);
        for _ in 0..60 {
            sim.step(&FlightInput::default());
        }
        let drift = (sim.flyer.x - x0).abs() + (sim.flyer.z - z0).abs();
        assert!(
            drift < 1e-4,
            "the dead wizard slid {drift} tiles off the grave"
        );
        assert_eq!(sim.flyer.vx, 0.0, "float velocity is pinned");
        assert_eq!(sim.flyer.vz, 0.0, "float velocity is pinned");
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

    // ---- the enhanced-flight enhancements (2026-07-22 rulings):
    // turn-rate damper, proportional bank, desired-altitude law ----

    /// A world-less MC1-arm sim on flat ground with the desired
    /// altitude seeded at `alt` tiles.
    fn lift_sim(alt: f32) -> Simulation {
        let mut sim = Simulation::with_terrain(vec![0u8; MAP_TILES * MAP_TILES]);
        sim.altitude_model = AltitudeModel::ExtendedLift;
        sim.flyer.y = alt;
        sim.sync_carpet_from_flyer();
        sim
    }

    #[test]
    fn desired_altitude_qe_steps_fast_and_pins() {
        let mut sim = lift_sim(1.0);
        assert_eq!(sim.lift_desired, 256, "seeded from the spawn pose");
        let up = FlightInput {
            lift: 1.0,
            ..Default::default()
        };
        for _ in 0..4 {
            sim.step(&up);
        }
        // The classic full-pitch climb rate, 56/tick.
        assert_eq!(sim.carpet.z, 256 + 4 * 56);
        assert_eq!(sim.lift_desired, 256 + 4 * 56, "desired climbs with you");
        // Release: PINNED — the old idle settle-to-floor is gone.
        for _ in 0..50 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.carpet.z, 256 + 4 * 56, "holds the pinned altitude");
    }

    #[test]
    fn desired_altitude_caps_ground_relative() {
        let mut sim = lift_sim(1.0);
        let up = FlightInput {
            lift: 1.0,
            ..Default::default()
        };
        for _ in 0..100 {
            sim.step(&up);
        }
        // Ruling 1: the cap is GROUND-RELATIVE — 1024 over terrain,
        // not the level's highest tile.
        assert_eq!(sim.carpet.z, 1024, "band top over flat ground");
        assert_eq!(sim.lift_desired, 1024);
    }

    #[test]
    fn desired_altitude_unclamped_debug_reaches_global_ceiling() {
        let mut th = vec![0u8; MAP_TILES * MAP_TILES];
        th[0] = 80; // a lone 10-tile peak far away
        let mut sim = Simulation::with_terrain(th);
        sim.altitude_model = AltitudeModel::ExtendedLift;
        sim.weightless = true;
        sim.flyer.y = 1.0;
        sim.sync_carpet_from_flyer();
        let up = FlightInput {
            lift: 1.0,
            ..Default::default()
        };
        for _ in 0..200 {
            sim.step(&up);
        }
        // The debug toggle restores the old absolute cap: highest
        // terrain (10 tiles) + the soft-ceiling band (4).
        assert_eq!(sim.carpet.z, 14 * 256, "global ceiling, z={}", sim.carpet.z);
    }

    #[test]
    fn desired_altitude_toward_key_boosts_away_key_sets() {
        // Flung below desired (ruling 4's unified q/e rule).
        let mut sim = lift_sim(3.0);
        assert_eq!(sim.lift_desired, 768);
        sim.carpet.z = 300;
        sim.step(&FlightInput {
            lift: 1.0,
            ..Default::default()
        });
        assert_eq!(sim.carpet.z, 356, "the toward-key boosts at 56/tick");
        assert_eq!(sim.lift_desired, 768, "…without moving desired");
        sim.step(&FlightInput {
            lift: -1.0,
            ..Default::default()
        });
        assert_eq!(sim.carpet.z, 300);
        assert_eq!(sim.lift_desired, 300, "the away-key SETS desired");
    }

    #[test]
    fn desired_altitude_recovers_from_flings_at_drift_rate() {
        // Ruling 6: desired is PINNED; the flyer recovers from ANY
        // displacement at the standard 8/tick drift.
        let mut sim = lift_sim(2.0);
        sim.carpet.z = 900; // flung up
        sim.step(&FlightInput::default());
        assert_eq!(sim.carpet.z, 892, "drifts down 8/tick");
        for _ in 0..100 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.carpet.z, 512, "parks at the pinned desired");
        sim.carpet.z = 200; // flung down
        sim.step(&FlightInput::default());
        assert_eq!(sim.carpet.z, 208, "drifts back up 8/tick");
    }

    #[test]
    fn desired_altitude_follows_terrain_offset() {
        // Ruling 1: the target is ground+desired, so terrain rising
        // underneath carries the whole band with it.
        let mut th = vec![0u8; MAP_TILES * MAP_TILES];
        for (i, h) in th.iter_mut().enumerate() {
            if i % MAP_TILES >= 100 {
                *h = 16; // a 2-tile plateau east of x=100
            }
        }
        let mut sim = Simulation::with_terrain(th);
        sim.altitude_model = AltitudeModel::ExtendedLift;
        sim.flyer.x = 50.0;
        sim.flyer.z = 128.0;
        sim.flyer.y = 2.0;
        sim.sync_carpet_from_flyer();
        assert_eq!(sim.lift_desired, 512);
        // Warp over the plateau (a fling analog): the floor bumps the
        // carpet first, then the drift restores the 512 offset OVER
        // the new ground.
        sim.carpet.x = 110 * 256;
        for _ in 0..200 {
            sim.step(&FlightInput::default());
        }
        assert_eq!(sim.carpet.z, 512 + 512, "offset held over the plateau");
    }

    #[test]
    fn desired_altitude_under_enhanced_thrust() {
        let mut sim = Simulation::with_terrain(vec![0u8; MAP_TILES * MAP_TILES]);
        sim.thrust_model = ThrustModel::Enhanced;
        sim.altitude_model = AltitudeModel::ExtendedLift;
        sim.flyer.y = 1.0;
        sim.sync_carpet_from_flyer();
        let up = FlightInput {
            lift: 1.0,
            ..Default::default()
        };
        for _ in 0..100 {
            sim.step(&up);
        }
        assert!(
            (sim.flyer.y - 4.0).abs() < 0.01,
            "ground-relative cap on the float arm too, y={}",
            sim.flyer.y
        );
        // Pinned on release — the old settle-to-floor is gone.
        for _ in 0..100 {
            sim.step(&FlightInput::default());
        }
        assert!(
            (sim.flyer.y - 4.0).abs() < 0.01,
            "pinned, y={}",
            sim.flyer.y
        );
        // q descends fast and drags desired with it; release holds.
        let down = FlightInput {
            lift: -1.0,
            ..Default::default()
        };
        for _ in 0..5 {
            sim.step(&down);
        }
        let held = sim.flyer.y;
        assert!(held < 3.0, "q descends at the fast rate, y={held}");
        for _ in 0..50 {
            sim.step(&FlightInput::default());
        }
        assert!(
            (sim.flyer.y - held).abs() < 0.01,
            "desired followed q down, y={}",
            sim.flyer.y
        );
    }

    #[test]
    fn chase_steering_converges_on_the_pointer() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        // One swipe pins a desired heading 0.4 rad right of the nose.
        sim.step(&FlightInput {
            yaw_delta: 0.4,
            ..Default::default()
        });
        let first = sim.flyer.yaw;
        assert!(first > 0.0, "starts turning immediately");
        assert!(
            first < 0.4,
            "…but gradually, not head-snap: first step {first}"
        );
        // The desired WORLD heading is pinned: yaw + lead is invariant
        // while the carpet converges with no further mouse motion.
        let desired = sim.aim_yaw();
        assert!((desired - 0.4).abs() < 1e-5, "desired = the swipe");
        let mut prev_step = f32::MAX;
        let mut prev_yaw = sim.flyer.yaw;
        for _ in 0..60 {
            sim.step(&FlightInput::default());
            let step = sim.flyer.yaw - prev_yaw;
            prev_yaw = sim.flyer.yaw;
            assert!(step >= 0.0, "never overshoots into a wobble");
            // Monotone ease-out — except the final snap that closes
            // the sub-1e-4 tail in one (imperceptible) step.
            assert!(
                step <= prev_step + 1e-6 || step < 2e-4,
                "eases out (rate ∝ error)"
            );
            prev_step = step.max(1e-9);
            assert!(
                (sim.aim_yaw() - desired).abs() < 1e-4,
                "the pointer stays world-pinned while chasing"
            );
        }
        assert!(
            (sim.flyer.yaw - 0.4).abs() < 1e-3,
            "arrives at the pointer, yaw={}",
            sim.flyer.yaw
        );
        let settled = sim.flyer.yaw;
        sim.step(&FlightInput::default());
        assert_eq!(sim.flyer.yaw, settled, "dead stop once closed");
    }

    #[test]
    fn chase_lead_clamps_on_screen_and_rate_caps() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        // Wild continuous swipes: the lead clamps at LEAD_MAX (the
        // crosshair stays on screen) and the per-tick turn never
        // exceeds the cap (≈ the classic model's own max turn rate).
        let swipe = FlightInput {
            yaw_delta: 3.0,
            ..Default::default()
        };
        let mut prev = 0.0f32;
        for _ in 0..20 {
            sim.step(&swipe);
            let d = sim.flyer.yaw - prev;
            prev = sim.flyer.yaw;
            assert!(d <= TURN_RATE_MAX * TICK_DT + 1e-5, "rate capped, d={d}");
            assert!(
                sim.aim_yaw() - sim.flyer.yaw <= LEAD_MAX + 1e-5,
                "lead clamped on screen"
            );
        }
    }

    #[test]
    fn bank_needs_motion_and_levels_out() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        // Turning in place: NO bank (ruling 5 — the departure from
        // retail's fixed velocity-independent bank).
        let turn = FlightInput {
            yaw_delta: 0.2,
            ..Default::default()
        };
        for _ in 0..10 {
            sim.step(&turn);
        }
        assert!(
            sim.flyer.roll.abs() < 0.02,
            "no bank standing, roll={}",
            sim.flyer.roll
        );
        // At cruise, a right turn banks right (positive roll).
        let fwd_turn = FlightInput {
            thrust: 1.0,
            yaw_delta: 0.2,
            ..Default::default()
        };
        for _ in 0..40 {
            sim.step(&fwd_turn);
        }
        assert!(
            sim.flyer.roll > 0.1,
            "banks into a moving right turn, roll={}",
            sim.flyer.roll
        );
        assert!(sim.flyer.roll <= BANK_MAX + 1e-6);
        // Turn released: the bank levels out with the rate.
        let fwd = FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..80 {
            sim.step(&fwd);
        }
        assert!(
            sim.flyer.roll.abs() < 0.02,
            "levels out, roll={}",
            sim.flyer.roll
        );
    }

    #[test]
    fn strafe_does_not_cancel_the_forward_bank() {
        // Player ruling 2026-07-27: banking follows the forward/backward
        // velocity vector, ignoring the left/right drift — pressing a
        // strafe key mid-turn must NOT snap the view level (the old
        // per-strafe gate did, which read as a jarring straighten-and-
        // re-tilt).
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        // Establish a forward, banked right turn.
        let fwd_turn = FlightInput {
            thrust: 1.0,
            yaw_delta: 0.2,
            ..Default::default()
        };
        for _ in 0..40 {
            sim.step(&fwd_turn);
        }
        let banked = sim.flyer.roll;
        assert!(banked > 0.1, "forward turn banks right, roll={banked}");
        // Add strafe: the bank must persist (no snap to level), staying
        // on the same side and comparable in magnitude.
        let fwd_turn_strafe = FlightInput {
            thrust: 1.0,
            strafe: 1.0,
            yaw_delta: 0.2,
            ..Default::default()
        };
        for _ in 0..30 {
            sim.step(&fwd_turn_strafe);
            assert!(
                sim.flyer.roll > 0.1,
                "bank holds while strafing, roll={}",
                sim.flyer.roll
            );
        }
    }

    #[test]
    fn strafe_alone_ignores_the_sideways_drift() {
        // The forward projection `v·fwd` excludes the instantaneous
        // strafe-aligned velocity, so strafing straight sideways with
        // no turn keeps the view level.
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        let strafe_only = FlightInput {
            strafe: 1.0,
            ..Default::default()
        };
        for _ in 0..40 {
            sim.step(&strafe_only);
        }
        assert!(
            sim.flyer.roll.abs() < 0.02,
            "pure sideways drift does not bank, roll={}",
            sim.flyer.roll
        );
    }

    #[test]
    fn altitude_model_switch_seeds_desired_from_pose() {
        let mut sim = Simulation::with_terrain(vec![0u8; MAP_TILES * MAP_TILES]);
        sim.flyer.y = 2.5;
        sim.sync_carpet_from_flyer();
        sim.lift_desired = 0; // stale
        sim.set_altitude_model(AltitudeModel::ExtendedLift);
        assert_eq!(sim.lift_desired, 640, "re-seeded at the live pose");
    }

    /// The 2026-07-19 ruling amendment: under Faithful altitude the
    /// enhanced mover runs the faithful vertical law — raw-aim dive,
    /// authority-banded climb — while aim pitch still never steals
    /// HORIZONTAL mobility (the level-plane thrust rule).
    #[test]
    fn faithful_altitude_dives_and_climbs_under_enhanced_thrust() {
        let mut sim = Simulation::new();
        sim.thrust_model = ThrustModel::Enhanced;
        sim.flyer.y = 2.0;
        let z0 = sim.flyer.z;
        // Aim hard down, then thrust: the raw-aim dive rides down to
        // the hover floor — at FULL horizontal speed.
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
        assert!(sim.flyer.y < 0.76, "dives to the floor, y={}", sim.flyer.y);
        assert!(
            z0 - sim.flyer.z > 8.0,
            "no horizontal mobility loss, z={} from {}",
            sim.flyer.z,
            z0
        );

        // Aim up: climbs, but climb authority zeroes at the
        // ground+band soft ceiling (4 tiles) — bounded, never free
        // vertical.
        let up = FlightInput {
            pitch_delta: 2.8, // clamps to +MAX_PITCH
            ..Default::default()
        };
        sim.step(&up);
        for _ in 0..300 {
            sim.step(&fwd);
        }
        assert!(sim.flyer.y > 2.0, "climbs, y={}", sim.flyer.y);
        assert!(
            sim.flyer.y <= 4.05,
            "authority caps at the soft ceiling, y={}",
            sim.flyer.y
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
