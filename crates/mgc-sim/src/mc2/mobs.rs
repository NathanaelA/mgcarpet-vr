//! MC2 creature machinery: the class-5 dispatch, shared state
//! primitives, the slice creatures (Goat m1, Archers m4, Villager
//! m13) and the (9,13) archer arrow, ported from remc2
//! EventsFunctions.cpp (`:N` cites; trace bank docs/archive/PHASE3-RESEARCH.md).
//! Runs on the SHARED chassis ([`crate::engine::features::Gen`]) — same
//! pool, mailboxes, LCG, terrain samplers. MC2's NewEvent defaults
//! match MC1's field-for-field (life 300, flags dword 8, speed 16,
//! strength 100, id = slot, filter bytes -1; Events.cpp:582-599).
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
//! bit 28 · byte[2]&0x20 forced-claim lock→flags bit 29.
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
use crate::engine::features::Gen;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// MC2-only flag bits on [`Ent::flags`] (high bits; MC1 owns the low
/// ones — see the module doc mapping).
///
/// ⚠ `F_WHOOSH` is NOT one of them: `AddArcherArrow_672E0` sets
/// `struct_byte_0xc_12_15.byte[0] |= 2` — retail's byte[0] bit 1, the
/// lane the obs projects as `flags.b0_done2` and the same seat every
/// other MC2 projectile's one-shot whoosh already uses (`mc2/proj.rs`,
/// `mc2/tail.rs`). Parking it in a private high bit cost two things:
/// the obs lane read 0 on every arrow in flight (mc2l0 t=3988), and an
/// IMPORTED mid-flight arrow — whose bit 1 the recording restores —
/// failed the port's test and re-rolled the GLOBAL LCG for a sound it
/// had already played.
pub(crate) const F_WHOOSH: u32 = 1 << 1; // byte[0] & 2 (arrow sound played)
pub(crate) const F_STOP: u32 = 1 << 26; // byte[1] & 8 (forced stop)
pub(crate) const F_BLOCKED: u32 = 1 << 27; // byte[2] & 4 (move blocked)
pub(crate) const F_NO_CORPSE: u32 = 1 << 28; // byte[2] & 0x10
/// byte[2] & 0x20 — the Mana Lock claim lock (EF:28026/26084): set by
/// a FORCED claim ((10,70) pulse, possession tier 2); a locked target
/// ignores weak claims — only another forced claim steals it.
pub(crate) const F_CLAIM_LOCK: u32 = 1 << 29;

const GOAT_BASE: u8 = 8;
const ARCHER_BASE: u8 = 32;
const VILLAGER_BASE: u8 = 104;
/// The arrow's action/state (= its model; :35031).
const ARROW_STATE: u8 = 13;

/// A/B toggle for the SUMMON-LEASE / SCRATCH-LANE SPLIT: set
/// `MGC_NO_SUMMON_LEASE_SPLIT` to restore the pre-dig behaviour, where
/// the port's single `f26` lane carried retail's `word_0x2E_46`
/// SUMMON LEASE for the whole life of a StageVar2 12/13/14/16/17
/// class-5 record — including after it left the StageVar2 state — so
/// every combat handler that reads `dword_0x10_16` (@0x10) read the
/// lease instead. `sub_1E580` (the ONLY @0x2E reader on those five
/// kinds) is dispatched from action `8*model + 7` alone, so @0x2E is
/// live on that state and DEAD everywhere else.
pub(crate) fn no_summon_lease_split() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_SUMMON_LEASE_SPLIT").is_some())
}

/// A/B toggle for the CONTROLLED seam's MODEL-WRAPPER TAIL: set
/// `MGC_NO_MC2_CONTROLLED_WRAPPER_TAIL` to restore the pre-dig
/// behaviour, where `mc2_creature_tick`'s StageVar2 12/13/14/16/17 arm
/// returned before the per-model phase-7 wrapper's own last statement
/// — so an archer the controlled handler promoted to `8m+2` skipped
/// `sub_20060`'s aim (EF:11960-66).
fn no_mc2_controlled_wrapper_tail() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_CONTROLLED_WRAPPER_TAIL").is_some())
}

/// A/B toggle for the ALLIANCE EXECUTOR RECORD: set
/// `MGC_NO_MC2_ALLIANCE_RECORD` to restore the pre-dig behaviour,
/// where the (10,74) impact arm converted the victims INLINE instead
/// of minting `sub_50800`'s record and letting its own class-10
/// action-0x51 tick (`sub_3A650`) do it one tick later.
pub(crate) fn no_mc2_alliance_record() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ALLIANCE_RECORD").is_some())
}

/// A/B toggle for the ALLIANCE CHARM'S PARENT SEAT: set
/// `MGC_NO_MC2_ALLIANCE_PARENT_SEAT` to restore the pre-dig behaviour,
/// where the pair importer cleared `mc2_allied` and never re-seeded it
/// from the victim's `parentId_0x28_40` (so every imported charm ended
/// on its first ticked pair), fused that `@0x28` into `id24` (losing
/// the victim's own `@0x1A`), and projected the `owner` obs lane as 0.
pub(crate) fn no_mc2_alliance_parent_seat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ALLIANCE_PARENT_SEAT").is_some())
}

/// A/B toggle for the ALLIANCE slot's `sub_1E700` CORE: set
/// `MGC_NO_MC2_ALLIANCE_CORE` to restore the pre-dig paraphrase, which
/// ran its own target scan and stood the ally STILL whenever it found
/// nobody, where `sub_1E9C0` (EF:10971-73) parks the lock on the
/// parent and runs the shared controlled-summon core — so a charmed
/// creature with no enemy follows its caster.
fn no_mc2_alliance_core() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ALLIANCE_CORE").is_some())
}

/// A/B toggle for THE VILLAGE HOUSE'S PERIODIC POPULATION SPAWN —
/// `GetRandManaSphere_38270`'s **SECOND** CALL SITE. Set
/// `MGC_NO_MC2_HOUSE_POP=1` to restore the pre-dig behaviour, where
/// `AddHouse0A_2D_38330` never ran the block at all: the helper was
/// ported as [`Gen::mc2_rand_occupant`] but reached from the COLLAPSE
/// path alone (EF:28121, [`World::mc2_house_collapse`]) — a law landed
/// on one call path is not landed.
pub(crate) fn no_mc2_house_pop() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_HOUSE_POP").is_some())
}

/// `MGC_NO_MC2_BUILDING_XTYPE_ZERO=1` restores the pre-dig
/// [`Gen::mc2_spawn_building`], which left `xtype_0x41_65` at
/// `NewEvent_4A050`'s fresh-record 0xFF instead of the ctor's own
/// `= 0` (EF:32796, shipped `NETHERW.EXE` file 0x6E3BD) — so a
/// villager-built shrine fired the phantom disposition 255 on death.
/// (Round 104, dig W3-S.)
fn no_mc2_building_xtype_zero() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_BUILDING_XTYPE_ZERO").is_some())
}

/// A/B toggle for the (10,78) MAGIC-MINE HOMING BEACON `sub_68940`
/// (EF:55315, shipped EXE 0x8D140): set `MGC_NO_MINE_BEACON` to
/// restore the pre-dig behaviour, where a class-9 flyer's one-shot
/// acquisition ran `sub_67CB0` alone.
#[allow(dead_code)]
pub(crate) fn no_mine_beacon() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_BEACON").is_some())
}

/// A/B toggle for the victim probe's placement of the OUT-OF-POOL
/// HUMAN INSIDE HIS OWN TILE (`MGC_NO_PLAYER_CELL_TAIL=1` restores
/// the pre-dig order, where the human was tested BEFORE the pool
/// records linked in that tile). Retail's `sub_10780` walks the tile
/// chain and returns the FIRST overlapping record; the chain is
/// HEAD-INSERTION on cell entry (`AddEventToMap_57D70` EF:40315-27:
/// `entity->oldMapEntity_0x16_22 = mapEntityIndex[cell];
/// mapEntityIndex[cell] = entity`) and `CopyEntityPosition_57CF0`
/// (EF:40282-99) relinks only ACROSS tiles — so a pool record that
/// flew into the human's tile after he parked there is walked BEFORE
/// him.
pub(crate) fn no_player_cell_tail() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_PLAYER_CELL_TAIL").is_some())
}

/// ARROW-VOLLEY PROBE (`MGC_ARROW_VOLLEY_TRACE=1`): the (5,15) guard volley gate and
/// every (9,13) mint — the instrument that showed the port firing a
/// guard arrow retail does not (mc2l22 pair 1887, guard 719).
pub(crate) fn arrow_volley_trace_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MGC_ARROW_VOLLEY_TRACE").is_ok())
}

/// A/B toggle for the MC2 SPRITE FRAME COUNT (`byte_0x5D_93`, our
/// [`crate::engine::features::Ent::frames89`], the raw-shadow `b5d`
/// lane): set `MGC_NO_MC2_FRAMES89` to restore the pre-dig behaviour,
/// where the MC2 column never wrote the lane at all and every MC2
/// record carried 0 where retail carries the sprite's frame count.
pub(crate) fn no_mc2_frames89() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FRAMES89").is_some())
}

/// A/B toggle for the ARCHER ACQUIRE **Scan B** (`sub_1FAA0` :11811):
/// set `MGC_NO_MC2_ARCHER_SCANB_M9_ONLY` to restore the pre-dig
/// "unnatural fallback", where an archer whose model-9 quarry was
/// EXTINCT re-ran the scan over model 3 (worms) and locked one.
/// The law is retail's single hard-coded model-9 chain walk.
pub(crate) fn no_mc2_archer_scanb_m9_only() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ARCHER_SCANB_M9_ONLY").is_some())
}

/// `x_BYTE_D8A2E[38]` (EF:2297) — DRAW TYPE -> animation frame count.
/// Verified byte-for-byte against the shipped `NETHERW.EXE` at file
/// offset **0xEE22E** (the data segment sits 0xF000 BELOW the code
/// mapping, i.e. `off = 0x24800 + linear - 0xF000` for data, not the
/// code rule `0x24800 + linear`); `SetEntityIndex_49C90` reads it as
/// `mov 0x8a2e(%eax),%al` at file 0x6E4BA.
///
/// Same table as MC1's [`crate::mc1::mobs::FRAME_COUNTS`] plus a
/// trailing 0: types 0/1 = single view, 2..=16 = that many animation
/// frames, 17..=21 = multi-VIEW families (one frame each), 22..=36 =
/// animated multi-view (2..16 frames), 37 = none.
pub(crate) const D8A2E_FRAMES: [u8; 38] = [
    1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, //
    1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 0,
];

/// `particlesParameters_D951C[row].byte_12` AS IT STANDS AT PLAY TIME.
///
/// ⚠ THE SHIPPED TABLE'S `byte_12` COLUMN IS ALL ZEROES — verified in
/// both the decompile (`Type_WORD_D951C.cpp`) and the shipped
/// `NETHERW.EXE` (file 0xEED1C, 347 x 14 bytes; every other column
/// matches the decompile exactly, only the two relocated pointer words
/// `word_2`/`word_4` differ). The column is FILLED AT BOOT by
/// `sub_71410_process_tmaps` (EF:44878-909), which decompresses each
/// row's sprite and copies the sprite header's FLAGS HIGH BYTE:
/// `particlesParameters_D951C[i].byte_12 = *(x_BYTE*)(v1 + 1)`
/// (EF:44906) — exactly MC1's `draw_type` column, which MC1 ships
/// pre-baked ([`crate::mc1::sprite_stats`] documents the same
/// `flags >> 8` identity).
///
/// So this table is the retail BOOT PASS evaluated once, against
/// **TMAPS0-0 (the DAY bank)** — the same day-sourced law
/// `Bundle::mc2_extent_dims` / [`crate::mc2::derive_sprite_extents`]
/// already carry for the sibling `speed_6`/`rotSpeed_8` derivation in
/// that very loop: retail derives the particle-param table ONCE at
/// boot and never recomputes it, so night and cave levels run day-art
/// frame counts (the banks genuinely differ — sprite 426 is draw type
/// 11 in the day bank and 0 in night/cave). Row 346 is the loop's
/// terminator (`speed_6 == 0 && rotSpeed_8 == 0`) and is never
/// reached, so it keeps the shipped 0.
///
/// Regenerated/asserted by
/// `mc2_sprite_draw_types_match_the_day_bank_flag_high_bytes` below.
/// Only SEVEN rows resolve to a count other than 1: 7, 222, 237 -> 16
/// and 244, 332, 333, 334 -> 11.
pub(crate) const MC2_SPRITE_DRAW_TYPE: [u8; 347] = [
    20, 20, 17, 17, 17, 17, 17, 36, 21, 21, 21, 21, 21, 21, 21, 21, 21, 17, 21, 21,
    21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 0, 17, 17, 17, 0,
    17, 21, 21, 0, 17, 21, 17, 17, 17, 17, 17, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    0, 0, 17, 17, 17, 0, 0, 21, 21, 21, 21, 21, 21, 21, 21, 17, 21, 1, 17, 0,
    0, 0, 17, 0, 0, 17, 17, 17, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    21, 21, 21, 21, 21, 21, 21, 21, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 19, 21, 21, 17, 17,
    17, 20, 20, 19, 17, 17, 20, 17, 17, 21, 21, 21, 1, 21, 21, 21, 21, 20, 20, 20,
    0, 20, 36, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 17, 16, 20, 20,
    17, 20, 20, 17, 11, 0, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 0, 20, 0, 17, 17, 17, 17, 17, 17, 17,
    1, 21, 1, 17, 1, 17, 17, 17, 17, 17, 17, 17, 17, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 17, 17, 17, 17, 17, 17, 17, 17, 0, 0, 17, 17, 0, 17, 21,
    21, 21, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 11, 11, 11, 17, 17, 17, 0, 0,
    21, 17, 17, 17, 17, 17, 0,
];

/// Retail's frame count for a particle-param row:
/// `x_BYTE_D8A2E[particlesParameters_D951C[row].byte_12]`
/// (`SetEntityIndex_49C90` EF:32834 / `sub_49D50` EF:32851).
pub(crate) fn mc2_sprite_frames(row: usize) -> u8 {
    let dt = MC2_SPRITE_DRAW_TYPE.get(row).copied().unwrap_or(0) as usize;
    D8A2E_FRAMES.get(dt).copied().unwrap_or(0)
}

/// A/B toggle for the `sub_585A0` FRAME CAP: set
/// `MGC_NO_MC2_FRAME_CAP` to restore the pre-dig behaviour, where the
/// four MC2 ports of `sub_585A0` ran an UNCAPPED `frame88 += 1` (they
/// had to: `frames89` was 0 on the whole column, so retail's gate
/// would have frozen every animation).
pub(crate) fn no_mc2_frame_cap() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FRAME_CAP").is_some())
}

/// ⭐⭐⭐ A/B toggle for `ApplyTerrainModification_37240`'s OPENING
/// STATEMENT — the build site adopts the BUILDING's footprint box on
/// the first countdown tick.
///
/// ```c
/// // EF:27234-36, ApplyTerrainModification_37240
/// v50 = 0;
/// if (event->maxLife_0x4 == event->life_0x8)
///     SetShiftByCastle_49EC0(event, event->byte_0x46_70);
/// if (!IsNextEvent0A_2A_37740(event)) { … }
/// ```
///
/// Shipped `NETHERW.EXE` 0x5BA40 (`sub_37240` linear 0x37240,
/// `off = 0x34800 + linear − 0x10000`):
///
/// ```text
///   5ba4e  8b 53 04         mov    0x4(%ebx),%edx    ; maxLife_0x4
///   5ba54  3b 53 08         cmp    0x8(%ebx),%edx    ; life_0x8
///   5ba57  75 0e            jne    0x5ba67
///   5ba59  0f be 43 46      movsbl 0x46(%ebx),%eax   ; byte_0x46_70 (SIGNED)
///   5ba5f  e8 5c 2c 01 00   call   0x6e6c0           ; SetShiftByCastle_49EC0
/// ```
///
/// and `SetShiftByCastle_49EC0` itself at file 0x6E6C0 — a 6-byte
/// BUILD00 row (`4*n − n`, doubled), `width` at +4 and `height` at +5:
///
/// ```text
///   6e6fc  c1 e0 08         shl  $0x8,%eax
///   6e6ff  05 00 05 00 00   add  $0x500,%eax          ; +1280
///   6e704  d1 e8            shr  $1,%eax
///   6e706  66 89 43 54      mov  %ax,0x54(%ebx)       ; pitch  = ((w<<8)+1280)>>1
///   6e70f  66 c7 43 52 00 00   movw $0x0,0x52(%ebx)   ; yaw    = 0
///   6e71a  66 c7 43 58 00 01   movw $0x100,0x58(%ebx) ; fov    = 256
///   6e722  66 89 43 56      mov  %ax,0x56(%ebx)       ; roll   = ((h<<8)+1280)>>1
/// ```
///
/// The port's `mc2_building_tick` never carried the opener, so a
/// raising building kept whatever quad its class-10 SPRITE ctor left
/// on `f78/f80/f82/f84` — and those four words are not rotation
/// speeds at all on a building, they are the AABB half-extents
/// `sub_10630`/`sub_106C0` measure it by.
///
/// WITNESS — mc2l22 pair 44510→44511, slot 853, a `(10,45)` shrine at
/// `action 51`, `b46 = 13`, `life 30 == max_life 30`. Retail rewrites
/// `ayaw/apitch/aroll/afov` **200/194/194/200 → 0/768/768/256** on
/// that exact tick (BUILD00 row 13 is 1×1: `((1<<8)+1280)>>1 = 768`)
/// while `f5a` STAYS 177 — proof it is not a re-sprite. The port kept
/// `Gen::mc2_set_sprite(177)`'s derived quad.
///
/// `MGC_NO_MC2_BUILD_SITE_FOOTPRINT_BOX=1` restores the old behaviour.
pub(crate) fn no_mc2_build_site_footprint_box() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_BUILD_SITE_FOOTPRINT_BOX").is_some())
}

impl Gen {
    // ---- shared MC2 helpers ------------------------------------------------

    /// One u16 LCG draw (`rand_0x14_20 = 9377*x + 9439`); the
    /// chassis-selected [`Gen::ent_rand`] does exactly this under
    /// RandWidth::U16.
    pub(crate) fn mc2_rand(&mut self, i: usize) -> u32 {
        self.ent_rand(i)
    }

    /// The `rand_0x14 += setting_30` per-entity stream perturb —
    /// retail's ONLY three sites: the pyramid's two pick rolls
    /// (EF:13140/13220) and the m27 branch bolt (EF:20521). Applied
    /// AFTER the modulo draw, so the current roll reads the clean
    /// LCG value and the NEXT roll starts from the shifted seed.
    /// `turn` = `MobCtx::mc2_turn` (the post-increment counter the
    /// cave carpet tail's corpus solve anchored, EF:59803).
    pub(crate) fn mc2_rand_perturb(&mut self, i: usize, turn: u32) {
        self.ent[i].rand = self.ent[i].rand.wrapping_add(turn) & 0xFFFF;
    }

    /// `SetEntityIndexAndRot_49CD0` (:32837): store the sprite-param
    /// row and derive the rot/extent quad from it (/2). No RNG.
    pub(crate) fn mc2_set_sprite(&mut self, i: usize, idx: u16) {
        let (s6, r8) = self.mc2_params_ext(idx as usize);
        let frames = mc2_sprite_frames(idx as usize);
        let e = &mut self.ent[i];
        e.type86 = idx;
        e.frame88 = 0;
        // ⭐ ROUND 98 — `SetEntityIndex_49C90`'s THIRD LINE
        // (EF:32834, shipped NETHERW.EXE file 0x6E4AE-0x6E4C0:
        // `mov 0x9528(,%eax,2),%al` with `eax = 7*idx` reads
        // `D951C + 14*idx + 12` = byte_12, then `mov 0x8a2e(%eax),%al`
        // / `mov %al,0x5d(%ebx)`). The MC1 twin has always done this
        // (`mc1/mobs.rs` `set_sprite`); the MC2 column never did, so
        // every MC2 record carried 0 in the lane where retail carries
        // the count -- the single biggest ungraded family in the
        // pair census.
        if !no_mc2_frames89() {
            e.frames89 = frames;
        }
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
    /// `SetEntityIndex_49C90` (EF:32832-36) — the PLAIN sprite commit:
    /// index, frame reset, frame count, and **nothing else**.
    ///
    /// ⭐⭐⭐ **THERE ARE TWO SPRITE SETTERS AND THE QUAD IS THE ONLY
    /// DIFFERENCE.** [`Self::mc2_set_sprite`] is
    /// `SetEntityIndexAndRot_49CD0`, which calls this and THEN stamps
    /// the `array_0x52_82` half-extents from the particle-param row
    /// (`.yaw = .fov = rotSpeed_8/2`, `.pitch = .roll = speed_6/2`).
    /// Handlers that re-sprite an entity mid-life mostly want THIS one
    /// — their box was set once at spawn and must not be re-derived
    /// every time the animation changes. Reaching for the `AndRot`
    /// twin on a re-sprite silently resizes the creature's collision
    /// box, which nothing else in the tick will put back.
    pub(crate) fn mc2_set_sprite_index(&mut self, i: usize, idx: u16) {
        let frames = mc2_sprite_frames(idx as usize);
        let e = &mut self.ent[i];
        e.type86 = idx;
        e.frame88 = 0;
        if !no_mc2_frames89() {
            e.frames89 = frames;
        }
    }

    pub(crate) fn mc2_set_sprite_x2(&mut self, i: usize, idx: u16) {
        self.mc2_set_sprite(i, idx);
        let e = &mut self.ent[i];
        e.f80 *= 2;
        e.f82 *= 2;
        e.f84 *= 2;
    }

    /// `sub_585A0` (EF:40438) — the ONE animation-frame advance the
    /// whole MC2 engine has, verbatim from the shipped `NETHERW.EXE`
    /// at file **0x7CDA0**:
    ///
    /// ```text
    ///   mov  0x5c(%edx),%al      ; animationFrame_0x5C_92
    ///   cmp  0x5d(%edx),%al      ; vs byte_0x5D_93 (the frame count)
    ///   jae  <out>               ; UNSIGNED >= -> no step
    ///   mov  0x5c(%edx),%bl ; inc %bl ; mov %bl,0x5c(%edx)
    /// ```
    ///
    /// i.e. `if frame88 < frames89 { frame88 += 1 }` — NOT the port's
    /// old uncapped `+= 1`, and NOT `frame88 + 1 < frames89`. Retail
    /// calls it from EF:11983 (m9 materialize), 22758 ((10,0) fire),
    /// 22852 ((10,86) cave drip), 23173 (splash), 23553/23567, 24855
    /// (the duel tether) and 28229.
    ///
    /// ⭐ Only reachable because `mc2_set_sprite` now stamps
    /// `frames89`: with the lane at 0 the gate would freeze every
    /// animation, which is why the four port sites open-coded an
    /// uncapped increment. Retail's own captures agree — across 80
    /// sampled `mc2l6-rsg` states the ONLY records with a nonzero
    /// `+0x5C` are `(10,0)` (sprite row 7, count 16), at 7/8/9.
    pub(crate) fn mc2_anim_step(&mut self, i: usize) {
        let e = &mut self.ent[i];
        if no_mc2_frame_cap() {
            e.frame88 = e.frame88.saturating_add(1);
        } else if e.frame88 < e.frames89 {
            e.frame88 += 1;
        }
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
    pub(crate) fn mc2_alt_core(z: &mut i16, ground: i16, hover: i16, z_step: i16) {
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
        // probe at the predicted point. Order matters — testing after
        // the step probes an extra 256-step point and false-blocks a
        // tile early (1-tile causeways).
        loop {
            if walked > reach {
                return false;
            }
            if !row.v_20 & self.cap_bit(pos.0, pos.1) != 0 {
                return true;
            }
            if self.is_cave() {
                let t = crate::engine::features::tile((pos.0 >> 8) as u8, (pos.1 >> 8) as u8);
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

    /// Diagnostic (the flocking terrain-fence check): the whole-map
    /// walkability of creature `i`'s behavior row, one byte per tile —
    /// bit 0 = roughness >= v_16 (the slope fence), bit 1 = tile-type
    /// blocked (`!v_20 & cap_bit`). Probes tile centers.
    pub(crate) fn mc2_block_map(&self, i: usize) -> Vec<u8> {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let mut out = vec![0u8; 256 * 256];
        for ty in 0..256u16 {
            for tx in 0..256u16 {
                let (x, y) = (tx * 256 + 128, ty * 256 + 128);
                let mut b = 0u8;
                if self.roughness(x, y) >= row.v_16 as i32 {
                    b |= 1;
                }
                if !row.v_20 & self.cap_bit(x, y) != 0 {
                    b |= 2;
                }
                out[(ty as usize) << 8 | tx as usize] = b;
            }
        }
        out
    }

    /// One predicted candidate of the MC2 move core: altitude core +
    /// polar step at the CURRENT yaw, then the block test (crossing
    /// into a new tile only).
    /// `always_test`: the retry predictions run the block/roughness
    /// test UNCONDITIONALLY (EF:8826/8840/8852) — only the FIRST
    /// prediction gates it on the tile change (EF:8806). A rotated
    /// retry that stays in-tile must still be terrain-tested.
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
        // creature_move already uses its v_2 twin). NOT v_4 (=5),
        // which under-turns 4-9x and can't catch the wander heading.
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
    /// `mc2_wanted` side channel.
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
        // ⚠ `pdead` is the roster's ENTRY test, not a walk test — see
        // [`Gen::mc2_wizard_scan`]. `dword_38519` only ever holds
        // `life_0x8 >= 0` records (EF:39975); the pool arm below
        // applies that itself, the out-of-pool human cannot.
        if !self.player_invisible && !ctx.pdead {
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
        // ⭐⭐⭐ AND THE ROSTER'S OWN MEMBERSHIP TEST IS A LIFE TEST,
        // WHICH THE OUT-OF-POOL HUMAN BYPASSES ENTIRELY. The case-3
        // arm of the tick-top sweep is `if (jx->life_0x8 >= 0)`
        // (EF:39975) — a DEAD wizard is simply not linked into
        // `dword_38519`, which is exactly why the walk itself needs no
        // mortality test. Retail's human is a pool record and gets
        // that test for free; ours is a ctx pose, so it must take the
        // roster's entry condition here or it stays scannable as a
        // corpse. mc2l3 t=11757: the player has been dead since
        // t≈11700 (life −720) and the (5,20) at slot 161 sits in idle
        // 161 for the rest of the take; the port's scan handed back
        // `PLAYER_TARGET`, took 162, and `m20_validate` zeroed
        // `byte_0x46_70` on the way through.
        //
        // ⚠ NOT the invisibility bit: the dead carpet's record reads
        // `flags` 269 at 11757, so bit 5 is CLEAR. 39's "the death
        // touchdown raises the invisibility bit" is a different tick
        // in the death sequence and does not cover this one.
        let human_skip =
            self.player_invisible || ctx.pdead || (wanted_only && self.player_aggro <= 0);
        consider(ctx.px, ctx.py, PLAYER_TARGET, human_skip, &mut best);
        // ⭐⭐⭐ THE TICK-TOP CLASS-3 ROSTER `dword_38519` (EF:9147), AND
        // THE WALK RE-ASKS NOTHING BUT THE INVISIBILITY BIT. Retail's
        // sweep is `while (v12x > Entities_EA3E4[0])` over `next_0`
        // with exactly two tests before the cone — the squared range
        // and `!(byte[0] & 0x20)` (EF:9154). Class, model, life and the
        // reap flag were ALL settled when the case-3 arm of the tick-top
        // sweep built the roster (EF:39972-85), so re-asking them here
        // is wrong in both directions, like the pack scan below.
        //
        // ⭐⭐ AND THERE IS NO MODEL TEST — THE SCAN CAN RETURN A CASTLE.
        // The port filtered `model65 <= 1` and so could never hand back
        // a (3,2), which is precisely what the m20/m13 state-2 WRAPPERS
        // exist to undo: `sub_25DE0` (EF:16637-42) re-reads the lock
        // after `sub_1BF90` returns and clears it unless the record is
        // class 3 AND model 0 or 1. A filter that makes a wrapper
        // unreachable is a rewrite of the mechanism, not a shortcut for
        // it — and it changes WHICH candidate wins, because the scorer
        // is nearest-in-cone and a castle can be the nearest.
        // mc2l3 t=10222: the (5,20) at slot 1 finds a castle first,
        // takes state 162 with the lock cleared by the wrapper, and
        // drops straight back to 161 at 10223. The port's filtered scan
        // skipped the castle, locked the HUMAN (`word_0x96_150` 65535),
        // survived the wrapper's wizard test and stayed in 162 chasing —
        // yaw 563 against retail's 336.
        for c in 0..self.wiz_chain.visible_len() {
            let j = self.wiz_chain.list[c] as usize;
            let w = &self.ent[j];
            // Pool wizards carry no wanted timer yet (see
            // mc2_arm_wanted) — under wanted_only they never
            // qualify, faithful to an unarmed timer.
            consider(
                w.x,
                w.y,
                j as u16,
                w.flags & 0x20 != 0 || wanted_only,
                &mut best,
            );
        }
        best.map(|(s, _)| s)
    }

    /// The same-model pack scan (:9197-9231): nearest leaderless
    /// same-model creature in range + cone. `reversed_cone` = the +0
    /// patrol quirk (:9038): its cone test uses the REVERSED bearing
    /// `tan2(candidate → self)`, unlike wander's `tan2(self →
    /// candidate)` (:9194) — vestigial for goats/townies (they never
    /// occupy +0) but kept verbatim.
    pub(crate) fn mc2_pack_scan(&self, i: usize, reversed_cone: bool) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let mut best: Option<(u16, i32)> = None;
        // ⭐⭐ THE SCAN WALKS THE TICK-TOP PER-MODEL ROSTER, NOT THE
        // LIVE POOL — `bytearray_38403x[a1x->model_0x40_64]` chased
        // through `next_0` (EF:9183), the same roster
        // [`Gen::mc2_avoid_packmate`] already reads. Retail admits on
        // exactly TWO conditions, `!jx->word_0x32_50` (leaderless) and
        // `jx != a1x` (EF:9185); class, model, life and the reap flag
        // were ALL settled when the roster was built at the top of the
        // frame (EF:39987-40008), so re-asking them at the walk is
        // wrong in both directions at once.
        // mc2l3 t=356: the dis-6 wave mints six (5,20)s mid-tick, so
        // model 20's roster is EMPTY for the rest of that frame —
        // retail's first-ordinal newborn (slot 131, the only one whose
        // `f63 % period` cadence gate opens on its birth tick) finds no
        // leader and stays at action 161, where the live-pool walk saw
        // its five just-minted siblings and promoted it to the
        // pack-follow state 163.
        for &s in self.mob_chains.visible(e.model65 as usize) {
            let j = s as usize;
            let c = &self.ent[j];
            if j == i || c.f52 != 0 {
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

    /// A/B toggle for the SEPARATION-BOX SPAN law (dig C2, session
    /// 96): set `MGC_NO_PACKBOX_SPAN` to restore the pre-2026-09-03
    /// WRAPPING 16-bit box test on the non-`sub_1DDA0` call paths.
    pub(crate) fn packbox_span_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_PACKBOX_SPAN").is_none())
    }

    /// The same-model AVOIDANCE override in chase/flee re-aims
    /// (:9643-56): first packmate closer than array.pitch on both
    /// axes steers us away from it.
    pub(crate) fn mc2_avoid_packmate(&mut self, i: usize) {
        self.mc2_avoid_packmate_at(i, Self::packbox_span_law());
    }

    /// `cast16` — THE SEPARATION BOX IS SEAM-BLIND, AND IT IS SEAM-
    /// BLIND ON **EVERY** WALK, NOT JUST `sub_1DDA0`'s.
    /// `sub_1DDA0`'s box test (EF:10404-09) sign-casts each u16
    /// position to int16 BEFORE the plain-int subtraction, so two
    /// packmates straddling the x/y 0x8000 map-CENTER seam read an
    /// astronomical difference and never separate: mc2l0-pd t=906,
    /// villager 66 (x 32745) vs 54 (x 32867, sign-cast NEGATIVE) —
    /// wrapped |dx| = 122 < apitch 128 but retail keeps the pure
    /// waypoint aim 507 where the wrapped test wrote the away-bearing
    /// 1388.
    ///
    /// ⭐⭐⭐ THE SHIPPED EXE SETTLES THE OTHER WALKS TOO, AND THEY
    /// AGREE. This doc used to say the chase/flee (:9644-46) and m15
    /// (:15304-05) walks "subtract the raw u16s with NO cast" and
    /// that the port's WRAPPED difference stood in for them. That is
    /// false: every one of them emits the same
    ///   `movsx eax,word[..+0x4c] ; movsx edx,word[..+0x4c]
    ///    sub eax,edx ; cdq/xor/sub ; cmp ; jnl`
    /// pair — sign-extend BOTH coordinates to 32 bits, THEN subtract,
    /// so the span never wraps. Four independent sites, disassembled
    /// (`off = 0x34800 + linear − 0x10000`):
    ///   `sub_1C980` chase/flee  +0x1CAEA (file 0x4196A)
    ///   `sub_24190` m15         +0x24330 (file 0x491B0)
    ///   `sub_1E700` summon crowd-steer +0x1E918 (file 0x43718)
    ///   `sub_27120` m22 anti-stack     +0x27168 (file 0x4B968)
    /// The contrast is real and it is with the DISTANCE primitives,
    /// not with these: `sub_583F0` (3-D distance) genuinely subtracts
    /// in 16 bits (`mov ax,[esi] ; sub ax,[ecx] ; movsx eax,dx` at
    /// +0x583FB) and DOES wrap. ⭐⭐ Which primitive the call site
    /// picks is the law.
    pub(crate) fn mc2_avoid_packmate_at(&mut self, i: usize, cast16: bool) {
        let (ex, ey, pitch, model, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.f80 as i32, e.model65, e.id24)
        };
        if pitch == 0 {
            return;
        }
        // ⭐⭐ THE SCAN WALKS THE TICK-TOP PER-MODEL ROSTER, NOT THE
        // LIVE POOL. `EF:9641-50` iterates `bytearray_38403[model]`
        // via `next_0` and admits the first record on THREE
        // conditions and no others — `id != self` and the two box
        // tests. Every liveness question was already settled when the
        // roster was BUILT, at the top of the frame (see the case-5
        // arm in `World::tick_inner`'s sweep), so re-asking it here
        // is wrong in both directions: a packmate that dies mid-tick
        // still shoves its neighbour aside for the rest of the frame,
        // and one that was already dead at the top stays invisible
        // however intact its record looks.
        //
        // mc2l3 dates both halves on one creature. Firebug 160 holds
        // life 600 at the t=252 boundary and −1 at t=253; at t=253
        // retail's scan from firebug 132 at (27371, 50485) STILL
        // takes it at (27354, 50421) and writes `roll` =
        // `angle_of(17, 64)` = 940, which the heading servo chases by
        // −113 the next tick (yaw 1771 → 1658). At t=257 the same
        // record sits in the same place in the same state and retail
        // does NOT take it — the roster rebuilt at the top of 254
        // without it. A live-pool walk with a life test misses the
        // first; a live-pool walk without one invents the second.
        //
        // ⚠ The `roll` lane is UNGRADED, so neither showed on its own
        // tick: the report named `heading` a tick later, once the
        // servo had something (or nothing) to chase. The divergence
        // report shows what DIFFERS, never what CHANGED.
        let roster: Vec<u16> = self.mob_chains.visible(model as usize).to_vec();
        let boxed = |a: u16, b: u16| {
            if cast16 {
                ((a as i16 as i32) - (b as i16 as i32)).abs()
            } else {
                ((a.wrapping_sub(b)) as i16 as i32).abs()
            }
        };
        for &s in &roster {
            let c = &self.ent[s as usize];
            if c.id24 != id && boxed(ex, c.x) < pitch && boxed(ey, c.y) < pitch {
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
            // ⭐ THE CARPET IS A POOL RECORD IN RETAIL AND TAKES THE
            // SAME LIFE TEST AS EVERY OTHER TARGET. Each caller guards
            // the resolved pointer with `v7x->life_0x8 < 0 ||
            // v7x->byte[1] & 4` (HitFirebug_25610 EF:16360), and
            // nothing in that test knows or cares that the record is
            // the human's. This arm returned unconditionally, so a
            // creature kept attacking a corpse: mc2l3 t=7885, the tick
            // the player dies, three (5,19) firebugs above the carpet's
            // slot take retail's LABEL_92 — `actionIndex` 154 → 153
            // (attack → idle) and `actSpeed` → `minSpeed` 76 — where
            // the port held them all in the attack run.
            // `pdead` is the ctx channel that already existed for
            // exactly this ("our player lives outside the pool, so
            // followers get the state through the ctx"); the MC2
            // carpet dispatch refreshes it mid-walk so a creature
            // ABOVE the carpet reads the death on its own tick.
            if ctx.pdead {
                return None;
            }
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

    /// [`Self::mc2_target`] WITHOUT the liveness test — the position of
    /// `Entities_EA3E4[slot]` whatever state the record is in.
    ///
    /// ⭐⭐⭐ **THE LIFE TEST BELONGS TO THE CALLER, AND HALF THE
    /// CALLERS DO NOT MAKE IT.** `mc2_target`'s own doc already says
    /// each caller guards the resolved pointer with
    /// `life_0x8 < 0 || byte[1] & 4` — but it then bakes that guard
    /// into the RESOLVER, which silently applies it to the call sites
    /// that have no such test. m18's barrage machine has BOTH shapes,
    /// eight lines apart (`sub_250B0` EF:15995 / EF:16028):
    /// ```text
    ///   case 0: sub_254E0(a1x, Entities_EA3E4[a1x->word_0x96_150], 4u);   // NO guard
    ///   case 1: v5x = sub_1ED30(a1x, Entities_EA3E4[a1x->word_0x96_150]);
    ///           if (v5x <= Entities_EA3E4[0] || v5x->life_0x8 < 0
    ///               || v5x->struct_byte_0xc_12_15.byte[1] & 4) …      // guarded
    /// ```
    /// So a tank in the barrage's aim phase KEEPS TURNING toward a
    /// corpse, and only the phase that actually FIRES rejects it.
    /// Applying case 1's guard to case 0 is an INVENTED GUARD of the
    /// rounds 83-84 class — false exactly when the target dies.
    ///
    /// Retail makes no bounds test either (case 0 would happily index
    /// `Entities_EA3E4[0]`, the sentinel); the port keeps only the
    /// slice bound, which cannot change a shipped-data outcome.
    pub(crate) fn mc2_target_raw(&self, slot: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if slot == PLAYER_TARGET {
            // The carpet is an ordinary pool record in retail, so an
            // UNGUARDED caller reads a dead player's position too —
            // no `ctx.pdead` arm here on purpose.
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = slot as usize;
        if j >= self.ent.len() {
            return None;
        }
        let t = &self.ent[j];
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
                                        // ⚠ NOT a wrapping delta. Retail
                                        // sign-casts EACH position on its
                                        // own and subtracts in 32 bits
                                        // (:9473-74, no outer cast — its
                                        // `ent_overlap` siblings at :3714
                                        // and :3728 DO carry one and DO
                                        // wrap). Two creatures straddling
                                        // the 32768 (tile 128) line
                                        // therefore read ~65k apart and
                                        // the sidestep never fires there.
                                        && ((ex as i16 as i32) - (c.x as i16 as i32)).abs() < 256
                                        && ((ey as i16 as i32) - (c.y as i16 as i32)).abs() < 256
                                    {
                                        self.ent[i].f34 = Self::angle_between(c.x, c.y, ex, ey);
                                        break;
                                    }
                                }
                                // Catch-up: leader max + act (:9482) —
                                // both operands from the LEADER.
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
        // killing your own creature earns nothing.
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
            // The (10,1) corpse burst.
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
            // ⚠ YAW ONLY — the roll (`roll_0x20_32`, our `f34`) stays
            // at the allocator's 0. `TransformEntityToManaSphere_36BA0`
            // (EF:26903) writes `yaw_0x1C_28` and nothing else; the
            // pairing of the two IS a real MC2 idiom, but it belongs to
            // the cave fan (EF:26221-22 writes both), not here.
            self.ent[b].f30 = yaw;
            let d2 = self.mc2_rand(b);
            let speed = (d2 % 0x30 + 16) as i16;
            // Retail WRITES the roll onto the sphere's actSpeed@0x82
            // (EF:26907), not just into the velocity step — the
            // scattered spheres carry 16..63, the ctor's 32 is only
            // the un-scattered default (mc2l3 t=245-260: every
            // crush-scatter sphere read 32).
            self.ent[b].f126 = speed;
            // Velocity into the MC1 ball's dest fields (the shared
            // ball tick consumes them), fall arc into f46 — signed
            // TRUNCATING /8 like the C idiom at EF:26909 (NOT
            // div_euclid, which floors: off by one for deaths > 1024
            // above terrain), NO clamp (MC1 clamps ≥ 0; MC2 does not).
            let mut v = (0u16, 0u16, 0i16);
            Self::polar_step(&mut v, yaw, 0, speed);
            self.ent[b].dest_x = v.0;
            self.ent[b].dest_y = v.1;
            let zdiff = (z as i32) - (ground as i32);
            self.ent[b].f46 = ((1024 - zdiff) / 8) as i16;
        }
        // `TransformEntityToManaSphere_36BA0`'s tail zeroes ONLY
        // `playerEntityIndex_0x94` — the corpse KEEPS its mana
        // (mc2l3 t=245: crushed firebug corpses read 300 until the
        // reap; the invented f140 wipe dirtied every corpse pair).
        // No double-scatter: every caller raises 0x400 right after.
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
        // Per-model spawn ordinal → f63 (:33740) — de-syncs the herd
        // cadence (else every `f63 & N` gate runs in lockstep at 0).
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
        // very next line; unset f63 collapses f58 to the constant
        // period+4 (no stagger, degenerate archer wake).
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
        // Per-model spawn ordinal → f63 (:34062) — herd cadence.
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
        // ARROW-VOLLEY PROBE (`MGC_ARROW_VOLLEY_TRACE=1`): every (9,13) mint.
        if arrow_volley_trace_on() {
            eprintln!("DIG4 arrow slot={i} at ({x},{y},{z})");
        }
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
    pub(crate) fn goat_snd(&mut self, i: usize, modulus: u32) {
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
                    // the whole scan (falls to Scan B).
                    let mut target = self.mc2_class3_scan(i, ctx).filter(|&s| {
                        let wizard = s == PLAYER_TARGET || self.ent[s as usize].model65 <= 1;
                        wizard && self.mc2_wanted_live(s)
                    });
                    if target.is_none() {
                        // Scan B (:11811): the nearest model-9 creature
                        // on the TICK-TOP per-model roster, `d2 <=
                        // v_28²` — no cone, no self test, and no
                        // life/reap re-test (the roster settled those
                        // when it was built, the [`Gen::mc2_pack_scan`]
                        // law). ONE chain, ONE walk, then straight on
                        // to the pack Scan C.
                        //
                        // ⭐⭐⭐ **SETTLED IN THE SHIPPED
                        // `NETHERW.EXE`** (code file = VA + 0x24800),
                        // because the whole question is whether a
                        // second model is reachable. `sub_1FAA0`'s
                        // Scan B loads its chain head with a SINGLE
                        // CONSTANT displacement —
                        // `A1 A4 41 00 00` `mov eax,[0x41a4]` at file
                        // 0x44539 and `8B 80 27 96 00 00`
                        // `mov eax,[eax+0x9627]` at 0x44545 — where
                        // the pack Scan C two blocks below indexes the
                        // SAME array by the walker's own model:
                        // `0F BE 43 40` `movsx eax,byte [ebx+0x40]` at
                        // 0x445E8 and `8B B4 82 03 96 00 00`
                        // `mov esi,[edx+eax*4+0x9603]` at 0x445EF.
                        // 0x9627 − 0x9603 = 0x24 = 9*4, so the
                        // decompile's `bytearray_38403x[36/4]` is exact
                        // and index 9 is BAKED IN. Between 0x44539 and
                        // the `85 F6 / 74 3B` `test esi,esi; jz <ScanC>`
                        // at 0x4458C there is no loop back, no second
                        // chain load, no extinction test and no
                        // fallback of any kind.
                        //
                        // ⚠ THIS REVERSES `docs/DEVIATIONS.md`'s
                        // *mobs.rs::archer_brain (Scan B unnatural
                        // fallback)* entry, which invented an
                        // extinct-then-worms retry off a 2026-07-24
                        // retail REPLAY observation. The recording
                        // overrules it: mc2l4 pair 485→486 slot 229 is
                        // an archer with every skeleton dead — retail
                        // takes the pack (`f52 0 → 219`, `action 33 →
                        // 35`, two wander draws and nothing else) while
                        // the port locked worm slot 176 (`target96 176`,
                        // `f66/f67 = 5/3`), stopped (`speed 30 → 0`)
                        // and re-sprited to 206. Retail archers reach a
                        // worm through the RETALIATION arm (`jy >= 1`
                        // → `word_0x96_150 = word_0x26_38`, action 34,
                        // :11713-19), not through an acquire.
                        // `MGC_NO_MC2_ARCHER_SCANB_M9_ONLY=1` restores
                        // the fallback.
                        let e = &self.ent[i];
                        let row = &BEHAVIOR[e.row156 as usize];
                        let range = (row.v_28 as i32) * (row.v_28 as i32);
                        let (ex, ey) = (e.x, e.y);
                        if no_mc2_archer_scanb_m9_only() {
                            for model in [9u8, 3] {
                                let mut best: Option<(u16, i32)> = None;
                                let mut extinct = true;
                                for (j, c) in self.ent.iter().enumerate().skip(1) {
                                    if c.class64 == 5
                                        && c.model65 == model
                                        && c.act_life >= 0
                                        && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                                        && c.flags & 0x400 == 0
                                    {
                                        extinct = false;
                                        let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                        if d2 <= range && best.is_none_or(|(_, bd)| d2 < bd) {
                                            best = Some((j as u16, d2));
                                        }
                                    }
                                }
                                target = best.map(|(s, _)| s);
                                if !extinct {
                                    break;
                                }
                            }
                        } else {
                            let mut best: Option<(u16, i32)> = None;
                            for &s in self.mob_chains.visible(9) {
                                let c = &self.ent[s as usize];
                                let d2 = Self::dist2_sq(ex, ey, c.x, c.y);
                                if d2 <= range && best.is_none_or(|(_, bd)| d2 < bd) {
                                    best = Some((s, d2));
                                }
                            }
                            target = best.map(|(s, _)| s);
                        }
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
    pub(crate) fn archer_aim(&mut self, i: usize) {
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
        // accuracy stat in retail.
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
        // the projectile's own xtype/xsubtype filter (:3766-69) via
        // the shared `mc2_proj_filter`. The fire seams stamp the
        // TARGET's class+model onto the arrow (creature thunks
        // sub_1CCE0/sub_1CDA0; the archer combat state's own bytes
        // come from sub_20060 — sub_200F0's 3/-1 is the IDLE reset,
        // not the combat filter) — so a skeleton volley strikes the
        // FIRST archer along its path, not just the locked target:
        // stray arrows through a packed flock are what spread the
        // mc2:04 war. (APPROX: the original keeps scanning the ring
        // past a non-matching body in the same tick; we let the
        // arrow fly on and re-probe next tick.)
        //
        // ⚠⚠ THE ARROW PROBES ITS *CURRENT* POSITION, NOT THE STEP
        // ENDPOINT. `AddArcherArrow_672E0` builds `predictedAxis` with
        // `MoveEntity_57FA0` — which writes the SCRATCH axis, not the
        // record — and only then calls `sub_10780(a1x)`, with `a1x`
        // still standing where it started; the commit
        // (`CopyEntityPosition_57CF0`) happens afterwards and only on
        // the no-victim branch (EF:58870-83). This is the exact
        // opposite of the fireball, whose `sub_65C20` commits FIRST
        // and probes at the landed position (EF:63126-29) — the two
        // must not be unified.
        //
        // mc2l0 t=4104 is the row: the arrow at (49664, 52830, 583)
        // overlaps the carpet at (49500, 52705, 482) by ONE unit on x
        // (|Δ| 164 < 44 + 121) where its 384-unit step lands it at
        // (49686, 52453, 648), a clear miss. Retail hit and knocked
        // the player; the port's endpoint probe flew past.
        let cur = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let scanned = self.victim_scan_at(i, cur, ctx);
        let hit = self.mc2_proj_filter(i, scanned);
        let above_ground = self.ground_z(pos.0, pos.1) as i16 <= pos.2;
        if above_ground {
            let life = self.ent[i].act_life;
            self.ent[i].act_life = life - 1;
            if life != 0 && hit.is_none() {
                self.move_relink(i, pos.0, pos.1, pos.2);
                return;
            }
        }
        // The Rebound gate (`sub_68740` at EF:58892): a shielded
        // victim throws the arrow back (model 13 passes the engine's
        // whitelist unconditionally).
        if let Some(h) = hit
            && self.mc2_rebound_deflect(i, h, ctx, (0x2D, 22))
        {
            return;
        }
        // Impact (LABEL_10 / the entity branch): move to the victim,
        // area-write ch0 with f44, despawn.
        //
        // ⚠ THE LANDING IS THE VICTIM'S *RAISED* POSITION. Retail
        // brackets the copy in `sub_65580(v3x)` / `sub_655A0(v3x)`
        // (EF:58894-96) — the same z += f78 box-centre lift the
        // homing servo uses ([`Ent::aim_z`], MODEL 2 excepted) — so
        // the arrow lands at the victim's middle, not its feet, and
        // the ch0 area-write below is centred there. mc2l0 t=4104
        // measures it: the carpet sits at z 482 with f78 = 100 and
        // the recorded arrow lands at 582.
        match hit {
            Some(crate::mc1::combat::MailTarget::Pool(v)) => {
                let (vx, vy) = (self.ent[v].x, self.ent[v].y);
                let vz = self.ent[v].aim_z();
                self.move_relink(i, vx, vy, vz);
            }
            Some(crate::mc1::combat::MailTarget::Player) => {
                let (px, py, pz) = (ctx.px, ctx.py, ctx.pz);
                self.move_relink(
                    i,
                    px,
                    py,
                    pz.wrapping_add(crate::mc1::combat::PLAYER_HH as i16),
                );
            }
            // ⭐⭐ **THE EXPIRY PATH COMMITS NO MOVE.**
            // `AddArcherArrow_672E0`'s only `CopyEntityPosition` on
            // the no-victim branch is inside `if (life--)`, so the
            // tick the countdown runs out the arrow falls straight to
            // LABEL_10 and area-writes AT ITS UNMOVED POSITION. The
            // port stepped it one last time first. mc2l0 t=3989 slot
            // 147: retail freezes at the t=3988 (46561, 51321, 1740)
            // and the port flew on to (46257, 51123, 1864) — and with
            // it the ch0 blast landed a tile and a half downrange.
            None => {}
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
                // `KillTownie_23680` EF:14677-79 arms the KILLER's
                // wizard, whoever it is (`wanted_any_wizard`).
                let killer = self.ent[i].f38;
                if if crate::mc2::roster::wanted_any_wizard() {
                    self.mc2_is_wizard(killer)
                } else {
                    killer == PLAYER_TARGET
                } {
                    self.mc2_arm_wanted(killer);
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
                if if crate::mc2::roster::wanted_any_wizard() {
                    self.mc2_is_wizard(src)
                } else {
                    src == PLAYER_TARGET
                } {
                    self.mc2_arm_wanted(src);
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
                        // nearest dwelling.
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
                                // must not capture them.
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
    /// The class-2 tick column (the tree burn ladder + static decay)
    /// lives in `scenery.rs`.
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
        // The 2500..7500 life roll is DEAD VALUE in retail: AddTree
        // (:33443-50) rolls it, then `CopyMaxLifeToLife_49A20` right
        // after the map link resets life = maxLife (the pool-default
        // 300 — mc2l0 t=3169's disposition wave records every tree at
        // 300/300). The draw itself must still burn (the record's
        // rand stream feeds the jitters + sprite pick).
        let d = self.mc2_rand(i);
        self.ent[i].act_life = (d % 0x1388 + 2500) as i32;
        let jx = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        let jy = ((self.mc2_rand(i) & 0x3F) as i32 - 32) as i16;
        let (nx, ny) = (x.wrapping_add(jx as u16), y.wrapping_add(jy as u16));
        self.link(i, nx, ny, z);
        self.refill_life(i);
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
            // ⭐ `NewEvent_4A050`'s 100 IS `subSpellIndex_0x2A_42`
            // (Events.cpp:569), NOT `word_0x2C_44` — the memset above
            // it leaves @0x2C at 0 and `NewAdd0A00_4E320` (EF:35332-52)
            // never writes it. The port's allocator seeds that 100 into
            // `f44`, and `f44` IS this model's @0x2C home
            // (`port_ent_lanes_mc2`), so every newborn fire carried a
            // flicker of 100 where retail has 0 (mc2l22: 61 rows,
            // t=10914..10932, every (10,0) minted in the window).
            // Inert on the fire itself — its first acting tick
            // re-stamps the flicker before `sub_580E0` reads it — but
            // the same allocator seed is LIVE on every other model
            // whose f44 is @0x2C: the (2,7)/(2,8) falling props' fall
            // velocity, the (5,21) devil's jump impulse, the (5,22)
            // worm segment orbit, (10,16), (10,76)/(10,77) and the
            // class-15 tokens. Fixed here at the one site the corpus
            // measures; the sweep is the main session's.
            e.f44 = 0; // word_0x2C_44 — memset-clean at birth
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

    /// `NewAdd0A02_4E430` (EF:35375) — the (10,2) AMBIENT PUFF, the
    /// Speed spell's slipstream marker (`GetScroll_69DB0` EF:56253 is
    /// its only caller). Four writes and no more: maxLife/life 8,
    /// action 2, `dword_0x10_16` = 0, and the flag word masked to
    /// `byte[0] |= 1` / `byte[0] &= ~8` (untargetable) /
    /// `byte[2] |= 2` (sacrificable) — recorded flags 0x20001.
    ///
    /// It is deliberately UNLINKED (the ctor assigns `position_0x4C_76`
    /// instead of calling `AddEventToMap_57D70`) and SPRITELESS — the
    /// MC1 twin of the same puff behaves identically
    /// (docs/traces/mc1-class12-spell-tokens.md).
    pub(crate) fn mc2_spawn_speed_puff(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 2;
        e.tick70 = 2;
        e.max_life = 8;
        e.act_life = 8;
        e.f26 = 0;
        e.flags = (e.flags & !0x2_0009) | 0x2_0001;
        e.x = x;
        e.y = y;
        e.z = z;
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
            let t = crate::engine::features::tile(cx, cy);
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
                        let raw = crate::engine::features::tile((x >> 8) as u8, (y >> 8) as u8);
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
        // sub_585A0 (EF:22758): frame advance, CAPPED by the sprite's
        // own count.
        self.mc2_anim_step(i);
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
            // ⭐ `minSpeed_0x84_132 = w * h >> 2`, NOT remc2's `>> 4`
            // (EF:32769) — the same 4x slip MC1's dwelling cap carries
            // (`a_dwelling_carries_the_z_center_marker_sprite_and_area_cap`).
            // The corpus pins BOTH factors at once: mc2l0's village
            // row 37 reads `min_speed = 6` in the recording, and its
            // recorded extents `apitch = aroll = 1280` invert through
            // `((w << 8) + 1280) >> 1` to w = h = 5 — so the row is 5x5
            // un-halved and 25 >> 2 = 6, where 25 >> 4 would be 1.
            // (10x10 with `>> 4` also lands on 6, which is why this
            // needed the extents to arbitrate.)
            e.f128 = ((w as u16 * h as u16) >> 2) as i16; // minSpeed_132
            // SetShiftByCastle_49EC0 (:32882): the footprint quad.
            e.f78 = 0;
            e.f80 = ((w as u16) << 8).wrapping_add(1280) >> 1;
            e.f82 = ((h as u16) << 8).wrapping_add(1280) >> 1;
            e.f84 = 256;
            e.f71 = bldg as u8;
            e.act_life = 30;
            // ⭐ THE PRODUCTION RATE IS `subSpellIndex_0x2A_42` (f44),
            // NOT the mana word: `subSpellIndex = bldgprm[a2].word_0`
            // (EF:32793), and the construction finish parks the
            // building's LIFE at `1000 * subSpellIndex` (EF:27291).
            // f140 is retail's `mana_0x90_144`, which the same ctor
            // zeroes and then re-derives FROM the rate below. Parking
            // the rate in f140 gave the right life in fresh play by
            // coincidence and 1000x the MANA under import, where the
            // importer faithfully restores @0x2A → f44 and @0x90 →
            // f140 (mc2l1 t=888 slot 161: retail life 190,000,
            // port 0 — the imported building's mana was 0).
            e.f44 = prm.rate;
            // ⭐ THE DEGRADATION LINK IS PER-ENTITY, NOT A TABLE READ:
            // `fontTypeIndex_0x3D_61 = bldgprm[a2].byte_3` (:32795-98).
            // Two crush paths ZERO it on the live entity — the castle
            // level-up pre-clear `sub_11960` (EF:4410-11, called from
            // EF:61128) and the (10,67) quake grab `sub_3A090`
            // (EF:29335-36) — and `RemoveCastleStage_385C0` branches on
            // the ENTITY's copy (EF:28090), so a crushed building
            // demolishes for good instead of rebuilding as its
            // successor forever. Reading the static table here is what
            // let the 16 self-chaining ids resurrect under a levelling
            // castle. Second consumer: the type-2 objective latch
            // (EF:40771-79) — a castle-crushed building COMPLETES it.
            e.f46 = prm.chain as i16;
            // ⭐ `xtype_0x41_65 = 0` (EF:32796), the instruction
            // IMMEDIATELY BEFORE the mana zero below — shipped
            // `NETHERW.EXE` file 0x6E3BD (linear 0x49BBD):
            //   6e3bd  c6 43 41 00              mov BYTE PTR [ebx+0x41],0
            //   6e3c1  c7 83 90 00 00 00 00 ..  mov DWORD PTR [ebx+0x90],0
            // A building's xtype is its ON-DEATH DISPOSITION
            // (`if (event->xtype_0x41_65) sub_4A1E0(xtype, 1)`,
            // EF:28172-73), and only the THING spawn writes a real one
            // (`v3x->xtype_0x41_65 = entity->par2_16`, EF:33091 — the
            // `(10, 45)` arm of `World::spawn_postinit`). Every OTHER
            // path — the villager's build, the degradation-chain
            // successor — must leave it ZERO. Dropping this line left
            // `NewEvent_4A050`'s fresh-record default 0xFF standing, so
            // every villager-built shrine fired disposition 255 the
            // tick it died: a fire with no authored rows, but
            // `sub_4A1E0` opens with `sub_49F90` (EF:32966), whose
            // ghost loop frees EVERY reap-flagged record and whose
            // descending rebuild re-ranks the free stack to hand out
            // the LOWEST slot. On mc2l22 that is six phantom fires —
            // t=17855, 23311, 25785, 43381, 44414, 45582 — each a
            // segment head.
            if !no_mc2_building_xtype_zero() {
                e.f66 = 0;
            }
            // `mana_0x90_144 = 0` (EF:32797), then the productive kind
            // (`byte_2 & 8 == 0`) re-derives it off the rate at
            // EF:32808. Retail leaves `maxMana_0x8C_140` (f136)
            // untouched on a building — the uniform import restores it
            // as the dead 0 it is.
            e.f140 = 0;
            if prm.flags & 8 == 0 {
                e.f56 |= 2;
                e.f28 |= 2; // claim channel, writer-gate mirror
                e.f140 = (1000 * prm.rate as i32) >> 7;
            }
        }
        Some(i)
    }

    /// `sub_57390` (:39746): building placement clears its footprint
    /// tile — scenery entities SOFT-KILLED (`byte[1] |= 4` = our
    /// 0x400, :39765 → :40332-35; NEVER freed here — see the class-2
    /// arm below), creatures killed EXCEPT the protected models
    /// {6, 8, 10, 16, 22, 23, 27} (+ 25 while in action 200, retail's
    /// `actionIndex != -56`).
    ///
    /// ⚠ This sentence used to read "scenery entities removed", and
    /// that self-authored claim is exactly what hid the mc2l3 t=5614
    /// allocation defect for four sessions.
    ///
    /// Retail call sites: :13834 `sub_22490`, :27102 `sub_36FC0` (the
    /// instant-placement sibling, no ported caller — see below),
    /// :27322 `ApplyTerrainModification_37240`, :27827
    /// `AddTerrainMod0A_2A_37BC0`.
    ///
    /// `owner` is the caller's `id_0x1A_26` and the skip test is
    /// `victim.id24 != owner` — an OWNER compare, not a slot compare:
    /// a wizard's own creatures walk through their own construction
    /// unharmed. It degenerates to "skip the builder itself" for an
    /// unowned building, whose `id24` defaults to its own slot, which
    /// is why the slot-compare this used to do was indistinguishable
    /// on the village path — but NOT on the castle path, where the
    /// castle carries its wizard's id.
    ///
    /// The victim's killer/attacker pair (`word_0x24_36` /
    /// `word_0x26_38`) is stamped with the owner, so the kill credits
    /// the builder.
    pub(crate) fn mc2_building_clear_tile(&mut self, t: usize, owner: u16) {
        let mut j = self.map_entity[t] as usize;
        while j != 0 {
            let next = self.ent[j].next20 as usize;
            if self.ent[j].id24 != owner {
                match self.ent[j].class64 {
                    // :39763-66 — the class-2 arm is
                    // `DisableEntityDrawing04_57F10` (:40332-35), a
                    // one-line `byte[1] |= 4` = our 0x400: a SOFT
                    // KILL, not a free. The static stays a class-2
                    // GHOST, still tile-linked and OFF the free
                    // stack, until either the next tick-top reap or a
                    // mid-tick disposition fire sweeps it through
                    // `mc2_rebuild_free`'s ghost loop — which then
                    // hands the slots back ASCENDING. Freeing here
                    // put them on the stack immediately and every
                    // later allocation in the same tick rotated:
                    // mc2l3 t=5614, the (10,45)'s footprint clear
                    // ghosts the (2,6)s at 92/96/97, and retail's
                    // dis-9 payload takes 92 for its (11,22)
                    // kill-watch where the port had already spent 92
                    // on a mana sphere and 96 on a projectile.
                    // MC1's twin `build_footprint_kill`
                    // (features.rs, :51747) has carried the soft kill
                    // since mc1l0 t=3855.
                    2 => self.ent[j].flags |= 0x400,
                    5 => {
                        let m = self.ent[j].model65;
                        let protected = matches!(m, 6 | 8 | 10 | 16 | 22 | 23 | 27)
                            || (m == 25 && self.ent[j].tick70 == 200);
                        if !protected {
                            // ⭐ `sub_57390` EF:39801-03 (shipped
                            // `NETHERW.EXE` 0x7BC1F-0x7BC2D):
                            // `life = -1; word_0x24_36 = a2;
                            //  word_0x26_38 = a2;`. The port stamped
                            // `f36`/`f38` — one home SHORT. `f38` IS
                            // `word_0x24_36`, but `word_0x26_38` is
                            // `f40` (the mapping is fixed by
                            // `mc2_state_head`'s own `f38 = f40`,
                            // mirroring retail's `word_0x24_36 =
                            // word_0x26_38` at EF:16137), and `f36` is
                            // `word_0x22_34` — an m27-only lane this
                            // arm can never reach, since m27 is
                            // protected. So the ATTACKER half of the
                            // killer pair was never written and
                            // everything reading it off a corpse read
                            // 0 — including `sub_28CE0`'s split, which
                            // seeds each mini's `word_0x24_36` from
                            // the parent's `word_0x26_38` (EF:19168),
                            // so the minis thought nobody killed their
                            // parent and wandered instead of hunting.
                            self.ent[j].act_life = -1;
                            self.ent[j].f38 = owner;
                            if crate::mc2::roster::clear_tile_attacker_stamp() {
                                self.ent[j].f40 = owner;
                            } else {
                                self.ent[j].f36 = owner;
                            }
                        }
                    }
                    _ => {}
                }
            }
            j = next;
        }
    }

    /// `IsNextEvent0A_2A_37740` (EF:27437) — is a live (10,42) BUILD
    /// PAINTER's 2-D box overlapping this record? `sub_37240`'s whole
    /// body, `life_0x8--` included, sits inside `if (!…)` (EF:27237),
    /// so a build whose plot a painter is working PAUSES its
    /// countdown: retail raises them one at a time.
    ///
    /// ⭐⭐ THE WALK IS THE TICK-TOP ROSTER `dword_38535`
    /// ([`Gen::paint_chain`]), and that is the load-bearing half. The
    /// painter that freezes a plot is minted by the build that just
    /// COMPLETED — galore t=6523: the (10,45) at slot 29 finishes
    /// (life 1 → 20000, action 51 → 52) and borns the painter at slot
    /// 194 — and slot 29 dispatches BELOW the four huts at 36/40/41/48
    /// that share its plot. On a live-pool scan those four see the
    /// newborn immediately and stall a tick early; on retail's chain
    /// they cannot see it at all this frame, so they advance at 6523
    /// (life 7 → 6, matching the capture) and stall from 6524 on.
    /// *That is why implementing the gate alone made galore WORSE
    /// (6523 → 6522) in 2026-08-25e — the gate was right and the
    /// roster was the missing half.* A/B measured here, not inherited:
    /// swap this walk for `1..ent.len()` and galore reads **6522**,
    /// reproducing that session's regression to the tick; on the
    /// chain it reads **7281**. The gate alone is worth −1, the gate
    /// on the right roster +758.
    ///
    /// The class/model test is retail's own (EF:27442) and is kept:
    /// the chain also carries 67/78 and the class-11 pair, and the
    /// walk skips past them rather than stopping.
    pub(crate) fn mc2_build_plot_frozen(&self, i: usize) -> bool {
        (0..self.paint_chain.visible_len()).any(|c| {
            let j = self.paint_chain.list[c] as usize;
            let p = &self.ent[j];
            p.class64 == 10 && p.model65 == 42 && self.mc2_overlap_xy(i, j)
        })
    }

    /// ⭐⭐⭐ `sub_68940` (EF:55315; shipped EXE 0x8D140) — THE MAGIC
    /// MINE IS A HOMING BEACON FOR ITS OWNER'S OWN SPELLS, and it runs
    /// BEFORE the generic acquisition on every class-9 one-shot lock:
    /// `if (sub_68940(a1x) || sub_67CB0(a1x))` (EF:62907 `sub_65820`,
    /// EF:63092 `sub_65C20`, EF:63450 `sub_662E0`, EF:63589
    /// `sub_66610`). A wizard plants a (10,78) and every qualifying
    /// bolt he casts inside a ±0xAA yaw cone bends onto it — the whole
    /// point of the Magic Mine spell, and the reason the port's own
    /// `mc2/proj.rs` header lists this pair as OPEN ("needs the (10,78)
    /// beacon column"). That column landed with `mc2_spawn_magic_mine`
    /// / `mc2_mine_tick`; this is its consumer.
    ///
    /// The gates, read off the shipped EXE statement for statement:
    /// - model ∈ {1,2,3,4,5,8,9,0xC,0x16,0x17,0x1A,0x1C,0x1E}
    ///   (0x8D15B-0x8D1AC, the same ladder `sub_68AC0` repeats);
    /// - `a1x->id_0x1A_26 != 0` and `Entities[owner]->class == 3`
    ///   (0x8D1B2-0x8D1D2) — a WIZARD-owned shot only;
    /// - walk the tick-top roster `dword_38535` (`0x8D1DE: mov
    ///   0x9687(%ebx),%ebx` = [`Gen::paint_chain`]) for `model == 78`;
    /// - `mine->word_0x32_50 == owner` (0x8D1F9) — HIS OWN mine (the
    ///   port already stamps that lane: mc2l6-rsg t=13545 slot 178
    ///   reads `f32` 343 on BOTH sides);
    /// - `mine->word_0x36_54 == -1` (0x8D201) — armed. The port has no
    ///   home for @0x36 on class 10 (`f56` is @0x38 there) and neither
    ///   half of retail's own machine ever leaves −1: `sub_50840` seeds
    ///   it and `sub_3A8B0` PARKS at case 1 because case 1 advances only
    ///   when it is not −1 (`mc2/effects.rs`); `sub_68AC0` is the only
    ///   consumer and is itself unported. Always-armed here, by that
    ///   citation.
    /// - `dist3d(mine, OWNER) < BEHAVIOR[owner.row].v_28` — the range is
    ///   measured from the WIZARD, not from the bolt (0x8D218-0x8D235
    ///   pushes `owner+0x4C` then `mine+0x4C`), and it is the owner's
    ///   own behaviour row, exactly [`Gen::mc2_owner_lock_range`];
    /// - nearest mine wins (`v8` starts 0x10000, strict `<`);
    /// - `sub_582B0(bolt.yaw, angle(bolt, mine)) < 0xAA` (0x8D261) —
    ///   a ±170-unit cone, half the map's turn but far WIDER than
    ///   `sub_67CB0`'s 0x71, which is why the beacon wins locks the
    ///   generic scan cannot even see.
    ///
    /// On a hit: `word_0x96_150 = mine` and `sub_655C0` (the desired
    /// aim at the mine's `aim_z`) — and NOTHING else: no `sub_68BD0`
    /// dodge alert, no `sub_5EF70` wizard alarm.
    ///
    /// mc2l6-rsg t=13545 is the corpus row: the human's charged meteor
    /// leaves at his own cast attitude (yaw 335, pitch 40 — the bolt's
    /// `axis_0x9A_154x` ray is 10240 units along exactly that), and
    /// retail's record shows it flying at yaw **183** / pitch **2017**
    /// with `word_0x96_150` = **178**, a (10,78) at (17213, 14243) —
    /// bearing 184 from the muzzle, 151 units off the cast yaw, i.e.
    /// outside `sub_67CB0`'s 0x71 cone and inside this one.
    #[allow(dead_code)] // wiring lives in `mc2/proj.rs` — see DIG G2 report
    pub(crate) fn mc2_mine_beacon(&mut self, i: usize, ctx: &MobCtx) -> bool {
        if no_mine_beacon() {
            return false;
        }
        let (model, own, px, py, pz, yaw) = {
            let e = &self.ent[i];
            (e.model65, e.id24, e.x, e.y, e.z, e.f30)
        };
        if !matches!(
            model,
            1 | 2 | 3 | 4 | 5 | 8 | 9 | 0x0C | 0x16 | 0x17 | 0x1A | 0x1C | 0x1E
        ) {
            return false;
        }
        if own == 0 {
            return false;
        }
        // `Entities[owner]->class_0x3F_63 == 3`. The out-of-pool human
        // IS a class-3 wizard record in retail, so he qualifies and his
        // raw pose is the measuring point (`sub_583F0` reads
        // `position_0x4C_76` with no `sub_65580` raise).
        let (ox, oy, oz) = if own == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz)
        } else {
            match self.ent.get(own as usize) {
                Some(o) if o.class64 == 3 => (o.x, o.y, o.z),
                _ => return false,
            }
        };
        let range = self.mc2_owner_lock_range(own);
        let mut best: Option<u16> = None;
        let mut bestd: i64 = 0x10000;
        for c in 0..self.paint_chain.visible_len() {
            let v = self.paint_chain.list[c] as usize;
            let e = &self.ent[v];
            // ⚠ THE ARMED GATE NOW HAS A HOME. Retail's beacon scan
            // requires `ix->word_0x36_54 == -1` (EF:55366) — a mine
            // that has already swallowed a spell bends nothing. The
            // old doc-comment's "always armed here, by that citation"
            // premise is obsolete: `sub_68AC0` writes the gate and the
            // importer now carries it (armed ⟺ `f36 == 0`).
            if e.model65 != 78 || e.f52 != own || e.f36 != 0 {
                continue;
            }
            let dz = (e.z as i64) - (oz as i64);
            let d = Self::isqrt(
                (Self::dist2_sq(ox, oy, e.x, e.y) as i64 + dz * dz).min(u32::MAX as i64) as u32,
            ) as i64;
            if d >= range || d >= bestd {
                continue;
            }
            if Self::arc_err(yaw, Self::angle_between(px, py, e.x, e.y)) >= 0xAA {
                continue;
            }
            best = Some(v as u16);
            bestd = d;
        }
        let Some(t) = best else {
            return false;
        };
        let (tx, ty, tz) = {
            let e = &self.ent[t as usize];
            (e.x, e.y, e.aim_z())
        };
        let dh = Self::isqrt(Self::dist2_sq(px, py, tx, ty) as u32) as i32;
        let ty2 = Self::angle_between(px, py, tx, ty);
        let tp = Self::pitch_toward(pz, tz, dh);
        let e = &mut self.ent[i];
        e.f146 = t;
        e.f34 = ty2;
        e.f36 = tp;
        true
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
    /// `human` = (the carpet's pose AS OF THIS WALK SLOT, carpet
    /// slot, alive-at-tick-top) for the `sub_377A0` completion pass
    /// — None on the load-time carousel (an APPROX like the
    /// concurrent raise: retail would mint for a wizard overlapping
    /// at load; the recording's seed state already carries those).
    pub(crate) fn mc2_building_tick(
        &mut self,
        i: usize,
        human: Option<((u16, u16, i16), u16, bool, u8)>,
    ) -> bool {
        // EF:27234-36 — the opener, ahead of everything including the
        // `IsNextEvent0A_2A_37740` carousel: on the FIRST countdown
        // tick the site swaps its sprite-derived quad for the
        // BUILDING's own AABB half-extents.
        // [`no_mc2_build_site_footprint_box`].
        if !no_mc2_build_site_footprint_box() && self.ent[i].act_life == self.ent[i].max_life as i32
        {
            let row = self.ent[i].f71;
            self.mc2_castle_box_quad(i, row);
        }
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
            // `life_0x8 = 1000 * subSpellIndex_0x2A_42` (EF:27291) —
            // the production rate, f44 (see `mc2_spawn_building`).
            e.act_life = 1000 * e.f44 as i32;
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
            // `sub_377A0` (:27304, the action-51 completion tail):
            // EVERY member of the class-3 live list `dword_38519`
            // whose box overlaps the finished building gets the
            // CASTLE re-paint `sub_5FBD0` — a (10,42) painter minted
            // ON THAT RECORD (see `mc2_spawn_wizard_painter`). The
            // list is NOT a castle list: retail rebuilds it at the
            // tick top (EF:39975) as an ASCENDING pool walk over
            // slots 1..1000 keeping every class-3 record with
            // `life >= 0`, model unchecked — so the human carpet
            // (3,0) and the rival wizards (3,1) are on it beside the
            // castles (3,2), and a building finishing under a wizard
            // "re-paints its castle" with the wizard's `@0x10` as
            // the rung (the registered out-of-bounds row on a human
            // who has died: `@0x10` = the 1200 respawn timer). The
            // overlap is the 2-D `CompareAxisWithShift_10750` —
            // extents sum, no z.
            //
            // ⭐ ORDER IS THE LAW: painters pop the free stack in
            // list order, so the owner/slot pairing is the ascending
            // slot order WITH THE HUMAN AT ITS OWN SLOT. Testing the
            // human after the pool walk permuted the owners wherever
            // several members overlapped the same building
            // (mc2l22 t=12,969 a 3-cycle, t=15,345 / 15,836 swaps;
            // rsg t=39,231 a swap — human 343 before rival 424).
            // The human's record is read AS OF THE BUILDING'S WALK
            // SLOT: pre-move for a building below the carpet slot
            // (all of mc2l0's), post-move above it (all of rsg's:
            // 616 / 694 / 749 over carpet 343) — the caller hands in
            // the mid-walk `ctx` pose. A dead human (`life < 0` at
            // the tick top) is not on the list.
            let (bx, by, bw, bh) = {
                let e = &self.ent[i];
                (e.x, e.y, e.f80 as i32, e.f82 as i32)
            };
            let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
            // Gated on `human`: the import-side pad reconstruct
            // (pads.rs, the only human:None caller) replays a build
            // retail already finished — its sub_377A0 pass must not
            // re-mint the (10,42) painters or re-stamp f46 (mc2l30
            // t=402: a phantom painter off the free stack every
            // pair, castle f2c 0→4; mc2l0-sg t=7283: seven at once).
            if let Some((pose, slot, alive, human_row)) = human {
                let pw = (self.mc2_params_ext(44).0 / 2) as i32;
                let slot = slot as usize;
                // A native world has no pooled carpet (slot 0):
                // retail's human sits SOMEWHERE in the walk, and
                // with no slot to place it the human tests after
                // the pool — the pre-fix order, kept for native
                // play (the painter is a row-0 no-op there anyway).
                let human_in_walk = slot != 0;
                let human_hit = alive && wd(pose.0, bx) < bw + pw && wd(pose.1, by) < bh + pw;
                let n = self.ent.len().max(slot + 1);
                for w in 1..n {
                    if human_in_walk && w == slot {
                        // The out-of-pool carpet, tested where its
                        // pool slot sits in retail's walk. Its spare
                        // axis @0x9A is unwritten on the wizard body
                        // — (0,0,0).
                        //
                        // ⭐ THE ROW IS THE HUMAN'S `dword_0x10_16`,
                        // NOT 0. `sub_377A0` stamps
                        // `byte_0x46_70 = wizard->dword_0x10_16` for
                        // EVERY class-3 member it re-paints, the human
                        // included — and on the human that word is the
                        // DEATH RESPAWN TIMER (0 before the first
                        // death, 1200 after; the human arm never counts
                        // it down). The port hard-coded 0 because the
                        // out-of-pool carpet had nowhere to keep it;
                        // [`Player::mc2_respawn_timer`] is that home
                        // now, and the caller hands the low byte in.
                        // mc2l15 t=24152 slot 719: `b46` retail 176
                        // (= 1200 & 0xFF) / port 0. PLAYER-RULED
                        // 2026-09-06.
                        // ⚠ The EXTENTS still diverge — row 176 is off
                        // the end of a 77-row table on both sides and
                        // retail reads heap residue there (the
                        // registered `mc2-painter-oob-build-row-*`
                        // deviation). This fixes the INDEX, not the
                        // residue it lands in.
                        if human_hit {
                            self.mc2_spawn_wizard_painter(
                                (0, 0, 0),
                                human_row,
                                crate::mc1::mobs::PLAYER_TARGET,
                                slot as u16,
                            );
                        }
                        continue;
                    }
                    if w >= self.ent.len() {
                        continue;
                    }
                    let e = &self.ent[w];
                    if e.class64 != 3 || e.flags & 0x400 != 0 {
                        continue;
                    }
                    if wd(e.x, bx) < bw + e.f80 as i32 && wd(e.y, by) < bh + e.f82 as i32 {
                        let (dest, row, own) = (
                            (e.dest_x, e.dest_y, e.site_z),
                            e.f26.clamp(0, 7) as u8,
                            e.id24,
                        );
                        if self
                            .mc2_spawn_wizard_painter(dest, row, own, w as u16)
                            .is_some()
                        {
                            self.ent[w].f46 = 4;
                        }
                    }
                }
                if !human_in_walk && human_hit {
                    self.mc2_spawn_wizard_painter(
                        (0, 0, 0),
                        human_row,
                        crate::mc1::mobs::PLAYER_TARGET,
                        0,
                    );
                }
            }
            return true;
        }

        // First countdown tick: the footprint kill (:27310-28).
        if self.ent[i].max_life as i32 - 1 == life {
            for dy in 0..h {
                for dx in 0..w {
                    let t = crate::engine::features::tile(
                        tlx.wrapping_add(dx as u8),
                        tly.wrapping_add(dy as u8),
                    );
                    self.mc2_building_clear_tile(t, self.ent[i].id24);
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
                let t = crate::engine::features::tile(
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
                    let t = crate::engine::features::tile(
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
    /// `sub_36FC0` (EF:27031-27171) — the INSTANT build-row stamp, the
    /// [`Gen::mc2_building_tick`] lerp collapsed to one pass: owner
    /// clear (`sub_57390`), pad cells written to `pad + (z >> 5)`
    /// outright (no `/ life` step), the angle-nibble seed + retile on
    /// a cleared cell, the cave headroom bubble asserted at once, then
    /// every paint code through `sub_45DC0` with the column argument
    /// **0** (`char v14 = 0`, :27153 — the lerp passes `dx`). Retail's
    /// callers run it on the scratch slot 0 with a copied position:
    /// the authored starting castle's `j`-loop (EF:43787-43800, one
    /// call per row `0..castle_level`) and the `sub_5C950` stage
    /// machinery. Round 110: the port used to settle the authored
    /// castle through the (10,42) repaint painter at row `level - 1`,
    /// which paints NOTHING at castle level 1 (row 0 is a real 4×4 row
    /// here) and 332 of level 5's 820 cells — mc2l22's four rival
    /// castles and mc2l4/mc2l6's level-1 stumps were absent from the
    /// generated terrain while the takes certified.
    pub(crate) fn mc2_stamp_build_row_instant(&mut self, pos: (u16, u16, i16), row: u8, owner: u16) {
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
        if std::env::var_os("MGC_SCULPT_TRACE").is_some() {
            let pads = cells.chunks(2).filter(|c| c[1] != 0xff).count();
            let codes = cells.chunks(2).filter(|c| c[0] != 0xff).count();
            eprintln!("SCULPT castle-row-stamp row={row} w={w} h={h} pads={pads} codes={codes} at ({},{})", pos.0 >> 8, pos.1 >> 8);
        }
        let cx = ((pos.0.wrapping_add(128)) >> 8) as u8;
        let cy = ((pos.1.wrapping_add(128)) >> 8) as u8;
        let tlx = cx.wrapping_sub((w / 2) as u8);
        let tly = cy.wrapping_sub((h / 2) as u8);
        let base = (pos.2 >> 5) as i32; // v25
        let cave_raise = self.is_cave()
            && self
                .assets
                .bldgprm
                .get(row as usize)
                .is_none_or(|b| b.flags & 4 == 0);
        for dy in 0..h {
            for dx in 0..w {
                let t = crate::engine::features::tile(
                    tlx.wrapping_add(dx as u8),
                    tly.wrapping_add(dy as u8),
                );
                self.mc2_building_clear_tile(t, owner);
                let pad = cells[2 * (dy * w + dx) + 1];
                if pad != 0xff {
                    self.t.height[t] = (pad as i32 + base) as u8;
                    if self.t.angle[t] & 7 == 0 {
                        self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        let (cx2, cy2) = (tlx.wrapping_add(dx as u8), tly.wrapping_add(dy as u8));
                        self.mc2_retile_region(cx2, cy2, cx2, cy2);
                    }
                }
                if cave_raise {
                    let bubble = ((self.t.height[t] as i32).max(base) + 80).min(255);
                    if bubble > self.t.ceiling[t] as i32 {
                        self.t.ceiling[t] = bubble as u8;
                    }
                    self.cave_seal_fixup(t);
                }
            }
        }
        for dy in 0..h {
            for dx in 0..w {
                let code = cells[2 * (dy * w + dx)];
                if code == 0xff {
                    continue;
                }
                self.mc2_paint_cell(
                    0,
                    tlx.wrapping_add(dx as u8),
                    tly.wrapping_add(dy as u8),
                    code,
                );
            }
        }
    }

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
    /// (claimed buildings fly the flag), and the per-tick terrain
    /// z-snap.
    ///
    /// APPROX register: the mana-sphere production roll (:28040-58,
    /// full enterable houses) and SetMaxDistance_5C8D0 are OPEN
    /// (economy track). The claimed sprite-row colorize
    /// (`word_0x5A_90 += color`, :28039) is ported: flag row 177 +
    /// owner color, index shifted AFTER the extent derivation off the
    /// base row (there is no team-tint stage in the billboard pass —
    /// the earlier note claiming one was wrong, and bare 177 flew the
    /// human's flag on rival-claimed houses).
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
        // The claim intake (:28016-42): possess ch1 → new owner,
        // chime 4 at the claimer, flag bit 0 cleared (the flag
        // FLIES), sprite re-set. Claimability is the DELIVERY's
        // f56-bit-1 gate — stone templates (bldgprm flags & 8) never
        // set it, so they can never receive this mail. The mail
        // AMOUNT is retail's `dword_0x64_100` force flag: a FORCED
        // claim (the tier-2 (10,70) pulse) steals unconditionally and
        // sets the claim lock (`byte[2] |= 0x20`, EF:28026); a weak
        // claim bounces off a locked building (EF:28031).
        if self.ent[i].mail[1].1 != 0 {
            let (force, src) = self.ent[i].mail[1];
            self.ent[i].mail[1] = (0, 0);
            if src != self.ent[i].f144 && (force != 0 || self.ent[i].flags & F_CLAIM_LOCK == 0) {
                self.ent[i].f144 = src;
                self.ent[i].flags &= !1;
                if force != 0 {
                    self.ent[i].flags |= F_CLAIM_LOCK;
                }
                if src == crate::mc1::mobs::PLAYER_TARGET {
                    self.snd_player(4);
                }
                self.mc2_set_sprite(i, 177);
                // Owner recolor (EF:28035-40; castle-builder trace
                // `+90 += TransformPlayerColorIndex`): the flag INDEX
                // shifts to the owner's ART row AFTER the extent
                // derivation off the base row — flag family 177 +
                // COLOR_ART[slot], same as the rival castle flag. Bare
                // 177 flew the HUMAN's flag on rival-claimed houses.
                let team = self.owner_team(src).unwrap_or(0);
                self.ent[i].type86 = 177 + crate::mc2::color_art(team) as u16;
            }
        }
        // ⭐⭐⭐ THE PERIODIC POPULATION SPAWN — AN ABSENCE IN AN
        // ENUMERATED CALL-SITE LIST. `GetRandManaSphere_38270` has
        // exactly TWO call sites in retail: the building COLLAPSE
        // (EF:28121, ported at [`World::mc2_house_collapse`]) and the
        // tail of THIS handler (EF:28043-58), which was missing — so a
        // full enterable house never shed a settler in the port.
        // `grep mc2_rand_occupant` returned one caller, not two.
        //
        // Shipped `NETHERW.EXE`, `AddHouse0A_2D_38330` (runtime 0x38330
        // = file 0x5CB30, prologue `53 56 57 55 89 e5`):
        //   0x5CCF2  f6 43 3e 1f   test BYTE PTR [ebx+0x3e],0x1f   f63 % 32
        //   0x5CCF6  0f 85 89..    jne  0x5cd85                    (skip)
        //   0x5CCFC  0f be 43 46   movsx eax,BYTE PTR [ebx+0x46]   bldgprm row
        //   0x5CD00  f6 04 85 c2 93 00 00 01  test [eax*4+0x93c2],0x1  enterable
        //   0x5CD0E  66 8b 8b 84.. mov cx,WORD PTR [ebx+0x84]      minSpeed
        //   0x5CD15  66 83 f9 05 / 7e 6a   cmp cx,5 / jle           minSpeed > 5
        //   0x5CD1E  3b 43 10 / 75 62      cmp eax,[ebx+0x10] / jne minSpeed == @0x10
        //   0x5CD23  66 69 43 14 a1 24 / 05 df 24 00 00            ONE entity-LCG draw
        //   0x5CD41  f7 f1         div ecx                          rand % minSpeed
        //   0x5CD4A  89 c1 c1 f9 04 / 29 c8 / 83 e8 02              ms - (ms>>4) - 2
        //   0x5CD54  39 c2 / 7e 2d cmp edx,eax / jle                spawn on `>`
        //   0x5CD58  predictedAxis = position; `mov ax,[ebx+0x54]`   x += apitch
        //   0x5CD7D  e8 ee fc ff ff  call 0x5ca70 = GetRandManaSphere_38270
        // and the callee at file 0x5CA70 draws a SECOND time and routes
        // `% 12` through the jump table located BY CONTENT at file
        // 0x5CA40 (arm base 0x282A8, `file = addr + 0x34800`):
        // 0,1 -> (5,4) action 0x21=33 · 2,3 -> (5,14) 0x71=113 ·
        // 4..8 -> (5,13) 0x69=105 · 9..11 -> (5,12) 0x61=97 — exactly
        // [`Gen::mc2_rand_occupant`]'s existing arms.
        //
        // WITNESS (mc2l22, building slot 744, `min_speed` 6,
        // `scratch10` = @0x10 = 6, `b46` 19, `apitch` 194): retail's
        // `f63` wraps 255 -> 0 at t=61429 and its `rand` steps
        // 39154 -> 4240 across the single tick 61429->61430 — that is
        // EXACTLY TWO 9377/9439 draws (39154 -> 23825 -> 4240), the gate
        // draw and the occupant draw. 23825 % 6 = 5 > 6-0-2 = 4 so the
        // gate opens, and 4240 % 12 = 4 selects the (5,13) villager —
        // which is precisely the `(5,13)` retail mints at slot 897 with
        // `action45 105`. The port drew NOTHING and minted NOTHING.
        if !no_mc2_house_pop() && self.ent[i].f63 & 0x1F == 0 {
            let bldg = self.ent[i].f71 as usize;
            let enterable = self
                .assets
                .bldgprm
                .get(bldg)
                .is_some_and(|b| b.flags & 1 != 0);
            if enterable {
                // `cmp cx,5 / jle` then `movsx eax,cx / cmp eax,[ebx+0x10]`:
                // a full house only.
                let ms = self.ent[i].f128 as i32;
                if ms > 5 && ms == self.ent[i].f26 as i32 {
                    let r = self.mc2_rand(i);
                    // `div ecx` is UNSIGNED on the zero-extended word;
                    // `sar ecx,4` and the compare are signed, and ms > 5.
                    if (r % ms as u32) as i32 > ms - (ms >> 4) - 2 {
                        let (x, y, z, off) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z, e.f80)
                        };
                        self.mc2_rand_occupant(i, x.wrapping_add(off), y, z);
                    }
                }
            }
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
    }

    // ---- dispatch + awake --------------------------------------------------

    /// A/B toggle for THE HIVE SPLIT'S ID INHERITANCE (dig 98-Q24).
    /// `MGC_NO_M9_SPLIT_INHERITS_ID=1` restores the pre-2026-09-04
    /// behaviour, where the `(5,9)` minted by a hive's consume sweep
    /// kept `NewEvent_4A050`'s own-slot `id_0x1A_26` seed instead of
    /// its parent's id.
    ///
    /// THE LAW — and it is a SPLIT IN A SIBLING PAIR, verified on both
    /// arms in the shipped `NETHERW.EXE`:
    /// - `sub_20940` (the GROUNDED hive, EF:12409-11; file
    ///   **0x45428-0x4543B**: `call 0x6e990` =
    ///   `IfSubtypeCallCreatingManaSphere_4A190(&pos, 5, 9)` ·
    ///   `mov 0x1a(%ebx),%dx` · `mov %dx,0x1a(%eax)`) copies the
    ///   parent's id **UNCONDITIONALLY**.
    /// - `sub_203D0` (the WALKING hive, EF:12213-16; file
    ///   **0x450F2-0x45117**: `call 0x6e990` ·
    ///   `movswl 0x1a(%ebx),%edx` · `mov 0x1a3e4(,%edx,4),%edx` ·
    ///   `cmpb $0x3,0x3f(%edx)` · `mov 0x1a(%ebx),%dx` ·
    ///   `mov %dx,0x1a(%eax)`) copies it **only when
    ///   `Entities[parent->id]->class_0x3F_63 == 3`** — i.e. only a
    ///   hive that already belongs to a wizard/castle/balloon passes
    ///   the badge on; a wild hive's walking split does not.
    ///
    /// The port folded both retail bodies onto one `m9_consume_scan`
    /// helper and carried NEITHER copy, so every hive split minted a
    /// stranger. The id is what `sub_10C80`'s damage sweep bills and
    /// what `m9_cone_scan`'s `id != my_id` gate excuses, so a split
    /// child with the wrong badge retaliates at the wrong slot.
    pub(crate) fn m9_split_inherits_id_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M9_SPLIT_INHERITS_ID").is_none())
    }

    /// A/B toggle for the CONTROLLED-SLOT SNAP (dig 98-Q22).
    /// `MGC_NO_SV_CONTROLLED_SLOT_SNAP=1` restores the pre-2026-09-04
    /// behaviour, where a StageVar2 13/16/17 creature that left the
    /// `8*model+7` slot never came back to it and only the ALLIANCE
    /// arm (14) carried a partial paraphrase.
    pub(crate) fn sv_controlled_slot_snap_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_SV_CONTROLLED_SLOT_SNAP").is_none())
    }

    /// ⭐⭐⭐ `sub_12500`'s CONTROLLED-SLOT SNAP — a TICK-TOP PRE-PASS
    /// THE PORT NEVER RAN.
    ///
    /// `UpdateEntities_57730` opens its unpaused half with
    /// `sub_12780()` and then walks **all 29 per-model class-5 roster
    /// chains**, calling `sub_12500` on every member whose
    /// `StageVar1_0x48_72` or `StageVar2_0x49_73` is non-zero
    /// (EF:40095-40101). Byte-for-byte in the shipped `NETHERW.EXE`
    /// at file `0x7C1A9-0x7C1E4`:
    /// `call 0x36f80` (sub_12780) · `cmpb $0x0,0x48(%ebx)` /
    /// `cmpb $0x0,0x49(%ebx)` · `call 0x36d00` (sub_12500) ·
    /// `mov (%ebx),%ebx` (next_0) · `cmp $0x1d,%esi` (29 chains).
    ///
    /// `sub_12500` itself opens with an outer phase gate and a
    /// `StageVar2 - 1 <= 0x10` jump table, and the arm shared by
    /// **13 (Summon Army), 14 (Alliance), 16 and 17 (the doomsday
    /// pyramid's summon pair)** is four instructions
    /// (`NETHERW.EXE` 0x36d8b-0x36da2):
    /// ```text
    ///   36d0c  mov  0x45(%ebx),%al      ; actionIndex
    ///   36d14  and  $0x7,%al
    ///   36d1f  cmp  $0x4,%si  / jb      ; phase < 4  -> proceed
    ///   36d25  cmp  $0x5,%si  / jbe     ; phase 4..5 -> return
    ///   36d8b  cmp  $0x2,%si  / je      ; attack     -> return
    ///   36d91  cmp  $0x6,%si  / je      ; flee       -> return
    ///   36d97  mov  0x40(%ebx),%al      ; model
    ///   36d9a  shl  $0x3,%al            ; *8
    ///   36d9d  add  $0x7,%al            ; +7
    ///   36d9f  mov  %al,0x45(%ebx)      ; actionIndex = 8*model + 7
    /// ```
    /// i.e. **a controlled creature is dragged back into its
    /// controlled slot at the top of every frame from phases 0, 1, 3
    /// and 7 — every state except attack (2), flee (6) and the
    /// prekill/kill pair (4/5).** That is how a Summon-Army creature
    /// whose victim dies mid-fight is handed straight back to
    /// `sub_1E580` instead of falling into the model's own idle
    /// brain.
    ///
    /// ⭐ **A LAW LANDED ON ONE CALL PATH IS NOT LANDED**: the port
    /// carried this snap ONLY inside [`Gen::mc2_alliance_clock`]
    /// (StageVar2 14), mid-walk instead of at the tick top and gated
    /// `phase < 2` instead of `phase not in {2,4,5,6}` — so 13, 16
    /// and 17 had none of it, and 14's own phase-3 case was missing.
    ///
    /// WITNESS (`recordings/mc2l6-rival-spells-galore.mgcr`): slots
    /// 53 and 102 are `(5,16)` Summon-Army wyverns (StageVar2 13,
    /// parent 343) attacking rival 378. 378 dies; at t=20,748 both
    /// drop out of the attack slot to `action45 = 129`; at t=20,749
    /// retail's pre-pass snaps both to `8*16+7 = 135` and runs
    /// `sub_1E580`, spending the no-lock **−5** lease (`f2e`
    /// 249→244 / 214→209) and clearing `target96` 378→0. The port
    /// ran m16's own state-1 idle instead.
    pub(crate) fn mc2_controlled_slot_snap(&mut self) {
        if !Self::sv_controlled_slot_snap_law() {
            return;
        }
        for m in 0..self.mob_chains.list.len() {
            let members: Vec<u16> = self.mob_chains.visible(m).to_vec();
            for s in members {
                let i = s as usize;
                if i == 0 || i >= self.ent.len() {
                    continue;
                }
                // The jump-table arm shared by 0xD/0xE/0x10/0x11.
                if !matches!(self.ent[i].site_z, 13 | 14 | 16 | 17) {
                    continue;
                }
                let phase = self.ent[i].tick70 & 7;
                // Outer gate (0x36d1f): phases 4 and 5 never react.
                // Inner gate (0x36d8b): attack (2) and flee (6) hold.
                if matches!(phase, 2 | 4 | 5 | 6) {
                    continue;
                }
                self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(7);
            }
        }
    }

    /// The MC2 class-5 per-state dispatch (`sub_57730`'s class-5
    /// table, :40116/:1242) — the MovementVerb::Mc2 arm. Unknown
    /// actions disable the entity like retail's invalid-row path
    /// (:40177) and count a misfit.
    pub(crate) fn mc2_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        // The ALLIANCE clock (`sub_1E9C0` head EF:10873 + expiry
        // EF:11003-10): the charm counts down in EVERY state (the
        // tier's lever IS the duration) and reverts through the
        // kind-10 resume shim on expiry or parent death; it also
        // re-enters the controlled slot after a combat resolves.
        if self.ent[i].site_z == 14 {
            self.mc2_alliance_clock(i);
        }
        let action = self.ent[i].tick70;
        // The shared class-5 `8*M+7` slot (`sub_1D5D0`, EF:9977) — a
        // CONTROLLED creature. StageVar2 (port field: site_z, free on
        // creatures) selects the body: 12 = Metamorph pose-puppet, 13 =
        // Summon-Army allied AI. Stage-HELD kinds (1..=10, 15) never
        // reach here — the world dispatch seam routes them through
        // `World::mc2_held_tick` (stagevars.rs).
        // StageVar2 == 0 (every ordinary spawn) is a no-op, so those
        // fall through to the per-model dispatch
        // (docs/spell-audit/summon-creatures.md).
        if action & 7 == 7 && self.ent[i].site_z != 0 {
            match self.ent[i].site_z {
                12 => self.mc2_metamorph_creature_tick(i, ctx),
                13 => self.mc2_summon_creature_tick(i, ctx),
                14 => self.mc2_alliance_creature_tick(i, ctx),
                // 16/17 = the pyramid-summon release chain.
                16 => self.mc2_doom_summon_home_tick(i, ctx),
                17 => self.mc2_doom_summon_spinup_tick(i, ctx),
                _ => {}
            }
            // ⭐⭐⭐ …AND THE MODEL WRAPPER'S TAIL STILL RUNS. `sub_1D5D0`
            // is called FROM the per-model phase-7 wrapper, so whatever
            // the StageVar2 case just did, the wrapper's own last
            // statement executes on the way out. The archer's is
            // `AddScroll05_04_20140` (EF:11960-66): `dword_0x10_16 = 0;
            // sub_1D5D0(entity, 32); if (actionIndex == 34)
            // sub_20060(entity);` — the aim test reads the action the
            // case JUST WROTE. The port had landed this on the STAGE-
            // HELD seam (`World::mc2_held_tick`, kinds 1..=10 — the
            // mc2l0 t=3945 fixture) and on `archer_tick`'s own role-7
            // arm, and stopped there: the CONTROLLED seam right above
            // `return`s, so an archer promoted to `8m+2` by the
            // alliance/summon/pyramid handler never took its aim.
            // A THIRD CALL PATH FOR A LAW ALREADY LANDED TWICE.
            // mc2l0-spells-galore t=25984, the charmed archer at slot
            // 621: `sub_1E9C0`'s engage hands it to 34 and retail then
            // fires the aim — `f5a` 0 -> 1, `speed` 30 -> 0 and one
            // entity-rand draw (48984 -> 56119) — where the port
            // walked on at patrol speed with no draw.
            // ⚠ OWED: the held seam runs FOUR more wrapper tails here
            // (the m17/m19/m20/m28 `byte_0x46_70 = 0` sub-state reset,
            // the goat bleat draw, the FLEE speed tail). They are owed
            // to this seam by the same argument and are NOT landed —
            // no corpus witness yet, and each is its own A/B.
            // `MGC_NO_MC2_CONTROLLED_WRAPPER_TAIL` reverts.
            if !no_mc2_controlled_wrapper_tail() {
                let e = &self.ent[i];
                if e.model65 == 4 && e.tick70 == e.model65.wrapping_mul(8).wrapping_add(2) {
                    self.archer_aim(i);
                }
            }
            // ⭐ A LAW ON ONE CALL PATH IS NOT LANDED — the m2
            // phase-7 wrapper's wander jiggle (`sub_1F8A0`,
            // EF:11567-74, [`Gen::m2_wrapper_jiggle`]) reads the
            // `StageVar2` the case body JUST WROTE, so a `(5,2)` a
            // controlled handler drops back into a held kind 1..9
            // takes it here too. UNWITNESSED on this seam (mc2l1's
            // witness is the stage-held seam) — the same
            // `MGC_NO_MC2_M2_WRAPPER_JIGGLE` switch covers it.
            if self.ent[i].model65 == 2 {
                self.m2_wrapper_jiggle(i);
                // A LAW ON ONE CALL PATH IS NOT LANDED — the same
                // wrapper's LAST statement, the lunge-countdown re-arm
                // (`if (actionIndex == 18) dword_0x10_16 = 1`,
                // EF:11576 / NETHERW.EXE 0x4413A), also reads the
                // action the case body just wrote. UNWITNESSED on this
                // seam (mc2l1's 22 witnesses are all stage-held); the
                // same `MGC_NO_MC2_M2_LUNGE_REARM` switch covers it.
                self.m2_wrapper_lunge_rearm(i);
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
        // The parent is the body's OWN caster carpet (parentId,
        // EF:10655) — a RIVAL's morph body follows the rival, not
        // the human (mc2l22 slot 673, owner 557); only the human's
        // body reads `ctx` (its carpet is out of pool).
        let (px, py, pz, pyaw) = if self.ent[i].id24 == PLAYER_TARGET {
            (ctx.px, ctx.py, ctx.pz, ctx.pyaw)
        } else {
            let p = self.ent[i].id24 as usize;
            if p == 0 || p >= self.ent.len() || self.ent[p].class64 == 0 {
                return;
            }
            let c = &self.ent[p];
            (c.x, c.y, c.z, c.f30)
        };
        // EF:10673-74: the offset z floors at ZERO — a low carpet
        // pins the body to the deck (mc2l0-sg slot 126: carpet 263,
        // 263-896 → retail 0 on every tick, 236 of 508 live-morph
        // ticks in the window sit on the clamp).
        let z = pz.saturating_sub(off).max(0);
        self.move_relink(i, px, py, z);
        self.ent[i].f30 = pyaw;
        self.ent[i].f34 = pyaw;
        // The creature's cry LOOPS while morphed — the FP effect: no
        // visible sprite from first person, just the monster's scream
        // on a loop (plus the distinct Morph cast sound 60). Play the
        // model's characteristic cry on a ~24-tick loop, anchored at
        // the creature (= the player pose).
        // ⛔ DIG 98-Q20 — StageVar2 12 IS NOT A TENANT OF EITHER WORD.
        // `sub_1D5D0`'s `case 0xC` arm is `sub_1E4D0`, and the shipped
        // `NETHERW.EXE` (file 0x42CD0-0x42D77) is twelve instructions:
        // the parent-liveness test, the three position words, the
        // per-model z offset, `CopyEntityPosition_57CF0`, then
        // `mov %dx,0x20(%esi)` / `mov %dx,0x1c(%esi)`. **No store to
        // +0x2E and none to +0x10.** So a metamorph puppet keeps its
        // ctor's `slot % 100` in @0x10 for its whole life, and this
        // cry loop — a PORT INVENTION with no retail counterpart — was
        // overwriting it. Park it in the (retail-dead) @0x2E home
        // instead, where nothing reads it.
        // ⚠ This also refutes the importer's own comment, which listed
        // 12 among the "five StageVar2 charm/latch kinds" that own
        // @0x2E and cited `sub_1E4D0` as counting it down.
        if self.ent[i].lease() <= 0 {
            let cry = match self.ent[i].model65 {
                16 => 39, // Wyvern
                25 => 37, // Cymmerian
                2 => 12,  // Day creature
                _ => 43,  // FireFly (19)
            };
            self.snd(cry, i);
            self.ent[i].set_lease(24);
        } else {
            self.ent[i].add_lease(-1);
        }
    }

    /// A/B toggle for the SUMMON-ARMY core law (dig A5, session 96):
    /// set `MGC_NO_SUMMON_CORE` to restore the pre-2026-09-03
    /// paraphrase, where StageVar2 13 aimed at its target EVERY tick
    /// and BEFORE the move core, had no wander jink, no crowd
    /// steer-away, a 2-D `< 1536` engage test and a −1/tick lease.
    fn summon_core_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_SUMMON_CORE").is_none())
    }

    /// `Entities[word_0x96_150]` read RAW. `sub_1E580`/`sub_1E700`
    /// test the LOCK POINTER only (`> Entities[0]`, i.e. non-zero) and
    /// never the target's life or reap bit, so a summon keeps aiming
    /// at a corpse until the 8-tick re-acquire drops the lock.
    pub(crate) fn mc2_summon_lock_pos(&self, t: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if t == 0 {
            return None;
        }
        if t == PLAYER_TARGET {
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = t as usize;
        if j >= self.ent.len() {
            return None;
        }
        let e = &self.ent[j];
        Some((e.x, e.y, e.z))
    }

    /// The lock-staleness test at `sub_1E580` EF:10712-16 (EXE
    /// 0x42DEB): `life <= 0 || byte[1] & 4` clears the lock — note the
    /// `<= 0`, and note that there is NO class or model re-check.
    fn mc2_summon_lock_ok(&self, t: u16, ctx: &MobCtx) -> bool {
        if t == PLAYER_TARGET {
            return !ctx.pdead;
        }
        let j = t as usize;
        if j == 0 || j >= self.ent.len() {
            return false;
        }
        let e = &self.ent[j];
        e.act_life > 0 && e.flags & 0x400 == 0
    }

    /// `sub_16FC0` (EF:8500, EXE 0x3B7C0) — the nearest LIVING class-3
    /// record of model 0 or 1 whose `id_0x1A_26` matches NEITHER
    /// argument's. `sub_1E580` passes the PARENT twice (EXE 0x42E14
    /// `push eax; push eax`), so it reads "the nearest wizard not on
    /// the caster's team". The chain it walks (`dword_38519`) is
    /// rebuilt every frame from the pool behind a `life >= 0` filter
    /// (EF:39972-81) — which is why a take whose rivals are all dead
    /// never locks one and every summon runs the parent-fallback leg.
    /// ⚠ The port's human lives OUTSIDE the pool, so it is offered
    /// separately through `ctx` as `PLAYER_TARGET`; retail's human
    /// carpet is an ordinary chain record.
    fn mc2_nearest_rival_wizard(&self, i: usize, ctx: &MobCtx) -> u16 {
        let (mx, my, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24)
        };
        let mut best = i32::MAX;
        let mut out = 0u16;
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
                out = j as u16;
            }
        }
        if own != PLAYER_TARGET && !ctx.pdead {
            let d = Self::dist2_sq(mx, my, ctx.px, ctx.py);
            if d < best {
                out = PLAYER_TARGET;
            }
        }
        out
    }

    /// `sub_1E700` (EF:10755, EXE 0x42F00) — the CONTROLLED-SUMMON
    /// CORE. ⭐⭐⭐ **THE SUMMON-ARMY SLOT AND THE PYRAMID-SUMMON SLOT
    /// ARE THE SAME RETAIL FUNCTION**: `sub_1D5D0`'s dispatch
    /// (EF:10013-16) puts `case 0xD:` and `case 0x10:` on ONE arm,
    /// both calling `sub_1E580`, so every law landed on the pyramid's
    /// StageVar2-16 path was owed to StageVar2 13 as well.
    ///
    /// Three details the port's StageVar2-13 paraphrase did not have:
    /// - the aim is on the **8-tick phase throttle**
    ///   (`!(byte_0x3E_62 & 7)`, EXE 0x42F94 `test byte [ebx+0x3e],7`)
    ///   and it runs **AFTER** `sub_1B8C0`, so this tick's turn commit
    ///   spends LAST tick's `roll`;
    /// - a **64-tick wander jink** on top of the aim (EF:10781-85);
    /// - the same-model **crowd steer-away** off the tick-top roster
    ///   (EF:10787-95 — the `bytearray_38403[model]` walk that
    ///   [`Gen::mc2_avoid_packmate`] already models).
    ///
    /// The recording dates all three. mc2l6-rival-spells-galore slot
    /// 495, a (5,19) firefly in `8*19+7` with StageVar2 13 and
    /// parentId 343 (the human's carpet): `roll` HOLDS at 492 for six
    /// ticks t=15656..15661 and moves only on the tick entered with
    /// `byte_0x3E_62 == 64` — `& 7 == 0` and `& 0x3F == 0` together —
    /// landing on 1448 after two rand draws; `yaw` then chases it at
    /// the row's full turn cap (row 88 `v_2` = 113): 492 → 605 → 718.
    /// Aiming every tick instead turned the full cap EVERY tick, which
    /// is 275 of the take's (5,19) heading rows, 77 of its (5,16)
    /// (row 84, cap 68) and 69 of its (5,25) (row 92, cap 113).
    fn mc2_summon_core(&mut self, i: usize, ctx: &MobCtx) {
        match self.mc2_state_head(i) {
            0 => {
                self.mc2_move_core(i);
                if self.ent[i].f63 & 7 != 0 {
                    return;
                }
                let t = self.ent[i].f146;
                let Some((tx, ty, _)) = self.mc2_summon_lock_pos(t, ctx) else {
                    return;
                };
                // ⭐⭐⭐ THE BLOCKED-TICK AIM GATE (EF:10817, EXE
                // 0x43068 `testb $0x4,0xe(%ebx)`). `byte[2] & 4` is
                // retail dword bit 18, but the PORT's flag word remaps
                // it to bit 27 (`F_BLOCKED`), so the literal
                // `1 << 18` here tested a bit nothing ever writes and
                // the gate was always open. See
                // `mc2::stagevars::summon_blocked_mask`.
                if self.ent[i].flags & crate::mc2::stagevars::summon_blocked_mask() == 0 {
                    let (mx, my) = (self.ent[i].x, self.ent[i].y);
                    self.ent[i].f34 = Self::angle_between(mx, my, tx, ty);
                    if self.ent[i].f63 & 0x3F == 0 {
                        self.mc2_wander_turn(i);
                    }
                }
                self.mc2_avoid_packmate(i);
            }
            1 => {
                // The hit arm (EF:10847-62): move, then retarget the
                // attacker unless it is a same-species peer or the
                // parent; flee rows hand to `+6`, others to `+2`.
                // ⚠ retail also fires `sub_6D8B0(parentId, 0x13, 1)`
                // — the caster's spell-19 XP report — which the port
                // has no home for yet.
                self.mc2_move_core(i);
                let (atk, own, cls, mdl) = {
                    let e = &self.ent[i];
                    (e.f40, e.id24, e.class64, e.model65)
                };
                let same_species = atk != PLAYER_TARGET
                    && (atk as usize) < self.ent.len()
                    && self.ent[atk as usize].class64 == cls
                    && self.ent[atk as usize].model65 == mdl;
                if atk != 0 && !same_species && atk != own {
                    self.ent[i].f146 = atk;
                    let flee =
                        BEHAVIOR[self.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                    self.ent[i].tick70 =
                        mdl.wrapping_mul(8).wrapping_add(if flee { 6 } else { 2 });
                }
            }
            _ => {
                // The dead arm (EF:10864-66) stamps the latch and
                // does NOT return — control lands back on the
                // caller's engage check.
                self.ent[i].set_lease(1);
            }
        }
    }

    /// `sub_1E580` (EF:10689, EXE 0x42D80), StageVar2 == 13 — the
    /// SUMMON-ARMY allied creature. The lease (`word_0x2E_46` → `f26`)
    /// is zeroed outright by a dead or missing PARENT and otherwise
    /// spends **one** per tick on the StageVar2-13 arm alone
    /// (EXE 0x42DB4 `cmp byte [ebx+0x49],0xd`); the lock is
    /// re-acquired through `sub_16FC0(parent, parent)` on the 8-tick
    /// throttle; and with NO lock the handler points `word_0x96_150`
    /// at the parent, runs the core, restores the null lock and spends
    /// **four more** lease (EF:10725-30, EXE 0x42E88-0x42EB3).
    ///
    /// ⭐ That −5/tick is what mc2l6-rival-spells-galore records:
    /// slot 495's `f2e` falls 245, 240, 235, 230, 225, 220 while
    /// `target96` stays 0 the whole time. All three rivals are dead by
    /// then (slots 370/378/383, life −349/−1080/−457) and the class-3
    /// chain is built `life >= 0` only, so `sub_16FC0` — which skips
    /// every record sharing the parent's id — finds nobody.
    ///
    /// The engage handoff is on the SAME 8-tick throttle, uses
    /// `sub_583F0`'s **3-D** distance against the row's `v_28`
    /// (EXE 0x42E60-0x42E7C), and does NOT clear StageVar2 — the port
    /// used a 2-D distance against a hardcoded 1536 and zeroed
    /// `site_z`.
    fn mc2_summon_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        if !Self::summon_core_law() {
            self.mc2_summon_creature_tick_legacy(i, ctx);
            return;
        }
        let parent = self.ent[i].id24;
        if self.mc2_target(parent, ctx).is_none() {
            self.ent[i].set_lease(0);
        } else {
            self.ent[i].add_lease(-1);
        }
        if self.ent[i].lease() <= 0 {
            let (x, y, z) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            // ⭐ THE LAPSED SUMMON DROPS **TWO** RECORDS, PUFF FIRST.
            // `sub_1E580`'s expiry arm is an ENUMERATED pair
            // (EF:10745-46): `if (StageVar2 == 13)
            // _4A190(&pos, 10, 73);` and only then
            // `_4A190(&pos, 10, 0);`. The `(10,73)` half is
            // StageVar2-13 EXCLUSIVE — the pyramid-summon twin
            // (StageVar2 16) reaches the same arm and drops the fire
            // alone. Minting only the fire lost a record AND pushed
            // every later free-stack pop one slot off.
            if !crate::mc2::roster::no_summon_nodes() && self.ent[i].site_z == 13 {
                self.mc2_spawn_summon_puff(x, y);
            }
            self.mc2_spawn_fire(x, y, z);
            self.ent[i].flags |= 0x400;
            return;
        }
        let mut target = self.ent[i].f146;
        if target != 0 && !self.mc2_summon_lock_ok(target, ctx) {
            self.ent[i].f146 = 0;
            target = 0;
        }
        if target == 0 && self.ent[i].f63 & 7 == 0 {
            target = self.mc2_nearest_rival_wizard(i, ctx);
            self.ent[i].f146 = target;
        }
        if target == 0 {
            self.ent[i].f146 = parent;
            self.mc2_summon_core(i, ctx);
            let v5 = self.ent[i].lease();
            self.ent[i].f146 = 0;
            self.ent[i].set_lease(v5 - 4);
            return;
        }
        self.mc2_summon_core(i, ctx);
        if self.ent[i].f63 & 7 == 0
            && let Some(tp) = self.mc2_summon_lock_pos(target, ctx)
        {
            let me = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            let reach = BEHAVIOR[self.ent[i].row156 as usize].v_28.max(0) as u32;
            if Self::mc2_dist3(me, tp) < reach {
                self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
                // ⭐ THE LANE CHANGES HANDS AT THE ENGAGE. The port
                // keeps retail's `word_0x2E_46` lease and its
                // `dword_0x10_16` scratch in the SAME `f26`, and the
                // lease is only ever read by `sub_1E580` on the
                // StageVar2 state we are leaving here. Every combat
                // handler on the state we are entering reads @0x10 —
                // m16's 15-bolt burst counter (`sub_24510` EF:15470-74),
                // m19's hover altitude, m17's dive index — and retail's
                // @0x10 on a freshly hatched summon is its ctor value.
                // Leaving 250 there made a summoned wyvern fire a (9,0)
                // homing bolt EVERY tick for the rest of its life.
                // ⭐ DIG 98-Q20 — WITH `lease2e` THIS LINE IS DEAD.
                // Retail clears nothing here (`sub_1E580` EF:10735-39
                // writes only `actionIndex`); round 97's patch had to
                // zero `f26` because the port homed the lease THERE.
                // The second field makes the clear unnecessary, which
                // is the A/B proof that 97's law was a symptom patch:
                // with the field on, `f26` already holds the ctor's
                // `slot % 100` that retail's combat brain reads.
                if !no_summon_lease_split() && crate::engine::features::no_summon_lease_field() {
                    self.ent[i].f26 = 0;
                }
            }
        }
    }

    /// The pre-2026-09-03 StageVar2-13 paraphrase, kept behind
    /// `MGC_NO_SUMMON_CORE` as the A/B arm for the dig above.
    fn mc2_summon_creature_tick_legacy(&mut self, i: usize, ctx: &MobCtx) {
        // Life countdown (word_0x2E_46): expire with a puff.
        self.ent[i].add_lease(-1);
        if self.ent[i].lease() <= 0 {
            let (x, y, z) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            if !crate::mc2::roster::no_summon_nodes() && self.ent[i].site_z == 13 {
                self.mc2_spawn_summon_puff(x, y);
            }
            self.mc2_spawn_fire(x, y, z);
            self.ent[i].flags |= 0x400;
            return;
        }
        let own = self.ent[i].id24;
        let (mx, my) = (self.ent[i].x, self.ent[i].y);
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
        let (tx, ty) = if target != 0 && (target as usize) < self.ent.len() {
            (self.ent[target as usize].x, self.ent[target as usize].y)
        } else {
            (ctx.px, ctx.py)
        };
        let yaw = Self::angle_between(mx, my, tx, ty);
        self.ent[i].f34 = yaw;
        self.mc2_move_core(i);
        if target != 0 {
            let d = Self::isqrt(Self::dist2_sq(mx, my, tx, ty) as u32);
            if d < 1536 {
                self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
                self.ent[i].site_z = 0;
            }
        }
    }

    /// A pyramid-summon target's position: PLAYER_TARGET is the
    /// out-of-pool human (never invalid — its death restarts the
    /// level), pool slots validate like retail's `Entities[t] >
    /// Entities[0] && life >= 0 && !(byte[1] & 4)` probe.
    fn mc2_doom_target_pos(&self, t: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if t == PLAYER_TARGET {
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = t as usize;
        if j == 0 || j >= self.ent.len() {
            return None;
        }
        let e = &self.ent[j];
        if e.flags & 0x400 != 0 || e.act_life < 0 {
            return None;
        }
        Some((e.x, e.y, e.z))
    }

    /// `sub_1E320` (EF:10566), StageVar2 == 17 — the pyramid-summon
    /// SPIN-UP: the hurled creature keeps flying at the summon's 320
    /// while decelerating `f126 -= 8`/tick and turning onto its
    /// target; at ≤ 16 it takes the per-model cruise (m0 → 30,
    /// m19 → 76, m21 → 96, m25 unchanged, EF:10588-601) and drops to
    /// the StageVar2-16 homing slot. An invalid target skips straight
    /// to slot 16 with the speed untouched (the `goto LABEL_14`).
    ///
    /// The head is the MOVE CORE ONLY, then a BARE life test
    /// (EF:10572-76) — `sub_1B8C0` is `mc2_move_core`, not the damage
    /// intake, and MC2 damage reaches an entity solely through the
    /// accumulate-mailbox that a state handler's head drains
    /// (EF:4023-25 and twins; `Gen::mail_write`). So retail applies
    /// NOTHING during the ~37-tick launch flight: a hit taken in
    /// flight stays QUEUED and is consumed on the first slot-16 tick,
    /// where it becomes either the `v2==1` retarget (the creature
    /// leaves the summon lane at once) or the `v2==2` husk. Draining
    /// the mailbox here instead swallowed that first hit — a
    /// non-fatal one lost retail's tick-1 retarget-out and left the
    /// creature in the husk-prone lane longer, and a fatal one made
    /// it vanish outright with no death animation and no puff. The
    /// life test is therefore unreachable in practice, exactly as in
    /// retail; it is kept because retail keeps it.
    fn mc2_doom_summon_spinup_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.mc2_move_core(i);
        if self.ent[i].act_life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let target = self.ent[i].f146;
        if let Some((tx, ty, _)) = self.mc2_doom_target_pos(target, ctx) {
            let (mx, my) = (self.ent[i].x, self.ent[i].y);
            self.ent[i].f34 = Self::angle_between(mx, my, tx, ty);
            self.ent[i].f126 -= 8;
            if self.ent[i].f126 > 16 {
                return;
            }
            match self.ent[i].model65 {
                0 => self.ent[i].f126 = 30,
                19 => self.ent[i].f126 = 76,
                21 => self.ent[i].f126 = 96,
                _ => {}
            }
        }
        self.ent[i].site_z = 16;
    }

    /// `sub_1E580` (EF:10689), StageVar2 == 16 — the pyramid-summon
    /// HOME slot: case 13's Summon-Army twin WITHOUT the per-tick
    /// life decrement (EF:10703-06 — pyramid summons persist while
    /// the pyramid lives; the life home is the spawn block's `f26`).
    /// Parent death zeroes the life → expire with a fire puff. The
    /// latch's home is **f26** — the class-5 @0x2E lane (the same home
    /// the StageVar2-13 Summon-Army twin counts down and the same one
    /// the conformance importer restores); it used to ride f46, which
    /// on a creature is `fontTypeIndex_0x3D_61` (the m0 dodge window)
    /// and has no @0x2E import, so replayed summons puffed on sight.
    /// Otherwise the `sub_1E700` core runs: mailbox intake (a KILL
    /// stamps @0x2E = 1 and freezes the body — no move, no state
    /// change, EF:10864-66), a hit re-targets the
    /// attacker — never the parent or a same-species peer; flee rows
    /// hand to +6, others +2 (the retail parent-XP `sub_6D8B0` award
    /// is a wizard-only no-op for the pyramid) — and the quiet path
    /// aims at the target on the 8-tick throttle with the 64-tick
    /// wander jink and the same-model crowd steer-away (EF:10814-40).
    /// ALL THREE arms then end in the engage check — the dead one
    /// included, because `sub_1E700` never returns early: 3-D reach
    /// inside the row's `v_28` hands to the model's +2 attack
    /// (site_z stays 16, as retail leaves StageVar2). That is how a
    /// husk leaves the lane: the frozen corpse keeps testing the
    /// reach on the `f63 & 7` throttle, converts to `8m+2`, and the
    /// model handler's own head turns `life < 0` into `8m+4` — the
    /// ordinary death animation. Corpus (mc2l24): NO doom summon
    /// anywhere in the take dies in `8m+7` — slot 573 (5,0) leaves
    /// via the `v2==1` retarget one tick into the lane (t=60142),
    /// slots 772/820 (5,19) leave via this engage check at FULL life
    /// (t=60153/60161) and are one-shot later in `8m+2`, each with a
    /// full 8-tick `8m+4`→`8m+5` death. Parent link: the level authors
    /// exactly ONE (5,10), scan-resolved (`parentId_0x28_40` has no
    /// entity home — the spawn-block APPROX).
    fn mc2_doom_summon_home_tick(&mut self, i: usize, ctx: &MobCtx) {
        let parent = (1..self.ent.len()).find(|&j| {
            let e = &self.ent[j];
            e.class64 == 5 && e.model65 == 10 && e.flags & 0x400 == 0 && e.act_life >= 0
        });
        if parent.is_none() {
            self.ent[i].set_lease(0);
        }
        if self.ent[i].lease() <= 0 {
            let (x, y, z) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z)
            };
            self.mc2_spawn_fire(x, y, z);
            self.ent[i].flags |= 0x400;
            return;
        }
        // Stale pool locks clear; the 8-tick re-acquire resolves the
        // pyramid's standing enemy (`sub_16FC0(parent)`) — the
        // out-of-pool player.
        let mut target = self.ent[i].f146;
        if target != 0 && self.mc2_doom_target_pos(target, ctx).is_none() {
            self.ent[i].f146 = 0;
            target = 0;
        }
        if target == 0 && self.ent[i].f63 & 7 == 0 {
            target = PLAYER_TARGET;
            self.ent[i].f146 = target;
        }
        if target == 0 {
            // Parentward drift + fast decay while unlocked
            // (EF:10725-31: aim the move at the parent, @0x2E -= 4).
            if let Some(p) = parent {
                let (mx, my) = (self.ent[i].x, self.ent[i].y);
                let (px, py) = (self.ent[p].x, self.ent[p].y);
                self.ent[i].f34 = Self::angle_between(mx, my, px, py);
            }
            // Retail runs the FULL `sub_1E700` core here (with
            // `word_0x96_150 = parentId`), THEN reads the latch back
            // and subtracts 4 (EF:10727-30) — so a summon that died
            // on this very tick reads the core's `= 1` and lands on
            // −3, expiring next tick, instead of draining its live
            // latch by 4s for ~62 ticks. APPROX: the port keeps the
            // manual parent-aim above and skips the core's wander
            // jink / crowd steer / `v2==1` retarget (whose
            // `word_0x96_150` lock retail clobbers back to 0 two
            // lines later anyway, EF:10729).
            if self.mc2_state_head(i) == 2 {
                self.ent[i].set_lease(1);
            } else {
                self.mc2_move_core(i);
            }
            self.ent[i].add_lease(-4);
            return;
        }
        // The `sub_1E700` core. NOTE the dead arm does NOT return:
        // retail's `else if (v2 == 2)` stamps the latch and falls off
        // the end of `sub_1E700` (EF:10864-66), so control lands back
        // on the caller's engage check at EF:10735 — a DEAD husk
        // keeps testing the reach and converts to the model's `+2`
        // the moment it is inside `v_28`, whereupon that handler's
        // own head sees `life < 0` and hands to `+4`, the ordinary
        // death animation. Returning here stranded the husk in
        // `8m+7` until the pyramid itself died (player-reported
        // "frozen forever", 2026-08-05).
        match self.mc2_state_head(i) {
            2 => {
                self.ent[i].set_lease(1);
            }
            1 => {
                self.mc2_move_core(i);
                let atk = self.ent[i].f40;
                let same_species = atk != 0
                    && atk != PLAYER_TARGET
                    && (atk as usize) < self.ent.len()
                    && self.ent[atk as usize].class64 == self.ent[i].class64
                    && self.ent[atk as usize].model65 == self.ent[i].model65;
                let is_parent = atk != PLAYER_TARGET && parent.is_some_and(|p| p == atk as usize);
                if atk != 0 && !same_species && !is_parent {
                    self.ent[i].f146 = atk;
                    target = atk;
                    let flee =
                        BEHAVIOR[self.ent[i].row156 as usize].flags & Mc2BehaviorRow::FLEE != 0;
                    self.ent[i].tick70 = self.ent[i]
                        .model65
                        .wrapping_mul(8)
                        .wrapping_add(if flee { 6 } else { 2 });
                }
            }
            _ => {
                self.mc2_move_core(i);
                if self.ent[i].f63 & 7 == 0 {
                    if let Some((tx, ty, _)) = self.mc2_doom_target_pos(target, ctx) {
                        let (mx, my) = (self.ent[i].x, self.ent[i].y);
                        // Same gate as `mc2_summon_core` — `sub_1D5D0`
                        // (EF:10013-16) sends StageVar2 13 and 16 to
                        // ONE retail function, so the pyramid summon
                        // carries the identical blocked-tick fence.
                        if self.ent[i].flags & crate::mc2::stagevars::summon_blocked_mask() == 0 {
                            self.ent[i].f34 = Self::angle_between(mx, my, tx, ty);
                            if self.ent[i].f63 & 0x3F == 0 {
                                self.mc2_wander_turn(i);
                            }
                        }
                        // Same-model crowd steer-away (EF:10829-38):
                        // the first DIFFERENT-owner same-species
                        // neighbour inside the f80 footprint box
                        // turns the heading straight away from it.
                        let pitch = self.ent[i].f80 as i32;
                        for j in 1..self.ent.len() {
                            if j == i {
                                continue;
                            }
                            let e = &self.ent[j];
                            if e.class64 != 5
                                || e.model65 != self.ent[i].model65
                                || e.flags & 0x400 != 0
                                || e.id24 == self.ent[i].id24
                            {
                                continue;
                            }
                            if (mx as i32 - e.x as i32).abs() < pitch
                                && (my as i32 - e.y as i32).abs() < pitch
                            {
                                self.ent[i].f34 = Self::angle_between(e.x, e.y, mx, my);
                                break;
                            }
                        }
                    }
                }
            }
        }
        // The engage handoff (EF:10735-40): 8-tick throttle, 3-D
        // reach inside the row's `v_28` → the model's +2 attack.
        if self.ent[i].f63 & 7 == 0
            && let Some(tp) = self.mc2_doom_target_pos(target, ctx)
        {
            let me = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            let reach = BEHAVIOR[self.ent[i].row156 as usize].v_28.max(0) as u32;
            if Self::mc2_dist3(me, tp) < reach {
                self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
            }
        }
    }

    /// `sub_3A7F0` (EF:29701) — THE CHARM-ELIGIBILITY PREDICATE, and
    /// it is a shared one: `sub_3A650`'s conversion sweep asks it, and
    /// so does `sub_67CB0`'s **`case 0x19`** — the (9,25) alliance
    /// carrier's own auto-target acquisition (EF:54991).
    ///
    /// Class 5 only; the model bar is 12-15, 22, 23, 26 and 27, plus
    /// 25 when `byte_0x46_70` is set (model 24 falls THROUGH the
    /// ladder and is eligible); then StageVar2 in {13, 14, 16, 17} —
    /// already summoned, allied, or on the pyramid release chain —
    /// and finally the child-follow action 232. **No life test and no
    /// reap test anywhere in it.**
    pub(crate) fn mc2_charm_eligible(&self, j: usize) -> bool {
        let Some(e) = self.ent.get(j) else {
            return false;
        };
        if e.class64 != 5 {
            return false;
        }
        let m = e.model65;
        if matches!(m, 12..=15 | 22 | 23 | 26 | 27) || (m == 25 && e.f71 != 0) {
            return false;
        }
        if matches!(e.site_z, 13 | 14 | 16 | 17) {
            return false;
        }
        e.tick70 != 232
    }

    /// `sub_3A650` (EF:29637; the (10,74) executor's class-10 action
    /// 0x51) — the ALLIANCE conversion: a SAME-SPECIES area charm.
    /// Sweep a square of tile-radius `radius` (the tier's 16/26/32)
    /// around the struck creature; every living creature of the
    /// victim's MODEL passing the `sub_3A7F0` eligibility filter
    /// (EF:29701) converts: sound 6, StageVar2 = 14, owner → the
    /// caster (the `mc2_allied` side table — `id24` stays the
    /// authored disposition), duration → f26, and either its target
    /// clears (mid-attack, `action & 7 == 2`) or it enters the
    /// model's controlled slot `8m+7` (EF:29660-90). Zero damage.
    /// APPROX: retail also converts stage-HELD creatures (StageVar1
    /// saved to `word_0x4A_74`, restored on expiry) — the port skips
    /// creatures under a live hold or another charm.
    pub(crate) fn mc2_alliance_convert(
        &mut self,
        victim: u16,
        parent: u16,
        radius: i32,
        duration: i32,
    ) {
        let v = victim as usize;
        if victim == 0 || victim == PLAYER_TARGET || v >= self.ent.len() || self.ent[v].class64 != 5
        {
            return;
        }
        let model = self.ent[v].model65;
        // `sub_3A7F0`'s model bar: 12-15, 22, 23, 26, 27 are never
        // charmable — a victim of a barred species converts nothing.
        if matches!(model, 12..=15 | 22 | 23 | 26 | 27) {
            return;
        }
        let (vx, vy) = (self.ent[v].x as i32 >> 8, self.ent[v].y as i32 >> 8);
        let dur = duration.clamp(1, i16::MAX as i32) as i16;
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if e.class64 != 5
                || e.model65 != model
                || e.flags & 0x400 != 0
                || e.act_life < 0
                || ((e.x as i32 >> 8) - vx).abs() > radius
                || ((e.y as i32 >> 8) - vy).abs() > radius
                // Charmed (13/14/16/17) or stage-held (port APPROX,
                // doc above) — only free-roaming creatures convert.
                || !matches!(e.site_z, 0 | 10)
                // The child-follow state (232) and the flagged m25
                // are ineligible (EF:29701-29726).
                || e.tick70 == 232
                || (model == 25 && e.f71 != 0)
            {
                continue;
            }
            self.snd(6, j);
            let e = &mut self.ent[j];
            e.site_z = 14;
            // ⭐ DIG 98-Q20 — THE PROOF THAT ONE WORD CANNOT SERVE
            // BOTH. `sub_3A650` (EF:29682-90) writes `word_0x2E_46`
            // AND `word_0x30_48` on a victim whose
            // `actionIndex & 7 == 2` — a creature in a combat state
            // whose handlers read `dword_0x10_16` — and does NOT move
            // it out of that state. Homing the two in one `f26` blew
            // away the victim's live combat scratch on the charm tick.
            e.set_lease(dur);
            if e.tick70 & 7 == 2 {
                e.f146 = 0;
            } else {
                e.tick70 = model.wrapping_mul(8).wrapping_add(7);
            }
            self.mc2_allied.0.insert(j as u16, parent);
        }
    }

    /// `sub_3A650` (EF:29637) — the (10,74) executor's ONLY tick, and
    /// the reason the alliance is a POOL RECORD rather than an inline
    /// call at the impact ([`Gen::mc2_spawn_alliance_exec`]). Run the
    /// same-species area charm around `word_0x96_150` (the struck
    /// victim, `f146`) with `byte_0x46_70` as the tile radius (`f71`)
    /// and `subSpellIndex_0x2A_42` as the duration (`f140`), then
    /// `DisableEntityDrawing04_57F10(a1x)` — the disable sits AFTER
    /// the whole `if (word_0x96_150)` block and is UNCONDITIONAL
    /// (EF:29694), so a victimless executor still burns its one tick.
    pub(crate) fn mc2_alliance_exec_tick(&mut self, i: usize) {
        let (victim, parent, radius, dur) = {
            let e = &self.ent[i];
            (e.f146, e.id24, e.f71 as i32, e.f140)
        };
        self.mc2_alliance_convert(victim, parent, radius, dur);
        self.ent[i].flags |= 0x400;
    }

    /// The per-tick half of the alliance law (`sub_1E9C0` head
    /// EF:10873 + expiry EF:11003-10), run from the class-5 dispatch
    /// head in EVERY state: count the charm down, revert on expiry /
    /// parent death through the kind-10 resume shim (`id24` was never
    /// touched, so the authored disposition simply resumes), and
    /// re-enter the controlled slot once a combat resolves (retail
    /// returns controlled creatures to `8m+7`; our model machines
    /// drop to their wander phases 0/1 instead).
    fn mc2_alliance_clock(&mut self, i: usize) {
        if self.ent[i].flags & 0x400 != 0 || self.ent[i].act_life < 0 {
            self.mc2_allied.0.remove(&(i as u16));
            return;
        }
        let parent = self.mc2_allied.0.get(&(i as u16)).copied().unwrap_or(0);
        // Parent-death probe on the 8-tick cadence (pool wizards by
        // owner id; the human parent's death restarts the level, so
        // it counts as alive here).
        let mut parent_dead = parent == 0;
        if parent != 0 && parent != PLAYER_TARGET && self.ent[i].f63 & 7 == 0 {
            parent_dead = !(1..self.ent.len()).any(|j| {
                let e = &self.ent[j];
                e.class64 == 3
                    && e.model65 <= 1
                    && e.id24 == parent
                    && e.flags & 0x400 == 0
                    && e.act_life >= 0
            });
        }
        self.ent[i].add_lease(-1);
        if self.ent[i].lease() <= 0 || parent_dead {
            self.ent[i].site_z = 10;
            self.ent[i].f146 = 0;
            self.mc2_allied.0.remove(&(i as u16));
            return;
        }
        // ⚠ DIG 98-Q22 — THIS IS A MIS-SITED PARAPHRASE OF
        // [`Gen::mc2_controlled_slot_snap`], WHICH NOW RUNS THE REAL
        // LAW. `sub_1E9C0` writes no `actionIndex` here; the snap is
        // `sub_12500`'s, it runs at the TICK TOP over the roster
        // chains (not mid-walk), it serves StageVar2 13/14/16/17 (not
        // 14 alone), and its gate is "every phase but 2/4/5/6" (not
        // `< 2`). Left standing because removing it is a separate,
        // separately measurable law — it only differs for a record
        // that enters phase 0/1 DURING this walk, which retail would
        // not snap until the next frame. 🏦 BANKED.
        if self.ent[i].tick70 & 7 < 2 {
            self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(7);
        }
    }

    /// `sub_1E9C0` (EF:10873), StageVar2 == 14 — the ALLIANCE-charmed
    /// creature's controlled slot.
    ///
    /// ⭐⭐⭐ **IT IS THE SAME `sub_1E700` CORE THE SUMMON SLOT RUNS**,
    /// and the port had a hand-rolled paraphrase instead. EF:10971-73
    /// is three statements — `v8 = word_0x96_150;
    /// word_0x96_150 = parentId_0x28_40; sub_1E700(a1x, a2);` — so an
    /// ally with no enemy does not "stand by": it **FOLLOWS ITS
    /// CASTER**, through the very core ([`Gen::mc2_summon_core`]) that
    /// already carries the 8-tick aim throttle, the 64-tick wander
    /// jink, the blocked-tick aim gate and the same-model crowd steer.
    /// The old body returned without moving whenever `word_0x96_150`
    /// was 0, which on mc2l0-spells-galore is every tick from the
    /// charm onward: t=23948, archers 559/621/627 walk `y` −30 at
    /// their row speed while the port left all three standing. That
    /// was the take's free-run horizon.
    ///
    /// Retail then ADOPTS the parent's fight (EF:10974-77): `v9` is
    /// the parent's own lock `word_0x96_150`, or its attacker
    /// `word_0x26_38` when the lock is empty, and it displaces the
    /// ally's own lock. The lock is dropped again (EF:10983-87) if it
    /// names a FELLOW ALLY OF THE SAME PARENT (`parentId` equal AND
    /// `StageVar2 == 14` — both halves), a corpse, or a reaped record.
    /// Only then does the reach test hand to the model's `8m+2` with
    /// `sub_583F0`'s 3-D distance against the row's `word_0x1c_28`
    /// (NOT the hardcoded 2-D 1536 the port used), keeping StageVar2
    /// = 14 so the clock keeps counting, and awards the caster
    /// Alliance XP (`sub_6D8B0(parentId, 0x18, 1)`, EF:10998).
    ///
    /// APPROX, cited: retail's `v9` reads the parent's two words off
    /// its POOL RECORD. A pool wizard parent gives both here; the
    /// HUMAN is out of pool and the ctx carries no lock, so the
    /// `word_0x26_38` half alone is served by its observable
    /// equivalent — the nearest pool record currently targeting the
    /// parent — on the same 8-tick throttle the aim uses. The old
    /// body's "else the nearest enemy wizard" fallback is GONE: retail
    /// reads two words and neither of them is a scan.
    /// A/B: `MGC_NO_MC2_ALLIANCE_CORE` restores the old paraphrase.
    fn mc2_alliance_creature_tick(&mut self, i: usize, ctx: &MobCtx) {
        if no_mc2_alliance_core() {
            self.mc2_alliance_creature_tick_legacy(i);
            return;
        }
        let parent = self.mc2_allied.0.get(&(i as u16)).copied().unwrap_or(0);
        // EF:10971-73 — the lock is parked on the parent for the core.
        let v8_in = self.ent[i].f146;
        self.ent[i].f146 = parent;
        self.mc2_summon_core(i, ctx);
        // EF:10974-77 — `v9` = the parent's lock, else its attacker.
        let v9 = match self.ent.get(parent as usize) {
            Some(p) if parent != PLAYER_TARGET && parent != 0 => {
                if p.f146 != 0 { p.f146 } else { p.f40 }
            }
            _ => self.mc2_alliance_parent_attacker(i, parent),
        };
        let mut v8 = v8_in;
        if v9 != 0 && v8 != v9 {
            v8 = v9;
        }
        self.ent[i].f146 = v8;
        if v8 == 0 {
            return;
        }
        // EF:10983-87 — drop a lock on a fellow ally, a corpse or a
        // reaped record. `PLAYER_TARGET` is the out-of-pool human and
        // has no record to test: retail's `Entities[v8]` would be the
        // carpet, which is neither charmed nor dead here.
        if v8 != PLAYER_TARGET {
            let Some(t) = self.ent.get(v8 as usize) else {
                self.ent[i].f146 = 0;
                return;
            };
            let ally = t.site_z == 14 && self.mc2_allied.0.get(&v8).copied() == Some(parent);
            if ally || t.act_life <= 0 || t.flags & 0x400 != 0 {
                self.ent[i].f146 = 0;
                return;
            }
        }
        // EF:10990-97 — the 3-D reach hand-off, row `word_0x1c_28`.
        let Some(tp) = self.mc2_summon_lock_pos(v8, ctx) else {
            return;
        };
        let me = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        let reach = BEHAVIOR[self.ent[i].row156 as usize].v_28.max(0) as u32;
        if Self::mc2_dist3(me, tp) < reach {
            self.ent[i].f146 = v8;
            self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
            self.mc2_cast_xp.0.push((parent, 24, 1));
        }
    }

    /// The `word_0x26_38` half of `sub_1E9C0`'s `v9` for an
    /// OUT-OF-POOL parent (the human): the nearest live pool record
    /// currently locked onto the parent. Retail reads the word off the
    /// parent's record; the human has none, and the ctx carries no
    /// lock. Throttled to the core's own 8-tick aim cadence so it
    /// cannot out-resolve the aim it feeds.
    fn mc2_alliance_parent_attacker(&self, i: usize, parent: u16) -> u16 {
        if parent == 0 || self.ent[i].f63 & 7 != 0 {
            return 0;
        }
        let (mx, my) = (self.ent[i].x, self.ent[i].y);
        let mut best = i32::MAX;
        let mut found = 0u16;
        for j in 1..self.ent.len() {
            let e = &self.ent[j];
            if j == i || e.flags & 0x400 != 0 || e.act_life < 0 || e.f146 != parent {
                continue;
            }
            if !matches!(e.class64, 3 | 5) {
                continue;
            }
            let d = Self::dist2_sq(mx, my, e.x, e.y);
            if d < best {
                best = d;
                found = j as u16;
            }
        }
        found
    }

    /// The pre-`sub_1E700` paraphrase of the alliance slot, kept for
    /// A/B under `MGC_NO_MC2_ALLIANCE_CORE`: a standalone target scan
    /// that idled the ally whenever it found nobody.
    fn mc2_alliance_creature_tick_legacy(&mut self, i: usize) {
        let parent = self.mc2_allied.0.get(&(i as u16)).copied().unwrap_or(0);
        let (mx, my) = (self.ent[i].x, self.ent[i].y);
        let mut target = self.ent[i].f146;
        let stale = target == 0
            || target == PLAYER_TARGET
            || (target as usize) >= self.ent.len()
            || self.ent[target as usize].flags & 0x400 != 0
            || self.ent[target as usize].act_life < 0;
        if stale {
            target = 0;
            self.ent[i].f146 = 0;
        }
        if target == 0 && self.ent[i].f63 & 7 == 0 {
            let mut best = i32::MAX;
            for j in 1..self.ent.len() {
                let e = &self.ent[j];
                if j == i || e.flags & 0x400 != 0 || e.act_life < 0 {
                    continue;
                }
                if self.mc2_allied.0.get(&(j as u16)) == Some(&parent) {
                    continue;
                }
                let attacks_parent = parent == PLAYER_TARGET
                    && matches!(e.class64, 3 | 5)
                    && e.f146 == PLAYER_TARGET;
                let enemy_wizard = e.class64 == 3 && e.model65 <= 1 && e.id24 != parent;
                if !(attacks_parent || enemy_wizard) {
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
        if target == 0 {
            return; // no fight to join — stand by
        }
        let (tx, ty) = (self.ent[target as usize].x, self.ent[target as usize].y);
        let yaw = Self::angle_between(mx, my, tx, ty);
        self.ent[i].f34 = yaw;
        self.mc2_move_core(i);
        let d = Self::isqrt(Self::dist2_sq(mx, my, tx, ty) as u32);
        if d < 1536 {
            self.ent[i].tick70 = self.ent[i].model65.wrapping_mul(8).wrapping_add(2);
            self.mc2_cast_xp.0.push((parent, 24, 1));
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
            if self.ent[i].model65 == 10 && self.ent[i].tick70 == 10 {
                // The castle ball rides its own dedicated flight
                // (CastCastleProjectile_66B30 / sub_66D00) — the
                // generic flyer's water arm was splashing the build
                // away (mc2l3 t=244's (10,5) where retail builds).
                self.mc2_castle_ball_tick(i);
            } else if self.ent[i].model65 == 3 && self.ent[i].tick70 == 3 {
                self.mc2_meteor_shot_tick(i, ctx);
            } else if self.ent[i].model65 == 9 && self.ent[i].tick70 == 9 {
                // Lightning L0 (subtype 9) = the `sub_66750` one-tick
                // hitscan BEAM, not a traveling ball. Resolve it whole
                // this tick (docs/spell-audit/lightning.md §5.A) so it
                // flashes to its impact and is gone — under RAPID
                // re-fire that reads as the authentic crackle.
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
            // ⭐⭐ A RECORD ALREADY DEAD AT THE TICK TOP IS NOT A
            // ROSTER MEMBER, SO RETAIL NEVER STAMPS IT. The tick-top
            // rebuild (EF:39988) drops anything with `life < 0` from
            // `bytearray_38403x`, so `sub_68BF0`'s `= 0xFA` arm
            // (EF:55484-85) can only ever reach a member that died
            // BETWEEN the rebuild and this pre-pass — for everything
            // else `byte_0x39_57` simply FREEZES at its last live
            // countdown. The port re-stamped 250 every tick for the
            // whole death animation and left 250 as freed-slot
            // residue. mc2l3 slot 151: 14 at t=246, 13 at t=247 as it
            // dies, and retail still reads 13 at t=248/249 where the
            // port had written 250.
            // ⚠ This is NOT a dead lane: `mc2/roster.rs` reads
            // `f58 != 0` inside the creature's own handler with no
            // life gate of its own, which is exactly how the MC1 twin
            // bit (a creature that died asleep froze at 0 in retail
            // while the port read back a nonzero counter).
            if e.act_life < 0 {
                continue;
            }
            self.mc2_awake_one(i, ctx);
        }
        // sub_68BF0's SECOND loop (EF:55489-90): dword_38523 = the
        // mana-sphere family awake-ticks too — spheres near the
        // player arm their f58 like creatures do. No dead reset here
        // (retail's sphere loop is unconditional), and NO model test:
        // the chain itself is the filter, and it is built from models
        // 39, 40 AND 57 (EF:40023-40062), so a fool's sphere wakes
        // exactly like a real one.
        //
        // ⚠⚠ AND THE POOL WALK STAYS HERE, DELIBERATELY — this looks
        // like the twin of the (10,54) aura's scan (`mc2_aura_tick`,
        // moved to the chain 2026-08-25g for mc2l3 t=9816), and it is
        // NOT. **`sub_68BF0` runs BEFORE the entity walk** (EF:40108
        // vs the walk at EF:40118), so it has already returned by the
        // time anything is born this tick — the newborn-invisibility
        // that makes the chain load-bearing for the aura cannot arise
        // here, which is exactly why swapping this one to the chain is
        // INERT on mc2l3 (10055 either way). It is not free, though:
        // it moved all four `mc2_cave` goldens, and an unattributable
        // golden move with no corpus row demanding it is not a fix.
        // Understand that delta before landing it.
        for i in 1..self.ent.len() {
            let e = &self.ent[i];
            if e.class64 == 10 && matches!(e.model65, 39 | 40 | 57) && e.flags & 0x400 == 0 {
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
        // high bits (F_STOP &c).
        if self.ent[i].flags & 1 != 0 {
            return;
        }
        if self.ent[i].f59 != 0 {
            self.ent[i].f59 -= 1;
            return;
        }
        let e = &self.ent[i];
        // Patch option `map_wide_ball_rolling` (player-ruled both games
        // 2026-09-06): the SPHERE leg re-arms without the 24-tile
        // radius, exactly like MC1's `mob_awake_pass` ball rows — every
        // sphere rolls to rest at retail's own 16-of-17 duty cycle
        // instead of "running away" when the human walks into wake
        // range. Spheres only (this leg is the whole class-10 walk,
        // 39/40/57 — the fool's sphere wakes like a real one under
        // retail and keeps doing so here); the creature gate stays.
        let ball = e.class64 == 10;
        if (ball && ctx.patches.map_wide_ball_rolling && !ctx.strict)
            || Self::dist2_sq(e.x, e.y, ctx.px, ctx.py) < 0x240_0000
        {
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

#[cfg(test)]
mod tests {
    use crate::engine::features::Gen;

    fn w3v_flat_gen() -> Gen {
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


    fn w3v_ctx() -> crate::mc1::mobs::MobCtx {
        crate::mc1::mobs::MobCtx {
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


    /// ⭐ ROUND 104 — **THE TANK'S HIT LATCHES ITS ATTACKER AS ITS
    /// TARGET.** `sub_252E0`, the m18 head, ends `if (result >= 1) {
    /// if (result <= 1) a1x->word_0x96_150 = a1x->word_0x26_38; else
    /// if (result == 2) a1x->actionIndex_0x45_69 = 148; }`
    /// (EF:16139-49; shipped `NETHERW.EXE` 0x49B77-0x49B97). The port
    /// split `sub_252E0` into `Gen::m18_head` plus the shared
    /// `Gen::mc2_state_head` and carried only the DEATH half, so a
    /// roaming tank — whose walk arm `sub_25050` zeroes
    /// `word_0x96_150` on every clean tick (EF:15960) — entered the
    /// barrage machine with NO target, and `sub_254E0`'s 22-unit
    /// ((4 << 11) / 360) turn never ran. mc2l22 t=11725 slot 956,
    /// t=18771 slot 26, t=25371 slot 5: heading off by exactly one
    /// 22-unit step, in both directions, with every other lane
    /// bit-exact.
    #[test]
    fn a_damaged_tank_latches_its_attacker_as_its_target() {
        let mut g = w3v_flat_gen();
        let ctx = w3v_ctx();
        let i = g.new_event().expect("tank slot");
        {
            let e = &mut g.ent[i];
            e.class64 = 5;
            e.model65 = 18;
            e.tick70 = 145; // M18_BASE + 1 — the walk (`sub_25050`)
            e.f71 = 0;
            e.max_life = 36_000;
            e.act_life = 36_000;
            e.f26 = 600;
            e.f146 = 0; // the walk zeroes it on every clean tick
            e.mail[0] = (1_600, 300); // a hit from slot 300
        }
        let (x, y) = (100u16 << 8, 100u16 << 8);
        let z = g.ground_z(x, y) as i16;
        g.link(i, x, y, z);
        g.m18_tick(i, &ctx);
        assert_eq!(
            g.ent[i].f146, 300,
            "sub_252E0's result==1 arm latches word_0x26_38 into word_0x96_150"
        );
        assert_eq!(g.ent[i].tick70, 144, "the walk's hit arm enters state 0…");
        assert_eq!(g.ent[i].f71, 1, "…sub-state 1, the watch");
    }


    /// ⭐ ROUND 104 — **THE FOOTPRINT CLEAR STAMPS BOTH HALVES OF THE
    /// KILLER PAIR.** `sub_57390` (EF:39801-03) is
    /// `life_0x8 = -1; word_0x24_36 = a2; word_0x26_38 = a2;`. The
    /// port wrote `f36`/`f38` — one home short: `f38` IS
    /// `word_0x24_36`, but `word_0x26_38` is `f40`, and `f36` is
    /// `word_0x22_34`, a lane only the m27 tree body uses and one
    /// this arm can never reach (m27 is on the protected list). So
    /// every corpse the build footprint made carried a ZERO attacker,
    /// and `sub_28CE0`'s Cymmerian split — which seeds each mini's
    /// `word_0x24_36` from the parent's `word_0x26_38` (EF:19168) —
    /// hatched three minis that thought nobody killed their parent.
    /// mc2l22 t=13405 slot 318 is the witness: retail `f24`/`f26`
    /// both 0 -> 424, the port only `f24`.
    #[test]
    fn the_footprint_clear_stamps_the_attacker_half_of_the_killer_pair() {
        let mut g = w3v_flat_gen();
        let victim = g.new_event().expect("victim slot");
        {
            let e = &mut g.ent[victim];
            e.class64 = 5;
            e.model65 = 25;
            e.tick70 = 202; // NOT the protected 200 (`actionIndex != -56`)
            e.act_life = 4_300;
            e.id24 = 7; // owned by wizard 7, so the owner test does not skip it
        }
        let (x, y) = (100u16 << 8, 100u16 << 8);
        let z = g.ground_z(x, y) as i16;
        g.link(victim, x, y, z);
        let t = crate::engine::features::tile((x >> 8) as u8, (y >> 8) as u8);
        g.mc2_building_clear_tile(t, 9); // builder 9 != owner 7
        assert_eq!(g.ent[victim].act_life, -1, "life_0x8 = -1");
        assert_eq!(g.ent[victim].f38, 9, "word_0x24_36 = a2");
        assert_eq!(
            g.ent[victim].f40, 9,
            "word_0x26_38 = a2 — the half the port dropped"
        );
        assert_eq!(
            g.ent[victim].f36, 0,
            "word_0x22_34 is NOT part of the stamp"
        );
    }


    /// ⭐⭐⭐ ROUND 98 — **THE MC2 COLUMN NEVER SEEDED `byte_0x5D_93`.**
    ///
    /// `x_BYTE_D8A2E` is the SAME table MC1 ships pre-baked as
    /// [`crate::mc1::mobs::FRAME_COUNTS`] (MC2 appends one trailing
    /// 0). Two independently extracted binaries agreeing on 37 bytes
    /// is the cross-engine anchor for the table.
    #[test]
    fn the_mc2_frame_count_table_is_mc1s() {
        assert_eq!(
            super::D8A2E_FRAMES[..37],
            crate::mc1::mobs::FRAME_COUNTS[..],
            "x_BYTE_D8A2E[0..37] == MC1's byte_90AD8"
        );
        assert_eq!(super::D8A2E_FRAMES[37], 0, "MC2's extra trailing entry");
    }

    /// PROVENANCE: `particlesParameters_D951C[].byte_12` is FILLED AT
    /// BOOT from the sprite's own flags high byte
    /// (`sub_71410_process_tmaps`, EF:44906 —
    /// `particlesParameters_D951C[i].byte_12 = *(x_BYTE*)(v1 + 1)`,
    /// byte 1 of the decompressed TMAPS entry = `flags >> 8`), once,
    /// against the DAY bank (the same load pass whose sibling
    /// derivation [`crate::mc2::derive_sprite_extents`] is already
    /// day-sourced). This re-derives [`super::MC2_SPRITE_DRAW_TYPE`]
    /// from the baked day sprite index so the table can never drift
    /// from the shipped art. Self-skips without a baked bundle.
    #[test]
    fn mc2_sprite_draw_types_are_the_day_banks_flag_high_bytes() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../baked/assets/mc2-day");
        let Ok(bundle) = mgc_formats::bundle::Bundle::load(&dir) else {
            return;
        };
        let Some((index, _)) = bundle.sprites.as_ref() else {
            return;
        };
        let flags: std::collections::HashMap<u32, u16> =
            index.sprites.iter().map(|e| (e.id, e.flags)).collect();
        // `sub_71410_process_tmaps` walks rows until the first with
        // BOTH speed_6 and rotSpeed_8 zero; rows from there on keep
        // the shipped 0.
        let term = super::SPRITE_PARAMS
            .iter()
            .position(|p| p.speed_6 == 0 && p.rot_speed_8 == 0)
            .expect("the loop terminator row");
        for (row, p) in super::SPRITE_PARAMS.iter().enumerate() {
            let want = if row >= term {
                0
            } else {
                (flags[&(p.word_0 as u32)] >> 8) as u8
            };
            assert_eq!(
                super::MC2_SPRITE_DRAW_TYPE[row], want,
                "row {row} (sprite {})",
                p.word_0
            );
        }
        // Only seven rows animate; everything else is a single frame.
        let odd: Vec<(usize, u8)> = (0..super::MC2_SPRITE_DRAW_TYPE.len())
            .map(|r| (r, super::mc2_sprite_frames(r)))
            .filter(|&(_, f)| f != 1)
            .collect();
        assert_eq!(
            odd,
            vec![(7, 16), (222, 16), (237, 16), (244, 11), (332, 11), (333, 11), (334, 11)]
        );
    }

    /// The stamp itself, and `sub_585A0`'s cap on top of it. Retail's
    /// own captures pin both: across 80 sampled `mc2l6-rsg` master
    /// images every live record with a set sprite carries
    /// `+0x5D == 1` EXCEPT the `(10,0)` fire (sprite row 7) at 16,
    /// and `+0x5C` is nonzero on `(10,0)` ALONE — 7, 8, 9 — i.e. the
    /// cap is real and the port's uncapped `frame88 += 1` was not.
    #[test]
    fn the_sprite_stamp_carries_the_frame_count_and_sub_585a0_caps_on_it() {
        let planes = crate::engine::features::Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let assets = crate::engine::features::FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
        };
        let mut g = Gen::new(
            planes,
            assets,
            1,
            crate::chassis::ChassisParams::MC2,
            crate::verbs::VerbSet::MC2,
        );
        let i = g.new_event().expect("a pool slot");
        g.mc2_set_sprite(i, 7);
        assert_eq!(g.ent[i].type86, 7);
        assert_eq!(g.ent[i].frames89, 16, "row 7 is draw type 36 -> 16 frames");
        assert_eq!(g.ent[i].frame88, 0);
        for want in 1..=16u8 {
            g.mc2_anim_step(i);
            assert_eq!(g.ent[i].frame88, want);
        }
        g.mc2_anim_step(i);
        assert_eq!(g.ent[i].frame88, 16, "sub_585A0 stops AT the count");

        // A single-frame row steps exactly once and then stops.
        let j = g.new_event().expect("a pool slot");
        g.mc2_set_sprite(j, 8);
        assert_eq!(g.ent[j].frames89, 1);
        g.mc2_anim_step(j);
        g.mc2_anim_step(j);
        assert_eq!(g.ent[j].frame88, 1);
    }

    fn q22_gen() -> Gen {
        let planes = crate::engine::features::Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let assets = crate::engine::features::FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
        };
        Gen::new(
            planes,
            assets,
            1,
            crate::chassis::ChassisParams::MC2,
            crate::verbs::VerbSet::MC2,
        )
    }

    /// The MC2 shape of the tick-top chain rebuild (world.rs:4642 —
    /// `reset(29)`); `Gen::rebuild_mob_chains` is the MC1-sized (20)
    /// test helper and drops every model above 19.
    fn q22_rebuild_mc2_chains(g: &mut Gen) {
        g.mob_chains.reset(29);
        for s in 1..g.ent.len() {
            let e = &g.ent[s];
            if e.class64 == 5
                && e.act_life >= 0
                && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                && (e.model65 as usize) < 29
            {
                g.mob_chains.list[e.model65 as usize].push(s as u16);
            }
        }
    }

    fn q22_ctx() -> crate::mc1::mobs::MobCtx {
        crate::mc1::mobs::MobCtx {
            px: 40 * 256,
            py: 40 * 256,
            pz: 400,
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

    /// ⭐⭐⭐ DIG 98-Q22 — **`sub_12500`'s CONTROLLED-SLOT SNAP.**
    ///
    /// `UpdateEntities_57730` walks all 29 per-model class-5 roster
    /// chains before the entity loop and calls `sub_12500` on every
    /// member with a StageVar (EF:40095-40101; `NETHERW.EXE`
    /// 0x7C1A9-0x7C1E4). Its `0xD/0xE/0x10/0x11` jump-table arm
    /// (`NETHERW.EXE` 0x36D8B-0x36DA2) drags the record back to
    /// `8*model + 7` from every phase except attack (2), flee (6) and
    /// the prekill/kill pair (4/5, refused by the outer gate at
    /// 0x36D1F). The port ran this NOWHERE — only a partial mid-walk
    /// paraphrase inside `mc2_alliance_clock`, for StageVar2 14 alone
    /// and only from phases 0/1.
    ///
    /// Non-vacuous: `MGC_NO_SV_CONTROLLED_SLOT_SNAP=1` fails the first
    /// assert.
    #[test]
    fn a_controlled_creature_is_dragged_back_to_its_own_slot_every_tick() {
        let mut g = q22_gen();
        // One wyvern per phase, all StageVar2 13 (Summon Army).
        let mut slots = Vec::new();
        for phase in 0u8..8 {
            let i = g.mc2_spawn_m16(40 * 256, 40 * 256, 400).expect("a pool slot");
            g.ent[i].site_z = 13;
            g.ent[i].tick70 = 16 * 8 + phase;
            slots.push((phase, i));
        }
        // A StageVar2 == 0 creature is never on this arm at all.
        let plain = g.mc2_spawn_m16(40 * 256, 40 * 256, 400).expect("a pool slot");
        g.ent[plain].site_z = 0;
        g.ent[plain].tick70 = 16 * 8 + 1;

        q22_rebuild_mc2_chains(&mut g);
        g.mc2_controlled_slot_snap();

        for (phase, i) in slots {
            let want = if matches!(phase, 2 | 4 | 5 | 6) {
                16 * 8 + phase
            } else {
                16 * 8 + 7
            };
            assert_eq!(
                g.ent[i].tick70, want,
                "phase {phase}: 2/4/5/6 hold, everything else snaps to 8*model+7"
            );
        }
        assert_eq!(g.ent[plain].tick70, 16 * 8 + 1, "StageVar2 0 is untouched");
    }

    /// The same arm serves 14/16/17, and it is the model's OWN base —
    /// `mov 0x40(%ebx),%al ; shl $0x3,%al ; add $0x7,%al`.
    #[test]
    fn the_controlled_slot_snap_serves_all_four_stagevar2_kinds() {
        let mut g = q22_gen();
        let mut made = Vec::new();
        for (model, sv2) in [(16u8, 13u8), (25, 14), (2, 16), (19, 17), (16, 12)] {
            let i = g
                .mc2_spawn_creature_model(model, 40 * 256, 40 * 256, 400)
                .expect("a pool slot");
            g.ent[i].site_z = sv2 as i16;
            g.ent[i].tick70 = model.wrapping_mul(8).wrapping_add(1);
            made.push((model, sv2, i));
        }
        q22_rebuild_mc2_chains(&mut g);
        g.mc2_controlled_slot_snap();
        for (model, sv2, i) in made {
            // 12 (metamorph) is NOT on the arm — its own handler owns
            // the record and `sub_12500`'s table sends it elsewhere.
            let want = if sv2 == 12 {
                model.wrapping_mul(8).wrapping_add(1)
            } else {
                model.wrapping_mul(8).wrapping_add(7)
            };
            assert_eq!(g.ent[i].tick70, want, "model {model} StageVar2 {sv2}");
        }
    }

    /// ⭐⭐⭐ DIG 98-Q22 — **`sub_24510` AIMS AT THE CORPSE AND ONLY
    /// THEN NOTICES IT IS ONE.**
    ///
    /// The lock is guarded by a bare POINTER compare
    /// (`NETHERW.EXE` 0x48E08 `cmp %ecx,%eax` / `jbe`), the roll aim
    /// lands at 0x48E43 (`mov %ax,0x20(%ebx)`), and the life/reap test
    /// is BELOW it at 0x48E47/0x48E4D — only then does 0x48E53 stamp
    /// `actionIndex = 129`. The port had fused both guards into
    /// `mc2_target`, so a wyvern whose victim died this frame kept a
    /// stale `roll` and its heading servo chased the wrong bearing for
    /// the rest of the take.
    ///
    /// Non-vacuous: `MGC_NO_M16_AIM_BEFORE_DEAD=1` leaves `f34` at its
    /// stale value and the first assert fails.
    #[test]
    fn the_wyvern_aims_at_its_victims_corpse_before_it_drops_the_attack() {
        let mut g = q22_gen();
        // The victim: a class-3 carpet, already dead, due north-east.
        let v = g.new_event().expect("a pool slot");
        g.ent[v].class64 = 3;
        g.ent[v].model65 = 1;
        g.link(v, 60 * 256, 60 * 256, 400);
        g.ent[v].act_life = -1;

        let i = g.mc2_spawn_m16(40 * 256, 40 * 256, 400).expect("a pool slot");
        g.ent[i].tick70 = 16 * 8 + 2; // the attack slot
        g.ent[i].f146 = v as u16;
        g.ent[i].f63 = 0; // the 8-tick aim throttle is OPEN
        g.ent[i].f34 = 0; // a stale bearing the port would have kept
        let stale = g.ent[i].f34;

        g.m16_tick(i, &q22_ctx());

        let (mx, my) = (g.ent[i].x, g.ent[i].y);
        let want = Gen::angle_between(mx, my, g.ent[v].x, g.ent[v].y);
        assert_ne!(want, stale, "the fixture must actually move the bearing");
        assert_eq!(
            g.ent[i].f34, want,
            "0x48E43 stores the aim BEFORE 0x48E47 reads the corpse's life"
        );
        assert_eq!(
            g.ent[i].tick70,
            16 * 8 + 1,
            "0x48E53 then drops the attack to actionIndex 129"
        );
    }

    /// Build a hive + one edible neighbour of the model its bucket
    /// selects, and return `(hive, prey, pool length before the
    /// split)`. Row 80's `v_26 = 25` and `f63` is the per-model spawn
    /// ordinal, so a freshly built world's first hive has
    /// `(f63 / 25) % 3 == 0` ⇒ bucket model 4.
    fn q24_hive_and_prey(g: &mut Gen) -> (usize, usize) {
        let hive = g.mc2_spawn_m9(40 * 256, 40 * 256, 400).expect("a hive");
        assert_eq!(
            [4u8, 12, 13][((g.ent[hive].f63 as i16 / 25) % 3) as usize],
            4,
            "the bucket this test feeds"
        );
        // The prey only has to satisfy the sweep's own filter
        // (class 5, the bucket model, alive, not reaped, and not one
        // of the three excused `tick70`s), so build it by hand —
        // model 4 lives in the other roster wave.
        let prey = g.new_event().expect("a prey slot");
        g.ent[prey].class64 = 5;
        g.ent[prey].model65 = 4;
        g.ent[prey].tick70 = 4 * 8 + 1;
        g.ent[prey].act_life = 100;
        let hz = g.ent[hive].z;
        g.link(prey, 40 * 256 + 400, 40 * 256, hz);
        (hive, prey)
    }

    /// The one new `(5,9)` the split minted, given the pool contents
    /// before it ran.
    fn q24_new_hive(g: &Gen, before: &[usize]) -> usize {
        let fresh: Vec<usize> = (1..g.ent.len())
            .filter(|s| {
                g.ent[*s].class64 == 5 && g.ent[*s].model65 == 9 && !before.contains(s)
            })
            .collect();
        assert_eq!(fresh.len(), 1, "the consume sweep splits exactly once");
        fresh[0]
    }

    fn q24_live_hives(g: &Gen) -> Vec<usize> {
        (1..g.ent.len())
            .filter(|s| g.ent[*s].class64 == 5 && g.ent[*s].model65 == 9)
            .collect()
    }

    /// ⭐⭐⭐ DIG 98-Q24 — **THE GROUNDED HIVE'S SPLIT HANDS THE CHILD
    /// ITS OWN BADGE, UNCONDITIONALLY.**
    ///
    /// `sub_20940` (EF:12409-11), shipped `NETHERW.EXE`
    /// **0x45428-0x4543B**:
    /// `call 0x6e990` (`IfSubtypeCallCreatingManaSphere_4A190(&pos,
    /// 5, 9)`) · `mov 0x1a(%ebx),%dx` · `mov %dx,0x1a(%eax)` — no
    /// gate of any kind between the call and the store. The port's
    /// `m9_consume_scan` threw the new record away
    /// (`let _ = self.mc2_spawn_m9(..)`), so the child kept
    /// `NewEvent_4A050`'s own-slot seed.
    ///
    /// Non-vacuous: `MGC_NO_M9_SPLIT_INHERITS_ID=1` leaves the child's
    /// `id24` at its own slot and the first assert fails.
    #[test]
    fn the_grounded_hive_split_always_inherits_its_parents_id() {
        let mut g = q22_gen();
        let (hive, _prey) = q24_hive_and_prey(&mut g);
        // A badge that is NOT a class-3 record — the walking arm would
        // refuse this one, the grounded arm must not.
        g.ent[hive].id24 = hive as u16;
        let before = q24_live_hives(&g);
        g.m9_consume_scan(hive, true);
        let child = q24_new_hive(&g, &before);
        assert_eq!(
            g.ent[child].id24, hive as u16,
            "sub_20940 copies @0x1A with no gate (NETHERW.EXE 0x45434-0x4543B)"
        );
        assert_ne!(
            child, hive,
            "the fixture must actually mint a new record"
        );
    }

    /// ⭐⭐⭐ …AND ITS SIBLING `sub_203D0` DOES NOT — **A SPLIT IN A
    /// SIBLING PAIR.**
    ///
    /// The walking hive's identical five statements (EF:12213-16) are
    /// fenced in the shipped `NETHERW.EXE` at **0x450FE-0x45117**:
    /// `movswl 0x1a(%ebx),%edx` · `mov 0x1a3e4(,%edx,4),%edx`
    /// (`Entities[parent->id]`) · `cmpb $0x3,0x3f(%edx)` — the badge
    /// is passed on only when it already names a class-3 record
    /// (wizard / castle / balloon). A WILD hive's walking split keeps
    /// its own slot; a wizard-owned one does not.
    #[test]
    fn the_walking_hive_split_inherits_only_a_class_three_badge() {
        // (a) a wild hive — the badge names itself, class 5 ⇒ refused.
        let mut g = q22_gen();
        let (hive, _) = q24_hive_and_prey(&mut g);
        g.ent[hive].id24 = hive as u16;
        let before = q24_live_hives(&g);
        g.m9_consume_scan(hive, false);
        let child = q24_new_hive(&g, &before);
        assert_eq!(
            g.ent[child].id24, child as u16,
            "cmpb $0x3,0x3f(%edx) fails ⇒ NewEvent_4A050's own-slot seed stands"
        );

        // (b) the same hive wearing a class-3 owner's badge ⇒ copied.
        let mut g = q22_gen();
        let (hive, _) = q24_hive_and_prey(&mut g);
        let wiz = g.new_event().expect("a pool slot");
        g.ent[wiz].class64 = 3;
        g.ent[wiz].model65 = 1;
        g.link(wiz, 90 * 256, 90 * 256, 400);
        g.ent[hive].id24 = wiz as u16;
        let before = q24_live_hives(&g);
        g.m9_consume_scan(hive, false);
        let child = q24_new_hive(&g, &before);
        assert_eq!(
            g.ent[child].id24, wiz as u16,
            "a class-3 badge passes the gate and is inherited"
        );
    }

    /// **THE 180° TURN TIE-BREAK IS SIGNED, NOT WRAPPED.** Retail's
    /// `sub_582F0` (Sound.cpp:6580; MC1's `sub_42240_42580` :52664 is
    /// the same body, marked SYNCHRONIZED) takes the plain integer
    /// difference of the two masked angles and unwraps it only when
    /// `abs(v3) > 1024` — STRICTLY greater. An exact half-turn keeps
    /// the raw sign, so a target numerically BELOW the current heading
    /// turns negative and one above turns positive; every other delta
    /// agrees with the wrapped form.
    ///
    /// The two rows this pins are the mc2l24 (5,23) dweller's, both on
    /// slot 363, the residue the riser-replay dig left behind. The move
    /// core's THIRD retry is the antipode (`yaw0 + 0x400`, EF:8846),
    /// and the dweller's wander target (`roll_0x20_32`) still equals
    /// its pre-retry heading, so the commit turn lands exactly on the
    /// tie every time that leg fires:
    ///
    /// | pair | yaw0 = target | retry-3 leg | retail | wrapped form |
    /// |---|---|---|---|---|
    /// | t=15044 | 437 | 1461 | **1205** | 1717 |
    /// | t=15129 | 519 | 1543 | **1287** | 1799 |
    ///
    /// Non-vacuous: `MGC_NO_TURN_TIE=1` restores the wrapped sign and
    /// this test fails on the first assert.
    #[test]
    fn turn_step_breaks_the_exact_half_turn_toward_the_lower_angle() {
        // The recorded (5,23) pairs, row 91's turn cap = 256.
        for (yaw0, leg, want) in [(437u16, 1461u16, 1205i32), (519, 1543, 1287)] {
            assert_eq!(
                yaw0.wrapping_add(0x400) & (0x700 + (yaw0 & 0xFF)),
                leg,
                "retry 3 is the antipode for this yaw"
            );
            let turned = leg as i32 + Gen::turn_step(leg, yaw0, 256) as i32;
            assert_eq!(turned & 0x7FF, want, "half-turn back onto the target");
        }
        // The sign is the RAW difference's, both ways round the tie.
        assert_eq!(Gen::turn_sign(1461, 437), -1, "target below → negative");
        assert_eq!(Gen::turn_sign(437, 1461), 1, "target above → positive");
        // ...and every non-tie delta is unchanged by the law.
        for (cur, tgt) in [(0u16, 1u16), (0, 2047), (0, 1023), (0, 1025), (100, 1500)] {
            let wrapped = if tgt.wrapping_sub(cur) & 0x7FF <= 1024 {
                1
            } else {
                -1
            };
            assert_eq!(
                Gen::turn_sign(cur, tgt),
                wrapped,
                "only the exact half-turn moves ({cur} → {tgt})"
            );
        }
    }
}
