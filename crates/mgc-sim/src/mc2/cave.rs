//! MC2 cave terrain machinery: the (10,80..86) sculptor band + the
//! (10,81) tube carver — the load-time cave generator that
//! carves caverns into the baked foundation ceiling, plus the shared
//! floor↔ceiling invariant every cave terrain writer re-runs.
//!
//! Traces: docs/traces/mc2-cave-terrain-foundation.md (the mirror
//! foundation + invariant), docs/traces/mc2-class10-high-band.md (the
//! band creators/actions verbatim), docs/traces/
//! mc2-terrain-author-painters.md §4 (the (10,80)→(10,81) chain).
//!
//! The foundation ceiling (mirror about `MapBasicHeight` + the
//! fixed-seed ±3 jitter, retail `sub_43B40`/`sub_43BB0`) is BAKED —
//! the native `mc2_terrain` port runs the original algorithm and the
//! package carries `terrain/ceiling.bin`. The sim starts from that plane and
//! runs the THING-authored sculptors here, in the generate-pass settle
//! loop, exactly like retail's `ApplyEvents`.
//!
//! Cave `mapAngle` bit3 = SEALED rock (ceiling pinned to floor−1) —
//! the OPPOSITE of its non-cave open-sea meaning. Field map: retail
//! `byte_0x46_70` → f71 (phase / packed radii / height multiplier),
//! `byte_0x43_67`/`byte_0x44_68` → f67/f68 (mesa half-extents),
//! `axis_0x9A_154x` → dest_x/dest_y/site_z (radius x / — / peak z).

use crate::engine::features::{Gen, lcg32, tile};

use super::morph::isqrt;
use super::sin_lut::SIN_DB750;

impl Gen {
    /// The level is a cave iff the package carried a ceiling plane
    /// (retail `isCaveLevel_D41B6`; non-cave worlds keep the field
    /// empty).
    pub(crate) fn is_cave(&self) -> bool {
        !self.t.ceiling.is_empty()
    }

    /// THE cave invariant (docs/traces/mc2-cave-terrain-foundation.md
    /// §3, re-run by every floor/ceiling writer): `ceiling > floor` =
    /// OPEN (clear bit3) else pin `ceiling = floor − 1` and SEAL
    /// (set bit3). No-op off-cave.
    pub(crate) fn cave_seal_fixup(&mut self, t: usize) {
        if self.t.ceiling.is_empty() {
            return;
        }
        let floor = self.t.height[t];
        if self.t.ceiling[t] > floor {
            self.t.angle[t] &= !8;
        } else {
            self.t.ceiling[t] = floor.wrapping_sub(1);
            self.t.angle[t] |= 8;
        }
    }

    /// `sub_11E70` (TR:2151) — the ceiling POKE test, margin 0: the
    /// entity's head clearance (`fov` = f84) + its behavior row's
    /// hover (v_12; resolved by the CALLER — row156 indexing differs
    /// per entity family) on the local floor reaches the ceiling.
    /// Cave-gated callers only.
    pub(crate) fn cave_poke(&self, fov: i32, hover: i32, x: u16, y: u16) -> bool {
        fov + self.ground_z(x, y) + hover > self.ceiling_z(x, y)
    }

    /// `sub_11E20` (EF:4620) — the ceiling COLLISION test, margin
    /// 384 (i16-truncated samples, verbatim). Its retail callers are
    /// the MC2 player commit gate's steer-search + stuck-nudge
    /// (moveTest_5D0A0/sub_5DD50 — the FlightVerb::Mc2 arm below).
    pub(crate) fn cave_collide(&self, fov: i32, hover: i32, x: u16, y: u16) -> bool {
        let v = self.ground_z(x, y) as i16 as i32 + hover + fov;
        v > self.ceiling_z(x, y) as i16 as i32 - 384
    }

    /// `sub_104D0_terrain_tile_is_water == 256` (TR:2058): the
    /// predicted tile is DEEP water (type 8) — MC2's one open-level
    /// flight barrier.
    fn mc2_deep_water(&self, x: u16, y: u16) -> bool {
        self.t.tile_type[tile((x >> 8) as u8, (y >> 8) as u8)] == 8
    }

    pub(crate) fn mc2_sealed(&self, x: u16, y: u16) -> bool {
        self.t.angle[tile((x >> 8) as u8, (y >> 8) as u8)] & 8 != 0
    }

    /// `moveTest_5D0A0` (EF:59429) — the MC2 PLAYER FLIGHT commit
    /// gate (docs/traces/mc2-flight-model.md §2; NOT a creature
    /// walker gate — that mislabel is corrected in the trace).
    /// `fov`/`clearance` = the carpet's `array_0x52_82.fov` (100 =
    /// params row 44 rotSpeed/2) and row `0xc` (256).
    ///
    /// Order, verbatim: (a) deep-water two-cardinal slide, refuse if
    /// both land wet (EF:59478-511); non-cave returns HERE — open
    /// refusals never zero speed. (b) cave free-commit when headroom
    /// clears ceiling−576 unsealed, else the 6-step widening ±512
    /// probe picks the roomier unsealed non-colliding side and turns
    /// yaw ±(17·i)/6, else straight-commit unless colliding
    /// (EF:59515-91). (c) the final sealed-tile check + the refusal
    /// block (speed zero + speed-up cancel — flagged to the mover,
    /// EF:59592-605).
    pub(crate) fn mc2_flight_gate(
        &self,
        fov: i32,
        clearance: i32,
        pos: (u16, u16, i16),
        pred: (u16, u16, i16),
    ) -> crate::flight::Mc2GateOut {
        let mut out = crate::flight::Mc2GateOut {
            pass: None,
            wet: false,
            zero_speed: false,
        };
        let mut ok = true;
        let mut pred = pred;

        // (a) the water barrier + cardinal slide (EF:59478-511).
        if self.mc2_deep_water(pred.0, pred.1) {
            out.wet = true;
            let elev = Gen::mc2_radix_tan(pos, pred); // v45
            let dist = Gen::mc2_dist3(pos, pred) as u16 as i32; // v42, u16-cast
            let bearing = Gen::angle_between(pos.0, pos.1, pred.0, pred.1); // v41
            let q = (bearing >> 9) as i32;
            let card_a = ((q << 9) & 0x7FF) as u16;
            let card_b = (((q + 1) << 9) & 0x7FF) as u16;
            let err_a = Gen::arc_err(bearing, card_a) as i32; // v5
            let mut cand = pos;
            Gen::polar_step(&mut cand, card_a, elev, (dist * (512 - err_a) / 512) as i16);
            if self.mc2_deep_water(cand.0, cand.1) {
                cand = pos;
                let err_b = Gen::arc_err(bearing, card_b) as i32;
                Gen::polar_step(&mut cand, card_b, elev, (dist * (512 - err_b) / 512) as i16);
                if self.mc2_deep_water(cand.0, cand.1) {
                    ok = false;
                }
            }
            pred = cand; // the slide commits (unused when refused)
        }
        if !self.is_cave() {
            out.pass = ok.then_some((pred, 0));
            return out;
        }

        // (b) the cave steer-search (EF:59515-91).
        let temp = pred;
        let mut dyaw: i16 = 0;
        let headroom = clearance + self.ground_z(temp.0, temp.1) as i16 as i32 + fov; // v8
        let ceil = self.ceiling_z(temp.0, temp.1) as i16 as i32; // v9
        if headroom < ceil - 576 && !self.mc2_sealed(temp.0, temp.1) {
            // free commit: pred stays as-is
        } else {
            let bearing = Gen::angle_between(pos.0, pos.1, temp.0, temp.1);
            let yaw_l = bearing.wrapping_sub(512) & 0x7FF;
            let yaw_r = bearing.wrapping_add(512) & 0x7FF;
            let mut found = 0u8;
            let mut pick = temp;
            let mut r: i32 = 16;
            let mut i: i32 = 0;
            loop {
                if i >= 6 || found != 0 {
                    break;
                }
                let mut cl = temp;
                Gen::polar_step(&mut cl, yaw_l, 0, r as i16);
                let hl = self.ceiling_z(cl.0, cl.1) as i16 as i32
                    - self.ground_z(cl.0, cl.1) as i16 as i32;
                let mut cr = temp;
                Gen::polar_step(&mut cr, yaw_r, 0, r as i16);
                let hr = self.ceiling_z(cr.0, cr.1) as i16 as i32
                    - self.ground_z(cr.0, cr.1) as i16 as i32;
                let sl = self.mc2_sealed(cl.0, cl.1);
                let sr = self.mc2_sealed(cr.0, cr.1);
                if !sl || !sr {
                    if hl > hr && !sl && !self.cave_collide(fov, clearance, cl.0, cl.1) {
                        pick = cl;
                        found = 1;
                    } else if hr > hl && !sr && !self.cave_collide(fov, clearance, cr.0, cr.1) {
                        pick = cr;
                        found = 2;
                    }
                }
                r += 16 * (i + 1);
                i += 1;
            }
            if found != 0 {
                pred = pick;
                // The steer-assist yaw turn; i has already advanced
                // past the finding iteration (the retail for-loop's
                // i++ runs before the break test).
                let t = (17 * i) / 6;
                dyaw = if found == 1 { -t as i16 } else { t as i16 };
            } else if self.cave_collide(fov, clearance, temp.0, temp.1) {
                ok = false;
            } else {
                pred = temp;
            }
        }

        // (c) final seal check + the cave refusal block.
        if ok && self.mc2_sealed(pred.0, pred.1) {
            ok = false;
        }
        if !ok {
            out.zero_speed = true;
            return out;
        }
        out.pass = Some((pred, dyaw));
        out
    }

    /// `sub_5DD50`'s wedged test (EF:59854-81): the carpet sits in
    /// deep water, on a sealed cave tile, or (mid-nudge, latched)
    /// still pokes the ceiling collision margin.
    pub(crate) fn mc2_flight_stuck(
        &self,
        fov: i32,
        clearance: i32,
        pos: (u16, u16, i16),
        latched: bool,
    ) -> bool {
        if self.mc2_deep_water(pos.0, pos.1) {
            return true;
        }
        if !self.is_cave() {
            return false;
        }
        self.mc2_sealed(pos.0, pos.1)
            || (latched && self.cave_collide(fov, clearance, pos.0, pos.1))
    }

    /// `sub_43C60` (EF:30953) — box-local ceiling roughen: a FRESH
    /// LCG seeded with the fixed 37487429 jitters every OPEN cell's
    /// ceiling ±3 (u8-wrapping, no clamp — unlike the load pass),
    /// then a second sweep re-asserts the invariant over the box.
    /// `a3` counts rows (y), `a4` cells per row (x), both u8-wrapped.
    pub(crate) fn cave_box_jitter(&mut self, ox: u8, oy: u8, rows: i32, cols: i32) {
        if self.t.ceiling.is_empty() {
            return;
        }
        let mut seed: u32 = 37487429;
        let mut y = oy;
        for _ in 0..rows {
            let mut x = ox;
            for _ in 0..cols {
                let t = tile(x, y);
                if self.t.angle[t] & 8 == 0 {
                    let r = lcg32(&mut seed);
                    let d = (r % 7) as i32 - 3;
                    self.t.ceiling[t] = (self.t.ceiling[t] as i32).wrapping_add(d) as u8;
                }
                x = x.wrapping_add(1);
            }
            y = y.wrapping_add(1);
        }
        let mut y = oy;
        for _ in 0..rows {
            let mut x = ox;
            for _ in 0..cols {
                self.cave_seal_fixup(tile(x, y));
                x = x.wrapping_add(1);
            }
            y = y.wrapping_add(1);
        }
    }

    /// `sub_34B00` (EF:25339) — the carved-box WALL RING: for each
    /// border cell of the `(w+? x h+?)` box that is still SEALED
    /// (`angle & 8` — the cavern wall where the carve meets rock),
    /// stamp `tile_type = 1` (wall material), force the walkable
    /// class nibble, and retile/reshade the cell (`sub_462A0`).
    /// `sub_34B00` (EF:25353-412) VERBATIM: rows run x∈[0,w),
    /// columns y∈[0,h) — the SE corner (ox+w, oy+h) is structurally
    /// NEVER visited and the NW corner is visited twice (top row +
    /// left column). The top row and LEFT column stamp angle +
    /// terrain type 1 + retile; the BOTTOM row and RIGHT column
    /// stamp angle + retile but NOT the type (retail's asymmetry).
    pub(crate) fn cave_wall_ring(&mut self, ox: u8, oy: u8, w: i32, h: i32) {
        let stamp = |g: &mut Self, x: u8, y: u8, with_type: bool| {
            let t = tile(x, y);
            if g.t.angle[t] & 8 != 0 {
                g.t.angle[t] = (g.t.angle[t] & 0xF8) | 1;
                if with_type {
                    g.t.tile_type[t] = 1;
                }
                g.mc2_retile_region(x, y, x, y);
            }
        };
        let (bx, by) = (ox.wrapping_add(w as u8), oy.wrapping_add(h as u8));
        let mut x = ox;
        for _ in 0..w {
            stamp(self, x, oy, true);
            stamp(self, x, by, false);
            x = x.wrapping_add(1);
        }
        let mut y = oy;
        for _ in 0..h {
            stamp(self, ox, y, true);
            stamp(self, bx, y, false);
            y = y.wrapping_add(1);
        }
    }

    /// `sub_48E90` — MAX floor over the box PERIMETER (the min twin
    /// lives in morph.rs; same walk shape, init 0).
    pub(crate) fn cave_perimeter_max_floor(&self, ox: u8, oy: u8, w: u16, h: u16) -> i32 {
        self.cave_perimeter(ox, oy, w, h, false, false)
    }
    /// `sub_48EC0` — MIN ceiling over the box perimeter (init 250).
    pub(crate) fn cave_perimeter_min_ceiling(&self, ox: u8, oy: u8, w: u16, h: u16) -> i32 {
        self.cave_perimeter(ox, oy, w, h, true, true)
    }
    /// `sub_48EF0` — MAX ceiling over the box perimeter.
    pub(crate) fn cave_perimeter_max_ceiling(&self, ox: u8, oy: u8, w: u16, h: u16) -> i32 {
        self.cave_perimeter(ox, oy, w, h, false, true)
    }
    /// The shared `sub_48F20` perimeter walk (EF:32647): top+bottom
    /// rows over `w` cells, then right+left columns over `h` cells
    /// (the right column starts at the walk's final x, like retail's
    /// running byte coordinate).
    /// The `sub_48F20`-family walk shape, TRANSPOSED like retail
    /// (see [`Gen::mc2_perimeter_min`]): rows = `h` samples,
    /// bottom row at `oy + w`; columns = `w` samples at `ox + h` /
    /// `ox`. Square-only callers today.
    fn cave_perimeter(&self, ox: u8, oy: u8, w: u16, h: u16, min: bool, ceiling: bool) -> i32 {
        let plane = if ceiling {
            &self.t.ceiling
        } else {
            &self.t.height
        };
        let mut result = if min { 250 } else { 0 };
        let acc = |r: i32, v: i32| if min { r.min(v) } else { r.max(v) };
        let mut x = ox;
        for _ in 0..h {
            result = acc(result, plane[tile(x, oy)] as i32);
            result = acc(result, plane[tile(x, oy.wrapping_add(w as u8))] as i32);
            x = x.wrapping_add(1);
        }
        let mut y = oy;
        for _ in 0..w {
            result = acc(result, plane[tile(x, y)] as i32);
            result = acc(result, plane[tile(x.wrapping_sub(h as u8), y)] as i32);
            y = y.wrapping_add(1);
        }
        result
    }

    // ---- creators ---------------------------------------------------------

    /// `sub_4FB80` (EF:36352) — (10,80) the chain-authoring subtype's
    /// own (inert) worker: cave-only, life 0, action 0x57 (a pure
    /// self-destruct), map-registered, no sprite, no RNG.
    pub(crate) fn mc2_spawn_cave_marker80(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if !self.is_cave() {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 0x50;
            e.tick70 = 0x57;
            e.max_life = 0;
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        Some(i)
    }

    /// `sub_4FB20` (EF:36329) — the (10,81) tube-carver worker:
    /// cave-only, action 0x58, packed radii default 2/2 (`f71`),
    /// actSpeed 256, life 0, untargetable, NOT map-registered. The
    /// chain author overwrites f71 with the leg's packed par3 radii
    /// and dest with the leg's endpoint.
    pub(crate) fn mc2_spawn_tube_carver(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if !self.is_cave() {
            return None;
        }
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 0x51;
        e.tick70 = 0x58;
        e.f71 = 2;
        e.f130 = 256;
        e.max_life = 0;
        e.act_life = 0;
        e.flags &= !8;
        e.x = x;
        e.y = y;
        e.z = z;
        Some(i)
    }

    /// `sub_4FBE0` (EF:36374) — (10,82) box mesa: cave-only, action
    /// 0x59, height multiplier f71 = 2 (raise 3·2 = 6), half-extents
    /// f67/f68 = 3 (a 6×6 box). No sprite, no map registration, life
    /// left at the NewEvent default (the one-shot tick ignores it).
    pub(crate) fn mc2_spawn_cave_mesa(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if !self.is_cave() {
            return None;
        }
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 0x52;
        e.tick70 = 0x59;
        e.max_life = 0;
        e.f71 = 2;
        e.f67 = 3;
        e.f68 = 3;
        e.x = x;
        e.y = y;
        e.z = z;
        Some(i)
    }

    /// `sub_4FC30` (EF:36397) — (10,83) animated dome: cave-only,
    /// action 0x5A, 16-tick life, phase f71 = 0, radius default 2
    /// (the THING post-init overrides with word_10), z = 0 sentinel.
    pub(crate) fn mc2_spawn_cave_dome(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        self.mc2_spawn_pit_hill_base(x, y).inspect(|&i| {
            self.ent[i].model65 = 0x53;
            self.ent[i].tick70 = 0x5A;
        })
    }

    /// `sub_4FCA0`/`sub_4FCD0` → `sub_4FD00` (EF:36421-36465) —
    /// (10,84) pit / (10,85) hill over the shared base ctor.
    pub(crate) fn mc2_spawn_cave_pit(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        self.mc2_spawn_pit_hill_base(x, y).inspect(|&i| {
            self.ent[i].model65 = 0x54;
            self.ent[i].tick70 = 0x5B;
        })
    }
    pub(crate) fn mc2_spawn_cave_hill(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        self.mc2_spawn_pit_hill_base(x, y).inspect(|&i| {
            self.ent[i].model65 = 0x55;
            self.ent[i].tick70 = 0x5C;
        })
    }
    /// The shared `sub_4FD00` base: life 16, class 10, phase 0,
    /// radius 2, bit0 set, z = 0 sentinel (phase 0 randomizes the
    /// depth/height when the THING's par3 left it 0), untargetable.
    fn mc2_spawn_pit_hill_base(&mut self, x: u16, y: u16) -> Option<usize> {
        if !self.is_cave() {
            return None;
        }
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.act_life = 16;
        e.f71 = 0;
        e.dest_x = 2;
        e.site_z = 0;
        e.flags |= 1;
        e.flags &= !8;
        e.x = x;
        e.y = y;
        e.z = 0;
        Some(i)
    }

    /// `sub_50960` (EF:37011) — (10,86) cave drip: maxLife 9, action
    /// 0x5D, z snapped to the floor, ONE local RNG draw for the
    /// sprite (332..334), rejected unless the cell's angle class is
    /// 0 (`sub_104A0 & 1`). NOT ctor-gated on the cave flag — its
    /// runtime spawner is (the authored records ride the settle
    /// disable band). Byte2 bit1 (recycle-list membership) has no
    /// ported home; the pool free list covers it.
    pub(crate) fn mc2_spawn_cave_drip(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 0x56;
            e.tick70 = 0x5D;
            e.max_life = 9;
            e.act_life = 9;
            e.flags &= !8;
        }
        let z = self.ground_z(x, y) as i16;
        self.link(i, x, y, z);
        // u16 entity draw (EF:37025-26), not raw lcg32.
        let r = self.ent_rand(i);
        self.mc2_set_sprite(i, (r % 3 + 332) as u16);
        let raw = tile((x >> 8) as u8, (y >> 8) as u8);
        if (1u32 << (self.t.angle[raw] & 0xF)) & 1 == 0 {
            self.ent[i].flags |= 0x400; // sub_57F20 reject
            return None;
        }
        Some(i)
    }

    /// `sub_50A20` (EF:37037) — the (10,89) CAVE-IN ground effect:
    /// CAVE-ONLY (returns None off-cave), action 0x60, life 40,
    /// byte[0] &= 0xF6 |= 1. Never authored (zero records) — spawned
    /// only by the Cave-In spell's (9,30) manifestation impact,
    /// whose action-31 wrapper (`sub_67910` EF:59218-30) then writes
    /// `maxLife = tier charge` (the ring-count key) and resets the
    /// phase to 0.
    pub(crate) fn mc2_spawn_cave_in(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if !self.is_cave() {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 89;
            e.tick70 = 0x60;
            e.flags = (e.flags & !0x8) | 1;
        }
        self.link(i, x, y, z);
        self.ent[i].act_life = 40;
        Some(i)
    }

    /// `sub_311E0` (EF:22860) — the (10,89) action-0x60 CAVE-IN
    /// collapse. Phases on `byte_0x46_70` (f71): 0 = init (anchor
    /// ceiling-MAX/floor-MIN over the box perimeter, wave 227,
    /// phase 2, then fall through), 1 = frozen (verbatim early
    /// return), 2 = running, 3 = done → despawn. Per tick: 6
    /// concentric rings (base 3/5/7 by `maxLife` = the tier charge,
    /// +2 tiles per ring), each carving cells in its [inner, outer)
    /// distance window — floor RAISED (rubble, forced-flat
    /// sub_570F0), ceiling DROPPED, both on a `sin_DB750` profile
    /// scaled by the wave phase; class-3 MODEL-0 entities (wizards)
    /// get a spherical survival POCKET carved around them (floor
    /// down / ceiling up — a pocket, NOT a burial);
    /// every box cell then runs the invariant WITH the ceiling
    /// pinned to the FLOOR (not floor−1 — this variant's quirk).
    /// Once past wave 455: one ring of ~74 (10,13) rocks flung
    /// outward (yaw k, maxSpeed 512, z = 32·floor of the LAST
    /// ring-swept cell — retail's stale `ix`). Wave += 22/tick,
    /// life += 4; wave > 1024 → phase 3. NO direct HP write — the
    /// terrain is the weapon.
    pub(crate) fn mc2_cave_in_tick(&mut self, i: usize) -> bool {
        let rings: i32 = match self.ent[i].max_life {
            1 => 5,
            2 => 7,
            _ => 3,
        };
        let box_r = rings + 12; // v36
        match self.ent[i].f71 {
            1 => return false,
            0 => {
                let ox = ((self.ent[i].x.wrapping_add(128) >> 8) as u8).wrapping_sub(box_r as u8);
                let oy = ((self.ent[i].y.wrapping_add(128) >> 8) as u8).wrapping_sub(box_r as u8);
                let zmax =
                    self.cave_perimeter_max_ceiling(ox, oy, 2 * box_r as u16, 2 * box_r as u16);
                let fmin = self.mc2_perimeter_min(ox, oy, 2 * box_r as u16, 2 * box_r as u16);
                let e = &mut self.ent[i];
                e.z = zmax as i16;
                e.act_life = 40;
                e.f44 = 227;
                e.f54 = 0;
                e.f71 = 2;
                e.site_z = fmin as i16;
                // falls through into the first wave tick (EF:22946).
            }
            2 => {}
            p => {
                if p == 3 {
                    self.ent[i].flags |= 0x400;
                }
                return false;
            }
        }
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let v32 = self.ent[i].z as i32; // ceiling anchor
        let v42 = self.ent[i].site_z as i32; // floor anchor
        let v44 = ((v32 - v42) >> 1) + (v32 - v42);
        let mut v7 = self.ent[i].f44 as i16 as i32; // wave phase
        let mut ring_r = rings; // v38, tiles
        let mut outer = 0i32; // v35, carried across rings
        let mut changed = false;
        let mut last_t: Option<usize> = None;
        for _ in 0..6 {
            let mut inner = (outer - 1024).max(0);
            if (box_r << 8) < inner {
                inner = box_r << 8;
            }
            outer = ring_r << 8;
            let side = 2 * ring_r;
            if v7 > 0 && v7 <= 512 {
                let ox = ((ex.wrapping_add(128) >> 8) as u8).wrapping_sub(ring_r as u8);
                let oy = ((ey.wrapping_add(128) >> 8) as u8).wrapping_sub(ring_r as u8);
                let mut y = oy;
                for _ in 0..side {
                    let mut x = ox;
                    for _ in 0..side {
                        let t = tile(x, y);
                        let d = super::morph::dist2d(ex, ey, (x as i32) << 8, (y as i32) << 8);
                        if d < outer && d >= inner {
                            changed = true;
                            let s1 = SIN_DB750[0x200 + ((d << 10) / outer) as usize] as i64;
                            let v11 = (((v44 as i64) * ((0x10000 + s1) >> 1)) >> 16)
                                * (0x10000 - SIN_DB750[0x200 + v7 as usize] as i64);
                            let v12 = (v11 >> 16) as i32;
                            let rise = ((v11 >> 18) as i32) + v42;
                            if (self.t.height[t] as i32) < rise {
                                self.cave_write_floor(x, y, rise, true, true);
                            }
                            let drop = (v32 - v12).max(0);
                            if self.t.ceiling[t] as i32 > drop {
                                self.t.ceiling[t] = drop as u8;
                            }
                            // The wizard survival pocket (EF:23003-37):
                            // class-3 model-0 within 0x64000 sq units
                            // gets a spherical cavity dug around it.
                            let (wx, wy) = (((x as u16) << 8), ((y as u16) << 8));
                            for j in 1..self.ent.len() {
                                let c = &self.ent[j];
                                if c.class64 != 3 || c.model65 != 0 || c.flags & 0x400 != 0 {
                                    continue;
                                }
                                let dx = (wx.wrapping_sub(c.x) as i16 as i32).abs();
                                let dy = (wy.wrapping_sub(c.y) as i16 as i32).abs();
                                let d2 = dx * dx + dy * dy;
                                if d2 <= 0x64000 {
                                    let r = (isqrt((0x64000 - d2) as u32) >> 5) as i32;
                                    let cz = (self.ent[j].z >> 5) as i32;
                                    let lo = (cz - r).clamp(0, 254);
                                    if self.t.height[t] as i32 > lo {
                                        self.cave_write_floor(x, y, lo, false, true);
                                    }
                                    let hi = (r + cz).clamp(0, 254);
                                    if (self.t.ceiling[t] as i32) < hi {
                                        self.t.ceiling[t] = hi as u8;
                                    }
                                }
                            }
                        }
                        // The invariant, ceiling pinned to the FLOOR
                        // (EF:23040-49 — this variant's quirk: v22,
                        // not floor − 1).
                        let fl = self.t.height[t];
                        if self.t.ceiling[t] > fl {
                            self.t.angle[t] &= !8;
                        } else {
                            self.t.ceiling[t] = fl;
                            self.t.angle[t] |= 8;
                        }
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
                // Retail's running index runs ONE PAST the box in
                // both axes when the walk ends (EF:22975-23052) —
                // the debris z below reads that stale NEIGHBOR
                // cell, not the last carved cell.
                last_t = Some(tile(
                    ox.wrapping_add(side as u8),
                    oy.wrapping_add(side as u8),
                ));
            }
            v7 -= 68;
            ring_r += 2;
        }
        // The one-shot debris burst (EF:23052-77).
        if self.ent[i].f54 == 0 && (self.ent[i].f44 as i16) > 455 {
            self.ent[i].f54 = 1;
            let dist = ((rings << 8) - 768).clamp(256, 0x2000) as i16;
            let rock_z = last_t.map_or(self.ent[i].z, |t| 32 * self.t.height[t] as i16);
            let mut k = 0u16;
            while k < 2048 {
                let mut p = (ex, ey, self.ent[i].z);
                Self::polar_step(&mut p, k, 0, dist);
                if let Some(s) = self.mc2_spawn_smoke_particle_for(13, p.0, p.1, p.2) {
                    self.ent[s].f30 = k;
                    self.ent[s].f130 = 512; // maxSpeed_0x86_134
                    self.move_relink(s, p.0, p.1, rock_z);
                }
                k += 28;
            }
        }
        self.ent[i].act_life += 4;
        let wave = (self.ent[i].f44 as i16).wrapping_add(22);
        self.ent[i].f44 = wave as u16;
        if wave > 1024 {
            self.ent[i].f71 = 3;
        }
        changed
    }

    // ---- action handlers ----------------------------------------------------

    /// `sub_34910` (EF:25265) — (10,82) action 0x59: the one-tick
    /// rectangular room carve. Lowers the floor by 3·f71 below the
    /// box-perimeter MIN and raises the ceiling the same amount above
    /// the perimeter MAX, seals/opens per cell, stamps the wall ring
    /// and roughens the box. One-shot.
    pub(crate) fn mc2_cave_mesa_tick(&mut self, i: usize) {
        let e = &self.ent[i];
        let raise = 3 * e.f71 as i32;
        let (hx, hy) = (e.f67 as i32, e.f68 as i32);
        let (w, h) = (2 * hx, 2 * hy);
        let ox = ((e.x >> 8) as i32 - hx) as u8;
        let oy = ((e.y >> 8) as i32 - hy) as u8;
        let lo = self.mc2_perimeter_min(ox, oy, w as u16, h as u16);
        let hi = self.cave_perimeter_max_floor(ox, oy, w as u16, h as u16);
        let floor_t = (lo - raise).clamp(0, 254);
        let ceil_t = (raise + hi).clamp(0, 254);
        let mut y = oy;
        for _ in 0..h {
            let mut x = ox;
            for _ in 0..w {
                let t = tile(x, y);
                if self.t.height[t] as i32 > floor_t {
                    self.cave_write_floor(x, y, floor_t, false, false);
                }
                if ceil_t > self.t.ceiling[t] as i32 {
                    self.t.ceiling[t] = ceil_t as u8;
                }
                self.cave_seal_fixup(t);
                x = x.wrapping_add(1);
            }
            y = y.wrapping_add(1);
        }
        self.cave_wall_ring(ox.wrapping_sub(1), oy.wrapping_sub(1), w + 1, h + 1);
        self.cave_box_jitter(ox, oy, w, h);
        self.ent[i].flags |= 0x400;
    }

    /// `sub_34C40` (EF:25419) — (10,83) action 0x5A: the animated
    /// cosine dome (floor rises toward the box MIN-floor + profile,
    /// ceiling descends toward MAX-ceiling − profile), ramped `/life`
    /// over the 16-tick animation. Phase f71: 0 = sample the box,
    /// 1 = animate, 2 = nothing to do.
    pub(crate) fn mc2_cave_dome_tick(&mut self, i: usize) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life <= 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let e = &self.ent[i];
        let r = e.dest_x as i32;
        let side = 2 * r;
        let ox = (((e.x.wrapping_add(128) >> 8) as i32) - r) as u8;
        let oy = (((e.y.wrapping_add(128) >> 8) as i32) - r) as u8;
        match self.ent[i].f71 {
            0 => {
                let base = self.mc2_perimeter_min(ox, oy, side as u16, side as u16);
                let peak = self.cave_perimeter_max_ceiling(ox, oy, side as u16, side as u16);
                let e = &mut self.ent[i];
                e.z = base as i16;
                e.site_z = peak as i16;
                e.f71 = if peak - base <= 0 { 2 } else { 1 };
            }
            1 => {
                let e = &self.ent[i];
                let rw = r << 8; // radius in world units
                let range = (e.site_z - e.z) as i64;
                let inner = (192 * rw) >> 8; // 0.75r: the walkable core
                let (cx, cy, base, peak) = (e.x, e.y, e.z as i32, e.site_z as i32);
                let life = self.ent[i].act_life as i64;
                let mut y = oy;
                for _ in 0..side {
                    let mut x = ox;
                    for _ in 0..side {
                        let t = tile(x, y);
                        // Retail measures to the tile CORNER (i<<8,
                        // EF:25496-98) — no +128.
                        let d = super::morph::dist2d(cx, cy, (x as i32) << 8, (y as i32) << 8);
                        if d < rw {
                            let s = SIN_DB750[0x200 + ((d << 10) / rw) as usize] as i64;
                            let hprof = ((range * ((0x10000 + s) >> 1)) >> 16) as i32;
                            let lift = (base + hprof).min(254);
                            let floor = self.t.height[t] as i32;
                            if lift > floor {
                                let step = floor + (lift - floor) / life as i32;
                                self.cave_write_floor(x, y, step, d <= inner, true);
                            }
                            let lower = (peak - hprof).max(0);
                            let cur = self.t.ceiling[t] as i32;
                            if lower < cur {
                                self.t.ceiling[t] = (cur - (cur - lower) / life as i32) as u8;
                            }
                            // Retail syncs bit3 inside the radius
                            // branch only (EF:25490-25510), and the
                            // dome's sync is SYNC-ONLY — no
                            // ceiling=floor-1 pin (EF:25522-25); the
                            // pit/hill/mesa/tube arms DO pin.
                            if self.t.ceiling[t] > self.t.height[t] {
                                self.t.angle[t] &= !8;
                            } else {
                                self.t.angle[t] |= 8;
                            }
                        }
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
            }
            _ => self.ent[i].act_life = 0,
        }
    }

    /// `sub_34EE0` (EF:25544) — (10,84) pit / (10,85) hill shared
    /// action 0x5B/0x5C: phase 0 picks the target depth/height at the
    /// center (authored par3 via the z sentinel, else ONE local RNG
    /// draw `rand % range`), phase 1 animates the cosine bowl/mound
    /// (pit digs the CEILING down, hill raises the FLOOR), phase 2
    /// finalizes (pit re-roughens its box).
    pub(crate) fn mc2_cave_pit_hill_tick(&mut self, i: usize) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life <= 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let e = &self.ent[i];
        let model = e.model65;
        let r = e.dest_x as i32;
        let side = 2 * r;
        let ox = (((e.x.wrapping_add(128) >> 8) as i32) - r) as u8;
        let oy = (((e.y.wrapping_add(128) >> 8) as i32) - r) as u8;
        match self.ent[i].f71 {
            0 => {
                let (a1, a2) = if model == 0x54 {
                    // PIT: from the MAX ceiling down toward MAX floor.
                    let a1 = self.cave_perimeter_max_ceiling(ox, oy, side as u16, side as u16);
                    (
                        a1,
                        a1 - self.cave_perimeter_max_floor(ox, oy, side as u16, side as u16) - 1,
                    )
                } else {
                    // HILL: from the MIN floor up toward MIN ceiling.
                    let a1 = self.mc2_perimeter_min(ox, oy, side as u16, side as u16);
                    (
                        a1,
                        self.cave_perimeter_min_ceiling(ox, oy, side as u16, side as u16) - a1 - 1,
                    )
                };
                if a2 <= 0 {
                    self.ent[i].f71 = 2;
                } else {
                    let amount = if self.ent[i].z != 0 {
                        51 * self.ent[i].z as i32 * a2 / 256
                    } else {
                        // u16 entity draw (EF:25639-40), not raw
                        // lcg32.
                        let d = self.ent_rand(i);
                        (d % a2 as u32) as i32
                    };
                    let e = &mut self.ent[i];
                    e.site_z = a1 as i16;
                    e.z = if model == 0x54 {
                        (a1 - amount) as i16
                    } else {
                        (amount + a1) as i16
                    };
                    e.f71 = 1;
                }
            }
            1 => {
                let e = &self.ent[i];
                let rw = r << 8;
                let range = (e.z as i32 - e.site_z as i32).unsigned_abs() as i64;
                let inner = (49152 * r) >> 8; // 0.75r << 8
                let (cx, cy, anchor) = (e.x, e.y, e.site_z as i32);
                let life = self.ent[i].act_life as i32;
                let mut y = oy;
                for _ in 0..side {
                    let mut x = ox;
                    for _ in 0..side {
                        let t = tile(x, y);
                        // Tile CORNER, not center (EF:25666-68).
                        let d = super::morph::dist2d(cx, cy, (x as i32) << 8, (y as i32) << 8);
                        if d < rw {
                            let s = SIN_DB750[0x200 + ((d << 10) / rw) as usize] as i64;
                            let prof = ((range * ((0x10000 + s) >> 1)) >> 16) as i32;
                            if model == 0x54 {
                                let lo = (anchor - prof).max(0);
                                let cur = self.t.ceiling[t] as i32;
                                let next = cur - (cur - lo) / life;
                                if next < cur {
                                    self.t.ceiling[t] = next as u8;
                                }
                            } else {
                                let hi = (anchor + prof).min(254);
                                let floor = self.t.height[t] as i32;
                                let step = (hi - floor) / life;
                                if step + floor > floor {
                                    self.cave_write_floor(x, y, step + floor, d <= inner, true);
                                }
                            }
                            self.cave_seal_fixup(t);
                        }
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
            }
            _ => {
                if model == 0x54 {
                    self.cave_box_jitter(ox, oy, side, side);
                }
                self.ent[i].act_life = 0;
            }
        }
    }

    /// `sub_34540` (EF:25083) — (10,81) action 0x58: the one-shot
    /// TUBE CARVER. Primes a 32-sample rolling midline buffer
    /// (`(floor+ceiling)/2` sampled ahead along the stroke, 85 units
    /// per step), then walks position→dest carving a disc per step:
    /// radius eased from the f71 high-nibble to the low-nibble
    /// (`(n<<8)+512`), floor LOWERED toward `mid − √(r²−d²)/32`,
    /// ceiling RAISED toward `mid + √(...)`, invariant per cell,
    /// wall-ring per step box. Deltas are torus-wrapped like retail's
    /// byte coordinates.
    pub(crate) fn mc2_tube_carve_tick(&mut self, i: usize) {
        let e = &self.ent[i];
        let start_r = ((e.f71 as i32 >> 4) << 8) + 512;
        let end_r = ((e.f71 as i32 & 0xF) << 8) + 512;
        let (sx, sy) = (e.x, e.y);
        let (dx_, dy_) = (e.dest_x, e.dest_y);
        let dist = {
            let dxw = (dx_ as i16).wrapping_sub(sx as i16) as i32;
            let dyw = (dy_ as i16).wrapping_sub(sy as i16) as i32;
            isqrt((dxw * dxw + dyw * dyw) as u32) as i32
        };
        let steps = dist / 0x55;
        let yaw = Self::angle_between(sx, sy, dx_, dy_);
        // Prime the 32-sample midline buffer.
        let mut buf = [0u8; 32];
        let mut probe = (sx, sy, 0i16);
        for slot in buf.iter_mut() {
            let t = tile((probe.0 >> 8) as u8, (probe.1 >> 8) as u8);
            let mid = ((self.t.height[t] as i32 + self.t.ceiling[t] as i32) / 2).clamp(0, 254);
            *slot = mid as u8;
            Self::polar_step(&mut probe, yaw, 0, 85);
        }
        let mut walk = (sx, sy, 0i16);
        for step in 0..steps {
            let radius = start_r + step * ((end_r - start_r) / steps);
            let r2 = radius * radius;
            let box_units = 2 * radius + 128;
            let side = box_units >> 8;
            let half = box_units >> 9;
            let cx = (walk.0.wrapping_add(128) >> 8) as u8;
            let cy = (walk.1.wrapping_add(128) >> 8) as u8;
            let baseline = buf[0] as i32;
            let ox = cx.wrapping_sub(half as u8);
            let oy = cy.wrapping_sub(half as u8);
            let mut y = oy;
            for _ in 0..side {
                let mut x = ox;
                for _ in 0..side {
                    let t = tile(x, y);
                    // Torus-wrapped tile deltas, in world units.
                    let wrapd = |a: u8, b: u8| -> i32 {
                        let d = (a as i32 - b as i32).abs();
                        if d >= 0x80 { (d - 256).abs() } else { d }
                    };
                    let ddx = wrapd(x, cx) << 8;
                    let ddy = wrapd(y, cy) << 8;
                    let d2 = ddx * ddx + ddy * ddy;
                    if d2 <= r2 {
                        let fall = (isqrt((r2 - d2) as u32) >> 5) as i32;
                        let lo = (baseline - fall).clamp(0, 254);
                        if self.t.height[t] as i32 > lo {
                            self.cave_write_floor(x, y, lo, false, true);
                        }
                        let hi = (baseline + fall).clamp(0, 254);
                        if (self.t.ceiling[t] as i32) < hi {
                            self.t.ceiling[t] = hi as u8;
                        }
                    }
                    self.cave_seal_fixup(t);
                    x = x.wrapping_add(1);
                }
                y = y.wrapping_add(1);
            }
            // Retail: sub_34B00(ox-1, oy-1, side+1, side+1)
            // (EF:25243) — the +1 dims cover the far row/column of
            // the wall ring.
            self.cave_wall_ring(ox.wrapping_sub(1), oy.wrapping_sub(1), side + 1, side + 1);
            Self::polar_step(&mut walk, yaw, 0, 85);
            buf.copy_within(1.., 0);
            let t = tile((probe.0 >> 8) as u8, (probe.1 >> 8) as u8);
            buf[31] =
                (((self.t.height[t] as i32 + self.t.ceiling[t] as i32) / 2).clamp(0, 254)) as u8;
            Self::polar_step(&mut probe, yaw, 0, 85);
        }
        self.ent[i].flags |= 0x400;
    }

    /// `sub_31120` (EF:22826) — (10,86) action 0x5D: the animated
    /// drip. Dies if its floor cell's height changed; at tick 4
    /// (maxLife−5 == life) emits one (10,87) smoke puff and, on a
    /// coin from the GLOBAL stream, the drip sound (sprite row −
    /// 282 → 50..52); otherwise counts down + advances the frame.
    pub(crate) fn mc2_cave_drip_tick(&mut self, i: usize) {
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        if self.ground_z(x, y) as i16 != z {
            self.ent[i].flags |= 0x400;
            return;
        }
        if self.ent[i].max_life as i32 - 5 == self.ent[i].act_life {
            self.mc2_spawn_smoke_particle_for(87, x, y, z);
            let r = lcg32(&mut self.rand);
            if r & 1 == 0 {
                let snd = self.ent[i].type86.wrapping_sub(282);
                self.snd(snd as u8, i);
            }
        }
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
        } else {
            // sub_585A0 (EF:40438): frame advance up to the count.
            let e = &mut self.ent[i];
            if e.frame88 + 1 < e.frames89 {
                e.frame88 += 1;
            }
        }
    }

    /// `sub_5B100` (EF:42529) — the (14,2) CAVE PILLAR, class-14
    /// action 7: a floor-to-ceiling column machine on `life_0x8`
    /// (our `act_life`): 0 = MEASURE (find the two flanking sealed
    /// walls along the long axis, base z = their floor midpoint,
    /// stamp the footprint dirty, then idle at 4), 1 = GROW (floor
    /// rises + ceiling drops by koef2/4 per tick toward the capped
    /// targets, loop sound 47, seal cells as they meet → life 3),
    /// 2 = RETRACT (relax both planes toward the flank profile →
    /// life 4). Externally triggered by the (10,63)/(10,64) riser
    /// triggers ([`Gen::mc2_riser_trigger_tick`] matches model 2).
    /// Footprint: `koef2 = 2·par3 + 4` long × 2 wide, `par1` picks
    /// the long axis (nonzero = X). Returns true when terrain
    /// changed. NOTE the grow/retract bit3 sync sets/clears the seal
    /// WITHOUT the ceiling-pin — verbatim retail, not the full
    /// invariant.
    pub(crate) fn mc2_pillar_tick(&mut self, i: usize) -> bool {
        let life = self.ent[i].act_life;
        if !(0..=2).contains(&life) {
            return false;
        }
        let ez = self.ent[i].z as i32;
        let koef2 = {
            let hw = self.ent[i].f146 as i32;
            (2 * ((hw << 8) + 512) + 128) >> 8
        };
        let orient = self.ent[i].f44;
        let (koef_x, koef_y) = if orient != 0 { (koef2, 2) } else { (2, koef2) };
        let ex = (self.ent[i].x >> 8) as u8;
        let ey = (self.ent[i].y >> 8) as u8;
        let kx = ex.wrapping_sub((koef_x >> 1) as u8);
        let ky = ey.wrapping_sub((koef_y >> 1) as u8);
        let s = (8 * koef2) >> 5; // my_sign32 fold: koef2/4 for positive
        match life {
            0 => {
                // MEASURE (EF:42566-42638). Retail scans until it
                // hits sealed rock, byte-wrapping forever; 256 steps
                // covers the full wrap period, beyond which retail
                // would hang (never in data).
                self.ent[i].act_life = 4;
                let scan = |g: &Self, dx: u8, dy: u8| -> i32 {
                    let (mut x, mut y) = (ex, ey);
                    for _ in 0..256 {
                        let t = tile(x, y);
                        if g.t.angle[t] & 8 != 0 {
                            return g.t.height[t] as i32;
                        }
                        x = x.wrapping_add(dx);
                        y = y.wrapping_add(dy);
                    }
                    -1
                };
                let (h1, h2) = if orient != 0 {
                    (scan(self, 0xFF, 0), scan(self, 1, 0))
                } else {
                    (scan(self, 0, 0xFF), scan(self, 0, 1))
                };
                if h1 == -1 || h2 == -1 {
                    self.ent[i].flags |= 0x400; // sub_57F20 despawn
                    return false;
                }
                self.ent[i].z = ((h2 + h1) >> 1) as i16;
                let mut y = ky;
                for _ in 0..koef_y {
                    let mut x = kx;
                    for _ in 0..koef_x {
                        self.t.angle[tile(x, y)] |= 0x80;
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
                true
            }
            1 => {
                // GROW (EF:42641-42697).
                self.snd(47, i);
                let ec = tile(ex, ey);
                let mut z1 = s + self.t.height[ec] as i32 + 1;
                let mut z2 = self.t.ceiling[ec] as i32 - s - 1;
                if z1 < 0 {
                    z1 = 0;
                }
                if z1 > ez {
                    z1 = ez;
                }
                if ez > z2 {
                    z2 = ez;
                }
                if z2 > 254 {
                    z2 = 254;
                }
                let mut done = true;
                let mut y = ky;
                for _ in 0..koef_y {
                    let mut x = kx;
                    for _ in 0..koef_x {
                        let t = tile(x, y);
                        if self.t.angle[t] & 8 == 0 {
                            let z5 = (s + self.t.height[t] as i32).min(z1);
                            if z5 > self.t.height[t] as i32 {
                                self.t.height[t] = z5 as u8;
                                done = false;
                            }
                            let z3 = (self.t.ceiling[t] as i32 - s).max(z2);
                            if z3 < self.t.ceiling[t] as i32 {
                                self.t.ceiling[t] = z3 as u8;
                                done = false;
                            }
                        }
                        if self.t.ceiling[t] > self.t.height[t] {
                            self.t.angle[t] &= !8;
                        } else {
                            self.t.angle[t] |= 8;
                        }
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
                if done {
                    self.ent[i].act_life = 3; // built + EndLoop 47
                }
                !done
            }
            _ => {
                // RETRACT (EF:42700-42783).
                self.snd(47, i);
                // The flank blend (EF:42723-42744 twice): midpoint of
                // the near flanks if they agree within 4, else the
                // far side unless the extended flank agrees.
                let pick = |a: i32, b: i32, c: i32| -> i32 {
                    if (a - b).abs() <= 4 {
                        (a + b) / 2
                    } else if (a - c).abs() > 4 {
                        b
                    } else {
                        a
                    }
                };
                let mut done = true;
                let mut y = ky;
                for _ in 0..koef_y {
                    let mut x = kx;
                    for _ in 0..koef_x {
                        let t = tile(x, y);
                        let (p3, p4, p5) = if orient != 0 {
                            (
                                tile(x, ky.wrapping_sub(1)),
                                tile(x, ky.wrapping_add(2)),
                                tile(x, ky.wrapping_add(3)),
                            )
                        } else {
                            (
                                tile(kx.wrapping_sub(1), y),
                                tile(kx.wrapping_add(2), y),
                                tile(kx.wrapping_add(3), y),
                            )
                        };
                        let z1 = pick(
                            self.t.height[p3] as i32,
                            self.t.height[p4] as i32,
                            self.t.height[p5] as i32,
                        );
                        let z2 = (self.t.height[t] as i32 - s).max(z1);
                        if z2 < self.t.height[t] as i32 {
                            self.t.height[t] = z2 as u8;
                            done = false;
                        }
                        let z3 = pick(
                            self.t.ceiling[p3] as i32,
                            self.t.ceiling[p4] as i32,
                            self.t.ceiling[p5] as i32,
                        );
                        let z4 = (s + self.t.ceiling[t] as i32).min(z3);
                        if (self.t.ceiling[t] as i32) < z4 {
                            self.t.ceiling[t] = z4 as u8;
                            done = false;
                        }
                        if self.t.ceiling[t] > self.t.height[t] {
                            self.t.angle[t] &= !8;
                        } else {
                            self.t.angle[t] |= 8;
                        }
                        x = x.wrapping_add(1);
                    }
                    y = y.wrapping_add(1);
                }
                if done {
                    self.ent[i].act_life = 4; // removed + EndLoop 47
                }
                !done
            }
        }
    }

    /// The floor-write primitive on the cave sculptor paths —
    /// `sub_570F0` with `protectAngle = 0`: clamp, write, force the
    /// walkable nibble for the inner core (`a5`) or auto-flat types,
    /// with (`a6`) or without the h==0 water-seal edge walk. The
    /// dome/pit path (a6=1) is `mc2_dome_write_height` (morph.rs);
    /// the mesa calls with a6=0 — no edge walk, but the h==0 nibble
    /// clear is then UNCONDITIONAL (EF:39660 `goto LABEL_32`), and
    /// the per-cell retile always fires: `sub_570F0` keys it on a4
    /// (protectAngle), not a6 — a4=0 ⇒ `AddBuildingToTerrain_46570`
    /// every call (EF:39702-08).
    fn cave_write_floor(&mut self, x: u8, y: u8, h: i32, force_flat: bool, edge: bool) {
        if edge {
            self.mc2_dome_write_height(x, y, h, force_flat);
        } else {
            let h = h.clamp(0, 255);
            let t = tile(x, y);
            self.t.height[t] = h as u8;
            if force_flat || super::morph::auto_flat(self.t.tile_type[t]) {
                self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
            }
            if h == 0 {
                self.t.angle[t] &= 0xF0;
            }
            self.mc2_add_building_region(x, y, x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes, tile};
    use crate::verbs::VerbSet;

    /// Flat 100-floor / 120-ceiling cave world.
    fn cave_gen() -> Gen {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: vec![120; 0x10000],
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

    /// The a6=0 floor write (the mesa's form of `sub_570F0`) always
    /// retiles the cell (a4=0 ⇒
    /// `AddBuildingToTerrain_46570`, EF:39702-08) and clears the whole
    /// low nibble on a floor carved to 0 (EF:39660).
    #[test]
    fn mesa_floor_write_retiles_and_clears_h0_nibble() {
        let mut g = cave_gen();
        let t = tile(50, 50);
        g.t.tile_type[t] = 77; // poison — the retile must recompute it
        g.cave_write_floor(50, 50, 30, false, false);
        assert_ne!(g.t.tile_type[t], 77, "a4=0 always retiles");
        g.cave_write_floor(50, 50, 0, false, false);
        assert_eq!(g.t.angle[t] & 0x0F, 0, "h==0 clears the low nibble");
    }

    /// The dome's per-cell seal sync is bit3-ONLY (EF:25522-25) —
    /// unlike pit/hill/mesa/tube it never pins ceiling = floor−1.
    #[test]
    fn dome_sync_never_pins_the_ceiling() {
        let mut g = cave_gen();
        let i = g.new_event().expect("dome slot");
        {
            let e = &mut g.ent[i];
            e.x = 50 << 8;
            e.y = 50 << 8;
            e.dest_x = 3; // radius, tiles
            e.f71 = 1; // phase: animate
            e.z = 100; // sampled box MIN floor
            e.site_z = 120; // sampled box MAX ceiling
            e.act_life = 16;
        }
        // A cell the dome does NOT floor-write (already above the
        // lift, so no retile chain runs — the retile's own cave arm
        // legitimately re-pins). The whole 3×3 sits above the lift:
        // a neighbour's retile shade-pass would also reach this cell.
        for gy in 49..=51u8 {
            for gx in 49..=51u8 {
                g.t.height[tile(gx, gy)] = 140;
            }
        }
        let t = tile(50, 50);
        g.t.ceiling[t] = 90; // stale seal: ceiling <= floor
        g.mc2_cave_dome_tick(i);
        assert_eq!(g.t.height[t], 140, "no floor write on this cell");
        assert_eq!(g.t.ceiling[t], 90, "sync-only: no floor-1 pin");
        assert_ne!(g.t.angle[t] & 8, 0, "sealed cell flagged");
    }

    /// The tube's wall ring is `sub_34B00(ox-1, oy-1, side+1,
    /// side+1)` (EF:25243) — the +1 dims reach the far row/column.
    #[test]
    fn tube_wall_ring_covers_the_far_corner() {
        let mut g = cave_gen();
        for t in 0..0x10000 {
            g.t.ceiling[t] = 90; // sealed everywhere
            g.t.angle[t] |= 8;
        }
        let i = g.new_event().expect("tube slot");
        {
            let e = &mut g.ent[i];
            e.x = 50 << 8;
            e.y = 50 << 8;
            e.dest_x = (50 << 8) + 90; // one 85-unit step
            e.dest_y = 50 << 8;
            e.f71 = 0; // radius 512 → 512
        }
        g.mc2_tube_carve_tick(i);
        // Ring box (47,47,5,5): the right column x=52 — outside the
        // (47,47,4,4) box — is wall-stamped (thin stamp)…
        assert_eq!(g.t.angle[tile(52, 47)] & 7, 1, "far column stamped");
        // …but the SE corner is structurally NEVER visited.
        assert_eq!(g.t.angle[tile(52, 52)] & 7, 5, "SE corner untouched");
    }
}
