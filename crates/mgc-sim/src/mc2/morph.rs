//! MC2 class-10 TERRAIN MORPH band — (10,9) the raise-land /
//! apocalypse dome. Trace bank:
//! docs/traces/mc2-class10-m9-dome-geometry.md (the three-phase
//! machine, verbatim) + mc2-class10-m9-dome-open-closure.md (the
//! 2-D distance form, the shading recompute, the spell-XP
//! correction) + mc2-class10-m6-m9-m11-m28-m31.md §2 (`EF:` =
//! remc2 EventsFunctions.cpp, `Maths:` = utilities/Maths.cpp).
//!
//! Entity-field homes follow the class-10 effect column: subSpell →
//! f140, phase `byte_0x46_70` → f71, dome height `word_0x2C_44` →
//! f44, the fixed footprint radius `array_0x52_82.pitch` → f80 (the
//! shift/rot home, [`Gen::mc2_shift_rot`]); the dome BASE z rides the
//! entity z (`position_0x4C_76.z` — retail reuses the position).
//!
//! APPROXIMATIONS:
//! - `sub_6D8B0(id, 0x12, hits)` = wizard spell-XP credit for row 18
//!   (EF:58228 — NOT an earthquake event, the open-closure trace §1).
//!   The dome's own area beat DOES credit it (the `mc2_cast_xp` mail
//!   push at the type-0 beat below, EF:23388-95); only the summit-rain
//!   flood's row-18 XP — on the (10,91) apocalypse mana-rain child —
//!   is still deferred, hits computed and dropped.
//! - The `life==3` children: (10,18) = the ground-vortex eruption
//!   controller (`sub_32A70` — emits (10,16) tornadoes riding the
//!   whirlwind driver, the (10,19) column + a (9,0) bolt on tick 0,
//!   the vortex/plume singletons on the MC1 volcano registers) and
//!   (10,91) = the apocalypse mana rain (`sub_32CF0`) — trace
//!   docs/traces/mc2-class10-m18-m91-summit.md; both runtime-only
//!   (never authored). Their own APPROXes sit on the methods.
//! - The apocalypse latch (`D41A0_0.byte_0x36E03`) lives on `World`
//!   (`mc2_apocalypse`); its only setter — the endgame state machine
//!   `sub_21030` case 0xF (EF:12864) — is unported, so the authored
//!   dome always runs the damage-dealing variant (correct: the ctor
//!   zeroes the latch, EF:35527).

use super::sin_lut::SIN_DB750;
use crate::engine::features::{Gen, tile};
use crate::mc1::mobs::MobCtx;

/// `x_WORD_727B0` (Maths:647-676), the Heron-sqrt seed table: entry n
/// = `round(2^(n/2))` for the highest set bit n. Only the first 32
/// entries are real (the decompile's tail is bled code bytes, never
/// indexed — open-closure §2.1).
const ISQRT_SEED: [u32; 32] = [
    0x1, 0x2, 0x2, 0x4, 0x5, 0x8, 0xB, 0x10, 0x16, 0x20, 0x2D, 0x40, 0x5A, 0x80, 0xB5, 0x100,
    0x16A, 0x200, 0x2D4, 0x400, 0x5A8, 0x800, 0xB50, 0x1000, 0x16A0, 0x2000, 0x2D41, 0x4000,
    0x5A82, 0x8000, 0xB504, 0xFFFF,
];

/// `Maths::sub_7277A_radix_3d` (Maths:747-755) — integer sqrt: bit-
/// scan seed, then Heron iteration `i ← (a/i + i)/2` while `a/i < i`
/// (signed compare).
pub(crate) fn isqrt(a: u32) -> u32 {
    if a == 0 {
        return 0;
    }
    let mut i = ISQRT_SEED[(31 - a.leading_zeros()) as usize];
    while ((a / i) as i32) < i as i32 {
        i = (a / i + i) >> 1;
    }
    i
}

/// `Maths::EuclideanDistXYZ_58490` (Maths:738-745) — despite the
/// name, 2-D: `isqrt(dx² + dy²)`, deltas truncated to i16 before
/// squaring (z is never read — open-closure §2.2).
pub(crate) fn dist2d(ax: u16, ay: u16, bx: i32, by: i32) -> i32 {
    let dx = (bx - ax as i32) as i16 as i32;
    let dy = (by - ay as i32) as i16 as i32;
    isqrt((dx * dx + dy * dy) as u32) as i32
}

/// `sub_57450` (EF:39818) — terrain types that auto-flatten (force
/// the mapAngle low nibble) when a height write lands on them; also
/// the flood's burnable→lava predicate (mc2::flood).
pub(crate) fn auto_flat(t: u8) -> bool {
    matches!(
        t,
        0 | 0x25 | 0x26 | 0x2C..=0x2F | 0x51 | 0x53 | 0x68 | 0x69 | 0x6D | 0x72 | 0x74
    )
}

impl Gen {
    /// `NewAdd0A09_4E760` (EF:35513) — the (10,9) dome ctor: action 9,
    /// maxLife 11 / life 17, subSpell 2000, untargetable (byte[0] &=
    /// 0xF7), pitch seed `ShiftRot(7, 0x4000)` (a throwaway the init
    /// phase overwrites), not map-registered. Retail also clears the
    /// apocalypse latch here — that write lives at the World call
    /// sites (the latch is a World field).
    pub(crate) fn mc2_spawn_dome(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 9;
            e.tick70 = 9;
            e.max_life = 11;
            e.act_life = 17;
            e.f140 = 2000;
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.mc2_shift_rot(i, 7, 0x4000);
        Some(i)
    }

    /// `sub_48E60` → `sub_48F20` (EF:32623/32647) — MIN terrain
    /// height over the PERIMETER of the tile box at (ox, oy), with
    /// retail's TRANSPOSED walk kept verbatim: the row loop
    /// runs `h` samples in +x with the bottom row at `y = oy + w`;
    /// the column loop runs `w` samples in +y at `x = ox + h`
    /// (right) and `ox` (left). Square boxes — every caller today —
    /// are unaffected; a non-square authored box genuinely samples
    /// this transposed shape (init 250; u8 coords wrap like
    /// retail's byte packing).
    pub(crate) fn mc2_perimeter_min(&self, ox: u8, oy: u8, w: u16, h: u16) -> i32 {
        let mut result = 250i32;
        let mut x = ox;
        for _ in 0..h {
            result = result.min(self.t.height[tile(x, oy)] as i32);
            result = result.min(self.t.height[tile(x, oy.wrapping_add(w as u8))] as i32);
            x = x.wrapping_add(1);
        }
        let mut y = oy;
        for _ in 0..w {
            result = result.min(self.t.height[tile(x, y)] as i32);
            result = result.min(self.t.height[tile(x.wrapping_sub(h as u8), y)] as i32);
            y = y.wrapping_add(1);
        }
        result
    }

    /// `sub_570F0` (EF:39602) on the dome path (a4=0, a6=1): clamp,
    /// write the height, force the flat nibble for the inner core
    /// (`a5`) or auto-flat terrain types, run the h==0 water-seal
    /// neighbour walk, then the per-cell `AddBuildingToTerrain_46570`
    /// retile/shade recompute.
    pub(crate) fn mc2_dome_write_height(&mut self, x: u8, y: u8, h: i32, a5: bool) {
        let h = h.clamp(0, 255);
        let t = tile(x, y);
        self.t.height[t] = h as u8;
        if a5 || auto_flat(self.t.tile_type[t]) {
            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
        }
        if h == 0 {
            // The water-seal walk (EF:39660-39700): if all 8
            // neighbours pass `sub_56EE0` (angle&7 not in {2,3,5}),
            // clear this cell's low nibble.
            let sealed = [
                (255u8, 255u8),
                (0, 255),
                (1, 255),
                (1, 0),
                (255, 0),
                (255, 1),
                (0, 1),
                (1, 1),
            ]
            .iter()
            .all(|&(dx, dy)| {
                let a = self.t.angle[tile(x.wrapping_add(dx), y.wrapping_add(dy))] & 7;
                a != 5 && a != 2 && a != 3
            });
            if sealed {
                self.t.angle[t] &= 0xF0;
            }
        }
        self.mc2_add_building_region(x, y, x, y);
    }

    /// The 2x2 summit cap (EF:23300-23318 / 23400-23423, the same
    /// stamp in grow's `life==3` beat and finalize): height
    /// `plateau - 16`, shading 63 on Day / 1 otherwise — DIRECT
    /// heightmap writes, no recompute.
    fn mc2_dome_cap(&mut self, cx: u8, cy: u8, plateau: i32) {
        for dy in 0..2u8 {
            for dx in 0..2u8 {
                let t = tile(
                    cx.wrapping_sub(1).wrapping_add(dx),
                    cy.wrapping_sub(1).wrapping_add(dy),
                );
                self.t.height[t] = (plateau - 16).clamp(0, 255) as u8;
                self.t.shading[t] = if self.mc2_night_shade.0 { 1 } else { 63 };
            }
        }
    }

    /// `sub_31940` (EF:23193-23433) — the three-phase dome machine:
    /// f71 0 = init (falls through into grow), 1 = grow per tick,
    /// 2 = finalize + despawn. Returns terrain-dirty.
    pub(crate) fn mc2_dome_tick(&mut self, i: usize, ctx: &MobCtx, apocalypse: bool) -> bool {
        let cx = ((self.ent[i].x.wrapping_add(128)) >> 8) as u8;
        let cy = ((self.ent[i].y.wrapping_add(128)) >> 8) as u8;

        // ---- INIT (EF:23245-23261) — radius fixed here, only read
        // after; base z = perimeter MIN under the footprint; height =
        // 2R+100 clamped so the summit stays <= 255.
        if self.ent[i].f71 == 0 {
            let r = (self.ent[i].max_life | 1) as i32;
            self.mc2_shift_rot(i, (r << 8) as u16, 0x4000);
            let ox = cx.wrapping_sub(r as u8);
            let oy = cy.wrapping_sub(r as u8);
            let base = self.mc2_perimeter_min(ox, oy, (2 * r) as u16, (2 * r) as u16);
            self.ent[i].z = base as i16;
            let mut ht = 2 * r + 100;
            if base + ht > 255 {
                ht = 255 - base;
            }
            self.ent[i].f44 = ht as u16;
            self.ent[i].f71 = 1;
            // falls through into the grow body (retail has no return)
        }

        // ---- FINALIZE (EF:23263-23319): flatten the footprint to
        // `summit - 24` (lower-only, direct writes), stamp the 2x2
        // cap, despawn.
        if self.ent[i].f71 >= 2 {
            let plateau = self.ent[i].z as i32 + self.ent[i].f44 as i32 - 24;
            let r = (self.ent[i].f80 >> 8) as u8;
            let side = self.ent[i].f80 >> 7;
            for j in 0..side {
                let y = cy.wrapping_sub(r).wrapping_add(j as u8);
                for k in 0..side {
                    let x = cx.wrapping_sub(r).wrapping_add(k as u8);
                    let t = tile(x, y);
                    if (self.t.height[t] as i32) > plateau {
                        self.t.height[t] = plateau.clamp(0, 255) as u8;
                    }
                }
            }
            self.mc2_dome_cap(cx, cy, plateau);
            self.ent[i].flags |= 0x400;
            return true;
        }

        // ---- GROW (EF:23324-23431).
        self.ent[i].act_life -= 1;
        let life = self.ent[i].act_life;
        if life <= 0 {
            self.ent[i].f71 = 2;
            return false;
        }
        let radius = self.ent[i].f80 as i32; // fixed pitch, <<8 units
        let side = radius >> 7; // box side = 2R tiles
        let r_tiles = (radius >> 8) as u8;
        // The inner-flat threshold: cells with dist <= v34 force the
        // walkable/flat mapAngle nibble (EF:23335).
        let v34 = radius - (((((radius >> 8) - 7) >> 1) << 8) + 512);
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let (base, ht) = (self.ent[i].z as i32, self.ent[i].f44 as i32);
        for j in 0..side {
            let y = cy.wrapping_sub(r_tiles).wrapping_add(j as u8);
            for k in 0..side {
                let x = cx.wrapping_sub(r_tiles).wrapping_add(k as u8);
                let d = dist2d(ex, ey, (x as i32) << 8, (y as i32) << 8);
                if d < radius {
                    // Raised-cosine profile: phase 0 (center, cos=+1)
                    // .. 0x400 (rim, cos=-1) → (1+cos)/2 of height.
                    let phase = ((d << 10) / radius) as usize;
                    let cosv = SIN_DB750[0x200 + phase] as i64;
                    let target = (((ht as i64 * ((0x10000 + cosv) >> 1)) >> 16) as i32) + base;
                    let cur = self.t.height[tile(x, y)] as i32;
                    // Ease 1/life of the remaining gap; raise-only.
                    let h = if target > cur {
                        (target - cur) / life + cur
                    } else {
                        cur
                    };
                    self.mc2_dome_write_height(x, y, h, d <= v34);
                    // Cave ceiling: ease UP toward floor + 64 (clamp
                    // 254) — the roof keeps clearance ahead of the
                    // rising dome (EF:23366-23379; only when the
                    // target is above the current ceiling).
                    if self.is_cave() {
                        let t = tile(x, y);
                        let tgt = (h + 64).min(254);
                        let c = self.t.ceiling[t] as i32;
                        if tgt > c {
                            self.t.ceiling[t] = (c - (c - tgt) / life) as u8;
                        }
                    }
                }
                // The bit-3 sync — EVERY box cell, sync-only, no pin
                // (EF:23381-23387).
                if self.is_cave() {
                    let t = tile(x, y);
                    if self.t.ceiling[t] > self.t.height[t] {
                        self.t.angle[t] &= !8;
                    } else {
                        self.t.angle[t] |= 8;
                    }
                }
            }
        }
        // Combat + audio pulse: type-0 area beat (sub_116A0) unless
        // apocalypse, with the row-18 batch XP (EF:23388-95).
        // Rumble sound 10 every tick; the apocalypse adds 63 on the
        // byte_0x3E_62 (f63) 4-tick cadence.
        if !apocalypse {
            let amt = self.ent[i].f140 as u32;
            let hits = self.area_write(i, 0, amt, ctx, false, false);
            if hits != 0 && self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                self.mc2_cast_xp.0.push((self.ent[i].id24, 18, hits as i32));
            }
        }
        self.snd(10, i);
        if apocalypse && self.ent[i].f63 & 3 == 0 {
            self.snd(63, i);
        }
        // The life==3 beat: pre-stamp the summit cap and birth the
        // child at terrain height (EF:23400-23430) — (10,18) the
        // ground-vortex eruption, or (10,91) the apocalypse mana
        // rain; only the dome's id is inherited
        // (docs/traces/mc2-class10-m18-m91-summit.md §1).
        if life == 3 {
            let plateau = self.ent[i].z as i32 + self.ent[i].f44 as i32 - 24;
            self.mc2_dome_cap(cx, cy, plateau);
            let (x, y, id) = {
                let e = &self.ent[i];
                (e.x, e.y, e.id24)
            };
            let gz = self.ground_z(x, y) as i16;
            let child = if apocalypse {
                self.mc2_spawn_summit91(x, y, gz)
            } else {
                self.mc2_spawn_summit18(x, y, gz)
            };
            if let Some(c) = child {
                self.ent[c].id24 = id;
            }
        }
        true
    }

    // ---- the summit children (mc2-class10-m18-m91-summit.md) ---------------

    /// `sub_4EED0` (EF:35777) — the (10,18) SUMMIT VORTEX controller
    /// ctor: action 18, subSpell 200, maxLife = life = 10000 (the
    /// machine self-terminates instead), invisible (no sprite, not
    /// map-linked, byte[0] bit 3 cleared), tick counter zeroed. No
    /// RNG.
    pub(crate) fn mc2_spawn_summit18(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 18;
        e.tick70 = 18;
        e.f140 = 200;
        e.f26 = 0;
        e.max_life = 10000;
        e.act_life = 10000;
        e.flags &= !8;
        e.x = x;
        e.y = y;
        e.z = z;
        Some(i)
    }

    /// `sub_4EF30` (EF:35797) — the (10,91) APOCALYPSE MANA-RAIN
    /// controller ctor: action 98 (0x62), otherwise the model-18
    /// numbers with byte[0] = (&0xF6)|1. No RNG.
    pub(crate) fn mc2_spawn_summit91(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.mc2_spawn_summit18(x, y, z)?;
        self.ent[i].model65 = 91;
        self.ent[i].tick70 = 98;
        self.ent[i].flags = (self.ent[i].flags & !0x9) | 1;
        Some(i)
    }

    /// `sub_32A70` (EF:23906, action 18) — the ground-vortex
    /// eruption controller: on each pulse (tick 0, then while
    /// `t < 128 && t & 0xF` on a 1-in-5 roll) it re-snaps to terrain
    /// (a changed floor despawns it), emits one (10,16) tornado
    /// (seeded from its own stream), spins its yaw +1280, and on
    /// tick 0 additionally seizes the vortex/plume singleton
    /// registers (`word_0x31`/`word_0x33` — our MC1 volcano
    /// `erupting`/`plume` homes: the previous vortex is fast-expired
    /// to t=250, the previous (10,19) column killed), spawns the
    /// persistent (10,19) fire-spray column and one visual (9,0)
    /// bolt pitched -386 with impact (10,17). Past 2500 ticks a
    /// 1-in-100 roll (only while no vortex is latched) restarts or
    /// despawns it; a pulse at `t >= 127` despawns and releases the
    /// latch. Deals NO damage itself — the children carry it.
    pub(crate) fn mc2_summit18_tick(&mut self, i: usize) {
        if self.ent[i].f26 > 2500 {
            let r = self.ent_rand(i);
            if r % 0x64 == 0 && self.erupting == 0 {
                let (x, y, z) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z)
                };
                let gz = self.ground_z(x, y) as i16;
                self.ent[i].z = gz;
                if z != gz {
                    self.ent[i].flags |= 0x400;
                    return;
                }
                self.ent[i].f26 = 0;
            }
        }
        let t = self.ent[i].f26;
        let pulse = (t < 128 && t & 0xF != 0 && self.ent_rand(i) % 5 == 0) || t == 0;
        if pulse {
            let (x, y, z, id) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24)
            };
            let gz = self.ground_z(x, y) as i16;
            self.ent[i].z = gz;
            if z != gz {
                self.ent[i].flags |= 0x400;
                self.erupting = 0;
                return;
            }
            if t == 0 {
                let prev = self.erupting as usize;
                if prev != 0 && self.ent[prev].flags & 0x400 == 0 {
                    self.ent[prev].f26 = 250; // fast-expire the old vortex
                }
                self.erupting = i as u16;
                if let Some(col) = self.mc2_spawn_fire_spray(x, y, gz) {
                    self.ent[col].id24 = id;
                    let old = self.plume as usize;
                    if old != 0 && old != col && self.ent[old].flags & 0x400 == 0 {
                        self.ent[old].flags |= 0x400;
                    }
                    self.plume = col as u16;
                }
            }
            if let Some(tw) = self.mc2_spawn_boulder16(x, y, gz) {
                self.ent[tw].id24 = id;
                let seed = self.ent_rand(i);
                self.ent[tw].rand = seed;
            }
            let yaw = self.ent[i].f30.wrapping_add(1280) & 0x7FF;
            self.ent[i].f30 = yaw;
            if t == 0
                && let Some(b) = self.mc2_spawn_bolt(x, y, gz)
            {
                let e = &mut self.ent[b];
                e.id24 = id;
                e.f32 = (-386i16) as u16; // steep upward pitch
                e.f68 = 10;
                e.f69 = 17; // impact = the (10,17) meteor
                e.act_life = 1;
                e.f30 = yaw;
                e.f34 = yaw;
                e.f36 = e.f32;
                let mut aim = (x, y, gz);
                Self::polar_step(&mut aim, yaw, 0, 1536);
                e.dest_x = aim.0;
                e.dest_y = aim.1;
            }
            if t >= 127 {
                self.ent[i].flags |= 0x400;
                self.erupting = 0;
                return;
            }
        }
        // Retail's counter is the i32 `dword_0x10_16` and simply keeps
        // counting past 32767 (the self-latched controller never
        // restarts — the 1-in-100 roll is gated on the vortex register
        // it holds — so it idles until the endgame teardown, OPEN).
        // Our i16 home would panic there; saturating is behaviorally
        // identical since every gate reads > 2500, < 128, or == 0.
        self.ent[i].f26 = self.ent[i].f26.saturating_add(1);
    }

    /// `sub_32CF0` (EF:24007, action 98) — the apocalypse MANA RAIN:
    /// every tick launch THREE (10,39) collectible spheres with the
    /// exact 5-draw arming order per sphere (speed % 0x300 clamped
    /// [64,768]; apex (r&0x7F)+128; color roll % 9 − 1; mana
    /// % 0xA00 + 1; yaw r & 0x7FF), life 140, scattered one launch
    /// step from the summit and dropped at ground + 96. Never
    /// despawns itself (retail relies on the endgame teardown, OPEN).
    ///
    /// APPROX register: the launch velocity rides the ball tick's
    /// native `dest_x/dest_y` throw deltas (retail stores the same
    /// delta on `axis_0x9A`; the ±64/tick clamp and the apex term
    /// `word_0x2C_44` are the shared-ball-machinery APPROX of
    /// mobs.rs); the color-variant sprite roll keeps its draw but
    /// the neutral ball family renders (ball_resize); retail expires
    /// its spheres via `byte[1] |= 0x20` + life 140 — the decay
    /// channel (ball_tick's decay tail, flag bit 13 — fade bits
    /// 24→23, expire at 0, no merge-initiate) bounds the rain at ~420
    /// live spheres like retail; the 200-slot free cushion is a
    /// pool-exhaustion belt (deliberate: retail has none); the
    /// every-other-tick 26-row spell-XP flood (`sub_6D8B0`, xp =
    /// tier-2 xpos1/512) is the lone remaining MC2 XP gap (hits
    /// computed then discarded — see the module header; every other
    /// XP source is wired through the `mc2_cast_xp` mail).
    pub(crate) fn mc2_summit91_tick(&mut self, i: usize) {
        for _ in 0..3 {
            let (x, y, z) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            let speed = (self.ent_rand(i) % 0x300).clamp(64, 768) as i16;
            let _apex = (self.ent_rand(i) & 0x7F) as u16 + 128;
            let _color = self.ent_rand(i) % 9; // the variant roll (draw kept)
            let mana = (self.ent_rand(i) % 0xA00) as i32 + 1;
            let yaw = (self.ent_rand(i) & 0x7FF) as u16;
            if self.free.len() <= 200 {
                continue; // the pool cushion (deliberate: retail has none)
            }
            // Spawn one launch step out (retail's first flight tick
            // — our ±64/tick ball clamp would otherwise hold the
            // three coincident at the summit and the merge pass
            // would absorb them into one).
            let mut lp = (x, y, z);
            Self::polar_step(&mut lp, yaw, 0, speed);
            let gz = (self.ground_z(lp.0, lp.1) + 96) as i16;
            if let Some(s) = self.spawn_mana_ball(lp.0, lp.1, gz.max(z)) {
                let e = &mut self.ent[s];
                e.max_life = 140;
                e.act_life = 140;
                // The retail decay channel `byte[1] |= 0x20` (port
                // flag bit 13): the sphere fades out over its 140-
                // tick life (ball_tick's decay tail) — the rain is
                // TIMED window dressing, not a permanent mana mine.
                e.flags |= 0x2000;
                e.f140 = mana;
                e.f144 = 0;
                e.dest_x = lp.0.wrapping_sub(x); // the throw velocity delta
                e.dest_y = lp.1.wrapping_sub(y);
                self.ball_resize(s);
            }
        }
        // (the 26-row XP flood — deferred, module-doc APPROX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isqrt_matches_floor_sqrt() {
        // The Heron loop lands on floor(sqrt) for the dome's whole
        // operating range (radix <= 2 * (12 << 8)^2, open-closure
        // §2.3) — spot the band edges and squares.
        for a in [0u32, 1, 2, 3, 4, 15, 16, 17, 100, 3072 * 3072, 18_874_368] {
            let r = isqrt(a);
            assert!(r * r <= a && (r + 1) * (r + 1) > a, "isqrt({a}) = {r}");
        }
    }

    #[test]
    fn auto_flat_set_matches_sub_57450() {
        // EF:39818 decision ladder, exhaustively re-derived.
        let expect = |t: u8| -> bool {
            if t < 0x53 {
                if t < 0x25 {
                    t == 0
                } else if t > 0x26 {
                    if t >= 0x2C {
                        !(t > 0x2F && t != 81)
                    } else {
                        false
                    }
                } else {
                    true
                }
            } else if t <= 0x53 {
                true
            } else if t < 0x6D {
                (0x68..=0x69).contains(&t)
            } else if t <= 0x6D {
                true
            } else if t >= 0x72 {
                !(t > 0x72 && t != 116)
            } else {
                false
            }
        };
        for t in 0..=255u8 {
            assert_eq!(auto_flat(t), expect(t), "type {t}");
        }
    }
}
