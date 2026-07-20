//! Native port of Magic Carpet 2's terrain generator.
//!
//! Reference: the remc2-carved `tools/mc2-genlevel/vendor/Terrain.cpp`
//! (GPL-3.0, see that dir's PROVENANCE.md), itself decompiled from
//! MC2.EXE. Entry point in the original is `GenerateLevelMap_43830`
//! (Terrain.cpp:19), which the standalone oracle drove from a level
//! buffer. This port takes the already-parsed [`Mc2Level`] instead and
//! returns the same planes the oracle emits, so it drops in for the
//! external `mc2-genlevel` tool (see bake.rs).
//!
//! Fidelity notes (shared with the MC1 port, [`crate::mc1_terrain`]):
//!
//! - The 256x256 grid is toroidal and walked with 8-bit coordinate
//!   arithmetic on a `u16` cell index (low byte = x, high byte = y,
//!   each wrapping independently). The C++ does this with the
//!   `uaxis_2d` union and `index._axis_2d.x++/.y--`; [`Cell`] below
//!   reproduces it exactly.
//! - One 16-bit LCG (`x = 9377*x + 9439`, wrapping u16) drives most
//!   passes — the same generator as MC1. The global stream is `rand2`
//!   (reset to the seed at entry, and to 0 in the shading tail); the
//!   fractal advances a private copy passed by pointer. `sub_43BB0`
//!   (cave ceiling fuzz) has its OWN 32-bit stream seeded to a constant.
//! - Integer widths are load-bearing: the height planes are `u8`, the
//!   fractal accumulator plane is `i16` (`mapEntityIndex`), and several
//!   passes rely on `u8` wrap and `int` intermediate products. Match
//!   the C++ widths exactly or the byte-for-byte oracle check fails.
//!
//! The generation chain touches only `unk_D47E0` (below); the texture/
//! rotation editor helpers (`sub_45DC0`/`45BE0`/`33F70`/`462A0`) and
//! `unk_D4A30`/`building_F2CD0x` are unreachable from the entry point
//! (verified by call-graph) and so are not ported.

use crate::level_mc2::{MapType, Mc2Level};

/// Cells in the 256x256 terrain grid.
pub const GRID: usize = 0x10000;

/// The generation-time scratch the engine carves out of its VGA screen
/// buffer (`pdwScreenBuffer_351628`); `sub_44580` uses the first
/// `25 * 2401` bytes as 2401 corner-combination buckets.
const SCRATCH: usize = 25 * 2401 + 4096;

/// The generated planes, engine layout (index = y*256 + x), matching
/// the planes the `mc2-genlevel` oracle emits and `.mgcl` stores.
///
/// `ceiling` is the second heightmap / cave-mirror plane; it is
/// all-zero off cave levels (the oracle's `sub_43D50` never writes it),
/// and the bake wiring drops an all-zero ceiling from the package.
pub struct Mc2Terrain {
    pub tile_type: Vec<u8>,
    pub height: Vec<u8>,
    pub shading: Vec<u8>,
    pub angle: Vec<u8>,
    pub ceiling: Vec<u8>,
}

/// A toroidal 256x256 cell index: low byte = x, high byte = y, each
/// wrapping independently — the C++ `uaxis_2d`. Step helpers mirror the
/// oracle's `index._axis_2d.x++/--` / `.y++/--`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Cell(pub u16);

impl Cell {
    #[inline]
    pub(crate) fn idx(self) -> usize {
        self.0 as usize
    }
    #[inline]
    pub(crate) fn x(self) -> u8 {
        self.0 as u8
    }
    #[inline]
    pub(crate) fn y(self) -> u8 {
        (self.0 >> 8) as u8
    }
    #[inline]
    pub(crate) fn xpp(&mut self) {
        self.0 = (self.0 & 0xFF00) | (self.x().wrapping_add(1) as u16);
    }
    #[inline]
    pub(crate) fn xmm(&mut self) {
        self.0 = (self.0 & 0xFF00) | (self.x().wrapping_sub(1) as u16);
    }
    #[inline]
    pub(crate) fn ypp(&mut self) {
        self.0 = (self.0 & 0x00FF) | ((self.y().wrapping_add(1) as u16) << 8);
    }
    #[inline]
    pub(crate) fn ymm(&mut self) {
        self.0 = (self.0 & 0x00FF) | ((self.y().wrapping_sub(1) as u16) << 8);
    }
    /// Add a wrapping delta to the x (low) byte only. The C++ fractal
    /// does `index._axis_2d.x += 2*a1` for step-sized moves.
    #[inline]
    pub(crate) fn xadd(&mut self, d: i32) {
        self.0 = (self.0 & 0xFF00) | (self.x().wrapping_add(d as u8) as u16);
    }
    /// Add a wrapping delta to the y (high) byte only.
    #[inline]
    pub(crate) fn yadd(&mut self, d: i32) {
        self.0 = (self.0 & 0x00FF) | ((self.y().wrapping_add(d as u8) as u16) << 8);
    }
    /// Index of the cell offset by `(dx, dy)` without mutating (each
    /// axis wraps in its byte).
    #[inline]
    pub(crate) fn at(self, dx: i32, dy: i32) -> usize {
        let x = self.x().wrapping_add(dx as u8);
        let y = self.y().wrapping_add(dy as u8);
        ((y as usize) << 8) | x as usize
    }
}

/// The 16-bit LCG shared with MC1: `x = 9377*x + 9439`, wrapping u16.
#[inline]
pub(crate) fn lcg(state: &mut u16) -> u16 {
    *state = state.wrapping_mul(9377).wrapping_add(9439);
    *state
}

/// `unk_D47E0` (Terrain.cpp lookup, 0x250 signed bytes): 148 four-byte
/// corner-class quads. Entries 0..6 are the pure classes, 7..34 the
/// 0xFF (invalid) building-slot gap, 35+ the transition tiles. Read by
/// `sub_44580` to build the corner-combination buckets.
#[rustfmt::skip]
const UNK_D47E0: [i8; 0x250] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3,
    4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 6, 0, 1, 4,
    1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1,
    6, 6, 4, 4, 6, 4, 6, 4, 6, 4, 6, 6, 4, 6, 4, 4,
    4, 4, 0, 0, 4, 0, 0, 0, 0, 4, 4, 4, 0, 4, 0, 4,
    1, 3, 3, 3, 1, 3, 1, 3, 3, 1, 1, 1, 1, 1, 3, 3,
    5, 1, 1, 1, 1, 1, 5, 5, 1, 5, 1, 5, 1, 5, 5, 5,
    2, 5, 2, 5, 5, 2, 2, 2, 2, 5, 5, 5, 5, 5, 2, 2,
    4, 4, 3, 3, 4, 3, 3, 3, 3, 4, 3, 4, 3, 4, 4, 4,
    4, 5, 5, 5, 5, 4, 4, 4, 5, 4, 5, 4, 4, 4, 5, 5,
    1, 2, 1, 2, 2, 1, 1, 1, 1, 2, 2, 2, 1, 1, 2, 2,
    4, 1, 1, 1, 1, 4, 1, 4, 1, 4, 4, 4, 1, 1, 4, 4,
    1, 6, 1, 1, 6, 6, 1, 1, 6, 1, 6, 1, 6, 1, 6, 6,
    6, 6, 0, 0, 6, 0, 6, 0, 6, 0, 6, 6, 0, 6, 0, 0,
    2, 1, 5, 1, 1, 1, 5, 2, 5, 1, 5, 2, 2, 1, 2, 5,
    2, 2, 1, 5, 5, 5, 1, 2, 3, 3, 4, 1, 4, 3, 4, 1,
    1, 1, 4, 3, 1, 4, 4, 3, 3, 4, 3, 1, 1, 3, 1, 4,
    1, 6, 4, 6, 1, 6, 1, 4, 1, 6, 6, 4, 1, 4, 6, 4,
    1, 6, 4, 1, 1, 6, 4, 4, 6, 4, 0, 4, 0, 4, 6, 6,
    0, 4, 0, 6, 0, 0, 4, 6, 0, 6, 4, 4, 6, 0, 6, 4,
    6, 0, 6, 1, 1, 0, 6, 0, 1, 6, 0, 0, 1, 6, 6, 0,
    1, 6, 1, 0, 1, 1, 0, 6, 1, 0, 4, 0, 1, 4, 0, 4,
    1, 4, 0, 0, 1, 1, 4, 0, 4, 1, 0, 4, 1, 4, 1, 0,
    1, 5, 5, 4, 4, 5, 4, 1, 1, 1, 4, 5, 1, 5, 4, 5,
    1, 4, 1, 5, 1, 4, 4, 5, 1, 6, 0, 4, 6, 1, 0, 4,
    6, 6, 5, 5, 6, 5, 6, 5, 6, 5, 6, 6, 5, 6, 5, 5,
    6, 6, 3, 3, 6, 3, 6, 3, 6, 3, 6, 6, 3, 6, 3, 3,
    1, 5, 5, 6, 6, 5, 6, 1, 1, 1, 6, 5, 1, 5, 6, 5,
    1, 6, 1, 5, 1, 6, 6, 5, 1, 3, 3, 6, 6, 3, 6, 1,
    1, 1, 6, 3, 1, 3, 6, 3, 1, 6, 1, 3, 1, 6, 6, 3,
];

/// Working state for one generation, mirroring the engine's terrain
/// globals. Field/plane names track the C++ symbols.
struct Gen {
    /// `mapTerrainType_10B4E0`: scratch during shaping, final texture
    /// indices after `sub_44580`.
    ttype: Vec<u8>,
    /// `mapHeightmap_11B4E0`.
    height: Vec<u8>,
    /// `mapShading_12B4E0`.
    shading: Vec<u8>,
    /// `mapAngle_13B4E0`: class in bits 0-2, orientation in bits 4-6,
    /// the second-heightmap/deep-water flag in bit 3.
    angle: Vec<u8>,
    /// `x_BYTE_14B4E0_second_heightmap` — the cave ceiling mirror; stays
    /// all-zero off cave levels.
    ceiling: Vec<u8>,
    /// `mapEntityIndex_15B4E0` — the i16 fractal accumulator (scratch;
    /// dead after the height trunc).
    field: Vec<i16>,
    /// `pdwScreenBuffer_351628` corner-bucket scratch.
    scratch: Vec<u8>,
    /// The global PRNG stream `rand2_17B4E0` (the fractal uses a private
    /// copy).
    rand: u16,
    is_cave: bool,
    /// `MapBasicHeight_D41B7` — the cave ceiling pivot (44 off-cave).
    basic_height: u8,
    map_type: MapType,
}

/// Midpoint noise (`sub_B5EFA`/`sub_B5F8F`): one PRNG draw feeds both
/// moduli. `sub_B5F8F` casts the corner sum through `(uint16_t)` before
/// the `>> 2`; `sub_B5EFA` does not — but the two differ only by a
/// multiple of 65536, which the final `as i16` store discards, so a
/// single amp serves both.
#[inline]
fn amp(sr: u16, sum: i16, step: i32, gnarl: i32) -> i16 {
    let m1 = (2 * gnarl + 1) as u16; // (uint16_t)(2*a3+1), always odd
    let m2 = ((step << 6) + 1) as u16; // (uint16_t)((a1<<6)+1), always odd
    let v = (sr % m1) as i32 + (sr % m2) as i32 + ((sum as i32) >> 2) - 32 * step - gnarl;
    v as i16
}

impl Gen {
    // ---- Fractal (sub_B5E70 + sub_B5EFA + sub_B5F8F) -----------------

    /// Diamond-square fractal over the torus
    /// (`sub_B5E70_decompress_terrain_map_level`, :59). The seed value
    /// `raise` is planted at `off`; midpoints are written only if still
    /// zero. `sum_ent` is reset to `off` at each level and CARRIES from
    /// the square phase into the diamond phase (both net-wrap 256 in x
    /// and y, so the diamond phase effectively restarts at `off`).
    fn fractal(&mut self, seed: u16, off: u16, raise: i16, gnarl: i32) {
        let mut rnd = seed;
        self.field[off as usize] = raise;
        for i in (0..=7i32).rev() {
            let step = 1i32 << i;
            let count = 1i32 << (7 - i);
            let mut idx = Cell(off);
            for _ in 0..count {
                for _ in 0..count {
                    self.frac_square(step, &mut idx, gnarl, &mut rnd);
                }
                idx.yadd(2 * step); // C++ `sumEnt.word += (2*step)<<8`
            }
            for _ in 0..count {
                for _ in 0..count {
                    self.frac_diamond(step, &mut idx, gnarl, &mut rnd);
                }
                idx.yadd(2 * step);
            }
        }
    }

    /// Square step (`sub_B5EFA`, :1293): the center of the a1-square gets
    /// the four-corner sum (accumulated in i16, wrapping) plus noise.
    /// `idx` advances by (2*a1, 0).
    fn frac_square(&mut self, a1: i32, idx: &mut Cell, gnarl: i32, rnd: &mut u16) {
        let mut sum = self.field[idx.idx()]; // (0,0)
        idx.xadd(2 * a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (2a1,0)
        idx.yadd(2 * a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (2a1,2a1)
        idx.xadd(-2 * a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (0,2a1)
        idx.xadd(a1);
        idx.yadd(-a1); // center (a1,a1)
        let sr = lcg(rnd);
        if self.field[idx.idx()] == 0 {
            self.field[idx.idx()] = amp(sr, sum, a1, gnarl);
        }
        idx.xadd(a1);
        idx.yadd(-a1); // return (2a1,0)
    }

    /// Diamond step (`sub_B5F8F`, :1323): the top and left edge midpoints
    /// each get their four-neighbor sum plus noise (one PRNG draw each;
    /// the private stream advances twice). `idx` advances by (2*a1, 0).
    fn frac_diamond(&mut self, a1: i32, idx: &mut Cell, gnarl: i32, rnd: &mut u16) {
        let mut sum = self.field[idx.idx()]; // (0,0)
        let mut sum2 = sum;
        idx.xadd(a1);
        idx.yadd(-a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (a1,-a1)
        idx.xadd(a1);
        idx.yadd(a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (2a1,0)
        idx.xadd(-a1);
        idx.yadd(a1);
        sum = sum.wrapping_add(self.field[idx.idx()]); // (a1,a1)
        let sr = lcg(rnd);
        sum2 = sum2.wrapping_add(self.field[idx.idx()]); // (a1,a1) into sum2
        idx.yadd(-a1); // TOP (a1,0)
        if self.field[idx.idx()] == 0 {
            self.field[idx.idx()] = amp(sr, sum, a1, gnarl);
        }
        idx.xadd(-2 * a1);
        idx.yadd(a1);
        sum2 = sum2.wrapping_add(self.field[idx.idx()]); // (-a1,a1)
        idx.xadd(a1);
        idx.yadd(a1);
        sum2 = sum2.wrapping_add(self.field[idx.idx()]); // (0,2a1)
        idx.yadd(-a1); // LEFT (0,a1)
        let sr = lcg(rnd);
        if self.field[idx.idx()] == 0 {
            self.field[idx.idx()] = amp(sr, sum2, a1, gnarl);
        }
        idx.xadd(2 * a1);
        idx.yadd(-a1); // return (2a1,0)
    }

    // ---- Height ------------------------------------------------------

    /// Normalize the fractal field into 0..=196 heights
    /// (`sub_44DB0_truncTerrainHeight`, :86). The scaled product is
    /// formed in wrapping `int` and viewed as `uint32`; bit 15 set (a
    /// negative result) truncates to water, then clamp to 196.
    fn trunc_height(&mut self) {
        let mut max_ent = -32000i32;
        for &v in &self.field {
            if v as i32 > max_ent {
                max_ent = v as i32;
            }
        }
        let rev = if max_ent != 0 {
            0xC4_0000i32 / max_ent
        } else {
            0
        };
        for i in 0..GRID {
            let w = rev.wrapping_mul(self.field[i] as i32) >> 16;
            self.field[i] = 0;
            let mut wv = w as u32;
            if wv & 0x8000 != 0 {
                wv = 0;
            }
            if wv > 196 {
                wv = 196;
            }
            self.height[i] = wv as u8;
        }
    }

    /// Seed the angle/class plane and carve the smoothing basins
    /// (`sub_44E40`, :176). Nonzero height -> class 5, else water 0.
    /// Then up to `count` times: probe (up to 1000 PRNG draws) for a
    /// land tile above `min_smooth` and run `smooth_tiles` on it; a full
    /// dry sweep abandons the rest. Ends filling the type plane with 255.
    fn field_seed(&mut self, count: i32, min_smooth: u8) {
        for i in 0..GRID {
            self.angle[i] = if self.height[i] != 0 { 5 } else { 0 };
        }
        let mut loc_count = count;
        let mut i: i32 = 0;
        while loc_count > 0 && i < 1000 {
            i = 0;
            while i < 1000 {
                self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
                let idx = (self.rand % 0xFFFF) as usize;
                if self.height[idx] > min_smooth && self.angle[idx] != 0 {
                    self.smooth_tiles(idx as u16);
                    loc_count -= 1;
                    break;
                }
                i += 1;
            }
        }
        self.ttype.fill(255);
    }

    /// Descend from a source cell to a basin, clamping heights down along
    /// the way (`sub_44EE0_smooth_tiles`, :1377). The type plane is the
    /// visited mask (3 unvisited, 0 visited); at each step it moves to
    /// the lowest unvisited 8-neighbor, stopping at water or a dead end.
    /// Visited cells become water class.
    fn smooth_tiles(&mut self, start: u16) {
        self.ttype.fill(3);
        let mut central = self.height[start as usize];
        let mut t1 = Cell(start);
        let mut t2 = Cell(0);
        loop {
            self.ttype[t1.idx()] = 0;
            let mut min_h = 255u8;
            // walk N, NE, E, SE, S, SW, W, NW keeping the first minimum
            t1.ymm();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.xpp();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.ypp();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.ypp();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.xmm();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.xmm();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.ymm();
            self.probe_min(t1, &mut min_h, &mut t2);
            t1.ymm();
            self.probe_min(t1, &mut min_h, &mut t2);
            if self.angle[t2.idx()] == 0 || min_h == 255 {
                break;
            }
            if min_h > central {
                self.height[t2.idx()] = central;
            }
            central = self.height[t2.idx()];
            t1 = t2;
            if central == 0 {
                break;
            }
        }
        for i in 0..GRID {
            if self.ttype[i] == 0 {
                self.angle[i] = 0;
            }
        }
    }

    /// Flatten any all-water 2x2 quad to its minimum height, to a
    /// fixpoint (`sub_45AA0_setMax4Tiles`, :209).
    fn set_max4_tiles(&mut self) {
        loop {
            let mut run_again = false;
            for i in 0..=0xFFFFu16 {
                let mut c = Cell(i);
                let mut angle_index = if self.angle[c.idx()] == 0 { 1 } else { 0 };
                let mut min_h = self.height[c.idx()];
                let mut max_h = min_h;
                c.xpp();
                self.min_max(&c, &mut min_h, &mut max_h);
                if self.angle[c.idx()] == 0 {
                    angle_index += 1;
                }
                c.ypp();
                self.min_max(&c, &mut min_h, &mut max_h);
                if self.angle[c.idx()] == 0 {
                    angle_index += 1;
                }
                c.xmm();
                self.min_max(&c, &mut min_h, &mut max_h);
                if self.angle[c.idx()] == 0 {
                    angle_index += 1;
                }
                c.ymm();
                if max_h != min_h && angle_index == 4 {
                    run_again = true;
                    self.height[c.idx()] = min_h;
                    c.xpp();
                    self.height[c.idx()] = min_h;
                    c.ypp();
                    self.height[c.idx()] = min_h;
                    c.xmm();
                    self.height[c.idx()] = min_h;
                    c.ymm();
                }
            }
            if !run_again {
                break;
            }
        }
    }

    #[inline]
    fn min_max(&self, c: &Cell, min_h: &mut u8, max_h: &mut u8) {
        let h = self.height[c.idx()];
        if *min_h > h {
            *min_h = h;
        }
        if *max_h < h {
            *max_h = h;
        }
    }

    /// `smooth_tiles` neighbor probe: keep the lowest unvisited (type != 0)
    /// cell seen, first-minimum wins.
    #[inline]
    fn probe_min(&self, c: Cell, min_h: &mut u8, t2: &mut Cell) {
        if self.ttype[c.idx()] != 0 && *min_h > self.height[c.idx()] {
            *min_h = self.height[c.idx()];
            *t2 = c;
        }
    }

    /// `sub_45210` neighbor probe: track height max/min and count classes
    /// 5 and 2.
    #[inline]
    fn hclass(&self, c: Cell, min_h: &mut u8, max_h: &mut u8, a5: &mut i32, a2: &mut i32) {
        let h = self.height[c.idx()];
        if *max_h < h {
            *max_h = h;
        }
        if *min_h > h {
            *min_h = h;
        }
        match self.angle[c.idx()] {
            5 => *a5 += 1,
            2 => *a2 += 1,
            _ => {}
        }
    }

    /// `sub_45600` second-loop probe: count classes 2, 3, 4, 5.
    #[inline]
    fn count4(&self, c: Cell, a4: &mut i32, a2: &mut i32, a3: &mut i32, a5: &mut i32) {
        match self.angle[c.idx()] {
            3 => *a3 += 1,
            2 => *a2 += 1,
            5 => *a5 += 1,
            4 => *a4 += 1,
            _ => {}
        }
    }

    // ---- Angle / class shaping ---------------------------------------

    /// First slope classification (`sub_440D0`, :271). For class-5 cells,
    /// a shallow (`<= a1`) 5-cell relief demotes to class 3, or 4 when
    /// exactly at the cut. Then any quad mixing 3 and 5 without a 2
    /// upgrades its 3s to 4.
    fn sub_440d0(&mut self, a1: u16) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            if self.angle[c.idx()] != 5 {
                continue;
            }
            let mut max_h = 0u8;
            let mut min_h = 255u8;
            if self.height[c.idx()] != 0 {
                max_h = self.height[c.idx()];
            }
            if self.height[c.idx()] < 255 {
                min_h = self.height[c.idx()];
            }
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // N
            c.xpp();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // E
            c.xmm();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // S
            c.xmm();
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // W
            let diff = max_h as i32 - min_h as i32;
            c.xpp(); // back to center
            if diff <= a1 as i32 {
                self.angle[c.idx()] = if diff == a1 as i32 { 4 } else { 3 };
            }
        }
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let (mut a3, mut a2, mut a5) = (0i32, 0i32, 0i32);
            Self::count3(self.angle[c.idx()], &mut a3, &mut a2, &mut a5);
            c.xpp();
            Self::count3(self.angle[c.idx()], &mut a3, &mut a2, &mut a5);
            c.ypp();
            Self::count3(self.angle[c.idx()], &mut a3, &mut a2, &mut a5);
            c.xmm();
            Self::count3(self.angle[c.idx()], &mut a3, &mut a2, &mut a5);
            c.ymm();
            if a2 == 0 && a3 != 0 && a5 != 0 {
                self.promote_quad(&mut c, 3, 4);
            }
        }
    }

    #[inline]
    fn count3(v: u8, a3: &mut i32, a2: &mut i32, a5: &mut i32) {
        match v {
            3 => *a3 += 1,
            2 => *a2 += 1,
            5 => *a5 += 1,
            _ => {}
        }
    }

    /// Overwrite each cell of the quad that currently equals `from` with
    /// `to`, in the C++ (0,0)-(1,0)-(1,1)-(0,1) order; leaves `c` back
    /// at the corner.
    #[inline]
    fn promote_quad(&mut self, c: &mut Cell, from: u8, to: u8) {
        if self.angle[c.idx()] == from {
            self.angle[c.idx()] = to;
        }
        c.xpp();
        if self.angle[c.idx()] == from {
            self.angle[c.idx()] = to;
        }
        c.ypp();
        if self.angle[c.idx()] == from {
            self.angle[c.idx()] = to;
        }
        c.xmm();
        if self.angle[c.idx()] == from {
            self.angle[c.idx()] = to;
        }
        c.ymm();
    }

    /// Flat-plateau upgrade (`sub_45060`, :390): snapshot angle into the
    /// type plane, then any nonzero-class cell whose 3x3 max is below
    /// `max_cut` and whose relief is `<= max_diff_cut` becomes class 5.
    fn sub_45060(&mut self, max_cut: u8, max_diff_cut: u8) {
        self.ttype.copy_from_slice(&self.angle);
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let mut max_h = 0u8;
            let mut min_h = 255u8;
            if self.height[c.idx()] != 0 {
                max_h = self.height[c.idx()];
            }
            if self.height[c.idx()] < 255 {
                min_h = self.height[c.idx()];
            }
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.xpp();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.xmm();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.xmm();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h);
            c.xpp();
            c.ypp(); // back to center
            if (max_h as i32) < max_cut as i32
                && (max_h as i32 - min_h as i32) <= max_diff_cut as i32
                && self.angle[c.idx()] != 0
            {
                self.angle[c.idx()] = 5;
            }
        }
    }

    /// Quad interface upgrades (`sub_44320`, :462): three rules applied
    /// in order on the same quad, each mutating angle in place using the
    /// pre-scan class counts.
    fn sub_44320(&mut self) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let (mut a0, mut a3, mut a5) = (0i32, 0i32, 0i32);
            Self::count053(self.angle[c.idx()], &mut a0, &mut a5, &mut a3);
            c.xpp();
            Self::count053(self.angle[c.idx()], &mut a0, &mut a5, &mut a3);
            c.ypp();
            Self::count053(self.angle[c.idx()], &mut a0, &mut a5, &mut a3);
            c.xmm();
            Self::count053(self.angle[c.idx()], &mut a0, &mut a5, &mut a3);
            c.ymm();
            if a3 != 0 && a5 != 0 {
                self.promote_quad(&mut c, 5, 4);
            }
            if a3 != 0 && a0 != 0 {
                self.promote_quad(&mut c, 3, 4);
            }
            if a0 != 0 && a5 != 0 {
                // any nonzero-class cell of the quad -> 4
                self.promote_quad_nonzero(&mut c);
            }
        }
    }

    #[inline]
    fn count053(v: u8, a0: &mut i32, a5: &mut i32, a3: &mut i32) {
        match v {
            0 => *a0 += 1,
            5 => *a5 += 1,
            3 => *a3 += 1,
            _ => {}
        }
    }

    #[inline]
    fn promote_quad_nonzero(&mut self, c: &mut Cell) {
        if self.angle[c.idx()] != 0 {
            self.angle[c.idx()] = 4;
        }
        c.xpp();
        if self.angle[c.idx()] != 0 {
            self.angle[c.idx()] = 4;
        }
        c.ypp();
        if self.angle[c.idx()] != 0 {
            self.angle[c.idx()] = 4;
        }
        c.xmm();
        if self.angle[c.idx()] != 0 {
            self.angle[c.idx()] = 4;
        }
        c.ymm();
    }

    /// Beach-plateau class 2 (`sub_45210`, :556): snapshot angle into the
    /// type plane, then a class-5 cell below `max_cut` with relief
    /// `<= max_diff_cut` whose 8 neighbors are all class 5 or 2 becomes
    /// class 2.
    fn sub_45210(&mut self, max_cut: u8, max_diff_cut: u8) {
        self.ttype.copy_from_slice(&self.angle);
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let mut max_h = 0u8;
            let mut min_h = 255u8;
            let (mut a5, mut a2) = (0i32, 0i32);
            if self.height[c.idx()] > 0 {
                max_h = self.height[c.idx()];
            }
            if self.height[c.idx()] < 255 {
                min_h = self.height[c.idx()];
            }
            c.ymm();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.xpp();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.ypp();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.ypp();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.xmm();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.xmm();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.ymm();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.ymm();
            self.hclass(c, &mut min_h, &mut max_h, &mut a5, &mut a2);
            c.xpp();
            c.ypp(); // back to center
            if (max_h as i32) < max_cut as i32
                && (max_h as i32 - min_h as i32) <= max_diff_cut as i32
                && self.angle[c.idx()] == 5
                && a5 + a2 == 8
            {
                self.angle[c.idx()] = 2;
            }
        }
    }

    /// Peak class 6 (`sub_454F0`, :668): a cell above `max_cut` with a
    /// 5-cell relief below `max_diff_cut` and nonzero class becomes 6.
    fn sub_454f0(&mut self, max_cut: u8, max_diff_cut: u8) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            if self.height[c.idx()] <= max_cut {
                continue;
            }
            let mut max_h = 0u8;
            let mut min_h = 255u8;
            if self.height[c.idx()] != 0 {
                max_h = self.height[c.idx()];
            }
            if self.height[c.idx()] < 255 {
                min_h = self.height[c.idx()];
            }
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // N
            c.xpp();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // E
            c.xmm();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // S
            c.xmm();
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // W
            c.xpp(); // back to center
            if self.angle[c.idx()] != 0 && (max_h as i32 - min_h as i32) < max_diff_cut as i32 {
                self.angle[c.idx()] = 6;
            }
        }
    }

    /// Cliff class 1 (`sub_45600`, :724). First loop: any nonzero-class
    /// cell with a 5-cell relief `>= a1` becomes class 1. Second loop:
    /// class-6 cells adjacent to certain class mixes collapse to 1.
    fn sub_45600(&mut self, a1: u8) {
        self.ttype.copy_from_slice(&self.angle);
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let mut max_h = 0u8;
            let mut min_h = 255u8;
            if self.height[c.idx()] != 0 {
                max_h = self.height[c.idx()];
            }
            if self.height[c.idx()] < 255 {
                min_h = self.height[c.idx()];
            }
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // N
            c.xpp();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // E
            c.xmm();
            c.ypp();
            self.min_max(&c, &mut min_h, &mut max_h); // S
            c.xmm();
            c.ymm();
            self.min_max(&c, &mut min_h, &mut max_h); // W
            c.xpp(); // back to center
            if self.angle[c.idx()] != 0 && (max_h as i32 - min_h as i32) >= a1 as i32 {
                self.angle[c.idx()] = 1;
            }
        }
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            if self.angle[c.idx()] != 6 {
                continue;
            }
            let (mut a4, mut a2, mut a3, mut a5) = (0i32, 0i32, 0i32, 0i32);
            c.ymm();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.xpp();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.ypp();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.ypp();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.xmm();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.xmm();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.ymm();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.ymm();
            self.count4(c, &mut a4, &mut a2, &mut a3, &mut a5);
            c.xpp();
            c.ypp(); // back to center
            if a3 != 0 {
                if a2 != 0 || a5 != 0 || a4 != 0 {
                    self.angle[c.idx()] = 1;
                }
            } else if a2 != 0 || (a5 != 0 && a4 != 0) {
                self.angle[c.idx()] = 1;
            }
        }
    }

    /// Majority smoothing (`sub_43FC0`, :882): a cell whose N neighbor is
    /// nonzero and whose other 7 surrounding cells all share that class
    /// adopts it.
    fn sub_43fc0(&mut self) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            c.ymm(); // N is the reference
            let center_angle = self.angle[c.idx()];
            let mut same = 0i32;
            c.xpp();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.ypp();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.ypp();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.xmm();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.xmm();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.ymm();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.ymm();
            same += (center_angle == self.angle[c.idx()]) as i32;
            c.xpp();
            c.ypp(); // (0,0)
            if center_angle != 0 && same == 7 {
                self.angle[c.idx()] = center_angle;
            }
        }
    }

    // ---- Height smoothing / rivers -----------------------------------

    /// Land height smoothing (`sub_43970` + `sub_439A0`, :924/:1459).
    /// In-place sequential sweep: each land cell is pulled toward its
    /// 8-neighbor average, fully for large relief, halfway for moderate.
    fn sub_43970(&mut self) {
        for i in 0..=0xFFFFu16 {
            let v = self.smooth_height(i);
            self.height[i as usize] = v;
        }
    }

    fn smooth_height(&self, index: u16) -> u8 {
        let mut result = self.height[index as usize];
        let c0 = Cell(index);
        if self.angle[c0.idx()] & 7 == 0 {
            return result;
        }
        let center = self.height[c0.idx()];
        let mut max_h = center;
        let mut min_h = center;
        let mut sum = 0i32;
        let mut c = c0;
        // N, NE, E, SE, S, SW, W, NW
        for &(step_x, step_y) in &[
            (0, -1),
            (1, 0),
            (0, 1),
            (0, 1),
            (-1, 0),
            (-1, 0),
            (0, -1),
            (0, -1),
        ] {
            c.xadd(step_x);
            c.yadd(step_y);
            let h = self.height[c.idx()];
            sum += h as i32;
            if h > max_h {
                max_h = h;
            }
            if h < min_h {
                min_h = h;
            }
        }
        let avg = (sum >> 3) as u8;
        let halfway = ((center as i32 + avg as i32) >> 1) as u8;
        let d_down = center.wrapping_sub(min_h);
        if d_down <= 4 {
            let d_up = max_h.wrapping_sub(center);
            if d_up <= 4 {
                return result;
            }
            if d_up <= 10 {
                return halfway;
            }
            result = avg;
        } else if d_down <= 10 {
            return halfway;
        } else {
            result = avg;
        }
        result
    }

    /// Shore zeroing (`sub_43EE0`, :935): a quad touching class 4 and
    /// sea-level water (min water height 0) has all four heights zeroed.
    fn sub_43ee0(&mut self) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            let mut h1 = self.height[c.idx()]; // center height, min over water
            let (mut a4, mut a0) = (0i32, 0i32);
            c.xpp();
            self.river_cell(&c, &mut a4, &mut a0, &mut h1);
            c.ypp();
            self.river_cell(&c, &mut a4, &mut a0, &mut h1);
            c.xmm();
            self.river_cell(&c, &mut a4, &mut a0, &mut h1);
            c.ymm();
            if a4 != 0 && a0 != 0 && h1 == 0 {
                self.height[c.idx()] = 0;
                c.xpp();
                self.height[c.idx()] = 0;
                c.ypp();
                self.height[c.idx()] = 0;
                c.xmm();
                self.height[c.idx()] = 0;
                c.ymm();
            }
        }
    }

    #[inline]
    fn river_cell(&self, c: &Cell, a4: &mut i32, a0: &mut i32, h1: &mut u8) {
        match self.angle[c.idx()] {
            0 => {
                *a0 += 1;
                if self.height[c.idx()] < *h1 {
                    *h1 = self.height[c.idx()];
                }
            }
            4 => *a4 += 1,
            _ => {}
        }
    }

    // ---- Texture / rotation selection (sub_44580) --------------------

    /// Assign texture indices and orientation codes from the corner
    /// buckets (`sub_44580`, :1011). Builds the 2401 corner-combination
    /// buckets from `unk_D47E0`, then for each cell keyed by its quad
    /// classes picks (PRNG-weighted, overflow doubling slot 0) a texture
    /// and rotation.
    fn sub_44580(&mut self) {
        // clear the 2401 bucket headers (stride 25)
        for b in 0..2401usize {
            self.scratch[25 * b] = 0;
        }
        for i in 0..148usize {
            let d0 = UNK_D47E0[4 * i] as i32;
            let d1 = UNK_D47E0[4 * i + 1] as i32;
            let d2 = UNK_D47E0[4 * i + 2] as i32;
            let d3 = UNK_D47E0[4 * i + 3] as i32;
            if d0 < 0 || d1 < 0 || d2 < 0 || d3 < 0 {
                continue;
            }
            // eight dihedral bucket placements, each with a rotation code
            let regs: [(i32, u8); 8] = [
                (49 * d1 + 7 * d2 + d3 + 343 * d0, 0),
                (49 * d0 + d2 + 7 * d3 + 343 * d1, 16),
                (49 * d3 + d1 + 7 * d0 + 343 * d2, 48),
                (49 * d2 + d0 + 7 * d1 + 343 * d3, 32),
                (49 * d2 + 7 * d3 + d0 + 343 * d1, 96),
                (49 * d1 + 7 * d0 + d3 + 343 * d2, 112),
                (49 * d0 + 7 * d1 + d2 + 343 * d3, 80),
                (343 * d0 + 7 * d2 + d1 + 49 * d3, 64),
            ];
            for (bucket, rot) in regs {
                let base = 25 * bucket as usize;
                let cnt = self.scratch[base];
                if cnt < 12 {
                    self.scratch[base + cnt as usize + 13] = rot;
                    self.scratch[base + cnt as usize + 1] = i as u8;
                    self.scratch[base] = cnt + 1;
                }
            }
        }
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            if self.ttype[c.idx()] != 0 {
                continue;
            }
            let p1 = (self.angle[c.idx()] & 7) as usize; // (0,0)
            c.xpp();
            let p2 = (self.angle[c.idx()] & 7) as usize; // (1,0)
            c.ypp();
            let p3 = (self.angle[c.idx()] & 7) as usize; // (1,1)
            c.xmm();
            let p4 = (self.angle[c.idx()] & 7) as usize; // (0,1)
            c.ymm(); // back to center
            let base = 25 * (343 * p1 + 49 * p2 + p4 + 7 * p3);
            let cnt = self.scratch[base];
            if cnt != 0 {
                self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
                let k = self.rand % (cnt as u16 + 1);
                let slot = if k >= cnt as u16 {
                    base
                } else {
                    base + k as usize
                };
                self.ttype[c.idx()] = self.scratch[slot + 1];
                self.angle[c.idx()] = (self.angle[c.idx()] & 7) + self.scratch[slot + 13];
            } else {
                self.ttype[c.idx()] = 1;
            }
        }
    }

    // ---- Ceiling / deep-water ----------------------------------------

    /// Cave ceiling mirror (`sub_43B40` + `sub_43BB0`, :1158/:1546). The
    /// ceiling is `basic_height - height` (clamped), with angle bit 3
    /// tracking whether the ceiling clears the floor; `sub_43BB0` then
    /// fuzzes open cells with its own 32-bit PRNG (seed 37487429).
    fn sub_43b40(&mut self) {
        let bh = self.basic_height;
        for i in 0..GRID {
            let mut loc = self.height[i];
            if loc > bh {
                loc = bh;
            }
            let sh = bh - loc; // loc <= bh, no underflow
            self.ceiling[i] = sh;
            if sh as i32 > self.height[i] as i32 {
                self.angle[i] &= 0xF7;
            } else {
                self.ceiling[i] = self.height[i].wrapping_sub(1);
                self.angle[i] |= 8;
            }
        }
        // sub_43BB0: fuzz open ceiling cells, then re-derive bit 3
        let mut rs: u32 = 37_487_429;
        for i in 0..GRID {
            if self.angle[i] & 8 == 0 {
                rs = rs.wrapping_mul(9377).wrapping_add(9439);
                // C++ clamps [0,254] with two sequential ifs (0 <= 254).
                let fuzzy = ((rs % 7) as i32 - 3 + self.ceiling[i] as i32).clamp(0, 254);
                self.ceiling[i] = fuzzy as u8;
            }
        }
        for i in 0..GRID {
            if self.ceiling[i] as i32 > self.height[i] as i32 {
                self.angle[i] &= 0xF7;
            } else {
                self.ceiling[i] = self.height[i].wrapping_sub(1);
                self.angle[i] |= 8;
            }
        }
    }

    /// Day/night deep-water flag (`sub_43D50`, :1183): clear angle bit 3
    /// everywhere, then set it on sea-level water cells whose 8 neighbors
    /// are all water and whose NW-side quad has no assigned type.
    fn sub_43d50(&mut self) {
        for i in 0..=0xFFFFu16 {
            let mut c = Cell(i);
            self.angle[c.idx()] &= 0xF7;
            if self.height[c.idx()] != 0 {
                continue;
            }
            let mut n = 0i32;
            c.ymm();
            n += (self.height[c.idx()] != 0) as i32;
            c.xpp();
            n += (self.height[c.idx()] != 0) as i32;
            c.ypp();
            n += (self.height[c.idx()] != 0) as i32;
            c.ypp();
            n += (self.height[c.idx()] != 0) as i32;
            c.xmm();
            n += (self.height[c.idx()] != 0) as i32;
            c.xmm();
            n += (self.height[c.idx()] != 0) as i32;
            c.ymm();
            n += (self.height[c.idx()] != 0) as i32;
            c.ymm();
            n += (self.height[c.idx()] != 0) as i32;
            c.xpp();
            c.ypp(); // back to center
            if n != 0 {
                continue;
            }
            // quad (0,0),(-1,0),(-1,-1),(0,-1) type test
            let mut t = 0i32;
            t += (self.ttype[c.idx()] != 0) as i32;
            c.xmm();
            t += (self.ttype[c.idx()] != 0) as i32;
            c.ymm();
            t += (self.ttype[c.idx()] != 0) as i32;
            c.xpp();
            t += (self.ttype[c.idx()] != 0) as i32;
            c.ypp();
            if t == 0 {
                self.angle[c.idx()] |= 8;
            }
        }
    }

    /// Shading (`sub_44D00`, :1242): shade = NW-to-SE height gradient +
    /// 32, in wrapping byte arithmetic viewed signed. Flat cells get a
    /// PRNG dither in 28..=36 (global stream reset to 0 first), slopes
    /// clamp into 28..=31 / 40..=47. Non-day maps mirror it as `64 - s`.
    fn sub_44d00(&mut self) {
        self.rand = 0;
        let non_day = self.map_type != MapType::Day;
        for i in 0..=0xFFFFu16 {
            let c = Cell(i);
            let hi = self.height[c.at(-1, -1)];
            let lo = self.height[c.at(1, 1)];
            let b = hi.wrapping_sub(lo).wrapping_add(32);
            let v: u8 = if b == 32 {
                self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
                (self.rand % 9) as u8 + 28
            } else if (b as i8) >= 28 {
                if (b as i8) > 40 { (b & 7) + 40 } else { b }
            } else {
                (b & 3) + 28
            };
            self.shading[i as usize] = if non_day { (64 - v as i32) as u8 } else { v };
        }
    }
}

/// Generate terrain for one MC2 level from its parsed GEN_MAP params
/// and header (map type + cave basic height).
///
/// Pass order (`GenerateLevelMap_43830`, Terrain.cpp:19): fractal
/// (`sub_B5E70`) -> height trunc (`sub_44DB0`) -> field/river seeding
/// (`sub_44E40`) -> max-4-tiles flatten (`sub_45AA0`) -> the angle/class
/// shaping passes (`sub_440D0`/`45060`/`44320`/`45210`/`454F0`/`45600`/
/// `43FC0`) -> type clear -> height smoothing (`sub_43970`) -> shore
/// rivers (`sub_43EE0`) -> texture/angle set (`sub_44580`) -> cave vs.
/// day ceiling (`sub_43B40`/`sub_43D50`) -> shading (`sub_44D00`).
pub fn generate(level: &Mc2Level) -> Mc2Terrain {
    let g = &level.gen_map;
    let is_cave = matches!(level.header.map_type, MapType::Cave);
    // Retail LevelInit_56C00: cave levels pivot the ceiling on level byte
    // 0x05; off-cave it stays the weak default 44 (Terrain.cpp:15).
    let basic_height = if is_cave {
        level.header.basic_height
    } else {
        44
    };
    let mut st = Gen {
        ttype: vec![0u8; GRID],
        height: vec![0u8; GRID],
        shading: vec![0u8; GRID],
        angle: vec![0u8; GRID],
        ceiling: vec![0u8; GRID],
        field: vec![0i16; GRID],
        scratch: vec![0u8; SCRATCH],
        rand: g.seed,
        is_cave,
        basic_height,
        map_type: level.header.map_type,
    };
    st.fractal(g.seed, g.off, g.raise, g.gnarl as i32);
    st.trunc_height();
    st.field_seed(g.river as i32, g.lriver as u8);
    st.set_max4_tiles();
    st.sub_440d0(g.snlin);
    st.sub_45060(g.snflt as u8, g.bhlin as u8);
    st.sub_44320();
    st.sub_45210(g.snflt as u8, g.bhlin as u8);
    st.sub_454f0(g.sourc as u8, g.rkste as u8);
    st.sub_45600(g.bhflt as u8);
    st.sub_43fc0();
    st.ttype.fill(0);
    st.sub_43970();
    st.sub_43ee0();
    st.sub_44580();
    if st.is_cave {
        st.sub_43b40();
    } else {
        st.sub_43d50();
    }
    st.sub_44d00();
    Mc2Terrain {
        tile_type: st.ttype,
        height: st.height,
        shading: st.shading,
        angle: st.angle,
        ceiling: st.ceiling,
    }
}
