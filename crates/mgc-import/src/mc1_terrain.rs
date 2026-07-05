//! Native port of Magic Carpet 1's terrain generator.
//!
//! Reference: the remc1 decompilation (`reference/remc1/sub_main.cpp`,
//! GPL3 assumed, same stance as remc2). Entry point in the original is
//! `sub_31AA0_31AE0` (:39289), called at level load with the raw level
//! buffer — GEN_MAP parameters are read by byte offset from its start.
//! Full pipeline documentation: docs/ROADMAP.md "MC1 reference generator
//! found". Each pass below cites its source function.
//!
//! Fidelity notes that shape the code:
//!
//! - The 256x256 grid is toroidal and the engine walks it with 8-bit
//!   coordinate arithmetic on a `u16` cell index (`x` = low byte, `y` =
//!   high byte, wrapping per byte). [`cell`] reproduces that exactly.
//! - One 16-bit LCG (`x = 9377*x + 9439`) drives everything, but in two
//!   streams: the fractal receives the seed BY VALUE and advances a
//!   private copy, so the river pass and the texture pass consume a
//!   global stream that starts over from the seed; the shading pass then
//!   resets the global stream to 0. The tile-type layer is therefore
//!   seeded-random and depends on the exact draw order of every pass.
//! - Class values live in the low 3 bits of the angle plane: 0 water,
//!   1 dark basalt, 2 sand, 3 vegetation, 4 dirt, 5 sand-variant (the
//!   default land fill), 6 brown rock. Texture selection then adds the
//!   dihedral orientation code in bits 4-6 (the renderer's angle.bin
//!   decode) and the deep-water flag in bit 3.

use crate::level_mc1::GenMap;

/// Cells in the 256x256 terrain grid.
pub const GRID: usize = 0x10000;

/// The four generated planes, engine layout (index = y*256 + x), matching
/// the byte planes the MC2 oracle emits and `.mgcl` stores.
pub struct Mc1Terrain {
    pub tile_type: Vec<u8>,
    pub height: Vec<u8>,
    pub shading: Vec<u8>,
    pub angle: Vec<u8>,
}

/// Generate terrain for one MC1 level from its GEN_MAP parameters.
///
/// Original pass order (sub_31AA0_31AE0): fractal, normalize, rivers,
/// water flattening, vegetation, low-flat revert, boundary dirt, beach
/// interior, rock, majority smoothing, type clear, land smoothing, shore
/// zeroing, texture selection, deep-water flag, shading.
///
/// `snlin` is consumed by nothing (MC1 has no snow pass; snow is the
/// arctic tileset's textures) and `pre_header` is not a generator input.
pub fn generate(g: &GenMap) -> Mc1Terrain {
    let mut state = Gen {
        field: vec![0i16; GRID],
        height: vec![0u8; GRID],
        class: vec![0u8; GRID],
        types: vec![0u8; GRID],
        shading: vec![0u8; GRID],
        rand: g.seed as u16,
    };
    state.fractal(g.seed as u16, g.off as u16, g.raise as i16, g.gnarl as u16);
    state.normalize();
    state.rivers(g.river as i32, g.sourc as u8);
    state.flatten_water();
    state.vegetation(g.snflt as u16);
    state.low_flat(g.bhlin as u8, g.bhflt as u8);
    state.boundary_dirt();
    state.beach_interior(g.bhlin as u8, g.bhflt as u8);
    state.rock(g.rkste as u8);
    state.majority();
    state.types.fill(0);
    state.smooth_land();
    state.shore_zero();
    state.textures();
    state.deep_water();
    state.shading_pass();
    Mc1Terrain {
        tile_type: state.types,
        height: state.height,
        shading: state.shading,
        angle: state.class,
    }
}

/// The engine's PRNG (`pseudoRand_12C1E0`).
#[inline]
fn lcg(s: &mut u16) -> u16 {
    *s = s.wrapping_mul(9377).wrapping_add(9439);
    *s
}

/// Toroidal cell addressing: the engine moves around the map by
/// incrementing/decrementing the low (x) and high (y) bytes of a u16
/// index, each wrapping independently.
#[inline]
fn cell(i: u16, dx: i32, dy: i32) -> usize {
    let x = (i as u8).wrapping_add(dx as u8);
    let y = ((i >> 8) as u8).wrapping_add(dy as u8);
    ((y as usize) << 8) | x as usize
}

/// The 8-neighborhood in the engine's scan order (N, NE, E, SE, S, SW,
/// W, NW). Order matters where the engine keeps the first minimum (the
/// river descent walk).
const N8: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// The 4-neighborhood (N, E, S, W) used by the flatness passes.
const PLUS4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// The 2x2 quad with this cell as its top-left corner. Quad passes read
/// and write these four cells together.
const QUAD: [(i32, i32); 4] = [(0, 0), (1, 0), (1, 1), (0, 1)];

struct Gen {
    /// Fractal scratch field (`mapEntityIndex_10C1E0` doubles as this
    /// during generation), i16 per cell.
    field: Vec<i16>,
    /// `mapHeightmap_DC1E0`.
    height: Vec<u8>,
    /// `mapAngle_FC1E0`: class in bits 0-2 during classification, then
    /// orientation bits 4-6 and deep-water bit 3.
    class: Vec<u8>,
    /// `mapTerrainType_CC1E0`: scratch during generation, final texture
    /// indices after [`Gen::textures`].
    types: Vec<u8>,
    /// `mapShading_EC1E0`.
    shading: Vec<u8>,
    /// The global PRNG stream (fractal uses a private copy).
    rand: u16,
}

impl Gen {
    /// Diamond-square fractal over the torus (`sub_725C8`, :81579).
    ///
    /// `origin` is the cell index where `raise` is planted as the seed
    /// value; every midpoint is written only if still zero, so the seed
    /// cell survives. Amplitude per write:
    /// `rand%(2*gnarl+1) + rand%(step*64+1) - 32*step - gnarl`
    /// (one PRNG draw feeds both moduli). remc1's decompile loses the
    /// gnarl argument in a reconstructed stack frame (`savedregs[5]`,
    /// frozen to 0); remc2's byte-identical fractal
    /// (`sub_B5E70_decompress_terrain_map_level`) names it explicitly,
    /// and our earlier oracle validation proved MC1 heights depend on it.
    fn fractal(&mut self, seed: u16, origin: u16, raise: i16, gnarl: u16) {
        // The engine passes its global PRNG by value: the fractal
        // advances this private copy, leaving the global stream at the
        // seed for the river and texture passes.
        let mut s = seed;
        self.field[origin as usize] = raise;
        for level in (0..=7i32).rev() {
            let step = 1i32 << level;
            let count = 1i32 << (7 - level);
            let mut pos = origin;
            for _ in 0..count {
                for _ in 0..count {
                    pos = self.square(step, pos, &mut s, gnarl);
                }
                pos = cell(pos, 0, 2 * step) as u16;
            }
            for _ in 0..count {
                for _ in 0..count {
                    pos = self.diamond(step, pos, &mut s, gnarl);
                }
                pos = cell(pos, 0, 2 * step) as u16;
            }
        }
    }

    /// Midpoint value: noise plus a quarter of the 4-point sum. The sum
    /// arrives ALREADY WRAPPED to i16 — the original accumulates it in
    /// 16-bit registers (remc2's `int16_t sumEnt`, remc1's `__int16`
    /// v4..v7 chains), and that overflow is load-bearing: on deep-water
    /// worlds (raise = -10000) four corners near -10000 wrap the sum
    /// positive, which is the only thing that breaks the fractal out of
    /// the all-negative plateau. Everything else computes in 32 bits and
    /// truncates to i16 on store, exactly like the 16-bit register math.
    #[inline]
    fn amp(sum: i16, step: i32, s: &mut u16, gnarl: u16) -> i16 {
        let r = lcg(s);
        let v = (r % gnarl.wrapping_mul(2).wrapping_add(1)) as i32
            + (r % (((step << 6) + 1) as u16)) as i32
            + ((sum as i32) >> 2)
            - 32 * step
            - gnarl as i32;
        v as i16
    }

    /// Square step (`sub_72652`): center of each square gets the corner
    /// average plus noise. Returns the next square's position.
    fn square(&mut self, step: i32, pos: u16, s: &mut u16, gnarl: u16) -> u16 {
        let sum = self.field[cell(pos, 0, 0)]
            .wrapping_add(self.field[cell(pos, 2 * step, 0)])
            .wrapping_add(self.field[cell(pos, 2 * step, 2 * step)])
            .wrapping_add(self.field[cell(pos, 0, 2 * step)]);
        let center = cell(pos, step, step);
        let v = Self::amp(sum, step, s, gnarl);
        if self.field[center] == 0 {
            self.field[center] = v;
        }
        cell(pos, 2 * step, 0) as u16
    }

    /// Diamond step (`sub_726E7`): the top and left edge midpoints of
    /// each square get their diamond average plus noise (one PRNG draw
    /// each). Returns the next square's position.
    fn diamond(&mut self, step: i32, pos: u16, s: &mut u16, gnarl: u16) -> u16 {
        let own = self.field[cell(pos, 0, 0)];
        let opposite = self.field[cell(pos, step, step)];

        let sum_top = own
            .wrapping_add(self.field[cell(pos, step, -step)])
            .wrapping_add(self.field[cell(pos, 2 * step, 0)])
            .wrapping_add(opposite);
        let v = Self::amp(sum_top, step, s, gnarl);
        let top = cell(pos, step, 0);
        if self.field[top] == 0 {
            self.field[top] = v;
        }

        let sum_left = own
            .wrapping_add(opposite)
            .wrapping_add(self.field[cell(pos, -step, step)])
            .wrapping_add(self.field[cell(pos, 0, 2 * step)]);
        let v = Self::amp(sum_left, step, s, gnarl);
        let left = cell(pos, 0, step);
        if self.field[left] == 0 {
            self.field[left] = v;
        }
        cell(pos, 2 * step, 0) as u16
    }

    /// Normalize the fractal field into 0..=196 heights and clear it
    /// (`sub_32A50`). The engine also tracks the minimum but never uses
    /// it. The quirky `& 0x8000` test and i16 view of the scaled value
    /// are reproduced bit-for-bit (i32 multiply wraps like the original's
    /// 32-bit `imul`).
    fn normalize(&mut self) {
        let mut max = -32000i16;
        for &v in &self.field {
            if v > max {
                max = v;
            }
        }
        let scale: i32 = if max != 0 { 12845056 / max as i32 } else { 0 };
        for i in 0..GRID {
            let mut r = scale.wrapping_mul(self.field[i] as i32) >> 16;
            self.field[i] = 0;
            if r & 0x8000 != 0 {
                r = 0;
            }
            if (r as i16) > 196 {
                r = 196;
            }
            self.height[i] = r as u8;
        }
    }

    /// River pass (`sub_32AE0`): initialize the class map (nonzero
    /// height -> class 5, else water), then carve `river` channels from
    /// random source cells above altitude `sourc`. Each source gets at
    /// most 999 placement probes; running dry abandons the remaining
    /// rivers. Ends by filling the type plane with 0xFF (scratch state
    /// the later passes snapshot into).
    fn rivers(&mut self, river: i32, sourc: u8) {
        for i in 0..GRID {
            self.class[i] = if self.height[i] != 0 { 5 } else { 0 };
        }
        let mut remaining = river;
        'rivers: while remaining > 0 {
            let mut tries = 1000i32;
            loop {
                let r = lcg(&mut self.rand);
                let idx = (r % 0xFFFF) as usize;
                let h = self.height[idx];
                tries -= 1;
                if tries == 0 {
                    break 'rivers;
                }
                if h > sourc && self.class[idx] != 0 {
                    remaining -= 1;
                    self.carve(idx as u16);
                    continue 'rivers;
                }
            }
        }
        self.types.fill(0xFF);
    }

    /// Carve one river (`sub_32B90`): walk to the lowest unvisited
    /// neighbor (first minimum in N8 scan order), clamping heights to be
    /// monotonically non-increasing along the path, until reaching
    /// existing water, height 0, or a dead end. Path cells become water
    /// class but KEEP their heights — MC1 rivers flow downhill at
    /// elevation. The type plane is the "visited" scratch mask.
    fn carve(&mut self, start: u16) {
        self.types.fill(3);
        let mut cur = start;
        let mut level_h = self.height[start as usize];
        loop {
            self.types[cur as usize] = 0;
            let mut best_h = 0xFFu8;
            let mut best = cur;
            for &(dx, dy) in &N8 {
                let n = cell(cur, dx, dy);
                if self.types[n] != 0 && best_h > self.height[n] {
                    best_h = self.height[n];
                    best = n as u16;
                }
            }
            if best_h == 0xFF || self.class[best as usize] == 0 {
                break;
            }
            if best_h > level_h {
                self.height[best as usize] = level_h;
            }
            level_h = self.height[best as usize];
            cur = best;
            if level_h == 0 {
                break;
            }
        }
        for i in 0..GRID {
            if self.types[i] == 0 {
                self.class[i] = 0;
            }
        }
    }

    /// Flatten water to a fixpoint (`sub_33500`): any quad whose four
    /// cells are all water but whose heights differ collapses to the
    /// quad minimum; repeat until stable.
    fn flatten_water(&mut self) {
        loop {
            let mut changed = false;
            for i in 0..=0xFFFFu16 {
                let mut water = 0;
                let mut min = self.height[i as usize];
                let mut max = min;
                for &(dx, dy) in &QUAD {
                    let n = cell(i, dx, dy);
                    if self.class[n] == 0 {
                        water += 1;
                    }
                    min = min.min(self.height[n]);
                    max = max.max(self.height[n]);
                }
                if max != min && water == 4 {
                    for &(dx, dy) in &QUAD {
                        self.height[cell(i, dx, dy)] = min;
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Vegetation pass (`sub_320A0`, param snflt): default land (class
    /// 5) flatter than the cut becomes vegetation (3); exactly at the
    /// cut, dirt (4). Then any quad mixing 3 and 5 without sand-2 turns
    /// its 3s to dirt (grows a dirt seam along the vegetation edge).
    fn vegetation(&mut self, snflt: u16) {
        for i in 0..=0xFFFFu16 {
            if self.class[i as usize] != 5 {
                continue;
            }
            let mut min = self.height[i as usize];
            let mut max = min;
            for &(dx, dy) in &PLUS4 {
                let h = self.height[cell(i, dx, dy)];
                min = min.min(h);
                max = max.max(h);
            }
            let d = max as i32 - min as i32;
            if d <= snflt as i32 {
                self.class[i as usize] = if d == snflt as i32 { 4 } else { 3 };
            }
        }
        for i in 0..=0xFFFFu16 {
            let (mut c3, mut c2, mut c5) = (0, 0, 0);
            for &(dx, dy) in &QUAD {
                match self.class[cell(i, dx, dy)] {
                    3 => c3 += 1,
                    2 => c2 += 1,
                    5 => c5 += 1,
                    _ => {}
                }
            }
            if c2 == 0 && c3 > 0 && c5 > 0 {
                for &(dx, dy) in &QUAD {
                    let n = cell(i, dx, dy);
                    if self.class[n] == 3 {
                        self.class[n] = 4;
                    }
                }
            }
        }
    }

    /// Low flat land reverts to the default class 5 (`sub_32D00`,
    /// params bhlin/bhflt): 3x3 neighborhood entirely below the beach
    /// line and flatter than the beach cut.
    fn low_flat(&mut self, bhlin: u8, bhflt: u8) {
        // The engine snapshots the class map into the type plane here
        // (and in the two passes below); nothing reads it before the
        // texture pass rewrites it, but keep the state identical.
        self.types.copy_from_slice(&self.class);
        for i in 0..=0xFFFFu16 {
            let mut min = self.height[i as usize];
            let mut max = min;
            for &(dx, dy) in &N8 {
                let h = self.height[cell(i, dx, dy)];
                min = min.min(h);
                max = max.max(h);
            }
            if (max as i32) < bhlin as i32
                && max as i32 - min as i32 <= bhflt as i32
                && self.class[i as usize] != 0
            {
                self.class[i as usize] = 5;
            }
        }
    }

    /// Boundary dirt (`sub_32300`): wherever a quad mixes vegetation
    /// with default land, vegetation with water, or water with land,
    /// the interface cells turn to dirt (4). The third rule flattens
    /// every non-water cell of the quad.
    fn boundary_dirt(&mut self) {
        for i in 0..=0xFFFFu16 {
            let (mut cw, mut c3, mut c5) = (0, 0, 0);
            for &(dx, dy) in &QUAD {
                match self.class[cell(i, dx, dy)] {
                    0 => cw += 1,
                    3 => c3 += 1,
                    5 => c5 += 1,
                    _ => {}
                }
            }
            if c3 > 0 && c5 > 0 {
                for &(dx, dy) in &QUAD {
                    let n = cell(i, dx, dy);
                    if self.class[n] == 5 {
                        self.class[n] = 4;
                    }
                }
            }
            if c3 > 0 && cw > 0 {
                for &(dx, dy) in &QUAD {
                    let n = cell(i, dx, dy);
                    if self.class[n] == 3 {
                        self.class[n] = 4;
                    }
                }
            }
            if cw > 0 && c5 > 0 {
                for &(dx, dy) in &QUAD {
                    let n = cell(i, dx, dy);
                    if self.class[n] != 0 {
                        self.class[n] = 4;
                    }
                }
            }
        }
    }

    /// Beach interior (`sub_32EB0`, params bhlin/bhflt): a class-5 cell
    /// below the beach line, flat, with all 8 neighbors in {5, 2},
    /// becomes sand (2) — beaches grow inward from their rim.
    fn beach_interior(&mut self, bhlin: u8, bhflt: u8) {
        self.types.copy_from_slice(&self.class);
        for i in 0..=0xFFFFu16 {
            let mut min = self.height[i as usize];
            let mut max = min;
            let (mut c5, mut c2) = (0, 0);
            for &(dx, dy) in &N8 {
                let n = cell(i, dx, dy);
                let h = self.height[n];
                min = min.min(h);
                max = max.max(h);
                match self.class[n] {
                    5 => c5 += 1,
                    2 => c2 += 1,
                    _ => {}
                }
            }
            if (max as i32) < bhlin as i32
                && max as i32 - min as i32 <= bhflt as i32
                && self.class[i as usize] == 5
                && c5 + c2 == 8
            {
                self.class[i as usize] = 2;
            }
        }
    }

    /// Rock pass (`sub_33180`, param rkste, UNSCALED): land steeper
    /// than the cut over the 4-neighborhood becomes brown rock (6);
    /// rock adjacent to mixed terrain (vegetation plus anything, sand,
    /// or default-land-plus-dirt) becomes dark basalt (1).
    fn rock(&mut self, rkste: u8) {
        self.types.copy_from_slice(&self.class);
        for i in 0..=0xFFFFu16 {
            let mut min = self.height[i as usize];
            let mut max = min;
            for &(dx, dy) in &PLUS4 {
                let h = self.height[cell(i, dx, dy)];
                min = min.min(h);
                max = max.max(h);
            }
            if self.class[i as usize] != 0 && max as i32 - min as i32 >= rkste as i32 {
                self.class[i as usize] = 6;
            }
        }
        for i in 0..=0xFFFFu16 {
            if self.class[i as usize] != 6 {
                continue;
            }
            let (mut c3, mut c2, mut c5, mut c4) = (0, 0, 0, 0);
            for &(dx, dy) in &N8 {
                match self.class[cell(i, dx, dy)] {
                    3 => c3 += 1,
                    2 => c2 += 1,
                    5 => c5 += 1,
                    4 => c4 += 1,
                    _ => {}
                }
            }
            let basalt = if c3 > 0 {
                c2 > 0 || c5 > 0 || c4 > 0
            } else {
                c2 > 0 || (c5 > 0 && c4 > 0)
            };
            if basalt {
                self.class[i as usize] = 1;
            }
        }
    }

    /// Majority smoothing (`sub_31FA0`): a cell whose 8 neighbors all
    /// share one non-water class adopts it (fills single-cell holes).
    fn majority(&mut self) {
        for i in 0..=0xFFFFu16 {
            let r = self.class[cell(i, 0, -1)];
            let mut eq = 0;
            for &(dx, dy) in &N8[1..] {
                if self.class[cell(i, dx, dy)] == r {
                    eq += 1;
                }
            }
            if r != 0 && eq == 7 {
                self.class[i as usize] = r;
            }
        }
    }

    /// Land height smoothing (`sub_31BB0`): pull land cells toward
    /// their 8-neighbor average, fully when the local relief is large,
    /// halfway when moderate, not at all when small.
    fn smooth_land(&mut self) {
        for i in 0..=0xFFFFu16 {
            if self.class[i as usize] & 7 == 0 {
                continue;
            }
            let own = self.height[i as usize];
            let (mut min, mut max) = (own, own);
            let mut sum = 0u32;
            for &(dx, dy) in &N8 {
                let h = self.height[cell(i, dx, dy)];
                sum += h as u32;
                min = min.min(h);
                max = max.max(h);
            }
            let avg = (sum >> 3) as u8;
            let halfway = ((avg as u16 + own as u16) >> 1) as u8;
            let d_down = own - min;
            if d_down > 4 {
                self.height[i as usize] = if d_down <= 10 { halfway } else { avg };
            } else {
                let d_up = max - own;
                if d_up <= 4 {
                    continue;
                }
                self.height[i as usize] = if d_up <= 10 { halfway } else { avg };
            }
        }
    }

    /// Shore zeroing (`sub_31EC0`): a quad that touches dirt (4) and
    /// sea-level water gets its heights zeroed — beaches meet the sea
    /// flat. Only the quad's E/SE/S cells are classified; the minimum
    /// starts from the cell's own height.
    fn shore_zero(&mut self) {
        for i in 0..=0xFFFFu16 {
            let mut c4 = false;
            let mut cw = false;
            let mut min_w = self.height[i as usize];
            for &(dx, dy) in &QUAD[1..] {
                let n = cell(i, dx, dy);
                let c = self.class[n];
                if c == 4 {
                    c4 = true;
                } else if c == 0 {
                    cw = true;
                    min_w = min_w.min(self.height[n]);
                }
            }
            if c4 && cw && min_w == 0 {
                for &(dx, dy) in &QUAD {
                    self.height[cell(i, dx, dy)] = 0;
                }
            }
        }
    }

    /// Texture selection (`sub_32560`): every tile's texture is chosen
    /// by matching its quad's corner classes (mgc_sim::mc1_tables)
    /// in all 8 dihedral arrangements. Candidate buckets are keyed by
    /// base-7 corner code; the PRNG picks among up to 12 candidates
    /// (quirk: `rand % (n+1)` maps the overflow back to candidate 0,
    /// doubling its weight). No match falls back to texture 1. The
    /// winning arrangement's orientation code lands in angle bits 4-6.
    ///
    /// (The engine also builds a flat 2401-entry first-candidate table
    /// here, `byte_B5D40`, used for in-game repaints after terrain
    /// deformation — no effect on generation; the feature pass gets it
    /// from `mgc_sim::mc1_tables::retile_table`.)
    fn textures(&mut self) {
        let buckets = mgc_sim::mc1_tables::corner_buckets();
        for i in 0..=0xFFFFu16 {
            if self.types[i as usize] != 0 {
                continue;
            }
            // Quad corner classes NW, NE, SE, SW; already-assigned
            // neighbors carry orientation bits, hence the & 7.
            let key = 343 * (self.class[cell(i, 0, 0)] & 7) as usize
                + 49 * (self.class[cell(i, 1, 0)] & 7) as usize
                + 7 * (self.class[cell(i, 1, 1)] & 7) as usize
                + (self.class[cell(i, 0, 1)] & 7) as usize;
            let bkt = &buckets[key];
            if bkt.count != 0 {
                let r = lcg(&mut self.rand);
                let mut k = (r % (bkt.count as u16 + 1)) as usize;
                if k >= bkt.count as usize {
                    k = 0;
                }
                self.types[i as usize] = bkt.tex[k];
                self.class[i as usize] = bkt.orient[k] + (self.class[i as usize] & 7);
            } else {
                self.types[i as usize] = 1;
            }
        }
    }

    /// Deep-water flag (`sub_31D40`): sea-level water cells fully
    /// surrounded by sea-level water, whose NW-side quad textures are
    /// all the pure water texture 0, get angle bit 3.
    fn deep_water(&mut self) {
        for i in 0..=0xFFFFu16 {
            // No-op on fresh generation (the bit is never set before
            // this pass); the engine clears it for in-game regeneration.
            self.class[i as usize] &= !8;
            if self.height[i as usize] != 0 {
                continue;
            }
            if N8.iter().any(|&(dx, dy)| self.height[cell(i, dx, dy)] != 0) {
                continue;
            }
            let typed = [(0, 0), (-1, 0), (-1, -1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| self.types[cell(i, dx, dy)] != 0);
            if !typed {
                self.class[i as usize] |= 8;
            }
        }
    }

    /// Shading (`sub_329C0`): shade = NW-to-SE height gradient + 32,
    /// computed in wrapping byte arithmetic and viewed signed. Flat
    /// cells get a PRNG dither in 28..=36 (the sparkling ocean); slopes
    /// clamp into 28..=31 dark / 40..=47 bright. The global PRNG is
    /// reset to 0 first.
    fn shading_pass(&mut self) {
        self.rand = 0;
        for i in 0..=0xFFFFu16 {
            let hi = self.height[cell(i, -1, -1)];
            let lo = self.height[cell(i, 1, 1)];
            let b = hi.wrapping_sub(lo).wrapping_add(32);
            self.shading[i as usize] = if b == 32 {
                (lcg(&mut self.rand) % 9) as u8 + 28
            } else if (b as i8) >= 28 {
                if (b as i8) > 40 { (b & 7) + 40 } else { b }
            } else {
                (b & 3) + 28
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mgc_sim::mc1_tables::CORNER_CLASSES;

    fn sample_params() -> GenMap {
        // Campaign level 001's real GEN_MAP: an 82%-water archipelago
        // with rivers — exercises every pass. (Synthetic combinations
        // can legitimately collapse to all-land plateaus: a negative
        // raise with unlucky top-level draws leaves the whole fractal
        // field negative, and the original's normalize then clamps the
        // least-negative cell to 196.)
        GenMap {
            pre_header: 110315,
            seed: 27324,
            off: 0,
            raise: -1010,
            gnarl: 21,
            river: 18,
            sourc: 47,
            snlin: 200,
            snflt: 50,
            bhlin: 68,
            bhflt: 36,
            rkste: 19,
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate(&sample_params());
        let b = generate(&sample_params());
        assert_eq!(a.tile_type, b.tile_type);
        assert_eq!(a.height, b.height);
        assert_eq!(a.shading, b.shading);
        assert_eq!(a.angle, b.angle);
    }

    #[test]
    fn output_invariants() {
        let t = generate(&sample_params());
        assert!(t.height.iter().all(|&h| h <= 196), "heights clamp to 196");
        assert!(
            t.tile_type.iter().all(|&ty| ty < 148),
            "generator only picks corner-table textures"
        );
        // Angle plane: class 0-6 in bits 0-2, deep-water bit 3,
        // orientation in bits 4-6; bit 7 never set.
        assert!(t.angle.iter().all(|&a| a & 0x80 == 0 && a & 7 != 7));
        assert!(
            t.shading.iter().all(|&s| (28..=47).contains(&s)),
            "shading stays in the engine's 28..=47 band"
        );
        let water = t.height.iter().filter(|&&h| h == 0).count() as f64 / GRID as f64;
        assert!(
            (0.70..0.90).contains(&water),
            "level 001 water fraction {water:.2} outside the known ~0.82"
        );
        // Water cells carry the pure water texture or a transition; the
        // deep-water flag only appears on water.
        for i in 0..GRID {
            if t.angle[i] & 8 != 0 {
                assert_eq!(t.height[i], 0, "deep-water flag on dry land");
                assert_eq!(t.tile_type[i], 0, "deep-water flag on non-water texture");
            }
        }
    }

    #[test]
    fn corner_table_shape() {
        // Pure classes and the building-slot gap, straight from the
        // decompilation's unk_9075C.
        for (i, entry) in CORNER_CLASSES.iter().enumerate().take(7) {
            assert_eq!(*entry, [i as u8; 4]);
        }
        for entry in &CORNER_CLASSES[7..35] {
            assert_eq!(*entry, [0xFF; 4]);
        }
        assert_eq!(CORNER_CLASSES[35], [6, 0, 1, 4]);
        // Every non-building entry uses only real classes.
        for e in CORNER_CLASSES.iter().filter(|e| !e.contains(&0xFF)) {
            assert!(e.iter().all(|&c| c <= 6));
        }
    }
}
