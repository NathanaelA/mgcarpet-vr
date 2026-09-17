//! MC2 multipart/segment-chain subsystem — class-5 models 0 (worm/
//! hydra), 3 (multipart flyer), 22 (segmented worm / castle-mana
//! thief) and 27 (3-tier tree kraken), ported from the trace bank:
//! - docs/traces/mc2-multipart-chains.md (ctors, segment tick,
//!   topology, collision/list skips)
//! - docs/traces/mc2-m0-m3-gaps.md (dispatch table, tether, bob,
//!   PreKillEntity cascade, unrecovered states)
//! - docs/traces/mc2-m22-worm-helpers.md (the m22 helper suite,
//!   drain chain, colorize walkers)
//! - docs/traces/mc2-m27-branch-machine.md (sub_29A90, positioning
//!   spline, data tables)
//!
//! `EF:` cites = remc2 EventsFunctions.cpp.
//!
//! Chain topology (shared): entities link via f52 (toward the head,
//! `word_0x32_50`) and f54 (toward the tail, `word_0x34_52`) — the
//! same homes MC1's worms use. Segment states are collision- and
//! scan-transparent (0xE8 m0/m3 children, 0xB4 m22 tail, 0xEA m27
//! tier-2; 0xE9 m27 branches ARE scannable — EF:39987-40009) — the
//! mc2 scan helpers already carry the exclusions.
//!
//! Field homes beyond the [`super::mobs`]/[`super::roster`] docs
//! (all remc2 names verbatim):
//! - `word_0x36_54` (link length; m22 head reuses it as the 2-bit
//!   writhe phase) → f56 — the MC1 worm's own home. The MC2 burn
//!   mask `byte_0x38_56` (never read at runtime; f28=1 is the
//!   cross-column admit) is NOT stored for these models.
//! - `byte_0x46_70` → f71 (m0 dodge-hook flag · m22 tail length on
//!   the head / SIGNED ring offset on segments, stored as the u8
//!   cast · m27 branch sub-state).
//! - `word_0x2C_44` → f44 (m0 dodge step timer · m22 spin rate ·
//!   m27 speed-mode selector).
//! - `fontTypeIndex_0x3D_61` (m0 dodge-alert window) → f46 — the
//!   effect columns' fontTypeIndex home (flood/tail), free on
//!   these heads.
//! - `subSpellIndex_0x2A_42` (m22 serpentine spiral angle) → f46 —
//!   deviation from the projectile column's f44 home, which m22
//!   already occupies with `word_0x2C_44`.
//! - `word_0x24_36` → f38 (m22 rise budget · m27 body exposure
//!   attacker · m0 hooked-projectile ref). m22/m27 heads never run
//!   the shared inbox; the m0 head DOES, and its death write
//!   (`mc2_state_head` kill-credit, retail's own field reuse)
//!   aliases the hook — [`Gen::m0_dodge`] bounds-guards the read
//!   (the out-of-pool human sentinel must read as "gone").
//! - `word_0x96_150` → f146 (m22 head grow timer / segment head-ref
//!   · m27 branch target). `playerEntityIndex_0x94_148` (the m22
//!   target player) → f144, the uniform @0x94 home the importer
//!   restores per pair and the graded `player_ent_idx` obs lane
//!   reads. (It used to ride dest_x, which the importer fills from
//!   the dead @0x9A — every replayed pair forgot the target: the
//!   0xB1 sweep recolored with the neutral base 52 instead of the
//!   owner's 105 (mc2l22 t=16-24 f5a 109/56) and ended in 0xB0
//!   instead of 0xB2 (t=27 action 178/177), after which the stale
//!   segment tags re-fired the relay.)
//! - `byte_0x3B_59` (m27 branch index 0..4 / the BODY's live-branch
//!   gauge) → f50 — free on this family (MC1's damage-response
//!   countdown never runs on MC2 creatures).
//! - `byte_0x43_67`/`byte_0x44_68` (m27 whip counters) → f68/f69 —
//!   the projectile impact pair is meaningless on creatures.
//! - `manaRegen_0x88_136` (m27 bolt power 1|2) → f136. The uniform
//!   MC2 import spends f136 on @0x8C, so `import_ent_mc2` carries a
//!   (5,27) home for it (@0x8C is dead 0 on the family) — without it
//!   every replayed pair re-read the power as 0 and the four a3=0
//!   RE-FIRES of each whip no-opped (one (9,9) arc per whip, not five).
//! - `word_0x5A_90` (particle/sprite row) → type86;
//!   `animationFrame_0x5C_92` → frame88.
//!
//! DELIBERATE APPROXIMATIONS (flagged in place too):
//! - m0 state 0x06 (`sub_1F2B0`) and its m3 twin state 0x1E
//!   (`sub_1FA40`) are compiled EMPTY STUBS in the shipped binary
//!   (files 0x43AB0/0x44240: push ebp/mov ebp,esp/pop ebp/ret —
//!   docs/AUDIT-STUBBED-ARMS-2026-07-26.md) — the no-op arms are
//!   FAITHFUL, not guesses. The trace §1/§8 "tether is dormant"
//!   claim was FALSE — see [`Gen::m0_dodge`]; m3's recovered
//!   states never call the tether and `sub_68BD0` arms model 0
//!   only, so m3 keeps no dodge.
//! - `struct_byte_0xc` group markers (m27 byte[2]/byte[3] bits, the
//!   m22 byte[2]|=0x20 sound split) are not modeled; the m27
//!   show/hide of segments (byte[0] bit 0) writes flags bit 0
//!   VERBATIM (the awake pass's hidden-skip and retail's 0x21 draw
//!   law read it) PLUS the port's 0x20 draw alias (the renderer's
//!   billboard skip — widening it to 0x21 globally would break the
//!   MC2 map-only house pose and the cave balloon), and the burrow
//!   ops carry the bit-3 targetable toggle (flags 0x08) — the
//!   billboard-suppress bit live_poses already honors.
//! - `byte_0x5D_93` (the sprite's ANIMATION FRAME COUNT — NOT a
//!   palette shade; `sub_585A0` reads it in the sim) is stamped by
//!   `sub_49D50` alongside type86 and the f78 spin, since round 98.
//! - m27 `sub_2A7F0`'s low-power path perturbs the branch LCG by
//!   the global `setting_30` counter — the post-increment turn
//!   (incremented beside `Turn++` in PlayerEvents, EF:37557;
//!   `MobCtx::mc2_turn` carries it). Modeled via
//!   [`Gen::mc2_rand_perturb`], like the pyramid's two pick rolls.
//!   (Level.cpp:340's "0x3D after load" is remc2's own debug
//!   reseed `//fix`, not retail law.)
//! - m27 `sub_2A940`'s `x_DWORD_E9BA8` freeze gate (writer
//!   untraced, likely pause/debug) reads as 0 — the normal path.
//! - The m27 emerge/teleport probe folds `sub_102D0(_, _, 4)` (the
//!   second capability mask) into the shared `mc2_path_blocked`
//!   (a3=1 arm) + roughness test, like the shared move core does.
//! - m22's castle-drain (0xB2/0xB3) reaches through the target
//!   player's `CastleEntityIndex_0x3A_58`; no MC2 level spawns a
//!   castle today, so the lookup returns None and the machine takes
//!   retail's own castle-less arm (LABEL_17 revert). The seam
//!   closes when MC2 castles land.

use super::behavior::BEHAVIOR;
use super::sprite_params::SPRITE_PARAMS;
use crate::engine::features::Gen;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// `MGC_NO_MC2_M27_HIDE_BIT=1` restores the port's invented `0x20` in
/// the m27 hydra's four burrow show/hide ops — i.e. it reverts this law.
///
/// ⭐ THE RIGHT SYMPTOM ON THE WRONG BIT, a second time (cf.
/// [`crate::mc2::mobs::no_mc2_class14_marker_hide_bit`], which found
/// the same invention on the (14,3)/(14,4) ending markers). This
/// module's own header said so out loud — *"writes flags bit 0
/// VERBATIM … PLUS the port's 0x20 draw alias"* — and the alias is
/// surplus: the port's MC2 billboard pass ALREADY hides a class-5
/// record on bit 0 (`live_poses_mc2`'s
/// `let hidden = (e.class64 == 5 && e.flags & 1 != 0) || enemy_mine`),
/// which is exactly retail's `byte[0] & 0x21` pair. Dropping 0x20
/// changes nothing on screen and stops the port stamping the
/// INVISIBILITY bit on fifty-odd hydra records at once.
///
/// `sub_29A90` (banner `//----- (00029A90)`) — all four ops, shipped
/// `NETHERW.EXE`, file = VA + 0x24800:
/// ```text
///   ; burrow hide, one node per gauge step (decompile :20093)
///   4e805  8a 68 0c     mov ch,[eax+0xc]
///   4e808  80 cd 01     or  ch,0x1
///   4e80b  88 ea        mov dl,ch
///   4e80d  80 e2 f7     and dl,0xf7
///   4e810  88 50 0c     mov [eax+0xc],dl      ; (byte[0]|1) & 0xF7
///   ; case 0xF, hide the branch and all nine segments (:20200)
///   4e8bc  8a 4a 0c     mov cl,[edx+0xc]
///   4e8bf  80 c9 01     or  cl,0x1
///   4e8c2  88 cd        mov ch,cl
///   4e8c4  80 e5 f7     and ch,0xf7
///   4e8c7  88 6a 0c     mov [edx+0xc],ch      ; (byte[0]|1) & 0xF7
///   ; case 0xA re-show + re-targetable
///   4e929  80 e1 f6     and cl,0xf6
///   4e93c  80 cd 08     or  ch,0x8
///   4e93f  88 6b 0c     mov [ebx+0xc],ch      ; (byte[0] & 0xF6) | 8
///   ; case 0xC sequential segment re-show (:20168)
///   4e99a  80 66 0c fe  and byte [esi+0xc],0xfe
/// ```
/// Bits 0 and 3 only, in every one of the four. The only three retail
/// setters of `0x20` are `sub_6B1C0`, `sub_5E310` and
/// `DisableEntitesDrawing_5E660` — all wizard-side.
///
/// WITNESS — free run, `MGC_RAW_SHADOW=1`: `(5,27) flags.b0_x20`
/// 16,210 rows over 6 takes, every row `retail 0 port 1`; mc2l24 7,304
/// rows t=13010..43341 over 118 slots, mc2l21 3,985 over 114,
/// mc2l22-new 1,725, mc2l19 1,319, mc2l19-taketwo 1,055, mc2l22 822.
pub(crate) fn no_mc2_m27_hide_bit() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M27_HIDE_BIT").is_some())
}

/// The bit(s) the m27 burrow ops touch on `byte[0]`: retail's bit 0
/// alone, or bit 0 + the port's legacy `0x20` draw alias under
/// [`no_mc2_m27_hide_bit`].
pub(crate) fn m27_hide_mask() -> u32 {
    if no_mc2_m27_hide_bit() { 0x21 } else { 0x01 }
}

pub(crate) const M0_BASE: u8 = 0;
pub(crate) const M3_BASE: u8 = 24;
pub(crate) const M22_BASE: u8 = 176;
pub(crate) const M27_BASE: u8 = 216;
/// m0/m3 child follow state (`sub_1B6B0`).
pub(crate) const CHILD_STATE: u8 = 232; // 0xE8
/// m27 branch / tier-2 segment (body-driven, no self-dispatch).
pub(crate) const BRANCH_STATE: u8 = 233; // 0xE9
pub(crate) const TIER2_STATE: u8 = 234; // 0xEA

/// `sub_29A90`'s `v34` on entry (retail `[ebp-0x10]`). See
/// `m27_v34_carry_law`. Only `!= 0`, `> 4` and `& 1` are ever tested,
/// so ANY even value above 4 is behaviourally identical; this one keeps
/// remc2's magnitude and fixes only its parity.
///
/// ⚠⚠⚠ THIS IS AN APPROXIMATION OF AN UNINITIALISED READ, AND ITS LOW
/// BIT IS NOT MODELLABLE — DO NOT RE-SWEEP IT (dig 99-17). The
/// prologue at NETHERW.EXE 0x4E290 is `53 56 57 55 89 e5 83 ec 10`
/// (`sub esp,0x10`) and the ONLY store into the 16-byte local block
/// before the chain loop is `30 d2 / 88 55 fc` (`xor dl,dl; mov
/// [ebp-0x4],dl` = `v37 = 0`, 0x4E2CA), so `[ebp-0x10]` genuinely
/// enters as stack residue; its one writer is 0x4E351 and its readers
/// 0x4E401/0x4E452. mc2l22 witnesses both parities on the SAME slots
/// within four ticks — retail SKIPS the wander draw (odd) at t=53,912/
/// 53,922/53,944/53,954/53,976/54,038/54,910/54,912/54,914/54,946/
/// 54,974/54,978/55,006/55,010/55,038/55,042/55,070/55,074/55,089 and
/// TAKES it (even) at t=53,168/53,848/53,857/53,858/53,880/53,889/
/// 54,882/54,942, each confirmed as an exact ±1 step of the
/// 9377/9439 LCG on the branch's own `rand`. The `!= 0` and `> 4`
/// bits ARE stable (retail's `byte_0x46_70` is 4 at every one of
/// those sites, both parities), which is why the magnitude here is
/// right and only the parity is a coin toss. Even loses ~18 mc2l22
/// segment heads, odd ~20; that ~18-head floor is a REGISTERED
/// DEVIATION (see `conformance/known-deviations.json`
/// `mc2l24-hydra-residual`), not a dig.
const V34_ENTRY: u32 = 0x1000_002A;

/// Which arm of the (5,15) guard's action-121 brain `sub_23C40` ran,
/// for [`Gen::m27_v34_publish_guard`].
#[derive(Clone, Copy)]
pub(crate) enum GuardV34 {
    /// The lethal arm (no call at all).
    Silent,
    /// The non-lethal-hit arm (`sub_1EEE0`).
    Hit,
    /// The clean arm: `sub_24190`'s packmate byte (`None` = its
    /// action-124 bail) and no acquire candidate tested.
    Wander(Option<bool>),
    /// The clean arm, and the acquire walk tested a candidate.
    Scanned,
}

/// `str_D404C[5]` — the m27 per-branch spline parameters
/// (engine/Type_D404C.cpp, a static array compiled into the binary;
/// only the sim-read fields are carried — w8/w16/w18/w20 are
/// renderer-only). Order: w0 anchor reach, w2 anchor yaw, w4 anchor
/// z, w6 trailing reach, w10 trailing z, w12 splay yaw, w14 splay
/// pitch.
const D404C: [[i16; 7]; 5] = [
    [390, 20, 610, 30, -80, 9, 1771],
    [440, 110, 600, 0, -100, 407, 1685],
    [430, -100, 600, 0, -100, 1641, 1707],
    [420, 50, 450, 0, -70, 284, 1905],
    [420, -10, 450, 40, -70, 770, 1157],
];
const D404C_W0: usize = 0;
const D404C_W2: usize = 1;
const D404C_W4: usize = 2;
const D404C_W6: usize = 3;
const D404C_W10: usize = 4;
const D404C_W12: usize = 5;
const D404C_W14: usize = 6;

/// `xx_DWORD_D40BC[17][3]` (EF:1092) — the m27 spline arc profile;
/// only columns 0/1 are read (outer/inner pitch-bend magnitudes).
const D40BC: [[i16; 2]; 17] = [
    [0, 0],
    [106, 36],
    [151, 51],
    [191, 65],
    [220, 75],
    [246, 84],
    [275, 94],
    [297, 102],
    [318, 109],
    [338, 116],
    [361, 124],
    [380, 130],
    [398, 136],
    [416, 143],
    [437, 150],
    [454, 156],
    [0, 0],
];

/// `x_BYTE_D400C[8][8]` (EF:1080) — the m22 colorize ramp, indexed
/// `[tailLen>>1][|ring offset|]`. Row 5 is non-monotone in retail
/// (…4,3,3,1…) — reproduced verbatim.
const D400C: [[u8; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 0, 0, 0, 0, 0, 0],
    [3, 2, 1, 0, 0, 0, 0, 0],
    [4, 3, 2, 1, 0, 0, 0, 0],
    [5, 4, 3, 3, 1, 0, 0, 0],
    [6, 5, 4, 3, 2, 1, 0, 0],
    [7, 6, 5, 4, 3, 2, 1, 0],
];

impl Gen {
    // ---- shared ------------------------------------------------------------

    /// `sub_58210_radix_tan` — the vertical bearing FROM a TO b
    /// (the MC1 segment-follow idiom, mc1/mobs.rs :21107 port).
    pub(crate) fn mc2_radix_tan(a: (u16, u16, i16), b: (u16, u16, i16)) -> u16 {
        let dh = Self::isqrt(Self::dist2_sq(a.0, a.1, b.0, b.1) as u32) as i16;
        Self::angle_of(a.2.wrapping_sub(b.2), dh.wrapping_neg())
    }

    /// A position read that tolerates corpses (retail dereferences
    /// the slot regardless); the human resolves through the ctx.
    fn mc2_raw_pos(&self, slot: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if slot == PLAYER_TARGET {
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = slot as usize;
        if j == 0 || j >= self.ent.len() {
            return None;
        }
        let e = &self.ent[j];
        Some((e.x, e.y, e.z))
    }

    /// `GetManaSphereColorIndexFromEntityId_369F0` (EF:26782): the
    /// owner's mana-sphere particle-row base — 52 wild, and
    /// `105 + 8·TransformPlayerColorIndex(team)` for ANY wizard
    /// (EF:26800; the human = team 0, rivals by slot; the sphere art
    /// families are authored in Transform order,
    /// crate::mc2::COLOR_ART).
    fn mc2_ball_color(&self, target: u16) -> u16 {
        if target == PLAYER_TARGET {
            return 105;
        }
        match self.rival_ents.iter().position(|&e| e != 0 && e == target) {
            Some(slot) => 105 + 8 * crate::mc2::color_art(slot as u8) as u16,
            None => 52,
        }
    }

    /// `sub_49D50` (EF:32847, shipped `NETHERW.EXE` file 0x6E550):
    /// the "color index" is a `particlesParameters_D951C` row —
    /// sprite row (@0x5A), the FRAME COUNT (@0x5D) and the yaw spin
    /// (@0x52). ⚠ Unlike `SetEntityIndex_49C90` it does NOT clear
    /// `animationFrame_0x5C_92` (verified in the disassembly: it
    /// writes 0x5a, 0x5d and 0x52 and nothing else).
    ///
    /// ⚠⚠ THE OLD COMMENT HERE ("the palette-shade byte_0x5D_93 is
    /// renderer-side, unmodeled") WAS WRONG on both counts —
    /// `byte_0x5D_93` is the sprite's ANIMATION FRAME COUNT and
    /// `sub_585A0` reads it in the SIM ([`Gen::mc2_anim_step`]).
    fn mc2_particle_row(&mut self, i: usize, row: u16) {
        let r = row as usize % SPRITE_PARAMS.len();
        let frames = crate::mc2::mobs::mc2_sprite_frames(r);
        let e = &mut self.ent[i];
        e.type86 = r as u16;
        if !crate::mc2::mobs::no_mc2_frames89() {
            e.frames89 = frames;
        }
        e.f78 = SPRITE_PARAMS[r].rot_speed_8 / 2;
    }

    // =========================================================================
    // MODELS 0 + 3 — worm/hydra + multipart flyer
    // (ctors sub_4B240 EF:33642 / sub_4B6F0 EF:33797; child tick
    // sub_1B6B0 EF:8696; head states = thin primitive wrappers,
    // docs/traces/mc2-m0-m3-gaps.md §5)
    // =========================================================================

    /// `sub_4B240` — model 0. ONE ctor RNG draw (facing); the child
    /// loop draws nothing (children byte-copy the head, inheriting
    /// its LCG). Bug-compatible: the per-child mana>>5 write lands
    /// on the HEAD (EF:33703) — children keep the copied 2250 each.
    pub(crate) fn mc2_spawn_m0(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.free.len() < 16 {
            return None; // sub_4A810 free-slot gate (EF:33655)
        }
        let head = self.new_event()?;
        {
            let e = &mut self.ent[head];
            e.class64 = 5;
            e.model65 = 0;
            e.tick70 = M0_BASE + 1;
            e.f28 = 1; // byte_0x38_56 = 1 — cross-column damage contract
            e.f128 = 80;
            e.f130 = 16;
            e.f126 = 30;
            e.max_life = 4000;
            e.f136 = 4500;
            e.f140 = 2250; // mana = 4500, maxMana = mana, mana /= 2
        }
        self.mc2_ctor_facing(head);
        let ord = self.mc2_ord(0);
        {
            let e = &mut self.ent[head];
            e.f36 = 0;
            e.f56 = 96; // word_0x36_54 — the link length
            e.f26 = (head % 100) as i16; // dword_0x10_16 — the bob seed
            e.f63 = ord;
            e.f66 = 3; // xtype
            e.f44 = 0;
            e.f71 = 0;
            e.row156 = 71;
        }
        self.ent[head].f58 = Self::mc2_wake_stagger(71, ord);
        self.mc2_spawn_chain_children(head, x, y, z, false);
        self.link(head, x, y, z);
        self.refill_life(head);
        self.mc2_set_sprite(head, 40);
        Some(head)
    }

    /// `sub_4B6F0` — model 3. ONE ctor RNG draw. NO free-slot gate
    /// (relies on NewEvent null-checks). Children carry their own
    /// mana (maxMana/32) and particle-driven link metrics.
    pub(crate) fn mc2_spawn_m3(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let head = self.new_event()?;
        {
            let e = &mut self.ent[head];
            e.class64 = 5;
            e.model65 = 3;
            e.tick70 = M3_BASE + 1;
            e.f28 = 1;
            e.f128 = 64;
            e.f130 = 16;
            e.f126 = 30;
            e.max_life = 9000;
        }
        self.mc2_set_mana_half(head); // SetEvent144: 4500
        {
            let e = &mut self.ent[head];
            e.f136 = e.f140; // maxMana = mana
            e.f140 /= 2; // 2250
        }
        self.mc2_ctor_facing(head);
        let ord = self.mc2_ord(3);
        {
            let e = &mut self.ent[head];
            e.f36 = 0;
            e.f56 = 96;
            e.f26 = (head % 100) as i16;
            e.f63 = ord;
            e.f66 = 3;
            e.row156 = 74;
        }
        self.ent[head].f58 = Self::mc2_wake_stagger(74, ord);
        self.mc2_spawn_chain_children(head, x, y, z, true);
        self.link(head, x, y, z);
        self.refill_life(head);
        self.mc2_set_sprite(head, 88);
        // Head segment metrics from the particle table (EF:33869-71) —
        // the SAME `particlesParameters_D951C` rows the child loop
        // reads two lines above, so the DERIVED pair, not the shipped
        // static row. The static `speed_6` column is zero almost
        // everywhere (it is filled at load from the sprite bitmap's
        // aspect), and reading it here collapsed the head's pitch/roll
        // box to 0.
        let (ps6, pr8) = self.mc2_params_ext(88);
        let shift = (60 * ps6 / 100, 60 * pr8 / 100);
        self.mc2_shift_rot(head, shift.0, shift.1);
        Some(head)
    }

    /// The shared 16-child spawn loop (EF:33691-33712 m0 /
    /// EF:33836-33865 m3): byte-copy of the head, chain links,
    /// state 0xE8, per-model sprite rows and link lengths. The
    /// free-slot gate (m0) makes the NewEvent null arm unreachable;
    /// m3 without a gate stops early like retail's skip.
    fn mc2_spawn_chain_children(&mut self, head: usize, x: u16, y: u16, z: i16, m3: bool) {
        let mut prev = head;
        for ci in 0..16u16 {
            let Some(seg) = self.new_event() else { break };
            // qmemcpy(child, head, 0xA8) — the child KEEPS the
            // head's id_0x1A_26 (owner immunity spans the chain);
            // only the chain links and identity below are rewritten.
            self.ent[seg] = self.ent[head];
            self.ent[seg].flags &= !4; // not yet map-linked
            self.ent[seg].thing_slot = 0;
            self.ent[seg].f52 = prev as u16;
            self.ent[prev].f54 = seg as u16;
            self.ent[seg].f54 = 0;
            self.ent[seg].tick70 = CHILD_STATE;
            if m3 {
                self.ent[seg].f140 = self.ent[head].f136 / 32;
            } else {
                // The m0 decompile-literal quirk: the write lands on
                // the HEAD (EF:33703).
                self.ent[head].f140 = self.ent[head].f136 / 32;
            }
            self.ent[seg].f63 = ci as u8;
            if m3 {
                self.mc2_set_sprite(seg, 89 + ci);
                // Per-child particle metrics override the /2 quad
                // (EF:33846-51): 65% of the row's raw values (the
                // DERIVED pair — retail computes speed_6 from the
                // sprite bitmap at load, EF:44870-44910).
                let (ps6, pr8) = self.mc2_params_ext((89 + ci) as usize);
                let (sh, fov) = (65 * ps6 / 100, 65 * pr8 / 100);
                self.mc2_shift_rot(seg, sh, fov);
                self.ent[seg].f56 = if ci == 0 { 125 * sh / 100 } else { sh };
            } else {
                self.mc2_set_sprite(seg, 19 + ci);
                self.ent[seg].f56 = self.ent[seg].f80; // word_0x36_54 = array.pitch
            }
            // The table's zero speed_6 is never the runtime value:
            // retail DERIVES it at load from the sprite bitmap's
            // aspect (EF:44870-44910, speed_6 = w·rotSpeed/h), which
            // the dims-fed assets reproduce. The 96 floor stays only
            // for dims-less callers (unit fixtures) where the
            // derivation can't run.
            if self.ent[seg].f56 == 0 {
                self.ent[seg].f56 = 96;
            }
            self.link(seg, x, y, z);
            self.refill_life(seg);
            prev = seg;
        }
    }

    /// A/B toggle for the ORPHAN-REAP FALL-THROUGH law (dig 126-F):
    /// `MGC_NO_MC2_CHILD_ORPHAN_FALLTHROUGH=1` restores the pre-126
    /// body — the invented early `return` after the orphan reap.
    fn child_orphan_fallthrough_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_CHILD_ORPHAN_FALLTHROUGH").is_none())
    }

    /// `sub_1B6B0` (EF:8696) — the m0/m3 child tick (state 0xE8):
    /// awake = rigid follow at -f56 behind the parent along the
    /// exact 3D bearing + own damage intake; asleep = every 4th
    /// phase snap onto the parent.
    ///
    /// ⭐ THE ORPHAN REAP IS NOT AN EARLY RETURN. Retail marks the
    /// child and CARRIES ON with the follow, using the (already
    /// freed, class-0) parent record's STALE pose. Shipped
    /// `NETHERW.EXE` file **0x3FEB0** (LE VA 0x1B6B0):
    /// ```text
    /// 3FEBE  66 8b 73 32     mov  si,[ebx+0x32]      ; parent index
    /// 3FEC2  8b 34 b5 ..     mov  esi,[esi*4+0x1A3E4]; Entities[idx]
    /// 3FEC9  80 7e 3f 05     cmp  byte [esi+0x3f],5  ; class == 5 ?
    /// 3FECD  74 09           je   0x3FED8            ; -> the follow
    /// 3FECF  53              push ebx
    /// 3FED0  e8 3b c8 03 00  call 0x57F10            ; the reap
    /// 3FED5  83 c4 04        add  esp,4
    /// 3FED8  8a 53 39        mov  dl,[ebx+0x39]      ; byte_0x39_57
    /// 3FEDB  8d 7e 4c        lea  edi,[esi+0x4c]     ; parent pose
    /// ```
    /// The `je` skips ONLY the call; 0x3FED8 is the common tail, and
    /// `esi` (the class-0 parent) is what `lea edi,[esi+0x4c]` feeds
    /// the follow. `DisableEntityDrawing04_57F10` (file 0x7C710) is
    /// `mov eax,[ebp+8] / or byte [eax+0x0d],4 / ret` — `flags |=
    /// 0x400` and nothing else; it never returns for its caller.
    /// The index-0 arm is the same code: retail dereferences
    /// `Entities[0]` (the sentinel, class 0), reaps, and follows it.
    ///
    /// mc2l24 t=51332→51333 is the witness. Vissuluth's worm chain
    /// dies link by link — the reap mark walks one link per tick and
    /// the tick-top reaper frees the previous link. At 51333 head
    /// 732 is freed (class3f 5 → 0) and link 727 is reap-flagged; the
    /// port stopped there, but retail still runs 727's follow (x/y/z
    /// 9932,53440,2324 → 9933,53456,2308 — exactly `f36`=98 behind
    /// 732's stale 9941,53522,2235) AND drains its mailbox (life
    /// 2400 → 2000, f26 0 → 853, mail0.src 853 → 0). Skipping that
    /// froze one link per tick for the whole chain, which is the
    /// t=51333..51354 cluster: 22 one-tick segments.
    ///
    /// ⭐⭐⭐ A LAW ON ONE CALL PATH IS NOT LANDED — here in its
    /// CROSS-GAME form. MC1's identical body-segment follow
    /// ([`World::segment_follow`], `sub_19550` :21116-17) already
    /// carried this law, comment and all ("FALLS THROUGH — the
    /// orphan still follows the stale leader slot this tick");
    /// only the MC2 sibling grew the invented `return`. The
    /// `l == 0` term below is redundant (slot 0 is the scratch
    /// record, `class64 == 0 != 5`, so it reaps and follows the
    /// scratch pose exactly as MC1 documents) and is kept only so
    /// the kill switch reproduces the pre-126 body byte for byte.
    pub(crate) fn mc2_child_tick(&mut self, i: usize) {
        let l = self.ent[i].f52 as usize;
        if l == 0 || self.ent[l].class64 != 5 {
            self.ent[i].flags |= 0x400;
            if !Self::child_orphan_fallthrough_law() {
                return;
            }
        }
        let (lx, ly, lz) = (self.ent[l].x, self.ent[l].y, self.ent[l].z);
        if self.ent[i].f58 != 0 {
            let e = &self.ent[i];
            let yaw = Self::angle_between(e.x, e.y, lx, ly);
            let pitch = Self::mc2_radix_tan((e.x, e.y, e.z), (lx, ly, lz));
            self.ent[i].f30 = yaw;
            self.ent[i].f32 = pitch;
            let mut pred = (lx, ly, lz);
            let d = self.ent[i].f56 as i16;
            Self::polar_step(&mut pred, yaw, pitch, -d);
            self.move_relink(i, pred.0, pred.1, pred.2);
            // Damage intake AFTER the follow (EF:8710-19); the
            // attacker latch is word_0x26_38 (f40), else zero.
            if self.ent[i].mail[0].1 != 0 {
                let (amt, src) = self.ent[i].mail[0];
                self.ent[i].mail[0].1 = 0;
                self.ent[i].f40 = src;
                self.ent[i].act_life -= amt as i32;
            } else {
                self.ent[i].f40 = 0;
            }
        } else if self.ent[i].f63 & 3 == 0 {
            self.move_relink(i, lx, ly, lz);
            self.ent[i].f30 = self.ent[l].f30;
        }
    }

    /// A/B toggle for the m0 PHASE-7 WRAPPER TABLE law (dig 124-G):
    /// `MGC_NO_MC2_M0_PHASE7_CONTROLLED=1` restores the pre-124 body —
    /// the wrapper physics on the stage-HELD seam only, and only for
    /// StageVar2 1..=10.
    fn m0_phase7_table_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M0_PHASE7_CONTROLLED").is_none())
    }

    /// A/B toggle for the m0 DODGE HUMAN-HOOK law:
    /// `MGC_NO_MC2_M0_DODGE_HUMAN_HOOK=1` restores the pre-fix body —
    /// a `word_0x24_36` naming the out-of-pool human reads as "gone"
    /// and releases the hook instead of strafing off his yaw.
    fn m0_dodge_human_hook_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_M0_DODGE_HUMAN_HOOK").is_none())
    }

    /// `sub_1F300` (EF:11352) — the m0 STATE-0x07 WRAPPER's tail, i.e.
    /// everything after the shared `sub_1D5D0(a2x, 0)` leg. Shipped
    /// `NETHERW.EXE` file **0x43B00** (LE VA 0x1F300, dispatch-table
    /// entry 0x200300):
    /// ```text
    /// 1F30A  e8 c1 e2 ff ff        call 0x1D5D0          ; the legs
    /// 1F30F  8a 43 49              mov  al,[ebx+0x49]    ; StageVar2
    /// 1F312  fe c8                 dec  al
    /// 1F317  3c 10                 cmp  al,0x10
    /// 1F319  77 1f                 ja   0x1F33A          ; -> ret
    /// 1F320  2e ff 24 85 b8 f2 ..  jmp  [cs:eax*4+0xF2B8]
    /// 1F328  e8 92 fd ff ff        call 0x1F0C0          ; dodge
    /// 1F331  e8 09 fd ff ff        call 0x1F040          ; bob
    /// 1F33A  5d 5b c3              ret
    /// ```
    /// and the 17-entry table at file 0x43AB8 (LE 0x1F2B8) reads,
    /// for StageVar2 = 1..0x11:
    /// `328 328 328 328 328 328 328 328 328 328 | 33A 33A | 328 328 |
    ///  33A | 328 | 331`
    /// — so **1..0xA, 0xD, 0xE, 0x10 → dodge+bob; 0xB/0xC/0xF →
    /// nothing; 0x11 → BOB ONLY**.
    ///
    /// ⭐⭐⭐ A LAW ON ONE CALL PATH IS NOT LANDED. The port had this
    /// tail on the stage-HELD seam alone ([`World::mc2_held_tick`],
    /// kinds 1..=10) — but `sub_1F300` IS the model-0 `8m+7` handler,
    /// so the CONTROLLED kinds reach it through exactly the same
    /// wrapper: 0xD Summon-Army, 0xE alliance, 0x10 pyramid-home and
    /// 0x11 pyramid SPIN-UP all take the ambient physics too.
    /// mc2l24's Vissuluth worms are the witness: a (5,0) head at
    /// StageVar2 **17** rises on `sub_1E320`'s launch flight while
    /// `sub_1F040` adds `dword_0x10_16` to z every tick and drops the
    /// velocity by 5. t=50700 slot 226 — retail z 981 → 1097 and
    /// velocity 120 → 115, where the port committed only the move
    /// core's −4 (z 977, velocity untouched at 120). Its first child
    /// segment then reads the wrong parent z and mis-pitches, which
    /// is where the family SURFACES ([`Gen::mc2_child_tick`]).
    /// The selector is read AFTER the legs, exactly as retail does
    /// (`sub_1E320` flips 0x11 → 0x10 on its last tick, and the
    /// wrapper then runs the 0x10 arm — dodge included).
    pub(crate) fn m0_phase7_physics(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].model65 != 0 {
            return;
        }
        if !Self::m0_phase7_table_law() {
            // Pre-124 body: the held seam's 1..=10 arm only.
            if matches!(self.ent[i].site_z, 1..=10) {
                self.m0_dodge(i, ctx);
                self.m0_bob(i);
            }
            return;
        }
        match self.ent[i].site_z {
            1..=10 | 13 | 14 | 16 => {
                self.m0_dodge(i, ctx);
                self.m0_bob(i);
            }
            17 => self.m0_bob(i),
            _ => {}
        }
    }

    /// `sub_1F040` (EF:11233) — the m0 vertical bob: velocity in
    /// f26 (`dword_0x10_16`), gravity −5/tick, floor bounce +150 at
    /// terrain+256; on caves, ceiling BOUNCE −150 above ceiling−256
    /// (EF:11244-48) — open levels have no upper clamp, exactly
    /// retail. Also the stage-HELD dragon's ambient physics
    /// (`sub_1F300` phase-7 wrapper, kinds 1-10 — the stagevars
    /// held seam).
    pub(crate) fn m0_bob(&mut self, i: usize) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let z = self.ent[i].z.wrapping_add(self.ent[i].f26);
        self.move_relink(i, x, y, z);
        let ground = self.ground_z(x, y) as i16;
        self.ent[i].f26 -= 5;
        if z < ground.wrapping_add(256) {
            self.ent[i].f26 = 150;
        } else if self.is_cave() && z as i32 > self.ceiling_z(x, y) - 256 {
            self.ent[i].f26 = -150;
        }
    }

    /// `sub_1F0C0` (EF:11259) — the m0 incoming-projectile DODGE.
    /// Armed on the PROJECTILE side: the class-9 one-shot
    /// acquisition (`sub_67CB0`) calls `sub_68BD0` (EF:55453),
    /// which sets the head's alert window `fontTypeIndex_0x3D_61 =
    /// 32` whenever the lock lands on a class-5 model-0 victim
    /// (EF:54848 — the only live call site; the trace-bank
    /// "gate never armed → dormant" claim was wrong). While the
    /// window runs (decrementing every call, EF:11277-80): with no
    /// hook, spiral the radius-4 tile disc for a class-9 whose
    /// homing target is this head and hook it, timer 5
    /// (EF:11310-42); with a hook live, strafe the HEAD
    /// perpendicular to the projectile's CURRENT heading — side by
    /// hooked-index parity, step `48·timer` (240..48, ≈720 units
    /// total), pitch 0, tile-relinked (EF:11293-11300) — and
    /// release when the timer expires or the projectile dies
    /// (EF:11286-89/11304-06). If the window closes mid-dodge the
    /// hook freezes in place until a fresh arm — retail's own
    /// residue law.
    pub(crate) fn m0_dodge(&mut self, i: usize, ctx: &MobCtx) {
        let gate = self.ent[i].f46;
        if gate == 0 {
            return;
        }
        self.ent[i].f46 = gate - 1;
        if self.ent[i].f71 != 0 {
            if self.ent[i].f44 == 0 {
                self.ent[i].f71 = 0;
                self.ent[i].f38 = 0;
                return;
            }
            // Validity = retail's `v7x <= Entities[0] ||
            // v7x->life_0x8 < 0 || v7x->byte[1] & 4` gone-check
            // (EF:11286-89) — all three terms are retail's own.
            //
            // ⭐⭐⭐ THE HOOK WORD IS THE KILL-CREDIT LATCH, AND ON A
            // DEATH TICK IT NAMES THE KILLER. `word_0x24_36` doubles
            // as the killer latch (`mc2_state_head` death write
            // `a1x->word_0x24_36 = a1x->word_0x26_38`, EF:9285), so a
            // head that dies with its alert window still running
            // reaches this branch with the ATTACKER in the hook word.
            // Retail's human carpet is an ORDINARY POOL RECORD, so
            // that resolves LIVE and the head strafes off the HUMAN's
            // yaw — shipped `NETHERW.EXE` 0x1F1F7 `mov ax,[ebx+0x24]`
            // then a PLAIN `mov esi,[eax*4+0x1a3e4]` with no bounds
            // test. Ours is the out-of-pool PLAYER_TARGET sentinel;
            // reading it as "gone" froze the whole follow chain for a
            // tick (mc2l30-new t=2368 slot 100: retail strafes
            // x 51584 -> 51567, y 9877 -> 10021 at human-yaw
            // 549 + 512 = 1061, step 48*3 = 144; the port released the
            // hook and never moved, and children 101/102 then
            // mis-followed — which is the whole "middle segment's
            // heading is wildly wrong" signature).
            let p = self.ent[i].f38;
            let hook_yaw = if p == PLAYER_TARGET && Self::m0_dodge_human_hook_law() {
                // `ctx.pdead` IS retail's `life_0x8 < 0` on the carpet
                // record; there is no reap bit for the out-of-pool
                // human.
                if ctx.pdead { None } else { Some(ctx.pyaw) }
            } else {
                let j = p as usize;
                if j == 0
                    || j >= self.ent.len()
                    || self.ent[j].act_life < 0
                    || self.ent[j].flags & 0x400 != 0
                {
                    None
                } else {
                    Some(self.ent[j].f30)
                }
            };
            let Some(hook_yaw) = hook_yaw else {
                self.ent[i].f71 = 0;
                self.ent[i].f38 = 0;
                return;
            };
            // The strafe SIDE is the parity of the RAW latch (0x1F21C
            // tests `[ebx+0x24]`, not the resolved record) — for the
            // human that is his real pool index in retail, which the
            // import pins; native play has no such index and keeps the
            // sentinel (0xFFFF, odd).
            let side = if p == PLAYER_TARGET && self.mc2_pinned.0 != 0 {
                self.mc2_pinned.0
            } else {
                p
            };
            let yaw = if side & 1 != 0 {
                hook_yaw.wrapping_add(512)
            } else {
                hook_yaw.wrapping_sub(512)
            } & 0x7FF;
            let mut pos = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            Self::polar_step(&mut pos, yaw, 0, (48 * self.ent[i].f44) as i16);
            self.move_relink(i, pos.0, pos.1, pos.2);
            self.ent[i].f44 -= 1;
        } else {
            let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
            let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
            let my_id = self.ent[i].id24;
            let mut hooked = 0usize;
            'scan: for (dx, dy) in self.ring_cells(0, 4) {
                let t = crate::engine::features::tile(cx.wrapping_add(dx), cy.wrapping_add(dy));
                let mut j = self.map_entity[t] as usize;
                while j != 0 {
                    if self.ent[j].class64 == 9 && self.ent[j].f146 == my_id {
                        hooked = j;
                        break 'scan;
                    }
                    j = self.ent[j].next20 as usize;
                }
            }
            if hooked != 0 {
                self.ent[i].f44 = 5;
                self.ent[i].f71 = self.ent[i].f71.wrapping_add(1);
                self.ent[i].f38 = hooked as u16;
            }
        }
    }

    /// m0 states 0x00-0x07 (docs/traces/mc2-m0-m3-gaps.md §5):
    /// primitive → dodge (`sub_1F0C0`) → bob (`sub_1F040`) in
    /// 0x01/0x02/0x03, per sub_1EF40/1EF70/1EFD0.
    pub(crate) fn m0_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M0_BASE {
            0 => self.mc2_patrol(i, M0_BASE),
            1 => {
                self.mc2_idle(i, M0_BASE, ctx);
                self.m0_dodge(i, ctx);
                self.m0_bob(i);
            }
            2 => {
                if self.mc2_chase_attack(i, M0_BASE, ctx, Self::mc2_atk_bolt) {
                    self.snd(8, i);
                }
                self.m0_dodge(i, ctx);
                self.m0_bob(i);
            }
            3 => {
                self.mc2_pack(i, M0_BASE);
                self.m0_dodge(i, ctx);
                self.m0_bob(i);
            }
            4 => self.mc2_prekill(i, M0_BASE),
            5 => self.mc2_kill(i),
            // 0x06 = sub_1F2B0: a compiled EMPTY STUB in the binary
            // (module doc) — the no-op is faithful.
            6 => {}
            _ => {
                // 0x07 sub_1F300: 1D5D0 no-op for StageVar2==0, and
                // 0 is outside the tether/bob case list → nothing.
            }
        }
    }

    /// m3 states 0x18-0x1F — pure primitive wrappers, aggro base 24
    /// (no tether/bob among the recovered states; trace §5).
    pub(crate) fn m3_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M3_BASE {
            0 => self.mc2_patrol(i, M3_BASE),
            1 => self.mc2_idle(i, M3_BASE, ctx),
            2 => {
                if self.mc2_chase_attack(i, M3_BASE, ctx, Self::mc2_atk_bolt) {
                    self.snd(8, i);
                }
            }
            3 => self.mc2_pack(i, M3_BASE),
            4 => self.mc2_prekill(i, M3_BASE),
            5 => self.mc2_kill(i),
            // 0x1E = sub_1FA40: a compiled EMPTY STUB in the binary
            // (module doc) — the no-op is faithful.
            6 => {}
            _ => {} // 0x1F sub_1FA50: 1D5D0 no-op for StageVar2==0
        }
    }

    // =========================================================================
    // MODEL 22 — segmented worm / castle-mana thief
    // (ctor sub_4CA00 EF:34377 + map tail sub_4CB60 EF:34420;
    // helpers docs/traces/mc2-m22-worm-helpers.md)
    // =========================================================================

    /// `sub_4CA00` + the map-placement arm (`sub_4A310` EF:33025-28:
    /// tail length = par1 & 0xFF, then `sub_4CB60` spawns the tail).
    /// ONE ctor RNG draw. Head z = terrain + 384.
    pub(crate) fn mc2_spawn_m22(&mut self, x: u16, y: u16, _z: i16, par1: u16) -> Option<usize> {
        if self.free.len() < 15 {
            return None; // free-slot gate (EF:34380)
        }
        let head = self.new_event()?;
        {
            let e = &mut self.ent[head];
            e.class64 = 5;
            e.model65 = 22;
            e.tick70 = M22_BASE; // 176
            // byte_0x38_56 = 3 (EF:34400) — bit 1 ADMITS the ch1
            // designation mail (the mc1/combat.rs:180 gate is
            // faithful). Needs f28=3 not 1: f28=1 drops every tag,
            // deadening the whole retarget→colorize machine.
            e.f28 = 3;
            e.f128 = 128;
            e.f130 = 16;
            e.f126 = 16;
        }
        self.mc2_ctor_facing(head);
        let ord = self.mc2_ord(22);
        {
            let e = &mut self.ent[head];
            e.max_life = 2000;
            e.f36 = 0;
            e.row156 = 90;
            e.f63 = ord;
            e.f66 = 3;
            e.f144 = 0; // playerEntityIndex_0x94_148
            e.f78 = 0; // array.yaw
            e.frame88 = 0;
            e.f44 = 11; // word_0x2C_44 — spin rate
            e.f46 = 0; // subSpellIndex — spiral angle
            e.f56 = 0; // word_0x36_54 — writhe phase bits
            e.f146 = 1024; // word_0x96_150 — the grow timer
            e.f38 = 0; // word_0x24_36 — rise budget
            e.f71 = 15; // byte_0x46_70 — default tail length
        }
        self.ent[head].f58 = Self::mc2_wake_stagger(90, ord);
        let z = (self.ground_z(x, y) as i16).wrapping_add(384);
        self.link(head, x, y, z);
        self.mc2_set_mana_half(head); // SetEvent144: 1000
        self.refill_life(head);
        // Map placement overrides the tail length then grows it.
        self.ent[head].f71 = (par1 & 0xFF) as u8;
        self.mc2_m22_spawn_tail(head);
        Some(head)
    }

    /// `sub_4CB60` (EF:34420): (tailLen/2) rings x 2 segments with
    /// signed offsets +1,-1,+2,-2,…; then colorize + shift-rot +
    /// one follow pass to seat everyone.
    fn mc2_m22_spawn_tail(&mut self, head: usize) {
        let rings = (self.ent[head].f71 / 2) as i16;
        let mut prev = head;
        for ring in 1..=rings {
            for side in 0..2 {
                let off = if side == 1 { -ring } else { ring };
                if let Some(seg) = self.mc2_m22_add_segment(head, prev, off as i8) {
                    prev = seg;
                }
            }
        }
        self.mc2_m22_colorize(head);
        self.mc2_m22_shift_rot(head);
        // sub_276E0: one spiral-follow pass over the chain.
        let mut j = self.ent[head].f54 as usize;
        while j != 0 {
            self.m22_tail_follow(j);
            j = self.ent[j].f54 as usize;
        }
    }

    /// `sub_274C0` (EF:17845) — the m22 segment-spawn primitive:
    /// full struct copy from the PREVIOUS link, chain re-link,
    /// state 0xB4, signed ring offset in f71, head ref in f146.
    fn mc2_m22_add_segment(&mut self, head: usize, prev: usize, off: i8) -> Option<usize> {
        let seg = self.new_event()?;
        self.ent[seg] = self.ent[prev];
        self.ent[seg].thing_slot = 0;
        self.ent[seg].f52 = prev as u16;
        self.ent[prev].f54 = seg as u16;
        self.ent[seg].f54 = 0;
        self.ent[seg].f63 = (off.unsigned_abs()) & 1; // parity seed
        self.ent[seg].flags &= !4;
        self.ent[seg].f71 = off as u8; // SIGNED ring offset
        self.ent[seg].tick70 = M22_BASE + 4; // 0xB4
        self.ent[seg].f44 = 0;
        self.ent[seg].f144 = 0;
        self.ent[seg].f140 = 0;
        self.ent[seg].f146 = head as u16; // word_0x96_150 = the worm head
        let (hx, hy, hz) = {
            let h = &self.ent[head];
            (h.x, h.y, h.z)
        };
        self.link(seg, hx, hy, hz);
        self.refill_life(seg);
        Some(seg)
    }

    /// `sub_278F0` (EF:18036): particle row = base + the D400C ramp.
    fn m22_color_idx(base: u16, tail_len: u8, off: i8) -> u16 {
        let row = (tail_len >> 1).min(7) as usize;
        let col = (off.unsigned_abs()).min(7) as usize;
        base + D400C[row][col] as u16
    }

    /// `sub_27590` (EF:17867): recolor head + chain to the owner's
    /// mana-sphere palette.
    fn mc2_m22_colorize(&mut self, head: usize) {
        let base = self.mc2_ball_color(self.ent[head].f144);
        let len = self.ent[head].f71;
        let hr = Self::m22_color_idx(base, len, 0);
        self.mc2_particle_row(head, hr);
        let mut j = self.ent[head].f54 as usize;
        while j != 0 {
            let r = Self::m22_color_idx(base, len, self.ent[j].f71 as i8);
            self.mc2_particle_row(j, r);
            j = self.ent[j].f54 as usize;
        }
    }

    /// `sub_27610` (EF:17893): per-link spacing/coil radius =
    /// 550 * the colorize row's rotSpeed / 1000.
    fn mc2_m22_shift_rot(&mut self, head: usize) {
        let base = self.mc2_ball_color(self.ent[head].f144);
        let len = self.ent[head].f71;
        let hrow = Self::m22_color_idx(base, len, 0) as usize % SPRITE_PARAMS.len();
        let v = 550 * SPRITE_PARAMS[hrow].rot_speed_8 as u32;
        self.mc2_shift_rot(head, (v / 1000) as u16, (v / 1000) as u16);
        let mut j = self.ent[head].f54 as usize;
        while j != 0 {
            let row = Self::m22_color_idx(base, len, self.ent[j].f71 as i8) as usize
                % SPRITE_PARAMS.len();
            let v = 550 * SPRITE_PARAMS[row].rot_speed_8 as u32;
            self.mc2_shift_rot(j, (v / 1000) as u16, (v / 1000) as u16);
            j = self.ent[j].f54 as usize;
        }
    }

    /// `sub_273C0` (EF:17780): the spiral angle a segment orbits the
    /// head at — magnitude grows with |offset| and the writhe frame,
    /// side/chirality from the offset sign and phase bit 1.
    fn m22_spiral(frame: i16, phase: u8, off: i16, tail_len: i16) -> u16 {
        let v4 = off.unsigned_abs() as i32;
        let result = (((15 - tail_len) as i32 * v4 + v4 * frame as i32) & 0x7FF) as u16;
        let v6 = if off >= 0 {
            if phase & 2 != 0 {
                return result;
            }
            2048u16.wrapping_sub(result)
        } else if phase & 2 != 0 {
            1024u16.wrapping_sub(result)
        } else {
            result.wrapping_add(1024)
        };
        v6 & 0x7FF
    }

    /// `sub_271D0` (EF:17685): the m22 spiral follow — positioned
    /// two links up, at the computed orbit angle, with pitch-based
    /// z offset.
    fn m22_tail_follow(&mut self, i: usize) {
        let head = self.ent[i].f146 as usize;
        if head == 0 || head >= self.ent.len() {
            return;
        }
        let v4 = {
            let h = &self.ent[head];
            let spiral = Self::m22_spiral(
                h.frame88 as i16,
                h.f56 as u8,
                self.ent[i].f71 as i8 as i16,
                h.f71 as i16,
            );
            (h.f46 as u16).wrapping_add(spiral) & 0x7FF
        };
        self.ent[i].f44 = v4 as i16 as u16;
        let mut anchor = self.ent[i].f52 as usize;
        if anchor != 0 && self.ent[anchor].f52 != 0 {
            anchor = self.ent[anchor].f52 as usize;
        }
        if anchor == 0 {
            return;
        }
        let (ax, ay, az, ap) = {
            let a = &self.ent[anchor];
            (a.x, a.y, a.z, a.f80 as i16)
        };
        let sp = self.ent[i].f80 as i16;
        let mut pred = (ax, ay, az);
        Self::polar_step(&mut pred, v4, 0, sp.wrapping_add(ap));
        pred.2 = ap.wrapping_sub(sp).wrapping_add(az);
        self.move_relink(i, pred.0, pred.1, pred.2);
    }

    /// `sub_26D20` (EF:17447): segment→head hit/aggro relay. Damage
    /// amounts are consumed for STEER only (the m22 head is
    /// damage-immune through its own suite — trace §12, retail
    /// check banked); the ch1 tag retargets the head.
    ///
    /// Returns the W-52 stack residue of its LAST call (`None` = no
    /// call; `Some(None)` = a call whose residue is not modelled) —
    /// see `no_mc2_m22_stale_probe`.
    fn m22_relay(&mut self, i: usize, ctx: &MobCtx) -> Option<Option<u32>> {
        if self.ent[i].f58 == 0 {
            return None;
        }
        let head = self.ent[i].f146 as usize;
        if head == 0 || head >= self.ent.len() {
            return None;
        }
        let ha = self.ent[head].tick70;
        if !(ha == M22_BASE || ha == M22_BASE + 2) {
            return None; // relay only acts in 0xB0 / 0xB2 (EF:17460)
        }
        let mut residue = None;
        if self.ent[i].mail[0].1 != 0 {
            // `26d97 e8 → sub_581E0`, two args: its return address.
            residue = Some(Some(0x0020_7D9C));
            let src = self.ent[i].mail[0].1;
            let (mn, mx) = (self.ent[head].f128, self.ent[head].f130);
            self.ent[head].f126 = ((mn - mx) >> 2) + mx;
            if let Some((ax, ay, _)) = self.mc2_raw_pos(src, ctx) {
                // Surge AWAY: yaw = tan2(attacker → hit SEGMENT)
                // (EF:17472-74) — anchored at the SEGMENT, not the
                // head, and away from the attacker.
                let (sx, sy) = (self.ent[i].x, self.ent[i].y);
                let yaw = Self::angle_between(ax, ay, sx, sy);
                self.ent[head].f30 = yaw;
                self.ent[head].f34 = yaw;
            }
            // Spin law (EF:17475-94): ADDITIVE and orbit-signed —
            // v4 = 56·|seg pos|/(len/2) unclamped, negated when the
            // segment sits on the far half of the ring (head yaw vs
            // the segment's orbit angle f44), ADDED to the head's
            // spin, and the SUM clamps min ±11 / max ±227.
            let so = (self.ent[i].f71 as i8).unsigned_abs() as i32;
            let half = (self.ent[head].f71 >> 1).max(1) as i32;
            let mut v4 = (56 * so / half) as i16;
            let orbit = self.ent[i].f44;
            if self.ent[head].f30.wrapping_sub(orbit) & 0x7FF >= 1024 {
                v4 = -v4;
            }
            let mut v5 = v4 + self.ent[head].f44 as i16;
            if v5.abs() < 11 {
                v5 = if v5 <= 0 { -11 } else { 11 };
            }
            if v5.abs() > 227 {
                v5 = if v5 <= 0 { -227 } else { 227 };
            }
            self.ent[head].f44 = v5 as u16;
            // Clear the hit source on EVERY segment (EF:17520).
            let mut j = self.ent[head].f54 as usize;
            while j != 0 {
                self.ent[j].mail[0].1 = 0;
                j = self.ent[j].f54 as usize;
            }
        }
        let tag = self.ent[i].mail[1].1;
        if tag != 0 {
            if tag != self.ent[head].f144 {
                // `26e9c 98 / 26e9d 50`: the sign-extended tag is the
                // sound call's third push. Retail skips the call when
                // `dword_0x64 == 0` and the seg's byte-0xE bit 0x20 is
                // already set (26e73-26e7d).
                if self.ent[i].mail[1].0 != 0 || self.ent[i].flags & 0x20_0000 == 0 {
                    residue = Some(if tag == PLAYER_TARGET {
                        None
                    } else {
                        Some(tag as i16 as i32 as u32)
                    });
                }
                self.ent[head].f144 = tag;
                self.ent[head].tick70 = M22_BASE + 1; // 177
                self.ent[head].f26 = ((self.ent[i].f71 as i8 as i16) << 8) as i16;
                if tag == PLAYER_TARGET {
                    self.snd_player(4);
                } else {
                    self.snd(4, tag as usize);
                }
            }
            let mut j = self.ent[head].f54 as usize;
            while j != 0 {
                self.ent[j].mail[1].1 = 0;
                j = self.ent[j].f54 as usize;
            }
        }
        residue
    }

    /// `sub_26CC0` (EF:17427): the chain-kill — one pass, every
    /// downstream segment then the head converts to mana spheres.
    fn m22_chain_kill(&mut self, i: usize) {
        let mut j = self.ent[i].f54 as usize;
        while j != 0 {
            let next = self.ent[j].f54 as usize;
            self.mc2_mana_spheres(j, false);
            self.ent[j].flags |= 0x400;
            j = next;
        }
        self.mc2_mana_spheres(i, false);
        self.ent[i].flags |= 0x400;
    }

    /// A/B toggle for the m22 CASTLE-ACQUIRE TARGET GATES (dig
    /// 98-Q27): set `MGC_NO_M22_TARGET_GATES` to restore the
    /// pre-2026-09-04 body, which resolved the possessed wizard's
    /// castle without first asking whether that wizard was still a
    /// live, unreaped class-3 record.
    /// `NETHERW.EXE` 0x4B305-0x4B32D (`sub_26AA0`, EF:17337-42).
    pub(crate) fn m22_target_gates_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M22_TARGET_GATES").is_none())
    }

    /// A/B toggle for the ANTI-STACK SPAN law (dig C2, session 96):
    /// set `MGC_NO_M22_ANTISTACK_SPAN` to restore the pre-2026-09-03
    /// WRAPPING 16-bit separation test.
    fn m22_antistack_span_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M22_ANTISTACK_SPAN").is_none())
    }

    /// A/B toggle for the M27 PERTURB-TURN law (dig C2, session 96):
    /// set `MGC_NO_M27_PERTURB_TURN` to restore the pre-2026-09-03
    /// PRE-increment `setting_30` addend on the m27 branch bolt.
    fn m27_perturb_turn(turn: u32) -> u32 {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *V.get_or_init(|| std::env::var_os("MGC_NO_M27_PERTURB_TURN").is_none()) {
            turn.wrapping_add(1)
        } else {
            turn
        }
    }

    /// A/B toggle for the ANTI-STACK ROSTER law (dig C2, session 96):
    /// set `MGC_NO_M22_ANTISTACK_ROSTER` to restore the pre-2026-09-03
    /// LIVE-POOL walk with its hand-rolled class/model/state/`0x400`
    /// filter.
    fn m22_antistack_roster_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M22_ANTISTACK_ROSTER").is_none())
    }

    /// `sub_27120` (EF:17655): anti-stack z-push vs OTHER worms
    /// (id-keyed; own segments share the head's id and are skipped;
    /// the bucket holds heads only — 0xB4 is list-excluded).
    ///
    /// ⭐⭐⭐ THE SEPARATION TEST DOES NOT WRAP. `NETHERW.EXE`
    /// +0x27168-9F (file offset 0x4B968) is three copies of
    ///   `movsx eax,word[ebx+0x4c] ; movsx edx,word[ecx+0x4c]
    ///    sub eax,edx ; cdq/xor/sub (abs) ; cmp eax,esi ; jnl`
    /// — each coordinate is SIGN-EXTENDED to 32 bits FIRST and the
    /// subtraction is 32-bit, so two worms straddling the ±32768
    /// coordinate seam are 60,000+ apart, never adjacent. The port
    /// subtracted in 16 bits and re-read the result as `i16`, which
    /// silently wraps the seam into a short distance and fires the
    /// +64 hop on a worm half a map away.
    ///
    /// mc2l22 t=1093→1094 is the witness: head 375 (x=35939, i16
    /// −29597) vs head 360 (x≈32261, i16 +32261) — retail's span is
    /// 61,858 ≥ hwin 7680 so retail HOLDS z (`explain 1094 375` shows
    /// no `z` row and `f24 62 -> 61`, the ceiling-hold branch), while
    /// the port's wrap gave 3,678 < 7680 and lifted the whole chain
    /// +64 (15 rows, head 375 + segments 376..389).
    fn m22_antistack(&mut self, i: usize) {
        let (ex, ey, ez, id, model, vwin, hwin) = {
            let e = &self.ent[i];
            (
                e.x,
                e.y,
                e.z,
                e.id24,
                e.model65,
                2 * e.f84 as i16 as i32 + 32,
                2 * e.f80 as i16 as i32,
            )
        };
        // ⭐⭐ THE WALK IS THE TICK-TOP PER-MODEL ROSTER, NOT THE LIVE
        // POOL. EXE +0x27141-4E loads `bytearray_38403x[model]` (the
        // `movsx eax,byte[ebx+0x40]` model index into the `+0x9603`
        // table) and chases `next_0` with `mov ecx,[ecx]`; the ONLY
        // per-candidate test in the loop body is `id != self.id`
        // (+0x2715E-66) plus the three box tests. Class, model, state
        // and life were ALL settled when the roster was built at the
        // top of the frame (EF:39987-40008), so re-asking them at the
        // walk is wrong in both directions — and the port's
        // `flags & 0x400` skip is an INVENTED guard retail has no
        // trace of.
        let roster: Vec<u16> = if Self::m22_antistack_roster_law() {
            self.mob_chains.visible(model as usize).to_vec()
        } else {
            (1..self.ent.len())
                .filter(|&j| {
                    let c = &self.ent[j];
                    c.class64 == 5
                        && c.model65 == 22
                        && !matches!(c.tick70, 0xB4 | 0xE8 | 0xEA)
                        && c.flags & 0x400 == 0
                })
                .map(|j| j as u16)
                .collect()
        };
        for &sj in &roster {
            let j = sj as usize;
            let c = &self.ent[j];
            if c.id24 == id {
                continue;
            }
            let (dx, dy) = if Self::m22_antistack_span_law() {
                // The EXE's `movsx` pair: widen each coordinate to 32
                // bits BEFORE subtracting, so the span never wraps.
                (
                    ((ex as i16 as i32) - (c.x as i16 as i32)).abs(),
                    ((ey as i16 as i32) - (c.y as i16 as i32)).abs(),
                )
            } else {
                (
                    ((ex.wrapping_sub(c.x)) as i16 as i32).abs(),
                    ((ey.wrapping_sub(c.y)) as i16 as i32).abs(),
                )
            };
            let dz = ((ez as i32) - (c.z as i32)).abs();
            if dx < hwin && dy < hwin && dz < vwin && ez >= c.z {
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                let z = self.ent[i].z.wrapping_add(64);
                self.move_relink(i, x, y, z);
            }
        }
    }

    /// `sub_26FF0`'s uninitialised `v9x` (`[ebp-0x10]`) as the 0xB0
    /// head frame sees it: the DWORD the slot directly below left at
    /// `W-52`, split x = low word, y = high word. `None` = not
    /// modelled (another head frame, another predecessor, or an
    /// unmodelled residue) — the caller keeps the head's own position.
    /// See `no_mc2_m22_stale_probe`.
    fn m22_stale_probe(&self, i: usize) -> Option<(u16, u16)> {
        if self.ent[i].tick70 != M22_BASE {
            return None;
        }
        match self.m22_seg_residue.0 {
            Some((slot, Some(v))) if slot as usize + 1 == i => Some((v as u16, (v >> 16) as u16)),
            _ => None,
        }
    }

    /// `sub_26FF0` (EF:17589): head move + altitude — actSpeed
    /// decay, the move core bracketed by a tail-length shift, the
    /// every-16th anti-stack, and the whole-chain ceiling clamp
    /// with the f38 rise budget.
    fn m22_move(&mut self, i: usize) {
        if self.ent[i].f126 > self.ent[i].f130 {
            self.ent[i].f126 -= 2;
        }
        let (save_p, save_f) = (self.ent[i].f80, self.ent[i].f84);
        let shift = (self.ent[i].f71 as u16) << 8;
        self.mc2_shift_rot(i, shift, save_f);
        self.mc2_move_core(i);
        if self.ent[i].f63 & 0xF == 0 {
            self.m22_antistack(i);
        }
        self.mc2_shift_rot(i, save_p, save_f);
        // Whole-chain highest terrain altitude (+ the position it
        // occurred at, for the rise-rate roughness test).
        // ⭐ Retail seeds the maximum at 0 and copies the position on a
        // strict `>` only, so an all-water chain probes the stale stack
        // word the previous handler left (`no_mc2_m22_stale_probe`).
        let stale = if crate::engine::features::no_mc2_m22_stale_probe() {
            None
        } else {
            self.m22_stale_probe(i)
        };
        let mut best: i16 = if stale.is_some() { 0 } else { i16::MIN };
        let mut best_pos = stale.unwrap_or((self.ent[i].x, self.ent[i].y));
        let mut j = i;
        loop {
            let (cx, cy) = (self.ent[j].x, self.ent[j].y);
            let g = self.ground_z(cx, cy) as i16;
            if g > best {
                best = g;
                best_pos = (cx, cy);
            }
            j = self.ent[j].f54 as usize;
            if j == 0 {
                break;
            }
        }
        let ceiling = best.wrapping_add(384);
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        if z >= ceiling {
            if self.ent[i].f38 != 0 {
                self.ent[i].f38 -= 1; // burn the rise budget, hold z
            } else {
                self.move_relink(i, x, y, z.wrapping_sub(2));
            }
        } else {
            let steep = self.roughness(best_pos.0, best_pos.1)
                > BEHAVIOR[self.ent[i].row156 as usize].v_16 as i32;
            let dz = if steep { 0x100 } else { 0x40 };
            self.move_relink(i, x, y, z.wrapping_add(dz));
            self.ent[i].f38 = 0x40;
        }
    }

    /// `sub_27430` (EF:17806): the writhe frame-step band.
    fn m22_anim_step(frame: u8) -> u8 {
        if frame >= 96 {
            2
        } else if frame >= 87 {
            3
        } else if frame >= 60 {
            4
        } else if frame < 30 {
            6
        } else {
            5
        }
    }

    /// `sub_272C0` (EF:17720): writhe animation (tailLen >= 11
    /// only; SOUND 48 in frames (0,16)), serpentine spin advance,
    /// spin-rate decay toward ±11 every 4th phase.
    fn m22_anim(&mut self, i: usize) {
        if self.ent[i].f71 >= 11 {
            let step = Self::m22_anim_step(self.ent[i].frame88);
            let frame = self.ent[i].frame88;
            if frame != 0 && frame < 0x10 {
                self.snd(48, i);
            }
            if self.ent[i].f56 & 1 != 0 {
                let v3 = frame as u16 + step as u16;
                if v3 > 0x64 {
                    self.ent[i].frame88 = 100;
                    self.ent[i].f56 &= 0xFE; // start counting down
                } else {
                    self.ent[i].frame88 = v3 as u8;
                }
            } else if frame > step {
                self.ent[i].frame88 = frame - step;
            } else {
                let v5 = self.ent[i].f56 | 1;
                self.ent[i].frame88 = 0;
                self.ent[i].f56 = v5 ^ 2; // chirality flip each bounce
            }
        }
        let spin = self.ent[i].f44 as i16;
        self.ent[i].f46 = (self.ent[i].f46.wrapping_add(spin)) & 0x7FF;
        if self.ent[i].f63 & 3 == 0 {
            let spin = self.ent[i].f44 as i16;
            let mag = (spin.abs() - 5).max(11);
            self.ent[i].f44 = (if spin <= 0 { -mag } else { mag }) as u16;
        }
    }

    /// `sub_26F10` (EF:17542): head damage-turn (accelerate by
    /// dmg/4, turn AWAY from the attacker) + ch1 retarget + the
    /// life<0 → chain-kill transition. The head's own life NEVER
    /// drops here — melee only enrages.
    fn m22_dmg(&mut self, i: usize, ctx: &MobCtx) {
        if self.ent[i].f58 != 0 {
            if self.ent[i].mail[0].1 != 0 {
                let (amt, src) = self.ent[i].mail[0];
                let mut v = ((amt >> 2) as u16 as i16).wrapping_add(self.ent[i].f126);
                if v < self.ent[i].f130 {
                    v = self.ent[i].f130;
                }
                if v > self.ent[i].f128 {
                    v = self.ent[i].f128;
                }
                self.ent[i].f126 = v;
                self.ent[i].mail[0].1 = 0;
                if let Some((ax, ay, _)) = self.mc2_raw_pos(src, ctx) {
                    // tan2(attacker → self) = turn AWAY (EF:17568).
                    let (ex, ey) = (self.ent[i].x, self.ent[i].y);
                    let yaw = Self::angle_between(ax, ay, ex, ey);
                    self.ent[i].f30 = yaw;
                    self.ent[i].f34 = yaw;
                }
            }
            let tag = self.ent[i].mail[1].1;
            if tag != 0 {
                if tag != self.ent[i].f144 {
                    self.ent[i].f144 = tag;
                    self.ent[i].tick70 = M22_BASE + 1;
                    self.ent[i].f26 = 0;
                    if tag == PLAYER_TARGET {
                        self.snd_player(4);
                    } else {
                        self.snd(4, tag as usize);
                    }
                }
                self.ent[i].mail[1].1 = 0;
            }
        }
        if self.ent[i].act_life < 0 {
            self.ent[i].tick70 = M22_BASE + 5; // 0xB5 chain-kill
        }
    }

    /// `sub_27880` (EF:18012): the 1024-tick grow cycle — tail +2
    /// (to <=15) and mana +1000 (cap 50000).
    fn m22_grow(&mut self, i: usize) {
        if self.ent[i].f146 != 0 {
            self.ent[i].f146 -= 1;
            return;
        }
        self.ent[i].f146 = 1024;
        let len = self.ent[i].f71;
        if len <= 13 {
            self.m22_resize(i, len + 2);
        }
        if self.ent[i].f140 < 50000 {
            self.ent[i].f140 += 1000;
        }
    }

    /// `sub_27720` (EF:17938): grow/shrink the tail to an odd
    /// target length — segment PAIRS (+n,−n) appended at the tail
    /// or hidden from it, then recolor + re-spacing.
    fn m22_resize(&mut self, head: usize, target: u8) {
        let target = target | 1;
        let cur = self.ent[head].f71;
        if !(1..=15).contains(&target) || cur == target {
            return;
        }
        // Walk to the tail end.
        let mut last = head;
        while self.ent[last].f54 != 0 {
            last = self.ent[last].f54 as usize;
        }
        let mut failed = false;
        if cur >= target {
            // SHRINK: remove ((cur-target)/2) ring pairs from the end.
            let mut removed = 0i16;
            while removed < ((cur - target) / 2) as i16 {
                let minus = self.ent[last].f52 as usize; // -offset twin
                if minus == 0 {
                    break;
                }
                let anchor = self.ent[minus].f52 as usize;
                self.ent[anchor].f54 = 0;
                self.ent[minus].flags |= 0x400;
                self.ent[last].flags |= 0x400;
                removed += 1;
                last = anchor;
            }
        } else {
            // GROW: one (+n, −n) ring; both slots allocated before
            // the copies, like retail's two NewEvents (alloc order
            // is stream-visible).
            let n = (self.ent[last].f71 as i8).unsigned_abs() as i8 + 1;
            if let Some(plus) = self.new_event() {
                if let Some(minus) = self.new_event() {
                    self.mc2_m22_seed_segment(head, last, plus, n);
                    self.mc2_m22_seed_segment(head, plus, minus, -n);
                } else {
                    self.ent[plus].flags |= 0x400; // rollback (EF:17982)
                    failed = true;
                }
            } else {
                failed = true;
            }
        }
        if !failed {
            self.ent[head].f71 = target;
            self.mc2_m22_colorize(head);
            self.mc2_m22_shift_rot(head);
        }
    }

    /// `sub_274C0` with a pre-allocated slot (the resize path).
    fn mc2_m22_seed_segment(&mut self, head: usize, prev: usize, seg: usize, off: i8) {
        self.ent[seg] = self.ent[prev];
        self.ent[seg].thing_slot = 0;
        self.ent[seg].f52 = prev as u16;
        self.ent[prev].f54 = seg as u16;
        self.ent[seg].f54 = 0;
        self.ent[seg].f63 = off.unsigned_abs() & 1;
        self.ent[seg].flags &= !4;
        self.ent[seg].f71 = off as u8;
        self.ent[seg].tick70 = M22_BASE + 4;
        self.ent[seg].f44 = 0;
        self.ent[seg].f144 = 0;
        self.ent[seg].f140 = 0;
        self.ent[seg].f146 = head as u16;
        let (hx, hy, hz) = {
            let h = &self.ent[head];
            (h.x, h.y, h.z)
        };
        self.link(seg, hx, hy, hz);
        self.refill_life(seg);
    }

    /// `sub_27470` (EF:17822): the segment at a signed ring offset
    /// (0 = the head itself).
    fn m22_find_segment(&self, head: usize, off: i16) -> Option<usize> {
        if off == 0 {
            return Some(head);
        }
        let mut j = self.ent[head].f54 as usize;
        while j != 0 {
            if self.ent[j].f71 as i8 as i16 == off {
                return Some(j);
            }
            j = self.ent[j].f54 as usize;
        }
        None
    }

    /// The m22 castle resolver: the target player's
    /// `CastleEntityIndex_0x3A_58`.
    fn m22_target_castle(&self, target: u16) -> Option<usize> {
        self.mc2_castle_of(target)
    }

    /// m22 states 0xB0-0xB7 (docs/traces/mc2-m22-worm-helpers.md).
    pub(crate) fn m22_tick(&mut self, i: usize, ctx: &MobCtx) {
        match self.ent[i].tick70 - M22_BASE {
            // 0xB0 idle: move/anim/damage/grow.
            0 => {
                self.m22_move(i);
                self.m22_anim(i);
                self.m22_dmg(i, ctx);
                self.m22_grow(i);
            }
            // 0xB1 chase + the colorize-inward sweep (sub_26990).
            1 => {
                self.m22_move(i);
                self.m22_anim(i);
                let center = (self.ent[i].f26 >> 8) as i16;
                let radius = (self.ent[i].f26 & 0xFF) as i16;
                let base = self.mc2_ball_color(self.ent[i].f144);
                let len = self.ent[i].f71;
                let mut found = false;
                let passes = if radius != 0 { 2 } else { 1 };
                for p in 0..passes {
                    let off = center + if p == 1 { -radius } else { radius };
                    if off.unsigned_abs() as u8 <= len / 2
                        && let Some(seg) = self.m22_find_segment(i, off)
                    {
                        let row = Self::m22_color_idx(base, len, off as i8);
                        self.mc2_particle_row(seg, row);
                        found = true;
                    }
                }
                if found {
                    self.ent[i].f26 = (radius + 1) | (center << 8);
                } else if self.ent[i].f144 != 0 {
                    self.ent[i].tick70 = M22_BASE + 2; // → castle acquire
                } else {
                    self.ent[i].tick70 = M22_BASE; // → idle
                }
            }
            // 0xB2 castle acquire (sub_26AA0) — every 32nd phase.
            2 => {
                self.m22_move(i);
                self.m22_anim(i);
                self.m22_dmg(i, ctx);
                self.m22_grow(i);
                if self.ent[i].f63 & 0x1F != 0 {
                    return;
                }
                let target = self.ent[i].f144;
                let mut revert = false;
                if target == 0 {
                    revert = true;
                } else if self.ent[i].f126 <= self.ent[i].f130 {
                    // (still accelerated → hold in 0xB2 without any check)
                    //
                    // ⭐⭐⭐ THE THREE TARGET GATES — AN ABSENCE IN AN
                    // ENUMERATED LIST. Before retail resolves the
                    // castle it interrogates the POSSESSED WIZARD
                    // ITSELF, and each answer reverts (LABEL_17):
                    //   `NETHERW.EXE` 0x4B305-0x4B32D (linear
                    //   0x26B05.., `sub_26AA0`, EF:17337-42):
                    //     mov  0x1a3e4(,%eax,4),%eax   ; Entities[t]
                    //     cmpb $0x3,0x3f(%eax)  ; jne LABEL_17
                    //     cmpl $0x0,0x8(%eax)   ; jl  LABEL_17
                    //     testb $0x4,0xd(%eax)  ; jne LABEL_17
                    // i.e. the target must still be CLASS 3, must have
                    // life >= 0 (a 32-bit `cmpl`, so the whole signed
                    // life), and must not carry `struct_byte_0xc`
                    // byte[1] bit 2 = dword bit 10 = the port's
                    // `flags & 0x400` reap mark. Only then is
                    // `dword_0xA4_164->CastleEntityIndex_0x3A_58`
                    // read. The port asked NONE of the three: it went
                    // straight to the castle lookup, so a worm that
                    // had latched onto a wizard kept draining toward
                    // that wizard's castle after the wizard died.
                    // rsg t=25,532 is the witness: head slot 3 holds
                    // `player_ent = 343`, and 343 (the human carpet)
                    // is at `life = -1270`; retail reverts
                    // (`scratch10 779 -> 0`, `player_ent 343 -> 0`,
                    // `action 178 -> 177`) while the port found the
                    // human's castle 63 nearly 10,000 units away,
                    // failed the 0x100 range test and simply held in
                    // 0xB2 forever.
                    //
                    // ⚠ THE HUMAN IS OUT-OF-POOL. `self.ent[343]` is a
                    // RESERVED HOLE in the port (all zeroes — see
                    // `dump-state --port`'s own banner), and the port
                    // names the human carpet with the `PLAYER_TARGET`
                    // sentinel wherever retail carries a pool index.
                    // Its mirror of `life_0x8 < 0` is `ctx.pdead`, the
                    // mid-walk republished `player.life < 0` echo that
                    // exists for exactly this reason (world.rs:3047 —
                    // "retail's followers read `life_0x8 < 0` off the
                    // pool record live"); it is always class 3 and is
                    // never reap-flagged.
                    let t = target as usize;
                    let dead_target = Self::m22_target_gates_law()
                        && if target == PLAYER_TARGET {
                            ctx.pdead
                        } else {
                            t < self.ent.len()
                                && (self.ent[t].class64 != 3
                                    || self.ent[t].act_life < 0
                                    || self.ent[t].flags & 0x400 != 0)
                        };
                    if dead_target {
                        revert = true;
                    } else {
                        match self.m22_target_castle(target) {
                            None => revert = true,
                            Some(c) => {
                                let (cx, cy) = {
                                    let e = &self.ent[c];
                                    (e.x, e.y)
                                };
                                let (ex, ey) = {
                                    let e = &self.ent[i];
                                    (e.x, e.y)
                                };
                                // ⭐ RETAIL RE-AIMS THE TARGET, NOT THE
                                // LIVE HEADING. EF:17348 is one write —
                                // `a1x->roll_0x20_32 = v5` — and the
                                // move core (`sub_26FF0` → `sub_1B8C0`,
                                // EF:17324, already run above) is what
                                // walks the live yaw toward it, capped
                                // at row 90's v_2 = 256/tick and one
                                // frame LATE (the roll posted here is
                                // first consumed on the NEXT tick).
                                // mc2l22 t=12183 slot 360 (explain
                                // t=12182→12183): retail yaw 1467 →
                                // 1382 — it arrives at the STALE roll
                                // 1382 — while roll goes 1382 → 1775;
                                // the snap wrote 1775 into the heading
                                // a whole slew early. Same law as m4's
                                // militia (CONFORMANCE-FINDINGS "The
                                // militiaman turns, never snaps", mc1l0
                                // t=5051).
                                let aim = Self::angle_between(ex, ey, cx, cy);
                                self.ent[i].f34 = aim;
                                // `EuclideanDistXYZ_58490` is 2-D despite
                                // the name (Maths:738-42 never reads z —
                                // the morph::dist2d law). A 3-D check here
                                // is unsatisfiable: the head cruises at
                                // chain-ground +384, the castle entity
                                // sits at ground, so the worm would hover
                                // at the flag forever, never absorbed.
                                let d2 = crate::mc2::morph::dist2d(ex, ey, cx as i32, cy as i32);
                                if self.ent[i].f63 & 3 == 0 && d2 <= 0x100 {
                                    let room =
                                        (self.ent[i].f140 + self.ent[c].f140) < self.ent[c].f136;
                                    if room {
                                        self.ent[i].f26 = 128;
                                        self.ent[i].tick70 = M22_BASE + 3; // → deposit
                                    } else {
                                        revert = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if revert {
                    self.ent[i].f26 = 0;
                    self.ent[i].f144 = 0;
                    self.ent[i].tick70 = M22_BASE + 1; // → chase/sweep
                }
            }
            // 0xB3 deposit / self-consume (sub_26BD0) — anim only.
            3 => {
                self.m22_anim(i);
                if self.ent[i].f26 != 0 {
                    self.ent[i].f26 -= 1;
                } else if self.ent[i].f63 & 1 == 0 {
                    // Timer expired: shrink every even phase (the
                    // countdown is NOT reloaded — EF:17381).
                    let len = self.ent[i].f71;
                    if len > 1 {
                        self.m22_resize(i, len - 2);
                    } else {
                        if let Some(c) = self.m22_target_castle(self.ent[i].f144) {
                            let total = self.ent[i].f140 + self.ent[c].f140;
                            self.ent[c].f140 = total.min(self.ent[c].f136);
                        }
                        self.ent[i].flags |= 0x400; // consumed
                    }
                }
            }
            // 0xB4 tail segment: spiral follow + hit relay.
            4 => {
                // `sub_271D0` reaches `CopyEntityPosition_57CF0` (whose
                // `push esi` leaves this record's pointer at W-52)
                // whenever the head link is set; a relay call then
                // overwrites it (`no_mc2_m22_stale_probe`).
                let reached = self.ent[i].f146 != 0;
                self.m22_tail_follow(i);
                let call = self.m22_relay(i, ctx);
                let own = crate::engine::features::MC2_RETAIL_REC_PTR_0
                    .wrapping_add(168 * i as u32);
                self.m22_seg_residue.0 = match call {
                    Some(v) => Some((i as u16, v)),
                    None if reached => Some((i as u16, Some(own))),
                    None => None,
                };
            }
            // 0xB5 chain-kill.
            5 => self.m22_chain_kill(i),
            // 0xB6: no unique body in retail (interior of sub_27720).
            6 => {}
            // 0xB7 spawn: sub_1D5D0 no-op for StageVar2==0.
            _ => {}
        }
    }

    // =========================================================================
    // MODEL 27 — the HYDRA: 5 bolt-spitting HEADS (branches) that
    // retract and re-grow when killed; the body is attackable only
    // while the f50 head gauge is 0.
    // (ctor sub_4D000 EF:34591 + finalizers; body brains EF:19443-
    // 19736; the branch machine docs/traces/mc2-m27-branch-machine.md)
    // =========================================================================

    /// `sub_4D000`: 1 body + 5 branches + 45 segments = 51 slots,
    /// one linear f54 chain; every member's f52/id24 point at the
    /// BODY. CAVE-EXCLUDED — on caves `v16 = 1` skips the whole
    /// construction and returns 0 (EF:34608-34690).
    pub(crate) fn mc2_spawn_m27(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        if self.is_cave() {
            return None;
        }
        if self.free.len() < 51 {
            return None;
        }
        // ⚠ `f44` on this family is `word_0x2C_44` (the branch
        // speed-mode selector), NOT `subSpellIndex_0x2A_42` — and
        // `new_event` seeds the @0x2A default 100 into the one field
        // both words share. Retail's `memset` leaves @0x2C at 0 and
        // `sub_4D000` (banner EF:34642) never writes it, so every
        // member is born in mode 0. See
        // [`crate::mc2::mobs::no_mc2_m27_2c_zero`].
        let zero2c = !crate::mc2::mobs::no_mc2_m27_2c_zero();
        let body = self.new_event()?;
        {
            let e = &mut self.ent[body];
            e.class64 = 5;
            e.model65 = 27;
            e.tick70 = M27_BASE + 1; // 0xD9
            if zero2c {
                e.f44 = 0;
            }
        }
        self.link(body, x, y, z);
        let mut prev = body;
        for b in 0..5u8 {
            let Some(br) = self.new_event() else { break };
            {
                let e = &mut self.ent[br];
                e.class64 = 5;
                e.model65 = 27;
                e.tick70 = BRANCH_STATE;
                e.f50 = b as i16; // byte_0x3B_59 — branch index
                e.id24 = body as u16;
                e.f52 = body as u16;
                e.f54 = 0;
                if zero2c {
                    e.f44 = 0;
                }
            }
            self.ent[prev].f54 = br as u16;
            self.link(br, x, y, z);
            prev = br;
            for _ in 0..9 {
                let Some(seg) = self.new_event() else { break };
                {
                    let e = &mut self.ent[seg];
                    e.class64 = 5;
                    e.model65 = 27;
                    e.tick70 = TIER2_STATE;
                    e.f50 = b as i16;
                    e.id24 = body as u16;
                    e.f52 = body as u16;
                    e.f54 = 0;
                    if zero2c {
                        e.f44 = 0;
                    }
                }
                self.ent[prev].f54 = seg as u16;
                self.link(seg, x, y, z);
                prev = seg;
            }
        }
        self.m27_body_init(body);
        self.m27_branch_init(body);
        self.m27_segment_init(body);
        Some(body)
    }

    /// `sub_2AC50` (EF:20730) — body finalize. Life = 1000000
    /// DIRECT (no CopyMaxLifeToLife); the f50 gauge starts at 5.
    fn m27_body_init(&mut self, body: usize) {
        let ord = self.mc2_ord(27);
        {
            let e = &mut self.ent[body];
            e.f30 = 0;
            e.f32 = 0;
            e.f34 = 0;
            e.f128 = 64;
            e.f130 = 0;
            e.f126 = 30;
            e.f50 = 5; // the live-branch gauge
            e.act_life = 1_000_000;
            e.max_life = 36000;
            e.f140 = 20000;
            e.f26 = (body % 100) as i16;
            e.f36 = 0;
            e.f28 = 1; // byte_0x38_56 = 1
            e.row156 = 97;
            e.f63 = ord;
            e.f66 = 3;
        }
        self.ent[body].f58 = BEHAVIOR[97].v_26 + 1; // byte_0x39_57 = v26+1
        self.mc2_set_sprite(body, 315);
        self.mc2_shift_rot(body, 1024, 1536);
    }

    /// `sub_2AD40` (EF:20770-800) — branch finalize: sprite 316, TWO
    /// RNG draws each (roll then fov), life ladder 460*v2+920 where
    /// v2 counts every chain NODE (the increment sits OUTSIDE the
    /// branch guard, EF:20798-99): branches sit at positions
    /// 1/11/21/31/41 → 1380/5980/10580/15180/19780. NOT
    /// branch-only counting, which makes 2-5 up to 6× too weak.
    fn m27_branch_init(&mut self, body: usize) {
        let mut v2 = 1i32;
        let mut j = self.ent[body].f54 as usize;
        while j != 0 {
            if self.ent[j].tick70 == BRANCH_STATE {
                self.mc2_set_sprite(j, 316);
                let d1 = self.mc2_rand(j);
                self.ent[j].f34 = (d1 & 0x7FF) as u16; // roll
                let d2 = self.mc2_rand(j);
                self.ent[j].f36 = (d2 & 0x7FF) as u16; // fov
                {
                    let e = &mut self.ent[j];
                    e.f128 = 16;
                    e.f126 = 16;
                    e.row156 = 103;
                    e.f28 = 1;
                    let v5 = (460 * v2 + 920) as u32;
                    e.max_life = v5;
                    e.act_life = v5 as i32;
                }
                // sub_2A940 places the fresh branch (EF:20798).
                self.m27_swing_branch(body, j);
            }
            v2 += 1;
            j = self.ent[j].f54 as usize;
        }
    }

    /// A/B toggle for the M27 BRANCH SCAN ROSTER law (round 104,
    /// dig W2-F): `MGC_NO_M27_SCAN_ROSTER` restores the pool sweep
    /// with the invented `model65 <= 1` filter the port carried
    /// before.
    fn m27_scan_roster_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_SCAN_ROSTER").is_none())
    }

    /// `sub_2A6F0` (EF:20452-83) — the m27 branch's OWN wizard scan:
    /// walks the wizard list with STRICT `<` on both dist² and the
    /// nearest compare and NO invisibility/hidden filter — unlike
    /// the shared `mc2_wizard_scan` (a different retail sub), which
    /// must keep its filters for its other callers.
    ///
    /// ⭐⭐⭐ AND THE LIST IT WALKS IS `dword_38519`, THE TICK-TOP
    /// CLASS-3 ROSTER (EF:20466 `v3x = x_D41A0_BYTEARRAY_4_struct.
    /// dword_38519`, walked to `Entities_EA3E4[0]` through `next_0`)
    /// — the same roster [`Gen::mc2_wizard_scan`] walks, with the
    /// same consequence: **THE WALK RE-ASKS NOTHING.** Class, life
    /// and the reap flag were settled when the case-3 arm of the
    /// tick-top sweep built the list (EF:39972-85, `class == 3 &&
    /// life >= 0`), and **THERE IS NO MODEL TEST ANYWHERE** — so a
    /// (3,2) castle or a (3,3) can win this nearest-in-cone scan and
    /// keep a farther wizard from winning it. The port swept the
    /// LIVE pool with an invented `model65 <= 1`, exactly the filter
    /// [`Gen::mc2_wizard_scan`] was already corrected for; mc2l22
    /// t=21616 is the witness — retail's branch at slot 256 locks
    /// the (3,3) at slot 268 (`byte_0x46_70` 1→2, `word_0x96_150`
    /// 0→268) and enters the mode-2 whip, whose decel takes
    /// `actSpeed` 192→176 at t=21617; the filtered port found
    /// nothing and stayed in mode 0 with `actSpeed` pinned at 192.
    fn m27_wizard_scan(&self, i: usize, ctx: &MobCtx) -> Option<u16> {
        let e = &self.ent[i];
        let row = &BEHAVIOR[e.row156 as usize];
        let range = (row.v_28 as i32) * (row.v_28 as i32);
        let cone = row.v_30 as u16;
        let (ex, ey, eyaw) = (e.x, e.y, e.f30);
        let mut best: Option<(u16, i32)> = None;
        let mut consider = |tx: u16, ty: u16, slot: u16| {
            let d2 = Self::dist2_sq(ex, ey, tx, ty);
            if d2 >= range {
                return; // strict < (EF:20461)
            }
            let bearing = Self::angle_between(ex, ey, tx, ty);
            if Self::angdist(eyaw, bearing) >= cone {
                return;
            }
            if best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((slot, d2));
            }
        };
        // The out-of-pool human takes the roster's ENTRY test
        // (`life >= 0` at tick top) — see `m27_scan_life_law`.
        // (`player_ghost`: the ghost cheat's own gate — retail lets
        // this scan see through a cloak, the cheat does not.)
        if (!ctx.pdead_top || !Self::m27_scan_life_law()) && !self.player_ghost.0 {
            consider(ctx.px, ctx.py, PLAYER_TARGET);
        }
        if Self::m27_scan_roster_law() {
            for c in 0..self.wiz_chain.visible_len() {
                let j = self.wiz_chain.list[c] as usize;
                let w = &self.ent[j];
                consider(w.x, w.y, j as u16);
            }
        } else {
            for (j, c) in self.ent.iter().enumerate().skip(1) {
                if c.class64 == 3 && c.model65 <= 1 && c.act_life >= 0 && c.flags & 0x400 == 0 {
                    consider(c.x, c.y, j as u16);
                }
            }
        }
        best.map(|(s, _)| s)
    }

    /// `sub_2AE30` (EF:20808) — segment finalize: sprite 317 only.
    fn m27_segment_init(&mut self, body: usize) {
        let mut j = self.ent[body].f54 as usize;
        while j != 0 {
            if self.ent[j].tick70 == TIER2_STATE {
                self.mc2_set_sprite(j, 317);
            }
            j = self.ent[j].f54 as usize;
        }
    }

    /// `sub_2A5B0` (EF:20374): branch-head anchor placement.
    fn m27_anchor_branch(&mut self, body: usize, br: usize, reach: i16) {
        let row = &D404C[(self.ent[br].f50 as usize).min(4)];
        let (bx, by, bz, byaw, bpitch) = {
            let b = &self.ent[body];
            (b.x, b.y, b.z, b.f30, b.f32)
        };
        let mut pred = (bx, by, bz);
        Self::polar_step(
            &mut pred,
            (row[D404C_W2] as u16).wrapping_add(byaw) & 0x7FF,
            0,
            row[D404C_W0],
        );
        pred.2 = pred.2.wrapping_add(row[D404C_W4]);
        Self::polar_step(
            &mut pred,
            (row[D404C_W12] as u16).wrapping_add(byaw) & 0x7FF,
            (row[D404C_W14] as u16).wrapping_add(bpitch) & 0x7FF,
            reach,
        );
        self.move_relink(br, pred.0, pred.1, pred.2);
    }

    /// `sub_2A940` (EF:20570): move the branch head by its swing
    /// speed along the splay direction (the `x_DWORD_E9BA8` freeze
    /// gate reads 0 — the normal arm; module doc).
    fn m27_swing_branch(&mut self, body: usize, br: usize) {
        if self.ent[br].f126 == 0 {
            return;
        }
        let (byaw, bpitch) = (self.ent[body].f30, self.ent[body].f32);
        let (x, y, z, roll, fov, spd) = {
            let e = &self.ent[br];
            (e.x, e.y, e.z, e.f34, e.f36, e.f126)
        };
        let mut pred = (x, y, z);
        Self::polar_step(
            &mut pred,
            roll.wrapping_add(byaw) & 0x7FF,
            fov.wrapping_add(bpitch) & 0x7FF,
            spd,
        );
        self.move_relink(br, pred.0, pred.1, pred.2);
    }

    /// `sub_2A9F0` (EF:20608): the trailing settle after the spline.
    fn m27_settle_branch(&mut self, body: usize, br: usize) {
        let row = &D404C[(self.ent[br].f50 as usize).min(4)];
        let (byaw, bpitch) = (self.ent[body].f30, self.ent[body].f32);
        let (x, y, z) = {
            let e = &self.ent[br];
            (e.x, e.y, e.z)
        };
        let mut pred = (x, y, z.wrapping_add(row[D404C_W10]));
        Self::polar_step(
            &mut pred,
            (row[D404C_W12] as u16).wrapping_add(byaw) & 0x7FF,
            (row[D404C_W14] as u16).wrapping_add(bpitch) & 0x7FF,
            row[D404C_W6],
        );
        self.move_relink(br, pred.0, pred.1, pred.2);
    }

    /// `sub_2AA90` (EF:20632): the 9-segment drooping-arc spline
    /// from the body anchor to the branch head; fixed 96-unit
    /// steps, symmetric pitch-bend pattern from the D40BC row.
    fn m27_spline_segments(&mut self, body: usize, br: usize) {
        let row = &D404C[(self.ent[br].f50 as usize).min(4)];
        let (bx, by, bz, byaw) = {
            let b = &self.ent[body];
            (b.x, b.y, b.z, b.f30)
        };
        let mut anchor = (bx, by, bz);
        Self::polar_step(
            &mut anchor,
            (row[D404C_W2] as u16).wrapping_add(byaw) & 0x7FF,
            0,
            row[D404C_W0],
        );
        anchor.2 = anchor.2.wrapping_add(row[D404C_W4]);
        let head_pos = {
            let e = &self.ent[br];
            (e.x, e.y, e.z)
        };
        let v5 = (Self::mc2_dist3(anchor, head_pos) as i32 - 468) / 24;
        let v18 = (16 - v5).clamp(0, 15) as usize;
        let yaw = Self::angle_between(anchor.0, anchor.1, head_pos.0, head_pos.1);
        let pitch = Self::mc2_radix_tan(anchor, head_pos);
        let extend = self.ent[br].f71 == 7 && self.ent[br].f69 == 8;
        let mut cursor = anchor;
        let mut seg = self.ent[br].f54 as usize;
        for v6 in 0..9 {
            if seg == 0 || self.ent[seg].tick70 != TIER2_STATE {
                break;
            }
            let bend: i16 = match v6 {
                0 => 0,
                1 | 8 => -D40BC[v18][0],
                2 | 7 => -D40BC[v18][1],
                3 | 6 => D40BC[v18][1],
                _ => D40BC[v18][0], // 4 | 5
            };
            if v6 != 0 {
                Self::polar_step(
                    &mut cursor,
                    yaw,
                    (bend as u16).wrapping_add(pitch) & 0x7FF,
                    96,
                );
            }
            let mut place = cursor;
            if extend {
                let g = self.ground_z(place.0, place.1) as i16;
                if place.2 <= g {
                    place.2 = g;
                }
            }
            self.move_relink(seg, place.0, place.1, place.2);
            seg = self.ent[seg].f54 as usize;
        }
    }

    /// `sub_2A340` (EF:20233): the branch speed/rotation integrator
    /// (mode in f44). Mode 0's default arm advances the branch LCG.
    fn m27_integrate(&mut self, br: usize) {
        match self.ent[br].f44 as i16 {
            0 => {
                {
                    let e = &mut self.ent[br];
                    let (mx, w36) = (e.f130, e.f56 as i16);
                    e.f34 = e.f34.wrapping_add((w36 + mx + 73) as u16);
                    e.f36 = e.f36.wrapping_add((w36 + mx + 62) as u16);
                    if e.f126 != 192 {
                        let v10 = e.f128 + e.f126;
                        e.f126 = v10;
                        if v10.abs() > 192 {
                            e.f126 = if e.f128 <= 0 { -192 } else { 192 };
                            e.f128 = -e.f128;
                        }
                    }
                    if e.f63 & 1 == 0 && e.f56 != 0 {
                        e.f56 -= 1;
                    }
                }
                if self.ent[br].f68 != 0 {
                    if self.ent[br].f68 == 3 && self.ent[br].f126 == 192 {
                        self.ent[br].f128 = -16;
                        self.ent[br].f126 += self.ent[br].f128;
                    }
                } else {
                    let d = self.mc2_rand(br);
                    self.ent[br].f130 = (d % 0x1C) as i16;
                }
            }
            1 => {
                let e = &mut self.ent[br];
                if e.f126.abs() < 192 {
                    e.f126 += e.f128;
                }
                if e.f128 <= 0 {
                    if e.f126 < -192 {
                        e.f126 = -192;
                    }
                } else if e.f126 > 192 {
                    e.f126 = 192;
                }
            }
            2 => {
                let e = &mut self.ent[br];
                if e.f126.abs() < e.f128 {
                    e.f126 = 0;
                } else if e.f126 <= 0 {
                    e.f126 += e.f128;
                } else {
                    e.f126 -= e.f128;
                }
            }
            3 | 4 => {
                let e = &mut self.ent[br];
                match e.f26 {
                    1 => e.f126 = -192,
                    2 => e.f126 = -130,
                    3 => e.f126 = -23,
                    4 => e.f126 = 192,
                    _ => {}
                }
            }
            6 => {
                let (x, y) = (self.ent[br].x, self.ent[br].y);
                let z = self.ent[br].z.wrapping_sub(self.ent[br].f56 as i16);
                self.ent[br].f126 -= self.ent[br].f128;
                let g = self.ground_z(x, y) as i16;
                self.move_relink(br, x, y, z.max(g));
            }
            _ => {}
        }
    }

    /// `sub_2A660` (EF:20395): branch hit intake — forward to the
    /// body's inbox, apply capped-76 damage to the BRANCH, death →
    /// sub-state 6 (retract; branches are regenerating limbs).
    fn m27_branch_intake(&mut self, body: usize, br: usize) {
        if self.ent[br].mail[0].1 == 0 {
            return;
        }
        let (amt, src) = self.ent[br].mail[0];
        self.ent[body].mail[0] = (amt, src);
        let v4 = amt.min(76);
        self.ent[br].act_life -= v4 as i32;
        self.ent[br].mail[0].1 = 0;
        self.ent[br].f40 = src;
        if self.ent[br].act_life < 0 {
            self.ent[br].f71 = 6;
        }
    }

    /// `sub_2A6B0` (EF:20423): body hit consume — 0 none / 1 hit
    /// while branches live / 2 exposed (gauge 0 → state 0xDC armed
    /// in place, attacker in f38).
    fn m27_body_intake(&mut self, body: usize) -> u8 {
        let src = self.ent[body].mail[0].1;
        if src == 0 {
            return 0;
        }
        self.ent[body].f40 = src;
        self.ent[body].mail[0].1 = 0;
        if self.ent[body].f50 != 0 {
            1
        } else {
            self.ent[body].tick70 = M27_BASE + 4; // 220
            self.ent[body].f38 = src;
            2
        }
    }

    /// `sub_2A7F0` (EF:20507): the branch bolt — (9,0) low / (9,9)
    /// high keyed on manaRegen (f136), subSpell 850, sounds 15/23
    /// at the BODY. The a3=0 re-fire path spawns only at regen 2.
    fn m27_branch_bolt(&mut self, br: usize, target: u16, low: bool, ctx: &MobCtx) {
        if low {
            let d = self.mc2_rand(br);
            // The `+= setting_30` perturb after the roll (EF:20521).
            //
            // ⭐⭐⭐ A LAW LANDED ON ONE CALL PATH IS NOT LANDED.
            // `setting_30` is bumped by PlayerEvents (EF:37557,
            // beside `Turn_2BE0++`) BEFORE the entity walk reads it,
            // so inside a tick the global reads (recorded turn@t)+1.
            // The port already established exactly that for the cave
            // drip — `World::tick_inner`'s "POST-increment counter"
            // note bumps `mc2_turn` before the `& 7` cadence test —
            // but `MobCtx::mc2_turn` is captured at the tick TOP, one
            // short, and this perturb spent the pre-increment value.
            //
            // mc2l22 dates it: 126 of the take's 161 dirty (5,27)
            // `rand` rows are off by EXACTLY −1 (121 distinct ticks,
            // 5 slots, t=16824..55120), and `mana_regen` — the `d %
            // 12 > 7` consumer of the SAME draw — is clean on every
            // one of them, so the LCG step is right and only the
            // addend is short.
            self.mc2_rand_perturb(br, Self::m27_perturb_turn(ctx.mc2_turn));
            self.ent[br].f136 = ((d % 12 > 7) as i32) + 1;
        }
        let regen = self.ent[br].f136;
        let (x, y, z, lift, body_id) = {
            let e = &self.ent[br];
            (e.x, e.y, e.z, (e.f84 / 2) as i16, e.id24)
        };
        let p = match regen {
            1 => {
                if !low {
                    return; // re-fire noop at regen 1
                }
                let Some(p) = self.mc2_spawn_bolt(x, y, z) else {
                    return;
                };
                self.ent[p].f68 = 10;
                self.ent[p].f69 = 0;
                self.snd(15, body_id as usize);
                p
            }
            2 => {
                let Some(p) = self.mc2_spawn_bolt9(x, y, z) else {
                    return;
                };
                self.ent[p].f68 = 10;
                self.ent[p].f69 = 23;
                self.snd(23, body_id as usize);
                p
            }
            _ => return,
        };
        self.ent[p].f44 = 850; // subSpellIndex
        self.ent[p].row156 = 106;
        let tpos =
            self.mc2_raw_pos(target, ctx)
                .unwrap_or((self.ent[p].x, self.ent[p].y, self.ent[p].z));
        self.mc2_arm_proj(p, br, target, tpos);
        self.ent[p].id24 = body_id;
        self.ent[p].f146 = self.ent[br].f146;
        let z2 = self.ent[p].z.wrapping_add(lift);
        let (px, py) = (self.ent[p].x, self.ent[p].y);
        self.move_relink(p, px, py, z2);
    }

    /// `sub_29A90` (EF:19737) — the body-driven branch machine.
    /// Walks the f54 chain, processes ONLY branches (0xE9): the
    /// pre-roll draws, the 16-way f71 switch, then the positioning
    /// dispatch (LABEL_94). The manual f63 increment here IS the
    /// branch's phase clock (branches have no dispatch of their
    /// own; the world loop skips their f63).
    pub(crate) fn m27_drive(&mut self, body: usize, ctx: &MobCtx) {
        let mut br = self.ent[body].f54 as usize;
        // ⭐ `v34` = retail's `[ebp-0x10]`, a FUNCTION-scope local of
        // `sub_29A90` with exactly ONE writer (NETHERW.EXE 0x4E351,
        // inside the draw-B arm) and TWO readers (0x4E401, 0x4E452),
        // and NO initialiser anywhere in the body. It therefore
        // CARRIES from one branch of the f54 chain to the next.
        let mut v34: u32 = match self.m27_v34_slot.0.take() {
            Some(f) if Self::m27_v34_residue_law() => f,
            _ => Self::m27_v34_seed(),
        };
        while br != 0 {
            let next = self.ent[br].f54 as usize;
            if self.ent[br].tick70 == BRANCH_STATE {
                self.m27_drive_branch(body, br, ctx, &mut v34);
            }
            br = next;
        }
        // The body's own frame re-occupied both dwords: a published
        // same-walk leaving does not survive past this body. (What
        // `sub_29A90` itself leaves — its final `v34` on one dword, its
        // projectile byte on the other — is not modelled; the next
        // body reads the seed.)
        self.m27_v34_slot.2 = None;
    }

    /// A/B toggle for the wander draw's behaviour-row read (dig
    /// 99-17): `MGC_NO_M27_WANDER_OWN_ROW` restores the hardcoded
    /// `BEHAVIOR[103]` the port carried before. Provably neutral on
    /// the corpus (every (5,27) branch, native or imported, carries
    /// row 103), so this is an invented-constant removal, not a
    /// behaviour change.
    fn m27_wander_own_row_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_WANDER_OWN_ROW").is_none())
    }

    fn m27_wander_row(&self, br: usize) -> i16 {
        if Self::m27_wander_own_row_law() {
            BEHAVIOR[self.ent[br].row156 as usize].v_30
        } else {
            BEHAVIOR[103].v_30
        }
    }

    /// A/B toggle for the M27 BRANCH `v34` CARRY law (dig 99-11):
    /// set `MGC_NO_M27_V34_CARRY` to restore the pre-2026-09-04 body,
    /// which re-seeded `v34` to remc2's invented `0x1000002B` for
    /// EVERY branch instead of carrying it down the chain.
    fn m27_v34_carry_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_CARRY").is_none())
    }

    fn m27_v34_seed() -> u32 {
        if !Self::m27_v34_carry_law() {
            return 0x1000_002B;
        }
        static V: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("MGC_M27_V34_SEED")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(V34_ENTRY)
        })
    }

    /// ⭐⭐⭐ BOTH FLAG WRITES ARE **BYTE** WRITES INTO A DWORD SLOT.
    /// `88 75 f4` (file 0x4F85E, `MOV r/m8,r8`) and `c6 45 f4 01`
    /// (0x4F8F3, `MOV r/m8,imm8`) touch ONLY byte 0 — and so does
    /// `sub_2AF10`'s own read, `80 7d f4 00` (0x4F920, `CMP r/m8,imm8`).
    /// `sub_29A90` then reads the slot as a FULL DWORD (`8b 55 f0`,
    /// 0x4E401) and tests it three ways: `test edx,edx` and the SIGNED
    /// `83 fa 04  cmp edx,4` (0x4E43F) see all four bytes, while only
    /// `f6 45 f0 01  test byte [ebp-0x10],1` (0x4E452) — the parity —
    /// sees the byte the mover wrote. So a turn-search decides the
    /// PARITY BIT and nothing else: the upper three bytes stay
    /// residue, which is why the `!= 0` / `> 4` arms must keep
    /// `V34_ENTRY`'s magnitude. (Writing a bare 0/1 instead measured
    /// mc2l22 END -> horizon 46488 and mc2l24 devs 0 -> 2: a `v34`
    /// of 0 sends the branch into the wizard-scan arm retail never
    /// takes, and a `v34` of 1 stops it stamping `f71 = 4`.)
    fn m27_v34_byte(flag: u32) -> u32 {
        (Self::m27_v34_seed() & !0xFF) | flag
    }

    /// A/B toggle for the M27 `v34` PAD-RESIDUE law (round 127):
    /// `MGC_NO_M27_V34_PAD_RESIDUE` restores (a) the unsigned `> 4`
    /// test and (b) the mover's flag write over the SEED's upper
    /// bytes, and (c) silences the (10,34) teleporter pad's stack
    /// leaving (`World::mc2_portal_tick`).
    ///
    /// ⭐⭐⭐ THE `> 4` TEST IS SIGNED. NETHERW.EXE 0x4E43F `83 fa 04
    /// cmp edx,4` is followed by 0x4E442 `7e 04 jle` — a SIGNED
    /// branch — so a residue with bit 31 set (a sign-extended
    /// negative word) is `!= 0`, is NOT `> 4`, and its low bit alone
    /// decides the wander draw. The port compared as `u32`, which
    /// sent every such residue into the `f71 = 4` arm.
    ///
    /// ⭐⭐⭐ THE (10,34) TELEPORTER PAD LEAVES ITS OWN `y` IN THE SLOT.
    /// `sub_35390` (file 0x59B90: 4 pushes + `sub esp,0x10`, frame
    /// W-8..W-36) ends every ALIVE tick with `push esi / call
    /// getTerrainAlt_10C40` (0x59D13-14, arg at W-40, return at W-44);
    /// `getTerrainAlt` (0x35440: `push ebp / mov ebp,esp`, W-48) then
    /// pushes `movsx eax, WORD [edx+2]` — the pad's OWN `position.y`,
    /// sign-extended — at **W-52** (0x3544A `50`), the exact dword
    /// `sub_29A90` reads as `[ebp-0x10]` on the 0xD9 path. Nothing
    /// dispatched after the pad on mc2l24 (idle risers, idle switches
    /// with a mis-aligned phase, the 0xE9/0xEA chain members — which
    /// the walk never calls at all, `str_D4C48ar[5]` rows 233/234 carry
    /// a NULL handler) reaches that depth, so a hydra body walked after
    /// the pad reads `sext16(pad.y)`. mc2l24 t=33370: two hydras are
    /// born in one tick (bodies 5 and 95, the pad at slot 66 between
    /// them); on their first tick the branches with `f63 & 7 == 0`
    /// (7 and 23 of body 5, 135 and 159 of body 95) all enter the
    /// state-1 arm on the residue — retail stamps `f71 = 4` on 7 and
    /// 23 (pre-walk residue, positive) and leaves 135/159 in state 1
    /// with the wander draw taken (pad y 43392 → −22144: nonzero,
    /// even, not > 4). The port stamped all four.
    /// The pad's EXPIRE tick (`life` 1 → 0) ends with
    /// `PrepareEventSound(idx, -1, 20)` instead — three args at
    /// W-40..W-48, return address at W-52 = linear 0x216415.
    fn m27_v34_pad_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_PAD_RESIDUE").is_none())
    }

    /// The mover's flag write is a BYTE store into whatever the slot
    /// holds: keep the LIVE residue's upper three bytes (the pad's
    /// sign-extended `y`, or the seed when nothing is published).
    fn m27_v34_flag(&self, flag: u32) -> u32 {
        if Self::m27_v34_pad_law() {
            let cur = self.m27_v34_slot.0.unwrap_or_else(Self::m27_v34_seed);
            (cur & !0xFF) | flag
        } else {
            Self::m27_v34_byte(flag)
        }
    }

    /// `cmp edx,4 / jle` (0x4E43F-42): signed.
    pub(crate) fn m27_v34_gt4(v34: u32) -> bool {
        if Self::m27_v34_pad_law() {
            (v34 as i32) > 4
        } else {
            v34 > 4
        }
    }

    /// The residue value the (10,34) pad's expire tick leaves: the
    /// return address of `call PrepareEventSound_6E450` at file
    /// 0x59C10 (next instruction 0x59C15 → linear 0x216415).
    pub(crate) const M27_V34_PAD_EXPIRE: u32 = 0x0021_6415;

    /// `World::mc2_portal_tick`'s seam: publish what `sub_35390` left
    /// at W-52 for the rest of this walk.
    pub(crate) fn m27_v34_publish_pad(&mut self, i: usize, expired: bool) {
        if !Self::m27_v34_pad_law() {
            return;
        }
        let v = if expired {
            Self::M27_V34_PAD_EXPIRE
        } else {
            self.ent[i].y as i16 as i32 as u32
        };
        self.m27_v34_publish_word(v);
    }

    /// A handler left `v` on the 0xD9-path dword (W-52) and a
    /// positive even stack/pointer value one frame deeper (W-64).
    pub(crate) fn m27_v34_publish_word(&mut self, v: u32) {
        self.m27_v34_slot.0 = Some(v);
        self.m27_v34_slot.1 = true;
        self.m27_v34_slot.2 = None;
        self.m27_v34_slot.3 = None;
    }

    /// A handler left an entity-pool or stack POINTER on the slot:
    /// positive, even, far above 4 — the seed's own class, so the
    /// seed stands in (`None`).
    pub(crate) fn m27_v34_publish_pointer(&mut self) {
        self.m27_v34_slot.0 = None;
        self.m27_v34_slot.1 = false;
        self.m27_v34_slot.2 = None;
        self.m27_v34_slot.3 = None;
    }

    /// A/B toggle for the M27 `v34` BUILDING-RESIDUE law (round 128):
    /// `MGC_NO_M27_V34_BUILDING_RESIDUE` silences the (10,45) village
    /// building's stack leaving (`Gen::mc2_house_tick`).
    ///
    /// ⭐⭐⭐ EVERY (10,45) ACTION-0x34 TICK ENDS WITH `getTerrainAlt(
    /// &position)`, AND ITS HELPER PUSHES THE WALK'S ESI ON W-64.
    /// `AddHouse0A_2D_38330` (file 0x5CB30, `53 56 57 55 89 e5`, no
    /// locals) funnels every arm — idle, hit, dead-to-53, claim — into
    /// its tail at 0x5CD9C: `lea eax,[ebx+0x4c] / push eax` (W-24) /
    /// `call getTerrainAlt_10C40` (return W-28). `getTerrainAlt`
    /// (0x35440) is `push ebp` (W-32), `push sext(y)` (W-36), `push
    /// sext(x)` (W-40), `call 0xDA460` (return W-44), and the
    /// interpolator's prologue is `push ebp / mov ebp,esp / push ebx /
    /// push ecx / push edx / push esi / push edi` = W-48..W-68 — so
    /// **W-52 = EBX = the building's own record pointer** (the 0xD9
    /// path's dword: a positive even address, the seed's class) and
    /// **W-64 = ESI** (the 0xD8/0xDA/0xDB dword).
    /// ⭐⭐⭐ ESI IS THE WALK'S OWN 29. `UpdateEntities_57730` (0x7BF30)
    /// zeroes esi at 0x7C1AE for its 29-roster `sub_12500` pre-walk
    /// loop (`inc esi / cmp esi,0x1d / jl`, 0x7C1E0-E4) and never
    /// writes it again until AFTER the walk (`mov si,[eax+0x3654a]`,
    /// 0x7C2B1); every callee in between saves and restores it
    /// (Watcom's esi is callee-saved: `sub_68BF0` 0x8D3F1, `sub_159E0`
    /// 0x3A1E1, `sub_12780` 0x36F81, every handler prologue), so each
    /// entity handler is entered with ESI = 29. The building tick
    /// keeps it — `CompareEvent08_38B00` (0x5D300..0x5D367) and the
    /// claim/sound/sprite callees never load esi — EXCEPT on two arms
    /// that load a pointer into it: the non-lethal hit with an
    /// occupant to pop (`mov esi,[0x1a3e4]` = `Entities_EA3E4[0]` at
    /// 0x5CBDD, run whenever `dword_0x10 > 2`) and the periodic
    /// population spawn's `lea esi,[ebx+0x4c]` (0x5CD5D, inside the
    /// `rand % minSpeed >` branch). `SetMaxDistance_5C8D0` saves esi
    /// (0x810D1) and its `sub_583F0` leaves an EVEN saved ebp on W-64
    /// that the tail's ESI push then overwrites.
    /// So a hydra body on the 0xDA path walked after a quiet village
    /// building reads **29**: nonzero, `> 4`, ODD — `f71 = 4` stamped
    /// and the wander draw SKIPPED. mc2l22's body 98 sits behind
    /// thirteen (10,45) buildings (the last at slot 93), and all 19
    /// registered boundaries (53911..55088) are exactly that outcome
    /// on state-0 head-of-chain branches; the port's even seed
    /// stamped 4 and drew.
    fn m27_v34_building_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_BUILDING_RESIDUE").is_none())
    }

    /// The walk's ESI: `UpdateEntities_57730`'s pre-walk chain counter
    /// after its 29 rosters (`cmp esi,0x1d`).
    pub(crate) const M27_V34_WALK_ESI: u32 = 29;

    /// `Gen::mc2_house_tick`'s seam: `esi_pointer` = this tick loaded
    /// a record pointer into ESI (the hit-with-occupants arm or the
    /// population spawn), so the deeper dword holds an even address of
    /// the seed's class instead of the walk's 29.
    pub(crate) fn m27_v34_publish_building(&mut self, esi_pointer: bool) {
        if !Self::m27_v34_building_law() {
            return;
        }
        // W-52: the terrain helper's saved EBX = the building's record
        // pointer.
        self.m27_v34_slot.0 = None;
        self.m27_v34_slot.1 = false;
        // W-64: its saved ESI.
        self.m27_v34_slot.2 = if esi_pointer {
            None
        } else {
            Some(Self::M27_V34_WALK_ESI)
        };
        self.m27_v34_slot.3 = None;
    }

    /// A/B toggle for the M27 `v34` FIRE-RESIDUE law (dig W19, round
    /// 144): `MGC_NO_M27_V34_FIRE_RESIDUE` silences the (10,0) fire
    /// tick's stack leaving (`Gen::mc2_fire_tick`).
    ///
    /// ⭐⭐⭐ A LIVE (10,0) FIRE BETWEEN THE LAST BUILDING AND THE BODY
    /// PUTS A POINTER BACK ON THE SLOT — AND IT IS THE *ACTING* ARM
    /// ONLY. `sub_30D50` (file 0x55550, prologue `53 56 57 55 89 e5 83
    /// ec 04` ⇒ `ebp = W-20`, `esp = W-24` — one dword of locals, so
    /// every frame below it sits FOUR BYTES LOWER than the (10,45)
    /// house's) has three exits:
    /// * the fuse arm `if (dword_0x10_16 & 3) dword_0x10_16--;`
    ///   (EF:22726) — NO call at all, so the previous handler's
    ///   leaving survives untouched;
    /// * the reap arm `DisableEntityDrawing04_57F10(a1x)` (file
    ///   0x7C710: `55 89 e5 8b 45 08 80 48 0d 04 5d c3` — a LEAF,
    ///   deepest write W-36) — also leaves the slot alone;
    /// * the acting arm, whose LAST TWO calls are
    ///   `sub_580E0(&position, alt, 0, 0, word_0x2C_44)` and
    ///   `sub_585A0(a1x)`. `sub_585A0` (file 0x7CDA0: `53 55 89 e5`, one
    ///   arg, and it CALLS NOTHING — `mov edx,[ebp+0xc]` / two byte
    ///   compares / `ret`) bottoms out at W-40, so the last writer of
    ///   W-64 is `sub_580E0` (file 0x7C8E0, `53 56 57 55 89 e5`): five
    ///   args at W-28..W-44, return at W-48, then `push ebx` W-52,
    ///   `push esi` W-56, `push edi` W-60, **`push ebp` W-64** — the
    ///   caller's own frame pointer, a 4-aligned stack address.
    ///   W-52 takes its saved EBX, which `8b 5d 14` loaded with
    ///   `a1x` itself. Both dwords are therefore POSITIVE, EVEN and
    ///   far above 4 — the seed's class — so a body walked after an
    ///   acting fire keeps `f71 = 4` AND TAKES the wander draw.
    ///
    /// ⭐⭐ A HANDLER'S OWN LOCAL-FRAME SIZE RE-HOMES WHICH REGISTER
    /// LANDS ON THE `v34` SLOT: the fire's `sub esp,4` is the only
    /// difference from the (10,45) house, and it is what puts
    /// `sub_580E0`'s saved EBP on W-64 where the house leaves ESI.
    ///
    /// WITNESS (mc2l19, body 5 behind the (10,45) house at slot 3 with
    /// the (10,0) smoke column at slots 1/2/4): 20 state-0 gate
    /// openings read the house's 29 in the port; retail skips the
    /// wander draw on SEVENTEEN of them — every one with slot 4
    /// already recycled to class 0 — and TAKES it on the three where
    /// slot 4 is still a live (10,0) that ran its acting arm
    /// (t=12,665 and t=12,949 on the `life-- == -1` last acting tick,
    /// t=13,388 with `life` 5). Those three are exactly the take's
    /// three divergent boundaries (`slot N heading` + `rand`, retail
    /// two LCG steps to the port's one). **mc2l19 CERTIFIED: 1
    /// segment, 0 deviations, horizon END.**
    fn m27_v34_fire_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_FIRE_RESIDUE").is_none())
    }

    /// `Gen::mc2_fire_tick`'s seam: the acting arm only.
    pub(crate) fn m27_v34_publish_fire(&mut self) {
        if Self::m27_v34_fire_law() {
            self.m27_v34_publish_pointer();
        }
    }

    /// A/B toggle for the M27 `v34` LEVIATHAN-RESIDUE law (dig W21,
    /// round 145): `MGC_NO_M27_V34_LEVIATHAN_RESIDUE` silences the
    /// (5,23) dweller's patrol-tick stack leaving
    /// (`Gen::m23_tick` sub-state 0).
    ///
    /// ⭐⭐⭐ A QUIET (5,23) PATROL TICK LEAVES ITS OWN **TARGET YAW**
    /// ON THE 0xDA DWORD AND ITS **PRE-RETRY YAW** ON THE 0xD9 ONE.
    /// `sub_27950` (file 0x4C150, `53 56 57 55 89 e5 83 ec 04` — the
    /// fire's frame shape, `ebp = W-20`, `esp = W-24`) opens with
    /// `push ebx / call sub_1B8C0` (0x4C15C-5D, the move core, file
    /// 0x400C0, same prologue ⇒ `ebp = W-48`, `esp = W-52`) and closes
    /// with `push ebx / call sub_28110` (0x4C30C-0D, the post pass).
    /// Inside the move core every COMMIT arm ends with the turn
    /// `sub_58350(yaw, target_yaw, v_4, v_2)` — four pushes at
    /// W-56/W-60/**W-64**/W-68 (0x401A1..0x401BA, 0x4023D..0x40246 and
    /// the two retry twins) — so the THIRD push,
    /// `xor eax,eax / mov ax,0x20(%ebx)` (0x401AE), parks the record's
    /// **zero-extended `+0x20`** on the 0xD8/0xDA/0xDB dword. And the
    /// retry head stores `xor eax,eax / mov ax,0x1c(%ebx) /
    /// mov %eax,-0x4(%ebp)` (0x40274-7A) — the move core's ONE local,
    /// `[ebp-4] = W-52`, the 0xD9 dword — so a move that had to rotate
    /// (result 3 or 4) also leaves its **pre-retry `+0x1C`** there,
    /// while results 1 and 2 never touch W-52 at all.
    ///
    /// ⭐⭐ WHAT THE LATER ARMS DO IS OVERWRITE BOTH WITH POINTERS.
    /// `sub_28420` (the node validity test, 0x4CC20) is a LEAF
    /// (`53 55 89 e5 … 5d 5b c3`, deepest write W-40) and
    /// `sub_27FE0` (the mode setter, 0x4C7E0, `55 89 e5 … 5d c3`)
    /// bottoms out at W-48, so neither disturbs the slot. The ones
    /// that do:
    /// * the `byte_0x46_70 == 2` re-aim (0x4C2CB-E6, gated on
    ///   `!(0x3E(%ebx) & 3)`) ends with
    ///   `push edi / push esi / call EuclideanDistXY_58490` (0x4C2D8-DE,
    ///   return W-36), whose `push ebx` W-40 / `push ebp` W-44 /
    ///   `push eax` W-48 / `call isqrt 0x96F7A` (return **W-52**) is
    ///   followed by the isqrt's `push ebp` W-56 / `push ebx` W-60 /
    ///   `push ecx` **W-64** — and ECX is still `0xc(%ebp)`, the
    ///   dweller's own `&position`. Both dwords become positive even
    ///   addresses, the seed's class.
    /// * the `== 1` arm always calls something deep — `sub_28000`
    ///   (the node hunt, 0x4C800) pushes two position pointers at
    ///   W-52/W-56 for `sub_584D0` whose `push ebx` lands on W-64, and
    ///   the no-node fallback is `push esi / call getTerrainAlt_10C40`
    ///   (0x4C29C-9D) whose interpolator parks its saved EBP on W-52
    ///   and the position pointer (EDX) on W-64.
    /// * `sub_28110`'s own arms (post): a wizard-sourced hit, a death,
    ///   or the `!(0x3E & 0x1F)` owner hunt. All three END on the
    ///   4-argument mode setter (0x4C9B5-CA, 0x4C9DE-F5, 0x4CAAE-BF)
    ///   whose pushes are `$0` W-52, `$0` W-56, `$0xBA` W-60,
    ///   `%ebx` W-64 — **ZERO on the 0xD9 dword** and the record
    ///   pointer on the 0xDA one — except the owner hunt's two early
    ///   exits (no target, or out of range), which bottom out on
    ///   pointer-class values instead.
    ///
    /// ⚠ SCOPED TO SUB-STATE 0. `sub_27B20` (action 185, file 0x4C320)
    /// is `53 56 55 89 e5 83 ec 04` — THREE pushed registers, not four
    /// — so every frame under it sits four bytes higher and none of
    /// this arithmetic carries. The other m23 sub-states are left
    /// alone.
    ///
    /// WITNESS (mc2l21, hydra body at slot 2 with the dweller at slot
    /// 1 immediately ahead of it in the walk — the body's `+0x45`
    /// ALTERNATES 0xD9/0xDA, so one take exercises both dwords).
    /// **21 of 21.** Twenty-one state-0 head-of-chain branches open the
    /// `f63 & 7` gate on a residue over the take — fifteen with the
    /// body on 0xDA (W-64), six on 0xD9 (W-52) — and the per-branch
    /// draw-count oracle (retail's recorded per-record LCG: one step =
    /// draw #A alone, the wander SKIPPED ⇒ odd; two = TAKEN ⇒ even)
    /// scores this model 21/21 where the even seed scored 13/21. The
    /// eight it gets right and the seed does not are exactly the
    /// take's eight body-2 divergent boundaries (t=11,460 / 12,402 /
    /// 12,648 / 12,680 / 12,720 / 12,840 / 12,908 / 12,940 —
    /// `heading` + `rand` on the branch, retail one LCG step to the
    /// port's two). `--segmented --classify`: **11 segments / 10
    /// deviation heads / horizon 11,459 → 3 / 2 / 14,867.** Decompile:
    /// EF:18068 (`sub_27950`), EF:8754 (`sub_1B8C0`), EF:18462
    /// (`sub_28110`), EF:18614 (`sub_28420`), EF:18388 (`sub_27FE0`),
    /// EF:18400 (`sub_28000`), EF:40696 (`sub_58350`) — banner lines,
    /// resolved by address.
    fn m27_v34_leviathan_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_LEVIATHAN_RESIDUE").is_none())
    }

    /// `Gen::m23_tick` sub-state 0's seam. `yaw0` is `+0x1C` as the
    /// move core entered, `code` its result, and `deep`/`post` say
    /// which later arm (if any) ran last — see
    /// [`Gen::m27_v34_leviathan_law`]. `post`: 0 = the post pass made
    /// no call, 1 = it ended on the 4-argument mode setter, 2 = it
    /// ended on a pointer-class call.
    pub(crate) fn m27_v34_publish_leviathan(
        &mut self,
        i: usize,
        yaw0: u16,
        code: u8,
        deep: bool,
        post: u8,
    ) {
        if !Self::m27_v34_leviathan_law() {
            return;
        }
        self.m27_v34_slot.1 = false;
        self.m27_v34_slot.3 = Some(i as u16);
        match post {
            1 => {
                // `sub_27FE0(a1x, 0xBA, 0, 0)`: `$0` on W-52, the
                // record pointer on W-64.
                self.m27_v34_slot.0 = Some(0);
                self.m27_v34_slot.2 = None;
            }
            2 => {
                self.m27_v34_slot.0 = None;
                self.m27_v34_slot.2 = None;
            }
            _ if deep => {
                self.m27_v34_slot.0 = None;
                self.m27_v34_slot.2 = None;
            }
            _ => {
                self.m27_v34_slot.0 = if code >= 3 { Some(yaw0 as u32) } else { None };
                self.m27_v34_slot.2 = Some(self.ent[i].f34 as u32);
            }
        }
    }

    /// `MGC_NO_MC2_M27_V34_GUARD_RESIDUE` — see
    /// [`crate::engine::features::no_mc2_m27_v34_guard_residue`] for
    /// the law and the NETHERW.EXE citation. `Gen::m15_brain`'s seam:
    /// `guard` = which arm of `sub_23C40` ran, `engaged` = the tick
    /// ended in action 122 (the `sub_24100` engage pose ran last).
    pub(crate) fn m27_v34_publish_guard(&mut self, i: usize, guard: GuardV34, engaged: bool) {
        if crate::engine::features::no_mc2_m27_v34_guard_residue() {
            return;
        }
        if matches!(guard, GuardV34::Silent) {
            // The lethal arm stores action 124 and returns without a
            // call: both dwords survive.
            self.m27_v34_transparent(i);
            return;
        }
        // The W-64 view this handler was entered with (the `[ebp-4]`
        // byte store keeps its upper three bytes).
        let prior = match self.m27_v34_slot.3 {
            Some(p) if self.m27_v34_broken(p as usize, i) => None,
            _ => self.m27_v34_slot.2,
        };
        let (w52, w64) = match guard {
            // `sub_24100` (file 0x48900): `push ebp` W-52, its
            // `call 0x6e4d0` return address (linear 0x205142) W-64.
            _ if engaged => (None, None),
            // `sub_1EEE0` (file 0x436E0): `push ebp` W-52, then
            // `movswl 0xc(row)` pushed on W-64.
            GuardV34::Hit => (
                None,
                Some(BEHAVIOR[self.ent[i].row156 as usize].v_12 as i32 as u32),
            ),
            // `sub_581E0`'s `push ebx` (the guard pointer) on W-52 and
            // its `cwtl` dy push on W-64 for the LAST tested candidate
            // (roster order — not modelled: seed class), then
            // `sub_582B0`'s `push ebp` on W-52 again.
            GuardV34::Scanned => (None, None),
            // `sub_24190` (file 0x48990): `push esi` = the brain's
            // arm selector, ZERO on this arm, lands on W-52; its
            // `[ebp-4]` is W-64 and takes a BYTE (0x48B07 / 0x48B71).
            GuardV34::Wander(None) => (Some(0), prior),
            GuardV34::Wander(Some(found)) => (
                Some(0),
                Some((prior.unwrap_or_else(Self::m27_v34_seed) & !0xFF) | found as u32),
            ),
            GuardV34::Silent => unreachable!(),
        };
        self.m27_v34_slot.0 = w52;
        self.m27_v34_slot.1 = false;
        self.m27_v34_slot.2 = w64;
        self.m27_v34_slot.3 = Some(i as u16);
    }

    /// `MGC_NO_MC2_M27_V34_SPHERE_RESIDUE` — see
    /// [`crate::engine::features::no_mc2_m27_v34_sphere_residue`].
    /// `Gen::ball_tick`'s seam, the (10,39) ball's MOVING arm only:
    /// `esi` = this tick's `getTerrainAlt` (the full sampler word) when
    /// the arm ended on `SetManaSphereColorAndRot_36920`, `None` when
    /// the decaying gate skipped that call (unmodelled: seed class).
    pub(crate) fn m27_v34_publish_sphere(&mut self, esi: Option<u32>) {
        if crate::engine::features::no_mc2_m27_v34_sphere_residue() {
            return;
        }
        // W-52: the pushed sphere record pointer.
        self.m27_v34_slot.0 = None;
        self.m27_v34_slot.1 = false;
        // W-64: `SetManaSphereColorAndRot_36920`'s saved ESI.
        self.m27_v34_slot.2 = esi;
        self.m27_v34_slot.3 = None;
    }

    /// `MGC_NO_MC2_M27_V34_BLAST_SOUND_RESIDUE` — see
    /// [`crate::engine::features::no_mc2_m27_v34_blast_sound_residue`].
    /// `Gen::mc2_blast23_tick`'s seam, the burst tick only (right after
    /// its `PrepareEventSound_6E450(id, -1, 24)`).
    pub(crate) fn m27_v34_publish_blast_sound(&mut self, i: usize, ctx: &MobCtx) {
        if crate::engine::features::no_mc2_m27_v34_blast_sound_residue() {
            return;
        }
        let (bx, by, fl) = {
            let e = &self.ent[i];
            (e.x, e.y, e.flags)
        };
        // `sub_584D0` (file 0x7CCD0): i16 deltas, squared, summed; the
        // `ja` at 0x92CE4 is unsigned.
        let dx = bx.wrapping_sub(ctx.px) as i16 as i32;
        let dy = by.wrapping_sub(ctx.py) as i16 as i32;
        let sq = (dx * dx).wrapping_add(dy * dy) as u32;
        // W-52: `sub_581E0`'s full EAX (the atan2 word) — not modelled.
        self.m27_v34_slot.0 = None;
        self.m27_v34_slot.1 = false;
        // W-64: `[ebp-0x14]` = `EuclideanDistXYZ_58490(listener, blast)`,
        // stored only past the silent-emitter and hearing-range exits.
        self.m27_v34_slot.2 = if fl & 0x80 == 0 && sq <= 0x0900_0000 {
            Some(crate::mc2::morph::dist2d(ctx.px, ctx.py, bx as i32, by as i32) as u32)
        } else {
            None
        };
        self.m27_v34_slot.3 = None;
    }

    /// `MGC_NO_MC2_M27_V34_WYVERN_RESIDUE` — see
    /// [`crate::engine::features::no_mc2_m27_v34_wyvern_residue`].
    /// `Gen::m16_tick` action 130's seam: `quiet` = the call-free hit /
    /// death arms; otherwise `w52` is the 0xD9 dword the normal arm
    /// has left so far (`None` = a pointer or unmodelled), and the
    /// 0xDA dword is not modelled.
    pub(crate) fn m27_v34_publish_wyvern(&mut self, i: usize, w52: Option<u32>, quiet: bool) {
        if crate::engine::features::no_mc2_m27_v34_wyvern_residue() {
            return;
        }
        if quiet {
            self.m27_v34_transparent(i);
            return;
        }
        self.m27_v34_slot.0 = w52;
        self.m27_v34_slot.1 = false;
        self.m27_v34_slot.2 = None;
        self.m27_v34_slot.3 = Some(i as u16);
    }

    /// The ring iterator's handle: `sub_10080` (file 0x34880) returns
    /// the LOWEST index of its 100-row table whose `+8` is −1, and every
    /// user releases its handle (`sub_10100`, file 0x34900) before
    /// returning — so a handler that opens one always holds 1.
    pub(crate) const M27_V34_RING_HANDLE: u32 = 1;

    /// `MGC_NO_MC2_M27_V34_METEOR_RESIDUE` — see
    /// [`crate::engine::features::no_mc2_m27_v34_meteor_residue`].
    /// `Gen::mc2_meteor_tick`'s seam, acting arm only (the expiry arm
    /// is `DisableEntityDrawing04_57F10`, a leaf: transparent — and the
    /// port models it as a break, which only restores the seed).
    pub(crate) fn m27_v34_publish_meteor(&mut self, i: usize) {
        if crate::engine::features::no_mc2_m27_v34_meteor_residue() {
            return;
        }
        self.m27_v34_slot.0 = None;
        self.m27_v34_slot.1 = false;
        self.m27_v34_slot.2 = Some(Self::M27_V34_RING_HANDLE);
        self.m27_v34_slot.3 = Some(i as u16);
    }

    /// A/B toggle for the M27 `v34` SWITCH-RESIDUE law (round 127):
    /// `MGC_NO_M27_V34_SWITCH_RESIDUE` silences the class-11 switch
    /// handlers' stack leavings (`World::mc2_switch_probe` /
    /// `mc2_switch_repeating`).
    ///
    /// ⭐⭐⭐ THE (11,0..3) SWITCH PROBE LEAVES ITS OWN `y` TOO. All four
    /// handlers (`sub_6F030`/`sub_6F070`/`sub_6F0B0`/`sub_6F100`, file
    /// 0x93830/0x93870/0x938B0/0x93900) are `push ebx / push ebp / mov
    /// ebp,esp` (ebp = W-12) and call `InitSwitchChainZaxisAndSound_
    /// 6F850` with two args (W-16/W-20, return W-24); the probe
    /// (0x94050) pushes ebx/esi/ebp (ebp = W-36). Past its `f63 & 7`
    /// gate it walks the tick-top class-3 roster `dword_38519`, and for
    /// every model-0 member calls `CompareAxisWithShift_10750` (args
    /// W-40/W-44, return W-48) whose first push (0x34F50 `53`) drops
    /// the MEMBER POINTER on W-52; when no member matches, the walk
    /// ends with `getTerrainAlt_10C40(&position)` (EF:44870 — arg
    /// W-40, return W-44, `push ebp` W-48) whose `movsx y` push lands
    /// on W-52 LAST. So: a trip leaves a pointer (the seed's class), a
    /// quiet probe leaves `sext16(switch.y)` — with or without a live
    /// human. mc2l24 walk 41397→41398: switch 75 (phase 40, y 24448 →
    /// positive even) sits between the slot-66 pad and body 95, so the
    /// body's aligned branch 124 read 24448, not the pad's −22144, and
    /// retail stamped `f71 = 4`; switch 79 (y 28032) does the same at
    /// 41681 and 41713.
    /// The REARM arm (`dword_0x10_16 != 0`, EF:54419) calls `sub_6F8E0`
    /// (0x940E0: four pushes, ebp = W-40) which, for the first player
    /// row, does `push edi / push edx / call CompareAxisWithShift` at
    /// 0x94108 — the return address 0x9410D (linear 0x25090D, ODD) lands
    /// on W-52 every rearm tick, no phase gate.
    fn m27_v34_switch_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_SWITCH_RESIDUE").is_none())
    }

    /// The return address `sub_6F8E0`'s player loop leaves on the
    /// slot: `call 0x34f50` at file 0x94108 → 0x9410D → linear
    /// 0x25090D.
    pub(crate) const M27_V34_SWITCH_REARM: u32 = 0x0025_090D;

    /// `World::mc2_switch_probe`'s seam (actions 0..=3 only — the
    /// (11,12)/(11,31) marker handlers reach the probe from a
    /// different frame): `tripped` = a roster member matched.
    pub(crate) fn m27_v34_publish_switch_probe(&mut self, i: usize, tripped: bool) {
        if !Self::m27_v34_switch_law() || self.ent[i].tick70 > 3 {
            return;
        }
        if tripped {
            self.m27_v34_publish_pointer();
        } else {
            let y = self.ent[i].y as i16 as i32 as u32;
            self.m27_v34_publish_word(y);
        }
    }

    /// `World::mc2_switch_repeating`'s rearm seam.
    pub(crate) fn m27_v34_publish_switch_rearm(&mut self) {
        if Self::m27_v34_switch_law() {
            self.m27_v34_publish_word(Self::M27_V34_SWITCH_REARM);
        }
    }

    /// A/B toggle for the M27 SCAN-ROSTER-LIFE law (round 127):
    /// `MGC_NO_M27_SCAN_ROSTER_LIFE` puts the out-of-pool human back
    /// into `sub_2A6F0`'s walk while dead.
    ///
    /// ⭐⭐ THE BRANCH'S WIZARD SCAN WALKS `dword_38519`, THE TICK-TOP
    /// CLASS-3 ROSTER (EF:20466), and that roster is built with
    /// `life_0x8 >= 0` (EF:39975) — a DEAD wizard is not on it. Retail's
    /// human is a pool record and takes that test for free; ours is a
    /// ctx pose and must take the roster's ENTRY condition here
    /// (`pdead_top`, the value the roster was built with — not
    /// `pdead`, the live sign). No invisibility test: `sub_2A6F0` has
    /// none. mc2l24 walk 35492→35493: the human has been a corpse
    /// since ≈35145 (action 3, life −347); body 5's branch 61 draws
    /// `v34 = 0` (draw #B & 7) on an aligned phase and scans — retail
    /// finds nobody and stays in state 1 (speed 112 → 128), the port
    /// locked the corpse (`f71` 2, target 116) and entered the whip.
    fn m27_scan_life_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_SCAN_ROSTER_LIFE").is_none())
    }

    /// A/B toggle for the M27 `v34` AIM-RESIDUE law (round 128):
    /// `MGC_NO_M27_V34_AIM_RESIDUE` restores the pre-2026-09-10 body,
    /// where the 0xDA aim pass merely CLOBBERED the slot back to the
    /// seed (even, > 4) — which stamped `f71 = 4` on every state-0
    /// branch that read it.
    ///
    /// ⭐⭐⭐ THE 0xDA AIM PASS LEAVES `target.y − body.y` ON THE v34
    /// SLOT. `sub_29710` (file 0x4DF10, `53 56 57 55 89 e5 83 ec 04`,
    /// ebp = W-20, esp = W-24) runs, on `f63 & 0x1F == 0` with a live
    /// target and a mover code other than 4: `sub_581E0` (tan2,
    /// 0x4DFD9), `sub_582B0` (0x4DFEE), `sub_2AED0` (the pose,
    /// 0x4E015) and LAST `sub_583F0_distance_3d` (0x4E02F: `push esi`
    /// W-28, `push eax` W-32, return W-36). `sub_583F0` (0x7CBF0:
    /// `push ebx/esi/ebp` W-40..W-48) computes `ax = t.x − b.x`,
    /// `bx = b.y`, `ax = t.y`, `sub eax,ebx` (0x7CC0B) and keeps that
    /// 32-bit difference in EBX (0x7CC0D `mov ebx,eax`), pushes the
    /// squared sum (W-52) and calls the isqrt at 0x96F7A (return
    /// W-56), whose prologue is `push ebp` (W-60) / `push ebx`
    /// (**W-64**) / `push ecx` / `push edx`. W-64 is `sub_29A90`'s
    /// `[ebp-0x10]` on the 0xDA path (`m27_v34_residue_law`), and no
    /// later call before the chain machine goes that deep (`sub_2AED0`
    /// is two pushes). The mover ran BEFORE the aim pass and committed
    /// the step, so `b.y` is the body's post-move y; the target's is
    /// its tick-top value for anything walked after the body.
    /// The upper halves of the registers ride along: `eax` was
    /// `lea eax,[ebx+0x4c]` (the body's position pointer) and `ebx`
    /// the body pointer, so the difference is `((rec+0x4c)>>16 −
    /// rec>>16) << 16 + (t.y − b.y)` — the carry term is 0 unless the
    /// record straddles a 64K line (pool base 0x35CE1E on both
    /// recorded runs: slots 76, 466, 856).
    /// mc2l24 walk 41656→41657: body 95 (`f63` 96) steps to y 22852
    /// against the human at y 20940 → −1912: nonzero, NOT > 4, EVEN.
    /// Retail keeps branch 145 in state 1 and takes the wander draw;
    /// the port's clobbered seed (0x1000002A, > 4) stamped `f71 = 4`,
    /// which surfaced one tick later as the 41657→41658 x/y/z/rand
    /// boundary (`b46` is ungraded).
    fn m27_v34_aim_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_AIM_RESIDUE").is_none())
    }

    /// Retail's entity-pool base (`Entities_EA3E4[0]` + 168), recovered
    /// from the recorded `next_0` pointers of mc2l24 and mc2l22 (both
    /// 0x35CE1E). Used ONLY for the 64K-line carry of the aim residue.
    pub(crate) const MC2_RETAIL_POOL_BASE: u32 = 0x0035_CE1E;

    /// `sub_583F0`'s EBX at its isqrt call: the 32-bit `t.y − b.y`
    /// with the pointer halves' difference carried in.
    pub(crate) fn m27_v34_aim_word(body: usize, body_y: u16, target_y: u16) -> u32 {
        let rec = Self::MC2_RETAIL_POOL_BASE.wrapping_add(168 * body as u32);
        let hi = ((rec.wrapping_add(0x4c)) >> 16).wrapping_sub(rec >> 16);
        (hi << 16).wrapping_add((target_y as u32).wrapping_sub(body_y as u32))
    }

    /// A/B toggle for the M27 `v34` SLOT-WRITER law (dig 126O):
    /// `MGC_NO_M27_V34_RESIDUE` restores the pure `V34_ENTRY` constant.
    ///
    /// ⭐ THE FRAME ARITHMETIC. Let `W` be `esp` at the entity walk's
    /// indirect `call` (after the one pushed entity pointer). Every m27
    /// body handler is dispatched from that one site, so `W` is the same
    /// for all of them.
    ///
    /// * `sub_29400` (0xD8, file 0x4DC00 `53 56 57 55 89 e5 83 ec 04`)
    ///   and `sub_29710` (0xDA, file 0x4DF10, same prologue): `ebp = W-20`,
    ///   one pushed arg (`53` at 0x4DE54 / 0x4E071) ⇒ `sub_29A90`'s `ebp
    ///   = W-48` and its `[ebp-0x10]` sits at **W-64**.
    /// * `sub_29670` (0xD9, file 0x4DE70 `53 55 89 e5`, NO locals):
    ///   `ebp = W-12` ⇒ `sub_29A90`'s `[ebp-0x10]` sits at **W-52**.
    /// * `sub_29890` (0xDB, file 0x4E090) delegates to `sub_29670` one
    ///   frame deeper, which puts that path back on **W-64**.
    ///
    /// `sub_2AF10` is called with TWO args from `sub_29670` (`6a 01 53`
    /// at 0x4DEA3, `e8` at 0x4DEA6) and from `sub_29710` (`50 53` at
    /// 0x4DF55, `e8` at 0x4DF57); its own prologue reserves 0x1c
    /// (0x4F710 `53 56 57 55 89 e5 83 ec 1c`), so its `ebp` is
    /// `caller_ebp - 28` and `[ebp-0xc]` lands at `caller_ebp - 40`.
    /// For `sub_29670` (no locals) that is W-52; for `sub_29710`
    /// (`sub esp,4`) it is W-64 — **the v34 slot on both paths, exactly.**
    ///
    /// ⚠ The scan flag is the ONLY writer the port can prove. On the
    /// arms where `sub_2AF10` returns without reaching 0x4F848 the slot
    /// keeps whatever an EARLIER ENTITY's handler left at that absolute
    /// address (`sub_2A6B0`, the other call on both paths, is a LEAF —
    /// file 0x4EEB0..0x4EEEB — so nothing inside the m27 handler itself
    /// touches it). That residue is a REGISTERED FLOOR, not a lane:
    /// see `conformance/known-deviations.json` `mc2l22-hydra-v34-parity`
    /// / `mc2l24-hydra-v34-parity` and dig 126O's 63-boundary scorecard.
    fn m27_v34_residue_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_V34_RESIDUE").is_none())
    }

    fn m27_drive_branch(&mut self, body: usize, br: usize, ctx: &MobCtx, v34c: &mut u32) {
        let mut v37 = 0u8; // projectile-this-tick flag
        let mut v34: u32 = if Self::m27_v34_carry_law() {
            *v34c
        } else {
            0x1000_002B // the leftover-nonzero seed (EF:19783)
        };
        let v5 = self.ent[br].f71;
        self.ent[br].f63 = self.ent[br].f63.wrapping_add(1);

        // Pre-roll (EF:19807-19838).
        if v5 <= 5 {
            let d = self.mc2_rand(br); // DRAW #A
            self.ent[br].f68 = (d % 0x14) as u8;
            self.m27_anchor_branch(body, br, 672);
            self.m27_branch_intake(body, br);
            if self.ent[br].f71 == 1 {
                let d = self.mc2_rand(br); // DRAW #B
                let v6 = d & 7;
                v34 = v6;
                *v34c = v6;
                let v7 = self.ent[br].f40;
                if v7 != 0 {
                    if v6 < 4 {
                        self.ent[br].f40 = 0;
                        self.ent[br].f71 = 2;
                        self.ent[br].f146 = v7;
                        let w = self.ent[br].f56.wrapping_add(22);
                        self.ent[br].f56 = w.min(68);
                    }
                } else if v6 < 4 && self.ent[body].f146 != 0 && self.ent[br].f63 & 7 == 0 {
                    self.ent[br].f71 = 2;
                    self.ent[br].f146 = self.ent[body].f146;
                }
            }
        }

        // The 16-way switch (EF:19839-20185), fall-throughs modeled
        // by consecutive ifs on the CURRENT f71.
        let mut state = self.ent[br].f71;
        if state == 0 {
            // case 0 → arm, fall into case 1.
            let e = &mut self.ent[br];
            e.f146 = 0;
            e.f71 = 1;
            e.f44 = 0;
            e.f56 = 0;
            e.f128 = 16;
            state = 1;
        }
        match state {
            1 => {
                if self.ent[body].f58 != 0 {
                    if self.ent[br].f63 & 7 == 0 {
                        if v34 != 0 {
                            if Self::m27_v34_gt4(v34) {
                                self.ent[br].f71 = 4;
                            }
                        } else if let Some(t) = self.m27_wizard_scan(br, ctx) {
                            self.ent[br].f71 = 2;
                            self.ent[br].f146 = t;
                        }
                    }
                    if self.ent[br].f63 & 7 == 0 && v34 & 1 == 0 {
                        let d = self.mc2_rand(br); // DRAW #C — wander yaw
                        // ⭐ THE BRANCH'S OWN BEHAVIOUR ROW, NOT A
                        // HARDCODED 103 (dig 99-17). NETHERW.EXE
                        // 0x4E462 `8b 8b a0 00 00 00  mov ecx,
                        // DWORD PTR [ebx+0xa0]` loads the BRANCH
                        // record's `dword_0xA0_160` (ebx = ix, the
                        // branch — NOT the body in [ebp+0x14]) and
                        // 0x4E475 `0f bf 79 1e  movsx edi,WORD PTR
                        // [ecx+0x1e]` sign-extends that row's `v_30`
                        // (EF:19877-79 `ix->dword_0xA0_160x->
                        // word_160_0x1e_30`). The baked-in 103 is
                        // correct only because `sub_2AD40` stamps
                        // `&str_D7BD6[103]` on every branch
                        // (EF:20787) and the importer resolves
                        // `ptr_a0` back to the same row — an
                        // invented constant that happens to hold.
                        // ⚠ 0x4E47D is a BARE `f7 f7  div edi` on
                        // the SIGN-EXTENDED word, so a NEGATIVE row
                        // divides as a huge unsigned (remainder =
                        // the whole draw, which `as i32 as u32`
                        // reproduces) and a ZERO row would `#DE`;
                        // `.max(1)` survives only as the crash guard
                        // for that unreachable zero. Whole-take
                        // mc2l22 census BYTE-IDENTICAL on and off.
                        let v30 = self.m27_wander_row(br);
                        let fov = (v30 as i32 as u32).max(1);
                        let w12 = D404C[(self.ent[br].f50 as usize).min(4)][D404C_W12];
                        // EF:19878 stores the wander sum UNMASKED —
                        // the yaw lane legitimately carries >0x7FF
                        // (mc2l24 t=2's (5,27) heading family; the
                        // one masking site in the machine is the
                        // body turn, EF:20967).
                        let base = (self.ent[body].f30 as i32 + w12 as i32 - v30 as i32) as u16;
                        self.ent[br].f30 = base.wrapping_add((d % fov) as u16);
                    }
                }
            }
            2 | 3 => {
                if state == 2 {
                    // case 2 → begin forward whip, fall into 3.
                    let e = &mut self.ent[br];
                    e.f71 = 3;
                    e.f69 = 0;
                    e.f44 = 2;
                    e.f128 = 16;
                }
                // case 3: forward whip, target-tracked (sub_2A7B0
                // validity ≡ mc2_target).
                if let Some(tpos) = self.mc2_target(self.ent[br].f146, ctx) {
                    let sub = self.ent[br].f69;
                    if sub == 0 {
                        if self.ent[br].f126 == 0 {
                            let (bx, by, bz) = {
                                let e = &self.ent[br];
                                (e.x, e.y, e.z)
                            };
                            let yaw = Self::angle_between(bx, by, tpos.0, tpos.1);
                            let pitch = Self::mc2_radix_tan((bx, by, bz), tpos);
                            let (byaw, bpitch) = (self.ent[body].f30, self.ent[body].f32);
                            let e = &mut self.ent[br];
                            e.f69 = 1;
                            e.f44 = 1;
                            e.f128 = 16;
                            e.f30 = yaw;
                            e.f32 = pitch;
                            e.f34 = yaw.wrapping_sub(byaw);
                            e.f36 = pitch.wrapping_sub(bpitch);
                        }
                    } else if sub == 1 {
                        if self.ent[br].f126 == 192 {
                            let e = &mut self.ent[br];
                            e.f44 = 3;
                            e.f69 = 3;
                            e.f26 = 4;
                            v37 = 1;
                        }
                    } else if sub == 3 {
                        v37 = 2;
                        self.ent[br].f26 -= 1;
                        if self.ent[br].f26 == 0 {
                            self.ent[br].f71 = 0;
                            self.ent[br].f26 = 1;
                        }
                    }
                } else {
                    self.ent[br].f71 = 0; // target lost
                }
            }
            4 | 5 => {
                if state == 4 {
                    let e = &mut self.ent[br];
                    e.f71 = 5;
                    e.f69 = 0;
                    e.f44 = 2;
                    e.f128 = 16;
                }
                // case 5: back-swing (no target).
                match self.ent[br].f69 {
                    0 => {
                        if self.ent[br].f126 == 0 {
                            let row = &D404C[(self.ent[br].f50 as usize).min(4)];
                            let (byaw, bpitch) = (self.ent[body].f30, self.ent[body].f32);
                            let e = &mut self.ent[br];
                            e.f69 = 1;
                            e.f44 = 1;
                            e.f128 = -16;
                            e.f34 = row[D404C_W12] as u16;
                            e.f36 = row[D404C_W14] as u16;
                            // EF:19956-57: unmasked sums (see the
                            // wander-yaw note above).
                            e.f30 = e.f34.wrapping_add(byaw);
                            e.f32 = e.f36.wrapping_add(bpitch);
                        }
                    }
                    1 => {
                        if self.ent[br].f126 == -192 {
                            self.ent[br].f69 = 2;
                            self.ent[br].f26 = 2;
                        }
                    }
                    2 => {
                        self.ent[br].f26 -= 1;
                        if self.ent[br].f26 == 0 {
                            let e = &mut self.ent[br];
                            e.f44 = 4;
                            e.f69 = 6;
                            e.f26 = 1;
                        }
                    }
                    5 => {
                        self.ent[br].f26 -= 1;
                        if self.ent[br].f26 <= 4 {
                            self.ent[br].f71 = 0;
                            self.ent[br].f26 = 4;
                        }
                    }
                    6 => {
                        self.snd(17, body); // whip crack (EF:19987)
                        self.ent[br].f26 += 1;
                        if self.ent[br].f26 >= 4 {
                            self.ent[br].f69 = 5;
                        }
                    }
                    _ => {} // → LABEL_94 with the current state
                }
            }
            6 | 7 => {
                if state == 6 {
                    let e = &mut self.ent[br];
                    e.f26 = 0;
                    e.f44 = 2;
                    e.f71 = 7;
                    e.f69 = 0;
                    e.f128 = 80;
                }
                // case 7: segment-extend animation.
                let mut v36 = false;
                self.m27_anchor_branch(body, br, 672);
                match self.ent[br].f69 {
                    0 => {
                        if self.ent[br].f126 == 0 {
                            let row = &D404C[(self.ent[br].f50 as usize).min(4)];
                            let e = &mut self.ent[br];
                            e.f69 = 1;
                            e.f44 = 1;
                            e.f34 = row[D404C_W12] as u16;
                            e.f36 = row[D404C_W14] as u16;
                        }
                    }
                    1 => {
                        v36 = true;
                        if self.ent[br].f126 == 192 {
                            let e = &mut self.ent[br];
                            e.f69 = 7;
                            e.f44 = 5;
                            e.f26 = 8;
                        }
                    }
                    7 => {
                        v36 = true;
                        self.ent[br].f26 -= 1;
                        if self.ent[br].f26 == 0 {
                            let e = &mut self.ent[br];
                            e.f69 = 8;
                            e.f44 = 6;
                            e.f36 = 0;
                            e.f68 = 0;
                            e.f56 = 0;
                            e.f128 = 12;
                            e.f26 = 0;
                        }
                    }
                    8 => {
                        let v18 = self.ent[br].f26;
                        if v18 > 10 {
                            self.ent[br].f71 = 8;
                        } else {
                            // Progressively HIDE the chain from the
                            // far end — retail `(byte[0]|1) & 0xF7`
                            // per node (EF:20055/20069-70): hidden +
                            // UNTARGETABLE while burrowing.
                            let hide: usize = if v18 != 0 {
                                let mut s = self.ent[br].f54 as usize;
                                let mut k = 0;
                                while k < 9 - v18 && s != 0 {
                                    s = self.ent[s].f54 as usize;
                                    k += 1;
                                }
                                s
                            } else {
                                br
                            };
                            if hide != 0 {
                                let f = &mut self.ent[hide].flags;
                                *f = (*f | m27_hide_mask()) & !0x08;
                            }
                            let v22 = self.ent[br].f68 + 1;
                            self.ent[br].f26 += 1;
                            self.ent[br].f68 = v22;
                            self.ent[br].f56 = self.ent[br].f56.wrapping_add(28 * v22 as u16);
                        }
                    }
                    _ => {}
                }
                if v36 {
                    let v23: i16 = if self.ent[br].f63 & 1 != 0 { -204 } else { 204 };
                    let w12 = D404C[(self.ent[br].f50 as usize).min(4)][D404C_W12];
                    // EF:20089: unmasked (see the wander-yaw note).
                    self.ent[br].f30 = (self.ent[body].f30 as i32 + w12 as i32 + v23 as i32) as u16;
                }
                self.m27_integrate(br);
                self.m27_swing_branch(body, br);
            }
            8 | 9 => {
                if state == 8 {
                    self.ent[br].f71 = 9;
                    self.ent[br].f26 = 100;
                    self.ent[body].f50 -= 1; // gauge --
                }
                self.ent[br].f26 -= 1;
                if self.ent[br].f26 == 0 {
                    self.ent[br].f71 = 10;
                }
            }
            10 | 11 => {
                if state == 10 {
                    self.ent[body].f50 += 1; // gauge ++
                    let first_seg = self.ent[br].f54;
                    let row = &D404C[(self.ent[br].f50 as usize).min(4)];
                    let e = &mut self.ent[br];
                    e.f71 = 11;
                    e.f44 = 5;
                    e.f26 = 7;
                    e.f146 = first_seg;
                    // Case 0xA re-show: `(byte[0] & 0xF6) | 8` —
                    // shown AND re-targetable (EF:20113-17).
                    e.flags = (e.flags & !m27_hide_mask()) | 0x08;
                    e.f34 = row[D404C_W12] as u16;
                    e.f126 = 156;
                    e.f36 = row[D404C_W14] as u16;
                }
                self.ent[br].f26 -= 1;
                if self.ent[br].f26 <= 0 {
                    self.ent[br].f71 = 12;
                    self.ent[br].f26 = 0;
                }
            }
            12 => {
                // case 0xC: sequential segment re-show + branch
                // regrow-life roll.
                if self.ent[br].f26 < 9 {
                    let mut s = self.ent[br].f54 as usize;
                    let mut k = 0;
                    while k < self.ent[br].f26 && s != 0 {
                        s = self.ent[s].f54 as usize;
                        k += 1;
                    }
                    if s != 0 {
                        // Case 0xC: `byte[0] &= 0xFE` — show only,
                        // bit 3 untouched (EF:20144).
                        self.ent[s].flags &= !m27_hide_mask();
                        self.ent[br].f146 = self.ent[s].f54;
                    }
                    self.ent[br].f26 += 1;
                    if self.ent[br].f26 >= 9 {
                        let d = self.mc2_rand(br); // DRAW #D — regrow life
                        self.ent[br].mail[0].1 = 0;
                        self.ent[br].f71 = 0;
                        self.ent[br].act_life = (d % 0x398) as i32 + 920;
                    }
                }
            }
            13 | 14 => {
                if state == 13 {
                    let e = &mut self.ent[br];
                    e.f71 = 14;
                    e.f68 = 10;
                    e.f26 = 10;
                }
                self.ent[br].f26 -= 1;
                if self.ent[br].f26 == 0 {
                    self.ent[br].f71 = 15;
                }
            }
            15 => {
                // case 0xF: hide branch + its 9 segments, → detach.
                self.ent[br].f71 = 8;
                let mut m = br;
                for _ in 0..10 {
                    // `(byte[0]|1) & 0xF7` on all 10 (EF:20177).
                    self.ent[m].flags = (self.ent[m].flags | m27_hide_mask()) & !0x08;
                    m = self.ent[m].f54 as usize;
                    if m == 0 {
                        break;
                    }
                }
            }
            _ => {}
        }

        // LABEL_94 (EF:20186): the positioning dispatch on the
        // (possibly updated) sub-state.
        match self.ent[br].f71 {
            0..=5 => {
                self.m27_integrate(br);
                self.m27_swing_branch(body, br);
                self.m27_spline_segments(body, br);
                if v37 != 0 {
                    let target = self.ent[br].f146;
                    self.m27_branch_bolt(br, target, v37 == 1, ctx);
                    self.snd(17, body);
                }
                self.m27_settle_branch(body, br);
            }
            6 | 7 => self.m27_spline_segments(body, br),
            11 | 12 => {
                self.m27_anchor_branch(body, br, 672);
                self.m27_swing_branch(body, br);
                self.m27_spline_segments(body, br);
                let t = self.ent[br].f146 as usize;
                if t != 0 && t < self.ent.len() {
                    let (tx, ty, tz) = {
                        let e = &self.ent[t];
                        (e.x, e.y, e.z)
                    };
                    self.move_relink(br, tx, ty, tz);
                }
                self.m27_settle_branch(body, br);
            }
            _ => {} // 8, 9, 10, 13, 14, 15 → no positioning
        }
    }

    /// `sub_102D0` mode 4 (EF:3687-3706) — the SLOPE-PITCH gate: the
    /// pitch from the entity's CURRENT position to the candidate,
    /// capped per direction by the row's `v_16` (one sign) / `v_18`
    /// (the other). The m27 ground mover is the mode's only caller
    /// (`1 || 4 || roughness>=32`, EF:19498/20912/20926) — the
    /// mode-1-only probe let the hydra crest slopes retail refuses.
    fn m27_slope_blocked(&self, i: usize, cand: (u16, u16, i16)) -> bool {
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let e = &self.ent[i];
        let pitch = Self::mc2_radix_tan((e.x, e.y, e.z), cand);
        match Self::turn_sign(0, pitch) {
            -1 => Self::angdist(0, pitch) as i32 > row.v_16 as i32,
            1 => Self::angdist(0, pitch) as i32 > row.v_18 as i32,
            _ => false,
        }
    }

    /// A/B toggle for the M27 MOVER FLAG-WORD law (round 104, dig
    /// W2-F): `MGC_NO_M27_MOVE_FLAGS` restores the pre-2026-09-05
    /// `m27_move`, which touched the flag word nowhere at all.
    fn m27_move_flags_law() -> bool {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *V.get_or_init(|| std::env::var_os("MGC_NO_M27_MOVE_FLAGS").is_none())
    }

    /// `sub_2AF10`'s blocked-status stamp (retail byte[2] bit 2).
    fn m27_set_blocked(&mut self, body: usize, on: bool) {
        if !Self::m27_move_flags_law() {
            return;
        }
        if on {
            self.ent[body].flags |= super::mobs::F_BLOCKED;
        } else {
            self.ent[body].flags &= !super::mobs::F_BLOCKED;
        }
    }

    /// The m27 ground mover (`sub_2AF10` EF:20869) — returns
    /// 1 same-tile / 2 moved / 3 turned / 4 fully blocked (which
    /// arms the 0xD8 teleport in place). `commit` = the a2 flag.
    pub(crate) fn m27_move(&mut self, body: usize, commit: bool) -> u8 {
        if Self::m27_move_flags_law() {
            // ⭐⭐⭐ THE MOVER'S OWN GRAB RELEASE (EF:20885-89 —
            // NETHERW.EXE 0x4F71E `f6 c6 10  test dh,0x10` /
            // 0x4F72C `80 63 0c fe  and BYTE PTR [ebx+0xc],0xfe` /
            // 0x4F734 `80 e5 ef  and ch,0xef`): retail's m27 body
            // mover clears the quake GRAB latch (byte[2] bit 4) on
            // every call, and when that latch is already clear it
            // clears the TOSSED latch (byte[0] bit 0) instead. The
            // port had neither, so an imported or freshly shoved
            // hydra body kept `F_TOSSED` forever and
            // [`Gen::flood_shovable`]'s class-5 `!held` arm refused
            // to shove it EVER AGAIN — mc2l22 t=22070 slot 98 is the
            // witness (retail's body is shoved +57/+68 to ground
            // z=92, the port's walks its own 30-unit step to z=360),
            // and the shove roll the port never spends is slot 691's
            // (10,67) `rand` at 22076/22079.
            if self.ent[body].flags & super::mobs::F_NO_CORPSE != 0 {
                self.ent[body].flags &= !super::mobs::F_NO_CORPSE;
            } else {
                self.ent[body].flags &= !(super::flood::F_TOSSED | 1);
            }
            // The forced-stop consume (EF:20890-94 — 0x4F73A
            // `8a 43 0d / a8 08  test al,8`, 0x4F9A5 `24 f7  and
            // al,0xf7` then code 4). Retail SKIPS the terminal
            // ground snap on this arm, so the early return is the
            // faithful shape.
            if self.ent[body].flags & super::mobs::F_STOP != 0 {
                self.ent[body].flags &= !super::mobs::F_STOP;
                self.ent[body].tick70 = M27_BASE; // 216
                self.ent[body].f26 = 0;
                return 4;
            }
        }
        let (x, y, z, yaw, roll, spd) = {
            let e = &self.ent[body];
            (e.x, e.y, e.z, e.f30, e.f34, e.f126)
        };
        let mut pred = (x, y, z);
        if commit {
            Self::polar_step(&mut pred, yaw, 0, spd);
        }
        pred.2 = self.ground_z(pred.0, pred.1) as i16;
        let mut moved = false;
        let mut turned = false;
        let code: u8;
        if commit && x >> 8 == pred.0 >> 8 && y >> 8 == pred.1 >> 8 {
            moved = true;
            turned = true;
            code = 1;
            // 0x4F7C4 `80 e5 fb  and ch,0xfb` (EF:20903).
            self.m27_set_blocked(body, false);
        } else if self.mc2_path_blocked(body, pred)
            || self.m27_slope_blocked(body, pred)
            || self.roughness_wide(pred.0, pred.1) >= 32
        {
            // BOTH blocked arms stamp the flag — 0x4F848
            // `80 4b 0e 04  or BYTE PTR [ebx+0xe],0x4` (yaw == roll,
            // EF:20915) and 0x4F834 `0c 04  or al,0x4` (yaw != roll,
            // EF:20951).
            self.m27_set_blocked(body, true);
            if yaw == roll {
                // Scan ±91-step yaws for a free heading.
                let mut v7: i32 = 91;
                let mut v9: i32 = 1;
                let mut found = None;
                while v7 <= 1024 {
                    let cand = ((yaw as i32 + v9 * v7) & 0x7FF) as u16;
                    let mut p2 = (x, y, z);
                    Self::polar_step(&mut p2, cand, 0, spd);
                    p2.2 = self.ground_z(p2.0, p2.1) as i16;
                    if !self.mc2_path_blocked(body, p2)
                        && !self.m27_slope_blocked(body, p2)
                        && self.roughness_wide(p2.0, p2.1) < 32
                    {
                        found = Some(cand);
                        break;
                    }
                    v9 = -v9;
                    if v9 == 1 {
                        v7 += 91;
                    }
                }
                // ⭐⭐⭐ THE ONE WRITER OF `sub_29A90`'s `v34` SLOT THAT
                // IS NOT STACK RESIDUE (dig 126O). `sub_2AF10`'s
                // `[ebp-0xc]` is the SAME absolute stack address as
                // `sub_29A90`'s `[ebp-0x10]` on every one of the four
                // dispatch paths, so a turn-search that RUNS leaves its
                // own 0/1 flag in the slot the branch machine then reads
                // as `v34`. NETHERW.EXE: `88 75 f4` (file 0x4F85E) zeroes
                // it on entry to the ±91 scan, `c6 45 f4 01` (0x4F8F3)
                // sets it when a heading is found, `80 7d f4 00` (0x4F920)
                // is the read that picks code 3 (`c6 45 fc 03`, 0x4F931)
                // over code 4 (`c6 45 fc 04`, 0x4F937). See
                // `m27_v34_residue_law` for the frame arithmetic.
                if let Some(cand) = found {
                    self.ent[body].f34 = cand;
                    turned = true;
                    code = 3;
                    self.m27_v34_slot.0 = Some(self.m27_v34_flag(1));
                } else {
                    code = 4;
                    self.m27_v34_slot.0 = Some(self.m27_v34_flag(0));
                }
            } else {
                turned = true;
                code = 3;
            }
        } else {
            moved = true;
            turned = true;
            code = 2;
            // 0x4F819 `80 e6 fb  and dh,0xfb` (EF:20958).
            self.m27_set_blocked(body, false);
        }
        if commit && moved {
            self.move_relink(body, pred.0, pred.1, pred.2);
        }
        if turned {
            // The live clamp is sub_58350's LAST arg = row v_2 = 22
            // (EF:20967-72); v_4 (=5) is the dead third arg — the
            // same trap the mc2 move core documents.
            let cap = BEHAVIOR[self.ent[body].row156 as usize].v_2;
            let e = &self.ent[body];
            let step = Self::turn_step(e.f30, e.f34, cap);
            self.ent[body].f30 = (self.ent[body].f30 as i32 + step as i32) as u16 & 0x7FF;
        }
        let (bx, by) = (self.ent[body].x, self.ent[body].y);
        let g = self.ground_z(bx, by) as i16;
        self.move_relink(body, bx, by, g);
        if code == 4 {
            self.ent[body].tick70 = M27_BASE; // 216
            self.ent[body].f26 = 0;
        }
        code
    }

    /// `sub_2AE80` (EF:20830): hide/reap the entire chain.
    fn m27_hide_chain(&mut self, i: usize) {
        let mut j = self.ent[i].f54 as usize;
        while j != 0 {
            let next = self.ent[j].f54 as usize;
            self.ent[j].flags |= 0x400;
            j = next;
        }
        self.ent[i].flags |= 0x400;
    }

    /// `sub_2AED0` (EF:20852): pose set (on change only).
    pub(crate) fn m27_pose(&mut self, i: usize, row: u16) {
        if self.ent[i].type86 != row {
            self.ent[i].type86 = row;
            self.ent[i].frame88 = 0;
        }
    }

    /// m27 states 0xD8-0xDF (EF:19443-19736 verbatim; branches and
    /// tier-2 segments never reach here — the world loop leaves
    /// 0xE9/0xEA undispatched like retail's null table entries).
    /// A pad-published residue is the 0xD9-path dword (W-52). The
    /// 0xD8/0xDA/0xDB handlers read one frame deeper (W-64), where
    /// the pad's `getTerrainAlt` left `sub_B5C60`'s saved `ebp` — a
    /// positive, even stack address, the seed's class — so a body on
    /// any other path drops the pad value and reads the seed.
    pub(crate) fn m27_v34_enter(&mut self, i: usize) {
        // ⭐ AN ADJACENCY-SCOPED PUBLICATION IS THE *PREVIOUS* HANDLER'S
        // LEAVING AND NOTHING MORE (dig W21). Every dispatched record
        // between the publisher and this body re-occupies the frames
        // the two dwords live in, and what an unmodelled handler leaves
        // there is the seed's class — so drop the publication unless
        // this body is the very next dispatched record. Conservative by
        // construction: a record the walk merely calls
        // `DisableEntityDrawing04_57F10` for (a LEAF, deepest write
        // W-36) counts as dispatched here, which only ever restores the
        // pre-law seed.
        if let Some(p) = self.m27_v34_slot.3.take() {
            if self.m27_v34_broken(p as usize, i) {
                self.m27_v34_slot.0 = None;
                self.m27_v34_slot.2 = None;
            }
        }
        if self.ent[i].tick70 != M27_BASE + 1 {
            // The deeper dword: what a same-walk writer published
            // there (`.2`), else the seed's class.
            self.m27_v34_slot.0 = self.m27_v34_slot.2;
        }
        self.m27_v34_slot.1 = false;
    }

    /// Did a dispatched record between the adjacency-scoped publisher
    /// at walk slot `p` and slot `i` re-occupy the `v34` frames? Class-0
    /// records are never dispatched, and the class-5 action-233/234
    /// chain members carry a NULL handler.
    pub(crate) fn m27_v34_broken(&self, p: usize, i: usize) -> bool {
        p >= i
            || (p + 1..i).any(|k| {
                self.ent[k].class64 != 0
                    && !(self.ent[k].class64 == 5 && matches!(self.ent[k].tick70, 233 | 234))
            })
    }

    /// A dispatched handler that provably leaves BOTH `v34` dwords
    /// alone: an adjacency-scoped publication survives it, so re-scope
    /// the publication to this walk slot (or drop it here if it was
    /// already broken upstream). Unscoped publications are untouched.
    pub(crate) fn m27_v34_transparent(&mut self, i: usize) {
        if let Some(p) = self.m27_v34_slot.3 {
            if self.m27_v34_broken(p as usize, i) {
                self.m27_v34_slot.0 = None;
                self.m27_v34_slot.2 = None;
                self.m27_v34_slot.3 = None;
            } else {
                self.m27_v34_slot.3 = Some(i as u16);
            }
        }
    }

    pub(crate) fn m27_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.m27_v34_enter(i);
        match self.ent[i].tick70 - M27_BASE {
            // 0xD8 — emerge/teleport sequencer on the f26 phase.
            0 => {
                let v1 = self.ent[i].f26;
                self.ent[i].f26 += 1;
                match v1 {
                    0 => {
                        self.m27_pose(i, 337);
                        self.ent[i].act_life = 1_000_000;
                        self.ent[i].f146 = 0;
                    }
                    9 => {
                        // The teleport probe: TWO draws, then up to
                        // 128 steps of 768 along the body yaw.
                        let d1 = self.mc2_rand(i);
                        let d2 = self.mc2_rand(i);
                        let (x, y, yaw) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.f30)
                        };
                        let mut pred = (
                            x.wrapping_add((((d1 & 7) + 8) << 8) as u16),
                            y.wrapping_add((((d2 & 7) + 8) << 8) as u16),
                            0i16,
                        );
                        for _ in 0..128 {
                            pred.2 = self.ground_z(pred.0, pred.1) as i16;
                            if !self.mc2_path_blocked(i, pred)
                                && !self.m27_slope_blocked(i, pred)
                                && self.roughness_wide(pred.0, pred.1) < 32
                            {
                                break;
                            }
                            Self::polar_step(&mut pred, yaw, 0, 768);
                        }
                        self.move_relink(i, pred.0, pred.1, pred.2);
                        self.snd(22, i);
                    }
                    18 => {
                        self.ent[i].tick70 = M27_BASE + 2; // 218
                        self.m27_pose(i, 337);
                        self.ent[i].f146 = 0;
                        self.ent[i].f71 = 1;
                    }
                    // Phases 3/6/12/15: chain draw-group staging —
                    // renderer markers, unmodeled (module doc).
                    _ => {}
                }
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                let g = self.ground_z(x, y) as i16;
                self.move_relink(i, x, y, g);
                // ⭐ THE 0xD8 TAIL RE-OCCUPIES THE SLOT. `sub_29400`
                // (0x4DC00, ebp = W-20, esp = W-24) ends EVERY arm with
                // `push eax / call getTerrainAlt_10C40` (0x4DE4B-4C:
                // arg W-28, return W-32); `getTerrainAlt` pushes ebp
                // (W-36), y (W-40), x (W-44) and calls 0xDA460 (return
                // W-48), whose prologue pushes ebp/ebx/ecx/edx/esi/edi
                // on W-52..W-72 — so W-64, this path's `[ebp-0x10]`,
                // holds EDX = `mov edx,[ebp+0x8]` (0x35443), the
                // position POINTER: positive, even, the seed's class.
                // A same-walk leaving (the building's 29) never reaches
                // a 0xD8 body. mc2l22 22098/22226/46464/46496/53167:
                // body 98 emerging behind thirteen village buildings,
                // retail stamps 4 and takes the draw.
                if Self::m27_v34_building_law() {
                    self.m27_v34_slot.0 = None;
                }
                self.m27_drive(i, ctx);
            }
            // 0xD9 — the main brain.
            1 => {
                match self.m27_body_intake(i) {
                    1 => {
                        self.ent[i].tick70 = M27_BASE + 2; // 218
                        let att = self.ent[i].f40;
                        self.ent[i].act_life = 1_000_000;
                        self.ent[i].f146 = att;
                    }
                    2 => {
                        self.m27_drive(i, ctx);
                        return;
                    }
                    _ => {}
                }
                let v3 = self.m27_move(i, true);
                if v3 >= 3 {
                    self.m27_drive(i, ctx);
                    return;
                }
                if self.ent[i].f63 & 0x3F == 0 {
                    let d = self.mc2_rand(i);
                    self.ent[i].f34 =
                        ((d % 0x1C7) as i32 + self.ent[i].f34 as i32 - 227) as u16 & 0x7FF;
                }
                self.m27_drive(i, ctx);
            }
            // 0xDA — chase-target brain.
            2 => {
                let mut drop_target = false;
                match self.m27_body_intake(i) {
                    1 => {
                        self.ent[i].act_life = 1_000_000;
                        let att = self.ent[i].f40;
                        self.ent[i].f146 = att;
                    }
                    2 => {
                        self.m27_drive(i, ctx);
                        return;
                    }
                    _ => {}
                }
                let commit = self.ent[i].f71 == 0;
                // ⭐ The 0xDA handler is the ONE m27 path with calls
                // BELOW `sub_2AF10` that re-occupy the v34 slot: the
                // aim/range helpers at file 0x4DFB1 / 0x4DFD9 / 0x4DFEE
                // / 0x4E015 / 0x4E02F and the pose set at 0x4E05C are
                // all TWO-arg calls from `esp = ebp-4`, which puts their
                // callee frames straight over W-64. When any of them
                // runs, the mover's flag is gone again and the read is
                // back on the registered floor.
                let mut v34_clobber = false;
                let mut v34_aim: Option<u32> = None;
                let v2 = self.m27_move(i, commit);
                if v2 == 4 {
                    // (m27_move already armed 216.)
                } else if let Some(tpos) = self.mc2_target(self.ent[i].f146, ctx) {
                    let (x, y, z) = {
                        let e = &self.ent[i];
                        (e.x, e.y, e.z)
                    };
                    if self.ent[i].f63 & 3 == 0 && v2 != 3 && self.ent[i].f71 == 0 {
                        v34_clobber = true;
                        self.ent[i].f34 = Self::angle_between(x, y, tpos.0, tpos.1);
                    }
                    if self.ent[i].f63 & 0x1F == 0 {
                        v34_clobber = true;
                        // The LAST deep call of this block is the
                        // range test's `sub_583F0` → isqrt, whose
                        // saved `ebx` is `target.y − body.y` (the
                        // body's POST-MOVE y: the mover committed
                        // above). See `m27_v34_aim_word`.
                        if Self::m27_v34_aim_law() {
                            v34_aim = Some(Self::m27_v34_aim_word(i, y, tpos.1));
                        }
                        let row = &BEHAVIOR[self.ent[i].row156 as usize];
                        let yaw = Self::angle_between(x, y, tpos.0, tpos.1);
                        if Self::angdist(self.ent[i].f30, yaw) <= row.v_30 as u16 {
                            self.ent[i].f71 = 1;
                            self.m27_pose(i, 337);
                        } else {
                            self.ent[i].f71 = 0;
                            self.m27_pose(i, 315);
                        }
                        if Self::mc2_dist3((x, y, z), tpos) >= row.v_28 as u32 {
                            drop_target = true;
                        }
                    }
                } else {
                    drop_target = true;
                }
                if drop_target {
                    self.ent[i].tick70 = M27_BASE + 1; // 217
                    self.m27_pose(i, 315);
                    self.ent[i].f146 = 0;
                    self.ent[i].f71 = 0;
                    v34_clobber = true;
                }
                if let Some(v) = v34_aim {
                    // The aim pass ran: its range test's leaving is
                    // what the chain machine reads, whatever the
                    // `f63 & 3` tan2 or the mover left before it.
                    self.m27_v34_slot.0 = Some(v);
                    self.m27_v34_slot.1 = false;
                } else if v34_clobber {
                    self.m27_v34_slot.0 = None;
                }
                self.m27_drive(i, ctx);
            }
            // 0xDB — return-then-idle.
            3 => {
                self.ent[i].tick70 = M27_BASE + 1;
                // ... and run the 0xD9 body this tick (sub_29890).
                self.m27_tick(i, ctx);
            }
            // 0xDC — life = -1, PreKill cascades 0xDD over the chain.
            4 => {
                self.ent[i].act_life = -1;
                self.mc2_prekill(i, M27_BASE);
            }
            // 0xDD — death: mana spheres (fraction mode), the (10,1)
            // burst, then hide the whole chain. Branches/segments
            // cascaded here each run this too (their own spheres are
            // empty — mana 0 — but the burst pops).
            5 => {
                self.ent[i].act_life = -1;
                self.mc2_mana_spheres(i, true);
                if self.ent[i].flags & super::mobs::F_NO_CORPSE == 0 {
                    self.mc2_corpse_burst(i);
                }
                self.m27_hide_chain(i);
            }
            // 0xDE — no unique body (tail of sub_298D0).
            6 => {}
            // 0xDF with StageVar2==0 (an ordinary spawn/appear —
            // never stage-held): retail's sub_29930 head `sub_1D5D0`
            // is a no-op at kind 0, the pose select reads v1=0 → 315,
            // and neither command arm can fire (tick70 stays 223), so
            // pose + life + drive IS the verbatim reduction. A
            // stage-HELD body (site_z 1..=9/10/15) never reaches this
            // dispatch — the world loop routes it through
            // `World::mc2_m27_held_tick` (stagevars.rs), the full
            // sub_29930 port with the 0xDA mass-attack broadcast and
            // the 0xD8→StageVar2=15 arm.
            _ => {
                self.m27_pose(i, 315);
                self.ent[i].act_life = 1_000_000;
                self.m27_drive(i, ctx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BRANCH_STATE, M22_BASE};
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes};
    use crate::mc1::mobs::MobCtx;
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

    fn ctx() -> MobCtx {
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
            patches: WorldPatches::RETAIL,
            mc2_turn: 0,
        }
    }

    /// A worm head parked in 0xB2 (castle acquire) on a phase where
    /// both `& 0x1F` and `& 3` are open, latched onto `target`, with
    /// that wizard's castle far out of the 0x100 deposit range — so
    /// the ONLY thing that can move it off 178 is a LABEL_17 revert.
    /// Returns `(head, wizard, castle)`.
    fn worm_on_wizard(g: &mut Gen, wizard_life: i32) -> (usize, usize, usize) {
        let wiz = g.new_event().expect("wizard slot");
        {
            let e = &mut g.ent[wiz];
            e.class64 = 3;
            e.model65 = 0;
            e.id24 = 7;
            e.max_life = 10_000;
            e.act_life = wizard_life;
            e.x = 0x1000;
            e.y = 0x1000;
        }
        let castle = g.new_event().expect("castle slot");
        {
            let e = &mut g.ent[castle];
            e.class64 = 3;
            e.model65 = 2;
            e.id24 = 7;
            e.max_life = 160_000;
            e.act_life = 160_000;
            e.x = 0x1000;
            e.y = 0x1000;
            e.f136 = 300_000_000; // maxMana @0x8C — always room
            e.f140 = 0; // mana @0x90
        }
        let head = g.new_event().expect("worm slot");
        {
            let e = &mut g.ent[head];
            e.class64 = 5;
            e.model65 = 22;
            e.tick70 = M22_BASE + 2; // 178 = the castle-acquire state
            e.max_life = 2000;
            e.act_life = 2000;
            e.x = 0xF000; // ~50,000 units from the castle: the
            e.y = 0x8000; // deposit range test can never pass
            e.z = 447;
            e.f63 = 0; // phase: & 0x1F == 0 AND & 3 == 0
            e.f71 = 1; // one-segment chain (no tail to walk)
            e.f126 = 0; // actSpeed @0x82 <= maxSpeed @0x86
            e.f130 = 0;
            e.f146 = 64; // grow timer: not due this tick
            e.f26 = 779; // the live `dword_0x10_16` the revert zeroes
            e.f144 = wiz as u16; // playerEntityIndex @0x94
        }
        (head, wiz, castle)
    }

    /// ⭐⭐⭐ `sub_2A6F0` WALKS `dword_38519` AND HAS NO MODEL TEST.
    /// The m27 branch's target scan (EF:20452-83, NETHERW.EXE
    /// 0x4F1F0 region) is `v3x = x_D41A0_BYTEARRAY_4_struct.
    /// dword_38519; while (v3x > Entities_EA3E4[0]) { ... }` — the
    /// tick-top CLASS-3 roster, built by the case-3 arm of
    /// `UpdateEntities_57730` on `class == 3 && life >= 0` alone
    /// (EF:39972-85). Class, life and the reap flag are settled by
    /// MEMBERSHIP, and no model test exists anywhere on the path, so
    /// a (3,2) castle or a (3,3) can win this nearest-in-cone scan
    /// and keep a wizard from winning it. The port filtered
    /// `model65 <= 1` on a LIVE pool sweep — the same invented
    /// filter [`Gen::mc2_wizard_scan`] was already corrected for.
    ///
    /// The free-run witness is `mc2l22` t=21616, where retail's
    /// branch at slot 256 locks the (3,3) at slot 268
    /// (`byte_0x46_70` 1→2, `word_0x96_150` 0→268) and the whip's
    /// mode-2 decel takes `actSpeed` 192→176 the next tick; that
    /// head is pair-CLEAN (both lanes are ungraded), so it is
    /// covered here rather than by a fixture.
    #[test]
    fn the_m27_branch_scan_has_no_model_filter() {
        let mut g = flat_gen();
        // A (3,3) — not a wizard by the port's old filter — sitting
        // exactly on the branch, so range and cone are both trivial.
        let other = g.new_event().expect("class-3 slot");
        {
            let e = &mut g.ent[other];
            e.class64 = 3;
            e.model65 = 3;
            e.max_life = 10_000;
            e.act_life = 10_000;
            e.x = 1000;
            e.y = 1000;
        }
        // A real wizard at the same spot, LATER in slot order: the
        // scan is strict `<` on the nearest compare, so the first
        // member of the roster keeps the lock.
        let wiz = g.new_event().expect("wizard slot");
        {
            let e = &mut g.ent[wiz];
            e.class64 = 3;
            e.model65 = 0;
            e.max_life = 10_000;
            e.act_life = 10_000;
            e.x = 1000;
            e.y = 1000;
        }
        let br = g.new_event().expect("branch slot");
        {
            let e = &mut g.ent[br];
            e.class64 = 5;
            e.model65 = 27;
            e.tick70 = BRANCH_STATE;
            e.row156 = 103; // every (5,27) branch carries row 103
            e.max_life = 5980;
            e.act_life = 5980;
            e.x = 1000;
            e.y = 1000;
            e.f30 = 0;
        }
        g.rebuild_wiz_chain();
        let mut c = ctx();
        c.px = 20_000; // the human is well outside row 103's 5120
        c.py = 20_000;
        assert_eq!(
            g.m27_wizard_scan(br, &c),
            Some(other as u16),
            "the (3,3) is a roster member and wins the tie"
        );
    }

    /// ⭐⭐⭐ `sub_26AA0` INTERROGATES THE POSSESSED WIZARD BEFORE IT
    /// TOUCHES THE CASTLE. `NETHERW.EXE` 0x4B305-0x4B32D:
    /// `mov 0x1a3e4(,%eax,4),%eax` / `cmpb $0x3,0x3f(%eax) ; jne` /
    /// `cmpl $0x0,0x8(%eax) ; jl` / `testb $0x4,0xd(%eax) ; jne` —
    /// all three land on LABEL_17, which zeroes `dword_0x10_16` and
    /// `playerEntityIndex_0x94_148` and drops the head to 177.
    /// The port asked none of them.
    ///
    /// The free-run witness is `mc2l6-rival-spells-galore` t=25,532,
    /// where the possessed wizard is the HUMAN and the port takes the
    /// `PLAYER_TARGET`/`ctx.pdead` arm. This test covers the POOL arm
    /// (a rival wizard), which the recording corpus does not exercise.
    #[test]
    fn a_worm_drops_its_castle_target_when_the_wizard_dies() {
        // Live wizard: no revert, the head stays in 0xB2.
        let mut g = flat_gen();
        let (head, _, _) = worm_on_wizard(&mut g, 10_000);
        g.m22_tick(head, &ctx());
        assert_eq!(g.ent[head].tick70, M22_BASE + 2, "a live target holds 0xB2");
        assert_ne!(g.ent[head].f144, 0, "and keeps its playerEntityIndex");

        // Dead wizard: `cmpl $0x0,0x8(%eax) ; jl LABEL_17`.
        let mut g = flat_gen();
        let (head, _, _) = worm_on_wizard(&mut g, -1270);
        g.m22_tick(head, &ctx());
        assert_eq!(
            g.ent[head].tick70,
            M22_BASE + 1,
            "a dead target reverts the head to 177 (LABEL_17)"
        );
        assert_eq!(g.ent[head].f144, 0, "playerEntityIndex_0x94_148 = 0");
        assert_eq!(g.ent[head].f26, 0, "dword_0x10_16 = 0");

        // Reap-flagged wizard: `testb $0x4,0xd(%eax) ; jne LABEL_17`.
        let mut g = flat_gen();
        let (head, wiz, _) = worm_on_wizard(&mut g, 10_000);
        g.ent[wiz].flags |= 0x400;
        g.m22_tick(head, &ctx());
        assert_eq!(
            g.ent[head].tick70,
            M22_BASE + 1,
            "a reap-flagged target reverts too"
        );

        // Not a wizard any more: `cmpb $0x3,0x3f(%eax) ; jne LABEL_17`.
        let mut g = flat_gen();
        let (head, wiz, _) = worm_on_wizard(&mut g, 10_000);
        g.ent[wiz].class64 = 5;
        g.m22_tick(head, &ctx());
        assert_eq!(
            g.ent[head].tick70,
            M22_BASE + 1,
            "a target that is no longer class 3 reverts too"
        );
    }

    /// ⭐⭐⭐ EVERY ONE OF THE HYDRA'S 51 RECORDS IS BORN IN SPEED MODE
    /// 0 (round 147, dig w147f;
    /// [`crate::mc2::mobs::no_mc2_m27_2c_zero`]).
    ///
    /// On this family `f44` is `word_0x2C_44`, the branch SPEED-MODE
    /// SELECTOR (`sub_2A340`, banner EF:20255) — not
    /// `subSpellIndex_0x2A_42`. Retail's `memset` leaves @0x2C at 0
    /// and `sub_4D000` (banner EF:34642) never writes it, so every
    /// body, branch and segment is born in mode 0; the port's shared
    /// `new_event` default put NewEvent's @0x2A **100** in that one
    /// field and sent all 51 records down the integrator's
    /// fall-through arm.
    ///
    /// ⛔ NATIVE-INIT ONLY, SO IT GETS A UNIT TEST: replay imports the
    /// pool, so no graded lane ever sees this ctor run.
    ///
    /// Non-vacuous by the `new_event` control below — the shared
    /// default really is 100 in this rig, so the 51 zeroes are the
    /// ctor's work. With `MGC_NO_MC2_M27_2C_ZERO=1` all 51 read 100.
    #[test]
    fn every_hydra_record_is_born_in_speed_mode_zero() {
        let mut g = flat_gen();
        // POSITIVE CONTROL: the shared ctor default this law overrides.
        let bare = g.new_event().expect("a bare record");
        assert_eq!(
            g.ent[bare].f44, 100,
            "`new_event` seeds NewEvent's @0x2A 100 into the shared field"
        );

        let body = g.mc2_spawn_m27(40 << 8, 40 << 8, 100).expect("hydra");
        // Walk the one linear f54 chain the ctor builds: body, then
        // each branch followed by its nine segments.
        let mut members = vec![body];
        let mut n = g.ent[body].f54 as usize;
        while n != 0 {
            members.push(n);
            n = g.ent[n].f54 as usize;
        }
        assert_eq!(members.len(), 51, "1 body + 5 branches + 45 segments");
        for m in members {
            assert_eq!(g.ent[m].class64, 5);
            assert_eq!(g.ent[m].model65, 27);
            assert_eq!(
                g.ent[m].f44, 0,
                "hydra member {m} (action {}) is born in speed mode 0",
                g.ent[m].tick70
            );
        }
    }
}
