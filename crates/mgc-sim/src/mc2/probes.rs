//! CONFORMANCE IMPORT ONLY — the MC2 **static ground-probe** terrain
//! reconstruct.
//!
//! `.mgcr` carries no terrain channel (docs/RECORDING.md), so a
//! mid-take import lands the pool on the level's PRISTINE heightfield
//! while retail's map still carries every terraform the run performed.
//! [`crate::mc2::riser`] and [`crate::mc2::pads`] recover the two
//! families whose stamper survives in the pool and whose write is a
//! pure function of its own state. This module recovers ground the
//! only other way the format allows: by reading the terrain samples the
//! pool already stores.
//!
//! THE HIDDEN TERRAIN CHANNEL. Three MC2 class-2 handlers end their
//! tick with a bare ground read on a STATIONARY entity:
//!
//! - `AddStatue02_01_65040` (EF:62519) — the (2,1) statue,
//! - `AddDolmen02_02_65080` (EF:62534) — the (2,2) dolmen, and
//! - `sub_65110` (EF:62545) — the (2,3) static prop
//!
//! all finish `position.z = getTerrainAlt_10C40(&position)`
//! ([`Gen::ground_z`] ≡ `sub_724C0` :81516) and never move. So each
//! prop's recorded `z` IS retail's interpolated ground height at a
//! known position, sampled by retail's own sampler on the tick before
//! the import — a one-sample terrain channel per prop, which the
//! recorder captured without knowing it.
//!
//! [`Gen::mc2_static_ground_reconstruct`] inverts that sampler: it
//! moves the (at most four) height cells the sample reads until
//! [`Gen::ground_z`] reproduces the recorded `z` EXACTLY, and picks
//! among the many assignments that do so by MINIMAL BLAST RADIUS —
//! the point-sample descendant of the pad replay's off-footprint
//! fence.
//!
//! WHAT THIS IS NOT. It is not a replay of the edits that dug the
//! ground. On mc2l24 those were `sub_30D50`'s one-shot fire scorch
//! (EF:22741 — `sub_572C0(fire, 0, 0, -(rand % 7), 1)`, fired on the
//! (10,0) fire's first acting tick) under the player's meteor barrage
//! at t≈23400-23440, and every one of those fires despawned thousands
//! of ticks before the pairs that carry the residue; the pool holds no
//! record of them. That is the LOST-BY-CONSTRUCTION class already
//! documented in [`crate::mc2::pads`] — "any crater whose caster
//! despawned". This arm recovers the RESULT at the sample points only,
//! and it recovers it from the very field the
//! `mc2l24-static-terrain-z` rule grades, so that rule's row count is
//! NOT independent evidence for the arm and stops being a sensor for
//! anything but the arm itself.
//!
//! What the reconstruct buys is real all the same: without it every
//! pair of the take compares a STALE baseline — the port's prop stands
//! on the level's authored ground for the whole run, wrong by the same
//! constant from the tick the dig landed to the end of the take, and
//! the pair tests nothing. With it the pair tests what a pair is for:
//! whether the imported TICK moves the ground under the prop the way
//! retail's tick did.

use crate::engine::features::{Gen, tile};
use std::collections::{BTreeMap, BTreeSet};

/// Retail's height clamp (`sub_56F10` EF:39519-39527 — every dig
/// saturates 0..=200), so a solved cell may never leave that band.
const H_MAX: i32 = 200;

/// What one WITNESS costs in the blast-radius score. An entity whose
/// `z` already equals the interpolated ground of its own position is
/// positive evidence that the cells its sample reads are already the
/// map retail had, so moving one is near-prohibitive; any other live
/// reader of a cell counts 1, because moving ground under it will
/// perturb whatever it does next.
const WITNESS: i64 = 1_000;

/// L1 budget for the tilt search's enumerated cells. A dig that tilts
/// a prop's tile moves its corners a few height bytes at a time (the
/// fire scorch is `-(rand % 7)` per hit); 12 covers several stacked
/// hits and still bounds the solve at a few hundred samples.
const TILT_BUDGET: i32 = 12;

impl Gen {
    /// True for the three class-2 laws whose tick is a bare ground
    /// sample on a stationary entity — see the module docs. Models
    /// 4/5 are the authentic no-op ticks (their `z` is frozen at the
    /// spawn value and says nothing about today's map), 0 is the tree
    /// (burn ladder + stump states), 6 the cave bee and 7/8 the
    /// falling props — all of those move or hold, none is a probe.
    pub(crate) fn mc2_is_ground_probe(&self, i: usize) -> bool {
        let e = &self.ent[i];
        e.class64 == 2 && matches!(e.model65, 1 | 2 | 3) && e.flags & 0x400 == 0
    }

    /// THE BLAST-RADIUS MAP — per height cell, what moving it would
    /// cost in observable damage. Built once per import, before any
    /// probe solves, and it is the whole difference between this arm
    /// helping and hurting.
    ///
    /// A probe's sample UNDER-DETERMINES its tile: two corners (even
    /// parity) or three (odd) share one scalar, so many assignments
    /// reproduce it and most are wrong. What decides between them is
    /// who else is standing on those cells. Every live entity's ground
    /// sample reads up to four of them; an entity whose `z` is already
    /// EXACTLY that sample is a WITNESS that the cells are right
    /// (they cost [`WITNESS`]), everything else that reads a cell
    /// costs 1 — a fabricated correction under an entity moves it,
    /// and a moved entity is a diff.
    ///
    /// Measured on mc2l24's (2,2) dolmen, whose −96 deficit can go
    /// −3/−3 across its two corners or −6 on the far one alone:
    /// - t≈23300, the (11,2) switch at (127,111) is a WITNESS on the
    ///   near corner (it reads pristine and conforms). Splitting −3/−3
    ///   drives it 1024 → 976 for **38 new unexplained rows** in
    ///   t=23300+300.
    /// - t≈3716 the switch is not in the pool, but an (5,3) body chain
    ///   runs over the near corner. Splitting −3/−3 there moved its
    ///   head, and the chain amplified it into **+981 unexplained
    ///   rows** over t=3560+440 (x/y/pitch/heading on 12 followers).
    /// Both windows want the same answer — put the whole deficit on
    /// the far corner — and the reader count is what says so with only
    /// the pool in hand. The switch also PROVES that answer
    /// independently: retail's digs only lower ground, so a witness
    /// reading the pristine sum over (127,111)+(128,112) means neither
    /// moved, which forces the dolmen's whole −6 onto (129,113).
    pub(crate) fn mc2_ground_reader_cost(&mut self) -> BTreeMap<usize, i64> {
        let mut cost: BTreeMap<usize, i64> = BTreeMap::new();
        for i in 1..self.ent.len() {
            let (class, flags) = (self.ent[i].class64, self.ent[i].flags);
            if class == 0 || flags & 0x400 != 0 {
                continue;
            }
            let (x, y, z) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            let w = if self.ground_z(x, y) == z as i32 {
                WITNESS
            } else {
                1
            };
            let (cx, cy) = ((x >> 8) as u8, (y >> 8) as u8);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let t = tile(cx.wrapping_add(dx), cy.wrapping_add(dy));
                if self.cell_span(t, x, y) > 0 {
                    *cost.entry(t).or_insert(0) += w;
                }
            }
        }
        cost
    }

    /// CONFORMANCE IMPORT ONLY — restore the height cells under one
    /// ground probe so [`Gen::ground_z`] reproduces its imported `z`.
    ///
    /// `claimed` carries the cells an earlier probe already solved:
    /// two probes whose tiles touch would otherwise fight over a shared
    /// corner, so the earlier claim wins and the later probe solves
    /// around it (or fails and leaves the plane alone). `cost` is
    /// [`Self::mc2_ground_reader_cost`].
    ///
    /// THE SOLVE. Every candidate is EXACT — a near miss is worth
    /// exactly as much as no correction under a byte-exact comparison
    /// — and the winner is the one with the smallest
    /// `(blast radius, edit size)`:
    /// - ONE-CELL solves, one per movable corner. The sample is
    ///   monotone in each corner within a sampler branch, so the exact
    ///   set is an interval, found by two binary searches; the value
    ///   nearest the cell's own height is taken.
    /// - the UNIFORM shift by `k`. The sampler is `(comp >> 3) + 32 *
    ///   p1` with `comp` a form in height DIFFERENCES (features.rs
    ///   `interp_plane`), so shifting every influential cell moves the
    ///   sample by exactly `32k` — the natural answer for a
    ///   flat-bottomed scorch that dropped the whole tile.
    /// - failing both, a TILT search: the other cells are enumerated
    ///   by rising L1 cost (budget [`TILT_BUDGET`]) with the one-cell
    ///   solve run at each leaf, so a dig that tipped the tile is
    ///   reproduced by the smallest corner triple that fits.
    ///
    /// RNG-free and entity-free: it writes `t.height` and the render
    /// dirty flag, nothing else. In particular it does NOT run
    /// retail's post-dig neighborhood recompute (`sub_56F10`'s slope/
    /// water/shading tail) — that pass belongs to the edits this arm
    /// cannot replay, and re-deriving it from a fabricated preimage
    /// would push guesswork into the angle plane, which far more
    /// families read than the height plane.
    pub(crate) fn mc2_static_ground_reconstruct(
        &mut self,
        i: usize,
        claimed: &mut BTreeSet<usize>,
        cost: &BTreeMap<usize, i64>,
    ) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let target = self.ent[i].z as i32;
        if self.ground_z(x, y) == target {
            return;
        }
        // THE FENCE: the only cells this arm may touch are the four
        // the sampler reads, minus any an earlier probe claimed, minus
        // any this branch of the sampler ignores outright. Everything
        // outside stays byte-identical to the baseline plane.
        let (cx, cy) = ((x >> 8) as u8, (y >> 8) as u8);
        let corners = [
            tile(cx, cy),
            tile(cx.wrapping_add(1), cy),
            tile(cx, cy.wrapping_add(1)),
            tile(cx.wrapping_add(1), cy.wrapping_add(1)),
        ];
        let mut cells: Vec<usize> = Vec::with_capacity(4);
        for &t in &corners {
            if !cells.contains(&t) && !claimed.contains(&t) && self.cell_span(t, x, y) > 0 {
                cells.push(t);
            }
        }
        if cells.is_empty() {
            return;
        }
        let saved: Vec<u8> = cells.iter().map(|&t| self.t.height[t]).collect();
        let score = |g: &Gen, cells: &[usize], saved: &[u8]| {
            let (mut blast, mut edit) = (0i64, 0i32);
            for (n, &t) in cells.iter().enumerate() {
                let d = g.t.height[t] as i32 - saved[n] as i32;
                if d != 0 {
                    blast += cost.get(&t).copied().unwrap_or(0);
                    edit += d.abs();
                }
            }
            (blast, edit)
        };
        let mut best: Option<((i64, i32), Vec<u8>)> = None;
        let offer = |g: &mut Self, best: &mut Option<((i64, i32), Vec<u8>)>| {
            let s = score(g, &cells, &saved);
            if best.as_ref().is_none_or(|(bs, _)| s < *bs) {
                *best = Some((s, cells.iter().map(|&t| g.t.height[t]).collect()));
            }
        };
        // ONE-CELL solves.
        for j in 0..cells.len() {
            if self.solve_one(cells[j], saved[j] as i32, x, y, target) {
                offer(self, &mut best);
            }
            self.t.height[cells[j]] = saved[j];
        }
        // The UNIFORM shift.
        let k = (target - self.ground_z(x, y) + 16).div_euclid(32);
        if k != 0 {
            for (n, &t) in cells.iter().enumerate() {
                self.t.height[t] = (saved[n] as i32 + k).clamp(0, H_MAX) as u8;
            }
            if self.ground_z(x, y) == target {
                offer(self, &mut best);
            }
            for (n, &t) in cells.iter().enumerate() {
                self.t.height[t] = saved[n];
            }
        }
        if let Some((_, heights)) = best {
            for (n, &t) in cells.iter().enumerate() {
                self.t.height[t] = heights[n];
            }
            claimed.extend(cells.iter().copied());
            self.terrain_dirty = true;
            return;
        }
        // THE TILT SEARCH — only when neither shape lands the sample.
        let free = (0..cells.len())
            .min_by_key(|&j| (self.cell_span(cells[j], x, y), cells[j]))
            .expect("non-empty");
        let rest: Vec<usize> = (0..cells.len()).filter(|&j| j != free).collect();
        for c in 1..=TILT_BUDGET {
            let mut deltas = vec![0i32; rest.len()];
            if self.tilt_walk(&cells, &saved, &rest, free, 0, c, &mut deltas, x, y, target) {
                claimed.extend(cells.iter().copied());
                self.terrain_dirty = true;
                return;
            }
        }
        for (n, &t) in cells.iter().enumerate() {
            self.t.height[t] = saved[n];
        }
    }

    /// Enumerate every delta vector over `rest` whose L1 norm is
    /// exactly `left`, running [`Self::solve_one`] on the `free` cell
    /// at each leaf. True — with the plane left solved — on the first
    /// exact hit.
    #[allow(clippy::too_many_arguments)]
    fn tilt_walk(
        &mut self,
        cells: &[usize],
        saved: &[u8],
        rest: &[usize],
        free: usize,
        at: usize,
        left: i32,
        deltas: &mut [i32],
        x: u16,
        y: u16,
        target: i32,
    ) -> bool {
        if at == rest.len() {
            if left != 0 {
                return false;
            }
            for (n, &j) in rest.iter().enumerate() {
                let v = saved[j] as i32 + deltas[n];
                if !(0..=H_MAX).contains(&v) {
                    return false;
                }
                self.t.height[cells[j]] = v as u8;
            }
            return self.solve_one(cells[free], saved[free] as i32, x, y, target);
        }
        for mag in 0..=left {
            for d in if mag == 0 { &[0][..] } else { &[-1, 1][..] } {
                deltas[at] = d * mag;
                if self.tilt_walk(
                    cells,
                    saved,
                    rest,
                    free,
                    at + 1,
                    left - mag,
                    deltas,
                    x,
                    y,
                    target,
                ) {
                    return true;
                }
            }
        }
        deltas[at] = 0;
        false
    }

    /// Solve ONE cell exactly: the sample is monotone in a corner's
    /// height within a sampler branch, so the exact-hit set is an
    /// interval; take the value nearest `base`. Leaves the cell on the
    /// solution when true, at `base` when false.
    fn solve_one(&mut self, t: usize, base: i32, x: u16, y: u16, target: i32) -> bool {
        let probe = |g: &mut Self, v: i32| {
            g.t.height[t] = v as u8;
            g.ground_z(x, y)
        };
        let up = probe(self, H_MAX) >= probe(self, 0);
        let key = |g: &mut Self, v: i32| if up { probe(g, v) } else { -probe(g, v) };
        let want = if up { target } else { -target };
        if key(self, 0) > want || key(self, H_MAX) < want {
            self.t.height[t] = base as u8;
            return false;
        }
        let (mut a, mut b) = (0, H_MAX);
        while a < b {
            let m = (a + b) / 2;
            if key(self, m) >= want {
                b = m
            } else {
                a = m + 1
            }
        }
        let first = a;
        let (mut a, mut b) = (0, H_MAX);
        while a < b {
            let m = (a + b + 1) / 2;
            if key(self, m) <= want {
                a = m
            } else {
                b = m - 1
            }
        }
        let last = a;
        if first > last || key(self, first) != want {
            self.t.height[t] = base as u8;
            return false;
        }
        self.t.height[t] = base.clamp(first, last) as u8;
        true
    }

    /// How far a cell's FULL legal band can move the sample — 0 for a
    /// corner this sampler branch never reads, the knob's coarseness
    /// otherwise.
    fn cell_span(&mut self, t: usize, x: u16, y: u16) -> i32 {
        let saved = self.t.height[t];
        self.t.height[t] = 0;
        let lo = self.ground_z(x, y);
        self.t.height[t] = H_MAX as u8;
        let hi = self.ground_z(x, y);
        self.t.height[t] = saved;
        (hi - lo).abs()
    }
}

#[cfg(test)]
mod tests {
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes, tile};
    use crate::verbs::VerbSet;
    use std::collections::BTreeSet;

    /// Flat 10-height OPEN (non-cave) world; no BUILD00 data — this
    /// arm reads only the height plane.
    fn flat_gen() -> Gen {
        let planes = Planes {
            height: vec![10; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
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

    /// An entity parked at a tile CENTRE, pinned to the ground the way
    /// every class-2 snap law leaves it.
    fn place(g: &mut Gen, class: u8, model: u8, tx: u16, ty: u16) -> usize {
        let i = g.new_event().expect("slot");
        {
            let e = &mut g.ent[i];
            e.class64 = class;
            e.model65 = model;
            e.tick70 = 9;
        }
        let (x, y) = (tx * 256 + 128, ty * 256 + 128);
        g.link(i, x, y, 0);
        g.ent[i].z = g.ground_z(x, y) as i16;
        i
    }

    /// The import hook (world/conformance.rs) in miniature.
    fn reconstruct(g: &mut Gen) {
        let cost = g.mc2_ground_reader_cost();
        let mut claimed = BTreeSet::new();
        for i in 0..g.ent.len() {
            if g.mc2_is_ground_probe(i) {
                g.mc2_static_ground_reconstruct(i, &mut claimed, &cost);
            }
        }
    }

    /// THE GROUND-PROBE LAW + its replay. A (2,3) prop's tick is a bare
    /// `z = ground_z(x, y)` (`sub_65110` EF:62545), so its recorded `z`
    /// is retail's own terrain sample. A `.mgcr` import lands on the
    /// PRISTINE plane, and the prop then reads authored ground for the
    /// rest of the take — a constant, permanent diff. The reconstruct
    /// must put the sample back EXACTLY, and must not touch a single
    /// cell outside the four the sampler reads.
    #[test]
    fn static_ground_reconstruct_restores_the_sample_under_a_dug_prop() {
        // (a) the LIVE history: a 5x5 scorch bowl 4 bytes deep, then
        // the prop's own snap.
        let mut live = flat_gen();
        let p = place(&mut live, 2, 3, 60, 60);
        for ty in 58u8..=62 {
            for tx in 58u8..=62 {
                live.t.height[tile(tx, ty)] = 6;
            }
        }
        let (px, py) = (live.ent[p].x, live.ent[p].y);
        live.ent[p].z = live.ground_z(px, py) as i16;
        assert_eq!(live.ent[p].z, 192, "the bowl drops the sample 4 bytes");

        // (b) the IMPORT: pristine plane + the prop's recorded z.
        let mut imported = flat_gen();
        let j = place(&mut imported, 2, 3, 60, 60);
        imported.ent[j].z = live.ent[p].z;
        let pristine = imported.t.height.clone();
        reconstruct(&mut imported);

        // The SAMPLE is the law — not the bowl, which is unrecoverable
        // (the fire that dug it despawned long ago).
        let (x, y) = (imported.ent[j].x, imported.ent[j].y);
        assert_eq!(
            imported.ground_z(x, y),
            live.ent[p].z as i32,
            "the replayed plane must reproduce retail's own sample"
        );
        assert_eq!(imported.ent[j].z, live.ent[p].z, "prop state untouched");

        // (c) NON-VACUITY: the plane is NOT pristine.
        assert_ne!(
            imported.t.height, pristine,
            "a no-op replay would leave the plane pristine"
        );

        // (d) THE FENCE: only the four corners the sampler reads may
        // move; the live bowl touched far more, and every one of those
        // cells must come back untouched.
        let corners = [tile(60, 60), tile(61, 60), tile(60, 61), tile(61, 61)];
        let mut witnesses = 0;
        for ty in 50u8..70 {
            for tx in 50u8..70 {
                let t = tile(tx, ty);
                if corners.contains(&t) {
                    continue;
                }
                assert_eq!(
                    imported.t.height[t], pristine[t],
                    "off-sample cell ({tx},{ty}) must be fenced"
                );
                witnesses += usize::from(live.t.height[t] != pristine[t]);
            }
        }
        assert!(
            witnesses > 0,
            "the live bowl must have moved ground outside the sampled \
             corners for the fence assertion above to mean anything"
        );

        // (e) IDEMPOTENCE: the sample now agrees, so a second pass is a
        // no-op — which is what lets the arm run over every import.
        let once = imported.t.height.clone();
        reconstruct(&mut imported);
        assert_eq!(imported.t.height, once, "second replay must not move");
    }

    /// THE BLAST-RADIUS RULE. A prop's sample under-determines its
    /// tile: on even parity `z = 16 * (h(cx,cy) + h(cx+1,cy+1))`, so
    /// the same deficit can land on either corner. A live WITNESS —
    /// an entity already standing at exactly the interpolated ground
    /// of its own position — says which corner retail did NOT dig, and
    /// the solve must respect it. (mc2l24: without this the dolmen's
    /// −96 is split −3/−3 and the (11,2) switch next door, byte-exact
    /// before, breaks; the same split moved a (5,3) body chain's head
    /// for +981 unexplained rows over t=3560+440.)
    #[test]
    fn ground_probe_blast_radius_spares_a_witness_cell() {
        // The shared corner is (60,60): the prop at tile (60,60) reads
        // it and so does a witness parked at tile (59,59).
        let deficit = 8; // bytes off the corner pair
        let solve = |with_witness: bool| {
            let mut g = flat_gen();
            let p = place(&mut g, 2, 3, 60, 60);
            if with_witness {
                place(&mut g, 5, 0, 59, 59);
            }
            g.ent[p].z -= 16 * deficit;
            reconstruct(&mut g);
            let (x, y) = (g.ent[p].x, g.ent[p].y);
            assert_eq!(
                g.ground_z(x, y),
                g.ent[p].z as i32,
                "the sample must be reproduced either way"
            );
            (
                g.t.height[tile(60, 60)],
                g.t.height[tile(61, 61)],
                g.t.height[tile(59, 59)],
            )
        };
        // With the witness the whole deficit goes to the FAR corner.
        assert_eq!(
            solve(true),
            (10, 10 - deficit as u8, 10),
            "the witness's corner must stand"
        );
        // NON-VACUITY: without it the solve takes the near corner, so
        // the assertion above is testing the witness and nothing else.
        assert_eq!(
            solve(false),
            (10 - deficit as u8, 10, 10),
            "no witness, no reason to spare the near corner"
        );
    }
}
