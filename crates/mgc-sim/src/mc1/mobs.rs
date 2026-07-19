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

use crate::mc1::behavior::{BEHAVIOR, BehaviorRow};
use crate::mc1::combat::{Inbox, MailTarget};
use crate::mc1::features::Gen;
use crate::mc1::sprite_stats::SPRITE_STATS;
use crate::mc1::tables::{COS, SIN};

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
    /// The player's heading (the wizard entity's +30) — the genie's
    /// ambush blink (sub_1E770 :24733) lands ahead of the TARGET
    /// along the target's own yaw.
    pub(crate) pyaw: u16,
    /// The player's castable pool (+140) — the genie's mana hunt
    /// (:24523-46) takes the first wizard holding ANY mana.
    pub(crate) pmana: u32,
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
        e.frames89 = FRAME_COUNTS.get(s.draw_type as usize).copied().unwrap_or(0);
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
                self.link(i, x.wrapping_add(jx as u16), y.wrapping_add(jy as u16), z);
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
    /// row per model; the shared shape is NewEvent + state/speeds/life +
    /// mana + facing draw + bookkeeping + place + RefillLife +
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
            1 => C {
                state: 7,
                life: 2000,
                act: 50,
                max: 100,
                accel: 16,
                row: 13,
                f44: 100,
            },
            2 => C {
                state: 13,
                life: 3000,
                act: 35,
                max: 70,
                accel: 30,
                row: 14,
                f44: 350,
            },
            4 => C {
                state: 25,
                life: 1000,
                act: 30,
                max: 30,
                accel: 0,
                row: 0,
                f44: 500,
            },
            5 => C {
                state: 31,
                life: 5000,
                act: 30,
                max: 30,
                accel: 3,
                row: 17,
                f44: 500,
            },
            7 => C {
                state: 43,
                life: 0,
                act: 20,
                max: 20,
                accel: 3,
                row: 19,
                f44: 500,
            },
            8 => C {
                state: 49,
                life: 10000,
                act: 40,
                max: 40,
                accel: 20,
                row: 20,
                f44: 1000,
            },
            // State 54, NOT 55 — breaks the 6n+1 family pattern (:45258).
            9 => C {
                state: 54,
                life: 1000,
                act: 20,
                max: 20,
                accel: 0,
                row: 21,
                f44: 500,
            },
            10 => C {
                state: 61,
                life: 2000,
                act: 60,
                max: 60,
                accel: 20,
                row: 22,
                f44: 500,
            },
            // State 66, NOT 67 (:45364).
            11 => C {
                state: 66,
                life: 20000,
                act: 60,
                max: 60,
                accel: 20,
                row: 23,
                f44: 500,
            },
            12 => C {
                state: 73,
                life: 1000,
                act: 40,
                max: 40,
                accel: 20,
                row: 10,
                f44: 500,
            },
            13 => C {
                state: 79,
                life: 1000,
                act: 40,
                max: 40,
                accel: 20,
                row: 10,
                f44: 500,
            },
            14 => C {
                state: 85,
                life: 1000,
                act: 40,
                max: 40,
                accel: 20,
                row: 10,
                f44: 500,
            },
            15 => C {
                state: 91,
                life: 1000,
                act: 30,
                max: 30,
                accel: 0,
                row: 24,
                f44: 500,
            },
            16 => C {
                state: 97,
                life: 100000,
                act: 60,
                max: 60,
                accel: 20,
                row: 25,
                f44: 500,
            },
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
                if ordinal & 1 != 0 {
                    85
                } else {
                    199
                }
            }
            8 => 47,
            9 => 220,
            10 => 208,
            11 => 200,
            12 => 221,
            13 => {
                if self.ent_rand(i) % 7 < 4 {
                    217
                } else {
                    218
                }
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
    pub(crate) fn cap_bit(&self, x: u16, y: u16) -> u32 {
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
    pub(crate) fn roughness(&self, x: u16, y: u16) -> i32 {
        let (tx, ty) = ((x >> 8) as u8, (y >> 8) as u8);
        let h = |dx: u8, dy: u8| {
            self.t.height[(((ty.wrapping_add(dy)) as usize) << 8) | tx.wrapping_add(dx) as usize]
                as i32
        };
        let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
        (h00 + h01 - h10 - h11)
            .abs()
            .max((h00 + h10 - h01 - h11).abs())
    }

    /// sub_42000_42340 (:52576): altitude clamp toward the behavior
    /// band [ground+v_12, ground+v_10] with step v_14 (quarter step
    /// inside the band, hard floor below).
    pub(crate) fn alt_clamp(z: &mut i16, ground: i16, row: &BehaviorRow) {
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
    pub(crate) fn polar_step(pos: &mut (u16, u16, i16), yaw: u16, pitch: u16, dist: i16) {
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
    pub(crate) fn angdist(a: u16, b: u16) -> u16 {
        let d = a.wrapping_sub(b) & 0x7FF;
        if d > 1024 { 2048 - d } else { d }
    }

    /// sub_422A0_425E0 (:52689): rate-limited turn from `cur` toward
    /// `tgt`, capped at the row's v_2 (v_4 is passed but dead).
    pub(crate) fn turn_step(cur: u16, tgt: u16, cap: i16) -> i16 {
        if cur == tgt {
            return 0;
        }
        let d = Self::angdist(cur, tgt) as i16;
        let s = if tgt.wrapping_sub(cur) & 0x7FF <= 1024 {
            1
        } else {
            -1
        };
        s * d.min(cap)
    }

    /// One candidate probe of the movement core: clamp + step from the
    /// current position, then the block test (terrain mask + local
    /// roughness; crossing into a new tile only).
    fn move_probe(
        &self,
        i: usize,
        yaw: u16,
        row: &BehaviorRow,
        first: bool,
    ) -> Option<(u16, u16, i16)> {
        let e = &self.ent[i];
        let mut tmp = (e.x, e.y, e.z);
        let ground = self.ground_z(e.x, e.y) as i16;
        Self::alt_clamp(&mut tmp.2, ground, row);
        Self::polar_step(&mut tmp, yaw, 0, e.f126);
        // The same-tile shortcut applies ONLY to the first candidate
        // (:21225-30) — the three retry headings test the mask
        // unconditionally (:21252/:21274/:21291). This is what kills
        // a BEACHED KRAKEN (row 18's v_20 = water-only): terrain
        // raised under it → the next boundary crossing fails all
        // four candidates → life = -1. Extending the shortcut to
        // every candidate lets it bounce forever inside the land tile.
        if first && e.x >> 8 == tmp.0 >> 8 && e.y >> 8 == tmp.1 >> 8 {
            return Some(tmp);
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
            if let Some(tmp) = self.move_probe(i, yaw, row, k == 0) {
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

    /// The human player's commit gate sub_45410_45750 (:55065):
    /// type-8 wall tiles are horizontally impassable for the carpet at
    /// ANY altitude (`sub_11810 == 0x100` — only the wall type maps to
    /// exactly that mask; the human row 7 clears bit 0x100 while every
    /// flying creature row allows it). A blocked move retries along
    /// the two cardinals adjacent to the move bearing (floor, then
    /// ceil multiple of 512), each stepped from the CURRENT position
    /// scaled by angular proximity `dist·(512-Δ)>>9` — the original's
    /// wall slide; both blocked → the whole move is discarded (None).
    /// The routine's unconditional trailing z-floor (ground + row
    /// v_12) stays with the flyer's own clamp for now (Phase 5).
    ///
    /// CAVE ARM (MC2, Phase 4.5): sealed bit3 tiles block like walls
    /// — retail's MC2 commit gate refuses any move onto a sealed
    /// tile (`moveTest_5D0A0` EF:59594-97). The full headroom
    /// steer-search (EF:59515-93) belongs to the real MC2 commit
    /// gate (Phase 4.4); until then the MC1 cardinal slide stands in
    /// for the steer.
    pub(crate) fn player_wall_gate(
        &self,
        cur: (u16, u16, i16),
        prop: (u16, u16, i16),
    ) -> Option<(u16, u16, i16)> {
        let blocked = |x: u16, y: u16| {
            self.cap_bit(x, y) == 0x100
                || (self.is_cave()
                    && self.t.angle[crate::mc1::features::tile((x >> 8) as u8, (y >> 8) as u8)] & 8
                        != 0)
        };
        if !blocked(prop.0, prop.1) {
            return Some(prop);
        }
        let v1 = Self::angle_between(cur.0, cur.1, prop.0, prop.1);
        // sub_42340 (3D distance) and sub_42180 (vertical bearing).
        let dh2 = Self::dist2_sq(cur.0, cur.1, prop.0, prop.1);
        let dz = prop.2.wrapping_sub(cur.2) as i32;
        let v7 = Self::isqrt((dh2 as u32).wrapping_add((dz * dz) as u32)) as i32;
        let v8 = Self::pitch_toward(cur.2, prop.2, Self::isqrt(dh2 as u32) as i32);
        for cardinal in [(v1 >> 9) << 9, ((v1 >> 9).wrapping_add(1) << 9) & 0x7FF] {
            let scaled = (v7 * (512 - Self::angdist(v1, cardinal) as i32)) >> 9;
            let mut slid = cur;
            Self::polar_step(&mut slid, cardinal, v8, scaled as i16);
            if !blocked(slid.0, slid.1) {
                return Some(slid);
            }
        }
        None
    }

    // ---- the six state primitives (:21311-:21871) --------------------------

    /// Squared 2D distance in engine units (16-bit wrapping deltas).
    pub(crate) fn dist2_sq(ax: u16, ay: u16, bx: u16, by: u16) -> i32 {
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
        // Invisible (spell 12, :65689-90 — the +16 0x20 bit): the
        // wizard scan skips the cloaked player entirely.
        if self.player_invisible {
            return false;
        }
        // The +24 owner gate (sub_1DCD0 :24242 + the scan-side +24
        // exclusions): a creature the player OWNS (undead army
        // skeletons) never targets its owner.
        if self.ent[i].id24 == PLAYER_TARGET {
            return false;
        }
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let r2 = (row.v_28 as i32) * (row.v_28 as i32);
        Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) <= r2
            && Self::angdist(e.f30, Self::angle_between(e.x, e.y, ctx.px, ctx.py)) < row.v_30 as u16
    }

    /// IDLE sub_19B10 (:21311): stationary; every v_26 ticks a pack
    /// scan. (The damage inbox runs in `creature_tick` before
    /// dispatch, as the original's per-handler prologue.)
    fn mob_idle(&mut self, i: usize, base: u8) {
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        if (self.ent[i].f63 as i16) % v26 == 0 {
            self.pack_scan(i, base);
        }
    }

    /// WANDER sub_19D70 (:21421): move every tick; every v_26 ticks
    /// the two-draw yaw jitter (:21506 — d1 picks the sign via % 157,
    /// d2's low byte + 85 the magnitude), then — ONLY WHEN AWAKE
    /// (:21514, `if (+58)`) — the wizard scan (Scan A, the class-3
    /// hunt list :21519-42), falling back to the same-owner pack scan
    /// (Scan B :21546-73) when no wizard is in range/cone. EVERY
    /// awake creature runs both scans — the engine has no per-model
    /// aggro list; `aggro` exists only for m8's wanted-timer CHASE
    /// gate (sub_1CA50 :23500). Asleep creatures never scan (getting
    /// this backwards packs whole distant crowds up onto the unbounded
    /// pack accel).
    fn mob_wander(&mut self, i: usize, base: u8, ctx: &MobCtx, aggro: bool) {
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
            if self.ent[i].f58 != 0 {
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
    /// tick; every v_26 ticks either drop back to WANDER when the 3D
    /// distance reaches v_28 (un-squared — asymmetric with the scan's
    /// entry test, verbatim) or fire the per-model attack thunk
    /// (:21665-72). m6 arms a burst counter instead; the burst spawns
    /// run every tick while armed. (m2/m8/m11/m16 chase through their
    /// own wrappers/handlers above.)
    fn mob_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        let model = self.ent[i].model65;
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let tgt = self.ent[i].f146;
        // tf66/tf67 = the target's OWN filter fields (the player
        // entity keeps NewEvent's -1/-1): m6/m8 copy them onto their
        // beams (:23261-64, :22156-60) — hit-anything vs the player.
        let (tx, ty, tz, tf66, tf67) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, 0xFFu8, 0xFFu8)
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0 || self.ent[t].act_life < 0
            {
                self.ent[i].tick70 = base + 1; // target lost (:21658)
                return;
            }
            let c = &self.ent[t];
            (c.x, c.y, c.z, c.f66, c.f67)
        };
        let e = &self.ent[i];
        if e.f63 & 3 == 0 {
            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
        }
        // m6's buffet drag (:23215-31): the counter +26 cycles 1..41
        // then -90 — 41 ON ticks per 132-tick cycle. Each ON tick
        // re-arms the victim's knock fields (Type_160 v_24 dir /
        // v_22 = 80): a per-tick pull TOWARD the kraken, applied by
        // the human move. These are DIRECT struct writes, not a
        // mailbox — spawn grace does not shield them (the "tractor
        // beam"). v_26 = 256 is written but read by nothing.
        if model == 6 {
            // Retail compares the PRE-increment value (:23219-22), so
            // 41 is still an ON tick before the reset to -90.
            let old = self.ent[i].f26;
            self.ent[i].f26 = old + 1;
            if old > 40 {
                self.ent[i].f26 = -90;
            }
            if self.ent[i].f26 > 0 && tgt == PLAYER_TARGET {
                let (kx, ky) = (self.ent[i].x, self.ent[i].y);
                let dir = Self::angle_between(kx, ky, ctx.px, ctx.py).wrapping_add(0x400) & 0x7FF;
                self.player_knock = (dir, 80);
                // Victim buffet cue (:23223) — the kraken tether's
                // distinctive "resonance". Sound 42 has its OWN mixer
                // case (sub_55370 :64625, priority-1, same group as
                // 3/9/40/43); retail PLAYS it. (An earlier note here
                // wrongly claimed it hits the default-drop — corrected,
                // and the mixer policy now admits 42.)
                self.snd(42, i);
            }
        }
        // m6's spit burst (:23243-66): while +71 > 0, one lightning
        // beam per tick, filter copied from the target's own fields,
        // beam row [6] (:23259 — inert in flight, no homing).
        if model == 6 && self.ent[i].f71 > 0 {
            self.ent[i].f71 -= 1;
            let (x, y, z, owner, f84) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24, e.f84)
            };
            if let Some(p) = self.spawn_zigzag(x, y, z.wrapping_add(f84 as i16)) {
                self.arm_projectile(p, owner, tf66, tf67, tgt, tx, ty, tz, 800, 23);
                self.ent[p].row156 = 6;
            }
        }
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        if (e.f63 as i16) % row.v_26 == 0 {
            let dz = tz.wrapping_sub(e.z) as i32;
            let sq = Self::dist2_sq(e.x, e.y, tx, ty).wrapping_add(dz.wrapping_mul(dz));
            if Self::isqrt(sq as u32) >= row.v_28 as u32 {
                self.ent[i].tick70 = base + 1;
            } else if model == 6 {
                // Kraken: growl + arm the 5-bolt spit (:23240-42).
                // Sound 37 sits BEHIND the range gate — an out-of-range
                // cadence tick bails silent (goto LABEL_30, no growl).
                self.snd(37, i);
                self.ent[i].f71 = 5;
            } else {
                self.attack_thunk(i, model, tgt, tx, ty, tz, tf66, tf67);
            }
        }
    }

    /// m2's CHASE wrapper sub_1B3C0 (:22335): the sting cooldown +26
    /// counts down BEFORE the shared chase; the tick it expires the
    /// bee LUNGES at 3x max speed (:22346-47) — the retail
    /// "no-escape" burst. The sting itself (in the melee thunk)
    /// recoils and re-arms the cooldown; leaving the chase state
    /// resets speed to base (:22363-66).
    fn bee_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        let v1 = self.ent[i].f26;
        if v1 != 0 {
            self.ent[i].f26 = v1 - 1;
            if v1 == 1 {
                self.ent[i].f126 = 3 * self.ent[i].f128;
            }
        }
        self.mob_chase(i, base, ctx);
        if self.ent[i].tick70 != base + 2 {
            self.ent[i].f126 = self.ent[i].f128;
        }
    }

    /// m8's CHASE sub_1CE30 (:23546): restore full speed while the
    /// cooldown runs, re-set the DEFLECTION bit EVERY tick (:23552 —
    /// the only creature that raises it, and nothing ever clears it:
    /// fireballs/meteors bounce off an attacking griffon for good;
    /// beams — lightning — never full-deflect, which is why lightning
    /// stays the counter), then the shared chase with the 4000-damage
    /// beam thunk, plus the screech throttle (sound 38) every v_26
    /// (:23563-65). The provoking hit lands BEFORE the first chase
    /// tick sets the bit (the first meteor connects).
    fn griffon_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        if self.ent[i].f26 != 0 {
            self.ent[i].f126 = self.ent[i].f128;
        }
        self.ent[i].flags |= 0x8000;
        self.mob_chase(i, base, ctx);
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
        if (self.ent[i].f63 as i16) % v26 == 0 {
            self.snd(38, i);
        }
    }

    /// m16's CHASE sub_207E0 (:26062) — the wyvern's own handler, NOT
    /// the shared sub_1A120. Bearing every 8th tick, and only when
    /// the target is a wizard or beyond 0x200 3D (:26146-49 — over a
    /// house it stops re-aiming instead of orbiting); target
    /// dead/expired → back to the hunt (:26152); while the burst
    /// counter +26 runs, one strongly homing 3000-damage fireball PER
    /// TICK from 4x launch height with the wyvern's own +66/+67
    /// filter (:26154-77); every v_26 a SQUARED 2D range drop-out
    /// (unlike the shared chase's un-squared 3D test), the roar at
    /// 2*v_26 (sound 39) and the 0xE3-cone burst re-arm to 15
    /// (:26178-90).
    fn wyvern_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let tgt = self.ent[i].f146;
        let (tx, ty, tz, tclass, tdead) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, 3u8, false)
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0 {
                self.ent[i].tick70 = base + 1;
                return;
            }
            let c = &self.ent[t];
            (
                c.x,
                c.y,
                c.z,
                c.class64,
                c.act_life < 0 || c.flags & 0x400 != 0,
            )
        };
        if self.ent[i].f63 & 7 == 0 {
            let e = &self.ent[i];
            let dz = tz.wrapping_sub(e.z) as i32;
            let d = Self::isqrt(
                Self::dist2_sq(e.x, e.y, tx, ty).wrapping_add(dz.wrapping_mul(dz)) as u32,
            );
            if tclass == 3 || d >= 0x200 {
                let e = &self.ent[i];
                self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
            }
        }
        if tdead {
            self.ent[i].tick70 = base + 1;
            return;
        }
        if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1;
            let (x, y, z, owner, f84, f66, f67) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24, e.f84, e.f66, e.f67)
            };
            if let Some(p) = self.spawn_fireball(x, y, z.wrapping_add(4 * f84 as i16)) {
                self.ent[p].row156 = 2; // unk_98F38[2], turn 0x71
                self.ent[p].f140 = 60000;
                self.arm_projectile(p, owner, f66, f67, tgt, tx, ty, tz, 3000, 0);
            }
        }
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let (v26, v28) = (row.v_26.max(1), row.v_28 as i32);
        if (self.ent[i].f63 as i16) % v26 == 0 {
            let e = &self.ent[i];
            if Self::dist2_sq(e.x, e.y, tx, ty) >= v28 * v28 {
                self.ent[i].tick70 = base + 1;
                return;
            }
            if (self.ent[i].f63 as i16) % (2 * v26) == 0 {
                self.snd(39, i);
            }
            let e = &self.ent[i];
            let bearing = Self::angle_between(e.x, e.y, tx, ty);
            if Self::angdist(e.f30, bearing) < 0xE3 {
                self.ent[i].f26 = 15;
            }
        }
    }

    /// sub_20710's custom layer over the shared wander (:26033-58):
    /// every v_26+1 ticks (offset from the scan cadence) the nearest
    /// HOUSE (class-10 m45) within v_28² becomes the chase target —
    /// pure 2D nearest-in-radius, NO facing cone, NO invisibility
    /// gate. Wyverns wreck dwellings on sight.
    fn wyvern_house_hunt(&mut self, i: usize, base: u8) {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let period = row.v_26 + 1;
        if (self.ent[i].f63 as i16) % period != 0 {
            return;
        }
        let r2 = (row.v_28 as i32) * (row.v_28 as i32);
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 10 || c.model65 != 45 || c.flags & 0x400 != 0 || c.act_life < 0 {
                continue;
            }
            let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
            if d2 <= r2 && best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j, d2));
            }
        }
        if let Some((j, _)) = best {
            self.ent[i].f146 = j as u16;
            self.ent[i].tick70 = base + 2;
        }
    }

    // ---- model 11, the genie (states 66-71, :24317-24770) ------------------

    /// m11 IDLE sub_1DE40 (:24317) — the blink cycle. While +26 runs
    /// it counts down; on expiry (sound 21) the phase bit (+16
    /// byte[0] bit 0; ours flags 0x2000) picks the exit: SET → drop
    /// the target and TELEPORT by a per-axis LCG offset
    /// ((rand % 0x3C) << 8) + 12800 (toroidal map) into WANDER;
    /// CLEAR → straight into CHASE with the target intact. At +26 ==
    /// 0 it lays the 12-puff (10,1) sparkle ring on a 3x4 grid of
    /// 40-unit cells, re-arms +26 = 1 and toggles the phase — ring,
    /// then blink, alternating.
    fn genie_idle(&mut self, i: usize, base: u8) {
        let v1 = self.ent[i].f26;
        if v1 != 0 {
            self.ent[i].f26 = v1 - 1;
            if v1 == 1 {
                self.snd(21, i);
                if self.ent[i].flags & 0x2000 != 0 {
                    self.ent[i].f146 = 0;
                    let d1 = self.ent_rand(i);
                    let d2 = self.ent_rand(i);
                    let (x, y, z) = {
                        let e = &self.ent[i];
                        (e.x, e.y, e.z)
                    };
                    let nx = x.wrapping_add((((d1 % 0x3C) << 8) + 12800) as u16);
                    let ny = y.wrapping_add((((d2 % 0x3C) << 8) + 12800) as u16);
                    self.move_relink(i, nx, ny, z);
                    self.ent[i].tick70 = base + 1;
                } else {
                    self.ent[i].tick70 = base + 2;
                }
            }
        } else {
            // The sparkle ring (:24361-84); each puff carries the
            // genie as owner (+24) and the original's +18 bit 0.
            let (x, y, z, id) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24)
            };
            for k in (0..12u16).rev() {
                let px = x.wrapping_add(40 * (k % 3));
                let py = y.wrapping_add(40 * (k / 3));
                if let Some(p) = self.spawn_effect(1, px, py, z) {
                    self.ent[p].id24 = id;
                }
            }
            self.ent[i].f26 = 1;
            self.ent[i].flags ^= 0x2000;
        }
    }

    /// sub_1E770 (:24733): the AMBUSH BLINK — with a target held,
    /// zero the blink timer, drop to IDLE (whose ring/blink cycle
    /// alternates back into chase) and TELEPORT to the point one
    /// actSpeed<<6 step (60<<6 = 15 tiles) AHEAD of the target along
    /// the TARGET's own heading, at the target's altitude.
    fn genie_ambush(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        let tgt = self.ent[i].f146;
        if tgt == 0 {
            return;
        }
        let (tx, ty, tz, tyaw) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, ctx.pyaw)
        } else {
            let t = tgt as usize;
            if t >= self.ent.len() {
                return;
            }
            let c = &self.ent[t];
            (c.x, c.y, c.z, c.f30)
        };
        self.ent[i].f26 = 0;
        self.ent[i].tick70 = base;
        let mut pos = (tx, ty, tz);
        let step = ((self.ent[i].f126 as i32) << 6).clamp(i16::MIN as i32, i16::MAX as i32);
        Self::polar_step(&mut pos, tyaw, 0, step as i16);
        self.move_relink(i, pos.0, pos.1, pos.2);
    }

    /// sub_1E720 (:24724): blink home — clear the target and timer,
    /// back to IDLE, sound 11.
    fn genie_home(&mut self, i: usize, base: u8) {
        self.ent[i].f146 = 0;
        self.ent[i].f26 = 0;
        self.ent[i].tick70 = base;
        self.snd(11, i);
    }

    /// sub_1E810 (:24751): eat a loose mana ball — while below max
    /// mana, the nearest class-10 m39 ball within v_28² is absorbed
    /// (+140 += ball's, ball unclaimed + destroyed) with a (10,0)
    /// explosion puff at the spot and sound 11. The other half of
    /// "genies steal mana": they drain the map economy too.
    fn genie_eat_ball(&mut self, i: usize) {
        if self.ent[i].f140 >= self.ent[i].f136 {
            return;
        }
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let r2 = (row.v_28 as i32) * (row.v_28 as i32);
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 10 || c.model65 != 39 || c.flags & 0x400 != 0 {
                continue;
            }
            let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
            if d2 <= r2 && best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j, d2));
            }
        }
        if let Some((t, _)) = best {
            self.ent[i].f140 += self.ent[t].f140;
            self.ent[t].f144 = 0;
            self.ent[t].flags |= 0x400;
            let (bx, by, bz) = (self.ent[t].x, self.ent[t].y, self.ent[t].z);
            let id = self.ent[i].id24;
            if let Some(p) = self.spawn_effect(0, bx, by, bz) {
                self.ent[p].id24 = id;
            }
            self.snd(11, i);
        }
    }

    /// m11 WANDER sub_1DFE0 (:24388) — a full active handler, not a
    /// caster-phase nop: move, then every v_26 the SELF-HEAL
    /// (+maxLife>>6, clamped), the awake- and quarter-life-gated
    /// wizard scan (range v_28 + cone v_30 + invisibility, owner ≤ 1)
    /// → AMBUSH BLINK, else eat a mana ball; the standard two-draw
    /// yaw jitter; and above 3/4 life the MANA HUNT (:24523-46) — the
    /// first wizard holding ANY mana, no range or cone gate, →
    /// ambush. (The hit-retaliation lives in the central inbox.)
    fn genie_wander(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
        if (self.ent[i].f63 as i16) % v26 != 0 {
            return;
        }
        {
            let e = &mut self.ent[i];
            e.act_life += (e.max_life >> 6) as i32;
            if e.act_life < -1 {
                e.act_life = -1;
            }
            if e.act_life > e.max_life as i32 {
                e.act_life = e.max_life as i32;
            }
        }
        if self.ent[i].f58 != 0 && self.ent[i].act_life > (self.ent[i].max_life >> 2) as i32 {
            if self.player_in_aggro_range(i, ctx) {
                self.ent[i].f146 = PLAYER_TARGET;
                self.genie_ambush(i, base, ctx);
            } else {
                self.genie_eat_ball(i);
            }
        }
        let d1 = self.ent_rand(i);
        let d2 = self.ent_rand(i);
        let mag = ((d2 & 0xFF) + 85) as i32;
        let sign = if d1 % 157 >= 79 { 1 } else { -1 };
        self.ent[i].f34 = ((self.ent[i].f34 as i32 + sign * mag) & 0x7FF) as u16;
        let e = &self.ent[i];
        if e.act_life > (e.max_life - (e.max_life >> 2)) as i32
            && ctx.pmana != 0
            && !self.player_invisible
            && self.ent[i].id24 != PLAYER_TARGET
        {
            self.ent[i].f146 = PLAYER_TARGET;
            self.genie_ambush(i, base, ctx);
        }
    }

    /// m11 CHASE sub_1E380 (:24554): move; at or above half life the
    /// bearing update every 8th tick (target dead/expired → eat a
    /// ball + blink home); below half life the BREAK-OFF blink home.
    /// Every v_26: 3D range ≥ v_28 → blink home; the chatter (sound
    /// 11) every 8*v_26; else the 3000-payload steal seeker (the
    /// attack thunk; the +26 counter the original bumps per window is
    /// vestigial — both decompiled branches spawn identically).
    /// Deviation noted: the original falls through to the attack
    /// block even after blinking home, firing one stray seeker with
    /// the CLEARED target slot (a null-target quirk); we return.
    fn genie_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let tgt = self.ent[i].f146;
        let (tx, ty, tz, tdead) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, false)
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0 {
                self.genie_home(i, base);
                return;
            }
            let c = &self.ent[t];
            (c.x, c.y, c.z, c.act_life < 0 || c.flags & 0x400 != 0)
        };
        if self.ent[i].act_life >= (self.ent[i].max_life >> 1) as i32 {
            if !tdead {
                if self.ent[i].f63 & 7 == 0 {
                    let e = &self.ent[i];
                    self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                }
            } else {
                self.genie_eat_ball(i);
                self.genie_home(i, base);
                return;
            }
        } else {
            self.genie_home(i, base);
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
        if (self.ent[i].f63 as i16) % v26 == 0 {
            let e = &self.ent[i];
            let dz = tz.wrapping_sub(e.z) as i32;
            let sq = Self::dist2_sq(e.x, e.y, tx, ty).wrapping_add(dz.wrapping_mul(dz));
            if Self::isqrt(sq as u32) >= BEHAVIOR[self.ent[i].row156 as usize].v_28 as u32 {
                self.genie_home(i, base);
                return;
            }
            if (self.ent[i].f63 as i16) % (8 * v26) == 0 {
                self.snd(11, i);
            }
            self.ent[i].f26 += 1;
            self.attack_thunk(i, 11, tgt, tx, ty, tz, 0, 0);
        }
    }

    /// m5's regen tail (sub_1BF60/sub_1C110 :22959-65, :22976-82):
    /// life += maxlife>>7 per tick while below max.
    fn m5_regen(&mut self, i: usize) {
        let e = &mut self.ent[i];
        if e.act_life < e.max_life as i32 {
            e.act_life += (e.max_life >> 7) as i32;
        }
    }

    /// sub_38820_38BA0 (:44943): the crab GROWS — size = clamp(mana /
    /// (maxmana/8), 0, 7) picks sprite 185+size (extents follow the
    /// new sprite's stats); a size-up adds 5000 max life (unrefilled).
    fn m5_grow(&mut self, i: usize) {
        let e = &self.ent[i];
        let step = (e.f136 >> 3).max(1);
        let size = (e.f140 / step).clamp(0, 7) as i16;
        if size > e.type86 as i16 - 185 {
            self.ent[i].max_life += 5000;
        }
        self.set_sprite(i, (185 + size) as u16);
    }

    /// m5 WANDER sub_1BF60 (:22775): move (NO yaw-jitter draws — the
    /// crab's wander is a custom handler); every v_26: wizard scan →
    /// CHASE, else steer toward / close on the targeted mana ball
    /// (within maxSpeed<<7 → EAT state, +26 = 15), else acquire the
    /// nearest ball and lay an egg when 500 over max mana.
    fn m5_wander(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        self.creature_move(i);
        if self.ent[i].act_life >= 0 {
            let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
            if (self.ent[i].f63 as i16) % v26 == 0 {
                if self.player_in_aggro_range(i, ctx) {
                    self.ent[i].f146 = PLAYER_TARGET;
                    self.ent[i].tick70 = base + 2;
                } else if self.ent[i].f146 != 0 {
                    let t = self.ent[i].f146 as usize;
                    let is_ball = t < self.ent.len()
                        && self.ent[t].class64 == 10
                        && self.ent[t].model65 == 39
                        && self.ent[t].flags & 0x400 == 0;
                    if is_ball {
                        let e = &self.ent[i];
                        let (bx, by) = (self.ent[t].x, self.ent[t].y);
                        let d = Self::isqrt(Self::dist2_sq(e.x, e.y, bx, by) as u32);
                        if d > (e.f128 as u32) << 7 {
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, bx, by);
                        } else {
                            self.ent[i].f26 = 15;
                            self.ent[i].tick70 = base + 3; // EAT (state 0x21)
                        }
                    } else {
                        self.ent[i].f146 = 0;
                    }
                } else {
                    // Nearest loose ball, any range (:22928-45).
                    let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                    let mut best: Option<(usize, i32)> = None;
                    for j in 1..self.ent.len() {
                        let c = &self.ent[j];
                        if c.class64 != 10 || c.model65 != 39 || c.flags & 0x400 != 0 {
                            continue;
                        }
                        let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                        if best.is_none_or(|(_, bd)| d2 < bd) {
                            best = Some((j, d2));
                        }
                    }
                    if let Some((j, _)) = best {
                        self.ent[i].f146 = j as u16;
                    }

                    // Egg-laying (:22945-55): 500 over max mana buys a
                    // class-10 m52 egg (1 own-LCG draw). DEVIATION:
                    // the egg's hatch handler is unported — it stands
                    // one tick and despawns (flagged in ROADMAP).
                    if self.ent[i].f136 + 500 < self.ent[i].f140 {
                        let (x, y, z) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z)
                        };
                        if let Some(egg) = self.spawn_creator(52, x, y, z) {
                            let d = self.ent_rand(i);
                            self.ent[egg].f26 = (10 * (d % 10) + 100) as i16;
                            self.ent[i].f140 -= 500;
                        }
                    }
                }
            }
        }
        self.m5_regen(i);
    }

    /// m5 EAT sub_1C170 (:22986): close on the ball at the +26 think
    /// period (15, dropping to 3 inside 20·maxSpeed); within
    /// 5·maxSpeed: absorb its mana, destroy it, GROW, back to wander.
    fn m5_eat(&mut self, i: usize, base: u8) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let period = self.ent[i].f26.max(1);
        if (self.ent[i].f63 as i16) % period != 0 {
            return;
        }
        let t = self.ent[i].f146 as usize;
        let is_ball = t != 0
            && t < self.ent.len()
            && self.ent[t].class64 == 10
            && self.ent[t].model65 == 39
            && self.ent[t].flags & 0x400 == 0;
        if !is_ball {
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = base + 1;
            return;
        }
        let e = &self.ent[i];
        let (bx, by, bz) = (self.ent[t].x, self.ent[t].y, self.ent[t].z);
        let dz = bz.wrapping_sub(e.z) as i32;
        let d2 = Self::dist2_sq(e.x, e.y, bx, by).wrapping_add(dz.wrapping_mul(dz));
        let dist = Self::isqrt(d2 as u32);
        let max = self.ent[i].f128 as u32;
        if dist > 5 * max {
            if dist <= 20 * max {
                self.ent[i].f26 = 3;
            }
            let e = &self.ent[i];
            self.ent[i].f34 = Self::angle_between(e.x, e.y, bx, by);
        } else {
            self.ent[i].f146 = 0;
            self.ent[i].f140 += self.ent[t].f140;
            self.ent[t].f144 = 0;
            self.ent[t].flags |= 0x400;
            self.ent[i].tick70 = base + 1;
            self.m5_grow(i);
        }
    }

    /// m4's stationary chase: face the target and fire the dart thunk
    /// every v_26 while in range; drop back to (stationary) wander
    /// when the target leaves or dies. Interim stub for the mimic's
    /// custom family handlers.
    /// m4 CHASE (sub_1BB20 :22690): the militiaman stands his ground
    /// and shoots. Entering arms him (sub_1BC50 :22745 — ONE LCG:
    /// sprite 206 on 11/20 else 1, speed 0, filter = target
    /// class/model); every v_26 in range fires the sub_1A990 dart and
    /// refreshes the wizard's wanted timer (:22714); break state is
    /// base+0 (24, the disarm slot).
    fn militia_chase(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        let tgt = self.ent[i].f146;
        let (tx, ty, tz, tc, tm) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, 3u8, 0u8)
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0 || self.ent[t].act_life < 0
            {
                self.ent[i].tick70 = base;
                return;
            }
            let c = &self.ent[t];
            (c.x, c.y, c.z, c.class64, c.model65)
        };
        if self.ent[i].type86 == 0 {
            let d = self.ent_rand(i);
            let armed = if d % 20 <= 10 { 206 } else { 1 };
            self.set_sprite(i, armed);
            self.ent[i].f126 = 0;
            self.ent[i].f66 = tc;
            self.ent[i].f67 = tm;
        }
        let e = &self.ent[i];
        if e.f63 & 3 == 0 {
            let yaw = Self::angle_between(e.x, e.y, tx, ty);
            self.ent[i].f34 = yaw;
            self.ent[i].f30 = yaw;
        }
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        if (e.f63 as i16) % row.v_26 == 0 {
            let dz = tz.wrapping_sub(e.z) as i32;
            let sq = Self::dist2_sq(e.x, e.y, tx, ty).wrapping_add(dz.wrapping_mul(dz));
            if Self::isqrt(sq as u32) >= row.v_28 as u32 {
                self.ent[i].tick70 = base;
            } else {
                self.attack_thunk(i, 4, tgt, tx, ty, tz, 0, 0);
                if tgt == PLAYER_TARGET {
                    self.player_aggro = 200;
                }
            }
        }
    }

    /// m4 IDLE, state 25 (sub_1B5D0 :22436): the unarmed-look /
    /// filter restore, then every 4·v_26 the acquisition ladder —
    /// (1) a wizard on the village wanted list (+528 ≠ 0, the
    /// hostility gate) within aggro range, (2) the nearest burrower
    /// (m9), NO gate — villagers fight burrowers on their own, (3) a
    /// house within 0x1000 to move back into (the death slot with
    /// +26 = 1 = the silent-absorb walk-in; house occupants++).
    /// Deviation: the idle pair-up pack (:22650-84) stays stubbed.
    fn militia_idle(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        if self.ent[i].type86 != 0 {
            // Disarm (sub_1BCE0 :22765).
            self.set_sprite(i, 0);
            self.ent[i].f66 = 3;
            self.ent[i].f67 = 0xFF;
        }
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let (v26, r) = (row.v_26, row.v_28 as i32);
        if (self.ent[i].f63 as i16) % (4 * v26) != 0 {
            return;
        }
        if self.player_aggro != 0 && self.player_in_aggro_range(i, ctx) {
            self.ent[i].f146 = PLAYER_TARGET;
            self.ent[i].tick70 = base + 2;
            return;
        }
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let r2 = r * r;
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 == 5 && c.model65 == 9 && c.tick70 != 120 && c.act_life >= 0 {
                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                if d2 <= r2 && best.is_none_or(|(_, b)| d2 < b) {
                    best = Some((j, d2));
                }
            }
        }
        if let Some((j, _)) = best {
            self.ent[i].f146 = j as u16;
            self.ent[i].tick70 = base + 2;
            return;
        }
        if let Some(b) = self.nearest_building(ex, ey, Some(0x1000 * 0x1000)) {
            self.ent[b].f26 += 1;
            self.ent[i].f26 = 1;
            self.ent[i].tick70 = base + 4;
        }
    }

    /// Nearest live m45 house (the original's per-tick +36470 list;
    /// pool order stands in for list order, same approximation as the
    /// pack scans). `max_d2` = squared engine-unit window.
    fn nearest_building(&self, x: u16, y: u16, max_d2: Option<i32>) -> Option<usize> {
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 10 || c.model65 != 45 || c.flags & 0x400 != 0 {
                continue;
            }
            let d2 = Self::dist2_sq(x, y, c.x, c.y);
            if max_d2.is_some_and(|m| d2 > m) {
                continue;
            }
            if best.is_none_or(|(_, b)| d2 < b) {
                best = Some((j, d2));
            }
        }
        best.map(|(j, _)| j)
    }

    /// m12 settler WANDER, state 73 (sub_1EED0 :24994): jitter-walk;
    /// +26 runs down one per think tick — at 0 → +26 = 1, SEEK (75).
    fn m12_wander(&mut self, i: usize) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        if (self.ent[i].f63 as i16) % v26 == 0 {
            let d1 = self.ent_rand(i);
            let d2 = self.ent_rand(i);
            let mag = ((d2 & 0xFF) + 85) as i32;
            let sign = if d1 % 157 >= 79 { 1 } else { -1 };
            self.ent[i].f34 = ((self.ent[i].f34 as i32 + sign * mag) & 0x7FF) as u16;
            self.ent[i].f26 -= 1;
            if self.ent[i].f26 <= 0 {
                self.ent[i].f26 = 1;
                self.ent[i].tick70 = 75;
            }
        }
    }

    /// m12 SEEK, state 75 (sub_1F390 :25198): the nearest house on
    /// the m45 list (state-51 sites included — settlers cluster
    /// around construction) → APPROACH; none on the map → wander
    /// forever (villages only grow around existing buildings).
    fn m12_seek(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        if let Some(b) = self.nearest_building(x, y, None) {
            self.ent[i].f146 = b as u16;
            self.ent[i].f26 = 10;
            self.ent[i].tick70 = 74;
        } else {
            self.ent[i].f26 = 5;
            self.ent[i].tick70 = 73;
        }
    }

    /// m12 APPROACH, state 74 (sub_1F120 :25101): steer to the anchor
    /// house; +26 runs down every v_26/2 ticks (target gone or
    /// patience out → wander); inside 0xA00 → BUILD with +26 = 0.
    fn m12_approach(&mut self, i: usize) {
        let t = self.ent[i].f146 as usize;
        let valid =
            t != 0 && t < self.ent.len() && self.ent[t].class64 == 10 && self.ent[t].model65 == 45;
        if !valid {
            self.ent[i].f26 = 5;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = 73;
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        if (self.ent[i].f63 as i16) % (v26 / 2).max(1) == 0 {
            self.ent[i].f26 -= 1;
            if self.ent[i].f26 <= 0 {
                self.ent[i].f26 = 5;
                self.ent[i].f146 = 0;
                self.ent[i].tick70 = 73;
                return;
            }
        }
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let (bx, by) = (self.ent[t].x, self.ent[t].y);
        self.ent[i].f34 = Self::angle_between(ex, ey, bx, by);
        self.creature_move(i);
        if Self::dist2_sq(ex, ey, bx, by) < 0xA00 * 0xA00 {
            self.ent[i].f26 = 0;
            self.ent[i].tick70 = 72;
        }
    }

    /// m12 BUILD, state 72 (sub_1EA40 :24835): one site attempt per
    /// tick against the anchor house +146 — attempt # = the side
    /// (E/W/S/N), three settler-LCG draws each (type (rand&7)+25 =
    /// tent..house range, gap roll, perpendicular jitter). Water
    /// aborts to wander (+26 = 2); a rough or overlapping site just
    /// burns the attempt; the fifth entry resets (+26 = 1) to
    /// wander. Success spawns the (10,45) site in state 51 — the
    /// SAME 30-tick construction the features pass runs — and the
    /// settler retires into villager-feeder state 79: model stays
    /// 12, dispatch is state-based, exactly the original's trick.
    fn m12_build(&mut self, i: usize) {
        let a = self.ent[i].f146 as usize;
        let anchor_ok =
            a != 0 && a < self.ent.len() && self.ent[a].class64 == 10 && self.ent[a].model65 == 45;
        if !anchor_ok {
            self.ent[i].f26 = 5;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = 73;
            return;
        }
        let pre = self.ent[i].f26;
        self.ent[i].f26 = pre + 1;
        if pre >= 4 {
            self.ent[i].f26 = 1;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = 73;
            return;
        }
        let d = self.ent_rand(i);
        let btype = ((d & 7) + 25) as u16;
        let def = self.assets.build_tab[btype as usize % self.assets.build_tab.len()];
        // sub_1E9B0 (:24815): inflated footprint halves — the house
        // spacing margin.
        let half_x = ((def.w as i32) << 8) / 2 + 768;
        let half_y = ((def.h as i32) << 8) / 2 + 768;
        let (ax, ay, az, af80, af82) = {
            let e = &self.ent[a];
            (e.x, e.y, e.z, e.f80 as i32, e.f82 as i32)
        };
        let d1 = (self.ent_rand(i) % 3) as i32;
        let d2 = (self.ent_rand(i) % 3) as i32;
        let (mut px, mut py) = (ax as i32, ay as i32);
        match self.ent[i].f26 {
            1 => {
                px += af80 + half_x + (d1 << 8) + 256;
                py += (d2 << 8) - 1280;
            }
            2 => {
                px -= af80 + half_x + (d1 << 8) + 256;
                py += (d2 << 8) - 1280;
            }
            3 => {
                px += (d1 << 8) - 1280;
                py += af82 + half_y + (d2 << 8) + 256;
            }
            _ => {
                px += (d1 << 8) - 1280;
                py -= af82 + half_y + (d2 << 8) + 256;
            }
        }
        let (px, py) = (px as u16, py as u16);
        if self.on_water_pub(px, py) {
            self.ent[i].f26 = 2;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = 73;
            return;
        }
        // Flatness (sub_1E920/sub_35EA0): 4-corner max−min under the
        // 15/16 threshold.
        let thr = if (half_y >> 7) + (half_x >> 7) > 4 {
            16
        } else {
            15
        };
        if self.site_roughness(px, py, (half_x >> 8) as u8, (half_y >> 8) as u8) >= thr {
            return;
        }
        // Overlap vs every house, then every castle (:24940-75).
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            let house = c.class64 == 10 && c.model65 == 45;
            let castle = c.class64 == 3 && c.model65 == 2;
            if !(house || castle) || c.flags & 0x400 != 0 {
                continue;
            }
            let dx = (c.x.wrapping_sub(px) as i16 as i32).abs();
            let dy = (c.y.wrapping_sub(py) as i16 as i32).abs();
            if dx <= c.f80 as i32 + half_x && dy <= c.f82 as i32 + half_y {
                return;
            }
        }
        // Site accepted: the house goes up, the settler settles.
        if let Some(b) = self.spawn_creator(45, px, py, az) {
            self.snd(10, i); // construction gong (:24983)
            self.building_fixup(b, btype);
            self.ent[b].tick70 = 51;
        }
        self.ent[i].f146 = 0;
        self.ent[i].tick70 = 79;
    }

    /// sub_1E920/sub_35EA0 (:24802/:36260): 4-corner max−min height
    /// of the prospective footprint (spans in tiles), with the parity
    /// nudge on the start corner.
    fn site_roughness(&self, x: u16, y: u16, w_tiles: u8, h_tiles: u8) -> i32 {
        let mut v4 = ((x >> 8) as u8).wrapping_sub(w_tiles >> 1);
        let v5 = ((y >> 8) as u8).wrapping_sub(h_tiles >> 1);
        if (v4 as u16 + v5 as u16) % 2 == 1 {
            v4 = v4.wrapping_add(1);
        }
        let h = |cx: u8, cy: u8| self.t.height[crate::mc1::features::tile(cx, cy)] as i32;
        let c = [
            h(v4, v5),
            h(v4.wrapping_add(w_tiles), v5),
            h(v4.wrapping_add(w_tiles), v5.wrapping_add(h_tiles)),
            h(v4, v5.wrapping_add(h_tiles)),
        ];
        *c.iter().max().unwrap() - *c.iter().min().unwrap()
    }

    /// m13/m14 feeder wander (sub_1F640 :25296 / sub_1FAC0 :25472):
    /// with a house target — steer in from beyond 0x800, drop it if
    /// the house fills (+128 ≤ +26), walk in the door inside 0x800
    /// (death slot with +26 = 1 = silent absorb, house occupants++);
    /// without one — jitter-walk and acquire the nearest house every
    /// v_26 (`distant`: m14 only migrates to a village farther than
    /// 0xE100000 dist² — wrapping 32-bit math, verbatim).
    fn feeder_wander(&mut self, i: usize, base: u8, distant: bool) {
        self.creature_move(i);
        if self.ent[i].act_life < 0 {
            return;
        }
        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
        let think = (self.ent[i].f63 as i16) % v26 == 0;
        let t = self.ent[i].f146 as usize;
        let valid =
            t != 0 && t < self.ent.len() && self.ent[t].class64 == 10 && self.ent[t].model65 == 45;
        if valid {
            if !think {
                return;
            }
            if self.ent[t].f128 <= self.ent[t].f26 {
                self.ent[i].f146 = 0; // the house is full (:25397-404)
                return;
            }
            let (ex, ey) = (self.ent[i].x, self.ent[i].y);
            let (bx, by) = (self.ent[t].x, self.ent[t].y);
            if Self::dist2_sq(ex, ey, bx, by) > 0x800 * 0x800 {
                self.ent[i].f34 = Self::angle_between(ex, ey, bx, by);
            } else {
                self.ent[t].f26 += 1;
                self.ent[i].f26 = 1;
                self.ent[i].tick70 = base + 4; // walks in the door
            }
            return;
        }
        if t != 0 {
            self.ent[i].f146 = 0;
        }
        if !think {
            return;
        }
        let d1 = self.ent_rand(i);
        let d2 = self.ent_rand(i);
        let mag = ((d2 & 0xFF) + 85) as i32;
        let sign = if d1 % 157 >= 79 { 1 } else { -1 };
        self.ent[i].f34 = ((self.ent[i].f34 as i32 + sign * mag) & 0x7FF) as u16;
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        if let Some(b) = self.nearest_building(ex, ey, None) {
            if distant {
                let d2 = Self::dist2_sq(ex, ey, self.ent[b].x, self.ent[b].y);
                if d2 <= 0xE100000u32 as i32 {
                    return;
                }
            }
            self.ent[i].f146 = b as u16;
        }
    }

    /// The per-model attack thunks CHASE fires in range. Constants per
    /// the banked combat trace (docs/ROADMAP.md); projectile damage
    /// rides +44, explosions on +68/+69, owner immunity on +24.
    #[allow(clippy::too_many_arguments)]
    fn attack_thunk(
        &mut self,
        i: usize,
        model: u8,
        tgt: u16,
        tx: u16,
        ty: u16,
        tz: i16,
        tf66: u8,
        tf67: u8,
    ) {
        let (x, y, z, owner, f44, f84) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f44, e.f84)
        };
        let launch_z = z.wrapping_add(f84 as i16);
        match model {
            // sub_1A8E0 (:21874): the 500-damage straight fireball.
            0 | 3 => {
                if let Some(p) = self.spawn_fireball(x, y, launch_z) {
                    self.ent[p].row156 = 6; // turn 0: no homing
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 500, 0);
                    self.snd(8, i); // :22182/:22406
                }
            }
            // sub_1AB10 (:21962): melee within 1024 units, m2 recoils.
            // (No cooldown gate — the thunk fires whenever the shared
            // chase cadence lands it in range; the bee's +26 only
            // drives the recoil/lunge cycle in bee_chase.)
            1 | 2 => {
                let d2 = Self::dist2_sq(x, y, tx, ty);
                let dz = tz.wrapping_sub(z) as i32;
                if Self::isqrt(d2.wrapping_add(dz.wrapping_mul(dz)) as u32) < 1024 {
                    let t = if tgt == PLAYER_TARGET {
                        MailTarget::Player
                    } else {
                        MailTarget::Pool(tgt as usize)
                    };
                    self.mail_write(t, 0, f44 as u32, owner);
                    self.snd(if model == 2 { 13 } else { 7 }, i); // :22294/:22358
                    if model == 2 {
                        // Recoil + cooldown (:22356-62).
                        self.ent[i].f126 = -self.ent[i].f130;
                        let v26 = BEHAVIOR[self.ent[i].row156 as usize].v_26;
                        self.ent[i].f26 = 3 * v26;
                    }
                }
            }
            // sub_1A990 (:21907): the 250-damage straight bolt.
            4 | 10 => {
                if let Some(p) = self.spawn_bolt(x, y, launch_z) {
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 250, 0);
                }
            }
            // sub_1AB70 (:21976): m5's mana-scaled multishot,
            // sound 32 (:22975).
            5 => {
                self.snd(32, i);
                let mana = self.ent[i].f140;
                let maxmana = self.ent[i].f136.max(1);
                let v2 = (7 * mana / maxmana).max(0) as u32;
                let v4 = if v2 != 0 {
                    (self.ent_rand(i) % (100 * v2)) / 100
                } else {
                    0
                };
                let n = (v2 as i32).clamp(1, 5);
                match v4 {
                    0 => {
                        for k in 0..n {
                            if let Some(p) = self.spawn_fireball(x, y, launch_z) {
                                self.ent[p].row156 = (6 - k).max(0) as u8;
                                self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 400, 0);
                            }
                        }
                    }
                    1 | 2 => {
                        for _ in 0..(n - 1).max(0) {
                            if let Some(p) = self.spawn_zigzag(x, y, launch_z) {
                                self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 800, 23);
                            }
                        }
                    }
                    _ => {
                        if let Some(p) = self.spawn_trail_bolt(x, y, launch_z) {
                            self.ent[p].row156 = 3;
                            self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 8000, 17);
                        }
                    }
                }
            }
            // sub_1AE30 (:22101): m7's 780-damage slow bolt (class-9
            // m14; interim straight-bolt flight — table truncation).
            7 => {
                if let Some(p) = self.spawn_slow_bolt(x, y, launch_z) {
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 780, 0);
                }
            }
            // sub_1AEE0 (:22134): m8's 4000-damage beam, filter
            // copied from the target's own fields, row [6] (:22155).
            // A landed attack refreshes the victim's wanted timer
            // (+528 = 200, sub_1CE30 :23557-60).
            8 => {
                if let Some(p) = self.spawn_zigzag(x, y, launch_z) {
                    self.arm_projectile(p, owner, tf66, tf67, tgt, tx, ty, tz, 4000, 23);
                    self.ent[p].row156 = 6;
                    self.snd(38, i); // :23555
                    if tgt == PLAYER_TARGET {
                        self.player_aggro = 200;
                    }
                }
            }
            // sub_1AA40 (:21935): m9's bolt — 600 with segments, else
            // 400. (Aimed at the TARGET; the transcription's
            // self-aim at :21947-48 is a decompile casualty.)
            9 => {
                let dmg = if self.ent[i].f144 != 0 { 600 } else { 400 };
                if let Some(p) = self.spawn_bolt(x, y, launch_z) {
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, dmg, 0);
                }
            }
            // sub_1E380 (:24554): m11's 3000-payload wizard-seeker
            // (explodes into the ch3 mana-steal flash, wizards only).
            11 => {
                if let Some(p) = self.spawn_seeker(x, y, launch_z) {
                    self.ent[p].f26 = 20;
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, 3000, 25);
                    self.snd(9, i); // :24700
                }
            }
            // m15 (:25846-59): a bare bolt — no +44 override, so the
            // NewEvent default 100 rides.
            15 => {
                if let Some(p) = self.spawn_bolt(x, y, launch_z) {
                    let dflt = self.ent[p].f44;
                    self.arm_projectile(p, owner, 3, 0xFF, tgt, tx, ty, tz, dflt, 0);
                }
            }
            _ => {}
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
        // a bounded "fly slightly faster than the leader". The remc1
        // source line reads `a1x->+126 += v3x->+130`, but the dead
        // decompiler temp preserved above it reads BOTH operands from
        // the LEADER (`v10 = v3x->+130 + v3x->+126`) — a dead temp of
        // the += form would read the member's +126. The original
        // computed the leader sum; the += is a maintainer mis-fix
        // whose unbounded accumulation is exactly the runaway (IDLE's
        // pack scan is NOT awake-gated, so distant idle crowds pack
        // up and would ratchet forever). The bee's retail "no escape"
        // is the 3x lunge in bee_chase, not this line.
        self.ent[i].f126 = self.ent[l].f126.wrapping_add(self.ent[l].f130);
    }

    /// DEATH sub_1A6C0 (:21820): one tick — body segments become
    /// corpses (any segment's killer propagates to the head), kill
    /// credit, then self to CORPSE. m13/m14 absorbed by a castle
    /// (+26 != 0) despawn silently instead (:25451-62, :25625-28).
    fn mob_death(&mut self, i: usize, base: u8) {
        if matches!(self.ent[i].model65, 13 | 14) && self.ent[i].f26 != 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let mut s = self.ent[i].f54 as usize;
        while s != 0 {
            self.ent[s].tick70 = base + 5;
            if self.ent[s].f38 != 0 {
                self.ent[i].f38 = self.ent[s].f38;
            }
            s = self.ent[s].f54 as usize;
        }
        // Kill credit (:21840-50): the human player, chain heads only,
        // spell-track models excluded. The reward itself is the ball.
        if self.ent[i].f38 == PLAYER_TARGET
            && self.ent[i].id24 == i as u16
            && !matches!(self.ent[i].model65, 9 | 12 | 13 | 14 | 15)
        {
            self.kills += 1;
        }
        self.ent[i].tick70 = base + 5;
    }

    /// CORPSE sub_1A800 (:21855), on every 8th phase tick: drop the
    /// mana ball (sub_27690) and the death-flame puff, then despawn.
    /// Every worm segment corpses independently — each drops its own.
    fn mob_corpse(&mut self, i: usize) {
        if self.ent[i].f63 & 7 == 0 {
            self.corpse_drop(i);
            self.corpse_puff(i);
            self.ent[i].flags |= 0x400;
        }
    }

    /// sub_42510_42850 (:52763): one animation-frame step; true =
    /// already finished (does not wrap).
    pub(crate) fn anim_advance(&mut self, i: usize) -> bool {
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
        // The segment's own damage intake (:21127-37): apply pending
        // ch0, latch the attacker — the head's inbox walk inherits it.
        if self.ent[i].f58 != 0 && self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].act_life -= amt as i32;
            self.ent[i].mail[0].1 = 0;
            self.ent[i].f40 = src;
            self.ent[i].f38 = src;
        }
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
    /// player is within 24 tiles (2D dist² < 0x2400000 — sub_42410
    /// :52748 reads only x/y; altitude never gates waking. remc2's
    /// sub_68C70 uses the same 2D distance, corroborated by the
    /// synchronized remc1 body).
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
            } else if Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) < self.chassis.awake_gate_sq {
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
        // The damage inbox block opening every live state handler
        // (:21330-81): apply pending damage, dispatch death/aggro.
        // Families 8/12/13/14 mark the attacker instead of chasing
        // (:25057-63 — the "under attack" memory, wizard-AI track).
        match self.inbox(i) {
            Inbox::Dead => {
                if role == 3 {
                    // Pack-member death (:21746): the leader retargets
                    // the killer and rejoins the hunt.
                    let l = self.ent[i].f52 as usize;
                    if l != 0 && self.ent[l].class64 == 5 {
                        self.ent[l].f146 = self.ent[i].f38;
                        self.ent[l].f52 = 0;
                        self.ent[l].tick70 = base + 2;
                    }
                }
                // Killing village folk puts the wizard on the wanted
                // list (m12 :25291, m13 :25459, m14 :25638, m4's
                // corpse analog) — and so does killing a griffon
                // (sub_1CF60 :23578-80): the flock avenges it.
                if matches!(model, 4 | 8 | 12 | 13 | 14) && self.ent[i].f38 == PLAYER_TARGET {
                    self.player_aggro = 200;
                }
                self.ent[i].tick70 = base + 4;
                return;
            }
            Inbox::Hit(src) => {
                // The "under attack" mark the m8/12/13/14 families
                // write instead of chasing (:25057-63) — for the
                // village families it feeds the wanted timer.
                if matches!(model, 8 | 12 | 13 | 14) && src == PLAYER_TARGET {
                    self.player_aggro = 200;
                }
                // m8 DOES retaliate — its IDLE promotes a hit-by-
                // wizard griffon straight into attack (sub_1CA50
                // :23455-58); only the villager families merely mark
                // the attacker (:25057-63) without chasing.
                if self.attacker_is_wizard(src) && !matches!(model, 12 | 13 | 14) {
                    match role {
                        0 | 1 => {
                            self.ent[i].f146 = src;
                            if model == 11 {
                                // The genie's retaliation blinks
                                // ahead of the attacker (sub_1DFE0
                                // :24459-62 → sub_1E770).
                                self.genie_ambush(i, base, ctx);
                            } else {
                                self.ent[i].tick70 = base + 2;
                            }
                            return;
                        }
                        2 => {
                            // CHASE just retargets and returns (:21636).
                            self.ent[i].f146 = src;
                            return;
                        }
                        3 => {
                            // PACK: leader and member both retarget
                            // (:21742-65).
                            let l = self.ent[i].f52 as usize;
                            if l != 0 && self.ent[l].class64 == 5 {
                                self.ent[l].f146 = src;
                                self.ent[l].tick70 = base + 2;
                            }
                            self.ent[i].f146 = src;
                            self.ent[i].f52 = 0;
                            self.ent[i].tick70 = base + 2;
                            return;
                        }
                        _ => {}
                    }
                }
            }
            Inbox::Quiet => {}
        }
        // Model 6 forces its speed every movement tick (:23116).
        if model == 6 && role >= 1 {
            self.ent[i].f126 = 30;
        }
        match (model, role) {
            // -- m4, the VILLAGE MILITIA (the "mimic" reading was
            // half the story): stand-and-shoot with the +528 wanted-
            // timer hostility gate, armed/unarmed sprite swaps and
            // the walk-back-into-a-house exit. State 24 = the disarm
            // slot the chase breaks to. Pack pair-up (27) stubbed.
            (4, 0) => {
                if self.ent[i].type86 != 0 {
                    self.set_sprite(i, 0);
                    self.ent[i].f66 = 3;
                    self.ent[i].f67 = 0xFF;
                }
                self.ent[i].tick70 = base + 1;
            }
            (4, 1) => self.militia_idle(i, base, ctx),
            (4, 2) => self.militia_chase(i, base, ctx),
            (4, 3) => {}

            // -- idles --
            // m5's spawn state falls straight through to wander
            // (:22775); m9 = the materialize sequence; m12's idle
            // slot 72 = the BUILD state; m11 = the blink cycle;
            // m13/14/15 idles are custom/parked nops.
            (5, 0) => self.ent[i].tick70 = base + 1,
            (9, 0) => self.m9_emerge(i),
            (11, 0) => self.genie_idle(i, base),
            (12, 0) => self.m12_build(i),
            (13 | 14 | 15, 0) => {}
            (_, 0) => self.mob_idle(i, base),

            // -- wanders --
            (0, 1) => {
                self.mob_wander(i, base, ctx, true);
                self.flyer_bob(i);
            }
            // m5, the crab: mana-hunting wander + EAT in the family's
            // pack slot (state 0x21) + regen — growth feeds straight
            // into the mana-scaled multishot.
            (5, 1) => self.m5_wander(i, base, ctx),
            (5, 3) => self.m5_eat(i, base),
            (5, 2) => {
                self.mob_chase(i, base, ctx);
                self.m5_regen(i);
            }
            (9, 1) => self.m9_hidden(i, base),
            (11, 1) => self.genie_wander(i, base, ctx),
            (15, 1) => self.grid_walk(i, base),
            // The villager families' custom hunts.
            (12, 1) => self.m12_wander(i),
            (13, 1) => self.feeder_wander(i, base, false),
            (14, 1) => self.feeder_wander(i, base, true),
            // Every remaining model runs the shared awake-gated
            // two-scan — the engine has no per-model aggro list. m8's
            // CHASE promotion alone is gated on the wanted timer
            // (sub_1CA50 :23500 — the griffon stays peaceful until
            // the wizard is marked); m16 layers the house hunt on top
            // of the shared scans (sub_20710 :26033) when it is still
            // wandering afterwards.
            (m, 1) => {
                let aggro = m != 8 || self.player_aggro != 0;
                self.mob_wander(i, base, ctx, aggro);
                if m == 16 && self.ent[i].tick70 == base + 1 {
                    self.wyvern_house_hunt(i, base);
                }
            }

            // -- chases --
            (0, 2) => {
                self.mob_chase(i, base, ctx);
                self.flyer_bob(i);
            }
            (2, 2) => self.bee_chase(i, base, ctx),
            (8, 2) => self.griffon_chase(i, base, ctx),
            (9, 2) => {
                // sub_1DA60's per-tick disguise (sub_1DCD0): the mound
                // pops up as the warrior form while it chases.
                if self.ent[i].type86 != 202 {
                    self.set_sprite(i, 202);
                }
                self.mob_chase(i, base, ctx);
            }
            (11, 2) => self.genie_chase(i, base, ctx),
            // m12's chase slot 74 = the house APPROACH.
            (12, 2) => self.m12_approach(i),
            (16, 2) => self.wyvern_chase(i, base, ctx),
            (_, 2) => self.mob_chase(i, base, ctx),

            // -- packs --
            (0, 3) => {
                self.mob_pack(i, base);
                self.flyer_bob(i);
            }
            // m12's pack slot 75 = the house SEEK; m13/m14's pack
            // slots stay parked (unreferenced in the trace).
            (12, 3) => self.m12_seek(i),
            (13, 3) | (14, 3) => {}
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
            e.f58 = if model == 6 {
                64
            } else {
                v26 - (ordinal as i16 % v26) + 4
            };
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
                    if si == 0 {
                        50
                    } else {
                        193
                    }
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
