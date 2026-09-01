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
//! `sub_122C0`/`sub_12870` (disposition arm / re-arm). All EF citations
//! are `reference/remc2/remc2/engine/EventsFunctions.cpp`.
//!
//! Hash discipline: the whole subsystem lives in two `World` vecs
//! (`mc2_stagevars`, `mc2_sv_held`) that hash ONLY when populated — MC1
//! and any MC2 level with no StageVars are byte-identical.
//!
//! HELD ≠ frozen: a phase-7 class-5 entity with `site_z` in 1..=10/15 is
//! intercepted at the world dispatch seam and runs
//! [`World::mc2_held_tick`] — the port of `sub_1D5D0`'s per-kind held
//! action (EF:9977). Every held tick drains the damage inbox (held
//! creatures are KILLABLE — a lethal hit routes to the model's prekill,
//! `actionIndex = 8m+4`), a hit from a foreign class/model breaks the
//! hold into aggro (`StageVar2 = 10` + `sub_1E040`'s `8m+2`/`8m+6` FLEE
//! split), and the kind-3 guardian arm aggros on the watched entity
//! when it nears `v_28` (the ambush law; kind 4's "join the watched
//! entity's fight" arm is retail-inert — see `mc2_held_watch`).
//! The m27 kraken body instead runs its full 0xDF stage-command state
//! ([`World::mc2_m27_held_tick`] = `sub_29930`). `site_z` carries the
//! KIND (retail's `StageVar2_0x49_73`), the same field metamorph/summon
//! use (12/13, which stay on the mobs.rs path) — level kinds are 1..9
//! plus the runtime 10 (aggro-broken) and 15 (inert), so they never
//! collide.
//!
//! MOVEMENT: stage-held creatures are ACTIVE — retail's `sub_1D5D0`
//! cases all MOVE. Kind 1 walks to the authored point (`sub_1DDA0`),
//! kind 2 is the graze LEASH (`sub_1DBF0`: a 12-tile box around the
//! anchor — outside walks home, inside circles at +142..254/16-ticks —
//! plus the awake wizard watch that breaks to kind 10), kinds 3/4/5
//! shadow their watched entity (`sub_1D8C0`), kinds 6-9 graze while
//! their gate runs. Movement lives in the `sub_1D5D0` legs the per-model
//! wrappers call, not the wrappers themselves. `byte_0x3E_62` DOES tick
//! while held (the Events.cpp dispatch loop increments every processed
//! entity), so all cadences are time-keyed.
//!
//! APPROX register (held reductions, deliberate): the per-model phase-7
//! wrapper EXTRAS around retail's `sub_1D5D0` (ambient-sound draws and
//! speed refresh, e.g. the goat's `AddGoat05_01_1F5B0` bleat; m18's
//! ground re-snap) and the `sub_1EEE0` settle on the walk leg's hit path
//! are not run — no idle SOUND rng is drawn.

use super::super::engine::world::World;
use super::super::mc1::mobs::MobCtx;
use super::behavior::{BEHAVIOR, Mc2BehaviorRow};
use super::multipart::BRANCH_STATE;

/// A/B toggle for the STAGEVAR SIBLING WATCH-HANDLE REUSE: set
/// `MGC_NO_SV_WATCH_SIBLING_CHAIN` to restore the pre-dig behaviour,
/// where `mc2_resolve_watch`'s `&1` arm reused ANY held sibling's
/// cached `word_0x4A_74` — including a DEAD one, which retail's
/// per-model roster chain (`bytearray_38403x[model]`, EF:10622-30)
/// does not carry. See the write-up at the call site.
fn no_sv_watch_sibling_chain() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SV_WATCH_SIBLING_CHAIN").is_some())
}

/// A/B toggle for the WATCH-HANDLE RESOLVE CADENCE: set
/// `MGC_NO_SV_WATCH_CADENCE` to restore the pre-dig behaviour, where
/// the kind-3/4/5 shadow leg resolved AND CACHED the watch handle on
/// every tick, outside retail's 8-tick gate. See the write-up at the
/// call site in `mc2_held_move`.
fn no_sv_watch_cadence() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SV_WATCH_CADENCE").is_some())
}

/// A/B toggle for the WATCH-HANDLE NEAREST SCAN: set
/// `MGC_NO_SV_WATCH_CHAIN_SCAN` to restore the pre-dig pool walk, which
/// scanned in slot order with three guards retail does not have and
/// without the tick-top roster snapshot. See the write-up at the scan.
fn no_sv_watch_chain_scan() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SV_WATCH_CHAIN_SCAN").is_some())
}

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
    /// `word_0x4A_74`, retail's dual-use word: the kind-6 countdown,
    /// AND the kind-3/4 cached watch handle on `&2` (watch-model)
    /// slots (`sub_1E3E0` writes it, the kind-3 release clears it).
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
        self.mc2_sv_deferred.clear();
        // Count = highest slot 1..10 whose byte0 low nibble is nonzero;
        // SLOT 0 IS INERT — retail's fill loop runs `index = 1..count`
        // (EF:4641) and every consumer scans from 1, so an authored
        // slot 0 never loads (no shipped level authors one). 0xFF rows
        // are the level editor's UNUSED fill — not a kind-15 row. Retail
        // would include a 0xFF tail and load it with a garbage
        // out-of-table subtype read; the port treats the fill as empty
        // (deliberate: no shipped row can bind through that).
        let count = vars
            .iter()
            .enumerate()
            .take(11)
            .skip(1)
            .filter(|(_, v)| (v.0 as u8) & 0xF != 0 && v.0 as u8 != 0xFF)
            .map(|(i, _)| i)
            .max();
        let Some(count) = count else { return };
        for (slot, &(index, stage, x, y, data)) in vars.iter().take(count + 1).enumerate() {
            let byte0 = index as u8;
            let kind = byte0 & 0xF;
            if kind == 0 || slot == 0 || byte0 == 0xFF {
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
            // m9 (hive imp) DEFERS the hold (retail's third arg
            // `model == 0x9` at EF:33030 → park the slot in word74,
            // EF:4716-22): the imp finishes its 16-tick materialize
            // first, then `sub_122A0` arms the parked slot.
            if self.g.ent[ent].model65 == 9 {
                self.mc2_sv_deferred.retain(|d| d.0 as usize != ent);
                self.mc2_sv_deferred.push((ent as u16, slot as u8));
            } else {
                self.mc2_stagevar_arm(ent, slot as u8);
            }
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
            // Retail's skip path calls `sub_12470` DIRECTLY (EF:5010-14)
            // — the unconditional full clear, never the chain-aware
            // `sub_12410` — so a skip cycle releases straight to active
            // even when the slot has a chain byte.
            self.mc2_stagevar_release(ent, slot, true);
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
    /// `direct == false` = the chain-aware release (`sub_12410`,
    /// EF:5023-33): a nonzero chain byte RE-ARMS the creature onto slot
    /// #chain (a chained/repeating trigger) — used by the per-tick
    /// reaction. `direct == true` = the unconditional full clear
    /// (`sub_12470`, EF:5035-42, a leaf): release to the active action
    /// `8*model+1` and drop the binding, bypassing the chain — used by
    /// the cadence-skip path in `mc2_stagevar_arm`, which retail routes
    /// straight to `sub_12470`.
    fn mc2_stagevar_release(&mut self, ent: usize, slot: u8, direct: bool) {
        let chain = self.mc2_stagevars.get(slot as usize).map_or(0, |v| v.chain);
        if chain != 0 && !direct && (chain as usize) < self.mc2_stagevars.len() {
            // Re-arm onto the chain slot (sub_12330 again).
            self.mc2_stagevar_arm(ent, chain);
            return;
        }
        let model = self.g.ent[ent].model65;
        {
            let e = &mut self.g.ent[ent];
            e.site_z = 0;
            e.tick70 = model.wrapping_mul(8).wrapping_add(1); // 8*model+1 = active
            // ⭐⭐ A GATE RELEASE DOES NOT TOUCH THE PHASE COUNTER.
            // `sub_12470` (EF:5035-42) is a FOUR-write leaf —
            // `StageVar2 = 0; StageVar1 = 0; word_0x4A_74 = 0;
            // actionIndex = a2` — and `sub_12410` (EF:5023-33) only
            // chains into it, so `byte_0x3E_62` survives the release
            // untouched and the freed creature's brain runs the
            // release tick on its OWN standing ordinal, cadence gates
            // wherever that ordinal leaves them.
            //
            // The port used to zero `f63` here (the "immediate rescan
            // on gate release" nudge, registered in DEVIATIONS.md as
            // an invention awaiting exactly this re-check), which
            // forced EVERY `f63 % v_26 == 0` gate open on the release
            // tick. mc2l0 t=7292 slot 68 REFUTES it: a kind-6 timer
            // hold expires on a (5,13) townie (sv_timer 1->0, sv1/sv2
            // 6->0, action45 111->105) and retail's own changelog
            // reads `phase3e 148 -> 149` — the plain post-handler loop
            // increment, no reset — so `148 % v_26(40) = 28` keeps
            // `sub_23340`'s gate SHUT (EF:14576) and retail falls
            // straight to LABEL_43. The port's zero opened it and paid
            // four raw lanes plus the graded `speed` row in one tick:
            // `mc2_wander_turn`'s two entity draws (rand 9370 -> 3640,
            // roll 1053 -> 1194), the dwelling scan's `f146 = 15`, and
            // `f126 = maxSpeed + 12 = 30` (EF:14637) where retail held
            // 18. ⚠ `direct` still gates the CHAIN hop above — only
            // the phase write was the invention.
        }
        self.mc2_sv_held.retain(|h| h.ent as usize != ent);
    }

    /// `sub_122A0` (EF:4953-58) — arm a DEFERRED m9 hold: called when
    /// the imp's 16-tick materialize completes (the `dword_0x10_16`
    /// countdown tail, EF:11984-95). No-op unless a slot was parked.
    pub(crate) fn mc2_stagevar_arm_deferred(&mut self, ent: usize) {
        if let Some(pos) = self
            .mc2_sv_deferred
            .iter()
            .position(|d| d.0 as usize == ent)
        {
            let (_, slot) = self.mc2_sv_deferred.remove(pos);
            self.mc2_stagevar_arm(ent, slot);
        }
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
    /// 5131) per-entity reaction, run once per tick FIRST among the
    /// pre-passes — retail's UpdateEntities order is stagevar → awake
    /// → drip → entity loop (EF:40093-40116) — so a released creature
    /// is awake-passed and acts the same tick.
    pub(crate) fn mc2_stagevar_tick(&mut self) {
        if self.mc2_stagevars.is_empty() {
            return;
        }
        // Deferred m9 arms run inside the completion tick, in the
        // imp's own dispatch (sub_20370 EF:11992 — see
        // `tick_arm_creature`); a dead/despawned imp's entry is
        // dropped by the kill lanes' `mc2_stagevar_release`. The old
        // pre-loop pass here armed one boundary LATE — a falsified
        // deviation on three graded lanes × 39 entities (mc2l4 t=9).
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
                        // The &2-clear DEATH WATCH — the AUTHORED
                        // semantics (player-ruled 2026-07-25,
                        // data-faithful): held while unbound (the
                        // watched thing hasn't spawned; retail
                        // null-guards, sub_12780 file 0x36F80) or
                        // while the bound entity lives; fires when it
                        // dies (retail: life<0 or the dying flag —
                        // the raw-pointer deref of the pass-3-bound
                        // entity). Retail campaign play NEVER runs
                        // this law past the first seconds: a one-shot
                        // in-level checkpoint autosave (sub_57640)
                        // serializes the pointer in place and the
                        // restore is structurally unable to undo it,
                        // severing the watch into a per-config
                        // march-at-load-or-never coin. The port
                        // implements what the level DATA says, not
                        // the autosave bug — see docs/traces/
                        // mc2-level004-stagevar-ground-truth.md and
                        // DEVIATIONS.md.
                        v.watch_ent != 0 && {
                            let w = v.watch_ent as usize;
                            w >= self.g.ent.len()
                                || self.g.ent[w].class64 != 5
                                || self.g.ent[w].act_life < 0
                                || self.g.ent[w].flags & 0x400 != 0
                        }
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
            // NOT on a negative life: retail's record keeps its
            // StageVar1 byte through the whole prekill/kill arc (the
            // reaction gate skips phases 4/5, EF:5050, and the byte
            // only clears when the record frees) — the obs sv1 lane
            // reads it there (mc2l3 t=244: the castle crush sends
            // twelve bound firebugs into prekill; retail sv1 holds 1
            // until the reap ~250, the port's early prune read 0).
            if ent >= self.g.ent.len()
                || self.g.ent[ent].class64 != 5
                || self.g.ent[ent].site_z == 0
                || self.g.ent[ent].flags & 0x400 != 0
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
            // `sub_12500` case 0xA (EF:5054-57): an AGGRO-BROKEN
            // (kind-10) creature RE-LEASHES the moment it is neither
            // attacking (phase 2) nor fleeing (phase 6) — its
            // chase/flee machine dropped it back to wander, and the
            // stage bind reclaims it (`sub_12330`). This is how the
            // retail herd calms down and walks back to the graze
            // anchor after a scatter.
            if self.g.ent[ent].site_z == 10 {
                if !matches!(phase, 2 | 6) {
                    self.mc2_stagevar_arm(ent, slot);
                }
                continue;
            }
            // The DEAD-WATCH SCRUB (`sub_12500`'s kind-3/4/5 quiet
            // arms, EF:5086-89 / :5098-5104): on a `&2` (watch-model)
            // row, a cached handle whose occupant reads dead or
            // being-removed clears EVERY tick. The moment a watched
            // archer dies, the whole pack's caches zero and the next
            // shadow-walk re-resolves to the NEXT LIVE victim — this
            // is what rolls the mc2:04 skeleton assault through the
            // flock kill by kill instead of camping the first
            // corpse. Kinds 8/9 deliberately keep their cache
            // (retail's clear is gated `index >= 4 && <= 5` in that
            // arm; kind 3 has its own).
            if matches!(v.kind, 3..=5) && v.flags & 0x02 != 0 {
                if let Some(x) = self.mc2_sv_held.iter_mut().find(|x| x.ent == h.ent) {
                    let w = x.timer as u16 as usize;
                    if w != 0
                        && (w >= self.g.ent.len()
                            || self.g.ent[w].act_life < 0
                            || self.g.ent[w].class64 == 0
                            || self.g.ent[w].flags & 0x400 != 0)
                    {
                        x.timer = 0;
                    }
                }
            }
            let (ex, ey) = (self.g.ent[ent].x, self.g.ent[ent].y);
            let release = match v.kind {
                1 => abs16(v.point.0, ex) <= 2048 && abs16(v.point.1, ey) <= 2048,
                // Kind 3 (EF:5077-90): the fired bit clears the
                // word74 watch cache UNCONDITIONALLY, but releases
                // only outside phases 2/6 — the two aggro-break
                // targets (`sub_1E040`'s `8m+2`/`8m+6`), so a
                // guardian that broke into attack/flee is not
                // clobbered back to active-start mid-fight.
                3 => {
                    if v.flags & 0x04 != 0 {
                        if let Some(x) = self.mc2_sv_held.iter_mut().find(|x| x.ent == h.ent) {
                            x.timer = 0;
                        }
                        !matches!(phase, 2 | 6)
                    } else {
                        false
                    }
                }
                // Kinds 4/5/8/9 release on the fired watch bit ONLY.
                // Retail's kind-9 "proximity fallback" (EF:5108-12)
                // reads `str_0x3647C_4.axis` — but the spawn bind
                // wrote a POINTER into that union (EF:4740), so the
                // "coordinates" are pointer bytes whose high half can
                // never sit within 3072 of a world position: the
                // branch is unreachable garbage in retail and is NOT
                // reproduced (deliberate; 3 shipped kind-9 levels, all
                // death-watch-released).
                4 | 5 | 8 | 9 => v.flags & 0x04 != 0,
                6 => {
                    // Timer countdown lives in the binding. Retail's
                    // `word_0x4A_74` is an UNSIGNED word released at
                    // exactly 0 (EF:5116-18): an authored-zero timer
                    // wraps 0→0xFFFF and holds ~65536 ticks — never
                    // release-on-negative (the wrap is the law; no
                    // shipped level authors a zero).
                    let t = self
                        .mc2_sv_held
                        .iter_mut()
                        .find(|x| x.ent == h.ent)
                        .map(|x| {
                            x.timer = (x.timer as u16).wrapping_sub(1) as i16;
                            x.timer as u16
                        })
                        .unwrap_or(0);
                    t == 0
                }
                7 if v.flags & 0x18 != 0 => {
                    self.mc2_stagevar_rearm_watchers();
                    true
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

    // ---- the HELD action (`sub_1D5D0`, EF:9977) ----

    /// The per-kind held head, run at the entity's own turn in the
    /// tick loop (the world dispatch seam calls this before the
    /// per-model machines). Returns `true` when the tick was consumed
    /// (a stage-held creature); `false` falls through to the normal
    /// dispatch (not held, or metamorph/summon 12/13). See the module
    /// doc for the law + the APPROX register.
    pub(crate) fn mc2_held_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let e = &self.g.ent[i];
        if e.class64 != 5 || e.tick70 & 7 != 7 {
            return false;
        }
        let kind = e.site_z;
        if !matches!(kind, 1..=10 | 15) {
            return false;
        }
        if e.model65 == 27 {
            self.mc2_m27_held_tick(i, ctx);
            return true;
        }
        let base = self.g.ent[i].model65.wrapping_mul(8);
        match kind {
            // `sub_1D5D0` default arm: kinds without a handler (15,
            // the m27 inert marker — unreachable for other models in
            // shipped data) do nothing.
            15 => {}
            // Case 0xA: an aggro-broken creature that re-entered its
            // phase-7 wait re-raises straight back out (`sub_1E040`).
            10 => self.mc2_aggro_raise(i, base),
            _ => match self.g.mc2_state_head(i) {
                // Lethal: route to the model's prekill (`a2 + 4`) —
                // held creatures are killable (EF:10242-45).
                2 => self.g.ent[i].tick70 = base.wrapping_add(4),
                1 => self.mc2_held_hit(i, base),
                _ => {
                    // The per-kind MOVEMENT leg: stage-held creatures
                    // are ACTIVE in retail — sub_1D5D0's cases
                    // walk/graze every tick, they never freeze. Then
                    // the kind-3/4 guardian arm and the kind-2 wizard
                    // watch.
                    self.mc2_held_move(i, kind, ctx);
                    self.mc2_held_watch(i, base);
                    if self.g.ent[i].tick70 & 7 == 7 && self.g.ent[i].site_z == 2 {
                        self.mc2_held_wizard_scan(i, base, ctx);
                    }
                }
            },
        }
        // Retail's per-model phase-7 wrappers run the model's AMBIENT
        // PHYSICS after the 1D5D0 legs — the held seam mirrors two:
        // - m21 (`sub_26470` EF:16938-61, kinds 1-10; 13/14/16 zero
        //   the rest base — outside the port's held set): the JUMP
        //   CYCLE. Required — the walker's alt law only ever lifts, so
        //   without it a held devil keeps the last high ground's
        //   altitude forever and never hops/cackles.
        // - m0 (`sub_1F300`: kinds 1-0xA + 0xD/0xE/0x10 dodge+bob,
        //   0x11 bob-only): the projectile DODGE + the VERTICAL BOB.
        //   The bob is required — retail's floor bounce (+150 below
        //   ground+256) launches the arc from spawn; without it a held
        //   dragon hugs the terrain and flies flat, bouncing only
        //   after release. The dodge keeps a held dragon evading
        //   locked-on fireballs like a free one.
        // Other models' +7 tails stay skipped (APPROX, module doc).
        //
        // ⭐⭐⭐ **THE SWITCH READS `StageVar2` AFTER THE LEGS, AND
        // THERE IS NO ACTION TEST.** Both wrappers are literally
        // `sub_1D5D0(a1x, 8m); switch (a1x->StageVar2_0x49_73) { … }`
        // (`sub_1F300` EF:11355-58, `sub_26470` EF:16938-41) — the
        // selector is the field the legs JUST WROTE, and the action
        // the legs may have promoted is never consulted. The port
        // gated on the ENTRY kind AND on the action still reading
        // phase 7, which is exactly false on the one tick that
        // matters: the non-lethal hit arm and the kind-10 re-raise
        // both leave `StageVar2 = 10` (still in the case list) while
        // moving the action to `8m+2`/`8m+6`, so retail runs the
        // ambient physics on the RELEASE tick and the port skipped
        // it. mc2l6-rsg t=2032 slot 157, a stage-held kind-1 dragon
        // taking the human's 200: retail settles the ground (z 1785
        // → 1781, `mc2_alt_commit`) and then bobs on top of it —
        // `z += dword_0x10_16` (75) → 1856, velocity 75 → 70. The
        // port stopped at the settle and held 1781/75, and the two
        // fresh (9,9) bolts the human aimed at the chain that same
        // tick were born on the wrong pitch because of it.
        if matches!(self.g.ent[i].site_z, 1..=10) {
            match self.g.ent[i].model65 {
                21 => self.g.m21_jump(i),
                0 => {
                    self.g.m0_dodge(i);
                    self.g.m0_bob(i);
                }
                _ => {}
            }
        }
        // ⭐⭐ **AND m4's TAIL IS AN AIM RE-TEST, NOT AMBIENT
        // PHYSICS.** `AddScroll05_04_20140` (EF:11960-66) is the
        // archer's phase-7 wrapper and its whole body is
        // `dword_0x10_16 = 0; sub_1D5D0(entity, 32); if (actionIndex
        // == 34) sub_20060(entity);` — the aim test reads the action
        // the 1D5D0 legs JUST WROTE, so the tick a held archer's
        // wizard-watch promotes it to its attack state is the tick it
        // takes its aim: the sprite roll (a per-entity draw), sprite
        // 206, the shift-rot, `f126 = 0` (speed to a standstill) and
        // the victim's class/model into +66/+67. The port's held seam
        // pre-empts the normal dispatch, so `archer_tick`'s own copy
        // of that test never ran and a promoted archer kept walking
        // at patrol speed with the patrol sprite. mc2l0 t=3946, slot
        // 142: retail `f5a 0 -> 206`, `speed 30 -> 0`, `+66/+67 ->
        // 3/0` and one entity-rand step; the port wrote none of it.
        // ⚠ the test is NOT under the `tick70 & 7 == 7` gate above —
        // by this point the action is `base + 2`, not phase 7.
        if self.g.ent[i].model65 == 4 && self.g.ent[i].tick70 == base.wrapping_add(2) {
            self.g.archer_aim(i);
        }
        // ⭐⭐ **AND FOUR MORE WRAPPERS END IN A SUB-STATE RESET.**
        // `sub_24DF0` (m17, EF:15833-37), `AddFirebug05_13_25D50`
        // (m19, EF:16605-09), `sub_26020` (m20, EF:16746-50) and
        // `sub_2B7B0` (m28, EF:21266-70) are each exactly
        // `sub_1D5D0(a1x, 8m); if (actionIndex_0x45_69 == 8m+2)
        // byte_0x46_70 = 0;` — the test reads the action the held legs
        // JUST WROTE, so a creature the wizard-watch, the kind-3
        // ambush, the kind-10 re-raise or a foreign-class hit promotes
        // out of the hold ENTERS its attack machine at sub-state 0,
        // whatever sub-state its previous release ended in. m28 reaches
        // it one call-hop away — `sub_2B840` (EF:21298-306) is
        // `actionIndex = 226; byte_0x46_70 = 0;`, and the 226 write is
        // a no-op because the caller's guard already proved it.
        // ⚠ only the PHASE-7 wrappers are this bare: the ordinary
        // m17/m20 wrappers (`sub_25D80` :16613) also null a non-wizard
        // target, so `m17_validate`/`m20_validate` must NOT be reused
        // here. The port already owns the m19 and m28 halves
        // (`Gen::m19_reset` roster.rs:2039, `m28_tick` roster.rs:3837)
        // but hangs them off the per-model tick, which this seam
        // pre-empts (world.rs:5037) — so a re-held creature kept a
        // stale sub-state. mc2l3 slot 146: knocked into the dive latch
        // `f71 = 7` at t=275, re-held at t=277, re-promoted at t=340 —
        // retail zeroes it and re-enters case 0 (`f126 = f128` = 76, no
        // z write); the port kept 7 and took the dive-launch arm
        // (`f126 = 3*f128` = 228, `z += 64` = 115).
        // ⚠ like the m4 test above, this is NOT under the `tick70 & 7
        // == 7` gate — by this point the action is `base + 2`.
        if matches!(self.g.ent[i].model65, 17 | 19 | 20 | 28)
            && self.g.ent[i].tick70 == base.wrapping_add(2)
        {
            self.g.ent[i].f71 = 0;
        }
        // The per-model wrapper's SPEED TAIL (the goat's
        // `AddGoat05_01_1F5B0` :11452 shape, shared by the townie
        // wrapper): the flee state runs at minSpeed — applied the
        // SAME tick an aggro raise above set `8m+6` — and every quiet
        // held tick refreshes actSpeed to maxSpeed. Scoped to the
        // FLEE-flagged prey rows (goats/townsfolk), whose wrappers
        // carry the tail; predator/guardian wrappers (m18/m19/m21...)
        // keep their spawn speed while held (APPROX — their tails
        // differ per model and stay skipped with the sound rolls,
        // module doc).
        //
        // The goat's tail also rolls the idle BLEAT on EVERY wrapper
        // run (`AddGoat05_01_1F5B0` :11452: one unconditional
        // per-entity draw before the speed refresh, sound 46 on
        // `% 0x4D`). The draw is SIM state — the u16 stream feeds
        // combat rolls after release — and the mc2l0 corpus measures
        // it: 95% of all per-entity rand divergence was held goats
        // missing this draw. Other models' sound rolls stay skipped
        // (their wrappers differ per model; APPROX, module doc).
        if self.g.ent[i].model65 == 1 {
            self.g.goat_snd(i, 0x4D);
        }
        if BEHAVIOR[self.g.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0 {
            let e = &mut self.g.ent[i];
            if e.tick70 == base.wrapping_add(6) {
                e.f126 = e.f128;
            } else if e.tick70 & 7 == 7 {
                e.f126 = e.f130;
            }
        }
        true
    }

    /// The `sub_1D5D0` per-kind MOVEMENT legs (quiet path only — the
    /// inbox head ran upstream). Kind 1 (`sub_1DDA0`, EF:10171-10218)
    /// walks toward the slot's authored POINT; kind 2 (`sub_1DBF0`,
    /// EF:10246-70) is the graze LEASH — a 3072-unit (12-tile) box
    /// around the point: outside walks home, inside grazes; kinds
    /// 3/4/5 (`sub_1D8C0`, EF:10111-68) SHADOW the watched entity;
    /// kinds 6/7/8/9 (`sub_1E000/1E020/1D880/1D8A0`) graze in place
    /// while their gate runs.
    fn mc2_held_move(&mut self, i: usize, kind: i16, _ctx: &MobCtx) {
        let Some(hpos) = self.mc2_sv_held.iter().position(|h| h.ent as usize == i) else {
            return;
        };
        let slot = self.mc2_sv_held[hpos].slot as usize;
        let Some(v) = self.mc2_stagevars.get(slot).copied() else {
            return;
        };
        match kind {
            1 => self.mc2_sv_walk(i, Some(v.point), None),
            2 => {
                let e = &self.g.ent[i];
                // Retail's leash test is the wrapped 16-bit box
                // (EF:10248-50).
                let out = ((v.point.0.wrapping_sub(e.x)) as i16 as i32).abs() > 3072
                    || ((v.point.1.wrapping_sub(e.y)) as i16 as i32).abs() > 3072;
                if out {
                    self.mc2_sv_walk(i, Some(v.point), None);
                } else {
                    self.mc2_sv_graze(i);
                }
            }
            3..=5 => {
                // ⭐⭐⭐ THE RESOLVE LIVES INSIDE THE 8-TICK CADENCE
                // GATE, AND THE PORT HOISTED IT OUT. `sub_1D8C0`
                // (NETHERW.EXE file 0x420C0) runs the move core every
                // tick and then gates EVERYTHING else on the entity's
                // own dispatch counter:
                //     421cd  53                 push ebx
                //     421ce  e8 ed de ff ff     call 0x400C0 (sub_1B8C0, the move)
                //     421d3  8a 6b 3e           mov ch,BYTE PTR [ebx+0x3e]
                //     421d9  f6 c5 07           test ch,0x7
                //     421dc  0f 85 03 02 00 00  jne  0x423e5      <-- off-cadence: SKIP
                //     421f4  f6 86 f5 65 03 00 02  test [sv+0x365f5],0x2
                //     421fd  66 83 7b 4a 00     cmp WORD PTR [ebx+0x4a],0x0
                //     42202  75 0d              jne  0x42211      <-- already cached
                //     42205  e8 d6 09 00 00     call 0x42BE0      <-- sub_1E3E0, THE RESOLVE
                //     4220d  66 89 43 4a        mov WORD PTR [ebx+0x4a],ax
                // `sub_1E3E0` has exactly ONE call site in the whole
                // decompile (EF:10177) and it is this one; the kind-3
                // guardian `sub_1D7C0` (EF:10080) and the pack head
                // `sub_1D700` (EF:10042) both read `word_0x4A_74`
                // RAW, behind the SAME `& 7` gate. The port shared
                // `mc2_watch_handle` between the shadow leg and the
                // guardian arm, gated only the guardian, and so
                // resolved-and-CACHED on the seven off-cadence ticks
                // retail never resolves on.
                //
                // WITNESS mc2l6-rsg, slot 49 (5,20), a kind-3 hold.
                // Retail's own `sv_timer` (= `word_0x4A_74`) lane,
                // tick by tick: 29 at t=34,560-34,562, 23 at
                // 34,563-34,566, then the re-arm zeroes it and it
                // reads **0 for FOUR ticks** (34,567-34,570, action
                // 167, StageVar2 3) before resolving to 32 at
                // t=34,571. `phase3e & 7` is 0 at 34,563 and 34,571
                // and 4 at 34,567 — the cadence exactly. The port
                // resolved at 34,567, cached slot 34 (the only live
                // (5,19) in range on THAT tick), and still held it at
                // 34,571 when 32 had become the nearer one — which is
                // the `slot 49 action` head at t=34,572.
                //
                // Off-cadence `mc2_sv_walk` returns straight after
                // `mc2_move_core`, so `target`/`watched` are dead
                // there and passing 0 changes nothing but the resolve.
                let w = if no_sv_watch_cadence() || self.g.ent[i].f63 & 7 == 0 {
                    self.mc2_watch_handle(i, hpos, &v)
                } else {
                    0
                };
                let target = (w != 0).then(|| {
                    let t = &self.g.ent[w];
                    (t.x, t.y)
                });
                self.mc2_sv_walk(i, target, (w != 0).then_some(w));
            }
            6..=9 => self.mc2_sv_graze(i),
            _ => {}
        }
    }

    /// A/B toggle for the SHADOW BACK-OFF law (dig D2, session 96):
    /// set `MGC_NO_SV_BACKOFF` to restore the pre-2026-09-03 shape,
    /// where the kind-3/4/5 shadow walk had only `sub_1DDA0`'s three
    /// steps and never peeled away from the entity it escorts.
    fn sv_backoff_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_SV_BACKOFF").is_none())
    }

    /// The shared WALK leg (`sub_1DDA0`/`sub_1D8C0` quiet path): move
    /// core, then every 8th tick aim at the target (unless the move
    /// just hit the terrain fence — the retry yaw stands), every 64th
    /// tick a ±(85..340) wander jitter on top, and the same-model
    /// separation override last (EF:10195-10218).
    fn mc2_sv_walk(&mut self, i: usize, target: Option<(u16, u16)>, watched: Option<usize>) {
        self.g.mc2_move_core(i);
        if self.g.ent[i].f63 & 7 != 0 {
            return;
        }
        if let Some((tx, ty)) = target
            && self.g.ent[i].flags & super::mobs::F_BLOCKED == 0
        {
            let e = &self.g.ent[i];
            let mut aim = super::super::engine::features::Gen::angle_between(e.x, e.y, tx, ty);
            if self.g.ent[i].f63 & 0x3F == 0 {
                let v = self.g.mc2_rand(i);
                let r = self.g.mc2_rand(i);
                let sign = 2 * ((v % 0x9D) / 79) as i32 - 1;
                aim = (aim as i32 + ((r & 0xFF) + 85) as i32 * sign) as u16 & 0x7FF;
            }
            self.g.ent[i].f34 = aim;
        }
        // The held walk's box test sign-casts (EF:10404-09) — see
        // `mc2_avoid_packmate_at`: seam-blind across the 0x8000 map
        // centre (mc2l0-pd t=906).
        self.g.mc2_avoid_packmate_at(i, true);
        // ⭐⭐⭐ **THE SHADOW LEG ENDS IN A PERSONAL-SPACE BACK-OFF
        // FROM THE WATCHED ENTITY, AND ONLY THE SHADOW LEG DOES.**
        // `sub_1D8C0` (kinds 3/4/5, EF:10210-16, NETHERW.EXE
        // 0x42345-0x42391) closes case 0 with a FOURTH step the
        // point-walk `sub_1DDA0` (kind 1, EF:10382-10418) does not
        // have and cannot have — it walks to a POINT, not an entity:
        //     if (abs(a1x->pos.x - v12x->pos.x) < a1x->pitch + v12x->pitch
        //      && abs(a1x->pos.y - v12x->pos.y) < a1x->pitch + v12x->pitch)
        //         a1x->roll_0x20_32 = tan2(&v12x->pos, &a1x->pos);
        // — the AWAY angle, overwriting BOTH the aim and the packmate
        // override, so an escort that closes on its charge peels off
        // instead of walking through it. The threshold is the SUM of
        // the two half-extents, not the walker's alone — the shipped
        // EXE settles it: `movswl 0x54(%ebx)` then `movswl 0x54(%esi)`
        // (ebx = a1x, esi = v12x) before each `cmp`, where the
        // packmate loop 0x60 bytes earlier loads only `0x54(%ebx)`.
        // Both `jge`s prove the test is STRICT `<`, and the `movswl`
        // on 0x4C/0x4E proves the positions are SIGN-EXTENDED i16.
        // The port shares ONE walk leg between the two retail
        // functions and had only `sub_1DDA0`'s shape, so the shadow
        // never backed off (⭐⭐⭐ A LAW LANDED ON ONE CALL PATH IS NOT
        // LANDED — here, a leg that was never on the path at all).
        // mc2l4 slot 175, a kind-4 (5,3) shadowing (5,9) slot 154:
        // pitches 414 + 74 = 488. t=23 |dy| = 528 → no fire, retail
        // `roll` 968 = the toward-aim; t=31 |dx| = 152, |dy| = 425 →
        // FIRES, retail `roll` 968 → 1937 = `angle_of(-152, -425)`,
        // exactly 1024 (180°) off the port's 913. The 425/528 bracket
        // pins the sum: 414 alone misses t=31, 2*414 wrongly takes
        // t=23. `roll` is UNGRADED, so the census stayed clean and the
        // free run broke one tick later on `heading`, mc2l4's whole
        // 31-tick horizon.
        if let Some(w) = watched
            && Self::sv_backoff_law()
        {
            let (ex, ey, p) = {
                let e = &self.g.ent[i];
                (e.x, e.y, e.f80 as i32)
            };
            let (wx, wy, wp) = {
                let c = &self.g.ent[w];
                (c.x, c.y, c.f80 as i32)
            };
            let lim = p + wp;
            let d = |a: u16, b: u16| ((a as i16 as i32) - (b as i16 as i32)).abs();
            if d(ex, wx) < lim && d(ey, wy) < lim {
                self.g.ent[i].f34 =
                    super::super::engine::features::Gen::angle_between(wx, wy, ex, ey);
            }
        }
    }

    /// The GRAZE leg (`sub_1E1C0` quiet path, EF:10520-45): move
    /// core, then every 16th tick (fence-clear) turn by +(142..254) —
    /// the constant-handedness drift that walks the retail herd in
    /// ~3.5-tile circles, one lap per ~165 ticks (the player's
    /// observed 6-7 s). HOLD_STILL rows idle in place.
    fn mc2_sv_graze(&mut self, i: usize) {
        if BEHAVIOR[self.g.ent[i].row156 as usize].flags & Mc2BehaviorRow::HOLD_STILL != 0 {
            return;
        }
        self.g.mc2_move_core(i);
        if self.g.ent[i].f63 & 0xF == 0 && self.g.ent[i].flags & super::mobs::F_BLOCKED == 0 {
            let r = self.g.mc2_rand(i);
            self.g.ent[i].f34 =
                (self.g.ent[i].f34 as u32).wrapping_add(r % 0x71 + 142) as u16 & 0x7FF;
        }
    }

    /// The kind-2 WIZARD WATCH (`sub_1DBF0` tail, EF:10275-10318):
    /// while still held with kind 2, an AWAKE creature scans the
    /// class-3 list (+ the human) on its row cadence — nearest in
    /// `v_28` range and `v_30` cone, invisibles skipped — and on
    /// sight targets it and breaks to kind 10 (`sub_1E040`'s
    /// aggro/flee raise). This is retail's calm "notice the wizard"
    /// path — the graze herd never panics from presence alone unless
    /// a wanderer actually sees one up close.
    fn mc2_held_wizard_scan(&mut self, i: usize, base: u8, ctx: &MobCtx) {
        if self.g.ent[i].f58 == 0 {
            return;
        }
        let period = BEHAVIOR[self.g.ent[i].row156 as usize].v_26.max(1) as u8;
        if self.g.ent[i].f63 % period != 0 {
            return;
        }
        if let Some(t) = self.g.mc2_class3_scan(i, ctx) {
            self.g.ent[i].f146 = t;
            self.g.ent[i].site_z = 10;
            self.mc2_aggro_raise(i, base);
        }
    }

    /// `sub_1E040` (EF:10459-71): leave the hold for the model's
    /// aggro state — `8m+6` for FLEE-flagged rows, else `8m+2`.
    fn mc2_aggro_raise(&mut self, i: usize, base: u8) {
        let flee = BEHAVIOR[self.g.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
        self.g.ent[i].tick70 = base.wrapping_add(if flee { 6 } else { 2 });
    }

    /// The non-lethal-hit arm shared by every kind handler
    /// (EF:10227-39): an attacker of a foreign class or model breaks
    /// the hold — target it, mark `StageVar2 = 10`, raise to aggro —
    /// and then the GROUND SETTLE runs UNCONDITIONALLY, OUTSIDE the
    /// foreign-attacker test. All three kind handlers end their
    /// `case 1` with a bare `sub_1EEE0(a1x)` (`sub_1D8C0` EF:10240,
    /// `sub_1DDA0` EF:10434, `sub_1E1C0` EF:10554), and `sub_29930`'s
    /// m27 head reaches the same arm through `sub_1D5D0(a1x, 216)`
    /// (EF:19702). The enclosing arm is `else if (v2 <= 1)` after
    /// `if (v2 < 1)`, so it is the v2 == 1 case alone — quiet ticks
    /// never reach it, and the port's `match` is faithful.
    ///
    /// `sub_1EEE0` (EF:11172) is `sub_580E0(&pos, getTerrainAlt(pos),
    /// v_12, v_10, v_14)`, and `sub_580E0` (EF:40372) is the 2-branch
    /// servo [`Gen::mc2_alt_core`] already implements: `if (z > G) z
    /// += step; if ((i16)z <= G + hover) z = G + hover` (its `a4` =
    /// `v_10` is dead, commented out in the decompile itself).
    ///
    /// ⭐⭐ **THE HELD WALK ONLY EVER LIFTS.** The move core
    /// `sub_1B8C0` runs its own `sub_580E0` on the PREDICTED axis at
    /// the PRE-move position (EF:8798-8805) and only then steps x/y,
    /// so a walking creature's `z` is the ground of the tile it LEFT
    /// and a DESCENT leaves it hanging at the last high ground.
    /// `sub_1EEE0` is retail's only settle at the CURRENT position,
    /// and on a stage-held creature this hit arm is its only caller —
    /// skip it and the creature keeps a stale altitude indefinitely.
    ///
    /// mc2l0 t=7744 slot 89, a stage-held goat (row 98: hover
    /// `v_12` = 0, step `v_14` = −256; `StageVar2` 2 → 10, action
    /// 15 → 14): retail `z 1695 → 1585`, the port carried 1695. The
    /// −110 is the CLAMP branch, not the step — 1695 − 256 = 1439,
    /// then clamped UP to ground 1585 — and two ticks later the same
    /// servo LIFTS 1585 → 1612, which a −256 step alone can never do.
    /// ⚠ The head arithmetic pins `hover = 0` but does NOT
    /// discriminate the step: any step ≤ −110 clamps to the same
    /// 1585. The step comes from the row table, not from this fit.
    ///
    /// ⭐ TRAP #3 (grep the LANE) CLEARED: retail has 14 `sub_1EEE0`
    /// call sites; the 11 free-creature ones are already ported
    /// (mobs.rs ×8, roster.rs ×3) and the remaining three all funnel
    /// through here, so one call closes the family.
    fn mc2_held_hit(&mut self, i: usize, base: u8) {
        let src = self.g.ent[i].f40 as usize;
        let differs = src == 0
            || src >= self.g.ent.len()
            || self.g.ent[src].class64 != self.g.ent[i].class64
            || self.g.ent[src].model65 != self.g.ent[i].model65;
        if differs {
            self.g.ent[i].f146 = self.g.ent[i].f40;
            self.g.ent[i].site_z = 10;
            self.mc2_aggro_raise(i, base);
        }
        self.g.mc2_alt_commit(i);
    }

    /// The kind-3 GUARDIAN arm, every 8th tick of the STATIC f63
    /// ordinal (module doc): the AMBUSH law (`sub_1D7C0`,
    /// EF:10069-95) — aggro on the WATCHED entity itself when it
    /// comes within the row's `v_28`; marks `StageVar2 = 10` + raise.
    ///
    /// Kind 4's "join the watched entity's fight" arm (`sub_1D700`,
    /// EF:10022-66) is NOT ported: it reads the watched creature's
    /// `word_0x96_150`, which on a stage-held creature is
    /// uninitialized pool garbage in the shipped engine — remc2 had
    /// to add the literal `if (v4 == 0xae02) return;` bandaid there
    /// after its level-5 replay hit exactly that junk — so the arm
    /// dereferences noise and never validly fires. Player-replayed on
    /// retail mc2:04 (2026-07-24): the worms crawl along with the
    /// skeleton column and never join the battle; a working join arm
    /// made our worms attack the archers, who then killed them and
    /// died in the death-novas. Kind 4 keeps the shadow walk, the
    /// held-hit retaliation and the fired-bit release.
    fn mc2_held_watch(&mut self, i: usize, base: u8) {
        if self.g.ent[i].f63 & 7 != 0 {
            return;
        }
        let kind = self.g.ent[i].site_z;
        if kind != 3 {
            return;
        }
        let Some(hpos) = self.mc2_sv_held.iter().position(|h| h.ent as usize == i) else {
            return;
        };
        let slot = self.mc2_sv_held[hpos].slot as usize;
        let Some(v) = self.mc2_stagevars.get(slot).copied() else {
            return;
        };
        // Resolve the watch: `&2` slots cache the handle in word74
        // (`sub_1E3E0`, resolved on first need — retail resolves it in
        // `sub_1D8C0`'s idle arm); else the bound entity.
        let watch = self.mc2_watch_handle(i, hpos, &v);
        if watch == 0 {
            return;
        }
        let aggro_at = watch;
        let me = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        let tp = {
            let e = &self.g.ent[aggro_at];
            (e.x, e.y, e.z)
        };
        let reach = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as u32;
        if super::super::engine::features::Gen::mc2_dist3(me, tp) <= reach {
            self.g.ent[i].f146 = aggro_at as u16;
            self.g.ent[i].site_z = 10;
            self.mc2_aggro_raise(i, base);
        }
    }

    /// Resolve a held creature's WATCHED entity: `&2` (watch-model)
    /// slots cache the handle in word74 (`sub_1E3E0`, resolved on
    /// first need — retail resolves it in `sub_1D8C0`'s idle arm);
    /// else the bound entity. 0 = none/dead. Shared by the kind-3
    /// guardian arm and the kind-3/4/5 shadow movement.
    fn mc2_watch_handle(&mut self, i: usize, hpos: usize, v: &Mc2StageVar) -> usize {
        let slot = self.mc2_sv_held[hpos].slot as usize;
        let watch = if v.flags & 0x02 != 0 {
            let mut w = self.mc2_sv_held[hpos].timer as u16;
            if w == 0 {
                w = self.mc2_resolve_watch(i, slot);
                self.mc2_sv_held[hpos].timer = w as i16;
            }
            w
        } else {
            v.watch_ent
        } as usize;
        // Retail consumes the handle RAW — `sub_1D8C0`'s walk and
        // `sub_1D7C0`'s release deref the slot with NO liveness test
        // (EF:10178-84, :10080-86). A dead watch keeps steering the
        // pack at its frozen corpse position (free only clears the
        // class byte); each arrival's proximity release aggro-fails
        // on the dead target and re-leashes through `sub_12330`,
        // which ZEROES word74 (EF:5017) — scrubbing the stale cache
        // creature by creature until a fresh `sub_1E3E0` resolve
        // finds the next LIVE victim. This corpse-beacon loop is how
        // mc2:04's skeleton assault works through the archer flock
        // kill by kill; a liveness filter here deadlocked the whole
        // pack on its first kill.
        if watch == 0 || watch >= self.g.ent.len() {
            return 0;
        }
        watch
    }

    /// `sub_1E3E0` (EF:10609-48): resolve a `&2` slot's watch handle —
    /// on `&1` (subtype-matched) slots first reuse a same-slot,
    /// same-model, ON-CHAIN sibling's cached word74 (see the block
    /// below), else the nearest live class-5 of the watched subtype by
    /// 2D distance. (Retail scans the per-model live list; we scan the
    /// pool — comparison-only, same nearest.)
    fn mc2_resolve_watch(&self, i: usize, slot: usize) -> u16 {
        let v = &self.mc2_stagevars[slot];
        if v.flags & 0x01 != 0 {
            // ⭐⭐⭐ RETAIL WALKS THE PER-MODEL ROSTER CHAIN, THE PORT
            // WALKED THE HELD SIDE TABLE. `sub_1E3E0`'s `&1` arm is
            // `for (ix = bytearray_38403x[a1x->model_0x40_64];
            //      ix > Entities_EA3E4[0] && !v2x; ix = ix->next_0)`
            // (EF:10622-30) — the SCANNER'S OWN MODEL chain, whose
            // membership is rebuilt every tick top from the LIVE
            // class-5 records (`class == 5 && life >= 0 &&
            // actionIndex != 120`, [`Gen::mob_chains`]). A sibling
            // that has died is OFF that chain, so retail never reads
            // the stale `word_0x4A_74` it is still carrying and falls
            // through to the nearest-live scan below. The port's
            // `mc2_sv_held` walk has no such membership: a dead
            // sibling kept handing its corpse-beacon out forever.
            //
            // WITNESS mc2l6-rsg t=34563 (segment anchored 30407):
            // slot 49 (5,20, StageVar1 = 1) re-arms kind 3, its own
            // word74 is zeroed, and it re-resolves. Sibling slot 47
            // is a (5,20) on the same StageVar slot with word74 = 29
            // — and `life = -900`. Retail's chain does not carry it,
            // so the model-19 nearest scan elects the LIVE (5,19) at
            // slot 23; the port reused 29, whose own (5,19) had died
            // one tick earlier at 34562. That single handle is the
            // whole `slot 49 action` head at t=34564: retail holds
            // action 162 on the fresh target while the port drops back
            // to 161. (The SECOND head at 34572 is the same family but
            // a different instance — a handle the port had already
            // cached four ticks earlier and retail had scrubbed; see
            // the banked lead in the round-99 ledger.)
            if no_sv_watch_sibling_chain() {
                for h in &self.mc2_sv_held {
                    if h.slot as usize == slot && h.ent as usize != i && h.timer != 0 {
                        return h.timer as u16;
                    }
                }
            } else {
                let mine = self.g.ent[i].model65 as usize;
                for k in 0..self.g.mob_chains.visible(mine).len() {
                    let j = self.g.mob_chains.visible(mine)[k] as usize;
                    if let Some(h) = self.mc2_sv_held.iter().find(|h| h.ent as usize == j)
                        && h.slot as usize == slot
                        && h.timer != 0
                    {
                        return h.timer as u16;
                    }
                }
            }
        }
        // ⭐⭐⭐ AND THE NEAREST SCAN IS THE SAME CHAIN, WITH NO GUARDS
        // AT ALL. `sub_1E3E0`'s second loop, NETHERW.EXE 0x42C73:
        //     42c73  8b b4 86 03 96 00 00  mov esi,[esi+eax*4+0x9603]  (chain head,
        //                                        indexed by the stagevar's WATCH MODEL)
        //     42c7a  eb 21                 jmp 0x42c9d      (test-first)
        //     42c7c  8d 46 4c              lea eax,[esi+0x4c]
        //     42c87  e8 44 a0 03 00        call 0x7CCD0     (EuclideanDistXY_584D0)
        //     42c92  39 d0                 cmp eax,edx
        //     42c94  73 05                 jae 0x42c9b      <-- STRICT `<`: first of a tie holds
        //     42c96  89 45 fc / 89 f7      best = d; winner = ix
        //     42c9b  8b 36                 mov esi,[esi]    <-- ix = ix->next_0: CHAIN ORDER
        //     42c9d  3b 35 e4 a3 01 00     cmp esi,Entities[0]
        //     42ca3  77 d7                 ja  0x42c7c
        // and `EuclideanDistXY_584D0` (0x7CCD0) is a SQUARED distance
        // with a 16-bit subtraction sign-extended before the multiply
        // (`66 8b 02 / 66 2b 01 / 98 / 0f af d8`), which is what the
        // `as i16` casts below reproduce.
        //
        // **There is no life test, no reap test and no self-exclusion
        // in that loop** — membership is entirely the roster chain's,
        // and the chain is a TICK-TOP SNAPSHOT (`class == 5 &&
        // life >= 0 && actionIndex not in {0xB4,0xE8,0xEA}`,
        // EF:39987-40006). The port walked the POOL in slot order with
        // a LIVE `act_life`/`flags & 0x400` read plus `j != i`, and
        // without the action exclusions — wrong on five counts, and
        // the live read is exactly the hazard `world.rs`'s chain
        // builder documents ("a live `act_life >= 0` read on a
        // full-array walk gets BOTH ticks wrong, in opposite
        // directions").
        //
        // WITNESS mc2l4 t=310 (segment anchored 308), slot 213 (5,4),
        // a kind-3 hold on a cadence tick (`phase3e` 65 in BOTH
        // columns): retail resolves **138**, the pool walk resolves
        // **172**. Before the cadence law above the port covered this
        // up by resolving on an earlier off-cadence tick that happened
        // to elect 138 — a wrong answer reached by a wrong route.
        let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best = 0u16;
        let mut bd = u64::MAX;
        let dist = |x: u16, y: u16, e: &crate::engine::features::Ent| {
            let dx = (x.wrapping_sub(e.x) as i16 as i64).unsigned_abs();
            let dy = (y.wrapping_sub(e.y) as i16 as i64).unsigned_abs();
            dx * dx + dy * dy
        };
        if no_sv_watch_chain_scan() {
            for (j, e) in self.g.ent.iter().enumerate().skip(1) {
                if e.class64 == 5
                    && e.model65 == v.watch_model
                    && e.act_life >= 0
                    && e.flags & 0x400 == 0
                    && j != i
                {
                    let d = dist(x, y, e);
                    if d < bd {
                        bd = d;
                        best = j as u16;
                    }
                }
            }
        } else {
            let wm = v.watch_model as usize;
            for k in 0..self.g.mob_chains.visible(wm).len() {
                let j = self.g.mob_chains.visible(wm)[k] as usize;
                let d = dist(x, y, &self.g.ent[j]);
                if d < bd {
                    bd = d;
                    best = j as u16;
                }
            }
        }
        best
    }

    /// `sub_29930` (EF:19696-733) — the m27 body's 0xDF stage-command
    /// state. Order is retail-verbatim: the `sub_1D5D0`
    /// head first (which may re-raise `tick70`), then the pose select
    /// on the possibly-updated kind, the life refresh, the command
    /// arms on the possibly-updated `tick70`, and the branch drive
    /// (tentacles animate while held/emerging).
    fn mc2_m27_held_tick(&mut self, i: usize, ctx: &MobCtx) {
        let kind = self.g.ent[i].site_z;
        if matches!(kind, 1..=9) {
            // The m27 head (`sub_1D8C0` shape) — drain-only: retail's
            // `model != 27` gate EXCLUDES the kraken body from the
            // weakest-linked-life inherit (its branch chain would
            // otherwise leak branch lives into the 1e6 body).
            let mut v = 0u8;
            if self.g.ent[i].mail[0].1 != 0 {
                let (amt, src) = self.g.ent[i].mail[0];
                self.g.ent[i].act_life -= amt as i32;
                self.g.ent[i].mail[0].1 = 0;
                self.g.ent[i].f40 = src;
                v = 1;
            } else {
                self.g.ent[i].f40 = 0;
            }
            if self.g.ent[i].act_life < 0 {
                self.g.ent[i].f38 = self.g.ent[i].f40;
                v = 2;
            }
            match v {
                // 216+4 = 0xDC: the m27 prekill cascade next tick.
                2 => self.g.ent[i].tick70 = 220,
                1 => self.mc2_held_hit(i, 216),
                _ => {
                    // `sub_1B8C0`'s m27 arm = `sub_2AF10` — the held
                    // kraken still WALKS; a blocked path (code 4) arms
                    // tick70 = 216 inside m27_move, which the 0xD8 arm
                    // below converts to the inert StageVar2 = 15.
                    // Kinds 1/3/4/5 (sub_1DDA0/1D7C0/1D700/1D8C0)
                    // always run the physics head; the generic
                    // kind-6..9 handler (`sub_1E1C0`, EF:11238-40)
                    // gates it on the type-row `&2` flag — SET for
                    // m27 (row 97 flags 0x7), so those holds stand
                    // still.
                    let physics = matches!(kind, 1 | 3 | 4 | 5)
                        || BEHAVIOR[self.g.ent[i].row156 as usize].flags & 2 == 0;
                    if physics {
                        self.g.m27_move(i, true);
                    }
                    self.mc2_held_watch(i, 216);
                }
            }
        } else if kind == 10 {
            self.mc2_aggro_raise(i, 216);
        }
        // Pose select on the (possibly updated) kind (EF:19700-06):
        // ATTACK pose 337 for kinds {2, 6..9}, else idle 315.
        let v1 = self.g.ent[i].site_z;
        let pose = if v1 == 2 || (6..=9).contains(&v1) {
            337
        } else {
            315
        };
        self.g.m27_pose(i, pose);
        self.g.ent[i].act_life = 1_000_000;
        match self.g.ent[i].tick70 {
            // 0xDA — the MASS-ATTACK broadcast (EF:19708-27): every
            // branch still in its idle scan (f71 == 1) jumps to the
            // begin-whip state (2) aimed at the body's target.
            218 => {
                self.g.ent[i].site_z = 10;
                let target = self.g.ent[i].f146;
                let mut j = self.g.ent[i].f54 as usize;
                while j != 0 {
                    if self.g.ent[j].tick70 == BRANCH_STATE && self.g.ent[j].f71 == 1 {
                        self.g.ent[j].f71 = 2;
                        self.g.ent[j].f146 = target;
                    }
                    j = self.g.ent[j].f54 as usize;
                }
            }
            // 0xD8 — an emerge/teleport armed while held marks the
            // body inert to the stage machinery (EF:19728-29).
            216 => self.g.ent[i].site_z = 15,
            _ => {}
        }
        self.g.m27_drive(i, ctx);
    }
}

/// ⭐⭐⭐ THE BLOCKED-TICK AIM GATE OF `sub_1E700` — A RETAIL DWORD BIT
/// INDEX DROPPED INTO THE PORT'S **REMAPPED** FLAG WORD.
///
/// `sub_1E700`'s aim leg (EF:10812-27) is fenced by
/// `if (!(a1x->struct_byte_0xc_12_15.byte[2] & 4))` — the MOVE-BLOCKED
/// bit `sub_1B8C0` has just set or cleared two statements earlier. On a
/// blocked tick retail keeps the retry yaw the move core wrote and
/// performs NEITHER the target aim NOR the 64-tick wander jink; only the
/// same-model crowd steer-away below it still runs.
/// Verified byte-for-byte in the shipped `NETHERW.EXE` at file
/// **0x43068** (`f6 43 0e 04  testb $0x4,0xe(%ebx)` / `0f 85 …
/// jne 0x430f0`) — @0x0E is `flags` byte[2], and the jump lands PAST
/// the `roll` store at 0x43085 (`66 89 43 20  mov %ax,0x20(%ebx)`) and
/// past the jink, on the crowd-steer loop.
///
/// Retail's flags live in ONE little-endian dword, so `byte[2] & 4` is
/// dword bit **18** — and both `sub_1E700` paraphrases in `mc2/mobs.rs`
/// wrote that literal `1 << 18`. **The port's `flags` is NOT retail's
/// dword**: `obs_project_mc2` maps `byte[1]&8 → 26`, `byte[2]&4 → 27`,
/// `byte[2]&0x10 → 28`, `byte[2]&0x20 → 29` (`mc2/mobs.rs`
/// `F_STOP`/`F_BLOCKED`/`F_NO_CORPSE`/`F_CLAIM_LOCK`), identity only
/// below bit 18. Port bit 18 has no writer at all, so the gate was
/// UNCONDITIONALLY OPEN and every blocked tick re-aimed.
/// ⭐ A SPLIT IN A SIBLING TRIO: `mc2_sv_walk` above models the SAME
/// retail gate and gets it right (`flags & F_BLOCKED`); the two
/// `sub_1E700` arms — the Summon-Army creature (`mc2_summon_core`) and
/// the doomsday-pyramid summon (`mc2_doom_summon_home_tick`), which
/// `sub_1D5D0`'s dispatch (EF:10013-16) sends to the SAME retail
/// function — both carried the raw retail bit index.
///
/// Set `MGC_NO_SUMMON_BLOCKED_BIT` to restore the dead `1 << 18` mask.
pub(crate) fn summon_blocked_mask() -> u32 {
    static V: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        if std::env::var_os("MGC_NO_SUMMON_BLOCKED_BIT").is_some() {
            1 << 18
        } else {
            super::mobs::F_BLOCKED
        }
    })
}

/// `Maths::Abs16` on the wrapping axis difference (engine units).
fn abs16(a: u16, b: u16) -> i32 {
    (a.wrapping_sub(b) as i16 as i32).abs()
}

// ------------------------------------------------------------ snapshot

use crate::snapshot::{Reader, Snap, SnapshotError, Writer};

impl Snap for Mc2StageVar {
    fn put(&self, w: &mut Writer) {
        let Mc2StageVar {
            kind,
            flags,
            chain,
            cadence,
            hold_word,
            hold_subtype,
            point,
            watch_template,
            watch_model,
            watch_ent,
            param,
        } = self;
        w.put(kind);
        w.put(flags);
        w.put(chain);
        w.put(cadence);
        w.put(hold_word);
        w.put(hold_subtype);
        w.put(point);
        w.put(watch_template);
        w.put(watch_model);
        w.put(watch_ent);
        w.put(param);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Mc2StageVar {
            kind: r.get()?,
            flags: r.get()?,
            chain: r.get()?,
            cadence: r.get()?,
            hold_word: r.get()?,
            hold_subtype: r.get()?,
            point: r.get()?,
            watch_template: r.get()?,
            watch_model: r.get()?,
            watch_ent: r.get()?,
            param: r.get()?,
        })
    }
}

impl Snap for Mc2Held {
    fn put(&self, w: &mut Writer) {
        let Mc2Held { ent, slot, timer } = self;
        w.put(ent);
        w.put(slot);
        w.put(timer);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Mc2Held {
            ent: r.get()?,
            slot: r.get()?,
            timer: r.get()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::summon_blocked_mask;
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes};
    use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};
    use crate::mc2::mobs::{F_BLOCKED, F_STOP};
    use crate::patches::WorldPatches;
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

    fn ctx_at(px: u16, py: u16) -> MobCtx {
        MobCtx {
            px,
            py,
            pz: 100,
            pyaw: 0,
            pmana: 0,
            pmana_max: 0,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: WorldPatches::RETAIL,
            mc2_turn: 0,
        }
    }

    /// One Summon-Army firebug parked at (0x2000, 0x2000), leased and
    /// on the 8-tick aim throttle, with `extra_flags` OR'd in.
    fn summoned_firebug(g: &mut Gen, extra_flags: u32) -> usize {
        let i = g.new_event().expect("summon slot");
        let e = &mut g.ent[i];
        e.class64 = 5;
        e.model65 = 19;
        e.tick70 = 8 * 19 + 7; // the shared class-5 controlled slot
        e.site_z = 13; // StageVar2 = Summon Army
        e.id24 = PLAYER_TARGET; // parent = the human's carpet
        e.row156 = 88; // the firebug's behaviour row
        e.max_life = 600;
        e.act_life = 600;
        e.x = 0x2000;
        e.y = 0x2000;
        e.z = 100;
        e.set_lease(500); // the lease (word_0x2E_46) — dig 98-Q20
        e.f63 = 8; // phase: & 7 == 0 (aim) but & 0x3F != 0 (no jink)
        e.f34 = 1234; // the sentinel target yaw (roll_0x20_32)
        // `sub_1B8C0`'s forced-stop head returns before it can touch
        // the blocked bit, so the rig can hand the gate the exact
        // flag word it is meant to read.
        e.flags |= F_STOP | extra_flags;
        i
    }

    /// ⭐⭐⭐ `sub_1E700` DOES NOT RE-AIM ON A MOVE-BLOCKED TICK.
    ///
    /// EF:10817 fences the whole aim leg with
    /// `if (!(a1x->struct_byte_0xc_12_15.byte[2] & 4))`, the bit
    /// `sub_1B8C0` has just written. Shipped `NETHERW.EXE` file
    /// 0x43068: `f6 43 0e 04  testb $0x4,0xe(%ebx)` /
    /// `0f 85 7e 00 00 00  jne 0x430f0` — the jump clears the `roll`
    /// store at 0x43085 (`66 89 43 20  mov %ax,0x20(%ebx)`) and the
    /// 64-tick jink, landing on the crowd-steer loop.
    ///
    /// The port wrote that retail DWORD bit index verbatim as
    /// `1 << 18`, but the port's flag word remaps `byte[2] & 4` to bit
    /// 27 (`F_BLOCKED`); bit 18 has no writer at all, so the gate was
    /// unconditionally open and every blocked tick re-aimed. On
    /// `recordings/mc2l6-rival-spells-galore.mgcr` that is the
    /// t=15,699 wall: firebug 498 is blocked on the one throttle-open
    /// tick t=15,696, retail holds `roll` at 1367 and the port
    /// re-aimed to 1380; three ticks later the heading servo arrives
    /// on the wrong bearing.
    ///
    /// `roll` (@0x20) has no comparator in `verify-deltas` and the
    /// pair importer restores it every tick, so no fixture can see
    /// this law — the whole-take pair census is byte-identical with it
    /// on and off. Kill switch: `MGC_NO_SUMMON_BLOCKED_BIT=1`, under
    /// which the blocked arm below aims too and this test fails.
    #[test]
    fn a_blocked_summon_keeps_the_retry_yaw_instead_of_re_aiming() {
        // (0x2000, 0x2000) -> (0x4000, 0x2000) is due +x, angle 512.
        let ctx = ctx_at(0x4000, 0x2000);

        let mut g = flat_gen();
        let free = summoned_firebug(&mut g, 0);
        g.mc2_creature_tick(free, &ctx);
        assert_eq!(
            g.ent[free].lease(), 495,
            "the StageVar2-13 leg ran (lease -1, then -4 on the \
             no-lock fallback)"
        );
        assert_eq!(
            g.ent[free].f34, 512,
            "an unblocked summon aims at its parent"
        );

        let mut g = flat_gen();
        let stuck = summoned_firebug(&mut g, F_BLOCKED);
        g.mc2_creature_tick(stuck, &ctx);
        assert_eq!(g.ent[stuck].lease(), 495, "the same leg ran on both arms");
        assert_eq!(
            g.ent[stuck].f34, 1234,
            "a BLOCKED summon keeps the yaw the move retry left \
             (EF:10817, NETHERW.EXE 0x43068)"
        );
    }

    /// The gate must read the bit `mc2_move_core` actually writes.
    /// Retail's `byte[2] & 4` is dword bit 18; the PORT files it at
    /// bit 27, and nothing in the tree ever writes port bit 18.
    #[test]
    fn the_summon_aim_gate_reads_the_port_s_own_blocked_bit() {
        assert_eq!(summon_blocked_mask(), F_BLOCKED);
        assert_ne!(F_BLOCKED, 1 << 18, "the port's flag word is remapped");
    }
}
