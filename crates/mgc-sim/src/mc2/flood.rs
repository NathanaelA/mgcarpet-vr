//! MC2 (10,67)=0x43 FLOOD/QUAKE — the three-action terrain-morph
//! quake: action 72 (`sub_39040`) raises a sin-profile dome ring with a
//! sinking crater center and converts the footprint to lava, action 73
//! (`sub_396A0`) holds the entity-shove while life runs out, action 74
//! (`sub_396D0`) settles the terrain back and despawns. Trace bank:
//! docs/traces/mc2-class10-tail-helper-closure.md §1 (the phase
//! machine) + mc2-class10-m67-flood-helpers.md (every helper VERBATIM)
//! (`EF:` = remc2 EventsFunctions.cpp, `Terrain:` = engine/Terrain.cpp,
//! `Maths:` = utilities/Maths.cpp).
//!
//! Entity-field homes follow the class-10 effect column: subSpell →
//! f140, `dword_0x10_16` countdown → f26, `byte_0x46_70` phase → f71,
//! dome-top reference `word_0x2C_44` → f44, grab owner `word_0x26_38`
//! → f40, grab timer `word_0x30_48` → f50 (the castle shake home —
//! retail's 30 write IS the blast shake). Retail's `dword_38519`
//! "object" list is the CLASS-3 list (the builder EF:39970-39985) —
//! the model-2 members the damage pass grabs are CASTLES — and
//! `dword_38527` is the class-10 MODEL-45 list (EF:40043-40052): the
//! quake ERASES overlapping village buildings.
//!
//! Flag decode (helpers doc §5): the `|= 0x100001` victim write =
//! byte[0] bit0 + byte[2] bit4. byte[0] bit0 in this band is the
//! TOSSED/handled latch (creatures normally carry it clear; the
//! shove filter skips victims with it set, and the action-74 release
//! clears it) — the quake band's own home is [`F_TOSSED`], but that is
//! a SECOND home for a bit the port already keeps positionally at
//! `Ent::flags` bit 0, so both the writes (round 99, dig 99-10) and
//! the READ ([`flood_held_bit0_law`], round 149, dig w149i) cover the
//! pair. ~~retail's bit0 aliases our "active" flag and cannot be
//! shared~~ — `retail_import_mc2` maps retail byte[0] bit 0 straight
//! onto `flags` bit 0 and the (3,3)/(14,3)/(14,4) markers GRADE it.
//! byte[2] bit4 = the grab
//! latch = [`super::mobs::F_NO_CORPSE`]'s retail bit — reusing it is
//! the authentic alias (quake victims leave no corpse).
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - `sub_6D8B0(id, 0x14, n)` spell-XP reports (EF:29367/:29436):
//!   counts computed and dropped, and the global objects-hit counter
//!   `x_DWORD_E9B90` (EF:28527) has no ported reader.
//! - The HUMAN player: **NARROWED round 146** (dig w146e,
//!   `MGC_NO_MC2_FLOOD_HUMAN_SEAT`; dig w146f,
//!   `MGC_NO_MC2_FLOOD_HUMAN_SPIN`). He is now visited AT HIS SEAT in
//!   the cell walk (a repeat visit when a shove carries him into an
//!   unvisited row), the horizontal shove, z pull and ground floor are
//!   an accumulated record write drained at the carpet's dispatch (no
//!   longer the `player_knock` channel), and the close band seizes
//!   `pitch`/`pitch_acc` = 512. Still approximated: the close-range
//!   1-in-7 kill roll mails a kill-scale 32000 (retail adds the
//!   victim's `life+1` — the guaranteed kill — which Gen cannot read
//!   for the player); the `|= 0x100001` flag write on the human
//!   record; and a quake ABOVE the carpet's walk slot (its delta is
//!   published after the carpet moved and is lost — no capture shows
//!   one).
//! - ~~The rival-wizard pitch-512 spin (presentation: the body flip)
//!   is skipped; the damage roll is faithful.~~ **NOT presentation**
//!   (round 146, dig w146f): it is the graded record pitch and the
//!   wizard's `pitch_0x157_343` accumulator — landed on the human seat
//!   and on the rival record behind `MGC_NO_MC2_FLOOD_HUMAN_SPIN`; the
//!   rival accumulator half still has no home.
//! - The action-74 release's local-player visibility juggle
//!   (EF:29118-29127: byte[0] bit0 set for the local wizard body,
//!   cleared for everyone else) is the draw latch — our release
//!   clears [`F_TOSSED`] for everyone (the observable single-player
//!   effect: victims become shoveable again).
//! - ~~The deep-sink skip (`word_160_0xe_14 < -64`, EF:29106 — the
//!   victim's Type_160 z-velocity) has no ported home; the z pull
//!   always applies before the ground clamp.~~ **PAID 2026-09-04
//!   (round 98, dig Q31).** The premise was false: `word_160_0xe_14`
//!   is `Mc2BehaviorRow::v_14`, the very z step `Gen::mc2_alt_core`
//!   reads on every creature move. Landed in `flood_shove` behind
//!   `MGC_NO_FLOOD_GROUND_SNAP`, byte-verified at `NETHERW.EXE`
//!   0x5E53C-0x5E549.
//! - Cave arms (second-heightmap easing + the mapAngle bit-3 seal) are
//!   not yet ported (TODO).
//! - `mana_0x90_144 = 0` in phase 1 (EF:28548) has no ported reader
//!   on this column and is skipped.

use super::behavior::BEHAVIOR;
use super::morph::{auto_flat, dist2d};
use super::sin_lut::SIN_DB750;
use crate::engine::features::{Gen, tile};
use crate::mc1::combat::MailTarget;
use crate::mc1::mobs::MobCtx;

/// A/B kill switch for the MISSING REAP GATE (round 98, dig Q31):
/// `MGC_NO_FLOOD_NO_REAP_GATE=1` restores the pre-2026-09-04 shape,
/// where `flood_shove`'s chain walk skipped every victim carrying the
/// reap mark. Retail's walk has no such term (`NETHERW.EXE` 0x5E40A).
pub(crate) fn flood_no_reap_gate_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_FLOOD_NO_REAP_GATE").is_none())
}

/// A/B kill switch for the DEEP-SINK GROUND SNAP (round 98, dig Q31):
/// `MGC_NO_FLOOD_GROUND_SNAP=1` restores the pre-2026-09-04 shape,
/// where every shoved victim took the z pull before the ground clamp.
pub(crate) fn flood_ground_snap_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_FLOOD_GROUND_SNAP").is_none())
}

/// ⭐⭐⭐ A/B kill switch for THE HUMAN ARM'S MISSING VERTICAL LEG:
/// `MGC_NO_MC2_FLOOD_HUMAN_Z_PULL=1` restores the pre-round-143 shape,
/// where the quake's special-cased human arm armed only the horizontal
/// `player_knock` and the z pull-down was banked (module header,
/// "the z pull and spin bank on the FlightVerb takeover seam").
///
/// Retail has no special case at all — the human wizard is an ordinary
/// victim of `sub_39B60`, and its WIZARD arm (`NETHERW.EXE` 0x5e4ef
/// `mov 0x3f(%ebx),%ah` / `cmp $0x3,%ah` / `jne 0x5e53c`, then 0x5e4fa
/// `cmpb $0x0,0x40(%ebx)`) takes the pull-then-clamp leg that the pool
/// half of [`Gen::flood_shove`] already ports.
///
/// WITNESS — mc2l23 t=6930..6961, the take's 32-deep reset run and its
/// whole `pose:pose.z` signature. The player flies into his own
/// Gravity Well's disc at t=6929 (quake slot 101, `refz` 110); retail
/// sinks 26, 27, 27, 27, 28 … per tick and the port sank 8 — the row-
/// 0xe buoyancy ALONE — from the same position, with `pose.x`/`pose.y`
/// bit-exact for eleven more ticks because the horizontal leg WAS
/// ported. 8 + the pull reproduces retail's step to the unit.
pub(crate) fn no_mc2_flood_human_z_pull() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FLOOD_HUMAN_Z_PULL").is_some())
}

/// ⭐⭐⭐ A/B kill switch for THE CHAIN CURSOR THE SHOVE RE-READS
/// (round 99, dig 99-3): `MGC_NO_FLOOD_CHAIN_REWALK=1` restores the
/// pre-2026-09-04 shape, where `flood_shove` cached the map-chain
/// `next` pointer BEFORE moving the victim.
///
/// Retail's `sub_39B60` chain walk is a bare `for (i = mapEntityIndex
/// [cell]; ; i = victim->oldMapEntity_0x16_22)`, and the C `for`
/// increment is evaluated AFTER the body — i.e. after
/// `CopyEntityPosition_57CF0(victim, &predicted)` may have RE-LINKED
/// the victim into a different cell's chain. `AddEventToMap_57D70`
/// pushes at the HEAD and parks the cell's previous head in the
/// victim's own `@0x16`, so a shove that crosses a tile edge makes the
/// walk CONTINUE DOWN THE DESTINATION CELL'S CHAIN — every entity
/// there is shoved by this cell's iteration too, and again when the
/// sweep reaches that cell itself.
///
/// Shipped `NETHERW.EXE` (sub_39B60 = runtime 0x39B60, file 0x5E360;
/// the region's rule is file = runtime + 0x24800, cross-checked on
/// five call targets — `call 0x5e7a0`=sub_39FA0, `call 0x35440`=
/// getTerrainAlt_10C40, `call 0x7cc90`=EuclideanDistXYZ_58490,
/// `call 0x7c7a0`=MoveEntity_57FA0, `call 0x7c4f0`=
/// CopyEntityPosition_57CF0):
/// ```text
///   5e58c: 68 98 b3 01 00        push $0x1b398        ; &predictedAxis
///   5e591: 53                    push %ebx            ; the VICTIM
///   5e592: e8 59 df 01 00        call 0x7c4f0         ; CopyEntityPosition_57CF0
///   5e597: eb 0a                 jmp  0x5e5a3         ; -> LABEL_25
///   …                                                 ; %ebx never reloaded
///   5e5e4: 31 c0                 xor  %eax,%eax
///   5e5e6: 66 8b 43 16           mov  0x16(%ebx),%ax  ; NEXT, off the MOVED victim
///   5e5ea: 8b 1c 85 e4 a3 01 00  mov  0x1a3e4(,%eax,4),%ebx
///   5e5f1: 3b 1d e4 a3 01 00     cmp  0x1a3e4,%ebx
///   5e5f7: 0f 85 0d fe ff ff     jne  0x5e40a         ; loop
/// ```
pub(crate) fn flood_chain_rewalk_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_FLOOD_CHAIN_REWALK").is_none())
}

/// ⭐⭐⭐ THE SHOVE FILTER'S `held` TEST READ ONLY THE **IMPORTED** HALF
/// OF A BIT WITH TWO PORT HOMES. Set `MGC_NO_MC2_FLOOD_HELD_BIT0=1` to
/// restore the pre-round-149 `flags & (F_TOSSED | 0x20)`.
///
/// Retail's shove-victim filter `sub_39FA0` tests `byte[0] & 0x21` on
/// three of its fifteen class arms, and byte[0] bit 0 is ONE retail bit.
/// The port homes it TWICE: positionally at `Ent::flags` bit 0 (every
/// native setter — the `(14,3)`/`(14,4)` marker pre-hide `sub_51570`
/// `80 4b 0c 01`, the castle walk latch, the in-book bit …) and, for
/// the quake band only, at the synthetic [`F_TOSSED`] (bit 31). Round
/// 99 dig 99-10 already put the quake's own WRITES on both homes
/// ([`Gen::flood_shove_hit`] and the action-74 release, behind
/// `MGC_NO_QUAKE_TOSS_BIT0` / `MGC_NO_QUAKE_RELEASE_BIT0`) and the
/// importer seats both (`retail_import_mc2`, `no_mc2_tossed_import`) —
/// but the READ stayed on bit 31 alone, so every victim whose byte[0]
/// bit 0 was set by some OTHER retail routine passed a filter retail
/// fails. ⭐ A LAW ON ONE CALL PATH IS NOT LANDED: the write half and
/// the import half landed, the read half did not.
///
/// Shipped `NETHERW.EXE`, `sub_39FA0` = runtime 0x39FA0, file 0x5E7A0
/// (file = runtime + 0x24800); the class-14 arm, file 0x5E879:
/// ```text
///   5e879: f6 42 0c 21    testb $0x21,0xc(%edx)   ; byte[0] & 0x21
///   5e87d: 75 07          jne   0x5e886           ; -> return 0
///   5e87f: 8a 62 40       mov   0x40(%edx),%ah    ; model
///   5e882: 38 c4          cmp   %al,%ah           ; al = 1
///   5e884: 75 02          jne   0x5e888           ; -> return 1
///   5e886: 30 c0          xor   %al,%al           ; return 0
/// ```
/// The same `f6 42 0c 21` opens the class-5 arm (0x5E7C6) and guards
/// the class-3 MODEL-1 arm (0x5E848); no other arm reads it.
///
/// WITNESS — mc2l22 t=63665, the take's LAST graded deviation. The
/// `(14,3)` checkpoint marker at slot 3 carries retail `flags 13`
/// (byte[0] bit 0 = the marker pre-hide its ctor stamps, never yet
/// tripped). Quake slot 906 sweeps it at `d = 3049`, `v5 = 2576`;
/// retail's filter returns 0 and the marker does not move. The port
/// had bit 0 set and bit 31 clear, passed the filter, and walked the
/// marker 10/256 of a tile toward the quake —
/// `slot 3 y: retail 125.5 port 125.5390625`. It classified as
/// INHERITED precisely because a slice's importer DOES seat
/// [`F_TOSSED`] from the same retail bit, so the head vanishes on
/// every re-anchored replay and only a 53,880-tick free run shows it.
pub(crate) fn flood_held_bit0_law() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FLOOD_HELD_BIT0").is_none())
}

/// ⭐⭐ THE QUAKE LATCH'S SECOND HOME — THE RELEASE CLEARS byte[0]
/// BIT 0 TOO. Retail's action-74 release (LABEL_25, EF:29062-75) is a
/// three-term interrogation of the victim followed by two masks, and
/// the port carried only the second. VERIFIED in the shipped
/// NETHERW.EXE (linear 0x39DAD = file 0x5E5AD, region rule
/// file = linear + 0x24800; bytes read by the main session
/// 2026-09-04, and this block butts directly onto 0x5E5E6, the
/// chain-rewalk cite above):
///   5e5ad: 75 35          jne  0x5e5e4                 ; action != 74
///   5e5af: f6 43 0e 10    test BYTE PTR [ebx+0xe],0x10 ; F_QUAKE_GRAB
///   5e5b5: 80 7b 3f 03    cmp  BYTE PTR [ebx+0x3f],0x3 ; class == 3
///   5e5bb: 80 7b 40 00    cmp  BYTE PTR [ebx+0x40],0x0 ; model == 0
///   5e5d6: 80 4b 0c 01    or   BYTE PTR [ebx+0xc],0x1  ; LOCAL wizard
///   5e5dc: 80 63 0c fe    and  BYTE PTR [ebx+0xc],0xfe ; EVERYONE ELSE
///   5e5e0: 80 63 0e ef    and  BYTE PTR [ebx+0xe],0xef
/// Since `import_ent_mc2` now fills BOTH homes of retail's single
/// byte[0] bit 0 (`MGC_NO_MC2_TOSSED_IMPORT`), every imported quake
/// victim arrived with bit 0 set and NOTHING ever cleared it — and
/// `mc2_awake_one`'s hidden `if flags & 1 { return }` gate (EF:55515)
/// then returned before the proximity test forever, freezing the
/// (10,57) ground mana spheres at their imported x/y/z. rsg
/// t=26588→26603 slot 676: `b39` retail 16 / port 0, zero writes to
/// `f58`, and `MGC_WRITE_TRACE=676:flags` named the survivor
/// (`0x9002000D -> 0x2000D by slot 481 (10,67)`).
/// ⚠ The port CLEARS bit 0 for everyone, collapsing retail's
/// local-wizard `or $1` arm — the same single-player-observable
/// collapse `DEVIATIONS.md` already registers for this release under
/// "flood.rs::flood_shove (action-74 visibility juggle)", now
/// extended to bit 0. Ledger ROUND 99 dig 99-10.
pub(crate) fn no_quake_release_bit0() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_QUAKE_RELEASE_BIT0").is_some())
}

/// The mirror absence on the SET side: `sub_3A200`'s
/// `or edx,0x100001` (NETHERW.EXE 0x5EA16 — the binary's ONLY
/// occurrence of those six bytes) writes byte[0] bit 0 AND byte[2]
/// bit 4 in one instruction. `flood_shove_hit` stamped only
/// `F_TOSSED | F_QUAKE_GRAB`, so a NATIVE toss left the awake gate
/// open and the port woke spheres retail keeps asleep. The two laws
/// are complementary — neither alone is the whole latch.
/// Ledger ROUND 99 dig 99-10.
pub(crate) fn no_quake_toss_bit0() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_QUAKE_TOSS_BIT0").is_some())
}

/// A/B toggle for THE QUAKE MAIL IS A BARE `+=`, NOT THE AREA WRITE
/// PROTOCOL: set `MGC_NO_QUAKE_MAIL_ACCUM` to restore the pre-dig
/// behaviour, where both of `sub_3A200`'s / `sub_39F60`'s mailbox
/// stamps went through [`crate::mc1::mobs::Gen::mail_write`] and so
/// OVERWROTE the amount whenever a reader had already cleared the
/// source. Ledger ROUND 99 dig 99-8.
fn quake_mail_accum_off() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_QUAKE_MAIL_ACCUM").is_some())
}

/// A/B toggle for THE QUAKE KILL MAIL IS A SIGNED 32-BIT ADD: set
/// `MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED` to restore the pre-dig
/// `(life + 1).max(0)` floor in [`Gen::flood_shove_hit`]. Retail's
/// stamp (`NETHERW.EXE` 0x5ea9b — `mov 0x8(%ebx),%eax` / `inc %eax` /
/// `add %eax,%esi`, EF:29435-37) has no floor, so a corpse the flood
/// keeps shoving mails a NEGATIVE amount and REFUNDS its own inbox.
/// Ledger ROUND 149 dig w149f.
fn no_mc2_quake_kill_mail_signed() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED").is_some())
}

/// Retail byte[0] bit0 in the quake band — the tossed/handled latch
/// `sub_3A200` sets (dword |= 0x100001) and the action-74 release
/// clears. A free high bit: 25..30 belong to the mobs/roster MC2
/// band, 29 is the whirlwind grab.
pub(crate) const F_TOSSED: u32 = 1 << 31;

/// The grab latch (retail byte[2] bit4) — the authentic alias of the
/// creature no-corpse bit (helpers doc §5 decode).
pub(crate) const F_QUAKE_GRAB: u32 = super::mobs::F_NO_CORPSE;

/// `sub_10590_terrain_tile_type` & 0x7F0000 (Terrain:2067, helpers
/// doc §7) — the damage pass's burnable-to-lava set: bits 20..22 =
/// types {10,11,12} (the water/lava-edge family) + bits 16..19 =
/// types {21,22,24}/{23}/{25,27}/{26} (the bridge/wall family).
/// DISTINCT from the phase-2/3 `sub_57450` predicate — keep both.
pub(crate) fn burn_flags(t: u8) -> bool {
    matches!(t, 10..=12 | 21..=27)
}

impl Gen {
    // ---- ctor + spawn seam -----------------------------------------------

    /// `sub_51730` (EF:37421) — the (10,67) flood/quake ctor: action
    /// 0x48 = 72, life 120, subSpell 20000, byte[0] = (&0xF6)|1,
    /// map-registered, AABB half-extents (4352, 4352) = ±17 tiles
    /// (the damage pass's overlap box). maxLife is NOT set (unlike
    /// the fissure ctor) — the flood never reads its own maxLife.
    /// No sprite (a terrain effect), no RNG.
    pub(crate) fn mc2_spawn_flood(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        // Retail's ctor makes NO store to +0x2C; this model's
        // port `f44` is that word. See `Gen::mc2_alloc_2c_zero`.
        self.mc2_alloc_2c_zero(i);
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 67;
            e.tick70 = 72;
            e.act_life = 120;
            e.f140 = 20000;
            e.f71 = 0;
            e.flags = (e.flags & !0x9) | 1;
        }
        self.link(i, x, y, z);
        self.mc2_shift_rot(i, 4352, 4352);
        Some(i)
    }

    // ---- terrain helpers ---------------------------------------------------

    /// `GetTerrainHeightFromSquare_48DF0` (EF:32605) — the 4-CORNER
    /// MEAN (truncating `>> 2`) of the box `(x,y)..(x+w,y+h)`. NOT a
    /// max, NOT a box scan (the helpers doc's correction #1).
    fn flood_corner_mean(&self, x: u8, y: u8, w: u8, h: u8) -> i32 {
        (self.t.height[tile(x, y)] as i32
            + self.t.height[tile(x.wrapping_add(w), y)] as i32
            + self.t.height[tile(x.wrapping_add(w), y.wrapping_add(h))] as i32
            + self.t.height[tile(x, y.wrapping_add(h))] as i32)
            >> 2
    }

    /// `sub_439A0` (Terrain:1459) — the MEAN-of-8-neighbours restore
    /// (correction #2: not sum-minus-extremes): gate on the low-3
    /// angle bits, ring sum `>> 3`, then the uint8-WRAP flatness
    /// ladder picks own height / half-blends / the full mean.
    fn flood_settle_cell(&self, x: u8, y: u8) -> u8 {
        let t = tile(x, y);
        let own = self.t.height[t];
        if self.t.angle[t] & 7 == 0 {
            return own;
        }
        let mut max = own;
        let mut min = own;
        let mut sum = 0u32;
        for (dx, dy) in [
            (0i8, -1i8),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ] {
            let h = self.t.height[tile(x.wrapping_add(dx as u8), y.wrapping_add(dy as u8))];
            max = max.max(h);
            min = min.min(h);
            sum += h as u32;
        }
        let mean = sum >> 3;
        if own.wrapping_sub(min) <= 4 {
            if max.wrapping_sub(own) <= 4 {
                own // flat: keep
            } else if max.wrapping_sub(own) <= 10 {
                ((own as u32 + mean) >> 1) as u8 // mild step up: half-blend
            } else {
                mean as u8
            }
        } else if own.wrapping_sub(min) <= 10 {
            ((mean + own as u32) >> 1) as u8 // a mild bump: half-blend
        } else {
            mean as u8 // incl. center-below-neighbours (u8 wrap fails both)
        }
    }

    /// The flood's inline per-cell shading (EF:28626-28654, byte-
    /// identical in the finisher :28962-28992): `H(NW) − H(SE) + 32`
    /// on the raw int, the 28/40 clamp bands, the night flip
    /// `64 − v`. Same formula as the retile pass C but computed
    /// in-place per cell as the sweep runs; on caves it also syncs
    /// bit3 — the sync-only form, NO ceiling pin (EF:28647-28654 /
    /// :28985-28992, byte-identical in both callers).
    fn flood_shade_cell(&mut self, x: u8, y: u8) {
        let nw = self.t.height[tile(x.wrapping_sub(1), y.wrapping_sub(1))] as i32;
        let se = self.t.height[tile(x.wrapping_add(1), y.wrapping_add(1))] as i32;
        let mut v = nw - se + 32;
        if v >= 28 {
            if v > 40 {
                v = (v & 7) + 40;
            }
        } else {
            v = (v & 3) + 28;
        }
        if self.mc2_night_shade.0 {
            v = 64 - v;
        }
        let t = tile(x, y);
        self.t.shading[t] = v as u8;
        if self.is_cave() {
            if self.t.ceiling[t] > self.t.height[t] {
                self.t.angle[t] &= !8;
            } else {
                self.t.angle[t] |= 8;
            }
        }
    }

    // ---- entity passes -------------------------------------------------------

    /// `sub_39FA0` (EF:29214) — the shove-victim filter, the
    /// class/model/flag decision ladder verbatim (helpers doc §2).
    /// `byte[0] & 0x21` = the tossed latch + invisible. Retail's
    /// byte[0] bit 0 has TWO port homes — the positional `flags` bit 0
    /// every native setter writes and the quake band's synthetic
    /// [`F_TOSSED`] — and the test must read BOTH
    /// ([`flood_held_bit0_law`]).
    fn flood_shovable(&self, i: usize, j: usize) -> bool {
        let e = &self.ent[j];
        let bit0 = if flood_held_bit0_law() { 1 } else { 0 };
        let held = e.flags & (F_TOSSED | bit0 | 0x20) != 0;
        match e.class64 {
            1 | 4 | 6 | 7 | 8 | 11 | 12 | 13 | 15 => false,
            2 => true,
            3 => match e.model65 {
                0 => e.id24 != self.ent[i].id24, // skip the CASTER's body
                1 => !held && e.id24 != self.ent[i].id24,
                2 => false, // castles never move
                _ => true,
            },
            5 => {
                !held
                    && e.tick70 != 232
                    && match e.model65 {
                        0x16 => false,
                        27 => !matches!(e.tick70, 233 | 234),
                        _ => true,
                    }
            }
            9 => matches!(e.model65, 0 | 13 | 14),
            10 => matches!(e.model65, 6 | 0x27 | 0x28 | 57),
            14 => !held && e.model65 != 1,
            _ => true, // classes 0 and >15 default shoveable
        }
    }

    /// `sub_3A200` (EF:29382) — the close-range shove callback: tag
    /// the victim (dword |= 0x100001 → [`F_TOSSED`] +
    /// [`F_QUAKE_GRAB`]), then on a 1-in-7 roll of the FLOOD's own
    /// RNG stream (forced for class-5 models 12/0x12, suppressed for
    /// model 27) mail the victim its own `life + 1` — the
    /// near-guaranteed kill. Victim gate `byte_0x38_56 & 1` → f28
    /// bit 0 (the cross-column damage contract). The class-3 model-0
    /// arm writes the record pitch 512 (`NETHERW.EXE` 0x5ea55) — see
    /// `engine::features::no_mc2_flood_human_spin`; its twin store into
    /// the wizard's `pitch_0x157_343` (0x5ea5b) has no per-rival home
    /// (the corpse mover publishes that accumulator as a constant 0).
    fn flood_shove_hit(&mut self, i: usize, j: usize) {
        self.ent[j].flags |= F_TOSSED | F_QUAKE_GRAB;
        if !no_quake_toss_bit0() {
            self.ent[j].flags |= 1;
        }
        let (class, model) = (self.ent[j].class64, self.ent[j].model65);
        if class == 3 && model == 0 && !crate::engine::features::no_mc2_flood_human_spin() {
            self.ent[j].f32 = 512;
        }
        let mut forced = false;
        let mut suppressed = false;
        if class == 5 {
            match model {
                12 | 0x12 => forced = true,
                27 => suppressed = true,
                _ => {}
            }
        }
        let rolled = forced || self.ent_rand(i) % 7 == 0;
        if rolled && !suppressed && self.ent[j].f28 & 1 != 0 {
            // ⭐⭐⭐ RETAIL ADDS `life + 1` WITH NO FLOOR, AND THE
            // "never in practice" ARM IS IN THE CORPUS. The comment
            // that used to stand here read "the u32 mail clamps the
            // never-in-practice life < −1 arm to 0 (NOT .max(1))" —
            // and mc2l0-spells-galore t=22008 slot 296 refutes it.
            // The shove callback has NO life test on the victim
            // (`sub_3A200`, the only gates are the 1-in-7 roll, the
            // model-27 suppression and `byte_0x38_56 & 1`), so it
            // keeps shoving a CORPSE — here a (3,3) mana balloon the
            // same flood killed one tick earlier (life 10000 →
            // −10002 at t=22007, its own tail `sub_60EA0` EF:62336
            // `if (life < 0) return;` so nothing ever drains the
            // box again). Retail's next mail is life+1 = **−10001**
            // and the add is a plain 32-bit `add`, so the inbox goes
            // 20002 → 10001; the `.max(0)` made the port add nothing
            // and it stood at 20002.
            // Shipped `NETHERW.EXE` file **0x5ea9b** (linear
            // 0x3A200 + 0x24800), the whole stamp:
            //   5ea95  f6 43 38 01   testb $0x1,0x38(%ebx) ; f28 bit0
            //   5ea99  74 22         je    0x5eabd
            //   5ea9b  8b 43 08      mov   0x8(%ebx),%eax  ; life, 32-bit
            //   5ea9e  8b 73 5e      mov   0x5e(%ebx),%esi ; mail0 AMT
            //   5eaa1  40            inc   %eax            ; life + 1
            //   5eaa2  01 c6         add   %eax,%esi       ; NO cmp, NO cmov
            //   5eaa4  89 73 5e      mov   %esi,0x5e(%ebx)
            // — no floor instruction anywhere between the load and
            // the store. `MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED=1`
            // restores the clamp.
            let amt = if no_mc2_quake_kill_mail_signed() {
                (self.ent[j].act_life + 1).max(0) as u32
            } else {
                self.ent[j].act_life.wrapping_add(1) as u32
            };
            let id = self.ent[i].id24;
            if quake_mail_accum_off() {
                self.mail_write(MailTarget::Pool(j), 0, amt, id);
            } else {
                // ⭐⭐⭐ THIS IS NOT THE AREA WRITE PROTOCOL. Retail's
                // two statements are a BARE `+=` and a BARE `=`:
                // `NETHERW.EXE` 0x5EA9B (linear 0x3A200 + 0x24800)
                //   8b 43 08   mov 0x8(%ebx),%eax    ; victim life
                //   8b 73 5e   mov 0x5e(%ebx),%esi   ; victim mail0 AMT
                //   40         inc %eax
                //   01 c6      add %eax,%esi         ; AMT += life+1
                //   89 73 5e   mov %esi,0x5e(%ebx)
                //   66 8b 41 1a mov 0x1a(%ecx),%ax
                //   66 89 43 62 mov %ax,0x62(%ebx)   ; SRC = id
                // — there is no test of the source word 0x62 anywhere
                // in the block (EF:29435-37). `Gen::mail_write`'s
                // stale-source branch OVERWRITES, so every quake kill
                // mail landing on a consumed mailbox threw the residue
                // away.
                self.ent[j].mail[0].0 = self.ent[j].mail[0].0.wrapping_add(amt);
                self.ent[j].mail[0].1 = id;
            }
            // +1 per near-guaranteed kill (EF:29437).
            if id == crate::mc1::mobs::PLAYER_TARGET {
                self.mc2_cast_xp.0.push((id, 20, 1));
            }
        }
    }

    /// `sub_39B60` (EF:29011) — the radius entity-shove: a 26×26
    /// spatial-hash window (origin center−13) gated by the SQUARED
    /// disc `dist² < 0xA90000` (= 3328², 13 tiles;
    /// `EuclideanDistXY_584D0` returns dist² — correction #3's
    /// sibling); per victim passing the filter, in true range
    /// `dist < 3328` and below the ceiling `z − ref < 4096`:
    /// very-close victims (`dist ≤ 32` or `z − ref ≤ 96`) take the
    /// damage callback, the rest are walked TOWARD the center by
    /// `(3328−d)·128/3328` clamped [4,128] capped to d, pulled down
    /// `48·((4096−(z−ref))·256 >> 12) >> 8`, and ground-clamped. In
    /// action-74 mode every grabbed entity in the disc has its
    /// tossed + grab latches released.
    fn flood_shove(&mut self, i: usize, ctx: &MobCtx) {
        let (ex, ey, ez, id, refz, action74) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f44 as i32, e.tick70 == 74)
        };
        let cx = (ex.wrapping_add(128) >> 8) as u8;
        let cy = (ey.wrapping_add(128) >> 8) as u8;
        let mut steps = 0usize;
        let human_seat = !crate::engine::features::no_mc2_flood_human_seat();
        let mut hp = (ctx.px, ctx.py, ctx.pz);
        let mut hmoved = false;
        for row in 0..26u8 {
            let ty = cy.wrapping_sub(13).wrapping_add(row);
            for col in 0..26u8 {
                let tx = cx.wrapping_sub(13).wrapping_add(col);
                let (wx, wy) = ((tx as i32) << 8, (ty as i32) << 8);
                if Self::dist2_sq(ex, ey, wx as u16, wy as u16) >= 0xA9_0000 {
                    continue;
                }
                let mut j = self.map_entity[tile(tx, ty)] as usize;
                // ⭐⭐⭐ THE HUMAN IS SHOVED AT HIS SEAT IN THIS WALK
                // (`no_mc2_flood_human_seat`). `cur` is the tile whose
                // chain the cursor is on — it changes when a relinked
                // victim (pooled or the human) hands the walk its
                // destination chain.
                let mut cur = tile(tx, ty);
                let mut seat_block = usize::MAX;
                // He heads `cur` after his own relink, so its TAIL is
                // not his seat until a pooled relink re-heads it.
                let mut headed = false;
                'cell: loop {
                    let mut resume = 0usize;
                    let mut seat_matched = false;
                    while j != 0 {
                        if human_seat
                            && seat_block != j
                            && self.player_chain.next as usize == j
                            && self.player_chain.cell == tile((hp.0 >> 8) as u8, (hp.1 >> 8) as u8)
                        {
                            resume = j;
                            seat_matched = true;
                            break;
                        }
                        seat_block = usize::MAX;
                        let next = self.ent[j].next20 as usize;
                        // ⭐⭐⭐ AN INVENTED GUARD: THE SHOVE LOOP HAS NO
                        // REAP TEST. `sub_39B60`'s chain walk is
                        // `for (i = mapEntityIndex[cell]; ...; i = next)
                        //  { if (sub_39FA0(flood, Entities[i])) ... }` and
                        // NOTHING else — `NETHERW.EXE` 0x5E40A-0x5E419 is
                        // `push %ebx / mov 0x14(%ebp),%edx / push %edx /
                        //  call 0x5e7a0 / test %al,%al / je LABEL_25`, with
                        // no flag load in between; and `sub_39FA0` itself
                        // (0x5E7A0-0x5E88A, a jump table over `class - 1`,
                        // disassembled in full) tests ONLY
                        // `testb $0x21,0xc(%edx)` — byte[0] bits 0 and 5,
                        // the tossed and invisible latches. There is no
                        // `testb $0x4,0xd(...)` anywhere in either, i.e. no
                        // `flags & 0x400` reap term. A record reap-flagged
                        // EARLIER IN THE SAME POOL WALK is still on the map
                        // chain (`sub_57F20` unlinks it on the NEXT frame's
                        // pre-walk), so retail shoves the fresh corpse and
                        // the port did not.
                        // WITNESS mc2l6-rsg t=26,325: creature slot 99 is
                        // reap-flagged by its own tick (slot 99 < flood 496
                        // in the ascending walk) and retail's flood then
                        // pushes it (11684,9866) -> (11656,9984), snaps it
                        // to the terrain at 119 — and on the SECOND visit
                        // of the same 26x26 sweep the now-low `v5` = 82
                        // takes the close band, stamping `byte[2] bit4`
                        // (the grab) and drawing the 1-in-7 roll
                        // (`rand 10536 -> 42759`). The port's slot 99 sat
                        // still with no grab and no draw.
                        if (self.ent[j].flags & 0x400 == 0 || flood_no_reap_gate_law()) && j != i {
                            let d = dist2d(ex, ey, self.ent[j].x as i32, self.ent[j].y as i32);
                            let v5 = self.ent[j].z as i32 - refz;
                            if self.flood_shovable(i, j) && d < 3328 && v5 < 4096 {
                                if d <= 32 || v5 <= 96 {
                                    self.flood_shove_hit(i, j);
                                } else {
                                    let mut v6 = (((3328 - d) << 8) / 3328) << 7 >> 8;
                                    v6 = v6.clamp(4, 128).min(d);
                                    let (vx, vy, vz) = {
                                        let e = &self.ent[j];
                                        (e.x, e.y, e.z)
                                    };
                                    let yaw = Self::angle_between(vx, vy, ex, ey);
                                    let mut pos = (vx, vy, vz);
                                    Self::polar_step(&mut pos, yaw, 0, v6 as i16);
                                    // ⭐⭐⭐ THE VERTICAL LEG IS A THREE-WAY
                                    // SPLIT, AND THE PORT CARRIED ONE ARM.
                                    // Retail takes the terrain altitude at
                                    // the STEPPED point FIRST and only then
                                    // asks who the victim is (EF:29092-108;
                                    // `NETHERW.EXE` 0x5E4E7-0x5E58C):
                                    //   0x5e4e7 call 0x35440        ; getTerrainAlt(predicted)
                                    //   0x5e4ec movswl %ax,%ecx     ; v8 = ground
                                    //   0x5e4ef mov 0x3f(%ebx),%ah  ; victim class
                                    //   0x5e4f5 cmp $0x3,%ah   ; jne 0x5e53c
                                    //   0x5e4fa cmpb $0x0,0x40(%ebx) ; jne 0x5e53c
                                    //   <arm 1: WIZARD — pull, then clamp>
                                    //   0x5e53c mov 0xa0(%ebx),%eax  ; the victim's Type_160 row
                                    //   0x5e542 movswl 0xe(%eax),%eax ; word_160_0xe_14, SIGNED
                                    //   0x5e546 cmp $0xffffffc0,%eax  ; -64
                                    //   0x5e549 jl  0x5e585           ; -> LABEL_40, NO PULL
                                    //   <arm 3: pull, then clamp>
                                    //   0x5e581 cmp %ecx,%eax ; jge 0x5e58c
                                    //   0x5e585 mov %cx,0x1b39c       ; predicted.z = ground
                                    // So a victim that is NOT a class-3
                                    // model-0 wizard and whose behaviour row
                                    // sinks faster than -64 per tick is
                                    // PLANTED ON THE TERRAIN OUTRIGHT — the
                                    // smooth pull is for flyers (and the
                                    // wizard body), never for a walker. The
                                    // difference is not cosmetic: the pull
                                    // arm can only ever LOWER z toward the
                                    // clamp, so a walker dragged UPHILL kept
                                    // the pre-step altitude and hung above
                                    // the slope. 40 of the 157 shipped rows
                                    // are below -64 (every -128/-256/-512
                                    // walker).
                                    // ⚠ `word_160_0xe_14` DOES have a
                                    // ported home and always did — it is
                                    // `Mc2BehaviorRow::v_14`, the z step
                                    // `Gen::mc2_alt_core` uses on every
                                    // creature move. The module header and
                                    // docs/DEVIATIONS.md both claimed
                                    // otherwise; both corrected.
                                    let ground = self.ground_z(pos.0, pos.1);
                                    let wizard = {
                                        let e = &self.ent[j];
                                        e.class64 == 3 && e.model65 == 0
                                    };
                                    let deep_sink =
                                        BEHAVIOR[self.ent[j].row156 as usize].v_14 < -64;
                                    if !wizard && deep_sink && flood_ground_snap_law() {
                                        pos.2 = ground as i16;
                                    } else {
                                        let pull = (48 * (((4096 - v5) << 8) >> 12)) >> 8;
                                        pos.2 = (pos.2 as i32 - pull) as i16;
                                        if (pos.2 as i32) < ground {
                                            pos.2 = ground as i16;
                                        }
                                    }
                                    let before = tile(
                                        (self.ent[j].x >> 8) as u8,
                                        (self.ent[j].y >> 8) as u8,
                                    );
                                    self.move_relink(j, pos.0, pos.1, pos.2);
                                    let after = tile((pos.0 >> 8) as u8, (pos.1 >> 8) as u8);
                                    if before != after && flood_chain_rewalk_law() {
                                        cur = after;
                                        headed = false;
                                    }
                                }
                            }
                            // The action-74 grab release (LABEL_25) runs
                            // for EVERY entity in the disc.
                            if action74 && self.ent[j].flags & F_QUAKE_GRAB != 0 {
                                self.ent[j].flags &= !(F_TOSSED | F_QUAKE_GRAB);
                                if !no_quake_release_bit0() {
                                    self.ent[j].flags &= !1;
                                }
                            }
                        }
                        // ⭐⭐⭐ THE CURSOR IS RE-READ OFF THE MOVED VICTIM
                        // (`NETHERW.EXE` 0x5e5e6, disassembled at
                        // [`flood_chain_rewalk_law`]). A shove that crosses
                        // a tile edge re-heads the victim in the DESTINATION
                        // cell, so retail's walk carries on down THAT chain.
                        j = if flood_chain_rewalk_law() {
                            self.ent[j].next20 as usize
                        } else {
                            next
                        };
                        // Retail has no bound here; ours only exists so a
                        // pathological cycle cannot hang the harness. It
                        // has never fired.
                        steps += 1;
                        if steps > 1_000_000 {
                            break;
                        }
                    }
                    if !human_seat {
                        break;
                    }
                    // ── THE HUMAN'S VISIT ── at his seat (`resume`), or as
                    // the TAIL of the chain the cursor is on when his seat
                    // is that tail (`next == 0`) or is not a live seat.
                    let htile = tile((hp.0 >> 8) as u8, (hp.1 >> 8) as u8);
                    let seat_live = self.player_chain.cell == htile;
                    if resume != 0
                        || (!seat_matched
                            && !headed
                            && cur == htile
                            && (!seat_live || self.player_chain.next == 0))
                    {
                        self.flood_shove_human(i, &mut hp, &mut hmoved);
                        // `CopyEntityPosition_57CF0` (0x5e592) relinks
                        // HIM: a new tile makes him its chain head, and the
                        // cursor (0x5e5e6) carries on down THAT chain from
                        // the head he displaced.
                        self.player_relink(hp.0, hp.1);
                        let ntile = tile((hp.0 >> 8) as u8, (hp.1 >> 8) as u8);
                        if ntile != htile && flood_chain_rewalk_law() {
                            cur = ntile;
                            headed = true;
                            j = self.map_entity[ntile] as usize;
                            seat_block = j;
                            continue 'cell;
                        }
                    }
                    if resume != 0 {
                        seat_block = resume;
                        j = resume;
                        continue 'cell;
                    }
                    break;
                }
            }
        }
        let _ = ez;
        if human_seat {
            // Publish the accumulated delta. Several quakes in one walk
            // ADD (each one's `ctx` is the pose the previous ones left,
            // via the walk hook's re-adoption).
            if hmoved {
                let f = &mut self.player_flood_pull;
                f.armed = true;
                f.fresh = true;
                f.dx += hp.0.wrapping_sub(ctx.px) as i16 as i32;
                f.dy += hp.1.wrapping_sub(ctx.py) as i16 as i32;
                f.pull += ctx.pz as i32 - hp.2 as i32;
            }
            return;
        }
        // The human player arm (module doc APPROX): the pull rides
        // the knock channel, the close band rolls the 1-in-7 kill.
        // Same-owner gate (`sub_39FA0` EF:29261 — the pool filter
        // `flood_shovable` already ports it; the special-cased human
        // arm was missing it): your OWN Gravity Well never pulls
        // you in, and can never roll the 32000 kill on your cast.
        let pd = dist2d(ex, ey, ctx.px as i32, ctx.py as i32);
        let pv5 = ctx.pz as i32 - refz;
        if pd < 3328 && pv5 < 4096 && id != crate::mc1::mobs::PLAYER_TARGET {
            if pd <= 32 || pv5 <= 96 {
                if !crate::engine::features::no_mc2_flood_human_spin() {
                    self.player_flood_pull.spin = true;
                }
                if self.ent_rand(i) % 7 == 0 {
                    self.mail_write(MailTarget::Player, 0, 32000, id);
                }
            } else {
                let mut v6 = (((3328 - pd) << 8) / 3328) << 7 >> 8;
                v6 = v6.clamp(4, 128).min(pd);
                let toward = Self::angle_between(ctx.px, ctx.py, ex, ey);
                self.player_knock = (toward, v6 as i16);
                // ⭐⭐ AND THE VERTICAL LEG, which the pool arm forty
                // lines above has had since round 98. Retail runs ONE
                // body for every victim (`sub_39B60`); its class-3
                // model-0 arm is `predicted.z -= pull` + the stepped
                // point's terrain floor. The human's out-of-pool seat
                // means the horizontal rides `player_knock` and this
                // rides its own one-shot, drained by the carpet's
                // dispatch — the `player_hurl` transport.
                //
                // ⚠ The terrain floor is NOT carried here: the mover's
                // own block-9 resolution raises anything below
                // `ground + clearance` back to it, and that bound is
                // STRICTLY ABOVE retail's bare `getTerrainAlt`, so the
                // clamp is dominated on this seat.
                if !no_mc2_flood_human_z_pull() {
                    self.player_flood_pull = crate::engine::features::PlayerFloodPull {
                        armed: true,
                        pull: (48 * (((4096 - pv5) << 8) >> 12)) >> 8,
                        bearing: toward,
                        dist: v6 as i16,
                        ..Default::default()
                    };
                }
            }
        }
    }

    /// ONE visit of `sub_39B60`'s body to the human wizard, at his
    /// seat ([`crate::engine::features::no_mc2_flood_human_seat`]).
    /// `sub_39FA0`'s class-3 model-0 filter is the same-owner test
    /// alone; the close band rolls the 1-in-7 kill (the module-doc
    /// APPROX `32000` scale), the shove band is the WIZARD arm verbatim
    /// (`NETHERW.EXE` 0x5e4ef..0x5e592): `MoveEntity_57FA0` toward the
    /// quake, `z -= pull`, floored at the stepped point's
    /// `getTerrainAlt`, written to `predictedAxis` and the record.
    fn flood_shove_human(&mut self, i: usize, hp: &mut (u16, u16, i16), moved: &mut bool) {
        let (ex, ey, id, refz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24, e.f44 as i32)
        };
        if id == crate::mc1::mobs::PLAYER_TARGET {
            return;
        }
        let pd = dist2d(ex, ey, hp.0 as i32, hp.1 as i32);
        let pv5 = hp.2 as i32 - refz;
        if pd >= 3328 || pv5 >= 4096 {
            return;
        }
        if pd <= 32 || pv5 <= 96 {
            // `sub_3A200`'s class-3 model-0 arm (0x5ea49..0x5ea62),
            // ahead of the roll: `pitch = pitch_acc = 512`.
            if !crate::engine::features::no_mc2_flood_human_spin() {
                self.player_flood_pull.spin = true;
            }
            if self.ent_rand(i) % 7 == 0 {
                self.mail_write(MailTarget::Player, 0, 32000, id);
            }
            return;
        }
        let mut v6 = (((3328 - pd) << 8) / 3328) << 7 >> 8;
        v6 = v6.clamp(4, 128).min(pd);
        let toward = Self::angle_between(hp.0, hp.1, ex, ey);
        let mut pos = *hp;
        Self::polar_step(&mut pos, toward, 0, v6 as i16);
        let ground = self.ground_z(pos.0, pos.1);
        let pull = (48 * (((4096 - pv5) << 8) >> 12)) >> 8;
        let z = (pos.2 as i32 - pull) as i16;
        pos.2 = if (z as i32) < ground {
            ground as i16
        } else {
            z
        };
        self.mc2_pred_axis = crate::engine::features::Mc2PredAxis(pos);
        *hp = pos;
        *moved = true;
    }

    /// `CompareAxisWithShift_10750` (EF:3726/3733, helpers doc §8) —
    /// XY-ONLY Minkowski AABB overlap, strict `<`, NO z term (our
    /// generic `ent_overlap` adds one — and the flood's z rides in
    /// HEIGHTMAP units, so the z test would falsely exclude
    /// everything). Shared by the whirlwind contact pass (mc2::tail)
    /// — the same retail helper.
    pub(crate) fn mc2_overlap_xy(&self, a: usize, b: usize) -> bool {
        let (ea, eb) = (&self.ent[a], &self.ent[b]);
        let wd = |p: u16, q: u16| (p.wrapping_sub(q) as i16 as i32).abs();
        wd(ea.x, eb.x) < ea.f80 as i32 + eb.f80 as i32
            && wd(ea.y, eb.y) < ea.f82 as i32 + eb.f82 as i32
    }

    /// `sub_3A090` (EF:29316) — the one-shot damage/grab pass
    /// (phase-2 countdown step 5 and finisher phase 0): ERASE every
    /// overlapping village BUILDING — the `dword_38527` list is the
    /// class-10 MODEL-45 list (the builder EF:40043-40052), not a
    /// generic effect list — (life = −1, fontType = 0); GRAB every
    /// overlapping CASTLE (class-3 model-2 — the dword_38519 list):
    /// grab latch + 30-tick shake (`word_0x30_48` = our f50 castle-
    /// shake home) + owner = self slot + the subSpell (20000) damage
    /// mail — NO owner immunity: the caster's own castle takes it
    /// too (EF:29339-29348 has no id gate); then the 30×30
    /// `0x7F0000`-family terrain sweep to lava. Of the
    /// `sub_6D8B0(id, 0x14, 2n)` XP report only the player push below
    /// is ported (module doc APPROX).
    fn flood_damage_pass(&mut self, i: usize) {
        let (id, amt) = (self.ent[i].id24, self.ent[i].f140 as u32);
        let mut buildings: Vec<usize> = Vec::new();
        let mut castles: Vec<usize> = Vec::new();
        for j in 1..self.ent.len() {
            let c = &self.ent[j];
            if j == i || c.flags & 0x400 != 0 || c.class64 == 0 {
                continue;
            }
            if c.class64 == 10 && c.model65 == 45 && self.mc2_overlap_xy(i, j) {
                buildings.push(j);
            } else if c.class64 == 3
                && c.model65 == 2
                && c.act_life >= 0
                && self.mc2_overlap_xy(i, j)
            {
                castles.push(j);
            }
        }
        for j in buildings {
            self.ent[j].act_life = -1;
            self.ent[j].f46 = 0; // fontTypeIndex_0x3D_61
        }
        let castles_hit = castles.len() as i32;
        for j in castles {
            self.ent[j].flags |= F_QUAKE_GRAB;
            self.ent[j].f50 = 30; // the grab timer = the blast shake
            self.ent[j].f40 = i as u16; // owner = self slot
            // ⭐ THE SECOND CALL PATH OF THE SAME LAW. EF:29348-49 is
            // the identical bare pair — `jx->dword_0x5E_94 +=
            // a1x->subSpellIndex_0x2A_42;` … `jx->word_0x62_98 =
            // a1x->id_0x1A_26;` — with no test of the source word.
            if quake_mail_accum_off() {
                self.mail_write(MailTarget::Pool(j), 0, amt, id);
            } else {
                self.ent[j].mail[0].0 = self.ent[j].mail[0].0.wrapping_add(amt);
                self.ent[j].mail[0].1 = id;
            }
        }
        // +2 per grabbed CASTLE (EF:29374 `v8 += 2`; buildings do
        // NOT count).
        if castles_hit != 0 && id == crate::mc1::mobs::PLAYER_TARGET {
            self.mc2_cast_xp.0.push((id, 20, 2 * castles_hit));
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                let t = tile(x, y);
                if burn_flags(self.t.tile_type[t]) {
                    self.t.tile_type[t] = 1;
                    self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                }
            }
        }
    }

    // ---- the phase machine ---------------------------------------------------

    /// `sub_39E40` (EF:29133) — the init probe: abort if ≥ 225 of
    /// the 30×30 footprint cells are open ground (type 0), or if
    /// another quake — class-10 model 0x2D (a building) in action
    /// 48/51, or another model-67 — is live within the 54×54 window.
    fn flood_probe(&mut self, i: usize) -> bool {
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let mut open = 0u32;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                if self.t.tile_type[tile(x, y)] == 0 {
                    open += 1;
                }
            }
        }
        if open >= 225 {
            return false;
        }
        for dy in 0..54u8 {
            let y = cy.wrapping_sub(27).wrapping_add(dy);
            for dx in 0..54u8 {
                let x = cx.wrapping_sub(27).wrapping_add(dx);
                let mut j = self.map_entity[tile(x, y)] as usize;
                while j != 0 {
                    let e = &self.ent[j];
                    if j != i
                        && e.class64 == 10
                        && (e.model65 == 67 || (e.model65 == 0x2D && matches!(e.tick70, 48 | 51)))
                    {
                        return false;
                    }
                    j = e.next20 as usize;
                }
            }
        }
        true
    }

    /// The phase-2 morph body (EF:28553-28701) — runs from phase 1's
    /// fall-through and every phase-2 tick. Returns terrain-dirty.
    fn flood_morph(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].f26 -= 1;
        let cd = self.ent[i].f26 as i32;
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        let mut dirty = false;
        if cd <= 0 {
            self.ent[i].f71 = 3;
        } else {
            let (ex, ey, ez) = {
                let e = &self.ent[i];
                (e.x, e.y, e.z as i32)
            };
            let mut relight = false;
            for dy in 0..30u8 {
                let y = cy.wrapping_sub(15).wrapping_add(dy);
                for dx in 0..30u8 {
                    let x = cx.wrapping_sub(15).wrapping_add(dx);
                    let (wx, wy) = ((x as i32) << 8, (y as i32) << 8);
                    let d = dist2d(ex, ey, wx, wy);
                    if d < 3840 {
                        let target = if d >= 2304 {
                            // OUTER ring: blend rim height → dome top
                            // on the raised cosine.
                            let yaw = Self::angle_between(ex, ey, wx as u16, wy as u16);
                            let mut rim = (ex, ey, 0i16);
                            Self::polar_step(&mut rim, yaw, 0, 3840);
                            let v11 = self.ground_z(rim.0, rim.1) >> 5;
                            if (self.ent[i].f44 as i32) < v11 {
                                self.ent[i].f44 = v11 as u16;
                            }
                            let cos =
                                SIN_DB750[0x200 + (((d - 2304) << 10) / 1536) as usize] as i64;
                            v11 - ((((0x10000 + cos) >> 1) * (v11 - (ez + 64)) as i64) >> 16) as i32
                        } else {
                            // INNER disc: the dome top with the
                            // center dip (the crater ring profile).
                            let cos = SIN_DB750[0x200 + (((2304 - d) << 9) / 2304) as usize] as i64;
                            ez + 64 - ((((0x10000 - cos) << 6) >> 16) as i32)
                        };
                        let t = tile(x, y);
                        let cur = self.t.height[t] as i32;
                        let h = ((target - cur) / cd + cur).clamp(1, 255);
                        self.t.height[t] = h as u8;
                        // Cave ceiling ease toward floor + 64
                        // clearance, /life, u8-truncated
                        // (EF:28604-28614).
                        if self.is_cave() {
                            let tgt = (h + 64).min(254);
                            let c = self.t.ceiling[t] as i32;
                            self.t.ceiling[t] = (c - (c - tgt) / cd) as u8;
                        }
                        if h <= ez + 64 && h >= ez + 6 * cd && auto_flat(self.t.tile_type[t]) {
                            relight = true;
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                    self.flood_shade_cell(x, y); // EVERY cell, in-place
                }
            }
            if cd == 5 {
                self.flood_damage_pass(i);
                relight = true;
            }
            if relight {
                self.mc2_retile_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
            }
            // The 2×2 crater floor (EF:28672-28696): drop the center
            // by 1/countdown of itself each tick.
            for dy in 0..2u8 {
                let y = cy.wrapping_sub(1).wrapping_add(dy);
                for dx in 0..2u8 {
                    let x = cx.wrapping_sub(1).wrapping_add(dx);
                    let t = tile(x, y);
                    let h = self.t.height[t] as i32;
                    self.t.height[t] = (h - h / cd).clamp(0, 255) as u8;
                    let s = if self.mc2_night_shade.0 { -31 } else { 31 };
                    self.t.shading[t] = (s / cd + 32) as u8;
                }
            }
            dirty = true;
        }
        if self.ent[i].f26 < 6 {
            self.flood_shove(i, ctx);
        }
        dirty
    }

    /// `sub_39040` (EF:28515) — the action-72 driver: life countdown
    /// with the finisher shortcut, then the phase switch — 0 probe,
    /// 1 the 18×18 4-corner-mean sample + arm (falls into 2), 2 the
    /// dome morph, 3 the lava/relight commit → action 73. Returns
    /// terrain-dirty.
    pub(crate) fn mc2_flood_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life <= 0 {
            self.ent[i].tick70 = 74;
            self.ent[i].f71 = 0;
            return false;
        }
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        match self.ent[i].f71 {
            0 => {
                if self.flood_probe(i) {
                    self.ent[i].f71 = 1;
                } else {
                    self.ent[i].flags |= 0x400; // DisableEntityDrawing04
                }
                false
            }
            1 => {
                // Phase 1 (EF:28539-28552): z = corner-mean − 64,
                // ref = 32·(mean − 80) in world units — the mixed-
                // unit max update in the morph is retail's own quirk
                // (helpers doc OPEN-4), ported verbatim.
                let v5 = self.flood_corner_mean(cx.wrapping_sub(9), cy.wrapping_sub(9), 18, 18);
                self.ent[i].z = 0;
                self.ent[i].f44 = 0;
                if v5 > 64 {
                    self.ent[i].z = (v5 - 64) as i16;
                    if v5 - 64 > 16 {
                        self.ent[i].f44 = (32 * (v5 - 80)) as u16;
                    }
                }
                self.ent[i].f71 = 2;
                self.ent[i].f26 = 12;
                self.snd(64, i);
                self.flood_morph(i, ctx) // retail falls through
            }
            2 => self.flood_morph(i, ctx),
            3 => {
                // Phase 3 (EF:28702-28751): commit — everything
                // burnable (or type 8) in the 30×30 → lava, retile,
                // force the crater-floor shading, final shove, → 73.
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        let t = tile(x, y);
                        let tt = self.t.tile_type[t];
                        if auto_flat(tt) || tt == 8 {
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                }
                self.mc2_add_building_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
                for dy in 0..2u8 {
                    let y = cy.wrapping_sub(1).wrapping_add(dy);
                    for dx in 0..2u8 {
                        let x = cx.wrapping_sub(1).wrapping_add(dx);
                        self.t.shading[tile(x, y)] = if self.mc2_night_shade.0 { 1 } else { 63 };
                    }
                }
                self.flood_shove(i, ctx);
                self.ent[i].tick70 = 73;
                true
            }
            _ => false,
        }
    }

    /// `sub_396A0` (EF:28764) — action 73: hold the shove while life
    /// runs down, then hand to the restore finisher.
    pub(crate) fn mc2_flood_shove_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].act_life -= 1;
        if self.ent[i].act_life > 0 {
            self.flood_shove(i, ctx);
        } else {
            self.ent[i].tick70 = 74;
            self.ent[i].f71 = 0;
        }
    }

    /// The finisher's settle body (EF:28911-28995): gated on
    /// `life & 3` (life sits at 0 on entry, so it fires every tick —
    /// the 16-step countdown is 16 ticks); eases each disc cell
    /// toward the rim-referenced raised cosine + jitter, snapping to
    /// the neighbour mean in the last 2 steps, and recomputes the
    /// shading for every cell.
    fn flood_settle(&mut self, i: usize) -> bool {
        if self.ent[i].act_life & 3 != 0 {
            return false;
        }
        self.ent[i].f26 -= 1;
        let cd = self.ent[i].f26 as i32;
        if cd <= 0 {
            self.ent[i].f71 = 2;
            return false;
        }
        let (ex, ey, ez) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z as i32)
        };
        let cx = (ex.wrapping_add(128) >> 8) as u8;
        let cy = (ey.wrapping_add(128) >> 8) as u8;
        for dy in 0..30u8 {
            let y = cy.wrapping_sub(15).wrapping_add(dy);
            for dx in 0..30u8 {
                let x = cx.wrapping_sub(15).wrapping_add(dx);
                let (wx, wy) = ((x as i32) << 8, (y as i32) << 8);
                let d = dist2d(ex, ey, wx, wy);
                if d < 3840 {
                    let yaw = Self::angle_between(ex, ey, wx as u16, wy as u16);
                    let mut rim = (ex, ey, 0i16);
                    Self::polar_step(&mut rim, yaw, 0, 3840);
                    let g = self.ground_z(rim.0, rim.1) >> 5;
                    let cos = SIN_DB750[0x200 + ((d << 10) / 3840) as usize] as i64;
                    let v13 = g - ((((0x10000 + cos) >> 1) * (g - ez) as i64) >> 16) as i32;
                    let v14 = (self.ent_rand(i) & 3) as i32 + v13 - 2;
                    let t = tile(x, y);
                    let h = self.t.height[t] as i32;
                    self.t.height[t] = (h + (v14 - h) / cd).clamp(1, 255) as u8;
                    if cd < 3 {
                        self.t.height[t] = self.flood_settle_cell(x, y);
                    }
                    // Cave ceiling ease toward floor + 64, /life
                    // (EF:28954-28961).
                    if self.is_cave() {
                        let tgt = (self.t.height[t] as i32 + 64).min(254);
                        let c = self.t.ceiling[t] as i32;
                        self.t.ceiling[t] = (c - (c - tgt) / cd) as u8;
                    }
                }
                self.flood_shade_cell(x, y);
            }
        }
        true
    }

    /// `sub_396D0` (EF:28783) — the action-74 RESTORE finisher:
    /// phase 0 = shove + damage pass + finish the lava conversion +
    /// arm the 16-step restore (falls into the settle); phase 1 =
    /// settle; phase 2 = snap the whole footprint to the neighbour
    /// mean, release the grabbed castles, despawn. Returns
    /// terrain-dirty.
    pub(crate) fn mc2_flood_finisher_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let cx = (self.ent[i].x.wrapping_add(128) >> 8) as u8;
        let cy = (self.ent[i].y.wrapping_add(128) >> 8) as u8;
        match self.ent[i].f71 {
            0 => {
                self.flood_shove(i, ctx);
                self.flood_damage_pass(i);
                self.ent[i].f71 = 1;
                self.ent[i].f26 = 16;
                self.ent[i].z = self.ent[i].z.wrapping_add(64);
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        let t = tile(x, y);
                        if auto_flat(self.t.tile_type[t]) {
                            self.t.tile_type[t] = 1;
                            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
                        }
                    }
                }
                self.mc2_retile_region(
                    cx.wrapping_sub(15),
                    cy.wrapping_sub(15),
                    cx.wrapping_add(15),
                    cy.wrapping_add(15),
                );
                self.snd(64, i);
                self.flood_settle(i); // retail falls through
                true
            }
            1 => self.flood_settle(i),
            2 => {
                // Settle every cell to the neighbour mean, IN PLACE
                // in scan order (later cells see earlier writes —
                // EF:28884-28895).
                for dy in 0..30u8 {
                    let y = cy.wrapping_sub(15).wrapping_add(dy);
                    for dx in 0..30u8 {
                        let x = cx.wrapping_sub(15).wrapping_add(dx);
                        self.t.height[tile(x, y)] = self.flood_settle_cell(x, y);
                    }
                }
                // Release every castle this flood grabbed
                // (EF:28898-28908): owner + grab bit only (the shake
                // timer expires on its own).
                for j in 1..self.ent.len() {
                    let c = &self.ent[j];
                    if c.class64 == 3
                        && c.model65 == 2
                        && c.flags & F_QUAKE_GRAB != 0
                        && c.f40 == i as u16
                    {
                        self.ent[j].f40 = 0;
                        self.ent[j].flags &= !F_QUAKE_GRAB;
                    }
                }
                self.ent[i].flags |= 0x400; // despawn
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat 100-height MC2 world — enough for the quake's own pass:
    /// the human arm runs AFTER the 26x26 map-chain walk and reads
    /// only the ctx, so an empty pool is the right fixture for it.
    fn flood_gen() -> Gen {
        let planes = crate::engine::features::Planes {
            height: vec![100; 0x10000],
            tile_type: vec![1; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![0; 0x10000],
            ceiling: Vec::new(),
        };
        let assets = crate::engine::features::FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
            mc1_sprite_ext: Vec::new(),
        };
        Gen::new(
            planes,
            assets,
            1,
            crate::chassis::ChassisParams::MC2,
            crate::verbs::VerbSet::MC2,
        )
    }

    fn flood_ctx(px: u16, py: u16, pz: i16) -> MobCtx {
        MobCtx {
            px,
            py,
            pz,
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

    /// ⭐⭐⭐ A CORPSE THE FLOOD KEEPS SHOVING **REFUNDS** ITS OWN
    /// INBOX — the kill mail is `life + 1` through a plain signed
    /// 32-bit `add`, with no floor anywhere (`NETHERW.EXE` 0x5ea9b
    /// `mov 0x8(%ebx),%eax` / 0x5eaa1 `inc %eax` / 0x5eaa2
    /// `add %eax,%esi`, EF:29435-37). `sub_3A200` has no life test on
    /// the victim, so once the victim is dead `life + 1` is NEGATIVE
    /// and the accumulate runs backwards.
    ///
    /// WITNESS mc2l0-spells-galore t=22008 slot 296: a `(3,3)` mana
    /// balloon killed at t=22007 (`life` 10000 -> -10002, `mail0` 0 ->
    /// (20002, 152)) is shoved again the next tick and retail's inbox
    /// reads **10001** where the port's `.max(0)` left 20002 — the
    /// take's only ungraded `(3,3) mail0.amt` row, and it clears with
    /// this arm. `MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED=1` restores the
    /// clamp and this test fails.
    #[test]
    fn a_shoved_corpse_mails_itself_a_negative_amount() {
        let mut g = flood_gen();
        let q = g.new_event().expect("quake slot");
        {
            let e = &mut g.ent[q];
            e.class64 = 10;
            e.model65 = 71;
            e.id24 = 7;
        }
        let v = g.new_event().expect("victim slot");
        {
            let e = &mut g.ent[v];
            // class 5 model 12 FORCES the 1-in-7 roll (EF:29430), so
            // the test does not ride the flood's RNG stream.
            e.class64 = 5;
            e.model65 = 12;
            e.f28 = 1; // byte_0x38_56 bit 0 — the damage contract
            e.act_life = -10002;
            e.mail[0] = (20002, 152);
        }
        g.flood_shove_hit(q, v);
        assert_eq!(
            g.ent[v].mail[0].0, 10001,
            "20002 + (life + 1) = 20002 + (-10001); retail has no floor"
        );
        assert_eq!(g.ent[v].mail[0].1, 7, "and the source is re-stamped");
    }

    /// ⭐⭐⭐ THE QUAKE'S HUMAN ARM HAS A VERTICAL LEG — RETAIL HAS NO
    /// SPECIAL CASE AT ALL.
    ///
    /// The human wizard is an ordinary victim of `sub_39B60`, and its
    /// WIZARD arm (`NETHERW.EXE` 0x5e4ef `mov 0x3f(%ebx),%ah` /
    /// `cmp $0x3,%ah` / `jne 0x5e53c`, then 0x5e4fa
    /// `cmpb $0x0,0x40(%ebx)`) takes the pull-then-clamp leg that the
    /// POOL half of [`Gen::flood_shove`] has ported since round 98.
    /// The port's out-of-pool seat special-cased him and armed only
    /// the horizontal `player_knock`; this module's own header banked
    /// the rest in writing ("the z pull and spin bank on the
    /// FlightVerb takeover seam") and it stayed banked for 45 rounds.
    ///
    /// WITNESS mc2l23 t=6,930..6,961 — the take's 32-deep reset run and
    /// its WHOLE `pose:pose.z` signature. The player flies into his own
    /// Gravity Well's disc at t=6,929 (quake slot 101, `refz` 110);
    /// retail sinks 26, 27, 27, 27, 28 … per tick and the port sank 8,
    /// the row-0xe buoyancy ALONE, from the same position — with
    /// `pose.x`/`pose.y` bit-exact for eleven more ticks, because the
    /// horizontal leg WAS ported. 8 + the pull reproduces retail's step
    /// to the unit. mc2l23 35 -> 13 segments, mc2l18 97 -> 59.
    ///
    /// ⛔ NOT FIXTURABLE: `exec_pair_mc2` pins the pose, so no boundary
    /// fixture can witness a `pose.z` the mover writes. Measured rather
    /// than assumed — the reversion probe
    /// (`MGC_NO_MC2_FLOOD_HUMAN_Z_PULL=1`, conforming-at-HEAD ∧
    /// divergent-with-the-law-off) finds ZERO candidate pairs on either
    /// mc2l23 or mc2l18, the two takes the law moves.
    ///
    /// `MGC_NO_MC2_FLOOD_HUMAN_Z_PULL=1` disarms the pull and this
    /// test's `pull` assertion fails.
    #[test]
    fn the_quake_pulls_the_human_down_as_well_as_sideways() {
        let mut g = flood_gen();
        // Flat ground at 0, so the wizard arm's `getTerrainAlt` floor
        // (carried since the seat law) cannot mask the pull.
        g.t.height.fill(0);
        // A (10,71) fissure at tile 50,50 with retail's `refz`
        // (`word_0x44_68`) at 110 — the disc's reference altitude.
        let q = g.new_event().expect("quake slot");
        {
            let e = &mut g.ent[q];
            e.class64 = 10;
            e.model65 = 71;
            e.x = 50 << 8;
            e.y = 50 << 8;
            e.z = 110;
            e.f44 = 110;
            // NOT the human's own cast: retail's `sub_39FA0` same-owner
            // gate means your own Gravity Well never pulls you in.
            e.id24 = 7;
        }
        // Inside the 3328 disc and above the 96 near-band, so the
        // graded arm is the SHOVE, not the 1-in-7 kill roll.
        let ctx = flood_ctx((50 << 8) + 1000, 50 << 8, 110 + 500);
        g.flood_shove(q, &ctx);

        // The horizontal leg (ported since the quake landed): on the
        // knock register before the seat law, in the record-write delta
        // after it (`no_mc2_flood_human_seat`).
        let bearing = Gen::angle_between(ctx.px, ctx.py, 50 << 8, 50 << 8);
        if crate::engine::features::no_mc2_flood_human_seat() {
            let (kb, dist) = g.player_knock;
            assert_eq!(dist, 89, "((3328-1000)<<8)/3328 <<7 >>8, clamped 4..128");
            assert_eq!(kb, bearing, "the knock bears TOWARD the fissure");
        } else {
            assert_eq!(g.player_knock, (0, 0), "retail never writes moveBoost here");
            let mut q = (ctx.px, ctx.py, ctx.pz);
            Gen::polar_step(&mut q, bearing, 0, 89);
            let f = g.player_flood_pull;
            assert_eq!(
                (f.dx, f.dy),
                (
                    q.0.wrapping_sub(ctx.px) as i16 as i32,
                    q.1.wrapping_sub(ctx.py) as i16 as i32
                ),
                "one 89-step TOWARD the fissure"
            );
        }

        // THE LAW: the vertical leg, armed on the same victim.
        // pv5 = 500, pull = (48 * (((4096 - 500) << 8) >> 12)) >> 8.
        let pull = std::mem::take(&mut g.player_flood_pull);
        assert!(pull.armed, "the human takes the quake's z pull");
        assert_eq!(pull.pull, (48 * (((4096 - 500) << 8) >> 12)) >> 8);
        assert_eq!(pull.pull, 42, "…and that is 42 at pv5 = 500");

        // NON-VACUITY OF THE GATE, in the same test: your OWN Gravity
        // Well arms neither leg (`sub_39FA0`'s same-owner filter).
        let mut g = flood_gen();
        let q = g.new_event().expect("quake slot");
        {
            let e = &mut g.ent[q];
            e.class64 = 10;
            e.model65 = 71;
            e.x = 50 << 8;
            e.y = 50 << 8;
            e.z = 110;
            e.f44 = 110;
            e.id24 = crate::mc1::mobs::PLAYER_TARGET;
        }
        g.flood_shove(q, &ctx);
        assert_eq!(g.player_knock, (0, 0), "your own well never shoves you");
        assert!(
            !g.player_flood_pull.armed,
            "…and never pulls you down either"
        );
    }

    /// ⭐⭐⭐ THE HUMAN IS SHOVED TWICE WHEN THE FIRST SHOVE CARRIES HIM
    /// INTO A ROW THE SWEEP HAS NOT REACHED
    /// ([`crate::engine::features::no_mc2_flood_human_seat`]).
    ///
    /// mc2l23 t=6,940→6,941's geometry verbatim: quake at (28800,
    /// 12160) refz 110, the human at (29841, 11775, 2279) = cell
    /// (116,45). Shove 1 lands y ≥ 46·256 = cell (116,46), which the
    /// y-outer sweep visits later, and shove 2 runs from THERE. The
    /// expected pose is re-derived here from the wizard arm's formula
    /// applied twice; the POSITIVE CONTROL is the same formula applied
    /// once, which is what the pre-law arm produced — and it differs.
    ///
    /// `MGC_NO_MC2_FLOOD_HUMAN_SEAT=1` fails this test.
    #[test]
    fn the_quake_shoves_the_human_again_in_his_destination_row() {
        let mut g = flood_gen();
        g.t.height.fill(0);
        let q = g.new_event().expect("quake slot");
        {
            let e = &mut g.ent[q];
            e.class64 = 10;
            e.model65 = 67;
            e.x = 28800;
            e.y = 12160;
            e.z = 0;
            e.f44 = 110;
            e.tick70 = 73;
            e.id24 = 7;
        }
        let (px, py, pz) = (29841u16, 11775u16, 2279i16);
        // His seat: the (empty) chain of his own tile, as the carpet's
        // walk slot seeds it.
        g.player_relink(px, py);
        let ctx = flood_ctx(px, py, pz);
        let shove = |g: &Gen, p: (u16, u16, i16)| {
            let d = dist2d(28800, 12160, p.0 as i32, p.1 as i32);
            let v5 = p.2 as i32 - 110;
            let v6 = ((((3328 - d) << 8) / 3328) << 7 >> 8).clamp(4, 128).min(d);
            let mut n = p;
            Gen::polar_step(
                &mut n,
                Gen::angle_between(p.0, p.1, 28800, 12160),
                0,
                v6 as i16,
            );
            let z = n.2 as i32 - ((48 * (((4096 - v5) << 8) >> 12)) >> 8);
            n.2 = z.max(g.ground_z(n.0, n.1)) as i16;
            n
        };
        let once = shove(&g, (px, py, pz));
        assert_eq!(
            (once.1 >> 8, py >> 8),
            (46, 45),
            "rig: shove 1 crosses into row 46"
        );
        let twice = shove(&g, once);
        assert_eq!(
            twice.1 >> 8,
            46,
            "rig: shove 2 stays in row 46 (no third visit)"
        );
        g.flood_shove(q, &ctx);
        let f = g.player_flood_pull;
        assert!(f.armed);
        let got = (
            px.wrapping_add(f.dx as u16),
            py.wrapping_add(f.dy as u16),
            (pz as i32 - f.pull) as i16,
        );
        assert_ne!(got, once, "POSITIVE CONTROL: not the single pre-law shove");
        assert_eq!(got, twice, "two shoves, the second from the moved pose");
        assert_eq!(
            g.player_chain.cell,
            tile((twice.0 >> 8) as u8, (twice.1 >> 8) as u8)
        );
    }

    /// ⭐⭐ `sub_3A200`'s CLASS-3 MODEL-0 ARM IS TWO PITCH STORES, NOT
    /// PRESENTATION (`engine::features::no_mc2_flood_human_spin`): the
    /// human seat stages the `pitch_acc = 512` seizure for the mover,
    /// and a pooled wizard's record pitch takes 512 on the spot.
    /// POSITIVE CONTROLS: the shove band stages nothing, and a
    /// close-band CREATURE keeps its pitch.
    ///
    /// `MGC_NO_MC2_FLOOD_HUMAN_SPIN=1` fails this test.
    #[test]
    fn the_quake_close_band_seizes_the_wizard_pitch() {
        let quake = |g: &mut Gen| {
            let q = g.new_event().expect("quake slot");
            let e = &mut g.ent[q];
            e.class64 = 10;
            e.model65 = 67;
            e.x = 28800;
            e.y = 12160;
            e.z = 0;
            e.f44 = 110;
            e.tick70 = 73;
            e.id24 = 7;
            q
        };
        // The human, 90 units above `refz`: the close band.
        let mut g = flood_gen();
        g.t.height.fill(0);
        let q = quake(&mut g);
        let (px, py) = (29000u16, 12160u16);
        g.player_relink(px, py);
        g.flood_shove(q, &flood_ctx(px, py, 200));
        let f = g.player_flood_pull;
        assert!(f.spin, "the close band seizes the pitch");
        assert!(!f.armed, "…and moves nothing");
        // POSITIVE CONTROL: the same seat 500 above `refz` is shoved.
        let mut g = flood_gen();
        g.t.height.fill(0);
        let q = quake(&mut g);
        g.player_relink(px, py);
        g.flood_shove(q, &flood_ctx(px, py, 610));
        let f = g.player_flood_pull;
        assert!(f.armed && !f.spin, "the shove band never seizes");

        // The pooled wizard (a rival) and a creature, both in the band.
        let mut g = flood_gen();
        g.t.height.fill(0);
        let q = quake(&mut g);
        let victim = |g: &mut Gen, class: u8, model: u8| {
            let v = g.new_event().expect("victim slot");
            {
                let e = &mut g.ent[v];
                e.class64 = class;
                e.model65 = model;
                e.id24 = 99;
                e.f32 = 1234;
            }
            g.move_relink(v, px, py, 150);
            v
        };
        let w = victim(&mut g, 3, 0);
        let c = victim(&mut g, 5, 1);
        g.flood_shove(q, &flood_ctx(0x8000, 0x8000, 5000));
        assert_ne!(
            g.ent[w].flags & F_TOSSED,
            0,
            "rig: the wizard took sub_3A200"
        );
        assert_ne!(
            g.ent[c].flags & F_TOSSED,
            0,
            "rig: the creature took sub_3A200"
        );
        assert_eq!(g.ent[w].f32, 512, "the wizard's record pitch is stamped");
        assert_eq!(
            g.ent[c].f32, 1234,
            "POSITIVE CONTROL: a creature keeps its pitch"
        );
    }

    /// ⭐⭐⭐ THE SHOVE FILTER MUST READ **BOTH** PORT HOMES OF RETAIL
    /// byte[0] BIT 0 ([`flood_held_bit0_law`]).
    ///
    /// mc2l22 t=63,665's geometry: the `(14,3)` checkpoint marker, its
    /// ctor's pre-hide bit set (port `flags` bit 0, NOT the quake
    /// band's synthetic [`F_TOSSED`]), sitting at `d = 3049` /
    /// `v5 = 2576` from a live quake — the far push band. Retail's
    /// `sub_39FA0` class-14 arm (`NETHERW.EXE` file 0x5E879,
    /// `f6 42 0c 21 / 75 07`) returns 0 and the marker does not move.
    ///
    /// POSITIVE CONTROL: the identical marker with byte[0] bit 0 clear
    /// IS pushed — so the assertion is not vacuous geometry.
    ///
    /// `MGC_NO_MC2_FLOOD_HELD_BIT0=1` fails exactly this test.
    #[test]
    fn the_shove_filter_honours_byte0_bit0_from_its_positional_home() {
        let mark = |g: &mut Gen, bit0: bool| {
            let v = g.new_event().expect("marker slot");
            {
                let e = &mut g.ent[v];
                e.class64 = 14;
                e.model65 = 3;
                e.id24 = 3;
                if bit0 {
                    e.flags |= 1; // `sub_51570`: `or BYTE PTR [ebx+0xc],0x1`
                }
            }
            g.move_relink(v, 33152, 32128, 4912);
            v
        };
        let rig = |g: &mut Gen| {
            let q = g.new_event().expect("quake slot");
            {
                let e = &mut g.ent[q];
                e.class64 = 10;
                e.model65 = 67;
                e.x = 30103; // d = 3049 from the marker
                e.y = 32128;
                e.z = 2336;
                e.f44 = 2336; // refz ⇒ v5 = 4912 − 2336 = 2576
                e.tick70 = 73; // not the action-74 release
                e.id24 = 906;
            }
            q
        };
        // The human parks far outside the 26×26 sweep.
        let ctx = flood_ctx(0xF000, 0xF000, 0);

        let mut g = flood_gen();
        g.t.height.fill(0);
        let hidden = mark(&mut g, true);
        let q = rig(&mut g);
        g.flood_shove(q, &ctx);
        assert_eq!(
            (g.ent[hidden].x, g.ent[hidden].y),
            (33152, 32128),
            "the pre-hidden (14,3) marker fails `byte[0] & 0x21` and is never shoved"
        );

        let mut g = flood_gen();
        g.t.height.fill(0);
        let shown = mark(&mut g, false);
        let q = rig(&mut g);
        g.flood_shove(q, &ctx);
        assert_ne!(
            (g.ent[shown].x, g.ent[shown].y),
            (33152, 32128),
            "POSITIVE CONTROL: with bit 0 clear the same marker IS pushed"
        );
    }

    #[test]
    fn burn_flags_matches_0x7f0000_table() {
        // Terrain:2067 — exactly the types whose flag word lands in
        // bits 16..22: {10,11,12} + {21..27}.
        for t in 0..=255u16 {
            let t = t as u8;
            let flags: u32 = match t {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                4 => 0x10,
                5 => 0x20,
                8 => 0x100,
                9 => 0x200,
                10 => 0x10_0000,
                11 => 0x20_0000,
                12 => 0x40_0000,
                13 | 14 => 0,
                15..=20 | 28..=34 => 0x400,
                21 | 22 | 24 => 0x2_0000,
                23 => 0x4_0000,
                25 | 27 => 0x8_0000,
                26 => 0x1_0000,
                _ => 0x80_0000,
            };
            assert_eq!(burn_flags(t), flags & 0x7F_0000 != 0, "type {t}");
        }
    }
}
