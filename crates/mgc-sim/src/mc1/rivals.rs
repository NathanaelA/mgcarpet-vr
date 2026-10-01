//! Hostile (AI) wizards — the class-3 model-1 rival carpets: spawn,
//! per-tick brain, casting arm, mortality and respawn. Direct port of
//! the remc1 AI wizard machinery; all citations remc1 sub_main.cpp.
//! The full trace record lives in docs/ROADMAP.md "HOSTILE WIZARDS
//! (RIVAL AI) — TRACE BANK".
//!
//! Architecture mirrors the original: the AI is a DECISION LAYER over
//! the shared engines — rivals cast through the same class-12
//! manifestation entities, spawn the same class-9/10 projectiles and
//! effects (owner = the rival's wizard entity slot, so the generic
//! combat plumbing serves them unchanged), obey the same mana economy
//! (census ceiling, spell costs, castle-stored thresholds, the debit
//! riding the regen delta), and die into the same jar-scatter + grave
//! sequence as the human.
//!
//! Retail AI-vs-human asymmetries, ported faithfully: AI heals 4x the
//! human's afield rate (:18013 vs :55418); AI at its castle DISCARDS
//! damage instead of forwarding it to the castle (:17975-79); the
//! AI's first castle is spawned directly, free and instant
//! (:19200-08); the AI carpet ignores walls, drag and knockback
//! (sub_14EB0 runs neither the wall gate nor the knock fields); AI
//! target scans are omniscient; the AI learns spells on a 200-tick
//! timer from any jar existing in the world instead of picking jars
//! up (:64805-14, :19381-443).
//!
//! Interim deviations (ours, flagged inline): the hate feed runs at
//! damage-intake and homing-acquisition time instead of the
//! original's per-projectile one-shot ledger scan (sub_16540 —
//! equivalent inputs, slightly later timing); the duel pull on the
//! CASTER is applied through the knock channel (magnitude from the
//! traced formula). Creature target scans now walk the full class-3
//! bucket[0] list (`Gen::nearest_wizard_target`) — carpets, castles
//! and balloons for the wyvern/crab/mound/guard, carpets-only for the
//! genie — so wild creatures fight rival wizards, not just the human.
//! The m4 militia and m8 griffon village-wanted gates are per-wizard
//! too: `Gen::rival_wanted` mirrors `player_aggro` for the rivals
//! (armed by a rival's own village offenses), so villages turn their
//! defenders on any hostile wizard (see docs/DEVIATIONS.md).

use crate::engine::features::{Gen, HashSilent};
use crate::engine::world::{LifeState, World};
use crate::mc1::behavior::BEHAVIOR;
use crate::mc1::mobs::PLAYER_TARGET;
use crate::mc1::spells::{SPELL_COUNT, SPELLS};

/// `MGC_NO_MC1_RIVAL_TOKEN_GATE_LIVE_PURSE=1` — see
/// [`crate::engine::features::no_mc1_rival_token_gate_live_purse`].
#[inline]
fn live_purse_legacy() -> bool {
    crate::engine::features::no_mc1_rival_token_gate_live_purse()
}

/// ⭐ THE RAID ARM READS THE OWNER'S CARPET RECORD RAW (`sub_143A0`
/// :18517-18). Set `MGC_NO_RAID_OWNER_RAW_POS=1` to restore the
/// pre-dig behaviour, where the defender lookup went through
/// [`World::wizard_pos`] and its invented liveness gates
/// (`LifeState::Alive` for the human, `tick70 == 1` for a rival) —
/// which made every castle whose owner was mid-death-fall read
/// UNDEFENDED. See `rival_pick_castle_target`.
pub(crate) fn raid_owner_pos_is_raw() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_RAID_OWNER_RAW_POS").is_none())
}

/// ⭐ THE BALL-CLAIM GUARD ELECTION SEATS THE HUMAN BY TICK-TOP
/// BUCKET[0] MEMBERSHIP (`sub_15340` :19003 walks the chain rebuilt at
/// :52290-97). Set `MGC_NO_BALL_GUARD_BUCKET=1` to restore the pre-dig
/// behaviour, where the seed came from [`World::wizard_pos`] and its
/// LIVE `LifeState::Alive` test — which dropped a human who died
/// mid-tick out of the election and unguarded the balls beside him.
/// See `rival_pick_ball_target`.
pub(crate) fn ball_guard_is_tick_top() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_BALL_GUARD_BUCKET").is_none())
}

/// ⭐⭐⭐ **THE GUARD ELECTION EXCLUDES THE CANDIDATE'S OWN `+24`, AND
/// A HUMAN-OWNED MANIFESTATION CARRIES THE HUMAN'S `+24`.**
///
/// `sub_15340` opens `v5 != *(_WORD *)(a1 + 24)` — a1 being the
/// CANDIDATE the ball claim is scoring, not the wizard — so any
/// class-3 chain member sharing the candidate's id is barred from
/// guarding it. Disassembled at file offset 0x2db5c (VA 0x15340 +
/// 0x187f8 = 0x2db38): `mov 0x14(%esp),%edx; mov 0x18(%ebx),%ax;
/// cmp 0x18(%edx),%ax; je <skip>`.
///
/// On an ordinary mana ball `+24` is the ball's own slot and the test
/// never fires. But the tick-top ball chain is walked LIVE: a slot it
/// held at the tick top can have been recycled into something else by
/// the time a rival brain reaches it, and `sub_15080` reads that
/// record raw. A human-cast BALLOON stamps `+24` with the CASTER's
/// carpet id — so retail's guard election drops the human himself and
/// the balloon reads UNGUARDED however close he is standing.
///
/// The port seats the human OUTSIDE the chain walk (he rides
/// out-of-pool), and that seat skipped the id test the loop applies to
/// every pooled member — so the human guarded his own balloon and the
/// claim was refused. mc1l48 t=23213: the human mints balloon 249 over
/// his castle; retail's Adhab (712) claims it (`+146` 690 -> 249,
/// `+148` 1068 = 681 + 3 + (3 << 7)), the port found him 1,920 units
/// off it, called it guarded, dropped out of the ball arm entirely and
/// fell through to the mana hunt (creature 43, 5,000 mana).
///
/// Set `MGC_NO_BALL_GUARD_OWN_ID=1` to restore the pre-dig behaviour.
pub(crate) fn ball_guard_excludes_own_id() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_BALL_GUARD_OWN_ID").is_none())
}

/// ⭐⭐⭐ **THE WAR TEST'S TEAM INDEX IS READ THROUGH THE TAG
/// RECORD'S `+160`, AND EVERY NON-WIZARD RECORD CARRIES `NewEvent`'s
/// DEFAULT WIZEXT — WHOSE `+48` IS A BSS ZERO. A CLASS-3 TAG THAT IS
/// NOT A WIZARD THEREFORE SCORES AGAINST `hate[0]`, THE HUMAN.**
///
/// The war test is one expression shared by the ball claim
/// `sub_15080` (`reference/remc1/sub_main.cpp:18895`) and the castle
/// raid `sub_143A0` (:18514):
///
/// ```text
/// 50000 - tagrec->+136 / 10 * me->wizext->+522 / 255
///        < me->wizext->[460 + 8 * tagrec->+160->+48]
/// ```
///
/// Both operands come off the TAG RECORD, not off a wizard seat.
/// `CARPET.EXE` VA 0x15080 = file **0x2D878**, the ball claim's copy,
/// verbatim from the shipped binary:
///
/// ```text
/// 2d8f6  8a 8a a3 74 00 00      mov  0x74a3(%edx),%cl   ; tagrec +64 class
/// 2d900  80 f9 03               cmp  $0x3,%cl
/// 2d903  0f 85 11 01 00 00      jne  0x2da1a            ; -> the wild arm
/// 2d915  8b 82 03 75 00 00      mov  0x7503(%edx),%eax  ; tagrec +160  (29955)
/// 2d921  0f bf 40 30            movswl 0x30(%eax),%eax  ; ITS +48, SIGNED
/// 2d925  66 8b 84 c5 cc 01 00 00 mov 0x1cc(%ebp,%eax,8),%ax ; my hate[that]
/// 2d935  8b 82 eb 74 00 00      mov  0x74eb(%edx),%eax  ; tagrec +136  (29931)
/// 2d93d  b9 0a 00 00 00 / f7 f9 idiv $10
/// 2d947  0f bf 95 0a 02 00 00   movswl 0x20a(%ebp),%edx ; my +522 (agg)
/// 2d95b  f7 fd                  idiv $255
/// 2d95d  ba 50 c3 00 00         mov  $0xc350,%edx       ; 50000
/// 2d962  29 c2                  sub  %eax,%edx
/// 2d966  3b 04 24 / 7d 1f       cmp (%esp),%eax ; jge   ; SIGNED, -> neutral arm
/// ```
///
/// `+160` is written in exactly two places. `NewEvent`'s two arms
/// (`sub_37220_375E0`, :43878 and :43903) stamp **every** record with
/// `unk_B7330_B7320` — a static `Type_160[30]` at VA 0xB7330 (MC1) /
/// 0xB7320 (HW), and `LE` object 3 (base 0x90000, 29 initialised
/// pages) only reaches VA 0xAD000, so that array is **BSS: zero at
/// load**. Nothing in the listing writes its `+48` (the writes that go
/// through a record's `+160` land at `+460+8i`, `+462+8i` and `+528`).
/// The only real wizexts are attached to the WIZARD CARPETS
/// (`:52173` at level load, `:54866` on respawn:
/// `v2x->var_u32_29955_160 = &a1x->str_1103;` with
/// `->var_48 = a1x - str_13323`, the wizard index; `str_13323`'s
/// stride is 2049, which is exactly the stride of the `owner_ptr`
/// words the recorder captures for the four carpets of mc1hwl5).
///
/// The port's `owner_slot` resolves a tag only to a seated wizard and
/// returned `None` otherwise, so the ball claim FELL THROUGH to the
/// neutral arm (carpet-guard + castle-overlap vetoes, scored from the
/// WIZARD) where retail takes the at-war arm (no vetoes, scored from
/// the castle REGISTER).
///
/// WITNESS mc1hwl5 t=25348..25486 (123 of the take's 135 heads, all
/// slot 386 `chase`): rival 2's wizext holds `agg 255` and a flat
/// `hate[..] = 24607`; **33 of the 76 balls on the tick-top chain are
/// tagged `+144 = 7`**, and slot 7 is a `(3,3)` keep of wizard 411 —
/// class 3, so retail takes the wizard arm, but not a wizard, so its
/// `+160` is the default. Retail's threshold is
/// `50000 - 1062574/10 * 255/255 = -56257 < hate[0]` → at war for all
/// 33, scored from his castle register 962 at (16384, 0), where ball
/// **50** is the global minimum (75,952,724 against 96,927,557 for the
/// ball 665 the port's from-the-wizard neutral arm elected — 665 sits
/// 736 units from the wizard and 50 sits 12,915 away).
///
/// ⚠ RESIDUAL, NOT FIXED HERE: `hate_over` clamps the threshold with
/// `50_000u32.saturating_sub(..)` where 0x2d962 is a plain signed
/// `sub` and 0x2d969 a signed `jge`. The two disagree only when the
/// product exceeds 50000 AND `hate[team] == 0` (retail: at war; port:
/// not). No corpus witness — mc1hwl5's hate is 24607 throughout.
///
/// `MGC_NO_MC1_DEFAULT_WIZEXT_TEAM=1` restores the `None` fall-through.
pub(crate) fn default_wizext_team() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_DEFAULT_WIZEXT_TEAM").is_none())
}

/// The war threshold's SIGNED, WRAPPING 32-bit arithmetic — see
/// [`World::hate_over`] for the shipped bytes and the witness.
/// `MGC_NO_MC1_HATE_SIGNED_WRAP=1` restores the u32 saturating form.
pub(crate) fn hate_signed_wrap() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_HATE_SIGNED_WRAP").is_none())
}

/// ⭐⭐⭐ **THE WAR CLEAR TESTS THE TARGET'S MODEL BYTE AND NOTHING
/// ELSE — SHOOTING A CREATURE DISCHARGES THE RIVAL'S GRUDGE.**
///
/// `sub_13DD0`'s tail (`reference/remc1/sub_main.cpp:18337-39`) ends a
/// successful cast with
///
/// ```c
/// if ( *(_BYTE *)(v1 + 65) <= 1u )
///   *(_WORD *)(*(_DWORD *)(a1 + 160) + 8 * *(__int16 *)(*(_DWORD *)(v1 + 160) + 48) + 462) = 0;
/// ```
///
/// where `v1 = pool[a1->+146]` is the TARGET record. THE SHIPPED
/// `CARPET.EXE` SETTLES IT — `sub_13DD0` = VA 0x13DD0 = file 0x2C5C8,
/// and the tail at file 0x2C683 is:
///
/// ```text
///   2c683  8a 66 41                    mov  0x41(%esi),%ah        ; TARGET +65 = MODEL
///   2c686  84 e4                       test %ah,%ah
///   2c688  74 05                       je   2c68f                 ; model 0 -> clear
///   2c68a  80 fc 01                    cmp  $0x1,%ah
///   2c68d  75 1a                       jne  2c6a9                 ; model > 1 -> skip
///   2c68f  8b 86 a0 00 00 00           mov  0xa0(%esi),%eax       ; TARGET's wizext, UNGUARDED
///   2c695  0f bf 48 30                 movswl 0x30(%eax),%ecx     ; wizext+48 = player slot
///   2c699  8b 83 a0 00 00 00           mov  0xa0(%ebx),%eax       ; MY wizext
///   2c69f  66 c7 84 c8 ce 01 00 00 00 00   movw $0,0x1ce(%eax,%ecx,8)  ; 0x1ce = 462 = war
/// ```
///
/// There is NO class test: `[esi+0x40]` is never read anywhere in the
/// function (all three encodings searched, 0 hits). The port had
/// invented `class64 == 3 && model65 <= 1` and then routed the index
/// through [`World::owner_slot`], which returns `None` for a creature
/// — so a rival that spent its whole life shooting creatures NEVER
/// discharged its war flag, [`World::rival_hate_decay`]'s `!war[p]`
/// gate stayed shut and its hate ledger PINNED at 65,535 forever.
///
/// ⚠ A creature's `+160` is the mint's NULL, so retail's `movswl
/// 0x30(%eax)` reads low memory and the index resolves to 0 — the
/// HUMAN's row. This is the unguarded-pointer constant class the
/// `rival_war_check` threshold ruling already names. mc1l6 t=9525 is
/// the measurement: the rival's `+146` is slot 25, a `(5,1)` creature,
/// its cast lands, and RETAIL's `hate[0]` breaks out of its war pin on
/// that exact boundary (t=9525 65535 -> t=9526 65373, a clean -162 =
/// -(256 - agg)) and decays for the next 300 ticks. The port held
/// 65,535 and its ball election then read `hate[0] > 47,153` TRUE,
/// taking the human-owned ball 301 (2.4 tiles off) through the at-war
/// arm where retail took the wild ball 131 (107 tiles off) through the
/// wild arm — mc1l6's whole 9,816 horizon.
///
/// Set `MGC_NO_MC1_WAR_CLEAR_MODEL_ONLY=1` to restore the pre-dig
/// class-3-only clear.
pub(crate) fn war_clear_is_model_only() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_WAR_CLEAR_MODEL_ONLY").is_none())
}

/// ⭐⭐⭐ **THE OUT-OF-POOL HUMAN IS A CLASS-3 ROSTER MEMBER, AND A
/// BLANKED OR SEVERED ROSTER HIDES HIM FROM THE WIZARD PICK.**
///
/// `sub_145B0` (:18554) elects its victim from `var_u32_36462[0]` and
/// NOTHING else — the human's carpet is an ordinary record on that
/// chain, so [`Gen::new_event`]'s mid-tick seizure blank (:43885-91,
/// which NULLS all four roster heads for the rest of the tick) and the
/// severed-chain law hide him exactly as they hide every pooled
/// carpet. The port carries the human OUT OF THE POOL and judged him
/// in a pre-pass placed OUTSIDE the chain walk, so a blanked roster
/// left him standing as the only candidate in the world and the rival
/// re-elected him on a tick retail could see nobody at all.
///
/// Retail's chain is ascending slot order, so the human's index in it
/// is the number of PORT-list members below his slot, and
/// `TickChain::cut` — measured in the port's list, which is one member
/// shorter — compares correctly against that index either way round: a
/// sever BELOW him yields `cut <= hpos` (hidden), a sever ABOVE him
/// leaves `cut > hpos` (visible). A native world has no carpet slot, so
/// index 0 degrades the rule to the blank alone (`cut == 0`), which is
/// all a world with no pooled carpet can express.
///
/// WITNESS mc1l48 t=26739 / 26741-26744 / 26747 / 26750 (wizard 2,
/// carpet slot 712, `+522` tempo 252 ⇒ think period 1, so the cascade
/// re-decides EVERY tick): a seizure blanks the rosters, retail finds
/// no candidate anywhere, and the cascade falls through arms 4-8 to
/// Idle → CRUISE (`+415` 8 → 12). `sub_13A10`, the Cruise handler,
/// stamps NO `+34` — so retail's `target_yaw` HOLDS across those ticks
/// while the port, still in AttackWizard, re-aimed every one of them.
/// The same blank already parks wizard 1 (`+415` 6 → 12) on exactly
/// those seven ticks in BOTH engines, because its arm is the ball
/// claim and the port's ball chain is chain-gated already.
///
/// `MGC_NO_MC1_HUMAN_ON_WIZ_ROSTER=1` restores the pre-dig pre-pass.
pub(crate) fn mc1_human_on_wiz_roster() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_HUMAN_ON_WIZ_ROSTER").is_none())
}

/// ⭐⭐ **THE JAR POLL IS A BUCKET[0] WALK TOO — A BLANKED ROSTER STAMPS
/// NOTHING, HITS NOBODY, GRANTS NOTHING.**
///
/// `sub_55A40_55F70` (:64729, CARPET.EXE file 0x6E238) does every one of
/// its three human-facing jobs INSIDE one walk of `var_u32_36462[0]`
/// (`mov 0x8e6e(%eax),%eax` at 0x6E2BA, the loop tail 0x6E4E3-0x6E4FE):
/// the "already known" bit-0 stamp (`cmpw $0,0x2a4(%edx,%eax,2)` at
/// 0x6E2FC, +676[model]), the AABB pickup (`call 0x2a148` = sub_11950
/// at 0x6E315) and, after the break, the rivals' learn-timer arm. The
/// human carpet is an ordinary node on that chain, so the mid-tick
/// seizure blank ([`Gen::new_event`] :43885-91 nulls all four heads for
/// the rest of the tick) and the severed-chain cut hide him from the
/// poll exactly as [`mc1_human_on_wiz_roster`] hides him from the wizard
/// pick. The port's poll read the out-of-pool human straight off
/// `player.state`, so on a dry-pool tick (999 live, every spawn a
/// sacrifice) it stamped bit 0 on a jar retail could not see the human
/// from.
///
/// WITNESS mc1l24 pair 15897→15898 (and 15898→15899, 16033→16034,
/// 16034→16035, 16037→16038, 16038→16039): rival 576 dies at 15896 and
/// scatters jars 577..600; the recycle stack drains 413 → 337 → 217 →
/// 118 → 12 → 0 over 15896..15901 (a spell storm at pool 999). Jars
/// whose `+63 & 3` poll lands on 15896/15897 stamp `flags 4 → 5` in both
/// engines (579, 578); the ones polling on 15898..15901 — 577/581/…/597
/// at 15898, 580/584/…/600 at 15899 — hold 4 in retail and only stamp on
/// their NEXT poll (15902/15903), when the stack is empty and nothing
/// seizes; the port stamped every one on the first poll. Same shape on
/// the 16032 scatter (528..549: polls on 16034..16041 refused, 16042+
/// stamp), and on mc1l23 t=8022, mc1l21 t=10290, mc1l18 t=10552.
///
/// `MGC_NO_MC1_JAR_POLL_ROSTER=1` restores the pre-dig `player.state`
/// read on both call paths (the strict-retail poll and the native
/// `try_pickup` gate).
pub(crate) fn mc1_jar_poll_roster_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_JAR_POLL_ROSTER").is_none())
}

/// ⭐⭐⭐ **THE OWNED REBUILD IS AN UNBOUNDED INDEXED WRITE**
/// (`sub_45C10_45F50` :55310-19). The decompile's own original line —
/// preserved as a comment beside the hand-converted one — is
/// `*(_WORD *)(v2 + 2 * *(char *)(164 * v3 + POOL + 65) + 676) =
/// *(_WORD *)(v2 + result + 532)`: no class filter, no bounds check,
/// and the model byte read as a SIGNED `char`. `+676` (the owned-spell
/// book) is only 24 entries wide, and `+724` — the AI RECAST COOLDOWN
/// table — sits immediately after it, `+628` (the learn countdowns)
/// immediately before. So an acquisition entry whose pool slot has
/// been RECYCLED into a record that is not a class-12 manifestation
/// writes the slot number into a NEIGHBOURING array.
///
/// And it is not a one-shot: the rebuild runs at the TOP OF EVERY
/// DISPATCH (:17969), just AFTER the cooldown decay loop (:17939-45),
/// so the stray write is RE-STAMPED every tick — whatever it lands on
/// is PINNED FOREVER.
///
/// mc1l49 t=36793, rival 594 (wizard 1): `acq[3]` still names pool
/// slot 26, which had just been recycled into a class-10 model-39
/// record. `676 + 2*39 = 754 = 724 + 2*15`, so the write puts the slot
/// number **26** into `cooldown[15]` — Lightning — and the recording
/// holds it at exactly 26 on every tick from 36793 to the end of the
/// take while `cooldown[0]/[8]/[17]/[20]` decay normally around it
/// (and wizard 4, the same tick, shows a healthy `cooldown[15] 1 -> 0`
/// off `AI_RECAST[15] = 1`). Retail's rival can never cast Lightning
/// again; the port's `m < SPELL_COUNT` guard was INVENTED, dropped the
/// stray write, and let it fire — and in `AiState::RaidBalloon` a
/// FIRED cast replaces the arrival hover step, so the port's z ran
/// exactly one `-v_14` (= 4) below retail's. MEASURED: mc1l49
/// `--segmented` 85 segments / 84 excess resets -> 62 / 61, the
/// whole slot-594 run t=36854..38078 gone, every one of them `z`
/// low by exactly 4.
///
/// `MGC_NO_MC1_OWNED_REBUILD_OVERFLOW=1` restores the pre-dig bound.
pub(crate) fn owned_rebuild_overflow() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_OWNED_REBUILD_OVERFLOW").is_none())
}

/// ⭐⭐ **MC1'S ONE CRT `rand()` DRAW SITS AT `LABEL_49`, BEHIND THE
/// 17/8 ARMS** (`sub_16030` :19492-19508). Retail's attack picker
/// walks Undead Army (17) then Volcano (8) and `return`s out of the
/// function the moment one is castable — or holds on it — so
/// `rand()` is simply never reached on those ticks. The port used to
/// roll the anti-rebound plan BEFORE walking 17/8, which spent a draw
/// retail never spends; because MC1 has exactly ONE `rand()` call
/// site and no `srand()`, every such phantom draw permanently shifts
/// the whole global Watcom stream relative to retail's.
///
/// `MGC_NO_MC1_CRT_DRAW_SITE=1` restores the pre-dig ordering (roll
/// first, then walk). See `World::rival_attack_pick`.
pub(crate) fn mc1_crt_draw_at_label49() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_CRT_DRAW_SITE").is_none())
}

/// `MGC_MC1_CRT_TRACE=1` — one stderr line per CRT draw
/// (`[crt] t= ri= ent= acc= roll= pass=`), the instrument the phase
/// recovery is fitted on.
pub(crate) fn crt_trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_MC1_CRT_TRACE").is_some())
}

/// One arm of `sub_16030`'s cascade — see `World::rival_arm`.
enum ArmStep {
    /// retail's `goto LABEL_5x`
    Next,
    /// retail's `return s` / fall-out-to-`return -1`
    Stop(Option<usize>),
}

/// MC1's mana-steal credit has NO CEILING — see
/// [`World::credit_wizard_mana`].
/// `MGC_NO_MC1_RAW_STEAL_CREDIT=1` restores the pre-dig `.min()`.
pub(crate) fn mc1_raw_steal_credit() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RAW_STEAL_CREDIT").is_none())
}

/// ⭐⭐⭐ **THE HUSK WATCH AND ITS GATE.** A state-3 wizard runs the
/// watch aim (`sub_463B0` :55575-91) — every tick it re-aims
/// `+34`/`+36` at the pool record named by `+38` (the KILLER slot)
/// and steps `+30` toward `+34` at 22/tick — but ONLY down
/// `sub_46480`'s ELSE arm (:55621-29), i.e. only when the wizard's
/// PLAYER ROSTER byte `str_13323[wizext->var_48].var_u8_13332_9` is
/// NOT 1 (CARPET.EXE `cmp byte [esi+0x3414],1` at file 0x5eca4). That
/// byte is set to 1 for every non-`var_u16_8` row by the per-level
/// roster reset and by NOTHING else — so the arm is the HUMAN's
/// death camera, not a rival's, and the companion
/// `sub_44BE0_44F20(a1, 7)` (:54767-70) confirms it: it writes the
/// global view-mode byte and only for the local player's row.
///
/// A rival can nevertheless fall into it, because the death scatter
/// overflows a 24-byte array with a MODEL index and lands on the next
/// player's copy of that byte — see [`World::rival_death_impact`],
/// which is where [`Rival::human_driven`] gets set. `MGC_NO_HUSK_WATCH=1`
/// retires the whole law (no overflow write, no watch), restoring the
/// frozen husk the port shipped.
pub(crate) fn husk_watch_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_HUSK_WATCH").is_none())
}

/// ⭐⭐⭐ THE SPEED-0 SINK OF THE SHARED MOVER (`sub_455D0` :55171-72)
/// runs on the RIVAL's state-2 death fall too — see
/// [`World::rival_death_fall`]. `MGC_NO_DEATH_SINK=1` restores the
/// sink-less fall the port shipped.
pub(crate) fn death_sink_runs() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_DEATH_SINK").is_none())
}

/// ⭐⭐⭐ THE SHARED MOVER'S COMMIT GATE RUNS ON THE RIVAL'S DEATH FALL
/// TOO. `sub_45FC0` (state 2, `reference/remc1/sub_main.cpp:55463`)
/// opens with `sub_455D0`, and that mover ends in `v26 = sub_45410(a1x);
/// if (v26) sub_41C70(a1x, &word_AE454_AE444);` (:55250-52 — EXE
/// 0x5E26A `call 0x5DC08; test ax,ax; je +0x0E; call 0x5A468`). The
/// gate (:55065-107) refuses any scratch whose tile is the type-8 wall
/// (`sub_11810 == 0x100`, 0x5DC29 `cmp $0x100,%eax`) and retries the
/// two cardinals adjacent to the move bearing, each re-seeded from the
/// pose and scaled by `dist·(512-Δ)>>9`; both blocked ⇒ nothing
/// commits, though the refused second cardinal stays in the scratch
/// (the trail puff's seat) and the trailing z-floor still lifts it.
/// The port ran the gate for the HUMAN's fall only
/// ([`Gen::player_wall_slide`]) and let a rival's corpse drift straight
/// through the wall. mc1l12 t=901→902 (slot 109, wizard 2's corpse):
/// retail's y freezes at 19966 for the whole 15-tick fall while x
/// steps 33, 35, 38 … (the cardinal-512 projection of a (46,39) step
/// = 60·(512−229)>>9 = 33), the port stepped (46,39) into tile row 78.
/// mc1l9 t=5769→5770 (slot 114) is the same shape at y = 37023.
/// `MGC_NO_MC1_RIVAL_FALL_WALL_GATE=1` restores the ungated fall.
pub(crate) fn rival_fall_wall_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_FALL_WALL_GATE").is_none())
}

/// ⭐ THE NATIVE HUMAN IS SEATED IN HIS TILE CHAIN WHERE HE IS MINTED.
/// Retail's carpet is linked the moment `sub_44D30` (MC2:
/// `AddPlayer_4A920`) pops it, so everything minted on his tile after
/// him — his own book's tokens first — head-inserts AHEAD of him and he
/// ends the prologue at the chain's TAIL. The port's out-of-pool seat
/// (`features::PlayerChain`) stayed unseeded through the constructor,
/// and the first walk then took him for a fresh arrival: HEAD of his
/// tile, the last token his successor. Found by the captured entity
/// index (round 166): record 0 reads successor 0 on all 115 whole
/// takes, the native build's seat was unseeded on all 115.
/// `MGC_NO_NATIVE_HUMAN_SEAT=1` restores the unseeded constructor.
pub(crate) fn native_human_seat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_NATIVE_HUMAN_SEAT").is_none())
}

/// ⭐ THE FALLING CORPSE IS RELINKED BEFORE ITS TRAIL PUFF IS MINTED.
/// `sub_45FC0` (`reference/remc1/sub_main.cpp:55463-84`) opens with the
/// mover `sub_455D0`, whose commit relinks the record (`sub_41C70`,
/// :55250-52), and only then mints the `(10,1)` puff (:55480). The port
/// minted first and relinked after, so on every tick the corpse crossed
/// into a new tile the puff sat BEHIND the carpet in that tile's chain
/// where retail walks it first. Found by the captured entity index
/// (round 166, `MGC_INDEX`): 10,883 chain rows on 62 MC1/HW takes, all
/// of them this one pair; witness mc1hwl15 t=1030 cell (204,101),
/// retail `[35, 234]` port `[234, 35]`. No graded field reads it.
/// `MGC_NO_MC1_FALL_RELINK_FIRST=1` restores the old order.
pub(crate) fn fall_relink_first() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_FALL_RELINK_FIRST").is_none())
}

/// ⭐⭐⭐ THE OWNED REGISTER HAS EXACTLY ONE WRITER IN THE WHOLE MC1
/// BINARY — `sub_45C10_45F50` (`reference/remc1/sub_main.cpp:55304-20`,
/// twin `remc1hw:51372`). A grep of every `+676` reference in the
/// decompile (:18773, :19407, :20672, :20707, :20732, :26921, :26966,
/// :26967, :48843, :54662, :54669, :54969, :55871-72, :55914-15,
/// :56629, :64794, :64813) finds READS at every site but two, and both
/// of those are inside `sub_45C10` itself: the `memset(..., 0, 48)` at
/// :55310 and the re-register at :55318.
///
/// In particular **the death scatter does NOT blank it**.
/// `sub_45FC0_46300`'s impact arm (:55516-49) walks the `+532`
/// ACQUISITION list, rewrites each entry to its token's model byte (or
/// −1), scatters the jar and brands the roster overflow — and never
/// touches `+676`. Because `sub_45C10` is reached only from the class-3
/// dispatch table's live arms (`str_254ADC[0]` = `sub_45C90_45FD0`
/// :55342 for the human, `str_254ADC[1]` = `sub_13170` → `sub_132B0`
/// :17969 for a rival) plus the two respawn calls (:54962, :55033),
/// a corpse's `+676` FREEZES at the book its last live tick published
/// and stays readable — by `sub_47EC0`'s castle probe (:56629), by
/// every rival's Create-Castle / mana-ball election (:54662, :54669),
/// by the trigger walk (:64794) — for the whole husk window.
///
/// The port blanked it in the scatter, an INVENTED WRITE, so a dead
/// rival's book vanished a tick early. WITNESS (mc1l49, the raw
/// wizext shadow over 9404-11604): all five `owned` divergence ticks
/// are death-scatter ticks — t=9640→9641 wiz 2 (ent 620) and
/// t=9645→9646 wiz 6 (ent 724), t=10392→10393 both, t=10693→10694
/// wiz 6 — each one ALL 24 indices at once, retail holding the intact
/// slot book (621..644 for wiz 2) against the port's zeros, with the
/// `+532` list on both sides rewritten to models by the same scatter.
/// The HUMAN's half was already right: `World::mc1_owned_rebuild` is
/// gated on `LifeState::Alive` and no human death path blanks the
/// book. `MGC_NO_MC1_OWNED_SCATTER_KEEP=1` restores the blank.
pub(crate) fn owned_survives_scatter() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_OWNED_SCATTER_KEEP").is_none())
}

/// ⭐ THE HOME ARM NEVER WRITES THE BRAIN BYTE. `sub_13A70`
/// (`reference/remc1/sub_main.cpp:18204-27`, CARPET.EXE 0x13A70-0x13B94)
/// cloaks, gates on the castle's signature (`sub_15440`, 0x13AEA) and,
/// when it passes, stamps `+34 = angle(me → castle)` (0x13B02 `call
/// 0x42150`, 0x13B16 `mov %ax,0x22(%ebx)`) and runs the travel helper
/// — there is no read of `+40`/`+8` and no write of wizext `+415`
/// anywhere in it. Retail's ONLY exits from Home are the cascade's
/// (`sub_136C0`): the hurt re-pick every tick and the think-tick
/// ladder, whose idle leg (:18749-62) chooses Cruise once life is
/// full. The port's Home arm INVENTED `act_life >= max_life ⇒ Fresh`,
/// and a Fresh rival's handler is empty — so from the heal-complete
/// tick to the next think tick the port stopped re-aiming `+34` while
/// retail kept tracking the castle bearing off its moving pose.
/// WITNESS mc1l26 t=4973: slot 603 (3,1) heals to 10000/10000 in Home
/// (target = castle 612), the port drops to Fresh, and at t=4975 the
/// bearing crosses an ATAN rung — retail 1675, port frozen at 1674
/// (again t=4982 1676 vs 1675, t=13970 61 vs 60); mc1l26-froze
/// t=22198 1848 vs 1849 is the same shape. The retail row is exactly
/// `Gen::angle_between(pose_t, castle)` on every tick of the window.
/// `MGC_NO_MC1_HOME_KEEPS_STATE=1` restores the invented Fresh drop.
pub(crate) fn home_keeps_state() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_HOME_KEEPS_STATE").is_none())
}

/// ⭐ THE UPGRADE ARM READS `+146` UNDER THE SIGNATURE GATE. `sub_13800`
/// (`reference/remc1/sub_main.cpp:18106-32`, CARPET.EXE 0x13800-0x13889)
/// opens `mov 0x92(%esi),%bx` (the wizard's OWN `+146`), calls
/// `sub_15440` (0x13839 → 0x15440: `sub_15420(castle) == +148`, the
/// `id24 + model + class<<7` signature) and on a miss returns 0 with
/// no write at all; on a hit it stamps `+34 = angle(me → castle)`
/// (0x13851 `call 0x42150`, 0x13865 `mov %ax,0x22(%esi)`) and runs
/// the travel helper. Nothing in it tests the castle's flags or `+70`,
/// and nothing writes wizext `+415`. The port instead scanned the pool
/// for a LIVE castle (`rival_castle`, which refuses `flags & 0x400`)
/// and, finding none, INVENTED a drop to Fresh — so the tick a rival's
/// castle is razed under it, retail keeps aiming at the ruin (same
/// slot, owner, class, model ⇒ same signature) while the port's `+34`
/// froze. WITNESS mc1l16 t=19162: castle 52 razed (flags 14 → 1038,
/// f70 6 → 4), rival 351 in Upgrade with target 52 — retail 1521, port
/// 1504 (the previous tick's aim). `MGC_NO_MC1_UPGRADE_SIG_GATE=1`
/// restores the pool scan and the Fresh drop.
pub(crate) fn upgrade_sig_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_UPGRADE_SIG_GATE").is_none())
}

/// A retail rival corpse KEEPS its Rebound/deflection bit (`+17`
/// bit 7 = our `0x8000`) for the whole of its death; only the token's
/// own tick clears it, and only when its burst lapses. See the site in
/// [`World::rival_entity_tick`]'s death arm.
/// `MGC_NO_MC1_DEATH_KEEPS_REBOUND=1` restores the pre-dig clear.
pub(crate) fn death_keeps_rebound() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_DEATH_KEEPS_REBOUND").is_none())
}

/// `sub_57610_57B40` bills the Create-Castle ladder price BEFORE it
/// asks the pool for the castle ball (:65880-81) — the `if (v3)` null
/// guard covers only the ball and its in-transit `+48` pin. A rival
/// who fires into an exhausted pool therefore PAYS IN FULL and gets
/// nothing. See the site in [`World::rival_castle_token_tick`].
/// `MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER=1` restores the pre-dig
/// spawn-then-bill order.
pub(crate) fn castle_ball_debit_order() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER").is_none())
}

/// A/B toggle for **THE KNOCKBACK BEARING READS A FREED SOURCE**: set
/// `MGC_NO_MC1_KNOCK_FROM_DEAD_SOURCE` to restore the pre-dig guard,
/// which only armed the register when the letter's source was still a
/// LIVE pool record. Retail's gate at :55712 is the pointer compare
/// `v13x > v12x` — `src > 0` — alone. See the site in
/// [`World::rival_mail_intake`].
fn no_mc1_knock_from_dead_source() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_KNOCK_FROM_DEAD_SOURCE").is_some())
}

/// `sub_44D30`'s wizext CLEAR LIST is exact, and the port's copy was
/// wrong on three lanes at once (:54868-70 and :55043-46):
///
/// - **`+24` (`knock_dir`) IS cleared** — `v_24 = 0; v_26 = 0;
///   v_22 = 0` is one statement group; the port cleared only `+22`.
/// - **`+462` (`war`) is NOT cleared** — the rival-only ledger loop
///   writes `str_456[kx].var_u16_4` (+460, hate) and nothing else.
/// - **`+406` (`poverty`) is NOT cleared** — `sub_44D30` never
///   mentions it; it is the attack picker's own latch (:19468-91).
///
/// See the site in [`World::rival_respawn`].
/// `MGC_NO_MC1_RESPAWN_CLEAR_LIST=1` restores the pre-dig list
/// (knock_dir kept, war and poverty blanked).
pub(crate) fn respawn_clear_list() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RESPAWN_CLEAR_LIST").is_none())
}

/// The AI cascade's two CASTLE arms read RAW REGISTERS, never a pool
/// scan, a `known` flag or the ladder table: `sub_13F00`'s head is
/// `!wizext->var_50 && sub_14E60(a1, 0x10) && sub_15E90(a1, 0x10)`
/// (:18359) and `sub_14120`'s mana leg is `a1->+136 < token->+136`
/// (:18427; `sub_15E90` itself is :19376-79, `sub_14E60` :18769-77) — both pricing off the TOKEN's live `+136`, which D5's
/// ladder-event law made a per-LEVEL-EVENT value rather than a
/// derivable one. See the sites in [`World::rival_selector`].
/// `MGC_NO_MC1_CASTLE_ARM_REGISTERS=1` restores the pre-dig gates.
pub(crate) fn castle_arm_registers() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_CASTLE_ARM_REGISTERS").is_none())
}

/// ⭐⭐⭐ THE BALL-CLAIM ARM'S SPELL TERM IS THE POSSESS **TOKEN
/// REGISTER**, NOT A `known` FLAG — the same shape as the two castle
/// arms above, on the arm nobody had re-read. `sub_14230`
/// (:18439-52) opens on `sub_14E60(a1, 3u)`: `pool + 164 *
/// wizext->var_676[3]`, bounded only from BELOW. A rival who KNOWS
/// Possess but whose token was scattered, spent or never re-minted
/// has a 0 register, and retail refuses arm 7 outright — the cascade
/// falls through to arm 8, the mana hunt (`sub_14B10`).
///
/// Verbatim from the shipped binaries (`CARPET.EXE` file 0x2CA28,
/// `HIDDEN.EXE` file 0x2CC28 — the SAME VA 0x14230 in both listings,
/// `reference/remc1/sub_main.cpp` banner `(00014230)` and
/// `reference/remc1hw/sub_main.cpp` banner `(00014230)`), the head of
/// `sub_14230` is:
///
/// ```text
///   2cc28  53              push %ebx
///   2cc29  56              push %esi
///   2cc2a  8b 5c 24 0c     mov  0xc(%esp),%ebx
///   2cc2e  6a 03           push $0x3          <- SPELL 3
///   2cc30  53              push %ebx
///   2cc31  e8 22 0c 00 00  call 0x2d858       <- sub_14E60 (VA 0x14E60)
///   2cc39  85 c0           test %eax,%eax
///   2cc3b  0f 84 b9 00 00 00  je 0x2ccfa      <- register 0 => return 0
/// ```
///
/// and `sub_14E60`'s own bound (HIDDEN.EXE 0x2D858-0x2D89E) is
/// `movswl 0x2a4(%edx,%eax,2),%edx` (SIGNED) … `cmp %edx,%eax ; jbe
/// ; ret` / `xor %eax,%eax ; ret` — so the term is exactly
/// `(i16)owned[3] > 0`. `sub_14230` has EXACTLY ONE caller in both
/// shipped binaries (an `e8 rel32` scan finds one site, VA 0x137A9,
/// inside the cascade `sub_13170`), so this is the whole call path.
///
/// WITNESS — `mc1hwl8` t=24661, wizard 6 (ent 60): his wizext carries
/// `owned = [(0,33),(2,43),(4,51),(13,889),(15,912),(16,49),(17,52),
/// (20,50),(23,53)]` — **no entry for spell 3**. Retail's arm 7 dies
/// on the first term and the cascade lands on HuntMana (`+415` = 13)
/// with creature 192 in `+146`; the port read `known[3]` (true),
/// opened the ball claim and re-pointed him at mana BALL 611
/// (class 10, model 39). 697 of the take's 704 rows were that one
/// `slot 60 chase` lane, in two clusters that begin and end exactly
/// where retail's `+415` enters and leaves state 13.
///
/// See the site in [`World::rival_selector`].
/// `MGC_NO_MC1_POSSESS_ARM_REGISTER=1` restores the pre-dig
/// `known[3]` gate.
pub(crate) fn possess_arm_register() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_POSSESS_ARM_REGISTER").is_none())
}

/// ⭐⭐⭐ `sub_14E60` (:18769-77) — THE MANIFESTATION RESOLVER — IS A
/// BARE ARRAY INDEX AND NOTHING ELSE:
///
/// ```text
/// result = POOL + 164 * wizext[676 + 2*spell];
/// if (result <= POOL) return 0;      // index 0 (or negative) = null
/// return result;
/// ```
///
/// No class test, no model test, no state test, and — the one that
/// matters — **no owner test**. Retail's acquisition register `+676`
/// is authoritative on its own: whatever record its index names IS
/// the wizard's token for that spell, even when that record's `+42`
/// (our `f144`) names a DIFFERENT wizard because the slot was
/// re-bound underneath it.
///
/// The castle commit `sub_155F0` case 0x10 (:19191-93) resolves
/// through exactly that helper and gates on only three things —
/// `!v6`, `v6->+48` (busy) and `a1->+140 < v6->+136` (price). The
/// port's [`World::rival_token`] adds five gates of its own, and the
/// `f144 == ent` one is the one retail does not have.
///
/// mc1l49 t=33081 pins it: rival 1 (entity 594, wizard slot 1) sits
/// arrived over its own castle 619 in the Upgrade state with
/// `owned[16]` = slot 26 — a live `(12,16)` token, `tick70` 48,
/// `flags` 0x5, `f136` 40,000 — whose `+42` reads **646**, another
/// wizard's entity. Retail fires: `explain` t=33081→33082 shows
/// `wiz 1 (ent 594): cooldown[16] 0 -> 40`, and `sub_13800`'s
/// `if (sub_155F0(a1, 0x10)) return 0;` (:18121) then leaves the
/// handler BEFORE its z-hover, so retail's z stays at the housekeeping
/// clamp's `ground + v_12` = 5476 + 128 = 5604. The port's commit
/// refused on the owner test, fell through to the hover, and climbed
/// `−v_14` = +4 to 5608 — the take's single most repeated head shape.
///
/// `MGC_NO_MC1_CASTLE_TOKEN_INDEX=1` restores the pre-dig owner-checked
/// resolve at the castle commit.
pub(crate) fn castle_token_index() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_CASTLE_TOKEN_INDEX").is_none())
}

/// ⭐⭐ **THE WIZARD PICK'S INVISIBILITY SKIP IS A LIVE TOKEN READ AT
/// THE CALLER'S OWN DISPATCH — the exact sibling of the Rebound
/// notice this file already models** (`the-anti-rebound-notice-is-a-
/// live-token-read` fixture, mc1l49 t=3559).
///
/// `sub_145B0` :18558 skips a candidate on `!sub_16000(i, 0xCu)`, and
/// `sub_16000` (:19449-56) is
/// `v = sub_14E60(i, s); v && *(__int16 *)(v + 48) > 0` — a SIGNED
/// read of the TARGET's spell-12 manifestation record's `+48`, taken
/// at the moment the ATTACKER's selector runs. The port read the
/// attacker's cached [`RivalMc1::invisible`] planner flag instead,
/// which [`World::rival_refresh_buffs`] refreshes at the TARGET's own
/// dispatch — and refresh runs BEFORE that rival's cast arm, so a
/// wizard who cloaks on tick T stays visible to every reader until
/// T+1.
///
/// mc1l49 t=47357 is the witness, read from `state.struct_b64`: rival
/// 646 casts Invisibility that tick — its (12,12) token 113 goes
/// `+48` 0 → 251 and its purse 23657 → 22657 — and rival 672's
/// selector, running afterwards at slot 672, DROPS it: retail's
/// `+146` goes 646 → 569, abandoning the nearer 646 (d² 3,145,090)
/// for the human at d² 4,098,505. The one-tick-late port kept 646.
/// The same tick's `+48 = 251` is already visible because the token
/// walks below both carpets (113 < 646 < 672) — walk order is the
/// law, exactly as for spell 14.
///
/// `MGC_NO_MC1_INVIS_LIVE_TOKEN=1` restores the cached planner flag.
pub(crate) fn invis_live_token() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_INVIS_LIVE_TOKEN").is_none())
}

/// A/B toggle for the CLOAK NOTICE'S RAW `+48` READ (wave 121, dig
/// D13): set `MGC_NO_MC1_INVIS_WORD48` to restore the pre-dig
/// [`World::mc1_invis_notice`], which read [`Ent::f26`] — retail's
/// `+26` on anything that is not a live class-12 manifestation —
/// where `sub_16000` (`CARPET.EXE` file 0x2E7F8) reads `+48`.
pub(crate) fn no_mc1_invis_word48() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_INVIS_WORD48").is_some())
}

/// A/B toggle for THE RIVAL KILL'S TALLY HOME (round 154, w154f; round
/// 153 finding #5): set `MGC_NO_MC1_RIVAL_KILL_NO_TALLY` to restore the
/// pre-dig `rival_death`, which bumped the human's creature-kill
/// counter (`Type_160+359`) when the human killed a rival wizard "for
/// parity with the creature track". Retail's rival death (:55488-97)
/// credits the killer's PER-VICTIM tally `+30 + 2 * victim.+48`
/// (CARPET.EXE 0x46100-0x46111 `mov 0x7503(%edx,%eax,4),%edx` … `incw 0x1e(%edx,%eax,2)`, and no `0x167` store anywhere in `sub_45FC0`) and nothing else; `+359` has exactly
/// one writer, the creature death handoff `sub_1A6C0` (:21840-50),
/// which the class-3 wizard never reaches. Witness mc1l49 t=9641:
/// rival 620 dies to the human's fireball, retail `kills` 49 flat,
/// the port 50 — and one high for the rest of the take.
pub(crate) fn no_mc1_rival_kill_no_tally() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_KILL_NO_TALLY").is_some())
}

/// A/B toggle for THE RIVAL'S LEVEL-START MINT ORDER (round 154,
/// w154d; round 153 finding #3): set `MGC_NO_MC1_RIVAL_BOOK_ORDER` to
/// restore the pre-dig `spawn_rival` loop, which minted the book in
/// ascending spell id. Retail's `sub_3DD50` grant loop (:49213-54,
/// CARPET.EXE VA 0x3DEF8 `mov 0x9b88(%esi),%al` … 0x3E03A `cmp
/// $0x18,%esi`) runs for EVERY wizard, human and AI, and fills the
/// `+532` acquisition list in `byte_99B88` order (`DISPLAY_ORDER`,
/// file 0x9F380: `0,3,2,16,1,14,4,12,6,9,7,8,15,18,17,19,13,5,11,10,
/// 20..23`); `sub_44D30`'s mint then walks the LIST (:54882-905), so
/// the token slots — and the free stack under everything popped
/// after them — follow book order. `init-check` read the id-order
/// mint as wiz `owned` 442 rows on 21 takes, the `slot disagreements`
/// and the free-stack DIFF on 12 (mc1l49: 17 rows per rival × 7).
/// The human's half landed in 154-1 (`grant_level_book`); the
/// respawn re-grant is list-driven already (:54884-923).
pub(crate) fn no_mc1_rival_book_order() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_BOOK_ORDER").is_some())
}

/// A/B toggle for THE RIVAL EMIT'S INVENTED `+36` WRITE (round 154,
/// w154b): set `MGC_NO_MC1_RIVAL_EMIT_PITCH_MIRROR` to restore the
/// pre-dig [`World::rival_emit`], which stamped the bolt's `+36`
/// (target pitch) with the caster's pitch. Retail's sixteen token
/// machines (:65029-66420, one routine for human and AI) write the
/// bolt's `+30/+32` and never `+36` (fireball :65070-71, CARPET.EXE
/// VA 0x56188-0x56197 `mov 0x1e(%ebp),%ax; mov %ax,0x1e(%ebx); mov
/// 0x20(%ebp),%ax; mov %ax,0x20(%ebx)` — no `0x24(%ebx)` store in
/// the function). The human arm never had the write. Shadow lane
/// `(9,x) f36`: the possess lob holds retail's 0 for its whole
/// flight (mc1l2 t=2380-91 slot 108, port 242); the fireball's
/// tracker mirrors `+32 → +36` on its first flight tick, so its rows
/// are birth-boundary only.
pub(crate) fn no_mc1_rival_emit_pitch_mirror() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_EMIT_PITCH_MIRROR").is_some())
}

/// A/B toggle for THE NATIVE HUMAN SEAT (round 154, the player's
/// ruling on round 153's finding #1): set `MGC_NO_MC1_NATIVE_HUMAN_RECORD`
/// to restore the pre-ruling layout — no pooled carpet, the rivals'
/// records first, the human's book minted after them by the app. The
/// MC1 twin of `mc2::rivals::no_mc2_native_human_record`. See
/// [`World::mc1_spawn_human_record`].
/// A/B toggle for THE MARKER-LESS ORIGIN SEAT (round 154, the player's
/// ruling): set `MGC_NO_MC1_MARKERLESS_ORIGIN_SEAT` to restore the
/// pre-ruling fallback, which seated a rival with no `(3,4+colour)`
/// THING row at the HUMAN's start marker. Retail seats it at the
/// zeroed `str_9177[colour]` = (0, 0) — see [`World::spawn_rival`].
pub(crate) fn no_mc1_markerless_origin_seat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_MARKERLESS_ORIGIN_SEAT").is_some())
}

pub(crate) fn no_mc1_native_human_record() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_NATIVE_HUMAN_RECORD").is_some())
}

/// A/B toggle for **THE MARKER-LESS HUMAN SEAT** (round 161, w161d):
/// set `MGC_NO_MC1_MARKERLESS_HUMAN_SEAT` to restore the pre-dig
/// [`World::mc1_spawn_human_record`], which returned without minting
/// the carpet record at all when the level authored no `(3,4)` row.
///
/// This is the HUMAN half of round 154's
/// [`no_mc1_markerless_origin_seat`] — ⭐⭐⭐ A LAW ON ONE CALL PATH IS
/// NOT LANDED: retail has ONE routine, `sub_44D30` (:54802), for
/// every wizard, and the port splits it into
/// `mc1_spawn_human_record` / `spawn_rival`; round 154 gave the
/// origin seat to the rival arm and left the human arm's
/// `else { return }` standing.
///
/// Retail (remc1 `sub_main.cpp`):
/// ```text
/// 51488  memset((void*)(&str_AE400_AE3F0->str_9177), 0, 48);   // sub_40550, level init
/// 54845  v32x = str_AE400_AE3F0->str_9177[v3].v_9177;          // UNCONDITIONAL
/// 54846-49  tempZ = sub_11F50(&v32x); tempZ.y++; v32x.z = tempZ.word;
/// 54850  if (event == str_AE400_AE3F0->str_29795)              // wizard 0 = the human
/// 54852      v2x = sub_373F0_377B0(&v32x, 3, …);               // POP the carpet record
/// ```
/// Shipped `HIDDEN.EXE` (file = VA + **0x18D38**; `CARPET.EXE` = VA +
/// 0x187F8 and is byte-for-byte the same shape at file 0x5D528):
/// ```text
///   5daa7  8d b4 41 d9 23 00 00  lea 0x23d9(%ecx,%eax,2),%esi  ; &str_9177[wizard], no test
///   5dab3  a5 / 66 a5            movsl ; movsw                 ; copy the 6-byte seat
///   5dab6  e8 8d ce fc ff        call 0x2a948                  ; sub_11F50 (ground z)
///   5dabe  fe c4                 inc %ah                       ; + 0x100
///   5dacb  8d 81 63 74 00 00     lea 0x7463(%ecx),%eax         ; str_29795 (pool base)
///   5dad1  39 c3 / 75 2b         cmp %eax,%ebx ; jne           ; human vs AI arm
///   5daec  e8 b7 26 ff ff        call 0x501a8                  ; sub_373F0 — the carpet pop
///   59320  6a 30 … 05 d9 23 00 00 … call 0x75b38               ; sub_40550: memset(str_9177,0,0x30)
/// ```
/// Witness `recordings/mc1hwl5.mgcr`: mc1hw level 5 authors `(3,5)`
/// through `(3,9)` and **no `(3,4)`**, and retail's record 0 holds the
/// human at slot 341, x 0 / y 0 / z 256 for its first 30+ ticks.
pub(crate) fn no_mc1_markerless_human_seat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_MARKERLESS_HUMAN_SEAT").is_some())
}

/// A/B toggle for **THE POST-SPAWN TRUCE WALKS THE TICK-TOP CLASS-3
/// ROSTER** (round 154, w154c — the MC1 twin of round 147's
/// `MGC_NO_MC2_RIVAL_TRUCE_ROSTER` + round 141's
/// `MGC_NO_MC2_HUMAN_RESPAWN_TRUCE`): set `MGC_NO_MC1_TRUCE_ROSTER`
/// to restore the pre-dig port, where all THREE call paths of
/// `sub_44D30`'s truce loop ran a flat loop over `self.rivals`
/// (the rival spawn tail stamped every already-seated rival, the
/// rival respawn every OTHER rival, the human respawn every
/// non-eliminated rival).
///
/// `sub_44D30` (:54802, the wizard (re)init — ONE routine for the
/// level-start seat :48633, the Space respawn :48633 case 0xF and
/// the AI respawn :55616) closes with (:55037-41):
/// ```text
/// for (jx = var_u32_36462[0]; jx > pool; jx = jx->next)
///   if (jx->id24 != v2x->id24 && jx->model65 <= 1)
///     jx->wizext->str_456[v2x->wizext->var_48].hate = -24609;
/// ```
/// `var_u32_36462[0]` is BUCKET 0 of the tick-top sweep
/// (:52253-62): class 3, `actLife >= 0`, `flags & 0x10 == 0`,
/// sampled ONCE per tick — the port's [`Gen::wiz_chain`]. Shipped
/// `CARPET.EXE` (file = VA + 0x187F8):
/// ```text
///   5dab6  8b 80 6e 8e 00 00        mov  0x8e6e(%eax),%eax   ; AE408+36462 = bucket[0] head
///   5dabe  66 8b 50 18 / 66 3b 53 18  mov 0x18(%eax),%dx ; cmp 0x18(%ebx),%dx  ; id24 != mine
///   5dac8  8a 48 41 / 84 c9 / 74 05 / 80 f9 01 / 75 21   ; model65 == 0 || == 1
///   5daeb  66 c7 84 11 cc 01 00 00 df 9f  movw $0x9fdf,0x1cc(%ecx,%edx,1)  ; +460 + 8·colour
///   5daf5  8b 00                    mov  (%eax),%eax        ; jx = jx->next (+0)
///   5daf7..5db05                    cmp against pool base; ja loop
/// ```
/// Two consequences, both measured on mc1l14:
/// 1. **LEVEL-START SEATING.** Every wizard is seated inside the
///    first tick's command processor (:48633), whose bucket-0 chain
///    was built before any wizard record existed — the walk visits
///    nobody. The flat loop stamped every already-seated rival: the
///    round-153 `init-check` census's 105 `hate` rows / 22 takes
///    (port 38652..40927 vs retail NEUTRAL 24607).
/// 2. **A RESPAWN WHILE OTHER WIZARDS ARE DEAD.** mc1l14 t=1344:
///    rival 2 (ent 524) respawns; the human (ent 504, alive) takes
///    `hate[2]` 513 → 40927 in retail, but rivals 1 (ent 519, life
///    −2420, state 3) and 3 (ent 528, life −560, state 3) are in
///    their dead-wait, off bucket 0, and take NOTHING — the port
///    stamped both (754 free-run rows on this take alone; the
///    round-153 census's 35,724 rows / 11 takes).
///
/// The human respawn path (`World::player_respawn`'s amnesty) had the
/// same flat loop with an `!eliminated` guard — a dead rival is not
/// eliminated and still took the truce there.
pub(crate) fn no_mc1_truce_roster() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_TRUCE_ROSTER").is_some())
}

/// A/B toggle for **THE DEAD-WAIT DISPATCH CLEARS THE KNOCK
/// MAGNITUDE EVERY TICK** (round 154, w154c — the MC1 twin of round
/// 147's `MGC_NO_MC2_DEAD_WAIT_KNOCK_CLEAR`): set
/// `MGC_NO_MC1_DEAD_WAIT_KNOCK_CLEAR` to restore the pre-dig port,
/// where a landed wizard (rival OR human) kept whatever `+22` the
/// death fall had not yet bled off for the whole wait.
///
/// `sub_46480` (:55594, the class-3 STATE-3 handler for every
/// wizard) opens with `*(wizext + 22) = 0` (:55601) ABOVE its
/// AI/human fork (`13332 == 1`, :55605) — shipped `CARPET.EXE` file
/// 0x5EC7F `8b 83 a0 00 00 00` = `mov 0xa0(%ebx),%eax` / 0x5EC85
/// `66 c7 40 16 00 00` = `movw $0x0,0x16(%eax)`, the FIRST two
/// instructions after the prologue, with `cmpb $0x1,0x3414(%esi)`
/// only at 0x5ECA4. `+24` (the bearing) is NOT in that handler —
/// only the respawn's `sub_44D30` clears it (:54871).
///
/// The fall (`sub_45FC0` :55434) runs the mover, which bleeds the
/// knock 4/tick (:55204-18), so a short fall lands with 16..76
/// still standing. WITNESS mc1l14: rival 2 (ent 524) dies t=926 with
/// `+22` 80, lands t=927 (80 → 76, one fall step), and t=928 — the
/// first state-3 tick — retail reads 0 where the port held 76 until
/// the t=1344 respawn (416 rows). The HUMAN: dies t≈1394, the fall
/// bleeds 60 → 56 → 52, lands t=1396, t=1397 retail 0, port 52 until
/// the t=1414 respawn — the SAME statement, the human arm (the
/// round-153 census's free-run human residue, 4,259 rows / 19
/// takes; the pair-mode +4 is the round-147 harness artifact and is
/// NOT this).
pub(crate) fn no_mc1_dead_wait_knock_clear() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_DEAD_WAIT_KNOCK_CLEAR").is_some())
}

/// A/B toggle for **THE KNOCK BEARING IS STORED RAW** (round 154,
/// w154c — the MC1 twin of round 147's `MGC_NO_MC2_KNOCK_DIR_RAW`,
/// a rider on the dead-wait law): set `MGC_NO_MC1_KNOCK_DIR_RAW` to
/// restore the `& 0x7FF` the port applied at both homes of
/// `sub_46540`'s hit arm (:55714 — the rival intake in
/// [`World::rival_mail_intake`] and the human's in
/// `World::player_mail_block` / the pinned-pair twin). Retail stores
/// `sub_42150`'s return verbatim, and that return is 2048 when the
/// source sits a hair to the −x side and far to the −y side
/// (`angle_of`'s `2048 − lut(−dx, −dy)` quadrant with a zero LUT
/// entry); the consumer masks (`polar_step`, `sub_41EC0`), so the
/// store is the only observable. WITNESS mc1l10 t=4503-4505, rival
/// 2: retail 2048, port 0 (the "MC1's own arm is unaudited" note at
/// the human's homes was the lead).
pub(crate) fn no_mc1_knock_dir_raw() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_KNOCK_DIR_RAW").is_some())
}

/// A/B toggle for **THE KRAKEN BUFFET DRAGS ANY WIZARD** (round 154,
/// w154j — the lead w154c left): set `MGC_NO_MC1_KRAKEN_BUFFET_RIVAL`
/// to restore the port's human-only arm. `sub_1C4F0`'s ON tick
/// (:23223-28, the counter `+26` in 1..=41 of its 132-tick cycle)
/// writes the TARGET wizard's record through `+146`'s `+160` pointer
/// — `+24 = sub_42150(kraken+72, target+72)` with `(hi + 4) & 7`
/// (bearing + 0x400, 11-bit), `+26 = 256`, `+22 = 80` — for whichever
/// record `+146` names: CARPET.EXE 0x1C6E6 `call sub_42150`, 0x1C6EB
/// `add $0x4,%ah`, 0x1C6EE `mov 0xa0(%edi),%edx`, 0x1C6F4 `and
/// $0x7,%ah`, 0x1C6F7 `mov %ax,0x18(%edx)`, 0x1C701 `movw
/// $0x100,0x1a(%eax)`, 0x1C719 `movw $0x50,0x16(%eax)`; no class or
/// owner test anywhere between the counter compare (0x1C6D1) and the
/// stores. The port's arm was `tgt == PLAYER_TARGET` only, so a rival
/// riding a kraken tether kept the knock its last damage letter armed
/// (mc1l14 t=2262..2276 wiz 1: retail `knock_mag` 80 flat, `knock_dir`
/// 447→458 re-bearing every tick; port 40 / 609). The rival's live
/// mover never spends the knock (`sub_14EB0`), so the observable is
/// the wizext pair itself plus the corpse drift of a rival killed
/// while tethered. The `+26 = 256` store has no port register and no
/// lifted lane (`mgcr::Wizard` lifts +22/+24 only); it stays
/// unmodelled, as before.
pub(crate) fn no_mc1_kraken_buffet_rival() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_KRAKEN_BUFFET_RIVAL").is_some())
}

/// ⭐⭐⭐ **THE RIVAL'S CHARGE MOVE IS THE WHOLE EMIT FAMILY'S, NOT
/// FOUR SPELLS' — the same law the HUMAN's arm already carries
/// (`crate::engine::world`'s `cast_projectile` :10385), never landed
/// on the AI's call path.** The class-12 token machines in
/// `str_2563D8` (`reference/remc1/sub_main.cpp:4957-5033`, state =
/// 3*spell) are ONE set of functions for the human and the AI, and
/// fourteen of them close their emit with the two-line pair
/// `bolt.+26 = wizext.u8_326; wizext.u8_326 = 0`: fireball :65072,
/// earthquake :65356, meteor :65414, volcano :65472, crater :65536,
/// steal mana :65756, **lightning :65846**, castle :65910, undead
/// army :65973, storm :66031, mana magnet :66092, wall of fire
/// :66153, global death :66278, rapid fireball :66339 — and possess
/// (:65246) zeroes WITHOUT stamping. Only the duel (`sub_57040`
/// :65620-710) leaves the meter standing.
///
/// The port's rival arm modelled four of them (0/7/8 stamp, 3 zero,
/// 20 stamp) and its comment asserted "the other rival arms never
/// touch +326" — refuted by the table above and by the corpus.
/// mc1l49's whole-take pair census makes it the take's LARGEST
/// family: **766 `(9,9)` `f26` rows + 766 complementary
/// `rival.charge` rows on the same 766 ticks** — 62% of all 2,468
/// dirty pair rows, which the law takes to 936 with every other
/// lane's row count UNCHANGED. First witness t=2926→2927 slot 920, a `(9,9)`
/// born with `+70`=9, `+69`=23, sprite 216 — the zigzag lightning
/// `sub_39EC0` (:46135) minted by spell 15's machine
/// `sub_57470_579A0` (:65806-61), whose spawner call is literally
/// `sub_373F0_377B0(caster.+72, 9, 9)` (:65847). Retail reads
/// `+26` = 1 with the rival's meter back at 0; the port left the
/// bolt's `+26` at 0 and let the meter run on to 2.
///
/// `MGC_NO_MC1_RIVAL_CHARGE_FAMILY=1` restores the four-spell arm.
pub(crate) fn rival_charge_family() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_CHARGE_FAMILY").is_none())
}

/// ⭐⭐ THE RIVAL'S "AT OWN CASTLE" IS THE `wizext+50` REGISTER, NOT
/// THE FIRST-COMMIT LATCH. `sub_132B0` (:17971-72; shipped `CARPET.EXE`
/// file 0x2BBF3 `mov 0xa0(%ebx),%eax; mov 0x32(%eax),%si; test %si,%si;
/// je` → `call 0x2a148` = `sub_11950` overlap) dereferences the
/// register with an index test alone — no class/model/owner/latch/
/// level test — and the regen fork at :18002-17 (file 0x2BD21-0x2BDB6:
/// `testb $0x10,0x11(%ebx)`; `/0xc8` floored to `0x3e8`, else `/0x7d0`
/// floored to `0x64`) spends the at-castle +1000/tick off that bool.
/// The port resolved the castle by pool SCAN and then demanded the
/// castle's `+16` bit 1 latch (`flags & 2`, the :56057 first-commit
/// team-recolor stamp), which an AUTHORED castle never earns: mc1l20's
/// slot 534 (level 3, wiz 2's keep) reads `flags 12` for the whole
/// take while retail's `wizext+50[2] = 534` from record 0. The port
/// therefore paid the rival the AFIELD +100/tick where retail paid
/// +1000/tick (pairs 1→2 … 34→35: `mana 11000 vs 10100`, `13000 vs
/// 12100`, …). The same latch-for-register stand-in gated the cast-16
/// bound arm (:19309 `if (+50)`) and the respawn re-price (:55034
/// `if (var_50) sub_47DD0`). `Gen::castle_reg` IS the register (bound
/// at the authored mint :54980, the plant :19206, the commit :56484;
/// seeded from the recorded wizext on import).
///
/// `MGC_NO_MC1_RIVAL_CASTLE_REGISTER=1` restores the latch-gated scan.
pub(crate) fn rival_castle_register() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_CASTLE_REGISTER").is_none())
}

/// `MGC_NO_MC1_RIVAL_CASTLE_TOKEN_REGISTER=1` restores the pre-dig
/// create-vs-upgrade split of the RIVAL's castle token, which resolved
/// the "established castle" by pool scan filtered on `f26 > 0` where
/// retail's `sub_57610_57B40` (`reference/remc1/sub_main.cpp` :65893-908)
/// reads the owner's `wizext+50` REGISTER with an index test alone —
/// CARPET.EXE 0x6FEE0 `mov 0xa0(%ebp),%eax; movzwl 0x32(%eax),%edx;
/// lea pool+164*edx,%esi; cmp %eax,%esi; jbe <create>` (0x6FF16), else
/// 0x6FF28 `movb $0xa,0x44(%ebx); movb $0x2b,0x45(%ebx); mov
/// %ax,0x92(%ebx)` (+68 = 10, +69 = 43, +146 = the register). No
/// class, life or LEVEL test: a freshly PLANTED level-0 castle (the
/// :19206 direct plant binds the register at spawn) is already "bound",
/// so the very next cast is an UPGRADE ball homing on it. The human's
/// split already reads the register (`World::player_castle_bound`); the
/// rival's token was the one call path left on the stand-in.
///
/// WITNESS mc1l26-froze t=25107-25108: wiz 3 (ent 603) plants castle
/// 985 at t=25107 (`wizext+50` 0 → 985 in the same record, `+26` 0),
/// recasts at t=25108 — retail's ball 953 is born `+68 10 +69 43 +146
/// 985` and flies (life 21 → 20); the port minted a `+68 3 +69 2`
/// create ball at the wizard's own site, `castle_site_ok` refused it
/// (castle 985 within 2048) and reap-flagged it on its birth tick
/// (`flags 4 vs 1030`, `chase 985 vs 0`, `target_yaw 1115 vs 0`).
pub(crate) fn rival_castle_token_register() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RIVAL_CASTLE_TOKEN_REGISTER").is_none())
}

/// Per-slot config from the level record (wizards.json), resolved by
/// the app: personality params, starting castle, and the two spell
/// masks (str_230867_37072[slot], :49222/:54965-67).
#[derive(Debug, Clone)]
pub struct RivalConfig {
    /// u16_522: hate rise rate, war thresholds, opportunism margins.
    pub aggression: u8,
    /// u16_524: commit aim cone, rebound-notice probability.
    pub accuracy: u8,
    /// u16_526: decision period, turn agility, burst pause, respawn.
    pub tempo: u8,
    /// Starting castle level: 0 = none, N = a castle at level N-1
    /// spawns with the wizard (level tail @38804+slot).
    pub castle_level: u8,
    /// Level-start book: pregrant && allowed (:49222).
    pub book: [bool; SPELL_COUNT],
    /// var_230983 — what the AI may LEARN mid-level (Type_160+796).
    pub allowed: [bool; SPELL_COUNT],
}

/// The rival wizard names, by player slot (off_99B68 :5741; slot 0 =
/// the human's default name).
pub const RIVAL_NAMES: [&str; 8] = [
    "Zanzamar",
    "Vodor",
    "Gryshnak",
    "Mahmoud",
    "Syed",
    "Raschid",
    "Alhabbal",
    "Scheherazade",
];

/// The hate ledger's neutral baseline (0x601F, :17946-67).
const HATE_NEUTRAL: u16 = 24607;
/// Hate toward a freshly (re)spawned wizard — elevated but decaying
/// (the post-respawn truce, -24609 as unsigned :55037-41).
const HATE_RESPAWN: u16 = 40927;

/// AI per-spell re-attempt cooldowns, ticks (word_90034 :2163).
const AI_RECAST: [u16; SPELL_COUNT] = [
    2, 1, 32, 10, 1, 0, 0, 4, 400, 0, 1, 0, 1, 0, 1, 1, 40, 600, 0, 1, 4, 2, 3, 4,
];

/// The AI brain state (Type_160+415). States 2/4/5/10 exist in the
/// original's table but no selector ever sets them (cut content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub(crate) enum AiState {
    /// Fresh spawn: decide immediately (cascade runs twice, :17850).
    #[default]
    Fresh,
    /// Fly home, cast 0x10 = upgrade (sub_13800 :18106).
    Upgrade,
    /// Fly to the scouted site, plant the castle (sub_138F0 :18142).
    Build,
    /// Claim a mana ball with spell 3 (sub_13BA0 :18236).
    Possess,
    /// Raid an enemy castle (sub_13CA0 :18271).
    RaidCastle,
    /// Attack an enemy wizard (sub_13DC0 :18314).
    AttackWizard,
    /// Intercept an enemy balloon (same handler, state 9).
    RaidBalloon,
    /// Hunt any mana-holding creature (state 0xD).
    HuntMana,
    /// Return home (heal / regroup; sub_13A70 :18204).
    Home,
    /// Cruise (full life, nothing to do; sub_13A10 :18188).
    Cruise,
}

impl AiState {
    /// Retail +415 byte → variant (dispatch sub_13170 :17847). The cut
    /// states 2/4/5/10 fall to Fresh — no selector ever sets them.
    pub(crate) fn from_retail(v: u8) -> Self {
        match v {
            1 => AiState::Upgrade,
            3 => AiState::Build,
            6 => AiState::Possess,
            7 => AiState::RaidCastle,
            8 => AiState::AttackWizard,
            9 => AiState::RaidBalloon,
            0xB => AiState::Home,
            0xC => AiState::Cruise,
            0xD => AiState::HuntMana,
            _ => AiState::Fresh,
        }
    }

    /// Variant → canonical +415 byte, the inverse of
    /// [`Self::from_retail`] up to the cut states (2/4/5/10 all read
    /// back as Fresh's 0) — compare retail bytes through
    /// `from_retail(a).to_retail()` so the collapse is symmetric.
    pub(crate) fn to_retail(self) -> u8 {
        match self {
            AiState::Fresh => 0,
            AiState::Upgrade => 1,
            AiState::Build => 3,
            AiState::Possess => 6,
            AiState::RaidCastle => 7,
            AiState::AttackWizard => 8,
            AiState::RaidBalloon => 9,
            AiState::Home => 0xB,
            AiState::Cruise => 0xC,
            AiState::HuntMana => 0xD,
        }
    }
}

/// One live rival: the Type_160 subset the AI machinery needs. The
/// wizard's position/yaw/life/speed live on its pool entity (class 3
/// model 1); carried mana rides the entity's f140 mirror for the
/// census.
#[derive(Hash, Clone)]
pub(crate) struct Rival {
    /// Player slot (1..=7); slot 0 = the human, never a Rival.
    pub slot: u8,
    /// Wizard entity pool index. Also the rival's OWNER TAG: its
    /// projectiles' id24 and its claims' f144 (the original's +24 =
    /// own entity index, :44219).
    pub ent: u16,
    /// Manifestation pool slots by spell id (var_676; 0 = not owned).
    pub owned: [u16; SPELL_COUNT],
    /// The +532 ACQUISITION LIST — manifestation pool slots in PICKUP
    /// order while alive. The death scatter iterates THIS, not the
    /// spell-id book (mc1l4 t=6885: two jars' scatter draws land in
    /// list order), rewriting each live entry to the token's MODEL
    /// number and each empty one to −1 (:55519-49); the respawn
    /// re-grant re-mints from the rewritten entries IN PLACE
    /// (:54884-923 — a scattered fireball's model 0 collides with the
    /// empty sentinel by design: the −1→0 reset skips, the 0 entry
    /// re-mints model 0, and that collision is exactly how fireball
    /// ownership survives death). Grants append at the first ZERO
    /// entry (:19421-31).
    pub(crate) acq: [i32; SPELL_COUNT],
    /// Spells known across deaths — respawn re-mints manifestations
    /// (:54884-923); the scattered jars decay independently.
    pub known: [bool; SPELL_COUNT],
    /// Learn eligibility (Type_160+796).
    pub(crate) allowed: [bool; SPELL_COUNT],
    /// Spell-learning countdowns (+628): armed to 200 by a matching
    /// jar existing anywhere; conjures an own copy at expiry.
    pub(crate) learn: [u16; SPELL_COUNT],
    /// AI re-attempt cooldowns (+724, from [`AI_RECAST`]). Slot 16 is
    /// initialized to 4*slot — the per-player castle-build stagger
    /// the decompile shows as "var_756" (:55049).
    pub(crate) cooldown: [u16; SPELL_COUNT],
    /// Carried mana (+140) / ceiling (+136, census-owned) / regen
    /// delta (+132 — cast debits ride it negative).
    pub mana: u32,
    pub mana_max: u32,
    pub(crate) mana_delta: i32,
    /// Personality (u16_522/524/526).
    agg: u16,
    acc: u16,
    /// Retail wizext +526 — the live cadence scalar (think period,
    /// turn-servo divisor, burst lockout). Seeded from the level
    /// config natively; the conformance import overwrites it with the
    /// recorded value (retail re-stamps it at init and every respawn).
    pub(crate) tempo: u16,
    pub state: AiState,
    /// Hate ledger + war flags per player slot (str_456; baseline
    /// [`HATE_NEUTRAL`]).
    pub(crate) hate: [u16; 8],
    pub(crate) war: [bool; 8],
    /// Fireball/lightning burst counter (+404): 8 shots then a
    /// negative lockout of (255-tempo)/8+1 ticks (:19129-36).
    burst: i16,
    /// Poverty latch (+406): mana < max/4 stops attack casting until
    /// it recovers past max/4+6000 (or max/2) (:19468-91).
    poverty: bool,
    /// Current target: entity slot or [`PLAYER_TARGET`]; 0 = none.
    /// (pub(crate) for the possess-emission tests in engine::world.)
    pub(crate) target: u16,
    /// Signature = team + model + (class<<7) (sub_15420 :19039).
    target_sig: u16,
    /// Scouted castle site (+150).
    site: (u16, u16),
    /// Lateral dodge velocity (v_16; impulse 80, decay 4/tick).
    pub(crate) jink: i16,
    /// Knockback bearing (v_24) and magnitude (v_22), armed by the
    /// shared damage intake (:55714-19) on EVERY letter including the
    /// fatal one. The AI's live mover `sub_14EB0` never spends it, so
    /// the impulse sits pending for the whole of a rival's life; the
    /// state-2 death fall's shared mover `sub_455D0` (:55204-19) is
    /// the only place a rival ever cashes it, drifting the corpse
    /// along the killing blow's bearing at 4/tick decay.
    pub(crate) knock_dir: u16,
    pub(crate) knock_mag: i16,
    /// Desired speed (v_12) toward which f126 accelerates 16/tick.
    pub(crate) vdes: i16,
    /// ⭐ THE SPEED-COLUMN LATCH (v_14) — "the BRAIN wrote v_12 this
    /// tick". `sub_15470` CLEARS it at its head (:19057) and every
    /// leg that actually writes v_12 sets it: the arrival stop
    /// (:19075-76) and the plain throttle (:19089-91), plus the
    /// cruise/home twins `sub_13A10` (:18197-98) and `sub_13A70`
    /// (:18220-21) — which do NOT clear it, so a state that never
    /// consults `sub_15470` leaves the latch standing.
    ///
    /// Its ONLY consumer is the speed token (`sub_56380` :65147-50 /
    /// `sub_57F00` :66186-89): a set latch means the AI has retaken
    /// the speed columns, so the burst KILLS ITSELF — `+48 = 1`, and
    /// the shared decrement below zeroes it that same tick. That is
    /// the two-phase kill: mc1l5's Vodor arrives at t=185 with the
    /// burst still 63 ticks from expiry and the token drops to 0 in
    /// one step, snapping his speed 160 → 80.
    ///
    /// Not in the recording (the closure carries v_12 but not v_14),
    /// so it rides the port's own snapshot like `vdes`.
    pub(crate) v14: bool,
    /// Spawn grace (u16_331): mailbox discarded while > 0.
    pub(crate) grace: u16,
    /// Post-hit regen stall (u32_383). Armed by the shared intake
    /// like retail's, but the AI regen NEVER reads it — only the
    /// HUMAN's regen tail does (:55387-90); the AI housekeeping
    /// (:17990-18021) heals straight through fresh hits.
    regen_stall: u16,
    /// Life-regen rate REGISTER (u16_341): the housekeeping APPLIES
    /// this, then re-selects it from the at-castle/shrine test
    /// (:17994-18018) — the rate applied at tick N was chosen at
    /// N−1, the AI twin of the human's :55388 staircase.
    life_rate: i32,
    /// Dead and castle-less: permanently out (byte_13329_6 = 0,
    /// :55622). Property is NOT torn down.
    pub eliminated: bool,
    /// Buff flags derived from the manifestations' bursts.
    pub shield: bool,
    pub invisible: bool,
    pub rebound: bool,
    /// ⭐⭐⭐ THE HUSK-WATCH GATE — this player's roster byte
    /// `str_13323[slot].var_u8_13332_9` read as "NOT 1", i.e. **this
    /// row is HUMAN-driven**. `sub_46480_467C0` (:55605, CARPET.EXE
    /// `cmp byte [esi+0x3414],1` at file 0x5eca4) dispatches the whole
    /// state-3 wizard tick on it: `== 1` takes the AI arm (respawn
    /// countdown / castle-less `13329 = 0`), anything else takes the
    /// ELSE arm — `sub_44BE0_44F20(a1, 7)` (the VIEW-MODE byte, only
    /// when the row is the local player's, :54767-70) plus
    /// `sub_463B0_466F0` (:55575-91), the killer watch.
    ///
    /// Retail writes the byte in exactly ONE place — `sub_3DD50_3E090`
    /// (:49154, `mov byte [ebx+0x9],1` at file 0x56614), the per-level
    /// roster reset, `1` for every row except `var_u16_8`'s. It is a
    /// per-LEVEL CONSTANT (the binary contains no other store to it: a
    /// full opcode scan of CARPET.EXE finds the two `+0x3414` readers
    /// at file 0x3042f/0x5eca4 and no `mov byte [reg+9]` writer of 0).
    /// So this flag is FALSE for every rival — until the death-scatter
    /// overflow below sets it. See [`World::rival_death_impact`].
    pub(crate) human_driven: bool,
}

impl Rival {
    pub(crate) fn new(slot: u8, ent: u16, cfg: &RivalConfig) -> Self {
        let mut cooldown = [0u16; SPELL_COUNT];
        // The castle-build stagger (:55049): 4 ticks per player slot.
        cooldown[16] = 4 * slot as u16;
        Rival {
            slot,
            ent,
            owned: [0; SPELL_COUNT],
            acq: [0; SPELL_COUNT],
            known: [false; SPELL_COUNT],
            allowed: cfg.allowed,
            learn: [0; SPELL_COUNT],
            cooldown,
            mana: 1000,
            mana_max: 1000,
            mana_delta: 0,
            agg: cfg.aggression as u16,
            acc: cfg.accuracy as u16,
            tempo: cfg.tempo as u16,
            state: AiState::Fresh,
            hate: [HATE_NEUTRAL; 8],
            war: [false; 8],
            burst: 0,
            poverty: false,
            target: 0,
            target_sig: 0,
            site: (0, 0),
            jink: 0,
            knock_dir: 0,
            knock_mag: 0,
            vdes: 0,
            v14: false,
            grace: 100,
            regen_stall: 0,
            life_rate: 0,
            eliminated: false,
            shield: false,
            invisible: false,
            rebound: false,
            // `sub_3DD50` :49154 — every non-`var_u16_8` row is 1.
            human_driven: false,
        }
    }

    /// Decision-tick gate: every `64 - tempo/4` ticks keyed on the
    /// entity age byte (:18024/:18065).
    fn think_period(&self) -> u8 {
        (64 - (self.tempo / 4) as i32).max(1) as u8
    }

    /// The pickup append (:19421-31): the first ZERO entry of the
    /// acquisition list takes the freshly minted token's pool slot; a
    /// full list drops the append, exactly as retail's 24-bounded scan.
    fn acq_push(&mut self, m: u16) {
        if let Some(e) = self.acq.iter_mut().find(|e| **e == 0) {
            *e = m as i32;
        }
    }

    /// The rival's wizext/brain registers as RETAIL-convention lanes —
    /// the per-rival half of `World::wiz_shadow_mc1` (this module owns
    /// the private brain fields, so the projection lives here). Lane
    /// names match `RetailWizardMc1`'s fields; `ai_state` is the
    /// canonical [`AiState::to_retail`] byte, `poverty` is the latch as
    /// 0/1 (retail keeps a mana threshold in the live latch, the port a
    /// bool — nonzero-ness is the comparable fact), `war` likewise.
    /// `v_14` and `target` are deliberately absent: v_14 is not in the
    /// recording, and the target rides the carpet entity's graded f146.
    pub(crate) fn wiz_shadow_lanes(
        &self,
    ) -> (Vec<(&'static str, i64)>, Vec<(&'static str, Vec<i64>)>) {
        let scalars = vec![
            ("cmd_speed", self.vdes as i64),
            ("strafe", self.jink as i64),
            ("knock_mag", self.knock_mag as i64),
            ("knock_dir", self.knock_dir as i64),
            ("grace", self.grace as i64),
            ("regen_stall", self.regen_stall as i64),
            ("life_rate", self.life_rate as i64),
            ("ai_state", self.state.to_retail() as i64),
            ("burst", self.burst as i64),
            ("poverty", self.poverty as i64),
            ("target_sig", self.target_sig as i64),
            ("mana_delta", self.mana_delta as i64),
            // Round 153's widening (the MC1 half of round 147's): the
            // TEMPO scalar behind every AI cadence (+526) and the
            // roster's AI-DRIVEN byte (+9) as retail's `== 1` test —
            // `human_driven` is that test's negation, and the death
            // scatter's `var_916` overflow is the only thing that can
            // move it mid-level.
            ("tempo", self.tempo as i64),
            ("ai_flag", (!self.human_driven) as i64),
        ];
        let arrays = vec![
            ("hate", self.hate.iter().map(|&v| v as i64).collect()),
            ("war", self.war.iter().map(|&v| v as i64).collect()),
            ("learn", self.learn.iter().map(|&v| v as i64).collect()),
            (
                "cooldown",
                self.cooldown.iter().map(|&v| v as i64).collect(),
            ),
            ("owned", self.owned.iter().map(|&v| v as i64).collect()),
            ("acq", self.acq.iter().map(|&v| v as i64).collect()),
        ];
        (scalars, arrays)
    }
}

/// Read-only snapshot of one rival's AI internals (diagnostics only).
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct RivalAiDebug {
    pub slot: u8,
    pub state: String,
    pub target: u16,
    pub known: Vec<usize>,
    pub owned: Vec<usize>,
    pub allowed: Vec<usize>,
    pub has_offense: bool,
    pub mana: u32,
    pub mana_max: u32,
    pub poverty: bool,
    pub burst: i16,
    pub castle_stored: Option<u32>,
}

impl World {
    /// Wire the level's wizards from the baked config: one rival per
    /// active AI slot (1..player_count), spawned at its start marker
    /// (class-3 model 4+slot placement, str_9177 :44068-107) with its
    /// level-start book and starting castle (sub_44D30 :54802-55005).
    /// Slot 0 (the human) is ignored here — the human's book comes
    /// from the campaign/jar machinery.
    pub fn set_wizards(&mut self, configs: &[Option<RivalConfig>; 8], player_count: u16) {
        for slot in 1..player_count.min(8) as u8 {
            let Some(cfg) = &configs[slot as usize] else {
                continue;
            };
            self.spawn_rival(slot, cfg.clone());
        }
    }

    /// ⭐ THE HUMAN'S OWN POOL RECORD, seated where retail seats it —
    /// the MC1 twin of [`World::mc2_spawn_human_record`] (round 112),
    /// added in round 153 for `init-check`'s native build and wired
    /// into the constructor in round 154 (the player's ruling).
    ///
    /// Retail's level start is the FIRST tick's command processor:
    /// `sub_3DD50` (:49154) arms a join command per wizard and the
    /// consume loop (:48633) runs `sub_44D30` for wizard 0..7 in
    /// order. For a wizard with `playIndex == 0` that routine pops a
    /// class-3 record at the (3,4+slot) start marker
    /// (`sub_373F0(&pos, 3, ai_flag)`, :54843-46) BEFORE it mints the
    /// book's class-12 tokens (:54882-905) and the AI's starting
    /// castle — so the human's carpet is the first wizard pop, his
    /// tokens follow it, and every rival record lands below them.
    /// The native port kept the human out of the pool and the app
    /// granted his book AFTER `set_wizards`, so every wizard-minted
    /// record sat 1 + (book size) slots off and every slot-seeded law
    /// downstream (`rand = slot + global`, `f63 % n` cadences, which
    /// free slot a painter pops) ran on the wrong slot — `terrain-check`
    /// read 11 DIFFERENT takes, of which EIGHT (mc1l10/13/15/16/20/22/
    /// hwl2/l49, the "first-tick runtime family" since round 114) are
    /// this layout and nothing else (round 153 §Finding #1).
    ///
    /// THE REPRESENTATION IS THE IMPORT'S ([`World::retail_import_mc1`]):
    /// the slot stays CLASS 0, a pinned record whose `rand` lane is
    /// the live per-entity stream, the pose the runner's input
    /// anchored at the slot by the walk (`mc1_carpet_slot != 0` takes
    /// the certified in-walk arm, which native play now runs too).
    /// ⚠ SINCE ROUND 161 (w161d) EVERY MC1 WORLD SEATS A CARPET —
    /// a marker-less level is seated at the origin rather than
    /// skipped, so `mc1_carpet_slot == 0` no longer means "no marker",
    /// it means **no MC1 world at all** (MC2, or the A/B switch).
    /// The bare-world fall-throughs still keyed on that sentinel
    /// (`engine::world` :10288/:10462/:10562) are therefore MC2-only
    /// on this path now. The constructor seats it; the book goes
    /// through [`World::grant_level_book`] BEFORE `set_wizards`.
    /// Set `MGC_NO_MC1_NATIVE_HUMAN_RECORD=1` for A/B.
    pub fn mc1_spawn_human_record(&mut self) {
        if no_mc1_native_human_record()
            || matches!(self.game(), crate::ids::GameId::Mc2)
            || self.mc1_carpet_slot != 0
        {
            return;
        }
        // ⚖ A MARKER-LESS **HUMAN** SEATS AT THE ORIGIN TOO — the
        // other half of round 154's `no_mc1_markerless_origin_seat`,
        // which landed on [`World::spawn_rival`] ONLY. `sub_44D30`
        // (:54845) reads `str_9177[wizard]` UNCONDITIONALLY and pops
        // the class-3 carpet on the `event == str_29795` arm (:54848)
        // with no marker test at all, and `sub_40550` (:51488)
        // memsets the whole 48-byte array with the pool, so colour 0
        // with no `(3,4)` THING row is seated at (0, 0, ground+0x100)
        // exactly like a marker-less rival. See
        // [`no_mc1_markerless_human_seat`].
        let (x, y) = match self.start_markers[0] {
            // `sub_44D30`'s pose: the marker's tile centre, ground +
            // 0x100 (:54838-42, the same snap `spawn_rival` uses).
            Some((mx, my)) => ((mx << 8).wrapping_add(128), (my << 8).wrapping_add(128)),
            None if no_mc1_markerless_human_seat() => return,
            // The memset seat is ENGINE units (0, 0), NOT tile (0,0)'s
            // centre — retail's `str_9177[0]` is zeroed bytes, never a
            // tile index run through the half-tile snap.
            None => (0u16, 0u16),
        };
        let Some(i) = self.g.new_event() else { return };
        let rand = self.g.ent[i].rand;
        self.g.ent[i] = crate::engine::features::Ent::default();
        self.g.ent[i].rand = rand;
        let z = (self.g.ground_z(x, y) as i16).wrapping_add(0x100);
        self.human_pose = (x, y, z);
        self.human_pose_prev = self.human_pose;
        self.mc1_cast_pose = crate::engine::world::PlayerPose {
            x,
            y,
            z,
            ..self.mc1_cast_pose
        };
        self.mc1_carpet_slot = i as u16;
        self.g.mc1_pinned = crate::engine::features::Mc1Pinned(i as u16);
        if native_human_seat() {
            self.g.player_relink(x, y);
        }
    }

    fn spawn_rival(&mut self, slot: u8, cfg: RivalConfig) {
        // Start marker: class 3, model 4+slot (tile-center position,
        // :44003). ⚖ A MARKER-LESS WIZARD SEATS AT THE ORIGIN (round
        // 154, the player's ruling "as faithful as possible"):
        // `sub_40550` memsets `str_9177[]` with the pool at level init
        // (:51488) and `sub_44D30` reads `str_9177[wizard]` unconditionally
        // (:54845), so a colour no THING row names is seated at (0, 0,
        // ground + 0x100) — mc1l24 authors `player_count` 5 with no
        // `(3,8)` marker and retail's rival 4 sits at record 0 at
        // x 0 / y 65520 / z 258 (one settle tick of flight from the
        // origin). The port fell back to the HUMAN's marker.
        // `MGC_NO_MC1_MARKERLESS_ORIGIN_SEAT=1` restores the fallback.
        let (x, y) = match self.start_markers[slot as usize] {
            Some((mx, my)) => ((mx << 8).wrapping_add(128), (my << 8).wrapping_add(128)),
            None if no_mc1_markerless_origin_seat() => {
                let Some((mx, my)) = self.start_markers[0] else {
                    return;
                };
                ((mx << 8).wrapping_add(128), (my << 8).wrapping_add(128))
            }
            None => (0, 0),
        };
        let z = (self.g.ground_z(x, y) as i16).wrapping_add(256);
        let Some(i) = self.g.spawn_class3(1, x, y, z) else {
            return;
        };
        // Per-slot rival art (:54927-55): sprite-stats rows 273-279
        // (slot 0 keeps 44). Draw type 0x11: 16 views by mirror.
        self.g.set_sprite(i, 272 + slot as u16);
        self.g.refill_life(i);
        // Retail keeps a wizard's mana ON the entity (+140); the
        // port's authority is `Rival::mana`, with the entity as the
        // combat-visible mirror — the Rebound deflection's afford
        // gate and quarter debit (sub_52B30 :62858-90) read and
        // write THIS field, so it must track the pool.
        self.g.ent[i].f140 = 1000;
        let mut r = Rival::new(slot, i as u16, &cfg);
        // Level-start book (:49222): pregrant && allowed, as resolved
        // by the app into cfg.book — walked in BOOK order
        // (`byte_99B88`), the order `sub_3DD50` fills the acquisition
        // list in (:49213-54) and the mint walks (:54882-905). The
        // slots follow the walk; `known`/`owned` are sets and do not
        // care. See [`no_mc1_rival_book_order`].
        let order: Vec<usize> = if no_mc1_rival_book_order() {
            (0..SPELL_COUNT).collect()
        } else {
            crate::mc1::spells::DISPLAY_ORDER
                .iter()
                .map(|&s| s as usize)
                .collect()
        };
        for s in order {
            if cfg.book[s] {
                r.known[s] = true;
                if let Some(m) = self.mint_manifestation(s, i as u16) {
                    r.owned[s] = m as u16;
                    r.acq_push(m as u16);
                }
            }
        }
        // Starting castle (:54963-55005): AI-only, needs the Castle
        // spell known and a nonzero tail level. Spawned at the wizard
        // (even-parity tile snap in the spawn handler), pre-leveled,
        // FULL of mana, terrain stamped to match.
        if cfg.castle_level > 0 && r.known[16] {
            self.spawn_starting_castle(&mut r, cfg.castle_level);
        }
        // The post-spawn truce (:55037-41): every OTHER wizard ON THE
        // TICK-TOP BUCKET-0 ROSTER takes the elevated-but-decaying
        // 40927 toward this newcomer. At level-start seating that
        // roster is EMPTY — the chain was built before any wizard
        // record existed — so nothing is stamped.
        // `MGC_NO_MC1_TRUCE_ROSTER=1` restores the flat loop.
        if no_mc1_truce_roster() {
            for other in &mut self.rivals {
                other.hate[slot as usize] = HATE_RESPAWN;
            }
        } else {
            self.mc1_truce_roster(slot as usize, i as u16);
        }
        // The team resolver for owner recolors (balls/balloons/flags).
        self.g.rival_ents[slot as usize] = i as u16;
        self.rivals.push(r);
        self.entities_dirty = true;
    }

    /// `sub_44D30`'s truce loop (:55037-41, shipped `CARPET.EXE` file
    /// 0x5DAB6..0x5DB05): walk bucket 0 of the tick-top sweep
    /// ([`Gen::wiz_chain`] — class 3, `actLife >= 0`, `flags & 0x10
    /// == 0`, sampled ONCE per tick) and stamp `hate[colour] =
    /// (u16)-24609` = [`HATE_RESPAWN`] on every member whose model is
    /// 0 (the human) or 1 (a rival), skipping the record whose `+24`
    /// id matches the respawner's. A wizard in its dead-wait is off
    /// the roster and takes no truce; at level-start seating the
    /// roster is still empty. The human's own ledger (model 0) has
    /// no port home, so only the rival members are written; a
    /// castle (model 2) on the same chain is skipped by the model
    /// test exactly as retail skips it.
    ///
    /// ONE routine, THREE call paths ([`Self::spawn_rival`], the
    /// respawn tail of [`Self::rival_respawn`], and the human's
    /// `World::player_respawn` amnesty) — see
    /// [`no_mc1_truce_roster`].
    pub(crate) fn mc1_truce_roster(&mut self, colour: usize, own_ent: u16) {
        if colour >= 8 {
            return;
        }
        for c in 0..self.g.wiz_chain.visible_len() {
            let j = self.g.wiz_chain.list[c] as usize;
            if j == own_ent as usize {
                continue;
            }
            if let Some(r) = self.rivals.iter_mut().find(|r| r.ent as usize == j) {
                r.hate[colour] = HATE_RESPAWN;
            }
        }
    }

    /// A rival-owned class-12 manifestation (the shared sub_3BF70
    /// slot economy; f144 = the owner tag — PLAYER_TARGET on the
    /// human's, 0 on ground jars; retail keeps the owner in +42 and
    /// +144 dead — the importer and every native mint normalize into
    /// f144).
    /// ⭐⭐ IT IS THE GROUND-JAR CTOR, RUN AT THE WIZARD'S OWN
    /// POSITION — every retail mint site hands the class-12 thunk
    /// `&wizard.position`: the learn expiry
    /// (`off_987DE[s].adress(a1 + 72)`, :19417), the level-start book
    /// and the respawn re-grant (`sub_373F0(&wiz.pos, 12, model)`,
    /// :54900). The token is then stamped OWNED — `+16 |= 1` and
    /// `+42 = the wizard slot` (:19428-29 / :54905-06) — and pushed
    /// onto the acquisition list by the caller.
    ///
    /// The old port mint wrote four fields onto a bare `new_event`
    /// and never LINKED it, so a rival's conjured token stood at the
    /// origin with `NewEvent`'s 300 life and an empty cost cache.
    /// mc1hwl0 t=2350 is the whole of it: rival 1's Armageddon
    /// countdown expires and retail conjures `(12,20)` at slot 256
    /// carrying `flags 5`, `+50 = 26`, `+136 = 5000`, `+140 = 192`,
    /// life `0/0` and the wizard's own x/y/z, against the port's
    /// zeros and `max_life 300`. [`World::spawn_spell_jar`] is that
    /// ctor verbatim and was already right for the world's jars —
    /// ⭐ *when two paths model one retail constructor, DIFF THEM*
    /// ([[mc1-jar-poll-walk-slot]] again).
    ///
    /// ⚠ `+70` follows the world's encoding, not a fixed one: a
    /// conformance import carries retail's `3·spell + phase` (phase 0
    /// = an owned token) and `class12_tick` dispatches on it, while a
    /// native world uses [`MANIFEST_BASE`]` + spell`. The `+16` bit 0
    /// is what tells the strict arm a `3·spell` record is a TOKEN and
    /// not a jar, so it is not decoration.
    fn mint_manifestation(&mut self, spell: usize, owner: u16) -> Option<usize> {
        let (wx, wy, wz) = {
            let e = self.g.ent.get(owner as usize)?;
            (e.x, e.y, e.z)
        };
        let state = if self.strict_retail {
            (spell * 3) as u8
        } else {
            crate::engine::world::MANIFEST_BASE + spell as u8
        };
        let m = self.spawn_spell_jar(spell, state, wx, wy, wz)?;
        {
            let e = &mut self.g.ent[m];
            e.flags |= 1; // :19428 / :54906 — the OWNED-token bit
            e.f26 = 0;
            // Retail's +42 = the owning wizard's slot; the port homes
            // that lane at f144 for class 12 (the importer's own
            // normalization — conformance.rs `f144: tr(r.f42)`).
            e.f144 = owner;
        }
        Some(m)
    }

    /// Starting castle: (3,2) at the wizard, level = castle_level-1,
    /// footprint terrain replayed per stage (the sub_279D0 loop
    /// :54982-93), capacity ladder, spawns FULL (:54996-55002),
    /// sound 30 (:54981).
    fn spawn_starting_castle(&mut self, r: &mut Rival, castle_level: u8) {
        let (wx, wy) = {
            let e = &self.g.ent[r.ent as usize];
            (e.x, e.y)
        };
        let gz = self.g.ground_z(wx, wy) as i16;
        let Some(c) = self.g.spawn_class3(2, wx, wy, gz) else {
            return;
        };
        let lvl = (castle_level - 1).min(7);
        {
            let e = &mut self.g.ent[c];
            e.id24 = r.ent;
            e.f26 = lvl as i16;
            // ⚠ NO `+70` write here: the mint (:54974-55002) calls the
            // ctor and never touches the job byte, so the authored
            // castle stands at the ctor's TRANSFORM state (5, sub-state
            // 0) and its FIRST tick runs the level-up commit — which is
            // what carries `+26` from `count - 1` to the authored level
            // and paints the top row. The mc1l5 capture reads it
            // mid-flight: castle 680 is `+70 = 5, +48 = 4` at t=1 (the
            // commit's own wait, :56469) and only settles later.
            // A hard `tick70 = 4` here — added when `f59` alone drove
            // the machine and this write was inert — now suppresses
            // that commit and leaves every authored castle one level
            // short with an unpainted top.
        }
        // Ctor sprite row 177 FLAT: the owner's team color lands via
        // the FIRST level-up commit's one-time `+86 += wizard +48`
        // stamp (:56057-62, the :30809-10 family) — for the authored
        // castle that is its first tick (the `+70` note above). A
        // pre-stamped 177+slot here would double-color the row now
        // that the commit stamp is ported.
        self.g.set_sprite(c, 177);
        // Terrain: replay the build painter per stage (instant, the
        // divisor-1 flatten + paint). Retail's loop runs one pass per
        // AUTHORED LEVEL with the build row = the pass index
        // (:54983-91 `+29866 = i`, i = 0..count-1), so the rows
        // stamped are 0..=lvl — and row 0 is EMPTY (w = h = 0). Our
        // rows 1..=lvl cover exactly the same ground; passing lvl + 1
        // stamped one row too many, which for the common authored
        // level 0 (`castle_level` 1) raised a whole level-1 tower that
        // the castle never owned and the demolish never removed.
        let (cx, cy, cz) = {
            let e = &self.g.ent[c];
            (
                ((e.x as u32 + 128) >> 8) as u8,
                ((e.y as u32 + 128) >> 8) as u8,
                (e.z >> 5) as i32,
            )
        };
        self.g.stamp_castle_terrain(lvl as usize, cx, cy, cz);
        self.terrain_dirty = true;
        // Extents + capacity ladder + full stored mana (cap 320000).
        // The box stamps at the AUTHORED level, level 0 included
        // (:54995 `sub_37150(v17x, byte_38C97[…] - 1)` — the guard
        // that skipped level 0 here was invented).
        self.g.castle_extents(c, lvl);
        let cap = Gen::CASTLE_CAP[lvl as usize];
        self.g.ent[c].f136 = cap;
        self.g.ent[c].f140 = cap.clamp(0, 320_000);
        // The rebuild binds the owner's wizext+50 at the mint
        // (:54980 `var_50 = v17x - pool`) — without it every
        // register-read gate sees a castle-capable wizard as
        // castle-less until the first level-up commit re-binds.
        self.g.castle_reg[r.slot as usize & 7] = c as u16;
        self.g.snd(30, c);
        let _ = r;
    }

    /// The rival's castle: (3,2) with id24 = the rival's entity (the
    /// original's Type_160.var_50, resolved by scan like
    /// [`World::player_castle`]).
    pub(crate) fn rival_castle(&self, ent: u16) -> Option<usize> {
        (1..self.g.ent.len()).find(|&j| {
            let e = &self.g.ent[j];
            e.class64 == 3 && e.model65 == 2 && e.flags & 0x400 == 0 && e.id24 == ent
        })
    }

    /// The rival's ESTABLISHED castle as retail resolves it: `pool +
    /// 164 * wizext[+50]` with an INDEX-NONZERO test alone — no class,
    /// model, life, owner or flags test, so a register naming a slot the
    /// pool has since re-minted resolves to whatever lives there now.
    /// Every MC1 rival reader of the castle takes this form (the Home
    /// predicates and handler, the mana-hunt anchor, the cast-16 split,
    /// the castle-required token gate, the dead-wait and the respawn);
    /// none scans. `MGC_NO_MC1_HOME_CASTLE_REGISTER=1` restores the
    /// [`Self::rival_castle`] pool scan on all of them. See
    /// [`crate::engine::features::no_mc1_home_castle_register`].
    pub(crate) fn rival_castle_reg(&self, ri: usize) -> Option<usize> {
        if crate::engine::features::no_mc1_home_castle_register() {
            return self.rival_castle(self.rivals[ri].ent);
        }
        let reg = self.wiz_castle_reg(self.rivals[ri].slot) as usize;
        (reg != 0 && reg < self.g.ent.len()).then_some(reg)
    }

    /// Read-only AI diagnostic dump (no state mutation, not hashed) — for
    /// "follows target, casts nothing" style rival-AI investigations.
    #[doc(hidden)]
    pub fn debug_rival_ai(&self) -> Vec<crate::mc1::rivals::RivalAiDebug> {
        self.rivals
            .iter()
            .enumerate()
            .map(|(ri, r)| {
                let castle = self.rival_castle(r.ent);
                RivalAiDebug {
                    slot: r.slot,
                    state: format!("{:?}", r.state),
                    target: r.target,
                    known: (0..SPELL_COUNT).filter(|&s| r.known[s]).collect(),
                    owned: (0..SPELL_COUNT).filter(|&s| r.owned[s] != 0).collect(),
                    allowed: (0..SPELL_COUNT).filter(|&s| r.allowed[s]).collect(),
                    has_offense: self.rival_has_offense(ri),
                    mana: r.mana,
                    mana_max: r.mana_max,
                    poverty: r.poverty,
                    burst: r.burst,
                    castle_stored: castle.map(|c| self.g.ent[c].f140.max(0) as u32),
                }
            })
            .collect()
    }

    /// Resolve an owner tag (projectile id24 / claim f144) to a
    /// player slot: PLAYER_TARGET = 0 (the human), a live rival's
    /// entity slot = its player slot. Consults both rival columns.
    pub(crate) fn owner_slot(&self, owner: u16) -> Option<u8> {
        if owner == PLAYER_TARGET {
            return Some(0);
        }
        self.rivals
            .iter()
            .find(|r| r.ent == owner)
            .map(|r| r.slot)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.ent == owner)
                    .map(|r| r.slot)
            })
    }

    // ---- the per-tick brain (sub_13170 :17842) ---------------------------

    /// Class-3 model-1 pool dispatch: resolve the rival record; a
    /// level-authored husk with no record stands and renders (the
    /// pre-rivals behavior).
    /// Drain [`Gen::mc1_buffet_post`] — the kraken buffet's write on a
    /// POOL wizard's knock pair (see [`no_mc1_kraken_buffet_rival`]):
    /// `+24 = bearing`, `+22 = 80`. Called by the walk right after the
    /// kraken's own dispatch, so the value stands for every later slot
    /// of the tick, retail's phase. A target with no rival record (an
    /// authored husk) takes nothing — retail's `+160` there is whatever
    /// the level left, not a wizext the port models.
    pub(crate) fn mc1_buffet_apply(&mut self) {
        let HashSilent((tgt, dir)) = std::mem::take(&mut self.g.mc1_buffet_post);
        if tgt == 0 {
            return;
        }
        if let Some(ri) = self.rivals.iter().position(|r| r.ent == tgt) {
            self.rivals[ri].knock_dir = dir;
            self.rivals[ri].knock_mag = 80;
        }
    }

    pub(crate) fn rival_entity_tick(&mut self, i: usize) {
        let Some(ri) = self.rivals.iter().position(|r| r.ent as usize == i) else {
            return;
        };
        // ⭐ `sub_46480`'s FIRST statement (:55601, shipped file
        // 0x5EC85 `movw $0x0,0x16(%eax)`): every state-3 dispatch —
        // eliminated husk, human-driven watch, countdown or castle-
        // less — wipes the knock MAGNITUDE before its AI/human fork.
        // The bearing (+24) is left standing; only the respawn clears
        // it. See [`no_mc1_dead_wait_knock_clear`].
        if self.g.ent[i].tick70 == 3 && !no_mc1_dead_wait_knock_clear() {
            self.rivals[ri].knock_mag = 0;
        }
        if self.rivals[ri].eliminated {
            // ⭐ NOT INERT. An eliminated wizard's record still enters
            // the state-3 handler every tick, and `sub_46480`'s ELSE
            // arm (:55621-22) runs `sub_463B0` — the watch aim. The
            // rest of that handler (the respawn countdown, the
            // castle-bound release) is what elimination takes away.
            if self.g.ent[i].tick70 == 3 && husk_watch_gate() && self.rivals[ri].human_driven {
                self.rival_watch_track(i);
            }
            return;
        }
        match self.g.ent[i].tick70 {
            // Death fall (state 2, sub_45FC0 :55434).
            2 => self.rival_death_fall(ri, i),
            // Dead on the ground (state 3, sub_46480 :55594).
            3 => self.rival_dead_wait(ri, i),
            // Alive (state 1).
            _ => self.rival_alive_tick(ri, i),
        }
    }

    fn rival_alive_tick(&mut self, ri: usize, i: usize) {
        // Pull combat-side debits out of the +140 mana mirror (the
        // deflection quarter, sub_52B30 :62884, writes the ENTITY
        // field). Downward-only: every port-side credit lands in
        // `Rival::mana` first and re-publishes at this tick's end.
        let mirrored = self.g.ent[i].f140.max(0) as u32;
        if mirrored < self.rivals[ri].mana {
            self.rivals[ri].mana = mirrored;
        }
        // ---- housekeeping (sub_132B0 :17903) ----
        // Burst lockout recovery (:17936-38).
        if self.rivals[ri].burst < 0 {
            self.rivals[ri].burst += 1;
        }
        // AI recast cooldowns (:17939-45).
        for c in self.rivals[ri].cooldown.iter_mut() {
            *c = c.saturating_sub(1);
        }
        self.rival_hate_decay(ri);
        // sub_45C10 at :17969 — the owned register is a per-tick
        // PROJECTION of the acquisition list, rebuilt at the TOP of
        // the dispatch, BEFORE the learn expiry's mint at :18022: a
        // spell minted at T reaches owned[] at T+1 (the human's
        // opposite phase is call order — :55342 — not law). The port
        // used to hand-maintain owned at the mint. mc1l37's rivals
        // start castle-less and learn Create Castle mid-take; the
        // one-tick-early owned[16] moved the whole castle-build
        // cascade a tick ahead (t=2487 family).
        self.rival_owned_rebuild(ri);

        // At own castle: grace 2 + the mailbox is DISCARDED — the
        // AI's damage does NOT forward into the castle. VERIFIED
        // verbatim (:17971-79: overlap test sub_11950 → +331=2;
        // while +331: memset(+90,0,36), no intake). The asymmetry
        // vs the human's explicit redirect (:55353-62) is retail's
        // own; the castle still takes AREA-blast collateral through
        // its normal ch0 mail, which is how a camping rival's
        // castle falls in retail.
        // ⭐ The castle resolves through wizext+50, the BOUND register
        // (:17971 `v14 = wizext+50`): an authored castle that never
        // leveled grants neither the mail-discard grace nor the fast
        // regen fork. And the probe is `sub_11950` = the FULL summed-
        // extents AABB (signed +78 z leg) — the same law the human's
        // `regen_boost` already wears (mc1l0 t=1827). mc1l5 t=11681:
        // Vodor brushes his keep's summed box at |dx| 3362 vs
        // 3328+125, and retail flips him to the at-castle +1000/tick
        // where the port's bare `<= f80/f82` point test kept the
        // away-rate +100.
        // ⭐⭐ …and "bound" IS the register word (`Gen::castle_reg`),
        // dereferenced with an index test alone (:17971 `if (v14 &&
        // sub_11950(a1, pool + 164*v14))` — file 0x2BBF9-0x2BC3B). The
        // `flags & 2` first-commit latch the port used to demand is
        // the :56057 team-recolor stamp, which an AUTHORED castle
        // never earns (mc1l20 slot 534 `flags 12` all take) — so the
        // rival sat on his own keep at the afield +100/tick. See
        // [`rival_castle_register`].
        let at_castle = if rival_castle_register() {
            let reg = self.wiz_castle_reg(self.rivals[ri].slot) as usize;
            reg != 0 && reg < self.g.ent.len() && self.g.ent_overlap(i, reg)
        } else {
            self.rival_castle(self.rivals[ri].ent)
                .filter(|&c| self.g.ent[c].flags & 2 != 0)
                .is_some_and(|c| self.g.ent_overlap(i, c))
        };
        if at_castle {
            // Retail SETS 2 (:17975 `+331 = 2`) — a spawn grace still
            // counting is OVERWRITTEN at the own castle, not floored.
            self.rivals[ri].grace = 2;
        }
        if self.rivals[ri].grace > 0 {
            self.rivals[ri].grace -= 1;
            self.g.ent[i].mail = [(0, 0); 6];
        } else {
            self.rival_damage_intake(ri, i);
            if self.g.ent[i].act_life < 0 {
                // ⭐ THE RIVAL'S DEATH ARM WRITES `+70 = 2` AND
                // NOTHING ELSE (:17980-83 — `else if (sub_46540(a1)
                // == 2) { *(a1+70) = 2; return 0; }`). The human's
                // twin (:55424-29) additionally zeroes `+46` and
                // screams sound 16; that arm belongs to sub_45C90,
                // which a rival never runs (sub_13170 is its carpet
                // tick — the same ownership split the +528 wanted
                // decay wears). Both writes were the port reading the
                // human's site for the rival's.
                //
                // So the corpse ENTERS THE FALL AT ITS LIVE CLIMB
                // RATE: mc1hwl0 t=23252, the dying rival carries
                // `+46` = −56 from its last flight tick and drops 56
                // on the very next fall step (z 2769 → 2713) where
                // the zeroed port hung at 2769 and only then began
                // accelerating. `+46` is not published by the pair
                // grader, so only the free run ever saw it.
                self.g.ent[i].tick70 = 2;
                // ⭐⭐⭐ A RETAIL CORPSE KEEPS DEFLECTING. This clear
                // was INVENTED, and the comment that justified it
                // ("the port drives rival tokens only from
                // `rival_refresh_buffs`, which death states 2/3 never
                // reach") went stale when `rival_rebound_token_tick`
                // moved to the class-12 walk (`engine/world.rs:11417`
                // / `:11603`) — the token now lapses the bit on its
                // own schedule, so the safety net is both redundant
                // and wrong.
                //
                // The rival death arm is `*(a1 + 70) = 2; return 0;`
                // and NOTHING else (:17980-83), and `+17` bit 7 — our
                // 0x8000 — has EXACTLY TWO writers in the whole
                // listing, both inside the token's own tick
                // `sub_573F0_57920` (:65785 `&= ~0x80u`, :65792
                // `|= 0x80u`; twin remc1hw :62007 / :62014). Nothing
                // in the death path writes it.
                //
                // mc1l49 t=6669 settles it from the RECORDING: slot
                // 646 takes a fatal 4000 (`act_life 2040 → −1960`)
                // and `explain`'s changelog lists `act_life`, `f38`,
                // `f63`, `f70`, `mail0.src` — and NO `flags` row. The
                // word is 0x800C at t=6666..6668, stays 0x800C through
                // the death, goes 0x802C at 6670, holds through death
                // states 2 AND 3 to t=6700, and only the RESPAWN
                // (t≈6750) rewrites it to 0x000C.
                // `MGC_NO_MC1_DEATH_KEEPS_REBOUND=1` restores the clear.
                if !death_keeps_rebound() {
                    self.g.ent[i].flags &= !0x8000;
                }
                // ⚠ The death scream is cited to :55424-30 — the
                // HUMAN's arm, the same mis-owned site the `+46 = 0`
                // above came from; the rival arm (:17980-83) sounds
                // nothing. Kept for now because the sim's sounds vec
                // is HASHED (removing it re-bases every golden) and
                // no capture grades it. Open: find whether a rival
                // death screams at all, and from where.
                self.g.snd(16, i);
                // The death arm returns from the HOUSEKEEPING only —
                // its caller runs the state handler regardless. See
                // [`Self::rival_dispatch_tail`].
                self.rival_dispatch_tail(ri, i);
                return;
            }
        }

        // Movement (sub_14EB0 :18780).
        self.rival_movement(ri, i);

        // The cast-charge meter (u8_326): +1 per live rival tick,
        // saturating at 200 (:17987-89) — right before the regen
        // block, exactly like retail's rival handler.
        let ws = self.rivals[ri].slot as usize;
        if self.wiz_charge[ws] < 200 {
            self.wiz_charge[ws] += 1;
        }
        // Regen (:17990-18021): mana += delta then recompute; life
        // regen at the AI's own (faster) rates. The dolmen-shrine
        // flag (+17 0x10, our 0x1000 — stamped by the dolmen's
        // sub_49AD0 sweep) rides the same fast/slow fork as the
        // own-castle overlap and is consumed (cleared) by the fast
        // branch (:18002-09).
        let at_shrine = self.g.ent[i].flags & 0x1000 != 0;
        {
            let r = &mut self.rivals[ri];
            // The purse reads SIGNED here (:55385-91 on `+140`): an
            // intake over-debit this same dispatch left a wrapped
            // shortfall, and retail's step adds the delta to the
            // NEGATIVE value then floors at 0 (the quantum is eaten,
            // not clipped where it landed).
            let stepped = r.mana as i32 as i64 + r.mana_delta as i64;
            r.mana = stepped.clamp(0, r.mana_max as i64) as u32;
            r.mana_delta = if at_castle || at_shrine {
                ((r.mana_max / 200) as i32).max(1000)
            } else {
                ((r.mana_max / 2000) as i32).max(100)
            };
        }
        if at_castle || at_shrine {
            self.g.ent[i].flags &= !0x1000;
        }
        // Life regen (:17990-18018): act_life += the RATE REGISTER,
        // floor −1 / ceiling max, then the register re-selects from
        // the same at-castle/shrine fork as the mana delta —
        // UNCONDITIONALLY. The AI regen has NO stall gate: the shared
        // intake arms +383 = 16 on every processed hit, but only the
        // human's regen tail reads that field; retail's rival heals
        // +20 straight through a −100 fireball tick (the l2 corpus
        // life lane measures the −80 net).
        {
            let max = self.g.ent[i].max_life as i32;
            let rate = self.rivals[ri].life_rate;
            let e = &mut self.g.ent[i];
            e.act_life = (e.act_life + rate).clamp(-1, max);
            self.rivals[ri].life_rate = if at_castle || at_shrine {
                max / 200
            } else {
                max / 500
            };
        }
        // ⚠ NO regen_stall decrement here: the shared intake arms
        // +383 = 16 on every processed hit, but the ONLY decrement in
        // retail lives in sub_45C90 (class-3 state 0 — the human's
        // record tick, str_254ADC). The rival tick never reads or
        // decays it, so a hit rival's +383 sits at 16 forever
        // (mc1hwl0: pinned from t=1361 to end of take). The port used
        // to invent a decay, which made the mirror lie on every
        // shadow row after the first hit.

        // Spell learning (sub_15EC0 :19381-443).
        self.rival_learn_tick(ri);

        // Buff flags from the manifestations' bursts.
        self.rival_refresh_buffs(ri);

        // Decision-tick work (:18024): incoming-projectile defense +
        // heal.
        let think = self.g.ent[i].f63 % self.rivals[ri].think_period() == 0;
        if think {
            self.rival_defense(ri, i);
            if self.g.ent[i].act_life < self.g.ent[i].max_life as i32 {
                self.rival_cast(ri, i, 1);
            }
        }

        // Altitude hard clamp (:18035-41).
        {
            let row = &BEHAVIOR[self.g.ent[i].row156 as usize];
            let ground = self.g.ground_z(self.g.ent[i].x, self.g.ent[i].y) as i16;
            let z = &mut self.g.ent[i].z;
            *z = (*z).clamp(
                ground.saturating_add(row.v_12),
                ground.saturating_add(row.v_10),
            );
        }

        self.rival_dispatch_tail(ri, i);
    }

    /// The brain's own body (sub_13170 :17846-51): the state handler
    /// and the decision cascade (a Fresh rival runs the cascade
    /// twice). It sits in the CALLER of the housekeeping, and the
    /// caller DISCARDS the housekeeping's return — so the death arm
    /// (:17980-84) ends `sub_132B0` only, never the brain. The dying
    /// rival therefore still runs its state handler on the fatal tick,
    /// which for an attack state is the hover leg (the cast attempt
    /// itself refuses — mc1l2 t=8278: Vodor is poverty-latched at mana
    /// 100/1000, so retail hovers 1514 → 1510 on row 8's v_14 = −4 and
    /// does not cast). Verbatim in the HW twin (:16112-17).
    fn rival_dispatch_tail(&mut self, ri: usize, i: usize) {
        let think = self.g.ent[i].f63 % self.rivals[ri].think_period() == 0;
        let fresh = self.rivals[ri].state == AiState::Fresh;
        self.rival_state_tick(ri, i, think);
        self.rival_selector(ri, i, think);
        if fresh {
            self.rival_selector(ri, i, think);
        }
        // Publish the +140 mana mirror for the combat reads.
        // ⭐ THE MIRROR IS A RAW 32-BIT COPY, NOT A CLAMP. Retail's
        // `+140` is the wizard's purse word itself; the shield-quarter
        // debit above subtracts RAW (`sub_46540_46880` :55703, on the
        // RECORD's own `+140`) and a fatal tick's shortfall WRAPS the
        // word negative, which the death arm then preserves (it
        // returns before the regen floor at :17990). The old
        // `.min(i32::MAX)` turned every wrapped purse into
        // `2147483647`. WITNESS mc1l49 pair 53215→53216, rival slot
        // 750 `(3,1)` life −301: retail `+140` = 4294964965
        // (= −2331 signed), the port published 2147483647.
        // `MGC_NO_MC1_RIVAL_MANA_WRAP_PUBLISH=1` restores the clamp.
        self.g.ent[i].f140 = if crate::engine::features::no_mc1_rival_mana_wrap_publish() {
            self.rivals[ri].mana.min(i32::MAX as u32) as i32
        } else {
            self.rivals[ri].mana as i32
        };
        self.entities_dirty = true;
    }

    /// Hate regression toward the baseline (:17946-67): below rises
    /// by agg+1, above decays by 256-agg — but a war flag pins it.
    fn rival_hate_decay(&mut self, ri: usize) {
        let (agg, war) = (self.rivals[ri].agg, self.rivals[ri].war);
        for (p, h) in self.rivals[ri].hate.iter_mut().enumerate() {
            if *h < HATE_NEUTRAL {
                *h = (*h + agg + 1).min(HATE_NEUTRAL);
            } else if *h > HATE_NEUTRAL && !war[p] {
                *h = h.saturating_sub(256 - agg).max(HATE_NEUTRAL);
            }
        }
    }

    /// The shared wizard damage intake (sub_46540 :55641) on the
    /// rival's mailbox. EVERY channel gates on its SOURCE word, and a
    /// consume clears ONLY the source — the amount stays behind as
    /// permanent residue retail never re-reads (the l2 corpus: Vodor
    /// carries a dead `(1400, 0)` ch0 letter for thousands of ticks;
    /// re-applying it at every imported pair was the 8k-row life
    /// family). Hate feed lives in `proj_hate_sweep`.
    fn rival_damage_intake(&mut self, ri: usize, i: usize) {
        // ch4 duel grip (:55663-82): the CASTER gets pulled toward
        // this victim; the victim only takes the side effects
        // (regen stall — the pull state lives on the ATTACKER).
        let (grip_amt, grip_src) = self.g.ent[i].mail[4];
        if grip_src != 0 {
            self.rivals[ri].regen_stall = 16;
            self.g.ent[i].mail[4] = (grip_amt, 0);
            if self.owner_slot_of_source(grip_src) == Some(0) {
                // u16_314/316/318 on the human: victim, counter,
                // clamp(dist, 1024, 3072) (:55671-77). ⚠ THE HOLD IS
                // 3-D — :55673 measures with `sub_42340_42680`, the
                // same helper the grip's own release test and closing
                // rate use (`flight::mc1_duel_dist`), not a ground
                // separation. Under
                // `MGC_NO_MC1_DUEL_GRIP_EXACT=1` the old 2-D hold
                // comes back with the rest of the pre-D18 grip.
                let e = &self.g.ent[i];
                let (vx, vy, vz) = (e.x, e.y, e.z);
                let dist = if crate::engine::world::mc1_duel_grip_exact_off() {
                    Gen::isqrt(Gen::dist2_sq(self.human_pose.0, self.human_pose.1, vx, vy) as u32)
                } else {
                    crate::flight::mc1_duel_dist(self.human_pose, (vx, vy, vz)) as u32
                };
                let hold = dist.clamp(1024, 3072);
                self.set_duel_latch(self.rivals[ri].ent, hold);
            }
        }
        // ch3 mana steal (:55689-91): the victim's subtract is RAW —
        // no clamp exists in retail, the only floor is the same-
        // dispatch regen step below (signed, so an over-steal eats
        // the tick's quantum) — and the attacker banks the FULL
        // amount, not the victim's remainder (mc1hwl0 t=23438+ slot
        // 473: the thief reads 100 off a broke victim where the
        // clamped take banked 0 ×24).
        let (steal_amt, steal_src) = self.g.ent[i].mail[3];
        if steal_src != 0 {
            self.rivals[ri].mana = self.rivals[ri].mana.wrapping_sub(steal_amt);
            self.credit_wizard_mana(steal_src, steal_amt);
            self.rivals[ri].regen_stall = 16;
            self.g.ent[i].mail[3] = (steal_amt, 0);
        }
        // ch0 damage (:55694-56737): SRC-gated — a src-0 letter is
        // dead residue, not applied and not cleared.
        let (amt, src) = self.g.ent[i].mail[0];
        if src == 0 {
            return;
        }
        let mut dmg = amt.min(i32::MAX as u32) as i32;
        // Shield quarter (:55700-07): keyed on the ENTITY's 0x4000
        // bit (retail +17 & 0x40) — the imported flag, not the
        // port-side buff mirror — quartered amount written BACK to
        // the letter (the residue keeps the reduced value), mana pays
        // it, and the bit clears ONE-SHOT.
        if self.g.ent[i].flags & 0x4000 != 0 {
            dmg /= 4;
            // RAW subtract (:55703) — retail has no afford clamp on
            // the shield quarter; a fatal tick's shortfall rides the
            // corpse (the death arm returns before the regen step,
            // the only floor).
            self.rivals[ri].mana = self.rivals[ri].mana.wrapping_sub(dmg.max(0) as u32);
            self.g.ent[i].flags &= !0x4000;
        }
        self.g.ent[i].act_life -= dmg;
        // Knockback (:55714-19), armed on ANY sourced letter — the
        // fatal one included, since this block precedes the death
        // return at :55726. v_24 = the attacker→victim bearing, v_22 =
        // amount/10 clamped to [0, 80]. Retail's gate is just
        // `src > 0`: the human is a pool entity there, but the port
        // stamps human-fired projectiles with PLAYER_TARGET, so that
        // case reads the pinned human pose instead (as `home` and the
        // area writers already do) — without it every rival the PLAYER
        // kills would drop straight down and the law would be
        // corpus-only.
        // ⭐⭐ AND THE SOURCE NEED NOT STILL BE ALIVE — :55712's gate is
        // the POINTER COMPARE `if (v13x > v12x)`, i.e. `src > 0`, and
        // `v14x = &v13x->var_u32_29867_72` is taken off the record
        // whatever its class. MC2's twin arm says the same thing
        // (EF:61037) and mc2l4 t=8541 proves it on the recording. The
        // `class64 != 0` here was an invented guard.
        let attacker = if src == PLAYER_TARGET {
            Some((self.human_pose.0, self.human_pose.1))
        } else {
            let s = src as usize;
            (s != 0 && s < self.g.ent.len())
                .then(|| (self.g.ent[s].x, self.g.ent[s].y))
                .filter(|_| !no_mc1_knock_from_dead_source() || self.g.ent[s].class64 != 0)
        };
        if let Some((ax, ay)) = attacker {
            let (vx, vy) = (self.g.ent[i].x, self.g.ent[i].y);
            // :55714 stores `sub_42150`'s return RAW (it can be 2048;
            // the mover masks) — see [`no_mc1_knock_dir_raw`].
            let raw = Gen::angle_between(ax, ay, vx, vy);
            self.rivals[ri].knock_dir = if no_mc1_knock_dir_raw() {
                raw & 0x7FF
            } else {
                raw
            };
            self.rivals[ri].knock_mag = ((dmg.max(0) / 10) as i16).clamp(0, 80);
        }
        self.rivals[ri].regen_stall = 16;
        self.g.snd(17, i);
        if self.g.ent[i].act_life < 0 {
            // Death (:55734-36): the killer latch stamps ONLY here,
            // and the letter is NOT consumed — the corpse keeps it.
            self.g.ent[i].f38 = src;
            self.g.ent[i].mail[0] = (dmg.max(0) as u32, src);
            return;
        }
        // Survive: consume = clear the source, keep the (possibly
        // quartered) amount (:55738-40).
        self.g.ent[i].mail[0] = (dmg.max(0) as u32, 0);
    }

    /// Resolve a mailbox source id to the attacking wizard's slot:
    /// sources carry the attacker's owner tag directly (our writers
    /// pass id24 through).
    pub(crate) fn owner_slot_of_source(&self, src: u16) -> Option<u8> {
        if src == PLAYER_TARGET {
            return Some(0);
        }
        // A pool slot: use its owner tag; a wizard entity is its own.
        let e = self.g.ent.get(src as usize)?;
        if e.class64 == 3 && e.model65 <= 1 {
            return self.owner_slot(src);
        }
        self.owner_slot(e.id24)
    }

    /// Bump the ledger (`str_456[shooter].u16_4` += amount, clamped —
    /// :19727-32). No war check here: retail raises the flag ONLY in
    /// the castle arm of the sweep (:19733-39); the carpet/balloon
    /// and mana-ball arms bump and stop.
    fn rival_add_hate(&mut self, ri: usize, shooter: u8, amount: u16) {
        // ⭐ NO SELF TEST (round 168). The ledger arms index the
        // victim-owner's table by the shooter's colour and add; when
        // the two are one wizard the row is its own. mc1hwl14 t=62:
        // wizard 3 lobs Possess at a sphere it already claims (716
        // mana) and its `hate[3]` reads 24607 -> 24786, the claimed-
        // ball arm's `mana / 4`; the port's invented exclusion left it
        // flat. `MGC_NO_MC1_SELF_HATE=1` restores the exclusion.
        if shooter as usize >= 8
            || (self.rivals[ri].slot == shooter && crate::engine::features::no_mc1_self_hate())
        {
            return;
        }
        let r = &mut self.rivals[ri];
        r.hate[shooter as usize] = r.hate[shooter as usize].saturating_add(amount);
    }

    /// The castle-arm war check (:19733-39): hate past the threshold
    /// raises the war flag. ⭐⭐ THE MC1 THRESHOLD IS FLAT 50000: the
    /// listing scales it by an aggression word read through the victim
    /// CASTLE entity's +160 (`*(v3+160)+522`) — but only CARPETS carry
    /// the wizext pointer at +160; a castle's is the mint's zero, so
    /// the read lands in low memory and the scaled term never
    /// contributes (the unguarded-pointer constant class — the same
    /// shape as the l42 `+146` null-probe ruling). Measured on mc1l5's
    /// four hate windows: war latches at 50518/50659/51518, each the
    /// FIRST crossing above 50000, never at 49518/49659, and window 4
    /// peaks at 49531 and decays out unlatched — the rival's real agg
    /// (115; the decay's 256−agg = 141, t=14600) would have latched
    /// five ticks early at 44505. The MC2 twin (EF:7402-03) reads real
    /// wizard structs (`v1x->maxMana_0x8C * v2x_owner->word_0x242`),
    /// so its threshold IS wealth-scaled; this ruling is MC1's alone.
    fn rival_war_check(&mut self, ri: usize, shooter: u8) {
        if shooter as usize >= 8 || self.rivals[ri].slot == shooter {
            return;
        }
        let r = &mut self.rivals[ri];
        if r.hate[shooter as usize] as u32 > 50_000 {
            r.war[shooter as usize] = true;
        }
    }

    /// The per-projectile hate/war ledger sweep `sub_16540` (:19643),
    /// called once per tick from [`crate::engine::world`]'s tick
    /// between the reap/list phase and the mana census (:52326,
    /// ahead of every entity handler). Each class-9 record is
    /// ledgered ONCE — flags 0x2000 is the mark (:19666/:19678), set
    /// the first tick the bolt has BOTH a class-3 owner and a victim
    /// in +146, whether or not a table below applies. A bolt that
    /// MISSED at the muzzle never latches (no victim), and is
    /// re-examined every tick until it dies or acquires one (the
    /// rebound path can hand it a victim mid-flight).
    ///
    /// The corpus-visible half is the mark itself — mc1l0's 202-row
    /// `flags want 8198 got 6` family. The tables drive rival
    /// aggression: victim's wizard gains hate against the shooter,
    /// keyed on the PROJECTILE model ({3,4,11,16} heavy → +3000
    /// carpet/balloon, +5000 castle; model 10 → nothing; else
    /// +500/+1000), the castle arm alone running the war check. A
    /// possess lob (m1) locked onto a CLAIMED mana ball (10,39)
    /// bumps the claimant's wizard by ball_mana/4 (:19742-61).
    ///
    /// ⚠ Two remc1 transcription slips corrected via the MC2 twin
    /// `sub_159E0` (EF:7320): the carpet-arm base-read is the
    /// VICTIM-owner's table (remc1's text reads `v2->id24` — the
    /// shooter's; the twin reads `ent[target.id]`), and BOTH arms
    /// key the bonus on the projectile MODEL (the text reads +63 in
    /// the carpet arm; the twin reads `model_0x40` in both). MC2 is
    /// not wired to this sweep yet — zero class-9 flags signal in the
    /// four MC2 takes; its own frame does call the twin (EF:786).
    ///
    /// Human-victim writes go to retail's human T160 tables, which
    /// nothing consumes (the human has no AI); the port keeps no such
    /// store, so those arms latch the mark and stop.
    pub(crate) fn proj_hate_sweep(&mut self) {
        for i in 1..self.g.ent.len() {
            let e = &self.g.ent[i];
            if e.class64 != 9 || e.flags & 0x2000 != 0 {
                continue;
            }
            let (own, tgt, model) = (e.id24, e.f146, e.model65);
            let owner_ok = own == PLAYER_TARGET
                || (own != 0
                    && (own as usize) < self.g.ent.len()
                    && self.g.ent[own as usize].class64 == 3);
            if !owner_ok || tgt == 0 {
                continue;
            }
            self.g.ent[i].flags |= 0x2000; // ledgered (:19678)
            let Some(shooter) = self.owner_slot_of_source(own) else {
                continue;
            };
            if tgt == PLAYER_TARGET || tgt as usize >= self.g.ent.len() {
                continue; // human tables unmodeled (see above)
            }
            let t = &self.g.ent[tgt as usize];
            let (tclass, tmodel) = (t.class64, t.model65);
            if tclass == 3 {
                let Some(victim) = self.owner_slot_of_source(tgt) else {
                    continue;
                };
                let Some(ri) = self.rivals.iter().position(|r| r.slot == victim) else {
                    continue;
                };
                if tmodel == 2 {
                    let bonus = match model {
                        3 | 4 | 11 | 16 => 5000,
                        10 => 0,
                        _ => 1000,
                    };
                    self.rival_add_hate(ri, shooter, bonus);
                    self.rival_war_check(ri, shooter);
                } else {
                    let bonus = match model {
                        3 | 4 | 11 | 16 => 3000,
                        10 => 0,
                        _ => 500,
                    };
                    self.rival_add_hate(ri, shooter, bonus);
                }
            } else if tclass == 10 && model == 1 && tmodel == 39 {
                // The claimed-ball arm: possessing someone's claimed
                // sphere is an act of war-adjacent theft.
                let claimant = t.f144;
                let mana = t.f140.max(0) as u32;
                if claimant == 0 {
                    continue;
                }
                let claimant_ok = claimant == PLAYER_TARGET
                    || ((claimant as usize) < self.g.ent.len()
                        && self.g.ent[claimant as usize].class64 == 3);
                if !claimant_ok {
                    continue;
                }
                let Some(victim) = self.owner_slot_of_source(claimant) else {
                    continue;
                };
                let Some(ri) = self.rivals.iter().position(|r| r.slot == victim) else {
                    continue;
                };
                let bump = (mana / 4).min(u16::MAX as u32) as u16;
                self.rival_add_hate(ri, shooter, bump);
            }
        }
    }

    /// Credit stolen mana to a wizard by owner tag.
    ///
    /// ⭐⭐⭐ **MC1'S STEAL CREDIT HAS NO CEILING.** `sub_46540_46880`'s
    /// ch3 arm is a raw 32-bit add on the THIEF's record
    /// (`reference/remc1/sub_main.cpp:55689`, twin `remc1hw:51757-58`
    /// byte-identical):
    /// `if (*(_BYTE *)(v8 + 64) == 3) *(_DWORD *)(v8 + 140) += mail3.amt;`
    /// — there is no `+140 > +136` test anywhere near it. Settled in the
    /// shipped bytes at `CARPET.EXE` 0x5EE7F:
    /// `80 78 40 03` `cmp byte [eax+0x40],3` · `75 09` `jne` ·
    /// `8b 53 6c` `mov edx,[ebx+0x6C]` (0x6C = 108 = mail3 amt) ·
    /// **`01 90 8C 00 00 00` `add dword [eax+0x8C],edx`** — a RAW ADD,
    /// with no compare against `+0x88` (136 = mana_max) in the sequence.
    ///
    /// The ONLY ceiling clamp in the MC1 wizard lane is the mana STEP
    /// (:55396-97), and that step runs at the THIEF's own pool slot — so
    /// when the victim is chained ABOVE the thief the credit lands after
    /// his step and **his purse sits over its ceiling for exactly one
    /// tick**, then the next step pulls it back. `f140 > f136` on an MC1
    /// wizard is LEGAL. mc1l48 t=61361: slot 712 (a rival wizard) carries
    /// `ch3 amt = 2000, src = 681`, and at t=61362 the human's `+140`
    /// reads 4,157,277 against an `f136` of 4,156,277.
    ///
    /// The `.min()` here was INVENTED — standing law 4's exact tell, a
    /// `.min()` where the decompile has a plain `+=`. ⚠ MC2 keeps its
    /// clamp: `sub_61050` (EF:62202-20) credits `min(mana + amount,
    /// mana_max)` explicitly, which is a genuinely different law.
    /// `MGC_NO_MC1_RAW_STEAL_CREDIT=1` restores the old behaviour.
    pub(crate) fn credit_wizard_mana(&mut self, owner: u16, amount: u32) {
        let raw = mc1_raw_steal_credit()
            && matches!(
                self.game(),
                crate::ids::GameId::Mc1 | crate::ids::GameId::Mc1Hw
            );
        if owner == PLAYER_TARGET {
            self.player.mana = if raw {
                self.player.mana.wrapping_add(amount)
            } else {
                (self.player.mana + amount).min(self.player.mana_max)
            };
            return;
        }
        if let Some(r) = self.rivals.iter_mut().find(|r| r.ent == owner) {
            r.mana = if raw {
                r.mana.wrapping_add(amount)
            } else {
                (r.mana + amount).min(r.mana_max)
            };
            return;
        }
        if let Some(ri) = self.mc2_rivals.iter().position(|r| r.ent == owner) {
            let r = &mut self.mc2_rivals[ri];
            r.mana = (r.mana as i64 + amount as i64).min(r.mana_max as i64) as i32;
            // Retail's purse IS the entity word `mana_0x90_144`; the
            // brain record only mirrors it. See
            // [`crate::mc2::cast::no_mc2_wiz_purse_is_entity`].
            if !crate::mc2::cast::no_mc2_wiz_purse_is_entity() {
                let m = self.mc2_rivals[ri].mana;
                let t = owner as usize;
                if t < self.g.ent.len() {
                    self.g.ent[t].f140 = m;
                }
            }
        }
    }

    /// AI carpet movement (sub_14EB0 :18780-859): band-settle
    /// altitude, always-level forward step, lateral dodge, 16/tick
    /// accel toward the desired speed, tempo-scaled turn toward the
    /// desired heading. No wall gate, no drag/knock.
    fn rival_movement(&mut self, ri: usize, i: usize) {
        let row = &BEHAVIOR[self.g.ent[i].row156 as usize];
        let (v10, v12, v14) = (row.v_10, row.v_12, row.v_14);
        let (v2, v4) = (row.v_2, row.v_4);
        let ground = self.g.ground_z(self.g.ent[i].x, self.g.ent[i].y) as i16;
        {
            let e = &mut self.g.ent[i];
            // sub_42000 (:52576): the band settle.
            if e.z > ground.saturating_add(v10) {
                e.z = e.z.saturating_add(v14);
            } else if e.z > ground.saturating_add(v12) {
                e.z = e.z.saturating_add((v14 as i32 * 25 / 100) as i16);
            }
            if e.z < ground.saturating_add(v12) {
                e.z = ground.saturating_add(v12);
            }
        }
        // Forward (always level) + lateral dodge, then commit.
        let (yaw, speed, jink) = {
            let e = &self.g.ent[i];
            (e.f30, e.f126, self.rivals[ri].jink)
        };
        let mut pos = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        Gen::polar_step(&mut pos, yaw, 0, speed);
        if jink != 0 {
            Gen::polar_step(&mut pos, yaw.wrapping_add(0x200) & 0x7FF, 0, jink);
            self.rivals[ri].jink -= 4 * jink.signum();
            if std::env::var_os("MGC_JINK_TRACE").is_some() {
                eprintln!(
                    "[jink] t={} ri={ri} mover decays {} -> {}",
                    crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                    jink,
                    self.rivals[ri].jink
                );
            }
        }
        self.g.move_relink(i, pos.0, pos.1, pos.2);
        // Accel 16/tick toward the desired speed (:18828-31).
        {
            let vdes = self.rivals[ri].vdes;
            let e = &mut self.g.ent[i];
            e.f126 += 16 * (vdes - e.f126).signum();
            // Turn toward the desired heading (:18835-57): rate =
            // err / (8 + (255-tempo)/16), clamped to the row's caps,
            // applied FULL then snapped to +34 only when the raw u16
            // compare says the step crossed it — retail keeps an
            // overshoot that wrapped through zero (no snap there),
            // where a min(err) step would land exactly.
            let err = Gen::angdist(e.f30, e.f34 & 0x7FF) as i32;
            let div = 8 + ((255 - self.rivals[ri].tempo as i32) / 16);
            let step = (err / div).clamp(v4 as i32, v2 as i32) as i16;
            let old = e.f30;
            let des = e.f34;
            let new = (old as i32 + (Gen::turn_sign(old, des) * step) as i32) as u16 & 0x7FF;
            e.f30 = new;
            if (old < des && new > des) || (old > des && des > new) {
                e.f30 = des;
            }
        }
    }

    /// `sub_45C10` (:55310-19) run for a RIVAL wizard — the same
    /// derive-every-tick law as [`World::mc1_owned_rebuild`]: memset
    /// owned[], then re-register each live acquisition entry under
    /// the MODEL byte of the pool record it points at. `known` is a
    /// port-only shadow with no retail twin (retail's "known" IS
    /// `+676 != 0`), so it advances here too — sticky, like the lane
    /// it stands in for between the scatter and the respawn regrant.
    fn rival_owned_rebuild(&mut self, ri: usize) {
        self.rivals[ri].owned = [0; SPELL_COUNT];
        for k in 0..SPELL_COUNT {
            let e = self.rivals[ri].acq[k];
            if e <= 0 {
                continue;
            }
            if let Some(r) = self.g.ent.get(e as usize) {
                // `*(char *)(record + 65)` (:55317) — the model byte is
                // read SIGNED, and the `+676 + 2*m` write is UNBOUNDED.
                // See [`owned_rebuild_overflow`].
                let m = r.model65 as i8 as i32;
                if (0..SPELL_COUNT as i32).contains(&m) {
                    let m = m as usize;
                    self.rivals[ri].owned[m] = e as u16;
                    self.rivals[ri].known[m] = true;
                } else if owned_rebuild_overflow() {
                    // Out of the book: land the word wherever
                    // `+676 + 2*m` actually points inside Type_160.
                    // The port models the three adjacent u16[24]
                    // tables — learn `+628`, owned `+676`, cooldown
                    // `+724`; a byte offset outside 628..772 is not
                    // modelled and is dropped (no witness yet).
                    let off = 676i32 + 2 * m;
                    let v = e as u16;
                    match off {
                        628..=674 => self.rivals[ri].learn[((off - 628) / 2) as usize] = v,
                        724..=770 => self.rivals[ri].cooldown[((off - 724) / 2) as usize] = v,
                        _ => {}
                    }
                }
            }
        }
    }

    /// Spell learning, the COUNTDOWN half (sub_15EC0 :19381-443): a
    /// live timer decrements 1/tick and the expiry conjures the
    /// rival's own manifestation. Arming lives on the JAR side
    /// ([`World::rival_learn_arm`], the pickup poll).
    ///
    /// The countdown's CLOCK is the tick-top roster's model-0 entry:
    /// :19394-99 walks bucket[0] and runs the dec pass once per
    /// model-0 carpet, so with the human dead (out of the roster) the
    /// timers freeze. The per-slot gate is `+676 == 0` alone
    /// (:19407) — no allowed test, no known test.
    fn rival_learn_tick(&mut self, ri: usize) {
        // The clock is the TICK-TOP roster's model-0 entry, so a
        // mid-tick death still clocks this tick (`human_bucket_alive`,
        // the :52254 membership sample).
        // …and :19394-99's walk starts from the roster HEAD, so a
        // seizure blank / a sever below the human's seat earlier this
        // tick parks the clock too (`Gen::mc1_human_on_wiz_chain`).
        if !self.human_bucket_alive || !self.g.mc1_human_on_wiz_chain() {
            return;
        }
        for s in 0..SPELL_COUNT {
            if self.rivals[ri].owned[s] != 0 {
                continue;
            }
            if self.rivals[ri].learn[s] > 1 {
                self.rivals[ri].learn[s] -= 1;
                continue;
            }
            if self.rivals[ri].learn[s] == 1 {
                // Conjure the copy (off_987DE[s] :19415-31). The mint
                // writes the ACQUISITION LIST only — owned[] (and the
                // port's known shadow) catch up at the next
                // housekeeping's rebuild (:17969), one tick later.
                let ent = self.rivals[ri].ent;
                self.rivals[ri].learn[s] = 0;
                if let Some(m) = self.mint_manifestation(s, ent) {
                    // ⭐⭐ THE FULL LIST ORPHANS THE TOKEN. The ctor
                    // runs FIRST — the pool slot is taken, the RNG
                    // spent, the record built — and only THEN does
                    // retail walk the rival's own `wizext+532` for the
                    // first empty seat (:19421-25):
                    //
                    //     v6 = 0;
                    //     while ( *(_DWORD *)(v5 + 532) )
                    //     { ++v6; v5 += 4; if ( v6 >= 24 ) goto LABEL_12; }
                    //     *(_BYTE *)(v4 + 16) |= 1u;              // :19428
                    //     *(_WORD *)(v4 + 42) = (a1 - v7) / 164;  // :19429
                    //     *(_DWORD *)(... + 4 * v6 + 532) = ...;  // :19430
                    //
                    // `LABEL_12` is the loop's CONTINUE, past all
                    // THREE writes — so with 24/24 seats taken the
                    // newborn keeps `flags 4` (no owned bit), `+42 = 0`
                    // (no owner) and never enters the book: a
                    // permanently orphaned `3*spell` record that
                    // nothing ever polls (`f70 % 3 == 0` is the
                    // spell's own manifest row in `str_2563D8`
                    // :4957-5028, never `sub_56250`/`sub_56260` →
                    // `sub_55A40`) and nothing ever frees (`actLife 0`).
                    // This is the SAME `v24 == -1` refusal as the human
                    // jar poll's ([`crate::engine::features::no_mc1_acq_list_full`])
                    // on a THIRD call path — the rival's learn timer —
                    // where retail lets the record be born and only
                    // withholds the registration.
                    //
                    // WITNESS mc1l49, wizard at carpet slot 594 with a
                    // full list: t=24927 pool slot 230 and t=25321 pool
                    // slot 123, both `(12,16)` `+70 = 48`, `+63 = the
                    // slot number` (the allocator's stamp, :43882),
                    // `actLife 0`. Retail: `flags 4`, `+42 0`. The port
                    // registered unconditionally and shipped `flags 5`,
                    // `+42 594`. Retail then declines all 9,942 / 9,843
                    // of each record's remaining polls, and neither
                    // record ever moves again — over 40,240 ticks only
                    // its `+20` list link and its `+63` clock change.
                    //
                    // `MGC_NO_MC1_RIVAL_LEARN_SEAT=1` restores the
                    // unconditional registration.
                    if crate::engine::features::no_mc1_rival_learn_seat()
                        || self.rivals[ri].acq.contains(&0)
                    {
                        self.rivals[ri].acq_push(m as u16);
                    } else {
                        let e = &mut self.g.ent[m];
                        e.flags &= !1;
                        e.f144 = 0;
                        self.entities_dirty = true;
                    }
                }
            }
        }
    }

    /// Spell learning, the ARM half (:64806-15) — runs inside the
    /// jar's pickup poll, ONLY on a tick the living human carpet
    /// AABB-hits the jar (sub_55A40's roster walk returns without
    /// reaching the arm unless the hit breaks it): every AI wizard
    /// (model 1) on the tick-top roster that neither owns nor is
    /// already learning the jar's spell, and whose book allows it
    /// (+796), arms the 200-tick countdown. The old port fold — "a
    /// matching jar exists anywhere" scanned from the rival's side
    /// every tick — armed off jars retail never dispatched: mc1hwl0's
    /// out-of-reach spell-6 jar armed Vodor at t=1 and the expiry
    /// minted a (12,6) retail never saw.
    pub(crate) fn rival_learn_arm(&mut self, spell: usize) {
        for c in 0..self.g.wiz_chain.visible_len() {
            let j = self.g.wiz_chain.list[c] as usize;
            if self.g.ent[j].model65 != 1 {
                continue;
            }
            let Some(ri) = (0..self.rivals.len())
                .find(|&r| self.rivals[r].ent == j as u16 && !self.rivals[r].eliminated)
            else {
                continue;
            };
            let r = &mut self.rivals[ri];
            if r.owned[spell] == 0 && r.learn[spell] == 0 && r.allowed[spell] {
                r.learn[spell] = 200;
            }
        }
    }

    /// Resolve a rival's own manifestation slot for `spell`, rejecting
    /// a STALE binding. `owned[]` is minted by the port
    /// ([`World::mint_manifestation`], `tick70 = MANIFEST_BASE +
    /// spell`, `f144` = the owner), but a conformance import replaces
    /// the whole pool from the recording WITHOUT rebinding it (the
    /// retail token's owner lives in its `+42`, which the port's `Ent`
    /// does not carry, so the importer cannot re-anchor the book) —
    /// the slot then holds a different entity entirely. Running the
    /// burst lanes on it would decrement a stranger's `f26` and
    /// publish buff bits from noise. Imported class-12 tokens keep
    /// RETAIL's encoding (`tick70 = spell*3 + phase`, always <
    /// [`MANIFEST_BASE`]), so the state test is exact and total.
    fn rival_token(&self, ri: usize, spell: usize) -> Option<usize> {
        let m = self.rivals[ri].owned[spell] as usize;
        let e = self.g.ent.get(m)?;
        // Both encodings are owned TOKENS: the native MANIFEST_BASE +
        // spell, and retail's phase-0 `spell*3` (a conformance import
        // — `owned` comes from the record's +676 there, and the
        // importer stamps +42 into f144, so the binding is anchored).
        let tick_ok =
            e.tick70 >= crate::engine::world::MANIFEST_BASE || e.tick70 as usize == spell * 3;
        (m != 0
            && e.class64 == 12
            && e.model65 as usize == spell
            && tick_ok
            && e.f144 == self.rivals[ri].ent
            && e.flags & 0x400 == 0)
            .then_some(m)
    }

    /// The rival-owned LAUNCHER token's burst machine (retail's
    /// sub_56090 :65100 head → sub_55DD0 gate → the per-spell bolt
    /// spawners), run at the TOKEN's own pool slot from
    /// `class12_tick` — both encodings. The commit
    /// ([`World::rival_cast`]) only arms +48 = +50; here the FULL
    /// tick fires the emission from the owner's settled pose (+30/+32
    /// — the commit stamped the pitch) and lands the sub_55E80 debit
    /// on the regen delta, a MID-burst tick pins a positive delta to
    /// 0 (the pool freezes for the whole burst), and the counter
    /// decrements LAST (:65260). A refused full tick (dead wizard /
    /// short pool) drops the burst to 1 so the shared decrement zeroes
    /// it (:64926-31) — silent for the AI, no buzz.
    pub(crate) fn rival_manifestation_tick(&mut self, m: usize, ri: usize, spell: usize) {
        if !matches!(spell, 0 | 3 | 7 | 8 | 11 | 13 | 15 | 17 | 20 | 23) {
            return;
        }
        let f26 = self.g.ent[m].f26;
        if f26 <= 0 {
            return;
        }
        let def = &self.spells()[spell];
        let count = def.count as i16;
        let cost = def.possess_mana;
        let i = self.rivals[ri].ent as usize;
        // The OWNER-DEAD refusal runs on EVERY burst tick, not just the
        // full one: the token handler calls sub_55DD0_56300 before the
        // full/mid split (:65030-88), and that gate refuses on
        // `owner.+140 < 0 || owner.+12 < 0` (:64915-24) — no state
        // test, and the mid-burst path short-circuits before the mana
        // compare. Any refusal sets +48 = 1 and falls into the
        // unconditional decrement (:64926-31, :65046), so a wizard
        // dying mid-burst cancels the rest of it in one double drop —
        // mc1l2 token 301 goes 2 → 0 on the tick rival 300's act_life
        // crosses to −280.
        // ⭐ AND THE GATE IS THE WHOLE `sub_55DD0`, NOT ITS LIFE LEG:
        // the owner's life, the token's live CASTLE REQUIREMENT and —
        // on a FULL tick only — the purse. The commit no longer
        // pre-screens the castle ladder ([`World::rival_cast`]), so
        // this is where a castle-gated burst dies: 26 → 1 → 0 in one
        // token tick (mc1hwl0 t=2426-27, token 256).
        if !self.rival_token_gate(ri, spell, f26 == count) {
            self.g.ent[m].f26 = 1;
            if self.g.ent[m].f26 > 0 {
                self.g.ent[m].f26 -= 1;
            }
            return;
        }
        if f26 == count {
            let alive = self.g.ent[i].tick70 == 1;
            if alive && self.rivals[ri].mana >= cost {
                let (ex, ey, ez, yaw, pitch, mz) = {
                    let e = &self.g.ent[i];
                    (e.x, e.y, e.z, e.f30, e.f32, e.z.wrapping_add(e.f78 as i16))
                };
                let _ = ez;
                self.rival_emit(ri, i, spell, ex, ey, mz, yaw, pitch);
                // sub_55E80's full arm (:64942-52) — LIVE in retail
                // (the remc1 `//fix` comment-out is the maintainer's).
                let r = &mut self.rivals[ri];
                let c = cost.min(i32::MAX as u32) as i32;
                r.mana_delta = if r.mana_delta >= 0 {
                    -c
                } else {
                    r.mana_delta - c
                };
            } else {
                self.g.ent[m].f26 = 1;
            }
        } else {
            // Mid-burst regen pin (sub_55E80's else arm :64956).
            if self.rivals[ri].mana_delta > 0 {
                self.rivals[ri].mana_delta = 0;
            }
        }
        if self.g.ent[m].f26 > 0 {
            self.g.ent[m].f26 -= 1;
        }
    }

    /// THE REBOUND TOKEN'S OWN MACHINE — retail's `sub_573F0_57920`
    /// (remc1 :65774 / remc1hw :61996, class-12 state 0x2A), run at
    /// the token's pool slot from `class12_tick` in both encodings;
    /// the human's twin is `manifestation_tick`'s spell-14 arm. The
    /// bare skeleton: while the `+48` burst is live, the WHOLE
    /// `sub_55DD0` gate runs every tick (owner alive, Rebound's 8000
    /// castle store, FULL tick adds the purse); a pass sets the
    /// OWNER's +17 bit 7 — our 0x8000, the deflection bit
    /// `proj_move_and_hit` reads — runs `sub_55E80` (full → the debit
    /// on the regen delta, mid-burst → the positive-delta pin) and
    /// takes the shared decrement; a refusal drops the burst to 1 for
    /// that same decrement, and the bit it leaves standing falls to
    /// the NEXT pass's `+48 <= 0` clear arm — the gate-fail path
    /// touches no flags.
    ///
    /// mc1hwl0 t=5592: rival 1's defense cast arms token 479 to 101
    /// at the carpet's slot 473, and retail's SAME-TICK token pass
    /// (479 walks after 473) gates, publishes carpet `flags`
    /// 12 → 32780 and decrements to 100. The port used to drive this
    /// from `rival_refresh_buffs` at the RIVAL's own tick — which
    /// runs before the defense cast — so the bit lagged one tick and
    /// the counter sat one above retail for the burst's whole life.
    pub(crate) fn rival_rebound_token_tick(&mut self, m: usize, ri: usize) {
        let i = self.rivals[ri].ent as usize;
        if i == 0 {
            return;
        }
        if self.g.ent[m].f26 <= 0 {
            self.g.ent[i].flags &= !0x8000;
            return;
        }
        let def = &self.spells()[14];
        let full = self.g.ent[m].f26 == def.count as i16;
        let cost = def.possess_mana;
        if self.rival_token_gate(ri, 14, full) {
            self.g.ent[i].flags |= 0x8000;
            // sub_55E80 from the token (:64942-56).
            let r = &mut self.rivals[ri];
            if full {
                let c = cost.min(i32::MAX as u32) as i32;
                r.mana_delta = if r.mana_delta >= 0 {
                    -c
                } else {
                    r.mana_delta - c
                };
            } else if r.mana_delta > 0 {
                r.mana_delta = 0;
            }
        } else {
            self.g.ent[m].f26 = 1;
        }
        if self.g.ent[m].f26 > 0 {
            self.g.ent[m].f26 -= 1;
        }
    }

    /// THE INVISIBLE TOKEN'S OWN MACHINE — retail's `sub_571B0_576E0`
    /// (hw :61899; class-12 state 36), the `sub_573F0` skeleton with
    /// the owner's +16 0x20 cloak bit: the FULL tick sets the bit and
    /// zeroes the owner's spawn grace (`wizext+331 = 0`); a mid-burst
    /// tick whose bit was externally BROKEN dies (`+48 = 1`); the
    /// `sub_55DD0` gate refusal dies the same way; `sub_55E80`
    /// debit/pin on admitted ticks; the shared decrement CLEARS the
    /// bit the moment it lands 0 — fizzle and expiry alike.
    ///
    /// mc1hwl0 t=20697: rival 1's flee re-decision commits Invisible
    /// (cooldown[12] arms, `+48 = +50` = 251) and the token's own
    /// pass at slot 481 — above carpet 473, SAME tick — gate-refuses
    /// on the castle-store ladder, so retail's burst dies 0 → 251 →
    /// 1 → 0 INSIDE the tick and no boundary ever shows it (the
    /// Wall-of-Fire spike shape, session 47). The inert port token
    /// kept 251, and the refresh mirror cloaked a wizard retail
    /// never hid — the t=20698 `(3,1)slot473:flags` head.
    pub(crate) fn rival_invis_token_tick(&mut self, m: usize, ri: usize) {
        let i = self.rivals[ri].ent as usize;
        if i == 0 || self.g.ent[m].f26 <= 0 {
            return;
        }
        let def = &self.spells()[12];
        let full = self.g.ent[m].f26 == def.count as i16;
        let cost = def.possess_mana;
        if self.rival_token_gate(ri, 12, full) {
            if full {
                self.rivals[ri].grace = 0; // :61913 `+331 = 0`
                self.g.ent[i].flags |= 0x20;
            } else if self.g.ent[i].flags & 0x20 == 0 {
                // The cloak was broken outside the machine — the
                // burst dies (:61917-19).
                self.g.ent[m].f26 = 1;
            }
            // sub_55E80 from the token (:64942-56).
            let r = &mut self.rivals[ri];
            if full {
                let c = cost.min(i32::MAX as u32) as i32;
                r.mana_delta = if r.mana_delta >= 0 {
                    -c
                } else {
                    r.mana_delta - c
                };
            } else if r.mana_delta > 0 {
                r.mana_delta = 0;
            }
        } else {
            self.g.ent[m].f26 = 1;
        }
        if self.g.ent[m].f26 > 0 {
            self.g.ent[m].f26 -= 1;
            if self.g.ent[m].f26 == 0 {
                self.g.ent[i].flags &= !0x20; // :61926-28
            }
        }
    }

    /// THE SHIELD TOKEN'S OWN MACHINE — retail's `sub_566C0` (:65266,
    /// class-12 state 12), the rival twin of
    /// [`World::mc1_shield_token_tick`]: the same `sub_573F0` skeleton
    /// as Rebound's, but the owner bit is +17 0x40 (our 0x4000) and
    /// there is NO clear arm — the bit is SET-only here and cleared
    /// PER-ABSORB by the damage intake (:55700-07,
    /// [`Self::rival_mail_block`]'s quarter), so an expired shield
    /// still quarters exactly one more hit.
    ///
    /// mc1hwl0 t=5593: rival 1's defense ladder falls through to
    /// Shield (Rebound live from 5592), the commit debits −1000 and
    /// retail's same-tick token pass publishes carpet 473's `flags`
    /// 0x800C → 0xC00C. The refresh-driven port paid but never
    /// published, and no rival shield ever quartered a hit.
    pub(crate) fn rival_shield_token_tick(&mut self, m: usize, ri: usize) {
        let i = self.rivals[ri].ent as usize;
        if i == 0 || self.g.ent[m].f26 <= 0 {
            return;
        }
        let def = &self.spells()[4];
        let full = self.g.ent[m].f26 == def.count as i16;
        let cost = def.possess_mana;
        if self.rival_token_gate(ri, 4, full) {
            self.g.ent[i].flags |= 0x4000;
            // sub_55E80 from the token (:64942-56).
            let r = &mut self.rivals[ri];
            if full {
                let c = cost.min(i32::MAX as u32) as i32;
                r.mana_delta = if r.mana_delta >= 0 {
                    -c
                } else {
                    r.mana_delta - c
                };
            } else if r.mana_delta > 0 {
                r.mana_delta = 0;
            }
        } else {
            self.g.ent[m].f26 = 1;
        }
        if self.g.ent[m].f26 > 0 {
            self.g.ent[m].f26 -= 1;
        }
    }

    /// THE HEAL TOKEN'S OWN MACHINE — retail's `sub_56270_567A0`
    /// (:65091 / hw :61313, byte-identical; class-12 state 3), the
    /// rival twin of [`World::mc1_heal_token_tick`]. A body of its
    /// OWN, not the launcher skeleton: admission is the shared
    /// `sub_55DD0` gate AND `actLife < maxLife` AND the purse covers
    /// the token's live `+136` cost — all three on EVERY tick
    /// (:65102-05). An admitted tick restores 5% of the life CEILING,
    /// capped (:65108-12), plays the cast sound on the FULL tick only
    /// (:65106-07, area channel at the owner's slot), and debits the
    /// WHOLE cost on the owner's regen delta — heal never calls
    /// `sub_55E80`, so there is no mid-burst positive-delta pin and
    /// the debit stacks every admitted tick (:65113-19; remc1
    /// comments the positive-arm write out behind the same `//fix`
    /// marker as sub_55E80's, and the corpus says it is live: retail
    /// f132 reads −1000 on the heal tick). A refusal RELEASES the
    /// burst (`+48 = 1`, then the shared decrement zeroes it,
    /// :65122-24) — exactly one decrement per tick either way.
    pub(crate) fn rival_heal_token_tick(&mut self, m: usize, ri: usize) {
        if self.g.ent[m].f26 <= 0 {
            return;
        }
        let i = self.rivals[ri].ent as usize;
        if i == 0 {
            // An unowned token still takes the shared decrement.
            self.g.ent[m].f26 -= 1;
            return;
        }
        let def = &self.spells()[1];
        let full = self.g.ent[m].f26 == def.count as i16;
        // The live per-manifestation cost (+136), not the table — the
        // same field the human body reads.
        let cost = {
            let e = &self.g.ent[m];
            if e.f136 > 0 {
                e.f136 as u32
            } else {
                def.possess_mana
            }
        };
        let (life, max) = {
            let e = &self.g.ent[i];
            (e.act_life, e.max_life as i32)
        };
        // `sub_56270`'s own extra afford leg is `v1[35] >= *(a1+136)` —
        // the RECORD's purse word again, not the mirror.
        let affords = if live_purse_legacy() {
            self.rivals[ri].mana >= cost
        } else {
            self.g.ent[i].f140 >= cost.min(i32::MAX as u32) as i32
        };
        if self.rival_token_gate(ri, 1, full) && life < max && affords {
            if full {
                self.g.snd(25, i); // :65106 — the burst's first tick only
            }
            self.g.ent[i].act_life = (life + max / 20).min(max);
            let r = &mut self.rivals[ri];
            let c = cost.min(i32::MAX as u32) as i32;
            r.mana_delta = if r.mana_delta >= 0 {
                -c
            } else {
                r.mana_delta - c
            };
        } else {
            self.g.ent[m].f26 = 1;
        }
        self.g.ent[m].f26 -= 1;
    }

    /// THE CASTLE TOKEN'S OWN MACHINE — retail's `sub_57610_57B40`
    /// (:65862-923, class-12 state 48), shared by human and rival
    /// owners alike. Unlike the generic launcher it has NO per-tick
    /// decrement: the commit arms `+48 = +50` (101), the FULL tick
    /// alone fires — sub_55E80 debit, the (9,10) castle ball minted at
    /// the OWNER's own axis — and `+48` then parks at `+50 − 1`, the
    /// IN-TRANSIT CHARGE PIN the ball's delivery or failure releases
    /// (`sub_46D20` → [`Gen::release_castle_charge_pin`]). A refused
    /// `sub_55DD0` gate zeroes the counter outright (:65920); a failed
    /// allocation leaves it FULL, so the mint retries next tick (the
    /// child-allocation guard family). The ball rides the owner's
    /// speed (+126 +=), banks the wizard's accumulated charge meter
    /// (+26 = wizext+326, zeroed), and splits on the ESTABLISHED
    /// castle: standing → homing upgrade ball (+146 = castle, explode
    /// child (10,43)); none → the 4096-ahead build lob whose child is
    /// the (3,2) castle itself. `sub_55EF0`'s hand-muzzle sidestep is
    /// gated on the owner's 0x100/0x200 fire bits — the commit clears
    /// 0x100 (:19110) and no rival path sets 0x200, so a rival's ball
    /// launches from the hull (no-op here).
    ///
    /// mc1l5 t=5152: Vodor upgrades his castle — charge 200 → 0 into
    /// ball 790's +26, cooldown[16] = 40, the ball arrives the same
    /// tick and morphs into the (10,43) at slot 737. The port's old
    /// ch5-mail shortcut (DEVIATIONS.md "rival_cast_castle (upgrade
    /// token)", now retired) skipped the whole ride, which the corpus
    /// proves is NOT cosmetic: two graded entity rows per upgrade.
    pub(crate) fn rival_castle_token_tick(&mut self, m: usize, ri: usize) {
        if self.g.ent[m].f26 <= 0 {
            return;
        }
        let i = self.rivals[ri].ent as usize;
        if i == 0 || i >= self.g.ent.len() {
            return; // owner gone: the token stalls (:65873-74)
        }
        let count = self.spells()[16].count as i16;
        let full = self.g.ent[m].f26 == count;
        let price = self.rival_castle_price(ri);
        // sub_55DD0 (:64915-24): owner-dead refuses every tick, the
        // cost compare runs on the FULL tick only.
        // The inlined twin of `rival_token_gate` — the SAME `sub_55DD0`
        // predicate, so the same live-purse operand and the same
        // negative-purse leg (round 104: a law on ONE call path is not
        // landed).
        let purse_short = if live_purse_legacy() {
            full && self.rivals[ri].mana < price
        } else {
            self.g.ent[i].f140 < 0
                || (full && self.g.ent[i].f140 < price.min(i32::MAX as u32) as i32)
        };
        if self.g.ent[i].act_life < 0 || purse_short {
            self.g.ent[m].f26 = 0;
            return;
        }
        if !full {
            return; // in transit — the pin holds
        }
        let (ex, ey, ez, yaw, pitch, ospeed, lift, otag) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.f30, e.f32, e.f126, e.f84 as i16, e.id24)
        };
        // ⭐⭐⭐ THE DEBIT IS BILLED BEFORE THE BALL EXISTS.
        // `sub_57610_57B40` (:65880-83, twin remc1hw :62103-06,
        // byte-identical) runs
        //   `sub_55E80_563B0(a1, v2);`             // the debit
        //   `v3 = sub_373F0_377B0(v2+72, 9, 10);`  // the ball
        //   `if (v3) { a1+48 = a1+50 - 1; … }`
        // — the null guard covers ONLY the ball and the in-transit
        // pin. On an exhausted pool retail STILL CHARGES the full
        // ladder price, spawns nothing, and leaves `+48` at the full
        // count; the NEXT tick's `sub_55DD0` afford leg then fails on
        // the emptied purse and its else arm releases the burst
        // (`+48 = 0`). The port hoisted the allocation guard above the
        // debit, so a dry-pool rival built no castle AND paid nothing.
        // Same shape as the volcano's plume handover (L14): the
        // handover lives inside the spawn's guard, the BILLING does
        // not.
        //
        // **mc1l49 t=15552 is exact, read from `state.struct_b64`.**
        // Wiz 7 (ent 750) holds a level-3 castle (slot 940), so his
        // (12,16) token slot 112 carries `+136 = 40000 = CASTLE_CAP[3]`
        // and `+140 = 396 = 40000/101`. The commit arms `+48 = 101` at
        // t=15551 with the purse at 40100 (it had been climbing
        // +1000/tick since t=15522). At t=15552 the free stack is 0:
        // retail's purse drops 40100 → **100** — a clean 40,000 debit —
        // `+48` stays **101**, and NO (9,10) castle ball ever appears
        // in the take. At t=15553 the gate refuses (100 < 40000) and
        // `+48` goes 101 → 0. The port free-ran to 41100 (40100 plus
        // its own +1000 regen step) with no debit at all.
        // `MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER=1` restores the old
        // spawn-then-bill order.
        let debit = |w: &mut Self| {
            let r = &mut w.rivals[ri];
            let c = price.min(i32::MAX as u32) as i32;
            r.mana_delta = if r.mana_delta >= 0 {
                -c
            } else {
                r.mana_delta - c
            };
        };
        let billed_first = castle_ball_debit_order();
        if billed_first {
            debit(self);
        }
        let Some(b) = self.g.spawn_castle_ball(ex, ey, ez) else {
            return; // allocation guard: stays FULL, re-fires next tick
        };
        // sub_55E80's full arm — the debit on the regen delta.
        if !billed_first {
            debit(self);
        }
        let (tok_f44, tok_f140) = {
            let t = &self.g.ent[m];
            (t.f44, t.f140)
        };
        {
            let e = &mut self.g.ent[b];
            e.f126 += ospeed; // *(v3+126) += *(v2+126)
            e.f44 = tok_f44; // *(v3+44) = *(a1+44)
            e.id24 = otag; // *(v3+24) = *(v2+24)
            e.z = e.z.wrapping_add(lift); // *(v3+76) += *(v2+84)
            e.f140 = tok_f140; // *(v3+140) = *(a1+140)
            e.f30 = yaw;
            e.f32 = pitch;
        }
        // The wizext+50 split (:65893-908): the REGISTER, index test
        // only (CARPET.EXE 0x6FEE0-0x6FF16) — a planted level-0
        // castle is already bound. See [`rival_castle_token_register`].
        let castle = if rival_castle_token_register() {
            let reg = self.wiz_castle_reg(self.rivals[ri].slot) as usize;
            (reg != 0 && reg < self.g.ent.len()).then_some(reg)
        } else {
            self.rival_castle(self.rivals[ri].ent)
                .filter(|&c| self.g.ent[c].f26 > 0)
        };
        if let Some(c) = castle {
            let e = &mut self.g.ent[b];
            e.f68 = 10;
            e.f69 = 43;
            e.f146 = c as u16;
        } else {
            let mut t = (ex, ey, 0i16);
            Gen::polar_step(&mut t, yaw, 0, 4096);
            let e = &mut self.g.ent[b];
            e.f68 = 3;
            e.f69 = 2;
            e.dest_x = t.0;
            e.dest_y = t.1;
            // …and `+154 = sub_11F50(+150)` (:65899-903): the ground
            // under the projected site, the human arm's twin.
            if !crate::engine::world::no_mc1_bolt_dest_stamp() {
                self.g.ent[b].site_z = self.g.ground_z(t.0, t.1) as i16;
            }
        }
        // The charge move (:65910-11): the ball banks the owner's
        // accumulated meter and zeroes it.
        let ws = self.rivals[ri].slot as usize;
        self.g.ent[b].f26 = self.wiz_charge[ws] as i16;
        self.wiz_charge[ws] = 0;
        self.g.snd(15, b); // :65918
        self.g.ent[m].f26 = count - 1; // the in-transit pin
        self.entities_dirty = true;
    }

    /// ⭐ THE RIVAL'S SPEED TOKEN, at its OWN pool slot — retail's
    /// `sub_56380_568B0` (:65131-99, spell 2) and its backwards twin
    /// `sub_57F00_58410` (:66172-231, spell 21), which are the SAME
    /// function with every speed term negated. The port used to run
    /// only the contrail leg of this (in `class12_tick`) and decrement
    /// the counter over in `rival_refresh_buffs`; everything else the
    /// handler does was missing, which is what mc1l4 breaks on at
    /// t=2 and mc1l5 at t=2:
    ///
    /// - **the v_14 KILL** (:65146-51): the owner's speed-column
    ///   latch standing means the brain has retaken v_12, so the burst
    ///   force-ends — `+48 = 1`, and the shared decrement below zeroes
    ///   it the same tick. mc1l5 t=185: Vodor arrives, +48 drops
    ///   63 → 0 in one step. A REFUSED `sub_55DD0` gate skips the
    ///   sustain arm but does NOT force the end.
    /// - **the spell-ACTIVE bit** `+16 bit 7` — set on the full tick
    ///   (:65154-57), released two ticks in (:65160-65) and again at
    ///   expiry (:65196). mc1l4 t=2 measures it: token 368's flags go
    ///   `5 → 133` (0x85).
    /// - **the SPEED OVERRIDE, SNAPPED into both columns**: `v_12 =
    ///   3·f128` on the full tick, `2·f128` mid-burst, `f126 = v_12`
    ///   (:65167-78) — not the AI's 16/tick ease. mc1l4 t=2/t=3
    ///   measures f126 `0 → 240 → 160` against f128 = 80.
    /// - **`sub_55E80`** (:65188): the full tick stamps the debit on
    ///   the regen delta, every mid-burst tick PINS a positive delta
    ///   to 0 — an active spell blocks mana regeneration. mc1l4 t=2:
    ///   f132 `100 → −1000` (the cost), then `−1000 → 0` at t=3 once
    ///   the wizard pass has spent it. mc1l5's Vodor sits under a
    ///   248-tick burst from tick 0, which is why his `+132` reads 0
    ///   forever and his purse never leaves zero.
    /// - **the EXPIRY SNAP** (:65192-97): the counter reaching 0
    ///   restores `v_12 = f126 = f128` (signed: −f128 backwards) and
    ///   drops the active bit.
    ///
    /// ⚠ The decrement and the expiry snap live INSIDE the
    /// owner-valid guard, so a token whose `+42` owner is gone stalls
    /// at its current count rather than winding down.
    pub(crate) fn rival_speed_token_tick(&mut self, m: usize, ri: usize, spell: usize) {
        if self.g.ent[m].f26 <= 0 {
            return; // :65141 — the whole handler is inside `+48 > 0`
        }
        let i = self.rivals[ri].ent as usize;
        if i == 0 || i >= self.g.ent.len() {
            return; // :65144 — no owner record, nothing runs
        }
        let count = self.spells()[spell].count as i16; // the token's +50
        let full = self.g.ent[m].f26 == count;
        // The backwards twin negates every speed term (:66207-27).
        let dir: i16 = if spell == 2 { 1 } else { -1 };
        if self.rivals[ri].v14 {
            self.g.ent[m].f26 = 1; // :65149-50 — the two-phase kill
        } else if self.rival_token_gate(ri, spell, full) {
            let mut armed = false;
            {
                let e = &mut self.g.ent[m];
                if full && e.flags & 0x80 == 0 {
                    e.flags |= 0x80; // :65157
                    armed = true;
                }
                if e.f26 == count - 2 {
                    e.flags &= !0x80; // :65160-65
                }
            }
            if armed {
                // :65158 — the arm chime, at the OWNER's pool slot and
                // `a2 = -1`. Case 19 (:64525) carries no local-player
                // arm, so the AI's Accelerate is audible exactly like
                // the human's (the same law as the id-17 hit grunt).
                self.g.snd(19, i);
            }
            let base = self.g.ent[i].f128;
            let v12 = if full { 3 } else { 2 } * base * dir;
            self.rivals[ri].vdes = v12;
            self.g.ent[i].f126 = v12;
            // The (10,2) contrail at the OWNER's axis every 4th token
            // tick (:65179-87) — id24 = the caster, act_life ×4.
            if self.g.ent[m].f63 & 3 == 0 {
                let (cx, cy, cz, own) = {
                    let e = &self.g.ent[i];
                    (e.x, e.y, e.z, e.id24)
                };
                if let Some(p) = self.g.spawn_effect(2, cx, cy, cz) {
                    self.g.ent[p].id24 = own;
                    self.g.ent[p].act_life *= 4;
                }
            }
            // sub_55E80 (:65188): the debit on the full tick, the
            // regen pin on every other.
            let cost = self.spells()[spell].possess_mana.min(i32::MAX as u32) as i32;
            let r = &mut self.rivals[ri];
            if full {
                r.mana_delta = if r.mana_delta >= 0 {
                    -cost
                } else {
                    r.mana_delta - cost
                };
            } else if r.mana_delta > 0 {
                r.mana_delta = 0;
            }
        }
        // The shared decrement and the expiry snap (:65190-97).
        self.g.ent[m].f26 -= 1;
        if self.g.ent[m].f26 == 0 {
            let base = self.g.ent[i].f128 * dir;
            self.rivals[ri].vdes = base;
            self.g.ent[i].f126 = base;
            self.g.ent[m].flags &= !0x80;
        }
    }

    /// `sub_55DD0_56300` (:64909-32) for a RIVAL's token — the gate
    /// every class-12 handler runs before its sustain arm. Reads the
    /// OWNER's purse and life, then the token's own live castle
    /// requirement (+132) against the ESTABLISHED castle's store, and
    /// finally admits a FULL tick only if the purse covers the cost
    /// (:64926) while a MID-burst tick admits unconditionally
    /// (:64928). The refusal buzz (:64931) is the local player's
    /// channel and stays unported for the AI.
    fn rival_token_gate(&self, ri: usize, spell: usize, full: bool) -> bool {
        let r = &self.rivals[ri];
        let i = r.ent as usize;
        // ⭐ THE PURSE IS THE OWNER RECORD'S OWN LIVE `+140`, NOT A
        // CACHED MIRROR. `sub_55DD0` refuses on a NEGATIVE purse
        // BEFORE it reaches the mid-burst escape (CARPET.EXE
        // `0x55DDA: 83 b9 8c 00 00 00 00` = `cmpl $0x0,0x8c(%ecx)`,
        // then `0f 8c 7a 00 00 00` = `jl 0x55E61`; the mid-burst arm
        // is at `0x55E4F`). The port had no such leg.
        if !live_purse_legacy() && self.g.ent[i].f140 < 0 {
            return false; // a2[35] — the purse wrapped negative
        }
        if self.g.ent[i].act_life < 0 {
            return false; // a2[3] — the owner is dying
        }
        let req = self.spells()[spell].castle_req;
        // :64922-23 — `!wizext+50 || req > pool[wizext+50].+140`: the
        // register, index test alone.
        if req != 0
            && !self
                .rival_castle_reg(ri)
                .is_some_and(|c| self.g.ent[c].f140.max(0) as u32 >= req)
        {
            return false;
        }
        // :64926 — `0x55E2F: 8b 81 8c 00 00 00` (`mov 0x8c(%ecx),%eax`,
        // the OWNER RECORD's live +140) compared SIGNED against the
        // TOKEN's live +136 (`0x55E35: 3b 82 88 00 00 00` / `7c 12`
        // = `jl`). `RivalState::mana` is a mirror re-synced only at
        // the owner's own dispatch, but combat debits `+140` DIRECTLY
        // and MID-TICK (`sub_52B30` :62884, `sub_46540` :55703), so a
        // token dispatched later in the same tick saw a purse retail
        // had already emptied. WITNESS mc1l49 t=19558 (rival 724).
        if live_purse_legacy() {
            return !full || r.mana >= self.spells()[spell].possess_mana;
        }
        !full || self.g.ent[i].f140 >= self.spells()[spell].possess_mana.min(i32::MAX as u32) as i32
    }

    /// Buff flags derive from the manifestations' burst counters
    /// (the human's manifestation_tick equivalents; the rival's
    /// bursts are armed by [`World::rival_cast`] and decremented
    /// here).
    fn rival_refresh_buffs(&mut self, ri: usize) {
        // Invisible (12) is NOT clocked here: retail's sub_571B0 is
        // the token's OWN class-12 body (state 36), run at the
        // token's pool slot ([`Self::rival_invis_token_tick`]) — it
        // owns the counter, the owner's 0x20 cloak bit, the grace
        // zero and the 55E80 debit/pin. The refresh-driven clock kept
        // a burst alive that retail's same-tick gate refusal killed
        // INSIDE the commit tick (mc1hwl0 t=20697), and its mirror
        // below cloaked the wizard off that phantom counter.
        let invisible = self
            .rival_token(ri, 12)
            .is_some_and(|m| self.g.ent[m].f26 > 0);
        // Shield (4) and Rebound (14) are NOT clocked here: their
        // tokens run retail's own machines at the token's pool slot
        // ([`Self::rival_shield_token_tick`] /
        // [`Self::rival_rebound_token_tick`], both encodings), which
        // own the counter, the owner's 0x4000/0x8000 bits and the
        // regen pin. The planner flags just mirror the live bursts.
        let shield = self
            .rival_token(ri, 4)
            .is_some_and(|m| self.g.ent[m].f26 > 0);
        let rebound = self
            .rival_token(ri, 14)
            .is_some_and(|m| self.g.ent[m].f26 > 0);
        // Heal (1) is NOT clocked here either: retail's sub_56270 is
        // the token's OWN class-12 body (state 3), run at the token's
        // pool slot ([`Self::rival_heal_token_tick`]) — it owns the
        // counter, the owner's +5%/tick restore and the full-cost
        // f132 debit. Driving it from this refresh healed one pass
        // late (the cast arms the token AFTER refresh has run) and
        // paid cost/count from the purse instead of the full cost
        // through the regen delta. mc1hwl0 t=17051: rival 1 eats a
        // quartered 25 through the shield, casts Heal at carpet slot
        // 473, and retail's SAME-TICK token pass (478 walks after
        // 473) restores 5% capped and parks the delta at −1000; the
        // refresh-driven port read 9995 on the graded life lane.
        // ⚠ The speed-up (2) burst is NOT decremented here. Retail
        // winds it down inside the token's OWN handler at the token's
        // pool slot ([`Self::rival_speed_token_tick`], :65190), which
        // is where the v_14 kill and the speed override live too —
        // clocking it from the wizard's slot ran the counter a full
        // pass early and skipped every other thing that handler does.
        {
            let r = &mut self.rivals[ri];
            r.shield = shield;
            r.invisible = invisible;
            r.rebound = rebound;
        }
        // (No cloak mirror here: the 0x20 bit is the invis token
        // machine's own — set on the FULL tick, cleared the moment
        // the counter lands 0, :61913/:61926-28.)
    }

    /// Incoming-projectile defense (sub_16800 :19769 + sub_16870/90):
    /// the nearest class-9 homing on me within 5120 → lateral jink 80 +
    /// a reactive cast (models {0,3,16} → 14 Rebound, {4,9} → 4
    /// Shield).
    ///
    /// ⭐ THE SCAN WALKS THE TICK-TOP CLASS-9 ROSTER
    /// (`var_u32_36462[3]`, :19777), not the pool — membership was
    /// sampled at the tick head with NO life or flags test, so a ball
    /// born mid-tick is not yet a threat (mc1l4 t=5377: the pelting
    /// stream's newborn must not trigger a dodge until next tick) and
    /// a soft-killed one still is. Both range gates are STRICT
    /// (`>= 0x1900000` rejects, the cast wants `< 0x100000`).
    fn rival_defense(&mut self, ri: usize, i: usize) {
        let me = self.rivals[ri].ent;
        let (px, py, pz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        // ⭐ THE RANGE IS 2D. Both this scan's 5120 gate and the
        // reactive-cast's 1024 gate measure through `sub_42410`
        // (:52748-54) = (Δx)² + (Δy)² — NO z term. The port's
        // invented dz² leg dropped a high bolt out of dodge range
        // one tick early (mc1hwl0 t=16771: threat 516 at dz 2617
        // reads 27.7M in 3D against the 26.2M gate, 20.9M in
        // retail's 2D — retail re-stamps the strafe, the port let
        // it decay, and the 4-unit lateral gap is the t=16772
        // x,y head).
        // The election is UNGATED and its key UNSIGNED (:19776-89):
        // the `>= 0x1900000` threshold tests the WINNER only, and the
        // `-1` seed answers "no candidate" through that same test.
        let mut best: Option<(usize, u32)> = None;
        for k in 0..self.g.proj_chain.visible_len() {
            let j = self.g.proj_chain.list[k] as usize;
            let e = &self.g.ent[j];
            if e.f146 != me {
                continue;
            }
            let d2 = Gen::dist2_sq(px, py, e.x, e.y) as u32;
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((j, d2));
            }
        }
        let best = best.filter(|&(_, d)| d < 0x190_0000);
        if std::env::var_os("MGC_JINK_TRACE").is_some() {
            let cand: Vec<(u16, u16, u16, u16, i16, i32)> = (0..self.g.proj_chain.visible_len())
                .map(|k| {
                    let j = self.g.proj_chain.list[k] as usize;
                    let e = &self.g.ent[j];
                    (
                        j as u16,
                        e.f146,
                        e.x,
                        e.y,
                        e.z,
                        Gen::dist2_sq(px, py, e.x, e.y),
                    )
                })
                .collect();
            eprintln!(
                "[jink] t={} ri={ri} me={me} scan best={best:?} cand={cand:?} me_pos=({px},{py},{pz}) jink_pre={}",
                crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                self.rivals[ri].jink
            );
        }
        let Some((threat, d3)) = best else { return };
        self.rivals[ri].jink = 80;
        if d3 < 1024 * 1024 {
            // Verbatim `sub_16890` (remc1 :19815-52 / remc1hw
            // :17947-84). Two corrections to the old port:
            //
            // (a) the model switch's DEFAULT arm casts nothing —
            //     models 1/2 fall out of the `< 4` branch and 5..8 out
            //     of the `>= 9` branch with no call, and `!= 16`
            //     returns outright. The port folded every unlisted
            //     model into Shield, burning the token (and 2000 mana)
            //     on threats retail ignores.
            // (b) the fire-spell arm is a LADDER, not a pick:
            //     `if (sub_15A00(a1,0xE)) sub_155F0(a1,0xE); else if
            //     (sub_15A00(a1,4)) sub_155F0(a1,4);` — with Rebound
            //     already live (its readiness gate), the rival falls
            //     through to Shield instead of standing there.
            let threat_model = self.g.ent[threat].model65;
            match threat_model {
                0 | 3 | 16 => {
                    if self.rival_cast_ready(ri, 14) {
                        self.rival_cast(ri, i, 14);
                    } else if self.rival_cast_ready(ri, 4) {
                        self.rival_cast(ri, i, 4);
                    }
                }
                4 | 9 if self.rival_cast_ready(ri, 4) => {
                    self.rival_cast(ri, i, 4);
                }
                _ => {}
            }
        }
    }

    // ---- the decision cascade (sub_136C0 :18048) --------------------------

    /// `sub_16000(target, 12)` — the spell-12 cloak notice, a LIVE
    /// read of the target's manifestation `+48` through the bare
    /// [`castle_token_index`] resolve (`sub_14E60` + `+48 > 0`,
    /// :19449-56). See [`invis_live_token`].
    fn mc1_invis_notice(&self, owned12: u16, cached: bool) -> bool {
        if !invis_live_token() {
            return cached;
        }
        let m = owned12 as usize;
        if m == 0 {
            return false;
        }
        if no_mc1_invis_word48() {
            return m < self.g.ent.len() && self.g.ent[m].f26 > 0;
        }
        // ⭐⭐⭐ THE SECOND CALL PATH OF THE SAME LAW. `sub_16000`
        // (VA 0x16000 = `CARPET.EXE` file 0x2E7F8) is
        // `call 0x2D658` (= `sub_14E60`) / `test %eax,%eax` /
        // `je -> 0` / `66 83 78 30 00  cmpw $0x0,0x30(%eax)` /
        // `7e jle -> 0` — a SIGNED `> 0` read of **`+48`** off the
        // same unguarded raw slot resolve the castle gate uses. The
        // port read [`Ent::f26`], which is retail's **`+26`** on
        // every record that is not a live class-12 manifestation, so
        // a stale `owned[12]` register naming a recycled slot made
        // the port's selector treat a wizard as CLOAKED off a
        // stranger's `+26` — measured on the corpus, the dominant
        // shape by far (mc1l49: `(0,13)` `+26` 116, `(10,13)` `+26`
        // 100..116, `(10,52)` `+26` 444..584, `(5,15)` `+26` 29, all
        // with `+48 == 0`).
        self.mc1_token_word48(m) > 0
    }

    /// The LIVE Create-Castle price — the manifestation's +136 cost
    /// cache, which retail's want/commit gates read (sub_15E90
    /// :19375: `manifest +136 <= wizard +136`). Ctor 1000; CAP[lvl]
    /// while a castle stands; re-stamped CAP[0] = 5000 by the
    /// teardown (sub_47A70 → sub_47C60 case 0) — the rival rebuild
    /// POVERTY GATE: a razed, mana-starved rival (census ceiling
    /// collapsed to the 1000 base) refuses to rebuild until claims
    /// push mana_max past 5000 (mc1l5 take: Vodor rebuilds at
    /// t=17643, the tick mana_max crosses 5000).
    fn rival_castle_price(&self, ri: usize) -> u32 {
        let m = self.rivals[ri].owned[16] as usize;
        if m != 0
            && m < self.g.ent.len()
            && self.g.ent[m].class64 == 12
            && self.g.ent[m].flags & 0x400 == 0
        {
            self.g.ent[m].f136.max(0) as u32
        } else {
            SPELLS[16].possess_mana
        }
    }

    fn rival_selector(&mut self, ri: usize, i: usize, think: bool) {
        let trace = std::env::var_os("MGC_RIVAL_TRACE").is_some();
        if trace {
            let t = crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed);
            let castle = self.rival_castle(self.rivals[ri].ent);
            eprintln!(
                "[rsel t={t}] ri={ri} state={:?} think={think} castle={castle:?} known16={} mana_max={} mana={} target={} f63={}",
                self.rivals[ri].state,
                self.rivals[ri].known[16],
                self.rivals[ri].mana_max,
                self.rivals[ri].mana,
                self.rivals[ri].target,
                self.g.ent[i].f63,
            );
        }
        // 1. Need a castle (sub_13F00 :18345). ⭐⭐⭐ ITS HEAD IS
        // THREE RAW REGISTER READS AND THE PORT HAD REPLACED ALL
        // THREE (`reference/remc1/sub_main.cpp:18359`):
        //
        //     if ( !wizext->var_50 && sub_14E60(a1, 0x10)
        //          && sub_15E90(a1, 0x10) )
        //
        // - `!wizext->var_50` is the ESTABLISHED-CASTLE REGISTER, the
        //   same one arm 3 already reads — not a pool scan.
        // - `sub_14E60(a1, 0x10)` (:18769-77) is a bare
        //   `pool + 164 * wizext->var_676[16]`, guarded only against
        //   slot 0: the test is "is my Create-Castle TOKEN REGISTER
        //   set", NOT "do I know spell 16". A rival who knows the
        //   spell but whose token was scattered and never re-minted
        //   does not build.
        // - `sub_15E90(a1, 0x10)` (:19376-79) is
        //   `pool[owned[16]]->+136 <= a1->+136` — the TOKEN's LIVE
        //   price cache, with NO class, flag or liveness guard and no
        //   spell-table fallback (the register is already proven
        //   nonzero by the term before it).
        //
        // WITNESS (mc1l49 t=37000, `state.struct_b64`): wiz 1 has
        // `+50 = 0` AND `owned[16] = 0`, so retail refuses at the
        // second term and stays in Possess (brain byte 6); the port
        // saw `known[16]` true, priced the build off
        // `SPELLS[16].possess_mana` and went to Build (3) on EVERY
        // pair tick of the window, which is what drove slot 594's
        // `target_yaw` torrent. `MGC_NO_MC1_CASTLE_ARM_REGISTERS=1`
        // restores the pre-dig gates.
        let castle = self.rival_castle(self.rivals[ri].ent);
        // ⭐ The two HOME arms (2 and 9) resolve the castle off the
        // wizext+50 REGISTER with an index test alone (`sub_14310`
        // :18486-90, `sub_14DC0` :18755-61) — a register that names a
        // re-minted slot still sends retail "home" to it. See
        // [`crate::engine::features::no_mc1_home_castle_register`].
        let home = self.rival_castle_reg(ri);
        let build_open = if castle_arm_registers() {
            let m16 = self.rivals[ri].owned[16] as usize;
            self.g.castle_reg[self.rivals[ri].slot as usize] == 0
                && m16 != 0
                && m16 < self.g.ent.len()
                && (self.g.ent[m16].f136 as u32) <= self.rivals[ri].mana_max
        } else {
            castle.is_none()
                && self.rivals[ri].known[16]
                && self.rivals[ri].mana_max >= self.rival_castle_price(ri)
        };
        if build_open {
            if self.rival_scout_site(ri, i) {
                self.rivals[ri].state = AiState::Build;
                return;
            }
        }
        // 2. Flee home hurt (sub_14310 :18480). ⭐ The PREDICATE
        // writes the target itself (:18489-90 — `+146` = the
        // established castle from wizext+50, `+148` = its signature);
        // this transition is NOT targetless.
        if let Some(c) = home
            && self.g.ent[i].act_life < (self.g.ent[i].max_life / 2) as i32
        {
            self.set_rival_state(ri, AiState::Home, c as u16);
            return;
        }
        if !think {
            return;
        }
        // 3. Upgrade the castle (sub_14120 :18408). ⭐⭐ The castle is
        // the wizext+50 REGISTER (:18415), not a pool scan, and the
        // space test runs BEFORE the mana and settled gates (:18425-28
        // short-circuit: cooldown → sub_12D10 → mana → +70==4) — so
        // its sub_12D10 box-stamp side effect lands even when
        // admission then fails. mc1hwl0 t=18950: the plant tick's
        // re-decision reaches sub_12D10 on the newborn castle (+70
        // still 5), which is where the 0xE000/640/640/0x4000 box
        // arrives; the old port order refused at `tick70 == 4` first
        // and the box never landed.
        let reg = self.g.castle_reg[self.rivals[ri].slot as usize] as usize;
        if reg != 0 {
            let m16 = self.rivals[ri].owned[16] as usize;
            if m16 != 0
                && self.mc1_token_word48(m16) == 0
                && self.rivals[ri].cooldown[16] == 0
                && self.g.castle_upgrade_space_ok(reg)
                // ⭐ `sub_14120` :18427 is `a1->+136 < v3->+136` on
                // the TOKEN (`v3` = `sub_14E60(a1, 0x10)`), exactly
                // like `sub_15E90`'s — NOT `CASTLE_CAP[level]`. The
                // two agreed only while the port re-priced the token
                // every tick; D5's ladder-event law (L6) made the
                // token's live `+136` the last LEVEL EVENT's price,
                // which on a register shared through a recycled slot
                // is a different castle's rung altogether (mc1l49
                // t=35404: token 26 holds 40,000 = CAP[3] under a
                // level-4 castle's CAP[4] = 80,000).
                && (if castle_arm_registers() {
                    self.rivals[ri].mana_max >= self.g.ent[m16].f136 as u32
                } else {
                    self.rivals[ri].mana_max
                        >= Gen::CASTLE_CAP[self.g.ent[reg].f26.clamp(0, 7) as usize] as u32
                })
                && self.g.ent[reg].tick70 == 4
            {
                // ⭐ sub_14120 :18432-33 — the predicate stamps the
                // castle into `+146`/`+148` on its way to returning 1.
                self.set_rival_state(ri, AiState::Upgrade, reg as u16);
                return;
            }
        }
        // 4. Raid an enemy castle (sub_143A0 :18496).
        if self.rival_has_offense(ri) && self.rival_pick_castle_target(ri, i) {
            self.rivals[ri].state = AiState::RaidCastle;
            return;
        }
        // 5. Attack an enemy wizard (sub_145B0 :18541).
        if self.rival_has_offense(ri) && self.rival_pick_wizard_target(ri, i) {
            self.rivals[ri].state = AiState::AttackWizard;
            return;
        }
        // 6. Intercept a fat enemy balloon (sub_147E0 :18596). The
        // pick opens on the same offense gate as the castle/wizard
        // arms (:18611 `sub_16920` alone — no castle-capable clause):
        // a disarmed wizard can't raid, so a razed, token-scattered
        // rival falls straight through to the ball claim (mc1l5
        // t=19577: Vodor abandons the human's fat balloon for the
        // wild ball the possess arm prices against his razed token).
        if self.rival_has_offense(ri) && self.rival_pick_balloon_target(ri, i) {
            self.rivals[ri].state = AiState::RaidBalloon;
            return;
        }
        // 7. Claim mana balls (sub_14230 :18439-52): needs spell 3;
        // with the castle spell owned, only while the ceiling sits at
        // or under the TOKEN's LIVE +136 price cache (:18452 reads
        // `wiz +136 <= manifestation +136` — sub_47DD0's stamp:
        // CAP[level] housed, 1000 ctor, 5000 after a raze), so
        // claiming re-opens after every upgrade AND while razed.
        // mc1l5 t=16081: castle-less Vodor at ceiling 1768 re-picks
        // the wild 2000-mana ball (Possess, target 553) against his
        // razed token's 5000 — the port's static-cost stand-in
        // (1768 > 1000) kept him parked on a freed balloon slot.
        let m16 = self.rivals[ri].owned[16] as usize;
        let claim_open = m16 == 0 || self.rivals[ri].mana_max <= self.g.ent[m16].f136.max(0) as u32;
        // ⭐⭐⭐ The spell term is `sub_14E60(a1, 3u)` — the POSSESS
        // TOKEN REGISTER, bounded only from below — not `known[3]`.
        // See [`possess_arm_register`] for the shipped bytes and the
        // mc1hwl8 t=24661 witness.
        let claim_spell = if possess_arm_register() {
            self.rivals[ri].owned[3] as i16 > 0
        } else {
            self.rivals[ri].known[3]
        };
        if claim_spell && claim_open && self.rival_pick_ball_target(ri, i) {
            self.rivals[ri].state = AiState::Possess;
            return;
        }
        // 8. Hunt any mana holder (sub_14B10 :18650).
        if self.rival_pick_mana_target(ri, i) {
            self.rivals[ri].state = AiState::HuntMana;
            return;
        }
        // 9. Idle (sub_14DC0 :18749). ⭐ The HOME leg stamps the
        // castle (:18760-61); only the CRUISE leg (:18756) writes
        // nothing but the brain byte.
        if let Some(c) = home
            && self.g.ent[i].act_life < self.g.ent[i].max_life as i32
        {
            self.set_rival_state(ri, AiState::Home, c as u16);
        } else {
            self.rivals[ri].state = AiState::Cruise;
        }
    }

    /// Conformance import: reconstruct the retail AI lanes so the
    /// imported rival resumes mid-decision. The state handler runs
    /// BEFORE the selector (sub_13170 :17847), so state and a target
    /// that survives `target_alive` must arrive together — a Fresh
    /// import re-runs the cascade and re-aims f34 off retail's lock.
    /// Target and site ride the already-imported carpet entity (+146
    /// tr-translated by import_ent, +150/+152); the signature is
    /// recomputed, which reproduces retail's stored +148 exactly.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reanchor_rival_ai(
        &mut self,
        ri: usize,
        ai_state: u8,
        burst: i16,
        poverty: u16,
        cooldown: &[u16; SPELL_COUNT],
        learn: &[u16; SPELL_COUNT],
        hate: &[u16; 8],
        war: &[u16; 8],
        owned_slots: &[u16; SPELL_COUNT],
        spell_list: &[i32; SPELL_COUNT],
        life_rate: u16,
        regen_stall: u16,
        stored_sig: u16,
    ) {
        let e = &self.g.ent[self.rivals[ri].ent as usize];
        let target = e.f146;
        let site = (e.dest_x, e.dest_y);
        // The stored signature imports RAW (the carpet's +148, passed
        // through from the record) — recomputing it from the live
        // target would blind the staleness test (sub_15440 is
        // sig-vs-stored ONLY): retail freezes on a target whose
        // record changed since the pick, and a recomputed sig always
        // matches itself. The human target keeps the port's sentinel
        // convention.
        let sig = if target == PLAYER_TARGET {
            PLAYER_TARGET
        } else {
            stored_sig
        };
        let r = &mut self.rivals[ri];
        r.state = AiState::from_retail(ai_state);
        r.target = target;
        r.target_sig = sig;
        r.burst = burst;
        r.poverty = poverty != 0;
        r.cooldown = *cooldown;
        r.learn = *learn;
        r.site = site;
        r.hate = *hate;
        for (w, &v) in r.war.iter_mut().zip(war) {
            *w = v != 0;
        }
        // The book columns ride the record: +676 (owned manifestation
        // slots by spell id — retail rebuilds it every housekeeping
        // tick from the +532 acquisition list, sub_45C10 :55304, so
        // the settled value IS the rebuild's output). Without this the
        // record kept the FRESH world's spawn slots and every cast arm
        // (`owned[s]`) stamped a stranger's f26 — the l2 corpus put
        // Vodor's burst on the HUMAN's imported fireball token, 1254
        // rows. `known` follows: a nonzero slot is an owned spell.
        r.owned = *owned_slots;
        for (s, &m) in owned_slots.iter().enumerate() {
            if m != 0 {
                r.known[s] = true;
            }
        }
        // The +532 acquisition list rides the record verbatim — the
        // death scatter and the respawn re-grant both iterate it in
        // place, so its ORDER (pickup history, unrecoverable from the
        // +676 book) is state.
        r.acq = *spell_list;
        // The regen lanes (:17990-18018): the applied-then-selected
        // life-rate register and the (AI-unread, but mirrored) stall.
        r.life_rate = life_rate as i32;
        r.regen_stall = regen_stall;
    }

    /// Retail's `+48` off a wizard's owned-token register, READ THE
    /// WAY `sub_14E60`'s callers read it: with no class guard.
    ///
    /// ⭐⭐⭐ THE TOKEN REGISTER IS A RAW SLOT INDEX AND RETAIL NEVER
    /// CHECKS WHAT LIVES THERE. `sub_14E60` (VA 0x14E60 = CARPET.EXE
    /// file 0x2D658) is, verbatim from the shipped binary:
    /// `movswl 0x2a4(%edx,%eax,2),%edx` (wizext+676+2*spell, SIGNED),
    /// three `lea`s for `164*idx`, `add` the pool base, `cmp`/`jbe`
    /// against that base, `ret`. No class, no flags, no liveness. Its
    /// caller `sub_14120` (file 0x2C971) then does
    /// `cmpw $0x0,0x30(%eax)` — the **+48** word of whatever record
    /// now occupies the slot.
    ///
    /// The port homes retail's `+48` in [`Ent::f26`] only while the
    /// record is a live class-12 manifestation (`import_ent`); on any
    /// other class `f26` is retail's `+26`, a different word entirely.
    /// mc1l49 t=26530 is the witness: wizard 1's `owned[16] = 26`, and
    /// slot 26 has been recycled into a `(9,16)` bolt whose `+26 = 16`
    /// and whose `+48 = 0`. Retail passes the gate and promotes the
    /// rival from RaidCastle to Upgrade (brain 7 → 1, `+146` 933 →
    /// 619, `+148` 1084 → 980); the port read 16, refused, and stayed
    /// on the raid.
    ///
    /// ⚠ THIS EXTENDS AN EXISTING LAW, IT DOES NOT REPLACE IT. The
    /// retired `mc1_freed_token_burst` import seat used to cover a
    /// **FREED** slot (`class64 == 0`) by writing retail's `+48`
    /// straight into `f26` at import time — and `f26` is right for that case *for the whole free
    /// run*, because the port used `f26` as the token's own `+48`
    /// home while it was alive and `Gen::free_entity` writes nothing
    /// but `class64 = 0`. The gap this closes is the slot that has
    /// been **RE-TAKEN by a live record of another class**, where the
    /// seat cannot fire without clobbering that record's own `+26`.
    ///
    /// ⭐⭐⭐ **AND IT IS ALREADY TRUE IN NATIVE PLAY, BECAUSE RETAIL'S
    /// ALLOCATOR MEMSETS.** `NewEvent_372C0` (VA 0x372C0 =
    /// `CARPET.EXE` file 0x4FAB8) merges both arms — the free-stack
    /// pop and the sacrifice — at file **0x4FB5B**, which is
    /// `push $0xa4 / push $0x0 / push %ebx / call 0x75428`, i.e.
    /// `memset(record, 0, 164)` over the WHOLE 164-byte record
    /// (0x75428 is Watcom's `memset`: it byte-replicates arg2 through
    /// `%edx` and tail-calls the filler). So a RE-TAKEN slot carries
    /// no stranger residue at all — retail reads `+48 == 0` there
    /// unless the new occupant writes the word itself, and the port's
    /// `Ent::default()` in [`Gen::new_event`] gives `raw48 == 0` by
    /// exactly the same act. And `sub_41E90` (file 0x5A688-0x5A6AE)
    /// frees with the unlink, `movb $0x0,0x40(%ebx)` and the stack
    /// push and NOTHING ELSE, so `+48` survives a free — which is why
    /// [`Gen::free_entity`] carries a dying manifestation's live
    /// `f26` across the seam into `raw48` and the class-0 arm below
    /// reads the shadow rather than the freed record's own `+26`.
    ///
    /// ⚠ RESIDUAL, MEASURED not assumed: classes that write `+48`
    /// THEMSELVES have no port home for the word at all, so `raw48`
    /// stays 0 natively where retail's is not. Off the three focus
    /// takes' `state.struct_b64`, the `owned[]`-named records with a
    /// nonzero `+48` and a class other than 12/0 are `(10,39)`,
    /// `(5,15)`, `(10,41)` and `(12,2)` — see the dig report.
    fn mc1_token_word48(&self, m16: usize) -> i16 {
        let Some(e) = self.g.ent.get(m16) else {
            // Retail's `sub_14E60` bounds only from BELOW (`jbe` the
            // pool base); an over-range register is an out-of-bounds
            // READ we cannot reproduce, and both callers already
            // treat 0 as "no notice / no refusal".
            return 0;
        };
        if e.class64 == 12
            || crate::engine::features::no_mc1_token_raw48()
            || (e.class64 == 0 && crate::engine::features::no_mc1_token_raw48_native())
        {
            // Class 12 — a live manifestation: `f26` IS the `+48`
            // home and the one the native token machine keeps up to
            // date.
            e.f26
        } else {
            // Class 0 — a FREED record — and every live record of
            // another class read the raw shadow. On the free path
            // [`Gen::free_entity`] carries a manifestation's live
            // `f26` across into `raw48`, so a freed TOKEN still wears
            // its burst; a freed NON-token wears whatever `+48` it
            // had, which for a natively allocated record is the
            // allocator's memset zero. The pre-dig code read `f26`
            // here, which on a freed non-token is that record's
            // `+26` — a different word (mc1l49 `owned[16]` names a
            // freed `(0,13)` whose `+26` is 116 while retail's `+48`
            // is 0, i.e. retail PROMOTES and the port refused).
            e.raw48.0 as i16
        }
    }

    /// THE ATTACK REGISTERS, READ BLIND (round 168). `sub_14E60` (remc1hw
    /// sub_main.cpp:16905; [`Self::mc1_token_word48`] carries the
    /// shipped bytes) resolves `pool[wizext+676+2*spell]` on a SIGNED
    /// index test alone, and the `+676` rebuild files an acquisition
    /// entry under the MODEL byte of whatever record its slot now holds.
    /// The picker `sub_16030`, the readiness `sub_15A00`, the
    /// wait-or-continue test `sub_15E90` and the commit `sub_155F0` then
    /// take that record's words as a token's: `+136` is the PRICE (no
    /// static table anywhere in the four), `+48` the busy word, and the
    /// commit's arm is `+48 = +50` on the record, whatever it is.
    ///
    /// WITNESS mc1hwl12 t=16994: wizard 3's acquisition list names slot
    /// 941, recycled into a `(5,15)` mob, so `owned[15] = 941`. The
    /// mob's `+136` is 0, which a purse of 1000 covers: retail walks
    /// 17 (13000 over the 1000 ceiling), 7, 20, 0 (on cooldown) and
    /// commits "Lightning" through the mob — `cooldown[15]` 0 -> 1,
    /// burst 1 -> 2, the mob's `+48 = +50` (0), no bolt — and a fired
    /// cast tick skips the arrival hover. The port priced spell 15 off
    /// the table, found the purse short, hovered: z 4 off on nine heads.
    ///
    /// The speed register has its own reader
    /// ([`Self::rival_blind_speed_reg`], round 167); Castle keeps
    /// [`Self::rival_castle_price`] and [`castle_token_index`].
    /// `MGC_NO_MC1_RIVAL_BLIND_REGISTER=1` restores the table price and
    /// the validated token.
    fn rival_reg(&self, ri: usize, s: usize) -> Option<usize> {
        let m = self.rivals[ri].owned[s] as i16;
        (m > 0 && (m as usize) < self.g.ent.len()).then_some(m as usize)
    }

    /// The commit's arm through [`Self::rival_reg`]: `+48 = +50`
    /// (`sub_155F0`, every case). `+48` homes in `f26` on a class-12
    /// record and in the raw shadow on any other.
    fn rival_reg_arm(&mut self, m: usize) {
        let reload = self.g.ent[m].f50;
        if self.g.ent[m].class64 == 12 {
            self.g.ent[m].f26 = reload;
        } else {
            self.g.ent[m].raw48 = crate::engine::features::Raw48(reload as u16);
        }
    }

    fn set_rival_state(&mut self, ri: usize, s: AiState, target: u16) {
        self.rivals[ri].state = s;
        self.rivals[ri].target = target;
        self.rivals[ri].target_sig = self.target_sig(target);
        // Every retail pick writes the wizard ENTITY's +146/+148
        // directly (sub_14B10 :18744-45 and its siblings) — the
        // corpus grades the column. ⚠ HOME AND UPGRADE ARE NOT
        // TARGETLESS: their selector PREDICATES stamp the established
        // castle themselves (`sub_14310` :18489-90, `sub_14120`
        // :18432-33, `sub_14DC0`'s home leg :18760-61), all three off
        // wizext+50. The only genuinely targetless transition is
        // Build (`sub_13F00`) and Idle's CRUISE leg (:18756), which
        // touch `+415` alone. mc1l5 t=933 is the exemplar: Vodor's
        // upgrade predicate fires and retail re-points him from the
        // mana ball 681 at his own keep 680 (`+148` 2000 -> 1061),
        // where the port left the ball standing.
        if target != 0 {
            let ent = self.rivals[ri].ent as usize;
            self.g.ent[ent].f146 = target;
        }
    }

    /// Target signature (sub_15420 :19039): team + model + class<<7.
    ///
    /// ⭐⭐ THE TEAM WORD IS RETAIL'S OWNER **SLOT**, AND THE PORT DOES
    /// NOT STORE SLOTS THERE. Every imported `+24` runs through the
    /// importer's `tr()`, which rewrites the human carpet's slot to
    /// the [`PLAYER_TARGET`] sentinel (the human is not a pool record
    /// in native play). The stored `+148` imports RAW — retail's own
    /// arithmetic off slot 472 — so a port-side recompute over a
    /// HUMAN-OWNED target read 0xFFFF where retail read 472 and the
    /// staleness test refused a target that was perfectly alive. The
    /// handler then returned with NO WRITES, which is invisible to
    /// pair mode as anything but a one-tick-stale lane
    /// (mc1hwl0 t=1902: rival 0 raids the human's castle 785,
    /// stored sig 858 = 472+2+(3<<7) against a recompute of 385
    /// = 0xFFFF+2+384 wrapped, and `+34` froze at the imported value
    /// for the rest of the take).
    ///
    /// Resolving the sentinel back to the imported carpet slot puts
    /// the recompute in retail's numbering. A NATIVE world has no
    /// pooled carpet (`mc1_carpet_slot` 0); there the sentinel stays
    /// and the arithmetic is merely self-consistent, which is all it
    /// ever has to be — nothing imported can disagree with it.
    fn target_sig(&self, target: u16) -> u16 {
        if target == 0 {
            return 0;
        }
        if target == PLAYER_TARGET {
            return PLAYER_TARGET;
        }
        let e = &self.g.ent[target as usize];
        let team = if e.id24 == PLAYER_TARGET && self.mc1_carpet_slot != 0 {
            self.mc1_carpet_slot
        } else {
            e.id24
        };
        team.wrapping_add(e.model65 as u16)
            .wrapping_add((e.class64 as u16) << 7)
    }

    /// Target staleness (sub_15440 :19044): the SIGNATURE compare and
    /// NOTHING else — no life test, no free-flag test. A dying
    /// creature stays a valid target (retail chases the corpse); a
    /// FREED slot goes stale because the free clears class64 and the
    /// sig moves. The human carpet's sig survives death AND respawn —
    /// retail never drops the human by staleness.
    fn target_alive(&self, target: u16, sig: u16) -> bool {
        if target == 0 {
            return false;
        }
        if target == PLAYER_TARGET {
            return sig == PLAYER_TARGET;
        }
        self.target_sig(target) == sig
    }

    /// Castle-site scout (sub_13F00 :18358-402): walk the 4x4 grid of
    /// supercells starting at the wizard's OWN cell (inner x, outer y),
    /// testing two candidates per cell in order — the cell corner, then
    /// the cell mid (corner + 0x1F00). A candidate is accepted when the
    /// wizard has no foreign castle, or the one nearest it (toroidal
    /// squared-Euclidean, sub_15260/sub_42410) sits farther than 12288
    /// in CHEBYSHEV distance (max|dx|,|dy|; sub_42300). Retail returns
    /// the FIRST candidate that passes — NOT the one nearest the
    /// wizard. For a crater-bound wizard the first pass is the corner of
    /// its home supercell (on the surrounding rim), never the crater
    /// centre; picking the nearest instead planted dead-centre in the
    /// crater — more "deliberate"-looking than retail but wrong.
    fn rival_scout_site(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (sx, sy) = (self.g.ent[i].x, self.g.ent[i].y);
        // ⭐⭐ The home supercell derives from the wizard's x/y read
        // as SIGNED i16, divided by 16384 TRUNCATING toward zero
        // (:18362-67's CFSHL signed-division idiom) — the movsx
        // class again. A wizard in the upper half of the wrap
        // (x >= 0x8000 → negative i16) starts the walk at cell 0 or
        // 3-from-truncation, NOT `u16 >> 14`: mc1l5 t=14694, Vodor
        // rebuilds at x=65333 (i16 −203 → cell 0) and retail's
        // FIRST candidate is (0,0) — accepted at once (the human's
        // castle wraps to Chebyshev 31232) — where the port's
        // `>> 14` began at cell 3 and planted a map-quadrant away.
        let cx0 = (sx as i16 / 16384) as i32;
        let cy0 = (sy as i16 / 16384) as i32;
        for dy in 0..4i32 {
            let by = ((((cy0 + dy) & 3) as u16) << 14) as u16;
            for dx in 0..4i32 {
                let bx = ((((cx0 + dx) & 3) as u16) << 14) as u16;
                for (ox, oy) in [(0u16, 0u16), (0x1F00, 0x1F00)] {
                    let (tx, ty) = (bx.wrapping_add(ox), by.wrapping_add(oy));
                    // Retail walks the candidates THROUGH THE SCRATCH
                    // RECORD — v1 is pool slot 0 and each probe writes
                    // its x/y (v1[36]/v1[37], :18374-75/:18385-86); the
                    // scratch keeps the last probed candidate after the
                    // scout ends (the parting-shot family reads it).
                    // Raw field writes: slot 0 is parked, never linked.
                    self.g.ent[0].x = tx;
                    self.g.ent[0].y = ty;
                    // The foreign castle nearest this candidate by
                    // toroidal squared-Euclidean — ⭐ sub_15260 WALKS
                    // THE TICK-TOP WIZ CHAIN (bucket[0]), per-node
                    // gates `+24 != mine && +65 == 2` and NOTHING
                    // else: no 0x400 test, no life re-test (the
                    // chain build is the life test). A husk
                    // soft-killed MID-TICK still vetoes a candidate
                    // (mc1l5 t=14694: rebuilding after the razed
                    // keep, retail's scan rejects the first two
                    // supercell candidates and plants at (0,0) where
                    // the port's 0x400-skipping pool scan took the
                    // first — Vodor flew off 292° instead of 179°).
                    // ⭐ The nearest-castle key is UNSIGNED (:18937
                    // `unsigned int v2 = -1`): sub_42410's i32 sum
                    // overflows at the exact half-map diagonal
                    // (dx = dy = ±32768 ⇒ 2·2^30 = i32::MIN), and
                    // retail reads that wrap as the LARGEST key. A
                    // signed seed elected the phantom "nearest" and
                    // green-lit a site retail rejects (mc1l49
                    // t=3298: cand (49152,16384) vs the far corner
                    // castle (16384,49152)).
                    let mut near_xy: Option<(u16, u16)> = None;
                    let mut near_d2 = u32::MAX;
                    for c in 0..self.g.wiz_chain.visible_len() {
                        let j = self.g.wiz_chain.list[c] as usize;
                        let e = &self.g.ent[j];
                        if e.model65 == 2 && e.id24 != me {
                            let d2 = Gen::dist2_sq(tx, ty, e.x, e.y) as u32;
                            if d2 < near_d2 {
                                near_d2 = d2;
                                near_xy = Some((e.x, e.y));
                            }
                        }
                    }
                    // Accept when clear, or the nearest castle's
                    // Chebyshev gap exceeds 12288 (sub_42300).
                    let ok = match near_xy {
                        None => true,
                        Some((cx, cy)) => {
                            let ddx = (tx.wrapping_sub(cx) as i16 as i32).abs();
                            let ddy = (ty.wrapping_sub(cy) as i16 as i32).abs();
                            ddx.max(ddy) > 12288
                        }
                    };
                    if std::env::var_os("MGC_RIVAL_TRACE").is_some() {
                        eprintln!(
                            "[scout t={}] cand=({tx},{ty}) near={near_xy:?} ok={ok}",
                            crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                        );
                    }
                    if ok {
                        self.rivals[ri].site = (tx, ty);
                        // The accept stamps the wizard's own site
                        // triple (:18381-83): +150/+152 = the winning
                        // candidate, +154 = the SCRATCH record's z —
                        // which the scout never writes, so the site
                        // datum is whatever z slot 0 carries. The
                        // Build hover (:18160-66) steers toward it.
                        let sz = self.g.ent[0].z;
                        let e = &mut self.g.ent[i];
                        e.dest_x = tx;
                        e.dest_y = ty;
                        e.site_z = sz;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Any offense spell owned (sub_16920 :19856: {0,15,8,17,20,7}).
    pub(crate) fn rival_has_offense(&self, ri: usize) -> bool {
        [0usize, 15, 8, 17, 20, 7]
            .iter()
            .any(|&s| self.rivals[ri].owned[s] != 0)
    }

    /// The war test's two TAG-RECORD operands (`sub_15080` :18895 /
    /// `sub_143A0` :18514): the hate index `tagrec->+160->+48` and the
    /// ceiling `tagrec->+136`. A tag that seats no wizard still has a
    /// `+160` — `NewEvent`'s default, whose `+48` is a BSS zero — so
    /// retail scores it against `hate[0]` off the tag record's own
    /// `+136`. See [`default_wizext_team`].
    fn hate_team_of_tag(&self, tag: u16) -> Option<(u8, i32)> {
        if let Some(o) = self.owner_slot(tag) {
            return Some((o, self.wizard_wealth(o) as i32));
        }
        if !default_wizext_team() {
            return None;
        }
        let wealth = self.g.ent.get(tag as usize).map_or(0, |e| e.f136);
        Some((0, wealth))
    }

    /// The hate gate (:18514 etc.): hate[owner] over the wealth-
    /// scaled threshold.
    ///
    /// ⭐⭐⭐ **THE WAR THRESHOLD IS 32-BIT SIGNED AND IT OVERFLOWS.**
    /// `CARPET.EXE` VA 0x15080 file 0x2D935-0x2D969 is
    /// `mov 0x74eb(%edx),%eax` (the ceiling `+136`) / `cdq` /
    /// `idiv $10` / `movswl 0x20a(%ebp),%edx` (`+522`, the aggression,
    /// SIGNED 16-bit) / `imul %eax,%edx` — a 32-bit `imul` that keeps
    /// the LOW half only — / `cdq` / `idiv $255` /
    /// `mov $0xc350,%edx` / `sub %eax,%edx` / `cmp (%esp),%eax` /
    /// `jge` — a SIGNED compare against the zero-extended hate word.
    /// The port's `50_000u32.saturating_sub(wealth / 10 * agg / 255)`
    /// was wrong twice over: it clamped a threshold retail lets go
    /// negative (so a rival with `hate == 0` read NOT-at-war where
    /// retail reads at-war), and it computed the product in u32, so a
    /// ceiling past 84,215,040 — where `+136/10 * 255` leaves i32 —
    /// never wrapped.
    ///
    /// The wrap is load-bearing, not a curiosity: mc1hwl5's `(3,3)`
    /// keep at slot 7 carries a `+136` that climbs past 10⁸ over the
    /// take (1,062,574 at t=25350; 87,064,642 at t=25514), so retail's
    /// threshold flips from −56,257 (at war, all 33 of its balls
    /// scored from the castle register) to +8,186,545 (NOT at war,
    /// scored from the wizard through the neutral arm) somewhere
    /// between t=25490 and t=25510 — and the port has to flip with it.
    ///
    /// `MGC_NO_MC1_HATE_SIGNED_WRAP=1` restores the u32 saturating form.
    fn hate_over(&self, ri: usize, slot: u8, wealth: i32) -> bool {
        let r = &self.rivals[ri];
        if hate_signed_wrap() {
            let threshold = 50_000i32 - (wealth / 10).wrapping_mul(r.agg as i16 as i32) / 255;
            return r.hate[slot as usize] as i32 > threshold;
        }
        let threshold = 50_000u32.saturating_sub(wealth.max(0) as u32 / 10 * r.agg as u32 / 255);
        r.hate[slot as usize] as u32 > threshold
    }

    /// Enemy-castle pick (sub_143A0 :18496-536): hated-and-undefended
    /// or plain poorer, nearest in range.
    ///
    /// ⭐⭐ IT WALKS THE TICK-TOP CLASS-3 CHAIN (`var_u32_36462[0]`
    /// :18507), NOT THE POOL — the same roster the wizard and balloon
    /// picks already use, and **membership IS the liveness filter**
    /// (`actLife >= 0 && !(flags & 0x10)`, sampled once at the tick
    /// top). The per-node tests are only `+24 != mine` and `+65 == 2`;
    /// there is no class test (the chain is class-3) and no `0x400`
    /// test (the chain build's life gate has already run).
    ///
    /// mc1hwl0 t=1920 is the exemplar and it is a DEAD CASTLE that
    /// still stands: the human's keep 785 takes its fatal hit at
    /// t=1920 (`act_life` 20000 → −2000, `+70` 4 → 6) but is not
    /// reap-flagged until t=1921, so a pool scan filtered on `0x400`
    /// still saw it, `poorer` still held (140 stored against the
    /// rival's own 4490), and the port re-elected a corpse it could
    /// never raid. Retail's chain had dropped it at the tick top, so
    /// its cascade fell straight through to the ball claim: `+415`
    /// 7 → 6, `+146` 785 → the mana ball 356.
    ///
    /// ⭐ And the range gate is the ELECTION WINNER's alone
    /// (:18531-34, strict `>=` → reject) — the same shape as the
    /// wizard pick. Filtering by range inside the election lets a
    /// far-but-nearest castle be replaced by a farther-ranked one.
    fn rival_pick_castle_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        // ⭐ Both lanes read the wizext+50 REGISTER, not a pool scan:
        // the gate (:18506 `!+50 && sub_14E60(0x10)` — castle-less
        // AND OWNS the m16 token; a scattered-token wizard raids
        // freely) and the wealth compare (:18517 indexes +29935 by
        // the register). A castle is "had" the moment the plant binds
        // +50 (:19206), establishment not required.
        let reg = self.g.castle_reg[self.rivals[ri].slot as usize] as usize;
        let my_stored = if reg != 0 {
            self.g.ent[reg].f140.max(0) as u32
        } else {
            0
        };
        if reg == 0 && self.rivals[ri].owned[16] != 0 {
            return false;
        }
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best: Option<(u16, u32)> = None;
        for c in 0..self.g.wiz_chain.visible_len() {
            let j = self.g.wiz_chain.list[c] as usize;
            let e = &self.g.ent[j];
            if e.model65 != 2 || e.id24 == me {
                continue;
            }
            let Some((owner, owner_wealth)) = self.hate_team_of_tag(e.id24) else {
                continue;
            };
            let hated = self.hate_over(ri, owner, owner_wealth);
            // Undefended: the owner is over 7680 away (:18517-22)
            // AND the owner's carpet does NOT box-overlap the castle
            // (:18518 — the third conjunct, `!sub_11950(ownerCarpet,
            // castle)` on the pool record AT the castle's id24). The
            // summed extents of a grown keep reach past the 7680
            // disc on the diagonal, so a carpet parked over its own
            // keep's corner still defends it (mc1hwl0 t=15800-18:
            // 19 pairs where the y-leg of the overlap holds the veto
            // until retail elects at 15820). The human's carpet is
            // out-of-pool here — its sub_11950 runs on the live pose
            // with the sprite-44 halves (World::overlap's box).
            // ⭐⭐ THE DEFENDER IS A RECORD, NOT A LIFE STATE. Retail
            // indexes the owner's carpet by the castle's OWN `+24`
            // (`v2 = 164 * *(__int16 *)(i + 24)`, :18517) and reads
            // `+72` / runs `sub_11950` on that record RAW — there is
            // no liveness gate, no `+70` test, no chain lookup. A
            // wizard who died THIS TICK still defends his keep,
            // because his carpet record still sits where it fell.
            // The port routed both legs through `wizard_pos`, whose
            // `LifeState::Alive` / `tick70 == 1` gates return None
            // for a falling wizard — and `is_none_or` reads None as
            // UNDEFENDED, which flips the whole raid arm on the tick
            // a hated wizard dies. mc1l49 t=5688 is the witness: the
            // human takes his fatal hit on that very tick (569 `+70`
            // 0 -> 2, life 3572 -> −428) 5,738 units from his own
            // castle 23; retail's conjunct fails and the cascade
            // falls through to arm 5, keeping `+146` = 569, while
            // the port raided the keep (`+146` 569 -> 23) and stayed
            // there for the next 298 segment heads.
            let owner_pos = if e.id24 == PLAYER_TARGET {
                Some(self.human_pose)
            } else {
                self.g.ent.get(e.id24 as usize).map(|o| (o.x, o.y, o.z))
            };
            let owner_pos = if raid_owner_pos_is_raw() {
                owner_pos
            } else {
                self.wizard_pos(owner)
            };
            let parked_on = if e.id24 == PLAYER_TARGET {
                owner_pos.is_some_and(|(wx, wy, wz)| {
                    use crate::mc1::combat::{PLAYER_HH, PLAYER_HW};
                    let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
                    wd(wx, e.x) < e.f80 as i32 + PLAYER_HW
                        && wd(wy, e.y) < e.f82 as i32 + PLAYER_HW
                        && ((e.z as i32 + e.f78 as i16 as i32) - (wz as i32 + PLAYER_HH)).abs()
                            < e.f84 as i32 + PLAYER_HH
                })
            } else {
                self.g.ent_overlap(e.id24 as usize, j)
            };
            let undefended = owner_pos
                .is_none_or(|(wx, wy, _)| Gen::dist2_sq(e.x, e.y, wx, wy) > 7680 * 7680)
                && !parked_on;
            let poorer = (e.f140.max(0) as u32)
                .saturating_add(640 * (255 - self.rivals[ri].agg as u32))
                < my_stored;
            if !(hated && undefended) && !poorer {
                continue;
            }
            let d = Gen::dist2_sq(px, py, e.x, e.y) as u32;
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        // The winner alone faces the range gate (:18531-34) — a SIGNED
        // recompute (`int >= v4*v4`), so the antipodal wrap passes it.
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32;
        let best = best.filter(|&(_, d)| (d as i32) < range.saturating_mul(range));
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::RaidCastle, t);
            true
        } else {
            false
        }
    }

    /// wizext+50, the ESTABLISHED-castle register: written only by the
    /// level-up commit (:56484), cleared at removal (:56534). The
    /// port's stand-in is the standing castle's FIRST-COMMIT latch
    /// (flags bit 1, :56057-62) — the same gate the token ladder stamp
    /// uses. An authored castle that has never leveled is UNBOUND.
    /// The pick gates' castle test (:18506/:18553/:18570): the raw
    /// wizext+50 REGISTER word — nonzero the moment the plant binds
    /// it (:19206), no establishment latch, no pool liveness. The old
    /// `flags & 2` "established" reading was invented strictness: at
    /// mc1hwl0 t=18950 it refused the whole AttackWizard pick on the
    /// plant tick (retail war-picks the human rangelessly there) and
    /// the cascade fell through to HuntMana.
    fn wiz_castle_reg(&self, slot: u8) -> u16 {
        self.g.castle_reg[slot as usize & 7]
    }

    /// `sub_47DD0` (:56617-73, CARPET.EXE file 0x605C8-0x606B4) on the
    /// record `pc` the respawning wizard's `wizext+50` register names —
    /// ONE retail routine for both columns: `sub_44D30`'s tail
    /// (0x4527A `mov 0x32(%eax),%di; test %di,%di; je`, 0x452B1 its
    /// ONLY caller) runs it for the human and every rival alike. It
    /// prices the token of the record's OWNER (`pool[+24]`, `+70 <=
    /// 1u`, `wizext+676[16]` index-tested only) at the record's `+26`
    /// through an unsigned switch whose `default:` is 0 (0x6064E `cmp
    /// $7; ja` → `xor %edx,%edx`, no clamp) and divides by the TOKEN's
    /// `movswl +50` (0x6069E). No class, model or `flags & 2` test.
    pub(crate) fn mc1_respawn_reprice(&mut self, pc: usize) {
        let (own, lvl) = (self.g.ent[pc].id24, self.g.ent[pc].f26 as u16);
        let tok = if own == PLAYER_TARGET {
            (self.player.state == LifeState::Alive).then_some(self.player.owned[16])
        } else {
            let o = own as usize;
            (o != 0 && o < self.g.ent.len() && self.g.ent[o].tick70 <= 1)
                .then(|| {
                    self.rivals
                        .iter()
                        .find(|r| r.ent == own)
                        .map(|r| r.owned[16])
                })
                .flatten()
        };
        let m = tok.unwrap_or(0) as usize;
        if m != 0 && m < self.g.ent.len() {
            let cap = if lvl <= 7 {
                Gen::CASTLE_CAP[lvl as usize]
            } else {
                0
            };
            let div = self.g.ent[m].f50 as i32;
            self.g.ent[m].f136 = cap;
            if div != 0 {
                self.g.ent[m].f140 = cap / div;
            }
        }
    }

    /// Enemy-wizard pick (sub_145B0 :18541-91), walking the tick-top
    /// wiz chain (candidates: class-3 carpets, `+65 <= 1`, not self,
    /// not spell-12-cloaked — and nothing else per node; liveness is
    /// the chain build's).
    ///
    /// ⭐⭐ THE WAR ARM SHORT-CIRCUITS (:18563-68): the FIRST chain
    /// candidate whose war flag is set is stamped as the target and
    /// the pick returns — NO nearest election, NO range test. Only
    /// hated/bully candidates enter the distance election, and the
    /// range gate (`v_28 + 10`, strict `<`) applies to that election's
    /// winner alone (:18585-87). mc1l5 t=11341: the human shells
    /// Vodor's castle from ~14,200 units out (far past 8192+10); war
    /// latches at t=11308 and retail still retargets him RANGELESSLY
    /// at his next think tick, where an in-election range test keeps
    /// him on a bee.
    ///
    /// ⭐ The hated threshold reads the CANDIDATE ENTITY's `+136`
    /// ceiling lane, inclusive (`50000 − agg·(f136/10)/255 <= hate`,
    /// :18570); the port reads the live ceiling mirrors those lanes
    /// track. ⭐ The bully arm (:18571-77) wants an UNBOUND candidate
    /// (`wizext+50 == 0`) that KNOWS the castle spell and is poorer by
    /// `+140` — not merely castle-less. ⭐ The leading self-test
    /// (:18549): a castle-capable rival whose own castle is UNBOUND
    /// returns 0 — while it could be building, it does not hunt
    /// wizards.
    fn rival_pick_wizard_target(&mut self, ri: usize, i: usize) -> bool {
        // :18553 — castle-less (raw +50) AND owns the m16 token.
        if self.rivals[ri].owned[16] != 0 && self.wiz_castle_reg(self.rivals[ri].slot) == 0 {
            return false;
        }
        let me = self.rivals[ri].ent;
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let my_agg = self.rivals[ri].agg as i64;
        // ⭐ Both `+140` reads are SIGNED (CARPET.EXE 0x14719-29 `jge`):
        // a fatal shield quarter's wrapped purse is the POOREST, not the
        // richest — see
        // [`crate::engine::features::no_mc1_rival_bully_signed_purse`].
        let signed = !crate::engine::features::no_mc1_rival_bully_signed_purse();
        let purse = move |m: u32| if signed { m as i32 as i64 } else { m as i64 };
        let my_mana = purse(self.rivals[ri].mana);
        // Candidates in CHAIN order. The human's carpet is never a
        // pool record in the port (imports anchor the human at the
        // walk slot without materializing the entity), so the human
        // is judged as a pre-pass — retail's chain order puts his
        // carpet below the rivals' in every corpus take.
        let mut war_pick: Option<u16> = None;
        let mut best: Option<(u16, u32)> = None;
        let judge = |tgt: u16,
                     x: u16,
                     y: u16,
                     invisible: bool,
                     ceiling: i64,
                     mana: i64,
                     unbound_knows16: bool,
                     hate: i64,
                     war: bool,
                     best: &mut Option<(u16, u32)>|
         -> bool {
            if invisible {
                return false; // spell-12 targets are skipped (:18558)
            }
            if war {
                return true; // ⭐⭐ first-in-chain, rangeless (:18563-68)
            }
            let hated = 50_000 - my_agg * (ceiling.max(0) / 10) / 255 <= hate;
            let bully = unbound_knows16 && mana + 32 * (255 - my_agg) < my_mana;
            if hated || bully {
                let d = Gen::dist2_sq(px, py, x, y) as u32;
                if best.is_none_or(|(_, bd)| d < bd) {
                    *best = Some((tgt, d));
                }
            }
            false
        };
        // Candidacy is TICK-TOP bucket[0] membership, not live state
        // (mc1hwl0 t=7592: the rival's own kill lands before its
        // selector runs, and retail still picks the corpse) — AND the
        // human rides the chain's VISIBLE PREFIX like every pooled
        // carpet: see [`mc1_human_on_wiz_roster`].
        let human_seen = self.human_bucket_alive && {
            if mc1_human_on_wiz_roster() {
                let hs = self.mc1_carpet_slot;
                let hpos = self.g.wiz_chain.list.partition_point(|&s| s < hs);
                hpos < self.g.wiz_chain.cut
            } else {
                true
            }
        };
        if human_seen
            && judge(
                PLAYER_TARGET,
                self.human_pose.0,
                self.human_pose.1,
                self.ghost || self.mc1_invis_notice(self.player.owned[12], self.player.invisible),
                self.player.mana_max as i64,
                purse(self.player.mana),
                // :18570-72 — the target's raw +50 register + owned
                // m16 token (sub_14E60), not the establishment latch.
                self.wiz_castle_reg(0) == 0 && self.player.owned[16] != 0,
                self.rivals[ri].hate[0] as i64,
                self.rivals[ri].war[0],
                &mut best,
            )
        {
            war_pick = Some(PLAYER_TARGET);
        }
        if war_pick.is_none() {
            for c in 0..self.g.wiz_chain.visible_len() {
                let j = self.g.wiz_chain.list[c] as usize;
                let e = &self.g.ent[j];
                if e.model65 != 1 || e.id24 == me {
                    continue;
                }
                let Some(oj) = self.rivals.iter().position(|r| r.ent as usize == j) else {
                    continue;
                };
                if self.rivals[oj].eliminated {
                    continue;
                }
                let o = &self.rivals[oj];
                let oslot = o.slot;
                if judge(
                    o.ent,
                    e.x,
                    e.y,
                    self.mc1_invis_notice(o.owned[12], o.invisible),
                    o.mana_max as i64,
                    purse(o.mana),
                    // :18570-72 — raw +50 register + owned token, as
                    // for the human above.
                    self.wiz_castle_reg(oslot) == 0 && o.owned[16] != 0,
                    self.rivals[ri].hate[oslot as usize] as i64,
                    self.rivals[ri].war[oslot as usize],
                    &mut best,
                ) {
                    war_pick = Some(j as u16);
                    break;
                }
            }
        }
        if let Some(t) = war_pick {
            self.set_rival_state(ri, AiState::AttackWizard, t);
            return true;
        }
        // The range gate applies to the ELECTION winner only, strict
        // and SIGNED (:18585-87: `int d² >= (v_28+10)² → return 0`).
        let Some((t, d)) = best else {
            return false;
        };
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32 + 10;
        if (d as i32) >= range.saturating_mul(range) {
            return false;
        }
        self.set_rival_state(ri, AiState::AttackWizard, t);
        true
    }

    /// Enemy-balloon pick (sub_147E0 :18596-645): a walk of the
    /// TICK-TOP class-3 chain (`var_u32_36462[0]` :18615 — no life or
    /// 0x400 test, the bucket[0] family) for foreign model-3s whose
    /// owner is hated (live wealth-scaled, the owner ENTITY's +136
    /// through id24), cargo over 10*(275-agg), and NOT at home — where
    /// "at home" is `sub_11950` = the FULL summed-extents AABB vs the
    /// owner's BOUND castle (wizext+50 :18628; unbound reads slot 0,
    /// the scratch). ⚠ NOT a distance disc: the human's level-6
    /// castle carries 6784-unit extents, so its balloons are exempt
    /// nearly 7000 out (mc1l5 t=19577 — balloon 900 at dx 5852 is
    /// docked, Vodor falls through to the ball claim). The range gate
    /// applies to the ELECTION WINNER only, strict (:18638-40
    /// `d² >= v_28² → return 0` — no fallback to the runner-up).
    fn rival_pick_balloon_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let cargo_gate = 10 * (275 - self.rivals[ri].agg as u32);
        let mut best: Option<(usize, u32)> = None;
        for c in 0..self.g.wiz_chain.visible_len() {
            let j = self.g.wiz_chain.list[c] as usize;
            let e = &self.g.ent[j];
            if e.model65 != 3 || e.id24 == me {
                continue;
            }
            let owner_ent = e.id24;
            // ⚠ THE THIRD SITE OF THE SAME RETAIL EXPRESSION IS
            // DELIBERATELY LEFT ON `owner_slot` (round 161, w161b).
            // `sub_147E0` (:18624) resolves the team the same way —
            // `v3 = ownerrec->+160; hate[*(i16*)(v3+48)]` — but it
            // then runs its overlap veto against
            // `pool[*(u16*)(v3 + 50)]`, that wizext's CASTLE
            // REGISTER, which on the DEFAULT wizext is a BSS 0, i.e.
            // pool slot 0, the SCRATCH record. The port's `home`
            // below is `castle_reg[owner & 7]`, so seating a
            // non-wizard tag at team 0 here would veto against the
            // HUMAN's castle instead. Needs its own witness.
            let Some(owner) = self.owner_slot(owner_ent) else {
                continue;
            };
            if !self.hate_over(ri, owner, self.wizard_wealth(owner) as i32) {
                continue;
            }
            if (self.g.ent[j].f140.max(0) as u32) <= cargo_gate {
                continue;
            }
            let home = self.g.castle_reg[owner as usize & 7] as usize;
            if self.g.ent_overlap(j, home) {
                continue;
            }
            let d = Gen::dist2_sq(px, py, self.g.ent[j].x, self.g.ent[j].y) as u32;
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j, d));
            }
        }
        let Some((t, _)) = best else {
            return false;
        };
        // The winner's range gate is a SIGNED recompute (:18640-42).
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32;
        let d = Gen::dist2_sq(px, py, self.g.ent[t].x, self.g.ent[t].y);
        if d >= range.saturating_mul(range) {
            return false;
        }
        self.set_rival_state(ri, AiState::RaidBalloon, t as u16);
        true
    }

    /// Mana-ball pick (sub_15080 :18862): wild balls by distance;
    /// at-war owners' balls; neutral-owned only if unguarded.
    fn rival_pick_ball_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best: Option<(u16, u32)> = None;
        // ⭐ THE PICK WALKS THE TICK-TOP BALL CHAIN, NOT THE POOL.
        // `sub_15080` (:18878) seeds from `var_u32_36462[1]` — the
        // ball roster the tick head rebuilt at :52290-97 before any
        // handler ran — and follows `+0` links to the end (:18919).
        // A ball MINTED MID-TICK is therefore invisible to every
        // rival brain until the next rebuild, and that is exactly
        // what mc1l4 t=257 turns on: the human's castle is torn down
        // that tick and ejects five balls into free slots, one of
        // them slot 32, and the port's pool sweep saw it while retail
        // could not — so retail's cascade found no eligible ball at
        // all, fell through to the mana hunt, and RE-PICKED creature
        // 85 (`+146` 85 and `+148` 728 both unchanged across the
        // boundary), while the port re-pointed at the newborn 32.
        // ⚠ Neither the model test nor the 0x400 test survives the
        // move: the chain build has no life or flag filter and admits
        // models 39 AND 40, and `sub_15080` adds no model test of its
        // own — membership IS the filter.
        // The at-war arm scores from the BOUND castle REGISTER's
        // entity (:18879 — `v9 = pool + 164·wizext+50`), which for an
        // UNBOUND rival is pool slot 0: the SCRATCH record, whose x/y
        // are live state (the scout walks its candidates through it).
        let reg = self.g.castle_reg[self.rivals[ri].slot as usize] as usize;
        let (rx, ry, reg_id) = {
            let e = &self.g.ent[reg];
            (e.x, e.y, e.id24)
        };
        let trace = std::env::var_os("MGC_RIVAL_TRACE").is_some();
        for c in 0..self.g.ball_chain.visible_len() {
            let j = self.g.ball_chain.list[c] as usize;
            let (bx, by, bid, tag) = {
                let e = &self.g.ent[j];
                (e.x, e.y, e.id24, e.f144)
            };
            if trace {
                eprintln!(
                    "[bpick t={}] ri={ri} ball={j} tag={tag} best={best:?}",
                    crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                );
            }
            // (:18884) the TAG'S ENTITY decides the arm: a tag whose
            // record is not class 3 (the wild 0 included — slot 0 is
            // the scratch) takes the ungated wild arm, scored from
            // ME. PLAYER_TARGET is the port's human tag.
            let owner_is_wiz = tag == PLAYER_TARGET
                || self.g.ent.get(tag as usize).is_some_and(|o| o.class64 == 3);
            if !owner_is_wiz {
                let d = Gen::dist2_sq(px, py, bx, by) as u32;
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((j as u16, d));
                }
                continue;
            }
            if tag == me {
                continue; // already mine (:18899)
            }
            // (:18886-90) ⭐ the war test here is the LIVE
            // wealth-scaled hate formula off the owner's ceiling —
            // the latched war[] flag is the castle sweep's lane and
            // is NOT read (mc1l5 t=16081: Vodor's flag vs the human
            // is still latched while the live hate has decayed out,
            // so retail claims the wild 2000-ball where the
            // flag-reading port chased a human-owned one). At-war
            // balls score from the REGISTER's entity, not from me.
            // ⭐⭐⭐ A class-3 tag that seats no wizard — a keep, a
            // castle, any `(3, m >= 2)` — still reaches this test:
            // its `+160` is `NewEvent`'s DEFAULT wizext, whose `+48`
            // is a BSS zero, so the hate index is the HUMAN's and the
            // ceiling is the TAG RECORD's own `+136` (mc1hwl5
            // t=25348-486, 33 of the 76 balls on the chain). See
            // [`default_wizext_team`].
            let team = self.hate_team_of_tag(tag);
            if let Some((o, wealth)) = team
                && self.hate_over(ri, o, wealth)
            {
                let d = Gen::dist2_sq(rx, ry, bx, by) as u32;
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((j as u16, d));
                }
                continue;
            }
            // Neutral-owned (:18905-16): the guard is the nearest
            // FOREIGN CARPET to the BALL on the tick-top wiz chain
            // (sub_15340 — model <= 1, excluding the ball's id24 and
            // me; NO carpet at all → no take), 5120 gate; plus the
            // castle-overlap veto (sub_153B0 bound / sub_15260
            // unbound: the nearest castle to the ball, excluding the
            // ball's id24 — and, bound, the register's own castle).
            // ⚠ The HUMAN's carpet rides OUT-OF-POOL in the port, so
            // the tick-top wiz chain never holds it — retail's chain
            // always does, and its nearest-carpet guard is exactly
            // what admits a human-claimed ball the human has wandered
            // 5120+ away from (mc1l5 t=253: ball 362 at 5,900 units).
            // Weigh the live human by pose before the chain walk.
            // ⭐ Both sub-elections ride the UNSIGNED key (:18989 /
            // :19018 `unsigned int v2 = -1`, the sub_15260 law's
            // sibling call sites); the 5120² unguarded test is the
            // caller's SIGNED recompute on the WINNER (:18910
            // `int > 26214400`), so an antipodal sole guard still
            // reads GUARDED there.
            // ⭐⭐ THE HUMAN'S SEAT IN THE GUARD ELECTION IS HIS
            // TICK-TOP BUCKET[0] MEMBERSHIP, NOT HIS LIVE LIFE STATE.
            // `sub_15340` (:19003) walks the chain the tick head
            // rebuilt at :52290-97 and reads each node's LIVE `+72`;
            // a carpet that takes its fatal hit mid-tick is still IN
            // that chain and still guards, exactly as the port's own
            // `human_bucket_alive` records for every other bucket[0]
            // consumer (the learn clock, the wizard pick). Seeding
            // from `wizard_pos(0)` instead read the LIVE
            // `LifeState::Alive`, so the moment the human died the
            // balls he was standing over went UNGUARDED for one tick.
            // mc1l48 t=6377 is the witness: the human's fatal hit
            // lands that tick (681 life 50 -> −350, `+70` 0 -> 2) at
            // (2363, 43268); ball 298 sits 2,345 units off him —
            // GUARDED for retail, which claims the farther ball 291
            // (4,947 off the corpse-to-be, past the 5120 disc), while
            // the port dropped the human from the election and took
            // the nearer 298.
            // ⭐⭐⭐ `sub_15340`'s FIRST test is `+24 != the
            // CANDIDATE's +24` (0x2db5c), and it applies to the human
            // like any other chain member. The port's out-of-pool
            // human is seated outside the loop, so give his seat the
            // same id test — a human-owned manifestation (a BALLOON
            // stamps `+24` with the caster's carpet id) is not guarded
            // by its own caster. See `ball_guard_excludes_own_id`.
            let human_is_own_id =
                bid == PLAYER_TARGET || (self.mc1_carpet_slot != 0 && bid == self.mc1_carpet_slot);
            let human_guard = if ball_guard_excludes_own_id() && human_is_own_id {
                None
            } else if ball_guard_is_tick_top() {
                // …and inside the chain's visible prefix this tick
                // (`Gen::mc1_human_on_wiz_chain`, the seizure-blank /
                // sever law) — `sub_15340` walks bucket[0] like the
                // wizard pick does.
                (self.human_bucket_alive && !self.ghost && self.g.mc1_human_on_wiz_chain())
                    .then_some(self.human_pose)
            } else {
                self.wizard_pos(0)
            };
            let mut guard: Option<u32> =
                human_guard.map(|(hx, hy, _)| Gen::dist2_sq(bx, by, hx, hy) as u32);
            let mut castle: Option<(usize, u32)> = None;
            for k in 0..self.g.wiz_chain.visible_len() {
                let w = self.g.wiz_chain.list[k] as usize;
                let we = &self.g.ent[w];
                if we.id24 == bid {
                    continue;
                }
                let d = Gen::dist2_sq(bx, by, we.x, we.y) as u32;
                if we.model65 <= 1 && we.id24 != me && guard.is_none_or(|gd| d < gd) {
                    guard = Some(d);
                }
                if we.model65 == 2
                    && (reg == 0 || we.id24 != reg_id)
                    && castle.as_ref().is_none_or(|&(_, cd)| d < cd)
                {
                    castle = Some((w, d));
                }
            }
            let unguarded = guard.is_some_and(|gd| (gd as i32) > 5120 * 5120);
            let housed = castle.is_some_and(|(cs, _)| self.g.ent_overlap(j, cs));
            if unguarded && !housed {
                let d = Gen::dist2_sq(px, py, bx, by) as u32;
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((j as u16, d));
                }
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::Possess, t);
            true
        } else {
            false
        }
    }

    /// Mana-holder hunt (sub_14B10 :18650): the nearest other-team CREATURE
    /// carrying mana (to the own castle, or self if castle-less), no range
    /// cap. Gated up-front on owning an offense spell (`sub_16920` :18662):
    /// a rival with nothing to attack with never enters HuntMana — else it
    /// shadows a mana creature and casts nothing.
    ///
    /// Retail walks the per-MODEL entity buckets `str_36382[+65]` for model
    /// indices 0..=19 (`i = 0; i != 80; i += 4`) — i.e. the living-creature
    /// models. Mana BALLS (model 39) and DWELLINGS (model 45) sit in higher
    /// buckets the loop never reaches, so the mana-hunt does NOT target them
    /// (balls are the Possess/ball-claim path's job, `rival_pick_ball_target`).
    /// The port keys this as `class64 == 5`, the faithful creature filter —
    /// a slight over-approximation of models 0..19 (a class-5 creature with
    /// model >= 20, e.g. the hydra m27, would be out of retail's scan) that
    /// is immaterial while such creatures carry no mana.
    pub(crate) fn rival_pick_mana_target(&mut self, ri: usize, i: usize) -> bool {
        // sub_16920 gate (:18662): no offense spell → no hunt.
        if !self.rival_has_offense(ri) {
            return false;
        }
        let me = self.rivals[ri].ent;
        // :18664-67 — the anchor is `pool + 164 * wizext[+50]`, the
        // wizard himself when the register is 0 (index test alone).
        let anchor = self
            .rival_castle_reg(ri)
            .map(|c| (self.g.ent[c].x, self.g.ent[c].y))
            .unwrap_or((self.g.ent[i].x, self.g.ent[i].y));
        let mut best: Option<(u16, u32)> = None;
        // Retail's walk (:18669-91) is the class-5 MODEL CHAINS
        // (heads at 36382 + 4·model, bucket-major) — the tick-top
        // membership snapshot, NOT the raw pool: a creature that died
        // after the rebuild is still visible, one that was dying AT
        // tick top never entered. Only +140 > 0 and the owner tag
        // filter at walk time (live fields off the members).
        for m in 0..self.g.mob_chains.list.len() {
            for jj in 0..self.g.mob_chains.visible(m).len() {
                let j = self.g.mob_chains.visible(m)[jj];
                let e = &self.g.ent[j as usize];
                if e.id24 == me || e.f140 <= 0 {
                    continue;
                }
                let d = Gen::dist2_sq(anchor.0, anchor.1, e.x, e.y) as u32;
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((j, d));
                }
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::HuntMana, t);
            true
        } else {
            false
        }
    }

    /// A wizard's live position by slot (0 = the human).
    pub(crate) fn wizard_pos(&self, slot: u8) -> Option<(u16, u16, i16)> {
        if slot == 0 {
            return (self.player.state == LifeState::Alive).then_some(self.human_pose);
        }
        let ent = self
            .rivals
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.ent)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.slot == slot)
                    .map(|r| r.ent)
            })?;
        let e = &self.g.ent[ent as usize];
        (e.tick70 == 1).then_some((e.x, e.y, e.z))
    }

    /// A wizard's mana ceiling (the wealth term in the hate gates).
    pub(crate) fn wizard_wealth(&self, slot: u8) -> u32 {
        if slot == 0 {
            return self.player.mana_max;
        }
        self.rivals
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.mana_max)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.slot == slot)
                    .map(|r| r.mana_max)
            })
            .unwrap_or(0)
    }

    // ---- state handlers -----------------------------------------------

    fn rival_state_tick(&mut self, ri: usize, i: usize, think: bool) {
        // ⭐⭐ A STALE TARGET DOES NOT RESET THE STATE. Every combat
        // handler opens on the sig-vs-stored test and returns 0 with
        // NO writes when it fails (sub_13BA0 :18246, sub_13CA0
        // :18281, sub_13DD0 :18323) — the state byte and the target
        // KEEP, the handler simply no-ops until the think-tick
        // cascade replaces the state. There is NO Fresh transition
        // anywhere in the retail machine. The port's old
        // drop-to-Fresh prologue re-entered the cascade off-cadence:
        // mc1l5 t=12158 — Vodor's claimed ball 908 is collected and
        // its slot re-minted, retail idles in Possess for 450 ticks
        // (the cascade refusing every think round) while the port's
        // Fresh re-pick went hunting, and 1,500 ticks later its
        // Upgrade chain fired a castle ball retail never cast
        // (t=13647, the extra (9,10)/(10,43) pair).
        let needs_target = matches!(
            self.rivals[ri].state,
            AiState::Possess
                | AiState::RaidCastle
                | AiState::AttackWizard
                | AiState::RaidBalloon
                | AiState::HuntMana
        );
        if needs_target && !self.target_alive(self.rivals[ri].target, self.rivals[ri].target_sig) {
            return;
        }
        match self.rivals[ri].state {
            AiState::Fresh => {}
            // Fly home; cast 0x10 on arrival = the upgrade chain
            // (sub_13800 :18106-32).
            AiState::Upgrade => {
                // ⭐ The castle is the ENTITY's own `+146` under the
                // signature gate, never a live-castle pool scan, and a
                // failed gate returns with NO writes (0x13843 `je` →
                // `return 0`). A castle razed mid-flight keeps its slot,
                // owner, class and model, so `sub_15440` still passes
                // and retail keeps AIMING and travelling to the ruin
                // until the think-tick cascade re-picks (mc1l16
                // t=19162: castle 52 razed, retail `+34` 1521, the port
                // dropped to Fresh and froze at 1504). See
                // [`upgrade_sig_gate`].
                let c = if upgrade_sig_gate() {
                    let (t, sig) = (self.rivals[ri].target, self.rivals[ri].target_sig);
                    if !self.target_alive(t, sig) {
                        return;
                    }
                    t as usize
                } else {
                    let Some(c) = self.rival_castle(self.rivals[ri].ent) else {
                        self.rivals[ri].state = AiState::Fresh;
                        return;
                    };
                    c
                };
                let (cx, cy, cz) = {
                    let e = &self.g.ent[c];
                    (e.x, e.y, e.z)
                };
                if self.rival_approach(ri, i, cx, cy, Some(cz), 512, 2048) {
                    // :18120-27 — a FIRED cast tick returns without
                    // hovering (same shape as RaidCastle); the z-hover
                    // toward castle+512 is the refused arm's alone.
                    if !self.rival_cast(ri, i, 16) {
                        self.rival_hover_toward(i, cz.saturating_add(512));
                    }
                }
            }
            // Fly to the scouted site; plant (sub_138F0 :18142-68).
            // ⭐ NO state write — retail's handler aims, arrives and
            // casts, nothing else; the state leaves only through the
            // think-tick cascade. The old plant→Fresh invention cost
            // the follow-up: retail's Build handler runs AGAIN the
            // tick after the plant, still arrived, and its cast-16
            // now takes the BOUND arm — arming the upgrade token on
            // the day-old castle (mc1l5 t=14772: the (9,10) ball the
            // port never fired).
            AiState::Build => {
                let (sx, sy) = self.rivals[ri].site;
                if self.rival_approach(ri, i, sx, sy, None, 2048, 3072)
                    && !self.rival_cast(ri, i, 16)
                {
                    // :18160-66 — the REFUSED cast's z-hover toward the
                    // scouted site datum (+154) + 512. The datum is the
                    // SCRATCH record's z at scout-accept time (see
                    // rival_scout_site), so a parked Vodor rides the
                    // settle floor and this nudge in alternation:
                    // mc1l5 t=14772-800, retail 975 = floor 979 − 4.
                    let sz = self.g.ent[i].site_z;
                    self.rival_hover_toward(i, sz.saturating_add(512));
                }
            }
            // Claim the ball (sub_13BA0 :18236-57): approach, cast 3,
            // and inside ~5 degrees write the claim directly.
            AiState::Possess => {
                let t = self.rivals[ri].target as usize;
                let (tx, ty, tz) = {
                    let e = &self.g.ent[t];
                    (e.x, e.y, e.z)
                };
                if self.rival_approach(ri, i, tx, ty, Some(tz), 1024, 3072) {
                    let cast = self.rival_cast(ri, i, 3);
                    let facing = Gen::angdist(
                        self.g.ent[i].f30,
                        Gen::angle_between(self.g.ent[i].x, self.g.ent[i].y, tx, ty),
                    );
                    // ⚠ THE CLAIM CONE IS STRICT: `< 0x1Cu` (:18254),
                    // not `<= 28`. mc1l3 t=447 lands on the boundary
                    // exactly — Vodor (slot 585) sits at `+30 = 1082`
                    // with ball 105 bearing 1054, an angular distance
                    // of precisely 28 — so retail refuses the claim
                    // and the port took it. The ball's `+144` is an
                    // UNGRADED lane, so no pair diff can see it; it
                    // surfaces one tick later through the mana census,
                    // which credits that ball's 512 to the rival's
                    // ceiling: `mana_max` retail 3048, port 3560.
                    if cast && facing < 28 {
                        self.g.ent[t].f144 = self.rivals[ri].ent;
                        // NO recolor at the claim — for the grave OR
                        // the ball. Retail's re-derive (sub_274D0)
                        // runs only in the ball's own MOVING arm
                        // (:29518-69), so a claimed settled ball keeps
                        // its stale row exactly like the claimed grave
                        // keeps sprite 65 (the invented intake recolor
                        // was the certified corpus's whole (10,39)
                        // type86 family — retail 52-family vs port
                        // 105+8·team across all ten takes).
                        self.g.snd(4, t); // the claim chime (:29444)
                        // The state STAYS Possess (:18250-56 writes
                        // no +415): the ball is now MINE, so the
                        // think-tick cascade re-picks past it (the
                        // ball pick's own-ball filter); until then
                        // the handler idles at the claimed sphere.
                    }
                    // The z-hover toward ball + 512 runs on EVERY
                    // arrived tick, cast or no cast (:18258-63).
                    self.rival_hover_toward(i, tz.saturating_add(512));
                }
            }
            // Castle raid (sub_13CA0 :18271-92): the cast attempt AND
            // the hover both live inside the arrived + think-period
            // gate; a fired cast tick does not hover.
            AiState::RaidCastle => {
                let t = self.rivals[ri].target as usize;
                let (tx, ty, tz) = {
                    let e = &self.g.ent[t];
                    (e.x, e.y, e.z)
                };
                self.rival_face_target(i, tx, ty, tz);
                if self.rival_approach(ri, i, tx, ty, Some(tz), 2048, 3584) && think {
                    let fired = match self.rival_attack_pick(ri, false) {
                        Some(s) => self.rival_cast(ri, i, s),
                        None => false,
                    };
                    if !fired {
                        self.rival_hover_toward(i, tz.saturating_add(512));
                    }
                }
            }
            // Wizard / balloon / mana-holder attack (sub_13DD0
            // :18314-40): the cast attempt runs ONLY when ARRIVED
            // (inside 3072) with the burst lockout clear — retail
            // returns before the pick otherwise (the l2 corpus wall:
            // the port fired every tick from 7300 units out while
            // retail held a saturated charge meter) — and the z-hover
            // toward target + 512 runs only when the attempt FAILED.
            AiState::AttackWizard | AiState::RaidBalloon | AiState::HuntMana => {
                let (tx, ty, tz) = match self.rivals[ri].target {
                    PLAYER_TARGET => self.human_pose,
                    t => {
                        let e = &self.g.ent[t as usize];
                        (e.x, e.y, e.z)
                    }
                };
                self.rival_face_target(i, tx, ty, tz);
                if self.rival_approach(ri, i, tx, ty, Some(tz), 3072, 4096)
                    && self.rivals[ri].burst >= 0
                {
                    let fired = match self.rival_attack_pick(ri, true) {
                        Some(s) => self.rival_cast(ri, i, s),
                        None => false,
                    };
                    if fired {
                        // Landing a cast clears MY war flag toward the
                        // struck record's TEAM ROW (:18337-39). ⭐ The
                        // gate is the target's `+65 <= 1` and NOTHING
                        // ELSE — no class test exists in sub_13DD0
                        // (CARPET.EXE 0x2c683, see
                        // [`war_clear_is_model_only`]), so a model-0
                        // or model-1 CREATURE discharges the grudge
                        // too, and its NULL wizext makes the index the
                        // low-memory constant 0 — the human's row.
                        let target = self.rivals[ri].target;
                        let model_only = war_clear_is_model_only();
                        let hit =
                            target == PLAYER_TARGET
                                || self.g.ent.get(target as usize).is_some_and(|e| {
                                    e.model65 <= 1 && (model_only || e.class64 == 3)
                                });
                        if hit {
                            // A carpet answers with its own wizext's
                            // `+48`; a record with no wizext reads
                            // through NULL and lands on row 0.
                            let row = match (self.owner_slot(target), model_only) {
                                (Some(o), _) => Some(o),
                                (None, true) => Some(0),
                                (None, false) => None,
                            };
                            if let Some(o) = row {
                                self.rivals[ri].war[o as usize] = false;
                            }
                        }
                    } else {
                        self.rival_hover_toward(i, tz.saturating_add(512));
                    }
                }
            }
            // Home (sub_13A70 :18204-27): cloak while fleeing; the
            // teleport-home attempt is authentically dead code.
            AiState::Home => {
                // ⭐ The handler resolves its destination off the
                // wizext+50 REGISTER (:18208 `164 * wizext[+50]`, an
                // index test alone) — the same read as the two
                // selector arms that enter Home — and then gates on
                // the SIGNATURE (:18224 `sub_15440(a1, v1)`: sig(reg)
                // == +148) before aiming. See
                // [`crate::engine::features::no_mc1_home_castle_register`].
                let reg_read = !crate::engine::features::no_mc1_home_castle_register();
                let Some(c) = self.rival_castle_reg(ri) else {
                    // Castle-less Home (:18209-19): cloak + the Cruise
                    // speed logic, and the state STAYS Home — the
                    // cascade is what moves it on.
                    self.rival_cast(ri, i, 12);
                    self.rival_cruise_speed(ri, i);
                    return;
                };
                let (cx, cy) = (self.g.ent[c].x, self.g.ent[c].y);
                self.rival_cast(ri, i, 12);
                if reg_read && self.target_sig(c as u16) != self.rivals[ri].target_sig {
                    // :18224-25 — a stale signature returns with no
                    // aim and no speed write.
                    return;
                }
                let cz = self.g.ent[c].z;
                self.rival_approach(ri, i, cx, cy, Some(cz), 256, 2048);
                // ⭐ NO state write on a full purse of life. The whole
                // of sub_13A70 (CARPET.EXE 0x13A70-0x13B94) never
                // touches `+415`, `+40` or `+8`: the healed rival keeps
                // AIMING (+34 at 0x13B16) and approaching its castle
                // every tick until the think-tick cascade re-picks
                // (Idle's cruise leg, :18756). See
                // [`home_keeps_state`].
                if !home_keeps_state() && self.g.ent[i].act_life >= self.g.ent[i].max_life as i32 {
                    self.rivals[ri].state = AiState::Fresh;
                }
            }
            // Cruise (sub_13A10 :18188).
            AiState::Cruise => {
                self.rival_cruise_speed(ri, i);
            }
        }
    }

    /// THE SPEED REGISTER, READ BLIND (round 167). `sub_14E60` (remc1hw
    /// sub_main.cpp:16905) resolves `pool[wizext+676+2*spell]` on an
    /// index test alone, and the `+676` rebuild re-points an entry at
    /// whatever record a recycled acquisition slot now holds. Every
    /// reader then takes that record's words as a token's: `sub_15E60`
    /// its `+48`, `sub_15A00` case 2 its `+136` as the PRICE, `sub_155F0`
    /// case 2 its `+50` as the reload. [`Self::rival_token`] validates
    /// the binding and answered "no token", which sent the travel
    /// helper to its plain-throttle leg — and that leg sets `v_14`,
    /// which kills the rival's REAL burst at the token's next pass.
    ///
    /// WITNESS mc1hwl14 t=13453: slot 733, a stale entry of wizard 5's
    /// acquisition list, is minted as a `(10,2)` contrail; the rebuild
    /// sets `owned[2]` 130 -> 733. The contrail's `+48`, `+50` and
    /// `+136` are all 0, so retail's brain "casts" through it every
    /// tick (`cooldown[2]` re-armed to 32 on each of them, `v_14`
    /// standing at 0) while the real token 130 (`+48` 246, 245, …)
    /// runs its burst out: speed 160, regen pinned. The port throttled,
    /// killed the burst at 13454 and paid the regen — 45 graded heads.
    ///
    /// Returns the record when the register names one that is NOT this
    /// rival's own token. `MGC_NO_MC1_RIVAL_BLIND_SPEED=1` restores the
    /// validated read.
    fn rival_blind_speed_reg(&self, ri: usize) -> Option<usize> {
        if crate::engine::features::no_mc1_rival_blind_speed() {
            return None;
        }
        let m = self.rivals[ri].owned[2] as usize;
        (m != 0 && m < self.g.ent.len() && self.rival_token(ri, 2).is_none()).then_some(m)
    }

    /// The three blind reads of [`Self::rival_blind_speed_reg`], in
    /// retail's order. `far` is the caller's boost-distance test
    /// (`sub_15470` :17199; the Cruise twin has none). `true` = the
    /// helper returns here, speed columns and `v_14` untouched.
    fn rival_blind_speed(&mut self, ri: usize, i: usize, b: usize, far: bool) -> bool {
        let word48 = self.mc1_token_word48(b);
        if word48 > 0 {
            return true; // sub_15E60 — "a burst is running"
        }
        // sub_15A00 case 2: the purse against the RECORD's `+136`.
        if !far || (self.rivals[ri].mana as i32) < self.g.ent[b].f136 {
            return false;
        }
        // sub_155F0: readiness passed, so `+17 &= ~1` lands; case 2
        // then refuses on a NONZERO `+48` and reloads otherwise.
        self.g.ent[i].flags &= !0x100;
        if word48 == 0 {
            let reload = self.g.ent[b].f50;
            if self.g.ent[b].class64 == 12 {
                self.g.ent[b].f26 = reload;
            } else {
                self.g.ent[b].raw48 = crate::engine::features::Raw48(reload as u16);
            }
            self.rivals[ri].cooldown[2] = AI_RECAST[2];
        }
        true
    }

    /// The Cruise speed logic (sub_13A10 :18188-203, shared by the
    /// castle-less Home arm sub_13A70 :18213-22): an ACTIVE speed
    /// burst owns the speed columns (sub_15E60's +48 test — vdes
    /// untouched); else the AI chain-casts the speed-up whenever
    /// ready, else full throttle.
    ///
    /// ⚠ NEITHER twin CLEARS `v_14` — only `sub_15470` does (:19057).
    /// The plain-throttle leg SETS it (:18198 / :18221), so a rival
    /// cruising on this arm re-arms the latch every tick it is not
    /// boosting, and the moment it does cast the burst the arm stops
    /// running entirely (the `sub_15E60` early return) and the latch
    /// keeps whatever the cast tick left.
    fn rival_cruise_speed(&mut self, ri: usize, i: usize) {
        if let Some(b) = self.rival_blind_speed_reg(ri) {
            if !self.rival_blind_speed(ri, i, b, true) {
                self.rivals[ri].vdes = self.g.ent[i].f128;
                self.rivals[ri].v14 = true;
            }
            return;
        }
        if self
            .rival_token(ri, 2)
            .is_some_and(|m| self.g.ent[m].f26 > 0)
        {
            return;
        }
        if self.rival_cast_ready(ri, 2) {
            self.rival_cast(ri, i, 2);
        } else {
            self.rivals[ri].vdes = self.g.ent[i].f128;
            self.rivals[ri].v14 = true;
        }
    }

    /// Shared travel helper (sub_15470 :19050-94): inside arriveR →
    /// stop, done. The distance is FULL 3D against an entity target
    /// (sub_42340: isqrt(dx²+dy²+dz²) — a wizard hovering high above
    /// a ground creature is NOT arrived; the 2-D read was the l2
    /// machine-gun wall's second half) and 2-D against a bare SITE
    /// (the a2==0 branch's sub_423D0 on +150). Beyond it, an ACTIVE
    /// speed burst owns the speed columns (:19063 sub_15E60 — return
    /// with vdes UNTOUCHED, the token machine is driving); a
    /// boost-cast tick returns the same way; only the plain leg
    /// writes vdes = f128. Returns "arrived". (Retail's callers stamp
    /// +34 themselves; the fold here matches every live call site.)
    #[allow(clippy::too_many_arguments)]
    fn rival_approach(
        &mut self,
        ri: usize,
        i: usize,
        tx: u16,
        ty: u16,
        tz: Option<i16>,
        arrive: i32,
        boost: i32,
    ) -> bool {
        let (px, py, pz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        // The speed-column latch clears at the HEAD (:19057), before
        // any leg decides — so a tick that returns through the
        // boost-active or boost-cast arms leaves it CLEAR and the
        // running burst survives.
        self.rivals[ri].v14 = false;
        // Retail compares the TRUNCATED scalar distance, never the
        // square: sub_15470 tests `sub_42340(...) > a3` (:19058-62)
        // and `> a4` (:19066), and both helpers end in the isqrt
        // (:52724 / :52744 — their squared-only twins sub_42390 /
        // sub_42410 exist and are deliberately NOT the ones called
        // here). The two forms differ across the whole band
        // arrive² < d² < (arrive+1)², where the square test refuses
        // but the isqrt truncates onto the boundary and ARRIVES —
        // mc1l2 t=1824/1895: Vodor at d² = 9,437,778 against
        // 3072² = 9,437,184 is 594 over on squares, exactly 3072 on
        // the isqrt, and retail casts.
        let d = {
            let dh = Gen::dist2_sq(px, py, tx, ty);
            let sum = match tz {
                Some(z) => {
                    let dz = z.wrapping_sub(pz) as i32;
                    dh.wrapping_add(dz.wrapping_mul(dz))
                }
                None => dh,
            };
            Gen::isqrt(sum as u32) as i32
        };
        self.g.ent[i].f34 = Gen::angle_between(px, py, tx, ty);
        if d <= arrive {
            self.rivals[ri].vdes = 0;
            self.rivals[ri].v14 = true; // :19075-76 — the arrival stop
            return true;
        }
        if let Some(b) = self.rival_blind_speed_reg(ri) {
            if !self.rival_blind_speed(ri, i, b, d > boost) {
                self.rivals[ri].vdes = self.g.ent[i].f128;
                self.rivals[ri].v14 = true;
            }
            return false;
        }
        if self
            .rival_token(ri, 2)
            .is_some_and(|m| self.g.ent[m].f26 > 0)
        {
            return false;
        }
        if d > boost && self.rival_cast_ready(ri, 2) {
            self.rival_cast(ri, i, 2);
            return false;
        }
        self.rivals[ri].vdes = self.g.ent[i].f128;
        self.rivals[ri].v14 = true; // :19089-91 — the plain throttle
        false
    }

    /// Aim the body at the target (desired yaw; the commit pitch is
    /// set at cast time, :19125-27).
    fn rival_face_target(&mut self, i: usize, tx: u16, ty: u16, _tz: i16) {
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        self.g.ent[i].f34 = Gen::angle_between(px, py, tx, ty);
    }

    /// Per-state altitude nudge toward target z + 512: `z +=
    /// sign(z − tz) · row.v_14` verbatim (:18258-63 / :18328-32 /
    /// :18287-91) — v_14 is the NEGATIVE settle step, so above sinks
    /// and below climbs; a zero row steps nothing.
    fn rival_hover_toward(&mut self, i: usize, tz: i16) {
        let v14 = BEHAVIOR[self.g.ent[i].row156 as usize].v_14;
        let e = &mut self.g.ent[i];
        let d = e.z as i32 - tz as i32;
        e.z = (e.z as i32 + d.signum() * v14 as i32) as i16;
    }

    /// The attack-spell picker (sub_16030 :19459 / castle variant
    /// sub_16310 :19559): poverty latch, then the priority walk
    /// 17 → 8 → (anti-rebound 15) → 7 → 20 → 0 → 15. Returns the
    /// spell to cast now; None = hold (save up or poor).
    pub(crate) fn rival_attack_pick(&mut self, ri: usize, vs_wizard: bool) -> Option<usize> {
        // Poverty latch (:19468-91): latch under max/4; release the
        // tick mana REACHES the threshold (min(max/4 + 6000, max/2)
        // — retail's `>` tests are on the still-poor side, so the
        // boundary itself releases; the port's old strict `>` held
        // one extra tick, which under the +100/tick floor pushed
        // every early-Vodor fireball one tick late).
        // ⭐ SIGNED (CARPET.EXE 0x16036 `sar`/`jle`, twin 0x16316): on
        // the fatal tick the purse is wrapped negative and retail
        // LATCHES — see
        // [`crate::engine::features::no_mc1_rival_poverty_signed_purse`].
        if !crate::engine::features::no_mc1_rival_poverty_signed_purse() {
            let r = &mut self.rivals[ri];
            let (mana, max) = (r.mana as i32, r.mana_max as i32);
            let quarter = max / 4;
            if quarter > mana {
                r.poverty = true;
            } else if r.poverty {
                let v3 = quarter.wrapping_add(6000);
                let still_poor = if v3 >= max { max / 2 > mana } else { v3 > mana };
                if !still_poor {
                    r.poverty = false;
                }
            }
            if r.poverty {
                return None;
            }
        } else {
            let r = &mut self.rivals[ri];
            let quarter = r.mana_max / 4;
            if r.mana < quarter {
                r.poverty = true;
            } else if r.poverty {
                let v3 = quarter + 6000;
                let still_poor = if v3 >= r.mana_max {
                    r.mana_max / 2 > r.mana
                } else {
                    v3 > r.mana
                };
                if !still_poor {
                    r.poverty = false;
                }
            }
            if r.poverty {
                return None;
            }
        }
        // Anti-rebound notice (:19507-16): the target visibly
        // rebounding switches the plan to lightning, acc% of the
        // time — and that success path ENDS the walk: 15-when-ready
        // or hold, never falling through to 7/20/0 (:19517-31; the
        // 7/20/0/15 ladder is the roll's ELSE arm). The notice and
        // its draw live in [`World::rival_rebound_roll`].
        let mut lightning_plan = false;
        let mut planned = false;
        // ⭐⭐ THE DRAW SITS AT `LABEL_49`, BEHIND THE 17/8 ARMS
        // (:19492-19506). Retail walks Undead Army (17) and Volcano (8)
        // FIRST and `return`s out of the picker the moment one is
        // ready or holds — `rand()` is never reached on those ticks.
        // The port used to roll BEFORE the walk, so every 17/8 tick
        // burnt a draw retail never spends and the global CRT stream
        // ran ahead of retail's by an unbounded, state-dependent
        // amount. See [`mc1_crt_draw_at_label49`].
        if !mc1_crt_draw_at_label49() {
            lightning_plan = self.rival_rebound_roll(ri, vs_wizard);
            planned = true;
        }
        for s in [17usize, 8] {
            match self.rival_arm(ri, s, false) {
                ArmStep::Next => {}
                ArmStep::Stop(v) => return v,
            }
        }
        // LABEL_49 (:19506-08) — the anti-rebound notice and its roll.
        if !planned {
            lightning_plan = self.rival_rebound_roll(ri, vs_wizard);
        }
        // The roll's SUCCESS path ends the walk at 15 (:19509-16):
        // 15-when-ready or hold, never falling through to 7/20/0.
        if lightning_plan {
            return match self.rival_arm(ri, 15, true) {
                ArmStep::Next => None,
                ArmStep::Stop(v) => v,
            };
        }
        for s in [7usize, 20, 0, 15] {
            match self.rival_arm(ri, s, false) {
                ArmStep::Next => {}
                ArmStep::Stop(v) => return v,
            }
        }
        None
    }

    /// ONE ARM of the picker cascade (`sub_16030`'s repeated
    /// owned → ready → wait-or-continue triple, e.g. :19492-98 for
    /// spell 17): `Next` = retail's `goto LABEL_5x`, `Stop(Some(s))` =
    /// its `return s`, `Stop(None)` = its fall-out-to-`return -1` hold.
    ///
    /// `plan15` is the anti-rebound SUCCESS arm (:19509-16), whose
    /// only outcomes are cast-15 and hold — it has no fall-through.
    fn rival_arm(&mut self, ri: usize, s: usize, plan15: bool) -> ArmStep {
        let blind = !crate::engine::features::no_mc1_rival_blind_register();
        let reg = self.rival_reg(ri, s);
        if if blind {
            reg.is_none()
        } else {
            self.rivals[ri].owned[s] == 0
        } {
            return ArmStep::Next;
        }
        if self.rival_cast_ready(ri, s) {
            return ArmStep::Stop(Some(s));
        }
        if plan15 {
            return ArmStep::Stop(None); // the plan holds for the bolt (:19525-29)
        }
        // WAIT-vs-continue discriminant (sub_15E90 :19497): fall
        // through to the next spell when this one is unaffordable by
        // ceiling OR on its recast cooldown; only HOLD (save mana /
        // settle the aim) when it's affordable-by-ceiling, off
        // cooldown, and merely short on current mana or unaimed.
        // (The cooldown escape is what lets a just-fired — or
        // castle-fizzled — high-priority spell yield to a cheaper
        // castle-free one like Fireball while it recharges.)
        // ⭐ The ceiling is held against the REGISTERED RECORD's `+136`
        // (`sub_15E90` :17510, signed `<=`) — see [`Self::rival_reg`].
        let over_ceiling = match reg {
            Some(m) if blind => self.g.ent[m].f136 > self.rivals[ri].mana_max as i32,
            _ => self.rivals[ri].mana_max < self.spells()[s].possess_mana,
        };
        if over_ceiling || self.rivals[ri].cooldown[s] != 0 {
            return ArmStep::Next;
        }
        ArmStep::Stop(None)
    }

    /// `sub_16000(target, 14) && rand() % 255 < acc` (:19507-08) — the
    /// anti-rebound notice. `&&` SHORT-CIRCUITS, so the global Watcom
    /// stream advances ONLY when the target holds a live Rebound token;
    /// that is why MC1's one CRT draw site has no steady phase drift.
    ///
    /// The notice is a LIVE read of the TARGET's spell-14 TOKEN
    /// record's +48 (sub_16000 :19449-56 — pool[wizext+676+2*14],
    /// `+48 > 0`), taken at the CASTER's own dispatch — not the
    /// carpet's 0x8000 mirror. Walk order is the law: a token
    /// arming below the reader is seen the same tick (mc1l49
    /// t=3560, token 600 < reader 620), and the owner-slot mirror
    /// refresh misses a token above its owner (t=4003, token 704
    /// > owner 698). The import re-homes class-12 +48 into f26.
    ///
    /// The roll is CRT `rand() % 255`, the global stream, NOT the
    /// wizard's own entity LCG — burning ent_rand here stole a
    /// graded-lane draw (mc1l49 t=2788).
    fn rival_rebound_roll(&mut self, ri: usize, vs_wizard: bool) -> bool {
        // `sub_16000` :17585-87 — the register's record, its `+48` word
        // (see [`Self::rival_reg`]; `f26` is `+26` off class 12).
        let blind = !crate::engine::features::no_mc1_rival_blind_register();
        let token_live = |m: u16| {
            m != 0
                && (m as usize) < self.g.ent.len()
                && if blind {
                    (m as i16) > 0 && self.mc1_token_word48(m as usize) > 0
                } else {
                    self.g.ent[m as usize].f26 > 0
                }
        };
        let target_rebounds = vs_wizard
            && match self.rivals[ri].target {
                PLAYER_TARGET => token_live(self.player.owned[14]),
                t => self
                    .rivals
                    .iter()
                    .find(|r| r.ent == t)
                    .is_some_and(|r| token_live(r.owned[14])),
            };
        if !target_rebounds {
            return false;
        }
        let roll = (self.g.watcom_rand() % 255) as u16;
        let acc = self.rivals[ri].acc;
        if crt_trace() {
            eprintln!(
                "[crt] t={} ri={ri} ent={} acc={acc} roll={roll} pass={}",
                crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                self.rivals[ri].ent,
                u8::from(roll < acc),
            );
        }
        roll < acc
    }

    // ---- the cast arm (readiness sub_15A00 :19219 + executor
    // ---- sub_155F0 :19096) ------------------------------------------

    /// Readiness (`sub_15A00` :19219): owned, not busy, cooldown clear,
    /// CURRENT mana covers the cost, and (for the aimed groups) the
    /// accuracy-scaled aim cone. The castle-stored unlock ladder is
    /// deliberately NOT here — retail's readiness has no castle term
    /// (verified in `sub_15A00`); the ladder is enforced downstream at
    /// emission ([`World::rival_cast`], mirroring retail's projectile-tick
    /// fizzle `sub_55DD0` :65049). Folding it into readiness froze rivals: a
    /// castle-tier spell they own but can't unlock (no big castle) reads as
    /// affordable-by-ceiling forever, so the picker parked on it and never
    /// fell through to Fireball.
    // `&mut` because the castle arm's space test (sub_12D10 :19315)
    // stamps the castle box as a side effect, in retail too.
    fn rival_cast_ready(&mut self, ri: usize, s: usize) -> bool {
        let r = &self.rivals[ri];
        let m = r.owned[s] as usize;
        if m == 0 {
            return false;
        }
        // ⭐ The register is an index and the record behind it answers
        // for the price and the busy word — see [`Self::rival_reg`].
        // Castle keeps its own price reader.
        let blind = if s == 16 || crate::engine::features::no_mc1_rival_blind_register() {
            None
        } else {
            let Some(b) = self.rival_reg(ri, s) else {
                return false; // `sub_14E60` answered 0
            };
            Some(b)
        };
        // The recast cooldown gates every case EXCEPT Accelerate —
        // sub_15A00's case 2 tests token + mana only (:19260-63);
        // its cadence comes from the burst window (the commit's +48
        // test), and the armed-but-unread AI_RECAST[2]=32 would have
        // starved retail's chain-cast Cruise boosts.
        if s != 2 && r.cooldown[s] != 0 {
            return false;
        }
        let def = &self.spells()[s];
        // Spell 16 prices through the LIVE manifestation cache
        // (sub_15A00 case 0x10 :19332 reads the token's +136, same
        // stamp as the want gate) — 1000 ctor, CAP[lvl] housed, 5000
        // after a raze. Every other spell is the static table cost.
        let cost = if s == 16 {
            self.rival_castle_price(ri)
        } else {
            def.possess_mana
        };
        // ⭐ SIGNED (`jl`, CARPET.EXE 0x15B8C and every sibling case):
        // a purse the fatal tick wrapped negative refuses — see
        // [`crate::engine::features::no_mc1_rival_cast_signed_purse`].
        let short = if let Some(b) = blind {
            (r.mana as i32) < self.g.ent[b].f136
        } else if crate::engine::features::no_mc1_rival_cast_signed_purse() {
            r.mana < cost
        } else {
            (r.mana as i32) < cost as i32
        };
        if short {
            return false;
        }
        // ALREADY-ACTIVE gate: a token still carrying burst (+48, our
        // f26) is NOT ready. Retail runs it for the self-buff group
        // (case 4/0xC/0xE :19289-96), for the AIMED group (case
        // 3/7/8/0x11/0x14 :19265-68) and Castle (:19305) — but NOT
        // for the fireball group (case 0/0xB/0xD/0xF), whose bolts
        // re-arm mid-burst freely. Accelerate's lives in the COMMIT
        // (sub_155F0 case 2 :19151), mirrored in `rival_cast`.
        // Blind, the word is tested NONZERO (`cmpw $0x0,0x30(%eax)` /
        // `jne`, HIDDEN.EXE file 0x2e45e).
        if matches!(s, 3 | 4 | 7 | 8 | 12 | 14 | 17 | 20)
            && match blind {
                Some(b) => self.mc1_token_word48(b) != 0,
                None => self
                    .rival_token(ri, s)
                    .is_some_and(|m| self.g.ent[m].f26 > 0),
            }
        {
            return false;
        }
        // Aimed groups: the readiness pre-gate cone ((255-acc)/4+20
        // degrees, :19252-57) — between the ACTUAL heading and the
        // DESIRED one (+30 vs +34, the state handler's stamp), not a
        // recomputed target bearing, and `>=` refuses.
        if matches!(s, 0 | 3 | 7 | 8 | 11 | 13 | 15 | 17 | 20) {
            let cone = ((255 - r.acc as u32) / 4 + 20) * 2048 / 360;
            let e = &self.g.ent[r.ent as usize];
            if Gen::angdist(e.f30, e.f34 & 0x7FF) as u32 >= cone {
                return false;
            }
        }
        // ⭐⭐ Castle (case 0x10 :19304-42): the BOUND arm alone
        // re-tests the upgrade SPACE (`sub_12D10` on the wizext+50
        // castle) and the SAME accuracy cone as the aimed groups;
        // the free-plant arm (:19343-47) is cooldown + mana only.
        // mc1l5 t=13646: Vodor settles into Upgrade over his rebuilt
        // keep with an aim error of 154 against a cone of ~130 —
        // retail hovers and re-aims (charge climbing 125→126) where
        // the port's coneless commit armed the token and fired the
        // castle ball retail never cast (the t=13647 extra
        // (9,10)/(10,43) pair).
        // ⭐ The bound arm's gate is the raw register word (:19309
        // `if (wizext+50)`), not the first-commit latch — see
        // [`rival_castle_register`].
        let bound = if rival_castle_register() {
            let reg = self.wiz_castle_reg(r.slot) as usize;
            (reg != 0 && reg < self.g.ent.len()).then_some(reg)
        } else {
            self.rival_castle(r.ent)
                .filter(|&c| self.g.ent[c].flags & 2 != 0)
        };
        if s == 16
            && let Some(c) = bound
        {
            let space = self.g.castle_upgrade_space_ok(c);
            let cone = ((255 - r.acc as u32) / 4 + 20) * 2048 / 360;
            let e = &self.g.ent[r.ent as usize];
            let aim = Gen::angdist(e.f30, e.f34 & 0x7FF) as u32;
            if std::env::var_os("MGC_RIVAL_TRACE").is_some() {
                eprintln!(
                    "[cast16 t={}] castle={c} space={space} aim={aim} cone={cone}",
                    crate::DEBUG_TICK.load(std::sync::atomic::Ordering::Relaxed),
                );
            }
            if !space || aim >= cone {
                return false;
            }
        }
        true
    }

    /// The commit (sub_155F0 :19096-215): arm the cooldown, aim the
    /// pitch at the target, run the burst counter, debit the mana
    /// through the delta, and emit through the shared spawners.
    /// Returns true when the cast fired. Spells 18/19/21/22/23 hit
    /// the original's default case — the AI can never cast them.
    fn rival_cast(&mut self, ri: usize, i: usize, s: usize) -> bool {
        if s >= SPELL_COUNT || matches!(s, 18 | 19 | 21 | 22 | 23) {
            return false;
        }
        if !self.rival_cast_ready(ri, s) {
            return false;
        }
        let def = &self.spells()[s];
        // Group gates beyond readiness (:19113-19209).
        let (tx, ty, tz) = match self.rivals[ri].target {
            0 => {
                let e = &self.g.ent[i];
                let mut fwd = (e.x, e.y, e.z);
                Gen::polar_step(&mut fwd, e.f30, 0, 4096);
                fwd
            }
            PLAYER_TARGET => self.human_pose,
            t => {
                let e = &self.g.ent[t as usize];
                (e.x, e.y, e.z)
            }
        };
        let (ex, ey, ez, yaw, des) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.f30, e.f34)
        };
        // The commit clears the entity's 0x100 bit whenever readiness
        // passed (:19110, `+17 &= ~1`), before any case gate.
        self.g.ent[i].flags &= !0x100;
        // Commit cones compare the ACTUAL heading against the DESIRED
        // one (+30 vs +34), `>=` refusing — not a recomputed target
        // bearing (:19120-23 / :19163-66).
        match s {
            // Precision-aimed burst pair (:19113-37).
            0 | 15 => {
                if self.rivals[ri].burst < 0 || Gen::angdist(yaw, des) >= 0xAA {
                    return false;
                }
                self.rivals[ri].burst += 1;
                if self.rivals[ri].burst >= 8 {
                    // Negative lockout (:19129-36).
                    self.rivals[ri].burst = ((self.rivals[ri].tempo as i32 - 255) / 8 - 1) as i16;
                }
            }
            // Aimed group (:19158-77): the wider cone.
            3 | 7 | 8 | 11 | 13 | 17 | 20 if Gen::angdist(yaw, des) >= 0xE3 => {
                return false;
            }
            // Accelerate (:19151): the busy gate lives HERE, not in
            // readiness — a live burst refuses the re-commit.
            2 if self
                .rival_token(ri, 2)
                .is_some_and(|m| self.g.ent[m].f26 > 0) =>
            {
                return false;
            }
            _ => {}
        }
        // Castle (0x10): with a castle → the upgrade chain; without →
        // the free direct plant at the site (:19190-209).
        if s == 16 {
            return self.rival_cast_castle(ri, i);
        }
        // Arm the re-attempt cooldown FIRST — retail's sub_155F0 sets it
        // regardless of the castle outcome, and the picker's cooldown
        // escape (sub_15E90 :19497) relies on it to advance past this
        // spell next tick.
        self.rivals[ri].cooldown[s] = AI_RECAST[s];
        // Absolute aim pitch to the target (:19125-27 / :19168-71):
        // the commit stamps the WIZARD's own +32 — the token-side
        // spawner reads the pose (and the corpus grades the column).
        //
        // ⭐ THIS SITS ABOVE THE CASTLE BAIL BECAUSE RETAIL HAS NO
        // CASTLE TEST IN `sub_155F0` AT ALL. Both commit arms run
        // cooldown → +32 → token `+48 = +50` with nothing between
        // (:19124-27 and :19172-76); the unlock ladder is the
        // PROJECTILE's own first tick (:65049), 40k lines away. The
        // collapse below is a legitimate shortcut for the emission,
        // but it inherited a stamp that retail performs BEFORE the
        // fizzle can matter — so a rival whose castle stores nothing
        // stopped aiming altogether. mc1hwl0-noskip t=359: rival 1's
        // castle (slot 522) holds `f140` 0 against Lightning's
        // castle_req, and its `+32` froze at the imported value for
        // the whole take.
        let dh = Gen::isqrt(Gen::dist2_sq(ex, ey, tx, ty) as u32) as i32;
        let pitch = Gen::pitch_toward(ez, tz, dh);
        if matches!(s, 0 | 3 | 7 | 8 | 11 | 13 | 15 | 17 | 20) {
            self.g.ent[i].f32 = pitch;
        }
        // ⭐⭐⭐ THE CASTLE-STORED LADDER IS NOT A COMMIT GATE — IT IS
        // THE TOKEN'S OWN (`sub_55DD0` :64917-19, reached from the
        // token handler `sub_56090` :65030-88, and separately from the
        // projectile's first tick :65049). `sub_155F0` runs
        // cooldown → `+32` → `+48 = +50` with NOTHING between
        // (:19124-27, :19172-76), so a rival whose castle stores
        // nothing still ARMS, and the burst dies one tick later when
        // the token's gate refuses and drops it to 1 for the shared
        // decrement. The port used to bail here instead, which ate
        // the arm exactly as an earlier version of the same bail ate
        // the `+32` stamp above — ⭐ *a collapse inherits every write
        // retail does before the point it collapsed to, and that
        // ledger has to be re-read every time the collapse moves*.
        // mc1hwl0 t=2425: rival 1's Wall of Fire token 256 reads
        // `+48` 0 → **26** → 0 across t=2425/2426/2427, a one-tick
        // spike the collapsed port flattened to a permanent 0 (98
        // rows over 62 pairs). Its castle 522 stores 4,490 against
        // HW spell 20's 60,000 requirement — it never fires, and
        // retail still counts.
        // The refusal stays SILENT for the AI (retail's buzz 29 is
        // the local player's channel and would storm at Lightning's
        // 1-tick recast).
        //
        // The commit only ARMS the token (+48 = +50, through the
        // VALIDATED binding) — the bolt, the sub_55E80 debit and the
        // mid-burst regen freeze all run at the TOKEN's own pool slot
        // ([`World::rival_manifestation_tick`], retail's sub_56090
        // machine). A token below the caster fires next pass, above
        // it the same tick — retail's phase for free.
        // ⭐ Blind, the arm is `+48 = +50` on whatever record the
        // register names — see [`Self::rival_reg`].
        if crate::engine::features::no_mc1_rival_blind_register() {
            if let Some(m) = self.rival_token(ri, s) {
                self.g.ent[m].f26 = def.count as i16;
            }
        } else if let Some(m) = self.rival_reg(ri, s) {
            self.rival_reg_arm(m);
        }
        let _ = (ex, ey, ez, yaw);
        true
    }

    /// Rival castle cast (:19190-209).
    fn rival_cast_castle(&mut self, ri: usize, i: usize) -> bool {
        // The commit's ONE gate for both arms (:19191-93): the token
        // exists, is NOT busy (`+48 != 0` — a cast in transit, the
        // charge pin), and the wizard's CURRENT mana covers the live
        // ladder price (wiz +140 vs the token's +136 stamp). Retail
        // has NO space test at the commit — that's the selector's
        // (:18408); and the recast cooldown is the UPGRADE arm's
        // alone (:19197), the free plant leaves it untouched.
        // ⭐ `sub_14E60` IS THE INDEX, NOT A VALIDATOR — see
        // [`castle_token_index`]. Retail's commit takes whatever
        // record `+676[16]` names, owner byte and all.
        let resolved = if castle_token_index() {
            let m = self.rivals[ri].owned[16] as usize;
            (m != 0 && m < self.g.ent.len()).then_some(m)
        } else {
            self.rival_token(ri, 16)
        };
        let Some(m) = resolved else {
            return false;
        };
        // The purse test is SIGNED (`jl`, CARPET.EXE 0x1582E) — see
        // [`crate::engine::features::no_mc1_rival_cast_signed_purse`].
        let (purse, price) = (self.rivals[ri].mana, self.rival_castle_price(ri));
        let short = if crate::engine::features::no_mc1_rival_cast_signed_purse() {
            purse < price
        } else {
            (purse as i32) < price as i32
        };
        if self.g.ent[m].f26 != 0 || short {
            return false;
        }
        // :19194 `if (wizext+50)` — the raw register word.
        if self.rival_castle_reg(ri).is_some() {
            // Established castle → THE UPGRADE CHAIN (:19196-97): arm
            // the token (+48 = +50); the debit, the (9,10) castle
            // ball and its (10,43) upgrade-token ride all run at the
            // token's own slot ([`Self::rival_castle_token_tick`],
            // retail's sub_57610 machine) — the corpus-refuted ch5
            // shortcut is retired (mc1l5 t=5152).
            self.g.ent[m].f26 = self.spells()[16].count as i16;
            self.rivals[ri].cooldown[16] = AI_RECAST[16];
            return true;
        }
        // Castle-less: the FREE direct plant at the scouted site
        // (:19200-08) — no debit, no projectile. ⚠ NO (0,0) sentinel:
        // the site is a supercell-corner value and (0,0) is a LEGAL
        // one — mc1l5 t=14771, Vodor's post-raze rebuild scouts
        // exactly (0,0) and retail plants castle 478 off it (the
        // port's invented empty-site test refused the plant forever).
        let (sx, sy) = self.rivals[ri].site;
        let gz = self.g.ground_z(sx, sy) as i16;
        let Some(c) = self.g.spawn_class3(2, sx, sy, gz) else {
            return false;
        };
        // The planted castle's recorded birth row (mc1l5 t=14771,
        // slot 478): state 5 (TRANSFORM — it rises through the level
        // machine, whose first commit re-binds +50 idempotently),
        // sprite 177 FLAT (no per-slot offset; its art extents are
        // the recorded 184 — the owner's team color lands one tick
        // later, at the first level-up commit's latch stamp,
        // :56057-62), ceiling 0 (the establish tick prices it — the
        // ctor stamps nothing).
        {
            let e = &mut self.g.ent[c];
            e.id24 = self.rivals[ri].ent;
            e.f26 = 0;
            e.tick70 = 5;
        }
        // The plant BINDS at spawn (:19206 writes wizext+50) — the
        // one bind site that precedes any level-up commit.
        self.g.castle_reg[self.rivals[ri].slot as usize] = c as u16;
        self.g.set_sprite(c, 177);
        // ⚠ NO terrain stamp: a level-0 castle is a BARE FLAG (BUILD
        // row 0 is empty, w = h = 0 — the teardown law's own guard).
        // The pad is painted by the LEVEL-UP commit's (10,42) painter
        // over the following ticks; an immediate stamp here raised
        // tile ground retail leaves flat (mc1l5 t=14772: the class-11
        // trigger volume at (128,128) rides its every-8th-tick ground
        // snap onto a mound retail never built).
        self.g.snd(30, c);
        self.entities_dirty = true;
        let _ = i;
        true
    }

    /// The per-spell emissions through the shared spawners, owner =
    /// the rival's entity slot (so friendly-fire immunity, homing and
    /// the damage plumbing all Just Work).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rival_emit(
        &mut self,
        ri: usize,
        i: usize,
        s: usize,
        x: u16,
        y: u16,
        z: i16,
        yaw: u16,
        pitch: u16,
    ) {
        let owner = self.rivals[ri].ent;
        let target = self.rivals[ri].target;
        let def = &self.spells()[s];
        let speed = self.g.ent[i].f126;
        let snd = match s {
            0 | 11 | 13 | 17 | 20 | 23 => Some(9u8),
            7 | 8 => Some(15),
            2 => Some(19),
            15 => Some(23),
            1 => Some(25),
            3 => Some(40),
            _ => None,
        };
        if let Some(id) = snd {
            self.g.snd(id, i);
        }
        let pr = match s {
            0 | 23 => self.g.spawn_fireball(x, y, z),
            3 => self.g.spawn_spell_lob(1, x, y, z),
            7 => self.g.spawn_trail_bolt(x, y, z),
            8 => self.g.spawn_spell_lob(4, x, y, z),
            11 => self.g.spawn_spell_lob(7, x, y, z),
            13 => self.g.spawn_seeker(x, y, z),
            15 => self.g.spawn_zigzag(x, y, z),
            17 => self.g.spawn_spell_lob(11, x, y, z),
            // The Wall-of-Fire token machine (sub_57D40 :66110-69) is
            // ONE function for human and AI — its full tick mints the
            // (9,16) firewall bolt itself. `spawn_spell_lob(9, ..)` has
            // no sprite arm and returned None: the rival's Wall of
            // Fire never produced a projectile (mc1l49 273 + mc1l48 81
            // missing-(9,16) heads).
            20 => self.g.spawn_firewall_bolt(x, y, z),
            // Self-buffs/channels have no projectile.
            _ => None,
        };
        let Some(pr) = pr else { return };
        let undead_44 = self
            .g
            .mc1_undead_bolt_44(self.rivals[ri].slot as usize, def.possess_mana);
        let e = &mut self.g.ent[pr];
        // Every retail emit arm adds the caster's speed to +126 ONLY
        // (:65237/:65956/:66143 …) — no emit site in the binary
        // writes +128, so the ctor's base survives and the flight
        // servo walks the launch boost back off (mc1l48 t=271 slot
        // 790: retail f126 462 / f128 384 one tick after birth).
        //
        // ⭐ …EXCEPT THE DUEL (s == 11). `sub_57040_57570` (:65620-72)
        // is the SAME token machine for the human and the AI, and it
        // is the one arm of the sixteen with no `+126 +=` line — see
        // [`crate::engine::world::mc1_duel_dart_exact_off`].
        if s != 11 || crate::engine::world::mc1_duel_dart_exact_off() {
            e.f126 += speed;
        }
        e.id24 = owner;
        e.f30 = yaw;
        e.f32 = pitch;
        // The aim pair ONLY. No token machine in the block writes
        // `+36` (grep `+ 36)` over :64900-66420: none) — the port's
        // `+36 = pitch` here was an invention, and on a lob the
        // flight never overwrites it: retail's possess lob carries
        // `+36 = 0` for its whole life (mc1l2 t=2380-91 slot 108,
        // port 242), the fireball for its birth boundary (t=2 slot
        // 197, port 1988) until the tracker's miss-arm mirror lands.
        // Round 153's `(9,1) f36` 108k free / 30 takes and `(9,0)
        // f36`. See [`no_mc1_rival_emit_pitch_mirror`].
        if no_mc1_rival_emit_pitch_mirror() {
            e.f36 = pitch;
        }
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        if s == 17 {
            // `sub_57800`'s own `+44` (:65962-71) — one machine for
            // the human and the AI.
            if let Some(v) = undead_44 {
                e.f44 = v;
            }
        }
        // +140 carries the per-burst-tick debit quantum (cost/count —
        // the token ctor's stamp, corpus: fireball token 200/5 = 40 on
        // the bolt), not the full one-shot cost.
        e.f140 = (def.possess_mana / (def.count as u32).max(1)) as i32;
        // NO +34 write and NO +146 pre-lock — retail's emission
        // (sub_56510 :65233-52) leaves the bolt's desired-yaw at the
        // ctor 0 and never writes the target: the bolt's own one-shot
        // muzzle acquisition (sub_54520 case 1, next tick — the bolt's
        // pool slot already ran this pass) picks the victim, for the
        // AI exactly as for the human. Pre-locking bypassed that scan
        // (no accidental house possession, no natural misses) and
        // faked a spawn-tick +34/+146 the corpus reads as 0. The
        // danger music arms at ACQUISITION (the class-9 machinery),
        // not at emission.
        let _ = target;
        match s {
            3 => {
                // The doubled extents live in the CTOR (sub_39A90
                // :45917), not here — see the human twin's note in
                // `cast_projectile` and [`Gen::spawn_spell_lob`].
                let e = &mut self.g.ent[pr];
                e.f68 = 10;
                e.f69 = 12;
                e.f66 = 10;
                e.f26 = 200;
            }
            7 => self.g.ent[pr].f69 = 17,
            // The duel dart's impact pair and aim point (:65654-55,
            // :65660-62) — the token machine stamps them for the AI
            // exactly as for the human, and the rival arm had
            // neither, so an AI duel dart flew with `+68`/`+69` 0 and
            // detonated into nothing.
            11 if !crate::engine::world::mc1_duel_dart_exact_off() => {
                // `*(v5+150) = *(v3+72)` / `*(v5+154) = *(v3+76)` is
                // the caster's RAW record position — `z` here is the
                // already-lifted muzzle height, so read the record.
                let mut d = {
                    let c = &self.g.ent[i];
                    (c.x, c.y, c.z)
                };
                Gen::polar_step(&mut d, yaw, pitch, 10240);
                let e = &mut self.g.ent[pr];
                e.f68 = 10;
                e.f69 = 26;
                e.dest_x = d.0;
                e.dest_y = d.1;
                e.site_z = d.2;
            }
            13 => {
                let e = &mut self.g.ent[pr];
                e.f44 = 2000;
                e.f69 = 25;
            }
            // Volcano 10/9 (:65460-61) and undead army 10/36
            // (:65953-54) — the token machine's own pair, the human
            // twin's note in `cast_projectile`.
            8 | 17 if !crate::engine::world::no_mc1_emit_detonation_pair() => {
                let e = &mut self.g.ent[pr];
                e.f68 = 10;
                e.f69 = if s == 8 { 9 } else { 36 };
            }
            15 => self.g.ent[pr].f69 = 23,
            // :66129-30 — the token machine's own f68/f69 stamp.
            20 => {
                let e = &mut self.g.ent[pr];
                e.f68 = 10;
                e.f69 = 53;
            }
            _ => {}
        }
        // The charge move — the AI's manifestations run the SAME
        // class-12 token machines as the human's (`str_2563D8`
        // :4957-5033, state = 3*spell), and FOURTEEN of them close
        // with `+26 = u8_326; u8_326 = 0`; possess (:65246) zeroes
        // without stamping and the duel (:65620-710) never reads the
        // meter at all. See [`rival_charge_family`].
        let ws = self.rivals[ri].slot as usize;
        let banks: bool = if rival_charge_family() {
            // ⭐ The FULL retail list — see [`rival_charge_family`].
            // Spell 16 (castle) banks in its own machine
            // (`rival_castle_commit`, :65910-11); 3 zeroes below.
            matches!(s, 0 | 6 | 7 | 8 | 9 | 13 | 15 | 17 | 18 | 19 | 20 | 22 | 23)
        } else {
            matches!(s, 0 | 7 | 8 | 20)
        };
        if banks {
            self.g.ent[pr].f26 = self.wiz_charge[ws] as i16;
            self.wiz_charge[ws] = 0;
        } else if s == 3 {
            // Possess zeroes WITHOUT stamping (:65246).
            self.wiz_charge[ws] = 0;
        }
        // The dest triple on every arm but the duel's (stamped above):
        // the SAME token machines the human runs, off the caster's
        // pool record as the walk holds it — `+72/+76` raw (not the
        // lifted muzzle `z`), projected along `+30/+32` (the `yaw` /
        // `pitch` this arm was handed ARE that record's pair). mc1l2
        // t=2 slot 197 / t=2380 slot 108. Tabled on
        // [`Gen::mc1_stamp_bolt_dest`].
        if s != 11 {
            let c = {
                let e = &self.g.ent[i];
                (e.x, e.y, e.z)
            };
            self.g.mc1_stamp_bolt_dest(pr, c, yaw, pitch, s);
        }
        self.entities_dirty = true;
    }

    // ---- mortality ------------------------------------------------------

    /// State 2 — the death fall (sub_45FC0 :55434-90): the shared
    /// death-drift mover first (sub_455D0 — stick lanes are zero for
    /// the AI, but the speed still chases the stale vdes 16/tick and
    /// the body drifts level), THEN gravity `z += OLD f46` with the
    /// decrement clamped into [−256, 0] after the add, the floor at
    /// ground + row.v_12, a (10,1) trail puff at the POST-drift
    /// PRE-gravity pose (flags |= 0x80 only, id24 = the faller), and
    /// the impact block exactly when z LANDED ON the floor.
    fn rival_death_fall(&mut self, ri: usize, i: usize) {
        {
            let vdes = self.rivals[ri].vdes;
            let e = &mut self.g.ent[i];
            e.f126 += 16 * (vdes - e.f126).signum();
        }
        let (yaw, speed, vz) = {
            let e = &self.g.ent[i];
            (e.f30, e.f126, e.f46)
        };
        let mut pos = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        // ⭐⭐⭐ THE SPEED-0 SINK (`sub_455D0` :55171-72). The shared
        // mover's vertical block ends in a bare `else` that no other
        // arm can reach: with the ACTUAL speed at zero and the body
        // still above `ground + row.v_10`, the SCRATCH drops 8 —
        // `if (!+126 && scratch.z > row156->v_10 + sub_11F50(scratch))
        // scratch.z -= 8;`. The human's mover has carried it since the
        // flight port (`flight.rs`, "Speed-0 sink above the soft
        // ceiling"), hard-coded at the carpet row's v_10 = 1024; the
        // RIVAL's state-2 fall runs the very same `sub_455D0` and the
        // port's copy skipped it, so a corpse whose `vdes` was already
        // 0 fell at pure gravity where retail sinks an extra 8/tick
        // until it drops through the band.
        //
        // mc1l49 slot 620 is the witness and it is the ROOT of that
        // take's biggest divergence family: retail's z runs 5777,
        // 5775, 5763, 5749 (−2, −12, −14) against the port's pure
        // −2/−4/−6 ladder, and the 8 lost units land the corpse — and
        // every jar, grave and free-stack pop its impact schedules —
        // on the wrong tick. `MGC_NO_DEATH_SINK=1` restores the
        // sink-less fall.
        if speed == 0 && death_sink_runs() {
            let g = self.g.ground_z(pos.0, pos.1) as i32;
            let v10 = BEHAVIOR[self.g.ent[i].row156 as usize].v_10 as i32;
            if (pos.2 as i32) > g + v10 {
                pos.2 -= 8;
            }
        }
        Gen::polar_step(&mut pos, yaw, 0, speed);
        // The strafe lane of the same shared mover (:55199-203): the
        // wizext's v_16 holds whatever jink residue the AI died with,
        // and NOTHING updates it during the fall — the dead brain
        // never runs its 4/tick decay — so the corpse sidesteps by
        // the SAME constant every fall tick until touchdown. mc1l3
        // t=1859: rival 585 falls with v_16 = 7 and the port's corpse
        // landed (−5,+5)/tick short of retail's, a residual every
        // (10,1) trail puff inherited at birth.
        {
            let jink = self.rivals[ri].jink;
            if jink != 0 {
                Gen::polar_step(&mut pos, yaw.wrapping_add(0x200) & 0x7FF, 0, jink);
            }
        }
        // The knock lane of the same shared mover (:55204-19). The AI's
        // live mover never runs it, so the killing blow's whole impulse
        // is still pending when the corpse enters state 2: retail's
        // body drifts along that bearing for ~10 ticks at 4/tick decay
        // (~180 units) — off whatever lip it was hovering over — and
        // only then meets the floor. Without it the port's corpse never
        // moved, so the very first fall tick clamped it onto the floor
        // and fired the impact 18 ticks early, sliding every later
        // allocation that tick down the free stack.
        // ⚠ The upper clamp is retail's (:55207-08); it has NO lower
        // clamp, so a negative magnitude would decay by +4 forever —
        // unreachable from the [0, 80] arm, and left as retail has it.
        {
            let r = &mut self.rivals[ri];
            if r.knock_mag != 0 {
                let mag = r.knock_mag.min(128);
                Gen::polar_step(&mut pos, r.knock_dir, 0, mag);
                let mut next = mag - mag.signum() * 4; // dword_93A94 = 4
                if next.abs() < 4 {
                    next = 0;
                }
                r.knock_mag = next;
            }
        }
        // sub_455D0 :55158-60 stamps the body's +32 from the control
        // block's u16_329 (HIBYTE &= 7), which is 0 for an AI.
        self.g.ent[i].f32 = 0;
        // The mover's wind-gust flutter (:55294-99) — every 64th tick
        // of the entity's OWN phase clock, one draw from its PRIVATE
        // LCG, 1-in-11 → sound 46. The live AI never runs sub_455D0,
        // so for a rival this fires only during the fall; the +63
        // read is PRE-increment (the walk clocks the record after the
        // handler). mc1l3 t=1890: the corpse's f63 crosses 192 and
        // retail's rand steps exactly once where the port's held.
        if self.g.ent[i].f63 & 0x3F == 0 {
            let roll = crate::engine::features::lcg32(&mut self.g.ent[i].rand);
            if roll % 11 == 0 {
                self.g.snd(46, i);
            }
        }
        // The mover's commit gate (:55250-52 → `sub_45410` :55065): a
        // scratch on a type-8 wall tile is refused and retried along
        // the two cardinals nearest the move bearing; both blocked ⇒
        // NO commit (the pose stands, the refused second cardinal
        // stays in the scratch). See [`rival_fall_wall_gate`].
        let cur = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        let committed = if rival_fall_wall_gate() {
            let (ok, slid) = self.g.player_wall_slide(cur, pos);
            pos = slid;
            ok
        } else {
            true
        };
        let v12 = BEHAVIOR[self.g.ent[i].row156 as usize].v_12;
        let floor = (self.g.ground_z(pos.0, pos.1) as i16).saturating_add(v12);
        // sub_455D0's own terrain keep-out lifts the moved body onto
        // the floor DURING the move — the fall's gravity + clamp
        // below only re-settle it — so a corpse drifting over RISING
        // ground already reads the lifted z when the trail spawns
        // (:51546 reads the mover's position scratch). mc1hwl0
        // t=20728: the landing tick's (10,1) puff 744 and the fire
        // it seeds are born at 4956 where the pre-lift z was 4955.
        // (:55103-05 — the gate's trailing z-floor is UNCONDITIONAL:
        // it lifts the scratch whether or not it commits.)
        if pos.2 < floor {
            pos.2 = floor;
        }
        let puff = pos;
        // Gravity (:55468-73) and the floor (:55474-77) act on the
        // ENTITY's pose (`a1 + 76`, `sub_11F50(a1 + 72)`): the scratch
        // when the gate committed it, the unmoved pose when it refused.
        let (mut body, floor) = if committed {
            (pos, floor)
        } else {
            (
                cur,
                (self.g.ground_z(cur.0, cur.1) as i16).saturating_add(v12),
            )
        };
        body.2 = body.2.saturating_add(vz);
        {
            let e = &mut self.g.ent[i];
            e.f46 = (vz - 2).clamp(-256, 0);
        }
        if body.2 < floor {
            body.2 = floor;
        }
        // The mover's commit relinked the body BEFORE the puff exists
        // (see [`fall_relink_first`]); `body` is final by here, so the
        // one relink carries the gravity and the floor with it.
        let first = fall_relink_first();
        if first {
            self.g.move_relink(i, body.0, body.1, body.2);
        }
        // The trail (10,1) burning puff (:55480-84) — at the SCRATCH.
        if let Some(s) = self.g.spawn_effect(1, puff.0, puff.1, puff.2) {
            self.g.ent[s].flags |= 0x80;
            self.g.ent[s].id24 = self.rivals[ri].ent;
        }
        if !first {
            self.g.move_relink(i, body.0, body.1, body.2);
        }
        if body.2 == floor {
            self.rival_death_impact(ri, i);
        }
        self.entities_dirty = true;
    }

    /// The impact block (:55488-568): kill credit, jar scatter, the
    /// grave, in-flight balls re-pointed, entity hidden, respawn
    /// timer armed.
    fn rival_death_impact(&mut self, ri: usize, i: usize) {
        // :55487 — the touchdown REBUILDS THE FREE LIST first, so the
        // grave and every jar it throws come off a freshly sorted
        // stack. The class-3 fall handler `sub_45FC0` is SHARED with
        // the human, so this is the same line `World::player_land`
        // carries; mc1l2 t=8297→8298 catches it on the rival side
        // (the record's free stack turns into the descending
        // 33, 32, 31 … 19 run with a 79-deep recycle stack, and the
        // next (10,40) lands on slot 18 against our 65).
        let pinned = self.mc1_carpet_slot;
        self.g.mc1_rebuild_free(pinned);
        // :43836-58 — sub_37220 rebuilds BOTH halves: the recycle
        // stack collects every live record with 0x20400 (descending
        // push => the LOWEST victim on top), and the landing leaves
        // it ARMED (no reset until the next reaper site), so an
        // exhausted pool's next allocation SACRIFICES a hut instead
        // of failing (mc1hwl0 t=25319: the rival respawn eats 20
        // village records for its re-minted book).
        self.g.rebuild_recycle(0x20400);
        // Kill credit (:55488-97): the killer wizard's tally.
        let killer = self.g.ent[i].f38;
        if let Some(k) = self.owner_slot_of_source(killer) {
            self.kill_tally[k as usize][self.rivals[ri].slot as usize] += 1;
            // ⚠ The rival kill does NOT feed the human's `+359`
            // creature counter: retail's :55488-97 writes the
            // per-victim `+30` tally above and nothing else, and
            // `sub_1A6C0`'s `+359++` is a class-5 death handoff a
            // wizard never runs (mc1l49 t=9641, retail 49 vs port
            // 50). The old "parity with the creature track" bump
            // survives only under the switch.
            if k == 0 && no_mc1_rival_kill_no_tally() {
                self.g.kills = self.g.kills.saturating_add(1);
            }
        }
        // Death message for the app ticker + toast (retail etext 54
        // = "has died." rendered "<Name> has died.", periods=100 —
        // :55499-517 + the drawType-0 sprintf :26518-33; NOT etext 56
        // "is dead", the wrong neighbor).
        let slot = self.rivals[ri].slot;
        self.rival_deaths.push(slot);
        let name = RIVAL_NAMES.get(slot as usize).copied().unwrap_or("?");
        self.set_notification(format!("{name} has died."), 100, [0xFF, 0, 0]);
        // :55518 `memset(a1 + 90, 0, 36)` — the touchdown wipes the
        // corpse's whole mailbox, the fatal letter the fall carried
        // included. The human's twin had the line
        // (`World::player_land`); the rival's arm did not. See
        // [`crate::engine::features::no_mc1_rival_landing_mail_clear`].
        if !crate::engine::features::no_mc1_rival_landing_mail_clear() {
            self.g.ent[i].mail = [(0, 0); 6];
        }
        // JAR SCATTER (:55519-49): every owned manifestation detaches
        // into a decaying ground jar around the corpse — iterated over
        // the +532 ACQUISITION LIST in PICKUP order, not the spell-id
        // book (mc1l4 t=6885: two tokens picked up out of spell order
        // swap their scatter draws under a book iteration). Each live
        // entry is rewritten to the token's MODEL number, each empty
        // one to −1, for the respawn re-grant's in-place refill.
        // The scatter anchors on the corpse's own +76 (:55537 copies
        // `*(WORD *)(a1 + 76)` into the position struct), i.e. the z
        // the fall just clamped onto the floor — not a fresh ground
        // sample. (Both readings coincide in the mc1l2 window, so this
        // is decompile authority, not a corpus-proven claim.)
        let (cx, cy, cz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        // ⚠ NO `owned` BLANK HERE. The scatter walks `+532` ONLY;
        // `+676` has a single writer in the whole binary and it is
        // `sub_45C10`, which the dispatch stops calling the moment
        // `+70` leaves the live arms — so the corpse's book FREEZES,
        // it does not clear. See [`owned_survives_scatter`].
        if !owned_survives_scatter() {
            self.rivals[ri].owned = [0; SPELL_COUNT];
        }
        for k in 0..SPELL_COUNT {
            let entry = self.rivals[ri].acq[k];
            if entry <= 0 || entry as usize >= self.g.ent.len() {
                self.rivals[ri].acq[k] = -1;
                continue;
            }
            let m = entry as usize;
            self.rivals[ri].acq[k] = self.g.ent[m].model65 as i32;
            // ⭐⭐⭐ THE ROSTER OVERFLOW (:55529-31, the two arms
            // `*(_BYTE *)(*(_DWORD *)(a1 + 160) + *(char *)(v16 + 65)
            // + 916) = 1 / = 0`; CARPET.EXE `mov byte
            // [edx+eax+0x394],1` at file 0x5ea06 and `…,0` at
            // 0x5ea1a, both off a FRESHLY RELOADED `[ebx+0xa0]`).
            //
            // The scatter records, per scattered token, whether it was
            // BURSTING — `Type_160.var_916[model] = (flags & 0x40000)
            // != 0` — for the respawn's re-mint to restore (:54908,
            // `if (var_916[model]) { +86 = 280; +132 = 0; flags.byte[2]
            // |= 4; }`). ⚠ `var_916` is TWENTY-FOUR bytes wide and the
            // index is the token's MODEL, a SIGNED char that runs well
            // past 24 — and `Type_160` is exactly 946 bytes at +1103
            // of a 2049-byte roster row, so `var_916[m]` for m >= 30
            // writes into the NEXT PLAYER'S ROW at byte `m - 30`:
            //
            //     1103 + 916 + m - 2049 = m - 30
            //
            // m = 39 is that row's `var_u8_13332_9` — the husk-watch
            // gate. A MANA BALL is class 10 MODEL 39, and a stale
            // acquisition entry can name one (the manifestation was
            // freed and its pool slot recycled into a ball), so a
            // dying wizard scatters "his" mana ball and silently
            // brands player `slot + 1` as HUMAN-DRIVEN, for good.
            //
            // mc1l49 is the witness and it is EXACT: at t=39094 rival
            // 594 (player 1) dies with `acq[3] = 26`, pool slot 26 is
            // (class 10, model 39) with `flags = 12` (bit 18 clear),
            // and the recorded roster shows player 2's byte 9 go
            // 1 -> 0 on that very tick and never move again — the ONLY
            // byte in the whole 2049-byte row to change. From then on
            // slot 620, player 2's husk, runs the killer watch every
            // tick to the end of the take: 15,532 `heading` + 14,660
            // `target_yaw` segment heads, 89% of the take's dirt.
            //
            // Only the m = 39 byte is modelled: the port has no other
            // roster byte for m in 30..=40 to land on, and m < 30 is
            // the in-bounds write of a register nothing else reads
            // (the port carries the burst restore on `Rival::acq`).
            // `MGC_NO_HUSK_WATCH=1` retires the whole law.
            if husk_watch_gate() && self.g.ent[m].model65 as i8 as i32 == 39 {
                let blue = self.g.ent[m].flags & crate::engine::world::BLUE_SPELL != 0;
                let next = self.rivals[ri].slot.wrapping_add(1);
                if let Some(t) = self.rivals.iter().position(|r| r.slot == next) {
                    self.rivals[t].human_driven = !blue;
                }
            }
            // The scatter draws ride the DYING WIZARD's own LCG
            // (:55563-70 — `a1+4`, three draws per jar), not the
            // jar's.
            let dx = (self.g.ent_rand(i) & 0x1FF) as i32 - 256;
            let dy = (self.g.ent_rand(i) & 0x1FF) as i32 - 256;
            let jx = (cx as i32 + dx) as u16;
            let jy = (cy as i32 + dy) as u16;
            let life = (self.g.ent_rand(i) % 90 + 200) as i16;
            {
                let e = &mut self.g.ent[m];
                // :55529-31 — the token is un-parked: flags bit 0
                // clears and the state byte INCREMENTS (`++*(v16+70)`,
                // :55535). Assigning the phase outright happened to
                // agree on mc1l2 only because both tokens sat at
                // phase 0.
                e.flags &= !1;
                // Strict-retail worlds (a conformance import) carry
                // RETAIL's class-12 encoding — a scattered jar is
                // spell*3 + 1 (a phase-1 world jar the strict pickup
                // poll serves), and its decay rides ACT_LIFE (the jar
                // tick's top, sub_55A40 :64755-61: nonzero counts
                // down, freed at zero; authored jars carry 0 and sit
                // forever). The native encoding keeps its own f26
                // countdown.
                if self.strict_retail {
                    e.tick70 = e.tick70.wrapping_add(1);
                    e.act_life = life as i32;
                    // :55529-49 writes NO +48: a live burst's leftover
                    // counter rides the jar verbatim (mc1hwl0 t=20728,
                    // the dying rival's accel token scatters with
                    // +48 = 220 still aboard).
                } else {
                    e.tick70 = crate::engine::world::DROPPED_JAR; // pickup-able, decaying
                    e.f26 = life; // the decay countdown
                }
                // ⛔ NO `+144` WRITE — see
                // [`crate::engine::features::no_mc1_scatter_jar_owner_clear`]:
                // the scatter loop (:55534-49, CARPET.EXE
                // `0x5E9B8..0x5EAE4`) never stores to `0x90`. The old
                // "free copy" clear cost the HUMAN 7,000 of ceiling at
                // mc1l49 t=39094, where this rival's book still named
                // pool slot 26 — his own token long gone, the slot now
                // the human's live `(10,39)` ball.
                if crate::engine::features::no_mc1_scatter_jar_owner_clear() {
                    e.f144 = 0; // no owner — a free copy
                }
            }
            // MOVE_RELINK (:55546 `sub_41C70_41FB0`), not the bare
            // link: `Gen::link` early-returns on flags bit 2, which an
            // imported parked token carries, so the scattered position
            // was silently never written and every jar stayed parked.
            self.g.move_relink(m, jx, jy, cz);
        }
        // The grave (10,40) + in-flight ball re-point (:55550-65).
        // The spawn axis is the CORPSE'S OWN `+72` (:55550 passes
        // `a1 + 72` straight into the creator), i.e. the z the death
        // fall clamped onto the ground+128 floor — not a fresh ground
        // sample. mc1l2 t=8298 reads the rival's grave at 1198 with
        // the ground under it at 1070, exactly one floor clearance
        // apart. (The human's own landing already spawns at
        // `player.z`, `World::player_land`.)
        if let Some(gv) = self.g.spawn_grave(cx, cy, cz) {
            let me = self.rivals[ri].ent;
            // ⭐⭐⭐ THE RE-POINT WALKS THE TICK-TOP BALL ROSTER, NOT
            // THE POOL (`var_u32_36462[1]`, HIDDEN VA 0x46681
            // `mov 0x8e72(%ebp),%ebp`) — so a SEIZING tick, which
            // blanks every roster head, re-points NOTHING. See
            // [`crate::engine::features::no_mc1_grave_repoint_chain`]
            // for the bytes and the mc1hwl2 t=15762 witness. Retail's
            // only member tests are `model65 == 39` and `+144 ==
            // corpse`: membership is the class gate and there is no
            // reap test, so a slot the tick recycled is read on its
            // NEW bytes.
            if crate::engine::features::no_mc1_grave_repoint_chain() {
                for j in 1..self.g.ent.len() {
                    let e = &self.g.ent[j];
                    if e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0 && e.f144 == me {
                        self.g.ent[j].f144 = gv as u16;
                    }
                }
            } else {
                for k in 0..self.g.ball_chain.visible_len() {
                    let j = self.g.ball_chain.list[k] as usize;
                    if self.g.ent[j].model65 == 39 && self.g.ent[j].f144 == me {
                        // +144 only — retail's death sweep (sub_275C0
                        // :29633-40) re-points and never re-derives;
                        // the stale team row rides until the ball's
                        // next moving tick names the grave (class
                        // 10 ≠ 3 → neutral).
                        self.g.ent[j].f144 = gv as u16;
                    }
                }
            }
        }
        // Hidden (flag 0x20, :55568) + state 3 + the respawn timer
        // (:55552-57): 32*((255-tempo)/8)+32 ticks.
        {
            let e = &mut self.g.ent[i];
            e.tick70 = 3;
            // :55568 is a bare `|= 0x20` — the hittable bit 3 is NOT
            // cleared here (retail's corpse goes 12 → 44, keeping it).
            e.flags |= 0x20;
            e.f26 = (32 * ((255 - self.rivals[ri].tempo as i32) / 8) + 32) as i16;
        }
        // ⭐⭐⭐ THE LANDING DISCARDS THE TOP VICTIM CELL. The very
        // last statement of the fall handler is a BARE decrement of
        // the recycle stack's top index — `--*(_DWORD *)(result +
        // 4593)` (hw :51637), which the shipped binaries carry
        // verbatim as `decl 0x11f1(%eax)` at HIDDEN 0x466DF and
        // CARPET 0x4639F. It is the ONLY bare `--top` in either image
        // (every other writer is the rebuild, the seizure's own
        // post-decrement, or a full `movl $-1` disarm), and it is
        // unconditional on the landing path.
        //
        // It is a pure DISCARD, not an allocation: no killer call, no
        // class wipe — the dropped record stays alive, it just stops
        // being the next victim. So this is `Vec::pop`, never
        // [`Gen::mc2_recycle_pop`] (which skips invalid cells, counts
        // a seizure, and hands the caller a record to kill).
        //
        // Position is load-bearing: retail runs it AFTER the grave
        // spawn (0x4663A → 0x466DF), so on a dry free stack the grave
        // seizes first and this discards whatever is left on top.
        //
        // mc1hwl0 t=31881 measures the omission exactly — the port's
        // victim stack was retail's stack plus ONE trailing cell
        // (len 515 vs 514, extra slot 16 on top), which shifted all
        // 54 of t=31888's seizures by one slot. That extra cell was
        // never a predicate error: under the descending scan slot 36
        // is pushed before slot 16, so 36 is present in BOTH stacks
        // and 16 simply sits above retail's top. Retail's index can
        // reach −2 here when the rebuild found no victims; the
        // seizure guard is `>= 0`, so popping an empty Vec is exact.
        self.g.mc2_recycle.stack.pop();
        // Post-death truce: everyone's hate toward this slot decays
        // from the elevated baseline once it respawns (:55037-41 —
        // set at re-init).
        self.entities_dirty = true;
    }

    /// `sub_463B0_466F0` (:55575-91) — THE WATCH AIM of a wizard whose
    /// state-3 handler took the non-respawning arm. Verbatim:
    ///
    /// ```text
    ///   v1 = &pool[*(u16*)(a1+38)].pos            ; +38 = the KILLER slot
    ///   *(a1+34) = sub_42150(a1+72, v1)           ; bearing to the watched record
    ///   *(a1+36) = sub_42180(a1+72, v1)           ; pitch  to the watched record
    ///   *(a1+30) += sub_422A0(*(a1+30), *(a1+34), 5, 0x16)
    ///   *(a1+31) &= 7                             ; i.e. +30 &= 0x7FF
    ///   *(a1+32) += sub_422A0(*(a1+32), *(a1+36), 5, 0x16)
    ///   *(a1+33) &= 7
    ///   *(a1+32) = 0                              ; …and then thrown away
    /// ```
    ///
    /// The `a3 = 5` argument of `sub_422A0` is DEAD (:52689 ignores
    /// it); the cap is `a4 = 0x16` = 22/tick, the same rate-limited
    /// step [`Gen::turn_step`] already models. The `+32` leg computes
    /// a pitch step and immediately overwrites it with 0 — retail's
    /// own dead store, kept.
    ///
    /// mc1l49 slot 620 (Vodor's husk) is the corpus witness: from its
    /// elimination to the end of the take retail re-aims `+34` at the
    /// human (slot 569) every single tick and walks `+30` after it,
    /// where the port's eliminated rival froze — 15,532 `heading` and
    /// 14,660 `target_yaw` segment heads, 89% of the take's dirt.
    /// t=64902 is the arithmetic check: the husk at (43761, 11714,
    /// 4768) against the human at (44529, 2779, 386) gives bearing 27
    /// and pitch 148, and retail holds exactly `f34 = 27, f36 = 148`.
    pub(crate) fn rival_watch_track(&mut self, i: usize) {
        let w = self.g.ent[i].f38;
        // The import re-homes the human's pool slot onto
        // `PLAYER_TARGET`, so the killer tag has to resolve back
        // through `human_pose` — RAW, with no liveness gate, exactly
        // like retail's bare `pool[+38].pos` read (the same law
        // `raid_owner_pos_is_raw` carries). mc1l49's husks watch the
        // human, so this arm IS the corpus witness.
        let (tx, ty, tz) = if w == PLAYER_TARGET {
            self.human_pose
        } else if let Some(t) = self.g.ent.get(w as usize) {
            (t.x, t.y, t.z)
        } else {
            return;
        };
        let (px, py, pz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        let yaw = Gen::angle_between(px, py, tx, ty);
        let dh = Gen::isqrt(Gen::dist2_sq(px, py, tx, ty) as u32) as i32;
        let pitch = Gen::pitch_toward(pz, tz, dh);
        let e = &mut self.g.ent[i];
        e.f34 = yaw;
        e.f36 = pitch;
        let step = Gen::turn_step(e.f30, e.f34, 22);
        e.f30 = e.f30.wrapping_add(step as u16) & 0x7FF;
        // The +32 leg's step is computed and discarded (:55586-88).
        e.f32 = 0;
    }

    /// State 3 — dead on the ground (sub_46480 :55594): with a
    /// castle, count down and re-init; castle-less = ELIMINATED
    /// (checked every tick — losing the castle during the wait
    /// counts, :55622).
    fn rival_dead_wait(&mut self, ri: usize, i: usize) {
        // ⭐ THE GATE COMES FIRST (:55605). The castle test, the
        // respawn countdown and the castle-less `13329 = 0` all live
        // INSIDE the `== 1` arm; a row the death scatter has branded
        // HUMAN-DRIVEN skips the lot and takes the ELSE arm — view
        // mode 7 (a presentation register the port has no seat for)
        // plus the killer watch — for the rest of the level, castle
        // or no castle. See [`husk_watch_gate`].
        if husk_watch_gate() && self.rivals[ri].human_driven {
            self.rival_watch_track(i);
            return;
        }
        // :55609 `if (wizext+50)` — the raw register word: a register
        // naming a re-minted slot still respawns the rival.
        if self.rival_castle_reg(ri).is_none() {
            // The FINAL-death broadcast (retail etext 62 via the
            // opcode-0x1D elimination arm, :48812-25: "<Name> has
            // been eliminated from the realm.", periods=100 — MC1
            // says "eliminated" where MC2 says "banished"). Once, on
            // the elimination edge.
            if !self.rivals[ri].eliminated {
                let slot = self.rivals[ri].slot;
                let name = RIVAL_NAMES.get(slot as usize).copied().unwrap_or("?");
                self.set_notification(
                    format!("{name} has been eliminated from the realm."),
                    100,
                    [0xFF, 0, 0],
                );
            }
            self.rivals[ri].eliminated = true;
            // The husk stays hidden and inert; property persists
            // (:55622). The WATCH AIM is NOT part of this arm — it is
            // the ELSE arm's, gated above.
            return;
        }
        if self.g.ent[i].f26 > 0 {
            self.g.ent[i].f26 -= 1;
            return;
        }
        self.rival_respawn(ri, i);
    }

    /// Re-init at the castle (sub_44D30 respawn arm :54857-64 +
    /// :55019-50): teleport to the castle, full life, base mana,
    /// grace 100, re-mint the remembered book, brain reset, truce.
    fn rival_respawn(&mut self, ri: usize, i: usize) {
        // :54842 — `sub_44D30`'s first statement, whoever is
        // respawning: the same rebuild, so the re-minted book takes
        // the slots the scatter freed.
        let pinned = self.mc1_carpet_slot;
        self.g.mc1_rebuild_free(pinned);
        // The recycle half too (:43836-58) — the respawn's re-grant
        // loop allocates 24 times into a possibly-exhausted pool and
        // retail SACRIFICES 0x20400 victims ascending (hw:50910;
        // cleared again at the fn tail, hw:51124).
        self.g.rebuild_recycle(0x20400);
        // :54861-64 — the position is `str_29795[wizext+50]`'s, the
        // register read with no validation.
        let Some(c) = self.rival_castle_reg(ri) else {
            // Even the castle-less early exit disarms like retail's
            // fn tail (hw:51124 runs on every path).
            self.g.mc2_recycle.stack.clear();
            return;
        };
        // :54858-61 — the respawn copies the castle's WHOLE position,
        // z included (the human's mc1l42 t=17398 law, same sub_44D30):
        // the tile-up `ground + 256` was invented, and every re-minted
        // token betrayed it (mc1hwl0 t=20761, the whole book born at
        // 6432 against the castle's 6176). The fall's own +46/+126
        // registers are NOT cleared either — retail's respawn clears
        // exactly v_12, v_16 and the knock triple (:54868-83); the
        // boundary reads the corpse's vz −56 and speed 160 verbatim.
        let (cx, cy, cz) = (self.g.ent[c].x, self.g.ent[c].y, self.g.ent[c].z);
        {
            let e = &mut self.g.ent[i];
            e.flags = (e.flags & !0x20) | 8;
            e.tick70 = 1;
        }
        self.g.move_relink(i, cx, cy, cz);
        self.g.refill_life(i);
        // The mana CAPACITY resets to the base pool with the purse
        // (+136 = 1000 at the boundary; the castle keeps banking into
        // its own store, not the wizard's).
        self.g.ent[i].f136 = 1000;
        let ent = self.rivals[ri].ent;
        // The re-grant is LIST-driven (:54884-923), not book-driven:
        // each acquisition entry the death rewrote to a model number
        // re-mints that model IN PLACE (−1 entries reset to 0 and
        // skip), so the reborn book keeps the pickup order — and the
        // mint order, which decides which free-stack slots the fresh
        // tokens take. A scattered fireball's entry is 0 (model 0 ≡
        // the empty sentinel) and still re-mints: retail's collision,
        // kept deliberately.
        for k in 0..SPELL_COUNT {
            let entry = self.rivals[ri].acq[k];
            if entry < 0 || entry as usize >= SPELL_COUNT {
                self.rivals[ri].acq[k] = 0;
                continue;
            }
            let s = entry as usize;
            if let Some(m) = self.mint_manifestation(s, ent) {
                self.rivals[ri].acq[k] = m as i32;
                self.rivals[ri].owned[s] = m as u16;
            } else {
                self.rivals[ri].acq[k] = 0;
            }
        }
        // :54962 / :55033 — the respawn's OWN `sub_45C10` calls, the
        // memset-and-rebuild that publishes the reborn book. Retail
        // runs it twice (once before the `+676[16]` castle probe at
        // :54969, once before `sub_47DD0` at :55034) and both are the
        // same derivation, so one call here covers both. It is what
        // clears a STARVED entry's stale rung now that the scatter no
        // longer blanks the register ([`owned_survives_scatter`]):
        // the in-place writes above alone would leave the pre-death
        // slot standing for any spell whose re-mint found no record.
        if owned_survives_scatter() {
            self.rival_owned_rebuild(ri);
        }
        // :55034 — the respawn's own `sub_47DD0` call on the bound
        // castle: the FRESH Create-Castle token leaves the respawn
        // wearing the ladder price at the STANDING castle's level,
        // not the ctor row (the human's respawn carries the same law;
        // mc1hwl0 t=20761: (12,16) slot 97 = 20000/198 under castle
        // 233's level 2, against the minted 1000/9).
        {
            // :55034 `if (var_50) sub_47DD0(pool[var_50])` — the
            // REGISTER names the castle and the ladder prices every
            // level, 0 included (:56640 `case 0: 5000`). See
            // [`rival_castle_register`].
            let priced = if rival_castle_register() {
                let reg = self.wiz_castle_reg(self.rivals[ri].slot) as usize;
                (reg != 0 && reg < self.g.ent.len()).then_some(reg)
            } else {
                let e = &self.g.ent[c];
                (e.f26 > 0 && e.flags & 2 != 0).then_some(c)
            };
            if let Some(pc) = priced
                && !crate::engine::features::no_mc1_respawn_reprice_owner()
            {
                // `sub_47DD0` (:56617-73) prices the token of the
                // CASTLE'S OWNER — `pool[castle.+24]`, gated `+70 <=
                // 1u`, its `wizext+676[16]` index-tested only — at the
                // castle's `+26` through a switch whose `default:` is
                // 0, and divides by the TOKEN's own `+50`. See
                // [`crate::engine::features::no_mc1_respawn_reprice_owner`].
                self.mc1_respawn_reprice(pc);
            } else if let Some(pc) = priced {
                let e = &self.g.ent[pc];
                let cap = Gen::CASTLE_CAP[(e.f26.max(0) as usize).min(7)];
                let m = self.rivals[ri].owned[16] as usize;
                if m != 0 {
                    self.g.ent[m].f136 = cap;
                    self.g.ent[m].f140 = cap / 101;
                }
            }
        }
        {
            let r = &mut self.rivals[ri];
            r.mana = 1000;
            r.mana_max = 1000;
            // +132 is NOT cleared by the respawn: the pre-death debit
            // still pending in the delta seat applies to the fresh
            // 1000 purse on the first live tick (mc1hwl0 t=20762,
            // f140 1000 → 0 under the carried −1000).
            r.grace = 100;
            // `memset(u8_333, 16, 8)` (:55048 / hw:51122): the respawn
            // SETS the stall block to 16 — the shadow's post-respawn
            // stall=16 was the respawn's own stamp, not a survival.
            r.regen_stall = 16;
            r.state = AiState::Fresh;
            r.target = 0;
            // :54868-83 — the respawn clears EXACTLY three flight
            // registers: v_12 (target speed), v_16 (strafe/jink) and
            // the knock triple; +126 keeps servoing from the fall's
            // 160 (mc1hwl0 t=20762 reads 160 → 144).
            //
            // ⭐ THE KNOCK "TRIPLE" IS THREE WRITES AND THE PORT MADE
            // ONE. `sub_44D30` :54868-70 is `v_24 = 0; v_26 = 0;
            // v_22 = 0` — DIRECTION (+24) as well as MAGNITUDE (+22).
            // The port cleared only `knock_mag`, so a rival reborn
            // after a knocked death carried the dead body's bearing
            // in `+24` forever (mc1l49 t=21968, wiz 1: retail's +24
            // goes 1300 → 0, the port's stays 1300 for the rest of
            // the take). See [`respawn_clear_list`].
            r.vdes = 0;
            r.jink = 0;
            r.knock_mag = 0;
            if respawn_clear_list() {
                r.knock_dir = 0;
            }
            // ⭐ THE RESPAWN'S LEDGER RESET IS HATE ONLY. The tail's
            // rival-only block (:55043-46) is
            // `for (kx = 0; kx < 8; kx++) str_456[kx].var_u16_4 =
            // 24607` — `var_u16_4` is +460, the HATE half of the
            // 8-byte `str_456` row; the WAR flag at +462
            // (`var_u16_6`) is not in the loop and appears nowhere
            // else in `sub_44D30`. A reborn rival therefore keeps
            // every war he declared before he died, on a NEUTRAL hate
            // ledger — which is exactly the state the wizard picker's
            // rangeless war arm (:18886-90) reads.
            r.hate = [HATE_NEUTRAL; 8];
            if !respawn_clear_list() {
                r.war = [false; 8];
            }
            // ⭐ AND THE POVERTY LATCH (+406) SURVIVES TOO — the
            // respawn never writes it. It is the attack picker's own
            // register (:19468-91) and only the picker moves it, so a
            // wizard who died poor is reborn poor and holds his fire
            // until the release threshold. The port's `poverty =
            // false` was an INVENTED WRITE: with a 1000-mana purse
            // against a 61,634 cap the reborn rival is poor by
            // retail's own test on his first live tick anyway, but the
            // port's clear let one dispatch through before the latch
            // could re-arm.
            if !respawn_clear_list() {
                r.poverty = false;
            }
            // ⭐ THE BURST AND COOLDOWN REGISTERS SURVIVE THE RESPAWN:
            // sub_44D30 writes NO +48 burst and exactly ONE cooldown
            // entry — `var_756 = 4·slot` = cooldown[16] (both
            // binaries; the array base is +724). The port's blanket
            // clears let the reborn picker reach spells on a schedule
            // retail's carried cooldowns refuse, and 500 ticks after
            // the t=20761 respawn the drift finally forked a commit:
            // mc1hwl0 t=21275 the port committed Wall of Fire where
            // retail committed Fireball, so retail's t=21276 fireball
            // emission (the tick's FIRST allocation) never happened in
            // the port and every later spawn of the mass fire-spread
            // tick popped one free-stack slot late (the
            // missing-(10,0)-at-951 head).
            r.cooldown[16] = 4 * r.slot as u16;
        }
        // Re-seat the +140 mana mirror with the base pool.
        self.g.ent[i].f140 = 1000;
        // Everyone else's ledger toward the respawner: the elevated-
        // but-decaying truce value (:55037-41) — on the TICK-TOP
        // BUCKET-0 ROSTER, not the rival vector: a wizard still in
        // its own dead-wait is off `var_u32_36462[0]` and takes no
        // truce (mc1l14 t=1344). `MGC_NO_MC1_TRUCE_ROSTER=1` restores
        // the flat every-other-rival loop.
        let slot = self.rivals[ri].slot as usize;
        if no_mc1_truce_roster() {
            for (oj, o) in self.rivals.iter_mut().enumerate() {
                if oj != ri {
                    o.hate[slot] = HATE_RESPAWN;
                }
            }
        } else {
            let own = self.rivals[ri].ent;
            self.mc1_truce_roster(slot, own);
        }
        // hw:51124 — sub_44D30's tail disarms the recycle stack the
        // entry armed: only the respawn window itself may sacrifice.
        self.g.mc2_recycle.stack.clear();
        self.entities_dirty = true;
    }
}

// ------------------------------------------------------------ snapshot

use crate::snapshot::{Reader, Snap, SnapshotError, Writer, snap_enum};

snap_enum!(
    AiState,
    "AiState",
    0 => AiState::Fresh,
    1 => AiState::Upgrade,
    2 => AiState::Build,
    3 => AiState::Possess,
    4 => AiState::RaidCastle,
    5 => AiState::AttackWizard,
    6 => AiState::RaidBalloon,
    7 => AiState::HuntMana,
    8 => AiState::Home,
    9 => AiState::Cruise,
);

impl Snap for Rival {
    fn put(&self, w: &mut Writer) {
        let Rival {
            slot,
            ent,
            owned,
            known,
            allowed,
            learn,
            acq,
            cooldown,
            mana,
            mana_max,
            mana_delta,
            agg,
            acc,
            tempo,
            state,
            hate,
            war,
            burst,
            poverty,
            target,
            target_sig,
            site,
            jink,
            knock_dir,
            knock_mag,
            vdes,
            v14,
            grace,
            regen_stall,
            life_rate,
            eliminated,
            shield,
            invisible,
            rebound,
            human_driven,
        } = self;
        w.put(slot);
        w.put(ent);
        w.put(owned);
        w.put(acq);
        w.put(known);
        w.put(allowed);
        w.put(learn);
        w.put(cooldown);
        w.put(mana);
        w.put(mana_max);
        w.put(mana_delta);
        w.put(agg);
        w.put(acc);
        w.put(tempo);
        w.put(state);
        w.put(hate);
        w.put(war);
        w.put(burst);
        w.put(poverty);
        w.put(target);
        w.put(target_sig);
        w.put(site);
        w.put(jink);
        w.put(knock_dir);
        w.put(knock_mag);
        w.put(vdes);
        w.put(v14);
        w.put(grace);
        w.put(regen_stall);
        w.put(life_rate);
        w.put(eliminated);
        w.put(shield);
        w.put(invisible);
        w.put(rebound);
        // v20 joiner — the husk-watch roster byte (`var_u8_13332_9`).
        w.put(human_driven);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        Ok(Rival {
            slot: r.get()?,
            ent: r.get()?,
            owned: r.get()?,
            acq: r.get()?,
            known: r.get()?,
            allowed: r.get()?,
            learn: r.get()?,
            cooldown: r.get()?,
            mana: r.get()?,
            mana_max: r.get()?,
            mana_delta: r.get()?,
            agg: r.get()?,
            acc: r.get()?,
            tempo: r.get()?,
            state: r.get()?,
            hate: r.get()?,
            war: r.get()?,
            burst: r.get()?,
            poverty: r.get()?,
            target: r.get()?,
            target_sig: r.get()?,
            site: r.get()?,
            jink: r.get()?,
            knock_dir: r.get()?,
            knock_mag: r.get()?,
            vdes: r.get()?,
            v14: r.get()?,
            grace: r.get()?,
            regen_stall: r.get()?,
            life_rate: r.get()?,
            eliminated: r.get()?,
            shield: r.get()?,
            invisible: r.get()?,
            rebound: r.get()?,
            human_driven: r.get()?,
        })
    }
}

// ------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::features::{FeatureAssets, Planes};
    use crate::engine::world::{PlayerCommand, PlayerPose};
    use mgc_formats::{Thing, ThingKind};

    /// Diamond-ring SEARCH.DAT + a 4x4 building row — the same
    /// synthetic shape the world/feature unit tests use, so no baked
    /// tree is needed.
    fn assets() -> FeatureAssets {
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
        for row in 0..4 {
            dat.push(4u8);
            if row == 1 || row == 2 {
                dat.extend_from_slice(&[0x10, 7, 7, 0x10]);
            } else {
                dat.extend_from_slice(&[0x10, 0x10, 0x10, 0x10]);
            }
            dat.push(0);
        }
        FeatureAssets::parse(&grid, &tab, &dat).unwrap()
    }

    /// One rival at tile (120,120) with Fireball + Shield + Rebound +
    /// Castle in its book and a level-1 starting castle —
    /// CASTLE_CAP[1] = 10000 clears Rebound's 8000 castle_req, so the
    /// token is not fizzled by the stored-mana ladder.
    /// A flat world with one rival whose book holds POSSESS (spell 3)
    /// and nothing else — the claim-cone probe's scaffolding.
    fn possess_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // ⚖ BOTH seats are authored. Before round 161 this fixture
        // named only colour 1, and the HUMAN's marker-less seat then
        // landed at engine (0, 0) — 14 tiles from the sleeper at tile
        // (10, 10), i.e. INSIDE the wake radius, so the sleeper woke on
        // tick 1 and the law under test was unobservable. Every shipped
        // level authors a `(3,4)`; so does this one now.
        let marker = |model: u16, x: u16, y: u16| Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model,
            x,
            y,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let things = vec![marker(4, 100, 100), marker(5, 120, 120)];
        let mut w = World::new(planes, &things, 1, assets());
        let mut book = [false; SPELL_COUNT];
        book[3] = true;
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 255,
            castle_level: 0,
            book,
            allowed: book,
        });
        w.set_wizards(&cfgs, 2);
        w
    }

    /// ⚖ A MARKER-LESS WIZARD SEATS AT THE ORIGIN (round 154, the
    /// player's ruling "as faithful as possible"): `str_9177[colour]`
    /// is memset with the pool at level init (:51488) and `sub_44D30`
    /// reads it unconditionally (:54845), so a colour with no
    /// `(3,4+colour)` THING row is seated at (0, 0, ground + 0x100) —
    /// mc1l24's rival 4 (record 0: x 0 / y 65520 / z 258 after one
    /// settle tick). The port used to fall back to the HUMAN's marker.
    /// Under `MGC_NO_MC1_MARKERLESS_ORIGIN_SEAT=1` the fallback returns.
    #[test]
    fn a_marker_less_rival_is_seated_at_the_origin_like_retail() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Only the HUMAN's marker (3,4) is authored; colour 1 has none.
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 4,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        assert_eq!(w.start_markers[1], None, "colour 1 authors no marker");
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 255,
            castle_level: 0,
            book: [false; SPELL_COUNT],
            allowed: [false; SPELL_COUNT],
        });
        w.set_wizards(&cfgs, 2);
        let r = w
            .rivals
            .iter()
            .find(|r| r.slot == 1)
            .expect("rival 1 seated");
        let e = &w.g.ent[r.ent as usize];
        let ground = w.g.ground_z(0, 0) as i16;
        assert_eq!(
            (e.x, e.y),
            (0, 0),
            "the zeroed str_9177[1] seats the rival at the origin, not at the human's (120,120)"
        );
        assert_eq!(
            e.z,
            ground.wrapping_add(256),
            "ground + 0x100, sub_44D30 :54838-42"
        );
    }

    /// THE HUMAN TWIN of the test above (round 161, w161d) — ⭐⭐⭐ A
    /// LAW ON ONE CALL PATH IS NOT LANDED. Retail has ONE wizard
    /// (re)init routine for all eight colours, `sub_44D30` (:54802);
    /// the port splits it into [`World::mc1_spawn_human_record`] and
    /// [`World::spawn_rival`], and round 154 gave the origin seat to
    /// the rival arm alone, leaving the human arm's `else { return }`
    /// standing 120 lines away in this same file.
    ///
    /// `sub_40550` (:51488) `memset(&str_9177, 0, 48)` zeroes the
    /// whole 8x6-byte seat array with the pool, and :54845 reads
    /// `str_9177[wizard]` UNCONDITIONALLY before popping the class-3
    /// carpet on the `event == str_29795` arm — so a colour no THING
    /// row names is seated at engine (0, 0, ground + 0x100), human
    /// included.
    ///
    /// Only two levels in the whole baked MC1+HW corpus author no
    /// `(3,4)`: **mc1hw 005** (a populated campaign level that names
    /// seats 1..5 and forgets the human's — the witness,
    /// `recordings/mc1hwl5.mgcr`, where retail holds the human at
    /// slot 341, x 0 / y 0 / z 256) and **mc1hw 198** (an empty
    /// terrain-only arena). The mirror-image omission is mc1l24,
    /// which names 0,1,2,3,5 and forgets colour 4 — the rival test
    /// above.
    ///
    /// ⚠ THE KILL-SWITCH PROOF: this test FAILS under
    /// `MGC_NO_MC1_MARKERLESS_HUMAN_SEAT=1` (no carpet is pooled at
    /// all, so `mc1_carpet_slot` reads 0). It is deliberately the ONE
    /// place in the suite that is not A/B-neutral — every other
    /// fixture seats its own carpet so the switch measures this law
    /// and nothing else.
    #[test]
    fn a_marker_less_human_is_seated_at_the_origin_like_retail() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Colour 1 has a seat; the HUMAN (model 4) has none — mc1hw
        // level 5's exact shape, in miniature.
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 5,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let w = World::new(planes, &things, 1, assets());
        assert_eq!(w.start_markers[0], None, "the human authors no marker");
        assert_ne!(
            w.mc1_carpet_slot, 0,
            "the carpet is pooled anyway: :54845 reads str_9177[0] with no marker test"
        );
        let ground = w.g.ground_z(0, 0) as i16;
        assert_eq!(
            w.human_pose,
            (0, 0, ground.wrapping_add(256)),
            "the zeroed str_9177[0] seats the human at the ORIGIN (engine units, \
             not tile (0,0)'s centre), ground + 0x100 — not at colour 1's (120,120) \
             and not at the map centre"
        );
    }

    fn rebound_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 5,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let mut book = [false; SPELL_COUNT];
        book[0] = true;
        book[4] = true;
        book[14] = true;
        book[16] = true;
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(RivalConfig {
            aggression: 200,
            // tempo 255 → think period 1: the defense arm runs every tick.
            accuracy: 255,
            tempo: 255,
            castle_level: 2,
            book,
            allowed: book,
        });
        w.set_wizards(&cfgs, 2);
        w
    }

    fn away() -> PlayerPose {
        PlayerPose::from_tiles(10.0, 105.0 / 8.0, 10.0, 0.0, 0.0, 0.0)
    }

    /// THE RIVAL'S LEVEL-START BOOK IS MINTED IN BOOK ORDER (round
    /// 154, w154d; round 153 finding #3). `sub_3DD50`'s grant loop
    /// (:49213-54, CARPET.EXE 0x3DEF8 `mov 0x9b88(%esi),%al` …
    /// 0x3E03A `cmp $0x18,%esi`) walks `byte_99B88` for EVERY wizard
    /// and appends each granted spell to the `+532` acquisition list
    /// in THAT order; `sub_44D30`'s mint (:54882-905) then walks the
    /// list, so a rival holding {0, 1, 3, 16} lists and mints 0, 3,
    /// 16, 1 — Fireball, Possess, Castle, Heal — not 0, 1, 3, 16.
    /// mc1l49 record 0: seven rivals × 17 `owned` rows, the pool's
    /// slot census 119 slots off, and on mc1l20/mc1l22 the free stack
    /// under everything popped after the rivals. `init-check` mc1l49:
    /// wiz 154 rows → 35, slots 119 differ → 0.
    ///
    /// The list is the witness (slot numbering is the pool's
    /// business). `MGC_NO_MC1_RIVAL_BOOK_ORDER=1` must fail the
    /// order assert.
    #[test]
    fn the_rival_book_is_minted_in_book_order() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 5,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let mut book = [false; SPELL_COUNT];
        for s in [0, 1, 3, 16] {
            book[s] = true;
        }
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 255,
            castle_level: 0,
            book,
            allowed: book,
        });
        w.set_wizards(&cfgs, 2);
        let r = &w.rivals[0];
        let owned = |s: usize| r.owned[s] as i32;
        assert!(
            [0, 1, 3, 16].iter().all(|&s| owned(s) != 0),
            "all four minted: {:?}",
            r.owned
        );
        assert_eq!(
            &r.acq[..5],
            &[owned(0), owned(3), owned(16), owned(1), 0],
            "the acquisition list is filled in `byte_99B88` order (:49213-54)"
        );
        // The mint walks the list, so the pool slots follow it too
        // (a fresh pool pops ascending): 0 < 3 < 16 < 1.
        assert!(
            owned(0) < owned(3) && owned(3) < owned(16) && owned(16) < owned(1),
            "slots follow the walk: {:?}",
            &r.acq[..4]
        );
    }

    /// Round 167 ([`World::rival_blind_speed_reg`]): the rival's travel
    /// helper reads its speed register BLIND. With `owned[2]` naming a
    /// recycled non-token record (mc1hwl14 t=13453: a `(10,2)`
    /// contrail, `+48` / `+50` / `+136` all 0) retail's brain finds no
    /// burst, finds the "token" affordable at price 0, reloads its
    /// `+48` from its `+50` and re-arms `cooldown[2]` — and never
    /// reaches the plain-throttle leg that sets `v_14`.
    /// `MGC_NO_MC1_RIVAL_BLIND_SPEED=1` fails the first `v14` assert.
    #[test]
    fn the_travel_helper_reads_a_recycled_speed_register_blind() {
        let mut w = rebound_world();
        let i = w.rivals[0].ent as usize;
        let (x, y, z) = (w.g.ent[i].x, w.g.ent[i].y, w.g.ent[i].z);
        let puff = w.g.spawn_effect(2, x, y, z).expect("a contrail");
        assert_ne!(w.g.ent[puff].class64, 12, "non-vacuity: not a token");
        w.rivals[0].owned[2] = puff as u16;
        assert!(
            w.rival_token(0, 2).is_none(),
            "the validated read refuses it"
        );
        w.rivals[0].mana = 750; // under Accelerate's real 1000
        w.rivals[0].cooldown[2] = 0;
        w.rivals[0].vdes = 160;
        w.g.ent[i].flags |= 0x100;
        let far = (x.wrapping_add(8000), y);

        // Beyond the boost range: the blind cast.
        let arrived = w.rival_approach(0, i, far.0, far.1, Some(z), 1024, 3072);
        assert!(!arrived);
        assert!(!w.rivals[0].v14, "the throttle leg is never reached");
        assert_eq!(w.rivals[0].vdes, 160, "the speed column is untouched");
        assert_eq!(
            w.rivals[0].cooldown[2], AI_RECAST[2],
            "the commit's cooldown"
        );
        assert_eq!(w.g.ent[i].flags & 0x100, 0, "sub_155F0's `+17 &= ~1`");
        assert_eq!(w.mc1_token_word48(puff), 0, "`+48 = +50`, both 0");

        // A nonzero `+48` on the record reads as a running burst.
        w.g.ent[puff].raw48 = crate::engine::features::Raw48(5);
        w.rivals[0].cooldown[2] = 0;
        w.rival_approach(0, i, far.0, far.1, Some(z), 1024, 3072);
        assert!(!w.rivals[0].v14);
        assert_eq!(w.rivals[0].cooldown[2], 0, "sub_15E60 returns first");

        // Inside the boost range (and not arrived) retail throttles.
        w.g.ent[puff].raw48 = crate::engine::features::Raw48(0);
        let near = (x.wrapping_add(2000), y);
        w.rival_approach(0, i, near.0, near.1, Some(z), 1024, 3072);
        assert!(w.rivals[0].v14, "the plain throttle sets the latch");
        assert_eq!(w.rivals[0].vdes, w.g.ent[i].f128);

        // A record priced above the purse is not "ready" either.
        w.g.ent[puff].f136 = 751;
        w.rivals[0].cooldown[2] = 0;
        w.rival_approach(0, i, far.0, far.1, Some(z), 1024, 3072);
        assert!(w.rivals[0].v14, "unaffordable: throttle");
        assert_eq!(w.rivals[0].cooldown[2], 0);
    }

    /// Round 168 ([`World::rival_reg`]): the attack picker, the
    /// readiness and the commit read their register BLIND. With
    /// `owned[15]` naming a recycled non-token record (mc1hwl12
    /// t=16994: a `(5,15)` mob, `+48` / `+50` / `+136` all 0) retail
    /// prices "Lightning" at the record's `+136`, commits it on a purse
    /// the table price refuses, and arms the record's `+48` from its
    /// `+50`. `MGC_NO_MC1_RIVAL_BLIND_REGISTER=1` fails the first pick.
    #[test]
    fn the_attack_picker_prices_a_recycled_register_off_the_record() {
        let mut w = rebound_world();
        let i = w.rivals[0].ent as usize;
        let (x, y, z) = (w.g.ent[i].x, w.g.ent[i].y, w.g.ent[i].z);
        let puff = w.g.spawn_effect(2, x, y, z).expect("a stand-in record");
        assert_ne!(w.g.ent[puff].class64, 12, "non-vacuity: not a token");
        let table = w.spells()[15].possess_mana;
        assert!(table > 1, "non-vacuity: Lightning has a table price");
        w.rivals[0].owned[0] = 0;
        w.rivals[0].owned[15] = puff as u16;
        assert!(
            w.rival_token(0, 15).is_none(),
            "the validated read refuses it"
        );
        // A purse the table price refuses and the poverty latch passes.
        w.rivals[0].mana_max = table - 1;
        w.rivals[0].mana = table - 1;
        w.rivals[0].poverty = false;
        w.rivals[0].burst = 0;
        w.rivals[0].cooldown = [0; SPELL_COUNT];
        w.g.ent[i].f34 = w.g.ent[i].f30;

        // `+136 == 0`: the purse covers it, the picker returns 15.
        assert_eq!(
            w.rival_attack_pick(0, false),
            Some(15),
            "priced off the record"
        );
        assert!(w.rival_cast(0, i, 15), "and the commit fires");
        assert_eq!(
            w.rivals[0].cooldown[15], AI_RECAST[15],
            "the commit's cooldown"
        );
        assert_eq!(w.rivals[0].burst, 1, "the precision pair's burst counter");
        assert_eq!(w.mc1_token_word48(puff), 0, "`+48 = +50`, both 0");

        // The arm is `+48 = +50` on the record, whatever its class.
        w.rivals[0].cooldown[15] = 0;
        w.g.ent[puff].f50 = 7;
        assert!(w.rival_cast(0, i, 15));
        assert_eq!(w.mc1_token_word48(puff), 7, "the record's own reload");

        // A record priced over the purse but under the ceiling HOLDS
        // the walk (`sub_15E90` reads the same `+136`)…
        w.rivals[0].cooldown[15] = 0;
        w.rivals[0].owned[7] = puff as u16;
        w.g.ent[puff].raw48 = crate::engine::features::Raw48(0);
        w.g.ent[puff].f136 = table as i32 - 1;
        w.rivals[0].mana = table - 2;
        assert_eq!(
            w.rival_attack_pick(0, false),
            None,
            "7 holds: affordable by ceiling"
        );
        // …and one priced over the ceiling is walked past.
        w.g.ent[puff].f136 = table as i32;
        assert_eq!(
            w.rival_attack_pick(0, false),
            None,
            "nothing affordable at all"
        );
        w.g.ent[puff].f136 = 0;
        // The aimed group's busy word is the record's `+48`, nonzero.
        w.g.ent[puff].raw48 = crate::engine::features::Raw48(0xFFFF);
        assert!(
            !w.rival_cast_ready(0, 7),
            "a nonzero `+48` refuses the aimed group"
        );
        assert!(
            w.rival_cast_ready(0, 15),
            "the fireball group has no busy gate"
        );
    }

    /// ⭐⭐⭐ **RETAIL'S `+48` IS JUST A WORD IN A 164-BYTE RECORD, AND
    /// THE PORT'S SHADOW OF IT MUST BEHAVE LIKE MEMORY — NATIVELY,
    /// WITH NO IMPORT ANYWHERE IN SIGHT.**
    ///
    /// This is the instrument for a lane both graded channels are
    /// blind to (`+48` is not in `EntObsMc1`, and the replay harness
    /// re-imports the pool every segment, so `--segmented` cannot see
    /// any of the three transitions below). Every assert is a byte
    /// fact off `CARPET.EXE`, re-verified in wave 121 dig D13:
    ///
    /// * `sub_41E90` (file **0x5A688-0x5A6AE**) frees with the tile
    ///   unlink, `movb $0x0,0x40(%ebx)` and the free-stack push —
    ///   AND NOTHING ELSE. `+48` SURVIVES A FREE.
    /// * `NewEvent_372C0` (file **0x4FAB8**) merges its free-pop and
    ///   sacrifice arms at file **0x4FB5B** on
    ///   `push $0xa4 / push $0x0 / push %ebx / call 0x75428`, i.e.
    ///   `memset(record, 0, 164)`. A RE-TAKEN SLOT CARRIES NO
    ///   RESIDUE — retail reads 0 there, so the port must too.
    /// * `sub_14E60` (file 0x2D658) resolves `wizext+676+2*spell` as
    ///   a raw slot index with no class, flag or liveness guard, and
    ///   `sub_14120` (file 0x2C971) reads `cmpw $0x0,0x30(%eax)` off
    ///   whatever record now occupies it.
    #[test]
    fn the_token_word48_lane_behaves_like_retail_memory_natively() {
        let mut w = rebound_world();
        let m = w.rivals[0].owned[16] as usize;
        assert!(m != 0, "non-vacuity: the rival must own a Castle token");
        assert_eq!(w.g.ent[m].class64, 12, "…and it is a live manifestation");

        // 1. LIVE: `f26` is the port's home for a manifestation's
        //    `+48`, and the reader takes it from there.
        w.g.ent[m].f26 = 100;
        assert_eq!(w.mc1_token_word48(m), 100, "live token: the burst");

        // 2. FREED: retail clears the class byte alone, so the word
        //    stands. The pre-dig lane lost it natively — only the
        //    now-RETIRED `mc1_freed_token_burst` import seat put it
        //    back, which is exactly the seat hazard (round 98).
        w.g.free_entity(m);
        assert_eq!(w.g.ent[m].class64, 0, "the free clears the class byte");
        assert_eq!(
            w.mc1_token_word48(m),
            100,
            "freed token: `+48` survives `sub_41E90` (0x5A688)"
        );

        // 3. RE-TAKEN: the allocator memsets 164 bytes, so the new
        //    occupant's `+48` is ZERO — not the stranger's residue
        //    (the brief's premise) and not the stranger's `+26`
        //    (the pre-D2 port). D2's mc1l49 witness shape: the slot
        //    comes back as a bolt whose `+26` is 16.
        assert_eq!(w.g.new_event(), Some(m), "the free stack is LIFO");
        w.g.ent[m].class64 = 9;
        w.g.ent[m].model65 = 16;
        w.g.ent[m].f26 = 16;
        assert_eq!(
            w.mc1_token_word48(m),
            0,
            "re-taken slot: `NewEvent_372C0`'s memset (0x4FB5B) zeroes \
             `+48`; reading the stranger's `+26` (16) is the pre-D2 bug"
        );
        assert_eq!(w.g.ent[m].f26, 16, "…and `f26` really does hold 16");

        // 4. FREED AGAIN, THIS TIME A NON-TOKEN — the arm the pre-dig
        //    lane got wrong NATIVELY and the import seat papered over.
        //    Retail's word is still the allocator's zero (nothing in
        //    the corpus's `(9,*)` writes `+48`) and `sub_41E90` does
        //    not touch it, so the gate reads 0 and PROMOTES. The
        //    pre-dig class-0 arm read `f26` — the bolt's `+26`, 16 —
        //    and refused. Corpus shape: mc1l49's `owned[16]` names a
        //    freed `(0,13)` carrying `+26 = 116` against `+48 = 0`.
        //    ⭐ NON-VACUITY: this assert FAILS under either kill
        //    switch (`MGC_NO_MC1_TOKEN_RAW48_NATIVE`,
        //    `MGC_NO_MC1_TOKEN_RAW48`), which is what makes it the
        //    fixture for this dig.
        w.g.free_entity(m);
        assert_eq!(w.g.ent[m].class64, 0, "freed a second time");
        assert_eq!(w.g.ent[m].f26, 16, "…and its own `+26` still stands");
        assert_eq!(
            w.mc1_token_word48(m),
            0,
            "freed NON-token: retail's `+48` is the allocator's zero, \
             not this record's `+26`"
        );
    }

    /// Plant a stationary class-9 model-0 (fireball) threat homing on
    /// the rival, 512 units off — inside `sub_16890`'s 1024 reactive
    /// radius and outside the extents that would resolve it as a hit.
    fn plant_threat(w: &mut World, ri: usize) -> usize {
        let me = w.rivals[ri].ent;
        let (x, y, z) = {
            let e = &w.g.ent[me as usize];
            (e.x.wrapping_add(512), e.y, e.z)
        };
        let p = w.g.spawn_fireball(x, y, z).expect("threat slot");
        let e = &mut w.g.ent[p];
        e.f146 = me;
        e.id24 = PLAYER_TARGET;
        // Parked: the reactive arm reads position, not motion, and a
        // moving bolt would resolve into the carpet within a tick.
        e.f126 = 0;
        e.f128 = 0;
        e.act_life = 4000;
        e.max_life = 4000;
        p
    }

    fn token_of(w: &World, ri: usize, spell: usize) -> i16 {
        let m = w.rivals[ri].owned[spell] as usize;
        assert!(m != 0, "the rival never minted a spell-{spell} token");
        w.g.ent[m].f26
    }

    fn token(w: &World, ri: usize) -> i16 {
        let m = w.rivals[ri].owned[14] as usize;
        assert!(m != 0, "the rival never minted a Rebound manifestation");
        w.g.ent[m].f26
    }

    fn rebound_bit(w: &World, ri: usize) -> bool {
        w.g.ent[w.rivals[ri].ent as usize].flags & 0x8000 != 0
    }

    /// ⭐ THE RIVAL'S BOLT IS BORN HOLDING THE SAME DEST TRIPLE AS
    /// THE HUMAN'S, AND NO `+36` (round 154, w154b). The token
    /// machines are ONE routine for both columns (`sub_56090`
    /// :65074-76 fireball, `sub_56AF0` :65474-77 volcano — CARPET.EXE
    /// VA 0x561B7-0x561E2 / 0x56C0D), read off the caster's pool
    /// record as the walk holds it: `+72/+76` raw (NOT the lifted
    /// muzzle z this arm is handed), projected along `+30/+32`. And
    /// the machine writes the bolt's `+30/+32` only (VA 0x56188-97):
    /// the port's `+36 = pitch` was invented — round 153's `(9,1) f36`
    /// 108k free rows / 30 takes (mc1l2 t=2380-91 slot 108, retail 0
    /// for the lob's whole life).
    ///
    /// Corpus: mc1l2 t=2 slot 197 (rival 300's fireball dest (36378,
    /// 5825, 3253) = (37760, 21872, 254) + 0x4000 along (2020, 1988));
    /// the whole `(9,x) dest_*` family on mc1l2 (44,877 free rows) and
    /// mc1l49 (208k) gone, graded lanes unmoved (l49 horizon 34,600
    /// both arms).
    ///
    /// NON-VACUITY: `MGC_NO_MC1_BOLT_DEST_STAMP=1` fails (a) and (b)
    /// at 0/0/0; `MGC_NO_MC1_RIVAL_EMIT_PITCH_MIRROR=1` fails (a)'s
    /// `+36`; `MGC_NO_MC1_EMIT_DETONATION_PAIR=1` fails (b)'s pair.
    #[test]
    fn rival_emit_stamps_the_dest_triple_off_the_record_and_never_f36() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let raw = {
            let e = &w.g.ent[i];
            (e.x, e.y, e.z)
        };
        let muzzle_z = raw.2.wrapping_add(w.g.ent[i].f78 as i16);
        assert_ne!(
            muzzle_z, raw.2,
            "the muzzle lift is non-zero (non-vacuous z)"
        );
        let (yaw, pitch) = (300u16, 1990u16);
        // (a) the fireball: pitched 0x4000 off the RAW record, `+36` untouched.
        w.rival_emit(ri, i, 0, raw.0, raw.1, muzzle_z, yaw, pitch);
        let bolt = (1..w.g.ent.len())
            .find(|&p| w.g.ent[p].class64 == 9 && w.g.ent[p].model65 == 0)
            .expect("the fireball arm mints its (9,0)");
        let mut want = raw;
        Gen::polar_step(&mut want, yaw, pitch, 0x4000);
        let e = &w.g.ent[bolt];
        assert_eq!(
            (e.dest_x, e.dest_y, e.site_z),
            want,
            "rival fireball: `+150/+152/+154` = record raw axis stepped 0x4000 along the aim"
        );
        assert_eq!(
            (e.f30, e.f32),
            (yaw, pitch),
            "the aim pair lands in +30/+32"
        );
        assert_eq!(e.f36, 0, "no token machine writes the bolt's +36");
        // (b) the volcano lob: flat 4096 + the ground under the dest,
        // and the machine's own `+68/+69 = 10/9` (VA 0x56B90/0x56B94).
        w.rival_emit(ri, i, 8, raw.0, raw.1, muzzle_z, yaw, pitch);
        let lob = (1..w.g.ent.len())
            .find(|&p| w.g.ent[p].class64 == 9 && w.g.ent[p].model65 == 4)
            .expect("the volcano arm mints its (9,4)");
        let mut flat = raw;
        Gen::polar_step(&mut flat, yaw, 0, 4096);
        let gz = w.g.ground_z(flat.0, flat.1) as i16;
        let e = &w.g.ent[lob];
        assert_eq!(
            (e.dest_x, e.dest_y, e.site_z),
            (flat.0, flat.1, gz),
            "rival volcano: flat 4096 projection, z = the ground under it"
        );
        assert_eq!(
            (e.f68, e.f69),
            (10, 9),
            "volcano `+68/+69` = (10,9) at the mint"
        );
    }

    /// ⭐⭐⭐ **THE RIVAL'S CHARGE MOVE IS THE WHOLE EMIT FAMILY'S,
    /// NOT FOUR SPELLS'** — see [`super::rival_charge_family`] for the
    /// citation. The class-12 token machines in `str_2563D8`
    /// (`reference/remc1/sub_main.cpp:4957-5033`, state = 3*spell) are
    /// ONE set of functions for the human and the AI; the human's arm
    /// (`crate::engine::world`'s `cast_projectile`) already banks the
    /// whole family, the rival's arm banked four spells.
    ///
    /// ⛔ **NO RECORDING FIXTURE IS POSSIBLE.** Neither observable is
    /// in the recorder's obs schema — `EntObsMc1`
    /// (`crates/mgc-formats/src/mgcr.rs:744-770`) has no `+26`, and
    /// `WizardMc1` has no charge lane. Both ride the RAW state channel
    /// (`mgc_conform::verify::append_charge_diffs`), which only the
    /// PAIR instrument reads, so `replay`/`fixtures` are blind to them.
    /// The corpus proof instead: mc1l49's whole-take pair census drops
    /// from **2,468 dirty rows to 936** with this law in — the entire
    /// 766-row `(9,9) f26` family and its 766 complementary
    /// `rival.charge` rows gone, every other lane's row count
    /// unchanged — mc1l48 16,019 -> 15,921, and all 27 graded takes'
    /// `replay --segmented --brief` byte-identical.
    ///
    /// NON-VACUITY: with `MGC_NO_MC1_RIVAL_CHARGE_FAMILY=1` leg (b)
    /// reads `+26` 0 and the meter still 117.
    #[test]
    fn rival_emit_banks_the_charge_meter_for_the_whole_family() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let ws = w.rivals[ri].slot as usize;
        let muzzle = {
            let e = &w.g.ent[i];
            (e.x, e.y, e.z)
        };
        // (a) The already-modelled arm still banks — the control.
        w.wiz_charge[ws] = 42;
        w.rival_emit(ri, i, 7, muzzle.0, muzzle.1, muzzle.2, 0, 0);
        let bolt = (1..w.g.ent.len())
            .find(|&p| w.g.ent[p].class64 == 9 && w.g.ent[p].model65 == 3)
            .expect("the meteor arm mints its (9,3) bolt");
        assert_eq!(w.g.ent[bolt].f26, 42, "the meteor arm stopped banking");
        assert_eq!(
            w.wiz_charge[ws], 0,
            "the meteor arm left the meter standing"
        );

        // (b) ⭐ THE WITNESS — spell 15, the zigzag lightning.
        // `sub_57470_579A0` (:65806-61) spawns `sub_373F0_377B0(
        // caster.+72, 9, 9)` (:65847) and closes with :65846-48
        // `+26 = u8_326; u8_326 = 0`. mc1l49 t=2926->2927 slot 920:
        // a (9,9) born `+70`=9 `+69`=23 sprite 216 with `+26` = 1 and
        // the rival's meter back at 0; the port left `+26` 0 and let
        // the meter run on to 2 — 766 pairs on that take alone.
        w.wiz_charge[ws] = 117;
        w.rival_emit(ri, i, 15, muzzle.0, muzzle.1, muzzle.2, 0, 0);
        let zig = (1..w.g.ent.len())
            .find(|&p| {
                let e = &w.g.ent[p];
                e.class64 == 9 && e.model65 == 9 && e.tick70 == 9
            })
            .expect("the lightning arm mints its (9,9) bolt");
        assert_eq!(
            w.g.ent[zig].f26, 117,
            "the rival's lightning did not bank the +326 meter (:65846)"
        );
        assert_eq!(
            w.wiz_charge[ws], 0,
            "the rival's lightning left the +326 meter standing (:65848)"
        );
        assert_eq!(w.g.ent[zig].f69, 23, "the lightning arm's +69 stamp");

        // (c) THE BOUNDARY: the duel (`sub_57040` :65620-710) is the
        // ONE spawning arm of the sixteen with no `+326` reference at
        // all — the meter must survive it untouched.
        w.wiz_charge[ws] = 91;
        w.rival_emit(ri, i, 11, muzzle.0, muzzle.1, muzzle.2, 0, 0);
        assert_eq!(
            w.wiz_charge[ws], 91,
            "the duel arm ate the +326 meter — retail's sub_57040 never reads it"
        );
    }

    /// ⭐ THE RIVAL PURSE MIRROR IS A RAW 32-BIT COPY, NOT A CLAMP.
    /// Retail's `+140` IS the wizard's purse word — the shield-quarter
    /// debit writes it directly and RAW (`sub_46540_46880` :55703,
    /// `a1x->var_u32_29935_140 -= v10`, no afford clamp), so a fatal
    /// tick's shortfall WRAPS the word negative and the death arm
    /// preserves it (that arm returns before the regen floor at
    /// :17990). `rival_dispatch_tail` mirrored the purse through
    /// `.min(i32::MAX)`, so every wrapped purse read back as
    /// `2147483647`.
    ///
    /// WITNESS mc1l49 pair 53215→53216, rival slot 750 `(3,1)`
    /// `act_life` −301: retail `+140` = 4294964965 (= −2331 signed),
    /// the port published 2147483647. That head is `replay
    /// --segmented`'s segment 22 boundary; the law retires it
    /// (segments 25 → 24, devs 24 → 23).
    ///
    /// NON-VACUITY: `MGC_NO_MC1_RIVAL_MANA_WRAP_PUBLISH=1` restores
    /// the clamp and the mirror reads `i32::MAX` instead of the purse.
    #[test]
    fn the_rival_purse_mirror_is_a_raw_32_bit_copy() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // The exact word mc1l49's slot 750 carries at t=53216.
        w.rivals[ri].mana = 4_294_964_965;
        w.rival_dispatch_tail(ri, i);
        assert!(
            w.rivals[ri].mana > i32::MAX as u32,
            "scaffold: the dispatch tail must leave the purse wrapped \
             (mana = {})",
            w.rivals[ri].mana
        );
        assert_eq!(
            w.g.ent[i].f140, w.rivals[ri].mana as i32,
            "the +140 purse mirror clamped a wrapped word — retail's \
             :55703 debits the record's own +140 RAW"
        );
    }

    /// ⭐ THE DEFENSE SCAN WALKS THE TICK-TOP CLASS-9 ROSTER
    /// (`sub_16800` :19777 seeds from `var_u32_36462[3]`, the case-9
    /// arm of the tick-head sweep at :52279 — every class-9 record, NO
    /// life or flags test): a ball born MID-tick cannot arm the dodge
    /// until the next rebuild, and a soft-killed member still can.
    ///
    /// This is the mc1l4 t=5377 law — the certification residue whose
    /// pair diff was CLEAN (jink is a wizext lane; the wizext shadow
    /// named it): the pool-scan port dodged the pelting stream's
    /// newborn ball, retail's roster could not yet hold it, and the
    /// rival's whole post-pelting flight parted one flight-tick over.
    ///
    /// NON-VACUITY: the old pool scan fails leg (a) — it sees the
    /// newborn and jinks — and its `flags & 0x400` conjunct fails leg
    /// (c).
    #[test]
    fn the_defense_scan_is_tick_top_and_keeps_soft_kills() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // (a) Mid-tick birth: the roster was sampled before the ball
        // existed — the pool holds it, the scan must not.
        w.g.rebuild_proj_chain();
        let threat = plant_threat(&mut w, ri);
        w.rival_defense(ri, i);
        assert_eq!(w.rivals[ri].jink, 0, "a mid-tick newborn armed the dodge");
        // (b) The next tick top holds it: jink 80 (sub_16870).
        w.g.rebuild_proj_chain();
        w.rival_defense(ri, i);
        assert_eq!(
            w.rivals[ri].jink, 80,
            "a tick-top member did not arm the dodge"
        );
        // (c) A soft kill is not a free: 0x400 landing mid-tick does
        // not hide a member (retail's per-node filter is chase alone).
        w.rivals[ri].jink = 0;
        w.g.ent[threat].flags |= 0x400;
        w.rival_defense(ri, i);
        assert_eq!(
            w.rivals[ri].jink, 80,
            "a soft-killed member stopped arming the dodge"
        );
        // (d) THE RANGE IS 2D (`sub_42410` :52748-54 = Δx² + Δy², no z
        // term): a bolt 5,000 units OVERHEAD is still a dodge threat —
        // in 3D its dz² alone would clear the 5120² gate. mc1hwl0
        // t=16771: threat 516 at dz 2617 read 27.7M in the port's old
        // 3D math against the 26.2M gate (out), 20.9M in retail's 2D
        // (in) — retail re-stamped the strafe, the port let it decay,
        // and the lateral gap became the t=16772 x,y head.
        w.rivals[ri].jink = 0;
        let high = {
            let e = &mut w.g.ent[threat];
            e.z = e.z.saturating_add(5000);
            e.z
        };
        w.g.rebuild_proj_chain();
        w.rival_defense(ri, i);
        assert_eq!(
            w.rivals[ri].jink, 80,
            "a bolt {high} high stopped arming the dodge — the range \
             gate grew a z leg retail does not have"
        );
        // (e) ⭐ THE ELECTION KEY IS UNSIGNED AND THE 0x1900000
        // THRESHOLD TESTS THE WINNER (:19776 `unsigned int v2 = -1`,
        // :19789 `if (v2 >= 0x1900000) return 0` — the seed itself is
        // the "no threat" answer). A bolt at the rival's EXACT
        // half-map antipode wraps to 0x80000000, the LARGEST key: it
        // is elected only as sole candidate and then FAILS the winner
        // threshold — no dodge. The old signed per-candidate gate read
        // i32::MIN < 26.2M and jinked at it.
        w.rivals[ri].jink = 0;
        {
            let (rx, ry) = {
                let e = &w.g.ent[i];
                (e.x, e.y)
            };
            let e = &mut w.g.ent[threat];
            e.x = rx ^ 0x8000;
            e.y = ry ^ 0x8000;
        }
        w.g.rebuild_proj_chain();
        w.rival_defense(ri, i);
        assert_eq!(
            w.rivals[ri].jink, 0,
            "an antipodal bolt armed the dodge — the election key went signed"
        );
    }

    /// The rival Rebound arm, end to end: an incoming fireball inside
    /// 1024 arms the token (`sub_16890` :19822 → `sub_155F0` case 0xE
    /// :19140-48), the token PUBLISHES the deflection bit on the
    /// wizard entity (`sub_573F0_57920` remc1 :65774 / remc1hw
    /// :61996 — `owner->+17 |= 0x80`, our 0x8000), the bit clears when
    /// the 101-tick burst lapses, and a fresh threat re-ups it.
    ///
    /// THE BALLOON RAID NEEDS AN OFFENSE TOKEN: sub_147E0 opens on
    /// sub_16920 (:18611) — owned-token slots for {0, 15, 8, 17, 20,
    /// 7}, the same gate the castle/wizard arms wear at :18506/:18553
    /// — so a disarmed (razed, token-scattered) wizard never raids a
    /// balloon and falls through toward the ball claim. Corpus-silent
    /// at mc1l5 t=19577 (Vodor still owned fireball there; the docked
    /// AABB was that tick's discriminator, fixture-pinned), so the
    /// gate is pinned here against the listing.
    ///
    /// NON-VACUITY: without the gate the second selector call keeps
    /// RaidBalloon — the balloon is still fat, hated and in range.
    #[test]
    fn the_balloon_raid_needs_an_offense_token() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // A fat HUMAN balloon 1500 east of the rival: hated owner
        // (hate over the wealth-scaled bar, war flag CLEAR so the
        // wizard pick's rangeless war arm stays cold and its hated
        // election loses to the range gate — the human pose sits at
        // the world origin, ~43k out), cargo over 10*(275-agg),
        // far from castle_reg[0] (unbound → the scratch slot 0).
        let (bx, by, bz) = {
            let e = &w.g.ent[i];
            (e.x.wrapping_add(1500), e.y, e.z)
        };
        let b = w.g.new_event().expect("balloon slot");
        {
            let e = &mut w.g.ent[b];
            e.class64 = 3;
            e.model65 = 3;
            e.tick70 = 9;
            e.id24 = PLAYER_TARGET;
            e.max_life = 10000;
            e.act_life = 9000;
            e.f140 = 5000;
            e.x = bx;
            e.y = by;
            e.z = bz;
        }
        w.rivals[ri].hate[0] = 65535;
        w.g.rebuild_wiz_chain();
        w.rival_selector(ri, i, true);
        assert_eq!(
            w.rivals[ri].state,
            AiState::RaidBalloon,
            "armed, the raid fires (the pick's own gates all pass)"
        );
        for s in [0usize, 15, 8, 17, 20, 7] {
            w.rivals[ri].owned[s] = 0;
        }
        w.rival_selector(ri, i, true);
        assert_ne!(
            w.rivals[ri].state,
            AiState::RaidBalloon,
            "disarmed, sub_16920 refuses the raid outright"
        );
    }

    /// NON-VACUITY: before the fix the port never wrote 0x8000 for a
    /// rival at all (the mirror was cloak-only), so `rebound_bit`
    /// was false at every one of these assertions and nothing could
    /// ever deflect off an AI wizard.
    #[test]
    fn rival_rebound_arms_publishes_expires_and_re_ups() {
        let mut w = rebound_world();
        assert!(!rebound_bit(&w, 0), "the bit starts clear");
        assert_eq!(token(&w, 0), 0, "the token starts idle");

        // ---- arm ------------------------------------------------------
        let threat = plant_threat(&mut w, 0);
        let mut armed = None;
        for n in 0..8 {
            w.tick(away(), PlayerCommand::default());
            if token(&w, 0) > 0 {
                armed = Some(n);
                break;
            }
        }
        let armed = armed.expect("the incoming fireball never armed Rebound");
        assert!(
            token(&w, 0) >= SPELLS[14].count as i16 - armed as i16 - 2,
            "the token armed short of its {} count",
            SPELLS[14].count
        );
        // The token publishes on its OWN pool slot's tick
        // (sub_573F0): minted after the carpet, it gates and sets the
        // bit the SAME tick the defense cast arms it — by the time
        // the arm is observable the bit is up.
        assert!(
            rebound_bit(&w, 0),
            "the armed token did not publish the 0x8000 deflection bit"
        );

        // ---- the already-active gate ----------------------------------
        // `sub_15A00` case 0xE (:19289-96) refuses a re-cast while the
        // burst is live, so the window is ONE uninterrupted countdown
        // even though the threat is still there and AI_RECAST[14] = 1.
        // Pre-gate the port re-armed to `count` every other tick.
        let mut prev = token(&w, 0);
        for _ in 0..20 {
            w.tick(away(), PlayerCommand::default());
            let now = token(&w, 0);
            assert!(
                now < prev,
                "the live Rebound token was re-armed ({prev} -> {now}) while its burst ran"
            );
            prev = now;
        }

        // ---- expiry ---------------------------------------------------
        w.g.ent[threat].flags |= 0x400;
        for _ in 0..(SPELLS[14].count as usize + 8) {
            w.tick(away(), PlayerCommand::default());
            if token(&w, 0) == 0 {
                break;
            }
        }
        assert_eq!(token(&w, 0), 0, "the token never expired");
        // The clear is the token's NEXT pass (sub_573F0's `+48 <= 0`
        // arm): the tick the counter reaches 0 still ran the gate arm
        // and left the bit standing.
        w.tick(away(), PlayerCommand::default());
        assert!(
            !rebound_bit(&w, 0),
            "the lapsed token left the deflection bit set"
        );

        // ---- re-up ----------------------------------------------------
        // The burst now really pays: sub_55E80's full-tick −1000 debit
        // plus its 101-tick regen pin drained the purse, and both the
        // commit readiness (:19260-63) and the token's own full-tick
        // gate refuse a broke wizard — retail behavior. Let the
        // economy restock before the fresh threat.
        for _ in 0..32 {
            if w.rivals[0].mana >= SPELLS[14].possess_mana {
                break;
            }
            w.tick(away(), PlayerCommand::default());
        }
        assert!(
            w.rivals[0].mana >= SPELLS[14].possess_mana,
            "the economy never restocked the re-up purse"
        );
        plant_threat(&mut w, 0);
        let mut re_upped = false;
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
            if token(&w, 0) > 0 && rebound_bit(&w, 0) {
                re_upped = true;
                break;
            }
        }
        assert!(re_upped, "a fresh threat did not re-up Rebound");
    }

    /// The deflection itself (`sub_52B30` :62858-90): a bolt striking
    /// a rebounding wizard pays a quarter of its own +140 out of the
    /// WIZARD's +140 (his mana — the port's entity mirror of
    /// `Rival::mana`), twangs (sound 28 — INSIDE the afford branch,
    /// :62861), and reverses onto its shooter with the wizard as its
    /// new owner — it never explodes. An unaffordable deflection is
    /// retail's silent fly-through: no hit, no sound, no explosion,
    /// no debit (the :62859 false arm leaves v24 clear).
    ///
    /// NON-VACUITY: pre-fix the port (a) never wrote the wizard's
    /// +140 mirror, so the gate compared against 0 and every real
    /// bolt fell through to the explode — the player-reported
    /// "rebound sound but the meteor explodes on him and nothing
    /// comes back" — and (b) played sound 28 BEFORE the gate. The
    /// deflect arm, the debit, the poor-arm silence and the mirror
    /// assertions all fail on that code.
    #[test]
    fn rebound_deflection_bounces_debits_and_is_silent_when_poor() {
        use crate::mc1::mobs::MobCtx;
        let mut w = rebound_world();
        // Settle until the pool is stocked, then check the WORLD
        // maintains the entity mana mirror at all. The starting
        // castle only joins the census once it reaches its
        // ESTABLISHED tick and echoes `+144 = +24` (sub_46DB0
        // :56015) — the ceiling, and with it the purse, is at the
        // intrinsic 1000 until then.
        for _ in 0..64 {
            w.tick(away(), PlayerCommand::default());
            if w.rivals[0].mana > 2000 {
                break;
            }
        }
        let me = w.rivals[0].ent as usize;
        assert_eq!(
            w.g.ent[me].f140, w.rivals[0].mana as i32,
            "the wizard entity's +140 does not mirror Rival::mana"
        );

        // The deflection reader is driven directly (the bit is the
        // published state, not the token) for slot-order-free
        // arithmetic.
        w.g.ent[me].flags |= 0x8000;
        let ctx = MobCtx {
            px: 10,
            py: 10,
            pz: 200,
            pyaw: 0,
            pmana: 0,
            pmana_max: 0,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        };

        // ---- the affordable deflect -----------------------------------
        // Park the encounter far from the starting castle: the keep's
        // 0x2000-tall envelope otherwise scan-resolves the CASTLE
        // (no bit) instead of the wizard hovering at it.
        w.g.ent[me].f140 = 1000;
        let (wx, wy) = (60u16 << 8, 60u16 << 8);
        let wz = (w.g.ground_z(wx, wy) as i16).wrapping_add(400);
        w.g.move_relink(me, wx, wy, wz);
        let bolt = w.g.spawn_fireball(wx, wy, wz).expect("bolt slot");
        w.g.move_relink(bolt, wx, wy, wz); // the spawner tile-snaps
        {
            let e = &mut w.g.ent[bolt];
            e.id24 = PLAYER_TARGET;
            e.f126 = 0; // parked on the wizard: the scan overlaps
            e.f140 = 400; // quarter = 100
        }
        w.g.sounds.clear();
        w.g.proj_tick(bolt, &ctx);
        {
            let e = &w.g.ent[bolt];
            assert_eq!(e.flags & 0x400, 0, "the deflected bolt exploded");
            assert_eq!(
                e.id24, w.rivals[0].ent,
                "ownership did not swap to the deflector"
            );
            assert_eq!(e.f146, PLAYER_TARGET, "not re-homed on the shooter");
        }
        assert_eq!(
            w.g.ent[me].f140, 900,
            "the deflection did not debit a quarter of the bolt's +140"
        );
        assert!(
            w.g.sounds.iter().any(|s| s.id == 28),
            "no twang on a successful deflection"
        );
        w.g.ent[bolt].flags |= 0x400;

        // ---- the poor wizard: silent fly-through ----------------------
        w.g.ent[me].f140 = 50;
        let bolt2 = w.g.spawn_fireball(wx, wy, wz).expect("bolt2 slot");
        w.g.move_relink(bolt2, wx, wy, wz);
        {
            let e = &mut w.g.ent[bolt2];
            e.id24 = PLAYER_TARGET;
            e.f126 = 0;
            e.f140 = 400; // quarter 100 > the 50 he holds
        }
        w.g.sounds.clear();
        w.g.proj_tick(bolt2, &ctx);
        {
            let e = &w.g.ent[bolt2];
            assert_eq!(e.flags & 0x400, 0, "the fly-through bolt exploded");
            assert_eq!(e.id24, PLAYER_TARGET, "the poor wizard still deflected");
        }
        assert_eq!(w.g.ent[me].f140, 50, "the failed deflection still debited");
        assert!(
            w.g.sounds.iter().all(|s| s.id != 28),
            "an unaffordable deflection twanged (retail is silent, :62861)"
        );

        // The debit round-trips into the pool on the wizard's next
        // tick: the downward reconcile pulls Rival::mana to the
        // debited mirror before the regen step re-adds its delta.
        w.g.ent[me].f140 = 900;
        let pre = w.rivals[0].mana;
        assert!(
            pre > 2000,
            "test premise: the pool must sit well above the debited mirror"
        );
        w.tick(away(), PlayerCommand::default());
        assert!(
            w.rivals[0].mana < pre,
            "the mirror debit never reconciled into Rival::mana"
        );
    }

    /// THE DEFLECTION STORES THE REVERSED PITCH TO BOTH WORDS (round
    /// 154, w154k — [`crate::mc1::combat::no_mc1_deflect_pitch_mirror`]):
    /// `v14 = -(sub_42240(0,+32) * sub_42210(0,+32)); BYTE1(v14) &= 7;
    /// +36 = v14; +32 = v14` (:62727-32 / :62867-72, CARPET.EXE VA
    /// 0x52963 / 0x52D2E). mc1hwl0 t=4494 slot 820: a (9,16) at pitch
    /// 40 deflects off a Rebound-shielded rival and retail reads
    /// `+32 = +36 = 2008`; the port left `+36` at 40. Both port arms
    /// (a pool deflector, the human) are driven here; the pre-dig
    /// value is planted in `+36` so a missed store is visible.
    #[test]
    fn the_deflection_mirrors_the_reversed_pitch_into_f36() {
        use crate::mc1::mobs::MobCtx;
        let mut w = rebound_world();
        for _ in 0..64 {
            w.tick(away(), PlayerCommand::default());
            if w.rivals[0].mana > 2000 {
                break;
            }
        }
        let me = w.rivals[0].ent as usize;
        w.g.ent[me].flags |= 0x8000;
        w.g.ent[me].f140 = 1000;
        let (wx, wy) = (60u16 << 8, 60u16 << 8);
        let wz = (w.g.ground_z(wx, wy) as i16).wrapping_add(400);
        w.g.move_relink(me, wx, wy, wz);
        let ctx = MobCtx {
            px: 10,
            py: 10,
            pz: 200,
            pyaw: 0,
            pmana: 0,
            pmana_max: 0,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        };
        // ---- the pool arm: a human bolt off the rival's shield ------
        let bolt = w.g.spawn_fireball(wx, wy, wz).expect("bolt slot");
        w.g.move_relink(bolt, wx, wy, wz);
        {
            let e = &mut w.g.ent[bolt];
            e.id24 = PLAYER_TARGET;
            e.f146 = 0;
            e.flags |= 2; // acquire spent, untargeted: no homing step before the hit
            e.f126 = 0;
            e.f140 = 400;
            e.f32 = 40;
            e.f36 = 40;
        }
        w.g.proj_tick(bolt, &ctx);
        {
            let e = &w.g.ent[bolt];
            assert_eq!(e.id24, w.rivals[0].ent, "premise: the pool arm deflected");
            assert_eq!(e.f32, 2008, "the live pitch is reversed (-40 & 0x7FF)");
            assert_eq!(
                e.f36, 2008,
                "the deflection stores the reversed pitch to +36 as well (:62731/0x52D2E)"
            );
        }
        // …and the fold is retail's `-(sign·dist) & 0x7FF`: a pitch
        // already "negative" comes back positive, 1024 is its own
        // mirror.
        w.g.ent[me].f140 = 1000;
        for (pitch, want) in [(2008u16, 40u16), (1024, 1024), (0, 0)] {
            let b = w.g.spawn_fireball(wx, wy, wz).expect("bolt slot");
            w.g.move_relink(b, wx, wy, wz);
            {
                let e = &mut w.g.ent[b];
                e.id24 = PLAYER_TARGET;
                e.f146 = 0;
                e.flags |= 2;
                e.f126 = 0;
                e.f140 = 4;
                e.f32 = pitch;
                e.f36 = 7;
            }
            w.g.proj_tick(b, &ctx);
            assert_eq!(
                w.g.ent[b].id24, w.rivals[0].ent,
                "premise: deflected at pitch {pitch}"
            );
            assert_eq!(w.g.ent[b].f32, want, "live pitch at {pitch}");
            assert_eq!(w.g.ent[b].f36, want, "target pitch at {pitch}");
        }
        // ---- the human arm: a rival bolt off the carpet's Rebound ----
        w.g.player_rebound = true;
        let (px, py, pz) = (80u16 << 8, 80u16 << 8, 3000i16);
        let hctx = MobCtx {
            px,
            py,
            pz,
            pmana: 5000,
            ..ctx
        };
        let bolt = w.g.spawn_fireball(px, py, pz).expect("bolt slot");
        w.g.move_relink(bolt, px, py, pz);
        {
            let e = &mut w.g.ent[bolt];
            e.id24 = w.rivals[0].ent;
            e.f146 = 0;
            e.flags |= 2;
            e.f126 = 0;
            e.f140 = 400;
            e.f32 = 100;
            e.f36 = 100;
        }
        w.g.proj_tick(bolt, &hctx);
        {
            let e = &w.g.ent[bolt];
            assert_eq!(e.id24, PLAYER_TARGET, "premise: the human arm deflected");
            assert_eq!(e.f32, 1948, "the live pitch is reversed");
            assert_eq!(
                e.f36, 1948,
                "+36 mirrors it on the human arm too (0x52D2E is one store)"
            );
        }
    }

    /// `sub_16890`'s default arm casts NOTHING (remc1 :19815-52 /
    /// remc1hw :17947-84): only projectile models {0,3,16} reach the
    /// Rebound/Shield ladder and {4,9} the Shield-only one. The port
    /// used to fall every other model through to Shield.
    /// NON-VACUITY: the pre-fix `_ => 4` fallback armed the SHIELD
    /// token on a model-2 threat; the model-9 control proves the
    /// Shield arm itself is live, so the first assertion is not
    /// passing for want of a castable Shield.
    #[test]
    fn unlisted_threat_models_provoke_no_reactive_cast() {
        let mut w = rebound_world();
        // Bank enough mana for Shield (2000) so a failed cast can only
        // be the model gate, never affordability.
        for _ in 0..40 {
            w.tick(away(), PlayerCommand::default());
        }
        assert!(w.rivals[0].mana >= SPELLS[4].possess_mana);

        let threat = plant_threat(&mut w, 0);
        w.g.ent[threat].model65 = 2;
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(
            token_of(&w, 0, 4),
            0,
            "a model-2 threat provoked the Shield cast retail ignores"
        );
        assert_eq!(token(&w, 0), 0, "a model-2 threat provoked Rebound");
        assert!(!rebound_bit(&w, 0));

        // Model 9 IS in retail's Shield-only arm (:19845-49).
        w.g.ent[threat].flags |= 0x400;
        let control = plant_threat(&mut w, 0);
        w.g.ent[control].model65 = 9;
        let mut shielded = false;
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
            if token_of(&w, 0, 4) > 0 {
                shielded = true;
                break;
            }
        }
        assert!(shielded, "the model-9 control never armed Shield");
    }

    /// ⭐ THE POSSESS CLAIM CONE IS STRICT — `< 0x1Cu` (:18254), not
    /// `<= 28`. `sub_13BA0`'s claim arm re-derives the bearing after
    /// the approach and writes `+144` only inside that cone:
    ///
    ///     if ( sub_155F0(a1, 3u) ) {
    ///       v3 = sub_42150_42490(a1 + 72, v1 + 36);
    ///       if ( (unsigned __int16)sub_42210_42550(*(_WORD *)(a1 + 30), v3) < 0x1Cu )
    ///         v1[72] = *(_WORD *)(a1 + 24);          // +144 = the claimant
    ///     }
    ///
    /// NOT A FIXTURE: `+144` is not in the graded obs, so a pair diff
    /// can never see the wrong claim — it re-imports the ball's owner
    /// every tick. mc1l3 lands on the boundary exactly (Vodor at
    /// `+30 = 1082`, ball 105 bearing 1054, angular distance 28) and
    /// the port's `<=` took a claim retail refuses; the free run only
    /// noticed one tick later, through the MANA CENSUS crediting that
    /// ball's 512 to his ceiling (`mana_max` retail 3048, port 3560 at
    /// t=448). This test pins both sides of the boundary directly.
    #[test]
    fn the_possess_claim_cone_refuses_at_exactly_28() {
        let claim_at = |off: u16| -> u16 {
            let mut w = possess_world();
            let ri = 0;
            let i = w.rivals[ri].ent as usize;
            // A wild (10,39) mana ball, well inside the 1024 arrive
            // ring so the approach reports ARRIVED and the claim arm
            // runs at all.
            let (rx, ry, rz) = {
                let e = &w.g.ent[i];
                (e.x, e.y, e.z)
            };
            let b = w.g.new_event().expect("ball slot");
            {
                let e = &mut w.g.ent[b];
                e.class64 = 10;
                e.model65 = 39;
                e.tick70 = 41; // settled
                e.f140 = 512;
                e.f144 = 0; // wild: eligible
                e.act_life = 300;
                e.max_life = 300;
            }
            let (bx, by) = (rx, ry.wrapping_add(512));
            w.g.move_relink(b, bx, by, rz);
            let bearing = Gen::angle_between(rx, ry, bx, by);
            w.g.ent[i].f30 = (bearing + off) & 0x7FF;
            w.g.ent[i].f34 = w.g.ent[i].f30; // inside every commit cone
            w.rivals[ri].mana = 200_000;
            w.rivals[ri].cooldown[3] = 0;
            w.rivals[ri].state = AiState::Possess;
            w.rivals[ri].target = b as u16;
            w.rivals[ri].target_sig = w.target_sig(b as u16);
            assert_eq!(
                Gen::angdist(w.g.ent[i].f30, bearing),
                off,
                "test premise: the offset IS the angular distance"
            );
            w.rival_state_tick(ri, i, false);
            w.g.ent[b].f144
        };
        assert_eq!(claim_at(28), 0, "28 is OUTSIDE the cone — retail refuses");
        assert_ne!(claim_at(27), 0, "27 is inside the cone — the claim lands");
    }

    /// ⭐ THE BALL GUARD'S ELECTION KEY IS UNSIGNED (sub_15340 :18989
    /// `unsigned int v2 = -1`, the landed sub_15260 scout law's
    /// sibling call site): a foreign carpet at the ball's EXACT
    /// half-map antipode wraps to 0x80000000 — the LARGEST key — and
    /// loses the guard election to any real carpet. The old signed
    /// key elected the phantom as "nearest guard" at i32::MIN, read
    /// the ball GUARDED, and refused a ball retail takes. The 5120²
    /// unguarded test is the caller's SIGNED compare on the winner
    /// (:18910 `int > 26214400`).
    ///
    /// NON-VACUITY: the signed key refuses the pick (guard i32::MIN
    /// fails `> 26.2M`) and the assertion fails.
    #[test]
    fn the_ball_guard_election_key_is_unsigned() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let me = w.rivals[ri].ent;
        assert!(
            w.wizard_pos(0).is_some(),
            "test premise: the human is alive (his pose seeds the guard election)"
        );
        let (hx, hy) = (w.human_pose.0, w.human_pose.1);
        // A neutral human-owned ball 20,000 out from the human — far
        // beyond the 5120 guard disc, so retail reads it UNGUARDED.
        let b = w.g.new_event().expect("ball slot");
        {
            let e = &mut w.g.ent[b];
            e.class64 = 10;
            e.model65 = 39;
            e.tick70 = 41;
            e.f140 = 512;
            e.f144 = PLAYER_TARGET; // neutral-owned (no hate latched)
            e.act_life = 300;
            e.max_life = 300;
        }
        let (bx, by) = (hx.wrapping_add(20_000), hy);
        let rz = w.g.ent[i].z;
        w.g.move_relink(b, bx, by, rz);
        // The phantom guard: a foreign carpet at the ball's exact
        // antipode, on the tick-top wiz chain.
        let p =
            w.g.spawn_fireball(bx ^ 0x8000, by ^ 0x8000, rz)
                .expect("carpet slot");
        {
            let e = &mut w.g.ent[p];
            e.class64 = 3;
            e.model65 = 1;
            e.id24 = me.wrapping_add(9); // foreign, and not the ball's id24
            e.act_life = 1000;
            e.flags &= !0x10;
        }
        w.g.rebuild_wiz_chain();
        w.g.rebuild_ball_chain();
        assert!(
            w.rival_pick_ball_target(ri, i),
            "the antipodal phantom guard poisoned the election — the key went signed"
        );
        assert_eq!(w.rivals[ri].state, AiState::Possess);
        assert_eq!(w.rivals[ri].target, b as u16);
    }

    /// ⭐ THE CASTLE-RAID ELECTION KEY IS UNSIGNED, ITS WINNER GATE
    /// SIGNED (sub_143A0 :18505 `unsigned int v7 = -1`, :18520
    /// `v3 < v7`; :18531 `int >= v4*v4` → reject): with a real
    /// eligible castle beyond raid range and a second one at the
    /// rival's EXACT antipode, retail elects the REAL one (the
    /// antipodal key 0x80000000 is the largest) and the winner gate
    /// refuses the raid. The old signed key elected the antipodal
    /// phantom at i32::MIN, slid it past the signed winner gate, and
    /// green-lit a cross-map raid retail never makes.
    ///
    /// NON-VACUITY: the signed key returns `true` (state RaidCastle
    /// at the phantom) and the assertion fails.
    #[test]
    fn the_castle_raid_election_key_is_unsigned() {
        use crate::mc1::behavior::BEHAVIOR;

        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let (rx, ry, rz) = {
            let e = &w.g.ent[i];
            (e.x, e.y, e.z)
        };
        // Both castles human-owned and hated — eligibility rides the
        // hated && undefended leg. ⚠ Park the human OFF the rival's
        // axis first: the fresh world spawns them together, which put
        // the phantom site at the HUMAN's antipode too and the SIGNED
        // undefended test read it "defended" — masking the election
        // (this pin probed VACUOUS in that geometry).
        w.human_pose = (
            rx.wrapping_add(20_000),
            ry.wrapping_add(20_000),
            w.human_pose.2,
        );
        w.rivals[ri].hate[0] = 60_000;
        let fake_castle = |w: &mut World, x: u16, y: u16| {
            let c = w.g.spawn_fireball(x, y, rz).expect("castle slot");
            let e = &mut w.g.ent[c];
            e.class64 = 3;
            e.model65 = 2;
            e.id24 = PLAYER_TARGET;
            e.act_life = 1000;
            e.flags &= !0x10;
            c
        };
        let v28 = BEHAVIOR[w.g.ent[i].row156 as usize].v_28 as i32;
        // The real eligible castle: nearest by the true metric, but
        // 5,000 past the raid range.
        let c1 = fake_castle(&mut w, rx.wrapping_add((v28 + 5_000) as u16), ry);
        // The phantom: the rival's exact antipode.
        fake_castle(&mut w, rx ^ 0x8000, ry ^ 0x8000);
        let (hx, hy) = (w.human_pose.0, w.human_pose.1);
        let (c1x, c1y) = (w.g.ent[c1].x, w.g.ent[c1].y);
        assert!(
            Gen::dist2_sq(c1x, c1y, hx, hy) as u32 > 7_680 * 7_680,
            "test premise: the human defends neither site"
        );
        w.g.rebuild_wiz_chain();
        assert!(
            !w.rival_pick_castle_target(ri, i),
            "the antipodal phantom won the election and slid past the \
             signed winner gate — the key went signed"
        );
    }

    /// ⭐⭐ A WIZARD WHO DIED THIS TICK STILL DEFENDS HIS KEEP
    /// (`sub_143A0` :18517-18). Retail indexes the defender off the
    /// castle's OWN `+24` (`164 * *(__int16 *)(i + 24)`) and reads
    /// that record's `+72` — and runs `sub_11950` on it — with NO
    /// life, `+70` or chain test of any kind. The port routed the
    /// lookup through `wizard_pos`, whose `LifeState::Alive` gate
    /// returns None for a falling carpet, and `is_none_or` read that
    /// as UNDEFENDED — so the raid arm switched on for exactly the
    /// tick a hated wizard died.
    ///
    /// mc1l49 t=5688: the human takes his fatal hit that tick (569
    /// life 3572 → −428, `+70` 0 → 2) 5,738 units from castle 23.
    /// Retail's conjunct fails, the cascade falls through to arm 5
    /// and `+146` stays 569; the port raided (`+146` 569 → 23) and
    /// held the wrong quarry for the next 298 segment heads.
    ///
    /// NON-VACUITY: the second leg moves the same DEAD owner past the
    /// 7680 disc and the raid must then be admitted — so the pin is
    /// about the DISTANCE being read at all, not about a blanket
    /// refusal.
    #[test]
    fn a_dying_owner_still_defends_his_castle() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let (rx, ry, rz) = {
            let e = &w.g.ent[i];
            (e.x, e.y, e.z)
        };
        w.rivals[ri].hate[0] = 60_000;
        // A hated human-owned keep, well inside the rival's raid range.
        let c =
            w.g.spawn_fireball(rx.wrapping_add(3_000), ry, rz)
                .expect("castle slot");
        {
            let e = &mut w.g.ent[c];
            e.class64 = 3;
            e.model65 = 2;
            e.id24 = PLAYER_TARGET;
            e.act_life = 1000;
            e.flags &= !0x10;
        }
        let (cx, cy) = (w.g.ent[c].x, w.g.ent[c].y);
        w.g.rebuild_wiz_chain();
        // THE OWNER IS MID-DEATH-FALL — and parked 4,000 from his keep.
        w.player.state = LifeState::Falling;
        w.human_pose = (cx.wrapping_add(4_000), cy, rz);
        assert!(
            w.wizard_pos(0).is_none(),
            "test premise: the live-state lookup has already dropped him"
        );
        assert!(
            !w.rival_pick_castle_target(ri, i),
            "a corpse-to-be still holds its ground: retail reads the \
             record's +72, not its life state"
        );
        // NON-VACUITY: the same dead owner, now 20,000 out.
        w.human_pose = (cx.wrapping_add(20_000), cy, rz);
        assert!(
            w.rival_pick_castle_target(ri, i),
            "20,000 is past the 7680 disc — the keep IS undefended"
        );
        assert_eq!(w.rivals[ri].state, AiState::RaidCastle);
        assert_eq!(w.rivals[ri].target, c as u16);
    }

    /// ⭐⭐ THE BALL GUARD SEATS THE HUMAN BY TICK-TOP MEMBERSHIP
    /// (`sub_15340` :19003 walks `var_u32_36462[0]`, rebuilt at
    /// :52290-97). A carpet that takes its fatal hit mid-tick is
    /// still in that chain for the rest of the tick and still guards
    /// the balls beside it. The port seeded the election from
    /// `wizard_pos(0)` — the LIVE `LifeState` — so the balls around a
    /// dying human went unguarded for one tick and the rival claimed
    /// the nearest instead of the nearest UNGUARDED one.
    ///
    /// mc1l48 t=6377: the human dies that tick (681 life 50 → −350)
    /// at (2363, 43268); retail's rival 712 skips ball 298 (2,345
    /// units off him, guarded) and claims ball 291 (4,947 off, past
    /// the 5120 disc). The port took 298.
    ///
    /// NON-VACUITY: clearing the tick-top membership admits the ball,
    /// so the pin turns on the membership bit and not on the geometry.
    #[test]
    fn a_ball_beside_a_dying_human_is_still_guarded() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let me = w.rivals[ri].ent;
        let rz = w.g.ent[i].z;
        // A neutral ball, and a far-off foreign carpet so the election
        // has a member even when the human is out of it.
        let b = w.g.new_event().expect("ball slot");
        {
            let e = &mut w.g.ent[b];
            e.class64 = 10;
            e.model65 = 39;
            e.tick70 = 41;
            e.f140 = 512;
            e.f144 = PLAYER_TARGET; // neutral-owned (no hate latched)
            e.act_life = 300;
            e.max_life = 300;
        }
        let (bx, by) = (w.g.ent[i].x.wrapping_add(1_000), w.g.ent[i].y);
        w.g.move_relink(b, bx, by, rz);
        let far =
            w.g.spawn_fireball(bx.wrapping_add(20_000), by, rz)
                .expect("carpet slot");
        {
            let e = &mut w.g.ent[far];
            e.class64 = 3;
            e.model65 = 1;
            e.id24 = me.wrapping_add(9); // foreign
            e.act_life = 1000;
            e.flags &= !0x10;
        }
        w.g.rebuild_wiz_chain();
        w.g.rebuild_ball_chain();
        // The human took his fatal hit AFTER the tick top: dead now,
        // but still a chain member, and parked 2,000 from the ball.
        w.player.state = LifeState::Falling;
        w.human_bucket_alive = true;
        w.human_pose = (bx.wrapping_add(2_000), by, rz);
        assert!(
            w.wizard_pos(0).is_none(),
            "test premise: the live-state lookup has already dropped him"
        );
        assert!(
            !w.rival_pick_ball_target(ri, i),
            "the dying human is still in the tick-top chain — his ball \
             is GUARDED for the rest of this tick"
        );
        // NON-VACUITY: he was already out of the chain at the tick top.
        w.human_bucket_alive = false;
        assert!(
            w.rival_pick_ball_target(ri, i),
            "off the tick-top chain he does not guard, and the only \
             other carpet is 20,000 out"
        );
        assert_eq!(w.rivals[ri].state, AiState::Possess);
        assert_eq!(w.rivals[ri].target, b as u16);
    }

    /// A castle-less rival with Create Castle (16) in its book — the
    /// plant/rebuild laws' scaffolding (ledger 2026-08-20b).
    fn castle_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 5,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let mut book = [false; SPELL_COUNT];
        book[16] = true;
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        cfgs[1] = Some(RivalConfig {
            aggression: 200,
            accuracy: 255,
            tempo: 255,
            castle_level: 0,
            book,
            allowed: book,
        });
        w.set_wizards(&cfgs, 2);
        w
    }

    /// ⭐ THE FULL-LIST LEARN ORPHAN IS RETAIL'S "HOVERING JAR", AND
    /// THE NATIVE ENCODING DOES NOT DRAW IT (round 158, w158c —
    /// player report on `recordings/mc1l45.mgcr`: "rivals started
    /// dropping a spell jar that HOVERED in the air"). Retail's learn
    /// expiry (`sub_15EC0` :19415-30) conjures FIRST and only then
    /// looks for an empty `+532` seat; with all 24 taken the token is
    /// born `flags 4` (hide bit clear → retail's painter DRAWS it),
    /// `+42 0`, `+70 = 3*spell` (phase 0: the manifest row, never the
    /// `sub_55A40` jar tick — no fall, no pickup poll, no decay), so it
    /// hangs at the rival's altitude for the rest of the level.
    /// mc1l45: rival 1 (carpet 444) mints four `(12,0)` orphans —
    /// slots 219/182/210/118 at t=25622/26679/31234/32560, 309..735
    /// above the ground, all alive at the take's end. The list was
    /// full because a stale volcano plume register soft-killed the
    /// rival's Fireball token (t=23826) and the death scatter then
    /// re-read the recycled slot's model (a `(10,2)`) into seat 0, so
    /// the respawn re-granted Accelerate twice and never Fireball.
    /// Every human contact with a Fireball jar re-arms the countdown.
    ///
    /// Native play mints the same orphan (the retail law is
    /// unconditional) but in the port's own `MANIFEST_BASE + spell`
    /// encoding, which both draw filters skip — so the port never
    /// shows the hovering jar outside a retail replay.
    #[test]
    fn a_full_list_learn_orphan_is_not_drawn_natively() {
        let mut w = castle_world();
        let ri = 0;
        let tok = w.rivals[ri].owned[16];
        assert!(tok != 0, "the (12,16) token stands");
        // Every seat taken, none of them a Fireball token.
        w.rivals[ri].acq = [tok as i32; SPELL_COUNT];
        w.rivals[ri].learn[0] = 1;
        w.tick(away(), PlayerCommand::default());
        assert_eq!(w.rivals[ri].learn[0], 0, "the countdown expired");
        let orphan = (1..w.g.ent.len())
            .find(|&k| {
                let e = &w.g.ent[k];
                e.class64 == 12 && e.model65 == 0 && e.flags & 1 == 0 && e.f144 == 0
            })
            .expect("the full list orphans the conjured (12,0)");
        assert!(
            !w.rivals[ri].acq.contains(&(orphan as i32)),
            "the orphan joins no book"
        );
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            w.rivals[ri].owned[0], 0,
            "and the rival still lacks Fireball"
        );
        // The human's own-spell hide must not be what hides it.
        w.player.owned[0] = 0;
        let drawn = |w: &World| {
            w.live_poses().iter().any(|p| p.class == 12 && p.model == 0)
                || w.live_things()
                    .iter()
                    .any(|t| t.class == 12 && t.model == 0)
        };
        assert!(!drawn(&w), "the native encoding draws no hovering jar");
        // Positive control: in retail's phase-0 encoding it IS drawn.
        w.g.ent[orphan].tick70 = 0;
        assert!(drawn(&w), "non-vacuous: retail's `3*spell` orphan is drawn");
    }

    /// ⭐⭐⭐ THE OWNED REBUILD WRITES PAST THE BOOK, INTO THE RECAST
    /// TABLE (`sub_45C10_45F50` :55310-19 — see
    /// [`owned_rebuild_overflow`]). Retail re-registers each
    /// acquisition entry under the MODEL byte of whatever pool record
    /// it now names, with NO class filter and NO bound: `+676 + 2*m`
    /// runs straight off the 24-entry book into `+724`, the AI recast
    /// cooldown table. mc1l49 t=36793: rival 594's `acq[3]` still
    /// names pool slot 26, recycled into a class-10 model-39 record,
    /// and `676 + 2*39 = 754 = 724 + 2*15` pins Lightning's recast at
    /// the slot number 26 — the recording holds it there for the
    /// remaining 28,000 ticks while its neighbours decay normally.
    ///
    /// NOT FIXTURE-PINNABLE: the write's only direct observable is the
    /// wizext shadow (`cooldown[]`), which the pair grader does not
    /// score — `verify-deltas` on the stamp pair 36792→36793 reports
    /// "2 conforming" with the pre-dig port writing nothing at all.
    #[test]
    fn the_owned_rebuild_writes_past_the_book_into_the_recast_table() {
        let mut w = castle_world();
        let ri = 0;
        // The recycled acquisition slot: NOT a class-12 manifestation.
        let stray = 26usize;
        w.g.ent[stray].class64 = 10;
        w.g.ent[stray].model65 = 39;
        w.rivals[ri].acq[3] = stray as i32;
        w.rivals[ri].cooldown[15] = 0;
        w.rival_owned_rebuild(ri);
        assert_eq!(
            w.rivals[ri].cooldown[15], stray as u16,
            "`+676 + 2*39` is `+754` = cooldown[15]: the blind write lands there"
        );
        assert_eq!(
            w.rivals[ri].owned[15], 0,
            "the BOOK slot for spell 15 is untouched — the rival still owns nothing there"
        );
        // And it is re-stamped every dispatch, one step AFTER the
        // decay loop (:17939-45 then :17969), so it never runs down.
        for _ in 0..3 {
            w.tick(away(), PlayerCommand::default());
        }
        assert_eq!(
            w.rivals[ri].cooldown[15], stray as u16,
            "the rebuild re-stamps it after every decay — the recast is pinned forever"
        );
    }

    /// ⭐⭐⭐ A BLANKED CLASS-3 ROSTER HIDES THE HUMAN FROM THE WIZARD
    /// PICK (`sub_145B0` :18554 — see [`mc1_human_on_wiz_roster`]).
    /// Retail's election walks `var_u32_36462[0]` and nothing else, and
    /// [`Gen::new_event`]'s mid-tick seizure NULLS that head for the
    /// rest of the tick; the port carries the human out of the pool and
    /// judged him in a pre-pass placed OUTSIDE the walk, so a blanked
    /// roster left him standing as the only candidate in the world.
    ///
    /// NOT FIXTURE-PINNABLE: the pair grader re-seeds `+415` from the
    /// record at every anchor ([`World::reanchor_rival_ai`]), so an
    /// import at `head - 1` starts the port ALREADY in Cruise and the
    /// pair conforms — mc1l48's four `target_yaw` heads on slot 712
    /// (t=26740/26742/26744/26751) all classify INHERITED, and
    /// `verify-deltas` over the whole 300-pair window reports not one
    /// `target_yaw` row on that slot. The defect lives only in the free
    /// run.
    #[test]
    fn a_blanked_class3_roster_hides_the_human_from_the_wizard_pick() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        w.g.rebuild_wiz_chain();
        w.human_bucket_alive = true;
        w.player.invisible = false;
        // The war arm is first-in-chain and RANGELESS (:18563-68), so
        // the election turns on candidacy alone.
        w.rivals[ri].war[0] = true;
        w.rivals[ri].state = AiState::Cruise;
        assert!(
            w.rival_pick_wizard_target(ri, i),
            "an intact roster seats the human: the war arm must elect him"
        );
        assert_eq!(w.rivals[ri].state, AiState::AttackWizard);
        assert_eq!(w.rivals[ri].target, PLAYER_TARGET);

        // Now blank the head, exactly as a mid-tick seizure does
        // (`Gen::new_event` :43885-91 nulls all four roster heads).
        w.rivals[ri].state = AiState::Cruise;
        w.rivals[ri].target = 0;
        w.g.wiz_chain.cut = 0;
        assert!(
            !w.rival_pick_wizard_target(ri, i),
            "a blanked `var_u32_36462[0]` head hides the human too — \
             retail's walk starts from NULL and terminates at once"
        );
        assert_eq!(
            w.rivals[ri].state,
            AiState::Cruise,
            "the refused arm writes nothing; the cascade falls through to Idle"
        );
    }

    /// ⭐⭐ THE MC1 CASTLE-ARM WAR THRESHOLD IS FLAT 50000 (:19733-39):
    /// the listing's aggression multiplier reads through the victim
    /// CASTLE's +160 — a wizext pointer only carpets carry, mint-zero
    /// on a castle — so the scaled term never contributes. Corpus: war
    /// latches at 50518/50659/51518 (first crossing ABOVE 50000),
    /// never at 49518/49659; window 4 peaks 49531 and decays out
    /// unlatched. The fixture cannot pin this (the war lane is
    /// pair-imported), so the constant is pinned here — with the
    /// rival's aggression at 200, where the scaled threshold would
    /// have been far lower.
    #[test]
    fn the_war_threshold_is_flat_50000() {
        let mut w = possess_world();
        let ri = 0;
        w.rivals[ri].hate[0] = 49_999;
        w.rival_war_check(ri, 0);
        assert!(!w.rivals[ri].war[0], "49,999 is under the threshold");
        w.rivals[ri].hate[0] = 50_000;
        w.rival_war_check(ri, 0);
        assert!(
            !w.rivals[ri].war[0],
            "the compare is strict: exactly 50,000 does not latch"
        );
        w.rivals[ri].hate[0] = 50_001;
        w.rival_war_check(ri, 0);
        assert!(
            w.rivals[ri].war[0],
            "the first crossing above 50,000 latches"
        );
    }

    /// ⭐⭐ THE SCOUT'S HOME SUPERCELL IS SIGNED x/16384, TRUNCATING
    /// TOWARD ZERO (:18362-67's CFSHL idiom), and the walk runs
    /// THROUGH THE SCRATCH RECORD: each candidate writes slot 0's x/y
    /// (:18374-86) and the accept stamps the wizard's +150/+152/+154
    /// with the candidate and the scratch's NEVER-WRITTEN z
    /// (:18381-83). mc1l5 t=14694: Vodor rebuilds at x=65333 (i16
    /// −203 → cell 0) and retail's first candidate is literally
    /// (0,0); a `u16 >> 14` start planted a map-quadrant away.
    #[test]
    fn the_scout_cell_is_signed_and_walks_the_scratch() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let z = w.g.ent[i].z;
        w.g.move_relink(i, 65333, 65333, z);
        w.g.rebuild_wiz_chain();
        w.g.ent[0].z = 352; // the scratch's standing z (imported state)
        assert!(
            w.rival_scout_site(ri, i),
            "no foreign castles: first candidate wins"
        );
        assert_eq!(
            w.rivals[ri].site,
            (0, 0),
            "i16 −203 / 16384 truncates to cell 0 — the home corner IS (0,0)"
        );
        let e = &w.g.ent[i];
        assert_eq!((e.dest_x, e.dest_y), (0, 0), "the accept stamps +150/+152");
        assert_eq!(e.site_z, 352, "+154 = the scratch record's unwritten z");
        assert_eq!(
            (w.g.ent[0].x, w.g.ent[0].y),
            (0, 0),
            "the scratch keeps the last probed candidate"
        );
    }

    /// ⭐⭐⭐ THE AI CASCADE'S CASTLE ARMS READ RAW REGISTERS — never a
    /// pool scan, a `known` flag or the ladder table. `sub_13F00`'s
    /// head (:18359) is three raw reads:
    ///
    ///     if ( !wizext->var_50 && sub_14E60(a1, 0x10)
    ///          && sub_15E90(a1, 0x10) )
    ///
    /// `sub_14E60` (:18769-77) is a bare `pool + 164 * var_676[16]`
    /// guarded only against slot 0 — "is my Create-Castle TOKEN
    /// REGISTER set", NOT "do I know spell 16" — and `sub_15E90`
    /// (:19376-79) is `pool[owned[16]]->+136 <= a1->+136`, the
    /// TOKEN's LIVE price cache with NO class, flag or liveness guard
    /// and no spell-table fallback. `sub_14120`'s mana leg (:18427)
    /// prices off the same `+136`. D5's ladder-EVENT law made that
    /// field a per-level-event value rather than a derivable one, so
    /// the two readings genuinely part company.
    ///
    /// WITNESS (mc1l49 t=37002, `state.struct_b64`): wiz 1 has
    /// `+50 = 0` AND `owned[16] = 0`, so retail refuses at the SECOND
    /// term and stays in Possess; the port saw `known[16]` true,
    /// priced off `SPELLS[16].possess_mana` and went to Build on
    /// every pair tick of the window.
    ///
    /// This law takes no recording fixture — `ObsMc1`/`EntObsMc1`
    /// emit neither the `owned` book nor `+50` nor the token's
    /// `+136`, and a 21-tick sweep either side of t=37002 is
    /// byte-identical under the switch — so the unit is the only pin.
    ///
    /// NON-VACUITY: under `MGC_NO_MC1_CASTLE_ARM_REGISTERS=1` the
    /// pre-dig gate reads `known[16]` and `rival_castle_price`, whose
    /// class/flag-guarded fallback lands on the table price, and BOTH
    /// refusals below become Build.
    #[test]
    fn the_castle_build_arm_gates_on_the_token_register_not_the_spell_book() {
        let mut w = castle_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let ws = w.rivals[ri].slot as usize;
        assert_eq!(w.g.castle_reg[ws], 0, "castle-less: the +50 register is 0");
        assert!(w.rivals[ri].known[16], "the rival knows Create Castle");
        let m = w.rivals[ri].owned[16] as usize;
        assert!(m != 0, "the rival minted its (12,16) token");
        // A purse that comfortably covers the SPELL-TABLE price — the
        // number the pre-dig gate falls back to whenever the token
        // register cannot be read.
        let table = SPELLS[16].possess_mana;
        let purse = table + 10_000;
        w.rivals[ri].mana_max = purse;
        w.rivals[ri].mana = purse;

        // ---- the anchor: every raw register says yes ------------------
        w.g.ent[m].f136 = (purse - 1) as i32;
        w.rivals[ri].state = AiState::Possess;
        w.rival_selector(ri, i, true);
        assert_eq!(
            w.rivals[ri].state,
            AiState::Build,
            "non-vacuous: +50 clear, the token register set and its live \
             +136 inside the purse — the build arm opens"
        );

        // ---- sub_14E60: the TOKEN REGISTER, not the book -------------
        // The scattered wizard still KNOWS spell 16 and could still
        // afford the table price; retail refuses at the second term
        // because `var_676[16]` is 0 (mc1l49 t=37002).
        w.rivals[ri].owned[16] = 0;
        assert!(w.rivals[ri].known[16], "the book entry is untouched");
        w.rivals[ri].state = AiState::Possess;
        w.rival_selector(ri, i, true);
        assert_ne!(
            w.rivals[ri].state,
            AiState::Build,
            "a scattered token register refuses the build — sub_14E60 \
             reads +676[16], never the `known` flag"
        );

        // ---- sub_15E90: the token's RAW +136, no guards, no table ----
        // The register points at a SOFT-KILLED token whose live price
        // cache is over the purse. Retail dereferences it anyway (no
        // class, flag or liveness test) and refuses; the pre-dig
        // `rival_castle_price` guard rejects the token and prices the
        // build off the spell table instead, which the purse covers.
        w.rivals[ri].owned[16] = m as u16;
        w.g.ent[m].f136 = (purse + 1) as i32;
        w.g.ent[m].flags |= 0x400;
        assert!(
            w.rivals[ri].mana_max >= table,
            "non-vacuous: the table price is still affordable"
        );
        w.rivals[ri].state = AiState::Possess;
        w.rival_selector(ri, i, true);
        assert_ne!(
            w.rivals[ri].state,
            AiState::Build,
            "the arm priced off the token's raw +136 — sub_15E90 has no \
             class/flag guard and no spell-table fallback"
        );
    }

    /// ⭐⭐ THE SAME LAW ON THE OTHER ARM: `sub_14120`'s mana leg
    /// (:18427) is `a1->+136 < v3->+136` where `v3` is
    /// `sub_14E60(a1, 0x10)` — the TOKEN again, NOT
    /// `CASTLE_CAP[level]`. The two readings agreed only while the
    /// port re-priced the token every tick; D5's ladder-EVENT law
    /// (L6) made the token's live `+136` the last LEVEL EVENT's
    /// price, which on a register shared through a recycled slot is a
    /// different castle's rung altogether — mc1l49 t=35404, token 26
    /// holds 40,000 = `CAP[3]` under a level-4 castle whose
    /// `CAP[4]` is 80,000.
    ///
    /// NON-VACUITY: under `MGC_NO_MC1_CASTLE_ARM_REGISTERS=1` the leg
    /// reads `CASTLE_CAP[reg.f26]`, which this purse covers, and the
    /// upgrade opens.
    #[test]
    fn the_castle_upgrade_arm_prices_off_the_token_not_the_ladder_table() {
        let mut w = castle_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let ws = w.rivals[ri].slot as usize;
        // Plant a castle the way Build does, so +50 is bound for real.
        w.rivals[ri].state = AiState::Build;
        w.rivals[ri].site = (w.g.ent[i].x, w.g.ent[i].y);
        w.rivals[ri].mana = 200_000;
        w.rivals[ri].cooldown[16] = 0;
        w.rival_state_tick(ri, i, false);
        let c = w.rival_castle(w.rivals[ri].ent).expect("the plant landed");
        assert_eq!(w.g.castle_reg[ws], c as u16, "+50 is bound to the castle");
        // A SETTLED level-1 castle: the ladder table would price its
        // next rung at CASTLE_CAP[1].
        {
            let e = &mut w.g.ent[c];
            e.f26 = 1;
            e.tick70 = 4;
            // Full health — arm 2 (flee home hurt) sits above arm 3.
            e.act_life = e.max_life as i32;
        }
        w.g.ent[i].act_life = w.g.ent[i].max_life as i32;
        let table = Gen::CASTLE_CAP[1] as u32;
        // The purse clears the LADDER rung and nothing else.
        let purse = table + 5_000;
        w.rivals[ri].mana_max = purse;
        w.rivals[ri].mana = purse;
        w.rivals[ri].cooldown[16] = 0;
        let m = w.rivals[ri].owned[16] as usize;
        assert!(m != 0, "the (12,16) token stands");
        w.g.ent[m].f26 = 0; // the burst is idle: the arm's first term

        // ---- the anchor: the token's own price is affordable ---------
        w.g.ent[m].f136 = (purse - 1) as i32;
        w.rivals[ri].state = AiState::Possess;
        w.rival_selector(ri, i, true);
        assert_eq!(
            w.rivals[ri].state,
            AiState::Upgrade,
            "non-vacuous: with the TOKEN's live +136 inside the purse the \
             upgrade arm opens"
        );

        // ---- the law: the token's +136, above the purse, refuses -----
        // The ladder rung is still comfortably affordable, so only a
        // gate reading the TOKEN can refuse here.
        w.g.ent[m].f26 = 0;
        w.g.ent[m].f136 = (purse + 1) as i32;
        w.rivals[ri].cooldown[16] = 0;
        assert!(
            w.rivals[ri].mana_max >= Gen::CASTLE_CAP[w.g.ent[c].f26 as usize] as u32,
            "non-vacuous: CASTLE_CAP[level] is still inside the purse"
        );
        w.rivals[ri].state = AiState::Possess;
        w.rival_selector(ri, i, true);
        assert_ne!(
            w.rivals[ri].state,
            AiState::Upgrade,
            "sub_14120's mana leg reads the TOKEN's live +136, never \
             CASTLE_CAP[level]"
        );
    }

    /// ⭐ THE OWNED REGISTER'S PHASE IS THE REBUILD'S (:17969):
    /// `sub_45C10` projects the acquisition list into `owned[]` at
    /// the TOP of the rival dispatch, BEFORE the learn expiry's mint
    /// at :18022 — a spell minted at T reaches `owned[]` at T+1 (the
    /// human's opposite phase is call order, :55342, not law). The
    /// heads are INHERITED (mc1l37 t=2487's castle-build cascade) —
    /// this pin carries the phase.
    #[test]
    fn a_learned_spell_reaches_owned_one_tick_late() {
        let mut w = castle_world();
        let ri = 0;
        w.rivals[ri].learn[3] = 1;
        w.tick(away(), PlayerCommand::default());
        assert_eq!(w.rivals[ri].learn[3], 0, "the countdown expired");
        assert_eq!(
            w.rivals[ri].owned[3], 0,
            "the mint tick leaves owned[] untouched (its rebuild had already run)"
        );
        w.tick(away(), PlayerCommand::default());
        assert_ne!(
            w.rivals[ri].owned[3], 0,
            "the next housekeeping's rebuild registers the token"
        );
    }

    /// ⭐ THE NEAREST-CASTLE KEY IS UNSIGNED (:18937 `unsigned int
    /// v2 = -1`): sub_42410's i32 dx²+dy² wraps to i32::MIN at the
    /// exact half-map diagonal (dx = dy = ±32768 ⇒ 2·2³⁰), and
    /// retail reads the wrap as the LARGEST key — a castle on that
    /// diagonal can never win the election over any real neighbour.
    /// A signed seed elected the phantom "nearest" (whose Chebyshev
    /// gap 32768 clears the 12288 veto) and green-lit the first
    /// candidate retail rejects (mc1l49 t=3298: cand (49152,16384)
    /// vs the far-corner castle (16384,49152)). The head is
    /// INHERITED (pair-clean) — this pin carries the law.
    #[test]
    fn the_scout_nearest_castle_key_is_unsigned() {
        let mut w = castle_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let me = w.rivals[ri].ent;
        let z = w.g.ent[i].z;
        // Home the wizard so the first candidate is (49152, 16384)
        // (x = 49152 as i16 = −16384 → cell 3; y = 16384 → cell 1).
        w.g.move_relink(i, 49152, 16384, z);
        let fake_castle = |w: &mut World, x: u16, y: u16| {
            let gz = w.g.ground_z(x, y) as i16;
            let c = w.g.spawn_fireball(x, y, gz).expect("castle slot");
            let e = &mut w.g.ent[c];
            e.class64 = 3;
            e.model65 = 2;
            e.id24 = me.wrapping_add(9);
            e.act_life = 1000;
            e.flags &= !0x10;
            c
        };
        // A foreign castle EXACTLY the half-map diagonal from the
        // first candidate — its key wraps to 0x80000000 — and the
        // true neighbour 8000 south, inside the home candidates'
        // 12288 Chebyshev veto.
        fake_castle(&mut w, 16384, 49152);
        fake_castle(&mut w, 49152, 24384);
        w.g.rebuild_wiz_chain();
        assert!(w.rival_scout_site(ri, i), "the walk still finds a site");
        assert_ne!(
            w.rivals[ri].site,
            (49152, 16384),
            "the phantom-nearest election green-lit the corner the true neighbour vetoes"
        );
        assert_eq!(
            w.rivals[ri].site,
            (0, 16384),
            "the true neighbour wins the election, vetoes the home cell, and the third candidate lands"
        );
    }

    /// ⭐⭐ A STALE TARGET KEEPS THE STATE: every combat handler opens
    /// on the sig-vs-stored test and returns with NO writes on
    /// mismatch (sub_13BA0 :18246, sub_13CA0 :18281, sub_13DD0
    /// :18323) — there is NO Fresh transition in the retail machine;
    /// the think cascade is the only mover. mc1l5 t=12158: the
    /// claimed ball dies and retail idles in Possess for 450 ticks.
    #[test]
    fn a_stale_target_keeps_the_state() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let b = w.g.new_event().expect("ball slot");
        {
            let e = &mut w.g.ent[b];
            e.class64 = 10;
            e.model65 = 39;
            e.tick70 = 41;
            e.act_life = 300;
        }
        w.rivals[ri].state = AiState::Possess;
        w.rivals[ri].target = b as u16;
        w.rivals[ri].target_sig = w.target_sig(b as u16);
        // The slot is reaped and re-minted as something else — the
        // signature moves (team + model + class<<7).
        w.g.ent[b].class64 = 9;
        for _ in 0..8 {
            w.rival_state_tick(ri, i, false);
        }
        assert_eq!(
            w.rivals[ri].state,
            AiState::Possess,
            "no drop-to-Fresh exists"
        );
        assert_eq!(w.rivals[ri].target, b as u16, "the stale target keeps too");
    }

    /// ⭐⭐ THE FREE PLANT IS A BARE FLAG AND BUILD WRITES NO STATE
    /// (:19200-08 / sub_138F0 :18142-68): the planted castle is born
    /// state 5 TRANSFORM, level 0, sprite 177, binds wizext+50 at
    /// spawn (:19206) and stamps NO terrain (BUILD row 0 is empty —
    /// the pad is the level-up commit's painter). The handler leaves
    /// the AI state alone, so the STILL-Build handler re-casts 16 the
    /// very next tick through the NOW-bound arm and arms the upgrade
    /// token (mc1l5 t=14772's (9,10)). Teardown-to-0 clears the
    /// binding blind (:56534).
    #[test]
    fn the_plant_is_a_bare_flag_and_build_recasts_bound() {
        let mut w = castle_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let ws = w.rivals[ri].slot as usize;
        w.rivals[ri].state = AiState::Build;
        w.rivals[ri].site = (w.g.ent[i].x, w.g.ent[i].y);
        w.rivals[ri].mana = 200_000;
        w.rivals[ri].cooldown[16] = 0;
        let pristine = w.g.t.height.clone();

        // Tick 1: arrived at the site, the free plant fires.
        w.rival_state_tick(ri, i, false);
        let c = w.rival_castle(w.rivals[ri].ent).expect("the plant landed");
        assert_eq!(w.rivals[ri].state, AiState::Build, "Build writes NO state");
        assert_eq!(w.g.castle_reg[ws], c as u16, "the plant binds wizext+50");
        {
            let e = &w.g.ent[c];
            assert_eq!(e.tick70, 5, "born TRANSFORM");
            assert_eq!(e.f26, 0, "level 0");
            assert_eq!(e.type86, 177, "sprite 177 flat");
        }
        assert_eq!(w.g.t.height, pristine, "a level-0 plant stamps NO terrain");

        // Tick 2: the still-Build handler re-casts through the BOUND
        // arm — the upgrade token arms on the day-old castle.
        w.rival_state_tick(ri, i, false);
        assert_eq!(w.rivals[ri].state, AiState::Build, "still no state write");
        assert!(
            token_of(&w, ri, 16) > 0,
            "the re-cast armed the upgrade token through the bound arm"
        );

        // Teardown to level 0 clears the binding blind.
        w.g.ent[c].tick70 = 6;
        w.g.ent[c].f26 = 0;
        w.g.castle_tick(c, crate::patches::WorldPatches::default());
        assert_eq!(w.g.castle_reg[ws], 0, "teardown-to-0 clears wizext+50");
        assert!(w.g.ent[c].flags & 0x400 != 0, "the flag soft-kills");
    }

    /// ⭐ THE REBUILT FLAG WEARS THE OWNER'S COLORS: the plant's ctor
    /// row is 177 FLAT and the recolor is the FIRST level-up commit's
    /// one-time latch stamp (`+86 += wizard +48`, :56057-62 — the
    /// claimed-dwelling family, :30808-09). The port used to latch
    /// the bit but skip the stamp (the flag billboard's art is the
    /// row; there is no pose.team recolor stage), so every post-raze
    /// rival castle flew the HUMAN's white flag — masked on FIRST
    /// castles because the authored mint pre-stamped 177+slot.
    #[test]
    fn the_first_commit_recolors_the_planted_flag() {
        let mut w = castle_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let team = w.rivals[ri].slot as u16;
        assert_ne!(team, 0, "a team-0 rival would mask the stamp");
        w.rivals[ri].state = AiState::Build;
        w.rivals[ri].site = (w.g.ent[i].x, w.g.ent[i].y);
        w.rivals[ri].mana = 200_000;
        w.rivals[ri].cooldown[16] = 0;
        w.rival_state_tick(ri, i, false);
        let c = w.rival_castle(w.rivals[ri].ent).expect("the plant landed");
        assert_eq!(w.g.ent[c].type86, 177, "born flat");
        // The castle's own first transform tick (born state 5,
        // sub-state 0) runs the level-up commit.
        w.g.castle_tick(c, crate::patches::WorldPatches::default());
        assert_eq!(
            w.g.ent[c].type86,
            177 + team,
            "the first commit stamps the owner's flag row"
        );
        // One-time: a later re-entry into the commit (the upgrade
        // chain) must not add the color again.
        w.g.ent[c].f59 = 0;
        w.g.castle_tick(c, crate::patches::WorldPatches::default());
        assert_eq!(w.g.ent[c].type86, 177 + team, "the stamp is latched");
    }

    /// ⭐⭐ THE CLAIM GATE READS THE TOKEN'S LIVE +136 PRICE CACHE
    /// (sub_14230 :18452: `wiz +136 <= manifestation +136`, no gate
    /// when 16 is unowned) — CAP[level] housed, 1000 ctor, 5000
    /// after a raze — so ball-claiming re-opens after every upgrade
    /// AND while razed. mc1l5 t=16081: razed Vodor at ceiling 1768
    /// vs his token's 5000 claims the wild 2000-ball; the port's
    /// static-cost stand-in (1768 > 1000) fell through to HuntMana.
    /// (The exemplar pair carries the at-castle mana-register
    /// residue, so the law is pinned here instead of a fixture.)
    #[test]
    fn the_claim_gate_reads_the_live_token_price() {
        let claim = |price: i32| -> AiState {
            let mut w = castle_world();
            let ri = 0;
            let i = w.rivals[ri].ent as usize;
            w.rivals[ri].known[3] = true;
            w.rivals[ri].allowed[3] = true;
            // `sub_14230`'s FIRST term is `sub_14E60(a1, 3u)` — the
            // POSSESS TOKEN REGISTER, not the book flag (see
            // [`possess_arm_register`]); this world's book only mints
            // spell 16, so seat the register by hand or the arm dies
            // above the price gate this test is about.
            let t3 = w.g.new_event().expect("possess token");
            w.g.ent[t3].class64 = 12;
            w.rivals[ri].owned[3] = t3 as u16;
            let m16 = w.rivals[ri].owned[16] as usize;
            w.g.ent[m16].f136 = price;
            w.rivals[ri].mana_max = 1768;
            // A wild settled ball in range, and no castle (razed).
            let (rx, ry, rz) = {
                let e = &w.g.ent[i];
                (e.x, e.y, e.z)
            };
            let b = w.g.new_event().expect("ball");
            {
                let e = &mut w.g.ent[b];
                e.class64 = 10;
                e.model65 = 39;
                e.tick70 = 41;
                e.f140 = 2000;
                e.act_life = 300;
            }
            w.g.move_relink(b, rx, ry.wrapping_add(600), rz);
            w.g.rebuild_ball_chain();
            w.rival_selector(ri, i, true);
            w.rivals[ri].state
        };
        assert_eq!(
            claim(5000),
            AiState::Possess,
            "ceiling 1768 <= the razed token's 5000: the claim is open"
        );
        assert_ne!(
            claim(1000),
            AiState::Possess,
            "ceiling 1768 > a 1000 cache: the claim gate is closed"
        );
    }

    /// ⭐⭐ THE WAKE PASS READS THE CARPET SETTLED LAST TICK
    /// (sub_54F00's :64352-53 pool read, a PRE-pass): native play
    /// feeds this tick's live pose as the `player` arg, so the
    /// settled-last-tick value is the `human_pose_prev` echo; the
    /// replay drivers already pass settled(N−1) AS the arg
    /// (`strict_retail`). Exposed only by a teleport-scale jump —
    /// mc1l5 t=14420, the human teleports back into bee 338's wake
    /// radius and retail's t=14421 pass arms the bee that very tick.
    #[test]
    fn the_wake_pass_reads_the_pose_settled_last_tick() {
        let cmd = PlayerCommand::default();
        let far = PlayerPose::level(200 << 8, 200 << 8, 6000, 0);
        let near = PlayerPose::level(15 << 8, 15 << 8, 6000, 0);
        let sleeper = |w: &mut World| {
            let g = w.g.ground_z(10 << 8, 10 << 8) as i16;
            let c =
                w.g.spawn_creature(4, 10 << 8, 10 << 8, g)
                    .expect("creature");
            w.g.ent[c].f58 = 0;
            w.g.ent[c].f59 = 0;
            c
        };

        // NATIVE: the near pose is the ARG of tick 2, but the gate
        // reads the pose settled during tick 1 (far) — the creature
        // sleeps one more tick and wakes on tick 3.
        let mut w = possess_world();
        let c = sleeper(&mut w);
        w.tick(far, cmd);
        assert_eq!(w.g.ent[c].f58 & 0xFF, 0, "far pose: asleep");
        w.tick(near, cmd);
        assert_eq!(
            w.g.ent[c].f58 & 0xFF,
            0,
            "native: the gate reads the carpet settled LAST tick"
        );
        w.tick(near, cmd);
        assert_eq!(
            w.g.ent[c].f58 & 0xFF,
            16,
            "the echo caught up: re-armed to 16"
        );

        // REPLAY DRIVERS (`strict_retail`): the arg IS settled(N−1),
        // so the same jump wakes the creature a tick earlier.
        let mut w = possess_world();
        let c = sleeper(&mut w);
        w.strict_retail = true;
        w.tick(far, cmd);
        w.tick(near, cmd);
        assert_eq!(
            w.g.ent[c].f58 & 0xFF,
            16,
            "strict_retail: the player arg is the gate pose"
        );
    }

    /// **THE RIVAL'S DEATH ARM WRITES `+70 = 2` AND NOTHING ELSE**
    /// (:17980-83). The human's twin (:55424-29) also zeroes `+46`
    /// and screams — but that arm lives in `sub_45C90`, which a rival
    /// never runs; its carpet tick is `sub_13170`. So the corpse
    /// enters the fall AT ITS LIVE CLIMB RATE and spends it on the
    /// very first fall step, on TOP of the speed-0 sink
    /// ([`death_sink_runs`]).
    ///
    /// ⚠ The witness this test used to cite (mc1hwl0 t=23252, "z 2769
    /// → 2713") is not in the corpus — mc1hwl0's rival at that tick is
    /// ALIVE. **mc1l49 rival 594 is the real one and it pins both
    /// laws at once** (measured off the recording, `f126 = 0`
    /// throughout, ground 4064):
    ///
    /// ```text
    ///   t=39083  +70=1  +46=-60  z=4707     alive
    ///   t=39084  +70=2  +46=-60  z=4711     death arm: +46 RIDES THROUGH
    ///   t=39085  +70=2  +46=-62  z=4643     first fall step: -68 = -60 - 8
    ///   t=39086  +70=2  +46=-64  z=4581     -62, no sink
    ///   t=39087  +70=2  +46=-66  z=4517     -64, no sink
    /// ```
    ///
    /// The first step is the inherited `-60` PLUS the speed-0 sink's
    /// 8; from the second on the sink is silent because z has dropped
    /// below `ground + row156.v_10` (4711 is above that band, 4643 is
    /// not). Both arms below reproduce that: the corpse starts 4000
    /// above the floor, so the sink is live for the whole test, and
    /// the second arm turns it off with speed alone.
    /// Non-vacuous: restoring the `f46 = 0` write parks the corpse.
    #[test]
    fn a_rival_corpse_enters_the_fall_at_its_live_climb_rate() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // Away from the keep: the authored castle is BOUND at its
        // mint (`wizext+50`, :54980), and a rival inside its own
        // castle's box DISCARDS its mailbox (:17975-78) — the packet
        // below must reach the intake. See [`rival_castle_register`].
        w.g.ent[i].x = w.g.ent[i].x.wrapping_add(0x4000);
        // Airborne, climbing, and one lethal packet in the box.
        w.g.ent[i].tick70 = 1;
        w.g.ent[i].f46 = -56;
        w.g.ent[i].z = w.g.ent[i].z.wrapping_add(4000);
        w.rivals[ri].grace = 0;
        w.g.ent[i].act_life = 10;
        w.g.ent[i].mail[0] = (10_000, 7);
        let z0 = w.g.ent[i].z;
        w.rival_entity_tick(i);
        assert_eq!(w.g.ent[i].tick70, 2, "the intake killed it into the fall");
        assert_eq!(
            w.g.ent[i].f46, -56,
            "the death arm leaves +46 alone — no human-side zero"
        );
        assert_eq!(w.g.ent[i].z, z0, "the death tick itself does not fall");
        // The next tick IS the fall. The fall's own speed decay drives
        // the corpse's `+126` to 0 BEFORE the mover reads it, so this
        // corpse spends the inherited rate AND the sink's 8.
        w.rival_entity_tick(i);
        assert_eq!(w.g.ent[i].f126, 0, "the corpse reached the mover speed-0");
        assert_eq!(
            w.g.ent[i].z,
            z0.wrapping_add(-64i16),
            "the first fall step drops the inherited climb rate plus the speed-0 sink"
        );
        assert_eq!(w.g.ent[i].f46, -58, "and gravity steps it by 2");
    }

    /// ⭐ THE FALLING CORPSE IS RELINKED BEFORE ITS TRAIL PUFF IS MINTED
    /// — see [`fall_relink_first`]. A corpse that crosses into a new
    /// tile on a fall step must sit BEHIND the puff that step mints:
    /// retail's mover relinks it (`sub_41C70`, :55250-52) and the
    /// `(10,1)` head-inserts after (:55480). The port minted first.
    /// The fall is run once to learn the step, then again from a pose
    /// one step short of a tile edge.
    /// NON-VACUITY: `MGC_NO_MC1_FALL_RELINK_FIRST=1` hands the tile
    /// back as carpet → puff and the head assert fails.
    #[test]
    fn a_falling_corpse_enters_a_tile_ahead_of_its_own_trail_puff() {
        let rig = |seat: Option<(u16, u16)>| -> (World, usize, (u16, u16), (u16, u16)) {
            let mut w = rebound_world();
            let i = w.rivals[0].ent as usize;
            let (x, y, z) = {
                let e = &w.g.ent[i];
                (e.x.wrapping_add(0x4000), e.y, e.z.wrapping_add(4000))
            };
            let (x, y) = seat.unwrap_or((x, y));
            w.g.move_relink(i, x, y, z);
            w.g.ent[i].tick70 = 1;
            w.g.ent[i].f46 = -56;
            w.g.ent[i].f126 = 160;
            w.rivals[0].grace = 0;
            w.g.ent[i].act_life = 10;
            w.g.ent[i].mail[0] = (10_000, 7);
            w.rival_entity_tick(i);
            assert_eq!(w.g.ent[i].tick70, 2, "the intake killed it into the fall");
            let from = (w.g.ent[i].x, w.g.ent[i].y);
            w.rival_entity_tick(i);
            let to = (w.g.ent[i].x, w.g.ent[i].y);
            (w, i, from, to)
        };
        let (_, _, from, to) = rig(None);
        let (dx, dy) = (
            to.0.wrapping_sub(from.0) as i16,
            to.1.wrapping_sub(from.1) as i16,
        );
        assert!(
            dx != 0 || dy != 0,
            "rig: the corpse moves on its first fall step"
        );
        // One unit short of the edge the step is heading for.
        let edge = |v: u16, d: i16| match d {
            d if d > 0 => (v & 0xFF00) | 0xFF,
            d if d < 0 => v & 0xFF00,
            _ => (v & 0xFF00) | 0x80,
        };
        let (w, i, from, to) = rig(Some((edge(from.0, dx), edge(from.1, dy))));
        assert_ne!(
            (from.0 >> 8, from.1 >> 8),
            (to.0 >> 8, to.1 >> 8),
            "rig: the fall step crosses a tile edge"
        );
        let cell = crate::engine::features::tile((to.0 >> 8) as u8, (to.1 >> 8) as u8);
        let head = w.g.map_entity[cell] as usize;
        assert_eq!(
            (
                w.g.ent[head].class64,
                w.g.ent[head].model65,
                w.g.ent[head].id24
            ),
            (10, 1, w.rivals[0].ent),
            "the tile's head is the corpse's own trail puff"
        );
        assert_eq!(
            w.g.ent[head].next20 as usize, i,
            "and the corpse sits right behind it"
        );
    }

    /// ⭐ THE NATIVE HUMAN IS SEATED WHERE HE IS MINTED, UNDER HIS OWN
    /// BOOK — see [`native_human_seat`]. Record 0 of all 115 whole
    /// takes holds the carpet at the TAIL of his tile's chain, the
    /// tokens minted after him in front.
    /// NON-VACUITY: `MGC_NO_NATIVE_HUMAN_SEAT=1` leaves the seat
    /// unseeded (`cell == usize::MAX`).
    #[test]
    fn the_native_human_is_seated_at_his_mint_behind_his_book() {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let things = vec![Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model: 4,
            x: 120,
            y: 120,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }];
        let mut w = World::new(planes, &things, 1, assets());
        let cell = crate::engine::features::tile(120, 120);
        assert_ne!(w.mc1_carpet_slot, 0, "rig: the carpet is pooled");
        let seat = w.g.player_chain;
        assert_eq!(
            seat.cell, cell,
            "seated on his start tile by the constructor"
        );
        w.grant_spells(&[0, 3]);
        let head = w.g.map_entity[cell] as usize;
        assert_eq!(
            (w.g.ent[head].class64, w.g.ent[head].model65),
            (12, 3),
            "rig: the book's last token heads his tile"
        );
        assert_eq!(
            w.g.player_chain, seat,
            "and the tokens went in AHEAD of him: his successor did not move"
        );
        assert_ne!(
            w.g.player_chain.next as usize, head,
            "he is not the head's predecessor"
        );
    }

    /// The SPEED arm of the same law (`sub_455D0` :55171-72): the
    /// sink's `!actSpeed` guard is real, so a corpse still carrying
    /// speed spends its `+46` and NOTHING else — the same first fall
    /// step as above, 8 units shallower. Pins the guard the other way
    /// round, so the sink cannot be turned into an unconditional −8.
    #[test]
    fn a_moving_corpse_takes_no_speed_zero_sink() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // Away from the keep: the authored castle is BOUND at its
        // mint (`wizext+50`, :54980), and a rival inside its own
        // castle's box DISCARDS its mailbox (:17975-78) — the packet
        // below must reach the intake. See [`rival_castle_register`].
        w.g.ent[i].x = w.g.ent[i].x.wrapping_add(0x4000);
        w.g.ent[i].tick70 = 1;
        w.g.ent[i].f46 = -56;
        // Well clear of one 16/tick decay step, so the mover still
        // sees a nonzero speed on the first fall tick.
        w.g.ent[i].f126 = 160;
        w.g.ent[i].z = w.g.ent[i].z.wrapping_add(4000);
        w.rivals[ri].grace = 0;
        w.g.ent[i].act_life = 10;
        w.g.ent[i].mail[0] = (10_000, 7);
        let z0 = w.g.ent[i].z;
        w.rival_entity_tick(i);
        assert_eq!(w.g.ent[i].tick70, 2, "the intake killed it into the fall");
        w.rival_entity_tick(i);
        assert_ne!(w.g.ent[i].f126, 0, "still moving on the first fall step");
        assert_eq!(
            w.g.ent[i].z,
            z0.wrapping_add(-56i16),
            "a moving corpse spends only its +46"
        );
    }

    /// `sub_132B0`'s at-castle probe (:17971-72, CARPET.EXE file
    /// 0x2BBF9-0x2BC3D) is `v14 = wizext+50; if (v14 && sub_11950(a1,
    /// pool + 164*v14))` — the REGISTER word and the summed-extent
    /// overlap, nothing else. The port used to demand the castle's
    /// `flags & 2` first-commit latch, which an AUTHORED castle never
    /// earns, so a rival parked on his own keep drew the afield
    /// `/2000` floored 100 instead of the at-castle `/200` floored
    /// 1000 (mc1l20 slot 518 from t=2, x10 under-funded for the whole
    /// pre-upgrade phase). `MGC_NO_MC1_RIVAL_CASTLE_REGISTER=1`
    /// restores the latch read and this test then fails.
    #[test]
    fn a_rival_on_its_unlatched_authored_keep_earns_the_at_castle_regen() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let reg = w.wiz_castle_reg(w.rivals[ri].slot) as usize;
        assert!(reg != 0, "non-vacuity: the starting castle binds wizext+50");
        assert_eq!(
            w.g.ent[reg].flags & 2,
            0,
            "…and it has never committed a level-up"
        );
        assert!(
            w.g.ent_overlap(i, reg),
            "…and the rival sits inside its box"
        );
        w.rivals[ri].mana = 0;
        w.rival_alive_tick(ri, i);
        let at_castle = ((w.rivals[ri].mana_max / 200) as i32).max(1000);
        assert_eq!(
            w.rivals[ri].mana_delta, at_castle,
            "the at-castle rate, not the afield 100"
        );
    }

    /// `sub_13A70` (Home, :18204-27; CARPET.EXE 0x13A70-0x13B94,
    /// disassembled whole) never writes the brain byte: a healed rival
    /// keeps aiming (+34 at 0x13B16) and approaching its castle every
    /// tick until the think-tick cascade re-picks. The port dropped it
    /// to `Fresh` (an empty handler) the tick `act_life` reached
    /// `max_life`, freezing +34 (mc1l26 t=4975/4982/13970, mc1l26-froze
    /// t=22198). `MGC_NO_MC1_HOME_KEEPS_STATE=1` restores the drop and
    /// this test then fails.
    #[test]
    fn a_healed_rival_at_home_keeps_its_state() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        assert!(
            w.rival_castle(w.rivals[ri].ent).is_some(),
            "non-vacuity: a castle to aim at"
        );
        w.rivals[ri].state = AiState::Home;
        w.g.ent[i].act_life = w.g.ent[i].max_life as i32;
        w.rival_state_tick(ri, i, false);
        assert!(
            matches!(w.rivals[ri].state, AiState::Home),
            "Home writes no brain byte"
        );
    }

    /// ⭐⭐⭐ THE DEATH SCATTER DOES NOT BLANK `+676`
    /// (`sub_45FC0_46300` :55516-49 walks the `+532` ACQUISITION list
    /// and nothing else). `sub_45C10_45F50` is the register's ONLY
    /// writer in the whole MC1 binary and the class-3 dispatch table
    /// reaches it from the LIVE arms alone, so a corpse's book freezes
    /// at what its last live tick published and stays readable through
    /// the husk — by the castle probe (:56629), the Create-Castle /
    /// mana-ball elections (:54662, :54669) and the trigger walk
    /// (:64794). Pins the invented blank out.
    /// WITNESS: mc1l49 t=9640→9641, wiz 2 (ent 620) scatters, its
    /// `+532` list is rewritten 621→0, 622→3, 623→2 … and its `+676`
    /// still reads the intact slot book 621..644 at t=9641.
    #[test]
    fn a_rival_death_scatter_leaves_the_owned_register_standing() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // Away from the keep: the authored castle is BOUND at its
        // mint (`wizext+50`, :54980), and a rival inside its own
        // castle's box DISCARDS its mailbox (:17975-78) — the packet
        // below must reach the intake. See [`rival_castle_register`].
        w.g.ent[i].x = w.g.ent[i].x.wrapping_add(0x4000);
        // One lethal packet in the box: this tick is the last LIVE
        // tick, so it runs `sub_45C10` and then dies into the fall.
        w.g.ent[i].tick70 = 1;
        w.rivals[ri].grace = 0;
        w.g.ent[i].act_life = 10;
        w.g.ent[i].mail[0] = (10_000, 7);
        w.rival_entity_tick(i);
        assert_eq!(w.g.ent[i].tick70, 2, "the intake killed it into the fall");
        let book = w.rivals[ri].owned;
        assert!(
            book.iter().any(|&m| m != 0),
            "non-vacuous: the corpse enters the fall holding a book"
        );
        // Fall to the impact — the scatter arm, which flips +70 to 3.
        for _ in 0..2000 {
            if w.g.ent[i].tick70 != 2 {
                break;
            }
            w.rival_entity_tick(i);
        }
        assert_eq!(w.g.ent[i].tick70, 3, "the corpse reached the scatter");
        assert!(
            w.rivals[ri].acq.iter().all(|&e| e < SPELL_COUNT as i32),
            "the scatter rewrote +532 to DEAD (model) form: {:?}",
            w.rivals[ri].acq
        );
        assert_eq!(
            w.rivals[ri].owned, book,
            "+676 is untouched — sub_45C10 is its only writer and the \
             dispatch stopped calling it when +70 left the live arms"
        );
    }

    /// ⭐⭐⭐ A RETAIL CORPSE KEEPS DEFLECTING. The rival death arm is
    /// `*(a1 + 70) = 2; return 0;` and NOTHING else (:17980-83), and
    /// `+17` bit 7 — our `0x8000` — has EXACTLY TWO writers in the
    /// whole listing, both inside the Rebound token's own tick
    /// `sub_573F0_57920` (:65785 `&= ~0x80u`, :65792 `|= 0x80u`; twin
    /// remc1hw :62007 / :62014). Nothing on the death path writes it,
    /// so the bit a live burst published rides straight through the
    /// kill and only lapses when the token's `+48` runs out — see
    /// [`World::rival_rebound_token_tick`].
    ///
    /// The port's clear in the death arm was an INVENTED WRITE, and
    /// the comment that justified it ("death states 2/3 never reach
    /// `rival_refresh_buffs`") went stale when the token moved to the
    /// class-12 walk.
    ///
    /// WITNESS: mc1l49 t=6669 — slot 646 takes a fatal 4000
    /// (`act_life 2040 → −1960`) and `explain`'s changelog lists
    /// `act_life`, `f38`, `f63`, `f70`, `mail0.src` and NO `flags`
    /// row; the word reads 0x800C at t=6666..6668, stays 0x800C
    /// across the death, and the deflection bit only leaves at the
    /// respawn (t≈6750, 0x000C).
    ///
    /// NON-VACUITY: under `MGC_NO_MC1_DEATH_KEEPS_REBOUND=1` the
    /// death arm blanks the bit and the final assertion fails.
    #[test]
    fn a_dying_rival_carries_its_rebound_bit_into_the_corpse() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        // Away from the keep: the authored castle is BOUND at its
        // mint (`wizext+50`, :54980), and a rival inside its own
        // castle's box DISCARDS its mailbox (:17975-78) — the packet
        // below must reach the intake. See [`rival_castle_register`].
        w.g.ent[i].x = w.g.ent[i].x.wrapping_add(0x4000);
        // A live Rebound burst on the books, its deflection bit
        // already published on the wizard entity the way the token's
        // own tick publishes it (sub_573F0 :65792).
        let m = w.rivals[ri].owned[14] as usize;
        assert!(m != 0, "the rival never minted a Rebound manifestation");
        w.g.ent[m].f26 = SPELLS[14].count as i16;
        w.g.ent[i].flags |= 0x8000;
        assert!(
            rebound_bit(&w, ri),
            "non-vacuous: the bit stands before the killing tick"
        );
        // One lethal packet in the box on a live tick with no spawn
        // grace left: the intake takes `act_life` negative and the
        // dispatch falls into the death arm.
        w.g.ent[i].tick70 = 1;
        w.rivals[ri].grace = 0;
        w.g.ent[i].act_life = 10;
        w.g.ent[i].mail[0] = (10_000, 7);
        w.rival_entity_tick(i);
        assert!(
            w.g.ent[i].act_life < 0,
            "non-vacuous: the packet was lethal"
        );
        assert_eq!(w.g.ent[i].tick70, 2, "the intake killed it into the fall");
        assert_eq!(
            w.g.ent[m].f26, SPELLS[14].count as i16,
            "the burst is untouched — only the token's own tick spends it"
        );
        assert!(
            rebound_bit(&w, ri),
            "the death arm blanked +17 bit 7 — retail's `*(a1+70) = 2; \
             return 0;` writes no flags and the corpse keeps deflecting"
        );
    }

    /// ⭐⭐⭐ THE GRAVE RE-POINT WALKS THE TICK-TOP BALL ROSTER, NOT
    /// THE LIVE POOL — SO A SEVERED CHAIN LEAVES THE DEAD WIZARD'S
    /// BALLS ON HIS OWN `+144`.
    ///
    /// `sub_45C10_45F50`'s landing block seeds from `var_u32_36462[1]`
    /// (HIDDEN VA 0x46681 `mov 0x8e72(%ebp),%ebp`) and steps
    /// `->next` (0x466be `mov 0x0(%ebp),%ebp`) to the pool-base
    /// terminator — the same tick-top roster the possession acquire
    /// and the magnet walk, with the same [`TickChain::cut`] law: a
    /// member REALLOCATED mid-tick has its `+0` link wiped, and an
    /// MC1 SEIZURE blanks the head outright, so the rest of the
    /// roster is unreachable for the rest of that tick.
    ///
    /// mc1hwl2 t=15762 is the witness and it certifies the take.
    /// Rival 3's corpse (slot 449) lands, its 24-jar scatter empties
    /// the free stack and seizes, every roster head goes null, and
    /// retail's walk therefore re-points NOTHING: slots 875/938/953
    /// keep `+144 = 449` and their 5000 + 134 + 3000 stays on the
    /// dead wizard's ceiling. The port's ascending POOL scan moved
    /// all three to the grave, and the NEXT tick's census
    /// (`recompute_mana`) read rival 449 back at the intrinsic 1000
    /// while the 8134 landed on the grave's own `+136` through the
    /// owner-credit fallback — the take's ONLY divergence in 28,580
    /// ticks (t=15763: slot 132 `mana_max` retail 0 / port 8134,
    /// slot 449 retail 9134 / port 1000).
    ///
    /// PAIR-BLIND, hence a unit test: the head is INHERITED (the
    /// 15762→15763 pair is clean — the importer hands the port
    /// retail's own `+144` rows), and `+144`/`ball_chain` are not in
    /// the graded projection at all.
    #[test]
    fn the_grave_repoint_walks_the_tick_top_ball_roster() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let me = w.rivals[ri].ent;

        // Three of the rival's own mana balls, ASCENDING by slot, so
        // a pool scan and a chain walk disagree only once the chain
        // is cut.
        let ball = |w: &mut World| {
            let (x, y) = (110u16 << 8, 110u16 << 8);
            let z = w.g.ground_z(x, y) as i16;
            let s = w.g.spawn_mana_ball(x, y, z).expect("mana ball");
            w.g.ent[s].f144 = me;
            w.g.ent[s].f140 = 1000;
            s
        };
        let head = ball(&mut w);
        let mid = ball(&mut w);
        let tail = ball(&mut w);
        assert!(head < mid && mid < tail, "roster order is by slot");

        // TOP OF TICK: the roster is filed with all three.
        w.g.rebuild_ball_chain();
        assert_eq!(w.g.ball_chain.visible_len(), 3, "all three are members");

        // MID-TICK: `head` is freed and immediately re-popped by some
        // other spawner, which wipes its `+0` link. `new_event` lowers
        // the cut to 1 — the walk sees the reallocated node and
        // NOTHING PAST IT.
        w.g.free_entity(head);
        let reborn = w.g.new_event().expect("reborn");
        assert_eq!(reborn, head, "the free stack re-pops the same slot");
        w.g.ent[head].class64 = 10;
        w.g.ent[head].model65 = 39;
        w.g.ent[head].f144 = me;
        assert_eq!(
            w.g.ball_chain.visible_len(),
            1,
            "the walk stops one node past the reuse"
        );
        // Non-vacuity: both survivors are still live, still the
        // rival's, and still perfectly good candidates — only the CUT
        // hides them, so a POOL scan takes all three.
        for b in [mid, tail] {
            assert!(
                w.g.ent[b].class64 == 10 && w.g.ent[b].model65 == 39 && w.g.ent[b].f144 == me,
                "the trap balls are live in the pool"
            );
        }

        // Round 168: the corpse lands with the fatal letter standing.
        w.g.ent[i].mail[0] = (6664, 472);
        w.g.ent[i].mail[3] = (3000, 0);
        w.rival_death_impact(ri, i);
        assert_eq!(
            w.g.ent[i].mail,
            [(0, 0); 6],
            "the touchdown memsets the mailbox (:55518; mc1hwl0 t=6809 slot 473)"
        );

        let gv = (1..w.g.ent.len())
            .find(|&j| w.g.ent[j].class64 == 10 && w.g.ent[j].model65 == 40)
            .expect("the landing planted a (10,40) grave");
        assert_eq!(
            w.g.ent[head].f144, gv as u16,
            "the one VISIBLE roster member is re-pointed at the grave"
        );
        assert_eq!(
            (w.g.ent[mid].f144, w.g.ent[tail].f144),
            (me, me),
            "the members PAST the severed link keep the dead wizard's +144 — \
             retail walks the roster, never the pool (mc1hwl2 t=15762)"
        );
    }

    /// The respawn block (sub_44D30 :54842-923 + :55031-41), rival
    /// side — mc1hwl0 t=20761/20762: the reborn wizard copies the
    /// castle's WHOLE axis (:54858-61, no ground+256 tile-up), keeps
    /// the fall's +46/+126 registers (only v_12/v_16/knock clear,
    /// :54868-83), resets the capacity mirror to the base 1000 while
    /// the +132 delta seat CARRIES the pre-death debit, and the
    /// re-minted (12,16) wears the STANDING castle's ladder price
    /// (:55034's sub_47DD0 call). Pinned by unit because the human's
    /// mana lane carries a pre-existing import-side residue in the
    /// t=20760 pair window (the census undercount, ledger SESSION 52),
    /// which poisons any pair fixture cut there.
    #[test]
    fn the_rival_respawn_copies_the_castle_axis_and_keeps_the_fall_registers() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let c = w.rival_castle(w.rivals[ri].ent).expect("starting castle");
        {
            let e = &mut w.g.ent[c];
            e.z = e.z.wrapping_add(300); // a datum OFF the ground plane
            e.flags |= 2; // the first-commit bind latch
        }
        let cz = w.g.ent[c].z;
        // The corpse enters the respawn with the fall's registers
        // live and a debit still pending in the delta seat.
        {
            let e = &mut w.g.ent[i];
            e.f46 = -56;
            e.f126 = 160;
            e.f136 = 14768;
        }
        {
            let r = &mut w.rivals[ri];
            r.mana_delta = -1000;
            r.vdes = 77;
            r.jink = 7;
            r.knock_mag = 5;
            // The death rewrote the acq list to banked MODELS.
            r.acq = [-1; SPELL_COUNT];
            r.acq[0] = 16;
        }
        w.rival_respawn(ri, i);
        let e = &w.g.ent[i];
        assert_eq!(
            (e.z, e.f46, e.f126),
            (cz, -56, 160),
            "castle axis copied; the fall's +46/+126 ride through"
        );
        assert_eq!(e.f136, 1000, "capacity resets to the base pool");
        let r = &w.rivals[ri];
        assert_eq!(
            (r.mana, r.mana_delta),
            (1000, -1000),
            "fresh purse; the +132 delta seat is CARRIED"
        );
        assert_eq!(
            (r.vdes, r.jink, r.knock_mag),
            (0, 0, 0),
            "exactly the three flight registers clear"
        );
        let m = r.owned[16] as usize;
        assert!(m != 0, "the book re-minted the castle spell");
        assert_eq!(w.g.ent[m].z, cz, "the token minted at the wizard's seat");
        let lvl = w.g.ent[c].f26 as usize;
        assert!(lvl > 0, "the starting castle stands leveled");
        assert_eq!(
            (w.g.ent[m].f136, w.g.ent[m].f140),
            (Gen::CASTLE_CAP[lvl], Gen::CASTLE_CAP[lvl] / 101),
            "the fresh (12,16) wears the standing castle's ladder price"
        );
    }

    /// The respawn's WIZEXT half (sub_44D30 tail, :55031-48 /
    /// hw:51110-22): the +48 burst counter and the +724 cooldown
    /// array SURVIVE the respawn — the function writes exactly ONE
    /// cooldown entry (`var_756 = 4·slot` = cooldown[16]) and no
    /// burst — and the stall block is SET to 16 (`memset(u8_333,
    /// 16, 8)`), not cleared. The port's blanket clears let the
    /// reborn picker reach spells on a schedule retail's carried
    /// cooldowns refuse: mc1hwl0 t=21275 (500 ticks after the
    /// t=20761 respawn) the port committed Wall of Fire where retail
    /// committed Fireball, so retail's t=21276 fireball emission —
    /// the tick's FIRST free-stack pop — never happened in the port
    /// and every spawn of the mass fire-spread tick landed one slot
    /// late (the missing-(10,0)-at-951 head). Pinned by unit: the
    /// head is INHERITED (the pair import restores the wizext from
    /// retail state).
    #[test]
    fn the_respawn_keeps_burst_and_cooldowns_and_stamps_the_stall() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        {
            let r = &mut w.rivals[ri];
            r.burst = 2;
            r.cooldown = [0; SPELL_COUNT];
            r.cooldown[17] = 3;
            r.cooldown[20] = 5;
            r.cooldown[16] = 9; // overwritten by the one respawn write
            r.regen_stall = 0;
            r.acq = [-1; SPELL_COUNT];
        }
        w.rival_respawn(ri, i);
        let r = &w.rivals[ri];
        assert_eq!(r.burst, 2, "the +48 burst counter rides through");
        assert_eq!(
            (r.cooldown[17], r.cooldown[20]),
            (3, 5),
            "carried cooldowns survive — no blanket clear"
        );
        assert_eq!(
            r.cooldown[16],
            4 * r.slot as u16,
            "the ONE cooldown write: var_756 = 4*slot"
        );
        assert_eq!(r.regen_stall, 16, "the stall block is SET to 16");
    }

    /// ⭐⭐⭐ `sub_44D30`'s WIZEXT CLEAR LIST IS EXACT, and the port's
    /// copy was wrong on three lanes at once.
    ///
    /// - `:54868-70` is `v_24 = 0; v_26 = 0; v_22 = 0` — the knock
    ///   DIRECTION as well as the magnitude. The port cleared only
    ///   `knock_mag`.
    /// - the rival-only tail (`:55043-46`) is
    ///   `for (kx = 0; kx < 8; kx++) str_456[kx].var_u16_4 = 24607`;
    ///   `var_u16_4` is `+460`, the HATE half of the 8-byte row. The
    ///   WAR flag at `+462` is not in the loop and appears nowhere
    ///   else in the function, so a reborn rival KEEPS every war he
    ///   declared.
    /// - `+406`, the poverty latch, is never written by the respawn
    ///   at all — it belongs to the attack picker (`:19468-91`).
    ///
    /// mc1l49 t=21968, rival 594's respawn (`explain` + the raw
    /// wizext shadow): retail's `+24` goes 1300 → 0 while `poverty`
    /// stays 1 and `war[6]` stays 1 across the whole transition. Both
    /// arms are pinned, so the test still means something under
    /// `MGC_NO_MC1_RESPAWN_CLEAR_LIST=1`.
    /// ⭐ THE POVERTY LATCH READS THE PURSE SIGNED (`sub_16030` /
    /// `sub_16310` heads, CARPET.EXE 0x16036 / 0x16316 — see
    /// [`crate::engine::features::no_mc1_rival_poverty_signed_purse`]).
    /// A fatal shield quarter leaves the purse wrapped negative while
    /// the brain's state handler still runs; retail latches (and a
    /// standing latch HOLDS), the `u32` read released it.
    #[test]
    fn the_poverty_latch_reads_a_wrapped_purse_as_poor() {
        let mut w = possess_world();
        let ri = 0;
        let wrapped = (-400i32) as u32;
        let law = !crate::engine::features::no_mc1_rival_poverty_signed_purse();
        // An un-latched rival on the fatal tick.
        w.rivals[ri].mana_max = 20_000;
        w.rivals[ri].mana = wrapped;
        w.rivals[ri].poverty = false;
        let pick = w.rival_attack_pick(ri, true);
        if law {
            assert!(
                w.rivals[ri].poverty,
                "−400 < 20000/4 (`jle` at 0x1604F): the latch sets"
            );
            assert_eq!(pick, None, "a latched picker holds");
        } else {
            assert!(
                !w.rivals[ri].poverty,
                "the u32 arm reads ~4.29e9: never poor"
            );
        }
        // A STANDING latch on the fatal tick (both pickers' release leg).
        for vs_wizard in [true, false] {
            w.rivals[ri].mana = wrapped;
            w.rivals[ri].poverty = true;
            let pick = w.rival_attack_pick(ri, vs_wizard);
            if law {
                assert!(
                    w.rivals[ri].poverty,
                    "−400 is below the release line: the latch holds"
                );
                assert_eq!(pick, None);
            } else {
                assert!(!w.rivals[ri].poverty, "the u32 arm released the latch");
            }
        }
        // Ordinary purses are untouched by the law: the boundary
        // itself releases (q + 6000 = 11000 < max: the `v3 > +140` leg).
        w.rivals[ri].mana = 11_000;
        w.rivals[ri].poverty = true;
        let _ = w.rival_attack_pick(ri, true);
        assert!(
            !w.rivals[ri].poverty,
            "reaching q + 6000 releases in both arms"
        );
        w.rivals[ri].mana = 4_999;
        let _ = w.rival_attack_pick(ri, true);
        assert!(w.rivals[ri].poverty, "under max/4 latches in both arms");
    }

    /// ⭐ THE BULLY ARM READS BOTH PURSES SIGNED (`sub_145B0`
    /// :18570-72, CARPET.EXE 0x14719-29 `add; cmp; jge` — see
    /// [`crate::engine::features::no_mc1_rival_bully_signed_purse`]).
    /// A wrapped-negative candidate is the poorest wizard alive; a
    /// wrapped-negative picker bullies nobody.
    #[test]
    fn the_bully_arm_reads_wrapped_purses_signed() {
        let mut w = possess_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let law = !crate::engine::features::no_mc1_rival_bully_signed_purse();
        w.g.rebuild_wiz_chain();
        w.human_bucket_alive = true;
        w.player.invisible = false;
        w.player.owned[12] = 0;
        // The human is an unbound castle-knower (:18570-71) …
        w.player.owned[16] = 1;
        assert_eq!(w.wiz_castle_reg(0), 0);
        // … never hated, never at war: only the bully leg can elect him.
        w.rivals[ri].hate[0] = 0;
        w.rivals[ri].war[0] = false;
        w.rivals[ri].owned[16] = 0; // the picker's own self-test passes
        let e = &w.g.ent[i];
        w.human_pose = (e.x, e.y, e.z);
        // 1. The CANDIDATE's purse is wrapped (a corpse's held shortfall).
        w.player.mana = (-400i32) as u32;
        w.rivals[ri].mana = 5_000;
        w.rivals[ri].state = AiState::Cruise;
        let picked = w.rival_pick_wizard_target(ri, i);
        assert_eq!(
            picked, law,
            "−400 + 32·(255−200) = 1360 < 5000 bullies the corpse; the u32 arm reads ~4.29e9"
        );
        // 2. The PICKER's own purse is wrapped (its fatal tick).
        w.player.mana = 0;
        w.rivals[ri].mana = (-400i32) as u32;
        w.rivals[ri].state = AiState::Cruise;
        let picked = w.rival_pick_wizard_target(ri, i);
        assert_eq!(
            picked, !law,
            "0 + 1760 < −400 is false in retail; the u32 arm read the picker as rich"
        );
        // Ordinary purses: the bully leg is unchanged.
        w.rivals[ri].mana = 5_000;
        w.rivals[ri].state = AiState::Cruise;
        assert!(
            w.rival_pick_wizard_target(ri, i),
            "1760 < 5000 bullies in both arms"
        );
    }

    #[test]
    fn the_respawn_clears_the_knock_direction_and_keeps_war_and_poverty() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        {
            let r = &mut w.rivals[ri];
            r.knock_dir = 1300;
            r.knock_mag = 40;
            r.poverty = true;
            r.war = [false; 8];
            r.war[6] = true;
            r.hate = [0; 8];
            r.acq = [-1; SPELL_COUNT];
        }
        w.rival_respawn(ri, i);
        let r = &w.rivals[ri];
        assert_eq!(r.knock_mag, 0, "v_22 is cleared on every arm");
        assert_eq!(
            r.hate, [HATE_NEUTRAL; 8],
            "str_456[kx].var_u16_4 = 24607 on every arm"
        );
        if respawn_clear_list() {
            assert_eq!(r.knock_dir, 0, "v_24 = 0 — the knock DIRECTION too");
            assert!(r.poverty, "+406 survives: sub_44D30 never writes it");
            assert!(r.war[6], "+462 survives: the ledger loop is hate-only");
        } else {
            assert_eq!(r.knock_dir, 1300, "pre-dig arm left the bearing standing");
            assert!(!r.poverty, "pre-dig arm blanked the latch");
            assert!(!r.war[6], "pre-dig arm blanked the war flags");
        }
    }

    /// ⭐⭐⭐ `sub_57610_57B40` BILLS BEFORE IT SPAWNS (:65880-83).
    /// The `if (v3)` null guard wraps only the castle ball and its
    /// in-transit `+48 = +50 − 1` pin; `sub_55E80_563B0` sits ABOVE
    /// it. A rival firing Create Castle into an exhausted pool pays
    /// the FULL ladder price, mints nothing, and leaves `+48` at the
    /// full count — the next tick's `sub_55DD0` afford leg then
    /// refuses on the emptied purse and releases the burst.
    /// mc1l49 t=15552: wiz 7's purse 40100 → 100 on a dry free stack,
    /// token 112 (`+136 = CASTLE_CAP[3] = 40000`) holding `+48 = 101`
    /// and no `(9,10)` ball anywhere in the take. Both arms are
    /// pinned so the test is meaningful under
    /// `MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER=1` too.
    #[test]
    fn a_dry_pool_castle_cast_is_billed_in_full() {
        let mut w = rebound_world();
        let ri = 0;
        let m = w.rivals[ri].owned[16] as usize;
        assert!(m != 0 && w.g.ent[m].class64 == 12, "the (12,16) token");
        let price = w.rival_castle_price(ri);
        assert!(price > 0, "the token carries a live ladder price");
        w.rivals[ri].mana = price + 1_000;
        w.rivals[ri].mana_delta = 0;
        // Arm the burst the way the commit does, then dry the pool.
        let count = w.spells()[16].count as i16;
        w.g.ent[m].f26 = count;
        while w.g.new_event().is_some() {}
        assert!(w.g.spawn_castle_ball(0, 0, 0).is_none(), "the pool is dry");
        let balls = |w: &World| {
            w.g.ent
                .iter()
                .filter(|e| e.class64 == 9 && e.model65 == 10 && e.flags & 0x400 == 0)
                .count()
        };
        let before = balls(&w);
        w.rival_castle_token_tick(m, ri);
        assert_eq!(balls(&w), before, "no ball was minted");
        assert_eq!(
            w.g.ent[m].f26, count,
            "the burst stays FULL — the `+48` pin is inside `if (v3)`"
        );
        if castle_ball_debit_order() {
            assert_eq!(
                w.rivals[ri].mana_delta,
                -(price as i32),
                "retail bills the ladder price ABOVE the spawn guard"
            );
        } else {
            assert_eq!(
                w.rivals[ri].mana_delta, 0,
                "the pre-dig arm billed nothing when the spawn refused"
            );
        }
    }

    // ---- round 154, w154c: the three round-147 MC2 laws, MC1 twins ---

    /// Three rivals (slots 1..3, start markers model 5/6/7 at three
    /// tiles), each with a level-1 starting castle so a dead one has a
    /// dead-wait to sit in rather than an elimination.
    fn three_rival_world() -> World {
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        let th = |model: u16, x: u16, y: u16| Thing {
            slot: 0,
            kind: ThingKind::Entity,
            class: 3,
            model,
            x,
            y,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        };
        let things = vec![th(5, 120, 120), th(6, 100, 100), th(7, 80, 80)];
        let mut w = World::new(planes, &things, 1, assets());
        let mut book = [false; SPELL_COUNT];
        book[0] = true;
        book[16] = true;
        let mut cfgs: [Option<RivalConfig>; 8] = Default::default();
        for slot in 1..4 {
            cfgs[slot] = Some(RivalConfig {
                aggression: 200,
                accuracy: 255,
                tempo: 255,
                castle_level: 1,
                book,
                allowed: book,
            });
        }
        w.set_wizards(&cfgs, 4);
        assert_eq!(w.rivals.len(), 3, "fixture: three rivals seated");
        w
    }

    /// Put a rival into its dead-wait by hand: state 3, life below
    /// zero, the countdown parked high so it does not respawn during
    /// the test. The next tick-top sweep drops it off bucket 0.
    fn park_dead(w: &mut World, ri: usize, countdown: i16) {
        let i = w.rivals[ri].ent as usize;
        let e = &mut w.g.ent[i];
        e.tick70 = 3;
        e.act_life = -100;
        e.f26 = countdown;
    }

    /// Round 168 ([`World::rival_add_hate`]): the hate ledger has no
    /// self test — a wizard whose own bolt feeds its own table takes
    /// the row under its own colour (mc1hwl14 t=62: `hate[3]` 24607 ->
    /// 24786 on wizard 3). `MGC_NO_MC1_SELF_HATE=1` leaves it flat.
    #[test]
    fn the_hate_ledger_has_no_self_test() {
        let mut w = three_rival_world();
        let own = w.rivals[0].slot;
        let other = w.rivals[1].slot;
        let h0 = w.rivals[0].hate[own as usize];
        w.rival_add_hate(0, own, 179);
        assert_eq!(w.rivals[0].hate[own as usize], h0 + 179, "its own colour");
        let o0 = w.rivals[0].hate[other as usize];
        w.rival_add_hate(0, other, 500);
        assert_eq!(w.rivals[0].hate[other as usize], o0 + 500, "and any other");
    }

    /// ⭐⭐⭐ THE POST-(RE)SPAWN TRUCE WALKS THE TICK-TOP BUCKET-0
    /// ROSTER (`sub_44D30` :55037-41, `CARPET.EXE` 0x5DAB6..0x5DB05),
    /// on the two RIVAL call paths. (1) Level-start seating walks an
    /// EMPTY chain, so every ledger starts NEUTRAL; the flat loop
    /// left the triangular `hate[j > slot] = 40927` residue
    /// (init-check mc1l14: 3 rows, mc1l20/l10: 1 each). (2) A rival
    /// in its dead-wait is off bucket 0 and takes NO truce when
    /// another colour respawns — mc1l14 t=1344 in miniature (rivals
    /// 1 and 3 dead, the human alive; retail stamped the human alone).
    ///
    /// Fails under `MGC_NO_MC1_TRUCE_ROSTER=1` at BOTH asserts.
    #[test]
    fn the_mc1_truce_walks_the_tick_top_roster_on_both_rival_paths() {
        let mut w = three_rival_world();
        // (1) SEATING: the chain the constructor's spawns walked was
        //     empty — nothing elevated anywhere.
        for r in &w.rivals {
            assert_eq!(
                r.hate, [HATE_NEUTRAL; 8],
                "slot {}: level-start seating walks an empty roster",
                r.slot
            );
        }
        // Settle so bucket 0 is built and populated.
        for _ in 0..8 {
            w.tick(away(), PlayerCommand::default());
        }
        // (2) RESPAWN WITH A DEAD SPECTATOR: rival 2 (slot 2) parks
        //     dead; rival 1 (slot 1) dies with its countdown at zero
        //     so the very next dead-wait tick respawns it. Rival 3
        //     stays alive as the positive control.
        park_dead(&mut w, 1, 2000);
        park_dead(&mut w, 0, 0);
        // One tick: the sweep rebuilds bucket 0 without rivals 1 and 2,
        // rival 1's dead-wait reaches 0 and `rival_respawn` runs its
        // truce loop against THAT roster.
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            w.g.ent[w.rivals[0].ent as usize].tick70, 1,
            "fixture: rival 1 respawned this tick"
        );
        // Slot 3 walks ABOVE slot 1, so its own alive tick has already
        // decayed the stamp by `256 - agg` = 56 by the boundary.
        assert_eq!(
            w.rivals[2].hate[1],
            HATE_RESPAWN - 56,
            "the LIVE spectator (slot 3) takes the truce toward the respawner              (then its own dispatch's decay, the same tick)"
        );
        assert_eq!(
            w.rivals[1].hate[1], HATE_NEUTRAL,
            "a rival in its dead-wait is off bucket 0 and takes NO truce \
             (mc1l14 t=1344: retail 24607, the flat loop's 40927)"
        );
    }

    /// ⭐ `sub_46480`'s FIRST statement (:55601, `CARPET.EXE` file
    /// 0x5EC85 `movw $0x0,0x16(%eax)`): every state-3 dispatch wipes
    /// the knock MAGNITUDE, above its AI/human fork; the bearing
    /// (+24) stands until the respawn. mc1l14 t=928: rival 2 lands
    /// with 76 still in `+22` and retail's first dead tick reads 0.
    ///
    /// Fails under `MGC_NO_MC1_DEAD_WAIT_KNOCK_CLEAR=1`.
    #[test]
    fn the_mc1_dead_wait_clears_the_knock_magnitude_every_tick() {
        let mut w = rebound_world();
        park_dead(&mut w, 0, 500);
        w.rivals[0].knock_mag = 76;
        w.rivals[0].knock_dir = 609;
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            (w.rivals[0].knock_mag, w.rivals[0].knock_dir),
            (0, 609),
            "the first state-3 tick zeroes +22 and leaves +24 standing"
        );
        // …and every dead tick, not just the first: a knock posted
        // onto the corpse is gone by the next boundary.
        w.rivals[0].knock_mag = 40;
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            w.rivals[0].knock_mag, 0,
            "wiped again on the next dead tick"
        );
        assert_eq!(
            w.g.ent[w.rivals[0].ent as usize].tick70, 3,
            "fixture: still in the dead-wait (the countdown is parked)"
        );
    }

    /// THE RIVAL KILL'S TALLY HOME (round 154, w154f; round 153
    /// finding #5). Retail's touchdown (:55488-97) credits the killer's
    /// per-victim `+30` tally and nothing else — `+359` (the creature
    /// kill counter) has one writer, `sub_1A6C0` (:21840-50), a class-5
    /// death handoff. mc1l49 t=9641: rival 620 dies to the human,
    /// retail `kills` 49 flat, the port 50.
    ///
    /// Fails under `MGC_NO_MC1_RIVAL_KILL_NO_TALLY=1`.
    #[test]
    fn a_rival_kill_credits_the_tally_and_not_the_creature_counter() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let slot = w.rivals[ri].slot as usize;
        w.g.ent[i].f38 = PLAYER_TARGET;
        w.g.kills = 5;
        w.rival_death_impact(ri, i);
        assert_eq!(
            w.kill_tally[0][slot], 1,
            "the killer's per-victim +30 tally"
        );
        assert_eq!(w.g.kills, 5, "+359 is the creature handoff's alone");
    }

    /// The knock BEARING is stored raw at :55714 — `sub_42150` can
    /// return 2048 (the `2048 − ATAN[0]` quadrant: the source a hair
    /// to +x and far to +y of the victim), and retail keeps it
    /// (mc1l10 t=4503, rival 2: retail 2048, port 0). The mover masks.
    ///
    /// Fails under `MGC_NO_MC1_KNOCK_DIR_RAW=1` (the port's `& 0x7FF`
    /// folds 2048 to 0).
    /// THE KRAKEN BUFFET DRAGS ANY WIZARD (round 154, w154j): an ON
    /// tick of `sub_1C4F0`'s counter writes the TARGET record's wizext
    /// `+24 = bearing + 0x400`, `+22 = 80` through `+146`'s `+160`
    /// pointer (CARPET.EXE 0x1C6EE/0x1C6F7/0x1C719), no owner test.
    /// A rival tethered by a kraken takes the pair at the kraken's
    /// walk slot; the human's own register is untouched. Fails under
    /// `MGC_NO_MC1_KRAKEN_BUFFET_RIVAL=1` (the human-only arm).
    #[test]
    fn the_kraken_buffet_drags_a_rival_wizard_too() {
        let mut w = rebound_world();
        let ri = 0;
        let r = w.rivals[ri].ent as usize;
        let (rx, ry) = (w.g.ent[r].x, w.g.ent[r].y);
        let g = w.g.ground_z(rx.wrapping_add(2 << 8), ry) as i16;
        let k =
            w.g.spawn_creature(6, rx.wrapping_add(2 << 8), ry, g)
                .expect("a kraken head");
        assert!(k > r, "fixture: the kraken walks AFTER the rival's slot");
        // CHASE (base 36 + 2) on the rival, counter mid-cycle (5 → 6,
        // an ON tick), awake.
        w.g.ent[k].tick70 = 38;
        w.g.ent[k].f146 = r as u16;
        w.g.ent[k].f26 = 5;
        w.g.ent[k].f58 = 64;
        w.rivals[ri].knock_mag = 0;
        w.rivals[ri].knock_dir = 0;
        w.tick(away(), PlayerCommand::default());
        assert_eq!(
            w.g.ent[k].f26, 6,
            "fixture: the counter stepped into an ON tick"
        );
        let (kx, ky) = (w.g.ent[k].x, w.g.ent[k].y);
        let (rx, ry) = (w.g.ent[r].x, w.g.ent[r].y);
        let want = Gen::angle_between(kx, ky, rx, ry).wrapping_add(0x400) & 0x7FF;
        assert_eq!(w.rivals[ri].knock_mag, 80, "+22 = 80 on the tethered rival");
        assert_eq!(
            w.rivals[ri].knock_dir, want,
            "+24 = kraken→rival bearing + 0x400"
        );
        assert_eq!(
            w.g.player_knock,
            (0, 0),
            "the human's register is not the target's"
        );
        assert_eq!(
            w.g.mc1_buffet_post.0,
            (0, 0),
            "the post is drained at the kraken's slot"
        );
    }

    #[test]
    fn the_mc1_knock_bearing_is_stored_raw() {
        let mut w = rebound_world();
        let ri = 0;
        let i = w.rivals[ri].ent as usize;
        let (vx, vy) = (w.g.ent[i].x, w.g.ent[i].y);
        let src = w.g.new_event().expect("a source record");
        w.g.ent[src].x = vx.wrapping_add(1);
        w.g.ent[src].y = vy.wrapping_add(300);
        assert_eq!(
            Gen::angle_between(vx.wrapping_add(1), vy.wrapping_add(300), vx, vy),
            2048,
            "fixture: this geometry is the 2048 return"
        );
        w.g.ent[i].mail[0] = (100, src as u16);
        w.rival_damage_intake(ri, i);
        assert_eq!(
            w.rivals[ri].knock_dir, 2048,
            "+24 takes sub_42150's return verbatim, 2048 included"
        );
        assert_eq!(w.rivals[ri].knock_mag, 10, "+22 = amount / 10");
    }
}
