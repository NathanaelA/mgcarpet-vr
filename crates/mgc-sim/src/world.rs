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
//! Deliberate deviations, tracked in docs/ROADMAP.md: no AI wizard
//! balloons (the probe/scan lists are the player alone); combat is
//! unported (chase closes in but the attack call is a no-op; damage
//! mailboxes unread); custom family behaviors beyond movement
//! (disguises, mana hunts, house building, ranged/teleport casters)
//! stand still pending the AI track; corpses despawn without dropping
//! mana balls/bones (mana track); class-12 pickup/mana transfer NOT
//! ported; damage broadcasts and sounds omitted as in the load-time
//! pass.

use crate::features::{
    self, FeatureAssets, Gen, Planes, Rec, TerrainPlanes, build_table, lcg32,
};
use crate::mc1_sprite_stats::SPRITE_STATS;
use crate::mobs::MobCtx;
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
}

impl PlayerPose {
    /// From world-space tile floats + yaw radians (the flyer's state).
    pub fn from_tiles(x: f32, y_alt: f32, z: f32, yaw: f32) -> Self {
        let wrap = |v: f32| (v.rem_euclid(256.0) * 256.0) as u16;
        PlayerPose {
            x: wrap(x),
            y: wrap(z),
            z: (y_alt * 256.0) as i16,
            heading: (yaw.rem_euclid(std::f32::consts::TAU) * (2048.0 / std::f32::consts::TAU))
                as u16
                & 0x7FF,
        }
    }
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
/// Class 10 is logic/terrain except the model-34 portal vortex.
fn drawable(class: u16, model: u16) -> bool {
    matches!(class, 2 | 3 | 5 | 12) || (class == 10 && model == 34)
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
            out.push(LivePose {
                class: e.class64,
                model: e.model65,
                type_index: e.type86,
                frame: e.frame88,
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                alt: e.z as f32 / 256.0,
                yaw: (e.f30 & 0x7FF) as f32 * (TAU / 2048.0),
                segment: e.class64 == 5 && e.tick70 == 120,
            });
        }
        out
    }

    /// One game turn (`sub_41780_41AC0`, :52197). `player` feeds the
    /// trigger volume probes, creature awake checks and aggro scans.
    pub fn tick(&mut self, player: PlayerPose) {
        // One global LCG draw per tick, before any handler (:52223).
        lcg32(&mut self.g.rand);

        // Broad-phase bucket counts for the kill triggers: class-5
        // events by model, excluding state 120 (multipart body
        // segments in the original; :52246 list building).
        let mut buckets = [0u32; 20];
        let mut any_creature = false;
        for e in &self.g.ent {
            if e.class64 == 5 && e.act_life >= 0 && e.tick70 != 120 {
                buckets[(e.model65 as usize).min(19)] += 1;
                any_creature = true;
            }
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
                10 if self.g.ent[i].tick70 == 36 => self.portal_tick(i, player),
                10 => {
                    // The load-time handlers ARE the runtime handlers.
                    self.g.tick(i);
                    self.terrain_dirty = true;
                }
                11 => self.trigger_tick(i, player, &buckets),
                // Scenery / effects / pickups: inert until their
                // tracks land — they stand and render.
                _ => {}
            }
            // Per-tick phase counter, incremented after the state
            // handler (:52406); gates digger growth and probe cadence.
            self.g.ent[i].f63 = self.g.ent[i].f63.wrapping_add(1);
            if self.g.ent[i].flags & 0x400 != 0 {
                self.free_slot(i);
            }
        }
        if any_creature {
            // Creatures move (or may): the pose consumer refreshes.
            self.entities_dirty = true;
        }
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
    /// logic, 9 = spell effects, 12 = mana pickups).
    fn spawn_inert(&mut self, class: u16, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.g.new_event()?;
        self.g.ent[s].class64 = class as u8;
        self.g.ent[s].model65 = model as u8;
        self.g.ent[s].tick70 = 0;
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
        PlayerPose::from_tiles(10.0, 105.0 / 8.0, 10.0, 0.0)
    }

    fn at_trigger() -> PlayerPose {
        PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5, 0.0)
    }

    #[test]
    fn deferred_things_stay_latent_until_triggered() {
        let mut w = flat_world();
        assert_eq!(w.live_things().len(), 0, "dis_id!=0 things must not spawn at init");
        for _ in 0..64 {
            w.tick(away());
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
            w.tick(at_trigger());
        }
        let live = w.live_things();
        assert_eq!(live.len(), 1, "the creature spawns via the disposition");
        assert_eq!((live[0].class, live[0].model), (5, 2));
        // The expanding crater digs -3 per covered ring per tick.
        for _ in 0..40 {
            w.tick(away());
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
            w.tick(at_trigger());
        }
        assert_eq!(w.live_things().len(), n, "one-shot trigger must not refire");
    }

    #[test]
    fn creatures_wander_when_awake() {
        let mut w = flat_world();
        // Fire the trigger so the (5,2) creature spawns; the player
        // stays nearby, keeping it awake.
        for _ in 0..16 {
            w.tick(at_trigger());
        }
        let start = w
            .live_poses()
            .into_iter()
            .find(|p| p.class == 5)
            .expect("creature spawned");
        for _ in 0..200 {
            w.tick(at_trigger());
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
        let p = PlayerPose::from_tiles(101.5, 14.0, 101.5, 0.0);
        for t in 0..400 {
            w.tick(p);
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

        let p = PlayerPose::from_tiles(101.5, 14.0, 101.5, 0.0);
        for _ in 0..60 {
            w.tick(p);
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
        let far = PlayerPose::from_tiles(10.0, 14.0, 10.0, 0.0);
        for _ in 0..3000 {
            w.tick(far);
        }
        let before: Vec<_> = w.live_poses();
        w.tick(far);
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
        let near = PlayerPose::from_tiles(102.5, 14.0, 102.5, 0.0);
        let mut seen = Vec::new();
        for _ in 0..80 {
            w.tick(near);
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
                w.tick(p);
            }
            (w.planes().height.clone(), w.live_things().len())
        };
        assert_eq!(run(), run());
    }
}

