//! MC2 class-2 scenery, Phase 4.3 — models 3..=8 and the tree
//! (2,0) lifespan/burn ticks that close the "class-2 tick column
//! inert" APPROX from Phase 3. Trace bank:
//! docs/traces/mc2-class5-m25-26-28-class2-treeburn.md (`EF:` =
//! remc2 EventsFunctions.cpp).
//!
//! DELIBERATE APPROXIMATIONS:
//! - (10,5) water splash, (10,13) debris smoke and the (10,6) tree
//!   flame all ride their real ported creators (the (10,0) stand-in
//!   APPROX for the flame is CLOSED — effects.rs `mc2_spawn_fire6`,
//!   docs/traces/mc2-class10-m6-m9-m11-m28-m31.md §1).
//! - Models 7/8's terminal behavior is despawn (the trace CLOSED the
//!   "states 19/27" question — those were goto labels).
//! - Model 6 (cave bee) is cave-gated like m24: no cave levels boot
//!   yet, the ctor returns None (retail's own off-cave arm).

use crate::mc1::features::Gen;

impl Gen {
    // ---- ctors (models 3-8) --------------------------------------------------

    /// `sub_4AE80` (EF:33503) — static prop (2,3): action 9,
    /// half-speed sprite 270, non-collidable.
    pub(crate) fn mc2_spawn_scenery3(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = 3;
            e.tick70 = 9;
            e.flags &= !8;
            e.f26 = (i % 11) as i16;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 270);
        Some(i)
    }

    /// `sub_4AF00` / `sub_4AF70` (EF:33521/:33538) — pure statics
    /// (2,4)/(2,5): sprite 48, no-op ticks, collidable.
    pub(crate) fn mc2_spawn_scenery45(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = model;
            e.tick70 = if model == 4 { 12 } else { 15 };
            e.f26 = (i % 11) as i16;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 48);
        Some(i)
    }

    /// `sub_4AFE0` (EF:33555) — the cave bee (2,6): CAVE-ONLY; no
    /// cave levels boot yet (Phase 4.5) so the gate returns None,
    /// exactly retail's off-cave arm. (4 RNG draws when live: life
    /// 100..179, x/y jitter, sprite 324..327.)
    pub(crate) fn mc2_spawn_cave_bee(&mut self, _x: u16, _y: u16, _z: i16) -> Option<usize> {
        None
    }

    /// `sub_4B150` (EF:33608) — the falling scenery (2,7)/(2,8):
    /// burnable physics props with gravity. THREE RNG draws (life
    /// 400..2447, x/y jitter).
    pub(crate) fn mc2_spawn_falling(&mut self, model: u8, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = model;
            e.tick70 = if model == 7 { 20 } else { 21 };
            e.f56 = 1; // burnable
            e.f28 = 1; // cross-column damage contract (ch0)
            e.f71 = 0;
            e.f44 = (-128i16) as u16; // word_0x2C_44: initial fall velocity
            e.f126 = 0;
        }
        let d = self.mc2_rand(i);
        self.ent[i].max_life = d % 0x7D0 + 400;
        let jx = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        let jy = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        self.link(i, x.wrapping_add(jx as u16), y.wrapping_add(jy as u16), z);
        self.refill_life(i);
        self.mc2_set_sprite(i, if model == 7 { 322 } else { 323 });
        Some(i)
    }

    // ---- ticks ---------------------------------------------------------------

    /// The tree damage inbox: MC2's area writers hit burnable
    /// class-2 entities through the same channel-0 mailbox our
    /// combat column already writes (`sub_11400`'s
    /// `(1 << ch) & byte_0x38_56` gate ≡ ch0 vs our f56 bit 0).
    fn mc2_scenery_hit(&mut self, i: usize) -> Option<(u32, u16)> {
        if self.ent[i].mail[0].1 == 0 {
            return None;
        }
        let (amt, src) = self.ent[i].mail[0];
        self.ent[i].mail[0].1 = 0;
        Some((amt, src))
    }

    /// Water check + splash despawn shared by the tree states
    /// (EF:62450-56): the real (10,5) splash (id inherited) + the
    /// despawn.
    fn mc2_scenery_water(&mut self, i: usize) -> bool {
        let (x, y, z, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        if self.cap_bit(x, y) == 1 {
            if let Some(s) = self.mc2_spawn_splash(x, y, z) {
                self.ent[s].id24 = id;
            }
            self.ent[i].flags |= 0x400;
            return true;
        }
        false
    }

    /// `AddTree02_00_64E20` (EF:62399) — the healthy tree: burn-hit
    /// intake; a lethal hit spawns the flame, re-seeds 130..189 burn
    /// life and advances to the burning state.
    pub(crate) fn mc2_tree_tick(&mut self, i: usize) {
        if let Some((amt, src)) = self.mc2_scenery_hit(i) {
            self.ent[i].act_life -= amt as i32;
            if self.ent[i].act_life < 0 {
                // The flame: the real (10,6) standing fire
                // (EF:62421-56 — id from the attacker, the
                // word_0x2C_44 = (3*fov)>>2 lift, re-seeded burn).
                let (x, y, z, fov) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z, e.f84)
                };
                let fz = if z > 128 { z - 128 } else { 0 };
                if let Some(f) = self.mc2_spawn_fire6(x, y, fz) {
                    self.ent[f].id24 = if (src as usize) < self.ent.len() && src != 0 {
                        self.ent[src as usize].id24
                    } else {
                        src
                    };
                    self.ent[f].f44 = (3 * fov) >> 2;
                    let d = self.mc2_rand(i);
                    let burn = (d % 0x3C + 130) as i32;
                    self.ent[f].act_life = burn;
                    self.ent[i].act_life = burn;
                } else {
                    let d = self.mc2_rand(i);
                    self.ent[i].act_life = (d % 0x3C + 130) as i32;
                }
                self.ent[i].tick70 = 1;
            }
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        self.mc2_scenery_water(i);
    }

    /// `sub_64F60` (EF:62462) — the burning tree: 1 life/tick; under
    /// 60 the charred sprite (83→226, 84→227) and the stump state.
    pub(crate) fn mc2_tree_burning_tick(&mut self, i: usize) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 60 {
            self.ent[i].tick70 = 2;
            let charred = match self.ent[i].type86 {
                83 => 226,
                84 => 227,
                other => other,
            };
            self.mc2_set_sprite(i, charred);
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        self.mc2_scenery_water(i);
    }

    /// `sub_64FF0` (EF:62500) — the charred stump: terminal, snap-z.
    pub(crate) fn mc2_tree_stump_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        self.mc2_scenery_water(i);
    }

    /// `sub_65110`-family (EF:62536) — statics that snap to terrain
    /// (models 1-3's shared shape; models 4/5 are the true no-ops).
    pub(crate) fn mc2_scenery_snap_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
    }

    /// `sub_652C0` (EF:62606) — the falling-physics prop (2,7)/(2,8):
    /// gravity (−24/tick clamped ±192), damage bounces it with THREE
    /// RNG draws, death → (10,13) gib (pending, misfit) + despawn,
    /// water → splash + despawn.
    pub(crate) fn mc2_falling_tick(&mut self, i: usize) {
        if self.ent[i].flags & super::mobs::F_STOP != 0 {
            self.ent[i].flags &= !super::mobs::F_STOP;
            return;
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        let mut pos = (x, y, self.ent[i].z);
        if pos.2 > ground {
            if self.ent[i].f126 != 0 {
                let (yaw, spd) = (self.ent[i].f30, self.ent[i].f126);
                Self::polar_step(&mut pos, yaw, 0, spd);
            }
        } else {
            self.ent[i].f126 = 0;
        }
        if self.ent[i].f126 > 0 {
            self.ent[i].f126 -= 1;
        }
        // Gravity (EF:62650-60).
        let mut v = self.ent[i].f44 as i16;
        v = (v - 24).clamp(-192, 192);
        self.ent[i].f44 = v as u16;
        pos.2 = pos.2.wrapping_add(v).max(ground);
        self.move_relink(i, pos.0, pos.1, pos.2);
        // Burn/impact intake (EF:62661-87).
        if let Some((amt, _src)) = self.mc2_scenery_hit(i) {
            if pos.2 <= ground {
                let kick = ((amt >> 2) as i16).clamp(2, 192);
                let d1 = self.mc2_rand(i);
                self.ent[i].f44 = ((d1 % kick as u32) as i16 + kick) as u16;
                let d2 = self.mc2_rand(i);
                self.ent[i].f126 = ((d2 % ((kick as u32 >> 1).max(1))) + 1) as i16;
                let d3 = self.mc2_rand(i);
                self.ent[i].f30 = (d3 & 0x7FF) as u16;
                self.ent[i].z = self.ent[i].z.wrapping_add(self.ent[i].f44 as i16);
            }
            self.ent[i].act_life -= amt as i32;
        }
        if self.ent[i].act_life < 0 {
            // The settled-debris smoke poof (EF:62688-91) — (10,13)
            // is the smoke puff, not a gib (trace headline 3).
            let (x, y, z) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            self.mc2_spawn_smoke_particle_for(13, x, y, z);
            self.ent[i].flags |= 0x400;
            return;
        }
        if self.ent[i].z <= ground {
            self.mc2_scenery_water(i);
        }
    }

    /// The MC2 class-2 tick column (replaces the Phase-3 inert
    /// hold): trees run the burn ladder, statics snap, falling props
    /// fall. Unknown states hold (authentic for the no-op slots).
    pub(crate) fn mc2_scenery_tick(&mut self, i: usize) {
        match (self.ent[i].model65, self.ent[i].tick70) {
            (0, 0) => self.mc2_tree_tick(i),
            (0, 1) => self.mc2_tree_burning_tick(i),
            (0, 2) => self.mc2_tree_stump_tick(i),
            (1..=3, _) => self.mc2_scenery_snap_tick(i),
            (6, 18) => {} // cave bee — Phase 4.5
            (7 | 8, _) => self.mc2_falling_tick(i),
            _ => {} // models 4/5: the authentic no-op ticks
        }
    }
}
