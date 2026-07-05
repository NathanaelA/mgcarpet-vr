//! MC1 load-time terrain features ("GenerateFeatures").
//!
//! Port of remc1's `GenerateFeatures_36430_367F0` (sub_main.cpp:43043):
//! the entity-driven post-generation phase that carves craters and
//! canyons, raises walls and ridges, paints tracks, and flattens/paints
//! building footprints into the pristine generated terrain. Baked
//! `.mgcl` terrain stays pristine by design (docs/FORMAT.md); the
//! engine applies these modifications at level load from `things.json`.
//!
//! Machinery (line references are remc1 sub_main.cpp):
//!
//! - Level entities with `class == 10 && dis_id == 0xFFFF` are terrain
//!   features, consumed in slot order 1..1999. Chained models (28
//!   walls, 29 tracks, 31 canyons, 50 ridges, with `swi_id != 0` as
//!   the not-yet-processed flag) run a polyline walker (sub_362C0,
//!   :42972): root-first via `parent` links, then one segment function
//!   per parent→child pair. Everything else spawns a runtime *event*
//!   through its per-model creator (`off_97D12`, :5075); model 45
//!   (building) additionally gets the footprint fix-up sub_36DF0.
//! - The event loop (sub_36620, :43181) then sweeps the 1000-slot
//!   event pool to fixpoint: craters dig ring by ring, canyon heads
//!   walk and spawn diggers, buildings flatten and paint over 30
//!   ticks, and every non-feature event is purged. Dispatch is by the
//!   entity's byte-70 tick index, not its model.
//! - Determinism: the pool allocates slots 1,2,3,… (free stack built
//!   999→1; frees push back LIFO), and each event seeds a per-entity
//!   LCG from `slot + global_rand`. Two behaviors depend on the slot
//!   number itself: digger radius growth (`slot % 3`, sub_25670) and
//!   dither draws — so slot churn from events that are spawned only to
//!   be purged is load-bearing and reproduced exactly.
//! - PRNG streams (all `x = 9377x + 9439`): the global u32 `rand_4` is
//!   the level seed at scan time and is advanced exactly once at event
//!   loop entry; retiling draws the u16 `pseudoRand` stream whose
//!   post-generation state is replayed from the height plane
//!   ([`post_generation_pseudo_rand`], the generator's shading pass
//!   reset it to 0 and drew once per flat tile).
//!
//! Deliberately omitted (terrain-neutral at load): damage broadcasts
//! (sub_127E0/sub_120B0 — they write damage fields on pool entities;
//! relevant once entities persist), sounds, and the surviving building
//! entities themselves (the entity track will need them; the terrain
//! effect is complete without).
//!
//! Entity-table indices: `things.json` slots are 0-based file order;
//! the engine indexes the same records 1-based (its record 1 = file
//! offset 0x442 = our slot 0), and `parent`/`child` values are those
//! 1-based indices. The pass rebuilds the 1-based table.

use crate::mc1_tables;
use crate::tables::{ATAN, BIT_SQRT, COS, PAINT_AC, PAINT_BC, PAINT_EC, PAINT_FC, SIN};
use mgc_formats::Thing;

/// Cells in the 256x256 terrain grid.
const GRID: usize = 0x10000;

/// Engine entity-table capacity; the feature scan visits 1..=1999.
const TABLE_SLOTS: usize = 2096;

/// Runtime event pool size (slot 0 never allocated).
const POOL: usize = 1000;

/// The four terrain planes the feature pass mutates, engine layout
/// (index = tile_y * 256 + tile_x).
pub struct TerrainPlanes<'a> {
    pub height: &'a mut [u8],
    pub tile_type: &'a mut [u8],
    pub shading: &'a mut [u8],
    pub angle: &'a mut [u8],
}

/// One building-footprint entry from `BUILD?-0.TAB` (6 bytes on disk:
/// u32 offset into the DAT blob, u8 width, u8 height in tiles).
#[derive(Clone, Copy)]
pub struct BuildDef {
    pub offset: u32,
    pub w: u8,
    pub h: u8,
}

/// Parsed game data the feature pass needs: the SEARCH.DAT ring table
/// and the building footprint RLE maps.
pub struct FeatureAssets {
    /// Per ring 0..31: (dx, dy) byte deltas from the dig center, in the
    /// original's row-major emission order (sub_11540, :16784).
    pub rings: Vec<Vec<(u8, u8)>>,
    pub build_tab: Vec<BuildDef>,
    pub build_dat: Vec<u8>,
}

impl FeatureAssets {
    /// `search` = decompressed SEARCH.DAT (1024 bytes, 32x32 ring-index
    /// grid); `build_tab`/`build_dat` = decompressed BUILD?-0.TAB/DAT.
    pub fn parse(search: &[u8], build_tab: &[u8], build_dat: &[u8]) -> Result<Self, String> {
        if search.len() != 1024 {
            return Err(format!("search grid: expected 1024 bytes, got {}", search.len()));
        }
        // Center = the first value-0 cell in row-major scan; ring j's
        // entries are all value-j cells in the same scan order.
        let c = search
            .iter()
            .position(|&v| v == 0)
            .ok_or("search grid has no ring-0 cell")?;
        let (cx, cy) = ((c % 32) as u8, (c / 32) as u8);
        let mut rings = vec![Vec::new(); 32];
        for (j, ring) in rings.iter_mut().enumerate() {
            for y in 0..32u8 {
                for x in 0..32u8 {
                    if search[y as usize * 32 + x as usize] == j as u8 {
                        ring.push((x.wrapping_sub(cx), y.wrapping_sub(cy)));
                    }
                }
            }
        }
        if build_tab.len() % 6 != 0 {
            return Err(format!("build tab: {} bytes is not 6-byte entries", build_tab.len()));
        }
        let tab: Vec<BuildDef> = build_tab
            .chunks_exact(6)
            .map(|e| BuildDef {
                offset: u32::from_le_bytes(e[0..4].try_into().unwrap()),
                w: e[4],
                h: e[5],
            })
            .collect();
        for (i, b) in tab.iter().enumerate() {
            if (b.offset as usize) >= build_dat.len() && (b.w != 0 || b.h != 0) {
                return Err(format!("build tab entry {i} offset {} past dat", b.offset));
            }
        }
        Ok(Self {
            rings,
            build_tab: tab,
            build_dat: build_dat.to_vec(),
        })
    }
}

/// The engine's LCG, 32-bit state (`rand_4` and per-entity streams).
#[inline]
fn lcg32(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(9377).wrapping_add(9439);
    *s
}

/// Tile index from u8 coordinates (low byte = x, high byte = y).
#[inline]
fn tile(x: u8, y: u8) -> usize {
    ((y as usize) << 8) | x as usize
}

#[inline]
fn tx(t: usize) -> u8 {
    t as u8
}
#[inline]
fn ty(t: usize) -> u8 {
    (t >> 8) as u8
}
/// Move a packed tile index by wrapping each byte axis independently.
#[inline]
fn step(t: usize, dx: i32, dy: i32) -> usize {
    tile(tx(t).wrapping_add(dx as u8), ty(t).wrapping_add(dy as u8))
}

/// Replay the generator's final shading pass on the pristine height
/// plane to recover the u16 `pseudoRand` state at GenerateFeatures
/// time (the pass reset the stream to 0, then drew once per flat cell
/// — `sub_329C0`, mirrored by mc1_terrain's `shading_pass`).
pub fn post_generation_pseudo_rand(height: &[u8]) -> u16 {
    let mut s = 0u16;
    for i in 0..=0xFFFFu16 {
        let hi = height[step(i as usize, -1, -1)];
        let lo = height[step(i as usize, 1, 1)];
        if hi.wrapping_sub(lo).wrapping_add(32) == 32 {
            s = s.wrapping_mul(9377).wrapping_add(9439);
        }
    }
    s
}

/// One record of the original 18-byte THING_INIT table (1-based copy).
#[derive(Clone, Copy, Default)]
struct Rec {
    class: u16,
    model: u16,
    x: u16,
    y: u16,
    dis_id: u16,
    swi_id: u16,
    parent: u16,
    child: u16,
}

/// Runtime event entity — the subset of remc1's 164-byte
/// `Type_AE400_29795` the load-time feature path uses. Names keep the
/// original byte offsets for traceability.
#[derive(Clone, Copy, Default)]
struct Ent {
    /// Per-entity LCG (offset 4), seeded `slot + global_rand` at alloc.
    rand: u32,
    max_life: u32,
    act_life: i32,
    /// Flags (offset 16). Bit 1 (0x2) = dug/second-phase, bit 2 (0x4) =
    /// linked into the tile map, bit 10 (0x400) = marked dead.
    flags: u32,
    next20: u16,
    prev22: u16,
    /// Generic counter (offset 26): crater ring counter, wall run length.
    f26: i16,
    f28: u16,
    /// Wall step dx/dy (offsets 30/32); canyon/ridge heading (30).
    f30: u16,
    f32: u16,
    /// Strength (offset 44).
    f44: u16,
    /// Slot index at alloc (offset 63) — gates digger radius growth.
    f63: u8,
    class64: u8,
    model65: u8,
    /// Tick-handler index (offset 70).
    tick70: u8,
    /// Building-table index (offset 71).
    f71: u8,
    /// Position, 8.8 fixed point (offsets 72/74/76).
    x: u16,
    y: u16,
    z: i16,
    /// Extents (offsets 80/82/84); high byte of f80 = dig radius in tiles.
    f80: u16,
    f82: u16,
    f84: u16,
    /// Advance per tick (offset 126); building area>>4 (offset 128).
    f126: i16,
    f128: i16,
}

struct Gen<'a> {
    t: TerrainPlanes<'a>,
    assets: &'a FeatureAssets,
    /// `byte_B5D40`: 2401 x {texture, orientation bits} retile table.
    retile: Vec<[u8; 2]>,
    /// Per-tile head of the event intrusive list (`mapEntityIndex`).
    map_entity: Vec<u16>,
    ent: Vec<Ent>,
    /// Free stack; built 999→1 so allocation pops 1, 2, 3, …
    free: Vec<u16>,
    /// Global LCG (`rand_4`), = the level seed at scan time.
    rand: u32,
    /// Terrain-retile LCG (`pseudoRand`), u16 stream.
    pseudo: u16,
}

/// Apply MC1's load-time terrain features.
///
/// `seed` is the level's GEN_MAP seed (`rand_4` is loaded from it and
/// nothing before GenerateFeatures advances it); pass 0 if unknown —
/// only dither variety is affected, not feature placement.
pub fn generate_features_mc1(
    planes: TerrainPlanes<'_>,
    things: &[Thing],
    seed: u32,
    assets: &FeatureAssets,
) {
    // Rebuild the original 1-based record table.
    let mut table = vec![Rec::default(); TABLE_SLOTS];
    for th in things {
        let i = th.slot as usize + 1;
        if i < TABLE_SLOTS {
            table[i] = Rec {
                class: th.class,
                model: th.model,
                x: th.x,
                y: th.y,
                dis_id: th.dis_id,
                swi_id: th.swi_id,
                parent: th.parent,
                child: th.child,
            };
        }
    }

    let pseudo = post_generation_pseudo_rand(planes.height);
    let mut g = Gen {
        t: planes,
        assets,
        retile: mc1_tables::retile_table(),
        map_entity: vec![0; GRID],
        ent: vec![Ent::default(); POOL],
        free: (1..POOL as u16).rev().collect(),
        rand: seed,
        pseudo,
    };

    // GenerateFeatures_36430: the spawn scan (slots 1..1999).
    for i in 1..2000usize {
        if table[i].dis_id == 0xFFFF && table[i].class == 10 {
            g.dispatch(&mut table, i);
            table[i].class = 0;
        }
    }
    g.event_loop();
}

impl<'a> Gen<'a> {
    // ---- pool primitives ------------------------------------------------

    /// NewEvent_372C0 (:43865). Seeds the per-entity LCG from the
    /// global stream WITHOUT advancing it.
    fn new_event(&mut self) -> Option<usize> {
        let idx = self.free.pop()? as usize;
        let e = &mut self.ent[idx];
        *e = Ent::default();
        e.max_life = 300;
        e.flags = 8;
        e.f126 = 16;
        e.f44 = 100;
        e.rand = (idx as u32).wrapping_add(self.rand);
        e.f63 = idx as u8;
        Some(idx)
    }

    /// sub_41CF0 (:52468): link into the per-tile list and set position.
    fn link(&mut self, i: usize, x: u16, y: u16, z: i16) {
        if self.ent[i].flags & 4 != 0 {
            return;
        }
        let t = tile((x >> 8) as u8, (y >> 8) as u8);
        self.ent[i].prev22 = 0;
        self.ent[i].next20 = self.map_entity[t];
        let head = self.map_entity[t] as usize;
        if head != 0 {
            self.ent[head].prev22 = i as u16;
        }
        self.map_entity[t] = i as u16;
        let e = &mut self.ent[i];
        e.x = x;
        e.y = y;
        e.z = z;
        e.flags |= 4;
    }

    /// sub_41DD0 (:52486).
    fn unlink(&mut self, i: usize) {
        if self.ent[i].flags & 4 == 0 {
            return;
        }
        let (next, prev) = (self.ent[i].next20, self.ent[i].prev22);
        if prev != 0 {
            self.ent[prev as usize].next20 = next;
        } else {
            let t = tile((self.ent[i].x >> 8) as u8, (self.ent[i].y >> 8) as u8);
            self.map_entity[t] = next;
        }
        if next != 0 {
            self.ent[next as usize].prev22 = prev;
        }
        self.ent[i].flags &= !4;
    }

    /// sub_41C70 (:52442): move, relinking only across tiles.
    fn move_relink(&mut self, i: usize, x: u16, y: u16, z: i16) {
        let e = &self.ent[i];
        if e.x >> 8 == x >> 8 && e.y >> 8 == y >> 8 {
            let e = &mut self.ent[i];
            e.x = x;
            e.y = y;
            e.z = z;
        } else {
            self.unlink(i);
            self.link(i, x, y, z);
        }
    }

    /// sub_41E90 (:52514): unlink, clear, return the slot (LIFO).
    fn free_entity(&mut self, i: usize) {
        self.unlink(i);
        self.ent[i].class64 = 0;
        self.free.push(i as u16);
    }

    // ---- terrain helpers ------------------------------------------------

    /// sub_724C0 (:81516): ground height at an 8.8 position,
    /// interpolated across the tile's two triangles, in engine units
    /// (one height byte = 32).
    fn ground_z(&self, x: u16, y: u16) -> i32 {
        let h = |dx: u8, dy: u8| self.t.height[tile(dx, dy)] as i32;
        let (cx, cy) = ((x >> 8) as u8, (y >> 8) as u8);
        let (fx, fy) = ((x & 0xFF) as i32, (y & 0xFF) as i32);
        let (p1, comp);
        if cx.wrapping_add(cy) & 1 == 1 {
            if fx + fy > 255 {
                p1 = h(cx, cy.wrapping_add(1));
                let p2 = h(cx.wrapping_add(1), cy.wrapping_add(1));
                comp = (255 - fy) * (h(cx.wrapping_add(1), cy) - p2) + fx * (p2 - p1);
            } else {
                p1 = h(cx, cy);
                let p2 = h(cx.wrapping_add(1), cy);
                comp = fy * (h(cx, cy.wrapping_add(1)) - p1) + fx * (p2 - p1);
            }
        } else if fx <= fy {
            p1 = h(cx, cy);
            let p2 = h(cx, cy.wrapping_add(1));
            comp = fy * (p2 - p1) + fx * (h(cx.wrapping_add(1), cy.wrapping_add(1)) - p2);
        } else {
            p1 = h(cx, cy);
            let p2 = h(cx.wrapping_add(1), cy);
            comp = fy * (h(cx.wrapping_add(1), cy.wrapping_add(1)) - p2) + fx * (p2 - p1);
        }
        (comp >> 3) + 32 * p1
    }

    /// sub_361C0 (:42956): average of the four footprint corners
    /// (x, y), (x+w, y), (x+w, y+h), (x, y+h), u8-wrapping.
    fn avg4(&self, x: u8, y: u8, h: u8, w: u8) -> u16 {
        let p1 = self.t.height[tile(x, y)] as u16;
        let p2 = self.t.height[tile(x.wrapping_add(w), y)] as u16;
        let p3 = self.t.height[tile(x.wrapping_add(w), y.wrapping_add(h))] as u16;
        let p4 = self.t.height[tile(x, y.wrapping_add(h))] as u16;
        (p1 + p2 + p3 + p4) >> 2
    }

    /// The shared passes 2+3 of the retexture helpers (sub_33B90 /
    /// sub_33E10, :41165/:41288): retile every type-1 cell of the rect
    /// grown by one on the -x/-y side through the `byte_B5D40` table
    /// (drawing pseudoRand for types < 8), then recompute shading over
    /// the rect grown once more.
    fn retile_and_shade(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let x_add = bx.wrapping_sub(ax).wrapping_add(2);
        let y_add = by.wrapping_sub(ay).wrapping_add(2);
        let (sx, sy) = (ax.wrapping_sub(1), ay.wrapping_sub(1));
        let mut cy = sy;
        for _ in 0..y_add {
            let mut cx = sx;
            for _ in 0..x_add {
                let t = tile(cx, cy);
                if self.t.tile_type[t] == 1 {
                    let p1 = self.t.angle[t] & 7;
                    let p2 = self.t.angle[tile(cx.wrapping_add(1), cy)] & 7;
                    let p3 = self.t.angle[tile(cx.wrapping_add(1), cy.wrapping_add(1))] & 7;
                    let p4 = self.t.angle[tile(cx, cy.wrapping_add(1))] & 7;
                    let idx =
                        p4 as usize + 7 * p3 as usize + 49 * p2 as usize + 343 * p1 as usize;
                    let [new_type, orient] = self.retile[idx];
                    self.t.tile_type[t] = new_type;
                    self.t.angle[t] = if new_type >= 8 {
                        orient.wrapping_add(self.t.angle[t] & 0x87)
                    } else {
                        self.pseudo = self.pseudo.wrapping_mul(9377).wrapping_add(9439);
                        (self.t.angle[t] & 0x87).wrapping_add(16 * (self.pseudo % 7) as u8)
                    };
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        // Pass 3: shading over the rect grown once more (3x3 for a
        // single cell). shade = NW height - SE height + 32, as signed
        // char; clamp <28 → (s&3)+28, >40 → (s&7)+40; clear angle bit 3.
        let mut cy = sy;
        for _ in 0..y_add.wrapping_add(1) {
            let mut cx = sx;
            for _ in 0..x_add.wrapping_add(1) {
                let t = tile(cx, cy);
                let se = self.t.height[tile(cx.wrapping_add(1), cy.wrapping_add(1))];
                let nw = self.t.height[tile(cx.wrapping_sub(1), cy.wrapping_sub(1))];
                let mut s = nw.wrapping_sub(se).wrapping_add(32);
                if (s as i8) < 28 {
                    s = (s & 3) + 28;
                } else if (s as i8) > 40 {
                    s = (s & 7) + 40;
                }
                self.t.shading[t] = s;
                self.t.angle[t] &= 0xF7;
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
    }

    /// sub_33B90 (:41165), "flag mode": stencil type 1 onto each rect
    /// cell + its W/NW/N neighbors where not building-protected (bit 7),
    /// then retile + shade.
    fn recompute_protected(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let (w, h) = (
            bx.wrapping_sub(ax).wrapping_add(1),
            by.wrapping_sub(ay).wrapping_add(1),
        );
        let mut cy = ay;
        for _ in 0..h {
            let mut cx = ax;
            for _ in 0..w {
                for t in [
                    tile(cx, cy),
                    tile(cx.wrapping_sub(1), cy),
                    tile(cx.wrapping_sub(1), cy.wrapping_sub(1)),
                    tile(cx, cy.wrapping_sub(1)),
                ] {
                    if self.t.angle[t] & 0x80 == 0 {
                        self.t.tile_type[t] = 1;
                    }
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        self.retile_and_shade(ax, ay, bx, by);
    }

    /// sub_33E10 (:41288), "dig mode": same but the stencil ignores the
    /// protection bit.
    fn recompute_unprotected(&mut self, ax: u8, ay: u8, bx: u8, by: u8) {
        let (w, h) = (
            bx.wrapping_sub(ax).wrapping_add(1),
            by.wrapping_sub(ay).wrapping_add(1),
        );
        let mut cy = ay;
        for _ in 0..h {
            let mut cx = ax;
            for _ in 0..w {
                for t in [
                    tile(cx, cy),
                    tile(cx.wrapping_sub(1), cy),
                    tile(cx.wrapping_sub(1), cy.wrapping_sub(1)),
                    tile(cx, cy.wrapping_sub(1)),
                ] {
                    self.t.tile_type[t] = 1;
                }
                cx = cx.wrapping_add(1);
            }
            cy = cy.wrapping_add(1);
        }
        self.retile_and_shade(ax, ay, bx, by);
    }

    /// sub_33AE0 (:41094), wall variant: write `ty` onto the cell and
    /// its W/NW/N neighbors unconditionally, then 3x3 shading with a
    /// hard floor of 32 (no retile, no PRNG).
    fn set_type_2x2(&mut self, t: usize, ty_val: u8) {
        let (cx, cy) = (tx(t), ty(t));
        self.t.tile_type[t] = ty_val;
        self.t.tile_type[tile(cx.wrapping_sub(1), cy)] = ty_val;
        self.t.tile_type[tile(cx.wrapping_sub(1), cy.wrapping_sub(1))] = ty_val;
        self.t.tile_type[tile(cx, cy.wrapping_sub(1))] = ty_val;
        let mut yy = cy.wrapping_sub(1);
        for _ in 0..3 {
            let mut xx = cx.wrapping_sub(1);
            for _ in 0..3 {
                let se = self.t.height[tile(xx.wrapping_add(1), yy.wrapping_add(1))];
                let nw = self.t.height[tile(xx.wrapping_sub(1), yy.wrapping_sub(1))];
                let mut s = nw.wrapping_sub(se).wrapping_add(32);
                if (s as i8) < 32 {
                    s = 32;
                } else if (s as i8) > 40 {
                    s = (s & 7) + 40;
                }
                let c = tile(xx, yy);
                self.t.shading[c] = s;
                self.t.angle[c] &= 0xF7;
                xx = xx.wrapping_add(1);
            }
            yy = yy.wrapping_add(1);
        }
    }

    /// sub_40A10 (:51621): adjust one cell's height by `delta` (clamped
    /// 0..200), update its slope nibble (1 = land; 0 = water when the
    /// floor is reached and no neighbor blocks conversion), then
    /// recompute the 1-cell neighborhood. `protect` mode aborts on
    /// building-protected cells and honors protection in the stencil.
    /// Returns true only via the literal `(0,0)` clamp latch (dead in
    /// practice; kept faithful).
    fn dig_cell(&mut self, ax: i16, ay: i16, delta: i16, protect: bool) -> bool {
        let t = tile(ax as u8, ay as u8);
        let mut saturated = false;
        let mut v = delta as i32 + self.t.height[t] as i32;
        if v > 200 {
            v = 200;
            if ax == 0 && ay == 0 {
                saturated = true;
            }
        }
        if v < 0 {
            v = 0;
            if ax == 0 && ay == 0 {
                saturated = true;
            }
        }
        if protect && self.t.angle[t] & 0x80 != 0 {
            return true;
        }
        self.t.height[t] = v as u8;
        if v != 0 {
            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
        } else {
            // Water conversion: all 8 neighbors must not carry slope
            // codes 2, 3 or 5 (sub_409E0), else leave the angle alone.
            let clear = [
                (-1, -1),
                (0, -1),
                (1, -1),
                (1, 0),
                (-1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ]
            .iter()
            .all(|&(dx, dy)| {
                let n = self.t.angle[step(t, dx, dy)] & 7;
                n != 5 && n != 2 && n != 3
            });
            if clear {
                self.t.angle[t] &= 0xF0;
            }
        }
        if protect {
            self.recompute_protected(tx(t), ty(t), tx(t), ty(t));
        } else {
            self.recompute_unprotected(tx(t), ty(t), tx(t), ty(t));
        }
        saturated
    }

    /// The ring iterator of sub_11410/sub_114B0 (:16697/:16732): yields
    /// every (dx, dy) of rings `lo..=hi` EXCEPT the last entry of ring
    /// `hi`, which the original fetches together with the stop code and
    /// drops — a faithful off-by-one.
    fn ring_cells(&self, lo: i32, hi: i32) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        if lo < 0 || lo > 31 {
            return out;
        }
        let hi_c = hi.min(31);
        let mut ring = lo;
        loop {
            let cells = &self.assets.rings[ring as usize];
            for (k, &d) in cells.iter().enumerate() {
                let last_of_ring = k + 1 == cells.len();
                if last_of_ring && ring >= hi_c {
                    return out; // fetched with stop code, dropped
                }
                out.push(d);
                if last_of_ring {
                    break;
                }
            }
            ring += 1;
            if ring > hi_c || ring > 31 {
                return out;
            }
        }
    }

    /// sub_40D30 (:51693): dig a disc of rings `lo..=hi` (clamped to
    /// the event's radius) around the event, height delta `delta`.
    fn dig_disc(&mut self, i: usize, lo: i32, hi: i32, delta: i16, protect: bool) -> bool {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as i32;
        let cy = ((e.y as u32 + 128) >> 8) as i32;
        let hi = hi.min((e.f80 >> 8) as i32);
        for (dx, dy) in self.ring_cells(lo, hi) {
            if self.dig_cell(
                (cx + dx as i32) as i16,
                (cy + dy as i32) as i16,
                delta,
                protect,
            ) && protect
            {
                return true;
            }
        }
        false
    }

    /// sub_255D0 (:28353): the -3 disc variant that never aborts.
    fn dig_disc_minus3(&mut self, i: usize, lo: i32, hi: i32) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as i32;
        let cy = ((e.y as u32 + 128) >> 8) as i32;
        let hi = hi.min((e.f80 >> 8) as i32);
        for (dx, dy) in self.ring_cells(lo, hi) {
            self.dig_cell((cx + dx as i32) as i16, (cy + dy as i32) as i16, -3, false);
        }
    }

    /// sub_11760 (:16869): true when the tile under the position (plain
    /// >>8, no rounding) is water (angle nibble 0) — the walker/digger
    /// stop probe.
    fn on_water(&self, x: u16, y: u16) -> bool {
        self.t.angle[tile((x >> 8) as u8, (y >> 8) as u8)] & 0xF == 0
    }

    // ---- math helpers ---------------------------------------------------

    /// sub_358D0 (:42470): shortest wrapped tile delta in -128..=128.
    fn wrap_delta(a: i16, b: i16) -> i32 {
        let d = b.wrapping_sub(a);
        if d > 128 {
            (d as i32) - 256
        } else if d < -128 {
            (d as i32) + 256
        } else {
            d as i32
        }
    }

    /// sub_40F87 (:51818): angle from delta in 1/2048 turns (0 = -y).
    fn angle_of(dx: i16, dy: i16) -> u16 {
        let lut = |n: i32, d: i32| ATAN[((n << 8) / d) as usize] as i32;
        let (a1, a2) = (dx as i32, dy as i32);
        let r = if a1 == 0 && a2 == 0 {
            0
        } else if a1 < 0 {
            if a2 < 0 {
                if -a1 < -a2 {
                    2048 - lut(-a1, -a2)
                } else {
                    1536 + lut(-a2, -a1)
                }
            } else if -a1 < a2 {
                1024 + lut(-a1, a2)
            } else {
                1536 - lut(a2, -a1)
            }
        } else if a2 < 0 {
            if a1 < -a2 {
                lut(a1, -a2)
            } else {
                512 - lut(-a2, a1)
            }
        } else if a1 < a2 {
            1024 - lut(a1, a2)
        } else {
            512 + lut(a2, a1)
        };
        r as u16
    }

    /// Distance_410CE (:51874): Newton integer sqrt with seed table.
    fn isqrt(square: u32) -> u32 {
        if square == 0 {
            return 0;
        }
        let bit = 31 - square.leading_zeros();
        let mut i = BIT_SQRT[bit as usize];
        while square / i < i {
            i = (square / i + i) >> 1;
        }
        i
    }

    /// sub_42150/sub_423D0 (:52638/:52739) on two 8.8 positions.
    fn angle_between(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
        Self::angle_of(
            (bx as i16).wrapping_sub(ax as i16),
            (by as i16).wrapping_sub(ay as i16),
        )
    }
    fn dist_between(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
        let dx = (bx as i16).wrapping_sub(ax as i16) as i32;
        let dy = (by as i16).wrapping_sub(ay as i16) as i32;
        Self::isqrt((dx * dx + dy * dy) as u32) as u16
    }

    /// sub_41EC0 (:52523), pitch-0 path: advance a position `speed`
    /// units along `angle` (16.16 trig, wrapping i16/u16 adds).
    fn advance(x: &mut u16, y: &mut u16, angle: u16, speed: i16) {
        if speed == 0 {
            return;
        }
        let a = (angle & 0x7FF) as usize;
        *x = x.wrapping_add(((speed as i32 * SIN[a]) >> 16) as u16);
        *y = y.wrapping_sub(((COS[a] * speed as i32) >> 16) as u16);
    }

    // ---- the spawn scan -------------------------------------------------

    /// sub_36480 (:43065): dispatch one feature entity.
    fn dispatch(&mut self, table: &mut [Rec], slot: usize) {
        let rec = table[slot];
        let model = rec.model;
        let chained = matches!(model, 28 | 29 | 31 | 50) && rec.swi_id != 0;
        if chained {
            self.walk_chain(table, slot);
            return;
        }
        let x = rec.x << 8;
        let y = rec.y << 8;
        let z = self.ground_z(x, y) as i16;
        if let Some(i) = self.spawn_creator(model, x, y, z) {
            if model == 45 {
                self.building_fixup(i, rec.parent.wrapping_add(16));
            }
        }
    }

    /// sub_362C0 (:42972): walk a feature chain root-first, clearing
    /// each node's pending flag and running the per-model segment
    /// function on every parent→child coordinate pair.
    fn walk_chain(&mut self, table: &mut [Rec], slot: usize) {
        let class = table[slot].class;
        let model = table[slot].model;
        let mut cur = slot;
        while table[cur].parent != 0 {
            cur = table[cur].parent as usize % TABLE_SLOTS;
        }
        loop {
            if table[cur].class != class || table[cur].model != model {
                return;
            }
            let child = table[cur].child as usize % TABLE_SLOTS;
            table[cur].swi_id = 0;
            if child == 0 {
                return;
            }
            let (x1, y1) = (table[cur].x, table[cur].y);
            let (x2, y2) = (table[child].x, table[child].y);
            match model {
                28 => self.segment_wall(x1 as i16, y1, x2 as i16, y2 as i16),
                29 => self.segment_track(x1 as i16, y1 as i16, x2 as i16, y2 as i16),
                31 => self.segment_canyon(x1, y1, x2, y2),
                50 => self.segment_ridge(x1, y1, x2, y2),
                _ => unreachable!(),
            }
            cur = child;
        }
    }

    /// Creators (`off_97D12`, :5075). Models absent from retail data or
    /// with null/stub creators spawn nothing. Non-ticking models spawn
    /// an event that the loop purges unticked — only its pool-slot
    /// churn is observable, so their creator bodies reduce to alloc +
    /// identity fields (positions kept for completeness).
    fn spawn_creator(&mut self, model: u16, x: u16, y: u16, z: i16) -> Option<usize> {
        // Null/stub creator entries: model 24 (stub returning 0),
        // 37, 46..49 (null). Everything else allocates one event.
        if matches!(model, 24 | 37 | 46..=49) || model > 61 {
            return None;
        }
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = model as u8;
        e.x = x;
        e.y = y;
        e.z = z;
        match model {
            // sub_3A8D0: growing hill / volcano.
            9 => {
                e.tick70 = 9;
                e.max_life = 17;
                e.act_life = 17;
                e.f44 = 2000;
                e.flags = 0;
                e.f80 = 768;
                e.f82 = 768;
                e.f84 = 0x2000;
            }
            // sub_3A930: one-shot shallow dish.
            10 => {
                e.tick70 = 10;
                e.max_life = 1;
                e.act_life = 1;
                e.f44 = 100;
                e.flags = 0x20000;
                e.f80 = 128;
                e.f82 = 128;
                e.f84 = 128;
            }
            // sub_3A9A0: expanding crater (also the canyon digger ctor).
            11 => {
                e.tick70 = 11;
                e.max_life = 40;
                e.act_life = 40;
                e.f44 = 200;
                e.flags = 0;
                e.f80 = 2304;
                e.f82 = 2304;
                e.f84 = 0x2000;
            }
            // sub_3B060/3B120/3B1D0/3B2A0: unchained wall/track/canyon/
            // ridge nodes; their events tick straight into the self-kill
            // handler (byte70 30/31/33/54 → sub_253E0).
            28 => {
                e.tick70 = 30;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            29 => {
                e.tick70 = 31;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            30 => {
                e.tick70 = 32;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            31 => {
                e.tick70 = 33;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            50 => {
                e.tick70 = 54;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            // sub_3B180: canyon head (only reached via segment spawns
            // in practice; unchained model-32 level entities are absent
            // from retail data).
            32 => {
                e.tick70 = 34;
                e.max_life = 0;
                e.act_life = 0;
                e.f126 = 256;
                e.flags = 0;
            }
            // sub_3B230: ridge head.
            51 => {
                e.tick70 = 55;
                e.max_life = 0;
                e.act_life = 0;
                e.f26 = 256;
                e.f126 = 1024;
                e.flags = 0;
                e.f80 = 768;
                e.f82 = 768;
                e.f84 = 768;
            }
            // sub_3B690: building/castle spawner (fix-up follows).
            45 => {
                e.tick70 = 51;
                e.max_life = 30;
                e.f44 = 100;
                e.f26 = 4;
                e.flags = 9;
                e.f28 = 33;
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            // sub_3ADB0: transient marker the volcano finish spawns.
            18 => {
                e.tick70 = 18;
                e.max_life = 10000;
                e.act_life = 10000;
                e.f44 = 200;
            }
            // sub_3B300 (model 34): draws once from its own LCG for a
            // target heading — kept for per-entity stream fidelity even
            // though the event is purged unticked.
            34 => {
                e.tick70 = 36;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                lcg32(&mut e.rand);
                let (x, y, z) = (e.x, e.y, e.z);
                self.link(i, x, y, z);
            }
            // All remaining retail models (0, 1, 5, 6, 8, 13, 14, 15,
            // 17, 23, 25, 33, 38, 39, 44, 52, …): purged unticked, no
            // terrain writes, no global PRNG — slot churn only. Models
            // 13/14/15 draw from their (doomed) entity LCG; unobservable.
            _ => {
                e.tick70 = model as u8; // never dispatched
            }
        }
        Some(i)
    }

    /// sub_36DF0 (:43707): building placement fix-up. `bt` = the level
    /// entity's parent + 16, an index into the build table.
    fn building_fixup(&mut self, i: usize, bt: u16) {
        let def = self.assets.build_tab[bt as usize % self.assets.build_tab.len()];
        let (bw, bh) = (def.w as u16, def.h as u16);
        self.ent[i].f26 = 2;
        self.ent[i].f128 = ((bw * bh) >> 4) as i16;
        // Snap to the tile origin.
        let (px, py, pz) = (self.ent[i].x & 0xFF00, self.ent[i].y & 0xFF00, self.ent[i].z);
        self.move_relink(i, px, py, pz);
        let e = &self.ent[i];
        let mut cx = ((e.x >> 8) as u8).wrapping_sub((bw >> 1) as u8);
        let cy = ((e.y >> 8) as u8).wrapping_sub((bh >> 1) as u8);
        if (cx as u16 + cy as u16) % 2 == 1 {
            // Odd corner parity: shift one tile east (relinks).
            let (nx, ny, nz) = (self.ent[i].x.wrapping_add(0x100), self.ent[i].y, self.ent[i].z);
            self.move_relink(i, nx, ny, nz);
            cx = cx.wrapping_add(1);
        }
        let z = 32 * self.avg4(cx, cy, bh as u8, bw as u8) as i32;
        let e = &mut self.ent[i];
        e.f80 = ((bw << 8).wrapping_add(1280)) >> 1;
        e.f82 = ((bh << 8).wrapping_add(1280)) >> 1;
        e.f84 = 0x4000;
        e.act_life = 30;
        e.f44 = 2000;
        e.z = z as i16;
        e.f28 |= 2;
        e.f71 = bt as u8;
    }

    // ---- segment functions ----------------------------------------------

    /// sub_35900 (:42487): the spawn z both wall segments use.
    fn seg_z(&self, x1: i16, y1: u16, x2lo: u8, y2lo: u8) -> i16 {
        let h1 = self.t.height[tile(x1 as u8, y1 as u8)];
        let h2 = self.t.height[tile(x2lo, y2lo)];
        32 * h1.max(h2) as i16
    }

    /// Spawn one wall piece (ctor model 27, sub_3B000 :47142).
    fn spawn_wall_piece(&mut self, x: i16, y: u16, z: i16, tick: u8, run: u16) {
        if let Some(i) = self.new_event() {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 27;
            e.tick70 = tick;
            e.max_life = 2;
            e.act_life = 2;
            e.f44 = ((z >> 5) + 48) as u16;
            e.f26 = run as i16;
            e.flags = 0;
            let (px, py) = ((x as u16) << 8, y << 8);
            self.link(i, px, py, z);
        }
    }

    /// sub_35960 (:42513), model 28: decompose the wrapped delta into a
    /// staircase of `|major|/10 + 1` alternating axis-aligned pieces
    /// (remainders folded into the first step) and spawn a wall-strip
    /// event per piece.
    fn segment_wall(&mut self, x1: i16, y1: u16, x2: i16, y2: i16) {
        let mut dx = Self::wrap_delta(x1, x2);
        let mut dy = Self::wrap_delta(y1 as i16, y2);
        if dx == 0 && dy == 0 {
            return;
        }
        let (mut cx, mut cy) = (x1, y1);
        let (mut ex, mut ey) = (x2 as u8, y2 as u8);
        if dx < 0 {
            dy = -dy;
            dx = -dx;
            // Swap endpoints (only the low bytes of the far end are used).
            let (sx, sy) = (cx as u8, cy as u8);
            cx = ex as i16;
            cy = ey as u16;
            ex = sx;
            ey = sy;
        }
        if dy.abs() >= dx {
            let steps = (dy / 10).abs() + 1;
            let (qy, mut ry) = (dy / steps, dy % steps);
            let (qx, mut rx) = (dx / steps, dx % steps);
            for _ in 0..steps {
                let z = self.seg_z(cx, cy, ex, ey as u8);
                if qy >= 0 {
                    self.spawn_wall_piece(cx, cy, z, 28, (ry + qy) as u16);
                } else {
                    self.spawn_wall_piece(cx, cy, z, 27, (-qy - ry) as u16);
                }
                cy = cy.wrapping_add((qy + ry) as u16);
                let z = self.seg_z(cx, cy, ex, ey as u8);
                self.spawn_wall_piece(cx, cy, z, 29, (rx + qx) as u16);
                cx = cx.wrapping_add((rx + qx) as i16);
                ry = 0;
                rx = 0;
            }
        } else {
            let steps = dx / 10 + 1;
            let (qx, mut rx) = (dx / steps, dx % steps);
            let (qy, mut ry) = (dy / steps, dy % steps);
            for _ in 0..steps {
                let z = self.seg_z(cx, cy, ex, ey as u8);
                self.spawn_wall_piece(cx, cy, z, 29, (rx + qx) as u16);
                cx = cx.wrapping_add((rx + qx) as i16);
                let z = self.seg_z(cx, cy, ex, ey as u8);
                if qy >= 0 {
                    self.spawn_wall_piece(cx, cy, z, 28, (ry + qy) as u16);
                } else {
                    self.spawn_wall_piece(cx, cy, z, 27, (-qy - ry) as u16);
                }
                cy = cy.wrapping_add((qy + ry) as u16);
                rx = 0;
                ry = 0;
            }
        }
    }

    /// sub_35BF0 (:42629), model 29: split the delta into a diagonal
    /// run and an axis-aligned run; spawn a track-painter event (ctor
    /// model 30, byte70 32) for each.
    fn segment_track(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) {
        let dx = Self::wrap_delta(x1, x2);
        let dy = Self::wrap_delta(y1, y2);
        let sdx = dx.signum();
        let sdy = dy.signum();
        let adx = dx.abs();
        let ady = dy.abs();
        let diag = adx.min(ady);
        let rest = (ady - adx).abs();
        let (rest_dx, rest_dy) = if adx <= ady { (0, sdy) } else { (sdx, 0) };
        let spawn_track = |g: &mut Self, x: i16, y: i16, count: i32, stx: i32, sty: i32| {
            if let Some(i) = g.new_event() {
                let e = &mut g.ent[i];
                e.class64 = 10;
                e.model65 = 30;
                e.tick70 = 32;
                e.max_life = 0;
                e.act_life = 0;
                e.flags = 0;
                e.f26 = count as i16;
                e.f30 = stx as u16;
                e.f32 = sty as u16;
                let (px, py) = ((x as u16) << 8, (y as u16) << 8);
                g.link(i, px, py, 0);
            }
        };
        spawn_track(self, x1, y1, diag, sdx, sdy);
        spawn_track(
            self,
            x1.wrapping_add((diag * sdx) as i16),
            y1.wrapping_add((diag * sdy) as i16),
            rest,
            rest_dx,
            rest_dy,
        );
    }

    /// sub_35D30 (:42697), model 31: spawn a canyon head aimed at the
    /// child, with a life of `distance >> 8` tiles.
    fn segment_canyon(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let (ax, ay) = (x1 << 8, y1 << 8);
        let (bx, by) = (x2 << 8, y2 << 8);
        let ang = Self::angle_between(ax, ay, bx, by);
        let dist = Self::dist_between(ax, ay, bx, by);
        if let Some(i) = self.new_event() {
            let z = 32 * self.t.height[tile(x1 as u8, y1 as u8)] as i16;
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 32;
            e.tick70 = 34;
            e.max_life = 0;
            e.f126 = 256;
            e.flags = 0;
            e.x = ax;
            e.y = ay;
            e.z = z;
            e.f30 = ang;
            e.act_life = (dist >> 8) as i32;
        }
    }

    /// sub_35DE0 (:42722), model 50: spawn a ridge head, life =
    /// `distance / 1024` (one raise every 4 tiles).
    fn segment_ridge(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
        let (ax, ay) = (x1 << 8, y1 << 8);
        let (bx, by) = (x2 << 8, y2 << 8);
        let ang = Self::angle_between(ax, ay, bx, by);
        let dist = Self::dist_between(ax, ay, bx, by);
        if let Some(i) = self.new_event() {
            let z = 16 * self.t.height[tile(x1 as u8, y1 as u8)] as i16;
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 51;
            e.tick70 = 55;
            e.max_life = 0;
            e.f26 = 256;
            e.f126 = 1024;
            e.flags = 0;
            e.f80 = 768;
            e.f82 = 768;
            e.f84 = 768;
            e.x = ax;
            e.y = ay;
            e.z = z;
            e.f30 = ang;
            e.act_life = dist as i32 / 1024;
        }
    }

    // ---- the event loop -------------------------------------------------

    /// sub_36620 (:43181): one global PRNG step, then sweep the pool to
    /// fixpoint. Eligibility is tested on the MODEL; the handler is
    /// selected by byte 70.
    fn event_loop(&mut self) {
        lcg32(&mut self.rand);
        loop {
            let mut run_again = false;
            for i in 1..POOL {
                if self.ent[i].class64 == 0 {
                    continue;
                }
                if self.ent[i].class64 != 10 {
                    self.ent[i].flags |= 0x400;
                } else {
                    let model = self.ent[i].model65;
                    let eligible = match model {
                        0..=0x1A => matches!(model, 9..=0xB),
                        0x1B..=0x20 => true,
                        0x21..=0x2C => false,
                        0x2D => self.ent[i].tick70 == 51,
                        0x2E..=0x31 => false,
                        0x32 | 0x33 => true,
                        _ => false,
                    };
                    if eligible {
                        run_again = true;
                        self.tick(i);
                    } else if model != 0x2D {
                        self.ent[i].flags |= 0x400;
                    }
                }
                if self.ent[i].flags & 0x400 != 0 {
                    self.free_entity(i);
                }
            }
            if !run_again {
                break;
            }
        }
    }

    /// str_255998 (:4856) dispatch by byte 70.
    fn tick(&mut self, i: usize) {
        match self.ent[i].tick70 {
            9 => self.tick_hill(i),
            10 => self.tick_dish(i),
            11 => self.tick_digger(i),
            27 => self.tick_wall_neg_y(i),
            28 => self.tick_wall_pos_y(i),
            29 => self.tick_wall_pos_x(i),
            32 => self.tick_track(i),
            34 => self.tick_canyon_head(i),
            51 => self.tick_building(i),
            55 => self.tick_ridge_head(i),
            // sub_253E0 rows (30, 31, 33, 54, …): pure self-kill.
            _ => self.ent[i].flags |= 0x400,
        }
    }

    /// sub_25470 (:28302), byte70 9: growing hill; finish punches a
    /// -40 pit at the center and spawns a transient model-18 marker.
    fn tick_hill(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        self.ent[i].act_life = life - 1;
        let finish = if life < 0 {
            true
        } else {
            let r = lcg32(&mut self.ent[i].rand);
            let hi = self.ent[i].f26 as i32 / 6;
            self.dig_disc(i, 0, hi, (r % 9) as i16, false)
        };
        if finish {
            self.dig_disc(i, 0, 0, -40, false);
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            let z = self.ground_z(x, y) as i16;
            self.spawn_creator(18, x, y, z);
            self.ent[i].flags |= 0x400;
        }
        // else: damage broadcast + sound (terrain-neutral, omitted).
    }

    /// sub_25570 (:28333), byte70 10: one-shot shallow dish, honoring
    /// building protection.
    fn tick_dish(&mut self, i: usize) {
        let e = self.ent[i];
        if !self.on_water(e.x, e.y) {
            let r = lcg32(&mut self.ent[i].rand);
            let hi = (self.ent[i].f80 >> 8) as i32;
            self.dig_disc(i, 0, hi, -((r % 7) as i16), true);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_25670 (:28379), byte70 11: expanding -3 crater; radius grows
    /// only when the event's pool slot is divisible by 3.
    fn tick_digger(&mut self, i: usize) {
        if self.ent[i].f63 % 3 == 0 {
            self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        }
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        // Damage broadcast (full first tick, /25 after): terrain-neutral,
        // omitted.
        let radius = (e.f80 >> 8) as i16;
        let mut upto = e.f26;
        if upto > radius - 1 {
            upto = radius - 1;
            if e.flags & 2 == 0 {
                self.dig_disc_minus3(i, radius as i32, radius as i32);
            }
        }
        self.ent[i].flags |= 2;
        self.dig_disc_minus3(i, 0, upto as i32);
    }

    /// sub_26670 (:29030), byte70 27: wall strip toward -Y.
    fn tick_wall_neg_y(&mut self, i: usize) {
        let e = self.ent[i];
        let x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = (((e.y as u32 + 128) >> 8) as u8).wrapping_add(2);
        let w = e.act_life as u16; // strip thickness (2)
        for _ in 0..w.wrapping_add(e.f26 as u16) {
            self.t.angle[tile(x.wrapping_sub(1), y)] |= 0x80;
            let mut t = tile(x, y);
            for _ in 0..w {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            self.t.angle[t] |= 0x80;
            y = y.wrapping_sub(1);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_26560 (:28999), byte70 28: wall strip toward +Y, x aligned
    /// even then shifted -1.
    fn tick_wall_pos_y(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = ((e.y as u32 + 128) >> 8) as u8;
        if x & 1 == 1 {
            x = x.wrapping_add(1);
        }
        let w = e.act_life as u16;
        x = x.wrapping_sub(w as u8).wrapping_add(1);
        for _ in 0..w.wrapping_add(e.f26 as u16) {
            self.t.angle[tile(x.wrapping_sub(1), y)] |= 0x80;
            let mut t = tile(x, y);
            for _ in 0..w {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            self.t.angle[t] |= 0x80;
            y = y.wrapping_add(1);
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_26760 (:29059), byte70 29: wall strip toward +X, aligned on
    /// (x+y) parity; border rows above and below.
    fn tick_wall_pos_x(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let y = ((e.y as u32 + 128) >> 8) as u8;
        if (x as u16 + y as u16) % 2 == 1 {
            x = x.wrapping_add(1);
        }
        let run = e.f26 as u16;
        let mut t = tile(x, y).wrapping_sub(256) & 0xFFFF; // row y-1
        for _ in 0..run {
            self.t.angle[t] |= 0x80;
            t = (t + 1) & 0xFFFF;
        }
        let mut yy = y;
        for _ in 0..e.act_life as u16 {
            let mut t = tile(x, yy);
            for _ in 0..run {
                self.wall_raise(t);
                t = (t + 1) & 0xFFFF;
            }
            yy = yy.wrapping_add(1);
        }
        let mut t = tile(x, yy);
        for _ in 0..run {
            self.t.angle[t] |= 0x80;
            t = (t + 1) & 0xFFFF;
        }
        self.ent[i].flags |= 0x400;
    }

    /// The shared wall raise op: +48 height (u8 wrap, no clamp) unless
    /// the tile is already wall (type 8) with a type-8 west neighbor
    /// and no 4-neighbor towering ≥ 31 above (sub_264D0, :28966), then
    /// stamp type 8 on the 2x2 and reshade.
    fn wall_raise(&mut self, t: usize) {
        let raise = if self.t.tile_type[t] != 8 {
            true
        } else {
            let (cx, cy) = (tx(t), ty(t));
            let lim = self.t.height[t] as i32 + 30;
            self.t.tile_type[tile(cx.wrapping_sub(1), cy)] != 8
                || self.t.height[tile(cx.wrapping_sub(1), cy)] as i32 > lim
                || self.t.height[tile(cx.wrapping_add(1), cy)] as i32 > lim
                || self.t.height[tile(cx, cy.wrapping_add(1))] as i32 > lim
                || self.t.height[tile(cx, cy.wrapping_sub(1))] as i32 > lim
        };
        if raise {
            self.t.height[t] = self.t.height[t].wrapping_add(48);
        }
        self.set_type_2x2(t, 8);
    }

    /// sub_26890 (:29106), byte70 32: track painter — walk f26 tiles
    /// stepping (f30, f32), stamping slope 1 + protected retexture.
    fn tick_track(&mut self, i: usize) {
        let e = self.ent[i];
        let mut x = ((e.x as u32 + 128) >> 8) as u8;
        let mut y = ((e.y as u32 + 128) >> 8) as u8;
        let mut n = e.f26 as i32;
        while n != 0 {
            let t = tile(x, y);
            self.t.angle[t] = (self.t.angle[t] & 0xF0) | 1;
            self.recompute_protected(x, y, x, y);
            x = x.wrapping_add(e.f30 as u8);
            y = y.wrapping_add(e.f32 as u8);
            n -= 1;
        }
        self.ent[i].flags |= 0x400;
    }

    /// sub_26920 (:29122), byte70 34: canyon head — spawn a 3-tick
    /// digger at the current position, advance one tile along the
    /// heading; stop on distance or water.
    fn tick_canyon_head(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        if let Some(d) = self.spawn_creator(11, e.x, e.y, e.z) {
            self.ent[d].act_life = 2;
            self.ent[d].f84 = e.f84;
        }
        let (mut x, mut y) = (self.ent[i].x, self.ent[i].y);
        Self::advance(&mut x, &mut y, self.ent[i].f30, self.ent[i].f126);
        self.ent[i].x = x;
        self.ent[i].y = y;
    }

    /// sub_269A0 (:29147), byte70 55: ridge head — raise a radius-3
    /// disc by rand%15+10, advance 4 tiles.
    fn tick_ridge_head(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        let e = self.ent[i];
        if life < 0 || self.on_water(e.x, e.y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        let r = lcg32(&mut self.ent[i].rand);
        self.dig_disc(i, 0, 1024, (r % 0xF + 10) as i16, false);
        // Damage broadcast + sound: terrain-neutral, omitted.
        let (mut x, mut y) = (self.ent[i].x, self.ent[i].y);
        Self::advance(&mut x, &mut y, self.ent[i].f30, self.ent[i].f126);
        self.ent[i].x = x;
        self.ent[i].y = y;
    }

    /// sub_27D30 (:29993), byte70 51: building construction — flatten
    /// the RLE footprint toward the placement height each tick, paint
    /// every 5th tick and at life 1; on the final tick retile the full
    /// rect and become a persistent (inert) castle entity.
    fn tick_building(&mut self, i: usize) {
        let e = self.ent[i];
        let cx = ((e.x as u32 + 128) >> 8) as u8;
        let cy = ((e.y as u32 + 128) >> 8) as u8;
        let target = (e.z >> 5) as i32;
        let def = self.assets.build_tab[e.f71 as usize % self.assets.build_tab.len()];
        let (w, h) = (def.w as u16, def.h as u16);
        let (half_w, half_h) = ((w >> 1) as u8, (h >> 1) as u8);
        self.ent[i].act_life -= 1;
        let life = self.ent[i].act_life;
        let x0 = cx.wrapping_sub(half_w);
        let y0 = cy.wrapping_sub(half_h);
        if life != 0 {
            // Flatten pass.
            let mut rows = h;
            let (mut x, mut y) = (x0, y0);
            let mut c = def.offset as usize;
            while rows != 0 {
                let ctl = self.assets.build_dat[c] as i8;
                c += 1;
                if ctl == 0 {
                    y = y.wrapping_add(1);
                    rows -= 1;
                    x = x0;
                    continue;
                }
                if ctl < 0 {
                    x = x.wrapping_add((-(ctl as i32)) as u8);
                    continue;
                }
                for _ in 0..ctl {
                    let b = self.assets.build_dat[c];
                    c += 1;
                    let t = tile(x, y);
                    let goal = if b < 0xF {
                        if b > 6 { Some(target) } else { None }
                    } else if b >> 4 == 3 {
                        match (b % 16) % 3 {
                            1 => Some(target + 12),
                            2 => Some(target + 16),
                            _ => None,
                        }
                    } else {
                        let lo = b % 16;
                        if lo != 0 {
                            Some(4 * (lo as i32 - 1) + target)
                        } else {
                            None
                        }
                    };
                    if let Some(goal) = goal {
                        let angle_before = self.t.angle[t];
                        let hh = self.t.height[t] as i32;
                        self.t.height[t] =
                            self.t.height[t].wrapping_add(((goal - hh) / life) as u8);
                        if angle_before & 7 == 0 {
                            self.t.angle[t] = (angle_before & 0xF0) | 1;
                            self.recompute_protected(x, y, x, y);
                        }
                    }
                    x = x.wrapping_add(1);
                }
            }
            // Paint pass.
            if life % 5 == 0 || life == 1 {
                let mut rows = h;
                let (mut x, mut y) = (x0, y0);
                let mut c = def.offset as usize;
                while rows != 0 {
                    let ctl = self.assets.build_dat[c] as i8;
                    c += 1;
                    if ctl == 0 {
                        y = y.wrapping_add(1);
                        rows -= 1;
                        x = x0;
                        continue;
                    }
                    if ctl < 0 {
                        x = x.wrapping_add((-(ctl as i32)) as u8);
                        continue;
                    }
                    for _ in 0..ctl {
                        let b = self.assets.build_dat[c];
                        c += 1;
                        let t = tile(x, y);
                        match b >> 4 {
                            0 => {
                                let k = b % 7;
                                if k != 0 {
                                    self.paint(k as i8, 7, t, k - 1);
                                }
                            }
                            hi @ 1..=2 => self.paint(0, b as i8, t, hi + 7),
                            3 => {
                                let lo = b % 16;
                                self.paint((lo % 3) as i8, (lo / 3 + 10) as i8, t, lo / 3 + 10)
                            }
                            hi => self.paint(0, b as i8, t, hi + 11),
                        }
                        x = x.wrapping_add(1);
                    }
                }
            }
        } else {
            // Final tick: retile the whole rect, become a castle.
            self.recompute_protected(x0, y0, cx.wrapping_add(half_w), cy.wrapping_add(half_h));
            // byte70 == 51 (the only load-time case): persist as an
            // inert entity (byte70 52) with perimeter smoothing.
            self.ent[i].act_life = self.ent[i].f44 as i32;
            self.ent[i].flags |= 1;
            self.ent[i].tick70 = 52;
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            self.ent[i].z = self.ground_z(x, y) as i16;
            self.smooth_perimeter(cx, cy, half_h as u16, half_w as u16, 2);
            self.smooth_perimeter(cx, cy, half_h as u16, half_w as u16, 5);
        }
    }

    /// sub_33800 (:40980): paint one building tile. `a4 < 8` writes a
    /// terrain class + retexture; higher codes select {type,
    /// orientation} pairs from the paint tables and set the protection
    /// bit (plus clear bit 3 on the E/SE/S neighbors).
    fn paint(&mut self, a1: i8, a2: i8, t: usize, a4: u8) {
        if a4 < 8 {
            self.t.angle[t] = a4 | (self.t.angle[t] & 0xF0);
            self.recompute_protected(tx(t), ty(t), tx(t), ty(t));
            return;
        }
        let checker = ((tx(t).wrapping_add(ty(t))) & 1) as usize;
        let pair: Option<[u8; 2]> = match a4 {
            8 => {
                self.t.tile_type[t] = 8;
                None
            }
            9 => {
                self.t.tile_type[t] = 9;
                None
            }
            10..=14 => {
                let (v, flat) = self.corner_orient(a1, a2, t);
                let idx = v as usize + if flat { 8 } else { 0 } + 16 * (a4 as usize - 10);
                Some(PAINT_FC[3 + idx / 8][idx % 8])
            }
            15 => {
                self.t.tile_type[t] = 11;
                None
            }
            16 => {
                let cur = self.t.tile_type[t];
                if matches!(cur, 10 | 11 | 12) {
                    None
                } else {
                    let (v, _) = self.corner_orient(cur as i8, a2, t);
                    Some(PAINT_AC[0][v as usize])
                }
            }
            17 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_EC[0][v as usize])
            }
            18 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_FC[checker][v as usize])
            }
            19 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_FC[1 + checker][v as usize])
            }
            20..=22 => {
                let (v, _) = self.corner_orient(a1, a2, t);
                Some(PAINT_BC[a4 as usize - 20][v as usize])
            }
            _ => None,
        };
        if let Some([ty_val, ang]) = pair {
            self.t.tile_type[t] = ty_val;
            self.t.angle[t] = (self.t.angle[t] & 0x8F) | ang;
        }
        // Protection marks: claim this tile, clear bit 3 on E/SE/S.
        self.t.angle[t] = (self.t.angle[t] & 0x77) | 0x80;
        let (cx, cy) = (tx(t), ty(t));
        self.t.angle[tile(cx.wrapping_add(1), cy)] &= 0xF7;
        self.t.angle[tile(cx.wrapping_add(1), cy.wrapping_add(1))] &= 0xF7;
        self.t.angle[tile(cx, cy.wrapping_add(1))] &= 0xF7;
    }

    /// sub_33640 (:40870): corner orientation of a tile's height quad.
    /// `a1`/`a2` act as caller defaults for the max / runner-up corner
    /// indices. Returns (code 0..7, flat) where flat = max-min <= 8.
    fn corner_orient(&self, mut a1: i8, mut a2: i8, t: usize) -> (u8, bool) {
        let (cx, cy) = (tx(t), ty(t));
        let c = [
            self.t.height[t],
            self.t.height[tile(cx.wrapping_add(1), cy)],
            self.t.height[tile(cx.wrapping_add(1), cy.wrapping_add(1))],
            self.t.height[tile(cx, cy.wrapping_add(1))],
        ];
        let mut vmax = 0u8;
        if c[0] != 0 {
            vmax = c[0];
            a1 = 0;
        }
        let mut vmin = 0xFFu8;
        if c[0] != 0xFF {
            vmin = c[0];
        }
        for k in 1..4 {
            if c[k] > vmax {
                vmax = c[k];
                a1 = k as i8;
            }
            if c[k] < vmin {
                vmin = c[k];
            }
        }
        let mut v2nd = 0u8;
        if a1 != 0 && c[0] != 0 {
            v2nd = c[0];
            a2 = 0;
        }
        for k in 1..4 {
            if a1 != k as i8 && c[k] > v2nd {
                v2nd = c[k];
                a2 = k as i8;
            }
        }
        let flat = vmax.wrapping_sub(vmin) as i32 <= 8;
        if vmax as i32 - v2nd as i32 >= 8 {
            return ((a1 as u8) & 7, flat);
        }
        let code = match a1 {
            0 => {
                if a2 == 1 {
                    4
                } else {
                    7
                }
            }
            1 => {
                if a2 == 2 {
                    5
                } else {
                    4
                }
            }
            2 => {
                if a2 == 3 {
                    6
                } else {
                    5
                }
            }
            3 => {
                if a2 != 0 {
                    6
                } else {
                    7
                }
            }
            _ => 0,
        };
        (code, flat)
    }

    /// sub_35F30 (:42799): smooth a ring of thickness `thick`+1 around
    /// the footprint (left+right column strips interleaved, then
    /// top+bottom row strips interleaved), each cell via sub_360C0.
    fn smooth_perimeter(&mut self, cx: u8, cy: u8, half_h: u16, half_w: u16, thick: u8) {
        let left_x = cx.wrapping_sub(half_w as u8).wrapping_sub(thick);
        let right_x = cx.wrapping_add(half_w as u8);
        let top_y = cy.wrapping_sub(half_h as u8);
        for row in 0..(2 * half_h) {
            let y = top_y.wrapping_add(row as u8);
            for k in 0..=thick {
                self.smooth_cell(tile(left_x.wrapping_add(k), y));
                self.smooth_cell(tile(right_x.wrapping_add(k), y));
            }
        }
        let strip_x = cx.wrapping_sub(half_w as u8).wrapping_sub(thick);
        let top_strip_y = cy.wrapping_sub(half_h as u8).wrapping_sub(thick);
        let bot_strip_y = cy.wrapping_add(half_h as u8);
        for col in 0..(2 * thick as u16 + 2 * half_w) {
            let x = strip_x.wrapping_add(col as u8);
            for k in 0..=thick {
                self.smooth_cell(tile(x, top_strip_y.wrapping_add(k)));
                self.smooth_cell(tile(x, bot_strip_y.wrapping_add(k)));
            }
        }
    }

    /// sub_360C0 (:42892): if the cell is land and its NW 2x2 quad has
    /// no building/wall texture (types 6..=0x22), replace its height by
    /// the 3x3 average over similarly-plain cells. Index arithmetic is
    /// linear u16 (rows wrap into each other) — faithful.
    fn smooth_cell(&mut self, t: usize) {
        if self.t.angle[t] & 7 == 0 || self.t.height[t] == 0 {
            return;
        }
        let plain = |ty_val: u8| ty_val <= 5 || ty_val > 0x22;
        let quad = [
            (t.wrapping_sub(257)) & 0xFFFF,
            (t.wrapping_sub(256)) & 0xFFFF,
            (t.wrapping_sub(1)) & 0xFFFF,
            t,
        ];
        if !quad.iter().all(|&q| plain(self.t.tile_type[q])) {
            return;
        }
        let mut sum = 0u32;
        let mut n = 0u32;
        let mut idx = (t.wrapping_sub(257)) & 0xFFFF;
        for _ in 0..3 {
            for _ in 0..3 {
                if plain(self.t.tile_type[idx]) {
                    n += 1;
                    sum += self.t.height[idx] as u32;
                }
                idx = (idx + 1) & 0xFFFF;
            }
            idx = (idx + 253) & 0xFFFF;
        }
        if n != 0 {
            self.t.height[t] = (sum / n) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_assets() -> FeatureAssets {
        // A tiny diamond ring grid centered at (15,15) mimicking
        // SEARCH.DAT's shape: ring = max(|dx|,|dy|) but with a 2x2 ring 0.
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        // One 4x4 building: plain floor (code 7) with a wall ring (0x10).
        let mut dat = Vec::new();
        for row in 0..4 {
            let inner = row == 1 || row == 2;
            dat.push(4u8);
            if inner {
                dat.extend_from_slice(&[0x10, 7, 7, 0x10]);
            } else {
                dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            }
            dat.push(0);
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.push(4);
                e.push(4);
                e
            })
            .collect();
        FeatureAssets::parse(&grid, &tab, &dat).unwrap()
    }

    fn thing(slot: u32, class: u16, model: u16, x: u16, y: u16) -> Thing {
        Thing {
            slot,
            kind: mgc_formats::ThingKind::Entity,
            class,
            model,
            x,
            y,
            dis_id: 0xFFFF,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }
    }

    struct Planes {
        height: Vec<u8>,
        tile_type: Vec<u8>,
        shading: Vec<u8>,
        angle: Vec<u8>,
    }

    fn flat_land(h: u8) -> Planes {
        Planes {
            height: vec![h; GRID],
            tile_type: vec![5; GRID],
            shading: vec![32; GRID],
            angle: vec![5; GRID], // class 5 land
        }
    }

    fn run(p: &mut Planes, things: &[Thing], seed: u32, assets: &FeatureAssets) {
        generate_features_mc1(
            TerrainPlanes {
                height: &mut p.height,
                tile_type: &mut p.tile_type,
                shading: &mut p.shading,
                angle: &mut p.angle,
            },
            things,
            seed,
            assets,
        );
    }

    #[test]
    fn ring_iterator_drops_last_cell_of_end_ring() {
        let assets = synthetic_assets();
        let g = Gen {
            t: TerrainPlanes {
                height: &mut [],
                tile_type: &mut [],
                shading: &mut [],
                angle: &mut [],
            },
            assets: &assets,
            retile: mc1_tables::retile_table(),
            map_entity: vec![],
            ent: vec![],
            free: vec![],
            rand: 0,
            pseudo: 0,
        };
        let r0 = assets.rings[0].len();
        let r1 = assets.rings[1].len();
        assert_eq!(g.ring_cells(0, 0).len(), r0 - 1);
        assert_eq!(g.ring_cells(0, 1).len(), r0 + r1 - 1);
    }

    #[test]
    fn crater_digs_a_bowl() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        let things = vec![thing(0, 10, 11, 128, 128)];
        run(&mut p, &things, 1234, &assets);
        let center = p.height[128 * 256 + 128];
        assert!(center < 100, "crater lowers the center, got {center}");
        // Far away untouched.
        assert_eq!(p.height[10 * 256 + 10], 100);
    }

    #[test]
    fn canyon_chain_carves_a_channel() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        // Two chained canyon nodes: slots 0 and 1 (engine 1 and 2).
        let mut a = thing(0, 10, 31, 100, 100);
        a.swi_id = 1;
        a.child = 2;
        let mut b = thing(1, 10, 31, 120, 100);
        b.swi_id = 1;
        b.parent = 1;
        run(&mut p, &[a, b], 99, &assets);
        // Sampled along the line: meaningfully dug.
        let dug = (100..120)
            .filter(|&x| p.height[100 * 256 + x as usize] < 95)
            .count();
        assert!(dug > 10, "canyon digs along the segment, {dug} tiles dug");
        assert_eq!(p.height[10 * 256 + 200], 100, "far tiles untouched");
    }

    #[test]
    fn building_flattens_and_paints() {
        let assets = synthetic_assets();
        let mut p = flat_land(100);
        // Slope under the building so flattening is observable.
        for y in 0..256 {
            for x in 0..256 {
                p.height[y * 256 + x] = (60 + (x / 8) as i32).min(200) as u8;
            }
        }
        let mut b = thing(0, 10, 45, 128, 128);
        b.parent = 0; // build type 16
        run(&mut p, &[b], 7, &assets);
        // The 4x4 footprint centered near (128,128) got wall paint
        // (types 8/9 or table pairs) and the protection bit.
        let protected = (125..132)
            .flat_map(|y| (125..132).map(move |x| (x, y)))
            .filter(|&(x, y)| p.angle[y * 256 + x] & 0x80 != 0)
            .count();
        assert!(protected >= 8, "building marks protected tiles, got {protected}");
    }

    #[test]
    fn deterministic() {
        let assets = synthetic_assets();
        let things = vec![
            thing(0, 10, 9, 50, 50),
            thing(1, 10, 11, 60, 60),
            thing(2, 10, 45, 80, 80),
        ];
        let mut p1 = flat_land(90);
        let mut p2 = flat_land(90);
        run(&mut p1, &things, 4242, &assets);
        run(&mut p2, &things, 4242, &assets);
        assert_eq!(p1.height, p2.height);
        assert_eq!(p1.tile_type, p2.tile_type);
        assert_eq!(p1.angle, p2.angle);
        assert_eq!(p1.shading, p2.shading);
    }
}
