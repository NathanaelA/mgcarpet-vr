//! MC2 (10,67)=0x43 FLOOD/QUAKE, Phase 4.3 — the three-action
//! terrain-morph quake: action 72 (`sub_39040`) raises a sin-profile
//! dome ring with a sinking crater center and converts the footprint
//! to lava, action 73 (`sub_396A0`) holds the entity-shove while life
//! runs out, action 74 (`sub_396D0`) settles the terrain back and
//! despawns. Trace bank:
//! docs/traces/mc2-class10-tail-helper-closure.md §1 (the phase
//! machine) + mc2-class10-m67-flood-helpers.md (every helper
//! VERBATIM; its three corrections to the parent doc are followed
//! here) (`EF:` = remc2 EventsFunctions.cpp, `Terrain:` =
//! engine/Terrain.cpp, `Maths:` = utilities/Maths.cpp).
//!
//! Entity-field homes follow the class-10 effect column: subSpell →
//! f140, `dword_0x10_16` countdown → f26, `byte_0x46_70` phase → f71,
//! dome-top reference `word_0x2C_44` → f44, grab owner `word_0x26_38`
//! → f40, grab timer `word_0x30_48` → f50 (the castle shake home —
//! retail's 30 write IS the blast shake). Retail's `dword_38519`
//! "object" list is the CLASS-3 list (the builder EF:39970-39985) —
//! the model-2 members the damage pass grabs are CASTLES — and
//! `dword_38527` is the class-10 MODEL-45 list (EF:40043-40052): the
//! quake ERASES overlapping village buildings.
//!
//! Flag decode (helpers doc §5): the `|= 0x100001` victim write =
//! byte[0] bit0 + byte[2] bit4. byte[0] bit0 in this band is the
//! TOSSED/handled latch (creatures normally carry it clear; the
//! shove filter skips victims with it set, and the action-74 release
//! clears it) — our home is [`F_TOSSED`] (retail's bit0 aliases our
//! "active" flag and cannot be shared). byte[2] bit4 = the grab
//! latch = [`super::mobs::F_NO_CORPSE`]'s retail bit — reusing it is
//! the authentic alias (quake victims leave no corpse).
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - `sub_6D8B0(id, 0x14, n)` spell-XP reports (EF:29367/:29436) land
//!   with Phase 4.2; counts computed and dropped, and the global
//!   objects-hit counter `x_DWORD_E9B90` (EF:28527) has no ported
//!   reader.
//! - The HUMAN player rides the whirlwind precedent: retail shoves
//!   the class-3 model-0 body toward the center with a z pull-down
//!   and pitch-512 spin (EF:29108/:29421) — our player lives outside
//!   the pool, so the pull rides the `player_knock` channel (the
//!   doomsday tractor-beam seam), the z pull and spin bank on the
//!   FlightVerb takeover seam, and the close-range 1-in-7 kill roll
//!   mails a kill-scale 32000 (retail adds the victim's `life+1` —
//!   the guaranteed kill — which Gen cannot read for the player).
//! - The rival-wizard pitch-512 spin (presentation: the body flip)
//!   is skipped; the damage roll is faithful.
//! - The action-74 release's local-player visibility juggle
//!   (EF:29118-29127: byte[0] bit0 set for the local wizard body,
//!   cleared for everyone else) is the draw latch — our release
//!   clears [`F_TOSSED`] for everyone (the observable single-player
//!   effect: victims become shoveable again).
//! - The deep-sink skip (`word_160_0xe_14 < -64`, EF:29106 — the
//!   victim's Type_160 z-velocity) has no ported home; the z pull
//!   always applies before the ground clamp.
//! - Cave arms (second-heightmap easing + the mapAngle bit-3 seal)
//!   defer to Phase 4.5 like every module before this one.
//! - `mana_0x90_144 = 0` in phase 1 (EF:28548) has no ported reader
//!   on this column and is skipped.

use super::morph::{auto_flat, dist2d};
use super::sin_lut::SIN_DB750;
use crate::mc1::combat::MailTarget;
use crate::mc1::features::{Gen, tile};
use crate::mc1::mobs::MobCtx;

/// Retail byte[0] bit0 in the quake band — the tossed/handled latch
/// `sub_3A200` sets (dword |= 0x100001) and the action-74 release
/// clears. A free high bit: 25..30 belong to the mobs/roster MC2
/// band, 29 is the whirlwind grab.
pub(crate) const F_TOSSED: u32 = 1 << 31;

/// The grab latch (retail byte[2] bit4) — the authentic alias of the
/// creature no-corpse bit (helpers doc §5 decode).
pub(crate) const F_QUAKE_GRAB: u32 = super::mobs::F_NO_CORPSE;

/// `sub_10590_terrain_tile_type` & 0x7F0000 (Terrain:2067, helpers
/// doc §7) — the damage pass's burnable-to-lava set: bits 20..22 =
/// types {10,11,12} (the water/lava-edge family) + bits 16..19 =
/// types {21,22,24}/{23}/{25,27}/{26} (the bridge/wall family).
/// DISTINCT from the phase-2/3 `sub_57450` predicate — keep both.
pub(crate) fn burn_flags(t: u8) -> bool {
    matches!(t, 10..=12 | 21..=27)
}

impl Gen {
    // ---- ctor + spawn seam -----------------------------------------------

    /// `sub_51730` (EF:37421) — the (10,67) flood/quake ctor: action
    /// 0x48 = 72, life 120, subSpell 20000, byte[0] = (&0xF6)|1,
    /// map-registered, AABB half-extents (4352, 4352) = ±17 tiles
    /// (the damage pass's overlap box). maxLife is NOT set (unlike
    /// the fissure ctor) — the flood never reads its own maxLife.
    /// No sprite (a terrain effect), no RNG.
    pub(crate) fn mc2_spawn_flood(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 67;
            e.tick70 = 72;
            e.act_life = 120;
            e.f140 = 20000;
            e.f71 = 0;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.mc2_shift_rot(i, 4352, 4352);
        Some(i)
    }

    // ---- terrain helpers ---------------------------------------------------

    /// `GetTerrainHeightFromSquare_48DF0` (EF:32605) — the 4-CORNER
    /// MEAN (truncating `>> 2`) of the box `(x,y)..(x+w,y+h)`. NOT a
    /// max, NOT a box scan (the helpers doc's correction #1).
    fn flood_corner_mean(&self, x: u8, y: u8, w: u8, h: u8) -> i32 {
        (self.t.height[tile(x, y)] as i32
            + self.t.height[tile(x.wrapping_add(w), y)] as i32
            + self.t.height[tile(x.wrapping_add(w), y.wrapping_add(h))] as i32
            + self.t.height[tile(x, y.wrapping_add(h))] as i32)
            >> 2
    }

    /// `sub_439A0` (Terrain:1459) — the MEAN-of-8-neighbours restore
    /// (correction #2: not sum-minus-extremes): gate on the low-3
    /// angle bits, ring sum `>> 3`, then the uint8-WRAP flatness
    /// ladder picks own height / half-blends / the full mean.
    fn flood_settle_cell(&self, x: u8, y: u8) -> u8 {
        let t = tile(x, y);
        let own = self.t.height[t];
        if self.t.angle[t] & 7 == 0 {
            return own;
        }
        let mut max = own;
        let mut min = own;
        let mut sum = 0u32;
        for (dx, dy) in [
            (0i8, -1i8),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ] {
            let h = self.t.height[tile(x.wrapping_add(dx as u8), y.wrapping_add(dy as u8))];
            max = max.max(h);
            min = min.min(h);
            sum += h as u32;
        }
        let mean = sum >> 3;
        if own.wrapping_sub(min) <= 4 {
            if max.wrapping_sub(own) <= 4 {
                own // flat: keep
            } else if max.wrapping_sub(own) <= 10 {
                ((own as u32 + mean) >> 1) as u8 // mild step up: half-blend
            } else {
                mean as u8
            }
        } else if own.wrapping_sub(min) <= 10 {
            ((mean + own as u32) >> 1) as u8 // a mild bump: half-blend
        } else {
            mean as u8 // incl. center-below-neighbours (u8 wrap fails both)
        }
    }

    /// The flood's inline per-cell shading (EF:28626-28654, byte-
    /// identical in the finisher :28962-28992): `H(NW) − H(SE) + 32`
    /// on the raw int, the 28/40 clamp bands, the night flip
    /// `64 − v`. Same formula as the retile pass C but computed
    /// in-place per cell as the sweep runs; on caves it also syncs
    /// bit3 — the sync-only form, NO ceiling pin (EF:28647-28654 /
    /// :28985-28992, byte-identical in both callers).
    fn flood_shade_cell(&mut self, x: u8, y: u8) {
        let nw = self.t.height[tile(x.wrapping_sub(1), y.wrapping_sub(1))] as i32;
        let se = self.t.height[tile(x.wrapping_add(1), y.wrapping_add(1))] as i32;
        let mut v = nw - se + 32;
        if v >= 28 {
            if v > 40 {
                v = (v & 7) + 40;
            }
        } else {
            v = (v & 3) + 28;
        }
        if self.mc2_night_shade.0 {
            v = 64 - v;
        }
        let t = tile(x, y);
        self.t.shading[t] = v as u8;
        if self.is_cave() {
            if self.t.ceiling[t] > self.t.height[t] {
                self.t.angle[t] &= !8;
            } else {
                self.t.angle[t] |= 8;
            }
        }
    }

    // ---- entity passes -------------------------------------------------------

    /// `sub_39FA0` (EF:29214) — the shove-victim filter, the
    /// class/model/flag decision ladder verbatim (helpers doc §2).
    /// `byte[0] & 0x21` = the tossed latch + invisible → our
    /// `F_TOSSED | 0x20`.
    fn flood_shovable(&self, i: usize, j: usize) -> bool {
        let e = &self.ent[j];
        let held = e.flags & (F_TOSSED | 0x20) != 0;
        match e.class64 {
            1 | 4 | 6 | 7 | 8 | 11 | 12 | 13 | 15 => false,
            2 => true,
            3 => match e.model65 {
                0 => e.id24 != self.ent[i].id24, // skip the CASTER's body
                1 => !held && e.id24 != self.ent[i].id24,
                2 => false, // castles never move
                _ => true,
            },
            5 => {
                !held
                    && e.tick70 != 232
                    && match e.model65 {
                        0x16 => false,
                        27 => !matches!(e.tick70, 233 | 234),
                        _ => true,
                    }
            }
            9 => matches!(e.model65, 0 | 13 | 14),
            10 => matches!(e.model65, 6 | 0x27 | 0x28 | 57),
            14 => !held && e.model65 != 1,
            _ => true, // classes 0 and >15 default shoveable
        }
    }

    /// `sub_3A200` (EF:29382) — the close-range shove callback: tag
    /// the victim (dword |= 0x100001 → [`F_TOSSED`] +
    /// [`F_QUAKE_GRAB`]), then on a 1-in-7 roll of the FLOOD's own
    /// RNG stream (forced for class-5 models 12/0x12, suppressed for
    /// model 27) mail the victim its own `life + 1` — the
    /// near-guaranteed kill. Victim gate `byte_0x38_56 & 1` → f28
    /// bit 0 (the cross-column damage contract). The class-3 model-0
    /// pitch-512 spin is presentation-skipped (module doc).
    fn flood_shove_hit(&mut self, i: usize, j: usize) {
        self.ent[j].flags |= F_TOSSED | F_QUAKE_GRAB;
        let (class, model) = (self.ent[j].class64, self.ent[j].model65);
        let mut forced = false;
        let mut suppressed = false;
        if class == 5 {
            match model {
                12 | 0x12 => forced = true,
                27 => suppressed = true,
                _ => {}
            }
        }
        let rolled = forced || self.ent_rand(i) % 7 == 0;
        if rolled && !suppressed && self.ent[j].f28 & 1 != 0 {
            // Retail adds life+1 with NO floor (EF:29435); the u32
            // mail clamps the never-in-practice life < -1 arm to 0
            // (G9m — the old .max(1) was invented).
            let amt = (self.ent[j].act_life + 1).max(0) as u32;
            let id = self.ent[i].id24;
            self.mail_write(MailTarget::Pool(j), 0, amt, id);
            // +1 per near-guaranteed kill (EF:29437) — F3.
            if id == crate::mc1::mobs::PLAYER_TARGET {
                self.mc2_cast_xp.0.push((id, 20, 1));
            }
        }
    }

    /// `sub_39B60` (EF:29011) — the radius entity-shove: a 26×26
    /// spatial-hash window (origin center−13) gated by the SQUARED
    /// disc `dist² < 0xA90000` (= 3328², 13 tiles;
    /// `EuclideanDistXY_584D0` returns dist² — correction #3's
    /// sibling); per victim passing the filter, in true range
    /// `dist < 3328` and below the ceiling `z − ref < 4096`:
    /// very-close victims (`dist ≤ 32` or `z − ref ≤ 96`) take the
    /// damage callback, the rest are walked TOWARD the center by
    /// `(3328−d)·128/3328` clamped [4,128] capped to d, pulled down
    /// `48·((4096−(z−ref))·256 >> 12) >> 8`, and ground-clamped. In
    /// action-74 mode every grabbed entity in the disc has its
    /// tossed + grab latches released.
    fn flood_shove(&mut self, i: usize, ctx: &MobCtx) {
        let (ex, ey, ez, id, refz, action74) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f44 as i32, e.tick70 == 74)
        };
        let cx = (ex.wrapping_add(128) >> 8) as u8;
        let cy = (ey.wrapping_add(128) >> 8) as u8;
        for row in 0..26u8 {
            let ty = cy.wrapping_sub(13).wrapping_add(row);
            for col in 0..26u8 {
                let tx = cx.wrapping_sub(13).wrapping_add(col);
                let (wx, wy) = ((tx as i32) << 8, (ty as i32) << 8);
                if Self::dist2_sq(ex, ey, wx as u16, wy as u16) >= 0xA9_0000 {
                    continue;
                }
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                while j != 0 {
                    let next = self.ent[j].next20 as usize;
                    if j != i && self.ent[j].flags & 0x400 == 0 {
                        let d = dist2d(ex, ey, self.ent[j].x as i32, self.ent[j].y as i32);
                        let v5 = self.ent[j].z as i32 - refz;
                        if self.flood_shovable(i, j) && d < 3328 && v5 < 4096 {
                            if d <= 32 || v5 <= 96 {
                                self.flood_shove_hit(i, j);
                            } else {
                                let mut v6 = (((3328 - d) << 8) / 3328) << 7 >> 8;
                                v6 = v6.clamp(4, 128).min(d);
                                let (vx, vy, vz) = {
                                    let e = &self.ent[j];
                                    (e.x, e.y, e.z)
                                };
                                let yaw = Self::angle_between(vx, vy, ex, ey);
                                let mut pos = (vx, vy, vz);
                                Self::polar_step(&mut pos, yaw, 0, v6 as i16);
                                let pull = (48 * (((4096 - v5) << 8) >> 12)) >> 8;
                                pos.2 = (pos.2 as i32 - pull) as i16;
                                let ground = self.ground_z(pos.0, pos.1);
                                if (pos.2 as i32) < ground {
                                    pos.2 = ground as i16;
                                }
                                self.move_relink(j, pos.0, pos.1, pos.2);
                            }
                        }
                        // The action-74 grab release (LABEL_25) runs
                        // for EVERY entity in the disc.
                        if action74 && self.ent[j].flags & F_QUAKE_GRAB != 0 {
                            self.ent[j].flags &= !(F_TOSSED | F_QUAKE_GRAB);
                        }
                    }
                    j = next;
                }
            }
        }
        let _ = ez;
        // The human player arm (module doc APPROX): the pull rides
        // the knock channel, the close band rolls the 1-in-7 kill.
        let pd = dist2d(ex, ey, ctx.px as i32, ctx.py as i32);
        let pv5 = ctx.pz as i32 - refz;
        if pd < 3328 && pv5 < 4096 {
            if pd <= 32 || pv5 <= 96 {
                if self.ent_rand(i) % 7 == 0 {
                    self.mail_write(MailTarget::Player, 0, 32000, id);
                }
            } else {
                let mut v6 = (((3328 - pd) << 8) / 3328) << 7 >> 8;
                v6 = v6.clamp(4, 128).min(pd);
                let toward = Self::angle_between(ctx.px, ctx.py, ex, ey);
                self.player_knock = (toward, v6 as i16);
            }
        }
    }

    /// `CompareAxisWithShift_10750` (EF:3726/3733, helpers doc §8) —
    /// XY-ONLY Minkowski AABB overlap, strict `<`, NO z term (our
    /// generic `ent_overlap` adds one — and the flood's z rides in
    /// HEIGHTMAP units, so the z test would falsely exclude
    /// everything). Shared by the whirlwind contact pass (mc2::tail)
    /// — the same retail helper.
    pub(crate) fn mc2_overlap_xy(&self, a: usize, b: usize) -> bool {
        let (ea, eb) = (&self.ent[a], &self.ent[b]);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        wd(ea.x, eb.x) < ea.f80 as i32 + eb.f80 as i32
            && wd(ea.y, eb.y) < ea.f82 as i32 + eb.f82 as i32
    }

    /// `sub_3A090` (EF:29316) — the one-shot damage/grab pass
    /// (phase-2 countdown step 5 and finisher phase 0): ERASE every
    /// overlapping village BUILDING — the `dword_38527` list is the
    /// class-10 MODEL-45 list (the builder EF:40043-40052), not a
    /// generic effect list — (life = −1, fontType = 0); GRAB every
    /// overlapping CASTLE (class-3 model-2 — the dword_38519 list):
    /// grab latch + 30-tick shake (`word_0x30_48` = our f50 castle-
    /// shake home) + owner = self slot + the subSpell (20000) damage
    /// mail — NO owner immunity: the caster's own castle takes it
    /// too (EF:29339-29348 has no id gate); then the 30×30
    /// `0x7F0000`-family terrain sweep to lava. The
    /// `sub_6D8B0(id, 0x14, 2n)` report banks with 4.2.
    fn flood_damage_pass(&mut self, i: usize) {
        let (id, amt) = (self.ent[i].id24, self.ent[i].f140 as u32);
        let mut buildings: Vec<usize> = Vec::new();
        let mut castles: Vec<usize> = Vec::new();
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if j == i || c.flags & 0x400 != 0 || c.class64 == 0 {
                continue;
            }
            if c.class64 == 10 && c.model65 == 45 && self.mc2_overlap_xy(i, j) {
                buildings.push(j);
            } else if c.class64 == 3
                && c.model65 == 2
                && c.act_life >= 0
                && self.mc2_overlap_xy(i, j)
            {
                castles.push(j);
            }
        }
        for j in buildings {
            self.ent[j].act_life = -1;
            self.ent[j].f46 = 0; // fontTypeIndex_0x3D_61
        }
        let castles_hit = castles.len() as i32;
        for j in castles {
            self.ent[j].flags |= F_QUAKE_GRAB;
            self.ent[j].f50 = 30; // the grab timer = the blast shake
            self.ent[j].f40 = i as u16; // owner = self slot
            self.mail_write(MailTarget::Pool(j), 0, amt, id);
        }
        // +2 per grabbed CASTLE (EF:29374 `v8 += 2`; buildings do
        // NOT count) — F3.
        if castles_hit != 0 && id == crate::mc1::mobs::PLAYER_TARGET {
            self.mc2_cast_xp.0.push((id, 20, 2 * castles_hit));
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                let t = tile(x, y);
                if burn_flags(self.t.tile_type[t]) {
                    self.t.tile_type[t] = 1;
                    self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                }
            }
        }
    }

    // ---- the phase machine ---------------------------------------------------

    /// `sub_39E40` (EF:29133) — the init probe: abort if ≥ 225 of
    /// the 30×30 footprint cells are open ground (type 0), or if
    /// another quake — class-10 model 0x2D (a building) in action
    /// 48/51, or another model-67 — is live within the 54×54 window.
    fn flood_probe(&mut self, i: usize) -> bool {
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let mut open = 0u32;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                if self.t.tile_type[tile(x, y)] == 0 {
                    open += 1;
                }
            }
        }
        if open >= 225 {
            return false;
        }
        for dy in 0..54u8 {
            let y = cy.wrapping_sub(27).wrapping_add(dy);
            for dx in 0..54u8 {
                let x = cx.wrapping_sub(27).wrapping_add(dx);
                let mut j = self.map_entity[tile(x, y)] as usize;
                while j != 0 {
                    let e = &self.ent[j];
                    if j != i
                        && e.class64 == 10
                        && (e.model65 == 67 || (e.model65 == 0x2D && matches!(e.tick70, 48 | 51)))
                    {
                        return false;
                    }
                    j = e.next20 as usize;
                }
            }
        }
        true
    }

    /// The phase-2 morph body (EF:28553-28701) — runs from phase 1's
    /// fall-through and every phase-2 tick. Returns terrain-dirty.
    fn flood_morph(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].f26 -= 1;
        let cd = self.ent[i].f26 as i32;
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let mut dirty = false;
        if cd <= 0 {
            self.ent[i].f71 = 3;
        } else {
            let (ex, ey, ez) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z as i32)
            };
            let mut relight = false;
            for dy in 0..30u8 {
                let y = cy.wrapping_sub(15).wrapping_add(dy);
                for dx in 0..30u8 {
                    let x = cx.wrapping_sub(15).wrapping_add(dx);
                    let (wx, wy) = ((x as i32) << 8, (y as i32) << 8);
                    let d = dist2d(ex, ey, wx, wy);
                    if d < 3840 {
                        let target = if d >= 2304 {
                            // OUTER ring: blend rim height → dome top
                            // on the raised cosine.
                            let yaw = Self::angle_between(ex, ey, wx as u16, wy as u16);
                            let mut rim = (ex, ey, 0i16);
                            Self::polar_step(&mut rim, yaw, 0, 3840);
                            let v11 = self.ground_z(rim.0, rim.1) >> 5;
                            if (self.ent[i].f44 as i32) < v11 {
                                self.ent[i].f44 = v11 as u16;
                            }
                            let cos =
                                SIN_DB750[0x200 + (((d - 2304) << 10) / 1536) as usize] as i64;
                            v11 - ((((0x10000 + cos) >> 1) * (v11 - (ez + 64)) as i64) >> 16) as i32
                        } else {
                            // INNER disc: the dome top with the
                            // center dip (the crater ring profile).
                            let cos = SIN_DB750[0x200 + (((2304 - d) << 9) / 2304) as usize] as i64;
                            ez + 64 - ((((0x10000 - cos) << 6) >> 16) as i32)
                        };
                        let t = tile(x, y);
                        let cur = self.t.height[t] as i32;
                        let h = ((target - cur) / cd + cur).clamp(1, 255);
                        self.t.height[t] = h as u8;
                        // Cave ceiling ease toward floor + 64
                        // clearance, /life, u8-truncated
                        // (EF:28604-28614).
                        if self.is_cave() {
                            let tgt = (h + 64).min(254);
                            let c = self.t.ceiling[t] as i32;
                            self.t.ceiling[t] = (c - (c - tgt) / cd) as u8;
                        }
                        if h <= ez + 64 && h >= ez + 6 * cd && auto_flat(self.t.tile_type[t]) {
                            relight = true;
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                    self.flood_shade_cell(x, y); // EVERY cell, in-place
                }
            }
            if cd == 5 {
                self.flood_damage_pass(i);
                relight = true;
            }
            if relight {
                self.mc2_retile_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
            }
            // The 2×2 crater floor (EF:28672-28696): drop the center
            // by 1/countdown of itself each tick.
            for dy in 0..2u8 {
                let y = cy.wrapping_sub(1).wrapping_add(dy);
                for dx in 0..2u8 {
                    let x = cx.wrapping_sub(1).wrapping_add(dx);
                    let t = tile(x, y);
                    let h = self.t.height[t] as i32;
                    self.t.height[t] = (h - h / cd).clamp(0, 255) as u8;
                    let s = if self.mc2_night_shade.0 { -31 } else { 31 };
                    self.t.shading[t] = (s / cd + 32) as u8;
                }
            }
            dirty = true;
        }
        if self.ent[i].f26 < 6 {
            self.flood_shove(i, ctx);
        }
        dirty
    }

    /// `sub_39040` (EF:28515) — the action-72 driver: life countdown
    /// with the finisher shortcut, then the phase switch — 0 probe,
    /// 1 the 18×18 4-corner-mean sample + arm (falls into 2), 2 the
    /// dome morph, 3 the lava/relight commit → action 73. Returns
    /// terrain-dirty.
    pub(crate) fn mc2_flood_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life <= 0 {
            self.ent[i].tick70 = 74;
            self.ent[i].f71 = 0;
            return false;
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        match self.ent[i].f71 {
            0 => {
                if self.flood_probe(i) {
                    self.ent[i].f71 = 1;
                } else {
                    self.ent[i].flags |= 0x400; // DisableEntityDrawing04
                }
                false
            }
            1 => {
                // Phase 1 (EF:28539-28552): z = corner-mean − 64,
                // ref = 32·(mean − 80) in world units — the mixed-
                // unit max update in the morph is retail's own quirk
                // (helpers doc OPEN-4), ported verbatim.
                let v5 = self.flood_corner_mean(cx.wrapping_sub(9), cy.wrapping_sub(9), 18, 18);
                self.ent[i].z = 0;
                self.ent[i].f44 = 0;
                if v5 > 64 {
                    self.ent[i].z = (v5 - 64) as i16;
                    if v5 - 64 > 16 {
                        self.ent[i].f44 = (32 * (v5 - 80)) as u16;
                    }
                }
                self.ent[i].f71 = 2;
                self.ent[i].f26 = 12;
                self.snd(64, i);
                self.flood_morph(i, ctx) // retail falls through
            }
            2 => self.flood_morph(i, ctx),
            3 => {
                // Phase 3 (EF:28702-28751): commit — everything
                // burnable (or type 8) in the 30×30 → lava, retile,
                // force the crater-floor shading, final shove, → 73.
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        let t = tile(x, y);
                        let tt = self.t.tile_type[t];
                        if auto_flat(tt) || tt == 8 {
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                }
                self.mc2_add_building_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
                for dy in 0..2u8 {
                    let y = cy.wrapping_sub(1).wrapping_add(dy);
                    for dx in 0..2u8 {
                        let x = cx.wrapping_sub(1).wrapping_add(dx);
                        self.t.shading[tile(x, y)] = if self.mc2_night_shade.0 { 1 } else { 63 };
                    }
                }
                self.flood_shove(i, ctx);
                self.ent[i].tick70 = 73;
                true
            }
            _ => false,
        }
    }

    /// `sub_396A0` (EF:28764) — action 73: hold the shove while life
    /// runs down, then hand to the restore finisher.
    pub(crate) fn mc2_flood_shove_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life > 0 {
            self.flood_shove(i, ctx);
        } else {
            self.ent[i].tick70 = 74;
            self.ent[i].f71 = 0;
        }
    }

    /// The finisher's settle body (EF:28911-28995): gated on
    /// `life & 3` (life sits at 0 on entry, so it fires every tick —
    /// the 16-step countdown is 16 ticks); eases each disc cell
    /// toward the rim-referenced raised cosine + jitter, snapping to
    /// the neighbour mean in the last 2 steps, and recomputes the
    /// shading for every cell.
    fn flood_settle(&mut self, i: usize) -> bool {
        if self.ent[i].act_life & 3 != 0 {
            return false;
        }
        self.ent[i].f26 -= 1;
        let cd = self.ent[i].f26 as i32;
        if cd <= 0 {
            self.ent[i].f71 = 2;
            return false;
        }
        let (ex, ey, ez) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z as i32)
        };
        let cx = (ex.wrapping_add(128) >> 8) as u8;
        let cy = (ey.wrapping_add(128) >> 8) as u8;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                let (wx, wy) = ((x as i32) << 8, (y as i32) << 8);
                let d = dist2d(ex, ey, wx, wy);
                if d < 3840 {
                    let yaw = Self::angle_between(ex, ey, wx as u16, wy as u16);
                    let mut rim = (ex, ey, 0i16);
                    Self::polar_step(&mut rim, yaw, 0, 3840);
                    let g = self.ground_z(rim.0, rim.1) >> 5;
                    let cos = SIN_DB750[0x200 + ((d << 10) / 3840) as usize] as i64;
                    let v13 = g - ((((0x10000 + cos) >> 1) * (g - ez) as i64) >> 16) as i32;
                    let v14 = (self.ent_rand(i) & 3) as i32 + v13 - 2;
                    let t = tile(x, y);
                    let h = self.t.height[t] as i32;
                    self.t.height[t] = (h + (v14 - h) / cd).clamp(1, 255) as u8;
                    if cd < 3 {
                        self.t.height[t] = self.flood_settle_cell(x, y);
                    }
                    // Cave ceiling ease toward floor + 64, /life
                    // (EF:28954-28961).
                    if self.is_cave() {
                        let tgt = (self.t.height[t] as i32 + 64).min(254);
                        let c = self.t.ceiling[t] as i32;
                        self.t.ceiling[t] = (c - (c - tgt) / cd) as u8;
                    }
                }
                self.flood_shade_cell(x, y);
            }
        }
        true
    }

    /// `sub_396D0` (EF:28783) — the action-74 RESTORE finisher:
    /// phase 0 = shove + damage pass + finish the lava conversion +
    /// arm the 16-step restore (falls into the settle); phase 1 =
    /// settle; phase 2 = snap the whole footprint to the neighbour
    /// mean, release the grabbed castles, despawn. Returns
    /// terrain-dirty.
    pub(crate) fn mc2_flood_finisher_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        match self.ent[i].f71 {
            0 => {
                self.flood_shove(i, ctx);
                self.flood_damage_pass(i);
                self.ent[i].f71 = 1;
                self.ent[i].f26 = 16;
                self.ent[i].z = self.ent[i].z.wrapping_add(64);
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        let t = tile(x, y);
                        if auto_flat(self.t.tile_type[t]) {
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                }
                self.mc2_retile_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
                self.snd(64, i);
                self.flood_settle(i); // retail falls through
                true
            }
            1 => self.flood_settle(i),
            2 => {
                // Settle every cell to the neighbour mean, IN PLACE
                // in scan order (later cells see earlier writes —
                // EF:28884-28895).
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        self.t.height[tile(x, y)] = self.flood_settle_cell(x, y);
                    }
                }
                // Release every castle this flood grabbed
                // (EF:28898-28908): owner + grab bit only (the shake
                // timer expires on its own).
                for j in 1..self.ent.len() {
                    let c = &self.ent[j];
                    if c.class64 == 3
                        && c.model65 == 2
                        && c.flags & F_QUAKE_GRAB != 0
                        && c.f40 == i as u16
                    {
                        self.ent[j].f40 = 0;
                        self.ent[j].flags &= !F_QUAKE_GRAB;
                    }
                }
                self.ent[i].flags |= 0x400; // despawn
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burn_flags_matches_0x7f0000_table() {
        // Terrain:2067 — exactly the types whose flag word lands in
        // bits 16..22: {10,11,12} + {21..27}.
        for t in 0..=255u16 {
            let t = t as u8;
            let flags: u32 = match t {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                4 => 0x10,
                5 => 0x20,
                8 => 0x100,
                9 => 0x200,
                10 => 0x10_0000,
                11 => 0x20_0000,
                12 => 0x40_0000,
                13 | 14 => 0,
                15..=20 | 28..=34 => 0x400,
                21 | 22 | 24 => 0x2_0000,
                23 => 0x4_0000,
                25 | 27 => 0x8_0000,
                26 => 0x1_0000,
                _ => 0x80_0000,
            };
            assert_eq!(burn_flags(t), flags & 0x7F_0000 != 0, "type {t}");
        }
    }
}
