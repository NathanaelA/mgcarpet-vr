//! MC2 class-10 TERRAIN MORPH band, Phase 4.3 — (10,9) the
//! raise-land / apocalypse dome. Trace bank:
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
//! DELIBERATE APPROXIMATIONS (cited):
//! - `sub_6D8B0(id, 0x12, hits)` = wizard spell-XP credit for row 18
//!   (EF:58228 — NOT an earthquake event, the open-closure trace §1);
//!   the spell-XP intake lands with Phase 4.2, hits computed and
//!   dropped like the rest of the tail band.
//! - The `life==3` child — (10,18) normal / (10,91) apocalypse, the
//!   volcano-summit eruption machine (sub_4EED0 / sub_32A70) — is
//!   UNPORTED; the spawn notes a misfit so the 4.3b level grind sees
//!   it honestly (the (10,5)-splash precedent).
//! - Cave arms (the second-heightmap ceiling raise + the mapAngle
//!   bit-3 seal, EF:23366-23387) defer to Phase 4.5 caves wholesale,
//!   like every module before this one.
//! - The apocalypse latch (`D41A0_0.byte_0x36E03`) lives on `World`
//!   (`mc2_apocalypse`); its only setter — the endgame state machine
//!   `sub_21030` case 0xF (EF:12864) — is unported, so the authored
//!   dome always runs the damage-dealing variant (correct: the ctor
//!   zeroes the latch, EF:35527).

use super::sin_lut::SIN_DB750;
use crate::mc1::features::{Gen, tile};
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
fn isqrt(a: u32) -> u32 {
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
fn dist2d(ax: u16, ay: u16, bx: i32, by: i32) -> i32 {
    let dx = (bx - ax as i32) as i16 as i32;
    let dy = (by - ay as i32) as i16 as i32;
    isqrt((dx * dx + dy * dy) as u32) as i32
}

/// `sub_57450` (EF:39818) — terrain types that auto-flatten (force
/// the mapAngle low nibble) when a height write lands on them.
fn auto_flat(t: u8) -> bool {
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
    /// height over the PERIMETER of the `w x h` tile box at (ox, oy):
    /// top row `y=oy` + bottom row `y=oy+h` over `w` cells, then
    /// right column `x=ox+w` + left column `x=ox` over `h` cells
    /// (init 250; the u8 coords wrap like retail's byte packing).
    fn mc2_perimeter_min(&self, ox: u8, oy: u8, w: u16, h: u16) -> i32 {
        let mut result = 250i32;
        let mut x = ox;
        for _ in 0..w {
            result = result.min(self.t.height[tile(x, oy)] as i32);
            result = result.min(self.t.height[tile(x, oy.wrapping_add(h as u8))] as i32);
            x = x.wrapping_add(1);
        }
        let mut y = oy;
        for _ in 0..h {
            result = result.min(self.t.height[tile(x, y)] as i32);
            result = result.min(self.t.height[tile(x.wrapping_sub(w as u8), y)] as i32);
            y = y.wrapping_add(1);
        }
        result
    }

    /// `sub_570F0` (EF:39602) on the dome path (a4=0, a6=1): clamp,
    /// write the height, force the flat nibble for the inner core
    /// (`a5`) or auto-flat terrain types, run the h==0 water-seal
    /// neighbour walk, then the per-cell `AddBuildingToTerrain_46570`
    /// retile/shade recompute.
    fn mc2_dome_write_height(&mut self, x: u8, y: u8, h: i32, a5: bool) {
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
                }
                // (cave open/seal flag arm APPROX-skipped, 4.5)
            }
        }
        // Combat + audio pulse: type-0 area beat (sub_116A0) unless
        // apocalypse; the hit count's sub_6D8B0 row-18 XP credit
        // banks with 4.2. Rumble sound 10 every tick; the apocalypse
        // adds 63 on the byte_0x3E_62 (f63) 4-tick cadence.
        if !apocalypse {
            let amt = self.ent[i].f140 as u32;
            let _hits = self.area_write(i, 0, amt, ctx, false, false);
        }
        self.snd(10, i);
        if apocalypse && self.ent[i].f63 & 3 == 0 {
            self.snd(63, i);
        }
        // The life==3 beat: pre-stamp the summit cap and birth the
        // child at terrain height (EF:23400-23430).
        if life == 3 {
            let plateau = self.ent[i].z as i32 + self.ent[i].f44 as i32 - 24;
            self.mc2_dome_cap(cx, cy, plateau);
            // (10,18) / (10,91) — the summit eruption machine,
            // unported (module doc); id inheritance rides the port.
            self.note_misfit(10, if apocalypse { 91 } else { 18 });
        }
        true
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
