//! MC2 (5,10) — THE DOOMSDAY PYRAMID, Phase 4.3. The campaign's
//! spell-of-extinction endgame device: a ground-clamped, unkillable
//! boss structure running a 16-state script that flattens terrain in
//! an expanding crater, summons creatures/projectiles, devours the
//! battlefield, and at climax kills everything and spawns the
//! (10,17) + (10,9) apocalypse spheres. Trace (all-helpers-closed,
//! port-ready): docs/traces/mc2-class5-m10-doomsday.md (`EF:` =
//! remc2 EventsFunctions.cpp).
//!
//! The tick lives on [`World`] (not `Gen`): the machine drives world
//! globals — the apocalypse latch `byte_0x36E03`
//! (`World::mc2_apocalypse` — the (10,9) dome's variant selector),
//! the doomsday-active flag `word_0x36548`, and the HUD doom meter
//! `x_BYTE_D9F50[0x87a]` (`World::mc2_doom_meter`, banked for the
//! 4.9 HUD track).
//!
//! Entity-field homes (creature column + this machine's own):
//! state `byte_0x46_70`→f71 · phase bitfield `subSpellIndex_0x2A_42`
//! →f44 · countdown `dword_0x10_16`→f26 · turn-rate `word_0x2C_44`
//! →f46 (f44 is taken by the bitfield — the trace confirms both are
//! live at once) · facing mode `byte_0x44_68`→f69 · summon selector
//! `byte_0x43_67`→f68 · repeat `word_0x24_36`→f38 · aim stride
//! `word_0x4A_74`→f50 · beam ramp `word_0x36546` (a retail global;
//! one pyramid per level)→f52 · target `word_0x96_150`→f146.
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - Sprites 343/344/345 auto-size their state timer to the
//!   animation length (`sub_221F0` EF:13661 via the sprite params
//!   and the frame table); the sim doesn't carry TMAPS frame counts,
//!   so the states' seeded timers (16/32) stand, a cadence-only
//!   deviation.
//! - `sub_5C800` palette flashes (case-7 beam flash 6) are
//!   presentation (docs/traces/mc2-class10-tail-helper-closure.md
//!   §4) — skipped like every flash before.
//! - The (9,3)/(9,26) projectile bursts (selector 9/8) LANDED
//!   2026-07-11 (mc2::proj meteor shot / whirlwind seed —
//!   docs/traces/mc2-class9-m3-m26.md): pre-locked at the avatar
//!   via mc2_arm_proj (retail self-acquires on tick 1 — the proj
//!   module's acquisition APPROX).
//! - The case-0xE global wipe writes byte[1]|=0x20 on every entity —
//!   an unmapped render-side bit (name-inferred); we apply the
//!   life/maxLife=140 reset and skip the bit.
//! - Retail's per-list scans (dword_38531 buckets) are pool
//!   slot-order scans — the mobs.rs list APPROX.
//! - The case-7 HURL-AWAY beam moves the human via the shared knock
//!   channel (`Gen::player_knock` — the kraken/buffet writer's home)
//!   rather than teleporting the pose with moveTest + floor clamp
//!   (the app owns the pose; same observable: violent outward
//!   displacement, 944 units on the first push decaying to 10).
//! - The `rand += setting_30` LCG perturb after the two pick rolls
//!   is unmodeled — the project-wide convention (multipart.rs:71).
//! - `word_0x36548` (set case 0, cleared case 0xF) has NO reader in
//!   retail (savegame/debug only) — not carried.

use crate::mc1::features::{Gen, tile};
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};
use crate::mc1::world::World;

/// The devourable class-9 projectile SUBTYPES (EF:13545-63) — the
/// pyramid is an anti-magic zone eating incoming spell projectiles;
/// subtype 10 (the castle-build projectile) has its own branch.
const DEVOUR_SUBTYPES: [u8; 7] = [2, 4, 5, 0x16, 0x17, 0x19, 30];

impl Gen {
    /// `sub_4BD00` (EF:33965) — the pyramid ctor MINUS the map gate
    /// (`byte_0x2FED2 & 2` lives on World — the spawn seam checks
    /// it). No ctor RNG. Sprite 341, behavior row 107, huge life,
    /// ground-clamped, ShiftRot(1024, 1280), extent yaw 512.
    pub(crate) fn mc2_spawn_doomsday(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 10;
            e.tick70 = 80;
            e.max_life = 300_000;
            e.f28 = 1; // cross-column damage contract
            e.f44 = 0; // subSpellIndex = the PHASE BITFIELD here
            e.f56 = 1;
            e.row156 = 107;
            e.f58 = 64; // byte_0x39_57 awake
            e.f66 = 3; // xtype
            e.f26 = 0;
            e.f71 = 0;
        }
        self.mc2_set_mana_half(i); // SetEvent144_49C70
        self.ent[i].f63 = self.mc2_ord(10);
        self.link(i, x, y, z);
        // GROUND-CLAMPED (getTerrainAlt_10C40); re-clamped per tick.
        self.ent[i].z = self.ground_z(x, y) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 341);
        self.ent[i].f78 = 512; // array yaw
        self.mc2_shift_rot(i, 1024, 1280);
        Some(i)
    }

    /// `sub_22490` (EF:13814) — the activation footprint wipe: over
    /// the 38x38 tile block, `sub_57390` per tile — the SAME clear
    /// the building creator uses ([`Gen::mc2_building_clear_tile`],
    /// scenery removed, unprotected creatures killed).
    fn mc2_pyramid_wipe(&mut self, cx: u8, cy: u8, own: usize) {
        for j in 0..38u8 {
            let y = cy.wrapping_sub(19).wrapping_add(j);
            for k in 0..38u8 {
                let x = cx.wrapping_sub(19).wrapping_add(k);
                self.mc2_building_clear_tile(tile(x, y), own);
            }
        }
    }

    /// `sub_56F10(x, y, -1, 0)` (EF:39499) — the flatten stamp:
    /// height += delta clamped [0,200]; nonzero heights force the
    /// flat angle nibble, a zero height runs the water-seal walk;
    /// then the per-cell AddBuildingToTerrain recompute (a4=0).
    fn mc2_doom_flatten_cell(&mut self, x: u8, y: u8) {
        let t = tile(x, y);
        let h = (self.t.height[t] as i16 - 1).clamp(0, 200);
        self.t.height[t] = h as u8;
        // The sub_56F10 cave arm (EF:39534-43, review 2026-07-15
        // C10): on a cave the ceiling counter-shifts by the raw
        // delta (dig down = roof up), saturating at 255 with
        // retail's char truncation below zero — the same arm the
        // shared dig_cell chassis carries.
        if self.is_cave() {
            let c = self.t.ceiling[t] as i32 + 1;
            self.t.ceiling[t] = if c >= 255 { 255 } else { c as u8 };
        }
        if h != 0 {
            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
        } else {
            let sealed = [
                (255u8, 255u8),
                (0, 255),
                (1, 255),
                (1, 0),
                (255, 0),
                (255, 1),
                (0, 1),
                (1, 1),
            ]
            .iter()
            .all(|&(dx, dy)| {
                let a = self.t.angle[tile(x.wrapping_add(dx), y.wrapping_add(dy))] & 7;
                a != 5 && a != 2 && a != 3
            });
            if sealed {
                self.t.angle[t] &= 0xF0;
            }
        }
        self.mc2_add_building_region(x, y, x, y);
    }

    /// `sub_22270` → `sub_222B0` (EF:13683-13774): re-clamp z to the
    /// ground and turn toward the player. The relative-yaw bucket
    /// picks a snap; otherwise the facing mode (f69) sets the roll
    /// target and a `sub_58350` rate-limited turn walks yaw to it.
    fn mc2_pyramid_face(&mut self, i: usize, ctx: &MobCtx) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        if self.ent[i].act_life < 10 {
            return;
        }
        let yaw = self.ent[i].f30;
        let bucket = ((yaw.wrapping_sub(ctx.pyaw) >> 3) & 0xF0) >> 4;
        if bucket <= 2 {
            self.ent[i].f30 = ctx.pyaw.wrapping_add(384) & 0x7FF;
            return;
        }
        if bucket >= 0xD {
            self.ent[i].f30 = ctx.pyaw.wrapping_add(6) & 0x7FF;
            return;
        }
        match self.ent[i].f69 {
            0 => {
                self.ent[i].f34 = Self::angle_between(x, y, ctx.px, ctx.py);
            }
            2 => {
                // ±512 alternating by frame parity, then hold (mode 1).
                let side = if self.ent[i].f63 & 1 == 0 {
                    512u16
                } else {
                    1536
                };
                self.ent[i].f34 = ctx.pyaw.wrapping_add(side) & 0x7FF;
                self.ent[i].f69 = 1;
            }
            3 => {
                self.ent[i].f34 = self.ent[i].f30;
            }
            _ => {}
        }
        let rate = self.ent[i].f46;
        let step = Self::turn_step(self.ent[i].f30, self.ent[i].f34, rate);
        self.ent[i].f30 = (self.ent[i].f30 as i32 + step as i32) as u16 & 0x7FF;
    }

    /// `sub_22190` (EF:13625) — the damage-mailbox read with the
    /// IMMORTAL CLAMP: damage lands (1..=300 per tick) but life is
    /// pinned back to 8 whenever it would drop below 10.
    fn mc2_pyramid_mail(&mut self, i: usize) {
        if self.ent[i].f58 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            if src != 0 {
                let v = (amt as i32).clamp(1, 300);
                self.ent[i].act_life -= v;
                self.ent[i].f40 = src;
                self.ent[i].mail[0].1 = 0;
            } else {
                // No pending hit: the attacker memory clears
                // (`else word_0x26_38 = 0`, EF:13648-51 — review C9;
                // the field is f40, NOT the ring-angle f42 home).
                self.ent[i].f40 = 0;
            }
        }
        if self.ent[i].act_life < 10 {
            self.ent[i].act_life = 8;
        }
    }
}

impl World {
    /// `sub_21030` (EF:12654-12880) — the pyramid's 16-state machine
    /// (actions 80..=87 all funnel here; the +1..+3/+6/+7 handlers
    /// are literal `actionIndex = 80` resets, EF:13844-13903).
    pub(crate) fn mc2_doomsday_tick(&mut self, i: usize, ctx: &MobCtx) {
        // The thin handlers: reset to the machine state.
        if matches!(self.g.ent[i].tick70, 81..=83 | 86 | 87) {
            self.g.ent[i].tick70 = 80;
        }
        // Retail's ctor map gate (`byte_0x2FED2 & 2` — EF:33968),
        // applied on the first tick (the spawn-seam ordering note):
        // no doom-palette level, no pyramid.
        if !self.mc2_doom_level {
            self.g.ent[i].flags |= 0x400;
            return;
        }
        let mut death_sound = false;
        // Prologue: the projectile-devour pass trips phase bit0.
        if self.mc2_pyramid_devour(i) {
            self.g.ent[i].f44 |= 1;
        }
        let state = self.g.ent[i].f71;
        if state > 1 && !(0xC..=0xF).contains(&state) && self.g.ent[i].act_life >= 10 {
            self.g.mc2_pyramid_mail(i);
        }
        let cx = ((self.g.ent[i].x.wrapping_add(128)) >> 8) as u8;
        let cy = ((self.g.ent[i].y.wrapping_add(128)) >> 8) as u8;
        // Retail's setup cases fall THROUGH to their successor's body
        // in the same tick (goto, no break — EF §5; review 2026-07-15
        // C4: every phase used to run a tick long, the first summon
        // fired a tick late).
        let mut fall = true;
        while std::mem::take(&mut fall) {
            match self.g.ent[i].f71 {
                0 => {
                    // Doomsday active ON; arm the terrain-flatten bit;
                    // wipe the footprint; target the player. Falls into
                    // case 1 — the first flatten tick runs NOW.
                    self.g.ent[i].f71 = 1;
                    self.g.ent[i].f44 = 8;
                    self.g.ent[i].f26 = 15;
                    self.g.ent[i].f46 = 22;
                    self.g.ent[i].f146 = PLAYER_TARGET;
                    self.mc2_doom_meter = 60;
                    self.g.mc2_pyramid_wipe(cx, cy, i);
                    fall = true;
                }
                1 => {
                    if self.mc2_pyramid_attack(i, ctx, cx, cy) {
                        self.g.ent[i].f71 = 4;
                        self.g.ent[i].f44 |= 0x80;
                    }
                }
                2 => {
                    let d = self.g.ent_rand(i);
                    let (life, maxl) = (self.g.ent[i].act_life, self.g.ent[i].max_life as i32);
                    let v = (26 * life / maxl.max(1)) - (d & 7) as i32;
                    self.g.ent[i].f26 = v.clamp(3, 26) as i16;
                    self.g.ent[i].f71 = 3;
                    self.g.ent[i].f69 = 0;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_set_sprite(i, 341);
                    fall = true;
                }
                3 => {
                    if self.g.ent[i].act_life < 10 {
                        self.g.ent[i].f71 = 12;
                    } else if self.g.ent[i].f44 & 1 != 0 {
                        self.g.ent[i].f71 = 6;
                    } else {
                        self.g.ent[i].f26 -= 1;
                        let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
                        let dx = (ctx.px as i32 - ex as i32) as i16 as i32;
                        let dy = (ctx.py as i32 - ey as i32) as i16 as i32;
                        let near = dx * dx + dy * dy < 0x2000i32.pow(2);
                        if near && self.g.ent[i].f26 <= 0 {
                            let d = self.g.ent_rand(i);
                            self.g.ent[i].f71 = if d % 0xC < 9 { 4 } else { 6 };
                        }
                    }
                }
                4 => {
                    self.g.ent[i].f71 = 5;
                    self.g.ent[i].f26 = 6;
                    self.g.ent[i].f69 = 2;
                    self.g.ent[i].f46 = 113;
                    fall = true; // states 4+5 span 6 ticks INCL. entry
                }
                5 => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 6;
                    }
                }
                6 => {
                    self.g.ent[i].f71 = 7;
                    self.g.ent[i].f26 = 16;
                    self.g.ent[i].f69 = 0;
                    self.g.ent[i].f46 = 113;
                    self.g.mc2_set_sprite(i, 343);
                    fall = true;
                }
                7 => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 8;
                    }
                }
                8 => {
                    self.g.ent[i].f71 = 9;
                    self.g.ent[i].f26 = 0;
                    self.g.ent[i].f69 = 3;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_set_sprite(i, 342);
                    self.mc2_pyramid_pick_summon(i);
                    fall = true; // the pick AND the first shot same tick
                }
                9 => {
                    self.mc2_pyramid_do_summon(i, ctx);
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 10;
                    }
                }
                0xA => {
                    self.g.ent[i].f71 = 11;
                    self.g.ent[i].f26 = 16;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_set_sprite(i, 344);
                    fall = true;
                }
                0xB => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 2;
                    }
                }
                0xC => {
                    // Death script begins: the (10,17) doomsday sphere.
                    self.g.ent[i].f71 = 13;
                    self.g.ent[i].f26 = 32;
                    let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
                    if let Some(s) = self.g.mc2_spawn_meteor(x, y, 0) {
                        self.g.ent[s].z = 0;
                        self.g.ent[s].max_life = 70;
                        self.g.ent[s].act_life = 70;
                        self.g.ent[s].id24 = PLAYER_TARGET;
                    }
                    fall = true;
                }
                0xD => {
                    death_sound = true;
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 14;
                        self.g.ent[i].f26 = 32;
                        self.g.mc2_set_sprite(i, 345);
                    }
                }
                0xE => {
                    death_sound = true;
                    self.g.snd(10, i);
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 15;
                        self.g.ent[i].f26 = 60;
                        self.g.ent[i].act_life = -1;
                        self.mc2_kill_all_creatures();
                        // The life reset walks `dword_38523` — the SPHERE
                        // family (10, 39/40/57) — not the whole pool
                        // (EF:12847-54; review 2026-07-15 P0-4). The
                        // byte[1]|=0x20 render bit is skipped — module doc.
                        for e in self.g.ent.iter_mut().skip(1) {
                            if e.class64 == 10 && matches!(e.model65, 39 | 40 | 57) {
                                e.max_life = 140;
                                e.act_life = 140;
                            }
                        }
                    }
                }
                0xF => {
                    self.mc2_kill_all_creatures();
                    death_sound = true;
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        // THE APOCALYPSE: the (10,9) dome in its endgame
                        // variant — create, force fields, THEN latch
                        // (the order is load-bearing: the ctor call site
                        // clears the latch, EF:12864-12872).
                        let (x, y, z) = {
                            let e = &self.g.ent[i];
                            (e.x, e.y, e.z)
                        };
                        self.mc2_apocalypse = false;
                        if let Some(d) = self.g.mc2_spawn_dome(x, y, z) {
                            self.g.ent[d].act_life = 32;
                            self.g.ent[d].max_life = 11;
                            self.g.ent[d].id24 = PLAYER_TARGET;
                            self.mc2_apocalypse = true;
                        }
                        // Retail leaves the doom meter AT 1200 here — no
                        // zero write in case 0xF (review 2026-07-15 C9).
                        self.g.ent[i].flags |= 0x400;
                    }
                }
                _ => {}
            }
        }
        // LABEL_48: the death-phase rumble + ground-clamp/facing.
        if death_sound && self.g.ent[i].f63 & 3 == 0 {
            self.g.snd(63, i);
        }
        self.g.mc2_pyramid_face(i, ctx);
    }

    /// `sub_21F60` (EF:13519-13620) — the DEVOUR pass: the pyramid
    /// eats incoming class-9 spell PROJECTILES (an anti-magic zone —
    /// review 2026-07-15 P1-24/C1: the old creature walk + the
    /// player-proximity trip were misreads). Eligible subtypes within
    /// 0xC00 (3-D) are absorbed: a (10,0) mana-absorb spawns at the
    /// projectile (owner = the pyramid) and it despawns. Subtype 10
    /// (the castle-build projectile) instead tests the pyramid's
    /// (5120,5120) exclusion box against the player's castle (or the
    /// landing spot) and, devoured, CANCELS the Castle spell (the
    /// manifestation window zeroed). Trip = devoured anything this
    /// tick OR the player's Rebound window is live (EF:13616-18) —
    /// Rebound would reflect the pyramid's shots, so it switches to
    /// the un-reboundable beam.
    fn mc2_pyramid_devour(&mut self, i: usize) -> bool {
        let (ex, ey, own) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.id24)
        };
        let mut devoured = false;
        for j in 1..self.g.ent.len() {
            let (sub, x, y, z) = {
                let e = &self.g.ent[j];
                if e.class64 != 9 || e.flags & 0x400 != 0 {
                    continue;
                }
                (e.model65, e.x, e.y, e.z)
            };
            let eat = if DEVOUR_SUBTYPES.contains(&sub) {
                // 2-D: retail's `EuclideanDistXYZ_58490` (EF:13567)
                // never reads z — the absorb bubble is a CYLINDER,
                // not a sphere; the 3-D translation leaked
                // vertically-offset projectiles through (2026-07-18
                // distance audit).
                let dx = (x.wrapping_sub(ex) as i16) as i64;
                let dy = (y.wrapping_sub(ey) as i16) as i64;
                dx * dx + dy * dy <= 0xC00 * 0xC00
            } else if sub == 10 {
                // The castle-build projectile: bbox overlap between
                // the pyramid's (5120,5120) box and the player's
                // castle extents (else the projectile's own spot).
                let (tx, ty, hx, hy) = match self.player_castle() {
                    Some(c) => {
                        let e = &self.g.ent[c];
                        (e.x, e.y, e.f80 as i32, e.f82 as i32)
                    }
                    None => (x, y, 5120, 5120),
                };
                let dx = ((tx.wrapping_sub(ex)) as i16 as i32).abs();
                let dy = ((ty.wrapping_sub(ey)) as i16 as i32).abs();
                dx <= 5120 + hx && dy <= 5120 + hy
            } else {
                false
            };
            if !eat {
                continue;
            }
            devoured = true;
            if sub == 10 {
                // Cancel the Castle spell (EF:13608-09; guarded —
                // retail writes the entity-0 sentinel unguarded).
                let m = self.mc2_book.ent[2] as usize;
                if m != 0 {
                    self.g.ent[m].f26 = 0;
                }
            }
            if let Some(s) = self.g.mc2_spawn_fire(x, y, z) {
                self.g.ent[s].id24 = own;
            }
            self.g.ent[j].flags |= 0x400;
        }
        // The Rebound trip (EF:13616-18).
        let m8 = self.mc2_book.ent[8] as usize;
        devoured || (m8 != 0 && self.g.ent[m8].f26 > 0)
    }

    /// `sub_21490` (EF:12886) — the phase-bit attack driver. Returns
    /// "idle" (no bit set) so the machine escalates.
    fn mc2_pyramid_attack(&mut self, i: usize, _ctx: &MobCtx, cx: u8, cy: u8) -> bool {
        let bits = self.g.ent[i].f44;
        let mut idle = false;
        let mut suppress_ring = false;
        if bits & 8 != 0 {
            // The terrain-flatten crater.
            if self.g.ent[i].f26 < 0 {
                // Expansion done: radius-7 disc fully flat?
                let flat = self.g.ring_cells(0, 7).iter().all(|&(dx, dy)| {
                    self.g.t.tile_type[tile(cx.wrapping_add(dx), cy.wrapping_add(dy))] == 0
                });
                if flat {
                    self.g.ent[i].f44 = (self.g.ent[i].f44 | 4) & !8;
                    self.g.ent[i].f26 = 70;
                } else {
                    self.g.ent[i].f26 = 15;
                }
            } else {
                self.g.snd(10, i);
                let radius = (15 - self.g.ent[i].f26).clamp(0, 15) as i32;
                for (dx, dy) in self.g.ring_cells(0, radius) {
                    self.g
                        .mc2_doom_flatten_cell(cx.wrapping_add(dx), cy.wrapping_add(dy));
                }
                self.terrain_dirty = true;
                self.g.ent[i].f26 -= 1;
            }
        } else if bits & 4 != 0 {
            // The kill-all countdown (70 ticks; the 0x23/0x11 render
            // checkpoints are the fade bits — presentation-skipped;
            // checkpoint 1's global wipe lands).
            self.mc2_kill_all_creatures();
            let v7 = self.g.ent[i].f26;
            self.g.ent[i].f26 -= 1;
            if v7 == 70 {
                // First tick of the kill-all phase: retail zeroes
                // `countStageVars_0x36E00` — the whole hold-gate/
                // objective StageVar subsystem dies with the world
                // (EF:12996-98; review 2026-07-15 C5). Clearing the
                // vec is the port's registration-count zero.
                self.mc2_stagevars.clear();
            }
            if v7 == 1 {
                // Checkpoint 1 despawns `dword_38523` — the sphere
                // family (10, 39/40/57) — NOT the world: retail's
                // v29==3 arm runs DisableEntityDrawing over that list
                // only (EF:13048-66; review 2026-07-15 P0-4). Castles,
                // wizards and effects survive the activation crater.
                for e in self.g.ent.iter_mut().skip(1) {
                    if e.class64 == 10 && matches!(e.model65, 39 | 40 | 57) {
                        e.flags |= 0x400;
                    }
                }
                self.entities_dirty = true;
            } else if v7 <= 0 {
                self.g.ent[i].f44 = (self.g.ent[i].f44 | 0x10) & !4;
                self.g.ent[i].f26 = 1;
            }
        } else if bits & 0x10 != 0 {
            if self.g.ent[i].f26 == 1 {
                self.g.ent[i].f26 = 0;
                self.g.ent[i].f44 &= !0x40;
            } else if bits & 0x40 != 0 {
                let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
                let dx = (_ctx.px as i32 - ex as i32) as i16 as i32;
                let dy = (_ctx.py as i32 - ey as i32) as i16 as i32;
                if dx * dx + dy * dy >= 0xA00i32.pow(2) {
                    self.g.ent[i].f44 &= !0x40;
                } else {
                    self.g.ent[i].f26 = 30;
                    self.g.ent[i].f44 = (self.g.ent[i].f44 | 0x20) & !0x10;
                }
            }
        } else if bits & 0x20 != 0 {
            // The HUD doom-meter ramp (0..1200).
            if self.g.ent[i].f26 >= 600 {
                suppress_ring = true;
            }
            let v = (self.g.ent[i].f26 + 30).min(1200);
            self.g.ent[i].f26 = v;
            if v >= 1200 {
                self.g.ent[i].f44 &= !0x20;
            }
            self.mc2_doom_meter = v;
        } else {
            idle = true;
        }
        // The spinning (10,14) falling-rock summon ring.
        if !suppress_ring {
            let spin = self.g.ent[i].f36.wrapping_add(96) & 0x7FF;
            self.g.ent[i].f36 = spin;
            let (ex, ey, ez) = {
                let e = &self.g.ent[i];
                (e.x, e.y, e.z)
            };
            for k in 0..4u16 {
                let ang = spin.wrapping_add(512 * k) & 0x7FF;
                let mut p = (ex, ey, ez);
                Gen::polar_step(&mut p, ang, 0, 192);
                if let Some(r) = self.g.mc2_spawn_smoke_particle_for(14, p.0, p.1, p.2) {
                    // Each successful spawn draws the PYRAMID's own
                    // LCG once for the rock's life (EF:13086-87;
                    // review C9 — was rolled on the rock's stream).
                    let d = self.g.ent_rand(i);
                    self.g.ent[r].act_life = ((d & 7) + 8) as i32;
                }
            }
        }
        idle
    }

    /// `sub_21850` (EF:13101-13265) — pick the summon: a weighted
    /// roll over creatures (population-capped), projectile bursts,
    /// or the player beam. Reworked per review 2026-07-15 C2/C8/C9:
    /// the f26/f38/f50 writes PRECEDE the cap test (a cap-failed
    /// roll still mutates them — retail quirk); the caps for picks
    /// 4/6 are evaluated against the MODEL-0 population (verbatim —
    /// sub_223E0's three identical bucket-0 loops; only picks 3 and
    /// 5 count their own kind, 5 excluding action 200); roll-2
    /// picks 8/9 fire ONE shot (f38=1, f26=5); the bit7 escalation
    /// forces roll 1 to 0; a trip while asleep writes NO pick
    /// fields; the trip-laser re-arms the beam ramp (bit1 — C8).
    fn mc2_pyramid_pick_summon(&mut self, i: usize) {
        // sub_223E0's population counts over the class-5 buckets
        // (live + bucketed: life >= 0, action not a corpse state).
        let count = |g: &Gen, m: u8, excl_200: bool| -> usize {
            g.ent
                .iter()
                .skip(1)
                .filter(|e| {
                    e.class64 == 5
                        && e.model65 == m
                        && e.flags & 0x400 == 0
                        && e.act_life >= 0
                        && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                        && !(excl_200 && e.tick70 == 200)
                })
                .count()
        };
        // bit1 cleared on every entry (EF:13122).
        self.g.ent[i].f44 &= !2;
        let mut laser = false;
        let mut picked: Option<u8> = None;
        if self.g.ent[i].f44 & 1 != 0 {
            // The devour/rebound trip: forced laser when awake, WITH
            // the bit1 beam-ramp re-arm (EF:13127-31). Asleep: bit0
            // clears and NOTHING else is written (stale selector).
            self.g.ent[i].f44 &= !1;
            if self.g.ent[i].f58 != 0 {
                laser = true;
                self.g.ent[i].f44 |= 2;
            }
        } else {
            self.g.ent[i].f44 |= 2;
            // bit7: the post-opening escalation forces roll 1 to 0 —
            // straight to the projectile roll (EF:13141-45).
            let v4 = if self.g.ent[i].f44 & 0x80 != 0 {
                self.g.ent[i].f44 &= 0x7F;
                0
            } else {
                self.g.ent_rand(i) % 0x46
            };
            let creature_writes = |w: &mut Self, f38: i16, f50: i16| {
                w.g.ent[i].f26 = 8;
                w.g.ent[i].f38 = f38 as u16;
                w.g.ent[i].f50 = f50;
            };
            match v4 {
                3..=6 => laser = true,
                40..=48 => {
                    creature_writes(self, 8, 256);
                    if count(&self.g, 0, false) < 28 {
                        picked = Some(6);
                    }
                }
                49..=58 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 0, false) < 4 {
                        picked = Some(3);
                    }
                }
                59..=68 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 25, true) < 6 {
                        picked = Some(5);
                    }
                }
                _ if v4 >= 69 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 0, false) < 12 {
                        picked = Some(4);
                    }
                }
                _ => {}
            }
            if picked.is_none() && !laser {
                match self.g.ent_rand(i) % 0x1D {
                    0..=7 => {
                        picked = Some(1);
                        self.g.ent[i].f38 = 10;
                        self.g.ent[i].f26 = 10;
                    }
                    8..=17 => {
                        picked = Some(2);
                        self.g.ent[i].f38 = 8;
                        self.g.ent[i].f26 = 8;
                    }
                    18..=25 => {
                        picked = Some(9);
                        self.g.ent[i].f38 = 1;
                        self.g.ent[i].f26 = 5;
                    }
                    26..=27 => {
                        picked = Some(8);
                        self.g.ent[i].f38 = 1;
                        self.g.ent[i].f26 = 5;
                    }
                    _ => laser = true,
                }
            }
        }
        if laser {
            picked = Some(7);
            self.g.ent[i].f38 = 24;
            self.g.ent[i].f26 = 32;
        }
        if let Some(p) = picked {
            self.g.ent[i].f68 = p;
        }
    }

    /// `sub_21AB0` (EF:13270-13511) — execute the summon/fire each
    /// state-9 tick while the repeat count lasts. Every launch
    /// shares the preamble (EF:13317-25): pyramid pos stepped 640
    /// along the pyramid yaw at z+768; creatures step 1792 further
    /// at the stride bearing (review 2026-07-15 C6 — projectiles
    /// used to fire from the raw center).
    fn mc2_pyramid_do_summon(&mut self, i: usize, ctx: &MobCtx) {
        if self.g.ent[i].f38 == 0 {
            return;
        }
        self.g.ent[i].f38 -= 1;
        let (ex, ey, ez, own_id) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        let tpos = (ctx.px, ctx.py, ctx.pz);
        // The shared launch point (EF:13321-25).
        let mut lp = (ex, ey, ez);
        Gen::polar_step(&mut lp, self.g.ent[i].f30, 0, 640);
        lp.2 = ez.wrapping_add(768);
        match self.g.ent[i].f68 {
            1 => {
                if let Some(p) = self.g.mc2_spawn_bolt(lp.0, lp.1, lp.2) {
                    self.g.ent[p].f44 = 800;
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.snd(15, i);
                }
            }
            2 => {
                if let Some(p) = self.g.mc2_spawn_bolt9(lp.0, lp.1, lp.2) {
                    self.g.ent[p].f44 = 800;
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.snd(23, i);
                }
            }
            3..=6 => {
                // The creature summon ring: aim stride × the ALREADY
                // DECREMENTED repeat (EF:13364), stepped 1792 from
                // the shared point, z re-forced +768 (EF:13365-68).
                let sel = self.g.ent[i].f68;
                let stride = self.g.ent[i].f50 as u16;
                let ang = stride
                    .wrapping_mul(self.g.ent[i].f38)
                    .wrapping_add(self.g.ent[i].f30)
                    & 0x7FF;
                let mut p = lp;
                Gen::polar_step(&mut p, ang, 0, 1792);
                p.2 = ez.wrapping_add(768);
                let spawned = match sel {
                    3 => self.g.mc2_spawn_m0(p.0, p.1, p.2),
                    4 => self.g.mc2_spawn_m21(p.0, p.1, p.2),
                    5 => self.g.mc2_spawn_m25(p.0, p.1, p.2),
                    _ => self.g.mc2_spawn_m19(p.0, p.1, p.2),
                };
                if let Some(s) = spawned {
                    // The summoned-creature writes (EF:13388-13425,
                    // review C7): stage tag 17, parent = the pyramid,
                    // and the ACTION OVERRIDES over the creators'
                    // defaults — written LAST (m0 1→7, m21 169→175,
                    // m25 201→207, m19 153→159).
                    let e = &mut self.g.ent[s];
                    e.f146 = PLAYER_TARGET;
                    e.id24 = own_id;
                    e.site_z = 17; // StageVar2_0x49_73
                    e.f46 = 250;
                    e.f126 = 320;
                    // `parentId_0x28_40 = pyramid` is unmodeled: the
                    // port has no creature parent-link home (f40 is
                    // the roster's attacker word) and no consumer
                    // reads it yet — APPROX. The dword_0x364D2 tally
                    // question is CLOSED (E23 census): it is the
                    // total-creatures-spawned DENOMINATOR of the
                    // level-complete "creatures killed %" stat
                    // (EF:43498-505; ++ at EF:13390/32988, boxed-in
                    // walkers decrement, EF:8860). Wire it when the
                    // stats screen is ported — no sim consumer today.
                    e.f30 = ang;
                    e.f34 = ang;
                    e.tick70 = match sel {
                        3 => 7,
                        4 => 175,
                        5 => 207,
                        _ => 159,
                    };
                    self.g.snd(
                        match sel {
                            3 => 8,
                            4 => 42,
                            5 => 37,
                            _ => 44,
                        },
                        i,
                    );
                }
            }
            7 => {
                // THE HURL-AWAY BEAM (EF:13427-56, review C3): ramp
                // 1024 → −80/tick → floor 10, applied OUTWARD along
                // pyramid→player. The pose displacement rides the
                // shared knock channel at the FULL ramp magnitude
                // (retail: MoveEntity + moveTest + floor clamp on
                // the pose — the app owns the pose; module APPROX).
                if self.g.ent[i].f44 & 2 != 0 {
                    self.g.ent[i].f52 = 1024;
                    self.g.snd(19, i);
                    self.g.ent[i].f44 &= !2;
                }
                let f = (self.g.ent[i].f52 - 80).clamp(10, 1024);
                self.g.ent[i].f52 = f;
                let away = Gen::angle_between(ex, ey, ctx.px, ctx.py);
                self.g.player_knock = (away, f as i16);
            }
            sel @ (8 | 9) => {
                // Case 8 = the (9,26) whirlwind seed, case 9 = the
                // (9,3) meteor shot (EF:13457-88): from the shared
                // launch point; owner = the pyramid; aimed at the
                // avatar; impact/damage/fuse armed per case; sound
                // 15 (docs/traces/mc2-class9-m3-m26.md §1).
                let spawned = if sel == 8 {
                    self.g.mc2_spawn_whirlwind_seed(lp.0, lp.1, lp.2)
                } else {
                    self.g.mc2_spawn_meteor_shot(lp.0, lp.1, lp.2)
                };
                if let Some(p) = spawned {
                    {
                        let e = &mut self.g.ent[p];
                        e.f68 = 10;
                        e.f69 = if sel == 8 { 22 } else { 17 };
                        e.f44 = if sel == 8 { 20 } else { 6000 };
                        e.f71 = if sel == 8 { 3 } else { 10 };
                    }
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.mc2_danger_poke(PLAYER_TARGET);
                    self.g.snd(15, i);
                }
            }
            _ => {}
        }
    }

    /// `KillAllCreatures_1B5F0` (EF:8669) — every class-5 creature
    /// dies (model 10 = the pyramid spared; model 27's branch heads
    /// get the action-221 teardown instead).
    fn mc2_kill_all_creatures(&mut self) {
        for e in self.g.ent.iter_mut().skip(1) {
            if e.class64 != 5 || e.model65 == 10 {
                continue;
            }
            if e.model65 == 27 {
                e.tick70 = 221;
                e.f38 = PLAYER_TARGET;
            } else {
                e.act_life = -1;
                e.f38 = PLAYER_TARGET;
            }
        }
    }
}
