//! MC2 creature machinery — the Phase-3 slice (ROADMAP "Phase 3"):
//! the class-5 dispatch, shared state primitives, the three slice
//! creatures (Goat m1, Archers m4, Villager m13) and the (9,13)
//! archer arrow, ported verbatim from remc2 EventsFunctions.cpp
//! (`:N` cites; the full trace bank: docs/PHASE3-RESEARCH.md). Runs
//! on the SHARED chassis ([`crate::mc1::features::Gen`]) — same
//! pool, mailboxes, LCG, terrain samplers, per the Phase-0 survey.
//! MC2's NewEvent defaults match MC1's field-for-field (life 300,
//! flags dword 8, speed 16, strength 100, id = slot, filter bytes
//! -1; Events.cpp:582-599) — the chassis holds.
//!
//! Entity-field mapping (MC2 name → our [`Ent`] field):
//! `actionIndex_0x45_69`→tick70 · `byte_0x3E_62` phase→f63 (both
//! engines increment AFTER the handler) · `yaw_0x1C_28`→f30 ·
//! `pitch_0x1E_30`→f32 · `roll_0x20_32` target-yaw→f34 ·
//! `word_0x24_36` killer→f38 · `word_0x26_38` hit-source→f40 ·
//! `word_0x32_50` pack-leader→f52 · `word_0x34_52` subentity
//! chain→f54 · `byte_0x39_57` awake→f58 (0xFA dead sentinel) ·
//! `byte_0x3A_58` wake delay→f59 · `word_0x96_150` target→f146 ·
//! `actSpeed_0x82_130`→f126 · `minSpeed_0x84_132`→f128 ·
//! `maxSpeed_0x86_134`→f130 (NB: MC1's f128/f130 mean max/accel —
//! per-column semantics, handlers never cross) ·
//! `subSpellIndex_0x2A_42`→f44 · `mana_0x90_144`→f140 ·
//! `playerEntityIndex_0x94_148` sphere owner→f144 ·
//! `dword_0x10_16` scratch/invis→f26 · `word_0x5A_90` sprite-param
//! index→type86 · `array_0x52_82` {yaw,pitch,roll,fov}→
//! {f78,f80,f82,f84} · `xtype_0x41_65`→f66 · `xsubtype_0x42_66`→f67
//! (their -1 default = MC1's 0xFF filter default — aligned) ·
//! `rand_0x14_20` (u16, global_types.h:331)→rand under the U16
//! chassis · melee inbox `str_0x5E_94` {damage, attacker}→mail[0]
//! (same clear-source-keep-amount quirk as MC1, :8966) ·
//! `struct_byte_0xc` byte[0]&0x20 invisible→flags 0x20 · byte[0]&2
//! arrow-whoosh-played→flags bit 25 · byte[1]&4 disabled→flags
//! 0x400 (our reap) · byte[1]&8 forced-stop→flags bit 26 · byte[2]&4
//! blocked-status→flags bit 27 · byte[2]&0x10 no-corpse→flags
//! bit 28.
//!
//! Per-wizard fields shared with the MC1 column (same gameplay
//! semantics, human = out-of-pool):
//! - `word_0x248_584` (the wizard "wanted" timer, armed to 200 by
//!   offenses against the village; archers only engage wizards with
//!   it live, :11799) → [`Gen::player_aggro`] for the human — the
//!   MC1 militia gate's exact analog.
//! - `word_0x36_54` = 100 on arrow fire (:60598 sub_5EF70) → the
//!   danger-music countdown, [`Gen::player_danger`].
//!
//! DELIBERATE APPROXIMATIONS (cited, revisit as the port widens):
//! - remc2 rebuilds per-tick entity LISTS in slot order (:39930:
//!   wizards → dword_38519, per-model class-5 → bytearray_38403x
//!   skipping 0xB4/0xE8/0xEA, buildings → dword_38527). We scan the
//!   pool in slot order — identical order and tie behavior.
//! - The human wizard lives OUTSIDE the pool: wizard scans visit
//!   the human via [`MobCtx`] first (retail's list is slot-ordered
//!   with the human in slot 1), then pool class-3 wizards.
//! - The arrow's impact effect `sub_10C80(arrow, 0, subSpell)` is
//!   not yet transcribed; the port writes channel-0 area damage of
//!   `f44` through the shared mailbox writer at the impact point —
//!   the same observable (creature inboxes + the player probe).
//!   The `sub_68740` shielded-target ricochet (word[0] & 0x8010)
//!   has no shielded targets in the slice and lands with the MC2
//!   damage arm.
//! - The arrow's hit probe `sub_10780` → our tile-chain victim scan
//!   ([`Gen::victim_scan_at`]'s MC2 twin pending the class-9 pass).
//! - `TransformEntityToManaSphere` spawns spheres through the MC1
//!   (10,39) ball ctor and writes the MC2 launch fields into the
//!   MC1 ball's field homes so the shared ball tick flies them —
//!   until MC2's own (10,39) handler is diffed.
//! - `sub_20130` (archer base+6) is MISSING from the decompile
//!   (gap between //2010f0 and //201140); unreachable for archers
//!   (row flags bit 8 clear) — stubbed as hold-state.
//! - The global creature counter (`dword_0x364D2--` on the boxed-in
//!   suicide, :8860) has no reader in the slice; not tracked.

use super::behavior::{BEHAVIOR, Mc2BehaviorRow};
use super::sprite_params::SPRITE_PARAMS;
use crate::mc1::features::Gen;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// MC2-only flag bits on [`Ent::flags`] (high bits; MC1 owns the low
/// ones — see the module doc mapping).
pub(crate) const F_WHOOSH: u32 = 1 << 25; // byte[0] & 2 (arrow sound played)
pub(crate) const F_STOP: u32 = 1 << 26; // byte[1] & 8 (forced stop)
pub(crate) const F_BLOCKED: u32 = 1 << 27; // byte[2] & 4 (move blocked)
pub(crate) const F_NO_CORPSE: u32 = 1 << 28; // byte[2] & 0x10

const GOAT_BASE: u8 = 8;
const ARCHER_BASE: u8 = 32;
const VILLAGER_BASE: u8 = 104;
/// The arrow's action/state (= its model; :35031).
const ARROW_STATE: u8 = 13;

impl Gen {
    // ---- shared MC2 helpers ------------------------------------------------

    /// One u16 LCG draw (`rand_0x14_20 = 9377*x + 9439`); the
    /// chassis-selected [`Gen::ent_rand`] does exactly this under
    /// RandWidth::U16.
    pub(crate) fn mc2_rand(&mut self, i: usize) -> u32 {
        self.ent_rand(i)
    }

    /// `SetEntityIndexAndRot_49CD0` (:32837): store the sprite-param
    /// row and derive the rot/extent quad from it (/2). No RNG.
    pub(crate) fn mc2_set_sprite(&mut self, i: usize, idx: u16) {
        let (s6, r8) = self.mc2_params_ext(idx as usize);
        let e = &mut self.ent[i];
        e.type86 = idx;
        e.frame88 = 0;
        e.f78 = r8 / 2; // array.yaw
        e.f80 = s6 / 2; // array.pitch
        e.f82 = s6 / 2; // array.roll
        e.f84 = r8 / 2; // array.fov
    }

    /// The (speed_6, rotSpeed_8) pair for a particle-param row —
    /// the DERIVED table when the dims-fed assets carry it
    /// ([`crate::mc2::derive_sprite_extents`]), else the raw static
    /// row (pre-dims callers keep the old behavior).
    pub(crate) fn mc2_params_ext(&self, idx: usize) -> (u16, u16) {
        self.assets
            .mc2_sprite_ext
            .get(idx)
            .copied()
            .unwrap_or_else(|| {
                let p = &SPRITE_PARAMS[idx];
                (p.speed_6, p.rot_speed_8)
            })
    }

    /// `sub_49E10` (:32865): sprite + the quad doubled (the arrow's
    /// call with 195).
    pub(crate) fn mc2_set_sprite_x2(&mut self, i: usize, idx: u16) {
        self.mc2_set_sprite(i, idx);
        let e = &mut self.ent[i];
        e.f80 *= 2;
        e.f82 *= 2;
        e.f84 *= 2;
    }

    /// `SetEntityShiftRot_49EA0` (:32874): pitch = roll = shift,
    /// fov = fov.
    pub(crate) fn mc2_shift_rot(&mut self, i: usize, shift: u16, fov: u16) {
        let e = &mut self.ent[i];
        e.f80 = shift;
        e.f82 = shift;
        e.f84 = fov;
    }

    /// `SetEvent144_49C70` (:32826): mana = maxLife >> 1.
    pub(crate) fn mc2_set_mana_half(&mut self, i: usize) {
        self.ent[i].f140 = (self.ent[i].max_life >> 1) as i32;
    }

    /// `sub_580E0` (:40372): sink by the row's zStep while above
    /// ground, clamp to ground + hover.
    fn mc2_alt_core(z: &mut i16, ground: i16, hover: i16, z_step: i16) {
        if *z > ground {
            *z = z.wrapping_add(z_step);
        }
        if *z <= ground.wrapping_add(hover) {
            *z = ground.wrapping_add(hover);
        }
    }

    /// `sub_1EEE0` (:11172): altitude commit at the current position.
    pub(crate) fn mc2_alt_commit(&mut self, i: usize) {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let (hover, z_step) = (row.v_12, row.v_14);
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        let mut z = self.ent[i].z;
        Self::mc2_alt_core(&mut z, ground, hover, z_step);
        self.move_relink(i, x, y, z);
    }

    /// `sub_102D0` with a3 = 1 (:3632): walk up to max(array.pitch,
    /// array.roll) units along yaw in 256 steps; blocked when a
    /// tile's capability bit falls outside the row's permission
    /// mask, and on caves also when the probe tile is bit3-SEALED or
    /// the ceiling poke test fires (:3674-83).
    pub(crate) fn mc2_path_blocked(&self, i: usize, from: (u16, u16, i16)) -> bool {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let reach = (e.f80).max(e.f82) as i32;
        let mut pos = from;
        let mut walked = 0i32;
        // Retail loop shape `while (walked <= reach) { probe; step }`
        // (:3659-3686): for walker extents <= 255 that is exactly ONE
        // probe at the predicted point. Probing after the bound check
        // tested one extra 256-step point and false-blocked a tile
        // early (the PLAYTEST-3 brownian settlers on the 1-tile
        // causeway — docs/traces/mc2-walker-wander-ai.md D2).
        loop {
            if walked > reach {
                return false;
            }
            if !row.v_20 & self.cap_bit(pos.0, pos.1) != 0 {
                return true;
            }
            if self.is_cave() {
                let t = crate::mc1::features::tile((pos.0 >> 8) as u8, (pos.1 >> 8) as u8);
                if self.t.angle[t] & 8 != 0
                    || self.cave_poke(e.f84 as i32, row.v_12 as i32, pos.0, pos.1)
                {
                    return true;
                }
            }
            walked += 256;
            Self::polar_step(&mut pos, self.ent[i].f30, 0, 256);
        }
    }

    /// One predicted candidate of the MC2 move core: altitude core +
    /// polar step at the CURRENT yaw, then the block test (crossing
    /// into a new tile only).
    /// `always_test`: the retry predictions run the block/roughness
    /// test UNCONDITIONALLY (EF:8826/8840/8852) — only the FIRST
    /// prediction gates it on the tile change (EF:8806). A rotated
    /// retry that stays in-tile must still be terrain-tested (E13).
    fn mc2_move_candidate(&self, i: usize, always_test: bool) -> ((u16, u16, i16), bool) {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let mut pos = (e.x, e.y, e.z);
        let ground = self.ground_z(pos.0, pos.1) as i16;
        Self::mc2_alt_core(&mut pos.2, ground, row.v_12, row.v_14);
        Self::polar_step(&mut pos, e.f30, 0, e.f126);
        let crossed = e.x >> 8 != pos.0 >> 8 || e.y >> 8 != pos.1 >> 8;
        let blocked = (always_test || crossed)
            && (self.mc2_path_blocked(i, pos) || self.roughness(pos.0, pos.1) >= row.v_16 as i32);
        (pos, blocked)
    }

    /// `sub_1B8C0` (:8741): the MC2 creature move core. Result codes
    /// 1 same-tile / 2 moved / 3 moved-after-retry / 4 blocked. The
    /// retry yaws replicate the decompile's byte arithmetic verbatim
    /// — including the third retry's C precedence quirk.
    pub(crate) fn mc2_move_core(&mut self, i: usize) -> u8 {
        if self.ent[i].flags & F_STOP != 0 {
            self.ent[i].flags &= !F_STOP;
            return 4;
        }
        // The commit turn is clamped by row v_2 (goat 45, villager 22
        // per tick): sub_58350's v_4 arg is DEAD in retail, the real
        // clamp is subtype_160_0x2_2 (EF:8868-75 + 40391-405; MC1's
        // creature_move already uses its v_2 twin). Clamping with v_4
        // (=5) made yaw 4-9x too slow to catch the wander heading —
        // ballistic walks = the PLAYTEST-3 herd dispersal
        // (docs/traces/mc2-walker-wander-ai.md D1).
        let turn_cap = BEHAVIOR[self.ent[i].row156 as usize].v_2;
        fn commit(g: &mut Gen, i: usize, pos: (u16, u16, i16), cap: i16) {
            g.move_relink(i, pos.0, pos.1, pos.2);
            let e = &g.ent[i];
            let turned = (e.f30 as i32 + Gen::turn_step(e.f30, e.f34, cap) as i32) as u16;
            g.ent[i].f30 = turned & 0x7FF;
        }

        let (pos, blocked) = self.mc2_move_candidate(i, false);
        let same_tile = self.ent[i].x >> 8 == pos.0 >> 8 && self.ent[i].y >> 8 == pos.1 >> 8;
        if same_tile {
            commit(self, i, pos, turn_cap);
            self.ent[i].flags &= !F_BLOCKED;
            return 1;
        }
        if !blocked {
            commit(self, i, pos, turn_cap);
            self.ent[i].flags &= !F_BLOCKED;
            return 2;
        }
        self.ent[i].flags |= F_BLOCKED;
        let yaw0 = self.ent[i].f30;
        // Retry 1: +341 (:8815).
        self.ent[i].f30 = yaw0.wrapping_add(341) & 0x7FF;
        let (pos, blocked) = self.mc2_move_candidate(i, true);
        if !blocked {
            commit(self, i, pos, turn_cap);
            return 3;
        }
        // Retry 2: LOBYTE = yaw0-85, HIBYTE = ((yaw0-341)>>8)&7 —
        // verbatim byte split (:8890-92).
        let lo = yaw0.wrapping_sub(85) as u8;
        let hi = ((yaw0.wrapping_sub(341) >> 8) & 7) as u8;
        self.ent[i].f30 = u16::from_le_bytes([lo, hi]);
        let (pos, blocked) = self.mc2_move_candidate(i, true);
        if !blocked {
            commit(self, i, pos, turn_cap);
            return 3;
        }
        // Retry 3: (yaw0 + 0x400) & (0x700 + LOBYTE(yaw0)) — the
        // decompile's precedence quirk kept verbatim (:8846).
        self.ent[i].f30 = yaw0.wrapping_add(0x400) & (0x700 + (yaw0 & 0xFF));
        let (pos, blocked) = self.mc2_move_candidate(i, true);
        if !blocked {
            commit(self, i, pos, turn_cap);
            return 3;
        }
        // All four blocked (:8855-62): die-on-water/boxed-in suicide.
        let row_flags = BEHAVIOR[self.ent[i].row156 as usize].flags;
        let on_water = self.cap_bit(self.ent[i].x, self.ent[i].y) == 1;
        if row_flags & Mc2BehaviorRow::DIE_ON_WATER != 0 || on_water {
            self.ent[i].act_life = -1;
        }
        4
    }

    /// The shared inbox/life head opening every MC2 state handler
    /// (:8960-8998 pattern): apply the melee mailbox (clear source,
    /// KEEP amount — the MC1 quirk, :8966), inherit the weakest
    /// linked-subentity life, latch killer on death. Returns
    /// 0 quiet / 1 hit / 2 dead.
    pub(crate) fn mc2_state_head(&mut self, i: usize) -> u8 {
        let mut v = 0u8;
        if self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].act_life -= amt as i32;
            self.ent[i].mail[0].1 = 0;
            self.ent[i].f40 = src;
            v = 1;
        } else {
            self.ent[i].f40 = 0;
        }
        let mut j = self.ent[i].f54 as usize;
        while j != 0 {
            if self.ent[j].act_life < self.ent[i].act_life {
                self.ent[i].act_life = self.ent[j].act_life;
                self.ent[i].f40 = self.ent[j].f40;
                v = 1;
                break;
            }
            j = self.ent[j].f54 as usize;
        }
        if self.ent[i].act_life < 0 {
            self.ent[i].f38 = self.ent[i].f40;
            v = 2;
        }
        v
    }

    /// The two-draw wander-turn idiom (:9136-38 and twins): `v =
    /// rand; rand; f34 += ((rand & 0xFF) + 85) * (2*((v % 0x9D)/79)
    /// - 1); f34 &= 0x7FF`.
    pub(crate) fn mc2_wander_turn(&mut self, i: usize) {
        let v = self.mc2_rand(i);
        let r = self.mc2_rand(i);
        let sign = 2 * ((v % 0x9D) / 79) as i32 - 1;
        let step = ((r & 0xFF) + 85) as i32 * sign;
        self.ent[i].f34 = (self.ent[i].f34 as i32 + step) as u16 & 0x7FF;
    }

    /// Arm the wizard "wanted" timer (`word_0x248_584 = 200`) on a
    /// hit/kill source when it is a wizard — the human maps to the
    /// shared aggro register, pool wizards to the hash-quiet
    /// `mc2_wanted` side channel (E12; the rival column is live).
    pub(crate) fn mc2_arm_wanted(&mut self, src: u16) {
        if src == PLAYER_TARGET {
            self.player_aggro = 200;
        } else {
            let j = src as usize;
            if j > 0 && j < self.ent.len() && self.ent[j].class64 == 3 && self.ent[j].model65 <= 1 {
                self.mc2_wanted.0.insert(src, 200);
            }
        }
    }

    /// Is `slot`'s wanted timer live? (the archer Scan-A post-reject
    /// gate, :11799-802.)
    pub(crate) fn mc2_wanted_live(&self, slot: u16) -> bool {
        if slot == PLAYER_TARGET {
            self.player_aggro > 0
        } else {
            self.mc2_wanted.0.get(&slot).is_some_and(|&t| t > 0)
        }
    }

    /// The full class-3 pool walk shared by the archer's Scan A
    /// (:11768-95) and m24 acquire (sub_28690 :18744-64): nearest
    /// class-3 ANYTHING (wizards, castles, balloons) with `d2 <=
    /// v_28²`, cone `< v_30`, skipping only invisibles (byte[0] &
    /// 0x20). The human wizard sits in retail's dword_38519 like any
    /// pool entity, so the out-of-pool pseudo-target joins the walk.
    pub(crate) fn mc2_class3_scan(&self, i: usize, ctx: &MobCtx) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let (ex, ey, eyaw) = (e.x, e.y, e.f30);
        let mut best: Option<(u16, i32)> = None;
        let mut consider = |tx: u16, ty: u16, slot: u16| {
            let d2 = Self::dist2_sq(ex, ey, tx, ty);
            if d2 > range {
                return;
            }
            let bearing = Self::angle_between(ex, ey, tx, ty);
            if Self::angdist(eyaw, bearing) >= cone {
                return;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((slot, d2));
            }
        };
        if !self.player_invisible {
            consider(ctx.px, ctx.py, PLAYER_TARGET);
        }
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if c.class64 == 3 && c.act_life >= 0 && c.flags & 0x400 == 0 && c.flags & 0x20 == 0 {
                consider(c.x, c.y, j as u16);
            }
        }
        best.map(|(s, _)| s)
    }

    /// The wizard-target scan of `sub_1BF90` (:9152-95): nearest
    /// live wizard within range and FOV cone, skipping invisibles
    /// (byte[0] & 0x20). `wanted_only` = the archer brain's extra
    /// gate (target's word_0x248_584 must be live, :11799).
    pub(crate) fn mc2_wizard_scan(&self, i: usize, ctx: &MobCtx, wanted_only: bool) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let (ex, ey, eyaw) = (e.x, e.y, e.f30);
        let mut best: Option<(u16, i32)> = None;
        let consider = |tx: u16, ty: u16, slot: u16, skip: bool, best: &mut Option<(u16, i32)>| {
            if skip {
                return;
            }
            let d2 = Self::dist2_sq(ex, ey, tx, ty);
            if d2 > range {
                return;
            }
            let ty_yaw = Self::angle_between(ex, ey, tx, ty);
            if Self::angdist(eyaw, ty_yaw) >= cone {
                return;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                *best = Some((slot, d2));
            }
        };
        let human_skip = self.player_invisible || (wanted_only && self.player_aggro <= 0);
        consider(ctx.px, ctx.py, PLAYER_TARGET, human_skip, &mut best);
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if c.class64 == 3 && c.model65 <= 1 && c.act_life >= 0 && c.flags & 0x400 == 0 {
                // Pool wizards carry no wanted timer yet (see
                // mc2_arm_wanted) — under wanted_only they never
                // qualify, faithful to an unarmed timer.
                consider(
                    c.x,
                    c.y,
                    j as u16,
                    c.flags & 0x20 != 0 || wanted_only,
                    &mut best,
                );
            }
        }
        best.map(|(s, _)| s)
    }

    /// The same-model pack scan (:9197-9231): nearest leaderless
    /// same-model creature in range + cone. `reversed_cone` = the +0
    /// patrol quirk (:9038): its cone test uses the REVERSED bearing
    /// `tan2(candidate → self)`, unlike wander's `tan2(self →
    /// candidate)` (:9194) — vestigial for goats/townies (they never
    /// occupy +0) but kept verbatim (walker trace D4).
    pub(crate) fn mc2_pack_scan(&self, i: usize, reversed_cone: bool) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let mut best: Option<(u16, i32)> = None;
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if j == i
                || c.class64 != 5
                || c.model65 != e.model65
                || c.f52 != 0
                || c.act_life < 0
                || matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                || c.flags & 0x400 != 0
            {
                continue;
            }
            let d2 = Self::dist2_sq(e.x, e.y, c.x, c.y);
            if d2 > range {
                continue;
            }
            let ty_yaw = if reversed_cone {
                Self::angle_between(c.x, c.y, e.x, e.y)
            } else {
                Self::angle_between(e.x, e.y, c.x, c.y)
            };
            if Self::angdist(e.f30, ty_yaw) >= cone {
                continue;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j as u16, d2));
            }
        }
        best.map(|(s, _)| s)
    }

    /// The same-model AVOIDANCE override in chase/flee re-aims
    /// (:9643-56): first packmate closer than array.pitch on both
    /// axes steers us away from it.
    pub(crate) fn mc2_avoid_packmate(&mut self, i: usize) {
        let (ex, ey, pitch, model, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.f80 as i32, e.model65, e.id24)
        };
        if pitch == 0 {
            return;
        }
        for c in self.ent.iter().skip(1) {
            if c.class64 == 5
                && c.model65 == model
                && c.id24 != id
                // Retail iterates the LIVE per-model bucket — the
                // dying never appear (EF:9641-50); the full-array
                // walk needs the explicit life gate (E27).
                && c.act_life >= 0
                && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                && c.flags & 0x400 == 0
                && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < pitch
                && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < pitch
            {
                let away = Self::angle_between(c.x, c.y, ex, ey);
                self.ent[i].f34 = away;
                break;
            }
        }
    }

    /// Resolve a target slot to (x, y, z) — `sub_1ED30`'s validation
    /// core for StageVar2 == 0 spawns (:11060: non-14 stage vars
    /// return the candidate; the caller then rejects dead/reaped).
    pub(crate) fn mc2_target(&self, slot: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if slot == PLAYER_TARGET {
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = slot as usize;
        if j == 0 || j >= self.ent.len() {
            return None;
        }
        let t = &self.ent[j];
        if t.class64 == 0 || t.act_life < 0 || t.flags & 0x400 != 0 {
            return None;
        }
        Some((t.x, t.y, t.z))
    }

    /// 3D distance (`sub_583F0`, 16-bit deltas).
    pub(crate) fn mc2_dist3(a: (u16, u16, i16), b: (u16, u16, i16)) -> u32 {
        let dx = (b.0.wrapping_sub(a.0)) as i16 as i32;
        let dy = (b.1.wrapping_sub(a.1)) as i16 as i32;
        let dz = (b.2 as i32) - (a.2 as i32);
        Self::isqrt((dx * dx + dy * dy + dz * dz) as u32)
    }

    /// `sub_1BD90` (:8945) — PATROL: inbox/life head, transitions,
    /// pack detection on the row cadence. No movement; altitude
    /// commit on the quiet and hit paths.
    pub(crate) fn mc2_patrol(&mut self, i: usize, base: u8) {
        match self.mc2_state_head(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                let flee = BEHAVIOR[self.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                self.ent[i].tick70 = base + if flee { 6 } else { 2 };
                self.mc2_alt_commit(i);
            }
            2 => {
                self.ent[i].tick70 = base + 4;
                self.mc2_alt_commit(i);
            }
            _ => {
                let row = &BEHAVIOR[self.ent[i].row156 as usize];
                let pack_ok = row.flags & Mc2BehaviorRow::PACK_DISABLE == 0;
                let period = row.v_26.max(1) as u8;
                if pack_ok && self.ent[i].f63 % period == 0 {
                    if let Some(l) = self.mc2_pack_scan(i, true) {
                        self.ent[i].f52 = l;
                        self.ent[i].tick70 = base + 3;
                    }
                }
                self.mc2_alt_commit(i);
            }
        }
    }

    /// `sub_1BF90` (:9064) — IDLE/WANDER (the spawn state): inbox
    /// head, move, wander turn + wizard scan on the row cadence
    /// (scan gated on the awake byte), pack fallback.
    pub(crate) fn mc2_idle(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                let flee = BEHAVIOR[self.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                self.ent[i].tick70 = base + if flee { 6 } else { 2 };
                self.mc2_alt_commit(i);
            }
            2 => self.ent[i].tick70 = base + 4,
            _ => {
                self.mc2_move_core(i);
                let row = &BEHAVIOR[self.ent[i].row156 as usize];
                let period = row.v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    self.mc2_wander_turn(i);
                    if self.ent[i].f58 != 0 {
                        if let Some(t) = self.mc2_wizard_scan(i, ctx, false) {
                            self.ent[i].f146 = t;
                            let flee = BEHAVIOR[self.ent[i].row156 as usize].flags
                                & Mc2BehaviorRow::FLEE
                                != 0;
                            self.ent[i].tick70 = base + if flee { 6 } else { 2 };
                        } else if BEHAVIOR[self.ent[i].row156 as usize].flags
                            & Mc2BehaviorRow::PACK_DISABLE
                            == 0
                            && let Some(l) = self.mc2_pack_scan(i, false)
                        {
                            self.ent[i].f52 = l;
                            self.ent[i].tick70 = base + 3;
                        }
                    }
                }
            }
        }
    }

    /// `sub_1C560` (:9345) — PACK-FOLLOW: validate the leader,
    /// inbox head (transitions also RETARGET the leader), then on
    /// the cadence copy the leader's state/target and match its
    /// speed (leader max + act, :9482).
    pub(crate) fn mc2_pack(&mut self, i: usize, base: u8) {
        if self.ent[i].f52 == 0 {
            self.ent[i].tick70 = base + 1;
            return;
        }
        let l = self.ent[i].f52 as usize;
        let leader_ok = l != 0
            && l < self.ent.len()
            && self.ent[l].act_life >= 0
            && self.ent[l].flags & 0x400 == 0
            && self.ent[l].class64 == self.ent[i].class64
            && self.ent[l].model65 == self.ent[i].model65;
        let v = self.mc2_state_head(i);
        match v {
            1 | 2 => {
                // The leader inherits our attacker as its target
                // (:9500-9516) before we transition.
                if leader_ok {
                    let flee =
                        BEHAVIOR[self.ent[l].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                    self.ent[l].f146 = self.ent[i].f40;
                    self.ent[l].f52 = 0;
                    self.ent[l].tick70 = base + if flee { 6 } else { 2 };
                }
                if v == 2 {
                    self.ent[i].f52 = 0;
                    self.ent[i].tick70 = base + 4;
                } else {
                    let flee =
                        BEHAVIOR[self.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                    self.ent[i].f146 = self.ent[i].f40;
                    self.ent[i].f52 = 0;
                    self.ent[i].tick70 = base + if flee { 6 } else { 2 };
                    self.mc2_alt_commit(i);
                }
            }
            _ => {
                self.mc2_move_core(i);
                if !leader_ok {
                    self.ent[i].f52 = 0;
                    self.ent[i].tick70 = base + 1;
                    return;
                }
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    let lrole = self.ent[l].tick70.wrapping_sub(base);
                    match lrole {
                        0 | 1 | 3 => {
                            if lrole == 3 {
                                self.ent[i].f52 = self.ent[l].f52;
                            }
                            // Aim at the (possibly re-linked) leader
                            // and sidestep a crowding packmate
                            // (:9455-77, threshold 256).
                            let ll = self.ent[i].f52 as usize;
                            if ll != 0 && ll < self.ent.len() {
                                let e = &self.ent[i];
                                self.ent[i].f34 =
                                    Self::angle_between(e.x, e.y, self.ent[ll].x, self.ent[ll].y);
                                let (ex, ey, model, id) = {
                                    let e = &self.ent[i];
                                    (e.x, e.y, e.model65, e.id24)
                                };
                                for c in self.ent.iter().skip(1) {
                                    if c.class64 == 5
                                        && c.model65 == model
                                        && c.id24 != id
                                        && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                                        && c.flags & 0x400 == 0
                                        && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < 256
                                        && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < 256
                                    {
                                        self.ent[i].f34 = Self::angle_between(c.x, c.y, ex, ey);
                                        break;
                                    }
                                }
                                // Catch-up: leader max + act (:9482) —
                                // retail MC1's line, RE-CONFIRMED by
                                // the survey; both operands from the
                                // LEADER.
                                self.ent[i].f126 = self.ent[l].f130 + self.ent[l].f126;
                            }
                        }
                        2 => {
                            self.ent[i].f146 = self.ent[l].f146;
                            self.ent[i].f52 = 0;
                            self.ent[i].tick70 = base + 2;
                        }
                        6 => {
                            self.ent[i].f146 = self.ent[l].f146;
                            self.ent[i].f52 = 0;
                            self.ent[i].tick70 = base + 6;
                        }
                        _ => {
                            self.ent[i].f52 = 0;
                            self.ent[i].tick70 = base + 1;
                        }
                    }
                }
            }
        }
    }

    /// `sub_1C980` (:9572) — FLEE: inbox head, move, re-aim AWAY
    /// every 4th phase (`HIBYTE += 4` = the 180° flip) with the
    /// packmate avoidance; drop to patrol when the threat dies or
    /// leaves range on the cadence tick.
    pub(crate) fn mc2_flee(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                self.mc2_alt_commit(i);
            }
            2 => self.ent[i].tick70 = base + 4,
            _ => {
                self.mc2_move_core(i);
                let Some((tx, ty, tz)) = self.mc2_target(self.ent[i].f146, ctx) else {
                    self.ent[i].tick70 = base + 1;
                    return;
                };
                if self.ent[i].f63 & 3 == 0 {
                    let e = &self.ent[i];
                    let away = Self::angle_between(e.x, e.y, tx, ty).wrapping_add(0x400) & 0x7FF;
                    self.ent[i].f34 = away;
                    self.mc2_avoid_packmate(i);
                }
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    let e = &self.ent[i];
                    let d3 = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                    if d3 >= BEHAVIOR[self.ent[i].row156 as usize].v_28 as u32 {
                        self.ent[i].tick70 = base + 1;
                    }
                }
            }
        }
    }

    /// `sub_1C310` (:9240) — CHASE-AND-ATTACK: inbox head, move,
    /// re-aim at the target every 4th phase (packmate avoidance),
    /// and on the cadence drop the chase (out of range → base+1) or
    /// fire the thunk. Returns true when the thunk fired.
    pub(crate) fn mc2_chase_attack(
        &mut self,
        i: usize,
        base: u8,
        ctx: &MobCtx,
        attack: fn(&mut Self, usize, u16, &MobCtx) -> bool,
    ) -> bool {
        match self.mc2_state_head(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                self.mc2_alt_commit(i);
                false
            }
            2 => {
                self.ent[i].tick70 = base + 4;
                false
            }
            _ => {
                self.mc2_move_core(i);
                let slot = self.ent[i].f146;
                let Some((tx, ty, tz)) = self.mc2_target(slot, ctx) else {
                    self.ent[i].tick70 = base + 1;
                    return false;
                };
                if self.ent[i].f63 & 3 == 0 {
                    let e = &self.ent[i];
                    self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                    self.mc2_avoid_packmate(i);
                }
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    let e = &self.ent[i];
                    let d3 = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                    if d3 >= BEHAVIOR[self.ent[i].row156 as usize].v_28 as u32 {
                        self.ent[i].tick70 = base + 1;
                        return false;
                    }
                    return attack(self, i, slot, ctx);
                }
                false
            }
        }
    }

    /// `PreKillEntity_1C890` (:9533): chain subentities to state+5,
    /// inherit their killer latch, kill credit (player killer,
    /// victim model NOT in {9, 12, 13, 14, 15}), then state+5.
    pub(crate) fn mc2_prekill(&mut self, i: usize, base: u8) {
        let mut j = self.ent[i].f54 as usize;
        while j != 0 {
            self.ent[j].tick70 = base + 5;
            if self.ent[j].f38 != 0 {
                self.ent[i].f38 = self.ent[j].f38;
            }
            j = self.ent[j].f54 as usize;
        }
        let killer = self.ent[i].f38;
        let model = self.ent[i].model65;
        // PreKillEntity_1C890 (EF:9543-51): credit gates on killer
        // class-3 MODEL-0 (the human avatar only — rivals are (3,1)
        // and never score creature kills) AND the SELF-ID check:
        // killing your own creature earns nothing (E27).
        if killer == PLAYER_TARGET
            && self.ent[i].id24 != PLAYER_TARGET
            && !matches!(model, 9 | 12 | 13 | 14 | 15)
        {
            self.kills += 1;
        }
        self.ent[i].tick70 = base + 5;
    }

    /// `KillEntity_1C930` (:9556): every 8th phase — mana spheres +
    /// the (10,1) corpse burst + reap.
    pub(crate) fn mc2_kill(&mut self, i: usize) {
        if self.ent[i].f63 & 7 != 0 {
            return;
        }
        self.mc2_mana_spheres(i, false);
        if self.ent[i].flags & F_NO_CORPSE == 0 {
            // The (10,1) corpse burst — Phase 4.3 closes the misfit:
            // the explosion creator is native now.
            self.mc2_corpse_burst(i);
        }
        self.ent[i].flags |= 0x400;
    }

    /// `TransformEntityToManaSphere_36BA0` (:26867), verbatim
    /// draws/order: one corpse draw before the loop; per sphere —
    /// draw #1 → yaw = (rand % 0x71 + heading − 56) & 0x7FF, draw
    /// #2 → speed = rand % 0x30 + 16; fall = signed (1024 − zdiff)/8.
    /// Spheres allocate through the shared (10,39) ball ctor and
    /// write the launch into the MC1 ball's field homes so the
    /// shared ball tick flies them (module-doc APPROX).
    pub(crate) fn mc2_mana_spheres(&mut self, i: usize, use_fraction: bool) {
        if self.ent[i].f140 <= 0 {
            return;
        }
        let total = self.ent[i].f140;
        let (fraction, loc) = if use_fraction {
            let f = (total / 1000).clamp(1, 16);
            (f, total / f)
        } else {
            (1, total)
        };
        let (x, y, z, heading, owner) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f144)
        };
        let _ = self.mc2_rand(i); // the pre-loop corpse draw (:26884)
        let ground = self.ground_z(x, y) as i16;
        for n in 0..fraction {
            let Some(b) = self.spawn_mana_ball(x, y, z) else {
                continue;
            };
            self.ent[b].f140 = if n == fraction - 1 {
                total - (fraction - 1) * loc
            } else {
                loc
            };
            self.ent[b].f144 = owner;
            let d1 = self.mc2_rand(b);
            let yaw = ((d1 % 0x71) as i32 + heading as i32 - 56) as u16 & 0x7FF;
            self.ent[b].f30 = yaw;
            self.ent[b].f34 = yaw;
            let d2 = self.mc2_rand(b);
            let speed = (d2 % 0x30 + 16) as i16;
            // Velocity into the MC1 ball's dest fields (the shared
            // ball tick consumes them), fall arc into f46 — signed
            // TRUNCATING /8 like the C idiom at EF:26909 (div_euclid
            // floored: off by one for deaths > 1024 above terrain;
            // castle.rs:530 was already right), NO clamp (MC1 clamps
            // ≥ 0; MC2 does not).
            let mut v = (0u16, 0u16, 0i16);
            Self::polar_step(&mut v, yaw, 0, speed);
            self.ent[b].dest_x = v.0;
            self.ent[b].dest_y = v.1;
            let zdiff = (z as i32) - (ground as i32);
            self.ent[b].f46 = ((1024 - zdiff) / 8) as i16;
        }
        self.ent[i].f140 = 0;
        self.ent[i].f144 = 0;
    }

    // ---- spawn ctors -------------------------------------------------------

    /// `AddCreature_4B490` (:33720) — the Goat (5,1). NO ctor RNG.
    pub(crate) fn mc2_spawn_goat(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 1;
            e.tick70 = GOAT_BASE + 1; // actionIndex 9
            // MC2 carries NO per-channel vulnerability mask — its
            // single damage gate is byte[0] & 8 (mapped to flags & 8,
            // the shared NewEvent default). MC1's writers additionally
            // check the +28 channel mask; admit their physical channel
            // at the seam (cross-column damage contract).
            e.f28 = 1;
            e.f128 = 54; // minSpeed
            e.f130 = 18; // maxSpeed
            e.f126 = 18; // actSpeed = maxSpeed
            e.max_life = 600;
        }
        self.mc2_set_mana_half(i); // 300
        {
            let e = &mut self.ent[i];
            e.f34 = 0;
            e.f30 = 0;
            e.f32 = 0;
            e.f26 = (i % 100) as i16;
            e.row156 = 98; // ABSOLUTE row index (:33739)
        }
        self.ent[i].f58 = BEHAVIOR[98].v_26 + 1;
        // Per-model spawn ordinal → f63 (:33740) — the herd cadence
        // de-sync (E10; every `f63 & N` gate ran in lockstep at 0).
        self.ent[i].f63 = self.mc2_ord(1);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 238);
        Some(i)
    }

    /// `AddArchers_4BA10` (:33878) — the Archers (5,4). ONE ctor RNG
    /// draw → facing.
    pub(crate) fn mc2_spawn_archers(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 4;
            e.tick70 = ARCHER_BASE + 1; // actionIndex 33
            // MC2 carries NO per-channel vulnerability mask — its
            // single damage gate is byte[0] & 8 (mapped to flags & 8,
            // the shared NewEvent default). MC1's writers additionally
            // check the +28 channel mask; admit their physical channel
            // at the seam (cross-column damage contract).
            e.f28 = 1;
            e.f128 = 30; // minSpeed
            e.f130 = 0; // maxSpeed — STATIONARY
            e.f126 = 30;
            e.max_life = 1000;
        }
        self.mc2_set_mana_half(i); // 500
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            let f = ((d & 0x7FF) as i32 - 1) as u16;
            e.f34 = f;
            e.f30 = f;
            e.f32 = f;
            e.f44 = 500;
            e.row156 = 75; // ABSOLUTE row index (:33899)
        }
        // Ordinal FIRST (:33900) — it feeds the wake stagger on the
        // very next line; unset f63 collapsed f58 to the constant
        // period+4 (no stagger — the degenerate archer wake, E10).
        self.ent[i].f63 = self.mc2_ord(4);
        let period = BEHAVIOR[75].v_26.max(1);
        self.ent[i].f58 = (period - (self.ent[i].f63 as i16 % period)) + 4; // :33902
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 0);
        self.mc2_shift_rot(i, 128, 256);
        Some(i)
    }

    /// `AddVilliger_4BF40` (:34037) — the Villager (5,13). TWO ctor
    /// RNG draws (facing, then the % 9 sprite pick) — the order is
    /// stream-visible.
    pub(crate) fn mc2_spawn_villager(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 13;
            e.tick70 = VILLAGER_BASE + 1; // actionIndex 105
            // MC2 carries NO per-channel vulnerability mask — its
            // single damage gate is byte[0] & 8 (mapped to flags & 8,
            // the shared NewEvent default). MC1's writers additionally
            // check the +28 channel mask; admit their physical channel
            // at the seam (cross-column damage contract).
            e.f28 = 1;
            e.f128 = 54;
            e.f130 = 18;
            e.f126 = 18;
        }
        let d = self.mc2_rand(i); // draw #1 (:34048)
        {
            let e = &mut self.ent[i];
            let f = ((d & 0x7FF) as i32 - 1) as u16;
            e.f34 = f;
            e.f30 = f;
            e.f32 = f;
            e.max_life = 1000;
            e.f140 = 0; // mana 0: drops nothing
            e.f44 = 500;
            e.row156 = 100; // ABSOLUTE row index (:34058)
            e.f58 = 64;
            e.f26 = 2;
        }
        // Per-model spawn ordinal → f63 (:34062) — herd cadence (E10).
        self.ent[i].f63 = self.mc2_ord(13);
        self.link(i, x, y, z);
        self.refill_life(i);
        let d2 = self.mc2_rand(i); // draw #2 (:34065)
        let sprite = match d2 % 9 {
            0..=2 => 242,
            3..=5 => 271,
            6 | 7 => 241,
            _ => 239,
        };
        self.mc2_set_sprite(i, sprite);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// `AddEvent09_0D_4DAB0` (:35031) — the (9,13) archer arrow:
    /// speed 384, life 5120/384 = 13, sprite 195 with the doubled
    /// quad.
    pub(crate) fn mc2_spawn_arrow(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 13;
            e.tick70 = ARROW_STATE;
            e.f126 = 384; // actSpeed
            e.f128 = 384; // minSpeed
            e.max_life = (5120 / 384) as u32; // 13
            e.flags &= !8; // byte[0] &= 0xF7 (:35038) — arrows are not targets
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite_x2(i, 195);
        Some(i)
    }

    // ---- the Goat block (8..=15, :11386-11462) --------------------------

    fn goat_tick(&mut self, i: usize, ctx: &MobCtx) {
        let role = self.ent[i].tick70 - GOAT_BASE;
        match role {
            0 => {
                self.mc2_patrol(i, GOAT_BASE);
                self.goat_snd(i, 0x4D);
                self.goat_speed_fixup(i);
            }
            1 => {
                self.mc2_idle(i, GOAT_BASE, ctx);
                self.goat_snd(i, 0x4D);
                self.goat_speed_fixup(i);
            }
            2 => {
                // sub_1F440 (:11410): the chase slot redirects into
                // FLEE.
                self.ent[i].tick70 = GOAT_BASE + 6;
                self.ent[i].f126 = self.ent[i].f128;
                self.goat_hit(i, ctx);
            }
            3 => {
                self.mc2_pack(i, GOAT_BASE);
                self.goat_snd(i, 0x4D);
                self.goat_speed_fixup(i);
            }
            4 => self.mc2_prekill(i, GOAT_BASE),
            5 => self.mc2_kill(i),
            6 => self.goat_hit(i, ctx),
            _ => {
                // AddGoat05_01 (:11452): sub_1D5D0 is a no-op for
                // StageVar2 == 0 — sound roll + speed by action.
                self.goat_snd(i, 0x4D);
                if self.ent[i].tick70 == GOAT_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                } else {
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
        }
    }

    /// `HitGoat_1F530` (:11441): flee + exit speed + the 0x2B roll.
    fn goat_hit(&mut self, i: usize, ctx: &MobCtx) {
        self.mc2_flee(i, GOAT_BASE, ctx);
        if self.ent[i].tick70 != GOAT_BASE + 6 {
            self.ent[i].f126 = self.ent[i].f130;
        }
        self.goat_snd(i, 0x2B);
    }

    /// The post-primitive `action == 14 → actSpeed = minSpeed` fixup
    /// shared by states 8/9/11/15 (:11393 etc.).
    fn goat_speed_fixup(&mut self, i: usize) {
        if self.ent[i].tick70 == GOAT_BASE + 6 {
            self.ent[i].f126 = self.ent[i].f128;
        }
    }

    /// The screech roll: one LCG, sound 46 on `% modulus == 0`.
    fn goat_snd(&mut self, i: usize, modulus: u32) {
        if self.mc2_rand(i) % modulus == 0 {
            self.snd(46, i);
        }
    }

    // ---- the Archer block (32..=39, :11624-11970) --------------------------

    fn archer_tick(&mut self, i: usize, ctx: &MobCtx) {
        let role = self.ent[i].tick70 - ARCHER_BASE;
        match role {
            0 => {
                self.mc2_patrol(i, ARCHER_BASE);
                if self.ent[i].tick70 == ARCHER_BASE + 2 {
                    self.archer_aim(i);
                }
            }
            1 => self.archer_brain(i, ctx),
            2 => {
                // AddArcher0504_1FF40 (:11884).
                let _ = self.mc2_chase_attack(i, ARCHER_BASE, ctx, Self::archer_fire);
                if self.ent[i].tick70 != ARCHER_BASE + 2 {
                    self.archer_unaim(i);
                    return;
                }
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    // Re-arm the target wizard's wanted timer per
                    // volley (:11900).
                    let t = self.ent[i].f146;
                    if t == PLAYER_TARGET {
                        self.mc2_arm_wanted(PLAYER_TARGET);
                    } else if (t as usize) < self.ent.len()
                        && self.ent[t as usize].class64 == 3
                        && self.ent[t as usize].model65 <= 1
                    {
                        self.mc2_arm_wanted(t);
                    }
                }
            }
            3 => {
                // sub_1FFE0 (:11907).
                self.mc2_pack(i, ARCHER_BASE);
                if self.ent[i].tick70 == ARCHER_BASE + 2 {
                    self.archer_aim(i);
                }
            }
            4 => {
                // HitArcher_20010 (:11918): the shrine-consumed
                // archer (f26 set) vanishes without a corpse.
                if self.ent[i].f26 != 0 {
                    self.ent[i].flags |= 0x400;
                } else {
                    self.mc2_prekill(i, ARCHER_BASE);
                }
            }
            5 => self.mc2_kill(i),
            6 => {
                // sub_20130: MISSING from the decompile (module
                // doc); unreachable for archers (flags bit 8
                // clear) — hold.
            }
            _ => {
                // AddScroll05_04_20140 (:11960): clear the shrine
                // flag; sub_1D5D0 no-op for StageVar2 == 0.
                self.ent[i].f26 = 0;
                if self.ent[i].tick70 == ARCHER_BASE + 2 {
                    self.archer_aim(i);
                }
            }
        }
    }

    /// `sub_1FAA0` (:11636) — the Archer idle/acquire brain.
    fn archer_brain(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].f26 = 0; // dword_0x10_16 = 0 every tick
        match self.mc2_state_head(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                self.ent[i].tick70 = ARCHER_BASE + 2; // 34 — hardwired
                self.mc2_alt_commit(i);
                self.archer_aim(i);
            }
            2 => self.ent[i].tick70 = ARCHER_BASE + 4,
            _ => {
                self.mc2_move_core(i);
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                if self.ent[i].f63 as i16 % period != 0 {
                    return;
                }
                if self.ent[i].f146 != 0 {
                    // Shrine handling (:11700-24): only a (10,45)
                    // stays a destination; walk to it and be
                    // consumed at 0x1000.
                    let t = self.ent[i].f146 as usize;
                    let shrine = t < self.ent.len()
                        && self.ent[t].class64 == 10
                        && self.ent[t].model65 == 45
                        && self.ent[t].flags & 0x400 == 0;
                    if !shrine {
                        self.ent[i].f146 = 0;
                    } else {
                        let (sp, tp) = {
                            let e = &self.ent[i];
                            let s = &self.ent[t];
                            ((e.x, e.y, e.z), (s.x, s.y, s.z))
                        };
                        if Self::mc2_dist3(sp, tp) > 0x1000 {
                            self.ent[i].f34 = Self::angle_between(sp.0, sp.1, tp.0, tp.1);
                        } else {
                            self.ent[i].f26 = 1;
                            self.ent[i].tick70 = ARCHER_BASE + 4;
                            self.ent[t].f26 += 1;
                        }
                    }
                    return;
                }
                self.mc2_wander_turn(i);
                let period4 = 4 * period;
                if self.ent[i].f63 as i16 % period4 == 0 {
                    // Scan A (:11768-11804): nearest class-3 ANYTHING,
                    // then POST-REJECT the single winner unless it is
                    // a wizard (model ≤ 1) with a live wanted timer —
                    // a nearer castle/balloon/non-wanted wizard voids
                    // the whole scan (falls to Scan B). E12.
                    let mut target = self.mc2_class3_scan(i, ctx).filter(|&s| {
                        let wizard = s == PLAYER_TARGET || self.ent[s as usize].model65 <= 1;
                        wizard && self.mc2_wanted_live(s)
                    });
                    if target.is_none() {
                        // Scan B: nearest model-9 creature, no cone
                        // (:11811).
                        let e = &self.ent[i];
                        let row = &BEHAVIOR[e.row156 as usize];
                        let range = (row.v_28 as i32) * (row.v_28 as i32);
                        let (ex, ey) = (e.x, e.y);
                        let mut best: Option<(u16, i32)> = None;
                        for (j, c) in self.ent.iter().enumerate().skip(1) {
                            if c.class64 == 5
                                && c.model65 == 9
                                && c.act_life >= 0
                                && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                                && c.flags & 0x400 == 0
                            {
                                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                if d2 <= range && best.is_none_or(|(_, bd)| d2 < bd) {
                                    best = Some((j as u16, d2));
                                }
                            }
                        }
                        target = best.map(|(s, _)| s);
                    }
                    if let Some(t) = target {
                        // Shrines never become targets (:11824).
                        let is_shrine = (t as usize) < self.ent.len()
                            && self.ent[t as usize].class64 == 10
                            && self.ent[t as usize].model65 == 45;
                        if !is_shrine {
                            self.ent[i].f146 = t;
                            self.ent[i].tick70 = ARCHER_BASE + 2;
                            self.archer_aim(i);
                            return;
                        }
                    }
                    // Scan C: pack (:11840-69).
                    if let Some(l) = self.mc2_pack_scan(i, false) {
                        self.ent[i].f52 = l;
                        self.ent[i].tick70 = ARCHER_BASE + 3;
                    }
                }
            }
        }
    }

    /// `sub_20060` (:11936): one LCG, stop, firing sprite 206 or 1
    /// by `% 0x14 <= 10`, shift-rot, record target class/model into
    /// the filter bytes.
    fn archer_aim(&mut self, i: usize) {
        let d = self.mc2_rand(i);
        self.ent[i].f126 = 0;
        let sprite = if d % 0x14 <= 10 { 206 } else { 1 };
        self.mc2_set_sprite(i, sprite);
        self.mc2_shift_rot(i, 128, 256);
        let t = self.ent[i].f146;
        let (c, m) = if t == PLAYER_TARGET {
            (3, 0)
        } else if (t as usize) < self.ent.len() {
            (self.ent[t as usize].class64, self.ent[t as usize].model65)
        } else {
            (3, 0)
        };
        self.ent[i].f66 = c;
        self.ent[i].f67 = m;
    }

    /// `sub_200F0` (:11950): back to the patrol sprite/speed.
    fn archer_unaim(&mut self, i: usize) {
        self.ent[i].f126 = self.ent[i].f128;
        self.mc2_set_sprite(i, 0);
        self.mc2_shift_rot(i, 128, 256);
        self.ent[i].f66 = 3;
        self.ent[i].f67 = 0xFF;
    }

    /// `sub_1CCE0` (:9713) — the arrow-fire thunk: spawn the (9,13)
    /// arrow aimed at the target (yaw + pitch), lift by fov/2, arm
    /// f44 = 250, and poke the target wizard's danger timer
    /// (sub_5EF70 → 100).
    fn archer_fire(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let (x, y, z, own, fov) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f84)
        };
        let Some((tx, ty, tz)) = self.mc2_target(target, ctx) else {
            return false;
        };
        let Some(a) = self.mc2_spawn_arrow(x, y, z) else {
            return false;
        };
        self.ent[a].id24 = own;
        let yaw = Self::angle_between(x, y, tx, ty);
        self.ent[a].f30 = yaw;
        let dh = Self::isqrt(Self::dist2_sq(x, y, tx, ty) as u32) as i32;
        self.ent[a].f32 = Self::pitch_toward(z, tz, dh);
        let (ax, ay) = (self.ent[a].x, self.ent[a].y);
        let az = self.ent[a].z.wrapping_add((fov / 2) as i16);
        self.move_relink(a, ax, ay, az);
        self.ent[a].f146 = self.ent[i].f146;
        self.ent[a].f44 = 250;
        let (tc, tm) = if target == PLAYER_TARGET {
            (3, 0)
        } else {
            (
                self.ent[target as usize].class64,
                self.ent[target as usize].model65,
            )
        };
        self.ent[a].f66 = tc;
        self.ent[a].f67 = tm;
        if target == PLAYER_TARGET {
            self.player_danger = 100; // sub_5EF70 (:60598)
        }
        // No shots++: a creature volley never bumps the player's
        // accuracy stat in retail (E27 sibling of roster's m15).
        true
    }

    /// `AddArcherArrow_672E0` (:58852) — the (9,13) flight tick:
    /// first-tick whoosh (global stage LCG picks sound 33/34), polar
    /// step, victim probe, terrain/expiry impact. Returns true when
    /// terrain changed (never — arrows don't dig).
    pub(crate) fn mc2_arrow_tick(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].flags & F_WHOOSH == 0 {
            self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
            let snd = ((self.rand & 1) + 33) as u8;
            self.snd(snd, i);
            self.ent[i].flags |= F_WHOOSH;
        }
        let e = &self.ent[i];
        let mut pos = (e.x, e.y, e.z);
        Self::polar_step(&mut pos, e.f30, e.f32, e.f126);
        // Victim probe (sub_10780 → the shared tile-chain scan;
        // module-doc APPROX). Owner-immunity via id24 like MC1, PLUS
        // the projectile's target-class filter: the archer's fire
        // state launches arrows with xtype=3/xsubtype=-1 (sub_200F0
        // :11955-56) and sub_10780 skips every victim outside that
        // class (:3766-69) — arrows pass through fellow archers and
        // villagers, they only strike WIZARDS. (APPROX: the original
        // keeps scanning the ring past a non-matching body in the
        // same tick; we let the arrow fly on and re-probe next tick.)
        let hit = match self.victim_scan_at(i, pos, ctx) {
            Some(crate::mc1::combat::MailTarget::Pool(v)) if self.ent[v].class64 != 3 => None,
            other => other,
        };
        let above_ground = self.ground_z(pos.0, pos.1) as i16 <= pos.2;
        if above_ground {
            let life = self.ent[i].act_life;
            self.ent[i].act_life = life - 1;
            if life != 0 && hit.is_none() {
                self.move_relink(i, pos.0, pos.1, pos.2);
                return;
            }
        }
        // Impact (LABEL_10 / the entity branch minus the shielded
        // sub_68740 ricochet — no shielded targets in the slice):
        // move to the victim, area-write ch0 with f44, despawn.
        match hit {
            Some(crate::mc1::combat::MailTarget::Pool(v)) => {
                let (vx, vy, vz) = (self.ent[v].x, self.ent[v].y, self.ent[v].z);
                self.move_relink(i, vx, vy, vz);
            }
            Some(crate::mc1::combat::MailTarget::Player) => {
                let (px, py, pz) = (ctx.px, ctx.py, ctx.pz);
                self.move_relink(i, px, py, pz);
            }
            None => self.move_relink(i, pos.0, pos.1, pos.2),
        }
        let amt = self.ent[i].f44 as u32;
        self.area_write(i, 0, amt, ctx, false, false);
        self.ent[i].flags |= 0x400;
    }

    // ---- the Villager block (104..=111, :14498-14718) ----------------------

    fn villager_tick(&mut self, i: usize, ctx: &MobCtx) {
        let role = self.ent[i].tick70 - VILLAGER_BASE;
        match role {
            0 | 2 | 3 => {
                // sub_23320/23640/23660: re-enter the brain.
                self.ent[i].tick70 = VILLAGER_BASE + 1;
                self.villager_brain(i, ctx);
            }
            1 => self.villager_brain(i, ctx),
            4 => {
                // KillTownie_23680 (:14668).
                if self.ent[i].f26 != 0 {
                    self.ent[i].flags |= 0x400;
                    return;
                }
                let killer = self.ent[i].f38;
                if killer == PLAYER_TARGET {
                    self.mc2_arm_wanted(PLAYER_TARGET);
                }
                self.mc2_prekill(i, VILLAGER_BASE);
            }
            5 => self.mc2_kill(i),
            6 => {
                // HitTownie_23710 (:14691).
                self.mc2_flee(i, VILLAGER_BASE, ctx);
                if self.ent[i].tick70 != VILLAGER_BASE + 6 {
                    self.ent[i].f146 = 0;
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
            _ => {
                // AddTownie05_0D_23750 (:14707): 1D5D0 no-op; speed
                // by action.
                if self.ent[i].tick70 == VILLAGER_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                } else {
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
        }
    }

    /// `sub_23340` (:14506) — the townie wander brain.
    fn villager_brain(&mut self, i: usize, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            1 => {
                // A wizard hit arms its wanted timer (:14561-63).
                let src = self.ent[i].f40;
                if src == PLAYER_TARGET {
                    self.mc2_arm_wanted(PLAYER_TARGET);
                }
                self.ent[i].f146 = src;
                self.ent[i].tick70 = VILLAGER_BASE + 6; // 110
            }
            2 => self.ent[i].tick70 = VILLAGER_BASE + 4, // 108
            _ => {
                self.mc2_move_core(i);
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    if self.ent[i].f146 != 0 {
                        // Rally to a (10,45) building flag within
                        // 0x800; consumed if it has capacity
                        // (:14584-99: shrine.minSpeed > shrine
                        // counter).
                        let t = self.ent[i].f146 as usize;
                        let shrine = t < self.ent.len()
                            && self.ent[t].class64 == 10
                            && self.ent[t].model65 == 45
                            && self.ent[t].flags & 0x400 == 0;
                        if shrine {
                            let (sp, tp) = {
                                let e = &self.ent[i];
                                let s = &self.ent[t];
                                ((e.x, e.y, e.z), (s.x, s.y, s.z))
                            };
                            if Self::mc2_dist3(sp, tp) > 0x800 {
                                self.ent[i].f34 = Self::angle_between(sp.0, sp.1, tp.0, tp.1);
                            } else if (self.ent[t].f128 as i32) > self.ent[t].f26 as i32 {
                                self.ent[i].f26 = 1;
                                self.ent[i].tick70 = VILLAGER_BASE + 4;
                                self.ent[t].f26 += 1;
                            } else {
                                self.ent[i].f146 = 0;
                                self.ent[i].f126 = self.ent[i].f130;
                            }
                        } else {
                            self.ent[i].f146 = 0;
                            self.ent[i].f126 = self.ent[i].f130;
                        }
                    } else {
                        self.mc2_wander_turn(i);
                        // Nearest ENTERABLE building — a (10,45)
                        // whose bldgprm row has byte_2 & 1 (:14619),
                        // no range limit: townies are NEVER in free
                        // wander, they permanently march at the
                        // nearest dwelling (the causeway files in
                        // retail are this scan + water flanks +
                        // slope refusal — trace §6).
                        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                        let mut best: Option<(u16, i32)> = None;
                        for (j, c) in self.ent.iter().enumerate().skip(1) {
                            if c.class64 == 10
                                && c.model65 == 45
                                && c.flags & 0x400 == 0
                                && self.assets.build_tab.get(c.f71 as usize).is_some()
                                // bldgprm byte_2 & 1 ENTERABLE gate
                                // (:14619): dwellings attract townies;
                                // stone/route templates (the dis-13
                                // causeway obelisks, flags 0x08/0x18)
                                // must not capture them — the walker
                                // trace's D3.
                                && self
                                    .assets
                                    .bldgprm
                                    .get(c.f71 as usize)
                                    .is_some_and(|p| p.flags & 1 != 0)
                            {
                                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                if best.is_none_or(|(_, bd)| d2 < bd) {
                                    best = Some((j as u16, d2));
                                }
                            }
                        }
                        if let Some((b, _)) = best {
                            self.ent[i].f146 = b;
                            self.ent[i].f126 = self.ent[i].f130 + 12;
                        }
                    }
                }
                let _ = ctx;
            }
        }
        // LABEL_43 tail: flee state walks at minSpeed.
        if self.ent[i].tick70 == VILLAGER_BASE + 6 {
            self.ent[i].f126 = self.ent[i].f128;
        }
    }

    // ---- class 2: scenery (tree / stone / dolmen) ---------------------------

    /// `AddTree_4AC40` (:33433) — the MC2 tree (2,0). FOUR per-entity
    /// LCG draws (lifespan, x/y jitter, sprite pick), byte-faithful.
    /// APPROX register: the class-2 tick column is unported (trees
    /// hold inert — the natural-lifespan decay and burn states join
    /// the Phase-4 roster).
    pub(crate) fn mc2_spawn_tree(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = 0;
            e.tick70 = 0;
            e.f26 = (i % 11) as i16; // dword_0x10_16: phase stagger
            e.f56 = 1; // byte_0x38_56: burnable (ch0 intake)
            // Cross-column damage contract: MC2's burnable gate IS
            // `(1 << ch) & byte_0x38_56` — admit ch0 through MC1's
            // +28 mask so the shared area writer reaches the tree.
            e.f28 = 1;
        }
        let d = self.mc2_rand(i);
        self.ent[i].act_life = (d % 0x1388 + 2500) as i32;
        let jx = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        let jy = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        let (nx, ny) = (x.wrapping_add(jx as u16), y.wrapping_add(jy as u16));
        self.link(i, nx, ny, z);
        let d = self.mc2_rand(i);
        self.mc2_set_sprite(i, if d & 1 != 0 { 84 } else { 83 });
        Some(i)
    }

    /// `AddStone_4AD70` (:33466) — the standing stone (2,1):
    /// non-collidable (byte[0] &= 0xF7), state 3, sprite row 79.
    pub(crate) fn mc2_spawn_stone(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = 1;
            e.tick70 = 3;
            e.f26 = (i % 11) as i16;
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 79);
        Some(i)
    }

    /// `AddDolmen_4ADF0` (:33484) — the dolmen (2,2), "similar as
    /// Obelisk": non-collidable, state 6, sprite row 39, quad
    /// ShiftRot(1024, 1024).
    pub(crate) fn mc2_spawn_dolmen(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 2;
            e.model65 = 2;
            e.tick70 = 6;
            e.f26 = (i % 11) as i16;
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 39);
        self.mc2_shift_rot(i, 1024, 1024);
        Some(i)
    }

    // ---- class 10 models 0/1: ground fire + the big explosion --------------

    /// `NewAdd0A00_4E320` (:35332) — the MC2 ground fire/eruption
    /// element (every explosion chain resolves into these): life 8,
    /// area-damage amount 400 (`subSpellIndex`), sprite row 7, quad
    /// (128, 128). Flag ops: `dword &= 0xFFFDFFF7` (clears collidable
    /// and byte[2] bit 1) then `byte[2] |= 2` — byte[2] doubles as
    /// the paint `inType` seed, its bit 0 as the no-damage gate.
    pub(crate) fn mc2_spawn_fire(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 0;
            e.tick70 = 0;
            e.max_life = 8;
            e.f140 = 400; // subSpellIndex = sub_10C80's ch0 amount
            e.f56 = 0;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.ent[i].act_life = 8;
        self.mc2_set_sprite(i, 7);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// `NewAdd0A01_4E3B0` (:35354) — the "Big explosion" (10,1), the
    /// route marker: a 1-life seeder whose whole job is the (10,0)
    /// cluster. Sprite row 41. (The dynamic light AddEvent2_847D0 is
    /// presentation, unported.)
    pub(crate) fn mc2_spawn_big_explosion(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 1;
            e.tick70 = 1;
            e.max_life = 1;
            e.f140 = 400;
            e.f26 = 0; // dword_0x10_16 = the seeding ring span
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.ent[i].act_life = 1;
        self.mc2_set_sprite(i, 41);
        Some(i)
    }

    /// `sub_30D50` (:22692) — the (10,0) fire tick: optional fuse
    /// (`dword_0x10_16 & 3`), then per active tick: one-shot
    /// activation (area damage 400 via sub_10C80 ≡ our `area_write`
    /// under the cross-column mask contract, gated on byte[2] bit 0;
    /// terrain burn — worn-path repaints 26/10/11 through the
    /// texture-band painter, else the scorch dig; flicker draw; sound
    /// 3), the z rule (drift by flicker above ground, clamp up, cave
    /// ceiling clamp), anim advance.
    pub(crate) fn mc2_fire_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].f26 & 3 != 0 {
            self.ent[i].f26 -= 1;
            return false;
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < -1 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        self.ent[i].flags &= !1;
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let ground = self.ground_z(x, y) as i16;
        let mut dirty = false;
        if self.ent[i].flags & 2 == 0 {
            let in_type = ((self.ent[i].flags >> 16) & 0xFF) as u8;
            if self.ent[i].flags & 0x1_0000 == 0 {
                let amt = self.ent[i].f140 as u32;
                self.area_write(i, 0, amt, ctx, false, false);
            }
            let (cx, cy) = (
                ((x.wrapping_add(128)) >> 8) as u8,
                ((y.wrapping_add(128)) >> 8) as u8,
            );
            let t = crate::mc1::features::tile(cx, cy);
            let ty = self.t.tile_type[t];
            if ty != 0 {
                match ty {
                    26 => {
                        self.mc2_paint_cell(in_type, cx, cy, 0x14);
                        dirty = true;
                    }
                    10 => {
                        self.mc2_paint_cell(in_type, cx, cy, 0x15);
                        dirty = true;
                    }
                    11 => {
                        self.mc2_paint_cell(in_type, cx, cy, 0x16);
                        dirty = true;
                    }
                    _ => {
                        // sub_104A0 (:2052) reads the UNROUNDED cell.
                        let raw = crate::mc1::features::tile((x >> 8) as u8, (y >> 8) as u8);
                        if !(6..=0x22).contains(&ty)
                            && self.t.angle[t] & 7 != 1
                            && (z as i32 - ground as i32) <= 128
                            && (1u32 << (self.t.angle[raw] & 0xF)) & 1 == 0
                        {
                            let d = self.ent_rand(i);
                            self.dig_scorch(i, -((d % 7) as i16));
                            dirty = true;
                        }
                    }
                }
            }
            self.ent[i].flags |= 2;
            let d = self.ent_rand(i);
            self.ent[i].f44 = ((d % 0x41) as i32 - 32) as u16;
            self.snd(3, i);
        }
        // sub_580E0(pos, ground, 0, 0, flicker).
        let mut nz = self.ent[i].z;
        Self::mc2_alt_core(&mut nz, ground, 0, self.ent[i].f44 as i16);
        self.ent[i].z = nz;
        // Cave ceiling clamp (EF:22752-58).
        if self.is_cave() {
            let c = (self.ceiling_z(x, y) - self.ent[i].f84 as i32) as i16;
            if self.ent[i].z > c {
                self.ent[i].z = c;
            }
        }
        // sub_585A0: frame advance (the renderer's 22..=36 band caps
        // by the sprite's span; retail caps by x_BYTE_D8A2E).
        self.ent[i].frame88 = self.ent[i].frame88.saturating_add(1);
        dirty
    }

    /// `AddQuickfair0A_01_30F60` (:22768) — the (10,1) tick: two
    /// acting ticks (post-decrement `life-- < 0`), sound 3 once, and
    /// per tick a sweep of SEARCH rings 0..=`dword_0x10_16` seeding
    /// (10,0) children at `pos - 96 + 192*cell ± rand%129-64` with a
    /// ~50% per-cell draw; children inherit id + yaw and raise
    /// byte[0] bit 7.
    pub(crate) fn mc2_big_explosion_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.snd(3, i);
        }
        let ring = self.ent[i].f26 as i32;
        let cells = self.ring_cells(ring, ring);
        let (px, py, pz, id, yaw) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30)
        };
        for (dx, dy) in cells {
            let d = self.ent_rand(i);
            if 2 * ((d % 0x9D) as i32 / 79) - 1 > 0 {
                let d = self.ent_rand(i);
                let nx = (px as i32 - 96 + 192 * dx as i32 + (d % 0x81) as i32 - 64) as u16;
                let d = self.ent_rand(i);
                let ny = (py as i32 - 96 + 192 * dy as i32 + (d % 0x81) as i32 - 64) as u16;
                if let Some(c) = self.mc2_spawn_fire(nx, ny, pz) {
                    self.ent[c].id24 = id;
                    self.ent[c].f30 = yaw;
                    self.ent[c].flags |= 0x80;
                }
            }
        }
    }

    // ---- class 10 model 45: buildings --------------------------------------

    /// `AddTerrainModification_50250` (:36677) + the `sub_49A30`
    /// building setup (:32753) that both spawn paths run right after
    /// the creator (PrepareEvents Events.cpp:348 / disposition
    /// :33089). `bldg` = the THING's par1 = the BUILD00/BLDGPRM
    /// building id. Draws NO entity RNG (SetEntityIndexAndRot is
    /// RNG-free).
    ///
    /// APPROX register (like the module doc): the VGA half-resolution
    /// footprint shrink (:32771) is the low-res render mode, skipped;
    /// `dword_0x10_16 = 2` has no ported consumer; the id-68 player
    /// castle global (:32812) lands with MC2 castles.
    pub(crate) fn mc2_spawn_building(
        &mut self,
        x: u16,
        y: u16,
        z: i16,
        bldg: u16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 45;
            e.max_life = 30;
            e.tick70 = 51; // actionIndex 0x33
            // byte_0x38_56 = 33 (:36688): ch0 damage intake + bit 5 —
            // buildings are DESTRUCTIBLE by area writers; the
            // productive kind adds bit 1 (claim channel) below.
            // f28 mirrors the intake bits for the SHARED writer gate
            // (the cross-column damage contract — area_write tests
            // f28, not f56; docs/traces/mc2-possession-delivery.md:
            // without it the possess pulse's ch1 claim mail and ch0
            // area damage are both dropped at the gate).
            e.f56 = 33;
            e.f28 = 1;
            // byte[0] = 9 (:36687): bit 3 targetable + bit 0 (the
            // unclaimed/no-flag marker; the claim clears it).
            e.flags |= 1;
            // dword_0x10_16: ctor 4 → sub_49A30 overwrites 2
            // (:32757) — the occupant count the house tick pops.
            e.f26 = 2;
        }
        self.mc2_set_sprite(i, 177);
        // sub_49A30: footprint metadata + snapped placement.
        let def = self.assets.build_tab.get(bldg as usize).copied();
        let (w, h) = def.map_or((0u8, 0u8), |d| (d.w, d.h));
        // Snap to the tile corner (:32777-79), then the parity
        // alignment: an odd top-left corner sum shifts one tile +x
        // (:32782-88).
        let mut sx = x & 0xFF00;
        let sy = y & 0xFF00;
        let mut tlx = ((sx >> 8) as u8).wrapping_sub(w / 2);
        let tly = ((sy >> 8) as u8).wrapping_sub(h / 2);
        if (tlx.wrapping_add(tly)) & 1 != 0 {
            sx = sx.wrapping_add(256);
            tlx = tlx.wrapping_add(1);
        }
        // z = 32 * the 4-corner average over the footprint (:32790,
        // GetTerrainHeightFromSquare_48DF0 ≡ our avg4 — chassis).
        let site = (32 * self.avg4(tlx, tly, h, w)) as i16;
        let _ = z;
        self.link(i, sx, sy, site);
        let prm = self
            .assets
            .bldgprm
            .get(bldg as usize)
            .copied()
            .unwrap_or_default();
        {
            let e = &mut self.ent[i];
            e.f128 = ((w as u16 * h as u16) >> 4) as i16; // minSpeed_132
            // SetShiftByCastle_49EC0 (:32882): the footprint quad.
            e.f78 = 0;
            e.f80 = ((w as u16) << 8).wrapping_add(1280) >> 1;
            e.f82 = ((h as u16) << 8).wrapping_add(1280) >> 1;
            e.f84 = 256;
            e.f71 = bldg as u8;
            e.act_life = 30;
            e.f140 = prm.rate as i32; // subSpellIndex = production
            e.f136 = 0;
            if prm.flags & 8 == 0 {
                e.f56 |= 2;
                e.f28 |= 2; // claim channel, writer-gate mirror
                e.f136 = (1000 * prm.rate as i32) >> 7;
            }
        }
        Some(i)
    }

    /// `sub_57390` (:39746): building placement clears its footprint
    /// tile — scenery entities removed, creatures killed EXCEPT the
    /// protected models {6, 8, 10, 16, 22, 23, 27} (+ 25 while in
    /// action 200). `builder` = the building's own slot (skipped).
    pub(crate) fn mc2_building_clear_tile(&mut self, t: usize, builder: usize) {
        let mut j = self.map_entity[t] as usize;
        while j != 0 {
            let next = self.ent[j].next20 as usize;
            if j != builder {
                match self.ent[j].class64 {
                    2 => self.free_entity(j),
                    5 => {
                        let m = self.ent[j].model65;
                        let protected = matches!(m, 6 | 8 | 10 | 16 | 22 | 23 | 27)
                            || (m == 25 && self.ent[j].tick70 == 200);
                        if !protected {
                            self.ent[j].act_life = -1;
                        }
                    }
                    _ => {}
                }
            }
            j = next;
        }
    }

    /// `ApplyTerrainModification_37240` (:27181), the 30-tick build
    /// action (state 51): first countdown tick clears the footprint
    /// (sub_57390), every tick lerps the height plane toward the
    /// building data's pad heights, every 5th tick (and the last)
    /// paints the walkable village tiles, and the final tick parks
    /// the entity as the static building (state 52) with its
    /// production timer. Footprint cells = BUILD00 data, TWO bytes
    /// per cell: [0] = paint code (0xff = none), [1] = pad height
    /// (0xff = none). Returns true (terrain changed).
    ///
    /// APPROX register: the one-at-a-time build carousel
    /// (IsNextEvent0A_2A_37740/sub_377A0) is skipped — all authored
    /// buildings raise concurrently at load. The sub_462A0 retile,
    /// the sub_45DC0 texture-band paint and the sub_48A20 pad-edge
    /// rings are the real ports ([`crate::mc2::terrain_paint`]) at
    /// the retail cadence. On caves, unless the bldgprm row carries
    /// flag 4 (no-cave-raise), EVERY footprint cell (pad or not)
    /// lerps the ceiling toward `min(max(floor, base) + 80, 255)`
    /// and re-asserts the invariant per tick (:27349-27373) — the
    /// headroom bubble that makes rock-embedded buildings enterable.
    /// The instant-placement sibling (`sub_36FC0`, same arm at
    /// :27114-27137) has no ported caller yet (`sub_5C950` stage
    /// machinery — unported).
    pub(crate) fn mc2_building_tick(&mut self, i: usize) -> bool {
        let bldg = self.ent[i].f71 as usize;
        let Some(def) = self.assets.build_tab.get(bldg).copied() else {
            self.ent[i].tick70 = 52;
            return false;
        };
        let (w, h) = (def.w as usize, def.h as usize);
        // Copy the footprint cells (2 bytes each) out of the bank —
        // the loops below write the terrain planes.
        let start = def.offset as usize;
        let Some(cells) = self
            .assets
            .build_dat
            .get(start..start + 2 * w * h)
            .map(<[u8]>::to_vec)
        else {
            self.ent[i].tick70 = 52;
            return false;
        };
        let cx = ((self.ent[i].x.wrapping_add(128)) >> 8) as u8;
        let cy = ((self.ent[i].y.wrapping_add(128)) >> 8) as u8;
        let tlx = cx.wrapping_sub((w / 2) as u8);
        let tly = cy.wrapping_sub((h / 2) as u8);
        let base = self.ent[i].z >> 5; // v35
        // v50 (:27251): raise the cave ceiling over the footprint
        // unless the bldgprm row says no-cave-raise (flags & 4).
        let cave_raise = self.is_cave()
            && self
                .assets
                .bldgprm
                .get(bldg)
                .is_none_or(|b| b.flags & 4 == 0);
        self.ent[i].act_life -= 1;
        let life = self.ent[i].act_life;

        if life <= 0 {
            // Final frame (:27256-79): the per-cell sub_462A0 sweep
            // over every footprint cell with a paint code, then park
            // as the static building with the pad-edge rings
            // (:27289-304, thickness 2 then 5).
            for dy in 0..h {
                for dx in 0..w {
                    if cells[2 * (dy * w + dx)] == 0xff {
                        continue;
                    }
                    let (cx2, cy2) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                    self.mc2_retile_region(cx2, cy2, cx2, cy2);
                }
            }
            let e = &mut self.ent[i];
            e.tick70 = 52;
            e.act_life = 1000 * e.f140;
            // The flag protocol (:27292-97): owned → bit 0 cleared
            // (the flag flies), unowned → set (no flag).
            if e.f144 != 0 {
                e.flags &= !1;
            } else {
                e.flags |= 1;
            }
            e.site_z = e.z;
            let (x, y) = (e.x, e.y);
            self.ent[i].z = self.ground_z(x, y) as i16;
            self.mc2_pad_edge_ring(tlx, tly, (h / 2) as u8, (w / 2) as u8, 2);
            self.mc2_pad_edge_ring(tlx, tly, (h / 2) as u8, (w / 2) as u8, 5);
            return true;
        }

        // First countdown tick: the footprint kill (:27310-28).
        if self.ent[i].max_life as i32 - 1 == life {
            for dy in 0..h {
                for dx in 0..w {
                    let t = crate::mc1::features::tile(
                        tlx.wrapping_add(dx as u8),
                        tly.wrapping_add(dy as u8),
                    );
                    self.mc2_building_clear_tile(t, i);
                }
            }
        }

        // Height lerp toward pad height + base (:27341-44), marking
        // touched flat tiles as village ground (angle low bits 1);
        // then the cave headroom-bubble ceiling lerp on EVERY
        // footprint cell — pad or not (:27349-73).
        for dy in 0..h {
            for dx in 0..w {
                let cell = dy * w + dx;
                let pad = cells[2 * cell + 1];
                let t = crate::mc1::features::tile(
                    tlx.wrapping_add(dx as u8),
                    tly.wrapping_add(dy as u8),
                );
                if pad != 0xff {
                    let target = pad as i32 + base as i32;
                    let cur = self.t.height[t] as i32;
                    self.t.height[t] = (cur + (target - cur) / life as i32) as u8;
                    if self.t.angle[t] & 7 == 0 {
                        self.t.angle[t] = (self.t.angle[t] & 0xF0) | 1;
                        let (cx2, cy2) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                        self.mc2_retile_region(cx2, cy2, cx2, cy2);
                    }
                }
                if cave_raise {
                    let bubble = (self.t.height[t] as i32).max(base as i32) + 80;
                    let bubble = bubble.min(255);
                    let cur = self.t.ceiling[t] as i32;
                    if bubble > cur {
                        self.t.ceiling[t] = (cur + (bubble - cur) / life as i32) as u8;
                    }
                    self.cave_seal_fixup(t);
                }
            }
        }

        // Every 5th tick + the last (:27381-27427): the walkable
        // village pre-paint for cells with a paint code, then the
        // sub_45DC0 texture-band overpaint (the code interpreter;
        // painted cells self-lock via angle bit 7 so the next village
        // pass can't clobber them).
        if life % 5 == 0 || life == 1 {
            for dy in 0..h {
                for dx in 0..w {
                    if cells[2 * (dy * w + dx)] == 0xff {
                        continue;
                    }
                    let t = crate::mc1::features::tile(
                        tlx.wrapping_add(dx as u8),
                        tly.wrapping_add(dy as u8),
                    );
                    self.t.angle[t] = (self.t.angle[t] & 0xF0) | 1;
                    self.t.tile_type[t] = 1;
                }
            }
            for dy in 0..h {
                for dx in 0..w {
                    let code = cells[2 * (dy * w + dx)];
                    if code == 0xff {
                        continue;
                    }
                    self.mc2_paint_cell(
                        dx as u8,
                        tlx.wrapping_add(dx as u8),
                        tly.wrapping_add(dy as u8),
                        code,
                    );
                }
            }
        }
        true
    }

    /// `GetRandManaSphere_38270` (:27917) — one occupant out of a
    /// dying/besieged building: ONE entity-RNG draw %12 → 0-1 archers
    /// (dock 33), 2-3 trader (113), 4-8 villager (105), 9-11 settler
    /// (97).
    pub(crate) fn mc2_rand_occupant(&mut self, i: usize, x: u16, y: u16, z: i16) -> Option<usize> {
        let d = self.mc2_rand(i) % 12;
        let (s, dock) = match d {
            0 | 1 => (self.mc2_spawn_archers(x, y, z), 33),
            2 | 3 => (self.mc2_spawn_m14(x, y, z), 113),
            4..=8 => (self.mc2_spawn_villager(x, y, z), 105),
            _ => (self.mc2_spawn_m12(x, y, z), 97),
        };
        let s = s?;
        self.ent[s].tick70 = dock;
        Some(s)
    }

    /// `AddHouse0A_2D_38330` (:27959), the parked building (state
    /// 52): the CompareEvent08_38B00 damage core (death → state 53),
    /// the militia pop on a non-lethal hit, the possess-claim intake
    /// (the PLAYTEST-2 flag report — claimed buildings fly the flag),
    /// and the per-tick terrain z-snap.
    ///
    /// APPROX register: the mana-sphere production roll (:28040-58,
    /// full enterable houses) and SetMaxDistance_5C8D0 are the
    /// Phase-4.6 economy track; the byte[2]&0x20 strong-claim lock
    /// waits for the MC2 spell column (all claims run the weak
    /// possess variant :28035-40); the claimed sprite-row colorize
    /// (`word_0x5A_90 += color`, :28039) rides our renderer's team
    /// tint instead of the pre-colored row band.
    pub(crate) fn mc2_house_tick(&mut self, i: usize) {
        // CompareEvent08_38B00 (:28255): 0 idle / 1 hit / 2 dead.
        self.ent[i].f40 = 0;
        let status = if self.ent[i].act_life < 0 {
            2
        } else if self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].act_life -= amt as i32;
            self.ent[i].f40 = src;
            if self.ent[i].act_life < 0 {
                self.ent[i].f38 = src;
                2
            } else {
                self.ent[i].mail[0] = (0, 0);
                1
            }
        } else {
            0
        };
        if status == 2 {
            // Lethal: the RemoveCastleStage_385C0 teardown (state 53).
            self.ent[i].tick70 = 53;
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            self.ent[i].z = self.ground_z(x, y) as i16;
            return;
        }
        if status == 1 && self.ent[i].f26 > 2 {
            // Militia pop (:27994-28015): one occupant out to defend
            // (enterable kind only), and the attacker goes wanted.
            self.ent[i].f26 -= 1;
            let bldg = self.ent[i].f71 as usize;
            let enterable = self
                .assets
                .bldgprm
                .get(bldg)
                .is_some_and(|b| b.flags & 1 != 0);
            if enterable {
                let (x, y, z, off, atk) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z, e.f80, e.f40)
                };
                if let Some(a) = self.mc2_spawn_archers(x.wrapping_add(off), y, z) {
                    self.ent[a].tick70 = 33;
                    self.ent[a].mail[0] = (1, atk);
                }
            }
            let atk = self.ent[i].f40;
            self.mc2_arm_wanted(atk);
        }
        // The claim intake (:28022-40): possess ch1 → new owner,
        // chime 4 at the claimer, flag bit 0 cleared (the flag
        // FLIES), sprite re-set. Claimability is the DELIVERY's
        // f56-bit-1 gate — stone templates (bldgprm flags & 8) never
        // set it, so they can never receive this mail.
        if self.ent[i].mail[1].1 != 0 {
            let src = self.ent[i].mail[1].1;
            self.ent[i].mail[1] = (0, 0);
            if src != self.ent[i].f144 {
                self.ent[i].f144 = src;
                self.ent[i].flags &= !1;
                if src == crate::mc1::mobs::PLAYER_TARGET {
                    self.snd_player(4);
                }
                self.mc2_set_sprite(i, 177);
            }
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
    }

    // ---- dispatch + awake --------------------------------------------------

    /// The MC2 class-5 per-state dispatch (`sub_57730`'s class-5
    /// table, :40116/:1242) — the MovementVerb::Mc2 arm. Unknown
    /// actions disable the entity like retail's invalid-row path
    /// (:40177) and count a misfit.
    pub(crate) fn mc2_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        let action = self.ent[i].tick70;
        // The shared class-5 `8*M+7` slot (`sub_1D5D0`, EF:9977) — a
        // CONTROLLED creature. StageVar2 (port field: site_z, free on
        // creatures) selects the body: 12 = Metamorph pose-puppet, 13 =
        // Summon-Army allied AI. Stage-HELD kinds (1..=10, 15) never
        // reach here — the world dispatch seam routes them through
        // `World::mc2_held_tick` (stagevars.rs, Session H6/E16).
        // StageVar2 == 0 (every ordinary spawn) is a no-op, so those
        // fall through to the per-model dispatch
        // (docs/spell-audit/summon-creatures.md).
        if action & 7 == 7 && self.ent[i].site_z != 0 {
            match self.ent[i].site_z {
                12 => self.mc2_metamorph_creature_tick(i, ctx),
                13 => self.mc2_summon_creature_tick(i, ctx),
                _ => {}
            }
            return;
        }
        match action {
            0..=7 => self.m0_tick(i, ctx),
            8..=15 => self.goat_tick(i, ctx),
            16..=23 => self.m2_tick(i, ctx),
            24..=31 => self.m3_tick(i, ctx),
            32..=39 => self.archer_tick(i, ctx),
            72..=79 => self.m9_tick(i, ctx),
            96..=103 => self.m12_tick(i, ctx),
            104..=111 => self.villager_tick(i, ctx),
            112..=119 => self.m14_tick(i, ctx),
            120..=127 => self.m15_tick(i, ctx),
            128..=135 => self.m16_tick(i, ctx),
            136..=143 => self.m17_tick(i, ctx),
            144..=151 => self.m18_tick(i, ctx),
            152..=159 => self.m19_tick(i, ctx),
            160..=167 => self.m20_tick(i, ctx),
            168..=175 => self.m21_tick(i, ctx),
            176..=183 => self.m22_tick(i, ctx),
            184..=191 => self.m23_tick(i, ctx),
            192..=199 => self.m24_tick(i, ctx),
            200..=207 => self.m25_tick(i, ctx),
            208..=215 => self.m26_tick(i, ctx),
            216..=223 => self.m27_tick(i, ctx),
            224..=231 => self.m28_tick(i, ctx),
            // The m0/m3 child follow (sub_1B6B0, table 0xE8).
            232 => self.mc2_child_tick(i),
            // m27 branches / tier-2 segments: NULL table entries —
            // body-driven via sub_29A90, never self-dispatched.
            233 | 234 => {}
            _ => {
                self.note_misfit(5, self.ent[i].model65 as u16);
                self.ent[i].flags |= 0x400;
            }
        }
    }

    /// `sub_1E4D0` (EF:10650), StageVar2 == 12 — the METAMORPH creature:
    /// a cosmetic pose-PUPPET slaved to the caster every tick (position +
    /// facing copied). The engine never rebinds control — the wizard
    /// stays under normal control and keeps casting; the carpet is just
    /// hidden (player.metamorph) and this creature draws in its place.
    /// The human is out of the pool, so the parent pose comes from `ctx`
    /// (the live player pose), not a pooled parent. The per-model z
    /// offset (m16 −896, m25 −512, EF:10664-74) drops the creature's
    /// origin so its sprite aligns where the carpet was. Teardown rides
    /// the cast window (mc2_cast_expire). No autonomous combat.
    fn mc2_metamorph_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        let off: i16 = match self.ent[i].model65 {
            16 => 896,
            25 => 512,
            _ => 0,
        };
        let z = ctx.pz.saturating_sub(off);
        self.move_relink(i, ctx.px, ctx.py, z);
        self.ent[i].f30 = ctx.pyaw;
        self.ent[i].f34 = ctx.pyaw;
        // The creature's cry LOOPS while morphed — the player-confirmed
        // (2026-07-14) FP effect: no visible sprite from first person,
        // just the monster's scream on a loop (plus the distinct Morph
        // cast sound 60). Play the model's characteristic cry on a
        // ~24-tick loop, anchored at the creature (= the player pose).
        if self.ent[i].f26 <= 0 {
            let cry = match self.ent[i].model65 {
                16 => 39, // Wyvern
                25 => 37, // Cymmerian
                2 => 12,  // Day creature
                _ => 43,  // FireFly (19)
            };
            self.snd(cry, i);
            self.ent[i].f26 = 24;
        } else {
            self.ent[i].f26 -= 1;
        }
    }

    /// `sub_1E580` (EF:10689), StageVar2 == 13 — the SUMMON-ARMY allied
    /// creature: free-roam AI that hunts enemy wizards for the caster
    /// (no player input). Acquire the nearest enemy wizard (class 3,
    /// model ≤ 1, not our team); with none, follow the caster; face and
    /// move toward it via the creature move core; once in engage range,
    /// hand off to the model's normal `+2` attack state (the landed
    /// class-5 combat). Self-expires after its 250-tick life (`f26`) with
    /// a fire puff. The idle-follow + acquire resolve the caster to the
    /// out-of-pool human via `ctx` (docs/spell-audit/summon-creatures.md).
    fn mc2_summon_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        // Life countdown (word_0x2E_46 → f26): expire with a puff.
        self.ent[i].f26 -= 1;
        if self.ent[i].f26 <= 0 {
            let (x, y, z) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            self.mc2_spawn_fire(x, y, z);
            self.ent[i].flags |= 0x400;
            return;
        }
        let own = self.ent[i].id24;
        let (mx, my) = (self.ent[i].x, self.ent[i].y);
        // Re-acquire on the throttle (byte_0x3E_62 & 7) or when the lock
        // is stale — nearest ENEMY wizard by 2-D distance.
        let mut target = self.ent[i].f146;
        let valid = target != 0
            && target != crate::mc1::mobs::PLAYER_TARGET
            && (target as usize) < self.ent.len()
            && self.ent[target as usize].class64 == 3
            && self.ent[target as usize].model65 <= 1
            && self.ent[target as usize].flags & 0x400 == 0
            && self.ent[target as usize].act_life >= 0;
        if !valid && self.ent[i].f63 & 7 == 0 {
            target = 0;
            let mut best = i32::MAX;
            for j in 1..self.ent.len() {
                let e = &self.ent[j];
                if e.class64 != 3
                    || e.model65 > 1
                    || e.id24 == own
                    || e.flags & 0x400 != 0
                    || e.act_life < 0
                {
                    continue;
                }
                let d = Self::dist2_sq(mx, my, e.x, e.y);
                if d < best {
                    best = d;
                    target = j as u16;
                }
            }
            self.ent[i].f146 = target;
        }
        // Face + move toward the target, or follow the caster (the human,
        // resolved via ctx) when there is none.
        let (tx, ty) = if target != 0 && (target as usize) < self.ent.len() {
            (self.ent[target as usize].x, self.ent[target as usize].y)
        } else {
            (ctx.px, ctx.py)
        };
        let yaw = Self::angle_between(mx, my, tx, ty);
        self.ent[i].f34 = yaw;
        self.mc2_move_core(i);
        // In engage range → hand off to the model's `+2` attack state
        // (leaving the controlled slot: StageVar2 → 0).
        if target != 0 {
            let d = Self::isqrt(Self::dist2_sq(mx, my, tx, ty) as u32);
            if d < 1536 {
                self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
                self.ent[i].site_z = 0;
            }
        }
    }

    /// The MC2 class-9 dispatch — the TargetingVerb::Mc2 arm's
    /// projectile side. Only the (9,13) arrow is MC2-ported; every
    /// other flight state falls back to the MC1 projectile handler
    /// with a fallback note — the player's spells stay MC1 until the
    /// MC2 spell column lands (deliberate cross-column play, the
    /// seam's graceful-degradation contract).
    pub(crate) fn mc2_proj_tick(&mut self, i: usize, ctx: &MobCtx) {
        // MC2-native projectiles carry the F_MC2PROJ marker (their
        // ctors set it); MC1-fallback spawns never do, so state
        // numbers can't collide across the columns.
        if self.ent[i].flags & super::proj::F_MC2PROJ != 0 {
            // The creature-launched family all rides the shared
            // flyer core (sub_65820 ≡ states 2..8, 0x0B, 0x0E-0x1C;
            // state 0's CastPlayerFire delta is initial-aim only —
            // creature launches pre-aim, so the core serves). The
            // (9,3) meteor shot's action-3 wrapper adds the trailing
            // spark (sub_66180, mc2::proj).
            if self.ent[i].model65 == 3 && self.ent[i].tick70 == 3 {
                self.mc2_meteor_shot_tick(i, ctx);
            } else if self.ent[i].model65 == 9 && self.ent[i].tick70 == 9 {
                // Lightning L0 (subtype 9) = the `sub_66750` one-tick
                // hitscan BEAM, not a traveling ball. Resolve it whole
                // this tick (docs/spell-audit/lightning.md §5.A) so it
                // flashes to its impact and is gone — under RAPID
                // re-fire that reads as the authentic crackle, vs the
                // old slow-bolt "stream of projectiles".
                self.mc2_lightning_beam_tick(i, ctx);
            } else if self.ent[i].model65 == 9 && self.ent[i].tick70 == 14 {
                // The beam's cosmetic sprite-216 trail billboards
                // (`sub_67410`, action 14): inert, self-despawning.
                self.mc2_lightning_node_tick(i);
            } else {
                self.mc2_flyer_tick(i, ctx);
            }
            return;
        }
        match self.ent[i].tick70 {
            // Keyed on model AND state: MC1 flight states (the
            // fallback below) may also use the value 13.
            ARROW_STATE if self.ent[i].model65 == 13 => self.mc2_arrow_tick(i, ctx),
            0xFE => {} // authored inert parking (shared convention)
            _ => {
                self.note_verb_fallback(crate::verbs::VerbKind::Targeting);
                if self.proj_tick(i, ctx) {
                    self.terrain_dirty = true;
                }
            }
        }
    }

    /// The MC2 awake pre-pass (`sub_68BF0`/`sub_68C70`,
    /// :55469/:55494) — the AwakeVerb::Mc2 arm. Order per the
    /// transcript: an armed counter propagates to followers THEN
    /// decrements; a zero counter waits out the wake delay (f59),
    /// then the 2D proximity probe (same 0x2400000 as MC1) arms 16
    /// (followers 18). Dead entities reset to the 0xFA sentinel.
    pub(crate) fn mc2_awake_pass(&mut self, ctx: &MobCtx) {
        for i in 1..self.ent.len() {
            let e = &self.ent[i];
            if e.class64 != 5 || matches!(e.tick70, 0xB4 | 0xE8 | 0xEA) || e.flags & 0x400 != 0 {
                continue;
            }
            if e.act_life < 0 {
                self.ent[i].f58 = 0xFA;
                self.ent[i].f59 = 0;
                continue;
            }
            self.mc2_awake_one(i, ctx);
        }
        // sub_68BF0's SECOND loop (EF:55489-90): dword_38523 = the
        // mana-sphere family (10, 39/40) awake-ticks too — spheres
        // near the player arm their f58 like creatures do. No dead
        // reset here (retail's sphere loop is unconditional). E15.
        for i in 1..self.ent.len() {
            let e = &self.ent[i];
            if e.class64 == 10 && matches!(e.model65, 39 | 40) && e.flags & 0x400 == 0 {
                self.mc2_awake_one(i, ctx);
            }
        }
    }

    /// One entity's `sub_68C70` body (EF:55494): f58 propagate +
    /// decrement, the HIDDEN-skip, the f59 hold, proximity-wake.
    fn mc2_awake_one(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].f58 != 0 {
            let v = self.ent[i].f58;
            let mut j = self.ent[i].f54 as usize;
            while j != 0 {
                self.ent[j].f58 = v;
                j = self.ent[j].f54 as usize;
            }
            self.ent[i].f58 = v - 1;
            return;
        }
        // The hidden-skip (`byte[0] & 1`, EF:55515): a hidden entity
        // (burrowed m27 etc.) never proximity-wakes. Registry: flags
        // bit 0 = hidden, bit 5 (0x20) = scan-invisible — both are
        // verbatim byte[0] mappings, distinct from the synthesized
        // high bits (F_STOP &c). E15.
        if self.ent[i].flags & 1 != 0 {
            return;
        }
        if self.ent[i].f59 != 0 {
            self.ent[i].f59 -= 1;
            return;
        }
        let e = &self.ent[i];
        if Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) < 0x240_0000 {
            self.ent[i].f58 = 16;
            let mut j = self.ent[i].f54 as usize;
            while j != 0 {
                self.ent[j].f58 = 18;
                j = self.ent[j].f54 as usize;
            }
        }
        self.ent[i].f59 = 0;
    }
}
