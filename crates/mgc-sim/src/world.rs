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
//! - **Spawned drawables** (classes 2/3/5/12) become inert pool events
//!   (no AI/movement yet — the mobs track) mirrored into a live-things
//!   list the app resolves to billboards/map dots exactly like
//!   load-time entities.
//!
//! Deliberate deviations, tracked in docs/ROADMAP.md: no AI wizard
//! balloons (the probe list is the player alone); the player's AABB is
//! a point (the original balloon carries a small extent); class-12
//! pickup/mana transfer is NOT ported (jars/mana spawn and render;
//! collection is the mana track — its machinery routes through owner
//! blocks and class-9 carrier effects); damage broadcasts and sounds
//! omitted as in the load-time pass.

use crate::features::{
    self, FeatureAssets, Gen, Planes, Rec, TerrainPlanes, build_table, lcg32,
};
use mgc_formats::{Thing, ThingKind};

/// The player's pose in engine units for trigger tests: x/y are 8.8
/// fixed-point tile coordinates, z is altitude in engine units
/// (256 = one tile of height, i.e. 32 per height byte).
#[derive(Debug, Clone, Copy)]
pub struct PlayerPose {
    pub x: u16,
    pub y: u16,
    pub z: i16,
}

impl PlayerPose {
    /// From world-space tile floats (the flyer's coordinates).
    pub fn from_tiles(x: f32, y_alt: f32, z: f32) -> Self {
        let wrap = |v: f32| (v.rem_euclid(256.0) * 256.0) as u16;
        PlayerPose {
            x: wrap(x),
            y: wrap(z),
            z: (y_alt * 256.0) as i16,
        }
    }
}

/// The runtime world of one loaded MC1/HW level.
pub struct World {
    g: Gen,
    /// Live 1-based THING table; dispositions consume from it.
    table: Vec<Rec>,
    /// Drawable spawned entities: (pool slot, THING snapshot for the
    /// app's billboard/map-dot resolution).
    live: Vec<(u16, Thing)>,
    /// Terrain planes changed since last cleared (renderer re-upload).
    pub terrain_dirty: bool,
    /// Live entity set changed since last cleared.
    pub entities_dirty: bool,
}

/// Classes the app can draw (mc1_entities has a sprite mapping).
fn drawable(class: u16) -> bool {
    matches!(class, 2 | 3 | 5 | 12)
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
            live: Vec::new(),
            terrain_dirty: false,
            entities_dirty: false,
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

    /// Snapshot of the live drawable entities (kind = Entity), for the
    /// app's billboard/map-dot resolution.
    pub fn live_things(&self) -> Vec<Thing> {
        self.live.iter().map(|(_, t)| *t).collect()
    }

    /// One game turn (`sub_41780_41AC0`, :52197). `player` feeds the
    /// trigger volume probes.
    pub fn tick(&mut self, player: PlayerPose) {
        // One global LCG draw per tick, before any handler (:52223).
        lcg32(&mut self.g.rand);

        // Broad-phase bucket counts for the kill triggers: class-5
        // events by model, excluding state 120 (multipart body
        // segments in the original; :52246 list building).
        let mut buckets = [0u32; 20];
        for e in &self.g.ent {
            if e.class64 == 5 && e.act_life >= 0 && e.tick70 != 120 {
                buckets[(e.model65 as usize).min(19)] += 1;
            }
        }

        for i in 1..features::POOL {
            if self.g.ent[i].class64 == 0 {
                continue;
            }
            match self.g.ent[i].class64 {
                10 => {
                    // The load-time handlers ARE the runtime handlers.
                    self.g.tick(i);
                    self.terrain_dirty = true;
                }
                11 => self.trigger_tick(i, player, &buckets),
                // Creatures / scenery / effects / pickups: inert until
                // their tracks (AI, mana) land — they stand and render.
                _ => {}
            }
            // Per-tick phase counter, incremented after the state
            // handler (:52406); gates digger growth and probe cadence.
            self.g.ent[i].f63 = self.g.ent[i].f63.wrapping_add(1);
            if self.g.ent[i].flags & 0x400 != 0 {
                self.free_slot(i);
            }
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
            10 => self.g.spawn_creator(r.model, x, y, z),
            11 => self.spawn_trigger(r.model, x, y, z),
            2 | 3 | 5 | 7 | 9 | 12 => self.spawn_inert(r.class, r.model, x, y, z),
            _ => None,
        };
        let Some(s) = slot else { return };
        self.g.ent[s].thing_slot = ti as u16;

        // Post-init (:44017-44050).
        match (r.class, r.model) {
            (12, _) => {
                // byte70 += swi_id; >= 3 = the village-owned jar
                // variant (-3, sprite 280 — the app's Mana pick reads
                // the THING's swi_id for that).
                let e = &mut self.g.ent[s];
                e.tick70 = e.tick70.wrapping_add((r.swi_id & 0xFF) as u8);
                if r.swi_id >= 3 {
                    e.tick70 = e.tick70.wrapping_sub(3);
                }
            }
            (10, 4) => {
                self.g.ent[s].id24 = r.swi_id;
                self.set_extents(s, r.swi_sz << 8, r.swi_sz << 8);
            }
            (10, 45) => {
                self.g.building_fixup(s, r.parent.wrapping_add(16));
                self.g.ent[s].id24 = r.swi_id;
                self.set_extents(s, r.swi_sz << 8, 4096);
                self.refill_life(s);
                self.g.ent[s].flags |= 1;
            }
            (c, _) if c <= 11 => {
                self.g.ent[s].id24 = r.swi_id;
                self.set_extents(s, r.swi_sz << 8, 4096);
                self.refill_life(s);
                self.g.ent[s].flags |= 1;
            }
            _ => {}
        }

        if drawable(r.class) {
            self.live.push((
                s as u16,
                Thing {
                    slot: (ti as u32).saturating_sub(1),
                    kind: ThingKind::Entity,
                    class: r.class,
                    model: r.model,
                    x: r.x,
                    y: r.y,
                    dis_id: r.dis_id,
                    swi_sz: r.swi_sz,
                    swi_id: r.swi_id,
                    parent: r.parent,
                    child: r.child,
                    par3: None,
                },
            ));
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
        self.refill_life(s);
        Some(s)
    }

    /// A drawable/latent entity as an inert pool event (creatures get
    /// their real spawn handlers with the AI track; for now they
    /// occupy their pool slot, count in the kill buckets, and render).
    fn spawn_inert(&mut self, class: u16, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        let s = self.g.new_event()?;
        self.g.ent[s].class64 = class as u8;
        self.g.ent[s].model65 = model as u8;
        self.g.ent[s].tick70 = 0;
        self.g.link(s, x, y, z);
        self.refill_life(s);
        self.g.ent[s].flags |= 1;
        Some(s)
    }

    /// sub_37130_374F0 (:43790): square horizontal extent + vertical.
    fn set_extents(&mut self, s: usize, horiz: u16, vert: u16) {
        let e = &mut self.g.ent[s];
        e.f80 = horiz;
        e.f82 = horiz;
        e.f84 = vert;
    }

    /// RefillLife_36DE0_371A0 (:43701).
    fn refill_life(&mut self, s: usize) {
        self.g.ent[s].act_life = self.g.ent[s].max_life as i32;
    }

    fn free_slot(&mut self, i: usize) {
        self.g.free_entity(i);
        if let Some(pos) = self.live.iter().position(|&(s, _)| s as usize == i) {
            self.live.swap_remove(pos);
            self.entities_dirty = true;
        }
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

    /// sub_118C0 (:16963): |dx| < ex, |dy| < ey, |dz + zoff| < ez —
    /// the player as a point (extent 0).
    fn overlap(&self, i: usize, p: PlayerPose) -> bool {
        let e = &self.g.ent[i];
        let wrap_d = |a: u16, b: u16| {
            let d = (a as i32 - b as i32) & 0xFFFF;
            (d as i16 as i32).abs()
        };
        wrap_d(p.x, e.x) < e.f80 as i32
            && wrap_d(p.y, e.y) < e.f82 as i32
            && ((e.z as i32) - (p.z as i32)).abs() < e.f84 as i32
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
        PlayerPose::from_tiles(10.0, 105.0 / 8.0, 10.0)
    }

    fn at_trigger() -> PlayerPose {
        PlayerPose::from_tiles(100.5, 105.0 / 8.0, 100.5)
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
