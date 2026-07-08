//! MC1 runtime world: the living level — trigger volumes, dispositions,
//! spawned entities, and runtime terrain-mutating events.
//!
//! This is the runtime face of the same event machinery the load-time
//! feature pass uses (`features::Gen`): in the original, one 1000-slot
//! pool and one dispatch family serve both. The runtime tick is a port
//! of `sub_41780_41AC0` (remc1 sub_main.cpp:52197), reduced to the
//! destructible-world slice:
//!
//! - **Dispositions** (`sub_37440_37800`, :43924): firing disposition N
//!   scans the live 1-based THING table and spawns every record whose
//!   `dis_id == N` (`sub_37560_37920`, :43988); one-shot fires zero the
//!   record. Level init fires disposition 0 — things authored with
//!   `dis_id != 0` do NOT exist until something fires their
//!   disposition (dis_id 0xFFFF = the load-time terrain features).
//! - **Class-11 trigger volumes** (spawn `sub_3BB20` :47771, tick table
//!   `str_256038` :4921): AABB volumes (radius = the THING's `swi_sz`
//!   tiles, height 4096 units) that fire the disposition in their
//!   `swi_id`. States 0-12 are proximity variants against the
//!   wizard-balloon list (for us: the player carpet), one-shot or
//!   repeating with a 10-tick player-absence rearm; states 13-30 fire
//!   when a class-5 model bucket has been empty 16 consecutive ticks
//!   ("all creatures of a kind dead"); state 4 is a collected-item
//!   trigger (stub until inventory exists).
//! - **Class-10 runtime events** reuse the load-time handlers verbatim
//!   (states 9/10/11 craters, walls, canyons, buildings...): the
//!   expanding crater that digs a few tiles per tick IS the original's
//!   "continuous" terrain alteration — the only difference from load
//!   time is one pass per turn instead of a fixpoint sweep, and the
//!   per-tick `f63` increment (:52406) that gates digger growth
//!   (`% 3`) and the trigger probe throttle (`& 7`).
//! - **Spawned drawables** run their real spawn handlers
//!   ([`crate::mobs`]): class-2 scenery, class-3 balloons/castles and
//!   class-5 creatures (with multipart body chains) carry authentic
//!   life/speed/extents/sprite state, and class-5 creatures TICK — the
//!   movement core, the six state primitives and the awake system are
//!   ported; the app consumes continuous poses via [`World::live_poses`].
//!
//! COMBAT (the combat slice, see [`crate::combat`]): class-5 attack
//! thunks fire class-9 projectiles / melee mailbox writes; class-10
//! combat effects deliver the damage; creatures read their inbox,
//! aggro on wizard-family attackers, die into DEATH/CORPSE and drop
//! mana balls. The player is MORTAL (2026-07-07): the six-channel
//! inbox applies for real — grace window, at-castle redirect, shield
//! quartering, hit knockback, the death fall with the jar scatter and
//! the m40 grave, and the Space respawn at the castle (castle-less =
//! the level restarts). The old invincibility survives as the
//! `invincible` dev/config toggle.
//!
//! Deliberate deviations, tracked in docs/ROADMAP.md: no AI wizard
//! balloons (the probe/scan lists are the player alone); custom
//! family behaviors beyond movement/combat (disguises, mana hunts,
//! house building, teleports) stand still pending the AI track;
//! class-12 pickup/mana transfer NOT ported (mana balls drop, merge
//! and take claims but nothing collects them yet); sounds omitted.

use crate::features::{
    self, FeatureAssets, Gen, Planes, Rec, TerrainPlanes, build_table, lcg32,
};
use crate::mc1_sprite_stats::SPRITE_STATS;
use crate::combat::MailTarget;
use crate::mobs::{MobCtx, PLAYER_TARGET};
use crate::spells::{SPELL_COUNT, SPELLS, SpellId};
use mgc_formats::{Thing, ThingKind};

/// The player's life ceiling: the human wizard ctor's maxLife 10000
/// (:44185; skill does NOT scale it — sub_44D30 :55026 resets to max
/// on every spawn). Heal's 5%-per-tick rate divides it.
pub const PLAYER_LIFE_MAX: i32 = 10000;

/// The class-12 state marking a death-scattered spell jar: it decays
/// (200-289 ticks, :55545-47) where the THING-placed jar states
/// (0..=2) sit forever. Pickup works from any sub-MANIFEST state.
const DROPPED_JAR: u8 = 3;

/// The player wizard's life state — the original class-3 states 0
/// (alive) / 2 (death fall) / 3 (dead, awaiting the respawn key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LifeState {
    #[default]
    Alive,
    /// The death fall (sub_45FC0 :55434): flight drifts on with no
    /// input, gravity −2/tick², a (10,1) fire trail, until the
    /// carpet lands at ground+128.
    Falling,
    /// Landed (sub_46480 :55594): grey screen, death camera toward
    /// the killer, waiting for Space.
    Dead,
}

/// The class-12 state marker separating OWNED spell manifestations
/// (tick70 = 200 + spell id, ours) from pre-placed JARS (tick70 0..=2
/// from the THING post-init) — the jar keeps its spawn state, the
/// manifestation keeps the jar's pool slot (slot economy is
/// load-bearing: level 032 depends on it).
const MANIFEST_BASE: u8 = 200;

/// The human player's carpet-side spell state — the original Type_160
/// slice: the +308 free mana pool, the var_940/944 hand equips, the
/// var_676 owned-spell table, and the +16/+17 effect flag bits.
pub struct Player {
    /// Wizard current mana (+140): stepped by [`Self::mana_delta`]
    /// each tick, clamped to [0, mana_max] (:55385-95).
    pub mana: u32,
    /// Wizard mana ceiling (+136): recomputed EVERY tick by
    /// sub_48230 (:56839, called :52327) = intrinsic base 1000
    /// (u32_322, :55031-33) + Σ +140 of everything claimed (+144):
    /// creatures, castle, balloons, mana balls, houses.
    pub mana_max: u32,
    /// Regen/debit delta (+132): +136/200 (min 1000) touching the
    /// own castle, else +136/2000 (min 100) (:55397-421); a cast
    /// debit OVERWRITES it negative for one tick (sub_55E80 :64936 —
    /// authored behavior; remc1 ships it commented out).
    mana_delta: i32,
    /// Banked mana: claimed-house tally (wizext u32_308) + own
    /// castle stored (+140) — the HUD % and win-check numerator.
    pub banked: u32,
    /// World total mana (str_184.u32_188), the HUD % denominator.
    pub world_mana: u32,
    /// Hand equips (var_940/944).
    pub left: Option<SpellId>,
    pub right: Option<SpellId>,
    /// Pool slot of each owned spell's class-12 manifestation entity,
    /// 0 = not owned (var_676).
    owned: [u16; SPELL_COUNT],
    /// Active toggle effects (carpet flag bits: shield +17 0x40,
    /// invisible +16 0x20, rebound +17 0x80). Derived each tick from
    /// the manifestations' burst counters.
    pub shield: bool,
    pub invisible: bool,
    pub rebound: bool,
    pub beyond_sight: bool,
    pub heal_active: bool,
    /// 0 none, +1 forward, -1 backward (types 2/21).
    pub accel: i8,
    /// The accelerate channel's cast button was held this tick — the
    /// held factor is 3.0 ("hold down the mouse button to achieve
    /// maximum speed"), 2.0 after release (:65169/:65175).
    accel_held: bool,
    /// Cached signed thrust-override factor (0.0 = inactive) —
    /// [`World::accel_override`].
    speed_boost: f32,
    /// Teleport return slot (:65554): recast returns here.
    teleport_return: Option<(u16, u16)>,
    /// Global Death's primed charge (:66235): ticks until the pulse
    /// fires around the carpet. APPROX ~2s pending a trace.
    bomb_timer: Option<i16>,
    /// Current life (actLife +12; signed — dying drives it below 0).
    pub life: i32,
    /// The spawn grace (Type_160 u16_331): while > 0 the whole
    /// mailbox is wiped each tick — total immunity (:55367-71).
    /// Respawn arms 100 (:54866); the at-castle redirect re-arms 2.
    grace: u16,
    /// Regen stall (u32_383): every processed hit sets 16 — no
    /// health regen for 16 ticks (:55387-90).
    regen_delay: u16,
    /// Life state (class-3 states 0/2/3).
    pub state: LifeState,
    /// Death-fall vertical speed (var_u16_29841_46), engine units.
    fall_speed: i16,
    /// The killer latch (+38): the death camera target and (were
    /// there rival wizards) the kill credit.
    killer: u16,
    /// Spell models owned at death — the original keeps them as
    /// MODEL NUMBERS in the 24 Type_160+532 slots and re-instantiates
    /// on respawn (:54884-923); the manifestation entities scatter
    /// as decaying jars meanwhile.
    death_owned: [bool; SPELL_COUNT],
    /// Red hit-flash ticks for the app overlay (sub_44BE0(2)).
    pub hit_flash: u8,
    /// Died castle-less in single player: the original sets the
    /// lost + level-over flags (+13325 |= 0xC, :48620-33) — the
    /// level restarts.
    pub lost: bool,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            mana: 1000,
            mana_max: 1000,
            mana_delta: 0,
            banked: 0,
            world_mana: 0,
            left: None,
            right: None,
            owned: [0; SPELL_COUNT],
            shield: false,
            invisible: false,
            rebound: false,
            beyond_sight: false,
            heal_active: false,
            accel: 0,
            accel_held: false,
            speed_boost: 0.0,
            teleport_return: None,
            bomb_timer: None,
            life: PLAYER_LIFE_MAX,
            grace: 100,
            regen_delay: 0,
            state: LifeState::Alive,
            fall_speed: 0,
            killer: 0,
            death_owned: [false; SPELL_COUNT],
            hit_flash: 0,
            lost: false,
        }
    }
}

/// The player's mortality snapshot for the app layer.
#[derive(Debug, Clone, Copy)]
pub struct PlayerVitals {
    pub life: i32,
    pub life_max: i32,
    pub state: LifeState,
    /// Remaining spawn-grace ticks (invulnerability window).
    pub grace: u16,
    /// Red hit-flash ticks remaining (sub_44BE0(2)).
    pub hit_flash: u8,
    /// Died castle-less — the level is lost (restarting).
    pub lost: bool,
    /// The own castle took a processed hit recently (+391 flash —
    /// the HUD castle sub-panel swaps to the alert marble [55]).
    pub castle_alert: bool,
    /// The player took a processed hit / steal / grip recently
    /// (+392 flash — the SELF sub-panel's alert marble, :55679-723).
    pub player_alert: bool,
    /// An own balloon took a processed hit recently (+393 flash —
    /// the balloon sub-panel's alert marble, :56826).
    pub balloon_alert: bool,
    pub has_castle: bool,
}

/// Spellbook/HUD snapshot for the app layer.
pub struct LoadoutView {
    pub owned: [bool; 24],
    pub left: Option<u8>,
    pub right: Option<u8>,
    /// 0.0 = ready, 1.0 = just fired (burst counter / count).
    pub cooldown: [f32; 24],
    pub mana: u32,
    pub mana_max: u32,
    /// Banked mana (claimed houses + castle stored) and the world
    /// total — the original castle-panel % (:54721) and the win
    /// check's numerator/denominator.
    pub banked: u32,
    pub world_mana: u32,
    /// Own castle (stored, capacity, level) when one stands.
    pub castle: Option<(u32, u32, u8)>,
    /// The player's mana-balloon ROSTER for the HUD's balloon
    /// sub-panel (sub_22E50 slot B, `var_52[]`). The length IS the
    /// roster size granted by castle level (1 at level 1-3, 2 at 4-5,
    /// 3 at 6-7 — the :27296-314 switch; the glyph is [50+len]);
    /// empty only with no castle (:27281 shows the marble [54] only
    /// when the castle pointer is invalid). Each slot = Some((hp_frac,
    /// cargo_frac)) for a live owned class-3/model-3 balloon, None for
    /// a dead/not-yet-spawned slot — retail keeps the roster width and
    /// just draws no bars for invalid entries (:27335-40). hp_frac =
    /// actLife/maxLife, cargo_frac = stored/capacity (+140/+136).
    pub balloons: Vec<Option<(f32, f32)>>,
    /// Own castle health (current, max) — the downgrade meter
    /// (functional-first; retail shows no castle HP number, but the
    /// player needs SOME way to see the vulture-bomb chip damage).
    pub castle_hp: Option<(i32, u32)>,
    /// The level goal: required banked % of the world total (the
    /// HUD goal tick, :27268). 0 = none wired.
    pub win_pct: u16,
    /// The latched completion flag.
    pub completed: bool,
    /// Per-spell BOOK BIND gate (the :26926 check): a spell can be
    /// hovered/bound iff its `castle_req` (+132, the castle-stored
    /// unlock ladder — ctor a8) is 0, or the linked castle STORES at
    /// least that much (+140). NOT a player-mana affordability test —
    /// retail never blocks binding on the castable pool; per-cast
    /// mana gates apply at cast time. All-true under dev_spells.
    pub bindable: [bool; 24],
}

/// The player's pose in engine units for trigger/portal tests: x/y are
/// 8.8 fixed-point tile coordinates, z is altitude in engine units
/// (256 = one tile of height, i.e. 32 per height byte), heading is the
/// engine's 11-bit angle (0 = north/-Z, matching the flyer's yaw 0).
#[derive(Debug, Clone, Copy)]
pub struct PlayerPose {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub heading: u16,
    /// Engine pitch (11-bit; POSITIVE pitches the polar step DOWN,
    /// matching the original's angle convention). 0 = level.
    pub pitch: u16,
    /// Forward speed in engine units per tick (the carpet's +126 —
    /// fired projectiles inherit it, :65060).
    pub speed: i16,
}

impl PlayerPose {
    /// From world-space tile floats + yaw/pitch radians (the flyer's
    /// state; flyer pitch is positive-up, engine pitch positive-down)
    /// and speed in tiles per tick.
    pub fn from_tiles(x: f32, y_alt: f32, z: f32, yaw: f32, pitch: f32, speed_tiles: f32) -> Self {
        const TAU: f32 = std::f32::consts::TAU;
        let wrap = |v: f32| (v.rem_euclid(256.0) * 256.0) as u16;
        PlayerPose {
            x: wrap(x),
            y: wrap(z),
            z: (y_alt * 256.0) as i16,
            heading: (yaw.rem_euclid(TAU) * (2048.0 / TAU)) as u16 & 0x7FF,
            pitch: ((-pitch).rem_euclid(TAU) * (2048.0 / TAU)) as u16 & 0x7FF,
            speed: (speed_tiles * 256.0) as i16,
        }
    }

    /// A level pose with no pitch/speed (tests, trigger probes).
    pub fn level(x: u16, y: u16, z: i16, heading: u16) -> Self {
        PlayerPose { x, y, z, heading, pitch: 0, speed: 0 }
    }
}

/// Player intent the sim consumes besides the pose. Part of the tick
/// input stream (replay-recorded once replays exist).
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerCommand {
    /// Left-hand cast held (the original's dw_0 fire bit 0x10; the
    /// carpet fire tick tests it per equipped hand,
    /// sub_46840_46B80 :55825-55834).
    pub fire_left: bool,
    /// Right-hand cast held (dw_0 bit 0x20).
    pub fire_right: bool,
    /// Equip a spell to a hand (the original's commands 0x15/0x16,
    /// :48717-48731) — from the book screen or a quick key.
    pub equip_left: Option<crate::spells::SpellId>,
    pub equip_right: Option<crate::spells::SpellId>,
    /// The respawn key (Space, command 15 :20081/:48620) — only
    /// consumed while dead.
    pub respawn: bool,
    /// The demolish key (Shift+L → the unique control word 48,
    /// :20496-501): sets the OWN castle's life to −1 (:55846-50) —
    /// one downgrade level per press.
    pub demolish: bool,
}

/// The runtime world of one loaded MC1/HW level.
pub struct World {
    g: Gen,
    /// Live 1-based THING table; dispositions consume from it.
    table: Vec<Rec>,
    /// Terrain planes changed since last cleared (renderer re-upload).
    pub terrain_dirty: bool,
    /// Live entity set changed since last cleared.
    pub entities_dirty: bool,
    /// A portal fired this tick: destination in tile units, consumed
    /// by the sim (which moves the flyer).
    pending_teleport: Option<(f32, f32)>,
    /// The human player's spell/mana state (spells cast through the
    /// per-hand dispatcher, sub_46B00_46E40 :55851).
    player: Player,
    /// Level completion threshold: the required banked percentage of
    /// the world total (the u16 at level-file offset 38800 — the
    /// first footer field; gamedata+232595, read by the win check
    /// :52128 and the HUD goal tick :27268). 0 = no goal wired.
    win_pct: u16,
    /// Consecutive ticks the banked share has exceeded the goal
    /// (sub_415C0 :52130-38; 16 latches the win).
    win_streak: u16,
    /// The latched completion flag (the original's per-player
    /// +13325 bit 2).
    completed: bool,
    /// Dev/playtest "all spells + infinite mana" switch (G-class).
    dev_spells: bool,
    /// Last tick's fire-button states — casts are EDGE-triggered (one
    /// cast per press) except the traced hold spells; the edges are
    /// derived sim-side from the held booleans.
    prev_fire: (bool, bool),
    /// The Accelerate brake veto for this tick, fed by
    /// [`World::thrust_cancel`]: .0 blocks type 2 (backward thrust
    /// held), .1 blocks type 21 (forward thrust held).
    accel_veto: (bool, bool),
    /// A respawn fired this tick: the sim moves the carpet there
    /// (tile units) and resets the flight state.
    pending_respawn: Option<(f32, f32)>,
    /// Castle-less death confirmed: the level restarts (the
    /// original's lost + level-over flags, :48620-33).
    pending_restart: bool,
    /// Dev/accessibility invincibility (config `invincible`, G-class
    /// dev family): the pre-mortality behavior — damage totaled for
    /// display, never applied.
    invincible: bool,
}

/// One live drawable entity, resolved for the app's billboard / map
/// layer: continuous pose (position in tile units, real-valued yaw)
/// plus the sprite-stats type index and animation frame the sim's
/// spawn/tick handlers assigned. Presentation resolves late — the
/// billboard backend snaps yaw to view sectors at draw time, a mesh
/// backend would consume the same pose unquantized.
#[derive(Debug, Clone, Copy)]
pub struct LivePose {
    pub class: u8,
    pub model: u8,
    /// Row into [`crate::mc1_sprite_stats::SPRITE_STATS`].
    pub type_index: u16,
    /// Animation frame (entity offset 88) for the 2..=16 draw types.
    pub frame: u8,
    /// Position, tile units (torus [0, 256)).
    pub x: f32,
    pub z: f32,
    /// Altitude, tile units.
    pub alt: f32,
    /// Facing, radians (0 = north/-Z like the flyer's yaw).
    pub yaw: f32,
    /// Multipart body segment (state 120) — drawn but excluded from
    /// entity counts/lists like the original's map/behavior scans.
    pub segment: bool,
    /// Remaining life fraction (0..=1) for monsters (class-5 chain
    /// heads) — feeds the unfaithful debug health-bar overlay. None
    /// for everything the overlay shouldn't tag.
    pub life_frac: Option<f32>,
    /// The entity belongs to the player: owner (+24) for projectiles/
    /// effects, claim owner (+144) for balls/houses, castle/balloon
    /// ownership for class 3. Drives the map's team-color rule
    /// (sub_48710: owner class-3 → byte_99B58 team pair).
    pub player_owned: bool,
}

/// Minimal live-event view for [`World::debug_pool`].
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct DebugEvent {
    pub slot: usize,
    pub class: u8,
    pub model: u8,
    pub state: u8,
    pub id24: u16,
    pub tx: u8,
    pub ty: u8,
    pub life: i32,
}

/// One tick's audio outputs: drained sound requests + the ambient
/// rule inputs (see [`World::take_audio`]).
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub events: Vec<crate::features::SoundEvent>,
    /// The carpet is over a water tile (waves vs wind, :55254-65).
    pub over_water: bool,
    pub fire_near: bool,
    pub market_near: bool,
    /// Danger-music mode (the wizard's v_46 countdown is live —
    /// recently hit or targeted; :55282-92).
    pub danger: bool,
}

/// A live gameplay volume for the map overlay (an opt-in enhancement
/// / debugging instrument — the original never reveals trigger areas).
#[derive(Debug, Clone, Copy)]
pub struct ActiveVolume {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub kind: VolumeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeKind {
    /// Fly-into proximity trigger (one-shot or repeating).
    Proximity,
    /// Fires when a watched creature kind is wiped out.
    KillWatch,
    /// Fires on a collected item (stub until inventory).
    Inventory,
    /// Teleporter vortex.
    Portal,
}

/// Records the app can draw (mc1_entities has a sprite mapping).
/// Class 9 = projectiles; class 10 is logic/terrain except the portal
/// vortex and the combat effects (fire, flame, splash, flashes, mana
/// ball — the model-17 blast driver is invisible by design).
fn drawable(class: u16, model: u16) -> bool {
    // The (10,12) possess flash carries the ctor's sprite row 41 but
    // draws NOTHING in retail (player-confirmed) — its draw gate is
    // whatever +16 bit the ctor clears; excluded here. Also excluded:
    // the genuinely invisible drivers (15 quake walker, 17 blast
    // ring, 18 eruption counter, 41/42 leveler/painter, 53 napalm
    // cloud — its visible part is the (10,6) sheets it spawns).
    // 6 standing fire / 16 lava bomb / 19 plume / 38 storm cloud /
    // 43 upgrade token ARE sprite-carrying visibles — their absence
    // here was the burning-tree-without-flame report (2026-07-07)
    // and playtest-3's "wall of fire didn't even show".
    matches!(class, 2 | 3 | 5 | 9 | 12)
        || (class == 10
            && matches!(model, 34 | 0 | 1 | 5 | 6 | 16 | 19 | 23 | 25 | 38 | 39 | 40 | 43 | 45))
}

impl World {
    /// Build the world: apply the load-time feature pass to the
    /// pristine planes, then fire disposition 0 (level init) so the
    /// initial population spawns. `things` come from the package;
    /// `seed` is the GEN_MAP seed.
    pub fn new(planes: Planes, things: &[Thing], seed: u32, assets: FeatureAssets) -> Self {
        let mut table = build_table(things);
        let mut g = Gen::new(planes, assets, seed);
        g.load_time_pass(&mut table);
        let mut w = World {
            g,
            table,
            terrain_dirty: false,
            entities_dirty: false,
            pending_teleport: None,
            player: Player::default(),
            win_pct: 0,
            win_streak: 0,
            completed: false,
            dev_spells: false,
            prev_fire: (false, false),
            accel_veto: (false, false),
            pending_respawn: None,
            pending_restart: false,
            invincible: false,
        };
        w.fire_disposition(0, true);
        // Starting spells AFTER the level population so the initial
        // spawns keep their original pool slots (per-slot LCG seeds).
        w.grant_starting_spells();
        w
    }

    /// Load-time-features-only view (parity helper for callers that
    /// want the planes without the runtime; MC2 uses `TerrainPlanes`
    /// directly until its feature pass is ported).
    pub fn planes(&self) -> &Planes {
        &self.g.t
    }

    /// Snapshot of the live drawable entities as THING-shaped records
    /// (kind = Entity), one per creature/scenery/pickup — multipart
    /// body segments excluded, like the original's entity lists.
    pub fn live_things(&self) -> Vec<Thing> {
        let mut out = Vec::new();
        for (i, e) in self.g.ent.iter().enumerate().skip(1) {
            if e.class64 == 0 || !drawable(e.class64 as u16, e.model65 as u16) {
                continue;
            }
            if e.class64 == 5 && e.tick70 == 120 {
                continue;
            }
            // Owned-spell manifestations occupy their (former jar)
            // slot but are not world drawables.
            if e.class64 == 12 && e.tick70 >= MANIFEST_BASE {
                continue;
            }
            out.push(Thing {
                slot: (e.thing_slot as u32).saturating_sub(1),
                kind: ThingKind::Entity,
                class: e.class64 as u16,
                model: e.model65 as u16,
                x: e.x >> 8,
                y: e.y >> 8,
                dis_id: 0,
                swi_sz: 0,
                swi_id: if e.type86 == 280 { 3 } else { 0 },
                parent: 0,
                child: 0,
                par3: None,
            });
            let _ = i;
        }
        out
    }

    /// The live drawable set with continuous pose + resolved sprite
    /// type — what the app's billboard and map-dot layers consume.
    pub fn live_poses(&self) -> Vec<LivePose> {
        const TAU: f32 = std::f32::consts::TAU;
        let mut out = Vec::new();
        for e in self.g.ent.iter().skip(1) {
            if e.class64 == 0 || !drawable(e.class64 as u16, e.model65 as u16) {
                continue;
            }
            if e.class64 == 12 && e.tick70 >= MANIFEST_BASE {
                continue; // owned manifestation, not a drawable
            }
            // Houses (m45): the visible building is painted terrain;
            // the entity billboard is the OWNER FLAG (sprite 177 +
            // color row) — drawn only once CLAIMED. APPROX: the
            // original's exact draw gate for the neutral state is
            // untraced (the claim clears +16 bit 0); claimed-only
            // matches the known "captured buildings fly your flag".
            if e.class64 == 10 && e.model65 == 45 && e.f144 == 0 {
                continue;
            }
            let segment = e.class64 == 5 && e.tick70 == 120;
            out.push(LivePose {
                class: e.class64,
                model: e.model65,
                type_index: e.type86,
                frame: e.frame88,
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                alt: e.z as f32 / 256.0,
                yaw: (e.f30 & 0x7FF) as f32 * (TAU / 2048.0),
                segment,
                life_frac: (e.class64 == 5 && !segment && e.max_life > 0).then(|| {
                    (e.act_life.max(0) as f32 / e.max_life as f32).min(1.0)
                }),
                player_owned: e.id24 == PLAYER_TARGET
                    || (e.class64 == 10 && e.f144 == PLAYER_TARGET),
            });
        }
        out
    }

    /// One game turn (`sub_41780_41AC0`, :52197). `player` feeds the
    /// trigger volume probes, creature awake checks and aggro scans;
    /// `cmd` is the rest of the player's tick input (fire).
    pub fn tick(&mut self, player: PlayerPose, cmd: PlayerCommand) {
        // One global LCG draw per tick, before any handler (:52223).
        lcg32(&mut self.g.rand);

        // Broad-phase bucket counts for the kill triggers: class-5
        // events by model, excluding state 120 (multipart body
        // segments in the original; :52246 list building).
        let mut buckets = [0u32; 20];
        let mut any_creature = false;
        let mut any_transient = false;
        for e in &self.g.ent {
            if e.class64 == 5 && e.act_life >= 0 && e.tick70 != 120 {
                buckets[(e.model65 as usize).min(19)] += 1;
                any_creature = true;
            }
            if e.class64 == 9
                || (e.class64 == 10
                    && matches!(e.tick70, 0 | 1 | 5 | 17 | 18 | 21 | 23 | 25 | 41))
            {
                any_transient = true;
            }
        }

        let ctx = MobCtx {
            px: player.x,
            py: player.y,
            pz: player.z,
        };

        // The per-tick mana census (sub_48230 :56839, called :52327
        // BEFORE all entity ticks).
        self.recompute_mana();

        // The completion check (sub_415C0 :52100-40): a wizard WITH
        // a castle whose banked share of the world total exceeds the
        // level goal (strictly — `<=` resets, :52128) for 16
        // consecutive ticks wins. Ours: the human player only.
        if self.win_pct > 0 && !self.completed {
            let over = self.player.world_mana != 0
                && self.player_castle().is_some()
                && 100u64 * self.player.banked as u64 / self.player.world_mana as u64
                    > self.win_pct as u64;
            if over {
                self.win_streak += 1;
                if self.win_streak >= 16 {
                    self.completed = true;
                }
            } else {
                self.win_streak = 0;
            }
        }

        // The wizard mana tick (:55385-421) — BEFORE cast handling,
        // like the original wizard tick (regen first, casts later in
        // the same function): step the pool by the delta (a cast
        // debit overwrote it negative last turn — it lands here),
        // clamp to [0, max], then recompute the delta: fast regen
        // touching the own castle (max/200, floor 1000), slow afield
        // (max/2000, floor 100).
        let stepped = self.player.mana as i64 + self.player.mana_delta as i64;
        self.player.mana = stepped.clamp(0, self.player.mana_max as i64) as u32;
        let at_castle = self.player_castle().is_some_and(|c| {
            let e = &self.g.ent[c];
            ((player.x.wrapping_sub(e.x) as i16).unsigned_abs() as u16) <= e.f80
                && ((player.y.wrapping_sub(e.y) as i16).unsigned_abs() as u16) <= e.f82
        });
        self.player.mana_delta = if at_castle {
            ((self.player.mana_max / 200) as i32).max(1000)
        } else {
            ((self.player.mana_max / 2000) as i32).max(100)
        };

        // Mirror the cloak/deflection flags into the pool engine for
        // the mob-side gates (invisible :65689-90 = +16 0x20, rebound
        // :65774 = the ported deflection bit +17 0x80). One-tick-old
        // view: the flags refresh in the manifestation ticks below.
        self.g.player_invisible = self.player.invisible;
        self.g.player_rebound = self.player.rebound;

        // Hand equips (the original's commands 0x15/0x16, :48717-31).
        self.equip_hands(cmd.equip_left, cmd.equip_right);

        // Per-hand cast triggers (the carpet fire tick,
        // sub_46840_46B80 :55825-34, dw_0 bits 0x10/0x20). Casting is
        // EDGE-triggered — one cast per press, re-arm on release —
        // except the traced hold spells (23 firehose, 15 stream, the
        // channels); the edges derive from last tick's held state.
        let edge = (
            cmd.fire_left && !self.prev_fire.0,
            cmd.fire_right && !self.prev_fire.1,
        );
        self.prev_fire = (cmd.fire_left, cmd.fire_right);
        let alive = self.player.state == LifeState::Alive;
        if alive && cmd.fire_left && let Some(s) = self.player.left {
            self.cast_spell(s, false, edge.0, player, &ctx);
        }
        if alive && cmd.fire_right && let Some(s) = self.player.right {
            self.cast_spell(s, true, edge.1, player, &ctx);
        }

        // The demolish key (:55846-50): the OWN castle's life goes
        // to −1 — the castle tick's damage path does the rest (one
        // downgrade level per press; the last one costs the respawn
        // point).
        if alive && cmd.demolish {
            if let Some(c) = self.player_castle() {
                self.g.ent[c].act_life = -1;
            }
        }

        // The awake pre-pass (sub_54F00, :64266) runs before dispatch.
        self.g.mob_awake_pass(&ctx);

        for i in 1..features::POOL {
            if self.g.ent[i].class64 == 0 {
                continue;
            }
            match self.g.ent[i].class64 {
                5 => self.g.creature_tick(i, &ctx),
                9 => {
                    if self.g.proj_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 if self.g.ent[i].tick70 == 36 => self.portal_tick(i, player),
                // Live village buildings and their collapse.
                10 if self.g.ent[i].tick70 == 52 => self.g.tick_building_live(i),
                10 if self.g.ent[i].tick70 == 53 => {
                    self.g.tick_building_collapse(i);
                    self.terrain_dirty = true;
                }
                // Combat effects (fire, spreader, splash, possess
                // flash, lava bomb, blast ring, eruption driver,
                // plume, magnet, hit-flash, steal-flash, storm
                // cloud, mana ball, grave, collapse magnet).
                10 if matches!(
                    self.g.ent[i].tick70,
                    0 | 1 | 5 | 6 | 12 | 16 | 17 | 18 | 19 | 21 | 23 | 25 | 40 | 41 | 42 | 58 | 59
                ) => {
                    if self.g.effect_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 => {
                    // The load-time handlers ARE the runtime handlers.
                    self.g.tick(i, Some(&ctx));
                    self.terrain_dirty = true;
                }
                11 => self.trigger_tick(i, player, &buckets),
                // The player-built castle's state machine (class-3
                // m2) and its mana balloons (m3; wizard castles =
                // a later track).
                3 if self.g.ent[i].model65 == 2 => self.g.castle_tick(i),
                3 if self.g.ent[i].model65 == 3 => self.g.balloon_tick(i),
                // Trees burn (states 0/1/2 + the standing fire).
                2 if self.g.ent[i].model65 == 0 => self.g.tree_tick(i),
                // Spell jars (pickup) and owned-spell manifestations
                // (burst countdown + continuous effects).
                12 => self.class12_tick(i, &ctx),
                // Scenery: inert until its tracks land — stands and
                // renders.
                _ => {}
            }
            // Per-tick phase counter, incremented after the state
            // handler (:52406); gates digger growth and probe cadence.
            self.g.ent[i].f63 = self.g.ent[i].f63.wrapping_add(1);
            if self.g.ent[i].flags & 0x400 != 0 {
                self.free_slot(i);
            }
        }
        if any_creature || any_transient {
            // Creatures/projectiles/effects move: poses refresh.
            self.entities_dirty = true;
        }

        // ---- player damage intake (the wizard tick's mailbox block,
        // sub_45C90 :55344-74 + sub_46540 :55641-737) ----
        if self.player.state == LifeState::Alive {
            // The at-castle redirect (:55353-62): with the own castle
            // underfoot, pending ch0 damage FORWARDS into the
            // castle's mailbox — the castle tanks for you — and the
            // grace re-arms to 2 (:55363, an unconditional write:
            // sitting home under fire deliberately shortens a fresh
            // 100-tick spawn grace).
            if at_castle && self.g.player_mail[0].1 != 0 {
                if let Some(c) = self.player_castle() {
                    let (amt, src) = self.g.player_mail[0];
                    self.g.mail_write(MailTarget::Pool(c), 0, amt, src);
                    self.g.player_mail[0] = (0, 0);
                    self.player.grace = 2;
                }
            }
            if self.invincible || self.player.grace > 0 {
                // The grace memset (:55367-71): every channel wiped,
                // total immunity — steal and grip included, and the
                // danger music stays calm (sub_46540 never runs).
                // Under the dev invincibility we keep the old
                // playtest behaviors: the ch0 total accumulates for
                // display and any mail arms the danger music.
                if self.invincible {
                    if self.g.player_mail[0].1 != 0 {
                        let amt = self.g.player_mail[0].0 as u64;
                        self.g.player_damage +=
                            if self.player.shield { amt / 4 } else { amt };
                    }
                    if self.g.player_mail.iter().any(|&(_, from)| from != 0) {
                        self.g.player_danger = 100;
                    }
                } else {
                    self.player.grace -= 1;
                }
                self.g.player_mail = [(0, 0); 6];
            } else {
                self.apply_player_damage(player);
            }
            // Health regen (:55381-421): stalled 16 ticks by every
            // processed hit, then maxLife/250 per tick at the own
            // castle vs maxLife/2000 afield.
            if self.player.regen_delay > 0 {
                self.player.regen_delay -= 1;
            } else if self.player.life < PLAYER_LIFE_MAX {
                let rate = if at_castle {
                    PLAYER_LIFE_MAX / 250
                } else {
                    PLAYER_LIFE_MAX / 2000
                };
                self.player.life = (self.player.life + rate).min(PLAYER_LIFE_MAX);
            }
        } else {
            // Falling/dead: the landing wipe already cleared the
            // mailbox; discard anything new (the original's dead
            // wizard never reads it).
            self.g.player_mail = [(0, 0); 6];
        }
        if self.g.player_danger > 0 {
            self.g.player_danger -= 1;
        }
        if self.player.hit_flash > 0 {
            self.player.hit_flash -= 1;
        }
        if self.g.castle_alert > 0 {
            self.g.castle_alert -= 1;
        }
        if self.g.player_alert > 0 {
            self.g.player_alert -= 1;
        }
        if self.g.balloon_alert > 0 {
            self.g.balloon_alert -= 1;
        }

        // ---- the death fall and the wait for Space ----
        match self.player.state {
            LifeState::Falling => {
                // The fire trail (:55478-83): one damage-suppressed
                // (10,1) spreader per tick at the carpet.
                if let Some(s) = self.g.spawn_effect(1, player.x, player.y, player.z) {
                    self.g.ent[s].flags |= 0x80 | 0x10000;
                    self.g.ent[s].id24 = PLAYER_TARGET;
                }
                // Landing (:55485): the sim's fall integration rides
                // the z-floor down; ground+128 is touchdown.
                let ground = self.g.ground_z(player.x, player.y) as i16;
                if player.z <= ground.saturating_add(128) {
                    self.player_land(player);
                }
            }
            LifeState::Dead => {
                if cmd.respawn {
                    self.player_respawn();
                }
            }
            LifeState::Alive => {}
        }
        // The village-aggro timer runs down once per wizard tick
        // (:55405-06) — ~200 ticks of militia hostility per offense.
        if self.g.player_aggro > 0 {
            self.g.player_aggro -= 1;
        }

        // Global Death's primed charge (:66235): no visible effect —
        // it counts down after the cast, then a single small pulse
        // around the CARPET at expiry ("you have to be straight below
        // a dragon to affect it"). APPROX ~55 ticks pending a trace.
        if let Some(t) = self.player.bomb_timer {
            if t <= 1 {
                self.player.bomb_timer = None;
                self.bomb_pulse(&ctx);
            } else {
                self.player.bomb_timer = Some(t - 1);
            }
        }

        // Types 2/21 thrust-override factor for the flyer (3.0 while
        // the cast button is held — "hold down the mouse button to
        // achieve maximum speed" — 2.0 after release, negative for
        // backward; :65169/:65175). Computed after the manifestation
        // ticks so an expired burst drops the override the same turn.
        self.player.speed_boost = match self.player.accel {
            0 => 0.0,
            a => (if self.player.accel_held { 3.0 } else { 2.0 }) * a.signum() as f32,
        };
        self.player.accel_held = false;
        self.accel_veto = (false, false);
    }

    // ---- player mortality (sub_46540 / sub_45FC0 / sub_44D30) -------------

    /// sub_46540 (:55641): apply the pending mailbox channels to the
    /// mortal player.
    fn apply_player_damage(&mut self, player: PlayerPose) {
        // ch4 grip (:55663-81): the wizard duel tether — no rival
        // wizards cast it yet; the intake side effects land (regen
        // stall + danger music), the tether itself is the duel track.
        if self.g.player_mail[4].1 != 0 {
            self.g.player_mail[4] = (0, 0);
            self.player.regen_delay = 16;
            self.g.player_danger = 100;
            // "You are being attacked" flash (+392=4, :55679) — the
            // HUD's SELF sub-panel, not the castle's.
            self.g.player_alert = 4;
        }
        // ch3 mana steal (:55683-97): the pool drains; a class-3
        // thief would bank it (mob feeders aren't wizards).
        if self.g.player_mail[3].1 != 0 {
            let amt = self.g.player_mail[3].0;
            self.g.player_mail[3] = (0, 0);
            self.player.mana = self.player.mana.saturating_sub(amt);
            self.player.regen_delay = 16;
            self.g.player_danger = 100;
            self.g.player_alert = 4; // +392=4 (:55692)
        }
        // ch0 physical (:55698-735).
        if self.g.player_mail[0].1 != 0 {
            let (mut amt, src) = self.g.player_mail[0];
            self.g.player_mail[0] = (0, 0);
            // Shield (:55700-07): quarter the damage, and the
            // quarter is ALSO paid from mana. (The original clears
            // the +17 0x40 flag per absorb; the manifestation
            // re-arms it every tick, so the quartering is
            // continuous while the spell runs.)
            if self.player.shield {
                amt /= 4;
                self.player.mana = self.player.mana.saturating_sub(amt);
            }
            self.g.player_damage += amt as u64;
            self.player.life -= amt as i32;
            // Knockback (:55711-21): v_24 = the source→victim
            // bearing, v_22 = amount/10 clamped [0, 80] — an
            // overwrite of whatever knock was pending.
            let s = src as usize;
            if src != 0
                && src != PLAYER_TARGET
                && s < features::POOL
                && self.g.ent[s].class64 != 0
            {
                let dir =
                    Gen::angle_between(self.g.ent[s].x, self.g.ent[s].y, player.x, player.y)
                        & 0x7FF;
                self.g.player_knock = (dir, ((amt / 10) as i16).clamp(0, 80));
            }
            // Red flash (sub_44BE0(2)), self-panel flash (+392=4,
            // :55723), regen stall, hit sound 17 (:55722-26) — all
            // fire even on a fatal hit.
            self.player.hit_flash = 5;
            self.g.player_alert = 4;
            self.player.regen_delay = 16;
            self.g.snd_player(17);
            if self.player.life < 0 {
                // Fatal (:55729): latch the killer; the state flip
                // + death sound are the wizard tick's death check
                // (:55424-29), same turn.
                self.player.killer = src;
                self.player.state = LifeState::Falling;
                self.player.fall_speed = 0;
                self.g.snd_player(16);
                return;
            }
            self.g.player_danger = 100;
        }
        // ch1/ch2/ch5 have no player consumers (mask 29 filters most
        // writers already); drop anything stale.
        self.g.player_mail = [(0, 0); 6];
    }

    /// The death landing (:55485-569): wipe the mailbox, scatter the
    /// spell inventory as decaying jars, raise the (10,40) grave and
    /// hand it the player's loose mana balls (possess the grave to
    /// reclaim them), then wait for Space.
    fn player_land(&mut self, player: PlayerPose) {
        self.g.player_mail = [(0, 0); 6];
        // Jar scatter (:55519-47): the 24 slots remember the MODELS
        // (re-instantiated on respawn); the manifestation entities
        // become world jars again, thrown into a ±1-tile box with
        // 200-289 ticks to live. Three LCG draws per jar; the
        // original rolls the dying wizard's private stream — ours
        // uses the world stream (APPROX: same constants; the wizard
        // stream isn't modeled outside flight).
        for s in 0..SPELL_COUNT {
            let m = self.player.owned[s] as usize;
            if m == 0 {
                continue;
            }
            self.player.death_owned[s] = true;
            self.player.owned[s] = 0;
            let d1 = features::lcg32(&mut self.g.rand);
            let d2 = features::lcg32(&mut self.g.rand);
            let d3 = features::lcg32(&mut self.g.rand);
            let x = player.x.wrapping_add(((d1 & 0x1FF) as i32 - 256) as u16);
            let y = player.y.wrapping_add(((d2 & 0x1FF) as i32 - 256) as u16);
            let z = self.g.ground_z(x, y) as i16;
            {
                let e = &mut self.g.ent[m];
                e.tick70 = DROPPED_JAR;
                e.f26 = (d3 % 90 + 200) as i16;
            }
            self.g.move_relink(m, x, y, z);
        }
        // The grave (:55550-65). On a full pool the original retries
        // the whole landing next tick; ours proceeds graveless (the
        // balls simply stay player-owned) — a benign deviation.
        let gz = self.g.ground_z(player.x, player.y) as i16;
        if let Some(gv) = self.g.spawn_grave(player.x, player.y, gz) {
            for j in 1..features::POOL {
                if self.g.ent[j].class64 == 10
                    && self.g.ent[j].model65 == 39
                    && self.g.ent[j].flags & 0x400 == 0
                    && self.g.ent[j].f144 == PLAYER_TARGET
                {
                    self.g.ent[j].f144 = gv as u16;
                }
            }
        }
        self.player.state = LifeState::Dead;
        self.entities_dirty = true;
    }

    /// sub_44D30 (:54802) via the Space command (case 0xF :48620-33):
    /// respawn at the castle; castle-less in single player = the
    /// lost + level-over flags — the level restarts.
    fn player_respawn(&mut self) {
        let Some(c) = self.player_castle() else {
            self.player.lost = true;
            self.pending_restart = true;
            return;
        };
        let e = &self.g.ent[c];
        self.pending_respawn = Some((e.x as f32 / 256.0, e.y as f32 / 256.0));
        // Type_160 re-arm (:54866-83) + HP/mana reset (:55019-32).
        self.player.state = LifeState::Alive;
        self.player.life = PLAYER_LIFE_MAX;
        self.player.grace = 100;
        self.player.regen_delay = 0;
        self.player.killer = 0;
        self.player.hit_flash = 0;
        self.g.player_knock = (0, 0);
        self.g.player_danger = 0;
        self.player.mana = self.player.mana_max;
        // Jar re-instantiation (:54884-923): every remembered model
        // returns as an owned manifestation; the scattered decaying
        // jars stay out in the world until they expire. Hand equips
        // survive death untouched (the original never clears
        // var_940/944 on respawn).
        for s in 0..SPELL_COUNT {
            if self.player.death_owned[s] {
                self.player.death_owned[s] = false;
                self.grant_spell(SpellId(s as u8));
            }
        }
    }

    // ---- player spells (sub_46B00_46E40 :55851 + the 24 cast arms) --------

    /// INTERIM until the level-data spell block is decoded: every new
    /// World grants Fireball (0) + Possess (3), auto-equipped L/R in
    /// the original's auto-fill order (:49246-54).
    fn grant_starting_spells(&mut self) {
        self.grant_spell(SpellId(0));
        self.grant_spell(SpellId(3));
    }

    /// Materialize an owned spell: a class-12 manifestation ENTITY in
    /// the pool (the original's sub_3BF70 slot economy — spell
    /// manifestations compete with monsters for slots). tick70 =
    /// [`MANIFEST_BASE`] + spell id; +48 burst counter → our f26,
    /// +44 damage → f44 (count/possess read from the static table).
    /// Auto-fills an empty hand, LEFT first (:49246-54).
    fn grant_spell(&mut self, spell: SpellId) -> Option<usize> {
        let id = spell.0 as usize;
        if id >= SPELL_COUNT {
            return None;
        }
        if self.player.owned[id] != 0 {
            return Some(self.player.owned[id] as usize);
        }
        let m = self.g.new_event()?;
        {
            let e = &mut self.g.ent[m];
            e.class64 = 12;
            e.model65 = spell.0;
            e.tick70 = MANIFEST_BASE + spell.0;
            e.flags &= !8; // never a damage victim
            e.f26 = 0;
            e.f44 = SPELLS[id].damage.min(u16::MAX as u32) as u16;
        }
        self.player.owned[id] = m as u16;
        if self.player.left.is_none() {
            self.player.left = Some(spell);
        } else if self.player.right.is_none() {
            self.player.right = Some(spell);
        }
        Some(m)
    }

    /// Dev/playtest toggle (G-class enhancement; the original ships
    /// equivalent debug commands — the :48836 cheat menu's "access
    /// all spells" / "more mana"): grants every spell (spawning the
    /// missing class-12 manifestations, auto-equipping L/R if empty)
    /// and pins the mana pool full (no gate, no deduction, and
    /// [`LoadoutView::mana`] reads as full while on). Turning it OFF
    /// keeps the granted manifestations and any spells acquired
    /// meanwhile (no un-granting — the slot economy stays honest; if
    /// the pool cannot fit all 24, what fits is granted).
    pub fn set_dev_spells(&mut self, on: bool) {
        self.dev_spells = on;
        if on {
            for s in 0..SPELL_COUNT as u8 {
                self.grant_spell(SpellId(s));
            }
        }
    }

    pub fn dev_spells(&self) -> bool {
        self.dev_spells
    }

    /// Grant a specific set of spells at level start — the app's
    /// `plausible_spellbook` playtest instrument (the campaign-inferred
    /// "could legitimately have" set; see mgc-app `campaign`). Reuses
    /// the normal grant path: each spawns a class-12 manifestation and
    /// auto-equips L/R if empty, honestly competing for pool slots.
    /// Unknown/duplicate ids are no-ops. Grants ON TOP of whatever the
    /// world already gave (starting spells etc.).
    pub fn grant_spells(&mut self, spells: &[u8]) {
        for &s in spells {
            if (s as usize) < SPELL_COUNT {
                self.grant_spell(SpellId(s));
            }
        }
    }

    /// Wire the level's completion goal: the required banked share
    /// (percent of world mana) — the level footer's first u16
    /// (offset 38800; the original's gamedata+232595).
    pub fn set_win_pct(&mut self, pct: u16) {
        self.win_pct = pct;
    }

    /// The latched level-completion flag (sub_415C0: banked share
    /// above the goal for 16 consecutive ticks).
    pub fn completed(&self) -> bool {
        self.completed
    }

    /// One hand's cast trigger — the port of sub_46B00_46E40 :55851 +
    /// LABEL_32 :55892, simplified per the agreed interim semantics.
    /// Gate: owned && mana covers the possess cost && the
    /// manifestation's burst counter (+48 → f26) is 0. On trigger:
    /// burst = count, emit ONCE, deduct possess/count — the authored
    /// per-shot deduction remc1 ships commented out by its maintainer
    /// (:64946-50, a known mis-fix pattern); we implement it.
    ///
    /// Trigger classes (player-validated 2026-07-06):
    /// - 23 Rapid Fireball: the ONLY hold-to-autofire — held fire
    ///   re-arms the window every tick (:20627-30), one emission per
    ///   game tick (the firehose).
    /// - 15 Lightning Bolt: hold = continuous stream that keeps
    ///   emitting at its burst pacing (manual: "hold down the mouse
    ///   for a continuous stream").
    /// - 1/2/4/5/12/14/21: hold-to-channel toggles.
    /// - Everything else (incl. 0 Fireball): EDGE-triggered — one
    ///   cast per press, release + re-press to fire again, still
    ///   paced by the burst counter.
    fn cast_spell(&mut self, spell: SpellId, right: bool, edge: bool, p: PlayerPose, ctx: &MobCtx) {
        let id = spell.0 as usize;
        if id >= SPELL_COUNT {
            return;
        }
        let m = self.player.owned[id] as usize;
        if m == 0 {
            return;
        }
        let def = &SPELLS[id];

        // 23: the firehose.
        if id == 23 {
            if !self.spell_gate(def) {
                self.g.snd_player(29); // cast-blocked buzz (:64930)
                return;
            }
            // Per-shot debit at cost/count: the original charges the
            // full +136 per 3-tick refire window at 1 shot/tick —
            // the same drain rate, and the negative delta correctly
            // starves regen while the stream is held.
            self.mana_debit(def.possess_mana / def.count as u32);
            self.g.ent[m].f26 = def.count as i16;
            self.break_cloak(id);
            // Per-shot discharge (:66296 family 9): every fireball of
            // the firehose thunks — the machine-gun sound is the
            // spell's identity (player-reported gap, playtest 3).
            self.g.snd_player(9);
            self.cast_fireball(p, right, id);
            return;
        }

        let armed = self.g.ent[m].f26 > 0;

        // The hold-to-channel toggles re-arm while held (:55871..).
        if matches!(id, 1 | 2 | 4 | 5 | 12 | 14 | 21) {
            // The Accelerate brake veto (manual: "press the down
            // cursor to cancel"): a resisting thrust input this tick
            // keeps the channel down ([`World::thrust_cancel`]).
            if (id == 2 && self.accel_veto.0) || (id == 21 && self.accel_veto.1) {
                return;
            }
            if !armed {
                if !self.spell_gate(def) {
                    self.g.snd_player(29); // cast-blocked buzz
                    return;
                }
                self.mana_debit(def.possess_mana);
            }
            self.g.ent[m].f26 = def.count as i16;
            if matches!(id, 2 | 21) {
                self.player.accel_held = true; // held = the 3.0 factor
            }
            if !armed {
                self.break_cloak(id);
                self.emit_spell(id, m, p, right, ctx);
            }
            return;
        }

        // Edge-triggered casts (15 streams while held), paced by the
        // burst spacing (fireball 5, meteor 11, castle 101 ticks).
        if (!edge && id != 15) || armed {
            return;
        }
        // Create Castle (the model-16 trigger arm, :55901-11): a
        // recast while the build chain lives FIZZLES (the original
        // pins the manifestation's charge through the build — +48
        // nonzero → buzz 29); the mana check is SILENT; and the
        // cast COST is dynamic — sub_47C60/sub_47DD0 rewrite the
        // manifestation's +136 to the capacity ladder at the
        // castle's CURRENT level on every init/level-up (the
        // player-remembered doubling threshold: upgrading costs the
        // FULL next-stage amount; the fresh castle keeps the ctor
        // 1000).
        if id == 16 {
            if self.castle_build_lives() {
                self.g.snd_player(29); // the pinned-charge fizzle
                return;
            }
            let cost = self
                .player_castle()
                .map(|c| {
                    Gen::CASTLE_CAP[self.g.ent[c].f26.clamp(0, 7) as usize] as u32
                })
                .unwrap_or(def.possess_mana);
            if !self.dev_spells && self.player.mana < cost {
                return; // silent (:55908-10)
            }
            self.mana_debit(cost);
            self.g.ent[m].f26 = def.count as i16;
            self.break_cloak(id);
            self.emit_spell(id, m, p, right, ctx);
            return;
        }
        if !self.spell_gate(def) {
            self.g.snd_player(29); // cast-blocked buzz
            return;
        }
        self.mana_debit(def.possess_mana);
        self.g.ent[m].f26 = def.count as i16;
        self.break_cloak(id);
        self.emit_spell(id, m, p, right, ctx);
    }

    /// Casting any other spell breaks the cloak (manual-confirmed;
    /// the +16 0x20 bit clears with the manifestation's burst).
    fn break_cloak(&mut self, casting: usize) {
        if casting != 12 && self.player.invisible {
            self.player.invisible = false;
            let m12 = self.player.owned[12];
            if m12 != 0 {
                self.g.ent[m12 as usize].f26 = 0;
            }
        }
    }

    /// The per-spell one-shot emissions (cite = the traced cast arm).
    fn emit_spell(&mut self, id: usize, m: usize, p: PlayerPose, right: bool, ctx: &MobCtx) {
        let _ = ctx;
        // Launch sounds at the original cast sites (sub_55370 calls
        // against the wizard's own entity — full volume), all ids
        // trace-confirmed 2026-07-06: the 9-family covers fireball/
        // rapid (:65079/:66296), earthquake (:65365), duel (:65665),
        // steal mana (:65764 — the player's possess-soft memory
        // loses to the trace here), undead (:65980), storm (:66039),
        // wall of fire (:66158); meteor/volcano/crater/castle 15
        // (:65422/:65481/:65544/:65914); accelerate 19; teleport 22;
        // lightning 23 (:65852); heal 25; possess 40 (:65252); mana
        // magnet 40 (:66097). Beyond Sight is authentically silent
        // (sub_56730 :65292 — mana gate only).
        if let Some(snd) = match id {
            0 | 23 | 6 | 11 | 13 | 17 | 18 | 20 => Some(9u8),
            7 | 8 | 9 | 16 => Some(15),
            2 | 21 => Some(19),
            10 => Some(22),
            15 => Some(23),
            1 => Some(25),
            3 | 19 => Some(40),
            _ => None,
        } {
            self.g.snd_player(snd);
        }
        match id {
            // 0 Fireball (:65029): edge-triggered single shot (the
            // hold-to-autofire lives on 23 alone).
            0 => self.cast_fireball(p, right, 0),
            // 1 Heal (:65091): continuous — runs in the manifestation
            // tick while the burst is live.
            1 => {}
            // 2/21 Accelerate fwd/back (:65131/:66172): mutually
            // exclusive — activating one force-clears the other's
            // charge (:55871/:55914). While active the spell REPLACES
            // the thrust model ([`World::accel_override`]); the
            // resisting thrust input cancels it
            // ([`World::thrust_cancel`]).
            2 | 21 => {
                let (dir, other) = if id == 2 { (1i8, 21usize) } else { (-1, 2) };
                let om = self.player.owned[other];
                if om != 0 {
                    self.g.ent[om as usize].f26 = 0;
                }
                self.player.accel = dir;
            }
            // Toggles (4 Shield :65266, 5 Beyond Sight :65292,
            // 12 Invisible :65675, 14 Rebound :65774): the flags
            // derive from the live burst in the manifestation tick.
            // 5 TODO: map reveal gating is a map-authenticity item
            // (our map is currently all-seeing).
            4 | 5 | 12 | 14 => {}
            // Projectile spells.
            3 | 6 | 7 | 8 | 9 | 11 | 13 | 15 | 17 | 19 => self.cast_projectile(id, p, right),
            // 10 Teleport (:65554).
            10 => self.cast_teleport(m, p),
            // 16 Create Castle (:65862).
            16 => self.cast_castle(p),
            // 18 Lightning Storm (:65988).
            18 => self.cast_storm(p),
            // 20 Wall of Fire (:66110).
            20 => self.cast_firewall(p, right),
            // 22 Global Death (:66235): PRIMES only — no visible
            // in-game effect; the pulse fires around the carpet at
            // expiry (player-validated). APPROX ~55 ticks (~2s).
            22 => self.player.bomb_timer = Some(55),
            _ => {}
        }
    }

    /// Muzzle placement shared by the hand casts (sub_56090 :65056-)
    /// — 256 units to the casting hand's side, launch height = the
    /// carpet's half-height, reverted when inside terrain.
    fn muzzle(&self, p: PlayerPose, right: bool) -> (u16, u16, i16) {
        use crate::combat::PLAYER_HH;
        let myaw = if right {
            p.heading.wrapping_add(512)
        } else {
            p.heading.wrapping_sub(512)
        } & 0x7FF;
        let mut mz = (p.x, p.y, p.z);
        Gen::polar_step(&mut mz, myaw, 0, 256);
        if self.g.ground_z(mz.0, mz.1) as i16 > p.z {
            mz = (p.x, p.y, p.z); // muzzle inside terrain: revert
        }
        (mz.0, mz.1, p.z.wrapping_add(PLAYER_HH as i16))
    }

    /// The fireball cast (spells 0/23, sub_58240/sub_56090 :65029/
    /// :66296): heading/pitch from the pose, carpet speed inherited.
    fn cast_fireball(&mut self, p: PlayerPose, right: bool, id: usize) {
        let (mx, my, mz) = self.muzzle(p, right);
        let Some(pr) = self.g.spawn_fireball(mx, my, mz) else {
            return;
        };
        let def = &SPELLS[id];
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed; // inherits carpet speed (:65060)
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        e.f30 = p.heading;
        e.f34 = p.heading;
        e.f32 = p.pitch;
        e.f36 = p.pitch;
        // Spell-row +44 (125/50 — vestigial on detonation: the fire
        // effect's own 400 is the fireball's real damage, sub_52B30
        // does not copy +44, :62928-30) and the possess pool onto
        // +140 (deflection economics).
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        self.entities_dirty = true;
    }

    /// The single-projectile spells: spawn the traced class-9 model
    /// from the hand muzzle, owner = the player, carpet speed
    /// inherited (like the fireball), damage = the spell row's.
    fn cast_projectile(&mut self, id: usize, p: PlayerPose, right: bool) {
        let (mx, my, mz) = self.muzzle(p, right);
        let pr = match id {
            // 3 Possess (:65203): c9 m1 lob; detonation claims the
            // nearest mana ball (payload in crate::combat).
            3 => self.g.spawn_spell_lob(1, mx, my, mz),
            // 6 Earthquake (:65314): c9 m2 lob.
            6 => self.g.spawn_spell_lob(2, mx, my, mz),
            // 7 Meteor (:65374): the m3 trail bolt — generic flight
            // with the decorative fire trail; impact = the growing
            // blast ring (f69 below) carrying the row's 10000 (+44 IS
            // copied on the generic path, :62759-72).
            7 => self.g.spawn_trail_bolt(mx, my, mz),
            // 8 Volcano (:65432): c9 m4 down-arc.
            8 => self.g.spawn_spell_lob(4, mx, my, mz),
            // 9 Crater (:65491): c9 m5 down-arc.
            9 => self.g.spawn_spell_lob(5, mx, my, mz),
            // 11 Duel to the Death (:65620): c9 m7 with a tether
            // effect on wizards — INTERIM: no rival wizards exist;
            // the projectile flies and latches nothing.
            11 => self.g.spawn_spell_lob(7, mx, my, mz),
            // 13 Steal Mana (:65711): c9 m8 — the mob steal ball's
            // ported tick (explodes only on wizard-family victims).
            13 => self.g.spawn_seeker(mx, my, mz),
            // 15 Lightning Bolt (:65806): c9 m9, the one-tick beam.
            15 => self.g.spawn_zigzag(mx, my, mz),
            // 17 Undead Army (:65927): c9 m11; skeletons at impact.
            17 => self.g.spawn_spell_lob(11, mx, my, mz),
            // 19 Mana Magnet (:66049): c9 m6; magnet event at impact.
            19 => self.g.spawn_spell_lob(6, mx, my, mz),
            _ => None,
        };
        let Some(pr) = pr else { return };
        let def = &SPELLS[id];
        // APPROX(original per-spell launch pitches, :65579-style):
        // the down-arc terrain spells get a fixed downward bias on
        // the pose pitch (engine pitch positive = down).
        let pitch = match id {
            6 | 8 | 9 => p.pitch.wrapping_add(0x60) & 0x7FF,
            _ => p.pitch,
        };
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed; // carpet speed inherited (:65060)
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        e.f30 = p.heading;
        e.f34 = p.heading;
        e.f32 = pitch;
        e.f36 = pitch;
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        match id {
            // Possess (:65236-52): detonation = the (10,12) ch1
            // claim flash; target-class filter 10 (the dedicated
            // ball/house victim scan), charge 200, doubled extents
            // (sub_39A90 :45917).
            3 => {
                e.f68 = 10;
                e.f69 = 12;
                e.f66 = 10;
                e.f26 = 200;
                e.f80 *= 2;
                e.f82 *= 2;
                e.f84 *= 2;
            }
            // Meteor detonates into the growing fire-ring blast (c10
            // m17): rings of fires along the round SEARCH annuli +
            // its 10000 broadcast over the ring's 10-tick growth
            // (trace-confirmed, sub_25CE0).
            7 => e.f69 = 17,
            // Steal Mana's damage is forced 2000 (:65754), exploding
            // into the m11 steal flash (ch3).
            13 => {
                e.f44 = 2000;
                e.f69 = 25;
            }
            // Player bolts detonate as the hit flash, like the mob
            // zigzags (:63421- endpoint effect 23).
            15 => e.f69 = 23,
            _ => {}
        }
        self.entities_dirty = true;
    }

    /// 10 Teleport (:65554): to the player's castle when one stands
    /// (the authentic anchor); the recast returns to the cast site.
    /// INTERIM (no castle built): the 64-tile LCG hop (0x4000 units,
    /// :65579-81) along a manifestation-LCG yaw.
    fn cast_teleport(&mut self, m: usize, p: PlayerPose) {
        if let Some((rx, ry)) = self.player.teleport_return.take() {
            self.pending_teleport = Some((rx as f32 / 256.0, ry as f32 / 256.0));
        } else {
            self.player.teleport_return = Some((p.x, p.y));
            let dest = if let Some(c) = self.player_castle() {
                (self.g.ent[c].x, self.g.ent[c].y)
            } else {
                let yaw = (self.g.ent_rand(m) & 0x7FF) as u16;
                let mut d = (p.x, p.y, 0i16);
                Gen::polar_step(&mut d, yaw, 0, 0x4000);
                (d.0, d.1)
            };
            self.pending_teleport = Some((dest.0 as f32 / 256.0, dest.1 as f32 / 256.0));
        }
    }

    /// The cast lockout: a castle ball (9,10) or upgrade token
    /// (10,43) still in flight. A STANDING castle no longer locks —
    /// the recast on it is the UPGRADE (:65904-08 morphs the ball
    /// into the token instead of a new castle).
    fn castle_build_lives(&self) -> bool {
        self.g.ent.iter().any(|e| {
            e.flags & 0x400 == 0
                && e.id24 == PLAYER_TARGET
                && ((e.class64 == 9 && e.model65 == 10)
                    || (e.class64 == 10 && e.model65 == 43))
        })
    }

    /// The player's established castle slot (teleport anchor).
    fn player_castle(&self) -> Option<usize> {
        (1..features::POOL).find(|&j| {
            let e = &self.g.ent[j];
            e.class64 == 3 && e.model65 == 2 && e.id24 == PLAYER_TARGET && e.flags & 0x400 == 0
        })
    }

    /// The player's balloon ROSTER (class-3/model-3) for the HUD
    /// balloon panel. The roster WIDTH comes from castle level alone
    /// (`var_52[]` shown 1/2/3 wide — the :27296-314 switch) and is
    /// kept even when balloons are dead: retail draws the [50+width]
    /// glyph regardless and simply skips the bars of invalid entries
    /// (:27335-40). Live owned balloons fill slots in pool order with
    /// (hp_frac = actLife/maxLife, cargo_frac = stored/capacity), both
    /// clamped to [0,1]. No castle → no roster (the marble [54] case,
    /// :27281).
    fn player_balloons(&self, castle: Option<usize>) -> Vec<Option<(f32, f32)>> {
        let level = match castle {
            Some(c) => self.g.ent[c].f26.clamp(0, 255),
            None => return Vec::new(),
        };
        let roster = match level {
            1..=3 => 1,
            4..=5 => 2,
            6.. => 3,
            _ => return Vec::new(),
        };
        let mut out = vec![None; roster];
        let mut k = 0;
        for j in 1..features::POOL {
            if k >= roster {
                break;
            }
            let e = &self.g.ent[j];
            if e.class64 == 3 && e.model65 == 3 && e.id24 == PLAYER_TARGET && e.flags & 0x400 == 0
            {
                let hp = e.act_life.max(0) as f32 / (e.max_life.max(1) as f32);
                let cargo = e.f140.max(0) as f32 / (e.f136.max(1) as f32);
                out[k] = Some((hp.clamp(0.0, 1.0), cargo.clamp(0.0, 1.0)));
                k += 1;
            }
        }
        out
    }

    /// sub_48230 (:56839): the per-tick mana census. The wizard
    /// ceiling (+136) resets to the intrinsic base 1000 (u32_322,
    /// :55031-33) and accumulates the +140 of every CLAIMED (+144)
    /// creature, castle, balloon, mana ball and house (:56860-907);
    /// claimed houses also feed the banked tally (u32_308, :56895);
    /// every counted entity feeds the world total regardless of
    /// owner. Model-40 claim totems are excluded (:56880). Banked =
    /// house tally + own castle stored (the HUD % numerator, :54721).
    fn recompute_mana(&mut self) {
        let mut max = 1000u32;
        let mut houses = 0u32;
        let mut world = 0u32;
        let mut castle_stored = 0u32;
        for j in 1..features::POOL {
            let e = &self.g.ent[j];
            if e.flags & 0x400 != 0 {
                continue;
            }
            if !matches!(
                (e.class64, e.model65),
                (5, _) | (3, 2) | (3, 3) | (10, 39) | (10, 45)
            ) {
                continue;
            }
            let m = e.f140.max(0) as u32;
            world = world.saturating_add(m);
            if e.f144 == PLAYER_TARGET {
                max = max.saturating_add(m);
                if e.class64 == 10 && e.model65 == 45 {
                    houses = houses.saturating_add(m);
                }
                if e.class64 == 3 && e.model65 == 2 {
                    castle_stored = castle_stored.saturating_add(m);
                }
            }
        }
        self.player.mana_max = max;
        self.player.banked = houses.saturating_add(castle_stored);
        self.player.world_mana = world;
        // The castle overflow ejector reads the house tally
        // (sub_47130 :56185-89 — wizext u32_308).
        self.g.banked_houses = houses.min(i32::MAX as u32) as i32;
    }

    /// sub_55DD0 (:64909): the cast gate — the castle ladder first
    /// (a nonzero `castle_req` needs an owned castle STORING at
    /// least that much), then the wizard pool covers the full cost.
    /// The fizzle 29 on failure is the caller's job.
    fn spell_gate(&self, def: &crate::spells::SpellDef) -> bool {
        if self.dev_spells {
            return true;
        }
        if def.castle_req > 0
            && !self
                .player_castle()
                .is_some_and(|c| self.g.ent[c].f140.max(0) as u32 >= def.castle_req)
        {
            return false;
        }
        self.player.mana >= def.possess_mana
    }

    /// sub_55E80 (:64936): the cast debit rides the regen delta —
    /// overwrite it negative, or deepen an already-negative one. The
    /// wizard mana tick applies it next turn and clamps at 0.
    /// (Authored behavior: remc1's maintainer ships this commented
    /// out — a known mis-fix.)
    fn mana_debit(&mut self, cost: u32) {
        if self.dev_spells {
            return;
        }
        let c = cost.min(i32::MAX as u32) as i32;
        if self.player.mana_delta >= 0 {
            self.player.mana_delta = -c;
        } else {
            self.player.mana_delta -= c;
        }
    }

    /// 16 Create Castle (sub_57610 :65862): the class-9 m10 castle
    /// ball from the caster. NO castle standing: target 16 tiles
    /// (4096 units) ahead at ground level (:65894-902), morph =
    /// the (3,2) castle; the flight runs the sub_12F70 placement
    /// scans (launch = silent abort, landing = flip 180 + step back,
    /// then build). Castle standing: the RECAST is the UPGRADE —
    /// the ball flies AT the castle and morphs into the (10,43)
    /// upgrade token instead (+68/69, +146 = castle idx, :65904-08).
    fn cast_castle(&mut self, p: PlayerPose) {
        use crate::combat::PLAYER_HH;
        let z = p.z.wrapping_add(PLAYER_HH as i16);
        let castle = self.player_castle();
        let Some(pr) = self.g.spawn_castle_ball(p.x, p.y, z) else {
            return;
        };
        let tgt = if let Some(c) = castle {
            (self.g.ent[c].x, self.g.ent[c].y, 0i16)
        } else {
            let mut t = (p.x, p.y, 0i16);
            Gen::polar_step(&mut t, p.heading, 0, 4096);
            t
        };
        let def = &SPELLS[16];
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed;
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        // The launch inherits the wizard's AIM — yaw and pitch both
        // (:65913-14 copies +30/+32) — and the flight EASES from it
        // toward the ground target (the playtest-6 "castle ignores
        // up/down aim" fix).
        e.f30 = p.heading;
        e.f32 = p.pitch;
        e.f34 = p.heading;
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        e.dest_x = tgt.0;
        e.dest_y = tgt.1;
        if let Some(c) = castle {
            e.f68 = 10;
            e.f69 = 43;
            e.f146 = c as u16;
        } else {
            e.f68 = 3;
            e.f69 = 2;
        }
        self.entities_dirty = true;
    }

    /// 18 Lightning Storm (sub_579D0 :65988): ONE class-9 m12
    /// carrier launched at the aim (target point 0x4000 ahead;
    /// wizard-homing when rivals exist), becoming the (10,38) storm
    /// cloud on any non-water end — the cloud climbs to ground+1024
    /// and rains 2 bolts/tick for 33 ticks at the spell's 2000.
    fn cast_storm(&mut self, p: PlayerPose) {
        use crate::combat::PLAYER_HH;
        let def = &SPELLS[18];
        let z = p.z.wrapping_add(PLAYER_HH as i16);
        let Some(pr) = self.g.spawn_storm_carrier(p.x, p.y, z) else {
            return;
        };
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed;
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        e.f30 = p.heading;
        e.f34 = p.heading;
        e.f32 = p.pitch;
        e.f36 = p.pitch;
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        e.f68 = 9;
        e.f69 = 9;
        self.entities_dirty = true;
    }

    /// 20 Wall of Fire (sub_57D40 :66110): the class-9 m16 bolt
    /// (fireball sprite, straight at the aim), detonating into the
    /// (10,53) NAPALM cloud — 15 waves of standing flames climbing
    /// 128 units/wave over the impact (the rising fire curtain).
    /// The row's 24464 stays dead weight (sub_53B50 does not copy
    /// +44; the flames' inherited 100/tick is the payload).
    fn cast_firewall(&mut self, p: PlayerPose, right: bool) {
        let (mx, my, mz) = self.muzzle(p, right);
        let Some(pr) = self.g.spawn_firewall_bolt(mx, my, mz) else {
            return;
        };
        let def = &SPELLS[20];
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed;
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        e.f30 = p.heading;
        e.f34 = p.heading;
        e.f32 = p.pitch;
        e.f36 = p.pitch;
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        e.f68 = 10;
        e.f69 = 53;
        self.entities_dirty = true;
    }

    /// 22 Global Death's expiry pulse (:66235; player-validated
    /// semantics): a single 7000 ch0 write in a VERY SMALL radius
    /// (~1.25 tiles — the spell's balance: "you have to be straight
    /// below a dragon to affect it") centered on the CARPET, no
    /// visual, no terrain scorch. A transient unlinked writer event
    /// carries the area-write protocol. The expiry plays the real
    /// explosion (30) at the carpet — the spell's only feedback
    /// (player ground truth: prime silent, blast audible).
    fn bomb_pulse(&mut self, ctx: &MobCtx) {
        self.g.snd_player(30);
        let Some(s) = self.g.new_event() else { return };
        {
            let e = &mut self.g.ent[s];
            e.class64 = 10;
            e.model65 = 41; // transient, never ticked or drawn
            e.flags &= !8;
            e.id24 = PLAYER_TARGET;
            e.x = ctx.px;
            e.y = ctx.py;
            e.z = ctx.pz;
            e.f80 = 320; // APPROX radius pending a trace
            e.f82 = 320;
            e.f84 = 320;
        }
        self.g.area_write(s, 0, SPELLS[22].damage, ctx, false, false);
        self.g.free_entity(s);
    }

    /// Class-12 dispatch: pre-placed JARS wait for pickup; owned
    /// manifestations run their burst countdown + continuous effects.
    fn class12_tick(&mut self, i: usize, ctx: &MobCtx) {
        let t = self.g.ent[i].tick70;
        if t >= MANIFEST_BASE {
            self.manifestation_tick(i, (t - MANIFEST_BASE) as usize);
            return;
        }
        // Death-scattered jars decay (life 200-289, :55545-47); the
        // THING-placed states 0..=2 sit forever.
        if t == DROPPED_JAR {
            self.g.ent[i].f26 -= 1;
            if self.g.ent[i].f26 <= 0 {
                self.g.ent[i].flags |= 0x400;
                self.entities_dirty = true;
                return;
            }
        }
        // Pickup needs a live carpet — the original's dead wizard is
        // out of play (flag 0x20), so the fresh scatter can't be
        // re-vacuumed while lying on it.
        if self.player.state == LifeState::Alive && self.g.player_overlap(i, ctx) {
            self.try_pickup(i);
        }
    }

    /// Jar pickup (:64843-58): flying through an unowned spell's jar
    /// grants it — the SAME entity converts into the manifestation
    /// (class stays 12, the pool slot stays occupied: slot economy),
    /// auto-equipped LEFT (:64855). Owned already: the jar stays —
    /// no duplicate upgrade (:64843).
    /// TODO(jar spell id): model65 carries the spell id per the
    /// off_987DE thunk dispatch; unverified against retail jar data.
    fn try_pickup(&mut self, i: usize) {
        let spell = self.g.ent[i].model65 as usize;
        if spell >= SPELL_COUNT || self.player.owned[spell] != 0 {
            return;
        }
        {
            let e = &mut self.g.ent[i];
            e.tick70 = MANIFEST_BASE + spell as u8;
            e.flags &= !8;
            e.f26 = 0;
            e.f44 = SPELLS[spell].damage.min(u16::MAX as u32) as u16;
        }
        self.player.owned[spell] = i as u16;
        self.player.left = Some(SpellId(spell as u8)); // auto-equip LEFT
        self.entities_dirty = true; // the jar sprite leaves the world
    }

    /// The owned-spell manifestation tick (the class-12 runtime arm):
    /// the burst counter (+48 → f26) decrements once per tick — it is
    /// the refire spacing — and the continuous/toggle effects derive
    /// from it.
    fn manifestation_tick(&mut self, i: usize, spell: usize) {
        if self.g.ent[i].f26 > 0 {
            self.g.ent[i].f26 -= 1;
        }
        let active = self.g.ent[i].f26 > 0;
        match spell {
            // 1 Heal (:65091): while active and the pool covers the
            // possess gate, heal 5% of the life ceiling per tick and
            // pay possess/count per tick of healing.
            1 => {
                self.player.heal_active = active;
                let def = &SPELLS[1];
                if active && (self.dev_spells || self.player.mana >= def.possess_mana) {
                    self.player.life =
                        (self.player.life + PLAYER_LIFE_MAX / 20).min(PLAYER_LIFE_MAX);
                    if !self.dev_spells {
                        self.player.mana -= def.possess_mana / def.count as u32;
                    }
                }
            }
            2 => {
                if !active && self.player.accel == 1 {
                    self.player.accel = 0;
                }
            }
            21 => {
                if !active && self.player.accel == -1 {
                    self.player.accel = 0;
                }
            }
            4 => self.player.shield = active,
            5 => self.player.beyond_sight = active,
            12 => self.player.invisible = active,
            14 => self.player.rebound = active,
            _ => {}
        }
    }

    /// Hand equips (the original's book/quickselect commands
    /// 0x15/0x16, :48717-31): only owned spells take. Public so the
    /// app can apply a book binding IMMEDIATELY while the sim clock is
    /// paused — binding is UI state, not simulation, and the frozen
    /// HUD must still reflect it (player 2026-07-08).
    pub fn equip_hands(&mut self, left: Option<SpellId>, right: Option<SpellId>) {
        if let Some(s) = left
            && (s.0 as usize) < SPELL_COUNT
            && self.player.owned[s.0 as usize] != 0
        {
            self.player.left = Some(s);
        }
        if let Some(s) = right
            && (s.0 as usize) < SPELL_COUNT
            && self.player.owned[s.0 as usize] != 0
        {
            self.player.right = Some(s);
        }
    }

    /// Spellbook/HUD snapshot.
    pub fn loadout(&self) -> LoadoutView {
        let mut owned = [false; SPELL_COUNT];
        let mut cooldown = [0f32; SPELL_COUNT];
        for s in 0..SPELL_COUNT {
            let m = self.player.owned[s] as usize;
            if m != 0 {
                owned[s] = true;
                cooldown[s] =
                    self.g.ent[m].f26.max(0) as f32 / SPELLS[s].count as f32;
            }
        }
        // One castle scan feeds castle/castle_hp/balloons/bindable.
        let castle_slot = self.player_castle();
        // The :26926 bind gate: castle_req (+132) vs the castle's
        // STORED mana (+140). `req == 0` spells are always bindable.
        let castle_stored = castle_slot.map(|c| self.g.ent[c].f140.max(0) as u32);
        let mut bindable = [false; SPELL_COUNT];
        for (s, b) in bindable.iter_mut().enumerate() {
            let req = SPELLS[s].castle_req;
            *b = self.dev_spells
                || req == 0
                || castle_stored.is_some_and(|stored| stored >= req);
        }
        LoadoutView {
            owned,
            left: self.player.left.map(|s| s.0),
            right: self.player.right.map(|s| s.0),
            cooldown,
            mana: if self.dev_spells { self.player.mana_max } else { self.player.mana },
            mana_max: self.player.mana_max,
            banked: self.player.banked,
            world_mana: self.player.world_mana,
            castle: castle_slot.map(|c| {
                let e = &self.g.ent[c];
                (
                    e.f140.max(0) as u32,
                    e.f136.max(0) as u32,
                    e.f26.clamp(0, 255) as u8,
                )
            }),
            balloons: self.player_balloons(castle_slot),
            castle_hp: castle_slot.map(|c| {
                let e = &self.g.ent[c];
                (e.act_life.max(0), e.max_life.max(1))
            }),
            win_pct: self.win_pct,
            completed: self.completed,
            bindable,
        }
    }

    /// The raw types-2/21 factor: 0.0 inactive, ±3.0 while the cast
    /// button is held, ±2.0 channeling after release (:65169/:65175).
    pub fn player_speed_boost(&self) -> f32 {
        self.player.speed_boost
    }

    /// The Accelerate thrust-model OVERRIDE (types 2/21): while
    /// channeling, the spell REPLACES the thrust model — the carpet
    /// is propelled along its facing at factor × normal-full-thrust
    /// speed regardless of thrust input (the original writes the
    /// carpet speed directly; player ground truth: "it propels you
    /// forward at maximum speed and you can't really stop it —
    /// merely trying to slow down cancels the spell"). Some(signed
    /// factor): +3.0/-3.0 with the button held ("hold down the mouse
    /// button to achieve maximum speed"), +2.0/-2.0 after release
    /// until the burst (count 251) drains. None = normal thrust.
    pub fn accel_override(&self) -> Option<f32> {
        (self.player.speed_boost != 0.0).then_some(self.player.speed_boost)
    }

    /// The Accelerate brake-cancel, fed by the sim from the tick's
    /// raw thrust input BEFORE the world turn (manual: "press the
    /// down cursor to cancel"; symmetric for Accelerate Backwards —
    /// the resisting input is the ONE control that works): negative
    /// thrust cancels/vetoes type 2, positive thrust type 21. The
    /// veto also blocks re-triggering for the rest of the tick.
    pub fn thrust_cancel(&mut self, thrust: f32) {
        if thrust < 0.0 {
            self.accel_veto.0 = true;
            if self.player.accel == 1 {
                self.stop_accel(2);
            }
        }
        if thrust > 0.0 {
            self.accel_veto.1 = true;
            if self.player.accel == -1 {
                self.stop_accel(21);
            }
        }
    }

    /// Kill an accelerate channel outright (brake cancel).
    fn stop_accel(&mut self, id: usize) {
        self.player.accel = 0;
        self.player.speed_boost = 0.0;
        let m = self.player.owned[id];
        if m != 0 {
            self.g.ent[m as usize].f26 = 0;
        }
    }

    /// Total ch0 damage the player has taken (a running stat; under
    /// the dev invincibility it is the only ledger).
    pub fn player_damage_taken(&self) -> u64 {
        self.g.player_damage
    }

    /// The mortality snapshot for the app: HUD bar, hit/death
    /// overlays, respawn prompt, castle-under-attack flash.
    pub fn vitals(&self) -> PlayerVitals {
        PlayerVitals {
            life: self.player.life.clamp(0, PLAYER_LIFE_MAX),
            life_max: PLAYER_LIFE_MAX,
            state: self.player.state,
            grace: self.player.grace,
            hit_flash: self.player.hit_flash,
            lost: self.player.lost,
            castle_alert: self.g.castle_alert > 0,
            player_alert: self.g.player_alert > 0,
            balloon_alert: self.g.balloon_alert > 0,
            has_castle: self.player_castle().is_some(),
        }
    }

    /// The death fall is running (class-3 state 2): the sim's mover
    /// suppresses input and integrates [`Self::death_fall_step`].
    pub fn player_falling(&self) -> bool {
        self.player.state == LifeState::Falling
    }

    /// Landed and waiting for Space (class-3 state 3).
    pub fn player_dead(&self) -> bool {
        self.player.state == LifeState::Dead
    }

    /// One tick of death-fall gravity (:55466-72): returns this
    /// tick's vertical delta (engine units, ≤ 0), then accelerates
    /// −2/tick² clamped to −256.
    pub fn death_fall_step(&mut self) -> i16 {
        let v = self.player.fall_speed;
        self.player.fall_speed = (v - 2).clamp(-256, 0);
        v
    }

    /// The killer's position in tile units (the death camera turns
    /// toward it, sub_463B0 :55575-91).
    pub fn killer_pos(&self) -> Option<(f32, f32)> {
        let k = self.player.killer as usize;
        if k != 0 && k < features::POOL && self.g.ent[k].class64 != 0 {
            let e = &self.g.ent[k];
            Some((e.x as f32 / 256.0, e.y as f32 / 256.0))
        } else {
            None
        }
    }

    /// A respawn fired this tick: destination in tile units. The sim
    /// moves the carpet there and resets the flight state.
    pub fn take_respawn(&mut self) -> Option<(f32, f32)> {
        self.pending_respawn.take()
    }

    /// Castle-less death confirmed: the app restarts the level (the
    /// original's lost + level-over flow).
    pub fn take_restart(&mut self) -> bool {
        std::mem::take(&mut self.pending_restart)
    }

    /// Test hook: zero the grace and hand the player a lethal hit
    /// from nothing (killer 0 — no death-camera target).
    #[doc(hidden)]
    pub fn debug_kill_player(&mut self) {
        self.player.grace = 0;
        self.g.player_mail[0] = (u32::MAX / 4, 1);
    }

    /// Dev/accessibility invincibility (the pre-mortality behavior).
    pub fn set_invincible(&mut self, on: bool) {
        self.invincible = on;
    }

    /// Combat stat counters: (kills, shots resolved, aimed hits) —
    /// the original's Type_160 +359/+343/+347.
    pub fn combat_stats(&self) -> (u32, u32, u32) {
        (self.g.kills, self.g.shots, self.g.hits)
    }

    /// The village-aggro ("wanted") timer — remaining ticks of
    /// militia hostility toward the player (+528 semantics).
    pub fn player_aggro(&self) -> i16 {
        self.g.player_aggro
    }

    // ---- dispositions ----------------------------------------------------

    /// sub_37440_37800 (:43924): spawn every live THING whose dis_id
    /// matches; one-shot consumes the records. (The disId-0 mana
    /// recount is the mana track's concern and omitted.)
    fn fire_disposition(&mut self, dis: u16, one_shot: bool) {
        for i in 1..2000usize.min(self.table.len()) {
            if self.table[i].class != 0 && self.table[i].dis_id == dis {
                self.spawn_from_thing(i);
                if one_shot {
                    self.table[i].class = 0;
                }
            }
        }
    }

    /// sub_37560_37920 (:43988): spawn one THING record as a pool
    /// event, with the original's per-class post-initialization.
    fn spawn_from_thing(&mut self, ti: usize) {
        let r = self.table[ti];
        // Entity records only (markers/junk never spawn).
        if r.x >= 256 || r.y >= 256 {
            return;
        }
        let x = (r.x << 8).wrapping_add(128);
        let y = (r.y << 8).wrapping_add(128);
        let z = self.g.ground_z(x, y) as i16;

        let slot = match r.class {
            2 => self.g.spawn_scenery(r.model, x, y, z),
            3 => self.g.spawn_class3(r.model, x, y, z),
            5 => self.g.spawn_creature(r.model, x, y, z),
            10 => self.g.spawn_creator(r.model, x, y, z),
            11 => self.spawn_trigger(r.model, x, y, z),
            7 | 9 | 12 => self.spawn_inert(r.class, r.model, x, y, z),
            _ => None,
        };
        let Some(s) = slot else { return };
        self.g.ent[s].thing_slot = ti as u16;
        if r.class == 11 {
            // Trigger volumes feed the map overlay, not billboards.
            self.entities_dirty = true;
        }

        // Post-init (:44017-44050). NOTE the original's branch shape:
        // classes BELOW 11 get nothing except the class-10 models 4
        // (spawner volume), 34 (portal) and 45 (building); exactly
        // class 11 gets id24/extents; class 12 the state bump.
        match (r.class, r.model) {
            (12, _) => {
                // byte70 += swi_id; >= 3 = the village-owned jar
                // variant (-3, sprite 280 written straight to +86).
                let e = &mut self.g.ent[s];
                e.tick70 = e.tick70.wrapping_add((r.swi_id & 0xFF) as u8);
                if r.swi_id >= 3 {
                    e.tick70 = e.tick70.wrapping_sub(3);
                    e.type86 = 280;
                    e.flags |= 0x40000; // +18 |= 4
                }
            }
            (10, 4) => {
                self.g.ent[s].id24 = r.swi_id;
                self.g.extents(s, r.swi_sz << 8, r.swi_sz << 8);
                self.g.refill_life(s);
            }
            // Portal destination (:44024): +150/+152 from the THING's
            // data_16/data_14 (our child/parent), tile centers.
            (10, 34) => {
                let e = &mut self.g.ent[s];
                e.dest_x = (r.child << 8).wrapping_add(128);
                e.dest_y = (r.parent << 8).wrapping_add(128);
            }
            (10, 45) => {
                self.g.building_fixup(s, r.parent.wrapping_add(16));
            }
            (11, _) => {
                self.g.ent[s].id24 = r.swi_id;
                self.g.extents(s, r.swi_sz << 8, 4096);
                self.g.refill_life(s);
                self.g.ent[s].flags |= 1;
            }
            _ => {}
        }

        if drawable(r.class, r.model) {
            self.entities_dirty = true;
        }
    }

    /// sub_3BB20 (:47771): a class-11 trigger volume event. State =
    /// model; extents arrive from the post-init.
    fn spawn_trigger(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.g.new_event()?;
        let e = &mut self.g.ent[s];
        e.class64 = 11;
        e.model65 = model as u8;
        e.tick70 = model as u8;
        e.f26 = 0;
        e.flags = (e.flags & !0x9) | 1;
        e.x = x;
        e.y = y;
        e.z = z;
        self.g.refill_life(s);
        Some(s)
    }

    /// A drawable/latent entity as an inert pool event — the classes
    /// whose real spawn handlers belong to later tracks (7 = spawner
    /// logic, 9 = spell effects, 12 = mana pickups). Authored class-9
    /// things park OUT of the flight-state range so they never tick
    /// as live projectiles.
    fn spawn_inert(&mut self, class: u16, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.g.new_event()?;
        self.g.ent[s].class64 = class as u8;
        self.g.ent[s].model65 = model as u8;
        self.g.ent[s].tick70 = if class == 9 { 0xFE } else { 0 };
        self.g.link(s, x, y, z);
        self.g.refill_life(s);
        self.g.ent[s].flags |= 1;
        if class == 12 {
            // Interim type for the pose/billboard layer (the real
            // class-12 spawner sub_3BF70 is the mana track's port).
            self.g.set_sprite(s, 77);
        }
        Some(s)
    }

    fn free_slot(&mut self, i: usize) {
        if drawable(self.g.ent[i].class64 as u16, self.g.ent[i].model65 as u16)
            || self.g.ent[i].class64 == 11
        {
            self.entities_dirty = true; // a drawable/overlay entity left
        }
        self.g.free_entity(i);
    }

    // ---- class-11 trigger ticking (str_256038, :4921) ---------------------

    fn trigger_tick(&mut self, i: usize, player: PlayerPose, buckets: &[u32; 20]) {
        match self.g.ent[i].tick70 {
            // One-shot proximity: fire when a wizard balloon is inside
            // (polarity 1) / outside (polarity 0) the volume.
            0 | 5 | 9 => self.one_shot(i, player, true),
            1 | 6 | 10 => self.one_shot(i, player, false),
            // Repeating proximity with a 10-tick rearm that waits for
            // the player to leave (:67249).
            2 | 7 | 11 => self.repeating(i, player, true),
            3 | 8 | 12 => self.repeating(i, player, false),
            // State 4: fires when the player carries a collected item
            // (:67293) — stub until inventory exists.
            4 => {}
            // States 13..=29: class-5 bucket 0..=16 empty for 16
            // ticks; state 30: buckets 0..=11 and 16 all empty.
            s @ 13..=29 => self.kill_trigger(i, Some((s - 13) as usize), buckets),
            30 => self.kill_trigger(i, None, buckets),
            _ => {}
        }
    }

    /// sub_5A090_5A5A0 (:67632): the wizard-balloon AABB probe,
    /// throttled to every 8th tick; on a quiet probe the volume's z
    /// follows the (possibly re-dug) ground. For us the balloon list
    /// is the player's carpet (AI wizards are a later track).
    fn balloon_probe(&mut self, i: usize, player: PlayerPose, want: bool) -> bool {
        if self.g.ent[i].f63 & 7 != 0 {
            return false;
        }
        if self.overlap(i, player) == want {
            return true;
        }
        let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
        self.g.ent[i].z = self.g.ground_z(x, y) as i16;
        false
    }

    /// sub_118C0 (:16963): both entities' extents SUM per axis, and
    /// each z is centered by its half-height (+78). The player carpet
    /// carries sprite 44's stats halves (spawn sub_378A0), replacing
    /// the earlier point-extent stub — the suspect in the portal-entry
    /// feel note.
    fn overlap(&self, i: usize, p: PlayerPose) -> bool {
        const PW: i32 = (SPRITE_STATS[44].width / 2) as i32;
        const PH: i32 = (SPRITE_STATS[44].height / 2) as i32;
        let e = &self.g.ent[i];
        let wrap_d = |a: u16, b: u16| {
            let d = (a as i32 - b as i32) & 0xFFFF;
            (d as i16 as i32).abs()
        };
        wrap_d(p.x, e.x) < e.f80 as i32 + PW
            && wrap_d(p.y, e.y) < e.f82 as i32 + PW
            && ((e.z as i32 + e.f78 as i32) - (p.z as i32 + PH)).abs() < e.f84 as i32 + PH
    }

    fn one_shot(&mut self, i: usize, player: PlayerPose, want: bool) {
        if self.balloon_probe(i, player, want) {
            let dis = self.g.ent[i].id24;
            self.fire_disposition(dis, true);
            self.g.ent[i].flags |= 0x400;
        }
    }

    fn repeating(&mut self, i: usize, player: PlayerPose, want: bool) {
        if self.g.ent[i].f26 != 0 {
            // Rearm countdown: only ticks down while the player probe
            // misses (:67254 — the player must leave the volume).
            if self.overlap(i, player) != want {
                self.g.ent[i].f26 -= 1;
            }
        } else if self.balloon_probe(i, player, want) {
            let dis = self.g.ent[i].id24;
            self.fire_disposition(dis, false);
            self.g.ent[i].f26 = 10;
        }
    }

    /// sub_26A60 (:29170), class-10 state 36: the portal vortex. A
    /// timed portal counts down actLife (authored ones carry 0 = stays
    /// forever); a player overlapping the 1-tile volume while FACING
    /// it (heading within 170/2048 of the bearing to the portal, i.e.
    /// you fly INTO the vortex) is moved to the destination point. The
    /// portal's altitude follows the ground each tick.
    fn portal_tick(&mut self, i: usize, player: PlayerPose) {
        let life = self.g.ent[i].act_life;
        if life > 0 {
            self.g.ent[i].act_life = life - 1;
            if life == 1 {
                self.g.ent[i].flags |= 0x400;
                return;
            }
        }
        if self.overlap(i, player) {
            let e = &self.g.ent[i];
            let bearing = Gen::angle_of(
                Gen::wrap_delta(e.x as i16, player.x as i16) as i16,
                Gen::wrap_delta(e.y as i16, player.y as i16) as i16,
            );
            let d = player.heading.wrapping_sub(bearing) & 0x7FF;
            if d.min(2048 - d) < 0xAA {
                let (dx, dy) = (self.g.ent[i].dest_x, self.g.ent[i].dest_y);
                self.pending_teleport =
                    Some((dx as f32 / 256.0, dy as f32 / 256.0));
                // PORTUSE — the same 22 as the teleport spell
                // (player-confirmed gap, playtest 3).
                self.g.snd_player(22);
            }
        }
        // Follow the ground; the pose consumer must see the drop from
        // the +640 spawn altitude (and any later re-dig under the
        // portal) even on levels with no creatures ticking.
        let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
        let ground = self.g.ground_z(x, y) as i16;
        if self.g.ent[i].z != ground {
            self.g.ent[i].z = ground;
            self.entities_dirty = true;
        }
    }

    /// Consume this tick's portal teleport, if one fired: destination
    /// in world tile units (x, z).
    pub fn take_teleport(&mut self) -> Option<(f32, f32)> {
        self.pending_teleport.take()
    }

    /// Drain this tick's sound requests plus the ambient-loop inputs
    /// the original's player tick derives (:55254-82): waves XOR wind
    /// from the terrain under the carpet, fire and market loops from
    /// emitter proximity. The original refreshes per-player countdown
    /// fields from the emitters' own handlers; the INTERIM probe here
    /// is a direct radius scan (8 tiles) over live fires (class 10
    /// m0/m6) and village houses (m45) — same audible result, exact
    /// hysteresis owed with the emitter trace.
    pub fn take_audio(&mut self, player: PlayerPose) -> AudioFrame {
        let over_water = self.g.on_water_pub(player.x, player.y);
        const AMBIENT_RANGE: i32 = 8 * 256;
        let (mut fire_near, mut market_near) = (false, false);
        for e in &self.g.ent {
            if e.flags & 1 == 0 || e.flags & 0x400 != 0 || e.class64 != 10 {
                continue;
            }
            let is_fire = matches!(e.model65, 0 | 6);
            let is_house = e.model65 == 45 && e.act_life >= 0;
            if !is_fire && !is_house {
                continue;
            }
            let dx = i32::from(e.x.wrapping_sub(player.x) as i16).abs();
            let dy = i32::from(e.y.wrapping_sub(player.y) as i16).abs();
            if dx.max(dy) > AMBIENT_RANGE {
                continue;
            }
            if is_fire {
                fire_near = true;
            } else {
                market_near = true;
            }
        }
        AudioFrame {
            events: std::mem::take(&mut self.g.sounds),
            over_water,
            fire_near,
            market_near,
            danger: self.g.player_danger > 0,
        }
    }

    /// Live gameplay volumes (trigger AABBs, portals) for the map
    /// debug/enhancement overlay: position + radius in tile units.
    pub fn active_volumes(&self) -> Vec<ActiveVolume> {
        let mut out = Vec::new();
        for e in &self.g.ent {
            let kind = match (e.class64, e.tick70) {
                (11, 0..=3 | 5..=12) => VolumeKind::Proximity,
                (11, 4) => VolumeKind::Inventory,
                (11, 13..=30) => VolumeKind::KillWatch,
                (10, 36) => VolumeKind::Portal,
                _ => continue,
            };
            out.push(ActiveVolume {
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                radius: (e.f80 as f32 / 256.0).max(0.5),
                kind,
            });
        }
        out
    }

    /// sub_59E40_5A350 (:67460): fire one-shot after the watched
    /// class-5 bucket(s) stay empty through a 16-tick countdown; a
    /// non-empty probe pauses (does not reset) the countdown.
    fn kill_trigger(&mut self, i: usize, list: Option<usize>, buckets: &[u32; 20]) {
        let empty = match list {
            Some(k) => buckets.get(k).copied().unwrap_or(0) == 0,
            // The -1 variant: buckets 0..=11 and 16.
            None => (0..=11).chain([16]).all(|k| buckets[k] == 0),
        };
        if !empty {
            return;
        }
        match self.g.ent[i].f26 {
            0 => self.g.ent[i].f26 = 16,
            1 => {
                let dis = self.g.ent[i].id24;
                self.fire_disposition(dis, true);
                self.g.ent[i].flags |= 0x400;
            }
            _ => self.g.ent[i].f26 -= 1,
        }
    }

    /// The flyer-side wall gate (sub_45410 :55065) in tile units:
    /// `from`/`to` = (x, z_map, altitude). Returns the position the
    /// move actually reaches — `to` unchanged when no wall is hit, a
    /// cardinal wall-slide otherwise — or None when both slides are
    /// blocked and the whole move is discarded. Type-8 walls block the
    /// player at ANY altitude.
    pub fn player_wall_gate(
        &self,
        from: (f32, f32, f32),
        to: (f32, f32, f32),
    ) -> Option<(f32, f32, f32)> {
        let fixed = |x: f32, z: f32, alt: f32| {
            (
                (x.rem_euclid(256.0) * 256.0) as u16,
                (z.rem_euclid(256.0) * 256.0) as u16,
                (alt * 256.0) as i16,
            )
        };
        let cur = fixed(from.0, from.1, from.2);
        let prop = fixed(to.0, to.1, to.2);
        let out = self.g.player_wall_gate(cur, prop)?;
        if out == prop {
            // Untouched move: hand back the caller's floats verbatim
            // (no 8.8 quantization outside collisions).
            return Some(to);
        }
        Some((
            out.0 as f32 / 256.0,
            out.1 as f32 / 256.0,
            out.2 as f32 / 256.0,
        ))
    }

    /// This tick's forced knock displacement on the player (the
    /// kraken buffet; later, hit knockback): Type_160 v_22/v_24
    /// consumed like the human move does (:55204-218) — magnitude
    /// clamped to 128, applied, then decayed 4/tick and snapped to 0
    /// below |4|. Returns (11-bit direction, engine units) or None at
    /// rest. The kraken re-arms 80 every ON tick of its 41/91 duty
    /// cycle, so the pull only bleeds off in the OFF phase.
    pub fn take_knock_step(&mut self) -> Option<(u16, i16)> {
        let (dir, mag) = self.g.player_knock;
        if mag == 0 {
            return None;
        }
        let mag = mag.clamp(-128, 128);
        let mut next = mag - mag.signum() * 4;
        if next.abs() < 4 {
            next = 0;
        }
        self.g.player_knock = (dir, next);
        Some((dir, mag))
    }

    /// The live knock magnitude (for the app's camera pitch kick:
    /// the original view drops by ~v_22/8 engine-angle units,
    /// :52433-37).
    pub fn knock_magnitude(&self) -> i16 {
        self.g.player_knock.1
    }

    /// Ground height in ENGINE units at an 8.8 position (the faithful
    /// mover's terrain probe — sub_11F50's triangle interpolation).
    pub fn ground_z_engine(&self, x: u16, y: u16) -> i16 {
        self.g.ground_z(x, y) as i16
    }

    /// The sub_45410 wall gate in engine units (the faithful mover
    /// applies the routine's trailing z-floor itself).
    pub fn player_wall_gate_fixed(
        &self,
        cur: (u16, u16, i16),
        prop: (u16, u16, i16),
    ) -> Option<(u16, u16, i16)> {
        self.g.player_wall_gate(cur, prop)
    }

    /// Emit a player-anchored sound from the sim boundary (the move's
    /// wind-gust flutter, remc1 :55294-99).
    pub fn push_player_sound(&mut self, id: u8) {
        self.g.snd_player(id);
    }

    /// The level's highest terrain tile in tile units, from the LIVE
    /// height plane (terrain is runtime-mutable). The extended-lift
    /// float-up cap anchors here so explicit lift can never reach a
    /// god's-eye view (player directive, 2026-07-07).
    pub fn max_ground_tiles(&self) -> f32 {
        let max = self.g.t.height.iter().copied().max().unwrap_or(0);
        max as f32 * crate::HEIGHT_SCALE
    }

    /// Ground height in tile units at world-space tile coordinates
    /// (for the flyer's terrain clamp against the LIVE planes).
    pub fn ground_height_tiles(&self, x: f32, z: f32) -> f32 {
        let xi = (x.rem_euclid(256.0) * 256.0) as u16;
        let zi = (z.rem_euclid(256.0) * 256.0) as u16;
        self.g.ground_z(xi, zi) as f32 / 256.0
    }

    /// Pool diagnostics (debug tooling; the level-032 chain-stall
    /// investigation): free slot count + a minimal live-event view.
    #[doc(hidden)]
    pub fn debug_pool(&self) -> (usize, Vec<DebugEvent>) {
        let free = self.g.free.len();
        let ev = self
            .g
            .ent
            .iter()
            .enumerate()
            .filter(|(_, e)| e.class64 != 0)
            .map(|(slot, e)| DebugEvent {
                slot,
                class: e.class64,
                model: e.model65,
                state: e.tick70,
                id24: e.id24,
                tx: (e.x >> 8) as u8,
                ty: (e.y >> 8) as u8,
                life: e.act_life,
            })
            .collect();
        (free, ev)
    }

    /// Copy the live planes into a caller's `TerrainPlanes` view (the
    /// renderer's update path).
    pub fn copy_planes_into(&self, out: TerrainPlanes<'_>) {
        out.height.copy_from_slice(&self.g.t.height);
        out.tile_type.copy_from_slice(&self.g.t.tile_type);
        out.shading.copy_from_slice(&self.g.t.shading);
        out.angle.copy_from_slice(&self.g.t.angle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::tile;

    /// Level-005-shaped micro-world: a proximity trigger that fires a
    /// disposition spawning an expanding crater + a creature.
    fn micro_things() -> Vec<Thing> {
        let th = |slot, class, model, x, y, dis_id, swi_sz, swi_id| Thing {
            slot,
            kind: ThingKind::Entity,
            class,
            model,
            x,
            y,
            dis_id,
            swi_sz,
            swi_id,
            parent: 0,
            child: 0,
            par3: None,
        };
        vec![
            // Trigger at (100,100), radius 3 tiles, fires disposition 1.
            th(0, 11, 0, 100, 100, 0, 3, 1),
            // Expanding crater (model 11) behind disposition 1, radius 4.
            th(1, 10, 11, 110, 110, 1, 4, 1),
            // A creature behind disposition 1.
            th(2, 5, 2, 112, 110, 1, 0, 1),
        ]
    }

    fn assets() -> FeatureAssets {
        // Diamond rings like features::tests::synthetic_assets.
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
        // Plain floor (7) with a wall ring (0x10) like real rows —
        // the collapse rubble stamp only marks WALL cells (:30944).
        let mut dat = Vec::new();
        for row in 0..4 {
            dat.push(4u8);
            if row == 1 || row == 2 {
                dat.extend_from_slice(&[0x10, 7, 7, 0x10]);
            } else {
                dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            }
            dat.push(0);
        }
        FeatureAssets::parse(&grid, &tab, &dat).unwrap()
    }

    fn flat_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        World::new(planes, &micro_things(), 1, assets())
    }

    fn away() -> PlayerPose {
        PlayerPose::from_tiles(10.0, 105.0 / 8.0, 10.0, 0.0, 0.0, 0.0)
    }

    fn at_trigger() -> PlayerPose {
        PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0)
    }

    #[test]
    fn walls_gate_the_player_slide_block_and_corner() {
        // A north-south type-8 wall line at tile x=120, plus an east-
        // west line at tile y=101 forming a corner.
        let mut planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        for y in 0..=255 {
            planes.tile_type[tile(120, y)] = 8;
        }
        for x in 0..=255 {
            planes.tile_type[tile(x, 101)] = 8;
        }
        let w = World::new(planes, &[], 1, assets());

        // Oblique approach hugging the wall: the eastward cardinal
        // slide still lands on the wall tile, the southward one
        // succeeds — the player skims along the wall without crossing.
        let slid = w
            .player_wall_gate((119.95, 99.5, 12.5), (120.85, 99.9, 12.5))
            .expect("oblique move slides");
        assert!(slid.0 < 120.0, "never crosses the wall line, x={}", slid.0);
        assert!(slid.1 > 99.5, "slides south along the wall, z={}", slid.1);

        // Farther out, the blocked-cardinal retry shortens the move
        // toward the wall instead (authentic: the scaled slide along
        // the move's own cardinal lands short of the wall tile).
        let short = w
            .player_wall_gate((119.2, 99.5, 12.5), (120.1, 99.9, 12.5))
            .expect("shortened approach");
        assert!(short.0 < 120.0 && short.0 > 119.2, "shortened, x={}", short.0);

        // Head-on at high altitude (way above the wall's +48 crest):
        // the aligned cardinal contributes a zero-length slide — the
        // move is voided in place. Walls block at ANY altitude.
        let stuck = w
            .player_wall_gate((119.5, 99.5, 30.0), (120.5, 99.5, 30.0))
            .expect("head-on voids in place");
        assert!(stuck.0 < 120.0, "altitude does not bypass, x={}", stuck.0);

        // Diagonal into the inside corner: both cardinal slides land
        // on wall tiles — the whole move is discarded.
        assert!(
            w.player_wall_gate((119.8, 100.8, 12.5), (120.2, 101.2, 12.5))
                .is_none(),
            "corner discards the whole move"
        );

        // No wall involved: the move passes through bit-identical.
        let free = w
            .player_wall_gate((110.0, 99.0, 12.5), (110.3, 99.2, 12.5))
            .expect("free move");
        assert_eq!(free, (110.3, 99.2, 12.5));
    }

    #[test]
    fn the_flyer_never_crosses_a_wall() {
        let mut planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        for y in 0..=255 {
            planes.tile_type[tile(120, y)] = 8;
            planes.height[tile(120, y)] = 148; // the wall's +48 crest
        }
        let w = World::new(planes, &[], 1, assets());
        let mut sim = crate::Simulation::with_world(w);
        sim.flyer.x = 117.0;
        sim.flyer.z = 99.5;
        sim.flyer.y = 30.0; // far above the crest
        sim.flyer.yaw = std::f32::consts::FRAC_PI_2; // facing +x (east)
        sim.flyer.pitch = 0.0;
        sim.sync_carpet_from_flyer(); // flyer set directly → re-seed
        let thrust = crate::FlightInput { thrust: 1.0, ..Default::default() };
        for _ in 0..600 {
            sim.step(&thrust);
            assert!(sim.flyer.x < 120.0, "wall crossed at x={}", sim.flyer.x);
        }
        assert!(sim.flyer.x > 119.0, "the flyer did reach the wall, x={}", sim.flyer.x);
    }

    #[test]
    fn deferred_things_stay_latent_until_triggered() {
        let mut w = flat_world();
        assert_eq!(w.live_things().len(), 0, "dis_id!=0 things must not spawn at init");
        for _ in 0..64 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.live_things().len(), 0);
        let center = tile(110, 110);
        assert_eq!(w.planes().height[center], 100, "crater must not dig while latent");
    }

    #[test]
    fn proximity_trigger_fires_disposition_and_crater_digs() {
        let mut w = flat_world();
        // Fly into the volume; the probe is throttled to every 8th
        // tick, so give it a few.
        for _ in 0..16 {
            w.tick(at_trigger(), PlayerCommand::default());
        }
        let live = w.live_things();
        assert_eq!(live.len(), 1, "the creature spawns via the disposition");
        assert_eq!((live[0].class, live[0].model), (5, 2));
        // The expanding crater digs -3 per covered ring per tick.
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        let center = tile(110, 110);
        assert!(
            w.planes().height[center] < 100,
            "crater dug: height {} at center",
            w.planes().height[center]
        );
        assert!(w.terrain_dirty);
        // One-shot: the records are consumed, the trigger is gone.
        let n = w.live_things().len();
        for _ in 0..32 {
            w.tick(at_trigger(), PlayerCommand::default());
        }
        assert_eq!(w.live_things().len(), n, "one-shot trigger must not refire");
    }

    #[test]
    fn creatures_wander_when_awake() {
        let mut w = flat_world();
        // Fire the trigger so the (5,2) creature spawns; the player
        // stays nearby, keeping it awake.
        for _ in 0..16 {
            w.tick(at_trigger(), PlayerCommand::default());
        }
        let start = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 5)
            .expect("creature spawned");
        for _ in 0..200 {
            w.tick(at_trigger(), PlayerCommand::default());
        }
        let now = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 5)
            .expect("creature alive");
        assert!(
            (now.x - start.x).abs() + (now.z - start.z).abs() > 0.05,
            "an awake creature wanders: {:?} -> {:?}",
            (start.x, start.z),
            (now.x, now.z)
        );
    }

    #[test]
    fn water_contains_a_grounded_creature() {
        // One land tile in an ocean: the movement core's terrain mask
        // (row 10 forbids water) must keep a villager on its island —
        // same-tile steps stay free, crossings are blocked.
        let mut planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![0; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        planes.tile_type[tile(100, 100)] = 5;
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 5,
            model: 12,
            x: 100,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        // Player adjacent: awake, jitter-walking every tick.
        let p = PlayerPose::from_tiles(101.5, 14.0, 101.5, 0.0, 0.0, 0.0);
        for t in 0..400 {
            w.tick(p, PlayerCommand::default());
            let pose = w
                .live_poses()
                .into_iter()
                .find(|q| q.class == 5)
                .expect("villager alive");
            assert_eq!(
                (pose.x.floor(), pose.z.floor()),
                (100.0, 100.0),
                "tick {t}: creature left its island: ({}, {})",
                pose.x,
                pose.z
            );
        }
    }

    #[test]
    fn worm_segments_trail_the_head() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 5,
            model: 0,
            x: 100,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let heads: Vec<_> = w.live_poses().into_iter().filter(|p| !p.segment).collect();
        let segs: Vec<_> = w.live_poses().into_iter().filter(|p| p.segment).collect();
        assert_eq!(heads.len(), 1, "one worm head");
        assert_eq!(segs.len(), 16, "sixteen body segments");
        assert_eq!(w.live_things().len(), 1, "segments hidden from entity lists");

        let p = PlayerPose::from_tiles(101.5, 14.0, 101.5, 0.0, 0.0, 0.0);
        for _ in 0..60 {
            w.tick(p, PlayerCommand::default());
        }
        let head = w
            .live_poses()
            .into_iter()
            .find(|p| !p.segment)
            .expect("head alive");
        let segs: Vec<_> = w.live_poses().into_iter().filter(|p| p.segment).collect();
        // Awake movement strings the body out: the first segment sits
        // its follow distance behind the head, not on it.
        let d0 = (segs[0].x - head.x).abs() + (segs[0].z - head.z).abs();
        assert!(
            d0 > 0.05,
            "segment 0 trails the head (offset {d0}, head at {:?})",
            (head.x, head.z)
        );
        let distinct: std::collections::HashSet<_> = segs
            .iter()
            .map(|s| ((s.x * 256.0) as i32, (s.z * 256.0) as i32))
            .collect();
        assert!(
            distinct.len() > 8,
            "segments spread out ({} distinct positions)",
            distinct.len()
        );
    }

    #[test]
    fn asleep_crowds_do_not_pack_and_accelerate() {
        // Regression (player-reported runaway worms/bees): WANDER's
        // scans are awake-gated in the original — a distant crowd
        // must never form packs and ride the unbounded pack accel.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let bee = |slot, x, y| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 5,
            model: 1,
            x,
            y,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let things: Vec<Thing> =
            (0..8).map(|k| bee(k, 100 + (k % 3) as u16, 100 + (k / 3) as u16)).collect();
        let mut w = World::new(planes, &things, 1, assets());
        // Player far away the whole time (> 24 tiles: asleep).
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        for _ in 0..3000 {
            w.tick(far, PlayerCommand::default());
        }
        let before: Vec<_> = w.live_poses();
        w.tick(far, PlayerCommand::default());
        let after: Vec<_> = w.live_poses();
        // Bee speed = 50 engine units/tick ≈ 0.195 tiles; pack
        // catch-up adds a bounded +16 per chain level. The compounding
        // mis-fix reached many tiles per tick and kept growing —
        // anything near a tile/tick means it is back.
        for (b, a) in before.iter().zip(&after) {
            let d = (a.x - b.x).abs().min(256.0 - (a.x - b.x).abs())
                + (a.z - b.z).abs().min(256.0 - (a.z - b.z).abs());
            assert!(
                d < 1.0,
                "asleep bee moved {d} tiles in one tick (speed ran away)"
            );
        }
    }

    #[test]
    fn burrower_materializes_then_hides() {
        // m9's spawn sequence (sub_1CFF0): flame form 220 → transform
        // animation 237 → the type-201 lurking mound at state 55.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 5,
            model: 9,
            x: 100,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let near = PlayerPose::from_tiles(102.5, 14.0, 102.5, 0.0, 0.0, 0.0);
        let mut seen = Vec::new();
        for _ in 0..80 {
            w.tick(near, PlayerCommand::default());
            let t = w.live_poses()[0].type_index;
            if seen.last() != Some(&t) {
                seen.push(t);
            }
        }
        assert_eq!(seen, vec![220, 237, 201], "materialize sequence");
    }

    #[test]
    fn deterministic_across_runs() {
        let run = || {
            let mut w = flat_world();
            for t in 0..200 {
                let p = if (40..80).contains(&t) { at_trigger() } else { away() };
                w.tick(p, PlayerCommand::default());
            }
            (w.planes().height.clone(), w.live_things().len())
        };
        assert_eq!(run(), run());
    }

    // ---- combat ------------------------------------------------------------

    /// Directly south of the combat worlds' creature (112,110),
    /// facing north (engine yaw 0 = -y): the fireball's line of fire.
    fn firing_line() -> PlayerPose {
        PlayerPose::level((112 << 8) + 128, (116 << 8) + 128, 3360, 0)
    }

    fn count(w: &World, class: u8, model: u8) -> usize {
        w.debug_pool()
            .1
            .iter()
            .filter(|e| e.class == class && e.model == model)
            .count()
    }

    /// Combat-test loadout: the Rapid Fireball firehose (23) on the
    /// left hand with the dev mana pin — the only hold-to-autofire
    /// spell, giving the 1-projectile-per-held-tick cadence the
    /// combat tests were written against. Invincibility pins the OLD
    /// dev-player semantics these tests assume (damage totaled from
    /// tick 0, no death mid-fight); mortality has its own tests.
    fn rapid_fire(w: &mut World) {
        w.set_dev_spells(true);
        w.set_invincible(true);
        w.player.left = Some(crate::spells::SpellId(23));
    }

    /// A flat world holding one load-time creature and nothing else —
    /// no crater rims for a chaser to wall-death on.
    fn bare_creature_world(model: u16) -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 5,
            model,
            x: 112,
            y: 110,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        World::new(planes, &things, 7, assets())
    }

    // ---- mortality (the 2026-07-07 track) -------------------------------

    /// A landed pose: ground 100*32 = 3200, the touchdown floor is
    /// ground+128 = 3328 (firing_line's 3360 stays airborne).
    fn grounded_line() -> PlayerPose {
        PlayerPose::level((112 << 8) + 128, (116 << 8) + 128, 3328, 0)
    }

    fn hit_player(w: &mut World, amt: u32, src: u16) {
        w.g.mail_write(crate::combat::MailTarget::Player, 0, amt, src);
    }

    #[test]
    fn spawn_grace_absorbs_then_real_damage_knocks_and_kills() {
        let mut w = bare_creature_world(2);
        w.set_dev_spells(true); // all spells owned → a real jar scatter
        // Park the creature far away; it stays alive as the damage
        // SOURCE entity (the knockback bearing needs its position).
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);

        // 1) The grace window: hits are wiped, not totaled (:55367-71).
        for _ in 0..50 {
            hit_player(&mut w, 500, 1);
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert_eq!(w.vitals().life, PLAYER_LIFE_MAX, "grace = total immunity");
        assert_eq!(w.player_damage_taken(), 0, "grace does not total");
        for _ in 0..50 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert_eq!(w.vitals().grace, 0, "the 100-tick grace expired");

        // 2) A real hit: life drops, knockback arms (amt/10 clamp 80),
        // the red flash and the regen stall run.
        hit_player(&mut w, 500, 1);
        w.tick(firing_line(), PlayerCommand::default());
        assert_eq!(w.vitals().life, PLAYER_LIFE_MAX - 500);
        assert_eq!(w.knock_magnitude(), 50, "v_22 = amount/10");
        assert!(w.vitals().hit_flash > 0, "the red flash armed");

        // 3) Regen: /2000 afield = 5/tick once the 16-tick stall ends.
        for _ in 0..20 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        let healed = w.vitals().life;
        assert!(healed > PLAYER_LIFE_MAX - 500, "afield regen ticked");
        assert!(healed < PLAYER_LIFE_MAX, "but nowhere near full yet");

        // 4) A lethal hit: the death fall begins.
        hit_player(&mut w, 30000, 1);
        w.tick(firing_line(), PlayerCommand::default());
        assert_eq!(w.vitals().state, LifeState::Falling);
        assert!(w.player_falling());

        // 5) Touchdown at ground+128: jars scatter, the grave rises,
        // the player-owned loose ball passes to the grave.
        let b = w.g.spawn_mana_ball((112 << 8) + 128, (114 << 8) + 128, 3200).unwrap();
        w.g.ent[b].f144 = PLAYER_TARGET;
        w.tick(grounded_line(), PlayerCommand::default());
        assert_eq!(w.vitals().state, LifeState::Dead);
        assert_eq!(count(&w, 10, 40), 1, "the grave stands");
        let grave = w
            .debug_pool()
            .1
            .into_iter()
            .find(|e| e.class == 10 && e.model == 40)
            .unwrap()
            .slot as u16;
        assert_eq!(w.g.ent[b as usize].f144, grave, "the grave inherits the ball");
        let jars = w
            .debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 12 && e.state == 3)
            .count();
        assert!(jars > 0, "the spell inventory scattered as decaying jars");

        // 6) Castle-less respawn = the level is lost and restarts.
        w.tick(grounded_line(), PlayerCommand { respawn: true, ..Default::default() });
        assert!(w.take_restart(), "castle-less death restarts the level");
        assert!(w.vitals().lost);
    }

    #[test]
    fn death_with_a_castle_respawns_there_with_fresh_grace() {
        let mut w = bare_creature_world(2);
        w.set_dev_spells(true);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let c = w.g.spawn_castle((140 << 8) + 128, (140 << 8) + 128).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..60 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert!(w.loadout().castle.is_some(), "castle established");

        w.player.grace = 0;
        hit_player(&mut w, 30000, 1);
        w.tick(firing_line(), PlayerCommand::default());
        w.tick(grounded_line(), PlayerCommand::default());
        assert_eq!(w.vitals().state, LifeState::Dead);
        let owned_before = w.loadout().owned.iter().filter(|&&o| o).count();
        assert_eq!(owned_before, 0, "ownership rides the death slots while dead");

        w.tick(grounded_line(), PlayerCommand { respawn: true, ..Default::default() });
        let (rx, rz) = w.take_respawn().expect("respawn fired");
        // The castle grid-snaps to even tile parity; just confirm the
        // destination is the castle's tile neighborhood.
        assert!((rx - 140.0).abs() < 2.0 && (rz - 140.0).abs() < 2.0);
        assert_eq!(w.vitals().state, LifeState::Alive);
        assert_eq!(w.vitals().life, PLAYER_LIFE_MAX);
        assert_eq!(w.vitals().grace, 100, "fresh spawn grace");
        assert!(!w.take_restart(), "no restart with a castle standing");
        let owned_after = w.loadout().owned.iter().filter(|&&o| o).count();
        assert!(owned_after >= 24, "the spell inventory re-instantiated");
    }

    #[test]
    fn castle_transformation_kills_the_footprint_but_spares_the_exempt() {
        let mut w = bare_creature_world(2); // wild lunger at ~(113,110)
        // An owned creature and a boss-exempt m16 on the footprint.
        let owned = w.g.spawn_creature(2, (112 << 8), (110 << 8), 3200).unwrap();
        w.g.ent[owned].id24 = PLAYER_TARGET;
        let boss = w.g.spawn_creature(16, (111 << 8), (110 << 8), 3200).unwrap();
        let wild_life = w.g.ent[1].act_life;
        assert!(wild_life > 0);

        // The castle rises straight under them (the level-0 build
        // skips the space gate — the initial cast is single-step).
        let c = w.g.spawn_castle((112 << 8), (110 << 8)).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..40 {
            w.tick(PlayerPose::level((90 << 8), (90 << 8), 3400, 0), PlayerCommand::default());
        }
        assert_eq!(count(&w, 5, 2) , 1, "exactly one m2 survives...");
        assert_eq!(w.g.ent[owned].id24, PLAYER_TARGET);
        assert!(w.g.ent[owned].act_life > 0, "...the OWNED one (owner immunity)");
        assert!(w.g.ent[boss].act_life > 0, "m16 is exempt from the execution");
        let (kills, _, _) = w.combat_stats();
        assert_eq!(kills, 1, "the execution credits the castle owner");
    }

    #[test]
    fn castle_downgrade_ejects_mana_and_demolish_razes() {
        let mut w = bare_creature_world(2);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let pose = PlayerPose::level((90 << 8), (90 << 8), 3400, 0);
        let c = w.g.spawn_castle((140 << 8), (140 << 8)).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..60 {
            w.tick(pose, PlayerCommand::default());
        }
        // Promote to level 2 through the authentic ch5 upgrade mail.
        w.g.ent[c].mail[5] = (10, PLAYER_TARGET);
        for _ in 0..60 {
            w.tick(pose, PlayerCommand::default());
        }
        let (_, cap, lvl) = w.loadout().castle.expect("castle stands");
        assert_eq!((lvl, cap), (2, 20_000));

        // Bank mana, then overkill it: one level down, the overkill
        // carries (capped at half), the spill flies out as balls.
        w.g.ent[c].f140 = 30_000;
        w.g.mail_write(crate::combat::MailTarget::Pool(c), 0, 45_000, 1);
        w.tick(pose, PlayerCommand::default());
        let (_, cap, lvl) = w.loadout().castle.expect("downgraded, not dead");
        assert_eq!((lvl, cap), (1, 10_000), "one level per lethal event");
        assert_eq!(
            w.g.ent[c].act_life, 15_000,
            "20000 max minus the 5000 overkill carry"
        );
        assert!(count(&w, 10, 39) >= 2, "the spill scattered as mana balls");
        assert_eq!(count(&w, 10, 54), 4, "the four collapse magnets");

        // Let the repaint cycle finish, then demolish: level 1 → 0
        // = total destruction, castle-less.
        for _ in 0..80 {
            w.tick(pose, PlayerCommand::default());
        }
        w.tick(pose, PlayerCommand { demolish: true, ..Default::default() });
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        assert!(w.loadout().castle.is_none(), "the demolish razed it");
        assert_eq!(count(&w, 3, 2), 0, "the entity is gone");
    }

    /// Playtest-6 orphaned-tower regression: a lethal (here the
    /// demolish key) landing MID-TRANSFORMATION must defer until the
    /// castle is established — the original's standing tick is the
    /// only damage processor. Processing it under a live painter
    /// collapsed the footprint while the painter kept painting,
    /// leaving castle terrain with no castle.
    #[test]
    fn demolish_during_the_build_defers_until_established() {
        let mut w = bare_creature_world(2);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let pose = PlayerPose::level((90 << 8), (90 << 8), 3400, 0);
        let c = w.g.spawn_castle((140 << 8), (140 << 8)).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        // Two ticks in: the painter is mid-flight, the castle waits.
        w.tick(pose, PlayerCommand::default());
        w.tick(pose, PlayerCommand::default());
        assert_eq!(w.g.ent[c].f59, 1, "mid-transformation wait state");
        w.tick(pose, PlayerCommand { demolish: true, ..Default::default() });
        assert!(w.g.ent[c].act_life < 0, "the lethal is pending");
        // The transformation runs to completion untouched...
        for _ in 0..10 {
            w.tick(pose, PlayerCommand::default());
            if w.g.ent[c].flags & 0x400 != 0 || w.g.ent[c].class64 != 3 {
                panic!("the castle died mid-transformation");
            }
        }
        // ...and the deferred lethal razes it once established
        // (level 1 → destruction).
        for _ in 0..80 {
            w.tick(pose, PlayerCommand::default());
        }
        assert_eq!(count(&w, 3, 2), 0, "processed at establishment");
        // No orphaned painters/levelers keep running afterwards.
        assert_eq!(count(&w, 10, 42) + count(&w, 10, 41), 0);
    }

    #[test]
    fn fireball_kills_and_the_corpse_drops_a_mana_ball() {
        let mut w = bare_creature_world(2);
        rapid_fire(&mut w);
        assert_eq!(count(&w, 5, 2), 1, "the creature spawned");
        // Hold fire from the firing line: the aim assist locks on,
        // the fire's 400-damage broadcast whittles the 3000 life.
        let fire = PlayerCommand { fire_left: true, ..Default::default() };
        let mut died_at = None;
        for t in 0..600 {
            w.tick(firing_line(), fire);
            if count(&w, 5, 2) == 0 {
                died_at = Some(t);
                break;
            }
        }
        assert!(died_at.is_some(), "the creature dies under fire");
        // The corpse dropped its mana ball (life/2 = 1500 mana).
        for _ in 0..16 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert!(count(&w, 10, 39) >= 1, "a mana ball dropped");
        // Ball size class by mana (sub_274D0): the lunger's 1500
        // (life/2) lands in class 3 → sprite type 55.
        let ball = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 10 && p.model == 39)
            .expect("ball pose");
        assert_eq!(ball.type_index, 55, "1500 mana = size class 3");
        let (kills, shots, _hits) = w.combat_stats();
        assert_eq!(kills, 1, "the kill credits the player");
        assert!(shots > 0, "shots were resolved");
    }

    #[test]
    fn hit_creatures_aggro_and_maul_the_invincible_player() {
        let mut w = bare_creature_world(2);
        rapid_fire(&mut w);
        // A three-tick burst wounds the lunger without killing it
        // (≤ 1200 of 3000 life)...
        for _ in 0..3 {
            w.tick(firing_line(), PlayerCommand { fire_left: true, ..Default::default() });
        }
        // ...then it chases the wizard-family attacker and melees.
        // The invincible player discards the damage but the total
        // records what would have killed you.
        for _ in 0..1500 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 5, 2), 1, "the wounded lunger survives");
        assert!(
            w.player_damage_taken() > 0,
            "the chaser's melee lands in the discarded inbox"
        );
    }

    #[test]
    fn worm_chain_dies_from_the_head_and_every_corpse_drops() {
        let mut w = bare_creature_world(0);
        rapid_fire(&mut w);
        assert_eq!(count(&w, 5, 0), 17, "head + 16 segments");
        let fire = PlayerCommand { fire_left: true, ..Default::default() };
        let mut cleared = false;
        for _ in 0..3000 {
            w.tick(firing_line(), fire);
            if count(&w, 5, 0) == 0 {
                cleared = true;
                break;
            }
        }
        assert!(cleared, "the whole chain dies (segments corpse with the head)");
        for _ in 0..16 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert!(
            count(&w, 10, 39) >= 1,
            "segment corpses dropped mana balls (merged or not)"
        );
        let (kills, _, _) = w.combat_stats();
        assert_eq!(kills, 1, "one worm, one kill");
    }

    /// A slot of the given class/model from the live pool.
    fn find_slot(w: &World, class: u8, model: u8) -> usize {
        w.debug_pool()
            .1
            .iter()
            .find(|e| e.class == class && e.model == model)
            .map(|e| e.slot)
            .expect("entity present")
    }

    #[test]
    fn village_building_survives_pops_militia_and_collapses() {
        use crate::combat::MailTarget;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 10,
            model: 45,
            x: 110,
            y: 110,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 3, assets());
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        // The house persists past construction at runtime (regression:
        // state 52 used to fall into the load loop's self-kill arm).
        assert_eq!(count(&w, 10, 45), 1, "the house persists");
        let b = find_slot(&w, 10, 45);
        // Give it extra occupants so non-lethal hits pop militia out
        // (fresh houses hold the floor of 2 — nobody spare).
        w.g.ent[b].f26 = 4;
        // Five 400-damage hits: 2000 life reaches exactly 0 — standing.
        for _ in 0..5 {
            w.g.mail_write(MailTarget::Pool(b), 0, 400, PLAYER_TARGET);
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 10, 45), 1, "still standing at 0 life");
        // Spare occupants pop out as militia on non-lethal hits (they
        // may mill back in — the walk-in door is live too, so counts
        // fluctuate; at least one is outside right after the barrage).
        assert!(count(&w, 5, 4) >= 1, "militia popped out under fire");
        assert!(w.player_aggro() > 0, "hitting the village flags the wizard");
        // The killing blow → collapse: everyone left evacuates, the
        // LAST one out is a settler, rubble is stamped.
        w.g.mail_write(MailTarget::Pool(b), 0, 400, PLAYER_TARGET);
        for _ in 0..3 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 10, 45), 0, "collapsed");
        assert!(count(&w, 5, 12) >= 1, "the last occupant out is a settler");
        let rubble = (108u8..=113)
            .any(|x| (108u8..=113).any(|y| w.planes().angle[tile(x, y)] & 7 == 1));
        assert!(rubble, "collapse stamps the rubble angle nibble");
    }

    #[test]
    fn balloon_collects_claimed_mana_to_the_castle() {
        let mut w = flat_world();
        // A player castle; let the build chain (level-up → painter →
        // leveler) run to established.
        let c = w.g.spawn_castle(110 << 8, 110 << 8).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..80 {
            w.tick(away(), PlayerCommand::default());
        }
        let (_, cap, lvl) = w.loadout().castle.expect("castle stands");
        assert_eq!((lvl, cap), (1, 10_000), "level 1 on the capacity ladder");
        // A claimed ball 4 tiles out: the dispatcher's balloon must
        // fetch it and empty the cargo into the castle store.
        let b = w.g.spawn_mana_ball(114 << 8, 110 << 8, 100 * 32).unwrap();
        w.g.ent[b].f140 = 512;
        w.g.ent[b].f144 = PLAYER_TARGET;
        let mut stored = 0;
        for _ in 0..600 {
            w.tick(away(), PlayerCommand::default());
            stored = w.loadout().castle.map_or(0, |(s, _, _)| s);
            if stored >= 512 {
                break;
            }
        }
        assert!(stored >= 512, "balloon delivered the cargo (stored {stored})");
        // One more tick: the census (tick-start) sees the delivery.
        w.tick(away(), PlayerCommand::default());
        // Castle-stored mana raises the wizard ceiling and counts as
        // banked (sub_48230).
        assert!(w.loadout().mana_max >= 1000 + 512, "ceiling includes the store");
        assert!(w.loadout().banked >= 512, "banked = castle stored");
    }

    #[test]
    fn castle_upgrade_costs_the_full_ladder_amount() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.grant_spell(SpellId(16));
        w.player.left = Some(SpellId(16));
        // A standing level-1 castle (build chain runs to established).
        let c = w.g.spawn_castle(110 << 8, 110 << 8).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..80 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.loadout().castle.map(|(_, _, l)| l), Some(1));
        // Recast with the bare 1000 pool: SILENT no-cast — the
        // upgrade costs the full ladder amount at the current level
        // (10000 at level 1; sub_47C60/sub_47DD0 rewrite the
        // manifestation's +136, gate :55908-10).
        let fire = PlayerCommand { fire_left: true, ..Default::default() };
        w.tick(away(), fire);
        assert_eq!(count(&w, 9, 10), 0, "pool 1000 cannot fund the 10000 upgrade");
        // Own enough mana (a claimed ball raises the ceiling) and
        // the same recast launches the upgrade ball.
        let b = w.g.spawn_mana_ball(50 << 8, 50 << 8, 100 * 32).unwrap();
        w.g.ent[b].f140 = 20_000;
        w.g.ent[b].f144 = PLAYER_TARGET;
        w.tick(away(), PlayerCommand::default()); // census + release
        w.player.mana = w.player.mana_max;
        w.tick(away(), fire);
        assert_eq!(count(&w, 9, 10), 1, "funded upgrade launches the ball");
    }

    #[test]
    fn trees_burn_to_char_and_spark_a_standing_fire() {
        use crate::combat::MailTarget;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 2,
            model: 0,
            x: 112,
            y: 110,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 3, assets());
        w.tick(away(), PlayerCommand::default());
        let t = find_slot(&w, 2, 0);
        // A fireball's fire (400) fells the 300-life tree.
        w.g.mail_write(MailTarget::Pool(t), 0, 400, PLAYER_TARGET);
        w.tick(away(), PlayerCommand::default());
        assert_eq!(count(&w, 10, 6), 1, "the standing fire ignites");
        // The flame must also DRAW — riding 3/4 up the trunk, above
        // the ground plane (the 2026-07-07 burning-tree report: the
        // fire existed but (10,6) was missing from the drawables).
        let flame = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 10 && p.model == 6)
            .expect("the standing fire is a drawable");
        let ground = w.ground_height_tiles(flame.x, flame.z);
        assert!(
            flame.alt > ground + 0.1,
            "the flame rides the trunk: alt {} ground {}",
            flame.alt,
            ground
        );
        for _ in 0..260 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 2, 0), 1, "the charred husk remains");
        let husk = w
            .debug_pool()
            .1
            .into_iter()
            .find(|e| e.class == 2)
            .unwrap();
        assert_eq!(husk.state, 2, "burned down to the char state");
        assert_eq!(count(&w, 10, 6), 0, "the fire burned out");
    }

    #[test]
    fn a_settler_builds_a_second_house_and_settles_as_a_villager() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let th = |slot, class, model, x, y| Thing {
            slot,
            kind: ThingKind::Entity,
            class,
            model,
            x,
            y,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let things = vec![th(0, 10, 45, 110, 110), th(1, 5, 12, 113, 110)];
        let mut w = World::new(planes, &things, 3, assets());
        let mut second_house_at = None;
        for t in 0..1200 {
            w.tick(away(), PlayerCommand::default());
            if count(&w, 10, 45) >= 2 {
                second_house_at = Some(t);
                break;
            }
        }
        assert!(
            second_house_at.is_some(),
            "the settler seeks the house and builds a second one"
        );
        // Construction completes and the settler has retired into the
        // villager-feeder state (model stays 12; dispatch is by state).
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 10, 45), 2, "the new house stands");
        let settler = w
            .debug_pool()
            .1
            .into_iter()
            .find(|e| e.class == 5 && e.model == 12);
        assert!(
            settler.is_none_or(|s| s.state >= 79),
            "the builder settled into the villager chain (or moved in)"
        );
    }

    #[test]
    fn kraken_beam_lays_segments_and_the_buffet_arms_the_knock() {
        let mut w = bare_creature_world(6);
        rapid_fire(&mut w);
        assert_eq!(count(&w, 5, 6), 3, "kraken head + 2 segments");
        // Aggro the kraken with a short burst; it closes in, arms its
        // 5-beam bursts and the 41-tick buffet phases.
        for _ in 0..3 {
            w.tick(firing_line(), PlayerCommand { fire_left: true, ..Default::default() });
        }
        let (mut saw_segments, mut knocked) = (false, false);
        for _ in 0..2000 {
            w.tick(firing_line(), PlayerCommand::default());
            // State-14 chain segments are class-9 m9 entities besides
            // the (already dead) one-tick beam.
            saw_segments |= count(&w, 9, 9) > 1;
            knocked |= w.knock_magnitude() > 0;
            if saw_segments && knocked && w.player_damage_taken() > 0 {
                break;
            }
        }
        assert!(saw_segments, "beams lay state-14 segment chains");
        assert!(knocked, "the buffet arms the player knock fields");
        assert!(
            w.player_damage_taken() > 0,
            "beam endpoint detonations land ch0 damage on the player"
        );
    }

    #[test]
    fn knock_step_clamps_decays_and_snaps() {
        let mut w = flat_world();
        w.g.player_knock = (512, 200);
        // Over-strength knocks clamp to 128 before applying
        // (:55207-08), then decay 4/tick from the clamped value.
        assert_eq!(w.take_knock_step(), Some((512, 128)));
        assert_eq!(w.take_knock_step(), Some((512, 124)));
        // Below |4| the remainder snaps to zero (:55217-18).
        w.g.player_knock = (512, 7);
        assert_eq!(w.take_knock_step(), Some((512, 7)));
        assert_eq!(w.take_knock_step(), None);
    }

    #[test]
    fn crab_eats_the_mana_grid_and_grows() {
        // A crab (m5) amid a grid of authored loose mana balls: it
        // must hunt them down, absorb their mana and grow through the
        // 185+N sprite sizes (sub_1C170 + sub_38820).
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let th = |slot, class, model, x, y| Thing {
            slot,
            kind: ThingKind::Entity,
            class,
            model,
            x,
            y,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let mut things = vec![th(0, 5, 5, 112, 110)];
        for (k, (dx, dy)) in [(2, 0), (0, 2), (-2, 0), (0, -2), (2, 2), (-2, -2)]
            .iter()
            .enumerate()
        {
            things.push(th(
                k as u32 + 1,
                10,
                39,
                (112 + dx) as u16,
                (110 + dy) as u16,
            ));
        }
        let mut w = World::new(planes, &things, 7, assets());
        assert_eq!(count(&w, 10, 39), 6, "the mana grid spawned");
        let far = PlayerPose::level(10 << 8, 10 << 8, 3360, 0);
        for _ in 0..6000 {
            w.tick(far, PlayerCommand::default());
            if count(&w, 10, 39) == 0 {
                break;
            }
        }
        assert_eq!(count(&w, 10, 39), 0, "the crab ate every ball");
        let crab = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 5 && p.model == 5)
            .expect("crab alive");
        assert!(
            crab.type_index > 185,
            "the crab grew (sprite {}, expected > 185)",
            crab.type_index
        );
    }

    // ---- player spells ------------------------------------------------------

    #[test]
    fn casting_fireball_spawns_a_projectile_and_deducts_mana() {
        let mut w = flat_world();
        let lv = w.loadout();
        assert!(lv.owned[0] && lv.owned[3], "starting spells granted");
        assert_eq!((lv.left, lv.right), (Some(0), Some(3)), "auto-fill L/R (:49246-54)");
        let fire = PlayerCommand { fire_left: true, ..Default::default() };
        w.tick(firing_line(), fire);
        assert_eq!(count(&w, 9, 0), 1, "the press edge casts one fireball");
        // Ceiling = the intrinsic 1000 with nothing claimed
        // (sub_48230); the FULL 200 debit rides the regen delta and
        // lands NEXT tick (sub_55E80 — the debit remc1 comments out).
        assert_eq!(w.loadout().mana, 1000, "debit is delta-deferred");
        assert!(w.loadout().cooldown[0] > 0.0, "burst window armed");
        w.tick(firing_line(), fire); // held: no re-cast
        assert_eq!(w.loadout().mana, 800, "the full 200 debit landed");
        // Fireball is EDGE-triggered (autofire is spell 23's alone):
        // holding adds nothing; a fresh press past the burst does.
        for _ in 0..8 {
            w.tick(firing_line(), fire);
        }
        assert_eq!(
            count(&w, 9, 0) + w.combat_stats().1 as usize,
            1,
            "held fire never re-casts"
        );
        w.tick(firing_line(), PlayerCommand::default()); // release
        w.tick(firing_line(), fire); // re-press
        let total = count(&w, 9, 0) + w.combat_stats().1 as usize;
        assert_eq!(total, 2, "release + re-press casts again");
        // Below the possess gate nothing fires.
        let mut w2 = flat_world();
        w2.player.mana = 100;
        w2.tick(firing_line(), fire);
        assert_eq!(count(&w2, 9, 0), 0, "mana-gated cast");
    }

    #[test]
    fn jar_pickup_grants_auto_equips_and_keeps_the_slot() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        // A class-12 jar carrying spell 7 (Meteor) via its model.
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 12,
            model: 7,
            x: 112,
            y: 116,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        assert!(!w.loadout().owned[7]);
        assert_eq!(w.live_poses().iter().filter(|p| p.class == 12).count(), 1);
        let (free0, _) = w.debug_pool();
        // Fly onto the jar (ground 100*32 = 3200 engine units).
        let on_jar = PlayerPose::level((112 << 8) + 128, (116 << 8) + 128, 3260, 0);
        for _ in 0..4 {
            w.tick(on_jar, PlayerCommand::default());
        }
        let lv = w.loadout();
        assert!(lv.owned[7], "the jar granted its spell");
        assert_eq!(lv.left, Some(7), "auto-equipped LEFT (:64855)");
        let (free1, _) = w.debug_pool();
        assert_eq!(free0, free1, "the manifestation keeps the jar's pool slot");
        assert!(
            w.live_poses().iter().all(|p| p.class != 12),
            "the picked-up jar no longer renders"
        );
        // Re-overlap: no duplicate, the manifestation stays (:64843).
        for _ in 0..4 {
            w.tick(on_jar, PlayerCommand::default());
        }
        assert_eq!(w.debug_pool().0, free1);
    }

    #[test]
    fn accelerate_directions_are_mutually_exclusive() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        // Toggle semantics under test, not the economy — the real
        // pool (base 1000) can't fund back-to-back 1000-cost arms.
        w.set_dev_spells(true);
        let equip = PlayerCommand {
            equip_left: Some(SpellId(2)),
            equip_right: Some(SpellId(21)),
            ..Default::default()
        };
        w.tick(away(), equip);
        assert_eq!(w.accel_override(), None, "no override at rest");
        // Forward: ±3.0 while the button is held ("hold down the
        // mouse button to achieve maximum speed").
        let fwd = PlayerCommand { fire_left: true, ..Default::default() };
        w.tick(away(), fwd);
        assert_eq!(w.accel_override(), Some(3.0), "held = 3.0 (:65169)");
        w.tick(away(), fwd);
        assert_eq!(w.accel_override(), Some(3.0), "still held = still 3.0");
        // Released: the channel keeps propelling at 2.0.
        w.tick(away(), PlayerCommand::default());
        assert_eq!(w.accel_override(), Some(2.0), "released = 2.0 channel");
        // Opposite activation force-clears forward (:55871/:55914).
        let back = PlayerCommand { fire_right: true, ..Default::default() };
        w.tick(away(), back);
        assert_eq!(w.player.accel, -1, "backward took over");
        assert_eq!(w.accel_override(), Some(-3.0), "negative held backward");
        let m2 = w.player.owned[2] as usize;
        assert_eq!(w.g.ent[m2].f26, 0, "forward's charge force-cleared");
        // The resisting thrust input cancels instantly (manual: the
        // down cursor; forward thrust for the backward spell).
        w.thrust_cancel(1.0);
        assert_eq!(w.accel_override(), None, "brake input kills the channel");
        // The veto also blocks re-triggering within the same tick.
        w.tick(away(), back);
        assert_eq!(w.accel_override(), None, "vetoed re-trigger");
        // Next tick (no veto) it channels again, then drains after
        // release (count 251) back to no override.
        w.tick(away(), back);
        assert_eq!(w.accel_override(), Some(-3.0));
        for _ in 0..252 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.accel_override(), None, "expired burst drops the override");
        assert_eq!(w.player_speed_boost(), 0.0);
    }

    #[test]
    fn lightning_bolt_streams_while_held() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(15));
        // Hold = continuous stream (manual), paced by count 2: the
        // one-tick beams resolve immediately into player shots.
        let fire = PlayerCommand { fire_left: true, ..Default::default() };
        for _ in 0..10 {
            w.tick(firing_line(), fire);
        }
        let (_, shots, _) = w.combat_stats();
        assert!(shots >= 4, "held bolt streams (got {shots})");
    }

    #[test]
    fn global_death_primes_then_pulses_point_blank() {
        use crate::spells::SpellId;
        let mut w = bare_creature_world(2); // wandering lunger, 3000 life
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(22));
        // Hover right on top of the creature WHEREVER it wanders: the
        // pulse's tiny radius ("straight below a dragon") demands it.
        let over = |w: &World| {
            let c = w
                .debug_pool()
                .1
                .into_iter()
                .find(|e| e.class == 5 && e.model == 2)
                .expect("creature alive");
            PlayerPose::level(
                ((c.tx as u16) << 8) + 128,
                ((c.ty as u16) << 8) + 128,
                3300,
                0,
            )
        };
        let p = over(&w);
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        // Primed: NO visible effect, no damage for ~2 seconds.
        for _ in 0..50 {
            let p = over(&w);
            w.tick(p, PlayerCommand::default());
        }
        assert_eq!(count(&w, 5, 2), 1, "nothing happens while primed");
        // The pulse lands a few ticks later; the death/corpse
        // pipeline takes a few dozen more.
        for _ in 0..60 {
            if count(&w, 5, 2) == 0 {
                break;
            }
            let p = over(&w);
            w.tick(p, PlayerCommand::default());
        }
        assert_eq!(count(&w, 5, 2), 0, "the expiry pulse one-shots point-blank");
    }

    #[test]
    fn earthquake_trench_travels_forward() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(6));
        // Fire north from the firing line: the lob impacts a few
        // tiles ahead, then the walker digs onward tile by tile.
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        for _ in 0..120 {
            w.tick(p, PlayerCommand::default());
        }
        let dug: usize = (80..=113u8)
            .filter(|&y| w.planes().height[tile(112, y)] < 100)
            .count();
        assert!(dug >= 5, "the quake trench travels north ({dug} dug rows)");
    }

    #[test]
    fn meteor_detonates_into_the_blast_ring() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(7));
        // Aim steeply down so the bolt grounds fast.
        let mut p = firing_line();
        p.pitch = 0x100;
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        let mut saw_ring = false;
        for _ in 0..40 {
            w.tick(p, PlayerCommand::default());
            saw_ring |= count(&w, 10, 17) > 0;
        }
        assert!(saw_ring, "meteor impact = the growing fire-ring blast");
    }

    #[test]
    fn volcano_erupts_periodically_after_the_cone() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(8));
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        let (mut saw_driver, mut saw_lava, mut saw_plume) = (false, false, false);
        for _ in 0..400 {
            w.tick(p, PlayerCommand::default());
            saw_driver |= count(&w, 10, 18) > 0;
            // The traced chain: ballistic (10,16) lava bombs + the
            // (10,19) plume during the ~127-tick eruption window.
            saw_lava |= count(&w, 10, 16) > 0;
            saw_plume |= count(&w, 10, 19) > 0;
        }
        assert!(saw_driver, "the cone finish spawned the eruption driver");
        assert!(saw_lava, "the eruption window launches ballistic lava bombs");
        assert!(saw_plume, "the eruption start raises the (10,19) plume");
        // FINITE: the window is over — no live bombs remain hundreds
        // of ticks past it (bomb life caps at 199).
        assert_eq!(count(&w, 10, 16), 0, "eruption activity ended");
    }

    #[test]
    fn possess_homes_on_and_claims_a_mana_ball() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(3));
        let p = firing_line();
        // A loose ball ~6 tiles dead ahead (heading 0 = -y) on the
        // aim line, at ground level.
        let (bx, by) = ((112u16 << 8) + 128, (110u16 << 8) + 128);
        let gz = w.g.ground_z(bx, by) as i16;
        let b = w.g.spawn_mana_ball(bx, by, gz).unwrap();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        assert_eq!(count(&w, 9, 1), 1, "the possess lob launched");
        let mut claimed = false;
        for _ in 0..120 {
            w.tick(p, PlayerCommand::default());
            claimed |= w.g.ent[b].f144 == PLAYER_TARGET;
        }
        assert!(claimed, "the m1 lob acquires + the (10,12) flash claims the ball");
    }

    #[test]
    fn possess_claims_a_neutral_house() {
        use crate::spells::SpellId;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 10,
            model: 45,
            x: 112,
            y: 110,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 3, assets());
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(3));
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        let b = find_slot(&w, 10, 45);
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        assert_eq!(count(&w, 9, 1), 1, "the possess lob launched");
        let mut claimed = false;
        for _ in 0..120 {
            w.tick(p, PlayerCommand::default());
            claimed |= w.g.ent[b].f144 == PLAYER_TARGET;
        }
        assert!(claimed, "the lob claims the neutral house (:30800-14)");
    }

    #[test]
    fn lightning_storm_cloud_rains_bolts() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(18));
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        assert_eq!(count(&w, 9, 12), 1, "the storm carrier launched");
        let (mut saw_cloud, mut bolts) = (false, 0usize);
        for _ in 0..80 {
            w.tick(p, PlayerCommand::default());
            saw_cloud |= count(&w, 10, 38) > 0;
            bolts += count(&w, 9, 9);
        }
        assert!(saw_cloud, "the carrier became the (10,38) storm cloud");
        // 2 bolts/tick over 33 firing ticks once on station — the
        // per-tick census over-counts long-lived segments, so just
        // demand a real rain, not a fan.
        assert!(bolts > 20, "the cloud rained bolts over time (saw {bolts})");
    }

    #[test]
    fn wall_of_fire_erupts_the_napalm_curtain() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(20));
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        assert_eq!(count(&w, 9, 16), 1, "the firewall bolt launched");
        let (mut saw_cloud, mut saw_flames) = (false, false);
        for _ in 0..80 {
            w.tick(p, PlayerCommand::default());
            saw_cloud |= count(&w, 10, 53) > 0;
            saw_flames |= count(&w, 10, 6) > 2;
        }
        assert!(saw_cloud, "impact spawned the (10,53) napalm cloud");
        assert!(saw_flames, "the cloud waves standing flames over the ring");
    }

    #[test]
    fn undead_army_raises_owned_skeletons() {
        use crate::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(17));
        let p = firing_line();
        w.tick(p, PlayerCommand { fire_left: true, ..Default::default() });
        let mut skeletons = 0usize;
        for _ in 0..80 {
            w.tick(p, PlayerCommand::default());
            skeletons = skeletons.max(count(&w, 5, 9));
        }
        assert_eq!(skeletons, 8, "8 skeletons on the ring");
        for e in w.debug_pool().1.iter().filter(|e| e.class == 5 && e.model == 9) {
            assert_eq!(e.id24, PLAYER_TARGET, "owner-tagged: never attacks the caster");
        }
    }

    #[test]
    fn dev_spells_grants_everything_and_pins_mana() {
        let mut w = flat_world();
        w.set_dev_spells(true);
        assert!(w.dev_spells());
        let lv = w.loadout();
        assert!(lv.owned.iter().all(|&o| o), "all 24 owned");
        assert_eq!(lv.mana, lv.mana_max, "pool reads full");
        // Casts neither gate nor deduct while on.
        w.player.mana = 0;
        w.tick(firing_line(), PlayerCommand { fire_left: true, ..Default::default() });
        assert_eq!(count(&w, 9, 0), 1, "no mana gate under dev spells");
        assert_eq!(w.loadout().mana, w.loadout().mana_max);
        // Off keeps the granted spells (no un-granting).
        w.set_dev_spells(false);
        assert!(w.loadout().owned.iter().all(|&o| o));
    }

    #[test]
    fn deterministic_with_scripted_fire() {
        let run = || {
            let mut w = flat_world();
            for t in 0..400 {
                let p = if t < 16 { at_trigger() } else { firing_line() };
                let cmd = PlayerCommand {
                    fire_left: (60..90).contains(&t),
                    ..Default::default()
                };
                w.tick(p, cmd);
            }
            let (free, pool) = w.debug_pool();
            let snapshot: Vec<_> = pool
                .iter()
                .map(|e| (e.slot, e.class, e.model, e.state, e.tx, e.ty, e.life))
                .collect();
            (
                free,
                snapshot,
                w.planes().height.clone(),
                w.player_damage_taken(),
                w.combat_stats(),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn loadout_surfaces_the_balloon_roster_by_castle_level() {
        // The HUD balloon panel (sub_22E50 slot B): the roster size is
        // 1/2/3 by castle level (1-3 / 4-5 / 6-7), each entry a
        // (hp_frac, cargo_frac) of a player-owned class-3/model-3
        // balloon. No castle → no roster.
        let mut w = flat_world();

        // No castle yet: empty roster.
        assert!(w.loadout().balloons.is_empty(), "no castle → no balloons");

        // Plant a player-owned castle at level 4 (roster = 2) and three
        // player-owned balloons with distinct HP/cargo.
        let castle = 1;
        w.g.ent[castle].class64 = 3;
        w.g.ent[castle].model65 = 2;
        w.g.ent[castle].id24 = PLAYER_TARGET;
        w.g.ent[castle].flags = 0;
        w.g.ent[castle].f26 = 4; // level 4 → roster 2

        for (k, slot) in [2usize, 3, 4].into_iter().enumerate() {
            w.g.ent[slot].class64 = 3;
            w.g.ent[slot].model65 = 3;
            w.g.ent[slot].id24 = PLAYER_TARGET;
            w.g.ent[slot].flags = 0;
            w.g.ent[slot].max_life = 100;
            w.g.ent[slot].act_life = 100 - (k as i32) * 25; // 100, 75, 50
            w.g.ent[slot].f136 = 200; // cargo capacity
            w.g.ent[slot].f140 = (k as i32 + 1) * 50; // 50, 100, 150
        }

        let balloons = w.loadout().balloons;
        assert_eq!(balloons.len(), 2, "level-4 castle → 2-balloon roster");
        let first = balloons[0].expect("first roster slot live");
        let second = balloons[1].expect("second roster slot live");
        assert!((first.0 - 1.0).abs() < 1e-3, "first balloon full HP");
        assert!((first.1 - 0.25).abs() < 1e-3, "first balloon 50/200 cargo");
        assert!((second.0 - 0.75).abs() < 1e-3, "second balloon 75/100 HP");
        assert!((second.1 - 0.5).abs() < 1e-3, "second balloon 100/200 cargo");

        // Dead balloons do NOT shrink the roster (retail keeps the
        // [50+width] glyph and just draws no bars, :27335-40): kill
        // two of three → the roster is still 2 wide, one slot live
        // (the third balloon backfills), one empty... kill all three
        // → 2 wide, all empty.
        w.g.ent[2].flags |= 0x400;
        w.g.ent[3].flags |= 0x400;
        let balloons = w.loadout().balloons;
        assert_eq!(balloons.len(), 2, "roster width survives balloon deaths");
        assert!(balloons[0].is_some(), "the surviving balloon fills slot 0");
        assert!(balloons[1].is_none(), "the lost slot draws no bars");
        w.g.ent[4].flags |= 0x400;
        let balloons = w.loadout().balloons;
        assert_eq!(balloons.len(), 2, "all balloons dead → roster still 2 wide");
        assert!(balloons.iter().all(Option::is_none));

        // A collapsed castle (flag 0x400) removes the roster.
        w.g.ent[castle].flags |= 0x400;
        assert!(w.loadout().balloons.is_empty(), "collapsed castle → no roster");
    }

    #[test]
    fn book_bind_gate_is_the_castle_stored_unlock_ladder() {
        // The :26926 gate: bindable iff castle_req == 0 OR the linked
        // castle STORES >= castle_req — never a player-mana test.
        let mut w = flat_world();
        let free = SPELLS.iter().position(|s| s.castle_req == 0).expect("a free spell");
        let (locked, req) = SPELLS
            .iter()
            .enumerate()
            .find_map(|(i, s)| (s.castle_req > 0).then_some((i, s.castle_req)))
            .expect("a ladder spell");

        // No castle: free spells bindable, ladder spells locked —
        // regardless of player mana.
        w.player.mana = 0;
        let l = w.loadout();
        assert!(l.bindable[free], "castle_req 0 binds even at 0 mana");
        assert!(!l.bindable[locked], "ladder spell locked with no castle");

        // A castle storing just under / at the requirement.
        let castle = 1;
        w.g.ent[castle].class64 = 3;
        w.g.ent[castle].model65 = 2;
        w.g.ent[castle].id24 = PLAYER_TARGET;
        w.g.ent[castle].flags = 0;
        w.g.ent[castle].f26 = 1;
        w.g.ent[castle].f140 = req as i32 - 1;
        assert!(!w.loadout().bindable[locked], "stored < req stays locked");
        w.g.ent[castle].f140 = req as i32;
        assert!(w.loadout().bindable[locked], "stored >= req unlocks");
    }
}
