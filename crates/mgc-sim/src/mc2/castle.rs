//! The MC2-NATIVE CASTLE COLUMN — class-3 model-2 and its court:
//! the THREE castle actionIndices (4 = standing tick, 5 = build
//! state machine, 6 = destroy-one-level), the MC2 HP/CAP ladder,
//! the straight-subtract intake, the sphere-absorb + overflow-eject
//! mana economy, the (3,3) balloon fleet, the (5,15) guard slots,
//! the (10,42) build painter and the (10,79) defender stage pieces.
//!
//! Traces: docs/traces/mc2-castle-builder.md +
//! mc2-castle-runtime.md + mc2-castle-open-items.md (the correction
//! pass) + mc2-castle-data-tables.md. Citations are `EF:line` into
//! the vendored remc2 `engine/EventsFunctions.cpp`.
//!
//! Structural key: MC1 keeps ONE castle action (5) with `f59`
//! sub-states; MC2 moves the phase to the actionIndex itself —
//! `tick70` 4/5/6 — and `f59` (retail `word_0x2E_46`) is the
//! within-action-5 build sub-state. The quake/whirlwind grab's
//! `f50 = 30` write (mc2::flood) is CONSUMED here as the settle
//! timer (EF:61057-61078): intake pauses while it runs — the same
//! "mailbox accrues during the shake" shape as MC1.
//!
//! Trace corrections banked during this port (verified against the
//! decompile, see ROADMAP): `word_0x80_128` is the UPGRADE-request
//! channel (written by the delivered castle cast `sub_389F0`
//! EF:28240 with the companion `word_0x7C_124 = 10` — the exact
//! MC1 ch5 `(10, owner)` token protocol, so the MC1 (10,43) token
//! serves both columns verbatim); `dword_38519` is the CLASS-3 live
//! list, so the flood grab DOES target castles (the flood port
//! stands as-is); `sub_60400` returns (balloons, guards) — the
//! same quota table as MC1's fleet dispatcher.
//!
//! APPROX register (all cited inline): the owner "colored" palette
//! shift (`word_0x5A_90 += TransformPlayerColorIndex`, EF:61139)
//! rides the renderer's team tint; `sub_5F890` (the Create-Castle
//! HUD spell-widget ghost sync, EF:61029) has no ported widget —
//! the calls are no-ops; `sub_6D8B0(owner, 2, 1)` +1 castle XP
//! (EF:61596) banks with Phase 4.2; the balloon/guard slot arrays
//! (`array_0x3C_60`/`array_0x5C_92`) are scan-collected like the
//! MC1 port (same membership, no per-slot indices); cave-level
//! balloon walking (`sub_60D50`) and the cave ceiling arm wait for
//! Phase 4.5; the (10,79) defender's target-scan + `sub_6DCA0`
//! spell launch (EF:30195-30284) bank with the 4.2 cast machinery —
//! pieces stand, dwell and ground-clamp, but do not fire yet.

use crate::mc1::features::{Gen, lcg32, tile};

/// `sub_60810` (EF:61695): capacity by level. Differs from MC1 at
/// every level >= 1; the level-7 sentinel is 300M (MC1: 30M).
pub(crate) const MC2_CASTLE_CAP: [i32; 8] =
    [5000, 8500, 18000, 38800, 78600, 158200, 317400, 300_000_000];

/// `sub_60810` (EF:61707-61728): max life by level, PRE-scale.
/// Level 0 = 0 (the ladder skips the life write — the footprint
/// keeps whatever it had). Scaled by the owner's Life personality
/// (`mc2_castle_life_factor`).
pub(crate) const MC2_CASTLE_HP: [u32; 8] = [0, 20000, 40000, 40000, 60000, 60000, 80000, 80000];

/// `byte[0] |= 0x40` (EF:61756): the "upgrade armed" latch the
/// standing tick converts into action 5 state 0. Bit 6 is unused on
/// class-3 entities in the MC1 column (the 0x40 ball-tether bit is
/// a class-10 home).
pub(crate) const F_UPGRADE_ARMED: u32 = 0x40;

/// `sub_60400` (EF:61523): (balloons, guards) by castle level —
/// byte-identical to MC1's fleet quota (sub_47400 :56264).
const fn mc2_castle_quota(lvl: i16) -> (usize, usize) {
    match lvl {
        1 | 2 => (1, 0),
        3 => (1, 4),
        4 => (2, 6),
        5 => (2, 14),
        6 => (3, 18),
        7 => (3, 34),
        _ => (0, 0),
    }
}

impl Gen {
    /// The class-3 model-2 dispatch under the MC2 column: retail
    /// runs `tick70` through the class-3 action table (EF:1206-08).
    /// Anything else on a (3,2) is a load-time husk — stand still.
    pub(crate) fn mc2_castle_tick(&mut self, i: usize) {
        match self.ent[i].tick70 {
            4 => self.mc2_castle_standing(i),
            5 => self.mc2_castle_build(i),
            6 => self.mc2_castle_destroy(i),
            _ => {}
        }
    }

    /// `EndOfCastleProjectile_5F8F0` (EF:61055) — action 4, the
    /// STANDING castle tick.
    fn mc2_castle_standing(&mut self, i: usize) {
        // (A) settle/projectile animation running (f50 = retail
        // word_0x30_48): armed 30 by the flood/quake grab
        // (mc2::flood), 5 by the destroy handler. Holds at 1 while
        // the grab bit is still set — the flood releases it.
        if self.ent[i].f50 != 0 {
            if self.ent[i].f50 == 1 {
                if self.ent[i].flags & super::flood::F_QUAKE_GRAB == 0 {
                    self.ent[i].tick70 = 5;
                    self.ent[i].f59 = 3; // → the repaint-painter arm
                    self.ent[i].f50 = 0;
                }
            } else {
                self.ent[i].f50 -= 1;
                // sub_5F890(a1x, 1): HUD build-ghost sync (APPROX
                // no-op — no ported widget).
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                self.ent[i].z = self.ground_z(x, y) as i16;
            }
            return;
        }
        // (B) normal standing tick.
        match self.mc2_castle_intake(i) {
            2 => {
                self.ent[i].tick70 = 6;
            }
            _ => {
                if self.ent[i].flags & F_UPGRADE_ARMED != 0 {
                    self.ent[i].flags &= !F_UPGRADE_ARMED;
                    self.ent[i].f59 = 0;
                    self.ent[i].tick70 = 5;
                }
            }
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        // playerEntityIndex = self.id every tick (EF:61092) — the
        // census claim key.
        self.ent[i].f144 = self.ent[i].id24;
        // Heavy work on even ticks only (EF:61094).
        if self.ent[i].f63 & 1 == 0 {
            self.mc2_castle_eject(i);
            let lvl = self.ent[i].f26;
            self.mc2_castle_extents(i, lvl.clamp(0, 7) as u8);
            self.mc2_castle_roster(i);
            self.mc2_castle_absorb(i);
        }
    }

    /// `BeginOfCastleCreation_5FA70` (EF:61123) — action 5, the
    /// build/repaint state machine on `f59` (retail word_0x2E_46).
    fn mc2_castle_build(&mut self, i: usize) {
        match self.ent[i].f59 {
            // ── pre-clear + level-up ──
            0 => {
                self.mc2_castle_preclear(i);
                if self.ent[i].f26 == 0 || self.mc2_castle_space_ok(i) {
                    // Owner palette shift (EF:61137-41): renderer
                    // team tint (APPROX).
                    self.mc2_castle_upgrade(i);
                } else {
                    self.ent[i].f59 = 2;
                    self.ent[i].flags &= !F_UPGRADE_ARMED;
                    // sub_88D00: "no room" hint toast (UI only).
                }
            }
            // ── ground settle waits ──
            1 | 6 => {
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                self.ent[i].z = self.ground_z(x, y) as i16;
            }
            // ── abort/pass-done → steady ──
            2 => {
                self.ent[i].tick70 = 4;
                // sub_5F890(a1x, 0): ghost reset (APPROX no-op).
                self.ent[i].f59 = 0;
            }
            // ── spawn a repaint painter ──
            3 => {
                self.mc2_spawn_castle_painter(i, true);
            }
            // ── wait for the painter ──
            4 => {
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                self.ent[i].z = self.ground_z(x, y) as i16;
                if self.ent[i].f63 & 0x1F == 0 {
                    // Any (10,42) still alive? (EF:61149-61158 —
                    // the painter signals f59=2 itself when it
                    // finishes; this poll only catches a painter
                    // that died without finalizing.)
                    let alive = (1..self.ent.len()).any(|j| {
                        self.ent[j].class64 == 10
                            && self.ent[j].model65 == 42
                            && self.ent[j].flags & 0x400 == 0
                    });
                    if !alive {
                        self.ent[i].f59 = 3;
                    }
                }
            }
            // ── the (10,41) leveler arm (EF:61162-67): dead code
            // at runtime — nothing in MC2 ever writes state 5
            // (verified by a full write-site sweep) ──
            _ => {}
        }
    }

    /// `sub_5FCA0_destroy_castle_level` (EF:61222) — action 6:
    /// gated on free pool slots (retail sub_4A810: "spheres can
    /// spawn"), one level off + ejector + roster, then a 5-tick
    /// settle into the repaint. No slots → retry from action 4.
    fn mc2_castle_destroy(&mut self, i: usize) {
        if !self.free.is_empty() {
            self.mc2_castle_downgrade(i);
            self.ent[i].tick70 = 4;
            if self.ent[i].flags & 0x400 != 0 {
                return; // level 0 died inside the downgrade
            }
            self.mc2_castle_eject(i);
            self.mc2_castle_roster(i);
            self.ent[i].f59 = 0;
            self.ent[i].f50 = 5;
        } else {
            self.ent[i].tick70 = 4;
        }
    }

    /// `sub_609E0` (EF:61733) — the damage intake: STRAIGHT subtract
    /// (no /10, no shield), single mail channel; the self-keyed
    /// upgrade-request channel arms bit6. Returns 0 idle / 1 hit /
    /// 2 lethal (already dead counts).
    fn mc2_castle_intake(&mut self, i: usize) -> u8 {
        if self.ent[i].act_life < 0 {
            return 2;
        }
        let mut result = 0;
        if self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].act_life -= amt as i32;
            if self.ent[i].act_life < 0 {
                self.ent[i].f36 = src; // killer memory (word_0x24_36)
                self.ent[i].mail[0].1 = 0;
                return 2;
            }
            self.ent[i].mail[0] = (0, 0);
            result = 1;
            // Owner "castle under attack" HUD flag (byte_0x195_405
            // = 4) — ours is the player-side alert latch.
            if self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                self.castle_alert = 4;
            }
        }
        // word_0x80_128 == own id (EF:61753): the UPGRADE request —
        // the delivered (10,43) token writes our mail[5] = (10,
        // owner), the same protocol both columns share (sub_389F0
        // EF:28240 writes word_0x7C_124 = 10 + word_0x80_128 = id).
        if self.ent[i].mail[5].1 != 0 {
            let sender = self.ent[i].mail[5].1;
            self.ent[i].mail[5] = (0, 0);
            if sender == self.ent[i].id24 && self.ent[i].f26 < 7 {
                self.ent[i].flags |= F_UPGRADE_ARMED;
            }
        }
        result
    }

    /// `sub_60480` (EF:61563) — the LEVEL-UP: painter spawn, sound
    /// 10, level++, back to wait-for-painter, extents, ladder,
    /// stage-piece rebuild. (+1 castle XP `sub_6D8B0(owner,2,1)`
    /// EF:61596 banks with 4.2.)
    fn mc2_castle_upgrade(&mut self, i: usize) {
        let lvl = (self.ent[i].f26 + 1).clamp(1, 7);
        // The painter first — retail aborts the whole level-up if
        // the pool is full (EF:61568).
        let Some(p) = self.mc2_spawn_castle_painter_at(i, lvl as u8, false) else {
            return;
        };
        self.snd(10, i);
        self.ent[i].f26 = lvl;
        self.ent[i].tick70 = 5;
        self.ent[i].f59 = 4; // wait-for-painter
        self.mc2_castle_extents(i, lvl as u8);
        self.mc2_castle_extents_ent(p, lvl as u8);
        self.mc2_castle_ladder(i);
        self.mc2_castle_stages(i);
    }

    /// `sub_605E0` (EF:61612) — ONE LEVEL DOWN: 10% capacity mana
    /// haircut (scattered), terrain restore for the removed level,
    /// ladder + stage rebuild; at level 0 the castle dies (owner
    /// unbind = the id24 link simply despawns with the entity).
    fn mc2_castle_downgrade(&mut self, i: usize) {
        if self.ent[i].f26 > 0 {
            // 10% capacity haircut. Widen to i64 for the multiply: a
            // castle over-filled past the normal cap ladder (the
            // level-0 mana-availability bug) can carry an f136 large
            // enough that `10 * f136` overflows i32 — player crash on
            // shift+L downgrade 2026-07-13. Same integer result as
            // retail's `10 * x / 100`.
            let cut = (10i64 * self.ent[i].f136 as i64 / 100) as i32;
            self.ent[i].f136 -= cut;
            self.mc2_castle_eject(i);
            self.ent[i].f136 += cut;
            self.snd(30, i);
            // The scratch-entity restore (EF:61632-61636): model 0
            // → datum-based heights, level 0 → no re-scatter.
            let lvl = self.ent[i].f26;
            self.mc2_castle_unstamp(i, lvl.clamp(1, 7) as u8);
            self.ent[i].f26 = lvl - 1;
            self.mc2_castle_extents(i, (lvl - 1).clamp(0, 7) as u8);
            self.mc2_castle_ladder(i);
            self.mc2_castle_stages(i);
        }
        if self.ent[i].f26 <= 0 {
            // Castle death (EF:61645-61665): free the pieces, drop
            // the balloons' castle (they dissolve in the next owner
            // pass — here: outright, like MC1's release), despawn.
            self.mc2_castle_free_stages(i);
            let own = self.ent[i].id24;
            for j in 1..self.ent.len() {
                if self.ent[j].class64 == 3
                    && self.ent[j].model65 == 3
                    && self.ent[j].id24 == own
                    && self.ent[j].flags & 0x400 == 0
                {
                    self.mc2_balloon_to_sphere(j);
                }
            }
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_60810` + `sub_60780` (EF:61695/61670) — the HP/CAP
    /// ladder. HP = base[lvl] * factor >> 8 where factor =
    /// (Life * ((research[lvl] << 8) + 256)) >> 8. CONFIRMED
    /// sources (mc2-castle-data-tables.md §2): Life = 256 default
    /// (the human ALWAYS — EF:43720; an AI wizard's comes from the
    /// map header's `WizardMapSettings.Life_0x3612F` via the rival
    /// spawn, EF:43768 — resolved per owner color below);
    /// research[lvl] = `array_0x24E_590[lvl]`, filled by the
    /// castle-research child from SPELLS.DAT (4.2) — zero today =
    /// identity, a fresh retail castle's exact state. Level 0
    /// skips the life write. A negative (overkill) life carries as
    /// debt capped at half the new max.
    pub(crate) fn mc2_castle_ladder(&mut self, i: usize) {
        let lvl = self.ent[i].f26.clamp(0, 7) as usize;
        // The owner's Life scalar × research 0 → Life/256 identity
        // for the human, the authored 16.8 factor for a rival.
        let own = self.ent[i].id24;
        let slot = self
            .rival_ents
            .iter()
            .position(|&e| e != 0 && e == own)
            .unwrap_or(0);
        let factor = self.mc2_life_scale.0[slot] as i64;
        let hp = ((MC2_CASTLE_HP[lvl] as i64 * factor) >> 8) as u32;
        if hp != 0 {
            let debt = if self.ent[i].act_life < 0 {
                (-self.ent[i].act_life).min(hp as i32 / 2)
            } else {
                0
            };
            self.ent[i].max_life = hp;
            self.ent[i].act_life = hp as i32 - debt;
        }
        self.ent[i].f136 = MC2_CASTLE_CAP[lvl];
    }

    /// `SetShiftByCastle_49EC0` (EF:32882): AABB half-extents from
    /// the BUILD00 row for the level — `((dim<<8)+1280)>>1`. The
    /// tick's follow-up yaw/fov writes land as the sprite fov home.
    pub(crate) fn mc2_castle_extents(&mut self, i: usize, lvl: u8) {
        self.mc2_castle_extents_ent(i, lvl);
    }

    fn mc2_castle_extents_ent(&mut self, i: usize, row: u8) {
        let Some(def) = self.assets.build_tab.get(row as usize).copied() else {
            return;
        };
        let e = &mut self.ent[i];
        e.f80 = (((def.w as u16) << 8).wrapping_add(1280)) >> 1;
        e.f82 = (((def.h as u16) << 8).wrapping_add(1280)) >> 1;
        e.f84 = 0x4000;
    }

    /// `sub_11960` (EF:4391) — the pre-clear: kill every EFFECT
    /// entity whose AABB overlaps the NEXT level's footprint
    /// (life = -1). Effects only — objects/terrain untouched. The
    /// retail effect list (`dword_38527`) is class-10 models
    /// 0x2D..=0x2D (buildings) — the flood's erase pass shares it;
    /// here the practical membership is the class-10 model-45
    /// building band (the MC1 column's pre-clear kills the same
    /// kind via its own list).
    fn mc2_castle_preclear(&mut self, i: usize) {
        let next = (self.ent[i].f26 + 1).clamp(1, 7) as usize;
        let Some(def) = self.assets.build_tab.get(next).copied() else {
            return;
        };
        let half_w = ((((def.w as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let half_h = ((((def.h as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if j != i
                && e.class64 == 10
                && e.model65 == 45
                && e.flags & 0x400 == 0
                // Inclusive compare — sub_11960's `<=` (the flood's
                // strict `<` is the other helper).
                && wd(e.x, x) <= e.f80 as i32 + half_w
                && wd(e.y, y) <= e.f82 as i32 + half_h
            {
                self.ent[j].act_life = -1;
                self.ent[j].f46 = 0; // fontTypeIndex = 0
            }
        }
    }

    /// `sub_11A10` (EF:4421) — the space check: (a) any class-10
    /// model-2 OBJECT overlapping the next-level box → no room;
    /// (b) scan the RING of newly-added border cells between the
    /// current and next footprints — a cell with `mapAngle` bit7
    /// (built/blocked), or on caves bit3 (SEALED), fails
    /// (`sub_11C80` EF:4543).
    pub(crate) fn mc2_castle_space_ok(&self, i: usize) -> bool {
        let cur = self.ent[i].f26.clamp(0, 7) as usize;
        let next = (self.ent[i].f26 + 1).clamp(1, 7) as usize;
        let (Some(dc), Some(dn)) = (
            self.assets.build_tab.get(cur).copied(),
            self.assets.build_tab.get(next).copied(),
        ) else {
            return true;
        };
        let half_w = ((((dn.w as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let half_h = ((((dn.h as u16) << 8).wrapping_add(1280)) >> 1) as i32;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if j != i
                && e.class64 == 10
                && e.model65 == 2
                && e.flags & 0x400 == 0
                && wd(e.x, x) < e.f80 as i32 + half_w
                && wd(e.y, y) < e.f82 as i32 + half_h
            {
                return false;
            }
        }
        // The ring scan: outer minus inner half-extents in tiles.
        let (iw, ih) = (
            ((((dc.w as u16) << 8).wrapping_add(1280)) >> 1) >> 8,
            ((((dc.h as u16) << 8).wrapping_add(1280)) >> 1) >> 8,
        );
        let (ow, oh) = ((half_w >> 8) as u16, (half_h >> 8) as u16);
        let ox = (x.wrapping_add(128) >> 8).wrapping_sub(ow) as u8;
        let oy = (y.wrapping_add(128) >> 8).wrapping_sub(oh) as u8;
        let (mx, my) = (ow.saturating_sub(iw) as u8, oh.saturating_sub(ih) as u8);
        let blocked = |gx: u8, gy: u8| {
            let a = self.t.angle[tile(gx, gy)];
            a & 0x80 != 0 || (self.is_cave() && a & 8 != 0)
        };
        // Top + bottom bands (my rows of full 2*ow width) and the
        // left/right columns (mx wide over the inner rows).
        for row in 0..my {
            for col in 0..2 * ow as u8 {
                if blocked(ox.wrapping_add(col), oy.wrapping_add(row))
                    || blocked(
                        ox.wrapping_add(col),
                        oy.wrapping_add((2 * oh) as u8)
                            .wrapping_sub(my)
                            .wrapping_add(row),
                    )
                {
                    return false;
                }
            }
        }
        for row in 0..(2 * ih) as u8 {
            for col in 0..mx {
                if blocked(ox.wrapping_add(col), oy.wrapping_add(my).wrapping_add(row))
                    || blocked(
                        ox.wrapping_add((2 * ow) as u8)
                            .wrapping_sub(mx)
                            .wrapping_add(col),
                        oy.wrapping_add(my).wrapping_add(row),
                    )
                {
                    return false;
                }
            }
        }
        true
    }

    /// `sub_5FD00` (EF:61240) — the overflow EJECTOR: spill = stored
    /// − capacity when (owner bank + stored) exceeds capacity (the
    /// "13C law" — the trigger reads the bank, the amount doesn't);
    /// a level-0 castle spills EVERYTHING. 1..=32 owner-tagged
    /// (10,39) spheres of spill/count each, teleported out at
    /// random yaws (dist rand%0x1400 + 3840, speed rand%0x30 + 16,
    /// the upward pop from the flag height).
    fn mc2_castle_eject(&mut self, i: usize) {
        let stored = self.ent[i].f140;
        let cap = self.ent[i].f136;
        let own = self.ent[i].id24;
        let bank = self.mc2_owner_bank(own);
        let mut spill = if bank.saturating_add(stored) > cap {
            stored - cap
        } else {
            0
        };
        if self.ent[i].f26 == 0 {
            spill = stored;
        }
        if spill <= 0 {
            return;
        }
        let count = (spill / 1000).clamp(1, 32);
        let mut share = spill / count;
        let (cx, cy, cz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let ground = self.ground_z(cx, cy) as i16;
        for _ in 0..count {
            let Some(b) = self.spawn_mana_ball(cx, cy, cz) else {
                break;
            };
            self.ent[b].f140 = share;
            self.ent[b].f144 = own;
            let d = self.ent_rand(b);
            self.ent[b].f126 = (d % 0x30 + 16) as i16;
            self.ent[b].dest_x = 0;
            self.ent[b].dest_y = 0;
            // word_0x2C_44 vertical arc (EF:61286) — our ball pop
            // home is f46 (the MC1 column's shared machinery).
            self.ent[b].f46 = ((1024 - (cz.wrapping_sub(ground)) as i32) / 8) as i16;
            let dist = (lcg32(&mut self.ent[i].rand) % 0x1400 + 3840) as i16;
            let yaw = (lcg32(&mut self.ent[i].rand) & 0x7FF) as u16;
            let mut pos = (cx, cy, cz);
            Self::polar_step(&mut pos, yaw, 0, dist);
            self.move_relink(b, pos.0, pos.1, pos.2);
            self.ball_resize(b);
            let taken = self.ent[b].f140;
            spill -= taken;
            self.ent[i].f140 -= taken;
            if spill < share {
                share = spill;
            }
            if spill <= 0 {
                break;
            }
        }
    }

    /// The owner's possessed-building bank — retail's per-tick
    /// census credit `dword_0x13C_316` (`sub_60F00` EF:62028): the
    /// summed mana of owned class-10 model-45 buildings.
    pub(crate) fn mc2_owner_bank(&self, own: u16) -> i32 {
        let mut bank = 0i64;
        for e in &self.ent[1..] {
            if e.class64 == 10 && e.model65 == 45 && e.flags & 0x400 == 0 && e.f144 == own {
                bank += e.f140.max(0) as i64;
            }
        }
        bank.min(i32::MAX as i64) as i32
    }

    /// The standing tick's sphere absorption (EF:61101-61116): ONE
    /// owned (10,39) sphere overlapping the castle per (even) tick,
    /// iff below capacity — the whole sphere lands.
    fn mc2_castle_absorb(&mut self, i: usize) {
        if self.ent[i].f140 >= self.ent[i].f136 {
            return;
        }
        let own = self.ent[i].id24;
        for j in 1..self.ent.len() {
            if self.ent[j].class64 == 10
                && self.ent[j].model65 == 39
                && self.ent[j].flags & 0x400 == 0
                && self.ent[j].f144 == own
                && self.mc2_overlap_xy(i, j)
            {
                self.ent[i].f140 += self.ent[j].f140;
                self.ent[j].flags |= 0x400;
                return; // one per tick (retail breaks after the first)
            }
        }
    }

    // ---- the court: balloons + guards (sub_5FF50, EF:61342) -----------------

    /// `sub_5FF50` (EF:61342): the balloon fleet + guard slots.
    /// Slot arrays scan-collected (module-doc APPROX); dead members
    /// dissolve into mana spheres carrying their cargo
    /// (`TransformEntityToManaSphere`), over-quota members too (a
    /// downgraded castle sheds fleet). Guard respawn: one per pass,
    /// 16-tick cooldown (f44 — retail word_0x2C_44), placed in the
    /// courtyard at (x+128, y+640) facing 512.
    fn mc2_castle_roster(&mut self, i: usize) {
        let own = self.ent[i].id24;
        let lvl = self.ent[i].f26;
        let (bq, gq) = mc2_castle_quota(lvl);
        let mut balloons: Vec<usize> = Vec::new();
        let mut guards = 0usize;
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if e.flags & 0x400 != 0 {
                continue;
            }
            match (e.class64, e.model65) {
                (3, 3) if e.id24 == own => balloons.push(j),
                (5, 15) if e.id24 == own => guards += 1,
                _ => {}
            }
        }
        // Dead + over-quota balloons → mana spheres (EF:61397-402 /
        // EF:61437-45).
        let mut alive: Vec<usize> = Vec::new();
        for &b in &balloons {
            if self.ent[b].act_life < 0 || alive.len() >= bq {
                self.mc2_balloon_to_sphere(b);
            } else {
                alive.push(b);
            }
        }
        let (cx, cy, cz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        // Shortfall spawn (EF:61382-90): one per empty slot.
        while alive.len() < bq {
            let Some(b) = self.mc2_spawn_balloon(cx, cy, cz, own) else {
                break;
            };
            alive.push(b);
        }
        // Retarget (EF:61403-31): default = come home; a sphere
        // override only on the fleet-staggered tick, with cargo
        // room, skipping the siblings' claims.
        let bank = self.mc2_owner_bank(own);
        let full = bank.saturating_add(self.ent[i].f140.max(0)) >= self.ent[i].f136;
        let stagger = !alive.is_empty() && self.ent[i].f63 as usize % alive.len() == 0;
        for k in 0..alive.len() {
            let b = alive[k];
            if full {
                self.ent[b].f146 = i as u16;
                continue;
            }
            if !stagger || self.ent[b].tick70 != 9 {
                continue;
            }
            self.ent[b].f146 = i as u16; // the castle default
            if self.ent[b].f140 >= self.ent[b].f136 {
                continue; // cargo full → home
            }
            // sub_5F810 (EF:60994): nearest own unclaimed sphere no
            // sibling is on.
            let (bx, by) = (self.ent[b].x, self.ent[b].y);
            let mut best = 0usize;
            let mut best_d = i32::MAX;
            for j in 1..self.ent.len() {
                let e = &self.ent[j];
                if e.class64 != 10 || e.model65 != 39 || e.flags & 0x400 != 0 || e.f144 != own {
                    continue;
                }
                if alive
                    .iter()
                    .any(|&s| s != b && self.ent[s].f146 as usize == j)
                {
                    continue;
                }
                let d = Self::dist2_sq(bx, by, e.x, e.y);
                if d < best_d {
                    best_d = d;
                    best = j;
                }
            }
            if best != 0 {
                self.ent[b].f146 = best as u16;
            }
        }
        // Guard slots (EF:61446-61510): cooldown, then one (5,15)
        // per pass into the courtyard.
        if self.ent[i].f44 > 0 {
            self.ent[i].f44 -= 1;
        }
        if guards < gq && self.ent[i].f44 == 0 {
            let gx = cx.wrapping_add(128);
            let gy = cy.wrapping_add(640);
            let gz = self.ground_z(gx, gy) as i16;
            if let Some(g) = self.mc2_spawn_m15(gx, gy, gz) {
                self.ent[g].id24 = own;
                self.ent[g].f144 = own;
                self.ent[g].f30 = 512;
                self.ent[g].f34 = 512;
                self.ent[i].f44 = 16;
            }
        }
    }

    /// `TransformEntityToManaSphere_36BA0` on a balloon: the cargo
    /// (plus nothing else — the balloon body itself carries no
    /// bounty) drops as one owned sphere; the balloon despawns.
    fn mc2_balloon_to_sphere(&mut self, b: usize) {
        let cargo = self.ent[b].f140;
        if cargo > 0 {
            let (x, y, z, own) = {
                let e = &self.ent[b];
                (e.x, e.y, e.z, e.id24)
            };
            if let Some(s) = self.spawn_mana_ball(x, y, z) {
                self.ent[s].f140 = cargo;
                self.ent[s].f144 = own;
                self.ball_resize(s);
            }
        }
        self.ent[b].flags |= 0x400;
    }

    /// `sub_4ABA0` (EF:33409) — the MC2 (3,3) balloon ctor: life
    /// 10000, speed 48, cargo cap 10000, ch0 intake, behavior row
    /// 68 (= ROW_BASE + 9, the same servo family as MC1's row 9),
    /// sprite 169 (+ team). The ctor's action 7 is overwritten to
    /// the working 9 by the roster (EF:61391) — spawned here as 9
    /// directly.
    fn mc2_spawn_balloon(&mut self, x: u16, y: u16, z: i16, own: u16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 3;
            e.model65 = 3;
            e.tick70 = 9;
            e.max_life = 10000;
            e.act_life = 10000;
            e.f126 = 48;
            e.f136 = 10000;
            e.f140 = 0;
            e.f28 = 1; // byte_0x38_56 = 1: ch0 vulnerable
            e.row156 = 9; // behavior row (MC2 abs 68 = base + 9)
            e.id24 = own;
            e.f144 = own;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        let team = self.owner_team(own).unwrap_or(0) as u16;
        self.mc2_set_sprite(i, 169 + team);
        if self.is_cave() {
            // The cave placement box override (EF:33426-27,
            // SetEntityShiftRot(256, 768)).
            self.mc2_shift_rot(i, 256, 768);
        }
        Some(i)
    }

    /// `AddBallon_60AB0` (EF:61763) — the MC2 balloon tick: fly at
    /// the target (f146); a class-10 sphere target is tethered
    /// within 1024 (2048 on caves, EF:61793-96 — cave castles
    /// vacuum spheres from twice as far), absorbed on overlap (cargo +
    /// owner claim + full heal); a class-3 castle target delivers
    /// the whole cargo inside the level×speed ring below the servo
    /// altitude. `sub_60EA0` intake at the tail: straight subtract,
    /// owner balloon-alert, killer memory — the corpse is the
    /// roster pass's business (no despawn here).
    pub(crate) fn mc2_balloon_tick(&mut self, i: usize) {
        use super::behavior::{BEHAVIOR, ROW_BASE};
        let t = self.ent[i].f146 as usize;
        let row = &BEHAVIOR[ROW_BASE + self.ent[i].row156 as usize];
        if t != 0 && self.ent[t].flags & 0x400 == 0 {
            let mut pos = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            let (tx, ty) = (self.ent[t].x, self.ent[t].y);
            let yaw = Self::angle_between(pos.0, pos.1, tx, ty);
            self.ent[i].f30 = yaw;
            let speed = self.ent[i].f126;
            let mut step = true;
            if self.ent[t].class64 == 10 {
                if self.ent[t].f144 != self.ent[i].id24 {
                    step = false; // not ours (EF:61791)
                } else {
                    let d = Self::isqrt(Self::dist2_sq(pos.0, pos.1, tx, ty) as u32) as i32;
                    let tether = if self.is_cave() { 2048 } else { 1024 };
                    if d > tether {
                        self.ent[t].flags &= !0x40; // release tether
                    } else {
                        self.ent[t].flags |= 0x40;
                        self.ent[t].f146 = i as u16;
                        if self.ent_overlap(i, t) {
                            let cargo = self.ent[t].f140;
                            let claim = self.ent[t].f144;
                            self.ent[i].f140 += cargo;
                            self.ent[i].f144 = claim;
                            self.ent[i].f146 = 0;
                            self.ent[i].act_life = self.ent[i].max_life as i32;
                            self.ent[t].flags |= 0x400;
                        }
                    }
                    if d <= speed as i32 {
                        pos.0 = tx;
                        pos.1 = ty;
                        step = false;
                    }
                }
            } else if self.ent[t].class64 == 3 {
                // Castle delivery ring = level * speed (EF:61828).
                let d = Self::isqrt(Self::dist2_sq(pos.0, pos.1, tx, ty) as u32) as i32;
                if d <= self.ent[t].f26 as i32 * speed as i32 {
                    let ground = self.ground_z(pos.0, pos.1) as i16;
                    if pos.2 <= ground.wrapping_add(row.v_12) && self.ent[t].f26 > 0 {
                        pos.0 = tx;
                        pos.1 = ty;
                        let cargo = self.ent[i].f140;
                        self.ent[t].f140 += cargo;
                        self.ent[i].f140 = 0;
                        self.ent[i].f144 = self.ent[i].id24;
                        self.ent[i].act_life = self.ent[i].max_life as i32;
                    }
                    step = false;
                }
            }
            if step {
                Self::polar_step(&mut pos, yaw, self.ent[i].f32, speed);
            }
            // The servo (sub_580E0 with the row's v_10/v_12/v_14).
            let ground = self.ground_z(pos.0, pos.1) as i16;
            let mut z = pos.2;
            let r1 = crate::mc1::behavior::BehaviorRow {
                v_10: row.v_10,
                v_12: row.v_12,
                v_14: row.v_14,
                ..crate::mc1::behavior::BEHAVIOR[9]
            };
            if self.is_cave() {
                // The CEILING WALK (`sub_60D50` EF:61872, called from
                // the cave branch EF:61848-50): flags bit0 = "walking
                // on the ceiling" — attach when the tile is sealed or
                // the poke test fires, detach when open sky returns;
                // actSpeed 96 walking / 48 flying; sound 22 on each
                // transition behind a 32-tick cooldown (byte_0x46_70
                // → f71); then the same row servo, and a ceiling−fov
                // clamp while FLYING only.
                let t = crate::mc1::features::tile((pos.0 >> 8) as u8, (pos.1 >> 8) as u8);
                let roof = self.t.angle[t] & 8 != 0
                    || self.cave_poke(self.ent[i].f84 as i32, row.v_12 as i32, pos.0, pos.1);
                let walking = self.ent[i].flags & 1 != 0;
                let mut transition = false;
                if walking {
                    if !roof {
                        self.ent[i].flags &= !1;
                        transition = true;
                    }
                    self.ent[i].f126 = 96;
                } else {
                    if roof {
                        self.ent[i].flags |= 1;
                        transition = true;
                    }
                    self.ent[i].f126 = 48;
                }
                if self.ent[i].f71 != 0 {
                    self.ent[i].f71 -= 1;
                }
                if transition && self.ent[i].f71 == 0 {
                    self.snd(22, i);
                    self.ent[i].f71 = 32;
                }
                Self::alt_clamp(&mut z, ground, &r1);
                if self.ent[i].flags & 1 == 0 {
                    let c = (self.ceiling_z(pos.0, pos.1) as i16 as i32 - self.ent[i].f84 as i32)
                        as i16;
                    if z > c {
                        z = c;
                    }
                }
            } else {
                Self::alt_clamp(&mut z, ground, &r1);
            }
            self.move_relink(i, pos.0, pos.1, z);
        }
        // sub_60EA0 (EF:61939): the tail intake.
        if self.ent[i].act_life >= 0 && self.ent[i].mail[0].1 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            self.ent[i].act_life -= amt as i32;
            if self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                self.balloon_alert = 4;
            }
            if self.ent[i].act_life < 0 {
                self.ent[i].f36 = src;
            } else {
                self.ent[i].mail[0].1 = 0;
            }
        }
    }

    // ---- the (10,42) build painter -------------------------------------------

    /// `sub_5FBD0`/`sub_50370` (EF:61182/36733): spawn a (10,42)
    /// painter at the castle's build site. `repaint` = the state-3
    /// arm (generic ctor → f59 = 1 → long settle); the upgrade
    /// spawns with f59 = 0 (short settle).
    pub(crate) fn mc2_spawn_castle_painter(&mut self, castle: usize, repaint: bool) {
        let row = self.ent[castle].f26.clamp(1, 7) as u8;
        if self
            .mc2_spawn_castle_painter_at(castle, row, repaint)
            .is_some()
        {
            self.ent[castle].f59 = 4;
        }
    }

    fn mc2_spawn_castle_painter_at(
        &mut self,
        castle: usize,
        row: u8,
        repaint: bool,
    ) -> Option<usize> {
        let (x, y, site_z, own) = {
            let e = &self.ent[castle];
            (e.x, e.y, e.site_z, e.id24)
        };
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 42;
            e.tick70 = 0x2C; // action 44 → AddTerrainMod0A_2A_37BC0
            e.max_life = 0;
            e.f59 = u8::from(repaint); // byte_0x3B_59: settle window
            e.f71 = row;
            e.id24 = own;
            e.f40 = castle as u16; // parentId_0x28_40
        }
        self.link(i, x, y, site_z);
        self.mc2_castle_extents_ent(i, row);
        Some(i)
    }

    /// `AddTerrainMod0A_2A_37BC0` (EF:27648) — the painter tick:
    /// 19-tick progressive rise of the CUMULATIVE footprint (BUILD00
    /// rows 1..=level, each cell toward authored height + datum),
    /// sprite/texture paint on the 1st, every 7th and last tick,
    /// then the settle window (f59: 1 tick, or 25 on a repaint)
    /// which flips built cells' angle bit3 → bit7 (feeding the
    /// space check), signals the parent castle (f59 = 2) and
    /// despawns. Returns true when terrain changed.
    pub(crate) fn mc2_castle_painter_tick(&mut self, i: usize) -> bool {
        // First tick: seed the countdown (byte[0] bit1 latch).
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.ent[i].f26 = 19;
        }
        let parent = self.ent[i].f40 as usize;
        let row = (self.ent[i].f71 as usize).min(7);
        let Some(def) = self.assets.build_tab.get(row).copied() else {
            self.ent[i].flags |= 0x400;
            return false;
        };
        // The working frame = the level row's footprint, widened to
        // the largest accumulated row (only bites at stage 7, whose
        // BUILD00 row is a degenerate 1x1 — retail's scratch there
        // writes OUTSIDE its buffer, a genuine memory stomp we
        // cannot reproduce; the widened frame repaints sanely).
        let (mut w, mut h) = (def.w as usize, def.h as usize);
        for r in 1..=row {
            if let Some(rd) = self.assets.build_tab.get(r) {
                w = w.max(rd.w as usize);
                h = h.max(rd.h as usize);
            }
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let tlx = cx.wrapping_sub((w / 2) as u8);
        let tly = cy.wrapping_sub((h / 2) as u8);

        if self.ent[i].f26 <= 0 {
            // ── phase B: settle, then finalize ──
            self.ent[i].f26 += 1;
            if self.ent[i].f26 == 0 {
                // bit3 → bit7 over the footprint (EF:27737-45).
                for dy in 0..h {
                    for dx in 0..w {
                        let t = tile(tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                        if self.t.angle[t] & 8 != 0 {
                            self.t.angle[t] = (self.t.angle[t] & 0xF7) | 0x80;
                        }
                    }
                }
                if parent != 0 && self.ent[parent].flags & 0x400 == 0 {
                    self.ent[parent].f59 = 2; // pass done
                }
                self.ent[i].flags |= 0x400;
            }
            return false;
        }
        // ── phase A: the progressive rise ──
        self.ent[i].f26 -= 1;
        if self.ent[i].f26 == 0 {
            self.ent[i].f26 = if self.ent[i].f59 != 0 { -25 } else { -1 };
            return false;
        }
        // Painting pauses while the castle runs its settle
        // animation (EF:27767).
        if parent != 0 && self.ent[parent].f50 != 0 {
            return false;
        }
        let countdown = self.ent[i].f26 as i32;
        let datum = (self.ent[i].z >> 5) as i32;
        // (1) accumulate per-cell targets over rows 1..=row, mapped
        // into the frame (retail writes a shared scratch keyed by
        // map cell — same cells).
        let mut delta = vec![0i32; w * h];
        let mut paint: Vec<(u8, u8, u8)> = Vec::new();
        let do_paint = countdown % 7 == 0 || countdown == 1;
        for r in 1..=row {
            let Some(rd) = self.assets.build_tab.get(r).copied() else {
                continue;
            };
            let (rw, rh) = (rd.w as usize, rd.h as usize);
            let start = rd.offset as usize;
            let Some(cells) = self.assets.build_dat.get(start..start + 2 * rw * rh) else {
                continue;
            };
            let cells = cells.to_vec();
            // Retail's per-row origin is center - (dim >> 1), i.e.
            // the frame offset is D/2 - d/2 — NOT (D - d)/2, which
            // loses a tile whenever D is even and d odd (EF:27798:
            // v33 = (v36>>1) - v8). That one tile was the playtest
            // "offset walkways / squashed tower / archers dying in
            // the wall" report on the 48x48 stage: every interior
            // ring sat one tile toward -x/-y of the outer ring.
            let offx = w / 2 - rw / 2;
            let offy = h / 2 - rh / 2;
            for dy in 0..rh {
                for dx in 0..rw {
                    let c = &cells[2 * (dy * rw + dx)..2 * (dy * rw + dx) + 2];
                    let gx = tlx.wrapping_add((offx + dx) as u8);
                    let gy = tly.wrapping_add((offy + dy) as u8);
                    if c[1] != 0xff {
                        let t = tile(gx, gy);
                        delta[(offy + dy) * w + offx + dx] =
                            c[1] as i32 + datum - self.t.height[t] as i32;
                    }
                    if do_paint && c[0] != 0xff {
                        paint.push((gx, gy, c[0]));
                    }
                }
            }
        }
        // (2) apply 1/countdown of each delta (EF:27846-70).
        for dy in 0..h {
            for dx in 0..w {
                let d = delta[dy * w + dx];
                if d == 0 {
                    continue;
                }
                let (gx, gy) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                let t = tile(gx, gy);
                if self.t.height[t] == 0 || super::flood::burn_flags(self.t.tile_type[t]) {
                    self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                    self.mc2_add_building_region(gx, gy, gx, gy);
                }
                self.t.height[t] = (self.t.height[t] as i32 + d / countdown) as u8;
                if countdown == 1 && self.t.angle[t] & 0x80 != 0 {
                    // Last rise tick: clear bit7, set bit3 — phase B
                    // re-promotes it (EF:27875-83).
                    self.t.angle[t] = (self.t.angle[t] & 0x7F) | 8;
                }
            }
        }
        for (gx, gy, code) in paint {
            // sub_45DC0(7, ...) — the groove-castle path's fixed
            // column counter (EF:27832).
            self.mc2_paint_cell(7, gx, gy, code);
        }
        true
    }

    // ---- the downgrade terrain restore ---------------------------------------

    /// `RemoveCastleStage_385C0` (EF:28071), the scratch-entity
    /// (model 0) arm the downgrade drives: un-stamp one BUILD00
    /// footprint — per active cell reset the angle nibble, the 2x2
    /// rubble stamp, drop the pad height back with the verbatim
    /// jitter RNG (datum-based zKoef, every 8th cell 10 lower is
    /// the sphere-drop height only — no spheres here: the scratch
    /// runs with level 0, the 10% haircut already scattered), then
    /// one retile over the footprint.
    fn mc2_castle_unstamp(&mut self, i: usize, row: u8) {
        self.terrain_dirty = true;
        let Some(def) = self.assets.build_tab.get(row as usize).copied() else {
            return;
        };
        let (w, h) = (def.w as usize, def.h as usize);
        let start = def.offset as usize;
        let Some(cells) = self
            .assets
            .build_dat
            .get(start..start + 2 * w * h)
            .map(<[u8]>::to_vec)
        else {
            return;
        };
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let tlx = ((ex.wrapping_add(128) >> 8) as u8).wrapping_sub((w / 2) as u8);
        let tly = ((ey.wrapping_add(128) >> 8) as u8).wrapping_sub((h / 2) as u8);
        for dy in 0..h {
            for dx in 0..w {
                let c = &cells[2 * (dy * w + dx)..2 * (dy * w + dx) + 2];
                if c[0] == 0xff && c[1] == 0xff {
                    continue;
                }
                let (gx, gy) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                let t = tile(gx, gy);
                self.t.angle[t] = (self.t.angle[t] & 0x70) | 1;
                self.mc2_add_building_region(gx, gy, gx, gy);
                if c[1] != 0xff {
                    let cur = self.t.height[t];
                    if c[1] >= cur {
                        self.t.height[t] = 0;
                    } else {
                        let d = self.ent_rand(i);
                        if d % 0x32 <= 20 {
                            self.t.height[t] = cur.wrapping_sub(c[1]);
                        } else {
                            let d2 = self.ent_rand(i);
                            self.t.height[t] =
                                cur.wrapping_sub(c[1].wrapping_sub((d2 % 0x14) as u8));
                        }
                    }
                }
            }
        }
        self.mc2_retile_region(
            tlx.wrapping_sub(1),
            tly.wrapping_sub(1),
            tlx.wrapping_add(w as u8),
            tly.wrapping_add(h as u8),
        );
    }

    // ---- the (10,79) stage pieces --------------------------------------------

    /// Free the castle's (10,79) piece set (identified by the
    /// back-link f146 = castle slot — the retail word_0x32_50 /
    /// word_0x34_52 chain, scan-collected).
    fn mc2_castle_free_stages(&mut self, i: usize) {
        for j in 1..self.ent.len() {
            if self.ent[j].class64 == 10
                && self.ent[j].model65 == 79
                && self.ent[j].f146 as usize == i
                && self.ent[j].flags & 0x400 == 0
            {
                self.ent[j].flags |= 0x400;
            }
        }
        self.ent[i].f52 = 0;
    }

    /// `sub_613D0` (EF:62233): rebuild the visible (10,79) piece
    /// set for the current level — free the old chain, then walk
    /// DOWN from the castle level to the highest RESEARCHED stage
    /// (`array_0x24E_590[9+lvl]` nonzero, EF:62271-77) and spawn
    /// one piece per [`MC2_STAGE_PARTS`] offset at that stage's
    /// footprint, z = ground + 384 (level <= 1) / 224 (EF:62315).
    /// Research is empty pre-4.2 (`mc2_castle_part_type`), so
    /// castles stand piece-less exactly like a retail castle whose
    /// research entities haven't completed — the painted terrain
    /// carries the shape.
    pub(crate) fn mc2_castle_stages(&mut self, i: usize) {
        self.mc2_castle_free_stages(i);
        let lvl = self.ent[i].f26;
        let own = self.ent[i].id24;
        if own == 0 || lvl <= 0 {
            return;
        }
        // The walk-down: the highest stage <= level with a
        // researched part-type (EF:62271-77).
        let mut stage = lvl.clamp(1, 7) as u8;
        let mut part = 0u8;
        while stage > 0 {
            part = self.mc2_castle_part_type(own, stage);
            if part != 0 {
                break;
            }
            stage -= 1;
        }
        if stage == 0 {
            return;
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let Some(def) = self.assets.build_tab.get(stage as usize).copied() else {
            return;
        };
        let tlx = cx.wrapping_sub(def.w / 2);
        let tly = cy.wrapping_sub(def.h / 2);
        for &(ox, oy) in mc2_stage_parts(stage) {
            let px = (
                (tlx.wrapping_add(ox) as u16) << 8,
                (tly.wrapping_add(oy) as u16) << 8,
            );
            let Some(p) = self.mc2_spawn_castle_piece(px.0, px.1, own, stage, part) else {
                break;
            };
            self.ent[p].f146 = i as u16; // back-link (word_0x32_50)
            self.ent[i].f52 = p as u16; // chain root (word_0x34_52)
        }
    }

    /// `array_0x24E_590[9 + stage]` — the researched PART-TYPE for
    /// a stage (EF:62274). Filled one stage at a time by the castle
    /// research/production child (`sub_69AB0` EF:56120-21, sourcing
    /// `SPELLS[model].subspell[row].life_0x1A`) — the 4.2 cast/XP
    /// machinery. Until that lands every stage is unresearched (0):
    /// no pieces, HP factor identity — a fresh retail castle's
    /// exact state.
    fn mc2_castle_part_type(&self, _own: u16, _stage: u8) -> u8 {
        0
    }

    /// `sub_508E0_castle_defend_create` (EF:36987): the (10,79)
    /// piece ctor — action 0x56, maxLife 100000, sprite 66,
    /// fontType 1. The level tag (word_0x4A_74) rides f26; the
    /// researched part-type (byte_0x43_67, EF:62310) rides f67.
    fn mc2_spawn_castle_piece(
        &mut self,
        x: u16,
        y: u16,
        own: u16,
        lvl: u8,
        part: u8,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 79;
            e.tick70 = 0x56;
            e.max_life = 100_000;
            e.id24 = own;
            e.f26 = lvl as i16; // level tag → the height offset
            e.f67 = part; // byte_0x43_67: the defender kind roll's key
            e.f71 = 0; // byte_0x46_70: the defender state machine
        }
        let z = self.ground_z(x, y) as i16 + if lvl <= 1 { 384 } else { 224 };
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 66);
        Some(i)
    }

    /// `sub_3AF00_castle_defend_event` (EF:30106) — the (10,79)
    /// piece tick, the DWELL arms: latch home, seed a random 16..63
    /// dwell, count it down, then hold armed. The target-scan +
    /// `sub_6DCA0` defender launch (states 3..8) bank with the 4.2
    /// cast machinery — until then the piece stands (the visible
    /// castle walls/towers) and ground-clamps like retail's tail.
    /// Dead or ownerless → despawn (retail's first two gates).
    pub(crate) fn mc2_castle_piece_tick(&mut self, i: usize) {
        if self.ent[i].act_life < 0 || self.ent[i].id24 == 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        match self.ent[i].f71 {
            0 => {
                self.ent[i].f71 = 1;
            }
            1 => {
                let d = self.mc2_rand(i);
                self.ent[i].f44 = (d % 0x30 + 16) as u16;
                self.ent[i].f71 = 2;
            }
            2 => {
                self.ent[i].f44 = self.ent[i].f44.saturating_sub(1);
                if self.ent[i].f44 == 0 {
                    self.ent[i].f71 = 3;
                }
            }
            // 3..: armed — the launch machinery banks with 4.2.
            _ => {}
        }
        // The LABEL_74 tail: ride the ground at the level height.
        let (x, y, lvl) = {
            let e = &self.ent[i];
            (e.x, e.y, e.f26)
        };
        let z = self.ground_z(x, y) as i16 + if lvl <= 1 { 384 } else { 224 };
        self.ent[i].z = z;
    }
}

/// `x_BYTE_DB038` (EF:2594) — the (10,79) piece offsets per level,
/// decoded (mc2-castle-data-tables.md §1.3): count at `[2*lvl]`,
/// pair-slot index at `[1+2*lvl]`, pairs at `[18..]`. Tile offsets
/// from the footprint's NW corner. L2/3, L4/5 and L6/7 share lists;
/// level 7 keeps L6's 48x48 list against BUILD00's degenerate 1x1
/// row 7 (retail reads it unclamped — the L7 extent quirk is the
/// data doc's OPEN).
const MC2_STAGE_PARTS: [&[(u8, u8)]; 8] = [
    &[],
    &[(4, 4)],
    &[(3, 3), (17, 3), (3, 17), (17, 17)],
    &[(3, 3), (17, 3), (3, 17), (17, 17)],
    &[(3, 3), (31, 3), (3, 31), (31, 31)],
    &[(3, 3), (31, 3), (3, 31), (31, 31)],
    &[
        (3, 3),
        (24, 3),
        (45, 3),
        (3, 24),
        (45, 24),
        (3, 45),
        (24, 45),
        (45, 45),
    ],
    &[
        (3, 3),
        (24, 3),
        (45, 3),
        (3, 24),
        (45, 24),
        (3, 45),
        (24, 45),
        (45, 45),
    ],
];

fn mc2_stage_parts(lvl: u8) -> &'static [(u8, u8)] {
    MC2_STAGE_PARTS[(lvl as usize).min(7)]
}

#[cfg(test)]
mod tests {
    use crate::chassis::ChassisParams;
    use crate::mc1::features::{FeatureAssets, Gen, Planes};
    use crate::verbs::VerbSet;

    fn flat_gen() -> Gen {
        let planes = Planes {
            height: vec![100; 0x10000],
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

    /// Downgrading a castle whose capacity `f136` was pumped past the
    /// normal ladder (the level-0 over-level bug) must not overflow the
    /// 10% haircut `10 * f136`. Regression for the shift+L panic
    /// "attempt to multiply with overflow" (player-reported 2026-07-13).
    #[test]
    fn mc2_castle_downgrade_survives_oversized_capacity() {
        let mut g = flat_gen();
        let i = g.new_event().expect("castle slot");
        {
            let e = &mut g.ent[i];
            e.class64 = 3;
            e.model65 = 2;
            e.f26 = 7; // level 7
            e.f136 = i32::MAX; // capacity pumped past the ladder
            e.f140 = 1_000; // little stored mana → eject is a no-op
            e.id24 = 1;
            e.x = 100 << 8;
            e.y = 100 << 8;
            e.act_life = 1;
        }
        g.link(i, 100 << 8, 100 << 8, g.ground_z(100 << 8, 100 << 8) as i16);
        // Must not panic on `10 * i32::MAX`.
        g.mc2_castle_downgrade(i);
        assert_eq!(g.ent[i].f26, 6, "one level off, no overflow");
    }

    /// Decode the verbatim `x_BYTE_DB038` bytes (EF:2594) and prove
    /// [`super::MC2_STAGE_PARTS`] matches: count at [2L], pair-slot
    /// index at [1+2L], pairs base at byte 18.
    #[test]
    fn stage_parts_match_the_db038_decode() {
        const DB038: [u8; 52] = [
            0x00, 0x00, 0x01, 0x00, 0x04, 0x01, 0x04, 0x01, 0x04, 0x05, 0x04, 0x05, 0x08, 0x09,
            0x08, 0x09, 0x00, 0x00, 0x04, 0x04, 0x03, 0x03, 0x11, 0x03, 0x03, 0x11, 0x11, 0x11,
            0x03, 0x03, 0x1F, 0x03, 0x03, 0x1F, 0x1F, 0x1F, 0x03, 0x03, 0x18, 0x03, 0x2D, 0x03,
            0x03, 0x18, 0x2D, 0x18, 0x03, 0x2D, 0x18, 0x2D, 0x2D, 0x2D,
        ];
        for lvl in 0..8usize {
            let count = DB038[2 * lvl] as usize;
            let slot = DB038[1 + 2 * lvl] as usize;
            let decoded: Vec<(u8, u8)> = (0..count)
                .map(|p| (DB038[18 + 2 * (slot + p)], DB038[18 + 2 * (slot + p) + 1]))
                .collect();
            assert_eq!(
                super::MC2_STAGE_PARTS[lvl],
                decoded.as_slice(),
                "level {lvl} piece list"
            );
        }
    }
}
