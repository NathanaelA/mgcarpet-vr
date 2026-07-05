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
//! mana balls. The player casts the dev repeat-fireball through the
//! tick input (`PlayerCommand`) and is invincible via a permanent
//! spawn grace — mob damage lands in a discarded-but-totaled inbox.
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
use crate::mobs::{MobCtx, PLAYER_TARGET};
use mgc_formats::{Thing, ThingKind};

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
    /// Fire the dev repeat-fireball (hold-to-autofire; spell 23's
    /// input gate with the mana cost bypassed).
    pub fire: bool,
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
    /// The dev fireball's refire window (+48 of the spell event,
    /// sub_58240 :66295): held fire re-arms it every tick — one
    /// projectile per game tick, the repeat-fireball firehose.
    spell48: i16,
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
    matches!(class, 2 | 3 | 5 | 9 | 12)
        || (class == 10 && matches!(model, 34 | 0 | 1 | 5 | 23 | 25 | 39))
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
            spell48: 0,
        };
        w.fire_disposition(0, true);
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
                || (e.class64 == 10 && matches!(e.tick70, 0 | 1 | 5 | 17 | 23 | 25 | 41))
            {
                any_transient = true;
            }
        }

        // The dev repeat-fireball (spell 23, sub_58240 :66295): a held
        // button re-arms +48 every tick (:20627-30) so the spawn gate
        // `+48 == window` passes every tick; mana gates/deduction are
        // the bypassed cheat. The window tail still decrements.
        if cmd.fire {
            self.spell48 = 3;
        }
        if self.spell48 == 3 {
            self.cast_fireball(player);
        }
        if self.spell48 > 0 {
            self.spell48 -= 1;
        }

        // The awake pre-pass (sub_54F00, :64266) runs before dispatch.
        let ctx = MobCtx {
            px: player.x,
            py: player.y,
            pz: player.z,
        };
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
                // Combat effects (fire, spreader, splash, blast ring,
                // hit-flash, steal-flash, mana ball).
                10 if matches!(self.g.ent[i].tick70, 0 | 1 | 5 | 17 | 23 | 25 | 41) => {
                    if self.g.effect_tick(i, &ctx) {
                        self.terrain_dirty = true;
                    }
                }
                10 => {
                    // The load-time handlers ARE the runtime handlers.
                    self.g.tick(i);
                    self.terrain_dirty = true;
                }
                11 => self.trigger_tick(i, player, &buckets),
                // Scenery / pickups: inert until their tracks land —
                // they stand and render.
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

        // The invincible player: a spawn grace that never decrements
        // (:55367-71 — all six channels discarded each tick). The ch0
        // total is kept for display/tests.
        if self.g.player_mail[0].1 != 0 {
            self.g.player_damage += self.g.player_mail[0].0 as u64;
        }
        self.g.player_mail = [(0, 0); 6];
    }

    /// The fireball cast (sub_58240/sub_56090 :65056-83, gates and
    /// mana deduction bypassed): muzzle offset 256 units to the left
    /// hand, launch height = the carpet's half-height, heading/pitch
    /// from the pose, carpet speed inherited.
    fn cast_fireball(&mut self, p: PlayerPose) {
        use crate::combat::PLAYER_HH;
        let myaw = p.heading.wrapping_sub(512) & 0x7FF;
        let mut muzzle = (p.x, p.y, p.z);
        Gen::polar_step(&mut muzzle, myaw, 0, 256);
        if self.g.ground_z(muzzle.0, muzzle.1) as i16 > p.z {
            muzzle = (p.x, p.y, p.z); // muzzle inside terrain: revert
        }
        let z = p.z.wrapping_add(PLAYER_HH as i16);
        let Some(pr) = self.g.spawn_fireball(muzzle.0, muzzle.1, z) else {
            return;
        };
        let e = &mut self.g.ent[pr];
        e.f126 += p.speed; // inherits carpet speed (:65060)
        e.f128 = e.f126;
        e.id24 = PLAYER_TARGET;
        e.f30 = p.heading;
        e.f34 = p.heading;
        e.f32 = p.pitch;
        e.f36 = p.pitch;
        e.f44 = 50; // spell-row +44 (vestigial; the fire's 400 is real)
        e.f140 = 200; // deflection economics (repeat-fireball row)
        self.entities_dirty = true;
    }

    /// Total ch0 damage the invincible player has absorbed (what the
    /// original would have subtracted from your life).
    pub fn player_damage_taken(&self) -> u64 {
        self.g.player_damage
    }

    /// Combat stat counters: (kills, shots resolved, aimed hits) —
    /// the original's Type_160 +359/+343/+347.
    pub fn combat_stats(&self) -> (u32, u32, u32) {
        (self.g.kills, self.g.shots, self.g.hits)
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
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[7, 7, 7, 7]);
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

    #[test]
    fn fireball_kills_and_the_corpse_drops_a_mana_ball() {
        let mut w = bare_creature_world(2);
        assert_eq!(count(&w, 5, 2), 1, "the creature spawned");
        // Hold fire from the firing line: the aim assist locks on,
        // the fire's 400-damage broadcast whittles the 3000 life.
        let fire = PlayerCommand { fire: true };
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
        // A three-tick burst wounds the lunger without killing it
        // (≤ 1200 of 3000 life)...
        for _ in 0..3 {
            w.tick(firing_line(), PlayerCommand { fire: true });
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
        assert_eq!(count(&w, 5, 0), 17, "head + 16 segments");
        let fire = PlayerCommand { fire: true };
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

    #[test]
    fn deterministic_with_scripted_fire() {
        let run = || {
            let mut w = flat_world();
            for t in 0..400 {
                let p = if t < 16 { at_trigger() } else { firing_line() };
                let cmd = PlayerCommand { fire: (60..90).contains(&t) };
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
}

