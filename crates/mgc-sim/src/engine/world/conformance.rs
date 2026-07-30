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
    ControlMc1, EntObsMc1, FlightMc1, ObsMc1, PlayerJoinMc1, RetailEntMc1, RetailMc1, WizardMc1,
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
        if live.len() == scan_free {
            self.g.free = live;
        } else {
            self.g.free = (1..pool as u16)
                .rev()
                .filter(|&s| self.g.ent[s as usize].class64 == 0 && s != human_slot)
                .collect();
        }

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
            mana_delta: carpet.f132 as i32,
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
