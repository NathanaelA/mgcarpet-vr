//! MC2 (5,10) — THE DOOMSDAY PYRAMID. The campaign's
//! spell-of-extinction endgame device: a ground-clamped, unkillable
//! boss structure running a 16-state script that flattens terrain in
//! an expanding crater, summons creatures/projectiles, devours the
//! battlefield, and at climax kills everything and spawns the
//! (10,17) + (10,9) apocalypse spheres. Trace:
//! docs/traces/mc2-class5-m10-doomsday.md (`EF:` =
//! remc2 EventsFunctions.cpp).
//!
//! The tick lives on [`World`] (not `Gen`): the machine drives world
//! globals — the apocalypse latch `byte_0x36E03`
//! (`World::mc2_apocalypse` — the (10,9) dome's variant selector),
//! the doomsday-active flag `word_0x36548`, and the HUD doom meter
//! `x_BYTE_D9F50[0x87a]` (`World::mc2_doom_meter`).
//!
//! Entity-field homes (creature column + this machine's own):
//! state `byte_0x46_70`→f71 · phase bitfield `subSpellIndex_0x2A_42`
//! →f44 · countdown `dword_0x10_16`→f26 · turn-rate `word_0x2C_44`
//! →f46 (f44 is taken by the bitfield — the trace confirms both are
//! live at once) · facing mode `byte_0x44_68`→f69 · summon selector
//! `byte_0x43_67`→f68 · repeat `word_0x24_36`→f38 · aim stride
//! `word_0x4A_74`→f50 · beam ramp `word_0x36546` (a retail global;
//! one pyramid per level)→f52 · target `word_0x96_150`→f146.
//!
//! DELIBERATE APPROXIMATIONS (cited):
//! - Sprites 343/344/345 auto-size their state timer to the
//!   animation length (`sub_221F0` EF:13661 via the sprite params
//!   and the frame table); the sim doesn't carry TMAPS frame counts,
//!   so the three counts are PINNED FROM THE CORPUS instead — see
//!   [`ANIM_FRAMES_343`]. (Before that pin the cases' pre-override
//!   seeds 16/16/32 stood, and the death animation looped.)
//! - `sub_5C800` palette flashes (case-7 beam flash 6) are
//!   presentation (docs/traces/mc2-class10-tail-helper-closure.md
//!   §4) — skipped like every flash before.
//! - The (9,3)/(9,26) projectile bursts (selector 9/8; mc2::proj
//!   meteor shot / whirlwind seed — docs/traces/mc2-class9-m3-m26.md)
//!   are pre-locked at the avatar via mc2_arm_proj (retail
//!   self-acquires on tick 1 — the proj module's acquisition APPROX).
//! - (RETIRED 2026-09-10, dig 126G: the case-0xE reset's
//!   `byte[1] |= 0x20` is NOT a render bit — it is the sphere DECAY
//!   channel, port flag bit 13, and it is now written. The old note
//!   read: "The case-0xE global wipe writes byte[1]|=0x20 on every
//!   entity — an unmapped render-side bit (name-inferred); we apply
//!   the life/maxLife=140 reset and skip the bit." ⭐⭐⭐ A COMMENT
//!   CLAIMING A LANE IS PRESENTATION IS A DIG LEAD.)
//! - Retail's per-list scans (dword_38531 buckets) are pool
//!   slot-order scans — the mobs.rs list APPROX.
//! - The case-7 HURL-AWAY beam moves the human via the shared knock
//!   channel (`Gen::player_knock` — the kraken/buffet writer's home)
//!   rather than teleporting the pose with moveTest + floor clamp
//!   (the app owns the pose; same observable: violent outward
//!   displacement, 944 units on the first push decaying to 10).
//! - The `rand += setting_30` LCG perturb after the two pick rolls
//!   is modeled ([`Gen::mc2_rand_perturb`], `MobCtx::mc2_turn` —
//!   the counter law lives at the multipart module doc).
//! - `word_0x36548` (set case 0, cleared case 0xF) has NO reader in
//!   retail (savegame/debug only) — not carried.

use crate::engine::features::{Gen, tile};
use crate::engine::world::World;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// The devourable class-9 projectile SUBTYPES (EF:13545-63) — the
/// pyramid is an anti-magic zone eating incoming spell projectiles;
/// subtype 10 (the castle-build projectile) has its own branch.
const DEVOUR_SUBTYPES: [u8; 7] = [2, 4, 5, 0x16, 0x17, 0x19, 30];

/// `sub_221F0` (EF:13662-73) — the pyramid's sprite setter. For the
/// THREE ANIMATED rows 343..=345 (0x157..0x159 — the wind-up, the
/// recover and the DEATH) it primes the FLC stream and then OVERWRITES
/// the state timer with that animation's frame count
/// (`GetAnimationByIndex_724F0(...)+16` = `CountOfFrames_16`), so each
/// of those states lasts EXACTLY ONE animation cycle. Rows 341/342
/// (0x155/0x156) fall outside the band and keep the case's own seed.
///
/// The sim carries no TMAPS frame table, so the three counts are
/// PINNED FROM THE CORPUS (recordings/mc2l24.mgcr, slot 7 — the state
/// is the recorded `byte_0x46_70`, sampled at the head of each tick):
/// - 343 → 5: states 6+7 span t=51778..51782 (again 51838..51842,
///   51890..51894, 63138..63142).
/// - 344 → 15: states 0xA+0xB span t=51793..51807 (again
///   51851..51865, 51903..51917, 63114..63128).
/// - 345 → 20: state 0xE spans t=63201..63220 — the corpse then
///   hides (`byte[0] |= 1`) and state 0xF runs its 60 ticks
///   (63221..63280) INVISIBLE before the despawn at 63281.
///
/// Before this the port kept the cases' pre-override seeds (16/16/32),
/// which stretched the death animation past one cycle and let it loop
/// (player-reported 2026-08-03) and ran the wind-up 3x too long.
const ANIM_FRAMES_343: i16 = 5;
const ANIM_FRAMES_344: i16 = 15;
const ANIM_FRAMES_345: i16 = 20;

/// A/B kill-switch for THE PYRAMID'S HIGH-BYTE FACING SNAP: set
/// `MGC_NO_MC2_PYRAMID_SNAP_HIGH_BYTE` to restore the pre-dig
/// behaviour, where `sub_222B0`'s `bucket >= 0xD` arm added the
/// decompile's literal `+ 6` to the player's yaw instead of the
/// shipped `add ah,0x6` (+1536). See the write site in
/// [`Gen::mc2_pyramid_face`] for the citations.
fn no_mc2_pyramid_snap_high_byte() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_SNAP_HIGH_BYTE").is_some())
}

/// A/B kill-switch for THE PYRAMID'S FLANK-PICK PARITY: set
/// `MGC_NO_MC2_PYRAMID_FLANK_FRAME_PARITY` to restore the pre-dig
/// behaviour, where `sub_222B0`'s case-2 arm picked its ±512 flank
/// from the entity's own `byte_0x3E_62` (`f63`) instead of the
/// GLOBAL `FrameTimingIndex_26`. See the write site in
/// KILL SWITCH (`MGC_NO_MC2_DOOM_BEAM_ONESHOT=1`) for the (5,10)
/// pyramid's HURL-AWAY BEAM transport: set it to restore the pre-dig
/// `Gen::player_knock` route (magnitude clamped to ±128, decayed
/// −4/tick, still shoving twenty ticks after the burst). Law ON =
/// retail's one-shot POSITION write.
fn no_mc2_doom_beam_oneshot() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_DOOM_BEAM_ONESHOT").is_some())
}

/// KILL SWITCH (`MGC_NO_MC2_CITADEL_DEVOUR=1`) for the SECOND, and
/// until now UNPORTED, call path into `sub_21F60` — the one that runs
/// from `UpdateEntities_57730`'s own tail, one statement after
/// `sub_585D0` (LAW 26):
///
/// ```c
///   sub_585D0();
///   if (D41A0_0.word_0x3654A)
///       sub_21F60(Entities_EA3E4[D41A0_0.word_0x3654A]);   // EF:40471-73
/// ```
///
/// `NETHERW.EXE` file 0x7C2A7-0x7C2CD (VA 0x57AA7): `e8 24 0b 00 00`
/// call 0x7CDD0 (the re-floor), then `a1 a0 41 00 00` /
/// `66 8b b0 4a 65 03 00` (`si = D41A0_0.word_0x3654A`) /
/// `66 85 f6` / `74 13` / `8b 3c 85 e4 a3 01 00` /
/// `57` / `e8 93 a4 fc ff` call 0x46760 = `sub_21F60`. An `e8 rel32`
/// scan of the shipped EXE finds EXACTLY TWO callers of 0x46760 —
/// 0x4586A (inside `sub_21AB0`, the pyramid's own tick, which is the
/// only one the port had) and 0x7C2C8 (this one). ⭐⭐⭐ A LAW ON ONE
/// CALL PATH IS NOT LANDED.
///
/// `word_0x3654A` is the slot of the building whose BLDGPRM id is 68
/// (`sub_49A30`, EF:32848-51) — mc2l24's Vissuluth citadel. The
/// recording is the witness: `D41A0_0 @0x3654A` reads **169** from
/// t≈7950 to the end of the take, and slot 169 is the `(10,45)`
/// building at (10240, 54272) with `b46 = 68`, `apitch = aroll =
/// 4992`. No other MC2 take in the corpus ever sets it (mc2l0, l1,
/// l3, l4, l15, l30 all read 0 for every sampled tick, 2026-09-10).
///
/// ⚠ THE ARG IS NOT THE PYRAMID, AND `sub_21F60` BRANCHES ON THAT.
/// Its first act is `v17 = a1->class != 5 || a1->model != 10`
/// (file 0x46797 `80 78 3f 05` / 0x4679D `80 78 40 0a`): the
/// EuclideanDist ≤ 0xC00 CYLINDER at 0x46825 is the `!v17` (pyramid)
/// arm, which is the only one [`World::mc2_pyramid_devour`] models.
/// For any other devourer the test is a BBOX through
/// `CompareAxisWithShift_106F0` (0x34EF0) with the DEVOURER'S OWN
/// `array_0x52_82` — `abs(dx) < a.apitch + b.apitch &&
/// abs(dy) < a.aroll + b.aroll`, STRICT `<` (0x34F1B `7d 29` jnl,
/// 0x34F3A `7d 0a`) — against `{5120, 5120}` on the projectile
/// (0x4683D `b9 00 14 00 00`), or against the player castle's own
/// extents for the model-10 castle-build shot. And it never sets the
/// trip `v19` on that arm (0x468EE writes `[ebp-8]` only), which is
/// moot here because the tail call DISCARDS the return.
fn no_mc2_citadel_devour() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_CITADEL_DEVOUR").is_some())
}

/// [`Gen::mc2_pyramid_face`] for the citations.
fn no_mc2_pyramid_flank_frame_parity() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_FLANK_FRAME_PARITY").is_some())
}

/// A/B kill-switch for THE DEAD-PLAYER BARRAGE FREEZE: set
/// `MGC_NO_MC2_PYRAMID_DEAD_PLAYER_GATE` to restore the pre-dig
/// behaviour, where `sub_21AB0` fired (and burned its repeat count)
/// even with the human wizard's pool record dead. See the gate in
/// [`World::mc2_pyramid_do_summon`] for the citations.
fn no_mc2_pyramid_dead_player_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_DEAD_PLAYER_GATE").is_some())
}

/// A/B toggle for `KillAllCreatures_1B5F0`'s CHAIN MEMBERSHIP: set
/// `MGC_NO_MC2_KILL_ALL_CHAIN_MEMBERS` to restore the old whole-pool
/// walk, which re-stamped already-dead creatures every tick of the
/// apocalypse. See the law note at the call site.
fn no_mc2_kill_all_chain_members() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_KILL_ALL_CHAIN_MEMBERS").is_some())
}

/// KILL SWITCH (`MGC_NO_MC2_DOOM_SPHERE_DECAY_BIT=1`) for THE CASE-0xE
/// SPHERE RESET'S DECAY BIT: set it to restore the pre-dig behaviour,
/// where the pyramid's death re-lifed every `dword_38523` sphere to
/// 140 but left `byte[1] & 0x20` clear, so [`Gen::ball_decay_tail`]
/// never armed and the spheres sat at life 140 FOREVER. Law ON =
/// retail's `or dl, 0x20`. See the write site in the 0xE arm.
fn no_mc2_doom_sphere_decay_bit() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_DOOM_SPHERE_DECAY_BIT").is_some())
}

/// A/B kill-switch for THE REBOUND TAIL'S `word_0x2E_46` HOME: set
/// `MGC_NO_MC2_DEVOUR_REBOUND_AT_2E` to restore the pre-dig read,
/// which took `f26` off whatever record the book's spell-8 index
/// pointed at — right only for a live class-15 token, wrong for every
/// other class the death's boolean-`1` marker dereferences to. See the
/// tail of [`World::mc2_pyramid_devour`] for the citations.
fn no_mc2_devour_rebound_at_2e() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_DEVOUR_REBOUND_AT_2E").is_some())
}

/// A/B kill-switch for THE PYRAMID'S PROJECTILES CARRY NO TARGET LOCK:
/// set `MGC_NO_MC2_PYRAMID_PROJ_NO_LOCK` to restore the pre-dig
/// behaviour, where `mc2_arm_proj` copied the pyramid's own
/// `word_0x96_150` onto the newborn. See
/// [`Gen::mc2_pyramid_arm_tail`] for the citations.
fn no_mc2_pyramid_proj_no_lock() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_PROJ_NO_LOCK").is_some())
}

/// A/B kill-switch for THE PYRAMID'S BEHAVIOUR ROW: set
/// `MGC_NO_MC2_PYRAMID_BEHAVIOR_ROW` to restore the pre-dig row 107 —
/// the decompile's `&str_D7BD6[107]` (EF:34021), which in the port's
/// verbatim `str_D7BD6` extract is the ALL-ZERO terminator row
/// (`v_2`/`v_16`/`v_26`/`v_28`/`v_30` all 0). Law ON = the shipped
/// EXE's row 105. See [`Gen::mc2_spawn_doomsday`] for the citations.
fn no_mc2_pyramid_behavior_row() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_BEHAVIOR_ROW").is_some())
}

/// A/B kill-switch for THE PYRAMID CTOR'S FLAG-WORD TRANSLATION: set
/// `MGC_NO_MC2_PYRAMID_CTOR_FLAG_XLAT` to restore the pre-dig raw
/// `|= 0x48800001`, whose bit 27 collides with the port's
/// [`crate::mc2::mobs::F_BLOCKED`]. Full citation and the shipped-EXE
/// scan live at the store in [`Gen::mc2_spawn_doomsday`].
pub(crate) fn no_mc2_pyramid_ctor_flag_xlat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_PYRAMID_CTOR_FLAG_XLAT").is_some())
}

impl Gen {
    /// `sub_4BD00` (EF:33965) — the pyramid ctor MINUS the map gate
    /// (`byte_0x2FED2 & 2` lives on World — the spawn seam checks
    /// it). No ctor RNG. Sprite 341, behavior row 105, huge life,
    /// ground-clamped, ShiftRot(1024, 1280), extent yaw 512.
    pub(crate) fn mc2_spawn_doomsday(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 5;
            e.model65 = 10;
            e.tick70 = 80;
            e.max_life = 300_000;
            e.f28 = 1; // cross-column damage contract
            e.f44 = 0; // subSpellIndex = the PHASE BITFIELD here
            // The ctor's `|= 0x48800001` (EF:33980). byte[3] & 0x40
            // (bit 30) is LOAD-BEARING: it is the render gate for the
            // detailed-draw pass's `subSpellIndex |= 0x40` arming
            // writer (GameRenderOriginal.cpp:4915-19) — the ONLY
            // setter of the wind-down escape bit in the whole engine
            // (NETHERW.EXE @0x45f11; the machine itself only ever ANDs
            // it off).
            //
            // ⭐⭐⭐ …BUT THE CONSTANT IS A **RETAIL-LAYOUT** WORD AND
            // THE PORT'S FLAG WORD IS A TRANSLATION, NOT A COPY.
            // `import_ent_mc2` unpacks retail's four flag bytes into
            // remapped port bits (`conformance.rs` ~5150-5280): three
            // of the four bits in 0x48800001 happen to land on their
            // own port home positionally — bit 0 (`byte[0] & 1`, the
            // walk bit), bit 23 (`byte[2] & 0x80`, the dormant boss's
            // raster mode, cleared again at EF:13024 below) and bit 30
            // (`byte[3] & 0x40`, the render-arm gate) — but **bit 27
            // does not**. Port bit 27 is [`crate::mc2::mobs::F_BLOCKED`],
            // the move core's block latch, which the importer fills
            // from retail's `byte[2] & 4`. So the raw stamp handed
            // every doomsday pyramid a permanent "move blocked" latch
            // it never earned.
            //
            // AND RETAIL'S `byte[3] & 0x08` IS WRITE-ONLY. A scan of
            // the whole shipped `NETHERW.EXE` for every `test`/`or`/
            // `and`/`cmp` of an imm8 against `byte [reg+0x0F]` (the
            // fourth flag byte) finds **no instruction whose mask
            // touches 0x08** — the only `and` masks on that byte are
            // 0xEF (file 0x57DF4), 0xF9 (six sites in 0x43239..0x43632)
            // and 0xFE (0x45F91), none of which clears it, and the
            // only imm32 form at the `+0x0C` dword is this very ctor
            // (`81 4b 0c 01 00 80 48` at file 0x7053E). One writer,
            // zero readers: the bit is inert in retail and has no port
            // home, so the port must simply not raise it.
            //
            // WITNESS: `(5,10) flags.b2_x4` — 9,499 free-run rows over
            // two takes, retail 0 / port 1 UNBROKEN from the pyramid's
            // birth tick to the end of the take (mc2l24 slot 5 from
            // t=44380; mc2l24-crazy slot 6 from t=52323). ⚠ the lane is
            // invisible in PAIR mode: the importer re-writes the flag
            // word every tick and never sets bit 27 there. The stray
            // latch is not cosmetic — `mc2/stagevars.rs` gates three
            // `sub_1D5D0` legs on `flags & F_BLOCKED == 0`.
            // `MGC_NO_MC2_PYRAMID_CTOR_FLAG_XLAT=1` restores the raw
            // stamp. (Round 148, dig w148t.)
            e.flags |= if no_mc2_pyramid_ctor_flag_xlat() {
                0x4880_0001
            } else {
                0x4080_0001
            };
            // `@0x36` is NOT `byte_0x38_56` — see `mc2_class5_w36_legacy`.
            if crate::mc2::roster::mc2_class5_w36_legacy() {
                e.f56 = 1;
            }
            // ⭐⭐⭐ ROW 105, NOT THE DECOMPILE'S 107 — AND 107 IS THE
            // ALL-ZERO TERMINATOR ROW. remc2 renders the ctor's row
            // store as `v2x->dword_0xA0_160x = &str_D7BD6[107]`
            // (EF:34021), but its OWN byte-offset comment on that line
            // (`unk_D7BD6[0xe4e]` = 3662) is not even a multiple of the
            // 34-byte row stride, so the index is a hand-conversion
            // slip. THE SHIPPED EXE SETTLES IT: `sub_4BD00` at VA
            // 0x4BD64 (`NETHERW.EXE` file 0x70564) is
            // `c7 83 a0 00 00 00 c8 89 00 00` =
            // `mov dword [ebx+0xA0], 0x89C8`, and
            // (0x89C8 − 0x7BD6) / 34 = 3570 / 34 = 105 exactly.
            // A scan of EVERY `mov dword [reg+0xA0], imm32` in the
            // shipped binary — 72 stores, 39 distinct values (plus
            // one `…, 0` null store) — decodes to an INTEGER index on
            // that same base/stride, spanning rows 59..=106, so the
            // base and the 34-byte stride are settled. Row 107 is
            // written NOWHERE: the four bytes `base + 34·107` =
            // 0x8A0C do not occur anywhere in the file at all.
            // (remc2's other high-row site, EF:20562's
            // `&str_D7BD6[106]` in `sub_2A7F0`, IS right: VA 0x2A912
            // stores 0x89EA = base + 34·106 — and remc2's own data
            // label two lines later, `off_D89EA`, is that very row.)
            //
            // The port's `BEHAVIOR` extract is verbatim, so index 107
            // is the table's ALL-ZERO row: `v_2` (turn cap) 0, `v_16`
            // (roughness fence) 0, `v_20` 0, `v_26` 0, `v_28` (the
            // class-3 LOCK RANGE) 0, `v_30` 0, flags 0 — against row
            // 105's 5 / 20 / 0xFFFFFFFF / 4 / 6400 / 512 / 0x7.
            //
            // RETAIL'S OWN RECORDING IS THE SECOND WITNESS: the
            // `.mgcr` carries `dword_0xA0` verbatim, and the importer
            // decodes it as `(ptr − base160)/34 + 59`. mc2l24's
            // pyramid is born into slot 5 at t=44380 with
            // `ptr_a0 = 2791880`, base160 = 2790316 ⇒ (1564/34) + 59 =
            // 105. That is why every `--start t−1` pair arm read 105
            // while the free run stamped 107: the pair importer seats
            // the row off retail's pointer and the ctor never runs.
            //
            // The measured consequence (mc2l24 t=44600): with `v_28`
            // 0 the pyramid-owned (9,26) whirlwind seed's one-shot
            // acquisition (`sub_67CB0`, [`Gen::mc2_autoaim`] via
            // [`Gen::mc2_owner_lock_range`]) finds NO candidate — the
            // human sits 5,000 units out, inside 6400 and outside 0 —
            // so the seed keeps its launch attitude (yaw 312, pitch
            // 2033) and flies straight where retail locks the avatar
            // and snaps to 280 / 26.
            e.row156 = if no_mc2_pyramid_behavior_row() {
                107
            } else {
                105
            };
            e.f58 = 64; // byte_0x39_57 awake
            e.f66 = 3; // xtype
            e.f26 = 0;
            e.f71 = 0;
        }
        self.mc2_set_mana_half(i); // SetEvent144_49C70
        self.ent[i].f63 = self.mc2_ord(10);
        self.link(i, x, y, z);
        // GROUND-CLAMPED (getTerrainAlt_10C40); re-clamped per tick.
        self.ent[i].z = self.ground_z(x, y) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 341);
        self.ent[i].f78 = 512; // array yaw
        self.mc2_shift_rot(i, 1024, 1280);
        Some(i)
    }

    /// `sub_221F0` (EF:13662) — set the pyramid sprite, applying the
    /// animated rows' state-timer override (see [`ANIM_FRAMES_343`]).
    pub(crate) fn mc2_pyramid_sprite(&mut self, i: usize, idx: u16) {
        // ⭐⭐ THE PYRAMID RE-SPRITES WITH THE INDEX-ONLY SETTER — IT
        // MUST NOT RE-DERIVE ITS HALF-EXTENT QUAD. `sub_221F0`'s
        // first act is `SetEntityIndex_49C90(a1x, a2)` (EF:13666),
        // NOT the `AndRot` twin. Shipped `NETHERW.EXE`, sub_221F0's
        // prologue at linear 0x221DA (file 0x469F0):
        //     53 56 57 55        push ebx/esi/edi/ebp
        //     89 e5              mov  ebp,esp
        //     8b 7d 14           mov  edi,[ebp+0x14]   ; entity
        //     8b 75 18           mov  esi,[ebp+0x18]   ; index
        //     0f bf de           movsx ebx,si
        //     53 57              push ebx / push edi
        //     e8 8a 7a 02 00     call 0x49c7a          ; SetEntityIndex
        // — 0x49C7A is the PLAIN setter. Its `AndRot` twin enters at
        // 0x49CBA, calls the SAME 0x49C7A (`e8 ad ff ff ff` at
        // 0x49CC8) and only THEN stamps the quad from the particle
        // row (0x49CDB.. `mov [esi+0x52],ax` / `[esi+0x54]`, the
        // rotSpeed_8/2 and speed_6/2 halves). The pyramid never
        // reaches that second half, so the boss keeps the ctor's
        // `ShiftRot(1024, 1280)` + extent yaw 512 for its whole life.
        // The port reached for `mc2_set_sprite` (the AndRot twin) —
        // exactly the trap [`Gen::mc2_set_sprite_index`]'s own doc
        // comment warns about — and row 341's authored pair
        // (rotSpeed_8 1200, speed_6 1714) re-derived the quad to
        // 600 / 857 on every state change.
        // WITNESS mc2l24 slot 5, 168 pair segments t=44,538..52,732
        // whose ONLY divergence is `applied_yaw` retail 512 port 600
        // + `applied_pitch` retail 1024 port 857 — the second-largest
        // single family left on the take.
        if crate::mc2::mobs::no_mc2_pyramid_sprite_keeps_rot() {
            self.mc2_set_sprite(i, idx);
        } else {
            self.mc2_set_sprite_index(i, idx);
        }
        let frames = match idx {
            343 => ANIM_FRAMES_343,
            344 => ANIM_FRAMES_344,
            345 => ANIM_FRAMES_345,
            _ => return,
        };
        self.ent[i].f26 = frames;
    }

    /// `sub_22490` (EF:13814) — the activation footprint wipe: over
    /// the 38x38 tile block, `sub_57390` per tile — the SAME clear
    /// the building creator uses ([`Gen::mc2_building_clear_tile`],
    /// scenery removed, unprotected creatures killed).
    fn mc2_pyramid_wipe(&mut self, cx: u8, cy: u8, own: u16) {
        for j in 0..38u8 {
            let y = cy.wrapping_sub(19).wrapping_add(j);
            for k in 0..38u8 {
                let x = cx.wrapping_sub(19).wrapping_add(k);
                self.mc2_building_clear_tile(tile(x, y), own);
            }
        }
    }

    /// `sub_56F10(x, y, -1, 0)` (EF:39499) — the flatten stamp:
    /// height += delta clamped [0,200]; nonzero heights force the
    /// flat angle nibble, a zero height runs the water-seal walk;
    /// then the per-cell AddBuildingToTerrain recompute (a4=0).
    fn mc2_doom_flatten_cell(&mut self, x: u8, y: u8) {
        let t = tile(x, y);
        let h = (self.t.height[t] as i16 - 1).clamp(0, 200);
        self.t.height[t] = h as u8;
        // The sub_56F10 cave arm (EF:39534-43): on a cave the
        // ceiling counter-shifts by the raw
        // delta (dig down = roof up), saturating at 255 with
        // retail's char truncation below zero — the same arm the
        // shared dig_cell chassis carries.
        if self.is_cave() {
            let c = self.t.ceiling[t] as i32 + 1;
            self.t.ceiling[t] = if c >= 255 { 255 } else { c as u8 };
        }
        if h != 0 {
            self.t.angle[t] = (self.t.angle[t] & 0xF8) | 1;
        } else {
            let sealed = [
                (255u8, 255u8),
                (0, 255),
                (1, 255),
                (1, 0),
                (255, 0),
                (255, 1),
                (0, 1),
                (1, 1),
            ]
            .iter()
            .all(|&(dx, dy)| {
                let a = self.t.angle[tile(x.wrapping_add(dx), y.wrapping_add(dy))] & 7;
                a != 5 && a != 2 && a != 3
            });
            if sealed {
                self.t.angle[t] &= 0xF0;
            }
        }
        self.mc2_add_building_region(x, y, x, y);
    }

    /// `sub_22270` → `sub_222B0` (EF:13683-13774): re-clamp z to the
    /// ground and turn toward the player. The relative-yaw bucket
    /// picks a snap; otherwise the facing mode (f69) sets the roll
    /// target and a `sub_58350` rate-limited turn walks yaw to it.
    fn mc2_pyramid_face(&mut self, i: usize, ctx: &MobCtx) {
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(x, y) as i16;
        if self.ent[i].act_life < 10 {
            return;
        }
        let yaw = self.ent[i].f30;
        let bucket = ((yaw.wrapping_sub(ctx.pyaw) >> 3) & 0xF0) >> 4;
        if bucket <= 2 {
            self.ent[i].f30 = ctx.pyaw.wrapping_add(384) & 0x7FF;
            return;
        }
        if bucket >= 0xD {
            // ⭐⭐⭐ THE SNAP IS +1536, NOT +6 — A HAND-CONVERSION
            // ERROR IN THE DECOMPILE THAT THE PORT INHERITED.
            // EF:13716 reads `a1x->yaw_0x1C_28 = v1x->yaw_0x1C_28 +
            // 6;` and its own commented-out original two lines above
            // gives the game away: `//LOWORD(v1) = *(x_WORD *)(v1 +
            // 28); //BYTE1(v1) += 6;`. It is a HIGH-BYTE add.
            // Shipped `NETHERW.EXE`, linear 0x22306 (file 0x46B1C):
            //     66 8b 40 1c   mov  ax,[eax+0x1c]   ; player yaw
            //     80 c4 06      add  ah,0x6          ; += 0x600 = 1536
            //     66 89 43 1c   mov  [ebx+0x1c],ax
            //     e9 a9 00 00 00  jmp 0x223bf         ; -> and [ebx+0x1d],0x7
            // — compare the SIBLING snap eight bytes earlier
            // (0x222F4 `05 80 01 00 00  add eax,0x180`), which really
            // is a plain +384, and the case-2 roll write at 0x2235B
            // (`80 c4 04  add ah,0x4` = +1024), where the port already
            // folded the high-byte add into its 512/1536 pair. Only
            // this arm was copied literally.
            // WITNESS mc2l24 slot 5, t=44,574: the boss holds yaw 1890
            // and the human (slot 116) yaw 202, so the bucket is 13.
            // Retail writes 202 + 1536 = 1738; the port wrote 202 + 6
            // = 208. Same at t=44,578 (101 -> 1637), 44,582 (2037 ->
            // 1525), 44,585 (1947 -> 1435), 44,588 (1855 -> 1343) and
            // 44,594 (1669 -> 1157) — every one exact.
            let snap = if no_mc2_pyramid_snap_high_byte() {
                6
            } else {
                1536
            };
            self.ent[i].f30 = ctx.pyaw.wrapping_add(snap) & 0x7FF;
            return;
        }
        match self.ent[i].f69 {
            0 => {
                self.ent[i].f34 = Self::angle_between(x, y, ctx.px, ctx.py);
            }
            2 => {
                // ±512 alternating by frame parity, then hold (mode 1).
                // ⭐⭐ THE PARITY IS THE GLOBAL FRAME COUNTER, NOT THE
                // ENTITY'S OWN `byte_0x3E_62`. EF:13738-45 reads
                // `x_D41A0_BYTEARRAY_4_struct.FrameTimingIndex_26 & 1`
                // — the same counter the port models as
                // `World::mc2_turn` (world.rs's objective-9 frame gate
                // `!(FrameTimingIndex_26 & 0xF)`, EF:40852). Shipped
                // `NETHERW.EXE`, linear 0x22343 (file 0x46B59):
                //     8b 15 a4 41 00 00  mov  edx,[0x41a4]
                //     f6 42 1a 01        test BYTE [edx+0x1a],0x1
                //     74 07              jz   0x22356
                //     ba 00 02 00 00     mov  edx,0x200      ; +512
                //     eb 05              jmp  0x2235b
                //     ba 00 fe ff ff     mov  edx,0xfffffe00 ; -512
                //     66 8b 40 1c        mov  ax,[eax+0x1c]  ; player yaw
                //     80 c4 04           add  ah,0x4         ; += 1024
                //     01 d0              add  eax,edx
                //     66 89 43 20        mov  [ebx+0x20],ax  ; roll
                // i.e. `pyaw + 1024 ± 512` = the port's own {512, 1536}
                // pair — only the SELECTOR was wrong. `MobCtx::mc2_turn`
                // is captured at the tick TOP and retail's counter is
                // read POST-increment (world.rs's own bump order, and
                // the m27 branch-bolt perturb's identical `+ 1`), so
                // the live parity is `ctx.mc2_turn + 1`.
                // WITNESS mc2l24 slot 5. t=44,532: pyaw 890, turn
                // 44,537 -> live 44,538 (even) -> +512 -> roll 1402,
                // retail 1402; the old `f63 & 1` read 151 (odd) and
                // wrote 890 + 1536 = 378. t=44,589: pyaw 1824, turn
                // 44,594 -> live 44,595 (odd) -> +1536 -> roll 1312,
                // retail 1312; the old read f63 208 (even) -> 288.
                // Both exactly 1024 (180°) out — the boss turned to
                // the wrong flank on every state-4 entry.
                let parity = if no_mc2_pyramid_flank_frame_parity() {
                    (self.ent[i].f63 & 1) as u32
                } else {
                    ctx.mc2_turn.wrapping_add(1) & 1
                };
                let side = if parity == 0 { 512u16 } else { 1536 };
                self.ent[i].f34 = ctx.pyaw.wrapping_add(side) & 0x7FF;
                self.ent[i].f69 = 1;
            }
            3 => {
                self.ent[i].f34 = self.ent[i].f30;
            }
            _ => {}
        }
        let rate = self.ent[i].f46;
        let step = Self::turn_step(self.ent[i].f30, self.ent[i].f34, rate);
        self.ent[i].f30 = (self.ent[i].f30 as i32 + step as i32) as u16 & 0x7FF;
    }

    /// `sub_22190` (EF:13625) — the damage-mailbox read with the
    /// IMMORTAL CLAMP: damage lands (1..=300 per tick) but life is
    /// pinned back to 8 whenever it would drop below 10.
    fn mc2_pyramid_mail(&mut self, i: usize) {
        if self.ent[i].f58 != 0 {
            let (amt, src) = self.ent[i].mail[0];
            if src != 0 {
                let v = (amt as i32).clamp(1, 300);
                self.ent[i].act_life -= v;
                self.ent[i].f40 = src;
                self.ent[i].mail[0].1 = 0;
            } else {
                // No pending hit: the attacker memory clears
                // (`else word_0x26_38 = 0`, EF:13648-51; the field is
                // f40, NOT the ring-angle f42 home).
                self.ent[i].f40 = 0;
            }
        }
        if self.ent[i].act_life < 10 {
            self.ent[i].act_life = 8;
        }
    }
}

impl Gen {
    /// `sub_21AB0`'s SHARED `if (v33x)` ARMING TAIL — the four
    /// PROJECTILE cases (1 = the (9,0) bolt, 2 = the (9,9) lightning
    /// beam, 8 = the (9,26) whirlwind seed, 9 = the (9,3) meteor shot)
    /// all fall into one block (EF:13491-13508), and it writes FIVE
    /// fields and calls `sub_5EF70` — **and it does NOT write
    /// `word_0x96_150`**. Every other launch thunk in the engine does
    /// (`sub_1CC20` EF:9700 `v5x->word_0x96_150 = a1x->word_0x96_150;`
    /// and its five siblings); the pyramid's does not, so its
    /// projectiles are born TARGETLESS and acquire on their own.
    ///
    /// NETHERW.EXE file 0x466E7-0x46739 is the whole block — five
    /// stores and one call, no `[edx+0x96]` anywhere:
    ///
    /// ```text
    ///   466e7  85 d2              test edx,edx        ; v33x
    ///   466e9  74 51              jz   0x4673c
    ///   466eb  8b 75 ec           mov  esi,[ebp-0x14] ; v31x = the avatar
    ///   466ee  83 c6 4c           add  esi,0x4c
    ///   466f2  8d 7b 4c           lea  edi,[ebx+0x4c] ; the pyramid
    ///   466fa  66 89 42 1a        mov  [edx+0x1a],ax  ; id_0x1A_26
    ///   466fe  e8 dd 62 03 00     call 0x7c9e0        ; sub_581E0 tan2
    ///   0670b  66 89 42 1c        mov  [edx+0x1c],ax  ; yaw
    ///   0670f  e8 fc 62 03 00     call 0x7ca10        ; sub_58210 radix_tan
    ///   46717  66 89 42 1e        mov  [edx+0x1e],ax  ; pitch
    ///   46721  88 42 42           mov  [edx+0x42],al  ; xsubtype = avatar model
    ///   46731  88 42 41           mov  [edx+0x41],al  ; xtype = avatar class
    ///   46734  e8 37 d0 03 00     call 0x83770        ; sub_5EF70(avatar)
    /// ```
    ///
    /// Contrast the SUMMON arm (cases 3..=6, `v35x`), which DOES stamp
    /// the lock — `v35x->word_0x96_150 = playerIndex` (EF:13413) — and
    /// which the port already reproduces. The asymmetry is retail's.
    ///
    /// WITNESS (mc2l24 t=45470 -> 45471, the pair lane): the pyramid's
    /// case-2 (9,9) beam is re-fired at the avatar. Retail's beam
    /// enters `sub_66750` with `word_0x96_150 == 0`, so `sub_66610`'s
    /// acquire block (EF:63583) runs, elects slot 377 (a (5,25) on the
    /// ground) and re-aims yaw 189 / pitch 102 (DOWN); it marches four
    /// steps and lays 32 trail nodes. The port inherited the pyramid's
    /// own lock (`PLAYER_TARGET`), the acquire gate refused, and the
    /// beam kept the launcher's pitch 2007 (UP at the carpet), marched
    /// ten steps and laid 81 nodes — 24 more free-stack pops than
    /// retail, so every later birth in the tick landed one slot off.
    ///
    /// `sub_5EF70` is the avatar's danger poke; it is on all FOUR
    /// cases in retail, and the port had it on 8/9 only.
    pub(crate) fn mc2_pyramid_arm_tail(&mut self, p: usize) {
        if !no_mc2_pyramid_proj_no_lock() {
            self.ent[p].f146 = 0;
        }
        self.mc2_danger_poke(PLAYER_TARGET);
    }
}

impl World {
    /// `sub_21030` (EF:12654-12880) — the pyramid's 16-state machine
    /// (actions 80..=87 all funnel here; the +1..+3/+6/+7 handlers
    /// are literal `actionIndex = 80` resets, EF:13844-13903).
    pub(crate) fn mc2_doomsday_tick(&mut self, i: usize, ctx: &MobCtx) {
        // The thin handlers: reset to the machine state.
        if matches!(self.g.ent[i].tick70, 81..=83 | 86 | 87) {
            self.g.ent[i].tick70 = 80;
        }
        // Retail's ctor map gate (`byte_0x2FED2 & 2` — EF:33968),
        // applied on the first tick (the spawn-seam ordering note):
        // no doom-palette level, no pyramid.
        if !self.mc2_doom_level {
            self.g.ent[i].flags |= 0x400;
            return;
        }
        let mut death_sound = false;
        // The RENDERER's arming writer (GameRenderOriginal.cpp:4915-19,
        // mirrored NG/HD): each frame the pyramid is drawn in the
        // DETAILED (near) pass, retail sets `subSpellIndex |= 0x40` on
        // it — gated on flags byte[3] & 0x40 (the ctor's 0x48800001).
        // That bit is the wind-down phase's ONLY escape: armed + player
        // within 0xA00 → the doom-meter ramp → the attack cycle →
        // states 2/3 where damage is read → killable (the machine's
        // own writes only ever CLEAR it).
        // The headless sim can't couple to a render pass, so the arm
        // is reproduced as the deterministic proximity analog: any
        // radius ≥ the machine's own 0xA00 far-gate is behaviorally
        // identical (far ticks just re-clear the bit), so the gate
        // distance itself is the faithful choice.
        //
        // ⛔⛔ ROUND 149 (dig w149e) TRIED TO CLOSE THE RESIDUAL RAW
        // LANE AND PROVED IT IS NOT CLOSEABLE. The census reports
        // `(5,10) f2a` retail 80 / port 16 — 23 rows on mc2l24
        // (t=44468..44490 slot 5) and 72 on mc2l24-crazy
        // (t=52527..52656 slot 6) — i.e. exactly bit 0x40. Moving the
        // arm to AFTER the machine (where the frame is drawn) and
        // dropping the distance test for the ctor's `byte[0] & 1`
        // dormant gate reproduces mc2l24 EXACTLY (23 rows → 0) and
        // makes mc2l24-crazy WORSE, 72 rows → **104, sign flipped**
        // (`t=52481 slot 6: retail 16 port 80`).
        //
        // ⭐⭐⭐ THE REASON IS IN RETAIL'S OWN TRAJECTORY: THE BIT
        // FLICKERS. `dump-state` on mc2l24-crazy slot 6, all with the
        // dormant bit already down (flags …8C):
        //   t=52500 f2a 16 · t=52526 16 · t=52527 **80** ·
        //   t=52600 **16** · t=52656 80 · t=52657 96 (the wake)
        // On mc2l24 the same window is a solid 80 because the pyramid
        // sits in frame throughout. So `@0x2A & 0x40` is a CAMERA
        // FRUSTUM lane — set by the sprite pass whenever the boss is
        // ON SCREEN, cleared only by the machine — and no deterministic
        // function of the world state can reproduce it. The proximity
        // analog is the best available approximation and the residual
        // is a DEVIATION CANDIDATE, not a port defect. Do not re-dig
        // it without a camera.
        if self.g.ent[i].flags & 0x4000_0000 != 0 {
            let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
            let dx = (ctx.px as i32 - ex as i32) as i16 as i32;
            let dy = (ctx.py as i32 - ey as i32) as i16 as i32;
            if dx * dx + dy * dy < 0xA00i32.pow(2) {
                self.g.ent[i].f44 |= 0x40;
            }
        }
        // Prologue: the projectile-devour pass trips phase bit0.
        if self.mc2_pyramid_devour(i) {
            self.g.ent[i].f44 |= 1;
        }
        let state = self.g.ent[i].f71;
        if state > 1 && !(0xC..=0xF).contains(&state) && self.g.ent[i].act_life >= 10 {
            self.g.mc2_pyramid_mail(i);
        }
        let cx = ((self.g.ent[i].x.wrapping_add(128)) >> 8) as u8;
        let cy = ((self.g.ent[i].y.wrapping_add(128)) >> 8) as u8;
        // Retail's setup cases fall THROUGH to their successor's body
        // in the same tick (goto, no break — EF §5); without it every
        // phase runs a tick long and the first summon fires late.
        let mut fall = true;
        while std::mem::take(&mut fall) {
            match self.g.ent[i].f71 {
                0 => {
                    // Doomsday active ON; arm the terrain-flatten bit;
                    // wipe the footprint; target the player. Falls into
                    // case 1 — the first flatten tick runs NOW.
                    self.g.ent[i].f71 = 1;
                    self.g.ent[i].f44 = 8;
                    self.g.ent[i].f26 = 15;
                    self.g.ent[i].f46 = 22;
                    self.g.ent[i].f146 = PLAYER_TARGET;
                    self.mc2_doom_meter = 60;
                    self.g.mc2_pyramid_wipe(cx, cy, self.g.ent[i].id24);
                    fall = true;
                }
                1 => {
                    if self.mc2_pyramid_attack(i, ctx, cx, cy) {
                        self.g.ent[i].f71 = 4;
                        self.g.ent[i].f44 |= 0x80;
                    }
                }
                2 => {
                    let d = self.g.ent_rand(i);
                    let (life, maxl) = (self.g.ent[i].act_life, self.g.ent[i].max_life as i32);
                    let v = (26 * life / maxl.max(1)) - (d & 7) as i32;
                    self.g.ent[i].f26 = v.clamp(3, 26) as i16;
                    self.g.ent[i].f71 = 3;
                    self.g.ent[i].f69 = 0;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_pyramid_sprite(i, 341);
                    fall = true;
                }
                3 => {
                    if self.g.ent[i].act_life < 10 {
                        self.g.ent[i].f71 = 12;
                    } else if self.g.ent[i].f44 & 1 != 0 {
                        self.g.ent[i].f71 = 6;
                    } else {
                        self.g.ent[i].f26 -= 1;
                        let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
                        let dx = (ctx.px as i32 - ex as i32) as i16 as i32;
                        let dy = (ctx.py as i32 - ey as i32) as i16 as i32;
                        let near = dx * dx + dy * dy < 0x2000i32.pow(2);
                        if near && self.g.ent[i].f26 <= 0 {
                            let d = self.g.ent_rand(i);
                            self.g.ent[i].f71 = if d % 0xC < 9 { 4 } else { 6 };
                        }
                    }
                }
                4 => {
                    self.g.ent[i].f71 = 5;
                    self.g.ent[i].f26 = 6;
                    self.g.ent[i].f69 = 2;
                    self.g.ent[i].f46 = 113;
                    fall = true; // states 4+5 span 6 ticks INCL. entry
                }
                5 => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 6;
                    }
                }
                6 => {
                    self.g.ent[i].f71 = 7;
                    self.g.ent[i].f26 = 16;
                    self.g.ent[i].f69 = 0;
                    self.g.ent[i].f46 = 113;
                    self.g.mc2_pyramid_sprite(i, 343);
                    fall = true;
                }
                7 => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 8;
                    }
                }
                8 => {
                    self.g.ent[i].f71 = 9;
                    self.g.ent[i].f26 = 0;
                    self.g.ent[i].f69 = 3;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_pyramid_sprite(i, 342);
                    self.mc2_pyramid_pick_summon(i);
                    fall = true; // the pick AND the first shot same tick
                }
                9 => {
                    self.mc2_pyramid_do_summon(i, ctx);
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 10;
                    }
                }
                0xA => {
                    self.g.ent[i].f71 = 11;
                    self.g.ent[i].f26 = 16;
                    self.g.ent[i].f46 = 22;
                    self.g.mc2_pyramid_sprite(i, 344);
                    fall = true;
                }
                0xB => {
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 2;
                    }
                }
                0xC => {
                    // Death script begins: the (10,17) doomsday sphere.
                    self.g.ent[i].f71 = 13;
                    self.g.ent[i].f26 = 32;
                    let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
                    if let Some(s) = self.g.mc2_spawn_meteor(x, y, 0) {
                        self.g.ent[s].z = 0;
                        self.g.ent[s].max_life = 70;
                        self.g.ent[s].act_life = 70;
                        self.g.ent[s].id24 = PLAYER_TARGET;
                    }
                    fall = true;
                }
                0xD => {
                    death_sound = true;
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 14;
                        self.g.ent[i].f26 = 32;
                        self.g.mc2_pyramid_sprite(i, 345);
                    }
                }
                0xE => {
                    death_sound = true;
                    self.g.snd(10, i);
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        self.g.ent[i].f71 = 15;
                        self.g.ent[i].f26 = 60;
                        self.g.ent[i].act_life = -1;
                        // `byte[0] |= 1` (EF:12846): the death re-sets
                        // the hidden bit the kill-all exit dropped.
                        self.g.ent[i].flags |= 1;
                        self.mc2_kill_all_creatures();
                        // The life reset walks `dword_38523` — the SPHERE
                        // family (10, 39/40/57) — not the whole pool
                        // (EF:12847-54).
                        //
                        // ⭐⭐⭐ `byte[1] |= 0x20` IS NOT A RENDER BIT —
                        // IT IS THE DECAY CHANNEL. The module doc used
                        // to call it "an unmapped render-side bit
                        // (name-inferred)" and skip it; port flag bit 13
                        // is the very channel [`Gen::ball_decay_tail`]
                        // (mc1::combat, the MC2 sphere mover's tail
                        // EF:26289-307) and the balloon-pick refusal
                        // (mc2::castle, EF:61009) already read, and
                        // which mc2::morph's mana rain already sets.
                        // Without it the apocalypse spheres are re-lifed
                        // to 140 and then NEVER count down — no fade, no
                        // expiry, and a balloon fleet that will happily
                        // fly for them.
                        //
                        // EF:12848-54:
                        //   for (ix = ...dword_38523; ix > Entities[0]; ix = ix->next_0)
                        //   { v19 = ix->struct_byte_0xc_12_15.byte[1];
                        //     ix->maxLife_0x4 = 140;
                        //     ix->struct_byte_0xc_12_15.byte[1] = v19 | 0x20;
                        //     ix->life_0x8 = ix->maxLife_0x4; }
                        //
                        // Shipped `NETHERW.EXE`, the case-0xE arm of
                        // `sub_21AB0` — the loop that follows the
                        // KillAllCreatures call at file 0x45BA6
                        // (`e8 45 a2 ff ff` -> 0x3FDF0, the same call
                        // site the chain-members fixture already cites),
                        // whose head loads `dword_38523` at 0x45BB0
                        // (`8b 80 7b 96 00 00`) and whose back-edge is
                        // 0x45BE1 `77 e0`. Body, file 0x45BC3-0x45BD8
                        // (VA 0x213C3; file = VA + 0x24800):
                        //   0x45BC3 `8a 50 0d`             mov dl,[eax+0xD]
                        //   0x45BC6 `c7 40 04 8c 00 00 00` mov [eax+4],140
                        //   0x45BCD `80 ca 20`             or  dl,0x20
                        //   0x45BD0 `88 50 0d`             mov [eax+0xD],dl
                        //   0x45BD3 `8b 50 04`             mov edx,[eax+4]
                        //   0x45BD6 `89 50 08`             mov [eax+8],edx
                        // (`eax+0xC` is the flags word, so `+0xD` is
                        // byte[1] and `0x20` there is bit 13 = 0x2000.)
                        //
                        // WITNESS — mc2l24 t=52751->52752, the pyramid's
                        // death tick: retail's slot 7 `(10,39)` reads
                        // `max_life 300 -> 140, life 300 -> 139,
                        // flags 12 -> 8204` (8204 = 12 | 0x2000) and
                        // `flags.b1_decay20 0 -> 1`. The 139 is not a
                        // second constant: the sphere's OWN mover runs
                        // later in the same tick (slot 7 > the pyramid's
                        // slot 5) and the freshly-armed decay tail takes
                        // the first tick off. ~274 records carried the
                        // off-by-one.
                        for e in self.g.ent.iter_mut().skip(1) {
                            if e.class64 == 10 && matches!(e.model65, 39 | 40 | 57) {
                                e.max_life = 140;
                                if !no_mc2_doom_sphere_decay_bit() {
                                    e.flags |= 0x2000;
                                }
                                e.act_life = 140;
                            }
                        }
                    }
                }
                0xF => {
                    self.mc2_kill_all_creatures();
                    death_sound = true;
                    self.g.ent[i].f26 -= 1;
                    if self.g.ent[i].f26 <= 0 {
                        // THE APOCALYPSE: the (10,9) dome in its endgame
                        // variant — create, force fields, THEN latch
                        // (the order is load-bearing: the ctor call site
                        // clears the latch, EF:12864-12872).
                        let (x, y, z) = {
                            let e = &self.g.ent[i];
                            (e.x, e.y, e.z)
                        };
                        self.mc2_apocalypse = false;
                        if let Some(d) = self.g.mc2_spawn_dome(x, y, z) {
                            self.g.ent[d].act_life = 32;
                            self.g.ent[d].max_life = 11;
                            self.g.ent[d].id24 = PLAYER_TARGET;
                            self.mc2_apocalypse = true;
                        }
                        // Retail leaves the doom meter AT 1200 here — no
                        // zero write in case 0xF.
                        self.g.ent[i].flags |= 0x400;
                    }
                }
                _ => {}
            }
        }
        // LABEL_48: the death-phase rumble + ground-clamp/facing.
        if death_sound && self.g.ent[i].f63 & 3 == 0 {
            self.g.snd(63, i);
        }
        self.g.mc2_pyramid_face(i, ctx);
        // ⭐⭐⭐ THE RENDERER'S ARM IS NOT DISTANCE-GATED, AND IT RUNS
        // **AFTER** THE TICK. `GameRenderOriginal.cpp` (the sprite
        // pass's `a1 == 1` arm, mirrored in the NG/HD renderers) ends
        //   `if (str_F2C20ar.dword0x14x->struct_byte_0xc_12_15.byte[3] & 0x40)`
        //   `    str_F2C20ar.dword0x14x->subSpellIndex_0x2A_42 |= 0x40u;`
        // — the ONLY setter of the wind-down escape bit in the engine
        // (`NETHERW.EXE` @0x45F11; every other site only ANDs it off).
        // It fires for EVERY DRAWN record carrying the ctor's flag bit
        // 30, with no proximity test whatsoever, and the frame is drawn
        // after the sim tick, so the RECORDED boundary value always
        // carries the bit once the boss is visible.
        //
        // The one gate the corpus does show is the ctor's `byte[0] & 1`
        // (the port's `flags & 1`, raised by the `|= 0x48800001` stamp
        // and dropped by the kill-all exit's `byte[0] &= 0xFE`,
        // EF:12983): while it is up the pyramid is the DORMANT sleeper
        // and never reaches the sprite pass.
        //
        // MEASURED, mc2l24 slot 5 (`dump-state`, retail's own `f2a`):
        //   t=44380 f2a 0   flags …8D (bit0 up)   — dormant, no 0x40
        //   t=44467 f2a 4   flags …8D             — still dormant
        //   t=44468 f2a 80  flags …8C (bit0 down) — the kill-all exit
        //                   sets 0x10 and the SAME tick's draw adds 0x40
        //   t=44490 f2a 80                        — far: the machine
        //                   clears 0x40, the draw puts it straight back
        //   t=44491 f2a 96  flags loses bit 23    — near: 0x10 → 0x20
        //   t=44600 f2a 66                        — 0x40 still held
        // The pre-dig proximity analog reproduced every one of those
        // EXCEPT the far window, where it left the bit down: `(5,10)
        // f2a` retail 80 / port 16, mc2l24 23 rows t=44468..44490
        // slot 5 and mc2l24-crazy 72 rows t=52527..52656 slot 6.
        //
        // BEHAVIOURALLY the two arms agree: the machine's `bits & 0x10`
        // branch is the only reader, and its far arm does nothing but
        // clear the bit again (EF:13010).
    }

    /// `sub_21F60` (EF:13519-13620) — the DEVOUR pass: the pyramid
    /// eats incoming class-9 spell PROJECTILES (an anti-magic zone —
    /// it scans projectiles, NOT creatures, and there is no
    /// player-proximity trip). Eligible subtypes within
    /// 0xC00 (3-D) are absorbed: a (10,0) mana-absorb spawns at the
    /// projectile (owner = the pyramid) and it despawns. Subtype 10
    /// (the castle-build projectile) instead tests the pyramid's
    /// (5120,5120) exclusion box against the player's castle (or the
    /// landing spot) and, devoured, CANCELS the Castle spell (the
    /// manifestation window zeroed). Trip = devoured anything this
    /// tick OR the player's Rebound window is live (EF:13616-18) —
    /// Rebound would reflect the pyramid's shots, so it switches to
    /// the un-reboundable beam.
    fn mc2_pyramid_devour(&mut self, i: usize) -> bool {
        let (ex, ey, own) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.id24)
        };
        let mut devoured = false;
        for j in 1..self.g.ent.len() {
            let (sub, x, y, z) = {
                let e = &self.g.ent[j];
                if e.class64 != 9 || e.flags & 0x400 != 0 {
                    continue;
                }
                (e.model65, e.x, e.y, e.z)
            };
            let eat = if DEVOUR_SUBTYPES.contains(&sub) {
                // 2-D: retail's `EuclideanDistXYZ_58490` (EF:13567)
                // never reads z — the absorb bubble is a CYLINDER,
                // not a sphere. A 3-D test leaks vertically-offset
                // projectiles through.
                let dx = (x.wrapping_sub(ex) as i16) as i64;
                let dy = (y.wrapping_sub(ey) as i16) as i64;
                dx * dx + dy * dy <= 0xC00 * 0xC00
            } else if sub == 10 {
                // The castle-build projectile: bbox overlap between
                // the pyramid's (5120,5120) box and the player's
                // castle extents (else the projectile's own spot).
                let (tx, ty, hx, hy) = match self.player_castle() {
                    Some(c) => {
                        let e = &self.g.ent[c];
                        (e.x, e.y, e.f80 as i32, e.f82 as i32)
                    }
                    None => (x, y, 5120, 5120),
                };
                let dx = ((tx.wrapping_sub(ex)) as i16 as i32).abs();
                let dy = ((ty.wrapping_sub(ey)) as i16 as i32).abs();
                dx <= 5120 + hx && dy <= 5120 + hy
            } else {
                false
            };
            if !eat {
                continue;
            }
            devoured = true;
            if sub == 10 {
                // Cancel the Castle spell (EF:13608-09; guarded —
                // retail writes the entity-0 sentinel unguarded).
                let m = self.mc2_book.ent[2] as usize;
                if m != 0 {
                    self.g.ent[m].f26 = 0;
                }
            }
            if let Some(s) = self.g.mc2_spawn_fire(x, y, z) {
                self.g.ent[s].id24 = own;
            }
            self.g.ent[j].flags |= 0x400;
        }
        // The Rebound trip (EF:13616-18):
        //
        // ```c
        //   v7 = v16x->…SpellsEnabled_0x333_819x.SpellEnabled[8];
        //   if (v7 && Entities_EA3E4[v7]->word_0x2E_46 > 0) v19 = 1;
        // ```
        //
        // ⭐⭐ AND THE INDEX IS ROUTINELY A MARKER, NOT A SLOT — the
        // same law `mc2_owner_castle_token` carries: the wizard death
        // handler `sub_5E310` stamps a boolean **1** into every
        // occupied book slot, so after the human dies this arm
        // dereferences POOL SLOT 1, whatever sits there. Retail reads
        // raw bytes at @0x2E; ours are HOMED PER CLASS
        // (`port_ent_lanes_mc2`'s `f2e` arm), and `f26` is @0x2E only
        // on a class-15 manifestation. Reading `f26` unconditionally
        // read a DIFFERENT PHYSICAL FIELD the moment the marker
        // pointed anywhere else.
        // MEASURED (mc2l24 t=45560, `--start 45559`): the human dies
        // at t=45559 and `spell_ent[8]` goes 42 -> 1 (the marker);
        // pool slot 1 is a `(14,1)` whose @0x2E is **0** — retail does
        // not trip and holds `word_0x2C_44` 0x42 — while our `f26`
        // there is **15** (the record's own @0x10 scratch), so the
        // port tripped bit 0 and carried 0x43 for the rest of the
        // take.
        let m8 = self.mc2_book.ent[8] as usize;
        let rebound = m8 != 0 && {
            let e = &self.g.ent[m8];
            let at2e = if no_mc2_devour_rebound_at_2e() {
                e.f26
            } else if e.class64 == 3 && e.model65 == 2 {
                e.f59 as i16 // the castle's @0x2E
            } else if e.class64 == 15 {
                e.f26
            } else if e.class64 == 5 {
                e.lease()
            } else {
                e.f46
            };
            at2e > 0
        };
        devoured || rebound
    }

    /// The BLDGPRM-68 building — the port's stand-in for retail's
    /// `D41A0_0.word_0x3654A` (`sub_49A30` EF:32850 sets it at the
    /// id-68 spawn, EF:28180 clears it at that building's raze). The
    /// port banks the id on the entity itself (`f71 = bldg`,
    /// `mc2_spawn_building`), so the live record IS the register.
    ///
    /// ⚠ ONE KNOWN DIFFERENCE, and it is an APPROX not a law: retail's
    /// global is a RAW SLOT INDEX that survives the building's death —
    /// on mc2l24 slot 169 is recycled into a `(9,9)` by t=49336 and
    /// @0x3654A still reads 169, so retail keeps devouring around
    /// whatever now occupies the slot (the MC2-reads-out-of-bounds
    /// class). A live-record lookup stops at the citadel's death. The
    /// faithful fix is a `u16` world register with an import seat off
    /// `RetailMc2 @0x3654A`, the way LAW 6 seated @0x36546.
    pub(crate) fn mc2_doom_citadel(&self) -> Option<usize> {
        (1..self.g.ent.len()).find(|&i| {
            let e = &self.g.ent[i];
            e.class64 == 10 && e.model65 == 45 && e.f71 == 68 && e.flags & 0x400 == 0
        })
    }

    /// `sub_21F60`'s `v17 == true` arm, driven from `UpdateEntities`'
    /// tail on [`World::mc2_doom_citadel`] — see
    /// [`no_mc2_citadel_devour`] for the bytes and the call-path scan.
    /// Same devourable subtype set and same effects as the pyramid's
    /// arm ((10,0) absorb owned by the devourer + despawn, and the
    /// Castle-spell cancel on the model-10 shot); only the geometry
    /// differs, and the return is discarded.
    pub(crate) fn mc2_citadel_devour(&mut self, a: usize) {
        if no_mc2_citadel_devour() {
            return;
        }
        let (ax, ay, sx, sy, own) = {
            let e = &self.g.ent[a];
            (e.x, e.y, e.f80 as i16 as i32, e.f82 as i16 as i32, e.id24)
        };
        for j in 1..self.g.ent.len() {
            let (sub, x, y, z) = {
                let e = &self.g.ent[j];
                if e.class64 != 9 || e.flags & 0x400 != 0 {
                    continue;
                }
                (e.model65, e.x, e.y, e.z)
            };
            // The model-10 castle-build shot is measured against the
            // PLAYER'S CASTLE (EF:13580-88), everything else against
            // the projectile itself with the {5120, 5120} default.
            let (tx, ty, hx, hy) = if sub == 10 {
                match self.player_castle() {
                    Some(c) => {
                        let e = &self.g.ent[c];
                        (e.x, e.y, e.f80 as i16 as i32, e.f82 as i16 as i32)
                    }
                    None => (x, y, 5120, 5120),
                }
            } else if DEVOUR_SUBTYPES.contains(&sub) {
                (x, y, 5120, 5120)
            } else {
                continue;
            };
            let dx = ((tx.wrapping_sub(ax)) as i16 as i32).abs();
            let dy = ((ty.wrapping_sub(ay)) as i16 as i32).abs();
            if dx >= sx + hx || dy >= sy + hy {
                continue;
            }
            if sub == 10 {
                let m = self.mc2_book.ent[2] as usize;
                if m != 0 {
                    self.g.ent[m].f26 = 0;
                }
            }
            if let Some(s) = self.g.mc2_spawn_fire(x, y, z) {
                self.g.ent[s].id24 = own;
            }
            self.g.ent[j].flags |= 0x400;
        }
    }

    /// `sub_21490` (EF:12886) — the phase-bit attack driver. Returns
    /// "idle" (no bit set) so the machine escalates.
    fn mc2_pyramid_attack(&mut self, i: usize, _ctx: &MobCtx, cx: u8, cy: u8) -> bool {
        let bits = self.g.ent[i].f44;
        let mut idle = false;
        let mut suppress_ring = false;
        if bits & 8 != 0 {
            // The terrain-flatten crater.
            if self.g.ent[i].f26 < 0 {
                // Expansion done: radius-7 disc fully flat?
                let flat = self.g.ring_cells(0, 7).iter().all(|&(dx, dy)| {
                    self.g.t.tile_type[tile(cx.wrapping_add(dx), cy.wrapping_add(dy))] == 0
                });
                if flat {
                    self.g.ent[i].f44 = (self.g.ent[i].f44 | 4) & !8;
                    self.g.ent[i].f26 = 70;
                } else {
                    self.g.ent[i].f26 = 15;
                }
            } else {
                self.g.snd(10, i);
                let radius = (15 - self.g.ent[i].f26).clamp(0, 15) as i32;
                for (dx, dy) in self.g.ring_cells(0, radius) {
                    self.g
                        .mc2_doom_flatten_cell(cx.wrapping_add(dx), cy.wrapping_add(dy));
                }
                self.terrain_dirty = true;
                self.g.ent[i].f26 -= 1;
            }
        } else if bits & 4 != 0 {
            // The kill-all countdown (70 ticks; the 0x23/0x11 render
            // checkpoints are the fade bits — presentation-skipped;
            // checkpoint 1's global wipe lands).
            self.mc2_kill_all_creatures();
            let v7 = self.g.ent[i].f26;
            self.g.ent[i].f26 -= 1;
            if v7 == 70 {
                // First tick of the kill-all phase: retail zeroes
                // `countStageVars_0x36E00` — the whole hold-gate/
                // objective StageVar subsystem dies with the world
                // (EF:12996-98). Clearing the vec is the port's
                // registration-count zero.
                self.mc2_stagevars.clear();
            }
            if v7 == 1 {
                // Checkpoint 1 despawns `dword_38523` — the sphere
                // family (10, 39/40/57) — NOT the world: retail's
                // v29==3 arm runs DisableEntityDrawing over that list
                // only (EF:13048-66). Castles,
                // wizards and effects survive the activation crater.
                for e in self.g.ent.iter_mut().skip(1) {
                    if e.class64 == 10 && matches!(e.model65, 39 | 40 | 57) {
                        e.flags |= 0x400;
                    }
                }
                self.entities_dirty = true;
            } else if v7 <= 0 {
                self.g.ent[i].f44 = (self.g.ent[i].f44 | 0x10) & !4;
                self.g.ent[i].f26 = 1;
                // `byte[0] &= 0xFE` (EF:12983): the kill-all exit
                // drops the ctor's hidden bit (0x48800001 & 1) — from
                // here the STANDARD proximity self-wake (sub_68C70)
                // applies, so a player closing in re-arms f58 and the
                // damage intake (`mc2_pyramid_mail`, f58-gated) goes
                // live. The boss is dormant-invulnerable only through
                // its opening ritual — this bit clear is what makes
                // him ultimately killable.
                self.g.ent[i].flags &= !1;
            }
        } else if bits & 0x10 != 0 {
            if self.g.ent[i].f26 == 1 {
                self.g.ent[i].f26 = 0;
                self.g.ent[i].f44 &= !0x40;
            } else if bits & 0x40 != 0 {
                // The wake gate (EF:13010):
                // `EuclideanDistXYZ_58490(&player.pos, &self.pos) >=
                // 0xA00`. The NAME LIES — Maths.cpp:738's body is
                // `radix = (int16)(dx)² + (int16)(dy)²` and nothing
                // else: **Z is never read**, so the gate is a flat 2-D
                // circle of radius 0xA00 (10 tiles) around the boss,
                // not a sphere and not a Manhattan diamond. The return
                // is `sub_7277A_radix_3d(radix)` (Maths.cpp:744) — a
                // Heron integer sqrt seeded from `x_WORD_727B0[bsr]`
                // that terminates on `radix / i >= i`, i.e. an exact
                // FLOOR sqrt. `floor(sqrt(r)) >= 0xA00` and
                // `r >= 0xA00²` are therefore the same predicate, so
                // the squared form below IS retail's metric, boundary
                // included. (Widened to i64 for the same reason retail
                // accumulates into a `uint32_t`: two i16 legs can sum
                // to 2³¹ and the i32 form wrapped negative there.)
                let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
                let dx = (_ctx.px as i32 - ex as i32) as i16 as i64;
                let dy = (_ctx.py as i32 - ey as i32) as i16 as i64;
                if dx * dx + dy * dy >= 0xA00i64.pow(2) {
                    self.g.ent[i].f44 &= !0x40;
                } else {
                    self.g.ent[i].f26 = 30;
                    self.g.ent[i].f44 = (self.g.ent[i].f44 | 0x20) & !0x10;
                    // `byte[2] &= 0x7F` (EF:13024), verbatim: the boss
                    // drops the ctor's raster-mode bit (flags bit 23)
                    // as the doom meter starts ramping — the DORMANT
                    // sprite draws through the special colour path
                    // (rotIdx 2, GameRenderOriginal.cpp:3798-3805) and
                    // the ACTIVE one through the plain descriptor.
                    // Corpus: mc2l24 slot 7 flags 0x4880000c →
                    // 0x4800000c at exactly t=51732.
                    self.g.ent[i].flags &= !(1 << 23);
                }
            }
        } else if bits & 0x20 != 0 {
            // The HUD doom-meter ramp (0..1200).
            if self.g.ent[i].f26 >= 600 {
                suppress_ring = true;
            }
            let v = (self.g.ent[i].f26 + 30).min(1200);
            self.g.ent[i].f26 = v;
            if v >= 1200 {
                self.g.ent[i].f44 &= !0x20;
            }
            self.mc2_doom_meter = v;
        } else {
            idle = true;
        }
        // The spinning (10,14) falling-rock summon ring.
        if !suppress_ring {
            let spin = self.g.ent[i].f36.wrapping_add(96) & 0x7FF;
            self.g.ent[i].f36 = spin;
            let (ex, ey, ez) = {
                let e = &self.g.ent[i];
                (e.x, e.y, e.z)
            };
            for k in 0..4u16 {
                let ang = spin.wrapping_add(512 * k) & 0x7FF;
                let mut p = (ex, ey, ez);
                Gen::polar_step(&mut p, ang, 0, 192);
                if let Some(r) = self.g.mc2_spawn_smoke_particle_for(14, p.0, p.1, p.2) {
                    // Each successful spawn draws the PYRAMID's own
                    // LCG once for the rock's life (EF:13086-87 — on
                    // the pyramid's stream, NOT the rock's).
                    let d = self.g.ent_rand(i);
                    self.g.ent[r].act_life = ((d & 7) + 8) as i32;
                }
            }
        }
        idle
    }

    /// `sub_21850` (EF:13101-13265) — pick the summon: a weighted
    /// roll over creatures (population-capped), projectile bursts,
    /// or the player beam. Retail quirks: the f26/f38/f50 writes
    /// PRECEDE the cap test (a cap-failed roll still mutates them);
    /// the caps for picks 4/6 are evaluated against the MODEL-0
    /// population (verbatim — sub_223E0's three identical bucket-0
    /// loops; only picks 3 and 5 count their own kind, 5 excluding
    /// action 200); roll-2 picks 8/9 fire ONE shot (f38=1, f26=5);
    /// the bit7 escalation forces roll 1 to 0; a trip while asleep
    /// writes NO pick fields; the trip-laser re-arms the beam ramp
    /// (bit1).
    fn mc2_pyramid_pick_summon(&mut self, i: usize) {
        // sub_223E0's population counts over the class-5 buckets
        // (live + bucketed: life >= 0, action not a corpse state).
        // ⭐⭐ THE POPULATION COUNTS ARE CHAIN LENGTHS. NETHERW.EXE
        // 0x46bf8 / 0x46c1a / 0x46c3c all load `0x9603(%ecx)` —
        // `bytearray_38403x[0]`, the MODEL-0 head — and 0x46c5e loads
        // `0x9667(%ecx)` = +0x64 = chain[25]; each `mov (%eax),%eax`
        // walks `next_0` and increments. The only body test in the four
        // loops is 0x46c66 `80 78 45 c8  cmpb $0xc8,0x45(%eax)` — the
        // action-200 skip on chain 25. Membership is the tick-top
        // rebuild's ([`Gen::mc2_roster`]); the port's live `flags &
        // 0x400` / `act_life` / action re-reads are not retail's.
        let count = |g: &Gen, m: u8, excl_200: bool| -> usize {
            if crate::engine::features::no_mc2_mob_chain_predicate() {
                return g
                    .ent
                    .iter()
                    .skip(1)
                    .filter(|e| {
                        e.class64 == 5
                            && e.model65 == m
                            && e.flags & 0x400 == 0
                            && e.act_life >= 0
                            && !matches!(e.tick70, 0xB4 | 0xE8 | 0xEA)
                            && !(excl_200 && e.tick70 == 200)
                    })
                    .count();
            }
            g.mc2_roster(m)
                .iter()
                .filter(|&&s| !(excl_200 && g.ent[s as usize].tick70 == 200))
                .count()
        };
        // bit1 cleared on every entry (EF:13122).
        self.g.ent[i].f44 &= !2;
        let mut laser = false;
        let mut picked: Option<u8> = None;
        if self.g.ent[i].f44 & 1 != 0 {
            // The devour/rebound trip: forced laser when awake, WITH
            // the bit1 beam-ramp re-arm (EF:13127-31). Asleep: bit0
            // clears and NOTHING else is written (stale selector).
            self.g.ent[i].f44 &= !1;
            if self.g.ent[i].f58 != 0 {
                laser = true;
                self.g.ent[i].f44 |= 2;
            }
        } else {
            self.g.ent[i].f44 |= 2;
            // Retail draws UNCONDITIONALLY (EF:13137-39) with the
            // setting_30 perturb (:13140); bit7 — the post-opening
            // escalation — only overrides the ROLL to 0 afterwards
            // (:13141-45), straight to the projectile roll. The
            // stream steps either way.
            let d = self.g.ent_rand(i) % 0x46;
            self.g.mc2_rand_perturb(i, self.mc2_turn);
            let v4 = if self.g.ent[i].f44 & 0x80 != 0 {
                self.g.ent[i].f44 &= 0x7F;
                0
            } else {
                d
            };
            let creature_writes = |w: &mut Self, f38: i16, f50: i16| {
                w.g.ent[i].f26 = 8;
                w.g.ent[i].f38 = f38 as u16;
                w.g.ent[i].f50 = f50;
            };
            match v4 {
                3..=6 => laser = true,
                40..=48 => {
                    creature_writes(self, 8, 256);
                    if count(&self.g, 0, false) < 28 {
                        picked = Some(6);
                    }
                }
                49..=58 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 0, false) < 4 {
                        picked = Some(3);
                    }
                }
                59..=68 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 25, true) < 6 {
                        picked = Some(5);
                    }
                }
                _ if v4 >= 69 => {
                    creature_writes(self, 3, 682);
                    if count(&self.g, 0, false) < 12 {
                        picked = Some(4);
                    }
                }
                _ => {}
            }
            if picked.is_none() && !laser {
                // Roll 2 carries the same perturb (EF:13218-20).
                let d2 = self.g.ent_rand(i) % 0x1D;
                self.g.mc2_rand_perturb(i, self.mc2_turn);
                match d2 {
                    0..=7 => {
                        picked = Some(1);
                        self.g.ent[i].f38 = 10;
                        self.g.ent[i].f26 = 10;
                    }
                    8..=17 => {
                        picked = Some(2);
                        self.g.ent[i].f38 = 8;
                        self.g.ent[i].f26 = 8;
                    }
                    18..=25 => {
                        picked = Some(9);
                        self.g.ent[i].f38 = 1;
                        self.g.ent[i].f26 = 5;
                    }
                    26..=27 => {
                        picked = Some(8);
                        self.g.ent[i].f38 = 1;
                        self.g.ent[i].f26 = 5;
                    }
                    _ => laser = true,
                }
            }
        }
        if laser {
            picked = Some(7);
            self.g.ent[i].f38 = 24;
            self.g.ent[i].f26 = 32;
        }
        if let Some(p) = picked {
            self.g.ent[i].f68 = p;
        }
    }

    /// `sub_21AB0` (EF:13270-13511) — execute the summon/fire each
    /// state-9 tick while the repeat count lasts. Every launch
    /// shares the preamble (EF:13317-25): pyramid pos stepped 640
    /// along the pyramid yaw at z+768; creatures step 1792 further
    /// at the stride bearing (NOT from the raw center).
    fn mc2_pyramid_do_summon(&mut self, i: usize, ctx: &MobCtx) {
        // ⭐⭐⭐ THE WHOLE BARRAGE — THE REPEAT-COUNT DECREMENT
        // INCLUDED — IS GATED ON THE HUMAN WIZARD'S POOL RECORD BEING
        // LIVE. `sub_21AB0` opens by loading `Entities[playerIndex]`
        // and bailing to its epilogue on any of THREE clauses
        // (EF:13312, shipped `NETHERW.EXE` file 0x462E7-0x4631A,
        // linear 0x21AE7; the whole function body is inside the
        // `if`):
        //
        // ```text
        //   462e7  8b 04 85 e4a30100  mov  eax,[Entities + eax*4]
        //   462ee  8b 3d e4a30100     mov  edi,[Entities]
        //   462f7  39 f8              cmp  eax,edi
        //   462f9  0f 86 57040000     jbe  0x46756   ; ptr <= Entities[0] -> EXIT
        //   462ff  83 78 08 00        cmpl [eax+0x8],0
        //   46303  0f 8c 4d040000     jl   0x46756   ; life_0x8 < 0   -> EXIT
        //   46309  f6 40 0d 04        testb [eax+0xd],0x4
        //   4630d  0f 85 43040000     jne  0x46756   ; byte[1]&4 reap -> EXIT
        //   46313  66 8b 53 24        mov  dx,[ebx+0x24]   ; word_0x24_36
        //   46317  66 85 d2           test dx,dx
        //   4631a  0f 86 1c040000     jbe  0x4673c   ; count == 0
        // ```
        //
        // 0x46756 is the epilogue — the `word_0x24_36--` at 0x4631d
        // is never reached — so a dead player FREEZES the count where
        // it stands instead of burning it. `e8 rel32` scan of the
        // shipped EXE: `sub_21AB0` has exactly ONE caller (0x45A91 =
        // the action's `case 9`, EF:12791), so this is the whole law.
        //
        // Our human lives outside the pool (and in conformance import
        // its slot is a zeroed husk), so `ctx.pdead` — the same
        // `life_0x8 < 0` read every other follower/leader site takes
        // through the ctx — stands in for the pool read; the pyramid
        // is walked long before the carpet slot, so the mid-walk
        // republish is moot here.
        //
        // MEASURED (mc2l24, pair lane, `--start t-1` local arms): the
        // human's record goes `life 410 -> -1190` during t=45558; the
        // recorded pyramid (slot 5) burns `word_0x24_36` 7/5/4/3/2
        // over t=45553..45558 and then FREEZES at 2 from t=45559 on,
        // while the port kept firing one `(9,0)` a tick. Every extra
        // fireball pops the free stack from slot 5 — the FIRST walk
        // position in the tick — so the whole tick's allocation
        // sequence slid one slot: at t=45560 the (10,76) firestorm's
        // hub took 17 and its 25 satellites 18..78 where retail put
        // the hub on 214 and the ring on 17..70 (pair 45559->45560,
        // 139 field rows + one extra entity), and at t=45559 the
        // (10,1) death blast took 24 instead of 855.
        if ctx.pdead && !no_mc2_pyramid_dead_player_gate() {
            return;
        }
        if self.g.ent[i].f38 == 0 {
            return;
        }
        self.g.ent[i].f38 -= 1;
        let (ex, ey, ez, own_id) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.id24)
        };
        let tpos = (ctx.px, ctx.py, ctx.pz);
        // The shared launch point (EF:13321-25).
        let mut lp = (ex, ey, ez);
        Gen::polar_step(&mut lp, self.g.ent[i].f30, 0, 640);
        lp.2 = ez.wrapping_add(768);
        match self.g.ent[i].f68 {
            1 => {
                if let Some(p) = self.g.mc2_spawn_bolt(lp.0, lp.1, lp.2) {
                    // Retail arming (EF:13315-18): impact (10,0)
                    // fire, behavior row 62 (the ctor's 64 is the
                    // generic bolt row — wrong turn caps), f44 800.
                    self.g.ent[p].f44 = 800;
                    self.g.ent[p].f68 = 10;
                    self.g.ent[p].f69 = 0;
                    self.g.ent[p].row156 = 62;
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.mc2_pyramid_arm_tail(p);
                    self.g.snd(15, i);
                }
            }
            2 => {
                if let Some(p) = self.g.mc2_spawn_bolt9(lp.0, lp.1, lp.2) {
                    // Retail arming (EF:13327-30): impact = the
                    // (10,23) BLAST (the ctor default spawned plain
                    // fire), row 62.
                    self.g.ent[p].f44 = 800;
                    self.g.ent[p].f68 = 10;
                    self.g.ent[p].f69 = 23;
                    self.g.ent[p].row156 = 62;
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.mc2_pyramid_arm_tail(p);
                    self.g.snd(23, i);
                }
            }
            3..=6 => {
                // The creature summon ring: aim stride × the ALREADY
                // DECREMENTED repeat (EF:13364), stepped 1792 from
                // the shared point, z re-forced +768 (EF:13365-68).
                let sel = self.g.ent[i].f68;
                let stride = self.g.ent[i].f50 as u16;
                let ang = stride
                    .wrapping_mul(self.g.ent[i].f38)
                    .wrapping_add(self.g.ent[i].f30)
                    & 0x7FF;
                let mut p = lp;
                Gen::polar_step(&mut p, ang, 0, 1792);
                p.2 = ez.wrapping_add(768);
                let spawned = match sel {
                    3 => self.g.mc2_spawn_m0(p.0, p.1, p.2),
                    4 => self.g.mc2_spawn_m21(p.0, p.1, p.2),
                    5 => self.g.mc2_spawn_m25(p.0, p.1, p.2),
                    _ => self.g.mc2_spawn_m19(p.0, p.1, p.2),
                };
                if let Some(s) = spawned {
                    // `dword_0x364D2++` (EF:13341) — the stats ledger's
                    // creature census, on a successful summon.
                    self.g.stats.0.census += 1;
                    // The summoned-creature writes (EF:13388-13425):
                    // stage tag 17, parent = the pyramid,
                    // and the ACTION OVERRIDES over the creators'
                    // defaults — written LAST (m0 1→7, m21 169→175,
                    // m25 201→207, m19 153→159).
                    let e = &mut self.g.ent[s];
                    e.f146 = PLAYER_TARGET;
                    e.id24 = own_id;
                    e.site_z = 17; // StageVar2_0x49_73
                    // `word_0x2E_46 = 250` (EF:13419) — the release
                    // chain's LIFE LATCH. Its class-5 home is f26 (the
                    // creature column's @0x2E charm/armed lane,
                    // conformance.rs's class-5 import), NOT f46: f46 is
                    // `fontTypeIndex_0x3D_61` on a creature, which for
                    // the selector-3 (5,0) worm IS the projectile-dodge
                    // alert window (`m0_dodge`, multipart.rs) — stamping
                    // 250 there armed 250 ticks of phantom dodging AND
                    // left the latch with no import home, so every
                    // imported pyramid summon read f46 ≈ 0 and puffed
                    // itself on its first replayed tick.
                    // dig 98-Q20: @0x2E's own home (`lease2e`).
                    e.set_lease(250);
                    e.f126 = 320;
                    // `parentId_0x28_40 = pyramid` is unmodeled: the
                    // port has no creature parent-link home (f40 is
                    // the roster's attacker word) — the consumers
                    // (the StageVar2 16/17 release chain, mobs.rs
                    // mc2_doom_summon_*) scan-resolve the level's
                    // single (5,10) instead. The dword_0x364D2 tally is
                    // the total-creatures-spawned DENOMINATOR of the
                    // level-complete "creatures killed %" stat
                    // (EF:43498-505; ++ at EF:13390/32988, boxed-in
                    // walkers decrement, EF:8860) — wired as
                    // `engine::stats`'s census, bumped above.
                    e.f30 = ang;
                    e.f34 = ang;
                    e.tick70 = match sel {
                        3 => 7,
                        4 => 175,
                        5 => 207,
                        _ => 159,
                    };
                    self.g.snd(
                        match sel {
                            3 => 8,
                            4 => 42,
                            5 => 37,
                            _ => 44,
                        },
                        i,
                    );
                }
            }
            7 => {
                // THE HURL-AWAY BEAM (EF:13427-56): ramp
                // 1024 → −80/tick → floor 10, applied OUTWARD along
                // pyramid→player. The pose displacement rides the
                // shared knock channel at the FULL ramp magnitude
                // (retail: MoveEntity + moveTest + floor clamp on
                // the pose — the app owns the pose; module APPROX).
                if self.g.ent[i].f44 & 2 != 0 {
                    self.g.ent[i].f52 = 1024;
                    self.g.snd(19, i);
                    self.g.ent[i].f44 &= !2;
                }
                // Retail's −80 runs on int before the word store, so
                // an un-re-armed entry (f52 below 80) must floor to 10,
                // NOT wrap the u16 (else debug-panic / release
                // full-blast 1024).
                let f = (self.g.ent[i].f52 as i32 - 80).clamp(10, 1024) as u16;
                self.g.ent[i].f52 = f;
                let away = Gen::angle_between(ex, ey, ctx.px, ctx.py);
                if no_mc2_doom_beam_oneshot() {
                    self.g.player_knock = (away, f as i16);
                } else {
                    // The PRE-DIG route is the `else` arm: retail
                    // writes the player's POSITION here (EF:13444-55,
                    // `NETHERW.EXE` 0x46682 MoveEntity / 0x4668e
                    // moveTest / 0x466dc CopyEntityPosition) and never
                    // touches `moveBoost_0x1E_30`, which the port's
                    // `take_knock_step` would have clamped to ±128 and
                    // decayed −4/tick for twenty ticks past the burst.
                    // Consumed at the carpet's own walk slot — see
                    // `World::step_player_flight_mc2`.
                    self.g.player_hurl = crate::engine::features::PlayerHurl {
                        armed: true,
                        bearing: away,
                        dist: f as i16,
                    };
                }
            }
            sel @ (8 | 9) => {
                // Case 8 = the (9,26) whirlwind seed, case 9 = the
                // (9,3) meteor shot (EF:13457-88): from the shared
                // launch point; owner = the pyramid; aimed at the
                // avatar; impact/damage/fuse armed per case; sound
                // 15 (docs/traces/mc2-class9-m3-m26.md §1).
                let spawned = if sel == 8 {
                    self.g.mc2_spawn_whirlwind_seed(lp.0, lp.1, lp.2)
                } else {
                    self.g.mc2_spawn_meteor_shot(lp.0, lp.1, lp.2)
                };
                if let Some(p) = spawned {
                    {
                        let e = &mut self.g.ent[p];
                        e.f68 = 10;
                        e.f69 = if sel == 8 { 22 } else { 17 };
                        e.f44 = if sel == 8 { 20 } else { 6000 };
                        e.f71 = if sel == 8 { 3 } else { 10 };
                    }
                    self.g.mc2_arm_proj(p, i, PLAYER_TARGET, tpos);
                    self.g.mc2_pyramid_arm_tail(p);
                    self.g.snd(15, i);
                }
            }
            _ => {}
        }
    }

    /// `KillAllCreatures_1B5F0` (EF:8669) — every class-5 creature
    /// dies (model 10 = the pyramid spared; model 27's branch heads
    /// get the action-221 teardown instead).
    fn mc2_kill_all_creatures(&mut self) {
        let mut hints = Vec::new();
        for (j, e) in self.g.ent.iter_mut().enumerate().skip(1) {
            if e.class64 != 5 || e.model65 == 10 {
                continue;
            }
            // ⭐⭐⭐ IT WALKS THE PER-MODEL CHAINS, NOT THE POOL —
            // `for (entity = bytearray_38403x[index]; entity >
            // Entities[0]; entity = entity->next_0)` (EF:8678/8685),
            // twenty-nine chains, and the tick-top sweep that REBUILDS
            // them (EF:40277-93) refuses a class-5 record on four
            // counts: `if (life_0x8 < 0) continue;` and
            // `actionIndex_0x45_69` 0xB4 / 0xE8 / 0xEA. So a creature
            // that is ALREADY DEAD is off every chain and the
            // apocalypse's 70-tick kill-all never touches it again —
            // neither its `life_0x8 = -1` nor its `word_0x24_36`
            // killer stamp.
            //
            // The port walked the whole pool, so it re-stamped every
            // corpse once a tick for seventy ticks. mc2l24 t=52751-53,
            // slots 602 and 603 (two (5,25) minis at action 205, life
            // -4 from their last mailbox): retail holds -4 across the
            // whole apocalypse, the port wrote -1 every tick — and
            // `MGC_WRITE_TRACE=602` names the writer as slot 5, the
            // (5,10) pyramid at action 80, which is this call site
            // (`bits & 4`, doomsday.rs's kill-all countdown).
            if !no_mc2_kill_all_chain_members()
                && (e.act_life < 0 || matches!(e.tick70, 0xB4 | 0xE8 | 0xEA))
            {
                continue;
            }
            if e.model65 == 27 {
                e.tick70 = 221;
                e.f38 = PLAYER_TARGET;
            } else {
                e.act_life = -1;
                e.f38 = PLAYER_TARGET;
                hints.push(j);
            }
        }
        // The victims' next inbox zeroes the latch; the stats ledger
        // keeps retail's PLAYER_TARGET stamp.
        for j in hints {
            self.g.stats.0.hint(j, PLAYER_TARGET);
        }
    }
}

