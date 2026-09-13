//! MC2 class-10 TAIL EFFECTS — the small-count effect
//! band: (10,52) castle anchor, (10,25)/(10,23) one-shot blasts,
//! (10,17) meteor, (10,15) fire trail + its (10,11→19) ground-fire
//! spray, (10,54) proximity aura. Trace bank:
//! docs/traces/mc2-class10-m50-chains-and-tail.md (§3-§7) +
//! mc2-class10-m6-m9-m11-m28-m31.md (§3, the 11→19 remap)
//! (`EF:` = remc2 EventsFunctions.cpp).
//!
//! Entity-field homes follow the class-10 effect column: subSpell
//! (the area amount) → f140, `dword_0x10_16` scratch → f26,
//! `byte_0x46_70` → f71, `word_0x26_38` → f40.
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - `sub_6D8B0(id, kind, hits)` spellbook reports ((10,17) kind 9,
//!   (10,23) kind 7, (10,15)'s spray kind — the spell-XP intake):
//!   the hit counts are computed and dropped.
//! - The (10,19) spray's `word_0x33` singleton latch IS ported: the
//!   summit-18 eruption registers each new column and kills the
//!   previous (`plume`, morph.rs `mc2_summit18_tick` — EF:23962-64),
//!   and the spray's death releases it (`mc2_fire_spray_tick`,
//!   EF:24148). The old "no ported writer" note here was stale.
//! - `AddEvent2_847D0` attached lights/children ((10,23)'s
//!   (128,9,0)) are presentation, unported (the (10,1) note).
//! - The (10,54) aura scans retail's `dword_38523` creature list —
//!   our pool slot-order scan over the mobs.rs list stands in.

use super::behavior::BEHAVIOR;
use super::sprite_params::SPRITE_PARAMS;
use crate::engine::features::Gen;
use crate::mc1::combat::MailTarget;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// The whirlwind's victim GRAB latch (retail byte[3] & 0x10, dword
/// 0x1000_0000) — a free high bit next to the mobs.rs MC2 band.
// NB: NOT 1 << 29 — that is [`super::proj::F_MC2PROJ`]'s bit, and the
// whirlwind teardown clears this flag over a radius-12 disc on EVERY
// entity class (tail of `sub_338D0`); reusing 1 << 29 would strip the
// MC2-column marker off any projectile caught in the sweep, dropping
// it to the MC1 handler with an MC2 behavior row.
pub(crate) const F_GRABBED: u32 = 1 << 22;

/// A/B toggle for the mana aura's PULL STAMP: set
/// `MGC_NO_MC2_AURA_STAMP` to restore the pre-dig behaviour, where
/// `sub_38D80` wrote the pulled sphere's VELOCITY (`axis_0x9A_154x`)
/// itself, at the AURA's own pool slot.
///
/// ⭐⭐⭐ THE AURA STAMPS A SPEED AND ITS OWN INDEX, AND NOTHING ELSE.
/// `sub_38D80` (EF:28353-83) is four statements inside the range
/// test, and the shipped `NETHERW.EXE` 0x5d5bd-0x5d610 is the whole
/// body of it:
///
/// ```text
///   5d5bd: cmpw $0x0,0x7a(%ebx)     ; if (sphere->@0x7A) skip
///   5d5cc: call 0x7ccd0             ; EuclideanDistXY_584D0
///   5d5d7: cmp  %edx,%eax           ; vs aura->@0x10 (range squared)
///   5d5dc: call 0x96f7a             ; radix_3d  (isqrt)
///   5d5e6: cmp  $0x2a,%eax ; 5d5eb: mov $0x2a,%ecx     ; cap 42
///   5d60a: mov  %ecx,0x76(%ebx)     ; sphere->@0x76 = speed (dword)
///   5d60d: mov  %ax,0x7a(%ebx)      ; sphere->@0x7A = aura index
/// ```
///
/// There is NO write to `0x9a` anywhere in the function. The velocity
/// is derived by the SPHERE, in the sphere's OWN tick, at the head of
/// `TransformArcherToMana_35940` (EF:26097-26110):
///
/// ```c
///   if (a1x->str_0x5E_94.word_0x7A_122) {
///       v35 = 1;
///       a1x->yaw_0x1C_28 = sub_581E0_maybe_tan2(&a1x->position, &Entities[w7A]->position);
///       predictedAxis = {0,0,0};
///       MoveEntity_57FA0(&predictedAxis, a1x->yaw_0x1C_28, 0, a1x->str_0x5E_94.word_0x76_118);
///       a1x->axis_0x9A_154x.x = predictedAxis.x;
///       a1x->axis_0x9A_154x.y = predictedAxis.y;
///       a1x->str_0x5E_94.word_0x7A_122 = 0;
///   }
/// ```
///
/// — and `word_0x76_118`/`word_0x7A_122` are exactly the port's
/// `mail[4]` amount/source pair, which [`Gen::ball_tick`]'s ch4 intake
/// already services verbatim for the IMPORTED half of the same cell.
/// So the whole fix is: stamp the pair, let the sphere do the polar
/// step.
///
/// ⭐⭐⭐ WHY IT IS A ONE-RAW-UNIT FAMILY AND NOT A GROSS ONE. The
/// aura's slot is almost always ABOVE the sphere's, so the port's
/// write landed AFTER the sphere had already ticked — the sphere then
/// flew on a bearing measured from where it stood BEFORE the next
/// tick's movers touched it. On a sphere whose only other writer is
/// its own dest the two agree; on one ALSO being dragged by another
/// pass in the same tick they do not. mc2l1 slot 170 is that sphere:
/// the (10,22) whirlwind at slot 31 swirls it 96 units every tick
/// (`sub_33340`'s mid ring) before slot 170 runs, so retail's bearing
/// is taken from the POST-swirl position and the port's from the
/// pre-swirl one. One raw unit of `y`, thirteen times, and the sign
/// FLIPS with the swirl — the take's whole divergence set.
fn no_mc2_aura_stamp() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_AURA_STAMP").is_some())
}

/// A/B toggle for the whirlwind lift pass's billing protocol: set
/// `MGC_NO_WHIRLWIND_SINGLE_BILL` to restore the pre-dig behaviour,
/// where `sub_33340`'s `sub_11900` call used the AREA protocol
/// ([`Gen::mail_write`]) instead of the SINGLE/INVERSE one.
fn no_whirlwind_single_bill() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_WHIRLWIND_SINGLE_BILL").is_some())
}

/// A/B toggle for the whirlwind's MID-RING arm on the human wizard:
/// set `MGC_NO_MC2_WW_MIDRING` to restore the pre-dig behaviour,
/// where the funnel drove the human through the KNOCK register
/// (`Gen::player_knock` = retail `moveBoost_0x1E_30`) — a lane
/// `sub_33340` never writes — instead of the direct pose seizure.
fn no_mc2_ww_midring() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_MIDRING").is_some())
}

/// A/B toggle for the lift pass's DISC WALK: set
/// `MGC_NO_MC2_WW_WALK` to restore the pre-dig traversal, which
/// captured the tile link BEFORE the victim body ran instead of
/// re-reading it after `CopyEntityPosition_57CF0` relinked the
/// victim (shipped `NETHERW.EXE` 0x57ea3 `mov bx,[ebx+0x16]`, the
/// loop's increment, is AFTER the call at 0x57e7b).
/// A/B toggle for the AURA's `w7A` GUARD: set
/// A/B kill-switch for THE FIRE-ORB SATELLITE'S CLONED `word_0x2C_44`:
/// set `MGC_NO_MC2_ORB_SATELLITE_F44` to restore the pre-dig
/// behaviour, where each (10,77) satellite kept `new_event`'s default
/// `word_0x2C_44 = 100` instead of the hub's 0. See the clone block in
/// [`Gen::mc2_spawn_fire_orb`] for the citations.
fn no_mc2_orb_satellite_f44() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ORB_SATELLITE_F44").is_some())
}

/// `MGC_NO_AURA_CLAIM_W7A_GUARD` to restore the pre-dig behaviour,
/// where `sub_38D80`'s `if (!word_0x7A_122)` (EF:28364) was modelled
/// against the claim map alone and missed the `mail[4]` half of the
/// same retail cell.
pub(crate) fn aura_claim_w7a_guard_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_AURA_CLAIM_W7A_GUARD").is_none())
}

fn no_mc2_ww_walk() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_WALK").is_some())
}

/// A/B toggle for the lift pass's REAP SKIP: set
/// `MGC_MC2_WW_REAP_SKIP` to restore the pre-dig invented guard,
/// which dropped a victim whose `0x400` had already gone up this
/// tick. `sub_33340`'s victim loop is `if (sub_33810(a1x, ix))` and
/// nothing else (EF:24286), and `sub_33810` (EF:24452-515) never
/// reads the flag word — see `mc2_whirlwind_lift`.
fn mc2_ww_reap_skip() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_MC2_WW_REAP_SKIP").is_some())
}

/// A/B toggle for the whirlwind's GRAB FAMILY on the human wizard
/// (dig Q7): set `MGC_NO_MC2_WW_HUMAN_GRAB` to restore the pre-dig
/// behaviour, where `sub_33340`'s inner LIFT arm, its near-grab and
/// far-grab arms and the `byte[3] |= 0x10` latch simply did not
/// exist for the out-of-pool human — the funnel could sway him on
/// the mid ring (dig W4) and otherwise shoved him through the
/// invented `Gen::player_knock` spiral registered at
/// `docs/DEVIATIONS.md` ("the full grab/lift/camera-roll takeover is
/// the deferred FlightVerb seam").
pub(crate) fn no_mc2_ww_human_grab() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_HUMAN_GRAB").is_some())
}

/// A/B toggle for `sub_33340`'s UNCONDITIONAL victim tail on the human
/// wizard: set `MGC_NO_MC2_WW_TAIL_BAND` to restore the pre-dig
/// behaviour, where the cave-ceiling clamp and `sub_580E0` float band
/// (EF:24382-94) ran on the GRAB arms only. Retail runs
/// `MoveEntity_57FA0` / the clamp / `sub_580E0` / `CopyEntityPosition`
/// on EVERY visit that survives `sub_33810`, the mid ring included —
/// the four arms all fall through to the same tail.
fn no_mc2_ww_tail_band() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_TAIL_BAND").is_some())
}

/// A/B toggle for the MID RING's publish protocol: set
/// `MGC_NO_MC2_WW_MIDRING_ABS` to restore the pre-dig behaviour, where
/// a mid-ring visit published a heading + one 96-unit step
/// ([`crate::engine::features::PlayerWhirl::grab`] = `None`) — a
/// channel that can carry exactly ONE visit, so retail's second
/// mid-ring visit of the same tick (the ring walk re-finds a victim
/// `CopyEntityPosition_57CF0` carried into a cell it has not reached
/// yet) was silently dropped, along with the z the tail band writes.
fn no_mc2_ww_midring_abs() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_MIDRING_ABS").is_some())
}

/// A/B toggle for the HUMAN'S RELINK CONTINUATION in the whirlwind
/// lift walk: set `MGC_NO_MC2_WW_HUMAN_RELINK_WALK` to restore the old
/// behaviour, where the human's per-cell visit ended the cell instead
/// of handing the walker the chain of the tile he was just relinked
/// into. See the law note at the call site.
fn no_mc2_ww_human_relink_walk() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_HUMAN_RELINK_WALK").is_some())
}

/// A/B toggle for `sub_33340`'s FAR-BAND VISIT on the human wizard:
/// set `MGC_NO_MC2_WW_TAIL_PUBLISH` to restore the pre-dig behaviour,
/// where a visit that seized nothing published nothing — so the
/// unconditional tail's `sub_580E0` float band was computed and thrown
/// away. Retail's far arm on an UNGRABBED victim is `testb $0x10,0xf(%ebx)`
/// / `je 0x57df8` (`NETHERW.EXE` 0x57db4/0x57db8): straight into the
/// tail, where `MoveEntity_57FA0` is a no-op (`v30 = 0`) and the BAND
/// plus `CopyEntityPosition_57CF0` are the entire point of the visit.
fn no_mc2_ww_tail_publish() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_TAIL_PUBLISH").is_some())
}

/// A/B toggle for the human's RANK inside the whirlwind's per-cell
/// chain walk: set `MGC_NO_MC2_WW_HUMAN_CHAIN_ORDER` to restore the
/// pre-dig placement, where his arm ran AFTER the whole cell's chain
/// (the code's own comment called it an approximation). Retail's
/// wizard is an ordinary linked record, so `sub_33340`'s walker
/// reaches him at his own seat; the port already tracks that seat as
/// [`crate::engine::features::PlayerChain`] (his chain SUCCESSOR in
/// his own cell), seeded at the carpet's walk slot by
/// `Gen::player_relink` and spliced by `Gen::unlink`.
fn no_mc2_ww_human_chain_order() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_HUMAN_CHAIN_ORDER").is_some())
}

/// A/B toggle for the wizard's RELINK **TEST** inside `sub_33340`'s
/// walk: set `MGC_NO_MC2_WW_HUMAN_MOVE_TEST` to restore the pre-dig
/// test, which asked whether his new tile differs from the RING CELL
/// `(tx, ty)` instead of from the tile HE HIMSELF occupied when the
/// arm started. See the law note at the call site.
fn no_mc2_ww_human_move_test() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_HUMAN_MOVE_TEST").is_some())
}

/// A/B toggle for the wizard's OWN relink inside `sub_33340`'s walk:
/// set `MGC_NO_MC2_WW_HUMAN_SEAT_RELINK` to restore the pre-dig
/// behaviour, where his seat ([`crate::engine::features::PlayerChain`])
/// was reseeded only at the carpet's walk slot, so a visit that
/// carried him into another tile left the seat naming the tile he had
/// LEFT — and every later arrival at it was refused.
fn no_mc2_ww_human_seat_relink() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_HUMAN_SEAT_RELINK").is_some())
}

/// ⭐⭐⭐ THE CRANK COUNT LAW. `sub_33340` cranks
/// `roll_0x155_341 += 28` once per visit that reaches the `v40` block
/// (`NETHERW.EXE` 0x57c67..0x57c8d), and its disc walk reaches the
/// same victim more than once whenever `CopyEntityPosition_57CF0`
/// carries it into a cell the walk has not reached yet. The port used
/// to publish `min(1)` of them, because the harness's stick inversion
/// (`mgc-formats::recover`) un-cranked the recorded `roll_acc` by
/// exactly ONE 28 and a second port-side crank would land 28 high.
/// With the replay driver counting the visits (a throwaway trial step)
/// and handing the number to the recovery, the cap comes off and the
/// port applies what retail did. Set `MGC_NO_MC2_WW_CRANK_COUNT` to
/// restore the one-crank pair: `min(1)` here AND the fixed `28` in the
/// recovery.
pub fn mc2_ww_crank_count_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_CRANK_COUNT").is_none())
}

/// How many 28-unit camera-roll cranks the whirlwind column has handed
/// the human carpet since [`reset_whirl_cranks`]. Harness telemetry
/// ONLY — the replay driver trial-steps a clone, reads this, and
/// re-recovers the stick with the true count (see
/// `mgc_formats::recover::recover_pair_mc2_k`). Never read by
/// simulation logic; the same contract as [`crate::DEBUG_TICK`].
pub static WHIRL_CRANKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Zero the crank telemetry (call before a step).
pub fn reset_whirl_cranks() {
    WHIRL_CRANKS.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Read the crank telemetry (call after a step).
pub fn whirl_cranks() -> u8 {
    WHIRL_CRANKS
        .load(std::sync::atomic::Ordering::Relaxed)
        .min(255) as u8
}

/// Book cranks as a consume site applies them.
pub(crate) fn note_whirl_cranks(n: u8) {
    if n != 0 {
        WHIRL_CRANKS.fetch_add(n as u32, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Gen {
    // ---- ctors ---------------------------------------------------------------

    /// `sub_50430` (EF:36772) — the (10,52) permanent CASTLE/BUILDING
    /// ANCHOR: sprite 205, maxLife 100000 (effectively immortal),
    /// subSpell 500, a 500/2000 mana pool, untargetable. Its action
    /// 0x38 is an EMPTY EV case (EV:2693) — the entity ticks nothing,
    /// which the class-10 dispatch's fall-through arm already is.
    /// maxMana (2000) has no ported home or reader until the MC2
    /// building economy pass — the mana pool rides f140's mana home.
    pub(crate) fn mc2_spawn_castle_anchor(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 52;
            e.tick70 = 0x38;
            e.max_life = 100000;
            e.f140 = 500; // mana_0x90_144 (subSpell 500 shares the value)
            e.f26 = 600;
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 205);
        Some(i)
    }

    /// `sub_50800` (EF:36945) — the (10,74) ALLIANCE EXECUTOR ctor.
    /// FOUR WRITES AND NOTHING ELSE: action 0x51, class 10, byte[0] =
    /// (&0xF6)|1, model 0x4A. No `maxLife`, no `CopyMaxLifeToLife_49A20`,
    /// no `AddEventToMap_57D70`, no `SetEntityIndexAndRot_49CD0` — so
    /// the record keeps `NewEvent_4A050`'s seeds (maxLife 300, life 0,
    /// `byte_0x3E_62` = its own slot, `byte_0x43/0x44` = 0xFF, speed 16)
    /// and it is born UNPOSITIONED and OFF the tile map: retail's
    /// mc2l0-spells-galore slot 70 at t=23946 records x/y/z = 0 and
    /// extents 0.
    ///
    /// ⭐ THE ALLIANCE IS A RECORD, NOT AN INLINE CALL. The impact does
    /// not convert anybody — it mints this, and the conversion is this
    /// record's own class-10 action-0x51 tick ONE TICK LATER
    /// ([`Gen::mc2_alliance_exec_tick`]). The same shape the (10,25)
    /// steal burst already carries; the port had the (10,74) arm poke
    /// the victims inline, which cost an allocation (every later free-
    /// stack pop shifted) and a tick of delay on every converted lane.
    pub(crate) fn mc2_spawn_alliance_exec(&mut self) -> Option<usize> {
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.tick70 = 0x51;
        e.class64 = 10;
        e.flags = (e.flags & !0x9) | 1;
        e.model65 = 74;
        Some(i)
    }

    /// `sub_4F6A0` (EF:36110) — the (10,25) one-shot area blast,
    /// damage TYPE 3: maxLife 8, subSpell 2000 (set but the burst
    /// amount is `byte_0x46_70` — par-set by the caster), byte[0] =
    /// (&0xF6)|1, map-registered, extents 512. No sprite, no RNG.
    pub(crate) fn mc2_spawn_blast25(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 25;
            e.tick70 = 0x19;
            e.max_life = 8;
            e.f140 = 2000;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `sub_4F5F0` (EF:36087) — the (10,23) one-shot area blast,
    /// type 0 amount 25: sprite 7, extents 200, the fire-ctor flag
    /// pattern + bit 0, sound 24 on the burst. The attached
    /// `AddEvent2_847D0(128, 9, 0)` child is presentation, skipped.
    pub(crate) fn mc2_spawn_blast23(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 23;
            e.tick70 = 0x17;
            e.max_life = 8;
            e.f140 = 25;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 7);
        self.mc2_shift_rot(i, 200, 200);
        self.ent[i].flags |= 1;
        Some(i)
    }

    /// `sub_4FFB0` (EF:36559) — the (10,38) LIGHTNING STORM cloud
    /// (Lightning L1/L2's `sub_66FD0` detonation, EF:58821): class-10
    /// model-38, action 40, maxLife 32, sprite 272, render scale 512. It
    /// hovers to +1024 above terrain, then RAINS (9,9) beams — the tick
    /// ([`Gen::mc2_storm_tick`]). The impact tail seeds `f140` with the
    /// tier's subSpell (300/800), which the tick hands to each beam.
    pub(crate) fn mc2_spawn_lightning_burst(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 38;
            e.tick70 = 40;
            e.max_life = 32;
            e.f140 = 300; // overridden by the impact tail (subSpell)
            e.flags &= !8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 272);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `sub_35640` (EF:25876, action 40) — the (10,38) STORM tick: first
    /// rise to +1024 above terrain (64/tick, life frozen while settling),
    /// then each tick fire TWO opposite-yaw (9,9) lightning beams DOWN
    /// (pitch 56), each with a third of the beam reach and a (10,23)
    /// ground impact carrying the storm's subSpell damage; the first of
    /// the pair claps thunder (sound 23). ~2 bolts/tick over 32 ticks =
    /// the rain. (docs/spell-audit/lightning.md — the storm is a cloud
    /// that rains chained beams, NOT a single blast.)
    pub(crate) fn mc2_storm_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i32;
        let target = (ground + 1024).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let z = self.ent[i].z;
        // Settle at the hover height before raining (life frozen).
        if z < target {
            self.ent[i].z = z.saturating_add(64).min(target);
            return;
        }
        if z > target {
            self.ent[i].z = target;
            return;
        }
        // PRE-decrement life test (`v3 = life; life = v3-1; if (v3
        // >= 0)`, EF:25905-07; a post-test cuts the storm one tick
        // short).
        let old_life = self.ent[i].act_life;
        self.ent[i].act_life = old_life - 1;
        if old_life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (sz, id, dmg) = {
            let e = &self.ent[i];
            (e.z, e.id24, e.f140)
        };
        // ⭐⭐⭐ **THE HALF-TURN IS THE LOOP'S FIRST STATEMENT, NOT ITS
        // LAST.** `yaw_0x1C_28 = rand & 0x7FF` seeds the pair, and then
        // the do-while OPENS with `HIBYTE(yaw) = (HIBYTE(yaw) + 4) & 7`
        // (EF:25911-13) — so the FIRST beam flies at `base + 1024` and
        // the second at `base` (two half-turns = back to the seed), the
        // exact inverse of a `base + k * 1024` loop. Both beams exist
        // either way, but they are spawned in the opposite order, so
        // every downstream slot the impact pair claims is swapped: at
        // mc2l6-rsg t=2547 retail's yaw-188 blast takes slot 733 and its
        // yaw-1212 twin takes 845, and the port had them the other way
        // round with all ~70 of their (9,9) children following. ⚠ the
        // sign of a mis-ordered PAIR is that the two records hold each
        // OTHER's values, lane for lane — nothing is missing.
        //
        // The storm also carries the pair on its OWN pose registers
        // (`pitch_0x1E_30 = 56` before the loop, `yaw_0x1C_28`
        // re-stamped each iteration) and hands them to each beam from
        // there, which is why the cloud ends the tick holding the seed
        // yaw and a pitch of 56.
        let r = self.mc2_rand(i);
        self.ent[i].f32 = 56; // pitch_0x1E_30 = 56 (EF:25909)
        let mut yaw = (r & 0x7FF) as u16;
        for k in 0..2u16 {
            yaw = yaw.wrapping_add(1024) & 0x7FF; // the half-turn, FIRST
            self.ent[i].f30 = yaw; // the beams read the CLOUD's yaw
            if let Some(b) = self.mc2_spawn_cast_proj(9, x, y, sz) {
                {
                    let e = &mut self.ent[b];
                    e.id24 = id; // the storm's owner owns the rained beams
                    e.f30 = yaw;
                    e.f32 = 56; // pitch DOWN
                    e.f34 = yaw;
                    e.f36 = 56;
                    e.f146 = 0; // no homing — rain straight down
                    e.f68 = 10;
                    e.f69 = 23; // each beam impacts into (10,23)
                    e.f44 = dmg.clamp(0, u16::MAX as i32) as u16;
                    // `life /= 3` on act_life ONLY, no max_life
                    // touch, no floor (EF:25928).
                    e.act_life /= 3;
                }
                // The thunder is SPAWN-GATED, first pair-iteration
                // only, keyed on the BEAM (EF:25935-36).
                if k == 0 {
                    self.snd(23, b);
                }
            }
        }
    }

    /// `AddMeteor_4ED70` (EF:35731) — the (10,17) METEOR impact:
    /// maxLife 10, subSpell 3000, untargetable, NOT map-registered,
    /// no sprite of its own (the tick grows the quad). No RNG.
    pub(crate) fn mc2_spawn_meteor(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 17;
            e.tick70 = 17;
            e.max_life = 10;
            e.f140 = 3000;
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        Some(i)
    }

    /// `sub_4ECD0` (EF:35707) — the (10,15) wandering FIRE TRAIL:
    /// maxLife 128, actSpeed 256, subSpell 100, ONE RNG draw for the
    /// random yaw, extents (1024, 0x4000). Not map-registered.
    pub(crate) fn mc2_spawn_fire_trail(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 15;
            e.tick70 = 15;
            e.max_life = 128;
            e.f126 = 256; // actSpeed
            e.flags &= !8;
            e.f140 = 100;
            e.f26 = 0;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        let d = self.mc2_rand(i);
        self.ent[i].f30 = (d & 0x7FF) as u16;
        self.refill_life(i);
        self.mc2_shift_rot(i, 1024, 0x4000);
        Some(i)
    }

    /// The (10,19) GROUND-FIRE-SPRAY creator (sprite 228, the fire
    /// family; maxLife 240, subSpell 200, map-registered, byte[0]
    /// bit0 set / bit3 clear, no RNG) — spawned by the dome summit
    /// and the (10,16) vortex machinery.
    ///
    /// A (10,11) THING is NOT a (10,19) entity — retail's
    /// creator-table row 0xB is `NewAdd0A0B_4E840` (EF:1715 →
    /// :35553), the (10,11) SCORCH RING below, a 40-tick one-shot.
    /// (Routing authored (10,11)s here exhausts the pool.)
    pub(crate) fn mc2_spawn_fire_spray(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 19;
            e.tick70 = 19;
            e.f140 = 200;
            e.max_life = 240;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        self.ent[i].flags |= 1;
        self.refill_life(i);
        self.mc2_set_sprite(i, 228);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `NewAdd0A0B_4E840` (EF:35553) — the REAL (10,11): the SCORCH
    /// RING (the volcano-spell ground burn; also the authored
    /// lava-pool decorations). Action 11, maxLife 40, subSpell 200
    /// (→ f140), `word_0x26_38 = 11` (→ f40, the spell-XP row key),
    /// extents (2304, 0x2000), byte[2] |= 2 with bit3 cleared,
    /// INVISIBLE (no sprite), NOT map-registered, no RNG.
    pub(crate) fn mc2_spawn_scorch_ring(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 11;
            e.tick70 = 11;
            e.max_life = 40;
            e.f140 = 200;
            e.f40 = 11;
            e.f26 = 0;
            e.flags = (e.flags & !0x8) | 0x2_0000;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        self.mc2_shift_rot(i, 2304, 0x2000);
        Some(i)
    }

    /// `sub_31FB0` (EF:23490) — the (10,11) action-11 tick: radius
    /// grows every 3rd frame; 40-tick life (despawn on expiry or a
    /// class-0 water cell); area burn each tick (full subSpell the
    /// FIRST tick, /25 after — byte[0] bit1 latches); on reaching
    /// the extents cap (f80>>8 − 1) the OUTER ring stamps once;
    /// every tick the disc 0..radius digs −3 (`sub_31F00` ≡
    /// [`Gen::dig_disc_minus3`]); sound 10. The `sub_6D8B0` XP
    /// rows 0x10/0x11 (f40 = 11/15) bank with the 4.2 ledger like
    /// the dome's row 18. Returns terrain-dirty.
    pub(crate) fn mc2_scorch_ring_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if self.ent[i].f63 % 3 == 0 {
            self.ent[i].f26 += 1;
        }
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        let raw =
            crate::engine::features::tile((self.ent[i].x >> 8) as u8, (self.ent[i].y >> 8) as u8);
        if life < 0 || (1u32 << (self.t.angle[raw] & 0xF)) & 1 != 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let amt = if self.ent[i].flags & 2 != 0 {
            self.ent[i].f140 / 25
        } else {
            self.ent[i].f140
        } as u32;
        // `sub_116A0` (EF:23513), NOT `sub_10C80` — see the dome's
        // twin in mc2::morph: the variant now decides whether the
        // building footprint pass runs at all.
        let hits = self.area_write(i, 0, amt, ctx, false, true);
        // The scorch-ring batch XP (sub_31FB0 EF:23521-25): f40 (the
        // retail word_0x26_38 stamp) discriminates the owning spell —
        // 15 = Earthquake (17), 11 = Crater (16). Human only (F3).
        if hits != 0 && self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
            let spell = match self.ent[i].f40 {
                15 => Some(17u16),
                11 => Some(16u16),
                _ => None,
            };
            if let Some(sp) = spell {
                self.mc2_cast_xp.0.push((self.ent[i].id24, sp, hits as i32));
            }
        }
        let cap = (self.ent[i].f80 >> 8) as i32;
        let mut r = self.ent[i].f26 as i32;
        if r > cap - 1 {
            r = cap - 1;
            if self.ent[i].flags & 2 == 0 {
                self.dig_disc_minus3(i, cap, cap);
            }
        }
        self.ent[i].flags |= 2;
        self.dig_disc_minus3(i, 0, r);
        self.snd(10, i);
        true
    }

    /// `AddAuxiliary_50500` (EF:36812) — the (10,54) proximity AURA
    /// field: invisible, life 128, ONE RNG draw (random yaw),
    /// `dword_0x10_16 = 12845056` (0xC40000 — the SQUARED range),
    /// extents (1024, 0x4000). Not map-registered.
    pub(crate) fn mc2_spawn_aura(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 54;
            e.tick70 = 0x3B;
            e.max_life = 128;
            e.f126 = 256;
            e.flags &= !8;
            e.f140 = 100;
        }
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            // dword_0x10_16 is homed in f26 as the TILE range; the
            // squared reach is derived in the tick. 14 = the ctor
            // default ((14<<8)² = 12845056); a disposition spawn
            // overrides it from the THING's stageTag (sub_4A310).
            e.f26 = 14;
            e.f30 = (d & 0x7FF) as u16;
            e.x = x;
            e.y = y;
            e.z = z;
            e.flags |= 1;
        }
        self.refill_life(i);
        self.mc2_shift_rot(i, 1024, 0x4000);
        Some(i)
    }

    /// `AddWind_4F040` (EF:35852) + `sub_4F1C0` (EF:35921) — the
    /// (10,22) WHIRLWIND: gated on >= 12 free slots; the head (ONE
    /// RNG draw seeds roll = yaw = pitch) plus 11 tail nodes
    /// (model 75, action 82 — an EV no-op, the head drags them)
    /// chained via word_0x32/word_0x34 (f52/f54), then the sprite
    /// stack: per node row 293+index, quad (550/450 per-mille of the
    /// row's rot_speed), z stacked by 2*roll-extent with the node's
    /// offset in the column scratch f50 (`word_0x36_54`).
    ///
    /// Column scratch f50: head = remembered eye z
    /// (`word_0x30_48`), nodes = the z-stack offset
    /// (`word_0x36_54`), victims = the swirl yaw (`word_0x30_48`) —
    /// disjoint entity sets, one home.
    pub(crate) fn mc2_spawn_whirlwind(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.free.len() < 12 {
            return None;
        }
        let h = self.new_event()?;
        {
            let e = &mut self.ent[h];
            e.class64 = 10;
            e.model65 = 22;
            e.tick70 = 22;
            e.f44 = 0;
            e.f46 = 1; // word_0x2E_46 — the lateral drift sign
            e.f128 = 20;
            e.f130 = 10;
            e.f126 = 50;
            e.max_life = 500;
            e.f140 = 1000; // subSpellIndex — the damage magnitude
            e.flags &= !8;
            e.f56 = 1; // byte_0x38_56 (ch0 enrolment; untargetable anyway)
            e.x = x;
            e.y = y;
            e.z = z;
        }
        let d = self.mc2_rand(h);
        {
            let e = &mut self.ent[h];
            e.f34 = ((d & 0x7FF) as u16).wrapping_sub(1) & 0x7FF; // roll
            e.f30 = e.f34; // yaw
            e.f32 = e.f34; // pitch
        }
        self.refill_life(h);
        let (hx, hy, hz) = (x, y, z);
        let mut prev = h;
        for i in 0..11u16 {
            let Some(c) = self.new_event() else { break };
            // `qmemcpy(child, head, 0xA8)` (EF:35882) — the WHOLE
            // record, not the subset the node machinery reads. Every
            // graded lane the head carries at this point is the
            // node's birth value: yaw/pitch/roll = the ctor's
            // `(rand & 0x7FF) - 1` (EF:35873-76 — the head's own
            // yaw/pitch are later overwritten by the bolt in
            // `sub_65820` EF:63000-01, the nodes keep the ctor's),
            // actSpeed 50 / minSpeed 20 / maxSpeed 10 (EF:35865-67),
            // subSpellIndex 1000 (EF:35869), byte_0x38_56 = 1
            // (EF:35872), word_0x2E_46 = 1 (EF:35863), fov 0
            // (EF:35871). Measured mc2l6-rsg pair 10086→10087: all
            // 11 nodes `heading/pitch: retail 1299 port 0`, `speed:
            // retail 50 port 16` (16 = NewEvent's default, EV:568).
            {
                let hd = self.ent[h];
                let e = &mut self.ent[c];
                e.class64 = 10;
                e.model65 = 75;
                e.tick70 = 82;
                e.max_life = 500;
                e.act_life = hd.act_life;
                e.id24 = hd.id24;
                e.rand = hd.rand;
                e.f30 = hd.f30; // yaw_0x1C_28
                e.f32 = hd.f32; // pitch_0x1E_30
                e.f34 = hd.f34; // roll_0x20_32
                e.f36 = hd.f36; // fov_0x22_34 (0)
                e.f46 = hd.f46; // word_0x2E_46
                e.f56 = hd.f56; // byte_0x38_56
                e.f126 = hd.f126; // actSpeed_0x82_130 = 50
                e.f128 = hd.f128; // minSpeed_0x84_132 = 20
                e.f130 = hd.f130; // maxSpeed_0x86_134 = 10
                e.f140 = hd.f140; // subSpellIndex_0x2A_42 = 1000
                e.flags &= !8;
                e.f44 = i + 1; // word_0x2C_44 — the node index
                e.f52 = prev as u16;
                e.f54 = 0;
                e.f63 = i as u8;
                e.x = hx;
                e.y = hy;
                e.z = hz;
            }
            self.ent[prev].f54 = c as u16;
            self.link(c, hx, hy, hz);
            prev = c;
        }
        self.link(h, hx, hy, hz);
        // sub_4F1C0 — the stacked sprite column.
        let ground = self.ground_z(hx, hy) as i16;
        let mut zoff = 0i32;
        let mut n = h;
        loop {
            let row = self.ent[n].f44 as usize + 293;
            let v5 = SPRITE_PARAMS[row].rot_speed_8 as i32;
            self.mc2_set_sprite(n, row as u16);
            let (shift, roll_ext) = ((550 * v5 / 1000) as u16, (450 * v5 / 1000) as i32);
            self.mc2_shift_rot(n, shift, roll_ext as u16);
            self.ent[n].z = (zoff as i16).wrapping_add(ground);
            self.ent[n].f50 = zoff as i16; // word_0x36_54
            zoff += 2 * roll_ext;
            let next = self.ent[n].f54 as usize;
            if next == 0 {
                break;
            }
            n = next;
        }
        Some(h)
    }

    /// `sub_4EDC0` (EF:35749) — the (10,16) TORNADO-DRAG the summit
    /// vortex (mc2::morph model 18) emits each pulse: subSpell 200,
    /// life 100..199 (RNG 1), launch speed 52..101 (RNG 2), random
    /// heading (RNG 3), vertical impulse 256 (f44 = `word_0x2C_44`),
    /// sprite 210, hover 64 above ground, reclaimable (byte[2] bit
    /// 1), untargetable. Its ACTION is 16 decimal → `sub_32600`, the
    /// ballistic rolling/burning BOULDER — NOT the whirlwind driver:
    /// `0x214110 = sub_33110` belongs to action 0x16 = 22 (dec/hex
    /// trap); the class-10 `strA0` row 0x0010 is `0x213600 =
    /// sub_32600`, EF:1618. The launch impulse is a VELOCITY DELTA in
    /// dest_x/dest_y (`MoveEntity_57FA0` onto the zeroed
    /// `axis_0x9A`, EF:35764-69 — the ball-machinery home), not an
    /// absolute eye point.
    pub(crate) fn mc2_spawn_boulder16(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 16;
            e.tick70 = 16;
            e.f140 = 200;
            e.f44 = 256;
            e.flags = (e.flags & !8) | 0x2_0000;
        }
        let r1 = self.ent_rand(i);
        self.ent[i].max_life = (r1 % 0x64 + 100) as u32;
        let r2 = self.ent_rand(i);
        self.ent[i].f126 = (r2 % 0x32 + 52) as i16;
        let r3 = self.ent_rand(i);
        let yaw = (r3 & 0x7FF) as u16;
        self.ent[i].f30 = yaw;
        self.link(i, x, y, z);
        let gz = (self.ground_z(x, y) + 64) as i16;
        self.ent[i].z = gz;
        let mut d = (0u16, 0u16, 0i16);
        Self::polar_step(&mut d, yaw, 0, self.ent[i].f126);
        self.ent[i].dest_x = d.0;
        self.ent[i].dest_y = d.1;
        self.refill_life(i);
        self.mc2_set_sprite(i, 210);
        Some(i)
    }

    /// `sub_32600` (0x213600, EF:23729-828) — the (10,16) volcano
    /// BOULDER: a ballistic rolling/burning rock. Velocity deltas
    /// ride dest_x/dest_y (clamped ±80/tick), vertical velocity in
    /// f44 (`word_0x2C_44`, gravity −28 clamped [−384, 256]). On
    /// terrain contact it rebounds `vz = −(vz/4)` (trunc), splashes
    /// out on water ((10,5), despawn), lights a `(10,6)` standing
    /// fire (life 30, subSpell ×3 = 150) where none burns, and
    /// settles when vz ≤ 28; resting, it takes the `sub_58030`
    /// terrain-slope push + 250/256 friction (the mana-ball roll
    /// law). NO sound, NO player sway, NO XP (unlike the whirlwind
    /// driver).
    pub(crate) fn mc2_boulder16_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        // byte[0] |= 2 (EF:23749-51).
        self.ent[i].flags |= 2;
        let vx = (self.ent[i].dest_x as i16).clamp(-80, 80);
        let vy = (self.ent[i].dest_y as i16).clamp(-80, 80);
        let (x, y, z, vz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f44 as i16)
        };
        let (px, py) = (x.wrapping_add(vx as u16), y.wrapping_add(vy as u16));
        let mut pz = z.wrapping_add(vz);
        // Gravity AFTER the step, on the old vz (EF:23765-70).
        self.ent[i].f44 = (vz - 28).clamp(-384, 256) as u16;
        let ground = self.ground_z(px, py) as i16;
        if ground > pz {
            pz = ground;
            // Rebound −(vz/4), truncated toward zero (EF:23778).
            let v8 = self.ent[i].f44 as i16 as i32;
            self.ent[i].f44 = (-(v8 / 4)) as i16 as u16;
            // Water (tested at the CURRENT position, EF:23779):
            // (10,5) splash, id inherited, gone — despawn only if
            // the splash actually spawned (pool-full keeps rolling).
            //
            // ⭐ THE SPLASH IS NOT AN EARLY EXIT. `DisableEntityDrawing04_
            // 57F10` (EF:238f10) is a ONE-LINE flag set —
            // `byte[1] |= 4` — and `sub_32600` runs its whole tail
            // afterwards regardless: `dword_0x10_16++`, the
            // `CopyEntityPosition_57CF0` move onto the predicted
            // point, and the resting slope+friction block. A drowned
            // boulder therefore takes ONE more step, lands ON the
            // splash, and banks the friction, all on its reap tick.
            // The port `return`ed here instead, freezing the record
            // where it stood. mc2l6-rsg t=6355 slot 553: retail steps
            // (51382, 15405, 84) → (51446, 15377, 0) with `scratch10`
            // 8 → 9 and `dest_x/y` 64/−28 → 62/−27, and the splash it
            // just spawned at slot 11 links AHEAD of it in the shared
            // cell — proof 553 was still in the map chain, and still
            // moving, after the flag went on.
            if self.cap_bit(x, y) == 1 {
                let own = self.ent[i].id24;
                if let Some(s) = self.mc2_spawn_splash(px, py, pz) {
                    self.ent[s].id24 = own;
                    self.ent[i].flags |= 0x400;
                }
            } else {
                // Light a (10,6) standing fire where none burns
                // (`sub_10B70` cell probe, EF:23790-801): life 30
                // (act only — max stays the ctor's), subSpell ×3.
                //
                // ⭐ THE PROBE IS A 2x2 RING **PLUS A RADIUS**, AND
                // THE PORT HAD ONLY THE CELL. `sub_10B70` (EF:3921)
                // anchors at `((x - 128) >> 8, (y - 128) >> 8)` —
                // each axis wrapped to a byte — walks all FOUR cells
                // of the 2x2 block from there, and gates every
                // candidate on `sub_583F0_distance_3d(a1, j.pos) <=
                // 0x80`. Cell membership alone is not the test: a
                // 256-unit tile is twice the radius wide and the
                // metric is genuinely 3-D (`dy² + dx² + **dz²**`), so
                // a fire on the same tile but down a slope is far
                // outside it. mc2l6-rsg t=6291 is the witness — the
                // boulder at slot 189 steps to (49018, 16375, 3540)
                // with its own earlier fire (slot 620) still burning
                // at (49087, 16382, 2982): the same cell (191, 63),
                // 69 units away horizontally and **562 in 3-D**.
                // Retail lights the second fire; the port's cell-only
                // probe read "already burning" and lit nothing.
                //
                // ⭐ AND THE PROBE HAS NO REAP TEST. `sub_10B70`'s
                // per-candidate predicate is exactly `class == a2 &&
                // a3 == model && dist3d <= 0x80` — a fire that died
                // earlier in this same walk still occupies its
                // ground until the tick-top reaper frees it, and it
                // still suppresses a re-light. mc2l6-rsg t=6423:
                // boulder 408 rests at (47360, 15649, 540) with fire
                // 104 burnt out 44 units away at (47337, 15611, 548)
                // — slot 104 < 408, so its own dispatch has already
                // stamped `flags.b1_reap4`; retail lights nothing and
                // the port's reap filter lit a fresh (10,6) every
                // tick the boulder sat there.
                let ax = (((px as i32) - 128) >> 8) as u8;
                let ay = (((py as i32) - 128) >> 8) as u8;
                let mut burning = false;
                'probe: for dy in 0..2u8 {
                    for dx in 0..2u8 {
                        let t =
                            crate::engine::features::tile(ax.wrapping_add(dx), ay.wrapping_add(dy));
                        let mut j = self.map_entity[t] as usize;
                        while j != 0 {
                            let e = &self.ent[j];
                            if e.class64 == 10
                                && e.model65 == 6
                                && Self::mc2_dist3((px, py, pz), (e.x, e.y, e.z)) <= 0x80
                            {
                                burning = true;
                                break 'probe;
                            }
                            j = e.next20 as usize;
                        }
                    }
                }
                if !burning {
                    let own = self.ent[i].id24;
                    if let Some(f) = self.mc2_spawn_fire6(px, py, pz) {
                        let e = &mut self.ent[f];
                        e.id24 = own;
                        e.act_life = 30;
                        e.f140 *= 3;
                        self.ent[i].f26 = 0; // dword_0x10_16 reset
                    }
                }
                // Settle (EF:23802-03).
                if (self.ent[i].f44 as i16) <= 28 {
                    self.ent[i].f44 = 0;
                }
            }
        }
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1); // dword_0x10_16++
        self.move_relink(i, px, py, pz);
        // Resting on ground: slope push + 250/256 friction (the
        // mana-ball `sub_58030` law, EF:23809-20; trunc division).
        if ground == pz {
            let (tx, ty) = ((px >> 8) as u8, (py >> 8) as u8);
            let h = |dx: u8, dy: u8| {
                self.t.height
                    [crate::engine::features::tile(tx.wrapping_add(dx), ty.wrapping_add(dy))]
                    as i32
            };
            let sx = h(0, 0) - h(1, 0) + h(0, 1) - h(1, 1);
            let sy = h(0, 0) + h(1, 0) - h(0, 1) - h(1, 1);
            let vx = ((vx as i32 + sx) * 250 / 256) as i16;
            let vy = ((vy as i32 + sy) * 250 / 256) as i16;
            self.ent[i].dest_x = vx as u16;
            self.ent[i].dest_y = vy as u16;
        } else {
            self.ent[i].dest_x = vx as u16;
            self.ent[i].dest_y = vy as u16;
        }
    }

    /// `sub_51790` (EF:37439) — the (10,71) expanding FISSURE:
    /// life = maxLife = 120, subSpell 20000, byte[0] = (&0xF6)|1,
    /// map-registered, extents (1280, 2048). No sprite, no RNG.
    pub(crate) fn mc2_spawn_fissure(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 71;
            e.tick70 = 0x4E;
            e.max_life = 120;
            e.act_life = 120;
            e.f140 = 20000;
            e.f71 = 0;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.mc2_shift_rot(i, 1280, 2048);
        Some(i)
    }

    /// `AddFireSpheres_4F2A0` (EF:35936) + `sub_4F440` (EF:35989) —
    /// the (10,76) orbiting FIRE-SPHERE ORB
    /// (docs/traces/mc2-class10-m76-fire-spheres.md): gated on >= 26
    /// free slots; one invisible hub (maxLife 80, subSpell 70,
    /// extents 640, action 0x53) + 25 sprite-340 satellites (model
    /// 77, action 0x54 = NO handler — the hub repositions them)
    /// chained via f52/f54, laid out as a 5-ring x 5-slot spherical
    /// lattice (ONE RNG draw per satellite = the 84..147 spin rate).
    /// Only the 5 slot-0 spheres are targetable damage-carriers; the
    /// other 20 are visuals (byte[2] bit7 render flag). The
    /// satellites' `AddEvent2(128,1,0)` children are presentation,
    /// skipped. Runtime-disposition-only in retail (no generate
    /// pass, no par consumption).
    pub(crate) fn mc2_spawn_fire_orb(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.free.len() < 26 {
            return None;
        }
        let h = self.new_event()?;
        {
            let e = &mut self.ent[h];
            e.class64 = 10;
            e.model65 = 76;
            e.tick70 = 0x53;
            e.max_life = 80;
            e.f140 = 70; // subSpellIndex — the per-sphere damage
            e.f126 = 40; // actSpeed
            e.f130 = 192; // maxSpeed — breathe bound A
            e.f128 = 480; // minSpeed — breathe bound B
            e.f56 = 1;
            e.f68 = 0;
            e.f69 = 0;
            e.f44 = 0; // current ring radius
            e.f46 = 0; // fontTypeIndex_0x3D_61 — the breathe step
            e.f71 = 0; // byte_0x46_70 — the phase machine
            e.flags = (e.flags & !0x9) | 1;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(h);
        let mut prev = h;
        for i in 0..25u8 {
            let Some(s) = self.new_event() else { break };
            {
                // `qmemcpy(entity2, entity, sizeof(type_entity_0x6E8E))`
                // (EF:35967) — the satellite is a FULL STRUCT CLONE of
                // the hub taken at CONSTRUCTION time, so it inherits
                // the hub's actSpeed/minSpeed/maxSpeed triple
                // (EF:35949-52 = 40/480/192) as well as maxLife and
                // subSpell. The loop then overrides only model, action,
                // the 0x32/0x34 links, 0x3E, 0x43 and 0x44.
                // The trace bank's satellite bullet
                // (docs/traces/mc2-class10-m76-fire-spheres.md:89) lists
                // "maxLife 80 / subSpell 70 / extents / byte[0]" and
                // omits the speed words; this ctor was built from that
                // enumeration, so the satellites kept `new_event`'s
                // defaults (+126 = 16, +128 = +130 = 0, features.rs:1723).
                // Measured: mc2l0-spells-galore pair 4397->4398, 25
                // (10,77) rows `speed retail 40 port 16` — the take's
                // entire free-run break (horizon 4397).
                // ⭐ Read the triple from the HUB rather than hardcoding
                // 40/480/192: the caller overrides the hub's maxLife to
                // 30 and subSpell to 180 AFTER this returns, which is
                // itself the proof that the clone happens here and now,
                // and the tier sweep in the take varies these values.
                // ⭐ AND THE SAME CLONE CARRIES `word_0x2C_44`. The
                // shipped `AddFireSpheres_4F2A0` copies the WHOLE
                // 168-byte record — `NETHERW.EXE` file 0x73B57
                // `b9 2a000000 mov ecx,0x2a` / 0x73B5C
                // `8b 75 fc mov esi,[ebp-4]` (the HUB) / 0x73B5F
                // `89 c7 mov edi,eax` (the new satellite) / 0x73B61
                // `f3 a5 rep movsl` = 42 dwords = 0xA8, the record
                // stride the same function divides by at 0x73B6C
                // (`be a8000000 mov esi,0xa8`) — and the hub's own
                // ctor has already stored ZERO there: 0x73B1B
                // `66 c7 43 2c 0000  movw $0x0,0x2c(%ebx)`
                // (EF:35989 `entity->word_0x2C_44 = 0`, the clone at
                // EF:35997). The loop's only overrides are 0x40/0x45/
                // 0x32/0x34/0x3E/0x43/0x44, so @0x2C reaches the
                // satellite as 0 while OUR satellites kept
                // `new_event`'s default 100 (features.rs `+44 = 100`).
                // MEASURED (mc2l24 t=45560, `dump-state --port
                // --start 45559`): retail's satellite at slot 17 holds
                // `f2c` 0, the port 100 — 25 records a firestorm.
                // ⚠ UNGRADED: `f2c` is not in `verify_mc2.rs`'s
                // compared field list, so this lane can only take a
                // unit test (`the_fire_orb_satellites_clone_the_hub_f44`).
                let (id, rand, life, act_spd, min_spd, max_spd, ring_w) = {
                    let e = &self.ent[h];
                    (e.id24, e.rand, e.act_life, e.f126, e.f128, e.f130, e.f44)
                };
                let e = &mut self.ent[s];
                e.class64 = 10;
                e.model65 = 77;
                e.tick70 = 0x54;
                e.max_life = 80;
                e.act_life = life;
                e.id24 = id;
                e.rand = rand;
                e.f126 = act_spd;
                e.f128 = min_spd;
                e.f130 = max_spd;
                e.f140 = 70;
                if !no_mc2_orb_satellite_f44() {
                    e.f44 = ring_w;
                }
                e.f56 = 1;
                e.flags = (e.flags & !0x9) | 1;
                e.f52 = prev as u16;
                e.f54 = 0;
                e.f63 = i;
                e.f68 = i / 5; // ring
                e.f69 = i % 5; // slot
                e.x = x;
                e.y = y;
                e.z = z;
            }
            self.ent[prev].f54 = s as u16;
            self.link(s, x, y, z);
            prev = s;
        }
        self.link(h, x, y, z);
        self.mc2_shift_rot(h, 640, 640);
        // sub_4F440 — the ring layout.
        {
            let e = &mut self.ent[h];
            e.f46 = 18; // breathe step
            e.f44 = e.f130 as u16; // radius := maxSpeed (192)
            e.f30 = 0;
            e.f32 = 0;
        }
        let mut n = self.ent[h].f54 as usize;
        while n != 0 {
            let slot = self.ent[n].f69;
            self.ent[n].flags &= !1;
            if slot != 0 {
                self.ent[n].flags = (self.ent[n].flags | 0x80_0000) & !8;
            } else {
                self.ent[n].flags |= 8; // the damage carriers
            }
            let d = self.mc2_rand(n);
            let spin = ((d & 0x3F) + 84) as u16;
            let ring = self.ent[n].f68;
            let (yaw, pitch, roll_spin, fov_spin) = match ring {
                0 => ((512 - 96 * slot as i32) as u16 & 0x7FF, 0u16, spin, 0u16),
                1 => (512, (512 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
                2 => (0, (-96 * slot as i32) as u16 & 0x7FF, 0, spin),
                3 => (256, (256 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
                _ => (768, (768 - 96 * slot as i32) as u16 & 0x7FF, 0, spin),
            };
            {
                let e = &mut self.ent[n];
                e.f30 = yaw;
                e.f32 = pitch;
                e.f34 = roll_spin;
                e.f36 = fov_spin;
            }
            let radius = self.ent[h].f44 as i16;
            let mut pos = (x, y, z);
            Self::polar_step(&mut pos, yaw, pitch, radius);
            self.move_relink(n, pos.0, pos.1, pos.2);
            self.mc2_set_sprite(n, 340);
            n = self.ent[n].f54 as usize;
        }
        Some(h)
    }

    // ---- ticks ---------------------------------------------------------------

    /// `sub_339B0` (EF:24562) — the orb hub tick: phase 0 init sizes
    /// the ring from the LEADER's extents when f146 carries one
    /// (`maxSpeed = pitch>>1` floored at 128, `minSpeed = 6*pitch>>2`
    /// capped at 640 — EF:24581-90; a wizard's 121 pitch gives the
    /// tight [128,181] shell, a castle brain's 128·w+640 flips the
    /// bounds INVERTED so the breathe hard-snaps across up to
    /// [640,3392]) → phase 1 pulse: snap to the leader + collapse on
    /// its death, terrain clamp (z >= ground + radius — `sub_33C70`),
    /// the ±18 radius breathe (`sub_33AD0`), the constellation
    /// tumble (+22/+16 head spin, per-sphere spin, all 25
    /// repositioned — `sub_33B20`), the slot-0 damage pass
    /// (`sub_10C80(type 0, 70)` per carrier, sound 3 on any hit —
    /// `sub_33C00`); life out → phase 2 collapse: keep tumbling,
    /// radius -= |step|, and at < 0 spawn a (10,0) ground fire and
    /// tear the whole 26-entity chain down (`sub_33D40`).
    ///
    /// The leader is the impact seam's struck victim (proj.rs
    /// (10,76) arm) — trace §2's "dead code" call is REFUTED
    /// (adjudication in docs/traces/mc2-class10-m76-fire-spheres.md
    /// §7): retail's `sub_65B50` (EF:63029) pins the hub via the
    /// charged fireball. An authored map-THING orb keeps f146 = 0
    /// and behaves exactly as before.
    pub(crate) fn mc2_fire_orb_tick(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].f71 == 0 {
            // Phase-0 leader sizing (EF:24581-90): ring bounds from
            // the victim's AABB half-extent. The human wizard lives
            // outside the pool — its extents are the sprite-44
            // derivation (`SetEntityIndexAndRot(44)`: pitch = s6/2),
            // same law the pool wizards get at spawn.
            let leader = self.ent[i].f146;
            let pitch = match leader {
                0 => None,
                PLAYER_TARGET => Some(self.mc2_params_ext(44).0 / 2),
                v => ((v as usize) < self.ent.len()).then(|| self.ent[v as usize].f80),
            };
            if let Some(p) = pitch {
                let e = &mut self.ent[i];
                e.f130 = ((p as i32) >> 1).max(128) as i16;
                e.f128 = ((6 * p as i32) >> 2).min(640) as i16;
            }
            self.ent[i].f71 = 1;
        } else if self.ent[i].f71 > 1 {
            if self.ent[i].f71 == 2 {
                if self.ent[i].f46 < 0 {
                    self.ent[i].f46 = -self.ent[i].f46;
                }
                self.mc2_orb_tumble(i);
                let v7 = self.ent[i].f44 as i16 - self.ent[i].f46;
                self.ent[i].f44 = v7 as u16;
                if v7 < 0 {
                    let (x, y, z) = {
                        let e = &self.ent[i];
                        (e.x, e.y, e.z)
                    };
                    self.mc2_spawn_fire(x, y, z);
                    let mut n = i;
                    loop {
                        self.ent[n].flags |= 0x400;
                        let next = self.ent[n].f54 as usize;
                        if next == 0 || next == n {
                            break;
                        }
                        n = next;
                    }
                }
            }
            return;
        }
        // Phase 1 — `sub_33C70` order: leader snap FIRST, then the
        // terrain/ceiling clamps, then the leader-death collapse
        // (EF:24726-45). The snap rides the leader's position plus
        // its `array_0x52_82.yaw` z-offset (f78; wizard = 100), so
        // an airborne victim wears the orb 100 units overhead while
        // a castle's huge radius lets the ground clamp win and the
        // sphere balloons over the footprint.
        let leader = self.ent[i].f146;
        let mut leader_dead = false;
        if leader != 0 {
            if leader == PLAYER_TARGET {
                let off = (self.mc2_params_ext(44).1 / 2) as i16;
                self.move_relink(i, ctx.px, ctx.py, ctx.pz.wrapping_add(off));
                leader_dead = ctx.pdead;
            } else if (leader as usize) < self.ent.len() {
                let v = leader as usize;
                let (vx, vy, vz, dead) = {
                    let t = &self.ent[v];
                    (
                        t.x,
                        t.y,
                        t.z.wrapping_add(t.f78 as i16),
                        t.act_life < 0 || t.flags & 0x400 != 0,
                    )
                };
                self.move_relink(i, vx, vy, vz);
                leader_dead = dead;
            }
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let floor = (self.ground_z(x, y) as i16).wrapping_add(self.ent[i].f44 as i16);
        if self.ent[i].z < floor {
            self.ent[i].z = floor;
        }
        // Cave ceiling clamp, margin = the RADIUS not fov
        // (EF:24751-58: ceiling − word_0x2C_44).
        if self.is_cave() {
            let c = (self.ceiling_z(x, y) as i16).wrapping_sub(self.ent[i].f44 as i16);
            if self.ent[i].z > c {
                self.ent[i].z = c;
            }
        }
        // Leader dead → collapse, set at `sub_33C70`'s tail
        // (EF:24743-45): the rest of THIS tick still pulses; the
        // next tick enters phase 2.
        if leader_dead {
            self.ent[i].f71 = 2;
        }
        // sub_33AD0 — the breathe bounce.
        {
            let e = &mut self.ent[i];
            let v2 = e.f46 + e.f44 as i16;
            let (lo, hi) = (e.f128 as i16, e.f130 as i16);
            e.f44 = v2 as u16;
            if v2 <= lo {
                if v2 < hi {
                    e.f44 = hi as u16;
                    e.f46 = -e.f46;
                }
            } else {
                e.f44 = lo as u16;
                e.f46 = -e.f46;
            }
        }
        self.mc2_orb_tumble(i);
        // sub_33C00 — the slot-0 damage pass. The hit sound fires
        // PER CARRIER inside the loop (EF:24710-14), not once for
        // the volley.
        let amt = self.ent[i].f140 as u32;
        let mut n = self.ent[i].f54 as usize;
        while n != 0 {
            if self.ent[n].f69 == 0 && self.area_write(n, 0, amt, ctx, false, false) != 0 {
                self.snd(3, i);
            }
            n = self.ent[n].f54 as usize;
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 1 {
            self.ent[i].f71 = 2;
        }
    }

    /// `sub_33B20` (EF:24656) — the constellation tumble: the hub
    /// spins +22 yaw / +16 pitch, each satellite advances its own
    /// spin rates, and every sphere is re-placed at hub + spherical
    /// (satAngle + hubAngle, radius). No RNG.
    fn mc2_orb_tumble(&mut self, i: usize) {
        {
            let e = &mut self.ent[i];
            e.f30 = e.f30.wrapping_add(22) & 0x7FF;
            e.f32 = e.f32.wrapping_add(16) & 0x7FF;
        }
        let (hx, hy, hz, hyaw, hpitch, radius) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.f44 as i16)
        };
        let mut n = self.ent[i].f54 as usize;
        while n != 0 {
            {
                let e = &mut self.ent[n];
                e.f30 = e.f30.wrapping_add(e.f34) & 0x7FF;
                e.f32 = e.f32.wrapping_add(e.f36) & 0x7FF;
            }
            let (syaw, spitch) = (self.ent[n].f30, self.ent[n].f32);
            let mut pos = (hx, hy, hz);
            Self::polar_step(
                &mut pos,
                syaw.wrapping_add(hyaw) & 0x7FF,
                spitch.wrapping_add(hpitch) & 0x7FF,
                radius,
            );
            self.move_relink(n, pos.0, pos.1, pos.2);
            n = self.ent[n].f54 as usize;
        }
    }

    /// `sub_3A2D0` (EF:29443) — the (10,71) fissure tick
    /// (docs/traces/mc2-class10-tail-helper-closure.md §2): phase 0
    /// init (`word_0x2C_44 = maxLife/8`, per-beat damage =
    /// 4*(20000/120) ≈ 664); each tick the disc radius ramps
    /// grow → pin-at-3*ref (with a 1-in-5 phase-jump roll) → shrink,
    /// clamped [0,15], and every cell of the disc takes a **±1
    /// heightmap jitter** (sign = life & 1 — the ground vibrates; no
    /// terrain-type write, no children); a `byte_0x46_70 > 1` tick
    /// adds a half-radius inner pass; `byte > 3` = the terminal
    /// tail-off (life only). Every 4th tick: sprite quad grows to
    /// the radius, sound 10, the type-0 area beat (the id-0xF
    /// spellbook report is emitted by the spell-XP column).
    pub(crate) fn mc2_fissure_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // ⭐ `if (life_0x8 >= 0)` GUARDS THE WHOLE BODY, AND ITS ELSE
        // ARM IS THE REAP FLAG ALONE — no life decrement. NETHERW.EXE
        // 0x5eadc `cmpl $0x0,0x8(%ebx)` / `jl 0x5ed96`, and 0x5ed96 is
        // `push %ebx; call 0x7c710` (= `DisableEntityDrawing04_57F10`,
        // `orb $0x4,0xd(%eax)` = our bit 10) and nothing else. Retail's
        // fall-through tail is a bare `decl 0x8(%ebx)` (0x5ED8C), so a
        // spent fissure PARKS at life -1 for one tick and is reaped on
        // the NEXT one. The port decremented unconditionally and
        // latched the flag the tick life first went negative — one tick
        // early (mc2l6-rsg t=24739 slot 401: retail life -1, port -2).
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        if self.ent[i].f71 == 0 {
            let maxl = self.ent[i].max_life as i32;
            self.ent[i].f44 = (maxl >> 3) as u16; // word_0x2C_44
            self.ent[i].f26 = 0;
            self.ent[i].f71 = 1;
            self.ent[i].f140 = 4 * (self.ent[i].f140 / maxl.max(1) as i32);
        }
        let mut dirty = false;
        if self.ent[i].f71 <= 3 {
            let v4 = self.ent[i].f44 as i32;
            let maxl = self.ent[i].max_life as i32;
            let life = self.ent[i].act_life;
            let mut v6 = if maxl - 3 * v4 >= life as i32 {
                if maxl - 5 * v4 > life as i32 {
                    self.ent[i].f26 -= 1;
                    self.ent[i].f26 as i32
                } else {
                    let d = self.mc2_rand(i);
                    if d % 5 == 0 {
                        self.ent[i].f71 += 2;
                    }
                    3 * v4
                }
            } else {
                self.ent[i].f26 += 1;
                self.ent[i].f26 as i32
            };
            v6 = v6.clamp(0, 3 * v4).clamp(0, 15);
            let second_pass = self.ent[i].f71 > 1;
            if second_pass {
                self.ent[i].f71 -= 1;
            }
            if v6 > 0 {
                // Cell center rounds `(pos + 128) >> 8` (EF:29527-28).
                let (cx, cy) = (
                    (self.ent[i].x.wrapping_add(128) >> 8) as i16,
                    (self.ent[i].y.wrapping_add(128) >> 8) as i16,
                );
                let sign: i16 = if self.ent[i].act_life & 1 == 1 { 1 } else { -1 };
                for r in [Some(v6), second_pass.then_some(v6 >> 1)]
                    .into_iter()
                    .flatten()
                {
                    for (dx, dy) in self.ring_cells(0, r) {
                        let t = crate::engine::features::tile(
                            (cx.wrapping_add((dx as i8) as i16)) as u8,
                            (cy.wrapping_add((dy as i8) as i16)) as u8,
                        );
                        let v = (self.t.height[t] as i16 + sign).clamp(0, 255);
                        self.t.height[t] = v as u8;
                    }
                }
                dirty = true;
                if self.ent[i].act_life & 3 == 0 {
                    self.mc2_shift_rot(i, (v6 << 8) as u16, 2048);
                    self.snd(10, i);
                    let amt = self.ent[i].f140 as u32;
                    let hits = self.area_write(i, 0, amt, ctx, false, false);
                    // Tremor batch XP (sub_3A2D0 EF:29580).
                    if hits != 0 && self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                        self.mc2_cast_xp.0.push((self.ent[i].id24, 15, hits as i32));
                    }
                }
            }
        }
        self.ent[i].act_life -= 1;
        dirty
    }

    /// `sub_33110` (EF:24155) — the whirlwind driver: while alive,
    /// wander + drag (`sub_331A0`), the lift-and-throw pass
    /// (`sub_33340`), the every-8th-tick contact pass (`sub_33710`),
    /// loop sound 49; on expiry the teardown (`sub_338D0`) clears
    /// the grabs and despawns the 12-node chain.
    pub(crate) fn mc2_whirlwind_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < 0 {
            self.mc2_whirlwind_teardown(i, ctx);
            return;
        }
        self.mc2_whirlwind_move(i);
        self.mc2_whirlwind_lift(i, ctx);
        self.mc2_whirlwind_contact(i);
        self.snd(49, i);
    }

    /// `sub_331A0` (EF:24177) — head wander (roll drift flips sign
    /// on a coin every 16 ticks, 32-unit lateral wobble → the eye
    /// center, +341 yaw and 120 forward, ground-clamped) + the tail
    /// drag (each node pulled toward its predecessor to the gap
    /// `72 - 4*(12 - index)`, z = head z + the node's f50 offset).
    /// The eye xy rides f142/f144-free scratch: we keep it in the
    /// head's dest fields (the portal column's home, unused here
    /// otherwise) — `axis_0x9A_154x`.
    fn mc2_whirlwind_move(&mut self, i: usize) {
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        self.ent[i].f50 = z; // word_0x30_48 — remembered eye z
        // `if (!(a1x->byte_0x3E_62 & 0xF))` (EF:24191) reads the tick
        // counter AS IT STANDS — `sub_331A0` never increments it; the
        // ONLY writer is the dispatch walk's post-handler `++`
        // (ported at world.rs `f63.wrapping_add(1)` after the match).
        // The port's own pre-check increment double-clocked the head
        // (coin every 8 ticks, one tick early) and shifted
        // `sub_33710`'s `& 7` cadence with it. Measured mc2l6-rsg pair
        // 10236→10237: head 547 `phase3e 64`, retail draws (rand
        // 31487→23358, `f2e 1→-1`), the port did not (65 & 0xF != 0).
        // NewEvent seeds the counter with the SLOT index (EV:577), so
        // the phase is per-head, not per-birth.
        if self.ent[i].f63 & 0xF == 0 {
            let d = self.mc2_rand(i);
            if d & 1 == 0 {
                self.ent[i].f46 = -self.ent[i].f46;
            }
        }
        let roll = (self.ent[i].f34 as i32 + 11 * self.ent[i].f46 as i32) as u16 & 0x7FF;
        self.ent[i].f34 = roll;
        let mut eye = (x, y, z);
        Self::polar_step(&mut eye, roll, 0, 32);
        self.ent[i].dest_x = eye.0;
        self.ent[i].dest_y = eye.1;
        let yaw = self.ent[i].f30.wrapping_add(341) & 0x7FF;
        self.ent[i].f30 = yaw;
        let mut pos = eye;
        Self::polar_step(&mut pos, yaw, 0, 120);
        let ground = self.ground_z(pos.0, pos.1) as i16;
        self.move_relink(i, pos.0, pos.1, ground);
        // Tail drag.
        let head_z = ground;
        let mut prev = i;
        let mut n = self.ent[i].f54 as usize;
        while n != 0 {
            let (nx, ny, nz) = {
                let e = &self.ent[n];
                (e.x, e.y, e.z)
            };
            let (px, py, pz) = {
                let e = &self.ent[prev];
                (e.x, e.y, e.z)
            };
            let yaw = Self::angle_between(nx, ny, px, py);
            self.ent[n].f30 = yaw;
            // 2-D: retail's `EuclideanDistXYZ_58490` (EF:24213)
            // never reads z — with the permanent per-node z offset
            // a 3-D read overshoots the gap and bunches the tail.
            let dh2 = Self::dist2_sq(nx, ny, px, py);
            let _ = pz;
            let d = Self::isqrt(dh2 as u32) as i32;
            let gap = 72 - 4 * (12 - self.ent[n].f44 as i32);
            let mut pos = (nx, ny, nz);
            if d > gap {
                Self::polar_step(&mut pos, yaw, 0, (d - gap) as i16);
            }
            let zoff = self.ent[n].f50;
            pos.2 = zoff.wrapping_add(head_z);
            self.move_relink(n, pos.0, pos.1, pos.2);
            prev = n;
            n = self.ent[n].f54 as usize;
        }
    }

    /// `sub_33340` (EF:24229) — the lift-and-throw pass over the
    /// radius-12 tile disc around the eye: pool CREATURES swirl
    /// inward (yaw = bearing+591, drift 96), lift near the eye
    /// (+114/tick above it, GRAB latched past the 768+rand%768
    /// threshold), spin at yaw-step 204 while grabbed, release past
    /// the far ring (d² >= 5308416), and take the head's 1000
    /// mailbox damage every airborne tick (`sub_11900`). The
    /// spellbook report (id 0x15) is emitted by the spell-XP column.
    ///
    /// Deliberate approximation (cited):
    /// - the HUMAN player arm (yaw-step 56, threshold 384, camera
    ///   roll crank, actSpeed 80) needs the FlightVerb takeover seam
    ///   (the level-end cinematic's seam) — until then the player is
    ///   damaged when overlapping the eye ring but not lifted.
    ///
    /// ⭐ THE BAND RUNS ON EVERY VICTIM, NOT ONLY THE ONES THE FUNNEL
    /// MOVED. `sub_580E0(&pred, getTerrainAlt(&pred), word_0xc,
    /// word_0xa, v37)` (EF:24390-94) sits BELOW the arm chain, after
    /// the swirl step and the cave clamp, and is followed by an
    /// UNCONDITIONAL `CopyEntityPosition_57CF0` (EF:24395) — so a
    /// victim the disc merely contains is still floor-clamped to
    /// `getTerrainAlt(pred) + word_0xc` every tick. `v37` (the sink
    /// step) is the row's `word_0xe` in the FAR-GRAB arm alone
    /// (EF:24315) and 0 in every other arm, and `word_0xa` is
    /// `sub_580E0`'s dead a4 (EF:40372 ignores it).
    ///
    /// mc2l6-rsg pair 10089→10090, slot 745 — a (10,39) sphere in the
    /// MID-RING arm (`explain`: yaw AND word_0x30_48 both 393→477 =
    /// `bearing+591`, xy stepped 96 to (6996,26015), and z 0→73 with
    /// no arm that writes z). Row 59 (`NewEvent_4A050` seeds
    /// `&str_D7BD6[59]`, Events.cpp:573, and the sphere ctor
    /// `CreateManaSphere_500C0` EF:36607 never overrides it) has
    /// `word_0xc` = 0 and `word_0xe` = −4, so the band is a pure
    /// clamp UP to the terrain: 73 is `getTerrainAlt(6996,26015)`.
    ///
    /// The victim filter is `sub_33810` VERBATIM (EF:24452-515):
    /// class-2 m7/8; class-3 non-castle, non-own (the ONLY owner
    /// check retail makes); class-5 minus actions {232,180} and
    /// models {10,15,18,27,28}; class-10 {13,14,39,57}.
    fn mc2_whirlwind_lift(&mut self, i: usize, ctx: &MobCtx) {
        let (ex, ey, eye_z, id, amt) = {
            let e = &self.ent[i];
            (e.dest_x, e.dest_y, e.f50, e.id24, e.f140 as u32)
        };
        // Cell center rounds: `(pos + 128) >> 8` (EF:29527-28 fissure,
        // :24273-74 lift, :24531-32 teardown; truncation would shift
        // the disc half a tile on the high side).
        let (cx, cy) = (
            (self.ent[i].x.wrapping_add(128) >> 8) as i16,
            (self.ent[i].y.wrapping_add(128) >> 8) as i16,
        );
        let mut hits = 0u32;
        // ⭐⭐⭐ THE HUMAN IS A VICTIM OF THE SAME BODY (dig Q7).
        // `sub_33810` (EF:24452-515) passes class 3 / non-castle /
        // non-own-id, and the human wizard IS class 3 model 0 — which
        // is precisely what `v40` (`0x57be8 cmp dl,3` + `0x57bed cmp
        // byte [ebx+0x40],0x0`) selects for its 56/384 constants. He
        // lives outside our pool, so he is not on a tile chain and the
        // walk above cannot reach him; instead his LIVE pose is tested
        // against each ring cell below, which reproduces retail's
        // MULTI-VISIT (a victim carried into a cell the ring has not
        // reached yet is lifted and moved a second time — the same
        // traversal the billing law already depends on).
        //
        // ⚠ THE BANKING, STATED DELIBERATELY (`docs/DEVIATIONS.md`
        // `flood.rs::flood_shove` is the precedent for getting this
        // wrong by accident): the three pieces of retail state this
        // arm keeps on the victim record all bank on the HUMAN'S OWN
        // POOL RECORD (`Gen::mc2_pinned`, the class-0 pinned seat
        // `mc2_spawn_human_record` pops at retail's own slot) — the
        // `byte[3] & 0x10` grab latch on its `F_GRABBED`, the `byte[1]
        // & 8` mover veto on its `F_STOP`, the latched `word_0x30_48`
        // swirl heading on its `f50`, and the `rand_0x14_20` draw on
        // its live `rand` lane. That is retail's OWN home for all four
        // (mc2l30 slot 83 records `flags 525 -> 268438029` at t=2985 =
        // `byte[1] |= 8` plus `byte[3] |= 0x10`, and `rand 19555 ->
        // 6946` = one 9377/9439 step). ONLY the pose rides a channel —
        // `Gen::player_whirl` — because the carpet's x/y/z/yaw live in
        // `flight::Mc1State`, not in the pool record.
        let cs = self.mc2_pinned.0 as usize;
        // `sub_33810` case 1 (EF:24473): your OWN funnel never grabs
        // you. `id24` on a whirlwind head is the caster's id.
        // ⚠ `MGC_NO_MC2_WW_MIDRING` is dig W4's older A/B and it takes
        // the WHOLE human column back to the pre-W4 knock spiral, so
        // this arm stands down under it too — the two switches are a
        // ladder, not independent axes.
        let human_law = !no_mc2_ww_human_grab()
            && !no_mc2_ww_midring()
            && cs != 0
            && cs < self.ent.len()
            && id != PLAYER_TARGET;
        let hrow = if self.is_cave() {
            crate::flight::Mc2Row::CAVE
        } else {
            crate::flight::Mc2Row::OPEN
        };
        let mut hp = (ctx.px, ctx.py, ctx.pz);
        let mut hyaw = ctx.pyaw & 0x7FF;
        let mut hgrab = human_law && self.ent[cs].flags & F_GRABBED != 0;
        let mut hstop = false;
        // Any GRAB-FAMILY visit this tick (inner lift / near-grab /
        // far-grab). The mid ring keeps dig W4's heading+step channel.
        let mut hseized = false;
        let mut hmid: Option<u16> = None;
        let mut hact80 = false;
        // ⭐ ANY visit that survived `sub_33810` — the tail runs on all
        // four arms, so even a far-band visit that seizes nothing is a
        // POSITION WRITE (`CopyEntityPosition_57CF0`).
        let mut hvisited = false;
        // Dig Q6 — one `roll_0x155_341 += 28` per visit that reaches
        // the `v40` block, i.e. exactly where `hact80` is raised.
        let mut hcrank: u8 = 0;
        for (dx, dy) in self.ring_cells(0, 12) {
            let tx = (cx.wrapping_add((dx as i8) as i16)) as u8;
            let ty = (cy.wrapping_add((dy as i8) as i16)) as u8;
            let cell_t = crate::engine::features::tile(tx, ty);
            let mut j = self.map_entity[cell_t] as usize;
            // ⭐⭐⭐ THE WIZARD HAS A RANK IN THIS CHAIN, AND IT IS NOT
            // "LAST". `sub_33340`'s walker is a plain chain walk and
            // retail's carpet is an ordinary linked record, so the
            // wizard is visited WHERE HE SITS — his rank decides
            // whether the victims behind him are walked from his OLD
            // tile or, once `CopyEntityPosition_57CF0` has relinked
            // him, from his NEW one (the law dig I landed as
            // `MGC_NO_MC2_WW_HUMAN_RELINK_WALK`). The port carries him
            // out of pool but DOES track the seat:
            // `Gen::player_chain` is `(cell, next)` — his chain
            // SUCCESSOR — seeded by `Gen::player_relink` at the
            // carpet's own walk slot (which is where retail's relink
            // happens) and spliced by `Gen::unlink` when that
            // successor leaves. So stop the walk in front of
            // `player_chain.next`, run his arm there, and resume.
            // `next == 0` is the chain TAIL, which is the old
            // after-the-chain placement and still correct.
            let mut seat_matched = false;
            let mut seat_block = usize::MAX;
            let mut resume = 0usize;
            'cell: loop {
                while j != 0 {
                    if human_law
                        && !no_mc2_ww_human_chain_order()
                        && seat_block != j
                        && self.player_chain.next as usize == j
                        && self.player_chain.cell
                            == crate::engine::features::tile((hp.0 >> 8) as u8, (hp.1 >> 8) as u8)
                    {
                        // His seat: he precedes this record. Break out,
                        // run the arm below, and come back here. ⚠ NOT a
                        // one-per-cell latch — a victim relinked in FRONT
                        // of him hands the walker his seat a second time,
                        // which is the whole point of the relink walk;
                        // `seat_block` suppresses only the immediate
                        // re-entry at the record we are resuming into.
                        resume = j;
                        seat_matched = true;
                        break;
                    }
                    let next = self.ent[j].next20 as usize;
                    let c = &self.ent[j];
                    let victim = match c.class64 {
                        2 => matches!(c.model65, 7 | 8),
                        3 => c.id24 != id && c.model65 != 2,
                        5 => {
                            !matches!(c.tick70, 232 | 180)
                                && !matches!(c.model65, 10 | 15 | 18 | 27 | 28)
                        }
                        10 => matches!(c.model65, 13 | 14 | 39 | 57),
                        _ => false,
                    };
                    // ⭐ NO REAP SKIP. `sub_33340`'s victim loop is
                    // `if (sub_33810(a1x, ix))` and NOTHING ELSE
                    // (EF:24286) — no `byte[1] & 4`, no life test — and
                    // `sub_33810` itself (EF:24452-515) tests only class,
                    // model, action and the same-owner id. A record the
                    // tick-top reap has not yet unlinked is still on the
                    // tile chain and retail still lifts, spins and drifts
                    // it. The old skip here was an INVENTED GUARD, and it
                    // was false exactly when it mattered: mc2l0-spells-
                    // galore t=12379, slot 101 — a (5,1) goat GRABBED by
                    // the funnel reaches `KillEntity_1C930`'s phase (120,
                    // `f63 & 7 == 0`), which raises 0x400 in the goat's
                    // OWN dispatch at slot 101; all three whirlwind heads
                    // sit at higher slots, so every one of them skipped it
                    // and the corpse froze in mid-air for its last tick
                    // while retail spun it 3x204 and drifted it 3x(128
                    // out, +114 up). It was the take's free-run horizon.
                    if !victim || (mc2_ww_reap_skip() && c.flags & 0x400 != 0) {
                        j = next;
                        continue;
                    }
                    let d2 = Self::dist2_sq(ex, ey, c.x, c.y) as i64;
                    let grabbed = c.flags & F_GRABBED != 0;
                    let (vx, vy, vz) = (c.x, c.y, c.z);
                    let mut pos = (vx, vy, vz);
                    let mut drift = 0i16;
                    let mut airborne = false;
                    // `v37` — `sub_580E0`'s sink step. Initialised 0 at
                    // the top of the victim body (EF:24291) and written
                    // in the FAR-GRAB arm ALONE (EF:24315, `v37 =
                    // ix->dword_0xA0_160x->word_160_0xe_14`).
                    let mut float = 0i16;
                    if d2 >= 3_211_264 {
                        if grabbed {
                            self.ent[j].flags |= super::mobs::F_STOP;
                            airborne = true;
                            drift = 64;
                            float = BEHAVIOR[self.ent[j].row156 as usize].v_14;
                            self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                            if d2 >= 5_308_416 {
                                self.ent[j].flags &= !F_GRABBED; // FLUNG
                            }
                        }
                    } else {
                        let bearing = Self::angle_between(ex, ey, vx, vy);
                        if grabbed {
                            self.ent[j].flags |= super::mobs::F_STOP;
                            drift = 128;
                            airborne = true;
                            pos.2 = pos.2.wrapping_add(114);
                            self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                        } else if d2 >= 0x40000 {
                            // Mid ring: swirl inward.
                            let v14 = bearing.wrapping_add(591) & 0x7FF;
                            self.ent[j].f50 = v14 as i16;
                            self.ent[j].f30 = v14;
                            drift = 96;
                        } else {
                            // Inner ring: the lift.
                            self.ent[j].flags |= super::mobs::F_STOP;
                            pos.0 = ex;
                            pos.1 = ey;
                            let v9 = vz as i32 - eye_z as i32 + 57;
                            let galt = self.ground_z(ex, ey) as i16;
                            pos.2 = ((v9 + galt as i32).max(galt as i32)) as i16;
                            self.ent[j].f30 = self.ent[j].f30.wrapping_add(204) & 0x7FF;
                            let d = self.ent_rand(j);
                            if v9 >= 768 + (d % 768) as i32 {
                                self.ent[j].flags |= F_GRABBED;
                                self.ent[j].f50 = self.ent[j].f30 as i16;
                            }
                        }
                    }
                    if drift != 0 {
                        let swirl = self.ent[j].f50 as u16 & 0x7FF;
                        Self::polar_step(&mut pos, swirl, 0, drift);
                    }
                    // Cave ceiling clamp on the thrown victim
                    // (EF:24382-88), before the band.
                    if self.is_cave() {
                        let c = (self.ceiling_z(pos.0, pos.1) as i16 as i32
                            - self.ent[j].f84 as i32) as i16;
                        if pos.2 > c {
                            pos.2 = c;
                        }
                    }
                    // `sub_580E0(&pred, getTerrainAlt(&pred), word_0xc,
                    // word_0xa, v37)` (EF:24390-94) — the victim's own
                    // float band, on EVERY victim in the disc, keyed off
                    // the ground under the PREDICTED xy. `word_0xc` =
                    // `v_12` (the floor offset), `word_0xa` = `v_10` (a4,
                    // dead), `v37` = the far-grab sink step above.
                    {
                        let row = &BEHAVIOR[self.ent[j].row156 as usize];
                        let galt = self.ground_z(pos.0, pos.1) as i16;
                        Self::mc2_alt_core(&mut pos.2, galt, row.v_12, float);
                    }
                    // `CopyEntityPosition_57CF0(ix, &pred)` (EF:24395) is
                    // UNCONDITIONAL — and it only relinks when the tile
                    // word changes (EF:40294-40302), exactly what
                    // `move_relink` does, so an unmoved victim is a plain
                    // position write, never a tile-chain reorder.
                    self.move_relink(j, pos.0, pos.1, pos.2);
                    if airborne {
                        hits += 1;
                        // ⭐⭐⭐ THE LIFT PASS BILLS THROUGH `sub_11900`, THE
                        // **SINGLE/INVERSE** PROTOCOL — `sub_33340`
                        // EF:24400 is a bare `sub_11900(a1x, ix, 0, v26)`,
                        // and `sub_11900` (EF:4375-88, EXE 0x3611f-36137)
                        // is `if (src) amt = a4; else amt += a4`. This
                        // writer had been on the AREA protocol
                        // ([`Gen::mail_write`], accumulate-while-pending),
                        // which is its exact inverse.
                        //
                        // It is load-bearing because the disc walk VISITS
                        // ONE VICTIM TWICE: `CopyEntityPosition_57CF0`
                        // (EF:24395) relinks the victim mid-walk, and a
                        // victim carried into a tile the ring walker has
                        // not reached yet is lifted, moved AND billed a
                        // second time on the same pass. Retail's second
                        // letter lands on a PENDING box and OVERWRITES it;
                        // the port's ACCUMULATED, doubling the hit.
                        //
                        // mc2l6-rsg t=10110 is the take's horizon: head 159
                        // lifts rival 378 at cell (25,105), the relink
                        // carries it to (25,106), and the walker bills it
                        // again. Retail reads `mail0.amt` 20 → life
                        // 10000 − 20 + 20 regen = 10000 (pinned at
                        // max_life); the port read 40 → 9960 + 20 = 9980.
                        // ⚠ The double VISIT is retail's own behaviour —
                        // both columns land the identical two lift steps
                        // and agree on the final (6600, 27378).
                        if no_whirlwind_single_bill() {
                            self.mail_write(MailTarget::Pool(j), 0, amt, id);
                        } else {
                            self.mail_write_single(MailTarget::Pool(j), 0, amt, id);
                        }
                    }
                    // ⭐⭐⭐ THE WALK RE-READS THE LINK **AFTER** THE BODY.
                    // Retail's per-cell loop is `for (ix =
                    // Entities[map[cell]]; ix != Entities[0]; ix =
                    // Entities[ix->oldMapEntity_0x16_22])` (EF:24283-85),
                    // and the shipped EXE settles it: the increment
                    // `mov bx,[ebx+0x16]` sits at `NETHERW.EXE` file
                    // 0x57ea3, AFTER `call 0x7c4f0`
                    // (`CopyEntityPosition_57CF0`, linear 0x57CF0) at
                    // 0x57e7b and after the `sub_11900` bill at 0x57e9b.
                    // So the link the walker follows is the one the
                    // RELINK just wrote: a victim carried into another
                    // tile hands the walker that tile's chain, and the
                    // walk continues there. The port captured `next20`
                    // BEFORE the body, which is the same multi-visit the
                    // billing law already depends on but a DIFFERENT
                    // traversal.
                    j = if no_mc2_ww_walk() {
                        next
                    } else {
                        self.ent[j].next20 as usize
                    };
                    seat_block = usize::MAX;
                }
                // ── THE HUMAN'S VISIT TO THIS CELL ────────────────────────
                // Reached either at his own seat (the `resume` break
                // above) or, when `player_chain.next` is 0, as the chain
                // TAIL — which is where this block always used to run.
                // ⚠ THE SEAT IS A POINTER POSITION, NOT A CELL TEST. Once
                // the walker has followed a relinked victim out of the ring
                // cell (`self.ent[j].next20` after `CopyEntityPosition`),
                // `tx`/`ty` no longer name the tile it is walking — so the
                // seat path must NOT be gated on them. Only the TAIL path
                // (no seat in this chain) still asks whether he is in the
                // ring cell at all.
                if human_law
                    && (resume != 0
                        || (!seat_matched && (hp.0 >> 8) as u8 == tx && (hp.1 >> 8) as u8 == ty))
                {
                    hvisited = true;
                    // ⭐⭐⭐ HIS OWN TILE AT THE HEAD OF THE ARM — the only
                    // thing `CopyEntityPosition_57CF0` compares
                    // (EF:40584, `NETHERW.EXE` 0x7c4fe/0x7c505). See the
                    // relink note in the tail below.
                    let (otx, oty) = ((hp.0 >> 8) as u8, (hp.1 >> 8) as u8);
                    let d2 = Self::dist2_sq(ex, ey, hp.0, hp.1) as i64;
                    // `v38`/`v34` — 56 and 384 for the human (`0x57c04 mov
                    // eax,0x38`, `0x57c19 mov eax,0x180`), against the
                    // creatures' 204/768 the pooled body above uses.
                    const V38: u16 = 56;
                    const V34: u32 = 384;
                    let mut swirl = self.ent[cs].f50 as u16;
                    let mut pred = hp;
                    let mut v30 = 0i16;
                    let mut float = 0i16;
                    let mut banded = false;
                    // `v39` — the BILL flag, PER VISIT (retail zeroes it
                    // at the top of the victim body, EF:24291 / `0x57bdf
                    // xor edi,edi`). ⚠ ONLY the two GRABBED arms raise it
                    // (`0x57d81 mov dh,1` near, `0x57dbd mov dl,1` far);
                    // the INNER LIFT arm does not, and mc2l30 agrees to
                    // the tick — slot 83 is still `life 10000, mail[0]
                    // (0,0)` on the grab tick t=2985 and only takes its
                    // first `(100, 133)` letter at t=2986.
                    let mut bill = false;
                    if d2 >= 3_211_264 {
                        // FAR (`0x57c41 cmp eax,0x310000` / `jnl 0x57da9`).
                        if hgrab {
                            hstop = true;
                            hseized = true;
                            banded = true;
                            bill = true; // `0x57dbd mov dl,1`
                            float = hrow.buoyancy; // `v37` = row `word_0xe`
                            v30 = 64; // `0x57dde mov esi,0x40`
                            hyaw = hyaw.wrapping_add(V38) & 0x7FF;
                            if d2 >= 5_308_416 {
                                // `0x57ded cmp eax,0x510000` — the FLING.
                                hgrab = false;
                            }
                        }
                    } else if hgrab {
                        // NEAR-GRABBED (`0x57c5e test cl,0x10` →`0x57d65`).
                        hstop = true;
                        hseized = true;
                        banded = true;
                        bill = true; // `0x57d81 mov dh,1`
                        v30 = 128; // `0x57d65 mov ecx,0x80`
                        pred.2 = pred.2.wrapping_add(114); // `0x57d96 add esi,0x72`
                        hyaw = hyaw.wrapping_add(V38) & 0x7FF;
                    } else {
                        // NOT YET GRABBED — the `v40` block runs first
                        // (`0x57c67`): camera roll +28 and `actSpeed = 80`.
                        // ⭐⭐⭐ BOTH HALVES ARE PORTED NOW (dig Q6). The
                        // `roll_0x155_341 += 28` half is NOT a separate
                        // Type_164 word: `sub_5D530` integrates and turns
                        // on the SAME `[ebx+0xa4] -> +0x155` (0x81d6d,
                        // 0x81df9) this block writes at 0x57c86, so it IS
                        // the carpet's recorded `roll_acc` bank filter.
                        // W4's mc2l1 t=268→269 counter-example was an
                        // artifact of the harness, not of retail: the
                        // stick is RECOVERED by inverting that same
                        // accumulator, so adding the crank without
                        // teaching `recover_stick` about it double-counts.
                        // With both halves in, t=269 closes on stick 48
                        // instead of 105 and lands the recorded 138 either
                        // way, while t=272 — where NO byte-range stick
                        // explains retail's +35 at all — only closes with
                        // the crank.
                        hact80 = true;
                        hcrank = hcrank.saturating_add(1);
                        if d2 >= 0x40000 {
                            // MID RING (`0x57c96` / `0x57d3d`) — dig W4's
                            // landed arm, kept on its own channel so the
                            // mc2l1 t=269 and mc2l30 t=2973 arithmetic is
                            // byte-for-byte the one it landed.
                            let v14 =
                                Self::angle_between(ex, ey, hp.0, hp.1).wrapping_add(591) & 0x7FF;
                            swirl = v14;
                            hyaw = v14;
                            v30 = 96;
                            hmid = Some(v14);
                        } else {
                            // INNER LIFT (`0x57ca3`) — the arm this dig
                            // ports. `predictedAxis = a1x->axis_0x9A_154x`
                            // (0x57cae `movsd`/`movsw` from `esi+0x9a`),
                            // `v9 = ix->position.z - a1x->word_0x30_48 +
                            // 57` (0x57cbf..0x57cce), z = v9 + terrainAlt
                            // clamped up to terrainAlt, `yaw += v38`, then
                            // the LCG and the LATCH:
                            // `if (v9 >= v34 + rand % v34) { byte[3] |=
                            // 0x10; word_0x30_48 = yaw_0x1C_28; }`
                            // (0x57d06..0x57d34). `v30` stays 0 — the
                            // closing `MoveEntity_57FA0` does not step.
                            hstop = true;
                            hseized = true;
                            banded = true;
                            pred.0 = ex;
                            pred.1 = ey;
                            let v9 = hp.2 as i32 - eye_z as i32 + 57;
                            let galt = self.ground_z(ex, ey) as i16;
                            pred.2 = ((v9 + galt as i32).max(galt as i32)) as i16;
                            hyaw = hyaw.wrapping_add(V38) & 0x7FF;
                            let d = self.ent_rand(cs);
                            if v9 >= (V34 + d % V34) as i32 {
                                hgrab = true;
                                swirl = hyaw;
                            }
                        }
                    }
                    self.ent[cs].f50 = swirl as i16;
                    if v30 != 0 {
                        Self::polar_step(&mut pred, swirl & 0x7FF, 0, v30);
                    }
                    if banded || !no_mc2_ww_tail_band() {
                        // EF:24382-94 — the cave ceiling clamp and then
                        // `sub_580E0(&pred, getTerrainAlt(&pred),
                        // word_0xc, word_0xa, v37)`. ⚠ Applied on the GRAB
                        // arms only: retail runs it on every visit, but the
                        // mid ring's channel is dig W4's landed one and
                        // touches no z at all — folding the band into it
                        // would move a law that is already pinned.
                        // `array_0x52_82.fov` on the carpet record is 100
                        // (`AddPlayer_4A920` EF:33334, params row 44).
                        if self.is_cave() {
                            let c = (self.ceiling_z(pred.0, pred.1) as i16).wrapping_sub(100);
                            if pred.2 > c {
                                pred.2 = c;
                            }
                        }
                        let galt = self.ground_z(pred.0, pred.1) as i16;
                        Self::mc2_alt_core(&mut pred.2, galt, hrow.clearance, float);
                    }
                    hp = pred;
                    // ⭐⭐⭐ AND `CopyEntityPosition_57CF0` RELINKS *HIM*
                    // TOO. The tail's last call (`0x57e7b`) is the same
                    // one every pooled victim gets: tile-word compare
                    // (0x7c4fb), UNLINK (0x7c50d), AddEventToMap
                    // (0x7c518) — so a visit that carries the wizard into
                    // another tile makes him that tile's chain HEAD, and
                    // his seat is a live thing for the rest of the walk.
                    // The port reseeded `Gen::player_chain` only at the
                    // carpet's own walk slot, so after an inner lift
                    // teleported him onto the eye the seat still named the
                    // tile he had left and every later arrival at it was
                    // refused.
                    //
                    // WITNESS (mc2l24 t=8033, head 200): the inner lift
                    // puts him on the eye (30124, 25678) = cell 25717 and
                    // latches the grab, a near-grab then lands
                    // (30182, 25792); `player_chain` still read
                    // `(25461, 0)` — the cell BEFORE the teleport, tail of
                    // that chain — so when victim 146 relinked
                    // 25973 -> 25717, in FRONT of him, the walker skipped
                    // his seat. Retail takes that third visit: one more
                    // `+114` z (5316 -> 5430), one more `+56` yaw
                    // (926 -> 982) and one more 128-step
                    // (30182, 25792) -> (30240, 25906) — its own
                    // `sqrt(58² + 114²) = 127.9`.
                    if !no_mc2_ww_human_seat_relink() {
                        self.player_relink(hp.0, hp.1);
                    }
                    if bill {
                        // `v39 = 1` ⇒ `v31++` and `sub_11900(a1x, ix, 0,
                        // v26)` (0x57e98-0x57e9b) — the SINGLE/INVERSE
                        // protocol, the same writer and the same
                        // `subSpellIndex_0x2A_42` amount the pooled body
                        // above bills, mailed to the human's own inbox.
                        // mc2l30 records it to the unit: slot 83 `mail[0]`
                        // is `(100, 133)` from t=2986 and `life` runs
                        // 10000 → 8000 across the hold.
                        hits += 1;
                        if no_whirlwind_single_bill() {
                            self.mail_write(MailTarget::Player, 0, amt, id);
                        } else {
                            self.mail_write_single(MailTarget::Player, 0, amt, id);
                        }
                    }
                    // ⭐⭐⭐ THE HUMAN IS ON THE TILE CHAIN, SO HIS RELINK
                    // HANDS THE WALKER HIS NEW CELL. Retail's per-cell walk
                    // is `for (ix = Entities[map[cell]]; ix != Entities[0];
                    // ix = Entities[ix->oldMapEntity_0x16_22])`
                    // (EF:24283-85) and the increment `mov bx,[ebx+0x16]`
                    // sits at `NETHERW.EXE` file 0x57ea3, AFTER
                    // `CopyEntityPosition_57CF0` at 0x57e7b — the SAME law
                    // the pooled body's `j = self.ent[j].next20` already
                    // carries. The human wizard is an ordinary class-3
                    // pool record ON THAT CHAIN, so when his own visit
                    // moves him into another tile the relink puts him at
                    // that tile's head and the walker walks THAT chain
                    // next, re-visiting every victim already parked there.
                    // We carry him out of pool, so the chain has to be
                    // handed over explicitly.
                    //
                    // mc2l24 t=8058 is the witness and it names its own
                    // proof: retail's slot 310 (a (5,25)) ends the tick
                    // with `prev18 = 116` — the HUMAN immediately in front
                    // of it in cell (120,98)'s chain — after whirl 200's
                    // inner lift teleported both onto the eye. That second
                    // visit is one more `yaw += 204` (1189 -> 1393, the
                    // port stopped at 1189), one more LCG step (42071 ->
                    // 48022 -> 13877, the port stopped at 48022) and one
                    // more z re-lift (2903 -> 2961 -> 3019, the port
                    // stopped at 2961) — and the second draw is what
                    // clears `768 + rand % 768` (821 against 1174), which
                    // is why retail latches `byte[3] |= 0x10` and copies
                    // `word_0x30_48 = yaw` while the port never grabs.
                    // t=8000 slot 133 (a (5,17)) is the same shape at
                    // three visits instead of two: `yaw` 1640 -> 204 =
                    // 1640 + 3*204 (the port lands 0 = 1640 + 2*204),
                    // `rand` three LCG steps to 51206 (the port two, to
                    // 60103) and z 2640 -> 2853 in three +71 lifts.
                    if !no_mc2_ww_human_relink_walk() {
                        let ntx = (hp.0 >> 8) as u8;
                        let nty = (hp.1 >> 8) as u8;
                        // ⭐⭐⭐ "HE MOVED" IS A TEST ON **HIS OWN** TILE, NOT
                        // ON THE RING CELL — and the note directly above
                        // this one already says why ("the seat is a
                        // pointer position, not a cell test"), for the
                        // ENTRY test alone. `CopyEntityPosition_57CF0`
                        // (EF:40584, `NETHERW.EXE` file 0x7c4f0) compares
                        // the RECORD'S OWN tile bytes and nothing else —
                        // `0x7c4fe mov al,[edi+0x4d]` / `0x7c501 cmp
                        // al,[edx]` (edx = &pos->x + 1) and `0x7c505 mov
                        // al,[edi+0x4f]` / `0x7c508 cmp al,[esi+0x3]`,
                        // both equal ⇒ `0x7c50b jz 0x7c529`, the plain
                        // `movsd`/`movsw` position copy with NO relink;
                        // otherwise `0x7c50e call 0x7c650` (unlink) +
                        // `0x7c518 call 0x7c570` (AddEventToMap, head
                        // insert). It never sees the walker's cell. Once
                        // the seat path has followed a chain OUT of
                        // `(tx, ty)`, the ring cell is no longer the tile
                        // he is standing in, so testing his new tile
                        // against `(tx, ty)` fires the relink-follow on a
                        // wizard WHO NEVER LEFT HIS TILE: the walker is
                        // handed that tile's HEAD and re-walks every
                        // victim already parked there.
                        //
                        // WITNESS mc2l24 t=7999->8000, head 10 (a (10,71)
                        // funnel), slot 133 (a (5,17)). Retail's walk
                        // reaches 133 THREE times: `yaw` 1640 -> 204 =
                        // 1640 + 3*204 (`0x57c0b mov eax,0xcc`), `rand`
                        // 43625 -> 51206 = three 9377x+9439 steps, `z`
                        // 2640 -> 2853 = three +71 lifts. The port took a
                        // FOURTH (yaw 408, rand 51365, z 2924): after the
                        // walker followed him out of ring cell (122,88)
                        // into (121,89) and ran his seat visit there, his
                        // near-grab left him at (31215,23010) — still tile
                        // (121,89) — and the `(tx, ty)` test read that as
                        // a move, restarting (121,89) at its head, which
                        // was the just-relinked 133. Retail's walker reads
                        // his UNCHANGED `oldMapEntity_0x16_22`
                        // (`0x57ea3 mov bx,[ebx+0x16]`) and carries on at
                        // his successor — which is exactly `resume`.
                        let (btx, bty) = if no_mc2_ww_human_move_test() {
                            (tx, ty)
                        } else {
                            (otx, oty)
                        };
                        if ntx != btx || nty != bty {
                            // He left the cell: the walker follows HIM,
                            // so the seat we broke out of is abandoned.
                            resume = 0;
                            j = self.map_entity[crate::engine::features::tile(ntx, nty)] as usize;
                            // ⚠ AND HE IS NOT RE-VISITED ON ARRIVAL.
                            // `AddEventToMap` puts him at the HEAD of the
                            // new chain, so retail's walker reads HIS
                            // `oldMapEntity_0x16` — the tile's previous
                            // head — and carries on from there. With the
                            // seat now naming that very record (his own
                            // relink just wrote it), the seat test would
                            // otherwise fire again the instant we arrive
                            // and hand him an extra visit: mc2l1 t=283 and
                            // mc2l30 t=2986 both break exactly that way.
                            seat_block = j;
                            continue 'cell;
                        }
                    }
                }
                // He stayed in the cell: pick the chain back up at the
                // record his seat sits in front of.
                if resume != 0 {
                    seat_block = resume;
                    j = resume;
                    resume = 0;
                    continue 'cell;
                }
                break;
            }
        }
        // ── PUBLISH THE HUMAN'S SEIZURE ────────────────────────
        // ⭐⭐⭐ EVERY VISIT IS A POSITION WRITE, NOT ONLY A SEIZING
        // ONE. `sub_33340`'s FAR arm on an ungrabbed victim
        // (`0x57db4 f6 43 0f 10` `testb $0x10,0xf(%ebx)` /
        // `0x57db8 74 3e` `je 0x57df8`) jumps straight into the
        // unconditional tail: `MoveEntity_57FA0(&pred, …, v30 = 0)` is
        // a no-op, nothing on the victim is touched — and then the cave
        // clamp, `sub_580E0` (`0x57e6d`) and `CopyEntityPosition_57CF0`
        // (`0x57e7b`) run anyway. The BAND is the whole content of that
        // visit: it floors the victim at `getTerrainAlt(pred) +
        // row.word_0xc`, which for the carpet's row is ground + 256.
        // The port computed it and threw it away, because it only
        // published a seizure.
        //
        // WITNESS (mc2l24 t=7914, the take's head after laws 11+12):
        // three funnels visit the wizard. Head 12 is FAR (d² = 3,963,650
        // ≥ 0x310000) and ungrabbed, so it seizes nothing — but the
        // ground under (31723, 29697) is 1590 and the band lifts z
        // 1818 -> **1846**, which is retail's recorded z to the unit.
        // The port published nothing and held 1818.
        let hpublish = hseized || hmid.is_some() || (hvisited && !no_mc2_ww_tail_publish());
        if human_law && hpublish {
            if hstop {
                // `byte[1] |= 8` — consumed (and cleared) by the veto
                // at the head of `sub_5D530` (`NETHERW.EXE` 0x81d39
                // `mov ah,[ebx+0xd]` / `test ah,0x8` / `and dl,0xf7`),
                // the human carpet mover. Same one-shot the rival
                // carpet, the corpse mover, the creature core and the
                // ball already read.
                self.ent[cs].flags |= super::mobs::F_STOP;
            }
            if hgrab {
                self.ent[cs].flags |= F_GRABBED;
            } else {
                self.ent[cs].flags &= !F_GRABBED;
            }
            // ⭐⭐⭐ THE MID RING PUBLISHES THE RESOLVED ABSOLUTE POSE.
            // Retail's four arms all fall through to the SAME tail —
            // `MoveEntity_57FA0(&pred, word_0x30_48, 0, v30)`, the cave
            // clamp, `sub_580E0` and `CopyEntityPosition_57CF0`
            // (EF:24375-95) — so the mid ring writes an absolute
            // position with a banded z, and a ring walk that reaches
            // the victim twice writes it twice. The heading+step
            // channel could carry one visit and no z at all; the ring
            // column above has already resolved every visit into
            // `hp`/`hyaw`, so hand that over instead. (This is the
            // `min` the Q7 note said would "come straight back out".)
            let abs = hseized
                || (hmid.is_some() && !no_mc2_ww_midring_abs())
                || (hvisited && !no_mc2_ww_tail_publish());
            self.player_whirl = crate::engine::features::PlayerWhirl {
                armed: true,
                heading: hmid.unwrap_or(0),
                step: if abs { 0 } else { 96 },
                grab: if abs {
                    Some((hp.0, hp.1, hp.2, hyaw))
                } else {
                    None
                },
                from: (ctx.px, ctx.py, ctx.pz, ctx.pyaw & 0x7FF),
                act80: hact80,
                // ⚠ ONE CRANK PER *PUBLISHED* SEIZURE, WHICH IS THE
                // CHANNEL'S ARITY, NOT A CAP ON RETAIL. The grab
                // family resolves every repeat visit into `hp`/`hyaw`
                // and publishes an ABSOLUTE pose, so a double
                // inner-lift tick carries both cranks. The MID RING
                // still publishes one heading+step pair (dig W4's
                // pinned arithmetic), so a double mid-ring visit is
                // only half-representable: the second 96-step is
                // dropped, and carrying its crank alone would be a
                // two-visit bank on a one-visit position. mc2l1 t=272
                // is exactly that tick — retail seized TWICE (two
                // 96-steps: y lands on 49844, one step lands 49748;
                // the recorded `word_0x30_48` 1052 is the SECOND
                // bearing while the port's single visit yields ~1035)
                // — and the port's ring walk does reach the block
                // twice. See the dig note: the fix is to publish the
                // resolved absolute pose whenever the mid ring is
                // visited more than once, at which point this `min`
                // comes straight back out.
                // ⚠ STILL `min(1)` ON THE MID RING, AND NOT BECAUSE OF
                // THE CHANNEL: `recover_pair_mc2` un-cranks the
                // recorded `roll_acc` by exactly ONE 28 before
                // inverting the stick filter (mgc-formats
                // `recover.rs`, `let crank = |acc| ... 28`), so a
                // second crank the port applies is one the recovery
                // never removed and lands 28 high. The extra visits
                // are absorbed into the recovered cursor; the POSE is
                // what the port has to carry, and `abs` now does.
                bumps: if hseized || mc2_ww_crank_count_law() {
                    hcrank
                } else {
                    hcrank.min(1)
                },
            };
        }
        // The player arm — the tornado SWAY (retail `sub_33340`'s
        // wizard branch, EF:24296: the human [class 3, model 0] is
        // swirled at yaw-step 56 and dragged toward the eye). The full
        // grab / lift / camera-roll takeover is the deferred FlightVerb
        // seam; the observable "the funnel drags you in" rides the
        // `player_knock` channel like the flood shove — a pull toward
        // the eye bent ~45° tangentially so it spirals inward rather
        // than sucking straight through (deliberate approximation).
        // Retail's whirlwind sways the wizard, it does not chip HP.
        // Same-owner gate (`sub_33810` case 1, EF:24473: `a2x->id ==
        // a1x->id → return 0`) — your OWN whirlwind never sways you.
        let pd = Self::isqrt(Self::dist2_sq(ex, ey, ctx.px, ctx.py) as u32) as i32;
        // RETAIL'S FAR BAND. `sub_33340`'s victim body (EF:24306,
        // `NETHERW.EXE` 0x57c41 `cmp $0x310000,%eax`) splits on
        // `EuclideanDistXY(eye, victim) >= 3211264` (d >= 1792) and in
        // that arm does NOTHING to an UNGRABBED victim: `v30` stays 0,
        // so the closing `MoveEntity_57FA0(..., 0)` is a no-op, and
        // nothing in the function ever writes `moveBoost_0x1E_30` —
        // retail's own knock register — at all. The port's human arm
        // reached 3328 with no band structure, arming `player_knock`
        // (a RECORDED, GRADED, decaying channel retail holds at 0)
        // about four ticks early and 1.9x too far out.
        let far_skip = (Self::dist2_sq(ex, ey, ctx.px, ctx.py) as i64) >= 3_211_264;
        let pd2 = Self::dist2_sq(ex, ey, ctx.px, ctx.py) as i64;
        if !human_law
            && pd < 3328
            && !far_skip
            && pd2 >= 0x40000
            && !no_mc2_ww_midring()
            && id != crate::mc1::mobs::PLAYER_TARGET
        {
            // ⭐⭐⭐ THE MID RING IS A POSE SEIZURE, NOT A KNOCK.
            // `v33 >= 0x40000 && !(byte[3] & 0x10)` (EF:24350-56) is
            // four writes on the victim and nothing else: `v14 =
            // (tan2(eye, victim) + 591) & 0x7FF` into BOTH
            // `word_0x30_48` and `yaw_0x1C_28`, `v30 = 96`, and — for
            // the HUMAN alone (`v40 = class == 3 && !model`,
            // EF:24345-49) — `roll_0x155_341 += 28` while it is under
            // 256 plus `actSpeed_0x82_130 = 80`. The move that
            // follows is `MoveEntity_57FA0(&pred, word_0x30_48, 0,
            // v30)` + `CopyEntityPosition_57CF0`, a direct POSITION
            // write; `moveBoost_0x1E_30` is never touched anywhere in
            // `sub_33340`, so the old `player_knock` shove (with its
            // invented +45° bias and its distance-ramped magnitude)
            // was writing a recorded, graded, decaying lane retail
            // holds at 0. The seizure rides
            // [`crate::engine::features::PlayerWhirl`] instead, and
            // the carpet's own walk slot applies it — see
            // `World::step_player_flight_mc2` for the mc2l1 t=269
            // arithmetic.
            self.player_whirl = crate::engine::features::PlayerWhirl {
                armed: true,
                heading: Self::angle_between(ex, ey, ctx.px, ctx.py).wrapping_add(591) & 0x7FF,
                step: 96,
                grab: None,
                from: (ctx.px, ctx.py, ctx.pz, ctx.pyaw & 0x7FF),
                act80: true,
                // Dig Q6's crank is the OTHER half of the same `v40`
                // block, so it rides this ladder rung too — under
                // `MGC_NO_MC2_WW_HUMAN_GRAB=1` the mid ring still
                // cranks.
                bumps: 1,
            };
        } else if !human_law && pd < 3328 && !far_skip && id != crate::mc1::mobs::PLAYER_TARGET {
            let toward = Self::angle_between(ctx.px, ctx.py, ex, ey);
            let dir = (toward as i32 + 256) as u16 & 0x7FF; // +45° spiral bias
            // Stronger closer (0..128 across the funnel), clamped to
            // the knock channel's band and never overshooting the eye.
            let mag = ((((3328 - pd) << 8) / 3328) << 7 >> 8).clamp(8, 80).min(pd);
            self.player_knock = (dir, mag as i16);
            // THE HEADING. Retail's victim block writes `yaw_0x1C_28`
            // on EVERY arm, and the wizard's step is `v38` = 56 —
            // `v40 = (class == 3 && !model)` picks 56 over the 204
            // creatures get (EF:24294-99), and the same 56 lands in
            // the far-grab, near-grab and inner-lift arms alike. Only
            // the MID RING (`d2 >= 0x40000`, not yet grabbed) sets an
            // absolute heading instead: the tangent `bearing + 591`
            // (EF:24350-56), which is what turns a straight fall
            // toward the eye into the spiral. The port shoved the
            // flyer and never touched its facing, so a tornado threw
            // you around while you kept staring the way you came in.
            //
            // The grab/lift/camera-roll takeover is still the
            // deferred FlightVerb seam — the spin rides the pose
            // channel on its own, which is what the report is about.
            let d2 = Self::dist2_sq(ex, ey, ctx.px, ctx.py) as i64;
            self.player_spin.0 = if d2 >= 0x40000 {
                let tangent = Self::angle_between(ex, ey, ctx.px, ctx.py).wrapping_add(591) & 0x7FF;
                // Absolute in retail; delivered as the delta that
                // reaches it, since the pose channel carries turns.
                (tangent as i16 - (ctx.pyaw & 0x7FF) as i16).rem_euclid(2048)
            } else {
                56
            };
        }
        // The grab-pass batch XP (sub_33340 EF:24407).
        if hits != 0 && id == crate::mc1::mobs::PLAYER_TARGET {
            self.mc2_cast_xp.0.push((id, 21, hits as i32));
        }
    }

    /// `sub_33710` (EF:24416) — the every-8th-tick CONTACT pass
    /// against the list builder (EF:39964-40075): `dword_38527` is
    /// the class-10
    /// MODEL-45 list ⇒ pass 1 mails overlapping village BUILDINGS
    /// (sub_11900 ch0, EF:24428-24430 — no owner gate); pass 2 =
    /// CASTLES (the class-3 list, model 2): the 30-tick shake
    /// (word_0x30_48 → f50), owner stamp (word_0x26_38 → f40) and
    /// the subSpell mail — also ungated (your own castle takes it).
    /// Overlap = CompareAxisWithShift_10750 (XY-only — the shared
    /// [`Gen::mc2_overlap_xy`]). The `sub_6D8B0(id, 0x15, 2n)`
    /// report is emitted by the spell-XP column.
    fn mc2_whirlwind_contact(&mut self, i: usize) {
        if self.ent[i].f63 & 7 != 0 {
            return;
        }
        let (id, amt) = (self.ent[i].id24, self.ent[i].f140 as u32);
        let mut hits: Vec<(usize, bool)> = Vec::new();
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if j == i || c.flags & 0x400 != 0 {
                continue;
            }
            let castle = c.class64 == 3 && c.model65 == 2 && c.act_life >= 0;
            let building = c.class64 == 10 && c.model65 == 45;
            if (castle || building) && self.mc2_overlap_xy(i, j) {
                hits.push((j, castle));
            }
        }
        let mut castles_hit = 0i32;
        for (j, castle) in hits {
            // ⭐⭐ TWO ARMS, TWO DIFFERENT WRITE PROTOCOLS, NEITHER OF
            // THEM THE AREA ONE. `sub_33710` bills the BUILDING list
            // through `sub_11900` (EF:24429 — the SINGLE/INVERSE
            // protocol, `if (src) amt = a4; else amt += a4`) and the
            // CASTLE list through an OPEN-CODED, UNCONDITIONAL
            // accumulate (EF:24438-40: `jx->dword_0x5E_94 +=
            // subSpellIndex; … word_0x62_98 = id`, with no source
            // test at all — a THIRD protocol, and the only site in
            // the whirlwind that has it). Both had been on
            // [`Gen::mail_write`], whose branches are the inverse of
            // `sub_11900` and which overwrites a consumed box where
            // the castle arm accumulates.
            if castle {
                self.ent[j].f50 = 30;
                self.ent[j].f40 = i as u16;
                castles_hit += 1;
            }
            if no_whirlwind_single_bill() {
                self.mail_write(MailTarget::Pool(j), 0, amt, id);
            } else if castle {
                let m = &mut self.ent[j].mail[0];
                m.0 = m.0.wrapping_add(amt); // :24438, unconditional
                m.1 = id; // :24440
            } else {
                self.mail_write_single(MailTarget::Pool(j), 0, amt, id);
            }
        }
        // The contact-pass batch XP: +2 per CASTLE struck
        // (sub_33710 EF:24444, `v1 += 2` per castle).
        if castles_hit != 0 && id == crate::mc1::mobs::PLAYER_TARGET {
            self.mc2_cast_xp.0.push((id, 21, 2 * castles_hit));
        }
    }

    /// `sub_338D0` (EF:24518) — teardown: clear every nearby
    /// victim's grab/stop latches over the radius-12 disc, end the
    /// wind loop (sound 49 stops with the emitter), despawn the head
    /// and all 11 nodes down the f54 chain.
    fn mc2_whirlwind_teardown(&mut self, i: usize, ctx: &MobCtx) {
        // Cell center rounds: `(pos + 128) >> 8` (EF:29527-28 fissure,
        // :24273-74 lift, :24531-32 teardown; truncation would shift
        // the disc half a tile on the high side).
        let (cx, cy) = (
            (self.ent[i].x.wrapping_add(128) >> 8) as i16,
            (self.ent[i].y.wrapping_add(128) >> 8) as i16,
        );
        // ⭐ THE HUMAN IS ON THE SAME DISC (dig Q7). Retail's release
        // sweep walks the tile chains, and retail's wizard is a chain
        // member; ours is not, so the human's own pool record — where
        // [`World::mc2_whirlwind_lift`] banks his `byte[3] & 0x10` and
        // `byte[1] & 8` — has to be swept by his LIVE pose instead.
        // Without it a funnel that expires while holding him leaves
        // the latch armed forever and the next funnel resumes mid-grab.
        {
            let cs = self.mc2_pinned.0 as usize;
            if !no_mc2_ww_human_grab() && cs != 0 && cs < self.ent.len() {
                let hx = ((ctx.px >> 8) as u8) as i16;
                let hy = ((ctx.py >> 8) as u8) as i16;
                let inside = self.ring_cells(0, 12).into_iter().any(|(dx, dy)| {
                    (cx.wrapping_add((dx as i8) as i16)) as u8 == hx as u8
                        && (cy.wrapping_add((dy as i8) as i16)) as u8 == hy as u8
                });
                if inside {
                    self.ent[cs].flags &= !(F_GRABBED | super::mobs::F_STOP);
                }
            }
        }
        for (dx, dy) in self.ring_cells(0, 12) {
            let tx = (cx.wrapping_add((dx as i8) as i16)) as u8;
            let ty = (cy.wrapping_add((dy as i8) as i16)) as u8;
            let mut j = self.map_entity[crate::engine::features::tile(tx, ty)] as usize;
            while j != 0 {
                let next = self.ent[j].next20 as usize;
                self.ent[j].flags &= !(F_GRABBED | super::mobs::F_STOP);
                j = next;
            }
        }
        let mut n = i;
        loop {
            self.ent[n].flags |= 0x400;
            let next = self.ent[n].f54 as usize;
            if next == 0 || next == n {
                break;
            }
            n = next;
        }
    }

    /// `sub_33E20` (EF:24817) — the (10,25) tick: life-- /f26++;
    /// while alive, ONE latched `sub_10C80(type 3, byte_0x46_70)`
    /// burst — the channel-3 payload is the steal TIER INDEX in f71
    /// (EF:24829-31), NOT an amount and NOT subSpell; the victims'
    /// `sub_61050` re-reads `SPELLS[13]` from it. A hit zeroes life
    /// (despawn next tick). The area write is `sub_10C80`'s class-3
    /// arm (EF:4034-60): see `Gen::area_write`'s MC2 ch3/ch4 branch.
    pub(crate) fn mc2_blast25_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].f26 += 1;
        self.ent[i].act_life = life;
        if life >= 0 {
            if self.ent[i].flags & 2 == 0 {
                self.ent[i].flags |= 2;
                let amt = self.ent[i].f71 as u32;
                if self.area_write(i, 3, amt, ctx, false, false) != 0 {
                    self.ent[i].act_life = 0;
                }
            }
        } else {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_33D80` (EF:24787) — the (10,23) tick: ONE latched
    /// `sub_10C80(type 0, 25)` burst + sound 24, then life pinned to
    /// 1 (one more visible tick). The `sub_6D8B0(id, 7, hits)`
    /// spellbook report is emitted by the spell-XP column.
    pub(crate) fn mc2_blast23_tick(&mut self, i: usize, ctx: &MobCtx) {
        // `v1 = life; dword_0x10_16++; life = v1-1; if (v1 >= 0)` —
        // the OLD life gates, and f26 counts up EVERY tick
        // (EF:24789-94; a post-test runs one tick short).
        let old_life = self.ent[i].act_life;
        self.ent[i].f26 += 1;
        self.ent[i].act_life = old_life - 1;
        if old_life >= 0 {
            if self.ent[i].flags & 2 == 0 {
                let amt = self.ent[i].f140 as u32;
                let hits = self.area_write(i, 0, amt, ctx, false, false);
                // Lightning burst batch XP (EF:24802).
                if hits != 0 && self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
                    self.mc2_cast_xp.0.push((self.ent[i].id24, 7, hits as i32));
                }
                self.snd(24, i);
                self.ent[i].act_life = 1;
                self.ent[i].flags |= 2;
            }
        } else {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_32880` (EF:23834) — the (10,17) meteor tick: sound 30 +
    /// the once-latch (dword |= 0x10002) on the first tick; the quad
    /// grows with the ring counter (`ShiftRot((768*f26 - 5*sign)>>2,
    /// 512)`); `sub_10C80(type 0, subSpell/maxLife)` = 300/tick (the
    /// kind-9 spellbook report is emitted by the spell-XP column);
    /// then ONE RING of
    /// (10,0) fire children at ring f26 — jittered (2 RNG each, cell
    /// pitch 160), id+yaw inherited, `dword |= 0x10080` (byte[0]
    /// bit7 + byte[2] bit0 — the children are DAMAGE-SUPPRESSED
    /// visuals, the fire tick's 0x1_0000 gate), quad (512,512); the
    /// ring cycles `(f26+2) % 11`.
    pub(crate) fn mc2_meteor_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].act_life = life;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2 | 0x1_0000;
            self.snd(30, i);
        }
        let ring = self.ent[i].f26 as i32;
        let grown = 768 * ring;
        // ⚠ `my_sign32` is −1/0, NEVER +1 (engine_support.cpp:2962) —
        // so EF:23864's `- my_sign32(768*ring) * 5` is a no-op on the
        // ONLY branch the ring counter ever takes (it cycles 0..10 via
        // `(f26+2) % 11`) and an ADD of 5 on the negative one. Reading
        // it as a signum cost the quad 2 units per ring step: mc2l3
        // t=1341 slot 163 at ring 2 wants 768*2 >> 2 = 384, and the
        // spurious −5 published 382 in BOTH the apitch and aroll lanes.
        let shift = (grown - 5 * if grown < 0 { -1 } else { 0 }) >> 2;
        self.mc2_shift_rot(i, shift as u16, 512);
        let amt = (self.ent[i].f140 / self.ent[i].max_life as i32) as u32;
        let hits = self.area_write(i, 0, amt, ctx, false, false);
        // Meteor batch XP (sub_32880 EF:23871).
        if hits != 0 && self.ent[i].id24 == crate::mc1::mobs::PLAYER_TARGET {
            self.mc2_cast_xp.0.push((self.ent[i].id24, 9, hits as i32));
        }
        let (px, py, pz, id, yaw) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30)
        };
        for (dx, dy) in self.ring_cells(ring, ring) {
            let d = self.ent_rand(i);
            let nx = (px as i32 - 96 + 160 * (dx as i8) as i32 + (d % 0x81) as i32 - 64) as u16;
            let d = self.ent_rand(i);
            let ny = ((d % 0x81) as i32 + 160 * (dy as i8) as i32 + py as i32 - 96 - 64) as u16;
            if let Some(c) = self.mc2_spawn_fire(nx, ny, pz) {
                {
                    let e = &mut self.ent[c];
                    e.id24 = id;
                    e.f30 = yaw;
                    e.flags |= 0x1_0080;
                    e.f26 = 0;
                }
                self.mc2_shift_rot(c, 512, 512);
            }
        }
        self.ent[i].f26 = ((ring + 2) % 11) as i16;
    }

    /// `sub_32530` (EF:23694) — the (10,15) fire-trail tick: the
    /// water counter (`sub_104A0 & 1` → f26++, else --), death on
    /// life < -1 OR 8 accumulated water ticks; ONE RNG wander
    /// (yaw += r%0x5B - 45), advance 256, drop a (10,11→19) spray
    /// (fov copied, life 10, word_0x26_38 = 15, id inherited).
    pub(crate) fn mc2_fire_trail_tick(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        if self.on_water(x, y) {
            self.ent[i].f26 += 1;
        } else if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1;
        }
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life < -1 || self.ent[i].f26 > 8 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let d = self.mc2_rand(i);
        let yaw = ((d % 0x5B) as i32 + self.ent[i].f30 as i32 - 45) as u16 & 0x7FF;
        self.ent[i].f30 = yaw;
        let mut pos = (x, y, self.ent[i].z);
        Self::polar_step(&mut pos, yaw, 0, 256);
        {
            let e = &mut self.ent[i];
            e.x = pos.0;
            e.y = pos.1;
            e.z = pos.2;
        }
        let (pitch, roll, fov, id) = {
            let e = &self.ent[i];
            (e.f80, e.f82, e.f84, e.id24)
        };
        // The trail lays a child SCORCH RING (10,11) — the earth-CARVE
        // (`sub_31FB0` digs the disc −3), NOT the (10,19) ground-fire
        // SPRAY. The spray is a fire effect that itself spews (10,14)
        // smoke puffs every odd tick, so a trail dropping one per tick
        // over its 128-life would exhaust the pool. Same
        // (10,11)-vs-(10,19) confusion as the cave column
        // (docs/spell-audit/quake-family.md §Earthquake). f40=15 keys
        // the ring's Earthquake-XP branch.
        if let Some(s) = self.mc2_spawn_scorch_ring(pos.0, pos.1, pos.2) {
            let e = &mut self.ent[s];
            // All THREE pose fields copy (EF:23719-21) — the ring
            // tick reads f80 (pitch>>8) for its carve radius (f84
            // alone digs the default disc, not the trail's radius 3).
            e.f80 = pitch;
            e.f82 = roll;
            e.f84 = fov;
            e.act_life = 10;
            e.f40 = 15; // word_0x26_38 → Earthquake XP row
            e.id24 = id;
        }
    }

    /// `sub_4FA60` (EF:36292) — the (10,32) RIVER HEAD, the worker the
    /// (10,31) author chain spawns per leg (`sub_487D0`, EV:5558):
    /// action 0x22, actSpeed 256, default width f71 = 2, maxLife 0,
    /// untargetable (byte[0] bit3 clear), NOT map-registered, no
    /// sprite. The chain painter overwrites yaw, life (= leg length
    /// in tiles) and the width.
    pub(crate) fn mc2_spawn_river_head(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 0x20;
            e.tick70 = 0x22;
            e.max_life = 0;
            e.f130 = 256;
            e.f71 = 2;
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        Some(i)
    }

    /// `sub_344A0` (EF:25052) — the (10,32) action-0x22 tick, the
    /// fire trail's AUTHORED twin: life--, death on life < 0 or a
    /// class-0 (nibble-0) cell under it (`sub_104A0 & 1`); else drop
    /// ONE (10,11) SCORCH RING here (fov + id inherited, `life =
    /// byte_0x46_70` = the river WIDTH: 2/6/16/32) and advance
    /// actSpeed (256 = one tile) along the yaw. No RNG, no sound.
    ///
    /// ⭐ THIS IS THE CAVE LAVA. The ring's own tick (`sub_31FB0`)
    /// digs its growing disc −3 every tick for `life` ticks, one ring
    /// per tile of the leg, and adjacent rings OVERLAP — so a width-16
    /// leg carves a ~4-wide channel to floor 0 with a ~5-cell graded
    /// rim, and a cave floor at 0 retiles to the lava types 36/37/39.
    /// mc2:15's two (10,31) rectangles (slots 50-54 and 268-272) ARE
    /// its lava moats: retail record 0 holds 1,747 floor-0 cells, the
    /// port before this arm 887, every missing one inside those two
    /// rings (round 109). docs/traces/mc2-terrain-author-painters.md
    /// §3.4 called the consumer a stub because it read `4A190(.., 10,
    /// 32)` as hex (10,50) — the argument is DECIMAL: (10,32) = 0x20.
    pub(crate) fn mc2_river_head_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        if life < 0 || self.on_water(x, y) {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (fov, id, width, yaw, speed) = {
            let e = &self.ent[i];
            (e.f84, e.id24, e.f71, e.f30, e.f130)
        };
        if let Some(s) = self.mc2_spawn_scorch_ring(x, y, z) {
            let e = &mut self.ent[s];
            e.f84 = fov;
            e.id24 = id;
            e.act_life = width as i32;
        }
        let mut pos = (x, y, z);
        Self::polar_step(&mut pos, yaw, 0, speed as i16);
        let e = &mut self.ent[i];
        e.x = pos.0;
        e.y = pos.1;
        e.z = pos.2;
    }

    /// `sub_32F40` (EF:24095) — the (10,19) ground-fire-spray tick:
    /// while alive, walk the radius-0 splat TEMPLATE — retail loops
    /// `AddE7EE0x_10080(0, 0)` = ring 0 (4 cells, last dropped as the
    /// stop code → 3 emission cells; EF:24112-40), NOT a single center
    /// cell. For EACH cell: a ~50% gate roll, two jitter rolls (offset
    /// by the cell's `192 * (dx, dy)`), and on ODD life ticks a 4-puff
    /// ring of (10,14) smoke (yaw start `(life/2 & 1) << 8`, step 0x200
    /// to 0x800, id inherited); z snaps to terrain. On death, release
    /// the word_0x33 singleton (`plume`, latched by morph.rs's
    /// summit-18). `sub_10C80(ch0, 200)` EVERY tick including the
    /// despawn tick. The single-cell port formerly under-produced the
    /// column's smoke by ~3x — the volcano (10,14) missing family.
    pub(crate) fn mc2_fire_spray_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life >= 0 {
            self.ent[i].f26 = 0;
            let (px, py, pz, id) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z, e.id24)
            };
            let odd = self.ent[i].act_life & 1 == 1;
            let v10_start = ((self.ent[i].act_life / 2) & 1) << 8;
            for (dx, dy) in self.ring_cells(0, 0) {
                let d = self.ent_rand(i);
                if 2 * ((d % 0x9D) as i32 / 79) - 1 <= 0 {
                    continue;
                }
                let d = self.ent_rand(i);
                let jx = (px as i32 - 96 + 192 * (dx as i8) as i32 + (d % 0x81) as i32 - 64) as u16;
                let d = self.ent_rand(i);
                let jy = (py as i32 - 96 + 192 * (dy as i8) as i32 + (d % 0x81) as i32 - 64) as u16;
                if odd {
                    let mut v10 = v10_start;
                    while v10 < 0x800 {
                        if let Some(p) = self.mc2_spawn_smoke_particle_for(14, jx, jy, pz) {
                            self.ent[p].id24 = id;
                            self.ent[p].f30 = v10 as u16;
                        }
                        v10 += 0x200;
                    }
                }
            }
            // EF:24141 — the alive branch's LAST statement is an
            // UNCONDITIONAL `z = getTerrainAlt(pos)`: the column
            // rides the dome's own terraform every alive tick
            // (mc2l24 slot 140: z == height×32 through the whole
            // raise AND the crater cut). The old strict-gated freeze
            // was a stale pristine-plane workaround — the format-2
            // measured channel carries the raised summit, and five
            // sibling newborns at the vent pin the port's ground to
            // retail's exactly (t=14: 4384 on both sides).
            {
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                self.ent[i].z = self.ground_z(x, y) as i16;
            }
        } else {
            self.ent[i].flags |= 0x400;
            // `D41A0_0.word_0x33 = 0` (EF:24148) — release the spray
            // singleton (latched by the summit-18 eruption,
            // morph.rs). Unconditional like retail: the latch kills
            // the previous spray on re-latch, so at most one is ever
            // alive. WITHOUT this, a stale `plume` outlives the
            // spray, and the next eruption's "kill the previous
            // column" write lands on whatever entity RE-USED the
            // slot — a silent arbitrary kill.
            self.plume = 0;
        }
        let amt = self.ent[i].f140 as u32;
        self.area_write(i, 0, amt, ctx, false, false);
    }

    /// `sub_38D80` (EF:28349) — the (10,54) MANA-MAGNET aura (retail
    /// `AddAuxiliary_50500`, `dword_0x10_16 = 0xC40000`): life-- (< 0
    /// → despawn), then over the SQUARED range 0xC40000 (≈14 tiles)
    /// drag every unowned MANA SPHERE toward the eye. Retail stamps
    /// each sphere a homing target (`word_0x7A_122` = this aura) +
    /// pull speed (`word_0x76_118 = min(dist, 42)`) which the ball
    /// tick (EF:26369) flies in, merging coincident balls into one.
    ///
    /// ⚠ The SCAN SOURCE is retail's (the tick-top `dword_38523`
    /// chain — see the loop). What is still collapsed is the WRITE:
    /// [`Self::ball_tick`] already consumes `dest_x/dest_y` as a
    /// decaying drift AND merges overlapping balls, so the aura writes
    /// the pull velocity onto each ball's dest exactly like
    /// [`Self::magnet_tick`] instead of stamping retail's speed word
    /// and letting the sphere derive its own heading (deliberate —
    /// and note that it is why the port never writes the sphere's
    /// `yaw`, which retail's own consume-side does).
    /// Only the TARGET half (`word_0x7A_122`) keeps a
    /// field home of its own, the aura claim map, because retail's
    /// handshake is load-bearing on BOTH sides: the aura re-stamps
    /// EVERY tick (`if (!w7A)`, EF:28364) and the sphere clears the
    /// stamp at the head of its own tick (EF:26109), latching the
    /// `v35` kick that drags a sphere whose settle counter has run
    /// out. Claim in, claim out, once per tick — a claim that
    /// outlives its tick silently retires the sphere from the aura's
    /// scan for good.
    pub(crate) fn mc2_aura_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        // dword_0x10_16 = (tile range << 8)² — f26 holds the tile
        // range (ctor default 14; disposition spawn overrides it from
        // the THING's stageTag, sub_4A310).
        let r = (self.ent[i].f26 as i32) << 8;
        let range_sq = r * r;
        let (ax, ay) = (self.ent[i].x, self.ent[i].y);
        // ⭐⭐⭐ THE SCAN IS `dword_38523`, THE TICK-TOP SPHERE CHAIN —
        // NOT THE LIVE POOL (EF:28362). This was a registered
        // approximation ("a pool slot-order list standing in for
        // retail's `dword_38523` list") until mc2l3 t=9816 demanded
        // it, and **membership sampled ONCE AT THE TOP is the whole
        // point**: a sphere born MID-TICK is not in this tick's chain,
        // so retail cannot pull it until the NEXT tick.
        //
        // mc2l3 t=9816 is that tick exactly — the player's cast borns
        // a (10,54) aura plus eleven (10,39) spheres in one frame, and
        // the aura dispatches at its own ascending slot afterwards.
        // Retail's newborn spheres sit still (`dest` 0/0, `yaw` 0);
        // the port's pool walk saw them immediately and pulled them a
        // tick early, so **the port's whole sphere grid was one tick
        // ahead of retail's forever after** — measured: the port's
        // t=9816 x/y/dest_x/dest_y for slot 170 ARE retail's t=9817
        // values. Same family as the ball chain's own law (mc1l4
        // t=5377, the mid-tick ball the tick-top scan cannot hold).
        //
        // ⚠ The chain's membership is retail's verbatim (models 39,
        // 40, 57 for MC2), but the model filter below is KEPT as it
        // was: retail's `sub_38D80` has no model test and therefore
        // pulls the (10,40) claim totem too, which the port has never
        // done. That is a separate pre-existing residual, and changing
        // the scan source and the pulled set in one step would make
        // neither attributable.
        for k in 0..self.ball_chain.visible_len() {
            let j = self.ball_chain.list[k] as usize;
            let c = &self.ent[j];
            // ⭐ THE MAGNET DOES NOT TEST THE REAP BIT — SOFT KILL IS
            // NOT A FREE. `sub_38D80`'s whole loop body is
            // `if (!ix->str_0x5E_94.word_0x7A_122)` (EF:28364): no
            // 0x400 test, no liveness test, no class or model test
            // either. A sphere flagged EARLIER in the same pass keeps
            // its class, model and chain links for the rest of the
            // tick, so retail's magnet still stamps it and the sphere
            // takes ONE MORE full pull step on its dying tick.
            // mc2l6-rsg t=3555 slot 743: the ball is already 0x400
            // when aura 569 dispatches, retail pulls it (−42,+3) —
            // MoveEntity's own floor of `(42·SIN[1519])>>16` — while
            // the port's gate skipped it and left it riding the
            // PREVIOUS tick's friction residue (−41,+2), the last
            // step of its life off by one unit on both axes.
            // ⚠ The class/model filter stays: it is the documented
            // pre-existing residual (retail pulls the (10,40) totem
            // too), and changing membership and the reap gate in one
            // step would make neither attributable.
            if c.class64 != 10 || !matches!(c.model65, 39 | 57) {
                continue;
            }
            // The claim handshake (EF:28364): only an UNCLAIMED ball
            // takes the pull; the ball's tick clears the claim after
            // consuming it. First aura in slot order keeps the ball.
            // ⭐ RETAIL'S GUARD IS ON `w7A`, WHICH THE PORT HOMES TWICE.
            // `sub_38D80` EF:28364 is `if (!ix->str_0x5E_94.word_0x7A_122)`
            // — ONE cell (line verified verbatim by the main session).
            // The port splits that cell across `mail[4].1` and this
            // claim map, so testing the map alone let an aura stamp a
            // sphere that already carried a ch4 source; the sphere then
            // ran BOTH of `ball_tick`'s intake arms and the aura arm
            // clobbered the mail arm's (correct) aim. mc2l22 t=8510:
            // the mail arm computed 1497 = retail's exact bearing to
            // 985, the aura arm overwrote it with 439 = the bearing to
            // 253. Ledger ROUND 99 dig 99-7.
            if self.mc2_aura_claim.0.contains_key(&(j as u16))
                || (aura_claim_w7a_guard_law() && self.ent[j].mail[4].1 != 0)
            {
                continue;
            }
            let d2 = Self::dist2_sq(ax, ay, c.x, c.y);
            if d2 >= range_sq {
                continue;
            }
            self.mc2_aura_claim.0.insert(j as u16, i as u16);
            // Pull speed = min(linear distance, 42) — retail's radix_3d
            // cap; it eases to 0 at the eye so the merged ball settles.
            let speed = (Self::isqrt(d2 as u32).min(42)) as i32;
            if !no_mc2_aura_stamp() {
                // ⭐⭐⭐ THE STAMP IS THE WHOLE WRITE (see
                // [`no_mc2_aura_stamp`]): `@0x76 = speed` (a DWORD,
                // `0x5d60a mov %ecx,0x76(%ebx)`) and `@0x7A = aura
                // index` (`0x5d60d mov %ax,0x7a(%ebx)`), which are the
                // port's `mail[4]` amount/source pair. The sphere then
                // derives `yaw` and `axis_0x9A` from its OWN position
                // at its OWN slot next tick (EF:26097-26110), which
                // `ball_tick`'s ch4 intake already does verbatim.
                // Writing the velocity HERE ran the polar step one
                // pass early, off the sphere's pre-mover position.
                self.ent[j].mail[4] = (speed as u32, i as u16);
                continue;
            }
            // `angle_of` returns 0..=2048 (2048 = the full-turn wrap);
            // mask to the table's 0..2047 like `advance` does, or a
            // ball at the exact diagonal panics SIN[2048] (len 2048).
            let dir = (Self::angle_between(c.x, c.y, ax, ay) & 0x7FF) as usize;
            let vx = ((speed * crate::mc1::tables::SIN[dir]) >> 16) as i16;
            let vy = (-((speed * crate::mc1::tables::COS[dir]) >> 16)) as i16;
            self.ent[j].dest_x = vx as u16;
            self.ent[j].dest_y = vy as u16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::F_GRABBED;
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes};
    use crate::engine::world::conformance::import_ent_mc2;
    use crate::mc1::combat::MailTarget;
    use crate::verbs::VerbSet;
    use mgc_formats::mgcr::RetailEntMc2;

    /// Flat 100-height OPEN world — the mailbox protocols are pure
    /// record arithmetic, so the terrain only has to exist.
    fn flat_gen() -> Gen {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Ring 0 is the CENTRE cell; the outer rings are parked on a
        // far offset so the disc walk visits exactly one tile and the
        // billing assertion counts one letter per visit.
        let mut rings: Vec<Vec<(u8, u8)>> = (0..32).map(|_| vec![(15u8, 15u8)]).collect();
        rings[0] = vec![(0u8, 0u8)];
        let assets = FeatureAssets {
            rings,
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
        };
        Gen::new(planes, assets, 1, ChassisParams::MC2, VerbSet::MC2)
    }

    /// ⭐⭐⭐ EVERY ARM OF `sub_33340`'s VICTIM BODY FALLS THROUGH THE
    /// SAME TAIL, THE MID RING INCLUDED — so a mid-ring visit writes
    /// an ABSOLUTE, BANDED position, not just a heading.
    ///
    /// The shipped `NETHERW.EXE` settles it by control flow: the four
    /// arms all reach `0x57df8`, and the MID RING's own exit is
    /// `0x57d59 c7 45 d4 60 00 00 00` (`movl $0x60,-0x2c(%ebp)` —
    /// `v30 = 96`) followed by `0x57d60 e9 93 00 00 00`
    /// (`jmp 0x57df8`). The tail is
    /// `0x57e0b call 0x7c7a0` (`MoveEntity_57FA0`),
    /// `0x57e10 mov 0x41b6,%cl` / `test cl,cl` / `je 0x57e45` (the
    /// cave-ceiling clamp, `0x57e26 call 0x35460` = `sub_10C60` less
    /// the victim's own `0x58` fov at `0x57e22`),
    /// `0x57e5e call 0x35440` (`getTerrainAlt_10C40`) feeding
    /// `0x57e6d call 0x7c8e0` (`sub_580E0`, the float band, with the
    /// victim row's `word_0xa`/`word_0xc` read at `0x57e4f`/`0x57e54`)
    /// and finally `0x57e7b call 0x7c4f0`
    /// (`CopyEntityPosition_57CF0`). EF:24375-95.
    ///
    /// ⚠ THIS LAW HAS NO FIXTURE HOME. The carpet's pose lives in
    /// `flight::Mc1State`, out of the pool, so `verify-deltas`' field
    /// channel cannot see it and its POSE CHANNEL is a shadow mover
    /// that never runs a funnel (measured on mc2l24 t=7910..8080: both
    /// channels are byte-identical with the law on and off). Only the
    /// free run sees it. Corpus receipt: `mc2l24` t=7913, whirlwind
    /// head 327 at eye (32406, 29591) mid-ringing the carpet TWICE as
    /// the walk carries it from cell (124,116) to (123,116) —
    /// retail lands (31723, 29697, 1818) yaw 2035, the port landed
    /// (31739, 29792, 1767) yaw 2033 with the heading+step channel
    /// and lands (31722, 29698, 1820) with this one.
    #[test]
    fn the_mid_ring_publishes_an_absolute_banded_pose() {
        let mut g = flat_gen();
        // Two cells in ring 0: the head's own, and one two tiles east
        // — far enough out to be the MID RING (d2 >= 0x40000) while
        // still a cell the disc walk names.
        g.assets.rings[0] = vec![(0u8, 0u8), (2u8, 0u8)];

        let head = g.new_event().expect("head slot");
        let pinned = g.new_event().expect("pinned human seat");
        let (hx, hy) = (40u16 * 256, 40u16 * 256);
        {
            let e = &mut g.ent[head];
            e.class64 = 10;
            e.model65 = 22;
            e.id24 = 159; // NOT the player: your own funnel never sways you
            e.x = hx;
            e.y = hy;
            e.z = 400;
            e.dest_x = hx; // the eye (`axis_0x9A_154x`)
            e.dest_y = hy;
            e.f50 = 400;
            e.f140 = 20;
        }
        {
            let e = &mut g.ent[pinned];
            e.class64 = 3;
            e.model65 = 0;
            e.id24 = crate::mc1::mobs::PLAYER_TARGET;
        }
        g.mc2_pinned = crate::engine::features::Mc2Pinned(pinned as u16);

        // 532 units east of the eye: cell (cx+2, cy), d2 = 283_024,
        // inside 3_211_264 and outside 0x40000 — the mid ring. The z
        // starts BELOW the band's floor so the tail has work to do.
        let (px, py) = (hx + 532, hy);
        let ctx = crate::mc1::mobs::MobCtx {
            px,
            py,
            pz: 0,
            pyaw: 0,
            pmana: 1000,
            pmana_max: 1000,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        };
        g.mc2_whirlwind_lift(head, &ctx);

        let w = g.player_whirl;
        assert!(w.armed, "the mid ring must arm the seizure");
        let (gx, gy, gz, gyaw) = w.grab.expect(
            "the MID RING publishes the RESOLVED ABSOLUTE pose \
             (`CopyEntityPosition_57CF0`, EF:24395). `None` here means \
             the heading+step channel is back, which can carry one \
             visit and no z at all.",
        );
        assert_eq!(w.step, 0, "an absolute payload has nothing left to step");
        assert_eq!(
            gyaw,
            Gen::angle_between(hx, hy, px, py).wrapping_add(591) & 0x7FF,
            "the mid ring's absolute heading (EF:24354-55 / 0x57d51-55)"
        );
        // The 96-unit step along that heading, then the band.
        let mut want = (px, py, 0i16);
        Gen::polar_step(&mut want, gyaw, 0, 96);
        assert_eq!((gx, gy), (want.0, want.1), "one 96-unit swirl step");
        let galt = g.ground_z(gx, gy) as i16;
        assert_eq!(
            gz,
            galt.wrapping_add(crate::flight::Mc2Row::OPEN.clearance),
            "`sub_580E0` (0x57e6d) floors the victim at ground + the \
             row clearance on EVERY visit, not just the grabbed ones — \
             a z of 0 here means the band is back on the GRAB arms only"
        );
    }

    /// ⭐⭐⭐ `CopyEntityPosition_57CF0` RELINKS THE **WIZARD** TOO,
    /// SO HIS SEAT IS RESEEDED INSIDE `sub_33340`'s OWN WALK (round
    /// 125, dig A round 5 — LAW 32; this is its owed unit pin).
    ///
    /// The funnel's victim tail ends in the same
    /// `CopyEntityPosition_57CF0` every pooled victim gets
    /// (`NETHERW.EXE` 0x57e7b `call 0x7c4f0`), and that helper's whole
    /// body is a tile-word compare followed by an UNLINK +
    /// `AddEventToMap` head-insert (0x7c4fe/0x7c505 the compare,
    /// 0x7c50b `jz` the no-relink path, 0x7c50e unlink, 0x7c518
    /// re-add). Retail's carpet is an ordinary linked class-3 record,
    /// so a visit that carries the wizard into another tile makes him
    /// that tile's chain HEAD there and then — his seat is a live
    /// thing for the rest of the walk, not something re-derived once
    /// per turn.
    ///
    /// The port carries him out of pool and tracks the seat as
    /// [`crate::engine::features::PlayerChain`], which used to be
    /// reseeded ONLY at the carpet's own walk slot
    /// (`Gen::player_relink` from `World::adopt_walk_pose`). So after
    /// a funnel visit moved him the seat still named the tile he had
    /// LEFT, and every later arrival at his new one was refused —
    /// mc2l24 t=8033 (head 200): the inner lift puts him on the eye
    /// (30124, 25678) = cell 25717 and `player_chain` still read
    /// (25461, 0), so when victim 146 relinked 25973 → 25717 in FRONT
    /// of him the walker skipped his seat and he lost a third visit
    /// (one more +114 z, one more +56 yaw, one more 128-step).
    ///
    /// ⚠ NO FIXTURE HOME, for the same reason the mid-ring pin above
    /// has none: the carpet's pose and its seat both live outside the
    /// pool, so `verify-deltas` cannot see either. Measured on the
    /// free run only (mc2l24 −3 segments, mc2l1/mc2l30 to END).
    /// `MGC_NO_MC2_WW_HUMAN_SEAT_RELINK=1` must FAIL the last assert.
    ///
    /// The rig is one NEAR-GRABBED visit (`0x57d65 mov ecx,0x80`) in
    /// the funnel's own ring cell, with the wizard parked 6 units
    /// short of that tile's east edge so the 128-unit swirl step
    /// carries him over it.
    #[test]
    fn the_wizards_own_relink_reseats_him_inside_the_funnel_walk() {
        use crate::engine::features::tile;
        let mut g = flat_gen();

        let head = g.new_event().expect("head slot");
        let pinned = g.new_event().expect("pinned human seat");
        let (hx, hy) = (40u16 * 256, 40u16 * 256);
        {
            let e = &mut g.ent[head];
            e.class64 = 10;
            e.model65 = 22;
            e.id24 = 159; // NOT the player: your own funnel never sways you
            e.x = hx;
            e.y = hy;
            e.z = 400;
            e.dest_x = hx; // the eye (`axis_0x9A_154x`)
            e.dest_y = hy;
            e.f50 = 400;
            e.f140 = 20;
        }
        {
            let e = &mut g.ent[pinned];
            e.class64 = 3;
            e.model65 = 0;
            e.id24 = crate::mc1::mobs::PLAYER_TARGET;
            // `byte[3] & 0x10` — the grab latch, banked on the human's
            // own pinned record; already GRABBED, so the visit takes
            // the near arm and its 128-unit step.
            e.flags |= F_GRABBED;
            // `word_0x30_48` — the latched swirl heading the tail
            // steps along. 512 = due EAST.
            e.f50 = 512;
        }
        g.mc2_pinned = crate::engine::features::Mc2Pinned(pinned as u16);

        // 250 units east of the eye: same tile (40, 40) — the ring
        // cell the disc walk names — and only 6 units short of its
        // edge. d² = 62,500, well under the 0x310000 far gate.
        let (px, py) = (hx + 250, hy);
        assert_eq!(
            ((px >> 8) as u8, (py >> 8) as u8),
            (40, 40),
            "rig: his tile"
        );
        // His seat as the carpet's own walk slot would have seeded it.
        g.player_relink(px, py);
        assert_eq!(g.player_chain.cell, tile(40, 40), "rig: the seat pre-walk");

        let ctx = crate::mc1::mobs::MobCtx {
            px,
            py,
            pz: 400,
            pyaw: 0,
            pmana: 1000,
            pmana_max: 1000,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        };
        g.mc2_whirlwind_lift(head, &ctx);

        // Where the near-grab arm leaves him: 128 units along the
        // latched swirl, i.e. over the tile boundary into (41, 40).
        let mut want = (px, py, 400i16);
        Gen::polar_step(&mut want, 512, 0, 128);
        let (ntx, nty) = ((want.0 >> 8) as u8, (want.1 >> 8) as u8);
        assert_eq!(
            (ntx, nty),
            (41, 40),
            "rig: the 128-unit step (0x57d65) must CROSS the tile edge"
        );
        assert_ne!(tile(ntx, nty), tile(40, 40), "rig: a real relink");
        assert_eq!(
            g.player_chain.cell,
            tile(ntx, nty),
            "`CopyEntityPosition_57CF0` (0x57e7b -> 0x7c518 AddEventToMap) \
             makes him the NEW tile's chain head inside the walk — a seat \
             still naming (40, 40) here is the pre-dig behaviour, where \
             every later arrival at his real cell was refused"
        );
    }

    /// ⭐⭐⭐ `sub_11900` AND THE AREA WRITER ARE EXACT INVERSES, AND
    /// THE WHIRLWIND BILLS THROUGH THE FORMER.
    ///
    /// `sub_11900` (EF:4375-88) is
    /// `if (dst->word_0x62_98) dword_0x5E_94 = a4; else += a4`, and
    /// the shipped EXE settles the branch direction — NETHERW.EXE
    /// 0x3611f `cmpw $0x0,0x62(%eax)` / `je 0x36130`, with the
    /// NOT-taken (source PENDING) arm at 0x3612b `mov %edx,0x5e(%eax)`
    /// = ASSIGN and the taken (source CONSUMED) arm at 0x36135
    /// `add %edx,0x5e(%eax)` = ACCUMULATE. The area writer
    /// ([`Gen::mail_write`], `sub_118C0`) is the other way round.
    ///
    /// This is load-bearing because retail's whirlwind disc walk
    /// VISITS ONE VICTIM TWICE on a single pass:
    /// `CopyEntityPosition_57CF0` (EF:24395) relinks the victim
    /// mid-walk, and a victim carried into a tile the ring walker has
    /// not reached yet is lifted, moved AND billed again. Retail's
    /// second letter lands on a PENDING box and OVERWRITES it, so two
    /// 20-point letters bill 20; on the area protocol they bill 40.
    ///
    /// ⚠ THIS LAW HAS NO FIXTURE HOME: `retail_import_mc2` does not
    /// restore mailbox state, so the pair channel is structurally
    /// blind to it (the whole-take census is byte-identical either
    /// way) and only the free run can see it. Hence a unit pin.
    /// Corpus receipt: `mc2l6-rival-spells-galore` t=10110, whirlwind
    /// head 159 lifting rival 378 across cells (25,105)→(25,106) —
    /// retail bills 20 and the rival's regen pins it at max_life
    /// 10000, the port billed 40 and landed on 9980. Free-run horizon
    /// 10,109 → 10,787.
    #[test]
    fn the_whirlwind_lift_bills_on_the_single_protocol_not_the_area_one() {
        let mut g = flat_gen();
        let v = g.new_event().expect("victim slot");

        // Two letters onto one box in a single pass, from one source,
        // exactly as the double visit delivers them.
        g.ent[v].mail[0] = (0, 0);
        g.mail_write_single(MailTarget::Pool(v), 0, 20, 159);
        g.mail_write_single(MailTarget::Pool(v), 0, 20, 159);
        assert_eq!(
            g.ent[v].mail[0],
            (20, 159),
            "sub_11900's second letter must OVERWRITE the pending box, \
             not add to it (EXE 0x3612b assigns while 0x62 is non-zero)"
        );

        // The area protocol is the exact inverse, and doubles it —
        // this is precisely the bug the law removed.
        g.ent[v].mail[0] = (0, 0);
        g.mail_write(MailTarget::Pool(v), 0, 20, 159);
        g.mail_write(MailTarget::Pool(v), 0, 20, 159);
        assert_eq!(
            g.ent[v].mail[0],
            (40, 159),
            "the AREA writer accumulates while a source is pending — \
             if this ever equals the single protocol the two have been \
             collapsed and the whirlwind law is untestable"
        );

        // And the other arm of each: onto a CONSUMED box (source 0)
        // the single protocol accumulates and the area one assigns.
        g.ent[v].mail[0] = (7, 0);
        g.mail_write_single(MailTarget::Pool(v), 0, 20, 159);
        assert_eq!(
            g.ent[v].mail[0],
            (27, 159),
            "sub_11900 accumulates once a reader has cleared the source"
        );
        g.ent[v].mail[0] = (7, 0);
        g.mail_write(MailTarget::Pool(v), 0, 20, 159);
        assert_eq!(
            g.ent[v].mail[0],
            (20, 159),
            "the area writer assigns onto a consumed box"
        );
    }

    /// ⭐⭐⭐ AND THE LAW ITSELF: THE LIFT PASS *CALLS* THE SINGLE
    /// PROTOCOL. The test above pins the two primitives apart; this
    /// one pins WHICH ONE `mc2_whirlwind_lift` reaches for, which is
    /// the thing that was actually wrong.
    ///
    /// The discriminator is a box that already holds a PENDING letter
    /// (`word_0x62_98 != 0`): `sub_11900` OVERWRITES it, the area
    /// writer ADDS to it. One visit is enough to tell them apart, so
    /// the test does not have to stage retail's double visit.
    #[test]
    fn the_whirlwind_lift_call_site_uses_sub_11900() {
        let mut g = flat_gen();
        let head = g.new_event().expect("head slot");
        let victim = g.new_event().expect("victim slot");

        // Cell ORIGIN, not centre: the disc walk rounds to nearest
        // with `(x + 128) >> 8` (EF:24273-74), so a head parked at
        // `tile*256 + 128` scans the NEXT cell along and would walk
        // straight past the victim.
        let (hx, hy) = (40u16 * 256, 40u16 * 256);
        {
            let e = &mut g.ent[head];
            e.class64 = 10;
            e.model65 = 22;
            e.id24 = 159;
            e.x = hx;
            e.y = hy;
            e.z = 400;
            e.dest_x = hx; // the eye
            e.dest_y = hy;
            e.f50 = 400;
            e.f140 = 20; // the amount each letter carries
        }
        {
            let e = &mut g.ent[victim];
            e.class64 = 10;
            e.model65 = 13; // on the lift's victim list
            e.id24 = 378;
            e.x = hx;
            e.y = hy;
            e.z = 400;
            e.flags |= F_GRABBED; // grabbed => the pass bills it
        }
        g.link(victim, hx, hy, 400);

        // A letter already in the box from an earlier writer, still
        // pending (source non-zero) — retail's second lift letter
        // lands on exactly this state.
        g.ent[victim].mail[0] = (200, 999);

        let ctx = crate::mc1::mobs::MobCtx {
            px: hx,
            py: hy,
            pz: 400,
            pyaw: 0,
            pmana: 1000,
            pmana_max: 1000,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        };
        g.mc2_whirlwind_lift(head, &ctx);

        assert_eq!(
            g.ent[victim].mail[0],
            (20, 159),
            "the lift must bill through sub_11900, which OVERWRITES a \
             pending box (EF:24400 is a bare sub_11900 call). Getting \
             (220, 159) means the call site is back on the AREA \
             protocol and every whirlwind hit double-bills."
        );
    }

    /// ⭐⭐⭐ THE `(10,77)` SATELLITE'S SPIN LIVES IN **TWO** WORDS,
    /// AND THE IMPORTER ONLY CARRIED ONE.
    ///
    /// `sub_4F440` (EF:36049-72; NETHERW.EXE 0x73C40) draws ONE spin
    /// rate per orb — `v6 = (rand & 0x3F) + 84` — and files it by
    /// RING (`byte_0x43_67 = i / 5`): ring 0 into `roll_0x20_32`
    /// (0x73CF2 `mov %ax,0x20(%ebx)`, its `fov` left 0), rings 1-4
    /// into `fov_0x22_34` (0x73D5E `mov %ax,0x22(%ebx)`, their `roll`
    /// left 0). `sub_33B20` (EF:24675-79; 0x5835B `add 0x20(%ebx),%ax`
    /// and 0x58366 `mov 0x22(%ebx),%cx`) then steps ONE axis each:
    /// `yaw += roll`, `pitch += fov`. So a ring-0 sphere sweeps a
    /// horizontal circle and a ring-1..4 sphere a vertical one, off the
    /// SAME number.
    ///
    /// [`Gen::mc2_orb_tumble`] ports both steps faithfully, but
    /// `import_ent_mc2` restored only `f34 <- @0x20`; its `f36` arm
    /// listed the m27 spline, the (5,10) pyramid and the (10,78) mine
    /// and gave everything else 0. Every imported pair therefore
    /// handed the 20 ring-1..4 spheres of every orb spin 0 while their
    /// 5 ring-0 siblings stepped correctly — the constellation froze
    /// one tumble behind retail's on the orb's FIRST graded tick and
    /// stayed exactly one behind for its whole life.
    ///
    /// ⚠ NO FIXTURE HOME. A `(10,77)` satellite never bumps its phase
    /// byte (action 0x54 is a NULL dispatch row — `mc2_no_bump_action`,
    /// verify_mc2.rs), so `torn_slots` excludes it from grading on
    /// every pair EXCEPT its birth pair, and on the birth pair both
    /// columns still hold the pristine `sub_4F440` layout with no
    /// tumble applied. The lane is visible only under
    /// `MGC_TEAR_PHASE_LAW=1 MGC_TEAR_NO_BUMP_C10=1`, where it was
    /// 76,300 dirty slot-ticks over 1,287 ticks and 387 slots on
    /// mc2l22 — fixed 301,612 rows of that census's 305,101, with 0
    /// introduced on the default oracle (which itself moved 570 dirty
    /// pairs -> 561). Corpus receipt: the orb born at t=47850, spin 111 —
    /// pair 47851→47852 moves ring-1 slot 822's pitch 512 → 623 and
    /// ring-0 slot 816's yaw 512 → 623; the port moved only 816.
    /// `MGC_NO_ORB_SAT_FOV=1` reverts the import arm.
    #[test]
    fn the_orb_satellite_pitch_spin_is_the_fov_word() {
        // 1. The import homes, ring by ring — retail's own t=47851
        //    records for slots 816 (ring 0) and 822 (ring 1).
        let ring0 = RetailEntMc2 {
            class3f: 10,
            model40: 77,
            yaw: 512,
            pitch: 0,
            roll: 111, // @0x20 — ring 0's spin
            f22: 0,    // @0x22 — unused on ring 0
            b43: 0,
            b44: 0,
            ..Default::default()
        };
        let ring1 = RetailEntMc2 {
            class3f: 10,
            model40: 77,
            yaw: 512,
            pitch: 512,
            roll: 0,  // @0x20 — unused on rings 1-4
            f22: 111, // @0x22 — THE PITCH SPIN
            b43: 1,
            b44: 0,
            ..Default::default()
        };
        let (e0, e1) = (
            import_ent_mc2(&ring0, 816, 0, &|v| v),
            import_ent_mc2(&ring1, 822, 0, &|v| v),
        );
        assert_eq!(e0.f34, 111, "ring-0 spin @0x20 -> f34");
        assert_eq!(e0.f36, 0, "ring-0 has no @0x22 spin");
        assert_eq!(e1.f34, 0, "ring-1 has no @0x20 spin");
        assert_eq!(
            e1.f36, 111,
            "ring-1..4 spin @0x22 -> f36. Reading 0 here is the bug: \
             the importer's f36 arm dropped the (10,77) lane."
        );

        // 2. And the consequence, through the tumble itself.
        let mut g = flat_gen();
        let h = g.new_event().expect("hub slot");
        let s0 = g.new_event().expect("ring-0 slot");
        let s1 = g.new_event().expect("ring-1 slot");
        {
            let e = &mut g.ent[h];
            e.class64 = 10;
            e.model65 = 76;
            e.f30 = 0; // hub yaw
            e.f32 = 0; // hub pitch
            e.f44 = 192; // word_0x2C_44, the ring radius
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 4411;
            e.f54 = s0 as u16;
        }
        for (slot, r) in [(s0, &e0), (s1, &e1)] {
            let e = &mut g.ent[slot];
            e.class64 = 10;
            e.model65 = 77;
            e.f30 = r.f30;
            e.f32 = r.f32;
            e.f34 = r.f34;
            e.f36 = r.f36;
        }
        g.ent[s0].f54 = s1 as u16;
        g.ent[s1].f54 = 0;

        g.mc2_orb_tumble(h);

        assert_eq!(g.ent[h].f30, 22, "hub yaw += 22 (EF:24668)");
        assert_eq!(g.ent[h].f32, 16, "hub pitch += 16 (EF:24671)");
        assert_eq!(g.ent[s0].f30, 623, "ring-0 yaw += roll");
        assert_eq!(g.ent[s0].f32, 0, "ring-0 pitch is fixed at 0");
        assert_eq!(g.ent[s1].f30, 512, "ring-1 yaw is fixed");
        assert_eq!(
            g.ent[s1].f32, 623,
            "ring-1 pitch += fov. Reading 512 means the spin never \
             arrived and every replayed orb tumbles one step behind."
        );
    }
}
