//! The shared chassis runtime world: the living level — trigger
//! volumes, dispositions, spawned entities, and runtime
//! terrain-mutating events. A verbatim remc1 port that runs all three
//! games (MC1, Hidden Worlds, MC2); the game-specific columns plug in
//! from [`crate::mc1`] and [`crate::mc2`].
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
//!   ([`crate::mc1::mobs`]): class-2 scenery, class-3 balloons/castles and
//!   class-5 creatures (with multipart body chains) carry authentic
//!   life/speed/extents/sprite state, and class-5 creatures TICK — the
//!   movement core, the six state primitives and the awake system are
//!   ported; the app consumes continuous poses via [`World::live_poses`].
//!
//! COMBAT (the combat slice, see [`crate::mc1::combat`]): class-5 attack
//! thunks fire class-9 projectiles / melee mailbox writes; class-10
//! combat effects deliver the damage; creatures read their inbox,
//! aggro on wizard-family attackers, die into DEATH/CORPSE and drop
//! mana balls. The player is MORTAL: the six-channel inbox applies for
//! real — grace window, at-castle redirect, shield quartering, hit
//! knockback, the death fall with the jar scatter and the m40 grave,
//! and the Space respawn at the castle (castle-less = the level
//! restarts). Invincibility survives as the `invincible` dev/config
//! toggle.
//!
//! Deliberate deviations, tracked in docs/ROADMAP.md: no AI wizard
//! balloons (the probe/scan lists are the player alone); custom
//! family behaviors beyond movement/combat (disguises, mana hunts,
//! house building, teleports) stand still pending the AI track;
//! class-12 pickup/mana transfer NOT ported (mana balls drop, merge
//! and take claims but nothing collects them yet); sounds omitted.

use crate::chassis::ChassisParams;
use crate::engine::features::{
    self, Ent, FeatureAssets, Gen, Planes, Rec, TerrainPlanes, build_table, lcg32,
};
use crate::ids::GameId;
use crate::mc1::combat::MailTarget;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};
use crate::mc1::spells::{DISPLAY_ORDER, SPELL_COUNT, SPELLS, SpellDef, SpellId};
use crate::mc1::sprite_stats::SPRITE_STATS;
use crate::verbs::{
    AwakeVerb, CommitGateVerb, DamageVerb, MovementVerb, ObjectiveVerb, TargetingVerb, VerbKind,
    VerbSet,
};
use mgc_formats::{Thing, ThingKind};

/// The player's life ceiling: the human wizard ctor's maxLife 10000
/// (:44185; skill does NOT scale it — sub_44D30 :55026 resets to max
/// on every spawn). Heal's 5%-per-tick rate divides it.
pub const PLAYER_LIFE_MAX: i32 = 10000;

/// The class-12 state marking a death-scattered spell jar: it decays
/// (200-289 ticks, :55545-47) where the THING-placed jar states
/// (0..=2) sit forever. Pickup works from any sub-MANIFEST state.
pub(crate) const DROPPED_JAR: u8 = 3;

/// Entity flag mirroring the original's `+18 byte[2] |= 4` — the BLUE
/// jar marker (THING `data_12 >= 3`, :44043-54). A blue jar grants
/// its spell UNRESTRICTED: the grant zeroes the manifestation's
/// castle requirement (+132 = 0, :64845), and both spell gates read
/// that entity field, not the spell table (:26924, :27860-64) — so
/// the spell binds and casts castle-less (the maze-level survival
/// loadout). Blue also swaps the jar/manifestation to sprite-type 280
/// (red = 77, :44052/:64897) and survives death: the scatter banks it
/// per spell (var_916, :55531-35), the respawn re-grant restores it
/// (:54908-12).
pub(crate) const BLUE_SPELL: u32 = 0x40000;

/// The player wizard's life state — the original class-3 states 0
/// (alive) / 2 (death fall) / 3 (dead, awaiting the respawn key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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
pub(crate) const MANIFEST_BASE: u8 = 200;

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
    pub(crate) accel_held: bool,
    /// MC2 Speed spell (`GetScroll_69DB0`): the per-tier travel
    /// multiplier `subSpellIndex_2` = {2,3,4} for tiers 0/1/2, held
    /// CONSTANT across the armed window (no MC1 held/released 3-vs-2
    /// distinction). 0 = inactive / use the MC1 accel factor.
    /// docs/spell-audit/speed.md.
    pub(crate) accel_mc2_factor: i8,
    /// MC2 Invisibility strength (`byte_0x1BF_447`): the invis tier's
    /// `life_0x1A` = {1,2,3}, set on the invis first tick, zeroed at
    /// window end / break. Drives the per-tier break-on-self-cast law
    /// (`sub_5F7E0` EF:60987): T0 any cast breaks, T1 all-but-possess,
    /// T2 nothing. docs/spell-audit/rival-spells.md §2.
    pub(crate) invis_strength: i8,
    /// MC2 Metamorph (spell 4): the class-5 model the caster is
    /// transformed into (2/19/25/16), or 0 when not transformed. Set on
    /// the metamorph cast, cleared at the cast-window expiry. The app
    /// reads it to HIDE the carpet (retail `caster.byte[0] |= 0x20`) —
    /// the pooled creature draws in its place (docs/spell-audit/
    /// summon-creatures.md Part A).
    pub(crate) metamorph: u8,
    /// Cached signed thrust-override factor (0.0 = inactive) —
    /// [`World::accel_override`].
    speed_boost: f32,
    /// Teleport return slot (:65554): recast returns here.
    teleport_return: Option<(u16, u16)>,
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
    /// Which of the death-remembered spells were BLUE-granted
    /// (unrestricted) — the original's per-spell var_916 bank, written
    /// by the jar scatter (:55531-35) and read back by the respawn
    /// re-grant to restore the blue marker + zero requirement
    /// (:54908-12). See [`BLUE_SPELL`].
    death_owned_blue: [bool; SPELL_COUNT],
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
            accel_mc2_factor: 0,
            invis_strength: 0,
            metamorph: 0,
            speed_boost: 0.0,
            teleport_return: None,
            life: PLAYER_LIFE_MAX,
            grace: 100,
            regen_delay: 0,
            state: LifeState::Alive,
            fall_speed: 0,
            killer: 0,
            death_owned: [false; SPELL_COUNT],
            death_owned_blue: [false; SPELL_COUNT],
            hit_flash: 0,
            lost: false,
        }
    }
}

/// Manual because `speed_boost` is the one float in persistent sim
/// state (hashed by bit pattern). The full destructure makes a new
/// `Player` field a compile error here: extend the hash deliberately.
impl std::hash::Hash for Player {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        let Player {
            mana,
            mana_max,
            mana_delta,
            banked,
            world_mana,
            left,
            right,
            owned,
            shield,
            invisible,
            rebound,
            beyond_sight,
            heal_active,
            accel,
            accel_held,
            // Not hashed directly: the derived `speed_boost` below
            // already reflects it; skipping it keeps the goldens
            // byte-stable.
            accel_mc2_factor: _,
            // MC2-only, nonzero only mid-invis-window; skipped to keep
            // the goldens byte-stable (no fixture casts invis).
            invis_strength: _,
            // MC2-only, nonzero only while transformed; skipped for the
            // same golden-stability reason (no fixture casts metamorph).
            metamorph: _,
            speed_boost,
            teleport_return,
            life,
            grace,
            regen_delay,
            state,
            fall_speed,
            killer,
            death_owned,
            death_owned_blue,
            hit_flash,
            lost,
        } = self;
        (mana, mana_max, mana_delta, banked, world_mana, left, right).hash(h);
        (owned, shield, invisible, rebound, beyond_sight, heal_active).hash(h);
        (accel, accel_held, speed_boost.to_bits(), teleport_return).hash(h);
        (life, grace, regen_delay, state, fall_speed, killer).hash(h);
        (death_owned, hit_flash, lost).hash(h);
        // Hashed only when armed: fixtures that never held a
        // blue-granted spell through a death keep their goldens
        // (the mc2_apocalypse precedent).
        if death_owned_blue.iter().any(|&b| b) {
            death_owned_blue.hash(h);
        }
    }
}

/// FNV-1a 64, spelled out so fixture hashes are stable across Rust
/// releases and platforms (std's default hasher guarantees neither).
struct Fnv(u64);

impl std::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01b3);
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
    /// The full-screen palette flash: the retail row code (0 = none)
    /// and the ticks left in its fade home. Row 3 is Global Death's
    /// violet wash — see `PalFlash`.
    pub pal_flash: (u8, u8),
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
/// One hand's live crosshair lock (see [`World::aim_preview`]):
/// the point the hand's shot would chase, in [`LivePose`] space
/// (tile x/z + altitude).
#[derive(Debug, Clone, Copy)]
pub struct AimLock {
    pub x: f32,
    pub z: f32,
    pub alt: f32,
}

pub struct LoadoutView {
    pub owned: [bool; 24],
    pub left: Option<u8>,
    pub right: Option<u8>,
    /// 0.0 = ready, 1.0 = just fired (burst counter / count).
    pub cooldown: [f32; 24],
    /// Effective per-cast mana cost of each spell RIGHT NOW
    /// ([`World::spell_cast_cost`]) — the HUD availability meter divides
    /// the pool by this (sub_23D40 :27703 reads the manifestation's live
    /// +136). Castle (16) scales with the own castle's level; the rest
    /// are static.
    pub cost: [u32; 24],
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
        PlayerPose {
            x,
            y,
            z,
            heading,
            pitch: 0,
            speed: 0,
        }
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
    pub equip_left: Option<crate::mc1::spells::SpellId>,
    pub equip_right: Option<crate::mc1::spells::SpellId>,
    /// MC2 spell selection (the CTRL-pane commit — retail's
    /// PlayerAction 0x1F/0x20 "Change Spell", remc2 EF:37898):
    /// (spell index 0..25, tier 0..2, hand 0 = left / 1 = right).
    pub mc2_select: Option<(u8, u8, u8)>,
    /// The respawn key (Space, command 15 :20081/:48620) — only
    /// consumed while dead.
    pub respawn: bool,
    /// The demolish key (Shift+L → the unique control word 48,
    /// :20496-501): sets the OWN castle's life to −1 (:55846-50) —
    /// one downgrade level per press.
    pub demolish: bool,
}

/// The runtime world of one loaded MC1/HW level.
/// One registered MC2 stage — the runtime state of a level
/// checkpoint (`InitStages_58940`, remc2 :40567). `kind` = the
/// objective type; `target` = per-type payload (type 0: the banked-%
/// goal; type 7: the target MODEL, resolved from the THING table at
/// registration — retail stores `entity_0x30311[stage_1].subtype`,
/// :40628); `point` = the type-5 fly-to point in engine units
/// (checkpoint coords << 8). `state`: 1 active, 2 done. `row` = the
/// checkpoint's AUTHORED row in the level's stage array — the key
/// the stage-gated (11,32) switch carries in its par1
/// (`AddSwitch0B_20_6F1C0` :54353 tests
/// `struct_0x3659C[p].stage_0x3659F[par1] == 2`).
#[derive(Debug, Clone, Copy, Hash)]
struct Mc2Stage {
    kind: u8,
    target: u32,
    point: (u16, u16),
    state: u8,
    row: u8,
    /// The external force-complete flag (`str_3654D_byte1 & 2`,
    /// :40737-42): consumed by the NEXT objective pass — completion
    /// and the cursor advance stay pass-timed, so the m32 pause can
    /// bridge the follow-up spawns.
    force: bool,
    /// The live pool slot the row's NAMED target bound to (`sub_58DA0`,
    /// EF:40650-90) — types 1/2 only. `None` until the referenced THING
    /// (index = `target`) spawns and the bind seam
    /// ([`World::mc2_bind_stage_target`]) matches it. The decompile's
    /// `str_3654D_byte1 & 1` "bound" bit is `bound.is_some()`. Because
    /// our slots recycle (LIFO free list), the completion test re-checks
    /// `thing_slot == target` identity rather than trusting a raw life
    /// read of a possibly-reused slot (see [`World::mc2_bound_gone`]).
    bound: Option<u16>,
}

pub struct World {
    pub(crate) g: Gen,
    /// Which game's profile this world runs ([`crate::ids`]): keys
    /// the misfit registry and labels the telemetry. The rules
    /// themselves live in `g.chassis` + `g.verbs`.
    game: GameId,
    /// The MC2 stage registry + the CURRENT stage cursor
    /// (`ObjectiveText_1` — types 5/7 only test while current).
    /// Empty on MC1 worlds (hashed only when populated so the MC1
    /// goldens hold across this layout change).
    mc2_stages: Vec<Mc2Stage>,
    mc2_stage_current: usize,
    /// The MC2 StageVar table (the triggered-spawn / hold-gate layer,
    /// `crate::mc2::stagevars`). Index-aligned with the level file's
    /// 11-slot array; empty on MC1 and StageVar-less MC2 levels (both
    /// vecs hash only when populated so those goldens hold).
    pub(crate) mc2_stagevars: Vec<crate::mc2::stagevars::Mc2StageVar>,
    /// The live HELD-creature ↔ StageVar bindings (retail's per-entity
    /// `StageVar1`/`word_0x4A_74`, kept off `Ent` for hash discipline).
    pub(crate) mc2_sv_held: Vec<crate::mc2::stagevars::Mc2Held>,
    /// Deferred m9 (hive imp) holds: `(ent, slot)` parked at spawn
    /// (retail stores the pending slot in `word_0x4A_74`, EF:4716-22)
    /// and armed when the 16-tick materialize completes (`sub_122A0`,
    /// EF:4953-58) — the imp visibly rises out of the ground BEFORE
    /// freezing on its gate. Hash rides the stagevar gate (populated ⇒
    /// mc2_stagevars is too).
    pub(crate) mc2_sv_deferred: Vec<(u16, u8)>,
    /// `ObjectiveDone_2` (:40724-27): the objective engine's pause
    /// countdown. The m32 stage-gated switch sets 1 as it fires
    /// (:54371) — the skipped pass bridges the one-tick gap between
    /// a row latching and the switch's disposition spawning the
    /// NEXT row's targets (without it a current type-7 row latches
    /// vacuously before its creatures exist).
    mc2_objective_pause: i16,
    /// `D41A0_0.byte_0x36E02` — the objective-message/voiceover
    /// trigger ramp (docs/traces/mc2-voiceover-triggers.md §3): set
    /// to 1 at level load and when the CURRENT stage row completes
    /// or the level ends; walks 1→8 over ~7 ticks (the deliberate
    /// delay between the objective latching and the voice), fires
    /// the sound-41 pre-cue at step 7 and the speech cue + the
    /// sound-61 advance chime at step 8, then idles 9→0xC8 (a long
    /// quiet tail so it can't re-fire) and resets.
    mc2_speech_ramp: u8,
    /// The pending speech cue (the segment index retail passes to
    /// `PlayCDTrackSegmentNumber`: objective row + 1, or 9 at level
    /// end), drained by [`World::take_audio`]. Presentation-side
    /// transient, never hashed.
    mc2_speech_cue: Option<u8>,
    /// `D41A0_0.byte_0x36E03` — the APOCALYPSE latch: selects the
    /// (10,9) dome's endgame variant (no damage, sound 63, (10,91)
    /// child). Cleared by the dome ctor (EF:35527 — done at the
    /// spawn call sites); set by the doomsday pyramid's case 0xF
    /// (`mc2::doomsday`, EF:12871).
    pub(crate) mc2_apocalypse: bool,
    /// The game-turn counter (retail `Turn_2BE0_11248`) — the
    /// (10,86) cave-drip spawner's 8-turn cadence key. HASH-EXCLUDED
    /// (Rec.par3 precedent): a pure function of the tick() call
    /// count; its effects reach the hash through the spawns it
    /// makes.
    mc2_turn: u32,
    /// The doomsday HUD meter `x_BYTE_D9F50[0x87a]` (0..1200),
    /// driven by the pyramid's bit-5 ramp — banked for the 4.9 HUD
    /// track (hash-transparent while 0, like the latch).
    pub(crate) mc2_doom_meter: i16,
    /// `terrain_2FECE.byte_0x2FED2 & 2` — the doom-palette level bit
    /// (the night-fog gfx variant): the (5,10) pyramid ctor returns
    /// NULL without it (EF:33968). Set by the app from the level's
    /// gfx environment; construction config like `placeholders`.
    pub(crate) mc2_doom_level: bool,
    /// Spawn a placeholder billboard (the class-2 marker stone) where
    /// an unknown `(class, model)` was authored — the seam's
    /// graceful-degradation visual, OFF by default (a faithful MC1
    /// world drops unknown things silently like retail). The misfit
    /// ledger counts either way.
    placeholders: bool,
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
    pub(crate) player: Player,
    /// Live AI wizards (player slots 1..=7) — see [`crate::mc1::rivals`].
    pub(crate) rivals: Vec<crate::mc1::rivals::Rival>,
    /// Live MC2-column AI wizards (colors 1..player_count) — see
    /// [`crate::mc2::rivals`]. Empty on MC1 worlds and on MC2 worlds
    /// wired before the rival column (hash-gated on non-empty).
    pub(crate) mc2_rivals: Vec<crate::mc2::rivals::Mc2Rival>,
    /// Kill tally [killer slot][victim slot] (the original keeps it
    /// on the killer's Type_160+30 — the book roster's numbers).
    pub(crate) kill_tally: [[u16; 8]; 8],
    /// The human carpet's engine-unit pose, refreshed each tick for
    /// the rival scans (the original reads the wizard entity; ours
    /// lives outside the pool).
    pub(crate) human_pose: (u16, u16, i16),
    /// Rival deaths this tick (player slots) — drained by the app
    /// for the death-message ticker (:55499-517).
    pub(crate) rival_deaths: Vec<u8>,
    /// The human-cast duel latch (ch4 grip, :55663-82 + :55228-48):
    /// (victim entity, tick counter, initial-distance hold). The
    /// CASTER is pulled toward the victim until 1000 ticks, 5120
    /// distance, or the victim dies.
    duel: Option<(u16, u16, u32)>,
    /// The MC2 duel LOCK (`dword_0xA4_164` fields 322/326/330, remc2
    /// EF:60648-56): (opponent avatar entity, held tether distance
    /// clamped [1024, 3072], tier 0..2). Set by the (10,26) tether
    /// grip ([`World::mc2_duel_tether_tick`]); enforced per tick
    /// beside the MC1 pull; hash-gated on Some (tag 0xE2).
    pub(crate) mc2_duel: Option<(u16, i32, u8)>,
    /// The human wizard's MC2 spell book (`str_611` subset: learned
    /// manifestations, XP, levels, tiers, quick-slots) — the Phase
    /// 4.2 cast column ([`crate::mc2::cast`]). Hashed only once
    /// touched (pristine = hash-transparent, the MC1 goldens hold).
    pub(crate) mc2_book: crate::mc2::cast::Mc2Spellbook,
    /// Player-start marker tiles by slot (class-3 models 4..=11,
    /// str_9177 :44068-107), captured before the level-init
    /// disposition consumes the records.
    pub(crate) start_markers: [Option<(u16, u16)>; 8],
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
    pub(crate) dev_spells: bool,
    /// Unfaithful improvement (deliberate, P-class): remove any spell
    /// jar the local player already owns. Retail leaves such jars in the
    /// world forever (placed jars carry life 0), but they can never be
    /// picked up — permanent, unidentifiable clutter. When on
    /// (single-player entity removal), an owned-spell jar self-culls on
    /// its next tick, covering both the level-load sweep and the instant
    /// the player gains the spell. Faithful default = OFF.
    pub(crate) prune_owned_jars: bool,
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
    /// The top-of-screen notification line (retail's per-player
    /// `CurrentNotificationText_0x01c_2BFA` + its ~200-tick life): the
    /// shared, game-generic message surface — spell selection, spell
    /// level-ups, and later deaths/rival events/objectives. Presentation
    /// transient, HASH-EXCLUDED (the goldens never see it). See
    /// [`World::set_notification`] / [`World::notification`].
    notification: Option<Notification>,
    /// The level is WON — the true terminator, distinct from
    /// [`World::completed`] (retail: MC1's cmd-27 win-exit
    /// `13325 = 10` :48804; MC2's endGameSeq phase 0xC
    /// `byte[2] |= 0x10` EF:60543). The app consumes it: fade out
    /// and end the game (deliberate: no stats screen / campaign
    /// stitching yet). Hash-transparent while false.
    won: bool,
    /// The MC2 level-ending sequence (`sub_5E8C0_endGameSeq`
    /// EF:60313-60589), installed by an ending-marker trip and
    /// advanced once per tick — the scripted decelerate → aim →
    /// launch → terrain-glued fly-in → fade. Drives the app's flyer
    /// via [`World::mc2_end_pose`] (the human lives outside the
    /// pool; retail swaps the player entity's actionIndex to 11/12).
    /// Hash-gated on Some.
    mc2_endseq: Option<Mc2EndSeq>,
    /// The trip mailbox — the class-14 TARGET MODEL (4 = the demon
    /// mouth, action 11; 3 = the checkpoint "X", action 12 — the
    /// mc2:00 ending), installed into `mc2_endseq` the same tick
    /// (the trip site has no PlayerPose in scope). Transient,
    /// hash-excluded like the other mailboxes.
    mc2_end_pending: Option<u8>,
}

/// The MC2 level-ending state machine (`sub_5E8C0_endGameSeq`,
/// EF:60313-60589), phase-numbered like retail's `byte_0x46_70`
/// (0, 1, 3..=12 — there is no phase 2). BOTH marker variants run
/// this one machine (retail action 11 → the (14,4) demon mouth via
/// `word_0x36DFC`; action 12 → the (14,3) checkpoint "X" via
/// `word_0x36DFE`, EF:60367-80 — level-000 ends through the 12
/// arm). The pose is the scripted carpet in engine units; the app
/// mirrors it while active.
#[derive(Clone, Copy, Debug, Hash)]
struct Mc2EndSeq {
    /// `byte_0x46_70`.
    phase: u8,
    /// `dword_0x10_16` — zoom steps (12), flight ticks (512/128),
    /// fade ticks (32).
    counter: i32,
    /// `actSpeed_0x82_130`, engine units/tick.
    speed: i16,
    /// `word_0x96_150` — the fly-to marker's pool slot, 0 = none.
    target: u16,
    /// The class-14 model to fly to (4 = mouth, 3 = checkpoint X).
    target_model: u8,
    /// Scripted pose (engine 8.8 torus / z / 11-bit yaw).
    x: u16,
    y: u16,
    z: i16,
    yaw: u16,
}

/// A transient top-of-screen notification (retail `CurrentNotification
/// Text`). `color` is the ink RGB — the original resolves DrawText's
/// colour from a CLRD-0 RGB444 code (the plain toast is `0xF00` = pure
/// red) to the nearest palette index; we carry the intended truecolor
/// RGB and let the app tint the white glyph mask, matching the intent
/// without the palette-quantization step (the UI already renders
/// truecolor). The team-colour categories (alliance/objective flashes)
/// will extend this later.
#[derive(Clone, Debug)]
pub struct Notification {
    pub text: String,
    /// Ticks remaining before the line clears.
    pub timer: u16,
    /// Ink colour, RGB (DrawText's resolved `color`).
    pub color: [u8; 3],
}

/// One live drawable entity, resolved for the app's billboard / map
/// layer: continuous pose (position in tile units, real-valued yaw)
/// plus the sprite-stats type index and animation frame the sim's
/// spawn/tick handlers assigned. Presentation resolves late — the
/// billboard backend snaps yaw to view sectors at draw time, a mesh
/// backend would consume the same pose unquantized.
#[derive(Debug, Clone, Copy)]
pub struct LivePose {
    /// Pool slot + spawn generation: a stable identity for matching
    /// poses across tick snapshots (the render interpolation pairs
    /// them). Slot alone aliases across reuse — a projectile dying
    /// into a fresh spawn the same tick — the (slot, gen) pair never
    /// does. `generation` is presentation-only, hash-silent in the sim.
    pub slot: u16,
    pub generation: u32,
    pub class: u8,
    pub model: u8,
    /// Row into [`crate::mc1::sprite_stats::SPRITE_STATS`].
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
    /// Owning wizard's player slot (0 = the human, 1-7 = rivals),
    /// when wizard-owned: the map team-pair index and the per-team
    /// castle/balloon stamp offset (58+team / 66+team).
    pub team: Option<u8>,
    /// Retail sprite raster mode for translucent effects (MC2;
    /// docs/traces/mc2-transparency-drawlist.md): 0 = opaque, 2 =
    /// 33%-opaque (`T[0x4000 + (src<<8)|dst]` — smoke), 3 =
    /// 67%-opaque (operands swapped — glows/death fades). The
    /// renderer maps 2/3 to plain alpha 1/3 / 2/3 (the blend matrix
    /// is `nearest_palette(⅓·src + ⅔·dst)` minus quantization).
    pub blend: u8,
    /// The entity appears on the overhead map but draws NO world
    /// billboard — MC2 unclaimed buildings: the flag sprite is
    /// suppressed (byte[0] bit 0, EF:27292-97) but the retail map
    /// pass still plots them (0xF0F UNPOSSESSED_BUILDING2,
    /// GameUI.cpp:1276-1295 — it never skips on the claim bit).
    pub map_only: bool,
}

/// One rival wizard's presentation snapshot ([`World::rival_views`]).
#[derive(Debug, Clone)]
pub struct RivalView {
    pub slot: u8,
    pub name: &'static str,
    pub alive: bool,
    pub eliminated: bool,
    /// Live position, tile units.
    pub x: f32,
    pub z: f32,
    pub mana: u32,
    pub mana_max: u32,
    pub life_frac: f32,
    /// This rival's kill row (victim slots 0-7).
    pub kills: [u16; 8],
    pub invisible: bool,
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
    pub row: u8,
    pub flags: u32,
}

/// One creature's full AI state for [`World::debug_flock_probe`]
/// (the flocking diagnostic): everything needed to attribute a
/// speed/state per tick — position (8.8 fixed), the speed triple
/// (`f126` act / `f128` min / `f130` max), the state byte, and the
/// awake/leader/target/attacker links.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct FlockProbeRow {
    pub slot: usize,
    pub id24: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    /// Facing (f30, 11-bit engine angle).
    pub yaw: u16,
    /// Target yaw (f34).
    pub aim: u16,
    pub speed: i16,
    pub min_speed: i16,
    pub max_speed: i16,
    /// Raw tick-handler byte (goat roles = state − 8).
    pub state: u8,
    pub life: i32,
    /// Awake countdown (f58; 0 = asleep, 0xFA = never-woken sentinel).
    pub awake: i16,
    /// StageVar2 / controlled-slot kind (site_z; 0 = free).
    pub hold: i16,
    /// Pack leader slot (f52; 0 = none).
    pub leader: u16,
    /// Chase/flee target slot (f146; 0xFFFF = the player).
    pub target: u16,
    /// Attacker latch (f40).
    pub attacker: u16,
    /// Per-tick cadence byte (f63).
    pub cadence: u8,
    /// Raw flag word — bit 27 = the move-core block latch (retail
    /// `byte[2] & 4`: the last move hit the terrain fence).
    pub flags: u32,
}

/// One tick's audio outputs: drained sound requests + the ambient
/// rule inputs (see [`World::take_audio`]).
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub events: Vec<crate::engine::features::SoundEvent>,
    /// The carpet is over a water tile (waves vs wind, :55254-65).
    pub over_water: bool,
    pub fire_near: bool,
    pub market_near: bool,
    /// Danger-music mode (the wizard's v_46 countdown is live —
    /// recently hit or targeted; :55282-92).
    pub danger: bool,
    /// MC2 voiceover cue: the pending speech SEGMENT (objective
    /// row + 1, or 9 at level end — the app supplies the level-row
    /// half of the `CdTracks_DB080` address). One-shot, drained.
    pub speech: Option<u8>,
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
    /// The WIN trigger (sub_59B80 :67293): fires its disposition on
    /// the player's completion latch and consumes the win.
    WinTrigger,
    /// Teleporter vortex.
    Portal,
    /// An MC2 stage checkpoint (the objective board's fly-to point /
    /// stage marker) — plotted for route troubleshooting; the CURRENT
    /// stage reports via [`super::world::World::mc2_objective_view`].
    Objective,
}

/// A live world-space target of the CURRENT MC2 objective, for the
/// non-optional objective-guide overlay (the flashing map/minimap
/// marks + the nearest-target arrow). Position is in TILE units,
/// matching [`LivePose`], so the app projects it with the same
/// map/minimap transform it uses for entities. `nearest` flags the
/// single closest piece of the goal (the arrow anchor); the rest are
/// highlight-only. `yellow` picks the retail outline colour — YELLOW
/// (CLRD 0xFF0) for the fly-to POINT objective (type 5), RED (0xF00)
/// for creature and building targets (remc2 GameUI DrawObjectiveRectangle
/// switch: only `case 8`/type-5 uses 0xFF0).
#[derive(Debug, Clone, Copy)]
pub struct ObjectiveTarget {
    pub x: f32,
    pub z: f32,
    pub nearest: bool,
    pub yellow: bool,
}

/// Records the app can draw (mc1_entities has a sprite mapping).
/// Class 9 = projectiles; class 10 is logic/terrain except the portal
/// vortex and the combat effects (fire, flame, splash, flashes, mana
/// ball — the model-17 blast driver is invisible by design).
fn drawable(game: GameId, class: u16, model: u16) -> bool {
    // The (10,12) possess flash carries the ctor's sprite row 41 but
    // draws NOTHING in retail — its draw gate is whatever +16 bit the
    // ctor clears; excluded here. Also excluded: the genuinely
    // invisible drivers (15 quake walker, 17 blast ring, 18 eruption
    // counter, 41/42 leveler/painter, 53 napalm cloud — its visible
    // part is the (10,6) sheets it spawns).
    // 6 standing fire / 16 lava bomb / 19 plume / 38 storm cloud /
    // 43 upgrade token ARE sprite-carrying visibles.
    // The (9,18) Global Death fuse carries ctor sprite 42 (fireball
    // boilerplate) but retail shows NO prime visual; its draw gate is
    // in the unported state-19 handler: invisible.
    // Class 14 = MC2's special map objects (X/end markers, scrolls)
    // — sprite-carrying pickups/markers, EXCEPT the terrain risers
    // (models 1/2): invisible machinery, no SetEntityIndex in their
    // creator path (mc2::riser).
    // MC2's (10,13)/(10,14) smoke particles are sprite-carrying;
    // their (10,59)/(10,60) emitters are invisible by construction
    // (no sprite, never map-linked) and stay excluded.
    // Class 15 = MC2's spell-jar tokens (fixed sprite 77).
    // The MC2-era arms (classes 14/15, class-10 models 13/14/22/75/77)
    // are gated on the game: an MC1 world's (10,13..) etc. are
    // unrelated logic models and must not acquire a sprite pose.
    let mc2 = matches!(game, GameId::Mc2);
    (matches!(class, 2 | 3 | 5 | 12)
        || (mc2 && (class == 15 || (class == 14 && !matches!(model, 1 | 2)))))
        || (class == 9 && model != 18)
        || (class == 10
            && (matches!(
                model,
                34 | 0 | 1 | 5 | 6 | 16 | 19 | 23 | 25 | 26 | 38 | 39 | 40 | 43 | 45
            )
                // MC2 effect billboards: sprite-carrying entities that
                // must be in the allowlist or the whole effect runs
                // INVISIBLE though it ticks (damage + sound). The
                // (10,22) WHIRLWIND head + its (10,75) funnel column
                // (sprite rows 293+index, mc2::tail), and the (10,76)
                // FIRE ORB's 25 (10,77) satellites (sprite 340). The
                // orb hub (76) + wind eye carry no sprite (pure
                // controllers) and stay out, as does the (10,54)
                // magnet aura (retail AddAuxiliary_50500 sets no
                // SetEntityIndex — the visual is the streaming mana).
                // (10,13)/(10,14) are the MC2 smoke particles.
                // (10,79) is the castle defend turret (ctor sprite 66,
                // sub_508E0 EF:37000).
                || (mc2 && matches!(model, 13 | 14 | 22 | 75 | 77 | 79))))
}

/// The game-keyed per-entity presentation decisions for
/// [`World::live_poses`] — the MC1/MC2 split (S3 code motion). All
/// fields are hash-excluded presentation.
struct PoseGameBits {
    /// MC1 skips unclaimed dwellings entirely (`continue`); MC2 keeps
    /// them and exports the pose as map-only.
    skip: bool,
    segment: bool,
    life_frac: Option<f32>,
    blend: u8,
    map_only: bool,
}

impl World {
    /// The player-spell stat table for this world's game — routes MC1/HW
    /// `SPELLS[id]` reads through the per-game accessor so Hidden Worlds'
    /// one divergent row (20, Fire Storm) applies without touching the
    /// base MC1 table or its goldens. Returns a `'static` reference, so
    /// `&self.spells()[id]` holds no borrow on `self`.
    pub(crate) fn spells(&self) -> &'static [SpellDef; SPELL_COUNT] {
        crate::mc1::spells::spells(self.game)
    }

    /// Build the world: apply the load-time feature pass to the
    /// pristine planes, then fire disposition 0 (level init) so the
    /// initial population spawns. `things` come from the package;
    /// `seed` is the GEN_MAP seed.
    pub fn new(planes: Planes, things: &[Thing], seed: u32, assets: FeatureAssets) -> Self {
        Self::new_with_chassis(planes, things, seed, assets, ChassisParams::MC1)
    }

    /// [`World::new`] with an explicit chassis set — the per-game
    /// pristine constants ([`crate::chassis`]), or a deliberately
    /// deviating set (limit-removing tests; G-class). MC1 verb column.
    pub fn new_with_chassis(
        planes: Planes,
        things: &[Thing],
        seed: u32,
        assets: FeatureAssets,
        chassis: ChassisParams,
    ) -> Self {
        Self::new_full(planes, things, seed, assets, chassis, GameId::Mc1)
    }

    /// A world under a game's PRISTINE profile — chassis + tier-5
    /// verb column selected by [`GameId`] ([`crate::ids`]). Pending
    /// verb arms fall back to MC1 with telemetry
    /// ([`World::verb_fallbacks`]) — the Phase-2 seam contract:
    /// degrade gracefully, never crash, tell the truth.
    pub fn new_for_game(
        planes: Planes,
        things: &[Thing],
        seed: u32,
        assets: FeatureAssets,
        game: GameId,
    ) -> Self {
        Self::new_full(planes, things, seed, assets, game.chassis(), game)
    }

    /// The full-control constructor: an explicit chassis under an
    /// explicit game profile — for callers that must combine a
    /// deviating chassis (limit-removing overrides; G-class) with a
    /// non-MC1 verb column. [`World::new_for_game`] with the game's
    /// pristine chassis is the faithful entry.
    pub fn new_full(
        planes: Planes,
        things: &[Thing],
        seed: u32,
        assets: FeatureAssets,
        chassis: ChassisParams,
        game: GameId,
    ) -> Self {
        let mut start_markers: [Option<(u16, u16)>; 8] = Default::default();
        for t in things {
            if t.class == 3 && (4..=11).contains(&t.model) {
                let slot = (t.model - 4) as usize;
                if start_markers[slot].is_none() {
                    start_markers[slot] = Some((t.x, t.y));
                }
            }
        }
        let table_base = if matches!(game, GameId::Mc2) { 0 } else { 1 };
        let mut table = build_table(things, chassis.level_table_slots, table_base);
        let mut g = Gen::new(planes, assets, seed, chassis, game.verbs());
        // MC2 generates its retile/blend table at level setup from the
        // engine's corner-class data (sub_44580, remc2 Terrain.cpp:1011
        // over unk_D47E0) — same generator as MC1's byte_B5D40, MC2
        // data. Construction-time dispatch, not a handler branch.
        if matches!(game, GameId::Mc2) {
            g.retile = crate::mc2::terrain_paint::retile_table_mc2();
        }
        // The load-time pass is per-game: MC1's class-10 terrain-
        // feature fixpoint vs MC2's (none — MC2 terrain is
        // pre-generated; its at-load spawns run below, remc2 has no
        // feature event loop, Events.cpp:152).
        if !matches!(game, GameId::Mc2) {
            g.load_time_pass(&mut table);
        }
        let mut w = World {
            g,
            game,
            placeholders: false,
            table,
            mc2_stages: Vec::new(),
            mc2_stage_current: 0,
            mc2_stagevars: Vec::new(),
            mc2_sv_held: Vec::new(),
            mc2_sv_deferred: Vec::new(),
            mc2_objective_pause: 0,
            mc2_speech_ramp: 0,
            mc2_speech_cue: None,
            mc2_apocalypse: false,
            mc2_turn: 0,
            mc2_doom_meter: 0,
            mc2_doom_level: false,
            terrain_dirty: false,
            entities_dirty: false,
            pending_teleport: None,
            player: Player::default(),
            win_pct: 0,
            win_streak: 0,
            completed: false,
            dev_spells: false,
            prune_owned_jars: false,
            prev_fire: (false, false),
            accel_veto: (false, false),
            pending_respawn: None,
            pending_restart: false,
            invincible: false,
            rivals: Vec::new(),
            mc2_rivals: Vec::new(),
            kill_tally: [[0; 8]; 8],
            start_markers,
            human_pose: (0, 0, 0),
            rival_deaths: Vec::new(),
            duel: None,
            mc2_duel: None,
            mc2_book: Default::default(),
            notification: None,
            won: false,
            mc2_endseq: None,
            mc2_end_pending: None,
        };
        // MC2 level init (remc2 EventsFunctions.cpp:39390-39425):
        // the GenerateEvents at-load passes over DisId == -1 records
        // (Events.cpp:152-282), THEN disposition 0 — the disposition
        // scan itself is the shared chassis shape (sub_4A1E0 :32950
        // ≡ MC1 sub_37440).
        if matches!(w.game, GameId::Mc2) {
            w.mc2_generate_events();
        }
        w.fire_disposition(0, true);
        // MC2's level-start book: fireball + possess at 0 XP. MC1
        // stays spell-less below — different game, different law.
        // (The real retail site is `InitialiseSpells_54A50`, NOT
        // `SetDefaultSpells_5C0A0` which grants nothing; the scope
        // caveat lives on `mc2_seed_default_spells`.)
        if matches!(w.game, GameId::Mc2) {
            w.mc2_seed_default_spells();
        }
        // NO free starting spells: the retail human grant is
        // (availability mask) AND (campaign collected flags)
        // (:49226-33) — with no campaign store, a fresh world's book
        // is EMPTY, exactly like retail level 1 (idx 000's
        // starting_spells row is empty; the first three spells are
        // its JARS, collected in play). Campaign-progress stand-ins
        // are the plausible_spellbook / dev_spells instruments.
        // The level-start screen-mode chime (sub_3DC90 :49072 plays
        // sound 14 on every mode set, the init included) — drained
        // by the app's first audio tick.
        w.g.snd_player(14);
        w
    }

    /// Load-time-features-only view (parity helper for callers that
    /// want the planes without the runtime; MC2 uses `TerrainPlanes`
    /// directly until its feature pass is ported).
    pub fn planes(&self) -> &Planes {
        &self.g.t
    }

    /// A LAYOUT-INDEPENDENT digest of the OBSERVABLE world: sprite
    /// poses (type + quantized position), the terrain height plane,
    /// and the population count. Deliberately blind to the hashed
    /// state's LAYOUT (field order, conditional-contribution shape),
    /// so a golden pinned on THIS survives layout-only `state_hash`
    /// re-pins and carries behavioral continuity across them (the
    /// same projection the pool-transparency test compares).
    pub fn observable_digest(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        for p in self.live_poses() {
            h.write_u16(p.type_index);
            h.write_i32((p.x * 256.0) as i32);
            h.write_i32((p.z * 256.0) as i32);
        }
        self.g.t.height.hash(&mut h);
        h.write_usize(self.live_things().len());
        h.finish()
    }

    /// Snapshot of the live drawable entities as THING-shaped records
    /// (kind = Entity), one per creature/scenery/pickup — multipart
    /// body segments excluded, like the original's entity lists.
    pub fn live_things(&self) -> Vec<Thing> {
        let mut out = Vec::new();
        for (i, e) in self.g.ent.iter().enumerate().skip(1) {
            if e.class64 == 0 || !drawable(self.game, e.class64 as u16, e.model65 as u16) {
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
            // MC2 likewise: a class-15 token in its effect state
            // (3·model) with a rebound owner is a wizard's spell
            // object; fresh authored state-3M jars keep their own
            // slot in id24 and stay visible (inert, like retail).
            if e.class64 == 15 && e.tick70 == e.model65.wrapping_mul(3) && e.id24 != i as u16 {
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
        for (i, e) in self.g.ent.iter().enumerate().skip(1) {
            if e.class64 == 0 || !drawable(self.game, e.class64 as u16, e.model65 as u16) {
                continue;
            }
            if e.class64 == 12 && e.tick70 >= MANIFEST_BASE {
                continue; // owned manifestation, not a drawable
            }
            // MC2 owned-spell manifestations (state 3M, rebound
            // owner) — cast machinery, not a drawable.
            if e.class64 == 15 && e.tick70 == e.model65.wrapping_mul(3) && e.id24 != i as u16 {
                continue;
            }
            // Hidden (dead wizard, :55568) / cloaked (spell 12,
            // :65689) — the original's byte16 0x20 suppresses the
            // billboard (the 0x21 draw skip, :36830).
            if e.flags & 0x20 != 0 {
                continue;
            }
            // The game-keyed presentation split (unclaimed-dwelling
            // skip/map-only, segment-hide states, life-bar
            // denominators, MC2 translucency): per-game bodies below.
            let bits = match self.game {
                GameId::Mc2 => self.live_poses_mc2(e),
                _ => self.live_poses_mc1(e),
            };
            if bits.skip {
                continue;
            }
            out.push(LivePose {
                slot: i as u16,
                generation: self.g.slot_gen.0.get(i).copied().unwrap_or(0),
                class: e.class64,
                model: e.model65,
                type_index: e.type86,
                frame: e.frame88,
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                alt: e.z as f32 / 256.0,
                yaw: (e.f30 & 0x7FF) as f32 * (TAU / 2048.0),
                segment: bits.segment,
                life_frac: bits.life_frac,
                player_owned: e.id24 == PLAYER_TARGET
                    || (e.class64 == 10 && e.f144 == PLAYER_TARGET),
                team: {
                    let owner = if e.class64 == 10 && matches!(e.model65, 39 | 45) {
                        e.f144
                    } else {
                        e.id24
                    };
                    // A rival wizard's own billboard is its own team.
                    self.owner_slot(owner)
                },
                blend: bits.blend,
                map_only: bits.map_only,
            });
        }
        out
    }

    /// The MC1 (and Hidden Worlds) per-entity presentation rules for
    /// [`World::live_poses`]: unclaimed dwellings are skipped
    /// entirely; body segments are state 120; the (10,45) dwelling
    /// life bar denominates against the parked build value (f44).
    fn live_poses_mc1(&self, e: &Ent) -> PoseGameBits {
        // Houses (m45): the visible building is painted terrain; the
        // entity billboard is the OWNER FLAG (sprite 177 + color row)
        // — drawn only once CLAIMED. MC1 keeps the full skip.
        let unclaimed_house = e.class64 == 10 && e.model65 == 45 && e.f144 == 0;
        if unclaimed_house {
            return PoseGameBits {
                skip: true,
                segment: false,
                life_frac: None,
                blend: 0,
                map_only: false,
            };
        }
        // Body segments hide from map dots + health bars (the heads
        // carry both) — MC1's state 120.
        let segment = e.class64 == 5 && e.tick70 == 120;
        // Class-5 heads + the wizard-family: rival carpets, castles,
        // balloons — all consumed by the opt-in debug bar overlay
        // only. Destructible STRUCTURES join — dwellings (10,45).
        let life_frac = ((e.class64 == 5 && !segment)
            || (e.class64 == 3 && e.model65 <= 3)
            // MC1's (10,45) dwellings: LIVE state 52 only — 51 is the
            // build countdown (act_life counts 30→0 and would read as
            // a dying bar), 53 the collapse.
            || (e.class64 == 10 && e.model65 == 45 && e.tick70 == 52))
            .then(|| {
                // MC1's live dwelling parks act_life at f44
                // (tick_building_live's build finish) and the damage mail
                // drains it; max_life (30) is the BUILD countdown length,
                // not health.
                let denom = if e.class64 == 10 && e.model65 == 45 && e.f44 > 0 {
                    e.f44 as f32
                } else {
                    e.max_life as f32
                };
                if denom <= 0.0 {
                    return 0.0;
                }
                (e.act_life.max(0) as f32 / denom).min(1.0)
            });
        PoseGameBits {
            skip: false,
            segment,
            life_frac,
            blend: 0,
            map_only: false,
        }
    }

    /// The MC2 per-entity presentation rules for [`World::live_poses`]:
    /// unclaimed dwellings export as map-only (never skipped);
    /// segment-hide states 0xB4/0xE8..0xEA; the parked-dwelling life
    /// bar denominates against 1000×rate; translucency blend modes.
    fn live_poses_mc2(&self, e: &Ent) -> PoseGameBits {
        // Houses (m45): MC2 exports the pose as map-only — retail's
        // MAP pass never skips on the claim bit (0xF0F unpossessed
        // dot, GameUI.cpp:1276-95); the claim protocol re-sets sprite
        // 177 + the claimer's color row (AddHouse0A_2D_38330
        // EF:28035-40).
        let unclaimed_house = e.class64 == 10 && e.model65 == 45 && e.f144 == 0;
        // Body segments hide from map dots + health bars — MC2's
        // chain children 0xE8/m27 branches 0xE9-0xEA (retail's own
        // map plot skips exactly 0xB4 + 0xE8..0xEA, GameUI.cpp:1220)
        // and the m22 tail 0xB4. Game-keyed: MC1's 120 is MC2's
        // (5,15) guard brain state.
        let segment = e.class64 == 5 && matches!(e.tick70, 0xB4 | 0xE8..=0xEA);
        // Class-5 heads + the wizard-family + destructible STRUCTURES
        // — dwellings (10,45), building anchors (10,52), castle stage
        // pieces (10,79).
        let life_frac = ((e.class64 == 5 && !segment)
            || (e.class64 == 3 && e.model65 <= 3)
            || (e.class64 == 10 && matches!(e.model65, 45 | 52 | 79)))
        .then(|| {
            // A parked MC2 dwelling's act_life IS its production
            // countdown (mc2_building_tick parks with 1000 x rate;
            // retail CompareEvent08 drains the SAME field on damage):
            // the bar denominates against the parked value, so damage
            // visibly eats it.
            let denom = if e.class64 == 10 && e.model65 == 45 && e.tick70 == 52 && e.f140 > 0 {
                (1000 * e.f140) as f32
            } else {
                e.max_life as f32
            };
            if denom <= 0.0 {
                return 0.0;
            }
            (e.act_life.max(0) as f32 / denom).min(1.0)
        });
        // MC2 translucency (docs/traces/mc2-transparency-
        // drawlist.md): smoke clouds (10,13)/(10,14) carry raster
        // mode 2 from their static particle descriptor
        // (particlesParameters_D951C rows 67/9, byte_10=2); per-entity
        // overrides are flags bit 23 → mode 2 (byte 0xE mask 0x80 —
        // DUAL-PURPOSE: also the m26 wraith's full-speed wake marker,
        // the ghost look IS the state, GRO:3779-3805/EF:19436) and
        // bit 24 → mode 3 (byte 0xF mask 0x01, the 67% death fades).
        // MC1's engine has the same modes but no world content sets
        // them. The bit-23 override is read ONLY by the
        // DrawSprites_3E360 billboard arm; the doomsday pyramid (5,10)
        // — whose faithful ctor flag 0x48800001 carries bit 23 —
        // draws through the sub_3FD60 → DrawSprite_41BD3(2) big-sprite
        // pass (GRO:2205-12, LABEL_70), which takes raster mode from
        // the static descriptor alone, so it stays opaque in retail.
        let blend = if (e.flags & (1 << 23) != 0 && !(e.class64 == 5 && e.model65 == 10))
            || (e.class64 == 10 && matches!(e.model65, 13 | 14))
        {
            2
        } else if e.flags & (1 << 24) != 0 {
            3
        } else {
            0
        };
        PoseGameBits {
            skip: false,
            segment,
            life_frac,
            blend,
            map_only: unclaimed_house,
        }
    }

    /// One game turn (`sub_41780_41AC0`, :52197). `player` feeds the
    /// trigger volume probes, creature awake checks and aggro scans;
    /// `cmd` is the rest of the player's tick input (fire).
    pub fn tick(&mut self, player: PlayerPose, cmd: PlayerCommand) {
        // One global LCG draw per tick, before any handler (:52223).
        lcg32(&mut self.g.rand);

        // The top-of-screen notification decays on its own clock (retail
        // decrements the per-player message life each frame, clearing at
        // zero). Presentation transient — hash-excluded, so this never
        // perturbs the goldens.
        if let Some(n) = &mut self.notification {
            n.timer = n.timer.saturating_sub(1);
            if n.timer == 0 {
                self.notification = None;
            }
        }

        // Broad-phase bucket counts for the kill triggers: class-5
        // events by model, excluding state 120 (multipart body
        // segments in the original; :52246 list building).
        let nb = self.g.chassis.bucket_models;
        let excluded = self.g.chassis.bucket_excluded_states;
        let mut buckets = vec![0u32; nb];
        let mut any_creature = false;
        let mut any_transient = false;
        for e in &self.g.ent {
            if e.class64 == 5 && e.act_life >= 0 && !excluded.contains(&e.tick70) {
                buckets[(e.model65 as usize).min(nb - 1)] += 1;
                any_creature = true;
            }
            if e.class64 == 9
                || (e.class64 == 10 && matches!(e.tick70, 0 | 1 | 5 | 17 | 18 | 21 | 23 | 25 | 41))
            {
                any_transient = true;
            }
        }

        let ctx = MobCtx {
            px: player.x,
            py: player.y,
            pz: player.z,
            pyaw: player.heading,
            pmana: self.player.mana,
        };
        self.human_pose = (player.x, player.y, player.z);

        // The duel pull on the CASTER (:55228-48): while latched,
        // drag the human toward the victim; release at 1000 ticks,
        // 5120 distance, or victim death. Applied through the knock
        // channel (deliberate: magnitude formula traced, transport
        // ours).
        if let Some((victim, count, hold)) = self.duel {
            let ve = &self.g.ent[victim as usize];
            let dead = ve.flags & 0x400 != 0 || ve.act_life < 0 || ve.tick70 != 1;
            let (vx, vy) = (ve.x, ve.y);
            let dist = Gen::isqrt(Gen::dist2_sq(player.x, player.y, vx, vy) as u32);
            if dead || count >= 1000 || dist >= 5120 {
                self.duel = None;
            } else {
                let speed = (player.speed.max(16)) as i32;
                let denom = (1024 / (3 * speed / 2)).max(1);
                let pull = ((dist as i32 - hold as i32) / denom).clamp(0, 3 * speed / 2);
                let yaw = Gen::angle_between(player.x, player.y, vx, vy);
                self.g.player_knock = (yaw, pull.clamp(0, 80) as i16);
                self.duel = Some((victim, count + 1, hold));
            }
        }
        // The MC2 duel enforcement (the lock's per-tick pass) —
        // rides the same knock transport as the MC1 pull above.
        if matches!(self.game, GameId::Mc2) {
            self.mc2_duel_enforce(&player);
        }

        // The per-tick mana census (sub_48230 :56839, called :52327
        // BEFORE all entity ticks).
        self.recompute_mana();

        // The win/objective engine — the ObjectiveVerb seam: MC1's
        // banked-share streak (sub_415C0) vs MC2's stage machine
        // (sub_58F00, single-player types 0/5/7).
        match self.g.verbs.objective {
            ObjectiveVerb::Mc1 => self.objective_mc1(),
            ObjectiveVerb::Mc2 => {
                self.objective_mc2();
                // The objective-message presenter runs right after
                // the stage engine every tick (EF:31817-31818).
                self.speech_ramp_mc2();
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
        // The MC1 hand cast never runs on the MC2 column: ALL MC2
        // casts ride mc2_cast_input. A bound MC1 hand here fires a
        // ghost cast (the dev grant auto-fills player.left).
        if !matches!(self.game, GameId::Mc2) {
            if alive
                && cmd.fire_left
                && let Some(s) = self.player.left
            {
                self.cast_spell(s, false, edge.0, player, &ctx);
            }
            if alive
                && cmd.fire_right
                && let Some(s) = self.player.right
            {
                self.cast_spell(s, true, edge.1, player, &ctx);
            }
        }

        // The MC2 spell column (Phase 4.2): pane selection, the
        // class-15 cast gate (held-button semantics — the gate
        // itself refuses re-arm, sub_5F660), the per-manifestation
        // effect states, and the impact-XP mail drain.
        if matches!(self.game, GameId::Mc2) {
            if let Some((s, t, h)) = cmd.mc2_select {
                self.mc2_select_spell(s, t, h);
            }
            if alive {
                self.mc2_cast_input(edge, (cmd.fire_left, cmd.fire_right));
            }
            self.mc2_cast_tick(player, &ctx);
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

        // The StageVar hold-gate pass (sub_12780 scan + sub_12500
        // react): retail runs it FIRST among the pre-passes —
        // stagevar → awake → drip → entity loop (UpdateEntities
        // EF:40093-40116) — so a released creature is awake-passed
        // and acts the same tick. No-op off MC2/StageVars.
        self.mc2_stagevar_tick();

        // The awake pre-pass runs before dispatch — the AwakeVerb
        // seam: MC1 sub_54F00 (:64266) vs MC2 sub_68BF0/sub_68C70
        // (remc2 :55469).
        match self.g.verbs.awake {
            AwakeVerb::Mc1 => self.g.mob_awake_pass(&ctx),
            AwakeVerb::Mc2 => self.g.mc2_awake_pass(&ctx),
        }

        // The (10,86) cave-drip ambient spawner (`sub_58630`
        // EF:40468, run cave-only each frame from UpdateEntities
        // EF:40113-14): single-player fires on every 8th turn; the
        // probe lands 2560 ahead of the player's facing; a 20×20
        // tile window is walked from GLOBAL-stream offsets — draw #1
        // = the FIRST row's column offset (zeroed for later rows,
        // retail v17 = 0), draw #2 = the row offset; columns step
        // 11; the first empty (type 0) non-sealed tile gets the
        // drip (roster trace §4).
        self.mc2_turn = self.mc2_turn.wrapping_add(1);
        if self.g.is_cave() && self.mc2_turn & 7 == 0 {
            let mut probe = (player.x, player.y, player.z);
            Gen::polar_step(&mut probe, player.heading, 0, 2560);
            let ox = ((probe.0.wrapping_add(128) >> 8) as u8).wrapping_sub(10);
            let oy = ((probe.1.wrapping_add(128) >> 8) as u8).wrapping_sub(10);
            let d1 = features::lcg32(&mut self.g.rand) % 20; // col offset
            let d2 = features::lcg32(&mut self.g.rand) % 20; // row offset
            let mut col0 = d1 as i32;
            let mut row = d2 as i32;
            let mut y = oy.wrapping_add(d2 as u8);
            'drip: while row < 20 {
                let mut col = col0;
                let mut x = ox.wrapping_add(col0 as u8);
                col0 = 0;
                while col < 20 {
                    let t = features::tile(x, y);
                    if self.g.t.tile_type[t] == 0 && self.g.t.angle[t] & 8 == 0 {
                        self.g
                            .mc2_spawn_cave_drip((x as u16) << 8, (y as u16) << 8, 0);
                        break 'drip;
                    }
                    col += 11;
                    x = x.wrapping_add(11);
                }
                row += 1;
                y = y.wrapping_add(1);
            }
        }

        for i in 1..self.g.ent.len() {
            if self.g.ent[i].class64 == 0 {
                continue;
            }
            match self.g.ent[i].class64 {
                // The creature brain/movement family — the
                // MovementVerb seam (the whole class-5 handler
                // column swaps per game: MC1 creature_tick vs MC2
                // mc2_creature_tick, remc2 :40116).
                // The (5,10) doomsday pyramid drives world globals
                // (the apocalypse latch, the doom meter) — its
                // machine lives on World (mc2::doomsday), actions
                // 80..=87.
                5 if matches!(self.game, GameId::Mc2)
                    && self.g.ent[i].model65 == 10
                    && matches!(self.g.ent[i].tick70, 80..=87) =>
                {
                    self.mc2_doomsday_tick(i, &ctx)
                }
                5 => self.tick_arm_creature(i, &ctx),
                // The projectile family rides the TargetingVerb
                // column (acquire/homing live inside the per-game
                // flight handlers).
                9 => self.tick_arm_projectile(i, &ctx),
                // The MC2 teleporter pad shadows MC1's vortex state
                // (both are 36 — retail action 0x24 vs sub_26A60).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 36 => {
                    self.mc2_portal_tick(i, player)
                }
                10 if self.g.ent[i].tick70 == 36 => self.portal_tick(i, player),
                // MC2 ground fire (0) + big explosion (1) — the route
                // chain (sub_30D50 / AddQuickfair0A_01_30F60).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0 => {
                    if self.g.mc2_fire_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 1 => {
                    self.g.mc2_big_explosion_tick(i)
                }
                // The (10,6) standing ground fire (sub_31760 —
                // state 6 shadowed from MC1's effect band).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 6 => {
                    self.g.mc2_fire6_tick(i, &ctx)
                }
                // MC2 buildings: the 30-tick build action (51) and
                // the parked static building (52). Keyed on the game
                // like the spawn column (tier-3 wiring) so MC1's own
                // village states below keep their handlers.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 51 => {
                    self.tick_arm_mc2_building(i)
                }
                // Parked MC2 building: damage/militia/claim intake
                // (AddHouse0A_2D_38330; the claim = the flag set) and
                // its teardown (RemoveCastleStage).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 52 => {
                    self.g.mc2_house_tick(i)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 53 => {
                    self.mc2_house_collapse(i);
                    self.terrain_dirty = true;
                    self.entities_dirty = true;
                }
                // MC2 smoke columns: the (10,59)/(10,60) emitters
                // (states 0x40/0x41) + their (10,13)/(10,14)
                // particles (state = model).
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 0x40 | 0x41) =>
                {
                    self.g.mc2_smoke_emitter_tick(i)
                }
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 13 | 14 | 0x5E) =>
                {
                    // 0x5E = the (10,87) third puff (sub_4EA60) —
                    // the same sub_32160 law under its own action.
                    self.g.mc2_smoke_particle_tick(i)
                }
                // MC2 water splash (state 5 — shadowed from MC1's
                // effect band) + the one-tick stage marker (0x1F).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 5 => {
                    self.g.mc2_splash_tick(i)
                }
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 0x1E | 0x1F | 0x21 | 0x36) =>
                {
                    // sub_34330/sub_34350/sub_34480/sub_352A0
                    // (EF:24989/:24996/:25046/:25732): the one-tick
                    // markers — (10,28)/(10,29)/(10,31) + the
                    // (10,50) stageTag-0 fallback.
                    self.g.ent[i].flags |= 0x400;
                }
                // The (10,51) traveling ridge/damage beam — the
                // disposition-fired runtime arm (the authored chains
                // settle at load inside mc2_author_chain).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x37 => {
                    if self.g.mc2_load_beam_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                // The MC2 (10,26) duel tether — the grip pass
                // (shadowed from MC1's homing tether action).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 26 => {
                    self.mc2_duel_tether_tick(i);
                }
                // The tail-effect band (mc2::tail): blasts 0x19/0x17,
                // meteor 17, fire trail 15, fire spray 19, aura 0x3B.
                // The (10,52) anchor's 0x38 is retail's EMPTY case
                // (EV:2693) — it rides the fall-through arm.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x19 => {
                    self.g.mc2_blast25_tick(i, &ctx)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x17 => {
                    self.g.mc2_blast23_tick(i, &ctx)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 17 => {
                    self.g.mc2_meteor_tick(i, &ctx)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 15 => {
                    self.g.mc2_fire_trail_tick(i)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 19 => {
                    self.g.mc2_fire_spray_tick(i, &ctx)
                }
                // The (10,38) lightning STORM cloud (Lightning L1/L2):
                // hovers then rains (9,9) beams (mc2::tail).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 40 => {
                    self.g.mc2_storm_tick(i)
                }
                // The (10,11) scorch ring (mc2::tail — the volcano
                // burn / authored lava pools).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 11 => {
                    if self.g.mc2_scorch_ring_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x3B => {
                    self.g.mc2_aura_tick(i)
                }
                // The (10,22) whirlwind head; its model-75 tail
                // nodes (action 82) are EV no-ops the head drags.
                // ONLY action 0x16 = 22 runs the whirlwind driver
                // `sub_33110` (0x214110, strA0 row 0x0016 EF:1624).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 22 => {
                    self.g.mc2_whirlwind_tick(i, &ctx)
                }
                // Action 16 DECIMAL = the (10,16) volcano BOULDER,
                // `sub_32600` (0x213600, row 0x0010 EF:1618) — a
                // separate ballistic machine, NOT the whirlwind driver
                // (action 0x16 = 22): the dec/hex distinction is
                // load-bearing (else volcano rocks sway + play the
                // cyclone sound + grant Whirlwind XP). Shadows MC1's
                // state 16 like the meteor's 17.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 16 => {
                    self.g.mc2_boulder16_tick(i)
                }
                // The dome's summit children (mc2::morph): 18 = the
                // ground-vortex eruption controller (shadows MC1's
                // state 18), 98 = the apocalypse mana rain.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 18 => {
                    self.g.mc2_summit18_tick(i)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 98 => {
                    self.g.mc2_summit91_tick(i)
                }
                // The (10,9) raise-land / apocalypse dome
                // (mc2::morph — the three-phase terrain morph).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 9 => {
                    if self.g.mc2_dome_tick(i, &ctx, self.mc2_apocalypse) {
                        self.terrain_dirty = true;
                    }
                }
                // The (10,71) fissure — the ±1 ground-jitter disc.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x4E => {
                    if self.g.mc2_fissure_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                // The (10,65)/(10,66) one-tick wizard-debuff stamps
                // (actions 0x46/0x47 — the (9,20)/(9,21) lob
                // payloads, mc2::proj).
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 70 | 71) =>
                {
                    self.g.mc2_debuff_stamp_tick(i, &ctx)
                }
                // The (10,67) flood/quake (mc2::flood): action 72 =
                // the dome-morph driver, 73 = the shove hold, 74 =
                // the restore finisher.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 72 => {
                    if self.g.mc2_flood_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 73 => {
                    self.g.mc2_flood_shove_tick(i, &ctx)
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 74 => {
                    if self.g.mc2_flood_finisher_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                // The (10,76) fire-sphere orb hub; its model-77
                // satellites (action 0x54) have NO handler — the hub
                // repositions them.
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x53 => {
                    self.g.mc2_fire_orb_tick(i, &ctx)
                }
                // The (10,89) Cave-In collapse (mc2::cave, action
                // 0x60 — sub_311E0; terrain is the weapon).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x60 => {
                    if self.g.mc2_cave_in_tick(i) {
                        self.terrain_dirty = true;
                    }
                }
                // Headless MC2 states that must NOT reach the MC1
                // class-10 catch-all below (it despawns unknown states,
                // corrupting the orb-satellite corpse chain): 82/84 =
                // the whirlwind tail and orb satellites (retail strA0
                // NULL entries — never dispatched, dragged by their
                // heads), 0x38 = the (10,52) anchor's EMPTY EV case
                // (EV:2693).
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 82 | 84 | 0x38) => {}
                // The (10,63)/(10,64) riser lower/raise triggers —
                // one-shot pokes at the co-located (14,1)'s phase
                // (mc2::riser §6).
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 0x44 | 0x45) =>
                {
                    self.g.mc2_riser_trigger_tick(i)
                }
                // The MC2 castle court (mc2::castle): the (10,42)
                // build painter (action 0x2C) and the (10,79)
                // defender stage piece (action 0x56).
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x2C => {
                    if self.g.mc2_castle_painter_tick(i) {
                        self.terrain_dirty = true;
                    }
                }
                10 if matches!(self.game, GameId::Mc2) && self.g.ent[i].tick70 == 0x56 => {
                    // The human is a scan/fire candidate only while
                    // alive (retail scans the pooled wizard, whose
                    // corpse unlinks; ours lives outside the pool).
                    let hp = (self.player.state == LifeState::Alive).then_some(self.human_pose);
                    self.g.mc2_castle_piece_tick(i, hp)
                }
                // Live village buildings and their collapse — MODEL 45
                // only. State 52/53 is house-exclusive (the construction
                // finish and tick_building_live set it only on m45); the
                // crab egg's model-52 no longer aliases into it (its
                // creator now stamps state 56, below). The castle-
                // demolish fake collapse is a direct call, not this
                // dispatch, so gating on m45 leaves it untouched.
                10 if self.g.ent[i].tick70 == 52 && self.g.ent[i].model65 == 45 => {
                    self.g.tick_building_live(i)
                }
                10 if self.g.ent[i].tick70 == 53 && self.g.ent[i].model65 == 45 => {
                    self.g.tick_building_collapse(i);
                    self.terrain_dirty = true;
                }
                // The crab egg (10,52): incubation timer (56) → hatch
                // (57), which lays a wild m5 crab and self-despawns.
                10 if self.g.ent[i].tick70 == 56 => self.g.tick_egg_incubate(i),
                10 if self.g.ent[i].tick70 == 57 => self.g.tick_egg_hatch(i),
                // Combat effects (fire, spreader, splash, possess
                // flash, lava bomb, blast ring, eruption driver,
                // plume, magnet, hit-flash, steal-flash, storm
                // cloud, mana ball, grave, napalm, collapse magnet,
                // death field, magic mine).
                10 if matches!(
                    self.g.ent[i].tick70,
                    0 | 1
                        | 5
                        | 6
                        | 12
                        | 16
                        | 17
                        | 18
                        | 19
                        | 21
                        | 23
                        | 25
                        | 26
                        | 40
                        | 41
                        | 42
                        | 58
                        | 59
                        | 60
                        | 85
                ) =>
                {
                    if self.g.effect_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                // The cave sculptor band (mc2::cave) — normally
                // settle-consumed at load; these arms serve
                // disposition-fired records and the runtime drips.
                10 if matches!(self.game, GameId::Mc2)
                    && matches!(self.g.ent[i].tick70, 0x57..=0x5D) =>
                {
                    self.tick_arm_mc2_cave_sculptor(i)
                }
                10 => {
                    // The load-time handlers ARE the runtime handlers.
                    self.g.tick(i, Some(&ctx));
                    self.terrain_dirty = true;
                }
                // MC2 switches (the strB0 tick table) — game-keyed
                // like the spawn column; the MC1 trigger family below
                // keeps its handlers.
                11 if matches!(self.game, GameId::Mc2) => self.mc2_switch_tick(i),
                11 => self.trigger_tick(i, player, &buckets),
                // Wizard castles and balloons — owner-generic (id24).
                // MC2 runs its native column (mc2::castle): the
                // three-actionIndex castle (tick70 4/5/6) and the
                // AddBallon_60AB0 balloon.
                3 if matches!(self.game, GameId::Mc2) && self.g.ent[i].model65 == 2 => {
                    self.g.mc2_castle_tick(i)
                }
                3 if matches!(self.game, GameId::Mc2) && self.g.ent[i].model65 == 3 => {
                    self.g.mc2_balloon_tick(i)
                }
                3 if self.g.ent[i].model65 == 2 => self.g.castle_tick(i),
                3 if self.g.ent[i].model65 == 3 => self.g.balloon_tick(i),
                // MC2 rival (AI) wizards — the MC2-native brain
                // column (mc2::rivals); husks with no record stand.
                3 if matches!(self.game, GameId::Mc2) && self.g.ent[i].model65 <= 1 => {
                    self.mc2_rival_entity_tick(i)
                }
                // Rival (AI) wizards; level-authored husks with no
                // rival record stand and render as before.
                3 if self.g.ent[i].model65 <= 1 => self.rival_entity_tick(i),
                // The MC2 class-2 tick column (Phase 4.3): the tree
                // burn ladder, terrain-pinned statics, falling props.
                2 if matches!(self.game, GameId::Mc2) => self.g.mc2_scenery_tick(i),
                // MC2 class-14 special map objects (markers/scroll).
                14 if matches!(self.game, GameId::Mc2) => self.mc2_class14_tick(i),
                // MC2 class-15 spell tokens (the jar pickup states).
                15 if matches!(self.game, GameId::Mc2) => self.mc2_spell_token_tick(i, player),
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
            // MC2's m27 branches/tier-2 segments (0xE9/0xEA) are
            // NULL dispatch entries in retail — their phase clock is
            // the body's manual increment (sub_29A90 EF:19806), so
            // the loop must not double-clock them.
            if !(matches!(self.game, GameId::Mc2)
                && self.g.ent[i].class64 == 5
                && matches!(self.g.ent[i].tick70, 233 | 234))
            {
                self.g.ent[i].f63 = self.g.ent[i].f63.wrapping_add(1);
            }
            if self.g.ent[i].flags & 0x400 != 0 {
                self.free_slot(i);
            }
        }
        if any_creature || any_transient {
            // Creatures/projectiles/effects move: poses refresh.
            self.entities_dirty = true;
        }
        // Gen-internal terrain writes with no dirty-returning arm
        // (the castle downgrade's synchronous un-stamp).
        if self.g.terrain_dirty {
            self.g.terrain_dirty = false;
            self.terrain_dirty = true;
        }
        // Drain the MC2 impact-XP mail the same tick it was pushed
        // (the pool ticks above are the `sub_6D8B0` award sites) —
        // empty again by hash time, like a read mailbox.
        if !self.g.mc2_cast_xp.0.is_empty() {
            let mail = std::mem::take(&mut self.g.mc2_cast_xp.0);
            for (owner, spell, amount) in mail {
                self.mc2_award_xp(owner, spell as usize, amount);
            }
        }
        // Drain the m26 spell-steal mail the same tick (the wraith's
        // roll is pool-side, the human book world-side); the jar's
        // action-78 detach arc starts on its next class-15 tick.
        if !self.g.mc2_steal_mail.0.is_empty() {
            let mail = std::mem::take(&mut self.g.mc2_steal_mail.0);
            for (wraith, hand) in mail {
                self.mc2_spell_steal(wraith, hand);
            }
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
            if self.invincible {
                // Dev god-mode = LIFE immunity only, and it OVERRIDES
                // spawn grace (a tester wants to see hostile effects
                // immediately, not wait out the 100-tick grace). The mana
                // steal (ch3) still DRAINS (and arms its 16-tick regen
                // stall) so the mana economy stays fully testable — a
                // genie can pin you castless and defenseless, you just
                // can't be killed. ch0 physical accumulates for the
                // damage readout and arms the flash/danger, but never
                // costs life or kills.
                if self.g.player_mail[3].1 != 0 {
                    let amt = self.g.player_mail[3].0;
                    self.player.mana = self.player.mana.saturating_sub(amt);
                    self.player.regen_delay = 16;
                }
                if self.g.player_mail[0].1 != 0 {
                    let (amt, src) = self.g.player_mail[0];
                    self.g.player_damage += if self.player.shield {
                        amt as u64 / 4
                    } else {
                        amt as u64
                    };
                    // Positional KNOCKBACK still lands (god-mode is
                    // life-only): a hit shoves the player, so the
                    // kraken's lightning still pushes you OUT.
                    let s = src as usize;
                    if src != 0
                        && src != PLAYER_TARGET
                        && s < self.g.ent.len()
                        && self.g.ent[s].class64 != 0
                    {
                        let dir = Gen::angle_between(
                            self.g.ent[s].x,
                            self.g.ent[s].y,
                            player.x,
                            player.y,
                        ) & 0x7FF;
                        self.g.player_knock = (dir, ((amt / 10) as i16).clamp(0, 80));
                    }
                    self.player.hit_flash = 5;
                }
                if self.g.player_mail.iter().any(|&(_, from)| from != 0) {
                    self.g.player_danger = 100;
                }
                self.g.player_mail = [(0, 0); 6];
            } else if self.player.grace > 0 {
                // The spawn-grace memset (:55367-71): every channel
                // wiped, total immunity — steal and grip included, and
                // the danger music stays calm (sub_46540 never runs).
                self.player.grace -= 1;
                self.g.player_mail = [(0, 0); 6];
            } else {
                // The player damage intake — the DamageVerb seam
                // (MC2 adds channels + the spell-XP decorators).
                match self.g.verbs.damage {
                    DamageVerb::Mc1 => self.apply_player_damage(player),
                    DamageVerb::Mc2 => {
                        self.g.note_verb_fallback(VerbKind::Damage);
                        self.apply_player_damage(player);
                    }
                }
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
        if self.g.pal_flash.ticks > 0 {
            self.g.pal_flash.ticks -= 1;
            if self.g.pal_flash.ticks == 0 {
                self.g.pal_flash.row = 0;
            }
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

        // ---- the MC2 level ending (sub_5E8C0_endGameSeq) ----
        // Installed by an ending-marker trip (models 12/31), advanced
        // once per tick; the app mirrors the scripted pose and locks
        // input while active.
        if let Some(target_model) = self.mc2_end_pending.take()
            && self.mc2_endseq.is_none()
        {
            self.mc2_endseq = Some(Mc2EndSeq {
                phase: 0,
                counter: 0,
                speed: player.speed,
                target: 0,
                target_model,
                x: player.x,
                y: player.y,
                z: player.z,
                yaw: player.heading,
            });
        }
        self.mc2_end_tick();

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
            // The MC1/HW win-exit (Space, command 27 :20910/:48804):
            // only while ALIVE and the win flag holds — retail's
            // handler issues cmd 15 BEFORE cmd 27, and a latched
            // revive blocks the win command (:20911), so dead+won+
            // Space revives first and a SECOND Space wins. That
            // ordering falls out here: the Dead arm above consumes
            // the same key. (MC2 ends via the demon-mouth sequence
            // instead — its `completed` only gates the trigger.)
            LifeState::Alive => {
                if cmd.respawn && self.completed && !matches!(self.game, GameId::Mc2) {
                    self.won = true;
                }
            }
        }
        // The village-aggro timer runs down once per wizard tick
        // (:55405-06) — ~200 ticks of militia hostility per offense.
        if self.g.player_aggro > 0 {
            self.g.player_aggro -= 1;
        }
        // Pool wizards' wanted timers (word_0x248_584) run down on
        // the same cadence; a drained entry leaves the map so the
        // hash-quiet side channel returns to silence.
        self.g.mc2_wanted.0.retain(|_, t| {
            *t = t.saturating_sub(1);
            *t > 0
        });

        // Types 2/21 thrust-override factor for the flyer (3.0 while
        // the cast button is held — "hold down the mouse button to
        // achieve maximum speed" — 2.0 after release, negative for
        // backward; :65169/:65175). Computed after the manifestation
        // ticks so an expired burst drops the override the same turn.
        self.player.speed_boost = match self.player.accel {
            0 => 0.0,
            // MC2 Speed: constant per-tier factor (2/3/4) for the
            // whole window — 160/240/320 sustained at ×80 base
            // (docs/spell-audit/speed.md); direction follows the
            // channel sign (accel is +1 forward on the MC2 cast).
            a if self.player.accel_mc2_factor != 0 => {
                self.player.accel_mc2_factor as f32 * a.signum() as f32
            }
            a => (if self.player.accel_held { 3.0 } else { 2.0 }) * a.signum() as f32,
        };
        self.player.accel_held = false;
        self.accel_veto = (false, false);
    }

    // ---- tick() per-class dispatch arm bodies (S1a code motion) ----

    /// Class-5 creature brain/movement (the MovementVerb seam): MC1
    /// `creature_tick` vs the MC2 held-gate + `mc2_creature_tick`.
    fn tick_arm_creature(&mut self, i: usize, ctx: &MobCtx) {
        match self.g.verbs.movement {
            MovementVerb::Mc1 => self.g.creature_tick(i, ctx),
            // A stage-HELD creature (phase 7, site_z 1..=10 or
            // 15) runs `sub_1D5D0`'s held action on World —
            // it needs the StageVar table (mc2::stagevars);
            // metamorph/summon (12/13) and everything else fall
            // through to the per-model machines.
            MovementVerb::Mc2 => {
                if !self.mc2_held_tick(i, ctx) {
                    self.g.mc2_creature_tick(i, ctx);
                }
            }
        }
    }

    /// Class-9 projectile flight (the TargetingVerb seam): MC1/MC1HW
    /// `proj_tick` (terrain-dirtying) vs MC2 `mc2_proj_tick`.
    fn tick_arm_projectile(&mut self, i: usize, ctx: &MobCtx) {
        match self.g.verbs.targeting {
            TargetingVerb::Mc1 | TargetingVerb::Mc1Hw => {
                if self.g.proj_tick(i, ctx) {
                    self.terrain_dirty = true;
                }
            }
            TargetingVerb::Mc2 => self.g.mc2_proj_tick(i, ctx),
        }
    }

    /// The MC2 (10,51) 30-tick building action; a completed build
    /// re-paints terrain and the entity list.
    fn tick_arm_mc2_building(&mut self, i: usize) {
        if self.g.mc2_building_tick(i) {
            self.terrain_dirty = true;
            self.entities_dirty = true;
        }
    }

    /// The MC2 cave sculptor band (0x57..=0x5D): disposition-fired
    /// tube/mesa/dome/pit-hill carves + the runtime drip fallback.
    fn tick_arm_mc2_cave_sculptor(&mut self, i: usize) {
        match self.g.ent[i].tick70 {
            0x57 => self.g.ent[i].flags |= 0x400,
            0x58 => {
                self.g.mc2_tube_carve_tick(i);
                self.terrain_dirty = true;
            }
            0x59 => {
                self.g.mc2_cave_mesa_tick(i);
                self.terrain_dirty = true;
            }
            0x5A => {
                self.g.mc2_cave_dome_tick(i);
                self.terrain_dirty = true;
            }
            0x5B | 0x5C => {
                self.g.mc2_cave_pit_hill_tick(i);
                self.terrain_dirty = true;
            }
            _ => self.g.mc2_cave_drip_tick(i),
        }
    }

    /// The MC1 objective arm (sub_415C0 :52100-40): a wizard WITH a
    /// castle whose banked share of the world total exceeds the level
    /// goal (strictly — `<=` resets, :52128) for
    /// `chassis.win_streak_ticks` consecutive ticks wins. Ours: the
    /// human player only.
    fn objective_mc1(&mut self) {
        if self.win_pct > 0 && !self.completed {
            let over = self.player.world_mana != 0
                && self.player_castle().is_some()
                && 100u64 * self.player.banked as u64 / self.player.world_mana as u64
                    > self.win_pct as u64;
            if over {
                self.win_streak += 1;
                if self.win_streak >= self.g.chassis.win_streak_ticks {
                    self.completed = true;
                }
            } else {
                self.win_streak = 0;
            }
        }
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
                && s < self.g.ent.len()
                && self.g.ent[s].class64 != 0
            {
                let dir = Gen::angle_between(self.g.ent[s].x, self.g.ent[s].y, player.x, player.y)
                    & 0x7FF;
                self.g.player_knock = (dir, ((amt / 10) as i16).clamp(0, 80));
            }
            // Red flash (sub_44BE0(2)), self-panel flash (+392=4,
            // :55723), regen stall, hit sound 17 (:55722-26) — all
            // fire even on a fatal hit.
            self.player.hit_flash = 5;
            self.g.player_alert = 4;
            self.player.regen_delay = 16;
            // The player-hit "ugh" grunt. MC1 uses sound 17; MC2's bank
            // maps 17 to a creature scream (Cymmerian), so MC2 needs its
            // own wizard grunt 54-57 — the same one the debuff-hit path
            // uses (mc2/proj.rs). Vary by attacker id (not the RNG
            // stream, so MC1 goldens are untouched). WATCH: whether a
            // hit WHILE MORPHED swaps to the creature's own hurt cry is
            // unverified — kept as the wizard grunt for now.
            let hit = if matches!(self.g.verbs.damage, DamageVerb::Mc2) {
                54 + (src & 3) as u8
            } else {
                17
            };
            self.g.snd_player(hit);
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
        // uses the world stream (deliberate: same constants; the
        // wizard stream isn't modeled outside flight).
        for s in 0..SPELL_COUNT {
            let m = self.player.owned[s] as usize;
            if m == 0 {
                continue;
            }
            self.player.death_owned[s] = true;
            // The var_916 bank (:55531-35): blue-granted spells come
            // back unrestricted on respawn even if the scattered jar
            // expires meanwhile.
            self.player.death_owned_blue[s] = self.g.ent[m].flags & BLUE_SPELL != 0;
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
        // balls simply stay player-owned) — a benign deviation
        // (deliberate).
        let gz = self.g.ground_z(player.x, player.y) as i16;
        if let Some(gv) = self.g.spawn_grave(player.x, player.y, gz) {
            for j in 1..self.g.ent.len() {
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
        // The respawn screen-mode chime (case 0xF runs sub_3DC90(0)
        // :48640 → sound 14).
        self.g.snd_player(14);
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
                let blue = std::mem::take(&mut self.player.death_owned_blue[s]);
                let m = self.grant_spell(SpellId(s as u8));
                if let (Some(m), true) = (m, blue) {
                    // :54908-12 — the re-grant restores blue: the
                    // unrestricted marker + the blue sprite type.
                    self.g.ent[m].flags |= BLUE_SPELL;
                    self.g.ent[m].type86 = 280;
                }
            }
        }
    }

    // ---- player spells (sub_46B00_46E40 :55851 + the 24 cast arms) --------

    /// Materialize an owned spell: a class-12 manifestation ENTITY in
    /// the pool (the original's sub_3BF70 slot economy — spell
    /// manifestations compete with monsters for slots). tick70 =
    /// [`MANIFEST_BASE`] + spell id; +48 burst counter → our f26,
    /// +44 damage → f44 (count/possess read from the static table).
    /// Auto-fills an empty hand, LEFT first (:49246-54).
    fn grant_spell(&mut self, spell: SpellId) -> Option<usize> {
        // MC1 class-12 manifestations never exist on the MC2 column
        // (the native book owns spells there; the dev/plausible
        // instruments grant through mc2_dev_grant instead). Without
        // this gate the dev toggle would bind player.left with an MC1
        // fireball (a ghost cast).
        if matches!(self.game, GameId::Mc2) {
            return None;
        }
        let id = spell.0 as usize;
        if id >= SPELL_COUNT {
            return None;
        }
        if self.player.owned[id] != 0 {
            return Some(self.player.owned[id] as usize);
        }
        let m = self.g.new_event()?;
        let f44 = self.spells()[id].damage.min(u16::MAX as u32) as u16;
        {
            let e = &mut self.g.ent[m];
            e.class64 = 12;
            e.model65 = spell.0;
            e.tick70 = MANIFEST_BASE + spell.0;
            e.flags &= !8; // never a damage victim
            e.f26 = 0;
            e.f44 = f44;
        }
        // The class-12 ctor sub_3BF70 (:47979-) gives EVERY jar sprite
        // type 77 + a 4x extent override; without it a death-scattered
        // manifestation draws as sprite 0.
        self.g.set_sprite(m, 77);
        let (h4, v4) = {
            let e = &self.g.ent[m];
            (e.f80 * 4, e.f84 * 4)
        };
        self.g.extents(m, h4, v4);
        self.player.owned[id] = m as u16;
        // NO hand binding here: retail's level-init rebuild assigns the
        // hands from the OWNED SET in book order, not in grant order
        // (see `rebind_hands_canonical`). Binding incrementally here
        // made the result depend on the order the app happened to
        // grant in — ascending spell id — which put Heal (1) in the
        // right hand where retail puts Possess (3).
        Some(m)
    }

    /// The level-init hand assignment (`sub_3DD50_3E090` :49193-49254,
    /// HW `hw:45339-45383`): clear both hands, then walk the canonical
    /// BOOK order [`DISPLAY_ORDER`] (`byte_99B88` :5752) and bind the
    /// first owned spell to the LEFT hand and the second to the RIGHT.
    /// Fewer than two owned leaves the remaining hand empty (retail's
    /// 255 sentinel; the input path simply suppresses that button).
    ///
    /// Book order is `[0, 3, 2, 16, 1, ...]` — Fireball, Possess,
    /// Accelerate, Castle, Heal — so a player holding Fireball, Heal
    /// and Possess starts Fireball/Possess, NOT Fireball/Heal: Heal
    /// sits at book position 5, behind three other spells.
    ///
    /// Order-independent by construction, so it does not matter which
    /// order the campaign carry or the plausible instrument grants in.
    /// Mid-level jar pickups do NOT come through here — retail binds
    /// those to the LEFT hand only (:64855), which
    /// [`Self::collect_spell_jar`] already does.
    fn rebind_hands_canonical(&mut self) {
        self.player.left = None;
        self.player.right = None;
        for &s in &DISPLAY_ORDER {
            if self.player.owned[s as usize] == 0 {
                continue;
            }
            if self.player.left.is_none() {
                self.player.left = Some(SpellId(s));
            } else if self.player.right.is_none() {
                self.player.right = Some(SpellId(s));
                break;
            }
        }
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
            // Retail's cheat leaves the hands alone, but this
            // instrument is usually flipped on a spell-less world
            // where they are empty; bind them the same way a level
            // start would so the toggle is usable.
            self.rebind_hands_canonical();
        }
    }

    pub fn dev_spells(&self) -> bool {
        self.dev_spells
    }

    /// Unfaithful improvement (P-class): remove spell jars the local
    /// player already owns (and therefore can never pick up). Faithful
    /// default = off (retail keeps the jars). Applies to both games'
    /// jar systems; the removal is single-player entity deletion, so it
    /// affects the state hash only when enabled (goldens run with it
    /// off). See docs/FIDELITY.md.
    pub fn set_prune_owned_jars(&mut self, on: bool) {
        self.prune_owned_jars = on;
    }

    /// Allocations dropped on pool exhaustion since the last call —
    /// the limit-removing register's telemetry (the app logs it; the
    /// sim itself fails open exactly like the original).
    pub fn take_pool_exhausted(&mut self) -> u32 {
        std::mem::take(&mut self.g.exhausted)
    }

    /// Deterministic digest of the full persistent sim state — the
    /// refactor guard (ROADMAP "MULTI-GAME ARCHITECTURE", Phase 1):
    /// golden fixtures pin these hashes across the verb extraction.
    /// Everything persistent hashes — pool internals, LCG states,
    /// mailboxes — so divergence can't hide behind the observable
    /// surface; the ignored fields are per-tick transients (dirty
    /// flags, pending moves, drained queues). The full destructure
    /// makes a new `World` field a compile error here: extend the
    /// hash deliberately.
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let World {
            g,
            game,
            placeholders,
            mc2_stages,
            mc2_stage_current,
            mc2_stagevars,
            mc2_sv_held,
            mc2_sv_deferred,
            mc2_objective_pause,
            mc2_speech_ramp,
            mc2_speech_cue: _,
            mc2_apocalypse,
            mc2_doom_meter,
            mc2_doom_level,
            mc2_turn: _,
            table,
            terrain_dirty: _,
            entities_dirty: _,
            pending_teleport: _,
            player,
            rivals,
            mc2_rivals,
            kill_tally,
            human_pose,
            rival_deaths: _,
            duel,
            mc2_book,
            start_markers,
            win_pct,
            win_streak,
            completed,
            dev_spells,
            prune_owned_jars: _,
            prev_fire,
            accel_veto: _,
            pending_respawn: _,
            pending_restart: _,
            invincible,
            notification: _,
            won,
            mc2_endseq,
            mc2_end_pending: _,
            mc2_duel,
        } = self;
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        g.hash(&mut h);
        table.hash(&mut h);
        player.hash(&mut h);
        rivals.hash(&mut h);
        kill_tally.hash(&mut h);
        human_pose.hash(&mut h);
        duel.hash(&mut h);
        start_markers.hash(&mut h);
        (
            win_pct, win_streak, completed, dev_spells, prev_fire, invincible,
        )
            .hash(&mut h);
        (game, placeholders).hash(&mut h);
        // The ending latches — hash-transparent until a level ending
        // actually runs (field tags per the aliasing discipline),
        // so every pinned golden is unmoved.
        if *won {
            h.write_u8(0xE0);
        }
        if let Some(seq) = mc2_endseq {
            h.write_u8(0xE1);
            seq.hash(&mut h);
        }
        if let Some(d) = mc2_duel {
            h.write_u8(0xE2);
            d.hash(&mut h);
        }
        // Hashed only when populated: MC1 worlds keep their goldens
        // across this MC2-only layout addition.
        if !mc2_stages.is_empty() {
            mc2_stages.hash(&mut h);
            mc2_stage_current.hash(&mut h);
            mc2_objective_pause.hash(&mut h);
            mc2_speech_ramp.hash(&mut h);
            // Hash-transparent while clear (the bldgprm/spells
            // precedent). Each contribution is preceded by a distinct
            // FIELD TAG: without one, adjacent conditional fields of
            // equal width alias (apocalypse=true/doom_level=false and
            // its mirror feed identical byte streams). The tags live
            // INSIDE the conditions, so worlds where the fields are
            // clear (every pinned golden) are unmoved.
            if *mc2_apocalypse {
                h.write_u8(0xA1);
                mc2_apocalypse.hash(&mut h);
            }
            if *mc2_doom_meter != 0 {
                h.write_u8(0xA2);
                mc2_doom_meter.hash(&mut h);
            }
            if *mc2_doom_level {
                h.write_u8(0xA3);
                mc2_doom_level.hash(&mut h);
            }
        } else if *mc2_speech_ramp != 0 {
            // `set_mc2_stages` arms the ramp even when ZERO rows
            // register, and `speech_ramp_mc2` then pushes chime sounds
            // into the hashed `sounds` vec on a stage-less MC2 world —
            // the driver must be hash-visible wherever its side effects
            // are. Transparent-while-clear AND appended after the
            // (empty) stages gate, so every existing pin's byte stream
            // is unchanged.
            mc2_speech_ramp.hash(&mut h);
        }
        // The MC2 spell book (Phase 4.2): pristine = transparent
        // (MC1 goldens hold; every MC2 world touches it via the
        // fireball+possess baseline, so it always hashes there).
        if !mc2_book.is_pristine() {
            mc2_book.hash(&mut h);
        }
        // The MC2 rival column (Phase 4.3b): hash-gated on presence
        // — every pre-rivals golden stands.
        if !mc2_rivals.is_empty() {
            mc2_rivals.hash(&mut h);
        }
        // The MC2 StageVar subsystem (the hold-gate layer): hash-gated
        // on presence so MC1 + StageVar-less MC2 goldens are untouched.
        if !mc2_stagevars.is_empty() {
            mc2_stagevars.hash(&mut h);
            mc2_sv_held.hash(&mut h);
            // Deferred m9 holds join only when live (own gate: an
            // empty vec must not shift the pinned stagevar goldens).
            if !mc2_sv_deferred.is_empty() {
                mc2_sv_deferred.hash(&mut h);
            }
        }
        h.finish()
    }

    /// The world's game profile and tier-5 verb column (fixed at
    /// construction; replays record them with the chassis).
    pub fn game(&self) -> GameId {
        self.game
    }
    pub fn verbs(&self) -> VerbSet {
        self.g.verbs
    }

    /// Enable the placeholder billboard for unknown authored things —
    /// the seam's graceful-degradation visual (cross-game content /
    /// MC2 dev worlds; a faithful world leaves it off).
    /// MC2 non-day environment (Night/Cave level): the runtime terrain
    /// repaint inverts relief shading (remc2 Terrain.cpp:2030-2033).
    /// Set by the app from the level's gfx environment; no-op on MC1.
    /// Also applies retail's LevelInit SPELLS rows-4/19 patch, which
    /// keys on exactly the same Day vs non-Day split
    /// ([`crate::mc2::spells::level_init_patch`]; empty table = MC1 or
    /// pre-spells bundle → no-op, hash-transparent).
    pub fn set_mc2_night_shade(&mut self, on: bool) {
        self.g.mc2_night_shade = crate::engine::features::NightShade(on);
        crate::mc2::spells::level_init_patch(&mut self.g.assets.spells, !on);
    }

    /// The doom-palette level bit (`byte_0x2FED2 & 2` — the
    /// night-fog gfx variant): gates the (5,10) doomsday pyramid's
    /// ctor. Set by the app alongside the bundle pick; no-op on MC1.
    pub fn set_mc2_doom_level(&mut self, on: bool) {
        self.mc2_doom_level = on;
    }

    pub fn set_placeholders(&mut self, on: bool) {
        self.placeholders = on;
    }

    /// The verbs whose requested arm is pending and fell back to the
    /// MC1 implementation (seam telemetry — the app logs it once).
    pub fn verb_fallbacks(&self) -> Vec<&'static str> {
        VerbKind::ALL
            .iter()
            .filter(|k| self.g.verb_fallbacks & (1 << **k as u8) != 0)
            .map(|k| k.name())
            .collect()
    }

    /// Unknown `(class, model, count)` things the spawn seam refused
    /// since construction (the graceful-degradation ledger).
    pub fn misfits(&self) -> &[(u16, u16, u32)] {
        &self.g.misfits
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
        // Retail binds the hands from the finished owned set, so this
        // runs once after the whole batch — never per grant.
        self.rebind_hands_canonical();
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

    /// The effective per-cast mana cost of a spell RIGHT NOW. Only
    /// castle (16) is dynamic: retail rewrites the manifestation's +136
    /// to the capacity ladder at the OWN castle's current level on every
    /// init/level-up (sub_47C60/sub_47DD0), so the real cost climbs with
    /// the castle — and the HUD availability dots (sub_23D40 :27703)
    /// divide the pool by this LIVE +136, not the static table. A fresh
    /// castle (none built) keeps the ctor 1000. Every other spell's cost
    /// is its static possess-mana. Shared by the cast gate and
    /// [`World::loadout`] so the shown dots can never drift from what a
    /// cast actually charges.
    pub(crate) fn spell_cast_cost(&self, id: usize) -> u32 {
        if id >= SPELL_COUNT {
            return 1;
        }
        if id == 16 {
            return self
                .player_castle()
                .map(|c| Gen::CASTLE_CAP[self.g.ent[c].f26.clamp(0, 7) as usize] as u32)
                .unwrap_or(SPELLS[16].possess_mana);
        }
        self.spells()[id].possess_mana
    }

    /// One hand's cast trigger — the port of sub_46B00_46E40 :55851 +
    /// LABEL_32 :55892, simplified per the agreed interim semantics.
    /// Gate: owned && mana covers the possess cost && the
    /// manifestation's burst counter (+48 → f26) is 0. On trigger:
    /// burst = count, emit ONCE, deduct possess/count — the authored
    /// per-shot deduction remc1 ships commented out by its maintainer
    /// (:64946-50, a known mis-fix pattern); we implement it.
    ///
    /// Trigger classes. Retail's latch re-issues a held cast only for
    /// spells whose manifestation +60 == 0 (:20601/:20621; ctor
    /// :47981, per-thunk args :48020-160) — that set is exactly
    /// {2, 15, 21, 23}. 1/4/5/12/14 are edge-only (NOT
    /// hold-to-channel):
    /// - 23 Rapid Fireball: hold-to-autofire — held fire re-arms the
    ///   window every tick (:20627-30), one emission per game tick
    ///   (the firehose).
    /// - 15 Lightning Bolt: hold = continuous stream that keeps
    ///   emitting at its burst pacing (manual: "hold down the mouse
    ///   for a continuous stream"); a dry stream needs a re-click.
    /// - 2/21 Accelerate fwd/back: hold-to-channel toggles.
    /// - Everything else (incl. 0 Fireball, and the 1/4/5/12/14
    ///   channels): EDGE-triggered — one cast per press, release +
    ///   re-press to renew, still paced by the burst counter.
    fn cast_spell(&mut self, spell: SpellId, right: bool, edge: bool, p: PlayerPose, ctx: &MobCtx) {
        let id = spell.0 as usize;
        if id >= SPELL_COUNT {
            return;
        }
        let m = self.player.owned[id] as usize;
        if m == 0 {
            return;
        }
        let def = &self.spells()[id];

        // 23: the firehose.
        if id == 23 {
            if !self.spell_gate(id, def) {
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
            // spell's identity.
            self.g.snd_player(9);
            self.cast_fireball(p, right, id);
            return;
        }

        let armed = self.g.ent[m].f26 > 0;

        // The hold-to-channel toggles re-arm while held — retail's
        // +60==0 latch set minus 15/23, i.e. the two Accelerates
        // ONLY (:20601/:20621). Heal/Shield/Beyond-Sight/Invis/
        // Rebound are +60==1 → edge-only, handled below (I2 ruling).
        if matches!(id, 2 | 21) {
            // The Accelerate brake veto (manual: "press the down
            // cursor to cancel"): a resisting thrust input this tick
            // keeps the channel down ([`World::thrust_cancel`]).
            if (id == 2 && self.accel_veto.0) || (id == 21 && self.accel_veto.1) {
                return;
            }
            if !armed {
                if !self.spell_gate(id, def) {
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

        // Edge-triggered casts (15 streams while held). A LIVE BURST
        // DOES NOT GATE THE RE-CAST: `sub_46B00_46E40` (:55851) reloads
        // the manifestation's +48 burst counter (`var_48 = var_50`,
        // LABEL_32 :55893) on EVERY fire, for every hand spell — the
        // only type it hard-gates on a live +48 is CASTLE (16, :55903:
        // `if (var_48) buzz`), which we handle in its own block below.
        // So fireball (and the rest) refire as fast as the player can
        // click, each recast resetting the burst; the re-arm's negative
        // mana delta keeps regen suppressed for the whole stream — the
        // "activity blocks regen but not re-fire" law.
        //
        // 15 Lightning held stream: the retail latch re-issues the
        // cast every held tick (+60==0, :20626-32) but ONLY while the
        // burst is live (+48 > 0) — a dry stream stays dry until a
        // fresh click (no auto-resume). The re-issued cast's mana
        // check is SILENT (:55890 — no buzz, no debit, no reload on
        // empty); the re-arm itself is free, the stream's cost is the
        // per-shot debit at cost/count (the firehose idiom), which
        // also keeps regen suppressed for the stream's duration.
        if !edge && id == 15 {
            if !armed {
                return; // dry stream: re-click to restart
            }
            if !self.dev_spells && self.player.mana < def.possess_mana {
                return; // silent (:55890)
            }
            self.mana_debit(def.possess_mana / def.count as u32);
            self.g.ent[m].f26 = def.count as i16;
            self.break_cloak(id);
            self.emit_spell(id, m, p, right, ctx);
            return;
        }
        if !edge {
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
            if self.castle_lock_active() {
                self.g.snd_player(29); // the pinned-charge fizzle
                return;
            }
            let cost = self.spell_cast_cost(id);
            if !self.dev_spells && self.player.mana < cost {
                return; // silent (:55908-10)
            }
            self.mana_debit(cost);
            self.g.ent[m].f26 = def.count as i16;
            self.break_cloak(id);
            self.emit_spell(id, m, p, right, ctx);
            return;
        }
        if !self.spell_gate(id, def) {
            self.g.snd_player(29); // cast-blocked buzz
            return;
        }
        self.mana_debit(def.possess_mana);
        self.g.ent[m].f26 = def.count as i16;
        self.break_cloak(id);
        self.emit_spell(id, m, p, right, ctx);
    }

    /// Casting any other spell breaks the cloak (the +16 0x20 bit
    /// clears with the manifestation's burst).
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
        // against the wizard's own entity — full volume): the 9-family
        // covers fireball/rapid (:65079/:66296), earthquake (:65365),
        // duel (:65665), steal mana (:65764), undead (:65980), storm
        // (:66039), wall of fire (:66158); meteor/volcano/crater/castle 15
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
            // 22 Global Death (sub_580A0 :66235): launches the (9,18)
            // bolt immediately — the "silent prime" the player sees
            // is the invisible-feeling flight + the field's life.
            22 => self.cast_bomb(p, right),
            _ => {}
        }
    }

    /// Muzzle placement shared by the hand casts (sub_56090 :65056-)
    /// — 256 units to the casting hand's side, launch height = the
    /// carpet's half-height, reverted when inside terrain.
    pub(crate) fn muzzle(&self, p: PlayerPose, right: bool) -> (u16, u16, i16) {
        use crate::mc1::combat::PLAYER_HH;
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
        let def = &self.spells()[id];
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
            // nearest mana ball (payload in crate::mc1::combat).
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
            // 11 Duel to the Death (:65620): c9 m7 — detonates into
            // the (10,26) tether that follows the victim and
            // broadcasts the ch4 grip (ctor :47116, tick sub_263C0
            // :28949).
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
        let def = &self.spells()[id];
        // Deliberate approximation of the original per-spell launch
        // pitches (:65579-style): the down-arc terrain spells get a
        // fixed downward bias on the pose pitch (engine pitch positive
        // = down).
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
            // The duel tether (ctor :47116: +44 = 200, the ch4
            // per-tick grip amount).
            11 => {
                e.f69 = 26;
                e.f44 = 200;
            }
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
    pub(crate) fn cast_teleport(&mut self, m: usize, p: PlayerPose) {
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

    /// MC2 Castle Teleport (`sub_6AD60` EF:56860) — a real 3-tier
    /// spell keyed on the manifestation tier `life`
    /// (docs/spell-audit/teleport.md): **T0** teleport to the own
    /// castle; **T1** a save/return toggle (to the castle, then back
    /// to where you cast it on the next press); **T2** cycle own
    /// castle → each rival's castle, one hop per cast. All land at
    /// the castle offset `-448` along `(yaw-204)`. The no-castle
    /// fallback is a SILENT random hop; a real castle teleport plays
    /// sound 22. The cycle/return state rides the manifestation's
    /// `f146` (`word_0x96_150`); the T1 saved position reuses the
    /// player's `teleport_return`. (Flight-speed zero on resolve —
    /// retail `speed_0xc_12 = 0` — is a banked follow-up.)
    pub(crate) fn mc2_cast_teleport(&mut self, m: usize, p: PlayerPose) {
        let tier = self.g.ent[m].f71;
        let yaw = p.heading;
        let mut castle_ok = false;
        match tier {
            0 => {
                if let Some(c) = self.player_castle() {
                    self.mc2_teleport_to(c, yaw);
                    castle_ok = true;
                } else {
                    self.mc2_teleport_random(m, p);
                }
                self.g.ent[m].f146 = 0;
            }
            1 => {
                if let Some(c) = self.player_castle() {
                    if self.g.ent[m].f146 == 1 {
                        if let Some((rx, ry)) = self.player.teleport_return.take() {
                            self.pending_teleport = Some((rx as f32 / 256.0, ry as f32 / 256.0));
                            castle_ok = true;
                        }
                        self.g.ent[m].f146 = 0;
                    } else {
                        self.player.teleport_return = Some((p.x, p.y));
                        self.mc2_teleport_to(c, yaw);
                        self.g.ent[m].f146 = 1;
                        castle_ok = true;
                    }
                } else {
                    self.mc2_teleport_random(m, p);
                }
            }
            _ => {
                castle_ok = self.mc2_teleport_cycle(m, yaw);
                if !castle_ok {
                    self.mc2_teleport_random(m, p);
                }
            }
        }
        if castle_ok {
            self.g.snd_player(22);
        }
    }

    /// Stage a relocation to a castle slot at the `-448`/`(yaw-204)`
    /// offset (all MC2 teleport castle branches).
    fn mc2_teleport_to(&mut self, c: usize, yaw: u16) {
        let e = &self.g.ent[c];
        let mut d = (e.x, e.y, e.z);
        Gen::polar_step(&mut d, yaw.wrapping_sub(204) & 0x7FF, 0, -448);
        self.pending_teleport = Some((d.0 as f32 / 256.0, d.1 as f32 / 256.0));
    }

    /// T2 cycle: advance the `f146` state 0..9 until a reachable
    /// castle — state 0 = own, 1 = skip, 2..8 = the `(state-2)`-th
    /// live rival's castle. One hop per cast.
    fn mc2_teleport_cycle(&mut self, m: usize, yaw: u16) -> bool {
        for _ in 0..9 {
            let state = self.g.ent[m].f146;
            self.g.ent[m].f146 = (state + 1) % 9;
            let castle = match state {
                0 => self.player_castle(),
                1 => None,
                s => {
                    let idx = (s - 2) as usize;
                    let rent = self
                        .mc2_rivals
                        .iter()
                        .filter(|r| !r.eliminated)
                        .nth(idx)
                        .map(|r| r.ent);
                    rent.and_then(|e| self.rival_castle(e))
                }
            };
            if let Some(c) = castle {
                self.mc2_teleport_to(c, yaw);
                return true;
            }
        }
        false
    }

    /// The no-castle random hop (`0x4000` ahead of the current pose),
    /// silent. (Retail's LCG `9377·r+9439` differs from `ent_rand`;
    /// reusing `ent_rand` keeps the fallback deterministic —
    /// deliberate; the exact stream is a banked nicety.)
    fn mc2_teleport_random(&mut self, m: usize, p: PlayerPose) {
        let yaw = (self.g.ent_rand(m) & 0x7FF) as u16;
        let mut d = (p.x, p.y, 0i16);
        Gen::polar_step(&mut d, yaw, 0, 0x4000);
        self.g.ent[m].f146 = 0;
        self.pending_teleport = Some((d.0 as f32 / 256.0, d.1 as f32 / 256.0));
    }

    /// The cast lockout: a castle ball (9,10) or upgrade token
    /// (10,43) still in flight. A STANDING castle no longer locks —
    /// the recast on it is the UPGRADE (:65904-08 morphs the ball
    /// into the token instead of a new castle).
    fn castle_build_lives(&self) -> bool {
        self.g.ent.iter().any(|e| {
            e.flags & 0x400 == 0
                && e.id24 == PLAYER_TARGET
                && ((e.class64 == 9 && e.model65 == 10) || (e.class64 == 10 && e.model65 == 43))
        })
    }

    /// The MC1 castle-spell UPGRADE LOCK: engaged while a cast is in
    /// transit (the (9,10) ball / (10,43) token) OR the player's castle
    /// is mid-transform — any `castle_tick` state other than ESTABLISHED
    /// (`f59 == 4`, the standing/damage-intake tick). Retail pins the
    /// castle-spell manifestation throughout the transform (MC2's
    /// `sub_5F890`; the MC1 decompile is truncated but the port already
    /// runs the between-transformations window in `castle_tick` — the
    /// "dragon-squat" trick), so the lock must follow the WHOLE transform,
    /// not just the ball flight. This is what lets you rebuild in the
    /// split second between an enemy's level-by-level downgrades.
    fn castle_lock_active(&self) -> bool {
        if self.castle_build_lives() {
            return true;
        }
        self.player_castle().is_some_and(|c| self.g.ent[c].f59 != 4)
    }

    /// The player's established castle slot (teleport anchor).
    pub(crate) fn player_castle(&self) -> Option<usize> {
        (1..self.g.ent.len()).find(|&j| {
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
        for j in 1..self.g.ent.len() {
            if k >= roster {
                break;
            }
            let e = &self.g.ent[j];
            if e.class64 == 3 && e.model65 == 3 && e.id24 == PLAYER_TARGET && e.flags & 0x400 == 0 {
                let hp = e.act_life.max(0) as f32 / (e.max_life.max(1) as f32);
                let cargo = e.f140.max(0) as f32 / (e.f136.max(1) as f32);
                out[k] = Some((hp.clamp(0.0, 1.0), cargo.clamp(0.0, 1.0)));
                k += 1;
            }
        }
        out
    }

    /// Arm the human-caster duel pull (the victim intake writes the
    /// ATTACKER's Type_160 u16_314/316/318, :55671-77).
    pub(crate) fn set_duel_latch(&mut self, victim: u16, hold: u32) {
        if self.duel.is_none() {
            self.duel = Some((victim, 0, hold));
        }
    }

    /// SPELLS row 14 (duel) tier params: `(subSpellIndex_2, life)` =
    /// (enforcement RANGE in engine units, DRAIN MODE 0/1/2) —
    /// shipped data 5170/0, 7720/1, 7720/2. Empty table (pre-import
    /// bundle) → a 20-tile range, mode = tier (the data's shape).
    fn mc2_duel_tier(&self, tier: usize) -> (i32, u8) {
        self.g
            .assets
            .spells
            .get(14)
            .map_or((5120, tier.min(2) as u8), |row| {
                let t = row.tiers[tier.min(2)];
                (t.sub_spell, t.life.clamp(0, 2) as u8)
            })
    }

    /// The MC2 (10,26) duel-tether grip pass — the victim-side
    /// resolve (`sub_5EFA0` EF:60643-63) collapsed onto the tether
    /// tick: while the 8-tick tether lives, a rival WIZARD avatar in
    /// tier range is gripped — the caster's LOCK is (re)stamped
    /// {opponent, dist(caster, victim) clamped [1024, 3072]
    /// (EF:60649-56), tier}, +1 duel XP per grip (EF:60657), victim
    /// recoil `word_0x36_54 = 100` (`sub_5EF70` EF:60598). Deliberate
    /// approximation: the retail tether's own grip-write instruction
    /// is not isolable in the symbolic decompile; the grip range used
    /// is the tier's ENFORCEMENT range — a farther grip would dissolve
    /// on the next enforcement pass anyway. Gripping only WIZARDS is
    /// exact (a gripped creature takes the yank path, never a duel —
    /// EF:26097/26369).
    fn mc2_duel_tether_tick(&mut self, i: usize) {
        let life = self.g.ent[i].act_life - 1;
        self.g.ent[i].act_life = life;
        if life < 0 {
            self.g.ent[i].flags |= 0x400;
            self.entities_dirty = true;
            return;
        }
        let tier = self.g.ent[i].f71.min(2);
        let (range, _) = self.mc2_duel_tier(tier as usize);
        let (tx, ty) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best: Option<(u16, u32)> = None;
        for r in &self.mc2_rivals {
            if r.eliminated || r.ent == 0 {
                continue;
            }
            let a = r.ent as usize;
            let Some(e) = self.g.ent.get(a) else { continue };
            if e.class64 != 3 || e.flags & 0x400 != 0 || e.act_life < 0 {
                continue;
            }
            let d = Gen::isqrt(Gen::dist2_sq(tx, ty, e.x, e.y) as u32);
            if (d as i32) < range && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((r.ent, d));
            }
        }
        if let Some((opp, _)) = best {
            let (px, py, _) = self.human_pose;
            let oe = &self.g.ent[opp as usize];
            let dist = Gen::isqrt(Gen::dist2_sq(px, py, oe.x, oe.y) as u32) as i32;
            self.mc2_duel = Some((opp, dist.clamp(1024, 3072), tier));
            self.g
                .mc2_cast_xp
                .0
                .push((crate::mc1::mobs::PLAYER_TARGET, 14, 1));
            self.g.ent[opp as usize].f54 = 100;
        }
    }

    /// The MC2 duel ENFORCEMENT (`sub_5DE30` EF:59889-59947), run
    /// per tick while the lock holds: liveness = the duel
    /// manifestation's charge (EF:59912-16), the opponent alive, and
    /// dist < the tier's `subSpellIndex_2` range — else the duel
    /// ends (`word_0x146_326 = 0`, EF:59947). While held: force-fly
    /// the caster toward the opponent to hold the tether distance,
    /// speed cap 3·minSpeed/2 (EF:59918-29; transport = the knock
    /// channel, the MC1-pull precedent), and DRAIN per the tier's
    /// `life` mode (EF:59930-43): >=1 mana (regen + 8), ==2 also
    /// life (regen + 2, via [`World::mc2_duel_drain`]).
    fn mc2_duel_enforce(&mut self, player: &PlayerPose) {
        let Some((opp, hold, tier)) = self.mc2_duel else {
            return;
        };
        let m = self.mc2_book.ent[14] as usize;
        let live = m != 0 && self.g.ent[m].f26 > 0;
        let (opp_dead, vx, vy) = match self.g.ent.get(opp as usize) {
            None => (true, 0, 0),
            Some(e) => (
                e.class64 != 3 || e.flags & 0x400 != 0 || e.act_life < 0,
                e.x,
                e.y,
            ),
        };
        let eliminated = self.mc2_rivals.iter().any(|r| r.ent == opp && r.eliminated);
        let dist = Gen::isqrt(Gen::dist2_sq(player.x, player.y, vx, vy) as u32) as i32;
        let (range, mode) = self.mc2_duel_tier(tier as usize);
        if !live || opp_dead || eliminated || dist >= range {
            self.mc2_duel = None;
            return;
        }
        let speed = (player.speed.max(16)) as i32;
        let cap = 3 * speed / 2;
        let pull = ((dist - hold) / (1024 / cap).max(1)).clamp(0, cap);
        let yaw = Gen::angle_between(player.x, player.y, vx, vy);
        self.g.player_knock = (yaw, pull.clamp(0, 80) as i16);
        if mode >= 1 {
            self.mc2_duel_drain(opp, mode);
        }
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
        for r in &mut self.rivals {
            r.mana_max = 1000; // the intrinsic base (u32_322, :55031-33)
        }
        // The MC2 column keeps its own roster; retail grows the same
        // per-wizard ceiling there (`maxMana_0x8C_140`, sub_13CE0
        // EF:6135 — the ladder/afford gates read it). Must be credited
        // or it pins at 1000 forever (no castle past rung 1, expensive
        // spells locked).
        for r in &mut self.mc2_rivals {
            r.mana_max = 1000;
        }
        let mut max = 1000u32;
        let mut houses = 0u32;
        // The world total SEEDS with the census caller's intrinsic
        // base (:56867 — u32_188 = a1's u32_322, the human's 1000).
        // Wizard-CARRIED mana is NOT counted: the pool walk admits
        // class 3 models 2/3 only (:56875-78 skips wizards) — every
        // HUD bar is world-relative. MC2's census is its own routine
        // (sub_61F50) and seeds the total at 1 — its type-0 objective
        // divides by this (EF:40751), so the MC1 seed would skew the
        // 15% thresholds.
        let mut world = match self.game {
            GameId::Mc2 => 1u32,
            _ => 1000,
        };
        let mut castle_stored = 0u32;
        for j in 1..self.g.ent.len() {
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
            // Fool's Mana decoys (MC2, f52 = trap owner) are FAKE mana:
            // they must NOT inflate the world-mana denominator, or their
            // permanently-uncollectable share (you can't trip your own
            // trap) dilutes the castle-share goal below reachability
            // (docs/spell-audit/fools-mana.md). MC1 and ordinary balls
            // carry f52 == 0.
            if e.class64 == 10 && e.model65 == 39 && e.f52 != 0 {
                continue;
            }
            let m = e.f140.max(0) as u32;
            world = world.saturating_add(m);
            // Claim owner: mana balls/houses carry it in +144, the
            // wizard-family (castle/balloon) in +24 (:56869-906).
            let owner = if matches!((e.class64, e.model65), (3, 2) | (3, 3)) {
                e.id24
            } else {
                e.f144
            };
            if owner == PLAYER_TARGET {
                max = max.saturating_add(m);
                if e.class64 == 10 && e.model65 == 45 {
                    houses = houses.saturating_add(m);
                }
                if e.class64 == 3 && e.model65 == 2 {
                    castle_stored = castle_stored.saturating_add(m);
                }
            } else if let Some(r) = self.rivals.iter_mut().find(|r| r.ent == owner) {
                r.mana_max = r.mana_max.saturating_add(m);
            } else if let Some(r) = self.mc2_rivals.iter_mut().find(|r| r.ent == owner) {
                r.mana_max = r.mana_max.saturating_add(m);
            }
        }
        self.player.mana_max = max;
        self.player.banked = houses.saturating_add(castle_stored);
        self.player.world_mana = world;
        // The castle overflow ejector reads the house tally
        // (sub_47130 :56185-89 — wizext u32_308).
        self.g.banked_houses = houses.min(i32::MAX as u32) as i32;
    }

    /// The owned manifestation's LIVE castle requirement — the
    /// original's `+132`, which the ctor bakes from the spell table
    /// and a BLUE jar grant zeroes (:64845). Both gates read this
    /// entity field, never the table (:26924, :27860-64), so blue =
    /// req 0 = bindable and castable castle-less. Unowned spells
    /// report the table value (the bind gate greys them anyway).
    fn spell_castle_req(&self, id: usize) -> u32 {
        let m = self.player.owned[id] as usize;
        if m != 0 && self.g.ent[m].flags & BLUE_SPELL != 0 {
            0
        } else {
            self.spells()[id].castle_req
        }
    }

    /// sub_55DD0 (:64909): the cast gate — the castle ladder first
    /// (a nonzero live requirement needs an owned castle STORING at
    /// least that much), then the wizard pool covers the full cost.
    /// The fizzle 29 on failure is the caller's job.
    fn spell_gate(&self, id: usize, def: &crate::mc1::spells::SpellDef) -> bool {
        if self.dev_spells {
            return true;
        }
        let req = self.spell_castle_req(id);
        if req > 0
            && !self
                .player_castle()
                .is_some_and(|c| self.g.ent[c].f140.max(0) as u32 >= req)
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
    pub(crate) fn mana_debit(&mut self, cost: u32) {
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

    /// sub_55E80/sub_68DE0 mid-burst else branch: while a spell burst
    /// is live past its first-fire tick, pin the caster's positive
    /// regen accumulator to 0 so an active spell blocks mana
    /// regeneration (docs/spell-audit/mana-regen.md). The `> 0` guard
    /// preserves a same-frame negative cast debit; dev_spells (the
    /// infinite-mana pin) is exempt.
    pub(crate) fn suppress_regen(&mut self) {
        if !self.dev_spells && self.player.mana_delta > 0 {
            self.player.mana_delta = 0;
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
    pub(crate) fn cast_castle(&mut self, p: PlayerPose) {
        use crate::mc1::combat::PLAYER_HH;
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
        // toward the ground target.
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
        // MC2: stamp the castle research for the stage this cast
        // builds (the A.5 shortcut — retail's research child
        // `sub_69AB0` writes `array_0x24E_590` for castleLevel+1
        // from the researched tier; ours stamps the SELECTED
        // castle-spell tier at cast time). Tier 1 → fire towers,
        // tier 2 → lightning towers, tier 0 → plain walls.
        if matches!(self.game, GameId::Mc2) {
            let stage = castle.map_or(1, |c| (self.g.ent[c].f26 + 1).clamp(1, 7)) as u8;
            let tier = self.mc2_book.sel[2];
            self.g.mc2_research_stamp(PLAYER_TARGET, stage, tier);
        }
        self.entities_dirty = true;
    }

    /// 18 Lightning Storm (sub_579D0 :65988): ONE class-9 m12
    /// carrier launched at the aim (target point 0x4000 ahead;
    /// wizard-homing when rivals exist), becoming the (10,38) storm
    /// cloud on any non-water end — the cloud climbs to ground+1024
    /// and rains 2 bolts/tick for 33 ticks at the spell's 2000.
    fn cast_storm(&mut self, p: PlayerPose) {
        use crate::mc1::combat::PLAYER_HH;
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
        let def = &self.spells()[20];
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

    /// 22 Global Death (sub_580A0 :66235, the state-0x42 manifestation
    /// arm): arm the (9,18) FUSE at the wizard — 21 ticks riding the
    /// caster (the blast lands AROUND THE CASTER) — then the (10,55)
    /// DEATH FIELD in place: 32 more ticks of the sound-43 priming
    /// tick-tock, then ONE instant-kill sweep over the 10-tile 2D
    /// radius (the infinite vertical kill cylinder — sub_299D0, see
    /// combat.rs). Total delay ~53 ticks ≈ 2s. +44 = the row's 7000
    /// (castles in range take it as ch0 mail). Unmodeled from the arm:
    /// the +26 charge byte (326, role unknown), the +150 target point
    /// projected 0x4000 ahead (dead weight in the caster-anchored
    /// reading), the 101-tick/742 mana drain (our economy debits at
    /// cast), and the sub_44BE0 screen flash — banked in ROADMAP;
    /// retail checks owed: blast tracks vs parks, overlapping charges.
    fn cast_bomb(&mut self, p: PlayerPose, right: bool) {
        let (mx, my, mz) = self.muzzle(p, right);
        let Some(pr) = self.g.spawn_bomb_fuse(mx, my, mz) else {
            return;
        };
        let def = &SPELLS[22];
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
        e.f69 = 55;
        self.entities_dirty = true;
    }

    /// Class-12 dispatch: pre-placed JARS wait for pickup; owned
    /// manifestations run their burst countdown + continuous effects.
    fn class12_tick(&mut self, i: usize, ctx: &MobCtx) {
        let t = self.g.ent[i].tick70;
        if t >= MANIFEST_BASE {
            // Rival-owned manifestations (f144 = the owner tag) are
            // driven by the rival tick, not the player's channels.
            if self.g.ent[i].f144 == 0 {
                self.manifestation_tick(i, (t - MANIFEST_BASE) as usize);
            }
            return;
        }
        // A resting jar rides its tile's ground. Retail spawns at
        // ground (:44005) and never legitimately diverges (jars have
        // no gravity and terrain writes ignore class 12, :51729) — but
        // HW's level shaping raised ground over ours (buried) and
        // destroyed ground left ours hovering. Idempotent snap:
        // hash-neutral while z already matches, so MC1 goldens only
        // move where terrain genuinely reshaped under a jar.
        {
            let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
            let gz = self.g.ground_z(x, y) as i16;
            if self.g.ent[i].z != gz {
                self.g.ent[i].z = gz;
                self.entities_dirty = true;
            }
        }
        // Unfaithful improvement (deliberate): a jar whose spell
        // the player already owns can never be picked up (try_pickup's
        // owned gate below) — remove it instead of leaving permanent
        // clutter. Covers both THING-placed (0..=2, life-0 forever) and
        // death-scatter (3) jars; self-culling here handles both the
        // level-load sweep and the tick after the player gains the
        // spell (every jar of it despawns on its next tick).
        if self.prune_owned_jars {
            let spell = self.g.ent[i].model65 as usize;
            if spell < SPELL_COUNT && self.player.owned[spell] != 0 {
                self.g.ent[i].flags |= 0x400;
                self.entities_dirty = true;
                return;
            }
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
    /// no duplicate upgrade (:64843). model65 = spell id CONFIRMED
    /// (off_987DE[+65] dispatch, :64884/:48853; ctor :47983).
    ///
    /// The in-place conversion carries [`BLUE_SPELL`] and type86 280
    /// for free — the original re-applies both on manifest
    /// (:64845/:64897); our entity already holds them.
    fn try_pickup(&mut self, i: usize) {
        let spell = self.g.ent[i].model65 as usize;
        if spell >= SPELL_COUNT || self.player.owned[spell] != 0 {
            return;
        }
        let f44 = self.spells()[spell].damage.min(u16::MAX as u32) as u16;
        {
            let e = &mut self.g.ent[i];
            e.tick70 = MANIFEST_BASE + spell as u8;
            e.flags &= !8;
            e.f26 = 0;
            e.f44 = f44;
        }
        self.player.owned[spell] = i as u16;
        self.player.left = Some(SpellId(spell as u8)); // auto-equip LEFT
        // The pickup chime (:64848 — sound 18 at the wizard).
        self.g.snd_player(18);
        self.entities_dirty = true; // the jar sprite leaves the world
    }

    /// The owned-spell manifestation tick (the class-12 runtime arm):
    /// the burst counter (+48 → f26) decrements once per tick — it is
    /// the refire spacing — and the continuous/toggle effects derive
    /// from it.
    fn manifestation_tick(&mut self, i: usize, spell: usize) {
        // CASTLE (16) is the UPGRADE LOCK, not a timed burst: its `f26`
        // tracks the castle transform (the flying ball / the `f59`
        // build-upgrade-downgrade state machine), NOT the fixed `count`
        // countdown, and it does NOT suppress mana regen — retail's
        // castle effect touches the caster's mana only at cast (the
        // one-shot debit), never during the hold (unlike a channelled
        // spell). Mirrors the MC2 fix (`mc2_castle_spell_tick`).
        if spell == 16 {
            self.g.ent[i].f26 = if self.castle_lock_active() {
                (SPELLS[16].count as i16 - 1).max(1)
            } else {
                0
            };
            return;
        }
        // `sub_55E80` (:64936) mid-burst regen suppression: while a
        // spell burst is live, pin the caster's regen accumulator to
        // 0 — the "an active spell blocks mana regeneration" law
        // (docs/spell-audit/mana-regen.md). The first-fire tick
        // already stamped `mana_delta` negative (`mana_debit`), so
        // the `> 0` guard preserves that per-cast debit and only the
        // positive regen the wizard tick recomputed this frame
        // (world.rs:1225) is clamped away.
        let was_live = self.g.ent[i].f26 > 0;
        if self.g.ent[i].f26 > 0 {
            self.g.ent[i].f26 -= 1;
        }
        if was_live {
            self.suppress_regen();
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
    /// HUD must still reflect it.
    pub fn equip_hands(&mut self, left: Option<SpellId>, right: Option<SpellId>) {
        let mut took = false;
        if let Some(s) = left
            && (s.0 as usize) < SPELL_COUNT
            && self.player.owned[s.0 as usize] != 0
        {
            self.player.left = Some(s);
            took = true;
        }
        if let Some(s) = right
            && (s.0 as usize) < SPELL_COUNT
            && self.player.owned[s.0 as usize] != 0
        {
            self.player.right = Some(s);
            took = true;
        }
        // The equip chime (:48721/:48729 — sound 14 per accepted
        // equip command).
        if took {
            self.g.snd_player(14);
        }
    }

    /// Per-hand crosshair preview (the P-class `crosshair`
    /// instrument): the target each hand's EQUIPPED spell would
    /// acquire if cast this instant, through the pure read-only twin
    /// of the acquire scans ([`crate::engine::features::Gen`]'s
    /// `aim_preview_scan` — no writes, no RNG). Runs from the same
    /// muzzle pose and pitch bias the real cast uses. `None` = the
    /// hand holds no spell, the spell never acquires (quake, crater,
    /// magnet, all non-projectile spells), or nothing is in the cone.
    ///
    /// Honest-instrument caveat: acquisition ≠ hit — homing is capped
    /// at the authentic 5/tick yaw, so a locked fast crosser can
    /// still evade. The marker shows what the shot will CHASE.
    pub fn aim_preview(&self, p: PlayerPose) -> [Option<AimLock>; 2] {
        let hand = |right: bool| -> Option<AimLock> {
            // An MC2-bound hand previews through the MC2 twin (the
            // projectile-side first-tick acquisition — retail MC2
            // has no reticle at all, so this is pure instrument).
            let mc2 = if right {
                self.mc2_book.right
            } else {
                self.mc2_book.left
            };
            if mc2 >= 0 {
                return self.mc2_aim_preview(p, right, mc2 as usize);
            }
            let spell = if right {
                self.player.right?.0
            } else {
                self.player.left?.0
            } as usize;
            use crate::mc1::combat::AimPreviewSet as Set;
            let set = match spell {
                // Fireball, meteor, volcano, lightning, rapid fireball.
                0 | 7 | 8 | 15 | 23 => Set::Creatures,
                3 => Set::Possess,
                // Duel, steal, undead army.
                11 | 13 | 17 => Set::Wizards,
                _ => return None,
            };
            // The down-arc launch bias (cast_projectile): the
            // volcano's acquire cone centers on the biased pitch.
            let pitch = if spell == 8 {
                p.pitch.wrapping_add(0x60) & 0x7FF
            } else {
                p.pitch
            };
            let (mx, my, mz) = self.muzzle(p, right);
            let slot = self.g.aim_preview_scan(mx, my, mz, p.heading, pitch, set)?;
            let e = &self.g.ent[slot as usize];
            Some(AimLock {
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                // The acquire aims at the +78 half-height point.
                alt: e.z.wrapping_add(e.f78 as i16) as f32 / 256.0,
            })
        };
        [hand(false), hand(true)]
    }

    /// Spellbook/HUD snapshot.
    pub fn loadout(&self) -> LoadoutView {
        let mut owned = [false; SPELL_COUNT];
        let mut cooldown = [0f32; SPELL_COUNT];
        let mut cost = [0u32; SPELL_COUNT];
        for s in 0..SPELL_COUNT {
            cost[s] = self.spell_cast_cost(s);
            let m = self.player.owned[s] as usize;
            if m != 0 {
                owned[s] = true;
                cooldown[s] = self.g.ent[m].f26.max(0) as f32 / self.spells()[s].count as f32;
            }
        }
        // One castle scan feeds castle/castle_hp/balloons/bindable.
        let castle_slot = self.player_castle();
        // The :26926 bind gate: the manifestation's LIVE requirement
        // (+132 — zeroed on blue-granted spells) vs the castle's
        // STORED mana (+140). `req == 0` spells are always bindable.
        let castle_stored = castle_slot.map(|c| self.g.ent[c].f140.max(0) as u32);
        let mut bindable = [false; SPELL_COUNT];
        for (s, b) in bindable.iter_mut().enumerate() {
            let req = self.spell_castle_req(s);
            *b = self.dev_spells || req == 0 || castle_stored.is_some_and(|stored| stored >= req);
        }
        LoadoutView {
            owned,
            left: self.player.left.map(|s| s.0),
            right: self.player.right.map(|s| s.0),
            cooldown,
            cost,
            mana: if self.dev_spells {
                self.player.mana_max
            } else {
                self.player.mana
            },
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
    /// carpet speed directly; it propels you forward at maximum speed
    /// and you can't really stop it — merely trying to slow down
    /// cancels the spell). Some(signed
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
        // MC2 Speed: a braking input INTERRUPTS the armed window early
        // (it must be interruptible — otherwise it flies way further
        // than you need). We terminate the window AND zero its burst
        // timer `f26` so mana regen lifts immediately: functional
        // termination clears the burst, otherwise the boost would stop
        // while the window keeps regen pinned off (the effect/timer
        // decoupling). NB the literal decompile `GetScroll_69DB0`
        // hard-overrides speed every tick with no brake input
        // (docs/spell-audit/speed.md §5) — this is the recorded-gameplay
        // interruptibility restored over the trace (deliberate: recorded
        // gameplay is senior). A reverse thrust brakes the
        // (always-forward) boost — the same resisting-only law as MC1
        // Accelerate below (retail's v_14 arms only when the press
        // moves v_12, :55766-80).
        if self.player.accel_mc2_factor != 0 {
            if thrust < 0.0 {
                let m = self.mc2_book.ent[3] as usize;
                if m != 0 {
                    self.g.ent[m].f26 = 0;
                    self.mc2_cast_expire(3, m);
                } else {
                    self.player.accel = 0;
                    self.player.accel_mc2_factor = 0;
                }
            }
            return;
        }
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

    /// The Backspace full-stop's spell kill (retail MC2 action 0x27,
    /// EF:37959-62: clears the `SpellEnabled[3]` manifestation's
    /// `word_0x2E_46`; the spell's drive block is GUARDED on that
    /// counter, so no minSpeed restore ever fires — EF:56203). Ends
    /// whichever accelerate channel is live, either game (the MC1
    /// side is the enhancement mirror of the same law).
    pub fn full_stop_cancel_accel(&mut self) {
        if self.player.accel_mc2_factor != 0 {
            let m = self.mc2_book.ent[3] as usize;
            if m != 0 {
                self.g.ent[m].f26 = 0;
                self.mc2_cast_expire(3, m);
            } else {
                self.player.accel = 0;
                self.player.accel_mc2_factor = 0;
            }
            return;
        }
        match self.player.accel {
            1 => self.stop_accel(2),
            -1 => self.stop_accel(21),
            _ => {}
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
            pal_flash: (self.g.pal_flash.row, self.g.pal_flash.ticks),
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
        if k != 0 && k < self.g.ent.len() && self.g.ent[k].class64 != 0 {
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

    /// Test/debug hook: set the player's mana pool directly (clamped to
    /// the current ceiling). Used to exercise the cast-affordability
    /// gates without scripting a full economy.
    pub fn set_player_mana(&mut self, mana: u32) {
        self.player.mana = mana.min(self.player.mana_max);
    }

    /// Test/debug hook: mark every owned manifestation BLUE (the
    /// blue-jar grant flag, :54908-12), zeroing its live castle
    /// requirement — lets tests exercise the cast/refire laws of
    /// castle-laddered spells without scripting a castle economy.
    pub fn debug_bless_owned_spells(&mut self) {
        for s in 0..SPELL_COUNT {
            let m = self.player.owned[s] as usize;
            if m != 0 {
                self.g.ent[m].flags |= BLUE_SPELL;
            }
        }
    }

    /// Test/debug hook: an MC2 manifestation's CACHED full-cast cost —
    /// the arm gate's word (retail `maxMana_0x8C_140`, our `max_life`,
    /// written by SetSpell). Distinct from the LIVE law
    /// ([`World::mc2_spell_mana_cost`]): the stale-cache class (the
    /// castle-downgrade ding) is exactly a divergence between the two.
    #[doc(hidden)]
    pub fn debug_spell_gate_cost(&self, spell: usize) -> Option<u32> {
        let m = *self.mc2_book.ent.get(spell)? as usize;
        (m != 0).then(|| self.g.ent[m].max_life)
    }

    /// Test/debug hook: raise the player's mana ceiling (and clamp the
    /// pool into it). Lets tests observe large mana credits/debits that
    /// the default 1000 ceiling would otherwise cap.
    pub fn set_player_mana_max(&mut self, mana_max: u32) {
        self.player.mana_max = mana_max;
        self.player.mana = self.player.mana.min(mana_max);
    }

    /// One rival's app-facing snapshot: the book-roster row
    /// (sub_22880 :27009-165) and the map name-label pass
    /// (:57413-48) consume these. Serves both columns — MC1 rivals
    /// and the MC2 rival column, whichever is populated.
    pub fn rival_views(&self) -> Vec<RivalView> {
        let mc1 = self.rivals.iter().map(|r| {
            let e = &self.g.ent[r.ent as usize];
            RivalView {
                slot: r.slot,
                name: crate::mc1::rivals::RIVAL_NAMES[r.slot as usize],
                alive: e.tick70 == 1 && !r.eliminated,
                eliminated: r.eliminated,
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                mana: r.mana,
                mana_max: r.mana_max,
                life_frac: if e.max_life > 0 {
                    (e.act_life.max(0) as f32 / e.max_life as f32).min(1.0)
                } else {
                    0.0
                },
                kills: self.kill_tally[r.slot as usize],
                invisible: r.invisible,
            }
        });
        let mc2 = self.mc2_rivals.iter().map(|r| {
            let e = &self.g.ent[r.ent as usize];
            RivalView {
                slot: r.slot,
                name: crate::mc2::rivals::MC2_RIVAL_NAMES[r.slot as usize],
                alive: e.tick70 == 1 && !r.eliminated,
                eliminated: r.eliminated,
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                mana: r.mana,
                mana_max: r.mana_max,
                life_frac: if e.max_life > 0 {
                    (e.act_life.max(0) as f32 / e.max_life as f32).min(1.0)
                } else {
                    0.0
                },
                kills: self.kill_tally[r.slot as usize],
                invisible: r.invisible,
            }
        });
        mc1.chain(mc2).collect()
    }

    /// The human's kill row of the tally (Type_160+30 on the human).
    pub fn player_kill_row(&self) -> [u16; 8] {
        self.kill_tally[0]
    }

    /// Rival deaths since the last drain (player slots) — the death
    /// message ticker ("%name% is dead", :55499-517).
    pub fn take_rival_deaths(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.rival_deaths)
    }

    /// Beyond Sight is live (v59 = the spell-5 jar's remaining burst,
    /// :57143-46) — gates rival balloon stamps and the rival name
    /// labels on the map.
    pub fn beyond_sight(&self) -> bool {
        self.player.beyond_sight
    }

    /// The live Beyond-Sight TIER (0/1/2) while the spell is armed,
    /// else `None` — the map reveal knob (docs/spell-audit/
    /// beyond-sight.md). MC2 reads it from the class-15 manifestation
    /// (`byte_0x46_70` while `word_0x2E_46 > 0`); the three tiers
    /// reveal progressively more (T0 plain wizards, T1 also through
    /// Invisible, T2 also enemy creatures). MC1 has no tiers — its
    /// bool maps to tier 0.
    pub fn beyond_sight_tier(&self) -> Option<u8> {
        if matches!(self.game, GameId::Mc2) {
            let m = self.mc2_book.ent[12] as usize;
            return (m != 0 && self.g.ent[m].f26 > 0).then(|| self.g.ent[m].f71.min(2));
        }
        self.player.beyond_sight.then_some(0)
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

    // ---- MC2 level init + objective engine ---------------------------------

    /// `GenerateEvents_49290` (remc2 Events.cpp:152-282): the at-load
    /// spawn passes over `DisId == -1` (0xFFFF) records, slot order,
    /// consuming each (`type := 0`). Pass filters verbatim; passes F/G
    /// (buildings split by the bldgprm 0x10 flag) run as one pass
    /// until the building creator + bldgprm data land — every covered
    /// creator is unported today, so each record degrades through the
    /// spawn seam (misfit + optional placeholder), which keeps the
    /// authored population visible.
    fn mc2_generate_events(&mut self) {
        // Passes A..G in slot order. F (:258) = buildings whose
        // BLDGPRM flags set 0x10, G (:271) = the rest; with no
        // bldgprm table (stand-in assets) F takes them all.
        for pass in 0..7 {
            for i in 1..self.table.len() {
                let r = self.table[i];
                if r.class == 0 || r.dis_id != 0xFFFF {
                    continue;
                }
                let hit = match pass {
                    0 => r.class == 10 && r.model == 0x52,
                    1 => {
                        r.class == 10
                            && matches!(
                                r.model,
                                0x9 | 0x53
                                    | 0x54
                                    | 0x55
                                    | 0xB
                                    | 0xF
                                    | 0x1E
                                    | 0x1D
                                    | 0x20
                                    | 0x1F
                                    | 0x33
                                    | 0x32
                                    | 0x58
                            )
                    }
                    2 => r.class == 10 && matches!(r.model, 0x51 | 0x50),
                    3 => r.class == 14 && r.model == 2,
                    4 => r.class == 10 && matches!(r.model, 0x1B | 0x1C),
                    5 | 6 => {
                        r.class == 10 && r.model == 0x2D && {
                            let f = self
                                .g
                                .assets
                                .bldgprm
                                .get(r.parent as usize)
                                .map_or(0x10, |b| b.flags);
                            (f & 0x10 != 0) == (pass == 5)
                        }
                    }
                    _ => false,
                };
                if hit {
                    // The terrain-authoring chains ride their own
                    // machinery instead of a creator (PrepareEvents
                    // cases {0x1C,0x1D,0x1F,0x32,0x50} EV:323-336 →
                    // sub_49090): 0x1D = waterpath, 0x32 = the
                    // (10,51) ridge-beam fence, 0x1C = the road
                    // staircase, 0x1F = the river (retail-inert),
                    // 0x50 = the (10,81) cave tube carver
                    // (docs/traces/mc2-terrain-author-painters.md).
                    if r.class == 10 && matches!(r.model, 0x1C | 0x1D | 0x1F | 0x32 | 0x50) {
                        self.mc2_author_chain(i);
                    } else {
                        self.spawn_from_thing(i);
                    }
                    self.table[i].class = 0;
                }
            }
            // `ApplyEvents_498A0` between passes (EV:161-282): tick
            // the live cave sculptors in slot order to completion —
            // the settle-TICK band (painters trace §4.4). Slot-order
            // rounds mean every animated sculptor's phase-0 terrain
            // SAMPLE happens before any same-pass sculptor's phase-1
            // WRITE, exactly like retail; dead slots are reaped so
            // later passes reuse them (per-entity rand = slot +
            // global stream).
            self.mc2_settle_cave_band();
        }
    }

    /// The load-time settle for the cave sculptor band — the cave
    /// slice of `ApplyEvents_498A0` (EV:410-526). Ticks actions
    /// 0x57..=0x5C in slot order until the band is quiet; action
    /// 0x5D (the drip) is in the settle DISABLE band (EV:516-25) —
    /// a live one despawns without running. No-op off-cave.
    fn mc2_settle_cave_band(&mut self) {
        if !self.g.is_cave() {
            return;
        }
        for _ in 0..1024 {
            let mut live = false;
            for i in 1..self.g.ent.len() {
                if self.g.ent[i].flags & 0x400 != 0 {
                    continue;
                }
                // The (14,2) pillar spawns in its own pass (EV:226-33)
                // and its MEASURE (life 0) runs in the ApplyEvents
                // settle like everything else; grow/retract can only
                // be trigger-fired at runtime.
                if self.g.ent[i].class64 == 14 && self.g.ent[i].tick70 == 7 {
                    if self.g.mc2_pillar_tick(i) {
                        live = true;
                    }
                    continue;
                }
                if self.g.ent[i].class64 != 10 {
                    continue;
                }
                match self.g.ent[i].tick70 {
                    0x57 => self.g.ent[i].flags |= 0x400,
                    0x58 => {
                        self.g.mc2_tube_carve_tick(i);
                        live = true;
                    }
                    0x59 => {
                        self.g.mc2_cave_mesa_tick(i);
                        live = true;
                    }
                    0x5A => {
                        self.g.mc2_cave_dome_tick(i);
                        live = true;
                    }
                    0x5B | 0x5C => {
                        self.g.mc2_cave_pit_hill_tick(i);
                        live = true;
                    }
                    0x5D => self.g.ent[i].flags |= 0x400,
                    _ => continue,
                }
            }
            if !live {
                break;
            }
        }
        for i in 1..self.g.ent.len() {
            if self.g.ent[i].class64 != 0 && self.g.ent[i].flags & 0x400 != 0 {
                self.free_slot(i);
            }
        }
        self.terrain_dirty = true;
    }

    /// The GenerateEvents terrain-authoring chains (remc2 `sub_49090`
    /// EV:5261): a par1/par2-linked list of same-model THINGs walks
    /// to its head, zeroes each node's stage tag (the re-entry guard
    /// — the second node's own generate hit no-ops), and runs the
    /// per-model stamper on each consecutive leg:
    /// - (10,0x1D) waypoint path → `sub_48690` (EV:5493) →
    ///   `ApplyPointToPath_343F0` (EF:25027): two axis-aligned runs
    ///   per leg; per cell, angle class-nibble := 1 (this CLEARS the
    ///   deep-water bit — water becomes walkable ground) + the
    ///   sub_462A0 retile. Level-000's "narrow straight path" from
    ///   the shore to the spire IS this pass. The intermediate (10,30)
    ///   one-shot segment entities are collapsed into a synchronous
    ///   stamp (no RNG, settle-time in retail).
    /// - (10,0x32) fence → `sub_48880` (EV:5586): one (10,51)
    ///   traveling ridge/damage beam per leg, settle-ticked to
    ///   completion (the ApplyEvents loop EV:497-521 keeps
    ///   0x32/0x33 live — collapsed here into an inline run; the
    ///   beam's RNG rides its own entity stream so the collapse is
    ///   draw-exact). The segment's par3 passes through unused for
    ///   0x32 (EV:5326-41 remaps it only for 0x1F/0x50).
    fn mc2_author_chain(&mut self, ti: usize) {
        if self.table[ti].swi_id == 0 {
            return; // stageTag guard (EV:333)
        }
        let model = self.table[ti].model;
        // Walk to the head via par1 links (EV:5308-5313).
        let mut cur = ti;
        while self.table[cur].parent != 0 {
            let p = self.table[cur].parent as usize;
            if p >= self.table.len() || p == cur {
                break;
            }
            cur = p;
        }
        // Walk forward via par2 links, stamping each leg. NO per-node
        // class/model check: retail's guard at EV:5316-19 tests the
        // loop-INVARIANT seed record (`entity`, not the walked
        // `tempEntity`) — trivially true, so the walk crosses nodes
        // of any class, including the passive class-0 rows chains
        // link through. Termination is par2 == 0, plus our bounds.
        let mut hops = 0;
        loop {
            let node = self.table[cur];
            hops += 1;
            if hops > self.table.len() {
                break; // cycle guard (retail would spin; ours exits)
            }
            self.table[cur].swi_id = 0; // stageTag = 0 (EV:5320)
            if node.child == 0 {
                break;
            }
            let next = node.child as usize;
            if next >= self.table.len() {
                break;
            }
            let (nx, ny) = (self.table[next].x, self.table[next].y);
            match model {
                0x1D => self.mc2_stamp_path_leg(node.x, node.y, nx, ny),
                0x32 => self.mc2_stamp_fence_leg(node.x, node.y, nx, ny),
                // (10,28) road: the sub_48400 ridge staircase
                // (docs/traces/mc2-terrain-author-painters.md §1-2).
                0x1C => self.g.mc2_stamp_road_leg(node.x, node.y, nx, ny),
                // (10,31) river: INERT IN RETAIL — sub_487D0 seeds
                // life/yaw/width on a (10,50) whose action 0x36
                // self-destructs without reading them (same doc
                // §3.4, OPEN-1: the carve consumer is a stub; river
                // geometry rides the level header). The walk still
                // consumes the chain's stage tags, faithfully.
                0x1F => {}
                // (10,80) cave tunnel chain → one (10,81) tube
                // carver per leg (sub_48930 EV:5621): packed radii
                // f71 = FROM node's par3 (high nibble) | TO node's
                // par3 (low, EV:5348-52), dest = the TO node;
                // settle-ticked to completion by the pass settle.
                0x50 => {
                    let np3 = self.table[next].par3;
                    let (fx, fy) = (node.x << 8, node.y << 8);
                    if let Some(c) = self.g.mc2_spawn_tube_carver(fx, fy, 0) {
                        let e = &mut self.g.ent[c];
                        e.f71 = ((np3 & 0xF) | ((node.par3 & 0xF) << 4)) as u8;
                        e.dest_x = nx << 8;
                        e.dest_y = ny << 8;
                    }
                }
                _ => {}
            }
            cur = next;
        }
    }

    /// `sub_48880` (EV:5586) + the ApplyEvents settle run: spawn one
    /// (10,51) beam at the FROM node's tile corner (verbatim: x<<8,
    /// no center offset) snapped to terrain, aimed at the TO node,
    /// `life = dist/actSpeed(1024)`, then tick it to completion.
    /// Distance is 2D (retail's EuclideanDistXYZ reads an
    /// uninitialized dest z — a decompile-visible quirk; the 2D form
    /// is the plausible intent, noted). The settle MobCtx stands at
    /// the slot-0 start marker — retail's player entity is at its
    /// start during ApplyEvents; only the load-time player damage
    /// probe sees it (edge case, same observable).
    fn mc2_stamp_fence_leg(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let (fx, fy) = (x1 << 8, y1 << 8);
        let (tx_, ty_) = (x2 << 8, y2 << 8);
        let fz = self.g.ground_z(fx, fy) as i16;
        let Some(b) = self.g.mc2_spawn_load_beam(fx, fy, fz) else {
            return;
        };
        let yaw = Gen::angle_between(fx, fy, tx_, ty_);
        let d2 = Gen::dist2_sq(fx, fy, tx_, ty_);
        let dist = Gen::isqrt(d2 as u32) as i32;
        self.g.ent[b].f30 = yaw;
        self.g.ent[b].act_life = dist / 1024;
        let (sx, sy) = self
            .start_markers
            .iter()
            .flatten()
            .next()
            .copied()
            .unwrap_or((0, 0));
        let ctx = MobCtx {
            px: (sx << 8) | 128,
            py: (sy << 8) | 128,
            pz: 0,
            pyaw: 0,
            pmana: 0,
        };
        while self.g.ent[b].flags & 0x400 == 0 {
            self.g.mc2_load_beam_tick(b, &ctx);
        }
    }

    /// `sub_48690` (EV:5493): one chain leg = the shared diagonal
    /// run (length min(|dx|,|dy|), step (sign dx, sign dy)) then the
    /// remainder run on the longer axis, both from the leg's start —
    /// deltas map-wrapped (`shortestLenght_48370`).
    fn mc2_stamp_path_leg(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let wrap = |a: u16, b: u16| -> i32 {
            let d = (b as i32 - a as i32) & 0xFF;
            if d >= 128 { d - 256 } else { d }
        };
        let (dx, dy) = (wrap(x1, x2), wrap(y1, y2));
        let (xdir, ydir) = (dx.signum(), dy.signum());
        let (ax, ay) = (dx.abs(), dy.abs());
        let diff = (ay - ax).abs();
        let (diag, s2x, s2y) = if ax <= ay {
            (ax, 0, ydir)
        } else {
            (ay, xdir, 0)
        };
        self.mc2_stamp_path_run(x1 as u8, y1 as u8, xdir, ydir, diag);
        let bx = (x1 as i32 + diag * xdir) as u8;
        let by = (y1 as i32 + diag * ydir) as u8;
        self.mc2_stamp_path_run(bx, by, s2x, s2y, diff);
    }

    /// `ApplyPointToPath_343F0` (EF:25027): stamp `len` cells from
    /// the start along the unit step — angle nibble 1 + retile, in
    /// that order per cell.
    fn mc2_stamp_path_run(&mut self, mut cx: u8, mut cy: u8, sx: i32, sy: i32, mut len: i32) {
        while len > 0 {
            let t = crate::engine::features::tile(cx, cy);
            self.g.t.angle[t] = (self.g.t.angle[t] & 0xF0) | 1;
            len -= 1;
            self.g.mc2_retile_region(cx, cy, cx, cy);
            cx = cx.wrapping_add(sx as u8);
            cy = cy.wrapping_add(sy as u8);
        }
    }

    /// Register the level's stage checkpoints (`InitStages_58940`,
    /// remc2 :40567-40647): skip unused (-1) entries; drop the
    /// entity-typed objectives {1,2,4,6,7,9} with no target; every
    /// registered stage activates immediately. `(index, stage, x, y)`
    /// = the package's 7-byte checkpoint rows.
    pub fn set_mc2_stages(&mut self, checkpoints: &[(i8, i16, i16, i16)]) {
        self.mc2_stages.clear();
        self.mc2_stage_current = 0;
        self.mc2_objective_pause = 0;
        // Level load arms the briefing voiceover (LevelInit.cpp:41).
        self.mc2_speech_ramp = 1;
        self.mc2_speech_cue = None;
        for (row, &(index, stage, x, y)) in checkpoints.iter().enumerate() {
            if index < 0 {
                continue;
            }
            // Retail's InitStages "drop typed rows with stage==0" guard
            // is DEAD CODE — its switch selector reads the memset-zero
            // DESTINATION row (`stages_0x3654C[stageIndex].byte0`,
            // EF:40589), never the source type, so it always falls to
            // `default` and EVERY `index != -1` row registers, ACTIVE
            // (state 1, EF:40607-09). 13 shipped levels author such
            // rows; some (type-1/2 at stage 0) bind the empty record 0
            // and are faithfully un-completable — retail leaves them
            // stuck and those levels end by other paths (the model-31
            // X-marker latch). Registration order matches retail's
            // compaction: the baker already removed the `-1` slots, so
            // this enumerate index == retail's compacted row index.
            let target = match index {
                // Type 7 stores the target's MODEL (:40628-30).
                7 => self.table.get(stage as usize).map_or(0, |r| r.model as u32),
                // Type 9 (destroy building) stores the referenced
                // THING's par1 = the BUILDING-TYPE tag (:40611-42,
                // `entity_0x30311[stage_1].par1_14`). par1 is our
                // `Thing.parent`; the spawned (10,45) building carries
                // the same value in `f71` (mc2_spawn_building), so the
                // completion test matches `f71 == target`. (level-001's
                // stage 153 → par1 21 = the two vaults by the tower.)
                9 => self
                    .table
                    .get(stage as usize)
                    .map_or(0, |r| r.parent as u32),
                // Type 3 (kill enemy player) rides InitStages'
                // DEFAULT arm (:40640: `stage_1 - 1`) — the authored
                // payload is the 1-BASED wizard color.
                3 => (stage as i32 - 1).max(0) as u32,
                _ => stage as u32,
            };
            self.mc2_stages.push(Mc2Stage {
                kind: index as u8,
                target,
                point: ((x as u16) << 8, (y as u16) << 8),
                state: 1,
                row: row as u8,
                force: false,
                bound: None,
            });
        }
        // Retroactive bind (`sub_58DA0` at the load boundary): retail
        // runs `InitStages` BEFORE any entity spawns, so a single
        // spawn-time bind suffices there. Our port inverts the order —
        // `new_for_game` fires disposition 0 (the dis==0 THINGs) INSIDE
        // the ctor, before the app hands us the checkpoints here — so a
        // type-1/2 target authored at dis 0 is already live when the
        // stages register. Walk the live pool once to catch those; every
        // later (disposition-fired / stage-gated) spawn binds through the
        // spawn-seam hook in `spawn_from_thing`.
        for i in 1..self.g.ent.len() {
            if self.g.ent[i].class64 != 0 {
                self.mc2_bind_stage_target(i);
            }
        }
    }

    /// `sub_58DA0` (remc2 EF:40650-90) — bind a named-target objective
    /// row (types 1/2) to the live entity it just matched. The retail
    /// template-pointer equality `a1x == &entity_0x30311[stage_1]`
    /// reduces in the port to `thing_slot == target` (the authored
    /// THING index the spawn seam stamps on every entity). Retail
    /// re-points UNCONDITIONALLY on EVERY matching spawn (EF:40656-63:
    /// no already-bound guard, no state gate) — the row tracks the
    /// NEWEST instance, so a respawning named template must be killed
    /// in its latest incarnation. Types 4/6 share this seam in
    /// retail but stay unported (types 4/6 = 0 shipped levels; type 6
    /// is un-completable in retail too — see
    /// docs/traces/mc2-objective-types-1-2-4-6.md).
    fn mc2_bind_stage_target(&mut self, slot: usize) {
        let ti = self.g.ent[slot].thing_slot;
        if ti == 0 {
            return;
        }
        for st in &mut self.mc2_stages {
            if matches!(st.kind, 1 | 2) && st.target == ti as u32 {
                st.bound = Some(slot as u16);
            }
        }
    }

    /// The bound named target is no longer a LIVE instance of its
    /// authored THING — the completion condition for objective types
    /// 1/2 (retail reads `ptr0x6E8E->life_0x8 <= -1`, EF:40765/40773).
    /// Our slots recycle through a LIFO free list, so a raw life read of
    /// the bound slot could observe a REUSED entity; we anchor on the
    /// `thing_slot` identity instead. "Gone" = the slot was freed
    /// (`class64 == 0`), reused by a different THING (`thing_slot !=
    /// target`), dead-marked (`flags & 0x400`), or negative-life —
    /// every terminal, one-way transition of the original creature.
    fn mc2_bound_gone(&self, slot: u16, target: u32) -> bool {
        self.g.ent.get(slot as usize).is_none_or(|e| {
            e.class64 == 0
                || e.thing_slot as u32 != target
                || e.flags & 0x400 != 0
                || e.act_life <= -1
        })
    }

    /// The stage board for the HUD/tests: the current-stage cursor +
    /// each registered stage's (objective type, state 1|2).
    pub fn mc2_objective_view(&self) -> (usize, Vec<(u8, u8)>) {
        (
            self.mc2_stage_current,
            self.mc2_stages.iter().map(|s| (s.kind, s.state)).collect(),
        )
    }

    /// Live world positions of every target of the CURRENT objective
    /// stage — the data behind MC2's objective-guide overlay (the
    /// flashing map/minimap marks + the nearest-target arrow, which
    /// retail cannot disable). Reuses the exact predicates in
    /// [`Self::objective_mc2`]: type 5 = the authored fly-to point;
    /// type 7 = every live class-5 of the target model (dwelling-spawned
    /// stragglers included, since it re-enumerates each call); type 9 =
    /// every live `(10,45)` whose `f71` tag is in the degradation chain;
    /// types 1/2 = the single bound entity. Non-spatial stages (0 mana,
    /// 3/8 kill-players) yield nothing to point at → empty. The closest
    /// piece to the human is flagged `nearest` (the arrow anchor), using
    /// a torus-wrapped metric — DELIBERATELY unlike the type-5 latch,
    /// which is retail's plain sign-extended abs; the overlay is a UI
    /// heuristic and the short way round is the useful arrow. Tile
    /// units. A read-only view — no hash/golden impact.
    pub fn mc2_objective_targets(&self) -> Vec<ObjectiveTarget> {
        let mut out = Vec::new();
        let Some(st) = self.mc2_stages.get(self.mc2_stage_current) else {
            return out;
        };
        if st.state != 1 {
            return out;
        }
        // Retail outline colour: YELLOW only for the fly-to point
        // (type 5), RED for creature/building targets.
        let yellow = st.kind == 5;
        let mut push = |x: u16, y: u16| {
            out.push(ObjectiveTarget {
                x: x as f32 / 256.0,
                z: y as f32 / 256.0,
                nearest: false,
                yellow,
            });
        };
        match st.kind {
            // Fly-to point (the authored checkpoint itself is the mark).
            5 => {
                if st.point != (0, 0) {
                    push(st.point.0, st.point.1);
                }
            }
            // Kill-by-MODEL: mirror objective_mc2's type-7 live test.
            7 => {
                for e in self.g.ent.iter().skip(1) {
                    if e.class64 == 5
                        && e.model65 as u32 == st.target
                        && e.act_life >= 0
                        && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                        && e.flags & 0x400 == 0
                    {
                        push(e.x, e.y);
                    }
                }
            }
            // Destroy-building: rebuild the degradation chain (identical
            // walk to objective_mc2's type-9 arm) then plot every live
            // (10,45) still in it.
            9 => {
                let mut chain = [0u32; 8];
                let mut n = 0;
                let mut j = st.target;
                while j != 0 && n < 8 {
                    chain[n] = j;
                    n += 1;
                    j = self
                        .g
                        .assets
                        .bldgprm
                        .get(j as usize)
                        .map_or(0, |b| b.chain as u32);
                    if chain[..n].contains(&j) {
                        break;
                    }
                }
                let chain = &chain[..n];
                for e in self.g.ent.iter().skip(1) {
                    if e.class64 == 10
                        && e.model65 == 45
                        && e.flags & 0x400 == 0
                        && chain.contains(&(e.f71 as u32))
                    {
                        push(e.x, e.y);
                    }
                }
            }
            // Kill-named (bound entity): plot the one live bound target.
            1 | 2 => {
                if let Some(b) = st.bound
                    && !self.mc2_bound_gone(b, st.target)
                    && let Some(e) = self.g.ent.get(b as usize)
                {
                    push(e.x, e.y);
                }
            }
            _ => {}
        }
        // Flag the closest piece as the arrow anchor (torus-wrapped —
        // an overlay-only heuristic; the type-5 LATCH is retail's
        // plain sign-extended abs, EF:40803-14).
        let (px, py, _) = self.human_pose;
        let (pxf, pyf) = (px as f32 / 256.0, py as f32 / 256.0);
        let span = crate::MAP_TILES as f32;
        let wrap = |d: f32| {
            let d = d.abs();
            d.min(span - d)
        };
        if let Some((best, _)) = out.iter().enumerate().min_by(|(_, a), (_, b)| {
            let da = wrap(a.x - pxf).hypot(wrap(a.z - pyf));
            let db = wrap(b.x - pxf).hypot(wrap(b.z - pyf));
            da.total_cmp(&db)
        }) {
            out[best].nearest = true;
        }
        out
    }

    /// `sub_58F00_game_objectives` (remc2 :40693), the single-player
    /// subset for types 0/5/7 (level-000's set): every tick, test
    /// active stages — type 0 anywhere in the list, types 5/7 only
    /// while CURRENT; on satisfaction advance the cursor to the next
    /// active stage or latch completion (`IsLevelEnd_0` → our
    /// `completed`).
    fn objective_mc2(&mut self) {
        if self.completed || self.mc2_stages.is_empty() {
            return;
        }
        // The pause head (:40724-27): an m32 switch fire skips the
        // next pass (see trigger model 32).
        if self.mc2_objective_pause > 0 {
            self.mc2_objective_pause -= 1;
            return;
        }
        let mut achieved = false;
        for idx in 0..self.mc2_stages.len() {
            // The external force-complete bit, ahead of the state
            // gate like retail (:40737-42).
            if self.mc2_stages[idx].force {
                self.mc2_stages[idx].force = false;
                self.mc2_stages[idx].state = 2;
                achieved = true;
                continue;
            }
            let st = self.mc2_stages[idx];
            if st.state != 1 {
                continue;
            }
            let done = match st.kind {
                // Castle-banked mana share ≥ target % (:40746-61).
                // NB `>=`, where MC1's banked check is strictly `>`.
                0 => {
                    self.player.world_mana != 0
                        && self.player_castle().is_some()
                        && 100u64 * self.player.banked as u64 / self.player.world_mana as u64
                            >= st.target as u64
                }
                // Fly-to-point, current stage only: |dx|,|dy| ≤ 768
                // engine units = 3 tiles (:40803-14). Retail's metric
                // is ONE plain abs over sign-extended int16 operands —
                // no torus/shortest-wrap min (its seam discontinuity is
                // the genuine quirk). The guide overlay keeps its torus
                // metric deliberately (UI heuristic, not the latch).
                5 => {
                    idx == self.mc2_stage_current && {
                        let (px, py, _) = self.human_pose;
                        let dx = ((st.point.0 as i16 as i32) - (px as i16 as i32)).abs();
                        let dy = ((st.point.1 as i16 as i32) - (py as i16 as i32)).abs();
                        dx <= 768 && dy <= 768
                    }
                }
                // Kill-THING = the target MODEL is extinct: the
                // per-model live list head is null (:40828-34;
                // bytearray_38403x skips the multipart states).
                // CURRENT stage only (:40827 `v3 == ObjectiveText_1`)
                // — without the cursor gate the row latches
                // vacuously at load, before its dis-gated targets
                // ever spawn (level-000's firefly wave, row 4).
                7 => {
                    idx == self.mc2_stage_current
                        && !self.g.ent.iter().skip(1).any(|e| {
                            e.class64 == 5
                                && e.model65 as u32 == st.target
                                && e.act_life >= 0
                                && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                                && e.flags & 0x400 == 0
                        })
                }
                // Kill enemy player (:40780-86): the target COLOR's
                // alive-flag (`byte_0x006_2BE4_11236`) is clear —
                // our rival's `eliminated`. A color that never
                // spawned (>= NumberOfPlayers dead data) reads as
                // vacuously done, like retail's zeroed stat block.
                3 => {
                    let color = st.target as u8;
                    color != 0
                        && !self
                            .mc2_rivals
                            .iter()
                            .any(|r| r.slot == color && !r.eliminated)
                }
                // Kill ALL players (:40835-50): every other color's
                // alive-flag clear. Current stage only.
                8 => idx == self.mc2_stage_current && self.mc2_rivals.iter().all(|r| r.eliminated),
                // DESTROY BUILDING (:40851-75): no live class-10
                // model-45 building whose `byte_0x46_70` (= our `f71`,
                // the par1 the ctor stamped) is in the target's
                // DEGRADATION CHAIN remains. Current stage only (like
                // 5/7/8) — the m32 stage-gated switch (mc2_switch_tick
                // model 32) fires the vault-spawn disposition + the
                // 1-tick objective pause when the PRIOR row completes,
                // so the buildings exist before this row is first
                // tested (no vacuous latch).
                //
                // THE CHAIN (EF:40851-75): a razed building does not
                // vanish — its state-53 `mc2_house_collapse` spawns a
                // FRESH tag-`bldgprm[N].chain` stage (the next BLDGPRM
                // byte_3 link) unless the link is 0. So the objective
                // needs EVERY stage in the chain gone, not just par1.
                // level-001's vault: par1 21 → 54 → 0, so each vault
                // takes two hits (tag 21 collapses to tag 54, which
                // collapses to nothing). Walk ≤8 links like retail,
                // with a cycle guard for the self-loop rows (a row
                // whose byte_3 points at itself never chains onward).
                // Retail scans only every 16th frame
                // (`!(FrameTimingIndex_26 & 0xF)`, EF:40852) — the
                // one objective with a frame gate; `mc2_turn` is the
                // port's hash-excluded frame counter.
                9 => {
                    idx == self.mc2_stage_current && self.mc2_turn & 0xF == 0 && {
                        let mut chain = [0u32; 8];
                        let mut n = 0;
                        let mut j = st.target;
                        while j != 0 && n < 8 {
                            chain[n] = j;
                            n += 1;
                            j = self
                                .g
                                .assets
                                .bldgprm
                                .get(j as usize)
                                .map_or(0, |b| b.chain as u32);
                            if chain[..n].contains(&j) {
                                break;
                            }
                        }
                        let chain = &chain[..n];
                        !self.g.ent.iter().skip(1).any(|e| {
                            e.class64 == 10
                                && e.model65 == 45
                                && e.flags & 0x400 == 0
                                && chain.contains(&(e.f71 as u32))
                        })
                    }
                }
                // KILL NAMED CREATURE (type 1, EF:40763-70): the row's
                // bound entity is dead. Requires the `& 1` bound bit
                // (`bound.is_some()`, set by `sub_58DA0`) so it cannot
                // fire before the target spawns. NOT current-stage gated
                // — a background kill row, like type 0. A morph husk
                // counts as gone (type 1 accepts any death).
                1 => st.bound.is_some_and(|b| self.mc2_bound_gone(b, st.target)),
                // KILL FOR REAL (type 2, EF:40771-79): life <= -1 AND
                // `!fontTypeIndex_0x3D_61` — reject a death that is a
                // mid-degradation handoff. Every shipped type-2 target
                // is a (10,45) BUILDING, and building degradation IS a
                // slot swap: `mc2_house_collapse`'s chain branch spawns
                // the byte_3 successor in a fresh slot and re-points
                // this row's `bound` to it (`sub_59760`, EF:40921-54).
                // A building's fontTypeIndex ≡ `bldgprm[type].byte_3`
                // (sub_49A30 EF:32794-98), never written elsewhere, so
                // the port reads the chain byte directly: a dead
                // building with a successor PENDING (chain != 0, the
                // pre-collapse window before the re-point runs) is not
                // done; only the FINAL stage (chain == 0) completes.
                // No `thing_slot` identity term here (unlike type 1):
                // the re-point deliberately moves `bound` to successor
                // slots whose thing_slot differs from the authored
                // target, and it keeps the binding live-tracked, so
                // the LIFO-reuse window type 1 guards against is
                // closed by the re-point cadence instead.
                2 => st.bound.is_some_and(|b| match self.g.ent.get(b as usize) {
                    None => true,
                    Some(e) => {
                        let dead = e.class64 == 0 || e.flags & 0x400 != 0 || e.act_life <= -1;
                        dead && (!(e.class64 == 10 && e.model65 == 45)
                            || self
                                .g
                                .assets
                                .bldgprm
                                .get(e.f71 as usize)
                                .map_or(0, |bp| bp.chain)
                                == 0)
                    }
                }),
                // Types 4/6 need the same bind seam plus (4) a
                // player-owner check / (6) an item carry-slot inventory
                // that nothing in the decompile writes — both authored
                // in ZERO shipped levels; kept unported
                // (docs/traces/mc2-objective-types-1-2-4-6.md §3/§4).
                _ => false,
            };
            if done {
                self.mc2_stages[idx].state = 2;
                achieved = true;
            }
        }
        if achieved {
            // Advance to the next still-active stage or end the
            // level (:40881-98).
            let cur_done = self
                .mc2_stages
                .get(self.mc2_stage_current)
                .is_some_and(|s| s.state == 2);
            match self.mc2_stages.iter().position(|s| s.state == 1) {
                Some(next) => self.mc2_stage_current = next,
                None => self.completed = true,
            }
            // The objective-message trigger (EF:40899/40911): armed
            // when the CURRENT row completed or the level ended —
            // never by a background row latching out of turn. The
            // chimes (41 pre-cue / 61 Success2) and the voiceover
            // ride the ramp in `speech_ramp_mc2`.
            if cur_done || self.completed {
                self.mc2_speech_ramp = 1;
            }
        }
    }

    /// `PresentObjective_59820`'s speech-enabled arm (EF:40957-41066;
    /// docs/traces/mc2-voiceover-triggers.md §3): walk the
    /// `byte_0x36E02` ramp — ~7 quiet ticks, the sound-41 pre-cue at
    /// step 7, the actual voiceover cue + the sound-61 advance chime
    /// at step 8 (61 suppressed while the cursor sits at row 0 — the
    /// briefing), then the long quiet tail to 0xC8. The retail
    /// fade-in gate (`paletteMod_51 >= 3`) has no analog here — we
    /// have no load fade. OPEN: the type-31 beacon variant
    /// (`byte_0x36E0B & 1` → secret-row speech + chime 41) waits on
    /// the beacon switch port.
    fn speech_ramp_mc2(&mut self) {
        match self.mc2_speech_ramp {
            0 => {}
            7 => {
                self.mc2_speech_ramp = 8;
                self.g.snd_player(41); // pre-cue chime (EF:41058)
            }
            8 => {
                self.mc2_speech_ramp = 9;
                if self.mc2_stage_current != 0 {
                    self.g.snd_player(61); // advance chime (EF:41019)
                }
                // Segment = objective row + 1, or 9 at level end
                // (EF:41031-44). The app owns the row = level-number
                // half of the address.
                self.mc2_speech_cue = Some(if self.completed {
                    9
                } else {
                    (self.mc2_stage_current as u8).saturating_add(1)
                });
            }
            0xC8 => self.mc2_speech_ramp = 0,
            _ => self.mc2_speech_ramp += 1,
        }
    }

    // ---- dispositions ----------------------------------------------------

    /// sub_37440_37800 (:43924): spawn every live THING whose dis_id
    /// matches; one-shot consumes the records. (The disId-0 mana
    /// recount is the mana track's concern and omitted.)
    fn fire_disposition(&mut self, dis: u16, one_shot: bool) {
        // sub_4A1E0 (EF:32967): firing a disposition arms every kind-7
        // StageVar keyed to it (a no-op when no StageVars are loaded).
        self.mc2_stagevar_arm_disposition(dis);
        for i in 1..self.table.len() {
            if self.table[i].class != 0 && self.table[i].dis_id == dis {
                self.spawn_from_thing(i);
                if one_shot {
                    self.table[i].class = 0;
                }
            }
        }
        // sub_4A1E0 tail (EF:32994): re-arm the watch-by-model gates.
        self.mc2_stagevar_rearm_watchers();
    }

    /// The MODEL of the THING at `slot` in the level table (used by the
    /// StageVar loader to resolve subtype/extinction references).
    pub(crate) fn mc2_table_model(&self, slot: usize) -> Option<u8> {
        self.table.get(slot).map(|r| r.model as u8)
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

        // The spawn seam's graceful degradation (ROADMAP "MULTI-GAME
        // ARCHITECTURE"): a `(class, model)` outside the serving
        // registry ([`GameId::known_thing`]) is counted as a misfit —
        // and, under `placeholders`, stands in as a marker-stone
        // billboard — never a crash. Known non-entities (start
        // markers, null creators) pass through to their authentic
        // no-spawn arms below.
        if !self.game.known_thing(r.class, r.model) {
            self.g.note_misfit(r.class, r.model);
            if self.placeholders
                && let Some(s) = self.g.spawn_scenery(4, x, y, z)
            {
                self.g.ent[s].thing_slot = ti as u16;
                self.entities_dirty = true;
            }
            return;
        }

        // The per-game spawn column (tier-3 wiring). MC2's creator
        // table (str_D4C48ar, remc2 Events.cpp:5186) grows one entry
        // per ported creator — the known_thing gate above admits
        // exactly the ported set.
        let slot = match self.game {
            GameId::Mc1 | GameId::Mc1Hw => match r.class {
                2 => self.g.spawn_scenery(r.model, x, y, z),
                3 => self.g.spawn_class3(r.model, x, y, z),
                5 => self.g.spawn_creature(r.model, x, y, z),
                10 => self.g.spawn_creator(r.model, x, y, z),
                11 => self.spawn_trigger(r.model, x, y, z),
                7 | 9 | 12 => self.spawn_inert(r.class, r.model, x, y, z),
                _ => None,
            },
            GameId::Mc2 => match (r.class, r.model) {
                // The wizard start-position markers (sub_4A820..
                // EF:33259): retail records array_0x2362[N] and
                // spawns nothing — the app reads the (3,4) record
                // for the human's spawn point directly.
                (3, 4..=11) => None,
                (2, 0) => self.g.mc2_spawn_tree(x, y, z),
                (2, 1) => self.g.mc2_spawn_stone(x, y, z),
                (2, 2) => self.g.mc2_spawn_dolmen(x, y, z),
                (2, 3) => self.g.mc2_spawn_scenery3(x, y, z),
                (2, 4 | 5) => self.g.mc2_spawn_scenery45(r.model as u8, x, y, z),
                // Cave-only: retail's own off-cave no-spawn arm until
                // Phase 4.5 boots caves.
                (2, 6) => self.g.mc2_spawn_cave_bee(x, y, z),
                (2, 7 | 8) => self.g.mc2_spawn_falling(r.model as u8, x, y, z),
                (5, 0) => self.g.mc2_spawn_m0(x, y, z),
                (5, 1) => self.g.mc2_spawn_goat(x, y, z),
                (5, 2) => self.g.mc2_spawn_m2(x, y, z),
                (5, 3) => self.g.mc2_spawn_m3(x, y, z),
                (5, 4) => self.g.mc2_spawn_archers(x, y, z),
                (5, 9) => self.g.mc2_spawn_m9(x, y, z),
                // The doomsday pyramid (mc2::doomsday). Retail's
                // ctor NULLs out unless the doom-palette level bit
                // is set (EF:33968); our gate runs on the machine's
                // FIRST TICK instead — dis-0 records spawn during
                // construction, before the app can deliver the
                // level bit (set_mc2_doom_level).
                (5, 10) => self.g.mc2_spawn_doomsday(x, y, z),
                (5, 12) => self.g.mc2_spawn_m12(x, y, z),
                (5, 13) => self.g.mc2_spawn_villager(x, y, z),
                (5, 14) => self.g.mc2_spawn_m14(x, y, z),
                // Never authored (the castle guard respawn is its
                // one retail launch site) — completeness with the
                // known_thing registry.
                (5, 15) => self.g.mc2_spawn_m15(x, y, z),
                (5, 16) => self.g.mc2_spawn_m16(x, y, z),
                (5, 17) => self.g.mc2_spawn_m17(x, y, z),
                (5, 18) => self.g.mc2_spawn_m18(x, y, z),
                (5, 19) => self.g.mc2_spawn_m19(x, y, z),
                (5, 20) => self.g.mc2_spawn_m20(x, y, z),
                (5, 21) => self.g.mc2_spawn_m21(x, y, z),
                // par1 = the tail length (sub_4A310 EF:33025-28).
                (5, 22) => self.g.mc2_spawn_m22(x, y, z, r.parent),
                (5, 23) => self.g.mc2_spawn_m23(x, y, z),
                (5, 24) => self.g.mc2_spawn_m24(x, y, z), // cave-only, no-spawn today
                (5, 25) => self.g.mc2_spawn_m25(x, y, z),
                (5, 26) => self.g.mc2_spawn_m26(x, y, z),
                (5, 27) => self.g.mc2_spawn_m27(x, y, z),
                (5, 28) => self.g.mc2_spawn_m28(x, y, z),
                (9, 13) => self.g.mc2_spawn_arrow(x, y, z),
                (10, 0) => self.g.mc2_spawn_fire(x, y, z),
                (10, 1) => self.g.mc2_spawn_big_explosion(x, y, z),
                // The water splash (NewAdd0A05_4E570 EF:35436).
                (10, 5) => self.g.mc2_spawn_splash(x, y, z),
                // The standing ground fire (NewAdd0A06_4E5F0
                // EF:35458); no par fields consumed (EF:33051-54 —
                // stage-bind only).
                (10, 6) => self.g.mc2_spawn_fire6(x, y, z),
                // The MC2 teleporter pad (sub_4FE40 EF:36506); the
                // par1/par2 destination lands in the shared (10,34)
                // post-init below (EF:33077 — same tile-center math
                // as MC1's :44024).
                (10, 34) => self.g.mc2_spawn_portal(x, y, z),
                // The chain markers' stageTag-0 fallbacks — one-tick
                // self-destructs (the live chains route through
                // mc2_author_chain and never reach the spawn seam,
                // EV:323-336): (10,28) road / (10,31) river
                // (sub_4F800/:36170, sub_4FAC0/:36311) + the (10,50)
                // fence (sub_4FDE0/:36488; its par-driven life
                // scaling EF:33095-33104 is dead against the
                // one-tick action, trace §1.6).
                (10, 28) => self.g.mc2_spawn_stage_marker_for(28, 0x1E, x, y, z),
                (10, 31) => self.g.mc2_spawn_stage_marker_for(31, 0x21, x, y, z),
                (10, 50) => self.g.mc2_spawn_stage_marker_for(50, 0x36, x, y, z),
                // A raw authored (10,51) beam (pass-2 list EV:190;
                // life 0 = one stamp then gone, settle-run below).
                (10, 51) => self.g.mc2_spawn_load_beam(x, y, z),
                // The tail-effect band (mc2::tail). (10,8)'s creator
                // is literally `return 0` (sub_4E750 EF:35507) — a
                // known no-spawn record, not a misfit. Authored
                // (10,11)/(10,15) par1 SPELLS.DAT overrides land in
                // the post-init below ((10,17) has no authored
                // override — EV:387's case list is 9/0xB/0xF only).
                (10, 8) => None,
                // The raise-land dome (mc2::morph); retail's ctor
                // clears the apocalypse latch (EF:35527) — the latch
                // lives on World, so the clear rides the call site.
                (10, 9) => {
                    self.mc2_apocalypse = false;
                    self.g.mc2_spawn_dome(x, y, z)
                }
                (10, 11) => self.g.mc2_spawn_scorch_ring(x, y, z),
                (10, 15) => self.g.mc2_spawn_fire_trail(x, y, z),
                (10, 17) => self.g.mc2_spawn_meteor(x, y, z),
                (10, 23) => self.g.mc2_spawn_blast23(x, y, z),
                (10, 25) => self.g.mc2_spawn_blast25(x, y, z),
                (10, 52) => self.g.mc2_spawn_castle_anchor(x, y, z),
                (10, 54) => self.g.mc2_spawn_aura(x, y, z),
                (10, 22) => self.g.mc2_spawn_whirlwind(x, y, z),
                // The (10,67) flood/quake (mc2::flood; the ctor
                // sub_51730 consumes no THING fields — the triggered
                // par1 seam lands in the post-init below).
                (10, 67) => self.g.mc2_spawn_flood(x, y, z),
                (10, 71) => self.g.mc2_spawn_fissure(x, y, z),
                (10, 76) => self.g.mc2_spawn_fire_orb(x, y, z),
                // The cave sculptor band (mc2::cave, Phase 4.5) —
                // cave-only ctors. (10,80..85) settle at load (the
                // per-pass mc2_settle_cave_band); (10,86) drips are
                // in NO generate pass (EV:161-282) — latent unless
                // disposition-fired; the live ambience is the
                // runtime spawner (sub_58630, roster port).
                (10, 80) => self.g.mc2_spawn_cave_marker80(x, y, z),
                (10, 81) => self.g.mc2_spawn_tube_carver(x, y, z),
                (10, 82) => self.g.mc2_spawn_cave_mesa(x, y, z),
                (10, 83) => self.g.mc2_spawn_cave_dome(x, y, z),
                (10, 84) => self.g.mc2_spawn_cave_pit(x, y, z),
                (10, 85) => self.g.mc2_spawn_cave_hill(x, y, z),
                (10, 86) => self.g.mc2_spawn_cave_drip(x, y, z),
                // Smoke particles authored directly (their global-RNG
                // life roll survives only on this path).
                (10, 13 | 14 | 87) => self.g.mc2_spawn_smoke_particle_for(r.model as u8, x, y, z),
                // The one-tick invisible stage/quest marker
                // (sub_4FA00 EF:36274) — the "beacon" record;
                // disposition-fired instances only (the generate-pass
                // chains are consumed by mc2_waypoint_chain).
                (10, 29) => self.g.mc2_spawn_stage_marker(x, y, z),
                // The "quest point" smoke-column emitters
                // (docs/traces/mc2-class10-m59-m60.md): no THING
                // fields consumed (EF:33107 — models 0x37..0x3C get
                // only the stage binding).
                (10, 59 | 60) => self.g.mc2_spawn_smoke_emitter(r.model as u8, x, y, z),
                // The riser's lower/raise triggers (mc2::riser §6) —
                // authored in the same map cell as their (14,1)/(14,2),
                // dis-gated for stage-scripted open/close.
                (10, 63 | 64) => self.g.mc2_spawn_riser_trigger(r.model as u8, x, y, z),
                // The authored ground mana economy: (10,39) 512-mana
                // spheres + the (10,58) 2560 variant — both yield a
                // model-39 ball (CreateManaSphere_500C0).
                (10, 39 | 57 | 58) => self.g.mc2_spawn_mana_sphere(r.model as u8, x, y, z),
                // par1 (our `parent`) = the BUILD00/BLDGPRM id.
                (10, 45) => self.g.mc2_spawn_building(x, y, z, r.parent),
                // Switches (AddSwitchXX_50A90 :37059): state = model,
                // never map-linked, no sprite — invisible by
                // construction, like MC1's volumes. The record's
                // id/extents land in the shared (11, _) post-init
                // below (≡ remc2's class-11 case :33198-33207:
                // id = stageTag_12, ShiftRot(word_10 << 8, 4096);
                // model 32 instead stores par1 = its stage row).
                // Phase 4.3 adds the slot-condition band 12..=44
                // (docs/traces/mc2-class11-switches-class14.md);
                // 5..=11 stay misfits (handlers OPEN in the trace).
                (11, 0..=4 | 12..=44) => self.spawn_trigger(r.model, x, y, z),
                // Class-14 special map objects (creator sub_514E0
                // :37315 + the per-model sub-creators :37332-37418).
                // The ENDING fly-to markers (3 = the checkpoint X,
                // 4 = the demon mouth) spawn HIDDEN when dis-gated:
                // player-verified retail shows the portal only once
                // the ending trigger is tripped (the trip reveals it
                // — the entity must pre-exist as the endGameSeq
                // fly-to target). Authored-at-load (dis 0) markers
                // stay visible (mid-level guidance X's).
                (14, 0..=5) => self.spawn_mc2_class14_gated(r, x, y, z),
                // Class-15 spell tokens — THE SPELL JARS (one shared
                // ctor for all 26 spells, mc2::tokens). The swi_id
                // state bump lands in the post-init below.
                (15, 0..=25) => self.g.mc2_spawn_spell_token(r.model as u8, x, y, z),
                _ => None,
            },
        };
        let Some(s) = slot else { return };
        self.g.ent[s].thing_slot = ti as u16;
        // `sub_58DA0` (EF:40650-90): bind any named-target objective row
        // (types 1/2) whose authored THING index matches the entity we
        // just spawned. Covers every post-load spawn (disposition-fired,
        // stage-gated waves); the load-time dis-0 spawns are caught by
        // the retroactive pass in `set_mc2_stages`.
        if !self.mc2_stages.is_empty() {
            self.mc2_bind_stage_target(s);
        }
        // `sub_12100` (EF:4684-4750): a class-5 spawn may be HELD by a
        // StageVar (hold-gate layer). Load-time dis-0 spawns are caught
        // by the retroactive pass in `set_mc2_stagevars`.
        if r.class == 5 && !self.mc2_stagevars.is_empty() {
            self.mc2_stagevar_attach(s, ti);
        }
        if r.class == 11 {
            // Trigger volumes feed the map overlay, not billboards.
            self.entities_dirty = true;
        }

        // Post-init (:44017-44050). NOTE the original's branch shape:
        // classes BELOW 11 get nothing except the class-10 models 4
        // (spawner volume), 34 (portal) and 45 (building); exactly
        // class 11 gets id24/extents; class 12 the state bump.
        match (r.class, r.model) {
            (12, _) => self.spawn_postinit_class12_jar(s, r),
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
            // MC1 (:43707 sub_36DF0): the build-table id is par1+16.
            (10, 45) if matches!(self.game, GameId::Mc1 | GameId::Mc1Hw) => {
                self.g.building_fixup(s, r.parent.wrapping_add(16));
            }
            // MC2 (remc2 EF:33089, the v4 == 0x2D case): the id is
            // par1 RAW — sub_49A30 already ran inside the ctor — and
            // par2 lands in xtype_0x41_65. MC1's fixup must NOT run
            // here (it clobbers f71 to par1+16, making every building
            // the spire template).
            (10, 45) => {
                self.g.ent[s].f66 = (r.child & 0xFF) as u8;
            }
            // The par1-authored SPELLS.DAT subspell overrides
            // (PrepareEvents EV:387-390, `case 0x09/0x0B/0x0F` only;
            // ≡ sub_4A310's bottom block EF:33163-70): par1 picks the
            // tier of row GetSpellIndex(model) — subSpell always;
            // model 9 writes maxLife (the dome radius/height driver),
            // 11/15 write life. Empty table = pre-import bundle →
            // the ctor-default approximation stands.
            (10, 9 | 11 | 15) if matches!(self.game, GameId::Mc2) => {
                self.spawn_postinit_mc2_subspell_9_11_15(s, r)
            }
            // The dis-fired METEOR (0x11) / FISSURE (0x47) tier
            // overrides: the sub_4A310 SPELLS block (EF:33148-78)
            // consumes par1 for 0x11/0x16/0x43/0x47 too — 0x11 writes
            // maxLife AND life (EF:33154/33178/33167), 0x47 life only
            // (EF:33165-67). NEITHER is in the LOAD list (EV:387 =
            // 9/0xB/0xF), and every shipped record is dis-gated.
            (10, 17) if matches!(self.game, GameId::Mc2) && r.dis_id != 0xFFFF => {
                self.spawn_postinit_mc2_meteor_tier(s, r)
            }
            (10, 71) if matches!(self.game, GameId::Mc2) && r.dis_id != 0xFFFF => {
                self.spawn_postinit_mc2_fissure_tier(s, r)
            }
            // The disposition-spawned WHIRLWIND (arm tornado) scaled
            // to its tier, like the cast path (`sub_678E0` →
            // life = 8 × row-21 tier.life). remc2's generate switch
            // (EV:387 / Events.cpp:362) omits model 22, leaving
            // `AddWind`'s 500-tick roamer that drifts ~60 tiles off the
            // arm — but recorded retail confines the arm tornadoes to a
            // couple seconds (Tornado I, par1=0 → 8×5 = 40 ticks; it
            // moves off-centre but dies before it gets anywhere).
            // Deliberate: both spawn paths unified under the 8×charge
            // law over the trace.
            (10, 22) if matches!(self.game, GameId::Mc2) => {
                self.spawn_postinit_mc2_whirlwind_tier(s, r)
            }
            // The disposition-spawned mana-magnet aura reads its RANGE
            // and LIFE from the THING's stageTag (`sub_4A310`
            // EF:33095-33104, the v4<=0x36 arm): range = (stageTag<<8)²
            // = stageTag tiles, life = 8*stageTag+16 (floor 128). The
            // `AddAuxiliary` ctor's 14-tile/128 defaults are only the
            // pre-override seed — a level never disposition-fires an
            // aura without this. level-001's 4 staged magnets carry
            // stageTag 33/45/64/31 → 33/45/64/31-tile reach (they pull
            // the arm balls 20-44 tiles out; the 14-tile default left
            // them stranded). Our `swi_id` is the `stageTag_12` field.
            (10, 54) if matches!(self.game, GameId::Mc2) => self.spawn_postinit_mc2_aura(s, r),
            // The cave sculptors' THING wiring (sub_4A310
            // EF:33118-46, high-band trace §4): dome radius =
            // word_10; pit/hill radius = word_10, depth/height seed
            // = par3 (via the z sentinel), recentred onto the tile
            // corner (−128,−128). (10,80) consumes nothing.
            //
            // The (10,82) room carve is LOAD-TIME-ONLY par
            // consumption, and on the OTHER path: PrepareEvents'
            // generate case 0x52 (EV:373-379) writes par1/par2 →
            // the box half-extents and par3 → the depth multiplier,
            // while sub_4A310 gives a dis-fired 0x52 only the
            // stage-bind (the ctor's 3/3/2 defaults stand). Without
            // this a cave's authored entry caverns carve as 6×6
            // closets.
            (10, 82) if matches!(self.game, GameId::Mc2) && r.dis_id == 0xFFFF => {
                self.spawn_postinit_mc2_cave_room_carve(s, r)
            }
            (10, 83) if matches!(self.game, GameId::Mc2) => {
                self.g.ent[s].dest_x = r.swi_sz;
            }
            (10, 84 | 85) if matches!(self.game, GameId::Mc2) => {
                self.spawn_postinit_mc2_cave_pit_hill(s, r)
            }
            // The (10,67) flood's par1 seam is TRIGGER-ONLY: the
            // sub_4A310 case-0xA path (EF:33148/:33165 → SPELLS row
            // 20 life + subSpell) fires for dis-gated rows, while
            // the load-time generate pass (EV:387's case list is
            // 9/0xB/0xF only) leaves the ctor defaults.
            // Gate = "reached via the DIS path" (anything but the
            // 0xFFFF load sentinel — dis 0 fires at init through
            // sub_4A1E0(0) and takes the same sub_4A310 arm;
            // shipped-data neutral: no load-time flood authors par1).
            (10, 67) if matches!(self.game, GameId::Mc2) && r.dis_id != 0xFFFF => {
                self.spawn_postinit_mc2_flood_tier(s, r)
            }
            // The MC2 class-15 token state bump (the shared class-
            // 12/15 spawn case, remc2 EF:33209-17): actionIndex +=
            // stageTag (0 = inert cast-slot, 1 = pickup, 2 = self-
            // replenishing pickup); >= 3 = the junk state 253.
            (15, _) if matches!(self.game, GameId::Mc2) => {
                self.spawn_postinit_mc2_spell_token(s, r)
            }
            // The (14,1) riser's THING wiring (sub_4A310 case 0xE
            // LABEL_49, remc2 EF:33228-31): par1 → orientation
            // (0 = +X strip, 1 = +Y), par2 → length. The ctor set
            // neither (mc2::riser field map).
            (14, 1) if matches!(self.game, GameId::Mc2) => {
                let e = &mut self.g.ent[s];
                e.f71 = (r.parent & 0xFF) as u8;
                e.f26 = r.child as i16;
            }
            // The (14,2) cave pillar's THING wiring (sub_4A310 case
            // 0xE model-2 arm, remc2 EF:33236-41): par1 → orientation
            // (word_0x2C_44), par3 → half-width koef (word_0x96_150).
            // par2/word_10/stageTag are NOT consumed.
            (14, 2) if matches!(self.game, GameId::Mc2) => {
                let e = &mut self.g.ent[s];
                e.f44 = r.parent;
                e.f146 = r.par3;
            }
            // The MC2 stage-gated switch stores par1 = the stage row
            // instead of extents (remc2 :33200-01; its tick never
            // probes proximity).
            (11, 32) if matches!(self.game, GameId::Mc2) => {
                self.spawn_postinit_mc2_stage_switch(s, r)
            }
            (11, _) => self.spawn_postinit_trigger_volume(s, r),
            _ => {}
        }

        if drawable(self.game, r.class, r.model) {
            self.entities_dirty = true;
        }
    }

    // ---- spawn_from_thing dispatch/post-init arm bodies (S1b) ----

    /// The MC2 class-14 map objects (0..=5); the ending fly-to
    /// markers (3/4) spawn hidden when disposition-gated.
    fn spawn_mc2_class14_gated(&mut self, r: Rec, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.mc2_spawn_class14(r.model as u8, x, y, z);
        if let Some(s) = s
            && matches!(r.model, 3 | 4)
            && r.dis_id != 0
        {
            self.g.ent[s].flags |= 0x20;
        }
        s
    }

    /// Class-12 spell-jar post-init (shared 12/15 spawn case): the
    /// swi_id state bump + the BLUE unrestricted-grant jar variant.
    fn spawn_postinit_class12_jar(&mut self, s: usize, r: Rec) {
        // byte70 += swi_id; >= 3 = the BLUE jar variant
        // (-3 recovers the same 0..=2 sub-state; sprite 280
        // straight to +86; the unrestricted-grant marker —
        // see [`BLUE_SPELL`]).
        let e = &mut self.g.ent[s];
        e.tick70 = e.tick70.wrapping_add((r.swi_id & 0xFF) as u8);
        if r.swi_id >= 3 {
            e.tick70 = e.tick70.wrapping_sub(3);
            e.type86 = 280;
            e.flags |= BLUE_SPELL; // +18 |= 4
        }
    }

    /// The MC2 (10,9)/(10,11)/(10,15) par1 SPELLS.DAT subspell
    /// overrides: model 9 writes maxLife (dome radius), 11/15 life.
    fn spawn_postinit_mc2_subspell_9_11_15(&mut self, s: usize, r: Rec) {
        let row = crate::mc2::spells::spell_index(r.model as u8);
        if let Some(row) = self.g.assets.spells.get(row) {
            let tier = row.tiers[(r.parent as usize).min(2)];
            let e = &mut self.g.ent[s];
            e.f140 = tier.sub_spell;
            if r.model == 9 {
                e.max_life = tier.life as u32;
            } else {
                e.act_life = tier.life as i32;
            }
        }
    }

    /// The dis-fired MC2 (10,17) meteor tier override (par1 → row-17
    /// tier: subspell + maxLife + life).
    fn spawn_postinit_mc2_meteor_tier(&mut self, s: usize, r: Rec) {
        let row = crate::mc2::spells::spell_index(17);
        if let Some(row) = self.g.assets.spells.get(row) {
            let tier = row.tiers[(r.parent as usize).min(2)];
            let e = &mut self.g.ent[s];
            e.f140 = tier.sub_spell;
            e.max_life = tier.life as u32;
            e.act_life = tier.life as i32;
        }
    }

    /// The dis-fired MC2 (10,71) fissure tier override (par1 → row-71
    /// tier: subspell + life).
    fn spawn_postinit_mc2_fissure_tier(&mut self, s: usize, r: Rec) {
        let row = crate::mc2::spells::spell_index(71);
        if let Some(row) = self.g.assets.spells.get(row) {
            let tier = row.tiers[(r.parent as usize).min(2)];
            let e = &mut self.g.ent[s];
            e.f140 = tier.sub_spell;
            e.act_life = tier.life as i32;
        }
    }

    /// The dis-spawned MC2 (10,22) whirlwind scaled to its tier
    /// (life = 8 × row-21 tier.life), unified with the cast path.
    fn spawn_postinit_mc2_whirlwind_tier(&mut self, s: usize, r: Rec) {
        let row = crate::mc2::spells::spell_index(22);
        if let Some(row) = self.g.assets.spells.get(row) {
            let tier = row.tiers[(r.parent as usize).min(2)];
            let ml = 8 * tier.life.max(0) as u32;
            let e = &mut self.g.ent[s];
            // Retail's 0x16 arm also stamps the tier's
            // subspell (EF:33176-78).
            e.f140 = tier.sub_spell;
            e.max_life = ml;
            e.act_life = ml as i32;
        }
    }

    /// The dis-spawned MC2 (10,54) mana-magnet aura: range/life from
    /// the THING stageTag (range = swi_id tiles, life = 8*tag+16).
    fn spawn_postinit_mc2_aura(&mut self, s: usize, r: Rec) {
        let tag = r.swi_id as u32;
        let e = &mut self.g.ent[s];
        e.f26 = r.swi_id as i16;
        e.max_life = (8 * tag + 16).max(128);
        e.act_life = e.max_life as i32;
    }

    /// The MC2 (10,82) cave room-carve THING wiring (load-time-only):
    /// box half-extents (par1/par2) + depth multiplier (par3).
    fn spawn_postinit_mc2_cave_room_carve(&mut self, s: usize, r: Rec) {
        let e = &mut self.g.ent[s];
        e.f67 = (r.parent & 0xFF) as u8;
        e.f68 = (r.child & 0xFF) as u8;
        e.f71 = (r.par3 & 0xFF) as u8;
    }

    /// The MC2 (10,84)/(10,85) cave pit/hill THING wiring: recentred
    /// onto the tile corner, radius (swi_sz) + depth/height seed (par3).
    fn spawn_postinit_mc2_cave_pit_hill(&mut self, s: usize, r: Rec) {
        let e = &mut self.g.ent[s];
        e.x = e.x.wrapping_sub(128);
        e.y = e.y.wrapping_sub(128);
        e.dest_x = r.swi_sz;
        e.z = r.par3 as i16;
    }

    /// The dis-fired MC2 (10,67) flood tier override (par1 → row-67
    /// tier: subspell + life).
    fn spawn_postinit_mc2_flood_tier(&mut self, s: usize, r: Rec) {
        let row = crate::mc2::spells::spell_index(67);
        if let Some(row) = self.g.assets.spells.get(row) {
            let tier = row.tiers[(r.parent as usize).min(2)];
            let e = &mut self.g.ent[s];
            e.f140 = tier.sub_spell;
            e.act_life = tier.life as i32;
        }
    }

    /// The MC2 class-15 spell-token state bump (actionIndex += stageTag;
    /// >= 3 = the junk state 253).
    fn spawn_postinit_mc2_spell_token(&mut self, s: usize, r: Rec) {
        let e = &mut self.g.ent[s];
        e.tick70 = e.tick70.wrapping_add((r.swi_id & 0xFF) as u8);
        if r.swi_id >= 3 {
            e.tick70 = 253;
        }
    }

    /// The MC2 stage-gated switch (11,32): stores par1 = the stage row
    /// instead of extents (its tick never probes proximity).
    fn spawn_postinit_mc2_stage_switch(&mut self, s: usize, r: Rec) {
        self.g.ent[s].id24 = r.swi_id;
        self.g.ent[s].f71 = (r.parent & 0xFF) as u8;
        self.g.refill_life(s);
        self.g.ent[s].flags |= 1;
    }

    /// Trigger-volume post-init (class 11): id24/extents + the volume
    /// flag (the original's :44017-50 class-11 branch).
    fn spawn_postinit_trigger_volume(&mut self, s: usize, r: Rec) {
        self.g.ent[s].id24 = r.swi_id;
        self.g.extents(s, r.swi_sz << 8, 4096);
        self.g.refill_life(s);
        self.g.ent[s].flags |= 1;
    }

    /// `sub_514E0` (remc2 EF:37315) + the per-model sub-creators —
    /// the MC2 class-14 special map objects: 0 = decorative marker
    /// (sprite 77), 1 = the terrain wall-riser (ctor sub_51660
    /// EF:37378: action 6, maxLife/life 0 ⇒ instant-build on the
    /// first tick, untargetable, NO sprite — invisible machinery;
    /// tick = [`crate::mc2::riser`]), 2 = the cave pillar (ctor
    /// sub_516C0 EF:37397: CAVE-ONLY, action 7, life 0 ⇒ the measure
    /// phase runs in the load settle, orientation/half-width zeroed
    /// for the THING wiring, retail leaves maxLife at the NewEvent
    /// default; tick = [`crate::mc2::cave`]), 3 = the map "X"
    /// checkpoint marker (sprite
    /// 338; hidden by switch model 12), 4 = the level-end marker
    /// (sprite 339; switch model 31), 5 = the pickup scroll (sprite
    /// 280, box 768x1280, 4 XP).
    fn mc2_spawn_class14(&mut self, model: u8, x: u16, y: u16, z: i16) -> Option<usize> {
        if model == 2 && !self.g.is_cave() {
            return None; // cave-only (sub_516C0 :37400)
        }
        let s = self.g.new_event()?;
        {
            let e = &mut self.g.ent[s];
            e.class64 = 14;
            e.model65 = model;
            e.f71 = 0;
            e.tick70 = match model {
                0 => 0,
                1 => 6,
                2 => 7,
                3 => 8,
                4 => 9,
                _ => 10,
            };
            if model == 1 {
                // byte[0] &= 0xF6 then |= 1; maxLife = 0; life = 0;
                // subSpellIndex = 0 (EF:37383-37390).
                e.flags = (e.flags & !0x8) | 1;
                e.max_life = 0;
                e.f44 = 0;
            }
            if model == 2 {
                // byte[0] &= 0xF6 |= 1; life = 0 (maxLife untouched);
                // word_0x2C_44 = word_0x96_150 = 0 (EF:37406-37414).
                e.flags = (e.flags & !0x8) | 1;
                e.f44 = 0;
                e.f146 = 0;
            }
        }
        self.g.link(s, x, y, z);
        self.g.refill_life(s);
        if model == 2 {
            self.g.ent[s].act_life = 0; // life_0x8 = 0 (EF:37410)
        }
        if model != 1 {
            let sprite = match model {
                0 => 77,
                3 => 338,
                4 => 339,
                _ => 280,
            };
            self.g.mc2_set_sprite(s, sprite);
        }
        match model {
            0 => self.g.extents(s, 384, 384),
            5 => self.g.extents(s, 768, 1280),
            _ => {}
        }
        Some(s)
    }

    /// The class-14 tick column (strE0): 8/9 = terrain-pinned
    /// markers, 10 = the pickup scroll (UpdateScroll_59C80 :41158 —
    /// grants +4 XP single-player to every owned spell on human
    /// overlap, tallied in [`Gen::mc2_scrolls`]; sound 63), 6 = the
    /// terrain
    /// riser (sub_59F60, [`crate::mc2::riser`]), 7 = the cave pillar
    /// (sub_5B100, [`crate::mc2::cave`]), 0..=5 = the authentic
    /// no-ops.
    fn mc2_class14_tick(&mut self, i: usize) {
        match self.g.ent[i].tick70 {
            6 => {
                if self.g.mc2_riser_tick(i) {
                    self.terrain_dirty = true;
                }
            }
            7 => {
                if self.g.mc2_pillar_tick(i) {
                    self.terrain_dirty = true;
                }
            }
            8 | 9 => {
                let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
                self.g.ent[i].z = self.g.ground_z(x, y) as i16;
            }
            10 => {
                let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
                self.g.ent[i].z = self.g.ground_z(x, y) as i16;
                // The player-overlap collect (sub_106C0 AABB; the
                // human's own extents are zero, the scroll's box
                // carries the pickup).
                let (px, py, _) = self.human_pose;
                let e = &self.g.ent[i];
                let wrap_d = |a: u16, b: u16| ((a.wrapping_sub(b)) as i16 as i32).abs();
                if wrap_d(px, e.x) < e.f80 as i32 && wrap_d(py, e.y) < e.f82 as i32 {
                    self.g.snd(63, i);
                    self.g.mc2_scrolls.0 += 1;
                    self.g.ent[i].flags |= 0x400;
                    self.entities_dirty = true;
                    // `UpdateExperience_6E090` (EF:44262): the scroll
                    // grants the FULL `countXP` to EVERY owned spell —
                    // no split / round-robin (docs/spell-audit/
                    // xp-scrolls.md corrects the equal-split
                    // hypothesis). 4 single-player (50 MP, out of
                    // scope). The castle-XP clamp at 7 and the
                    // level-up notification live in `mc2_award_xp`/
                    // `mc2_relevel`.
                    for s in 0..26usize {
                        if self.mc2_book.ent[s] != 0 {
                            self.mc2_award_xp(PLAYER_TARGET, s, 4);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The class-15 token tick (`sub_68FF0` EF:55676 behind the
    /// 3M+1/3M+2 pickup wrappers; docs/traces/
    /// mc2-class15-spell-tokens.md §3/§4): life countdown (scattered
    /// tokens — authored jars carry 0), fall −128/tick to the
    /// terrain (sub_580E0), and every 4th phase an AABB overlap scan
    /// against the wizards — single-player: the human. Collection
    /// (sound 18, played at the collector) KEEPS the entity — it
    /// becomes the wizard's live spell manifestation, state 3M
    /// ([`World::mc2_adopt_manifestation`], the Phase-4.2 slot
    /// economy); state 3M+2 drops a fresh state-3M+2 token in place
    /// (`sub_69250`). State 3M (the spell EFFECT — the cast
    /// machinery ticks it wizard-side) and the junk state 253 are
    /// inert here. The "local player already owns the spell"
    /// byte[0] re-mark (EF:55706) is presentation-side (token tint)
    /// and unmodeled; [`Gen::mc2_spell_tokens`] stays in sync as
    /// the grant mask.
    fn mc2_spell_token_tick(&mut self, i: usize, player: PlayerPose) {
        let t = self.g.ent[i].tick70;
        let model = self.g.ent[i].model65;
        // Action 78 — the shared class-15 slot past the 3M+2 states
        // (26·3 = 78): the STOLEN jar's detach/homing arc.
        if t == 78 {
            self.mc2_stolen_jar_tick(i, player);
            return;
        }
        if t == 253 || t == model.wrapping_mul(3) {
            return;
        }
        let life = self.g.ent[i].act_life;
        if life > 0 {
            self.g.ent[i].act_life = life - 1;
            if life == 1 {
                self.g.ent[i].flags |= 0x400;
                self.entities_dirty = true;
                return;
            }
        }
        // sub_580E0(pos, alt, 0, _, -128): fall 128/tick, clamp at
        // the terrain altitude.
        let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
        let alt = self.g.ground_z(ex, ey) as i16;
        let z = self.g.ent[i].z;
        if z > alt {
            self.g.ent[i].z = (z - 128).max(alt);
            self.entities_dirty = true;
        } else if z < alt {
            self.g.ent[i].z = alt;
            self.entities_dirty = true;
        }
        // The scan stagger (byte_0x3E_62 & 3, EF:55698).
        if self.g.ent[i].f63 & 3 != 0 {
            return;
        }
        // The wizard scan — the human, alive, not yet holding this
        // spell (the SpellEnabled[model] gate, EF:55713).
        let owned = self.g.mc2_spell_tokens.0 & (1 << model) != 0;
        if self.prune_owned_jars
            && self
                .mc2_book
                .ent
                .get(model as usize)
                .is_some_and(|&e| e != 0)
        {
            // Unfaithful improvement (deliberate, P-class): an
            // owned-spell jar can never be collected — remove it
            // instead of leaving permanent clutter. Self-culling here
            // covers both the level-load sweep and the tick after the
            // player gains the spell (every jar of it despawns on its
            // next tick).
            // The predicate keys on the XP BOOK, not the SpellEnabled
            // mask above: the central grant (`mc2_adopt_manifestation`)
            // sets only the book, so campaign-carried spells leave the
            // mask at the fireball+possess seed and the mask reads
            // "unowned" for everything actually carried.
            self.g.ent[i].flags |= 0x400;
            self.entities_dirty = true;
            return;
        }
        if self.player.state != LifeState::Alive || owned {
            return;
        }
        let (px, py, pz) = self.human_pose;
        let (tx, ty, tz, bx, bz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.f80, e.f84)
        };
        let wd = |a: u16, b: u16| ((a.wrapping_sub(b)) as i16 as i32).abs();
        if wd(px, tx) < bx as i32
            && wd(py, ty) < bx as i32
            && ((pz as i32) - (tz as i32)).abs() < bz as i32
        {
            // COLLECT (EF:55715-55749): the token is KEPT and
            // becomes the wizard's live spell manifestation
            // (state 3M) — the Phase-4.2 slot economy; the old
            // bank-and-despawn interim is closed.
            self.g.snd_player(18);
            self.g.mc2_spell_tokens.0 |= 1 << model;
            self.entities_dirty = true;
            if t == model.wrapping_mul(3).wrapping_add(2) {
                // The self-replenishing state drops a replacement.
                if let Some(n) = self.g.mc2_spawn_spell_token(model, tx, ty, tz) {
                    self.g.ent[n].tick70 = model.wrapping_mul(3).wrapping_add(2);
                }
            }
            self.mc2_adopt_manifestation(i, model as usize);
        }
    }

    /// `sub_692C0` (EF:55774-89) — the action-78 wrapper: run the
    /// detach/homing arc; when it reports done, flip to the ordinary
    /// ground-jar pickup state `3M+1` and drop the wraith ref.
    /// Retail also stamps `byte[3] |= 2` (the "dropped by a steal"
    /// marker) — write-only in the whole engine (its only consumer
    /// is the pickup's clear), unmodeled.
    fn mc2_stolen_jar_tick(&mut self, i: usize, player: PlayerPose) {
        if self.mc2_stolen_arc(i, player) {
            let model = self.g.ent[i].model65;
            self.g.ent[i].tick70 = model.wrapping_mul(3).wrapping_add(1);
            self.g.ent[i].f38 = 0;
        }
        self.entities_dirty = true;
    }

    /// `sub_59DC0` (EF:41199-252) — the stolen jar's flight, true =
    /// done. Counter (`dword_0x10_16` → f26) 0..=5: DETACH — the jar
    /// rides 384 units ahead of the PLAYER's aim, pitch sweeping off
    /// it by 16/tick (`playerPitch − 16·n`). Counter ≥ 6: HOMING —
    /// step from the jar's own position toward a point 384 ahead of
    /// the WRAITH (f38) along its heading at pitch 0, at speed
    /// `32·(n−5)`; once below `terrainAlt + 64`, snap to the terrain
    /// and finish. Owner dead/gone at entry, or wraith dead
    /// mid-flight → finish where it is.
    fn mc2_stolen_arc(&mut self, i: usize, player: PlayerPose) -> bool {
        if self.player.state != LifeState::Alive {
            return true;
        }
        let n = self.g.ent[i].f26;
        if n <= 5 {
            let mut pos = (player.x, player.y, player.z);
            Gen::polar_step(
                &mut pos,
                player.heading,
                player.pitch.wrapping_sub(16 * n as u16),
                384,
            );
            self.g.move_relink(i, pos.0, pos.1, pos.2);
            self.g.ent[i].f26 = n + 1;
            return false;
        }
        self.g.ent[i].f26 = n + 1;
        let w = self.g.ent[i].f38 as usize;
        if w == 0
            || w >= self.g.ent.len()
            || self.g.ent[w].act_life < 0
            || self.g.ent[w].flags & 0x400 != 0
        {
            return true; // wraith dead → drop in place
        }
        let mut tgt = (self.g.ent[w].x, self.g.ent[w].y, self.g.ent[w].z);
        Gen::polar_step(&mut tgt, self.g.ent[w].f30, 0, 384);
        let jar = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        let yaw = Gen::angle_between(jar.0, jar.1, tgt.0, tgt.1);
        let pitch = Gen::mc2_radix_tan(jar, tgt);
        let mut pos = jar;
        Gen::polar_step(&mut pos, yaw, pitch, 32 * (n - 5));
        let alt = self.g.ground_z(pos.0, pos.1) as i16;
        if pos.2 >= alt + 64 {
            self.g.move_relink(i, pos.0, pos.1, pos.2);
            return false;
        }
        self.g.move_relink(i, pos.0, pos.1, alt);
        true
    }

    /// `RemoveCastleStage_385C0` (remc2 EF:28065) — the MC2 building
    /// teardown (state 53). No chain: evacuate one occupant per
    /// footprint cell (archers/villager/trader/settler docks, the
    /// killer poked into their damage mail), restore the footprint
    /// terrain (angle nibble → 1, type-1 rubble stamps, pad-height
    /// removal with the verbatim two-draw RNG), fire the building's
    /// on-death disposition (xtype = the THING's par2), despawn.
    /// With a bldgprm chain byte: the building REBUILDS as its chain
    /// successor (the downgrade ladder), inheriting owner + xtype +
    /// the saved completion z.
    ///
    /// Approximation register: the rubble texture pass
    /// (AddBuildingToTerrain_46570's 343*(angle&7) block table) and
    /// the SetHeightmapByBuildingArea_48B50 smoothing run through
    /// mc2_retile_region over the footprint window (bodies unread —
    /// flagged in docs/traces); the id-68 player-castle global lands
    /// with MC2 castles. The `sub_59760` type-2 objective re-point on
    /// the chain rebuild is PORTED below.
    fn mc2_house_collapse(&mut self, i: usize) {
        let bldg = self.g.ent[i].f71 as usize;
        let chain = self.g.assets.bldgprm.get(bldg).map_or(0, |b| b.chain);
        let Some(def) = self.g.assets.build_tab.get(bldg).copied() else {
            self.g.ent[i].flags |= 0x400;
            return;
        };
        let (w, h) = (def.w as usize, def.h as usize);
        let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
        let tlx = (((ex.wrapping_add(128)) >> 8) as u8).wrapping_sub((w / 2) as u8);
        let tly = (((ey.wrapping_add(128)) >> 8) as u8).wrapping_sub((h / 2) as u8);
        if chain != 0 {
            // The chain rebuild (:28185-28216).
            let (z, owner, xt) = {
                let e = &self.g.ent[i];
                (e.site_z, e.f144, e.f66)
            };
            if let Some(n) = self.g.mc2_spawn_building(ex, ey, z, chain as u16) {
                self.g.ent[n].z = z; // the saved completion z (:28190)
                self.g.ent[n].f66 = xt;
                if owner != 0 {
                    self.g.ent[n].f144 = owner;
                    self.g.ent[n].flags |= 1; // :28196 verbatim
                    self.g.mc2_set_sprite(n, 177);
                }
                // `sub_59760` (:28204, body EF:40921-54): every ACTIVE
                // type-2 objective row bound to the collapsing building
                // follows the successor slot — the degradation-chain
                // succession that keeps a kill-named-building objective
                // alive across intermediate collapses. Type 2 ONLY
                // (retail checks `byte0 == 2`); type 1 never re-points.
                if !self.completed {
                    for st in &mut self.mc2_stages {
                        if st.kind == 2 && st.state == 1 && st.bound == Some(i as u16) {
                            st.bound = Some(n as u16);
                        }
                    }
                }
            }
            // The footprint dirty-bit clear (:28225-33).
            for dy in 0..h {
                for dx in 0..w {
                    let t = crate::engine::features::tile(
                        tlx.wrapping_add(dx as u8),
                        tly.wrapping_add(dy as u8),
                    );
                    self.g.t.angle[t] &= 0x7F;
                }
            }
            self.g.ent[i].flags |= 0x400;
            return;
        }
        let start = def.offset as usize;
        let cells = self
            .g
            .assets
            .build_dat
            .get(start..start + 2 * w * h)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        if cells.is_empty() {
            self.g.ent[i].flags |= 0x400;
            return;
        }
        let zk = self.g.avg4(tlx, tly, h as u8, w as u8) as i16; // model != 0 arm
        let killer = self.g.ent[i].f40;
        let enterable = self
            .g
            .assets
            .bldgprm
            .get(bldg)
            .is_some_and(|b| b.flags & 1 != 0);
        let mut nth = 0u32;
        for dy in 0..h {
            for dx in 0..w {
                let c = &cells[2 * (dy * w + dx)..2 * (dy * w + dx) + 2];
                if c[0] == 0xff && c[1] == 0xff {
                    continue;
                }
                let (cx, cy) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                nth += 1;
                let sz = if nth & 7 == 0 {
                    32 * (zk - 10)
                } else {
                    32 * zk
                };
                let (sx, sy) = ((cx as u16) << 8, (cy as u16) << 8);
                // One occupant out per cell (:28112-28141).
                if self.g.ent[i].f26 > 0 {
                    self.g.ent[i].f26 -= 1;
                    if enterable {
                        let left = self.g.ent[i].f26;
                        let s = if left != 0 {
                            if left >= 4 {
                                self.g.mc2_rand_occupant(i, sx, sy, sz)
                            } else {
                                let a = self.g.mc2_spawn_archers(sx, sy, sz);
                                if let Some(a) = a {
                                    self.g.ent[a].tick70 = 33;
                                }
                                a
                            }
                        } else {
                            let s = self.g.mc2_spawn_m12(sx, sy, sz);
                            if let Some(s) = s {
                                self.g.ent[s].tick70 = 97;
                            }
                            s
                        };
                        if let Some(s) = s {
                            self.g.ent[s].mail[0] = (1, killer);
                        }
                    }
                }
                // Terrain restore (:28143-28169): angle nibble → 1,
                // the AddBuildingToTerrain 2x2 type-1 rubble stamp,
                // pad-height removal with the verbatim RNG.
                let t = crate::engine::features::tile(cx, cy);
                self.g.t.angle[t] = (self.g.t.angle[t] & 0x70) | 1;
                for (ddx, ddy) in [(0i32, 0i32), (-1, 0), (-1, -1), (0, -1)] {
                    let rt = crate::engine::features::tile(
                        cx.wrapping_add(ddx as u8),
                        cy.wrapping_add(ddy as u8),
                    );
                    self.g.t.tile_type[rt] = 1;
                }
                if c[1] != 0xff {
                    let cur = self.g.t.height[t];
                    if c[1] >= cur {
                        self.g.t.height[t] = 0;
                    } else {
                        let d = self.g.ent_rand(i);
                        if d % 0x32 <= 20 {
                            self.g.t.height[t] = cur.wrapping_sub(c[1]);
                        } else {
                            let d2 = self.g.ent_rand(i);
                            self.g.t.height[t] =
                                cur.wrapping_sub(c[1].wrapping_sub((d2 % 0x14) as u8));
                        }
                    }
                }
            }
        }
        // The texture rebuild + smoothing window (APPROX — module
        // doc): one retile over the footprint + 1 ring.
        self.g.mc2_retile_region(
            tlx.wrapping_sub(1),
            tly.wrapping_sub(1),
            tlx.wrapping_add(w as u8),
            tly.wrapping_add(h as u8),
        );
        // The on-death disposition (:28174-75).
        let dis = self.g.ent[i].f66 as u16;
        if dis != 0 {
            self.fire_disposition(dis, true);
        }
        self.g.ent[i].flags |= 0x400;
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
            // sub_3BF70 (:47979-): sprite 77 for every jar model +
            // the 4x extent override (the generous pickup vacuum).
            self.g.set_sprite(s, 77);
            let (h4, v4) = {
                let e = &self.g.ent[s];
                (e.f80 * 4, e.f84 * 4)
            };
            self.g.extents(s, h4, v4);
        }
        Some(s)
    }

    pub(crate) fn free_slot(&mut self, i: usize) {
        if drawable(
            self.game,
            self.g.ent[i].class64 as u16,
            self.g.ent[i].model65 as u16,
        ) || self.g.ent[i].class64 == 11
        {
            self.entities_dirty = true; // a drawable/overlay entity left
        }
        self.g.free_entity(i);
    }

    // ---- class-11 trigger ticking (str_256038, :4921) ---------------------

    fn trigger_tick(&mut self, i: usize, player: PlayerPose, buckets: &[u32]) {
        match self.g.ent[i].tick70 {
            // One-shot proximity: fire when a wizard balloon is inside
            // (polarity 1) / outside (polarity 0) the volume.
            0 | 5 | 9 => self.one_shot(i, player, true),
            1 | 6 | 10 => self.one_shot(i, player, false),
            // Repeating proximity with a 10-tick rearm that waits for
            // the player to leave (:67249).
            2 | 7 | 11 => self.repeating(i, player, true),
            3 | 8 | 12 => self.repeating(i, player, false),
            // State 4: the WIN trigger (sub_59B80 :67293-67315) —
            // waits for the human's castle-holding completion latch
            // (13325 bit 2 from the win check), fires its
            // disposition, despawns, CONSUMES THE WIN (13325 &=
            // 0xFD) and plays sound 41. Campaign levels script the
            // goal this way: reaching the share spawns the next
            // stage instead of ending the level (level 010 unleashes
            // a mana-stealing genie), and only a re-held share with
            // no armed win trigger left ends it.
            4 => {
                if self.completed && self.player_castle().is_some() {
                    let dis = self.g.ent[i].id24;
                    self.fire_disposition(dis, false);
                    self.g.ent[i].flags |= 0x400;
                    self.completed = false;
                    self.win_streak = 0;
                    self.g.snd(41, i);
                }
            }
            // States 13..=29: class-5 bucket 0..=16 empty for 16
            // ticks; state 30: buckets 0..=11 and 16 all empty.
            s @ 13..=29 => self.kill_trigger(i, Some((s - 13) as usize), buckets),
            30 => self.kill_trigger(i, None, buckets),
            _ => {}
        }
    }

    // ---- MC2 class-11 switches (remc2 strB0 table, EventsFunctions
    // :44499-44541 + :54306-54428) --------------------------------------

    /// The per-model MC2 switch dispatch. Models 0/1 =
    /// `AddSwitch0B_00_6F030`/`CheckpointArrived_6F070` (:44499/:44511)
    /// — enter/leave one-shots that CONSUME their record set; 2/3 =
    /// `sub_6F0B0`/`sub_6F100` (:54408/:54306) — enter/leave with a
    /// 10-count rearm and non-consuming fire. Only 0..=3 pass the
    /// spawn seam; others misfit there.
    fn mc2_switch_tick(&mut self, i: usize) {
        match self.g.ent[i].tick70 {
            0 => self.mc2_switch_one_shot(i, true),
            1 => self.mc2_switch_one_shot(i, false),
            2 => self.mc2_switch_repeating(i, true),
            3 => self.mc2_switch_repeating(i, false),
            // Model 4, `AddSwitch0B_04_6F150` (:54329): the level-end
            // release — fires (consuming) when a player's
            // IsLevelEnd_0 latches = our `completed`. Level-000
            // gates its victory cluster (dis 4) on it.
            4 => {
                if self.completed {
                    let dis = self.g.ent[i].id24;
                    self.fire_disposition(dis, true);
                    self.g.ent[i].flags |= 0x400;
                    self.entities_dirty = true;
                }
            }
            // Model 32, `AddSwitch0B_20_6F1C0` (:54353): the
            // stage-gated release — fires (consuming) when the
            // checkpoint ROW its par1 names reaches state 2
            // (`stage_0x3659F[par1] == 2`). Level-000 chains its
            // whole progression through these: checkpoint 1 → dis 2,
            // checkpoint 2 → dis 3 (the kill-target archers), the
            // kill objective → dis 5, the mana goal → dis 6. Every
            // authored (`index != -1`) row registers, so par1 always
            // resolves; a row that can never complete (the
            // faithfully-stuck type-1/2 stage-0 binds) holds its
            // switch forever, like retail.
            32 => {
                let par1 = self.g.ent[i].f71;
                if self
                    .mc2_stages
                    .iter()
                    .any(|s| s.row == par1 && s.state == 2)
                {
                    // ObjectiveDone_2 = 1 (:54371): pause the
                    // objective pass one tick, so the disposition's
                    // spawns exist before the NEXT row is tested —
                    // the bridge that keeps a freshly-current type-7
                    // row from latching vacuously.
                    self.mc2_objective_pause = 1;
                    let dis = self.g.ent[i].id24;
                    self.fire_disposition(dis, true);
                    self.g.ent[i].flags |= 0x400;
                    self.entities_dirty = true;
                }
            }
            // Model 12, `sub_6F2B0` (:54431) and model 31, `sub_6F7E0`
            // (:54690): the two ENDING X-marker trips. Both seize the
            // flyer into the SAME endGameSeq (retail sets the touched
            // player's actionIndex — 12 targets the (14,3) checkpoint
            // "X" via word_0x36DFE, 31→11 targets the (14,4) demon
            // mouth via word_0x36DFC; level-000 ends through the 12
            // arm). The marker entity is REVEALED and persists as the
            // fly-to target — retail clears only its map-icon draw
            // bit (:54701). The level ends at endGameSeq phase 0xC,
            // after the fly-in + fade.
            m @ (12 | 31) if self.g.ent[i].f63 & 7 == 0 && self.mc2_switch_overlap(i) => {
                let target = if m == 12 { 3 } else { 4 };
                self.mc2_end_pending = Some(target);
                for j in 1..self.g.ent.len() {
                    let e = &self.g.ent[j];
                    if e.class64 == 14 && e.model65 == target && e.flags & 0x400 == 0 {
                        self.g.ent[j].flags &= !0x20;
                    }
                }
                self.g.ent[i].flags |= 0x400;
                self.entities_dirty = true;
            }
            // The slot-condition band, `sub_6F300` (:54457): the
            // switch watches the per-class-5-model live list; when
            // its slot EMPTIES, a 16-tick countdown arms and the
            // switch chain-fires (sound 41, sub_4A1E0(id, 1)) and
            // despawns. Model→slot: 13..=29 → 0..=16, 33..=44 →
            // 0x11..=0x1C (docs/traces/mc2-class11-switches-
            // class14.md §3); model 30 = the ANY-slot variant.
            // ANY scans slots 0..=0xB and 0x10 ONLY: the retail scan
            // loop's bound is `<= 16` (NETHERW.EXE @0x93BA6 `cmp
            // eax,0x10; jng` — the body's `<= 0x1C` arm is dead past
            // the bound), so high models 0x11..=0x1C never gate it.
            // Same law as MC1's -1 variant. Load-bearing on level 024:
            // the wandering (5,27) hydra must NOT block the gauntlet's
            // (11,30) wall gates.
            m @ (13..=30 | 33..=44) => {
                let occupied = if m == 30 {
                    (0..=0x0Bu8)
                        .chain([0x10])
                        .any(|s| self.mc2_slot_occupied(s))
                } else {
                    let slot = if m <= 29 { m - 13 } else { m - 16 };
                    self.mc2_slot_occupied(slot)
                };
                if occupied {
                    return;
                }
                let v3 = self.g.ent[i].f26;
                if v3 == 0 {
                    self.g.ent[i].f26 = 16;
                } else if v3 == 1 {
                    self.g.snd(41, i);
                    let dis = self.g.ent[i].id24;
                    self.fire_disposition(dis, true);
                    self.g.ent[i].flags |= 0x400;
                    self.entities_dirty = true;
                } else {
                    self.g.ent[i].f26 = v3 - 1;
                }
            }
            _ => {}
        }
    }

    /// `bytearray_38403x[slot]` (:39987-40009): is any class-5
    /// entity of model == `slot` live? (dead / reaped / segment
    /// states 0xB4/0xE8/0xEA excluded — the retail list-rebuild's
    /// exact skip set.)
    fn mc2_slot_occupied(&self, slot: u8) -> bool {
        self.g.ent.iter().skip(1).any(|c| {
            c.class64 == 5
                && c.model65 == slot
                && c.act_life >= 0
                && c.flags & 0x400 == 0
                && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
        })
    }

    /// Advance the MC2 ending sequence one tick (`sub_5E8C0`,
    /// EF:60313-60589) — verbatim phases. Approximation register: the
    /// retail moveTest (terrain-block abort) is skipped (the glue
    /// keeps the scripted carpet above ground and the tick timeouts
    /// stand); the fov dolly-zoom (phase 5) and motion blur (launch
    /// tail) are presentation, deliberately skipped; the roll/pitch
    /// auto-level tail lives app-side on the flyer.
    fn mc2_end_tick(&mut self) {
        let Some(mut s) = self.mc2_endseq else { return };
        let mut launch = false; // retail v28 — the launch tick
        match s.phase {
            // Seize control: sound 41, cancel an active Speed
            // manifestation (EF:60360-62), resolve the fly-to marker
            // — action 11 → the (14,4) mouth, action 12 → the (14,3)
            // checkpoint X (word_0x36DFC/word_0x36DFE, class/model-
            // validated EF:60367-87).
            0 => {
                self.g.snd_player(41);
                let m = self.mc2_book.ent[3] as usize;
                if m != 0 {
                    self.g.ent[m].f26 = 0;
                }
                s.target = (1..self.g.ent.len())
                    .find(|&j| {
                        let e = &self.g.ent[j];
                        e.class64 == 14 && e.model65 == s.target_model && e.flags & 0x400 == 0
                    })
                    .map_or(0, |j| j as u16);
                s.phase = 1;
            }
            // Decelerate: coast on the current yaw, bleed 4/tick
            // (EF:60390-411).
            1 => {
                let mut pos = (s.x, s.y, s.z);
                Gen::polar_step(&mut pos, s.yaw, 0, s.speed);
                (s.x, s.y, s.z) = pos;
                if s.speed.abs() <= 4 {
                    s.speed = 0;
                } else {
                    s.speed += if s.speed <= 0 { 4 } else { -4 };
                }
                if s.speed == 0 {
                    s.phase = if s.target == 0 { 4 } else { 3 };
                }
            }
            // Aim: turn toward the mouth at ≤11/tick; snap inside 11
            // (EF:60413-25).
            3 => {
                let (tx, ty) = {
                    let e = &self.g.ent[s.target as usize];
                    (e.x, e.y)
                };
                let want = Gen::angle_between(s.x, s.y, tx, ty);
                if Gen::angdist(s.yaw, want) <= 0xB {
                    s.yaw = want;
                    s.phase = 4;
                } else {
                    s.yaw =
                        ((s.yaw as i32 + Gen::turn_step(s.yaw, want, 0xB) as i32) & 0x7FF) as u16;
                }
            }
            // Zoom setup + countdown (EF:60426-56) → launch with a
            // target (6) or without (8). Retail swaps in the ending
            // data here (GTD2.DAT, EF:60449-54) — campaign material.
            4 | 5 => {
                if s.phase == 4 {
                    s.phase = 5;
                    s.counter = 12;
                }
                s.counter -= 1;
                if s.counter == 0 {
                    s.phase = if s.target == 0 { 8 } else { 6 };
                }
            }
            // LAUNCH + FLY TO THE MOUTH (EF:60457-84): re-aim every
            // tick, accelerate +8 to 200; arrive at 3D distance
            // < 0x180 or on the 512-tick timeout.
            6 | 7 => {
                if s.phase == 6 {
                    s.counter = 512;
                    s.speed = 100;
                    s.phase = 7;
                    launch = true;
                }
                s.counter -= 1;
                let mut arrived = s.counter <= 0;
                if !arrived {
                    let (tx, ty, tz) = {
                        let e = &self.g.ent[s.target as usize];
                        (e.x, e.y, e.z)
                    };
                    s.yaw = Gen::angle_between(s.x, s.y, tx, ty);
                    let mut pos = (s.x, s.y, s.z);
                    Gen::polar_step(&mut pos, s.yaw, 0, s.speed);
                    (s.x, s.y, s.z) = pos;
                    s.speed = (s.speed + 8).clamp(0, 200);
                    // 2-D (EF:60482 — `EuclideanDistXYZ` never
                    // reads z).
                    let dx = (s.x.wrapping_sub(tx) as i16) as i64;
                    let dy = (s.y.wrapping_sub(ty) as i16) as i64;
                    let _ = tz;
                    arrived = dx * dx + dy * dy < 0x180 * 0x180;
                }
                if arrived {
                    s.phase = 10;
                }
            }
            // Targetless launch: straight ahead for 128 ticks
            // (EF:60485-510).
            8 | 9 => {
                if s.phase == 8 {
                    s.counter = 128;
                    s.speed = 100;
                    s.phase = 9;
                    launch = true;
                }
                s.counter -= 1;
                if s.counter <= 0 {
                    s.phase = 10;
                } else {
                    let mut pos = (s.x, s.y, s.z);
                    Gen::polar_step(&mut pos, s.yaw, 0, s.speed);
                    (s.x, s.y, s.z) = pos;
                    s.speed = (s.speed + 8).clamp(0, 200);
                }
            }
            // Fade arm + creep (EF:60511-33): 32 fade ticks (retail
            // waits on paletteSubMod-5 — ours models the 32-tick
            // cap), creeping forward at speed 2 while it runs.
            10 | 11 => {
                if s.phase == 10 {
                    s.phase = 11;
                    s.counter = 32;
                }
                s.counter -= 1;
                if s.counter > 0 {
                    let mut pos = (s.x, s.y, s.z);
                    Gen::polar_step(&mut pos, s.yaw, 0, 2);
                    (s.x, s.y, s.z) = pos;
                } else {
                    s.phase = 12;
                }
            }
            // LEVEL END (EF:60534-43): the victory flag.
            _ => {
                self.won = true;
            }
        }
        if launch {
            // The SpeedUp sample — the same id the Speed spell plays
            // (19, EF:60552/56230), via the sequence's own call, NOT
            // the speed machinery.
            self.g.snd_player(19);
        }
        // The terrain glue (EF:60561-73), every tick: steady state =
        // ground + 128, approached at ≤128/tick — elevation
        // differences between trigger and mouth are absorbed here,
        // so the carpet cannot vertically overshoot the mouth.
        let g = self.g.ground_z(s.x, s.y) as i16;
        if s.z <= g.saturating_add(256) {
            if s.z >= g {
                s.z = g.saturating_add(128);
            } else {
                s.z = s.z.saturating_add(128);
            }
        } else {
            s.z -= 128;
        }
        self.mc2_endseq = Some(s);
    }

    /// The scripted ending pose for the app, in flyer space
    /// (x tiles, altitude tiles, z tiles, yaw radians) — Some while
    /// the demon-mouth sequence runs. The app mirrors it onto the
    /// flyer and suppresses player input (the retail actionIndex-11
    /// control seizure).
    pub fn mc2_end_pose(&self) -> Option<(f32, f32, f32, f32)> {
        const TAU: f32 = std::f32::consts::TAU;
        self.mc2_endseq.map(|s| {
            (
                s.x as f32 / 256.0,
                s.z as f32 / 256.0,
                s.y as f32 / 256.0,
                (s.yaw & 0x7FF) as f32 * (TAU / 2048.0),
            )
        })
    }

    /// Ending fade progress 0..=1 (the retail paletteSubMod-5 final
    /// fade, the phase-10/11 32-tick window). 0 while no ending
    /// fade runs.
    pub fn end_fade(&self) -> f32 {
        match self.mc2_endseq {
            Some(s) if s.phase == 11 => 1.0 - s.counter.max(0) as f32 / 32.0,
            Some(s) if s.phase >= 12 => 1.0,
            _ => 0.0,
        }
    }

    /// The level is WON — the true terminator, distinct from
    /// [`World::completed`]: MC1's Space win-exit / MC2's endGameSeq
    /// phase 0xC. The app consumes it: fade out and end the game.
    pub fn won(&self) -> bool {
        self.won
    }

    /// Which MC2 ending marker the endseq is flying to — the class-14
    /// TARGET MODEL: 3 = the (14,3) checkpoint "X" (retail action 12),
    /// 4 = the (14,4) demon mouth (action 11). None while no ending
    /// sequence runs. The campaign driver's exit-taken record: the
    /// demon mouth routes into the attached secret level
    /// (EF:60534-44 / EF:31510-48).
    pub fn mc2_exit_model(&self) -> Option<u8> {
        self.mc2_endseq.map(|s| s.target_model)
    }

    /// The MC2 level-exit map markers' `(x, z, model)` in tile units
    /// — the (11,12)/(11,31) ENDING TRIP SWITCHES (model 12 = the
    /// checkpoint-X trigger, 31 = the demon-mouth/secret trigger),
    /// NOT the (14,3)/(14,4) fly-to portals: retail's minimap plots
    /// sprites 83/84 at the TRIGGER's location (GameUI.cpp:2049-53,
    /// runtime class 0x0B models 0x0C/0x1F), unconditionally from
    /// level start, and the trip
    /// clears only the map-icon bit (:54701) — mirrored here by
    /// excluding tripped (0x400) switches.
    pub fn mc2_exit_marker_poses(&self) -> Vec<(f32, f32, u8)> {
        if !matches!(self.game, GameId::Mc2) {
            return Vec::new();
        }
        self.g
            .ent
            .iter()
            .skip(1)
            .filter(|e| e.class64 == 11 && matches!(e.model65, 12 | 31) && e.flags & 0x400 == 0)
            .map(|e| (e.x as f32 / 256.0, e.y as f32 / 256.0, e.model65))
            .collect()
    }

    /// `InitSwitchChainZaxisAndSound_6F850` (:44523): the shared
    /// arming primitive — every-8th-tick phase gate (byte 62 & 7),
    /// then the PLAYER proximity sense (class-3 model-0 chain walk;
    /// our human lives outside the pool); a quiet probe re-grounds
    /// the switch's z. Models > 3 play WAV 41 on a match (:44538) —
    /// outside the ported 0..=3 set, kept for when they land.
    fn mc2_switch_probe(&mut self, i: usize, want: bool) -> bool {
        if self.g.ent[i].f63 & 7 != 0 {
            return false;
        }
        if self.mc2_switch_overlap(i) == want {
            if self.g.ent[i].model65 > 3 {
                self.g.snd_player(41);
            }
            return true;
        }
        let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
        self.g.ent[i].z = self.g.ground_z(x, y) as i16;
        false
    }

    /// `CompareAxisWithShift_106F0` (:3726): a 2D box test — extents
    /// SUM per axis, NO z term (MC2 switches trigger at any
    /// altitude). The player's half-extents are its sprite-params row
    /// 44 `speed_6 / 2` (AddPlayer_4A920 :33333 →
    /// SetEntityIndexAndRot_49CD0 :32841-44) — zero, faithfully: the
    /// box is the switch's own `word_10 << 8` square.
    fn mc2_switch_overlap(&self, i: usize) -> bool {
        let pw = (crate::mc2::sprite_params::SPRITE_PARAMS[44].speed_6 / 2) as i32;
        let e = &self.g.ent[i];
        let (px, py, _) = self.human_pose;
        let wrap_d = |a: u16, b: u16| {
            let d = (a as i32 - b as i32) & 0xFFFF;
            (d as i16 as i32).abs()
        };
        wrap_d(px, e.x) < e.f80 as i32 + pw && wrap_d(py, e.y) < e.f82 as i32 + pw
    }

    /// Models 0/1: fire own disposition (id = the record's
    /// stageTag_12), CONSUME the released records (`sub_4A1E0(id, 1)`
    /// zeroes their types), then die (`DisableEntityDrawing04` — the
    /// cleanup pass frees it; our 0x400 flag does the same).
    fn mc2_switch_one_shot(&mut self, i: usize, want: bool) {
        if self.mc2_switch_probe(i, want) {
            let dis = self.g.ent[i].id24;
            self.fire_disposition(dis, true);
            self.g.ent[i].flags |= 0x400;
            self.entities_dirty = true;
        }
    }

    /// Models 2/3: non-consuming fire + a 10-count rearm
    /// (`dword_0x10_16`; the re-fire cadence while the player holds
    /// position is inferred from :54408-28 — the countdown decrement
    /// condition was not pinned, MC1's leave-to-rearm shape serves).
    fn mc2_switch_repeating(&mut self, i: usize, want: bool) {
        if self.g.ent[i].f26 != 0 {
            if self.mc2_switch_overlap(i) != want {
                self.g.ent[i].f26 -= 1;
            }
        } else if self.mc2_switch_probe(i, want) {
            let dis = self.g.ent[i].id24;
            self.fire_disposition(dis, false);
            self.g.ent[i].f26 = 10;
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
    /// carries sprite 44's stats halves (spawn sub_378A0).
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
                self.pending_teleport = Some((dx as f32 / 256.0, dy as f32 / 256.0));
                // PORTUSE — the same 22 as the teleport spell.
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

    /// `sub_35390` (EF:25761), class-10 action 0x24 — the MC2
    /// teleporter pad (docs/traces/mc2-class10-m50-chains-and-tail.md
    /// §2.3): hum sound 21 once on the first tick (the byte[0]-bit-1
    /// latch), then per tick a player within the pad's reach FACING
    /// it (front cone < 0xAA — the same math as the MC1 vortex)
    /// warps to the par-authored destination with sound 22; the pad
    /// re-clamps to the terrain (the ctor's +640 hover lasts one
    /// tick, verbatim); a timed pad (authored maxLife 0 = never)
    /// expires with sound 20.
    ///
    /// APPROX register:
    /// - retail warps EVERY player in the list (AI wizards included,
    ///   the NumberOfPlayers scan) — the rival-warp arm is owed with
    ///   a level that authors a pad near a rival start;
    /// - the warp-out altitude (`dword_0xA0_160x->word_160_0xc_12` +
    ///   dest terrain, trace OPEN-2) rides the consumer's own
    ///   placement, like the MC1 vortex;
    /// - `sub_5C800(player, 6)` — the blue/cyan full-screen palette
    ///   flash (mc2-class10-tail-helper-closure.md §4) — is
    ///   presentation, banked with 4.9.
    fn mc2_portal_tick(&mut self, i: usize, player: PlayerPose) {
        if self.g.ent[i].flags & 2 == 0 {
            self.g.ent[i].flags |= 2;
            self.g.snd(21, i);
        }
        let life = self.g.ent[i].act_life;
        if life > 0 {
            self.g.ent[i].act_life = life - 1;
            if life == 1 {
                self.g.ent[i].flags |= 0x400;
                self.g.snd(20, i);
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
                self.pending_teleport = Some((dx as f32 / 256.0, dy as f32 / 256.0));
                self.g.snd_player(22);
            }
        }
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

    /// Raise the top-of-screen notification (retail `SetCurrentNotif
    /// icationMessage`): `text` shown for `ticks` (retail's level-up
    /// path uses 200, the select toast 20), inked `color` (RGB). The
    /// shared message surface — spell selection/level-ups now, deaths/
    /// rival events/objectives later. Replaces any current line (last
    /// writer wins, like the single retail buffer).
    pub(crate) fn set_notification(&mut self, text: impl Into<String>, ticks: u16, color: [u8; 3]) {
        self.notification = Some(Notification {
            text: text.into(),
            timer: ticks,
            color,
        });
    }

    /// App-side OPTION toast (game speed cycling, the live F-key /
    /// letter toggles — retail echoes these on screen): the same
    /// top-of-screen line the spell toasts ride,
    /// ~2.5s, white ink to stay apart from the red spell/event line.
    /// Hash-excluded like every notification.
    pub fn notify_option(&mut self, text: impl Into<String>) {
        self.set_notification(text, 60, [255, 255, 255]);
    }

    /// The active notification (text, ink RGB) for the app to draw, or
    /// None when the line is idle/expired.
    pub fn notification(&self) -> Option<(&str, [u8; 3])> {
        self.notification
            .as_ref()
            .map(|n| (n.text.as_str(), n.color))
    }

    /// Drain this tick's sound requests plus the ambient-loop inputs
    /// the original's player tick derives (:55254-82): waves XOR wind
    /// from the terrain under the carpet, fire and market loops from
    /// emitter proximity. The original refreshes per-player countdown
    /// fields from the emitters' own handlers; the INTERIM probe here
    /// is a direct radius scan (8 tiles) over live BIG fires (class
    /// 10 m6 — the only model whose handler latches the retail fire
    /// countdown) and village houses (m45) — same audible result,
    /// exact hysteresis owed with the emitter trace.
    pub fn take_audio(&mut self, player: PlayerPose) -> AudioFrame {
        let over_water = self.g.on_water_pub(player.x, player.y);
        const AMBIENT_RANGE: i32 = 8 * 256;
        let (mut fire_near, mut market_near) = (false, false);
        for e in &self.g.ent {
            if e.flags & 1 == 0 || e.flags & 0x400 != 0 || e.class64 != 10 {
                continue;
            }
            // Fire-ambient loop: retail latches the per-player fire
            // countdown ONLY from the persistent (10,6) big fire
            // (MC2 `sub_31760`/`sub_5C870` EF:43602-14; MC1
            // `sub_252D0` remc1:28215). The (10,0) SMALL fire never
            // latches it — admitting model 0 here would drag the
            // fire-crackle loop along a meteor's per-tick spark trail
            // for the whole flight.
            let is_fire = e.model65 == 6;
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
            events: {
                let mut evs = std::mem::take(&mut self.g.sounds);
                // The mixer's channel key is the (OWNER word, id)
                // pair (remc1 sub_483C0 matches both words). The hashed
                // pending vec carries the EMITTER index — resolve it to
                // the emitter's owner tag HERE, at drain time: the
                // frame is not hashed, and the sim-side vec must stay
                // byte-stable (the "audio fixes go in mgc-audio, not
                // snd()" trap).
                for e in &mut evs {
                    if !e.player && (e.tag as usize) < self.g.ent.len() {
                        e.tag = self.g.ent[e.tag as usize].id24;
                    }
                }
                evs
            },
            over_water,
            fire_near,
            market_near,
            danger: self.g.player_danger > 0,
            speech: self.mc2_speech_cue.take(),
        }
    }

    /// Live gameplay volumes (trigger AABBs, portals) for the map
    /// debug/enhancement overlay: position + radius in tile units.
    pub fn active_volumes(&self) -> Vec<ActiveVolume> {
        let mut out = Vec::new();
        for e in &self.g.ent {
            let kind = match (e.class64, e.tick70) {
                (11, 0..=3 | 5..=12) => VolumeKind::Proximity,
                (11, 4) => VolumeKind::WinTrigger,
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
        // MC2 stage checkpoints: still-active point objectives plot
        // with the fly-to latch radius (768 engine units = 3 tiles,
        // :40803-14) so the authored route is visible for
        // troubleshooting.
        for st in &self.mc2_stages {
            if st.state != 1 || st.point == (0, 0) {
                continue;
            }
            out.push(ActiveVolume {
                x: st.point.0 as f32 / 256.0,
                z: st.point.1 as f32 / 256.0,
                radius: 3.0,
                kind: VolumeKind::Objective,
            });
        }
        out
    }

    /// sub_59E40_5A350 (:67460): fire one-shot after the watched
    /// class-5 bucket(s) stay empty through a 16-tick countdown; a
    /// non-empty probe pauses (does not reset) the countdown.
    fn kill_trigger(&mut self, i: usize, list: Option<usize>, buckets: &[u32]) {
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
        // The CommitGateVerb seam — the one genuinely REWRITTEN verb.
        let out = match self.g.verbs.commit_gate {
            // MC2's water/blocked-flag/cave-steer arm lands here
            // (Phase 3); fallback telemetry rides the sim boundary
            // (lib.rs), where &mut is available.
            CommitGateVerb::Mc1 | CommitGateVerb::Mc2 => self.g.player_wall_gate(cur, prop)?,
        };
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

    /// The cave narrow-space law for the DEVIATION mover (the
    /// faithful mover gets it inside `moveTest_5D0A0`): a point is a
    /// squeeze when its tile is sealed or its air band is tighter
    /// than clearance + fov + 384 (`sub_11E20` EF:4620-28 + the
    /// sealed check EF:59592-97 — the same predicate that makes
    /// retail refuse spaces "narrower than X", keeping the eye away
    /// from the floor-meets-ceiling seams entirely). False off-cave.
    pub fn player_cave_squeeze(&self, x: f32, z: f32) -> bool {
        if !self.g.is_cave() {
            return false;
        }
        let ex = (x.rem_euclid(256.0) * 256.0) as u16;
        let ez = (z.rem_euclid(256.0) * 256.0) as u16;
        let clr = self.mc2_carpet_row().clearance as i32;
        self.g.mc2_sealed(ex, ez) || self.g.cave_collide(100, clr, ex, ez)
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

    /// The MC2 player mover's cave ceiling clamp target (sub_5D530
    /// EF:59758-63): the player CLAMPS — no bounce, no damage — at
    /// `ceiling − 384`. None off-cave (no ceiling plane).
    pub fn player_cave_ceiling(&self, x: u16, y: u16) -> Option<i16> {
        self.g
            .is_cave()
            .then(|| (self.g.ceiling_z(x, y) - 384) as i16)
    }

    /// The MC2 player flight commit gate — `moveTest_5D0A0`
    /// (EF:59429), the [`crate::flight::mc2_move`] boundary hook
    /// (docs/traces/mc2-flight-model.md §2). Carpet head clearance
    /// `fov` = 100 (params row 44 `rotSpeed/2`, EF:33334 — a genuine
    /// constant); ground clearance comes from the map-type tuning
    /// row (`0xc`), the same row the mover reads.
    pub fn player_mc2_gate(
        &self,
        cur: (u16, u16, i16),
        prop: (u16, u16, i16),
    ) -> crate::flight::Mc2GateOut {
        let clr = self.mc2_carpet_row().clearance as i32;
        self.g.mc2_flight_gate(100, clr, cur, prop)
    }

    /// `sub_5DD50`'s wedged test for the MC2 nudge (EF:59854-81).
    pub fn player_mc2_stuck(&self, pos: (u16, u16, i16), latched: bool) -> bool {
        let clr = self.mc2_carpet_row().clearance as i32;
        self.g.mc2_flight_stuck(100, clr, pos, latched)
    }

    /// The MC2 carpet tuning row by map type (`AddPlayer_4A920`
    /// EF:33329-32: row 104 on caves, row 66 otherwise — NOT the
    /// generic default row 59; trace §0.1).
    pub fn mc2_carpet_row(&self) -> crate::flight::Mc2Row {
        if self.g.is_cave() {
            crate::flight::Mc2Row::CAVE
        } else {
            crate::flight::Mc2Row::OPEN
        }
    }

    /// The cave-block speed-up cancel (EF:59603 clears the
    /// `SpellEnabled[3]` manifestation's `word_0x2E_46` — MC2 spell
    /// 3 = the accelerate channel).
    pub fn mc2_cancel_accel(&mut self) {
        self.player.accel = 0;
        self.player.speed_boost = 0.0;
    }

    /// Pending MC2 debuff-stamp hits on the player — (slow webs,
    /// paralyze webs) since the last drain; the boundary feeds them
    /// into the flight ext's `slow_hit`/`stun_hit`.
    pub fn take_mc2_debuffs(&mut self) -> (u8, u8) {
        let out = (self.g.mc2_debuffs.slow, self.g.mc2_debuffs.stun);
        self.g.mc2_debuffs.slow = 0;
        self.g.mc2_debuffs.stun = 0;
        out
    }

    /// The sub_45410 wall gate in engine units (the faithful mover
    /// applies the routine's trailing z-floor itself).
    pub fn player_wall_gate_fixed(
        &self,
        cur: (u16, u16, i16),
        prop: (u16, u16, i16),
    ) -> Option<(u16, u16, i16)> {
        // The CommitGateVerb seam (see `player_wall_gate`).
        match self.g.verbs.commit_gate {
            CommitGateVerb::Mc1 | CommitGateVerb::Mc2 => self.g.player_wall_gate(cur, prop),
        }
    }

    /// Seam-telemetry hook for the boundary verbs the flyer consumes
    /// through `&self` closures (commit gate, flight model) — the sim
    /// boundary notes their fallbacks here, where `&mut` exists.
    pub(crate) fn note_verb_fallback(&mut self, kind: VerbKind) {
        self.g.note_verb_fallback(kind);
    }

    /// Emit a player-anchored sound from the sim boundary (the move's
    /// wind-gust flutter, remc1 :55294-99).
    pub fn push_player_sound(&mut self, id: u8) {
        self.g.snd_player(id);
    }

    /// The level's highest terrain tile in tile units, from the LIVE
    /// height plane (terrain is runtime-mutable). The extended-lift
    /// float-up cap anchors here so explicit lift can never reach a
    /// god's-eye view (deliberate).
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

    /// Fire a disposition by id (test/dev instrument — the
    /// frankenstein smoke test uses it to push EVERY authored thing
    /// through the spawn seam regardless of trigger wiring).
    /// One-shot semantics, like the trigger path.
    #[doc(hidden)]
    pub fn debug_fire_disposition(&mut self, dis: u16) {
        self.fire_disposition(dis, true);
    }

    /// Force an MC2 stage row complete (test/dev instrument — lets a
    /// level chain be exercised past objectives whose economy is
    /// still landing, e.g. the type-0 banked-mana share). Sets
    /// retail's external force-complete bit (`str_3654D_byte1 & 2`,
    /// :40737-42); the next objective pass consumes it.
    #[doc(hidden)]
    pub fn debug_complete_mc2_stage(&mut self, row: u8) {
        if let Some(s) = self.mc2_stages.iter_mut().find(|s| s.row == row) {
            s.force = true;
        }
    }

    /// The MC2 duel lock (opponent slot, held distance, tier) — the
    /// duel-machinery test oracle.
    #[doc(hidden)]
    pub fn debug_mc2_duel(&self) -> Option<(u16, i32, u8)> {
        self.mc2_duel
    }

    /// Deliver a raw melee-mailbox hit `(amount, source)` to an
    /// entity — the held-creature damage-path oracle.
    #[doc(hidden)]
    pub fn debug_mail_hit(&mut self, i: usize, amount: u32, src: u16) {
        if let Some(e) = self.g.ent.get_mut(i) {
            e.mail[0] = (amount, src);
        }
    }

    /// An m27 body's `(tick70, branches)` where each branch reports
    /// `(f71 sub-state, f63 tick counter)` — the kraken stage-command
    /// oracle.
    #[doc(hidden)]
    pub fn debug_mc2_m27_branches(&self, body: usize) -> (u8, Vec<(u8, u8)>) {
        let mut out = Vec::new();
        let mut j = self.g.ent.get(body).map_or(0, |e| e.f54) as usize;
        while j != 0 {
            let e = &self.g.ent[j];
            if e.tick70 == 233 {
                out.push((e.f71, e.f63));
            }
            j = e.f54 as usize;
        }
        (self.g.ent.get(body).map_or(0, |e| e.tick70), out)
    }

    /// The live StageVar HELD bindings as `(entity_slot, model, kind)` —
    /// the hold-gate layer's oracle (test/dev instrument). A held
    /// creature runs `sub_1D5D0`'s held action at its phase-7 wait
    /// until its gate fires (killable; kind-3/4 guardian arms live).
    #[doc(hidden)]
    pub fn debug_mc2_held(&self) -> Vec<(u16, u8, u8)> {
        self.mc2_sv_held
            .iter()
            .map(|h| {
                (
                    h.ent,
                    self.g.ent.get(h.ent as usize).map_or(0, |e| e.model65),
                    self.g.ent.get(h.ent as usize).map_or(0, |e| e.site_z as u8),
                )
            })
            .collect()
    }

    /// The MC2 manifestation's "cast in progress" timer (`word_0x2E_46`
    /// → `f26`) for a spell — the HUD glow + re-cast gate. For the castle
    /// spell (2) this is the UPGRADE LOCK, nonzero while the castle is
    /// transforming (test/dev instrument).
    #[doc(hidden)]
    pub fn debug_mc2_spell_active(&self, spell: usize) -> i16 {
        let m = self.mc2_book.ent.get(spell).copied().unwrap_or(0) as usize;
        self.g.ent.get(m).map_or(0, |e| e.f26)
    }

    /// The castle-spell manifestation's active timer (`f26`) — the
    /// UPGRADE LOCK, for whichever game (MC2 book slot 2 / MC1 owned
    /// slot 16). Nonzero while the castle is transforming (test/dev).
    #[doc(hidden)]
    pub fn debug_castle_lock(&self) -> i16 {
        let m = match self.game {
            GameId::Mc2 => self.mc2_book.ent.get(2).copied().unwrap_or(0) as usize,
            _ => self.player.owned.get(16).copied().unwrap_or(0) as usize,
        };
        self.g.ent.get(m).map_or(0, |e| e.f26)
    }

    /// Count live class-10 model-45 buildings carrying the given
    /// build-type tag (`f71`) — the type-9 destroy-building objective's
    /// per-stage oracle (test/dev instrument; level-001 vaults). Alive =
    /// present in the census (`flags & 0x400 == 0`),
    /// matching the objective's own test.
    #[doc(hidden)]
    pub fn debug_mc2_count_buildings(&self, tag: u8) -> usize {
        self.g
            .ent
            .iter()
            .skip(1)
            .filter(|e| e.class64 == 10 && e.model65 == 45 && e.f71 == tag && e.flags & 0x400 == 0)
            .count()
    }

    /// Smite every live (class, model) pool entity — life to -1, the
    /// death paths run normally next tick (test/dev instrument for
    /// exercising kill objectives without marksmanship; a level-grind
    /// checklist tool). Returns how many were hit.
    #[doc(hidden)]
    pub fn debug_smite(&mut self, class: u8, model: u8) -> usize {
        let mut n = 0;
        for e in self.g.ent.iter_mut().skip(1) {
            if e.class64 == class && e.model65 == model && e.act_life >= 0 && e.flags & 0x400 == 0 {
                e.act_life = -1;
                n += 1;
            }
        }
        n
    }

    /// Test hook (Fool's Mana): stamp a possession claim from `claimer`
    /// onto the first un-sprung decoy (a class-10 trap sphere, f52 != 0
    /// and f146 == 0), exactly as a possession bolt's ch1 mail would
    /// (mc1/combat.rs `ball_tick`). Returns the sphere's slot, or 0.
    #[doc(hidden)]
    pub fn debug_mc2_claim_fool_sphere(&mut self, claimer: u16) -> usize {
        for (slot, e) in self.g.ent.iter_mut().enumerate().skip(1) {
            if e.class64 == 10 && e.f52 != 0 && e.f146 == 0 && e.flags & 0x400 == 0 {
                e.mail[1] = (0, claimer);
                return slot;
            }
        }
        0
    }

    /// Test hook (Rebound): spawn a HOSTILE class-9 subtype-0 bolt
    /// (impact (10,0), the whitelisted fireball pair) owned by the
    /// fake enemy id `owner`, at 8.8 position (x, y, z) flying
    /// `yaw` — as an enemy shooter's launch would. Returns the slot,
    /// or 0 on a full pool.
    #[doc(hidden)]
    pub fn debug_mc2_hostile_bolt(
        &mut self,
        x: u16,
        y: u16,
        z: i16,
        yaw: u16,
        owner: u16,
    ) -> usize {
        let Some(i) = self.g.mc2_spawn_cast_proj(0, x, y, z) else {
            return 0;
        };
        let e = &mut self.g.ent[i];
        e.id24 = owner;
        e.f68 = 10;
        e.f69 = 0;
        e.f30 = yaw;
        e.f34 = yaw;
        e.f32 = 0;
        e.f36 = 0;
        e.f44 = 300;
        e.flags |= crate::mc2::proj::F_AIMED; // fly straight, no autoaim
        i
    }

    /// The class-5 model the human is currently transformed into
    /// (Metamorph, spell 4), or 0 when not transformed. The app HIDES the
    /// carpet while this is nonzero (the pooled creature draws in its
    /// place — docs/spell-audit/summon-creatures.md Part A).
    pub fn mc2_metamorph_model(&self) -> u8 {
        self.player.metamorph
    }

    /// Test hook (Magic Mine): place a persistent proximity mine at a
    /// tile with the given tier and owner (bypassing the carrier), as
    /// the carrier's landing would. Returns the mine slot, or 0.
    #[doc(hidden)]
    pub fn debug_mc2_place_mine(&mut self, cx: u16, cy: u16, tier: u8, owner: u16) -> usize {
        let (x, y) = ((cx << 8) | 0x80, (cy << 8) | 0x80);
        let lifespan = self
            .g
            .assets
            .spells
            .get(23)
            .map_or(1000, |r| r.tiers[tier.min(2) as usize].sub_spell.max(1));
        if let Some(s) = self.g.mc2_spawn_magic_mine(x, y, tier, lifespan) {
            self.g.ent[s].id24 = owner;
            return s;
        }
        0
    }

    /// Test hook (rival grave / mana reclaim): stamp
    /// a possession claim from `claimer` onto the first live (10,40)
    /// grave and run its action once, exactly as a possession bolt's
    /// ch1 mail into `grave_tick` (action 42) would. Returns
    /// `(spheres_owned_by_grave_before, spheres_now_owned_by_claimer,
    /// grave_freed)`, or None when no grave is live. Verifies the grave
    /// is reachable (targetable bit 8 kept, `f28 == 2`) as a debug
    /// assert — a broken inert grave would have neither.
    #[doc(hidden)]
    pub fn debug_mc2_possess_grave(&mut self, claimer: u16) -> Option<(usize, usize, bool)> {
        let g = self
            .g
            .ent
            .iter()
            .position(|e| e.class64 == 10 && e.model65 == 40 && e.flags & 0x400 == 0)?;
        debug_assert_eq!(self.g.ent[g].flags & 8, 8, "grave must keep targetable bit");
        debug_assert_eq!(
            self.g.ent[g].f28, 2,
            "grave must carry the ch1 claim channel"
        );
        let before = self
            .g
            .ent
            .iter()
            .filter(|e| e.class64 != 0 && e.f144 == g as u16)
            .count();
        self.g.ent[g].mail[1] = (0, claimer);
        self.g.grave_tick(g);
        let after = self
            .g
            .ent
            .iter()
            .filter(|e| e.class64 != 0 && e.f144 == claimer)
            .count();
        let freed = self.g.ent[g].class64 == 0 || self.g.ent[g].flags & 0x400 != 0;
        Some((before, after, freed))
    }

    /// Pool diagnostics (debug tooling): free slot count + a minimal
    /// live-event view.
    #[doc(hidden)]
    /// Diagnostic: the actSpeed (`f126`) of every live creature of
    /// `(class, model)` — for the flocking speed-compounding probe.
    pub fn debug_creature_speeds(&self, class: u8, model: u8) -> Vec<i16> {
        self.g
            .ent
            .iter()
            .skip(1)
            .filter(|e| e.class64 == class && e.model65 == model && e.act_life >= 0)
            .map(|e| e.f126)
            .collect()
    }

    /// Diagnostic: the FULL per-tick AI state of every live
    /// `(class, model)` creature —
    /// full-resolution position, speed triple, state, awake/leader/
    /// target links. Read-only (hash-neutral); consumed by the app's
    /// `--flock-probe` headless CSV dump.
    pub fn debug_flock_probe(&self, class: u8, model: u8) -> Vec<FlockProbeRow> {
        self.g
            .ent
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, e)| e.class64 == class && e.model65 == model && e.flags & 0x400 == 0)
            .map(|(slot, e)| FlockProbeRow {
                slot,
                id24: e.id24,
                x: e.x,
                y: e.y,
                z: e.z,
                yaw: e.f30,
                aim: e.f34,
                speed: e.f126,
                min_speed: e.f128,
                max_speed: e.f130,
                state: e.tick70,
                life: e.act_life,
                awake: e.f58,
                hold: e.site_z,
                leader: e.f52,
                target: e.f146,
                attacker: e.f40,
                cadence: e.f63,
                flags: e.flags,
            })
            .collect()
    }

    /// Diagnostic companion of [`Self::debug_flock_probe`]: the
    /// 256x256 move-block map for the behavior row of the first live
    /// `(class, model)` creature — bit 0 = roughness fence, bit 1 =
    /// tile-type block. None when no such creature lives.
    pub fn debug_block_map(&self, class: u8, model: u8) -> Option<Vec<u8>> {
        let i = self
            .g
            .ent
            .iter()
            .position(|e| e.class64 == class && e.model65 == model && e.act_life >= 0)?;
        Some(self.g.mc2_block_map(i))
    }

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
                row: e.row156,
                flags: e.flags,
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

    /// The LIVE cave ceiling plane (empty off-cave) — the renderer's
    /// fifth plane.
    pub fn ceiling_plane(&self) -> &[u8] {
        &self.g.t.ceiling
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::features::tile;

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
            ceiling: Vec::new(),
        };
        World::new(planes, &micro_things(), 1, assets())
    }

    fn away() -> PlayerPose {
        PlayerPose::from_tiles(10.0, 105.0 / 8.0, 10.0, 0.0, 0.0, 0.0)
    }

    fn at_trigger() -> PlayerPose {
        PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0)
    }

    /// MC2's level-init hand assignment (`InitialiseSpells_54A50`
    /// EF:38755-62): first enabled index → LEFT, second → RIGHT.
    /// MC2's canonical order is the identity (`spellIndex_D94FF`,
    /// GameUI.cpp:59), so it is always the two lowest owned indices —
    /// Fireball (0) and Possession (1) with every authored level row.
    ///
    /// Player report: "in level 003 the left hand is Beyond Sight and
    /// the right is possession; in 001 it's speed + possession". The
    /// batch grant was running the jar-PICKUP law, which binds left,
    /// then right, then overwrites LEFT for every spell past the
    /// second — leaving left = the LAST granted and right = the
    /// SECOND. This test pins the level-start law; the pickup law is
    /// pinned below it and must stay as it is.
    #[test]
    fn mc2_level_start_hands_are_the_two_lowest_indices() {
        // mc2:003's authored human row.
        let grants: Vec<(u8, i32)> = [0u8, 1, 2, 3, 4, 6, 11, 12]
            .iter()
            .map(|&s| (s, 0i32))
            .collect();
        let mut w = mc2_flat_world();
        w.mc2_grant_plausible(&grants);
        assert_eq!(w.mc2_book.left, 0, "LEFT = Fireball, not the last granted");
        assert_eq!(w.mc2_book.right, 1, "RIGHT = Possession");

        // Order-independent, like MC1's.
        let mut rev = mc2_flat_world();
        let mut back = grants.clone();
        back.reverse();
        rev.mc2_grant_plausible(&back);
        assert_eq!((rev.mc2_book.left, rev.mc2_book.right), (0, 1));

        // Every MC2 world is seeded with 0 and 1 at construction, so a
        // later batch must not disturb the pair no matter what it adds
        // — "always fireball + possession", the player's requirement.
        let mut extra = mc2_flat_world();
        assert_eq!((extra.mc2_book.left, extra.mc2_book.right), (0, 1));
        extra.mc2_grant_plausible(&[(9, 0), (21, 0), (25, 0)]);
        assert_eq!(
            (extra.mc2_book.left, extra.mc2_book.right),
            (0, 1),
            "high-index grants must not steal a hand"
        );

        // The PICKUP law is untouched: collecting a further spell with
        // both hands full overwrites the LEFT hand (EF:55735-49).
        w.mc2_dev_grant_for_test(13);
        assert_eq!(w.mc2_book.left, 13, "a pickup still takes the left hand");
        assert_eq!(w.mc2_book.right, 1, "...and leaves the right alone");
    }

    /// Retail's level-init hand assignment (`sub_3DD50_3E090`
    /// :49213-49254): LEFT/RIGHT = the first two owned spells walked in
    /// BOOK order `byte_99B88` = `[0, 3, 2, 16, 1, ...]`, NOT in
    /// ascending spell id.
    ///
    /// Player report: "you start with fireball + heal, it should be
    /// fireball + possession". The port bound hands incrementally as
    /// spells were granted, and every caller grants ascending, so Heal
    /// (id 1) took the right hand — but Heal is book position 5,
    /// behind Possess, Accelerate and Castle, and can only reach a
    /// hand when nothing above it is owned.
    #[test]
    fn level_start_hands_follow_book_order_not_spell_id() {
        // The mc1:005 owned set.
        let mut w = flat_world();
        w.grant_spells(&[0, 1, 2, 3, 4, 16, 23]);
        assert_eq!(w.player.left, Some(SpellId(0)), "LEFT = Fireball");
        assert_eq!(
            w.player.right,
            Some(SpellId(3)),
            "RIGHT = Possess (book pos 2), NOT Heal (id 1, book pos 5)"
        );

        // Order-independent: retail rebuilds from the owned SET, so the
        // same spells granted in reverse must bind identically.
        let mut rev = flat_world();
        rev.grant_spells(&[23, 16, 4, 3, 2, 1, 0]);
        assert_eq!(
            (rev.player.left, rev.player.right),
            (w.player.left, w.player.right),
            "hand binding must not depend on grant order"
        );

        // Fewer than two owned leaves the other hand empty (retail 255).
        let mut one = flat_world();
        one.grant_spells(&[7]);
        assert_eq!(one.player.left, Some(SpellId(7)));
        assert_eq!(one.player.right, None, "one spell leaves RIGHT empty");

        // Heal DOES reach a hand when it outranks everything owned:
        // book order puts 1 (pos 5) ahead of 7 (pos 11).
        let mut heal = flat_world();
        heal.grant_spells(&[1, 7]);
        assert_eq!(
            (heal.player.left, heal.player.right),
            (Some(SpellId(1)), Some(SpellId(7)))
        );
    }

    /// The m13 archer bolt's constructor uses the DOUBLING sprite
    /// setter (`sub_370A0_37460`, :46274) where every other class-9
    /// ctor uses the plain one — so the arrow carries twice the
    /// collision half-extents. The port applied the plain setter and
    /// gave every archer bolt (m4/m9/m10 creatures + the m15 castle
    /// guard) a half-size hitbox.
    ///
    /// Also pins m9's re-skin (:21957): row 203 is sprite family base
    /// 215 against 195's base 193, but identical 45x60 size and
    /// 5-view fold — cosmetic only, no geometry change.
    ///
    /// Like the boulder, no golden fixture reaches this path: nothing
    /// in level-005 or level-032 ever fires a bolt.
    #[test]
    fn archer_bolt_has_double_extents_and_m9_reskin_is_cosmetic() {
        let mut w = flat_world();
        let (bx, by, bz) = (100 * 256, 100 * 256, (100i16 / 8 + 4) * 256);

        // Row 195 = 45x60 -> plain halves would be 22/22/30.
        let p = w.g.spawn_bolt(bx, by, bz).unwrap();
        assert_eq!(w.g.ent[p].type86, 195);
        let box_of = |w: &World, i: usize| (w.g.ent[i].f80, w.g.ent[i].f82, w.g.ent[i].f84);
        assert_eq!(box_of(&w, p), (44, 44, 60), "the arrow's box is DOUBLED");
        assert_eq!(w.g.ent[p].f78, 30, "+78 is never doubled");

        // m9's override: new billboard family, same geometry.
        w.g.set_sprite_x2(p, 203);
        assert_eq!(w.g.ent[p].type86, 203);
        assert_eq!(box_of(&w, p), (44, 44, 60), "203 changes art, not geometry");

        // Idempotent: retail calls it twice on the same entity (ctor
        // :46274 then thunk :21928) and the box must not reach 4x.
        w.g.set_sprite_x2(p, 203);
        assert_eq!(
            box_of(&w, p),
            (44, 44, 60),
            "repeat calls must not grow the box"
        );

        // The sibling boulder ctor (:46297) stays on the PLAIN setter:
        // row 196 = 128x100 -> undoubled halves.
        let b = w.g.spawn_slow_bolt(bx, by, bz).unwrap();
        assert_eq!(w.g.ent[b].type86, 196);
        assert_eq!(box_of(&w, b), (64, 64, 50), "the boulder is NOT doubled");
    }

    /// The Troll/Ape boulder (class-9 m14, flight state 15) must be
    /// SILENT in flight and speak only through its `(10,0)` impact.
    ///
    /// Player report: the stone-throwers "sound like arrows being
    /// shot". Cause: state 15 was aliased onto state 13's handler,
    /// whose first tick rolls ids 33-36 — the `arrow1`..`arrow4`
    /// samples, and `:63799` is the ONLY site in the whole binary that
    /// emits them. The arrow roll is FAITHFUL for state 13's real
    /// users (the m4/m9/m10 archer creatures and the m15 castle
    /// guard), so the second half of this test pins that it survives:
    /// the fix must not over-correct into silencing the archers.
    ///
    /// No golden fixture reaches state 15 — level-005 and level-032
    /// are full of (5,7) trolls but neither script ever lands a throw
    /// — which is exactly why the alias went unnoticed.
    #[test]
    fn troll_boulder_is_silent_in_flight_and_booms_on_impact() {
        // Drive the WHOLE world so the spawned impact entity ticks
        // too — the boom belongs to the effect, not the projectile.
        // Sounds accumulate because nothing drains them here.
        let fly = |w: &mut World| {
            let mut ids = Vec::new();
            for _ in 0..48 {
                w.tick(away(), PlayerCommand::default());
                ids.extend(w.g.sounds.drain(..).map(|s| s.id));
            }
            ids
        };

        // The boulder: class-9 m14 / state 15, as sub_1AE30 arms it
        // (780 damage, impact descriptor (10,0)).
        let mut w = flat_world();
        let (bx, by, bz) = (100 * 256, 100 * 256, (100i16 / 8 + 4) * 256);
        let b = w.g.spawn_slow_bolt(bx, by, bz).unwrap();
        w.g.arm_projectile(b, 1, 3, 0xFF, 0, bx + 4096, by, bz, 780, 0);
        assert_eq!(w.g.ent[b].tick70, 15, "fixture must be state 15");
        w.g.sounds.clear();
        let ids = fly(&mut w);
        assert!(
            !ids.iter().any(|&id| (33..=36).contains(&id)),
            "the boulder must NOT roll the arrow quartet: {ids:?}"
        );
        assert!(
            ids.contains(&3),
            "the boulder's (10,0) impact must sound (sub_3A490 -> :28114): {ids:?}"
        );

        // The archer bolt: state 13 keeps its arrow roll — faithful
        // retail asset reuse, not a bug to "fix".
        let mut w = flat_world();
        let a = w.g.spawn_bolt(bx, by, bz).unwrap();
        w.g.arm_projectile(a, 1, 3, 0xFF, 0, bx + 4096, by, bz, 250, 0);
        assert_eq!(w.g.ent[a].tick70, 13, "fixture must be state 13");
        w.g.sounds.clear();
        let ids = fly(&mut w);
        assert!(
            ids.iter().any(|&id| (33..=36).contains(&id)),
            "the archer bolt keeps arrow1..arrow4: {ids:?}"
        );
    }

    #[test]
    fn genie_steal_flash_drains_stationary_player() {
        // Repro of the level-042 genie mana-steal gap: a genie-owned
        // ch3 steal flash (state 25) sitting on the player must drain
        // the pool. Big extents isolate the mana path from overlap
        // geometry.
        let mut w = flat_world();
        w.player.mana = 1000;
        w.player.mana_max = 1000;
        w.player.grace = 0; // past spawn invulnerability
        let pose = PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let flash =
            w.g.spawn_effect(25, 100u16 << 8, 100u16 << 8, 800)
                .expect("flash slot");
        w.g.ent[flash].id24 = 9; // a non-player owner (a "genie")
        w.g.ent[flash].f44 = 3000;
        w.g.ent[flash].f80 = 30000;
        w.g.ent[flash].f82 = 30000;
        w.g.ent[flash].f84 = 30000;
        let before = w.player.mana;
        // Two ticks: the flash writes ch3 on its first tick, the apply
        // pass drains on the same/next.
        w.tick(pose, PlayerCommand::default());
        w.tick(pose, PlayerCommand::default());
        assert!(
            w.player.mana + 500 < before,
            "genie steal must drain the player (was {before}, now {})",
            w.player.mana
        );
    }

    #[test]
    fn genie_seeker_end_to_end_drains_stationary_player() {
        // The genie's actual steal seeker (m8 projectile, payload 3000,
        // detonating into the ch3 flash 25) fired at a stationary player
        // must land a drain.
        let mut w = flat_world();
        let pose = PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let (px, py, pz) = (25728u16, 25728u16, 3360i16);
        w.tick(pose, PlayerCommand::default());
        w.player.mana = 1000;
        w.player.mana_max = 1000;
        w.player.grace = 0;
        let before = w.player.mana;
        // Fire from 3 tiles west, aimed at the player.
        let seeker = w.g.spawn_seeker(px - 3 * 256, py, pz).expect("seeker slot");
        w.g.arm_projectile(seeker, 9, 3, 0xFF, PLAYER_TARGET, px, py, pz, 3000, 25);
        let mut min_mana = before;
        for _ in 0..12 {
            w.tick(pose, PlayerCommand::default());
            min_mana = min_mana.min(w.player.mana);
        }
        assert!(
            min_mana + 500 < before,
            "one seeker should land a drain (min {min_mana}, before {before})"
        );
    }

    #[test]
    fn invincibility_is_life_only_mana_steal_still_drains() {
        // God-mode must NOT block the genie mana steal (ch3). Life stays
        // immune; mana drains.
        let mut w = flat_world();
        w.invincible = true;
        w.player.mana = 1000;
        w.player.mana_max = 1000;
        w.player.grace = 0;
        let life_before = w.player.life;
        let pose = PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let flash =
            w.g.spawn_effect(25, 100u16 << 8, 100u16 << 8, 800)
                .expect("flash slot");
        w.g.ent[flash].id24 = 9;
        w.g.ent[flash].f44 = 3000;
        w.g.ent[flash].f80 = 30000;
        w.g.ent[flash].f82 = 30000;
        w.g.ent[flash].f84 = 30000;
        w.tick(pose, PlayerCommand::default());
        w.tick(pose, PlayerCommand::default());
        assert!(
            w.player.mana < 500,
            "steal drains even under invincibility (was 1000, now {}) — not the old wipe-all",
            w.player.mana
        );
        assert_eq!(w.player.life, life_before, "life stays immune");
        assert_eq!(w.player.state, LifeState::Alive, "still can't be killed");
    }

    #[test]
    fn hidden_worlds_spell20_stats_diverge_only_at_row_20() {
        // Fire Storm (20) → homing meteor is the ONLY table divergence
        // (SURVEY-MC1HW §3b): count 51→26, castle_req 12000→60000,
        // damage 24464→5000. Every other row equals base MC1 — the
        // guard that keeps MC1 goldens pinned.
        let mc1 = crate::mc1::spells::spells(GameId::Mc1);
        let hw = crate::mc1::spells::spells(GameId::Mc1Hw);
        for i in 0..SPELL_COUNT {
            if i == 20 {
                continue;
            }
            assert_eq!(mc1[i], hw[i], "spell {i} must match base MC1");
        }
        assert_eq!(hw[20].count, 26, "HW burst count");
        assert_eq!(hw[20].castle_req, 60000, "HW castle req");
        assert_eq!(hw[20].damage, 5000, "HW damage");
        assert_eq!(mc1[20].count, 51, "base count untouched");
        assert_eq!(mc1[20].damage, 24464, "base damage untouched");
    }

    #[test]
    fn hidden_worlds_verbset_wiring_preserves_discriminants() {
        use crate::verbs::{TargetingVerb, VerbSet};
        assert_eq!(GameId::Mc1Hw.verbs().targeting, TargetingVerb::Mc1Hw);
        assert_eq!(GameId::Mc1.verbs().targeting, TargetingVerb::Mc1);
        assert_eq!(GameId::Mc2.verbs().targeting, TargetingVerb::Mc2);
        // The HW column is the MC1 column with only targeting flipped.
        assert_eq!(VerbSet::MC1HW.awake, VerbSet::MC1.awake);
        assert_eq!(VerbSet::MC1HW.flight, VerbSet::MC1.flight);
        assert_eq!(VerbSet::MC1HW.commit_gate, VerbSet::MC1.commit_gate);
        // Mc1/Mc2 discriminants MUST precede Mc1Hw: the VerbSet feeds the
        // state hash, so inserting the HW variant anywhere but last would
        // move every MC2 golden.
        assert!((TargetingVerb::Mc1 as u8) < (TargetingVerb::Mc1Hw as u8));
        assert!((TargetingVerb::Mc2 as u8) < (TargetingVerb::Mc1Hw as u8));
    }

    #[test]
    fn hidden_worlds_firewall_child_homes_in_the_widened_cone() {
        // The m16 Fire Storm child (state 17) runs acquire case 0x10 in
        // HW (yaw cone 0x100) but has NO acquire case in base MC1. A
        // creature at yaw offset 0xA0 (> 0x71, < 0x100), pitch aligned,
        // is picked up only under HW (SURVEY-MC1HW §3a).
        fn acquires(game: GameId) -> bool {
            let planes = Planes {
                height: vec![0; 0x10000],
                tile_type: vec![5; 0x10000],
                shading: vec![32; 0x10000],
                angle: vec![5; 0x10000],
                ceiling: Vec::new(),
            };
            let mut w = World::new_for_game(planes, &[], 1, assets(), game);
            let (bx, by, bz) = (100u16 << 8, 100u16 << 8, 1000i16);
            let bolt = w.g.spawn_firewall_bolt(bx, by, bz).expect("bolt slot");
            w.g.ent[bolt].id24 = PLAYER_TARGET;
            // A live class-5 creature 10 tiles east, same altitude.
            let (cx, cy) = (110u16 << 8, 100u16 << 8);
            let cre = w.g.new_event().expect("creature slot");
            {
                let c = &mut w.g.ent[cre];
                c.class64 = 5;
                c.f58 = 1; // awake / alive
                c.act_life = 100;
                c.tick70 = 1; // != 120 (the asleep state)
                c.id24 = 7; // not the bolt's owner
                c.f78 = 0;
                c.x = cx;
                c.y = cy;
                c.z = bz;
            }
            // Aim so the creature sits exactly 0xA0 off the bolt's yaw.
            let a = Gen::angle_between(bx, by, cx, cy);
            w.g.ent[bolt].f30 = (a + 0xA0) & 0x7FF;
            w.g.ent[bolt].f32 = 0;
            w.g.ent[bolt].f146 = 0;
            let ctx = MobCtx {
                px: 10 << 8,
                py: 10 << 8,
                pz: 0,
                pyaw: 0,
                pmana: 0,
            };
            w.g.proj_tick(bolt, &ctx);
            w.g.ent[bolt].f146 != 0
        }
        assert!(
            acquires(GameId::Mc1Hw),
            "HW firewall child homes (case 0x10, cone 0x100)"
        );
        assert!(
            !acquires(GameId::Mc1),
            "base MC1 firewall child flies straight (no case 16)"
        );
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
            ceiling: Vec::new(),
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
        assert!(
            short.0 < 120.0 && short.0 > 119.2,
            "shortened, x={}",
            short.0
        );

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
            ceiling: Vec::new(),
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
        let thrust = crate::FlightInput {
            thrust: 1.0,
            ..Default::default()
        };
        for _ in 0..600 {
            sim.step(&thrust);
            assert!(sim.flyer.x < 120.0, "wall crossed at x={}", sim.flyer.x);
        }
        assert!(
            sim.flyer.x > 119.0,
            "the flyer did reach the wall, x={}",
            sim.flyer.x
        );
    }

    #[test]
    fn deferred_things_stay_latent_until_triggered() {
        let mut w = flat_world();
        assert_eq!(
            w.live_things().len(),
            0,
            "dis_id!=0 things must not spawn at init"
        );
        for _ in 0..64 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.live_things().len(), 0);
        let center = tile(110, 110);
        assert_eq!(
            w.planes().height[center],
            100,
            "crater must not dig while latent"
        );
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
            ceiling: Vec::new(),
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
            ceiling: Vec::new(),
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
        assert_eq!(
            w.live_things().len(),
            1,
            "segments hidden from entity lists"
        );

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
        // WANDER's scans are awake-gated in the original — a distant
        // crowd must never form packs and ride the unbounded pack
        // accel.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        let things: Vec<Thing> = (0..8)
            .map(|k| bee(k, 100 + (k % 3) as u16, 100 + (k / 3) as u16))
            .collect();
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
        // catch-up adds a bounded +16 per chain level. The runaway
        // failure mode reached many tiles per tick and kept growing —
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
            ceiling: Vec::new(),
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
                let p = if (40..80).contains(&t) {
                    at_trigger()
                } else {
                    away()
                };
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
    /// combat tests were written against. Invincibility pins the
    /// dev-player semantics these tests assume (damage totaled from
    /// tick 0, no death mid-fight); mortality has its own tests.
    fn rapid_fire(w: &mut World) {
        w.set_dev_spells(true);
        w.set_invincible(true);
        w.player.left = Some(crate::mc1::spells::SpellId(23));
    }

    /// A flat world holding one load-time creature and nothing else —
    /// no crater rims for a chaser to wall-death on.
    fn bare_creature_world(model: u16) -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            // The kraken (m6) is water-masked and now dies on land
            // (the :21225-91 mover rule) — the bare fixture is an
            // ocean so every model can live on it.
            tile_type: vec![0; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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

    // ---- mortality -------------------------------------------------------

    /// A landed pose: ground 100*32 = 3200, the touchdown floor is
    /// ground+128 = 3328 (firing_line's 3360 stays airborne).
    fn grounded_line() -> PlayerPose {
        PlayerPose::level((112 << 8) + 128, (116 << 8) + 128, 3328, 0)
    }

    fn hit_player(w: &mut World, amt: u32, src: u16) {
        w.g.mail_write(crate::mc1::combat::MailTarget::Player, 0, amt, src);
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
        let b =
            w.g.spawn_mana_ball((112 << 8) + 128, (114 << 8) + 128, 3200)
                .unwrap();
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
        assert_eq!(
            w.g.ent[b as usize].f144, grave,
            "the grave inherits the ball"
        );
        let jars = w
            .debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 12 && e.state == 3)
            .count();
        assert!(jars > 0, "the spell inventory scattered as decaying jars");

        // 6) Castle-less respawn = the level is lost and restarts.
        w.tick(
            grounded_line(),
            PlayerCommand {
                respawn: true,
                ..Default::default()
            },
        );
        assert!(w.take_restart(), "castle-less death restarts the level");
        assert!(w.vitals().lost);
    }

    #[test]
    fn death_with_a_castle_respawns_there_with_fresh_grace() {
        let mut w = bare_creature_world(2);
        w.set_dev_spells(true);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let c =
            w.g.spawn_castle((140 << 8) + 128, (140 << 8) + 128)
                .unwrap();
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
        assert_eq!(
            owned_before, 0,
            "ownership rides the death slots while dead"
        );

        w.tick(
            grounded_line(),
            PlayerCommand {
                respawn: true,
                ..Default::default()
            },
        );
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
        let owned = w.g.spawn_creature(2, 112 << 8, 110 << 8, 3200).unwrap();
        w.g.ent[owned].id24 = PLAYER_TARGET;
        let boss = w.g.spawn_creature(16, 111 << 8, 110 << 8, 3200).unwrap();
        let wild_life = w.g.ent[1].act_life;
        assert!(wild_life > 0);

        // The castle rises straight under them (the level-0 build
        // skips the space gate — the initial cast is single-step).
        let c = w.g.spawn_castle(112 << 8, 110 << 8).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..40 {
            w.tick(
                PlayerPose::level(90 << 8, 90 << 8, 3400, 0),
                PlayerCommand::default(),
            );
        }
        assert_eq!(count(&w, 5, 2), 1, "exactly one m2 survives...");
        assert_eq!(w.g.ent[owned].id24, PLAYER_TARGET);
        assert!(
            w.g.ent[owned].act_life > 0,
            "...the OWNED one (owner immunity)"
        );
        assert!(
            w.g.ent[boss].act_life > 0,
            "m16 is exempt from the execution"
        );
        let (kills, _, _) = w.combat_stats();
        assert_eq!(kills, 1, "the execution credits the castle owner");
    }

    #[test]
    fn castle_downgrade_ejects_mana_and_demolish_razes() {
        let mut w = bare_creature_world(2);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let pose = PlayerPose::level(90 << 8, 90 << 8, 3400, 0);
        let c = w.g.spawn_castle(140 << 8, 140 << 8).unwrap();
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
        w.g.mail_write(crate::mc1::combat::MailTarget::Pool(c), 0, 45_000, 1);
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
        w.tick(
            pose,
            PlayerCommand {
                demolish: true,
                ..Default::default()
            },
        );
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        assert!(w.loadout().castle.is_none(), "the demolish razed it");
        assert_eq!(count(&w, 3, 2), 0, "the entity is gone");
    }

    /// A lethal (here the demolish key) landing MID-TRANSFORMATION must
    /// defer until the castle is established — the original's standing
    /// tick is the only damage processor. Processing it under a live
    /// painter collapses the footprint while the painter keeps
    /// painting, leaving castle terrain with no castle.
    #[test]
    fn demolish_during_the_build_defers_until_established() {
        let mut w = bare_creature_world(2);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200);
        let pose = PlayerPose::level(90 << 8, 90 << 8, 3400, 0);
        let c = w.g.spawn_castle(140 << 8, 140 << 8).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        // Two ticks in: the painter is mid-flight, the castle waits.
        w.tick(pose, PlayerCommand::default());
        w.tick(pose, PlayerCommand::default());
        assert_eq!(w.g.ent[c].f59, 1, "mid-transformation wait state");
        w.tick(
            pose,
            PlayerCommand {
                demolish: true,
                ..Default::default()
            },
        );
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
    fn final_destruction_marks_the_terrain_dirty() {
        // The un-stamp runs inside castle_tick — with no follow-up
        // painter at the final destruction, the renderer only learns
        // of the flattened tower through the Gen terrain_dirty merge.
        // Height assertions live in the real-BUILD.DAT integration test
        // (tests/spell_castle.rs).
        let mut w = flat_world();
        let c = w.g.spawn_castle(110 << 8, 110 << 8).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        for _ in 0..80 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.loadout().castle.map(|(_, _, l)| l), Some(1));
        w.terrain_dirty = false;
        w.tick(
            away(),
            PlayerCommand {
                demolish: true,
                ..Default::default()
            },
        );
        assert!(
            w.terrain_dirty,
            "the destruction tick re-uploads the flattened footprint"
        );
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(count(&w, 3, 2), 0, "the castle is gone");
    }

    #[test]
    fn fireball_kills_and_the_corpse_drops_a_mana_ball() {
        let mut w = bare_creature_world(2);
        rapid_fire(&mut w);
        assert_eq!(count(&w, 5, 2), 1, "the creature spawned");
        // Hold fire from the firing line: the aim assist locks on,
        // the fire's 400-damage broadcast whittles the 3000 life.
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
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
            w.tick(
                firing_line(),
                PlayerCommand {
                    fire_left: true,
                    ..Default::default()
                },
            );
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
    fn fireball_snaps_to_offaxis_targets() {
        // sub_52B30's per-tick re-acquire (:62815 → sub_54520): a
        // bolt launched ~4° wide of an awake creature snaps to it
        // mid-flight (the spell autoaim). Target = the stationary
        // militia; the fireball row's authentic caps
        // (v_2 = 5/tick yaw) can't run a pursuit curve onto a fast
        // lateral mover — which is the retail "crows are
        // near-impossible to hit".
        let mut w = bare_creature_world(4);
        rapid_fire(&mut w);
        let b = find_slot(&w, 5, 4);
        let start = w.g.ent[b].act_life;
        let off = PlayerPose::level((112 << 8) + 128, (128 << 8) + 128, 3360, 0x18);
        for _ in 0..6 {
            w.tick(
                off,
                PlayerCommand {
                    fire_left: true,
                    ..Default::default()
                },
            );
        }
        for _ in 0..200 {
            w.tick(off, PlayerCommand::default());
        }
        assert!(
            w.g.ent[b].act_life < start || w.g.ent[b].class64 != 5,
            "an off-axis fireball snaps onto the bee"
        );
    }

    #[test]
    fn wyvern_aggros_the_player_on_sight() {
        // m16 inherits the shared awake-gated wizard scan (sub_20710
        // calls sub_19D70 first) — no provocation needed. The player
        // just hovers in range.
        let mut w = bare_creature_world(16);
        w.set_invincible(true);
        // 14 tiles out — inside the 18-tile scan and 24-tile awake
        // radii but far enough for a stable approach bearing (the
        // 0xE3 burst cone can't align during a close orbit).
        let pose = PlayerPose::level((112 << 8) + 128, (124 << 8) + 128, 3360, 0);
        let mut hostile = false;
        for _ in 0..2000 {
            w.tick(pose, PlayerCommand::default());
            if w.player_damage_taken() > 0 {
                hostile = true;
                break;
            }
        }
        assert!(hostile, "an unprovoked wyvern opens fire on sight");
    }

    #[test]
    fn wyvern_hunts_and_burns_houses() {
        // sub_20710's custom layer (:26033-58): nearest house in
        // v_28², no cone, no awake gate — wyverns wreck dwellings
        // with nobody around.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let t = |slot, class, model, x, y| Thing {
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
        let things = vec![t(0, 5, 16, 112, 110), t(1, 10, 45, 115, 110)];
        let mut w = World::new(planes, &things, 3, assets());
        let house = find_slot(&w, 10, 45);
        let start = w.g.ent[house].act_life;
        let wyv = find_slot(&w, 5, 16);
        let mut chased = false;
        let mut wounded = false;
        for _ in 0..3000 {
            w.tick(away(), PlayerCommand::default());
            if w.g.ent[wyv].tick70 == 98 && w.g.ent[wyv].f146 == house as u16 {
                chased = true;
            }
            if w.g.ent[house].class64 != 10 || w.g.ent[house].act_life < start {
                wounded = true;
                break;
            }
        }
        assert!(chased, "the wyvern targets the house and enters its chase");
        assert!(wounded, "wyvern flame reaches the dwelling");
    }

    #[test]
    fn griffon_peaceful_until_hit_then_rebounds_and_retaliates() {
        use crate::mc1::combat::MailTarget;
        use crate::mc1::mobs::PLAYER_TARGET;
        let mut w = bare_creature_world(8);
        w.set_invincible(true);
        let g = find_slot(&w, 5, 8);
        // Short peaceful window: a longer one lets the griffon wander
        // past the 24-tile awake radius, where the (verbatim) awake-
        // gated damage intake would just bank the provoking hit.
        for _ in 0..200 {
            w.tick(firing_line(), PlayerCommand::default());
        }
        assert_eq!(w.player_damage_taken(), 0, "unprovoked griffon holds fire");
        assert_eq!(
            w.g.ent[g].flags & 0x8000,
            0,
            "no deflection while peaceful — the first hit must land"
        );
        // One wizard-source hit provokes it (sub_1CA50 :23455-58)...
        w.g.mail_write(MailTarget::Pool(g), 0, 500, PLAYER_TARGET);
        let mut deflecting = false;
        let mut mauled = false;
        for _ in 0..1500 {
            w.tick(firing_line(), PlayerCommand::default());
            deflecting |= w.g.ent[g].flags & 0x8000 != 0;
            mauled |= w.player_damage_taken() > 0;
        }
        // ...and the attack state raises the permanent deflection bit
        // (sub_1CE30 :23552) while the beam thunk answers.
        assert!(deflecting, "attacking griffon raises the rebound bit");
        assert!(mauled, "provoked griffon fights back");
    }

    #[test]
    fn bee_lunges_at_triple_speed_after_the_sting() {
        // sub_1B3C0 (:22346-47): the sting recoils and arms +26; the
        // tick the cooldown expires the bee bursts to 3x max speed.
        let mut w = bare_creature_world(2);
        w.set_invincible(true);
        let b = find_slot(&w, 5, 2);
        let max = w.g.ent[b].f128;
        let mut lunged = false;
        for _ in 0..3000 {
            w.tick(grounded_line(), PlayerCommand::default());
            if w.g.ent[b].f126 == 3 * max {
                lunged = true;
                break;
            }
        }
        assert!(lunged, "the post-sting lunge reaches 3x max speed");
    }

    #[test]
    fn genie_blinks_ambushes_and_steals_mana() {
        // sub_1DFE0's mana hunt (:24523-46, no range gate) → the
        // sub_1E770 ambush blink → the blink cycle → chase seekers →
        // the (10,25) steal flash on the player.
        let mut w = bare_creature_world(11);
        w.set_invincible(true);
        let g = find_slot(&w, 5, 11);
        let (x0, y0) = (w.g.ent[g].x, w.g.ent[g].y);
        let mana0 = w.player.mana;
        assert!(mana0 > 0, "the hunt needs a mana-holding wizard");
        let mut blinked = false;
        let mut flashed = false;
        for _ in 0..2000 {
            let (px, py) = (w.g.ent[g].x, w.g.ent[g].y);
            w.tick(firing_line(), PlayerCommand::default());
            let e = &w.g.ent[g];
            if e.class64 == 5 {
                let jump = crate::engine::features::Gen::dist2_sq(px, py, e.x, e.y);
                // One-tick displacement far beyond move speed = a blink.
                if jump > 1024 * 1024 {
                    blinked = true;
                }
            }
            flashed |= count(&w, 10, 25) > 0;
            if blinked && flashed {
                break;
            }
        }
        let _ = (x0, y0);
        assert!(blinked, "the genie teleports (ambush/blink cycle)");
        assert!(
            flashed,
            "the steal seeker lands the (10,25) mana-drain flash"
        );
    }

    #[test]
    fn worm_chain_dies_from_the_head_and_every_corpse_drops() {
        let mut w = bare_creature_world(0);
        rapid_fire(&mut w);
        assert_eq!(count(&w, 5, 0), 17, "head + 16 segments");
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        let mut cleared = false;
        for _ in 0..3000 {
            w.tick(firing_line(), fire);
            if count(&w, 5, 0) == 0 {
                cleared = true;
                break;
            }
        }
        assert!(
            cleared,
            "the whole chain dies (segments corpse with the head)"
        );
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
    /// The (5,10) doomsday DEATH SCRIPT (mc2::doomsday states
    /// 3→12→…→15): the immortal clamp pins damaged life to 8, which
    /// trips state 3's `life < 10` → the scripted extinction — the
    /// (10,17) sphere, the creature mass-kill + global life reset,
    /// and the (10,9) APOCALYPSE dome with the extinction latch set
    /// (docs/traces/mc2-class5-m10-doomsday.md §2.2 cases 0xC-0xF).
    #[test]
    fn mc2_doomsday_death_script() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        let things = vec![th(1, 5, 10, 100, 100), th(2, 5, 1, 90, 90)];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.set_mc2_doom_level(true);
        w.tick(away(), PlayerCommand::default());
        let p = find_slot(&w, 5, 10);
        // NATURAL escalation: the player parked INSIDE the 0xA00
        // proximity gate stands in for retail's detailed-render pass
        // arming `f44 |= 0x40` each frame, so the machine walks its
        // whole opening — flatten → kill-all → wind-down → doom-meter
        // — into the 2..0xB attack cycle on its own.
        let near = PlayerPose::level((104 << 8) | 128, (100 << 8) | 128, 3260, 0);
        let mut reached = false;
        for _ in 0..2000 {
            w.tick(near, PlayerCommand::default());
            if (2..=0xB).contains(&w.g.ent[p].f71) {
                reached = true;
                break;
            }
        }
        assert!(reached, "the armed machine escalates out of state 1");
        // Pound it (life shortened only so the 300/tick intake cap
        // floors it inside the loop): the read pins life to 8 —
        // never a kill by damage alone, but 8 < 10 IS state 3's
        // death trigger.
        w.g.ent[p].act_life = 9000;
        for _ in 0..40 {
            w.g.mail_write(MailTarget::Pool(p), 0, 60000, PLAYER_TARGET);
            w.tick(near, PlayerCommand::default());
        }
        assert!(
            w.g.ent[p].act_life >= 8 || count(&w, 5, 10) == 0,
            "the immortal clamp held under fire: {}",
            w.g.ent[p].act_life
        );
        // Run the script out: the summon cycle comes back around to
        // the state-2/3 charge volley on its own clock, and state 3
        // at life 8 (< 10) is the death trigger: 3 → 12 → 13(32) →
        // 14(32) → 15(60) → the apocalypse.
        for _ in 0..3000 {
            if count(&w, 5, 10) == 0 {
                break;
            }
            w.tick(near, PlayerCommand::default());
        }
        assert_eq!(count(&w, 5, 10), 0, "the pyramid removed itself");
        assert_eq!(count(&w, 5, 1), 0, "the extinction killed the goat");
        assert!(
            count(&w, 10, 9) > 0 || w.mc2_apocalypse,
            "the apocalypse dome spawned with the latch"
        );
        assert!(w.mc2_apocalypse, "the extinction latch is set");
    }

    /// The pyramid's summons spawn into StageVar2 17 (`sub_1E320`
    /// spin-up) → 16 (`sub_1E580` home) — the two `sub_1D5D0` cases
    /// (without them the summons freeze, unkillable, into a "barrier").
    /// The chain: spin-up decelerates 320 → the per-model cruise
    /// (m21 = 96) and drops to slot 16; slot 16 MOVES the creature and
    /// takes damage (a kill leaves the corpse standing at f46 = 1,
    /// EF:10864-67); the pyramid's death expires every summon with a
    /// fire puff.
    #[test]
    fn mc2_pyramid_summons_release_fight_and_expire() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        let things = vec![th(1, 5, 10, 100, 100)];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.set_mc2_doom_level(true);
        w.tick(away(), PlayerCommand::default());
        let p = find_slot(&w, 5, 10);
        // Hand-plant a summoned devil exactly as the case-3..6 exec
        // stamps it (doomsday.rs summon block).
        let (sx, sy) = mc2_pos(104, 100);
        let gz = w.g.ground_z(sx, sy) as i16;
        let s = w.g.mc2_spawn_m21(sx, sy, gz + 768).expect("summon");
        {
            let own = w.g.ent[p].id24;
            let e = &mut w.g.ent[s];
            e.f146 = PLAYER_TARGET;
            e.id24 = own;
            e.site_z = 17;
            e.f46 = 250;
            e.f126 = 320;
            e.tick70 = 175;
        }
        // Spin-up: ~38 ticks of deceleration, then the m21 cruise 96
        // and the drop to the StageVar2-16 homing slot.
        let mut released = false;
        for _ in 0..60 {
            w.tick(away(), PlayerCommand::default());
            if w.g.ent[s].site_z == 16 {
                released = true;
                break;
            }
        }
        assert!(released, "the spin-up drops to the homing slot");
        assert_eq!(w.g.ent[s].f126, 96, "the m21 cruise speed took");
        // The homing slot MOVES the devil toward its target (the
        // far-away player) — the frozen-barrier regression guard.
        let (x0, y0) = (w.g.ent[s].x, w.g.ent[s].y);
        for _ in 0..30 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(
            (w.g.ent[s].x, w.g.ent[s].y) != (x0, y0) || w.g.ent[s].tick70 != 175,
            "the released summon moves (not parked)"
        );
        // Killable: the mailbox drains through the slot's intake; the
        // kill leaves the corpse standing at f46 = 1 until the
        // pyramid dies (retail's EF:10864-67 law).
        w.g.mail_write(MailTarget::Pool(s), 0, 60000, PLAYER_TARGET);
        for _ in 0..3 {
            w.tick(away(), PlayerCommand::default());
        }
        if w.g.ent[s].site_z == 16 && w.g.ent[s].tick70 == 175 {
            assert!(w.g.ent[s].act_life < 0, "the summon took the kill");
            assert_eq!(w.g.ent[s].f46, 1, "the corpse stands at f46 = 1");
            // The pyramid's death expires it with a fire puff.
            w.g.ent[p].flags |= 0x400;
            for _ in 0..3 {
                w.tick(away(), PlayerCommand::default());
            }
            assert!(
                w.g.ent[s].flags & 0x400 != 0,
                "the parent's death expired the summon"
            );
        } else {
            // The devil had already handed off to its model machine
            // (+2/+6) — the normal death path owns it there.
            assert!(
                w.g.ent[s].act_life < 0 || w.g.ent[s].flags & 0x400 != 0,
                "the summon died through its model machine"
            );
        }
    }

    /// The doomsday checkpoints act on `dword_38523` — the SPHERE
    /// family (10, 39/40/57) — plus the class-5 creature buckets
    /// (`KillAllCreatures_1B5F0`), NOT the whole pool: retail's v29==3
    /// wipe is `DisableEntityDrawing` over that list only
    /// (EF:13048-66) and case 0xE's 140-life reset walks the same list
    /// (EF:12847-54). Castles and other non-sphere entities must
    /// survive.
    #[test]
    fn mc2_doomsday_checkpoint_spares_the_world() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        // A pyramid, a goat (kill-all's legitimate victim), and two
        // witnesses crafted below: a castle and a mana sphere.
        let things = vec![th(1, 5, 10, 100, 100), th(2, 5, 1, 90, 90)];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.set_mc2_doom_level(true);
        w.tick(away(), PlayerCommand::default());
        let p = find_slot(&w, 5, 10);
        let (x, y) = mc2_pos(110, 110);
        let gz = w.g.ground_z(x, y) as i16;
        let craft = |w: &mut World, class: u8, model: u8| -> usize {
            let j = w.g.mc2_spawn_fire(x, y, gz).expect("slot");
            let e = &mut w.g.ent[j];
            e.class64 = class;
            e.model65 = model;
            e.tick70 = 0;
            e.max_life = 2000;
            e.act_life = 2000;
            j
        };
        let castle = craft(&mut w, 3, 2);
        let sphere = craft(&mut w, 10, 39);
        // Jump the attack driver (states 0/1 run it) to the kill-all
        // countdown's final checkpoint (bits & 4, v7 == 1).
        w.g.ent[p].f71 = 1;
        w.g.ent[p].f44 = 4;
        w.g.ent[p].f26 = 1;
        w.tick(away(), PlayerCommand::default());
        assert!(
            w.g.ent[sphere].flags & 0x400 != 0,
            "the sphere family IS despawned at checkpoint 1"
        );
        assert!(
            w.g.ent[castle].flags & 0x400 == 0,
            "the castle survives the activation crater"
        );
        assert_eq!(
            w.g.ent[castle].max_life, 2000,
            "no 140-life reset outside the sphere family"
        );
        assert!(
            w.g.ent[find_slot(&w, 5, 1)].act_life < 0 || count(&w, 5, 1) == 0,
            "the creature mass-kill still lands (goat dead)"
        );
    }

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
        use crate::mc1::combat::MailTarget;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        // The house persists past construction at runtime.
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
        let rubble =
            (108u8..=113).any(|x| (108u8..=113).any(|y| w.planes().angle[tile(x, y)] & 7 == 1));
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
        assert!(
            stored >= 512,
            "balloon delivered the cargo (stored {stored})"
        );
        // One more tick: the census (tick-start) sees the delivery.
        w.tick(away(), PlayerCommand::default());
        // Castle-stored mana raises the wizard ceiling and counts as
        // banked (sub_48230).
        assert!(
            w.loadout().mana_max >= 1000 + 512,
            "ceiling includes the store"
        );
        assert!(w.loadout().banked >= 512, "banked = castle stored");
        // With no pickups left, the dispatcher's default target is
        // the CASTLE (:56376): the balloon comes home and hovers
        // there instead of parking at the last pickup.
        for _ in 0..600 {
            w.tick(away(), PlayerCommand::default());
        }
        let bal = (1..w.g.ent.len())
            .find(|&j| {
                w.g.ent[j].class64 == 3 && w.g.ent[j].model65 == 3 && w.g.ent[j].flags & 0x400 == 0
            })
            .expect("the fleet balloon lives");
        assert_eq!(w.g.ent[bal].f146, c as u16, "homes the castle when idle");
        let (cx, cy) = (w.g.ent[c].x, w.g.ent[c].y);
        let d = crate::engine::features::Gen::dist2_sq(w.g.ent[bal].x, w.g.ent[bal].y, cx, cy);
        assert!(
            crate::engine::features::Gen::isqrt(d as u32) < 4 * 256,
            "hovers the castle neighborhood, not the pickup spot"
        );
    }

    #[test]
    fn castle_upgrade_costs_the_full_ladder_amount() {
        use crate::mc1::spells::SpellId;
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
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        w.tick(away(), fire);
        assert_eq!(
            count(&w, 9, 10),
            0,
            "pool 1000 cannot fund the 10000 upgrade"
        );
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
    fn active_spell_burst_blocks_mana_regen() {
        // docs/spell-audit/mana-regen.md (general note 2): while a
        // spell burst is live, the caster's regen accumulator is
        // pinned to 0 (`sub_55E80`/`sub_68DE0` else branch) so an
        // active spell suppresses mana regeneration.
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        let m = w.grant_spell(SpellId(0)).unwrap(); // Fireball (cost 200, burst 5)
        w.player.left = Some(SpellId(0));
        // Afield (see `away()`, far from any castle) → slow-regen
        // floor 100/tick, with headroom below the 1000 ceiling.
        w.player.mana = 500;
        // Fire once → arms the burst (f26 = count) and stamps the
        // negative cast debit.
        w.tick(
            away(),
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        assert!(w.g.ent[m].f26 > 0, "a burst is live right after firing");
        // Tick through the rest of the burst WITHOUT firing: mana must
        // never climb while the burst counter is live (regen pinned).
        let mut prev = w.player.mana;
        while w.g.ent[m].f26 > 0 {
            w.tick(away(), PlayerCommand::default());
            assert!(
                w.player.mana <= prev,
                "no regen while a burst is live (was {prev}, now {})",
                w.player.mana
            );
            prev = w.player.mana;
        }
        // Once the burst expires, idle regen resumes.
        let idle_start = w.player.mana;
        for _ in 0..5 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(
            w.player.mana > idle_start,
            "regen resumes once no burst is live (stuck at {idle_start})"
        );
    }

    #[test]
    fn trees_burn_to_char_and_spark_a_standing_fire() {
        use crate::mc1::combat::MailTarget;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        // the ground plane.
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
        let husk = w.debug_pool().1.into_iter().find(|e| e.class == 2).unwrap();
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
            ceiling: Vec::new(),
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
            w.tick(
                firing_line(),
                PlayerCommand {
                    fire_left: true,
                    ..Default::default()
                },
            );
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
            ceiling: Vec::new(),
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
        // A fresh world's book is EMPTY (the retail human grant is
        // availability ∩ collected — no campaign store, no spells).
        let lv = w.loadout();
        assert!(!lv.owned.iter().any(|&o| o), "fresh book starts empty");
        assert_eq!((lv.left, lv.right), (None, None));
        w.grant_spells(&[0, 3]);
        let lv = w.loadout();
        assert!(lv.owned[0] && lv.owned[3], "granted");
        assert_eq!(
            (lv.left, lv.right),
            (Some(0), Some(3)),
            "auto-fill L/R (:49246-54)"
        );
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
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
            ceiling: Vec::new(),
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

    /// A resting jar rides its tile's ground — raised terrain must not
    /// bury it (HW level 00 spawned one below the surface) and
    /// destroyed ground must not leave it hovering.
    /// Retail spawns at ground (:44005) and its static z can never
    /// legitimately diverge (jars have no gravity, terrain writes
    /// ignore class 12, :51729); the snap is idempotent so it is
    /// hash-neutral wherever a jar already sits right.
    #[test]
    fn jars_ride_terrain_changes() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 12,
            model: 7,
            x: 200,
            y: 200,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let away = PlayerPose::level(10 << 8, 10 << 8, 3260, 0);
        w.tick(away, PlayerCommand::default());
        let jar = (1..w.g.ent.len())
            .find(|&i| w.g.ent[i].class64 == 12 && w.g.ent[i].flags & 0x400 == 0)
            .expect("the placed jar exists");
        let (x, y) = (w.g.ent[jar].x, w.g.ent[jar].y);
        assert_eq!(
            w.g.ent[jar].z,
            w.g.ground_z(x, y) as i16,
            "spawns on ground"
        );

        // Raise the ground under it (the HW burial shape): the jar
        // must ride up, not stay buried.
        let (tx, ty) = ((x >> 8) as usize, (y >> 8) as usize);
        for dy in 0..2 {
            for dx in 0..2 {
                w.g.t.height[(ty + dy) % 256 * 256 + (tx + dx) % 256] = 140;
            }
        }
        w.tick(away, PlayerCommand::default());
        assert_eq!(
            w.g.ent[jar].z,
            w.g.ground_z(x, y) as i16,
            "raised ground lifts the jar"
        );

        // Destroy the ground: the jar settles down instead of hovering.
        for dy in 0..2 {
            for dx in 0..2 {
                w.g.t.height[(ty + dy) % 256 * 256 + (tx + dx) % 256] = 40;
            }
        }
        w.tick(away, PlayerCommand::default());
        assert_eq!(
            w.g.ent[jar].z,
            w.g.ground_z(x, y) as i16,
            "lowered ground drops the jar"
        );
    }

    /// With `prune_owned_jars` on, a placed jar whose spell the player
    /// already owns self-culls — retail leaves such uncollectable jars
    /// in the world forever. Faithful default (off) keeps them.
    #[test]
    fn owned_spell_jars_are_pruned_when_enabled() {
        use crate::mc1::spells::SpellId;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // A placed class-12 jar carrying spell 7, far from the carpet.
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 12,
            model: 7,
            x: 200,
            y: 200,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let jars = |w: &World| {
            w.debug_pool()
                .1
                .into_iter()
                .filter(|e| e.class == 12 && e.state < MANIFEST_BASE && e.flags & 0x400 == 0)
                .count()
        };

        // Faithful default: the jar of an already-owned spell stays.
        let mut keep = World::new(planes.clone(), &things, 1, assets());
        keep.grant_spell(SpellId(7));
        let away = PlayerPose::level(10 << 8, 10 << 8, 3260, 0);
        for _ in 0..8 {
            keep.tick(away, PlayerCommand::default());
        }
        assert_eq!(
            jars(&keep),
            1,
            "off by default: the uncollectable jar remains"
        );

        // With the improvement on, the same jar is removed.
        let mut prune = World::new(planes, &things, 1, assets());
        prune.set_prune_owned_jars(true);
        prune.grant_spell(SpellId(7));
        assert_eq!(jars(&prune), 1, "jar present before it ticks");
        for _ in 0..8 {
            prune.tick(away, PlayerCommand::default());
        }
        assert_eq!(jars(&prune), 0, "the owned-spell jar self-culls");
    }

    #[test]
    fn accelerate_directions_are_mutually_exclusive() {
        use crate::mc1::spells::SpellId;
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
        let fwd = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        w.tick(away(), fwd);
        assert_eq!(w.accel_override(), Some(3.0), "held = 3.0 (:65169)");
        w.tick(away(), fwd);
        assert_eq!(w.accel_override(), Some(3.0), "still held = still 3.0");
        // Released: the channel keeps propelling at 2.0.
        w.tick(away(), PlayerCommand::default());
        assert_eq!(w.accel_override(), Some(2.0), "released = 2.0 channel");
        // Opposite activation force-clears forward (:55871/:55914).
        let back = PlayerCommand {
            fire_right: true,
            ..Default::default()
        };
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
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(15));
        // Hold = continuous stream (manual), paced by count 2: the
        // one-tick beams resolve immediately into player shots.
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        for _ in 0..10 {
            w.tick(firing_line(), fire);
        }
        let (_, shots, _) = w.combat_stats();
        assert!(shots >= 4, "held bolt streams (got {shots})");
    }

    #[test]
    fn earthquake_trench_travels_forward() {
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(6));
        // Fire north from the firing line: the lob impacts a few
        // tiles ahead, then the walker digs onward tile by tile.
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(7));
        // Aim steeply down so the bolt grounds fast.
        let mut p = firing_line();
        p.pitch = 0x100;
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        let mut saw_ring = false;
        for _ in 0..40 {
            w.tick(p, PlayerCommand::default());
            saw_ring |= count(&w, 10, 17) > 0;
        }
        assert!(saw_ring, "meteor impact = the growing fire-ring blast");
    }

    #[test]
    fn volcano_erupts_periodically_after_the_cone() {
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(8));
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
        assert!(
            saw_lava,
            "the eruption window launches ballistic lava bombs"
        );
        assert!(saw_plume, "the eruption start raises the (10,19) plume");
        // FINITE: the window is over — no live bombs remain hundreds
        // of ticks past it (bomb life caps at 199).
        assert_eq!(count(&w, 10, 16), 0, "eruption activity ended");
    }

    #[test]
    fn possess_homes_on_and_claims_a_mana_ball() {
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(3));
        let p = firing_line();
        // A loose ball ~6 tiles dead ahead (heading 0 = -y) on the
        // aim line, at ground level.
        let (bx, by) = ((112u16 << 8) + 128, (110u16 << 8) + 128);
        let gz = w.g.ground_z(bx, by) as i16;
        let b = w.g.spawn_mana_ball(bx, by, gz).unwrap();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        assert_eq!(count(&w, 9, 1), 1, "the possess lob launched");
        let mut claimed = false;
        for _ in 0..120 {
            w.tick(p, PlayerCommand::default());
            claimed |= w.g.ent[b].f144 == PLAYER_TARGET;
        }
        assert!(
            claimed,
            "the m1 lob acquires + the (10,12) flash claims the ball"
        );
    }

    #[test]
    fn possess_claims_a_neutral_house() {
        use crate::mc1::spells::SpellId;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(18));
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(20));
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
    fn global_death_fuses_at_the_caster_into_the_flat_plane_field() {
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(22));
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        assert_eq!(count(&w, 9, 18), 1, "the (9,18) death fuse armed");
        // Charges STACK: a release + re-press primes a second
        // independent fuse.
        w.tick(p, PlayerCommand::default());
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        assert_eq!(count(&w, 9, 18), 2, "overlapping charges both live");

        // The fuse rides the caster ~21 ticks, then the (10,55)
        // field detonates AT the caster (never a downrange bolt).
        let mut field = None;
        for _ in 0..40 {
            w.tick(p, PlayerCommand::default());
            if field.is_none() {
                field = (1..w.g.ent.len()).find(|&j| {
                    w.g.ent[j].class64 == 10
                        && w.g.ent[j].model65 == 55
                        && w.g.ent[j].flags & 0x400 == 0
                });
                if field.is_some() {
                    break;
                }
            }
        }
        let f = field.expect("the fuse raised the (10,55) death field");
        let d = crate::engine::features::Gen::isqrt(crate::engine::features::Gen::dist2_sq(
            w.g.ent[f].x,
            w.g.ent[f].y,
            p.x,
            p.y,
        ) as u32);
        assert!(
            (d as i32) < 3 * 256,
            "the field detonated around the caster, not downrange (d {d})"
        );
        assert_eq!(
            w.g.ent[f].f44, 7000,
            "the detonation copied the spell's damage"
        );

        // The kill cylinder is 2D (sub_423D0 is x/y only): a creature
        // FAR ABOVE inside the 10-tile radius dies; one 15 tiles to
        // the side survives. No terrain is touched.
        let (fx, fy, fz) = (w.g.ent[f].x, w.g.ent[f].y, w.g.ent[f].z);
        let above = w.g.spawn_creature(2, fx, fy, fz + 12000).unwrap();
        let aside =
            w.g.spawn_creature(2, fx.wrapping_add(15 << 8), fy, fz + 200)
                .unwrap();
        let ground_before = {
            let g = &w.g;
            g.ground_z(fx, fy)
        };
        // Ride out the 32-tick priming tick-tock + the sweep.
        let mut flashed = false;
        for _ in 0..40 {
            w.tick(p, PlayerCommand::default());
            // The detonation's only sighting: sub_44BE0(owner, 3), the
            // violet full-screen wash (the player owns this field).
            flashed |= w.vitals().pal_flash.0 == 3;
        }
        assert!(flashed, "the detonation armed the row-3 palette flash");
        assert!(
            w.g.ent[above].class64 != 5 || w.g.ent[above].act_life < 0,
            "the vertical kill cylinder reached the creature far above"
        );
        assert!(
            w.g.ent[aside].class64 == 5 && w.g.ent[aside].act_life > 0,
            "15 tiles out of the cylinder survives"
        );
        assert_eq!(
            w.g.ground_z(fx, fy),
            ground_before,
            "Global Death never modifies terrain"
        );
    }

    #[test]
    fn undead_army_raises_owned_skeletons() {
        use crate::mc1::spells::SpellId;
        let mut w = flat_world();
        w.set_dev_spells(true);
        w.player.left = Some(SpellId(17));
        let p = firing_line();
        w.tick(
            p,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        let mut skeletons = 0usize;
        for _ in 0..80 {
            w.tick(p, PlayerCommand::default());
            skeletons = skeletons.max(count(&w, 5, 9));
        }
        assert_eq!(skeletons, 8, "8 skeletons on the ring");
        for e in w
            .debug_pool()
            .1
            .iter()
            .filter(|e| e.class == 5 && e.model == 9)
        {
            assert_eq!(
                e.id24, PLAYER_TARGET,
                "owner-tagged: never attacks the caster"
            );
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
        w.tick(
            firing_line(),
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
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
        assert!(
            (second.1 - 0.5).abs() < 1e-3,
            "second balloon 100/200 cargo"
        );

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
        assert!(
            w.loadout().balloons.is_empty(),
            "collapsed castle → no roster"
        );
    }

    #[test]
    fn book_bind_gate_is_the_castle_stored_unlock_ladder() {
        // The :26926 gate: bindable iff castle_req == 0 OR the linked
        // castle STORES >= castle_req — never a player-mana test.
        let mut w = flat_world();
        let free = SPELLS
            .iter()
            .position(|s| s.castle_req == 0)
            .expect("a free spell");
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

    #[test]
    fn blue_jar_unrestricts_its_spell_and_survives_death() {
        // BLUE jars (THING data_12 >= 3, :44043-54): the same spell,
        // but the grant leaves the requirement zeroed (:64845) — the
        // spell binds and casts CASTLE-LESS (the maze-level survival
        // loadout). The death scatter banks blue per spell and the
        // respawn re-grant restores it (:55531-35, :54908-12).
        let mut w = bare_creature_world(2);
        w.g.move_relink(1, 30 << 8, 30 << 8, 3200); // creature offstage
        let mut ladder = SPELLS
            .iter()
            .enumerate()
            .filter(|(_, s)| s.castle_req > 0 && s.possess_mana <= 1000);
        let (blue, _) = ladder.next().expect("a ladder spell");
        let (red, _) = ladder.next().expect("a second ladder spell");

        // A blue and a red jar of two ladder spells, picked up
        // castle-less (the THING post-init already set flag + 280).
        let j = w
            .spawn_inert(12, blue as u16, 112 << 8, 110 << 8, 3200)
            .unwrap();
        w.g.ent[j].flags |= BLUE_SPELL;
        w.g.ent[j].type86 = 280;
        w.try_pickup(j);
        let k = w
            .spawn_inert(12, red as u16, 112 << 8, 110 << 8, 3200)
            .unwrap();
        w.try_pickup(k);
        assert_eq!(w.player.owned[blue] as usize, j, "blue jar granted");
        assert_eq!(w.player.owned[red] as usize, k, "red jar granted");

        // No castle: the blue-granted spell binds AND passes the cast
        // gate; the red-granted twin stays locked.
        let l = w.loadout();
        assert!(l.bindable[blue], "blue grant binds castle-less");
        assert!(!l.bindable[red], "red grant still needs the ladder");
        w.player.mana = 2000;
        assert!(
            w.spell_gate(blue, &SPELLS[blue]),
            "blue grant casts castle-less"
        );
        assert!(!w.spell_gate(red, &SPELLS[red]), "red grant gate holds");

        // Death and castle respawn: blue survives the scatter bank.
        let c =
            w.g.spawn_castle((140 << 8) + 128, (140 << 8) + 128)
                .unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.g.ent[c].f144 = PLAYER_TARGET;
        w.player.grace = 0;
        hit_player(&mut w, 30000, 1);
        w.tick(firing_line(), PlayerCommand::default());
        w.tick(grounded_line(), PlayerCommand::default());
        assert_eq!(w.vitals().state, LifeState::Dead);
        w.tick(
            grounded_line(),
            PlayerCommand {
                respawn: true,
                ..Default::default()
            },
        );
        assert_eq!(w.vitals().state, LifeState::Alive);
        let m = w.player.owned[blue] as usize;
        assert_ne!(m, 0, "blue spell re-instantiated on respawn");
        assert_ne!(m, j, "a fresh manifestation, not the scattered jar");
        assert!(
            w.g.ent[m].flags & BLUE_SPELL != 0 && w.g.ent[m].type86 == 280,
            "the re-grant restored the blue marker"
        );
        w.g.ent[c].f140 = 0; // the castle stores nothing
        let l = w.loadout();
        assert!(l.bindable[blue], "blue still binds off an empty castle");
        assert!(!l.bindable[red], "red re-grant is restricted again");
    }

    // ---- hostile wizards (rival AI) ----------------------------------

    fn rival_cfg(book16: bool, castle_level: u8) -> crate::mc1::rivals::RivalConfig {
        let mut book = [false; SPELL_COUNT];
        book[0] = true; // fireball
        book[16] = book16;
        crate::mc1::rivals::RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 255,
            castle_level,
            book,
            allowed: book,
        }
    }

    /// A start-marker THING for player slot 1 (class 3 model 5).
    fn rival_marker_things() -> Vec<Thing> {
        vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 5,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }]
    }

    fn rival_world(book16: bool, castle_level: u8) -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut w = World::new(planes, &rival_marker_things(), 1, assets());
        let mut cfgs: [Option<crate::mc1::rivals::RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(rival_cfg(book16, castle_level));
        w.set_wizards(&cfgs, 2);
        w
    }

    #[test]
    fn rival_spawns_at_marker_with_book_and_starting_castle() {
        let w = rival_world(true, 3);
        assert_eq!(w.rivals.len(), 1);
        let r = &w.rivals[0];
        let e = &w.g.ent[r.ent as usize];
        assert_eq!((e.class64, e.model65), (3, 1));
        // Slot 1 wears the second wizard art row (273).
        assert_eq!(e.type86, 273);
        assert_eq!((e.x >> 8, e.y >> 8), (120, 120));
        assert!(r.owned[0] != 0 && r.owned[16] != 0, "the book minted");
        // The starting castle: level 2 (= tail 3 - 1), full, owned.
        let c = w.rival_castle(r.ent).expect("starting castle");
        assert_eq!(w.g.ent[c].f26, 2);
        assert_eq!(w.g.ent[c].id24, r.ent);
        assert_eq!(w.g.ent[c].f140, Gen::CASTLE_CAP[2].clamp(0, 320_000));
        // The census credits the castle to the rival, not the player.
        let mut w = w;
        w.tick(away(), PlayerCommand::default());
        assert!(w.rivals[0].mana_max > 1000);
        assert_eq!(w.player.mana_max, 1000);
    }

    #[test]
    fn rival_at_war_fires_on_the_player() {
        let mut w = rival_world(false, 0);
        w.rivals[0].war[0] = true;
        w.rivals[0].hate[0] = 60000;
        // Sit the player in range, awake and visible.
        let pose = PlayerPose::from_tiles(120.5, 108.0 / 8.0, 112.0, 0.0, 0.0, 0.0);
        let mut fired = false;
        for _ in 0..400 {
            w.tick(pose, PlayerCommand::default());
            let rid = w.rivals[0].ent;
            if w.g
                .ent
                .iter()
                .any(|e| e.class64 == 9 && e.flags & 0x400 == 0 && e.id24 == rid)
            {
                fired = true;
                break;
            }
        }
        assert!(fired, "the rival never fired on the player");
        assert_eq!(w.rivals[0].state, crate::mc1::rivals::AiState::AttackWizard);
    }

    #[test]
    fn rival_death_scatters_jars_and_castleless_death_eliminates() {
        let mut w = rival_world(false, 0);
        let rid = w.rivals[0].ent as usize;
        // Kill it: a lethal ch0 hit from the player (grace spent).
        w.rivals[0].grace = 0;
        w.g.ent[rid].mail[0] = (60000, PLAYER_TARGET);
        for _ in 0..300 {
            w.tick(away(), PlayerCommand::default());
            if w.rivals[0].eliminated {
                break;
            }
        }
        assert!(w.rivals[0].eliminated, "castle-less death must eliminate");
        // The kill credited to the human.
        assert_eq!(w.player_kill_row()[1], 1);
        // The known book scattered as decaying ground jars + a grave.
        let jars =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 12 && e.tick70 == DROPPED_JAR && e.flags & 0x400 == 0)
                .count();
        assert_eq!(jars, 1, "one owned spell scatters one jar");
        assert!(
            w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 40 && e.flags & 0x400 == 0),
            "the grave spawned"
        );
        // Hidden husk: not in the drawable set.
        assert!(
            !w.live_poses().iter().any(|p| p.class == 3 && p.model == 1),
            "the dead wizard billboard must be hidden"
        );
        let death_slots = w.take_rival_deaths();
        assert_eq!(death_slots, vec![1]);
    }

    #[test]
    fn rival_with_castle_respawns_with_grace() {
        let mut w = rival_world(true, 1);
        let rid = w.rivals[0].ent as usize;
        w.rivals[0].grace = 0;
        w.g.ent[rid].mail[0] = (60000, PLAYER_TARGET);
        let mut respawned = false;
        for _ in 0..2000 {
            w.tick(away(), PlayerCommand::default());
            if w.g.ent[rid].tick70 == 1 && w.g.ent[rid].act_life > 0 {
                respawned = true;
                break;
            }
        }
        assert!(respawned, "the castled rival must respawn");
        assert!(!w.rivals[0].eliminated);
        // Back at the castle.
        let c = w.rival_castle(w.rivals[0].ent).unwrap();
        assert_eq!(w.g.ent[rid].x >> 8, w.g.ent[c].x >> 8);
    }

    #[test]
    fn castleless_rival_scouts_and_plants_a_castle() {
        let mut w = rival_world(true, 0);
        let mut built = None;
        for _ in 0..6000 {
            w.tick(away(), PlayerCommand::default());
            if let Some(c) = w.rival_castle(w.rivals[0].ent) {
                built = Some(c);
                break;
            }
        }
        let c = built.expect("the rival never planted its castle");
        assert_eq!(w.g.ent[c].id24, w.rivals[0].ent);
        // The free initial plant: level 0 shell, terrain stamped.
        assert!(w.g.ent[c].f26 >= 0);
    }

    #[test]
    fn beached_kraken_dies_landlocked_kraken_swims() {
        // All-land world: a spawned kraken (row 18, water-only mask)
        // must die within a few boundary crossings (the :21225-91
        // mover rule — same-tile shortcut is FIRST-candidate-only).
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut w = World::new(planes, &[], 1, assets());
        let k =
            w.g.spawn_creature(6, 120 << 8 | 128, 120 << 8 | 128, 3200)
                .unwrap();
        w.g.ent[k].f58 = 16; // awake
        let mut died = false;
        for _ in 0..200 {
            w.tick(away(), PlayerCommand::default());
            if w.g.ent[k].act_life < 0 || w.g.ent[k].tick70 == 40 {
                died = true;
                break;
            }
        }
        assert!(died, "the beached kraken must die");

        // All-water world: it lives.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![0; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut w = World::new(planes, &[], 1, assets());
        let k =
            w.g.spawn_creature(6, 120 << 8 | 128, 120 << 8 | 128, 3200)
                .unwrap();
        w.g.ent[k].f58 = 16;
        for _ in 0..200 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(
            w.g.ent[k].act_life > 0 && w.g.ent[k].tick70 != 40,
            "the swimming kraken must live"
        );
    }

    #[test]
    fn win_trigger_consumes_the_completion_and_fires_its_disposition() {
        // A state-4 win trigger whose disposition 1 spawns a creature.
        let things = vec![
            Thing {
                slot: 0,
                kind: ThingKind::Entity,
                class: 11,
                model: 4,
                x: 75,
                y: 162,
                dis_id: 0,
                swi_sz: 9,
                swi_id: 1,
                parent: 0,
                child: 0,
                par3: None,
            },
            Thing {
                slot: 1,
                kind: ThingKind::Entity,
                class: 5,
                model: 2,
                x: 112,
                y: 110,
                dis_id: 1,
                swi_sz: 0,
                swi_id: 1,
                parent: 0,
                child: 0,
                par3: None,
            },
        ];
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut w = World::new(planes, &things, 1, assets());
        // No win: the trigger sits armed.
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.live_things().len(), 0);
        // Latch a win with a standing castle: the trigger EATS it.
        let c = w.g.spawn_class3(2, 100 << 8, 100 << 8, 3200).unwrap();
        w.g.ent[c].id24 = PLAYER_TARGET;
        w.completed = true;
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(!w.completed, "the win trigger must consume the win bit");
        let creatures = w.live_things().iter().filter(|t| t.class == 5).count();
        assert_eq!(creatures, 1, "the disposition spawned its stage");
    }

    #[test]
    fn castle_balloons_are_damageable() {
        let mut w = rival_world(true, 3);
        // The castle tick spawns the fleet on its every-other-tick
        // dispatcher.
        let mut balloon = None;
        for _ in 0..64 {
            w.tick(away(), PlayerCommand::default());
            balloon = (1..w.g.ent.len()).find(|&j| {
                let e = &w.g.ent[j];
                e.class64 == 3 && e.model65 == 3 && e.flags & 0x400 == 0
            });
            if balloon.is_some() {
                break;
            }
        }
        let b = balloon.expect("the level-3 castle must field a balloon");
        assert_eq!(w.g.ent[b].f28, 1, "the ch0 vulnerability bit (+28)");
        assert!(w.g.ent[b].flags & 4 != 0, "linked into the cell grid");
        // Docked at the castle the delivery pass heals it to full
        // every tick (authentic: sub_47F90's LABEL_17 order) — hit
        // it IN FLIGHT instead: drag it out of the delivery ring.
        let (bx, by, bz) = {
            let e = &w.g.ent[b];
            (e.x.wrapping_add(2560), e.y.wrapping_add(2560), e.z)
        };
        w.g.move_relink(b, bx, by, bz);
        let before = w.g.ent[b].act_life;
        let f = w.g.spawn_effect(0, bx, by, bz).unwrap();
        w.g.ent[f].id24 = PLAYER_TARGET;
        for _ in 0..3 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(
            w.g.ent[b].act_life < before,
            "the flying balloon must take area damage ({} -> {})",
            before,
            w.g.ent[b].act_life
        );
    }

    /// The crosshair preview (aim_preview) is hand-keyed by the
    /// equipped spell's candidate set and mirrors the acquire cone:
    /// the default grant is Fireball LEFT (creature set) + Possess
    /// RIGHT (balls/houses only). Purity is compiler-guaranteed
    /// (&self); this pins the keying + the ±0x71 cone.
    #[test]
    fn aim_preview_is_hand_keyed_and_cone_gated() {
        let mut w = bare_creature_world(2); // wild lunger at ~(112,110)
        // Fireball LEFT + Possess RIGHT (the auto-fill order — the
        // fresh book is empty).
        w.grant_spells(&[0, 3]);
        let alt = w.g.ent[1].z;
        let pose = move |heading: u16| PlayerPose::level(108 << 8, 110 << 8, alt, heading);
        for _ in 0..5 {
            w.tick(pose(0), PlayerCommand::default());
        }
        // Exactly one compass octant faces the lunger (cone ±0x71 <
        // the 256 octant spacing).
        let hits: Vec<u16> = (0..8u16)
            .map(|k| k * 256)
            .filter(|&h| w.aim_preview(pose(h))[0].is_some())
            .collect();
        assert_eq!(hits.len(), 1, "one heading locks the lunger: {hits:?}");
        // The lock reports the creature's position.
        let l = w.aim_preview(pose(hits[0]))[0].unwrap();
        assert!((l.x - w.g.ent[1].x as f32 / 256.0).abs() < 0.51);
        // The possess hand never locks a creature (its set is
        // balls/houses, and this world has neither).
        for k in 0..8u16 {
            assert!(w.aim_preview(pose(k * 256))[1].is_none());
        }
    }

    // ---- Phase-4.3 MC2 roster probes ----------------------------------

    fn mc2_flat_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        World::new_for_game(planes, &[], 1, assets(), GameId::Mc2)
    }

    fn mc2_pos(tx: u16, ty: u16) -> (u16, u16) {
        ((tx << 8) | 128, (ty << 8) | 128)
    }

    /// The possession projectile (action 18) rides the CLAIM probe
    /// `sub_108B0` — it detonates only on claimable targets (mana
    /// spheres, possessable buildings, worm heads) and flies THROUGH
    /// everything else (un-possessable factory/terrain buildings,
    /// wizards). The whitelist keeps un-possessable factory/terrain
    /// sinks (e.g. level-001's crosses) from eating the possession
    /// shot — not a generic any-solid `victim_scan`.
    #[test]
    fn mc2_possession_claim_probe_whitelists_targets() {
        use crate::engine::features::BldgParam;
        use crate::mc1::combat::MailTarget;

        // Build a world with a possession projectile at tile (100,100)
        // and ONE candidate overlapping it; return the claim-probe hit.
        let setup = |class: u8, model: u8, f71: u8| -> (usize, Option<MailTarget>) {
            let mut w = mc2_flat_world();
            w.g.assets.bldgprm = vec![
                // 0: un-possessable factory / terrain-mod sink.
                BldgParam {
                    rate: 50,
                    flags: 0x08,
                    chain: 0,
                },
                // 1: a possessable building.
                BldgParam {
                    rate: 20,
                    flags: 0x00,
                    chain: 0,
                },
            ];
            let (x, y) = mc2_pos(100, 100);
            let gz = w.g.ground_z(x, y) as i16;
            let p = w.g.new_event().expect("projectile slot");
            {
                let e = &mut w.g.ent[p];
                e.class64 = 9;
                e.model65 = 17;
                e.tick70 = 18;
                e.id24 = crate::mc1::mobs::PLAYER_TARGET;
                e.f80 = 256;
                e.f82 = 256;
                e.f84 = 256;
                e.f78 = 0;
            }
            w.g.link(p, x, y, gz);
            let j = w.g.new_event().expect("candidate slot");
            {
                let e = &mut w.g.ent[j];
                e.class64 = class;
                e.model65 = model;
                e.flags |= 8; // solid
                e.f71 = f71;
                e.id24 = 7; // a foreign owner (≠ PLAYER_TARGET)
                e.f40 = 7;
                e.f80 = 256;
                e.f82 = 256;
                e.f84 = 256;
                e.f78 = 0;
            }
            w.g.link(j, x, y, gz);
            (j, w.g.claim_victim_scan_at(p, (x, y, gz)))
        };

        // Un-possessable factory building (bldgprm flags&8) → passthrough.
        assert!(
            setup(10, 45, 0).1.is_none(),
            "possession flies through the un-possessable sink building"
        );
        // A possessable building → claim it.
        let (j, hit) = setup(10, 45, 1);
        assert_eq!(
            hit,
            Some(MailTarget::Pool(j)),
            "claims a possessable building"
        );
        // Mana spheres (10,39)/(10,40)/(10,57) → detonate.
        for m in [39u8, 40, 57] {
            let (j, hit) = setup(10, m, 0);
            assert_eq!(
                hit,
                Some(MailTarget::Pool(j)),
                "possession detonates on mana sphere model {m}"
            );
        }
        // Worm head (5,22) → detonate.
        let (j, hit) = setup(5, 22, 0);
        assert_eq!(hit, Some(MailTarget::Pool(j)), "detonates on a worm head");
        // A wizard body (class 3) is NOT claimable → passthrough.
        assert!(setup(3, 1, 0).1.is_none(), "flies through a wizard body");
        // A generic creature (class 5, non-worm) → passthrough.
        assert!(
            setup(5, 3, 0).1.is_none(),
            "flies through a non-worm creature"
        );
    }

    /// The (10,54) mana-magnet aura (`sub_38D80`) drags nearby mana
    /// balls toward its eye so they converge and merge — the retail
    /// centre-island magnet — it must grip the balls, not creatures.
    #[test]
    fn mc2_aura_magnetizes_mana_balls() {
        let mut w = mc2_flat_world();
        let (ax, ay) = mc2_pos(100, 100);
        let gz = w.g.ground_z(ax, ay) as i16;
        let aura = w.g.mc2_spawn_aura(ax, ay, gz).expect("aura");
        // Two mana balls ~6 tiles out on opposite sides, inside the
        // ~14-tile magnet range.
        let (b1x, b1y) = mc2_pos(106, 100);
        let (b2x, b2y) = mc2_pos(94, 100);
        let b1 =
            w.g.spawn_mana_ball(b1x, b1y, w.g.ground_z(b1x, b1y) as i16)
                .expect("ball 1");
        let b2 =
            w.g.spawn_mana_ball(b2x, b2y, w.g.ground_z(b2x, b2y) as i16)
                .expect("ball 2");
        let hdist = |w: &World, j: usize| -> f64 {
            let (a, b) = (&w.g.ent[aura], &w.g.ent[j]);
            let dx = (a.x.wrapping_sub(b.x)) as i16 as f64;
            let dy = (a.y.wrapping_sub(b.y)) as i16 as f64;
            (dx * dx + dy * dy).sqrt()
        };
        let (d1_0, d2_0) = (hdist(&w, b1), hdist(&w, b2));
        // Fly the player far away so it never collects the balls.
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        for _ in 0..40 {
            w.tick(far, PlayerCommand::default());
        }
        // Both balls were pulled a long way inward (≥ 2 tiles).
        assert!(
            hdist(&w, b1) < d1_0 - 512.0,
            "ball 1 magnetized inward: {d1_0} -> {}",
            hdist(&w, b1)
        );
        assert!(
            hdist(&w, b2) < d2_0 - 512.0,
            "ball 2 magnetized inward: {d2_0} -> {}",
            hdist(&w, b2)
        );
    }

    /// A CHARGED/repeat fireball impact spawns the (10,76) fire-orb
    /// firestorm — it must not fall through to the misfit arm
    /// (`mc2_proj_impact` needs a (10,76) case, else the charged
    /// fireball degrades to a bare damage write).
    #[test]
    fn mc2_charged_fireball_impact_spawns_firestorm() {
        let mut w = mc2_flat_world();
        w.g.assets.mc2_sprite_ext = crate::mc2::derive_sprite_extents(&[(32, 32); 400]);
        let (x, y) = mc2_pos(100, 100);
        let ground = w.g.ground_z(x, y) as i16;
        // A charged-fireball projectile (subtype 28) aimed at the
        // (10,76) impact, placed below the surface so it detonates.
        let p =
            w.g.mc2_spawn_cast_proj(28, x, y, ground - 200)
                .expect("proj");
        {
            let e = &mut w.g.ent[p];
            e.f68 = 10;
            e.f69 = 76;
            e.id24 = crate::mc1::mobs::PLAYER_TARGET;
        }
        let misfits0 = w.misfits().len();
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        for _ in 0..4 {
            w.tick(far, PlayerCommand::default());
        }
        assert!(count(&w, 10, 76) > 0, "the fire-orb firestorm spawned");
        assert!(count(&w, 10, 77) > 0, "with its satellite ring");
        assert_eq!(
            w.misfits().len(),
            misfits0,
            "no (10,76) misfit was logged: {:?}",
            w.misfits()
        );
    }

    /// The magnet aura must not panic when it pulls a ball whose
    /// bearing rounds to the full-turn wrap: `angle_of` returns 0..=2048
    /// and SIN/COS are len 2048, so the index needs masking (else
    /// `SIN[2048]` panics when the magnet pulls a diagonally-placed
    /// ball).
    #[test]
    fn mc2_aura_pull_survives_full_turn_bearing() {
        let mut w = mc2_flat_world();
        let (ax, ay) = mc2_pos(100, 100);
        let gz = w.g.ground_z(ax, ay) as i16;
        let aura = w.g.mc2_spawn_aura(ax, ay, gz).expect("aura");
        // angle_between(ball, aura) = angle_of(ax-bx, ay-by): place the
        // ball so that offset is (-1, -300), which `angle_of` maps to
        // exactly 2048 (the 2048 - lut(1,300)=2048-0 branch).
        let (bx, by) = (ax.wrapping_add(1), ay.wrapping_add(300));
        assert_eq!(
            crate::engine::features::Gen::angle_between(bx, by, ax, ay),
            2048,
            "the crafted bearing hits the wrap value"
        );
        let ball =
            w.g.spawn_mana_ball(bx, by, w.g.ground_z(bx, by) as i16)
                .expect("ball");
        // The aura tick runs here — must not panic on SIN[2048].
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        w.tick(far, PlayerCommand::default());
        // And the ball got a real (masked, angle-0) pull.
        assert!(
            w.g.ent[ball].dest_x != 0 || w.g.ent[ball].dest_y != 0,
            "the ball was still magnetized"
        );
        let _ = aura;
    }

    /// When two MC2 mana balls merge, the survivor inherits the OWNER
    /// (colour) of the BIGGER contributor — retail `sub_36D50`, not the
    /// survivor's own owner (else a merged ball takes the "last ball
    /// merged" colour).
    #[test]
    fn mc2_mana_merge_takes_bigger_owner() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        // Two coincident balls. Spawn the SMALLER first so it holds the
        // lower slot and ticks first — it becomes the survivor, so the
        // owner it ends with is decided by the merge RULE (bigger wins),
        // not by which ball happened to survive (a survivor-keeps-owner
        // rule would leave it with its own smaller owner).
        let small = w.g.spawn_mana_ball(x, y, gz).expect("small");
        let big = w.g.spawn_mana_ball(x, y, gz).expect("big");
        for &b in &[big, small] {
            // Non-zero collision boxes so the coincident pair overlaps
            // (the flat fixture bakes no sprite dims).
            let e = &mut w.g.ent[b];
            e.f78 = 0;
            e.f80 = 200;
            e.f82 = 200;
            e.f84 = 200;
            e.f46 = 0; // no launch hop
        }
        w.g.ent[big].f140 = 5000;
        w.g.ent[big].f144 = 20; // bigger ball's owner
        w.g.ent[small].f140 = 200;
        w.g.ent[small].f144 = 10; // smaller ball's owner
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        w.tick(far, PlayerCommand::default());
        // Exactly one survives, holding the summed mana and the bigger
        // contributor's owner (20), not the smaller's (10).
        let live: Vec<usize> = [big, small]
            .into_iter()
            .filter(|&j| w.g.ent[j].flags & 0x400 == 0)
            .collect();
        assert_eq!(live.len(), 1, "one ball absorbed the other");
        let s = live[0];
        assert_eq!(w.g.ent[s].f140, 5200, "mana summed");
        assert_eq!(
            w.g.ent[s].f144, 20,
            "the merged ball carries the BIGGER ball's owner, not the last-merged"
        );
    }

    /// A DISPOSITION-spawned (10,54) aura reads its RANGE and LIFE from
    /// the THING's stageTag (`swi_id`) — `sub_4A310`: range = swi_id
    /// tiles (squared), life = 8·swi_id + 16 (floor 128). This is how
    /// level-001's staged magnets (swi_id 33/45/64/31) reach the arm
    /// balls sitting 20-44 tiles out; the `AddAuxiliary` 14-tile/128
    /// ctor default left them stranded.
    #[test]
    fn mc2_aura_range_and_life_from_stagetag() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // A 40-tile magnet at (100,100), disposition-fired.
        let things = vec![Thing {
            slot: 1,
            kind: ThingKind::Entity,
            class: 10,
            model: 54,
            x: 100,
            y: 100,
            dis_id: 5,
            swi_sz: 0,
            swi_id: 40,
            parent: 0,
            child: 0,
            par3: Some(0),
        }];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.debug_fire_disposition(5);
        let aura = (1..w.g.ent.len())
            .find(|&i| w.g.ent[i].class64 == 10 && w.g.ent[i].model65 == 54)
            .expect("aura spawned");
        // stageTag → range (f26) + life (8·40+16 = 336).
        assert_eq!(w.g.ent[aura].f26, 40, "range = stageTag tiles");
        assert_eq!(w.g.ent[aura].max_life, 336, "life = 8·stageTag + 16");
        // A ball 25 tiles out — beyond the 14-tile ctor default, but
        // inside the 40-tile staged reach — is pulled inward.
        let (bx, by) = mc2_pos(125, 100);
        let ball =
            w.g.spawn_mana_ball(bx, by, w.g.ground_z(bx, by) as i16)
                .expect("ball");
        let (ax, ay) = (w.g.ent[aura].x, w.g.ent[aura].y);
        let hdist = |w: &World| -> f64 {
            let b = &w.g.ent[ball];
            let dx = (ax.wrapping_sub(b.x)) as i16 as f64;
            let dy = (ay.wrapping_sub(b.y)) as i16 as f64;
            (dx * dx + dy * dy).sqrt()
        };
        let d0 = hdist(&w);
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0, 0.0, 0.0);
        for _ in 0..40 {
            w.tick(far, PlayerCommand::default());
        }
        assert!(
            hdist(&w) < d0 - 512.0,
            "the 25-tile ball is pulled by the 40-tile magnet: {d0} -> {}",
            hdist(&w)
        );
    }

    /// A stage-HELD devil (phase-7 wait) still runs the jump cycle
    /// (`sub_26470` EF:16938-61 — 1D5D0 legs then `sub_265A0` for
    /// hold kinds 1-10): it SETTLES to the terrain instead of
    /// floating at whatever altitude the held walk last lifted it
    /// to, and it keeps hopping ambient without ever aggroing.
    #[test]
    fn mc2_held_devil_settles_and_hops() {
        let mut w = mc2_flat_world();
        let (dx, dy) = mc2_pos(100, 100);
        let gz = w.g.ground_z(dx, dy) as i16;
        // Spawn 800 units up — the float a high plateau leaves behind.
        let d = w.g.mc2_spawn_m21(dx, dy, gz + 800).expect("devil");
        {
            let e = &mut w.g.ent[d];
            e.tick70 = 175; // 8·21 + 7 — the phase-7 stage wait
            e.site_z = 6; // a timer-gate hold kind (jump-eligible 1-10)
        }
        let pose = PlayerPose::from_tiles(102.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let (mut touched, mut hopped) = (false, false);
        for _ in 0..200 {
            w.tick(pose, PlayerCommand::default());
            let (ex, ey, ez) = {
                let e = &w.g.ent[d];
                (e.x, e.y, e.z)
            };
            let g = w.g.ground_z(ex, ey) as i16;
            if ez <= g {
                touched = true;
            }
            if touched && ez > g + 100 {
                hopped = true;
            }
            assert_eq!(w.g.ent[d].tick70, 175, "stays held — no aggro");
        }
        assert!(touched, "the held devil settled out of the float");
        assert!(hopped, "the held devil keeps hopping while held");
    }

    /// A stage-HELD dragon (m0, phase-7 wait) still runs the
    /// vertical bob (`sub_1F300`'s phase-7 wrapper calls `sub_1F040`
    /// for hold kinds 1-10): the floor bounce (+150 below
    /// terrain+256) launches the ballistic arc straight from a
    /// ground-level spawn. Without it the held dragon hugs the terrain
    /// and flies flat like a ground worm.
    #[test]
    fn mc2_held_dragon_bobs_from_the_ground() {
        let mut w = mc2_flat_world();
        let (dx, dy) = mc2_pos(100, 100);
        let gz = w.g.ground_z(dx, dy) as i16;
        let d = w.g.mc2_spawn_m0(dx, dy, gz).expect("dragon");
        {
            let e = &mut w.g.ent[d];
            e.tick70 = 7; // 8·0 + 7 — the phase-7 stage wait
            e.site_z = 6; // a timer-gate hold kind (bob-eligible 1-10)
            e.f58 = 64;
        }
        let pose = PlayerPose::from_tiles(102.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let mut apex = 0i32;
        for _ in 0..120 {
            w.tick(pose, PlayerCommand::default());
            let (ex, ey, ez) = {
                let e = &w.g.ent[d];
                (e.x, e.y, e.z)
            };
            let g = w.g.ground_z(ex, ey) as i16;
            apex = apex.max(ez as i32 - g as i32);
            assert_eq!(w.g.ent[d].tick70, 7, "stays held — no aggro");
        }
        assert!(
            apex > 1000,
            "the held dragon arcs high off the spawn ({apex})"
        );
    }

    /// The m22 mana-worm CASTLE DEPOSIT chain (`sub_26AA0` EF:17313
    /// → `sub_26BD0`): a designated worm banks to the owner's
    /// castle, arms the 128-tick deposit inside 256 units, shrinks,
    /// and dumps its mana capped at the castle's maximum. The
    /// proximity gate is retail's `EuclideanDistXYZ_58490` — 2-D
    /// DESPITE THE NAME (Maths:738-42 never reads z): the head
    /// cruises at chain-ground +384, so a 3-D distance would never
    /// pass and the worm would hover at the flag forever.
    #[test]
    fn mc2_worm_deposits_into_the_castle() {
        let mut w = mc2_flat_world();
        let (cx, cy) = mc2_pos(100, 100);
        let gz = w.g.ground_z(cx, cy) as i16;
        let castle = w.g.new_event().expect("castle");
        {
            let e = &mut w.g.ent[castle];
            e.class64 = 3;
            e.model65 = 2;
            e.id24 = PLAYER_TARGET;
            e.f136 = 10_000; // maxMana
            e.f140 = 0; // stored mana
        }
        w.g.link(castle, cx, cy, gz);
        let worm = w.g.mc2_spawn_m22(cx, cy, gz, 6).expect("worm");
        let mana = w.g.ent[worm].f140;
        assert!(mana > 0, "the worm carries mana");
        w.g.ent[worm].dest_x = PLAYER_TARGET; // designated at the human
        w.g.ent[worm].tick70 = 178; // 0xB2 castle acquire
        // Parked in awake range, away from the flag.
        let pose = PlayerPose::from_tiles(104.5, 105.0 / 8.0, 104.5, 0.0, 0.0, 0.0);
        let (mut armed, mut consumed) = (false, false);
        for _ in 0..1000 {
            w.tick(pose, PlayerCommand::default());
            let e = &w.g.ent[worm];
            if e.tick70 == 179 {
                armed = true; // the 2-D gate passed → deposit state
            }
            if e.flags & 0x400 != 0 || e.class64 != 5 {
                consumed = true;
                break;
            }
        }
        assert!(armed, "the worm armed the deposit at the flag");
        assert!(consumed, "the head consumed itself after the dump");
        assert_eq!(
            w.g.ent[castle].f140, mana,
            "the castle absorbed the worm's mana"
        );
    }

    /// The m26 wraith SPELL-STEAL round trip (`sub_69300` EF:55792 +
    /// `sub_59DC0` EF:41199 + the `sub_68FF0` hand-hint re-pickup):
    /// the equipped jar is yanked (book unlearned, hand emptied,
    /// hint = the robbed hand), detaches off the player, homes to
    /// the wraith, drops to the ground-pickup state, and re-pickup
    /// restores the SAME jar to the SAME hand with the 64-tick
    /// re-steal lock re-armed. The lock also blocks a fresh steal
    /// (retail checks it INSIDE the effect, after the %63 roll).
    #[test]
    fn mc2_wraith_spell_steal_round_trip() {
        let mut w = mc2_flat_world();
        // Learn spell 0 via the dev grant, bound to the RIGHT hand
        // only (the grant's quick-slot law also takes the free LEFT
        // — unbind it, or the steal exercises the both-hands edge
        // where the left-hand hint wins).
        w.set_dev_spells(true);
        w.mc2_select_spell(0, 0, 1);
        w.mc2_select_spell(26, 0, 0);
        let jar = w.mc2_book.ent[0] as usize;
        assert_ne!(jar, 0, "dev grant learned spell 0");
        assert_eq!(w.mc2_book.right, 0, "bound to the right hand");
        assert_eq!(w.mc2_book.left, -1, "left hand unbound");
        let pose = PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0, 0.0, 0.0);
        let (wx, wy) = mc2_pos(104, 100);
        let gz = w.g.ground_z(wx, wy) as i16;
        let m26 = w.g.mc2_spawn_m26(wx, wy, gz).expect("wraith");

        // The fresh adopt armed the 64-tick lock — a steal no-ops.
        assert_eq!(w.g.ent[jar].f54, 64, "adopt armed the re-steal lock");
        w.mc2_spell_steal(m26 as u16, 1);
        assert_eq!(w.mc2_book.right, 0, "locked jar is not stolen");

        // Lock expired → the steal lands, via the pool-side mail.
        w.g.ent[jar].f54 = 0;
        w.g.mc2_steal_mail.0.push((m26 as u16, 1));
        w.tick(pose, PlayerCommand::default());
        assert_eq!(w.mc2_book.right, -1, "right hand emptied");
        assert_eq!(w.mc2_book.ent[0], 0, "spell unlearned while stolen");
        assert_eq!(w.g.ent[jar].tick70, 78, "jar in the detach action");
        assert_eq!(w.g.ent[jar].f36, 1, "hand hint = right");
        assert_eq!(w.g.mc2_spell_tokens.0 & 1, 0, "grant mask cleared");

        // Detach off the player, home to the wraith, drop.
        let mut landed = false;
        for _ in 0..48 {
            w.tick(pose, PlayerCommand::default());
            if w.g.ent[jar].tick70 == 1 {
                landed = true;
                break;
            }
        }
        assert!(landed, "the jar dropped into the ground-pickup state");
        assert_eq!(w.g.ent[jar].f38, 0, "wraith ref dropped on landing");

        // Park ON the jar: re-pickup restores the SAME jar to the
        // SAME hand and re-arms the lock.
        let (jx, jy) = (w.g.ent[jar].x, w.g.ent[jar].y);
        let jpose = PlayerPose::from_tiles(
            jx as f32 / 256.0,
            105.0 / 8.0,
            jy as f32 / 256.0,
            0.0,
            0.0,
            0.0,
        );
        for _ in 0..8 {
            w.tick(jpose, PlayerCommand::default());
            if w.mc2_book.ent[0] != 0 {
                break;
            }
        }
        assert_eq!(w.mc2_book.ent[0] as usize, jar, "re-learned the SAME jar");
        assert_eq!(w.mc2_book.right, 0, "the hint re-equipped the RIGHT hand");
        assert_eq!(w.mc2_book.left, -1, "the left hand untouched");
        assert_eq!(w.g.ent[jar].f36, 0, "hint consumed");
        assert_eq!(w.g.ent[jar].f54, 64, "re-steal lock re-armed");
    }

    /// The MC2 rival carpet carries `byte_0x38_56 = 29` (the wizard
    /// vulnerability mask). Without it f28 stays 0 and `area_write`'s
    /// per-channel gate drops every hit at the mailbox, so a fireball
    /// detonates on the rival but deals nothing.
    #[test]
    fn mc2_rival_wizard_carries_the_vulnerability_mask() {
        use crate::mc2::rivals::Mc2RivalConfig;
        let mut w = mc2_flat_world();
        let mut cfg: [Option<Mc2RivalConfig>; 8] = Default::default();
        cfg[1] = Some(Mc2RivalConfig {
            aggression: 128,
            perception: 128,
            reflexes: 128,
            life: 0,
            castle_level: 0,
            start: [false; 26],
            start_level: [0; 26],
            blocked: [false; 26],
        });
        w.set_mc2_wizards(&cfg, 2);
        let rival = (1..w.g.ent.len())
            .find(|&j| w.g.ent[j].class64 == 3 && w.g.ent[j].model65 == 1)
            .expect("rival spawned");
        assert_eq!(
            w.g.ent[rival].f28, 29,
            "the rival carpet carries the ch0 vulnerability mask"
        );

        // End-to-end: a fireball's ground fire delivers ch0 area damage
        // that `area_write` now ADMITS (f28 & 1 != 0) into the mailbox.
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.move_relink(rival, x, y, gz);
        let fire = w.g.mc2_spawn_fire(x, y, gz).expect("fire");
        w.g.ent[fire].id24 = crate::mc1::mobs::PLAYER_TARGET; // foreign owner
        let ctx = MobCtx {
            px: 0,
            py: 0,
            pz: 0,
            pyaw: 0,
            pmana: 0,
        };
        w.g.area_write(fire, 0, 400, &ctx, false, false);
        assert!(
            w.g.ent[rival].mail[0].1 != 0,
            "the fire's ch0 area damage reached the rival's mailbox"
        );
    }

    fn one_rival_world(start_spell: usize) -> World {
        use crate::mc2::rivals::Mc2RivalConfig;
        let mut w = mc2_flat_world();
        let mut cfg: [Option<Mc2RivalConfig>; 8] = Default::default();
        let mut start = [false; 26];
        start[start_spell] = true;
        cfg[1] = Some(Mc2RivalConfig {
            aggression: 128,
            perception: 128,
            reflexes: 128,
            life: 0,
            castle_level: 0,
            start,
            start_level: [0; 26],
            blocked: [false; 26],
        });
        w.set_mc2_wizards(&cfg, 2);
        w
    }

    /// Every manifestation's armed window (`word_0x2E_46`) is a LIVE
    /// countdown in retail — the class-15 entity's own action counts
    /// it down and the readiness gates (EF:6997/7014/7065) rely on it
    /// expiring. If only the buff set counts down, one cast of possess
    /// or any homing spell locks that spell for the rival's whole life.
    #[test]
    fn mc2_rival_armed_window_expires_for_homing_spells() {
        let mut w = one_rival_world(1); // possess — in the homing set
        let m = w.mc2_rivals[0].book.ent[1] as usize;
        assert!(m != 0, "the possess manifestation exists");
        // Arm the window exactly as a landed cast does.
        w.g.ent[m].f26 = (w.g.ent[m].f28 as i16).max(1);
        let window = w.g.ent[m].f26;
        assert!(window > 0);
        for _ in 0..(window as usize + 1) {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(
            w.g.ent[m].f26, 0,
            "the homing armed window counts down to 0 (was pinned forever)"
        );
    }

    /// `recompute_mana` grows the MC2 rival's `mana_max` from claimed
    /// entities exactly as it does the MC1 vec (retail
    /// `maxMana_0x8C_140`, sub_13CE0 EF:6135). Left uncredited it
    /// pins at the intrinsic 1000 — no castle past rung 1 and the
    /// expensive-spell ceiling gate shut forever.
    #[test]
    fn mc2_rival_mana_ceiling_grows_with_claims() {
        let mut w = one_rival_world(1);
        let rival = w.mc2_rivals[0].ent;
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        // A mana ball claimed by the rival (+144 = the claim owner).
        let ball = w.g.mc2_spawn_fire(x, y, gz).expect("slot");
        {
            let e = &mut w.g.ent[ball];
            e.class64 = 10;
            e.model65 = 39;
            e.max_life = 0;
            e.act_life = 1;
            e.tick70 = 0;
            e.f140 = 500;
            e.f144 = rival;
        }
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            w.mc2_rivals[0].mana_max, 1500,
            "the census credits the MC2 rival's ceiling (1000 base + 500 claimed)"
        );
    }

    /// Synthetic SPELLS.DAT rows: 3 tiers with the given per-tier
    /// costs, armed window 6 (word_0x18), no ceiling gate.
    fn mc2_synth_spell_rows(rows: &[(usize, [i32; 3])]) -> Vec<crate::mc2::spells::Mc2SpellRow> {
        let mut spells =
            vec![crate::mc2::spells::Mc2SpellRow::default(); crate::mc2::spells::MC2_SPELL_ROWS];
        for &(s, costs) in rows {
            spells[s].byte_0 = 3;
            for t in 0..3 {
                spells[s].tiers[t].mana_cost = costs[t];
                spells[s].tiers[t].word_0x18 = 6;
            }
        }
        spells
    }

    /// A flat MC2 world with the synthetic spell table and rivals
    /// granted `(spell, authored level)` per config slot.
    fn mc2_brain_world(grants: &[&[(usize, u8)]], rows: &[(usize, [i32; 3])]) -> World {
        use crate::mc2::rivals::Mc2RivalConfig;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut a = assets();
        a.spells = mc2_synth_spell_rows(rows);
        let mut w = World::new_for_game(planes, &[], 1, a, GameId::Mc2);
        let mut cfg: [Option<Mc2RivalConfig>; 8] = Default::default();
        for (slot, gs) in grants.iter().enumerate() {
            let mut start = [false; 26];
            let mut start_level = [0u8; 26];
            for &(s, lvl) in gs.iter() {
                start[s] = true;
                start_level[s] = lvl;
            }
            cfg[slot + 1] = Some(Mc2RivalConfig {
                aggression: 128,
                perception: 128,
                reflexes: 128,
                life: 0,
                castle_level: 0,
                start,
                start_level,
                blocked: [false; 26],
            });
        }
        w.set_mc2_wizards(&cfg, grants.len() as u16 + 1);
        w
    }

    /// The attack pick TIER-DOWNS instead of holding: a crater (0x10)
    /// authored at tier 2 with only tier 0 affordable is still picked
    /// — at tier 0, with the manifestation retuned by the walk's
    /// SetSpell side effect. (Returning None would stall ALL attack
    /// casting on one cooling spell.)
    #[test]
    fn mc2_rival_attack_pick_tiers_down_instead_of_waiting() {
        let mut w = mc2_brain_world(&[&[(0x10, 2)]], &[(0x10, [100, 1000, 10_000])]);
        // mana 500: >= maxMana/4 (no poverty), affords tier 0 only.
        w.mc2_rivals[0].mana = 500;
        let pick = w.mc2_rival_attack_pick(0, true);
        assert_eq!(pick, Some(0x10), "the walk lands on the affordable tier");
        let m = w.mc2_rivals[0].book.ent[0x10] as usize;
        assert_eq!(
            w.g.ent[m].f71, 0,
            "the winning probe left the manifestation retuned to tier 0"
        );
        assert_eq!(
            w.g.ent[m].max_life, 100,
            "the tier-0 cost stamp rode the retune"
        );
    }

    /// A homing-family spell casts, cools down, and casts AGAIN —
    /// the end-to-end cast-twice pin (one possess must not lock the
    /// spell for the rival's life).
    #[test]
    fn mc2_rival_casts_the_same_spell_twice() {
        let mut w = mc2_brain_world(&[&[(1, 0)]], &[(1, [50, 50, 50])]);
        let i = w.mc2_rivals[0].ent as usize;
        assert!(w.mc2_rival_cast(0, i, 1), "the first possess fires");
        assert!(
            !w.mc2_rival_cast(0, i, 1),
            "immediately after, the armed window + cooldown refuse"
        );
        // Window 6 + AI_RECAST[1] = 10 → a dozen-plus brain ticks
        // clear both.
        for _ in 0..16 {
            w.tick(away(), PlayerCommand::default());
        }
        let i = w.mc2_rivals[0].ent as usize;
        assert!(
            w.mc2_rival_cast(0, i, 1),
            "after window + cooldown expire the spell re-arms (cast-twice)"
        );
    }

    /// A landed cast de-latches the war toward ANY wizard target —
    /// not just the human (EF:5966-68). Rival 1 at war with rival 2
    /// closes in, fires, and the war flag drops.
    #[test]
    fn mc2_rival_war_delatches_on_landed_cast_vs_rival() {
        let mut w = mc2_brain_world(&[&[(0x10, 0)], &[]], &[(0x10, [50, 50, 50])]);
        let (r1, r2) = (w.mc2_rivals[0].ent, w.mc2_rivals[1].ent);
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.move_relink(r1 as usize, x, y, gz + 300);
        let (x2, _) = mc2_pos(104, 100);
        w.g.move_relink(r2 as usize, x2, y, gz + 300);
        let slot2 = w.mc2_rivals[1].slot as usize;
        w.mc2_rivals[0].war[slot2] = true;
        w.mc2_set_rival_state(0, crate::mc2::rivals::Mc2AiState::AttackWizard, r2);
        w.mc2_rivals[0].grace = 0;
        let mut delatched = false;
        for _ in 0..200 {
            w.tick(away(), PlayerCommand::default());
            if !w.mc2_rivals[0].war[slot2] {
                delatched = true;
                break;
            }
        }
        assert!(
            delatched,
            "a landed cast on a RIVAL wizard clears the war flag toward it"
        );
    }

    /// The castle raid NEVER claims the castle: retail state 7 lobs
    /// spells on cadence — it must not re-own the castle (`id24 = me`)
    /// once aimed.
    #[test]
    fn mc2_rival_raid_never_steals_the_castle() {
        let mut w = mc2_brain_world(
            &[&[(0x10, 0)], &[(2, 0)]],
            &[(0x10, [50, 50, 50]), (2, [100, 100, 100])],
        );
        let (r1, r2) = (w.mc2_rivals[0].ent, w.mc2_rivals[1].ent);
        // Rival 2's castle, standing.
        let (cx, cy) = mc2_pos(110, 100);
        let gz = w.g.ground_z(cx, cy) as i16;
        let c = w.g.new_event().expect("castle slot");
        {
            let e = &mut w.g.ent[c];
            e.class64 = 3;
            e.model65 = 2;
            e.tick70 = 4;
            e.max_life = 40000;
            e.id24 = r2;
        }
        w.g.link(c, cx, cy, gz);
        w.g.refill_life(c);
        // Rival 1 parked inside the 2048 cast ring, facing it.
        let (x, y) = mc2_pos(106, 100);
        w.g.move_relink(r1 as usize, x, y, gz + 300);
        w.mc2_set_rival_state(0, crate::mc2::rivals::Mc2AiState::RaidCastle, c as u16);
        let i = r1 as usize;
        for _ in 0..8 {
            w.mc2_rival_state_tick(0, i, true);
        }
        assert_eq!(
            w.g.ent[c].id24, r2,
            "the raided castle keeps its owner — no steal"
        );
        assert_eq!(
            w.mc2_rivals[0].state,
            crate::mc2::rivals::Mc2AiState::RaidCastle,
            "the raid posture holds (no claim-and-done exit)"
        );
    }

    /// The DEFENSE selector is the metamorph MIMICRY pick: a wizard
    /// threat nearby + a disguisable creature near THAT WIZARD arms
    /// the matching metamorph tier and targets the CREATURE.
    #[test]
    fn mc2_rival_defense_picks_the_disguise_tier() {
        let mut w = mc2_brain_world(&[&[(4, 2)]], &[(4, [10, 10, 10])]);
        let i = w.mc2_rivals[0].ent as usize;
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.move_relink(i, x, y, gz + 300);
        // The human 4 tiles away (inside 0x1400 = 20 tiles)...
        let pose = PlayerPose::from_tiles(104.5, (gz as f32 + 300.0) / 256.0, 100.5, 0.0, 0.0, 0.0);
        w.tick(pose, PlayerCommand::default());
        // ...and a tier-1 disguise creature (model 0x19) next to
        // the human.
        let (mx, my) = mc2_pos(105, 100);
        let anchor =
            w.g.mc2_spawn_creature_model(0x19, mx, my, gz)
                .expect("anchor creature");
        let picked = w.mc2_rival_pick_defense(0, i);
        assert!(picked, "the mimicry pick engages");
        assert_eq!(
            w.mc2_rivals[0].state,
            crate::mc2::rivals::Mc2AiState::Defense
        );
        assert_eq!(
            w.mc2_rivals[0].target, anchor as u16,
            "the disguise ANCHOR creature is the target — not the wizard"
        );
        let m4 = w.mc2_rivals[0].book.ent[4] as usize;
        assert_eq!(
            w.g.ent[m4].f71, 1,
            "model 0x19 maps to metamorph tier 1 (EF:7705-06)"
        );
    }

    /// The (10,22) whirlwind SWAYS the human player (retail
    /// `sub_33340`'s wizard branch): within the funnel it drags the
    /// player toward the eye via `player_knock` (not just chipping HP
    /// on direct overlap — the outer tornadoes must pull).
    #[test]
    fn mc2_whirlwind_sways_the_player() {
        let mut w = mc2_flat_world();
        let (wx, wy) = mc2_pos(100, 100);
        let gz = w.g.ground_z(wx, wy) as i16;
        w.g.mc2_spawn_whirlwind(wx, wy, gz).expect("whirlwind");
        // Park the player ~4 tiles from the funnel — inside the sway
        // band (< 13 tiles).
        let pose = PlayerPose::from_tiles(104.0, 14.0, 100.0, 0.0, 0.0, 0.0);
        w.tick(pose, PlayerCommand::default());
        assert!(
            w.g.player_knock.1 > 0,
            "the whirlwind drags the player inward (knock = {:?})",
            w.g.player_knock
        );
    }

    /// The whirlwind scales its life by tier (`sub_678E0` /
    /// EV:387-omitted arm-tornado modifier): `8 × row-21 tier.life`,
    /// overriding `AddWind`'s 500-tick ctor default. Tornado I
    /// (par1 0, tier life 5) = 40 ticks — a couple seconds, so it dies
    /// before drifting off the arm (so Tornado I/II/III differ and the
    /// level-01 arm tornado doesn't travel away).
    #[test]
    fn mc2_arm_tornado_scales_to_tier() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Spawn each tier in its own world (a whirlwind reserves 12
        // slots; a couple per world keeps the synthetic pool clear).
        let spawn_tier = |tier: u16| -> u32 {
            let mut a = assets();
            let mut spells = vec![
                crate::mc2::spells::Mc2SpellRow::default();
                crate::mc2::spells::MC2_SPELL_ROWS
            ];
            // Row 21 (whirlwind) tier lives, per baked SPELLS.DAT.
            spells[21].tiers[0].life = 5;
            spells[21].tiers[1].life = 10;
            spells[21].tiers[2].life = 10;
            a.spells = spells;
            // dis 5 = trigger-fired, like the level-01 arm dispositions
            // (9/11/13/15) — not a level-init (dis 0) spawn.
            let things = vec![Thing {
                slot: 1,
                kind: ThingKind::Entity,
                class: 10,
                model: 22,
                x: 100,
                y: 100,
                dis_id: 5,
                swi_sz: 0,
                swi_id: 5,
                parent: tier,
                child: 0,
                par3: Some(0),
            }];
            let planes = planes.clone();
            let mut w = World::new_for_game(planes, &things, 1, a, GameId::Mc2);
            w.debug_fire_disposition(5);
            let head = (1..w.g.ent.len())
                .find(|&i| w.g.ent[i].class64 == 10 && w.g.ent[i].model65 == 22)
                .expect("arm tornado spawned");
            w.g.ent[head].max_life
        };
        // Tornado I = 8×5 = 40, Tornado III = 8×10 = 80 — no longer the
        // identical 500-tick roamer.
        assert_eq!(spawn_tier(0), 40, "Tornado I = 8 × tier-0 life (5)");
        assert_eq!(spawn_tier(2), 80, "Tornado III = 8 × tier-2 life (10)");
    }

    /// Mana balls roll DOWNHILL under the terrain gradient (retail
    /// `sub_58030` inside the ball tick) — the level-001 transport that
    /// carries arm balls into the central basin before the 14-tile
    /// magnet aura can reach them (the aura is not the only thing
    /// pulling the balls to centre).
    #[test]
    fn mc2_mana_balls_roll_downhill() {
        // A west-facing slope: height rises with x, so a grounded ball
        // accelerates toward −x (the lower ground).
        let mut height = vec![0u8; 0x10000];
        for ty in 0..256usize {
            for tx in 0..256usize {
                height[(ty << 8) | tx] = (tx as u8).min(120);
            }
        }
        let planes = Planes {
            height,
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut w = World::new_for_game(planes, &[], 1, assets(), GameId::Mc2);
        let (bx, by) = mc2_pos(100, 100);
        let gz = w.g.ground_z(bx, by) as i16;
        let ball = w.g.mc2_spawn_mana_sphere(39, bx, by, gz).expect("ball");
        let x_start = w.g.ent[ball].x;
        let pose = PlayerPose::from_tiles(200.0, 20.0, 200.0, 0.0, 0.0, 0.0);
        for _ in 0..40 {
            w.tick(pose, PlayerCommand::default());
        }
        assert!(
            w.g.ent[ball].x < x_start,
            "the resting ball rolled downhill (west): {} -> {}",
            x_start,
            w.g.ent[ball].x
        );
    }

    /// The MC1/HW win-exit (Space = retail command 27, :20910/:48804):
    /// gated on the ALIVE state and the latched win flag — a Space
    /// without the flag does nothing, and the flag alone (retail
    /// 13325&2) never ends the level by itself.
    #[test]
    fn mc1_space_wins_only_when_alive_and_completed() {
        let mut w = flat_world();
        let pose = PlayerPose::level(100 << 8, 100 << 8, 1000, 0);
        let space = PlayerCommand {
            respawn: true,
            ..PlayerCommand::default()
        };
        w.tick(pose, space);
        assert!(!w.won(), "no win flag: Space is inert while alive");
        w.completed = true;
        w.tick(pose, PlayerCommand::default());
        assert!(!w.won(), "the win flag alone never ends the level");
        w.tick(pose, space);
        assert!(w.won(), "alive + won + Space = the win-exit");
    }

    /// The demon-mouth ending (sub_5E8C0_endGameSeq): the (14,4)
    /// mouth spawns HIDDEN; tripping the (11,31) marker does NOT end
    /// the level — it reveals the mouth and seizes the flyer into
    /// the scripted decelerate → aim → launch → terrain-glued fly-in
    /// → 32-tick fade, and only phase 0xC reports WON. (The portal
    /// must NOT be pre-shown, despawned on trip, or end instantly.)
    #[test]
    fn mc2_demon_mouth_ending_runs_the_fly_in() {
        let mut w = mc2_flat_world();
        let (mx, my) = mc2_pos(110, 100);
        let gz = w.g.ground_z(mx, my) as i16;
        let mouth = w.mc2_spawn_class14(4, mx, my, gz).expect("mouth");
        // The dis-gated spawn seam hides ending markers (models 3/4)
        // until the trip; mirror it for the direct spawn.
        w.g.ent[mouth].flags |= 0x20;
        let (tx, ty) = mc2_pos(100, 100);
        let tz = w.g.ground_z(tx, ty) as i16;
        let trig = w.spawn_trigger(31, tx, ty, tz).expect("trigger");
        w.g.ent[trig].f80 = 2 << 8;
        w.g.ent[trig].f82 = 2 << 8;
        // Park the player on the trigger; the phase gate (f63 & 7)
        // opens within 8 ticks.
        let pose = PlayerPose::from_tiles(100.0, 2.0, 100.0, 0.0, 0.0, 0.0);
        for _ in 0..10 {
            w.tick(pose, PlayerCommand::default());
            if w.mc2_end_pose().is_some() {
                break;
            }
        }
        assert!(w.mc2_end_pose().is_some(), "the trip seizes the flyer");
        assert_eq!(
            w.g.ent[mouth].flags & (0x20 | 0x400),
            0,
            "the trip REVEALS the mouth — it persists as the target"
        );
        assert!(!w.won(), "the trip alone must not end the level");
        // Run the sequence out: decelerate, aim east, launch, glue,
        // arrive (< 0x180), fade 32 — well inside 1000 ticks.
        let mut won_at = None;
        for t in 0..1000 {
            w.tick(pose, PlayerCommand::default());
            if w.won() {
                won_at = Some(t);
                break;
            }
        }
        assert!(won_at.is_some(), "the fly-in reaches the mouth and wins");
        let (ex, _, _, _) = w.mc2_end_pose().expect("pose holds through the end");
        assert!(
            (ex - 110.0).abs() < 2.0,
            "the scripted carpet stopped at the mouth (x = {ex})"
        );
        assert!(w.end_fade() >= 1.0, "faded to black");
    }

    /// The (10,22) whirlwind funnel and (10,76) fire-orb satellites
    /// carry real sprites and must appear as billboards — if the
    /// sprite-carrying effect entities are missing from the `drawable`
    /// allowlist, the whole effect runs invisible.
    #[test]
    fn mc2_whirlwind_and_fire_orb_are_drawable() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.mc2_spawn_whirlwind(x, y, gz).expect("whirlwind");
        let (fx, fy) = mc2_pos(120, 120);
        let fz = w.g.ground_z(fx, fy) as i16 + 500;
        w.g.mc2_spawn_fire_orb(fx, fy, fz).expect("fire orb");
        let poses = w.live_poses();
        let has = |c: u8, m: u8| poses.iter().any(|p| p.class == c && p.model == m);
        assert!(has(10, 22), "the whirlwind head draws");
        assert!(has(10, 75), "the whirlwind funnel column draws");
        assert!(has(10, 77), "the fire-orb satellites draw");
        // The invisible controllers stay out of the draw list.
        assert!(!has(10, 76), "the fire-orb hub is an invisible controller");
    }

    /// m18's 5-shot fan (sub_1D460): forcing the strike sub-state
    /// launches five (9,0) bolts aimed at the player in one volley,
    /// and the bolts fly the shared flyer core toward the target.
    #[test]
    fn mc2_m18_fan_launches_five_bolts() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(103, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let i = w.g.mc2_spawn_m18(x, y, gz).expect("m18 spawns");
        // Force the barrage strike sub-state at the player.
        w.g.ent[i].f146 = crate::mc1::mobs::PLAYER_TARGET;
        w.g.ent[i].tick70 = 146;
        w.g.ent[i].f71 = 1;
        w.g.ent[i].f26 = 50;
        w.g.ent[i].f58 = 64;
        let player = at_trigger(); // tile (100.5, 100.5)
        // The strike loop fires a volley every v_26 (4) ticks — the
        // FIRST observation must be exactly one 5-bolt fan.
        let mut first = 0usize;
        for _ in 0..8 {
            w.tick(player, PlayerCommand::default());
            let bolts =
                w.g.ent
                    .iter()
                    .filter(|e| e.class64 == 9 && e.model65 == 0 && e.flags & 0x400 == 0)
                    .count();
            if bolts > 0 {
                first = bolts;
                break;
            }
        }
        assert_eq!(first, 5, "the fan volley is five (9,0) bolts");
        // The volley resolves into (10,0) fire impacts (the bolts
        // carry impact class/model 10/0 and damage 800).
        for _ in 0..40 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(
            w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 0 && e.f140 == 800),
            "an impact fire carries the fan's 800 subSpell"
        );
    }

    /// m21 the floating caster: parked facing the player it acquires
    /// through the cone scan and launches (9,0) bolts (sub_1CC20,
    /// subSpell 500).
    #[test]
    fn mc2_m21_acquires_and_bolts() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(102, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let i = w.g.mc2_spawn_m21(x, y, gz + 200).expect("m21 spawns");
        // Force the engage mode at the player (the cone acquisition
        // rides the shared wizard scan, validated elsewhere; the
        // random wander-yaw makes it non-deterministic here).
        w.g.ent[i].f146 = crate::mc1::mobs::PLAYER_TARGET;
        w.g.ent[i].tick70 = 170;
        w.g.ent[i].f68 = 0;
        let player = at_trigger();
        let mut bolt_seen = false;
        for _ in 0..64 {
            w.tick(player, PlayerCommand::default());
            bolt_seen |=
                w.g.ent.iter().any(|e| {
                    e.class64 == 9 && e.model65 == 0 && e.f44 == 500 && e.flags & 0x400 == 0
                });
        }
        assert!(bolt_seen, "the caster launched its 500-damage bolt");
    }

    /// The tree burn ladder: channel-0 damage over the tree's life
    /// spawns the flame element, re-seeds a 130..189 burn life and
    /// walks state 0 -> 1 -> 2 with the charred sprite swap.
    #[test]
    fn mc2_tree_burns_to_charred_stump() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(120, 120);
        let gz = w.g.ground_z(x, y) as i16;
        let t = w.g.mc2_spawn_tree(x, y, gz).expect("tree spawns");
        let life = w.g.ent[t].act_life;
        w.g.mail_write(
            crate::mc1::combat::MailTarget::Pool(t),
            0,
            life as u32 + 1,
            999,
        );
        let player = away();
        w.tick(player, PlayerCommand::default());
        assert_eq!(w.g.ent[t].tick70, 1, "tree entered the burning state");
        assert!(
            w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 6 && e.flags & 0x400 == 0),
            "the (10,6) standing fire spawned on the tree"
        );
        let burn = w.g.ent[t].act_life;
        assert!(
            (70..=189).contains(&burn),
            "re-seeded burn life, got {burn}"
        );
        for _ in 0..200 {
            w.tick(player, PlayerCommand::default());
            if w.g.ent[t].tick70 == 2 {
                break;
            }
        }
        assert_eq!(w.g.ent[t].tick70, 2, "tree charred");
        assert!(
            matches!(w.g.ent[t].type86, 226 | 227),
            "charred sprite swap, got {}",
            w.g.ent[t].type86
        );
    }

    /// The (10,6) standing ground fire (sub_31760): the 6-step
    /// sprite ramp-up, per-tick channel-0 area heat (a tree standing
    /// in it takes the sub_11400 tenth), and the water extinguish.
    #[test]
    fn mc2_standing_fire_burns_and_drowns() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(120, 120);
        let gz = w.g.ground_z(x, y) as i16;
        let t = w.g.mc2_spawn_tree(x, y, gz).expect("tree spawns");
        let f = w.g.mc2_spawn_fire6(x, y, gz).expect("fire spawns");
        assert_eq!(w.g.ent[f].type86, 228, "sprite row 228");
        assert_eq!(w.g.ent[f].max_life, 240);
        let life0 = w.g.ent[t].act_life;
        let player = away();
        for _ in 0..8 {
            w.tick(player, PlayerCommand::default());
        }
        // The ramp-up latches at dword_0x10_16 == 7, row 228 -> 235.
        assert_eq!(w.g.ent[f].f26, 7, "grow steps latched");
        assert_eq!(w.g.ent[f].type86, 235, "sprite row grown");
        assert!(
            w.g.ent[t].act_life < life0,
            "the tree in the fire takes per-tick ch0 heat"
        );
        assert_eq!(
            w.g.ent[t].tick70, 0,
            "a tenth per tick has not ignited it yet"
        );
        // Water under the fire extinguishes it.
        w.g.t.tile_type[tile(120, 120)] = 0;
        w.tick(player, PlayerCommand::default());
        assert!(w.g.ent[f].flags & 0x400 != 0, "extinguished by water");
    }

    /// The (10,34) MC2 teleporter pad (sub_35390): a THING-authored
    /// pad carries its par1/par2 destination tile; a player in reach
    /// FACING the pad warps there (sound 22); facing away does not.
    #[test]
    fn mc2_teleporter_warps_facing_player() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 1,
            kind: ThingKind::Entity,
            class: 10,
            model: 34,
            x: 100,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 40, // par1 = destination tile Y
            child: 30,  // par2 = destination tile X
            par3: None,
        }];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        let p =
            w.g.ent
                .iter()
                .position(|e| e.class64 == 10 && e.model65 == 34)
                .expect("pad spawned");
        assert_eq!(w.g.ent[p].type86, 223, "pad sprite 223");
        assert_eq!(w.g.ent[p].dest_x, (30 << 8) + 128, "par2 -> dest X");
        assert_eq!(w.g.ent[p].dest_y, (40 << 8) + 128, "par1 -> dest Y");
        let (padx, pady) = (w.g.ent[p].x, w.g.ent[p].y);
        // Stand just east of the pad, facing AWAY: no warp.
        let (px, py) = (padx.wrapping_add(200), pady);
        let bearing = Gen::angle_of(
            Gen::wrap_delta(padx as i16, px as i16) as i16,
            Gen::wrap_delta(pady as i16, py as i16) as i16,
        );
        let mut pose = away();
        pose.x = px;
        pose.y = py;
        pose.z = w.g.ground_z(px, py) as i16;
        pose.heading = bearing.wrapping_add(1024) & 0x7FF;
        w.tick(pose, PlayerCommand::default());
        assert!(w.take_teleport().is_none(), "facing away: no warp");
        // Turn toward the pad: warp to the par-authored tile.
        pose.heading = bearing;
        w.tick(pose, PlayerCommand::default());
        let dest = w.take_teleport().expect("facing the pad warps");
        assert_eq!(dest, (30.5, 40.5), "destination = par2/par1 center");
        assert!(
            w.g.ent[p].flags & 0x400 == 0,
            "the pad persists (maxLife 0)"
        );
    }

    /// The (10,50) ridge-fence chain (sub_49090 → sub_48880): a
    /// 2-node par-linked chain settles at load — the (10,51) beams
    /// fly the segment raising the heightmap (sub_56F10 +10..24 per
    /// disc) and are gone before the first frame.
    #[test]
    fn mc2_fence_chain_raises_ridge_at_load() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |slot, x, y, parent, child| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 10,
            model: 50,
            x,
            y,
            dis_id: 0xFFFF, // DisId -1 = the generate pass
            swi_sz: 0,
            swi_id: 1, // stageTag != 0 = chained
            parent,
            child,
            par3: None,
        };
        let things = vec![th(1, 100, 100, 0, 2), th(2, 110, 100, 1, 0)];
        let w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        assert!(
            !w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && matches!(e.model65, 50 | 51) && e.flags & 0x400 == 0),
            "the beams settled at load, no live fence entities"
        );
        let mid = tile(104, 100);
        assert!(
            w.g.t.height[mid] > 100,
            "the ridge rose along the segment, got {}",
            w.g.t.height[mid]
        );
        assert_eq!(w.g.t.angle[mid] & 7, 1, "raised cells carry class 1");
    }

    /// The (10,28) road chain (sub_49090 → sub_48400 → the (10,27)
    /// walkers, collapsed): a 2-node chain paints a type-8 ridge
    /// staircase (+48 height, borders angle-locked) at load.
    #[test]
    fn mc2_road_chain_paints_ridge_at_load() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |slot, x, y, parent, child| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 10,
            model: 28,
            x,
            y,
            dis_id: 0xFFFF,
            swi_sz: 0,
            swi_id: 1,
            parent,
            child,
            par3: None,
        };
        let things = vec![th(1, 100, 100, 0, 2), th(2, 120, 100, 1, 0)];
        let w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        // An X-major road: type 8 + the +48 raise along the run.
        let hits = (100..120)
            .filter(|&x| w.g.t.tile_type[tile(x, 100)] == 8)
            .count();
        assert!(hits >= 15, "road surface painted along the run, got {hits}");
        let raised = (100..120)
            .filter(|&x| w.g.t.height[tile(x, 100)] == 148)
            .count();
        assert!(raised >= 10, "ridge rose +48 along the run, got {raised}");
        assert!(
            (100..120).any(|x| w.g.t.angle[tile(x, 99)] & 0x80 != 0),
            "border rows are authored-locked"
        );
    }

    /// The (10,17) meteor (sub_32880): 10 ticks of 300 area damage
    /// laying rings of damage-suppressed (10,0) fire visuals
    /// (dword |= 0x10080), then gone.
    #[test]
    fn mc2_meteor_lays_fire_rings() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(120, 120);
        let gz = w.g.ground_z(x, y) as i16;
        let t = w.g.mc2_spawn_tree(x, y, gz).expect("tree");
        let m = w.g.mc2_spawn_meteor(x, y, gz).expect("meteor");
        let life0 = w.g.ent[t].act_life;
        let player = away();
        for _ in 0..4 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(
            w.g.ent.iter().any(|e| e.class64 == 10
                && e.model65 == 0
                && e.flags & 0x1_0080 == 0x1_0080
                && e.flags & 0x400 == 0),
            "damage-suppressed (10,0) ring children live"
        );
        assert!(
            w.g.ent[t].act_life < life0,
            "the tree took the meteor's 300/tick (tenth) heat"
        );
        for _ in 0..12 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(
            w.g.ent[m].flags & 0x400 != 0,
            "meteor expired after 10 ticks"
        );
    }

    /// The (10,15) fire trail (sub_32530): wanders dropping a (10,11)
    /// SCORCH RING (the earth-carve) each tick — each child lives only
    /// 10 ticks, so the concurrent population stays ~11 (NOT a (10,19)
    /// spray, whose 240-life smoke-spewing would flood the pool).
    #[test]
    fn mc2_fire_trail_drops_scorch_rings() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(140, 140);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.mc2_spawn_fire_trail(x, y, gz).expect("trail");
        let player = away();
        let live = |w: &World, m: u8| {
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == m && e.flags & 0x400 == 0)
                .count()
        };
        let mut peak_rings = 0;
        for _ in 0..30 {
            w.tick(player, PlayerCommand::default());
            peak_rings = peak_rings.max(live(&w, 11));
            assert_eq!(live(&w, 19), 0, "the trail lays NO (10,19) fire sprays");
        }
        assert!(peak_rings >= 3, "the trail lays (10,11) scorch rings");
        // 10-tick child life keeps the concurrent count small — no flood.
        assert!(
            peak_rings <= 20,
            "scorch rings are transient (~11 concurrent), got {peak_rings}"
        );
    }

    /// The (10,22) whirlwind (AddWind_4F040 + sub_33110): the head
    /// and its 11 tail nodes spawn as a chained sprite stack; a
    /// nearby creature gets swirled/grabbed and takes the 1000 mail
    /// while airborne; teardown despawns the whole chain.
    #[test]
    fn mc2_whirlwind_grabs_and_expires() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(150, 150);
        let gz = w.g.ground_z(x, y) as i16;
        let h = w.g.mc2_spawn_whirlwind(x, y, gz).expect("wind");
        let nodes =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == 75 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(nodes, 11, "the 11-node tail chain");
        let g = w.g.mc2_spawn_goat(x, y, gz).expect("goat");
        let life0 = w.g.ent[g].act_life;
        let player = away();
        for _ in 0..30 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(
            w.g.ent[g].act_life < life0 || w.g.ent[g].flags & 0x400 != 0,
            "the goat took whirlwind damage"
        );
        // Expiry: force the tail of life and check the teardown.
        w.g.ent[h].act_life = 0;
        w.tick(player, PlayerCommand::default());
        w.tick(player, PlayerCommand::default());
        assert!(w.g.ent[h].flags & 0x400 != 0, "head despawned");
        assert_eq!(
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == 75 && e.flags & 0x400 == 0)
                .count(),
            0,
            "tail nodes torn down with the head"
        );
    }

    /// The (10,71) fissure (sub_3A2D0): the ground jitters ±1 inside
    /// the ramping disc and the 4th-tick area beat damages a
    /// bystander.
    #[test]
    fn mc2_fissure_vibrates_ground() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(160, 160);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.mc2_spawn_fissure(x, y, gz).expect("fissure");
        let player = away();
        let mut moved = false;
        for _ in 0..40 {
            w.tick(player, PlayerCommand::default());
            moved |= w.g.t.height[tile(160, 160)] != 100;
        }
        assert!(moved, "the fissure jittered the heightmap");
    }

    /// The (10,67) flood/quake (mc2::flood, sub_39040 + the 72/73/74
    /// action chain): the crater center punches to height 0 by the
    /// end of phase 2, the damage pass grabs+shakes+mails an
    /// overlapping castle (±17-tile AABB, XY-only), the shove tags a
    /// center bystander with the tossed latch, and the action-74
    /// finisher restores the terrain, releases the castle and
    /// despawns.
    #[test]
    fn mc2_flood_quake_craters_shoves_and_restores() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(180, 180);
        // Retail's (pos+128)>>8 center-tile trap: the machine runs on
        // tile (181, 181).
        let (cx, cy) = (181u8, 181u8);
        let gz = w.g.ground_z(x, y) as i16;
        let f = w.g.mc2_spawn_flood(x, y, gz).expect("flood");
        assert_eq!(
            (w.g.ent[f].act_life, w.g.ent[f].f140, w.g.ent[f].f80),
            (120, 20000, 4352),
            "ctor: life 120, subSpell 20000, ±17-tile AABB"
        );
        let goat = w.g.mc2_spawn_goat(x, y, gz).expect("goat");
        // A castle 15 tiles east: inside the grab AABB (3968 < 4352),
        // outside the 13-tile shove disc (so only the finisher's
        // phase-2 scan releases it).
        let castle = w.g.new_event().expect("castle slot");
        {
            let e = &mut w.g.ent[castle];
            e.class64 = 3;
            e.model65 = 2;
            e.x = (cx as u16 + 15) << 8;
            e.y = (cy as u16) << 8;
            e.max_life = 100_000;
            e.act_life = 100_000;
            e.f59 = 4; // standing
        }
        let player = away();
        // Tick 1 = probe, tick 2 = sample + first morph (countdown
        // 12→11), ticks 3..8 → countdown 5 = the damage pass.
        for _ in 0..8 {
            w.tick(player, PlayerCommand::default());
        }
        use crate::mc2::mobs::F_NO_CORPSE;
        assert!(
            w.g.ent[castle].flags & F_NO_CORPSE != 0,
            "damage pass grabbed the castle"
        );
        assert_eq!(w.g.ent[castle].f40, f as u16, "grab owner = the flood");
        assert!(w.g.ent[castle].f50 > 0, "the 30-tick castle shake armed");
        assert!(
            w.g.ent[castle].act_life < 100_000 || w.g.ent[castle].mail[0].0 > 0,
            "the castle took the 20000 subSpell mail"
        );
        // Run out phase 2 (countdown → 0 at tick 13) into phase 3.
        for _ in 0..6 {
            w.tick(player, PlayerCommand::default());
        }
        assert_eq!(w.g.ent[f].tick70, 73, "phase 3 handed to action 73");
        assert_eq!(
            w.g.t.height[tile(cx, cy)],
            0,
            "the 2x2 crater floor punched to height 0"
        );
        assert!(
            w.g.t.height[tile(cx.wrapping_sub(5), cy)] < 100,
            "the inner disc sank"
        );
        assert!(
            w.g.ent[goat].flags & crate::mc2::flood::F_TOSSED != 0
                || w.g.ent[goat].flags & 0x400 != 0,
            "the center bystander was tossed (or killed on the 1-in-7)"
        );
        // Fast-forward the action-73 shove hold into the finisher.
        w.g.ent[f].act_life = 1;
        w.tick(player, PlayerCommand::default());
        assert_eq!(w.g.ent[f].tick70, 74, "life out → the restore finisher");
        for _ in 0..24 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(w.g.ent[f].flags & 0x400 != 0, "the flood despawned");
        assert!(
            w.g.ent[castle].flags & F_NO_CORPSE == 0 && w.g.ent[castle].f40 == 0,
            "the finisher released the grabbed castle"
        );
        assert!(
            w.g.t.height[tile(cx, cy)] > 40,
            "the settle eased the crater back toward the rim, got {}",
            w.g.t.height[tile(cx, cy)]
        );
    }

    /// The (10,67) par1 seam is DIS-PATH-ONLY: only a DisId == -1
    /// record would keep the ctor defaults (EV:387's override list is
    /// 9/0xB/0xF — and retail never generate-passes 0x43 at all),
    /// while EVERY fired record — dis 0 at init via sub_4A1E0(0)
    /// included — consumes par1 → SPELLS row 20 life + subSpell
    /// (sub_4A310 case 0xA, EF:33148/:33165).
    #[test]
    fn mc2_flood_par1_trigger_seam() {
        // Synthetic SPELLS.DAT: row 20 tier 2 = subSpell 5555, life 77.
        let mut bytes = vec![0u8; 26 * 80];
        let base = 20 * 80 + 2 + 26 * 2;
        bytes[base..base + 4].copy_from_slice(&5555i32.to_le_bytes());
        bytes[base + 24] = 77;
        let mut a = assets();
        a.spells = crate::mc2::spells::parse(&bytes).unwrap();
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let rec = |slot, x, dis| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 10,
            model: 67,
            x,
            y: 60,
            dis_id: dis,
            swi_sz: 0,
            swi_id: 0,
            parent: 2, // par1 = tier 2
            child: 0,
            par3: None,
        };
        let things = [rec(1, 60, 0), rec(2, 200, 7)];
        let mut w = World::new_for_game(planes, &things, 1, a, GameId::Mc2);
        let at = |w: &World, tx: u16| {
            (1..w.g.ent.len())
                .find(|&i| {
                    w.g.ent[i].class64 == 10 && w.g.ent[i].model65 == 67 && w.g.ent[i].x >> 8 == tx
                })
                .expect("flood spawned")
        };
        let a0 = at(&w, 60);
        // dis 0 is NOT the load path (that is DisId == -1, which
        // retail never generate-passes for 0x43): dis-0 records fire
        // at init via sub_4A1E0(0) → sub_4A310 and consume par1 like
        // any triggered spawn.
        assert_eq!(
            (w.g.ent[a0].act_life, w.g.ent[a0].f140),
            (77, 5555),
            "the init-fired dis-0 spawn consumed par1 via SPELLS row 20"
        );
        w.fire_disposition(7, true);
        let a1 = at(&w, 200);
        assert_eq!(
            (w.g.ent[a1].act_life, w.g.ent[a1].f140),
            (77, 5555),
            "the triggered spawn consumed par1 via SPELLS row 20"
        );
    }

    /// Dis-fired (10,17) meteors and (10,71) fissures consume par1 as
    /// their SPELLS tier (sub_4A310 EF:33148-78 — 0x11 writes maxLife
    /// AND life, 0x47 life only). All 69/21 shipped records are
    /// dis-gated with par1 = tier 1..2.
    #[test]
    fn mc2_meteor_and_fissure_consume_dis_par1_tier() {
        // Synthetic SPELLS.DAT: row 9 (meteor) tier 1 = subSpell 4444,
        // life 55; row 15 (fissure) tier 2 = subSpell 3333, life 66.
        let mut bytes = vec![0u8; 26 * 80];
        let m = 9 * 80 + 2 + 26;
        bytes[m..m + 4].copy_from_slice(&4444i32.to_le_bytes());
        bytes[m + 24] = 55;
        let f = 15 * 80 + 2 + 26 * 2;
        bytes[f..f + 4].copy_from_slice(&3333i32.to_le_bytes());
        bytes[f + 24] = 66;
        let mut a = assets();
        a.spells = crate::mc2::spells::parse(&bytes).unwrap();
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let rec = |slot, model, x, par1| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 10,
            model,
            x,
            y: 60,
            dis_id: 7,
            swi_sz: 0,
            swi_id: 0,
            parent: par1,
            child: 0,
            par3: None,
        };
        let things = [rec(1, 17, 60, 1), rec(2, 71, 200, 2)];
        let mut w = World::new_for_game(planes, &things, 1, a, GameId::Mc2);
        w.fire_disposition(7, true);
        let at = |w: &World, model: u16, tx: u16| {
            (1..w.g.ent.len())
                .find(|&i| {
                    w.g.ent[i].class64 == 10
                        && w.g.ent[i].model65 == model as u8
                        && w.g.ent[i].x >> 8 == tx
                })
                .expect("spawned")
        };
        let meteor = at(&w, 17, 60);
        assert_eq!(
            (
                w.g.ent[meteor].max_life,
                w.g.ent[meteor].act_life,
                w.g.ent[meteor].f140
            ),
            (55, 55, 4444),
            "meteor tier-1: maxLife AND life + subspell (0x11 arm)"
        );
        let fissure = at(&w, 71, 200);
        assert_eq!(
            (w.g.ent[fissure].act_life, w.g.ent[fissure].f140),
            (66, 3333),
            "fissure tier-2: life + subspell (0x47 arm)"
        );
    }

    /// The (10,18) summit vortex (sub_32A70, mc2::morph): tick 0's
    /// unconditional pulse seizes the vortex/plume singletons and
    /// births the (10,19) fire column, a (10,16) tornado (the
    /// whirlwind driver under action 16) and the visual (9,0) bolt.
    #[test]
    fn mc2_summit_vortex_erupts() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(140, 200);
        let gz = w.g.ground_z(x, y) as i16;
        let v = w.g.mc2_spawn_summit18(x, y, gz).expect("vortex");
        let player = away();
        w.tick(player, PlayerCommand::default());
        let live = |w: &World, c: u8, m: u8| {
            w.g.ent
                .iter()
                .any(|e| e.class64 == c && e.model65 == m && e.flags & 0x400 == 0)
        };
        assert!(live(&w, 10, 19), "the fire-spray column rose");
        assert!(live(&w, 10, 16), "a tornado spun up");
        assert_eq!(w.g.erupting, v as u16, "the vortex latch seized");
        assert_eq!(
            w.g.plume,
            w.g.ent
                .iter()
                .position(|e| e.class64 == 10 && e.model65 == 19 && e.flags & 0x400 == 0)
                .unwrap() as u16,
            "the plume latch tracks the column"
        );
        for _ in 0..30 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(w.g.ent[v].f26 > 30, "the pulse clock advances");
    }

    /// The self-latched vortex idles forever post-win (its restart
    /// roll is gated on the latch it holds) and retail's i32 clock just
    /// keeps counting — our i16 home must saturate, not panic, at 32767
    /// (~30 min real time).
    #[test]
    fn mc2_summit_vortex_clock_saturates() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(140, 200);
        let gz = w.g.ground_z(x, y) as i16;
        let v = w.g.mc2_spawn_summit18(x, y, gz).expect("vortex");
        let player = away();
        w.g.erupting = v as u16; // the self-latch that pins the roll shut
        w.g.ent[v].f26 = i16::MAX - 2;
        for _ in 0..8 {
            w.tick(player, PlayerCommand::default());
        }
        assert_eq!(w.g.ent[v].f26, i16::MAX, "clock saturates at the rim");
        assert_eq!(w.g.ent[v].flags & 0x400, 0, "the idle vortex stays live");
    }

    /// The (10,91) apocalypse mana rain (sub_32CF0, mc2::morph):
    /// three thrown (10,39) spheres per tick, mana 1..=2560 (the
    /// 5-draw arming order), riding the ball machinery.
    #[test]
    fn mc2_summit91_rains_mana() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(60, 200);
        let gz = w.g.ground_z(x, y) as i16;
        let c = w.g.mc2_spawn_summit91(x, y, gz).expect("rain");
        let player = away();
        // Capture each sphere's roll the tick it appears — resting
        // balls MERGE (mobs.rs ball tick, f140 sums), so a late
        // sweep can read a summed value instead of the arming roll.
        let mut seen = std::collections::HashSet::new();
        let mut balls: Vec<i32> = Vec::new();
        for _ in 0..4 {
            w.tick(player, PlayerCommand::default());
            for (j, e) in w.g.ent.iter().enumerate() {
                if j != c
                    && e.class64 == 10
                    && e.model65 == 39
                    && e.flags & 0x400 == 0
                    && seen.insert(j)
                {
                    balls.push(e.f140);
                }
            }
        }
        assert!(
            balls.len() >= 9,
            "3 spheres/tick rained, got {}",
            balls.len()
        );
        // ≥ 9 in-band rolls pins the arming law; a same-tick merge
        // can still fold two rolls into one out-of-band sum.
        assert!(
            balls.iter().filter(|&&m| (1..=2560).contains(&m)).count() >= 9,
            "mana in the 1..=2560 roll band: {balls:?}"
        );
    }

    /// The apocalypse-rain DECAY channel: retail's rain spheres carry
    /// `byte[1] |= 0x20` + life 140 and
    /// fade out of existence — at life 12 the 67% death-fade bit
    /// (24) arms, at 6 the bit-23 ghost, at 0 the sphere expires
    /// (EF:26289-307). Decaying spheres never INITIATE a merge. The
    /// rain must be timed window dressing, never a permanent mana
    /// mine; ordinary (unflagged) spheres are untouched.
    #[test]
    fn mc2_rain_spheres_decay_and_expire() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(60, 200);
        let gz = w.g.ground_z(x, y) as i16;
        // One rain-flagged sphere and one ordinary control, far
        // apart (no merge interference).
        let r = w.g.spawn_mana_ball(x, y, gz).expect("rain ball");
        {
            let e = &mut w.g.ent[r];
            e.max_life = 140;
            e.act_life = 140;
            e.flags |= 0x2000;
            e.f140 = 500;
        }
        let (cx, cy) = mc2_pos(80, 200);
        let cgz = w.g.ground_z(cx, cy) as i16;
        let c = w.g.spawn_mana_ball(cx, cy, cgz).expect("control ball");
        w.g.ent[c].f140 = 500;
        let player = away();
        let mut saw_fade = false;
        let mut saw_ghost = false;
        for _ in 0..150 {
            w.tick(player, PlayerCommand::default());
            let e = &w.g.ent[r];
            if e.flags & 0x400 != 0 {
                break;
            }
            if e.act_life <= 12 && e.act_life > 6 {
                saw_fade = e.flags & (1 << 24) != 0;
            }
            if e.act_life <= 6 && e.act_life > 0 {
                saw_ghost = e.flags & (1 << 23) != 0 && e.flags & (1 << 24) == 0;
            }
        }
        assert!(saw_fade, "the 67% fade bit armed at life 12");
        assert!(saw_ghost, "the ghost bit took over at life 6");
        assert!(
            w.g.ent[r].flags & 0x400 != 0,
            "the rain sphere expired at the end of its life"
        );
        assert_eq!(
            w.g.ent[c].flags & 0x400,
            0,
            "the ordinary sphere persists untouched"
        );
        assert_eq!(
            w.g.ent[c].act_life, w.g.ent[c].max_life as i32,
            "no stray decay"
        );
    }

    /// The (10,65)/(10,66) one-tick debuff stamps (sub_38E70/38F70,
    /// mc2::proj): a paralyze stamp aimed at a wizard body mails its
    /// subSpell 200 and despawns; the stagger variant mails nothing.
    #[test]
    fn mc2_debuff_stamps_mail_wizards() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(70, 200);
        // A rival wizard body husk.
        let wiz = w.g.new_event().expect("wizard slot");
        {
            let e = &mut w.g.ent[wiz];
            e.class64 = 3;
            e.model65 = 0;
            e.max_life = 1000;
            e.act_life = 1000;
            e.x = x;
            e.y = y;
        }
        let s = w.g.mc2_spawn_paralyze(x, y, 3200).expect("stamp");
        w.g.ent[s].f146 = wiz as u16;
        let g = w.g.mc2_spawn_stagger(x, y, 3200).expect("stagger");
        w.g.ent[g].f146 = wiz as u16;
        let player = away();
        w.tick(player, PlayerCommand::default());
        assert_eq!(w.g.ent[wiz].mail[0].0, 200, "the paralyze 200 mail landed");
        assert!(
            w.g.ent[s].flags & 0x400 != 0 && w.g.ent[g].flags & 0x400 != 0,
            "one-tick stamps despawned"
        );
    }

    /// The (10,76) fire-sphere orb (AddFireSpheres_4F2A0): hub + 25
    /// satellites (5 targetable slot-0 damage carriers), pulsing and
    /// tumbling; collapse leaves a (10,0) ground fire and tears the
    /// chain down.
    #[test]
    fn mc2_fire_orb_pulses_and_collapses() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(170, 170);
        let gz = w.g.ground_z(x, y) as i16;
        let h = w.g.mc2_spawn_fire_orb(x, y, gz + 500).expect("orb");
        let sats =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == 77 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(sats, 25, "the 25-satellite lattice");
        assert_eq!(
            w.g.ent
                .iter()
                .filter(|e| e.model65 == 77 && e.flags & 8 != 0)
                .count(),
            5,
            "five slot-0 damage carriers"
        );
        let player = away();
        let mut fire_seen = false;
        for _ in 0..140 {
            w.tick(player, PlayerCommand::default());
            // The collapse fire lives 8 ticks — observe it in flight.
            fire_seen |=
                w.g.ent
                    .iter()
                    .any(|e| e.class64 == 10 && e.model65 == 0 && e.flags & 0x400 == 0);
        }
        assert!(
            w.g.ent[h].flags & 0x400 != 0,
            "orb collapsed after life+radius"
        );
        assert_eq!(
            w.g.ent
                .iter()
                .filter(|e| e.model65 == 77 && e.flags & 0x400 == 0)
                .count(),
            0,
            "satellites torn down"
        );
        assert!(fire_seen, "the collapse ground fire burned");
    }

    /// The class-11 slot-condition switch (sub_6F300): a model-13
    /// switch watches class-5 model 0; with the slot empty its
    /// 16-tick countdown arms and it chain-fires its disposition.
    #[test]
    fn mc2_slot_switch_fires_when_slot_empty() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
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
        let things = vec![
            // The slot-0 watcher, firing disposition 1 when no
            // class-5 model-0 creature lives.
            th(1, 11, 13, 100, 100, 0, 3, 1),
            // A standing stone behind disposition 1.
            th(2, 2, 1, 110, 110, 1, 0, 0),
        ];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        let player = away();
        let stone_live = |w: &World| {
            w.g.ent
                .iter()
                .any(|e| e.class64 == 2 && e.model65 == 1 && e.flags & 0x400 == 0)
        };
        assert!(!stone_live(&w), "disposition 1 is gated at start");
        for _ in 0..40 {
            w.tick(player, PlayerCommand::default());
        }
        assert!(stone_live(&w), "the empty slot chain-fired the switch");
    }

    /// The ANY-slot variant (model 30, `sub_6F300` a2 == -1) watches
    /// slots 0..=0xB and 0x10 ONLY — the retail scan loop's bound is
    /// `<= 16` (NETHERW.EXE @0x93BA6, `cmp eax,0x10; jng`), so high
    /// models 0x11..=0x1C never gate it. Level 024's opening gauntlet
    /// depends on this: the authored wandering hydra (5,27) must not
    /// block the (11,30) wall-expansion gates.
    #[test]
    fn mc2_any_slot_switch_ignores_high_models() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
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
        let things = vec![
            // The ANY-slot watcher, firing disposition 1.
            th(1, 11, 30, 100, 100, 0, 3, 1),
            // A live HIGH-model creature (the level-024 hydra shape):
            // slot 0x1B is outside the retail scan bound.
            th(2, 5, 27, 200, 200, 0, 0, 0),
            // A standing stone behind disposition 1.
            th(3, 2, 1, 110, 110, 1, 0, 0),
        ];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        let stone_live = |w: &World| {
            w.g.ent
                .iter()
                .any(|e| e.class64 == 2 && e.model65 == 1 && e.flags & 0x400 == 0)
        };
        let hydra_live = |w: &World| {
            w.g.ent.iter().any(|e| {
                e.class64 == 5
                    && e.model65 == 27
                    && e.act_life >= 0
                    && e.flags & 0x400 == 0
                    && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
            })
        };
        assert!(!stone_live(&w), "disposition 1 is gated at start");
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(hydra_live(&w), "the high-model blocker is still live");
        assert!(
            stone_live(&w),
            "the ANY-slot switch fired despite the live high model"
        );
    }

    // ---- Phase-4.3 MC2 terrain riser probes ----------------------------

    fn riser_things(tx: u16, ty: u16, orient: u16, len: u16, with_lower: bool) -> Vec<Thing> {
        let th = |slot, class, model, parent, child| Thing {
            slot,
            kind: ThingKind::Entity,
            class,
            model,
            x: tx,
            y: ty,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent,
            child,
            par3: None,
        };
        let mut v = vec![th(1, 14, 1, orient, len)];
        if with_lower {
            // The campaign pattern: the (10,63) LOWER trigger in the
            // SAME map cell as its riser.
            v.push(th(2, 10, 63, 0, 0));
        }
        v
    }

    /// The (14,1) riser's life-0 INSTANT build (sub_59F60,
    /// docs/traces/mc2-class14-m1-riser.md §3): a dis-0 orientation-0
    /// (+X strip) riser one-shots +48 height over 2 rows x L cols,
    /// stamps ridge type 8 / angle 1 over 3 rows x L+1 cols (base
    /// cell = the authentic +128 round-up minus one), marks dirty,
    /// and parks idle-built (life 3, sub 48, L = par2+1).
    #[test]
    fn mc2_riser_instant_build_x() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = riser_things(100, 100, 0, 8, false);
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.tick(away(), PlayerCommand::default());
        let t = crate::engine::features::tile;
        let r = &w.g.ent[1];
        assert_eq!((r.class64, r.model65), (14, 1));
        assert_eq!(r.act_life, 3, "idle-built");
        assert_eq!(r.f44, 48);
        assert_eq!(r.f26, 9, "length = par2+1 after the life-0 ++");
        // Base B = (100, 101): derived cell (101,101) minus one on X.
        // Strip rows by/by-1 = 101/100 rise +48 over cols bx..bx+8.
        assert_eq!(w.g.t.height[t(104, 100)], 148);
        assert_eq!(w.g.t.height[t(104, 101)], 148);
        // Row by-2 = 99 is stamped ridge but NOT raised.
        assert_eq!(w.g.t.height[t(104, 99)], 100);
        for row in [99u8, 100, 101] {
            assert_eq!(w.g.t.tile_type[t(104, row)], 8, "ridge type, row {row}");
            let a = w.g.t.angle[t(104, row)];
            assert_eq!(a & 0xF, 1, "class nibble 1 (deep-water bit clear)");
            assert_ne!(a & 0x80, 0, "renderer dirty bit");
        }
        // Outside the stamp footprint: untouched.
        assert_eq!(w.g.t.tile_type[t(104, 103)], 5);
        assert_eq!(w.g.t.height[t(104, 103)], 100);
    }

    /// Orientation 1 (+Y strip) mirror: base B = (151, 150), 2
    /// columns bx/bx-1 rise, 3 columns stamp.
    #[test]
    fn mc2_riser_instant_build_y() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = riser_things(150, 150, 1, 8, false);
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        w.tick(away(), PlayerCommand::default());
        let t = crate::engine::features::tile;
        assert_eq!(w.g.ent[1].act_life, 3);
        assert_eq!(w.g.t.height[t(151, 154)], 148);
        assert_eq!(w.g.t.height[t(150, 154)], 148);
        // Column bx-2 = 149: stamped, not raised.
        assert_eq!(w.g.t.height[t(149, 154)], 100);
        assert_eq!(w.g.t.tile_type[t(149, 154)], 8);
    }

    /// The full lower/raise cycle (§5/§6): the co-located (10,63)
    /// pokes life 2 on its one-shot tick and despawns; 48 sink ticks
    /// take the strip interior back to the flank average; the final
    /// tick restores flank terrain (type/angle copy, shading 32) and
    /// the next parks life 4. A (10,64) then re-runs the ANIMATED
    /// raise (first-tick interior stamp + 48 x +1) back to built.
    /// The never-animated strip ENDS keep their instant-built ridge —
    /// verbatim retail asymmetry.
    #[test]
    fn mc2_riser_lower_restore_and_reraise() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = riser_things(100, 100, 0, 8, true);
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        let t = crate::engine::features::tile;
        // Tick 1: the riser instant-builds, then the trigger (later
        // slot) pokes life 2 and despawns — retail's ordering (§6).
        w.tick(away(), PlayerCommand::default());
        assert_eq!(w.g.ent[1].act_life, 2, "trigger poked LOWER");
        assert_eq!(w.g.ent[2].class64, 0, "one-shot trigger despawned");
        for _ in 0..50 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.g.ent[1].act_life, 4, "idle-removed");
        // Interior col 104 sank to the flank average and was re-typed
        // from the south flank (row 103: type 5).
        assert_eq!(w.g.t.height[t(104, 100)], 100);
        assert_eq!(w.g.t.height[t(104, 101)], 100);
        assert_eq!(w.g.t.tile_type[t(104, 100)], 5, "flank terrain restored");
        assert_eq!(w.g.t.shading[t(104, 100)], 32);
        // The instant-built END never animates: still ridge at +48.
        assert_eq!(w.g.t.height[t(100, 100)], 148);
        assert_eq!(w.g.t.tile_type[t(100, 100)], 8);
        // RAISE trigger: the full animation re-runs.
        let (x, y) = mc2_pos(100, 100);
        let gz = w.g.ground_z(x, y) as i16;
        w.g.mc2_spawn_riser_trigger(64, x, y, gz)
            .expect("raise trigger");
        for _ in 0..52 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(w.g.ent[1].act_life, 3, "idle-built again");
        assert_eq!(w.g.t.height[t(104, 100)], 148, "interior re-raised +48");
        assert_eq!(w.g.t.tile_type[t(104, 100)], 8, "interior re-stamped");
    }

    /// The authored ground mana economy: (10,39)/(10,58) THINGs both
    /// spawn model-39 spheres (CreateManaSphere_500C0 — the §8
    /// numbering note) carrying 512 / 2560 mana, unowned (neutral
    /// sprite family 52), persistent on the ground.
    #[test]
    fn mc2_authored_mana_spheres() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |slot, model, x: u16| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 10,
            model,
            x,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let things = vec![th(1, 39, 100), th(2, 58, 110)];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        for _ in 0..20 {
            w.tick(away(), PlayerCommand::default());
        }
        let balls: Vec<_> =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0)
                .collect();
        assert_eq!(balls.len(), 2, "both records live as model-39 spheres");
        let manas: Vec<i32> = balls.iter().map(|e| e.f140).collect();
        assert!(manas.contains(&512) && manas.contains(&2560), "{manas:?}");
        for e in &balls {
            assert_eq!(e.f144, 0, "unowned");
            assert!(
                (52..60).contains(&e.type86),
                "neutral 52-family sprite, got {}",
                e.type86
            );
        }
    }

    /// The class-15 spell jars: an authored swi_id-2 token (the
    /// self-replenishing pickup) grants its spell into the Phase-4.2
    /// bank on carpet overlap (sound 18) and drops a replacement in
    /// place; a swi_id-3 record parks in the junk state 253 and never
    /// collects (the shared class-12/15 spawn bump, EF:33209-17).
    #[test]
    fn mc2_spell_token_pickup_and_replenish() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |slot, model, x: u16, swi_id| Thing {
            slot,
            kind: ThingKind::Entity,
            class: 15,
            model,
            x,
            y: 100,
            dis_id: 0,
            swi_sz: 0,
            swi_id,
            parent: 0,
            child: 0,
            par3: None,
        };
        // Spell 4 as the live pickup; spell 7 authored junk (>= 3).
        let things = vec![th(1, 4, 100, 2), th(2, 7, 100, 3)];
        let mut w = World::new_for_game(planes, &things, 1, assets(), GameId::Mc2);
        assert_eq!(w.g.ent[1].tick70, 4 * 3 + 2, "state 3M+2");
        assert_eq!(w.g.ent[2].tick70, 253, "junk state");
        // Park the carpet straight over the token.
        let tz = w.g.ground_z(w.g.ent[1].x, w.g.ent[1].y) as i16;
        let pose = PlayerPose::level((100 << 8) | 128, (100 << 8) | 128, tz, 0);
        let mut sound18 = false;
        for _ in 0..8 {
            w.tick(pose, PlayerCommand::default());
            sound18 |= w.g.sounds.iter().any(|s| s.id == 18 && s.player);
            w.g.sounds.clear();
        }
        assert_ne!(
            w.g.mc2_spell_tokens.0 & (1 << 4),
            0,
            "spell 4 banked for Phase 4.2"
        );
        assert!(sound18, "the pickup chime at the collector");
        // The collected jar is KEPT as the wizard's manifestation
        // (state 3M, book-linked — the 4.2 slot economy); the
        // replacement sits in state 3M+2 at the same spot and is
        // NOT re-collected (the SpellEnabled gate).
        let live: Vec<_> =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 15 && e.model65 == 4 && e.flags & 0x400 == 0)
                .collect();
        assert_eq!(live.len(), 2, "manifestation + replacement jar");
        assert!(live.iter().any(|e| e.tick70 == 4 * 3 + 2), "replacement");
        assert!(live.iter().any(|e| e.tick70 == 4 * 3), "manifestation");
        assert_ne!(w.mc2_book.ent[4], 0, "spell 4 learned into the book");
        // The junk-state record never collects.
        for _ in 0..8 {
            w.tick(pose, PlayerCommand::default());
        }
        assert_eq!(w.g.mc2_spell_tokens.0 & (1 << 7), 0, "state 253 is inert");
    }

    /// With `prune_owned_jars` on, a class-15 spell token whose spell
    /// the player already owns self-culls (the SpellEnabled gate would
    /// otherwise leave it uncollectable). Both the level-load sweep and
    /// the post-gain sweep come free from the per-tick self-cull.
    /// Faithful default (off) keeps the token.
    #[test]
    fn mc2_owned_spell_tokens_are_pruned_when_enabled() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |model, swi_id| Thing {
            slot: 1,
            kind: ThingKind::Entity,
            class: 15,
            model,
            x: 200,
            y: 200,
            dis_id: 0,
            swi_sz: 0,
            swi_id,
            parent: 0,
            child: 0,
            par3: None,
        };
        // Count live spell-4 JARS only — the dev-grant/carry route
        // spawns a hidden MANIFESTATION (state 3M) that is not a jar.
        let live_tokens = |w: &World| {
            w.g.ent
                .iter()
                .filter(|e| {
                    e.class64 == 15 && e.model65 == 4 && e.flags & 0x400 == 0 && e.tick70 != 4 * 3
                })
                .count()
        };

        // A spell-4 token, carpet parked far away.
        let away = PlayerPose::level(10 << 8, 10 << 8, 3260, 0);

        // Faithful default: the uncollectable token stays put even
        // with the spell owned both ways (mask + book).
        let mut keep = World::new_for_game(planes.clone(), &[th(4, 2)], 1, assets(), GameId::Mc2);
        keep.g.mc2_spell_tokens.0 |= 1 << 4;
        keep.mc2_grant_plausible(&[(4, 0)]);
        for _ in 0..8 {
            keep.tick(away, PlayerCommand::default());
        }
        assert_eq!(live_tokens(&keep), 1, "off by default: token remains");

        // With the improvement on, the owned-spell token is removed.
        // Ownership arrives via the campaign-carry route
        // (`mc2_grant_plausible` → adopt): the XP BOOK is set but the
        // SpellEnabled mask stays at the level-start seed — the desync
        // that would make a mask-keyed prune a silent no-op.
        let mut prune = World::new_for_game(planes, &[th(4, 2)], 1, assets(), GameId::Mc2);
        prune.set_prune_owned_jars(true);
        prune.mc2_grant_plausible(&[(4, 0)]);
        assert_eq!(
            prune.g.mc2_spell_tokens.0 & (1 << 4),
            0,
            "campaign carry leaves the SpellEnabled mask clear"
        );
        for _ in 0..8 {
            prune.tick(away, PlayerCommand::default());
        }
        assert_eq!(live_tokens(&prune), 0, "the carried-spell jar self-culls");
    }

    /// `sub_65610`'s per-tick homing steers at the victim RAISED to
    /// its z-box CENTER (`sub_65580` EF:62750: z += f78 unless class
    /// 2), exactly like the acquisition sites — aiming at the origin
    /// z instead grazes under small high-altitude flyers. Also guards
    /// the FAITHFUL
    /// 3-D contact law end-to-end: a locked meteor must land ON the
    /// flyer, not fall through to terrain (retail's `sub_10630` box
    /// overlap — do NOT widen it; the homing z is the knob).
    #[test]
    fn mc2_meteor_homing_aims_at_target_box_center() {
        use crate::engine::features::Gen;
        use crate::mc1::mobs::MobCtx;

        let mut w = mc2_flat_world();
        let (tx, ty) = mc2_pos(110, 100);
        let ground = w.g.ground_z(tx, ty) as i16;
        // A wyvern-shaped stand-in: class-5 flyer 16 tiles up, awake,
        // target-eligible, with a tall-offset z-box (f78 = 200 makes
        // the origin-vs-center aim divergence visible past rounding).
        let t = w.g.new_event().unwrap();
        let tz = ground + 16 * 256;
        {
            let e = &mut w.g.ent[t];
            e.class64 = 5;
            e.model65 = 16;
            e.act_life = 60000;
            e.max_life = 60000;
            e.f58 = 64;
            e.flags |= 8;
            e.f28 = 1; // damage-channel mask (retail byte_0x38_56)
            e.f80 = 128;
            e.f82 = 128;
            e.f78 = 200;
            e.f84 = 300;
        }
        w.g.link(t, tx, ty, tz);
        // The meteor, LOCKED on the flyer (acquisition is faithful and
        // cone-gated — tested elsewhere), launched 10 tiles out at
        // carpet height, aimed dead-on at the box center.
        let (mx, my) = mc2_pos(100, 100);
        let mz = ground + 512;
        let m = w.g.mc2_spawn_meteor_shot(mx, my, mz).unwrap();
        let dh = Gen::isqrt(Gen::dist2_sq(mx, my, tx, ty) as u32) as i32;
        {
            let e = &mut w.g.ent[m];
            e.id24 = PLAYER_TARGET;
            e.f146 = t as u16;
            // The launcher's impact arm + tier fuse (spell 9 →
            // (10,17), charge).
            e.f68 = 10;
            e.f69 = 17;
            e.f44 = 6000;
            e.f71 = 2;
            e.f30 = Gen::angle_between(mx, my, tx, ty);
            e.f32 = Gen::pitch_toward(mz, tz + 200, dh);
        }
        let ctx = MobCtx {
            px: mx,
            py: my,
            pz: mz,
            pyaw: 0,
            pmana: 0,
        };
        // First homing tick: the steer target is the BOX CENTER.
        w.g.mc2_flyer_tick(m, &ctx);
        let center_aim = Gen::pitch_toward(mz, tz + 200, dh);
        let origin_aim = Gen::pitch_toward(mz, tz, dh);
        assert_ne!(center_aim, origin_aim, "geometry must discriminate");
        assert_eq!(w.g.ent[m].f36, center_aim, "homing aims at z + f78");
        // Fly it out: the meteor must die ON the flyer — at its z-box
        // CENTER (the sub_65580 raise-copy-restore at the victim-hit
        // relink), never on the ground below. The centered landing is
        // what puts the impact burst inside the victim's own area-
        // damage window.
        let mut landed = None;
        for _ in 0..60 {
            if w.g.ent[m].flags & 0x400 != 0 || w.g.ent[m].class64 != 9 {
                break;
            }
            w.g.mc2_flyer_tick(m, &ctx);
            let e = &w.g.ent[m];
            if (e.x, e.y, e.z) == (tx, ty, tz + 200) {
                landed = Some(true);
                break;
            }
            if e.z <= ground {
                landed = Some(false);
                break;
            }
        }
        assert_eq!(
            landed,
            Some(true),
            "locked meteor lands on the high flyer's box center"
        );
        // The impact burst must actually reach the flyer: tick the
        // spawned (10,17) burst and watch the damage mail drain life.
        let burst = (1..w.g.ent.len()).find(|&j| {
            let e = &w.g.ent[j];
            e.class64 == 10 && e.model65 == 17 && e.flags & 0x400 == 0
        });
        assert!(burst.is_some(), "the meteor impact spawned its burst");
        for _ in 0..6 {
            w.tick(PlayerPose::level(mx, my, mz, 0), PlayerCommand::default());
        }
        assert!(
            w.g.ent[t].act_life < 60000 || w.g.ent[t].mail[0].0 > 0,
            "the burst's area write reached the high flyer: life {} mail {:?}",
            w.g.ent[t].act_life,
            w.g.ent[t].mail[0]
        );
    }

    /// The player is a RAISED victim too: retail's local player is a
    /// boxed pool wizard and `sub_65580` lifts it like any other victim
    /// at both the homing aim and the victim-hit relink. The pose-only
    /// player's box center is pz + PLAYER_HH; aiming/landing at the raw
    /// pose z puts every pyramid attack at the player's FEET —
    /// undershooting the flying player into the terrain below.
    #[test]
    fn mc2_hostile_bolt_lands_at_the_player_box_center() {
        use crate::engine::features::Gen;
        use crate::mc1::combat::PLAYER_HH;
        use crate::mc1::mobs::MobCtx;

        let mut w = mc2_flat_world();
        let (px, py) = mc2_pos(110, 100);
        let ground = w.g.ground_z(px, py) as i16;
        let pz = ground + 16 * 256; // the player rides high
        // A pyramid-armed (9,0) bolt, 10 tiles out at launch height —
        // the doomsday case-1 payload (f44 800, impact (10,0), row
        // 62), locked on the player.
        let (bx, by) = mc2_pos(100, 100);
        let bz = ground + 768;
        let b = w.g.mc2_spawn_bolt(bx, by, bz).unwrap();
        let dh = Gen::isqrt(Gen::dist2_sq(bx, by, px, py) as u32) as i32;
        let center = Gen::pitch_toward(bz, pz + PLAYER_HH as i16, dh);
        {
            let e = &mut w.g.ent[b];
            e.f146 = PLAYER_TARGET;
            e.f44 = 800;
            e.f68 = 10;
            e.f69 = 0;
            e.row156 = 62;
            e.f66 = 3;
            e.f67 = 0;
            e.f30 = Gen::angle_between(bx, by, px, py);
            e.f32 = center;
            e.f36 = center;
        }
        let ctx = MobCtx {
            px,
            py,
            pz,
            pyaw: 0,
            pmana: 0,
        };
        // First homing tick: the steer target is the player's BOX
        // CENTER, not the pose z.
        w.g.mc2_flyer_tick(b, &ctx);
        let origin = Gen::pitch_toward(bz, pz, dh);
        assert_ne!(center, origin, "geometry must discriminate");
        assert_eq!(
            w.g.ent[b].f36, center,
            "homing aims at the player's box center"
        );
        // Fly it out: the bolt must die ON the player — relinked to
        // (px, py, pz + PLAYER_HH) — never on the ground below.
        let mut landed = None;
        for _ in 0..60 {
            if w.g.ent[b].flags & 0x400 != 0 || w.g.ent[b].class64 != 9 {
                break;
            }
            w.g.mc2_flyer_tick(b, &ctx);
            let e = &w.g.ent[b];
            if (e.x, e.y, e.z) == (px, py, pz + PLAYER_HH as i16) {
                landed = Some(true);
                break;
            }
            if e.z <= ground {
                landed = Some(false);
                break;
            }
        }
        assert_eq!(
            landed,
            Some(true),
            "the locked bolt lands at the player's box center"
        );
        // And the landing actually HURTS: the (10,0) burst's area
        // write drains the player's life through the world tick.
        // (Spawn grace wipes all player damage mail for the first
        // ~100 ticks — faithful :55367-71 — burn it off first.)
        assert!(
            (1..w.g.ent.len()).any(|j| {
                let e = &w.g.ent[j];
                e.class64 == 10 && e.model65 == 0 && e.flags & 0x400 == 0
            }),
            "the impact spawned its (10,0) fire burst"
        );
        w.player.grace = 0;
        let life0 = w.player.life;
        for _ in 0..8 {
            w.tick(PlayerPose::level(px, py, pz, 0), PlayerCommand::default());
        }
        assert!(
            w.player.life < life0,
            "the pyramid bolt damages the player: {} -> {}",
            life0,
            w.player.life
        );
    }

    /// The Phase-4.2 CAST COLUMN laws on a synthetic SPELLS table:
    /// the SetDefaultSpells baseline (fireball+possess, left/right),
    /// SetSpell tier wiring, the sub_5F660 gate + mana debit, the
    /// first-tick (9,0) spawn, and the sub_6D9C0 xpos1 level ladder
    /// with the selected-tier clamp.
    #[test]
    fn mc2_cast_column_laws() {
        let mut w = mc2_flat_world();
        // The campaign baseline (SetDefaultSpells_5C0A0).
        let v = w.mc2_book_view();
        assert!(v.owned[0] && v.owned[1], "fireball + possess granted");
        assert_eq!((v.left, v.right), (0, 1), "default hand binding");
        assert_eq!(v.xp[0], 0, "0 XP at init");
        // Synthetic table: fireball = 3 tiers, cost 100, duration 5,
        // payload 250, xpos1 ladder {0, 400, 12000} (the CD shape).
        let mut bytes = vec![0u8; 26 * 80];
        for (t, (cost, xp1, life)) in [(100i32, 0i32, 0i8), (250, 400, 1), (2500, 12000, 2)]
            .iter()
            .enumerate()
        {
            let b = 2 + 26 * t;
            bytes[b..b + 4].copy_from_slice(&250i32.to_le_bytes());
            bytes[b + 4..b + 8].copy_from_slice(&cost.to_le_bytes());
            bytes[b + 12..b + 16].copy_from_slice(&xp1.to_le_bytes());
            bytes[b + 22] = 5; // word_0x18 duration
            bytes[b + 24] = *life as u8;
        }
        bytes[0] = 3; // byte_0 = tier count
        w.g.assets.spells = crate::mc2::spells::parse(&bytes).unwrap();
        // Re-select tier 0 → SetSpell wires the manifestation.
        w.mc2_select_spell(0, 0, 0);
        let m = w.mc2_book.ent[0] as usize;
        assert_eq!(w.g.ent[m].f28, 5, "duration = word_0x18");
        assert_eq!(w.g.ent[m].max_life, 100, "full cost = tier manaCost");
        assert_eq!(w.g.ent[m].f30, 250, "payload = subSpellIndex");
        // Fire: the gate arms, the first effect tick spawns (9,0)
        // and debits the cost (the negative-delta stamp lands on the
        // NEXT tick's mana step).
        let pose = PlayerPose::level(100 << 8, 100 << 8, 3712, 0);
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        let mana0 = w.player.mana;
        assert!(mana0 >= 100, "regen gave enough for one cast");
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        w.tick(pose, fire);
        let balls =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 9 && e.model65 == 0 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(balls, 1, "the first cast tick spawned the fireball");
        let before = w.player.mana;
        w.tick(pose, PlayerCommand::default());
        assert!(
            w.player.mana < before || w.player.mana <= mana0,
            "the cast cost landed through the delta stamp"
        );
        // The XP ladder: +400 effective XP crosses tier-1 xpos1.
        w.mc2_award_xp(crate::mc1::mobs::PLAYER_TARGET, 0, 400);
        let v = w.mc2_book_view();
        assert_eq!(v.levels[0], 1, "xpos1 400 reached → level 1");
        assert_eq!(v.xp[0], 400);
        // Tier select clamps to the level.
        w.mc2_select_spell(0, 2, 0);
        assert_eq!(w.mc2_book_view().sel[0], 1, "selected tier ≤ level");
        // The cast window is still live (duration 5, two ticks
        // spent) — the change is PENDING (sub_6D880) and applies
        // when the timer expires.
        assert!(w.g.ent[m].f44 > 0, "tier change deferred mid-cast");
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        assert_eq!(w.g.ent[m].f71, 1, "live tier = clamped selection");
        assert_eq!(w.g.ent[m].max_life, 250, "tier-1 cost wired");
    }

    /// The all-spells (G) instrument keeps EVERY tier exercisable
    /// regardless of earned XP — the sim's select clamp must honour
    /// `dev_spells` the same way the app's pane does (main.rs:2035),
    /// else a higher tier selected in the pane still casts at tier 0.
    #[test]
    fn mc2_dev_spells_selects_any_tier() {
        let mut w = mc2_flat_world();
        // Synthetic 3-tier fireball with distinct per-tier costs so the
        // wired tier is readable off the manifestation.
        let mut bytes = vec![0u8; 26 * 80];
        for (t, (cost, life)) in [(100i32, 0i8), (250, 1), (2500, 2)].iter().enumerate() {
            let b = 2 + 26 * t;
            bytes[b + 4..b + 8].copy_from_slice(&cost.to_le_bytes());
            bytes[b + 24] = *life as u8;
        }
        bytes[0] = 3; // tier count
        w.g.assets.spells = crate::mc2::spells::parse(&bytes).unwrap();
        // 0 XP → earned level 0. Without the dev arm a tier-2 select
        // clamps to 0 (the bug); with it, the top tier wires through.
        w.set_dev_spells(true);
        w.mc2_select_spell(0, 2, 0);
        let m = w.mc2_book.ent[0] as usize;
        assert_eq!(
            w.mc2_book_view().sel[0],
            2,
            "the dev instrument selects the top tier at 0 XP"
        );
        assert_eq!(
            w.g.ent[m].f71, 2,
            "the manifestation is wired to tier 2, not clamped to level 0"
        );
        // A mid-play XP relevel must not yank the dev selection down.
        w.mc2_award_xp(crate::mc1::mobs::PLAYER_TARGET, 0, 0);
        assert_eq!(
            w.mc2_book_view().sel[0],
            2,
            "the XP relevel keeps the dev-selected tier"
        );
        // And faithful (non-dev) play still clamps to the earned level.
        let mut w2 = mc2_flat_world();
        w2.g.assets.spells = crate::mc2::spells::parse(&bytes).unwrap();
        w2.mc2_select_spell(0, 2, 0);
        assert_eq!(
            w2.mc2_book_view().sel[0],
            0,
            "without the dev toggle, tier stays clamped to XP level 0"
        );
    }

    /// The cast CADENCE law (docs/traces/mc2-cast-input.md §1-2):
    /// fire bits are EDGE-triggered — a held button gives ONE cast
    /// for a click tier, and auto-repeats only on a RAPID tier
    /// (`fontType_0x1B & 1`, e.g. Repeat Fireball). Plus the
    /// sub_67CB0 auto-aim: a cast with a creature in the cone locks
    /// and curves; the alarm-free straight flight otherwise.
    #[test]
    fn mc2_cast_cadence_and_autoaim() {
        let mut w = mc2_flat_world();
        // The load-time extents derivation (EF:44870-44910) with
        // uniform synthetic dims: speed_6 = rotSpeed_8 per row —
        // real collision boxes (the flat fixture has no baked
        // sprite index).
        w.g.assets.mc2_sprite_ext = crate::mc2::derive_sprite_extents(&[(32, 32); 400]);
        // Synthetic fireball row: tier 0 CLICK, tier 1 RAPID
        // (fontType 1), free-ish costs, duration 5.
        let mut bytes = vec![0u8; 26 * 80];
        for (t, font) in [0u8, 1, 0].iter().enumerate() {
            let b = 2 + 26 * t;
            bytes[b..b + 4].copy_from_slice(&250i32.to_le_bytes());
            bytes[b + 4..b + 8].copy_from_slice(&10i32.to_le_bytes());
            bytes[b + 12..b + 16].copy_from_slice(&(400 * t as i32).to_le_bytes());
            bytes[b + 22] = 5;
            bytes[b + 25] = *font;
        }
        bytes[0] = 3;
        w.g.assets.spells = crate::mc2::spells::parse(&bytes).unwrap();
        w.mc2_select_spell(0, 0, 0);
        let balls = |w: &World| {
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 9 && e.model65 == 0 && e.flags & 0x400 == 0)
                .count()
        };
        let pose = PlayerPose::level(100 << 8, 100 << 8, 3712, 0);
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        // CLICK tier: 8 held ticks = ONE cast (the press edge).
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        for _ in 0..8 {
            w.tick(pose, fire);
        }
        assert_eq!(balls(&w), 1, "click tier: one cast per press");
        // Release, unlock + select the RAPID tier, hold again.
        w.tick(pose, PlayerCommand::default());
        w.mc2_award_xp(crate::mc1::mobs::PLAYER_TARGET, 0, 400);
        w.mc2_select_spell(0, 1, 0);
        let before = balls(&w);
        for _ in 0..8 {
            w.tick(pose, fire);
        }
        assert!(
            balls(&w) > before + 1,
            "rapid tier: auto-repeat while held ({} -> {})",
            before,
            balls(&w)
        );
        // Auto-aim: a goat parked ahead-but-off-axis gets locked by
        // the next cast (sub_67CB0 offensive branch; heading 0 =
        // -y, the goat sits 4 tiles out, ~1 tile east).
        for _ in 0..30 {
            w.tick(pose, PlayerCommand::default());
        }
        // On-axis for the LEFT-hand muzzle (the launch sits a
        // 256-unit lateral step west of the pose): the faithful
        // first-tick turn is yaw-capped at 34 (EF:63108-13) with
        // 5/tick row-64 homing after — an off-axis goat grazes past
        // and ground-detonates — so the pin geometry puts the goat
        // straight down the muzzle line.
        let (gx, gy) = (99u16 << 8, (93 << 8) | 128u16);
        let gz = w.g.ground_z(gx, gy) as i16;
        let goat = w.g.mc2_spawn_goat(gx, gy, gz).expect("goat");
        // Low pass so the pitch bearing stays inside the 0x71 cone,
        // and a couple of ticks so the awake pre-pass arms the goat
        // (the `byte_0x39_57` acquisition gate).
        let pose = PlayerPose::level(100 << 8, 100 << 8, gz + 150, 0);
        for _ in 0..3 {
            w.tick(pose, PlayerCommand::default());
        }
        let before: Vec<usize> =
            w.g.ent
                .iter()
                .enumerate()
                .filter(|(_, e)| e.class64 == 9 && e.model65 == 0 && e.flags & 0x400 == 0)
                .map(|(j, _)| j)
                .collect();
        w.tick(pose, fire);
        w.tick(pose, PlayerCommand::default());
        let new_ball =
            w.g.ent
                .iter()
                .enumerate()
                .find(|(j, e)| {
                    e.class64 == 9 && e.model65 == 0 && e.flags & 0x400 == 0 && !before.contains(j)
                })
                .map(|(j, _)| j)
                .expect("the aimed cast spawned");
        assert_eq!(
            w.g.ent[new_ball].f146, goat as u16,
            "auto-aim locked the goat"
        );
        // The lock must CONNECT: the chord-marched probe (sub_10780's
        // cell walk) lands the hit, and the impact XP award fires
        // only on a victim strike (an end-point-only probe tunnels
        // through the zero-box fireball's targets).
        let xp0 = w.mc2_book_view().xp[0];
        for _ in 0..25 {
            w.tick(pose, PlayerCommand::default());
        }
        assert!(
            w.mc2_book_view().xp[0] > xp0,
            "the aimed fireball STRUCK (impact XP awarded)"
        );
        // The carried damage is the TIER's subSpellIndex (250 here,
        // matching retail's shipped value — EF:55864: the projectile
        // must not keep the new_event default 100). One strike's
        // (10,0) fire burns ≥ 250 off the goat
        // (docs/traces/mc2-fireball-damage.md; goat retail maxLife 600
        // → dead in 3).
        assert!(
            w.g.ent[goat].act_life <= 600 - 250,
            "the strike delivered the tier payload ({} left of 600)",
            w.g.ent[goat].act_life
        );
    }

    /// The GHOST FIREBALL guard: the G dev toggle must not grant the 24
    /// MC1 spells (auto-filling player.left with the MC1 fireball) and
    /// let the MC1 hand-cast arm run on the MC2 column — else EVERY MC2
    /// cast (heal, shield, ...) also launches an MC1 fireball. Both
    /// gates pinned here: grant_spell no-ops on MC2, and the MC1 cast
    /// arm is column-gated.
    #[test]
    fn mc2_dev_spells_cast_no_mc1_ghost() {
        let mut w = mc2_flat_world();
        let mut bytes = vec![0u8; 26 * 80];
        for sp in 0..26 {
            let b = 80 * sp + 2;
            bytes[b..b + 4].copy_from_slice(&250i32.to_le_bytes());
            bytes[b + 4..b + 8].copy_from_slice(&10i32.to_le_bytes());
            bytes[b + 22] = 5;
        }
        bytes[0] = 3;
        w.g.assets.spells = crate::mc2::spells::parse(&bytes).unwrap();
        // The trigger: the dev toggle (G) — must NOT materialize MC1
        // manifestations or bind the MC1 hands on the MC2 column.
        w.set_dev_spells(true);
        assert_eq!(w.player.left, None, "no MC1 hand bind on MC2");
        assert!(
            !w.g.ent
                .iter()
                .any(|e| e.class64 == 12 && e.flags & 0x400 == 0),
            "no MC1 class-12 manifestations on MC2"
        );
        let pose = PlayerPose::level(100 << 8, 100 << 8, 3712, 0);
        for _ in 0..4 {
            w.tick(pose, PlayerCommand::default());
        }
        // Select HEAL (spell 5, dev-granted) onto the LEFT hand, fire.
        w.mc2_select_spell(5, 0, 0);
        let fire = PlayerCommand {
            fire_left: true,
            ..Default::default()
        };
        w.tick(pose, fire);
        w.tick(pose, PlayerCommand::default());
        assert!(w.player.heal_active, "heal armed");
        assert!(
            !w.g.ent
                .iter()
                .any(|e| e.class64 == 9 && e.flags & 0x400 == 0),
            "a non-projectile MC2 cast spawns NO projectile (the ghost fireball)"
        );
    }

    // ---- Phase-4.3 MC2 multipart probes -------------------------------

    /// m3's ctor builds the 17-entity chain (head + 16 state-0xE8
    /// children on sprite rows 89+i, first link 25% longer), and the
    /// awake chain stretches out behind the head at the per-link
    /// follow distance.
    #[test]
    fn mc2_m3_chain_spawns_and_follows() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(102, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let head = w.g.mc2_spawn_m3(x, y, gz).expect("m3 spawns");
        // Chain topology: 16 children, forward links intact.
        let mut chain = vec![head];
        let mut j = w.g.ent[head].f52; // head keeps its own f52 (0)
        assert_eq!(j, 0);
        j = w.g.ent[head].f54;
        while j != 0 {
            chain.push(j as usize);
            j = w.g.ent[j as usize].f54;
        }
        assert_eq!(chain.len(), 17, "head + 16 children");
        for (ci, &c) in chain[1..].iter().enumerate() {
            assert_eq!(w.g.ent[c].tick70, 232, "children ride state 0xE8");
            assert_eq!(w.g.ent[c].type86, 89 + ci as u16, "sprite row 89+i");
            assert_eq!(
                w.g.ent[c].id24, w.g.ent[head].id24,
                "the qmemcpy keeps the head's id across the chain"
            );
        }
        // First link is 25% longer than its row metric — but the
        // particle rows' speed_6 column is zero (verified in the
        // pristine EXE too), which would collapse the whole chain onto
        // the head: zero-length links keep the head ctor's authored 96
        // (multipart.rs approximation; the true retail spacing source
        // is OPEN, banked with the disassembly-authors questions).
        let p89 = 65 * crate::mc2::sprite_params::SPRITE_PARAMS[89].speed_6 / 100;
        let link = if p89 == 0 { 96 } else { 125 * p89 / 100 };
        assert_eq!(w.g.ent[chain[1]].f56, link);
        assert_ne!(
            w.g.ent[chain[1]].f56, 0,
            "a zero link length re-blobs the worm"
        );
        // Awake (player at tile 100.5) the chain stretches behind
        // the head — consecutive links end up near their follow
        // distance apart.
        let player = at_trigger();
        for _ in 0..12 {
            w.tick(player, PlayerCommand::default());
        }
        let mut stretched = 0;
        for pair in chain.windows(2).take(6) {
            let (a, b) = (&w.g.ent[pair[0]], &w.g.ent[pair[1]]);
            let d = Gen::mc2_dist3((a.x, a.y, a.z), (b.x, b.y, b.z));
            if d >= b.f56 as u32 / 2 {
                stretched += 1;
            }
        }
        assert!(stretched >= 4, "the chain trails out ({stretched}/6 links)");
    }

    /// m0 head death: PreKillEntity cascades the kill state over the
    /// chain, and every member (children carry the bug-compatible
    /// 2250 mana each) converts within its 8-tick f63 stagger.
    #[test]
    fn mc2_m0_head_death_cascades_the_chain() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(102, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let head = w.g.mc2_spawn_m0(x, y, gz).expect("m0 spawns");
        assert_eq!(w.g.ent[head].f140, 4500 / 32, "the head-mana quirk");
        w.g.ent[head].f58 = 64;
        w.g.ent[head].act_life = -1;
        // Player far away — near, the (certified) collection loop
        // hoovers the drops before the assertion can see them.
        let player = away();
        for _ in 0..16 {
            w.tick(player, PlayerCommand::default());
        }
        let live_m0 =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 5 && e.model65 == 0 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(live_m0, 0, "the whole chain died with the head");
        assert!(
            w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0),
            "the corpses dropped mana spheres"
        );
    }

    /// m22's map spawn grows the (par1/2)-ring spiral tail (signed
    /// offsets ±1..±7, owner-colorized), and a lethal head write
    /// converts the entire chain to mana spheres (state 0xB5).
    #[test]
    fn mc2_m22_worm_tail_and_chain_kill() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(103, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let head = w.g.mc2_spawn_m22(x, y, gz, 15).expect("m22 spawns");
        assert_eq!(w.g.ent[head].z, gz + 384, "head floats at terrain+384");
        let mut offs = Vec::new();
        let mut j = w.g.ent[head].f54;
        while j != 0 {
            let s = &w.g.ent[j as usize];
            assert_eq!(s.tick70, 180, "tail segments ride state 0xB4");
            assert_eq!(s.f146, head as u16, "segments carry the head ref");
            offs.push(s.f71 as i8);
            j = s.f54;
        }
        assert_eq!(offs.len(), 14, "tail length 15 = 7 rings x 2");
        assert_eq!(offs[..4], [1, -1, 2, -2], "signed ring offsets");
        // Wild colorize: base row 52 + the D400C ramp (head = +7).
        assert_eq!(w.g.ent[head].type86, 59, "head at the ramp top");
        // The worm writhes awake, then a lethal external write flips
        // it to the chain-kill.
        let player = at_trigger();
        for _ in 0..8 {
            w.tick(player, PlayerCommand::default());
        }
        w.g.ent[head].act_life = -1;
        for _ in 0..4 {
            w.tick(player, PlayerCommand::default());
        }
        let live_m22 =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 5 && e.model65 == 22 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(live_m22, 0, "the chain-kill consumed the whole worm");
        assert!(
            w.g.ent
                .iter()
                .any(|e| e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0),
            "the head's stolen mana dropped as spheres"
        );
    }

    /// m27 spawns the full 51-entity tree (body gauge 5), branch
    /// damage is capped at 76/hit (regenerating limbs), and with the
    /// gauge at 0 a body hit exposes it into the death cascade —
    /// the whole chain hides and the 20000-mana body scatters.
    #[test]
    fn mc2_m27_branches_shield_the_body() {
        let mut w = mc2_flat_world();
        let (x, y) = mc2_pos(103, 100);
        let gz = w.g.ground_z(x, y) as i16;
        let body = w.g.mc2_spawn_m27(x, y, gz).expect("m27 spawns");
        let mut branches = Vec::new();
        let mut segs = 0;
        let mut j = w.g.ent[body].f54;
        while j != 0 {
            let e = &w.g.ent[j as usize];
            match e.tick70 {
                233 => branches.push(j as usize),
                234 => segs += 1,
                other => panic!("unexpected chain state {other}"),
            }
            j = e.f54;
        }
        assert_eq!((branches.len(), segs), (5, 45), "1+5+45 topology");
        assert_eq!(w.g.ent[body].f50, 5, "the live-branch gauge");
        assert_eq!(
            w.g.ent[branches[0]].act_life, 1380,
            "the branch life ladder"
        );
        // A huge hit on a branch lands capped at 76.
        w.g.ent[branches[0]].mail[0] = (10000, 77);
        let player = at_trigger();
        w.tick(player, PlayerCommand::default());
        assert_eq!(
            w.g.ent[branches[0]].act_life,
            1380 - 76,
            "branch damage caps at 76/hit"
        );
        assert_eq!(
            w.g.ent[body].act_life, 1_000_000,
            "the body tanks nothing while branches live"
        );
        // Gauge forced to 0: the next body hit exposes it (0xDC) and
        // the death cascade converts + hides the entire chain. The
        // player leaves first — near, the collection loop would
        // hoover the scatter before the assertion.
        w.g.ent[body].f50 = 0;
        w.g.ent[body].mail[0] = (500, 77);
        let player = away();
        for _ in 0..12 {
            w.tick(player, PlayerCommand::default());
        }
        let live_m27 =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 5 && e.model65 == 27 && e.flags & 0x400 == 0)
                .count();
        assert_eq!(live_m27, 0, "the exposed body took the chain down");
        // The fraction volley (16 balls of 1250) merges where the
        // scatter overlaps — the CARRIED MANA is what must survive.
        let ball_mana: i32 =
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0)
                .map(|e| e.f140)
                .sum();
        assert!(
            ball_mana >= 20000,
            "the body's 20000 mana lands in the scatter (got {ball_mana})"
        );
    }
}
