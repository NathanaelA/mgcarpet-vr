//! MC1 creature/scenery spawn handlers and (movement track) the mob
//! state machine — direct ports of remc1's per-model spawn functions.
//! All citations remc1 sub_main.cpp.
//!
//! Spawn dispatch in the original: `dword_96902[class].str_4` →
//! per-class tables str_254D48 (class 2), str_254B84 (class 3),
//! str_255478 (class 5). Every handler allocates from the shared event
//! pool ([`Gen::new_event`]) and rolls only the event's OWN LCG
//! (`rand_29799_4`, seeded `slot + global_rand` — the global stream is
//! read, never advanced, by spawning), so spawn randomness is
//! byte-faithful regardless of spawn order.
//!
//! Fidelity notes:
//! - Class-5 model 0's segment-mana write targets the HEAD (+140,
//!   :44644) where model 3's identical construct writes the SEGMENT
//!   (:44861) — ported literally from the decompile; flagged in
//!   docs/ROADMAP.md (mana-track concern only).
//! - The kraken head (m6) is linked into the tile chain twice
//!   (:45086-:45087); `link` guards on the placed flag in both the
//!   original and this port, so the second call is a no-op.

use crate::features::Gen;
use crate::mc1_behavior::{BEHAVIOR, BehaviorRow};
use crate::mc1_sprite_stats::SPRITE_STATS;
use crate::tables::{COS, SIN};

/// Sentinel chase-target slot for the player's carpet (the original
/// chases a class-3 pool entity; our player lives outside the pool).
pub(crate) const PLAYER_TARGET: u16 = 0xFFFF;

/// Per-tick context the creature handlers need: the player's position
/// in engine units (the wizard list of the original, reduced to the
/// one human player until AI wizards land).
#[derive(Debug, Clone, Copy)]
pub(crate) struct MobCtx {
    pub(crate) px: u16,
    pub(crate) py: u16,
    pub(crate) pz: i16,
}

/// Animation frame counts by sprite draw type (`byte_90AD8`, :2716):
/// the 2..=16 animation draw types carry their frame count in the
/// type itself; view-select types have 1.
pub(crate) const FRAME_COUNTS: [u8; 37] = [
    1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, //
    1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];

impl Gen {
    /// sub_36FA0_37360 (:43751): assign the sprite-stats type index and
    /// derive extents from the sprite's world size halves.
    pub(crate) fn set_sprite(&mut self, i: usize, t: u16) {
        let s = SPRITE_STATS[t as usize];
        let e = &mut self.ent[i];
        e.frame88 = 0;
        e.type86 = t;
        e.frames89 = FRAME_COUNTS
            .get(s.draw_type as usize)
            .copied()
            .unwrap_or(0);
        e.f78 = s.height / 2;
        e.f80 = s.width / 2;
        e.f82 = s.width / 2;
        e.f84 = s.height / 2;
    }

    /// sub_37130_374F0 (:43790): explicit extent override.
    pub(crate) fn extents(&mut self, i: usize, horiz: u16, vert: u16) {
        let e = &mut self.ent[i];
        e.f80 = horiz;
        e.f82 = horiz;
        e.f84 = vert;
    }

    /// RefillLife_36DE0_371A0 (:43701).
    pub(crate) fn refill_life(&mut self, i: usize) {
        self.ent[i].act_life = self.ent[i].max_life as i32;
    }

    /// The spawn facing draw shared by most models (:44751-:44755):
    /// `(lcg & 0x7FF) - 1`, written to +34/+30/+32.
    fn spawn_facing(&mut self, i: usize, f: u16) {
        let e = &mut self.ent[i];
        e.f34 = f;
        e.f30 = f;
        e.f32 = f;
        e.f36 = 0;
    }

    // ---- class 2: scenery (str_254D48, :4359) -----------------------------

    /// Class-2 spawn dispatch. All models set `+26 = slot % 11`.
    pub(crate) fn spawn_scenery(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        if model > 5 {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = model as u8;
            e.f26 = (i % 11) as i16;
        }
        match model {
            // sub_37BC0 (:44402): the tree. Four draws of the event LCG
            // in strict order: a discarded life roll, x jitter, y
            // jitter (±32 units), then the variant bit (83/84).
            0 => {
                let e = &mut self.ent[i];
                e.tick70 = 0;
                e.f28 = 1;
                let life = self.ent_rand(i) % 0x1388 + 2500;
                self.ent[i].act_life = life as i32; // clobbered by RefillLife below, as the original
                let jx = ((self.ent_rand(i) & 0x3F) as i32 - 32) as i16;
                let jy = ((self.ent_rand(i) & 0x3F) as i32 - 32) as i16;
                self.link(
                    i,
                    x.wrapping_add(jx as u16),
                    y.wrapping_add(jy as u16),
                    z,
                );
                self.refill_life(i);
                let t = if self.ent_rand(i) & 1 != 0 { 84 } else { 83 };
                self.set_sprite(i, t);
            }
            // sub_37CF0/37D70/37E00 (:44451-): clear flag bit 3.
            1 | 2 | 3 => {
                let e = &mut self.ent[i];
                e.flags &= !8;
                e.tick70 = [3, 6, 9][model as usize - 1];
                self.link(i, x, y, z);
                self.refill_life(i);
                self.set_sprite(i, [79, 39, 270][model as usize - 1]);
                if model == 2 {
                    self.extents(i, 1024, 1024);
                }
            }
            // sub_37E80/37EF0 (:44526-): both the type-48 marker stone.
            _ => {
                self.ent[i].tick70 = if model == 4 { 12 } else { 15 };
                self.link(i, x, y, z);
                self.refill_life(i);
                self.set_sprite(i, 48);
            }
        }
        Some(i)
    }

    // ---- class 3: balloons / castle (str_254B84, :4367) --------------------

    /// Class-3 spawn dispatch; models 4..=11 are the player-start
    /// position markers (no entity — handled by the app), 12+ nothing.
    pub(crate) fn spawn_class3(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        if model > 3 {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 3;
            e.model65 = model as u8;
        }
        match model {
            // sub_37820/sub_378A0 (:44180/:44201): the wizard carpet —
            // model 0 = the HUMAN player's entity (row 7), model 1 =
            // an AI wizard (row 8, re-sets +24 to its own slot). No
            // facing draw. Wizard AI/flight is the Phase-5 track —
            // level-authored ones stand and render.
            0 | 1 => {
                let e = &mut self.ent[i];
                e.tick70 = model as u8;
                e.max_life = 10000;
                e.f128 = 80;
                e.f28 = 29;
                e.row156 = if model == 0 { 7 } else { 8 };
                if model == 1 {
                    e.id24 = i as u16;
                }
                self.link(i, x, y, z);
                self.refill_life(i);
                self.set_sprite(i, 44);
            }
            // sub_37920 (:44229): the castle. Spawn position snaps to a
            // tile corner of even parity; +150/152 keep the snapped
            // position (the castle's anchor).
            2 => {
                let e = &mut self.ent[i];
                e.tick70 = 5;
                e.max_life = 40000;
                e.f26 = 0;
                e.f28 = 33;
                let mut tx = x >> 8;
                let ty = y >> 8;
                if (tx.wrapping_add(ty)) & 1 == 1 {
                    tx = tx.wrapping_add(1);
                }
                let (sx, sy) = (tx << 8, ty << 8);
                self.ent[i].dest_x = sx;
                self.ent[i].dest_y = sy;
                self.link(i, sx, sy, z);
                self.refill_life(i);
                self.set_sprite(i, 177);
            }
            // sub_37A00 (:44266).
            _ => {
                let e = &mut self.ent[i];
                e.tick70 = 7;
                e.max_life = 10000;
                e.f126 = 48;
                e.f136 = 10000;
                e.f140 = 0;
                e.f28 = 1;
                e.row156 = 9;
                self.link(i, x, y, z);
                self.refill_life(i);
                self.set_sprite(i, 169);
            }
        }
        Some(i)
    }

    // ---- class 5: creatures (str_255478, :4420) ----------------------------

    /// Class-5 spawn dispatch, models 0..=16 (17+ hit the table's null
    /// terminator — no spawn). Returns the head slot.
    pub(crate) fn spawn_creature(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        match model {
            0 => self.spawn_worm(0, x, y, z),
            3 => self.spawn_worm(3, x, y, z),
            6 => self.spawn_worm(6, x, y, z),
            1..=16 => self.spawn_simple_creature(model, x, y, z),
            _ => None,
        }
    }

    /// The single-entity creature spawns (:44664-:45640), one table
    /// row per model; the shared shape is NewEvent + state/speeds/life
    /// + mana + facing draw + bookkeeping + place + RefillLife +
    /// sprite/extents.
    fn spawn_simple_creature(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        // Per-model constants (sub_38270..sub_396E0):
        //   state, life, act_speed, max_speed, accel, row, f44,
        //   mana mode, facing mode, type pick, f58 mode, f26 override,
        //   extent override (128,128).
        struct C {
            state: u8,
            life: u32,
            act: i16,
            max: i16,
            accel: i16,
            row: u8,
            f44: u16,
        }
        let c = match model {
            1 => C { state: 7, life: 2000, act: 50, max: 100, accel: 16, row: 13, f44: 100 },
            2 => C { state: 13, life: 3000, act: 35, max: 70, accel: 30, row: 14, f44: 350 },
            4 => C { state: 25, life: 1000, act: 30, max: 30, accel: 0, row: 0, f44: 500 },
            5 => C { state: 31, life: 5000, act: 30, max: 30, accel: 3, row: 17, f44: 500 },
            7 => C { state: 43, life: 0, act: 20, max: 20, accel: 3, row: 19, f44: 500 },
            8 => C { state: 49, life: 10000, act: 40, max: 40, accel: 20, row: 20, f44: 1000 },
            // State 54, NOT 55 — breaks the 6n+1 family pattern (:45258).
            9 => C { state: 54, life: 1000, act: 20, max: 20, accel: 0, row: 21, f44: 500 },
            10 => C { state: 61, life: 2000, act: 60, max: 60, accel: 20, row: 22, f44: 500 },
            // State 66, NOT 67 (:45364).
            11 => C { state: 66, life: 20000, act: 60, max: 60, accel: 20, row: 23, f44: 500 },
            12 => C { state: 73, life: 1000, act: 40, max: 40, accel: 20, row: 10, f44: 500 },
            13 => C { state: 79, life: 1000, act: 40, max: 40, accel: 20, row: 10, f44: 500 },
            14 => C { state: 85, life: 1000, act: 40, max: 40, accel: 20, row: 10, f44: 500 },
            15 => C { state: 91, life: 1000, act: 30, max: 30, accel: 0, row: 24, f44: 500 },
            16 => C { state: 97, life: 100000, act: 60, max: 60, accel: 20, row: 25, f44: 500 },
            _ => return None,
        };
        let i = self.new_event()?;

        // Model 7 (sub_38C60 :45123): life and sprite alternate on the
        // per-model spawn ordinal's parity (sub_38C00 :45101).
        let ordinal = self.spawn_count[model as usize];
        let life = if model == 7 {
            if ordinal & 1 != 0 { 4000 } else { 2000 }
        } else {
            c.life
        };

        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = model as u8;
            e.tick70 = c.state;
            e.max_life = life;
            e.f126 = c.act;
            e.f128 = c.max;
            e.f130 = c.accel;
            e.f44 = c.f44;
            e.row156 = c.row;
            e.f66 = 3;
        }

        // Mana: most models sub_36F90 (+140 = life/2, :43741); the
        // m5 growth creature and the m12/13/14 villagers are explicit.
        match model {
            5 => {
                self.ent[i].f140 = 500;
                self.ent[i].f136 = 12000;
            }
            12 | 13 | 14 => self.ent[i].f140 = 0,
            15 => self.ent[i].f140 = 0,
            _ => self.ent[i].f140 = (life >> 1) as i32,
        }
        if model == 11 {
            // :45370: +136 = 2 * (+140).
            self.ent[i].f136 = 2 * self.ent[i].f140;
        }

        // Facing draw — the event LCG's first draw. m1/m15 draw
        // nothing (facing 0); m9 rolls % 0x832 (:45264).
        let facing = match model {
            1 | 15 => 0u16,
            9 => (self.ent_rand(i) % 0x832).wrapping_sub(1) as u16,
            _ => (self.ent_rand(i) & 0x7FF).wrapping_sub(1) as u16,
        };
        self.spawn_facing(i, facing);
        self.ent[i].f28 = 1;

        // Bookkeeping: +26 state timer, +63 = per-model spawn ordinal
        // (counter incremented), +58 scan phase from the behavior
        // row's word 26.
        let v26 = BEHAVIOR[c.row as usize].v_26;
        {
            let e = &mut self.ent[i];
            e.f26 = match model {
                9 => (i % 10) as i16 + 29,
                11 | 16 => 0,
                12 | 13 | 14 => 2,
                _ => (i % 100) as i16,
            };
            e.f63 = ordinal;
        }
        self.spawn_count[model as usize] = ordinal.wrapping_add(1);
        self.ent[i].f58 = match model {
            1 => (v26 & 0xFF) + 1,
            2 | 4 | 9 | 15 => v26 - (ordinal as i16 % v26) + 4,
            _ => 64,
        };
        if model == 15 {
            self.ent[i].flags |= 0x20000; // :45622, +18 |= 2
        }

        self.link(i, x, y, z);
        // m7 sets +12 = +8 inline instead of RefillLife (:45118) —
        // identical result.
        self.refill_life(i);

        // Sprite type; m13 draws the event LCG a second time here
        // (:45505): % 7 in 0..3 → 217, else 218.
        let t = match model {
            1 => 86,
            2 => 3,
            4 | 15 => 0,
            5 => 185,
            7 => {
                if ordinal & 1 != 0 { 85 } else { 199 }
            }
            8 => 47,
            9 => 220,
            10 => 208,
            11 => 200,
            12 => 221,
            13 => {
                if self.ent_rand(i) % 7 < 4 { 217 } else { 218 }
            }
            14 => 219,
            16 => 207,
            _ => unreachable!(),
        };
        self.set_sprite(i, t);
        if model == 7 {
            self.ent[i].f71 = if ordinal & 1 != 0 { 1 } else { 2 };
        }
        // All simple creatures override the horizontal extents to a
        // half-tile square (sub_37130(128,128)) — except m1.
        if model != 1 {
            self.extents(i, 128, 128);
        }
        Some(i)
    }

    // ---- movement core -----------------------------------------------------

    /// sub_11810 (:16879): terrain capability bit by the tile's type
    /// byte; a creature may stand on a tile iff its behavior row's
    /// v_20 mask has the bit set.
    fn cap_bit(&self, x: u16, y: u16) -> u32 {
        let t = self.t.tile_type[(((y >> 8) as usize) << 8) | (x >> 8) as usize];
        match t {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 0x10,
            5 => 0x20,
            8 => 0x100,
            9 => 0x200,
            10 => 0x100000,
            11 => 0x200000,
            12 => 0x400000,
            13 | 14 => 0,
            15..=20 | 28..=34 => 0x400,
            21 | 22 | 24 => 0x20000,
            23 => 0x40000,
            25 | 27 => 0x80000,
            26 => 0x10000,
            _ => 0x800000,
        }
    }

    /// sub_19650 (:21149): local roughness — max corner-height cross
    /// difference of the tile under the position, raw height bytes.
    fn roughness(&self, x: u16, y: u16) -> i32 {
        let (tx, ty) = ((x >> 8) as u8, (y >> 8) as u8);
        let h = |dx: u8, dy: u8| {
            self.t.height[(((ty.wrapping_add(dy)) as usize) << 8)
                | tx.wrapping_add(dx) as usize] as i32
        };
        let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
        (h00 + h01 - h10 - h11).abs().max((h00 + h10 - h01 - h11).abs())
    }

    /// sub_42000_42340 (:52576): altitude clamp toward the behavior
    /// band [ground+v_12, ground+v_10] with step v_14 (quarter step
    /// inside the band, hard floor below).
    fn alt_clamp(z: &mut i16, ground: i16, row: &BehaviorRow) {
        if *z > ground.wrapping_add(row.v_10) {
            *z = z.wrapping_add(row.v_14);
        } else if *z > ground.wrapping_add(row.v_12) {
            *z = z.wrapping_add((25 * row.v_14 as i32 / 100) as i16);
        }
        if *z < ground.wrapping_add(row.v_12) {
            *z = ground.wrapping_add(row.v_12);
        }
    }

    /// sub_41EC0_42200 (:52523): polar step — dist along (yaw, pitch)
    /// on the 16.16 sine tables; yaw 0 = -y (north), positive pitch
    /// steps downward (z -= dist·sin).
    fn polar_step(pos: &mut (u16, u16, i16), yaw: u16, pitch: u16, dist: i16) {
        if dist == 0 {
            return;
        }
        let yaw = (yaw & 0x7FF) as usize;
        let pitch = (pitch & 0x7FF) as usize;
        let (horiz, dz) = if pitch != 0 {
            (
                ((dist as i32 * COS[pitch]) >> 16),
                ((dist as i32 * SIN[pitch]) >> 16),
            )
        } else {
            (dist as i32, 0)
        };
        pos.2 = pos.2.wrapping_sub(dz as i16);
        pos.0 = pos.0.wrapping_add(((horiz * SIN[yaw]) >> 16) as u16);
        pos.1 = pos.1.wrapping_sub(((horiz * COS[yaw]) >> 16) as u16);
    }

    /// sub_42210 (:52652): angular distance on 11-bit angles.
    fn angdist(a: u16, b: u16) -> u16 {
        let d = a.wrapping_sub(b) & 0x7FF;
        if d > 1024 { 2048 - d } else { d }
    }

    /// sub_422A0_425E0 (:52689): rate-limited turn from `cur` toward
    /// `tgt`, capped at the row's v_2 (v_4 is passed but dead).
    fn turn_step(cur: u16, tgt: u16, cap: i16) -> i16 {
        if cur == tgt {
            return 0;
        }
        let d = Self::angdist(cur, tgt) as i16;
        let s = if tgt.wrapping_sub(cur) & 0x7FF <= 1024 { 1 } else { -1 };
        s * d.min(cap)
    }

    /// One candidate probe of the movement core: clamp + step from the
    /// current position, then the block test (terrain mask + local
    /// roughness; crossing into a new tile only).
    fn move_probe(&self, i: usize, yaw: u16, row: &BehaviorRow) -> Option<(u16, u16, i16)> {
        let e = &self.ent[i];
        let mut tmp = (e.x, e.y, e.z);
        let ground = self.ground_z(e.x, e.y) as i16;
        Self::alt_clamp(&mut tmp.2, ground, row);
        Self::polar_step(&mut tmp, yaw, 0, e.f126);
        if e.x >> 8 == tmp.0 >> 8 && e.y >> 8 == tmp.1 >> 8 {
            return Some(tmp); // same tile → free (:21225)
        }
        // sub_11640 mode 1: capability mask; then roughness < v_16.
        if self.cap_bit(tmp.0, tmp.1) & !row.v_20 != 0 {
            return None;
        }
        if self.roughness(tmp.0, tmp.1) >= row.v_16 as i32 {
            return None;
        }
        Some(tmp)
    }

    /// Movement core sub_196E0 (:21182): altitude clamp → polar step →
    /// wall rule with three retry headings (±341 ≈ ±60°, then
    /// reversed) — all four blocked kills the creature (life = -1,
    /// the emergent behavior the carpet inherits differently). Commits
    /// via move_relink, then turns toward +34 capped at v_2.
    fn creature_move(&mut self, i: usize) {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let v31 = self.ent[i].f30;
        let candidates = [
            v31,
            v31.wrapping_add(341) & 0x7FF,
            v31.wrapping_sub(341) & 0x7FF,
            v31.wrapping_add(1024) & 0x7FF,
        ];
        let mut committed = false;
        for (k, &yaw) in candidates.iter().enumerate() {
            if k > 0 {
                // Failed candidates leave +30 mutated (:21239).
                self.ent[i].f30 = yaw;
            }
            if let Some(tmp) = self.move_probe(i, yaw, row) {
                self.move_relink(i, tmp.0, tmp.1, tmp.2);
                committed = true;
                break;
            }
        }
        if !committed {
            self.ent[i].act_life = -1; // :21293
            return;
        }
        let e = &self.ent[i];
        let turn = Self::turn_step(e.f30, e.f34, row.v_2);
        self.ent[i].f30 = (self.ent[i].f30 as i32 + turn as i32) as u16 & 0x7FF;
    }

    // ---- the six state primitives (:21311-:21871) --------------------------

    /// Squared 2D distance in engine units (16-bit wrapping deltas).
    fn dist2_sq(ax: u16, ay: u16, bx: u16, by: u16) -> i32 {
        let dx = bx.wrapping_sub(ax) as i16 as i32;
        let dy = by.wrapping_sub(ay) as i16 as i32;
        dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
    }

    /// Pack scan (inside IDLE :21384 / asleep WANDER): nearest same-
    /// model packless creature within v_28² and the v_30 facing cone
    /// becomes the leader; state → base+3.
    fn pack_scan(&mut self, i: usize, base: u8) {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let (ex, ey, yaw, model) = (e.x, e.y, e.f30, e.model65);
        let r2 = (row.v_28 as i32) * (row.v_28 as i32);
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if j == i
                || c.class64 != 5
                || c.model65 != model
                || c.tick70 == 120
                || c.act_life < 0
                || c.f52 != 0
            {
                continue;
            }
            let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
            if d2 > r2 {
                continue;
            }
            if Self::angdist(yaw, Self::angle_between(ex, ey, c.x, c.y)) >= row.v_30 as u16 {
                continue;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j, d2));
            }
        }
        if let Some((j, _)) = best {
            self.ent[i].f52 = j as u16;
            self.ent[i].tick70 = base + 3;
        }
    }

    /// The awake WANDER's wizard scan against the player (list[20] of
    /// the original — AI wizards are a later track): v_28² range +
    /// v_30 cone.
    fn player_in_aggro_range(&self, i: usize, ctx: &MobCtx) -> bool {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let r2 = (row.v_28 as i32) * (row.v_28 as i32);
        Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) <= r2
            && Self::angdist(e.f30, Self::angle_between(e.x, e.y, ctx.px, ctx.py))
                < row.v_30 as u16
    }

    /// IDLE sub_19B10 (:21311): stationary; every v_26 ticks a pack
    /// scan. (The damage prologue reduces to the death check until
    /// combat lands.)
    fn mob_idle(&mut self, i: usize, base: u8) {
        if self.ent[i].act_life < 0 {
            self.ent[i].tick70 = base + 4;
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        if (self.ent[i].f63 as i16) % v26 == 0 {
            self.pack_scan(i, base);
        }
    }

    /// WANDER sub_19D70 (:21421): move every tick; every v_26 ticks
    /// the two-draw yaw jitter (:21506 — d1 picks the sign via % 157,
    /// d2's low byte + 85 the magnitude), then — ONLY WHEN AWAKE
    /// (:21514, `if (+58)`) — the wizard scan, falling back to the
    /// pack scan when no wizard is in range/cone. Asleep creatures
    /// never scan (getting this backwards is what let whole distant
    /// crowds pack up and ride the unbounded pack accel — the
    /// player-reported runaway worms/bees).
    fn mob_wander(&mut self, i: usize, base: u8, ctx: &MobCtx, scan: bool, aggro: bool) {
        if self.ent[i].act_life < 0 {
            self.ent[i].tick70 = base + 4;
            return;
        }
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return; // walled in — dies via the prologue next tick
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        if (self.ent[i].f63 as i16) % v26 == 0 {
            let d1 = self.ent_rand(i);
            let d2 = self.ent_rand(i);
            let mag = ((d2 & 0xFF) + 85) as i32;
            let sign = if d1 % 157 >= 79 { 1 } else { -1 };
            self.ent[i].f34 = ((self.ent[i].f34 as i32 + sign * mag) & 0x7FF) as u16;
            if scan && self.ent[i].f58 != 0 {
                if aggro && self.player_in_aggro_range(i, ctx) {
                    self.ent[i].f146 = PLAYER_TARGET;
                    self.ent[i].tick70 = base + 2;
                } else {
                    self.pack_scan(i, base);
                }
            }
        }
    }

    /// CHASE sub_1A120 (:21580): move; bearing to the target every 4th
    /// tick; every v_26 ticks drop back to WANDER when the 3D distance
    /// reaches v_28 (un-squared — asymmetric with the scan's entry
    /// test, verbatim). The attack call is the combat track — a no-op
    /// here, so chasers close in and shadow their target.
    fn mob_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        if self.ent[i].act_life < 0 {
            self.ent[i].tick70 = base + 4;
            return;
        }
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let tgt = self.ent[i].f146;
        let (tx, ty, tz) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz)
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0
                || self.ent[t].act_life < 0
            {
                self.ent[i].tick70 = base + 1; // target lost (:21658)
                return;
            }
            (self.ent[t].x, self.ent[t].y, self.ent[t].z)
        };
        let e = &self.ent[i];
        if e.f63 & 3 == 0 {
            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
        }
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        if (e.f63 as i16) % row.v_26 == 0 {
            let dz = tz.wrapping_sub(e.z) as i32;
            let sq = Self::dist2_sq(e.x, e.y, tx, ty).wrapping_add(dz.wrapping_mul(dz));
            if Self::isqrt(sq as u32) >= row.v_28 as u32 {
                self.ent[i].tick70 = base + 1;
            }
            // attackFn: combat track.
        }
    }

    /// PACK sub_1A390 (:21677): mirror the leader — follow its
    /// heading, join its hunts, chain to its leader — with same-model
    /// separation and a per-v_26 speed bump of the leader's accel.
    fn mob_pack(&mut self, i: usize, base: u8) {
        let l = self.ent[i].f52 as usize;
        if l == 0 {
            self.ent[i].tick70 = base + 1;
            return;
        }
        if self.ent[i].act_life < 0 {
            // Member death (:21746): the leader retargets the killer
            // (slot 0 without combat → back to wander next tick).
            self.ent[l].f146 = 0;
            self.ent[l].f52 = 0;
            self.ent[l].tick70 = base + 2;
            self.ent[i].tick70 = base + 4;
            return;
        }
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        if (e.f63 as i16) % row.v_26 != 0 {
            return;
        }
        // Only the follow cases (leader idling/wandering/packing) fall
        // through to separation + accel; joining a chase and the
        // default both RETURN (:21781, :21793 — running the accel on
        // those paths too was part of the runaway).
        match (self.ent[l].tick70 as i16) - base as i16 {
            0 | 1 => {
                let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                let (lx, ly) = (self.ent[l].x, self.ent[l].y);
                self.ent[i].f34 = Self::angle_between(ex, ey, lx, ly);
            }
            2 => {
                self.ent[i].f146 = self.ent[l].f146;
                self.ent[i].f52 = 0;
                self.ent[i].tick70 = base + 2;
                return;
            }
            3 => {
                // Leader is packing too: chain to the grand-leader.
                self.ent[i].f52 = self.ent[l].f52;
                let g = self.ent[i].f52 as usize;
                if g != 0 {
                    let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                    let (gx, gy) = (self.ent[g].x, self.ent[g].y);
                    self.ent[i].f34 = Self::angle_between(ex, ey, gx, gy);
                }
            }
            _ => {
                self.ent[i].f52 = 0;
                self.ent[i].tick70 = base + 1;
                return;
            }
        }
        // Separation (:21796): first same-model neighbor within a tile
        // square points us away from it.
        let e = &self.ent[i];
        let (ex, ey, id, model) = (e.x, e.y, e.id24, e.model65);
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 5 || c.model65 != model || c.tick70 == 120 || c.act_life < 0 {
                continue;
            }
            if c.id24 == id {
                continue;
            }
            let dx = (ex.wrapping_sub(c.x) as i16 as i32).abs();
            let dy = (ey.wrapping_sub(c.y) as i16 as i32).abs();
            if dx < 256 && dy < 256 {
                self.ent[i].f34 = Self::angle_between(c.x, c.y, ex, ey);
                break;
            }
        }
        // Catch-up (:21814): member speed = LEADER's speed + accel —
        // a bounded "fly slightly faster than the leader". NOTE the
        // remc1 source line reads `a1x->+126 += v3x->+130`, but the
        // decompiler's raw output preserved above it
        // (`v10 = v3x->+130 + v3x->+126`) shows the original computes
        // the sum of the LEADER's fields — the += is a maintainer
        // mis-fix, and porting it verbatim is exactly the
        // player-reported runaway worm/bee acceleration.
        self.ent[i].f126 = self.ent[l].f126.wrapping_add(self.ent[l].f130);
    }

    /// DEATH sub_1A6C0 (:21820): body segments become corpses, then
    /// self. (Kill credit is the combat track.)
    fn mob_death(&mut self, i: usize, base: u8) {
        let mut s = self.ent[i].f54 as usize;
        while s != 0 {
            self.ent[s].tick70 = base + 5;
            s = self.ent[s].f54 as usize;
        }
        self.ent[i].tick70 = base + 5;
    }

    /// CORPSE sub_1A800 (:21855), on every 8th phase tick: despawn.
    /// DEVIATION (mana track): the original first drops a class-10
    /// m39 mana ball (1 draw on our stream + 2 on the ball's) and a
    /// class-10 m1 bones pickup; both unported until mana mechanics.
    fn mob_corpse(&mut self, i: usize) {
        if self.ent[i].f63 & 7 == 0 {
            self.ent[i].flags |= 0x400;
        }
    }

    /// sub_42510_42850 (:52763): one animation-frame step; true =
    /// already finished (does not wrap).
    fn anim_advance(&mut self, i: usize) -> bool {
        if self.ent[i].frame88 >= self.ent[i].frames89 {
            true
        } else {
            self.ent[i].frame88 += 1;
            false
        }
    }

    // ---- model 9, the burrower (states 54/55, :23591-:23920) ---------------

    /// sub_1DD50 (:24255): the hidden-mound disguise.
    fn m9_disguise(&mut self, i: usize) {
        self.ent[i].f126 = self.ent[i].f128;
        self.set_sprite(i, 201);
        self.ent[i].f66 = 3;
        self.ent[i].f67 = 0xFF; // sModel = -1
        self.ent[i].f26 = 50;
        self.ent[i].f71 = 0;
    }

    /// Spawn state 54, sub_1CFF0 (:23591): the materialize sequence —
    /// the spawn form (type 220, the player's "blue flame") counts
    /// down, swaps to the 16-frame transform animation (type 237) at
    /// 17, steps its frames every other tick, then settles into the
    /// type-201 mound at state 55.
    fn m9_emerge(&mut self, i: usize) {
        let v1 = self.ent[i].f26;
        self.ent[i].f26 = v1.wrapping_sub(1);
        if v1 != 0 {
            if v1 == 17 {
                self.set_sprite(i, 237);
            } else if v1 - 1 < 16 && (v1 - 1) % 2 == 0 {
                self.anim_advance(i);
            }
        } else {
            self.m9_disguise(i);
            self.ent[i].tick70 = 55;
            self.ent[i].f26 = 400;
            self.ent[i].f71 = 0;
        }
    }

    /// Hidden state 55, sub_1D060 (:23627): the mound lurks — burrow
    /// timer (bury as type 245 when the countdown runs out and the
    /// player is away), burrow-walk + every v_26 a CASTLE hunt
    /// (nearest class-3 model-2; within its extent + v_28 → chase) or
    /// the standard yaw jitter. The buried mode's villager hunting
    /// (sub_1D6D0) is the AI track — buried mounds sit still here.
    fn m9_hidden(&mut self, i: usize, base: u8) {
        if self.ent[i].type86 == 202 {
            // Back from a chase: sub_1DA60's exit path restores the
            // mound (sub_1DD50).
            self.m9_disguise(i);
            self.ent[i].f26 = 400;
        }
        let v1 = self.ent[i].f26;
        if v1 > 0 {
            self.ent[i].f26 = v1 - 1;
            if v1 == 1 {
                // sub_1DD90: bury.
                self.set_sprite(i, 245);
                self.ent[i].f71 = 1;
            }
        }
        if self.ent[i].f71 != 0 {
            return; // buried (sub_1D6D0 — AI track)
        }
        if self.ent[i].f58 != 0 {
            self.ent[i].f26 = 400; // player near: stay surfaced
        }
        if self.ent[i].act_life < 0 {
            self.ent[i].tick70 = base + 4;
            return;
        }
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        if (self.ent[i].f63 as i16) % row.v_26 != 0 {
            return;
        }
        // Nearest castle (:23752 — the wizard list filtered to model
        // 2, id != own; unbounded radius).
        let e = &self.ent[i];
        let (ex, ey, ez, id) = (e.x, e.y, e.z, e.id24);
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 3 || c.model65 != 2 || c.act_life < 0 || c.id24 == id {
                continue;
            }
            let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j, d2));
            }
        }
        if let Some((j, _)) = best {
            let (cx, cy, cz) = (self.ent[j].x, self.ent[j].y, self.ent[j].z);
            self.ent[i].f34 = Self::angle_between(ex, ey, cx, cy);
            let dz = cz.wrapping_sub(ez) as i32;
            let sq = Self::dist2_sq(ex, ey, cx, cy).wrapping_add(dz.wrapping_mul(dz));
            let range = self.ent[j].f80 as u32 + row.v_28 as u32;
            if Self::isqrt(sq as u32) <= range {
                self.ent[i].f146 = j as u16;
                self.ent[i].tick70 = base + 2;
            }
        } else {
            let d1 = self.ent_rand(i);
            let d2 = self.ent_rand(i);
            let mag = ((d2 & 0xFF) + 85) as i32;
            let sign = if d1 % 157 >= 79 { 1 } else { -1 };
            self.ent[i].f34 = ((self.ent[i].f34 as i32 + sign * mag) & 0x7FF) as u16;
        }
    }

    /// Flyer altitude oscillator sub_1B120 (:22206) — model 0's
    /// wander/chase/pack wrappers; +26 doubles as vertical speed.
    fn flyer_bob(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        let e = &mut self.ent[i];
        e.z = e.z.wrapping_add(e.f26);
        e.f26 -= 5;
        if e.z < ground.wrapping_add(256) {
            e.f26 = 150;
        }
    }

    /// Body segment state 120, sub_19550 (:21107): rigid follow —
    /// awake segments sit at distance +56 behind their leader along
    /// the exact bearing (position derived from the leader every
    /// tick); asleep ones collapse onto it every 4th tick.
    fn segment_follow(&mut self, i: usize) {
        let l = self.ent[i].f52 as usize;
        if l == 0 || self.ent[l].class64 != 5 {
            self.ent[i].flags |= 0x400; // orphaned (sub_41E80)
            return;
        }
        let (lx, ly, lz) = (self.ent[l].x, self.ent[l].y, self.ent[l].z);
        if self.ent[i].f58 != 0 {
            let e = &self.ent[i];
            let yaw = Self::angle_between(e.x, e.y, lx, ly);
            // Vertical bearing sub_42180 (:52644).
            let dh = Self::isqrt(Self::dist2_sq(e.x, e.y, lx, ly) as u32) as i16;
            let pitch = Self::angle_of(e.z.wrapping_sub(lz), dh.wrapping_neg());
            self.ent[i].f30 = yaw;
            self.ent[i].f32 = pitch;
            let mut tmp = (lx, ly, lz);
            let d = self.ent[i].f56 as i16;
            Self::polar_step(&mut tmp, yaw, pitch, -d);
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
        } else if self.ent[i].f63 & 3 == 0 {
            self.move_relink(i, lx, ly, lz);
            self.ent[i].f30 = self.ent[l].f30;
        }
    }

    /// Model 15's grid-walker movement sub_20480 (:25906): every 8th
    /// phase tick a weighted 4-way heading vote (die on forbidden
    /// terrain); every 16th a lane snap to tile centers; same-model
    /// repulsion; then a gated move (aligned, or a 55% coin).
    /// The vote's 4-entry weight table lives at a code/data alias the
    /// decompile can't express (`*(_DWORD*)sub_1FF40`) — uniform
    /// weights stand in (identical draw count, so streams align;
    /// extract from the retail binary someday).
    fn grid_walk(&mut self, i: usize, base: u8) {
        const WEIGHTS: [u32; 4] = [16, 16, 16, 16];
        let row = BEHAVIOR[self.ent[i].row156 as usize];
        if self.ent[i].f63 % 8 == 0 {
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            if self.cap_bit(x, y) & !row.v_20 != 0 {
                self.ent[i].tick70 = base + 4; // state 94: die on bad ground
                return;
            }
            let v31 = self.ent[i].f30;
            let mut best_score = 1u32;
            for k in 0..4u16 {
                let cand = v31.wrapping_add(512 * k) & 0x7FF;
                let e = &self.ent[i];
                let mut tmp = (e.x, e.y, e.z);
                Self::polar_step(&mut tmp, cand, 0, 256);
                let r = self.ent_rand(i);
                let free = self.cap_bit(tmp.0, tmp.1) & !row.v_20 == 0;
                let score = (r % WEIGHTS[k as usize] + 2) * free as u32;
                if score > best_score {
                    best_score = score;
                    self.ent[i].f30 = cand;
                }
            }
        }
        let e = &self.ent[i];
        let mut tmp = (e.x, e.y, e.z);
        if e.f63 % 16 == 0 {
            match (e.f30.wrapping_sub(256) >> 9) & 3 {
                0 | 2 => tmp.1 = (tmp.1 & !255) + 128,
                _ => tmp.0 = (tmp.0 & !255) + 128,
            }
        }
        // Same-model repulsion (:25984).
        let (ex, ey, id, model) = (e.x, e.y, e.id24, e.model65);
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 5 || c.model65 != model || c.tick70 == 120 || c.act_life < 0 {
                continue;
            }
            if c.id24 == id {
                continue;
            }
            let dx = (ex.wrapping_sub(c.x) as i16 as i32).abs();
            let dy = (ey.wrapping_sub(c.y) as i16 as i32).abs();
            if dx < 256 && dy < 256 {
                self.ent[i].f34 = Self::angle_between(c.x, c.y, ex, ey);
                break;
            }
        }
        let e = &self.ent[i];
        let aligned = e.f34 == e.f30;
        if aligned || self.ent_rand(i) % 20 <= 10 {
            let e = &self.ent[i];
            let (yaw, speed) = (e.f30, e.f126);
            Self::polar_step(&mut tmp, yaw, 0, speed);
            let ground = self.ground_z(tmp.0, tmp.1) as i16;
            Self::alt_clamp(&mut tmp.2, ground, &row);
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
        }
    }

    // ---- dispatch -----------------------------------------------------------

    /// The awake pre-pass sub_54F80 (:64300), run before dispatch:
    /// awake creatures count down (+58, mirrored into their body
    /// segments); asleep ones re-arm to 16 (segments 18) when the
    /// player is within 24 tiles (2D dist² < 0x2400000).
    pub(crate) fn mob_awake_pass(&mut self, ctx: &MobCtx) {
        for i in 1..self.ent.len() {
            let e = &self.ent[i];
            if e.class64 != 5 || e.tick70 == 120 {
                continue;
            }
            if e.act_life < 0 {
                self.ent[i].f58 = 250;
                self.ent[i].f59 = 0;
                continue;
            }
            if e.f58 > 0 {
                self.ent[i].f58 -= 1;
                let v = self.ent[i].f58;
                let mut s = self.ent[i].f54 as usize;
                while s != 0 {
                    self.ent[s].f58 = v;
                    s = self.ent[s].f54 as usize;
                }
            } else if e.f59 > 0 {
                self.ent[i].f59 -= 1;
            } else if Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) < 0x240_0000 {
                self.ent[i].f58 = 16;
                self.ent[i].f59 = 0;
                let mut s = self.ent[i].f54 as usize;
                while s != 0 {
                    self.ent[s].f58 = 18;
                    s = self.ent[s].f54 as usize;
                }
            }
        }
    }

    /// Class-5 per-state dispatch (str_254DCC, :4687). Family blocks
    /// of 6 per model: base+0 IDLE, +1 WANDER, +2 CHASE, +3 PACK,
    /// +4 DEATH, +5 CORPSE; state 120 = body segment. Custom family
    /// behavior beyond movement (disguises, mana hunts, house
    /// building, ranged/teleport casters) is the AI/combat track —
    /// those states stand still here; every simplification is flagged
    /// in docs/ROADMAP.md.
    pub(crate) fn creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        let s = self.ent[i].tick70;
        if s == 120 {
            return self.segment_follow(i);
        }
        if s > 101 {
            return; // parked states (data10 = 0)
        }
        let base = s - s % 6;
        let model = s / 6;
        let role = s % 6;
        match role {
            4 => return self.mob_death(i, base),
            5 => return self.mob_corpse(i),
            _ => {}
        }
        // Model 6 forces its speed every movement tick (:23116).
        if model == 6 && role >= 1 {
            self.ent[i].f126 = 30;
        }
        match (model, role) {
            // -- idles --
            // m5's spawn state falls straight through to wander
            // (:22775); m9 = the materialize sequence; m11-15 idles
            // are custom/parked (m13/14/15 literal nops).
            (5, 0) => self.ent[i].tick70 = base + 1,
            (9, 0) => self.m9_emerge(i),
            (11..=15, 0) => {}
            (_, 0) => self.mob_idle(i, base),

            // -- wanders --
            (0, 1) => {
                self.mob_wander(i, base, ctx, true, true);
                self.flyer_bob(i);
            }
            (9, 1) => self.m9_hidden(i, base),
            (11, 1) => {} // caster-phase: stationary until AI lands
            (15, 1) => self.grid_walk(i, base),
            // Aggro through the standard wizard scan; m4/m8 scan but
            // their wizard branch is possession-gated (mana track),
            // and m5/m12/13/14/16's wanders replace the whole scan
            // block with custom hunts — jitter-walk only until those
            // land.
            (m, 1) => {
                let scan = !matches!(m, 5 | 12 | 13 | 14 | 16);
                let aggro = matches!(m, 1 | 2 | 3 | 6 | 7 | 10);
                self.mob_wander(i, base, ctx, scan, aggro);
            }

            // -- chases --
            (0, 2) => {
                self.mob_chase(i, base, ctx);
                self.flyer_bob(i);
            }
            (9, 2) => {
                // sub_1DA60's per-tick disguise (sub_1DCD0): the mound
                // pops up as the warrior form while it chases.
                if self.ent[i].type86 != 202 {
                    self.set_sprite(i, 202);
                }
                self.mob_chase(i, base, ctx);
            }
            (11, 2) | (12, 2) => {} // ranged caster / house approach: AI track
            (_, 2) => self.mob_chase(i, base, ctx),

            // -- packs --
            (0, 3) => {
                self.mob_pack(i, base);
                self.flyer_bob(i);
            }
            (12, 3) | (13, 3) | (14, 3) => {} // villager house states
            (_, 3) => self.mob_pack(i, base),

            _ => unreachable!(),
        }
    }
    /// Multipart spawns — worms m0 (sub_38030 :44570) / m3 (sub_384B0
    /// :44799) with 16 segments, kraken m6 (sub_389E0 :45015) with 2.
    /// Segments are byte-copies of the head (inheriting its LCG state,
    /// no draws of their own) at state 120, chained via +52 (toward
    /// head) / +54 (toward tail).
    fn spawn_worm(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        // :44586: all three guard on 16 free pool slots.
        if self.free.len() < 16 {
            return None;
        }
        let (state, max_speed, row, head_type): (u8, i16, u8, u16) = match model {
            0 => (1, 80, 12, 40),
            3 => (19, 64, 15, 88),
            _ => (37, 80, 18, 49),
        };
        let seg_count = if model == 6 { 2 } else { 16 };

        let head = self.new_event()?;
        let ordinal = self.spawn_count[model as usize];
        {
            let e = &mut self.ent[head];
            e.class64 = 5;
            e.model65 = model as u8;
            e.tick70 = state;
            e.max_life = 9000;
            e.f126 = 30;
            e.f128 = max_speed;
            e.f130 = 16;
            e.row156 = row;
            e.f66 = 3;
            e.f28 = 1;
            // sub_36F90 then the explicit pool writes (:44601-:44605):
            e.f136 = 4500;
            e.f140 = if model == 6 { 1500 } else { 2250 };
        }
        let facing = (self.ent_rand(head) & 0x7FF).wrapping_sub(1) as u16;
        self.spawn_facing(head, facing);
        let v26 = BEHAVIOR[row as usize].v_26;
        {
            let e = &mut self.ent[head];
            e.f26 = (head % 100) as i16;
            e.f63 = ordinal;
            e.f58 = if model == 6 { 64 } else { v26 - (ordinal as i16 % v26) + 4 };
            if model != 6 {
                e.f56 = 96;
            }
        }
        self.spawn_count[model as usize] = ordinal.wrapping_add(1);

        let mut prev = head;
        for si in 0..seg_count {
            let Some(seg) = self.new_event() else { break };
            // qmemcpy(seg, head, 164) — then the alloc identity fields
            // the copy must not clobber are re-established.
            let slot_id = seg as u16;
            self.ent[seg] = self.ent[head];
            self.ent[seg].id24 = slot_id;
            self.ent[seg].flags &= !4; // not yet placed
            self.ent[seg].f52 = prev as u16;
            self.ent[prev].f54 = seg as u16;
            self.ent[seg].f54 = 0;
            self.ent[seg].tick70 = 120;
            match model {
                // Decompile-literal m0 quirk (:44644): the write lands
                // on the HEAD's +140, not the segment's.
                0 => self.ent[head].f140 = self.ent[head].f136 / 32,
                3 => self.ent[seg].f140 = self.ent[head].f136 / 32,
                _ => self.ent[seg].f140 = self.ent[head].f136 / 3,
            }
            self.ent[seg].f63 = si as u8;
            let seg_type = match model {
                0 => 19 + si as u16,
                3 => 89 + si as u16,
                _ => {
                    if si == 0 { 50 } else { 193 }
                }
            };
            self.set_sprite(seg, seg_type);
            self.ent[seg].f56 = if model == 6 {
                4 * self.ent[seg].f80
            } else {
                self.ent[seg].f80
            };
            self.link(seg, x, y, z);
            self.refill_life(seg);
            prev = seg;
        }

        self.link(head, x, y, z); // m6 calls this twice; guarded no-op
        self.refill_life(head);
        self.set_sprite(head, head_type);
        Some(head)
    }
}
