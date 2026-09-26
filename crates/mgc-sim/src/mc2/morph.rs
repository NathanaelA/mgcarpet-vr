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
//!   child's own row-18 AREA-BEAT credit — on the (10,91) apocalypse
//!   mana-rain child — is still deferred, hits computed and dropped.
//!   That child's 26-row `sub_6D8B0` FLOOD is a different call and IS
//!   landed ([`no_mc2_rain_spell_xp`]).
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

/// A/B toggle for the APOCALYPSE MANA-RAIN ARMING (`sub_32CF0`,
/// EF:24042-71): set `MGC_NO_MC2_RAIN_ARM` to restore the pre-dig
/// behaviour, where each rain sphere was born one full launch step
/// OUT of the summit, at `max(ground+96, summit z)`, with no
/// `actSpeed`/`yaw`/`roll`/`axis_0x9A.z` arming and with the
/// `SetManaSphereColorAndRot_36920` owner-derive + per-size rotation
/// constants instead of the colour-rolled `SetEntityIndexAndRot_49CD0`
/// family. See the write-up at [`Gen::mc2_summit91_tick`].
fn no_mc2_rain_arm() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_RAIN_ARM").is_some())
}

/// A/B toggle for the APOCALYPSE RAIN'S 26-ROW SPELL-XP FLOOD
/// (`sub_32CF0`'s second half, EF:24096-24112): set
/// `MGC_NO_MC2_RAIN_SPELL_XP` to restore the pre-dig behaviour, where
/// the summit spawned its three spheres and awarded nothing.
///
/// Retail closes `sub_32CF0` with, verbatim from the shipped
/// `NETHERW.EXE` (file = VA + 0x24800; the loop is file
/// 0x576DD..0x5772D, VA 0x32EDD..0x32F2D):
/// ```text
///   576dd  8b 45 14              mov    0x14(%ebp),%eax      ; the summit
///   576e0  f6 40 3e 01           testb  $0x1,0x3e(%eax)      ; byte_0x3E_62 & 1
///   576e4  75 49                 jne    0x5772f              ;   odd phase: nothing
///   576e6  be 18 a8 00 00        mov    $0xa818,%esi         ; &SPELLS[0], stride 0x50
///   576eb  31 db                 xor    %ebx,%ebx            ; spell = 0
///   576ef  8b 46 42              mov    0x42(%esi),%eax      ; SPELLS[s].subspell[2].xpos1_E
///   576f2  89 c2                 mov    %eax,%edx
///   576f4  c1 fa 1f              sar    $0x1f,%edx           ; sign mask
///   576f7  c1 e2 09              shl    $0x9,%edx            ; 0 / -512, CF = sign
///   576fa  1b c2                 sbb    %edx,%eax            ; += 511 when negative
///   576fc  c1 f8 09              sar    $0x9,%eax            ; => xpos1 / 512, toward 0
///   576ff  50                    push   %eax                 ; amount
///   5770f  53                    push   %ebx                 ; spell
///   57710  66 8b 84 02 e8 2b …   mov    0x2be8(%edx,%eax,1),%ax  ; the LOCAL player's ent
///   5771d  50                    push   %eax                 ; owner
///   5771e  43                    inc    %ebx
///   5771f  83 c6 50              add    $0x50,%esi
///   57722  e8 89 a9 03 00        call   0x920b0              ; sub_6D8B0
///   5772a  83 fb 1a              cmp    $0x1a,%ebx           ; 26 rows
///   5772d  7c c0                 jl     0x576ef
/// ```
/// — i.e. on every EVEN `byte_0x3E_62` phase the rain pays the LOCAL
/// player `SPELLS[s].tiers[2].xpos1 / 512` volatile XP on all 26 rows,
/// owner-blind (the summit's own `id_0x1A_26` is never read) and
/// life-blind. The `sbb` after `shl` is retail's idiom for C's
/// truncate-toward-zero `/ 512`, which is Rust's `/` on `i32`.
/// The decompile is EF:24096-24112 with the broken `__CFSHL__` carry
/// macro spelled out by hand; the EXE above is the authority.
/// See [`Gen::mc2_summit91_tick`].
fn no_mc2_rain_spell_xp() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_RAIN_SPELL_XP").is_some())
}

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
        // Retail's ctor makes NO store to +0x2C; this model's
        // port `f44` is that word. See `Gen::mc2_alloc_2c_zero`.
        self.mc2_alloc_2c_zero(i);
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
            // `sub_116A0` (EF:23393), NOT `sub_10C80` — the two MC2
            // area writers differ by more than the castle shake now
            // that pass 2 is in: only `sub_10C80` walks the building
            // footprint list, so a site bound to the wrong variant
            // would silently acquire (or lose) that pass.
            let hits = self.area_write(i, 0, amt, ctx, false, true);
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
        // Retail's ctor makes NO store to +0x2C; this model's
        // port `f44` is that word. See `Gen::mc2_alloc_2c_zero`.
        self.mc2_alloc_2c_zero(i);
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 18;
        e.tick70 = 18;
        e.f140 = 200;
        // `movl $0x0,0x10(%ebx)` (NETHERW.EXE file 0x736F9) — the arc
        // counter is stored 32 bits wide; both homes start at 0.
        e.f26 = 0;
        e.summit10 = crate::engine::features::Summit10(0);
        e.max_life = 10000;
        e.act_life = 10000;
        e.flags &= !8;
        e.x = x;
        e.y = y;
        e.z = z;
        Some(i)
    }

    /// `sub_4EF30` (address banner `0004EF30`, EF:35848) — the (10,91)
    /// APOCALYPSE MANA-RAIN controller ctor: action 98 (0x62),
    /// otherwise the model-18 numbers with byte[0] = (&0xF6)|1. No RNG.
    ///
    /// ⭐ IT SEEDS `subSpellIndex_0x2A_42 = 200` EXACTLY LIKE ITS
    /// (10,18) SIBLING, and the port dropped it. Byte-verified in the
    /// shipped `NETHERW.EXE` — file **0x73750**
    /// `66 c7 40 2a c8 00  movw $0xc8,0x2a(%eax)`, the same six bytes
    /// `sub_4EED0` has at file 0x736F0, sitting between
    /// `movb $0x5b,0x40(%eax)` (model 91) and `movl $0x0,0x10(%ebx)`.
    /// Model 91 is NOT a [`c10_2a_in_f140`] member, so the port's home
    /// for its @0x2A is the uniform class-10 seat `f44` (which is what
    /// `import_ent_mc2`'s `f44: … else { r.f2a }` already restores and
    /// what `port_ent_lanes_mc2` publishes on the `f2a` lane) — and
    /// [`Gen::mc2_alloc_2c_zero`], inherited from the model-18 ctor,
    /// had left it at 0. WITNESS (free run, `MGC_RAW_SHADOW=1`):
    /// `(10,91) f2a` retail **200** / port 0, 5,202 rows on
    /// mc2l24-crazy slot 747 (t=73863..79064) and 1,218 on mc2l24 slot
    /// 823 (t=52840..54057). Nothing in `sub_32CF0` spends it — the
    /// rain bills its own `% 0xA00 + 1` roll per sphere — so the seat
    /// is home-alignment, like (10,1)/(10,15)/(10,25).
    pub(crate) fn mc2_spawn_summit91(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.mc2_spawn_summit18(x, y, z)?;
        self.ent[i].model65 = 91;
        self.ent[i].tick70 = 98;
        self.ent[i].flags = (self.ent[i].flags & !0x9) | 1;
        if !crate::engine::features::no_mc2_rain_2a_seed() {
            // @0x2A = 200; model 91's port home for it is `f44`, not
            // the model-18 sibling's `f140`.
            self.ent[i].f44 = 200;
            self.ent[i].f140 = 0;
        }
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
    ///
    /// The re-snap and the ground-move despawn are UNCONDITIONAL
    /// (EF:23922-31): retail's own vortex dies the moment the dome —
    /// after it in the walk — moves the ground under it (mc2l24
    /// t=17). The old strict-gated FROZEN-Z variant was a
    /// pristine-plane workaround; the format-2 measured terrain
    /// channel carries the raised summit as ground truth, so
    /// conformance replay runs the exact retail check too. The
    /// over-eruption the un-latched port would otherwise do (every
    /// roll past 2500 where retail holds word_0x31) stays fenced by
    /// importing the captured latch into `erupting` (conformance.rs,
    /// mgcr word_0x31).
    ///
    /// ⭐ THE REGISTER WRITES ARE BLIND (round 158, dig w158e; PATCH
    /// OPTION `volcano_register_revalidate`, shared with the MC1 twin
    /// `eruption_tick`). `NETHERW.EXE` file 0x5735A-0x57379: the kick
    /// is `mov 0x31(D41A0),%ax` → `Entities[ax]` → `cmp Entities[0];
    /// jbe` → `movl $0xfa,0x10(%eax)` — a 32-bit `@0x10 = 250` into
    /// WHATEVER holds the slot, no class/model/life test (a castle
    /// there is levelled to 250; the new vortex's own slot self-kicks
    /// it, and `@0x10` is RE-READ from memory afterwards — `0x5743C
    /// mov 0x10(%ebx),%esi` for the bolt, `0x574CB cmpl $0x7f,0x10(%ebx)`
    /// for the despawn). File 0x573BE-0x573DE: `word_0x33` is read
    /// AFTER the new `(10,19)` spawn (and only if it succeeded), gated
    /// only on `slot != 0`, and `call 0x7c710` (`flags |= 0x400`)
    /// soft-kills whatever holds it — a stranger (mc2l22 t=23012: a
    /// loose `(10,39)` 1000-mana sphere in slot 986, the register
    /// stale since t=18692), or the brand-new column itself when it
    /// was minted into the stale slot. The port's former
    /// `flags & 0x400 == 0` (both writes) and `old != col` guards were
    /// invented; the kick's `set_arc10` wrote the vortex's @0x10 home
    /// on every class — see [`Gen::mc2_write_raw10`].
    pub(crate) fn mc2_summit18_tick(&mut self, i: usize, ctx: &MobCtx) {
        if self.arc10(i) > 2500 {
            let r = self.ent_rand(i);
            if r % 0x64 == 0 && self.erupting == 0 {
                let (x, y, z) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z)
                };
                {
                    let gz = self.ground_z(x, y) as i16;
                    self.ent[i].z = gz;
                    if z != gz {
                        self.ent[i].flags |= 0x400;
                        return;
                    }
                }
                self.set_arc10(i, 0);
            }
        }
        let t = self.arc10(i);
        let pulse = (t < 128 && t & 0xF != 0 && self.ent_rand(i) % 5 == 0) || t == 0;
        if pulse {
            let (x, y, z, id) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24)
            };
            // EF:23926-31 — the pulse re-snaps to getTerrainAlt and
            // a moved ground DESPAWNS the vortex (retail's own
            // controller dies at mc2l24 t=17 when the dome, after it
            // in the walk, cuts the crater under it). Unconditional:
            // the measured channel carries the raised summit.
            let gz = self.ground_z(x, y) as i16;
            self.ent[i].z = gz;
            if z != gz {
                self.ent[i].flags |= 0x400;
                self.erupting = 0;
                return;
            }
            if t == 0 {
                // PATCH `volcano_register_revalidate` (retail bug, see
                // the patch doc): each register write lands only on the
                // record the register was meant to name.
                let revalidate = ctx.patches.volcano_register_revalidate
                    && (!ctx.strict || crate::engine::features::force_volcano_guard());
                let prev = self.erupting as usize;
                if prev != 0
                    && prev < self.ent.len()
                    && (!revalidate
                        || prev != i
                            && self.ent[prev].class64 == 10
                            && self.ent[prev].model65 == 18)
                {
                    // `movl $0xfa,0x10(%eax)` — fast-expire the old
                    // vortex, or stamp whatever inherited its slot.
                    self.mc2_write_raw10(prev, 250);
                }
                self.erupting = i as u16;
                if let Some(col) = self.mc2_spawn_fire_spray(x, y, gz) {
                    self.ent[col].id24 = id;
                    let old = self.plume as usize;
                    if old != 0
                        && old < self.ent.len()
                        && (!revalidate
                            || old != col
                                && self.ent[old].class64 == 10
                                && self.ent[old].model65 == 19)
                    {
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
            // `a1x->yaw_0x1C_28 += 1280` is UNMASKED (i16 wrap); only
            // the bolt copy is folded to 11 bits (`HIBYTE(v11) &= 7`).
            let yaw = self.ent[i].f30.wrapping_add(1280);
            self.ent[i].f30 = yaw;
            // ⭐ RE-READ, NOT `t`: `v10 = a1x->dword_0x10_16` (file
            // 0x5743C `mov 0x10(%ebx),%esi`) and the despawn's
            // `cmpl $0x7f,0x10(%ebx)` (0x574CB) read memory again, so a
            // stale kick that named THIS slot (a self-kick) mints no
            // bolt and despawns the new vortex on its first tick.
            let t_now = self.arc10(i);
            if t_now == 0
                && let Some(b) = self.mc2_spawn_bolt(x, y, gz)
            {
                let byaw = yaw & 0x7FF; // HIBYTE(v11) &= 7 (EF:23987)
                let mut aim = (x, y, gz);
                Self::polar_step(&mut aim, byaw, 0, 1536);
                // EF:23990 — the dest triple's z is the TERRAIN at
                // the aim point, not the launch z.
                let aim_z = self.ground_z(aim.0, aim.1) as i16;
                let e = &mut self.ent[b];
                e.id24 = id;
                e.f32 = (-386i16) as u16; // steep upward pitch
                e.f68 = 10;
                e.f69 = 17; // impact = the (10,17) meteor
                e.act_life = 1;
                e.f30 = byaw;
                // ⭐ NO @0x20 / @0x22 STORE IN `sub_32A70` EITHER —
                // the (9,0) block writes yaw, pitch, b43, b44, life
                // and the @0x9A vector and stops (shipped bytes on
                // [`crate::mc2::cast::no_spawn_roll_absence`]). The
                // corpus signature is the constant `port 1280` on the
                // `(9,0) roll` lane: the vortex's own `yaw += 1280`.
                if crate::mc2::cast::no_spawn_roll_absence() {
                    e.f34 = byaw;
                    e.f36 = e.f32;
                }
                e.dest_x = aim.0;
                e.dest_y = aim.1;
                e.site_z = aim_z;
            }
            if t_now >= 127 {
                // Retail's `dword_0x10_16++` sits OUTSIDE the pulse
                // block (EF:23997): this despawn arm FALLS THROUGH to
                // the counter increment (only the >2500 arm and the
                // ground-move despawn return before it).
                self.ent[i].flags |= 0x400;
                self.erupting = 0;
            }
        }
        // Retail's counter is the i32 `dword_0x10_16` and simply keeps
        // counting past 32767 (the self-latched controller never
        // restarts — the 1-in-100 roll is gated on the vortex register
        // it holds — so it idles until the endgame teardown, OPEN).
        // [`Ent::summit10`] is that 32-bit home; `f26` keeps the
        // saturating i16 mirror, which is behaviourally identical
        // because every gate above reads `> 2500`, `< 128`, `& 0xF`
        // (dead above 127), `== 0` or `>= 127`. See
        // [`crate::engine::features::no_mc2_summit_arc_wide`].
        let n = self.arc10(i).wrapping_add(1);
        self.set_arc10(i, n);
    }

    /// Retail's `dword_0x10_16` on a SUMMIT CONTROLLER, read 32 bits
    /// wide. See [`crate::engine::features::no_mc2_summit_arc_wide`].
    #[inline]
    pub(crate) fn arc10(&self, i: usize) -> i32 {
        if crate::engine::features::no_mc2_summit_arc_wide() {
            self.ent[i].f26 as i32
        } else {
            self.ent[i].summit10.0
        }
    }

    /// Write retail's `dword_0x10_16` on a SUMMIT CONTROLLER: the wide
    /// home takes the exact value and `f26` — the class-wide `i16`
    /// home every other reader and every `import_ent_mc2` arm uses —
    /// takes the saturating mirror.
    #[inline]
    pub(crate) fn set_arc10(&mut self, i: usize, v: i32) {
        if !crate::engine::features::no_mc2_summit_arc_wide() {
            self.ent[i].summit10 = crate::engine::features::Summit10(v);
        }
        // The pre-dig `saturating_add(1)` is exactly this clamp, so the
        // `f26` half is byte-identical in both arms.
        self.ent[i].f26 = v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    /// Write retail's raw `dword_0x10_16` (@0x10) = `v` into whatever
    /// port field is @0x10's home for record `i`'s CURRENT class/model
    /// — the seat `import_ent_mc2` (engine/world/conformance.rs) fills
    /// from `r.scratch10`, arm for arm, and the one
    /// `port_ent_lanes_mc2` publishes on the `scratch10` lane. Returns
    /// `false` (and writes nothing) where the port has NO home for
    /// @0x10 on that record — a registered gap: retail's store lands in
    /// a word the port does not model.
    ///
    /// | record | @0x10 home | import arm |
    /// |---|---|---|
    /// | `(10,18)`/`(10,91)` summits | `summit10` + `f26` mirror ([`Gen::set_arc10`]) | `summit10: … matches!(r.model40, 18 \| 91)` + `_ => r.scratch10 as i16` |
    /// | `(10,79)` defender piece | `f44` | `if r.class3f == 10 && r.model40 == 79 { … e.f44 = r.scratch10 as u16` |
    /// | `(10,54\|69)` auras, `(9,17)` possession bolt | `f26` = `isqrt(@0x10) >> 8` (tile radius) | `(10, 54 \| 69) =>` / `(9, 17) if !no_posses_reach_root()` |
    /// | class 15, action 78 (detached jar) | `f50` (`f26` under `MGC_NO_MC2_STOLEN_ARC_KEEPS_CAST_STATE`) | `if r.action45 == 78 { … e.f50 = r.scratch10` |
    /// | class 15, any other action | **none** (`f26` is @0x2E) | `(15, _) => r.f2e` |
    /// | `(5,21)` devil | **none** (`f26` is @0x44) | `(5, 21) => r.b44 as i16` |
    /// | class 5, other | `f26` (bar the legacy StageVar2 lease arms under `MGC_NO_SUMMON_LEASE_FIELD`) | `(5, _) if !no_summon_lease_field() && r.model40 != 21 => r.scratch10` |
    /// | everything else (castle `(3,2)`: its LEVEL) | `f26` | `_ => r.scratch10 as i16` |
    ///
    /// The out-of-pool human carpet's @0x10 (`Player::mc2_respawn_timer`)
    /// has no pool slot, so no register can name it.
    pub(crate) fn mc2_write_raw10(&mut self, i: usize, v: i32) -> bool {
        let e = &self.ent[i];
        let (c, m, sv2, act) = (e.class64, e.model65, e.site_z, e.tick70);
        let lease_kind = matches!(sv2, 12 | 13 | 14 | 16 | 17);
        match (c, m) {
            (10, 18 | 91) => self.set_arc10(i, v),
            (10, 79) => self.ent[i].f44 = v as u16,
            (10, 54 | 69) => self.ent[i].f26 = (Gen::isqrt(v.max(0) as u32) >> 8) as i16,
            (9, 17) if !crate::mc2::cast::no_posses_reach_root() => {
                self.ent[i].f26 = (Gen::isqrt(v.max(0) as u32) >> 8) as i16
            }
            (15, _) => {
                if act != 78 {
                    return false;
                }
                if crate::mc2::cast::no_mc2_stolen_arc_keeps_cast_state() {
                    self.ent[i].f26 = v as i16;
                } else {
                    self.ent[i].f50 = v as i16;
                }
            }
            (5, 21) => return false,
            (5, _) if !crate::engine::features::no_summon_lease_field() => {
                self.ent[i].f26 = v as i16
            }
            // The legacy arms `MGC_NO_SUMMON_LEASE_FIELD` restores, in
            // the import's order: a lease-kind record whose `f26` holds
            // @0x2E keeps no @0x10.
            (5, _) if lease_kind && act % 8 == 7 && !crate::mc2::mobs::no_summon_lease_split() => {
                return false;
            }
            (5, 0 | 19 | 27) if !matches!(sv2, 16 | 17) => self.ent[i].f26 = v as i16,
            (5, 10) => self.ent[i].f26 = v as i16,
            (5, _) if crate::mc2::mobs::no_summon_lease_split() && lease_kind => return false,
            _ => self.ent[i].f26 = v as i16,
        }
        true
    }

    /// `sub_32CF0` (EF:24007, action 98) — the apocalypse MANA RAIN:
    /// every tick launch THREE (10,39) collectible spheres with the
    /// exact 5-draw arming order per sphere (speed % 0x300 clamped
    /// [64,768]; apex (r&0x7F)+128; color roll % 9 − 1; mana
    /// % 0xA00 + 1; yaw r & 0x7FF), life 140, scattered one launch
    /// step from the summit and dropped at ground + 96. Never
    /// despawns itself (retail relies on the endgame teardown, OPEN).
    ///
    /// APPROX register: the horizontal launch velocity rides the ball
    /// tick's native `dest_x/dest_y` throw deltas (retail stores the
    /// same delta on `axis_0x9A`, ±64/tick clamped in the mover); the
    /// VERTICAL launch is faithful — `word_0x2C_44` → f46 (the ball
    /// tick's z-vel lane), so the sphere arcs up then falls; the
    /// color-variant sprite roll keeps its draw but
    /// the neutral ball family renders (ball_resize); retail expires
    /// its spheres via `byte[1] |= 0x20` + life 140 — the decay
    /// channel (ball_tick's decay tail, flag bit 13 — fade bits
    /// 24→23, expire at 0, no merge-initiate) bounds the rain at ~420
    /// live spheres like retail; the 200-slot free cushion is a
    /// pool-exhaustion belt (deliberate: retail has none); the
    /// every-other-tick 26-row spell-XP flood (`sub_6D8B0`, xp =
    /// tier-2 `xpos1` / 512) is LANDED — see
    /// [`no_mc2_rain_spell_xp`] for the shipped bytes; every XP
    /// source here is wired through the `mc2_cast_xp` mail.
    pub(crate) fn mc2_summit91_tick(&mut self, i: usize) {
        let armed = !no_mc2_rain_arm();
        for _ in 0..3 {
            let (x, y, z) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            let speed = (self.ent_rand(i) % 0x300).clamp(64, 768) as i16;
            // The apex = the sphere's VERTICAL launch velocity
            // `word_0x2C_44 = (rand & 0x7F) + 128` (EF:24052): the ball
            // tick reads it as the z-velocity lane (f46 — the sphere
            // import maps word_0x2C_44 → f46), so the sphere ARCS up
            // ~128..255/tick then falls under −16/tick gravity. Without
            // it the rain sprayed flat along the ground.
            let apex = (self.ent_rand(i) & 0x7F) as i16 + 128;
            let color = (self.ent_rand(i) % 9) as i32 - 1; // `v7 = rand % 9 - 1`
            let mana = (self.ent_rand(i) % 0xA00) as i32 + 1;
            let yaw = (self.ent_rand(i) & 0x7FF) as u16;
            if self.free.len() <= 200 {
                continue; // the pool cushion (deliberate: retail has none)
            }
            // ⭐ THE THROW IS A SCRATCH VECTOR, NOT A DISPLACEMENT.
            // `v1x->axis_0x9A_154x = a1x->position; MoveEntity_57FA0(
            // &axis_0x9A, yaw, 0, actSpeed); axis_0x9A.x -= pos.x;
            // axis_0x9A.y -= pos.y;` (EF:24065-70) steps a COPY of the
            // summit position and keeps only the delta — the sphere
            // itself is born AT the summit, and `axis_0x9A.z` keeps the
            // summit's z (only x/y are differenced), which is why every
            // recorded rain sphere reads `dest_z` = the emitter z.
            let mut lp = (x, y, z);
            Self::polar_step(&mut lp, yaw, 0, speed);
            // `v1x->position.z = getTerrainAlt(&v1x->position) + 96`
            // (EF:24071) — sampled at the SPHERE's own (= the summit's)
            // x/y, with no clamp of any kind.
            let (sx, sy, sz) = if armed {
                (x, y, (self.ground_z(x, y) + 96) as i16)
            } else {
                (lp.0, lp.1, ((self.ground_z(lp.0, lp.1) + 96) as i16).max(z))
            };
            if let Some(s) = self.spawn_mana_ball(sx, sy, sz) {
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
                e.f46 = apex; // vertical launch (word_0x2C_44, EF:24052)
                e.dest_x = lp.0.wrapping_sub(x); // the throw velocity delta
                e.dest_y = lp.1.wrapping_sub(y);
                if armed {
                    // `actSpeed_0x82_130` (EF:24042-48), `pitch = 0`,
                    // `yaw = roll = rand & 0x7FF` (EF:24063-65) and
                    // `axis_0x9A.z` (EF:24067) — the four arming
                    // writes the port dropped, which is why every
                    // recorded rain sphere read heading 0 / speed 32
                    // (the `CreateManaSphere_500C0` ctor defaults).
                    e.f126 = speed;
                    e.f30 = yaw;
                    e.f32 = 0;
                    e.f34 = yaw;
                    e.site_z = z;
                }
                if armed {
                    // `SetEntityIndexAndRot_49CD0(v1x, v8 + v9)`
                    // (EF:24060-61): the rain picks its own family by
                    // a RANDOM COLOUR ROLL and takes the particle-param
                    // extents — it does NOT run
                    // `SetManaSphereColorAndRot_36920`'s owner derive
                    // + per-size rotation constant. `v9 =
                    // GetManaSphereIndexFromId_36A50(rand % 9 - 1)` =
                    // 52 on the -1 arm, else `105 + 8 *
                    // TransformPlayerColorIndex(colour)`.
                    let base: u16 = if color < 0 {
                        52
                    } else {
                        105 + 8 * crate::mc2::color_art(color as u8) as u16
                    };
                    // `manaSphereSizeTable_DB538` (EF:2600), the same
                    // eight-class ladder `ball_resize` walks.
                    const SIZES: [i32; 7] = [256, 512, 1024, 2048, 4096, 9192, 18384];
                    let mut size = 7u16;
                    for (k, t) in SIZES.iter().enumerate() {
                        if mana <= *t {
                            size = k as u16;
                            break;
                        }
                    }
                    self.mc2_set_sprite(s, base + size);
                } else {
                    self.ball_resize(s);
                }
            }
        }
        // ⭐⭐⭐ THE APOCALYPSE RAIN PAYS THE WHOLE SPELLBOOK, EVERY
        // OTHER TICK — `sub_32CF0`'s second half (EF:24096-24112; the
        // shipped bytes are quoted on [`no_mc2_rain_spell_xp`]). It is
        // the last MC2 `sub_6D8B0` caller with no port home, and it is
        // a FLOOD, not a credit: 26 rows of `SPELLS[s].tiers[2].xpos1
        // / 512` to the LOCAL player on every even `byte_0x3E_62`.
        // Three things make it unlike every other XP source here:
        // the owner is the local player's entity out of
        // `D41A0_0.array_0x2BDE[LevelIndex]`, NOT the summit's
        // `id_0x1A_26` (the rain is never player-owned); the phase
        // gate is the entity's own `f63` parity, which is why the
        // measured cadence is every second tick and not every tick;
        // and the amount is a per-row CONSTANT off the SPELLS table,
        // so it is nonzero for rows the player has never cast.
        // WITNESS mc2l24-crazy, `MGC_RAW_SHADOW=1 verify-deltas`: the
        // (10,91) summit lights at t=73864 and from that tick retail
        // steps all 25 non-castle rows every second boundary
        // (`xp_vol[0]` +23, `[1]` +9, `[9]` +19, `[18]` +15 …) while
        // the port stepped none — 65,003 of the take's 65,030 human
        // `xp_vol` rows, and the `levels[20]` tier-up at t=74066 under
        // them. See [`no_mc2_rain_spell_xp`].
        if !no_mc2_rain_spell_xp() && self.ent[i].f63 & 1 == 0 {
            for s in 0..26usize {
                let Some(row) = self.assets.spells.get(s).copied() else {
                    break;
                };
                // `sar $0x9` after the `sbb` sign fixup == C's
                // truncate-toward-zero divide, == Rust's `i32 / i32`.
                let amt = row.tiers[2].xpos1 / 512;
                // Owner-blind: retail pushes the LOCAL player's entity,
                // which is `PLAYER_TARGET` on this side; `mc2_award_xp`
                // re-applies `sub_6D8B0`'s own class-3/model-0 guard.
                self.mc2_cast_xp
                    .0
                    .push((crate::mc1::mobs::PLAYER_TARGET, s as u16, amt));
            }
        }
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

    /// Flat 100-height CAVE world (a ceiling plane makes
    /// [`Gen::is_cave`] true, which the (10,89) ctor gates on).
    fn flat_cave_gen() -> Gen {
        use crate::chassis::ChassisParams;
        use crate::engine::features::{FeatureAssets, Planes};
        use crate::verbs::VerbSet;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: vec![200; 0x10000],
        };
        let assets = FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
        };
        Gen::new(planes, assets, 1, ChassisParams::MC2, VerbSet::MC2)
    }

    /// ⭐ THE SUMMIT ARC COUNTER RUNS PAST AN i16, AND THE GATES DO
    /// NOT CARE (round 148, dig w148o;
    /// [`crate::engine::features::no_mc2_summit_arc_wide`]).
    ///
    /// `sub_32A70`'s counter is the 32-bit `dword_0x10_16` and its
    /// last statement is a bare `++` with no cap; the port's `i16`
    /// `f26` parked on 32,767 while retail reached 79,064 on
    /// mc2l24-crazy. Drive a latched summit from just under the `i16`
    /// ceiling and the wide home keeps counting.
    ///
    /// ⛔ UNIT TEST, NOT A FIXTURE: the lane is the raw shadow's
    /// `scratch10`, ungraded by construction, and replay re-imports
    /// every pool record at the anchor.
    ///
    /// POSITIVE CONTROL: the counter really does step in this rig (a
    /// fresh summit goes 0 -> 1), and the SATURATING `f26` mirror the
    /// gates read is identical in both arms. REVERSION PROOF: this
    /// test fails with `MGC_NO_MC2_SUMMIT_ARC_WIDE=1`, which parks the
    /// published count at 32,767.
    #[test]
    fn the_summit_arc_counter_is_32_bit() {
        let mut g = flat_cave_gen();
        let (px, py) = (40u16 << 8, 40u16 << 8);
        let p = (px, py, g.ground_z(px, py) as i16);

        // POSITIVE CONTROL: a fresh summit's counter steps at all.
        let fresh = g.mc2_spawn_summit18(p.0, p.1, p.2).expect("summit18");
        assert_eq!(g.arc10(fresh), 0, "the ctor's `movl $0x0,0x10`");
        g.mc2_summit18_tick(fresh, &test_ctx(false, false));
        assert_eq!(g.arc10(fresh), 1, "the tail `dword_0x10_16++`");

        // A LATCHED summit past the restart gate: `erupting` holds its
        // own slot, so the `> 2500` arm's 1-in-100 roll can never fire
        // and it only ever counts.
        let i = g.mc2_spawn_summit18(p.0, p.1, p.2).expect("summit18");
        g.erupting = i as u16;
        g.set_arc10(i, 32_760);
        for _ in 0..20 {
            g.mc2_summit18_tick(i, &test_ctx(false, false));
        }
        assert_eq!(
            g.arc10(i),
            32_780,
            "retail's `int` keeps counting past 32,767"
        );
        // …and the gates it feeds answer the same as they did at the
        // i16 ceiling: no pulse, no despawn, no latch release.
        assert_eq!(
            g.ent[i].flags & 0x400,
            0,
            "a latched summit does not despawn"
        );
        assert_eq!(g.erupting, i as u16, "it keeps the vortex register");
        assert_eq!(
            g.ent[i].f26,
            i16::MAX,
            "`f26` keeps the saturating mirror every gate reads"
        );
    }

    fn test_ctx(patched: bool, strict: bool) -> MobCtx {
        let mut patches = crate::patches::WorldPatches::RETAIL;
        patches.volcano_register_revalidate = patched;
        MobCtx {
            px: 0,
            py: 0,
            pz: 100,
            pyaw: 0,
            pmana: 0,
            pmana_max: 0,
            pdead: false,
            pdead_top: false,
            strict,
            patches,
            mc2_turn: 0,
        }
    }

    /// The three arms every register test runs: retail, patched, and
    /// patched-under-strict (which must pin retail).
    const ARMS: [(bool, bool); 3] = [(false, false), (true, false), (true, true)];

    /// A fresh `(10,18)` at its eruption start (`@0x10 == 0`).
    fn fresh_vortex(g: &mut Gen) -> usize {
        let (px, py) = (40u16 << 8, 40u16 << 8);
        let z = g.ground_z(px, py) as i16;
        let v = g.mc2_spawn_summit18(px, py, z).expect("summit18");
        assert_eq!(g.arc10(v), 0);
        v
    }

    /// ⭐ THE MC2 ERUPTION KICK IS BLIND AND WRITES @0x10 (round 158,
    /// dig w158e; PATCH `volcano_register_revalidate`). `sub_32A70`'s
    /// `movl $0xfa,0x10(%eax)` (`NETHERW.EXE` file 0x57379, gated only
    /// by the `jbe` at 0x57377) lands on whatever holds the stale
    /// `word_0x31` slot; a CASTLE's @0x10 is its level (`f26`). Retail
    /// and strict: level 250. Patched: untouched. POSITIVE CONTROL: a
    /// real old `(10,18)` is fast-expired to 250 in every arm.
    #[test]
    fn the_mc2_eruption_kick_levels_a_castle_unless_patched() {
        for (patched, strict) in ARMS {
            let mut g = flat_cave_gen();
            let castle = g.new_event().expect("castle slot");
            g.ent[castle].class64 = 3;
            g.ent[castle].model65 = 2;
            g.ent[castle].f26 = 3;
            let v = fresh_vortex(&mut g);
            g.erupting = castle as u16;
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            assert_eq!(g.erupting, v as u16, "the start registers the new vortex");
            let want = if patched && !strict { 3 } else { 250 };
            assert_eq!(
                g.ent[castle].f26, want,
                "castle level, patched={patched} strict={strict}"
            );

            // POSITIVE CONTROL: the real previous vortex is kicked.
            let mut g = flat_cave_gen();
            let old = fresh_vortex(&mut g);
            g.set_arc10(old, 60);
            let v = fresh_vortex(&mut g);
            g.erupting = old as u16;
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            assert_eq!(g.arc10(old), 250, "the old vortex is fast-expired");
        }
    }

    /// ⭐ THE MC2 COLUMN KILL IS BLIND (round 158, dig w158e). File
    /// 0x573BE-0x573DE: `word_0x33` is read after the new `(10,19)`
    /// spawn, gated only on `slot != 0`, and `flags |= 0x400` lands on
    /// whatever holds it. WITNESS mc2l22 t=23012: a loose `(10,39)`
    /// 1000-mana sphere (slot 986). Retail/strict: the sphere is
    /// soft-killed. Patched: it survives. POSITIVE CONTROL: the real
    /// old `(10,19)` column is killed in every arm.
    #[test]
    fn the_mc2_column_kill_spares_a_stranger_when_patched() {
        for (patched, strict) in ARMS {
            let mut g = flat_cave_gen();
            let sphere = g.new_event().expect("sphere slot");
            g.ent[sphere].class64 = 10;
            g.ent[sphere].model65 = 39;
            let v = fresh_vortex(&mut g);
            g.plume = sphere as u16;
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            assert_ne!(
                g.plume, sphere as u16,
                "the register moved on to the new column"
            );
            let killed = g.ent[sphere].flags & 0x400 != 0;
            assert_eq!(
                killed,
                !patched || strict,
                "sphere, patched={patched} strict={strict}"
            );

            // POSITIVE CONTROL: the real previous column dies.
            let mut g = flat_cave_gen();
            let z = g.ground_z(40 << 8, 40 << 8) as i16;
            let col = g.mc2_spawn_fire_spray(40 << 8, 40 << 8, z).expect("column");
            let v = fresh_vortex(&mut g);
            g.plume = col as u16;
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            assert_ne!(g.ent[col].flags & 0x400, 0, "the old column is killed");
        }
    }

    /// ⭐ THE NEW COLUMN KILLS ITSELF WHEN IT IS BORN IN THE STALE SLOT
    /// (round 158, dig w158e). Retail reads `word_0x33` AFTER the spawn
    /// (file 0x573BE) with no identity test, so when the allocator
    /// hands the new `(10,19)` the very slot the stale register names,
    /// `call 0x7c710` disables the brand-new column (and the register
    /// then names it). The port's `old != col` guard was invented.
    /// Patched: the new column lives.
    #[test]
    fn a_new_mc2_column_in_the_stale_slot_kills_itself_unless_patched() {
        for (patched, strict) in ARMS {
            let mut g = flat_cave_gen();
            let v = fresh_vortex(&mut g);
            // The stale slot, freed: the next allocation takes it.
            let stale = g.new_event().expect("stale slot");
            g.free_entity(stale);
            g.plume = stale as u16;
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            assert_eq!(
                g.plume, stale as u16,
                "the new column was minted into the stale slot"
            );
            assert_eq!((g.ent[stale].class64, g.ent[stale].model65), (10, 19));
            let killed = g.ent[stale].flags & 0x400 != 0;
            assert_eq!(
                killed,
                !patched || strict,
                "new column, patched={patched} strict={strict}"
            );
        }
    }

    /// ⭐ THE MC2 SELF-KICK (round 158, dig w158e). A stale `word_0x31`
    /// naming the slot just recycled into THIS vortex stamps its own
    /// `@0x10 = 250`, and retail RE-READS it (file 0x5743C for the
    /// `(9,0)` bolt, 0x574CB for the `>= 127` despawn): no bolt, the
    /// vortex despawns and releases the register on its first tick,
    /// then `++` leaves 251. Patched: a normal eruption start.
    #[test]
    fn a_self_kicked_mc2_vortex_dies_unless_patched() {
        for (patched, strict) in ARMS {
            let mut g = flat_cave_gen();
            let v = fresh_vortex(&mut g);
            g.erupting = v as u16;
            let bolts = |g: &Gen| {
                g.ent
                    .iter()
                    .filter(|e| e.class64 == 9 && e.model65 == 0)
                    .count()
            };
            let before = bolts(&g);
            g.mc2_summit18_tick(v, &test_ctx(patched, strict));
            if patched && !strict {
                assert_eq!(g.arc10(v), 1, "patched: a normal start");
                assert_eq!(g.ent[v].flags & 0x400, 0);
                assert_eq!(g.erupting, v as u16);
                assert_eq!(bolts(&g), before + 1, "the start's (9,0) bolt");
            } else {
                assert_eq!(g.arc10(v), 251, "retail: self-kicked, then `++`");
                assert_ne!(g.ent[v].flags & 0x400, 0, "retail: despawned");
                assert_eq!(g.erupting, 0, "retail: the register is released");
                assert_eq!(bolts(&g), before, "retail: no bolt (the re-read @0x10)");
            }
        }
    }

    /// The @0x10 home table of [`Gen::mc2_write_raw10`] — each arm
    /// mirrors an `import_ent_mc2` seat for `r.scratch10`.
    #[test]
    fn the_raw10_write_lands_on_each_class_home() {
        let mut g = flat_cave_gen();
        let mk = |g: &mut Gen, c: u8, m: u8| {
            let i = g.new_event().expect("slot");
            g.ent[i].class64 = c;
            g.ent[i].model65 = m;
            i
        };
        // Castle: `_ => r.scratch10` → f26 (its level).
        let c = mk(&mut g, 3, 2);
        assert!(g.mc2_write_raw10(c, 250));
        assert_eq!(g.ent[c].f26, 250);
        // (10,79) piece: `e.f44 = r.scratch10`; f26 is @0x4A.
        let p = mk(&mut g, 10, 79);
        g.ent[p].f26 = 7;
        assert!(g.mc2_write_raw10(p, 250));
        assert_eq!((g.ent[p].f44, g.ent[p].f26), (250, 7));
        // (10,54) aura: the tile radius `isqrt(@0x10) >> 8`.
        let a = mk(&mut g, 10, 54);
        g.ent[a].f26 = 14;
        assert!(g.mc2_write_raw10(a, (25 << 8) * (25 << 8)));
        assert_eq!(g.ent[a].f26, 25);
        // Class 15 off the detached arc: f26 is @0x2E — no home.
        let j = mk(&mut g, 15, 3);
        g.ent[j].tick70 = 10;
        g.ent[j].f26 = 9;
        assert!(!g.mc2_write_raw10(j, 250));
        assert_eq!(g.ent[j].f26, 9);
        // …on the detached arc (action 78) the counter is f50.
        g.ent[j].tick70 = 78;
        assert!(g.mc2_write_raw10(j, 250));
        assert_eq!((g.ent[j].f50, g.ent[j].f26), (250, 9));
        // (5,21) devil: f26 is @0x44 — no home.
        let d = mk(&mut g, 5, 21);
        g.ent[d].f26 = 4;
        assert!(!g.mc2_write_raw10(d, 250));
        assert_eq!(g.ent[d].f26, 4);
        // Summit: the wide home and its f26 mirror.
        let s = mk(&mut g, 10, 18);
        assert!(g.mc2_write_raw10(s, 250));
        assert_eq!((g.arc10(s), g.ent[s].f26), (250, 250));
    }

    /// ⭐ THE (10,91) APOCALYPSE-RAIN SUMMIT SEEDS `@0x2A` = 200 LIKE
    /// ITS (10,18) SIBLING (round 148, dig w148o;
    /// [`crate::engine::features::no_mc2_rain_2a_seed`]).
    ///
    /// Both ctors carry the identical `66 c7 40 2a c8 00
    /// movw $0xc8,0x2a(%eax)` — `sub_4EED0` at `NETHERW.EXE` file
    /// 0x736F0 and `sub_4EF30` at 0x73750 — but model 91 is not a
    /// `c10_2a_in_f140` member, so its port home is the uniform
    /// class-10 seat `f44`, which the port left at 0.
    ///
    /// ⛔ NATIVE-SPAWN ONLY, SO IT GETS A UNIT TEST.
    ///
    /// POSITIVE CONTROL: the (10,18) sibling's own `f140` seat, which
    /// this law does not touch. REVERSION PROOF: fails with
    /// `MGC_NO_MC2_RAIN_2A_SEED=1`.
    #[test]
    fn the_rain_summit_seeds_2a_like_its_vortex_sibling() {
        let mut g = flat_cave_gen();
        let p = (40u16 << 8, 40u16 << 8, 100i16);

        // POSITIVE CONTROL: model 18's @0x2A home (`f140`) is 200.
        let v = g.mc2_spawn_summit18(p.0, p.1, p.2).expect("summit18");
        assert_eq!(g.ent[v].f140, 200, "sub_4EED0 `movw $0xc8,0x2a`");

        let r = g.mc2_spawn_summit91(p.0, p.1, p.2).expect("summit91");
        assert_eq!((g.ent[r].class64, g.ent[r].model65), (10, 91));
        assert_eq!(g.ent[r].f44, 200, "sub_4EF30 `movw $0xc8,0x2a`");
    }

    /// ⭐⭐⭐ THE MC2 ALLOCATOR SEEDS `@0x2A`, NOT `@0x2C` — SO EVERY
    /// `@0x2C`-HOMED MODEL IS BORN WITH A ZERO THERE (round 148, dig
    /// w148d; [`crate::engine::features::no_mc2_alloc_2c_seed`]).
    ///
    /// `NewEvent_4A050` is `movw $0x64,0x2a` (`NETHERW.EXE` 0x6E940),
    /// where MC1's `NewEvent_372C0` is `movw $0x64,0x2c`
    /// (`CARPET.EXE` 0x4FB98) — the port's shared `Gen::new_event`
    /// reproduces MC1's store, and `f44` is `word_0x2C_44` on these
    /// six models, so each was born carrying retail's @0x2A default in
    /// retail's zero word. None of the six retail ctors makes any
    /// store to +0x2C.
    ///
    /// ⛔ NATIVE-SPAWN ONLY, SO IT GETS A UNIT TEST: the graded lanes
    /// are the raw shadow's `f2c` (ungraded by construction) and
    /// replay re-imports every pool record at the anchor.
    ///
    /// Non-vacuous by the `new_event` positive control below — the
    /// shared default really is 100 in this rig. With
    /// `MGC_NO_MC2_ALLOC_2C_SEED=1` all six read 100.
    #[test]
    fn every_2c_homed_mc2_model_is_born_with_a_zero_there() {
        let mut g = flat_cave_gen();
        // POSITIVE CONTROL: the shared MC1-shaped ctor default this
        // law overrides.
        let bare = g.new_event().expect("a bare record");
        assert_eq!(
            g.ent[bare].f44, 100,
            "`new_event` seeds NewEvent's 100 into the shared field"
        );

        let p = (40u16 << 8, 40u16 << 8, 100i16);
        let born: Vec<(&str, usize)> = vec![
            (
                "(10,9) dome",
                g.mc2_spawn_dome(p.0, p.1, p.2).expect("dome"),
            ),
            (
                "(10,18) summit vortex",
                g.mc2_spawn_summit18(p.0, p.1, p.2).expect("summit18"),
            ),
            (
                "(10,19) fire spray",
                g.mc2_spawn_fire_spray(p.0, p.1, p.2).expect("fire spray"),
            ),
            (
                "(10,67) flood",
                g.mc2_spawn_flood(p.0, p.1, p.2).expect("flood"),
            ),
            (
                "(10,71) fissure",
                g.mc2_spawn_fissure(p.0, p.1, p.2).expect("fissure"),
            ),
            (
                "(10,89) cave-in",
                g.mc2_spawn_cave_in(p.0, p.1, p.2).expect("cave-in"),
            ),
        ];
        for (name, i) in born {
            assert_eq!(g.ent[i].class64, 10, "{name} slot {i} is class 10");
            assert_eq!(
                g.ent[i].f44, 0,
                "{name} slot {i} (model {}) is born with word_0x2C_44 = 0",
                g.ent[i].model65
            );
        }
    }

    /// A (10,91) apocalypse summit on a flat world, with a SPELLS
    /// table whose 26 tier-2 `xpos1` values are all different, ticked
    /// once at the given `byte_0x3E_62` phase. Returns the XP mail
    /// the tick posted.
    ///
    /// ⛔ The XP book is re-imported at every conformance anchor, so
    /// the mail is the only thing a test can read here — the same
    /// contract as the impact-XP rig in `mc2::proj`.
    fn summit_rain_xp(phase: u8) -> Vec<(u16, u16, i32)> {
        use crate::chassis::ChassisParams;
        use crate::engine::features::{FeatureAssets, Planes};
        use crate::mc2::spells::{Mc2SpellRow, Mc2SubSpell};
        use crate::verbs::VerbSet;
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Row s carries tier-2 `xpos1` = `XPOS1[s]`: 26 DIFFERENT
        // values, one of them negative and none of them a multiple of
        // 512, so no single constant and no floor-instead-of-truncate
        // divide can satisfy the set.
        let spells: Vec<Mc2SpellRow> = (0..26)
            .map(|s| {
                let mut row = Mc2SpellRow::default();
                row.tiers[2] = Mc2SubSpell {
                    xpos1: xpos1_for(s),
                    ..Mc2SubSpell::default()
                };
                row
            })
            .collect();
        let assets = FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells,
            mc2_sprite_ext: Vec::new(),
        };
        let mut g = Gen::new(planes, assets, 1, ChassisParams::MC2, VerbSet::MC2);
        // ⚠ The summit MUST come out of `new_event()` — a hand-stamped
        // slot is recycled by the rain's own three sphere mints.
        let i = g.new_event().expect("summit slot");
        {
            let e = &mut g.ent[i];
            e.class64 = 10;
            e.model65 = 91;
            e.id24 = i as u16; // NOT the human: the flood is owner-blind
            e.max_life = 1000;
            e.act_life = 1000;
            e.f63 = phase;
        }
        g.link(i, 40 << 8, 40 << 8, 100);
        g.mc2_summit91_tick(i);
        g.mc2_cast_xp.0.clone()
    }

    fn xpos1_for(s: usize) -> i32 {
        if s == 7 {
            -1000 // the `sbb` sign fixup arm: -1000 / 512 == -1
        } else {
            513 * (s as i32 + 1) + 7
        }
    }

    /// `sub_32CF0`'s 26-row spell-XP flood: every EVEN `byte_0x3E_62`
    /// phase the apocalypse rain pays the LOCAL player
    /// `SPELLS[s].tiers[2].xpos1 / 512` on all 26 rows, and an ODD
    /// phase pays nothing (NETHERW.EXE file 0x576DD-0x5772D, quoted on
    /// [`no_mc2_rain_spell_xp`]).
    ///
    /// Non-vacuous by construction: 26 distinct amounts including a
    /// negative one, plus the empty odd-phase arm, out of one rig
    /// whose only variable is the phase byte. With
    /// `MGC_NO_MC2_RAIN_SPELL_XP=1` the even arm mails nothing and
    /// this fails on the first row.
    #[test]
    fn the_apocalypse_rain_floods_all_26_spell_rows_on_an_even_phase() {
        let mail = summit_rain_xp(0);
        assert_eq!(mail.len(), 26, "one row per spell: {mail:?}");
        for (s, row) in mail.iter().enumerate() {
            assert_eq!(
                *row,
                (
                    crate::mc1::mobs::PLAYER_TARGET,
                    s as u16,
                    xpos1_for(s) / 512
                ),
                "spell {s}"
            );
        }
        // The rounding is retail's truncate-toward-zero, not a floor:
        // spell 7's -1000 pays -1, and every other row drops its
        // remainder.
        assert_eq!(mail[7].2, -1);
        assert_eq!(mail[0].2, 1);
        assert_eq!(mail[25].2, 26);
        // ODD phase: `testb $0x1,0x3e(%eax) / jne` the epilogue.
        assert!(
            summit_rain_xp(1).is_empty(),
            "an odd byte_0x3E_62 phase pays nothing"
        );
    }
}
