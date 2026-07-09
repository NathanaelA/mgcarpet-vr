//! MC2 class-10 TAIL EFFECTS, Phase 4.3 — the small-count effect
//! band: (10,52) castle anchor, (10,25)/(10,23) one-shot blasts,
//! (10,17) meteor, (10,15) fire trail + its (10,11→19) ground-fire
//! spray, (10,54) proximity aura. Trace bank:
//! docs/traces/mc2-class10-m50-chains-and-tail.md (§3-§7) +
//! mc2-class10-m6-m9-m11-m28-m31.md (§3, the 11→19 remap)
//! (`EF:` = remc2 EventsFunctions.cpp).
//!
//! Entity-field homes follow the class-10 effect column: subSpell
//! (the area amount) → f140, `dword_0x10_16` scratch → f26,
//! `byte_0x46_70` → f71, `word_0x26_38` → f40.
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - `sub_6D8B0(id, kind, hits)` spellbook reports ((10,17) kind 9,
//!   (10,23) kind 7, (10,15)'s spray kind — the spell-XP intake)
//!   land with Phase 4.2; the hit counts are computed and dropped.
//! - The (10,19) spray's `word_0x33` singleton latch (EF:23962
//!   registers a new spray and disables the previous one from a
//!   DIFFERENT action's context) has no ported writer; the release
//!   write on death is a no-op without it (trace OPEN-6).
//! - `AddEvent2_847D0` attached lights/children ((10,23)'s
//!   (128,9,0)) are presentation, unported (the (10,1) note).
//! - The (10,54) aura scans retail's `dword_38523` creature list —
//!   our pool slot-order scan is the mobs.rs list APPROX.

use super::sprite_params::SPRITE_PARAMS;
use crate::mc1::combat::MailTarget;
use crate::mc1::features::Gen;
use crate::mc1::mobs::MobCtx;

/// The whirlwind's victim GRAB latch (retail byte[3] & 0x10, dword
/// 0x1000_0000) — a free high bit next to the mobs.rs MC2 band.
pub(crate) const F_GRABBED: u32 = 1 << 29;

impl Gen {
    // ---- ctors ---------------------------------------------------------------

    /// `sub_50430` (EF:36772) — the (10,52) permanent CASTLE/BUILDING
    /// ANCHOR: sprite 205, maxLife 100000 (effectively immortal),
    /// subSpell 500, a 500/2000 mana pool, untargetable. Its action
    /// 0x38 is an EMPTY EV case (EV:2693) — the entity ticks nothing,
    /// which the class-10 dispatch's fall-through arm already is.
    /// maxMana (2000) has no ported home or reader until the MC2
    /// building economy pass — the mana pool rides f140's mana home.
    pub(crate) fn mc2_spawn_castle_anchor(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 52;
            e.tick70 = 0x38;
            e.max_life = 100000;
            e.f140 = 500; // mana_0x90_144 (subSpell 500 shares the value)
            e.f26 = 600;
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 205);
        Some(i)
    }

    /// `sub_4F6A0` (EF:36110) — the (10,25) one-shot area blast,
    /// damage TYPE 3: maxLife 8, subSpell 2000 (set but the burst
    /// amount is `byte_0x46_70` — par-set by the caster), byte[0] =
    /// (&0xF6)|1, map-registered, extents 512. No sprite, no RNG.
    pub(crate) fn mc2_spawn_blast25(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 25;
            e.tick70 = 0x19;
            e.max_life = 8;
            e.f140 = 2000;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `sub_4F5F0` (EF:36087) — the (10,23) one-shot area blast,
    /// type 0 amount 25: sprite 7, extents 200, the fire-ctor flag
    /// pattern + bit 0, sound 24 on the burst. The attached
    /// `AddEvent2_847D0(128, 9, 0)` child is presentation, skipped.
    pub(crate) fn mc2_spawn_blast23(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 23;
            e.tick70 = 0x17;
            e.max_life = 8;
            e.f140 = 25;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 7);
        self.mc2_shift_rot(i, 200, 200);
        self.ent[i].flags |= 1;
        Some(i)
    }

    /// `AddMeteor_4ED70` (EF:35731) — the (10,17) METEOR impact:
    /// maxLife 10, subSpell 3000, untargetable, NOT map-registered,
    /// no sprite of its own (the tick grows the quad). No RNG.
    pub(crate) fn mc2_spawn_meteor(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 17;
            e.tick70 = 17;
            e.max_life = 10;
            e.f140 = 3000;
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        Some(i)
    }

    /// `sub_4ECD0` (EF:35707) — the (10,15) wandering FIRE TRAIL:
    /// maxLife 128, actSpeed 256, subSpell 100, ONE RNG draw for the
    /// random yaw, extents (1024, 0x4000). Not map-registered.
    pub(crate) fn mc2_spawn_fire_trail(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 15;
            e.tick70 = 15;
            e.max_life = 128;
            e.f126 = 256; // actSpeed
            e.flags &= !8;
            e.f140 = 100;
            e.f26 = 0;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        let d = self.mc2_rand(i);
        self.ent[i].f30 = (d & 0x7FF) as u16;
        self.refill_life(i);
        self.mc2_shift_rot(i, 1024, 0x4000);
        Some(i)
    }

    /// `NewAdd0A0B_4E840` (EF:35553) — the (10,11) GROUND-FIRE-SPRAY
    /// creator, which REMAPS the entity to model/action 19 (0x13) —
    /// a (10,11) THING IS a (10,19) entity (the m6-doc §0 numbering
    /// trap; never port 11 as a distinct model). Sprite 228 (the
    /// fire family), maxLife 240, subSpell 200, map-registered,
    /// byte[0] bit0 set / bit3 clear. No RNG.
    pub(crate) fn mc2_spawn_fire_spray(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 19;
            e.tick70 = 19;
            e.f140 = 200;
            e.max_life = 240;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.ent[i].flags |= 1;
        self.refill_life(i);
        self.mc2_set_sprite(i, 228);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `AddAuxiliary_50500` (EF:36812) — the (10,54) proximity AURA
    /// field: invisible, life 128, ONE RNG draw (random yaw),
    /// `dword_0x10_16 = 12845056` (0xC40000 — the SQUARED range),
    /// extents (1024, 0x4000). Not map-registered.
    pub(crate) fn mc2_spawn_aura(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 54;
            e.tick70 = 0x3B;
            e.max_life = 128;
            e.f126 = 256;
            e.flags &= !8;
            e.f140 = 100;
        }
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            e.f26 = 0; // dword_0x10_16 is i16-homed; the squared range is a const below
            e.f30 = (d & 0x7FF) as u16;
            e.x = x;
            e.y = y;
            e.z = z;
            e.flags |= 1;
        }
        self.refill_life(i);
        self.mc2_shift_rot(i, 1024, 0x4000);
        Some(i)
    }

    /// `AddWind_4F040` (EF:35852) + `sub_4F1C0` (EF:35921) — the
    /// (10,22) WHIRLWIND: gated on >= 12 free slots; the head (ONE
    /// RNG draw seeds roll = yaw = pitch) plus 11 tail nodes
    /// (model 75, action 82 — an EV no-op, the head drags them)
    /// chained via word_0x32/word_0x34 (f52/f54), then the sprite
    /// stack: per node row 293+index, quad (550/450 per-mille of the
    /// row's rot_speed), z stacked by 2*roll-extent with the node's
    /// offset in the column scratch f50 (`word_0x36_54`).
    ///
    /// Column scratch f50: head = remembered eye z
    /// (`word_0x30_48`), nodes = the z-stack offset
    /// (`word_0x36_54`), victims = the swirl yaw (`word_0x30_48`) —
    /// disjoint entity sets, one home.
    pub(crate) fn mc2_spawn_whirlwind(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.free.len() < 12 {
            return None;
        }
        let h = self.new_event()?;
        {
            let e = &mut self.ent[h];
            e.class64 = 10;
            e.model65 = 22;
            e.tick70 = 22;
            e.f44 = 0;
            e.f46 = 1; // word_0x2E_46 — the lateral drift sign
            e.f128 = 20;
            e.f130 = 10;
            e.f126 = 50;
            e.max_life = 500;
            e.f140 = 1000; // subSpellIndex — the damage magnitude
            e.flags &= !8;
            e.f56 = 1; // byte_0x38_56 (ch0 enrolment; untargetable anyway)
            e.x = x;
            e.y = y;
            e.z = z;
        }
        let d = self.mc2_rand(h);
        {
            let e = &mut self.ent[h];
            e.f34 = ((d & 0x7FF) as u16).wrapping_sub(1) & 0x7FF; // roll
            e.f30 = e.f34; // yaw
            e.f32 = e.f34; // pitch
        }
        self.refill_life(h);
        let (hx, hy, hz) = (x, y, z);
        let mut prev = h;
        for i in 0..11u16 {
            let Some(c) = self.new_event() else { break };
            // qmemcpy(child, head, 0xA8) — the gameplay fields the
            // node machinery reads, id included (nodes share the
            // head's id).
            {
                let (head_id, head_life, head_rand) =
                    { (self.ent[h].id24, self.ent[h].act_life, self.ent[h].rand) };
                let e = &mut self.ent[c];
                e.class64 = 10;
                e.model65 = 75;
                e.tick70 = 82;
                e.max_life = 500;
                e.act_life = head_life;
                e.id24 = head_id;
                e.rand = head_rand;
                e.flags &= !8;
                e.f44 = i + 1; // word_0x2C_44 — the node index
                e.f52 = prev as u16;
                e.f54 = 0;
                e.f63 = i as u8;
                e.x = hx;
                e.y = hy;
                e.z = hz;
            }
            self.ent[prev].f54 = c as u16;
            self.link(c, hx, hy, hz);
            prev = c;
        }
        self.link(h, hx, hy, hz);
        // sub_4F1C0 — the stacked sprite column.
        let ground = self.ground_z(hx, hy) as i16;
        let mut zoff = 0i32;
        let mut n = h;
        loop {
            let row = self.ent[n].f44 as usize + 293;
            let v5 = SPRITE_PARAMS[row].rot_speed_8 as i32;
            self.mc2_set_sprite(n, row as u16);
            let (shift, roll_ext) = ((550 * v5 / 1000) as u16, (450 * v5 / 1000) as i32);
            self.mc2_shift_rot(n, shift, roll_ext as u16);
            self.ent[n].z = (zoff as i16).wrapping_add(ground);
            self.ent[n].f50 = zoff as i16; // word_0x36_54
            zoff += 2 * roll_ext;
            let next = self.ent[n].f54 as usize;
            if next == 0 {
                break;
            }
            n = next;
        }
        Some(h)
    }

    /// `sub_51790` (EF:37439) — the (10,71) expanding FISSURE:
    /// life = maxLife = 120, subSpell 20000, byte[0] = (&0xF6)|1,
    /// map-registered, extents (1280, 2048). No sprite, no RNG.
    pub(crate) fn mc2_spawn_fissure(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 71;
            e.tick70 = 0x4E;
            e.max_life = 120;
            e.act_life = 120;
            e.f140 = 20000;
            e.f71 = 0;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.mc2_shift_rot(i, 1280, 2048);
        Some(i)
    }

    /// `AddFireSpheres_4F2A0` (EF:35936) + `sub_4F440` (EF:35989) —
    /// the (10,76) orbiting FIRE-SPHERE ORB
    /// (docs/traces/mc2-class10-m76-fire-spheres.md): gated on >= 26
    /// free slots; one invisible hub (maxLife 80, subSpell 70,
    /// extents 640, action 0x53) + 25 sprite-340 satellites (model
    /// 77, action 0x54 = NO handler — the hub repositions them)
    /// chained via f52/f54, laid out as a 5-ring x 5-slot spherical
    /// lattice (ONE RNG draw per satellite = the 84..147 spin rate).
    /// Only the 5 slot-0 spheres are targetable damage-carriers; the
    /// other 20 are visuals (byte[2] bit7 render flag). The
    /// satellites' `AddEvent2(128,1,0)` children are presentation,
    /// skipped. Runtime-disposition-only in retail (no generate
    /// pass, no par consumption).
    pub(crate) fn mc2_spawn_fire_orb(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.free.len() < 26 {
            return None;
        }
        let h = self.new_event()?;
        {
            let e = &mut self.ent[h];
            e.class64 = 10;
            e.model65 = 76;
            e.tick70 = 0x53;
            e.max_life = 80;
            e.f140 = 70; // subSpellIndex — the per-sphere damage
            e.f126 = 40; // actSpeed
            e.f130 = 192; // maxSpeed — breathe bound A
            e.f128 = 480; // minSpeed — breathe bound B
            e.f56 = 1;
            e.f68 = 0;
            e.f69 = 0;
            e.f44 = 0; // current ring radius
            e.f46 = 0; // fontTypeIndex_0x3D_61 — the breathe step
            e.f71 = 0; // byte_0x46_70 — the phase machine
            e.flags = (e.flags & !0x9) | 1;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(h);
        let mut prev = h;
        for i in 0..25u8 {
            let Some(s) = self.new_event() else { break };
            {
                let (id, rand, life) = {
                    let e = &self.ent[h];
                    (e.id24, e.rand, e.act_life)
                };
                let e = &mut self.ent[s];
                e.class64 = 10;
                e.model65 = 77;
                e.tick70 = 0x54;
                e.max_life = 80;
                e.act_life = life;
                e.id24 = id;
                e.rand = rand;
                e.f140 = 70;
                e.f56 = 1;
                e.flags = (e.flags & !0x9) | 1;
                e.f52 = prev as u16;
                e.f54 = 0;
                e.f63 = i;
                e.f68 = i / 5; // ring
                e.f69 = i % 5; // slot
                e.x = x;
                e.y = y;
                e.z = z;
            }
            self.ent[prev].f54 = s as u16;
            self.link(s, x, y, z);
            prev = s;
        }
        self.link(h, x, y, z);
        self.mc2_shift_rot(h, 640, 640);
        // sub_4F440 — the ring layout.
        {
            let e = &mut self.ent[h];
            e.f46 = 18; // breathe step
            e.f44 = e.f130 as u16; // radius := maxSpeed (192)
            e.f30 = 0;
            e.f32 = 0;
        }
        let mut n = self.ent[h].f54 as usize;
        while n != 0 {
            let slot = self.ent[n].f69;
            self.ent[n].flags &= !1;
            if slot != 0 {
                self.ent[n].flags = (self.ent[n].flags | 0x80_0000) & !8;
            } else {
                self.ent[n].flags |= 8; // the damage carriers
            }
            let d = self.mc2_rand(n);
            let spin = ((d & 0x3F) + 84) as u16;
            let ring = self.ent[n].f68;
            let (yaw, pitch, roll_spin, fov_spin) = match ring {
                0 => ((512 - 96 * slot as i32) as u16 & 0x7FF, 0u16, spin, 0u16),
                1 => (512, (512 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
                2 => (0, (-96 * slot as i32) as u16 & 0x7FF, 0, spin),
                3 => (256, (256 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
                _ => (768, (768 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
            };
            {
                let e = &mut self.ent[n];
                e.f30 = yaw;
                e.f32 = pitch;
                e.f34 = roll_spin;
                e.f36 = fov_spin;
            }
            let radius = self.ent[h].f44 as i16;
            let mut pos = (x, y, z);
            Self::polar_step(&mut pos, yaw, pitch, radius);
            self.move_relink(n, pos.0, pos.1, pos.2);
            self.mc2_set_sprite(n, 340);
            n = self.ent[n].f54 as usize;
        }
        Some(h)
    }

    // ---- ticks ---------------------------------------------------------------

    /// `sub_339B0` (EF:24562) — the orb hub tick: phase 0 init (the
    /// leader arm is dead code for model 76 — word_0x96_150 has no
    /// writer, trace §2) → phase 1 pulse: terrain clamp
    /// (z >= ground + radius, `sub_33C70`), the ±18 radius breathe
    /// bouncing across [192,480] (`sub_33AD0`), the constellation
    /// tumble (+22/+16 head spin, per-sphere spin, all 25
    /// repositioned — `sub_33B20`), the slot-0 damage pass
    /// (`sub_10C80(type 0, 70)` per carrier, sound 3 on any hit —
    /// `sub_33C00`); life out → phase 2 collapse: keep tumbling,
    /// radius -= |step|, and at < 0 spawn a (10,0) ground fire and
    /// tear the whole 26-entity chain down (`sub_33D40`).
    pub(crate) fn mc2_fire_orb_tick(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].f71 == 0 {
            self.ent[i].f71 = 1;
        } else if self.ent[i].f71 > 1 {
            if self.ent[i].f71 == 2 {
                if self.ent[i].f46 < 0 {
                    self.ent[i].f46 = -self.ent[i].f46;
                }
                self.mc2_orb_tumble(i);
                let v7 = self.ent[i].f44 as i16 - self.ent[i].f46;
                self.ent[i].f44 = v7 as u16;
                if v7 < 0 {
                    let (x, y, z) = {
                        let e = &self.ent[i];
                        (e.x, e.y, e.z)
                    };
                    self.mc2_spawn_fire(x, y, z);
                    let mut n = i;
                    loop {
                        self.ent[n].flags |= 0x400;
                        let next = self.ent[n].f54 as usize;
                        if next == 0 || next == n {
                            break;
                        }
                        n = next;
                    }
                }
            }
            return;
        }
        // Phase 1: terrain clamp (leader arm dead for model 76).
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let floor = (self.ground_z(x, y) as i16).wrapping_add(self.ent[i].f44 as i16);
        if self.ent[i].z < floor {
            self.ent[i].z = floor;
        }
        // sub_33AD0 — the breathe bounce.
        {
            let e = &mut self.ent[i];
            let v2 = e.f46 + e.f44 as i16;
            let (lo, hi) = (e.f128 as i16, e.f130 as i16);
            e.f44 = v2 as u16;
            if v2 <= lo {
                if v2 < hi {
                    e.f44 = hi as u16;
                    e.f46 = -e.f46;
                }
            } else {
                e.f44 = lo as u16;
                e.f46 = -e.f46;
            }
        }
        self.mc2_orb_tumble(i);
        // sub_33C00 — the slot-0 damage pass.
        let amt = self.ent[i].f140 as u32;
        let mut n = self.ent[i].f54 as usize;
        let mut hit = false;
        while n != 0 {
            if self.ent[n].f69 == 0 {
                hit |= self.area_write(n, 0, amt, ctx, false, false) != 0;
            }
            n = self.ent[n].f54 as usize;
        }
        if hit {
            self.snd(3, i);
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 1 {
            self.ent[i].f71 = 2;
        }
    }

    /// `sub_33B20` (EF:24656) — the constellation tumble: the hub
    /// spins +22 yaw / +16 pitch, each satellite advances its own
    /// spin rates, and every sphere is re-placed at hub + spherical
    /// (satAngle + hubAngle, radius). No RNG.
    fn mc2_orb_tumble(&mut self, i: usize) {
        {
            let e = &mut self.ent[i];
            e.f30 = e.f30.wrapping_add(22) & 0x7FF;
            e.f32 = e.f32.wrapping_add(16) & 0x7FF;
        }
        let (hx, hy, hz, hyaw, hpitch, radius) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.f44 as i16)
        };
        let mut n = self.ent[i].f54 as usize;
        while n != 0 {
            {
                let e = &mut self.ent[n];
                e.f30 = e.f30.wrapping_add(e.f34) & 0x7FF;
                e.f32 = e.f32.wrapping_add(e.f36) & 0x7FF;
            }
            let (syaw, spitch) = (self.ent[n].f30, self.ent[n].f32);
            let mut pos = (hx, hy, hz);
            Self::polar_step(
                &mut pos,
                syaw.wrapping_add(hyaw) & 0x7FF,
                spitch.wrapping_add(hpitch) & 0x7FF,
                radius,
            );
            self.move_relink(n, pos.0, pos.1, pos.2);
            n = self.ent[n].f54 as usize;
        }
    }

    /// `sub_3A2D0` (EF:29443) — the (10,71) fissure tick
    /// (docs/traces/mc2-class10-tail-helper-closure.md §2): phase 0
    /// init (`word_0x2C_44 = maxLife/8`, per-beat damage =
    /// 4*(20000/120) ≈ 664); each tick the disc radius ramps
    /// grow → pin-at-3*ref (with a 1-in-5 phase-jump roll) → shrink,
    /// clamped [0,15], and every cell of the disc takes a **±1
    /// heightmap jitter** (sign = life & 1 — the ground vibrates; no
    /// terrain-type write, no children); a `byte_0x46_70 > 1` tick
    /// adds a half-radius inner pass; `byte > 3` = the terminal
    /// tail-off (life only). Every 4th tick: sprite quad grows to
    /// the radius, sound 10, the type-0 area beat (the id-0xF
    /// spellbook report banks with 4.2).
    pub(crate) fn mc2_fissure_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].f71 == 0 {
            let maxl = self.ent[i].max_life as i32;
            self.ent[i].f44 = (maxl >> 3) as u16; // word_0x2C_44
            self.ent[i].f26 = 0;
            self.ent[i].f71 = 1;
            self.ent[i].f140 = 4 * (self.ent[i].f140 / maxl.max(1) as i32);
        }
        let mut dirty = false;
        if self.ent[i].f71 <= 3 {
            let v4 = self.ent[i].f44 as i32;
            let maxl = self.ent[i].max_life as i32;
            let life = self.ent[i].act_life;
            let mut v6 = if maxl - 3 * v4 >= life as i32 {
                if maxl - 5 * v4 > life as i32 {
                    self.ent[i].f26 -= 1;
                    self.ent[i].f26 as i32
                } else {
                    let d = self.mc2_rand(i);
                    if d % 5 == 0 {
                        self.ent[i].f71 += 2;
                    }
                    3 * v4
                }
            } else {
                self.ent[i].f26 += 1;
                self.ent[i].f26 as i32
            };
            v6 = v6.clamp(0, 3 * v4).clamp(0, 15);
            let second_pass = self.ent[i].f71 > 1;
            if second_pass {
                self.ent[i].f71 -= 1;
            }
            if v6 > 0 {
                let (cx, cy) = ((self.ent[i].x >> 8) as i16, (self.ent[i].y >> 8) as i16);
                let sign: i16 = if self.ent[i].act_life & 1 == 1 { 1 } else { -1 };
                for r in [Some(v6), second_pass.then_some(v6 >> 1)]
                    .into_iter()
                    .flatten()
                {
                    for (dx, dy) in self.ring_cells(0, r) {
                        let t = crate::mc1::features::tile(
                            (cx.wrapping_add((dx as i8) as i16)) as u8,
                            (cy.wrapping_add((dy as i8) as i16)) as u8,
                        );
                        let v = (self.t.height[t] as i16 + sign).clamp(0, 255);
                        self.t.height[t] = v as u8;
                    }
                }
                dirty = true;
                if self.ent[i].act_life & 3 == 0 {
                    self.mc2_shift_rot(i, (v6 << 8) as u16, 2048);
                    self.snd(10, i);
                    let amt = self.ent[i].f140 as u32;
                    let _hits = self.area_write(i, 0, amt, ctx, false, false);
                }
            }
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
        }
        dirty
    }

    /// `sub_33110` (EF:24155) — the whirlwind driver: while alive,
    /// wander + drag (`sub_331A0`), the lift-and-throw pass
    /// (`sub_33340`), the every-8th-tick contact pass (`sub_33710`),
    /// loop sound 49; on expiry the teardown (`sub_338D0`) clears
    /// the grabs and despawns the 12-node chain.
    pub(crate) fn mc2_whirlwind_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.mc2_whirlwind_teardown(i);
            return;
        }
        self.mc2_whirlwind_move(i);
        self.mc2_whirlwind_lift(i, ctx);
        self.mc2_whirlwind_contact(i);
        self.snd(49, i);
    }

    /// `sub_331A0` (EF:24177) — head wander (roll drift flips sign
    /// on a coin every 16 ticks, 32-unit lateral wobble → the eye
    /// center, +341 yaw and 120 forward, ground-clamped) + the tail
    /// drag (each node pulled toward its predecessor to the gap
    /// `72 - 4*(12 - index)`, z = head z + the node's f50 offset).
    /// The eye xy rides f142/f144-free scratch: we keep it in the
    /// head's dest fields (the portal column's home, unused here
    /// otherwise) — `axis_0x9A_154x`.
    fn mc2_whirlwind_move(&mut self, i: usize) {
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        self.ent[i].f50 = z; // word_0x30_48 — remembered eye z
        self.ent[i].f63 = self.ent[i].f63.wrapping_add(1);
        if self.ent[i].f63 & 0xF == 0 {
            let d = self.mc2_rand(i);
            if d & 1 == 0 {
                self.ent[i].f46 = -self.ent[i].f46;
            }
        }
        let roll = (self.ent[i].f34 as i32 + 11 * self.ent[i].f46 as i32) as u16 & 0x7FF;
        self.ent[i].f34 = roll;
        let mut eye = (x, y, z);
        Self::polar_step(&mut eye, roll, 0, 32);
        self.ent[i].dest_x = eye.0;
        self.ent[i].dest_y = eye.1;
        let yaw = self.ent[i].f30.wrapping_add(341) & 0x7FF;
        self.ent[i].f30 = yaw;
        let mut pos = eye;
        Self::polar_step(&mut pos, yaw, 0, 120);
        let ground = self.ground_z(pos.0, pos.1) as i16;
        self.move_relink(i, pos.0, pos.1, ground);
        // Tail drag.
        let head_z = ground;
        let mut prev = i;
        let mut n = self.ent[i].f54 as usize;
        while n != 0 {
            let (nx, ny, nz) = {
                let e = &self.ent[n];
                (e.x, e.y, e.z)
            };
            let (px, py, pz) = {
                let e = &self.ent[prev];
                (e.x, e.y, e.z)
            };
            let yaw = Self::angle_between(nx, ny, px, py);
            self.ent[n].f30 = yaw;
            let dh2 = Self::dist2_sq(nx, ny, px, py);
            let dz = (nz as i32 - pz as i32).unsigned_abs();
            let d = Self::isqrt((dh2 as u32).wrapping_add(dz * dz)) as i32;
            let gap = 72 - 4 * (12 - self.ent[n].f44 as i32);
            let mut pos = (nx, ny, nz);
            if d > gap {
                Self::polar_step(&mut pos, yaw, 0, (d - gap) as i16);
            }
            let zoff = self.ent[n].f50;
            pos.2 = zoff.wrapping_add(head_z);
            self.move_relink(n, pos.0, pos.1, pos.2);
            prev = n;
            n = self.ent[n].f54 as usize;
        }
    }

    /// `sub_33340` (EF:24229) — the lift-and-throw pass over the
    /// radius-12 tile disc around the eye: pool CREATURES swirl
    /// inward (yaw = bearing+591, drift 96), lift near the eye
    /// (+114/tick above it, GRAB latched past the 768+rand%768
    /// threshold), spin at yaw-step 204 while grabbed, release past
    /// the far ring (d² >= 5308416), and take the head's 1000
    /// mailbox damage every airborne tick (`sub_11900`). The
    /// spellbook report (id 0x15) banks with 4.2.
    ///
    /// APPROX register (cited, the 4.3b grind refines):
    /// - the HUMAN player arm (yaw-step 56, threshold 384, camera
    ///   roll crank, actSpeed 80) needs the FlightVerb takeover seam
    ///   (the level-end cinematic's seam) — until then the player is
    ///   damaged when overlapping the eye ring but not lifted;
    /// - `sub_33810`'s grab filter is unread — creatures gate on
    ///   targetable + ch0-enrolled (flags&8 + f28 bit 0), the
    ///   observable superset of the gloss;
    /// - the victim z-float band (`sub_580E0` row args) collapses to
    ///   the computed lift z (the row hover clamp needs the behavior
    ///   rows' word_0xa/0xc homes).
    fn mc2_whirlwind_lift(&mut self, i: usize, ctx: &MobCtx) {
        let (ex, ey, eye_z, id, amt) = {
            let e = &self.ent[i];
            (e.dest_x, e.dest_y, e.f50, e.id24, e.f140 as u32)
        };
        let (cx, cy) = ((self.ent[i].x >> 8) as i16, (self.ent[i].y >> 8) as i16);
        let mut hits = 0u32;
        for (dx, dy) in self.ring_cells(0, 12) {
            let tx = (cx.wrapping_add((dx as i8) as i16)) as u8;
            let ty = (cy.wrapping_add((dy as i8) as i16)) as u8;
            let mut j = self.map_entity[crate::mc1::features::tile(tx, ty)] as usize;
            while j != 0 {
                let next = self.ent[j].next20 as usize;
                let c = &self.ent[j];
                if c.class64 != 5
                    || c.flags & 8 == 0
                    || c.f28 & 1 == 0
                    || c.id24 == id
                    || c.flags & 0x400 != 0
                {
                    j = next;
                    continue;
                }
                let d2 = Self::dist2_sq(ex, ey, c.x, c.y) as i64;
                let grabbed = c.flags & F_GRABBED != 0;
                let (vx, vy, vz) = (c.x, c.y, c.z);
                let mut pos = (vx, vy, vz);
                let mut drift = 0i16;
                let mut airborne = false;
                if d2 >= 3_211_264 {
                    if grabbed {
                        self.ent[j].flags |= super::mobs::F_STOP;
                        airborne = true;
                        drift = 64;
                        self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                        if d2 >= 5_308_416 {
                            self.ent[j].flags &= !F_GRABBED; // FLUNG
                        }
                    }
                } else {
                    let bearing = Self::angle_between(ex, ey, vx, vy);
                    if grabbed {
                        self.ent[j].flags |= super::mobs::F_STOP;
                        drift = 128;
                        airborne = true;
                        pos.2 = pos.2.wrapping_add(114);
                        self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                    } else if d2 >= 0x40000 {
                        // Mid ring: swirl inward.
                        let v14 = bearing.wrapping_add(591) & 0x7FF;
                        self.ent[j].f50 = v14 as i16;
                        self.ent[j].f30 = v14;
                        drift = 96;
                    } else {
                        // Inner ring: the lift.
                        self.ent[j].flags |= super::mobs::F_STOP;
                        pos.0 = ex;
                        pos.1 = ey;
                        let v9 = vz as i32 - eye_z as i32 + 57;
                        let galt = self.ground_z(ex, ey) as i16;
                        pos.2 = ((v9 + galt as i32).max(galt as i32)) as i16;
                        self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                        let d = self.ent_rand(j);
                        if v9 >= 768 + (d % 768) as i32 {
                            self.ent[j].flags |= F_GRABBED;
                            self.ent[j].f50 = self.ent[j].f30 as i16;
                        }
                    }
                }
                if drift != 0 {
                    let swirl = self.ent[j].f50 as u16 & 0x7FF;
                    Self::polar_step(&mut pos, swirl, 0, drift);
                }
                if pos != (vx, vy, vz) {
                    self.move_relink(j, pos.0, pos.1, pos.2);
                }
                if airborne {
                    hits += 1;
                    self.mail_write(MailTarget::Pool(j), 0, amt, id);
                }
                j = next;
            }
        }
        // The player arm — damage on eye-ring overlap (lift APPROX-
        // banked on the flight takeover seam).
        let pd2 = Self::dist2_sq(ex, ey, ctx.px, ctx.py) as i64;
        if pd2 < 0x40000 {
            self.mail_write(MailTarget::Player, 0, amt, id);
            hits += 1;
        }
        let _ = hits; // sub_6D8B0(id, 0x15, hits) — Phase 4.2
    }

    /// `sub_33710` (EF:24416) — the every-8th-tick CONTACT pass:
    /// overlapping castles take the 1000 mail + the 30-tick blast
    /// shake (retail writes word_0x30_48 = 30, our f50 shake home),
    /// and overlapping effect entities take the mail directly. The
    /// spellbook report banks with 4.2.
    fn mc2_whirlwind_contact(&mut self, i: usize) {
        if self.ent[i].f63 & 7 != 0 {
            return;
        }
        let (id, amt) = (self.ent[i].id24, self.ent[i].f140 as u32);
        let mut hits: Vec<(usize, bool)> = Vec::new();
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if j == i || c.flags & 0x400 != 0 {
                continue;
            }
            let castle = c.class64 == 3 && c.model65 == 2;
            let effect = c.class64 == 10 && c.flags & 8 != 0 && c.f28 & 1 != 0;
            if (castle || effect) && c.id24 != id && self.ent_overlap(i, j) {
                hits.push((j, castle));
            }
        }
        for (j, castle) in hits {
            if castle {
                self.ent[j].f50 = 30;
            }
            self.mail_write(MailTarget::Pool(j), 0, amt, id);
        }
    }

    /// `sub_338D0` (EF:24518) — teardown: clear every nearby
    /// victim's grab/stop latches over the radius-12 disc, end the
    /// wind loop (sound 49 stops with the emitter), despawn the head
    /// and all 11 nodes down the f54 chain.
    fn mc2_whirlwind_teardown(&mut self, i: usize) {
        let (cx, cy) = ((self.ent[i].x >> 8) as i16, (self.ent[i].y >> 8) as i16);
        for (dx, dy) in self.ring_cells(0, 12) {
            let tx = (cx.wrapping_add((dx as i8) as i16)) as u8;
            let ty = (cy.wrapping_add((dy as i8) as i16)) as u8;
            let mut j = self.map_entity[crate::mc1::features::tile(tx, ty)] as usize;
            while j != 0 {
                let next = self.ent[j].next20 as usize;
                self.ent[j].flags &= !(F_GRABBED | super::mobs::F_STOP);
                j = next;
            }
        }
        let mut n = i;
        loop {
            self.ent[n].flags |= 0x400;
            let next = self.ent[n].f54 as usize;
            if next == 0 || next == n {
                break;
            }
            n = next;
        }
    }

    /// `sub_33E20` (EF:24817) — the (10,25) tick: life-- /f26++;
    /// while alive, ONE latched `sub_10C80(type 3, byte_0x46_70)`
    /// burst (the amount is the par-set f71, NOT subSpell); a hit
    /// zeroes life (despawn next tick).
    pub(crate) fn mc2_blast25_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].f26 += 1;
        self.ent[i].act_life = life;
        if life >= 0 {
            if self.ent[i].flags & 2 == 0 {
                self.ent[i].flags |= 2;
                let amt = self.ent[i].f71 as u32;
                if self.area_write(i, 3, amt, ctx, false, false) != 0 {
                    self.ent[i].act_life = 0;
                }
            }
        } else {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_33D80` (EF:24787) — the (10,23) tick: ONE latched
    /// `sub_10C80(type 0, 25)` burst + sound 24, then life pinned to
    /// 1 (one more visible tick). The `sub_6D8B0(id, 7, hits)`
    /// spellbook report banks with 4.2.
    pub(crate) fn mc2_blast23_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].act_life = life;
        if life >= 0 {
            if self.ent[i].flags & 2 == 0 {
                let amt = self.ent[i].f140 as u32;
                let _hits = self.area_write(i, 0, amt, ctx, false, false);
                self.snd(24, i);
                self.ent[i].act_life = 1;
                self.ent[i].flags |= 2;
            }
        } else {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_32880` (EF:23834) — the (10,17) meteor tick: sound 30 +
    /// the once-latch (dword |= 0x10002) on the first tick; the quad
    /// grows with the ring counter (`ShiftRot((768*f26 - 5*sign)>>2,
    /// 512)`); `sub_10C80(type 0, subSpell/maxLife)` = 300/tick (the
    /// kind-9 spellbook report banks with 4.2); then ONE RING of
    /// (10,0) fire children at ring f26 — jittered (2 RNG each, cell
    /// pitch 160), id+yaw inherited, `dword |= 0x10080` (byte[0]
    /// bit7 + byte[2] bit0 — the children are DAMAGE-SUPPRESSED
    /// visuals, the fire tick's 0x1_0000 gate), quad (512,512); the
    /// ring cycles `(f26+2) % 11`.
    pub(crate) fn mc2_meteor_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].act_life = life;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2 | 0x1_0000;
            self.snd(30, i);
        }
        let ring = self.ent[i].f26 as i32;
        let grown = 768 * ring;
        let shift = (grown - if grown > 0 { 5 } else { 0 }) >> 2;
        self.mc2_shift_rot(i, shift as u16, 512);
        let amt = (self.ent[i].f140 / self.ent[i].max_life as i32) as u32;
        let _hits = self.area_write(i, 0, amt, ctx, false, false);
        let (px, py, pz, id, yaw) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30)
        };
        for (dx, dy) in self.ring_cells(ring, ring) {
            let d = self.ent_rand(i);
            let nx = (px as i32 - 96 + 160 * (dx as i8) as i32 + (d % 0x81) as i32 - 64) as u16;
            let d = self.ent_rand(i);
            let ny = ((d % 0x81) as i32 + 160 * (dy as i8) as i32 + py as i32 - 96 - 64) as u16;
            if let Some(c) = self.mc2_spawn_fire(nx, ny, pz) {
                {
                    let e = &mut self.ent[c];
                    e.id24 = id;
                    e.f30 = yaw;
                    e.flags |= 0x1_0080;
                    e.f26 = 0;
                }
                self.mc2_shift_rot(c, 512, 512);
            }
        }
        self.ent[i].f26 = ((ring + 2) % 11) as i16;
    }

    /// `sub_32530` (EF:23694) — the (10,15) fire-trail tick: the
    /// water counter (`sub_104A0 & 1` → f26++, else --), death on
    /// life < -1 OR 8 accumulated water ticks; ONE RNG wander
    /// (yaw += r%0x5B - 45), advance 256, drop a (10,11→19) spray
    /// (fov copied, life 10, word_0x26_38 = 15, id inherited).
    pub(crate) fn mc2_fire_trail_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        if self.on_water(x, y) {
            self.ent[i].f26 += 1;
        } else if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1;
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < -1 || self.ent[i].f26 > 8 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let d = self.mc2_rand(i);
        let yaw = ((d % 0x5B) as i32 + self.ent[i].f30 as i32 - 45) as u16 & 0x7FF;
        self.ent[i].f30 = yaw;
        let mut pos = (x, y, self.ent[i].z);
        Self::polar_step(&mut pos, yaw, 0, 256);
        {
            let e = &mut self.ent[i];
            e.x = pos.0;
            e.y = pos.1;
            e.z = pos.2;
        }
        let (fov, id) = (self.ent[i].f84, self.ent[i].id24);
        if let Some(s) = self.mc2_spawn_fire_spray(pos.0, pos.1, pos.2) {
            let e = &mut self.ent[s];
            e.f84 = fov;
            e.act_life = 10;
            e.f40 = 15; // word_0x26_38
            e.id24 = id;
        }
    }

    /// `sub_32F40` (EF:24095) — the (10,19) ground-fire-spray tick:
    /// while alive, walk the radius-0 splat template (the center
    /// cell): a ~50% gate roll, two jitter rolls, and on ODD life
    /// ticks a 4-puff ring of (10,14) smoke (yaw start
    /// `(life/2 & 1) << 8`, step 0x200 to 0x800, id inherited);
    /// z snaps to terrain. On death, release the word_0x33 singleton
    /// (no ported latch — module-doc APPROX). `sub_10C80(ch0, 200)`
    /// EVERY tick including the despawn tick.
    pub(crate) fn mc2_fire_spray_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life >= 0 {
            self.ent[i].f26 = 0;
            let d = self.ent_rand(i);
            if 2 * ((d % 0x9D) as i32 / 79) - 1 > 0 {
                let (px, py, pz, id) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z, e.id24)
                };
                let d = self.ent_rand(i);
                let jx = (px as i32 - 96 - 64 + (d % 0x81) as i32) as u16;
                let d = self.ent_rand(i);
                let jy = (py as i32 - 96 + (d % 0x81) as i32 - 64) as u16;
                if self.ent[i].act_life & 1 == 1 {
                    let mut v10 = ((self.ent[i].act_life / 2) & 1) << 8;
                    while v10 < 0x800 {
                        if let Some(p) = self.mc2_spawn_smoke_particle_for(14, jx, jy, pz) {
                            self.ent[p].id24 = id;
                            self.ent[p].f30 = v10 as u16;
                        }
                        v10 += 0x200;
                    }
                }
            }
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            self.ent[i].z = self.ground_z(x, y) as i16;
        } else {
            self.ent[i].flags |= 0x400;
            // D41A0_0.word_0x33 = 0 — the singleton release (no
            // ported latch; see the module doc).
        }
        let amt = self.ent[i].f140 as u32;
        self.area_write(i, 0, amt, ctx, false, false);
    }

    /// `sub_38D80` (EF:28349) — the (10,54) aura tick: life-- (< 0 →
    /// despawn), then scan the creature list and stamp every entity
    /// within the SQUARED range 0xC40000 whose channel-4 mailbox
    /// source is clear: `mail[4] = (min(isqrt(d²), 42), self id)` —
    /// the word_0x76/78/7A field triplet (amount ≤ 42 so the high
    /// word is always 0). First-come, one tag per victim, no direct
    /// HP damage, no sound.
    pub(crate) fn mc2_aura_tick(&mut self, i: usize) {
        const RANGE_SQ: i32 = 12_845_056; // 0xC40000
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (ax, ay, src) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24)
        };
        for j in 1..self.ent.len() {
            if j == i || self.ent[j].class64 != 5 || self.ent[j].flags & 0x400 != 0 {
                continue;
            }
            if self.ent[j].mail[4].1 != 0 {
                continue;
            }
            let d2 = Self::dist2_sq(ax, ay, self.ent[j].x, self.ent[j].y);
            if d2 < RANGE_SQ {
                let mag = Self::isqrt(d2 as u32).min(42);
                self.ent[j].mail[4] = (mag, src);
            }
        }
    }
}
