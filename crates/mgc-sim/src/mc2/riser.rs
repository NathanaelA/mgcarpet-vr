//! MC2 class-14 model-1 terrain riser (`sub_59F60`) + its class-10
//! (10,63)/(10,64) lower/raise triggers — Phase 4.3. Trace bank:
//! docs/traces/mc2-class14-m1-riser.md (`EF:` = remc2
//! EventsFunctions.cpp, `EV:` = Events.cpp; section cites below).
//!
//! The riser is a three-phase terrain machine on `life_0x8` (our
//! `act_life`): 0 = INSTANT build (+48 in one tick), 1 = ANIMATED
//! raise (+1/tick x 48, loop sound 47), 2 = ANIMATED lower (-1/tick to
//! flank level, then terrain-type restore), 3/4 = idle built/removed.
//! It rewrites height, tile type (8 = ridge/wall), angle (class
//! nibble 1 — clears the deep-water bit), shading and the renderer
//! dirty bit itself — no retile. On caves every bit-3 clear becomes
//! the full floor↔ceiling invariant ([`Gen::cave_seal_fixup`],
//! Phase 4.5) — this is what makes a riser a solid pillar in a cave
//! (ceiling-sim trace §5a).
//!
//! Field map (remc2 -> ours): `life_0x8` -> `act_life`,
//! `dword_0x10_16` (length, THING par2) -> `f26`,
//! `subSpellIndex_0x2A_42` (0..48 progress) -> `f44`,
//! `byte_0x46_70` (orientation, THING par1: 0 = +X strip, 1 = +Y
//! strip, >=2 counters-only) -> `f71` (the (11,32) precedent for
//! byte-70 data; our `tick70` holds the dispatch index).

use crate::mc1::features::{Gen, tile};

/// 16-bit cell-index step — the original's `uaxis_2d.word`
/// arithmetic: x carries/borrows into y at the byte boundary;
/// +-256/512/768 = y +- 1/2/3 (trace §8).
#[inline]
fn w(t: usize, d: i32) -> usize {
    (t as u16).wrapping_add(d as u16) as usize
}

/// Byte-axis step (`._axis_2d.x/y +-`): each axis wraps independently.
#[inline]
fn b(t: usize, dx: i32, dy: i32) -> usize {
    tile(
        (t as u8).wrapping_add(dx as u8),
        ((t >> 8) as u8).wrapping_add(dy as u8),
    )
}

/// The 28..47 shade fold (EF:41604-41612 et al.): `>= 28`:
/// `> 40 -> (s&7)+40`; else `(s&3)+28`.
#[inline]
fn fold_shade(s: i32) -> u8 {
    (if s >= 28 {
        if s > 40 { (s & 7) + 40 } else { s }
    } else {
        (s & 3) + 28
    }) as u8
}

impl Gen {
    /// The riser's round-to-nearest cell: `((pos + 128) >> 8)` per
    /// axis (EF:41479, 41803, 42146) — one MORE than the authored
    /// tile for center-spawned THINGs (authentic off-by-one; the base
    /// cell derivations below step back from it).
    fn riser_cell(&self, i: usize) -> usize {
        tile(
            (self.ent[i].x.wrapping_add(128) >> 8) as u8,
            (self.ent[i].y.wrapping_add(128) >> 8) as u8,
        )
    }

    /// Day law (EF:41613-41616 et al.): non-Day maps invert the shade
    /// (`32 - s + 32`) — the same [`Gen::mc2_night_shade`] switch the
    /// retile pass keys.
    #[inline]
    fn day_shade(&self, s: u8) -> u8 {
        if self.mc2_night_shade.0 {
            64u8.wrapping_sub(s)
        } else {
            s
        }
    }

    /// `sub_59F60` (EF:41255-42492), class-14 action 6 — one tick.
    /// Returns true when terrain changed.
    pub(crate) fn mc2_riser_tick(&mut self, i: usize) -> bool {
        match self.ent[i].act_life {
            0 => self.riser_instant(i),
            1 => self.riser_raise(i),
            2 => self.riser_lower(i),
            // < 0 dead; 3/4 idle built/removed (EF:42138).
            _ => false,
        }
    }

    /// LIFE 0 — instant build (EF:41470-41796): `++length` (so an
    /// instant-built riser is par2+1 long forever — §2 lifecycle
    /// nuance), one-shot +48 build, then `sub = 48; life = 3` for ANY
    /// orientation (junk orientations tick the counters, write
    /// nothing).
    fn riser_instant(&mut self, i: usize) -> bool {
        self.ent[i].f26 += 1; // EF:41481
        let l = self.ent[i].f26 as i32;
        let orient = self.ent[i].f71;
        let c = self.riser_cell(i);
        match orient {
            // Base cell: strip-axis byte-dec off the derived cell
            // (EF:41483-41486).
            1 => self.riser_build_y(b(c, 0, -1), l),
            0 => self.riser_build_x(b(c, -1, 0), l),
            _ => {}
        }
        self.ent[i].f44 = 48; // EF:41641/41792
        self.ent[i].act_life = 3;
        orient <= 1
    }

    /// Orientation 1 (strip along +Y), life 0 (EF:41488-41643).
    fn riser_build_y(&mut self, bb: usize, l: i32) {
        // (a) RAISE +48 — 2 columns (bx, bx-1; word--) x L rows
        // ascending (EF:41492-41513). Skip rule: an already-type-8
        // cell with a smooth (<=30) step to its +Y neighbor is not
        // re-raised.
        let mut col = bb;
        for _ in 0..2 {
            let mut cell = col;
            for _ in 0..l {
                let nb = b(cell, 0, 1);
                if self.t.tile_type[cell] != 8
                    || (self.t.height[cell] as i32 - self.t.height[nb] as i32).abs() > 30
                {
                    self.t.height[cell] = self.t.height[cell].wrapping_add(48);
                }
                cell = b(cell, 0, 1);
            }
            col = w(col, -1);
        }
        // (b) TYPE/ANGLE STAMP — 3 cols (word--) x (L+1) rows
        // by-1..by+L-1 (EF:41514-41531): ridge type 8, angle 1
        // (class nibble 1, ALL other bits cleared).
        let mut col = bb;
        for _ in 0..3 {
            let mut cell = w(col, -256);
            for _ in 0..(l + 1) {
                self.t.tile_type[cell] = 8;
                self.t.angle[cell] = 1;
                cell = b(cell, 0, 1);
            }
            col = w(col, -1);
        }
        // (c)/(d) bit-3 sync — 4 cols bx+1..bx-2 (init word+1, step
        // x--) x (L+2) rows by-1..by+L: on caves the full invariant
        // (EF:41535-41563), else the plain deep-water/solid clear
        // (EF:41564-41583).
        let mut col = w(bb, 1);
        for _ in 0..4 {
            let mut cell = w(col, -256);
            for _ in 0..(l + 2) {
                if self.is_cave() {
                    self.cave_seal_fixup(cell);
                } else {
                    self.t.angle[cell] &= 0xF7;
                }
                cell = b(cell, 0, 1);
            }
            // G9i: the CAVE loop steps the column byte-wise
            // (EF:41560); the NON-CAVE loop decrements the packed
            // WORD (EF:41579) — at x==0 the borrow crosses into
            // y−1. (The build-X mirror is word-stepped in BOTH
            // branches, EF:41711/41731 — no split there.)
            col = if self.is_cave() {
                b(col, -1, 0)
            } else {
                w(col, -1)
            };
        }
        // (e) SHADING — L+1 cells (bx, by-1+k) off the NW-SE diagonal
        // h[(bx-1, by+k-2)] - h[(bx+1, by+k)] + 32 (EF:41584-41622).
        for k in 0..=l {
            let hi = self.t.height[b(bb, -1, k - 2)] as i32;
            let lo = self.t.height[b(bb, 1, k)] as i32;
            let s = self.day_shade(fold_shade(hi - lo + 32));
            self.t.shading[b(bb, 0, k - 1)] = s;
        }
        // (f) DIRTY (EF:41623-41639).
        self.riser_dirty_y(bb, l);
    }

    /// Orientation 0 (strip along +X), life 0 — the exact mirror
    /// (EF:41644-41794).
    fn riser_build_x(&mut self, bb: usize, l: i32) {
        // (a') RAISE — L cols ascending x rows {by, by-1} (y--);
        // neighbor test +X along the strip (word+1) (EF:41646-41666).
        let mut col = bb;
        for _ in 0..l {
            let mut cell = col;
            for _ in 0..2 {
                let nb = w(cell, 1);
                if self.t.tile_type[cell] != 8
                    || (self.t.height[cell] as i32 - self.t.height[nb] as i32).abs() > 30
                {
                    self.t.height[cell] = self.t.height[cell].wrapping_add(48);
                }
                cell = b(cell, 0, -1);
            }
            col = w(col, 1);
        }
        // (b') STAMP — cols bx-1..bx+L-1 (L+1) x 3 rows by..by-2
        // (EF:41667-41685).
        let mut col = w(bb, -1);
        for _ in 0..(l + 1) {
            let mut cell = col;
            for _ in 0..3 {
                self.t.tile_type[cell] = 8;
                self.t.angle[cell] = 1;
                cell = b(cell, 0, -1);
            }
            col = w(col, 1);
        }
        // (c')/(d') bit-3 sync — cols bx-1..bx+L (L+2) x 4 rows
        // by+1..by-2 (init +256, y--): cave invariant (EF:41686-
        // 41713) else the plain clear (EF:41714-41733).
        let mut col = w(bb, -1);
        for _ in 0..(l + 2) {
            let mut cell = w(col, 256);
            for _ in 0..4 {
                if self.is_cave() {
                    self.cave_seal_fixup(cell);
                } else {
                    self.t.angle[cell] &= 0xF7;
                }
                cell = b(cell, 0, -1);
            }
            col = w(col, 1);
        }
        // (e') SHADING — L+1 cells (bx-1+k, by) (EF:41734-41774).
        for k in 0..=l {
            let hi = self.t.height[b(bb, k - 2, -1)] as i32;
            let lo = self.t.height[b(bb, k, 1)] as i32;
            let s = self.day_shade(fold_shade(hi - lo + 32));
            self.t.shading[b(bb, k - 1, 0)] = s;
        }
        // (f') DIRTY (EF:41775-41791).
        self.riser_dirty_x(bb, l);
    }

    /// The orientation-1 dirty block — 9 cols bx-3..bx+5 (word++) x
    /// (L+6) rows by-3..by+L+2 ascending (EF:41623-41639, 41858-41874;
    /// the life-2 restore uses this SYMMETRIC form too — the trace's
    /// OPEN-1 remc2-transcription fix, EF:42326-42344).
    fn riser_dirty_y(&mut self, bb: usize, l: i32) {
        let mut col = w(bb, -3);
        for _ in 0..9 {
            let mut cell = w(col, -768);
            for _ in 0..(l + 6) {
                self.t.angle[cell] |= 0x80;
                cell = b(cell, 0, 1);
            }
            col = w(col, 1);
        }
    }

    /// The orientation-0 dirty block — cols bx-3..bx+L+2 (word++) x 9
    /// rows by+3 DOWN to by-5 (init +768, y--) (EF:41917-41931,
    /// 42473-42490).
    fn riser_dirty_x(&mut self, bb: usize, l: i32) {
        let mut col = w(bb, -3);
        for _ in 0..(l + 6) {
            let mut cell = w(col, 768);
            for _ in 0..9 {
                self.t.angle[cell] |= 0x80;
                cell = b(cell, 0, -1);
            }
            col = w(col, 1);
        }
    }

    /// LIFE 1 — animated raise (EF:41798-42136): loop sound 47 every
    /// tick; first tick stamps type/angle over the strip INTERIOR;
    /// every tick +1 height on rows/cols +-3 in from the ends (the
    /// ends stay low — retail's implicit ramp); the tick `sub`
    /// reaches 48 shades; the NEXT tick takes the gate -> life 3.
    fn riser_raise(&mut self, i: usize) -> bool {
        if self.ent[i].f44 >= 0x30 {
            // life = 3 + EndLoop_6EAB0(47) — our per-tick sound
            // requests simply cease (EF:42133-42135).
            self.ent[i].act_life = 3;
            return false;
        }
        self.snd(47, i); // PrepareEventSound, every raise tick (EF:41802)
        let l = self.ent[i].f26 as i32;
        let orient = self.ent[i].f71;
        let c = self.riser_cell(i);
        // Base cell (EF:41803-41812): orient 1 byte-dec, orient 0
        // WORD-dec (x==0 borrows into y — port faithfully, §1).
        let bb = if orient == 1 { b(c, 0, -1) } else { w(c, -1) };
        let first = self.ent[i].f44 == 0;
        self.ent[i].f44 += 1; // EF:41934
        let last = self.ent[i].f44 >= 0x30;
        match orient {
            1 => {
                if first {
                    self.riser_stamp_y(bb, l);
                }
                // +1 height, rows by+3..by+L-4, cols bx and bx-1
                // (EF:41938-41955), then the cave invariant re-walk
                // over the same cells (EF:41957-41991).
                for k in 3..(l - 3) {
                    for d in [0, -1] {
                        let cell = b(bb, d, k);
                        self.t.height[cell] = self.t.height[cell].wrapping_add(1);
                    }
                }
                if self.is_cave() {
                    for k in 3..(l - 3) {
                        for d in [0, -1] {
                            self.cave_seal_fixup(b(bb, d, k));
                        }
                    }
                }
                if last {
                    // Last-tick shading: ONLY 3 cells (bx..bx+2, by)
                    // — verbatim retail asymmetry (EF:42050-42089).
                    for k in 0..3 {
                        let hi = self.t.height[b(bb, k - 1, -1)] as i32;
                        let lo = self.t.height[b(bb, k + 1, 1)] as i32;
                        let s = self.day_shade(fold_shade(hi - lo + 32));
                        self.t.shading[b(bb, k, 0)] = s;
                    }
                }
            }
            0 => {
                if first {
                    self.riser_stamp_x(bb, l);
                }
                // +1 height, cols bx+3..bx+L-4, rows by and by-1
                // (EF:41996-42010), then the cave invariant re-walk
                // (EF:42012-42043).
                for k in 3..(l - 3) {
                    for d in [0, -1] {
                        let cell = b(bb, k, d);
                        self.t.height[cell] = self.t.height[cell].wrapping_add(1);
                    }
                }
                if self.is_cave() {
                    for k in 3..(l - 3) {
                        for d in [0, -1] {
                            self.cave_seal_fixup(b(bb, k, d));
                        }
                    }
                }
                if last {
                    // L+1 cells (bx-1+k, by) (EF:42094-42128).
                    for k in 0..=l {
                        let hi = self.t.height[b(bb, k - 2, -1)] as i32;
                        let lo = self.t.height[b(bb, k, 1)] as i32;
                        let s = self.day_shade(fold_shade(hi - lo + 32));
                        self.t.shading[b(bb, k - 1, 0)] = s;
                    }
                }
            }
            _ => {}
        }
        orient <= 1
    }

    /// The raise's first-tick stamp, orientation 1 (EF:41813-41874):
    /// interior-only (endpoints excluded).
    fn riser_stamp_y(&mut self, bb: usize, l: i32) {
        // STAMP — 3 cols (word--) x rows by+2..by+L-4 (EF:41820-41837).
        let mut col = bb;
        for _ in 0..3 {
            let mut cell = w(col, 512);
            let mut k = 2;
            while k < l - 3 {
                self.t.tile_type[cell] = 8;
                self.t.angle[cell] = 1;
                cell = b(cell, 0, 1);
                k += 1;
            }
            col = w(col, -1);
        }
        // NON-CAVE CLEAR — 4 cols bx+1..bx-2 (word--) x rows
        // by+2..by+L-3 (`if (!isCaveLevel)` EF:41838-41857; cave
        // levels run the per-raise-tick invariant instead).
        if !self.is_cave() {
            let mut col = w(bb, 1);
            for _ in 0..4 {
                let mut cell = w(col, 512);
                let mut k = 2;
                while k < l - 2 {
                    self.t.angle[cell] &= 0xF7;
                    cell = b(cell, 0, 1);
                    k += 1;
                }
                col = w(col, -1);
            }
        }
        self.riser_dirty_y(bb, l); // EF:41858-41874
    }

    /// The raise's first-tick stamp, orientation 0 (EF:41875-41931).
    fn riser_stamp_x(&mut self, bb: usize, l: i32) {
        // STAMP — cols bx+2..bx+L-4 x 3 rows by..by-2 (EF:41879-41896).
        let mut col = w(bb, 2);
        let mut k = 2;
        while k < l - 3 {
            let mut cell = col;
            for _ in 0..3 {
                self.t.tile_type[cell] = 8;
                self.t.angle[cell] = 1;
                cell = b(cell, 0, -1);
            }
            col = w(col, 1);
            k += 1;
        }
        // NON-CAVE CLEAR — cols bx+2..bx+L-3 x 4 rows by+1..by-2
        // (init +256, y--) (`if (!isCaveLevel)` EF:41897-41916).
        if !self.is_cave() {
            let mut col = w(bb, 2);
            let mut k = 2;
            while k < l - 2 {
                let mut cell = w(col, 256);
                for _ in 0..4 {
                    self.t.angle[cell] &= 0xF7;
                    cell = b(cell, 0, -1);
                }
                col = w(col, 1);
                k += 1;
            }
        }
        self.riser_dirty_x(bb, l); // EF:41917-41931
    }

    /// LIFE 2 — animated lower + restore (EF:42138-42491): sink the
    /// strip interior 1/tick toward the flank average; on the tick
    /// `sub` hits 0, re-type the strip from the flank terrain (over
    /// water flanks it becomes water again); the NEXT tick sees
    /// `sub == 0` -> life 4.
    fn riser_lower(&mut self, i: usize) -> bool {
        if self.ent[i].f44 == 0 {
            self.ent[i].act_life = 4; // + EndLoop 47 (EF:42140-42143)
            return false;
        }
        self.snd(47, i); // EF:42145
        let l = self.ent[i].f26 as i32;
        let orient = self.ent[i].f71;
        let c = self.riser_cell(i);
        let bb = if orient == 1 { b(c, 0, -1) } else { w(c, -1) };
        match orient {
            1 => {
                // Sink rows by+3..by+L-4 toward the flank-column
                // average (bx-2, bx+1), clamped — no underswing
                // (EF:42159-42181).
                for k in 3..(l - 3) {
                    let flank = (self.t.height[b(bb, -2, k)] as i32
                        + self.t.height[b(bb, 1, k)] as i32)
                        >> 1;
                    for d in [0, -1] {
                        let cell = b(bb, d, k);
                        if flank < self.t.height[cell] as i32 {
                            self.t.height[cell] = self.t.height[cell].wrapping_sub(1);
                        }
                    }
                }
            }
            0 => {
                // Cols bx+3..bx+L-4, flank rows (by-2, by+1)
                // (EF:42185-42203).
                for k in 3..(l - 3) {
                    let flank = (self.t.height[b(bb, k, -2)] as i32
                        + self.t.height[b(bb, k, 1)] as i32)
                        >> 1;
                    for d in [0, -1] {
                        let cell = b(bb, k, d);
                        if flank < self.t.height[cell] as i32 {
                            self.t.height[cell] = self.t.height[cell].wrapping_sub(1);
                        }
                    }
                }
            }
            _ => {}
        }
        self.ent[i].f44 -= 1; // EF:42205-42206
        if self.ent[i].f44 == 0 {
            // Final tick: terrain-type restore (EF:42207-42490).
            // Orientation >= 2 returns with NO cleanup (EF:42212-13).
            match orient {
                1 => self.riser_restore_y(bb, l),
                0 => self.riser_restore_x(bb, l),
                _ => {}
            }
        }
        orient <= 1
    }

    /// The lower's final-tick restore, orientation 1 (EF:42214-42344).
    fn riser_restore_y(&mut self, bb: usize, l: i32) {
        // RESTORE rows by+3..by+L-5: copy type/angle from the east
        // flank cell (bx+2, row) — jumped +L/2 ALONG the strip if the
        // flank is still ridge — onto the 4 strip cells; shading 32;
        // dirty bit (EF:42214-42252).
        for k in 3..(l - 4) {
            let mut src = b(bb, 2, k);
            if self.t.tile_type[src] == 8 {
                src = b(src, 0, l >> 1); // +halfL along +Y (byte add)
            }
            for d in [1, 0, -1, -2] {
                let cell = b(bb, d, k);
                self.t.tile_type[cell] = self.t.tile_type[src];
                self.t.angle[cell] = self.t.angle[src] | 0x80;
                self.t.shading[cell] = 32;
            }
        }
        // Bit-3 sync: on caves the invariant over the SAME restored
        // strip cells, rows by+3..by+L-5 (EF:42253-42312); else the
        // 4-cell endcap clear across row by+3 (EF:42317-42325).
        if self.is_cave() {
            for k in 3..(l - 4) {
                for d in [1, 0, -1, -2] {
                    self.cave_seal_fixup(b(bb, d, k));
                }
            }
        } else {
            for d in [1, 0, -1, -2] {
                self.t.angle[b(bb, d, 3)] &= 0xF7;
            }
        }
        // DIRTY — the SYMMETRIC word++ form (trace OPEN-1: remc2's
        // transcription steps y here, asymmetric vs every other dirty
        // block; EF:42326-42344).
        self.riser_dirty_y(bb, l);
    }

    /// The lower's final-tick restore, orientation 0 (EF:42347-42490).
    fn riser_restore_x(&mut self, bb: usize, l: i32) {
        // RESTORE cols bx+3..bx+L-5 from the south flank (col, by+2),
        // jumped +L/2 along +X (WORD add) if still ridge
        // (EF:42347-42389).
        for k in 3..(l - 4) {
            let mut src = b(bb, k, 2);
            if self.t.tile_type[src] == 8 {
                src = w(src, l >> 1);
            }
            for d in [1, 0, -1, -2] {
                let cell = b(bb, k, d);
                self.t.tile_type[cell] = self.t.tile_type[src];
                self.t.angle[cell] = self.t.angle[src] | 0x80;
                self.t.shading[cell] = 32;
            }
        }
        // Bit-3 sync: cave invariant over the restored strip, cols
        // bx+3..bx+L-5 (EF:42390-42450); else the 4-cell endcap
        // clear at col bx+3 (EF:42456-42472).
        if self.is_cave() {
            for k in 3..(l - 4) {
                for d in [1, 0, -1, -2] {
                    self.cave_seal_fixup(b(bb, k, d));
                }
            }
        } else {
            for d in [1, 0, -1, -2] {
                self.t.angle[b(bb, 3, d)] &= 0xF7;
            }
        }
        self.riser_dirty_x(bb, l); // EF:42473-42490
    }

    // ---- the (10,63)/(10,64) lower/raise triggers -------------------------

    /// `sub_4F900` / `sub_4F950` (EF:36222/:36238) — the trigger
    /// ctors: maxLife 1, action 0x44 (model 63, LOWER) / 0x45 (model
    /// 64, RAISE), untargetable, map-linked (so [`Self::
    /// mc2_find_riser`]'s cell chain sees co-located pairs), no
    /// sprite. Campaign wiring: authored in the SAME map cell as a
    /// (14,1)/(14,2) THING, dis-gated for stage-scripted open/close
    /// (724 records, levels 002/003/005/007/008...).
    pub(crate) fn mc2_spawn_riser_trigger(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = if model == 63 { 0x44 } else { 0x45 };
            e.max_life = 1;
            // byte[0] = (&0xF6)|1.
            e.flags = (e.flags & !0x8) | 1;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        Some(i)
    }

    /// `sub_5B070` (EF:42497-42526) — find the riser/pillar in MY map
    /// cell: `(pos - 128) >> 8` per axis (NOTE the -128 bias vs the
    /// riser's +128 — identical for tile-center spawns, which is how
    /// the campaign authors every pair), then the per-tile entity
    /// chain, first class-14 model 1|2 wins.
    fn mc2_find_riser(&self, x: u16, y: u16) -> Option<usize> {
        let t = tile(
            (x.wrapping_sub(128) >> 8) as u8,
            (y.wrapping_sub(128) >> 8) as u8,
        );
        let mut v = self.map_entity[t] as usize;
        while v != 0 {
            let e = &self.ent[v];
            if e.class64 == 14 && matches!(e.model65, 1 | 2) {
                return Some(v);
            }
            v = e.next20 as usize;
        }
        None
    }

    /// `sub_34390` / `sub_343C0` (EF:25003/:25015, EV:2495-2502) —
    /// the one-shot trigger actions: poke the co-located riser's
    /// phase (0x44 -> life 2 LOWER, 0x45 -> life 1 RAISE), then
    /// DisableEntityDrawing (despawn). RAISE on an idle-built riser
    /// is a no-op next tick (sub >= 48); RAISE on a lowered one
    /// re-runs the full animation including the first-tick stamp.
    pub(crate) fn mc2_riser_trigger_tick(&mut self, i: usize) {
        let raise = self.ent[i].tick70 == 0x45;
        if let Some(r) = self.mc2_find_riser(self.ent[i].x, self.ent[i].y) {
            self.ent[r].act_life = if raise { 1 } else { 2 };
        }
        self.ent[i].flags |= 0x400;
    }
}
