//! INPUT RECOVERY from retail `.mgcr` takes — the shared home for
//! every consumer that turns a recorded closure pair back into the
//! tick's player input: `mgc-conform` (`verify-deltas` pose channel,
//! `replay`) and the app's `--replay` (docs/RECORDING.md "Consumers").
//! Measurement provenance lives in docs/CONFORMANCE.md "The replay
//! verifier" / "The pose channel"; the laws here are decompile-
//! verified and corpus-measured — change them only with the ledger.
//!
//! The recovery is exact, byte-domain (no latency modeling):
//! - the move/fire byte (`Type_160/164 dw_0`) is stamped by retail's
//!   consume loop — bits 1/2 speed, 4/8 strafe, 0x10/0x20 the
//!   CONSUMED fire levels (cast dispatch is LEVEL-triggered via the
//!   reload ladder `f48 := f50`; no edge detection exists in retail).
//!   Corpus: 2,368/2,368 MC1 casts and 560/560 MC2 casts carry the
//!   bit on the dispatch record. PHASE is per-game: MC1 stamps
//!   post-pass (the pair reads record N), MC2 stamps in PlayerEvents
//!   (read record N+1).
//! - the stick enters the mover only through the low-pass filter
//!   `acc += (2·stick − acc)/4`, recorded at both ends of the pair,
//!   so the filter inverts exactly ([`recover_stick`]).
//! - equips/rebinds replay the recorded hand change itself
//!   ([`recover_pair_mc1`]/[`recover_pair_mc2`]).
//! - respawn rides the SPACE lane of the raw input channel; MC2 dates
//!   the press with the recentre witness ([`Mc2RespawnWitness`]), MC1
//!   has no latch and keeps the ±1-tick caveat (docs/RECORDING.md).

use crate::mgcr::{Notify, ObsMc1, RetailEntMc2, RetailMc1, RetailMc2};

/// A retail CHEAT fired by the recorded player — control opcode 30
/// (`0x1E`), `param1` = the discriminant below. Both engines bind the
/// same keys: ALT held + F1..F7 on MC1 (remc1 :20018-46 / :20423-61),
/// ALT + F1..F10 on MC2 (PlayerInput.cpp:95-160); the enable gate is
/// MC1's `-cheat N` command line and MC2's tester flag
/// (`setting_byte2_23 < 0`) or the wizard name "chronicle".
///
/// ## Why the toast, and not the keys
///
/// The opcode itself is UNRECOVERABLE from a capture: retail memsets
/// the 10-byte control command at the end of the same event pass
/// (remc1 :49044) and the recorder's settled window opens after that,
/// so opcode 30 appears zero times in any take. The raw key channel
/// does see the F-key, but it carries the documented ±1-tick
/// attribution caveat and cannot tell a held key from a repeat.
///
/// The handler's OWN on-screen message can do both: it names the cheat
/// and it re-arms a lifetime counter that otherwise only counts down,
/// and it lands in the per-player block INSIDE the captured closure.
/// Measured over the two cheat takes: mc1l0-test 23/23 fires and
/// mc2l0-test 103/103, each matching a key press edge 1:1 with zero
/// misses and zero false positives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cheat {
    /// 1 — grant every spell not already held, as a real pool
    /// manifestation each (MC1 class 12 ×24, MC2 class 15 ×26).
    AllSpells,
    /// 2 — spawn a (10,39) sphere holding 100000 mana, and top the
    /// caster's own mana up to its maximum.
    MoreMana,
    /// 3/4/5 — mass kill by model: rival players / castles / balloons.
    DestroyPlayers,
    DestroyCastles,
    DestroyBalloons,
    /// 6 — restore the caster to full life.
    Heal,
    /// 7 — kill every creature on the map.
    KillCreatures,
    /// 8 (MC2) — +100 volatile XP on all 26 spells, then re-derive
    /// every tier. THE TIER UNLOCK: [`Cheat::AllSpells`] grants spells
    /// at level 0 only, so a take that exercises tier 1/2 needs this.
    SpellXp,
    /// 9 (MC2) — toggle free spell usage (the built manifestation
    /// takes `mana = 1, manaRegen = 0`).
    FreeSpell,
    /// 10 (MC2) — toggle invincibility.
    Invincible,
    /// 11 (PORT-ONLY code; never emitted by the toast detector) —
    /// force the level-complete latch, goals met or not. Retail has
    /// this in BOTH games, as TESTER KEYS rather than opcode-30
    /// sub-codes: MC1 `'c'` sets `var_u16_13325 |= 2` (the
    /// press-space completion latch; remc1 :20281-84, gate
    /// `var_u8_1 < 0`), MC2 SHIFT+`'c'` sets `IsLevelEnd_0 = 1`
    /// (PlayerInput.cpp:251-59, gate `setting_byte2_23 < 0`).
    WinLevel,
}

impl Cheat {
    /// The retail sub-code (`param1` of control opcode 30).
    pub fn code(self) -> u8 {
        match self {
            Cheat::AllSpells => 1,
            Cheat::MoreMana => 2,
            Cheat::DestroyPlayers => 3,
            Cheat::DestroyCastles => 4,
            Cheat::DestroyBalloons => 5,
            Cheat::Heal => 6,
            Cheat::KillCreatures => 7,
            Cheat::SpellXp => 8,
            Cheat::FreeSpell => 9,
            Cheat::Invincible => 10,
            Cheat::WinLevel => 11,
        }
    }

    /// Parse a sub-code back (the `.mgcr` port-input lane).
    pub fn from_code(code: u8) -> Option<Cheat> {
        Some(match code {
            1 => Cheat::AllSpells,
            2 => Cheat::MoreMana,
            3 => Cheat::DestroyPlayers,
            4 => Cheat::DestroyCastles,
            5 => Cheat::DestroyBalloons,
            6 => Cheat::Heal,
            7 => Cheat::KillCreatures,
            8 => Cheat::SpellXp,
            9 => Cheat::FreeSpell,
            10 => Cheat::Invincible,
            11 => Cheat::WinLevel,
            _ => return None,
        })
    }

    /// A short label for reports.
    pub fn name(self) -> &'static str {
        match self {
            Cheat::AllSpells => "all-spells",
            Cheat::MoreMana => "more-mana",
            Cheat::DestroyPlayers => "destroy-players",
            Cheat::DestroyCastles => "destroy-castles",
            Cheat::DestroyBalloons => "destroy-balloons",
            Cheat::Heal => "heal",
            Cheat::KillCreatures => "kill-creatures",
            Cheat::SpellXp => "spell-xp",
            Cheat::FreeSpell => "free-spell",
            Cheat::Invincible => "invincible",
            Cheat::WinLevel => "win-level",
        }
    }
}

/// The handlers' message strings, verbatim (remc1 :48904-49009, remc2
/// EF:37817-91). The ON/OFF toggles share a prefix — matching the
/// prefix keeps the TOGGLE semantic (the recording tells us it
/// flipped, not which way, and retail's own state is the flip).
const CHEAT_TOASTS: &[(&str, Cheat)] = &[
    (".. CHEAT: access all spells", Cheat::AllSpells),
    (".. CHEAT: more mana", Cheat::MoreMana),
    (".. CHEAT: destroy all players", Cheat::DestroyPlayers),
    (".. CHEAT: destroy all castles", Cheat::DestroyCastles),
    (".. CHEAT: destroy all balloons", Cheat::DestroyBalloons),
    (".. CHEAT: heal", Cheat::Heal),
    (".. CHEAT: Kill all creatures", Cheat::KillCreatures),
    (".. CHEAT: More Spell Experience Points", Cheat::SpellXp),
    (".. CHEAT: Free Spell Usage", Cheat::FreeSpell),
    (".. CHEAT: Invincability", Cheat::Invincible),
];

/// The cheat this toast NAMES, ignoring whether it just fired.
fn cheat_named(cur: &Notify) -> Option<Cheat> {
    let text = cur.text();
    CHEAT_TOASTS
        .iter()
        .find(|(s, _)| text.starts_with(s))
        .map(|&(_, c)| c)
}

/// The cheat fired across one recorded MC1 pair, from the caster's own
/// message slot: the counter must have been RE-ARMED (it only counts
/// down otherwise) and the text must name a cheat. Repeats of the
/// same cheat are distinguished by the counter alone — the text does
/// not change between them.
///
/// ⚠⚠ THE FIRE EDGE IS PER-GAME — see [`Notify::fired_since_mc1`].
/// MC1's counter clamps at 0 where MC2's wraps to 0xFFFF, so sharing
/// one rule made every expired MC1 cheat toast re-fire its cheat on
/// every subsequent tick.
pub fn cheat_fired_mc1(prev: &Notify, cur: &Notify) -> Option<Cheat> {
    cheat_named(cur).filter(|_| cur.fired_since_mc1(prev))
}

/// The MC2 twin of [`cheat_fired_mc1`], on MC2's own expiry rule
/// ([`Notify::fired_since_mc2`]).
pub fn cheat_fired_mc2(prev: &Notify, cur: &Notify) -> Option<Cheat> {
    cheat_named(cur).filter(|_| cur.fired_since_mc2(prev))
}

/// `MGC_NO_MC2_WHIRL_ROLL_CRANK=1` — the kill switch shared with the
/// sim half (`mgc_sim::engine::world::no_mc2_whirl_roll_crank`); the
/// stick inversion and the mover must move together or they disagree.
pub fn no_mc2_whirl_roll_crank() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WHIRL_ROLL_CRANK").is_some())
}

/// `MGC_NO_MC2_WW_VETO_WITNESS=1` restores the pre-dig witness for the
/// whirlwind crank — the SWIRL HEADING EDGE alone
/// (`word_0x30_48` changed), which misses every near-arm seizure.
pub fn no_mc2_ww_veto_witness() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_WW_VETO_WITNESS").is_some())
}

/// Invert the stick filter across one recorded tick: find a stick
/// value whose increment `(2·stick − acc)/4` (trunc toward zero, the
/// :49018 law) lands the accumulator exactly on acc@N+1. The mover
/// reads only the filtered value, so any solution is equivalent
/// downstream; the smallest-|stick| one is returned. None = no
/// command-range stick explains the transition (respawn wipe, a
/// non-mouse write).
pub fn recover_stick(acc_n: i16, acc_n1: i16) -> Option<i16> {
    let a = acc_n as i32;
    let d = acc_n1 as i32 - a;
    let (lo, hi) = if d > 0 {
        (4 * d, 4 * d + 3)
    } else if d < 0 {
        (4 * d - 3, 4 * d)
    } else {
        (-3, 3)
    };
    let mut best: Option<i16> = None;
    for n in lo..=hi {
        // n = 2·stick − acc: parity is pinned by acc.
        if (n + a) % 2 != 0 {
            continue;
        }
        let s = (n + a) / 2;
        if (-128..=127).contains(&s) && best.is_none_or(|b| s.abs() < (b as i32).abs()) {
            best = Some(s as i16);
        }
    }
    best
}

/// `MGC_NO_MC2_SLOW_STICK_INVERT=1` — restore the un-scaled stick
/// inversion (the pre-2026-09-09 behaviour) on a web-slowed carpet.
pub fn no_mc2_slow_stick_invert() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SLOW_STICK_INVERT").is_some())
}

/// ⭐⭐⭐ THE STICK INVERSION HAS A *SCALE* TO UNDO AS WELL AS A SECOND
/// WRITER. [`recover_stick`] inverts `acc += trunc((2·stick − acc)/4)`,
/// but `sub_5D530` does not add that delta raw while the WEB SLOW is
/// latched: EF:59622-30 folds it through `(v · (4 − moveSpeed))/4`
/// (`Mc2Ext::slow_scale`, the same round-toward-zero fold the speeds
/// take). ⭐ VERIFIED IN THE SHIPPED `NETHERW.EXE`, not just the
/// decompile — file offsets and bytes:
///   `0x81d5f 8a b1 4c 01 00 00`  `mov dh,[ecx+0x14c]` (moveSpeed_0x14C_332)
///   `0x81d65 84 f6 75 1e`        `test dh,dh` / `jne` — the ZERO branch is the RAW add
///   `0x81d8b ba 04 00 00 00`     `mov edx,4`
///   `0x81d90 29 c2`              `sub edx,eax`        (4 − moveSpeed)
///   `0x81d92 0f bf 41 04`        `movsx eax,[ecx+4]`  (rollDelta_0x4_4)
///   `0x81d96 0f af d0`           `imul edx,eax`
///   `0x81d99 89 d0 c1 fa 1f c1 e2 02 1b c2 c1 f8 02`  the trunc-toward-zero `/4` idiom
///   `0x81da8 0f bf 81 55 01 00 00` `movsx eax,[ecx+0x155]` / `01 d0` / store (roll_acc, +341)
/// pitch's twin runs at `0x81db8..0x81df2` on `[ecx+0x157]` (pitch_acc, +343).
/// So on a slowed carpet the recorded accumulator step is the
/// SCALED one and inverting the unscaled law recovers the wrong
/// cursor — the port then re-scales that wrong cursor and lands a few
/// units off, every tick, on BOTH pose accumulators.
///
/// Witness, mc2l24 t=15551→15552 with `moveSpeed = 1` (k = 3):
/// `roll_acc −5 → 1` (Δ +6) and the recorded `rollDelta_0x4_4` is 8 —
/// `trunc(8·3/4) = 6`, retail's own scaled step. The old inversion
/// read Δ +6 as a RAW step, recovered stick 10, and the mover then
/// scaled *that*: `trunc((2·10 + 5)/4) = 6`, `trunc(6·3/4) = 4`,
/// `roll_f = −1` — the exact value the port reported. Same tick,
/// `pitch_acc 17 → 14` recovers stick 1 → `dp = −3` → scaled −2 →
/// `pitch_f = 15` against retail's 14.
///
/// `move_speed` is the value at the END of tick N, because the
/// slow/mobilize decay walk is `sub_5D530`'s block 8
/// (`Mc2Ext::tick_debuffs`) — it runs AFTER the pose filter, so tick
/// N+1's filter reads N's latch.
///
/// The search is a plain 256-wide sweep of the signed-byte cursor
/// (`0x77482 movsx eax,byte [eax+0x3]`) rather than an algebraic
/// preimage, because the scale is lossy; `move_speed == 0` reduces to
/// [`recover_stick`] exactly (same domain, same smallest-|stick|
/// tie-break), which is what keeps every un-slowed take byte-identical.
pub fn recover_stick_slowed(acc_n: i16, acc_n1: i16, move_speed: u8) -> Option<i16> {
    if move_speed == 0 || no_mc2_slow_stick_invert() {
        return recover_stick(acc_n, acc_n1);
    }
    let a = acc_n as i32;
    let k = 4 - (move_speed as i32);
    let mut best: Option<i16> = None;
    for s in -128i32..=127 {
        let raw = (2 * s - a) / 4;
        if a + (raw * k) / 4 != acc_n1 as i32 {
            continue;
        }
        if best.is_none_or(|b| s.abs() < (b as i32).abs()) {
            best = Some(s as i16);
        }
    }
    best
}

/// The knock the pair's mover consumed. Normally N's channel, clamped
/// ±128 like `take_knock_step`. A hit ARMS the channel mid-pass
/// before the carpet's slot, so when N+1's stored value is not the
/// pure decay of N's, reconstruct the armed value by un-decaying it
/// (measured on mc1hwl0 t=371: kmag 0→76 = 80 armed − 4, right at the
/// first dirty x/y window). Same channel shape both games (cap 128,
/// decay −4, snap <4; EF:59695-711).
pub fn consumed_knock(mag0: i16, dir0: u16, mag1: i16, dir1: u16) -> Option<(u16, i16)> {
    let m0 = mag0.clamp(-128, 128);
    let decay = if m0 == 0 {
        0
    } else {
        let d = m0 - 4 * m0.signum();
        if d.abs() < 4 { 0 } else { d }
    };
    let rearmed = mag1 != decay || (mag1 != 0 && dir1 != dir0);
    if rearmed && mag1 != 0 {
        let m = mag1 + 4 * mag1.signum();
        Some((dir1 & 0x7FF, m.clamp(-128, 128)))
    } else if m0 != 0 {
        Some((dir0 & 0x7FF, m0))
    } else {
        None
    }
}

/// SPACE (scancode 57 = 0x39) — retail's RESPAWN key, read off the
/// raw input channel's `keys_down` lane. The input dispatcher raises
/// `PlayerAction` 0xF from it, and PI:1102 accepts that command only
/// while `life < 0 && actionIndex == 3`, so a SPACE held in any other
/// state is inert and the port's own `LifeState::Dead` gate
/// reproduces the filter. Without this lane the replayed MC2 human
/// can never leave the corpse state, and the two ticks where retail
/// rebuilds the spellbook (mc2l3 t=15314→15315 and t=20611→20612)
/// are unreachable. MC2 dates the press with [`Mc2RespawnWitness`];
/// MC1 has no press latch, so the caller reads the pair's END record
/// and accepts the ±1-tick dating caveat (docs/RECORDING.md).
pub fn respawn_key(input: Option<&serde_json::Value>) -> bool {
    key_held(input, 57)
}

/// A scancode's held state off the raw input channel's `keys_down`
/// lane (the recorder emits every held scancode, unfiltered).
pub fn key_held(input: Option<&serde_json::Value>, scancode: i64) -> bool {
    input
        .and_then(|i| i.get("keys_down"))
        .and_then(|k| k.as_array())
        .is_some_and(|k| k.iter().any(|v| v.as_i64() == Some(scancode)))
}

/// The recorded LIVE cursor `(x, y)` — `input.mouse`, the twin of
/// [`press_pos`]'s press snapshot.
pub fn mouse_pos(input: Option<&serde_json::Value>) -> Option<(i16, i16)> {
    let p = input?.get("mouse")?;
    let g = |k: &str| p.get(k).and_then(|v| v.as_i64()).map(|v| v as i16);
    Some((g("x")?, g("y")?))
}

/// The recorded cursor-AT-PRESS `(x, y)` — `input.mouse_press_pos`,
/// raw twin `state.ext.press_b64` ([`crate::mgcr::Ext::press`]).
///
/// **It is NOT the cast's aim.** The ISR snapshots it on every press
/// edge (EF:51478-97) and the poll copies it to
/// `unk_18058C.x_DWORD_1805B8/1805BC` (EF:49664-65 and the three
/// sibling control-mode arms), but the ONLY consumer downstream is
/// `sub_1A7A0_fly_asistant` (PI:1988-2013) — the fly-assistant
/// idle-recentre watchdog. The player's aim/attitude command is
/// computed from the LIVE cursor, and the cast gate `sub_5F660`
/// (EF:60874) takes no aim argument at all: the launch direction
/// comes off the caster entity's own pose.
pub fn press_pos(input: Option<&serde_json::Value>) -> Option<(i16, i16)> {
    let p = input?.get("mouse_press_pos")?;
    let g = |k: &str| p.get(k).and_then(|v| v.as_i64()).map(|v| v as i16);
    Some((g("x")?, g("y")?))
}

/// The MC2 park witness — the modal-park state shape OR the FULL-STOP
/// key (BACKSPACE = scancode 14, PlayerAction 0x27 EF:37954-65, which
/// zeroes `actSpeed` and the command in one tick). `frozen` is the
/// carpet-pinned-across-the-pair clause: it discriminates the KEY-LESS
/// modal park, and the key BYPASSES it because a same-tick knockback
/// moves the parked carpet (mc2l0-permadeath t=2191/2229). The held
/// speed key (`mb & 3`) guards both arms against the mc2l3 t=605
/// zero-crossing false positive. Pair-blind harness machinery — pinned
/// by unit, not fixture (the fixtures runner pins the pose, which is
/// the very channel this law moves).
pub fn mc2_park_witness(
    cmd_speed: i16,
    mb: u32,
    ent_speed: i16,
    frozen: bool,
    full_stop: bool,
) -> bool {
    cmd_speed == 0 && mb & 3 == 0 && ent_speed == 0 && (full_stop || frozen)
}

/// DATING THE MC2 RESPAWN PRESS. The key registers carry no press
/// LATCH (the mouse's disambiguator), and the corpus shows BOTH sides
/// of retail's poll: SPACE first appears at record 15314 with the
/// reset in frame 15315 (pressed AFTER that frame's poll), and at
/// record 20612 with the reset in frame 20612 (pressed BEFORE it).
/// No held-key rule can split those, so the witness adds retail's
/// own: the 0xF handler runs `SetCenterScreenForFlyAssistant_6EDB0`
/// (EF:37653), which slams the cursor to the screen centre — so a
/// record whose cursor JUMPED this pair and now equals the
/// press-position snapshot is a record whose frame ran the command:
///
/// ```text
///   fire(pair) = space(end) && (space(start) || recentred(end))
/// ```
///
/// Measured over the whole mc2l3 take: 348 records carry the
/// recentre shape (ordinary clicks), and exactly TWO of them also
/// have SPACE down — the two reset frames. Feed [`observe`] every
/// record in stream order (anchors included).
///
/// [`observe`]: Mc2RespawnWitness::observe
#[derive(Default)]
pub struct Mc2RespawnWitness {
    prev_space: bool,
    prev_press: Option<(i16, i16)>,
}

impl Mc2RespawnWitness {
    /// Fold one record's raw input channel; returns whether the frame
    /// that produced this record ran the respawn command.
    ///
    /// The recentre discriminator reads the PRESS-POS snapshot's jump,
    /// not the cursor's: the 0xF handler slams the cursor to centre,
    /// but when the cursor already SITS at centre (a respawn pressed
    /// while parked — mc2l0-permadeath t=1440/2574, `mouse_press_pos`
    /// jumps to an unmoved 320,200 cursor) the cursor shows no edge
    /// and the old rule dated the press one frame late. The genuine
    /// within-frame press (mc2l3 record 20612) snaps BOTH registers
    /// at once, so the press-jump form covers it too; a false fire on
    /// an ordinary click+SPACE is inert (the sim's Dead gate filters
    /// a respawn on a living wizard).
    pub fn observe(&mut self, input: Option<&serde_json::Value>) -> bool {
        let press = press_pos(input);
        let space = respawn_key(input);
        let mouse = mouse_pos(input);
        let recentred = press.is_some() && press != self.prev_press && mouse == press;
        let fire = space && (self.prev_space || recentred);
        self.prev_space = space;
        self.prev_press = press.or(self.prev_press);
        fire
    }
}

/// MC1 dw_0 fire bits (0x10/0x20) — the CONSUMED per-tick fire
/// levels, stamped by the same consume loop as the move bits.
pub fn mc1_fire(mb: u32) -> (bool, bool) {
    (mb & 0x10 != 0, mb & 0x20 != 0)
}

/// ⭐ MC2'S **THIRD** CAST BUTTON. `sub_5F380`'s cast tail is three
/// flat `testb`/`call` pairs on the wizard extension's command word
/// `entityIndex_0x0`, not two (EF:60850-62):
///
/// ```text
/// if (w & 0x10) sub_5F660(a1x, SpellEnabled[SpellIndexLeft ], 256);
/// if (w & 0x20) sub_5F660(a1x, SpellEnabled[SpellIndexRight], 512);
/// if (w & 0x40) sub_5F660(a1x, SpellEnabled[spellIndex_D94FF[spellIndex_0x458_1112]], 256);
/// ```
///
/// The third arm is the CYCLE-RING SHORTCUT: it casts the spell under
/// the ring cursor **without equipping it**, and it stamps the LEFT
/// hand bit (256) like the left button. `spellIndex_D94FF`
/// (GameUI.cpp:59) is the identity over 0..25, so the index IS the
/// recorded cursor `byte_0x458_1112` (the `ring_cursor` lane).
///
/// The bit rides the SAME consumed command word `mc1_fire` reads, so
/// it needs no latch reconstruction — which is what the port had, and
/// which refused. Corpus traffic: **one record in ten MC2 takes** —
/// mc2l22 t=63318, `move_bits = 64`, `ring_cursor = 20`, arming the
/// `(15,20)` token while both hands held 9 and 1.
pub fn mc2_ring_cast(mb: u32, ring_cursor: u8) -> Option<u8> {
    (mb & 0x40 != 0 && ring_cursor <= 25).then_some(ring_cursor)
}

/// One pair's recovered input, format-domain (raw ids and bits; the
/// consumer widens into its own input types). MC1 fills the equip
/// lanes, MC2 the select lane; the stick lanes are per-axis Options
/// so an unrecoverable transition (respawn wipe, a non-mouse write)
/// stays visible to the consumer's gating.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecoveredPair {
    pub stick_x: Option<i16>,
    pub stick_y: Option<i16>,
    /// MC2 only: this pair carries the whirlwind's camera-roll crank
    /// witness (the funnel wrote the HUMAN's `word_0x30_48` across the
    /// pair). The stick inversion has to guess how MANY 28s to undo;
    /// [`recover_pair_mc2_k`] takes the count, and this flag tells the
    /// caller a trial step is worth running to find it.
    pub whirl_crank: bool,
    /// The consumed move/fire byte driving the pair's mover (bits 1/2
    /// speed, 4/8 strafe, 0x10/0x20 fire) — record N on MC1, N+1 on
    /// MC2 (the per-game stamp phase, module doc).
    pub move_byte: u32,
    pub fire_left: bool,
    pub fire_right: bool,
    /// MC1: a recorded hand change across the pair, resolved to the
    /// INTERNAL spell id via the acquisition list at N+1.
    pub equip_left: Option<u8>,
    pub equip_right: Option<u8>,
    /// MC2: a recorded hand change as the pane select
    /// `(spell, tier, hand)`; `(255, 0, hand)` = the unbind commit.
    pub mc2_select: Option<(u8, u8, u8)>,
    /// MC2: the cycle-ring cast bit (`move_bits & 0x40`) resolved to
    /// the spell index under the ring cursor — see [`mc2_ring_cast`].
    pub mc2_ring_cast: Option<u8>,
    /// Both MC2 hands changed in one pair — one select per tick, the
    /// left wins, the right is DROPPED (counted by consumers).
    pub rebind_dropped: bool,
    pub respawn: bool,
    /// MC1 Shift+K, the SELF-KILL (:20488-93): a direct
    /// `actLife = -1` on the local carpet, bypassing
    /// MakeControlCommand entirely — no control word, no toast, no
    /// knock. The witness is therefore the STATE pair itself: the
    /// human carpet's life crossing `> 0` → exactly −1 with no fresh
    /// knock impulse. Every projectile kill in the corpus overshoots
    /// (−65/−160/−215 …) and arms the knock channel, so the exact −1
    /// is the writer's fingerprint (mc1l0-pd t=2644, mc1hwl0-pd
    /// t=2658/2782 — 3/3, and the certified sweep is the standing
    /// no-false-positive probe).
    pub suicide: bool,
    /// Shift+L, destroy own castle one level. MC1: the move byte IS
    /// the witness (`dw_0 == 48`, retail's own predicate :55760 —
    /// measured 18/18 on mc1l0, zero false positives over 7,098
    /// records). MC2: the castle-entity witness (no move-byte trace
    /// there — the command rides `PlayerAction` 0x2A).
    pub demolish: bool,
    /// MC2: the consumed target-speed COMMAND at N+1 (per-player
    /// `cmd_speed` — mouse-proportional, not the ±16 key servo). Fed
    /// to the mover as the pair's speed target the way the stick
    /// lanes feed the filters.
    pub mc2_cmd_speed: Option<i16>,
    /// MC2: the carpet is PARKED by a modal UI (big map / spell
    /// book — retail keeps playing but stops the carpet dead).
    /// Witness: command 0 AND the carpet entity frozen in place at
    /// zero speed across the pair (mc2l0 t=598: speed 80→0 in one
    /// tick, x/y/z pinned for the whole window, yaw/pitch still
    /// servoing the re-centred cursor).
    pub mc2_park: bool,
    /// A retail cheat the recorded player fired on this pair — the
    /// one input verb that MUTATES the world rather than steering it,
    /// so a free-running consumer has to apply it or diverge
    /// permanently from that tick ([`Cheat`]).
    pub cheat: Option<Cheat>,
}

impl RecoveredPair {
    /// Both stick axes recovered (a pair the mover chain may own).
    pub fn stick_ok(&self) -> bool {
        self.stick_x.is_some() && self.stick_y.is_some()
    }

    /// The stick to feed the mover — centered where unrecoverable.
    pub fn stick(&self) -> (i16, i16) {
        (self.stick_x.unwrap_or(0), self.stick_y.unwrap_or(0))
    }

    /// Move byte exactly 48: retail's DEMOLISH command word
    /// (`MakeControlCommand(6, 48)` from Shift+L — the only writer
    /// that can produce exactly 16|32). It short-circuits sub_46840
    /// WHOLE (:55759): no move, no casts, and the held strafe's
    /// decay freezes. `Mc1Input` carries no fire state, so the
    /// consumer pre-feeds one decay quantum — the mover's decay
    /// lands back on the frozen value. MC1-only — MC2's sub_5F380
    /// has no such short-circuit.
    pub fn mc1_strafe_freeze(&self) -> bool {
        self.move_byte == 48
    }
}

/// `MGC_NO_MC1_RESPAWN_STATE_DATING=1` — restore the bare SPACE lane
/// (the key alone dates the MC1 respawn, ±1 tick). See the `respawn`
/// arm of [`recover_pair_mc1`].
fn mc1_respawn_key_only() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC1_RESPAWN_STATE_DATING").is_some())
}

/// Recover the input consumed across an MC1 pair (records N → N+1).
/// `input_end` is the END record's raw input channel (the respawn
/// SPACE lane — MC1's ±1-tick dating caveat, docs/RECORDING.md, which
/// the `respawn` arm below resolves against retail's own revival).
pub fn recover_pair_mc1(
    pst: &RetailMc1,
    st: &RetailMc1,
    input_end: Option<&serde_json::Value>,
) -> RecoveredPair {
    let pw = &pst.wizards[pst.local_player as usize];
    let cw = &st.wizards[st.local_player as usize];
    let mb = pw.move_bits;
    // Byte 48 exactly = the DEMOLISH command word: retail's :55760
    // short-circuit skips the WHOLE mover including both cast calls,
    // so a demolish tick fires NEITHER hand despite carrying both
    // fire bits (measured: dw_0 == 48 on exactly the 18 demolish
    // press edges of mc1l0, and the castle's `act_life = -1` lands
    // on the very next record each time).
    let demolish = mb == 48;
    let (fire_left, fire_right) = if demolish {
        (false, false)
    } else {
        mc1_fire(mb)
    };
    // Equips: a recorded hand change across the pair replays as the
    // equip command (resolved to the internal spell id via the
    // acquisition list at N+1).
    let equip = |prev_raw: u16, cur_raw: u16| -> Option<u8> {
        (prev_raw != cur_raw)
            .then(|| st.hand_spell(st.local_player as usize, cur_raw))
            .flatten()
    };
    RecoveredPair {
        stick_x: recover_stick(pw.roll_acc as i16, cw.roll_acc as i16),
        stick_y: recover_stick(pw.pitch_acc as i16, cw.pitch_acc as i16),
        move_byte: mb,
        fire_left,
        fire_right,
        equip_left: equip(pw.hand_left, cw.hand_left),
        equip_right: equip(pw.hand_right, cw.hand_right),
        // ⭐ THE RESPAWN IS DATED BY RETAIL'S OWN REVIVAL, NOT BY THE
        // KEY. SPACE is MC1's LAST raw-key lane — every other verb in
        // this recovery (move byte, stick, fires, equips, demolish,
        // suicide, cheat) already reads the state closure — and it is
        // the one lane that carries the ±1-tick attribution caveat,
        // because MC1's held scancodes live in the STATIC FRAME, which
        // is sampled outside the consensus window (docs/RECORDING.md
        // "`input`"). The caveat is not a constant offset: mc1l49
        // records BOTH signs inside one take. SPACE first appears at
        // record 5723 and retail's carpet revives across 5723 → 5724
        // (`act_life` −428 → 10000, `+70` 3 → 0), so the key is a tick
        // EARLY there; SPACE first appears at record 7072 and retail
        // revives across 7071 → 7072 (−1705 → 10000), so the same key
        // is dated exactly there. Feeding the key alone therefore
        // respawns the free run a tick early on one death and on time
        // on the next — measured, `mc1l49-5487-7690`.
        //
        // Retail's own record settles it. `sub_44D30` reseats and
        // REFILLS the carpet in the frame that consumes the command,
        // so the pair whose END record shows the local carpet back
        // above zero IS the tick that ran the respawn. The key stays
        // as corroboration (a held SPACE somewhere across the pair);
        // the STATE picks which pair. Same doctrine as the `suicide`
        // lane just below and as the cheat witness — "raw input is
        // advisory; retail recordings are validated by state".
        //
        // ⚠ Residual: a press held for exactly ONE record that lands
        // on the record BEFORE the revival would be missed. Both
        // mc1l49 deaths hold SPACE for 4-5 records; retail's own key
        // repeat makes a sub-frame press unlikely, and the fallback
        // costs a tick, not the respawn.
        // Kill switch `MGC_NO_MC1_RESPAWN_STATE_DATING=1` restores the
        // bare key lane.
        respawn: {
            let key = respawn_key(input_end);
            if mc1_respawn_key_only() {
                key
            } else {
                let s = cw.play_index as usize;
                key && matches!((pst.ents.get(s), st.ents.get(s)), (Some(p), Some(c))
                    if p.act_life < 0 && c.act_life > 0)
            }
        },
        // The Shift+K state witness (field doc): life > 0 → exactly
        // −1 on the local carpet, knock channel without a fresh
        // impulse (a decay step is fine; a hit's re-arm is not).
        suicide: {
            let s = cw.play_index as usize;
            matches!((pst.ents.get(s), st.ents.get(s)), (Some(p), Some(c))
                if p.act_life > 0 && c.act_life == -1)
                && cw.knock_mag <= pw.knock_mag
        },
        demolish,
        cheat: cheat_fired_mc1(&pw.notify, &cw.notify),
        ..RecoveredPair::default()
    }
}

/// Recover the input consumed across an MC2 pair (records N → N+1).
/// `respawn` is the [`Mc2RespawnWitness`] verdict for the END record
/// (the witness folds EVERY record in stream order, anchors included,
/// so its state machine lives outside the pair). `input_end` is the
/// END record's raw input channel (the demolish key corroboration).
/// ⭐ THE WALL DEAD-STOP IS A SIM EFFECT, NOT AN INPUT — so the
/// capture's own speed command has to be un-done before it can be
/// replayed as one.
///
/// A BLOCKED move restores the carpet's position and zeroes
/// `speed_0xc_12` at the END of the frame (EF:59595-602) — after
/// `sub_5D530`'s servo has already stepped `actSpeed` from the command
/// as it stood mid-frame. The capture at N+1 therefore holds the
/// POST-block 0, and handing that straight back as the pair's target
/// applies the dead-stop a whole frame early. Consumers model the stop
/// themselves (`flight::move_mc2`, `out.zero_speed`), so they re-zero
/// on their own; what they need is the command the frame actually
/// served.
///
/// That command is N's, advanced by the frame's OWN ±16 key
/// integration (`sub_5F380`, EF:60748 — the guarded form: a key only
/// steps while the target is inside ±80), which the move_bits lane
/// still reports. mc2l3 t=3407: the carpet flies into a wall at 48
/// with forward held, retail integrates the command to 64 and the
/// servo lands actSpeed on 64 before the block wipes it — reading the
/// wiped 0 back produced 48. mc2l3 t=2639 is the other branch: backing
/// at −80 with the target already at the floor, the integration is a
/// no-op and the servo holds −80 for the frame.
///
/// Witness (`pose_forced`) = the carpet's pose was NOT the mover's
/// output this pair, with the command newly 0 while `actSpeed` is
/// still RUNNING. Two shapes reach it and both zero the command from
/// outside the servo:
///   * FROZEN — the blocked move above. (The modal park is the
///     `speed == 0` twin of the same freeze; the two stay disjoint.)
///   * WARPED — the pose jumped further than any mover step could
///     carry it, so a spell/pad placed it. mc2l3 t=3933: a Teleport
///     lands the carpet 119 tiles away and clears the command; retail
///     holds actSpeed at −16 for that frame and drops to 0 at 3934,
///     where the port read the cleared command and dropped a tick
///     early.
///
/// ⭐⭐ A BLOCK THAT LASTS MORE THAN ONE FRAME HAS N's COMMAND ALREADY
/// WIPED. The two shapes above both entered the block from a live
/// command, so the first draft also demanded `prev_cmd != 0` — an
/// accidental property of its two exemplars, not part of the law. Hold
/// a wall down and the stop fires EVERY frame: from the second one on
/// the capture reads 0 at both ends while `sub_5F380` keeps re-adding
/// its +16, so `actSpeed` STALLS at 16 instead of decaying. mc2l3
/// t=6514..6516 is three such frames in a row — retail holds 16, 16,
/// 16 with forward held and the pose pinned, then drops to 0 at 6517
/// the moment the key releases. The integration is the law; the
/// starting value is just its argument.
pub fn mc2_pair_cmd_speed(
    prev_cmd: i16,
    cur_cmd: i16,
    move_bits: u32,
    pose_forced: bool,
    cur_speed: i16,
) -> i16 {
    if !(cur_cmd == 0 && cur_speed != 0 && pose_forced) {
        return cur_cmd;
    }
    let mut dir: i16 = 0;
    if move_bits & 1 != 0 && prev_cmd < 80 {
        dir = 1;
    }
    if move_bits & 2 != 0 && prev_cmd > -80 {
        dir = -1;
    }
    (prev_cmd + 16 * dir).clamp(-80, 80)
}

/// `MGC_NO_MC2_STEAL_NOT_A_REBIND=1` — the A/B arm for the
/// world-took-it discriminator in [`recover_pair_mc2`]. Set it to
/// restore the pre-dig behaviour, where every recorded hand-pointer
/// clear replayed as a pane UNBIND command.
fn mc2_steal_not_a_rebind_off() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_STEAL_NOT_A_REBIND").is_some())
}

/// `MGC_NO_MC2_ROLL_DELTA_CAPTURE=1` — restore the pre-dig stick
/// inversion for the ROLL axis, which un-cranked the recorded
/// `roll_acc` by a GUESSED number of 28s instead of reading the
/// tick's own `rollDelta_0x4_4` out of the capture.
pub fn no_mc2_roll_delta_capture() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_ROLL_DELTA_CAPTURE").is_some())
}

/// `MGC_NO_MC2_SLOW_STAMP_PHASE=1` — restore the pre-dig reading of
/// [`pair_move_speed`], which always handed the stick inversion tick
/// N's END latch and so missed a slow stamped INSIDE tick N+1.
fn no_mc2_slow_stamp_phase() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SLOW_STAMP_PHASE").is_some())
}

/// ⭐⭐⭐ THE SCALE THE PAIR'S POSE FILTER RAN UNDER IS NOT ALWAYS
/// TICK N'S LATCH — A WEB LANDING *INSIDE* TICK N+1 RAISES IT FIRST.
/// `moveSpeed` lives at `[ebx+0xa4] -> +0x14c` and THREE sites touch
/// it in one tick, in this order:
///   1. `sub_38E70`, the SLOW debuff stamp (`NETHERW.EXE` file
///      0x5d6ae `mov dh,[eax+0x14c]` / 0x5d6b4 `cmp dh,3`, reached
///      through the class-3 / model-0 wizard test at 0x5d694 `cmp [ebx+0x3f],3` / 0x5d69e `cmp [ebx+0x40],0`)
///      — it runs at the STAMPING record's own walk slot, which for a
///      web projectile is ahead of the carpet's;
///   2. `sub_5D530`'s POSE FILTER reads it at 0x81d5f
///      (`mov dh,[ecx+0x14c]`), branches at 0x81d65 `test dh,dh` and,
///      on the slowed arm, scales `rollDelta` by `4 - moveSpeed`
///      (0x81d8b `mov edx,4` / 0x81d90 `sub edx,eax` / 0x81d92
///      `movsx eax,[ecx+4]` / 0x81d96 `imul` / the 0x81d99
///      trunc-toward-zero `/4` / 0x81da8 load + 0x81db1 store of
///      `+0x155`);
///   3. `sub_5D530`'s DECAY WALK, six hundred bytes further down at
///      0x8216e-0x821ae (`mov al,[edx+0x14c]` / `je` / `mov
///      ah,[edx+0x14d]` / `dec ah` / store / `jne` / `dec ch` on
///      `+0x14c` / `mov byte [eax+0x14d],8`) — [`Mc2Ext::tick_debuffs`].
///
/// So the filter's operand is `pre + (a stamp landed ahead of the
/// carpet ? 1 : 0)`, and the DECAY that the old note keyed off
/// happens strictly after it. The recorded END pair names which of
/// the two happened, because the decay always leaves `+0x14d` one
/// below what the stamp wrote: a level that ROSE across the pair with
/// `move_speed_ctr == 7` at N+1 is a stamp that beat the mover
/// (`slow_hit` wrote 8, `tick_debuffs` took it to 7); the same rise
/// with `ctr == 8` is a stamp that landed BEHIND the carpet's slot
/// and did not touch this tick's filter.
///
/// WITNESS (mc2l24 t=8699, the take's free-run horizon after wave
/// 126's `MGC_NO_MC2_WW_HUMAN_MOVE_TEST`): `move_speed 0 -> 1`,
/// `move_speed_ctr 0 -> 7`, `roll_acc -23 -> -28`, and the capture's
/// own `rollDelta_0x4_4` is -7 — `trunc(-7*3/4) = -5`, retail's
/// scaled step under k = 3. Inverting with the END latch of tick N
/// (0, i.e. unscaled) recovers cursor -22 and a raw `dr` of -5; the
/// mover then scales THAT (`trunc(-5*3/4) = -3`) and lands `roll_f`
/// -26 — the port's reported value to the unit. Inverting with 1
/// recovers cursor -26, whose `dr` is exactly the recorded -7, and
/// the mover lands -28.
pub fn pair_move_speed(pre: u8, post_ms: u8, post_ctr: u8) -> u8 {
    if !no_mc2_slow_stamp_phase() && post_ms > pre && post_ctr == 7 {
        post_ms
    } else {
        pre
    }
}

/// The 28-unit total `sub_33340` added to `roll_0x155_341` across one
/// tick, given the accumulator as the frame head saw it and the number
/// of crank visits. Retail re-tests `< 256` before EVERY add
/// (`NETHERW.EXE` 0x57c7a `cmp $0x100,%cx` / 0x57c7f `jge 0x57c8d`,
/// inside the per-visit `v40` block), so this is a ladder, not `28*k`.
pub fn crank_units(acc: i16, k: u8) -> i16 {
    let mut v = acc as i32;
    let mut n = k;
    while n > 0 {
        if v < 256 {
            v += 28;
        }
        n -= 1;
    }
    (v - acc as i32) as i16
}

/// ⭐⭐⭐ THE CRANK'S WITNESS IS THE VETO LATCH, NOT THE SWIRL HEADING.
/// Did `sub_33340` crank the WIZARD's `roll_0x155_341` across this
/// recorded pair?
///
/// `sub_33340` writes the victim's `word_0x30_48` on only TWO of its
/// arms — the FAR one (`NETHERW.EXE` file 0x57d51 `66 89 53 30`, taken
/// when the squared range is >= 0x40000: 0x57c96 `81 7d e0 00 00 04 00`
/// / 0x57c9d `0f 8d 9a 00 00 00`) and the near arm's SUCCESSFUL grab
/// roll (0x57d30 `66 8b 43 1c` / 0x57d34 `66 89 43 30`) — but the
/// 28-unit crank at 0x57c86 (`66 89 b0 55 01 00 00`, under 0x57c7a
/// `66 81 f9 00 01` / 0x57c7f `7d 0c`) sits ABOVE that split, so every
/// near-arm visit whose grab roll FAILS cranks the accumulator and
/// leaves `word_0x30_48` exactly as it was. The swirl-heading EDGE is
/// therefore the FAR arm's witness only.
///
/// What the near arm DOES leave is the mover-veto one-shot: 0x57ca3
/// `8a 43 0d` / 0x57ca6 `0c 08` / 0x57cab `88 43 0d` = `byte[1] |= 8`
/// = `flags & 0x800`, the bit the pool importer already reads as
/// `F_STOP`. The GRABBED arm (0x57c58 `8a 4b 0f`, 0x57c5e
/// `f6 c1 10` + 0x57c61 `0f 85 fe 00 00 00` -> 0x57d65) sets the same
/// bit at 0x57d78 `88 53 0d` WITHOUT cranking, so that disjunct is gated on
/// the grab latch `flags & 0x1000_0000` (`byte[3] & 0x10`, set at
/// 0x57d2c `80 4b 0f 10`) being CLEAR at the frame head — the very
/// word `cl` is loaded from.
///
/// `[ebp-4]`, the flag the crank arm tests at 0x57c67
/// `80 7d fc 00` / 0x57c6b `74 29`, is set at 0x57bf3 `c6 45 fc 01`
/// iff `class3f == 3` (0x57bd8 `8a 53 3f` / 0x57be8 `80 fa 03`) and
/// `model40 == 0` (0x57bed `80 7b 40 00`) — this predicate's existing
/// clause, read out of the shipped bytes.
///
/// Measured on mc2l24, three heads, all three the SAME defect:
/// t=44613 (`roll_acc` -147 -> -97 = `rollDelta` 22 + 28, `f30`
/// 1390 -> 1390, `flags` 269 -> 2317), t=46814 (174 -> 206 = 4 + 28,
/// `f30` 1369 -> 1369, `flags` 525 -> 2573) and t=50677
/// (205 -> 186 = -47 + 28, `f30` 1810 -> 1810, `flags` 525 -> 2573).
/// Without the disjunct the inversion absorbs retail's own crank into
/// the cursor and the port's funnel then cranks AGAIN: `pose.roll_f`
/// -69 against -97 and 214 against 186 to the unit; at t=46814 the
/// doubled +32 needs a cursor of 151 and the pair simply reports
/// STICK-UNRECOVERABLE. Set `MGC_NO_MC2_WW_VETO_WITNESS` to restore
/// the heading-edge-only witness.
pub fn mc2_crank_witness(p: &RetailEntMc2, c: &RetailEntMc2) -> bool {
    p.class3f == 3
        && p.model40 == 0
        && ((c.f30 != 0 && c.f30 != p.f30)
            || (!no_mc2_ww_veto_witness()
                && c.flags & 0x800 != 0
                && p.flags & 0x1000_0000 == 0))
}

/// [`recover_pair_mc2_k`] with the historical ONE-crank assumption.
pub fn recover_pair_mc2(
    pst: &RetailMc2,
    st: &RetailMc2,
    respawn: bool,
    input_end: Option<&serde_json::Value>,
) -> RecoveredPair {
    recover_pair_mc2_k(pst, st, respawn, input_end, 1)
}

/// ⭐⭐⭐ THE CRANK COUNT IS AN ARGUMENT, NOT A CONSTANT. `sub_33340`
/// cranks once per visit that reaches the `v40` block, and its disc
/// walk reaches the same victim more than once whenever
/// `CopyEntityPosition_57CF0` carries it into a cell the walk has not
/// reached yet (mc2l24 t=7913: head 327 mid-rings the carpet in cells
/// (124,116) and (123,116) on one pass). The pair alone CANNOT count
/// them — both k=1 and k=2 invert to a legal signed-byte cursor there
/// — so the SIMULATION counts them and hands the number back; see
/// `mgc-conform`'s replay driver (throwaway trial step, then
/// re-recover).
pub fn recover_pair_mc2_k(
    pst: &RetailMc2,
    st: &RetailMc2,
    respawn: bool,
    input_end: Option<&serde_json::Value>,
    crank_k: u8,
) -> RecoveredPair {
    let pp = &pst.players[pst.local_player as usize];
    let cp = &st.players[st.local_player as usize];
    // MC2 stamps the move byte in PlayerEvents — read the END record.
    // Fire rides the same CONSUMED byte: measured strictly stronger
    // than the press-latch law — 560/560 retail arms carry the bit on
    // the same record, and the latch's extra edges are UI clicks the
    // byte correctly omits (ledger §THE REPLAY VERIFIER).
    let mb = cp.move_bits;
    let (fire_left, fire_right) = mc1_fire(mb);
    // The THIRD cast arm rides the same consumed word (`mc2_ring_cast`).
    let mc2_ring_cast = mc2_ring_cast(mb, cp.ring_cursor);
    // Hand rebinds: a recorded hand change replays as the pane select
    // (tier = the recorded per-spell selection at N+1; out-of-range
    // spell = the unbind commit).
    let rebind = |hand: u8, prev: i16, cur: i16| -> Option<(u8, u8, u8)> {
        (prev != cur).then(|| {
            if (0..26i16).contains(&cur) {
                (cur as u8, cp.sel[cur as usize], hand)
            } else {
                (255, 0, hand)
            }
        })
    };
    // ⭐⭐⭐ A HAND POINTER THAT MOVES IS NOT ALWAYS A COMMAND. The
    // m26 wraith's SPELL-STEAL (`sub_69300`, EF:55811-24) writes
    // `SpellEnabled[model] = 0`, clears the book pointer
    // `spell_ent[s]`, and then unequips EVERY hand holding that
    // model — all three from the WRAITH's own dispatch, with no
    // player input anywhere. Reading the resulting `hand_left`
    // 0 → −1 as a pane UNBIND replays `mc2_select_spell(255, …)`
    // BEFORE the walk, so `sub_69300`'s port twin then finds an empty
    // hand and takes its `spell < 0` abort — the jar never detaches,
    // and the steal's whole round trip (the action-78 arc out and the
    // re-bind on landing) is missing. The end state of the tick looks
    // right, which is why this hid: only the JAR's record diverges.
    //
    // The discriminator is in the recorded closure and needs no
    // heuristic: a pane unbind NEVER clears `spell_ent[s]` (+0x719),
    // and the steal always does. Same shape covers the death-drop.
    //
    // mc2l24 t=5206: wraith slot 193 rolls 5 on the %63 hijack,
    // retail detaches the class-15 record at slot 6 (action 0 → 78,
    // snapped onto the carpet at (31639, 25796, 1725)) and returns it
    // to the book 13 ticks later at t=5219 — the take's next two
    // segment heads, both of them.
    let world_took = |prev: i16, cur: i16| -> bool {
        !mc2_steal_not_a_rebind_off()
            && cur < 0
            && (0..26i16).contains(&prev)
            && pp.spell_ent[prev as usize] != 0
            && cp.spell_ent[prev as usize] == 0
    };
    let left = (!world_took(pp.hand_left, cp.hand_left))
        .then(|| rebind(0, pp.hand_left, cp.hand_left))
        .flatten();
    let right = (!world_took(pp.hand_right, cp.hand_right))
        .then(|| rebind(1, pp.hand_right, cp.hand_right))
        .flatten();
    // ⭐⭐ A TIER SWAP IS A SELECT THAT MOVES NO HAND POINTER. The
    // pane commits every pick through the same handler (PlayerAction
    // 0x1F/0x20, EF:37898-928: persist the tier, bind the quick-slot,
    // SetSpell, sound 14) and TWO recorded shapes reach it — the
    // pointer moves because a DIFFERENT spell was equipped, or the
    // pointer stays and only `array_0x437[spell]` moves because the
    // player picked a higher TIER of the spell already in that hand.
    // Keying recovery on the pointer alone saw the first and dropped
    // the second, which is how EVERY upgrade past level 0 arrives:
    // mc2l0 t=7728 records `hand_pending 1 -> 0`, `sel[0] 0 -> 1` and
    // the notification "FireBall" -> "Rapid Fire", with both hand
    // pointers untouched — retail re-prices the held class-15 token
    // (mana_max 100 -> 250, @0x2A 250 -> 160) and the port stayed on
    // the base tier for the rest of the take. Every MC2 recording
    // that uses a spell above level 0 breaks at its first swap.
    //
    // The hand is named by the pane's PENDING byte at the START
    // record (`byte_0x457_1111`: 1 = a left equip mid-flight, 2 =
    // right, PI:806-91), which the commit clears to 0. With no
    // pending byte to read, fall back to whichever hand already holds
    // the spell — the bind is a no-op there by definition.
    let tier_swap = (0..26).find(|&s| pp.sel[s] != cp.sel[s]).and_then(|s| {
        let hand = match pp.hand_pending {
            1 => 0u8,
            2 => 1u8,
            _ if cp.hand_left == s as i16 => 0,
            _ if cp.hand_right == s as i16 => 1,
            _ => return None,
        };
        Some((s as u8, cp.sel[s], hand))
    });
    let (mc2_select, rebind_dropped) = match (left, right) {
        (Some(l), Some(_)) => (Some(l), true),
        (l, r) => (l.or(r).or(tier_swap), false),
    };
    // Demolish (Shift+L → `PlayerAction` 0x2A, EF:37991-96): MC2's
    // command never touches the move byte, so the witness is the own
    // CASTLE's KILL EDGE — alive at the START record, exactly −1 at
    // the END record — corroborated by the held Shift+L scancodes
    // (L = 38, either shift 42/54; measured at mc2l24 t=41798).
    //
    // ⭐⭐⭐ THE ACTION IS NOT PART OF THE WITNESS. Retail's handler is
    // three lines — `if (castle > Entities[0]) { if (level == 1)
    // byte_0x1BE_446 = 1; castle->life_0x8 = -1; }` — and it tests
    // NOTHING about the castle's state. An `action45 == 6` clause is
    // therefore a test the PORT invented, and it only happens to hold
    // for a demolish that lands on a castle standing at rest: the
    // standing tick converts life < 0 into action 6 in the SAME tick,
    // so the END record shows 6. A castle demolished again while it is
    // still inside the previous rung's build state machine (action 5)
    // never reaches the destroy intake that tick, so the clause
    // silently dropped the press.
    //
    // mc2l3's self-destruct is exactly that case, and it is the level's
    // certification blocker: the player HAMMERS Shift+L to walk the
    // castle down rung by rung, and the second press lands at t=15910
    // with the castle at action 5 / `word_0x2E_46` 3 (the repaint
    // painter it just minted). Retail writes life −1 anyway; the port
    // saw action 5 and replayed no press at all, so the whole rest of
    // the demolish ladder — and the re-site that opens the sealed
    // chamber — never happened.
    //
    // The kill EDGE (rather than `life == -1` outright) is what keeps
    // the parked castle from re-firing for the ~45 ticks it sits dead
    // at action 5 with the key still down; the write itself is
    // idempotent, but the +3000 surcharge latch below is not.
    let demolish = {
        let castle = cp.castle_ent as usize;
        let alive_before = pst
            .ents
            .get(castle)
            .is_some_and(|c| c.class3f == 3 && c.life >= 0);
        cp.castle_ent > 0
            && alive_before
            && st
                .ents
                .get(castle)
                .is_some_and(|c| c.class3f == 3 && c.life == -1)
            && key_held(input_end, 38)
            && (key_held(input_end, 42) || key_held(input_end, 54))
    };
    // The modal park (big map / spell book): the game keeps running
    // but the carpet stops dead. Witness = the consumed command at 0
    // AND the carpet entity pinned in place at zero speed across the
    // pair.
    //
    // ⚠⚠ THE OLD WITNESS'S OWN JUSTIFICATION WAS FALSE — it read "a
    // live zero-crossing never freezes the position too", and every
    // zero-crossing freezes it. `sub_5D530` runs the ±16 servo BEFORE
    // the polar step (EF:59636-44, then :59668), so on the very tick a
    // braking command reaches 0 the step is taken at speed 0 and x/y
    // do not move at all. mc2l3 t=605 is the counterexample: the down
    // key walks the command 16 → 0, retail's carpet holds 25240/48943
    // exactly, and all three park clauses fire on a carpet nobody
    // parked. Harmless while the harness pinned the command every
    // tick; fatal once the register carries itself, because the park
    // arm zeroes BOTH registers ahead of `sub_5F380` and the next key
    // press then integrates from 0 instead of 16.
    //
    // A held speed key is the discriminator: the modal screens eat the
    // movement keys, so a real park never carries one.
    //
    // THE FULL-STOP KEY (BACKSPACE = scancode 14, PlayerAction 0x27
    // EF:37954-65) zeroes BOTH `actSpeed_0x82` and `speed_0xc_12` in
    // one tick — the same effect the modal park models, so it rides
    // the same flag, read off `keys_down` like SPACE's respawn lane.
    // The key BYPASSES the position-frozen clause: a knockback
    // carries the carpet on the very tick of the press
    // (mc2l0-permadeath t=2191: x/y move 4 units under knock_mag 21;
    // t=2229 the same under a 26 → 22 decay), so "did not move" was
    // false exactly when the player braked under fire. The key-less
    // clause stays for the modal park; the mb & 3 guard covers both
    // against the mc2l3 t=605 zero-crossing false positive.
    let ci = cp.play_index as usize;
    // ⭐⭐⭐ THE STICK INVERSION HAS A SECOND WRITER TO UNDO.
    // `recover_stick` inverts `roll_acc[t] = roll_acc[t-1] +
    // trunc((2*stick - roll_acc[t-1]) / 4)` — but MC2's whirlwind
    // cranks the SAME accumulator by 28 on every NOT-YET-GRABBED visit
    // (`sub_33340` EF:24344-46 / `NETHERW.EXE` 0x57c86, and `sub_5D530`
    // reads that very word at 0x81d6d / 0x81df9). The seizure's own
    // fingerprint is in the recorded pool — but ⚠ NOT ONLY in the
    // swirl heading: the claim that these arms write `word_0x30_48`
    // (0x57d51) "every tick they hold you" is FALSE, and the near arm
    // leaves the mover-veto latch instead. [`mc2_crank_witness`] owns
    // the predicate and cites the bytes.
    // Without this, mc2l1 t=272/273/277/279/283/1343/1344 invert to NO
    // stick at all — retail's +35 at t=272 is outside the signed-byte
    // range the input pass reads (0x77482 `movsx eax,byte [eax+0x3]`)
    // — and the free run fabricates a centred cursor; mc2l30 t=2973
    // inverts to 85 where retail's cursor was 29, and the mover's
    // `yaw += roll/8` then steps the carpet 1 unit wide in x.
    // ⚠ ONE crank, not a count: the grabbed arms skip the `v40` block
    // (0x57c61 `jnz 0x57d65`) and a repeat visit inside one tick is not
    // separable from the recorded pair here. See the dig note.
    let whirl_crank = matches!((pst.ents.get(ci), st.ents.get(ci)),
        (Some(p), Some(c)) if mc2_crank_witness(p, c));
    let crank = |acc: i16| -> i16 {
        if whirl_crank && !no_mc2_whirl_roll_crank() {
            crank_units(acc, crank_k)
        } else {
            0
        }
    };
    let full_stop = key_held(input_end, 14);
    let mc2_park = matches!((pst.ents.get(ci), st.ents.get(ci)), (Some(p), Some(c))
        if mc2_park_witness(cp.cmd_speed, mb, c.speed, p.x == c.x && p.y == c.y, full_stop));
    // Pose NOT mover-driven this pair: frozen (a blocked move) or
    // warped further than any mover step could carry it (2048 is the
    // pose channel's own warp gate; the mover's reach is ~450).
    let pose_forced = matches!((pst.ents.get(ci), st.ents.get(ci)), (Some(p), Some(c))
        if (p.x == c.x && p.y == c.y)
            || (c.x.wrapping_sub(p.x) as i16).unsigned_abs() > 2048
            || (c.y.wrapping_sub(p.y) as i16).unsigned_abs() > 2048);
    let cur_speed = st.ents.get(ci).map_or(0, |c| c.speed);
    // ⭐⭐⭐ THE TICK'S OWN `rollDelta` IS IN THE CAPTURE — STOP GUESSING.
    // `PlayerEvents_51BB0` stores `rollDelta_0x4_4` at the player
    // struct's +4 (`NETHERW.EXE` 0x77480 `movsx eax,byte [eax+0x3]` /
    // `movsx edx,[ecx+0x155]` / `mov [ecx+0x4],ax`) and the recorder
    // captures it: `RetailMc2`'s `roll_delta`, which until now only
    // `explain` ever read. The delta recorded at N+1 is the delta tick
    // N+1 used, so the filter inverts against `acc + rollDelta`
    // EXACTLY and the whirlwind crank never enters the arithmetic at
    // all. Measured on mc2l24 t=7913..7919, seven consecutive ticks:
    // `roll_acc[N+1] == ladder(roll_acc[N] + rollDelta[N+1], k)` with
    // k = 2,1,1,2,2,3,2 — and t=7919's k=2 is a pair the un-cranking
    // inversion CANNOT reach (its ladder saturates at 257 from
    // `acc0 = 229`, yielding dr = -19 where the truth is -47).
    // ⚠ SCOPED TO THE WHIRLWIND TICKS, AND THAT SCOPE IS THE LAW.
    // Read unconditionally it is a −175-segment REGRESSION on mc2l24
    // (925 vs 750 segments, measured 2026-09-10): the inversion targets
    // the ACCUMULATOR, which is right whenever the input pass is its
    // only writer — death-fall decay (`sub_5E8C0`, EF:60577-87), paused
    // ticks where the stored delta is stale, and every ordinary tick
    // agree with `acc1 - acc0` exactly. `rollDelta` is only the INPUT
    // PASS'S SHARE of the move, so it wins precisely where a second
    // writer (the funnel's 0x57c86 store) is also moving the
    // accumulator and the count of its 28s is not recoverable from the
    // pair. Fall back to the inversion when no signed byte reproduces
    // the recorded delta (a slow-scaled or clipped capture).
    // The scale `sub_5D530`'s pose filter actually ran under — see
    // [`pair_move_speed`]: a web stamped ahead of the carpet's slot
    // raises `+0x14c` BEFORE the filter reads it at 0x81d5f.
    let ms_eff = pair_move_speed(pp.move_speed, cp.move_speed, cp.move_speed_ctr);
    let roll_from_delta = (whirl_crank && !no_mc2_roll_delta_capture())
        .then(|| {
            recover_stick_slowed(
                pp.roll_acc as i16,
                (pp.roll_acc as i16).wrapping_add(cp.roll_delta),
                ms_eff,
            )
        })
        .flatten();
    RecoveredPair {
        whirl_crank,
        stick_x: roll_from_delta.or_else(|| {
            recover_stick_slowed(
                pp.roll_acc as i16,
                (cp.roll_acc as i16).wrapping_sub(crank(pp.roll_acc as i16)),
                ms_eff,
            )
        }),
        stick_y: recover_stick_slowed(pp.pitch_acc as i16, cp.pitch_acc as i16, ms_eff),
        move_byte: mb,
        fire_left,
        fire_right,
        mc2_select,
        mc2_ring_cast,
        rebind_dropped,
        respawn,
        demolish,
        mc2_cmd_speed: Some(mc2_pair_cmd_speed(
            pp.cmd_speed,
            cp.cmd_speed,
            mb,
            pose_forced,
            cur_speed,
        )),
        mc2_park,
        cheat: cheat_fired_mc2(&pp.notify, &cp.notify),
        ..RecoveredPair::default()
    }
}

// ------------------------------------------------- capture-grade laws

/// Is an MC1 boundary gradeable? The recorder's snapshot can tear
/// across retail's entity pass; a torn snapshot grades nothing (the
/// consumer's chain runs on regardless). Tear witnesses: entities
/// whose per-tick byte did not advance exactly once across the pair,
/// and the global LCG not being exactly one step ahead of N's.
///
/// ⚠⚠ **A SLOT THAT WAS REAPED AND RE-MINTED IS NOT A TEAR WITNESS.**
/// The `+63` census assumes the record at a slot is the SAME entity at
/// both ends of the pair, and `class`/`model` equality does not
/// establish that: a mass-spawn burst recycles slots into the same
/// `(class, model)` constantly. Worse, the collision is SYSTEMATIC
/// rather than chance — `NewEvent` seeds `+63` from the slot index
/// (:43883), so a re-minted record lands on exactly the value its
/// predecessor had been walking, i.e. a delta of 0, i.e. a "tear".
///
/// The per-entity LCG (`+4`) settles it: `NewEvent` re-seeds it
/// (`slot + global rand`, :43882), so `rand` changing across the pair
/// means a DIFFERENT entity and the `+63` comparison is meaningless.
///
/// Measured on mc1l42's kraken clash, where this mattered: boundaries
/// t=6612..6623 were all called TORN on 3-7 suspects, **every one of
/// them a re-minted `(9,9)` beam node**, while the global-LCG clause —
/// the strong witness — passed at every single tick. The recording is
/// gapless and untorn there; the heuristic went blind for twelve ticks
/// precisely because the level was spawning hard, which is where the
/// grading is worth most. Un-blinding it moves mc1l42's first
/// divergence from t=6624 (a mass-spawn slot desync, six ticks
/// downstream and nearly unreadable) to t=6618 (one entity, one flag).
/// Is this boundary a PAUSED turn rather than a torn capture?
///
/// ⭐⭐⭐ **A PAUSED TURN IS ITS OWN CATEGORY — IT DRAWS AND DOES
/// NOTHING ELSE.** Retail's turn function steps the global LCG as its
/// FIRST statement and tests the pause flag as its second (MC1
/// `sub_41780_41AC0` :52197; MC2's frame function draws at EF:39947
/// and gates below), so a paused frame advances the RNG exactly one
/// step and leaves the pool untouched. That is indistinguishable from
/// a tear to [`capture_clean_mc1`], whose LCG clause PASSES while its
/// `+63` clause fires on every live entity at once — which is exactly
/// how mc1l6's 66.8-second opening pause was booked as 1,602 torn
/// boundaries and free-run through.
///
/// The test is deliberately STRICTER than the tear heuristic's: every
/// comparable entity must be frozen (`+63` delta exactly 0, not the
/// tear clause's `0 | 2`), none re-minted, and the LCG must have moved
/// one step. A real tear freezes SOME slots; a pause freezes ALL of
/// them.
///
/// Measured both games: mc1l6 t=1..1602 (P at t=0/1 and t=1602/1603)
/// and mc2l0-test's three cycles (P at 27/98, 143/186, 229/272), every
/// boundary reading one LCG step with zero pool changes.
pub fn paused_turn_mc1(pst: &RetailMc1, retail: &ObsMc1) -> bool {
    if pst.rand.wrapping_mul(9377).wrapping_add(9439) != retail.rng {
        return false;
    }
    let mut frozen = 0u32;
    for re in &retail.entities {
        let prev = &pst.ents[re.slot as usize];
        if prev.class64 == 0 || prev.class64 != re.class || prev.model65 != re.model {
            // A slot that changed class/model did something: not a pause.
            return false;
        }
        if re.rand != prev.rand || re.tick_byte != prev.f63 {
            return false;
        }
        frozen += 1;
    }
    frozen > 0
}

/// The MC2 twin of [`paused_turn_mc1`]: one LCG step, every live
/// entity's phase byte (`+0x3E`) frozen. Same law, same evidence
/// (`mc2l0-test`), different phase field — see [`capture_clean_mc2`].
pub fn paused_turn_mc2(pst: &RetailMc2, st: &RetailMc2) -> bool {
    if pst.rand.wrapping_mul(9377).wrapping_add(9439) != st.rand {
        return false;
    }
    let mut frozen = 0u32;
    for slot in 1..pst.ents.len().min(st.ents.len()) {
        let (a, b) = (&pst.ents[slot], &st.ents[slot]);
        if a.class3f == 0 {
            continue;
        }
        if a.class3f != b.class3f || a.model40 != b.model40 || a.phase3e != b.phase3e {
            return false;
        }
        frozen += 1;
    }
    frozen > 0
}

pub fn capture_clean_mc1(pst: &RetailMc1, retail: &ObsMc1) -> bool {
    let mut tear_suspects = 0u32;
    for re in &retail.entities {
        let prev = &pst.ents[re.slot as usize];
        if prev.class64 == 0 || prev.class64 != re.class || prev.model65 != re.model {
            continue;
        }
        if re.rand != prev.rand {
            continue; // re-minted slot: a different entity, not a tear
        }
        if matches!(re.tick_byte.wrapping_sub(prev.f63), 0 | 2) {
            tear_suspects += 1;
            if tear_suspects > 2 {
                return false;
            }
        }
    }
    let mut x = pst.rand;
    x = x.wrapping_mul(9377).wrapping_add(9439);
    x == retail.rng
}

/// Is an MC2 pair fixture-grade? Step-1 dominance of the per-entity
/// phase byte across entities live (same class+model) at both ends.
/// Pairs with no live-in-both population (never happens on real
/// levels) fail closed.
pub fn capture_clean_mc2(pst: &RetailMc2, st: &RetailMc2) -> bool {
    let (mut d0, mut d1, mut d2) = (0u32, 0u32, 0u32);
    for slot in 1..pst.ents.len().min(st.ents.len()) {
        let (a, b) = (&pst.ents[slot], &st.ents[slot]);
        if a.class3f == 0 || a.class3f != b.class3f || a.model40 != b.model40 {
            continue;
        }
        match b.phase3e.wrapping_sub(a.phase3e) {
            0 => d0 += 1,
            1 => d1 += 1,
            2 => d2 += 1,
            _ => {}
        }
    }
    d1 > 0 && d1 >= d0 && d1 >= d2
}

#[cfg(test)]
mod cheat_tests {
    use super::*;

    fn notify(text: &str, ticks: u16) -> Notify {
        let mut raw = [0u8; Notify::CAP];
        raw[..text.len()].copy_from_slice(text.as_bytes());
        Notify::from_parts(raw, ticks)
    }

    /// The counter is what dates a cheat, not the text: retail re-arms
    /// it to 100 and it ticks DOWN, so a snapshot taken after the
    /// firing tick reads 99. Any INCREASE is a fresh `ShowMessage`.
    /// Both engines agree on this much.
    #[test]
    fn a_repeat_of_the_same_cheat_is_a_fresh_fire() {
        let a = notify(".. CHEAT: more mana", 96);
        let b = notify(".. CHEAT: more mana", 99);
        // …and the ordinary count-down in between is not.
        let c = notify(".. CHEAT: more mana", 98);
        for fired in [cheat_fired_mc1, cheat_fired_mc2] {
            assert_eq!(fired(&a, &b), Some(Cheat::MoreMana));
            assert_eq!(fired(&b, &c), None);
        }
    }

    /// ⚠⚠ THE EXPIRED SLOT IS WHERE THE TWO ENGINES PART, AND GETTING
    /// IT WRONG RE-FIRES THE CHEAT FOREVER.
    ///
    /// remc1 decrements only from inside `if (periods > 0)`
    /// (:26526/:26531), so an MC1 toast CLAMPS at 0 and holds there
    /// with its text intact. Applying MC2's rule — which expects the
    /// step off 0 to wrap to 0xFFFF — reads every one of those
    /// stationary boundaries as a fresh fire, which is exactly what
    /// made an MC1 cheat take dispense mana on every tick from ~100
    /// ticks after the single real press.
    #[test]
    fn an_expired_mc1_toast_clamps_at_zero_and_never_refires() {
        let dead = notify(".. CHEAT: more mana", 0);
        assert_eq!(cheat_fired_mc1(&dead, &dead), None);
        // The last real step down, and the clamp it lands on.
        assert_eq!(
            cheat_fired_mc1(&notify(".. CHEAT: more mana", 1), &dead),
            None
        );
        // NON-VACUITY: the MC2 rule is what got this wrong.
        assert!(dead.fired_since_mc2(&dead));
        assert!(!dead.fired_since_mc1(&dead));
        // A genuine re-press out of the clamped slot still registers.
        assert_eq!(
            cheat_fired_mc1(&dead, &notify(".. CHEAT: more mana", 99)),
            Some(Cheat::MoreMana)
        );
    }

    /// MC1 ages the slot in the RENDER loop, so a recorded boundary
    /// with no intervening draw leaves the counter unmoved. That is a
    /// hold, not a fire — and unlike MC2 there is no `== prev - 1`
    /// requirement to trip over.
    #[test]
    fn an_mc1_boundary_without_a_draw_is_a_hold_not_a_fire() {
        let a = notify(".. CHEAT: heal", 57);
        assert_eq!(cheat_fired_mc1(&a, &a), None);
        // Two frames drawn inside one recorded boundary: still decay.
        assert_eq!(cheat_fired_mc1(&a, &notify(".. CHEAT: heal", 55)), None);
    }

    /// The MC2 side of the same seam, kept pinned beside its twin: the
    /// counter steps off 0 to 65535 and PARKS, and a real fire out of
    /// that parked slot is the `@65535 -> @99` shape (the
    /// mc2l0-spells-galore t=909 corpus row).
    #[test]
    fn an_expired_mc2_toast_parks_on_ffff() {
        let zero = notify(".. CHEAT: more mana", 0);
        let parked = notify(".. CHEAT: more mana", u16::MAX);
        assert_eq!(cheat_fired_mc2(&zero, &parked), None);
        assert_eq!(cheat_fired_mc2(&parked, &parked), None);
        assert_eq!(
            cheat_fired_mc2(
                &notify("Lightning Tower", u16::MAX),
                &notify(".. CHEAT: more mana", 99)
            ),
            Some(Cheat::MoreMana)
        );
    }

    /// Non-cheat toasts share the lane (level-up re-arms to 200, the
    /// spell-select toast to 20) and must never be mistaken for one.
    #[test]
    fn ordinary_toasts_are_not_cheats() {
        let a = notify("", 0);
        for fired in [cheat_fired_mc1, cheat_fired_mc2] {
            assert_eq!(fired(&a, &notify("Lightning Tower", 19)), None);
            assert_eq!(
                fired(&a, &notify("has been banished from the realm.", 99)),
                None
            );
        }
    }

    /// The ON/OFF toggles share a prefix — the recording says it
    /// flipped, and retail's own flag is the direction.
    #[test]
    fn toggle_cheats_match_on_the_shared_prefix() {
        let a = notify("", 0);
        for s in [
            ".. CHEAT: Free Spell Usage ON",
            ".. CHEAT: Free Spell Usage OFF",
        ] {
            assert_eq!(cheat_fired_mc1(&a, &notify(s, 99)), Some(Cheat::FreeSpell));
            assert_eq!(cheat_fired_mc2(&a, &notify(s, 99)), Some(Cheat::FreeSpell));
        }
    }

    /// Every handler string in both engines resolves, and the sub-code
    /// round-trips (the `.mgcr` port-input lane depends on it).
    #[test]
    fn every_toast_maps_and_every_code_round_trips() {
        let a = notify("", 0);
        for &(text, want) in CHEAT_TOASTS {
            assert_eq!(cheat_fired_mc1(&a, &notify(text, 99)), Some(want), "{text}");
            assert_eq!(cheat_fired_mc2(&a, &notify(text, 99)), Some(want), "{text}");
            assert_eq!(Cheat::from_code(want.code()), Some(want), "{text}");
        }
        // The port-only win-level code has no retail toast, but its
        // sub-code must round-trip like the rest (the `.mgcr`
        // port-input lane carries it).
        assert_eq!(Cheat::from_code(Cheat::WinLevel.code()), Some(Cheat::WinLevel));
    }
}

#[cfg(test)]
mod park_tests {
    use super::mc2_park_witness;

    /// The BACKSPACE full stop (PlayerAction 0x27) vs the modal park
    /// — measured rows: mc2l0-permadeath t=2191 (park under a
    /// same-tick knockback: position MOVED, key down) and t=2229
    /// (same, knock 26 → 22); mc2l3 t=605 (a braking zero-crossing
    /// with the down key held is NOT a park, key or no key).
    #[test]
    fn the_full_stop_key_parks_through_a_same_tick_knockback() {
        // The knockback carries the carpet: frozen = false. Key down
        // → parks; key up (the old heuristic) → dropped.
        assert!(mc2_park_witness(0, 0, 0, false, true), "t=2191");
        assert!(
            !mc2_park_witness(0, 0, 0, false, false),
            "the pre-key heuristic dropped it"
        );
        // The key-less modal park still parks on the frozen clause.
        assert!(mc2_park_witness(0, 0, 0, true, false), "modal park");
        // The mc2l3 t=605 zero-crossing: held speed key blocks both.
        assert!(!mc2_park_witness(0, 2, 0, true, true), "t=605");
    }
}

#[cfg(test)]
mod wall_stop_tests {
    use super::mc2_pair_cmd_speed;

    /// ⭐ THE WALL/WARP DEAD-STOP IS A SIM EFFECT, NOT AN INPUT
    /// ([`mc2_pair_cmd_speed`]). Pair-BLIND by construction — the
    /// recovery is harness machinery, so no `.mgcr` fixture can pin
    /// it; all three rows below are measured off mc2l3.
    #[test]
    fn the_mc2_dead_stop_is_undone_before_the_command_is_replayed() {
        // The ordinary pair: the capture's command IS what the frame
        // served, whatever the carpet did.
        assert_eq!(mc2_pair_cmd_speed(48, 64, 1, false, 64), 64, "live command");
        assert_eq!(
            mc2_pair_cmd_speed(48, 64, 1, true, 64),
            64,
            "a frozen carpet with a NONZERO command is not a dead-stop"
        );
        // mc2l3 t=2639 — backing into a wall at the floor. The command
        // reads 0 at N+1 because the block wiped it; the frame served
        // −80, and the guarded integration cannot step past ±80, so
        // the servo holds actSpeed at −80 for that frame.
        assert_eq!(
            mc2_pair_cmd_speed(-80, 0, 2, true, -80),
            -80,
            "the wiped command is the pre-block one, and ±80 is a floor"
        );
        // mc2l3 t=3407 — flying into a wall at 48 with forward HELD:
        // `sub_5F380` integrated the command to 64 before the mover,
        // and only then did the block wipe it.
        assert_eq!(
            mc2_pair_cmd_speed(48, 0, 1, true, 48),
            64,
            "the frame's own ±16 key integration still applies"
        );
        // mc2l3 t=3933 — a Teleport places the carpet 119 tiles away
        // and clears the command; `pose_forced` covers the warp too.
        assert_eq!(
            mc2_pair_cmd_speed(-16, 0, 16, true, -16),
            -16,
            "a warp clears the command the same way a wall does"
        );
        // The modal park is the `speed == 0` twin and must NOT be
        // re-armed: it really did zero the command as an input.
        assert_eq!(
            mc2_pair_cmd_speed(-80, 0, 0, true, 0),
            0,
            "the modal park stays disjoint from the dead-stop"
        );
        // mc2l3 t=6514..6516 — the SECOND and later frames of one
        // block: N's command is already wiped, forward is still held,
        // and retail's servo stalls actSpeed at 16 rather than
        // decaying to 0. Demanding `prev_cmd != 0` read this as an
        // ordinary zero command and dropped the carpet a tick early.
        assert_eq!(
            mc2_pair_cmd_speed(0, 0, 1, true, 16),
            16,
            "a multi-frame block re-integrates from an already-wiped command"
        );
        // ...and with no key held the wiped command really is 0, so
        // the widened gate stays a no-op on every quiet blocked frame.
        assert_eq!(
            mc2_pair_cmd_speed(0, 0, 0, true, 16),
            0,
            "no key, nothing to re-integrate"
        );
    }
}

#[cfg(test)]
mod respawn_dating_tests {
    use super::*;
    use crate::mgcr::{RetailEntMc1, RetailWizardMc1};

    /// One MC1 closure with the local wizard's carpet parked in pool
    /// slot 1 (`play_index`), carrying retail's own life word: negative
    /// while the carpet is down, back above zero on the frame
    /// `sub_44D30` reseats and refills it.
    fn closure(act_life: i32) -> RetailMc1 {
        let mut st = RetailMc1 {
            wizards: vec![RetailWizardMc1::default()],
            ents: vec![RetailEntMc1::default(); 4],
            ..RetailMc1::default()
        };
        st.wizards[0].play_index = 1;
        st.ents[1].act_life = act_life;
        st
    }

    /// The END record's raw input channel — scancode 57 is SPACE, the
    /// respawn key ([`respawn_key`]).
    fn space(held: bool) -> serde_json::Value {
        serde_json::json!({ "keys_down": if held { vec![57] } else { Vec::<i64>::new() } })
    }

    /// A pair's recovered `respawn` verdict, END-record key and all.
    fn respawn(prev_life: i32, cur_life: i32, key: bool) -> bool {
        let input = space(key);
        recover_pair_mc1(&closure(prev_life), &closure(cur_life), Some(&input)).respawn
    }

    /// ⭐ THE MC1 RESPAWN IS DATED BY RETAIL'S OWN REVIVAL, NOT BY THE
    /// SPACE KEY. SPACE is MC1's last raw-key lane and the only one
    /// carrying the ±1-tick attribution caveat (the held scancodes live
    /// in the static frame, sampled outside the consensus window), and
    /// the caveat is NOT a constant offset — mc1l49 records BOTH SIGNS
    /// inside one take, so no fixed shift can fix the key lane. Rows
    /// below are that take's two deaths, `mc1l49-5487-7690`.
    ///
    /// Pair-BLIND harness machinery like the wall-stop above: the
    /// recovery runs between records, so no `.mgcr` fixture reaches it.
    #[test]
    fn the_mc1_respawn_is_dated_by_the_revival_not_the_space_key() {
        // Death A — SPACE first appears at record 5723 while the
        // carpet is still down at −428, and retail revives it across
        // 5723 → 5724. The key is a tick EARLY here, and the state
        // refuses the early pair.
        assert!(
            !respawn(-428, -428, true),
            "the key alone respawns mc1l49's first death a tick early"
        );
        assert!(
            respawn(-428, 10000, true),
            "the pair whose END record has the carpet back above zero IS the respawn"
        );
        // Death B — the same key, dated exactly: SPACE first appears at
        // record 7072 and retail revives across 7071 → 7072. One take,
        // both signs; the state lane lands on the revival either way.
        assert!(respawn(-1705, 10000, true), "mc1l49 t=7072, the on-time death");
        // The key stays as CORROBORATION — a revival with no held
        // SPACE across the pair is not the player's respawn command.
        assert!(!respawn(-428, 10000, false), "no key, no respawn");
        // A live carpet never respawns, key or no key (the `< 0 → > 0`
        // crossing is the whole witness, not just the end state).
        assert!(!respawn(10000, 10000, true), "a carpet that never died");
        // NON-VACUITY: the first row IS the kill switch's lane — run
        // this test under `MGC_NO_MC1_RESPAWN_STATE_DATING=1` and the
        // bare key fires on a pair with no revival in it, so the
        // assertion fails. Every other row holds under both arms.
    }
}

#[cfg(test)]
mod whirl_crank_tests {
    use super::*;

    /// ⭐⭐⭐ **THE WHIRLWIND CRANKS THE SAME ACCUMULATOR THE STICK
    /// INVERSION READS, AND WITHOUT UNDOING IT THE INVERSION IS EITHER
    /// IMPOSSIBLE OR SILENTLY WRONG.** `sub_33340`'s human arm
    /// (EF:24344-46; `NETHERW.EXE` file 0x57c67 `cmp byte [ebp-0x4],0`
    /// / 0x57c73 `mov cx,[eax+0x155]` / 0x57c7a `cmp cx,0x100` /
    /// 0x57c83 `add esi,0x1c` / 0x57c86 `mov [eax+0x155],si`) adds 28
    /// to `roll_0x155_341`, and `sub_5D530` reads that very field
    /// (0x81d69 `mov ax,[ecx+0x4]` / 0x81d6d `add [ecx+0x155],ax`, and
    /// 0x81df9 `movsx edx,word [edx+0x155]` for the yaw rate). The
    /// stick itself is a signed BYTE (`PlayerEvents_51BB0` 0x77482
    /// `movsx eax,byte [eax+0x3]`), which is why the ±127 clamp is the
    /// failure witness.
    ///
    /// Both rows are the recorded corpus, not synthetic:
    /// - mc2l1 t=271→272 `roll_acc` 167 → 202 — NO byte-range stick
    ///   reaches +35, so the free run fabricated a centred cursor and
    ///   drove the accumulator to 126 instead;
    /// - mc2l30 t=2972→2973 `roll_acc` 33 → 67 — recoverable BOTH
    ///   ways, and the crank-free answer (85) is the wrong one: it
    ///   lands `yaw += roll/8` on 121 where retail had 117, and the
    ///   mover then steps the carpet to x 50981 against retail's
    ///   50980. That single unit was the take's free-run horizon.
    #[test]
    fn the_whirlwind_crank_is_part_of_the_stick_inversion() {
        // mc2l1 t=272 — impossible without the crank.
        assert_eq!(recover_stick(167, 202), None);
        assert_eq!(recover_stick(167, 202 - 28), Some(98));
        // …and the recovered cursor is CONTINUOUS with its neighbour
        // (t=271 is 44 with the crank, 102 without): the crank-free
        // series jumps, the cranked one does not.
        assert_eq!(recover_stick(155, 167 - 28), Some(44));
        // mc2l30 t=2973 — recoverable either way, and only the
        // cranked answer reproduces retail's mover.
        assert_eq!(recover_stick(33, 67), Some(85));
        assert_eq!(recover_stick(33, 67 - 28), Some(29));
    }

    /// ⭐⭐⭐ THE WEB SLOW SCALES THE POSE FILTER, SO THE INVERSION HAS
    /// TO UNSCALE IT. `sub_5D530` adds `slow_scale(dr)`, not `dr`
    /// (EF:59622-30, `(v · (4 − moveSpeed))/4` round-toward-zero), so
    /// on a slowed carpet the RECORDED accumulator step is already
    /// scaled and inverting the raw law recovers the wrong cursor —
    /// which the mover then scales a second time.
    ///
    /// mc2l24 t=15551 → 15552, `moveSpeed = 1` (the recorded player
    /// block's `+332`; `move_speed_ctr` walks 4 → 3 across the pair).
    /// Retail: `roll_acc −5 → 1`, and the recorded `rollDelta_0x4_4`
    /// is **8** — `trunc(8·3/4) = 6`, exactly the step taken.
    /// `pitch_acc 17 → 14` on a raw −4 (`trunc(−4·3/4) = −3`).
    ///
    /// ⚠ NON-VACUITY: this pair is INVISIBLE to the fixture suite —
    /// the pose PAIR channel's `GATE_DEBUFF` refuses every web-slowed
    /// pair, so only the SEGMENTED free-run lane (and
    /// `MGC_POSE_GRADE_DEBUFF=1`) can see the law at all. A recording
    /// fixture anchored at head−1 would be vacuous; this is the pin.
    #[test]
    fn the_web_slow_scale_comes_off_the_stick_inversion() {
        // move_speed 0 is the identity — every un-slowed take is
        // byte-identical to the old inversion.
        for (a, b) in [(167i16, 174i16), (33, 67), (-5, 1), (17, 14), (155, 139)] {
            assert_eq!(recover_stick_slowed(a, b, 0), recover_stick(a, b));
        }
        // The mc2l24 t=15552 roll lane. The OLD answer, 10, is what
        // the port replayed — and it lands on −1, not retail's 1.
        assert_eq!(recover_stick(-5, 1), Some(10));
        let slow = |s: i32, acc: i32, m: i32| acc + ((2 * s - acc) / 4) * (4 - m) / 4;
        assert_eq!(slow(10, -5, 1), -1, "the replayed (wrong) cursor");
        let sx = recover_stick_slowed(-5, 1, 1).expect("a slowed cursor exists");
        assert_eq!(slow(sx as i32, -5, 1), 1, "the recovered cursor reproduces retail");
        // …and the same tick's pitch lane, where the old answer was 1.
        assert_eq!(recover_stick(17, 14), Some(1));
        assert_eq!(slow(1, 17, 1), 15, "the replayed (wrong) cursor");
        let sy = recover_stick_slowed(17, 14, 1).expect("a slowed cursor exists");
        assert_eq!(slow(sy as i32, 17, 1), 14, "the recovered cursor reproduces retail");
        // Every reachable slow level round-trips, both signs, across
        // the accumulator's live band.
        for m in 1u8..=3 {
            for acc in [-300i16, -37, -5, 0, 17, 199, 255, 301] {
                for s in -128i32..=127 {
                    let k = 4 - m as i32;
                    let a = acc as i32;
                    let end = a + ((2 * s - a) / 4) * k / 4;
                    if let Ok(end16) = i16::try_from(end) {
                        let got = recover_stick_slowed(acc, end16, m)
                            .expect("the forward image is always invertible");
                        let a2 = acc as i32;
                        assert_eq!(
                            a2 + ((2 * got as i32 - a2) / 4) * k / 4,
                            end,
                            "m={m} acc={acc} s={s} -> {got}"
                        );
                    }
                }
            }
        }
    }
    /// ⭐⭐⭐ THE CRANK IS A LADDER, NOT A MULTIPLY, AND THE PAIR ALONE
    /// CANNOT COUNT IT. `sub_33340` re-tests `< 256` before every add
    /// (`NETHERW.EXE` 0x57c7a `cmp $0x100,%cx` / 0x57c7f `jge 0x57c8d`
    /// — inside the per-visit `v40` block, so the test is per visit),
    /// which is why [`crank_units`] walks instead of multiplying.
    ///
    /// The second half is the reason the count has to come from the
    /// SIMULATION: mc2l24 t=7913 records `roll_acc` 31 -> 87, and both
    /// k=1 and k=2 invert to a legal signed-byte cursor (72 and the
    /// smallest-magnitude 14), while k=0 does not exist at all. The
    /// recording cannot choose; the port's own ring walk can, and does
    /// (two mid-ring visits as `CopyEntityPosition_57CF0` carries the
    /// carpet from cell (124,116) to (123,116)).
    #[test]
    fn the_whirl_crank_is_a_ladder_and_the_pair_cannot_count_it() {
        assert_eq!(crank_units(31, 0), 0);
        assert_eq!(crank_units(31, 1), 28);
        assert_eq!(crank_units(31, 2), 56);
        // The `< 256` gate stops the ladder mid-climb — 240 takes one
        // more 28 (to 268) and then no more, so three visits still
        // only bank 28.
        assert_eq!(crank_units(240, 1), 28);
        assert_eq!(crank_units(240, 3), 28);
        assert_eq!(crank_units(256, 4), 0);

        // mc2l24 t=7913: 31 -> 87. k=0 is impossible (the cursor is a
        // signed byte, 0x77482 `movsx eax,byte [eax+0x3]`), k=1 and
        // k=2 are both legal — the ambiguity the trial step resolves.
        let target = |k: u8| (87i16).wrapping_sub(crank_units(31, k));
        assert_eq!(recover_stick(31, target(0)), None, "k=0 needs a cursor of 128");
        assert_eq!(recover_stick(31, target(1)), Some(72));
        assert_eq!(recover_stick(31, target(2)), Some(14));
    }

    /// ⭐⭐⭐ A WIZARD HELD BY A FUNNEL WHOSE SWIRL HEADING DID NOT
    /// CHANGE STILL GETS CRANKED. The three mc2l24 heads this pins are
    /// the SAME tick shape: the near arm seizes, sets the mover-veto
    /// one-shot (`flags & 0x800`) and cranks — and never touches
    /// `word_0x30_48`, because its grab roll failed
    /// (`NETHERW.EXE` 0x57d2c/0x57d34 is the only near-arm writer).
    /// See [`mc2_crank_witness`] for the byte-level derivation.
    ///
    /// ⚠ NON-VACUITY: every `assert` below flips under
    /// `MGC_NO_MC2_WW_VETO_WITNESS=1` — the three `witness` rows read
    /// FALSE and the three `recover_stick` rows then invert against
    /// the un-cranked accumulator, which is the exact arithmetic the
    /// port replayed (and, at t=46814, cannot replay at all).
    #[test]
    fn a_held_wizard_is_cranked_even_when_the_swirl_heading_repeats() {
        // (p.f30, c.f30, p.flags, c.flags) as recorded on the human,
        // slot 116, at each head.
        let pair = |pf30: u16, cf30: u16, pflags: u32, cflags: u32| {
            let mut p = RetailEntMc2 { class3f: 3, model40: 0, ..RetailEntMc2::default() };
            let mut c = p;
            p.f30 = pf30;
            c.f30 = cf30;
            p.flags = pflags;
            c.flags = cflags;
            (p, c)
        };
        // mc2l24 t=44613, 46814, 50677 — the heading never moves, the
        // veto latch appears, the grab latch is clear at the head.
        for (pf30, cf30, pflags, cflags) in
            [(1390u16, 1390u16, 269u32, 2317u32), (1369, 1369, 525, 2573), (1810, 1810, 525, 2573)]
        {
            let (p, c) = pair(pf30, cf30, pflags, cflags);
            assert!(mc2_crank_witness(&p, &c), "the near-arm seizure is a crank");
        }
        // …and the tick AFTER the grab latch lands is NOT a crank:
        // the grabbed arm (0x57d65..0x57d78) sets the same 0x800 and
        // skips the `v40` block. mc2l24 t=44617: flags 268437773 on
        // BOTH sides, `roll_acc` 65523 -> 65523 with `rollDelta` 0.
        let (p, c) = pair(1369, 1369, 268437773, 268437773);
        assert!(!mc2_crank_witness(&p, &c), "an already-grabbed wizard is not cranked");
        // A pair with neither witness (mc2l24 t=46813: flags
        // 269 -> 525, no 0x800, heading parked) stays out.
        let (p, c) = pair(1369, 1369, 269, 525);
        assert!(!mc2_crank_witness(&p, &c), "0x100 is not the veto bit");
        // A rival wizard is class 3 model 0 too, but a non-wizard
        // record never reaches 0x57bf3's `[ebp-4] = 1`.
        let mut p = RetailEntMc2 { class3f: 5, model40: 20, ..RetailEntMc2::default() };
        p.flags = 525;
        let mut c = p;
        c.flags = 2573;
        assert!(!mc2_crank_witness(&p, &c), "only class 3 model 0 is cranked");

        // The arithmetic the witness buys, at the three heads. Each
        // row is `roll_acc[N] -> roll_acc[N+1]` with the recorded
        // `rollDelta_0x4_4`: the cranked target is `acc + rollDelta`,
        // and the UN-cranked one is what the port inverted instead.
        // t=44613: -147 -> -97, rollDelta 22.
        assert_eq!(recover_stick(-147, -147 + 22), Some(-28));
        assert_eq!(recover_stick(-147, -97), Some(27), "the crank absorbed into the cursor");
        // t=50677: 205 -> 186, rollDelta -47.
        assert_eq!(recover_stick(205, 205 - 47), Some(7));
        assert_eq!(recover_stick(205, 186), Some(63), "the crank absorbed into the cursor");
        // t=46814: 174 -> 206, rollDelta 4. Un-cranked, the step is
        // +32 and NO signed byte reaches it — the pair was reported
        // stick-unrecoverable and the pose lane gated.
        assert_eq!(recover_stick(174, 174 + 4), Some(95));
        assert_eq!(recover_stick(174, 206), None, "+32 needs a cursor of 151");
        // …and the ladder confirms the recorded step is one crank:
        for (acc, delta, end) in [(-147i16, 22i16, -97i16), (205, -47, 186), (174, 4, 206)] {
            assert_eq!(acc + delta + crank_units(acc + delta, 1), end);
        }
    }

    /// ⭐⭐⭐ A WEB STAMPED AHEAD OF THE CARPET SCALES THE SAME TICK'S
    /// POSE FILTER. `+0x14c` has three writers per tick and they run
    /// in this order: `sub_38E70`'s SLOW STAMP at the stamping
    /// record's own walk slot (`NETHERW.EXE` file 0x5d6ae `mov
    /// dh,[eax+0x14c]` / 0x5d6b4 `cmp dh,3`), `sub_5D530`'s POSE
    /// FILTER read at 0x81d5f, and `sub_5D530`'s DECAY WALK at
    /// 0x8216e-0x821ae. Reading the pair's END latch — right for the
    /// decay — is wrong for the stamp, and the recorded
    /// `move_speed_ctr` says which happened (`slow_hit` writes 8,
    /// `tick_debuffs` immediately takes it to 7).
    ///
    /// mc2l24 t=8699 is the take's free-run horizon and it closes on
    /// exactly this: `move_speed 0 -> 1`, `move_speed_ctr 0 -> 7`,
    /// `roll_acc -23 -> -28`, capture `rollDelta_0x4_4` = -7.
    #[test]
    fn a_slow_stamped_ahead_of_the_carpet_scales_the_same_ticks_filter() {
        assert_eq!(pair_move_speed(0, 1, 7), 1, "the stamp beat the mover (ctr 8 -> 7)");
        assert_eq!(pair_move_speed(0, 1, 8), 0, "the stamp landed behind the carpet's slot");
        assert_eq!(pair_move_speed(1, 0, 0), 1, "the decay walk runs AFTER the filter");
        assert_eq!(pair_move_speed(2, 1, 8), 2, "a decay to a still-live level, same rule");
        assert_eq!(pair_move_speed(1, 1, 3), 1, "a steady level is its own answer");
        assert_eq!(pair_move_speed(3, 3, 7), 3, "a stamp at the cap moves nothing");

        // The forward law, `sub_5D530` 0x81d8b-0x81db1.
        let slow = |s: i32, acc: i32, m: i32| acc + ((2 * s - acc) / 4) * (4 - m) / 4;
        let ms = pair_move_speed(0, 1, 7);
        // The OLD reading (tick N's END latch, 0) inverts the UNSCALED
        // law, recovers a cursor whose `dr` is -5, and the mover then
        // scales THAT: roll_f -26, the port's reported value.
        let old = recover_stick_slowed(-23, -28, 0).expect("an unslowed cursor exists");
        assert_eq!((2 * old as i32 + 23) / 4, -5);
        assert_eq!(slow(old as i32, -23, 1), -26, "the value the port reported");
        // The law's reading recovers the cursor whose `dr` is the
        // RECORDED -7 and lands retail's -28.
        let sx = recover_stick_slowed(-23, -28, ms).expect("a slowed cursor exists");
        assert_eq!((2 * sx as i32 + 23) / 4, -7, "the recorded rollDelta_0x4_4");
        assert_eq!(slow(sx as i32, -23, 1), -28, "retail's roll_acc at mc2l24 t=8699");
    }
}
