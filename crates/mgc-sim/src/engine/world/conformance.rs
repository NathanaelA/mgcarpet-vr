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
use crate::flight::{Mc1State, Mc2Ext, Mc2Row};
use crate::mc1::mobs::PLAYER_TARGET;
use crate::mc1::spells::{SPELL_COUNT, SpellId};
use mgc_formats::mgcr::{
    ControlMc1, ControlMc2, EntObsMc1, EntObsMc2, FlightMc1, FlightMc2, ObsMc1, ObsMc2,
    PlayerJoinMc1, PlayerJoinMc2, PlayerMc2, RetailEntMc1, RetailEntMc2, RetailMc1, RetailMc2,
    RetailPlayerMc2, RetailWizardMc1, WizardMc1,
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

/// One entity's ungraded raw lanes — see [`World::raw_shadow_mc1`].
///
/// EVERY per-entity field the port models and `EntObsMc1` does not
/// carry. The four the lane started with (`+70`/`+71`/`+58`/`+44`)
/// each paid for themselves, and the rest are the same blind spot: the
/// recording holds them, [`World::retail_import_mc1`] restores them
/// every pair, and the graded diff can never see a WRITE go wrong. The
/// cost of widening is a longer report, and the report is per-(class,
/// model, field) — noise stays legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawShadowMc1 {
    pub slot: u16,
    pub class: u8,
    pub model: u8,
    /// `+70` — the handler state byte.
    pub f70: u8,
    /// `+71` — the burst / charge register (the kraken's bolt count).
    pub f71: u8,
    /// `+58`.
    pub f58: i16,
    /// `+44` — damage/potency. Ungraded like the rest, and the lane a
    /// spawn ctor writes ONCE: an effect born with the wrong potency
    /// reads clean in pair mode forever (the importer restores it
    /// every pair) and only bites a free run, as the victims' life
    /// diverging one tick after the broadcast.
    pub f44: u16,
    /// `+20`/`+22` — the tile-list links. Rebuilt every tick from the
    /// walk order, so a mismatch is a CHAIN-ORDER divergence: the
    /// membership laws this campaign keeps finding (tick-top chain,
    /// ball list, bucket[0]) all land here first.
    pub next20: u16,
    pub prev22: u16,
    /// `+26` — the generic counter (crater ring, wall run length,
    /// trigger rearm, acquire latch).
    pub f26: i16,
    pub f28: u16,
    pub f36: u16,
    /// `+38`/`+40` — killer-id and attacker latches.
    pub f38: u16,
    pub f40: u16,
    /// `+46` — vertical velocity.
    pub f46: i16,
    /// `+50` — the damage-response countdown.
    pub f50: i16,
    /// `+52`/`+54` — the multipart chain links (head / tail), and
    /// `+56` the follow distance. Named in the ledger as an obs blind
    /// spot since mc1l2.
    pub f52: u16,
    pub f54: u16,
    pub f56: u16,
    /// `+59` — the awake re-probe delay.
    pub f59: u8,
    /// `+68`/`+69` — the class/model this record detonates into.
    pub f68: u8,
    pub f69: u8,
    /// `+78`..`+84` — the sprite half-height and the collision extents.
    pub f78: u16,
    pub f80: u16,
    pub f82: u16,
    pub f84: u16,
    /// `+86`/`+88`/`+89` — sprite stats row, animation frame, frame count.
    pub type86: u16,
    pub frame88: u8,
    pub frames89: u8,
    /// `+90..+126` — the six damage mailboxes {amount, source}.
    pub mail: [(u32, u16); 6],
    /// `+128`/`+130` — target speed and acceleration.
    pub f128: i16,
    pub f130: i16,
    /// `+144` — the mana-ball owner.
    pub f144: u16,
    /// `+150`/`+152`/`+154` — teleport destination and build-site z.
    pub dest_x: u16,
    pub dest_y: u16,
    pub site_z: i16,
}

/// One wizard's ungraded wizext/brain lanes — see
/// [`World::wiz_shadow_mc1`]. The Type_160 counterpart of
/// [`RawShadowMc1`]: the recording carries the whole wizard slice,
/// [`World::retail_import_mc1`] restores it every pair, and the graded
/// diff sees none of it — a knock impulse or a brain register written
/// wrong reads CLEAN in pair mode forever and only bites a free run,
/// as the carpet's graded x/y/target_yaw parting ticks later.
///
/// Lane names match [`RetailWizardMc1`]'s fields. Values are already
/// in retail convention except where the doc on
/// `Rival::wiz_shadow_lanes` says otherwise (`ai_state` canonical
/// byte, `poverty`/`war` as 0/1). `ent` is [`PLAYER_TARGET`] for the
/// human (wiz 0).
#[derive(Debug, Clone)]
pub struct WizShadowMc1 {
    pub wiz: u8,
    pub ent: u16,
    pub scalars: Vec<(&'static str, i64)>,
    pub arrays: Vec<(&'static str, Vec<i64>)>,
}

/// Collapse a retail `+415` brain byte onto the byte the port's
/// [`WizShadowMc1::scalars`] `ai_state` lane reports — the cut states
/// (2/4/5/10) read back as Fresh's 0, exactly as retail's own dispatch
/// treats them.
pub fn norm_retail_ai_state_mc1(v: u8) -> i64 {
    crate::mc1::rivals::AiState::from_retail(v).to_retail() as i64
}

/// One MC2 wizard's ungraded player-block/brain lanes — see
/// [`World::wiz_shadow_mc2`]. The MC2 twin of [`WizShadowMc1`], and
/// the arm that was MISSING: the recording carries the whole 2124-byte
/// player block per wizard, [`World::retail_import_mc2`] restores it
/// every pair, and neither the graded diff nor the raw entity shadow
/// ever looked at it — so the entire MC2 rival brain (state, burst,
/// cooldowns, hate ledger, weave/steer FSMs, the spell book) was
/// UNGRADED, and the shadow report said so in words rather than
/// numbers.
///
/// Lane names match [`mgc_formats::mgcr::RetailPlayerMc2`]'s fields;
/// conventions are documented on `Mc2Rival::wiz_shadow_lanes`. `ent`
/// is [`PLAYER_TARGET`] for the human (wiz 0).
#[derive(Debug, Clone)]
pub struct WizShadowMc2 {
    pub wiz: u8,
    pub ent: u16,
    pub scalars: Vec<(&'static str, i64)>,
    pub arrays: Vec<(&'static str, Vec<i64>)>,
}

/// Collapse a retail `byte_0x1C1_449` brain byte onto the byte the
/// port's [`WizShadowMc2::scalars`] `ai_state` lane reports — the
/// APPROACH arm 4 folds onto the casting arm 6 (the port fuses them),
/// and the `_nmemneed` stubs 2/5/10 plus 15+ read back as Fresh's 0,
/// exactly as retail's own dispatch treats them.
pub fn norm_retail_ai_state_mc2(v: u8) -> i64 {
    crate::mc2::rivals::Mc2AiState::from_retail(v).to_retail() as i64
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

/// An opaque snapshot of the level's authored THING record table
/// ([`World::thing_table_clone`]).
pub struct ThingTable(Vec<crate::engine::features::Rec>);

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

    /// The post-build THING record table, cloned — the twin of
    /// [`World::planes_clone`] for the level's authored records. A
    /// one-shot disposition ZEROES the records it releases
    /// (`sub_4A1E0(id, 1)`), and that consumption is NOT part of the
    /// `D41A0_0` closure the recording captures, so it cannot be
    /// re-imported per pair: a single mis-timed trip anywhere in a run
    /// silently disarms the disposition for every later pair. The
    /// runner captures the table right after the level build and
    /// re-imprints it via [`World::restore_thing_table`].
    pub fn thing_table_clone(&self) -> ThingTable {
        ThingTable(self.table.clone())
    }

    /// Re-imprint the post-build THING record table
    /// ([`World::thing_table_clone`]).
    pub fn restore_thing_table(&mut self, table: &ThingTable) {
        self.table.clear();
        self.table.extend_from_slice(&table.0);
    }

    /// Re-imprint the pristine terrain planes (the map file's blocks
    /// are not part of the master-struct closure; craters and retile
    /// do not survive a retail import).
    pub fn restore_planes(&mut self, planes: &Planes) {
        self.measured_terrain = false;
        self.g.t = Planes {
            height: planes.height.clone(),
            tile_type: planes.tile_type.clone(),
            shading: planes.shading.clone(),
            angle: planes.angle.clone(),
            ceiling: planes.ceiling.clone(),
        };
        self.terrain_dirty = true;
    }

    /// Overwrite the height and tile-type planes — plus, on MC2 cave
    /// takes, the CEILING (cave carves edit it mid-level and the cave
    /// clamp laws read it), and the ANGLE plane when the take
    /// measures it (the sub_11760 water probe, the scorch gates and
    /// the castle protection bits all read it live; the mid-paint
    /// walkability/protection dance is not reconstructible from the
    /// pool closure — mc1l0 pair 565) — with MEASURED images (the
    /// recording's format-2 terrain channel, accumulated to the
    /// pair's start tick). The primary terrain source when present,
    /// layered over [`World::restore_planes`]'s pristine base so
    /// shading (and unmeasured planes) keep their level values.
    /// Wrong-size slices are an error, never a partial write; a
    /// measured CEILING on a level that carries no ceiling plane is
    /// dropped rather than installed (see the body).
    pub fn install_measured_terrain(
        &mut self,
        height: &[u8],
        tile_type: &[u8],
        ceiling: Option<&[u8]>,
        angle: Option<&[u8]>,
    ) -> Result<(), String> {
        if height.len() != self.g.t.height.len() || tile_type.len() != self.g.t.tile_type.len() {
            return Err(format!(
                "measured terrain {}+{} cells, want {}+{}",
                height.len(),
                tile_type.len(),
                self.g.t.height.len(),
                self.g.t.tile_type.len()
            ));
        }
        // OFF-CAVE CEILING: the port allocates `ceiling` only on MC2
        // cave levels, and an allocated plane IS the cave signal
        // (`Gen::is_cave`, world.rs's night-shade derive) — so a
        // measured ceiling on a Day/Night take must NOT be installed;
        // it would flip the level into cave mode. The recorder
        // captures the plane on every map type now (retail's painters
        // keep it live off-cave even though only the cave generator
        // seeds it), so this is a lane the capture carries and the
        // port does not model, not a size error. Drop it; see
        // `has_ceiling_plane` for the harness's report of the drop.
        let ceiling = ceiling.filter(|_| !self.g.t.ceiling.is_empty());
        if let Some(c) = ceiling
            && c.len() != self.g.t.ceiling.len()
        {
            return Err(format!(
                "measured ceiling {} cells, want {}",
                c.len(),
                self.g.t.ceiling.len()
            ));
        }
        if let Some(a) = angle
            && a.len() != self.g.t.angle.len()
        {
            return Err(format!(
                "measured angle {} cells, want {}",
                a.len(),
                self.g.t.angle.len()
            ));
        }
        self.g.t.height.copy_from_slice(height);
        self.g.t.tile_type.copy_from_slice(tile_type);
        if let Some(c) = ceiling {
            self.g.t.ceiling.copy_from_slice(c);
        }
        if let Some(a) = angle {
            self.g.t.angle.copy_from_slice(a);
        }
        self.measured_terrain = true;
        self.terrain_dirty = true;
        Ok(())
    }

    /// Does this level carry a CEILING plane at all? MC2 cave levels
    /// do; Day and Night levels do not, and for them the plane's
    /// absence is the cave signal itself. The harness asks so it can
    /// name the measured ceiling that [`Self::install_measured_terrain`]
    /// drops as an UNGRADED LANE rather than leaving the drop silent.
    pub fn has_ceiling_plane(&self) -> bool {
        !self.g.t.ceiling.is_empty()
    }

    /// Seed the cast edge-trigger baseline (the held state of the tick
    /// BEFORE the imported one) so a held button does not re-edge on
    /// every imported pair.
    pub fn set_prev_fire(&mut self, left: bool, right: bool) {
        self.prev_fire = (left, right);
    }

    /// Arm the pose channel's mid-tick ground snapshot: the NEXT tick
    /// copies the height plane when its entity walk reaches the
    /// carpet anchor slot — the phase retail's own carpet mover
    /// probes ground at (:55151/:55103), with every lower-slot
    /// terraform of the tick applied and every higher-slot one still
    /// pending. Neither record endpoint has this image: terrain@N
    /// misses the low-slot digs, terrain@N+1 already carries the
    /// high-slot ones (the mc1l0 t=567 fire family, slots 692-705
    /// over carpet 630).
    pub fn arm_midtick_ground_snapshot(&mut self) {
        self.midtick_ground_armed = true;
        self.midtick_ground = None;
    }

    /// The armed snapshot, if the walk crossed the carpet anchor
    /// (consumes it; disarms a never-fired arm).
    pub fn take_midtick_ground_snapshot(&mut self) -> Option<Vec<u8>> {
        self.midtick_ground_armed = false;
        self.midtick_ground.take()
    }

    /// The engine's own bilinear ground sampler over a bare height
    /// plane (the mid-tick snapshot), engine units.
    pub fn ground_z_on_plane(plane: &[u8], x: u16, y: u16) -> i16 {
        crate::engine::features::Gen::interp_plane(plane, x, y) as i16
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
        self.patches = crate::patches::WorldPatches::RETAIL;
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
        // Seed the pose registers from the closure so the first tick
        // after the import sees the RECORDED carpet as its previous
        // position (retail pass order — the strict jar poll and its
        // kin read the carpet before its slot runs).
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.human_pose_prev = self.human_pose;
        // The wizard pass anchors at the recorded carpet slot (the
        // class-3 dispatch position, sub_45C90) — above the spell
        // tokens, which is the cast-phase law's whole ordering.
        self.mc1_carpet_slot = human_slot;
        // The cast-arm hand bits (+16 & 0x300, :55886-95) and the
        // carpet pose the token fires measure from (retail reads the
        // wizard entity's own fields at the token's walk position =
        // the closure's settled values).
        self.hand_bits = carpet.flags & 0x300;
        self.mc1_cast_pose = crate::engine::world::PlayerPose {
            x: carpet.x,
            y: carpet.y,
            z: carpet.z,
            heading: carpet.f30,
            pitch: carpet.f32,
            speed: carpet.f126,
        };
        // The human's acquisition list (`Type_160+532`) VERBATIM,
        // sentinels included: the entries are a phase-tagged union
        // (alive = pool slot, `<= 0` empty; dead = model, −1 empty —
        // :55517-24) and the dead-window −1s are real state the
        // respawn regrant reads back (:54893-95). The old u16 clamp
        // folded −1 into 0 = fireball's model — the exact collision
        // the union is built around avoiding.
        self.mc1_acq = std::array::from_fn(|i| wiz.spell_list.get(i).copied().unwrap_or(0));
        // The carpet's Type_156 is the canonical `&unk_98F38[7]`
        // (retail's own load-fixup anchor) — derive the table base
        // from it instead of hardcoding a per-build guest address.
        let behavior_base = carpet.model_ptr.wrapping_sub(7 * 32);
        let tr = |v: u16| if v == human_slot { PLAYER_TARGET } else { v };

        let n = pool.min(st.ents.len());
        let mut active = 0usize;
        let mut bad_rows = 0usize;
        // ⭐ SLOT 0 IS IMPORTED TOO, AND IT IS NOT DECORATION. The
        // SCRATCH record is real recorded state (the demolish fake
        // collapse stamps it: mc1l3 carries `x 41472 y 40960 z 352
        // id24 579 flags 0x400 class64 0` from t=417 on), and retail's
        // blind dereferences STEER OFF IT — `sub_1A120` forms
        // `&pool[+146]` with no validity test (:21655) and re-bears
        // off it before the lost test, so a pack survivor handed
        // `+146 = 0` aims at slot 0's coordinates. Skipping it left an
        // isolated fixture world computing 724 where retail records
        // 275. Nothing dispatches it — the walk runs 1..999 and slot 0
        // is class 0, so it joins no chain — and it is excluded from
        // the `active` census below for the same reason.
        for slot in 0..n {
            let r = &st.ents[slot];
            if slot == human_slot as usize {
                self.g.ent[slot] = Ent::default();
                // The carpet record stays class 0 (the pose is the
                // runner's input and the pass anchors at the slot),
                // but its +4 is a LIVE stream: the death scatter
                // spends three draws per jar on the dying wizard's
                // own `*(a1+4)` (:55538-46), never the world LCG.
                // Seed it or the landing throws the wrong offsets and
                // over-draws the graded rng channel by 15.
                self.g.ent[slot].rand = r.rand;
                continue;
            }
            if r.class64 == 0 {
                // A freed slot is not an EMPTY slot: retail's free
                // path clears +64 and pushes the stack — every other
                // byte stays, and the blind tracker steers at
                // whatever the record still holds (ledger §THE
                // PROJECTILE LEDGER + BLIND TRACKER; mc1l0 t=3464-70:
                // bolt 557 tracks reaped slot 534's stale position —
                // a defaulted slot re-aims it at the origin). Import
                // the stale bytes, class 0, not counted active; row
                // 0 stands in for the stale model_ptr (nothing live
                // dereferences a freed row).
                self.g.ent[slot] = import_ent(r, 0, &tr);
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

        // Tile lists: the heads live in the map file, not the struct,
        // but the per-entity next20/prev22 ARE recorded — and chain
        // ORDER is observable: the first-hit probes (sub_11D10's cell
        // walk) resolve ties by it. mc1l0 pair 2371: two balls overlap
        // a grounding third; retail's chain order feeds the walk slot
        // 94 before 500, the old ascending re-link handed it 500
        // (head-insertion reverses slot order), and the merge picked
        // the wrong partner — a phantom (10,39) desync + mana fork.
        // So rebuild each cell chain in the RECORDED order: walk the
        // recorded links from each head (prev22 == 0), then link in
        // reverse (head-insertion restores the walk order). The human
        // slot is spliced out (the port's human is out-of-pool);
        // slots left unreachable by torn links keep the ascending
        // fallback. `import_ent` cleared the link bit; `link` re-sets
        // it.
        for h in self.g.map_entity.iter_mut() {
            *h = 0;
        }
        // ⭐⭐⭐ DIG W2-F — THE MC1 TWIN of the ghost splice. MC1 has no
        // `byte[1] & 4` ghost class in the importer, but the SHAPE is
        // identical: a chain whose head record is not linkable (class
        // 0, or the link bit clear) orphaned every record below it —
        // their `prev22` is non-zero, so no walk could start there —
        // into the ascending fallback, which head-inserts in slot order
        // and hands the chain back REVERSED. Walk THROUGH a
        // non-linkable record on both legs instead of stopping at it.
        // Shares MC2's kill switch; see [`chain_ghost_splice`].
        let mc1_linkable = |r: &RetailEntMc1| r.class64 != 0 && r.flags & 4 != 0;
        let splice = chain_ghost_splice();
        let hop = |mut s: usize, back: bool| -> usize {
            let mut guard = 0usize;
            while s != 0 && s < n && !mc1_linkable(&st.ents[s]) {
                s = if back {
                    st.ents[s].prev22 as usize
                } else {
                    st.ents[s].next20 as usize
                };
                guard += 1;
                if guard > n {
                    return 0;
                }
            }
            if s >= n { 0 } else { s }
        };
        let mut seen = vec![false; n];
        let mut chains: Vec<Vec<usize>> = Vec::new();
        for head in 1..n {
            let r = &st.ents[head];
            let prev = if splice {
                hop(r.prev22 as usize, true)
            } else {
                r.prev22 as usize
            };
            if r.class64 == 0 || r.flags & 4 == 0 || prev != 0 || seen[head] {
                continue;
            }
            let mut chain = Vec::new();
            let mut cur = head;
            loop {
                if seen[cur] {
                    break; // cycle guard — torn capture
                }
                seen[cur] = true;
                if cur != human_slot as usize {
                    chain.push(cur);
                }
                let next = st.ents[cur].next20 as usize;
                let next = if splice {
                    hop(next, false)
                } else if next == 0 || next >= n || !mc1_linkable(&st.ents[next]) {
                    0
                } else {
                    next
                };
                if next == 0 {
                    break;
                }
                cur = next;
            }
            chains.push(chain);
        }
        for chain in &chains {
            for &slot in chain.iter().rev() {
                let e = &self.g.ent[slot];
                let (x, y, z) = (e.x, e.y, e.z);
                self.g.link(slot, x, y, z);
            }
        }
        for slot in 1..n {
            if seen[slot] || slot == human_slot as usize {
                continue;
            }
            let e = &self.g.ent[slot];
            if e.class64 != 0 && st.ents[slot].flags & 4 != 0 {
                let (x, y, z) = (e.x, e.y, e.z);
                self.g.link(slot, x, y, z);
            }
        }

        // ⭐⭐⭐ THE HUMAN'S SEAT IN HIS OWN CELL CHAIN. He is spliced
        // OUT of the rebuild above (out-of-pool), but retail's carpet
        // is an ordinary member and `sub_11980`'s first-match probe
        // reads his RANK — so carry it: his successor is the recorded
        // `+20`, hopped past anything the port could not link, and
        // accepted only when it really shares his tile. 0 = tail.
        // See [`crate::engine::features::PlayerChain`].
        {
            let cell = crate::engine::features::tile((carpet.x >> 8) as u8, (carpet.y >> 8) as u8);
            let succ = hop(carpet.next20 as usize, false);
            let next = if succ != 0
                && succ < n
                && crate::engine::features::tile(
                    (st.ents[succ].x >> 8) as u8,
                    (st.ents[succ].y >> 8) as u8,
                ) == cell
            {
                succ as u16
            } else {
                0
            };
            self.g.player_chain = crate::engine::features::PlayerChain { cell, next };
        }

        // Free stack: the LIVE recorded order, so port-side spawns land
        // on the same slots the recording's do. Fall back to the
        // load-rebuild scan (999→1) only when the recorded stack is
        // unusable. The reserved human hole stays OUT either way.
        //
        // The RECYCLE stack is imported SEPARATELY below, never chained
        // into this Vec. Chaining it was tried once and was wrong twice:
        // it put recycle entries at the TOP of a Vec that `new_event`
        // pops from the end (inverting retail's free-first priority,
        // :43867-83 vs :43885-908), and it inflated the census so
        // `live.len() == scan_free` could never hold once a respawn had
        // happened — throwing the whole recorded order away for the
        // fallback's lowest-free-slot rule (mc1l2 t≈8291→end: at t=9089
        // retail's newborn (10,0) took slot 80, the port took 18, and
        // the birth-frame blast never arrived).
        //
        // ⚠ The old "MC1's recycle list is the respawn-window sacrifice
        // set alone — an arm the allocator never reaches" rationale for
        // skipping the import entirely is REFUTED by the corpus scan:
        // the death LANDING's rebuild (:55487) arms the stack and never
        // disarms it (742-5,499 armed snapshots per take), and the
        // eruption/swarm windows drain free to 0 with the stack armed —
        // ~3,750 real seizures across mc1hwl0/l48/l49/l37. A pair
        // anchored inside an armed window MUST hold retail's victim
        // ranking or every seizure lands in the wrong slot.
        let live: Vec<u16> = st
            .free_stack
            .iter()
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
        // The recycle-victim stack, order preserved, so a 0-free spawn
        // sacrifices the SAME slot retail's `NewEvent_372C0` second arm
        // does (:43885-908). Unlike MC2 there is NO liveness filter
        // here: MC1's free path never removes a dying victim from the
        // stack (`sub_41E90` :52512-20 — see `Gen::free_entity`), so
        // retail's own stack legitimately holds stale dead cells; the
        // pop's dead-cell skip stands in for retail's unwitnessed
        // unconditional seizure of those. `refill` stays clear — the
        // recorded stack is retail's snapshot, and running it dry is
        // retail returning null.
        self.g.mc2_recycle.refill = false;
        self.g.mc2_recycle.stack = st
            .recycle_stack
            .iter()
            .copied()
            .filter(|&s| (s as usize) < pool && s != human_slot && s != 0)
            .collect();

        // Globals in the closure.
        self.g.rand = st.rand;
        self.g.spawn_count = st.spawn_count;
        // Outside the closure: the retile LCG (pseudo) has no capture.
        self.g.pseudo = 0;
        // The volcano registers (gamedata+36/+38) sit INSIDE the
        // captured struct image, between spawn_count and the free-stack
        // top. They are NOT reconstructable from entity state — a
        // dead-idle (10,18) reads identically latched or not, and a
        // forced 0 lets every dormant driver re-arm on its 1/100 roll
        // where retail holds the latch (mc1l3 slot 347 after its
        // t≈2303 eruption goes dead-idle at c=127: 76 f26 rows, one
        // per d%100==0 tick, the MC1 face of the MC2 vortex law).
        self.g.erupting = st.erupting;
        self.g.plume = st.plume;

        // The wizext+84 GUARD REGISTER **IS** in the recording (the
        // closure carries the whole Type_160 slice; `guard_reg`
        // decodes +84..+152) — import it verbatim like the balloon
        // register below. A census rebuild could never reproduce a
        // POINTER register: (a) STALE entries — retail's memory of
        // dead guards — re-arm the +46 cooldown and block every
        // empty index behind them (mc1hwl0 t=15515: 7 stale cleared,
        // f46 0→16, NOTHING spawns, where the rebuilt register's
        // empty index 6 fired slot 282); (b) ABA duplicates occupy
        // an index forever (mc1l48 t=5343: slot 206 at index 4 AND
        // 10); (c) membership is by pointer, so an owner re-stamped
        // (charmed) guard stays in its original register (mc1l49
        // t=3779 slot 994). Slot values are pool identity — `tr()`
        // applies to play_index only, exactly as the balloon
        // register. This retires the old "stale entries are
        // unknowable" caveat and mc1l1 t=2571's delayed-first-guard
        // note (same defect).
        self.g.mc1_guard_reg.0.clear();
        for w in &st.wizards {
            if w.play_index == 0 {
                continue;
            }
            self.g
                .mc1_guard_reg
                .0
                .insert(tr(w.play_index), w.guard_reg.to_vec());
        }

        // The wizext+52 BALLOON REGISTER, by contrast, IS in the
        // recording — the closure carries the whole Type_160 slice
        // and `balloon_reg` decodes +52/+54/+56. Import it verbatim:
        // the register's INDEX order (spawn order, unrecoverable from
        // a pool census) is what decides which balloon claims which
        // ball and which one a downgrade culls, so a rebuilt-by-slot
        // stand-in hands the fleet its targets backwards. The KEY is
        // the owner stamp the dispatcher reads off the castle (+24),
        // so the human's carpet slot goes through `tr` like every
        // other owner reference.
        self.g.mc1_balloon_reg.0.clear();
        for w in &st.wizards {
            if w.play_index == 0 {
                continue;
            }
            self.g
                .mc1_balloon_reg
                .0
                .insert(tr(w.play_index), w.balloon_reg.to_vec());
        }

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
        // The SHIELD bit (+17 0x40 — our 0x4000) rides the same
        // flags word: without it every pair's damage intake runs
        // unshielded (mc1l3 t=4596: retail life −112/mana −112 on a
        // 450 hit, the port −450/−0).
        self.player.shield = carpet.flags & 0x4000 != 0;
        for i in 1..8 {
            self.g.rival_ents[i] = st.wizards[i].play_index;
            self.g.rival_wanted[i] = st.wizards[i].aggro;
        }
        self.g.rival_ents[0] = 0;
        // The ESTABLISHED-castle register (wizext+50) imports RAW —
        // pool slots map 1:1 — so a bound-at-plant level-0 castle
        // and a stale/cleared bind both arrive exactly as recorded.
        for i in 0..8 {
            self.g.castle_reg[i] = st.wizards.get(i).map_or(0, |w| w.castle);
        }

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
            // ⭐ THE HUSK-WATCH GATE (`var_u8_13332_9`, +9). A
            // per-level constant EXCEPT for the death scatter's
            // `var_916[39]` overflow (MC1 `Rival::human_driven`), so
            // it has to be READ, not derived: mc1l49's player 2
            // carries 0 from t=39094 on and its husk watches its
            // killer every tick for the rest of the take. Set before
            // the eliminated early-out — the state-3 else arm runs
            // whether or not the row still owns a carpet.
            r.human_driven = w.ai_flag != 1;
            r.eliminated = w.play_index == 0;
            if r.eliminated {
                continue;
            }
            let e = &st.ents[w.play_index as usize];
            // BIT-COPY, not a clamp: a corpse HOLDS a wrapped-negative
            // purse (+140 raw subtracts, the living regen tail is the
            // only floor — mc1hwl0 t=23462-23530, −100 held for the
            // whole window). Clamping at import ate the shortfall.
            r.mana = e.f140 as u32;
            r.mana_max = e.f136.max(0) as u32;
            // +132 is a SIGNED 32-bit delta (cast debits are negative;
            // castle casts exceed 16 bits) — the old u16 decode turned
            // a −50 debit into +65486 and the apply clamped to the
            // ceiling.
            r.mana_delta = e.f132;
            r.vdes = w.cmd_speed;
            // ⭐ THE RIVAL'S v_14 KILL LATCH (Type_160 +14), the
            // register that sits ONE FIELD ABOVE the v_12 imported on
            // the line above and was the only half of the pair left
            // unseated. `sub_56380_568B0` (:65146-50) opens its speed
            // token with `if (!sub_55DD0_56300(...) ||
            // *(_WORD *)(*(_DWORD *)(v1 + 160) + 14)) { if
            // (*(_WORD *)(... + 14)) *(_WORD *)(a1 + 48) = 1; }` — a
            // standing latch on the OWNER's control block force-ends
            // the burst, and the shared decrement below zeroes the
            // counter that same tick. The port already models the
            // read ([`World::rival_speed_token_tick`]) and the human
            // half is imported below (`self.mc1_v14`), but a RIVAL's
            // latch defaulted to `false` on every import, so no
            // imported pair could ever take the kill arm.
            //
            // The latch is sampled POST-tick at N, which is exactly
            // what a token walking BELOW its owner's pool slot reads
            // during pair N→N+1 (a token above its owner is re-served
            // by the port's own `sub_15470` head before it runs) —
            // the same seat rationale as the human's.
            //
            // mc1l49 t=21961: rival 3 (ent 646) ARRIVES (:19075-76
            // stamps `v_12 = 0, v_14 = 1`), and on 21962 retail's
            // token slot 154 snaps `+48` 106 → 0, the expiry snap
            // restores `+126` 160 → 80, and — the sustain arm having
            // been skipped — `sub_55E80`'s mid-burst regen pin never
            // runs, so the wizard's purse takes its +100 (1100 →
            // 1200). Unseated, the port decremented to 105, held 160
            // and pinned the regen: all five rows of that pair.
            r.v14 = w.v14 != 0 && !crate::engine::world::mc1_rival_v14_seat_off();
            r.jink = w.strafe;
            // The pending knock impulse (Type_160 +24/+22). A live
            // rival never spends it, so a mid-life import carries a
            // stale one that only its death fall will cash.
            r.knock_dir = w.knock_dir;
            r.knock_mag = w.knock_mag;
            r.grace = w.grace;
            // The TEMPO scalar (+526) — the live field behind every
            // AI cadence (think period, turn-servo divisor, burst
            // lockout). The level config's static twin approximates
            // it for native play, but retail re-stamps it from the
            // per-wizard level table at init and at every respawn;
            // an imported world must carry the recorded value
            // (mc1hwl0 rival 1: 252+ ⇒ think period 1 — the port's
            // config 16 made the dodge re-stamp fire every 16th tick
            // and the strafe decay one step ahead of retail, the
            // t=16772 x,y head).
            r.tempo = w.tempo;
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
                &w.owned_slots,
                &w.spell_list,
                w.life_rate,
                w.regen_stall.min(u16::MAX as u32) as u16,
                st.ents[w.play_index as usize].f148,
            );
        }

        // ⭐⭐⭐ THE BURST LANE OF A **FREED** TOKEN.
        //
        // `import_ent` re-homes retail `+48` into the port's `f26`
        // for class 12 alone (the port has ONE field where retail has
        // two: `+26` is the spell LEVEL on a token, `+48` the burst
        // counter). But `Gen::free_entity` only writes `class64 = 0`
        // — the port's `f26` SURVIVES a free exactly as retail's
        // `+48` does — so a record that WAS a token and is now on the
        // free stack still wears its burst natively, and the
        // class-gated import threw it away. The lane's meaning is
        // decided by the record's LAST OCCUPANT, not its current
        // class.
        //
        // It matters because every `wizext+676` reader is a RAW
        // register read with no liveness test whatever: `sub_14E60`
        // (`reference/remc1/sub_main.cpp:18769-77`) is
        // `pool + 164 * wizext->var_676[s]`, guarded only against
        // slot 0. So a stale register naming a RECYCLED slot hands
        // the caller that slot's live bytes, and `sub_14120`'s
        // castle-upgrade gate refuses outright on `+48 != 0`
        // (:18420-21).
        //
        // WITNESS (mc1l49 t=35404, read from `state.struct_b64`):
        // wiz 1's `owned[16]` is 26; pool slot 26 is `class64 = 0`,
        // `model65 = 16`, `+42 = 646` — wizard 3's Create-Castle
        // token, freed and not yet re-taken — and it still holds
        // `+48 = 101`. Retail refuses the upgrade and rival 594 falls
        // through the cascade to HuntMana (brain byte 13, chase 822,
        // a class-5 creature); the port imported `f26 = +26 = 0`,
        // admitted the upgrade (state 1) and chased its own castle
        // 619. The head repeats every 5 ticks — the wizard's whole
        // think cadence — for the rest of the take.
        //
        // ⚰ THE IMPORT SEAT THAT USED TO SIT HERE WAS RETIRED
        // 2026-09-08 (wave 121, digs D2 + D13; player-ruled). It
        // wrote `f26 = +48` on every FREED slot a `+676` register
        // named, to feed the owned-token readers. That is no longer
        // needed and was actively unfaithful: `Ent::raw48` now
        // carries retail's `+48` RAW FOR EVERY CLASS (set from
        // `r.f48` in the MC1 import arm below, and carried across the
        // `f26` -> `raw48` seam natively by `Gen::free_entity`), so
        // the readers take the word from there and `f26` is left
        // holding retail's `+26` — which is what a freed non-token
        // actually has. Measured before removal: with the seat
        // disabled the whole fixture corpus ran 484 pass / 0
        // regressions, i.e. it had become behaviourally INERT.
        // ⭐ This is the round-98 IMPORT-SEAT hazard closed rather
        // than merely documented: an import-only compensation makes
        // the graded lane green while the NATIVE game stays wrong,
        // and a passing test then encodes an invented law.

        // Hands: the raw +940/+944 bytes index the ACQUISITION list,
        // not the spell table — resolve through the manifestation.
        //
        // A CORPSE'S LIST HOLDS MODELS, NOT SLOTS. The death landing
        // overwrites every live `+532` entry with its entity's +65
        // (:55523) and −1 for the empty ones, so the pool-slot
        // resolution above returns nothing for the whole dead window
        // — retail's own hands read empty there, which is the
        // measured `mc1-death-hand-spell-loss` law. But the RAW hand
        // bytes never move (:54884-923 refills the same list slots in
        // place), so the respawn hands back exactly the spells the
        // corpse went down with; resolve them straight off the list
        // or the re-grant has nothing to bind (mc1l42 t=17397, where
        // retail comes back holding Fireball/Possess and ours came
        // back empty-handed).
        let dead = carpet.f70 == 3;
        let hand = |raw: u16| {
            let s = if dead {
                wiz.spell_list
                    .get(raw as usize)
                    .and_then(|&m| u8::try_from(m).ok())
            } else {
                st.hand_spell(local, raw)
            };
            s.filter(|&s| (s as usize) < SPELL_COUNT).map(SpellId)
        };
        // The death bank is port scaffolding for the dead window —
        // retail's only bank IS the model-form list. Arming it from a
        // LIVE wizard's +676 register invented a bank for spells that
        // were never lost, and the alive-retry arm then "restored"
        // them into the list: mc1hwl0's t=4438 retry wrote a
        // duplicate slot-form 17 that aliased the (9,1) claim bolt
        // reusing slot 17 at t≈7991 into phantom owned[1] — the
        // t=7993 `(12,1)slot106:flags` head. Dead imports keep it
        // (the +676 the recorder sampled is the pre-death owned set,
        // frozen because the corpse's dispatch never rebuilds it).
        let mut death_owned = [false; SPELL_COUNT];
        let mut death_owned_blue = [false; SPELL_COUNT];
        for s in 0..SPELL_COUNT {
            death_owned[s] = dead && wiz.owned_slots[s] != 0;
            death_owned_blue[s] = wiz.blue[s] != 0;
        }
        self.player = Player {
            // BIT-COPY, not a clamp (see the rival seed above): the
            // human corpse holds a wrapped-negative purse too
            // (mc1hwl0 t=23462: 0 − the shield quarter = 4294967196,
            // held 69 ticks to the respawn re-mint).
            mana: carpet.f140 as u32,
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
            // +48 == +50, does not). The LAUNCHER and SPEED (2/21)
            // tokens now run that machine live (manifestation_tick
            // under strict, with the wizard pass applying after
            // them), so their pairs seed the recorded delta raw — a
            // mid-glide pair keeps an ABOVE-carpet token's pending
            // debit (mc1l1 t=8889-8910: six fireball −200 stamps
            // rode f132 through the accel glide). Only the
            // still-inert hold/channel/toggle tokens keep the seed
            // clamp.
            //
            // HEAL (1) COUNTS AS LIVE. `sub_56270` shares nothing
            // with the launcher skeleton but the +48 countdown — it
            // never calls the `sub_55E80` delta debit — so a wizard
            // mid-heal keeps his +100 floor, and the port runs the
            // token itself ([`World::mc1_heal_token_tick`], the same
            // exclusion the strict class-12 dispatch already makes).
            // Clamping on it cost the recorded regen outright
            // (mc1l42 t=10677/10684/10696: retail 800/400/500 against
            // our 700/300/400, two rows a tick).
            //
            // SHIELD (4) COUNTS AS LIVE TOO — its `sub_566C0` machine
            // (the l3 t=4275 law) runs under strict, and its burst is
            // the full 251-tick +48 countdown: clamping on it zeroed
            // every CONCURRENT pending debit for four minutes of play
            // (mc1l3 t=4334-4525: 49 fireball −200 stamps, one per
            // autofire anchor, gone until the shield lapsed at
            // 4275+251).
            // ⚠ The scan walks the HUMAN'S OWN token register, never
            // the pool: class-12 f144 is 0 for EVERY wizard's tokens
            // (shared owner_ptr constant), so a pool-wide `any` took
            // a RIVAL's mid-burst Rebound for the human's and pinned
            // the seed to 0 — on 8-wizard mc1l49 the clamp fired on
            // 7,572 of the first 8,001 ticks (~15k mana rows; retail
            // sub_55E80's `a2` is the token's OWN caster, :56xxx).
            // Same owner join as the accel lane below. Vacuous on
            // every certified take (161,846 ticks, zero firings).
            // Retail only ever zeroes a POSITIVE delta (sub_55E80's
            // live half, :64956-59: `if (v2 && +132 > 0) +132 = 0`) —
            // a negative seed (a pending debit) rides through
            // mid-burst untouched (mc1l48 t=27415-53: 39 firings
            // with f132 = −50 that the old unconditional zero ate).
            mana_delta: if carpet.f132 > 0
                && wiz.owned_slots.iter().any(|&s| {
                    let s = s as usize;
                    if s == 0 || s >= st.ents.len() {
                        return false;
                    }
                    let e = &st.ents[s];
                    // The exempt test keys on the SPELL ID alone (f70/3
                    // for tokens, phases 0..2 alike): the old
                    // `f70 % 3 == 0` conjunct carved a live spell's
                    // token out at phase 0 but NOT at phase 1 — half of
                    // mc1l48's 204 firings were phase-1 tokens of spells
                    // the list itself declares live (spell 16 f70=49,
                    // spell 2 f70=7).
                    // 14 REBOUND joined the keep-list with the
                    // command-arm law: its machine debits the full
                    // cost on the arm tick THROUGH +132 (sub_573F0 →
                    // sub_55E80), and the boundary after the cast
                    // already shows the token decremented (mid-burst)
                    // — clamping the seed ate the −1000 on every
                    // re-cast (mc1l32-quick t=18014 ff., 148 rows).
                    e.class64 == 12
                        && e.f144 == 0
                        && e.f48 != 0
                        && e.f48 as i32 != e.f50 as i32
                        && !(e.f70 < 72
                            && matches!(
                                e.f70 / 3,
                                0 | 1
                                    | 2
                                    | 3
                                    | 4
                                    | 6
                                    | 7
                                    | 8
                                    | 9
                                    | 10
                                    | 11
                                    | 13
                                    | 14
                                    | 16
                                    | 17
                                    | 18
                                    | 19
                                    | 20
                                    | 21
                                    | 22
                                    | 23
                            ))
                }) {
                0
            } else {
                // SIGNED 32-bit seed (see the rival arm above): +132
                // carries the cast debit as a negative value, and
                // castle-cast debits exceed 16 bits (mc1l1 t=3807:
                // −40000). Zero-extending the old u16 raw pinned the
                // player at the mana ceiling on every recorded cast
                // tick — the mc1l1 player.mana + carpet-mirror family
                // (2238 rows), and the idle 950-vs-1000 breathing
                // pairs before it.
                carpet.f132
            },
            life: carpet.act_life,
            // The player's life state rides the carpet's TICK-HANDLER
            // byte +70 (`*(_BYTE*)(a1+70) = 3`, :55550) — +66 is
            // sClass (255 on the carpet, so the old read left every
            // dead player Alive-with-negative-life and re-ran the
            // whole death cascade each pair: the HW 33 rng over-draw
            // runs at t=21468.. were exactly this).
            state: match carpet.f70 {
                2 => LifeState::Falling,
                3 => LifeState::Dead,
                _ => LifeState::Alive,
            },
            left: hand(wiz.hand_left),
            right: hand(wiz.hand_right),
            // A CORPSE OWNS NOTHING. `var_676` is the "spells ever
            // acquired" table — the jar poll's already-known marker
            // reads it (:64790) — and the death scatter never clears
            // it, so a dead wizard's entries still point at the jars
            // his landing threw away (:55519-47 rewrites the +532
            // acquisition list to MODEL numbers and clears each
            // token's owned bit instead). Importing that as ownership
            // made the respawn's re-grant hand back the SCATTERED
            // jars — mc1l42 t=17397 warped the five decaying jars to
            // the castle instead of minting the five fresh tokens
            // retail lays there.
            //
            // A CORPSE, THOUGH — NOT A FALLER. The rewrite is the
            // LANDING's (:55519-47), and `+70` only becomes 3 at
            // :55550, past it; a wizard still falling (`+70` 2) owns
            // his tokens exactly as an alive one does, and that
            // ownership IS what the landing scatters. Zeroing state 2
            // as well left the port's own landing with nothing to
            // throw — retail's five jars leapt to the death point
            // with fresh ttls and ours sat where they were (mc1l42
            // t=17343, 25 rows across flags/life/x/y/z).
            // ⚠ NO STATE-3 BLANK. `+676` has exactly TWO writers in
            // CARPET.EXE and both sit inside `sub_45C10_45F50`
            // (`memset(wizext+676, 0, 48)` at file 0x5e41a, the
            // indexed `mov [ebx+ecx*2+0x2A4], dx` at 0x5e466); every
            // other reference to offset 0x2A4 in the shipped binary is
            // a READ, and HIDDEN.EXE is byte-identical (0x5e9a6). The
            // landing rewrites the `+532` ACQUISITION list and NOTHING
            // else (:55516-49), and the class-3 dispatch table stops
            // calling `sub_45C10` once `+70` leaves the live arms — so
            // a husk's book FREEZES at what its last live tick
            // published and the recording carries it verbatim.
            // WITNESS (mc1l49 34550..35134, the human's husk window):
            // 1,112 raw-shadow rows across 23 indices, retail naming a
            // live jar slot against the port's zero. Retiring this
            // blank alone takes them to 54 — the rest belong to the
            // landing's own clear in [`World::player_land`], gated by
            // the same switch on the strict arm, which takes them to
            // 8. `MGC_NO_MC1_OWNED_SCATTER_KEEP=1` restores both.
            owned: if crate::mc1::rivals::owned_survives_scatter() {
                wiz.owned_slots
            } else {
                match carpet.f70 {
                    3 => [0u16; SPELL_COUNT],
                    _ => wiz.owned_slots,
                }
            },
            grace: wiz.grace,
            // The 16-tick post-hit life-regen stall (u32_383,
            // :55387-90). Unseeded, every pair inside retail's stall
            // window applied one heal quantum retail withheld — the
            // persistent life+5/+40 skew family.
            regen_delay: wiz.regen_stall.min(u16::MAX as u32) as u16,
            // The rate REGISTER (u16_341): applied-then-selected, so
            // a pair straddling a rate flip must inherit the stale
            // value (the castle-establish +5-then-+40 staircase).
            life_rate: wiz.life_rate as i32,
            killer: tr(carpet.f38),
            fall_speed: carpet.f46,
            shield: carpet.flags & 0x4000 != 0,
            invisible: carpet.flags & 0x20 != 0,
            rebound: carpet.flags & 0x8000 != 0,
            death_owned,
            death_owned_blue,
            ..Player::default()
        };

        // The speed-token boost direction: retail keeps the live burst
        // in the token's +48 and the Type_160 speed override; the
        // port's shadow is `player.accel`. Unseeded, a pair inside a
        // burst flew unboosted (the accel-domain pose gate papers over
        // the pose half) AND the v_14 resist arm read no-boost — so
        // the down-cursor cancel never fired and the token ran laps
        // retail had killed (mc1l32 t=9218/11984/…: +48 snapped 7→0
        // against the port's decrement, one +100 regen and a contrail
        // puff along with it).
        // The burst rides the LOCAL wizard's own manifestation — the
        // derived owned register names its slot. A pool-wide scan took
        // a RIVAL's pre-armed book token for the human's burst (HW l0
        // pre-arms Vodor's whole book: slot 476, model 2, f48=229 at
        // t=1) and the free run flew boosted from the seed.
        self.player.accel = [(2usize, 1i8), (21, -1)]
            .into_iter()
            .find_map(|(spell, dir)| {
                let s = wiz.owned_slots[spell] as usize;
                let e = st.ents.get(s).filter(|_| s != 0)?;
                (e.class64 == 12 && e.f144 == 0 && e.f48 != 0 && matches!(e.model65, 2 | 21))
                    .then_some(dir)
            })
            .unwrap_or(0);
        // The v_14 kill latch rides the captured Type_160 directly:
        // sampled post-tick at N, it is exactly what a below-carpet
        // speed token reads during pair N→N+1 (the pair never runs
        // the carpet dispatch that would recompute it). Unseeded, the
        // resisting-press cancel never fired in pair mode.
        self.mc1_v14 = wiz.v14 != 0;

        // Per-wizard cast-charge meters (Type_160 u8_326) — seeded
        // like the regen stall: unseeded, every bolt spawned inside a
        // pair would bank a made-up charge in its +26.
        for (i, w) in st.wizards.iter().enumerate().take(8) {
            self.wiz_charge[i] = w.charge;
        }
        // World-level latches: cleared like retail's load discards its
        // mode block; the tick mailboxes must not leak across pairs.
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.pending_teleport = None;
        self.pending_teleport_slot = None;
        self.pending_respawn = None;
        self.pending_restart = false;
        // ⭐ THE DUEL LOCK IS IMPORTED STATE, NOT A TICK MAILBOX.
        // `Type_160` +314/+316/+318 (victim / counter / hold distance)
        // survive across ticks for up to 800 of them, and the mover's
        // tail (:55228-50) servos the caster's HEADING and steps him
        // toward the victim on every one — so wiping it handed every
        // imported pair, and every `--segmented` reset, a carpet that
        // retail was still dragging. mc1l48's duel on rival 712
        // (t=59119..59839, `duel_count` 200 -> 681) is 720 of that
        // slice's 2,230 pose pairs.
        self.duel = if crate::engine::world::mc1_duel_yaw_drag_off() || wiz.duel_victim == 0 {
            None
        } else {
            Some((wiz.duel_victim, wiz.duel_count, wiz.duel_hold))
        };
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
    /// THE UNGRADED RAW LANES — the per-entity bytes `EntObsMc1` does
    /// NOT carry: `+70` (the handler state), `+71` (the burst/charge
    /// register) and `+58`. The recording holds all three, and
    /// [`Self::retail_import_mc1`] restores all three every pair, so the
    /// graded diff can never see them: a handler that READS one
    /// correctly and WRITES it wrong is erased before it is ever
    /// observed, and pair mode reports CLEAN forever. Only a free run,
    /// which carries its own copy for thousands of ticks, feels it —
    /// which is why an mc1l42 free replay can be bit-exact in every
    /// graded field at t=6623 and still drop two `(10,23)` beam
    /// endpoints at t=6624.
    ///
    /// This is the shadow diff that catches the WRITE. Join it against
    /// the recorded state@N+1 in pair mode and every ungraded write bug
    /// in the take surfaces in one pass.
    /// THE WHOLE-WORLD DUMP, sectioned — the instrument of last resort
    /// when two runs of the PORT disagree and no schema-shaped lane can
    /// say why.
    ///
    /// The raw shadow and the free-stack lane cover everything the
    /// RECORDING holds; this covers everything the PORT holds, which is
    /// strictly more (the terrain planes, the tile heads, the wizard
    /// registers, the THING table, the player column). Diff two of
    /// these and the first differing section names the state that
    /// parted — the only way to attribute a free-run break whose entity
    /// pool, free list and every graded field are bit-identical.
    ///
    /// Sections rather than one blob because a byte offset into a
    /// 400 KB stream is not an answer; a section name is.
    pub fn debug_state_sections(&self) -> Vec<(&'static str, Vec<u8>)> {
        use crate::snapshot::Writer;
        let one = |f: &dyn Fn(&mut Writer)| {
            let mut w = Writer::new();
            f(&mut w);
            w.into_buf()
        };
        vec![
            ("terrain.height", one(&|w| w.put(&self.g.t.height))),
            ("terrain.tile_type", one(&|w| w.put(&self.g.t.tile_type))),
            ("terrain.shading", one(&|w| w.put(&self.g.t.shading))),
            ("terrain.angle", one(&|w| w.put(&self.g.t.angle))),
            ("terrain.ceiling", one(&|w| w.put(&self.g.t.ceiling))),
            ("map_entity", one(&|w| w.put(&self.g.map_entity))),
            ("ent", one(&|w| w.put(&self.g.ent))),
            ("free", one(&|w| w.put(&self.g.free))),
            ("rand", one(&|w| w.put(&self.g.rand))),
            ("pseudo", one(&|w| w.put(&self.g.pseudo))),
            ("spawn_count", one(&|w| w.put(&self.g.spawn_count))),
            ("player_mail", one(&|w| w.put(&self.g.player_mail))),
            ("player_knock", one(&|w| w.put(&self.g.player_knock))),
            ("player_aggro", one(&|w| w.put(&self.g.player_aggro))),
            ("rival_wanted", one(&|w| w.put(&self.g.rival_wanted))),
            ("erupting", one(&|w| w.put(&self.g.erupting))),
            ("plume", one(&|w| w.put(&self.g.plume))),
            ("kills.shots.hits", {
                let mut w = Writer::new();
                w.put(&self.g.kills);
                w.put(&self.g.shots);
                w.put(&self.g.hits);
                w.into_buf()
            }),
            ("player_danger", one(&|w| w.put(&self.g.player_danger))),
            ("banked_houses", one(&|w| w.put(&self.g.banked_houses))),
            ("exhausted", one(&|w| w.put(&self.g.exhausted))),
            ("misfits", one(&|w| w.put(&self.g.misfits))),
            // The two wizard registers are maps, not `Snap` values —
            // rendered as text, which diffs just as well.
            (
                "mc1_guard_reg",
                format!("{:?}", self.g.mc1_guard_reg.0).into_bytes(),
            ),
            (
                "mc1_balloon_reg",
                format!("{:?}", self.g.mc1_balloon_reg.0).into_bytes(),
            ),
            ("thing_table", one(&|w| w.put(&self.table))),
            ("player", one(&|w| w.put(&self.player))),
            ("rivals", one(&|w| w.put(&self.rivals))),
            ("kill_tally", one(&|w| w.put(&self.kill_tally))),
            ("human_pose", one(&|w| w.put(&self.human_pose))),
            ("rival_deaths", one(&|w| w.put(&self.rival_deaths))),
            ("duel", one(&|w| w.put(&self.duel))),
            ("mc1_ring", one(&|w| w.put(&self.mc1_ring))),
            ("mc1_v14", one(&|w| w.put(&self.mc1_v14))),
            ("prev_fire", one(&|w| w.put(&self.prev_fire))),
            ("accel_veto", one(&|w| w.put(&self.accel_veto))),
            ("win_pct", one(&|w| w.put(&self.win_pct))),
            ("placeholders", one(&|w| w.put(&self.placeholders))),
        ]
    }

    pub fn raw_shadow_mc1(&self) -> Vec<RawShadowMc1> {
        (1..self.g.ent.len() as u16)
            .filter_map(|slot| {
                let e = &self.g.ent[slot as usize];
                (e.class64 != 0).then_some(RawShadowMc1 {
                    slot,
                    class: e.class64,
                    model: e.model65,
                    f70: e.tick70,
                    f71: e.f71,
                    f58: e.f58,
                    f44: e.f44,
                    next20: e.next20,
                    prev22: e.prev22,
                    f26: e.f26,
                    f28: e.f28,
                    f36: e.f36,
                    f38: e.f38,
                    f40: e.f40,
                    f46: e.f46,
                    f50: e.f50,
                    f52: e.f52,
                    f54: e.f54,
                    f56: e.f56,
                    f59: e.f59,
                    f68: e.f68,
                    f69: e.f69,
                    f78: e.f78,
                    f80: e.f80,
                    f82: e.f82,
                    f84: e.f84,
                    type86: e.type86,
                    frame88: e.frame88,
                    frames89: e.frames89,
                    mail: e.mail,
                    f128: e.f128,
                    f130: e.f130,
                    f144: e.f144,
                    dest_x: e.dest_x,
                    dest_y: e.dest_y,
                    site_z: e.site_z,
                })
            })
            .collect()
    }

    /// Every wizard's ungraded wizext/brain lanes, retail convention —
    /// the Type_160 counterpart of [`Self::raw_shadow_mc1`]. Wiz 0 is
    /// the human's Gen mirrors; each live rival contributes the
    /// registers `Rival::wiz_shadow_lanes` projects plus the
    /// World-held charge meter, wanted timer and balloon register.
    /// Eliminated rivals are omitted (the roster comparison owns that
    /// story).
    pub fn wiz_shadow_mc1(&self) -> Vec<WizShadowMc1> {
        let breg = |key: u16| -> Vec<i64> {
            let mut v: Vec<i64> = self
                .g
                .mc1_balloon_reg
                .0
                .get(&key)
                .map(|r| r.iter().map(|&s| s as i64).collect())
                .unwrap_or_default();
            v.resize(3, 0);
            v
        };
        let mut out = vec![WizShadowMc1 {
            wiz: 0,
            ent: PLAYER_TARGET,
            scalars: vec![
                ("charge", self.wiz_charge[0] as i64),
                ("knock_dir", self.g.player_knock.0 as i64),
                ("knock_mag", self.g.player_knock.1 as i64),
                ("danger", self.g.player_danger as i64),
                ("aggro", self.g.player_aggro as i64),
                ("banked_houses", self.g.banked_houses as i64),
                ("castle_alert", self.g.castle_alert as i64),
                ("player_alert", self.g.player_alert as i64),
                ("balloon_alert", self.g.balloon_alert as i64),
                ("kills", self.g.kills as i64),
                ("shots", self.g.shots as i64),
                ("hits", self.g.hits as i64),
                // The regen/debit accumulator (carpet +132) — the
                // lane every mana divergence rides one tick before
                // the graded f140 moves.
                ("mana_delta", self.player.mana_delta as i64),
                // The life-regen chain — rival-only through session
                // 49's t=10097 `life` head (the same asymmetry as
                // `acq`/`owned` below): the graded life lane shows
                // the sum, these show the terms.
                ("grace", self.player.grace as i64),
                ("regen_stall", self.player.regen_delay as i64),
                ("life_rate", self.player.life_rate as i64),
            ],
            arrays: vec![
                ("balloon_reg", breg(PLAYER_TARGET)),
                // The human's acquisition list + owned register were
                // shadow-blind through session 48 (only RIVALS
                // exported `acq`/`owned`) — the t=7993 stale-entry
                // alias could only be seen by hand-dumping. 5th
                // instrument-asymmetry occurrence; keep both sides
                // exporting the same lanes.
                ("acq", self.mc1_acq.iter().map(|&v| v as i64).collect()),
                (
                    "owned",
                    self.player.owned.iter().map(|&v| v as i64).collect(),
                ),
            ],
        }];
        for r in &self.rivals {
            if r.eliminated {
                continue;
            }
            let (mut scalars, mut arrays) = r.wiz_shadow_lanes();
            scalars.push(("charge", self.wiz_charge[r.slot as usize] as i64));
            scalars.push(("aggro", self.g.rival_wanted[r.slot as usize] as i64));
            arrays.push(("balloon_reg", breg(r.ent)));
            out.push(WizShadowMc1 {
                wiz: r.slot,
                ent: r.ent,
                scalars,
                arrays,
            });
        }
        out
    }

    /// Every MC2 wizard's ungraded player-block/brain lanes, retail
    /// convention — the twin of [`Self::wiz_shadow_mc1`] and the arm
    /// the harness never had. Wiz 0 is the human's book + charge; each
    /// live rival contributes the registers
    /// `Mc2Rival::wiz_shadow_lanes` projects plus the World-held
    /// charge meter and the castle index retail STORES and the port
    /// re-derives (`rival_castle`) — that last one is the whole
    /// `castle_ent` story: 21 brain sites read the stored index, so a
    /// re-derivation that disagrees for one tick is a different
    /// decision, and nothing compared them before.
    ///
    /// Eliminated rivals are omitted (the roster comparison owns that
    /// story).
    pub fn wiz_shadow_mc2(&self) -> Vec<WizShadowMc2> {
        let book = |b: &crate::mc2::cast::Mc2Spellbook| -> Vec<(&'static str, Vec<i64>)> {
            vec![
                ("spell_ent", b.ent.iter().map(|&v| v as i64).collect()),
                ("levels", b.levels.iter().map(|&v| v as i64).collect()),
                ("sel", b.sel.iter().map(|&v| v as i64).collect()),
                ("ring", b.ring.iter().map(|&v| v as i64).collect()),
                ("xp_bank", b.xp_bank.iter().map(|&v| v as i64).collect()),
                ("xp_vol", b.xp_vol.iter().map(|&v| v as i64).collect()),
            ]
        };
        let mut out = vec![WizShadowMc2 {
            wiz: 0,
            ent: PLAYER_TARGET,
            scalars: vec![
                ("charge", self.wiz_charge[0] as i64),
                ("hand_left", self.mc2_book.left as i64),
                ("hand_right", self.mc2_book.right as i64),
            ],
            arrays: book(&self.mc2_book),
        }];
        for r in &self.mc2_rivals {
            if r.eliminated || r.ent == 0 {
                continue;
            }
            // The rival's own projection already carries the book
            // (same six array names the `book` helper builds for the
            // human) — this half adds only what the World holds.
            let (mut scalars, arrays) = r.wiz_shadow_lanes();
            scalars.push(("charge", self.wiz_charge[r.slot as usize] as i64));
            scalars.push((
                "castle_ent",
                self.rival_castle(r.ent).map_or(0, |c| c as i64),
            ));
            out.push(WizShadowMc2 {
                wiz: r.slot,
                ent: r.ent,
                scalars,
                arrays,
            });
        }
        out
    }

    /// The port's free list, bottom-to-top (`new_event` pops the END) —
    /// the WORLD-level counterpart of [`Self::raw_shadow_mc1`].
    ///
    /// It is the widest ungraded lane in the harness: the recording
    /// carries the stack per tick, [`Self::retail_import_mc1`] installs
    /// it every pair, and the obs schema never compares it. So a port
    /// that pushes a freed slot at the wrong moment — or frees a
    /// different NUMBER of slots — reads CLEAN in pair mode forever and
    /// only bites a free run, where it surfaces as balanced
    /// same-`(class, model)` missing/extra rows once the two allocators
    /// hand out different slots for the same spawn.
    pub fn free_stack_mc1(&self) -> &[u16] {
        &self.g.free
    }

    /// The port's MC1 RECYCLE stack (the victim roster `sub_37220`
    /// rebuilds at 0x20400), bottom-to-top like
    /// [`Self::free_stack_mc1`]. MC1 shares the MC2 allocator's recycle
    /// half — the seizure path only runs once the free stack is dry.
    ///
    /// Ungraded for the same reason the free stack is, and it bites the
    /// same way: mc1hwl0's t=31888 head is 54 consecutive seizures off
    /// by exactly one victim, invisible to every pair.
    pub fn recycle_stack_mc1(&self) -> &[u16] {
        &self.g.mc2_recycle.stack
    }

    /// Arm the mid-walk pool probe (the port-side `dump-state
    /// --at-slot` instrument): the NEXT tick's entity walk snapshots
    /// the whole pool as it reaches `slot`, before that slot
    /// dispatches. Read back with [`Self::port_ent_lanes_mc1`]
    /// (`from_probe = true`); [`Self::walk_probe_hit`] says whether
    /// the walk actually crossed the slot.
    pub fn arm_walk_probe(&mut self, slot: u16) {
        self.walk_probe_arm = Some(slot);
        self.walk_probe = None;
    }

    pub fn walk_probe_hit(&self) -> bool {
        self.walk_probe.is_some()
    }

    /// The port's record at `slot` as RETAIL-convention lanes — the
    /// port-side half of the `dump-state --port` side-by-side. Same
    /// lane names and order as [`retail_ent_lanes_mc1`]; `None` = the
    /// port does not model that lane (`f61`/`f62`, the wizard mana
    /// delta `f132`, the AI signature `f148`, plain `f48`).
    /// Translation back into retail space mirrors the importer:
    /// `PLAYER_TARGET` → the human's recorded slot on every owner/
    /// target lane, `f58` as the unsigned byte, the class-12 owner
    /// re-homed to `f42`, the class-12 burst and the (10,41) leveler
    /// rung re-homed to `f48`, the castle's transform sub-state
    /// (port `f59`) shown on the `f48` lane (retail's pure-wait 4
    /// imports as 1 — a `retail 4 port 1` row on a transforming
    /// castle is the known representation merge, not a divergence).
    ///
    /// `from_probe = true` reads the mid-walk snapshot armed by
    /// [`Self::arm_walk_probe`] instead of the live pool.
    pub fn port_ent_lanes_mc1(
        &self,
        slot: u16,
        human_slot: u16,
        from_probe: bool,
    ) -> Option<Vec<(&'static str, Option<i64>)>> {
        let e = if from_probe {
            self.walk_probe.as_ref()?.get(slot as usize)?
        } else {
            self.g.ent.get(slot as usize)?
        };
        let untr = |v: u16| -> i64 {
            if v == PLAYER_TARGET {
                human_slot as i64
            } else {
                v as i64
            }
        };
        let castle = e.class64 == 3 && e.model65 == 2;
        let some = |v: i64| Some(v);
        Some(vec![
            ("rand", some(e.rand as i64)),
            ("max_life", some(e.max_life as i64)),
            ("act_life", some(e.act_life as i64)),
            ("flags", some(e.flags as i64)),
            ("next20", some(e.next20 as i64)),
            ("prev22", some(e.prev22 as i64)),
            (
                "id24",
                some(if e.class64 == 11 {
                    e.id24 as i64
                } else {
                    untr(e.id24)
                }),
            ),
            (
                "f26",
                if e.class64 == 12 {
                    None // retail +26 = spell level there, unmodeled
                } else {
                    some(e.f26 as i64)
                },
            ),
            (
                "f28",
                if e.class64 == 10 && e.model65 == 41 {
                    None // the leveler rung lives on the f48 lane
                } else {
                    some(e.f28 as i64)
                },
            ),
            ("f30", some(e.f30 as i64)),
            ("f32", some(e.f32 as i64)),
            ("f34", some(e.f34 as i64)),
            ("f36", some(e.f36 as i64)),
            ("f38", some(untr(e.f38))),
            ("f40", some(untr(e.f40))),
            (
                "f42",
                if e.class64 == 12 {
                    some(untr(e.f144))
                } else {
                    None
                },
            ),
            ("f44", some(e.f44 as i64)),
            ("f46", some(e.f46 as i64)),
            (
                "f48",
                if e.class64 == 12 {
                    some(e.f26 as i64)
                } else if e.class64 == 10 && e.model65 == 41 {
                    some(e.f28 as i64)
                } else if castle && e.tick70 == 5 {
                    some(e.f59 as i64)
                } else if castle {
                    some(0)
                } else {
                    None
                },
            ),
            ("f50", some(e.f50 as i64)),
            ("f52", some(untr(e.f52))),
            ("f54", some(untr(e.f54))),
            ("f56", some(e.f56 as i64)),
            ("f58", some(e.f58 as i64 & 0xFF)),
            ("f59", if castle { None } else { some(e.f59 as i64) }),
            ("f61", None),
            ("f62", None),
            ("f63", some(e.f63 as i64)),
            ("class64", some(e.class64 as i64)),
            ("model65", some(e.model65 as i64)),
            ("f66", some(e.f66 as i64)),
            ("f67", some(e.f67 as i64)),
            ("f68", some(e.f68 as i64)),
            ("f69", some(e.f69 as i64)),
            ("f70", some(e.tick70 as i64)),
            ("f71", some(e.f71 as i64)),
            ("x", some(e.x as i64)),
            ("y", some(e.y as i64)),
            ("z", some(e.z as i64)),
            ("f78", some(e.f78 as i64)),
            ("f80", some(e.f80 as i64)),
            ("f82", some(e.f82 as i64)),
            ("f84", some(e.f84 as i64)),
            ("type86", some(e.type86 as i64)),
            ("frame88", some(e.frame88 as i64)),
            ("frames89", some(e.frames89 as i64)),
            ("mail0.amt", some(e.mail[0].0 as i64)),
            ("mail0.src", some(untr(e.mail[0].1))),
            ("mail1.amt", some(e.mail[1].0 as i64)),
            ("mail1.src", some(untr(e.mail[1].1))),
            ("mail2.amt", some(e.mail[2].0 as i64)),
            ("mail2.src", some(untr(e.mail[2].1))),
            ("mail3.amt", some(e.mail[3].0 as i64)),
            ("mail3.src", some(untr(e.mail[3].1))),
            ("mail4.amt", some(e.mail[4].0 as i64)),
            ("mail4.src", some(untr(e.mail[4].1))),
            ("mail5.amt", some(e.mail[5].0 as i64)),
            ("mail5.src", some(untr(e.mail[5].1))),
            ("f126", some(e.f126 as i64)),
            ("f128", some(e.f128 as i64)),
            ("f130", some(e.f130 as i64)),
            ("f132", None),
            ("f136", some(e.f136 as i64)),
            ("f140", some(e.f140 as i64)),
            ("f144", some(if e.class64 == 12 { 0 } else { untr(e.f144) })),
            ("f146", some(untr(e.f146))),
            ("f148", None),
            ("dest_x", some(e.dest_x as i64)),
            ("dest_y", some(e.dest_y as i64)),
            ("site_z", some(e.site_z as i64)),
        ])
    }

    /// The port's free/recycle stacks for the MC2 dump tail — MC2 pops
    /// FREE first and sacrifices a recycle victim only when it is dry
    /// (`NewEvent_4A050`; the opposite of MC1's order). Both
    /// bottom-to-top, next pop LAST.
    pub fn free_stacks_mc2(&self) -> (&[u16], &[u16]) {
        (&self.g.free, &self.g.mc2_recycle.stack)
    }

    /// The port's record at `slot` as RETAIL-convention MC2 lanes —
    /// [`Self::port_ent_lanes_mc1`]'s twin, the port half of the MC2
    /// `dump-state --port` side-by-side. Same lane names and order as
    /// [`retail_ent_lanes_mc2`]; `None` = the port does not model the
    /// lane FOR THIS RECORD'S CLASS. The translation mirrors
    /// `import_ent_mc2` exactly, so the per-class dual homes route the
    /// same way (the scratch quartet, the class-15 cast machine, the
    /// m27/m23/(14,2) `@0x2C` tenants, the (10,79) defender-piece
    /// layout, the castle's `@0x2E` build sub-state, ...). Fusions the
    /// import cannot be inverted through print on their best-known
    /// lane: `f1a`/`owner28` split per the `obs_project_mc2` owner
    /// families, `rand` prints the port's low 16 bits (retail's u16
    /// stream — the low bits of one shared LCG), and the raw `flags`
    /// dword prints `—` (compare the bit sub-lanes instead).
    ///
    /// `from_probe = true` reads the mid-walk snapshot armed by
    /// [`Self::arm_walk_probe`] — the walk loop is game-shared, so
    /// `--at-slot` works on MC2 unchanged.
    /// The port's tile-chain head for one cell of the 256x256 map
    /// (`map_entity`), for the shadow census's head-table check — retail's
    /// table is derivable from a record (every linked entity with
    /// `prev18 == 0` heads its own cell's chain), so the two tables are
    /// COMPARED, never imported.
    pub fn map_head_cell(&self, cell: usize) -> u16 {
        self.g.map_entity.get(cell).copied().unwrap_or(0)
    }

    pub fn port_ent_lanes_mc2(
        &self,
        slot: u16,
        human_slot: u16,
        from_probe: bool,
    ) -> Option<Vec<(&'static str, Option<i64>)>> {
        let pool = if from_probe {
            self.walk_probe.as_ref()?
        } else {
            &self.g.ent
        };
        let e = pool.get(slot as usize)?;
        let untr = |v: u16| -> i64 {
            if v == PLAYER_TARGET {
                human_slot as i64
            } else {
                v as i64
            }
        };
        let (c, m) = (e.class64, e.model65);
        let castle = c == 3 && m == 2;
        let piece = c == 10 && m == 79;
        let sphere = c == 10 && matches!(m, 39 | 57);
        // The (10,89) CAVE-IN keeps its wave in `f44` (@0x2C) and its
        // one-shot debris latch in `f54` (@0x36) — see
        // `import_ent_mc2`. Its `@0x2A` (the NewEvent default 100) has
        // NO port home at all, and `@0x34` is dead on it, so both of
        // those lanes must print `—` rather than borrow a field that
        // means something else. Publishing `f44` in the `f2a` lane and
        // `f54` in the `f34` lane made every live collapse read as
        // four bogus `≠` rows.
        let cavein = c == 10 && m == 89;
        let m27 = c == 5 && m == 27;
        let pyramid = c == 5 && m == 10;
        // The class-9 F_MC2PROJ stamp overwrites both bit 3 and bit 29
        // (proj.rs) — those two retail bits are unrecoverable there.
        let proj9 = c == 9 && m != 13;
        // The `ramp2c` set — the `word_0x2C_44` tenants whose f44
        // holds @0x2C, displacing @0x2A. The (2,7)/(2,8) falling
        // props are tenants too: their f44 is the live gravity
        // velocity (sub_652C0 EF:62650-60; the port's own ctor and
        // `mc2_falling_tick` already treat it that way).
        // The (5,21) DEVIL joined 2026-08-29: @0x2C is its live JUMP
        // IMPULSE (sub_265A0 EF:17098-151; the port reader is
        // `Gen::m21_jump`'s f44 integrator). The uniform @0x2A home
        // seeded every imported devil with the dead 400 (mc2l24
        // t=7918: 47 devils each +400/tick; mc2l22 t=1: Δ = 485 =
        // 400 − (−85)).
        // The (5,22) WORM joined 2026-09-02: @0x2C is the head's SPIN
        // RATE / a segment's ORBIT ANGLE (`word_0x2C_44`, f44) and
        // @0x2A the head's live SERPENTINE ANGLE (`subSpellIndex`, f46
        // — multipart.rs header). Published as f2c/f2a below.
        let worm22 = c == 5 && m == 22;
        // The (10,75) whirlwind TAIL NODE seats @0x36 (the z-stack
        // offset) in f50; the (10,22) HEAD keeps the uniform @0x30
        // there (`sub_331A0` EF:24188 is its live writer).
        let wind_node = c == 10 && m == 75 && c10_field_home();
        let ramp2c = m27
            || (c == 5 && matches!(m, 21 | 22 | 23))
            || (c == 14 && m == 2)
            || (c == 2 && matches!(m, 7 | 8));
        // The doomsday-release life latch: sv2 (relocated to site_z)
        // 16/17 puts the @0x2E latch in f26 even on the @0x10 models.
        let doom_latch = matches!(e.site_z, 16 | 17);
        // The EXACT INVERSE of `import_ent_mc2`'s class-5 `f26` arms,
        // arm for arm: the m0/m19/m27 family keeps @0x10 unless the
        // pyramid-summon latch (16/17) claims @0x2E; the (5,10) pyramid
        // keeps @0x10 unconditionally; the (5,21) DEVIL keeps neither
        // (its f26 is `byte_0x44_68`, published on the `b44` lane
        // below); everything else on class 5 keeps @0x10 unless one of
        // the five StageVar2 charm/latch kinds owns @0x2E.
        // ⚠ This also repairs a PRE-EXISTING inversion break: the two
        // arms below used `doom_latch` for the (5,13) townie where the
        // importer uses the five-kind guard, so a CHARMED townie
        // round-tripped through the wrong word.
        let devil5 = c == 5 && m == 21;
        let c5_scratch = c == 5
            && !devil5
            && (matches!(m, 0 | 19 | 27) && !doom_latch
                || pyramid
                || !matches!(e.site_z, 12 | 13 | 14 | 16 | 17));
        // The owner-fusion split, `obs_project_mc2`'s families: the
        // three translated-owner classes recover retail's `@0x28` from
        // the fused id24; the pyramid keeps its SPIN ANGLE there.
        // The (10,57) FOOL'S-MANA sphere is a fourth family:
        // `sub_6C870` stamps `parentId_0x28 = caster.id_0x1A`
        // (EF:57905) and the importer fuses it into id24 (the
        // `owner28 != 0` branch); an authored/unowned sphere keeps
        // `id24 == slot` (new_event's default) and projects 0 exactly
        // like retail's owner28 = 0.
        let translated = (c == 15 && e.id24 != slot)
            || (c == 10 && m == 42 && e.id24 != slot)
            || (c == 10 && m == 57 && e.id24 != slot)
            || (c == 5
                && matches!(m, 0 | 19 | 21 | 25)
                && self
                    .g
                    .ent
                    .get(untr(e.id24) as usize)
                    .is_some_and(|p| p.class64 == 5 && p.model65 == 10));
        // The raw `owner28` row's own predicate — the same families
        // `obs_project_mc2` recovers, PLUS the summon-army chain
        // (EF:57669 / :59170 / :29611 / :58072). Kept separate from
        // `translated` on purpose: `f1a` below must stay published for
        // class 9/10 (their @0x1A is the caster too and it MATCHES —
        // mc2l6-rsg t=15631 slot 6, retail 343 / port 343), while the
        // metamorph puppet (sv2 == 12) is a `translated` member here
        // only through the pyramid arm.
        let owner_fused = translated
            || (c == 9 && matches!(m, 24 | 25) && e.id24 != slot)
            || (c == 10 && m == 72 && e.id24 != slot)
            || (c == 5 && matches!(e.site_z, 12 | 13) && e.id24 != slot);
        let bit = |k: u32| Some((e.flags >> k & 1) as i64);
        let some = |v: i64| Some(v);
        let held = self.mc2_sv_held.iter().find(|h| h.ent == slot);
        Some(vec![
            ("next0", None),
            (
                "max_life",
                if c == 15 {
                    None
                } else {
                    some(e.max_life as i64)
                },
            ),
            ("life", some(e.act_life as i64)),
            ("flags", None),
            (
                "flags.b0_walk1",
                if c == 3 && m == 3 { bit(0) } else { None },
            ),
            ("flags.b0_done2", bit(1)),
            ("flags.b0_link4", bit(2)),
            ("flags.b0_coll8", if proj9 { None } else { bit(3) }),
            ("flags.b0_x20", bit(5)),
            ("flags.b0_chase40", if sphere { bit(6) } else { None }),
            ("flags.b1_reap4", bit(10)),
            ("flags.b1_x8", bit(26)),
            // Watched for the spheres (the decay channel) AND for
            // class 9 (the `sub_159E0` hate-ledger stamp, EF:7347) —
            // the ledger half had NO port lane at all, which is why
            // the missing import seat was invisible to every graded
            // pair and to the raw shadow alike.
            (
                "flags.b1_decay20",
                if sphere || c == 9 { bit(13) } else { None },
            ),
            ("flags.b2_kill1", bit(16)),
            ("flags.b2_x4", bit(27)),
            ("flags.b2_x10", bit(28)),
            ("flags.b2_x20", if proj9 { None } else { bit(29) }),
            (
                "scratch10",
                if piece {
                    some(e.f44 as i64)
                } else if c == 5 && !devil5 && !crate::engine::features::no_summon_lease_field() {
                    // ⭐ DIG 98-Q20 — WITH THE SECOND FIELD BOTH RETAIL
                    // WORDS ARE PUBLISHABLE AT ONCE. Class 5's `f26` is
                    // @0x10 unconditionally now (the (5,21) devil's is
                    // @0x44 and is published on `b44`), so this lane
                    // stops going dark on every charmed record.
                    some(e.f26 as i64)
                } else if c5_scratch {
                    some(e.f26 as i64)
                } else if c == 5 {
                    None
                } else if c == 15 {
                    if e.tick70 == 78 {
                        // The detach-arc counter's own home — see
                        // [`World::no_mc2_stolen_arc_keeps_cast_state`].
                        some(if crate::mc2::cast::no_mc2_stolen_arc_keeps_cast_state() {
                            e.f26 as i64
                        } else {
                            e.f50 as i64
                        })
                    } else {
                        None
                    }
                } else {
                    some(e.f26 as i64)
                },
            ),
            ("rand", some((e.rand & 0xFFFF) as i64)),
            ("next16", some(e.next20 as i64)),
            ("prev18", some(e.prev22 as i64)),
            (
                "f1a",
                if pyramid {
                    some(untr(e.id24))
                } else if translated && c != 5 {
                    None // @0x1A unrecoverable behind the @0x28 fusion
                } else {
                    some(untr(e.id24))
                },
            ),
            (
                "yaw",
                if c == 15 {
                    some(0) // f30 holds @0x2A there; retail yaw dead 0
                } else {
                    some(if piece { e.f34 } else { e.f30 } as i16 as i64)
                },
            ),
            (
                "pitch",
                some(if piece { e.f36 } else { e.f32 } as i16 as i64),
            ),
            (
                "roll",
                if piece {
                    None
                } else {
                    some(e.f34 as i16 as i64)
                },
            ),
            ("f22", if m27 { some(e.f36 as i16 as i64) } else { None }),
            // @0x24 is honestly the killer latch again now that the
            // jar arc's wraith ref lives in `f40` (@0x26) where retail
            // puts it — the suppression below was hiding that misfiling.
            ("f24", some(untr(e.f38))),
            ("f26", some(untr(e.f40))),
            (
                "owner28",
                some(if pyramid {
                    e.f36 as i64
                } else if c == 5
                    && e.site_z == 14
                    && !crate::mc2::mobs::no_mc2_alliance_parent_seat()
                {
                    // The ALLIANCE charm's parent — the one @0x28 the
                    // port cannot fuse into id24 (the victim keeps its
                    // own @0x1A). Same source as `obs_project_mc2`'s
                    // arm: the `mc2_allied` side map.
                    untr(self.g.mc2_allied.0.get(&slot).copied().unwrap_or(0))
                } else if owner_fused {
                    untr(e.id24)
                } else {
                    0
                }),
            ),
            (
                "f2a",
                if c == 15 {
                    some(e.f30 as i64)
                } else if c == 10 && c10_2a_in_f140(m) {
                    // 18/19 joined 2026-08-29: the (10,19) column's
                    // area damage is @0x2A (sub_10C80's subSpell arg,
                    // EF:24151), homed in f140 like the rest of the
                    // volcano family — an f44 home read the caster's
                    // subspell and every IMPORTED column posted zero
                    // mail (retail's (10,19) @0x90 is 0).
                    some(e.f140 as i64)
                } else if worm22 {
                    some(e.f46 as i64) // the head's spiral angle (@0x2A → f46)
                } else if ramp2c || piece || cavein || (c == 10 && m == 16) {
                    None
                } else {
                    some(e.f44 as i64)
                },
            ),
            (
                "f2c",
                if ramp2c || c == 15 || (c == 10 && (m == 16 || c10_2c_in_f44(m))) {
                    some(e.f44 as i16 as i64)
                } else if pyramid {
                    // The (5,10)'s @0x2C is its TURN RATE and it lives
                    // in `f46` (see `import_ent_mc2`) — the same
                    // @0x2C -> f46 re-home the castle and the mana
                    // sphere already carry. DIAGNOSTIC ONLY.
                    some(e.f46 as i64)
                } else if sphere || castle || (c == 3 && matches!(m, 0 | 1)) {
                    // A WIZARD's @0x2C is the DEATH-FALL VELOCITY (see
                    // `import_ent_mc2`) and it lives in `f46` for the
                    // same reason the sphere's z-velocity does.
                    // The castle's @0x2C is the GUARD COOLDOWN and it
                    // now lives in `f46` (see `import_ent_mc2`) — the
                    // same @0x2C -> f46 re-home the mana sphere
                    // already uses. Publishing it is what makes
                    // `dump-state`, `explain` and the MGC_RAW_SHADOW
                    // census able to see the 0 -> 16 latch and the
                    // 16-pass ladder instead of a `—`; printing `—`
                    // here is why nobody could tell the port had no
                    // cooldown state at all. DIAGNOSTIC ONLY —
                    // `port_ent_lanes_mc2` feeds the raw-lane dump and
                    // the shadow census, never `obs_project_mc2`, so
                    // this adds no grading floor.
                    some(e.f46 as i64)
                } else if piece {
                    some(e.f30 as i16 as i64)
                } else {
                    None
                },
            ),
            (
                "f2e",
                if castle {
                    some(e.f59 as i64)
                } else if c == 15 {
                    some(e.f26 as i64)
                } else if c == 5 {
                    if !crate::engine::features::no_summon_lease_field() {
                        // ⭐ DIG 98-Q20 — @0x2E has its OWN home, so it
                        // publishes for every class-5 record instead of
                        // only the ones whose `f26` happened to be
                        // seated on it.
                        some(e.lease2e.0 as i64)
                    } else if c5_scratch || devil5 {
                        None // f26 holds @0x10 (or @0x44); @0x2E is dead
                    } else {
                        some(e.f26 as i64)
                    }
                } else if sphere
                    || (c == 10 && m == 45)
                    || (c == 3 && matches!(m, 0 | 1))
                    || orb_breathe_at_3d(c, m)
                {
                    None // f46 repurposed (z-vel from @0x2C / @0x3D link)
                } else {
                    some(e.f46 as i64)
                },
            ),
            (
                "f30",
                if c == 15 {
                    some(e.f28 as i64)
                } else if pyramid || m27 || wind_node {
                    None // the (10,75) node's f50 holds @0x36
                } else {
                    some(e.f50 as i64)
                },
            ),
            (
                "f32",
                if pyramid {
                    None // `f52` homes the GLOBAL beam ramp here (@0x32 is dead)
                } else {
                    some(untr(e.f52))
                },
            ),
            (
                "f34",
                if c == 15 || piece || cavein {
                    None // f54 holds @0x36 there; @0x34 unrecoverable
                } else {
                    some(untr(e.f54))
                },
            ),
            (
                "f36",
                if c == 15 || piece || cavein {
                    some(e.f54 as i64)
                } else if wind_node {
                    some(e.f50 as i64) // the whirlwind node's z-stack offset
                } else if matches!(c, 2 | 10) {
                    None // f56 holds @0x38 there
                } else {
                    some(e.f56 as i64)
                },
            ),
            (
                "b38",
                if matches!(c, 2 | 10) {
                    some(e.f56 as i64)
                } else if c == 15 {
                    None // f28 holds @0x30 there; @0x38 is write-only
                } else {
                    some(e.f28 as i64)
                },
            ),
            ("b39", some(e.f58 as i64 & 0xFF)),
            (
                "b3a",
                if castle || c == 15 || (c == 10 && m == 42) {
                    None // f59 holds @0x2E / @0x3B there
                } else {
                    some(e.f59 as i64)
                },
            ),
            (
                "b3b",
                if m27 {
                    some(e.f50 as u8 as i64)
                } else if c == 15 {
                    some(e.f59 as i64)
                } else if c == 10 && m == 42 {
                    // The painter's settle latch — `sub_50370` stamps
                    // `@0x3B` (EXE 0x74BAF), not `@0x3A`, and `f59`
                    // is where the port holds it. See
                    // [`crate::mc2::castle::no_mc2_painter_settle_lane`].
                    some(e.f59 as i64)
                } else {
                    None
                },
            ),
            ("b3c", None),
            (
                "b3d",
                if piece {
                    some(e.f69 as i64)
                } else if worm22 || pyramid {
                    // f46 holds @0x2A on the worm and @0x2C on the
                    // pyramid; @0x3D is dead on both.
                    None
                } else if c == 5 || (c == 10 && matches!(m, 45 | 78)) || orb_breathe_at_3d(c, m) {
                    // ⭐ THE (10,78) MAGIC MINE JOINED 2026-09-04: its
                    // `@0x3D` is the LIVE SHOT COUNTER of a detonation
                    // burst — `sub_3A8B0` case 2 sizes it 6 or 1 from
                    // the swallowed tier's `fontType_0x1B & 1`
                    // (EF:29922-25) and case 5 spends one per relaunched
                    // bolt, ending the burst at zero (EF:30021-24). The
                    // ctor seeds 1 (EF:36973). With the lane unpublished
                    // and unimported, a pair that lands mid-burst seeded
                    // 0 and the counter wrapped to 255 instead of ending.
                    some(e.f46 as u8 as i64)
                } else {
                    None
                },
            ),
            ("phase3e", some(e.f63 as i64)),
            ("class3f", some(e.class64 as i64)),
            ("model40", some(e.model65 as i64)),
            ("b41", some(e.f66 as i64)),
            ("b42", if piece { None } else { some(e.f67 as i64) }),
            ("b43", some(if piece { e.f67 } else { e.f68 } as i64)),
            // ⚠ The (5,21) DEVIL's `byte_0x44_68` lives in f26, not f69
            // (mc2/roster.rs:2545) — the inverse of the importer's
            // `(5, 21) => r.b44` arm.
            (
                "b44",
                some(if piece {
                    e.f68 as i64
                } else if devil5 {
                    e.f26 as i64
                } else {
                    e.f69 as i64
                }),
            ),
            ("action45", some(e.tick70 as i64)),
            ("b46", some(e.f71 as i64)),
            ("b47", None),
            ("sv1", some(held.map_or(0, |h| h.slot as i64))),
            (
                "sv2",
                if c == 5 {
                    some(e.site_z as u8 as i64)
                } else {
                    None
                },
            ),
            (
                "sv_timer",
                if pyramid {
                    some(e.f50 as i64)
                } else {
                    held.map(|h| h.timer as i64)
                },
            ),
            ("x", some(e.x as i64)),
            ("y", some(e.y as i64)),
            ("z", some(e.z as i64)),
            ("ayaw", some(e.f78 as i16 as i64)),
            ("apitch", some(e.f80 as i16 as i64)),
            ("aroll", some(e.f82 as i16 as i64)),
            ("afov", some(e.f84 as i16 as i64)),
            ("f5a", some(e.type86 as i16 as i64)),
            ("b5c", some(e.frame88 as i64)),
            ("b5d", some(e.frames89 as i64)),
            ("mail0.amt", some(e.mail[0].0 as i64)),
            ("mail0.src", some(untr(e.mail[0].1))),
            ("mail1.amt", some(e.mail[1].0 as i64)),
            ("mail1.src", some(untr(e.mail[1].1))),
            ("mail2.amt", some(e.mail[2].0 as i64)),
            ("mail2.src", some(untr(e.mail[2].1))),
            ("mail3.amt", some(e.mail[3].0 as i64)),
            ("mail3.src", some(untr(e.mail[3].1))),
            ("mail4.amt", some(e.mail[4].0 as i64)),
            ("mail4.src", some(untr(e.mail[4].1))),
            ("mail5.amt", some(e.mail[5].0 as i64)),
            ("mail5.src", some(untr(e.mail[5].1))),
            ("speed", some(e.f126 as i64)),
            ("min_speed", some(e.f128 as i64)),
            ("max_speed", some(e.f130 as i64)),
            (
                "d88",
                if m27 || c == 15 {
                    some(e.f136 as i64)
                } else {
                    None
                },
            ),
            (
                "mana_max",
                if m27 {
                    None // f136 holds the bolt power (@0x88); @0x8C dead
                } else if c == 15 {
                    some(e.max_life as i64)
                } else {
                    some(e.f136 as i64)
                },
            ),
            (
                "mana",
                // ⚠ This table is the exact INVERSE of `import_ent_mc2`
                // (`the_mc2_port_lane_table_inverts_the_import`), so the
                // list here must track the import's f140 diversions, NOT
                // the broader dead-@0x90 law `obs_project_mc2` grades by.
                // A class-10 model outside it that the SIM stamps with an
                // @0x2A amount reads ≠ against retail's dead 0 in this
                // dump — which is the tell that its import case is
                // missing, exactly how (10,17) was found.
                if c == 10 && c10_2a_in_f140(m) {
                    some(0) // f140 holds the @0x2A amount; @0x90 dead
                } else {
                    some(e.f140 as i64)
                },
            ),
            ("player_ent", some(untr(e.f144))),
            // ⭐⭐ THE (10,79) PIECE'S `@0x96` HOME IS `f28`, NOT `f146`
            // — the enumerated `piece` list above had a HOLE. The
            // import already diverts it (`import_ent_mc2`:
            // `e.f28 = tr(r.target96)` — "target slot f28 ←
            // word_0x96_150"), the brain reads only `f28`
            // (`mc2_castle_piece_tick`, mc2/castle.rs: "its target is
            // f28"), and this table is documented as the exact INVERSE
            // of the import — but `target96` was the one diverted lane
            // with no `piece` arm. The generic seat ALSO writes
            // `f146: tr(r.target96)`, so the lane published last
            // pair's target: the mc2l22 census showed 4,636 rows in
            // PERFECTLY SYMMETRIC pairs (14 rows `retail 611/port 0`
            // beside 14 rows `retail 0/port 611`, and so on for every
            // value) — the signature of a ONE-TICK-STALE register, not
            // of a defect. INSTRUMENT-ONLY, like the `max_life` i32/u32
            // artifact: `port_ent_lanes_mc2` feeds `dump-state`,
            // `explain` and the MGC_RAW_SHADOW census, never
            // `obs_project_mc2`, so this adds no grading floor.
            (
                "target96",
                some(untr(if piece && mc2_piece_target96_lane() {
                    e.f28
                } else {
                    e.f146
                })),
            ),
            ("f98", None),
            ("dest_x", some(e.dest_x as i64)),
            ("dest_y", some(e.dest_y as i64)),
            ("dest_z", if c == 5 { None } else { some(e.site_z as i64) }),
            ("ptr_a0", None),
            ("ptr_a4", None),
        ])
    }

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
        // Retail's wizard +50 is the live [`Gen::castle_reg`]: written
        // only by the level-up commit (:56484) and the rival's direct
        // plant (:19206), cleared by the teardown to level 0
        // (:56534). The old established-level pool scan could not
        // represent a bound-at-plant level-0 castle (mc1l5 t=14771)
        // nor a fresh unbound flag (mc1l0 t=562) simultaneously.
        let castle_of = |wiz: usize| -> u16 { self.g.castle_reg[wiz & 7] };
        // A CORPSE'S HANDS ALIAS THROUGH THE POOL. The raw +940/+944
        // registers survive the death untouched, but the list they
        // index has been rewritten to MODEL numbers by the landing
        // (:55523) — and a class-12 token's model IS its spell id —
        // so retail's resolution (and the comparator's `hand_spell`,
        // the same walk) reads the book entry as a POOL SLOT: the
        // dead-window hand shows pool slot #(spell id in hand) —
        // class 12 there → Some(its model), anything else → None.
        // mc1l42's window (spell 3 in hand, slot 3 a class-2 static)
        // reads None; mc1hwl0-pd's (spell 16, slot 16 a (12,6) token)
        // reads Some(6) — the old blanket None passed the former by
        // coincidence. The registers themselves stay untouched:
        // clearing them would lose what the respawn hands straight
        // back (mc1l42 t=17343 vs t=17397, the mirrored pair).
        let corpse = self.player.state == LifeState::Dead;
        let spell_u16 = |s: Option<SpellId>| -> Option<u16> {
            let s = s?;
            if corpse {
                let e = self.g.ent.get(s.0 as usize)?;
                (e.class64 == 12).then_some(e.model65 as u16)
            } else {
                Some(s.0 as u16)
            }
        };
        let wizards: Vec<WizardMc1> = (0..8u16)
            .map(|i| {
                let localw = i == pin.local;
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
                    castle: castle_of(i as usize),
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

    /// The conformance RAW-lane projection: per-slot f26 (the burst/
    /// level lane) plus the per-wizard charge meters. These never
    /// entered the recorder's obs schema (adding them would break
    /// `check-decode` against the whole corpus), so the comparator
    /// reads them from the raw state channel instead — this is the
    /// port-side half of that comparison.
    pub fn charge_lane_mc1(&self) -> (Vec<(u16, i16)>, [u8; 8]) {
        let f26 = self
            .g
            .ent
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, e)| e.class64 != 0)
            .map(|(s, e)| (s as u16, e.f26))
            .collect();
        (f26, self.wiz_charge)
    }

    /// The SPRITE lane (player-banked instrument change 2026-08-27,
    /// landed session 68): per-slot `type86` — MC1's `+86` row, MC2's
    /// `f5a` — the visual-only lane the flag-recolor bug class lives
    /// in (a certified take and a white-flag regression could coexist
    /// because no grader ever compared the row). Same raw-channel
    /// story as `charge_lane_mc1`: the obs schema is check-decode-
    /// locked, so the comparator reads both halves off the raw state.
    /// Yields (slot, type86, class, model) — the class/model ride
    /// along so the appender can skip disagreeing slots (those are
    /// the graded diff's missing/extra/desync story).
    pub fn sprite_lane(&self) -> Vec<(u16, u16, u8, u8)> {
        self.g
            .ent
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, e)| e.class64 != 0)
            .map(|(s, e)| (s as u16, e.type86, e.class64, e.model65))
            .collect()
    }

    /// Apply a decoded MC2 retail closure onto this (already-built,
    /// same-level) world. The MC2 twin of [`World::retail_import_mc1`]
    /// — same shape: overwrite the pool, rebuild the tile lists and
    /// the free stack, seed the globals and the human column, clear
    /// the cross-pair latches.
    pub fn retail_import_mc2(&mut self, st: &RetailMc2) -> Result<ImportReport, String> {
        self.strict_retail = true;
        self.patches = crate::patches::WorldPatches::RETAIL;
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
        // Pose registers from the closure (see the MC1 twin): the
        // first post-import tick's previous-position reads must see
        // the RECORDED carpet, not a stale pose.
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.human_pose_prev = self.human_pose;
        self.human_yaw = carpet.yaw as u16;
        self.human_yaw_prev = self.human_yaw;
        // Cast-charge meters (wizext `byte_0x154_340`) — the MC1
        // twin's seeding law: unseeded, every projectile spawned
        // inside a pair banks a made-up charge in its @0x10. The
        // byte was in the capture all along (the per-player block
        // embeds the wizext at +998 and reaches +2103); only the
        // decoder was missing it, so the earlier CAP prior is gone.
        self.wiz_charge = [0; 8];
        for (i, p) in st.players.iter().enumerate().take(8) {
            self.wiz_charge[i] = p.charge;
        }
        // ⭐ A PENDING MAIL IS A SLOT NUMBER, AND AN IMPORT REPLACES
        // WHAT THE NUMBERS NAME. The ladder mail is our stand-in for
        // the INLINE second half of `sub_60780` (retail re-prices the
        // owner's Create-Castle manifestation right there at the
        // castle, so the record is a castle by construction). The
        // level build pushes one entry per authored castle and the
        // drain is post-walk, so those entries are still queued when
        // this import overwrites the whole pool — after which the
        // number names whatever landed in that slot. mc2l6-rsg: the
        // build queued slot 367 as a level-0 (3,2); the import made
        // 367 a (15,23) manifestation owned by the HUMAN, which still
        // passes the drain's owner test, so every pair re-priced the
        // human's Create-Castle token against a phantom rung-0 castle
        // — max_life/f140 1500/14 (`MC2_CASTLE_COST[0]·384>>8`) where
        // retail's own level-1 castle says 15000/148, from the tick
        // the castle stood to the end of the take.
        // ⚠ DROP THE MAIL, DO NOT FILTER IT AT THE DRAIN: the drain
        // legitimately serves castles that are already dead by then —
        // the level-0 downgrade re-prices at the rung-0 run through a
        // freed record (mc2l3's death-downgrade / balloon-pop /
        // un-stamp fixtures all pin that path).
        self.g.mc2_ladder_sync.0.clear();
        self.g.mc2_ladder_sync.1.clear();
        self.g.mc2_castle_lock_mail.0.clear();
        // THE FIRING HAND (`struct_byte_0xc_12_15` & 0x300) — the MC1
        // twin's line verbatim (:377). `sub_5F7B0` stamps it on the
        // CASTER at the arm (EF:60977-78) and `sub_68E50` reads it
        // back at every spawn to place the 256-unit lateral muzzle,
        // and the arm sits a walk pass BEFORE the token that fires
        // (every recorded spell token slots below the carpet), so a
        // pair-mode world never runs the arm that would set it. It is
        // in the capture because retail's home for it is the carpet.
        self.hand_bits = carpet.flags & 0x300;
        // The human's PARALYZE latch — see `Gen::mc2_mobilize`. The
        // driver's flight ext owns it; a pair-mode world has no ext
        // history, so seed the mirror from the capture.
        self.g.mc2_mobilize.0 = ply.mobilize as i32;
        // …and the WEB-SLOW level beside it (`moveSpeed_0x14C_332`),
        // which `sub_38E70` reads to decide whether the stagger stamp
        // kicks and draws at all. Same reason: no ext history in a
        // pair-mode world.
        self.g.mc2_slow.0 = ply.move_speed as i32;
        let tr = |v: u16| if v == human_slot { PLAYER_TARGET } else { v };

        // The MANA-BALLOON REGISTER (`array_0x3C_60`, +998+60) — the
        // MC1 twin's law verbatim (`Type_160 +52`): the register's
        // INDEX order is spawn order and is NOT recoverable from a
        // pool census, and it is what decides which balloon claims
        // which sphere and which one the cull frees. The KEY is the
        // owner stamp the dispatcher reads off the castle (@0x1A), so
        // the human's carpet slot goes through `tr`.
        self.g.mc1_balloon_reg.0.clear();
        for p in st.players.iter().take(8) {
            if p.play_index == 0 {
                continue;
            }
            self.g
                .mc1_balloon_reg
                .0
                .insert(tr(p.play_index), p.balloons.to_vec());
        }

        // The CASTLE-GUARD REGISTER (`array_0x5C_92`, +998+92) — the
        // fleet register's twin one field along, and imported the same
        // way for the same reason: its STALE entries are retail-only
        // memory of dead guards, so the MC1 side's rebuild-from-census
        // cannot recover them (it says so, and eats a divergence for
        // it). MC2's capture carries the whole owner block, so take it
        // verbatim. Without this the register imports EMPTY and every
        // castle mints a guard on its first pass — five mc2l0 fixtures
        // regressed on exactly that (`extra:5,15`, plus three more that
        // are only the free-stack pop order rotating behind it).
        self.g.mc1_guard_reg.0.clear();
        for p in st.players.iter().take(8) {
            if p.play_index == 0 {
                continue;
            }
            self.g.mc1_guard_reg.0.insert(
                tr(p.play_index),
                p.guards.0.iter().map(|&s| tr(s)).collect(),
            );
        }

        // Anchor the per-tick counter to the recording: it feeds the
        // cave-drip 8-turn cadence AND the cave carpet-tail rand
        // perturbation (World::tick) — both key on its POST-increment
        // value. Retail resets it at level load, so the local
        // player's Turn is its exact value. The carpet's byte[1]&8
        // one-shot (EF:59616) arms the tail skip, and so do the
        // action arms that never call the mover `sub_5D530`: only
        // flying (0, EF:59994) and the death-test arm (2, EF:60074)
        // reach it — the level-end arm (12, mc2l30 t=9090..) parks
        // the tail entirely, and possession holds byte[1]&8 across
        // its whole window (t=3257-3267).
        self.mc2_turn = ply.turn.max(0) as u32;
        self.mc2_carpet_slot = human_slot;
        // …and the Gen-level mirror, for the `sub_49F90` rebuild
        // sites that live below the World layer (`Gen::mc2_eject_gc`).
        self.g.mc2_pinned = crate::engine::features::Mc2Pinned(human_slot);
        self.mc2_carpet_stall = carpet.flags & 0x800 != 0 || !matches!(carpet.action45, 0 | 2);

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
        // Slot 0 is the SCRATCH record and it is real recorded state
        // (see [`no_mc2_seat_slot0`]): it is class 0, so it takes the
        // freed-slot arm below, joins no tile chain and is counted on
        // neither census — exactly the MC1 importer's shape.
        let first = usize::from(no_mc2_seat_slot0());
        for slot in first..n {
            let r = &st.ents[slot];
            if slot == human_slot as usize {
                self.g.ent[slot] = Ent::default();
                // ⭐⭐⭐ THE HUMAN'S PINNED SEAT HAD NO IMPORT LANE FOR
                // THE WHIRLWIND LATCHES (dig Q7 — the lane census's
                // "a port register with NO import seat"). The port's
                // representation zeroes this record deliberately (see
                // `mc2_spawn_human_record`), which is right for class,
                // pose and links — but `sub_33340`'s victim body keeps
                // FOUR live words on it, and every one of them is in
                // the capture: the grab latch `byte[3] & 0x10`
                // (`flags & 0x1000_0000`), the mover-veto one-shot
                // `byte[1] & 8` (`flags & 0x800` — the very bit two
                // lines above already reads into `mc2_carpet_stall`),
                // the latched swirl heading `word_0x30_48` and the
                // per-entity LCG `rand_0x14_20`. Without them every
                // segmented reset handed the funnel an UNGRABBED
                // wizard and it re-ran the inner LIFT arm on a victim
                // retail was already dragging: mc2l30 t=2986 is the
                // exemplar (retail `flags 268438029`, bit 28 set,
                // steps the near-grab +114/+56/128; the port re-lifted
                // him onto the eye).
                if !crate::mc2::tail::no_mc2_ww_human_grab() {
                    let e = &mut self.g.ent[slot];
                    if r.flags & 0x800 != 0 {
                        e.flags |= crate::mc2::mobs::F_STOP;
                    }
                    if r.flags & 0x1000_0000 != 0 {
                        e.flags |= crate::mc2::tail::F_GRABBED;
                    }
                    e.f50 = r.f30 as i16;
                    e.rand = r.rand as u32;
                }
                continue;
            }
            if r.class3f == 0 {
                // A freed slot is not an EMPTY slot (the MC1
                // importer's blind-tracker law, same block shape):
                // retail's free path clears the class byte and pushes
                // the stack — every other byte stays, and blind
                // target reads steer at whatever the record still
                // holds (mc2l3 t=354: balloon 162 chases freed sphere
                // 247's stale position; a defaulted slot re-aims it
                // at the origin). Import the stale bytes, class 0,
                // not counted active.
                //
                // ⭐⭐⭐ AND THE ROW IS PART OF "EVERY OTHER BYTE
                // STAYS". This arm used to hand the record the
                // bad-row stand-in 59 with the note "nothing live
                // dereferences a freed row" — a comment claiming a
                // lane is swept. `dword_0xA0` is written ONLY by the
                // 72 `c7 8? a0 00 00 00 <imm32>` ctor stores in
                // `NETHERW.EXE` (rows 59..=106) and by the loader
                // fixup (Level.cpp:1255-57). The image's other two
                // +0xA0 stores — file 0xcbd06 `c7 80 a0 00 00 00 00
                // 00 00 00` and the ONE register form, file 0xcd683
                // `89 83 a0 00 00 00` after `8b 82 e0 00 00 00` —
                // both address a STRIDE-4 array (`8d 04 85 00 00 00
                // 00` + fields at +0x20/+0x60/+0xa0/+0xe0), not the
                // 0x160-stride entity pool. And the free path
                // `sub_57F20` (Events.cpp:5236-38) touches ONE byte —
                // file 0x7c78a `c6 43 3f 00` (`movb $0x0,0x3f(%ebx)`)
                // then `8b 50 35` / `42` / `89 50 35` /
                // `89 9c 90 46 02 00 00` (the free-stack push) and
                // `c3`. So a freed record keeps the row its ctor
                // stamped until the slot is re-allocated, where
                // `NewEvent_4A050` re-stamps row 59 (file 0x6e955,
                // Events.cpp:573/599) and the new ctor may overwrite.
                // Decode it like the live arm and fall back to 59 only
                // when the pointer does not decode (a never-allocated
                // slot's zero, and the slot-0 scratch record).
                let row = if no_mc2_freed_row_import() {
                    59
                } else {
                    decode_row156(r.ptr_a0, st.base160).unwrap_or(59)
                };
                self.g.ent[slot] = import_ent_mc2(r, slot as u16, row, &tr);
                continue;
            }
            if !ghost(r) {
                active += 1;
            }
            // Behavior row: `ptr_a0` points into `str_D7BD6[]`;
            // retail's own load fixup is `(ptr − base160)/34 + 59`
            // (Level.cpp:1255-57; base160 = the saved `&str_D7BD6[59]`).
            // This ABSOLUTE `str_D7BD6` index is what every MC2 tick
            // reads via `BEHAVIOR[row156]`.
            // ⚠ ABSOLUTE, WITH NO EXCEPTIONS. The balloon used to be
            // one: `mc2_balloon_tick` indexed `BEHAVIOR[ROW_BASE +
            // row156]` off a RELATIVE `mc2_spawn_balloon` stamp, so
            // the generic absolute import (68, the ctor `sub_4ABA0`'s
            // `&str_D7BD6[68]`, EF:33422) double-offset to
            // `BEHAVIOR[127]` (v_12 = 0, v_14 = −128) and sank every
            // imported balloon 128/tick. Both sides are absolute now.
            let row156 = match decode_row156(r.ptr_a0, st.base160) {
                Some(row) => row,
                None => {
                    bad_rows += 1;
                    59
                }
            };
            self.g.ent[slot] = import_ent_mc2(r, slot as u16, row156, &tr);
        }
        for slot in n..pool {
            self.g.ent[slot] = Ent::default();
        }

        // ⭐⭐⭐ THE DOOMSDAY BEAM RAMP IS A GLOBAL, AND IT HAD NO
        // IMPORT SEAT AT ALL. `sub_21AB0` case 7 (EF:13431-40) steps
        // `D41A0_0.word_0x36546` — not the pyramid's own record:
        //     if (subSpellIndex_0x2A_42 & 2) D41A0_0.word_0x36546 = 1024;
        //     D41A0_0.word_0x36546 -= 80;
        //     if (< 10) = 10;  if (> 1024) = 1024;
        // The port homes that word on the (5,10)'s `f52`, and the
        // table above seats `f52` from @0x32 — a lane retail leaves 0
        // on the pyramid for the level's whole life. So EVERY imported
        // pair handed the beam a ramp of 0 and the `-80` step floored
        // it on the spot: retail's 944, 864, ..., 64 read back as 10.
        // The capture held the word all along (two bytes below the
        // `stages_0x3654C` table the importer already reads).
        // ⚠ The port's `f52` therefore does NOT publish @0x32 for a
        // pyramid — see `World::port_ent_lanes_mc2`.
        if !no_mc2_doom_beam_import() {
            for e in self.g.ent[1..n].iter_mut() {
                if e.class64 == 5 && e.model65 == 10 {
                    e.f52 = st.doom_beam;
                }
            }
        }

        // Tile lists: the per-tile head array (`mapEntityIndex_15B4E0`)
        // lives OUTSIDE `D41A0_0` (a separate SMAP global the recording
        // does not carry) — but the per-entity chain words ARE captured
        // (`next16` @0x16 = the walk lane `sub_108B0`/`sub_10780`
        // iterate, `prev18` @0x18 = 0 at the head), so the RECORDED
        // order rebuilds exactly, the MC1 shape (see the MC1 importer
        // above). Chain order is load-bearing: the first-hit probes
        // return the first admissible entity in walk order (mc2l3
        // t=268: the possession bolt's endpoint cell held firebug 152 →
        // sphere 137 → sphere 241; retail claims 137, the old ascending
        // rebuild put 241 at the head and claimed it instead — the
        // first divergent pair). Walk each recorded chain from its head
        // and link in reverse (head-insertion restores the walk order);
        // slots left unreachable by torn links keep the ascending
        // fallback.
        for h in self.g.map_entity.iter_mut() {
            *h = 0;
        }
        // Ghosts never link: retail unlinks at disable — the
        // record's link bit is stale bytes. A linked ghost whose
        // slot is later reallocated leaves a dangling chain
        // pointer (a tile-chain CYCLE once the new occupant
        // relinks on the same tile — the pair-9074 OOM).
        let linkable = |r: &RetailEntMc2| r.class3f != 0 && r.flags & 4 != 0 && !ghost(r);
        // ⭐⭐⭐ DIG W2-F — SPLICE the ghosts out instead of TRUNCATING
        // at them. A ghost at a chain's head orphaned every record
        // below it into the ascending fallback, which head-inserts in
        // slot order and hands the chain back REVERSED (86% of
        // mc2l22's whole ungraded census). See [`chain_ghost_splice`]
        // for the citation, the mc2l22 320→321 witness and the
        // measured numbers. Both legs walk THROUGH a non-linkable
        // record — exactly what retail's own tick-top reap leaves
        // behind — and the step cap is the torn-capture guard.
        // ⚠ Slot 0 stays out of `1..n`: it is the chain's NULL
        // sentinel (`map_entity == 0` means "empty tile"), not a
        // skipped law surface — unlike the import loop above, which
        // round 105 moved to `0..n` for the scratch record.
        let splice = chain_ghost_splice();
        let hop = |mut s: usize, back: bool| -> usize {
            let mut guard = 0usize;
            while s != 0 && s < n && !linkable(&st.ents[s]) {
                s = if back {
                    st.ents[s].prev18 as usize
                } else {
                    st.ents[s].next16 as usize
                };
                guard += 1;
                if guard > n {
                    return 0;
                }
            }
            if s >= n { 0 } else { s }
        };
        let mut seen = vec![false; n];
        let mut chains: Vec<Vec<usize>> = Vec::new();
        for head in 1..n {
            let r = &st.ents[head];
            let prev = if splice {
                hop(r.prev18 as usize, true)
            } else {
                r.prev18 as usize
            };
            if !linkable(r) || prev != 0 || seen[head] {
                continue;
            }
            let mut chain = Vec::new();
            let mut cur = head;
            loop {
                if seen[cur] {
                    break; // cycle guard — torn capture
                }
                seen[cur] = true;
                chain.push(cur);
                let next = st.ents[cur].next16 as usize;
                let next = if splice {
                    hop(next, false)
                } else if next == 0 || next >= n || !linkable(&st.ents[next]) {
                    0
                } else {
                    next
                };
                if next == 0 {
                    break;
                }
                cur = next;
            }
            chains.push(chain);
        }
        for chain in &chains {
            for &slot in chain.iter().rev() {
                let e = &self.g.ent[slot];
                let (x, y, z) = (e.x, e.y, e.z);
                self.g.link(slot, x, y, z);
            }
        }
        for slot in 1..n {
            if seen[slot] {
                continue;
            }
            if linkable(&st.ents[slot]) {
                let e = &self.g.ent[slot];
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
        // NEXT frame's top reap (UpdateEntities EF:39948-56) unlinks,
        // class-zeroes and pushes it (measured: the t=1 snapshot's
        // stack is exactly the ghost count short, and the reused
        // slots pop highest-first = an ascending push scan). tick()'s
        // top reap performs that push for strict MC2 — the import
        // only counts ghosts for the census; appending them here too
        // would double-push the slots.
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
        // The recycle-victim stack rides along, order preserved, so a
        // full-pool spawn sacrifices the SAME live entity retail's
        // `NewEvent_4A050` fallback would (:581). `refill` stays clear:
        // the recorded stack is retail's own snapshot, and running it
        // dry is retail returning null, not a cue to re-rank the pool.
        // Ghosts are excluded — they are still class-bearing here, but
        // `tick()`'s top reap pushes them onto the FREE stack, and a
        // slot on both stacks could be handed out twice.
        //
        // `MGC_NO_RECYCLE_VICTIM=1` is the A/B toggle: it leaves the
        // stack empty, i.e. the pre-dig port that simply fails every
        // full-pool spawn. The runner's measurements are taken unset.
        // ⭐⭐⭐ AND AN "EMPTY" RECORDED VICTIM STACK ON A FULL POOL WAS
        // A DECODER ARTEFACT, NOT "retail returned null" (dig C3,
        // session 96). `mc2_pool_base` recovers the pool base from the
        // FREE stack's cells; with the pool full that stack is empty,
        // the live-victim fallback could not pin a unique base under
        // an "is occupied" test, and BOTH stacks decoded EMPTY on
        // exactly the frames that sacrifice. mc2l22 t=10030 really
        // records `dword_0x11e6` = 66 — a 67-deep stack reading
        // bottom-up 998, 997, 995 … 867, 211, 860, 859, 858, 854,
        // i.e. `sub_49F90`'s descending push AS SCRAMBLED BY
        // `sub_57F20`'s swap-with-top — retail pops all 67 that tick
        // and t=10031 records -1. So the snapshot IS the ranking, and
        // it is NOT sorted: a synthetic `rebuild_recycle` refill pops
        // strictly ascending and picks a DIFFERENT victim set (mc2l22
        // t=10029's stack has 4, 40, 45, 57, 108, 197, 210 sitting
        // ABOVE 665/816/847/850/853). `refill` therefore stays clear —
        // running the recorded stack dry is retail running dry.
        self.g.mc2_recycle.refill = false;
        let no_victims = std::env::var_os("MGC_NO_RECYCLE_VICTIM").is_some();
        self.g.mc2_recycle.stack = st
            .recycle_stack
            .iter()
            .copied()
            .filter(|&s| {
                if no_victims {
                    return false;
                }
                let live = (s as usize) < pool && s != human_slot && s != 0;
                live && {
                    let e = &self.g.ent[s as usize];
                    // ⭐⭐⭐ A GHOST STAYS ON THE VICTIM STACK, AND THE
                    // TICK-TOP REAP IS WHAT TAKES IT OFF — BY
                    // SWAP-WITH-TOP. Dropping ghosts HERE removes them
                    // ORDER-PRESERVINGLY, which is not the same list:
                    // retail's `sub_57F20` (Events.cpp:5215-35) writes
                    // `dword_0x11EA[hole] = dword_0x11EA[top]` and
                    // decrements, so every ghost the next frame reaps
                    // pulls the CURRENT TOP down into its hole. The
                    // port's own tick-top reap (`world.rs` ~:4566 ->
                    // `free_slot` -> `Gen::free_entity`) performs
                    // exactly that swap_remove, in the same ascending
                    // slot order retail reaps in — but only for cells
                    // that are still ON the stack. See
                    // [`recycle_ghost_keep`].
                    e.class64 != 0 && (recycle_ghost_keep() || e.flags & 0x400 == 0)
                }
            })
            .collect();

        // NO ghost push here — see the census note above: `tick()`'s
        // strict-MC2 top reap is the ONE pusher (measured 2026-08-03,
        // mc2l24 pair 53808: the extra `extend` left the pyramid's
        // 17-slot worm chain popping [905, 837, 813, 796, 727, 690]
        // TWICE, so the second pop of 905 re-`NewEvent`ed the chain's
        // own HEAD — `Ent::default()` over a live record — and the
        // whole chain projected as class 0. Retail's stack is exactly
        // the recorded 716 + the 6 ghosts the reap pushes.)

        // Globals in the closure.
        self.g.rand = st.rand;
        self.g.mc2_spawn_ord.0[..29].copy_from_slice(&st.spawn_ord);
        // Outside the closure: the retile LCG (pseudo) has no capture.
        self.g.pseudo = 0;
        // The volcano-vortex / fire-column singletons (D41A0 word_0x31
        // / word_0x33, header +0x31/+0x33) ARE captured for MC2. The
        // (10,18) re-eruption reset (`sub_32A70`, EF:23924) gates on
        // word_0x31 being clear, and it is NOT reconstructable from
        // entity state: the persistent controller reads it 0 before
        // re-erupting and its own slot afterwards, with an identical
        // entity record either way. A forced 0 makes it re-erupt on
        // every >2500 roll where retail actually holds the latch
        // (mc2l30 slot 134 after t=2536, ~13 phantom eruptions). Both
        // are 0 on non-volcano levels, so mc2l0/l4 are unaffected.
        self.g.erupting = st.vortex;
        self.g.plume = st.fire_col;

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
        // LAW A — THE THING TABLE IMPORTS VERBATIM
        // (`entity_0x30311[1200]`, decoded in mgcr.rs): the whole
        // authored table rides every capture and its only runtime
        // write is the consumption zero (`sub_4A1E0(id, 1)`), so the
        // import carries retail's own consumption state to the pair —
        // a row retail has spent reads type 0, a row it still holds
        // re-arms even if an earlier port pair mis-tripped it. The
        // runner's post-build re-imprint (verify_mc2.rs) still runs
        // first and is now superseded by this overwrite (kept
        // load-bearing for the MC1 column, which has no such lane).
        // MC2's table base is 0, so port index == retail row.
        for (k, raw) in st.things.iter().enumerate() {
            if let Some(rec) = self.table.get_mut(k) {
                *rec = crate::engine::features::Rec {
                    class: raw[0],
                    model: raw[1],
                    x: raw[2],
                    y: raw[3],
                    dis_id: raw[4],
                    swi_sz: raw[5],
                    swi_id: raw[6],
                    parent: raw[7],
                    child: raw[8],
                    par3: raw[9],
                };
            }
        }
        // THE OBJECTIVE BOARD (`struct_0x3659C[local]`,
        // LevelStructs.h:190-196). Retail's per-row state, the
        // current-row cursor, the m32 one-pass pause and the level-end
        // latch are GLOBALS, not entity state — `sub_58F00_game_
        // objectives` (EF:40693) reads and writes them at the frame
        // tail and the class-11 model-32 switches gate on
        // `stage_0x3659F[par1] == 2` (EF:54369). Without this import
        // the port's board FREE-RAN from `set_mc2_stages` on every
        // pair and from level load in the free replay, so a
        // stage-gated disposition fires on the port's own trajectory
        // instead of retail's: mc2l3 t=356 lost the whole dis-6 wave
        // (17 records missing in port, and with them slot 53 — retail
        // spends it on the wave's first ring, the port hands it to the
        // human's possession bolt). `mc2_stages[k].row == k` by
        // construction (`set_mc2_stages` — the baker has already
        // compacted the -1 rows), so k IS retail's row index.
        if let Some(board) = st.objectives.get(local) {
            self.completed = board[0] != 0;
            self.mc2_stage_current = board[1] as usize;
            self.mc2_objective_pause = board[2] as i16;
            for (k, s) in self.mc2_stages.iter_mut().enumerate().take(8) {
                s.state = board[3 + k];
                // Law B1 — the BIND TABLE (`stages_0x3654C[k]`,
                // decoded in mgcr.rs): `bound` and `force` were
                // port-carried, so every anchored run's type-1/2
                // rows were DEAD (the bind seam only fires on a
                // spawn the anchor already spent). `flags & 1` is
                // retail's own bound bit; the slot converts from
                // the row's guest pointer.
                let (_kind, flags, slot) = st.stage_binds[k];
                s.force = flags & 2 != 0;
                if matches!(s.kind, 1 | 2) {
                    // ⭐⭐⭐ ROUND 98 — **AN UNREADABLE FIELD IS NOT A
                    // CLEARED FIELD.** `stage_binds[k].2` is `None` in
                    // TWO completely different situations and this
                    // line used to collapse them:
                    //   (a) retail's row is genuinely unbound
                    //       (`flags & 1` clear), and
                    //   (b) retail's row IS bound but
                    //       `mgcr::mc2_pool_base` could not recover the
                    //       pool base, so the guest pointer at +6 could
                    //       not be converted.
                    // (b) is not rare and it is not random: the base is
                    // recovered from the FREE stack's pointer cells and,
                    // on a full pool, from the RECYCLE stack's
                    // (`mgcr.rs:2537-59`) — so a frame on which BOTH
                    // stacks are empty has no pointer set left to
                    // recover from, and EVERY bind on that frame reads
                    // unreadable. That is exactly the state a level
                    // reaches while the human is razing the objective
                    // buildings: the debris fills the pool, both stacks
                    // drain, and the pair importer then hands the port a
                    // board whose type-1/2 rows are UNBOUND on precisely
                    // the ticks those objectives complete. `objective_
                    // mc2`'s type-1/2 arms are `st.bound.is_some_and(..)`,
                    // so the row cannot latch and the port completes it
                    // one tick late — mc2l22 rows 2 and 3 (t=13102,
                    // t=15393), found by the OBJECTIVE-BOARD shadow the
                    // moment that lane was first graded.
                    // Keeping the port's carried bind is the strictly
                    // better reading: in pair mode it is the previous
                    // pair's import, which was itself retail's.
                    // `MGC_NO_STAGE_BIND_KEEP_UNREADABLE=1` restores the
                    // clearing form for A/B.
                    s.bound = match (flags & 1 != 0, slot) {
                        (false, _) => None,
                        (true, Some(sl)) => Some(sl),
                        (true, None) if stage_bind_keep_unreadable() => s.bound,
                        (true, None) => None,
                    };
                }
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
            // A materializing m9's PARKED hold: retail keeps it in
            // the record's own @0x4A (sub_12100 EF:4716-22 stamps
            // the SLOT there when a3 = model==9), consumed by the
            // completion tick's sub_122A0. sv1==0 + action 72 +
            // @0x4A in 1..=10 is the only state where @0x4A holds a
            // slot rather than a kind-6 countdown or a watch handle
            // — without the rebuild the park is destroyed and the
            // deferred arm can never fire in pair mode (mc2l4 t=9).
            if r.class3f == 5
                && r.model40 == 9
                && slot != human_slot as usize
                && !ghost(r)
                && r.sv1 == 0
                && r.action45 == 72
                && (1..=10).contains(&r.sv_timer)
            {
                self.mc2_sv_deferred.push((slot as u16, r.sv_timer as u8));
            }
        }

        // Per-player columns: pool wizard slots + WANTED timers.
        // MC2's wanted table keys on the wizard's ENTITY slot
        // (`mc2_wanted`, hash-quiet while empty); MC1's per-player
        // `rival_wanted` array stays zero.
        self.g.mc2_wanted.0.clear();
        // ⭐⭐⭐ THE ALLIANCE CHARM'S PARENT HAD NO IMPORT SEAT. Retail
        // keeps it in the victim's own `parentId_0x28_40` (`sub_3A650`
        // EF:29680) — a RECORD FIELD the importer already carries as
        // `r.owner28` — while the port keeps it in the `mc2_allied`
        // side map, which this `clear()` then emptied on every single
        // pair. `mc2_alliance_clock` reads that map: parent 0 ⇒
        // `parent_dead` ⇒ the charm ENDS. So every imported charmed
        // creature dropped `sv2` 14 → 10 on its first ticked pair, no
        // matter how long retail's lease still had to run
        // (mc2l0-spells-galore t=23948, the three (5,4) archers 559/
        // 621/627: retail `sv2` 14 with `f2e` counting 608, the port
        // 10). ⚠ NOT an id24 fusion like sv2 12/13: those two are born
        // owned, and the charm's victim is a PRE-EXISTING creature
        // whose `@0x1A` retail leaves alone (559 stays 559 across the
        // whole charm), so fusing would corrupt the `f1a` lane.
        self.g.mc2_allied.0.clear();
        if !crate::mc2::mobs::no_mc2_alliance_parent_seat() {
            for slot in 1..n {
                let r = &st.ents[slot];
                if r.class3f == 5 && r.sv2 == 14 && r.owner28 != 0 && !ghost(r) {
                    self.g.mc2_allied.0.insert(slot as u16, tr(r.owner28));
                }
            }
        }
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
        // THE CASTLE REGISTER (`CastleEntityIndex_0x3A_58`, +998+58)
        // — imported RAW, like the MC1 wizext+50 twin above, and for
        // the same reason: it is NOT derivable from the pool. A
        // player mid-SPLIT has two live castles and a word naming one
        // of them; a player whose orphan just died has a live castle
        // and a word naming NOTHING. Only the capture knows which.
        // Must run AFTER `rival_ents`, which `owner_team` reads.
        for t in 0..8 {
            self.g.castle_reg[t] = 0;
        }
        for (i, p) in st.players.iter().enumerate().take(8) {
            if p.play_index == 0 {
                continue;
            }
            let owner = if i == local { PLAYER_TARGET } else { tr(p.play_index) };
            if let Some(team) = self.g.owner_team(owner) {
                self.g.castle_reg[team as usize] = p.castle_ent.max(0) as u16;
            }
        }
        // MC2 rival re-anchor — the MC1 rival-freeze twin: the
        // class-3 dispatch keys on `mc2_rivals[ri].ent`, which the
        // world-build seeded with fresh spawn slots, so every
        // imported rival carpet replayed as a frozen husk (the mc2l4
        // (3,1) family: obs@1 == state@0 verbatim for the wizard's
        // whole life — the motion law itself is verbatim EF:6484).
        // The DECISION half follows in `reanchor_mc2_rival_ai`: the
        // wizard-extension brain lanes plus the two that ride the
        // wizard entity, so the replayed rival resumes retail's
        // decision instead of re-running the cascade.
        //
        // `SpellIndexLeft/Right` are DIRECT spell indices in MC2 (-1 =
        // empty) — shared by the rival books here and the human's
        // below.
        let book_hand = |raw: i16| {
            if (0..26).contains(&raw) {
                raw as i8
            } else {
                -1
            }
        };
        for ri in 0..self.mc2_rivals.len() {
            let slot = self.mc2_rivals[ri].slot as usize;
            match st.players.get(slot) {
                Some(p) if slot != local && p.play_index != 0 => {
                    let ent = tr(p.play_index);
                    let e = &st.ents[p.play_index as usize];
                    self.reanchor_mc2_rival(
                        ri,
                        ent,
                        p.cmd_speed,
                        p.strafe,
                        p.invuln.max(0) as u16,
                        if crate::mc2::rivals::no_rival_mana_overdraft() {
                            e.mana.max(0)
                        } else {
                            e.mana
                        },
                        e.mana_max.max(0) as u32,
                        e.d88,
                    );
                    let ai = crate::mc2::rivals::Mc2RivalAi {
                        state: p.ai_state,
                        target: tr(e.target96),
                        target_sig: e.f98,
                        site: (e.dest_x, e.dest_y),
                        burst: p.burst,
                        poverty: p.poverty,
                        cooldown: p.cooldown,
                        hate: p.hate,
                        war: p.war,
                        weave: p.weave.max(0) as u8,
                        weave_dir: p.weave_dir.max(0) as u8,
                        avoid: p.avoid.max(0) as u8,
                        avoid_exit: p.avoid_exit.max(0) as u8,
                        aggression: p.aggression.max(0) as u16,
                        perception: p.perception.max(0) as u16,
                        reflexes: p.reflexes.max(0) as u16,
                        life_scale: p.life_scale.max(0) as u16,
                        brake: p.brake,
                        life_regen: p.life_regen,
                        knock_dir: p.knock_dir,
                        knock_mag: p.knock_mag,
                    };
                    let book = crate::mc2::cast::Mc2Spellbook {
                        ent: p.spell_ent,
                        xp_vol: p.xp_vol,
                        xp_bank: p.xp_bank,
                        levels: p.levels,
                        sel: p.sel,
                        left: book_hand(p.hand_left),
                        right: book_hand(p.hand_right),
                        ring: p.ring,
                    };
                    self.reanchor_mc2_rival_ai(ri, &ai, &book);
                }
                _ => self.reanchor_mc2_rival(ri, 0, 0, 0, 0, 0, 0, 0),
            }
        }
        self.g.player_aggro = ply.wanted;
        self.g.player_danger = carpet.f36 as i16;
        self.g.player_mail = carpet.mail.map(|(a, s)| (a.max(0) as u32, tr(s)));
        self.g.player_invisible = carpet.flags & 0x20 != 0;
        // D11 PROBE
        if std::env::var_os("MGC_NO_MC2_REBOUND_SEAT").is_none() {
            self.g.player_rebound = carpet.flags & 0x8000 != 0;
            self.g.mc2_rebound_precise.0 = i32::from(carpet.flags & 0x10 != 0);
        }
        self.g.mc2_player_drain.0 = 0;

        // The human column. MC2 hands are DIRECT spell indices
        // (SpellIndexLeft/Right; −1 = empty) — no acquisition-list
        // indirection like MC1.
        // ⭐⭐⭐ …AND THE BOUND IS **MC2's 26**, NOT MC1's `SPELL_COUNT`.
        // `mc1::spells::SPELL_COUNT` is 24, so this closure silently
        // dropped MC2's spells **24 (Alliance) and 25** — the only two
        // outside MC1's book — and seated an EMPTY hand for them. The
        // `book_hand` twin eight lines down already uses `0..26` (so
        // does `mc2_select_spell`'s unbind gate), which is what made
        // the break invisible: `mc2_book.left` imported 24 correctly
        // while the `Player` mirror the obs lane is projected from
        // imported `None`, and the two are supposed to be one register
        // (see `Gen::mc2_set_hand`). FREE RUN never sees it — the port
        // carries its own hand across ticks — so it is a PAIR-ONLY
        // divergence: mc2l0-spells-galore t=23983..24560, EVERY pair
        // dirty on `player0.hand_left` (retail Some(24), port None)
        // for as long as the human holds Alliance in the left hand.
        // SPELLS.DAT's spell-6 tier `life_0x1A` column (0 / 0 / 1) —
        // the key `sub_6A480` forks its two statement orders on. Read
        // here so [`mc2_applied_mana_delta`] stays a free function.
        let spell6_life = self.g.assets.spells.get(6).map_or([0i8; 3], |r| {
            [r.tiers[0].life, r.tiers[1].life, r.tiers[2].life]
        });
        let hand = |raw: i16| {
            (0..crate::mc2::cast::MC2_SPELL_COUNT as i16)
                .contains(&raw)
                .then_some(SpellId(raw as u8))
        };
        self.player = Player {
            mana: carpet.mana.max(0) as u32,
            mana_max: carpet.mana_max.max(0) as u32,
            // The pending regen/debit delta (@0x88) — the value the
            // wizard body will APPLY next frame, which is NOT always
            // the recorded one: see [`mc2_applied_mana_delta`].
            mana_delta: mc2_applied_mana_delta(st, ply, human_slot, &carpet, spell6_life),
            life: carpet.life,
            // MORTALITY (the MC1 arm's twin): the human carpet's
            // `actionIndex_0x45_69` IS the wizard's life state on the
            // MC2 column too — 0 alive (`AddPlayer03_00_5E010`), 2 the
            // death fall (`sub_5E310`), 3 the corpse waiting for Space
            // (`sub_5E7C0`). Pinning `Alive` here ran the whole regen
            // block on a corpse: +maxLife/250 life and the stale
            // `manaRegen` (@0x88) both landed every corpse pair, and
            // the imported mana clamped to `mana_max` — retail's
            // corpse touches neither (EF:59994-60040 gates the block
            // on `life >= 0`).
            state: match carpet.action45 {
                _ if super::mc2_death_off() => LifeState::Alive,
                2 => LifeState::Falling,
                3 => LifeState::Dead,
                _ => LifeState::Alive,
            },
            left: hand(ply.hand_left),
            right: hand(ply.hand_right),
            grace: ply.invuln.max(0) as u16,
            // The 16-tick post-hit life-regen stall (dword_0x18D_397,
            // EF:60000-60003; armed EF:60662/60710/62222 on
            // hit/grip/steal). Unseeded, every pair inside retail's
            // stall window applied one heal quantum (5 afield, 40 at
            // castle/dolmen) retail withheld — the cross-take
            // player.life +5 family.
            regen_delay: ply.regen_stall.clamp(0, u16::MAX as i32) as u16,
            // The rate REGISTER (`lifeRegen_0x163_355`, flight +355 —
            // the MC1 arm's `u16_341` one game over): applied and only
            // THEN re-selected, so a pair straddling a rate flip must
            // inherit the stale value. The lane was decoded all along;
            // only this seat was missing, which is what forced
            // `player_regen_block`'s MC2 exception.
            life_rate: ply.life_regen as i32,
            killer: tr(carpet.f24 as u16),
            fall_speed: carpet.f2c,
            invisible: carpet.flags & 0x20 != 0,
            // D11 PROBE
            shield: std::env::var_os("MGC_NO_MC2_SHIELD_SEAT").is_none()
                && carpet.flags & 0x4000 != 0,
            // The shield's ARMED stage rides the SAME word one byte
            // up — retail `byte[2] & 0x40`, dword bit 22 (0x40_0000).
            // The pool importer already translates it for rivals
            // (`mc2::rivals::F_SHIELD_ARMED`); the human's own seat
            // was the last hole. mc2l6-rsg's carpet 343 holds
            // `flags` 0x40010D across t=22531..22705, 22730..22866
            // and 23116..23166.
            shield_armed: !crate::mc2::cast::no_mc2_shield_armed()
                && std::env::var_os("MGC_NO_MC2_SHIELD_SEAT").is_none()
                && carpet.flags & 0x40_0000 != 0,
            rebound: std::env::var_os("MGC_NO_MC2_REBOUND_SEAT").is_none()
                && carpet.flags & 0x8000 != 0,
            // ⭐ THE HUMAN'S `dword_0x10_16` (@0x10) — the death
            // respawn timer, 0 before the first death and 1200 after
            // (EF:60170; the human arm of `sub_5E7C0` never counts it
            // down). The port's home is
            // [`Player::mc2_respawn_timer`], because the human carpet
            // is OUT OF POOL and so cannot ride the entity import, and
            // its one reader is `sub_377A0`'s painter mint, which
            // stamps it into the (10,42)'s `byte_0x46_70`. Without
            // this seat a pair-mode world minted row 0 forever and
            // published `b46` 0 against retail's 176 — mc2l15 t=24152
            // slot 719 and t=24184 slot 314, on a human that died
            // earlier in the take.
            mc2_respawn_timer: carpet.scratch10.max(0) as u32,
            ..Player::default()
        };
        // The knock/buffet channel (`moveBoost` @+30 + direction @+32:
        // the MC1 channel's retail home on this column — same cap 128,
        // decay −4, snap <4): the MC1 arm seeds its twin from the
        // Type_160 tail; without this a free-running replay anchored
        // mid-buffet starts with a silently empty channel.
        self.g.player_knock = (ply.knock_dir, ply.knock_mag);

        // The human's str_611 spellbook: manifestation slots, XP,
        // and tier state live in the per-player block and mutate at
        // runtime (casts, kills, releveling) — the world-build
        // seeding is cross-pair state, so rebuild from the closure.
        // Without this the cast machinery ticks whatever slots the
        // level build assigned, not the imported manifestations.
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
        // The +3000 re-cast surcharge latch (`byte_0x1BE_446`). It is
        // carried across ticks by a single input event and read by
        // every castle-spell re-price, so a pair landing INSIDE a
        // latched window must import it — unseeded, every such pair
        // prices the token 3000 light. (mc2l3 584-597, 16012-16200,
        // 16247-17127, 17162-18075; spells-galore 32476-33465.)
        self.mc2_recast_surcharge = ply.recast_surcharge != 0;

        // TERRAIN REPLAY. `.mgcr` has no terrain channel, so the pool
        // lands on PRISTINE heights while retail's map still carries
        // every already-run (14,1) riser's write. That write is a
        // pure function of the riser's own imported state, so replay
        // it (mc2::riser::mc2_riser_reconstruct) — a removed riser's
        // 3-row endcaps stand at +48 forever and are what fences the
        // walkers/dwellers retail keeps out of the walled compounds.
        // The BUILD00 pad stampers (mc2::pads) are the same shape and
        // dominate the residual: a (3,2) castle's cumulative (10,42)
        // painter pad and a village building's own action-51 terrace
        // both end at an ABSOLUTE `pad + datum`, and the recording
        // carries every input (cell, BUILD00 row, `site_z` datum). The
        // world build already settles the AUTHORED stamps, so both
        // replays are no-ops there and only recover what the take
        // itself built or levelled up. Castles first: a castle build
        // purges the buildings inside its footprint, so a surviving
        // building never overlaps a castle pad.
        //
        // `MGC_NO_PAD_REPLAY` is the terrain-replay A/B toggle:
        // `1`/`all` disables both arms, `castle`/`building` one of
        // them. The runner's own measurements are taken with it unset.
        //
        // A MEASURED terrain channel (format-2) disables ALL of the
        // reconstruction heuristics outright: the planes are ground
        // truth, and a reconstruct that "solves to a no-op" on a
        // finished map does NOT no-op on a mid-rise one — the castle
        // pad stamper wrote the full 48-byte pad over the measured
        // partial rise, so every pair inside a build window read
        // finished ground (mc2l3 t=244: castle z 1536 vs retail 64,
        // and the painter's own delta solved to 0 — no crush sweep,
        // no rise).
        let off = std::env::var("MGC_NO_PAD_REPLAY").unwrap_or_default();
        let off = |arm: &str| self.measured_terrain || off == "1" || off == "all" || off == arm;
        if !off("castle") {
            for i in 0..self.g.ent.len() {
                if self.g.ent[i].class64 == 3 && self.g.ent[i].model65 == 2 {
                    self.g.mc2_castle_pad_reconstruct(i);
                }
            }
        }
        if !off("building") {
            for i in 0..self.g.ent.len() {
                if self.g.ent[i].class64 == 10 && matches!(self.g.ent[i].tick70, 51 | 52) {
                    self.g.mc2_building_pad_reconstruct(i);
                }
            }
        }
        if !self.measured_terrain {
            for i in 0..self.g.ent.len() {
                if self.g.ent[i].class64 == 14 && self.g.ent[i].model65 == 1 {
                    self.g.mc2_riser_reconstruct(i);
                }
            }
        }
        // The STATIC GROUND PROBES run LAST (mc2::probes): the three
        // class-2 snap laws pin `z` to the interpolated ground every
        // tick on an entity that never moves, so each prop's imported
        // `z` is a terrain SAMPLE the recorder captured without
        // knowing it — the only handle the format gives on ground the
        // take dug with edits whose casters are long gone (fire
        // scorch, craters). Inverting the sampler over the ≤4 cells it
        // reads is the last pass, so a prop standing on a replayed
        // pad/riser sees the finished map and solves to a no-op.
        // `MGC_NO_STATIC_TERRAIN_REPLAY=1` is its A/B toggle.
        if !self.measured_terrain
            && std::env::var("MGC_NO_STATIC_TERRAIN_REPLAY").unwrap_or_default() != "1"
        {
            let cost = self.g.mc2_ground_reader_cost();
            let mut claimed = std::collections::BTreeSet::new();
            for i in 0..self.g.ent.len() {
                if self.g.mc2_is_ground_probe(i) {
                    self.g.mc2_static_ground_reconstruct(i, &mut claimed, &cost);
                }
            }
        }

        // Cross-pair latches, same wipe as the MC1 arm.
        self.human_pose = (carpet.x, carpet.y, carpet.z);
        self.pending_teleport = None;
        self.pending_teleport_slot = None;
        self.pending_respawn = None;
        self.pending_restart = false;
        self.duel = None;
        self.won = false;
        // ⭐⭐⭐ ROUND 98 (dig 98-Q11) — THE LEVEL-END LATCH IS
        // IMPORTED STATE, AND THIS WIPE ATE IT. The objective-board
        // import above already seeds `completed` from
        // `struct_0x3659C[local].IsLevelEnd_0`; this line (inherited
        // from the MC1 arm, which has no such board) cleared it again,
        // so EVERY MC2 pair ticked with `completed == false` and the
        // class-11 model-4 LEVEL-END RELEASE (`AddSwitch0B_04_6F150`,
        // EF:54329-46 — `sub_4A1E0(id_0x1A_26, 1)` the first frame a
        // class-3 model-0 wizard's board carries the latch) never
        // fired. See `mc2::scenery::no_import_level_end_latch`.
        self.completed = !crate::mc2::scenery::no_import_level_end_latch()
            && st.objectives.get(local).is_some_and(|b| b[0] != 0);
        self.win_streak = 0;
        self.prev_fire = (false, false);
        self.accel_veto = (false, false);
        self.rival_deaths.clear();
        self.notification = None;
        self.kill_tally = [[0; 8]; 8];
        self.entities_dirty = true;

        // ⭐ THE CARPET'S PRIVATE LCG GETS A HOME. The human is out
        // of pool here, so its `rand_0x14_20` had nowhere to live and
        // the death scatter fell back to each token's own seed (the
        // registered `mc2_scatter_spells` deviation). The RESERVED
        // carpet slot is the natural home — nothing else reads or
        // writes it, exactly as `mc1_carpet_slot` works one game over.
        // ⚠ AFTER the pool loop: that loop clears the reserved slot.
        self.g.ent[human_slot as usize].rand = carpet.rand as u32;

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
                // ⚠⚠ THE (10,79) DEFENDER PIECE RE-HOMES @0x1C/@0x1E.
                // `import_ent_mc2` puts retail's `yaw_0x1C`/`pitch_0x1E`
                // (the LATCHED FIRING DIRECTION) into `f34`/`f36` for
                // this family — `f30`/`f32` are the fire MODE and the
                // windup scratch there — and `port_ent_lanes_mc2`
                // already publishes it that way. This projection did
                // not, so a turret that had fired graded its firing
                // pitch against `f32` = 0 forever: an INSTRUMENT floor
                // at every shot, not a sim divergence. mc2l3 t=18620 is
                // the row — retail `pitch` 270, obs port 0, while the
                // raw-lane dump of the same tick reads 270 on both
                // sides.
                heading: if e.class64 == 10 && e.model65 == 79 {
                    e.f34 as i16
                } else {
                    e.f30 as i16
                },
                pitch: if e.class64 == 10 && e.model65 == 79 {
                    e.f36 as i16
                } else {
                    e.f32 as i16
                },
                applied_yaw: e.f78 as i16,
                applied_pitch: e.f80 as i16,
                speed: e.f126,
                mana: e.f140,
                mana_max: e.f136,
                // Retail's parentId @0x28 (the recorded `owner` lane) is
                // live on FOUR families on this corpus — the old
                // "class-15 only" premise is REFUTED (mc2l24 whole-file
                // owner census: 47k+ rows). Each is recovered per family:
                //   • class-15 manifestations — parentId = wizard, fused
                //     into id24 (@0x28 != 0 branch); `id24 != slot`
                //     excludes a detached manifestation (projects 0).
                //   • (5,10) DOOMSDAY PYRAMID — @0x28 is REPURPOSED as
                //     the (10,14) rock-ring spin angle (`f36` port-side,
                //     +96 & 0x7FF per un-suppressed tick), from f36.
                //   • (10,42) build painter — parentId = the owning
                //     castle entity (fixture t=10062 slot 162: @0x28=426
                //     = the (3,2) castle slot; a wizard-owned variant
                //     stamps 116). No wild (10,42) exists, so the fused
                //     id24 = tr(@0x28) recovers it directly.
                //   • (5,{0,19,21,25}) pyramid-summoned creatures — the
                //     apocalypse summon (EF:13420) stamps parentId = the
                //     pyramid (entity 7 = the (5,10) here) into both @0x28
                //     and @0x1A, so id24 = tr(7). CAUTION: model 0 is ALSO
                //     the generic worm / multipart body, whose id24 points
                //     at its BODY slot, not a parent (261k wild rows if
                //     read blindly). The discriminator that survives both
                //     import AND the native summon (`own_id = pyramid.id24`
                //     = 7, doomsday.rs, once the importer stops fusing the
                //     pyramid's spin-angle @0x28 into its id24) is: the
                //     referenced entity IS a live (5,10) pyramid. A wild
                //     body points at a (5,0)/(5,27) segment → projects 0.
                owner: if e.class64 == 5 && e.model65 == 10 {
                    e.f36
                } else {
                    // The three translated-owner lanes (class-15
                    // manifestation → wizard, (10,42) painter →
                    // parent castle, live-pyramid summon → pyramid)
                    // all project the same way; everything else 0.
                    // (10,57) FOOL'S-MANA sphere: retail's `sub_6C870`
                    // stamps `parentId_0x28 = caster.id_0x1A`
                    // (EF:57905), the owner every m57 reader keys on
                    // (`sub_36680`'s skip gate EF:26623; the autoaim's
                    // `model == 57 → parentId` test EF:55027-31). The
                    // port carries it in id24 (import + both native
                    // spawners) but projected 0 — mc2l22 t=9785-13683:
                    // rival 557's spheres read owner28 = 557 vs port 0.
                    // ⭐ THE SUMMON-ARMY CHAIN CARRIES THE CASTER'S
                    // parentId DOWN THREE RECORDS. The enumerated
                    // `parentId_0x28_40 =` write list (17 sites in
                    // EventsFunctions.cpp) puts three of them on one
                    // spell: `sub_6C170` stamps the (9,24) carrier
                    // `v6x->parentId_0x28_40 = a1x->parentId_0x28_40`
                    // (EF:57669, the class-15 token's own parentId =
                    // the wizard that picked it up, EF:55723);
                    // `sub_67800` — the (9,24)'s OWN action-25 body —
                    // copies it onto every (10,72) ring node it mints
                    // (`resultx->parentId_0x28_40 =
                    // a1x->parentId_0x28_40`, EF:59170, walked down
                    // the whole `word_0x34_52` chain); and each node's
                    // `sub_3A5B0` copies it again onto the class-5
                    // creature it hatches (EF:29611) beside the
                    // `StageVar2_0x49_73 = 13` summoned-army marker
                    // (EF:29609). The (9,25) ALLIANCE carrier takes
                    // the same stamp from its own `sub_6CD20`
                    // (EF:58072). The port carries the caster in id24
                    // at every one of those seats (`mc2_launch`'s
                    // `e.id24 = PLAYER_TARGET`, `mc2_spawn_summon_ring`'s
                    // `e.id24 = own`, and the importer's @0x28 fusion)
                    // but projected 0, so mc2l6-rival-spells-galore
                    // reported `owner: retail 343 port 0` on 1,022 of
                    // its 1,024 owner rows — slot 6 (the (9,24)), the
                    // (10,72) nodes at 3/4/32/58 and the (5,19)
                    // fireflies at 491-495, EVERY tick from t=15631 on.
                    // ⚠ NOT a blanket class-9/class-10 rule: the
                    // `sub_693F0` fire block (EF:55861-78), the
                    // `sub_6DCA0` band (EF:44224-34), the mine
                    // (`sub_6CAC0` EF:57984-58002) and both possession
                    // arms (EF:55950-82 / EF:56045-69) stamp NO
                    // parentId at all, and the take confirms it —
                    // every one of its 1,022 rows is the army chain.
                    let translated = (e.class64 == 15 && e.id24 != slot)
                        || (e.class64 == 10 && e.model65 == 42 && e.id24 != slot)
                        || (e.class64 == 10 && e.model65 == 57 && e.id24 != slot)
                        || (e.class64 == 9
                            && matches!(e.model65, 24 | 25)
                            && e.id24 != slot)
                        // The (10,72) ring node (EF:59170). The
                        // (10,74) alliance executor is deliberately
                        // NOT here: its carrier's action body is
                        // `sub_67890`, a bare `sub_65820` + wizard-
                        // lock clear with no parentId copy
                        // (EF:59181-97).
                        || (e.class64 == 10 && e.model65 == 72 && e.id24 != slot)
                        // The metamorph pose-puppet (sv2 == 12, any
                        // class-5 model): retail stamps parentId =
                        // the caster's entity (EF:56328-29) — the
                        // live-pyramid gate can never admit it since
                        // its parent is a class-3 carpet (mc2l0-sg
                        // slot 28 owner 152; mc2l22 slot 673 owner
                        // 557, a RIVAL carpet).
                        || (e.class64 == 5 && e.site_z == 12)
                        // The SUMMON-ARMY creature (sv2 == 13, any
                        // class-5 model): `sub_3A5B0` stamps parentId
                        // = the ring node's parentId = the caster
                        // (EF:29611) one statement after the marker
                        // itself (EF:29609). Like sv2 == 12 its parent
                        // is a class-3 carpet, so the live-pyramid
                        // gate below can never admit it.
                        || (e.class64 == 5 && e.site_z == 13 && e.id24 != slot)
                        || (e.class64 == 5
                            && matches!(e.model65, 0 | 19 | 21 | 25)
                            && self
                                .g
                                .ent
                                .get(untr(e.id24) as usize)
                                .is_some_and(|p| p.class64 == 5 && p.model65 == 10));
                    // ⭐⭐⭐ THE ALLIANCE CHARM IS THE FOURTH KIND RETAIL
                    // STAMPS, AND IT IS THE ONE THAT CANNOT RIDE id24.
                    // `sub_3A650` writes `v6x->parentId_0x28_40 =
                    // a1x->id_0x1A_26` (EF:29680) on a PRE-EXISTING
                    // creature and leaves its `@0x1A` untouched —
                    // mc2l0-spells-galore's archer 559 reads `f1a` 559
                    // and `owner28` 152 side by side for the whole
                    // charm — so the fusion every arm above uses would
                    // publish the victim's own authored id. The parent
                    // lives in the `mc2_allied` side map instead (now
                    // seeded by the pair importer off this very lane),
                    // and this is where it surfaces.
                    if e.class64 == 5
                        && e.site_z == 14
                        && !crate::mc2::mobs::no_mc2_alliance_parent_seat()
                    {
                        untr(self.g.mc2_allied.0.get(&slot).copied().unwrap_or(0))
                    } else if translated {
                        untr(e.id24)
                    } else {
                        0
                    }
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
            // ⭐ CLASS 10 IS THE EFFECT CLASS AND ITS @0x90 MANA LANE
            // IS DEAD — the ONLY members that carry mana there are the
            // (10,39)/(10,57) mana spheres and the (10,45) buildings.
            // Census over all seven MC2 takes (levels 0/1/3/4/24/30,
            // 8.7M class-10 obs rows, 49 distinct models): every model
            // outside those three is 0 in EVERY row. The decompile
            // agrees — the only writers of `mana_0x90_144` onto a
            // class-10 record are the mana-sphere ctors (EF:24057 the
            // wandering sphere `rand % 0xA00 + 1`, EF:25997 the
            // split-into-N `IfSubtypeCallCreatingManaSphere_4A190(...,
            // 5, 9)` share).
            //
            // What the port keeps in f140 for the rest of the family
            // is retail's `subSpellIndex_0x2A_42` — the effect's
            // AMOUNT. `mc2_impact_spawn`'s tail stamps it on every
            // effect it spawns (mirroring retail's own
            // `v11x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42`,
            // EF:62993) and the ticks read it back there:
            // `mc2_meteor_tick`'s `f140 / max_life` per-tick burn
            // (mc2l3 t=1340, the (10,17) at f2a 16000 over maxLife 10),
            // `mc2_debuff_stamp_tick`'s mailed stun amount (mc2l3
            // t=1312, the (10,66) at 780), the (10,11) SCORCH RING's
            // burn, the (10,0)/(10,6) fires. Publishing that amount in
            // the `mana` lane graded it against retail's dead 0.
            if e.class64 == 10 && !matches!(e.model65, 39 | 45 | 57) {
                row.mana = 0;
            }
            // The m27 HYDRA keeps its bolt power (@0x88) in f136
            // (import_ent_mc2's `m27` arm); retail's @0x8C mana_max
            // lane is dead 0 across the whole family (mc2l24 census,
            // 87,210 rows), so the obs lane re-zeroes rather than
            // reporting the power.
            if e.class64 == 5 && e.model65 == 27 {
                row.mana_max = 0;
            }
            // The (10,79) castle defender piece keeps its world-yaw
            // (@0x1C, the obs heading lane) in f34 — the piece brain's
            // firing-yaw home (import_ent_mc2's (10,79) block,
            // mc2_castle_piece_tick) — not the uniform f30, which now
            // holds the @0x2C fire-mode selector. (Pitch stays on the
            // uniform f32=@0x1E copy: the piece's live @0x1E lives in
            // f36 but projecting it there only trades the static-copy
            // capture residual for the firing-elevation one, both
            // terrain-closure, so leave f32.)
            if e.class64 == 10 && e.model65 == 79 {
                row.heading = e.f34 as i16;
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

/// `MGC_NO_MC2_IMPORT_TOKEN_GATE=1` — the A/B lane for BOTH import
/// gates below (the `sub_68D50` afford refusal and Shield III's
/// pre-decrement): restore the pre-dig import, which pinned the
/// wizard's applied regen on every live token tick without ever
/// asking whether retail's own handler reached `sub_68DE0` at all.
/// The Shield III half ALSO answers the live pass's own
/// `MGC_NO_MC2_SHIELD3_PREDECREMENT`, so that switch keeps turning
/// the whole law off on both call paths.
pub(crate) fn mc2_import_token_gate_off() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_IMPORT_TOKEN_GATE").is_some())
}

/// ⭐⭐⭐ **`sub_68D50` — THE AFFORD GATE THE IMPORT PATH NEVER RAN.**
/// The live token pass has had it since the `afford` flag in
/// [`World::mc2_manifestation_tick`] (`World::mc2_afford`); the
/// CONFORMANCE IMPORT's twin below did not, and a law on one call
/// path is not landed.
///
/// Every one of the 26 class-15 handlers wraps its `sub_68DE0` call
/// in `if (sub_68D50(token, wizard))` and answers a refusal with the
/// collapse arm (`word_0x2E_46 = 1` — 24 of them; `= 0` for the
/// castle and the shield), so a REFUSED tick is the tick the window
/// dies AND the tick nobody touches the wizard's regen register: the
/// fresh recompute stands and the wizard applies it in full.
///
/// `sub_68D50` (EF:55883-55901, `NETHERW.EXE` file 0x8D550 = VA
/// 0x68D50), statement for statement:
///
/// ```text
///   wizard.mana_0x90_144 < 0                    -> false   (0x8d55b `83 ba 90 00 00 00 00` / `0f 8c`)
///   wizard.life_0x8      < 0                    -> false   (0x8d568 `83 7a 08 00`)
///   token.manaRegen_0x88_136 != 0               -> the castle-upkeep leg
///       no castle, or upkeep > castle.mana      -> false   (0x8d572/0x8d586 `74 4b`/0x8d59a `7f 34`)
///   wizard.mana >= token.maxMana_0x8C_140  &&  @0x2E == @0x30  -> TRUE
///                                                          (0x8d5a8 `3b 90 8c 00 00 00` / `7c 13`)
///   @0x2E == @0x30                              -> false   (0x8d5c3 `66 8b 50 2e` / `74 09`)
///   otherwise                                   -> true
/// ```
///
/// i.e. the purse is only re-checked on the ARM tick, and a mid-burst
/// tick can still be refused by the castle-upkeep leg.
///
/// mc2l24 witnesses (pair lane, both formerly excused by `mc2l24-player-mana-regen` — rule RETIRED 2026-09-10
/// before this landed): t=7284, spell 0 token slot 6 armed at
/// `@0x2E == @0x30 == 5` with purse 64 against a 100 cost — retail
/// refuses, keeps `manaRegen` at its recomputed +100 and the carpet
/// applies it (64 + 100 − 114 wraith leech = 50) where the port
/// stamped the first-tick zero and landed 0; and t=9753, the same
/// token armed at 11/11 with purse 121 against 250 — retail 121 →
/// 1121, the port 121 flat.
fn mc2_token_afford_retail(
    st: &RetailMc2,
    ply: &mgc_formats::mgcr::RetailPlayerMc2,
    wiz: &RetailEntMc2,
    tok: &RetailEntMc2,
) -> bool {
    if wiz.mana < 0 || wiz.life < 0 {
        return false;
    }
    // ⚠ retail tests the upkeep word for NON-ZERO (`test %esi,%esi`),
    // not `> 0` the way the live `World::mc2_afford` does.
    if tok.d88 != 0 {
        let c = ply.castle_ent as usize;
        if c == 0 || c >= st.ents.len() || tok.d88 > st.ents[c].mana {
            return false;
        }
    }
    if tok.f2e as i32 == tok.f30 as i32 {
        return wiz.mana >= tok.mana_max;
    }
    true
}

/// **THE RECORDED `manaRegen` IS NOT ALWAYS THE ONE THAT GETS
/// APPLIED — THE MANIFESTATION'S POOL SLOT DECIDES.**
///
/// Both engines run the wizard's mana as an applied-then-recomputed
/// pipeline: `AddPlayer03_00_5E010` does `mana += manaRegen` and then
/// recomputes `manaRegen` to the regen floor (EF:59996-60033). The
/// CAST machinery writes the same word from a different place —
/// `sub_68DE0` (EF:55569), run from the manifestation's OWN class-15
/// action:
///
/// ```text
///   word_0x2E_46 == word_0x30_48  (the burst's FIRST tick)
///       caster.manaRegen  = -maxMana_0x8C (or -= it, accumulating)
///   else if word_0x2E_46 != 0     (mid-burst)
///       if caster.manaRegen > 0 { caster.manaRegen = 0 }   // the PIN
/// ```
///
/// Both run inside the SAME ascending entity walk, so the
/// manifestation's slot against the carpet's decides which of the two
/// writes the recorder's frame-tail snapshot catches:
///
/// - **token ABOVE the carpet** — the wizard applies, recomputes, then
///   the token overwrites: the record holds the token's stamp and
///   applying it next frame is exactly right. (mc2l24 slot 118 vs
///   carpet 116: `d88` −100 with mana flat, then mana −100 with `d88`
///   0, then flat again.)
/// - **token BELOW the carpet** — the token stamps FIRST and the
///   wizard applies it and then recomputes: the record holds the
///   RECOMPUTED floor (100/1000), and what the next frame applies is
///   whatever the token stamps then. Seeding the recorded value made
///   the port hand the wizard a full regen quantum on every casting
///   tick — the `player.mana` family, 3,710 pairs of `want + 100` on
///   mc2l3 take-2 — and, on a first tick, miss the debit outright
///   (t=8445: the Create Castle cast, want 1359 got 41359, the whole
///   40,000).
///
/// So: start from the recorded word (which IS what `manaRegen` holds
/// when the next frame opens) and replay `sub_68DE0` for every human
/// manifestation the walk reaches BEFORE the carpet, in slot order.
///
/// The CASTLE (spell 2) is the one exception, and retail's own
/// dispatch is why: its timer is an upgrade LOCK, not a countdown, so
/// its body only reaches `sub_68DE0` on the fresh-cast sentinel and
/// never pins the regen while the tower transforms (mc2l3 t=8446+:
/// `word_0x2E_46` parked at 100 while mana climbs +1000/tick).
fn mc2_applied_mana_delta(
    st: &RetailMc2,
    ply: &mgc_formats::mgcr::RetailPlayerMc2,
    human_slot: u16,
    carpet: &RetailEntMc2,
    spell6_life: [i8; 3],
) -> i32 {
    let recorded = carpet.d88;
    // `MGC_NO_MC2_BURST_DELTA=1` — the A/B lane (both halves; see
    // `world::mc2_burst_delta_off`): seed the recorded word verbatim,
    // i.e. the pre-dig import.
    if super::mc2_burst_delta_off() {
        return recorded;
    }
    // The apply lives in the action-0 body alone. Actions 2/3 (the
    // death fall and the corpse) are the port's `LifeState`, which
    // gates the step directly — and their HELD delta is the reset's
    // 750/2000 residue law, so it must survive the import untouched.
    // Every OTHER body simply never reaches the regen block: action
    // 12, the level-end sequence (`sub_5E8C0_endGameSeq` EF:60336),
    // freezes mana for its whole run — 176 of mc2l3 take-2's 177
    // action-12 ticks apply 0 against a recorded 100, including the
    // take's last rng-mismatched pair (t=22621).
    if !matches!(carpet.action45, 0 | 2 | 3) {
        return 0;
    }
    let mut delta = recorded;
    // Ascending slot order — the walk's, and the accumulate branch
    // above makes two first-ticks in one frame order-dependent.
    let mut below: Vec<(u16, usize)> = (0..26usize)
        .map(|s| (ply.spell_ent[s], s))
        .filter(|&(m, _)| m != 0 && m < human_slot && (m as usize) < st.ents.len())
        .collect();
    below.sort_unstable();
    for (m, spell) in below {
        let e = &st.ents[m as usize];
        // The book entry must still BE that manifestation in its
        // owned action state (3M): the death scatter parks a boolean
        // 1 marker in the book (`sub_5E310` EF:60146) and a
        // wraith-stolen jar runs action 78 — neither reaches
        // `sub_68DE0`.
        if e.class3f != 15 || e.model40 as usize != spell || e.action45 as usize != spell * 3 {
            continue;
        }
        if e.f2e == 0 {
            continue;
        }
        // HEAL (5) NEVER REACHES `sub_68DE0` AT ALL: `sub_6A300`
        // (EF:56432-77) stamps its own accumulating `-maxMana` INLINE,
        // and only on a tick that actually heals — so NEITHER arm
        // below describes it, and the port's `mc2_heal_token_tick`
        // already lands that debit itself. mc2l22 t=53203 is the
        // witness: on heal's FIRST tick (`f2e == f30 == 41`, token
        // slot 11 under carpet 424) and all 41 ticks after, retail
        // pays the full +1042 regen where the port applied 0.
        if spell == 5 {
            continue;
        }
        // ⭐⭐⭐ THE AFFORD GATE, ON THE IMPORT PATH TOO — see
        // [`mc2_token_afford_retail`]. A refused tick never reaches
        // `sub_68DE0`, so the wizard's freshly recomputed regen
        // stands and he applies all of it.
        if !mc2_import_token_gate_off() && !mc2_token_afford_retail(st, ply, carpet, e) {
            continue;
        }
        // ⭐⭐⭐ SHIELD III DECREMENTS BEFORE IT CALLS, AND THE LIVE
        // PASS ALREADY KNEW. `sub_6A480`'s `life_0x1A == 1` arm
        // (EF:56855-65, `NETHERW.EXE` 0x8ED29 `66 8b 43 2e` / `48` /
        // `66 89 43 2e` = the `dec`, THEN 0x8ED34 `e8 a7 e8 ff ff` =
        // `call 0x8D5E0` = `sub_68DE0`) hands the callee a counter one
        // lower than the record holds, so the full-cost debit is
        // unreachable and the mid-burst pin is keyed on `@0x2E - 1`:
        // the tick the window ENDS on (`@0x2E == 1`) pins NOTHING.
        // `World::mc2_manifestation_tick` has carried this as
        // `shield3_predecrement` since round 121; the import twin did
        // not. Its tier-0 arm (0x8ECF8 `call`, THEN 0x8ECFD `dec`) is
        // the ordinary shape and stays on the plain counter. mc2l24
        // witnesses t=36824 / t=40238 / t=40567, token slot 40 at
        // `@0x2E == 1` against `@0x30 == 301`: retail pays +3355 /
        // +345 / +345, the port held the purse flat.
        let v2 = if spell == 6
            && !mc2_import_token_gate_off()
            && !crate::mc2::cast::no_mc2_shield3_predecrement()
            && spell6_life[(e.b46.max(0) as usize).min(2)] == 1
        {
            e.f2e as i32 - 1
        } else {
            e.f2e as i32
        };
        if v2 == e.f30 as i32 {
            // FIRST tick: retail's `manaRegen = -maxMana_0x8C` wipes
            // the recompute outright. The DEBIT itself is not seeded
            // here — the port's own manifestation pass stamps it and
            // lands it in the same tick
            // (`World::mc2_same_frame_debit`), which keeps retail's
            // ordering intact: the afford gate reads the purse BEFORE
            // the debit, exactly as it does at the token's own slot.
            delta = 0;
        } else if v2 != 0 && !crate::mc2::cast::NO_MID_BURST_REGEN_PIN.contains(&spell) && delta > 0
        {
            // The mid-burst PIN. Spell 2 is exempt: the castle's
            // timer is an upgrade LOCK, so its body never reaches
            // `sub_68DE0` again and the regen runs on (mc2l3 t=8446+:
            // the timer parks at 100 while mana climbs +1000/tick).
            delta = 0;
        }
    }
    delta
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
///
/// ⭐⭐ THE CLASS-10 `@0x2A` AMOUNT HOME IS AN ENUMERATED LIST AND
/// THREE ROWS WERE MISSING. The class-10 ctor set was audited whole
/// (every `class_0x3F_63 = 0xA` constructor in the decompile against
/// every class-10 tick that reads the port's `f140` amount home): the
/// models whose tick spends `subSpellIndex_0x2A_42` as an area amount
/// are `0/6/9/11/17/18/19/23/38/51/65/66/71/76/77`, and the import's
/// diversion list carried all of them but **23, 38 and 51**.
///   • (10,23) one-shot blast — ctor `sub_4F5F0` (EF:36096) stamps
///     `@0x2A = 25`, tick `sub_33D80` (EF:24800) deals it. Shipped EXE
///     0x73E15 `66 c7 40 2a 19 00` = `mov WORD [eax+0x2a],25` (a WORD
///     write, so the `uint16_t` in the listing is right), and 0x585AF
///     `31 c0 / 66 8b 43 2a` = a ZERO-EXTENDED 16-bit read pushed as
///     `sub_10C80`'s amount.
///   • (10,51) ridge beam — tick `sub_352C0` (EF:25753) posts
///     `sub_10C80(a1x, 0, @0x2A)` every tick; EXE 0x59B2D.
///   • (10,38) lightning storm — tick `sub_35640` (EF:25931/25934) hands
///     `@0x2A` to each (9,9) beam it rains, which stamps its own child
///     `byte_0x43 = 0xA / byte_0x44 = 0x17` = the (10,23) blast; EXE
///     0x59F61 `66 8b 43 2a` → 0x59F70 `66 89 41 2a`.
/// ⚠ (10,25) is NOT on the list and its banked brief was REFUTED: its
/// ctor `sub_4F6A0` stamps `@0x2A = 2000` (EXE 0x73ECD `66 c7 40 2a d0
/// 07`), but its tick `sub_33E20` spends `byte_0x46_70` instead (EXE
/// 0x5864F `0f be 43 46` — a SIGN-EXTENDED byte at @0x46), which the
/// port already homes in f71. The (10,25) @0x2A stamp has no reader.
/// The amount is LIVE on all three: the generic impact core ends
/// `v11x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42`
/// (EF:62993), so the blast a bolt leaves carries the BOLT's payload,
/// not the ctor's seed. Without the diversion `f140 <- @0x90` — dead 0
/// on the whole family — so an IMPORTED blast/beam/storm deals ZERO.
/// `obs_project_mc2` documents the tell and the census showed it:
/// mc2l22 t=7336 slot 713 and t=10937 slot 783 publish `mana` want 0
/// / got 50 and 200 — the port's own spawn stamping the amount into
/// f140 where the reverse map still published it.
/// `MGC_NO_C10_2A_HOME=1` restores the old (broken) homes for A/B.
pub(crate) fn c10_2a_home() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_C10_2A_HOME").is_none())
}

/// The class-10 models whose per-tick area amount lives in
/// `subSpellIndex_0x2A_42` (→ the port's `f140`). Kept in ONE place so
/// the import diversion and `obs_project_mc2`'s inverse cannot drift.
pub(crate) fn c10_amount_at_2a(model: u8) -> bool {
    matches!(model, 23 | 38 | 51) && c10_2a_home()
}

/// `MGC_NO_C10_FIELD_HOME=1` reverts the class-10 field-home audit
/// additions below (the (10,{1,15,22,25,67,75}) `@0x2A` seats and the
/// whirlwind column's `@0x2C`/`@0x36` seats) for A/B.
pub(crate) fn c10_field_home() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_C10_FIELD_HOME").is_none())
}

/// A/B toggle for the (10,79) defender piece's `target96` lane home:
/// set `MGC_NO_MC2_PIECE_TARGET96_LANE` to restore the pre-dig
/// behaviour, where `port_ent_lanes_mc2` published the UNIFORM `f146`
/// for a piece even though `import_ent_mc2` seats its `@0x96` into
/// `f28` and `mc2_castle_piece_tick` reads only `f28`. INSTRUMENT-ONLY
/// (the lane table feeds `dump-state`, `explain` and the
/// `MGC_RAW_SHADOW` census, never `obs_project_mc2`). See the read
/// site in [`World::port_ent_lanes_mc2`].
pub(crate) fn mc2_piece_target96_lane() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PIECE_TARGET96_LANE").is_none())
}

/// ⭐⭐⭐ THE PORT'S CLASS-10 EFFECT COLUMN HOMES
/// `subSpellIndex_0x2A_42` IN `f140`, AND THE IMPORT'S DIVERSION LIST
/// WAS SHORT SIX MODELS. Every ctor in `mc2/{tail,mobs,flood,effects,
/// morph}.rs` that mints a class-10 effect stamps its retail subSpell
/// constant into `f140` — the (10,1) big explosion's 400
/// (`mobs.rs::mc2_spawn_big_explosion`), the (10,15) fire trail's 100
/// and the (10,25) blast's 2000 (`tail.rs`), the (10,22) whirlwind
/// head's 1000 (`AddWind_4F040` EF:35869) which the ctor then
/// `qmemcpy`s onto all 11 (10,75) nodes (EF:35880), and the (10,67)
/// flood's 20000 (`sub_51730` EF:37421) — and
/// `mc2_proj_impact`'s tail overwrites the same lane with the
/// carrier's `@0x2A` (`sub_65820` EF:62993 /`sub_65C20` EF:63191).
/// The importer, however, routed only part of the list, so a
/// natively spawned effect held its amount in `f140` while an
/// IMPORTED one held it in `f44` and `f140` took the DEAD `@0x90`
/// (0 on every class-10 model outside 39/45/57 — the census below).
/// The port's own spawn and its own importer disagreed about where
/// the field lives.
///
/// The two members with a LIVE reader, both measured on the focus
/// takes' pair census:
/// - **(10,67) FLOOD/QUAKE.** `sub_3A090`'s castle grab mails
///   `jx->str_0x5E_94.dword_0x5E_94 += a1x->subSpellIndex_0x2A_42`
///   (EF:29347) and the port's `flood_damage_pass` (mc2/flood.rs)
///   reads exactly `self.ent[i].f140 as u32`. Retail's `@0x2A` is
///   **3000** on every recorded flood (mc2l6-rsg t=26310-37510 and
///   mc2l22 t=21925-64325, 80 sampled rows, all 3000; `@0x90` 0 on
///   all of them), so every imported flood grabbed the castles and
///   billed them **nothing**.
/// - **(10,22)/(10,75) WHIRLWIND.** `sub_33340`'s lift bills each
///   airborne victim `sub_11900(a1x, ix, 0, a1x->subSpellIndex_0x2A_42)`
///   (EF:24429) and `mc2_whirlwind_lift` reads `e.f140 as u32`.
///
/// - **(10,74) ALLIANCE EXECUTOR.** `sub_3A650` writes the charm's
///   duration onto every victim out of `a1x->subSpellIndex_0x2A_42`
///   (EF:29683-84) and [`Gen::mc2_alliance_exec_tick`] reads `e.f140`
///   — the same seat the generic impact tail writes (`e.f140 =
///   payload`, `sub_65820` EF:62993). It joined when the executor
///   stopped being an inline call and became a record: outside this
///   list its `mana` lane would have published the duration against
///   retail's dead `@0x90` 0 and its `f2a` lane a 0 against retail's
///   subSpell — the exact ≠ pair the note above calls the tell.
///
/// (10,1)/(10,15)/(10,25) join for list completeness: no port reader
/// spends their amount, so the seat is home-alignment only — but an
/// absence in an enumerated list is what hid the other three.
/// ⚠ (10,78) the MAGIC MINE is deliberately NOT here: its `@0x2A` is
/// the TIER INDEX and its port home is `f44` (`sub_3A8B0` case 0,
/// NETHERW.EXE 0x5F2B4).
pub(crate) fn c10_2a_in_f140(model: u8) -> bool {
    matches!(
        model,
        0 | 6 | 9 | 11 | 17 | 18 | 19 | 65 | 66 | 71 | 74 | 76 | 77
    ) || c10_amount_at_2a(model)
        || (matches!(model, 1 | 15 | 22 | 25 | 67 | 75) && c10_field_home())
}

/// The class-10 models whose `word_0x2C_44` the import seats in `f44`
/// (displacing the uniform `@0x2A` copy). The inverse of this list is
/// what `port_ent_lanes_mc2` publishes in its `f2c` lane.
pub(crate) fn c10_2c_in_f44(model: u8) -> bool {
    matches!(model, 0 | 6 | 9 | 16 | 18 | 19 | 67 | 71 | 76 | 77 | 89)
        || (matches!(model, 22 | 75) && c10_field_home())
}

/// ⭐ THE (10,76)/(10,77) FIRE-SPHERE ORB KEEPS ITS BREATHE STEP IN
/// `fontTypeIndex_0x3D_61` (@0x3D), NOT `word_0x2E_46`.
/// `sub_4F440` stamps `a1x->fontTypeIndex_0x3D_61 = 18` (EF:36006) at
/// layout time and `sub_339B0` is its only reader — BOTH arms:
/// the phase-1 breathe bounce `sub_33AD0` (EF:24629-45,
/// `word_0x2C_44 += fontTypeIndex_0x3D_61`, sign-flipped at each
/// bound) and the phase-2 collapse (EF:24594-606,
/// `word_0x2C_44 -= fontTypeIndex_0x3D_61`; `< 0` spawns the (10,0)
/// ground fire and tears the 26-record chain down through
/// `sub_33D40`). The port's `f46` IS that home (its ctor writes
/// `e.f46 = 18` and `mc2_fire_orb_tick` reads it), but
/// `import_ent_mc2`'s class-10 fall-through routed the DEAD @0x2E
/// into `f46`, so **every imported orb breathed with step 0**: the
/// radius froze, and the collapse's `radius - 0` never went negative,
/// so the death fire was never minted and the chain never torn down.
/// The missing allocation is invisible to the graded field set
/// (@0x2C/@0x3D/flags are not graded) and only surfaces as a ONE-SLOT
/// SHIFT of every pop that follows it in the tick — mc2l22 t=20296:
/// retail pops 509 for the (10,0), the port pops it for the orb hub,
/// and all 26 orb records plus their `rand` land one seat early.
/// `MGC_NO_ORB_BREATHE_HOME=1` restores the old (broken) home for A/B.
pub(crate) fn orb_breathe_at_3d(class: u8, model: u8) -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let on = *V.get_or_init(|| std::env::var_os("MGC_NO_ORB_BREATHE_HOME").is_none());
    on && class == 10 && matches!(model, 76 | 77)
}

/// ⭐⭐⭐ THE `(10,77)` FIRE-SPHERE SATELLITE'S PITCH SPIN LIVES IN
/// `fov_0x22_34` — AND THE IMPORTER DROPPED IT.
///
/// `sub_4F440` (EF:36049-72; NETHERW.EXE 0x73C40, the ring layout)
/// gives every satellite ONE per-orb spin rate `v6 = (rand & 0x3F) +
/// 84` and files it in ONE of two words depending on the ring
/// (`byte_0x43_67 = i / 5`): ring 0 puts it in `roll_0x20_32`
/// (0x73CF2 `mov %ax,0x20(%ebx)`) and rings 1-4 put it in
/// `fov_0x22_34` (0x73D5E `mov %ax,0x22(%ebx)`). `sub_33B20`
/// (EF:24675-79; 0x58357-0x58373) then reads BOTH, one per axis:
/// `yaw += roll` and `pitch += fov`.
///
/// The port models the pair as `f34`/`f36` and steps them in
/// `Gen::mc2_orb_tumble` (mc2/tail.rs) exactly as retail does — but
/// `import_ent_mc2` restored only `f34 <- roll`; the `f36` arm below
/// listed the m27 spline, the (5,10) pyramid and the (10,78) mine and
/// gave everything else 0. So on EVERY imported pair the 20 ring-1..4
/// satellites of every fire-sphere orb had spin 0 while their 5 ring-0
/// siblings (whose spin rides `roll`) stepped correctly: the port's
/// satellites froze one tumble step behind retail's and stayed there
/// for the orb's whole life. Measured on mc2l22 under
/// `MGC_TEAR_PHASE_LAW=1 MGC_TEAR_NO_BUMP_C10=1`: 76,300 slot-ticks
/// across 1,287 ticks and 387 slots, `pitch` off by EXACTLY the orb's
/// own spin (e.g. the t=47851 orb, slots 135/445/789/…, delta 111 on
/// every tick of its life) and x/y/z off with it. ON/OFF on one
/// binary, whole take: the widened census goes 1,836 dirty pairs /
/// 305,101 field rows -> 561 / 3,498 (fixed 301,612, introduced 3, and
/// those 3 are `(10,39)` z rows present in BOTH default-oracle runs);
/// the DEFAULT census goes 570 / 3,507 -> 561 / 3,498 (fixed 9,
/// introduced 0). HORIZON-NEUTRAL by construction — the free run
/// always built its own orbs with the spin, so only the imported pair
/// lane was ever wrong. ⭐ With the law on, the widened and default
/// censuses are the SAME 3,919 rows row for row, so
/// `MGC_TEAR_NO_BUMP_C10` is now free to arm on mc2l22 — the wall
/// `mc2_no_bump_action`'s comment warned about was this one law.
/// ⭐⭐⭐ AN UNREADABLE FIELD IS NOT A CLEARED FIELD — when the
/// recording says an objective row is BOUND but `mgcr::mc2_pool_base`
/// could not convert its guest pointer (both allocator stacks empty
/// on that frame, so there is no pointer set to recover the base
/// from), keep the port's carried bind instead of clearing it. See
/// the citation block in `import_ent_mc2`'s objective-board arm.
/// `MGC_NO_STAGE_BIND_KEEP_UNREADABLE=1` restores the clearing form.
fn stage_bind_keep_unreadable() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_STAGE_BIND_KEEP_UNREADABLE").is_none())
}

/// ⭐⭐⭐ **THE IMPORT SEAT WAS DOING RETAIL'S REMOVAL WITH THE WRONG
/// ALGORITHM.** Retail's victim stack is a rebuild (`sub_49F90`'s
/// descending 999→1 push) SCRAMBLED by `sub_57F20`'s SWAP-WITH-TOP
/// removal (Events.cpp:5228-35 / `NETHERW.EXE` file 0x8C7xx). Every
/// disabled record still sits on that stack when the snapshot is
/// taken — retail's own reap (`UpdateEntities_57730`'s top pre-walk,
/// EF:39948-56 → `sub_57F20`) takes it off on the NEXT frame, pulling
/// the stack's current TOP down into the hole it leaves.
///
/// `retail_import_mc2` used to filter those ghost cells out of the
/// imported stack. That is a removal too — but an ORDER-PRESERVING
/// one, which leaves the top elements where they were. The result is
/// the same SET with a different RANKING, on every pair that carries
/// a disabled sacrificable record, so a full-pool `NewEvent_4A050`
/// seizes a different slot from retail's.
///
/// Witness (rsg pair 1884→1885, `MGC_ALLOC_CENSUS`): retail@1884's
/// 101-cell stack loses slots 750, 807, 808, 809 and 848 (all
/// `flags & 0x400`, e.g. 750 = `(10,0)` life −2 flags 0x20486) and
/// retail@1885 holds 111, 112, 113, 114, 115 — the five successive
/// TOPS — in exactly those five holes, in ascending reap order. The
/// port's imported stack held all five at the top and every one of
/// its next seizures was off by five.
///
/// Keeping the cell is safe against double-allocation: a ghost is
/// class-bearing, so it is not on the imported FREE stack either, and
/// the port's tick-top reap frees it (pushing it onto the free stack)
/// through `Gen::free_entity`, whose `swap_remove` takes it back off
/// the victim stack in the same motion.
///
/// `MGC_NO_RECYCLE_GHOST_KEEP=1` restores the pre-dig filter.
pub(crate) fn recycle_ghost_keep() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_RECYCLE_GHOST_KEEP").is_none())
}

/// ⭐⭐⭐ DIG W2-F — THE GHOST AT THE HEAD OF THE CHAIN.
///
/// The tile-chain rebuild in [`World::retail_import_mc2`] starts a
/// chain only at a record that is itself linkable AND carries
/// `prev18 == 0`. A GHOST (`byte[1] & 4`, the disable bit retail's
/// `UpdateEntities_57730` top pre-walk reaps through `sub_57F20`,
/// EF:39948-56 — file offset 0x7BF30 in the shipped `NETHERW.EXE`)
/// is deliberately NOT linkable — but the recording still captures it
/// IN the chain, with its neighbours' `next16`/`prev18` pointing at
/// it. So a ghost sitting at the head of a tile chain made every
/// record BELOW it unreachable: no walk ever reached them (their
/// `prev18` is non-zero), and they all fell through to the ASCENDING
/// FALLBACK at the bottom of the rebuild — which head-inserts in slot
/// order and therefore hands the chain back REVERSED.
///
/// Witness (mc2l22 pair 320→321, the (5,9) herd on tile (43,110)):
/// slot 492 is `class 9 model 13 flags 0x406` — a ghost — with
/// `next16 = 194, prev18 = 0`, i.e. the chain head. The recorded
/// chain is 492 → 194 → 201 → 48. The rebuild dropped 492, could not
/// start at 194 (`prev18 = 492`), and the fallback linked 48, 194,
/// 201 ascending ⇒ **201 → 194 → 48**. Retail@321, having reaped 492
/// in its own tick-top pass, holds **194 → 201 → 48** with
/// `194.prev18 = 0` — the ghost SPLICED OUT of an otherwise unchanged
/// chain. A `link`/`unlink` trace over that pair proved the tick
/// itself issues NO chain call for any of the three: every one of
/// those census rows was minted by the IMPORT, not by the sim.
///
/// The fix is exactly what retail's reap does: walk THROUGH a
/// non-linkable record instead of stopping at it, on both the
/// backward (head test) and forward (chain walk) legs, so a ghost
/// anywhere in a chain — head, middle or tail — is spliced out and
/// the surviving members keep their recorded order.
///
/// Measured on mc2l22's whole-take all-lane census: **265,623 →
/// 43,262 ungraded rows**, the `next16`/`prev18` families falling
/// from 227,560 rows over 70 `(class, model)` families to 2,890 over
/// 20. Chain order is what every first-hit cell probe consumes
/// (`sub_108B0`/`sub_10780`) and what the z-buffer-less sprite pass
/// paints in, so this is the reliability of every pair-import probe
/// in the tree, not a cosmetic lane.
///
/// `MGC_NO_CHAIN_GHOST_SPLICE=1` restores the truncating rebuild.
pub(crate) fn chain_ghost_splice() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_CHAIN_GHOST_SPLICE").is_none())
}

/// `MGC_NO_ORB_SAT_FOV=1` restores the dropped lane for A/B.
pub(crate) fn orb_sat_fov(class: u8, model: u8) -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let on = *V.get_or_init(|| std::env::var_os("MGC_NO_ORB_SAT_FOV").is_none());
    on && class == 10 && model == 77
}

/// ⭐⭐⭐ THE SACRIFICABLE BIT — retail's `byte[2] & 2`, the ONE
/// predicate `sub_49F90` (Level.cpp:1289) ranks the recycle-victim
/// stack by, and the one `sub_57F20` (Events.cpp:5215) pulls a dying
/// victim out of it by. The port homes it POSITIONALLY at `0x2_0000`
/// (every MC2 ctor that stamps `byte[2] |= 2` writes exactly that bit
/// — mc2/scenery.rs, mc2/effects.rs, mc2/tail.rs, mc2/mobs.rs) and
/// `Gen::rebuild_recycle(0x2_0000)` reads it — but `import_ent_mc2`
/// never CARRIED it, so an imported world held **zero** sacrificable
/// records and `NewEvent_4A050`'s second arm (Events.cpp:581-605)
/// could never fire.
///
/// mc2l22 t=10030 is the witness: retail's snapshot has 999 live
/// records, 0 free, 141 disabled and **80 sacrificable**; the port
/// imported 141/**0**. Retail's tick allocates 66 records by
/// SACRIFICE — slots 211, 854, 858, 859, 860, 867, 874, 875, then 877
/// … 998, i.e. `sub_49F90`'s descending 999→1 push popped
/// lowest-first — where the port seized none and DROPPED 9 spawns.
/// The take reports dropped spawns on 1,107 of its 65,556 pairs.
///
/// ⭐⭐⭐ AND THE OTHER HALF WAS A DECODER HOLE, NOT A MISSING RETAIL
/// SITE (dig C3, session 96). The first attempt at this bit ALSO
/// armed `mc2_recycle.refill` — a synthetic ascending
/// `rebuild_recycle` — because the recorded victim stack looked
/// EMPTY on every full-pool frame. It is not: `mc2_pool_base`
/// recovers the pool base from the FREE stack's pointer cells, a
/// full pool leaves that stack empty, and the live-victim fallback
/// could not pin a unique base under a bare "is occupied" test. With
/// the fallback validated against the SACRIFICABLE predicate
/// instead, mc2l22 t=10030 decodes retail's real 67-deep stack and
/// the pair grades BIT-CLEAN (929 -> 0 unexplained rows), all 67
/// seized in retail's own order. The refill is NOT needed and must
/// stay off: retail's stack is scrambled by `sub_57F20`'s
/// swap-with-top and an ascending rebuild picks different victims.
///
/// The SACRIFICABLE bit, retail's `struct_byte_0xc_12_15.byte[2] & 2`
/// — the only flag `import_ent_mc2` used to drop. `Gen::free_entity`
/// gates its swap-remove mirror on `flags & 0x2_0000`, so the import
/// must deliver it or the victim stack cannot be maintained. Pairs
/// with the decoder's full-pool recovery (`mc2_full_pool_victims`);
/// `MGC_NO_MC2_SAC_BIT=1` reverts both.
///
/// ⚠ THE SYNTHETIC REBUILD IS NOT PART OF THIS AND IS ACTIVELY WRONG:
/// `rebuild_recycle` pops strictly ASCENDING, while retail's live
/// stack is SCRAMBLED — `sub_57F20` (Events.cpp:5209-37) removes a
/// dying victim by SWAP-WITH-TOP, so at mc2l22 t=10029 slots
/// 4/40/45/57/108/197/210 sit ABOVE 665/816/847/850/853. Arming the
/// rebuild is what cost the first attempt at this law its 78 rows.
pub(crate) fn mc2_sac_bit() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SAC_BIT").is_none())
}

/// A/B kill-switch for THE MC2 SCRATCH SEAT: set
/// `MGC_NO_MC2_SEAT_SLOT0` to restore the pre-dig behaviour, where the
/// MC2 entity import loop started at slot 1 and pool slot 0 arrived at
/// the origin.
///
/// ⭐ **SLOT 0 IS NOT DECORATION, AND THE MC1 IMPORTER HAS SAID SO FOR
/// MANY ROUNDS** (see the `for slot in 0..n` block in
/// [`World::reanchor_mc1`] and its note). MC2 has three readers of the
/// scratch record now — `sub_605E0`'s castle-downgrade stamp
/// (`mc2/castle.rs`, round 99), the castle-less hated-ball anchor
/// (`mc2/rivals.rs`, round 101) and `sub_169C0`'s bare
/// `Entities_EA3E4[a1x->word_0x96_150]` on a ZERO target word
/// (EF:8147, round 104) — and its MC2 import loop still began at 1.
/// *A law landed on one call path is not landed.*
///
/// Round 104 measured this NEUTRAL-BUT-CORRECT and deliberately left
/// it unlanded pending its own corpus sweep; round 105 ran that sweep
/// and landed it.
pub(crate) fn no_mc2_seat_slot0() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SEAT_SLOT0").is_some())
}

/// `ptr_a0` -> the ABSOLUTE `str_D7BD6` row index, by retail's own
/// load fixup `(ptr - base160)/34 + 59` (Level.cpp:1255-57; base160 =
/// the saved `&str_D7BD6[59]`). `None` = the pointer is not a row
/// pointer at all (a never-allocated slot's zero).
pub(crate) fn decode_row156(ptr_a0: u32, base160: u32) -> Option<u8> {
    let d = ptr_a0.wrapping_sub(base160) as i32;
    let steps = d / 34;
    (d % 34 == 0 && (-59..98).contains(&steps)).then(|| (steps + 59) as u8)
}

/// A/B kill-switch for THE FREED RECORD KEEPS ITS BEHAVIOUR ROW: set
/// `MGC_NO_MC2_FREED_ROW_IMPORT` to restore the pre-dig behaviour,
/// where every class-0 record was imported on the stand-in row 59.
pub(crate) fn no_mc2_freed_row_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FREED_ROW_IMPORT").is_some())
}

/// A/B kill-switch for the `byte[3] & 0x10` whirlwind GRAB-latch
/// import: set `MGC_NO_MC2_GRAB_IMPORT` to restore the pre-dig
/// behaviour, where an imported victim always arrived UNGRABBED.
pub(crate) fn no_mc2_grab_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_GRAB_IMPORT").is_some())
}

/// A/B kill-switch for the `byte[3] & 0x40` DOOMSDAY RENDER-ARM gate
/// import: set `MGC_NO_MC2_RENDER_ARM_IMPORT` to restore the pre-dig
/// behaviour, where the bit had no import seat at all and every
/// imported pyramid arrived with its wind-down escape permanently
/// disarmed. See the write site in [`import_ent_mc2`] for the bytes.
pub(crate) fn no_mc2_render_arm_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_RENDER_ARM_IMPORT").is_some())
}

/// A/B kill-switch for the `byte[0] & 1` quake TOSSED-latch import:
/// set `MGC_NO_MC2_TOSSED_IMPORT` to restore the pre-dig behaviour,
/// where the retail bit landed in the port's positional bit 0 ONLY
/// and every imported quake victim arrived UN-TOSSED. See the write
/// site in [`import_ent_mc2`] for the citations.
pub(crate) fn no_mc2_tossed_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_TOSSED_IMPORT").is_some())
}

/// A/B kill-switch for THE PYRAMID'S TURN-RATE IMPORT SEAT: set
/// `MGC_NO_MC2_PYRAMID_TURN_RATE_IMPORT` to restore the pre-dig
/// behaviour, where the (5,10) doomsday pyramid's `word_0x2C_44`
/// had no home in the pair importer and every imported boss arrived
/// with `f46 = 0` — i.e. a turn cap of zero, so `sub_222B0`'s
/// `sub_58350` walk could not move its yaw at all. See the write
/// site in [`import_ent_mc2`] for the citations.
pub(crate) fn no_mc2_pyramid_turn_rate_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_TURN_RATE_IMPORT").is_some())
}

/// KILL SWITCH (`MGC_NO_MC2_DOOM_BEAM_IMPORT=1`) for the (5,10)
/// pyramid's HURL-AWAY BEAM RAMP seat — the port's `f52` re-seeded
/// from the closure GLOBAL `D41A0_0.word_0x36546` instead of the
/// pyramid's own @0x32. See the write site in
/// [`World::retail_import_mc2`].
pub(crate) fn no_mc2_doom_beam_import() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_DOOM_BEAM_IMPORT").is_some())
}

pub(crate) fn import_ent_mc2(
    r: &RetailEntMc2,
    slot: u16,
    row156: u8,
    tr: &dyn Fn(u16) -> u16,
) -> Ent {
    let (b0, b1, b2, b3) = (
        r.flags & 0xFF,
        (r.flags >> 8) & 0xFF,
        (r.flags >> 16) & 0xFF,
        (r.flags >> 24) & 0xFF,
    );
    let mut flags = 0u32;
    // ⭐ byte[0] & 1 — THE HIDDEN BIT, AND IT GATES A PRE-PASS.
    // `sub_68C70` (EF:55515; shipped EXE 0x8D481 `testb $0x1,0xc(%ebx)`
    // -> a bare `ret`) returns BEFORE the 0x2400000 proximity test, so
    // a hidden record never re-arms `byte_0x39_57`. The port carries
    // that gate verbatim (`mc2/mobs.rs`, `mc2_awake_one`) and its
    // comment even calls bit 0 "a verbatim byte[0] mapping" — but the
    // bit never ARRIVED, so the gate was dead on every imported pair
    // and every hidden record inside 24 tiles of the human woke: an
    // imported mana sphere took `f58 = 16` and ran a whole mover tick
    // retail skipped, stepping its stale `axis_0x9A` and its saturated
    // -128 lift. mc2l22 t=42407-42429 slot 441 is the witness — retail
    // holds x 12378 / z 2298 for 23 ticks with `b39 = 0` and
    // `byte[0] = 0x0D`, while the port walked +6 / -128 on every pair.
    // Same shape as the `byte[1] & 0x20` hole below; MC1's twin
    // importer has always carried the whole word (`flags: r.flags & !4`).
    if b0 & 1 != 0 {
        flags |= 1;
        // ⭐⭐⭐ ONE RETAIL BIT, **TWO** PORT HOMES — AND THE IMPORTER
        // FILLED ONLY THE FIRST. The same `byte[0] & 1` is the QUAKE
        // TOSSED/HANDLED latch: `sub_3A200`, the (10,67) flood's
        // close-range shove callback, stamps `flags |= 0x100001` —
        // byte[0] bit0 AND byte[2] bit4 — on every victim it touches
        // (shipped `NETHERW.EXE` file 0x5EA16, linear 0x3A200:
        // `8b 53 0c  mov edx,[ebx+0xc]` / `81 ca 01 00 10 00
        // or edx,0x100001` / `89 53 0c  mov [ebx+0xc],edx`). The port
        // cannot home it positionally — bit 0 is already spoken for —
        // so it lives at [`crate::mc2::flood::F_TOSSED`] (bit 31),
        // and byte[2] bit4 at bit 28 (imported below). Only the
        // SECOND half ever arrived.
        //
        // The bit is read back by `sub_39FA0`, the shove filter, on
        // the CLASS-3 MODEL-1 arm — the rival wizard's carpet
        // (0x5E830-0x5E85C in the same binary):
        //     5e830  8a 62 40        mov  ah,[edx+0x40]   ; model
        //     5e833  38 c4 / 72 0a   cmp  ah,al(=1); jb  0x5e841
        //     5e837  76 0f           jbe  0x5e848         ; model 1
        //     5e848  f6 42 0c 21     test BYTE PTR [edx+0xc],0x21
        //     5e84c  75 38           jne  0x5e886         ; -> 0
        //     5e84e  66 8b 59 1a     mov  bx,[ecx+0x1a]   ; owner ids
        //     5e852  66 3b 5a 1a     cmp  bx,[edx+0x1a]
        //     5e856  75 30           jne  0x5e888         ; -> 1
        // — i.e. `if (byte[0] & 0x21) return 0;` BEFORE the owner
        // compare, exactly what [`crate::Gen::flood_shovable`]'s
        // `!held && id24 != flood.id24` models. `0x21` = bit 0 (this
        // latch) | bit 5 (invisible, already imported).
        //
        // WITNESS mc2l6-rival-spells-galore, rival 378 (player 2's
        // wizard carpet) standing inside its own quake crater from
        // t=26,533 on. The recording holds `flags 0x10000D` — byte[0]
        // bit0 SET, byte[2] bit4 SET — so retail's flood at slot 362
        // (a (10,67) in action 73, `f1a` 343) SKIPS it and 378 takes
        // only its own mover step, (8842,15281,768) -> (8844,15284,
        // 764). The imported port arrived un-tossed, passed the
        // filter, and the 26x26 sweep dragged it onto the flood
        // TWICE in one tick — (8953,15397,690) BEFORE its own
        // dispatch even began, then (8955,15400,686). That is the
        // whole `slot 378 x|y|z` family: 54 of the take's 156
        // whole-take segments, t=26,534..29,869.
        //
        // ⭐ SAME SHAPE, SAME SLOT, SAME TABLE as the `b3 & 0x10`
        // whirlwind-grab hole below — a one-bit alias with no import
        // seat, found on rival 378 of this very take.
        if !no_mc2_tossed_import() {
            flags |= crate::mc2::flood::F_TOSSED;
        }
    }
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
    if b1 & 0x20 != 0 {
        // ⭐ ONE RETAIL BIT, TWO READERS — AND THE IMPORTER ONLY EVER
        // CARRIED THE SECOND. `byte[1] & 0x20` is the port's bit 13
        // (0x2000) POSITIONALLY on both of its jobs:
        //   • the CLASS-9 HATE LEDGER. `sub_159E0` (EF:7319, called
        //     once per tick from `UpdateEntities_57730` EF:40112)
        //     walks the class-9 roster `dword_38531`, skips an
        //     already-stamped record (`if (byte[1] & 0x20) goto
        //     LABEL_48`, EF:7337) and stamps
        //     `v0x->struct_byte_0xc_12_15.byte[1] |= 0x20u` (EF:7347)
        //     the FIRST tick the bolt has both a class-3 owner and a
        //     non-zero `word_0x96_150`. Each bolt scores its victim's
        //     hate ledger EXACTLY ONCE for its whole flight.
        //   • the mana-sphere DECAY channel (`TransformArcherToMana`
        //     EF:26268/26286/26289 and the balloon pick's refusal at
        //     EF:61009) — the only half the importer used to carry,
        //     and only for (10,39)/(10,57) (below).
        // So every IMPORTED bolt arrived UNLEDGERED and
        // `mc2_proj_hate_sweep` (mc2/rivals.rs) re-scored it on every
        // pair and on every tick of a free run. mc2l6-rsg t=6479:
        // slot 596 is the human's castle-guard arrow (9,13), flags
        // 8198 = 0x2006 — bit 13 ALREADY SET — owner `f1a` 343 (the
        // human carpet), `target96` 378 (rival wizard 2). Retail
        // scores wizard 2 once (`hate[0]` 33991 -> 34423 = +500 less
        // the 68 decay) and then only decays: 34355 / 34287 / 34219 /
        // 34151. The port re-minted the +500 on all four pairs
        // (34855 / 34787 / 34719 / 34651) until the arrow was reaped
        // at t=6483 — a free run compounds it toward the 50,000 war
        // latch, which pins the decay (EF:5388) and never comes back.
        // MC1's twin importer never had this hole: `import_ent`
        // carries retail's flags word VERBATIM (`flags: r.flags & !4`
        // — only the link bit is dropped), so MC1's own once-per-bolt
        // ledger `sub_16540` (:19678, the same 0x2000) has always
        // round-tripped.
        flags |= 0x2000;
    }
    // ⭐⭐⭐ THE FOURTH HOLE IN THIS TABLE — THE WIZARD BUFF BITS, AND
    // BOTH READERS WERE ALREADY IN THE TREE. `byte[1] & 0x40` is the
    // shield's CHARGED stage and `byte[2] & 0x40` its ARMED stage
    // (`sub_5EFA0`'s absorb gate `if (byte[1] & 0x40 || byte[2] &
    // 0x40)`, EF:60676); `byte[1] & 0x80` is the scatter REBOUND
    // window (`sub_68740`'s `word[0] & 0x8010` gate, EF:62939). The
    // human column reads exactly these bits OFF THE SAME RECORD three
    // hundred lines up (`player.shield = carpet.flags & 0x4000`,
    // `player_rebound = carpet.flags & 0x8000`) and
    // `Gen::mc2_rebound_deflect` reads `ent[j].flags & 0x8000` on POOL
    // victims — but the pool import never carried them, so no rival
    // wizard has ever deflected or absorbed in a graded pair.
    // ⚠ `byte[2] & 0x40` is NOT positional: dword bit 22 is
    // `mc2::tail::F_GRABBED`, so the ARMED stage is homed at
    // `mc2::rivals::F_SHIELD_ARMED` (bit 20).
    if !crate::mc2::rivals::rival_buff_bits_off() {
        if b1 & 0x40 != 0 {
            flags |= crate::mc2::rivals::F_SHIELD_CHARGED;
        }
        if b1 & 0x80 != 0 {
            flags |= crate::mc2::rivals::F_REBOUND;
        }
        if b2 & 0x40 != 0 {
            flags |= crate::mc2::rivals::F_SHIELD_ARMED;
        }
    }
    if b2 & 1 != 0 {
        // byte[2] bit 0 = the NO-CH0-BROADCAST stamp, and the port
        // already homes it POSITIONALLY at 0x1_0000 — the (10,0)
        // fire's `if (!(byte[2] & 1)) sub_10C80(...)` (EF:22719) and
        // its two siblings in mc2::effects read exactly that bit.
        // The importer never filled it, so every DECORATIVE imported
        // fire — the 0x10080-stamped light-show kind, the same family
        // the mc1l5 wall-of-fire dig pinned — broadcast full damage
        // in the port. Invisible while buildings were only reachable
        // at their anchor; the footprint pass surfaced it as 41 fires
        // × 400 landing on one mc2l24 village house in a single tick.
        flags |= 0x1_0000;
        // The SAME retail bit is the (10,42) castle painter's KILL
        // ARM (`byte[2] |= 1`, sub_60480 EF:61602 — see
        // [`crate::mc2::castle::F_BUILD_KILL`]), which the port homes
        // at bit 21. Without the translation an imported mid-rise
        // painter never runs its footprint purge, so every pair
        // inside a build window kept alive what retail crushed
        // (mc2l3 t=245: firebug 147, life -1 vs 600).
        if r.class3f == 10 && r.model40 == 42 {
            flags |= crate::mc2::castle::F_BUILD_KILL;
        }
    }
    if b2 & 2 != 0 && mc2_sac_bit() {
        // byte[2] bit 1 = SACRIFICABLE (see `mc2_sac_bit`): the record
        // `NewEvent_4A050`'s full-pool arm is allowed to seize.
        flags |= 0x2_0000;
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
    // ⭐⭐⭐ THE THIRD HOLE IN THIS TABLE, AND THE ONLY BYTE IT NEVER
    // EVEN UNPACKED. `byte[3] & 0x10` (dword 0x1000_0000) is the
    // whirlwind's victim GRAB latch — `sub_33340` EF:24375 sets it
    // (`ix->struct_byte_0xc_12_15.byte[3] |= 0x10u`) once the inner
    // lift has carried the victim `768 + rand%768` above the eye,
    // EF:24309/24327 are its only two readers (the far-grab and
    // near-grab arms), and EF:24322 clears it past the fling ring.
    // The port homes it at [`crate::mc2::tail::F_GRABBED`] and its
    // own lift pass sets/clears it correctly — but the importer
    // destructured b0/b1/b2 ONLY, so every imported victim arrived
    // UNGRABBED and the pass re-ran the MID-RING arm on a wizard
    // retail was already spinning.
    //
    // mc2l6-rsg pair 10109→10110, rival 378 (the take's biggest
    // census family, 419 rows, ALL on this one slot): the recording
    // has `flags 12 -> 268435468` at t=10109 — bit 28 SET — and
    // retail's grab arm steps `yaw_0x1C_28 += 204` and drifts 128
    // along the latched `word_0x30_48` (= 934, stamped at the grab)
    // per visit, twice on this pair: yaw 934 -> 1342 (+408) and
    // (6532,27130) -> (6600,27378) = 2 x 128 @ 934. It also arms
    // `byte[1] |= 8` (F_STOP), so the wizard's OWN mover `sub_146F0`
    // is vetoed and its speed servo does not run — retail holds
    // `actSpeed` 45. The port, ungrabbed, took the mid-ring arm
    // (absolute `bearing+591` -> yaw 1138, drift 96, no F_STOP), then
    // ran the rival body on top: heading 1138, speed 45 -> 29, and a
    // position that goes the other way.
    if b3 & 0x10 != 0 && !no_mc2_grab_import() {
        flags |= crate::mc2::tail::F_GRABBED;
    }
    // ⭐⭐⭐ THE FOURTH HOLE, AND THE SECOND BIT OF THE BYTE THIS
    // TABLE NEVER UNPACKED. `byte[3] & 0x40` (dword 0x4000_0000) is the
    // DOOMSDAY PYRAMID's RENDER-ARM GATE, and the shipped NETHERW.EXE
    // says it has exactly ONE setter and exactly ONE reader in the
    // whole binary:
    //   SET  file 0x7053E (VA 0x4BD3E), inside the pyramid ctor
    //        `sub_4BD00`: `81 4b 0c 01 00 80 48`
    //        = `or dword [ebx+0xC], 0x48800001`.
    //   READ file 0x67FDF (VA 0x437DF), the DETAILED-draw pass:
    //        `a1 70 2c 02 00  mov eax,[0x22C70]`
    //        `f6 40 0f 40     test byte [eax+0xF],0x40`
    //        `74 09           jz  +9`
    //        `a1 70 2c 02 00  mov eax,[0x22C70]`
    //        `80 48 2a 40     or  byte [eax+0x2A],0x40`
    //   (a scan of the whole file for every other encoding of a
    //   read/write of bit 0x40 at +0xF, and of bit 0x4000_0000 at the
    //   +0xC dword, returns NOTHING else.)
    // That `subSpellIndex |= 0x40` is the pyramid machine's ONLY
    // re-arm: `sub_21490` only ever ANDs the bit OFF (file 0x45F01
    // `80 63 2a bf` = `and byte [ebx+0x2A],0xBF`), and the wind-down
    // phase (`subSpellIndex & 0x10`) escapes to the doom-meter ramp
    // (`& 0x20`) ONLY through it. The port carries the gate verbatim
    // (`mc2/doomsday.rs`, `mc2_doomsday_tick`'s proximity analog of the
    // draw pass, `flags & 0x4000_0000`) and the ctor stamps the whole
    // 0x48800001 — but the bit never ARRIVED on an import, so every
    // imported pyramid was permanently disarmed and could never leave
    // the dormant `0x10` phase. The (10,14) falling-rock summon ring at
    // the tail of `sub_21490` is suppressed only by the `& 0x20` ramp
    // (`f26 >= 600`), so a disarmed pyramid spawns FOUR rocks EVERY
    // TICK, forever.
    //
    // WITNESS (mc2l24, the pair/entity-set lane): anchoring the port at
    // any t in [44380, 44489] — the window that opens at the pyramid's
    // birth tick 44380 and closes when retail's own `@0x2A` reaches
    // 0x60 — diverged at t=44511 with `extra in port: slot 945/960/982/
    // 999 (class 10 model 14)` and `slot 5 owner: retail 192 port 288`
    // (retail's ring spin `@0x28` FROZEN by the suppression, the port's
    // still stepping +96). By t=44600 the port held 53 extra live
    // records. Retail's own trace over the window is unambiguous:
    // `@0x10` counts the kill-all down to 0 at t=44467, sits at 0 while
    // the wind-down waits, then at t=44491 jumps to 30 and ramps +30 a
    // tick — the `& 0x20` meter — reaching 600 at t=44510, one tick
    // before the ring stops. `@0x2A` reads 0x50 (0x10|0x40) across the
    // whole wait and 0x60 from t=44491: the 0x40 the draw pass keeps
    // re-arming is present in the recording on EVERY tick of it.
    if b3 & 0x40 != 0 && !no_mc2_render_arm_import() {
        flags |= 1 << 30;
    }
    // The port routes MC2-native projectiles by the F_MC2PROJ marker
    // its ctors set (bit 29 — mc2/proj.rs); retail has no such marker,
    // so stamp every class-9 projectile except the (9,13) arrow
    // (state-keyed, no marker). Without it an imported projectile
    // falls into the MC1 fallback arm and indexes MC1's 31-row table
    // with an MC2 row.
    //
    // ⭐ BIT 3 IS RETAIL'S AND IT IS NOT UNIFORM ON CLASS 9. The bolt
    // ctors clear it (`byte[0] &= 0xF7`, EF:34952 &c.) but the
    // lightning TRAIL NODE `sub_66750` (EF:58336-45) writes no flag
    // word at all and stays COLLIDABLE — a legal `sub_10780` victim.
    // Clearing it here erased that distinction at every import seat,
    // so a segment reset re-blinded the beam even with the ctor fixed.
    // Only bit 29 is sacrificed to the marker now.
    if r.class3f == 9 && r.model40 != 13 {
        flags = if crate::mc2::proj::no_lightning_node_collide() {
            (flags & !8) | crate::mc2::proj::F_MC2PROJ
        } else {
            flags | crate::mc2::proj::F_MC2PROJ
        };
    }
    // The m27 HYDRA reuses three struct words the uniform MC2 map
    // spends elsewhere (the branch machine's own field homes,
    // docs/traces/mc2-m27-branch-machine.md): the spline pitch angle
    // `fov_0x22_34` → f36, the speed-mode selector `word_0x2C_44` →
    // f44 (NOT the projectile column's `subSpellIndex_0x2A_42`), and
    // the branch index / body live-branch gauge `byte_0x3B_59` → f50
    // (NOT the uniform @0x30 lane). Importing the uniform homes froze
    // the whole hydra: every branch head collapsed onto one z, the
    // integrator hit its no-op arm (roll/fov/speed never advanced),
    // and all five branches read D404C[0] with the body gauge at 0.
    let m27 = r.class3f == 5 && r.model40 == 27;
    // The m23 DWELLER is the second such model: its whole machine
    // runs on `word_0x2C_44` — the cruise altitude 0x2000 (ctor
    // EF:34474, servo :18081-86) and then the SIPHON RISE STEP the
    // grabbed sphere reads off it (:18238 seeds 18, :18270 ramps +10,
    // TransformArcherToMana EF:26120 consumes it). Its
    // `subSpellIndex_0x2A_42` (500) has no reader on our side — the
    // (9,9) bolt launcher stamps its own payload (mc2/proj.rs
    // `mc2_atk_heavy9`). Importing the uniform @0x2A home made every
    // imported dweller lift its sphere by a flat 500/tick instead of
    // the ramp (mc2l24 t=14519-14523: retail +98/+108/+118/+128).
    // The (14,2) CAVE PILLAR is the third `word_0x2C_44` tenant: its
    // long-axis ORIENT lives there (THING wiring EF:33236-41,
    // sub_5B100's koefX/koefY split) and @0x2A is dead. The uniform
    // @0x2A home re-seeded orient 0 every pair, so an X-run pillar
    // grew a TRANSPOSED 2×8 footprint and its GROW arm missed
    // retail's convergence tick (mc2l3 t=15/16/18: slots 195/196/203
    // life retail 3 port 1 — the built-seal transition).
    // The (2,7)/(2,8) FALLING PROPS are tenants four and five: f44
    // is the live gravity velocity (`mc2_falling_tick`, sub_652C0
    // EF:62650-60 — position takes the old velocity, then −24/tick
    // clamped ±192). The uniform @0x2A home seeded an imported prop
    // with @0x2A (100) — an upward kick where retail held a fall
    // (mc2l6-rsg t=1 slot 135: f2c −192; mc2l22 t=1).
    // The (5,21) DEVIL is tenant six: @0x2C is the live jump impulse
    // (`Gen::m21_jump`'s f44 integrator, sub_265A0 EF:17098-151).
    // The (5,22) segmented WORM is tenant seven: on the HEAD @0x2C is
    // the spin rate `word_0x2C_44` (ctor 11, EF:34404; `sub_272C0`
    // EF:17761 adds it to the angle every tick and EF:17764-72 decays
    // it toward ±11) and on a SEGMENT it is the orbit angle
    // `sub_271D0` stamps (EF:17699-700). The head's live serpentine
    // angle `subSpellIndex_0x2A_42` (ctor 0 EF:34405, EF:17761-62) is the port's f46
    // (multipart.rs header) and is seated below. The uniform @0x2A
    // home seeded every imported head with f44 = the ANGLE and f46 =
    // @0x3D (dead 0), so each pair the head advanced angle += angle
    // from 0 — one spin step (11) BEHIND retail — and every one of
    // the 14 segments orbited 11/2048 of a turn short of its retail
    // seat (mc2l22 t=1: head @0x2A retail 110 / port 99, segment
    // 346 @0x2C retail 160 / port 149, x 568 / 555; ALL 53,813 dirty
    // pairs of the take carry this (5,22) x/y family).
    let ramp2c = m27
        || (r.class3f == 5 && matches!(r.model40, 21 | 22 | 23))
        || (r.class3f == 14 && r.model40 == 2)
        || (r.class3f == 2 && matches!(r.model40, 7 | 8));
    let mut e = Ent {
        // ⭐ DIG 98-Q20 — THE SECOND HOME. Retail's `word_0x2E_46`
        // (@0x2E) now has its own field, so the importer no longer has
        // to CHOOSE between it and `dword_0x10_16` (@0x10) per
        // (class, model, StageVar2, action): it seats BOTH. With
        // `MGC_NO_SUMMON_LEASE_FIELD` the lane is inert and the `f26`
        // arms below pick one word exactly as they did before.
        lease2e: crate::engine::features::Lease2e(r.f2e),
        // MC1-only lane (the ungated `sub_14E60` token read).
        raw48: crate::engine::features::Raw48(0),
        rand: r.rand as u32,
        // Bit-preserving: retail's lightning trail stamps a node's
        // maxLife to -1 (sub_66750 EF:58336-43) and the port's own
        // (9,9) ctor round-trips it through u32 — the old `.max(0)`
        // floor turned every imported -1 into 0 (mc2l24: 14,662 of
        // 15,421 max_life rows were exactly retail -1 / port 0).
        // MC1's twin import_ent has never clamped.
        max_life: r.max_life as u32,
        act_life: r.life,
        flags,
        next20: 0,
        prev22: 0,
        // The port fuses retail's own-id (`id_0x1A`) and
        // `parentId_0x28` into id24. @0x1A is the LIVE owner-or-self
        // lane (mc2l0 census): the caster on projectiles, the owning
        // wizard on castles/balloons/charmed creatures, the watch
        // target on class-11 triggers, self everywhere else. @0x28 is a
        // live parentId on class-15 manifestations, (10,42) painters,
        // and the pyramid-summoned (5,{0,19,21,25}) creatures — all
        // recovered by `obs_project_mc2` from this fused lane.
        // EXCEPTION: the (5,10) DOOMSDAY PYRAMID repurposes @0x28 as its
        // ring-spin angle (imported to f36), NOT a parent — fusing it
        // here stamped a garbage id24 (= the spin angle) that the
        // apocalypse summon then copied onto every child creature
        // (`own_id = pyramid.id24`, doomsday.rs), so their `owner` obs
        // read the spin angle instead of the pyramid id. Take @0x1A (the
        // pyramid's own id) for it, matching the retail summon which
        // stamps the child's parentId = the pyramid entity index.
        // ⚠ THE ALLIANCE CHARM'S `@0x28` IS THE ONE THIS FUSION MUST
        // NOT EAT. Every other family fused here is born owned, so
        // retail's `@0x1A` and `@0x28` agree and one word serves both;
        // the charm's victim is a PRE-EXISTING creature whose `@0x1A`
        // `sub_3A650` never touches (mc2l0-spells-galore archer 559
        // reads `f1a` 559 beside `owner28` 152 for the whole charm).
        // Fusing overwrote the archer's own id with the caster's, so
        // the pair-import view published `f1a` 152 against retail's
        // 559 on every charmed creature — invisible to grading,
        // because `EntObsMc2` has no `f1a` lane. The parent now rides
        // the `mc2_allied` seat beside this loop, which is where
        // `obs_project_mc2` reads it back from.
        id24: if r.owner28 != 0
            && !(r.class3f == 5 && r.model40 == 10)
            && !(r.class3f == 5 && r.sv2 == 14 && !crate::mc2::mobs::no_mc2_alliance_parent_seat())
        {
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
            // The (10,54) MANA-MAGNET / (10,69) MANA-LOCK aura keeps
            // its SQUARED RANGE in `dword_0x10_16`: `sub_38D80`
            // (EF:28371) compares the raw squared xy distance
            // (`EuclideanDistXY_584D0`, Maths.cpp:1043 — dx²+dy² over
            // i16 deltas) straight against it, and the possession
            // impact hands the aura the BOLT's own value verbatim
            // (`v14x->dword_0x10_16 = a1x->dword_0x10_16`, EF:59053 =
            // `(subSpellIndex << 8)²`). The port's field home is the
            // TILE RADIUS (`mc2_aura_tick` squares `f26 << 8`), and
            // @0x10 here is 32-BIT and ALWAYS a multiple of 65536 —
            // the ctor's 12,845,056 = (14<<8)² (EF:36825), mc2l3's
            // 40,960,000 = (25<<8)² and 198,246,400 = (55<<8)². The
            // catch-all `as i16` therefore truncated EVERY one of them
            // to exactly 0, so every imported aura ran at range 0 and
            // dragged nothing — 431 of the (10,39) family's 490 dirty
            // ticks on mc2l3, in two magnet windows (t=9815..10094 and
            // t=11278..11456). Invert the square back to the radius;
            // exact for both writers, which store `(k<<8)²`.
            // ⭐ It also closes a latent overflow: today's truncated
            // i16 can reach 32767, and `mc2_aura_tick`'s `r * r` on
            // `(32767 << 8)²` panics in a debug build.
            (10, 54 | 69) => {
                (crate::engine::features::Gen::isqrt(r.scratch10.max(0) as u32) >> 8) as i16
            }
            // The m0 worm/hydra keeps its BOB VELOCITY in @0x10
            // (multipart ctor seed + sub_1F040's home); importing
            // the charm lane left the bob dead — the whole chain
            // sank instead of undulating (mc2l4 corpus, slot 2). The
            // m27 hydra shares the @0x10 home: the body's wander/
            // emerge phase seed AND the branch machine's whip counter
            // (sub_2A340 mode-3/4 reads it — mc2l24 t=180 slot 46:
            // @0x10 steps 1→2→3→4 in lockstep with the crack speeds
            // -192/-130/-23/192; the @0x2E charm lane stays 0 and
            // parked the port one step behind). EXCEPT in the pyramid's
            // release chain: a StageVar2 16/17 summon (mc2::doomsday's
            // spawn block, EF:13419) has `word_0x2E_46` as its LIFE
            // LATCH, and mobs.rs's `mc2_doom_summon_*` expire it the
            // moment it reads <= 0 — an m0 worm summon imported with the
            // bob velocity there puffs itself on the first replayed
            // tick. The latch wins for exactly those two slots.
            // The m19 firebug shares the @0x10 home too: the attack
            // machine's HOVER ALTITUDE (case-2 `dword_0x10_16 =
            // rand&0x3FF + target z`, EF:16410-16; the case-3 bob
            // compares z against it, :16455-61) and the ctor's i%100
            // seed (:34290). Importing the dead @0x2E charm lane (0)
            // flipped every imported bob DOWN (z<=0 never holds) —
            // mc2l3 t=206 slot 151: retail 51→115 (hover 62), port
            // 51→-13, the 395-row (5,19) z family's head.
            // ⭐⭐⭐ @0x2E IS LIVE ONLY ON THE StageVar2 STATE. The
            // charm/lease kinds are all dispatched by `sub_1D5D0` from
            // action `8*model + 7`, and its 22 call sites are every
            // per-model `+7` handler.
            // ⛔ CORRECTED BY DIG 98-Q20: the tenants are FOUR, not
            // five, and `sub_1E4D0` is not one of them. An `E8 rel32`
            // byte-scan of the whole shipped image finds EXACTLY ONE
            // direct call site for each arm, all inside `sub_1D5D0`
            // (`sub_1E580` 0x41ECF, `sub_1E9C0` 0x41EF5, `sub_1E4D0`
            // 0x41EBC, `sub_1E320` 0x41EE2), so the list is closed:
            // **13 and 16** share `sub_1E580`, **14** is `sub_1E9C0`,
            // **17** merely CARRIES the pyramid's 250 until the state
            // flips to 16 (`sub_1E320` never touches it) — and
            // **StageVar2 12 owns NEITHER WORD**: `sub_1E4D0` in the
            // shipped `NETHERW.EXE` (file 0x42CD0-0x42D77) is twelve
            // instructions of position copy plus `mov %dx,0x20(%esi)`
            // / `mov %dx,0x1c(%esi)`, with no store to +0x2E and none
            // to +0x10, so a metamorph puppet keeps its ctor's
            // `slot % 100` in @0x10 for life. The moment a summon
            // ENGAGES (`sub_1E580`'s handoff sets `actionIndex = 8*model
            // + 2`) its lease is dead data and every combat handler on
            // the new state reads `dword_0x10_16` instead. Keying the
            // home on StageVar2 ALONE fed the engaged creature's combat
            // brain its 250-tick lease: mc2l6-rsg t=20580-20740, the two
            // summoned wyverns (slots 53/102, sv2 13, action 130) read it
            // as `sub_24510`'s 15-bolt burst counter (EF:15470-74) and
            // fired a (9,0) homing bolt EVERY tick — 278 `extra (9,0)`
            // rows, the take's largest non-pose family. Retail's own
            // `scratch10` on those two records is 0 while `f2e` sits
            // frozen at 249/245.
            // ⚠ The comment further down claiming "@0x2E is DEAD on
            // class 5 … `f2e` is 0 on EVERY one" (263 of 264 records
            // over four censuses) is true ONLY because those censuses
            // held no ENGAGED summons. Slot 53 here is the exact
            // inverse: `f2e` 249, `scratch10` 0.
            // ⭐ DIG 98-Q20 — WITH THE SECOND FIELD THIS ARM IS DEAD.
            // `lease2e` above carries @0x2E for every record, so class
            // 5's `f26` is retail's @0x10 unconditionally (bar the
            // (5,21) devil's @0x44 below) and the StageVar2/action
            // guess-work disappears. Round 97's
            // `no_summon_lease_split` patched the same collision AT
            // THE ENGAGE only; the pre-engage half — a summon whose
            // ctor `slot % 100` seed (`sub_4C6B0` EF:34291 and its
            // twelve siblings) was overwritten by the 250-tick lease —
            // is what the field fixes.
            (5, _) if !crate::engine::features::no_summon_lease_field() && r.model40 != 21 => {
                r.scratch10 as i16
            }
            (5, m)
                if m != 21
                    && matches!(r.sv2, 12 | 13 | 14 | 16 | 17)
                    && r.action45 % 8 == 7
                    && !crate::mc2::mobs::no_summon_lease_split() =>
            {
                r.f2e
            }
            (5, 0 | 19 | 27) if !matches!(r.sv2, 16 | 17) => r.scratch10 as i16,
            // The (5,10) DOOMSDAY PYRAMID drives its whole 16-state
            // machine off `dword_0x10_16` (@0x10 = scratch10): the
            // per-state countdown AND the 0..1200 doom-meter ramp
            // (`sub_21030`/`sub_21490`). Importing the @0x2E charm lane
            // (0) reset the doom-meter to 0 every pair, so it re-ramped
            // to only 30 and NEVER crossed the 600 gate that suppresses
            // the (10,14) rock ring — the port then spawned 4 rocks/tick
            // (each a global-LCG draw) while retail, suppressed, drew
            // none: the got[t]==want[t+4] rng window t=51751-70 (mc2l24;
            // retail `owner`/parentId spin freezes at 192 there, the
            // suppression tell) plus the epoch's isolated (1,5) pairs.
            (5, 10) => r.scratch10 as i16,
            // The (5,13) TOWNIE is the fourth @0x10 tenant, and its
            // reader is a DEATH GATE. `AddVilliger_4BF40` stamps
            // `dword_0x10_16 = 2` on EVERY townie ever minted
            // (EF:34061 — the `% 100` seed at :34056 is overwritten
            // dead), the brain only ever re-stamps it to 1 when a
            // (10,45) dwelling swallows it (EF:14594), and nothing
            // zeroes it — so `KillTownie_23680`'s
            // `if (a1x->dword_0x10_16) { DisableEntityDrawing04_57F10;
            // return; }` (EF:14672-74) is ALWAYS taken. A retail
            // townie NEVER reaches `PreKillEntity_1C890`: it is reaped
            // in place with `actionIndex` frozen at `8m+4` = 108 — no
            // death animation, no `KillEntity_1C930`, no (10,1) corpse
            // burst. Importing the dead @0x2E charm lane (0) sent every
            // replayed townie corpse down the prekill arm instead, and
            // mc2l0's free run died on exactly that: at t=4731 retail
            // holds 108 and raises 0x400 where the port stepped to 109.
            // The guard keeps @0x2E for the StageVar2 kinds that own it
            // — 12 metamorph (`sub_1E4D0` counts @0x2E down, EF:10701-30,
            // and `action & 7 == 7` routes an idle townie there), 13
            // summon-army life, 14 alliance duration, 16/17 the
            // pyramid-summon latch.
            // ⭐ AND THE SAME HOME IS THE WHOLE CLASS-5 COLUMN'S, not
            // just the four models carved out above. The port's own
            // module doc says so (`dword_0x10_16` scratch/invis → f26,
            // mc2/mobs.rs:23) and every roster ctor stamps it there.
            // The (5,12) BUILDER's site-pass counter is minted at 2
            // (`sub_4BDF0` EF:34026), POST-incremented and switched on
            // (EF:13982/13991), and its give-up arm `if (v2 >= 4) {
            // @0x10 = 1; goto LABEL_51; }` (EF:13983-86) steps action
            // 96 → 97 with NO entity-LCG draw. The (5,14) TRADER is the
            // same shape (`AddTrader_4C0B0` EF:34118 mints 2,
            // `sub_22E60` EF:14293-99 counts down and re-stamps 5), and
            // the shared villager roam body `sub_22C80` (EF:14194-200)
            // drives m12/m13/m14 alike; m2/m9/m15/… all mint
            // `(slot % 100)` there (`sub_4C1E0` EF:34146).
            // @0x2E is DEAD on class 5: over four tick censuses on four
            // takes (galore t=4382, mc2l3 t=5706, mc2l24 t=53808, mc2l0
            // t=9962 — 264 live class-5 records, models 1/3/9/12/13/15/
            // 19) `f2e` is 0 on EVERY one while `scratch10` is nonzero
            // on 263. Only the five StageVar2 charm/latch kinds own
            // @0x2E, and the guard below keeps it for them.
            // What the old catch-all cost: galore pair 4380→4381,
            // builder slot 430 — retail holds 4, gives up, steps action
            // 96→97 and leaves rand 29768; the port (imported 0)
            // evaluates a site and burns two draws. 3 of the 4 dirty
            // pairs in the 140-pair window t=4260..4399 are this record.
            //
            // ⚠ (5,21) IS NOT A TENANT OF EITHER WORD. The DEVIL's f26
            // is retail's `byte_0x44_68` — the port says so itself at
            // mc2/roster.rs:2545 (`e.f26 = 0; // byte_0x44_68 — rest
            // countdown (:34368)`) — so it takes @0x44 and is carved
            // out ABOVE the general arm. Surfaced by an adversarial
            // verifier, which measured six divergent devils in one pair
            // (mc2l24 t=20000) that the "whole column" claim would have
            // silently left broken. A/B this arm separately from the
            // general one.
            (5, 21) => r.b44 as i16,
            (5, _)
                if crate::mc2::mobs::no_summon_lease_split()
                    && matches!(r.sv2, 12 | 13 | 14 | 16 | 17) =>
            {
                r.f2e
            }
            (5, _) => r.scratch10 as i16,
            // Class-15 manifestations: the port's f26 is the ACTIVE-
            // CAST countdown, retail's `word_0x2E_46` (the cast
            // machine arms f26 = f28 and counts down — cast.rs; the
            // SetSpell short-arm gate reads the same word, L:1511).
            // Importing @0x10 (0) parked every mid-cast token INERT:
            // no effect tick, no charge, no cost refresh (mc2l3
            // t=243: the castle token froze at 1000/9 while retail's
            // active cast re-derived 10000/99 the tick the castle
            // stood).
            (15, _) => r.f2e,
            _ => r.scratch10 as i16,
        },
        f28: r.b38 as u8 as u16,
        f30: r.yaw as u16,
        f32: r.pitch as u16,
        f34: r.roll as u16,
        // The (5,10) pyramid keeps its ring-spin angle in
        // `parentId_0x28` (@0x28 = owner28; the RENDERER-arm exception
        // to "@0x28 is class-15 only"). The ring driver steps it
        // `+96 & 0x7FF` per un-suppressed tick (EF:13072), so it must
        // be RESTORED each pair — importing 0 both mis-angled the
        // (10,14) rock ring and left the `owner` obs (which captures
        // @0x28) reading retail's spin vs the port's 0 on every active
        // tick.
        f36: if m27 || orb_sat_fov(r.class3f, r.model40) {
            r.f22 as u16
        } else if r.class3f == 5 && r.model40 == 10 {
            r.owner28 as u16
        } else if r.class3f == 10 && r.model40 == 78 {
            // The magic mine's ARMED GATE. Retail parks it at −1 and
            // `sub_68AC0` writes the swallowed spell's model into
            // `word_0x36_54` to disarm it one-shot (EF:55438). The
            // port carries armed ⟺ 0 and stamps `0x8000 | spell` so
            // that swallowing spell 0 still disarms. Dropping this
            // lane re-armed, on EVERY imported pair, a mine retail had
            // already disarmed — 116 introduced rows on mc2l6-rsg.
            if r.f36 == 0xFFFF { 0 } else { 0x8000 | r.f36 }
        } else {
            0
        },
        f38: tr(r.f24 as u16),
        f40: tr(r.f26 as u16),
        f44: if ramp2c { r.f2c as u16 } else { r.f2a },
        // The (10,45) BUILDING keeps its DEGRADATION LINK in
        // `fontTypeIndex_0x3D_61` (@0x3D, the same home the class-5
        // column already imports) — seeded from `bldgprm[type].byte_3`
        // by `sub_49A30` (EF:32795-98) and ZEROED in place by the
        // castle level-up pre-clear / the quake grab, which is the
        // whole point of it being per-entity. @0x2E is dead for
        // buildings on both sides, so a replayed building used to
        // import link 0 and demolish where retail rebuilds its
        // successor ([`Gen::mc2_spawn_building`]).
        // The (3,2) CASTLE keeps its GUARD-RESPAWN COOLDOWN in
        // `word_0x2C_44` (@0x2C): `sub_5FF50` decrements it at
        // EF:61446-48, gates the spawn on it at EF:61486 and latches
        // 16 at EF:61491. Its port home is `f46`
        // (`Gen::mc2_castle_roster`), the same lane MC1's certified
        // twin uses for its own `+46` cooldown (remc1 :56414) — the
        // word moved from @0x2E to @0x2C between the games, the
        // algorithm did not. @0x2E is a REDUNDANT copy of the build
        // sub-state here (it already has its own home in `f59` below,
        // published as the `f2e` lane), so the uniform map left
        // retail's real cooldown with NO home at all and the roster
        // read `f44` <- @0x2A instead, which is 100 on every castle
        // for life (the `NewEvent_4A050` default, Events.cpp:569/595,
        // that the castle ctor never overwrites). mc2l0 t=7610 slot
        // 235 and galore t=1101 slot 266: retail f2c = 0 with the
        // guard due, port unmodelled. Same @0x2C -> f46 re-home the
        // (10,{39,57}) mana sphere already carries below.
        f46: if r.class3f == 5 && r.model40 == 22 {
            r.f2a as i16 // the worm head's serpentine angle (see ramp2c)
        } else if r.class3f == 5 && r.model40 == 10 && !no_mc2_pyramid_turn_rate_import() {
            // ⭐⭐⭐ THE (5,10) DOOMSDAY PYRAMID'S TURN RATE — @0x2C,
            // AND IT HAD NO SEAT IN THIS TABLE AT ALL. `sub_222B0`
            // (EF:13697-13770) ends its facing pass with
            //     yaw += sub_58350(yaw, roll, row->v_2, word_0x2C_44)
            // and — exactly as the m27 spline's twin call documents
            // (`mc2/multipart.rs`, EF:20967-72) — the LIVE clamp is
            // sub_58350's LAST argument, i.e. @0x2C; the row field is
            // the dead third arg. Shipped `NETHERW.EXE`, linear
            // 0x22388 (file 0x46B9E):
            //     22388  31 c0              xor  eax,eax
            //     2238a  66 8b 43 2c        mov  ax,[ebx+0x2c]   ; @0x2C
            //     2238e  50                 push eax             ; LAST arg
            //     2238f  8b 83 a0 00 00 00  mov  eax,[ebx+0xa0]
            //     22395  66 8b 40 04        mov  ax,[eax+0x4]    ; row v_2
            //     2239e  50                 push eax             ; dead arg
            //     223a1  66 8b 43 20  push  ; roll (target)
            //     223a8  66 8b 43 1c  push  ; yaw  (current)
            //     223ad  e8 88 5f 03 00     call 0x5833a         ; sub_58350
            // The pyramid's own machine stamps that word — 22 on the
            // state-0/2 entries and 113 on the 4/6 entries
            // (`mc2/doomsday.rs`, EF:12700/12760/12800/12820) — and
            // the port models it verbatim in `f46`. But the class-5
            // arm below seats `f46` from `b3d` (@0x3D, the mine's
            // burst counter), which is 0 on every pyramid record for
            // the whole level, so EVERY IMPORTED PAIR handed
            // `mc2_pyramid_face` a turn cap of ZERO and
            // `Gen::turn_step`'s `d.min(cap)` returned 0. The boss
            // simply never turned: `port_yaw[t] == retail_yaw[t-1]`,
            // tick after tick, and the only ticks it DID move were
            // the ones where the state machine re-stamped the rate
            // on a state entry.
            // WITNESS mc2l24 slot 5 (class 5 model 10, 300k life):
            // t=44,533..44,535 retail yaw 1621 / 1508 / 1402 walking
            // to its roll target 1402 at exactly 113 a tick, port
            // 1734 / 1621 / 1508 — one tick behind, every tick. That
            // is 2,486 of the take's 7,505 segments with `heading` as
            // their ONLY divergent field, plus most of the 548-strong
            // `heading pitch x y z` family on the same slot.
            // ⚠ @0x3D is dead on the pyramid (0 for the whole take),
            // so `port_ent_lanes_mc2` re-homes the pair: `f2c`
            // publishes `f46` for the pyramid and `b3d` prints `—`.
            r.f2c
        } else if r.class3f == 5
            || (r.class3f == 10 && matches!(r.model40, 45 | 78))
            || orb_breathe_at_3d(r.class3f, r.model40)
        {
            // The (10,78) mine's burst shot counter — the exact inverse
            // of `port_ent_lanes_mc2`'s `b3d` arm above.
            r.b3d as i16
        } else if r.class3f == 3 && r.model40 == 2 {
            r.f2c
        } else {
            r.f2e
        },
        // The (5,10) DOOMSDAY PYRAMID keeps its SUMMON-RING STRIDE in
        // `word_0x4A_74` (@0x4A = sv_timer): `sub_21850` stamps
        // 682 (creatures) / 256 (the m19 swarm) with the pick
        // (EF:13160/13173/13186/13199) and `sub_21AB0` fans the ring at
        // `stride * repeat + yaw` (EF:13364). @0x30 is dead for the
        // pyramid, so the uniform import parked the stride at 0 and
        // every replayed summon spawned stacked on the pyramid's own
        // bearing instead of fanning (mc2l24 t=53808: retail x 7616 vs
        // port 7936). The pyramid is never a StageVar hold (sv1 = 0),
        // so @0x4A is free for it.
        f50: if m27 {
            r.b3b as i16
        } else if r.class3f == 5 && r.model40 == 10 {
            r.sv_timer
        } else {
            r.f30 as i16
        },
        f52: tr(r.f32),
        f54: tr(r.f34),
        f56: if matches!(r.class3f, 2 | 10) {
            r.b38 as u8 as u16
        } else {
            r.f36
        },
        // `as u8` first, same law as the MC1 importer above:
        // `byte_0x39_57` is an `int8_t` (global_types.h:351) whose
        // ctor sentinel is −6 (Events.cpp:576/602), and the port's
        // canonical form for it is the unsigned byte.
        f58: r.b39 as u8 as i16,
        // The (3,2) castle's BUILD SUB-STATE lives in @0x2E
        // (word_0x2E_46 → f59, docs/traces/mc2-castle-builder.md §2);
        // @0x3A is dead for castles, and importing its 0 parked every
        // castle in the level-up state — one phantom upgrade + one
        // phantom (10,42) painter per pair, z frozen for the tick
        // (the MC2 twin of MC1's phantom-upgrade family).
        f59: if r.class3f == 3 && r.model40 == 2 {
            r.f2e as u8
        } else if r.class3f == 10
            && r.model40 == 42
            && !crate::mc2::castle::no_mc2_painter_settle_lane()
        {
            // ⭐ THE (10,42) PAINTER'S SETTLE LATCH IS `@0x3B`, NOT
            // `@0x3A`. `sub_50370` stamps `byte_0x3B_59 = 1` (EXE
            // 0x74BAF `movb $0x1,0x3b(%ebx)`, EF:36745) and the
            // painter tick's countdown end reads the same byte (EXE
            // 0x5C8C7 `cmpb $0x0,0x3b(%ebx)` → `-25` set / `-1`
            // clear, EF:27760-65). The port homes that latch in
            // `f59`, whose general seat here is `@0x3A` — so every
            // imported painter came back with the latch clear and
            // died 21 ticks after birth instead of parking out the
            // settle window. See
            // [`crate::mc2::castle::no_mc2_painter_settle_lane`].
            r.b3b as u8
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
        // The m27 HYDRA's BOLT POWER is `manaRegen_0x88_136` (@0x88 —
        // `sub_2A7F0` EF:20513-16 rolls it `(rand%12 > 7) + 1` on the
        // a3=1 shot and every a3=0 RE-FIRE reads it back, EF:20518-40),
        // and the port's `m27_branch_bolt` keeps it in f136. The
        // uniform map spends f136 on @0x8C, so every pair re-imported
        // the branch's power as 0 and the four re-fires of each whip
        // hit the `_ => return` arm: one arc per whip instead of five.
        // Retail's @0x8C is DEAD 0 on the whole (5,27) family (mc2l24
        // census, 87,210 rows: @0x8C 0×87,210; @0x88 0/1/2), so the
        // lane is free — the obs `mana_max` projection re-zeroes it.
        f136: if m27 { r.d88 } else { r.mana_max },
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
        // The DETACHED spell-jar (action 78) — the m26-wraith steal's
        // fling/homing arc `sub_59DC0` (EF:41198-41243) — abandons the
        // dormant-manifestation homes above. Its arc runs off DIFFERENT
        // fields: the arc counter `dword_0x10_16` (@0x10 = scratch10,
        // steps 0..5 rising then homing) → f26, and the wraith slot
        // `word_0x26_38` (@0x26) → f38 (`Entities[word_0x26_38]` is the
        // homing target, EF:41224). `sub_69300` (EF:55807) zeroes @0x10
        // at the steal; the parent (@0x28 = the caster/player) drives the
        // rising leg. Without these homes `mc2_stolen_arc` read the armed
        // timer as the counter (n≫5 → straight to the homing branch),
        // found no wraith in f38, and dropped the jar in place with
        // action 3M+1 on frame 1 (mc2l24 slot 73 t=15080-95: action
        // 78→1, the arc frozen a tick behind retail).
        // ⚠ …AND THE ARC COUNTER HAS ITS OWN HOME NOW (`f50`, dead for
        // class 15 two lines up): retail keeps @0x10 and @0x2E LIVE AT
        // THE SAME TIME, so seating the counter on `f26` threw the
        // armed cast timer away on every imported mid-arc jar. See
        // [`World::no_mc2_stolen_arc_keeps_cast_state`].
        if r.action45 == 78 {
            if crate::mc2::cast::no_mc2_stolen_arc_keeps_cast_state() {
                e.f26 = r.scratch10 as i16;
            } else {
                e.f50 = r.scratch10 as i16;
            }
            e.f38 = tr(r.f26 as u16);
        }
    }
    // Class-10 fires keep the area amount in `subSpellIndex_0x2A`
    // (→ the port's f140 amount home, sub_30D50's sub_10C80 call /
    // sub_31760) and the z flicker/lift in `word_0x2C_44` (→ f44);
    // the @0x90 mana lane is dead 0 on them (reverse-mapped in
    // `obs_project_mc2`).
    // The (10,9) SUMMIT DOME shares the shape (morph.rs module doc:
    // subSpell @0x2A → f140 = the per-tick area amount EF:23393,
    // dome height @0x2C → f44 EF:23258 `2r+100`) — the old uniform
    // fall-through fed f44 the 1200 area amount and the dome pushed
    // terrain to the 255 saturation cap (mc2l24 t=13 slot 91: the
    // child born at z=8160 = 255×32 where retail reads 3776).
    // The (10,76) FIRE-ORB HUB (and its 77 sibling, kept for census
    // honesty) share it whole: damage @0x2A (sub_33C00 EF:24712) and
    // ring radius @0x2C (sub_33C70 EF:24741-50). The uniform homes
    // fed f44 the subSpell and f140 the dead @0x90 — an imported orb
    // breathed 70→192 and posted ZERO mail (mc2l1 t=204-216 slot 46
    // z; t=218+ the building's 350/tick life family).
    if r.class3f == 10 && matches!(r.model40, 0 | 6 | 9 | 76 | 77) {
        e.f140 = r.f2a as i32;
        e.f44 = r.f2c as u16;
    }
    // The (10,11) SCORCH RING is the same shape: its ctor stamps
    // `subSpellIndex_0x2A_42 = 200` (`NewAdd0A0B_4E840`, EF:35563),
    // the authored/disposition par1 override rewrites @0x2A
    // (EF:33148) and `sub_31FB0` reads @0x2A as the per-tick burn
    // amount (EF:23510-12) — the port's amount home is f140. The
    // uniform map would hand it f140 <- the DEAD @0x90 (identically 0
    // on this family), i.e. an imported ring that burns nothing, and
    // it published the port's 900 against retail's 0 in the graded
    // `mana` lane (mc2l3 t=355, eight rows). @0x2C is untouched on the
    // ring, so f44 keeps the uniform (inert) copy.
    if r.class3f == 10 && r.model40 == 11 {
        e.f140 = r.f2a as i32;
    }
    // The (10,17) METEOR keeps its area amount there too: `sub_32880`
    // burns `subSpellIndex_0x2A_42 / maxLife_0x4` per tick (EF:23869)
    // and the port's `mc2_meteor_tick` reads exactly `f140 / max_life`.
    // The uniform map would hand it the DEAD @0x90, i.e. an imported
    // meteor that burns nothing (mc2l3 t=1340, f2a 16000 over maxLife
    // 10 = 1600/tick).
    // The (10,65)/(10,66) STAGGER/PARALYZE stamps are the same shape.
    // `sub_507C0` (EF:36936) seeds `subSpellIndex_0x2A_42 = 200` and
    // the impact seam overwrites it with the flyer's carried payload
    // (EF:62993) — `mc2_debuff_stamp_tick` mails that amount out of
    // f140, so the uniform f140 ← the DEAD @0x90 would import a
    // paralyze that stuns for nothing. @0x2C is untouched by both
    // ctors, so f44 keeps the uniform copy.
    if r.class3f == 10 && matches!(r.model40, 17 | 65 | 66) {
        e.f140 = r.f2a as i32;
    }
    // The (10,19) FIRE-SPRAY column (and its (10,18) vortex parent,
    // kept for census honesty) carries its per-tick area amount in
    // @0x2A too: sub_32F40's alive branch posts
    // `sub_10C80(a1x, 0, subSpellIndex_0x2A)` (EF:24151) and the
    // port's spray tick mails out of f140 (tail.rs mints 200). The
    // uniform f140 ← the DEAD @0x90 imported a column that posts
    // ZERO area mail on every pair and anchored run; @0x2C is dead
    // on the family, so f44 takes the uniform copy of it instead of
    // the payload.
    if r.class3f == 10 && matches!(r.model40, 18 | 19) {
        e.f140 = r.f2a as i32;
        e.f44 = r.f2c as u16;
    }
    // The (10,71) expanding FISSURE is the same shape (`sub_3A2D0`
    // EF:29443; EXE 0x5EAD0): the per-beat area damage is
    // `subSpellIndex_0x2A_42` — the phase-0 init rewrites it in place
    // as `4 * (@0x2A / maxLife)` (EXE 0x5EB01-0x5EB25) and the
    // every-4th-tick beat posts it through `sub_10C80` (EF:29578) —
    // while the disc's RAMP REFERENCE is `word_0x2C_44 = maxLife >> 3`
    // (EXE 0x5EAF6-0x5EB08), read as `v4` by every arm of the
    // grow/pin/shrink ladder (`movswl 0x2c(%ebx),%esi`, EXE 0x5EB26).
    // The port's homes are f140 and f44 (`mc2_fissure_tick`). The
    // uniform map fed f44 the @0x2A DAMAGE and f140 the dead @0x90, so
    // an imported fissure evaluated `maxLife - 3*damage >= life`, fell
    // into the GROW arm on EVERY tick, never reached the PIN arm that
    // draws `rand_0x14_20` (EF:29500), and posted a zero beat.
    // mc2l6-rsg t=24708 slot 401 (maxLife 48, @0x2A 16, @0x2C 6):
    // retail scratch10 18 / port 19, rand 55999 / 2528.
    if r.class3f == 10 && r.model40 == 71 {
        e.f140 = r.f2a as i32;
        e.f44 = r.f2c as u16;
    }
    // The (10,23) BLAST / (10,51) ridge BEAM / (10,38) lightning STORM
    // — the three rows the class-10 ctor audit found missing from this
    // list; see the `c10_2a_home` doc above for the EXE citations.
    // @0x2C is untouched by all three ctors, so f44 keeps the uniform
    // (inert) copy, exactly as on (10,11)/(10,17)/(10,65)/(10,66).
    if r.class3f == 10 && c10_amount_at_2a(r.model40) {
        e.f140 = r.f2a as i32;
    }
    // The (10,16) volcano boulder keeps its VERTICAL VELOCITY in
    // `word_0x2C_44` (`sub_32600` EF:23765 reads it as vz, gravity
    // −28 clamp [−384,256]) — the port's `mc2_boulder16_tick` vz lane
    // is f44. The uniform map homes f44 ← `subSpellIndex_0x2A` (=200
    // on every boulder), so an imported boulder re-launched at vz=200
    // each pair: pz = z + 200 (mc2l24 (10,16) z = retail + 200 —
    // resting summit boulders 173/329/447/574/626 and mid-roll
    // 428/449/623). The tick never reads f140, so leaving f140 ← mana
    // is inert; only f44 matters.
    if r.class3f == 10 && r.model40 == 16 {
        e.f44 = r.f2c as u16;
    }
    // The (10,67) FLOOD/QUAKE is the next `word_0x2C_44` tenant: @0x2C
    // is the DOME-TOP REFERENCE every shove victim is measured against
    // (phase 1 seeds `32*(mean-80)`, EF:28546; the phase-2 outer ring
    // raises it to the rim's `alt >> 5`), and `sub_39B60` reads it as
    // `v5 = victim.z - word_0x2C_44` for both the ceiling test
    // (`< 4096`) and the close band (`<= 96`, EF:29053-77). The
    // uniform @0x2A home handed it the subSpell instead — 3000 on
    // mc2l6-rsg slot 496 where @0x2C is 36 — so every victim in the
    // disc read `z - 3000 <= 96` and took `sub_3A200`'s DAMAGE arm
    // instead of the push, stepping the flood's private LCG once per
    // tick forever (t=26315+, `rand` want 10536 / got 42759 =
    // 9377*10536 + 9439 mod 2^16). It also suppressed the phase-2
    // outer-ring raise `if f44 < v11 { f44 = v11 }`.
    if r.class3f == 10 && r.model40 == 67 {
        e.f44 = r.f2c as u16;
    }
    // ⭐ The (10,89) CAVE-IN is the next `word_0x2C_44` tenant, and it
    // is the first one to need a SECOND word with it. `sub_311E0`
    // (EF:22860) runs the collapse entirely out of two homes the
    // uniform map spends elsewhere:
    //   @0x2C = the WAVE PHASE — phase 0 seeds 227 (EF:22936), every
    //     tail adds 22 (:23085) and trips phase 3 past 1024 (:23088),
    //     and the six-ring sculpt reads it as `v7` (:22957) to scale
    //     the `sin_DB750` rise/drop profile;
    //   @0x36 = the one-shot DEBRIS LATCH — the burst is
    //     `if (!word_0x36_54 && word_0x2C_44 > 455)` (:23052), which
    //     flings ~74 (10,13) rocks and latches @0x36 = 1.
    // The port's homes are `f44` and `f54` (`mc2_cave_in_tick`,
    // mc2/cave.rs). The uniform map fed f44 the subSpell @0x2A (100 on
    // every cave-in) and f54 the multipart TAIL LINK @0x34 (dead 0
    // here), so an imported collapse was wrong three ways at once:
    //   1. `100 > 455` is never true ⇒ THE DEBRIS BURST NEVER FIRED.
    //      mc2l15 pair 35203→35204: retail spawns the ring, the port
    //      spawns nothing, and the free stack reads retail 766 /
    //      port 840 — exactly the 74 unallocated rocks.
    //   2. the wave restarts from 100 instead of its real phase, so
    //      the sculpt profile is evaluated at the wrong point of the
    //      sine and the ring writes the WRONG FLOOR/CEILING heights —
    //      which is what the take's `z` and `pose.z` rows are.
    //   3. 100 + 22/tick takes far longer to pass 1024, so phase 3
    //      lands late and `life` (+4/tick, EF:23086) over-runs its
    //      188 cap — the recurring `life: retail 188 port 192`
    //      signature (mc2l15 slots 684/709/737/857).
    // ⚠ @0x36 must NOT go through `tr()`: it is a latch, not a slot.
    if r.class3f == 10 && r.model40 == 89 {
        e.f44 = r.f2c as u16;
        e.f54 = r.f36;
    }
    // ⭐ THE CLASS-10 `@0x2A` FIELD-HOME AUDIT (`c10_2a_in_f140`):
    // every remaining class-10 model whose PORT CTOR stamps its
    // retail subSpell constant into `f140`. The (10,67) flood's 3000
    // castle bill and the (10,22) whirlwind's lift damage are the two
    // with live readers; (10,1)/(10,15)/(10,25) are home-alignment.
    // (0/6/9/11/17/18/19/65/66/71/76/77 + 23/38/51 are seated by the
    // blocks above and are folded into the same predicate.)
    if r.class3f == 10 && matches!(r.model40, 1 | 15 | 22 | 25 | 67 | 75) && c10_field_home() {
        e.f140 = r.f2a as i32;
    }
    // ⭐⭐ THE (10,22) WHIRLWIND COLUMN HAS THREE DISPLACED HOMES, AND
    // TWO OF THEM ARE THE TAIL DRAG'S ONLY INPUTS. `AddWind_4F040`
    // (EF:35852-84) mints the head then `qmemcpy`s it onto 11 (10,75)
    // nodes, overwriting `word_0x2C_44 = i + 1` — the NODE INDEX —
    // and `sub_4F1C0` (EF:35921-33) then walks the chain stamping
    // `word_0x36_54 = v2`, the cumulative Z-STACK OFFSET
    // (62/134/214/304/402/510/626/760/922/1120/1372 on mc2l6-rsg).
    // The head tick `sub_331A0` spends BOTH every tick:
    //   `v6 = 72 - 4 * (12 - v7x->word_0x2C_44);`      (EF:24213)
    //   `predictedAxis.z = v7x->word_0x36_54 + a1x->position.z;`
    //                                                   (EF:24217)
    // — ported verbatim in `mc2_whirlwind_move` as
    // `72 - 4 * (12 - self.ent[n].f44)` and
    // `zoff = self.ent[n].f50`.
    // The uniform map fed f44 the `@0x2A` payload (1000 on every
    // node) and f50 the `@0x30` remembered-eye-z (0 on a node), so an
    // imported column read a gap of `72 - 4*(12-1000)` = 4024 and
    // stacked all 11 nodes at the head's own z: the tail flew apart
    // and collapsed to one plane in the same pair. The HEAD's `@0x30`
    // IS its eye-z home (`sub_331A0` EF:24188 writes it) so f50 stays
    // uniform there; only the nodes divert.
    // Census, mc2l6-rsg t=10086..10570 (the take's only whirlwind,
    // 4043 sampled ticks): model 75 `@0x2C` = 1..11 exactly 36 rows
    // each, `@0x30` = 0 on all 396, `@0x36` = the 11 stack offsets,
    // `@0x2A` = 1000 on all 396; model 22 `@0x2C`/`@0x30`/`@0x36` = 0.
    if r.class3f == 10 && matches!(r.model40, 22 | 75) && c10_field_home() {
        e.f44 = r.f2c as u16;
        if r.model40 == 75 {
            e.f50 = r.f36 as i16;
        }
    }
    // The (10,39)/(10,57) mana sphere keeps its z-velocity in
    // `word_0x2C_44` (TransformArcherToMana EF:26188-91; the uniform
    // @0x2E home is dead on spheres) — the ball tick's z-vel lane is
    // f46. The uniform flag map also drops two mover latches: byte0
    // & 0x40 = the absorb-chase mode (EF:26111), byte1 & 0x20 = the
    // decay channel (EF:26289 — the port's bit-13 tail). The settle
    // countdown @0x39 already rides the generic f58 ← b39 map.
    if r.class3f == 10 && matches!(r.model40, 39 | 57) {
        e.f46 = r.f2c;
        if b0 & 0x40 != 0 {
            e.flags |= 0x40;
        }
        // (`byte[1] & 0x20` — the decay channel — is now carried
        // UNIFORMLY by the flag translation above, which the class-9
        // hate ledger needs too; this scoped copy was its only seat.)
    }
    // The (10,79) castle DEFENDER PIECE (ctor sub_508E0 EF:36987,
    // tick sub_3AF00 EF:30106) is minted with a FRESH field layout —
    // the piece never carried any prior class's homes, so the uniform
    // alias table mis-reads eleven of them (mc2/castle.rs
    // mc2_castle_piece_tick lists the homes). The killer is
    // recoil f68: the uniform map reads @0x43 (part-type, nonzero) as
    // the recoil step, so every imported piece re-applies a 115-unit
    // (0.449-tile) launch displacement each pair — the whole 335k-row
    // y family. Restore all eleven from their retail offsets (f63 tick
    // counter @0x3E, f71 state @0x46, and the @0x9A/@0x9C/@0x9E home
    // anchor are already uniform-correct):
    //   dwell/windup  f44 ← dword_0x10_16 (scratch10)
    //   fire mode     f30 ← word_0x2C_44  (f2c)
    //   burst count   f69 ← fontTypeIndex_0x3D_61 (b3d)
    //   recoil step   f68 ← byte_0x44_68  (b44)
    //   windup z-boost f54 ← word_0x36_54 (f36)
    //   target slot   f28 ← word_0x96_150 (target96)
    //   firing yaw    f34 ← yaw_0x1C, pitch f36 ← pitch_0x1E
    //   level tag     f26 ← word_0x4A_74  (sv_timer → z height offset)
    //   part-type     f67 ← byte_0x43_67  (b43)
    if r.class3f == 10 && r.model40 == 79 {
        e.f26 = r.sv_timer;
        e.f28 = tr(r.target96);
        e.f30 = r.f2c as u16;
        e.f34 = r.yaw as u16;
        e.f36 = r.pitch as u16;
        e.f44 = r.scratch10 as u16;
        e.f54 = r.f36;
        e.f67 = r.b43 as u8;
        e.f68 = r.b44 as u8;
        e.f69 = r.b3d as u8;
    }
    // Balloon ceiling-walk latch (sub_60D50 EF:61896/61905/61921,
    // byte0 & 1): actSpeed 96 walking / 48 flying, ceiling clamp
    // flying-only. Port bit 0 is overloaded per class, so the import
    // stays (3,3)-scoped (mc2/castle.rs is the sole reader); without
    // it every imported ceiling-walker re-took the flying branch —
    // the mc2l30 (3,3) retail-+48 speed family.
    if r.class3f == 3 && r.model40 == 3 && b0 & 1 != 0 {
        e.flags |= 1;
    }
    // ⭐ A WIZARD'S FALL VELOCITY LIVES AT @0x2C, NOT @0x2E. The death
    // fall's gravity accumulator is `word_0x2C_44` (`sub_5E310`
    // EF:60081-90 — `z += word_0x2C_44; word_0x2C_44 -= 2`, terminal
    // −256, upward zeroed), and @0x2E is DEAD on the class (retail's
    // rival 378 steps `f2c` 0 → −18 across mc2l6-rsg t=1846..1854
    // while `f2e` holds 0 throughout). The port's accumulator is
    // `f46`, which the uniform map homes at @0x2E — so the lane was
    // graded against the wrong offset AND had no import seat, and
    // EVERY imported corpse restarted its fall from velocity 0: the
    // mc2l6-rsg t=1882 pair lands the wizard at z 145 where retail
    // records 128 and misses the whole payout by one tick.
    if r.class3f == 3 && matches!(r.model40, 0 | 1) {
        e.f46 = r.f2c;
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
/// EVERY field of a recorded MC1 pool record as named lanes, in the
/// retail struct's own order — the recording-side half of the
/// `dump-state --port` side-by-side and the whole vocabulary of the
/// `explain` changelog. Lane names and order are the contract shared
/// with [`World::port_ent_lanes_mc1`] (the port half joins by NAME,
/// so a missing lane is loud, never silently misaligned). `f58`
/// prints as the unsigned byte — the canonical representation
/// (`import_ent`'s `as u8` law). Guest pointers are omitted.
pub fn retail_ent_lanes_mc1(r: &RetailEntMc1) -> Vec<(&'static str, i64)> {
    vec![
        ("rand", r.rand as i64),
        ("max_life", r.max_life as i64),
        ("act_life", r.act_life as i64),
        ("flags", r.flags as i64),
        ("next20", r.next20 as i64),
        ("prev22", r.prev22 as i64),
        ("id24", r.id24 as i64),
        ("f26", r.f26 as i64),
        ("f28", r.f28 as i64),
        ("f30", r.f30 as i64),
        ("f32", r.f32 as i64),
        ("f34", r.f34 as i64),
        ("f36", r.f36 as i64),
        ("f38", r.f38 as i64),
        ("f40", r.f40 as i64),
        ("f42", r.f42 as i64),
        ("f44", r.f44 as i64),
        ("f46", r.f46 as i64),
        ("f48", r.f48 as i64),
        ("f50", r.f50 as i64),
        ("f52", r.f52 as i64),
        ("f54", r.f54 as i64),
        ("f56", r.f56 as i64),
        ("f58", r.f58 as u8 as i64),
        ("f59", r.f59 as i64),
        ("f61", r.f61 as i64),
        ("f62", r.f62 as i64),
        ("f63", r.f63 as i64),
        ("class64", r.class64 as i64),
        ("model65", r.model65 as i64),
        ("f66", r.f66 as i64),
        ("f67", r.f67 as i64),
        ("f68", r.f68 as i64),
        ("f69", r.f69 as i64),
        ("f70", r.f70 as i64),
        ("f71", r.f71 as i64),
        ("x", r.x as i64),
        ("y", r.y as i64),
        ("z", r.z as i64),
        ("f78", r.f78 as i64),
        ("f80", r.f80 as i64),
        ("f82", r.f82 as i64),
        ("f84", r.f84 as i64),
        ("type86", r.type86 as i64),
        ("frame88", r.frame88 as i64),
        ("frames89", r.frames89 as i64),
        ("mail0.amt", r.mail[0].0 as i64),
        ("mail0.src", r.mail[0].1 as i64),
        ("mail1.amt", r.mail[1].0 as i64),
        ("mail1.src", r.mail[1].1 as i64),
        ("mail2.amt", r.mail[2].0 as i64),
        ("mail2.src", r.mail[2].1 as i64),
        ("mail3.amt", r.mail[3].0 as i64),
        ("mail3.src", r.mail[3].1 as i64),
        ("mail4.amt", r.mail[4].0 as i64),
        ("mail4.src", r.mail[4].1 as i64),
        ("mail5.amt", r.mail[5].0 as i64),
        ("mail5.src", r.mail[5].1 as i64),
        ("f126", r.f126 as i64),
        ("f128", r.f128 as i64),
        ("f130", r.f130 as i64),
        ("f132", r.f132 as i64),
        ("f136", r.f136 as i64),
        ("f140", r.f140 as i64),
        ("f144", r.f144 as i64),
        ("f146", r.f146 as i64),
        ("f148", r.f148 as i64),
        ("dest_x", r.dest_x as i64),
        ("dest_y", r.dest_y as i64),
        ("site_z", r.site_z as i64),
    ]
}

/// EVERY field of a recorded MC2 pool record as named lanes, in the
/// retail struct's own order — [`retail_ent_lanes_mc1`]'s twin, the
/// shared vocabulary of the MC2 `explain` changelog and the
/// recording-side half of `dump-state --port`. Same contract: lane
/// names and order are shared with [`World::port_ent_lanes_mc2`], the
/// port half joins BY NAME. Conventions:
/// - the `int8_t` byte lanes (`b38`..`b47`, `sv1`/`sv2`, the anim
///   bytes) print as the UNSIGNED byte — the import's canonical form
///   (the `b39` −6 sentinel law), matching what the port holds;
/// - `flags` prints raw, then the TRANSLATED bits (the exact set
///   `import_ent_mc2` carries) as 0/1 sub-lanes named by retail byte
///   (`b0`..`b2`) — the raw lane's port half is `—` because the two
///   flag words share no whole-dword representation;
/// - the guest pointers print raw (`ptr_a0` moving = the behavior
///   row changed), port half `—`.
pub fn retail_ent_lanes_mc2(r: &RetailEntMc2) -> Vec<(&'static str, i64)> {
    let bit = |k: u32| (r.flags >> k & 1) as i64;
    vec![
        ("next0", r.next0 as i64),
        // ⚠ INSTRUMENT-ONLY NORMALISATION, not a law. `RetailEntMc2`
        // types @0x04 as `i32` while the port's `Ent::max_life` is the
        // `u32` the importer deliberately seats BIT-PRESERVING (see
        // `import_ent_mc2`: retail's lightning trail stamps a node's
        // maxLife to -1, sub_66750 EF:58336-43). Rendering the retail
        // half signed made every one of those records read
        // `retail -1 / port 4294967295` — the SAME 32 BITS shown two
        // ways. Round 104 banked that as "a DISPLAY artifact, not a
        // diff"; round 105 measured the cost of leaving it in the
        // table: on mc2l22 it was 577,985 of the all-lane census's
        // 843,608 rows — 68.5% — all of it on (9,9), drowning the
        // real ungraded-lane signal the census exists to surface.
        // `as u32` matches the importer's own canonical
        // representation, so the round-trip contract below
        // (`world.rs` lane-name/value test) still holds exactly.
        // ⭐ MC1's twin `retail_ent_lanes_mc1` needs NO such change —
        // `RetailEntMc1::max_life` is already `u32`. Checked, per "a
        // law landed on one call path is not landed".
        ("max_life", r.max_life as u32 as i64),
        ("life", r.life as i64),
        ("flags", r.flags as i64),
        ("flags.b0_walk1", bit(0)),
        ("flags.b0_done2", bit(1)),
        ("flags.b0_link4", bit(2)),
        ("flags.b0_coll8", bit(3)),
        ("flags.b0_x20", bit(5)),
        ("flags.b0_chase40", bit(6)),
        ("flags.b1_reap4", bit(10)),
        ("flags.b1_x8", bit(11)),
        ("flags.b1_decay20", bit(13)),
        ("flags.b2_kill1", bit(16)),
        ("flags.b2_x4", bit(18)),
        ("flags.b2_x10", bit(20)),
        ("flags.b2_x20", bit(21)),
        ("scratch10", r.scratch10 as i64),
        ("rand", r.rand as i64),
        ("next16", r.next16 as i64),
        ("prev18", r.prev18 as i64),
        ("f1a", r.f1a as i64),
        ("yaw", r.yaw as i64),
        ("pitch", r.pitch as i64),
        ("roll", r.roll as i64),
        ("f22", r.f22 as i64),
        ("f24", r.f24 as i64),
        ("f26", r.f26 as i64),
        ("owner28", r.owner28 as i64),
        ("f2a", r.f2a as i64),
        ("f2c", r.f2c as i64),
        ("f2e", r.f2e as i64),
        ("f30", r.f30 as i64),
        ("f32", r.f32 as i64),
        ("f34", r.f34 as i64),
        ("f36", r.f36 as i64),
        ("b38", r.b38 as u8 as i64),
        ("b39", r.b39 as u8 as i64),
        ("b3a", r.b3a as u8 as i64),
        ("b3b", r.b3b as u8 as i64),
        ("b3c", r.b3c as u8 as i64),
        ("b3d", r.b3d as u8 as i64),
        ("phase3e", r.phase3e as i64),
        ("class3f", r.class3f as i64),
        ("model40", r.model40 as i64),
        ("b41", r.b41 as u8 as i64),
        ("b42", r.b42 as u8 as i64),
        ("b43", r.b43 as u8 as i64),
        ("b44", r.b44 as u8 as i64),
        ("action45", r.action45 as i64),
        ("b46", r.b46 as u8 as i64),
        ("b47", r.b47 as u8 as i64),
        ("sv1", r.sv1 as u8 as i64),
        ("sv2", r.sv2 as u8 as i64),
        ("sv_timer", r.sv_timer as i64),
        ("x", r.x as i64),
        ("y", r.y as i64),
        ("z", r.z as i64),
        ("ayaw", r.ayaw as i64),
        ("apitch", r.apitch as i64),
        ("aroll", r.aroll as i64),
        ("afov", r.afov as i64),
        ("f5a", r.f5a as i64),
        ("b5c", r.b5c as u8 as i64),
        ("b5d", r.b5d as u8 as i64),
        ("mail0.amt", r.mail[0].0 as i64),
        ("mail0.src", r.mail[0].1 as i64),
        ("mail1.amt", r.mail[1].0 as i64),
        ("mail1.src", r.mail[1].1 as i64),
        ("mail2.amt", r.mail[2].0 as i64),
        ("mail2.src", r.mail[2].1 as i64),
        ("mail3.amt", r.mail[3].0 as i64),
        ("mail3.src", r.mail[3].1 as i64),
        ("mail4.amt", r.mail[4].0 as i64),
        ("mail4.src", r.mail[4].1 as i64),
        ("mail5.amt", r.mail[5].0 as i64),
        ("mail5.src", r.mail[5].1 as i64),
        ("speed", r.speed as i64),
        ("min_speed", r.min_speed as i64),
        ("max_speed", r.max_speed as i64),
        ("d88", r.d88 as i64),
        ("mana_max", r.mana_max as i64),
        ("mana", r.mana as i64),
        ("player_ent", r.player_ent as i64),
        ("target96", r.target96 as i64),
        ("f98", r.f98 as i64),
        ("dest_x", r.dest_x as i64),
        ("dest_y", r.dest_y as i64),
        ("dest_z", r.dest_z as i64),
        ("ptr_a0", r.ptr_a0 as i64),
        ("ptr_a4", r.ptr_a4 as i64),
    ]
}

fn import_ent(r: &RetailEntMc1, row156: u8, tr: &dyn Fn(u16) -> u16) -> Ent {
    // The castle (3,2) keeps its MACRO state in retail's job byte +70
    // (4 = settled, 5 = transforming, 6 = leveler — the three dispatch
    // rows at :4673-75), which `tick70` carries verbatim below, and
    // its TRANSFORM SUB-STATE in +48, which the port keeps in f59
    // (0 = level-up commit, 1/6 = painter/leveler waits, 2/3/5 =
    // finish/repaint/handoff). Retail's +59 byte is itself dead for
    // castles — importing it verbatim parked every settled castle in
    // f59 = 0 and re-upgraded it one level per tick (the phantom-
    // upgrade family, docs/CONFORMANCE-FINDINGS.md entry 3).
    //
    // Retail's pure-wait +48 values 1 and 4 are the same wait (the
    // level-up painter's and the repaint painter's) and both land on
    // our state 1. Outside +70 == 5 the sub-state is DEAD — every
    // entry into the transform machine writes +48 first (:55988,
    // :56010, :56469) — so it imports as 0 and the raw-shadow f59
    // lane stays quiet on the settled castles that make up almost
    // every captured tick.
    let f59 = if r.class64 == 3 && r.model65 == 2 {
        if r.f70 == 5 {
            match r.f48 {
                1 | 4 => 1,
                s => (s as u8).min(6),
            }
        } else {
            0
        }
    } else {
        r.f59
    };
    Ent {
        // MC1 has no @0x2E charm lane (dig 98-Q20's field is MC2-only).
        lease2e: crate::engine::features::Lease2e(0),
        // ⭐ Retail's `+48`, RAW, for every class — `sub_14E60`
        // (CARPET.EXE 0x2D658) has no class guard, so its callers read
        // this word off whatever record now sits in a wizard's stale
        // owned-token slot. `f26` above only homes it while the record
        // is still a class-12 manifestation.
        raw48: crate::engine::features::Raw48(r.f48),
        rand: r.rand,
        max_life: r.max_life,
        act_life: r.act_life,
        flags: r.flags & !4,
        next20: 0,
        prev22: 0,
        // Class-11 id24 is the trigger's DISPOSITION id, not a slot
        // reference: a dis that numerically equals the human's pool
        // slot must not become PLAYER_TARGET (l32's breadcrumb dis 14
        // vs human slot 14 — the fire resolved dis 65535, whose table
        // rows are the consumed load-sentinel set, and the mass spawn
        // silently vanished; obs untr() masked the id from the diff).
        id24: if r.class64 == 11 { r.id24 } else { tr(r.id24) },
        f38: tr(r.f38),
        f40: tr(r.f40),
        f46: r.f46,
        f50: r.f50,
        f68: r.f68,
        f69: r.f69,
        mail: r.mail.map(|(a, s)| (a, tr(s))),
        // Class-12 tokens: retail +144 is always 0; the token's OWNER
        // wizard carpet slot lives in +42 (the lane the Ent doesn't
        // otherwise model). Stamp it into f144 so the active-token
        // arms (the Accelerate contrail) can resolve a RIVAL owner's
        // pose — corpus-proven: every hw:0 token reads f42 = its
        // wizard's carpet slot, f144 = 0.
        f144: if r.class64 == 12 && r.f144 == 0 {
            tr(r.f42)
        } else {
            tr(r.f144)
        },
        // The port keeps a manifestation's burst/refire counter in f26
        // (retail: +48; retail's +26 is the SPELL LEVEL there).
        f26: if r.class64 == 12 { r.f48 as i16 } else { r.f26 },
        // The castle ground LEVELER's "current" rung lives at retail
        // +48 (sub_28200 :30333-36); the port keeps it in f28.
        // Without the re-home an imported MID-RUN leveler read
        // current 0 and stepped (target − 0)/counter — the mound
        // ROSE runaway where retail translated it down (mc1l0
        // castle-663 transform windows t=1164-1350, +160/tick
        // through 52/1 = +1664 at the window end; the terrain
        // divergence every downstream walker then inherited).
        f28: if r.class64 == 10 && r.model65 == 41 {
            r.f48
        } else {
            r.f28
        },
        f30: r.f30,
        f32: r.f32,
        f44: r.f44,
        f34: r.f34,
        f36: r.f36,
        f52: tr(r.f52),
        f54: tr(r.f54),
        f56: r.f56,
        // ⚠ `as u8` FIRST. `+58` is an `int8_t` in retail (Basic.h:394)
        // and the recorder lifts it as `i8`, so the never-woken
        // sentinel 0xFA arrives as −6 — while `Gen::new_event` mints
        // it as 250. Two representations of ONE byte inside the port
        // is how `f58 <= 0` in the MC2 aim scan came to mean opposite
        // things for an imported and a native record. Canonical form
        // is the UNSIGNED byte, which is what `mob_awake_pass`'s
        // `& 0xFF` countdown and the raw shadow's mask already assume.
        f58: r.f58 as u8 as i16,
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

// ------------------------------------------------- replay chain seeding
//
// The pure-input replay consumers (`mgc-conform replay`, the app's
// `--replay`) seed the chained human flight state ONCE from a recorded
// closure and free-run on recovered input. The field maps are the pose
// channel's (docs/CONFORMANCE.md "The pose channel"); they live here so
// both consumers share one seeding law.

/// Seed the chained MC1/HW flight state from the recorded closure at
/// an anchor.
pub fn mc1_state_from_retail(st: &RetailMc1, slot: u16) -> Mc1State {
    let e = &st.ents[slot as usize];
    let w = &st.wizards[st.local_player as usize];
    Mc1State {
        x: e.x,
        y: e.y,
        z: e.z,
        yaw: e.f30 & 0x7FF,
        roll_f: w.roll_acc as i16,
        pitch_f: w.pitch_acc as i16,
        aim_pitch: e.f32 & 0x7FF,
        eff_pitch: w.eff_pitch & 0x7FF,
        act_speed: e.f126,
        tgt_speed: w.cmd_speed,
        strafe: w.strafe,
        tick_ctr: e.f63,
        rand: e.rand,
    }
}

/// The MC2 twin — plus the debuff ladders and water/nudge channels
/// the pose channel gates instead of seeding. `row` is the world's
/// live carpet tuning row ([`World::mc2_carpet_row`]).
pub fn mc2_state_from_retail(st: &RetailMc2, slot: u16, row: Mc2Row) -> (Mc1State, Mc2Ext) {
    let e = &st.ents[slot as usize];
    let p = &st.players[st.local_player as usize];
    (
        Mc1State {
            x: e.x,
            y: e.y,
            z: e.z,
            yaw: e.yaw as u16 & 0x7FF,
            roll_f: p.roll_acc as i16,
            pitch_f: p.pitch_acc as i16,
            aim_pitch: e.pitch as u16 & 0x7FF,
            eff_pitch: p.eff_pitch & 0x7FF,
            act_speed: e.speed,
            tgt_speed: p.cmd_speed,
            strafe: p.strafe,
            tick_ctr: 0,
            rand: 0,
        },
        Mc2Ext {
            move_speed: p.move_speed,
            move_speed_ctr: p.move_speed_ctr,
            mobilize: p.mobilize,
            mobilize_ctr: p.mobilize_ctr,
            add: (0, 0, 0),
            water_ctr: p.water_ctr as u16,
            nudge_latch: p.nudge_latch != 0,
            row,
            whirl_bumps: 0,
        },
    )
}

/// The integer carpet as the world-tick pose — the faithful path's
/// pose law: heading/pitch/speed straight off the chained state, no
/// float round-trip.
pub fn integer_pose(s: &Mc1State) -> PlayerPose {
    PlayerPose {
        x: s.x,
        y: s.y,
        z: s.z,
        heading: s.yaw,
        pitch: s.aim_pitch,
        speed: s.act_speed,
    }
}

/// Keep only the lanes that part. The grader's filter, factored out so
/// the full-lane form below can share one lane list with it.
fn dirty(rows: Vec<(&'static str, i64, i64)>) -> Vec<(&'static str, i64, i64)> {
    rows.into_iter().filter(|&(_, w, g)| w != g).collect()
}

/// `MGC_POSE_WINDOW=<t0>-<t1>` — THE CROSS-DRIVER POSE MICROSCOPE.
///
/// Every graded lane at every tick in the window, retail beside port,
/// from BOTH retail drivers (`mgc-conform replay` and the app's
/// `--replay-check`). Dirty lanes print `retail|port` and the line
/// names them at the end; clean lanes print one value.
///
/// ⭐⭐ **THIS EXISTS BECAUSE IT WAS HAND-BUILT AND THROWN AWAY TWICE**
/// (sessions 38 and 40), and both times it cracked a fork that had been
/// banked for multiple sessions in minutes. The grade sites hold both
/// sides already — the only thing ever missing was the print.
///
/// ⚠ It prints the GRADED boundary state. A lane that moves and comes
/// back inside one tick is invisible here; that is what the world-side
/// [`crate::engine::world::World`] carpet probe (`MGC_CARPET_PROBE`) is
/// for.
pub fn pose_window() -> Option<(u64, u64)> {
    static W: std::sync::OnceLock<Option<(u64, u64)>> = std::sync::OnceLock::new();
    *W.get_or_init(|| {
        let v = std::env::var("MGC_POSE_WINDOW").ok()?;
        let (a, b) = v.split_once('-')?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    })
}

/// Print one pose-window line. `extras` carries retail-only context the
/// lane set has no port counterpart for (`f2c`, `action45` — the death
/// column's two tells, which is how t=7367 was ruled OUT of it).
pub fn emit_pose_window(t: u64, rows: &[(&'static str, i64, i64)], extras: &[(&'static str, i64)]) {
    let Some((t0, t1)) = pose_window() else {
        return;
    };
    if t < t0 || t > t1 {
        return;
    }
    let mut line = format!("POSE t={t}");
    let mut bad = Vec::new();
    for &(name, want, got) in rows {
        let short = name.strip_prefix("pose.").unwrap_or(name);
        if want == got {
            line.push_str(&format!(" {short}={want}"));
        } else {
            line.push_str(&format!(" {short}={want}|{got}"));
            bad.push(short);
        }
    }
    for &(name, v) in extras {
        line.push_str(&format!(" {name}={v}"));
    }
    if !bad.is_empty() {
        line.push_str(&format!("  <-- DIRTY {}", bad.join(",")));
    }
    println!("{line}");
}

/// Pose lanes: a chained carpet vs the recorded pose at a graded
/// boundary (the pose channel's lane set). Rows are
/// `(lane, retail, port)`, dirty lanes only.
pub fn pose_lanes_mc1(
    s: &Mc1State,
    e: &RetailEntMc1,
    w: &RetailWizardMc1,
) -> Vec<(&'static str, i64, i64)> {
    dirty(pose_all_mc1(s, e, w))
}

/// The same lane set with the CLEAN lanes kept — what
/// [`emit_pose_window`] prints. `pose_lanes_mc1` is this filtered, so
/// there is one definition of "the pose channel's lanes" and the
/// microscope cannot drift from the grader.
pub fn pose_all_mc1(
    s: &Mc1State,
    e: &RetailEntMc1,
    w: &RetailWizardMc1,
) -> Vec<(&'static str, i64, i64)> {
    let mut rows = Vec::new();
    let mut lane = |name, want: i64, got: i64| {
        rows.push((name, want, got));
    };
    lane("pose.x", e.x as i64, s.x as i64);
    lane("pose.y", e.y as i64, s.y as i64);
    lane("pose.z", e.z as i64, s.z as i64);
    lane("pose.yaw", (e.f30 & 0x7FF) as i64, s.yaw as i64);
    lane("pose.aim_pitch", (e.f32 & 0x7FF) as i64, s.aim_pitch as i64);
    lane(
        "pose.eff_pitch",
        (w.eff_pitch & 0x7FF) as i64,
        (s.eff_pitch & 0x7FF) as i64,
    );
    lane("pose.act_speed", e.f126 as i64, s.act_speed as i64);
    lane("pose.tgt_speed", w.cmd_speed as i64, s.tgt_speed as i64);
    lane("pose.strafe", w.strafe as i64, s.strafe as i64);
    lane("pose.roll_f", w.roll_acc as i16 as i64, s.roll_f as i64);
    lane("pose.pitch_f", w.pitch_acc as i16 as i64, s.pitch_f as i64);
    lane("pose.tick_ctr", e.f63 as i64, s.tick_ctr as i64);
    lane("pose.rand", e.rand as i64, s.rand as i64);
    rows
}

/// The MC2 lane set. `water_ctr` IS a lane as of 2026-09-15 — the last
/// item the remc2 round-5 waterCounter finding left owed.
///
/// Its law was verified from the other side first. The remc2 replay
/// corpus found the same hole in remc2 — retail bumps the counter on
/// EVERY refused move in a cave, `incb 0x262(%eax)` at NETHERW.EXE
/// 0x81ccc, not only on the deep-water head branch at 0x818e3 — and
/// with that instruction restored its two cave takes (`mc2l7`,
/// `mc2l30`) grade BIT-PERFECT end to end, a positive test of the
/// ++/−− law over 43,000 cave ticks. The port had the identical hole;
/// `flight.rs` carries the refusal bump. Grading it here is what the
/// register meant by "plumb `Mc2Ext` into `pose_all_mc2`": the counter
/// lives in `Mc2Ext`, not `Mc1State`, so the function had no way to
/// see it. Retail holds it in one byte (player +610) and the port in a
/// u16 masked to 0xFF on each bump, so the lane masks to compare.
pub fn pose_lanes_mc2(
    s: &Mc1State,
    ext: &Mc2Ext,
    e: &RetailEntMc2,
    p: &RetailPlayerMc2,
) -> Vec<(&'static str, i64, i64)> {
    dirty(pose_all_mc2(s, ext, e, p))
}

/// The MC2 lane set with the clean lanes kept — see [`pose_all_mc1`].
pub fn pose_all_mc2(
    s: &Mc1State,
    ext: &Mc2Ext,
    e: &RetailEntMc2,
    p: &RetailPlayerMc2,
) -> Vec<(&'static str, i64, i64)> {
    let mut rows = Vec::new();
    let mut lane = |name, want: i64, got: i64| {
        rows.push((name, want, got));
    };
    lane("pose.x", e.x as i64, s.x as i64);
    lane("pose.y", e.y as i64, s.y as i64);
    lane("pose.z", e.z as i64, s.z as i64);
    lane("pose.yaw", (e.yaw as u16 & 0x7FF) as i64, s.yaw as i64);
    lane(
        "pose.aim_pitch",
        (e.pitch as u16 & 0x7FF) as i64,
        s.aim_pitch as i64,
    );
    lane(
        "pose.eff_pitch",
        (p.eff_pitch & 0x7FF) as i64,
        (s.eff_pitch & 0x7FF) as i64,
    );
    lane("pose.act_speed", e.speed as i64, s.act_speed as i64);
    lane("pose.tgt_speed", p.cmd_speed as i64, s.tgt_speed as i64);
    lane("pose.strafe", p.strafe as i64, s.strafe as i64);
    lane("pose.roll_f", p.roll_acc as i16 as i64, s.roll_f as i64);
    lane("pose.pitch_f", p.pitch_acc as i16 as i64, s.pitch_f as i64);
    // `water_ctr` — promoted to a lane 2026-09-15, the last thing the
    // remc2 round-5 waterCounter finding left owed. Retail keeps it in a
    // single byte at player +610 and the port keeps it in `Mc2Ext` as a
    // u16 it masks to 0xFF on every bump, so the comparison masks too.
    lane(
        "pose.water_ctr",
        p.water_ctr as i64,
        (ext.water_ctr & 0xFF) as i64,
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ THE MICROSCOPE AND THE GRADER READ ONE LANE LIST.
    /// `MGC_POSE_WINDOW` prints `pose_all_*`; the verdict is
    /// `pose_lanes_*`, which is that filtered — so a lane added to the
    /// grader appears in the window automatically and the two can never
    /// disagree about what "the pose channel" is. (The temporary
    /// versions of this window, hand-built twice, each maintained their
    /// own lane list.)
    ///
    /// Non-vacuous: this state parts exactly two of the eleven lanes,
    /// so the assert fails both if the filter stops filtering and if
    /// the two lists drift apart.
    #[test]
    fn the_pose_window_prints_the_lanes_the_grader_filters() {
        let e = RetailEntMc2 {
            x: 100,
            y: 200,
            z: 300,
            yaw: 5,
            pitch: 6,
            speed: 7,
            ..Default::default()
        };
        let p = RetailPlayerMc2::default();
        let s = Mc1State {
            x: 100,
            y: 201, // parts
            z: 300,
            yaw: 5,
            aim_pitch: 6,
            act_speed: 8, // parts
            ..Default::default()
        };
        let ext = Mc2Ext::default();
        let all = pose_all_mc2(&s, &ext, &e, &p);
        let want: Vec<_> = all.iter().copied().filter(|&(_, w, g)| w != g).collect();
        assert_eq!(all.len(), 12, "the MC2 lane set");
        assert_eq!(pose_lanes_mc2(&s, &ext, &e, &p), want);
        assert_eq!(
            want.iter().map(|r| r.0).collect::<Vec<_>>(),
            ["pose.y", "pose.act_speed"]
        );

        // The MC1 twin — same law, two more lanes (`tick_ctr`/`rand`).
        let e1 = RetailEntMc1 {
            x: 100,
            ..Default::default()
        };
        let w1 = RetailWizardMc1::default();
        let s1 = Mc1State {
            x: 101, // parts
            ..Default::default()
        };
        let all1 = pose_all_mc1(&s1, &e1, &w1);
        assert_eq!(all1.len(), 13, "the MC1 lane set");
        assert_eq!(
            pose_lanes_mc1(&s1, &e1, &w1),
            all1.iter()
                .copied()
                .filter(|&(_, w, g)| w != g)
                .collect::<Vec<_>>()
        );
        assert_eq!(pose_lanes_mc1(&s1, &e1, &w1).len(), 1);
    }

    /// The (10,79) castle DEFENDER PIECE (ctor sub_508E0 / tick
    /// sub_3AF00) invents a fresh field layout the uniform alias map
    /// mis-reads on import — most damagingly f68 (recoil) off the
    /// part-type byte @0x43, which re-applies a 115-unit launch
    /// displacement every pair (the mc2l24 335k-row y family). Pin
    /// each home to its retail offset. Distinct sentinels make this
    /// non-vacuous: reverting the import block reads f68←@0x43(=2),
    /// f26←@0x10(=42), f34←@0x20(=0), f69←@0x44(=251 from b44), f28←0,
    /// so each assert below flips.
    #[test]
    fn mc2_castle_piece_import_field_homes() {
        let r = RetailEntMc2 {
            class3f: 10,
            model40: 79,
            scratch10: 42, // @0x10 dwell/windup → f44
            yaw: 300,      // @0x1C firing yaw + obs heading → f34
            pitch: 111,    // @0x1E firing pitch + obs pitch → f36
            roll: 0,       // @0x20 (uniform f34) — kept distinct from yaw
            f2c: 3,        // @0x2C fire mode → f30
            f36: 160,      // @0x36 windup z-boost → f54
            b3d: 6,        // @0x3D burst count → f69
            phase3e: 251,  // @0x3E tick counter → f63 (already uniform)
            b43: 2,        // @0x43 part-type → f67
            b44: -5,       // @0x44 recoil step → f68
            b46: 3,        // @0x46 state → f71 (already uniform)
            sv_timer: 6,   // @0x4A level tag → f26 (z height offset)
            target96: 77,  // @0x96 latched target → f28
            dest_x: 1000,  // @0x9A/@0x9C/@0x9E home anchor → dest/site
            dest_y: 2000,
            dest_z: 1760,
            ..Default::default()
        };
        let e = import_ent_mc2(&r, 619, 79, &|v| v);
        assert_eq!(e.class64, 10);
        assert_eq!(e.model65, 79);
        assert_eq!(e.f44, 42, "dwell @0x10");
        assert_eq!(e.f34, 300, "firing yaw / obs heading @0x1C");
        assert_eq!(e.f36, 111, "firing pitch / obs pitch @0x1E");
        assert_eq!(e.f30, 3, "fire mode @0x2C");
        assert_eq!(e.f54, 160, "windup z-boost @0x36");
        assert_eq!(e.f69, 6, "burst @0x3D");
        assert_eq!(e.f63, 251, "tick counter @0x3E");
        assert_eq!(e.f67, 2, "part-type @0x43");
        assert_eq!(e.f68, (-5i8) as u8, "recoil @0x44 (NOT part-type @0x43)");
        assert_eq!(e.f71, 3, "state @0x46");
        assert_eq!(e.f26, 6, "level tag @0x4A");
        assert_eq!(e.f28, 77, "latched target @0x96");
        assert_eq!(e.dest_x, 1000);
        assert_eq!(e.dest_y, 2000);
        assert_eq!(e.site_z, 1760);
    }

    /// ⭐⭐ **THE LANE TABLE'S `piece` LIST HAD A HOLE, AND IT WAS THE
    /// ONE DIVERTED LANE NOBODY CHECKED.** `import_ent_mc2` seats a
    /// (10,79) defender piece's `@0x96` into **`f28`** (the test above
    /// pins it) and `mc2_castle_piece_tick` reads only `f28` — but
    /// `port_ent_lanes_mc2`'s `target96` lane published `f146`, the
    /// UNIFORM home, which for a piece only ever holds the value the
    /// generic seat wrote from LAST pair's `target96`. Every other
    /// diverted piece home (`scratch10`, `heading`, `pitch`, `roll`,
    /// `f2a`, `f2c`, `f34`, `f36`, `b3d`, `b42`, `b43`, `b44`) already
    /// had its `piece` arm; `target96` did not.
    ///
    /// The census signature was a ONE-TICK-STALE register, not a
    /// defect: mc2l22's 4,636 `(10,79) target96` rows came in PERFECTLY
    /// SYMMETRIC pairs — 14 rows `retail 611 / port 0` beside 14 rows
    /// `retail 0 / port 611`, 10 and 10 for 328, 8 and 8 for 320, and
    /// so on for every value in the family. Concrete witness —
    /// `dump-state recordings/mc2l22.mgcr 1069 727 --port --start 1068`
    /// (slot 727 is a castle defender piece): the lane read
    /// `retail 277 / port 0 ≠` before and `retail 277 / port 277`
    /// after, and at t=1070 it was already clean either way because
    /// the stale `f146` had caught up one tick later.
    ///
    /// Non-vacuous by distinct sentinels: with
    /// `MGC_NO_MC2_PIECE_TARGET96_LANE=1` the lane falls back to
    /// `e.f146` and publishes 999 here instead of 77.
    #[test]
    fn the_piece_s_target96_lane_reads_f28_not_f146() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);

        let lane = |w: &World, slot: u16, name: &str| -> Option<i64> {
            w.port_ent_lanes_mc2(slot, 424, false)
                .expect("lanes")
                .into_iter()
                .find(|(n, _)| *n == name)
                .and_then(|(_, v)| v)
        };

        // A (10,79) castle defender piece: its live target is f28; the
        // uniform f146 carries a STALE value the piece brain never reads.
        let mut w2 = w;
        {
            let e = &mut w2.g.ent[80];
            *e = Ent::default();
            e.class64 = 10;
            e.model65 = 79;
            e.id24 = 80;
            e.max_life = 100;
            e.act_life = 100;
            e.f28 = 77; // @0x96, where the import seats it
            e.f146 = 999; // the uniform home — a stale ghost on a piece
        }
        assert_eq!(
            lane(&w2, 80, "target96"),
            Some(77),
            "a (10,79) piece publishes its @0x96 from f28, not the uniform f146"
        );

        // …and every OTHER class still publishes f146 (a (10,23) blast).
        {
            let e = &mut w2.g.ent[81];
            *e = Ent::default();
            e.class64 = 10;
            e.model65 = 23;
            e.id24 = 81;
            e.max_life = 100;
            e.act_life = 100;
            e.f28 = 77;
            e.f146 = 999;
        }
        assert_eq!(
            lane(&w2, 81, "target96"),
            Some(999),
            "a non-piece still publishes the uniform f146"
        );
    }

    /// ⭐⭐⭐ **THE (10,42) BUILD PAINTER'S SETTLE LATCH IS `@0x3B`.**
    /// `sub_50370` stamps `byte_0x3B_59 = 1` and touches `@0x3A`
    /// nowhere — shipped `NETHERW.EXE` file **0x74BAF**,
    /// `c6 43 3b 01  movb $0x1,0x3b(%ebx)` (EF:36745) — and the sole
    /// reader is the painter tick's countdown end at file
    /// **0x5C8C7**, `80 7b 3b 00  cmpb $0x0,0x3b(%ebx) / je`: set ⇒
    /// `@0x10 = -25` (`c7 43 10 e7 ff ff ff`), clear ⇒ `@0x10 = -1`
    /// (EF:27760-65), and `-1` is a death.
    ///
    /// The port homes that latch in `f59`, whose GENERAL seat is
    /// `@0x3A`, so every imported painter came back with the latch
    /// clear and died 21 ticks after birth instead of parking out the
    /// 25-tick settle window. Non-vacuous by distinct sentinels: with
    /// `MGC_NO_MC2_PAINTER_SETTLE_LANE=1` this reads `f59 = 7` (the
    /// `@0x3A` value) and the assert flips.
    #[test]
    fn the_mc2_build_painter_settle_latch_is_the_0x3b_byte() {
        let r = RetailEntMc2 {
            class3f: 10,
            model40: 42,
            b3a: 7, // @0x3A — dead on a painter, retail holds 0
            b3b: 1, // @0x3B — sub_50370's settle latch
            ..Default::default()
        };
        let e = import_ent_mc2(&r, 631, 42, &|v| v);
        assert_eq!(e.class64, 10);
        assert_eq!(e.model65, 42);
        if crate::mc2::castle::no_mc2_painter_settle_lane() {
            assert_eq!(e.f59, 7, "switch set: the old @0x3A seat");
        } else {
            assert_eq!(
                e.f59, 1,
                "the painter's settle latch is @0x3B (sub_50370, EXE 0x74BAF)"
            );
        }

        // Every OTHER class-10 model keeps the general @0x3A seat.
        let other = RetailEntMc2 {
            class3f: 10,
            model40: 41,
            b3a: 7,
            b3b: 1,
            ..Default::default()
        };
        assert_eq!(
            import_ent_mc2(&other, 632, 41, &|v| v).f59,
            7,
            "the (10,42) arm must not widen to the rest of class 10"
        );
    }

    /// The (5,21) DEVIL is a `ramp2c` tenant (2026-08-29): its @0x2C
    /// is the LIVE jump impulse `Gen::m21_jump` integrates (sub_265A0
    /// EF:17098-151), while @0x2A holds a dead uniform 400. Importing
    /// the @0x2A home seeded every devil with +400/tick of bogus
    /// impulse (mc2l24 t=7918: 47 devils; mc2l22 t=1: Δ = 485 =
    /// 400 − (−85)). Its f26 rides the `b44` rest-countdown lane, not
    /// @0x10/@0x2E. Non-vacuous: reverting the ramp2c membership
    /// makes f44 read 400.
    #[test]
    fn mc2_devil_import_takes_the_jump_impulse_from_2c() {
        let r = RetailEntMc2 {
            class3f: 5,
            model40: 21,
            f2a: 400, // the dead @0x2A home — must NOT reach f44
            f2c: -85, // @0x2C — the live (falling) jump impulse
            b44: 12,  // @0x44 rest countdown → f26
            ..Default::default()
        };
        let e = import_ent_mc2(&r, 100, 0, &|v| v);
        assert_eq!(
            e.f44,
            (-85i16) as u16,
            "devil f44 = the @0x2C jump impulse, not the dead @0x2A"
        );
        assert_eq!(e.f26, 12, "devil f26 = the @0x44 rest countdown");
    }

    /// The `owner` obs lane = retail parentId @0x28. The importer must
    /// feed it correctly for the two families that carry a live parent,
    /// and must NOT let the (5,10) pyramid pollute id24 with its
    /// repurposed @0x28 (mc2l24 owner census, 47k rows):
    ///  • (10,42) build painter: @0x28 = the owning castle → fused into
    ///    id24 (the `owner28 != 0` branch) so `obs_project_mc2` recovers
    ///    it directly.
    ///  • (5,0) pyramid-summoned creature: @0x28 = @0x1A = the pyramid
    ///    (entity 7) → id24 = tr(7).
    ///  • (5,10) DOOMSDAY PYRAMID: @0x28 is the (10,14) ring-SPIN ANGLE
    ///    (→ f36), NOT a parent. It must NOT reach id24, or the
    ///    apocalypse summon (`own_id = pyramid.id24`) copies the spin
    ///    angle onto every child; id24 falls through to @0x1A (own id).
    /// Non-vacuous: reverting the (5,10) id24 exclusion makes the last
    /// assert read 288 (the spin angle) instead of 7.
    #[test]
    fn mc2_owner_import_field_homes() {
        let tr = |v: u16| v;
        // (10,42) painter: parent castle @0x28=426, @0x1A=116 (wizard).
        let painter = RetailEntMc2 {
            class3f: 10,
            model40: 42,
            owner28: 426,
            f1a: 116,
            ..Default::default()
        };
        assert_eq!(
            import_ent_mc2(&painter, 162, 0, &tr).id24,
            426,
            "painter id24 = @0x28 castle"
        );
        // (5,0) summoned creature: @0x28 = @0x1A = 7 (the pyramid).
        let summoned = RetailEntMc2 {
            class3f: 5,
            model40: 0,
            owner28: 7,
            f1a: 7,
            ..Default::default()
        };
        assert_eq!(
            import_ent_mc2(&summoned, 917, 0, &tr).id24,
            7,
            "summoned creature id24 = pyramid @0x28"
        );
        // (5,10) pyramid: @0x28=288 (spin angle), @0x1A=7 (own id).
        let pyramid = RetailEntMc2 {
            class3f: 5,
            model40: 10,
            owner28: 288,
            f1a: 7,
            ..Default::default()
        };
        let pe = import_ent_mc2(&pyramid, 7, 0, &tr);
        assert_eq!(
            pe.id24, 7,
            "pyramid id24 = @0x1A own id, NOT the @0x28 spin angle"
        );
        assert_eq!(
            pe.f36, 288,
            "pyramid ring-spin angle still carried in f36 (arm untouched)"
        );
    }

    /// The m27 HYDRA's field homes. The four dig-A words plus the BOLT
    /// POWER `manaRegen_0x88_136` (@0x88 → f136): `sub_2A7F0`
    /// (EF:20513-16) rolls it on the a3=1 shot and the four a3=0
    /// re-fires only read it back, so the uniform f136←@0x8C home
    /// silenced 4/5 of every heavy barrage on replay. @0x8C is dead 0
    /// across the whole (5,27) family (mc2l24 census, 87,210 rows), so
    /// the lane is free. Non-vacuous: the sentinels are all distinct —
    /// reverting the arm reads f136←@0x8C(=999), f36←0, f44←@0x2A(=7),
    /// f50←@0x30(=8), f26←@0x2E(=9).
    #[test]
    fn mc2_m27_import_field_homes() {
        let r = RetailEntMc2 {
            class3f: 5,
            model40: 27,
            scratch10: 4,  // @0x10 whip counter → f26
            f22: 1433,     // @0x22 spline pitch → f36
            f2a: 7,        // @0x2A (uniform f44) — kept distinct
            f2c: 3,        // @0x2C integrate mode → f44
            f2e: 9,        // @0x2E charm lane (uniform f26) — distinct
            f30: 8,        // @0x30 (uniform f50) — distinct
            b3b: 2,        // @0x3B branch index → f50
            d88: 2,        // @0x88 bolt power → f136
            mana_max: 999, // @0x8C — the uniform f136 home, dead on m27
            mana: 20000,   // @0x90 → f140 (the body's carried mana)
            ..Default::default()
        };
        let e = import_ent_mc2(&r, 26, 103, &|v| v);
        assert_eq!(e.f26, 4, "whip counter @0x10");
        assert_eq!(e.f36, 1433, "spline pitch @0x22");
        assert_eq!(e.f44, 3, "integrate mode @0x2C");
        assert_eq!(e.f50, 2, "branch index @0x3B");
        assert_eq!(e.f136, 2, "bolt power @0x88 (NOT the @0x8C lane)");
        assert_eq!(e.f140, 20000, "carried mana @0x90");
    }

    /// End-to-end owner-lane projection: the pyramid-summon
    /// discriminator. `obs_project_mc2` must recover retail parentId
    /// @0x28 for a pyramid-summoned creature (id24 → the (5,10) pyramid)
    /// WITHOUT firing on a WILD worm of the same model 0 — whose id24
    /// points at its multipart BODY, not a parent (the 261k-row
    /// over-projection trap). Also the (10,42) painter (id24 → castle)
    /// and the pyramid's own spin-angle owner (from f36). Non-vacuous:
    /// dropping the "id24 refs a (5,10)" gate makes the wild worm
    /// project its body slot (30), and dropping the (10,42) arm makes
    /// the painter project 0.
    #[test]
    fn mc2_owner_projection_pyramid_gated() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Minimal build assets (mirrors world.rs `tests::assets`): a
        // diamond search grid (needs a ring-0 cell) + flat build tab.
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);

        let put = |w: &mut World, slot: usize, class: u8, model: u8, id24: u16, f36: u16| {
            let e = &mut w.g.ent[slot];
            *e = Ent::default();
            e.class64 = class;
            e.model65 = model;
            e.id24 = id24;
            e.f36 = f36;
            e.max_life = 100;
            e.act_life = 100;
        };
        put(&mut w, 7, 5, 10, 7, 288); // pyramid: own id in id24, spin in f36
        put(&mut w, 20, 5, 0, 7, 0); // summoned m0 → id24 refs pyramid 7
        put(&mut w, 30, 5, 0, 30, 0); // wild worm body (id24 = self)
        put(&mut w, 31, 5, 0, 30, 0); // wild worm segment → id24 refs a (5,0) body
        put(&mut w, 40, 3, 2, 40, 0); // castle
        put(&mut w, 41, 10, 42, 40, 0); // painter → id24 refs castle 40

        let pin = PinnedMc2 {
            slot: 1,
            local: 0,
            player_count: 1,
            pose: PlayerPose {
                x: 0,
                y: 0,
                z: 0,
                heading: 0,
                pitch: 0,
                speed: 0,
            },
            castles: [0; 8],
        };
        let obs = w.obs_project_mc2(&pin);
        let owner_of = |slot: u16| {
            obs.entities
                .iter()
                .find(|e| e.slot == slot)
                .map(|e| e.owner)
        };
        assert_eq!(
            owner_of(20),
            Some(7),
            "pyramid-summoned m0 owner = the pyramid (id24 refs a (5,10))"
        );
        assert_eq!(
            owner_of(31),
            Some(0),
            "wild worm owner = 0 (id24 refs a (5,0) body, NOT a pyramid)"
        );
        assert_eq!(
            owner_of(30),
            Some(0),
            "wild worm body owner = 0 (id24 = self)"
        );
        assert_eq!(
            owner_of(41),
            Some(40),
            "painter owner = the referenced castle"
        );
        assert_eq!(
            owner_of(7),
            Some(288),
            "pyramid own owner = ring-spin angle from f36"
        );
    }

    /// The (5,10) DOOMSDAY PYRAMID's SUMMON-RING STRIDE lives in
    /// `word_0x4A_74` (@0x4A), not the uniform @0x30 lane: `sub_21850`
    /// stamps 682 with every creature pick (EF:13160/13173/13186) and
    /// `sub_21AB0` fans the ring at `stride * repeat + yaw`
    /// (EF:13364). Non-vacuous: the two sentinels differ, so the
    /// uniform import reads f50 = 3 and every replayed summon stacks
    /// on the pyramid's own bearing.
    #[test]
    fn mc2_pyramid_import_keeps_the_summon_stride() {
        let pyr = RetailEntMc2 {
            class3f: 5,
            model40: 10,
            f30: 3,        // @0x30 — dead for the pyramid
            sv_timer: 682, // @0x4A — the summon stride
            ..Default::default()
        };
        assert_eq!(
            import_ent_mc2(&pyr, 7, 107, &|v| v).f50,
            682,
            "summon stride @0x4A (NOT @0x30)"
        );
        let worm = RetailEntMc2 {
            class3f: 5,
            model40: 0,
            f30: 3,
            sv_timer: 682,
            ..Default::default()
        };
        assert_eq!(
            import_ent_mc2(&worm, 8, 71, &|v| v).f50,
            3,
            "every other creature keeps the uniform @0x30 home"
        );
    }

    /// The (5,22) segmented worm's three field homes (mc2l22 t=1 /
    /// t=15 / t=27 — the take's whole (5,22) x/y family): @0x2C →
    /// f44 (spin rate / orbit angle), @0x2A → f46 (the head's
    /// serpentine angle), @0x94 → f144 (the target player).
    #[test]
    fn import_mc2_worm22_spin_angle_and_target_homes() {
        let head = RetailEntMc2 {
            class3f: 5,
            model40: 22,
            f2a: 110,        // subSpellIndex_0x2A_42 — the live spiral angle
            f2c: 11,         // word_0x2C_44 — the spin rate
            b3d: 0,          // dead on the worm
            player_ent: 424, // playerEntityIndex_0x94_148
            dest_x: 0,       // @0x9A — dead on creatures
            ..Default::default()
        };
        let e = import_ent_mc2(&head, 345, 90, &|v| {
            if v == 424 { PLAYER_TARGET } else { v }
        });
        assert_eq!(e.f44, 11, "@0x2C spin rate → f44 (NOT the @0x2A angle)");
        assert_eq!(e.f46, 110, "@0x2A serpentine angle → f46 (NOT @0x3D)");
        assert_eq!(
            e.f144, PLAYER_TARGET,
            "@0x94 target → f144 (the m22 reader)"
        );
    }

    /// ⭐⭐⭐ A GHOST AT THE HEAD OF A TILE CHAIN MUST BE SPLICED OUT,
    /// NOT TRUNCATED AT — see [`chain_ghost_splice`].
    ///
    /// The recording captures a ghost (`byte[1] & 4`) still IN its
    /// tile chain, because retail only reaps it at the TOP of the next
    /// frame (`UpdateEntities_57730`, EF:39948-56 → `sub_57F20`). The
    /// rebuild refuses to link ghosts — correctly — but used to start
    /// chains only at `prev18 == 0`, so every record under a ghost head
    /// was unreachable and fell into the ascending fallback, which
    /// head-inserts in slot order and hands the chain back REVERSED.
    ///
    /// Two shapes are pinned here, because they fail differently:
    /// a ghost at the HEAD (the whole tail reverses) and a ghost in the
    /// MIDDLE (the chain splits and the two halves swap).
    ///
    /// NON-VACUITY: with `MGC_NO_CHAIN_GHOST_SPLICE=1` tile A comes
    /// back as 5 → 4 → 3 and tile B as 8 → 6, and every assert below
    /// fails.
    #[test]
    fn a_ghost_is_spliced_out_of_its_tile_chain_not_truncated_at() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);
        let pool = w.g.ent.len();

        // `flags & 4` = linked; `flags & 0x400` = retail byte[1] & 4,
        // the disable bit that makes a record a GHOST.
        let node = |flags: u32, x: u16, y: u16, next16: u16, prev18: u16| RetailEntMc2 {
            class3f: 5,
            model40: 0,
            flags,
            max_life: 100,
            life: 100,
            x,
            y,
            z: 100,
            next16,
            prev18,
            ..Default::default()
        };
        let (ax, ay) = (0x10u16 << 8, 0x14u16 << 8); // tile (16, 20)
        let (bx, by) = (0x12u16 << 8, 0x16u16 << 8); // tile (18, 22)
        let mut ents = vec![RetailEntMc2::default(); pool];
        ents[1] = RetailEntMc2 {
            class3f: 3,
            max_life: 100,
            life: 100,
            ..Default::default()
        }; // the human carpet (out-of-pool in the port)
        // Tile A — the GHOST IS THE HEAD: 2* -> 3 -> 4 -> 5.
        ents[2] = node(0x404, ax, ay, 3, 0);
        ents[3] = node(0x004, ax, ay, 4, 2);
        ents[4] = node(0x004, ax, ay, 5, 3);
        ents[5] = node(0x004, ax, ay, 0, 4);
        // Tile B — the GHOST IS IN THE MIDDLE: 6 -> 7* -> 8.
        ents[6] = node(0x004, bx, by, 7, 0);
        ents[7] = node(0x404, bx, by, 8, 6);
        ents[8] = node(0x004, bx, by, 0, 7);
        let stack: Vec<u16> = (9..pool as u16).collect();
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 1,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![mgc_formats::mgcr::RetailPlayerMc2 {
                flags: 0,
                is_ai: false,
                play_index: 1,
                turn: 0,
                castle: 0,
                cmd_speed: 0,
                strafe: 0,
                invuln: 0,
                wanted: 0,
                hand_left: -1,
                hand_right: -1,
                ..Default::default()
            }],
            ents,
            free_stack: stack,
            recycle_stack: Vec::new(),
            level: 1,
            base160: 0,
            objectives: [[0u8; 11]; 8],
            stagevars: [[0u8; 8]; 11],
            doom_beam: 0,
        };
        w.retail_import_mc2(&st).expect("import");

        let ta = crate::engine::features::tile(0x10, 0x14);
        let tb = crate::engine::features::tile(0x12, 0x16);
        // A ghost never joins a chain, on either arm.
        assert_eq!(w.g.ent[2].flags & 4, 0, "the ghost head must not link");
        assert_eq!(w.g.ent[7].flags & 4, 0, "the mid-chain ghost must not link");
        // Tile A: the ghost head is spliced out and 3 -> 4 -> 5 keeps
        // the RECORDED order (the fallback would give 5 -> 4 -> 3).
        assert_eq!(w.g.map_entity[ta] as usize, 3, "tile A head");
        assert_eq!((w.g.ent[3].next20, w.g.ent[3].prev22), (4, 0), "tile A: 3");
        assert_eq!((w.g.ent[4].next20, w.g.ent[4].prev22), (5, 3), "tile A: 4");
        assert_eq!((w.g.ent[5].next20, w.g.ent[5].prev22), (0, 4), "tile A: 5");
        // Tile B: the mid-chain ghost is spliced out and the two halves
        // stay in order (the truncating rebuild swapped them to 8 -> 6).
        assert_eq!(w.g.map_entity[tb] as usize, 6, "tile B head");
        assert_eq!((w.g.ent[6].next20, w.g.ent[6].prev22), (8, 0), "tile B: 6");
        assert_eq!((w.g.ent[8].next20, w.g.ent[8].prev22), (0, 6), "tile B: 8");
    }

    /// A FREED MC2 slot keeps the behaviour row its ctor stamped.
    ///
    /// `dword_0xA0` has exactly two writers in the shipped
    /// `NETHERW.EXE`: the 72 `c7 8? a0 00 00 00 <imm32>` entity-ctor
    /// stores (rows 59..=106; e.g. file 0x70f7f
    /// `c7 83 a0 00 00 00 86 87 00 00` = `(0x8786−0x7BD6)/34` = row
    /// 88, the firebug) and the loader fixup (Level.cpp:1255-57).
    /// The only other +0xA0 stores in the whole image — file 0xcbd06
    /// (imm 0) and the single register form at file 0xcd683
    /// (`89 83 a0 00 00 00`, fed by `8b 82 e0 00 00 00`) — address a
    /// STRIDE-4 array (`8d 04 85 00 00 00 00` with fields at
    /// +0x20/+0x60/+0xa0/+0xe0), not the 0x160-stride entity pool.
    /// And retail's free path writes ONE byte: `sub_57F20`
    /// (Events.cpp:5236-38) is file 0x7c78a `c6 43 3f 00`
    /// (`movb $0x0,0x3f(%ebx)` — the class byte) followed only by the
    /// free-stack push (`8b 50 35`, `42`, `89 50 35`,
    /// `89 9c 90 46 02 00 00`) and `c3`. So a freed record still
    /// points at its ctor's row until the slot is re-allocated, where
    /// `NewEvent_4A050` re-stamps row 59 (file 0x6e955).
    ///
    /// The importer's freed arm used to hand every class-0 record the
    /// stand-in row 59 with the note "nothing live dereferences a
    /// freed row" — a comment claiming a lane is swept. The decode is
    /// the same one the live arm runs, so this is the freed arm
    /// joining the "every OTHER byte stays" rule the stale-position
    /// law already established for the blind tracker.
    ///
    /// Non-vacuous: `MGC_NO_MC2_FREED_ROW_IMPORT=1` restores the
    /// stand-in and the first assert fails.
    /// Selective: a record whose `ptr_a0` is not a row pointer at all
    /// (a never-allocated slot's zero) still lands on 59, exactly as
    /// the live arm's `bad_rows` fallback does.
    #[test]
    fn a_freed_mc2_record_keeps_the_behaviour_row_its_ctor_stamped() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);
        let pool = w.g.ent.len();
        // `base160` is the saved `&str_D7BD6[59]`, so row R sits at
        // `base160 + 34*(R − 59)`.
        let base160: u32 = 0x000D_7BD6 + 34 * 59;
        let ptr_of = |row: u32| base160 + 34 * (row - 59);
        let mut ents = vec![RetailEntMc2::default(); pool];
        ents[1] = RetailEntMc2 {
            class3f: 3,
            max_life: 100,
            life: 100,
            ..Default::default()
        }; // the human carpet (out-of-pool in the port)
        // Slot 2: a LIVE firebug (5,19) on row 88 — the control.
        ents[2] = RetailEntMc2 {
            class3f: 5,
            model40: 19,
            max_life: 100,
            life: 100,
            x: 0x1000,
            y: 0x1400,
            z: 100,
            ptr_a0: ptr_of(88),
            ..Default::default()
        };
        // Slot 3: the SAME record after retail's free path — class
        // byte cleared, every other byte (row included) left alone.
        ents[3] = RetailEntMc2 {
            class3f: 0,
            model40: 19,
            max_life: 100,
            life: 100,
            x: 0x1000,
            y: 0x1400,
            z: 100,
            ptr_a0: ptr_of(88),
            ..Default::default()
        };
        // Slot 4: a slot that was never allocated — `ptr_a0` is 0 and
        // decodes to nothing.
        ents[4] = RetailEntMc2 {
            class3f: 0,
            ptr_a0: 0,
            ..Default::default()
        };
        let stack: Vec<u16> = (5..pool as u16).collect();
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 1,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![mgc_formats::mgcr::RetailPlayerMc2 {
                flags: 0,
                is_ai: false,
                play_index: 1,
                turn: 0,
                castle: 0,
                cmd_speed: 0,
                strafe: 0,
                invuln: 0,
                wanted: 0,
                hand_left: -1,
                hand_right: -1,
                ..Default::default()
            }],
            ents,
            free_stack: stack,
            recycle_stack: Vec::new(),
            level: 1,
            base160,
            objectives: [[0u8; 11]; 8],
            stagevars: [[0u8; 8]; 11],
            doom_beam: 0,
        };
        w.retail_import_mc2(&st).expect("import");
        assert_eq!(
            w.g.ent[3].row156, 88,
            "the freed record keeps its ctor's row (the stand-in 59 is the pre-dig arm)"
        );
        assert_eq!(w.g.ent[3].class64, 0, "and it is still a freed record");
        assert_eq!(
            w.g.ent[2].row156, 88,
            "the live control decodes to the same row (else the test is vacuous)"
        );
        assert_eq!(
            w.g.ent[4].row156, 59,
            "a never-allocated slot's zero pointer still falls back to 59"
        );
    }

    /// The import must NOT push a GHOST slot onto the free stack. The
    /// recorded stack is retail's PRE-reap image; `tick()`'s
    /// strict-MC2 top pass (UpdateEntities EF:39948-56 → `sub_57F20`,
    /// which class-zeroes and pushes) is the ONE pusher. Pushing here
    /// too double-listed every ghost, so any spawn burst deeper than
    /// the ghost count re-`NewEvent`ed a slot it had just filled:
    /// mc2l24 pair 53808 lost the doomsday pyramid's whole 17-record
    /// worm chain that way — the free list popped
    /// [905, 837, 813, 796, 727, 690] TWICE and the second pop of 905
    /// reset the chain's own live HEAD to `Ent::default()` (class 0 =
    /// invisible to the projection).
    /// ⭐⭐⭐ THE DOOMSDAY BEAM RAMP IS A CLOSURE **GLOBAL** AND HAD
    /// NO IMPORT SEAT AT ALL. `sub_21AB0` case 7 steps
    /// `D41A0_0.word_0x36546`, never the (5,10)'s own record — shipped
    /// `NETHERW.EXE`, linear (file offsets, VA = file − 0x24816):
    ///
    /// ```text
    ///   465cd  f6 43 2a 02                 testb $0x2,0x2a(%ebx)   ; subSpellIndex & 2
    ///   465de  a1 a0 41 00 00              mov   0x41a0,%eax       ; D41A0_0
    ///   465e3  66 c7 80 46 65 03 00 00 04  movw  $0x400,0x36546(%eax)
    ///   46605  66 8b b0 46 65 03 00        mov   0x36546(%eax),%si
    ///   4660c  83 ee 50                    sub   $0x50,%esi        ; −80
    ///   4660f  66 89 b0 46 65 03 00        mov   %si,0x36546(%eax)
    ///   46616  66 83 fe 0a / 7d 09         cmp   $0xa,%si / jge    ; floor 10
    ///   4662a  66 81 b8 46 65 03 00 00 04  cmpw  $0x400,0x36546(%eax)
    ///   4666c  0f bf 80 46 65 03 00        movswl 0x36546(%eax),%eax ; the DISTANCE
    ///   46682  e8 19 61 03 00              call  MoveEntity_57FA0
    /// ```
    ///
    /// The port homes that word on the (5,10)'s `f52`, and the entity
    /// table seats `f52` from @0x32 — a lane retail leaves 0 on the
    /// pyramid for the level's whole life. So every imported pair used
    /// to hand the beam a ramp of 0, which `(f52 − 80).clamp(10, 1024)`
    /// floors to 10 on the spot: retail's 944, 864, …, 64 read back as
    /// 10 on all twenty-four ticks but the first.
    ///
    /// Non-vacuous: `MGC_NO_MC2_DOOM_BEAM_IMPORT=1` (the pre-dig seat)
    /// leaves `f52` at @0x32's 0 and the first assert fails.
    /// Selective: a non-pyramid class-5 record keeps its @0x32.
    #[test]
    fn mc2_doomsday_beam_ramp_is_seated_from_the_closure_global() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);
        let pool = w.g.ent.len();

        // The mc2l24 boss (slot 5, 300k life) and a plain class-5
        // creature that must KEEP its @0x32 pack-leader word.
        let mut ents = vec![RetailEntMc2::default(); pool];
        ents[1] = RetailEntMc2 {
            class3f: 3,
            model40: 0,
            max_life: 10_000,
            life: 10_000,
            ..Default::default()
        };
        ents[5] = RetailEntMc2 {
            class3f: 5,
            model40: 10,
            max_life: 300_000,
            life: 299_700,
            b43: 7,
            f24: 22,
            f32: 0,
            ..Default::default()
        };
        ents[6] = RetailEntMc2 {
            class3f: 5,
            model40: 24,
            max_life: 100,
            life: 100,
            f32: 44,
            ..Default::default()
        };
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 1,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![mgc_formats::mgcr::RetailPlayerMc2 {
                play_index: 1,
                hand_left: -1,
                hand_right: -1,
                ..Default::default()
            }],
            ents,
            free_stack: (7..pool as u16).collect(),
            recycle_stack: Vec::new(),
            level: 24,
            base160: 0,
            objectives: [[0u8; 11]; 8],
            stagevars: [[0u8; 8]; 11],
            // mc2l24 t=44656: the third beam tick of the 44654..44677
            // burst, ramp 784 (1024 − 80·3).
            doom_beam: 784,
        };
        w.retail_import_mc2(&st).expect("import");
        assert_eq!(
            w.g.ent[5].f52, 784,
            "the (5,10)'s beam ramp is the closure GLOBAL, not its @0x32"
        );
        assert_eq!(
            w.g.ent[6].f52, 44,
            "…and every other class-5 record keeps @0x32 verbatim"
        );
    }

    /// Non-vacuous: restoring the `free.extend(ghost_slots)` appends
    /// slot 3 and both asserts below fail.
    #[test]
    fn mc2_import_leaves_the_ghost_free_push_to_the_tick_reap() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);
        let pool = w.g.ent.len();

        let live = |class3f: u8, flags: u32| RetailEntMc2 {
            class3f,
            model40: 0,
            flags,
            max_life: 100,
            life: 100,
            ..Default::default()
        };
        let mut ents = vec![RetailEntMc2::default(); pool];
        ents[1] = live(3, 0); // the human carpet (the reserved hole)
        ents[2] = live(5, 0); // one live creature
        ents[3] = live(5, 0x400); // one GHOST (retail byte[1] & 4)
        // Retail's stack at capture: every genuinely free slot, ghost
        // NOT among them (retail pushes it at the next frame's top).
        let stack: Vec<u16> = (4..pool as u16).collect();
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 1,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![mgc_formats::mgcr::RetailPlayerMc2 {
                flags: 0,
                is_ai: false,
                play_index: 1,
                turn: 0,
                castle: 0,
                cmd_speed: 0,
                strafe: 0,
                invuln: 0,
                wanted: 0,
                hand_left: -1,
                hand_right: -1,
                ..Default::default()
            }],
            ents,
            free_stack: stack.clone(),
            recycle_stack: Vec::new(),
            level: 1,
            base160: 0,
            objectives: [[0u8; 11]; 8],
            stagevars: [[0u8; 8]; 11],
            doom_beam: 0,
        };
        let report = w.retail_import_mc2(&st).expect("import");
        assert_eq!(
            report.stack_fallback, None,
            "the census must accept the recorded stack (else the test is vacuous)"
        );
        assert_eq!(
            w.g.free, stack,
            "the imported free list IS the recorded stack, verbatim"
        );
        assert!(
            !w.g.free.contains(&3),
            "the ghost's push belongs to tick()'s top reap, not the import"
        );
    }

    /// A freed MC1 slot is not an EMPTY slot: retail's free path
    /// clears +64 and pushes the stack — every OTHER byte stays, and
    /// the blind tracker (`sub_52550`, ledger §THE PROJECTILE
    /// LEDGER + BLIND TRACKER) steers at whatever the record still
    /// holds.
    /// mc1l0 t=3464-70: bolt 557 tracked reaped slot 534's stale
    /// position (pitch pinned level by the corpse's raw-2048 bearing);
    /// the old `Ent::default()` import re-aimed it at the ORIGIN and
    /// the bolt's whole heading/pitch/aim column diverged. The import
    /// must carry the stale bytes through — class 0, unlinked, not
    /// counted active, still on the free stack.
    /// Non-vacuous: restoring the default arm zeroes the position and
    /// the stale-byte asserts fail.
    #[test]
    fn mc1_import_keeps_a_freed_slots_stale_bytes_for_the_blind_tracker() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc1);
        let pool = w.g.ent.len();

        let mut ents = vec![RetailEntMc1::default(); pool];
        // Slot 1: the human carpet (row-7 model_ptr anchors the base).
        ents[1] = RetailEntMc1 {
            class64: 3,
            model65: 0,
            model_ptr: 7 * 32,
            x: 100 << 8,
            y: 100 << 8,
            ..Default::default()
        };
        // Slot 2: a reaped corpse — class 0, every stale byte intact
        // (the mc1l0 slot-534 shape, model_ptr stale/dangling too).
        ents[2] = RetailEntMc1 {
            class64: 0,
            model65: 1,
            act_life: -400,
            flags: 0x408,
            f58: 7,
            f78: 50,
            x: 53174,
            y: 17486,
            z: 1101,
            model_ptr: 0xDEAD_BEEF,
            ..Default::default()
        };
        // Slot 3: a live bolt still chasing the freed slot.
        ents[3] = RetailEntMc1 {
            class64: 9,
            model65: 0,
            flags: 0x2006,
            act_life: 7,
            x: 57998,
            y: 15993,
            z: 1146,
            f146: 2,
            model_ptr: 0,
            ..Default::default()
        };
        // Retail's stack: every class-0 slot (the fresh corpse rides
        // it — its reap already pushed), human hole and the bolt out.
        let stack: Vec<u16> = std::iter::once(2u16).chain(4..pool as u16).collect();
        let st = RetailMc1 {
            rand: 1,
            local_player: 0,
            player_count: 1,
            spawn_count: [0; 20],
            wizards: {
                let mut ws = vec![RetailWizardMc1::default(); 8];
                ws[0] = RetailWizardMc1 {
                    play_index: 1,
                    hand_left: 0xFFFF,
                    hand_right: 0xFFFF,
                    ..Default::default()
                };
                ws
            },
            ents,
            free_stack: stack.clone(),
            recycle_stack: Vec::new(),
            level: 0,
            erupting: 0,
            plume: 0,
        };
        let report = w.retail_import_mc1(&st).expect("import");
        assert_eq!(report.active, 1, "only the bolt counts active");
        assert_eq!(
            report.bad_rows, 0,
            "a freed slot's dangling model_ptr is not a bad row"
        );
        let corpse = &w.g.ent[2];
        assert_eq!(corpse.class64, 0, "freed stays freed");
        assert_eq!(
            (corpse.x, corpse.y, corpse.z),
            (53174, 17486, 1101),
            "the stale position survives the import — the blind \
             tracker's whole aim"
        );
        assert_eq!(corpse.model65, 1);
        assert_eq!(corpse.f78, 50, "aim_z's stale +78 lift survives too");
        assert_eq!(corpse.flags & 4, 0, "never linked into a tile chain");
        assert_eq!(
            report.stack_fallback, None,
            "the freed slot still counts as free for the stack census"
        );
        assert!(w.g.free.contains(&2), "and rides the recorded stack");
        assert_eq!(w.g.ent[3].f146, 2, "the bolt still chases the slot");
    }

    /// The +132 seed clamp, BOTH halves (sub_55E80's live half,
    /// :64956-59: `if (v2 && +132 > 0) +132 = 0`):
    ///  • retail only ever zeroes a POSITIVE delta — a negative seed
    ///    (a pending debit) rides through mid-burst untouched
    ///    (mc1l48 t=27415-53: 39 firings with f132 = −50 the old
    ///    unconditional zero ate);
    ///  • the exempt test keys on the SPELL ID alone (f70/3, phases
    ///    alike): a phase-1 token of a live spell (16 → f70=49) is
    ///    still exempt — the old `f70 % 3 == 0` conjunct carved it
    ///    out at phase 0 but not phase 1 (half of mc1l48's 204
    ///    firings).
    /// Non-vacuous: restoring the unconditional zero fails the first
    /// import; restoring the %3 conjunct fails the third.
    #[test]
    fn mc1_import_seed_clamp_spares_negative_deltas_and_keys_on_spell_id() {
        let import = |f132: i32, f70: u8| {
            let planes = Planes {
                height: vec![100; 0x10000],
                tile_type: vec![5; 0x10000],
                shading: vec![32; 0x10000],
                angle: vec![5; 0x10000],
                ceiling: Vec::new(),
            };
            let mut grid = vec![31u8; 1024];
            for y in 0..32i32 {
                for x in 0..32i32 {
                    let (dx, dy) = (x - 15, y - 15);
                    let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                    grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
                }
            }
            let tab: Vec<u8> = (0..24u32)
                .flat_map(|_| {
                    let mut e = 0u32.to_le_bytes().to_vec();
                    e.extend_from_slice(&[4, 4]);
                    e
                })
                .collect();
            let mut dat = Vec::new();
            for _ in 0..4 {
                dat.push(4u8);
                dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
                dat.push(0);
            }
            let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
            let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc1);
            let pool = w.g.ent.len();
            let mut ents = vec![RetailEntMc1::default(); pool];
            ents[1] = RetailEntMc1 {
                class64: 3,
                model65: 0,
                model_ptr: 7 * 32,
                x: 100 << 8,
                y: 100 << 8,
                f132,
                ..Default::default()
            };
            // Slot 2: the wizard's own MID-burst token (+48 live and
            // != +50), owner register f144 = 0 like every token.
            ents[2] = RetailEntMc1 {
                class64: 12,
                model65: 2,
                f48: 100,
                f50: 251,
                f70,
                f144: 0,
                model_ptr: 0,
                ..Default::default()
            };
            let stack: Vec<u16> = (3..pool as u16).collect();
            let st = RetailMc1 {
                rand: 1,
                local_player: 0,
                player_count: 1,
                spawn_count: [0; 20],
                wizards: {
                    let mut ws = vec![RetailWizardMc1::default(); 8];
                    ws[0] = RetailWizardMc1 {
                        play_index: 1,
                        hand_left: 0xFFFF,
                        hand_right: 0xFFFF,
                        owned_slots: {
                            let mut o = [0u16; 24];
                            o[(f70 / 3) as usize] = 2;
                            o
                        },
                        ..Default::default()
                    };
                    ws
                },
                ents,
                free_stack: stack,
                recycle_stack: Vec::new(),
                level: 0,
                erupting: 0,
                plume: 0,
            };
            w.retail_import_mc1(&st).expect("import");
            w.player.mana_delta
        };
        // Spell 5 (f70=15) is NOT on the keep-list: its mid-burst
        // token clamps a positive seed…
        assert_eq!(import(100, 15), 0, "positive delta + live token → 0");
        // …but a NEGATIVE seed rides through the same token.
        assert_eq!(import(-50, 15), -50, "negative delta is never zeroed");
        // Spell 16's PHASE-1 token (f70=49) is exempt by spell id.
        assert_eq!(
            import(100, 49),
            100,
            "exempt spell keeps the seed at phase 1"
        );
    }

    /// A FULL MC2 pool still spawns: `NewEvent_4A050` (:581) falls
    /// through to the recycle stack and SACRIFICES the top-ranked live
    /// victim — bare seizure, no death. The import must carry the
    /// recorded ranking verbatim so replay sacrifices retail's victims
    /// in retail's order, and must stop where retail's list stops
    /// (`refill` off: the snapshot IS the law under strict replay).
    /// Non-vacuous: clearing `w.g.mc2_recycle.stack` — the pre-dig
    /// port, i.e. `MGC_NO_RECYCLE_VICTIM=1` — makes the very first
    /// `new_event` return None instead of slot 300.
    #[test]
    fn mc2_full_pool_sacrifices_the_recorded_recycle_victims_in_order() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let tab: Vec<u8> = (0..24u32)
            .flat_map(|_| {
                let mut e = 0u32.to_le_bytes().to_vec();
                e.extend_from_slice(&[4, 4]);
                e
            })
            .collect();
        let mut dat = Vec::new();
        for _ in 0..4 {
            dat.push(4u8);
            dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            dat.push(0);
        }
        let fa = crate::engine::features::FeatureAssets::parse(&grid, &tab, &dat).unwrap();
        let mut w = World::new_for_game(planes, &[], 1, fa, crate::ids::GameId::Mc2);
        let pool = w.g.ent.len();

        // Every slot occupied: the free stack is EMPTY, exactly the 74
        // mc2l24 snapshots that motivated the arm.
        let mut ents = vec![RetailEntMc2::default(); pool];
        for (s, e) in ents.iter_mut().enumerate().skip(1) {
            *e = RetailEntMc2 {
                class3f: if s == 1 { 3 } else { 5 },
                model40: 0,
                flags: 0,
                max_life: 100,
                life: 100,
                ..Default::default()
            };
        }
        // Retail's ranking, bottom-up: 300 pops first, then 500, 700.
        let victims: Vec<u16> = vec![700, 500, 300];
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 1,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![mgc_formats::mgcr::RetailPlayerMc2 {
                flags: 0,
                is_ai: false,
                play_index: 1,
                turn: 0,
                castle: 0,
                cmd_speed: 0,
                strafe: 0,
                invuln: 0,
                wanted: 0,
                hand_left: -1,
                hand_right: -1,
                ..Default::default()
            }],
            ents,
            free_stack: Vec::new(),
            recycle_stack: victims.clone(),
            level: 1,
            base160: 0,
            objectives: [[0u8; 11]; 8],
            stagevars: [[0u8; 8]; 11],
            doom_beam: 0,
        };
        let report = w.retail_import_mc2(&st).expect("import");
        assert_eq!(
            report.stack_fallback, None,
            "the census must accept the empty free stack (else the test is vacuous)"
        );
        assert!(w.g.free.is_empty(), "a full pool has no free slot");
        assert_eq!(
            w.g.mc2_recycle.stack, victims,
            "the recorded ranking rides across verbatim"
        );

        // Seizure order = retail's pop order, and the seized record is
        // a fresh `NewEvent` (id24 = own slot, maxLife 300), NOT a
        // corpse: the victim never reaches the free stack.
        assert_eq!(w.g.new_event(), Some(300), "the stack TOP is sacrificed");
        assert_eq!(
            w.g.ent[300].id24, 300,
            "the seized slot was re-`NewEvent`ed"
        );
        assert_eq!(w.g.ent[300].max_life, 300, "…with the allocator defaults");
        assert!(
            w.g.free.is_empty(),
            "a sacrifice is not a death — the slot skips the free stack"
        );
        assert_eq!(w.g.new_event(), Some(500), "then the next-ranked victim");
        assert_eq!(w.g.new_event(), Some(700), "then the last");
        assert_eq!(
            w.g.new_event(),
            None,
            "retail's list ran out, so the port's does too (refill is off \
             under the strict import)"
        );
        assert_eq!(w.take_recycle_seized(), 3, "three victims, counted");

        // A victim that dies normally must LEAVE the stack
        // (`sub_57F20` :5215-34), or the allocator would hand its slot
        // out twice — once from the free stack, once as a sacrifice.
        w.g.mc2_recycle.stack = vec![700, 500, 300];
        w.g.ent[500].flags |= 0x2_0000;
        w.g.free_entity(500);
        assert_eq!(
            w.g.mc2_recycle.stack,
            vec![700, 300],
            "retail's removal swaps the TOP into the hole"
        );
        assert_eq!(w.g.free, vec![500], "the dying victim went free, once");
    }

    /// Minimal MC2 closure for the delta reconstruction: one carpet
    /// and one book manifestation, both of which the caller shapes.
    fn burst_closure(
        carpet_slot: u16,
        carpet: RetailEntMc2,
        tok_slot: u16,
        tok: RetailEntMc2,
    ) -> (RetailMc2, mgc_formats::mgcr::RetailPlayerMc2) {
        let mut ents = vec![RetailEntMc2::default(); 300];
        ents[carpet_slot as usize] = carpet;
        ents[tok_slot as usize] = tok;
        let mut ply = mgc_formats::mgcr::RetailPlayerMc2 {
            play_index: carpet_slot,
            ..Default::default()
        };
        ply.spell_ent[tok.model40 as usize] = tok_slot;
        let st = RetailMc2 {
            things: vec![],
            stage_binds: [(0, 0, None); 8],
            rand: 0,
            vortex: 0,
            fire_col: 0,
            local_player: 0,
            player_count: 1,
            spawn_ord: [0; 29],
            players: vec![ply],
            ents,
            free_stack: Vec::new(),
            recycle_stack: Vec::new(),
            level: 3,
            base160: 0,
            objectives: [[0; 11]; 8],
            stagevars: [[0; 8]; 11],
            doom_beam: 0,
        };
        (st, ply)
    }

    /// **THE RECORDED `manaRegen` IS ONLY THE APPLIED ONE WHEN THE
    /// MANIFESTATION SITS ABOVE THE CARPET.** See
    /// [`mc2_applied_mana_delta`] for the law; this pins every arm of
    /// it with the mc2l3 take-2 numbers that measured it.
    ///
    /// Non-vacuous: returning `carpet.d88` unconditionally (the
    /// pre-dig import, still reachable as `MGC_NO_MC2_BURST_DELTA=1`)
    /// fails four of the six asserts — the two pins, the end-sequence
    /// freeze and the first-tick wipe.
    #[test]
    fn mc2_burst_delta_is_the_applied_word_not_the_recorded_one() {
        // The carpet as recorded mid-cast: the regen recompute (100
        // afield / 1000 at the castle) is what the frame tail holds.
        // `mana` is the purse `sub_68D50`'s arm-tick leg re-checks
        // (mc2l3 t=8445's 41,359, which covers the 40,000 castle).
        let carpet = |d88: i32, action45: u8| RetailEntMc2 {
            class3f: 3,
            model40: 0,
            action45,
            d88,
            mana: 41_359,
            ..Default::default()
        };
        // A live manifestation: @0x2E armed timer, @0x30 duration,
        // @0x8C full cost, action 3M (the owned state).
        let tok = |spell: u8, f2e: i16, f30: u16, cost: i32| RetailEntMc2 {
            class3f: 15,
            model40: spell,
            action45: spell * 3,
            f2e,
            f30,
            mana_max: cost,
            ..Default::default()
        };

        // BELOW the carpet, mid-burst → the pin (mc2l3 t=9034-9035:
        // recorded 100, mana FLAT).
        let (st, ply) = burst_closure(167, carpet(100, 0), 109, tok(1, 2, 3, 100));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 167, &st.ents[167], [0, 0, 1]),
            0
        );

        // BELOW the carpet, FIRST tick → the recompute is wiped; the
        // debit itself is left to `World::mc2_same_frame_debit`, which
        // is what keeps the afford gate reading the pre-debit purse
        // (mc2l3 t=8445, the 40,000 Create Castle out of 41,359).
        let (st, ply) = burst_closure(167, carpet(1000, 0), 114, tok(2, 101, 101, 40000));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 167, &st.ents[167], [0, 0, 1]),
            0
        );

        // BELOW the carpet, the CASTLE's upgrade LOCK (timer parked,
        // not counting) → no pin at all: mc2l3 t=8446+ climbs +1000.
        let (st, ply) = burst_closure(167, carpet(1000, 0), 114, tok(2, 100, 101, 40000));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 167, &st.ents[167], [0, 0, 1]),
            1000
        );

        // ABOVE the carpet → the record already holds the token's own
        // stamp; it is applied verbatim (mc2l24 slot 118 vs carpet
        // 116, recorded −100 then 0).
        let (st, ply) = burst_closure(116, carpet(-100, 0), 118, tok(1, 2, 3, 100));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            -100
        );

        // A DETACHED jar (the wraith steal's action 78) never reaches
        // `sub_68DE0`, so it pins nothing.
        let mut stolen = tok(1, 2, 3, 100);
        stolen.action45 = 78;
        let (st, ply) = burst_closure(167, carpet(100, 0), 109, stolen);
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 167, &st.ents[167], [0, 0, 1]),
            100
        );

        // Action 12, the level-end sequence: the regen block is in the
        // action-0 body alone, so NOTHING is applied (mc2l3 t=22621+).
        let (st, ply) = burst_closure(167, carpet(100, 12), 109, tok(1, 0, 3, 100));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 167, &st.ents[167], [0, 0, 1]),
            0
        );
    }

    /// ⭐⭐⭐ **A REFUSED TOKEN TICK NEVER REACHES `sub_68DE0`, AND
    /// SHIELD III DECREMENTS BEFORE IT CALLS — ON THE IMPORT PATH
    /// TOO.** Both gates were live-pass-only (`World::mc2_afford`'s
    /// `afford` flag and `shield3_predecrement` in
    /// `World::mc2_manifestation_tick`); the conformance import ran
    /// neither, and that is the WHOLE residue of mc2l24's
    /// `player.mana` / `(3,0) mana` capture rules — 5 pairs of 54,057.
    ///
    /// Non-vacuous: `MGC_NO_MC2_IMPORT_TOKEN_GATE=1` fails the first
    /// and the third assert (so does `MGC_NO_MC2_SHIELD3_PREDECREMENT=1`
    /// for the third alone).
    #[test]
    fn a_refused_token_tick_and_shield_iii_leave_the_wizard_regen_alone() {
        let carpet = |d88: i32, mana: i32| RetailEntMc2 {
            class3f: 3,
            model40: 0,
            action45: 0,
            d88,
            mana,
            ..Default::default()
        };
        let tok = |spell: u8, f2e: i16, f30: u16, cost: i32| RetailEntMc2 {
            class3f: 15,
            model40: spell,
            action45: spell * 3,
            f2e,
            f30,
            mana_max: cost,
            ..Default::default()
        };

        // mc2l24 t=7284: spell 0's token re-arms at 5/5 with 64 in the
        // purse against a 100 cost. `sub_68D50` refuses, the handler
        // collapses the window instead of calling `sub_68DE0`, and the
        // recomputed +100 is applied in full.
        let (st, ply) = burst_closure(116, carpet(100, 64), 6, tok(0, 5, 5, 100));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            100
        );
        // The same token one tick earlier, affordable: the first-tick
        // wipe stands (the port's own pass lands the debit).
        let (st, ply) = burst_closure(116, carpet(100, 292), 6, tok(0, 5, 5, 100));
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            0
        );

        // mc2l24 t=40238: Shield III (spell 6, tier `life_0x1A == 1`)
        // on the LAST tick of a 301-tick window — `@0x2E` 1 → 0 before
        // the call, so `sub_68DE0` sees 0 and pins nothing.
        let mut t3 = tok(6, 1, 301, 0);
        t3.b46 = 2;
        let (st, ply) = burst_closure(116, carpet(345, 645_325), 40, t3);
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            345
        );
        // …and one tick earlier it still pins (`@0x2E - 1 == 1`).
        let mut t3 = tok(6, 2, 301, 0);
        t3.b46 = 2;
        let (st, ply) = burst_closure(116, carpet(345, 645_325), 40, t3);
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            0
        );
        // Shield I/II (`life_0x1A == 0`) keep the ordinary order: the
        // call comes FIRST, so the last tick still pins.
        let mut t1 = tok(6, 1, 101, 0);
        t1.b46 = 0;
        let (st, ply) = burst_closure(116, carpet(345, 645_325), 40, t1);
        assert_eq!(
            mc2_applied_mana_delta(&st, &ply, 116, &st.ents[116], [0, 0, 1]),
            0
        );
    }
}
