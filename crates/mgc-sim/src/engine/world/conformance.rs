//! Retail-conformance seams (docs/RECORDING.md "fixture runner"):
//! import a decoded retail closure onto a built world, and project the
//! world into the recorder's obs schema for tick-by-tick comparison.
//!
//! The importer is the port-side analog of retail's own in-level LOAD
//! (docs/traces/mc1-campaign-save-menu.md): the raw image lands over
//! the live state, the mode/settings words are discarded, the free
//! stack and the per-tile lists are REBUILT, and owner links are
//! re-derived. Retail's pointer fixups become index arithmetic here —
//! guest addresses are stable, so the behavior-row pointer converts to
//! a row index anchored on the human carpet's canonical row 7.
//!
//! The human player lives OUTSIDE the pool in this port, so the
//! recorded carpet slot stays a reserved hole: its state routes to
//! [`Player`]/`human_pose`, every pool field that references the
//! carpet slot translates to [`PLAYER_TARGET`], and the projection
//! synthesizes the carpet entity back at the recorded slot. The
//! conformance runner drives the pose per tick (pin-the-human), so
//! world fidelity verifies with zero dependence on input
//! reconstruction.
//!
//! Known non-closure state (retail keeps these OUTSIDE the saved
//! struct; import resets them and the runner buckets any fallout):
//! the terrain planes (craters/retile — restore via
//! [`World::restore_planes`]), the retile LCG `pseudoRand`, and the
//! volcano registers (`gamedata+36/38`).

use super::{LifeState, PLAYER_LIFE_MAX, Player, PlayerPose, World};
use crate::engine::features::{Ent, Planes};
use crate::mc1::mobs::PLAYER_TARGET;
use crate::mc1::spells::{SPELL_COUNT, SpellId};
use mgc_formats::mgcr::{
    ControlMc1, ControlMc2, EntObsMc1, EntObsMc2, FlightMc1, FlightMc2, ObsMc1, ObsMc2,
    PlayerJoinMc1, PlayerJoinMc2, PlayerMc2, RetailEntMc1, RetailEntMc2, RetailMc1, RetailMc2,
    WizardMc1,
};

/// What the importer did — counts for the runner's coverage report.
#[derive(Debug, Clone)]
pub struct ImportReport {
    /// Active pool entities imported (human carpet excluded).
    pub active: usize,
    /// The recorded human carpet slot (the reserved hole).
    pub human_slot: u16,
    /// Derived `unk_98F38` guest base (carpet row-7 anchor).
    pub behavior_base: u32,
    /// Entities whose behavior-row pointer did not convert (row 0
    /// fallback).
    pub bad_rows: usize,
    /// The recorded free/recycle stacks failed the census check and
    /// the free list fell back to the descending slot scan (spawn
    /// slot ORDER diverges from retail on such pairs): the observed
    /// live/expected counts, None when the recorded stack was used.
    pub stack_fallback: Option<(usize, usize)>,
}

/// The pinned human context the projection needs: where the carpet
/// sits in the recording and the pose the runner is driving.
#[derive(Debug, Clone, Copy)]
pub struct PinnedMc1 {
    pub slot: u16,
    pub local: u16,
    pub player_count: u16,
    pub pose: PlayerPose,
}

/// The MC2 twin of [`PinnedMc1`].
#[derive(Debug, Clone, Copy)]
pub struct PinnedMc2 {
    pub slot: u16,
    pub local: u16,
    pub player_count: u16,
    pub pose: PlayerPose,
    /// The recorded per-player `CastleEntityIndex` words (+1080),
    /// echoed through the projection: the lane holds the AUTHORED
    /// castle binding — a runtime-BUILT castle never fills it (mc2l0:
    /// 0 across the whole take with the human's castle live), so it
    /// cannot be derived from the pool.
    pub castles: [i16; 8],
}

impl World {
    /// The live terrain planes, cloned — the runner captures them
    /// right after the level build (POST feature pass: the load-time
    /// crater/flatten/wall edits are part of level init, not runtime
    /// state) and re-imprints per pair via [`World::restore_planes`].
    pub fn planes_clone(&self) -> Planes {
        Planes {
            height: self.g.t.height.clone(),
            tile_type: self.g.t.tile_type.clone(),
            shading: self.g.t.shading.clone(),
            angle: self.g.t.angle.clone(),
            ceiling: self.g.t.ceiling.clone(),
        }
    }

    /// Re-imprint the pristine terrain planes (the map file's blocks
    /// are not part of the master-struct closure; craters and retile
    /// do not survive a retail import).
    pub fn restore_planes(&mut self, planes: &Planes) {
        self.g.t = Planes {
            height: planes.height.clone(),
            tile_type: planes.tile_type.clone(),
            shading: planes.shading.clone(),
            angle: planes.angle.clone(),
            ceiling: planes.ceiling.clone(),
        };
        self.terrain_dirty = true;
    }

    /// Seed the cast edge-trigger baseline (the held state of the tick
    /// BEFORE the imported one) so a held button does not re-edge on
    /// every imported pair.
    pub fn set_prev_fire(&mut self, left: bool, right: bool) {
        self.prev_fire = (left, right);
    }

    /// Apply a decoded MC1/HW retail closure onto this (already-built,
    /// same-level) world. Overwrites the pool, the free stack, the
    /// tile lists, the global LCG/spawn ordinals and the human player
    /// column; leaves terrain planes alone (see
    /// [`World::restore_planes`]).
    pub fn retail_import_mc1(&mut self, st: &RetailMc1) -> Result<ImportReport, String> {
        // Replaying retail state means retail law exactly: deliberate
        // gameplay deviations (DEVIATIONS.md) switch off for this world.
        self.strict_retail = true;
        let local = st.local_player as usize;
        let wiz = st
            .wizards
            .get(local)
            .ok_or_else(|| format!("local player {local} out of range"))?;
        let human_slot = wiz.play_index;
        let pool = self.g.ent.len();
        if human_slot == 0 || (human_slot as usize) >= pool.min(st.ents.len()) {
            return Err(format!("human carpet slot {human_slot} out of range"));
        }
        let carpet = st.ents[human_slot as usize];
        if carpet.class64 != 3 {
            return Err(format!(
                "human carpet slot {human_slot} is class {}, want 3",
                carpet.class64
            ));
        }
        // The carpet's Type_156 is the canonical `&unk_98F38[7]`
        // (retail's own load-fixup anchor) — derive the table base
        // from it instead of hardcoding a per-build guest address.
        let behavior_base = carpet.model_ptr.wrapping_sub(7 * 32);
        let tr = |v: u16| if v == human_slot { PLAYER_TARGET } else { v };

        let n = pool.min(st.ents.len());
        let mut active = 0usize;
        let mut bad_rows = 0usize;
        for slot in 1..n {
            let r = &st.ents[slot];
            if r.class64 == 0 || slot == human_slot as usize {
                self.g.ent[slot] = Ent::default();
                continue;
            }
            active += 1;
            let row156 = if r.model_ptr == 0 {
                0
            } else {
                let d = r.model_ptr.wrapping_sub(behavior_base);
                if d % 32 == 0 && d / 32 < 256 {
                    (d / 32) as u8
                } else {
                    bad_rows += 1;
                    0
                }
            };
            self.g.ent[slot] = import_ent(r, row156, &tr);
        }
        for slot in n..pool {
            self.g.ent[slot] = Ent::default();
        }

        // Tile lists: the heads live in the map file, not the struct —
        // rebuild like retail's own per-tick list pass, ascending slot
        // order. `import_ent` cleared the link bit; `link` re-sets it.
        for h in self.g.map_entity.iter_mut() {
            *h = 0;
        }
        for slot in 1..n {
            let e = &self.g.ent[slot];
            if e.class64 != 0 && st.ents[slot].flags & 4 != 0 {
                let (x, y, z) = (e.x, e.y, e.z);
                self.g.link(slot, x, y, z);
            }
        }

        // Free stack: the LIVE recorded order (retail pops from the
        // end; recycle entries — if any — sit on top, popped first),
        // so port-side spawns land on the same slots the recording's
        // do. Fall back to the load-rebuild scan (999→1) only when the
        // recorded stack is unusable. The reserved human hole stays
        // OUT either way.
        let live: Vec<u16> = st
            .free_stack
            .iter()
            .chain(st.recycle_stack.iter())
            .copied()
            .filter(|&s| {
                (s as usize) < pool && s != human_slot && self.g.ent[s as usize].class64 == 0
            })
            .collect();
        let scan_free = pool - 1 - active - 1; // slots minus actives minus the hole
        let stack_fallback = if live.len() == scan_free {
            self.g.free = live;
            None
        } else {
            let got = live.len();
            self.g.free = (1..pool as u16)
                .rev()
                .filter(|&s| self.g.ent[s as usize].class64 == 0 && s != human_slot)
                .collect();
            Some((got, scan_free))
        };

        // Globals in the closure.
        self.g.rand = st.rand;
        self.g.spawn_count = st.spawn_count;
        // Outside the closure (retail leaves them unsaved too).
        self.g.pseudo = 0;
        self.g.erupting = 0;
        self.g.plume = 0;

        // The human column: pool-entity state routes to Player, the
        // Type_160 tail to the Gen mirrors.
        self.g.player_mail = carpet.mail.map(|(a, s)| (a, tr(s)));
        self.g.player_knock = (wiz.knock_dir, wiz.knock_mag);
        self.g.player_aggro = wiz.aggro;
        self.g.player_danger = wiz.danger;
        self.g.banked_houses = wiz.banked_houses;
        self.g.castle_alert = wiz.castle_alert;
        self.g.player_alert = wiz.player_alert;
        self.g.balloon_alert = wiz.balloon_alert;
        self.g.kills = wiz.kills;
        self.g.shots = wiz.shots;
        self.g.hits = wiz.hits;
        self.g.player_invisible = carpet.flags & 0x20 != 0;
        self.g.player_rebound = carpet.flags & 0x8000 != 0;
        for i in 1..8 {
            self.g.rival_ents[i] = st.wizards[i].play_index;
            self.g.rival_wanted[i] = st.wizards[i].aggro;
        }
        self.g.rival_ents[0] = 0;

        // Re-anchor the rival AI records to the imported pool. The
        // records were built for the fresh world's spawn slots, and
        // rival_entity_tick keys on r.ent — without the rebind every
        // imported rival carpet is a frozen husk (its motion arm is
        // verbatim sub_14EB0 and simply never ran; the first HW
        // divergence family). Flight/economy lanes reseed from the
        // recorded closure so the one tick integrates from retail's
        // own state: vdes/jink are the Type_160 v_12/v_16 the motion
        // arm consumes, grace comes from the record (the fresh-spawn
        // 100 would wipe the imported mailbox), mana lanes come from
        // the carpet entity (f132 carries cast debits).
        for ri in 0..self.rivals.len() {
            let w = &st.wizards[self.rivals[ri].slot as usize];
            let r = &mut self.rivals[ri];
            r.ent = w.play_index;
            r.eliminated = w.play_index == 0;
            if r.eliminated {
                continue;
            }
            let e = &st.ents[w.play_index as usize];
            r.mana = e.f140.max(0) as u32;
            r.mana_max = e.f136.max(0) as u32;
            r.mana_delta = e.f132 as i32;
            r.vdes = w.cmd_speed;
            r.jink = w.strafe;
            r.grace = w.grace;
            // Brain lanes: without these the record imports as Fresh
            // and the cascade re-aims f34 away from retail's lock.
            self.reanchor_rival_ai(
                ri,
                w.ai_state,
                w.burst,
                w.poverty,
                &w.cooldown,
                &w.learn,
                &w.hate,
                &w.war,
            );
        }

        // Hands: the raw +940/+944 bytes index the ACQUISITION list,
        // not the spell table — resolve through the manifestation.
        let hand = |raw: u16| {
            st.hand_spell(local, raw)
                .filter(|&s| (s as usize) < SPELL_COUNT)
                .map(SpellId)
        };
        let mut death_owned = [false; SPELL_COUNT];
        let mut death_owned_blue = [false; SPELL_COUNT];
        for s in 0..SPELL_COUNT {
            death_owned[s] = wiz.owned_slots[s] != 0;
            death_owned_blue[s] = wiz.blue[s] != 0;
        }
        self.player = Player {
            mana: carpet.f140.max(0) as u32,
            mana_max: carpet.f136.max(0) as u32,
            // The pending regen amount (+132, applied-then-recomputed
            // by the wizard tick :55390/:55415-21 — the port keeps the
            // same one-tick pipeline). Left unseeded, every imported
            // pair ticked with delta 0 and missed retail's +100 floor
            // (or the +1000 castle-boost arm) — the two biggest
            // player.mana families in the corpus.
            //
            // The recorder samples +132 AFTER the recompute, so the
            // closure always reads the refreshed floor — but every
            // live MID-burst spell event zeroes it again before the
            // next apply (sub_55E80 :64956; the first burst tick,
            // +48 == +50, does not). Re-derive the suppression or
            // every hold-fire pair over-regens one quantum (there is
            // no regen clock — the drifting cadence IS this
            // suppression beating against slot order).
            mana_delta: if st.ents.iter().any(|e| {
                e.class64 == 12 && e.f144 == 0 && e.f48 != 0 && e.f48 as i32 != e.f50 as i32
            }) {
                0
            } else {
                carpet.f132 as i32
            },
            life: carpet.act_life,
            state: match carpet.f66 {
                2 => LifeState::Falling,
                3 => LifeState::Dead,
                _ => LifeState::Alive,
            },
            left: hand(wiz.hand_left),
            right: hand(wiz.hand_right),
            owned: wiz.owned_slots,
            grace: wiz.grace,
            killer: tr(carpet.f38),
            fall_speed: carpet.f46,
            shield: carpet.flags & 0x4000 != 0,
            invisible: carpet.flags & 0x20 != 0,
            rebound: carpet.flags & 0x8000 != 0,
            death_owned,
            death_owned_blue,
            ..Player::default()
        };

        // World-level latches: cleared like retail's load discards its
        // mode block; the tick mailboxes must not leak across pairs.
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.pending_teleport = None;
        self.pending_respawn = None;
        self.pending_restart = false;
        self.duel = None;
        self.won = false;
        self.completed = false;
        self.win_streak = 0;
        self.prev_fire = (false, false);
        self.accel_veto = (false, false);
        self.rival_deaths.clear();
        self.notification = None;
        self.kill_tally = [[0; 8]; 8];
        self.entities_dirty = true;

        Ok(ImportReport {
            active,
            human_slot,
            behavior_base,
            bad_rows,
            stack_fallback,
        })
    }

    /// Project this world into the recorder's MC1 obs schema. The
    /// human carpet is synthesized back at the pinned slot;
    /// `owner_ptr` (a guest pointer) is emitted as 0 and skipped by
    /// the comparator.
    pub fn obs_project_mc1(&self, pin: &PinnedMc1) -> ObsMc1 {
        let untr = |v: u16| if v == PLAYER_TARGET { pin.slot } else { v };
        let mut entities: Vec<EntObsMc1> = Vec::new();
        for slot in 1..self.g.ent.len() as u16 {
            if slot == pin.slot {
                entities.push(self.synth_carpet_obs(pin));
                continue;
            }
            let e = &self.g.ent[slot as usize];
            if e.class64 == 0 {
                continue;
            }
            entities.push(EntObsMc1 {
                slot,
                class: e.class64,
                model: e.model65,
                sclass: e.f66,
                smodel: e.f67,
                flags: e.flags,
                id: untr(e.id24),
                life: e.act_life,
                max_life: e.max_life,
                x: e.x as f64 / 256.0,
                y: e.y as f64 / 256.0,
                z: e.z,
                heading: e.f30,
                pitch: e.f32,
                target_yaw: e.f34,
                speed: e.f126,
                mana: e.f140 as u32,
                mana_max: e.f136 as u32,
                chase: untr(e.f146),
                owner_ptr: 0,
                tick_byte: e.f63,
                rand: e.rand,
            });
        }
        let castle_of = |owner: u16| -> u16 {
            if owner == 0 {
                return 0;
            }
            self.g
                .ent
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, e)| {
                    e.class64 == 3 && e.model65 == 2 && e.id24 == owner && e.flags & 0x400 == 0
                })
                .map_or(0, |(s, _)| s as u16)
        };
        let spell_u16 = |s: Option<SpellId>| s.map(|s| s.0 as u16);
        let wizards: Vec<WizardMc1> = (0..8u16)
            .map(|i| {
                let localw = i == pin.local;
                let owner = if localw {
                    PLAYER_TARGET
                } else {
                    self.g.rival_ents[i as usize]
                };
                WizardMc1 {
                    index: i,
                    play_index: if localw {
                        pin.slot
                    } else {
                        self.g.rival_ents[i as usize]
                    },
                    hand_left: if localw {
                        spell_u16(self.player.left)
                    } else {
                        None
                    },
                    hand_right: if localw {
                        spell_u16(self.player.right)
                    } else {
                        None
                    },
                    castle: castle_of(owner),
                    flight: FlightMc1 {
                        cmd_speed: if localw { pin.pose.speed } else { 0 },
                        strafe: 0,
                        roll_acc: 0,
                        pitch_acc: 0,
                    },
                }
            })
            .collect();
        let control: Vec<ControlMc1> = (0..8u16).map(zero_control).collect();
        let player = Some(PlayerJoinMc1 {
            carpet_slot: pin.slot,
            life: self.player.life,
            max_life: PLAYER_LIFE_MAX as u32,
            mana: self.player.mana,
            mana_max: self.player.mana_max,
            x: pin.pose.x as f64 / 256.0,
            y: pin.pose.y as f64 / 256.0,
            z: pin.pose.z,
            heading: pin.pose.heading,
            pitch: pin.pose.pitch,
            speed: pin.pose.speed,
            hand_left: spell_u16(self.player.left),
            hand_right: spell_u16(self.player.right),
            castle: wizards[pin.local as usize].castle,
            flight: wizards[pin.local as usize].flight.clone(),
            control: Some(zero_control(pin.local)),
        });
        ObsMc1 {
            rng: self.g.rand,
            n_active: entities.len() as u32,
            local_player: pin.local,
            player_count: pin.player_count,
            wizards,
            control,
            player,
            entities,
        }
    }

    /// Apply a decoded MC2 retail closure onto this (already-built,
    /// same-level) world. The MC2 twin of [`World::retail_import_mc1`]
    /// — same shape: overwrite the pool, rebuild the tile lists and
    /// the free stack, seed the globals and the human column, clear
    /// the cross-pair latches.
    pub fn retail_import_mc2(&mut self, st: &RetailMc2) -> Result<ImportReport, String> {
        self.strict_retail = true;
        let local = st.local_player as usize;
        let ply = st
            .players
            .get(local)
            .ok_or_else(|| format!("local player {local} out of range"))?;
        let human_slot = ply.play_index;
        let pool = self.g.ent.len();
        if human_slot == 0 || (human_slot as usize) >= pool.min(st.ents.len()) {
            return Err(format!("human carpet slot {human_slot} out of range"));
        }
        let carpet = st.ents[human_slot as usize];
        if carpet.class3f != 3 {
            return Err(format!(
                "human carpet slot {human_slot} is class {}, want 3",
                carpet.class3f
            ));
        }
        let tr = |v: u16| if v == human_slot { PLAYER_TARGET } else { v };

        // Anchor the per-tick counter to the recording: it feeds the
        // cave-drip 8-turn cadence AND the cave carpet-tail rand
        // perturbation (World::tick). Retail resets it at level load,
        // so the local player's Turn is its exact value. The carpet's
        // byte[1]&8 one-shot (EF:59616) arms the tail skip.
        self.mc2_turn = ply.turn.max(0) as u32;
        self.mc2_carpet_stall = carpet.flags & 0x800 != 0;

        let n = pool.min(st.ents.len());
        let mut active = 0usize;
        let mut bad_rows = 0usize;
        // A record with the disable bit (byte[1] & 4) is a GHOST:
        // retail pushed its slot to the free stack at disable but
        // nothing zeroes the pool bytes, so the stale record persists
        // (and projects) until reallocation overwrites it. Import the
        // record for the projection, but the slot belongs to the free
        // side of the census.
        let ghost = |r: &RetailEntMc2| (r.flags >> 8) & 4 != 0;
        for slot in 1..n {
            let r = &st.ents[slot];
            if r.class3f == 0 || slot == human_slot as usize {
                self.g.ent[slot] = Ent::default();
                continue;
            }
            if !ghost(r) {
                active += 1;
            }
            // Behavior row: `ptr_a0` points into `str_D7BD6[]`;
            // retail's own load fixup is `(ptr − base160)/34 + 59`
            // (Level.cpp:1255-57; base160 = the saved `&str_D7BD6[59]`).
            let row156 = {
                let d = r.ptr_a0.wrapping_sub(st.base160) as i32;
                let steps = d / 34;
                if d % 34 == 0 && (-59..98).contains(&steps) {
                    (steps + 59) as u8
                } else {
                    bad_rows += 1;
                    59
                }
            };
            self.g.ent[slot] = import_ent_mc2(r, slot as u16, row156, &tr);
        }
        for slot in n..pool {
            self.g.ent[slot] = Ent::default();
        }

        // Tile lists: MC2 maintains its chains incrementally, but the
        // per-tile head array (`mapEntityIndex_15B4E0`) lives OUTSIDE
        // `D41A0_0` (a separate SMAP global the recording does not
        // carry), so the chains rebuild here in ascending slot order —
        // retail's historical insertion order is unrecoverable, and
        // any chain-order-sensitive tie surfaces as a family.
        for h in self.g.map_entity.iter_mut() {
            *h = 0;
        }
        for slot in 1..n {
            let e = &self.g.ent[slot];
            // Ghosts never link: retail unlinks at disable — the
            // record's link bit is stale bytes. A linked ghost whose
            // slot is later reallocated leaves a dangling chain
            // pointer (a tile-chain CYCLE once the new occupant
            // relinks on the same tile — the pair-9074 OOM).
            if e.class64 != 0 && st.ents[slot].flags & 4 != 0 && !ghost(&st.ents[slot]) {
                let (x, y, z) = (e.x, e.y, e.z);
                self.g.link(slot, x, y, z);
            }
        }

        // Free stack: retail pops the FREE stack first and recycle
        // victims only when it is exhausted (`NewEvent_4A050`) — the
        // opposite priority of MC1. The port pops from the Vec's end,
        // so the free stack goes on top (recycle below), preserving
        // the recorded allocation order. Fallback = retail's own load
        // rebuild (`sub_49F90`): descending slot scan, lowest free
        // slot ends on top.
        // Ghost slots are NOT in the recorded stacks: retail's
        // disable leaves the record and the slot in limbo until the
        // NEXT frame's remove pass unlinks and pushes it (measured:
        // the t=1 snapshot's stack is exactly the ghost count short,
        // and the reused slots pop highest-first = an ascending push
        // scan). Mirror it — recorded stack first, ghosts appended
        // ascending so the LIFO pop takes them first, descending.
        let ghost_slots: Vec<u16> = (1..n as u16)
            .filter(|&s| {
                let e = &self.g.ent[s as usize];
                s != human_slot && e.class64 != 0 && e.flags & 0x400 != 0
            })
            .collect();
        let live: Vec<u16> = st
            .recycle_stack
            .iter()
            .chain(st.free_stack.iter())
            .copied()
            .filter(|&s| {
                (s as usize) < pool && s != human_slot && self.g.ent[s as usize].class64 == 0
            })
            .collect();
        let scan_free = pool - 1 - active - 1 - ghost_slots.len();
        let stack_fallback = if live.len() == scan_free {
            self.g.free = live;
            None
        } else {
            let got = live.len();
            self.g.free = (1..pool as u16)
                .rev()
                .filter(|&s| s != human_slot && self.g.ent[s as usize].class64 == 0)
                .collect();
            Some((got, scan_free))
        };
        self.g.free.extend(ghost_slots);

        // Globals in the closure.
        self.g.rand = st.rand;
        self.g.mc2_spawn_ord.0[..29].copy_from_slice(&st.spawn_ord);
        // Outside the closure (same as MC1: retile LCG + the volcano
        // singleton registers live in statics retail never saves).
        self.g.pseudo = 0;
        self.g.erupting = 0;
        self.g.plume = 0;

        // StageVar held bindings: retail keeps `StageVar1_0x48_72` +
        // the `word_0x4A_74` timer ON the entity; the port's side-vec
        // rebuilds from them.
        //
        // The live var table's RUNTIME lanes overlay from the recorded
        // rows @0x365F4 each pair (kind/flags/chain/cadence, and the
        // kind-6/7 param word) — without this the port's table carried
        // its own FIRED/cadence mutations across pairs (the suite's
        // self-drift). Loader-DERIVED fields (hold_word/subtypes/
        // watch_template) stay from the level build: the &2-clear
        // watch payload can be a bound-entity guest pointer in the
        // raw row (EF:4740), which the sv1 lanes already reconstruct.
        for (i, raw) in st.stagevars.iter().enumerate() {
            let Some(v) = self.mc2_stagevars.get_mut(i) else {
                break;
            };
            v.kind = raw[0] & 0xF;
            v.flags = raw[1];
            v.chain = raw[2];
            v.cadence = raw[3];
            if matches!(v.kind, 6 | 7) {
                v.param = u16::from_le_bytes([raw[4], raw[5]]);
            }
        }
        self.mc2_sv_held.clear();
        self.mc2_sv_deferred.clear();
        for slot in 1..n {
            let r = &st.ents[slot];
            if r.class3f != 0 && slot != human_slot as usize && r.sv1 > 0 && !ghost(r) {
                self.mc2_sv_held.push(crate::mc2::stagevars::Mc2Held {
                    ent: slot as u16,
                    slot: r.sv1 as u8,
                    timer: r.sv_timer,
                });
            }
        }

        // Per-player columns: pool wizard slots + WANTED timers.
        // MC2's wanted table keys on the wizard's ENTITY slot
        // (`mc2_wanted`, hash-quiet while empty); MC1's per-player
        // `rival_wanted` array stays zero.
        self.g.mc2_wanted.0.clear();
        self.g.mc2_allied.0.clear();
        self.g.mc2_aura_claim.0.clear();
        self.g.mc2_debuffs = Default::default();
        for i in 0..8 {
            let p = st.players.get(i);
            self.g.rival_ents[i] = match p {
                Some(p) if i != local => tr(p.play_index),
                _ => 0,
            };
            self.g.rival_wanted[i] = 0;
            if let Some(p) = p {
                if i != local && p.play_index != 0 && p.wanted > 0 {
                    self.g.mc2_wanted.0.insert(p.play_index, p.wanted as u16);
                }
            }
        }
        self.g.rival_ents[local] = 0;
        self.g.player_aggro = ply.wanted;
        self.g.player_danger = carpet.f36 as i16;
        self.g.player_mail = carpet.mail.map(|(a, s)| (a.max(0) as u32, tr(s)));
        self.g.player_invisible = carpet.flags & 0x20 != 0;
        self.g.mc2_player_drain.0 = 0;

        // The human column. MC2 hands are DIRECT spell indices
        // (SpellIndexLeft/Right; −1 = empty) — no acquisition-list
        // indirection like MC1.
        let hand = |raw: i16| {
            (0..SPELL_COUNT as i16)
                .contains(&raw)
                .then_some(SpellId(raw as u8))
        };
        self.player = Player {
            mana: carpet.mana.max(0) as u32,
            mana_max: carpet.mana_max.max(0) as u32,
            // The pending regen/debit delta (@0x88, the applied-then-
            // recomputed pipeline both engines share) — the tick
            // applies it before recomputing, same seed law as MC1's
            // f132 import. KNOWN RESIDUAL (~232 pairs): a freshly
            // stamped −cost pends TWO recorded frames (t=0/1 both
            // show d88=−100 with the manifestation timer FROZEN
            // between them — the recorder's mid-frame window catches
            // the pre-apply state), so a single-pair import cannot
            // tell "stamped, applies next tick" from "stamped after
            // the apply, holds a frame"; an f2e-first-tick clamp was
            // tried 2026-07-30 and bought exactly one pair.
            mana_delta: carpet.d88,
            life: carpet.life,
            state: LifeState::Alive,
            left: hand(ply.hand_left),
            right: hand(ply.hand_right),
            grace: ply.invuln.max(0) as u16,
            killer: tr(carpet.f24 as u16),
            fall_speed: carpet.f2c,
            invisible: carpet.flags & 0x20 != 0,
            ..Player::default()
        };

        // The human's str_611 spellbook: manifestation slots, XP,
        // and tier state live in the per-player block and mutate at
        // runtime (casts, kills, releveling) — the world-build
        // seeding is cross-pair state, so rebuild from the closure.
        // Without this the cast machinery ticks whatever slots the
        // level build assigned, not the imported manifestations.
        let book_hand = |raw: i16| {
            if (0..26).contains(&raw) {
                raw as i8
            } else {
                -1
            }
        };
        self.mc2_book = crate::mc2::cast::Mc2Spellbook {
            ent: ply.spell_ent,
            xp_vol: ply.xp_vol,
            xp_bank: ply.xp_bank,
            levels: ply.levels,
            sel: ply.sel,
            left: book_hand(ply.hand_left),
            right: book_hand(ply.hand_right),
            ring: ply.ring,
        };

        // Cross-pair latches, same wipe as the MC1 arm.
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.pending_teleport = None;
        self.pending_respawn = None;
        self.pending_restart = false;
        self.duel = None;
        self.won = false;
        self.completed = false;
        self.win_streak = 0;
        self.prev_fire = (false, false);
        self.accel_veto = (false, false);
        self.rival_deaths.clear();
        self.notification = None;
        self.kill_tally = [[0; 8]; 8];
        self.entities_dirty = true;

        Ok(ImportReport {
            active,
            human_slot,
            behavior_base: st.base160,
            bad_rows,
            stack_fallback,
        })
    }

    /// Project this world into the recorder's MC2 obs schema — the
    /// twin of [`World::obs_project_mc1`]. Port fields translate back
    /// through the SEMANTIC alias table (mc2/mobs.rs), the reverse of
    /// `import_ent_mc2`.
    pub fn obs_project_mc2(&self, pin: &PinnedMc2) -> ObsMc2 {
        let untr = |v: u16| if v == PLAYER_TARGET { pin.slot } else { v };
        let held: std::collections::BTreeMap<u16, &crate::mc2::stagevars::Mc2Held> =
            self.mc2_sv_held.iter().map(|h| (h.ent, h)).collect();
        let mut entities: Vec<EntObsMc2> = Vec::new();
        for slot in 1..self.g.ent.len() as u16 {
            if slot == pin.slot {
                entities.push(self.synth_carpet_obs_mc2(pin));
                continue;
            }
            let e = &self.g.ent[slot as usize];
            if e.class64 == 0 {
                continue;
            }
            let mut row = EntObsMc2 {
                slot,
                class: e.class64,
                model: e.model65,
                life: e.act_life,
                max_life: e.max_life as i32,
                x: e.x as f64 / 256.0,
                y: e.y as f64 / 256.0,
                z: e.z,
                heading: e.f30 as i16,
                pitch: e.f32 as i16,
                applied_yaw: e.f78 as i16,
                applied_pitch: e.f80 as i16,
                speed: e.f126,
                mana: e.f140,
                mana_max: e.f136,
                // Retail's parentId @0x28 is nonzero ONLY on class-15
                // manifestations (mc2l0 census); every other class
                // carries its owner in the id lane @0x1A, which is
                // fused into id24 and not part of the obs schema. A
                // DETACHED manifestation (spell steal) has id24 ==
                // slot and projects 0 like everything else.
                owner: if e.class64 == 15 && e.id24 != slot {
                    untr(e.id24)
                } else {
                    0
                },
                action: e.tick70,
                sv1: held.get(&slot).map_or(0, |h| h.slot),
                sv2: if e.class64 == 5 { e.site_z as u8 } else { 0 },
                player_ent_idx: untr(e.f144),
                rand: e.rand as u16,
            };
            // Class-15 reverse map (`import_ent_mc2`'s override): the
            // obs heading lane (@0x1C) and max_life lane (@0x04) are
            // dead 0 on retail manifestations — f30 carries the
            // payload and max_life the cast cost, which retail keeps
            // in the obs mana_max lane (@0x8C).
            if e.class64 == 15 {
                row.heading = 0;
                row.max_life = 0;
                row.mana_max = e.max_life as i32;
            }
            // Class-10 fires carry their amount in f140 (imported
            // from @0x2A); retail's @0x90 mana lane is dead 0.
            if e.class64 == 10 && matches!(e.model65, 0 | 6) {
                row.mana = 0;
            }
            entities.push(row);
        }
        let spell_i16 = |s: Option<SpellId>| s.map(|s| s.0 as i16);
        let players: Vec<PlayerMc2> = (0..pin.player_count)
            .map(|i| {
                let localp = i == pin.local;
                PlayerMc2 {
                    index: i,
                    is_ai: !localp,
                    play_index: if localp {
                        pin.slot
                    } else {
                        untr(self.g.rival_ents[i as usize])
                    },
                    turn: 0,
                    name: String::new(),
                    // Echoed, not derived — see [`PinnedMc2::castles`].
                    castle: pin.castles[i as usize & 7],
                    hand_left: if localp {
                        spell_i16(self.player.left)
                    } else {
                        None
                    },
                    hand_right: if localp {
                        spell_i16(self.player.right)
                    } else {
                        None
                    },
                    flight: FlightMc2 {
                        cmd_speed: if localp { pin.pose.speed } else { 0 },
                        v16: 0,
                    },
                }
            })
            .collect();
        let control: Vec<ControlMc2> = (0..pin.player_count).map(zero_control_mc2).collect();
        let player = players.get(pin.local as usize).map(|p| PlayerJoinMc2 {
            carpet_slot: pin.slot,
            name: String::new(),
            is_ai: false,
            turn: 0,
            life: self.player.life,
            max_life: PLAYER_LIFE_MAX,
            mana: self.player.mana as i32,
            mana_max: self.player.mana_max as i32,
            x: pin.pose.x as f64 / 256.0,
            y: pin.pose.y as f64 / 256.0,
            z: pin.pose.z,
            heading: pin.pose.heading as i16,
            pitch: pin.pose.pitch as i16,
            applied_yaw: 0,
            applied_pitch: 0,
            speed: pin.pose.speed,
            hand_left: p.hand_left,
            hand_right: p.hand_right,
            castle: p.castle,
            flight: p.flight.clone(),
            control: Some(zero_control_mc2(pin.local)),
        });
        ObsMc2 {
            rng: self.g.rand,
            n_active: entities.len() as u32,
            local_player: pin.local,
            player_count: pin.player_count,
            players,
            control,
            player,
            entities,
        }
    }

    /// The synthesized MC2 human-carpet obs row.
    fn synth_carpet_obs_mc2(&self, pin: &PinnedMc2) -> EntObsMc2 {
        EntObsMc2 {
            slot: pin.slot,
            class: 3,
            model: 0,
            life: self.player.life,
            max_life: PLAYER_LIFE_MAX,
            x: pin.pose.x as f64 / 256.0,
            y: pin.pose.y as f64 / 256.0,
            z: pin.pose.z,
            heading: pin.pose.heading as i16,
            pitch: pin.pose.pitch as i16,
            applied_yaw: 0,
            applied_pitch: 0,
            speed: pin.pose.speed,
            mana: self.player.mana as i32,
            mana_max: self.player.mana_max as i32,
            owner: pin.slot,
            action: 0,
            sv1: 0,
            sv2: 0,
            player_ent_idx: pin.slot,
            rand: 0,
        }
    }

    /// The synthesized human-carpet obs row: pose fields from the pin,
    /// life/mana from the player column. `flags`/`rand`/`tick_byte`
    /// have no port-side counterpart outside the pool — the comparator
    /// treats the pinned slot specially.
    fn synth_carpet_obs(&self, pin: &PinnedMc1) -> EntObsMc1 {
        EntObsMc1 {
            slot: pin.slot,
            class: 3,
            model: 0,
            sclass: match self.player.state {
                LifeState::Alive => 0,
                LifeState::Falling => 2,
                LifeState::Dead => 3,
            },
            smodel: 0,
            flags: 0,
            id: pin.slot,
            life: self.player.life,
            max_life: PLAYER_LIFE_MAX as u32,
            x: pin.pose.x as f64 / 256.0,
            y: pin.pose.y as f64 / 256.0,
            z: pin.pose.z,
            heading: pin.pose.heading,
            pitch: pin.pose.pitch,
            target_yaw: pin.pose.heading,
            speed: pin.pose.speed,
            mana: self.player.mana,
            mana_max: self.player.mana_max,
            chase: 0,
            owner_ptr: 0,
            tick_byte: 0,
            rand: 0,
        }
    }
}

fn zero_control_mc2(player: u16) -> ControlMc2 {
    ControlMc2 {
        player,
        opcode: 0,
        param1: 0,
        param2: 0,
        aim_yaw: 0,
        aim_pitch: 0,
        buttons: 0,
    }
}

/// One retail MC2 pool record → the port's `Ent`, per the SEMANTIC
/// alias table (mc2/mobs.rs doc header) — MC2 offsets do NOT line up
/// with the port's MC1-numbered field names. Entity-reference fields
/// go through the human-slot translation; the link bit (byte[0] & 4)
/// is cleared for the caller's relink pass.
///
/// Flag translation covers the bits the port reads (mobs.rs):
/// byte0&8 collidable and byte0&4 link keep their positions;
/// byte0&0x20 invisible → 0x20; byte0&2 whoosh-played → bit 25;
/// byte1&4 disabled → 0x400 (reap); byte1&8 forced-stop → bit 26;
/// byte2&4 blocked → bit 27; byte2&0x10 no-corpse → bit 28;
/// byte2&0x20 forced-claim → bit 29. Unmapped retail bits drop (the
/// obs channel does not carry flags; only behavior reads them).
fn import_ent_mc2(r: &RetailEntMc2, slot: u16, row156: u8, tr: &dyn Fn(u16) -> u16) -> Ent {
    let (b0, b1, b2) = (
        r.flags & 0xFF,
        (r.flags >> 8) & 0xFF,
        (r.flags >> 16) & 0xFF,
    );
    let mut flags = 0u32;
    if b0 & 8 != 0 {
        flags |= 8;
    }
    if b0 & 0x20 != 0 {
        flags |= 0x20;
    }
    if b0 & 2 != 0 {
        // Retail's byte0&2 is the generic one-shot-done latch. The
        // port keeps it POSITIONAL (bit 1 — the fire/explosion
        // activation gates) and ALSO mirrors it to bit 25 (the
        // whoosh-played home). Importing only the mirror re-ran
        // every active fire's activation block (area damage +
        // flicker draw + scorch) on each pair.
        flags |= (1 << 25) | 2;
    }
    if b1 & 4 != 0 {
        flags |= 0x400;
    }
    if b1 & 8 != 0 {
        flags |= 1 << 26;
    }
    if b2 & 4 != 0 {
        flags |= 1 << 27;
    }
    if b2 & 0x10 != 0 {
        flags |= 1 << 28;
    }
    if b2 & 0x20 != 0 {
        flags |= 1 << 29;
    }
    // The port routes MC2-native projectiles by the F_MC2PROJ marker
    // its ctors set (bit 29, with the collidable bit cleared —
    // mc2/proj.rs); retail has no such marker, so stamp every class-9
    // projectile except the (9,13) arrow (state-keyed, no marker).
    // Without it an imported projectile falls into the MC1 fallback
    // arm and indexes MC1's 31-row table with an MC2 row.
    if r.class3f == 9 && r.model40 != 13 {
        flags = (flags & !8) | crate::mc2::proj::F_MC2PROJ;
    }
    let mut e = Ent {
        rand: r.rand as u32,
        max_life: r.max_life.max(0) as u32,
        act_life: r.life,
        flags,
        next20: 0,
        prev22: 0,
        // The port fuses retail's own-id (`id_0x1A`) and
        // `parentId_0x28` into id24. @0x1A is the LIVE owner-or-self
        // lane (mc2l0 census): the caster on projectiles, the owning
        // wizard on castles/balloons/charmed creatures, the watch
        // target on class-11 triggers, self everywhere else. @0x28
        // is nonzero only on class-15 manifestations.
        id24: if r.owner28 != 0 {
            tr(r.owner28)
        } else if r.f1a != 0 {
            tr(r.f1a)
        } else {
            slot
        },
        // The scratch quartet is DUAL-HOMED per class (mc2/ handler
        // survey): creatures keep the charm/armed timer (@0x2E) in
        // f26 and the font-type byte (@0x3D) in f46; effects keep
        // dword @0x10 scratch in f26 and the z-velocity (@0x2E) in
        // f46. f28 is a port artifact (the cross-column damage
        // contract; retail's @0x38 mask is write-only in MC2). m27's
        // link length (@0x36) rides f56; everything else keeps @0x38
        // there. Class-15 manifestations override eight of these
        // below (the cast.rs field map).
        f26: match (r.class3f, r.model40) {
            // The m0 worm/hydra keeps its BOB VELOCITY in @0x10
            // (multipart ctor seed + sub_1F040's home); importing
            // the charm lane left the bob dead — the whole chain
            // sank instead of undulating (mc2l4 corpus, slot 2).
            (5, 0) => r.scratch10 as i16,
            (5, _) => r.f2e,
            _ => r.scratch10 as i16,
        },
        f28: r.b38 as u8 as u16,
        f30: r.yaw as u16,
        f32: r.pitch as u16,
        f34: r.roll as u16,
        f36: 0,
        f38: tr(r.f24 as u16),
        f40: tr(r.f26 as u16),
        f44: r.f2a,
        f46: if r.class3f == 5 { r.b3d as i16 } else { r.f2e },
        f50: r.f30 as i16,
        f52: tr(r.f32),
        f54: tr(r.f34),
        f56: if matches!(r.class3f, 2 | 10) {
            r.b38 as u8 as u16
        } else {
            r.f36
        },
        f58: r.b39 as i16,
        // The (3,2) castle's BUILD SUB-STATE lives in @0x2E
        // (word_0x2E_46 → f59, docs/traces/mc2-castle-builder.md §2);
        // @0x3A is dead for castles, and importing its 0 parked every
        // castle in the level-up state — one phantom upgrade + one
        // phantom (10,42) painter per pair, z frozen for the tick
        // (the MC2 twin of MC1's phantom-upgrade family).
        f59: if r.class3f == 3 && r.model40 == 2 {
            r.f2e as u8
        } else {
            r.b3a as u8
        },
        f63: r.phase3e,
        class64: r.class3f,
        model65: r.model40,
        f66: r.b41 as u8,
        f67: r.b42 as u8,
        f68: r.b43 as u8,
        f69: r.b44 as u8,
        tick70: r.action45,
        f71: r.b46 as u8,
        x: r.x,
        y: r.y,
        z: r.z,
        f78: r.ayaw as u16,
        f80: r.apitch as u16,
        f82: r.aroll as u16,
        f84: r.afov as u16,
        type86: r.f5a as u16,
        frame88: r.b5c as u8,
        frames89: r.b5d as u8,
        mail: r.mail.map(|(a, s)| (a.max(0) as u32, tr(s))),
        f126: r.speed,
        f128: r.min_speed,
        f130: r.max_speed,
        f136: r.mana_max,
        f140: r.mana,
        f144: tr(r.player_ent),
        f146: tr(r.target96),
        row156,
        thing_slot: 0,
        dest_x: r.dest_x,
        dest_y: r.dest_y,
        // Creatures keep the StageVar KIND in the port's site_z (the
        // relocated `StageVar2_0x49_73`); other classes carry the
        // destination z there.
        site_z: if r.class3f == 5 {
            r.sv2 as i16
        } else {
            r.dest_z
        },
    };
    // Class-15 manifestations keep the cast machinery in different
    // homes than the uniform alias table (cast.rs module doc):
    // armed timer @0x2E → f26, duration/mana divisor @0x30 → f28,
    // sub-spell payload @0x2A → f30 (the yaw lane is dead 0),
    // pending tier+1 @0x2C → f44, cooldown @0x36 → f54, cadence
    // flag @0x3B → f59, upkeep regen @0x88 → f136, full cast cost
    // @0x8C → max_life (the @0x04 lane is dead 0). @0x90 per-tick
    // mana → f140 and @0x46 tier → f71 coincide with the uniform
    // map. The displaced uniform homes are dead for class 15.
    if r.class3f == 15 {
        e.f26 = r.f2e;
        e.f28 = r.f30;
        e.f30 = r.f2a;
        e.f44 = r.f2c as u16;
        e.f54 = r.f36;
        e.f59 = r.b3b as u8;
        e.f136 = r.d88;
        e.max_life = r.mana_max.max(0) as u32;
        e.f46 = 0;
        e.f50 = 0;
        e.f56 = 0;
    }
    // Class-10 fires keep the area amount in `subSpellIndex_0x2A`
    // (→ the port's f140 amount home, sub_30D50's sub_10C80 call /
    // sub_31760) and the z flicker/lift in `word_0x2C_44` (→ f44);
    // the @0x90 mana lane is dead 0 on them (reverse-mapped in
    // `obs_project_mc2`).
    if r.class3f == 10 && matches!(r.model40, 0 | 6) {
        e.f140 = r.f2a as i32;
        e.f44 = r.f2c as u16;
    }
    e
}

fn zero_control(player: u16) -> ControlMc1 {
    ControlMc1 {
        player,
        opcode: 0,
        param1: 0,
        param2: 0,
        aim_yaw: 0,
        aim_pitch: 0,
        move_fire: 0,
        thrust: false,
        decel: false,
        strafe_left: false,
        strafe_right: false,
        fire_left: false,
        fire_right: false,
    }
}

/// One retail pool record → the port's `Ent`, with human-slot id
/// translation applied to every entity-reference field. The link bit
/// (flags & 4) is cleared — the caller relinks through `Gen::link` so
/// the tile lists stay consistent.
fn import_ent(r: &RetailEntMc1, row156: u8, tr: &dyn Fn(u16) -> u16) -> Ent {
    // The castle (3,2) keeps its macro-state in retail's JOB byte +70
    // (4 = settled, 5 = transforming, 6 = full build — sub_46DB0
    // :55978 / sub_46F10 :56043) with the transform sub-state in +48;
    // the port's `castle_tick` fuses both into f59 (0 = level-up
    // commit, 1/6 = painter/leveler waits, 2/3/5 = finish/repaint/
    // handoff, 4 = settled). Retail's +59 byte is dead for castles —
    // importing it verbatim parked every settled castle in f59 = 0 and
    // re-upgraded it one level per tick (the phantom-upgrade family,
    // docs/CONFORMANCE-FINDINGS.md entry 3). Retail's pure-wait +48
    // values 1 and 4 both land on the port's painter-wait state 1.
    let f59 = if r.class64 == 3 && r.model65 == 2 {
        match r.f70 {
            4 => 4,
            5 => match r.f48 {
                1 | 4 => 1,
                s => (s as u8).min(6),
            },
            6 => 0,
            _ => 4,
        }
    } else {
        r.f59
    };
    Ent {
        rand: r.rand,
        max_life: r.max_life,
        act_life: r.act_life,
        flags: r.flags & !4,
        next20: 0,
        prev22: 0,
        id24: tr(r.id24),
        f38: tr(r.f38),
        f40: tr(r.f40),
        f46: r.f46,
        f50: r.f50,
        f68: r.f68,
        f69: r.f69,
        mail: r.mail.map(|(a, s)| (a, tr(s))),
        f144: tr(r.f144),
        // The port keeps a manifestation's burst/refire counter in f26
        // (retail: +48; retail's +26 is the SPELL LEVEL there).
        f26: if r.class64 == 12 { r.f48 as i16 } else { r.f26 },
        f28: r.f28,
        f30: r.f30,
        f32: r.f32,
        f44: r.f44,
        f34: r.f34,
        f36: r.f36,
        f52: tr(r.f52),
        f54: tr(r.f54),
        f56: r.f56,
        f58: r.f58 as i16,
        f59,
        f63: r.f63,
        class64: r.class64,
        model65: r.model65,
        f66: r.f66,
        f67: r.f67,
        tick70: r.f70,
        f71: r.f71,
        x: r.x,
        y: r.y,
        z: r.z,
        f78: r.f78,
        f80: r.f80,
        f82: r.f82,
        f84: r.f84,
        type86: r.type86,
        frame88: r.frame88,
        frames89: r.frames89,
        f126: r.f126,
        f128: r.f128,
        f130: r.f130,
        f136: r.f136,
        f140: r.f140,
        f146: tr(r.f146),
        row156,
        thing_slot: 0,
        dest_x: r.dest_x,
        dest_y: r.dest_y,
        site_z: r.site_z,
    }
}
