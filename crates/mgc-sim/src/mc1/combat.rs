//! MC1 combat: damage mailboxes, class-9 projectiles, class-10
//! combat effects (fire/explosion, fire-spreader, splash, blast ring,
//! hit-flash, mana-steal flash, mana ball) and the corpse pipeline —
//! ports of remc1 sub_main.cpp. Full specs in docs/ROADMAP.md
//! ("Combat, damage, death & corpses", "Fireball / repeat fireball").
//!
//! Deviations from the decompile:
//! - `sub_12B50`'s inverted accumulate/overwrite is NOT ported; the
//!   direct write uses the area writers' protocol (:17301-05)
//!   (deliberate: suspect transcription swap, like :21814).
//! - The m9 ranged thunk aims at the TARGET, not the atan2(0,0)
//!   self-aim (:21947-48) (deliberate: decompile casualty).
//! - Aim assist scores candidates by angular miss (Δyaw² + Δpitch²)
//!   with a distance tiebreak (deliberate approximation of sub_54A90's
//!   squared-miss-distance metric; exact port OPEN for the CREATURE
//!   cones — the possess acquisition runs the exact metric, see
//!   `aim_assist_possess_mc1`).
//! - The m9 lightning BEAM (sub_535E0 :63272) is a full port (one-tick
//!   hitscan walk + state-14 segment chain, confirmed vs remc2
//!   sub_66750); the explosion's +146 stamps hit-or-0 where the
//!   original writes garbage on a miss (deliberate).
//! - Class-9 model 14 / state 15 (the Troll & Ape boulder) has no
//!   TRANSCRIBED handler — remc1's class-9 tick table is truncated —
//!   so `proj_boulder_tick` reconstructs it: straight flight, silent,
//!   `(10,0)` impact. It must NOT alias onto state 13, whose
//!   first-tick roll is the arrow quartet (OPEN: retail table).
//! - Mana-shield reflection (+17 bit 7) is ported but nothing sets the
//!   flag yet (OPEN: wizard shields are the spell track).

use crate::engine::features::{Gen, lcg32, tile};
use crate::mc1::behavior::BEHAVIOR;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};
use crate::mc1::sprite_stats::SPRITE_STATS;
use crate::verbs::{CorpseVerb, TargetingVerb, VerbKind};

/// The player carpet's half-extents (sprite 44 stats halves — the
/// same constants the trigger/portal overlap uses).
pub(crate) const PLAYER_HW: i32 = (SPRITE_STATS[44].width / 2) as i32;
pub(crate) const PLAYER_HH: i32 = (SPRITE_STATS[44].height / 2) as i32;

/// Candidate set of the pure crosshair preview — the sub_54520
/// subtype blocks the player's own spells can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AimPreviewSet {
    /// Blocks 0/3/4 + the beam's one-shot snap: awake creatures +
    /// rival wizards (fireball, meteor, volcano, lightning).
    Creatures,
    /// Block 1: unowned mana balls + houses (possess).
    Possess,
    /// Blocks 7/8/B/C: rival wizards only (duel, steal, undead).
    Wizards,
}

/// A mailbox recipient: a pool event or the out-of-pool player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MailTarget {
    Pool(usize),
    Player,
}

/// The inbox verdict a state handler dispatches on (hitflag 0/1/2).
pub(crate) enum Inbox {
    Quiet,
    Hit(u16),
    Dead,
}

impl Gen {
    // ---- mailbox writes ---------------------------------------------------

    /// The shared write protocol (:17301-05): accumulate while a
    /// source is pending, overwrite a stale amount (readers clear the
    /// source but never the amount).
    pub(crate) fn mail_write(&mut self, tgt: MailTarget, ch: usize, amt: u32, src: u16) {
        let m = match tgt {
            MailTarget::Pool(i) => &mut self.ent[i].mail[ch],
            MailTarget::Player => &mut self.player_mail[ch],
        };
        if m.1 != 0 {
            m.0 = m.0.wrapping_add(amt);
        } else {
            m.0 = amt;
        }
        m.1 = src;
    }

    /// sub_118C0 (:16963) between two pool events: extents SUM per
    /// axis, z centered by each half-height (+78).
    pub(crate) fn ent_overlap(&self, a: usize, b: usize) -> bool {
        let (ea, eb) = (&self.ent[a], &self.ent[b]);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        wd(ea.x, eb.x) < ea.f80 as i32 + eb.f80 as i32
            && wd(ea.y, eb.y) < ea.f82 as i32 + eb.f82 as i32
            && ((ea.z as i32 + ea.f78 as i32) - (eb.z as i32 + eb.f78 as i32)).abs()
                < ea.f84 as i32 + eb.f84 as i32
    }

    /// sub_118C0 against the player carpet.
    pub(crate) fn player_overlap(&self, i: usize, ctx: &MobCtx) -> bool {
        let e = &self.ent[i];
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        wd(e.x, ctx.px) < e.f80 as i32 + PLAYER_HW
            && wd(e.y, ctx.py) < e.f82 as i32 + PLAYER_HW
            && ((e.z as i32 + e.f78 as i32) - (ctx.pz as i32 + PLAYER_HH)).abs()
                < e.f84 as i32 + PLAYER_HH
    }

    /// The writer's +66/+67 target filter (-1/-1 = wildcard).
    fn filter_admits(f66: u8, f67: u8, class: u8, model: u8) -> bool {
        (f66 == 0xFF || f66 == class) && (f67 == 0xFF || f67 == model)
    }

    /// sub_120B0 (:17235) / sub_124F0 (:17399) / sub_127E0 (:17502):
    /// the channel-N area write around event `i`. Gates per
    /// candidate: owner immunity (+24 equality — the engine's only
    /// friendly-fire rule), the damageable flag (+16&8), the
    /// vulnerability mask (+28 bit ch), the writer's +66/+67 filter,
    /// AABB overlap; the tile scan skips class-3 model 2 (:17372) —
    /// castles get their own ch0 pre-pass instead (:17325-34): every
    /// overlapping castle on ANOTHER team takes the mail (this is
    /// how mob-death fire cells fell castles), and under the 127E0
    /// variant (`shake`) EVERY castle in range — own included — arms
    /// its 30-tick blast-shake repaint (:17522). `building_tenth` =
    /// the 124F0 variant where class-2 model-0 TREES take amt/10
    /// (:17465 — the discount that keeps area spells from vaporizing
    /// forests; village buildings are class-10 m45 and take full
    /// amounts).
    /// Returns the number of mails written (retail's sub_124F0-family
    /// and MC2's sub_10C80/sub_116A0 return the hit count — the
    /// spellbook reports and the (10,9) earthquake gate consume it;
    /// MC1 callers ignore it).
    pub(crate) fn area_write(
        &mut self,
        i: usize,
        ch: usize,
        amt: u32,
        ctx: &MobCtx,
        building_tenth: bool,
        shake: bool,
    ) -> u32 {
        let mut count = 0u32;
        let (wx, wy, id, f66, f67) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24, e.f66, e.f67)
        };
        // The castle pre-pass (ch0 only).
        if ch == 0 {
            let mut hits: Vec<usize> = Vec::new();
            for j in 1..self.ent.len() {
                let c = &self.ent[j];
                if c.class64 == 3
                    && c.model65 == 2
                    && c.flags & 0x400 == 0
                    && j != i
                    && self.ent_overlap(i, j)
                {
                    hits.push(j);
                }
            }
            for j in hits {
                if shake {
                    self.ent[j].f50 = 30;
                }
                if self.ent[j].id24 != id {
                    self.mail_write(MailTarget::Pool(j), 0, amt, id);
                    count += 1;
                }
            }
        }
        let r = ((self.ent[i].f80 as i32 + 255) >> 8).max(1);
        let mut victims: Vec<(usize, u32)> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                let tx = ((wx >> 8) as i32 + dx) as u8;
                let ty = ((wy >> 8) as i32 + dy) as u8;
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                while j != 0 {
                    let c = &self.ent[j];
                    let next = c.next20 as usize;
                    if c.id24 != id
                        && c.flags & 8 != 0
                        && c.f28 & (1 << ch) != 0
                        && Self::filter_admits(f66, f67, c.class64, c.model65)
                        && !(ch == 0 && c.class64 == 3 && c.model65 == 2)
                        && self.ent_overlap(i, j)
                    {
                        let a = if building_tenth && c.class64 == 2 && c.model65 == 0 {
                            amt / 10
                        } else {
                            amt
                        };
                        victims.push((j, a));
                    }
                    j = next;
                }
            }
        }
        for (j, a) in victims {
            self.mail_write(MailTarget::Pool(j), ch, a, id);
            count += 1;
        }
        // The player probe (the human wizard is outside the pool; the
        // original reaches it through the same grid).
        if id != PLAYER_TARGET && Self::filter_admits(f66, f67, 3, 0) && self.player_overlap(i, ctx)
        {
            self.mail_write(MailTarget::Player, ch, amt, id);
            count += 1;
        }
        count
    }

    // ---- the creature inbox (the block opening every state handler) -------

    /// :21330-67: apply pending ch0 damage (awake only), inherit the
    /// weakest body segment's life, latch attacker (+40) and killer
    /// (+38), and report the hitflag.
    pub(crate) fn inbox(&mut self, i: usize) -> Inbox {
        let mut hit = 0u8;
        if self.ent[i].f58 != 0 {
            if self.ent[i].mail[0].1 != 0 {
                let (amt, src) = self.ent[i].mail[0];
                self.ent[i].act_life -= amt as i32;
                self.ent[i].mail[0].1 = 0; // amount stays stale (:21337)
                self.ent[i].f40 = src;
                hit = 1;
            } else {
                self.ent[i].f40 = 0;
            }
            let mut s = self.ent[i].f54 as usize;
            while s != 0 {
                if self.ent[s].act_life < self.ent[i].act_life {
                    self.ent[i].act_life = self.ent[s].act_life;
                    self.ent[i].f40 = self.ent[s].f40;
                    hit = 1;
                    break;
                }
                s = self.ent[s].f54 as usize;
            }
        }
        if self.ent[i].act_life < 0 {
            hit = 2;
        }
        self.ent[i].f38 = self.ent[i].f40;
        match hit {
            1 => Inbox::Hit(self.ent[i].f40),
            2 => Inbox::Dead,
            _ => Inbox::Quiet,
        }
    }

    /// Aggro test on a mailbox source: only class-3 (wizard-family)
    /// attackers provoke a chase (:21370-76).
    pub(crate) fn attacker_is_wizard(&self, src: u16) -> bool {
        if src == PLAYER_TARGET {
            return true;
        }
        let s = src as usize;
        s != 0 && s < self.ent.len() && self.ent[s].class64 == 3
    }

    // ---- class-9 projectiles ----------------------------------------------

    /// The shared class-9 init shape (str_255870 :4463): 8.8 position,
    /// not hittable (+16 &= ~8), refilled life, sprite-derived extents.
    /// `speed`/`life`/`row`/`sprite` per the model column; state = the
    /// model's flight state.
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        model: u8,
        state: u8,
        x: u16,
        y: u16,
        z: i16,
        speed: i16,
        life: u32,
        row: u8,
        sprite: u16,
    ) -> Option<usize> {
        let p = self.new_event()?;
        {
            let e = &mut self.ent[p];
            e.class64 = 9;
            e.model65 = model;
            e.tick70 = state;
            e.f126 = speed;
            e.f128 = speed;
            e.max_life = life;
            e.f140 = 50;
            e.row156 = row;
            e.flags &= !8;
        }
        self.link(p, x, y, z);
        self.refill_life(p);
        self.set_sprite(p, sprite);
        Some(p)
    }

    /// sub_39A10 (:45861): the fireball. Base speed 384, life 21
    /// ticks, homing row [5] (thunks override), sprite 42.
    pub(crate) fn spawn_fireball(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(0, 0, x, y, z, 384, 21, 5, 42)
    }

    /// sub_39BC0 (:45954): the m3 trail bolt (meteor). Row [1].
    pub(crate) fn spawn_trail_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(3, 3, x, y, z, 384, 21, 1, 76)
    }

    /// sub_39E40 (:46104): the m8 wizard-seeker. Row [4] (yaw 0x100).
    pub(crate) fn spawn_seeker(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(8, 8, x, y, z, 384, 21, 4, 214)
    }

    /// sub_39EC0 (:46135): the m9 zigzag lightning. Life 9.
    pub(crate) fn spawn_zigzag(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(9, 9, x, y, z, 384, 9, 4, 216)
    }

    /// sub_3A0C0 (:46256): the m13 straight bolt. Life 13, default
    /// row/damage (NewEvent's +44 = 100 unless the thunk overrides).
    pub(crate) fn spawn_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let p = self.spawn_projectile(13, 13, x, y, z, 384, 13, 0, 195)?;
        // The ctor's sprite call is the DOUBLING setter (:46274), not
        // the plain one every other class-9 ctor uses — the arrow
        // carries twice the collision half-extents (44/44/60 rather
        // than 22/22/30 for its 45x60 row).
        self.set_sprite_x2(p, 195);
        Some(p)
    }

    /// sub_3A390 (:46392): the m18 GLOBAL DEATH fuse. Fireball-shaped
    /// ctor (speed 384, life 0x2000/384 = 21, row [5], sprite 42) but
    /// state 19 sits past remc1's transcribed class-9 table. Observed
    /// retail behavior: never a bolt — fire once, wait, the blast lands
    /// AROUND THE CASTER. Reconstructed as a caster-anchored fuse: 21
    /// ticks tracking the caster, then the generic +44-copying
    /// detonation into the (10,55) field at the caster's position
    /// (deliberate reconstruction). The ctor's speed/aim/+150 target
    /// are carried but unused; the +26 charge byte (spawner moves the
    /// wizard's accumulated charge into it) stays unmodeled — role
    /// unknown. OPEN: retail may allow MULTIPLE overlapping charges,
    /// each detonating on its own delay; our cast gate (the row's
    /// 101-tick burst counter, decompile-consistent) blocks recast ~4s.
    pub(crate) fn spawn_bomb_fuse(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(18, 19, x, y, z, 384, 21, 5, 42)
    }

    /// State 19: the Global Death fuse tick — ride the caster, burn
    /// the 21-tick life, detonate in place (see spawn_bomb_fuse).
    fn bomb_fuse_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
            self.move_relink(i, ctx.px, ctx.py, ctx.pz);
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.proj_explode(i, ctx, None, true);
        }
        false
    }

    /// sub_3A1A0 (:46281): m7's slow bolt — state 15 is PAST remc1's
    /// transcribed table; interim straight-bolt flight (see header).
    pub(crate) fn spawn_slow_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(14, 15, x, y, z, 128, 32, 0, 196)
    }

    /// The player-spell payload projectiles (c9 m1 possess / m2
    /// earthquake / m4 volcano / m5 crater / m7 duel / m11 undead /
    /// m17 magnet): fireball-shaped init, state = model, dispatched
    /// to [`Gen::proj_payload_tick`] — except the MAGNET bolt (m17),
    /// which runs possession's state-1 flight: its ctor writes state
    /// 18 (:46371), past remc1's 14-entry class-9 state table, and
    /// the m1 flight is the behavior-matched stand-in. Inside it the
    /// m17 bolt diverges from possession twice, both decompile-
    /// corroborated: NO acquisition (sub_54520 has no model-17 case,
    /// default return 0 :64185 — it flies straight) and the
    /// model-39-ONLY contact scan (sub_11C00 :17083, not possession's
    /// 39/40/45 sub_11AC0).
    /// Sprites per the class-9 rows in `mc1_entities` — the magnet
    /// bolt shares possession's sprite 209 (both ctors call
    /// sub_36FA0(entity, 209), :45916/:46384: distinct models, one
    /// look). APPROX(original: each model's own flight state past
    /// remc1's transcribed table).
    pub(crate) fn spawn_spell_lob(&mut self, model: u8, x: u16, y: u16, z: i16) -> Option<usize> {
        let sprite = match model {
            1 | 17 => 209,
            2 => 211,
            4 => 210,
            5 => 211,
            7 => 213,
            11 => 281,
            _ => return None,
        };
        let state = if model == 17 { 1 } else { model };
        self.spawn_projectile(model, state, x, y, z, 384, 21, 0, sprite)
    }

    /// Vertical bearing (sub_42180 :52644): the pitch whose polar step
    /// descends from `fz` toward `tz` over horizontal distance `dh`.
    pub(crate) fn pitch_toward(fz: i16, tz: i16, dh: i32) -> u16 {
        Self::angle_of(fz.wrapping_sub(tz), (-(dh.clamp(0, 0x7FFF))) as i16)
    }

    /// Aim a fresh projectile from an attacker at a target point
    /// (sub_42150/42180 pair) and stamp the combat fields the thunks
    /// share: owner, filter, homing target, damage, explosion.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arm_projectile(
        &mut self,
        p: usize,
        owner: u16,
        f66: u8,
        f67: u8,
        target: u16,
        tx: u16,
        ty: u16,
        tz: i16,
        f44: u16,
        expl_model: u8,
    ) {
        let (px, py, pz) = (self.ent[p].x, self.ent[p].y, self.ent[p].z);
        let yaw = Self::angle_between(px, py, tx, ty);
        let dh = Self::isqrt(Self::dist2_sq(px, py, tx, ty) as u32) as i32;
        let pitch = Self::pitch_toward(pz, tz, dh);
        let e = &mut self.ent[p];
        e.id24 = owner;
        e.f66 = f66;
        e.f67 = f67;
        e.f146 = target;
        e.f30 = yaw;
        e.f34 = yaw;
        e.f32 = pitch;
        e.f36 = pitch;
        e.f44 = f44;
        e.f68 = 10;
        e.f69 = expl_model;
    }

    /// The TargetingVerb seam (crate::verbs) — the acquire subtypes
    /// dispatch here. MC2's own acquire column lives in mc2::mobs;
    /// this dispatcher is only reached from MC1-spell paths, where an
    /// MC2 world serves the MC1 scan and notes the fallback (the
    /// pinned frankenstein ledger).
    fn aim_assist(&mut self, i: usize, ctx: &MobCtx) {
        match self.verbs.targeting {
            TargetingVerb::Mc1 | TargetingVerb::Mc1Hw => self.aim_assist_mc1(i, ctx),
            TargetingVerb::Mc2 => {
                self.note_verb_fallback(VerbKind::Targeting);
                self.aim_assist_mc1(i, ctx);
            }
        }
    }

    /// Is this the Hidden Worlds engine? HW's entire live sim delta is
    /// the original's single compiled `IsHiddenWord` bool (two branches:
    /// the model-16 homing meteor and the napalm-geometry fork). We
    /// carry it as the one HW-distinct verb — the targeting column —
    /// rather than a parallel flag; every HW branch reads it here. If HW
    /// ever needs a divergence on a column that also varies for MC2,
    /// promote this to a dedicated field.
    pub(crate) fn is_hidden_worlds(&self) -> bool {
        matches!(self.verbs.targeting, TargetingVerb::Mc1Hw)
    }

    /// One-time target acquisition sub_54520 (:63943): nearest awake
    /// creature (any range) or wizard within the caster row's v_28,
    /// inside a ±0x71 yaw AND pitch cone, 3D distance ≤ 5120.
    fn aim_assist_mc1(&mut self, i: usize, ctx: &MobCtx) {
        self.aim_assist_mc1_cone(i, ctx, 0x71, 0x71);
    }

    /// [`Self::aim_assist_mc1`] with an explicit acquire cone. The base
    /// MC1 scan is `0x71`/`0x71`; Hidden Worlds' Fire Storm child (model
    /// 16, acquire switch case 0x10, remc1hw :60322) widens the YAW cone
    /// to `0x100` while the pitch stays `0x71`. APPROX: case 0x10 scans
    /// the spatial buckets for any awake entity; we reuse the shared
    /// creature+wizard+player candidate set (the meaningful enemy set),
    /// only widening the cone.
    fn aim_assist_mc1_cone(&mut self, i: usize, ctx: &MobCtx, yaw_cone: u32, pitch_cone: u32) {
        let (px, py, pz, yaw, pitch, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.id24)
        };
        let mut best: Option<(u16, u32, u16, u16)> = None; // (slot, score, yaw, pitch)
        let consider =
            |tx: u16, ty: u16, tz: i16, slot: u16, best: &mut Option<(u16, u32, u16, u16)>| {
                let d2 = Self::dist2_sq(px, py, tx, ty);
                let dz = tz.wrapping_sub(pz) as i32;
                let d3 = d2.wrapping_add(dz.wrapping_mul(dz));
                if d3 > 5120 * 5120 {
                    return;
                }
                let ty_yaw = Self::angle_between(px, py, tx, ty);
                let dh = Self::isqrt(d2 as u32) as i32;
                let ty_pitch = Self::pitch_toward(pz, tz, dh);
                let dy = Self::angdist(yaw, ty_yaw) as u32;
                let dp = Self::angdist(pitch, ty_pitch) as u32;
                if dy > yaw_cone || dp > pitch_cone {
                    return;
                }
                let score = dy * dy + dp * dp;
                // Strictly-less: on a score tie the earlier slot wins,
                // matching the original's scan order.
                if best.is_none() || best.is_some_and(|(_, bs, _, _)| score < bs) {
                    *best = Some((slot, score, ty_yaw, ty_pitch));
                }
            };
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 5 || c.tick70 == 120 || c.act_life < 0 || c.f58 == 0 {
                continue;
            }
            if c.id24 == own {
                continue;
            }
            let (tx, ty, tz) = (c.x, c.y, c.z.wrapping_add(c.f78 as i16));
            consider(tx, ty, tz, j as u16, &mut best);
        }
        // Rival wizards (class 3, models 0/1): live, not hidden or
        // cloaked (the shared +16 0x20 bit), not the caster's own.
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 3
                || c.model65 > 1
                || c.tick70 != 1
                || c.flags & (0x400 | 0x20) != 0
                || c.id24 == own
            {
                continue;
            }
            let (tx, ty, tz) = (c.x, c.y, c.z.wrapping_add(c.f78 as i16));
            consider(tx, ty, tz, j as u16, &mut best);
        }
        // Invisible (spell 12, :65689-90 — the +16 0x20 bit): the
        // cloaked player is skipped by mob-side target acquisition.
        if own != PLAYER_TARGET && !self.player_invisible {
            consider(
                ctx.px,
                ctx.py,
                ctx.pz.wrapping_add(PLAYER_HH as i16),
                PLAYER_TARGET,
                &mut best,
            );
        }
        if let Some((slot, _, ty_yaw, ty_pitch)) = best {
            self.ent[i].f146 = slot;
            self.ent[i].f34 = ty_yaw;
            self.ent[i].f36 = ty_pitch;
            // Being targeted arms the danger music (:64013/:64095 —
            // acquire of a class-3 m0 human calls sub_46520).
            if slot == PLAYER_TARGET {
                self.player_danger = 100;
            }
        }
    }

    /// Read-only twin of the acquire family below for the crosshair
    /// instrument (P-class `crosshair` option): identical candidate
    /// filters, cone (±0x71 yaw AND pitch), 3D range (≤ 5120) and
    /// min-score pick as [`Self::aim_assist`] /
    /// [`Self::aim_assist_wizards`] / [`Self::aim_assist_possess`] —
    /// but NO entity writes, NO `player_danger` arming and NO LCG
    /// draws, so it is safe to run every frame without touching
    /// simulation state. The caster is the human player
    /// (own = PLAYER_TARGET), so the mob scans' player-candidate arm
    /// never applies. Returns the acquired slot.
    pub(crate) fn aim_preview_scan(
        &self,
        px: u16,
        py: u16,
        pz: i16,
        yaw: u16,
        pitch: u16,
        set: AimPreviewSet,
    ) -> Option<u16> {
        let own = PLAYER_TARGET;
        let mut best: Option<(u16, u32)> = None;
        let mut consider = |tx: u16, ty: u16, tz: i16, slot: u16| {
            let d2 = Self::dist2_sq(px, py, tx, ty);
            let dz = tz.wrapping_sub(pz) as i32;
            if d2.wrapping_add(dz.wrapping_mul(dz)) > 5120 * 5120 {
                return;
            }
            let ty_yaw = Self::angle_between(px, py, tx, ty);
            let dh = Self::isqrt(d2 as u32) as i32;
            let ty_pitch = Self::pitch_toward(pz, tz, dh);
            let dy = Self::angdist(yaw, ty_yaw) as u32;
            let dp = Self::angdist(pitch, ty_pitch) as u32;
            if dy > 0x71 || dp > 0x71 {
                return;
            }
            let score = dy * dy + dp * dp;
            if best.is_none_or(|(_, bs)| score < bs) {
                best = Some((slot, score));
            }
        };
        if set == AimPreviewSet::Possess {
            // Mirror of aim_assist_possess: unowned/unclaimed awake
            // mana balls (m39/40) + anyone else's houses (m45).
            for j in 1..self.ent.len() {
                let c = &self.ent[j];
                if c.class64 != 10 || c.flags & 0x400 != 0 {
                    continue;
                }
                let candidate = match c.model65 {
                    39 | 40 => c.f58 != 0 && c.f144 != own && c.id24 != own,
                    45 => c.f144 != own && c.id24 != own,
                    _ => false,
                };
                if candidate {
                    consider(c.x, c.y, c.z.wrapping_add(c.f78 as i16), j as u16);
                }
            }
            return best.map(|(slot, _)| slot);
        }
        if set == AimPreviewSet::Creatures {
            // Mirror of aim_assist's creature scan.
            for j in 1..self.ent.len() {
                let c = &self.ent[j];
                if c.class64 != 5 || c.tick70 == 120 || c.act_life < 0 || c.f58 == 0 {
                    continue;
                }
                if c.id24 == own {
                    continue;
                }
                consider(c.x, c.y, c.z.wrapping_add(c.f78 as i16), j as u16);
            }
        }
        // Both remaining sets scan the rival-wizard list (live, not
        // hidden or cloaked, not own).
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 3
                || c.model65 > 1
                || c.tick70 != 1
                || c.flags & (0x400 | 0x20) != 0
                || c.id24 == own
            {
                continue;
            }
            consider(c.x, c.y, c.z.wrapping_add(c.f78 as i16), j as u16);
        }
        best.map(|(slot, _)| slot)
    }

    /// The wizard-only acquire subtype's TargetingVerb seam (see
    /// [`Self::aim_assist`]).
    fn aim_assist_wizards(&mut self, i: usize, ctx: &MobCtx) {
        match self.verbs.targeting {
            TargetingVerb::Mc1 | TargetingVerb::Mc1Hw => self.aim_assist_wizards_mc1(i, ctx),
            TargetingVerb::Mc2 => {
                self.note_verb_fallback(VerbKind::Targeting);
                self.aim_assist_wizards_mc1(i, ctx);
            }
        }
    }

    /// The wizard-only acquire (sub_54520 blocks 7/8/B/C — duel m7,
    /// steal m8, undead m11): same cone/range as [`Self::aim_assist`]
    /// but the candidate set is the class-3 wizard list alone.
    fn aim_assist_wizards_mc1(&mut self, i: usize, ctx: &MobCtx) {
        let (px, py, pz, yaw, pitch, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.id24)
        };
        let mut best: Option<(u16, u32, u16, u16)> = None;
        let consider =
            |tx: u16, ty: u16, tz: i16, slot: u16, best: &mut Option<(u16, u32, u16, u16)>| {
                let d2 = Self::dist2_sq(px, py, tx, ty);
                let dz = tz.wrapping_sub(pz) as i32;
                if d2.wrapping_add(dz.wrapping_mul(dz)) > 5120 * 5120 {
                    return;
                }
                let ty_yaw = Self::angle_between(px, py, tx, ty);
                let dh = Self::isqrt(d2 as u32) as i32;
                let ty_pitch = Self::pitch_toward(pz, tz, dh);
                let dy = Self::angdist(yaw, ty_yaw) as u32;
                let dp = Self::angdist(pitch, ty_pitch) as u32;
                if dy > 0x71 || dp > 0x71 {
                    return;
                }
                let score = dy * dy + dp * dp;
                if best.is_none_or(|(_, bs, _, _)| score < bs) {
                    *best = Some((slot, score, ty_yaw, ty_pitch));
                }
            };
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 3
                || c.model65 > 1
                || c.tick70 != 1
                || c.flags & (0x400 | 0x20) != 0
                || c.id24 == own
            {
                continue;
            }
            consider(
                c.x,
                c.y,
                c.z.wrapping_add(c.f78 as i16),
                j as u16,
                &mut best,
            );
        }
        if own != PLAYER_TARGET && !self.player_invisible {
            consider(
                ctx.px,
                ctx.py,
                ctx.pz.wrapping_add(PLAYER_HH as i16),
                PLAYER_TARGET,
                &mut best,
            );
        }
        if let Some((slot, _, ty_yaw, ty_pitch)) = best {
            self.ent[i].f146 = slot;
            self.ent[i].f34 = ty_yaw;
            self.ent[i].f36 = ty_pitch;
            if slot == PLAYER_TARGET {
                self.player_danger = 100;
            }
        }
    }

    /// sub_52550 (:62534): per-tick homing — recompute bearing to the
    /// target (z-centered) and turn yaw/pitch capped at the row's
    /// v_2/v_6.
    fn home(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let tgt = self.ent[i].f146;
        let (tx, ty, tz) = if tgt == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz.wrapping_add(PLAYER_HH as i16))
        } else {
            let t = tgt as usize;
            if t == 0 || t >= self.ent.len() || self.ent[t].class64 == 0 || self.ent[t].act_life < 0
            {
                self.ent[i].f146 = 0;
                return false;
            }
            let c = &self.ent[t];
            (c.x, c.y, c.z.wrapping_add(c.f78 as i16))
        };
        let e = &self.ent[i];
        let yaw = Self::angle_between(e.x, e.y, tx, ty);
        let dh = Self::isqrt(Self::dist2_sq(e.x, e.y, tx, ty) as u32) as i32;
        let pitch = Self::pitch_toward(e.z, tz, dh);
        let row = &BEHAVIOR[e.row156 as usize];
        let (v2, v6) = (row.v_2, row.v_6);
        self.ent[i].f34 = yaw;
        self.ent[i].f36 = pitch;
        let ty_ = Self::turn_step(self.ent[i].f30, yaw, v2);
        self.ent[i].f30 = (self.ent[i].f30 as i32 + ty_ as i32) as u16 & 0x7FF;
        let tp = Self::turn_step(self.ent[i].f32, pitch, v6);
        self.ent[i].f32 = (self.ent[i].f32 as i32 + tp as i32) as u16 & 0x7FF;
        true
    }

    /// sub_11980 (:16988) from a projectile: first overlapped victim
    /// in the surrounding cells passing the filter/owner/damageable
    /// gates. Also probes the out-of-pool player.
    fn victim_scan(&self, i: usize, ctx: &MobCtx) -> Option<MailTarget> {
        let (wx, wy, id, f66, f67) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24, e.f66, e.f67)
        };
        let r = ((self.ent[i].f80 as i32 + 255) >> 8).max(1);
        for dy in -r..=r {
            for dx in -r..=r {
                let tx = ((wx >> 8) as i32 + dx) as u8;
                let ty = ((wy >> 8) as i32 + dy) as u8;
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                while j != 0 {
                    let c = &self.ent[j];
                    // Class-14 map objects (MC2 XP scrolls, mouth/
                    // checkpoint markers) are OBSERVABLE pass-through:
                    // retail's probe admits them mechanically (the
                    // (14,5) ctor keeps byte[0]&8, EF:37315/37365,
                    // and a player bolt's xtype is the −1 wildcard)
                    // but its ≈0-box, own-cell, endpoint-only probe
                    // never reaches the scroll's 768/1280 PICKUP box
                    // in practice (EF:63127-28 + Events.cpp:132 ring
                    // 0). Our anti-tunneling ring + chord-march
                    // (below/mc2 proj) WOULD reach it — the player's
                    // "fireballs detonate on scrolls / scrolls steal
                    // autoaim" report — so the guard restores the
                    // retail observable (2026-07-16 scroll trace;
                    // MC1 has no class-14, goldens untouched).
                    if c.id24 != id
                        && c.flags & 8 != 0
                        && c.class64 != 14
                        && Self::filter_admits(f66, f67, c.class64, c.model65)
                        && self.ent_overlap(i, j)
                    {
                        return Some(MailTarget::Pool(j));
                    }
                    j = c.next20 as usize;
                }
            }
        }
        if id != PLAYER_TARGET && Self::filter_admits(f66, f67, 3, 0) && self.player_overlap(i, ctx)
        {
            return Some(MailTarget::Player);
        }
        None
    }

    /// The CLAIM/possession candidate test `sub_108B0` (EF:3766)'s
    /// whitelist body. The possession projectile (action 18) does NOT
    /// collide with every solid like the generic `victim_scan`
    /// (`sub_10780`) — it detonates ONLY on entities it could claim
    /// and flies straight through everything else. Whitelist (verbatim
    /// sub_108B0, EF:3826-58): worm heads (5,22); the 512/random mana
    /// spheres (10,39)/(10,40); the foreign-owned sphere variant
    /// (10,57) when its parent tag differs from the caster; and
    /// buildings (10,45) ONLY when POSSESSABLE — `bldgprm.flags & 8
    /// == 0`. The un-possessable factory / terrain-modification
    /// buildings (level-001 cross sinks, level-000 spires) and every
    /// wizard / marker keep the bit set or fall off the list, so
    /// possession passes through them (NOT the generic probe, which
    /// would consume the shot on those sinks). Retail's accept filter
    /// (EF:3862-67) is TWO-armed: the creator half (`id_0x1A_26` →
    /// `id24`) AND the claim-owner half (`playerEntityIndex_0x94_148`
    /// → `f144`, the field both claim intakes write) — a ball or
    /// building the caster already possesses does NOT eat the bolt;
    /// it flies through to the unclaimed field behind. A
    /// rival-claimed target fails neither half and stays claimable.
    fn claim_admits(&self, j: usize, own: u16) -> bool {
        let c = &self.ent[j];
        if c.flags & 8 == 0 {
            return false;
        }
        match (c.class64, c.model65) {
            (5, 22) => c.id24 != own && c.f144 != own,
            (10, 39) | (10, 40) => c.id24 != own && c.f144 != own,
            // The (10,57) foreign sphere: gated on the parent tag
            // (+40), no id/owner re-check (sub_108B0's early-return
            // arm, EF:3846).
            (10, 57) => c.f40 != own,
            (10, 45) => {
                c.id24 != own
                    && c.f144 != own
                    && self
                        .assets
                        .bldgprm
                        .get(c.f71 as usize)
                        .is_none_or(|b| b.flags & 8 == 0)
            }
            _ => false,
        }
    }

    /// The possession victim probe `sub_108B0` (EF:3766): the same
    /// tile-chain sweep as [`Self::victim_scan`] but under the
    /// claim whitelist ([`Self::claim_admits`]) — and with NO player
    /// probe (sub_108B0 never reaches the human wizard; you cannot
    /// possess a wizard).
    fn claim_victim_scan(&self, i: usize) -> Option<MailTarget> {
        let (wx, wy, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24)
        };
        let r = ((self.ent[i].f80 as i32 + 255) >> 8).max(1);
        for dy in -r..=r {
            for dx in -r..=r {
                let tx = ((wx >> 8) as i32 + dx) as u8;
                let ty = ((wy >> 8) as i32 + dy) as u8;
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                while j != 0 {
                    let next = self.ent[j].next20 as usize;
                    if self.claim_admits(j, id) && self.ent_overlap(i, j) {
                        return Some(MailTarget::Pool(j));
                    }
                    j = next;
                }
            }
        }
        None
    }

    /// [`Self::claim_victim_scan`] at a temporary probe position (the
    /// marched-substep companion of [`Self::victim_scan_at`]).
    pub(crate) fn claim_victim_scan_at(
        &mut self,
        i: usize,
        tmp: (u16, u16, i16),
    ) -> Option<MailTarget> {
        let old = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        self.ent[i].x = tmp.0;
        self.ent[i].y = tmp.1;
        self.ent[i].z = tmp.2;
        let v = self.claim_victim_scan(i);
        self.ent[i].x = old.0;
        self.ent[i].y = old.1;
        self.ent[i].z = old.2;
        v
    }

    /// The explode tail shared by the flight handlers: accuracy stats
    /// (sub_526C0 :62585), spawn the +68/+69 effect, despawn. The
    /// generic sub_52770 path (:62759-72) also copies +44 and the
    /// victim; sub_52B30 (fireball) does NOT (:62928-30) — the fire's
    /// own 400 is the fireball's real damage.
    fn proj_explode(&mut self, i: usize, ctx: &MobCtx, struck: Option<MailTarget>, copy_f44: bool) {
        let (x, y, z, owner, yaw, pitch, f44, f69) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30, e.f32, e.f44, e.f69)
        };
        if owner == PLAYER_TARGET {
            self.shots += 1;
            let aimed = self.ent[i].f146;
            if struck.is_some_and(|s| match s {
                MailTarget::Pool(j) => aimed == self.ent[j].id24 || aimed == j as u16,
                MailTarget::Player => false,
            }) {
                self.hits += 1;
            }
        }
        // Mana Magnet bolt (m17): the magnet manifests ONLY on an
        // actual ball strike — a bolt that grounds or expires on
        // nothing fizzles with NO effect (player-verified; the same
        // law as MC2's possession magnet, which never drops a
        // free-floating magnet at a terrain detonation).
        let magnet_bolt = self.ent[i].class64 == 9 && self.ent[i].model65 == 17;
        if !(magnet_bolt && struck.is_none()) {
            if let Some(fx) = self.spawn_effect(f69, x, y, z) {
                let e = &mut self.ent[fx];
                e.id24 = owner;
                e.f30 = yaw;
                e.f32 = pitch;
                if copy_f44 {
                    e.f44 = f44;
                }
            }
        }
        // On a strike, pair the (10,54) with a LOCALIZED possession-
        // style claim of the struck ball(s). Retail claims what the
        // bolt hits (player-verified) but the reconstruction lost the
        // call (the +66=0 flashes neuter sub_120B0's filter, and the
        // (10,54) tick claims nothing) — the port bridges the gap
        // with possession's own (10,12) claim flash, filtered to mana
        // balls only (f66/f67 = 10/39: unlike a possess flash it must
        // not claim houses or graves — the spell is a mana tool;
        // APPROX pending retail evidence). The pulled remainder
        // claims by MERGING into the claimed ball(s) (owned-beats-
        // unowned, sub_277D0 :29717).
        if magnet_bolt && struck.is_some() {
            if let Some(fl) = self.spawn_effect(12, x, y, z) {
                let e = &mut self.ent[fl];
                e.id24 = owner;
                e.f66 = 10;
                e.f67 = 39;
            }
        }
        let _ = ctx;
        self.ent[i].flags |= 0x400;
    }

    /// Class-9 flight dispatch by state (str_25573C :4838).
    pub(crate) fn proj_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        match self.ent[i].tick70 {
            0 => self.proj_m0_tick(i, ctx),
            1 => self.proj_m1_tick(i, ctx),
            // Global Death's m18 fuse (state 19, reconstruction — see
            // spawn_bomb_fuse): rides the caster, detonates the
            // (10,55) field in place.
            19 => self.bomb_fuse_tick(i, ctx),
            3 => self.proj_generic_tick(i, ctx, true),
            8 => self.proj_m8_tick(i, ctx),
            9 => self.proj_m9_tick(i, ctx),
            10 => self.proj_castle_ball_tick(i, ctx),
            12 => self.proj_m12_tick(i, ctx),
            13 => self.proj_bolt_tick(i, ctx),
            // The Troll/Ape boulder — its own state, silent in flight
            // (it used to alias onto 13 and inherit the arrow roll).
            15 => self.proj_boulder_tick(i, ctx),
            17 => self.proj_firewall_tick(i, ctx),
            // Player-spell payload projectiles (spell track). The
            // m17 magnet bolt is NOT here — it rides possession's
            // state-1 flight (see spawn_spell_lob).
            2 | 4 | 5 | 7 | 11 => self.proj_payload_tick(i, ctx),
            // Beam segment (state 14; remc1's table is truncated here
            // — lifecycle reconstructed from the slot-order life trick
            // :63349-53): kill on the PRE-decrement value so every
            // segment renders exactly one frame regardless of whether
            // its slot ticks before or after the beam's.
            14 => {
                if self.ent[i].act_life < 0 {
                    self.ent[i].flags |= 0x400;
                } else {
                    self.ent[i].act_life -= 1;
                }
                false
            }
            _ => false,
        }
    }

    /// sub_52B30 (:62779): the fireball. Returns terrain_dirty.
    fn proj_m0_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // Steering: while untargeted the acquire scan re-runs EVERY
        // tick (:62815 — +146 invalid → sub_54520; model 0 is an
        // acquire case), so a bolt launched wide SNAPS mid-flight
        // the moment a victim enters the ±0x71 cone. Then a ≤34/tick
        // yaw ease; homing once a target exists.
        if self.ent[i].f146 == 0 {
            self.aim_assist(i, ctx);
            if self.ent[i].f146 == 0 {
                self.ent[i].f34 = self.ent[i].f30;
                self.ent[i].f36 = self.ent[i].f32;
            }
            let t = Self::turn_step(self.ent[i].f30, self.ent[i].f34, 34);
            self.ent[i].f30 = (self.ent[i].f30 as i32 + t as i32) as u16 & 0x7FF;
            self.ent[i].f32 = self.ent[i].f36;
        } else {
            self.home(i, ctx);
        }
        self.proj_move_and_hit(i, ctx, false)
    }

    /// sub_52ED0 (:62937): the POSSESS lob (c9 m1). Its flight z is
    /// clamped UP to the terrain each tick (:62975-77 — the lob skims
    /// rising ground), its acquisition scans ONLY mana balls and
    /// houses (sub_54520 case 1, :64040-77 — never creatures or
    /// wizards), and its victim scan is the dedicated sub_11AC0
    /// (:17033): class-10 models 39/40/45 only, skipping entities the
    /// shooter already owns or claimed. Any end detonates into the
    /// (10,12) ch1-claim flash.
    fn proj_m1_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let _ = ctx;
        // ONE acquisition roll on the first untargeted tick — the
        // +16&2 latch (:62952-60), same idiom as the HW dart and the
        // castle ball. A lob that finds nothing (or later loses its
        // slot) flies straight and never re-acquires.
        if self.ent[i].f146 == 0 {
            if self.ent[i].flags & 2 == 0 {
                self.ent[i].flags |= 2;
                self.aim_assist_possess(i);
            }
        } else {
            self.home_possess(i);
        }
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let g = self.ground_z(tmp.0, tmp.1) as i16;
        if tmp.2 < g {
            tmp.2 = g; // ground clamp (:62975-77)
        }
        let hit = self.possess_victim_at(i, tmp);
        self.move_relink(i, tmp.0, tmp.1, tmp.2);
        self.ent[i].act_life -= 1;
        if let Some(j) = hit {
            let (jx, jy, jz) = (self.ent[j].x, self.ent[j].y, self.ent[j].z);
            self.move_relink(i, jx, jy, jz);
            self.proj_explode(i, ctx, Some(MailTarget::Pool(j)), false);
        } else if self.ent[i].act_life < 0 {
            self.proj_explode(i, ctx, None, false);
        }
        false
    }

    /// The possess-acquire subtype's TargetingVerb seam (see
    /// [`Self::aim_assist`]).
    fn aim_assist_possess(&mut self, i: usize) {
        match self.verbs.targeting {
            TargetingVerb::Mc1 | TargetingVerb::Mc1Hw => self.aim_assist_possess_mc1(i),
            TargetingVerb::Mc2 => {
                self.note_verb_fallback(VerbKind::Targeting);
                self.aim_assist_possess_mc1(i);
            }
        }
    }

    /// sub_54520 case 1 (:64040-77): possess acquisition — the awake
    /// (+58 != 0) mana balls (m39/40) and houses (m45) not already
    /// CLAIMED by the shooter (+144 only — the creator +24 half of the
    /// gate is impact-only, :17067), inside the ±0x71 yaw+pitch cone
    /// within 2-D distance 5120 (sub_423D0 has no z term). Best by
    /// sub_54A90's score (:64212-17): the distance decomposed onto the
    /// angular-error axes — 16.16 cos terms >>16, sin terms >>14
    /// through an i16 truncation (~4x misalignment weight) — compared
    /// UNSIGNED (the -1 reject sentinel = u32::MAX). Snaps the heading
    /// on success.
    fn aim_assist_possess_mc1(&mut self, i: usize) {
        use crate::mc1::tables::{COS, SIN};
        let (px, py, pz, yaw, pitch, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.id24)
        };
        // The Mana Magnet bolt (m17) acquires NOTHING: sub_54520
        // switches on the bolt's model and has no case for 17 —
        // default `return 0` (:63977/:64185). The magnet bolt flies
        // straight; only its contact scan detonates it. (Possession
        // is case 1: the balls + graves/dwellings lists, :64040-58.)
        if self.ent[i].model65 == 17 {
            return;
        }
        let mut best: Option<(u16, u32, u16, u16)> = None;
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 != 10 || c.flags & 0x400 != 0 {
                continue;
            }
            let candidate = match c.model65 {
                39 | 40 | 45 => c.f144 != own && c.f58 != 0,
                _ => false,
            };
            if !candidate {
                continue;
            }
            let (tx, ty, tz) = (c.x, c.y, c.z.wrapping_add(c.f78 as i16));
            let ty_yaw = Self::angle_between(px, py, tx, ty);
            let dy = Self::angdist(yaw, ty_yaw) as usize;
            if dy > 0x71 {
                continue;
            }
            let dist = Self::isqrt(Self::dist2_sq(px, py, tx, ty) as u32) as i32;
            let ty_pitch = Self::pitch_toward(pz, tz, dist);
            let dp = Self::angdist(pitch, ty_pitch) as usize;
            if dp > 0x71 || dist > 5120 {
                continue;
            }
            let v8 = dist * COS[dy];
            let v9 = dist * SIN[dy];
            let v10 = dist * COS[dp];
            let v11 = ((SIN[dp] * dist) >> 14) as i16 as i32;
            let score = ((v10 >> 16) * (v10 >> 16)
                + (v8 >> 16) * (v8 >> 16)
                + ((v9 >> 14) as i16 as i32) * ((v9 >> 14) as i16 as i32)
                + v11 * v11) as u32;
            if best.is_none() || best.is_some_and(|(_, bs, _, _)| score < bs) {
                best = Some((j as u16, score, ty_yaw, ty_pitch));
            }
        }
        if let Some((slot, _, ty_yaw, ty_pitch)) = best {
            let e = &mut self.ent[i];
            e.f146 = slot;
            e.f30 = ty_yaw;
            e.f32 = ty_pitch;
            e.f34 = ty_yaw;
            e.f36 = ty_pitch;
        }
    }

    /// Pool-target homing for the possess lob (the sub_52550 steer
    /// against a class-10 target — the generic home() only handles
    /// creatures/the player).
    fn home_possess(&mut self, i: usize) {
        let t = self.ent[i].f146 as usize;
        if t == 0
            || t >= self.ent.len()
            || self.ent[t].class64 != 10
            || self.ent[t].flags & 0x400 != 0
        {
            self.ent[i].f146 = 0;
            return;
        }
        let (px, py, pz) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (tx, ty, tz) = (
            self.ent[t].x,
            self.ent[t].y,
            self.ent[t].z.wrapping_add(self.ent[t].f78 as i16),
        );
        let yaw = Self::angle_between(px, py, tx, ty);
        let dh = Self::isqrt(Self::dist2_sq(px, py, tx, ty) as u32) as i32;
        let pitch = Self::pitch_toward(pz, tz, dh);
        let e = &mut self.ent[i];
        let ty_ = Self::turn_step(e.f30, yaw, 34);
        e.f30 = (e.f30 as i32 + ty_ as i32) as u16 & 0x7FF;
        e.f32 = pitch;
    }

    /// sub_11AC0 (:17033): the possess victim scan — class-10 models
    /// 39/40/45 only, not the shooter's own or already-claimed, AABB.
    /// The Mana Magnet bolt (m17) instead gets retail's model-39-ONLY
    /// sibling scan (sub_11C00 :17083-121) — it must not detonate on
    /// graves or dwelling flags (player-certified, and the magnet's
    /// own gather tick filters +65==39 the same way, :31252).
    fn possess_victim_at(&mut self, i: usize, tmp: (u16, u16, i16)) -> Option<usize> {
        let old = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        self.ent[i].x = tmp.0;
        self.ent[i].y = tmp.1;
        self.ent[i].z = tmp.2;
        let own = self.ent[i].id24;
        let balls_only = self.ent[i].model65 == 17;
        let mut found = None;
        let r = ((self.ent[i].f80 as i32 + 255) >> 8).max(1);
        'scan: for dy in -r..=r {
            for dx in -r..=r {
                let tx = ((tmp.0 >> 8) as i32 + dx) as u8;
                let ty = ((tmp.1 >> 8) as i32 + dy) as u8;
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                while j != 0 {
                    let c = &self.ent[j];
                    if c.flags & 8 != 0
                        && c.class64 == 10
                        && (c.model65 == 39 || (!balls_only && matches!(c.model65, 40 | 45)))
                        && c.id24 != own
                        && c.f144 != own
                        && self.ent_overlap(i, j)
                    {
                        found = Some(j);
                        break 'scan;
                    }
                    j = c.next20 as usize;
                }
            }
        }
        self.ent[i].x = old.0;
        self.ent[i].y = old.1;
        self.ent[i].z = old.2;
        found
    }

    /// sub_53DC0 (:63628): the storm-carrier flight (c9 m12) — the
    /// Lightning Storm's projectile. Speed eases ±2, homes on an
    /// acquired class-3 target (none exist for us yet → straight
    /// flight); on ANY end but water it becomes the (10,38) storm
    /// cloud, passing owner/heading/victim/damage and the (9,9)
    /// bolt spec down (:63767-83).
    fn proj_m12_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let e = &mut self.ent[i];
        e.f126 += (e.f128 - e.f126).clamp(-2, 2);
        if self.ent[i].f146 != 0 {
            self.home(i, ctx);
        }
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let hit = self.victim_scan_at(i, tmp, ctx);
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        let grounded = ground > tmp.2;
        self.move_relink(i, tmp.0, tmp.1, if grounded { ground } else { tmp.2 });
        self.ent[i].act_life -= 1;
        if hit.is_none() && !grounded && self.ent[i].act_life >= 0 {
            return false;
        }
        if grounded && self.on_water_pub(tmp.0, tmp.1) {
            self.splash_and_die(i); // stormless water end (:63704)
            return false;
        }
        let (x, y, z, own, f44, f30, f32) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f44, e.f30, e.f32)
        };
        if let Some(s) = self.spawn_effect(38, x, y, z) {
            let e = &mut self.ent[s];
            e.id24 = own;
            e.f30 = f30;
            e.f32 = f32;
            e.f44 = f44;
            e.f68 = 9;
            e.f69 = 9;
            e.f146 = match hit {
                Some(MailTarget::Pool(j)) => j as u16,
                Some(MailTarget::Player) => PLAYER_TARGET,
                None => 0,
            };
        }
        self.ent[i].flags |= 0x400;
        false
    }

    /// sub_39F40 (:46166): the castle ball (c9 m10) — sprite 18,
    /// speed 384, life 0x2000/384 = 21.
    pub(crate) fn spawn_castle_ball(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(10, 10, x, y, z, 384, 21, 0, 18)
    }

    /// sub_3A040 (:46226): the storm carrier (c9 m12) — sprite 216,
    /// speed 384, life 2048/384 = 5.
    pub(crate) fn spawn_storm_carrier(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(12, 12, x, y, z, 384, 5, 0, 216)
    }

    /// sub_3A270 (:46330): the Wall of Fire bolt (c9 m16, state 17)
    /// — fireball sprite 42, speed 384, life 21. remc1's state table
    /// is truncated before 17; the flight is the sub_53B50 shape =
    /// straight at the aim (the +150 target sits ON the aim line),
    /// exploding into +68/+69 WITHOUT the +44 copy.
    pub(crate) fn spawn_firewall_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.spawn_projectile(16, 17, x, y, z, 384, 21, 0, 42)
    }

    /// The m16 firewall flight (state 17): generic ease + move, no
    /// fire trail, NO +44 copy into the explosion (the napalm keeps
    /// its own 100).
    fn proj_firewall_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let e = &mut self.ent[i];
        e.f126 += (e.f128 - e.f126).clamp(-2, 2);
        // Hidden Worlds turns Fire Storm (spell 20) into a homing meteor:
        // the m16 child is acquire-switch case 0x10 (remc1hw :60322) —
        // while untargeted it scans awake entities within a WIDENED yaw
        // cone 0x100 (pitch stays 0x71) and homes on the pick. Base MC1
        // has no case 16, so f146 stays 0 and the child flies straight
        // (the fire-rain wall). Seamed on TargetingVerb::Mc1Hw; every
        // other acquire site treats HW exactly as MC1. SURVEY-MC1HW §3a.
        //
        // Acquisition is ONE-SHOT, latched on flags bit 2 even on a
        // miss (remc1hw :58731-49): a miss flies straight forever, a
        // hit SNAPS the live heading to the pick (f30/f32 = f34/f36,
        // :58742-43). Only the post-lock tracker eases (sub_52550,
        // :58754 = home()). Same idiom as the m9 beam (proj_m9_tick).
        // The latch stays inside the HW gate so the shared MC1 path
        // never writes flags bit 2.
        if self.is_hidden_worlds() && self.ent[i].f146 == 0 && self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.aim_assist_mc1_cone(i, ctx, 0x100, 0x71);
            if self.ent[i].f146 != 0 {
                self.ent[i].f30 = self.ent[i].f34;
                self.ent[i].f32 = self.ent[i].f36;
            }
        }
        if self.ent[i].f146 != 0 {
            self.home(i, ctx);
        }
        self.proj_move_and_hit(i, ctx, false)
    }

    /// sub_53980/sub_53B50 (:63453/:63525): the castle ball's flight
    /// — steered at the +150 ground target (dest_x/dest_y). The
    /// LAUNCH tick re-runs the placement scan at the cast spot; a
    /// failure is a silent despawn (:63617-21). On landing the scan
    /// runs again: a failure flips the heading 180° and steps back
    /// once, then the class-3 m2 castle is created anyway
    /// (:63590-606). APPROX: snap-steer in place of the original's
    /// eased turn.
    fn proj_castle_ball_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let _ = ctx;
        // The UPGRADE variant (+69 = 43, :65904-08) skips the
        // placement scans — it flies at the OWN castle and morphs
        // into the (10,43) token there.
        let upgrade = self.ent[i].f69 == 43;
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            if !upgrade && !self.castle_site_ok(i, x, y) {
                self.ent[i].flags |= 0x400;
                return false;
            }
        }
        let (px, py, pz) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (dx, dy) = (self.ent[i].dest_x, self.ent[i].dest_y);
        let tz = self.ground_z(dx, dy) as i16;
        // EASED steering (sub_53B50 :63548-65 via sub_422A0 with the
        // behavior-row caps): the ball leaves along the wizard's aim
        // and turns toward the ground target at row-0 rates — the aim
        // pitch shapes the early arc (NOT snap-steer, which ignores
        // the aim).
        let tgt_yaw = Self::angle_between(px, py, dx, dy);
        let dh = Self::isqrt(Self::dist2_sq(px, py, dx, dy) as u32) as i32;
        let tgt_pitch = Self::pitch_toward(pz, tz, dh);
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let (v2, v6) = (row.v_2, row.v_6);
        {
            let e = &mut self.ent[i];
            e.f34 = tgt_yaw;
            e.f36 = tgt_pitch;
            let ty = Self::turn_step(e.f30, tgt_yaw, v2);
            e.f30 = (e.f30 as i32 + ty as i32) as u16 & 0x7FF;
            let tp = Self::turn_step(e.f32, tgt_pitch, v6);
            e.f32 = (e.f32 as i32 + tp as i32) as u16 & 0x7FF;
        }
        let (yaw, pitch) = (self.ent[i].f30, self.ent[i].f32);
        let mut tmp = (px, py, pz);
        let speed = self.ent[i].f126;
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        let mut grounded = ground > tmp.2;
        self.move_relink(i, tmp.0, tmp.1, if grounded { ground } else { tmp.2 });
        // The with-castle flight lands on OVERLAP with the linked
        // castle — the ball snaps onto it and morphs (:63484-88);
        // the castle's 0x4000 z-extent makes any overflight count.
        if upgrade {
            let c = self.ent[i].f146 as usize;
            if c != 0
                && self.ent[c].class64 == 3
                && self.ent[c].flags & 0x400 == 0
                && self.ent_overlap(i, c)
            {
                let (cx, cy, cz) = (self.ent[c].x, self.ent[c].y, self.ent[c].z);
                self.move_relink(i, cx, cy, cz);
                tmp = (cx, cy, cz);
                grounded = true;
            }
        }
        self.ent[i].act_life -= 1;
        if grounded || self.ent[i].act_life < 0 {
            let own = self.ent[i].id24;
            if upgrade {
                // Morph into the (10,43) upgrade token at the castle
                // (the token mails the castle's ch5 on touch).
                let (z, link) = (self.ent[i].z, self.ent[i].f146);
                if let Some(t) = self.spawn_creator(43, tmp.0, tmp.1, z) {
                    self.ent[t].id24 = own;
                    self.ent[t].f146 = link;
                }
                self.ent[i].flags |= 0x400;
                return false;
            }
            let (mut bx, mut by) = (tmp.0, tmp.1);
            if !self.castle_site_ok(i, bx, by) {
                let back = yaw.wrapping_add(0x400) & 0x7FF;
                let mut t = (bx, by, 0i16);
                Self::polar_step(&mut t, back, 0, speed);
                bx = t.0;
                by = t.1;
            }
            if let Some(c) = self.spawn_castle(bx, by) {
                self.ent[c].id24 = own;
                // Claim owner (+144) — the mana census counts the
                // castle's stored mana into the owner's ceiling.
                self.ent[c].f144 = own;
            }
            self.ent[i].flags |= 0x400;
        }
        false
    }

    /// sub_12F70 (:17786): the castle placement scan — fails when
    /// another castle (c3 m2) is within extents+2048 on both axes,
    /// or any tile of the 8x8 block at (tx-8..tx-1, ty-8..ty-1) —
    /// the original's asymmetric window, ported verbatim — carries
    /// the protection bit.
    pub(crate) fn castle_site_ok(&self, i: usize, x: u16, y: u16) -> bool {
        let (f80, f82) = (self.ent[i].f80 as i32, self.ent[i].f82 as i32);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if c.class64 == 3
                && c.model65 == 2
                && c.flags & 0x400 == 0
                && wd(c.x, x) < c.f80 as i32 + f80 + 2048
                && wd(c.y, y) < c.f82 as i32 + f82 + 2048
            {
                return false;
            }
        }
        let (tx, ty) = ((x >> 8) as i32, (y >> 8) as i32);
        for dy in -8..0i32 {
            for dx in -8..0i32 {
                if self.t.angle[tile((tx + dx) as u8, (ty + dy) as u8)] & 0x80 != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// sub_37920 (:44229): the class-3 model-2 CASTLE entity —
    /// grid-snapped with (tx+ty) even parity, state 5 machine
    /// (sub-state f59 = 0 → the level-up arm builds level 1),
    /// sprite 177, life 40000. The visible castle is painted
    /// terrain; this entity is the anchor/state machine.
    pub(crate) fn spawn_castle(&mut self, x: u16, y: u16) -> Option<usize> {
        let mut cx = ((x as u32 + 128) >> 8) as u8;
        let cy = ((y as u32 + 128) >> 8) as u8;
        if (cx as u16 + cy as u16) % 2 == 1 {
            cx = cx.wrapping_add(1); // parity snap (:44246-52)
        }
        let (px, py) = ((cx as u16) << 8, (cy as u16) << 8);
        // The build datum: MC1 = the center ground; MC2's ctor
        // (sub_4AA40 EF:33390-99) = 32 x the perimeter-MIN over the
        // BUILD00 row-1 footprint.
        let z = match self.verbs.movement {
            crate::verbs::MovementVerb::Mc2 => self.mc2_castle_site_z(cx, cy),
            _ => self.ground_z(px, py) as i16,
        };
        let s = self.new_event()?;
        {
            let e = &mut self.ent[s];
            e.class64 = 3;
            e.model65 = 2;
            e.tick70 = 5;
            e.f59 = 0;
            e.f26 = 0;
            e.max_life = 40000;
            // Build-site z (+154): the painter/leveler datum. The
            // entity z (+76) is refreshed to live ground per tick —
            // the flag rides the painted tower.
            e.site_z = z;
            // Channel mask (+28 = 33, ch0+ch5 — sub_37920 :44247).
            e.f28 = 33;
        }
        self.link(s, px, py, z);
        self.refill_life(s);
        self.set_sprite(s, 177);
        Some(s)
    }

    /// sub_52770 (:62618): the generic flight (m3 trail bolt) — speed
    /// eases ±2 toward +128, homing, explode copies +44 + victim.
    /// `fire_trail`: m3 drops a damage-suppressed fire-seeder per tick
    /// (:63027-38).
    fn proj_generic_tick(&mut self, i: usize, ctx: &MobCtx, fire_trail: bool) -> bool {
        let e = &mut self.ent[i];
        e.f126 += (e.f128 - e.f126).clamp(-2, 2);
        // The generic flight re-acquires while untargeted (:62652 →
        // sub_54520); the meteor's m3 is an acquire case (block
        // 0/3/4) — the retail meteor SNAPS to a bee in the cone and
        // the blast ring does the cluster.
        if self.ent[i].f146 == 0 {
            self.aim_assist(i, ctx);
        }
        if self.ent[i].f146 != 0 {
            self.home(i, ctx);
        }
        if fire_trail {
            let (x, y, z, owner) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24)
            };
            if let Some(s) = self.spawn_effect(1, x, y, z) {
                // +16|=0x80, +18|=1: the seeder's fires inherit the
                // no-damage bit — a decorative trail (:63033-38).
                self.ent[s].flags |= 0x80 | 0x10000;
                self.ent[s].id24 = owner;
            }
        }
        self.proj_move_and_hit(i, ctx, true)
    }

    /// sub_530C0 (:63048): m11's bolt — explodes ONLY on wizard-family
    /// victims (class 3 model ≤ 1 / the player); every other end is a
    /// silent despawn (:63188-210).
    fn proj_m8_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let e = &mut self.ent[i];
        e.f126 += (e.f128 - e.f126).clamp(-2, 2);
        if self.ent[i].f146 != 0 {
            self.home(i, ctx);
        }
        // Move.
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        if let Some(v) = self.victim_scan_at(i, tmp, ctx) {
            let wizard = match v {
                MailTarget::Player => true,
                MailTarget::Pool(j) => self.ent[j].class64 == 3 && self.ent[j].model65 <= 1,
            };
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
            if wizard {
                self.proj_explode(i, ctx, Some(v), true);
            } else {
                self.ent[i].flags |= 0x400;
            }
            return false;
        }
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        if ground <= tmp.2 {
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life < 0 {
                self.ent[i].flags |= 0x400; // silent timeout
            }
        } else if self.on_water_pub(tmp.0, tmp.1) {
            self.splash_and_die(i);
        } else {
            self.ent[i].flags |= 0x400; // silent ground end
        }
        false
    }

    /// sub_535E0 (:63272): the lightning BEAM — resolves in ONE tick.
    /// The flight walks to termination inside the handler in 384-unit
    /// steps (life counts STEPS, not ticks; victim snap / terrain
    /// stop / expiry; NO water splash, NO deflection), then the beam
    /// redraws itself as a chain of short-lived state-14 segment
    /// entities along a ±1 random walk (8 sub-steps per flight step)
    /// and explodes at the segment-walk endpoint. The kraken fires
    /// one beam per burst tick — a beam re-laid every tick, not a
    /// traveling ball.
    fn proj_m9_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].f126 = self.ent[i].f128;
        let spawn = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        // sub_534C0 (:63216): one-time aim assist only while
        // untargeted (+146 == 0 — fire sites that pre-lock +146 also
        // pre-aim +30/+32 at the target, closing this gate); snap to
        // the acquired angles, no per-tick easing, no homing ever.
        // The snap runs inside the FIRST flight call (:63312), BEFORE
        // the chain heading is saved.
        if self.ent[i].f146 == 0 && self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.aim_assist(i, ctx);
            if self.ent[i].f146 != 0 {
                self.ent[i].f30 = self.ent[i].f34;
                self.ent[i].f32 = self.ent[i].f36;
            }
        }
        // Yaw/pitch are saved AFTER the snap (:63313-14) and restored
        // for the segment chain (:63327-28) — the visible chain and
        // the endpoint explosion follow the AIMED heading, which is
        // why the retail bolt points at (and lands on) its victim.
        let (yaw0, pitch0) = (self.ent[i].f30, self.ent[i].f32);
        let mut steps: i32 = 0;
        let mut hit: Option<MailTarget> = None;
        loop {
            steps += 1;
            let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            let (yaw, pitch, speed) = {
                let e = &self.ent[i];
                (e.f30, e.f32, e.f126)
            };
            Self::polar_step(&mut tmp, yaw, pitch, speed);
            if let Some(v) = self.victim_scan_at(i, tmp, ctx) {
                // Snap to the victim's exact position — no +78
                // half-height, unlike the fireball (:63252-56).
                match v {
                    MailTarget::Pool(j) => {
                        let (jx, jy, jz) = (self.ent[j].x, self.ent[j].y, self.ent[j].z);
                        self.move_relink(i, jx, jy, jz);
                    }
                    MailTarget::Player => self.move_relink(i, ctx.px, ctx.py, ctx.pz),
                }
                hit = Some(v);
                break;
            }
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
            if self.ground_z(tmp.0, tmp.1) as i16 > tmp.2 {
                break; // terrain stop — sub_534C0 has no water case
            }
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life < 0 {
                break; // expired midair (≤ 10 steps for life 9)
            }
        }
        self.ent[i].f30 = yaw0;
        self.ent[i].f32 = pitch0;
        // ---- the segment chain (:63329-63420): 8·steps+1 segments
        // along the straight spawn-heading path, sub-step = speed/8.
        let beam_slot = i;
        let owner = self.ent[i].id24;
        let substep = self.ent[i].f126 / 8; // v33 = 48
        let scale = (substep / 4) as i32; // offset unit = 12
        let mut delta = (0u16, 0u16, 0i16);
        Self::polar_step(&mut delta, yaw0, pitch0, substep);
        let mut base = spawn;
        let mut disp = spawn;
        let (mut v32, mut v31): (i32, i32) = (0, 0);
        let mut v30 = steps * 8;
        loop {
            if let Some(s) = self.new_event() {
                // NewEvent defaults kept (hittable bit SET, speed 16,
                // +44 100, filter -1). Slot-order life: a slot that
                // ticks later this frame gets 0, an already-ticked
                // one -1 — one rendered frame each under the
                // state-14 pre-decrement test (:63345-56).
                {
                    let e = &mut self.ent[s];
                    e.class64 = 9;
                    e.model65 = 9;
                    e.tick70 = 14;
                    e.id24 = owner;
                }
                self.link(s, disp.0, disp.1, disp.2);
                self.set_sprite(s, 216);
                self.ent[s].act_life = if s >= beam_slot { 0 } else { -1 };
            }
            // Amplitude pinches toward the endpoint (:63358-62).
            let amp = (v30 / 2).clamp(0, 8);
            // Offset walk v32 (applied) then phantom walk v31 (its
            // draws only advance the RNG — confirmed in BOTH
            // decompiles, remc2 sub_66750): ±1 steps with p(+1) =
            // 78/157; draws CONDITIONAL on being inside ±amp, out-of-
            // band offsets pull back deterministically (:63363-92).
            for w in [&mut v32, &mut v31] {
                if *w <= amp {
                    if *w >= -amp {
                        let d = self.ent_rand(i);
                        *w += 2 * ((d % 0x9D) / 79) as i32 - 1;
                    } else {
                        *w += 1;
                    }
                } else {
                    *w -= 1;
                }
            }
            // Advance; the display point offsets by v32·12 in BOTH z
            // and the yaw+0x200 horizontal perpendicular — a diagonal
            // zigzag plane, max ±96 units (:63394-412).
            base.0 = base.0.wrapping_add(delta.0);
            base.1 = base.1.wrapping_add(delta.1);
            base.2 = base.2.wrapping_add(delta.2);
            let off = (v32 * scale) as i16;
            disp = (base.0, base.1, base.2.wrapping_add(off));
            let mut p = (disp.0, disp.1, 0i16);
            Self::polar_step(&mut p, yaw0.wrapping_add(0x200) & 0x7FF, 0, off);
            disp.0 = p.0;
            disp.1 = p.1;
            v30 -= 1;
            if v30 < 0 {
                break;
            }
        }
        // ---- endpoint (:63421-49) ----
        let (f69, f44, f140, f146) = {
            let e = &self.ent[i];
            (e.f69, e.f44, e.f140, e.f146)
        };
        // Accuracy stats sub_526C0 (:62585): human-owned shots only.
        if owner == PLAYER_TARGET {
            self.shots += 1;
            if hit.is_some_and(|s| match s {
                MailTarget::Pool(j) => f146 == self.ent[j].id24 || f146 == j as u16,
                MailTarget::Player => false,
            }) {
                self.hits += 1;
            }
        }
        // Enhanced-lightning presentation feed: the resolved strike,
        // muzzle → chain endpoint (hash-silent, drained by the
        // frontend).
        if self.bolt_fx.0.len() < 256 {
            self.bolt_fx.0.push(crate::engine::features::BoltStrike {
                start: spawn,
                end: disp,
                owner,
            });
        }
        // The explosion lands at the SEGMENT-WALK endpoint, not the
        // beam's snapped position. Shielded (+17 bit7) class-3
        // victims with mana ≥ +140/4 quarter the payload — no drain,
        // no deflection (:63435-47). +146: the original stamps
        // garbage when nothing was hit (remc2 guards this) — we
        // stamp hit-or-0, flagged deviation.
        if let Some(fx) = self.spawn_effect(f69, disp.0, disp.1, disp.2) {
            let quartered = match hit {
                Some(MailTarget::Pool(j)) => {
                    self.ent[j].flags & 0x8000 != 0
                        && self.ent[j].class64 == 3
                        && f140 / 4 <= self.ent[j].f140
                }
                _ => false, // player shields = the spell track
            };
            let e = &mut self.ent[fx];
            e.id24 = owner;
            e.f30 = yaw0;
            e.f32 = pitch0;
            e.f146 = match hit {
                Some(MailTarget::Pool(j)) => j as u16,
                Some(MailTarget::Player) => PLAYER_TARGET,
                None => 0,
            };
            e.f44 = if quartered { f44 >> 2 } else { f44 };
        }
        self.ent[i].flags |= 0x400;
        false
    }

    /// Class-9 m14 / **state 15** (`sub_3A1A0` :46281) — the Troll and
    /// Ape boulder (class-5 m7's throw, `sub_1AE30` :22101). Its own
    /// flight state, NOT the arrow's.
    ///
    /// The boulder is SILENT in flight; the only sound it makes is its
    /// impact. Retail's arrow roll (ids 33-36 = `arrow1`..`arrow4`)
    /// lives solely in state 13's `sub_54180` (:63799 — the binary's
    /// ONLY emitter of those four ids), and this is state 15. Proof
    /// they are different handlers: `sub_1AE30` writes the impact
    /// descriptor `+68 = 10` / `+69 = 0` (:22103-04), which state 13
    /// never reads — dead stores otherwise. So the throw speaks
    /// through its `(10,0)` impact (`sub_3A490` :46454), whose tick
    /// plays sound 3 (:28114).
    ///
    /// APPROX(original: state 15's table entry is NOT transcribed —
    /// remc1's class-9 tick table `str_25573C` (:4838) stops at state
    /// 0x0D while its address span holds 22 entries, the only short
    /// table in the block; the best-fit orphan is `sub_542B0_54640`
    /// :63841). Two deliberate departures from that orphan, registered
    /// in docs/DEVIATIONS.md: the flight stays STRAIGHT (the orphan
    /// steers toward `+146`), and the impact inherits the thrown
    /// `+44 = 780` (:22112) instead of the `(10,0)` default 400 — the
    /// transcribed 780 write would otherwise be a dead store.
    fn proj_boulder_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        let hit = self.victim_scan_at(i, tmp, ctx);
        let grounded = ground > tmp.2;
        self.move_relink(i, tmp.0, tmp.1, if grounded { ground } else { tmp.2 });
        self.ent[i].act_life -= 1;
        if hit.is_some() || grounded || self.ent[i].act_life < 0 {
            self.proj_explode(i, ctx, hit, true);
        }
        false
    }

    /// sub_54180 (:63789): the straight bolt (m13) — first-tick LCG
    /// sound roll (the `arrow1`..`arrow4` quartet), direct ch0 area
    /// write on any end. Retail reuses the arrow samples across every
    /// user of this state — the skeleton/archer creatures m4/m9/m10
    /// and the castle guard m15 — including m9, whose projectile wears
    /// a different billboard (sprite 203, :21947). That reuse IS
    /// faithful; only the boulder was wrongly borrowing it.
    fn proj_bolt_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            let d = self.ent_rand(i); // :63795
            self.snd(33 + (d & 3) as u8, i);
        }
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        let hit = self.victim_scan_at(i, tmp, ctx).is_some();
        let grounded = ground > tmp.2;
        self.move_relink(i, tmp.0, tmp.1, if grounded { ground } else { tmp.2 });
        self.ent[i].act_life -= 1;
        if hit || grounded || self.ent[i].act_life < 0 {
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 0, amt, ctx, false, false);
            self.ent[i].flags |= 0x400;
        }
        false
    }

    /// The player-spell payload flight. APPROX(original: c9 m1/m2/m4/
    /// m5/m7/m11/m17 have their own states past remc1's transcribed
    /// table): m13-bolt-shaped straight flight at the cast pitch (the
    /// down-arc arrives via the cast's pitch bias); on any end
    /// (victim / ground / expiry) the struck victim takes the row
    /// damage on ch0 and the per-model payload fires.
    fn proj_payload_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // These states run the engine's generic homing flight
        // (sub_52770): re-acquire while untargeted per the sub_54520
        // subtype switch — m4 (volcano) sits in the 0/3/4 creature
        // block; m7/m11 acquire only wizards (block 7/8/B/C — a no-op
        // until AI wizards land); m2/m5/m17 are default: no acquire.
        // All of them home once +146 holds a target.
        if self.ent[i].model65 == 4 && self.ent[i].f146 == 0 {
            self.aim_assist(i, ctx);
        }
        if matches!(self.ent[i].model65, 7 | 11) && self.ent[i].f146 == 0 {
            self.aim_assist_wizards(i, ctx);
        }
        if self.ent[i].f146 != 0 {
            self.home(i, ctx);
        }
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        let hit = self.victim_scan_at(i, tmp, ctx);
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        let grounded = ground > tmp.2;
        self.move_relink(i, tmp.0, tmp.1, if grounded { ground } else { tmp.2 });
        self.ent[i].act_life -= 1;
        if hit.is_some() || grounded || self.ent[i].act_life < 0 {
            if let Some(MailTarget::Pool(j)) = hit {
                let amt = self.ent[i].f44 as u32;
                let src = self.ent[i].id24;
                self.mail_write(MailTarget::Pool(j), 0, amt, src);
            }
            self.spell_payload(i);
            self.ent[i].flags |= 0x400;
        }
        false
    }

    /// The per-model detonation payloads of the player-spell
    /// projectiles (each cite = the traced cast arm's effect).
    fn spell_payload(&mut self, i: usize) {
        let (x, y, z, model) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.model65)
        };
        let gz = self.ground_z(x, y) as i16;
        let own = self.ent[i].id24;
        match model {
            // Earthquake (:65314): the authentic (10,15) crevice
            // walker — random start heading off its own LCG, ±45
            // wander, a 10-tick m11 digger per step (the rumble is
            // the diggers' loop-10).
            2 => {
                if let Some(w) = self.spawn_creator(15, x, y, gz) {
                    self.ent[w].id24 = own;
                }
            }
            // Volcano (:65432): the growing hill + pit IS the
            // authentic model (trace :65466, effect c10 m9); the
            // finished cone spawns the model-18 eruption driver
            // ([`Gen::eruption_tick`]).
            4 => {
                if let Some(h) = self.spawn_creator(9, x, y, gz) {
                    self.ent[h].id24 = own;
                }
            }
            // Crater (:65491): the expanding bowl (authentic:
            // effect c10 m11).
            5 => {
                if let Some(c) = self.spawn_creator(11, x, y, gz) {
                    self.ent[c].id24 = own;
                }
            }
            // Duel to the Death (:65620 → (10,26) ctor :47116): the
            // tether follows the homed wizard and broadcasts the ch4
            // grip 200/tick (sub_263C0 :28949). No wizard target →
            // the bolt ends in a hit flash.
            7 => {
                let victim = self.ent[i].f146;
                let is_wizard = victim == crate::mc1::mobs::PLAYER_TARGET
                    || (victim != 0
                        && self.ent[victim as usize].class64 == 3
                        && self.ent[victim as usize].model65 <= 1);
                if is_wizard {
                    if let Some(t) = self.spawn_effect(26, x, y, z) {
                        self.ent[t].id24 = own;
                        self.ent[t].f146 = victim;
                        self.ent[t].f44 = 200;
                    }
                } else if let Some(f) = self.spawn_effect(23, x, y, z) {
                    self.ent[f].id24 = own;
                }
            }
            // Undead Army (:65927 → the (10,36) spawner sub_26E90
            // :29353): up to 8 class-5 model-9 SKELETONS on a
            // 512-unit ring (angles k·2048/N, facing radial+180°),
            // zero mana (no corpse balls, :29672 gate), capped at 64
            // live skeletons per owner (:29375-81). Owner goes on
            // BOTH +24 and +144 — remc1 writes only +144 (:29399),
            // which would turn gen-1 skeletons on their caster;
            // transcription-slip suspicion beside the :29366
            // hardcode (converted skeletons DO get +24, :23913).
            // Deferred: the human→skeleton conversion AI arm.
            11 => {
                let live = (1..self.ent.len())
                    .filter(|&j| {
                        let c = &self.ent[j];
                        c.class64 == 5 && c.model65 == 9 && c.flags & 0x400 == 0 && c.f144 == own
                    })
                    .count() as i32;
                let n = 8i32.min(64 - live).max(0);
                for k in 0..n {
                    let ang = ((k * (2048 / n)) as u16) & 0x7FF;
                    let mut pos = (x, y, 0i16);
                    Self::polar_step(&mut pos, ang, 0, 512);
                    let sz = self.ground_z(pos.0, pos.1) as i16;
                    if let Some(s) = self.spawn_creature(9, pos.0, pos.1, sz) {
                        let facing = ang.wrapping_add(0x400) & 0x7FF;
                        let e = &mut self.ent[s];
                        e.id24 = own;
                        e.f144 = own;
                        e.f140 = 0;
                        e.f30 = facing;
                        e.f34 = facing;
                    }
                }
            }
            _ => {}
        }
    }

    /// sub_25EC0 (:28731): the volcano eruption driver (m18, state
    /// 18). Counter +26 runs the machine; maxLife (10000) never
    /// counts down:
    /// - counter 0: eruption start — always activates, registers as
    ///   THE erupting volcano (kicking any previous one to counter
    ///   250), swaps the global (10,19) plume, and fires the
    ///   once-per-eruption blast fireball ((10,17) payload, pitch
    ///   -386, life 1) at the rotating heading (:28778-823).
    /// - counters 1..126: activate at p=1/5, except every 16th tick
    ///   (counter&0xF == 0) which never does (:28768-71). Every
    ///   activation lobs ONE ballistic (10,16) lava bomb and turns
    ///   the heading by 0x500 (:28795-804).
    /// - an activation at 127 is the CLEAN death: clears the global
    ///   register (:28825-29). Missing that 1/5 roll leaves the
    ///   register pointing at a dead-idle volcano — the authentic
    ///   no-more-eruptions-anywhere quirk.
    /// - counter > 2500: dormant; p=1/100 per tick to re-arm to 0,
    ///   only while NO volcano is registered (:28750-66).
    /// - every activation (and every re-arm) dies instead if the
    ///   ground height under the driver changed (:28773-77).
    ///
    /// No driver-level sound: eruption audio = the bombs' seeded
    /// fires (crackle 3) + the blast ring (30).
    fn eruption_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let _ = ctx;
        let c = self.ent[i].f26;
        let (x, y, z, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        if c > 2500 {
            let d = self.ent_rand(i);
            if d % 100 == 0 && self.erupting == 0 {
                if self.ground_z(x, y) as i16 != z {
                    self.ent[i].flags |= 0x400;
                    return false;
                }
                self.ent[i].f26 = 0;
            } else if self.ent[i].f26 < i16::MAX - 1 {
                self.ent[i].f26 = c + 1;
            }
            return false;
        }
        let fire = if c != 0 && c < 128 && c & 0xF != 0 {
            self.ent_rand(i) % 5 == 0
        } else {
            c == 0
        };
        if fire {
            if self.ground_z(x, y) as i16 != z {
                self.ent[i].flags |= 0x400; // deformed under: dead
                return false;
            }
            if c == 0 {
                // Register self; kick the previous eruption (:28778-92).
                let prev = self.erupting as usize;
                if prev != 0 && self.ent[prev].class64 == 10 && self.ent[prev].model65 == 18 {
                    self.ent[prev].f26 = 250;
                }
                self.erupting = i as u16;
                let pl = self.plume as usize;
                if pl != 0 && self.ent[pl].class64 == 10 && self.ent[pl].model65 == 19 {
                    self.ent[pl].flags |= 0x400;
                }
                let g = self.ground_z(x, y) as i16;
                self.plume = match self.spawn_effect(19, x, y, g) {
                    Some(p) => {
                        self.ent[p].id24 = own;
                        p as u16
                    }
                    None => 0,
                };
            }
            // One ballistic lava bomb per activation (:28795-801):
            // owner AND the driver's LCG seed pass on.
            let seed = self.ent[i].rand;
            if let Some(b) = self.spawn_lava_bomb(x, y) {
                self.ent[b].id24 = own;
                self.ent[b].rand = seed;
            }
            // Heading advances 0x500 per activation (:28804).
            self.ent[i].f30 = self.ent[i].f30.wrapping_add(0x500);
            if c == 0 {
                // The eruption-start blast fireball (:28805-23):
                // pitch -386, life 1, detonates into the (10,17)
                // fire-field. APPROX: the +150 position-target
                // steering is skipped (aim assist suppressed) — it
                // flies the armed heading.
                let yaw = self.ent[i].f30 & 0x7FF;
                if let Some(p) = self.spawn_fireball(x, y, z) {
                    let e = &mut self.ent[p];
                    e.id24 = own;
                    e.f30 = yaw;
                    e.f34 = yaw;
                    e.f32 = (-386i16 as u16) & 0x7FF;
                    e.f36 = e.f32;
                    e.f68 = 10;
                    e.f69 = 17;
                    e.act_life = 1;
                    e.flags |= 2;
                }
            }
            if c >= 127 {
                self.erupting = 0; // the clean death (:28825-29)
                self.ent[i].flags |= 0x400;
                return false;
            }
        }
        self.ent[i].f26 = c + 1;
        false
    }

    /// sub_3ACC0 (:46958): the (10,16) lava bomb — draws IN ORDER
    /// off its own LCG: life = %100+100, speed = %50 (held), vz =
    /// 256 up, yaw = rand & 0x7FF; speed applies as +52; spawned
    /// map-linked at ground+64 with the horizontal velocity vector
    /// pre-advanced into +150/+152 (our dest_x/dest_y), sprite 210.
    fn spawn_lava_bomb(&mut self, x: u16, y: u16) -> Option<usize> {
        let b = self.new_event()?;
        {
            let e = &mut self.ent[b];
            e.class64 = 10;
            e.model65 = 16;
            e.tick70 = 16;
            e.f44 = 200;
            e.flags = (e.flags & !(8 | 0x20000)) | 0x20000;
            let d1 = lcg32(&mut e.rand);
            e.max_life = d1 % 0x64 + 100;
            let d2 = lcg32(&mut e.rand);
            e.f46 = 256;
            let d3 = lcg32(&mut e.rand);
            e.f30 = (d3 & 0x7FF) as u16;
            e.f126 = (d2 % 0x32) as i16 + 52;
        }
        let gz = (self.ground_z(x, y) + 64) as i16;
        self.link(b, x, y, gz);
        {
            let (yaw, speed) = (self.ent[b].f30, self.ent[b].f126);
            let mut v = (0u16, 0u16, 0i16);
            Self::polar_step(&mut v, yaw, 0, speed);
            let e = &mut self.ent[b];
            e.dest_x = v.0;
            e.dest_y = v.1;
        }
        self.refill_life(b);
        self.set_sprite(b, 210);
        Some(b)
    }

    /// sub_25A60 (:28573): the lava bomb's ballistic flight —
    /// per-axis velocity clamp ±80, gravity -28/tick (vz clamped
    /// [-384, 256]), ground bounce vz = -vz/4, water splash, and at
    /// rest a 30-tick standing fire at 3x damage (if none already
    /// burns on the cell), then downhill roll under 250/256
    /// friction. Slope roll APPROX: central-difference gradient in
    /// place of sub_41F50's table.
    fn lava_bomb_tick(&mut self, i: usize) -> bool {
        // :28592-94 — the life test reads the PRE-decrement value: the
        // whole class-10 effect family is pre-decrement in retail (the
        // class-9 flight handlers genuinely are not), so this runs one
        // more tick than the post-decrement form allows.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let mut vx = (self.ent[i].dest_x as i16).clamp(-80, 80);
        let mut vy = (self.ent[i].dest_y as i16).clamp(-80, 80);
        let mut vz = (self.ent[i].f46 - 28).clamp(-384, 256);
        let (x0, y0, z0) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let x = x0.wrapping_add(vx as u16);
        let y = y0.wrapping_add(vy as u16);
        let mut z = z0.wrapping_add(vz);
        let g = self.ground_z(x, y) as i16;
        let mut grounded = false;
        if z <= g {
            z = g;
            grounded = true;
            if self.on_water_pub(x, y) {
                self.move_relink(i, x, y, z);
                self.splash_and_die(i);
                return false;
            }
            vz = -vz / 4; // bounce (:28625)
            if vz.abs() <= 28 {
                vz = 0;
                // Seed a standing fire at rest if the cell has none
                // (:28637-47): life 30, 3x the bomb's 200.
                let mut burning = false;
                let mut j = self.map_entity[tile((x >> 8) as u8, (y >> 8) as u8)] as usize;
                while j != 0 {
                    if self.ent[j].class64 == 10
                        && self.ent[j].model65 == 6
                        && self.ent[j].flags & 0x400 == 0
                    {
                        burning = true;
                        break;
                    }
                    j = self.ent[j].next20 as usize;
                }
                if !burning {
                    let own = self.ent[i].id24;
                    let amt = 3 * self.ent[i].f44;
                    if let Some(f) = self.spawn_effect(6, x, y, z) {
                        self.ent[f].id24 = own;
                        self.ent[f].act_life = 30;
                        self.ent[f].f44 = amt;
                    }
                }
            }
        }
        if grounded {
            // Downhill roll + friction (:28655-67).
            let gxm = self.ground_z(x.wrapping_sub(256), y) as i16;
            let gxp = self.ground_z(x.wrapping_add(256), y) as i16;
            let gym = self.ground_z(x, y.wrapping_sub(256)) as i16;
            let gyp = self.ground_z(x, y.wrapping_add(256)) as i16;
            vx = (vx + (gxm - gxp) / 8).clamp(-80, 80);
            vy = (vy + (gym - gyp) / 8).clamp(-80, 80);
            vx = ((250 * vx as i32) >> 8) as i16;
            vy = ((250 * vy as i32) >> 8) as i16;
        }
        let e = &mut self.ent[i];
        e.dest_x = vx as u16;
        e.dest_y = vy as u16;
        e.f46 = vz;
        self.move_relink(i, x, y, z);
        false
    }

    /// The (10,19) eruption plume: a 240-tick flame-family visual
    /// riding the crater. APPROX(state-19 handler untraced): life
    /// countdown + animation only.
    fn plume_tick(&mut self, i: usize) -> bool {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
            if self.plume == i as u16 {
                self.plume = 0;
            }
            return false;
        }
        self.anim_advance(i);
        false
    }

    /// sub_26D20 (:29279), state 40: the lightning STORM cloud.
    /// Rises 64/tick until 1024 above the terrain (doing nothing
    /// else while climbing), then holds that altitude and fires TWO
    /// (9,9) bolts per tick in opposite random directions (pitch 56
    /// down, yaw flipped 0x400 between them), each with a third of
    /// the bolt life, the storm's 2000 damage, and the (10,23)
    /// endpoint flash; thunder 23 per firing tick. Life 32 ticks of
    /// fire (~66 bolts).
    fn storm_cloud_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let _ = ctx;
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let g = self.ground_z(x, y) as i16;
        if z < g.wrapping_add(1024) {
            let nz = z.wrapping_add(64);
            self.move_relink(i, x, y, nz);
            return false;
        }
        self.move_relink(i, x, y, g.wrapping_add(1024));
        // :29311-13 — PRE-decrement life test, as across the whole
        // class-10 effect family: 33 bolt ticks, not 32.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let d = self.ent_rand(i);
        self.ent[i].f32 = 56;
        self.ent[i].f30 = (d & 0x7FF) as u16;
        for _ in 0..2 {
            // Yaw flips 180° BEFORE each launch (:29321-23).
            self.ent[i].f30 = self.ent[i].f30.wrapping_add(0x400) & 0x7FF;
            let (yaw, pitch, f44, own, hh) = {
                let e = &self.ent[i];
                (e.f30, e.f32, e.f44, e.id24, e.f78 as i16)
            };
            let (bx, by, bz) = (self.ent[i].x, self.ent[i].y, self.ent[i].z.wrapping_add(hh));
            if let Some(b) = self.spawn_zigzag(bx, by, bz) {
                let e = &mut self.ent[b];
                e.id24 = own;
                e.act_life /= 3; // shorter beams (:29334)
                e.f30 = yaw;
                e.f34 = yaw;
                e.f32 = pitch;
                e.f36 = pitch;
                e.f68 = 10;
                e.f69 = 23;
                e.f44 = f44;
            }
        }
        self.snd(23, i); // :29343
        false
    }

    /// sub_25760 (:28426), state 12: the possess detonation — a ch1
    /// claim broadcast every tick of its 8-tick life over the 512
    /// extents; balls and built houses consume the SENDER field.
    fn possess_flash_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // :28433-36 — the life test reads the PRE-decrement value: the
        // whole class-10 effect family is pre-decrement in retail (the
        // class-9 flight handlers genuinely are not), so this runs one
        // more tick than the post-decrement form allows.
        // :28432 — retail bumps +26 every tick, BEFORE the life test, so
        // it counts even on the tick the flash dies.
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        // :28437 — the anim step runs before the ch1 write.
        self.anim_advance(i);
        let amt = self.ent[i].f44 as u32;
        self.area_write(i, 1, amt, ctx, false, false);
        false
    }

    /// Move + hit scan + terrain shared by m0/m3/m9 (:62842-932).
    /// Returns terrain_dirty (always false here — craters come from
    /// the explosion).
    fn proj_move_and_hit(&mut self, i: usize, ctx: &MobCtx, copy_f44: bool) -> bool {
        let mut tmp = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        Self::polar_step(&mut tmp, yaw, pitch, speed);
        if let Some(v) = self.victim_scan_at(i, tmp, ctx) {
            // Rebound (+17 bit 7): mana-shield deflection (:62858-90).
            // The human carpet's bit is the Rebound spell (14, :65774
            // — the ported deflection-bit semantics).
            let rebound = match v {
                MailTarget::Pool(j) => self.ent[j].flags & 0x8000 != 0,
                MailTarget::Player => self.player_rebound,
            };
            if rebound {
                self.snd(28, i); // deflection twang (:62880)
                match v {
                    MailTarget::Pool(j) => {
                        let quarter = (self.ent[i].f140 / 4).max(0);
                        if quarter <= self.ent[j].f140 {
                            self.ent[j].f140 -= quarter;
                            let deflector_id = self.ent[j].id24;
                            let shooter = self.ent[i].id24;
                            let d = self.ent_rand(i);
                            let e = &mut self.ent[i];
                            e.f34 = e.f30.wrapping_add(0x400) & 0x7FF;
                            e.f30 = (e.f34 as i32 + (d % 0x5B) as i32 - 45) as u16 & 0x7FF;
                            e.f32 = e.f32.wrapping_neg() & 0x7FF;
                            e.f146 = if shooter == PLAYER_TARGET {
                                PLAYER_TARGET
                            } else {
                                shooter
                            };
                            e.id24 = deflector_id;
                            e.act_life = e.max_life as i32;
                            let (jx, jy, jz) = (self.ent[j].x, self.ent[j].y, self.ent[j].z);
                            self.move_relink(i, jx, jy, jz);
                            return false;
                        }
                    }
                    MailTarget::Player => {
                        // The projectile reverses heading and swaps
                        // owner to the player, re-homing on its
                        // shooter. INTERIM: no mana-economy debit on
                        // the player pool (the original quarters the
                        // projectile's +140 against the shield pool).
                        let shooter = self.ent[i].id24;
                        let d = self.ent_rand(i);
                        let e = &mut self.ent[i];
                        e.f34 = e.f30.wrapping_add(0x400) & 0x7FF;
                        e.f30 = (e.f34 as i32 + (d % 0x5B) as i32 - 45) as u16 & 0x7FF;
                        e.f32 = e.f32.wrapping_neg() & 0x7FF;
                        e.f146 = shooter;
                        e.id24 = PLAYER_TARGET;
                        e.act_life = e.max_life as i32;
                        self.move_relink(i, ctx.px, ctx.py, ctx.pz);
                        return false;
                    }
                }
            }
            // Teleport onto the victim, explode there (:62852-55).
            match v {
                MailTarget::Pool(j) => {
                    let (jx, jy, jz) = (
                        self.ent[j].x,
                        self.ent[j].y,
                        self.ent[j].z.wrapping_add(self.ent[j].f78 as i16),
                    );
                    self.move_relink(i, jx, jy, jz);
                }
                MailTarget::Player => {
                    self.move_relink(i, ctx.px, ctx.py, ctx.pz);
                }
            }
            self.proj_explode(i, ctx, Some(v), copy_f44);
            return false;
        }
        let ground = self.ground_z(tmp.0, tmp.1) as i16;
        if ground <= tmp.2 {
            self.move_relink(i, tmp.0, tmp.1, tmp.2);
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life < 0 {
                self.proj_explode(i, ctx, None, copy_f44); // midair expiry
            }
        } else if self.on_water_pub(tmp.0, tmp.1) {
            self.splash_and_die(i); // :62916-21, no explosion/crater
        } else {
            self.proj_explode(i, ctx, None, copy_f44); // terrain hit (pre-move pos)
        }
        false
    }

    /// The victim scan evaluated at a prospective position (the
    /// original moves first and scans at the new position).
    pub(crate) fn victim_scan_at(
        &mut self,
        i: usize,
        tmp: (u16, u16, i16),
        ctx: &MobCtx,
    ) -> Option<MailTarget> {
        let old = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        self.ent[i].x = tmp.0;
        self.ent[i].y = tmp.1;
        self.ent[i].z = tmp.2;
        let v = self.victim_scan(i, ctx);
        self.ent[i].x = old.0;
        self.ent[i].y = old.1;
        self.ent[i].z = old.2;
        v
    }

    fn splash_and_die(&mut self, i: usize) {
        let (x, y, z, owner) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        if let Some(s) = self.spawn_effect(5, x, y, z) {
            self.ent[s].id24 = owner;
        }
        self.ent[i].flags |= 0x400;
    }

    pub(crate) fn on_water_pub(&self, x: u16, y: u16) -> bool {
        self.t.tile_type[(((y >> 8) as usize) << 8) | (x >> 8) as usize] == 0
    }

    // ---- class-10 combat effects -------------------------------------------

    /// The class-10 effect inits (states = the original's +70 writes).
    pub(crate) fn spawn_effect(&mut self, model: u8, x: u16, y: u16, z: i16) -> Option<usize> {
        // On the MC2 column the shared-lineage effects resolve into
        // their NATIVE ctors — the ground fire (0) and the explosion
        // seeder (1) are the same entity in both engines (life 8/1,
        // damage 400, sprite 7/41, extents 128) but tick through the
        // per-game arms (MC2: sub_30D50 worn-path repaints + ring
        // cluster). Without this, an MC1-fallback fireball on an MC2
        // world spawns an MC1-shaped fire that the game-keyed dispatch
        // feeds to the MC2 handler (damage field mismatch → silent
        // fire).
        if matches!(self.verbs.movement, crate::verbs::MovementVerb::Mc2) {
            match model {
                0 => return self.mc2_spawn_fire(x, y, z),
                1 => return self.mc2_spawn_big_explosion(x, y, z),
                _ => {}
            }
        }
        // sub_3B970 (:47672): the (10,54) mana MAGNET — reached here
        // as the Mana Magnet bolt's +69 detonation (:66084-85); the
        // caller stamps the owner like on every effect.
        if model == 54 {
            return self.spawn_mana_magnet(x, y, z, 0);
        }
        let s = self.new_event()?;
        self.ent[s].class64 = 10;
        self.ent[s].model65 = model;
        match model {
            // sub_3A490 (:46454): the fire/explosion. Damage 400.
            0 => {
                let e = &mut self.ent[s];
                e.tick70 = 0;
                e.max_life = 8;
                e.f44 = 400;
                e.f28 = 0;
                e.flags = (e.flags & !(8 | 0x20000)) | 0x20000;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 7);
                self.extents(s, 128, 128);
            }
            // sub_3A510 (:46482): the fire-spreader / corpse flame.
            1 => {
                let e = &mut self.ent[s];
                e.tick70 = 1;
                e.max_life = 1;
                e.f44 = 400;
                e.flags &= !8;
                e.flags |= 0x20000;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 41);
            }
            // The standing fire / ground wave (state 6, sub_252D0):
            // life 240, 50 ch0 per tick via the /10 writer, sprite
            // 228 (the flame-size family +86 walks ±1). Tree deaths
            // override life and set the f46 trunk offset.
            6 => {
                let e = &mut self.ent[s];
                e.tick70 = 6;
                e.max_life = 240;
                e.f44 = 50;
                e.flags &= !8;
                e.flags |= 0x20000;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 228);
            }
            // sub_3A6B0 (:46560 region): the water splash. Grounded.
            5 => {
                let e = &mut self.ent[s];
                e.tick70 = 5;
                e.max_life = 8;
                e.f44 = 0;
                e.flags &= !8;
                e.flags |= 0x20000;
                self.link(s, x, y, z);
                let (px, py) = (self.ent[s].x, self.ent[s].y);
                self.ent[s].z = self.ground_z(px, py) as i16;
                self.refill_life(s);
                self.set_sprite(s, 244);
            }
            // (10,26) ctor (:47116): the duel tether — life 8,
            // sprite row 284, +44 = the 200/tick ch4 grip amount.
            26 => {
                let e = &mut self.ent[s];
                e.tick70 = 26;
                e.max_life = 8;
                e.f44 = 200;
                e.flags &= !8;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 284);
            }
            // sub_3AC70 (:46935): the invisible fire-ring blast driver.
            17 => {
                let e = &mut self.ent[s];
                e.tick70 = 17;
                e.max_life = 10;
                e.f44 = 3000;
                e.flags &= !8;
                self.link(s, x, y, z);
                self.refill_life(s);
            }
            // sub_3AA10 (:46790): the POSSESS detonation flash —
            // an 8-tick ch1 claim broadcast over 512-unit extents.
            // The original's +44 = -1536 is a mana-drain amount the
            // claim readers never consume (they act on the SENDER
            // field alone); our u16 +44 carries 0 — the drain joins
            // the mana-economy track.
            12 => {
                let e = &mut self.ent[s];
                e.tick70 = 12;
                e.max_life = 8;
                e.f44 = 0;
                e.flags &= !8;
                self.link(s, x, y, z);
                self.refill_life(s);
                // The ctor's sub_36FA0(41) — the visible claim
                // sparkle (extents then overridden to 512).
                self.set_sprite(s, 41);
                self.extents(s, 512, 512);
            }
            // sub_3AE00 (:47034): the volcano's (10,19) smoke/fire
            // plume — a 240-tick visual at the crater (sprite 228,
            // the flame family), no damage (+18 bit1 set).
            19 => {
                let e = &mut self.ent[s];
                e.tick70 = 19;
                e.max_life = 240;
                e.f44 = 200;
                e.flags = (e.flags & !8) | 0x20000 | 1;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 228);
                self.extents(s, 512, 512);
            }
            // The Wall of Fire NAPALM cloud (state 58 — NOT 53;
            // class-10 state 53 is the building collapse walker). The
            // model-53 creator was SWAPPED between builds (both spell-20
            // paths detonate the m16 bolt into this (10,53) via the
            // +68=10/+69=53 descriptor — trace SURVEY-MC1HW §3/§7):
            // - base MC1 `sub_3B8E0` (:47639): a persistent low-damage
            //   wall — life 128, f44 100, random yaw, extents 1024/0x4000.
            // - Hidden Worlds `sub_3BC60` (remc1hw :43766): a brief,
            //   devastating expanding-ring detonation — life 6, f44 3000,
            //   NO extents (the state-58 HW handler re-derives them each
            //   tick) and NO yaw LCG draw (stream-faithful).
            53 => {
                let hw = self.is_hidden_worlds();
                let e = &mut self.ent[s];
                e.tick70 = 58;
                e.f26 = 0;
                e.flags &= !8;
                if hw {
                    e.max_life = 6;
                    e.f44 = 3000;
                } else {
                    e.max_life = 128;
                    e.f44 = 100;
                    let d = lcg32(&mut e.rand);
                    e.f30 = (d & 0x7FF) as u16;
                    e.f80 = 1024;
                    e.f82 = 1024;
                    e.f84 = 0x4000;
                }
                self.link(s, x, y, z);
                self.refill_life(s);
            }
            // sub_3BA00 (:47705): the GLOBAL DEATH field (state 60).
            // +26 = 32 = the priming tick-tock; +44 = 100 (the
            // detonation copy overrides with the spell's 7000). The
            // ctor's life 19 / speed 256 / random heading / extents
            // (1024, 0x4000) are DEAD WEIGHT for the state-60
            // handler (verbatim anyway); the flat plane lives in
            // the sweep's 2D distance. No sprite — the spell is
            // authentically invisible.
            55 => {
                let e = &mut self.ent[s];
                e.tick70 = 60;
                e.max_life = 19;
                e.f44 = 100;
                e.f26 = 32;
                e.f126 = 256;
                let d = lcg32(&mut e.rand);
                e.f30 = (d & 0x7FF) as u16;
                e.flags &= !8;
                e.f80 = 1024;
                e.f82 = 1024;
                e.f84 = 0x4000;
                self.link(s, x, y, z);
                self.refill_life(s);
            }
            // sub_3B460 (:47396): the lightning STORM cloud — note
            // state 40 (not 38), life 32, sprite 272. The caller
            // copies heading/target/damage/bolt-spec from the (9,12)
            // storm projectile (:63775-81).
            38 => {
                let e = &mut self.ent[s];
                e.tick70 = 40;
                e.max_life = 32;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 272);
                self.extents(s, 512, 512);
            }
            // sub_3AE80 (:47062): the bolt hit-flash (one-shot ch0).
            23 => {
                let e = &mut self.ent[s];
                e.tick70 = 23;
                e.max_life = 8;
                e.f44 = 25;
                e.flags |= 0x20000 | 1;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 7);
                self.extents(s, 200, 200);
            }
            // sub_3AF00 (:47090): m11's mana-steal flash (ch3).
            25 => {
                let e = &mut self.ent[s];
                e.tick70 = 25;
                e.max_life = 8;
                e.f44 = 2000;
                e.flags &= !8;
                self.link(s, x, y, z);
                self.refill_life(s);
                self.set_sprite(s, 283);
                self.extents(s, 512, 512);
            }
            _ => {
                self.free_entity(s);
                return None;
            }
        }
        Some(s)
    }

    /// sub_3B5A0 (:47443): the mana ball (state 41). Callers override
    /// +140/+144; the tick re-derives the size sprite every turn.
    pub(crate) fn spawn_mana_ball(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let b = self.new_event()?;
        {
            let e = &mut self.ent[b];
            e.class64 = 10;
            e.model65 = 39;
            e.tick70 = 41;
            e.f140 = 512;
            e.f46 = 128;
            e.f28 = 3;
            e.f58 = 0x80;
        }
        self.link(b, x, y, z);
        self.refill_life(b);
        self.ball_resize(b);
        Some(b)
    }

    /// dword_900A4 (:2215): the ball size-class thresholds.
    const BALL_SIZES: [i32; 7] = [256, 512, 1024, 2048, 4096, 9192, 18384];

    /// sub_274D0 (:29574): ball sprite = family base + size class by
    /// carried mana (8 classes; > 36768 = the dragon-drop boulder);
    /// nonzero sizes halve the extents (sub_370E0 :43781). Family 52
    /// = unowned; the owner palette families (105 + 8·player-slot)
    /// are the mana-collection track (our claims use the
    /// PLAYER_TARGET sentinel, not a pool wizard).
    pub(crate) fn ball_resize(&mut self, i: usize) {
        let mana = self.ent[i].f140;
        let mut size = 7usize;
        for (k, t) in Self::BALL_SIZES.iter().enumerate() {
            if mana <= *t {
                size = k;
                break;
            }
        }
        // Owner recolor (:29627-32): claimed balls swap to the owner
        // wizard's color row (base 105 + 8*color, wizext var_48);
        // unowned/wild stay on the neutral 52 row. MC1 art is in raw
        // slot order; MC2's sphere families are authored in Transform
        // order (GetManaSphereIndexFromId EF:26800 routes through
        // TransformPlayerColorIndex — crate::mc2::COLOR_ART).
        let base = match self.owner_team(self.ent[i].f144) {
            Some(team) => {
                let art = if matches!(self.verbs.movement, crate::verbs::MovementVerb::Mc2) {
                    crate::mc2::color_art(team)
                } else {
                    team
                };
                105 + 8 * art as usize
            }
            None => 52,
        };
        let ty = (base + size) as u16;
        if self.ent[i].type86 != ty {
            self.set_sprite(i, ty);
            if size != 0 {
                let e = &mut self.ent[i];
                e.f80 /= 2;
                e.f82 /= 2;
                e.f84 /= 2;
            }
        }
    }

    /// Class-10 combat-effect dispatch. Returns terrain_dirty.
    pub(crate) fn effect_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        match self.ent[i].tick70 {
            0 => self.fire_tick(i, ctx),
            1 => self.spreader_tick(i),
            6 => self.standing_fire_tick(i, ctx),
            5 => {
                // :28285-87 — PRE-decrement life test (class-10
                // family): the splash animates 9 ticks, not 8.
                let life = self.ent[i].act_life;
                self.ent[i].act_life = life - 1;
                if life < 0 {
                    // :28294 — retail frees and returns here: no anim
                    // step and no sound on the death tick.
                    self.ent[i].flags |= 0x400;
                    return false;
                }
                self.anim_advance(i);
                // :28288-91 — the one-shot splash sound, latched on the
                // same `& 2` bit the rest of the family uses.
                if self.ent[i].flags & 2 == 0 {
                    self.ent[i].flags |= 2;
                    self.snd(27, i);
                }
                false
            }
            12 => self.possess_flash_tick(i, ctx),
            16 => self.lava_bomb_tick(i),
            17 => self.blast_ring_tick(i, ctx),
            18 => self.eruption_tick(i, ctx),
            19 => self.plume_tick(i),
            23 => self.hit_flash_tick(i, ctx),
            26 => self.duel_tether_tick(i, ctx),
            25 => self.steal_flash_tick(i, ctx),
            40 => self.storm_cloud_tick(i, ctx),
            41 => self.ball_tick(i),
            42 => {
                self.grave_tick(i);
                false
            }
            85 => self.mc2_mine_tick(i, ctx), // Magic Mine (10,78), action 0x55
            58 => self.napalm_tick(i, ctx),
            59 => {
                self.mana_magnet_tick(i);
                false
            }
            60 => self.death_field_tick(i, ctx),
            _ => false,
        }
    }

    /// sub_263C0 (:28949), class-10 state 26 — the DUEL TETHER:
    /// life-- per tick, follows the victim, broadcasts the ch4 grip
    /// (+44 = 200) into it each tick. The victim's intake latches
    /// the CASTER-side pull (:55663-82).
    fn duel_tether_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // :28956-58 — the life test reads the PRE-decrement value: the
        // whole class-10 effect family is pre-decrement in retail (the
        // class-9 flight handlers genuinely are not), so this runs one
        // more tick than the post-decrement form allows.
        // :28955 — retail bumps +26 every tick, BEFORE the life test, so
        // it counts even on the tick the flash dies.
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        // :28959 — the anim step. (The victim-tracking transport below
        // is OURS: retail's sub_263C0 simply broadcasts ch4 over the
        // tether's own extents and never moves it. See ROADMAP.)
        self.anim_advance(i);
        let victim = self.ent[i].f146;
        let amt = self.ent[i].f44 as u32;
        if victim == crate::mc1::mobs::PLAYER_TARGET {
            // The human victim (AI-cast duel — unreachable today:
            // no AI selector emits spell 11).
            let (x, y, z) = (ctx.px, ctx.py, ctx.pz);
            self.move_relink(i, x, y, z);
        } else if victim != 0 {
            let v = &self.ent[victim as usize];
            if v.flags & 0x400 != 0 || v.act_life < 0 {
                self.ent[i].flags |= 0x400;
                return false;
            }
            let (x, y, z) = (v.x, v.y, v.z);
            self.mail_write(MailTarget::Pool(victim as usize), 4, amt, i as u16);
            self.move_relink(i, x, y, z);
        }
        false
    }

    /// sub_299D0 (:31263), class-10 STATE 60 — the real GLOBAL DEATH
    /// field. LAW: the class-10 table is keyed by STATE, not MODEL
    /// (model-keying lands on state 55's terrain-raising volcano
    /// riser; cross-check against the napalm cloud's state 58 →
    /// sub_29780). Verbatim: while +26 (32 from the ctor) runs, tick
    /// it down with sound 43 (the audible priming tick-tock); then
    /// ONE full-pool sweep — every enemy entity within 0xA00 (10
    /// tiles) by PURE 2D DISTANCE (sub_423D0 is x/y only: an infinite
    /// vertical kill cylinder): class 2/5 die instantly (life = -1,
    /// no kill credit, no explosion effect), class 3 take the +44
    /// (7000) on ch0, own-team skipped, and an in-range class-9/10
    /// re-arms the field's OWN life to 0 (verbatim quirk,
    /// inconsequential — it frees this tick regardless). Finish:
    /// sound 44 at the field AND at the owner, the sub_44BE0(owner, 3)
    /// full-screen PALETTE FLASH — the violet wash, armed only when the
    /// field's owner is the local player ([`crate::engine::features::PalFlash`])
    /// — then free. NO terrain change, NO drift, NO entity visual: the
    /// screen flash IS the spell's only sighting, and the ctor's
    /// speed/heading/extents are dead weight.
    fn death_field_tick(&mut self, i: usize, _ctx: &MobCtx) -> bool {
        if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1;
            self.snd(43, i);
            return false;
        }
        let pre = self.ent[i].act_life;
        self.ent[i].act_life = pre - 1;
        if pre >= 0 {
            let (fx, fy, own, amt) = {
                let e = &self.ent[i];
                (e.x, e.y, e.id24, e.f44 as u32)
            };
            for j in 1..self.ent.len() {
                if j == i {
                    continue;
                }
                let (class, team) = (self.ent[j].class64, self.ent[j].id24);
                if class == 0 || team == own {
                    continue;
                }
                let d2 = Self::dist2_sq(fx, fy, self.ent[j].x, self.ent[j].y);
                if Self::isqrt(d2 as u32) >= 0xA00 {
                    continue;
                }
                match class {
                    2 | 5 => self.ent[j].act_life = -1,
                    3 => self.mail_write(MailTarget::Pool(j), 0, amt, own),
                    9 | 10 => self.ent[i].act_life = 0,
                    _ => {}
                }
            }
            self.snd(44, i);
            if own == crate::mc1::mobs::PLAYER_TARGET {
                self.snd_player(44);
                // sub_44BE0(owner, 3): row 3 = red +48 / blue
                // saturated over the untouched green — the violet
                // flash. Gated on the owner being the local player,
                // exactly as sub_44BE0's slot compare is.
                self.pal_flash.arm(3);
            }
        }
        self.ent[i].flags |= 0x400;
        false
    }

    /// sub_29780 (:31140), class-10 state 58 (the m53 Wall of Fire
    /// cloud). The original branches on `IsHiddenWord`:
    /// - base MC1 (`!IsHiddenWord`, below): 15 waves of standing flames
    ///   over the impact ring (112-unit pitch over SEARCH rings 0..1,
    ///   ±64 jitter, the -96 2x2-center recenter): wave 0 = a persistent
    ///   14-tick ground fire patch, waves 1..14 = 1-tick flame sheets
    ///   climbing 128 units per wave — the rising fire curtain. The
    ///   cloud's own ch0 write is +44/maxLife; the flames' inherited
    ///   100/tick is the damage.
    /// - Hidden Worlds ([`Self::napalm_tick_hw`]): a different geometry —
    ///   one EXPANDING (10,0) ring per tick (160-unit pitch), stepped
    ///   `(var26+2)%7`, until `actLife` runs out; sound 30 once. The
    ///   `IsHiddenWord=true` else-branch (remc1hw :29740; the HW path,
    ///   NOT a multiplayer branch — SURVEY-MC1HW §2).
    fn napalm_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.is_hidden_worlds() {
            return self.napalm_tick_hw(i, ctx);
        }
        {
            let e = &mut self.ent[i];
            e.f80 = 512;
            e.f82 = 512;
            e.f84 = 2048;
        }
        let amt = self.ent[i].f44 as u32 / self.ent[i].max_life.max(1);
        self.area_write(i, 0, amt, ctx, false, false);
        let wave = self.ent[i].f26;
        let (x, y, z, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        let cells = self.ring_cells_pub(0, 1);
        for (dx, dy) in cells {
            let d1 = self.ent_rand(i);
            let d2 = self.ent_rand(i);
            let fx = x.wrapping_add((112 * dx as i32 + (d1 % 0x81) as i32 - 64 - 96) as u16);
            let fy = y.wrapping_add((112 * dy as i32 + (d2 % 0x81) as i32 - 64 - 96) as u16);
            if let Some(f) = self.spawn_effect(6, fx, fy, z) {
                let e = &mut self.ent[f];
                e.id24 = own;
                e.f44 = 100;
                e.act_life = if wave == 0 { 14 } else { 1 };
                e.type86 += 7;
                e.f26 += 7;
                e.f46 = wave * 128;
            }
        }
        self.ent[i].f26 = wave + 1;
        if wave >= 14 {
            self.ent[i].flags |= 0x400;
        }
        false
    }

    /// The Hidden Worlds Wall-of-Fire cloud (sub_29780 `IsHiddenWord`
    /// else-branch, remc1hw :29740). Where base MC1 stacks rising waves,
    /// HW paints ONE expanding ground ring per tick: the (10,0) fire on
    /// a 160-unit grid at radius `var26` (`+26`), the radius stepped
    /// `(var26+2)%7` so it sweeps 0,2,4,6,1,3,5, running until the
    /// cloud's `actLife` expires (no wave cap — the spawner's life is the
    /// terminator). Sound 30 plays once (the `+16` bit-1 latch plus a
    /// persistent 0x10000 marker set together on the first surviving
    /// tick). The cloud's own extent tracks the ring (192·var26 wide =
    /// `(768·var26)>>2`, 512 tall); each child is a full 512³ (10,0)
    /// flame inheriting the cloud's owner and yaw, keeping the (10,0)
    /// ctor's own life/damage.
    ///
    /// NOTE (SURVEY-MC1HW §7 — emit chain UNTRACED): the observable
    /// damage/duration follow the cloud's spawn params (`f44`/`max_life`)
    /// and WHICH creator HW's Fire Storm routes through (`sub_3B8E0`
    /// life-128/f44-100 vs `sub_3BC60` life-6/f44-3000), and whether HW
    /// spell-20 spawns a napalm cloud at all beside the homing meteor.
    /// This handler is faithful for any params; only the trigger is open.
    fn napalm_tick_hw(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 0x10002;
            self.snd(30, i);
        }
        let var26 = self.ent[i].f26;
        self.extents(i, 192u16.wrapping_mul(var26 as u16), 512);
        let amt = self.ent[i].f44 as u32 / self.ent[i].max_life.max(1);
        self.area_write(i, 0, amt, ctx, false, false);
        let (x, y, z, own, yaw) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30)
        };
        for (dx, dy) in self.ring_cells_pub(var26 as i32, var26 as i32) {
            let d1 = self.ent_rand(i);
            let d2 = self.ent_rand(i);
            let fx = x.wrapping_add((160 * dx as i32 + (d1 % 0x81) as i32 - 64 - 96) as u16);
            let fy = y.wrapping_add((160 * dy as i32 + (d2 % 0x81) as i32 - 64 - 96) as u16);
            if let Some(f) = self.spawn_effect(0, fx, fy, z) {
                let e = &mut self.ent[f];
                e.id24 = own;
                e.f30 = yaw; // child copies the cloud's yaw (var30)
                e.flags |= 0x10080;
                e.f80 = 512;
                e.f82 = 512;
                e.f84 = 512;
                e.f26 = 0;
            }
        }
        self.ent[i].f26 = (var26 + 2) % 7;
        false
    }

    /// sub_252D0 (:28199), class-10 state 6: the STANDING fire (tree
    /// burn / ground wave). The flame sprite family walks +86 up 7
    /// steps then back down over the last 12 ticks; the fire rides
    /// ground + f46 (3/4 up a burning tree's trunk), dies on water,
    /// and — while +18 bit0 (0x10000) is clear — broadcasts +44 ch0
    /// through the /10 tree-discount writer EVERY tick, so burning
    /// trees torch their neighbors (~5/tick) and forests chain-burn.
    /// Deviation: the original also spits a (10,13) smoke puff on
    /// 1/7 of shrink ticks — the LCG draw is kept for stream parity,
    /// the puff itself is skipped (decorative).
    fn standing_fire_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let pre = self.ent[i].act_life;
        self.ent[i].act_life = pre - 1;
        let mut done = pre < 0;
        if !done {
            // sub_44C10 player-distance bookkeeping omitted (HUD/AI).
            if self.ent[i].act_life < 12 {
                if self.ent[i].f26 > 0 {
                    self.ent[i].f26 -= 1;
                    self.ent[i].type86 -= 1;
                    if self.ent[i].flags & 0x80 == 0 {
                        let d = self.ent_rand(i);
                        if d % 7 == 0 {
                            // (10,13) smoke puff — skipped.
                        }
                    }
                }
            } else if self.ent[i].f26 <= 6 {
                self.ent[i].f26 += 1;
                self.ent[i].type86 += 1;
            }
            let (x, y, f46) = {
                let e = &self.ent[i];
                (e.x, e.y, e.f46)
            };
            self.ent[i].z = (self.ground_z(x, y) as i16).wrapping_add(f46);
            if self.on_water_pub(x, y) {
                done = true;
            }
        }
        if done {
            self.ent[i].flags |= 0x400;
        }
        // The damage write runs even on the death tick (:28255-56
        // falls through LABEL_11).
        if self.ent[i].flags & 0x10000 == 0 {
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 0, amt, ctx, true, false);
        }
        false
    }

    /// sub_49890/499C0/49A50 (:57662-57790), class-2 model 0 — the
    /// TREE. State 0: ch0 intake; death sparks a (10,6) standing fire
    /// owned by the attacker, riding 3/4 up the trunk, with ONE
    /// tree-LCG draw setting rand%60+130 as BOTH the fire's life and
    /// the tree's burn timer; the tree goes un-hittable, state 1.
    /// State 1: burn down; below 60 → state 2 + the charred sprite
    /// (83→226, 84→227). All states follow the ground and splash-die
    /// on water. (Pool-full fire spawn skips the draw and retries
    /// next tick, as the original.)
    pub(crate) fn tree_tick(&mut self, i: usize) {
        match self.ent[i].tick70 {
            0 => {
                self.ent[i].flags |= 0x20000; // +18 |= 2 (:57674)
                if self.ent[i].mail[0].1 != 0 {
                    let (amt, src) = self.ent[i].mail[0];
                    self.ent[i].mail[0].1 = 0;
                    self.ent[i].act_life -= amt as i32;
                    if self.ent[i].act_life < 0 {
                        let (x, y, z, f84) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z, e.f84)
                        };
                        if let Some(f) = self.spawn_effect(6, x, y, z) {
                            // The mailbox source is already the
                            // broadcaster's +24 owner; the original's
                            // second +24 hop is the identity for it.
                            self.ent[f].id24 = src;
                            self.ent[f].f46 = (3 * f84 as i32 / 4) as i16;
                            let d = self.ent_rand(i);
                            let burn = (d % 60 + 130) as i32;
                            self.ent[f].act_life = burn;
                            self.ent[i].act_life = burn;
                            self.ent[i].flags &= !8; // no longer hittable
                            self.ent[i].tick70 = 1;
                        }
                    }
                }
                self.tree_ground_water(i);
            }
            1 => {
                self.ent[i].act_life -= 1;
                if self.ent[i].act_life < 60 {
                    self.ent[i].tick70 = 2;
                    match self.ent[i].type86 {
                        83 => self.set_sprite(i, 226),
                        84 => self.set_sprite(i, 227),
                        _ => {}
                    }
                }
                self.tree_ground_water(i);
            }
            _ => self.tree_ground_water(i),
        }
    }

    /// The tree handlers' shared tail (:57703-11): z follows the live
    /// ground; water under the trunk → splash (owner passed on) and
    /// despawn.
    fn tree_ground_water(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        if self.on_water_pub(x, y) {
            let owner = self.ent[i].id24;
            let z = self.ent[i].z;
            if let Some(s) = self.spawn_effect(5, x, y, z) {
                self.ent[s].id24 = owner;
            }
            self.ent[i].flags |= 0x400;
        }
    }

    /// sub_49AA0_49DE0 / sub_49B50_49E90 (:57770/:57805), class-2
    /// states 3/9 — the standing stone and the bad stone: the static
    /// draw bit (+18 |= 2), then the per-tick terrain snap that rides
    /// deforming ground. No water arm — statics stand in the sea
    /// (only trees splash-die).
    pub(crate) fn static_snap_tick(&mut self, i: usize) {
        self.ent[i].flags |= 0x20000;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
    }

    /// sub_24F60 (:28047): the fire. One ch0 broadcast + terrain
    /// reaction on the first active tick, then flicker/anim out.
    fn fire_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].f26 & 3 != 0 {
            self.ent[i].f26 -= 1;
            return false;
        }
        // :28068-70 — PRE-decrement life test (class-10 family): every
        // fire burns one tick longer than the post form allowed.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let mut dirty = false;
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            if self.ent[i].flags & 0x10000 == 0 {
                let amt = self.ent[i].f44 as u32;
                self.area_write(i, 0, amt, ctx, false, false);
            }
            // Terrain reaction (:28083-104): burn conversions, else a
            // small scorch crater on flat, low, dry ground.
            let (x, y, z) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            let t = tile((x >> 8) as u8, (y >> 8) as u8);
            let ty = self.t.tile_type[t];
            let conv = match ty {
                26 => Some(0x14),
                10 => Some(0x15),
                11 => Some(0x16),
                _ => None,
            };
            if let Some(c) = conv {
                // The real sub_33800 paint call (:28086-92) — the
                // damage-stage TYPES come from PAINT_BC (10/11/12), NOT
                // the paint code (writing the code as the type =
                // wrong texture). a1/a2 are leftover registers in the
                // original; they only seed corner_orient ties.
                self.paint(0, 0, t, c);
                dirty = true;
            } else if !(6..=0x22).contains(&ty)
                && self.t.angle[t] & 7 != 1
                && (z as i32 - self.ground_z(x, y)) <= 128
                && !self.on_water_pub(x, y)
            {
                let d = self.ent_rand(i);
                self.dig_scorch(i, -((d % 7) as i16));
                dirty = true;
            }
            let d2 = self.ent_rand(i);
            self.ent[i].f46 = ((d2 % 0x41) as i32 - 32) as i16;
            self.snd(3, i); // :28118
        }
        // z rule sub_42000_42340 (:52576-601, called :28116 with
        // (ground, 0, 0, flicker)): ABOVE ground the fire drifts by
        // the fixed flicker delta each tick; below ground it clamps
        // UP to ground; at ground it stays. The original never pulls
        // a fire down to terrain — a midair explosion (max-range
        // fireball expiry, the meteor's trail) stays at altitude.
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let g = self.ground_z(x, y) as i16;
        if self.ent[i].z > g {
            self.ent[i].z = self.ent[i].z.wrapping_add(self.ent[i].f46);
        }
        if self.ent[i].z < g {
            self.ent[i].z = g;
        }
        self.anim_advance(i);
        dirty
    }

    /// sub_25130 (:28127): the fire-spreader — one ring of fires at
    /// radius +26 (0 = the single corpse flame), then gone.
    fn spreader_tick(&mut self, i: usize) -> bool {
        // :28142-48 — the life test reads the PRE-decrement value, so a
        // life-1 puff ticks TWICE before it is freed.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        // :28149-53 — the `& 2` latch guards ONLY the one-shot sound.
        // The ring spawn below runs on EVERY tick, exactly as the
        // sibling blast_ring_tick does; hoisting the whole body under
        // this latch halved the corpse flame (one pass instead of two)
        // and with it every "castle as weapon" crush.
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.snd(3, i); // :28152
        }
        let (x, y, z, owner, radius, inherit) = {
            let e = &self.ent[i];
            (
                e.x,
                e.y,
                e.z,
                e.id24,
                e.f26.max(0) as i32,
                e.flags & 0x10000,
            )
        };
        let cells = self.ring_cells_pub(radius, radius);
        for (dx, dy) in cells {
            let skip = self.ent_rand(i) & 1 != 0; // 50% skip draw
            let j1 = (self.ent_rand(i) % 0x81) as i32 - 64;
            let j2 = (self.ent_rand(i) % 0x81) as i32 - 64;
            if skip {
                continue;
            }
            // x - 96 + 192·dx + jitter (:28167-70), 2x2-center recenter.
            let fx = x.wrapping_add((192 * dx as i32 + j1 - 96) as u16);
            let fy = y.wrapping_add((192 * dy as i32 + j2 - 96) as u16);
            if let Some(f) = self.spawn_effect(0, fx, fy, z) {
                self.ent[f].id24 = owner;
                self.ent[f].flags |= 0x80 | inherit;
            }
        }
        false
    }

    /// sub_25CE0 (:28671): the growing fire-ring blast — per-tick ch0
    /// at +44/maxLife, a ring of fires per tick, radius (+2) % 11.
    fn blast_ring_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // :28685-88 — the life test reads the PRE-decrement value, so the
        // ring runs one more pass than the post-decrement form allows.
        // Measured 9 -> 10 passes, 376 -> 417 fires; the per-tick ch0
        // write is f44/max_life, so the ring was landing 90% of its
        // authored damage.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2 | 0x10000;
            self.snd(30, i);
        }
        let radius = self.ent[i].f26.max(0) as i32;
        {
            // Half-extents 192·ring, z 512 (:28696-97) — no floor; the
            // AABB damage test sums both parties' extents, so ring 0
            // still hits a victim on the impact point.
            let e = &mut self.ent[i];
            e.f80 = (768 * radius / 4) as u16;
            e.f82 = e.f80;
            e.f84 = 512;
        }
        let per_tick = (self.ent[i].f44 as u32) / self.ent[i].max_life.max(1);
        self.area_write(i, 0, per_tick, ctx, false, false);
        let (x, y, z, owner) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        let _ = self.ent_rand(i); // pre-loop draw (:28699)
        let cells = self.ring_cells_pub(radius, radius);
        for (dx, dy) in cells {
            // x - 96 + 160·dx + rand%0x81 - 64 (:28707-09): the -96
            // recenters the ring table's 2x2 zero block.
            let j1 = (self.ent_rand(i) % 0x81) as i32 - 64;
            let j2 = (self.ent_rand(i) % 0x81) as i32 - 64;
            let fx = x.wrapping_add((160 * dx as i32 + j1 - 96) as u16);
            let fy = y.wrapping_add((160 * dy as i32 + j2 - 96) as u16);
            if let Some(f) = self.spawn_effect(0, fx, fy, z) {
                self.ent[f].id24 = owner;
                self.ent[f].flags |= 0x80 | 0x10000;
                self.extents(f, 512, 512);
                self.ent[f].f26 = 0;
            }
        }
        self.ent[i].f26 = ((radius + 2) % 11) as i16;
        false
    }

    /// sub_262D0 (:28898): the bolt hit-flash — one ch0 write and the
    /// thunder-crack 24 (:28911), brief.
    fn hit_flash_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // :28906-08 — the life test reads the PRE-decrement value: the
        // whole class-10 effect family is pre-decrement in retail (the
        // class-9 flight handlers genuinely are not), so this runs one
        // more tick than the post-decrement form allows.
        // :28905 — retail bumps +26 every tick, BEFORE the life test, so
        // it counts even on the tick the flash dies.
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 0, amt, ctx, false, false);
            self.snd(24, i);
            self.ent[i].act_life = 1;
        }
        self.anim_advance(i);
        false
    }

    /// sub_26360 (:28924): m11's mana-steal flash — one ch3 write.
    fn steal_flash_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // :28933-35 — the life test reads the PRE-decrement value: the
        // whole class-10 effect family is pre-decrement in retail (the
        // class-9 flight handlers genuinely are not), so this runs one
        // more tick than the post-decrement form allows.
        // :28932 — retail bumps +26 every tick, BEFORE the life test, so
        // it counts even on the tick the flash dies.
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            let amt = self.ent[i].f44 as u32;
            self.area_write(i, 3, amt, ctx, false, false);
        }
        self.anim_advance(i);
        false
    }

    /// sub_27030 (:29416): the mana ball — claim intake, launch-arc
    /// physics (gravity 16, quarter-bounce, 250/256 friction, ±64
    /// clamp), merge on overlap (sub_277D0 :29700).
    fn ball_tick(&mut self, i: usize) -> bool {
        // A ball absorbed by an earlier slot THIS tick is already
        // despawning — without this guard two coincident balls merge
        // into each other mutually and the mana vanishes (retail's
        // merged ball is display-disabled and can't re-merge).
        if self.ent[i].flags & 0x400 != 0 {
            return false;
        }
        // MC2 Fool's Mana trap (docs/spell-audit/fools-mana.md): a
        // neutral sphere carrying a trap OWNER in f52 is one of the six
        // fake-mana decoys `sub_6C870` throws. A NON-owner possession
        // claim springs the tier retaliation (`sub_36680`) that homes
        // the possessor instead of transferring ownership; an owner
        // reclaim is a no-op. f50 = tier, f136 = payload, f146 = latched
        // claimer (0 = not yet sprung), f56 = counter. MC2-only; MC1 and
        // ordinary balls carry f52 == 0, so the goldens stay untouched.
        let is_fool =
            matches!(self.verbs.movement, crate::verbs::MovementVerb::Mc2) && self.ent[i].f52 != 0;
        if is_fool {
            if self.ent[i].f146 != 0 {
                // sprung: run the tier machine each tick until spent.
                if self.mc2_fools_retaliate(i) {
                    self.ent[i].flags |= 0x400;
                }
                return false; // a sprung trap does no ball physics
            }
            if self.ent[i].mail[1].1 != 0 {
                let claim = self.ent[i].mail[1].1;
                self.ent[i].mail[1] = (0, 0);
                if claim != self.ent[i].f52 {
                    self.ent[i].f146 = claim; // latch the possessor → sprung
                    self.ent[i].f56 = 0;
                    if self.mc2_fools_retaliate(i) {
                        self.ent[i].flags |= 0x400;
                    }
                    return false;
                }
                // owner reclaim: no trap, sphere persists (retail
                // `sub_36680` parentId==claimer skip).
            }
        }
        // ch1 collection claim (:29439-45): the ball takes the
        // claimant as owner — only on an owner CHANGE (the possess
        // flash re-broadcasts for 8 ticks; the guard keeps the claim
        // chime single).
        if !is_fool && self.ent[i].mail[1].1 != 0 {
            let src = self.ent[i].mail[1].1;
            self.ent[i].mail[1] = (0, 0);
            if src != self.ent[i].f144 {
                self.ent[i].f144 = src;
                self.ent[i].flags &= !0x40;
                // The chime anchors at the CLAIMANT, not the ball
                // (:29444 sub_55370(claimant, -1, 4)) — the player-
                // gated id 4 is heard exactly when YOU claim.
                if src == crate::mc1::mobs::PLAYER_TARGET {
                    self.snd_player(4);
                }
            }
        }
        // ch4 attract (:29451-62): the (10,54) magnet tagged this
        // ball (+118 = magnet slot, the ch4 mail source: +114/+118
        // ARE the channel-4 amount/source pair, +90+6·4/+94+6·4) —
        // aim at it and add a magnitude-4 impulse onto the velocity
        // accumulator, then acknowledge. Against the ±64 clamp and
        // 250/256 friction below this shapes the retail stream. The
        // pull NEVER claims (the ch4 amount is read by nothing;
        // player-confirmed): claim = the bolt's localized impact
        // flash + the merge's owned-beats-unowned adoption.
        if self.ent[i].mail[4].1 != 0 {
            let m = self.ent[i].mail[4].1 as usize;
            self.ent[i].mail[4] = (0, 0);
            if m < self.ent.len() {
                let (bx, by) = (self.ent[i].x, self.ent[i].y);
                let (mx, my) = (self.ent[m].x, self.ent[m].y);
                // Mask to 0..2047 — `angle_of` can return 2048 (full-
                // turn wrap) and SIN/COS are len 2048.
                let dir = (Self::angle_between(bx, by, mx, my) & 0x7FF) as usize;
                let ivx = ((4 * crate::mc1::tables::SIN[dir]) >> 16) as i16;
                let ivy = (-((4 * crate::mc1::tables::COS[dir]) >> 16)) as i16;
                let e = &mut self.ent[i];
                e.dest_x = (e.dest_x as i16).wrapping_add(ivx) as u16;
                e.dest_y = (e.dest_y as i16).wrapping_add(ivy) as u16;
            }
        }
        // Balloon tether (flag 0x40): the ball FLIES to the balloon
        // (+146) instead of ground physics (:29464-90). Every
        // tethered tick re-arms the +46 lift at 128 (the release pop)
        // and turns +30 to the balloon; ≥16 out the ball steps
        // horizontally at 16/tick, under 16 it snaps over the balloon
        // and z-servos into the hover band [balloon z, +512]:
        // +32/tick from below, −32/tick from more than 512 ABOVE —
        // without the descend arm an overhead ball deadlocks the
        // pickup (the balloon parks under it forever). Ground-
        // clamped; the band sits inside the absorb window (balloon
        // half-height 400), so the balloon side's ent_overlap
        // finishes the pickup. Past 1024 the ball drops the tether
        // itself; a tethered tick never runs ball physics (retail's
        // else-if), even on the tick the tether clears.
        if self.ent[i].flags & 0x40 != 0 {
            let b = self.ent[i].f146 as usize;
            let live_balloon = b != 0
                && self.ent[b].class64 == 3
                && self.ent[b].model65 == 3
                && self.ent[b].flags & 0x400 == 0;
            if live_balloon {
                self.ent[i].f46 = 128;
                let (bx, by, bz) = {
                    let e = &self.ent[b];
                    (e.x, e.y, e.z)
                };
                let mut pos = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z)
                };
                let yaw = Self::angle_between(pos.0, pos.1, bx, by);
                self.ent[i].f30 = yaw;
                let d = Self::isqrt(Self::dist2_sq(pos.0, pos.1, bx, by) as u32) as i32;
                if d <= 1024 {
                    if d >= 16 {
                        Self::polar_step(&mut pos, yaw, 0, 16);
                    } else {
                        pos.0 = bx;
                        pos.1 = by;
                        if pos.2 as i32 >= bz as i32 {
                            if pos.2 as i32 > bz as i32 + 512 {
                                pos.2 -= 32;
                            }
                        } else {
                            pos.2 += 32;
                        }
                    }
                    let ground = self.ground_z(pos.0, pos.1) as i16;
                    if ground > pos.2 {
                        pos.2 = ground;
                    }
                    self.move_relink(i, pos.0, pos.1, pos.2);
                } else {
                    self.ent[i].flags &= !0x40; // strayed: the ball side lets go
                }
            } else {
                self.ent[i].flags &= !0x40; // dangling tether
            }
            self.ball_resize(i);
            return false;
        }
        let mut vx = self.ent[i].dest_x as i16;
        let mut vy = self.ent[i].dest_y as i16;
        vx = vx.clamp(-64, 64);
        vy = vy.clamp(-64, 64);
        let (x0, y0, z0) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let x = x0.wrapping_add(vx as u16);
        let y = y0.wrapping_add(vy as u16);
        let ground = self.ground_z(x, y) as i16;
        // Vertical: gravity only while airborne or launched — a ball
        // at rest stays at rest (applying gravity at rest makes
        // settled balls oscillate 16 units).
        let mut z = z0;
        let mut grounded = false;
        if z > ground || self.ent[i].f46 > 0 {
            z = z.wrapping_add(self.ent[i].f46);
            self.ent[i].f46 = (self.ent[i].f46 - 16).max(-128);
        }
        if z <= ground {
            z = ground;
            grounded = true;
            let v = self.ent[i].f46;
            self.ent[i].f46 = if v < -32 { -v / 4 } else { 0 };
        }
        if matches!(self.verbs.movement, crate::verbs::MovementVerb::Mc2) {
            // MC2 downhill roll + friction — GROUNDED only (retail
            // `sub_58030` inside `TransformArcherToMana`'s `v22 == z`
            // branch): a resting ball takes the terrain gradient onto
            // its velocity, so balls stream down the island's slopes
            // into the low basin where the 14-tile magnet aura finishes
            // the merge (on level-001 the aura alone cannot pull them:
            // arm balls spawn 22–44 tiles out, the aura reaches only
            // 14). `sub_58030` is a RAW-heightmap forward difference
            // over the ball's 2×2 tile quad, added un-divided (a height
            // byte ≈ 32 world units), then the 250/256 friction.
            // Airborne balls keep their velocity.
            if grounded {
                let (tx, ty) = ((x >> 8) as u8, (y >> 8) as u8);
                let h = |dx: u8, dy: u8| {
                    self.t.height[tile(tx.wrapping_add(dx), ty.wrapping_add(dy))] as i32
                };
                let sx = h(0, 0) - h(1, 0) + h(0, 1) - h(1, 1);
                let sy = h(0, 0) + h(1, 0) - h(0, 1) - h(1, 1);
                vx = ((vx as i32 + sx) * 250 / 256) as i16;
                vy = ((vy as i32 + sy) * 250 / 256) as i16;
            }
        } else {
            // MC1: unconditional friction, no slope roll — the
            // original ball physics (goldens locked).
            vx = (vx as i32 * 250 / 256) as i16;
            vy = (vy as i32 * 250 / 256) as i16;
        }
        self.ent[i].dest_x = vx as u16;
        self.ent[i].dest_y = vy as u16;
        // The MC2 aura claim clears once the ball has consumed the
        // pull (EF:28383) — the one-tick handshake's release side.
        // No-op for MC1 (the map only fills under MC2 auras).
        self.mc2_aura_claim.0.remove(&(i as u16));
        if (x, y, z) != (x0, y0, z0) {
            self.move_relink(i, x, y, z);
        }
        // Merge with an overlapping ball: absorb, despawn the other.
        // A DECAYING ball (the apocalypse-rain channel below) never
        // INITIATES a merge (EF:26268 gates `sub_36D50` on
        // `!(byte[1] & 0x20)`) — but a live ball may still absorb
        // it, which is retail's own mana-retention loophole (magnet/
        // balloon consolidation into a permanent sphere).
        let decaying = self.ent[i].flags & 0x2000 != 0;
        for j in 1..self.ent.len() {
            if decaying {
                break;
            }
            // Fool's-Mana traps never merge — the six decoys stay
            // distinct, and a real ball must not absorb one (the merge
            // copies only mana/owner, dropping the trap fields).
            if j == i
                || is_fool
                || self.ent[j].class64 != 10
                || self.ent[j].model65 != 39
                || self.ent[j].f52 != 0
                || self.ent[j].flags & 0x400 != 0
            {
                continue;
            }
            if self.ent_overlap(i, j) {
                let (fi, fj) = (self.ent[i].f140, self.ent[j].f140);
                // MC2 owner rule (retail `sub_36D50` EF:26919): the
                // surviving ball takes the OWNER (colour) of the larger
                // contributor — an unowned ball defers to an owned
                // partner, two owned balls resolve to the bigger (NOT
                // the survivor's own owner, which colours a merged ball
                // as "the last ball merged"). (Retail breaks the
                // owned-vs-owned tie on the owner wizards' maxMana; ball
                // mana is the observable proxy and is what the
                // single-owner economy levels turn on.)
                if matches!(self.verbs.movement, crate::verbs::MovementVerb::Mc2) {
                    let (oi, oj) = (self.ent[i].f144, self.ent[j].f144);
                    let winner = if oi == 0 {
                        oj
                    } else if oj == 0 {
                        oi
                    } else if fj > fi {
                        oj
                    } else {
                        oi
                    };
                    self.ent[i].f144 = winner;
                } else {
                    // MC1 owner rule (`sub_277D0` :29700): OWNED BEATS
                    // UNOWNED — an unowned survivor ADOPTS the absorbed
                    // ball's owner (:29717; this is how magnet-pulled
                    // balls become claimed as they coalesce into the
                    // claimed one). A class-10 owner (a grave's bank
                    // tag) loses to a real owner (:29734-50); two
                    // DIFFERENT real owners contest on the owner
                    // wizards' +136 (:29755-73: strictly larger keeps
                    // the survivor's owner, else the absorbed side
                    // wins). Port note: MC1 wizard ents don't carry a
                    // +136 bank (only castles do) and the human has no
                    // pool entity, so both sides resolve 0 and the
                    // contest falls to retail's else-arm (absorbed
                    // side's owner) — structure faithful, operands
                    // approximated. Mana is ALWAYS additive: the
                    // reconstruction's two `*=` branches (:29750,
                    // :29773) are transcription slips (every sibling
                    // branch is `+=`).
                    let (oi, oj) = (self.ent[i].f144, self.ent[j].f144);
                    let is_c10 = |g: &Self, o: u16| {
                        o != crate::mc1::mobs::PLAYER_TARGET
                            && (o as usize) < g.ent.len()
                            && g.ent[o as usize].class64 == 10
                    };
                    let w136 = |g: &Self, o: u16| {
                        if o != crate::mc1::mobs::PLAYER_TARGET && (o as usize) < g.ent.len() {
                            g.ent[o as usize].f136
                        } else {
                            0
                        }
                    };
                    if oi == 0 {
                        self.ent[i].f144 = oj;
                    } else if oj != 0 && oi != oj {
                        let (ci, cj) = (is_c10(self, oi), is_c10(self, oj));
                        // Two distinct retail branches that share an
                        // outcome: the class-10-loses arm and the
                        // lost +136 contest — kept separate to match
                        // the trace.
                        #[allow(clippy::if_same_then_else)]
                        if ci && !cj {
                            self.ent[i].f144 = oj;
                        } else if !ci && !cj && w136(self, oi) <= w136(self, oj) {
                            self.ent[i].f144 = oj;
                        }
                    }
                }
                self.ent[i].f140 = fi + fj;
                self.ent[j].flags |= 0x400;
                break;
            }
        }
        // Size re-derivation every tick (:29569) — merged/claimed
        // balls visibly grow/recolor in the original.
        self.ball_resize(i);
        // The apocalypse-rain DECAY channel (`byte[1] |= 0x20` — port
        // flag bit 13; the MC2 sphere mover's tail, EF:26289-307):
        // the timed sphere counts its life down — at 12 the 67%
        // death-fade bit (24) arms, at 6 it swaps to the bit-23
        // ghost, at 0 it expires. Only the doomsday mana rain sets
        // the bit (mc2::morph summit91), so MC1 and ordinary spheres
        // never enter; a balloon tether returns before this tail,
        // reproducing retail's pickup-retains-the-ball behavior.
        if decaying {
            self.ent[i].act_life -= 1;
            let l = self.ent[i].act_life;
            if l < 6 {
                if l == 0 {
                    self.ent[i].flags |= 0x400;
                }
            } else if l == 6 {
                self.ent[i].flags = (self.ent[i].flags | 1 << 23) & !(1 << 24);
            } else if l == 12 {
                self.ent[i].flags |= 1 << 24;
            }
        }
        false
    }

    // ---- corpse pipeline ----------------------------------------------------

    /// The CorpseVerb seam (crate::verbs): MC1 scatters mana
    /// balls/jars. MC2's death drops (spell tokens, mana-sphere
    /// split/merge) live in the mc2 death handlers, which do not
    /// route through here — an MC2 world reaching THIS drop serves
    /// the MC1 scatter and says so in telemetry.
    pub(crate) fn corpse_drop(&mut self, i: usize) {
        match self.verbs.corpse {
            CorpseVerb::Mc1 => self.corpse_drop_mc1(i),
            CorpseVerb::Mc2 => {
                self.note_verb_fallback(VerbKind::Corpse);
                self.corpse_drop_mc1(i);
            }
        }
    }

    /// sub_27690 (:29663): the corpse's mana-ball drop — one unused
    /// draw on the CORPSE's seed (kept for stream parity), then the
    /// ball with two launch draws on its OWN seed.
    fn corpse_drop_mc1(&mut self, i: usize) {
        if self.ent[i].f140 <= 0 {
            return;
        }
        let _ = self.ent_rand(i); // :29674 — result unused, draw kept
        let (x, y, z, heading, mana, owner) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f140, e.f144)
        };
        if let Some(b) = self.spawn_mana_ball(x, y, z) {
            self.ent[b].f140 = mana;
            self.ent[b].f144 = owner;
            let d1 = self.ent_rand(b);
            let yaw = ((d1 % 0x71) as i32 - 56 + heading as i32) as u16 & 0x7FF;
            let d2 = self.ent_rand(b);
            let speed = (d2 % 0x30 + 16) as i16;
            self.ent[b].f30 = yaw;
            self.ent[b].f34 = yaw;
            let vx = ((speed as i32 * crate::mc1::tables::SIN[yaw as usize]) >> 16) as i16;
            let vy = (-((speed as i32 * crate::mc1::tables::COS[yaw as usize]) >> 16)) as i16;
            self.ent[b].dest_x = vx as u16;
            self.ent[b].dest_y = vy as u16;
            let ground = self.ground_z(x, y) as i16;
            self.ent[b].f46 = (1024 - (z.wrapping_sub(ground)) as i32).max(0) as i16 >> 3;
        }
        self.ent[i].f144 = 0;
    }

    /// The corpse's death-flame puff: class-10 m1 at radius 0 with
    /// +24 = the corpse (:21866).
    pub(crate) fn corpse_puff(&mut self, i: usize) {
        let (x, y, z, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        if let Some(p) = self.spawn_effect(1, x, y, z) {
            self.ent[p].id24 = id;
            self.ent[p].f26 = 0;
        }
    }

    // ---- helpers over private feature internals ------------------------------

    /// Ring cell offsets for radius lo..=hi — the real SEARCH.DAT
    /// ring table (the original's precomputed rings, row-major
    /// emission order + the dropped-last-cell quirk, features.rs
    /// `ring_cells`), sign-extended for unit-space scaling. The retail
    /// rings are ROUND (not a Chebyshev box = a square blast);
    /// tile-space callers (dig_disc) keep the raw u8 deltas and wrap
    /// mod 256.
    fn ring_cells_pub(&self, lo: i32, hi: i32) -> Vec<(i8, i8)> {
        self.ring_cells(lo, hi)
            .into_iter()
            .map(|(dx, dy)| (dx as i8, dy as i8))
            .collect()
    }

    /// The fire's scorch dig (sub_40D30(expl, 0, 0, -depth, 1)):
    /// a single-cell protected dig at the fire's position. Also the
    /// MC2 fire's (sub_30D50 → sub_572C0 — same chassis shape).
    pub(crate) fn dig_scorch(&mut self, i: usize, delta: i16) {
        if delta == 0 {
            return;
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let _ = self.dig_cell_pub((x >> 8) as i16, (y >> 8) as i16, delta, true);
    }
}

// Global-stream helper kept close to the module using it.
#[allow(dead_code)]
pub(crate) fn global_draw(rand: &mut u32) -> u32 {
    lcg32(rand)
}
