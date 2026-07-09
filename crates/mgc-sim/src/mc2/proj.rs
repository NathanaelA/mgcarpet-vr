//! MC2 class-9 projectile family — the Phase-4.3 flyer core and the
//! creature attack thunks, ported verbatim from remc2 (trace bank:
//! docs/traces/mc2-class9-flyers.md; `EF:` = EventsFunctions.cpp,
//! `EV:` = Events.cpp cites).
//!
//! Field mapping additions over the [`super::mobs`] module doc:
//! `byte_0x43_67` impact class→f68 · `byte_0x44_68` impact model→f69
//! (the MC1 fields mean exactly this — detonation class/model) ·
//! `fov_0x22_34` desired-pitch→f36 · `roll_0x20_32` desired-yaw→f34
//! (as everywhere in the MC2 column) · `subSpellIndex_0x2A_42`
//! carried damage→f44 · `mana_0x90_144`→f140.
//!
//! DELIBERATE APPROXIMATIONS (cited, all counted where observable):
//! - The shielded-target ricochet `sub_68740` (EF:55220) and the
//!   friendly-shield homing/detonate pair `sub_68940`/`sub_68AC0`
//!   need the (10,78) shield entity — unported (MC2 spell column).
//!   No shields exist, so the gates are never live; skipped.
//! - The no-target acquisition `sub_67CB0` (EF:54710, model-keyed
//!   bucket sweeps) serves PLAYER-CAST spells; creature launches
//!   pre-lock `word_0x96_150`. Until the spell column lands, a
//!   target-less flyer snapshots its aim once (the retail else-arm,
//!   EF:62914-16) and flies straight.
//! - Water splash: retail spawns (10,5) (EF:62957-63); the (10,5)
//!   ctor/tick are unported — counted as a misfit, flyer despawns.
//! - An impact whose (f68, f69) effect is unported applies its f44
//!   as channel-0 area damage at the impact point (the effect IS the
//!   damage carrier in retail) and counts the misfit — damage lands,
//!   the visual gap stays visible in the ledger.
//! - `(9,9)` creator body pending (the subtype 0-0x0C trace); interim
//!   fields marked OPEN below.

use super::behavior::BEHAVIOR;
use crate::mc1::combat::MailTarget;
use crate::mc1::features::Gen;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// MC2-native projectile marker on [`Ent::flags`] (see
/// [`super::mobs`] for the other high bits). MC1-fallback projectiles
/// spawned on the MC2 column never carry it, so the class-9 dispatch
/// can tell the columns apart without guessing at state numbers.
pub(crate) const F_MC2PROJ: u32 = 1 << 29;
/// byte[0] bit 1 — the flyer's "aim acquired" latch (EF:62904).
const F_AIMED: u32 = 2;

impl Gen {
    // ---- class-9 creators ---------------------------------------------------

    /// `SummonFireball_4D2E0` (EF:34729) — the (9,0) bolt every
    /// creature ranged attack resolves into: action 0, speed 384,
    /// life 0x2000/384 = 21, mana 50, row 64, sprite 340. (The
    /// trailing `AddEvent2_847D0` dynamic light is presentation.)
    pub(crate) fn mc2_spawn_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 0;
            e.tick70 = 0;
            e.f126 = 384;
            e.f128 = 384;
            e.f140 = 50;
            e.max_life = (0x2000 / 384) as u32; // 21
            e.row156 = 64;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 340);
        Some(i)
    }

    /// `sub_4DC40` (EF:35071) — the (9,20) lob: action 21, speed 394,
    /// life 7680/394 = 19, sprite 196, NO behavior row (the launcher
    /// sets row 65).
    pub(crate) fn mc2_spawn_lob20(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 20;
            e.tick70 = 21;
            e.f126 = 394;
            e.f128 = 394;
            e.max_life = (7680 / 394) as u32; // 19
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 196);
        Some(i)
    }

    /// `sub_4DCC0` (EF:35091) — the (9,21) arc: action 22, speed 394,
    /// life 19, sprite 319, ShiftRot(256, 512).
    pub(crate) fn mc2_spawn_lob21(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 21;
            e.tick70 = 22;
            e.f126 = 394;
            e.f128 = 394;
            e.max_life = (7680 / 394) as u32;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 319);
        self.mc2_shift_rot(i, 256, 512);
        Some(i)
    }

    /// `sub_4D860` (EF:34942) — the (9,9) bolt (m23's `sub_1D260`
    /// payload, also the player thunder family): action 9, speed 384,
    /// life 3584/384 = 9, mana 50, row 63, sprite 216. (The trailing
    /// `AddEvent2_847D0` sub-effect is presentation.)
    pub(crate) fn mc2_spawn_bolt9(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 9;
            e.tick70 = 9;
            e.f126 = 384;
            e.f128 = 384;
            e.f140 = 50;
            e.max_life = (3584 / 384) as u32; // 9
            e.row156 = 63;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 216);
        Some(i)
    }

    // ---- the shared flyer flight (sub_65820, EF:62882) ----------------------

    /// Class filter of the victim probe `sub_10780` (EF:3766-69):
    /// `xtype == -1` admits anything, else class must match and
    /// `xsubtype == -1` or model must match. The human counts as
    /// class 3 model 0.
    fn mc2_proj_filter(&self, i: usize, hit: Option<MailTarget>) -> Option<MailTarget> {
        let (fc, fm) = (self.ent[i].f66, self.ent[i].f67);
        if fc == 0xFF {
            return hit;
        }
        match hit {
            Some(MailTarget::Pool(v)) => {
                let e = &self.ent[v];
                (e.class64 == fc && (fm == 0xFF || e.model65 == fm))
                    .then_some(hit)
                    .flatten()
            }
            Some(MailTarget::Player) => (fc == 3 && (fm == 0xFF || fm == 0))
                .then_some(hit)
                .flatten(),
            None => None,
        }
    }

    /// Impact-effect spawn (the sub_65820 expiry block, EF:62972-96):
    /// spawn `(f68, f69)` at the flyer's position, hand it the id,
    /// heading, victim and carried damage. Unported effects apply the
    /// damage directly (module-doc APPROX) and count the misfit.
    fn mc2_proj_impact(&mut self, i: usize, victim: u16, ctx: &MobCtx) {
        let (fc, fm, x, y, z, id, yaw, dmg) = {
            let e = &self.ent[i];
            (e.f68, e.f69, e.x, e.y, e.z, e.id24, e.f30, e.f44)
        };
        let spawned = match (fc, fm) {
            (10, 0) => self.mc2_spawn_fire(x, y, z),
            (10, 1) => self.mc2_spawn_big_explosion(x, y, z),
            _ => {
                self.note_misfit(fc as u16, fm as u16);
                let amt = dmg as u32;
                self.area_write(i, 0, amt, ctx, false, false);
                None
            }
        };
        if let Some(s) = spawned {
            let e = &mut self.ent[s];
            e.id24 = id;
            e.f30 = yaw;
            e.f146 = victim;
            e.f140 = dmg as i32; // subSpellIndex rides onto the effect
        }
        self.ent[i].flags |= 0x400;
    }

    /// `sub_65820` (EF:62882) — the shared class-9 flyer/projectile
    /// tick: per-tick homing with the behavior row's yaw/pitch caps
    /// (`sub_65610`, EF:62781 — caps v_2/v_6 via `sub_58350`), a ±2
    /// speed ramp toward minSpeed, the polar step, the tile-chain
    /// victim probe under the xtype/xsubtype filter, terrain CLAMP
    /// (flyers skim, they don't detonate on ground), water despawn,
    /// life expiry, and the (f68, f69) impact spawn.
    pub(crate) fn mc2_flyer_tick(&mut self, i: usize, ctx: &MobCtx) {
        // Homing / acquisition (EF:62902-21).
        match self.mc2_target(self.ent[i].f146, ctx) {
            Some((tx, ty, tz)) => {
                let e = &self.ent[i];
                let (yaw, pitch) = (e.f30, e.f32);
                let f34 = Self::angle_between(e.x, e.y, tx, ty);
                let dh = Self::isqrt(Self::dist2_sq(e.x, e.y, tx, ty) as u32) as i32;
                let f36 = Self::pitch_toward(e.z, tz, dh);
                let row = &BEHAVIOR[e.row156 as usize];
                let (cy, cp) = (row.v_2, row.v_6);
                let e = &mut self.ent[i];
                e.f34 = f34;
                e.f36 = f36;
                e.f30 = (yaw as i32 + Self::turn_step(yaw, f34, cy) as i32) as u16 & 0x7FF;
                e.f32 = (pitch as i32 + Self::turn_step(pitch, f36, cp) as i32) as u16 & 0x7FF;
            }
            None => {
                if self.ent[i].flags & F_AIMED == 0 {
                    // Snapshot aim once (the no-acquisition arm).
                    let e = &mut self.ent[i];
                    e.flags |= F_AIMED;
                    e.f34 = e.f30;
                    e.f36 = e.f32;
                }
            }
        }
        // Speed ramp toward minSpeed (EF:62923-31).
        {
            let e = &mut self.ent[i];
            if e.f126 < e.f128 {
                e.f126 += 2;
            } else if e.f126 > e.f128 {
                e.f126 -= 2;
            }
        }
        // Polar step + victim probe.
        let e = &self.ent[i];
        let mut pos = (e.x, e.y, e.z);
        Self::polar_step(&mut pos, e.f30, e.f32, e.f126);
        let scanned = self.victim_scan_at(i, pos, ctx);
        let hit = self.mc2_proj_filter(i, scanned);
        if hit.is_none() {
            // Terrain clamp — skim, don't detonate (EF:62947-53).
            let ground = self.ground_z(pos.0, pos.1) as i16;
            if pos.2 < ground {
                pos.2 = ground;
            }
            // Water despawn, models {4,22,24,26} exempt: spawn the
            // (10,5) splash with the projectile's id (EF:62955-65 —
            // was a pending-splash misfit note before the class-10
            // effects band landed the creator).
            if !matches!(self.ent[i].model65, 4 | 22 | 24 | 26) && self.cap_bit(pos.0, pos.1) == 1 {
                let own = self.ent[i].id24;
                if let Some(s) = self.mc2_spawn_splash(pos.0, pos.1, pos.2) {
                    self.ent[s].id24 = own;
                }
                self.ent[i].flags |= 0x400;
                return;
            }
            // Life countdown (EF:62966-70).
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life >= 0 {
                self.move_relink(i, pos.0, pos.1, pos.2);
                return;
            }
        }
        // Impact / expiry: land on the victim, spawn the effect.
        let victim = match hit {
            Some(MailTarget::Pool(v)) => {
                let (vx, vy, vz) = (self.ent[v].x, self.ent[v].y, self.ent[v].z);
                self.move_relink(i, vx, vy, vz);
                v as u16
            }
            Some(MailTarget::Player) => {
                self.move_relink(i, ctx.px, ctx.py, ctx.pz);
                PLAYER_TARGET
            }
            None => {
                self.move_relink(i, pos.0, pos.1, pos.2);
                0
            }
        };
        self.mc2_proj_impact(i, victim, ctx);
    }

    // ---- launch helpers ------------------------------------------------------

    /// `sub_5EF70` (EF:60598): poke the target wizard's danger timer.
    /// Pool wizards carry no reader yet (the rival MC2 column).
    pub(crate) fn mc2_danger_poke(&mut self, target: u16) {
        if target == PLAYER_TARGET {
            self.player_danger = 100;
        }
    }

    /// `sub_11900` (EF:4375) — the melee mailbox write: accumulate
    /// `amt` into the target's channel-0 inbox and stamp the attacker
    /// id (MC2 targets carry no per-channel mask; the human's inbox
    /// feeds the World intake).
    pub(crate) fn mc2_melee_write(&mut self, target: u16, amt: u32, src: u16) {
        let tgt = if target == PLAYER_TARGET {
            MailTarget::Player
        } else {
            MailTarget::Pool(target as usize)
        };
        self.mail_write(tgt, 0, amt, src);
    }

    /// The target's (class, model) for the projectile filter bytes —
    /// the human is faithfully (3, 0).
    fn mc2_target_cm(&self, target: u16) -> (u8, u8) {
        if target == PLAYER_TARGET || target as usize >= self.ent.len() {
            (3, 0)
        } else {
            let t = &self.ent[target as usize];
            (t.class64, t.model65)
        }
    }

    /// Shared field arming every launch thunk performs after the
    /// creator (id, aim, target hand-off, filter bytes).
    pub(crate) fn mc2_arm_proj(&mut self, p: usize, i: usize, target: u16, tpos: (u16, u16, i16)) {
        let (own, f146) = (self.ent[i].id24, self.ent[i].f146);
        let (px, py, pz) = (self.ent[p].x, self.ent[p].y, self.ent[p].z);
        self.ent[p].id24 = own;
        let yaw = Self::angle_between(px, py, tpos.0, tpos.1);
        let dh = Self::isqrt(Self::dist2_sq(px, py, tpos.0, tpos.1) as u32) as i32;
        self.ent[p].f30 = yaw;
        self.ent[p].f34 = yaw;
        let pitch = Self::pitch_toward(pz, tpos.2, dh);
        self.ent[p].f32 = pitch;
        self.ent[p].f36 = pitch;
        self.ent[p].f146 = f146;
        let (tc, tm) = self.mc2_target_cm(target);
        self.ent[p].f66 = tc;
        self.ent[p].f67 = tm;
    }

    // ---- the attack thunks (mc2_chase_attack-compatible) --------------------

    /// `sub_1CE80` (EF:9772): melee within 1024, damage = own f44.
    pub(crate) fn mc2_atk_melee_1024(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 1024)
    }

    /// `sub_1CED0` (EF:9786): melee within 768.
    pub(crate) fn mc2_atk_melee_768(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 768)
    }

    /// `sub_1CF20` (EF:9800): melee within 1536.
    pub(crate) fn mc2_atk_melee_1536(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 1536)
    }

    fn mc2_atk_melee(&mut self, i: usize, target: u16, ctx: &MobCtx, range: u32) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let e = &self.ent[i];
        if Self::mc2_dist3((e.x, e.y, e.z), tpos) >= range {
            return false;
        }
        let (amt, src) = (self.ent[i].f44 as u32, self.ent[i].id24);
        self.mc2_melee_write(target, amt, src);
        true
    }

    /// `sub_1CC20` (EF:9680): the (9,0) bolt — impact (10,0) fire,
    /// row 65, subSpell 500, z-lift = own fov (f84), danger poke.
    pub(crate) fn mc2_atk_bolt(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let Some(p) = self.mc2_spawn_bolt(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 0;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 500;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D0E0` (EF:9814): the (9,20) lob — impact (10,65),
    /// row 65, subSpell 780, z-lift = own fov.
    pub(crate) fn mc2_atk_lob20(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let Some(p) = self.mc2_spawn_lob20(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 65;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 780;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D1A0` (EF:9847): the (9,21) arc — impact (10,66),
    /// row 65, subSpell 780, fixed z-lift 128.
    pub(crate) fn mc2_atk_lob21(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let Some(p) = self.mc2_spawn_lob21(x, y, z.wrapping_add(128)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 66;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 780;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D260` (EF:9883): m23's (9,9) heavy bolt — spawned at
    /// pos + fov, impact (10,23), row 64, subSpell 4000.
    pub(crate) fn mc2_atk_heavy9(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let Some(p) = self.mc2_spawn_bolt9(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 23;
        self.ent[p].row156 = 64;
        self.ent[p].f44 = 4000;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D460` (EF:9918): m18's 5-shot fan — yaw offsets −226,
    /// −113, 0, +113, +226, each a (9,0) with impact (10,0), row 61,
    /// subSpell 800, z-lift 200.
    pub(crate) fn mc2_atk_fan(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let mut fired = false;
        for off in [-226i32, -113, 0, 113, 226] {
            let Some(p) = self.mc2_spawn_bolt(x, y, z.wrapping_add(200)) else {
                continue;
            };
            self.ent[p].f68 = 10;
            self.ent[p].f69 = 0;
            self.ent[p].row156 = 61;
            self.ent[p].f44 = 800;
            self.mc2_arm_proj(p, i, target, tpos);
            let yaw = (self.ent[p].f30 as i32 + off) as u16 & 0x7FF;
            self.ent[p].f30 = yaw;
            self.ent[p].f34 = yaw;
            fired = true;
        }
        if fired {
            self.mc2_danger_poke(target);
        }
        fired
    }

    /// `sub_1CDA0` (EF:9742): m9's (9,13) arrow — z-lift = own roll
    /// (f82), subSpell 600 when owned (f144 set) else 400, sprite 195
    /// doubled (the arrow ctor's own), danger poke.
    pub(crate) fn mc2_atk_arrow(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift, owned) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f82 as i16, e.f144 != 0)
        };
        let Some(p) = self.mc2_spawn_arrow(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f44 = if owned { 600 } else { 400 };
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }
}
