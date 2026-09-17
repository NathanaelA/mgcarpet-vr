//! MC2 class-5 roster, wave A — every non-multipart creature on the
//! shared primitives ([`super::mobs`]). Traces: docs/traces/
//! mc2-class5-*.md; `EF:` cites = remc2 EventsFunctions.cpp. Models
//! here: 2, 9, 12, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 28.
//!
//! NOT here: 10 (the doomsday mana pyramid — its scripted sequence
//! leans on untraced helpers) and the multipart family 0, 3, 22, 27
//! (its own subsystem, docs/traces/mc2-multipart-chains.md). Model 15
//! (the castle guard archer) is never authored by any level — its one
//! launch site is the castle's guard respawn (EF:61488).
//!
//! Field-mapping additions over the [`super::mobs`] module doc:
//! `word_0x2C_44`→f44 (when a model reuses the strength slot as a
//! counter the trace says so in place) · `word_0x30_48`→f50 ·
//! `byte_0x46_70` sub-state→f71 · `byte_0x43_67`→f68 ·
//! `byte_0x44_68`→f69 · `manaRegen_0x88_136`→f136 ·
//! `fov_0x22_34`→f36 · byte[2] of struct_byte_0xc→flags bits 16..24.
//!
//! The per-model spawn ordinal (`byte_0x3E_62 = array_0x10[m]++`)
//! comes from [`Gen::mc2_spawn_ord`] and lands in f63. The slice
//! creatures (goat/archer/villager) still use the alloc-slot f63;
//! aligning them is a banked fidelity pass (goldens pinned).
//!
//! DELIBERATE APPROXIMATIONS (all flagged in place too):
//! - Every `+6` state whose body is MISSING from the decompile (m2,
//!   m9, m16, m17, m18, m19, m20 nominal, m21, m23, m25, m26, m28 —
//!   the dispatch would crash in remc2) holds inert; retail can
//!   never reach them (their rows' flee bit is clear).
//! - m18's `sub_253B0` duration table is partially pinned (the trace
//!   lists the formulas, not the (state,sub)→formula map).
//! - m26's drain bills the HUMAN through the same one-slot mailbox
//!   as a pool rival (`Gen::mc2_rival_leech` →
//!   `World::mc2_rival_leech_apply`); the flat-+14 stand-in is
//!   retired. The %63 spell-hijack is live: the roll mails
//!   [`crate::engine::world::World::mc2_spell_steal`].
//! - m12's footprint-clear/overlap scans (EF:14036-14093) are shaped,
//!   not verbatim. ⚠ NOT because the helpers are untraced — `sub_22640`
//!   is at EF:13906 and `sub_48990` at EF:32301, both fully decompiled
//!   in-tree; the site-jitter half (EF:13991-14024) went verbatim in
//!   2026-08-24e and the ANCHOR PICK (`sub_23020`, EF:14395-99) in
//!   2026-08-24f.

use super::behavior::BEHAVIOR;
use super::multipart::GuardV34;
use crate::engine::features::Gen;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// `sub_1F6D0`'s vertical servo lifts the target by its
/// `array_0x52_82.yaw` — including the human carpet, whose lift is
/// [`crate::mc1::combat::PLAYER_HH`]. `MGC_NO_M2_PLAYER_LIFT=1`
/// restores the pre-2026-09-03 bare-`ctx.pz` stand-in.
fn m2_player_lift() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M2_PLAYER_LIFT").is_none())
}

/// ⭐ THE m21 WATER SPLASH IS SPAWNED AT RETAIL'S GLOBAL SCRATCH AXIS,
/// NOT AT THE WALKER — see [`Gen::m21_jump`] for the shipped bytes.
/// `MGC_NO_MC2_SPLASH_PRED_AXIS=1` restores the pre-2026-09-10 port
/// behaviour (spawn at the walker's own position).
pub(crate) fn mc2_splash_pred_axis_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SPLASH_PRED_AXIS").is_none())
}

/// ⭐⭐⭐ THE MC2 WIZARD DEATH-FALL (10,1) PUFF IS SPAWNED AT RETAIL'S
/// GLOBAL SCRATCH AXIS TOO — `sub_5E310` pushes `0x1b398`, never
/// `&a1x->position` (shipped `NETHERW.EXE` file 0x82BDD, the bytes are
/// quoted at the two call sites). It agrees with the corpse's own
/// position on an ordinary fall, because `sub_5D530` committed the
/// carpet into that global two statements earlier — and parts from it
/// whenever the mover took its `byte[1] & 8` early return (file
/// 0x81d3f/0x81d4d: the veto path RETURNS without writing 0x1b398), as
/// it does for every tick of a death inside a whirlwind.
/// `MGC_NO_MC2_FALL_PUFF_PRED_AXIS=1` restores the pre-round-138 port
/// behaviour: the puff at the corpse's own settled axis, and no
/// carpet-side publish into `Gen::mc2_pred_axis`.
/// See [`crate::engine::world::World::mc2_player_fall`] and
/// `step_player_flight_mc2`.
pub(crate) fn no_mc2_fall_puff_pred_axis() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FALL_PUFF_PRED_AXIS").is_some())
}

/// ⭐⭐⭐ A/B toggle for **THE m15 WANDER'S UNCONDITIONAL AXIS SEED**:
/// set `MGC_NO_MC2_M15_WANDER_PRED_AXIS` to restore the pre-dig port,
/// which published [`Gen::mc2_pred_axis`] only when the wander
/// actually stepped. See the citation on [`Gen::m15_wander`].
pub(crate) fn no_mc2_m15_wander_pred_axis() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M15_WANDER_PRED_AXIS").is_some())
}

const M2_BASE: u8 = 16;
const M9_BASE: u8 = 72;

/// A/B toggle for the m28 POSE-SETTER's `byte[0]` LAW: set
/// `MGC_NO_M28_POSE_FLAGS` to restore the pre-dig `sub_2B860`, which
/// wrote the row/sprite/speed of all three poses and NOT ONE of the
/// three `struct_byte_0xc_12_15.byte[0]` stores retail makes there.
/// Shipped `NETHERW.EXE`, `sub_2B860` = file **0x50060** (VA 0x2B860):
///   pose 1 — `8a 63 0c` / `80 e4 f6` / `88 e2` / `80 ca 08` /
///            `88 53 0c`            (file 0x50085-0x500a2)
///   pose 2 — `8a 73 0c` / `80 e6 f6` / `88 73 0c` / `80 c9 08` /
///            `88 4b 0c`            (file 0x500df-0x50107)
///   pose 3 — `8a 6b 0c` / `80 cd 01` / `88 e8` / `24 f7` /
///            `88 43 0c`            (file 0x5017e-0x50194)
/// i.e. poses 1/2 write `byte[0] = (byte[0] & 0xF6) | 8` and pose 3
/// writes `byte[0] = (byte[0] | 1) & 0xF7` (EF:21319/21331/21347).
/// Bit 0 is the HIDDEN bit `sub_68C70`'s proximity-wake refuses
/// (`mc2_awake_one`, EF:55515) and bit 3 the collide/damage bit; so a
/// port m28 stuck at `byte[0] = 5` never re-wakes after its first
/// swing — `byte_0x39_57` freezes at 0 and every class-9 auto-aim
/// scan (`sub_67CB0`, EF:54811/54917/54964/54992) skips it forever.
/// All four retail call sites (VA 0x2B29C/0x2B482/0x2B64A/0x2B72E,
/// an `e8 rel32` scan of the shipped EXE) go through this one thunk,
/// which is [`Gen::m28_pose`]'s four callers here.
pub(crate) fn no_m28_pose_flags() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M28_POSE_FLAGS").is_some())
}
/// A/B toggle for the m28 STRIKE-ARM FALL-THROUGH law: set
/// `MGC_NO_M28_STRIKE_FALLTHROUGH` to restore the pre-dig
/// `sub_2B260`, whose `byte_0x46_70` switch RETURNED from arms 3 and
/// 7 instead of running on into 4 and 8 the same tick.
/// `case 3:` ends `PrepareEventSound_6E450(…, 38)` and then
/// `goto LABEL_35` — the shipped EXE has NO epilogue there: file
/// **0x4FCB4** `e8 97 2f 04 00` (the sound call) is followed at
/// **0x4FCBC** by `83 7b 10 00` / `0f 8e 6f 01 00 00`
/// (`if (dword_0x10_16 <= 0) sub_2BA50(6)`), which is LABEL_35 — the
/// arm-4 body (EF:21123-31 `case 3: … goto LABEL_35;`).
/// `case 7:` likewise ends `sub_2BA50(a1x, 8u)` at file **0x4FEFD**
/// and falls straight into LABEL_76 at **0x4FF05** `53` /
/// `e8 b5 01 ff ff` (`sub_1B8C0`, the move core) — EF:21186-89.
/// Without the arm-3 fall-through the port's m28 spends its whole
/// wind-up tick doing nothing: no move, and no `±56` swing crank, so
/// every subsequent heading trails retail's by exactly 56.
pub(crate) fn no_m28_strike_fallthrough() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M28_STRIKE_FALLTHROUGH").is_some())
}
/// A/B toggle for the m28 STRIKE-ANIMATION LENGTH law: set
/// `MGC_NO_M28_STRIKE_FRAMES` to restore the pre-dig `sub_2B860`
/// pose 2, which hard-coded `dword_0x10_16 = 16`, never wrote
/// `word_0x2C_44`, hard-coded the melee window as `4..=12`, and
/// zeroed the swing's `subSpellIndex_0x2A_42` (`f44`) — retail zeroes
/// `word_0x2C_44`, a DIFFERENT word, so the port's brute lost its
/// 2000-damage swing on its first wind-up and never got it back.
/// Retail (EF:21335-40): `word_0x2C_44 = 0` … then
/// `dword_0x10_16 = GetAnimationByIndex(animations_E9C08, v5)
/// ->CountOfFrames_16; word_0x2C_44 = dword_0x10_16`, and the melee
/// gate is `word_0x2C_44 - 3 > dword_0x10_16 && dword_0x10_16 > 3`
/// (EF:21157; shipped EXE file **0x4FD45** `0f bf 43 2c` /
/// `8b 73 10` / `83 e8 03` / `39 f0` / `7e 1b` / `83 fe 03` /
/// `7e 16`) — NOT a literal `4..=12`. The recording witnesses the
/// frame count directly: mc2l24 t=28346 and t=28661 both step slot
/// 10's `word_0x2C_44` to **24** with `dword_0x10_16` landing on 23
/// after the same tick's arm-4 decrement, and the melee latch
/// (`byte_0x46_70` 4 -> 5) fires at t=28671 with `dword_0x10_16` =
/// 14 — outside the port's `4..=12`.
pub(crate) fn no_m28_strike_frames() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M28_STRIKE_FRAMES").is_some())
}
/// The m28 strike animation's frame count — retail's
/// `GetAnimationByIndex(animations_E9C08x, *(int16_t *)&x_BYTE_D9F50[0x5b6])
/// ->CountOfFrames_16`, witnessed as 24 on mc2l24 (see
/// [`no_m28_strike_frames`]).
const M28_STRIKE_FRAMES: i16 = 24;
/// A/B toggle for the m28 CHASE STRIKE-RANGE law: set
/// `MGC_NO_M28_STRIKE_RANGE_PRED` to restore the pre-dig
/// `sub_2B260` arm 2, which measured the strike range to the
/// TARGET'S OWN position. Retail measures it to `v23x` — the STACK
/// COPY of the target's position already stepped 768 units along the
/// TARGET's yaw (`MoveEntity_57FA0(&v23x, v25x->yaw_0x1C_28, 0,
/// 768)`, EF:21076), i.e. the very point the chase is already
/// steering at one line later (EF:21078 `roll_0x20_32 =
/// sub_581E0_maybe_tan2(&a1x->position_0x4C_76, &v23x)`) and the only
/// thing `v23x` ever holds — retail never re-reads
/// `v25x->position_0x4C_76` in this arm.
/// The shipped EXE settles it. `v23x` is `[ebp-0x1c]`: built at file
/// **0x4FB2B** `8d 7d e4` (`lea edi,[ebp-0x1c]`) with the 6-byte
/// position copy `a5` / `66 a5`, then stepped by
/// **0x4FB49** `e8 52 cc 02 00` (`sub_57FA0`). The range test pushes
/// **0x4FC0E** `8d 45 e4 50` (`lea eax,[ebp-0x1c]`; push) and
/// `8d 43 4c 50` (`lea eax,[ebx+0x4c]` = SELF; push) into
/// **0x4FC16** `e8 b5 d0 02 00` (`EuclideanDistXY_584D0`) and
/// compares **0x4FC1E** `3d 00 40 2a 00` (`cmp eax,0x2a4000` =
/// 2768896). The pushed pointer is the PREDICTED point; `[ebx+0x4c]`
/// is self, and `v25x->position_0x4C_76` is never pushed.
/// Witness — mc2l24 pair t=28659->28660, slot 10 (5,28) action 226:
/// self (12199,45030), target = human slot 116 at (14407,44745) yaw
/// 1479. Raw target d2 = 2208² + 285² = 4,956,489 — no strike (what
/// the port did: `dword_0x10_16` 5 -> 4, `byte_0x46_70` stays 2).
/// Predicted point = (13651,44879), d2 = 1452² + 151² = 2,131,105 <
/// 2,768,896 — retail takes `sub_2BA50(a1x, 3u)`: `byte_0x46_70`
/// 2 -> 3 and `dword_0x10_16` 4 -> **0** (the recording's
/// `scratch10` 5 -> 0 at t=28660), and the swing arm runs at
/// t=28661. Same shape on slot 11 at t=29795->29796.
pub(crate) fn no_m28_strike_range_pred() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M28_STRIKE_RANGE_PRED").is_some())
}
/// A/B toggle for the Summon-Army `(10,72)` RING-NODE law: set
/// `MGC_NO_SUMMON_NODES` to restore the pre-dig behaviour, where
/// `sub_51800`'s node ring was collapsed to a direct class-5 creature
/// ring (no `(10,72)` records, no 16-tick materialize delay) and a
/// lapsed summon dropped only the `(10,0)` fire, never the `(10,73)`
/// puff `sub_1E580` mints ahead of it (EF:10745).
pub(crate) fn no_summon_nodes() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SUMMON_NODES").is_some())
}
/// A/B toggle for the SUMMON-RING HEAD BEARING law: set
/// `MGC_NO_SUMMON_HEAD_BEARING` to restore the pre-dig behaviour,
/// where the `(10,72)` ring HEAD kept `NewEvent_4A050`'s memset zeros
/// in `yaw_0x1C_28`/`pitch_0x1E_30` because `mc2_proj_impact`'s
/// `(10,72)` arm returns `None` and so skipped the generic impact
/// tail. Retail's generic flight worker `sub_65820` stamps the
/// RETURNED effect record — i.e. `sub_51800`'s HEAD only, the
/// `qmemcpy` tails having already been minted — with
/// `v11x->yaw_0x1C_28 = a1x->yaw_0x1C_28;
///  v11x->pitch_0x1E_30 = a1x->pitch_0x1E_30;` (EF:62989-90).
pub(crate) fn no_summon_head_bearing() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SUMMON_HEAD_BEARING").is_some())
}
/// `MGC_NO_MC2_M23_LOCK_TESTS=1` restores the pre-dig m23 (dweller)
/// mana-node LOCK re-check, which carried an INVENTED action test
/// (`tick70 != 62`) and OMITTED retail's `life_0x8 < 0` reject.
/// `sub_28420`, shipped `NETHERW.EXE` file 0x4CC20, rejects on exactly
/// four tests: life, the reap bit, class 10 and model 39.
/// ⚠ The sibling SCAN (`sub_28000`) is model-ONLY in retail and is
/// deliberately NOT changed — see [`Gen::m23_find_node`]'s note.
pub(crate) fn no_mc2_m23_lock_tests() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M23_LOCK_TESTS").is_some())
}
/// A/B toggle for m24's UNCONDITIONAL IDLE `else` (`sub_28500`
/// EF:18670-72 — see the write-up at the arm): set
/// `MGC_NO_MC2_M24_IDLE_ELSE_FORCES_CHARGE` to restore the pre-dig
/// extra arm, which preserved action 194 when the IDLE PRIMITIVE
/// promoted the creature, where retail overwrites any such promotion
/// with 198 (the charge) and keeps 194 only for the acquire the
/// handler runs itself.
fn no_mc2_m24_idle_else_forces_charge() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M24_IDLE_ELSE_FORCES_CHARGE").is_some())
}
/// A/B toggle for m24's PLAIN SPRITE COMMIT (`sub_287B0`'s guarded
/// tail, EF:18816-23 — see [`Gen::m24_pose`]): set
/// `MGC_NO_MC2_M24_POSE_PLAIN_SPRITE` to restore the pre-dig pose,
/// which committed through `SetEntityIndexAndRot_49CD0` (re-deriving
/// the `array_0x52_82` extents quad on every animation change),
/// re-stamped sprite 336 unguarded every tick, and never committed
/// sprite 335 from actions 194/198 at all.
fn no_mc2_m24_pose_plain_sprite() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M24_POSE_PLAIN_SPRITE").is_some())
}
/// A/B toggle for m18's UNGUARDED BARRAGE AIM (`sub_250B0` case 0,
/// EF:15995 — see [`Gen::m18_face_raw`]): set
/// `MGC_NO_MC2_M18_AIM_UNGUARDED` to restore the pre-dig liveness
/// lookup, which applied case 1's `life_0x8 < 0 || byte[1] & 4` reject
/// to case 0, where retail has no test at all — so a tank whose quarry
/// died stopped tracking it instead of turning after the corpse.
fn no_mc2_m18_aim_unguarded() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M18_AIM_UNGUARDED").is_some())
}
/// A/B toggle for m18's UNGUARDED WATCH ARM (`sub_24E20`'s
/// `byte_0x46_70 == 1` branch, EF:15882-15902 / NETHERW.EXE VA 0x24F9D
/// — see [`Gen::m18_tick`]'s `0 =>`): set
/// `MGC_NO_MC2_M18_WATCH_UNGUARDED` to restore the pre-dig behaviour,
/// where the watch arm resolved `word_0x96_150` through the
/// LIVENESS-GUARDED [`Gen::mc2_target`], so the tick its quarry died
/// the tank dropped the target and re-armed the roam timer instead of
/// keeping its bearing on the corpse — and where an empty
/// `word_0x96_150` (or a `byte_0x46_70` of 2+) also fell into that
/// drop instead of retail's bare `return`.
fn no_mc2_m18_watch_unguarded() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M18_WATCH_UNGUARDED").is_some())
}
/// A/B toggle for the m21 PHASE-7 WRAPPER TAIL (`sub_26470`'s last
/// two lines, EF:16963-65 — see [`Gen::m21_wrapper_tail`]): set
/// `MGC_NO_MC2_M21_WRAPPER_TAIL` to restore the pre-dig behaviour,
/// where the port ran the jump cycle and stopped, so a devil the
/// `sub_1D5D0` legs promoted out of its hold kept the IDLE rest base
/// (`byte_0x43_67 = 64`) that retail had just zeroed.
pub(crate) fn no_mc2_m21_wrapper_tail() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M21_WRAPPER_TAIL").is_some())
}
/// A/B toggle for the m21 ATTACK-ARM REACH TEST (`sub_26220`'s
/// `sub_583F0_distance_3d`, EF:16890-91 / `NETHERW.EXE` file
/// 0x4AB5A-0x4AB76 — see the block in [`Gen::m21_tick`]'s `2 =>` arm):
/// set `MGC_NO_MC2_M21_RANGE_3D` to restore the pre-dig behaviour,
/// where the port passed the DEVIL's own z as the target's z, turning
/// retail's 3-D reach test into a planar one that can only understate
/// the distance — so a devil fired bolts retail never fires and stayed
/// in ATTACK where retail drops back to IDLE.
pub(crate) fn no_mc2_m21_range_3d() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M21_RANGE_3D").is_some())
}
/// A/B toggle for the m2 PHASE-7 WRAPPER JIGGLE (`sub_1F8A0`'s middle
/// block, EF:11567-74 — see [`Gen::m2_wrapper_jiggle`]): set
/// `MGC_NO_MC2_M2_WRAPPER_JIGGLE` to restore the pre-dig behaviour,
/// where a STAGE-HELD `(5,2)` took no wander jiggle at all and its
/// per-entity LCG ran two draws per 8 ticks behind retail's.
pub(crate) fn no_mc2_m2_wrapper_jiggle() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M2_WRAPPER_JIGGLE").is_some())
}
/// A/B toggle for the m2 PHASE-7 WRAPPER'S LUNGE RE-ARM
/// (`sub_1F8A0`'s LAST statement, EF:11576 — see
/// [`Gen::m2_wrapper_lunge_rearm`]): set
/// `MGC_NO_MC2_M2_LUNGE_REARM` to restore the pre-dig behaviour,
/// where a STAGE-HELD `(5,2)` the legs had just promoted to the
/// chase kept its ctor `slot % 100` countdown, so the lunge speed
/// `5 * minSpeed / 2` fired dozens of ticks late (or never).
pub(crate) fn no_mc2_m2_lunge_rearm() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M2_LUNGE_REARM").is_some())
}
/// A/B toggle for the METAMORPH CLOAK law (`sub_6A030`, EXE
/// 0x8E92F `or dl,0x21` / 0x8EA50 `and cl,0xdf`, EF:56335 /
/// EF:56403): set `MGC_NO_METAMORPH_CLOAK` to restore the
/// pre-dig behaviour, where the port's metamorph hid the carpet
/// from the RENDERER only and left the `byte[0] & 0x20`
/// scan-invisibility bit clear, so every mob scanner kept seeing
/// a wizard retail had made unscannable.
pub(crate) fn no_metamorph_cloak() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_METAMORPH_CLOAK").is_some())
}
/// A/B toggle for the m12 BUILD-SITE VETO SCANS (`sub_22760`
/// EF:14056-93) and the `fontTypeIndex_0x3D_61` latch they read
/// (:14105): set `MGC_NO_M12_SITE_VETOS` to restore the pre-2026-09-03
/// behaviour, where the port ran only the building-chain scan and never
/// stamped the one-house latch.
fn m12_site_vetos() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M12_SITE_VETOS").is_none())
}
/// A/B toggle for THE M12 SITE-LIVENESS TEST IS `class`, NOT THE REAP
/// BIT: set `MGC_NO_M12_SITE_REAP_BLIND=1` to restore the pre-dig
/// behaviour, where the villager's walk-to-site (`sub_22E60` EF:14288-92)
/// and build-execute (`sub_22760` EF:13969) both ALSO required
/// `flags & 0x400 == 0` on the anchor. Retail tests only
/// `!v8x->class_0x3F_63` / `!v1x->class_0x3F_63 || model != 45` — the
/// reap bit is invisible to both, so a villager keeps walking to (and
/// building beside) a shrine that was reaped this tick and stays
/// class-10 until the free pass actually recycles the record.
fn m12_site_reap_blind() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M12_SITE_REAP_BLIND").is_none())
}
/// A/B toggle for THE TOWNIE RALLY TARGET IS REAP-BLIND TOO — the
/// villager brain `sub_23340` (EF:14599-601, NETHERW.EXE 0x47d9d
/// `cmpb $0xa,0x3f(%esi)` / 0x47daf `cmpb $0x2d,0x40(%esi)`) and the
/// trader brain `sub_237B0` (EF:14828-30, NETHERW.EXE 0x4822d /
/// 0x48233) both validate `word_0x96_150` on **class 10 + model 45 and
/// nothing else**; neither reads the reap bit. Set
/// `MGC_NO_MC2_TOWNIE_TARGET_REAP_BLIND=1` to restore the pre-dig
/// behaviour, where both ALSO required `flags & 0x400 == 0` and so
/// dropped a dwelling the tick it was reap-flagged — the twin of
/// [`m12_site_reap_blind`], one call path over.
pub(crate) fn townie_target_reap_blind() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_TOWNIE_TARGET_REAP_BLIND").is_none())
}
/// A/B toggle for THE TRADER'S INVALID TARGET RETURNS — `sub_237B0`
/// tests `word_0x96_150` FIRST (NETHERW.EXE 0x480ee
/// `mov 0x96(%ebx),%di; test %di,%di; jne 0x48221`) and the whole
/// wander-turn + far-dwelling scan lives on the **zero-target** arm
/// only. A non-zero target that fails validation clears the handle,
/// sets `actSpeed = maxSpeed` and jumps straight to LABEL_44
/// (0x48286-0x4829d) — no LCG draws, no rescan, this tick. Set
/// `MGC_NO_MC2_TRADER_INVALID_TARGET_RETURN=1` to restore the pre-dig
/// behaviour, where the port merged "target invalid" into "no target"
/// and so spent two per-entity draws and re-acquired at `maxSpeed+12`.
pub(crate) fn trader_invalid_target_return() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_TRADER_INVALID_TARGET_RETURN").is_none())
}
/// A/B toggle for THE GROUNDED HIVE'S ENGAGE-POSE TAIL: set
/// `MGC_NO_M9_GROUNDED_POSE_TAIL=1` to restore the pre-dig behaviour,
/// where every early return inside the grounded body (`sub_20940`)
/// skipped `sub_203D0`'s `LABEL_85` tail. Retail's dispatch is
/// `v2 = a1x->byte_0x46_70; if (v2) { if (v2 == 1) sub_20940(a1x);
/// goto LABEL_85; }` (EF:12055-60) and `LABEL_85` is
/// `if (actionIndex == 74) sub_20EC0(a1x);` (EF:12282-84) — so a
/// grounded hive that takes a hit and flips to 74 inside `sub_20940`
/// STILL gets the engage pose in the same tick.
fn m9_grounded_pose_tail() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M9_GROUNDED_POSE_TAIL").is_none())
}
/// A/B toggle for THE WANTED TIMER ARMS ON **ANY** WIZARD, NOT ONLY
/// THE HUMAN: set `MGC_NO_WANTED_ANY_WIZARD=1` to restore the pre-dig
/// behaviour, where the four townie/villager kill-and-hit stamps fired
/// only when the source was [`PLAYER_TARGET`]. Retail's four sites are
/// all the SAME shape and none of them knows who the human is —
/// `v1x = Entities_EA3E4[a1x->word_0x24_36];
///  if (v1x > Entities_EA3E4[0] && (!v1x->model_0x40_64 ||
///      v1x->model_0x40_64 == 1))
///      …->dword_0xA4_164x->word_0x248_584 = 200;`
/// (`sub_23200` EF:14440-42 = m12 kill, `sub_23B30` EF:14913-16 =
/// m14 kill, `KillTownie_23680` EF:14677-79, and the m13 hit head
/// EF:14561-63) — the same predicate [`Gen::mc2_is_wizard`] already
/// carries for `mc2_head_wanted`. A rival's kill arms ITS timer, which
/// is what the archer's Scan-A post-reject then reads.
pub(crate) fn wanted_any_wizard() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_WANTED_ANY_WIZARD").is_none())
}
/// A/B toggle for THE TANK'S HIT LATCHES ITS ATTACKER AS ITS TARGET:
/// set `MGC_NO_M18_HIT_TARGET_LATCH=1` to restore the pre-dig
/// behaviour, where a damaged m18 kept whatever `word_0x96_150` it
/// already had (0 while walking, because `sub_25050`'s roam arm zeroes
/// it every tick). Retail's m18 head `sub_252E0` ends
/// `if (result >= 1) { if (result <= 1) a1x->word_0x96_150 =
///  a1x->word_0x26_38; else if (result == 2)
///  a1x->actionIndex_0x45_69 = 148; }` (EF:16140-49; shipped
/// `NETHERW.EXE` 0x49B77-0x49B97) — the DAMAGE half of that tail was
/// lost when `sub_252E0` was split into [`Gen::m18_head`] and the
/// shared [`Gen::mc2_state_head`], which only carried the death half.
/// ⚠ Only `sub_252E0` has this tail; `sub_28860`'s inlined m25 head
/// (EF:18856-90) deliberately does not — do not copy it there.
fn m18_hit_target_latch() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M18_HIT_TARGET_LATCH").is_none())
}
/// A/B toggle for THE FOOTPRINT CLEAR STAMPS THE **ATTACKER** HALF OF
/// THE KILLER PAIR: set `MGC_NO_CLEAR_TILE_ATTACKER_STAMP=1` to
/// restore the pre-dig behaviour, where
/// [`Gen::mc2_building_clear_tile`] wrote `f36`/`f38` instead of
/// `f38`/`f40`. Retail `sub_57390` is
/// `life_0x8 = -1; word_0x24_36 = a2; word_0x26_38 = a2;`
/// (EF:39801-03; shipped `NETHERW.EXE` 0x7BC1F-0x7BC2D) — BOTH halves.
/// `word_0x26_38` is the port's `f40`, not `f38`: the mapping is fixed
/// by `mc2_state_head`'s own `f38 = f40`, which mirrors retail's
/// `a1x->word_0x24_36 = a1x->word_0x26_38` (EF:16137).
pub(crate) fn clear_tile_attacker_stamp() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_CLEAR_TILE_ATTACKER_STAMP").is_none())
}

const M12_BASE: u8 = 96;
const M14_BASE: u8 = 112;
const M15_BASE: u8 = 120;
const M16_BASE: u8 = 128;
const M17_BASE: u8 = 136;
const M18_BASE: u8 = 144;
const M19_BASE: u8 = 152;
const M20_BASE: u8 = 160;
const M21_BASE: u8 = 168;
const M23_BASE: u8 = 184;
const M24_BASE: u8 = 192;
const M25_BASE: u8 = 200;
const M26_BASE: u8 = 208;
const M28_BASE: u8 = 224;

/// ⛔⛔ **`byte_0x38_56` IS `@0x38` AND ITS PORT HOME IS `Ent::f28`, NOT
/// `Ent::f56`. `Ent::f56` IS `@0x36` (`word_0x36_54`) ON CLASS 5.**
///
/// Every class-5 creature ctor in this module — and
/// [`Gen::mc2_spawn_doomsday`], the 18th, which lives in
/// `mc2/doomsday.rs` and a roster-only sweep would miss — writes the
/// ch0 damage contract TWICE: once correctly as `e.f28 = 1`
/// (`// cross-column damage contract`) and once again, a dozen lines
/// later, as `e.f56 = 1`. The second write is a duplicate that lands
/// in a DIFFERENT retail word.
///
/// SHIPPED-EXE PROOF, on the `(5,21)` devil ctor `sub_4C8F0`
/// (`NETHERW.EXE` file offset **0x710F0** = linear 0x4C8F0 + 0x24800):
/// ```text
///   4c941  66 c7 40 22 00 00   mov word [eax+0x22],0x0   ; fov_0x22_34 = 0
///   4c963  c6 40 38 01         mov byte [eax+0x38],0x1   ; byte_0x38_56 = 1
/// ```
/// `@0x22` is a WORD and `@0x38` is a BYTE — two distinct fields — and
/// **the function never writes `[eax+0x36]` at all** (the complete set
/// of offsets it stores to is 0x04, 0x14, 0x1c, 0x1e, 0x20, 0x22, 0x2a,
/// 0x2c, 0x38, 0x39, 0x3e, 0x3f, 0x40, 0x41, 0x43, 0x44, 0x45, 0x46,
/// 0x82, 0x84, 0x90, 0xa0). So retail's `@0x36` on a fresh class-5
/// creature is `NewEvent_4A050`'s zero, which is exactly what the
/// recording captures.
///
/// The importer agrees with the EXE and not with these ctors:
/// `import_ent_mc2` seats `f28: r.b38` (`@0x38`) unconditionally and
/// `f56: if matches!(r.class3f, 2 | 10) { r.b38 } else { r.f36 }` —
/// so on class 5 `Ent::f56` is `@0x36`, and the multipart family
/// (models 0/3/22/27, the only readers of `f56` on this class) uses it
/// as exactly that: `word_0x36_54`, the segment link length / writhe
/// phase. **None of the 18 models gated below is a multipart model and
/// no port reader consumes their `f56`, so the stray 1 is inert in the
/// port and visible only as a wrong recorded lane.**
///
/// MEASURED — the round-105 all-lane census (`MGC_RAW_SHADOW_ALL=1
/// verify-deltas`, PAIR view) counts **1,199 rows over 11 class-5
/// families on mc2l22, every one `retail 0 port 1`**, one row per
/// spawn; `f36` is the census's third-widest lane (36 families) and
/// this law removes 11 of them. Re-measured on the first 12,000 pairs:
/// **520 rows over 8 families before, ZERO after.**
///
/// ⚠ It is an UNGRADED lane: `port_ent_lanes_mc2` publishes it (so
/// `dump-state` shows it) and `obs_project_mc2` never projects it, so
/// no fixture and no `replay` boundary can see it today. The law is
/// landed on the citation, not on a moved number.
///
/// `MGC_NO_MC2_CLASS5_W36=1` restores the old behaviour.
pub(crate) fn mc2_class5_w36_legacy() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_CLASS5_W36").is_some())
}

impl Gen {
    // ---- shared bits ---------------------------------------------------------

    /// `D41A0_0.array_0x10[model]++` — the per-model instance
    /// counter every ctor stores into byte_0x3E_62 (f63).
    pub(crate) fn mc2_ord(&mut self, model: usize) -> u8 {
        let o = self.mc2_spawn_ord.0[model];
        self.mc2_spawn_ord.0[model] = o.wrapping_add(1);
        o
    }

    /// The common wake stagger `word_0x1a - ord % word_0x1a + 4`.
    pub(crate) fn mc2_wake_stagger(row: usize, ord: u8) -> i16 {
        let v26 = BEHAVIOR[row].v_26.max(1);
        v26 - (ord as i16 % v26) + 4
    }

    /// The one-draw facing idiom shared by most ctors:
    /// `roll = yaw = (rand & 0x7FF) - 1; pitch = roll`.
    pub(crate) fn mc2_ctor_facing(&mut self, i: usize) {
        let d = self.mc2_rand(i);
        let f = ((d & 0x7FF) as i32 - 1) as u16;
        let e = &mut self.ent[i];
        e.f34 = f;
        e.f30 = f;
        e.f32 = f;
    }

    /// Face a point and sidestep a crowding packmate (the every-4th
    /// tick idiom of the custom attack states).
    fn mc2_aim_avoid(&mut self, i: usize, tx: u16, ty: u16) {
        let e = &self.ent[i];
        self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
        self.mc2_avoid_packmate(i);
    }

    /// Target-is-a-wizard check (class 3 model 0|1; the human counts).
    pub(crate) fn mc2_is_wizard(&self, t: u16) -> bool {
        t == PLAYER_TARGET
            || ((t as usize) < self.ent.len()
                && t != 0
                && self.ent[t as usize].class64 == 3
                && self.ent[t as usize].model65 <= 1)
    }

    /// The bare POINTER test on `word_0x96_150` —
    /// `Entities_EA3E4[target] > Entities_EA3E4[0]`, i.e. "the slot
    /// resolves to something other than the null record". Entity
    /// records are contiguous, so retail's `<=` is exactly `target ==
    /// 0`; the human's own record is a pool record like any other, so
    /// the port's out-of-pool [`PLAYER_TARGET`] passes it.
    ///
    /// ⚠ THIS IS NOT [`Gen::mc2_target`]. That one carries the
    /// life/reap/class guard the CHASE applies (`sub_1C310`
    /// EF:9297-9302), and a state body that re-asks it is re-asking
    /// what its own chase would ask one level down — with a whole
    /// tick of the state machine in between. Several state heads take
    /// only this pointer test and then run arms that WRITE before the
    /// chase ever resolves the target.
    fn mc2_target_ptr(&self, t: u16) -> bool {
        t == PLAYER_TARGET || (t != 0 && (t as usize) < self.ent.len())
    }

    /// `KillEntity_1C930`'s corpse effect is the (10,1) explosion
    /// (id inherited).
    pub(crate) fn mc2_corpse_burst(&mut self, i: usize) {
        let (x, y, z, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        if let Some(b) = self.mc2_spawn_big_explosion(x, y, z) {
            self.ent[b].id24 = id;
        }
    }

    // =========================================================================
    // MODEL 2 — day-only pack hunter (ctor sub_4B590 EF:33751,
    // states 0x10-17, trace: mc2-class5-m2-9-12-14-15.md)
    // =========================================================================

    pub(crate) fn mc2_spawn_m2(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.mc2_night_shade.0 {
            return None; // DAY-ONLY (:33758)
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 2;
            e.tick70 = M2_BASE + 1; // 17
            e.f28 = 1; // cross-column damage contract
            e.f128 = 64;
            e.f130 = 30;
            e.max_life = 3000;
            e.f126 = 32; // minSpeed / 2 (:33771)
        }
        self.mc2_set_mana_half(i); // 1500
        self.mc2_ctor_facing(i);
        let ord = self.mc2_ord(2);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 200; // melee damage
            e.f66 = 3;
            e.f67 = 0;
            e.f26 = (i % 100) as i16;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.  // burnable
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 73;
            e.f63 = ord;
        }
        self.ent[i].f58 = Self::mc2_wake_stagger(73, ord);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 3);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// ⭐⭐⭐ **THE m2 PHASE-7 WRAPPER ENDS IN A TWO-DRAW WANDER
    /// JIGGLE, AND IT FIRES FOR EVERY STAGE-HELD `(5,2)`.**
    /// `sub_1F8A0` (EF:11563-77) is the model-2 `8*2+7 = 23` wrapper
    /// and its whole body is
    /// ```text
    ///   sub_1D5D0(a1x, 16);
    ///   if (!(byte_0x3E_62 & 7) && (unsigned __int8)(StageVar2_0x49_73 - 1) <= 8u) {
    ///       rand = 9377*rand + 9439;   v1 = 2 * ((int)(rand % 0x9D) / 79);
    ///       rand = 9377*rand + 9439;   roll_0x20_32 += rand % 0x55 * (v1 - 1);
    ///       roll_0x20_32 &= 0x7ff;
    ///   }
    ///   if (actionIndex_0x45_69 == 18) dword_0x10_16 = 1;
    /// ```
    /// SHIPPED EXE (NETHERW.EXE file 0x440A0 = the `sub_1D5D0` call
    /// with `push 0x10`, so this is unambiguously the m2 wrapper):
    /// ```text
    ///   440a8  6a 10              push 0x10
    ///   440ab  e8 20 dd ff ff     call 0x41dd0        ; sub_1D5D0
    ///   440b0  8a 63 3e           mov  ah,[ebx+0x3e]
    ///   440b6  f6 c4 07           test ah,0x7
    ///   440b9  0f 85 7b 00 00 00  jne  0x4413a        ; off-cadence: SKIP
    ///   440bf  8a 53 49           mov  dl,[ebx+0x49]  ; StageVar2
    ///   440c2  fe ca              dec  dl
    ///   440c4  80 fa 08           cmp  dl,0x8
    ///   440c7  77 71              ja   0x4413a        ; kinds 1..9 only
    ///   440cd  2e ff 24 85 78 f8  jmp  [cs:eax*4+0xf878]  ; 9-entry table,
    ///                                                     ; ONE body follows
    ///   440d5  66 69 43 14 a1 24  imul ax,[ebx+0x14],0x24a1   ; 9377
    ///   440db  05 df 24 00 00     add  eax,0x24df             ; 9439
    ///   440e0  b9 9d 00 00 00     mov  ecx,0x9d               ; 157
    ///   440f3  b9 4f 00 00 00     mov  ecx,0x4f               ; 79
    ///   44106  66 69 43 14 a1 24  imul ax,[ebx+0x14],0x24a1   ; second draw
    ///   44119  b9 55 00 00 00     mov  ecx,0x55               ; 85
    ///   44124  8d 46 ff           lea  eax,[esi-0x1]          ; v1 - 1
    ///   44127  0f af c2           imul eax,edx
    ///   4412c  66 8b 53 20        mov  dx,[ebx+0x20]          ; roll
    ///   44132  66 89 53 20        mov  [ebx+0x20],dx
    ///   44136  80 63 21 07        and  byte [ebx+0x21],0x7    ; &= 0x7FF
    /// ```
    /// The function ends at 0x4414a and there is exactly ONE body
    /// between the table jump and the `cmp byte [ebx+0x45],0x12` tail,
    /// so all nine table entries land on 0x440d5 — the decompile's
    /// `<= 8u` collapse is right.
    ///
    /// The `& 7` gate is the SAME 8-tick cadence the `sub_1D5D0` legs
    /// use, and the `StageVar2` it reads is the one the legs JUST
    /// WROTE (a wizard-watch promotion to 10 suppresses the jiggle on
    /// the release tick). This is [`crate::mc2::stagevars`]'s
    /// "⚠ OWED: the held seam runs FOUR more wrapper tails here"
    /// register, now witnessed.
    ///
    /// WITNESS — `recordings/mc2l1.mgcr` slot 24, a `(5,2)` on
    /// StageVar1 6 / StageVar2 2 (the kind-2 graze leash) at action 23.
    /// Retail's own `rand` lane steps **exactly 2 LCG draws on every
    /// `f63 & 7 == 0` tick and 4 on every `f63 & 0x3F == 0` tick**
    /// (t=1573 phase 0: 39121→64269 = 4 steps; t=1581 phase 8:
    /// 64269→14443 = 2; t=1589 phase 16: →21321 = 2; … t=1637 phase
    /// 64: 29727→51547 = 4). The port had ONLY `sub_1DDA0`'s
    /// `& 0x3F` pair, so it drew 2 every 64 ticks and 0 on the other
    /// seven cadence ticks: `rand` retail 64269 vs port 47919 at
    /// t=1573 and a period-8 head every 8 ticks to the end of the
    /// take — **3,645 of the take's 3,713 excess resets.**
    ///
    /// `roll` (`f34`) is UNGRADED, so only the `rand` lane shows.
    ///
    /// 🏦 OWED, NOT LANDED — the wrapper's LAST line
    /// `if (actionIndex == 18) dword_0x10_16 = 1` (the `f26` lunge
    /// countdown re-arm, which `m2_tick`'s arms 0/1/3/_ already carry
    /// on the UNHELD path) is also missing from both seams. It has no
    /// witness in this corpus (no held `(5,2)` was promoted to 18 on a
    /// wrapper tick here) and wants its own A/B.
    ///
    /// `MGC_NO_MC2_M2_WRAPPER_JIGGLE=1` restores the old behaviour.
    pub(crate) fn m2_wrapper_jiggle(&mut self, i: usize) {
        if no_mc2_m2_wrapper_jiggle() {
            return;
        }
        {
            let e = &self.ent[i];
            if e.f63 & 7 != 0 || !matches!(e.site_z, 1..=9) {
                return;
            }
        }
        let v = self.mc2_rand(i);
        let sign = 2 * ((v % 0x9D) / 79) as i32 - 1;
        let r = self.mc2_rand(i);
        let e = &mut self.ent[i];
        e.f34 = ((e.f34 as i32 + (r % 0x55) as i32 * sign) as u16) & 0x7FF;
    }

    /// ⭐⭐⭐ **THE m2 PHASE-7 WRAPPER RE-ARMS THE LUNGE COUNTDOWN ON
    /// THE RELEASE TICK, AND THAT IS WHERE THE `speed = 160` LUNGE
    /// COMES FROM.**
    ///
    /// `sub_1F8A0`'s last statement is
    /// `if (actionIndex_0x45_69 == 18) dword_0x10_16 = 1;`
    /// (EF:11576) — the SAME line `m2_tick`'s arms 0/1/3/_ carry, and
    /// it reads the action the `sub_1D5D0` legs JUST WROTE, so it
    /// fires on the tick a held `(5,2)` is released into the chase.
    ///
    /// SHIPPED EXE — NETHERW.EXE file 0x4413A, the fall-through of
    /// the wrapper's `& 7` cadence gate (`jne 0x4413a` at 0x440B9),
    /// so it is UNCONDITIONAL on the cadence and on `StageVar2`:
    /// ```text
    ///   4413a  80 7b 45 12              cmpb $0x12,0x45(%ebx)   ; action == 18
    ///   4413e  75 07                    jne  0x44147
    ///   44140  c7 43 10 01 00 00 00     movl $0x1,0x10(%ebx)    ; @0x10 = 1
    ///   44147  5d 5e 5b c3              pop/pop/pop/ret
    /// ```
    ///
    /// The next tick `m2_tick`'s arm 2 sees `f26 == 1`, decrements it
    /// to 0 and takes `f126 = 5 * f128 / 2 = 160`. Without the re-arm
    /// the countdown is still the ctor's `slot % 100` seed (24, 25,
    /// 26, 28 … on mc2l1) so the lunge fired that many ticks late or
    /// not at all, and the port ran the chase at `f128` (64) or the
    /// ctor's `minSpeed / 2` (32) where retail ran 160.
    ///
    /// WITNESS — `recordings/mc2l1.mgcr`, 22 of the take's 34 excess
    /// resets. Every one is the same shape: a `(5,2)` on `StageVar1 6
    /// / StageVar2 2` (the wizard-watch leash) at action 23 with
    /// `@0x10 == slot % 100`; on the release tick the legs write
    /// `StageVar2 = 10`, action 18, `target = 111`, and `@0x10` drops
    /// to **1** — e.g. slot 28 t=4310 `@0x10=28` -> t=4311 `@0x10=1`
    /// -> t=4312 `@0x10=0, speed=160` (port: `@0x10=28`, speed 32).
    ///
    /// ⚠ This is the "🏦 OWED, NOT LANDED" register on
    /// [`Gen::m2_wrapper_jiggle`], whose "no held `(5,2)` was promoted
    /// to 18 on a wrapper tick here" was a FALSE CONFIDENT NEGATIVE:
    /// the promotion does not need the `& 7` cadence, only the legs.
    ///
    /// `MGC_NO_MC2_M2_LUNGE_REARM=1` restores the old behaviour.
    pub(crate) fn m2_wrapper_lunge_rearm(&mut self, i: usize) {
        if no_mc2_m2_lunge_rearm() {
            return;
        }
        if self.ent[i].tick70 == M2_BASE + 2 {
            self.ent[i].f26 = 1;
        }
    }

    /// The wake yelp `(rand & 1) + 12` (:11483 / :11524).
    fn m2_yelp(&mut self, i: usize) {
        let d = self.mc2_rand(i);
        self.snd(((d & 1) + 12) as u8, i);
    }

    pub(crate) fn m2_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M2_BASE {
            0 => {
                self.mc2_patrol(i, M2_BASE);
                if self.ent[i].tick70 == M2_BASE + 2 {
                    self.ent[i].f26 = 1;
                }
            }
            1 => {
                self.mc2_idle(i, M2_BASE, ctx);
                if self.ent[i].tick70 == M2_BASE + 2 {
                    self.m2_yelp(i);
                    self.ent[i].f26 = 1;
                }
            }
            2 => {
                // sub_1F6D0 (:11490): lunge speed on the countdown's
                // last tick, vertical homing, chase w/ 1024-melee.
                if self.ent[i].f26 != 0 {
                    let v2 = self.ent[i].f26;
                    self.ent[i].f26 = v2 - 1;
                    if v2 == 1 {
                        self.ent[i].f126 = 5 * self.ent[i].f128 / 2; // 160
                    }
                }
                if self.ent[i].f146 != 0 {
                    // Vertical homing toward the target's top
                    // (:11509-20).
                    //
                    // ⭐ THE TOP IS `position.z + array_0x52_82.yaw`
                    // FOR EVERY TARGET, THE HUMAN INCLUDED. NETHERW.EXE
                    // 0x43f1e-0x43f2c is `movswl 0x50(%eax)` +
                    // `movswl 0x52(%eax)` + `add` + `sub` off the RAW
                    // `Entities_EA3E4[word_0x96_150]` record — the
                    // human's carpet is just another pool record there,
                    // so its `+0x52` lift is in the sum like anyone
                    // else's. Our human lives OUTSIDE the pool, so the
                    // lift rides `PLAYER_HH` (=100, and the MC2 carpet
                    // record's measured `ayaw` is exactly 100 — see
                    // `mc1::combat::PLAYER_HH`), the same constant
                    // `mc2_rebound_deflect`, `mc2_proj_land` and
                    // `mc2_autoaim` already add to `ctx.pz`.
                    // Dropping it flips the servo's sign whenever the
                    // goat sits within 100 units above the carpet:
                    // retail lifts +32, we sank -32, 64 per tick.
                    // `MGC_NO_M2_PLAYER_LIFT=1` restores the old
                    // bare-`ctx.pz` stand-in.
                    //
                    // ⭐⭐⭐ THE RECORD IS READ RAW — NO LIFE, NO REAP,
                    // NO CLASS TEST. `sub_1F6D0` dereferences
                    // `Entities_EA3E4[word_0x96_150]` and reads
                    // `position.z + array_0x52_82.yaw` straight
                    // (EF:11507-20): in NETHERW.EXE 0x43f17
                    // (`mov 0x1a3e4(,%eax,4),%eax`) runs into the
                    // `movswl 0x50/0x52` pair and the z store at
                    // 0x43f5d with ZERO conditional branches between.
                    // The liveness test belongs to `sub_1C310` /
                    // `sub_1ED30` (EF:9295-9301), ONE CALL LATER at
                    // EF:11521 — by which time this lift has landed.
                    // Asking `mc2_target` here (which rejects on
                    // `act_life < 0`) skipped the lift the tick a
                    // victim died, so the altitude servo's -32 went
                    // uncancelled and every goat sank 32 units:
                    // mc2l22 pair 72->73, 25 z rows at -32 on slots
                    // 13..45 the tick rival wizard 477 hit life -625.
                    // That was the take's ENTIRE horizon wall.
                    // ⚠ The `pdead` gate `mc2_target` carries for the
                    // human is a law of the CHASE site (EF:16360), not
                    // of this one.
                    let tslot = self.ent[i].f146 as usize;
                    if self.ent[i].f146 == PLAYER_TARGET || tslot < self.ent.len() {
                        let top = if self.ent[i].f146 == PLAYER_TARGET {
                            if m2_player_lift() {
                                ctx.pz.wrapping_add(crate::mc1::combat::PLAYER_HH as i16)
                            } else {
                                ctx.pz
                            }
                        } else {
                            let t = &self.ent[tslot];
                            t.z.wrapping_add(t.f78 as i16)
                        };
                        let v4 = (self.ent[i].z - top).signum();
                        let step = BEHAVIOR[self.ent[i].row156 as usize].v_14;
                        self.ent[i].z = self.ent[i].z.wrapping_add(v4 * step);
                    }
                    if self.mc2_chase_attack(i, M2_BASE, ctx, Self::mc2_atk_melee_1024) {
                        self.m2_yelp(i);
                        self.ent[i].f126 = -self.ent[i].f130; // recoil (:11525)
                        self.ent[i].f26 = 3 * BEHAVIOR[self.ent[i].row156 as usize].v_26;
                    }
                } else {
                    self.ent[i].tick70 = M2_BASE + 1;
                }
                if self.ent[i].tick70 != M2_BASE + 2 {
                    self.ent[i].f126 = self.ent[i].f128;
                }
            }
            3 => {
                self.mc2_pack(i, M2_BASE);
                if self.ent[i].tick70 == M2_BASE + 2 {
                    self.ent[i].f26 = 1;
                }
            }
            4 => self.mc2_prekill(i, M2_BASE),
            5 => self.mc2_kill(i),
            6 => {} // no body in the decompile; unreachable (row 73 flee bit clear)
            _ => {
                // +7 (:11563): the StageVar2 1..=9 wander jiggle never
                // fires for our StageVar2==0 spawns.
                if self.ent[i].tick70 == M2_BASE + 2 {
                    self.ent[i].f26 = 1;
                }
            }
        }
    }

    // =========================================================================
    // MODEL 9 — the hive imp (ctor sub_4BBB0 EF:33912, states
    // 0x48-4F; the most-authored creature in the campaign)
    // =========================================================================

    pub(crate) fn mc2_spawn_m9(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 9;
            e.tick70 = M9_BASE; // 72 — spawns into the materialize countdown
            e.f28 = 1;
            e.f128 = 20;
            e.f130 = 0;
            e.max_life = 1000;
            e.f126 = 20;
        }
        self.mc2_set_mana_half(i); // 500
        // ONE draw, modulus 0x832 (NOT the 0x7FF mask — verbatim,
        // :33937).
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            let v5 = ((d % 0x832) as i32 - 1) as u16;
            e.f34 = v5;
            e.f30 = v5;
            e.f32 = v5;
            e.f44 = 500;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 80;
            e.f66 = 3; // xtype_0x41_65 = 3 (EF:33947)
        }
        let ord = self.mc2_ord(9);
        self.ent[i].f63 = ord;
        self.ent[i].f26 = 16; // the materialize countdown (:33948)
        self.ent[i].f58 = Self::mc2_wake_stagger(80, ord);
        let gz = self.ground_z(x, y) as i16;
        self.link(i, x, y, gz); // :33951 ground snap
        self.refill_life(i);
        self.mc2_set_sprite(i, 220);
        self.mc2_shift_rot(i, 128, 128);
        // Blocked-placement despawn (:33955-59).
        if self.mc2_path_blocked(i, (x, y, gz)) {
            self.free_entity(i);
            return None;
        }
        Some(i)
    }

    /// `sub_20EC0` (:12283) — the engage pose: stop, sprite 202,
    /// filter = target's class/model; targeting self resets to idle.
    pub(crate) fn m9_engage_pose(&mut self, i: usize) {
        self.ent[i].f126 = 0;
        self.mc2_set_sprite(i, 202);
        let t = self.ent[i].f146;
        if t == i as u16 {
            self.ent[i].tick70 = M9_BASE + 1;
            return;
        }
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

    /// `sub_20F20` (:11988) — the walk pose.
    fn m9_walk_pose(&mut self, i: usize) {
        self.ent[i].f126 = self.ent[i].f128;
        self.mc2_set_sprite(i, 201);
        self.ent[i].f66 = 3;
        self.ent[i].f67 = 0xFF;
        self.ent[i].f26 = 50;
        self.ent[i].f71 = 0;
    }

    /// The hive's prey-consumption sweep (:12196-12218 / :12399-415):
    /// bucket = model {4, 12, 13} by `(f63 / v26) % 3`; a victim
    /// within 0x600 is consumed and a NEW (5,9) materializes there.
    ///
    /// ⭐ THE NEW HIVE INHERITS ITS PARENT'S `id_0x1A_26` — and the two
    /// retail bodies this one helper serves DISAGREE about when.
    /// `grounded` selects the arm: `sub_20940` (the grounded hive,
    /// EF:12409-11) copies unconditionally, `sub_203D0` (the walking
    /// hive, EF:12213-16) only when `Entities[parent->id]->class == 3`.
    /// Both byte-verified — see [`Gen::m9_split_inherits_id_law`],
    /// which is also the `MGC_NO_M9_SPLIT_INHERITS_ID` kill switch.
    pub(crate) fn m9_consume_scan(&mut self, i: usize, grounded: bool) {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let sel = [4u8, 12, 13][((self.ent[i].f63 as i16 / row.v_26.max(1)) % 3) as usize];
        let (ex, ey, ez) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let mut best: Option<(usize, i32)> = None;
        // ⭐⭐ THE FOOD SCAN WALKS THE TICK-TOP ROSTER CHAIN, NOT THE
        // POOL. Both bodies load `bytearray_38403x[16/4]`, `[48/4]`
        // and `[52/4]` — models 4, 12, 13, exactly this `sel` — and
        // chase `next_0` (sub_203D0 EF:12225/12243/12263, sub_20940
        // EF:12418/12435/12454). The loop body is the two distance
        // tests and NOTHING else; see [`Gen::mc2_roster`].
        if crate::engine::features::no_mc2_mob_chain_predicate() {
            for (j, c) in self.ent.iter().enumerate().skip(1) {
                if c.class64 == 5
                    && c.model65 == sel
                    && c.act_life >= 0
                    && c.flags & 0x400 == 0
                    && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                {
                    let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                    if d2 <= range && best.is_none_or(|(_, bd)| d2 < bd) {
                        best = Some((j, d2));
                    }
                }
            }
        } else {
            for k in 0..self.mc2_roster(sel).len() {
                let j = self.mc2_roster(sel)[k] as usize;
                let c = &self.ent[j];
                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                if d2 <= range && best.is_none_or(|(_, bd)| d2 < bd) {
                    best = Some((j, d2));
                }
            }
        }
        if let Some((j, _)) = best {
            let (vx, vy, vz) = (self.ent[j].x, self.ent[j].y, self.ent[j].z);
            if Self::mc2_dist3((ex, ey, ez), (vx, vy, vz)) <= 0x600 {
                self.ent[j].flags |= 0x400; // consumed
                let child = self.mc2_spawn_m9(vx, vy, vz); // the hive splits
                if let Some(c) = child {
                    if Self::m9_split_inherits_id_law() {
                        let pid = self.ent[i].id24;
                        let pass = grounded
                            || ((pid as usize) < self.ent.len()
                                && self.ent[pid as usize].class64 == 3);
                        if pass {
                            self.ent[c].id24 = pid;
                        }
                    }
                }
            }
        }
    }

    /// The awake cone scan of the m9 brain (:12159-93): the walk is
    /// over `dword_38519` — the CLASS-3 chain (wizards, castles,
    /// balloons; no model filter, :12164-68) — NOT the creature
    /// pool. Nearest in range + FOV, invisibility (byte[0] & 0x20)
    /// skipped; the id gate excuses the summoner's own things.
    fn m9_cone_scan(&self, i: usize, ctx: &MobCtx) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let (ex, ey, eyaw, my_id) = (e.x, e.y, e.f30, e.id24);
        let mut best: Option<(u16, i32)> = None;
        let mut consider = |tx: u16, ty: u16, slot: u16| {
            let d2 = Self::dist2_sq(ex, ey, tx, ty);
            if d2 > range {
                return;
            }
            if Self::angdist(eyaw, Self::angle_between(ex, ey, tx, ty)) >= cone {
                return;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((slot, d2));
            }
        };
        // ⚠ `pdead` is `dword_38519`'s ENTRY test (EF:39975), which the
        // pool arm below applies itself and the out-of-pool human
        // cannot — see [`Gen::mc2_wizard_scan`].
        if !self.player_invisible && !ctx.pdead {
            consider(ctx.px, ctx.py, PLAYER_TARGET);
        }
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if c.class64 == 3 && c.id24 != my_id && c.act_life >= 0 && c.flags & (0x400 | 0x20) == 0
            {
                consider(c.x, c.y, j as u16);
            }
        }
        best.map(|(s, _)| s)
    }

    pub(crate) fn m9_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M9_BASE {
            0 => {
                // sub_20370 (:11969): the materialize countdown.
                let v = self.ent[i].f26;
                self.ent[i].f26 = v - 1;
                if v != 0 {
                    if v - 1 < 16 && (v - 1) % 2 == 0 {
                        // sub_585A0 (EF:11983) — capped.
                        self.mc2_anim_step(i);
                    }
                } else {
                    self.m9_walk_pose(i);
                    self.ent[i].tick70 = M9_BASE + 1;
                    self.ent[i].f26 = 400;
                    self.ent[i].f71 = 0;
                }
            }
            1 => {
                // sub_203D0 (:11998) — the hive brain.
                if self.ent[i].f26 > 0 {
                    self.ent[i].f26 -= 1;
                    if self.ent[i].f26 == 0 {
                        // sub_20F60: grounded/summon posture.
                        self.mc2_set_sprite(i, 201);
                        self.ent[i].f71 = 1;
                    }
                }
                if self.ent[i].f71 != 0 {
                    // sub_20940 (EF:12291) — the GROUNDED variant.
                    //
                    // The damage/death head runs FIRST and
                    // short-circuits, exactly as in the walking arm
                    // (EF:12357-75). Omitting it made a grounded hive
                    // unkillable.
                    //
                    // ⭐⭐ AND `sub_20940` IS A CALLEE, NOT A TAIL —
                    // ITS EARLY RETURNS DO NOT SKIP `LABEL_85`.
                    // The dispatch is `v2 = a1x->byte_0x46_70;
                    // if (v2) { if (v2 == 1) sub_20940(a1x);
                    // goto LABEL_85; }` (EF:12055-60) and LABEL_85 is
                    // `if (actionIndex == 74) sub_20EC0(a1x);`
                    // (EF:12282-84). The port had inlined the callee
                    // and let its four `return`s leave `sub_203D0`
                    // outright, so a grounded hive that took an arrow
                    // flipped to 74 WITHOUT the engage pose: mc2l22
                    // pair 34198->34199, hive 259 hit for 250 by (9,13)
                    // slot 105 off archer 914 — retail action 73 -> 74
                    // AND sprite 201 -> 202, `xtype/xsubtype` 3/255 ->
                    // 5/4 (the archer's class/model), `actSpeed` 20 ->
                    // 0; the port moved the action alone and kept the
                    // mound disguise. Same head at 34359 on slots 247
                    // and 268. A labelled block reproduces the `goto`.
                    let tail_law = m9_grounded_pose_tail();
                    'grounded: {
                        match self.mc2_state_head(i) {
                            1 => {
                                self.ent[i].f146 = self.ent[i].f40;
                                self.ent[i].tick70 = M9_BASE + 2; // action 74
                                if tail_law {
                                    break 'grounded;
                                }
                                return;
                            }
                            2 => {
                                self.ent[i].tick70 = M9_BASE + 4; // action 76
                                if tail_law {
                                    break 'grounded;
                                }
                                return;
                            }
                            _ => {}
                        }
                        // EF:12377-84 — the stand-up counts UP toward 0 and
                        // only the tick that READS -1 fires sub_20F80
                        // (EF:12638: f71 = 0, f26 = 400, sprite 201). No
                        // consume sweep runs during it.
                        let v7 = self.ent[i].f26;
                        if v7 < 0 {
                            self.ent[i].f26 = v7 + 1;
                            if v7 == -1 {
                                self.mc2_set_sprite(i, 201);
                                self.ent[i].f71 = 0;
                                self.ent[i].f26 = 400;
                            }
                            if tail_law {
                                break 'grounded;
                            }
                            return;
                        }
                        // EF:12385-89 — an AWAKE hive arms the 50-tick
                        // stand-up and scans nothing this tick. The player
                        // being near is what stands the hive back up.
                        if self.ent[i].f58 != 0 {
                            self.ent[i].f26 = -50;
                            if tail_law {
                                break 'grounded;
                            }
                            return;
                        }
                        // Asleep: f26 stays parked at 0, so the hive squats
                        // and feeds in place indefinitely — retail never
                        // walks a hive that no wizard has approached.
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                        if self.ent[i].f63 as i16 % period == 0 {
                            self.m9_consume_scan(i, true); // sub_20940 arm: unconditional id copy
                        }
                    }
                    // LABEL_85 (EF:12282-84).
                    if self.ent[i].tick70 == M9_BASE + 2 {
                        self.m9_engage_pose(i);
                    }
                    return;
                }
                if self.ent[i].f58 != 0 {
                    self.ent[i].f26 = 400;
                }
                match self.mc2_state_head(i) {
                    1 => {
                        self.ent[i].f146 = self.ent[i].f40;
                        self.ent[i].tick70 = M9_BASE + 2;
                    }
                    2 => self.ent[i].tick70 = M9_BASE + 4,
                    _ => {
                        self.mc2_move_core(i);
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                        if self.ent[i].f63 as i16 % period == 0 {
                            // Prey seek (:12117-48): the nearest
                            // model-2 on the `dword_38519` chain —
                            // the CLASS-3 chain, so the prey is a
                            // CASTLE, not the (5,2) creature. The
                            // skeleton FACES it unconditionally at
                            // ANY distance (:12137 — the map-wide
                            // castle march; on mc2:04 the channel
                            // bank + the move-core retries funnel
                            // the column onto the authored ford
                            // straight past the archer island) and
                            // only CHASES within pitch + v_28 (3D,
                            // :12138-39). The id skip (:12121) also
                            // excuses a wizard-summoned skeleton
                            // from besieging its owner's castle.
                            let (ex, ey, ez) = {
                                let e = &self.ent[i];
                                (e.x, e.y, e.z)
                            };
                            let my_id = self.ent[i].id24;
                            let row = &BEHAVIOR[self.ent[i].row156 as usize];
                            // ⭐⭐ THE PREY SCAN WALKS THE TICK-TOP
                            // CLASS-3 ROSTER `dword_38519`, NOT THE
                            // LIVE POOL, AND IT ASKS NOTHING ABOUT
                            // LIFE. The shipped EXE settles it at
                            // `sub_203D0` +0x20502 (file 0x44D02):
                            //   mov esi,[0x41a4] / mov esi,[esi+0x9677]
                            //   cmp byte [esi+0x40],2   ; model
                            //   mov ax,[esi+0x1a] / cmp ax,[ebx+0x1a]
                            //   ...  / cmp eax,edx / jnc  ; d2 < best
                            //   mov esi,[esi]           ; ->next_0
                            // THREE admission tests and no others —
                            // no `life >= 0`, no reap-flag 0x400. Every
                            // liveness question was settled when the
                            // roster was BUILT at the top of the frame,
                            // exactly as in [`Gen::mc2_avoid_packmate_at`].
                            //
                            // The port's live-pool walk saw a castle
                            // BORN EARLIER IN THE SAME TICK and took the
                            // absolute face-the-prey `roll` write where
                            // retail, finding the roster empty, took the
                            // two-draw wander turn: mc2l5 t=32531,
                            // skeleton 169 — retail `rand` 25598 → 47772
                            // (exactly two LCG steps) and `roll` 287 →
                            // 46 = 287 − ((47772 & 0xFF) + 85); the port
                            // drew nothing and wrote `roll` = 617, the
                            // bearing to slot 86, a (3,2) castle born at
                            // t=32531 and therefore absent from the
                            // roster retail was walking. ⚠ `roll` (@0x20)
                            // is UNGRADED, so the report names `rand`.
                            let mut prey: Option<(usize, i32)> = None;
                            if m9_prey_roster_law() {
                                for k in 0..self.wiz_chain.visible_len() {
                                    let j = self.wiz_chain.list[k] as usize;
                                    let c = &self.ent[j];
                                    if c.model65 == 2 && c.id24 != my_id {
                                        let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                        if best_d2(&prey, d2) {
                                            prey = Some((j, d2));
                                        }
                                    }
                                }
                            } else {
                                for (j, c) in self.ent.iter().enumerate().skip(1) {
                                    if c.class64 == 3
                                        && c.model65 == 2
                                        && c.id24 != my_id
                                        && c.act_life >= 0
                                        && c.flags & 0x400 == 0
                                    {
                                        let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                        if best_d2(&prey, d2) {
                                            prey = Some((j, d2));
                                        }
                                    }
                                }
                            }
                            // `v41x` — set only when a chase target
                            // was ACQUIRED this tick. A castle found
                            // but out of reach is DISCARDED (:12141)
                            // yet still steers the march, and the
                            // cone/convert scans run in its shadow;
                            // the random turn runs only with NO
                            // castle on the map at all (:12149-56).
                            let mut acquired = false;
                            if let Some((j, _)) = prey {
                                let (tx2, ty2, tz2, pitch) = {
                                    let c = &self.ent[j];
                                    (c.x, c.y, c.z, c.f82)
                                };
                                self.ent[i].f34 = Self::angle_between(ex, ey, tx2, ty2);
                                let reach = (pitch as i32 + row.v_28 as i32).max(0) as u32;
                                if Self::mc2_dist3((ex, ey, ez), (tx2, ty2, tz2)) <= reach {
                                    self.ent[i].f146 = j as u16;
                                    self.ent[i].tick70 = M9_BASE + 2;
                                    acquired = true;
                                }
                            } else {
                                self.mc2_wander_turn(i);
                            }
                            if !acquired {
                                if self.ent[i].f58 != 0 {
                                    if let Some(t) = self.m9_cone_scan(i, ctx) {
                                        self.ent[i].f146 = t;
                                        self.ent[i].tick70 = M9_BASE + 2;
                                        acquired = true;
                                    }
                                }
                                if !acquired {
                                    self.m9_consume_scan(i, false); // sub_203D0 arm: class-3 gated id copy
                                }
                            }
                        }
                    }
                }
                if self.ent[i].tick70 == M9_BASE + 2 {
                    self.m9_engage_pose(i);
                }
            }
            2 => {
                // sub_20C50 (:12476) — chase + arrow volley.
                match self.mc2_state_head(i) {
                    1 => self.ent[i].f146 = self.ent[i].f40,
                    2 => self.ent[i].tick70 = M9_BASE + 4,
                    _ => {
                        self.mc2_move_core(i);
                        let slot = self.ent[i].f146;
                        let Some((tx, ty, tz)) = self.mc2_target(slot, ctx) else {
                            self.ent[i].tick70 = M9_BASE + 1;
                            self.m9_walk_pose(i);
                            return;
                        };
                        if self.ent[i].f63 % 10 == 0 {
                            let e = &self.ent[i];
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                        }
                        let row = &BEHAVIOR[self.ent[i].row156 as usize];
                        let period = row.v_26.max(1);
                        // A castle target extends the ring by its
                        // PITCH extent (:12551 — array_0x52_82.pitch,
                        // the second of the extent trio).
                        let range = row.v_28 as u32
                            + if slot != PLAYER_TARGET
                                && (slot as usize) < self.ent.len()
                                && self.ent[slot as usize].class64 == 3
                                && self.ent[slot as usize].model65 == 2
                            {
                                self.ent[slot as usize].f82 as u32
                            } else {
                                0
                            };
                        if self.ent[i].f63 as i16 % period == 0 {
                            let e = &self.ent[i];
                            if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz)) < range {
                                self.mc2_atk_arrow(i, slot, ctx);
                            } else {
                                self.ent[i].tick70 = M9_BASE + 1;
                            }
                        }
                    }
                }
                if self.ent[i].tick70 != M9_BASE + 2 {
                    self.m9_walk_pose(i);
                }
            }
            3 => {
                self.mc2_pack(i, M9_BASE);
                if self.ent[i].tick70 == M9_BASE + 2 {
                    self.m9_engage_pose(i);
                }
            }
            4 => self.mc2_prekill(i, M9_BASE),
            5 => self.mc2_kill(i),
            6 => {} // missing body — unreachable
            _ => {
                if self.ent[i].tick70 == M9_BASE + 2 {
                    self.m9_engage_pose(i);
                }
            }
        }
    }

    // =========================================================================
    // MODEL 12 — the builder (ctor sub_4BDF0 EF:33999, states
    // 0x60-67; completing a building RETIRES it into the villager
    // brain, actionIndex 105)
    // =========================================================================

    pub(crate) fn mc2_spawn_m12(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 12;
            e.tick70 = M12_BASE + 1; // 97
            e.f28 = 1;
            e.f130 = 24;
            e.f126 = 24;
            e.f128 = 54;
            e.max_life = 1000;
        }
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f140 = 0;
            e.f36 = 0;
            e.f44 = 500;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 101;
            e.f58 = 64;
            e.f66 = 3; // xtype_0x41_65 = 3 (EF:34026)
            e.f26 = 2;
        }
        self.ent[i].f63 = self.mc2_ord(12);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 221);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// State head with the townie wanted-timer stamp on wizard
    /// offenses (shared by m12/m14, :14186-94 pattern). Returns the
    /// head code.
    fn mc2_head_wanted(&mut self, i: usize) -> u8 {
        let v = self.mc2_state_head(i);
        if v != 0 {
            let src = if v == 2 {
                self.ent[i].f38
            } else {
                self.ent[i].f40
            };
            if self.mc2_is_wizard(src) {
                self.mc2_arm_wanted(src);
            }
        }
        v
    }

    /// `sub_232C0` (:14474): the GLOBAL-LCG building-template pick —
    /// `rand % 0x3C + 17`, then walk up to 0x4D slots for a
    /// townie-flagged bldgprm row (byte_2 & 2). The walk wraps the
    /// LOW BYTE at 0x4C back to 17 (EF:14489-91) and exhaustion
    /// returns 17, never a failure (EF:14493). Retail accepts on
    /// byte_2 & 2 alone (no extra build_tab gate).
    pub(crate) fn m12_pick_template(&mut self) -> u16 {
        self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
        let mut pick = (self.rand % 0x3C + 17) as usize;
        for _ in 0..0x4D {
            if self
                .assets
                .bldgprm
                .get(pick)
                .is_some_and(|p| p.flags & 2 != 0)
            {
                return pick as u16;
            }
            pick = (pick + 1) & 0xFF;
            if pick >= 0x4C {
                pick = 17;
            }
        }
        17
    }

    pub(crate) fn m12_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M12_BASE {
            0 => self.mc2_m12_build(i),
            1 => {
                // sub_22C80 (:14118) — roam.
                // ⭐ :14128 is `a1x->fontTypeIndex_0x3D_61 = 0`, i.e.
                // @0x3D = the port's `f46`, NOT @0x46 = `f71`. The
                // whole of `sub_22C80` never touches `byte_0x46_70`.
                // It is the RELEASE half of the one-house latch
                // `sub_22760` sets at :14105 (see `mc2_m12_build`):
                // a builder retires to action 105 holding @0x3D = 1
                // and blocks every villager's build until it re-enters
                // roam. Homed on @0x46 the latch could never clear —
                // and the clear was landing on a graded lane that is
                // the class-5 SUB-STATE byte.
                if m12_site_vetos() {
                    self.ent[i].f46 = 0;
                } else {
                    self.ent[i].f71 = 0;
                }
                match self.mc2_head_wanted(i) {
                    1 => {
                        self.ent[i].f146 = self.ent[i].f40;
                        self.ent[i].tick70 = M12_BASE + 6;
                    }
                    2 => self.ent[i].tick70 = M12_BASE + 4,
                    _ => {
                        self.mc2_move_core(i);
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                        if self.ent[i].f63 % period == 0 {
                            self.mc2_wander_turn(i);
                            // :14195-99 — the roam counter test reads
                            // the PRE-decrement value and compares
                            // `== 0`, so the villager spends one MORE
                            // period-hit roaming than the post-form
                            // allows (the ctor's 2 buys three hits,
                            // the state re-entries' 5 buys six).
                            let pre = self.ent[i].f26;
                            self.ent[i].f26 = pre - 1;
                            if pre == 0 {
                                self.ent[i].tick70 = M12_BASE + 3;
                                self.ent[i].f26 = 1;
                            }
                        }
                    }
                }
                if self.ent[i].tick70 == M12_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                }
            }
            2 => {
                // sub_22E60 (:14216) — walk to the chosen site.
                match self.mc2_head_wanted(i) {
                    1 => {
                        self.ent[i].f146 = self.ent[i].f40;
                        self.ent[i].tick70 = M12_BASE + 6;
                    }
                    2 => self.ent[i].tick70 = M12_BASE + 4,
                    _ => {
                        self.mc2_move_core(i);
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                        if (self.ent[i].f63 as i16 % period) / 2 == 0 {
                            // ⭐ :14290-92 READS THE **PRE**-DECREMENT
                            // COUNTER (`v7 = a1x->dword_0x10_16;
                            // a1x->dword_0x10_16 = v7 - 1;
                            // if (!v7 || ...)`), the same idiom the roam
                            // brain above already carries.
                            let pre = self.ent[i].f26;
                            self.ent[i].f26 = pre - 1;
                            // :14294 latches the site pointer BEFORE
                            // the decrement, and an out-of-pool handle
                            // lands on `Entities_EA3E4[0]`, the class-0
                            // sentinel — so resolve the slot once, up
                            // front, and let both the liveness test and
                            // the aim below read the same record.
                            let t = self.ent[i].f146 as usize;
                            let t = if t < self.ent.len() { t } else { 0 };
                            // ⭐⭐ RETAIL'S SITE TEST IS `!v8x->class_0x3F_63`
                            // AND NOTHING ELSE (:14292). The extra
                            // `flags & 0x400 == 0` was an INVENTED GUARD
                            // and it is false exactly when it matters: a
                            // (10,45) shrine is reap-flagged the tick it
                            // dies but keeps its class until the free
                            // pass recycles it, so retail's villager
                            // walks on while the port bailed to 97 with
                            // @0x10 = 5. mc2l22 pair 18061->18062 slot
                            // 837 (site 75, a (10,45) whose reap bit
                            // sets ON that tick): retail action 98 /
                            // @0x10 0, port 97 / 5.
                            let live = t != 0
                                && self.ent[t].class64 != 0
                                && (m12_site_reap_blind() || self.ent[t].flags & 0x400 == 0);
                            if pre == 0 || !live {
                                self.ent[i].f26 = 5;
                                self.ent[i].tick70 = M12_BASE + 1;
                            }
                            // ⚠ THE RE-AIM AND THE ARRIVAL TEST ARE NOT
                            // IN AN `else`. Retail's give-up arm
                            // (:14296-300) is a BARE `if`: it resets the
                            // counter and flips to 97, then FALLS
                            // THROUGH (:14301-09) to
                            // `roll_0x20_32 = tan2(self, site)` and the
                            // `< 0xA00` arrival test regardless — so the
                            // walk's LAST tick still re-aims, and can
                            // still arrive (96 overrides the 97 flip).
                            // Gating them behind the live arm left a
                            // STALE @0x20 for the roam brain to carry:
                            // mc2l0-spells-galore t=3848 retail re-aimed
                            // 625 -> 617 while the port held 625, the
                            // t=3864 wander turn added the same +219 to
                            // both (836 vs 844), and at t=3891 the
                            // move-core yaw servo (row 101 `v_2` = 22)
                            // clamped onto 844 where retail parked on
                            // 836 — 43 ticks of an UNGRADED lane before
                            // the graded `heading` could see it.
                            let (tx, ty, tz) = {
                                let s = &self.ent[t];
                                (s.x, s.y, s.z)
                            };
                            let e = &self.ent[i];
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                            let e = &self.ent[i];
                            if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz)) < 0xA00 {
                                self.ent[i].tick70 = M12_BASE;
                                self.ent[i].f26 = 0;
                            }
                        }
                    }
                }
                if self.ent[i].tick70 == M12_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                }
            }
            3 => {
                // sub_23020 (:14323) — pick the nearest building as
                // the anchor to build near.
                match self.mc2_head_wanted(i) {
                    1 => {
                        self.ent[i].f146 = self.ent[i].f40;
                        self.ent[i].tick70 = M12_BASE + 6;
                    }
                    2 => self.ent[i].tick70 = M12_BASE + 4,
                    _ => {
                        // ⭐⭐ :14395-99 — THE ANCHOR PICK IS A **3-D**
                        // NEAREST OVER THE **BUILDING CHAIN**, NOT A
                        // 2-D POOL SCAN.
                        //
                        // `v9 = sub_583F0_distance_3d(&a1x->position_0x4C_76,
                        //  &jx->position_0x4C_76); if (v9 && v9 < v11)`
                        // — `sub_583F0` (:40421) is
                        // `radix_3d(dy² + dx² + **dz²**)`, a TRUNCATED
                        // isqrt; `v11` starts at unsigned −1 (:14337);
                        // the `v9 &&` drops a coincident record and the
                        // strict `<` resolves equal roundings to the
                        // EARLIER chain entry. The walk is
                        // `jx = dword_38527; jx > Entities_EA3E4[0];
                        // jx = jx->next_0` (:14395) — the TICK-TOP
                        // building roster ([`Gen::bldg_chain`]), which
                        // carries no class/model/flags test of its own
                        // and severs at a slot freed earlier in the
                        // tick.
                        //
                        // ⚠ Retail uses BOTH forms deliberately: the
                        // m14 townie's walk (:14617-25) runs the same
                        // chain with a genuinely 2-D `dx² + dy²`. Only
                        // `sub_23020` is the 3-D one.
                        //
                        // mc2l0-spells-galore t=4360, builder slot 430
                        // at (15254, 50184, 3567), 42 live (10,45)
                        // houses: slot 35 at (16128, 48640, 5152) is
                        // the 2-D nearest (1774 v 1900) and slot 25 at
                        // (16384, 51712, 4992) is the 3-D nearest
                        // (2375 v 2379) — a FOUR-unit margin the
                        // dropped z axis inverts. The port anchored on
                        // 35, so the t=4376 arrival re-aim wrote `roll`
                        // 105 (the bearing to 35) where retail wrote
                        // 862 (the bearing to 25); six ticks later the
                        // roam brain's heading servo (row 101 `v_2` =
                        // 22) stepped −22 against retail's +22 and the
                        // graded `heading` finally showed it at t=4382
                        // — 21 ticks and two ungraded lanes downstream.
                        //
                        // ⭐ MC1's m12 SEEK has carried this exact
                        // helper all along
                        // (`Gen::nearest_building_3d`, mc1/mobs.rs:2431
                        // — 3-axis, truncated isqrt, strict `<`,
                        // `d != 0` skip, its doc comment already
                        // quoting `if (v10 && v10 < v1)`). The TENTH
                        // MC2-is-the-laggard case of the campaign.
                        let (ex, ey, ez) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z)
                        };
                        let mut best: Option<(usize, u32)> = None;
                        for c in 0..self.bldg_chain.visible_len() {
                            let j = self.bldg_chain.list[c] as usize;
                            let b = &self.ent[j];
                            let d = Self::mc2_dist3((ex, ey, ez), (b.x, b.y, b.z));
                            if d != 0 && best.is_none_or(|(_, bd)| d < bd) {
                                best = Some((j, d));
                            }
                        }
                        if let Some((j, _)) = best {
                            self.ent[i].f146 = j as u16;
                            self.ent[i].f26 = 10;
                            self.ent[i].tick70 = M12_BASE + 2;
                        } else {
                            self.ent[i].f26 = 5;
                            self.ent[i].tick70 = M12_BASE + 1;
                        }
                    }
                }
                if self.ent[i].tick70 == M12_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                }
            }
            4 => self.mc2_prekill(i, M12_BASE),
            5 => {
                // `sub_23200`/`sub_23B30` arm the KILLER's wizard,
                // whoever it is (EF:14440-42 / 14913-16) —
                // `wanted_any_wizard`.
                let killer = self.ent[i].f38;
                if if wanted_any_wizard() {
                    self.mc2_is_wizard(killer)
                } else {
                    killer == PLAYER_TARGET
                } {
                    self.mc2_arm_wanted(killer);
                }
                self.mc2_kill(i);
            }
            6 => {
                self.mc2_flee(i, M12_BASE, ctx);
                if self.ent[i].tick70 != M12_BASE + 6 {
                    self.ent[i].f26 = 5;
                    self.ent[i].f146 = 0;
                    self.ent[i].tick70 = M12_BASE + 1;
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
            _ => {
                // +7 (:14466): respawn straight into the roam brain.
                self.ent[i].f26 = 5;
                self.ent[i].tick70 = M12_BASE + 1;
                self.m12_tick(i, ctx);
            }
        }
    }

    /// `sub_22760` (:13942) — the build-execute state: jitter a
    /// candidate around the anchor building, clear-check, place a
    /// (10,45) and retire into the villager brain. The jitter /
    /// footprint-clear scans are shaped from the trace, not verbatim
    /// (deliberate).
    fn mc2_m12_build(&mut self, i: usize) {
        let t = self.ent[i].f146 as usize;
        // :13969 is `if (!v1x->class_0x3F_63 || v1x->model_0x40_64 != 45)`
        // — CLASS-NONZERO plus MODEL 45, with NO reap-bit test (twin of
        // the `sub_22E60` walk above; `m12_site_reap_blind`).
        let anchor_ok = t != 0
            && t < self.ent.len()
            && self.ent[t].class64 == 10
            && self.ent[t].model65 == 45
            && (m12_site_reap_blind() || self.ent[t].flags & 0x400 == 0);
        if !anchor_ok {
            self.ent[i].f26 = 5;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = M12_BASE + 1;
            return;
        }
        let v2 = self.ent[i].f26;
        self.ent[i].f26 = v2 + 1;
        if v2 >= 4 {
            self.ent[i].f26 = 1;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = M12_BASE + 1;
            return;
        }
        let pick = self.m12_pick_template();
        let (w, h) = self
            .assets
            .build_tab
            .get(pick as usize)
            .map_or((2u16, 2u16), |d| (d.w as u16, d.h as u16));
        // Candidate: one of four sides of the anchor by pass number,
        // jittered by two draws (cases 1-4, :13991-14024).
        let (ax, ay, az) = {
            let s = &self.ent[t];
            (s.x, s.y, s.z)
        };
        let d1 = ((self.mc2_rand(i) % 3) << 8) as i32;
        let d2 = ((self.mc2_rand(i) % 3) << 8) as i32;
        // ⭐⭐ :13991 SWITCHES ON THE **POST**-INCREMENT PASS COUNTER.
        // `switch (a1x->dword_0x10_16)` runs AFTER
        // `a1x->dword_0x10_16 = v2 + 1` (:13982), so the four passes
        // the `v2 >= 4` give-up budgets (:13983) are sides 1,2,3,4 in
        // that order. Matching the PRE-increment value rotated them to
        // 4,1,2,3 — the second attempt took the EAST face where retail
        // takes the WEST one, and side 4 was never reached at all.
        // (`switch(v2)` cannot be the original: an arrival enters this
        // state with @0x10 = 0, which would fall to `default:` and site
        // the house on the anchor's own centre.)
        //
        // Sides 1/2 straddle X off the anchor's PITCH extent
        // (:13996/:14003); sides 3/4 straddle Y off its ROLL extent
        // (:14013/:14020). And in EVERY case the FIRST draw jitters X
        // and the SECOND jitters Y — the port had d1/d2 swapped on 3/4
        // and read the pitch/width pair there as well.
        //
        // mc2l0-spells-galore t=4282: builder slot 430, @0x10 1 -> 2,
        // anchor (10,45) slot 25 — the port minted a (10,45) at slot
        // 478 off the east candidate while retail's blocked west one
        // left the pool alone (`extra(10,45)slot478x1`).
        let apitch = self.ent[t].f80 as i32;
        let aroll = self.ent[t].f82 as i32;
        // `sub_226D0` (:13933/:13935): the pass extent is the template
        // HALF-size plus a flat 768 (3-tile) clearance —
        // `*exwidth = (width_4 << 8) / 2 + 768`. `(w << 7)` was only
        // the first term, so every candidate sat 3 tiles too close to
        // the anchor. ⚠ Not modelled: :13928 halves both extents when
        // `x_WORD_180660_VGA_type_resolution == 1`.
        let exw = ((w as i32) << 7) + 768;
        let exh = ((h as i32) << 7) + 768;
        let (cx, cy) = match v2 + 1 {
            1 => (
                (ax as i32 + d1 + apitch + exw + 256) as u16,
                (ay as i32 + d2 - 1280) as u16,
            ),
            2 => (
                (ax as i32 - (d1 + apitch + exw + 256)) as u16,
                (ay as i32 + d2 - 1280) as u16,
            ),
            3 => (
                (ax as i32 + d1 - 1280) as u16,
                (ay as i32 + d2 + aroll + exh + 256) as u16,
            ),
            _ => (
                (ax as i32 + d1 - 1280) as u16,
                (ay as i32 - (aroll + exh + d2 + 256)) as u16,
            ),
        };
        // Water veto (:14031-35).
        if self.cap_bit(cx, cy) == 1 {
            self.ent[i].f26 = 2;
            self.ent[i].f146 = 0;
            self.ent[i].tick70 = M12_BASE + 1;
            return;
        }
        // ⭐⭐ THE SITE TEST IS FLATNESS + AN **EXTENT BOX**, NOT A
        // FOOTPRINT-TILE WALK.
        //
        // `sub_22640` (:13906-16) first: the 4-corner max−min of the
        // inflated footprint under `((exw >> 7) + (exh >> 7) > 4) + 15`
        // (:14036-40). Then :14044-53 walks the BUILDING CHAIN
        // (`dword_38527` = `Gen::bldg_chain`) and rejects on
        // `|dx| <= ix.pitch + exwidth && |dy| <= exheight + ix.roll`
        // — each scanned building's OWN apitch/aroll widening the box.
        // Village dwellings carry apitch/aroll of 1536..2560, so the
        // real exclusion reaches ~15 tiles.
        //
        // The port walked only the candidate's own w×h tiles, a ±768
        // box that fires only when a building's single `link` cell
        // lands inside the footprint. mc2l0-spells-galore t=4282: the
        // west candidate (13886, 50944) is NINE TILES from live
        // (10,45) slot 35 at (16128, 48640) (apitch 2304, aroll 2560)
        // — dx 2242 ≤ 3840 and dy 2304 ≤ 3840, a wide margin — so
        // retail vetoes and the port built, minting the extra (10,45)
        // at slot 478 (`extra(10,45)slot478x1`).
        //
        // ⭐ MC1's `m12_build` HAS CARRIED BOTH GATES ALL ALONG
        // (mc1/mobs.rs: the 15/16 threshold and the
        // `dx <= c.f80 + half_x && dy <= c.f82 + half_y` box) — the
        // NINTH "grep MC1 before building for MC2" case.
        //
        // ⚠ Retail's :14056+ adds two more chain scans (class-2
        // model 2, class-2 model 67) and the model-12 `@0x3D` gate at
        // :14086-93; those stay unported and are still registered in
        // docs/DEVIATIONS.md.
        let thr = if (exh >> 7) + (exw >> 7) > 4 { 16 } else { 15 };
        if self.site_roughness(cx, cy, (exw >> 8) as u8, (exh >> 8) as u8) >= thr {
            return;
        }
        for c in 0..self.bldg_chain.visible_len() {
            let b = self.bldg_chain.list[c] as usize;
            let e = &self.ent[b];
            let dx = (e.x.wrapping_sub(cx) as i16 as i32).abs();
            if dx <= e.f80 as i32 + exw {
                let dy = (e.y.wrapping_sub(cy) as i16 as i32).abs();
                if dy <= exh + e.f82 as i32 {
                    return; // occupied — try again next tick
                }
            }
        }
        // ⭐⭐⭐ THE THREE MISSING VETO SCANS (:14056-93). Retail runs
        // FOUR `!v9` scans back to back, not one; the port carried only
        // the first. All three extras were registered in
        // docs/DEVIATIONS.md as "shaped" and none of them needs a
        // helper:
        //   b) `dword_38519` (`Gen::wiz_chain`, the tick-top CLASS-3
        //      roster, EF:39977-83 — the DEVIATIONS entry's "class-2"
        //      is wrong) filtered `model == 2` = THE CASTLE, same
        //      extent box. EXE `0x472e4: mov 0x9677(%esi),%esi` /
        //      `0x472f6: cmpb $0x2,0x40(%esi)`;
        //   c) ⚠ NOT PORTED, and the DECOMPILE MISNAMES ITS ROSTER:
        //      EF:14074 says `dword_38519`, but the shipped EXE reads
        //      `0x47357: mov 0x9687(%esi),%esi` = @0x9687 =
        //      **`dword_38535`** (@0x9677 is 38519; the singles run
        //      38519/23/27/31/35 at 0x9677/7B/7F/83/87), filtered
        //      `model == 0x43` (`0x4735f: cmpb $0x43,0x40(%esi)`).
        //      `dword_38535` is the class-10 {0x2A, 0x43, 0x4E} +
        //      class-11 {0x0C, 0x1F} bucket (EF:40032-40072), so this
        //      is the **(10,67)** flood tower, not a class-3 record —
        //      the port has no chain for that bucket yet, so the scan
        //      is left owed rather than faked off the pool.
        //   d) ⭐ `bytearray_38403x[48/4]` = the tick-top CLASS-5
        //      **MODEL-12** roster ([`Gen::mob_chains`] bucket 12) with
        //      NO POSITION TEST AT ALL: `if (lx->fontTypeIndex_0x3D_61)
        //      v9 = 1;` (:14088-90). One villager anywhere on the map
        //      carrying a nonzero @0x3D vetoes EVERY further build, by
        //      any villager, anywhere — and :14106 stamps exactly that
        //      byte on the builder the moment a house goes up. It is a
        //      ONE-HOUSE-PER-MAP-EPOCH latch, and the port had neither
        //      half: it never read the byte and never wrote it, so it
        //      kept minting dwellings for the rest of the take.
        //
        // @0x3D is the port's `f46` on class 5 and the importer already
        // carries it (`import_ent_mc2`, the `b3d -> f46` arm), so the
        // gate reads retail's own latch in the pair census.
        //
        // mc2l22: the port minted ~145 spurious `(10,45)`s from
        // t=17804 on, and EVERY ONE consumed a pool slot — which is
        // what phase-shifts the `(10,0)` meteor bursts by exactly one
        // allocation (t=18139/40/41, 25993, 30017, 31284, 54919: the
        // port emits retail's cell/jitter sequence VERBATIM, one slot
        // off, and spills the tail into the next free slot).
        if m12_site_vetos() {
            for c in 0..self.wiz_chain.visible_len() {
                let b = self.wiz_chain.list[c] as usize;
                let e = &self.ent[b];
                if e.model65 != 2 {
                    continue;
                }
                let dx = (e.x.wrapping_sub(cx) as i16 as i32).abs();
                if dx <= e.f80 as i32 + exw {
                    let dy = (e.y.wrapping_sub(cy) as i16 as i32).abs();
                    if dy <= exh + e.f82 as i32 {
                        return;
                    }
                }
            }
            for c in 0..self.mob_chains.visible(12).len() {
                let b = self.mob_chains.visible(12)[c] as usize;
                if self.ent[b].f46 != 0 {
                    return;
                }
            }
        }
        // Place it (:14096-106). ⚠ The retire is OUTSIDE retail's
        // `if (v8y)` (:14104-06) — a failed allocation still retires
        // the builder and still latches @0x3D.
        let placed = self.mc2_spawn_building(cx, cy, az, pick);
        if let Some(b) = placed {
            self.snd(10, i);
            self.ent[b].tick70 = 51;
        }
        if placed.is_none() && !m12_site_vetos() {
            return;
        }
        self.ent[i].f146 = 0;
        if m12_site_vetos() {
            self.ent[i].f46 = 1; // fontTypeIndex_0x3D_61 = 1 (:14105)
        }
        self.ent[i].tick70 = 105; // retire into the villager brain
    }

    // =========================================================================
    // MODEL 14 — the trader (ctor AddTrader_4C0B0 EF:34094, states
    // 0x70-77; passive, docks into far-away buildings)
    // =========================================================================

    pub(crate) fn mc2_spawn_m14(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 14;
            e.tick70 = M14_BASE + 1; // 113
            e.f28 = 1;
            e.f130 = 18;
            e.f126 = 18;
            e.f128 = 54;
            e.max_life = 1000;
        }
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f140 = 0;
            e.f36 = 0;
            e.f44 = 500;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 100;
            e.f58 = 64;
            e.f66 = 3; // xtype_0x41_65 = 3 (EF:34117)
            e.f26 = 2;
        }
        self.ent[i].f63 = self.mc2_ord(14);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 219);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    pub(crate) fn m14_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M14_BASE {
            0 | 2 | 3 => {
                self.ent[i].tick70 = M14_BASE + 1;
                self.m14_brain(i, ctx);
            }
            1 => self.m14_brain(i, ctx),
            4 => {
                // :14898 — docked traders vanish instead of dying.
                if self.ent[i].f26 != 0 {
                    self.ent[i].flags |= 0x400;
                } else {
                    self.mc2_prekill(i, M14_BASE);
                }
            }
            5 => {
                // `sub_23200`/`sub_23B30` arm the KILLER's wizard,
                // whoever it is (EF:14440-42 / 14913-16) —
                // `wanted_any_wizard`.
                let killer = self.ent[i].f38;
                if if wanted_any_wizard() {
                    self.mc2_is_wizard(killer)
                } else {
                    killer == PLAYER_TARGET
                } {
                    self.mc2_arm_wanted(killer);
                }
                self.mc2_kill(i);
            }
            6 => {
                self.mc2_flee(i, M14_BASE, ctx);
                if self.ent[i].tick70 != M14_BASE + 6 {
                    self.ent[i].f146 = 0;
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
            _ => {
                if self.ent[i].tick70 == M14_BASE + 6 {
                    self.ent[i].f126 = self.ent[i].f128;
                } else {
                    self.ent[i].f126 = self.ent[i].f130;
                }
            }
        }
    }

    /// `sub_237B0` (:14728) — the trader brain.
    fn m14_brain(&mut self, i: usize, ctx: &MobCtx) {
        match self.mc2_head_wanted(i) {
            1 => {
                self.ent[i].f146 = self.ent[i].f40;
                self.ent[i].tick70 = M14_BASE + 6;
            }
            2 => self.ent[i].tick70 = M14_BASE + 4,
            _ => {
                self.mc2_move_core(i);
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                if self.ent[i].f63 % period == 0 {
                    let t = self.ent[i].f146 as usize;
                    // ⭐⭐ RETAIL'S TARGET TEST IS `class == 10 &&
                    // model == 45` AND NOTHING ELSE (NETHERW.EXE
                    // 0x4822d/0x48233) — the reap bit is invisible to
                    // it, exactly as it is to the m12 site test
                    // ([`m12_site_reap_blind`]).
                    let building = t != 0
                        && t < self.ent.len()
                        && self.ent[t].class64 == 10
                        && self.ent[t].model65 == 45
                        && (townie_target_reap_blind() || self.ent[t].flags & 0x400 == 0);
                    // ⭐⭐⭐ `word_0x96_150 != 0` IS THE OUTER TEST
                    // (0x480ee `test %di,%di; jne 0x48221`): a
                    // non-zero handle NEVER reaches the wander turn or
                    // the far-dwelling scan, whatever it resolves to.
                    // An invalid one clears, takes `maxSpeed` and
                    // returns (0x48286-0x4829d).
                    let had_target = t != 0 && trader_invalid_target_return();
                    if building {
                        let (sp, tp) = {
                            let e = &self.ent[i];
                            let s = &self.ent[t];
                            ((e.x, e.y, e.z), (s.x, s.y, s.z))
                        };
                        if Self::mc2_dist3(sp, tp) > 0x800 {
                            self.ent[i].f34 = Self::angle_between(sp.0, sp.1, tp.0, tp.1);
                        } else if (self.ent[t].f128 as i32) > self.ent[t].f26 as i32 {
                            // Dock (:14820-27).
                            self.ent[i].f26 = 1;
                            self.ent[i].tick70 = M14_BASE + 4;
                            self.ent[t].f26 += 1;
                        } else {
                            self.ent[i].f146 = 0;
                            self.ent[i].f126 = self.ent[i].f130;
                        }
                    } else if had_target {
                        self.ent[i].f146 = 0;
                        self.ent[i].f126 = self.ent[i].f130;
                    } else {
                        self.ent[i].f146 = 0;
                        self.mc2_wander_turn(i);
                        // Seek a FAR trade building (bldgprm byte_2
                        // & 1, dist² > 0xE100000 — :14841-68).
                        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                        let mut best: Option<(usize, i32)> = None;
                        for (j, c) in self.ent.iter().enumerate().skip(1) {
                            if c.class64 == 10
                                && c.model65 == 45
                                && c.flags & 0x400 == 0
                                && self
                                    .assets
                                    .bldgprm
                                    .get(c.f71 as usize)
                                    .is_some_and(|p| p.flags & 1 != 0)
                            {
                                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                // 0xE100000 (~60 tiles, EF:14854).
                                if d2 > 0xE100000 && best_d2(&best, d2) {
                                    best = Some((j, d2));
                                }
                            }
                        }
                        if let Some((j, _)) = best {
                            self.ent[i].f146 = j as u16;
                            self.ent[i].f126 = self.ent[i].f130 + 12;
                        }
                    }
                }
                let _ = ctx;
            }
        }
        if self.ent[i].tick70 == M14_BASE + 6 {
            self.ent[i].f126 = self.ent[i].f128;
        }
    }

    // =========================================================================
    // MODEL 15 — the CASTLE GUARD archer (ctor sub_4C1E0 EF:34129,
    // states 0x78-7F; trace mc2-class5-m2-9-12-14-15.md §MODEL 15).
    // Never authored by any level: its one launch site is the castle
    // guard respawn (EF:61488).
    // =========================================================================

    /// `sub_4C1E0` (:34129) — ZERO ctor RNG draws; rotations
    /// hardcoded 0; mana 0 (no SetEvent144). The retail
    /// `struct_byte[2] |= 2` tracked-entity registration (:34153) has
    /// no reader in our port (sub_57F20's list isn't modeled).
    pub(crate) fn mc2_spawn_m15(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 15;
            e.tick70 = M15_BASE + 1; // actionIndex 121 (:34134)
            // MC2 carries NO per-channel vulnerability mask; admit
            // the physical channel at the seam (cross-column damage
            // contract).
            e.f28 = 1;
            e.f128 = 30; // minSpeed (:34137)
            e.f130 = 0; // maxSpeed (:34138)
            e.max_life = 1000;
            e.f126 = 30; // actSpeed = minSpeed (:34141)
            e.f34 = 0; // yaw = roll = pitch = 0 (:34140-43)
            e.f30 = 0;
            e.f32 = 0;
            e.f140 = 0; // mana (:34144)
            e.f36 = 0; // fov (:34145)
            e.f26 = (i % 100) as i16; // (:34146)
            e.f44 = 500; // subSpellIndex (:34147)
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.  // byte_0x38_56 (:34148)
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 83; // (:34149)
            e.f66 = 3; // xtype (:34151)
            // ⭐ THE SACRIFICABLE BIT (`struct_byte_0xc_12_15.byte[2] |= 2`,
            // EF:34153). `sub_4C1E0` is the ONLY class-5 creator in the
            // game that sets it. `sub_49F90`'s victim half ranks every
            // record carrying it (`NETHERW.EXE` 0x6E819
            // `testb $0x2,0xe(%eax)`, inside the descending 999->1 scan at
            // 0x6E7CC) and `NewEvent_4A050` (Events.cpp:581) seizes off
            // that list once the free stack is dry.
            // ⚠ The doc that used to sit on this ctor said the bit "has no
            // reader in our port (sub_57F20's list isn't modeled)". That is
            // FALSE today: `Gen::rebuild_recycle` reads mask 0x2_0000,
            // `Gen::mc2_recycle_pop` seizes off it, and `Gen::free_entity`'s
            // swap_remove IS `sub_57F20`'s list removal.
            e.flags |= 0x2_0000;
        }
        let ord = self.mc2_ord(15);
        self.ent[i].f63 = ord;
        self.ent[i].f58 = Self::mc2_wake_stagger(83, ord); // (:34152)
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 0);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// `sub_24100` (:15198) — the engage pose: ONE RNG draw picks
    /// stand sprite 206 (draw ≤ 10) or 1; stop.
    fn m15_engage_pose(&mut self, i: usize) {
        let d = self.mc2_rand(i) % 0x14;
        self.ent[i].f126 = 0;
        self.mc2_set_sprite(i, if d <= 10 { 206 } else { 1 });
    }

    /// `sub_24150` (:15214) — the walk pose: full speed, sprite 0.
    fn m15_walk_pose(&mut self, i: usize) {
        self.ent[i].f126 = self.ent[i].f128;
        self.mc2_set_sprite(i, 0);
    }

    /// `sub_24190` (:15221) — the guard's own wander. Every 8th
    /// phase: die where standing is disallowed (`cap & ~v_20`), else
    /// probe the 4 quadrant headings with RNG-weighted scores
    /// (`(rand % w + 2) * unblocked`, weights {0x1B58, 0x1B58, 0xA,
    /// 0x1B58} — the reverse heading is biased against). Every 16th
    /// phase the move candidate snaps to the tile axis. Packmate
    /// separation writes the COMMITTED heading (roll) directly; the
    /// step happens when roll caught up with yaw or on the 55% roll.
    /// Returns `sub_24190`'s `[ebp-4]` packmate byte (`None` = the
    /// `actionIndex = 124` bail, which never stores it) — see
    /// [`Gen::m27_v34_publish_guard`].
    fn m15_wander(&mut self, i: usize) -> Option<bool> {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        if self.ent[i].f63 % 8 == 0 {
            let (ex, ey) = (self.ent[i].x, self.ent[i].y);
            if self.cap_bit(ex, ey) & !row.v_20 != 0 {
                self.ent[i].tick70 = M15_BASE + 4; // (:15248-56)
                return None;
            }
            const W: [u32; 4] = [0x1B58, 0x1B58, 0x000A, 0x1B58];
            let mut heading = self.ent[i].f30;
            let mut best = 1u16; // v12 init (:15247)
            for w in W {
                let mut pos = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
                Self::polar_step(&mut pos, heading, 0, 256);
                let d = self.mc2_rand(i) % w;
                let score = (d + 2) as u16 * u16::from(!self.mc2_path_blocked(i, pos));
                if score > best {
                    best = score;
                    self.ent[i].f30 = heading;
                }
                heading = (heading + 0x200) & 0x7FF;
            }
        }
        // The move candidate re-seeds from the CURRENT position
        // (:15284): the %16 tile snap keys on the heading quadrant.
        let mut pos = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        if self.ent[i].f63 % 16 == 0 {
            match (self.ent[i].f30.wrapping_sub(256) >> 9) & 3 {
                0 | 2 => pos.1 = (pos.1 >> 8 << 8) + 128,
                _ => pos.0 = (pos.0 >> 8 << 8) + 128,
            }
        }
        // ⭐⭐⭐ AND THAT CANDIDATE **IS** RETAIL'S GLOBAL SCRATCH AXIS
        // (`predictedAxis_EB398ar`, [`Gen::mc2_pred_axis`]) — the
        // re-seed and the tile snap are both writes to the GLOBAL, and
        // they are UNCONDITIONAL: they happen on every wander tick,
        // including the ~45% that never take the step below. So an m15
        // that stands still still leaves its own position in the
        // global for the rest of the tick. The port kept the candidate
        // in a local and published only inside the `if`, so a
        // non-stepping guard published NOTHING. Shipped `NETHERW.EXE`,
        // `sub_24190` = file **0x48990** (VA 0x24190, file = VA +
        // 0x24800); the seed is the first thing past the `%8` probe
        // loop and every path but the `actionIndex = 124` bail reaches
        // it (`jne 0x48a95` at 0x489c6, `jae 0x48a95` at 0x489fc):
        // ```text
        //   48a95: bf 98 b3 01 00  mov  edi,0x1b398    ; &predictedAxis
        //   48a9a: 8d 73 4c        lea  esi,[ebx+0x4c] ; &a1x->position
        //   48a9f: a5              movsd               ; ⭐ x,y
        //   48aa0: 66 a5           movsw               ; ⭐ z
        //   48aa2: 8a 53 3e        mov  dl,[ebx+0x3e]  ; phase — the %16
        //   48aaf: f7 f9           idiv ecx            ;   gate comes AFTER
        //   48ab3: 75 50           jne  0x48b05
        //   48ae5: 66 a3 9a b3 01 00  mov [0x1b39a],ax ; snap y IN THE GLOBAL
        //   48aff: 66 a3 98 b3 01 00  mov [0x1b398],ax ; snap x IN THE GLOBAL
        // ```
        // and the step the port was keying on is guarded, the seed is
        // not (file 0x48bb2-0x48be6):
        // ```text
        //   48bb2: 7f 2c           jg   0x48be0        ; rand%0x14 > 10 ⇒ NO STEP
        //   48bc5: 68 98 b3 01 00  push 0x1b398        ; MoveEntity_57FA0
        //   48bca: e8 d1 3b 03 00  call 0x7c7a0        ;   (VA 0x57FA0)
        //   48bd2: 68 98 b3 01 00  push 0x1b398
        //   48bd8: e8 13 39 03 00  call 0x7c4f0        ; CopyEntityPosition_57CF0
        //   48be1: e8 fa aa ff ff  call 0x436e0        ; sub_1EEE0 — NO publish
        // ```
        // WITNESS: the other half of the mc2l16 t=7844 pair. With the
        // alt-commit over-publish removed, retail's axis at the human's
        // fall puff is slot 300's (7002, 20864, 5248) — a (5,15) whose
        // `rand % 0x14` came up 11+ that tick, so it did not step and
        // the port published nothing at all, leaving the puff on slot
        // 298, two slots earlier.
        // `MGC_NO_MC2_M15_WANDER_PRED_AXIS=1` restores the old gate.
        if !no_mc2_m15_wander_pred_axis() {
            self.mc2_pred_axis = crate::engine::features::Mc2PredAxis(pos);
        }
        // Packmate separation (:15301-11): first same-model neighbor
        // within 256 on both axes — the away bearing lands in ROLL
        // (f34), the wander heading stays in YAW (f30): retail
        // scores/steps yaw_0x1C_28 and writes roll_0x20_32 here
        // (EF:15257-316).
        let (ex, ey, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24)
        };
        let mut packmate = false;
        // ⭐⭐ RETAIL WALKS `bytearray_38403x[a1x->model]`, NOT THE POOL
        // (sub_24190, EF:15301-11): the ONLY body tests are `id !=
        // self` and the two 256 boxes. See [`Gen::mc2_roster`].
        if crate::engine::features::no_mc2_mob_chain_predicate() {
            for c in self.ent.iter().skip(1) {
                if c.class64 == 5
                    && c.model65 == 15
                    && c.id24 != id
                    && c.act_life >= 0
                    && c.flags & 0x400 == 0
                    && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                    && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < 256
                    && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < 256
                {
                    let away = Self::angle_between(c.x, c.y, ex, ey);
                    self.ent[i].f34 = away;
                    packmate = true;
                    break;
                }
            }
        } else {
            let model = self.ent[i].model65;
            for k in 0..self.mc2_roster(model).len() {
                let c = &self.ent[self.mc2_roster(model)[k] as usize];
                if c.id24 != id
                    && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < 256
                    && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < 256
                {
                    let away = Self::angle_between(c.x, c.y, ex, ey);
                    self.ent[i].f34 = away;
                    packmate = true;
                    break;
                }
            }
        }
        if self.ent[i].f30 == self.ent[i].f34 || self.mc2_rand(i) % 0x14 <= 10 {
            let speed = self.ent[i].f126;
            Self::polar_step(&mut pos, self.ent[i].f30, 0, speed);
            self.move_relink(i, pos.0, pos.1, pos.2);
        }
        self.mc2_alt_commit(i);
        Some(packmate)
    }

    /// The brain's acquire scan (:15020-56): nearest CLASS-3 entity
    /// (any model — wizards, castles, balloons; the human counts) in
    /// range + cone off the WANDER heading (yaw, not roll), skipping
    /// invisibles AND same-owner entities — retail gates the walk on
    /// `id_0x1A_26 != own` (:15031), so a CASTLE GUARD never turns
    /// on its own castle/balloons/wizard. A wild archer's id24 is its
    /// own slot, so the gate only drops self-aggro there.
    fn m15_scan(&self, i: usize, ctx: &MobCtx) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let (ex, ey, eyaw, own) = (e.x, e.y, e.f30, e.id24);
        let mut best: Option<(u16, i32)> = None;
        let consider = |tx: u16, ty: u16, slot: u16, best: &mut Option<(u16, i32)>| {
            let d2 = Self::dist2_sq(ex, ey, tx, ty);
            if d2 > range {
                return;
            }
            if Self::angdist(eyaw, Self::angle_between(ex, ey, tx, ty)) >= cone {
                return;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                *best = Some((slot, d2));
            }
        };
        // ⭐ `sub_23C40` walks `dword_38519` (the tick-top class-3
        // roster), testing only `id != own`, the range and
        // `byte[0] & 0x20` — see
        // `features::no_mc2_m15_scan_roster`. The roster's ENTRY test
        // (`life >= 0`) is the only mortality test, and the out-of-pool
        // human must take it here (`ctx.pdead`), as in
        // [`Gen::mc2_class3_scan`].
        let roster = !crate::engine::features::no_mc2_m15_scan_roster();
        if !self.player_invisible && own != PLAYER_TARGET && !(roster && ctx.pdead) {
            consider(ctx.px, ctx.py, PLAYER_TARGET, &mut best);
        }
        if roster {
            for c in 0..self.wiz_chain.visible_len() {
                let j = self.wiz_chain.list[c] as usize;
                let w = &self.ent[j];
                if j != i && w.id24 != own && w.flags & 0x20 == 0 {
                    consider(w.x, w.y, j as u16, &mut best);
                }
            }
            return best.map(|(s, _)| s);
        }
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if j != i
                && c.class64 == 3
                && c.id24 != own
                && c.act_life >= 0
                && c.flags & (0x400 | 0x20) == 0
            {
                consider(c.x, c.y, j as u16, &mut best);
            }
        }
        best.map(|(s, _)| s)
    }

    /// `sub_23C40` (:14958) — the idle/scan brain. Clean tick:
    /// wander, then on the row cadence (while the ctor stagger
    /// lives) the class-3 acquire scan. Non-lethal hit: chase a
    /// class-3 source. The engage pose fires whenever the tick ends
    /// in the chase state.
    fn m15_brain(&mut self, i: usize, ctx: &MobCtx) {
        let head = self.mc2_state_head(i);
        // What the tick leaves on the hydra's `v34` dwords
        // ([`Gen::m27_v34_publish_guard`]).
        let mut guard = GuardV34::Hit;
        match head {
            1 => {
                // (:15060-70) — retarget ONLY on a class-3 source.
                let src = self.ent[i].f40;
                let is_c3 = src == PLAYER_TARGET
                    || ((src as usize) < self.ent.len()
                        && src != 0
                        && self.ent[src as usize].class64 == 3);
                // ⭐ THE SECOND TEST IS AN **OWNER** TEST, NOT AN
                // INDEX TEST: retail reads
                // `Entities_EA3E4[word_0x26_38]->id_0x1A_26` and
                // compares it to the guard's own `id_0x1A_26`
                // (NETHERW.EXE 0x48501 `mov 0x1a(%eax),%ax` /
                // 0x48505 `cmp 0x1a(%ebx),%ax`), so a guard hit by
                // ANY class-3 record of its OWN wizard — its castle,
                // its balloons, the wizard himself — stays in 121.
                // The port compared the SOURCE SLOT against the owner
                // tag, which is only ever equal for a wizard whose
                // id24 is its own slot. Kill switch
                // `MGC_NO_MC2_M15_HIT_OWNER_GATE`.
                let own = self.ent[i].id24;
                let foreign = if crate::mc2::mobs::no_mc2_m15_hit_owner_gate() {
                    src != own
                } else if src == PLAYER_TARGET {
                    PLAYER_TARGET != own
                } else if (src as usize) < self.ent.len() {
                    self.ent[src as usize].id24 != own
                } else {
                    src != own
                };
                if is_c3 && foreign {
                    self.ent[i].tick70 = M15_BASE + 2;
                    self.ent[i].f146 = src;
                }
                self.mc2_alt_commit(i);
            }
            2 => {
                self.ent[i].tick70 = M15_BASE + 4;
                guard = GuardV34::Silent;
            }
            _ => {
                guard = GuardV34::Wander(self.m15_wander(i));
                let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                if self.ent[i].f63 as i16 % period == 0 && self.ent[i].f58 != 0 {
                    if self.m15_scan_tests_any(i, ctx) {
                        guard = GuardV34::Scanned;
                    }
                    if let Some(t) = self.m15_scan(i, ctx) {
                        self.ent[i].tick70 = M15_BASE + 2;
                        self.ent[i].f146 = t;
                    }
                }
            }
        }
        let engaged = self.ent[i].tick70 == M15_BASE + 2;
        if engaged {
            self.m15_engage_pose(i);
        }
        self.m27_v34_publish_guard(i, guard, engaged);
    }

    /// Does `sub_23C40`'s acquire walk reach its `sub_581E0` bearing
    /// call for ANY candidate (`id != own`, squared range `ja`,
    /// `byte[0] & 0x20`, NETHERW.EXE 0x48581-0x485B6)? The same
    /// candidate set as [`Gen::m15_scan`], before the cone test.
    fn m15_scan_tests_any(&self, i: usize, ctx: &MobCtx) -> bool {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let (ex, ey, own) = (e.x, e.y, e.id24);
        // Same membership as `m15_scan` (`no_mc2_m15_scan_roster`).
        let roster = !crate::engine::features::no_mc2_m15_scan_roster();
        if !self.player_invisible
            && own != PLAYER_TARGET
            && !(roster && ctx.pdead)
            && Self::dist2_sq(ex, ey, ctx.px, ctx.py) <= range
        {
            return true;
        }
        if roster {
            return (0..self.wiz_chain.visible_len()).any(|c| {
                let j = self.wiz_chain.list[c] as usize;
                let w = &self.ent[j];
                j != i
                    && w.id24 != own
                    && w.flags & 0x20 == 0
                    && Self::dist2_sq(ex, ey, w.x, w.y) <= range
            });
        }
        self.ent.iter().enumerate().skip(1).any(|(j, c)| {
            j != i
                && c.class64 == 3
                && c.id24 != own
                && c.act_life >= 0
                && c.flags & (0x400 | 0x20) == 0
                && Self::dist2_sq(ex, ey, c.x, c.y) <= range
        })
    }

    /// The (9,13) volley (:15154-66) — the archer's launch minus the
    /// `sub_200F0` overrides: the projectile keeps its template
    /// subSpell (NO f44 write) and inherits xtype/xsubtype from the
    /// GUARD, not the target.
    fn m15_fire(&mut self, i: usize, target: u16, tpos: (u16, u16, i16)) {
        let (x, y, z, own, fov) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f84)
        };
        let Some(a) = self.mc2_spawn_arrow(x, y, z) else {
            return;
        };
        self.ent[a].id24 = own;
        self.ent[a].f30 = Self::angle_between(x, y, tpos.0, tpos.1);
        let dh = Self::isqrt(Self::dist2_sq(x, y, tpos.0, tpos.1) as u32) as i32;
        self.ent[a].f32 = Self::pitch_toward(z, tpos.2, dh);
        let (ax, ay) = (self.ent[a].x, self.ent[a].y);
        let az = self.ent[a].z.wrapping_add((fov / 2) as i16);
        self.move_relink(a, ax, ay, az);
        self.ent[a].f146 = self.ent[i].f146;
        self.ent[a].f66 = self.ent[i].f66;
        self.ent[a].f67 = self.ent[i].f67;
        if target == PLAYER_TARGET {
            self.player_danger = 100; // sub_5EF70
        }
        // No shots++: retail's volley (EF:15154-66) never touches
        // the player stat — the counter is the PLAYER's own.
    }

    /// `sub_23E60` (:15083) — chase/volley. STATIONARY: no move
    /// core; faces the target every 4th tick (the committed heading
    /// directly); NO retarget on a non-lethal hit (unlike the
    /// generic chase core). Out-of-range / dead target → back to the
    /// brain; the walk pose restores on any exit from the state.
    fn m15_chase(&mut self, i: usize, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            2 => self.ent[i].tick70 = M15_BASE + 4,
            _ => {
                let slot = self.ent[i].f146;
                match self.mc2_target(slot, ctx) {
                    None => self.ent[i].tick70 = M15_BASE + 1, // dead/draw-off (:15138-42)
                    Some((tx, ty, tz)) => {
                        if self.ent[i].f63 & 3 == 0 {
                            let e = &self.ent[i];
                            // ⚠ EF:15135-36 writes `roll_0x20_32` —
                            // the COMMITTED heading (`f34`), NOT yaw.
                            // Aiming `f30` dragged the GRADED `heading`
                            // obs lane with it: 477 unexplained (5,15)
                            // heading rows (mc2l4 466, galore 8,
                            // mc2l30 3), none carried by any ledger
                            // rule. mc2l4 slot 370 is the proof from
                            // the CAPTURE, not the decompile — a
                            // stationary guard in state 122 whose
                            // retail yaw is PINNED at 512 while its
                            // retail roll steps 512 (t=2380) -> 668
                            // (t=2381), against the grader's
                            // `heading: retail 512 port 668`. Same
                            // idiom as `mc2_aim_avoid` (roster.rs:88),
                            // and the docstring above this fn already
                            // says "the committed heading directly".
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                        }
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1) as u8;
                        let mut left = false;
                        // ARROW-VOLLEY PROBE (`MGC_ARROW_VOLLEY_TRACE=1`): the m15 volley gate.
                        if crate::mc2::mobs::arrow_volley_trace_on() {
                            let e = &self.ent[i];
                            let d3 = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                            eprintln!(
                                "DIG4 m15 slot={i} ph={} per={period} mod={} tgt={slot} d3={d3} v28={} pos=({},{},{}) tp=({tx},{ty},{tz})",
                                e.f63,
                                e.f63 % period,
                                BEHAVIOR[e.row156 as usize].v_28,
                                e.x,
                                e.y,
                                e.z
                            );
                        }
                        if self.ent[i].f63 % period == 0 {
                            let e = &self.ent[i];
                            let d3 = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                            if d3 >= BEHAVIOR[self.ent[i].row156 as usize].v_28 as u32 {
                                self.ent[i].tick70 = M15_BASE + 1; // (:15149-53)
                                left = true;
                            } else {
                                self.m15_fire(i, slot, (tx, ty, tz));
                            }
                        }
                        if !left {
                            self.mc2_alt_commit(i); // sub_1EEE0 (:15168)
                        }
                    }
                }
            }
        }
        if self.ent[i].tick70 != M15_BASE + 2 {
            self.m15_walk_pose(i); // LABEL_26 (:15175-76)
        }
    }

    pub(crate) fn m15_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M15_BASE {
            0 => self.mc2_patrol(i, M15_BASE),
            1 => self.m15_brain(i, ctx),
            2 => self.m15_chase(i, ctx),
            3 => self.mc2_pack(i, M15_BASE),
            4 => self.mc2_prekill(i, M15_BASE),
            5 => self.mc2_kill(i),
            // +6 (0x243F0): MISSING from the decompile; unreachable
            // (the row's flee bit is clear). +7 spawn hook sub_1D5D0:
            // a no-op for StageVar2 == 0.
            _ => {}
        }
    }

    // =========================================================================
    // MODEL 16 — the boss (ctor sub_4C310 EF:34163, states 0x80-87;
    // 60000 life, 15-bolt homing bursts, trace mc2-class5-m16-20.md)
    // =========================================================================

    /// `sub_67800`/`sub_51800`→`sub_3A5B0` (EF:59138) — the Summon Army
    /// creature ring. The army SIZE keys off the model: firefly/bee
    /// (19/2) → 8, Cymmerian (25) → 4, wyvern (16) → 2 (weak swarm vs
    /// strong pack). Each node spawns a class-5 creature marked as the
    /// allied controlled-creature (StageVar2/site_z = 13, action `8*M+7`,
    /// owner = caster, 250-tick `f26` life). Radius 512, angle
    /// `k·2048/N` (docs/spell-audit/summon-creatures.md Part B).
    /// Returns the ring HEAD's slot — the record `sub_51800` itself
    /// returns, and therefore the only one `sub_65820`'s impact tail
    /// stamps (see [`World::mc2_summon_head_bearing`]).
    pub(crate) fn mc2_spawn_summon_ring(
        &mut self,
        x: u16,
        y: u16,
        model: u8,
        own: u16,
    ) -> Option<usize> {
        let n: u32 = match model {
            25 => 4,
            16 => 2,
            _ => 8, // firefly (19) / bee (2)
        };
        if !no_summon_nodes() {
            return self.mc2_spawn_summon_nodes(x, y, model, own, n);
        }
        for k in 0..n {
            let ang = ((k * 2048 / n) as u16) & 0x7FF;
            let mut p = (x, y, self.ground_z(x, y) as i16);
            Gen::polar_step(&mut p, ang, 0, 512);
            let gz = self.ground_z(p.0, p.1) as i16;
            let Some(s) = self.mc2_spawn_creature_model(model, p.0, p.1, gz) else {
                continue;
            };
            let e = &mut self.ent[s];
            e.site_z = 13; // StageVar2 = 13 (summon-army allied AI)
            e.tick70 = model.wrapping_mul(8).wrapping_add(7); // action 8*M+7
            e.id24 = own; // caster's team → allied
            e.f26 = 250; // 250-tick lifespan
            e.f146 = 0; // no target yet
        }
        None
    }

    /// ⭐⭐⭐ **THE IMPACT TAIL STAMPS THE RECORD `_4A190` RETURNED —
    /// AND FOR THE SUMMON RING THAT IS THE HEAD ALONE.** The generic
    /// class-9 flight worker `sub_65820` closes every detonation with
    /// an enumerated copy block off the PROJECTILE (EF:62988-94):
    /// ```text
    ///   v11x->id_0x1A_26        = a1x->id_0x1A_26;
    ///   v11x->yaw_0x1C_28       = a1x->yaw_0x1C_28;
    ///   v11x->pitch_0x1E_30     = a1x->pitch_0x1E_30;
    ///   if (v5x) v11x->word_0x96_150 = v5x - D41A0_0.struct_0x6E8E;
    ///   v11x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42;
    ///   v11x->byte_0x46_70      = a1x->byte_0x46_70;
    /// ```
    /// `v11x` is `sub_51800`'s return value — the ring HEAD. The
    /// tail nodes are `qmemcpy` copies taken BEFORE the stamp, so
    /// they keep `NewEvent_4A050`'s zeros, and `sub_67800`'s chain
    /// walk afterwards re-stamps only `byte_0x46_70` / `parentId` /
    /// `id` down the `word_0x34_52` list (EF:59166-72) — never the
    /// bearing pair. Retail records exactly that split: mc2l6-rsg
    /// t=20564 head slot 81 `yaw` 502 / `pitch` 119 (its caster 343's
    /// own bearing one tick earlier: 495 / 120) while its ring-mate
    /// slot 13 reads 0 / 0; mc2l22 t=53599 head slot 421 reads
    /// 676 / 1993 — the caster 424's bearing TO THE UNIT — and its
    /// tail slot 978 reads 0 / 0.
    ///
    /// The port's `mc2_proj_impact` `(10,72)` arm returns `None`
    /// (it mints its own children), which skips the whole generic
    /// tail, so every ring head carried 0 / 0 for its entire 16-tick
    /// life. Free-run witness (`dump-state --port --start 20563`,
    /// mc2l6-rsg t=20564 slot 81): the ONLY two `!=` lanes in the
    /// whole record are `yaw` and `pitch`.
    pub(crate) fn mc2_summon_head_bearing(&mut self, head: Option<usize>, yaw: u16, pitch: u16) {
        if no_summon_head_bearing() {
            return;
        }
        if let Some(h) = head {
            let e = &mut self.ent[h];
            e.f30 = yaw; // yaw_0x1C_28   (EF:62989)
            e.f32 = pitch; // pitch_0x1E_30 (EF:62990)
        }
    }

    /// ⭐⭐⭐ **THE SUMMON RING IS A RING OF NODES, NOT OF CREATURES.**
    /// `sub_51800` (EF:37459; EXE 0x7603e-0x7605d writes `[eax+0x45] =
    /// 0x4f`, `[eax+0x3f] = 0xa`, `[eax+0x40] = 0x48`, `[eax+0x4] =
    /// 0x10`, `and dl,0xf7` on `[eax+0xc]`) mints ONE `(10,72)` record
    /// per ring member — action `0x4F`, maxLife/life **16**, the
    /// collide bit CLEARED — and only when that node's own life
    /// counts down to zero does `sub_3A5B0` (EF:29590) hatch the
    /// class-5 creature. The port hatched the creatures immediately,
    /// so every summon was 16 ticks early, minted HALF the records
    /// retail mints (N nodes + N creatures), and shifted every later
    /// free-stack pop.
    ///
    /// Three exact details the corpus grades:
    /// - the ring COUNT is `x_D41A0_BYTEARRAY_4_struct.byteindex_224`,
    ///   set by `sub_67800` from the creature model (EF:59147-62), and
    ///   the whole ring is ALL-OR-NOTHING behind
    ///   `sub_4A810_get_0x35plus() >= count` (EF:37462);
    /// - nodes 1..N-1 are a **`qmemcpy` of the head record** (EF:37478)
    ///   — so every node carries the HEAD's entity RNG seed, which is
    ///   exactly what mc2l6-rsg records (t=15641 slots 3/4/32 all
    ///   `rand = 36786`; t=20563 slots 13/81 both 42525);
    /// - the head is placed and drawn LAST, after the whole loop
    ///   (EF:37494-99) — its `MoveEntity_57FA0` step is angle 0, and
    ///   node k's is `k * (2048 / count)`, an INTEGER division done
    ///   before the multiply.
    ///
    /// `sub_67800`'s chain walk then stamps `byte_0x46_70` (the
    /// creature model), `parentId` and `id` down the `word_0x34_52`
    /// list (EF:59166-72) — the port carries the caster in `id24` and
    /// the model in `f71`.
    fn mc2_spawn_summon_nodes(
        &mut self,
        x: u16,
        y: u16,
        model: u8,
        own: u16,
        n: u32,
    ) -> Option<usize> {
        // EF:37462 — the ring is all-or-nothing on the free-slot count.
        if self.free.len() < n as usize {
            return None;
        }
        let head = self.new_event()?;
        {
            let e = &mut self.ent[head];
            e.tick70 = 0x4F; // actionIndex → sub_3A5B0
            e.class64 = 10;
            e.model65 = 72;
            e.max_life = 16;
            e.flags &= !8; // struct_byte_0xc.byte[0] &= 0xF7
            e.f63 = 0; // byte_0x3E_62 = 0 (the ring index)
            e.f71 = model; // byte_0x46_70 — sub_67800's chain stamp
            e.id24 = own; // id_0x1A_26 — ditto
        }
        self.refill_life(head); // life_0x8 = maxLife_0x4
        let step = 2048 / n; // INTEGER division, then the multiply
        let mut prev = head;
        for k in 1..n {
            let Some(t) = self.new_event() else {
                break;
            };
            self.ent[t] = self.ent[head]; // qmemcpy (EF:37478)
            self.ent[t].f52 = prev as u16; // word_0x32_50 = prev
            self.ent[prev].f54 = t as u16; // prev->word_0x34_52 = new
            self.ent[t].f54 = 0;
            self.ent[t].f63 = k as u8; // byte_0x3E_62 = index
            let mut p = (x, y, 0i16);
            Gen::polar_step(&mut p, ((k * step) as u16) & 0x7FF, 0, 512);
            let gz = self.ground_z(p.0, p.1) as i16;
            self.link(t, p.0, p.1, gz);
            self.mc2_shift_rot(t, 128, 256);
            self.mc2_set_sprite(t, 220);
            prev = t;
        }
        let mut p = (x, y, 0i16);
        Gen::polar_step(&mut p, 0, 0, 512);
        let gz = self.ground_z(p.0, p.1) as i16;
        self.link(head, p.0, p.1, gz);
        self.mc2_shift_rot(head, 128, 256);
        self.mc2_set_sprite(head, 220);
        Some(head)
    }

    /// `sub_3A5B0` (EF:29590) — the `(10,72)` ring node's own tick
    /// (action `0x4F`, `x_DWORD_D4C52ar_strA0[0x4F] = 0x0021B5B0`):
    /// `life -= 1`; below zero it despawns; **at exactly zero** it
    /// calls `_4A190(&pos, 5, byte_0x46_70)` and hatches the summon
    /// army creature — StageVar2 13, action `8*model + 7`, the
    /// caster's `parentId`/`id`, mana and target cleared and the
    /// 250-tick lease in `word_0x2E_46`.
    pub(crate) fn mc2_summon_node_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].act_life = life;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        if life != 0 {
            return;
        }
        // `&a1x->position_0x4C_76` — the node's own seat, which
        // `sub_51800` already put at `getTerrainAlt_10C40`.
        let (x, y, model, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.f71, e.id24)
        };
        let gz = self.ground_z(x, y) as i16;
        let Some(s) = self.mc2_spawn_creature_model(model, x, y, gz) else {
            return;
        };
        let e = &mut self.ent[s];
        e.site_z = 13; // StageVar2_0x49_73 = 13 (EF:29609)
        e.tick70 = e.model65.wrapping_mul(8).wrapping_add(7); // 8*model + 7
        e.id24 = own; // id_0x1A_26 (EF:29616)
        e.f146 = 0; // word_0x96_150 = 0
        e.f140 = 0; // mana_0x90_144 = 0
        e.f136 = 0; // maxMana_0x8C_140 = 0
        // ⭐ DIG 98-Q20 — the lease has its OWN home now, so the
        // hatched creature keeps `mc2_spawn_creature_model`'s ctor
        // seed (`dword_0x10_16 = slot % 100`, EF:34291 and siblings)
        // in `f26`, exactly as `sub_3A5B0` (EF:29590-616) leaves it:
        // that block writes StageVar2/action/parentId/id/target/mana/
        // maxMana and `word_0x2E_46` — and NOTHING at @0x10.
        e.set_lease(250); // word_0x2E_46 = 250
    }

    /// `sub_3A630` (EF:29617) — the `(10,73)` summon-army DESPAWN PUFF
    /// (action `0x50`, `strA0[0x50] = 0x0021B630`): a bare
    /// `life -= 1; if (life <= 0) DisableEntityDrawing04_57F10`. Its
    /// ctor is `sub_51A00` (EF:37506): action `0x50`, class 10, model
    /// `0x49`, life = maxLife = **16**, collide bit cleared, snapped
    /// to the terrain altitude, sprite 220.
    pub(crate) fn mc2_summon_puff_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life - 1;
        self.ent[i].act_life = life;
        if life <= 0 {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_51A00` (EF:37506) — mint the `(10,73)` despawn puff.
    pub(crate) fn mc2_spawn_summon_puff(&mut self, x: u16, y: u16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.tick70 = 0x50;
            e.class64 = 10;
            e.model65 = 73;
            e.act_life = 16;
            e.flags &= !8;
            e.max_life = 16;
        }
        let gz = self.ground_z(x, y) as i16;
        self.link(i, x, y, gz);
        self.mc2_shift_rot(i, 128, 256);
        self.mc2_set_sprite(i, 220);
        Some(i)
    }

    /// Spawn a controlled-creature roster model (the Metamorph / Summon
    /// Army `{2,16,19,25}` ladder) through its normal class-5 ctor. The
    /// caller then overrides the action to `8*M+7` and the StageVar2
    /// marker (site_z) — docs/spell-audit/summon-creatures.md.
    pub(crate) fn mc2_spawn_creature_model(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        match model {
            2 => self.mc2_spawn_m2(x, y, z),
            16 => self.mc2_spawn_m16(x, y, z),
            19 => self.mc2_spawn_m19(x, y, z),
            25 => self.mc2_spawn_m25(x, y, z),
            _ => None,
        }
    }

    pub(crate) fn mc2_spawn_m16(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 16;
            e.tick70 = M16_BASE + 1; // 129
            e.f28 = 1;
            e.f128 = 60;
            e.f130 = 20;
            e.max_life = 60000;
            e.f126 = 60;
        }
        self.mc2_set_mana_half(i);
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 500;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.f26 = 0; // :34187 re-zero
            e.row156 = 84;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(16);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 207);
        // :34192-94: array.yaw = 5·word(D9F50+294)/8. D9F50 row 21's
        // word_0 = 0x5DC (1500), and offset 294 is never written at
        // runtime (the table's only writers hit 0x87A/0x5B6/0x126) —
        // so the wyvern's z-box center is the CONSTANT 937.
        self.ent[i].f78 = 937;
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// m16's homing bolt (:15474): a (9,0) with row 61, damage 1600,
    /// mana 50000, z-lift 6·fov.
    fn m16_bolt(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, (6 * e.f84) as i16)
        };
        let Some(p) = self.mc2_spawn_bolt(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 0;
        self.ent[p].row156 = 61;
        self.ent[p].f44 = 1600;
        self.ent[p].f140 = 50000;
        self.mc2_arm_proj(p, i, target, tpos);
        true
    }

    /// A/B toggle for the m16 AIM-BEFORE-DEAD law (dig 98-Q22).
    /// `MGC_NO_M16_AIM_BEFORE_DEAD=1` restores the pre-2026-09-04
    /// order, where `sub_24510`'s pointer guard and its life guard
    /// were fused into one `mc2_target` call and the aim was skipped
    /// on a corpse.
    fn m16_aim_before_dead_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M16_AIM_BEFORE_DEAD").is_none())
    }

    pub(crate) fn m16_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M16_BASE {
            0 => self.mc2_patrol(i, M16_BASE),
            1 => {
                // sub_24440 (:15339): the shared idle PLUS the wide
                // building sweep on the cadence.
                self.mc2_idle(i, M16_BASE, ctx);
                let row = &BEHAVIOR[self.ent[i].row156 as usize];
                let period = (row.v_26 + 1).max(1);
                if self.ent[i].tick70 == M16_BASE + 1 && self.ent[i].f63 as i16 % period == 0 {
                    let range = (row.v_28 as i32) * (row.v_28 as i32);
                    let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                    let mut best: Option<(usize, i32)> = None;
                    if crate::engine::features::no_mc2_m16_sweep_roster() {
                        for (j, c) in self.ent.iter().enumerate().skip(1) {
                            if c.class64 == 10 && c.model65 == 45 && c.flags & 0x400 == 0 {
                                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                if d2 <= range && best_d2(&best, d2) {
                                    best = Some((j, d2));
                                }
                            }
                        }
                    } else {
                        // ⭐ `dword_38527`, the TICK-TOP building roster:
                        // the walk tests range and nearest only — a
                        // building reap-flagged earlier this tick is
                        // still a candidate. See
                        // `features::no_mc2_m16_sweep_roster`.
                        for k in 0..self.bldg_chain.visible_len() {
                            let j = self.bldg_chain.list[k] as usize;
                            let c = &self.ent[j];
                            let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                            if d2 <= range && best_d2(&best, d2) {
                                best = Some((j, d2));
                            }
                        }
                    }
                    if let Some((j, _)) = best {
                        self.ent[i].tick70 = M16_BASE + 2;
                        self.ent[i].f146 = j as u16;
                    }
                }
            }
            2 => {
                // sub_24510 (:15389) — the burst-attack brain.
                match self.mc2_state_head(i) {
                    1 => {
                        self.ent[i].f146 = self.ent[i].f40;
                        self.m27_v34_publish_wyvern(i, None, true);
                    }
                    2 => {
                        self.ent[i].tick70 = M16_BASE + 4;
                        self.m27_v34_publish_wyvern(i, None, true);
                    }
                    _ => {
                        self.mc2_move_core(i);
                        // EF:15451 — `sub_1ED30(a1x, Entities[word_0x96_150])`:
                        // the charmed branch counts the charm clock and
                        // can answer `Entities[0]` ([`Gen::mc2_ally_resolve`]).
                        let raw = self.ent[i].f146;
                        let slot = self.mc2_ally_resolve(i, raw).unwrap_or(0);
                        // `sub_1ED30`'s saved ESI — the brain's arm
                        // selector, 0 here — on W-52.
                        self.m27_v34_publish_wyvern(i, Some(0), false);
                        // ⭐⭐⭐ RETAIL'S POINTER TEST AND ITS LIFE TEST
                        // ARE TWO DIFFERENT TESTS, AND THE AIM SITS
                        // BETWEEN THEM. `sub_24510` (EF:15450-66)
                        // resolves the lock through `sub_1ED30` —
                        // which for StageVar2 != 14 hands back
                        // `Entities[word_0x96_150]` verbatim
                        // (EF:11167) — guards it with a bare POINTER
                        // compare, steers `roll_0x20_32` onto it, and
                        // only THEN asks whether it is alive. Shipped
                        // `NETHERW.EXE`, file 0x48DFD-0x48E53:
                        // ```text
                        //   48dfd  mov  0x1a3e4,%ecx      ; Entities[0]
                        //   48e08  cmp  %ecx,%eax
                        //   48e0a  jbe  0x48fc3           ; POINTER ONLY
                        //   48e10  testb $0x7,0x3e(%ebx)  ; 8-tick throttle
                        //   48e16  cmpb $0x3,0x3f(%eax)   ; TARGET's class
                        //   48e1c  call 0x7cbf0           ; dist3d >= 0x200
                        //   48e43  mov  %ax,0x20(%ebx)    ; *** roll = aim ***
                        //   48e47  cmpl $0x0,0x8(%edi)    ; life < 0   <-- AFTER
                        //   48e4d  testb $0x4,0xd(%edi)   ; reap bit
                        //   48e53  movb $0x81,0x45(%ebx)  ; action = 129
                        // ```
                        // ⭐ The `cmpb $0x3,0x3f(%eax)` reads the
                        // TARGET's class, not the attacker's — the
                        // decompile's `a1x->class_0x3F_63 == 3` is a
                        // hand-conversion error, and the port already
                        // had the EXE's reading.
                        // The port fused both tests into
                        // `mc2_target`, so a wyvern whose victim died
                        // this frame skipped the aim entirely and its
                        // heading servo chased a stale `roll` for the
                        // rest of the take. Kill switch
                        // `MGC_NO_M16_AIM_BEFORE_DEAD`.
                        let aim_law = Self::m16_aim_before_dead_law();
                        let resolved = if aim_law {
                            self.mc2_summon_lock_pos(slot, ctx)
                        } else {
                            self.mc2_target(slot, ctx)
                        };
                        let Some((tx, ty, tz)) = resolved else {
                            // ⭐⭐⭐ THE WYVERN'S ATTACK STATE HAS NO NULL
                            // ARM. `sub_24510` is the ONLY one of
                            // `sub_1ED30`'s ten callers that does
                            // nothing when the resolver answers
                            // `Entities[0]`: `if (iz > Entities[0])
                            // {…}` and the function ends —
                            // `NETHERW.EXE` 0x48e0a `jbe 0x48fc3`, the
                            // epilogue. So a charmed wyvern whose charm
                            // clock lapses in action 130, or that was
                            // charmed mid-attack (`sub_3A650` zeroes
                            // its lock and leaves it in 130, EXE
                            // 0x5ef8f), flies its last heading for the
                            // rest of its life: the state-7 expiry
                            // never runs, the tick-top snap skips
                            // phase 2, and damage only rewrites the
                            // lock. WITNESS `recordings/mc2l17.mgcr`:
                            // seven wyverns (slots 1/2/3/5/7 from
                            // t=26939, 8 from t=11964, 11 from
                            // t=33560), every one in 130 with lock 0
                            // and the clock running negative (slot 8
                            // to −11,806) until the player killed it.
                            // PATCH OPTION `mc2_wyvern_alliance_brain`
                            // (docs/DEVIATIONS.md): the missing arm,
                            // the same `actionIndex = 8m+1` the other
                            // nine callers take. The legacy `!aim_law`
                            // arm folds the life test into
                            // `mc2_target`, so its `None` keeps the
                            // old exit.
                            let stay = aim_law
                                && !(ctx.patches.mc2_wyvern_alliance_brain && !ctx.strict);
                            if !stay {
                                self.ent[i].tick70 = M16_BASE + 1;
                            }
                            return;
                        };
                        if self.ent[i].f63 & 7 == 0 {
                            let e = &self.ent[i];
                            let far = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz)) >= 0x200;
                            let is_c3 =
                                slot == PLAYER_TARGET || self.ent[slot as usize].class64 == 3;
                            if is_c3 || far {
                                let e = &self.ent[i];
                                self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                            }
                        }
                        // 0x48E47: the life/reap test, in retail's own
                        // place — below the aim, above the burst.
                        if aim_law && self.mc2_target(slot, ctx).is_none() {
                            self.ent[i].tick70 = M16_BASE + 1;
                            return;
                        }
                        if self.ent[i].f26 > 0 {
                            self.ent[i].f26 -= 1;
                            self.m16_bolt(i, slot, ctx);
                            // The spawn's frame, then `sub_581E0` /
                            // `sub_58210` with the new record in ESI.
                            self.m27_v34_publish_wyvern(i, None, false);
                        }
                        let row = &BEHAVIOR[self.ent[i].row156 as usize];
                        let period = row.v_26.max(1);
                        if self.ent[i].f63 as i16 % period == 0 {
                            let e = &self.ent[i];
                            let d2 = Self::dist2_sq(e.x, e.y, tx, ty);
                            let range = (row.v_28 as i32) * (row.v_28 as i32);
                            if d2 < range {
                                // 0x48F00 `movzbl 0x3e(%ebx),%esi`, then
                                // `sub_581E0` (0x48F90) saves it on W-52.
                                let phase = self.ent[i].f63 as u8 as u32;
                                self.m27_v34_publish_wyvern(i, Some(phase), false);
                                if self.ent[i].f63 as i16 % (2 * period) == 0 {
                                    self.snd(39, i);
                                }
                                let e = &self.ent[i];
                                let aim = Self::angle_between(e.x, e.y, tx, ty);
                                if Self::angdist(e.f30, aim) < 0xE3 {
                                    self.ent[i].f26 = 15; // arm the burst
                                    self.mc2_danger_poke(slot);
                                }
                            } else {
                                self.ent[i].tick70 = M16_BASE + 1;
                            }
                        }
                    }
                }
            }
            3 => self.mc2_pack(i, M16_BASE),
            4 => self.mc2_prekill(i, M16_BASE),
            5 => self.mc2_kill(i),
            6 => {} // no handler in the dispatch — unreachable (row 84 flee clear)
            _ => {}
        }
    }

    // =========================================================================
    // MODEL 17 — the dive-bomber (ctor sub_4C460 EF:34201, states
    // 0x88-8F; long-range (9,20) lobs, then a 3x-speed dive on row 87)
    // =========================================================================

    pub(crate) fn mc2_spawn_m17(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 17;
            e.tick70 = M17_BASE + 1; // 137
            e.f28 = 1;
            e.f128 = 68;
            e.f130 = 20;
            e.max_life = 10000;
            e.f126 = 68;
        }
        self.mc2_set_mana_half(i);
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f26 = 0; // :34218/:34226 — set %100 then re-zeroed
            e.f44 = 350;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 85;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(17);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 285);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// The wizard-target validation deviation shared by m17's
    /// wrapper states (:15560+): a non-wizard acquire is dropped.
    fn m17_validate(&mut self, i: usize) {
        if self.ent[i].tick70 == M17_BASE + 2 {
            let t = self.ent[i].f146;
            if !self.mc2_is_wizard(t) {
                self.ent[i].f146 = 0;
            }
            self.ent[i].f71 = 0;
        }
    }

    /// The dive z-curve (:15726-44, VERBATIM): 5 rising ticks
    /// (+192,+96,+48,+24,+12) then a sharp fall (−24,−48,−96,−192,
    /// held at −192).
    fn m17_dive_step(n: i16) -> i16 {
        if n <= 4 {
            192 >> n
        } else {
            let s = (4 - (n - 4)).max(0);
            (-(192 >> s)).max(-192)
        }
    }

    pub(crate) fn m17_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M17_BASE {
            0 => {
                self.mc2_patrol(i, M17_BASE);
                self.m17_validate(i);
            }
            1 => {
                self.mc2_idle(i, M17_BASE, ctx);
                self.m17_validate(i);
            }
            2 => {
                // sub_24930 (:15596) — the dive machine.
                self.snd(58, i); // idle-loop, every tick in state
                match self.mc2_state_head(i) {
                    1 => self.ent[i].f146 = self.ent[i].f40,
                    2 => {
                        self.ent[i].tick70 = M17_BASE + 4;
                        return;
                    }
                    _ => {}
                }
                let v13 = self.mc2_move_core(i);
                let slot = self.ent[i].f146;
                let Some((tx, ty, tz)) = self.mc2_target(slot, ctx) else {
                    self.ent[i].row156 = 85;
                    self.ent[i].f146 = 0;
                    self.ent[i].tick70 = M17_BASE + 1;
                    self.ent[i].f126 = self.ent[i].f128;
                    return;
                };
                if self.ent[i].f63 & 3 == 0 && matches!(self.ent[i].f71, 0 | 4) {
                    self.mc2_aim_avoid(i, tx, ty);
                }
                let row_period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                match self.ent[i].f71 {
                    0 => {
                        if self.ent[i].f63 as i16 % row_period == 0 {
                            let e = &self.ent[i];
                            let d = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                            if d >= BEHAVIOR[e.row156 as usize].v_28 as u32 {
                                self.ent[i].tick70 = M17_BASE + 1;
                            } else if d >= 0x700 {
                                self.mc2_atk_lob20(i, slot, ctx);
                            } else {
                                self.ent[i].f71 = 1;
                            }
                        }
                    }
                    1 => {
                        let e = &self.ent[i];
                        let aim = Self::angle_between(e.x, e.y, tx, ty);
                        self.ent[i].f34 = aim;
                        self.ent[i].f30 = aim;
                        self.ent[i].f126 = 3 * self.ent[i].f128; // 204
                        self.ent[i].row156 = 87; // the dive row
                        self.ent[i].f26 = 0;
                        self.ent[i].f71 = 2;
                    }
                    2 | 3 => {
                        if v13 != 3 {
                            self.ent[i].f30 = self.ent[i].f34;
                        }
                        let n = self.ent[i].f26;
                        self.ent[i].f26 = n + 1;
                        let v14 = Self::m17_dive_step(n);
                        self.ent[i].f126 = (self.ent[i].f126 - 8).max(self.ent[i].f130);
                        let (x, y, z) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z)
                        };
                        let nz = z.wrapping_add(v14);
                        if nz <= self.ground_z(x, y) as i16 {
                            self.ent[i].f71 = 4;
                            self.ent[i].f26 = 18;
                        } else {
                            self.ent[i].z = nz;
                            if self.ent[i].f71 == 2 && self.mc2_atk_melee_768(i, slot, ctx) {
                                self.ent[i].f71 = 3;
                            }
                        }
                        let _ = tz;
                    }
                    // Leap recovery (EF:15771-88): retail reads the
                    // OLD counter, decrements, then compares the OLD
                    // value — the ground row 85 (v_14=-128) restores
                    // on the FIRST recover tick. Decrement AFTER the
                    // `== 18` compare, or it never fires and the leaper
                    // stays on dive row 87 (v_14=0) running on air.
                    4 => {
                        let v = self.ent[i].f26;
                        self.ent[i].f26 = v - 1;
                        if v != 0 {
                            if v == 18 {
                                self.ent[i].row156 = 85;
                                self.ent[i].f126 = self.ent[i].f130;
                            }
                        } else {
                            self.ent[i].f71 = 0;
                            self.ent[i].f126 = self.ent[i].f128;
                        }
                    }
                    _ => self.ent[i].f71 = 0,
                }
            }
            3 => {
                self.mc2_pack(i, M17_BASE);
                self.m17_validate(i);
            }
            4 => self.mc2_prekill(i, M17_BASE),
            5 => self.mc2_kill(i),
            6 => {} // unreachable
            _ => self.m17_validate(i),
        }
    }

    // =========================================================================
    // MODEL 18 — the slow tank (ctor sub_4C590 EF:34236, states
    // 0x90-97; ground-locked, 5-shot (9,0) fans)
    // =========================================================================

    pub(crate) fn mc2_spawn_m18(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 18;
            e.tick70 = M18_BASE + 3; // 147 — spawns into the pack slot (:34240)
            e.f28 = 1;
            e.f128 = 10;
            e.f130 = 6;
            e.max_life = 36000;
            e.f126 = 10;
        }
        self.mc2_set_mana_half(i);
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 500;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 86;
            e.f58 = 64;
            e.f66 = 3;
            e.f26 = 100; // :34262
        }
        self.ent[i].f63 = self.mc2_ord(18);
        // The ctor keeps the loader's z (EF:34262 passes the record
        // straight through); the state head snaps to ground on the
        // first TICK.
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 286);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `sub_252E0` (:16092): ground pin + state head; death routes to
    /// prekill.
    fn m18_head(&mut self, i: usize) -> u8 {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        let v = self.mc2_state_head(i);
        // ⭐ `sub_252E0`'s TAIL, BOTH ARMS (EF:16140-49): a HIT
        // (`result == 1`) latches the attacker `word_0x26_38` into
        // `word_0x96_150`; only DEATH (`result == 2`) writes the
        // action. The port kept the death half and dropped the latch,
        // so a roaming tank — whose `sub_25050` arm zeroes
        // `word_0x96_150` on every walking tick (EF:15960) — entered
        // the barrage machine with NO target and `sub_254E0` never
        // turned it. The `22` in `m18_face(i, ctx, 22)` is
        // `(4<<11)/360` and was always correct; there was nothing to
        // turn toward.
        if v == 1 && m18_hit_target_latch() {
            self.ent[i].f146 = self.ent[i].f40;
        }
        if v == 2 {
            self.ent[i].tick70 = M18_BASE + 4;
        }
        v
    }

    /// `sub_253B0` (:16155-229): enter (state base+`role`, sub-state
    /// `sub`) with the pinned duration table. ONLY the %-forms draw
    /// the per-entity LCG — the flat forms (0,≥2)/(2,1..3) draw
    /// NOTHING (an unconditional pre-draw would desync every tank's
    /// rand stream).
    pub(crate) fn m18_timer(&mut self, i: usize, role: u8, sub: u8) {
        self.ent[i].tick70 = M18_BASE + role;
        self.ent[i].f71 = sub;
        self.ent[i].f26 = match (role, sub) {
            (0, 0) => {
                let d = self.mc2_rand(i);
                (d % 400 + 400) as i16
            }
            (0, 1) => {
                let d = self.mc2_rand(i);
                (d % 60 + 60) as i16 // :16172-86, v4 = 60
            }
            (0, _) => return, // ≥2: no draw, f26 unchanged (:16187)
            (1, _) => {
                let d = self.mc2_rand(i);
                (d % 0x190 + 400) as i16
            }
            (2, 0) => {
                let d = self.mc2_rand(i);
                (d % 200 + 200) as i16
            }
            (2, 1) => 10, // flat, no draw (:16215)
            (2, 2) => 12, // flat, no draw (:16220)
            (2, 3) => 14, // flat, no draw (:16225)
            _ => return,  // (2,≥4): no draw, f26 unchanged
        };
    }

    /// `sub_254E0` (:16232): turn toward the target by `cap`. The
    /// LIVENESS-GUARDED entry — for the call sites whose retail arm
    /// really does reject a dead pointer.
    fn m18_face(&mut self, i: usize, ctx: &MobCtx, cap: i16) {
        if let Some(t) = self.mc2_target(self.ent[i].f146, ctx) {
            self.m18_face_at(i, t, cap);
        }
    }

    /// `sub_254E0` on the RAW pointer — the barrage's AIM phase.
    ///
    /// ⭐⭐⭐ **CASE 0 AIMS AT A CORPSE AND CASE 1 DOES NOT.**
    /// `sub_250B0`'s two arms sit eight lines apart and guard
    /// differently: case 0 is a bare
    /// `sub_254E0(a1x, Entities_EA3E4[a1x->word_0x96_150], 4u)`
    /// (EF:15995) with NO liveness test, while case 1 (EF:16028-31)
    /// rejects on `v5x <= Entities_EA3E4[0] || v5x->life_0x8 < 0 ||
    /// v5x->struct_byte_0xc_12_15.byte[1] & 4` and drops to sub-state
    /// 2. So a tank whose quarry dies mid-barrage KEEPS TRACKING the
    /// corpse through the aim phase, and only the firing phase lets go.
    /// The port routed both through `mc2_target`, whose baked-in life
    /// test is case 1's — an INVENTED GUARD on case 0, false exactly
    /// when the target dies ([`Gen::mc2_target_raw`] carries the law).
    ///
    /// WITNESS — mc2l15 pair 16733→16734, slot 3, a (5,18) in the
    /// barrage aim phase (`action45 146`, `f71 0`) tracking slot 47 —
    /// a `(10,0)` at **`life = -1`** on BOTH sides. Retail turns
    /// `yaw 1692 → 1714` (`turn_step(1692, 1897, 22)` = +22, the
    /// `(4<<11)/360` cap) while decrementing the timer and drawing;
    /// the port matched the timer and the draw and did not turn at
    /// all, nor rewrite `roll` — the exact signature of the resolver
    /// returning `None`.
    ///
    /// `MGC_NO_MC2_M18_AIM_UNGUARDED=1` restores the guarded lookup.
    fn m18_face_raw(&mut self, i: usize, ctx: &MobCtx, cap: i16) {
        let slot = self.ent[i].f146;
        let t = if no_mc2_m18_aim_unguarded() {
            self.mc2_target(slot, ctx)
        } else {
            self.mc2_target_raw(slot, ctx)
        };
        if let Some(t) = t {
            self.m18_face_at(i, t, cap);
        }
    }

    /// The body of `sub_254E0` once the target pointer is in hand:
    /// stamp the bearing into `roll_0x20_32`, step the yaw toward it by
    /// `sub_58350(.., 5, cap)` and fold to 11 bits (retail DOES mask
    /// here — EF:16238 — unlike the fan spawner, see
    /// [`Gen::mc2_atk_fan`]).
    fn m18_face_at(&mut self, i: usize, (tx, ty, _): (u16, u16, i16), cap: i16) {
        let e = &self.ent[i];
        let aim = Self::angle_between(e.x, e.y, tx, ty);
        self.ent[i].f34 = aim;
        let yaw = self.ent[i].f30;
        self.ent[i].f30 = (yaw as i32 + Self::turn_step(yaw, aim, cap) as i32) as u16 & 0x7FF;
    }

    pub(crate) fn m18_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M18_BASE {
            0 => {
                // sub_24E20 (:15841) — the watch/roam split on f71.
                let r = self.m18_head(i);
                if r == 1 {
                    self.m18_timer(i, 2, 0);
                    return;
                }
                if r != 0 {
                    return;
                }
                if self.ent[i].f71 != 0 {
                    // ⭐⭐⭐ THE WATCH ARM AIMS AT A CORPSE, TOO — the
                    // OWED sibling of [`Gen::m18_face_raw`]'s barrage
                    // law, and mc2l14 is its witness. Retail is
                    // `sub_24E20`'s `byte_0x46_70 == 1` branch,
                    // EF:15882-15902 (⚠ the stale EF:15872/15875/
                    // 15890-92 cites in this arm are ~16 lines short —
                    // the file has grown since). Disassembled from the
                    // shipped NETHERW.EXE (VA 0x24E20 = file 0x49620,
                    // VA + 0x24800), addresses shown as VAs:
                    //
                    //   0x24e4e  cmp  $1,%al        ; byte_0x46_70
                    //   0x24e50  je   0x24f9d       ; ...else RETURN
                    //   0x24f9d  cmpw $0,0x96(%ebx) ; word_0x96_150
                    //   0x24fa5  je   0x25044       ; ...RETURN
                    //   0x24fb4  mov  0x1a3e4(,%esi,4),%esi   ; RAW
                    //   0x24fc3  call 0x58490       ; EuclideanDistXYZ
                    //   0x24fd5  cmp  %edx,%eax     ; word_160_0x1c_28
                    //   0x24fd7  jl   0x24ff6       ; near ⇒ face+roll
                    //   0x24fde  movw $0,0x96(%ebx) ; far ⇒ drop
                    //   0x24ff6  push $4 / push %esi / call 0x254E0
                    //
                    // THE TABLE LOAD HAS NO LIFE / CLASS / REAP TEST,
                    // exactly like the barrage's case 0 eight hundred
                    // bytes away; the port routed it through
                    // `mc2_target`, whose baked-in guard belongs to the
                    // FIRING phase only. And the two `RETURN` exits
                    // above are bare — retail does NOT clear the target
                    // or re-arm the timer on them, where the port's
                    // `if let … else` fell through to the drop for
                    // *any* failed resolve (an extra `mc2_rand` draw
                    // plus an `f26` reload).
                    //
                    // WITNESS — mc2l14 pair 5939→5940, slot 38: the
                    // tank was shot by slot 66 at t=5932 (mail0.src 66
                    // ⇒ `f146 = 66`, `f71 = 1`, action 144) and had
                    // been turning at the `(4<<11)/360` = 22/tick cap
                    // toward bearing 1533 since t=5933. Slot 66 is
                    // REAP-FLAGGED on t=5940 (`flags.b1_reap4 0 → 1`).
                    // Retail turns 25 more ticks — yaw 17, 2043, 2021,
                    // … 1537 — and clamps onto 1533 at t=5965; the port
                    // stopped dead at yaw 17, cleared `target96` 66 → 0
                    // and `b46` 1 → 0, and reloaded `scratch10` 61 →
                    // 681 (the `(0,0)` timer's `d % 400 + 400`).
                    // Retail's real exit is the `% 0x31` roll, which
                    // hits at t=5973: action 144 → 146 with
                    // `scratch10 = 55191 % 200 + 200 = 391`.
                    let slot = self.ent[i].f146;
                    if !no_mc2_m18_watch_unguarded() && (self.ent[i].f71 != 1 || slot == 0) {
                        return; // VA 0x24e56 / 0x24fa5 — bare `return`
                    }
                    let t = if no_mc2_m18_watch_unguarded() {
                        self.mc2_target(slot, ctx)
                    } else {
                        self.mc2_target_raw(slot, ctx)
                    };
                    if let Some((tx, ty, _tz)) = t {
                        let e = &self.ent[i];
                        // 2-D: retail's `EuclideanDistXYZ_58490`
                        // (EF:15888) never reads z (2-D despite the
                        // name) — VA 0x58490 reads only `+0`/`+2` and
                        // sqrts the sum, verified in NETHERW.EXE at
                        // file 0x7cc90.
                        let d = crate::mc2::morph::dist2d(e.x, e.y, tx as i32, ty as i32) as u32;
                        if d < BEHAVIOR[e.row156 as usize].v_28 as u32 {
                            // VA 0x24ff6: retail hands `sub_254E0` the
                            // SAME `v10x` it just measured — one
                            // lookup, not two.
                            self.m18_face_at(i, (tx, ty, _tz), 22); // (4<<11)/360 (EF:15891)
                            let d2 = self.mc2_rand(i);
                            if d2 % 0x31 == 0 {
                                self.m18_timer(i, 2, 0);
                            }
                            return;
                        }
                    }
                    self.ent[i].f146 = 0;
                    self.m18_timer(i, 0, 0);
                } else {
                    self.ent[i].f26 -= 1;
                    // EF:15890-92 — retail tests the post value for `!= 0`.
                    if self.ent[i].f26 != 0 {
                        if self.ent[i].f58 != 0 {
                            let d = self.mc2_rand(i);
                            if d & 1 == 0
                                && let Some(t) = self.mc2_wizard_scan(i, ctx, false)
                            {
                                self.ent[i].f146 = t;
                                self.m18_timer(i, 0, 1);
                            }
                        }
                    } else {
                        self.m18_timer(i, 1, 0);
                    }
                }
            }
            1 => {
                // sub_25050 (:15952) — the walk.
                let r = self.m18_head(i);
                if r == 1 {
                    self.m18_timer(i, 0, 1);
                } else if r == 0 {
                    self.ent[i].f146 = 0;
                    self.mc2_move_core(i);
                    self.ent[i].f26 -= 1;
                    if self.ent[i].f26 <= 0 {
                        self.m18_timer(i, 0, 0);
                    }
                }
            }
            2 => {
                // sub_250B0 (:15976) — the barrage machine.
                let v2 = self.m18_head(i);
                if v2 > 1 {
                    return;
                }
                match self.ent[i].f71 {
                    0 => {
                        // EF:15995 — UNGUARDED: retail hands
                        // `Entities_EA3E4[word_0x96_150]` straight to
                        // `sub_254E0` with no liveness test, so the aim
                        // phase tracks a corpse. See [`Self::m18_face_raw`].
                        // 🏦 OWED: the ROAM arm (`sub_24E20` EF:15872-76,
                        // `m18_tick`'s `0 =>`) is unguarded in retail too —
                        // its only gate is `word_0x96_150 != 0`, and the
                        // DISTANCE test runs on the raw pointer as well —
                        // but it has no witness on this corpus yet, so it
                        // keeps the guarded lookup pending its own A/B.
                        self.m18_face_raw(i, ctx, 22); // (4<<11)/360
                        if v2 == 1 {
                            self.ent[i].f26 -= 47;
                            if self.ent[i].f26 < 0 {
                                self.m18_timer(i, 2, 1);
                            }
                        } else {
                            let d = self.mc2_rand(i);
                            if d % 0x29 != 0 {
                                self.ent[i].f26 -= 1;
                                if self.ent[i].f26 < 0 {
                                    self.m18_timer(i, 2, 2);
                                }
                            } else {
                                self.m18_timer(i, 2, 1);
                            }
                        }
                    }
                    1 => {
                        self.ent[i].f26 -= 1;
                        if self.ent[i].f26 <= 0 {
                            self.m18_timer(i, 2, 2);
                            return;
                        }
                        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
                        if self.ent[i].f63 as i16 % period == 0 {
                            let slot = self.ent[i].f146;
                            if self.mc2_target(slot, ctx).is_none() {
                                self.m18_timer(i, 2, 2);
                                return;
                            }
                            self.m18_face(i, ctx, 0x400); // barrage-1 inlines the 0x400 snap (EF:16038)
                            self.mc2_atk_fan(i, slot, ctx);
                        }
                    }
                    2 => {
                        self.ent[i].f26 -= 1;
                        if self.ent[i].f26 <= 0 {
                            self.m18_timer(i, 2, 3);
                        }
                    }
                    // EF:16050-63 — retail's case 3 is EXPLICIT and its
                    // `default:` RETURNS. Nothing seeds a sub-state past
                    // 3 today, so a catch-all was equivalent; keep the
                    // arm literal so a future sub-state cannot silently
                    // inherit the spin-down body.
                    3 => {
                        self.ent[i].f26 -= 1;
                        if self.ent[i].f26 < 0 {
                            self.m18_timer(i, 1, 0);
                        } else if self.ent[i].f26 >= 8 {
                            let yaw = (self.ent[i].f30 + 170) & 0x7FF;
                            self.ent[i].f30 = yaw;
                            self.ent[i].f34 = yaw;
                        }
                    }
                    _ => {} // EF:16065 — `default: return;` (nothing follows either match)
                }
            }
            3 => self.m18_timer(i, 0, 0), // :16074 — re-enter roam
            4 => self.mc2_prekill(i, M18_BASE),
            5 => self.mc2_kill(i),
            6 => {} // unreachable
            _ => {
                // +7 (:16247): ground-lock; a chase entry re-arms.
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                self.ent[i].z = self.ground_z(x, y) as i16;
                if self.ent[i].tick70 == M18_BASE + 2 {
                    self.m18_timer(i, 2, 0);
                }
            }
        }
    }

    // =========================================================================
    // MODEL 19 — the firebug flyer (ctor sub_4C6B0 EF:34271, states
    // 0x98-9F; level-000's final wave — flank, hover, strafe-bolt,
    // dive-melee; flight = handler-driven z writes)
    // =========================================================================

    pub(crate) fn mc2_spawn_m19(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 19;
            e.tick70 = M19_BASE + 1; // 153
            e.f28 = 1;
            e.f128 = 76;
            e.f130 = 8;
            e.max_life = 600;
            e.f126 = 76;
        }
        self.mc2_set_mana_half(i); // 300
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 300;
            e.f66 = 3;
            e.f67 = 0;
            e.f26 = (i % 100) as i16; // kept (:34290)
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 88;
        }
        let ord = self.mc2_ord(19);
        self.ent[i].f63 = ord;
        self.ent[i].f58 = Self::mc2_wake_stagger(88, ord); // :34297 staggered wake
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 287);
        self.mc2_shift_rot(i, 85, 51);
        Some(i)
    }

    fn m19_reset(&mut self, i: usize) {
        if self.ent[i].tick70 == M19_BASE + 2 {
            self.ent[i].f71 = 0;
        }
    }

    /// The flank point 2048 ahead of the target's facing (:16379).
    fn m19_flank(&mut self, i: usize, ctx: &MobCtx, jitter: bool) -> Option<(u16, u16, i16)> {
        let slot = self.ent[i].f146;
        let (tx, ty, tz) = self.mc2_target(slot, ctx)?;
        let tyaw = if slot == PLAYER_TARGET {
            ctx.pyaw
        } else {
            self.ent[slot as usize].f30
        };
        let ang = if jitter {
            let d = self.mc2_rand(i);
            (tyaw as i32 - 256 + (((d % 0x5A) << 11) / 360) as i32) as u16 & 0x7FF
        } else {
            tyaw
        };
        let mut p = (tx, ty, tz);
        Self::polar_step(&mut p, ang, 0, 2048);
        Some(p)
    }

    pub(crate) fn m19_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M19_BASE {
            0 => {
                self.mc2_patrol(i, M19_BASE);
                self.m19_reset(i);
            }
            1 => {
                self.mc2_idle(i, M19_BASE, ctx);
                self.m19_reset(i);
            }
            2 => self.m19_attack(i, ctx),
            3 => {
                self.mc2_pack(i, M19_BASE);
                self.m19_reset(i);
            }
            4 => self.mc2_prekill(i, M19_BASE),
            5 => self.mc2_kill(i),
            6 => {} // unreachable; the hit response is +2 case 7
            _ => self.m19_reset(i),
        }
    }

    /// `HitFirebug_25610` (:16281) — the attack-run machine.
    pub(crate) fn m19_attack(&mut self, i: usize, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            1 => {
                // The hit arm is a TAIL arm (`else if (v1 <= 1)`,
                // EF:16565-69): latch the dive + retarget the
                // attacker and END the dispatch — no move core, no
                // rand, no dive step until next tick (mc2l3 t=252:
                // the ground-fire hit tick, retail's record freezes
                // everything but life/f71 while the fallen-through
                // port dove same-tick).
                self.ent[i].f71 = 7; // damage → straight into a dive
                self.ent[i].f146 = self.ent[i].f40;
                return;
            }
            2 => {
                self.ent[i].tick70 = M19_BASE + 4;
                return;
            }
            _ => {}
        }
        self.mc2_move_core(i);
        let slot = self.ent[i].f146;
        let Some((tx, ty, tz)) = self.mc2_target(slot, ctx) else {
            self.ent[i].tick70 = M19_BASE + 1;
            self.ent[i].f126 = self.ent[i].f128;
            return;
        };
        let period = BEHAVIOR[self.ent[i].row156 as usize].v_26.max(1);
        loop {
            match self.ent[i].f71 {
                0 => {
                    self.ent[i].f126 = self.ent[i].f128;
                    self.ent[i].f71 = 1;
                }
                1 => {
                    let Some(p) = self.m19_flank(i, ctx, true) else {
                        break;
                    };
                    let e = &self.ent[i];
                    if Self::mc2_dist3((e.x, e.y, e.z), p) <= 0x500 {
                        // Retail case 1 (EF:16386-88) sets byte_0x46=2 and
                        // RETURNS (:16407) — the flank roll has already fired,
                        // actSpeed is left at minSpeed, and case 2 (which drops
                        // to maxSpeed + rolls again) runs only NEXT tick. A
                        // `continue` here would collapse that two-tick
                        // transition into one, dropping actSpeed to maxSpeed a
                        // tick early and double-rolling rand.
                        self.ent[i].f71 = 2;
                        break;
                    }
                    let e = &self.ent[i];
                    self.ent[i].f34 = Self::angle_between(e.x, e.y, p.0, p.1);
                    if self.ent[i].f63 & 3 == 0 {
                        self.mc2_avoid_packmate(i);
                    }
                    break;
                }
                2 => {
                    self.ent[i].f126 = self.ent[i].f130;
                    let d = self.mc2_rand(i);
                    self.ent[i].f26 = ((d & 0x3FF) as i32 + tz as i32) as i16; // hover altitude
                    self.ent[i].f71 = 3;
                }
                3 => {
                    // Aim runs EVERY tick, BEFORE the gate
                    // (EF:16419); the `f63 & 3` gate covers all of
                    // the avoidance/flank/roll below (EF:16420).
                    {
                        let e = &self.ent[i];
                        self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                    }
                    if self.ent[i].f63 & 3 != 0 {
                        break;
                    }
                    self.mc2_avoid_packmate(i);
                    let Some(p) = self.m19_flank(i, ctx, false) else {
                        break;
                    };
                    let e = &self.ent[i];
                    if Self::mc2_dist3((e.x, e.y, e.z), p) > 0x500 {
                        self.ent[i].f71 = 0;
                        break;
                    }
                    let d = self.mc2_rand(i);
                    let v16 = d % 0x11F;
                    // CASCADING independent ifs (EF:16449-55): a
                    // 0x3F-multiple arms 6, a 0x1F-multiple then
                    // OVERRIDES to 7, zero lands 4; the bob fires on
                    // every multiple of 4 including the above (the
                    // else-if chain made 4 unreachable and 6 final).
                    if v16 & 0x3F == 0 {
                        self.ent[i].f71 = 6;
                    }
                    if v16 & 0x1F == 0 {
                        self.ent[i].f71 = 7;
                    }
                    if v16 == 0 {
                        self.ent[i].f71 = 4;
                    }
                    if v16 & 3 == 0 {
                        // The vertical bob toward the hover altitude —
                        // the flying evidence (:16455-61).
                        let hover = self.ent[i].f26;
                        self.ent[i].z += if self.ent[i].z <= hover { 64 } else { -64 };
                    }
                    break;
                }
                4 => {
                    self.ent[i].f126 = self.ent[i].f128;
                    self.ent[i].f71 = 5;
                }
                5 => {
                    // LABEL_81 (EF:16472-93): the `phase & 3` gate
                    // covers the TARGET AIM and the pack-avoid both —
                    // aim first, pack override second. (Case 3's aim
                    // is the opposite: pre-gate, every tick.)
                    if self.ent[i].f63 & 3 == 0 {
                        {
                            let e = &self.ent[i];
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                        }
                        self.mc2_avoid_packmate(i);
                    }
                    if self.ent[i].f63 as i16 % period == 0 {
                        let e = &self.ent[i];
                        if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz))
                            < BEHAVIOR[e.row156 as usize].v_28 as u32
                        {
                            self.mc2_atk_bolt(i, slot, ctx);
                        } else {
                            self.ent[i].f71 = 6;
                        }
                    }
                    break;
                }
                6 => {
                    self.ent[i].tick70 = M19_BASE + 1;
                    self.ent[i].f126 = self.ent[i].f128;
                    break;
                }
                7 => {
                    let d = self.mc2_rand(i);
                    self.ent[i].f126 = 3 * self.ent[i].f128; // 228
                    self.snd(((d & 1) + 43) as u8, i);
                    self.ent[i].f26 = 24;
                    self.ent[i].f71 = 8;
                }
                _ => {
                    // 8/9 — the dive-melee (:16542-63).
                    self.ent[i].f26 -= 1;
                    // EF:16517-19 — retail tests the post value for `== 0`.
                    if self.ent[i].f26 == 0 {
                        self.ent[i].f71 = 0;
                        break;
                    }
                    // LABEL_59's phase gate (EF:16521) jumps past
                    // BOTH the `v21 > 16` re-aim and the pack-avoid
                    // to LABEL_70 — the dive aims only on phase-0
                    // ticks (mc2l3 t=242: slot 148's roll froze at
                    // 1932 through its 7→8 entry tick, phase 10&3=2,
                    // while the ungated port aim wrote 1936 — the
                    // head standing right behind the castle ball).
                    if self.ent[i].f63 & 3 == 0 {
                        if self.ent[i].f26 > 16 {
                            let e = &self.ent[i];
                            self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                        }
                        self.mc2_avoid_packmate(i);
                    }
                    let dz = (tz as i32 - self.ent[i].z as i32).clamp(-64, 64);
                    self.ent[i].z = self.ent[i].z.wrapping_add(dz as i16);
                    if self.ent[i].f71 == 8 && self.mc2_atk_melee_768(i, slot, ctx) {
                        self.ent[i].f71 = 9;
                    }
                    if self.ent[i].f63 as i16 % period == 0 {
                        let e = &self.ent[i];
                        if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz))
                            >= BEHAVIOR[e.row156 as usize].v_28 as u32
                        {
                            self.ent[i].f71 = 6;
                        }
                    }
                    break;
                }
            }
        }
    }

    // =========================================================================
    // MODEL 20 — dual-mode skirmisher (ctor sub_4C7F0 EF:34307,
    // states 0xA0-A7; (9,21) arcs at range, 1024-melee rushes close)
    // =========================================================================

    pub(crate) fn mc2_spawn_m20(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 20;
            e.tick70 = M20_BASE + 1; // 161
            e.f28 = 1;
            e.f128 = 32;
            e.f130 = 20;
            e.max_life = 5500;
            e.f126 = 32;
        }
        self.mc2_set_mana_half(i);
        {
            // :34320 — fov zeroed BEFORE the draw's facing writes.
            self.ent[i].f36 = 0;
        }
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f44 = 100;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 89;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(20);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 288);
        self.mc2_shift_rot(i, 384, 512);
        Some(i)
    }

    /// The state-2 lock validator — retail's `sub_25D80` (patrol,
    /// EF:16619) and `sub_25DE0` (idle, EF:16637) tails, VERBATIM: if
    /// the wrapped state body left us at action 162, re-read
    /// `word_0x96_150` and clear it unless the record is class 3 and
    /// model 0 or 1, then zero `byte_0x46_70`.
    ///
    /// ⚠ THIS WAS LABELLED "the wizard-validation wrapper DEVIATION"
    /// for months and it is nothing of the kind — it is the decompile
    /// line for line. The mislabel survived because the scan it
    /// guards had an invented `model65 <= 1` filter, which made the
    /// clear unreachable and left the wrapper looking like an
    /// unmotivated port-side extra. Both halves of that were wrong at
    /// once: retail's `sub_1BF90` sweep has NO model test precisely
    /// because this wrapper does it afterwards, and the difference is
    /// live — the scan is nearest-in-cone, so a castle it must be
    /// allowed to WIN is a castle that stops a farther wizard from
    /// winning (mc2l3 t=10222, [`Gen::mc2_wizard_scan`]).
    ///
    /// *When a guard looks unmotivated, check what its caller was
    /// prevented from producing before calling the guard the deviation.*
    fn m20_validate(&mut self, i: usize) {
        if self.ent[i].tick70 == M20_BASE + 2 {
            let t = self.ent[i].f146;
            if !self.mc2_is_wizard(t) {
                self.ent[i].f146 = 0;
            }
            self.ent[i].f71 = 0;
        }
    }

    pub(crate) fn m20_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M20_BASE {
            0 => {
                self.mc2_patrol(i, M20_BASE);
                self.m20_validate(i);
            }
            1 => {
                self.mc2_idle(i, M20_BASE, ctx);
                self.m20_validate(i);
            }
            2 => {
                // sub_25E40 (:16649).
                //
                // ⭐⭐⭐ THE HEAD TEST IS A BARE POINTER TEST (:16661-66)
                // — `Entities_EA3E4[word_0x96_150] <= Entities_EA3E4[0]`
                // and nothing else. NO life test, NO reap test, NO
                // class test. The port asked `mc2_target`, which
                // carries all three, and so dropped out of state 162
                // on a target the chase would still have honoured.
                //
                // The life/reap guard is REAL but it lives one level
                // down, in `sub_1C310`'s QUIET arm (EF:9297-9302,
                // `mc2_chase_attack`) — reached only when the
                // creature took no damage this tick. So a m20 that is
                // itself being hit never re-tests its target at all,
                // and the arms below have already committed by then.
                //
                // mc2l3 t=11731: the player is dead (life −720) and
                // this m20 at slot 161 takes 160 damage on the same
                // tick. Retail's `byte_0x46_70` 1 → 2 arm fires
                // regardless — `dword_0x10_16` = 32, `actSpeed` =
                // 2*minSpeed = 64 — then `sub_1C310` takes its
                // damaged path and never looks at the target. The
                // port bailed to 161 with speed 32, and left the 160
                // damage sitting unconsumed in the inbox.
                if !self.mc2_target_ptr(self.ent[i].f146) {
                    self.ent[i].tick70 = M20_BASE + 1;
                    self.ent[i].f126 = self.ent[i].f128;
                    return;
                }
                self.snd(32, i);
                match self.ent[i].f71 {
                    0 => {
                        // Approach + (9,21) arcs. ⭐⭐ THE COMMIT TEST
                        // IS TARGET-KEYED AND AGAINST A WIZARD IT
                        // DOES NOT READ THE ATTACK AT ALL (EF:16673-
                        // 79): the chase runs either way, but
                        //   v4 = (target is class-3 model-0)
                        //        ? (target.mobilizeCounter == 0)
                        //        : (attack == 0);
                        //   if (!v4) byte_0x46_70 = 1;
                        // — so a m20 only commits its melee rush on a
                        // PARALYZED human, never on a landed lob. The
                        // old note here called the counter "MC2 flight
                        // state, not modeled"; `Mc2Ext::mobilize` has
                        // modeled it all along and `Gen::mc2_mobilize`
                        // is the pool-side mirror. mc2l3 t=1294: the
                        // port's lob landed and set f71 = 1, retail
                        // held 0 through the whole window, and the
                        // rush doubled slot 145's speed to 64 at 1295.
                        let hit = self.mc2_chase_attack(i, M20_BASE, ctx, Self::mc2_atk_lob21);
                        let t = self.ent[i].f146;
                        let wizard = t == PLAYER_TARGET
                            || (t != 0
                                && (t as usize) < self.ent.len()
                                && self.ent[t as usize].class64 == 3
                                && self.ent[t as usize].model65 == 0);
                        let commit = if wizard {
                            self.mc2_mobilize.0 != 0
                        } else {
                            hit
                        };
                        if commit {
                            self.ent[i].f71 = 1;
                        }
                    }
                    1 => {
                        self.ent[i].f71 = 2;
                        self.ent[i].f26 = 32;
                        self.ent[i].f126 = 2 * self.ent[i].f128; // 64
                        let hit = self.mc2_chase_attack(i, M20_BASE, ctx, Self::mc2_atk_melee_1024);
                        self.ent[i].f26 -= 1;
                        // EF:16699 — retail is `if (!(--f26))`, an exact `== 0`.
                        if hit || self.ent[i].f26 == 0 {
                            self.ent[i].f71 = 0;
                            self.ent[i].f126 = self.ent[i].f128;
                        }
                    }
                    _ => {
                        let hit = self.mc2_chase_attack(i, M20_BASE, ctx, Self::mc2_atk_melee_1024);
                        self.ent[i].f26 -= 1;
                        // EF:16699 — retail is `if (!(--f26))`, an exact `== 0`.
                        if hit || self.ent[i].f26 == 0 {
                            self.ent[i].f71 = 0;
                            self.ent[i].f126 = self.ent[i].f128;
                        }
                    }
                }
                if self.ent[i].tick70 != M20_BASE + 2 {
                    self.ent[i].f126 = self.ent[i].f128;
                }
            }
            3 => {
                self.mc2_pack(i, M20_BASE);
                self.m20_validate(i);
            }
            4 => self.mc2_prekill(i, M20_BASE),
            5 => self.mc2_kill(i),
            6 => {} // unreachable
            _ => self.m20_validate(i),
        }
    }

    // =========================================================================
    // MODEL 21 — the DEVIL, a frog-jumping caster (ctor sub_4C8F0
    // EF:34340, states 0xA8-AF; the sub_265A0 jump cycle + (9,0)
    // bolts; the third most-authored creature; trace
    // docs/traces/mc2-m21-jump-m26-steal.md §A)
    // =========================================================================

    pub(crate) fn mc2_spawn_m21(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 21;
            // actSpeed = maxSpeed READ BEFORE ANY WRITE — NewEvent's
            // zero, verbatim bug-compatible (:34345).
            e.f126 = e.f130;
            e.tick70 = M21_BASE + 1; // 169
            e.f28 = 1;
            e.f128 = 96;
            e.max_life = 1000;
            e.f140 = 1000;
            e.f36 = 0;
        }
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            // Retail's `subSpellIndex_0x2A_42 = 400` has no port home
            // (the bolt thunk hard-sets 500); f44 is the jump impulse
            // `word_0x2C_44`, ctor'd 0 (:34367).
            e.f44 = 0;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 96;
            e.f58 = 64;
            e.f66 = 3;
            e.f71 = 0; // byte_0x46_70 — jump state: landed rest
            e.f26 = 0; // byte_0x44_68 — rest countdown (:34368)
            e.f68 = 64; // byte_0x43_67 — rest base (sub_268F0(1) post-ctor)
        }
        self.ent[i].f63 = self.mc2_ord(21);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.m21_pose(i);
        self.mc2_shift_rot(i, 128, 128);
        Some(i)
    }

    /// `sub_26500` (:16970): sprite by jump-cycle state. The pose
    /// selector calls ONLY SetEntityIndexAndRot (which derives the
    /// whole applied quad from the sprite-param row); the one
    /// `SetEntityShiftRot(128,128)` in the machine is the CTOR's
    /// tail (:34371) — a per-tick re-stamp here clobbered the
    /// sprite-derived pitch/roll forever (mc2l24 t=7918: 47 devils
    /// applied_pitch 128 vs retail's row-derived 56) and fed the
    /// walker's f80/f82 step cap a 128 half-extent.
    fn m21_pose(&mut self, i: usize) {
        let sprite = match self.ent[i].f71 {
            0 => 311,
            1..=3 | 9 => 308,
            4 => 309,
            5 => 310,
            6 => 305,
            7 => 306,
            8 => 307,
            _ => 312,
        };
        if self.ent[i].type86 != sprite {
            self.mc2_set_sprite(i, sprite);
        }
    }

    /// `sub_268F0` (:17212): mode switch — 1 idle (can-turn 64,
    /// target cleared), 2 attack.
    /// `sub_268F0` (:17222) — the m21 mode write. The action byte is
    /// `a2 - 88` in u8 arithmetic, which for `a2 = mode` is exactly
    /// `M21_BASE + mode`; and **only modes 1 and 2 carry a side
    /// effect** — 1 arms the idle rest base and drops the target
    /// (`word_0x96_150 = 0`), 2 zeroes the rest base (the attack's
    /// single-draw rest). Retail leaves `byte_0x43_67` ALONE for
    /// every other `a2`, including 0.
    ///
    /// ⚠ The bare `else` that zeroed f68 for every non-1 mode was an
    /// INVENTED ARM — harmless while only 1 and 2 were ever passed,
    /// and false the moment the wrapper tail re-applies a LIVE action
    /// ([`Self::m21_wrapper_tail`], which passes whatever the legs
    /// left behind).
    fn m21_mode(&mut self, i: usize, mode: u8) {
        if mode == 1 {
            self.ent[i].f68 = 64;
            self.ent[i].f146 = 0;
        } else if mode == 2 {
            self.ent[i].f68 = 0;
        }
        self.ent[i].tick70 = M21_BASE.wrapping_add(mode);
    }

    /// `sub_26470`'s TAIL (EF:16963-65) — the LAST two lines of m21's
    /// phase-7 wrapper, after the `sub_1D5D0` legs and the
    /// `StageVar2` switch:
    /// ```text
    ///   if (a1x->actionIndex_0x45_69 != 175)
    ///       sub_268F0(a1x, a1x->actionIndex_0x45_69 + 88);
    /// ```
    /// ⭐ **IT IS AN IDENTITY ON THE ACTION AND A PURE SIDE EFFECT.**
    /// `a2 = action + 88` is u8 arithmetic and `action = M21_BASE + k`
    /// = `168 + k`, so `a2 = 256 + k ≡ k`, and `sub_268F0` then writes
    /// `action = a2 - 88` — the same byte back. The call exists only to
    /// RE-APPLY THE MODE SIDE EFFECT to whatever action the legs just
    /// promoted the devil to. A non-lethal hit leaves it at
    /// `M21_BASE + 2`, so the tail zeroes the rest base — which turns
    /// the state-9 rest draw from TWO entity draws into ONE
    /// (`m21_jump`'s div-by-zero special case).
    ///
    /// ⚠ It sits OUTSIDE the switch and retail's `default:` arm
    /// **breaks** (unlike `sub_1F300`'s m0 twin, which `return`s and
    /// has no tail at all), so the tail runs for EVERY `StageVar2` —
    /// held kinds, kind 0, and the unhandled ones alike.
    ///
    /// WITNESS — mc2l15 pair 7527→7528, slot 4: the stage-held (5,21)
    /// takes the human's 160 (`mail0.src 165`), `sv2 6 → 10`,
    /// `action 175 → 170`, and retail writes **`b43 64 → 0`**. The port
    /// reproduced every other field and held `b43 = 64`; the extra rest
    /// draw then desynced the entity LCG, surfacing 47 ticks later as
    /// the take's FIRST divergence — slot 4 `rand` retail 28492 vs port
    /// 54187 at t=7575.
    ///
    /// `MGC_NO_MC2_M21_WRAPPER_TAIL=1` restores the old behaviour.
    pub(crate) fn m21_wrapper_tail(&mut self, i: usize) {
        if no_mc2_m21_wrapper_tail() {
            return;
        }
        let k = self.ent[i].tick70.wrapping_sub(M21_BASE);
        if k != 7 {
            self.m21_mode(i, k);
        }
    }

    /// `sub_26930` (:17234-44): yaw may commit only at the landing
    /// tick (state 9), or — for a wading devil — on its aligned
    /// ticks (`!(f63 & 7)`). Direction commits at landing.
    fn m21_can_turn(&self, i: usize) -> bool {
        let s = self.ent[i].f71;
        s == 9 || (s == 10 && self.ent[i].f63 & 7 == 0)
    }

    /// `sub_265A0` (:17010-151) — the frog-jump cycle, VERBATIM.
    /// Field homes: f71 = `byte_0x46_70` jump state,
    /// f44 = `word_0x2C_44` SIGNED impulse, f26 = `byte_0x44_68`
    /// rest countdown, f68 = `byte_0x43_67` rest base (64 idle / 0
    /// attack via [`Self::m21_mode`]). All draws on the ENTITY LCG;
    /// state 9 draws the cackle roll always, the rest roll only when
    /// the base is nonzero (the div-by-zero special case — attack
    /// rests 1 tick on a single draw). The XY veto (`v13` clear →
    /// retail `byte[1] |= 8`) is F_STOP, consumed by the NEXT tick's
    /// walker — both handlers call the walker first; the one-tick
    /// lag is authentic. Also the stage-HELD devil's ambient physics
    /// (`sub_26470` EF:16938-61 runs this after the 1D5D0 legs —
    /// the stagevars held seam).
    pub(crate) fn m21_jump(&mut self, i: usize) {
        let mut v12 = false; // settle: z -= 42 this tick
        let mut v13 = true; // moved: XY allowed this tick
        match self.ent[i].f71 {
            // Landed rest: countdown, then crouch.
            0 | 1 => {
                let n = self.ent[i].f26;
                if n != 0 {
                    self.ent[i].f26 = n - 1;
                } else {
                    self.ent[i].f71 = 2;
                }
                v12 = true;
                v13 = false;
            }
            2 => {
                v12 = true;
                self.ent[i].f71 = 3;
                v13 = false;
            }
            // Launch: airborne — XY moves, z rides the (spent)
            // impulse through the integrator, floored at terrain.
            3 => self.ent[i].f71 = 4,
            // Impulse seed: rand%100 + 140.
            4 => {
                let d = self.mc2_rand(i);
                self.ent[i].f44 = (d % 0x64 + 140) as u16;
                self.ent[i].f71 = 5;
            }
            // Rise → apex (the integrator decays the impulse).
            5 => {
                if (self.ent[i].f44 as i16) < 0 {
                    self.ent[i].f71 = 6;
                }
            }
            // Fall until 230 above the terrain.
            6 => {
                let e = &self.ent[i];
                let ground = self.ground_z(e.x, e.y) as i16;
                if (self.ent[i].z as i32) - (ground as i32) < 230 {
                    self.ent[i].f71 = 7;
                }
            }
            // Pre-land: STILL FALLING (v12 stays 0 — the re-extract's
            // correction to the trace table), XY frozen.
            7 => {
                self.ent[i].f71 = 8;
                v13 = false;
            }
            8 => {
                v12 = true;
                self.ent[i].f71 = 9;
                v13 = false;
            }
            // Landing: cackle roll (always), rest roll (base != 0),
            // land state by rest parity (even → 1, odd → 0).
            9 => {
                let d = self.mc2_rand(i);
                if d % 0xB == 0 {
                    self.snd(42, i);
                }
                let base = self.ent[i].f68;
                if base != 0 {
                    let d = self.mc2_rand(i);
                    let r = (d % base as u32) as i16;
                    self.ent[i].f26 = r;
                    self.ent[i].f71 = (r & 1 == 0) as u8;
                } else {
                    self.ent[i].f26 = 1;
                    self.ent[i].f71 = 0;
                }
                v12 = true;
                v13 = false;
            }
            // 0xA WATER WADE: settle z, XY keeps walking.
            _ => v12 = true,
        }
        // Shared tail (:17098-151): integrator → floor clamp → cave
        // ceiling clamp → water enter/exit → speed → sprite → veto.
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        if v12 {
            self.ent[i].z = self.ent[i].z.wrapping_sub(42);
        } else {
            let imp = self.ent[i].f44 as i16;
            self.ent[i].z = self.ent[i].z.wrapping_add(imp);
            self.ent[i].f44 = imp.wrapping_sub(42) as u16;
        }
        if self.ent[i].z < ground {
            self.ent[i].z = ground;
        }
        if self.is_cave() {
            // Ceiling − the params fov (EF:17111-20); impulse zeroed.
            let c = (self.ceiling_z(x, y) as i16 as i32 - self.ent[i].f84 as i32) as i16;
            if self.ent[i].z > c {
                self.ent[i].f44 = 0;
                self.ent[i].z = c;
            }
        }
        let attack = self.ent[i].tick70 == M21_BASE + 2;
        let speed = if self.cap_bit(x, y) == 1 {
            if self.ent[i].f71 == 10 {
                if self.ent[i].z > ground {
                    self.ent[i].f71 = 0; // lifted off the surface
                }
            } else if self.ent[i].z == ground {
                // Grounded on a water tile → wade + (10,5) splash.
                //
                // ⭐⭐⭐ THE SPLASH IS NOT SPAWNED AT THE WALKER. Retail
                // pushes the GLOBAL scratch axis, not `&a1x->position`
                // — and this function never writes that global, so the
                // splash lands wherever the LAST ENTITY TO COMMIT A
                // MOVE THIS TICK went (`Gen::mc2_pred_axis`). Shipped
                // `NETHERW.EXE`, file 0x4AFAD-0x4AFC7 (VA 0x267AD, the
                // region rule file = VA + 0x24800; EF:17131):
                //   4afad: 0f bf 43 50     movsx eax,[ebx+0x50]   ; z
                //   4afb1: 39 f0           cmp   eax,esi          ; ground
                //   4afb3: 75 23           jne   0x4afd8
                //   4afb5: 6a 05           push  0x5              ; model
                //   4afb7: 6a 0a           push  0xa              ; class
                //   4afb9: 68 98 b3 01 00  push  0x1b398          ; &predictedAxis
                //   4afbe: c6 43 46 0a     mov   BYTE [ebx+0x46],0xa
                //   4afc2: e8 c9 39 02 00  call  0x6e990          ; _4A190
                // Contrast the three sites just above it — 0x4af26,
                // 0x4af71, 0x4af96 — which all do `lea eax,[ebx+0x4c];
                // push eax` for the walker's OWN position. The
                // immediate 0x1b398 is the same global m15's wander
                // writes at file 0x48A18/0x48A50 (`push 0x1b398` into
                // `MoveEntity_57FA0` then `CopyEntityPosition_57CF0`).
                //
                // WITNESS (mc2l24 t=27275, `explain`): slot 377 (5,21)
                // wades at (44906, 38618) and the port put its splash
                // there; retail's slot 778 (10,5) is BORN at (19072,
                // 34928, z 512) — the exact post-move position of slot
                // 305, the (5,15) guard that ticked earlier in the very
                // same tick. The tile-chain links prove the order:
                // 778's `@0x16` = 305 (the cell's previous head) and
                // 305's `@0x18` = 778.
                self.ent[i].f71 = 10;
                let (sx, sy, sz) = if crate::mc2::roster::mc2_splash_pred_axis_law() {
                    self.mc2_pred_axis.0
                } else {
                    (x, y, self.ent[i].z)
                };
                self.mc2_spawn_splash(sx, sy, sz);
            }
            if attack { 66 } else { 40 }
        } else {
            if self.ent[i].f71 == 10 {
                self.ent[i].f71 = 0;
            }
            if attack { 96 } else { 60 }
        };
        self.ent[i].f126 = speed;
        self.m21_pose(i);
        if !v13 {
            self.ent[i].flags |= super::mobs::F_STOP;
        }
    }

    pub(crate) fn m21_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M21_BASE {
            0 | 3 => self.m21_mode(i, 1),
            1 => {
                // sub_26070 (:16760).
                // ⭐ THE HIT TICK STILL JUMPS. `sub_26830`'s hit arm
                // re-targets and switches the devil to ATTACK
                // (:17196-99 — `word_0x96_150 = word_0x26_38` then
                // `sub_268F0(self, 2)`), and the caller then runs the
                // WHOLE body for head 0 **OR** 1: `:16777`
                // `if ((unsigned int)v1 <= 1)`, and in the shipped EXE
                // `cmp $0x1,%eax; ja` at NETHERW.EXE 0x4A885 (MC2 file
                // offset = linear + 0x24800). So a non-lethally hit
                // devil still steps, still advances the jump cycle,
                // and `sub_265A0` still writes `actSpeed_0x82_130` —
                // at the ATTACK value 96, because the head has already
                // moved the action to 170. The GENERIC walker is the
                // OPPOSITE shape (`sub_1BF90` :9129,
                // `if (v2 < 1) { if (!v2) …`) and the port had copied
                // that one here; m21's ATTACK twin (:16853, the `2 =>`
                // arm of the attack handler) was already right.
                // ⚠ The transition must run BEFORE `m21_jump`, as it
                // does inside retail's head: `m21_mode(2)` zeroes f68
                // (`byte_0x43_67`), which turns the state-9 rest draw
                // from two draws into one.
                // mc2l22 pair 3751->3752 slot 301: retail steps 60 at
                // yaw 1090 (dx -13, dy +59), advances jump 6->7, lands
                // z 2410 and writes speed 96; the port reproduced the
                // import verbatim.
                let head = self.mc2_state_head(i);
                if head == 1 {
                    self.ent[i].f146 = self.ent[i].f40;
                    self.m21_mode(i, 2);
                }
                match head {
                    2 => self.ent[i].tick70 = M21_BASE + 4,
                    _ => {
                        self.mc2_move_core(i);
                        self.m21_jump(i);
                        // Wander (:16781-91): both draws gated on the
                        // can-turn primitive — heading commits only
                        // at landing (or aligned wade ticks).
                        // ⚠ m21's wander is NOT the shared walker
                        // idiom. `sub_26070` (:16787-90) adds the
                        // kick to the LIVE yaw and copies the result
                        // into roll — `yaw += kick; yaw &= 0x7FF;
                        // roll = yaw` — where the generic walker
                        // (:9139, `mc2_wander_turn`) adds it to ROLL
                        // and leaves yaw to the servo. The two agree
                        // only while yaw == roll, which the attack
                        // arm's snap used to guarantee.
                        if self.m21_can_turn(i) {
                            let v = self.mc2_rand(i);
                            let r = self.mc2_rand(i);
                            let sign = 2 * ((v % 0x9D) / 79) as i32 - 1;
                            let step = ((r & 0xFF) + 85) as i32 * sign;
                            let yaw = (self.ent[i].f30 as i32 + step) as u16 & 0x7FF;
                            self.ent[i].f30 = yaw;
                            self.ent[i].f34 = yaw;
                        }
                        if self.ent[i].f63 & 0x3F == 0
                            && self.ent[i].f58 != 0
                            && let Some(t) = self.mc2_wizard_scan(i, ctx, false)
                        {
                            self.ent[i].f146 = t;
                            self.m21_mode(i, 2);
                        }
                    }
                }
            }
            2 => {
                // sub_26220 (:16838).
                match self.mc2_state_head(i) {
                    1 => self.ent[i].f146 = self.ent[i].f40,
                    2 => {
                        self.ent[i].tick70 = M21_BASE + 4;
                        return;
                    }
                    _ => {}
                }
                let slot = self.ent[i].f146;
                let Some((tx, ty, tz)) = self.mc2_target(slot, ctx) else {
                    self.mc2_move_core(i);
                    self.m21_jump(i);
                    self.m21_mode(i, 1);
                    return;
                };
                // Target facing (:16869-85): the OUTER gate is the
                // can-turn primitive; the packmate override runs on
                // the inner 1-in-4 partition.
                // ⭐ THE DEVIL TURNS, IT NEVER SNAPS. `sub_26220`
                // writes `roll_0x20_32` and NOTHING ELSE in this
                // block (:16871 the target bearing, :16880 the
                // packmate override) — the live yaw belongs to
                // `sub_1B8C0`'s commit turn (:16895, run AFTER),
                // which is frozen outright on the jump cycle's
                // pinned frames (`byte[1] & 8` → :8786-90 returns
                // result 4 before any turn). mc2l22 t=50 slot 289
                // (explain t=49→50): retail rolls 1425 → 1414 with
                // yaw HELD at 1425; the snap wrote 1414. Same law
                // as m4's militia (CONFORMANCE-FINDINGS "The
                // militiaman turns, never snaps", mc1l0 t=5051).
                if self.m21_can_turn(i) {
                    let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                    self.ent[i].f34 = Self::angle_between(ex, ey, tx, ty);
                    if self.ent[i].f63 & 3 == 0 {
                        self.mc2_avoid_packmate(i);
                    }
                }
                let mut out_of_range = false;
                if self.ent[i].f63 & 0x1F == 0 {
                    // ⭐ THE BOLT'S REACH TEST IS A TRUE 3-D DISTANCE.
                    // `sub_26220` (EF:16890-91) pushes the TARGET's whole
                    // `axis_3d` — `sub_583F0_distance_3d(&a2x->position_0x4C_76,
                    // &v4x->position_0x4C_76)` — and the shipped EXE says the
                    // same at `NETHERW.EXE` file **0x4AB5A-0x4AB70** (linear
                    // 0x26B5A; MC2 file = VA + 0x24800), directly under the
                    // `testb $0x1f,0x3e(%ebx)` cadence gate at 0x4AB54:
                    //   4ab54: f6 43 3e 1f   testb $0x1f,0x3e(%ebx)   ; f63 & 0x1F
                    //   4ab58: 75 2e         jne   0x4ab88            ; -> LABEL_22
                    //   4ab5a: 8d 46 4c      lea   0x4c(%esi),%eax    ; TARGET pos
                    //   4ab5d: 50            push  %eax
                    //   4ab5e: 8d 43 4c      lea   0x4c(%ebx),%eax    ; devil pos
                    //   4ab61: 50            push  %eax
                    //   4ab6c: e8 7f 20 03 00 call 0x7cbf0            ; sub_583F0
                    //   4ab74: 39 f8         cmp   %edi,%eax          ; vs v_28
                    //   4ab76: 73 0c         jae   0x4ab84            ; out -> v8 = 1
                    // `esi` is `v4x`, the resolved target record, so `0x50` —
                    // its z — is read like x and y. The port had dropped the
                    // resolved z on the floor (`let Some((tx, ty, _))`) and fed
                    // `mc2_dist3` the DEVIL's own z for both ends, which makes
                    // the term a PLANAR distance and can only ever UNDERSTATE
                    // the reach — so the port fires bolts retail does not, and
                    // stays in ATTACK where retail falls back to IDLE.
                    //
                    // WITNESS — mc2l7 pair 23807→23808, slot 236, a (5,21)
                    // devil at (51761, 28536, 4767) whose `word_0x96_150` is
                    // 209, a (3,3) building that has already moved this tick to
                    // (52740, 24093, 3820). Row 96's `v_28` is 4608; the planar
                    // distance is **4549** (in reach) and the true 3-D distance
                    // is **4648** (out of reach). Retail takes `v8 = 1`:
                    // `sub_268F0(a1x, 1)` writes `byte_0x43_67` 0 → 64,
                    // `word_0x96_150` 209 → 0 and `actionIndex` 170 → 169. The
                    // port fired instead — an extra (9,0) bolt in slot 104 off
                    // the free stack — and held `actionIndex` at 170.
                    //
                    // `MGC_NO_MC2_M21_RANGE_3D=1` restores the planar test.
                    let tgt_z = if no_mc2_m21_range_3d() {
                        self.ent[i].z
                    } else {
                        tz
                    };
                    let e = &self.ent[i];
                    if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tgt_z))
                        < BEHAVIOR[e.row156 as usize].v_28 as u32
                    {
                        self.mc2_atk_bolt(i, slot, ctx);
                    } else {
                        out_of_range = true;
                    }
                }
                self.mc2_move_core(i);
                self.m21_jump(i);
                if out_of_range {
                    self.m21_mode(i, 1);
                }
            }
            4 => self.mc2_prekill(i, M21_BASE),
            5 => self.mc2_kill(i),
            6 => {} // sub_26420 MISSING from the decompile — unreachable
            _ => {
                // +7 (:16925) — `sub_26470`. 1D5D0 is a no-op for our
                // StageVar2==0 spawns; the jump cycle keeps the devil
                // alive. This arm serves the devils the HELD SEAM does
                // NOT claim (`mc2_held_tick` takes kinds 1-10 and 15),
                // i.e. StageVar2 0 and 11/12/13/14/16+ — and retail's
                // tail runs for those too (its `default:` BREAKS).
                self.m21_jump(i);
                self.m21_wrapper_tail(i);
            }
        }
    }

    // =========================================================================
    // MODEL 23 — the mana leviathan (ctor sub_4CBF0 EF:34454, states
    // 0xB8-BF; the only ctor-flying creature: z = 0x2000, siphons
    // (10,39) mana spheres, (9,9) heavy bolts at wizards)
    // =========================================================================

    pub(crate) fn mc2_spawn_m23(&mut self, x: u16, y: u16, _z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 23;
            e.tick70 = M23_BASE; // 184
            e.f28 = 1;
            e.f128 = 24;
            e.f130 = 14;
            e.f126 = 24;
            e.max_life = 10000;
            e.f140 = 100;
        }
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            let f = ((d & 0x7FF) as i32 - 1) as u16;
            e.f34 = f;
            e.f30 = f;
            // pitch NOT set — verbatim (:34469 note).
            e.f44 = 0x2000; // the flying altitude target
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 91;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(23);
        self.link(i, x, y, 0x2000);
        self.refill_life(i);
        self.mc2_set_sprite(i, 289);
        self.mc2_shift_rot(i, 384, 384);
        Some(i)
    }

    /// `sub_27FE0`: state + sub-state + timer in one.
    fn m23_mode(&mut self, i: usize, action: u8, sub: u8, timer: i16) {
        self.ent[i].tick70 = action;
        self.ent[i].f71 = sub;
        self.ent[i].f26 = timer;
    }

    /// `sub_28000` (:18384): nearest live (10,39) mana sphere.
    ///
    /// The scanned list (`dword_38523`) carries class-10 models 39,
    /// 40 AND 57 (:40018-63 builds it), and the scan filters
    /// `model == 39` — the (10,57) FOOL'S-MANA sphere is deliberately
    /// NOT siphonable. Since OPEN-6 a NATIVE m57 carries model 57 too
    /// (mc2/effects.rs), so the model test alone is the filter on both
    /// paths.
    ///
    /// ⚠⚠ **RETAIL'S SCAN IS MODEL-ONLY AND WE CANNOT PORT THAT YET —
    /// HELD, ROUND 105 DIG W1-E.** Shipped `NETHERW.EXE` file 0x4C800,
    /// the whole loop body at 0x4C828:
    /// ```text
    ///   4c828  mov  0x40(%ebx),%al        ; model_0x40_64
    ///   4c82b  cmp  $0x27,%al             ; == 39
    ///   4c82d  jne  0x4c84a               ; ...and that is all
    ///   4c83a  call 0x7ccd0               ; EuclideanDistXY_584D0 (SQUARED)
    ///   4c844  jae  0x4c84a               ; UNSIGNED nearer-than
    ///   4c84a  mov  (%ebx),%ebx           ; ebx = ebx->next_0
    /// ```
    /// It never loads +0x3F (class), +0x45 (action) or +0xD (reap) —
    /// so the `tick70 != 62` test below is INVENTED, and the asymmetry
    /// is RETAIL'S OWN: the sibling LOCK re-check `sub_28420`
    /// (file 0x4CC20) spells out four tests on the same (10,39)
    /// sphere. This is the `sub_5F810` balloon law (dig W1-C) on a
    /// second call path. **But the port walks the POOL where retail
    /// walks `dword_38523`, and these guards are this port's only
    /// stand-in for CHAIN MEMBERSHIP.** MEASURED, all four corners on
    /// mc2l22: dropping the class test alone is neutral (31 segments /
    /// 4 excess resets), dropping the reap test alone is neutral,
    /// dropping BOTH is **77 segments / 50 excess resets** — a slot
    /// that is class-cleared AND reaped keeps a stale `model65 == 39`
    /// retail's chain no longer carries. And the safest single drop
    /// (class + action, keeping reap) still **REGRESSES mc2l24 by one
    /// excess reset** (7537 → 7538 segments). ⇒ **THE FIX IS THE LIST,
    /// NOT THE GUARDS: port the class-10 live chain first.** Round 68's
    /// banked form — RETAIL ASKS THE CHAIN, THE PORT ASKED THE POOL.
    fn m23_find_node(&self, i: usize) -> Option<u16> {
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let mut best: Option<(usize, i32)> = None;
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if c.class64 == 10 && c.model65 == 39 && c.tick70 != 62 && c.flags & 0x400 == 0 {
                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                if best_d2(&best, d2) {
                    best = Some((j, d2));
                }
            }
        }
        best.map(|(j, _)| j as u16)
    }

    /// `sub_28420` (:18603): the locked node must still be a (10,39).
    ///
    /// ⚠ NOT "the same fool's-mana exclusion as
    /// [`Self::m23_find_node`]" — that was this comment's old claim
    /// and retail contradicts it (dig W1-E, round 105). Shipped
    /// `NETHERW.EXE` file 0x4CC20 rejects on FOUR tests and no others:
    /// ```text
    ///   4cc43  cmpl $0x0,0x8(%eax) / jl   ; life_0x8 < 0
    ///   4cc49  testb $0x4,0xd(%eax) / jne ; byte[1] & 4  (reap)
    ///   4cc4f  cmpb $0xa,0x3f(%eax) / jne ; class != 10
    ///   4cc55  cmpb $0x27,0x40(%eax)      ; model != 39
    /// ```
    /// The port had class/model/reap, **omitted the LIFE test** and
    /// **added an action test retail does not make**. Zero rows on
    /// this corpus (mc2l22 31/4 and mc2l24 7537/7533 unchanged, all
    /// 440 fixtures green) — a correctness-only law.
    /// Kill switch `MGC_NO_MC2_M23_LOCK_TESTS`.
    fn m23_node_ok(&self, i: usize) -> bool {
        let t = self.ent[i].f146 as usize;
        if t == 0 || t >= self.ent.len() {
            return false;
        }
        if no_mc2_m23_lock_tests() {
            return self.ent[t].class64 == 10
                && self.ent[t].model65 == 39
                && self.ent[t].tick70 != 62
                && self.ent[t].flags & 0x400 == 0;
        }
        self.ent[t].act_life >= 0
            && self.ent[t].flags & 0x400 == 0
            && self.ent[t].class64 == 10
            && self.ent[t].model65 == 39
    }

    /// `sub_28110` (:18446): the dweller's post pass — damage intake,
    /// wizard retaliation, and the castle-owner hunt.
    ///
    /// ⚠ LAW A — THE INTAKE IS AWAKE-GATED. :18465 `if
    /// (a1x->byte_0x39_57)` wraps the WHOLE mailbox block (debit :18468,
    /// source latch :18469, clear :18471, the class-3 retaliation
    /// :18472-86 AND the quiet-tick `word_0x26_38 = 0` :18490). This is
    /// the one thing m23's post differs in from the generic head
    /// (:8964, no gate). An ASLEEP dweller (f58 == 0: more than 24
    /// tiles from the human, `sub_68C70` EF:55494-535) reads NO mail:
    /// a (10,23) blast's 200/tick sits in `mail0` unconsumed and life
    /// holds at 10000. mc2l22 slot 1 t=20136: retail `b39` 0, `mail0`
    /// (200, 530), life 10000 / action 185 for the next 2,500 ticks;
    /// the port debited 200 a tick and retaliated into 186 (slot 221
    /// from t=21951 is the same head).
    ///
    /// ⚠ LAW B — THE CASTLE-OWNER HUNT (:18500-21) IS NOT awake-gated
    /// and runs from EVERY m23 state (all four handlers end in
    /// `sub_28110`): on the `byte_0x3E_62 & 0x1F` cadence, when no mail
    /// landed (`!v1`), `sub_282D0` picks the nearest undocked wizard /
    /// loaded balloon and, inside `word_160_0x1c_28` (row 91: 3072) in
    /// 3-D (`sub_583F0`), drops a held node and turns on it (186/0/0).
    /// mc2l22 slot 214 t=1912→1913: 184 → 186, `target96` 499 (its
    /// node) → 530 (rival wizard at 3-D 2947), no mail, f63 192.
    /// Returns what this pass left on the `v34` dwords: 0 = it made
    /// no call at all, 1 = it ended on the 4-argument mode setter
    /// (`$0` on W-52, the record pointer on W-64), 2 = it ended on a
    /// pointer-class call. See [`Gen::m27_v34_leviathan_law`].
    fn m23_post(&mut self, i: usize, ctx: &MobCtx) -> u8 {
        let mut left = 0u8;
        let mut v1 = 0u8;
        if self.ent[i].f58 != 0 {
            // :18465 — the awake gate.
            if self.ent[i].mail[0].1 != 0 {
                let (amt, src) = self.ent[i].mail[0];
                self.ent[i].act_life -= amt as i32; // :18468
                self.ent[i].f40 = src; // :18469
                self.ent[i].mail[0].1 = 0; // :18471 (amount kept)
                // :18472 — the listing's `v3 = 0` is a hand-conversion
                // stub; the operand is the mail SOURCE's class == 3.
                if self.mc2_is_wizard(src) {
                    v1 = 1;
                    left = 1;
                    self.m23_release_node(i); // :18475-81
                    self.ent[i].f146 = src; // :18485
                    self.m23_mode(i, M23_BASE + 2, 0, 0); // :18486
                }
            } else {
                self.ent[i].f40 = 0; // :18490
            }
        }
        if self.ent[i].act_life < 0 {
            v1 = 2;
            left = 1;
            self.ent[i].f38 = self.ent[i].f40; // :18497
            self.m23_mode(i, M23_BASE + 4, 0, 0); // :18498
        }
        if v1 == 0 && self.ent[i].f63 & 0x1F == 0 {
            // :18500-21 — the castle-owner hunt.
            left = 2;
            if let Some((t, tp)) = self.m23_owner_scan(i, ctx) {
                let range = BEHAVIOR[self.ent[i].row156 as usize].v_28 as u32;
                let e = &self.ent[i];
                if Self::mc2_dist3((e.x, e.y, e.z), tp) < range {
                    left = 1;
                    self.m23_release_node(i); // :18509-16
                    self.ent[i].f146 = t; // :18518
                    self.m23_mode(i, M23_BASE + 2, 0, 0); // :18519
                }
            }
        }
        left
    }

    /// :18475-81 / :18509-16 — before turning on a wizard, drop the
    /// held (10,39) node, but only if it is grabbed BY THIS dweller
    /// (`byte[0] & 0x40 && node.word_0x96_150 == self`).
    fn m23_release_node(&mut self, i: usize) {
        if !self.m23_node_ok(i) {
            return;
        }
        let t = self.ent[i].f146 as usize;
        if self.ent[t].flags & 0x40 != 0 && self.ent[t].f146 == i as u16 {
            self.ent[t].f146 = 0;
            self.ent[t].flags &= !0x40;
        }
    }

    /// `sub_282D0` (:18530): nearest-by-XY (`EuclideanDistXY_584D0`,
    /// :18556) class-3 record that is a WIZARD (model 0/1, :18537) or a
    /// LOADED BALLOON (model 3 with mana > 0 → its owner's player block,
    /// :18541-48) and is NOT parked over its owner's castle
    /// (`!CompareAxisWithShift_10750(ix, castle)`, XY summed-extents box,
    /// :18554). Castles (model 2) are skipped (:18541). A castle-less
    /// owner indexes retail's scratch record and overlaps nothing, so it
    /// stays a candidate. The list (`dword_38519`, EF:39975) carries only
    /// `life >= 0` class-3 records; the human is a pool record in retail
    /// and rides the ctx here (`MC2_PLAYER_HW` half-extents, the same box
    /// as the rival's own at-castle probe).
    fn m23_owner_scan(&self, i: usize, ctx: &MobCtx) -> Option<(u16, (u16, u16, i16))> {
        let (ex, ey) = (self.ent[i].x, self.ent[i].y);
        let mut best: Option<(u16, (u16, u16, i16), i32)> = None;
        let mut consider = |slot: u16, pos: (u16, u16, i16)| {
            let d = Self::dist2_sq(ex, ey, pos.0, pos.1);
            if best.is_none_or(|(_, _, bd)| d < bd) {
                best = Some((slot, pos, d));
            }
        };
        if !ctx.pdead {
            let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
            let parked = self.mc2_castle_of(PLAYER_TARGET).is_some_and(|c| {
                let h = &self.ent[c];
                wd(h.x, ctx.px) < h.f80 as i32 + crate::mc1::combat::MC2_PLAYER_HW
                    && wd(h.y, ctx.py) < h.f82 as i32 + crate::mc1::combat::MC2_PLAYER_HW
            });
            if !parked {
                consider(PLAYER_TARGET, (ctx.px, ctx.py, ctx.pz));
            }
        }
        for (j, c) in self.ent.iter().enumerate().skip(1) {
            if c.class64 != 3 || c.act_life < 0 || c.flags & 0x400 != 0 {
                continue;
            }
            let owner = match c.model65 {
                0 | 1 => j as u16,
                3 if c.f140 > 0 => c.id24,
                _ => continue,
            };
            if self
                .mc2_castle_of(owner)
                .is_some_and(|h| self.mc2_overlap_xy(j, h))
            {
                continue;
            }
            consider(j as u16, (c.x, c.y, c.z));
        }
        best.map(|(s, p, _)| (s, p))
    }

    /// `sub_28390` (:18580) — the landing servo, and the gate that
    /// starts the siphon. Two independent axes, each with its own
    /// tolerance, and "settled" means BOTH are in band:
    ///   - 2-D reach 128 (`EuclideanDistXYZ_58490` is XY-only,
    ///     utilities/Maths.cpp:738): outside it, turn to the node and
    ///     walk (`sub_1B8C0`); inside it, hold — retail does NOT run
    ///     the mover once aligned.
    ///   - station-keeping 640 ABOVE the node within ±64, stepping
    ///     32/tick.
    ///
    /// Corpus (mc2l24, 14 siphon entries between t=14512 and t=15648):
    /// every dweller enters the siphon with `dz` in [588, 701] and
    /// 2-D gap ≤ 121 — the 640±64 band and the 128 reach exactly.
    fn m23_station_keep(&mut self, i: usize, t: usize) -> bool {
        let mut settled = true;
        let (sp, tp) = {
            let e = &self.ent[i];
            let s = &self.ent[t];
            ((e.x, e.y), (s.x, s.y, s.z))
        };
        if Self::isqrt(Self::dist2_sq(sp.0, sp.1, tp.0, tp.1) as u32) as i32 > 128 {
            settled = false;
            self.ent[i].f34 = Self::angle_between(sp.0, sp.1, tp.0, tp.1);
            self.mc2_move_core(i);
        }
        // Read z AFTER the move commit — retail's servo reads the
        // post-`sub_1B8C0` position.
        let gap = self.ent[i].z as i32 - (tp.2 as i32 + 640);
        if gap.abs() > 64 {
            settled = false;
            let step = if gap <= 0 { 32 } else { -32 };
            self.ent[i].z = self.ent[i].z.wrapping_add(step);
        }
        settled
    }

    /// `sub_28060` (:18415): a descending dweller stacked on a
    /// packmate LIFTS 16 and aborts the approach — only the HIGHER of
    /// the pair moves, and the box is 2·pitch in x/y, 2·fov in z (its
    /// own extents both times). Retail walks the live per-model
    /// bucket, same gates as [`Gen::mc2_avoid_packmate`].
    fn m23_lift_off_packmate(&mut self, i: usize) -> bool {
        let (ex, ey, ez, span, zspan, model, id) = {
            let e = &self.ent[i];
            (
                e.x,
                e.y,
                e.z,
                2 * e.f80 as i32,
                2 * e.f84 as i32,
                e.model65,
                e.id24,
            )
        };
        // ⭐⭐ `bytearray_38403x[a1x->model]` again (EF:18424) — the
        // body is `id != self`, three boxes and `z >= ix->z`, with no
        // liveness question of its own ([`Gen::mc2_roster`]).
        if crate::engine::features::no_mc2_mob_chain_predicate() {
            for c in self.ent.iter().skip(1) {
                if c.class64 == 5
                    && c.model65 == model
                    && c.id24 != id
                    && c.act_life >= 0
                    && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                    && c.flags & 0x400 == 0
                    && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < span
                    && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < span
                    && (ez as i32 - c.z as i32).abs() < zspan
                    && ez >= c.z
                {
                    self.ent[i].z = ez.wrapping_add(16);
                    return true;
                }
            }
        } else {
            for k in 0..self.mc2_roster(model).len() {
                let c = &self.ent[self.mc2_roster(model)[k] as usize];
                if c.id24 != id
                    && ((ex.wrapping_sub(c.x)) as i16 as i32).abs() < span
                    && ((ey.wrapping_sub(c.y)) as i16 as i32).abs() < span
                    && (ez as i32 - c.z as i32).abs() < zspan
                    && ez >= c.z
                {
                    self.ent[i].z = ez.wrapping_add(16);
                    return true;
                }
            }
        }
        false
    }

    /// The altitude-keeping z step of `sub_27950` (:18052).
    fn m23_altitude(&mut self, i: usize) {
        let v2 = self.ent[i].z as i32 - self.ent[i].f44 as i32;
        if v2.abs() >= 256 {
            self.ent[i].z = self.ent[i].z.wrapping_add(if v2 <= 0 { 32 } else { -32 });
        }
    }

    pub(crate) fn m23_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M23_BASE {
            0 => {
                // sub_27950 — the patrol/hunt loop.
                // ⭐ The `v34` seam: `+0x1C` as the move core is
                // entered and its result code decide the 0xD9 dword,
                // `+0x20` the 0xDA one — see
                // [`Gen::m27_v34_leviathan_law`].
                let yaw0 = self.ent[i].f30;
                let code = self.mc2_move_core(i);
                let mut deep = false;
                self.mc2_avoid_packmate(i);
                self.m23_altitude(i);
                // Retail's `byte_0x46_70` switch is `0 / 1 / 2 /
                // default`, and the DEFAULT arm (3 and up) jumps
                // straight to the post pass with no call at all — the
                // `_` arm below stands in for case 2 only.
                let arm = self.ent[i].f71;
                match self.ent[i].f71 {
                    0 => {
                        self.ent[i].f126 = self.ent[i].f130;
                        // PRE-decrement test (EF:18100-02: `if (v5y)
                        // return` on the OLD value; a post-test fires
                        // one tick early).
                        let old = self.ent[i].f26;
                        self.ent[i].f26 = old - 1;
                        if old <= 0 {
                            self.m23_mode(i, M23_BASE, 1, 0);
                        }
                    }
                    1 => {
                        // sub_28000 / the getTerrainAlt fallback: both
                        // arms reach the v34 frames.
                        deep = true;
                        if let Some(n) = self.m23_find_node(i) {
                            self.ent[i].f44 = 0x2000;
                            self.ent[i].f146 = n;
                            self.m23_mode(i, M23_BASE, 2, 0);
                        } else {
                            let (x, y) = (self.ent[i].x, self.ent[i].y);
                            let hover = (self.ground_z(x, y) as i16).wrapping_add(0x700);
                            self.ent[i].f44 = hover as u16;
                            self.m23_mode(i, M23_BASE, 0, 80);
                        }
                    }
                    _ => {
                        if self.m23_node_ok(i) {
                            // :18140 — the re-aim AND the range test
                            // ride the 4-tick cadence byte; testing
                            // every tick hands the descend over up to
                            // 3 ticks early (mc2l24 slot 230, the
                            // residual `action 184 vs 185` rows).
                            if self.ent[i].f63 & 3 == 0 {
                                // sub_58490's isqrt parks &position on
                                // W-64 and its own return on W-52.
                                deep = arm == 2;
                                let t = self.ent[i].f146 as usize;
                                let (sp, tp) = {
                                    let e = &self.ent[i];
                                    let s = &self.ent[t];
                                    ((e.x, e.y, e.z), (s.x, s.y, s.z))
                                };
                                self.ent[i].f34 = Self::angle_between(sp.0, sp.1, tp.0, tp.1);
                                // 2-D (EF:18144 — `EuclideanDistXYZ`
                                // never reads z): the leviathan flies
                                // far above its node, so a 3-D read
                                // would stall the descend transition.
                                if crate::mc2::morph::dist2d(sp.0, sp.1, tp.0 as i32, tp.1 as i32)
                                    < 768
                                {
                                    self.m23_mode(i, M23_BASE + 1, 0, 500);
                                }
                            }
                        } else {
                            self.m23_mode(i, M23_BASE, 1, 0);
                        }
                    }
                }
                let post = self.m23_post(i, ctx);
                self.m27_v34_publish_leviathan(i, yaw0, code, deep, post);
            }
            1 => {
                // sub_27B20 (:18250) — descend/land onto the node.
                match self.ent[i].f71 {
                    0 => {
                        self.ent[i].f126 = self.ent[i].f128;
                        // PRE-decrement: retail stores `--v2` and
                        // tests the NEW value (:18186-88).
                        self.ent[i].f26 = self.ent[i].f26.wrapping_sub(1);
                        // The abort trio is evaluated BEFORE the
                        // approach servo and short-circuits in this
                        // order: timer, node still a (10,39), and the
                        // anti-stack lift.
                        let approach = self.ent[i].f26 != 0
                            && self.m23_node_ok(i)
                            && !self.m23_lift_off_packmate(i);
                        if approach {
                            let t = self.ent[i].f146 as usize;
                            if self.m23_station_keep(i, t) {
                                self.m23_mode(i, M23_BASE + 3, 0, 0);
                            }
                        } else {
                            self.m23_mode(i, M23_BASE + 1, 1, 0);
                        }
                    }
                    1 => {
                        if self.ent[i].z >= 0x2000 {
                            // No f44 write (EF:18174-84 leaves the
                            // stale value — it governs the NEXT
                            // descent's target).
                            self.m23_mode(i, M23_BASE, 0, 80);
                        } else {
                            self.ent[i].z = self.ent[i].z.wrapping_add(32);
                        }
                    }
                    // :18173 acts on sub 1 alone; anything higher is
                    // a bare post pass.
                    _ => {}
                }
                self.m23_post(i, ctx);
            }
            2 => {
                // sub_27E00 — the (9,9) ranged retaliation.
                self.snd(59, i);
                self.ent[i].f126 = self.ent[i].f128;
                self.mc2_move_core(i);
                let slot = self.ent[i].f146;
                let mut broke = self.mc2_target(slot, ctx).is_none();
                if !broke {
                    let (tx, ty, tz) = self.mc2_target(slot, ctx).unwrap();
                    if self.ent[i].f63 & 3 == 0 {
                        self.mc2_aim_avoid(i, tx, ty);
                    }
                    let row = &BEHAVIOR[self.ent[i].row156 as usize];
                    if self.ent[i].f63 as i16 & row.v_26 == 0 {
                        let e = &self.ent[i];
                        if Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz)) < row.v_28 as u32 {
                            self.mc2_atk_heavy9(i, slot, ctx);
                        } else {
                            broke = true;
                        }
                    }
                }
                self.m23_post(i, ctx);
                if broke {
                    self.ent[i].f146 = 0;
                    self.m23_mode(i, M23_BASE + 3, 3, 0);
                }
            }
            3 => {
                // sub_27C10 — the siphon. Retail's control flow is a
                // FALL-THROUGH, not a switch: sub 0 seeds the rise
                // step and the 64-tick timer and then runs the siphon
                // body in that same tick (:18226-40 has no return —
                // only sub >= 2 jumps past the body to LABEL_24). So
                // the grab, the +10 ramp and the swallow test all
                // start on the ARRIVAL tick, and an arrival onto a
                // ball another dweller already holds still steals the
                // grab on its way out (v9 is set, the body runs).
                self.snd(59, i);
                let mut abort = false; // v9  → re-hunt   (base+3 sub 3)
                let mut lost = false; // v10 → climb-out (base+1 sub 1)
                let mut body = true;
                match self.ent[i].f71 {
                    0 => {
                        let free = self.m23_node_ok(i) && {
                            let t = self.ent[i].f146 as usize;
                            self.ent[t].flags & 0x40 == 0
                        };
                        if free {
                            // `word_0x2C_44 = 18` (:18238) — the rise
                            // step the GRABBED BALL reads off its
                            // collector every tick (mc1/combat.rs
                            // `ball_tick`, EF:26120), ramped +10 per
                            // siphon tick below.
                            self.ent[i].f44 = 18;
                            self.m23_mode(i, M23_BASE + 3, 1, 64);
                        } else {
                            abort = true;
                        }
                    }
                    1 => {}
                    _ => {
                        // :18242-59 — sub 2 is a bare no-op; only sub
                        // 3 re-hunts. Both skip the siphon body.
                        body = false;
                        if self.ent[i].f71 == 3 {
                            self.ent[i].f146 = 0;
                            if let Some(n) = self.m23_find_node(i) {
                                let t = n as usize;
                                // :18249 assigns the target INSIDE the
                                // condition, BEFORE the range test —
                                // an out-of-reach node still latches
                                // (it is what the next descend reads).
                                self.ent[i].f146 = n;
                                let (sp, tp) = {
                                    let e = &self.ent[i];
                                    let s = &self.ent[t];
                                    ((e.x, e.y, e.z), (s.x, s.y, s.z))
                                };
                                // 2-D (EF:18250 — `EuclideanDistXYZ`
                                // never reads z).
                                if crate::mc2::morph::dist2d(sp.0, sp.1, tp.0 as i32, tp.1 as i32)
                                    <= 3584
                                {
                                    self.m23_mode(i, M23_BASE + 1, 0, 500);
                                } else {
                                    lost = true;
                                }
                            } else {
                                lost = true;
                            }
                        }
                    }
                }
                if body {
                    // :18261-86. The 64-tick f26 timeout decrements
                    // INSIDE the node-ok arm (retail's `v3x &&
                    // (--f26)` short-circuit) — an unreachable ball
                    // aborts to re-hunt instead of siphoning forever.
                    if self.ent[i].f146 != 0 {
                        let held = self.m23_node_ok(i) && {
                            self.ent[i].f26 = self.ent[i].f26.wrapping_sub(1);
                            self.ent[i].f26 != 0
                        };
                        if held {
                            let t = self.ent[i].f146 as usize;
                            self.ent[t].flags |= 0x40; // grabbed
                            self.ent[t].f146 = i as u16;
                            self.ent[i].f44 = self.ent[i].f44.wrapping_add(10);
                            // :18271 is the 3-axis extent overlap
                            // `sub_106C0` (NOT a radius) — with the
                            // leviathan's 384 half-extents the ball is
                            // swallowed well before it reaches the
                            // body, and the `ball.z > self.z` half
                            // catches the ball that overshoots.
                            if self.ent_overlap(i, t) || self.ent[t].z > self.ent[i].z {
                                // Swallow: steal the mana, consume it.
                                self.ent[i].f140 += self.ent[t].f140;
                                self.ent[t].flags |= 0x400;
                                abort = true;
                            }
                        } else {
                            abort = true;
                        }
                    } else {
                        lost = true;
                    }
                    if abort {
                        self.m23_mode(i, M23_BASE + 3, 3, 0);
                    }
                }
                if lost {
                    self.m23_mode(i, M23_BASE + 1, 1, 0);
                }
                self.m23_post(i, ctx);
            }
            4 => self.mc2_prekill(i, M23_BASE),
            5 => self.mc2_kill(i),
            6 => {} // sub_28460 MISSING — unreachable
            _ => {}
        }
    }

    // =========================================================================
    // MODEL 24 — cave brute (ctor sub_4CCF0 EF:34487; CAVE-ONLY —
    // aggros the class-3 building list via the shared idle scan,
    // not the player; melee 1500 @ 1536; snd 7 on chase)
    // =========================================================================

    pub(crate) fn mc2_spawn_m24(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if !self.is_cave() {
            return None; // `if MapType != Cave return 0` (:34490)
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 24;
            e.tick70 = M24_BASE + 1; // 193 idle (:34495)
            e.f28 = 1;
            e.f71 = 0;
            e.f128 = 80;
            e.f130 = 24;
            e.max_life = 16000;
            e.f126 = 24; // actSpeed = maxSpeed (:34502)
        }
        self.mc2_set_mana_half(i);
        self.ent[i].f36 = 0;
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f44 = 1500; // melee damage (sub_1CF20 @ 1536)
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 102;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(24);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 335);
        self.mc2_shift_rot(i, 256, 640);
        Some(i)
    }

    /// `sub_28690` (:18723): the shared m24 target acquisition.
    fn m24_acquire(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].f58 == 0 || self.ent[i].f63 & 0xF != 0 {
            return;
        }
        // sub_28690 (:18744-71) walks the WHOLE class-3 list — the
        // brute aggros castles and balloons too, nearest-wins, not
        // just wizards. Winner validity: alive + not reaped
        // (the byte[1]&4 check = our 0x400, already in the scan).
        if let Some(t) = self.mc2_class3_scan(i, ctx) {
            self.ent[i].tick70 = M24_BASE + 2;
            self.ent[i].f146 = t;
        }
    }

    /// `sub_287B0` (EF:18779-18825) — m24's pose. Retail picks a speed
    /// and a sprite in one `actionIndex` ladder and then commits the
    /// sprite through ONE guarded tail:
    /// ```text
    ///   0xC0        -> v3 = 336, actSpeed = 0
    ///   0xC2        -> v4 = minSpeed_0x84_132,     v3 = 335
    ///   0xC6        -> v4 = 2 * maxSpeed_0x86_134, v3 = 335
    ///   otherwise   -> v4 = maxSpeed_0x86_134,     v3 = 335
    ///   if (v3 != a1x->word_0x5A_90) { word_0x5A_90 = v3;
    ///       animationFrame_0x5C_92 = 0;
    ///       byte_0x5D_93 = D8A2E[particlesParameters_D951C[v3].byte_12]; }
    /// ```
    /// ⭐⭐⭐ **THAT TAIL IS `SetEntityIndex_49C90` INLINED, NOT
    /// `SetEntityIndexAndRot_49CD0`** — the `array_0x52_82` extents
    /// quad is NEVER touched by this pose. The port called
    /// [`Gen::mc2_set_sprite`] (the `AndRot` twin), so every time the
    /// creature changed animation its collision box was re-derived
    /// from the new sprite's particle-param row.
    /// ⭐⭐ And the guard is on BOTH sprites and the write is on ALL
    /// FOUR arms: the port guarded only the 335 arm, re-stamped 336
    /// every single tick (resetting `animationFrame` with it), and
    /// never committed 335 at all from actions 194/198.
    ///
    /// WITNESS — mc2l15 pair 19531→19532, slot 32, a (5,24) idling to
    /// `action45 193 → 192`: retail's changelog moves `f5a 335 → 336`
    /// and `speed 24 → 0` and **leaves the quad alone**, while the
    /// port re-derived it to 152/152/225 against retail's held
    /// 256/256/640 (`apitch`/`aroll`/`afov`). It was the head of a
    /// 7,618-tick reset run.
    ///
    /// `MGC_NO_MC2_M24_POSE_PLAIN_SPRITE=1` restores the old behaviour.
    pub(crate) fn m24_pose(&mut self, i: usize) {
        if no_mc2_m24_pose_plain_sprite() {
            match self.ent[i].tick70 - M24_BASE {
                0 => {
                    self.mc2_set_sprite(i, 336);
                    self.ent[i].f126 = 0;
                }
                2 => self.ent[i].f126 = self.ent[i].f128,
                6 => self.ent[i].f126 = 2 * self.ent[i].f130,
                _ => {
                    self.ent[i].f126 = self.ent[i].f130;
                    if self.ent[i].type86 != 335 {
                        self.mc2_set_sprite(i, 335);
                    }
                }
            }
            return;
        }
        let (speed, sprite) = match self.ent[i].tick70 - M24_BASE {
            0 => (0, 336),
            2 => (self.ent[i].f128, 335),     // minSpeed_0x84_132
            6 => (2 * self.ent[i].f130, 335), // 2 * maxSpeed_0x86_134
            _ => (self.ent[i].f130, 335),     // maxSpeed_0x86_134
        };
        self.ent[i].f126 = speed;
        if self.ent[i].type86 != sprite {
            self.mc2_set_sprite_index(i, sprite);
        }
    }

    pub(crate) fn m24_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M24_BASE {
            0 => {
                self.mc2_patrol(i, M24_BASE);
                if self.ent[i].tick70 == M24_BASE {
                    if self.ent[i].f63 & 7 == 0 {
                        let d = self.mc2_rand(i);
                        if d % 3 == 0 {
                            self.ent[i].tick70 = M24_BASE + 1;
                        }
                    }
                    if self.ent[i].tick70 == M24_BASE {
                        self.m24_acquire(i, ctx);
                    }
                } else {
                    self.ent[i].tick70 = M24_BASE + 6;
                }
                self.m24_pose(i);
            }
            1 => {
                self.mc2_idle(i, M24_BASE, ctx);
                if self.ent[i].tick70 == M24_BASE + 1 {
                    if self.ent[i].f63 & 7 == 0 {
                        let d = self.mc2_rand(i);
                        if d % 3 == 0 {
                            self.ent[i].tick70 = M24_BASE;
                        }
                    }
                    if self.ent[i].tick70 == M24_BASE + 1 {
                        self.m24_acquire(i, ctx);
                    }
                } else if no_mc2_m24_idle_else_forces_charge() && self.ent[i].tick70 == M24_BASE + 2
                {
                    // The pre-dig arm — see below; kept only under the
                    // kill switch.
                } else {
                    // ⭐⭐⭐ **RETAIL'S `else` IS UNCONDITIONAL, AND
                    // THE PORT'S EXTRA ARM WAS INVENTED.** `sub_28500`
                    // (EF:18659-72), m24's action-193 handler, is
                    // ```text
                    //   sub_1BF90(a1x, 192);
                    //   if (a1x->actionIndex_0x45_69 == 193) {
                    //       … the 1-in-3 flip to 192 …
                    //       if (actionIndex == 193) sub_28690(a1x);
                    //   } else {
                    //       a1x->actionIndex_0x45_69 = 198;
                    //   }
                    // ```
                    // so ANY action the IDLE PRIMITIVE itself moved to
                    // — 194 included — is overwritten with 198, the
                    // charge. Only the acquire the handler runs ITSELF
                    // (`sub_28690`, inside the taken branch) survives
                    // at 194. Its action-192 twin `sub_28490`
                    // (EF:18637-52) has the identical shape, and the
                    // port's `0 =>` arm already matched it — this arm
                    // alone carried an extra `else if action == 194 {}`
                    // that preserved the primitive's promotion.
                    //
                    // WITNESS — mc2l15 pair 19531→19532, slot 46, a
                    // (5,24) whose idle primitive acquires the human:
                    // retail `action45 193 → 198`, `target96 0 → 165`,
                    // `speed 24 → 48` (= 2 × maxSpeed, the 0xC6 pose
                    // arm). The port kept `action 194` and
                    // `speed 80` (= minSpeed, the 0xC2 arm).
                    //
                    // `MGC_NO_MC2_M24_IDLE_ELSE_FORCES_CHARGE=1`
                    // restores the old behaviour.
                    self.ent[i].tick70 = M24_BASE + 6;
                }
                self.m24_pose(i);
            }
            2 => {
                self.snd(7, i);
                if self.mc2_chase_attack(i, M24_BASE, ctx, Self::mc2_atk_melee_1536) {
                    self.ent[i].tick70 = M24_BASE + 6;
                }
                self.m24_pose(i);
            }
            3 => {
                self.ent[i].tick70 = M24_BASE + 1;
                self.mc2_patrol(i, M24_BASE);
                self.m24_pose(i);
            }
            4 => self.mc2_prekill(i, M24_BASE),
            5 => self.mc2_kill(i),
            6 => {
                self.mc2_flee(i, M24_BASE, ctx);
                self.m24_acquire(i, ctx);
                self.m24_pose(i);
            }
            _ => self.m24_pose(i),
        }
    }

    // =========================================================================
    // MODEL 25 — the swarm splitter (ctor sub_4CE00 EF:34523, states
    // 0xC8-CF; castle-drain minis, splits into 3 on death,
    // trace mc2-class5-m25-26-28-class2-treeburn.md)
    // =========================================================================

    pub(crate) fn mc2_spawn_m25(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 25;
            e.tick70 = M25_BASE + 1; // 201
            e.f28 = 1;
            e.f71 = 0;
            e.f128 = 60;
            e.f130 = 20;
            e.max_life = 7500;
            e.f126 = 60;
        }
        self.mc2_set_mana_half(i);
        {
            self.ent[i].f36 = 0;
        }
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f44 = 300; // damage AND the brain's lifetime countdown
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 92;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(25);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 290);
        self.mc2_shift_rot(i, 384, 384);
        Some(i)
    }

    /// The castle of a wizard slot (class 3 model 2 keyed on id24;
    /// the human's is id24 == PLAYER_TARGET).
    pub(crate) fn mc2_castle_of(&self, wiz: u16) -> Option<usize> {
        let want = if wiz == PLAYER_TARGET {
            PLAYER_TARGET
        } else if (wiz as usize) < self.ent.len() {
            self.ent[wiz as usize].id24
        } else {
            return None;
        };
        self.ent
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| {
                c.class64 == 3 && c.model65 == 2 && c.id24 == want && c.flags & 0x400 == 0
            })
            .map(|(j, _)| j)
    }

    /// ⭐⭐⭐ THE M25 BRAIN'S CASTLE LOOKUP IS A **REGISTER READ**, NOT
    /// A POOL SCAN. `sub_28860`'s three castle questions all
    /// dereference the WIZARD's extension block and read its
    /// `CastleEntityIndex_0x3A_58` word:
    ///   case 3 (EF:18951) — shipped NETHERW.EXE `0x4d1b6`
    ///       `mov 0xa4(%eax),%eax` / `cmpw $0x0,0x3a(%eax)`;
    ///   case 5 (EF:18975) — `0x4d23f-0x4d245`
    ///       `mov 0xa4(%eax),%eax` / `mov 0x3a(%eax),%di`;
    ///   case 7 (EF:18988) — `0x4d2ae-0x4d2b4`
    ///       `mov 0xa4(%eax),%eax` / `mov 0x3a(%eax),%dx`,
    /// and cases 5/7 then use that word DIRECTLY as the pool index
    /// (`mov 0x1a3e4(,%eax,4),%esi`) with no class, model, owner or
    /// reap test of any kind.
    ///
    /// [`Gen::mc2_castle_of`] is a POOL SCAN, and round 136's law
    /// applies verbatim: A POOL SCAN RETURNS THE LOWEST-NUMBERED
    /// MATCH, A REGISTER THE CHOSEN ONE — ONLY A CASTLE SPLIT MAKES
    /// THEM DIFFER. mc2l12 has a split: orphan castle 678 died at
    /// ~t=19714 and the UNCONDITIONAL teardown clear took the
    /// register for castle 293, which is still standing. From
    /// t=42388 retail's register reads 0 while 293 is alive, so
    /// retail's `case 3` takes its ELSE arm and BURNS ONE LCG DRAW
    /// (`imul $0x24a1 … add $0x24df`, `0x4d1d4`) that the port's
    /// scan-satisfied `case 5` arm never takes. The entity's wander
    /// pair then reads the wrong two words and the heading walks off
    /// by up to ±380 per re-anchor.
    ///
    /// Witness, mc2l12 pair 42983→42984, slot 868 (5,25):
    /// retail `rand` 42974 → 63707 (THREE draws), port → 22140 (TWO);
    /// retail `scratch10` −1 → 109 = 61309 % 100 + 100, the case-3
    /// draw itself; retail `yaw`/`roll` 268 → 188 = 268 − (63707 %
    /// 381) with sign −1 from 22140 % 157 / 79, port 310 = 268 +
    /// (22140 % 381) with sign +1 from 61309 % 157 / 79.
    ///
    /// `MGC_NO_MC2_M25_CASTLE_REGISTER=1` restores the pool scan.
    pub(crate) fn mc2_castle_reg_of(&self, wiz: u16) -> Option<usize> {
        if crate::mc2::mobs::no_mc2_m25_castle_register() {
            return self.mc2_castle_of(wiz);
        }
        // The wizard whose extension block carries the register.
        // Retail indexes `Entities[word]` and takes `+0xA4`; the
        // port keeps the same word per TEAM in `Gen::castle_reg`,
        // so resolve the wizard record to its team the way
        // `mc2_castle_of` resolves it to its `id24` owner tag.
        let own = if wiz == PLAYER_TARGET {
            PLAYER_TARGET
        } else if (wiz as usize) < self.ent.len() {
            self.ent[wiz as usize].id24
        } else {
            return None;
        };
        let team = self.owner_team(own)?;
        let c = self.castle_reg[team as usize] as usize;
        (c != 0 && c < self.ent.len()).then_some(c)
    }

    pub(crate) fn m25_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M25_BASE {
            0 => self.m25_brain(i, ctx),
            1 => {
                self.mc2_idle(i, M25_BASE, ctx);
                self.ent[i].act_life = self.ent[i].act_life.max(0); // :19062 clamp
            }
            2 => {
                self.snd(37, i);
                let _ = self.mc2_chase_attack(i, M25_BASE, ctx, Self::mc2_atk_bolt);
                self.ent[i].act_life = self.ent[i].act_life.max(0);
            }
            3 => {
                self.ent[i].tick70 = M25_BASE + 1;
                self.ent[i].act_life = self.ent[i].act_life.max(0);
            }
            4 => self.m25_split(i),
            5 => {
                // :19187 — kill/score (the shared 1C890 gate:
                // human killer + self-id exclusion).
                if self.ent[i].f38 == PLAYER_TARGET && self.ent[i].id24 != PLAYER_TARGET {
                    self.kills += 1;
                }
                if self.ent[i].f71 != 0 {
                    self.mc2_kill(i);
                } else {
                    self.ent[i].act_life = -1;
                    self.ent[i].flags |= 0x400;
                }
            }
            6 => {} // sub_28F40 MISSING — unreachable
            _ => {
                // :19205 — respawn hook.
                if self.ent[i].f71 != 0 {
                    self.ent[i].tick70 = M25_BASE;
                    self.ent[i].f71 = 3;
                } // else sub_1D5D0 no-op
            }
        }
    }

    /// `sub_28860` (:18828) — the mini/adult brain: lifetime
    /// countdown, castle hunt, water sprite swap.
    fn m25_brain(&mut self, i: usize, ctx: &MobCtx) {
        let sub = self.ent[i].f71;
        let mut v2 = 0u8;
        if !matches!(sub, 1 | 2) {
            v2 = self.mc2_state_head(i);
        }
        // subSpellIndex-- lifetime (:18896).
        self.ent[i].f44 = self.ent[i].f44.wrapping_sub(1);
        if self.ent[i].f44 == 0 {
            v2 = 2;
        }
        if v2 == 2 {
            self.ent[i].tick70 = M25_BASE + 4;
            return;
        }
        let mut speed_reset = false;
        match self.ent[i].f71 {
            1 | 2 => {
                // ⭐ `case 1:` ARMS THE COUNTER AND FALLS THROUGH INTO
                // `case 2` IN THE SAME TICK — `goto LABEL_20`
                // (EF:18908-13); NETHERW.EXE 0x4d141-0x4d14c has no
                // branch between them. So the birth tick already
                // clears the mail source, steps 52 -> 51 and, since
                // 51 > 13, spins `roll_0x20_32` by +0x100
                // (`HIBYTE(roll) = (HIBYTE(roll) + 1) & 7`, EF:18923-27
                // — the old citation :19926-28 was wrong). `sub_1B8C0`
                // then chases it by row 95's `v_2 = 170`. Stopping at
                // case 1 left roll == yaw, the capped turn 0, and every
                // split mini's heading exactly 170 short: mc2l22 pair
                // 1246->1247, slots 731/755/805 heading retail
                // 1411/1179/1037 vs port 1241/1009/867.
                if self.ent[i].f71 == 1 {
                    self.ent[i].f26 = 52;
                    self.ent[i].f71 = 2;
                }
                self.ent[i].act_life = self.ent[i].max_life as i32;
                self.ent[i].mail[0].1 = 0;
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 < 0 {
                    self.ent[i].f71 = 3;
                    speed_reset = true;
                } else if self.ent[i].f26 > 13 {
                    // The hatch spin (:19926-28).
                    let f34 = self.ent[i].f34;
                    self.ent[i].f34 = (f34 & 0xFF) | ((((f34 >> 8) + 1) & 7) << 8);
                }
            }
            3 => {
                let t = self.ent[i].f38;
                if !self.mc2_is_wizard(t) {
                    self.ent[i].f71 = 8;
                    self.ent[i].f26 = 100;
                } else if self.mc2_castle_reg_of(t).is_some() {
                    self.ent[i].f71 = 5;
                    self.ent[i].f146 = t;
                } else {
                    let d = self.mc2_rand(i);
                    self.ent[i].f71 = 4;
                    self.ent[i].f26 = (d % 100 + 100) as i16;
                }
            }
            4 => {
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 < 0 {
                    self.ent[i].f71 = 3;
                }
            }
            5 => {
                if let Some(c) = self.mc2_castle_reg_of(self.ent[i].f146) {
                    if self.ent[i].f63 & 7 == 0 {
                        let (cx, cy) = (self.ent[c].x, self.ent[c].y);
                        let e = &self.ent[i];
                        self.ent[i].f34 = Self::angle_between(e.x, e.y, cx, cy);
                        // In-range = the box overlap (deliberate: for
                        // CompareAxisWithShift_10750).
                        let e = &self.ent[i];
                        let near = ((e.x.wrapping_sub(cx)) as i16 as i32).abs()
                            < (e.f80 + self.ent[c].f80) as i32
                            && ((e.y.wrapping_sub(cy)) as i16 as i32).abs()
                                < (e.f82 + self.ent[c].f82) as i32;
                        if near {
                            self.ent[i].f71 = 6;
                        }
                    }
                } else {
                    self.ent[i].f71 = 3;
                }
            }
            6 | 7 => {
                if self.ent[i].f71 == 6 {
                    // The 6→7 transition sets v26 unconditionally
                    // (EF:18980-83) before falling into LABEL_41.
                    speed_reset = true;
                }
                self.ent[i].f71 = 7;
                if let Some(c) = self.mc2_castle_reg_of(self.ent[i].f146) {
                    let (cx, cy) = (self.ent[c].x, self.ent[c].y);
                    let e = &self.ent[i];
                    let near = ((e.x.wrapping_sub(cx)) as i16 as i32).abs()
                        < (e.f80 + self.ent[c].f80) as i32
                        && ((e.y.wrapping_sub(cy)) as i16 as i32).abs()
                            < (e.f82 + self.ent[c].f82) as i32;
                    if near {
                        // The castle gnaw: 60 into its inbox (:18992).
                        let src = self.ent[i].id24;
                        self.mc2_melee_write(c as u16, 0x3C, src);
                    } else {
                        self.ent[i].f71 = 5;
                        speed_reset = true;
                    }
                } else {
                    self.ent[i].f71 = 3;
                    speed_reset = true;
                }
            }
            8 => {
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 < 0 {
                    // ⭐⭐⭐ DIG 98-Q25 — `case 8`'s expiry WRITES THE
                    // ACTION AND `break`s (EF:19003-08); it does NOT
                    // leave the handler. The wander draw, `sub_1B8C0`,
                    // the water sprite swap and the speed reset below
                    // all still run this tick. Only the `v2 == 2`
                    // death arm above skips them — a DIFFERENT `if`,
                    // one level out. See
                    // `mc2::effects::no_m25_c8_fallthrough`.
                    self.ent[i].tick70 = M25_BASE + 4;
                    if crate::mc2::effects::no_m25_c8_fallthrough() {
                        return;
                    }
                }
            }
            _ => {}
        }
        if v2 == 1 {
            // Damage retarget: the brain hunts the ATTACKER's castle.
            self.ent[i].f38 = self.ent[i].f40;
        }
        // Wander + move (:19018-25).
        if self.ent[i].f63 & 7 == 0 {
            let d1 = self.mc2_rand(i);
            let d2 = self.mc2_rand(i);
            let sign = 2 * ((d1 % 157) / 79) as i32 - 1;
            let f34 = self.ent[i].f34;
            self.ent[i].f34 = (f34 as i32 + sign * (d2 % 381) as i32) as u16 & 0x7FF;
        }
        self.mc2_move_core(i);
        // Water sprite swap (:19026-47).
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let on_water = self.cap_bit(x, y) == 1;
        if on_water {
            if self.ent[i].type86 == 314 {
                // Already swimming: ABOVE ground swaps back to 313
                // with no minSpeed and no v26; otherwise a total
                // no-op (EF:19029-35).
                if z > self.ground_z(x, y) as i16 {
                    self.mc2_set_sprite(i, 313);
                }
            } else {
                self.mc2_set_sprite(i, 314);
                self.ent[i].f128 = 35;
                speed_reset = true;
            }
        } else if self.ent[i].type86 != 313 {
            self.mc2_set_sprite(i, 313);
            self.ent[i].f128 = 60;
            speed_reset = true;
        }
        if speed_reset {
            self.ent[i].f126 = self.ent[i].f128;
            if self.ent[i].f71 == 2 {
                self.ent[i].f126 = self.ent[i].f128 + 50;
            }
        }
        let _ = ctx;
    }

    /// `sub_28CE0` (:19103) — the death split: 3 minis + the (10,1)
    /// burst.
    fn m25_split(&mut self, i: usize) {
        if self.ent[i].f71 != 0 {
            self.mc2_prekill(i, M25_BASE);
            return;
        }
        let (x, y, z, mana, killer) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f140, e.f40)
        };
        // Pool exhaustion trades the 3 minis for a sphere dump, but
        // FALLS THROUGH to the shared burst + state advance — the
        // (10,1) spawn sits outside the if/else (EF:19176-81); the
        // old early return skipped the burst.
        if self.free.len() <= 1 {
            self.mc2_mana_spheres(i, false);
        } else {
            self.m25_split_minis(x, y, z, mana, killer);
        }
        self.mc2_corpse_burst(i);
        self.ent[i].tick70 = M25_BASE + 5;
        self.ent[i].f71 = 0;
    }

    /// The 3-mini spawn loop of `sub_28CE0` (:19110-70).
    fn m25_split_minis(&mut self, x: u16, y: u16, z: i16, mana: i32, killer: u16) {
        let share = mana / 3;
        for n in 0..3 {
            let Some(c) = self.new_event() else { continue };
            {
                let e = &mut self.ent[c];
                e.class64 = 5;
                e.model65 = 25;
                e.tick70 = M25_BASE;
                e.f28 = 1;
                e.f71 = 1;
                e.f128 = 35;
                e.f130 = 60;
                e.f126 = 85;
                e.f140 = if n == 2 { mana - 2 * share } else { share };
                e.max_life = 80;
            }
            let d = self.mc2_rand(c);
            {
                let e = &mut self.ent[c];
                let f = ((d & 0x7FF) as i32 - 1) as u16;
                e.f34 = f;
                e.f30 = f;
                e.f44 = 15000; // the mini's lifetime seed (:19161)
                // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
                if mc2_class5_w36_legacy() {
                    e.f56 = 1;
                }
                e.row156 = 95;
                e.f58 = 64;
                e.f66 = 3;
                e.f38 = killer;
            }
            self.ent[c].f63 = self.mc2_ord(25);
            self.link(c, x, y, z);
            self.refill_life(c);
            self.mc2_set_sprite(c, 314);
            self.mc2_shift_rot(c, 32, 32);
        }
    }

    // =========================================================================
    // MODEL 26 — the mana leech (ctor sub_4CF00 EF:34557, states
    // 0xD0-D7; drains wizard mana, forces spell discharges)
    // =========================================================================

    pub(crate) fn mc2_spawn_m26(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 26;
            e.tick70 = M26_BASE + 1; // 209
            e.f28 = 1;
            e.f128 = 25;
            e.f130 = 25;
            e.max_life = 4400;
            e.f126 = 25;
        }
        self.mc2_set_mana_half(i);
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 300;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 99;
            e.f58 = 64;
            e.f66 = 3;
        }
        self.ent[i].f63 = self.mc2_ord(26);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 318);
        self.mc2_shift_rot(i, 256, 384);
        // sub_293D0 post-init (:34585) — the wake primitive.
        self.m26_wake(i);
        Some(i)
    }

    /// `sub_293D0` (:19425): outside the attack state, clear the
    /// target and go full-speed (byte[2] bit 7 = flags bit 23).
    /// DUAL-PURPOSE bit: the renderer's per-entity override reads it
    /// as translucency mode 2 (GRO:3779-3805) — retail's wraith is
    /// deliberately 33%-opaque while hunting, solid while draining
    /// (docs/traces/mc2-transparency-drawlist.md §6.2).
    fn m26_wake(&mut self, i: usize) {
        if self.ent[i].tick70 != M26_BASE + 2 {
            self.ent[i].f146 = 0;
            self.ent[i].flags |= 1 << 23;
            self.ent[i].f126 = self.ent[i].f130;
        }
    }

    /// `sub_293B0` (:19411): in the attack state, slow down.
    fn m26_calm(&mut self, i: usize) {
        if self.ent[i].tick70 == M26_BASE + 2 {
            self.ent[i].flags &= !(1 << 23);
            self.ent[i].f126 = self.ent[i].f128;
        }
    }

    pub(crate) fn m26_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M26_BASE {
            0 => {
                self.mc2_patrol(i, M26_BASE);
                self.m26_calm(i);
            }
            1 => {
                self.mc2_idle(i, M26_BASE, ctx);
                self.m26_calm(i);
            }
            2 => {
                // sub_28FF0 (:19233) — the leech.
                if self.ent[i].f63 & 0x1F == 0 {
                    self.snd(62, i);
                }
                match self.mc2_state_head(i) {
                    1 => self.ent[i].f146 = self.ent[i].f40,
                    2 => {
                        self.ent[i].tick70 = M26_BASE + 4;
                        self.m26_wake(i);
                        return;
                    }
                    _ => {}
                }
                self.mc2_move_core(i);
                let slot = self.ent[i].f146;
                if self.mc2_is_wizard(slot) && self.mc2_target(slot, ctx).is_some() {
                    let (tx, ty, tz) = self.mc2_target(slot, ctx).unwrap();
                    if self.ent[i].f63 & 3 == 0 {
                        self.mc2_aim_avoid(i, tx, ty);
                    }
                    // The drain (:19331-34): −(manaRegen + 14).
                    //
                    // ⭐⭐⭐ ONE STATEMENT, OWNER-AGNOSTIC. Retail's
                    // `v6x` is whatever wizard `word_0x96_150` names
                    // (class 3, model 0|1, life >= 0, not reaped —
                    // EF:19310-13); the human's carpet is a pool
                    // record like every other, so THE SAME LINE BILLS
                    // THE HUMAN. The old human arm banked a flat +14
                    // into `mc2_player_drain`, a counter with NO
                    // READER anywhere in the tree — the human was
                    // never billed at all. mc2l22 t=5654→5655,
                    // wraith 490 draining carpet 424: retail
                    // 118326 + 1369 regen − (1369 + 14) = 118312,
                    // the port 118326 + 1369 = 119695.
                    //
                    // ⭐⭐⭐ WHOSE RECORD THE OPERAND COMES OFF. A
                    // rival's purse and its `manaRegen_0x88_136`
                    // live on the WORLD's `mc2_rivals` record
                    // (`mana` / `mana_delta`); the entity's `f140`
                    // is a per-tick MIRROR that `mc2_rival_alive`
                    // overwrites, and its `f136` is the wizard's
                    // maxMana, not the regen. The old write here
                    // debited `maxMana + 14` into a lane the
                    // record clobbered a slot later — the leech
                    // never landed (mc2l22 t=1210: one wraith on
                    // 451, retail 9500 → 9486, the port 9500).
                    // Post the victim; World applies it at THIS
                    // walk slot (`mc2_rival_leech_apply`) — the
                    // human rides the SAME mailbox, as PLAYER_TARGET.
                    if slot == PLAYER_TARGET {
                        self.mc2_player_drain.0 += 14; // legacy tally, no reader
                    }
                    self.mc2_rival_leech.0 = slot as i32;
                    if self.ent[i].f63 & 3 == 0 {
                        let e = &self.ent[i];
                        let v10 = Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz));
                        let row = &BEHAVIOR[e.row156 as usize];
                        if v10 <= row.v_28 as u32 {
                            let target_is_avatar =
                                slot == PLAYER_TARGET || self.ent[slot as usize].model65 == 0;
                            // ALL in-range paths STAY DRAINING: every
                            // `return sub_293D0` is a state no-op at
                            // 210 (EF:19338-76 + 19426-40) — the only
                            // exit to 209 is v10 > v_28 below.
                            if !(v10 >= 2048 || !target_is_avatar) {
                                // The %63 spell-hijack roll
                                // (EF:19346-47, ONE global-LCG draw):
                                // 4 = steal the RIGHT hand, 5 = the
                                // LEFT, all else nothing. The
                                // empty-hand/slot-0/re-steal-lock
                                // aborts run AFTER the draw
                                // (world-side, sub_69300) — the roll
                                // is spent either way. Only the
                                // human's book exists port-side, so
                                // the mail is PLAYER_TARGET-gated
                                // (retail model-0 targets only).
                                self.rand = self.rand.wrapping_mul(9377).wrapping_add(9439);
                                let roll = self.rand % 63;
                                if slot == PLAYER_TARGET && (roll == 4 || roll == 5) {
                                    self.mc2_steal_mail.0.push((i as u16, (roll - 3) as u8));
                                }
                            }
                        } else {
                            self.ent[i].tick70 = M26_BASE + 1;
                        }
                    }
                } else {
                    self.ent[i].tick70 = M26_BASE + 1;
                }
                self.m26_wake(i);
            }
            3 => {
                self.mc2_pack(i, M26_BASE);
                self.m26_calm(i);
            }
            4 => self.mc2_prekill(i, M26_BASE),
            5 => self.mc2_kill(i),
            6 => {} // sub_29370 MISSING — unreachable
            _ => {
                self.m26_calm(i);
            }
        }
    }

    // =========================================================================
    // MODEL 28 — the melee brute (ctor sub_4D1D0 EF:34695, states
    // 0xE0-E7; the fastest creature, 2000-damage swing arcs)
    // =========================================================================

    pub(crate) fn mc2_spawn_m28(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 28;
            e.tick70 = M28_BASE + 1; // 225
            e.f28 = 1;
            e.f128 = 120;
            e.f130 = 64;
            e.max_life = 8000;
        }
        self.mc2_set_mana_half(i);
        // byte[3] |= 8 (:34707) — no ported reader; bit 30 is its
        // home (27 belongs to the blocked-status mapping).
        self.ent[i].flags |= 1 << 30;
        self.mc2_ctor_facing(i);
        {
            let e = &mut self.ent[i];
            e.f36 = 0;
            e.f44 = 2000;
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            e.row156 = 93;
            e.f58 = 64;
            e.f66 = 3;
            e.f126 = e.f130 + (e.f128 - e.f130) / 2; // 92 (:34719)
        }
        self.ent[i].f63 = self.mc2_ord(28);
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 292);
        self.mc2_shift_rot(i, 85, 42);
        Some(i)
    }

    /// `sub_2B860` (:21308): sprite/row config.
    fn m28_pose(&mut self, i: usize, mode: u8) {
        match mode {
            1 => {
                self.ent[i].row156 = 93;
                if !no_m28_pose_flags() {
                    // `byte[0] = (byte[0] & 0xF6) | 8` — file 0x50085.
                    self.ent[i].flags = (self.ent[i].flags & !9) | 8;
                }
                self.mc2_set_sprite(i, 292);
                self.mc2_shift_rot(i, 85, 42);
                self.ent[i].f126 = self.ent[i].f130;
            }
            2 => {
                self.ent[i].row156 = 93;
                if no_m28_strike_frames() {
                    // ⚠ MIS-HOMED: retail's `word_0x2C_44 = 0` here is
                    // NOT `subSpellIndex_0x2A_42` (`f44`), which holds
                    // the 2000-damage swing `sub_1CED0` passes to
                    // `sub_11900` (EF:9793) and which `sub_4D1D0`
                    // stamps once at birth (EF:34751).
                    self.ent[i].f44 = 0;
                }
                if !no_m28_pose_flags() {
                    // `byte[0] &= 0xF6` then `|= 8` — file 0x500df.
                    self.ent[i].flags = (self.ent[i].flags & !9) | 8;
                }
                self.ent[i].f126 = self.ent[i].f128;
                self.mc2_set_sprite(i, 291);
                self.mc2_shift_rot(i, 384, 768);
                // `dword_0x10_16 = anim->CountOfFrames_16`, then
                // `word_0x2C_44 = dword_0x10_16` (EF:21338-39).
                self.ent[i].f26 = if no_m28_strike_frames() {
                    16
                } else {
                    M28_STRIKE_FRAMES
                };
            }
            _ => {
                self.ent[i].f58 = 0;
                self.ent[i].row156 = 94;
                if !no_m28_pose_flags() {
                    // `byte[0] = (byte[0] | 1) & 0xF7` — file 0x5017e.
                    self.ent[i].flags = (self.ent[i].flags | 1) & !8;
                }
                self.ent[i].f126 = self.ent[i].f128 - 28; // 92
            }
        }
    }

    /// `sub_2BA50` (:21416).
    fn m28_sub(&mut self, i: usize, n: u8) {
        self.ent[i].f71 = n;
        self.ent[i].f26 = match n {
            2 => 32,
            8 => 16,
            _ => 0,
        };
    }

    /// `sub_2B7E0` (:21273): only one m28 strikes at a time.
    fn m28_strike_taken(&self, i: usize) -> bool {
        // ⭐⭐ `v1x = bytearray_38403x[112 / 4]` — the MODEL-28 chain
        // head, loaded once and chased through `next_0` (EF:21279-92).
        // The body tests `v1x != a1x`, `actionIndex == 226` and
        // `byte_0x46_70 ∈ {3,4,5}`; model, class, life and the reap
        // flag came from the tick-top rebuild ([`Gen::mc2_roster`]).
        // ⚠ Model 28 is ABOVE the MC1-sized (20) test helper's cap, so
        // an MC2 test must build its chains with
        // `rebuild_mob_chains_mc2`.
        if crate::engine::features::no_mc2_mob_chain_predicate() {
            return self.ent.iter().enumerate().skip(1).any(|(j, c)| {
                j != i
                    && c.class64 == 5
                    && c.model65 == 28
                    && c.flags & 0x400 == 0
                    && matches!(c.f71, 3 | 4 | 5)
                    && c.tick70 == M28_BASE + 2
            });
        }
        self.mc2_roster(28).iter().any(|&s| {
            let j = s as usize;
            let c = &self.ent[j];
            j != i && matches!(c.f71, 3 | 4 | 5) && c.tick70 == M28_BASE + 2
        })
    }

    pub(crate) fn m28_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M28_BASE {
            0 => {
                self.mc2_patrol(i, M28_BASE);
                if self.ent[i].tick70 == M28_BASE + 2 {
                    self.ent[i].f71 = 0;
                }
            }
            1 => {
                self.mc2_idle(i, M28_BASE, ctx);
                if self.ent[i].tick70 == M28_BASE + 2 {
                    self.ent[i].f71 = 0;
                }
            }
            2 => self.m28_attack(i, ctx),
            3 => self.ent[i].tick70 = M28_BASE + 1,
            4 => self.mc2_prekill(i, M28_BASE),
            5 => {
                self.mc2_kill(i);
            }
            6 => {} // sub_2B7A0 MISSING — unreachable
            _ => {
                if self.ent[i].tick70 == M28_BASE + 2 {
                    self.ent[i].f71 = 0;
                }
            }
        }
    }

    /// LABEL_35 of `sub_2B260` (EF:21132-21170 + LABEL_58 at
    /// EF:21226-35) — the wind-up/swing body. Arm 3 falls into it
    /// the same tick (file 0x4FCBC), which is why it is a method.
    fn m28_windup(&mut self, i: usize, slot: u16, ctx: &MobCtx) {
        if self.ent[i].f26 <= 0 {
            self.m28_sub(i, 6);
            return;
        }
        self.ent[i].f30 = self.ent[i].f50 as u16;
        self.ent[i].f34 = self.ent[i].f30;
        if self.ent[i].f71 == 4 {
            if let Some((tx, ty, _)) = self.mc2_target(slot, ctx) {
                if self.ent[i].f63 & 7 == 0 {
                    let e = &self.ent[i];
                    if Self::dist2_sq(e.x, e.y, tx, ty) > 802_816 {
                        let e = &self.ent[i];
                        self.ent[i].f34 = Self::angle_between(e.x, e.y, tx, ty);
                    }
                }
                // `word_0x2C_44 - 3 > dword_0x10_16 &&
                //  dword_0x10_16 > 3` (EF:21157, file 0x4FD45)
                // — the port's `4..=12` was the 16-frame
                // hard-code's window.
                let f26 = self.ent[i].f26;
                let hot = if no_m28_strike_frames() {
                    (4..=12).contains(&f26)
                } else {
                    M28_STRIKE_FRAMES - 3 > f26 && f26 > 3
                };
                if hot && self.mc2_atk_melee_768(i, slot, ctx) {
                    self.ent[i].f71 = 5;
                }
            }
        }
        self.ent[i].f26 -= 1;
        if self.ent[i].f63 & 3 == 0 {
            self.mc2_avoid_packmate(i);
        }
        self.mc2_move_core(i);
        self.ent[i].f50 = self.ent[i].f30 as i16;
        let swing = if self.ent[i].f26 & 4 != 0 { 56 } else { -56 };
        self.ent[i].f30 = (self.ent[i].f30 as i32 + swing) as u16 & 0x7FF;
    }

    /// LABEL_76 of `sub_2B260` (EF:21188-21203) — the random-heading
    /// walk. Arm 7 falls into it the same tick (file 0x4FF05).
    fn m28_walk(&mut self, i: usize) {
        self.mc2_move_core(i);
        self.ent[i].f26 -= 1;
        if self.ent[i].f26 <= 0 {
            self.m28_sub(i, 9);
        }
    }

    /// `sub_2B260` (:21010) — the swing machine.
    fn m28_attack(&mut self, i: usize, ctx: &MobCtx) {
        let v1 = {
            let v = self.mc2_state_head(i);
            if v == 2 {
                self.ent[i].tick70 = M28_BASE + 4;
                return;
            }
            v
        };
        if v1 == 1 {
            self.ent[i].f146 = self.ent[i].f40;
        }
        let slot = self.ent[i].f146;
        match self.ent[i].f71 {
            0 => {
                self.m28_pose(i, 3);
                self.m28_sub(i, 1);
            }
            1 => {
                let (x, y, z) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z)
                };
                self.mc2_spawn_splash(x, y, z);
                self.m28_sub(i, 2);
            }
            2 => {
                let Some((tx, ty, _)) = self.mc2_target(slot, ctx) else {
                    if !self.m28_strike_taken(i) {
                        self.m28_sub(i, 3);
                    }
                    return;
                };
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 <= 0 {
                    if !self.m28_strike_taken(i) {
                        self.m28_sub(i, 3);
                    }
                    return;
                }
                // Chase the point 768 ahead of the target's facing.
                let tyaw = if slot == PLAYER_TARGET {
                    ctx.pyaw
                } else {
                    self.ent[slot as usize].f30
                };
                let mut pred = (tx, ty, 0i16);
                Self::polar_step(&mut pred, tyaw, 0, 768);
                if self.ent[i].f63 & 3 == 0 {
                    self.mc2_aim_avoid(i, pred.0, pred.1);
                }
                let mv = self.mc2_move_core(i);
                if mv == 3 {
                    self.m28_sub(i, 7);
                } else if self.ent[i].f63 & 3 == 0 && self.ent[i].f26 < 14 {
                    // ⭐ THE RANGE TEST MEASURES TO `v23x`, THE POINT
                    // THE CHASE IS STEERING AT — the target's position
                    // stepped 768 along the target's own yaw, NOT
                    // `v25x->position_0x4C_76` (file 0x4FC0E pushes
                    // `[ebp-0x1c]`; see [`no_m28_strike_range_pred`]).
                    let (rx, ry) = if no_m28_strike_range_pred() {
                        (tx, ty)
                    } else {
                        (pred.0, pred.1)
                    };
                    let e = &self.ent[i];
                    let d2 = Self::dist2_sq(e.x, e.y, rx, ry);
                    if d2 < 2_768_896 && !self.m28_strike_taken(i) {
                        self.m28_sub(i, 3);
                    }
                }
            }
            3 => {
                self.m28_sub(i, 4);
                self.m28_pose(i, 2);
                self.ent[i].f50 = self.ent[i].f30 as i16;
                self.snd(38, i);
                // `goto LABEL_35` — arm 3 has no epilogue (file
                // 0x4FCBC follows the sound call directly).
                if !no_m28_strike_fallthrough() {
                    self.m28_windup(i, slot, ctx);
                }
            }
            4 | 5 => self.m28_windup(i, slot, ctx),
            6 => {
                self.m28_pose(i, 3);
                {
                    let (x, y, z) = {
                        let e = &self.ent[i];
                        (e.x, e.y, e.z)
                    };
                    self.mc2_spawn_splash(x, y, z);
                }
                let ok = self.mc2_target(slot, ctx).is_some_and(|(tx, ty, tz)| {
                    let e = &self.ent[i];
                    Self::mc2_dist3((e.x, e.y, e.z), (tx, ty, tz))
                        < BEHAVIOR[e.row156 as usize].v_28 as u32
                });
                if ok {
                    self.m28_sub(i, 2);
                } else {
                    self.m28_sub(i, 7);
                }
            }
            7 => {
                let d = self.mc2_rand(i);
                self.ent[i].f34 = (d & 0x7FF) as u16;
                self.m28_sub(i, 8);
                // `goto LABEL_76` — arm 7 has no epilogue either
                // (file 0x4FF05 follows `sub_2BA50(a1x, 8u)`).
                if !no_m28_strike_fallthrough() {
                    self.m28_walk(i);
                }
            }
            8 => self.m28_walk(i),
            _ => {
                self.m28_pose(i, 1);
                self.ent[i].tick70 = M28_BASE + 1;
                self.ent[i].f146 = 0;
            }
        }
    }
}

/// Nearest-candidate accumulator test.
fn best_d2(best: &Option<(usize, i32)>, d2: i32) -> bool {
    best.is_none_or(|(_, bd)| d2 < bd)
}

/// Set `MGC_NO_M9_PREY_ROSTER=1` to restore the pre-2026-09-13 LIVE-POOL
/// castle walk in the model-9 (skeleton) prey seek (`sub_203D0`,
/// EF:12117-48) — the walk that could see a castle born earlier in the
/// same tick and skipped one that died mid-tick.
fn m9_prey_roster_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M9_PREY_ROSTER").is_none())
}

#[cfg(test)]
mod tests {
    use super::{BEHAVIOR, M14_BASE};
    use crate::engine::features::Gen;
    use crate::mc1::mobs::MobCtx;

    fn m14_flat_gen() -> Gen {
        use crate::chassis::ChassisParams;
        use crate::engine::features::{FeatureAssets, Planes};
        use crate::verbs::VerbSet;
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

    fn m14_ctx() -> MobCtx {
        MobCtx {
            px: 0,
            py: 0,
            pz: 100,
            pyaw: 0,
            pmana: 0,
            pmana_max: 0,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        }
    }

    /// ⭐⭐ THE TRADER'S HALF OF **THE TOWNIE RALLY TARGET IS
    /// REAP-BLIND** — the third call path, and the reason the law is
    /// not landed until all three carry it.
    ///
    /// `sub_237B0` (EF:14828-30; `NETHERW.EXE` 0x4822d
    /// `cmpb $0xa,0x3f(%esi)` / 0x48233 `cmpb $0x2d,0x40(%esi)`)
    /// validates `word_0x96_150` on class 10 + model 45 and NOTHING
    /// else. The villager and archer halves are pinned by
    /// `mc2::mobs::tests::a_townie_keeps_rallying_to_a_dwelling_reaped_this_tick`;
    /// this is the same predicate on the trader brain, where the port
    /// had added the same invented `flags & 0x400 == 0` term.
    ///
    /// The second assertion pins the OTHER law on this brain, the one
    /// the mc2l19 fixture
    /// `a-trader-s-invalid-target-returns-and-spends-no-draws` grades
    /// on the corpus: `word_0x96_150 != 0` is the OUTER branch
    /// (0x480ee `mov 0x96(%ebx),%di` / `test %di,%di` / `jne 0x48221`),
    /// so an INVALID non-zero handle clears, takes `maxSpeed` and
    /// RETURNS — it never reaches the wander turn's two per-entity LCG
    /// draws or the far-dwelling rescan.
    ///
    /// `MGC_NO_MC2_TOWNIE_TARGET_REAP_BLIND=1` fails the first;
    /// `MGC_NO_MC2_TRADER_INVALID_TARGET_RETURN=1` fails the second.
    #[test]
    fn the_trader_rally_target_is_reap_blind_and_an_invalid_one_returns() {
        let trader = |g: &mut Gen, target: u16| -> usize {
            let i = g.new_event().expect("trader slot");
            {
                let e = &mut g.ent[i];
                e.class64 = 5;
                e.model65 = 14;
                e.tick70 = M14_BASE; // state 0 — the walk
                e.max_life = 1_000;
                e.act_life = 1_000;
                e.f146 = target;
                e.f63 = 0; // on the cadence
                e.f34 = 0;
                e.f126 = 0;
                e.f130 = 40; // maxSpeed
            }
            let (x, y) = (100u16 << 8, 100u16 << 8);
            let z = g.ground_z(x, y) as i16;
            g.link(i, x, y, z);
            i
        };

        // THE REAP-BLIND LAW: a dwelling flagged for the free pass
        // this tick is still class 10 / model 45, so the rally stands.
        let mut g = m14_flat_gen();
        let ctx = m14_ctx();
        let d = g.new_event().expect("dwelling slot");
        {
            let e = &mut g.ent[d];
            e.class64 = 10;
            e.model65 = 45;
            e.act_life = 1_000;
            e.f128 = 4;
            e.flags |= 0x400; // reaped THIS tick
        }
        let (dx, dy) = (120u16 << 8, 100u16 << 8);
        let dz = g.ground_z(dx, dy) as i16;
        g.link(d, dx, dy, dz);
        let t = trader(&mut g, d as u16);
        g.m14_brain(t, &ctx);
        assert_eq!(
            g.ent[t].f146, d as u16,
            "the trader's rally handle survives the reap stamp"
        );
        let (tx, ty) = (g.ent[t].x, g.ent[t].y);
        assert_eq!(
            g.ent[t].f34,
            Gen::angle_between(tx, ty, dx, dy),
            "…and it keeps walking there"
        );

        // THE INVALID-TARGET RETURN: a non-zero handle that is not a
        // (10,45) clears, takes maxSpeed and spends NO draws.
        let mut g = m14_flat_gen();
        let junk = g.new_event().expect("junk slot");
        g.ent[junk].class64 = 10;
        g.ent[junk].model65 = 44; // not a dwelling
        let t = trader(&mut g, junk as u16);
        let rng_before = g.ent[t].rand;
        assert!(
            BEHAVIOR[g.ent[t].row156 as usize].v_26.max(1) == 1
                || g.ent[t].f63 % BEHAVIOR[g.ent[t].row156 as usize].v_26.max(1) as u8 == 0,
            "fixture: the trader is on its cadence tick"
        );
        g.m14_brain(t, &ctx);
        assert_eq!(g.ent[t].f146, 0, "the invalid handle is cleared");
        assert_eq!(
            g.ent[t].f126, g.ent[t].f130,
            "…actSpeed = maxSpeed, LABEL_44"
        );
        assert_eq!(
            g.ent[t].rand, rng_before,
            "…and the wander turn's two per-entity draws are NOT spent"
        );
    }
}
