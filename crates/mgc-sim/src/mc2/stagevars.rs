//! The MC2 StageVar subsystem — the level's TRIGGERED-SPAWN / hold-gate
//! layer (distinct from the objective board in `objective_mc2`). A level
//! authors up to 11 StageVars; each names a creature TEMPLATE that, when
//! it spawns, is put into a HELD state (`actionIndex = 8*model+7`, the
//! phase-7 wait) until the var's GATE fires — proximity, a timer, a
//! referenced model going extinct, a bound entity dying, or a
//! disposition firing. On release the creature drops to its active
//! action (`8*model+1`); a nonzero chain byte re-holds it on another
//! slot (a repeating/chained trigger).
//!
//! Port of `InitStageVars_11EE0` (loader), `sub_12100`/`sub_12330`
//! (attach-at-spawn), `sub_12780` (per-tick global scan), `sub_12500`
//! (per-entity reaction), `sub_12410`/`sub_12470` (release/clear),
//! `sub_122C0`/`sub_12870` (disposition arm / re-arm). Verified against
//! the decompile in `docs/traces/mc2-stage-engine-completion.md` §2 +
//! the port-spec corrections banked 2026-07-14. All EF citations are
//! `reference/remc2/remc2/engine/EventsFunctions.cpp`.
//!
//! Hash discipline: the whole subsystem lives in two `World` vecs
//! (`mc2_stagevars`, `mc2_sv_held`) that hash ONLY when populated — MC1
//! and any MC2 level with no StageVars are byte-identical to before.
//!
//! HELD = frozen: a phase-7 class-5 entity with `site_z != 0` early-
//! returns in `mc2_creature_tick` (mobs.rs), so a held creature runs no
//! per-model behaviour. Retail also runs the model's phase-7 action
//! while held, but for these gated creatures that action is a wait/idle;
//! with no retail hash to match, "held until the gate, then active" is
//! the behaviour we reproduce. `site_z` carries the KIND (retail's
//! `StageVar2_0x49_73`), the same field metamorph/summon use (12/13) —
//! level kinds are 1..9, so they never collide.

use super::super::mc1::world::World;

/// One live StageVar slot (`D41A0_0.StageVars2_0x365F4[slot]`, LS:249).
/// Index-aligned with the level file's 11-slot array; slot 0 is unused.
#[derive(Debug, Clone, Copy, Default, Hash)]
pub(crate) struct Mc2StageVar {
    /// `index_0x3647A_0` low nibble — the KIND (1..9). 0 = empty slot.
    pub(crate) kind: u8,
    /// The LIVE `stage_0x3647A_1` flag byte: `&1` = match spawns by
    /// SUBTYPE (else by template index); `&2` = watch a referenced
    /// MODEL's extinction (else watch a bound entity's death); `&4` =
    /// FIRED; `&0x08`/`&0x10` = kind-7 disposition-armed (2-tick decay);
    /// `&0x20`/`&0x40` = the retrigger cadence mode.
    pub(crate) flags: u8,
    /// Source byte1 — the CHAIN slot: on release, re-hold the creature
    /// on StageVar slot #chain (a repeating trigger). 0 = terminate.
    pub(crate) chain: u8,
    /// The cadence counter (`_axis_2d.y`), advanced on each arm.
    pub(crate) cadence: u8,
    /// `str_0x3647A_2.word` — the template index whose spawn this var
    /// HOLDS (matched by index when `&1` clear).
    pub(crate) hold_word: u16,
    /// Model of `table[hold_word]` — the subtype matched when `&1` set.
    pub(crate) hold_subtype: u8,
    /// The fly-point (engine units) for kind 1 proximity and the kind-9
    /// proximity fallback (`str_0x3647C_4.axis` after the loader `<<8`).
    pub(crate) point: (u16, u16),
    /// Source `data.lo` — the template the death/extinction watch keys
    /// off (kinds 3/4/5/8/9).
    pub(crate) watch_template: u16,
    /// Model of `table[watch_template]` — the subtype whose extinction
    /// satisfies the gate when `&2` set.
    pub(crate) watch_model: u8,
    /// The bound live entity slot for the death-watch (`&2` clear);
    /// 0 = unbound. Set when the `watch_template` spawns.
    pub(crate) watch_ent: u16,
    /// Raw `data.lo`: kind-6 timer init, kind-7 disposition id.
    pub(crate) param: u16,
}

/// One HELD creature ← StageVar binding (retail keeps `StageVar1_0x48_72`
/// = slot and `word_0x4A_74` = timer/handle ON the entity; the port
/// holds them here to keep `Ent`'s hash — and the MC1 goldens —
/// untouched).
#[derive(Debug, Clone, Copy, Hash)]
pub(crate) struct Mc2Held {
    /// The held entity's pool slot.
    pub(crate) ent: u16,
    /// The StageVar slot gating it (retail `StageVar1_0x48_72`).
    pub(crate) slot: u8,
    /// `word_0x4A_74` — the kind-6 countdown (0 for other kinds).
    pub(crate) timer: i16,
}

impl World {
    /// `InitStageVars_11EE0` (EF:4631-4681): unpack the level file's
    /// 11-slot StageVar array into the live table. `vars` is the raw
    /// `(index, stage, x, y, data)` per slot, index-aligned (slot 0
    /// included but unused). Clears any prior holds.
    pub fn set_mc2_stagevars(&mut self, vars: &[(i8, i8, u8, u8, u32)]) {
        self.mc2_stagevars.clear();
        self.mc2_sv_held.clear();
        // Count = highest slot 1..10 whose byte0 low nibble is nonzero
        // (EF:4635-40); nothing to load below that.
        let count = vars
            .iter()
            .enumerate()
            .take(11)
            .filter(|(_, v)| (v.0 as u8) & 0xF != 0)
            .map(|(i, _)| i)
            .max();
        let Some(count) = count else { return };
        for &(index, stage, x, y, data) in vars.iter().take(count + 1) {
            let byte0 = index as u8;
            let kind = byte0 & 0xF;
            if kind == 0 {
                self.mc2_stagevars.push(Mc2StageVar::default());
                continue;
            }
            // Flag remap from byte0's high bits (EF:4646-53).
            let mut flags = 0u8;
            if byte0 & 0x80 != 0 {
                flags |= 0x01;
            }
            if byte0 & 0x40 != 0 {
                flags |= 0x02;
            }
            if byte0 & 0x10 != 0 {
                flags |= 0x20;
            }
            if byte0 & 0x20 != 0 {
                flags |= 0x40;
            }
            let hold_word = (x as u16) | ((y as u16) << 8);
            let hold_subtype = self.mc2_table_model(hold_word as usize).unwrap_or(0);
            let watch_template = (data & 0xFFFF) as u16;
            // Payload per kind (EF:4654-77). The fly-point stores
            // `source.axis << 8` back into a u16 = only the LOW byte of
            // each axis survives (the loader's truncation).
            let point = if matches!(kind, 1 | 2) {
                (
                    ((data & 0xFF) as u16) << 8,
                    (((data >> 16) & 0xFF) as u16) << 8,
                )
            } else {
                (0, 0)
            };
            // Extinction subtype: only meaningful when &2 (watch-model).
            let watch_model = if matches!(kind, 3 | 4 | 5 | 8 | 9) && flags & 0x02 != 0 {
                self.mc2_table_model(watch_template as usize).unwrap_or(0)
            } else {
                0
            };
            self.mc2_stagevars.push(Mc2StageVar {
                kind,
                flags,
                chain: stage as u8,
                cadence: 0,
                hold_word,
                hold_subtype,
                point,
                watch_template,
                watch_model,
                watch_ent: 0,
                param: watch_template, // kind 6 timer / kind 7 dis-id
            });
        }
        // Retroactive attach (the load-order accommodation, mirroring the
        // objective bind): `new_full` fires disposition 0 INSIDE the ctor
        // — before the app hands us these StageVars — so any class-5
        // creature authored at dis 0 is already live. Walk the live pool
        // once to hold/watch-bind those; every later spawn attaches
        // through the `spawn_from_thing` hook.
        for i in 1..self.g.ent.len() {
            if self.g.ent[i].class64 == 5 && self.g.ent[i].thing_slot != 0 {
                let ti = self.g.ent[i].thing_slot as usize;
                self.mc2_stagevar_attach(i, ti);
            }
        }
    }

    /// `sub_12100` (EF:4684-4750) — at every class-5 spawn, decide which
    /// StageVar (if any) HOLDS this creature, and bind any death-watch
    /// keyed to it. `thing_idx` = the spawning entity's template index
    /// (its `thing_slot`), `ent` = the live pool slot.
    pub(crate) fn mc2_stagevar_attach(&mut self, ent: usize, thing_idx: usize) {
        if self.mc2_stagevars.is_empty() {
            return;
        }
        let model = self.g.ent[ent].model65;
        // Pass 1 — match by template INDEX (slots with &1 clear).
        // Pass 2 — else match by SUBTYPE (slots with &1 set).
        let mut hit = None;
        for (s, v) in self.mc2_stagevars.iter().enumerate() {
            if v.kind != 0 && v.flags & 0x01 == 0 && v.hold_word as usize == thing_idx {
                hit = Some(s);
                break;
            }
        }
        if hit.is_none() {
            for (s, v) in self.mc2_stagevars.iter().enumerate() {
                if v.kind != 0 && v.flags & 0x01 != 0 && v.hold_subtype == model {
                    hit = Some(s);
                    break;
                }
            }
        }
        if let Some(slot) = hit {
            self.mc2_stagevar_arm(ent, slot as u8);
        }
        // Pass 3 — bind the live entity for a death-watch (kinds
        // 3/4/5/8/9 with &2 clear whose watch_template == this spawn),
        // and un-fire the slot (EF:4724-49).
        for v in &mut self.mc2_stagevars {
            if matches!(v.kind, 3 | 4 | 5 | 8 | 9)
                && v.flags & 0x02 == 0
                && v.watch_template as usize == thing_idx
            {
                v.watch_ent = ent as u16;
                v.flags &= !0x04;
            }
        }
    }

    /// `sub_12330` (EF:4971-5021) — arm a matched spawn: advance the
    /// cadence, and either HOLD it (phase-7 wait) or, when the cadence
    /// mode says "skip this cycle", release it straight to active.
    fn mc2_stagevar_arm(&mut self, ent: usize, slot: u8) {
        let (mode, ctr) = {
            let v = &mut self.mc2_stagevars[slot as usize];
            let c = v.cadence & 3;
            v.cadence = v.cadence.wrapping_add(1);
            (v.flags & 0x60, c)
        };
        // Cadence: hold EXCEPT the marked cycles (EF:4986-5008). `skip`
        // = release immediately (do not hold this cycle).
        let skip = match mode {
            0x20 => ctr == 3,
            0x40 => ctr & 1 != 0,
            0x60 => ctr & 3 != 0,
            _ => false,
        };
        let model = self.g.ent[ent].model65;
        if skip {
            self.mc2_stagevar_release(ent, slot, false);
            return;
        }
        let kind = self.mc2_stagevars[slot as usize].kind;
        let timer = if kind == 6 {
            self.mc2_stagevars[slot as usize].param as i16
        } else {
            0
        };
        {
            let e = &mut self.g.ent[ent];
            e.tick70 = model.wrapping_mul(8).wrapping_add(7); // 8*model+7 = HELD
            e.site_z = kind as i16; // StageVar2 = the kind (freezes at phase 7)
        }
        // Drop any stale binding for this slot recycle, then record.
        self.mc2_sv_held.retain(|h| h.ent as usize != ent);
        self.mc2_sv_held.push(Mc2Held {
            ent: ent as u16,
            slot,
            timer,
        });
    }

    /// `sub_12410`/`sub_12470` (EF:5024-42) — release a held creature.
    /// If the slot's chain byte is set, RE-ARM the creature onto slot
    /// #chain (a chained/repeating trigger); otherwise fully release to
    /// the active action `8*model+1` and clear the binding. `via_chain`
    /// guards the recursion (a chain step never re-chains here).
    fn mc2_stagevar_release(&mut self, ent: usize, slot: u8, via_chain: bool) {
        let chain = self.mc2_stagevars.get(slot as usize).map_or(0, |v| v.chain);
        if chain != 0 && !via_chain && (chain as usize) < self.mc2_stagevars.len() {
            // Re-arm onto the chain slot (sub_12330 again).
            self.mc2_stagevar_arm(ent, chain);
            return;
        }
        let model = self.g.ent[ent].model65;
        {
            let e = &mut self.g.ent[ent];
            e.site_z = 0;
            e.tick70 = model.wrapping_mul(8).wrapping_add(1); // 8*model+1 = active
        }
        self.mc2_sv_held.retain(|h| h.ent as usize != ent);
    }

    /// `sub_122C0` (EF:4961-68) — firing disposition `dis` arms every
    /// kind-7 StageVar whose stored id matches (`|= 0x18`). Called from
    /// `fire_disposition`.
    pub(crate) fn mc2_stagevar_arm_disposition(&mut self, dis: u16) {
        if self.mc2_stagevars.is_empty() {
            return;
        }
        for v in &mut self.mc2_stagevars {
            if v.kind == 7 && v.param == dis {
                v.flags |= 0x18;
            }
        }
    }

    /// `sub_12780` (EF:5135-5211) global scan + `sub_12500` (EF:5045-
    /// 5131) per-entity reaction, run once per tick BEFORE the entity
    /// tick loop (so a released creature acts the same tick, like
    /// retail's UpdateEntities ordering).
    pub(crate) fn mc2_stagevar_tick(&mut self) {
        if self.mc2_stagevars.is_empty() {
            return;
        }
        // ---- global scan: latch the FIRED bit for the watch kinds ----
        for s in 1..self.mc2_stagevars.len() {
            let v = self.mc2_stagevars[s];
            match v.kind {
                3 | 4 | 5 | 8 | 9 => {
                    if v.flags & 0x04 != 0 {
                        continue; // already latched
                    }
                    let fired = if v.flags & 0x02 != 0 {
                        // watch-by-model: the referenced subtype extinct
                        self.mc2_model_extinct(v.watch_model)
                    } else {
                        // watch a bound entity's death
                        v.watch_ent != 0 && self.mc2_ent_dead(v.watch_ent)
                    };
                    if fired {
                        self.mc2_stagevars[s].flags |= 0x04;
                    }
                }
                7 => {
                    // The 0x18 disposition-arm decays one bit per tick
                    // (0x10 first, then 0x08) — a 2-tick window.
                    let f = self.mc2_stagevars[s].flags;
                    if f & 0x18 != 0 {
                        self.mc2_stagevars[s].flags =
                            if f & 0x10 != 0 { f & !0x10 } else { f & !0x08 };
                    }
                }
                _ => {}
            }
        }
        // ---- per-entity reaction: release satisfied holds ----
        let held = self.mc2_sv_held.clone();
        for h in held {
            let ent = h.ent as usize;
            // Prune bindings whose entity is gone or no longer held.
            if ent >= self.g.ent.len()
                || self.g.ent[ent].class64 != 5
                || self.g.ent[ent].site_z == 0
                || self.g.ent[ent].flags & 0x400 != 0
                || self.g.ent[ent].act_life < 0
            {
                self.mc2_sv_held.retain(|x| x.ent != h.ent);
                continue;
            }
            let slot = h.slot;
            let v = self.mc2_stagevars[slot as usize];
            // Gate skips phases 4/5 (prekill/kill) like retail (EF:5050).
            let phase = self.g.ent[ent].tick70 & 7;
            if (4..=5).contains(&phase) {
                continue;
            }
            let (ex, ey) = (self.g.ent[ent].x, self.g.ent[ent].y);
            let release = match v.kind {
                1 => abs16(v.point.0, ex) <= 2048 && abs16(v.point.1, ey) <= 2048,
                3 => v.flags & 0x04 != 0,
                4 | 5 | 8 | 9 => {
                    if v.flags & 0x04 != 0 {
                        true
                    } else if v.flags & 0x02 == 0
                        && v.kind == 9
                        && (v.point.0 != 0 || v.point.1 != 0)
                    {
                        abs16(v.point.0, ex) <= 3072 && abs16(v.point.1, ey) <= 3072
                    } else {
                        false
                    }
                }
                6 => {
                    // Timer countdown lives in the binding.
                    let t = self
                        .mc2_sv_held
                        .iter_mut()
                        .find(|x| x.ent == h.ent)
                        .map(|x| {
                            x.timer -= 1;
                            x.timer
                        })
                        .unwrap_or(0);
                    t <= 0
                }
                7 => {
                    if v.flags & 0x18 != 0 {
                        self.mc2_stagevar_rearm_watchers();
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if release {
                self.mc2_stagevar_release(ent, slot, false);
            }
        }
    }

    /// `sub_12870` (EF:5214-40) — clear the FIRED bit on `&2` (watch-
    /// model) slots so a model-extinction gate can re-fire. Called from
    /// the kind-7 release and the disposition-fire tail.
    pub(crate) fn mc2_stagevar_rearm_watchers(&mut self) {
        for v in &mut self.mc2_stagevars {
            if matches!(v.kind, 3 | 4 | 5 | 8 | 9) && v.flags & 0x04 != 0 && v.flags & 0x02 != 0 {
                v.flags &= !0x04;
            }
        }
    }

    /// The referenced MODEL is extinct — no live class-5 instance
    /// (mirrors the type-7 objective oracle: skip the corpse/multipart
    /// phases and despawn-marked slots).
    fn mc2_model_extinct(&self, model: u8) -> bool {
        !self.g.ent.iter().skip(1).any(|e| {
            e.class64 == 5
                && e.model65 == model
                && e.act_life >= 0
                && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                && e.flags & 0x400 == 0
        })
    }

    /// A bound live entity slot reads dead / being-removed.
    fn mc2_ent_dead(&self, slot: u16) -> bool {
        self.g
            .ent
            .get(slot as usize)
            .is_none_or(|e| e.class64 == 0 || e.act_life < 0 || e.flags & 0x400 != 0)
    }
}

/// `Maths::Abs16` on the wrapping axis difference (engine units).
fn abs16(a: u16, b: u16) -> i32 {
    (a.wrapping_sub(b) as i16 as i32).abs()
}
