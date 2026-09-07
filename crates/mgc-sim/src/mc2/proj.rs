//! MC2 class-9 projectile family — the flyer core and the creature
//! attack thunks, ported from remc2 (trace bank:
//! docs/traces/mc2-class9-flyers.md; `EF:` = EventsFunctions.cpp,
//! `EV:` = Events.cpp cites).
//!
//! Field mapping additions over the [`super::mobs`] module doc:
//! `byte_0x43_67` impact class→f68 · `byte_0x44_68` impact model→f69
//! (the MC1 fields mean exactly this — detonation class/model) ·
//! `fov_0x22_34` desired-pitch→f36 · `roll_0x20_32` desired-yaw→f34
//! (as everywhere in the MC2 column) · `subSpellIndex_0x2A_42`
//! carried damage→f44 · `mana_0x90_144`→f140.
//!
//! Deliberate approximations and open items (cited, counted where
//! observable):
//! - The shielded-target ricochet `sub_68740` is ported
//!   ([`Gen::mc2_rebound_deflect`]). ✅ The friendly-shield
//!   homing/detonate pair `sub_68940`/`sub_68AC0` landed with the
//!   (10,78) beacon column ([`Gen::mc2_mine_beacon`] /
//!   [`Gen::mc2_mine_swallow`]) — no longer OPEN. Still open: the
//!   rival-window mirror onto pool entities.
//! - The no-target acquisition `sub_67CB0` (EF:54710, model-keyed
//!   bucket sweeps) serves PLAYER-CAST spells; creature launches
//!   pre-lock `word_0x96_150`. A target-less flyer snapshots its aim
//!   once (the retail else-arm, EF:62914-16) and flies straight.
//! - Water splash spawns (10,5) (EF:62957-63, `mc2_spawn_splash`),
//!   gated inside the terrain-contact branch
//!   (docs/traces/mc2-projectile-terrain-water.md §3).
//! - An impact whose (f68, f69) effect is unported applies its f44
//!   as channel-0 area damage at the impact point (the effect IS the
//!   damage carrier in retail) and counts the misfit (deliberate).
//! - `(9,9)` creator body pending (the subtype 0-0x0C trace); interim
//!   fields marked OPEN below.

use super::behavior::BEHAVIOR;
use crate::engine::features::Gen;
use crate::mc1::combat::MailTarget;
use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};

/// MC2-native projectile marker on [`Ent::flags`] (see
/// [`super::mobs`] for the other high bits). MC1-fallback projectiles
/// spawned on the MC2 column never carry it, so the class-9 dispatch
/// can tell the columns apart without guessing at state numbers.
pub(crate) const F_MC2PROJ: u32 = 1 << 29;
/// byte[0] bit 1 — the flyer's "aim acquired" latch (EF:62904).
pub(crate) const F_AIMED: u32 = 2;

/// A/B toggle for LIGHTNING'S MISSING WORM-CHAIN BRANCH: set
/// `MGC_NO_LIGHTNING_WORM_CHAIN` to restore the pre-dig behaviour,
/// where model 9 ran the worm HEAD + `f54` SEGMENT expansion that
/// `sub_67CB0`'s **case 9** does not have (EF:54912-54927 walks the
/// 29 per-model buckets and stops; the segment branch lives only in
/// the big case's no-candidate fallback EF:54826 and in case 1's
/// unconditional tail EF:55072).
fn no_lightning_worm_chain() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_LIGHTNING_WORM_CHAIN").is_some())
}

/// A/B toggle for THE LIGHTNING TRAIL NODE'S COLLIDABLE BIT: set
/// `MGC_NO_LIGHTNING_NODE_COLLIDE` to restore the pre-dig behaviour,
/// where `mc2_spawn_lightning_node` cleared byte[0] bit 3 like the
/// real class-9 bolt ctors do. `sub_66750` (EF:58336-45) writes NO
/// flag word at all — six fields and nothing else — so the node keeps
/// `NewEvent_4A050`'s default `dword = 8` (Events.cpp:567) plus
/// `AddEventToMap_57D70`'s bit 2 (the recorded `flags 12`) and IS a
/// legal `sub_10780` victim (EF:3765 tests `byte[0] & 8`). Every real
/// class-9 bolt ctor spells `byte[0] &= 0xF7` (`sub_4D860` EF:34952
/// &c.); the trail node does not. THE ASYMMETRY IS RETAIL'S.
pub(crate) fn no_lightning_node_collide() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_LIGHTNING_NODE_COLLIDE").is_some())
}

/// A/B toggle for `sub_66750`'s REBOUND PAYLOAD ABSORB: set
/// `MGC_NO_LIGHTNING_REBOUND_ABSORB` to restore the pre-dig
/// behaviour, where the lightning blast carried the beam's whole
/// `subSpellIndex` onto a rebounding wizard.
fn no_lightning_rebound_absorb() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_LIGHTNING_REBOUND_ABSORB").is_some())
}

/// A/B toggle for the chord march's muzzle admission (OPEN-7): set
/// `MGC_NO_MUZZLE_ADMISSION` to restore the pre-dig behaviour, where
/// every sub-step from the muzzle out could detonate the shot.
fn no_muzzle_admission() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MUZZLE_ADMISSION").is_some())
}

/// A/B toggle for `sub_1D460`'s UNMASKED FAN YAW (EF:9962 — see the
/// write-up at [`Gen::mc2_atk_fan`]): set `MGC_NO_MC2_FAN_YAW_UNMASKED`
/// to restore the pre-dig `& 0x7FF`, which retail's raw `int16_t` store
/// does not perform and which is wrong on the ±226 wings of every fan.
fn no_mc2_fan_yaw_unmasked() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_FAN_YAW_UNMASKED").is_some())
}

/// A/B toggle for `sub_1D260`'s MUZZLE-ORIGIN AIM: set
/// `MGC_NO_MC2_HEAVY9_MUZZLE_AIM` to restore the pre-dig behaviour,
/// where m23's (9,9) heavy bolt was aimed from the caster's RAW z
/// like its six siblings instead of from the fov-lifted muzzle it is
/// actually spawned at (EF:9887-9899; `NETHERW.EXE` 0x41A80-0x41AE4).
fn no_heavy9_muzzle_aim() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_HEAVY9_MUZZLE_AIM").is_some())
}

/// A/B toggle for `sub_68AC0`, the Magic Mine's SPELL SWALLOW (and
/// the collide bit `sub_50840` gives the mine so the victim probe can
/// see it at all): set `MGC_NO_MINE_SWALLOW` to restore the
/// pre-dig behaviour, where a wizard's own bolt flew straight through
/// his mine and detonated normally.
pub(crate) fn no_mine_swallow() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_SWALLOW").is_some())
}

/// A/B toggle for `sub_68AC0`'s TOKEN RE-ARM (`v5x->word_0x2E_46 = 1`,
/// EF:55444): set `MGC_NO_MINE_TOKEN_REARM` to restore the pre-dig
/// behaviour, where a swallowed bolt left the caster's (15,x) window
/// running out on its own schedule.
pub(crate) fn no_mine_token_rearm() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_TOKEN_REARM").is_some())
}

/// A/B toggle for `sub_674C0`'s claim-victim gate: set
/// `MGC_NO_A18_VICTIM_GATE` to restore the pre-dig behaviour, where an
/// action-18 (leveled possession) bolt minted its impact payload on a
/// victimless expiry.
fn no_a18_victim_gate() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_A18_VICTIM_GATE").is_some())
}

// A/B toggle for the action-30 MAGIC-MINE CARRIER shape (`sub_67960`,
// EF:59240) — the same `MGC_NO_MC2_MINE` switch that reverts the
// (10,78) ctor/arm half in `mc2/effects.rs`: set it to restore the
// pre-dig behaviour, where the (9,29) carrier ran the generic
// `sub_65820` tail — life decremented only on a contactless tick,
// and the impact tail stamped the mine's bearing words.
use super::effects::no_mc2_mine as no_mine_carrier;
use super::mobs::no_mine_beacon;

/// A/B toggle for the action-18 ceiling pre-clamp removal
/// (`sub_674C0` EF:58993-96 has no cave arm before its commit, unlike
/// `CastPosses_65F60` EF:63265-70): set `MGC_NO_A18_CEILING_PRECLAMP`
/// to restore the pre-dig behaviour, where BOTH possession workers
/// pre-clamped to the cave ceiling.
fn no_a18_ceiling_preclamp() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_A18_CEILING_PRECLAMP").is_some())
}

/// A/B toggle for `sub_65C20`'s REFUSED-DEFLECTION NO-OP: set
/// `MGC_NO_MC2_SHIELD_SWALLOW` to restore the pre-dig behaviour, where
/// a fireball-family flyer whose shielded victim could not afford the
/// rebound fell through to the STRIKE arm instead of passing through.
fn no_mc2_shield_swallow() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_SHIELD_SWALLOW").is_some())
}

/// A/B toggle for `sub_65C20`'s STRIKE-ARM PAYLOAD CLOBBER: set
/// `MGC_NO_MC2_STRIKE_SUBSPELL_1` to restore the pre-dig behaviour,
/// where a fireball-family flyer carried its full `subSpellIndex`
/// onto the impact effect even when the struck victim's `str_D7BD6`
/// row is flagged `byte_160_0x20_32 & 0x10`.
fn no_mc2_strike_subspell_1() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_STRIKE_SUBSPELL_1").is_some())
}

/// ⭐⭐⭐ RETAIL'S TWO **UNGUARDED** `word_0x96_150` STAMPS — the
/// LIGHTNING pair. `sub_65820`'s generic impact tail writes the
/// effect's leader as `if (v5x) v11x->word_0x96_150 = v5x - base;`
/// (EF:62991-92; shipped `NETHERW.EXE` file **0x8a2cc** `test %esi,%esi`
/// / `0x8a2ce je 0x8a2f1` / `0x8a2ea mov %ax,0x96(%edi)`) — a terrain
/// hit leaves `NewEvent_4A050`'s memset zero. **`sub_66750` (action 9,
/// the lightning bolt's impact → the (10,23) blast) and `sub_66FD0`
/// (action 12, the Lightning L1/L2 carrier → the (10,38) storm cloud)
/// have NO SUCH TEST**: both divide first and test for NULL afterwards.
///
/// ```text
/// sub_66750, NETHERW.EXE 0x8b26f:          sub_66FD0, NETHERW.EXE 0x8ba7d:
///   mov  eax,ds:0x41a0                       mov  eax,ds:0x41a0
///   mov  edx,esi        ; the victim         mov  edx,edi        ; the victim
///   add  eax,0x6e8e     ; Entities base      add  eax,0x6e8e
///   sub  edx,eax                             sub  edx,eax
///   mov  ecx,0xa8       ; sizeof(record)     mov  edi,0xa8
///   sar  edx,0x1f / idiv ecx                 sar  edx,0x1f / idiv edi
///   mov  WORD PTR [edi+0x96],ax  <-- STORE   mov  WORD PTR [esi+0x96],ax
///   test esi,esi        ; ...only NOW        (no NULL test at all)
/// ```
/// With no victim the quotient is `-(EntitiesBase / 168)`, truncated to
/// 16 bits — a HEAP-ADDRESS ARTIFACT, constant for the shipped DOS
/// build. **MEASURED EXHAUSTIVELY on the whole mc2l22 take
/// (`MGC_RAW_SHADOW_ALL=1 verify-deltas`, 65,556 pairs): turning this
/// on takes `(10,38) target96` from 44 rows to ZERO and `(10,23)
/// target96` from 7,159 rows to 27** — i.e. 7,176 of 7,203 rows in the
/// two families were this one constant, and cross-take on mc2l30 the
/// value is the same 44546.
///
/// ⭐⭐ AND THE 27 SURVIVORS ARE A REAL DEBT THE GHOST WAS BURYING:
/// `t=7439 slot 563: retail 228 / port 242` and 26 more like it, each
/// on its own slot — a genuine (10,23) leader divergence that was
/// invisible while 7,159 artifact rows sat on top of it. Whoever takes
/// it wants `sub_10780`'s victim pick, not this stamp.
/// 44546 = 0xAE02 ⇒ `EntitiesBase / 168 == 0x51FE` ⇒ base ≈ 0x35CFB0,
/// a plausible DOS4GW flat address. remc2 hand-patched the same
/// constant into both sites (`EventsFunctions.cpp:58418` and `:58832`,
/// the latter tagged `//fix`) and into `sub_1D700`'s reader
/// (`:10050 if (v4 == 0xae02) return;`) — see `docs/DEVIATIONS.md`
/// "MC2 RETAIL READS OUT OF BOUNDS".
///
/// ⚠ **OPT-IN, DEFAULT OFF** — the value is not derivable from the
/// shipped assets, which is the campaign's REGISTERED-DEVIATION class
/// (same footing as `MGC_MC2_IVT_GHOST`). It is also INERT on this
/// corpus: nothing in retail or the port reads a (10,23) or (10,38)
/// `@0x96` (`sub_33D80` EF:24787 and `sub_10C80` EF:3953-4155 never
/// touch it), so turning it on only makes the ungraded-lane census
/// honest. Set `MGC_MC2_NULL_VICTIM_GHOST=1` to reproduce it.
/// A PLAYER RULING IS OWED on whether it should be the default.
pub(crate) fn mc2_null_victim_ghost() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_MC2_NULL_VICTIM_GHOST").is_some())
}

/// The observed `-(EntitiesBase / 168) & 0xFFFF` of the shipped DOS
/// build (see [`mc2_null_victim_ghost`]).
pub(crate) const MC2_NULL_VICTIM_GHOST: u16 = 44546;

/// A/B toggle for `sub_65C20`'s MISS-CLEARS-THE-LOCK tail: set
/// `MGC_NO_FIREBALL_LOCK_CLEAR` to restore the pre-dig behaviour,
/// where the action-29 firestorm hub inherited the flyer's stale
/// homing lock even when the flyer struck NOTHING. See the read site
/// in [`Gen::mc2_proj_impact`] for the citations.
/// A/B toggle for `sub_67CB0` `case 0x19`'s `sub_3A7F0` FILTER: set
/// `MGC_NO_MC2_AIM_CHARM_FILTER` to restore the pre-dig behaviour,
/// where the (9,25) alliance carrier's acquisition screened creatures
/// by an invented "on the ground" test instead of the charm-
/// eligibility predicate retail actually calls (EF:54991) — so it
/// could lock onto a creature the caster had already charmed.
fn no_mc2_aim_charm_filter() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_AIM_CHARM_FILTER").is_some())
}

fn no_fireball_lock_clear() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_FIREBALL_LOCK_CLEAR").is_some())
}

/// A/B toggle for the debuff stamp's knock: set
/// `MGC_NO_STALE_DEBUFF_KNOCK` to restore the pre-dig behaviour,
/// where the kick INVENTED a heading (`pyaw + half turn`) and a
/// POSITIVE magnitude instead of writing retail's signed
/// `moveBoost_0x1E_30 = -80` onto the STALE `yaw_0x1E_30`.
fn no_stale_debuff_knock() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_STALE_DEBUFF_KNOCK").is_some())
}

/// The virtual projectile the acquisition scan scores from — either
/// a live flyer on its first tick (`mc2_autoaim`) or the crosshair
/// instrument's would-be launch (`World::mc2_aim_preview`).
pub(crate) struct AimProbe {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: u16,
    pub pitch: u16,
    /// The PROJECTILE model — keys the candidate lists.
    pub model: u8,
    pub own: u16,
    /// ⭐⭐ THE CLASS-3 LOCK RANGE IS THE **OWNER'S** BEHAVIOUR ROW,
    /// NOT THE PROJECTILE'S — `v43x = Entities[a1x->id_0x1A_26]`
    /// hoisted before the walk, then
    /// `v10 = v43x->dword_0xA0_160x->word_160_0x1c_28` inside it
    /// (EF:54783-88, and the same hoist in the sibling cases at
    /// EF:54866/54941). Every class-9 row carries 4096; the wizard
    /// ctors carry 8192 (`AddPlayer_4A920` → `str_D7BD6[66]`, cave
    /// 104; `sub_4A9C0` → `[67]`), so a WIZARD-cast bolt reaches a
    /// full 4096 further for its lock than the port's projectile-row
    /// reading allowed. See [`Gen::mc2_owner_lock_range`].
    pub range: i64,
    /// Lightning's wizard range = minSpeed · maxLife (EF:54896);
    /// unused for every other model.
    pub reach: i64,
}

impl Gen {
    // ---- class-9 creators ---------------------------------------------------

    /// `SummonFireball_4D2E0` (EF:34729) — the (9,0) bolt every
    /// creature ranged attack resolves into: action 0, speed 384,
    /// life 0x2000/384 = 21, mana 50, row 64, sprite 340. (The
    /// trailing `AddEvent2_847D0` dynamic light is presentation.)
    pub(crate) fn mc2_spawn_bolt(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 0;
            e.tick70 = 0;
            e.f126 = 384;
            e.f128 = 384;
            e.f140 = 50;
            e.max_life = (0x2000 / 384) as u32; // 21
            e.row156 = 64;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 340);
        Some(i)
    }

    /// `sub_4D500` (EF:34810) — the (9,3) METEOR SHOT (spell 9's
    /// projectile; the doomsday pyramid's case-9 summon): action 3,
    /// speed 384, life 21, mana 50, row 60 (yaw/pitch caps 22),
    /// sprite 76, untargetable. The launcher arms impact/damage/fuse
    /// (docs/traces/mc2-class9-m3-m26.md §1). No RNG in the ctor.
    pub(crate) fn mc2_spawn_meteor_shot(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 3;
            e.tick70 = 3;
            e.f126 = 384;
            e.f128 = 384;
            e.f140 = 50;
            e.max_life = (0x2000 / 384) as u32; // 21
            e.row156 = 60;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 76);
        Some(i)
    }

    /// `sub_4E180` (EF:35266) — the (9,26) WHIRLWIND SEED (spell 21's
    /// projectile; the pyramid's case-8 summon): the meteor-shot
    /// numbers under action 27 with sprite 320. Its impact
    /// owner-lock-clear (`sub_67890` EF:59181 — a player-avatar
    /// homing-lock release) only fires on the player-cast path; for
    /// the pyramid owner it is retail's own no-op (same doc §3.2).
    pub(crate) fn mc2_spawn_whirlwind_seed(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.mc2_spawn_meteor_shot(x, y, z)?;
        self.ent[i].model65 = 26;
        self.ent[i].tick70 = 27;
        self.mc2_set_sprite(i, 320);
        Some(i)
    }

    /// `sub_66180` (EF:63340, action 3) — the meteor shot's wrapper
    /// around the flyer core: every tick lay one damage-suppressed
    /// (10,0) spark (dword |= 0x10080) at a ±64-box jitter centered
    /// 96 units toward −x/−y of the shot (`rand%0x81 + pos − 160`
    /// per axis, EF:63356-59; 2 draws of its own stream), life 4,
    /// frame 3, yaw inherited. Retail lays it even on the impact tick
    /// (the class stays set until the removal pass). The fuse stamp
    /// onto the impact entity (`v1x->maxLife/life = byte_0x46_70`) is
    /// IDENTITY for the pyramid values (fuse 10 = the meteor ctor's
    /// maxLife 10); the charge-tiered player-cast fuse is separate.
    pub(crate) fn mc2_meteor_shot_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.mc2_flyer_tick(i, ctx);
        let (x, y, z, id, yaw) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.id24, e.f30)
        };
        let jx = (self.ent_rand(i) % 0x81) as u16;
        let jy = (self.ent_rand(i) % 0x81) as u16;
        let sx = x.wrapping_add(jx).wrapping_sub(160);
        let sy = y.wrapping_add(jy).wrapping_sub(160);
        if let Some(s) = self.mc2_spawn_fire(sx, sy, z) {
            let e = &mut self.ent[s];
            e.flags |= 0x10080;
            e.id24 = id;
            e.act_life = 4;
            e.frame88 = 3;
            e.f30 = yaw;
        }
    }

    /// `sub_4DC40` (EF:35071) — the (9,20) lob: action 21, speed 394,
    /// life 7680/394 = 19, sprite 196, NO behavior row (the launcher
    /// sets row 65).
    pub(crate) fn mc2_spawn_lob20(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 20;
            e.tick70 = 21;
            e.f126 = 394;
            e.f128 = 394;
            e.max_life = (7680 / 394) as u32; // 19
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 196);
        Some(i)
    }

    /// `sub_4DCC0` (EF:35091) — the (9,21) arc: action 22, speed 394,
    /// life 19, sprite 319, ShiftRot(256, 512).
    pub(crate) fn mc2_spawn_lob21(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 21;
            e.tick70 = 22;
            e.f126 = 394;
            e.f128 = 394;
            e.max_life = (7680 / 394) as u32;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 319);
        self.mc2_shift_rot(i, 256, 512);
        Some(i)
    }

    /// `sub_4D860` (EF:34942) — the (9,9) bolt (m23's `sub_1D260`
    /// payload, also the player thunder family): action 9, speed 384,
    /// life 3584/384 = 9, mana 50, row 63, sprite 216. (The trailing
    /// `AddEvent2_847D0` sub-effect is presentation.)
    pub(crate) fn mc2_spawn_bolt9(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 9;
            e.tick70 = 9;
            e.f126 = 384;
            e.f128 = 384;
            e.f140 = 50;
            e.max_life = (3584 / 384) as u32; // 9
            e.row156 = 63;
            e.flags = (e.flags & !8) | F_MC2PROJ;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 216);
        Some(i)
    }

    // ---- the shared flyer flight (sub_65820, EF:62882) ----------------------

    /// `sub_68740` (EF:55221-310) — the REBOUND deflection engine,
    /// gated at every projectile mover's victim-hit site (EF:62939
    /// generic flyer, 58892 archer arrow, 58770 lightning carrier,
    /// 63484/63162 variants). A victim with a live Rebound window
    /// throws the projectile back at its shooter. Cost gate first
    /// (`proj.mana/4 > victim.mana` → it hits normally); then the
    /// impact-pair whitelist: class 10 with subtype ∈ {0,1,9,11,15,
    /// 17,22,67,71,89} (the 0x44-0x46 range FAILS, EF:55247-53), OR
    /// model-13 arrows unconditionally.
    /// On deflect: sound 28, Rebound XP to the deflector
    /// (`sub_6D8B0(victim, 8, 1)` EF:55283), victim mana −quarter,
    /// heading reversed (`f34 = f30 + 0x400`, pitch negated). The
    /// PRECISE tier (T3, `mc2_rebound_precise`) returns it EXACTLY
    /// down the reverse ray with a DOUBLED payload; the scatter fan is
    /// the CALL SITE's `(a3, a4)` window (see `window` below). The
    /// bolt re-owns to the victim, re-homes on the old shooter (f146),
    /// re-keys its collide pair off that shooter, refills life,
    /// relinks at the victim's RAISED position, and flies on.
    ///
    /// ⭐⭐⭐ THE SCATTER WINDOW IS A PARAMETER, NOT A CONSTANT.
    /// `sub_68740`'s last two arguments are the modulus and the
    /// half-width of `yaw = roll + rand % a3 − a4`, and retail's five
    /// call sites do NOT agree: `sub_65820` (the shared flyer core,
    /// EF:62939), `AddArcherArrow_672E0` (58892), `sub_66FD0` (58770)
    /// and `sub_662E0` (63484) all pass `(0x2D, 22)`, but **`sub_65C20`
    /// — the state-0 fireball body, and so also state 29, which opens
    /// by calling it — passes `(0x5B, 45)`** at EF:63162. The port
    /// hardcoded 0x2D/22 everywhere and its own doc comment asserted
    /// that as "MC2's own window — NOT MC1's 0x5B/45", which is
    /// exactly backwards for the one body that actually flies a plain
    /// fireball. mc2l6-rsg t=4567: rival 378's (9,0) strikes the
    /// human's Rebound with `roll` 751 and `rand` 59063, and retail
    /// turns to **710** (59063 % 91 = 4, −45) where the narrow window
    /// gives 752 (59063 % 45 = 23, −22).
    /// ⭐ A LAW LANDED ON ONE CALL PATH IS NOT LANDED.
    ///
    /// ⭐⭐ THE VICTIM'S MANA IS DEBITED WHOEVER THE VICTIM IS.
    /// EF:55232 gates on `proj.mana/4 > a2x->mana` and EF:55284
    /// subtracts the same quarter — `a2x` is an entity pointer and
    /// retail has no player test on either statement. The human's
    /// purse lives outside our pool, so the quarter rides the
    /// [`Gen::player_deflect_debit`] seat the MC1 column has used
    /// since its own deflect law; the gate reads `ctx.pmana` less
    /// what this tick's earlier deflections already owe, exactly as
    /// `mc1::combat` does. mc2l6-rsg t=4567 measures the debit
    /// directly: the (9,0) carries mana 20, and the human's purse
    /// steps 1433596 → 1433591.
    ///
    /// ⭐⭐ THE LANDING IS THE VICTIM'S POSITION PLUS ITS `.fov`, NOT
    /// ITS FEET. EF:55302-04 copies `a2x->position` with
    /// `z += a2x->array_0x52_82.fov` — the FOURTH word of the extent
    /// quad (`axis_4d {yaw, pitch, roll, fov}`), not the `.yaw`
    /// first word that `sub_65580`'s impact lift uses. Same tick:
    /// the human sits at z 652 with `afov` 100 and the deflected
    /// bolt parks at 752.
    ///
    /// ⭐ THE PITCH FLIP *IS* A NEGATE — CLOSED. EF:55292-96 spells it
    /// `-(sub_582F0(0, pitch) * sub_582B0(0, pitch))` with `BYTE1 &= 7`,
    /// and the two helpers (Sound.cpp:6569/6580) are the SIGN and the
    /// MAGNITUDE of the shortest turn from 0 — so their product is
    /// `pitch` folded into (−1024, 1024] and the store is
    /// `(−pitch) & 0x7FF` for every input, including out-of-range ones
    /// (negation commutes with the mask). mc2l6-rsg t=4847 is the
    /// witness that closed it: the earlier "53 → 1984, a negate gives
    /// 1995" reading had sampled the pitch BEFORE `sub_65610`'s steer
    /// leg, one statement up the same function — here the bolt enters
    /// on 73, steers to 85, and retail stores 1963 = −85.
    ///
    /// ⚠ HALF OF THIS NOTE WAS STALE (round 99, DIG 99-6). "RIVAL
    /// windows are not yet mirrored onto their entities" has not been
    /// true since the rival-buff-bits landing: `import_ent_mc2`
    /// translates `byte[1] & 0x80` into
    /// [`crate::mc2::rivals::F_REBOUND`] and
    /// `Gen::mc2_rival_publish_buff` stamps it every afforded tick, so
    /// the POOL arm below is a live, load-bearing gate — mc2l22
    /// t=3742's wall was a `(9,13)` arrow deflecting off a rival's
    /// stale window. What actually kept that bit alive was the
    /// TEARDOWN, not the mirror: see
    /// `crate::mc2::rivals::rival_corpse_token_off`.
    ///
    /// OPEN, and STILL open: the pool arm tests `0x8000` where every
    /// call site tests `word[0] & 0x8010` (EF:58892 / 62939), and it
    /// hardcodes `precise = false` where retail reads
    /// `a2x->byte[0] & 0x10` (EF:55297). That second bit is the
    /// REBOUND tier-1 PRECISE window, and in this tree it has NO
    /// WRITER AT ALL on the pool column — `import_ent_mc2` never
    /// carries `byte[0] & 0x10` (it is the one low byte-0 bit the
    /// translation table skips) and `mc2_rival_publish_buff`'s
    /// `tier == 1` case is an empty `_ => {}` against retail's
    /// `sub_6AA00` `orb $0x10,0xc(%esi)` (NETHERW.EXE 0x8F276). So
    /// widening the gate here is INERT until those two writers exist;
    /// it is a SIXTH-class import-seat hole plus a missing publish,
    /// not a proj.rs law. Unwitnessed as of round 99.
    /// 🏦 Neither possession worker (actions 1/18) is on `sub_68740`'s
    /// call-site list at all: those two take no rebound gate.
    pub(crate) fn mc2_rebound_deflect(
        &mut self,
        i: usize,
        hit: MailTarget,
        ctx: &MobCtx,
        window: (u32, i32),
    ) -> bool {
        // The victim's live-window test (retail `word[0] & 0x8010`).
        let (active, precise) = match hit {
            MailTarget::Player => (self.player_rebound, self.mc2_rebound_precise.0 != 0),
            MailTarget::Pool(j) => (self.ent[j].flags & 0x8000 != 0, false),
        };
        if !active {
            return false;
        }
        // Whitelist (EF:55232-53).
        let (fc, fm, model) = {
            let e = &self.ent[i];
            (e.f68, e.f69, e.model65)
        };
        if !(model == 13
            || (fc == 10 && matches!(fm, 0 | 1 | 9 | 11 | 15 | 17 | 22 | 67 | 71 | 89)))
        {
            return false;
        }
        // Cost gate + debit (EF:55232 / 55284) — the victim pays,
        // whoever the victim is.
        let quarter = (self.ent[i].f140 / 4).max(0);
        match hit {
            MailTarget::Pool(j) => {
                if quarter > self.ent[j].f140 {
                    return false;
                }
                self.ent[j].f140 -= quarter;
            }
            MailTarget::Player => {
                let owed = quarter.max(0) as u32;
                if owed > ctx.pmana.saturating_sub(self.player_deflect_debit.0) {
                    return false;
                }
                self.player_deflect_debit.0 += owed;
            }
        }
        self.snd(28, i); // the deflection twang
        let deflector = match hit {
            MailTarget::Player => PLAYER_TARGET,
            MailTarget::Pool(j) => self.ent[j].id24,
        };
        if deflector == PLAYER_TARGET {
            self.mc2_cast_xp.0.push((PLAYER_TARGET, 8, 1));
        }
        let shooter = self.ent[i].id24;
        let (modulus, half) = window;
        {
            let e = &mut self.ent[i];
            e.f34 = e.f30.wrapping_add(0x400) & 0x7FF;
            // The pitch flip writes the EXTENT QUAD's `.fov` too
            // (EF:55292-96 assigns `v9` to `fov_0x22_34` and then to
            // `pitch_0x1E_30`); the port only ever wrote the pitch.
            e.f32 = e.f32.wrapping_neg() & 0x7FF;
            e.f36 = e.f32;
        }
        // ⭐ THE SCATTER DRAW LIVES INSIDE THE SCATTER ARM. EF:55307-09
        // is `else { a1x->rand_0x14_20 = 9377 * rand + 9439; yaw = roll
        // + rand % a3 - a4; }` — the PRECISE tier (EF:55304-06) takes
        // `yaw = roll` and doubles `subSpellIndex_0x2A_42` with NO LCG
        // step at all. The port drew unconditionally, so every precise
        // deflection advanced the bolt's stream one draw too far:
        // mc2l6-rsg t=5564 slot 856 is a T3 return (`f2a` 250 -> 500,
        // `yaw` = `roll` = 1415) whose `rand` retail holds at 45665 and
        // the port stepped to 63456 — the pair's ONLY field diff.
        {
            let e = &mut self.ent[i];
            if precise {
                e.f30 = e.f34;
                e.f44 = e.f44.saturating_mul(2);
            }
        }
        if !precise {
            let d = self.ent_rand(i);
            let e = &mut self.ent[i];
            // Raw store — EF:55299 has no mask, like the MC1 twin.
            e.f30 = (e.f34 as i32 + (d % modulus) as i32 - half) as u16;
        }
        {
            let e = &mut self.ent[i];
            e.f146 = shooter; // re-home on the old shooter
            e.id24 = deflector;
            e.act_life = e.max_life as i32;
        }
        // The collide pair re-keys off the OLD SHOOTER's record
        // (EF:55298-300 `Entities[word_0x96_150]`, no player test) —
        // the returned bolt now hunts the kind that fired it.
        // mc2l6-rsg t=4567: 378 is a (3,1), and retail stamps 3/1.
        let (sx, sy) = if shooter == PLAYER_TARGET {
            (3, 0) // the human carpet's class/model
        } else {
            let e = &self.ent[shooter as usize];
            (e.class64, e.model65)
        };
        {
            let e = &mut self.ent[i];
            e.f66 = sx;
            e.f67 = sy;
        }
        // The landing is the victim's position raised by its extent
        // quad's `.fov` (EF:55302-04) — the human's is 100, the same
        // as its `.yaw` half-height.
        match hit {
            MailTarget::Pool(j) => {
                let (jx, jy, jz, jf) = {
                    let e = &self.ent[j];
                    (e.x, e.y, e.z, e.f84)
                };
                self.move_relink(i, jx, jy, jz.wrapping_add(jf as i16));
            }
            MailTarget::Player => self.move_relink(
                i,
                ctx.px,
                ctx.py,
                ctx.pz.wrapping_add(crate::mc1::combat::PLAYER_HH as i16),
            ),
        }
        true
    }

    /// Class filter of the victim probe `sub_10780` (EF:3766-69):
    /// `xtype == -1` admits anything, else class must match and
    /// `xsubtype == -1` or model must match. The human counts as
    /// class 3 model 0.
    pub(crate) fn mc2_proj_filter(&self, i: usize, hit: Option<MailTarget>) -> Option<MailTarget> {
        let (fc, fm) = (self.ent[i].f66, self.ent[i].f67);
        if fc == 0xFF {
            return hit;
        }
        match hit {
            Some(MailTarget::Pool(v)) => {
                let e = &self.ent[v];
                (e.class64 == fc && (fm == 0xFF || e.model65 == fm))
                    .then_some(hit)
                    .flatten()
            }
            Some(MailTarget::Player) => (fc == 3 && (fm == 0xFF || fm == 0))
                .then_some(hit)
                .flatten(),
            None => None,
        }
    }

    /// `sub_50780` (EF:36912) — the (10,65) STAGGER stamp ctor:
    /// action 0x46 = 70, byte[0] = (&0xF6)|1, position only — no
    /// life override, not map-linked, no extents, no sprite, no RNG.
    /// A one-tick carrier the projectile impact seam aims at its
    /// victim (the flyer copy hands it `word_0x96_150` → f146).
    pub(crate) fn mc2_spawn_stagger(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        let e = &mut self.ent[i];
        e.class64 = 10;
        e.model65 = 65;
        e.tick70 = 70;
        e.flags = (e.flags & !0x9) | 1;
        e.x = x;
        e.y = y;
        e.z = z;
        Some(i)
    }

    /// `sub_507C0` (EF:36928) — the (10,66) PARALYZE stamp: the
    /// stagger ctor + subSpell 200 (the mail its tick delivers).
    pub(crate) fn mc2_spawn_paralyze(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.mc2_spawn_stagger(x, y, z)?;
        self.ent[i].model65 = 66;
        self.ent[i].tick70 = 71;
        self.ent[i].f140 = 200;
        Some(i)
    }

    /// `sub_38E70` (EF:28400, action 0x46) / `sub_38F70` (EF:28424,
    /// action 0x47) — the one-tick wizard-debuff stamps: if the
    /// carried victim (`word_0x96_150` → f146) is a wizard body
    /// (class-3 model-0 / the human player), kick it BACKWARD
    /// (`moveBoost_0x1E_30 = -80`) with the 54..57 grunt
    /// (`54 + rand&3` on the stamp's own stream); the paralyze
    /// variant additionally mails its subSpell (200, `sub_11900`)
    /// and arms the mobilize stun. Then self-despawn.
    ///
    /// The flight-struct channels have ported homes:
    /// `moveSpeed_0x14C_332` 0..3 stagger ramp (65) and
    /// the `mobilizeCounter_0x14E_334` stun latch (66) queue through
    /// `Gen::mc2_debuffs` into the boundary's
    /// `flight::Mc2Ext`; the kick rides `player_knock` (backward =
    /// pyaw + half turn — the retail `moveBoost = −80`, EF:28411/
    /// 28437, both variants). The `SetPaletteModification_5C830`
    /// GREEN tint (subMod 3, EF:31935-32002: R and B darkened by
    /// `56*count>>8`, green untouched — NOT the subMod-2 red damage
    /// flash) is presentation — the app reads the slow level off
    /// the ext. Rival bodies take the mail + grunt
    /// (their brain owns their movement — no positional kick
    /// channel).
    pub(crate) fn mc2_debuff_stamp_tick(&mut self, i: usize, ctx: &MobCtx) {
        let (victim, id, amt) = {
            let e = &self.ent[i];
            (e.f146, e.id24, e.f140 as u32)
        };
        let paralyze = self.ent[i].tick70 == 71;
        if victim == PLAYER_TARGET {
            // ⭐⭐⭐ THE GRUNT DRAW, THE BACKWARD KICK AND THE STUN
            // MAIL ALL LIVE INSIDE THE "NOT ALREADY PARALYZED" ARM.
            // `sub_38F70` EF:28434-43 verbatim:
            //     if (!v1x->dword_0xA4_164x->mobilizeCounter_0x14E_334)
            //     {
            //         v1x->dword_0xA4_164x->moveBoost_0x1E_30 = -80;
            //         a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
            //         PrepareEventSound_6E450(a1x->word_0x96_150, -1,
            //                                 (a1x->rand_0x14_20 & 3) + 54);
            //         sub_11900(a1x, v1x, 0, a1x->subSpellIndex_0x2A_42);
            //     }
            //     v1x->dword_0xA4_164x->mobilizeCounter_0x14E_334 = 1;
            //     v1x->dword_0xA4_164x->mobilizeCounter2_0x150_336 = 10;
            // Only the two latch writes are unconditional. A second
            // stamp landing on an ALREADY-stunned wizard re-arms the
            // latch and does NOTHING else — no LCG step, no kick, no
            // damage mail. [`Gen::mc2_mobilize`] is this file's own
            // pool-side mirror of `mobilizeCounter_0x14E_334` (the
            // carpet dispatch republishes it every tick), so it is
            // the exact operand retail reads at EF:28434.
            //
            // mc2l22 t=375→376 is the corpus witness: the (10,66) at
            // slot 481 (`action45` 0x47, `target96` 424 = the human)
            // ends the take's longest early clean run with
            // `rand: retail 55670 port 32789`, and
            // 32789 == (9377*55670 + 9439) mod 2^16 — the port took
            // EXACTLY ONE draw where retail took none. Slot 481
            // repeats it at t=377→378 (retail 51603 / port 38482).
            let already_stunned = paralyze && self.mc2_mobilize.0 != 0;
            if !already_stunned {
                let grunt = 54 + (self.ent_rand(i) & 3) as u8;
                self.snd_player(grunt);
                // ⭐⭐⭐ AN INVENTED WRITE: retail writes ONLY the
                // MAGNITUDE, and it writes it NEGATIVE —
                // `moveBoost_0x1E_30 = -80` (EF:28411 / EF:28437).
                // The heading `yaw_0x1E_30` (str_164 +32) is NOT
                // touched, so the kick reuses whatever bearing the
                // last buffet left in the register, and the negative
                // magnitude sends the carpet 180 degrees off it:
                // `MoveEntity_57FA0(&predicted, ext->yaw_0x1E_30, 0,
                // ext->moveBoost_0x1E_30)` (EF:59698) takes the sign
                // straight into the polar step. The port used to
                // fabricate BOTH halves — heading `pyaw + 0x400`,
                // magnitude `+80` — which happens to look the same
                // only when the stale bearing already equals the
                // carpet's own yaw.
                //
                // It hid for two sessions because the paralyze stamp
                // ALSO mails damage, and the damage buffet
                // (`player_knock = (dir, amt/10)`) overwrites both
                // halves before the mover ever consumes them. The -80
                // only survives when the mail does NO damage: mc2l22
                // t=411->412 is the corpus witness (the human's +345
                // latch has stood at 1 since t=399, the 780 ch0 mail
                // takes no life and regen keeps paying +40/tick, and
                // `moveBoost` stays -80 decaying +4 through t=420) —
                // retail steps (+66,+45) = polar(1729, -80) off the
                // bearing a buffet left there at t=393, while the port
                // stepped (-77,+25) = polar(1434, +80). The same stamp
                // at t=336 and t=374 DID damage, so the buffet
                // (`amt/10` = 78 on a fresh heading) overwrote the -80
                // before the mover ran and both columns agreed.
                if no_stale_debuff_knock() {
                    self.player_knock = ((ctx.pyaw.wrapping_add(1024)) & 0x7FF, 80);
                } else {
                    self.player_knock.1 = -80;
                }
            }
            if paralyze {
                self.mc2_debuffs.stun = self.mc2_debuffs.stun.saturating_add(1);
                // ⭐⭐⭐ …AND THE LATCH IS VISIBLE IMMEDIATELY. Retail
                // has ONE storage: `sub_38F70` writes
                // `mobilizeCounter_0x14E_334` straight onto the
                // wizard's `str_164` here (EF:28442-43), so every
                // record dispatching after this stamp — this tick and
                // the whole of the next, up to the carpet's own
                // dispatch — reads the latch set. The port has TWO
                // (the flight ext, and this pool-side mirror the
                // creature brains read), and the queue above only
                // reaches the ext at the NEXT carpet dispatch, which
                // is a walk pass too late for everything below the
                // carpet's slot. Stamping the mirror here is what
                // makes the two storages behave like retail's one.
                //
                // mc2l3 t=10297: the m20's own (9,21) lob lands and
                // its (10,66) stamp at slot 175 paralyzes the human;
                // retail's m20 at slot 1 reads the latch at 10298 and
                // commits its melee rush (`byte_0x46_70` 0 → 1,
                // speed 32 → 64 at 10299). The queued-only port read
                // 0, committed at 10299 and doubled at 10300 — the
                // whole chase column one tick late.
                //
                // ⚠ The counter/decay stay the ext's: the drain
                // re-arms `mobilize_ctr = 10` at the next carpet
                // dispatch and `mc2_move` step 8 decays it in the
                // same pass, which is retail's own phase (stamp at T,
                // first decay at T+1). Only the READ window moves.
                self.mc2_mobilize.0 = 1;
                // `sub_38F70`'s write is `sub_11900` — the POINT
                // protocol, not the area one ([`Gen::mc2_melee_write`]).
                // It is the LAST statement of the gated arm above
                // (EF:28440), so a re-stamp on a stunned wizard bills
                // nothing.
                if !already_stunned {
                    self.mc2_melee_write(PLAYER_TARGET, amt, id);
                }
            } else {
                self.mc2_debuffs.slow = self.mc2_debuffs.slow.saturating_add(1);
            }
        } else {
            let v = victim as usize;
            if v != 0
                && v < self.ent.len()
                && self.ent[v].class64 == 3
                && self.ent[v].model65 == 0
                && self.ent[v].flags & 0x400 == 0
            {
                let grunt = 54 + (self.ent_rand(i) & 3) as u8;
                self.snd(grunt, v);
                if paralyze {
                    self.mc2_melee_write(v as u16, amt, id);
                }
            }
        }
        self.ent[i].flags |= 0x400;
    }

    /// ⭐⭐⭐ `sub_68AC0` (EF:55397; shipped EXE 0x8D2C0) — THE MAGIC
    /// MINE SWALLOWS ITS OWNER'S SPELL. The consumer half of the
    /// (10,78) beacon ([`Gen::mc2_mine_beacon`] = `sub_68940`, which
    /// BENDS the bolt onto the mine): when a qualifying wizard-owned
    /// bolt actually contacts its owner's ARMED mine, retail spawns a
    /// bare `(10,0)` at the bolt, plays sound 26, RECORDS the spell in
    /// the mine and returns 1 — and every flyer worker's whole impact
    /// tail is the `else` of that (see the call at
    /// [`Gen::mc2_proj_impact`]). Verbatim (EF:55429-45):
    /// ```text
    ///     if (a2x && a2x->class == 10 && a2x->model == 78
    ///         && a2x->word_0x32_50 == a1x->id_0x1A_26
    ///         && a2x->word_0x36_54 == -1)
    ///     {
    ///         _4A190(&a1x->position_0x4C_76, 10, 0);
    ///         PrepareEventSound_6E450(a1x - Entities, -1, 26);
    ///         v4 = a1x->word_0x26_38;
    ///         if (v4) { v5x = Entities[v4];
    ///                   a2x->word_0x36_54 = v5x->model_0x40_64;
    ///                   a2x->word_0x34_52 = v5x->byte_0x46_70;
    ///                   v5x->word_0x2E_46 = 1; }
    ///         v3 = 1;
    ///     }
    /// ```
    /// The model ladder is `sub_68940`'s own, repeated statement for
    /// statement (EF:55407-27) — the same thirteen bolt models.
    ///
    /// ⚠ THE ARMED GATE IS A ONE-SHOT AND IT IS WHAT KEEPS THE MINE
    /// FROM EATING THE WHOLE BARRAGE. `word_0x36_54` is `-1` from
    /// `sub_50840` (EF:36971) and this is its ONLY writer; after the
    /// first swallow it holds the token's model, so both `sub_68940`
    /// and this function stop matching. mc2l6-rsg t=13555 is the
    /// witness: retail's slot 178 goes `f36` 65535 → **9** (the
    /// (15,9) meteor token's model) as the human's first charged
    /// meteor lands on it, and the next meteors — the take's 4-tick
    /// barrage beat — fly past. The class-10 `f36` lane has NO
    /// recorder home (the export returns `None` for class 2/10, where
    /// `f56` holds `@0x38`) and the importer seeds it 0, so the port
    /// carries armed ⟺ `f36 == 0` and stamps `0x8000 | model` — the
    /// retail value with a forced high bit, so that swallowing spell
    /// **0** still disarms.
    ///
    /// ⚠ OPEN — `word_0x26_38` IS THE TOKEN SLOT AND THE PORT KEEPS
    /// THE SPELL INDEX THERE. Retail's bolt carries the (15,x) spell
    /// token's SLOT in `@0x26` (mc2l6-rsg slot 463 reads **353**, the
    /// live (15,9) token) and this function dereferences it; the port
    /// stores the raw spell index (**9**) in the same lane. The mine
    /// stamp lands right anyway — a (15,N) token's `model_0x40_64`
    /// IS N — but `a2x->word_0x34_52 = token->byte_0x46_70` and
    /// `token->word_0x2E_46 = 1` (retail slot 353 `f2e` 2 → 1 on that
    /// tick, the port's class-15 `f26`) cannot be reached until the
    /// launcher stamps the slot. Owner: the cast column.
    fn mc2_mine_swallow(&mut self, i: usize, victim: u16) -> bool {
        if no_mine_swallow() || victim == 0 || victim == PLAYER_TARGET {
            return false;
        }
        let (model, own, x, y, z, tok) = {
            let e = &self.ent[i];
            (e.model65, e.id24, e.x, e.y, e.z, e.f40)
        };
        if !matches!(
            model,
            1 | 2 | 3 | 4 | 5 | 8 | 9 | 0x0C | 0x16 | 0x17 | 0x1A | 0x1C | 0x1E
        ) {
            return false;
        }
        let v = victim as usize;
        let Some(m) = self.ent.get(v) else {
            return false;
        };
        // `word_0x36_54 == -1` (armed) → the port's `f36 == 0`.
        if m.class64 != 10 || m.model65 != 78 || m.f52 != own || own == 0 || m.f36 != 0 {
            return false;
        }
        // `_4A190(&a1x->position, 10, 0)` — a BARE ctor call: no id,
        // no yaw, no life override, nothing. mc2l6-rsg t=13555 slot
        // 439 records exactly `sub_4EE60`'s defaults (max_life 8,
        // life 8, `id_0x1A_26` = its own slot) beside the meteor's
        // OWN trailing spark at 438, which does carry the overrides.
        self.mc2_spawn_fire(x, y, z);
        // ⭐ THE CLANG (EF:55437) — `PrepareEventSound_6E450(a1x, -1,
        // 26)` at the SWALLOWED SHOT's slot, between the ctor and the
        // two writes. The port had the swallow but not its sound, so
        // a magic mine eating a spell was silent.
        if !crate::engine::world::no_mc2_mine_clang() {
            self.snd(26, i);
        }
        // `v4 = a1x->word_0x26_38; if (v4) { v5x = Entities_EA3E4[v4];
        //   a2x->word_0x36_54 = v5x->model_0x40_64;
        //   a2x->word_0x34_52 = v5x->byte_0x46_70;
        //   v5x->word_0x2E_46 = 1; }` (EF:55437-44) — the back-ref is
        // the (15,x) TOKEN'S POOL SLOT and retail DEREFERENCES it.
        let spell = self.mc2_token_model(tok).unwrap_or(0);
        // The one-shot disarm + the spell record (see the doc note).
        self.ent[v].f36 = 0x8000 | spell;
        // ⭐ `a2x->word_0x34_52 = v5x->byte_0x46_70` (EF:55440) — the
        // TIER the swallowed spell was cast at, which is what
        // `sub_3A8B0` case 5 indexes the subspell row with when it
        // relaunches. Reachable now that `word_0x26_38` carries the
        // token's SLOT (`no_token_slot_backref`); f54 is the `f34`
        // lane the importer seats from `@0x34`.
        if let Some(t) = self.ent.get(tok as usize)
            && t.class64 == 15
        {
            let tier = t.f71 as u16;
            self.ent[v].f54 = tier;
        }
        // ⭐⭐⭐ THE SWALLOW HANDS THE CASTER'S WINDOW ONE MORE TICK.
        // `v5x->word_0x2E_46 = 1` is written at the BOLT's walk slot,
        // which is above the book (the human's is 344..369), so the
        // token has already run its own countdown this frame: the
        // write is read on the NEXT frame, and it buys exactly one
        // extra live tick of `sub_68DE0`'s mid-burst regen pin.
        if !no_mine_token_rearm()
            && let Some(t) = self.ent.get_mut(tok as usize)
            && t.class64 == 15
        {
            t.f26 = 1;
        }
        true
    }

    /// `Entities_EA3E4[bolt->word_0x26_38]->model_0x40_64` — the spell
    /// a bolt was cast from, resolved through the (15,x) token slot
    /// the launcher stamped. `None` for the null back-ref (retail's
    /// own `if (v4)` / `if (a1x->word_0x26_38)` gate) or a slot that
    /// no longer holds a class-15 record.
    fn mc2_token_model(&self, tok: u16) -> Option<u16> {
        let e = self.ent.get(tok as usize)?;
        (tok != 0 && e.class64 == 15).then_some(e.model65 as u16)
    }

    /// Impact-effect spawn (the sub_65820 expiry block, EF:62972-96):
    /// spawn `(f68, f69)` at the flyer's position, hand it the id,
    /// heading, victim and carried damage. Routed: fire, big
    /// explosion, meteor, whirlwind, blast23, and the (10,65)/(10,66)
    /// debuff stamps (the (9,20)/(9,21) lobs' payloads). Unported
    /// effects apply the damage directly (deliberate) and count the
    /// misfit.
    ///
    /// ⚠ This seam folds THREE retail impact workers into one, and
    /// they do NOT agree about the spawned effect's leader — see the
    /// stamp block at the tail.
    pub(crate) fn mc2_proj_impact(
        &mut self,
        i: usize,
        victim: u16,
        ctx: &MobCtx,
        at: Option<(u16, u16, i16)>,
    ) {
        // ⭐ THE LIGHTNING BEAM PARKS ITS IMPACT UNTIL THE TRAIL IS DOWN
        // ([`Gen::mc2_beam_defer`]). Retail's per-step worker for the
        // beam is `sub_66610`, which is a bare walk + blocker test whose
        // whole hit arm is `DisableEntityDrawing04_57F10` — reproduced
        // here as the reap bit — and the impact resolves after
        // `sub_66750` has laid all `steps * 8` trail nodes.
        if self.mc2_beam_defer.armed {
            self.mc2_beam_defer.pending = Some((i, victim));
            self.ent[i].flags |= 0x400;
            return;
        }
        let (fc, fm, x, y, z, id, yaw, pitch, dmg, act, lock) = {
            let e = &self.ent[i];
            // ⭐ `at` OVERRIDES THE FLYER'S OWN POSITION — the lightning
            // beam detonates at `v20x`, the trail's next node position
            // (EF:58401), NOT where the marched beam stopped. Every
            // other caller passes None and spawns at the flyer.
            let (x, y, z) = at.unwrap_or((e.x, e.y, e.z));
            (
                e.f68, e.f69, x, y, z, e.id24, e.f30, e.f32, e.f44, e.tick70, e.f146,
            )
        };
        // ⭐⭐⭐ THE MINE SWALLOWS THE SPELL AND THE WHOLE IMPACT TAIL
        // IS GATED ON IT. Retail opens every flyer's detonation block
        // with `if (sub_68AC0(a1x, victim)) { Disable(a1x); }` and
        // spawns NOTHING in that arm — no `_4A190` effect, no
        // `sub_65780` mail, no `sub_686D0` re-point, no `sub_6D8B0`
        // XP: `sub_65820` EF:62974, `sub_65C20` EF:63177, `sub_662E0`
        // EF:63533, `sub_66FD0` EF:58814. All four are folded into
        // this tail. The two POSSESSION workers (`CastPosses_65F60`
        // action 1, `sub_674C0` action 18) and the mine CARRIER's own
        // `sub_67960` (action 30) are NOT in that list — an enumerated
        // absence — and the lightning walk `sub_66750` (EF:58409)
        // calls it as a bare statement and IGNORES the verdict, so the
        // deferred beam keeps its impact too.
        if !matches!(act, 1 | 18 | 30) && !self.mc2_beam_defer.armed && self.mc2_mine_swallow(i, victim) {
            self.ent[i].flags |= 0x400; // DisableEntityDrawing04_57F10
            return;
        }
        // ⭐⭐⭐ STATE 0x0C'S DETONATION IS A CONSTANT, NOT A DESCRIPTOR.
        // Every other worker folded into this tail spawns
        // `_4A190(pos, byte_0x43_67, byte_0x44_68)`; `sub_66FD0` — the
        // Lightning L1/L2 carrier (9,12) — spawns `_4A190(pos, 10, 38)`
        // LITERALLY (EF:58813; shipped NETHERW.EXE file 0x8ba23:
        // `6a 26  push $0x26` / `6a 0a  push $0xa` /
        // `8d 43 4c  lea 0x4c(%ebx)` / `e8 ..  call 0x6e990` =
        // `sub_4A190`) and keeps its own b43/b44 only to CHAIN them
        // onto the storm below. mc2l22 pair 1079: retail revives slot
        // 812 as the (10,38) cloud off the (9,12) at slot 485
        // (b43 9 / b44 9, subSpell 300); the port, reading (9,9) off
        // the IMPORTED bolt, found no arm and minted nothing.
        let (fc, fm) = if act == 12 && self.ent[i].class64 == 9 {
            (10u8, 38u8)
        } else {
            (fc, fm)
        };
        // ⭐ AN IMPACT THAT CANNOT MINT ITS EFFECT DOES NOT DESPAWN
        // THE PROJECTILE. Every MC2 flight worker gates its ENTIRE
        // post-impact tail on the effect entity existing:
        //   `sub_65820`  v11x = _4A190(pos, byte_0x43_67, byte_0x44_68);
        //                if (!v11x) return 0;                EF:62979-81
        //                … DisableEntityDrawing04_57F10(a1x); EF:62995
        //   `sub_65C20`  the whole block is `if (v18x) { … }` EF:63183-97
        //                and its wrappers disable on the RETURNED
        //                effect (`CastPlayerFire_65B30` EF:63007-08,
        //                `sub_65B50` EF:63023-25 + :63050)
        //   `CastPosses_65F60`  `if (v12x) { …; Disable… }`  EF:63310-19
        // So with the pool saturated retail's projectile SURVIVES its
        // own landing — it keeps its slot, its position (already
        // committed by the caller, EF:62954/62941-43), its life (the
        // contact arm never decrements) — and retries next tick. It
        // also delivers NO mail and NO XP: `sub_65780` and `sub_686D0`
        // are inside the same arm.
        // MEASURED (mc2l22, `MGC_WRITE_TRACE=621:flags`): the (9,3)
        // meteor shot at slot 621 contacts terrain at t=10931 with
        // `free_stack len 0 / recycle_stack len 0` on BOTH sides;
        // retail holds flags 6 and life 17 through t=10932 while the
        // port raised 0x400 ("by slot 621 (9,3) f70=3"). The tick-top
        // sweep then freed 621 — ABOVE every slot that tick's walk
        // frees — so it sat on TOP of the free stack and the (10,17)
        // meteor's ring at slot 429 popped it FIRST, sliding all 28
        // ring children one slot down the pop list (t=10932, 80 field
        // rows, port slot 24 = retail slot 3 verbatim).
        // ⭐⭐⭐ THE VICTIM GATE BELONGS TO THE WORKER, NOT TO THE
        // PAYLOAD. `sub_674C0` — action 18, the LEVELED possession
        // (9,17) — wraps its ENTIRE spawn block in `if (v6x)`, the
        // `sub_108B0` claim probe's result (EF:59029-59058): the
        // ground-stop arm and the life-expiry arm both fall through to
        // `LABEL_16` with `v6x == 0`, so retail runs
        // `DisableEntityDrawing04_57F10(a1x)` and NOTHING else — no
        // claim pulse, no aura, no `sub_65780` mail, no `sub_6D8B0`.
        // Its basic twin `CastPosses_65F60` (action 1) has no such
        // gate: its whole block is unconditional (EF:63305-19), which
        // is exactly why the (10,12) arm below mints on a ground miss.
        //
        // The port carried the gate on the (10,54)/(10,69) PAYLOAD
        // arm instead, so an action-18 bolt holding any other payload
        // minted on a victimless expiry. mc2l22 t=523 is the witness:
        // rival 611's leveled possession bolt at slot 718 runs its
        // life out in mid-air (retail: life 0 -> -1, flags 6 -> 1030,
        // nothing allocated, free stack 296 -> 296 with next-pop still
        // 729) while the port popped 729 for a (10,12) claim pulse —
        // the take's whole horizon wall at 522.
        if act == 18 && victim == 0 && !no_a18_victim_gate() {
            self.ent[i].flags |= 0x400;
            return;
        }
        let alloc_watermark = self.exhausted;
        let spawned = match (fc, fm) {
            (10, 0) => self.mc2_spawn_fire(x, y, z),
            (10, 1) => self.mc2_spawn_big_explosion(x, y, z),
            // The possession delivery (the basic `(9,1)` bolt's
            // payload): retail does NOT write the claim from the
            // bolt — it spawns a separate (10,12) CLAIM PULSE entity
            // (`_4A190(&pos, byte_0x43, byte_0x44)`, EF:63306-19 /
            // EF:59053-58) that broadcasts the ch1 channel from its
            // own 9-tick action (docs/traces/mc2-possession-delivery
            // .md §1-§2). The bolt then copies its id/yaw/pitch onto
            // it (EF:63315-17) — the claim's OWNER lane rides the
            // pulse's `id_0x1A_26`, so the pulse must carry the
            // caster or every intake reads the pulse's own slot.
            //
            // The port used to fold the broadcast into a single
            // `area_write` from the bolt: that dropped the entity
            // (the mc2l4 (10,12) 279-row missing family in the first
            // 4,000 pairs, l30 313, l4 779 full-take) AND narrowed
            // the claim to the bolt's own box for one tick instead of
            // the pulse's 512³ box for nine — retail's near-miss
            // claims come from exactly that reach.
            (10, 12) => {
                if let Some(p) = self.mc2_spawn_claim_pulse(x, y, z, false) {
                    let e = &mut self.ent[p];
                    e.id24 = id;
                    e.f30 = yaw;
                    e.f32 = pitch;
                }
                None
            }
            // Possession tiers 1/2 (docs/spell-audit/possession.md):
            // the weak claim pulse PLUS a persistent attract aura —
            // the Mana Magnet (model 54, range 15 tiles) / Mana Lock
            // (model 69, range 20). The aura (`mc2_spawn_aura` +
            // `mc2_aura_tick`) drags unowned mana spheres to the
            // caster and merges under the caster's owner. Range = the
            // tier's `subSpell` (15/20, CD spells.bin); the ctor
            // default 14 is for authored magnets.
            //
            // Retail gates BOTH children on an actual probe victim
            // (`if (v6x)`, EF:59032-59058): a ground stop / expiry
            // with no victim spawns neither the claim pulse nor the
            // aura (unlike basic possession's ground-miss pulse).
            // The claim pulse fires on ANY victim (balls and
            // dwellings claim alike — player retail-verified); the
            // AURA manifests ONLY when the victim is a mana sphere —
            // building/worm possession never magnets, and neither
            // does mid-terrain (PLAYER RETAIL-CERTIFIED 2026-07-27,
            // overruling a decompile pass that read EF:59048 as
            // unconditional inside the victim arm — the recorded
            // gameplay wins; MC1's magnet differs on BOTH counts:
            // it drops its pair mid-terrain too, and never magnets
            // buildings because its scan is balls-only).
            // Tier 2 (Mana Lock, model 69) delivers the FORCED claim:
            // retail's impact spawns the (10,70) steal pulse instead
            // of (10,12) when xsubtype == 69 (EF:59036-39), whose
            // action broadcasts force = 1 (sub_32120 → sub_112D0(1),
            // EF:23559) — the intakes steal unconditionally and set
            // the byte[2]&0x20 claim lock, which weak claims then
            // bounce off.
            (10, 54) | (10, 69) => {
                let struck =
                    victim != 0 && victim != PLAYER_TARGET && (victim as usize) < self.ent.len();
                if struck {
                    // EF:59036-45 — the pulse spawns FIRST (it takes
                    // the lower pool slot; the l24 `want=12 got=54`
                    // rows were the whole aura column shifted by this
                    // one missing allocation), model chosen by the
                    // PAYLOAD: (10,70) forced when the payload is
                    // (10,69), (10,12) weak otherwise.
                    if let Some(p) = self.mc2_spawn_claim_pulse(x, y, z, fm == 69) {
                        let e = &mut self.ent[p];
                        e.id24 = id;
                        e.f30 = yaw;
                        e.f32 = pitch;
                    }
                    // ⭐⭐⭐ THE AURA IS MINTED FOR **ANY** PROBE VICTIM —
                    // THE RECORDING OVERRULES THE 2026-07-27 VISUAL
                    // CERTIFICATION ABOVE. `sub_674C0`'s victim arm
                    // (EF:59032-58) spawns its second child
                    // `_4A190(&pos, byte_0x43_67, byte_0x44_68)` at
                    // EF:59048 with no test on what `v6x` is, and
                    // mc2l22 pair 8184→8185 records exactly that on a
                    // BUILDING: rival 557's Mana Lock (9,17) at slot
                    // 691 (b44 69, `target96` 146 = the (10,45) at
                    // (47104, 48128, 0)) lands, and retail borns the
                    // (10,70) forced pulse at 763 AND the (10,54)
                    // aura at 892 (`f1a` 557, yaw/pitch 963/79 = the
                    // bolt's, `dword_0x10_16` 26214400 = the tier-2
                    // range) — and on the same tick the aura's pull
                    // already reaches the (10,39) sphere at 979
                    // (`mail4.amt` 0 → 42). The port's sphere gate
                    // skipped that allocation, so both archer arrows
                    // born later in the walk (retail 692/972) popped
                    // one slot early (892/692) and retail's arrow at
                    // 972 read `missing in port`. The old gate's
                    // "building/worm possession never magnets" came
                    // from visual play, not from a recording; the
                    // pool says otherwise.
                    {
                        if let Some(a) = self.mc2_spawn_aura(x, y, z) {
                            // ⭐ BOTH TIERS' AURA IS MODEL 54 — THE
                            // TIER LIVES IN `dword_0x10_16`, NOT IN
                            // THE MODEL. Retail spawns the second
                            // child as `_4A190(&pos, byte_0x43_67,
                            // byte_0x44_68)` (EF:59048), and the
                            // subtype-69 row resolves to the SAME
                            // ctor as 54 — `AddAuxiliary_50500`,
                            // whose third statement is the literal
                            // `model_0x40_64 = 0x36` (EF:36819). A
                            // ctor's hardcoded model OUTRANKS the
                            // subtype that selected it. mc2l6-rsg
                            // t=3920 slot 598: the Mana Lock's aura
                            // is (10,**54**) with `dword_0x10_16`
                            // 26214400 = (20<<8)², beside its own
                            // (10,70) forced pulse at 599 — where the
                            // port stamped model 69. Nothing reads
                            // the model (the tick dispatches on
                            // action 0x3B and the field is invisible),
                            // so the range home below carries the
                            // whole tier difference.
                            self.ent[a].f26 = if fm == 54 { 15 } else { 20 };
                            self.ent[a].id24 = id;
                            // ⭐ THE AURA TAKES THE BOLT'S BEARING TOO.
                            // `sub_674C0` stamps its SECOND child with
                            // the same three words as its first —
                            // `v14x->id_0x1A_26 / yaw_0x1C_28 /
                            // pitch_0x1E_30 = a1x->…` (EF:59050-52),
                            // the claim pulse's EF:59042-44 one line
                            // up — and the ctor's own random yaw
                            // (`AddAuxiliary_50500`'s single draw) is
                            // overwritten, not kept. Only `id24`
                            // crossed here, so a magnet stood at its
                            // ctor roll: mc2l6-rsg t=3551 slot 569,
                            // retail heading 446 / pitch 35 (its
                            // bolt's post-move bearing) against the
                            // port's 1105 / 0.
                            self.ent[a].f30 = yaw;
                            self.ent[a].f32 = pitch;
                        }
                    }
                }
                None
            }
            // Crater (spell 16): the action wrapper `sub_66280`
            // (EF:63400-02) overrides the scorch ring's LIFE with the
            // tier charge (6/12/24) — the carve radius grows every
            // 3rd frame, so life IS the tier scaling.
            (10, 11) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_scorch_ring(x, y, z);
                if let Some(s) = s {
                    self.ent[s].act_life = charge as i32; // 6/12/24
                }
                s
            }
            // Meteor (spell 9): the fuse override `v1x->maxLife/life
            // = byte_0x46_70` lives ONLY in the ACTION-3 wrapper
            // `sub_66180` (EF:63372-73), selected by the
            // function-pointer action table (Events.cpp:3281) — it is
            // the PROJECTILE'S action row's law, not the impact
            // pair's. Any other shot landing a (10,17) — the volcano
            // bolt — takes the generic impact tail (EF:63306-19),
            // which has NO override, so the impact keeps the ctor's
            // 10/10 (mc2l30 t=230/234: life/max_life retail 9/10
            // where the folded override minted 1/1 off the volcano
            // bolt's unset f71 through an invented `.max(1)`).
            (10, 17) => {
                let s = self.mc2_spawn_meteor(x, y, z);
                if self.ent[i].tick70 == 3
                    && let Some(s) = s
                {
                    let ml = self.ent[i].f71 as u32;
                    self.ent[s].max_life = ml;
                    self.ent[s].act_life = ml as i32;
                }
                s
            }
            // The ground/quake family (docs/spell-audit/quake-family
            // .md + gravity-cavein.md): each spell's projectile impact
            // routes to a terrain effect whose handler is already
            // ported. The impact tail propagates the tier's
            // subSpell→f140 (damage) and leaves f71 as the ctor's
            // phase seed.
            // Tremor (spell 15): `sub_677D0` (EF:59128-32) sets BOTH
            // lives to `charge & 0xF0` (60/80/120 → 48/80/112) and
            // zeroes the phase seed (byte_0x46_70).
            (10, 71) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_fissure(x, y, z);
                if let Some(s) = s {
                    let ml = (charge & 0xF0) as u32;
                    self.ent[s].max_life = ml;
                    self.ent[s].act_life = ml as i32;
                    self.ent[s].f71 = 0;
                }
                s
            }
            // Earthquake (spell 17): the action wrapper `sub_66160`
            // (EF:63333-35) sets the trail's LIFE = 1× charge
            // (16/32/64), life ONLY — the 8× law belongs to
            // whirlwind's sub_678E0 alone.
            (10, 15) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_fire_trail(x, y, z);
                if let Some(s) = s {
                    self.ent[s].act_life = charge as i32; // 16/32/64
                }
                s
            }
            // Volcano (spell 18): `sub_66250` (EF:63388-90) overrides
            // the dome's MAX life (the radius law R = maxLife|1 →
            // 7/9/11 per tier) and zeroes the phase seed; act_life
            // (the raise duration 17) stays the ctor's.
            (10, 9) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_dome(x, y, z);
                if let Some(s) = s {
                    self.ent[s].max_life = charge as u32; // 7/9/11
                    self.ent[s].f71 = 0;
                }
                s
            }
            // Gravity Well (spell 20): `sub_677A0` (EF:59112-14) sets
            // the flood's LIFE = charge (16/26/40) + phase 0.
            (10, 67) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_flood(x, y, z);
                if let Some(s) = s {
                    self.ent[s].act_life = charge as i32; // 16/26/40
                    self.ent[s].f71 = 0;
                }
                s
            }
            // The whirlwind's action wrapper `sub_678E0` (class-9
            // action 27, EF:59109-22) overrides `AddWind`'s ctor life
            // with `8 * byte_0x46_70` (the tier charge) — THIS is what
            // scales Tornado I/II/III (row-21 tier lives 5/10/10 →
            // 40/80/80 ticks). Without it every tier casts at the ctor
            // default 500 and roams identically.
            (10, 22) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_whirlwind(x, y, z);
                if let Some(s) = s {
                    let ml = 8 * charge as u32;
                    self.ent[s].max_life = ml;
                    self.ent[s].act_life = ml as i32;
                }
                s
            }
            // The CHARGED/repeat fireball's firestorm (spell 0, tier
            // life>=2 → arm 28/(10,76)): retail's action `sub_65B50`
            // (EF:63012-53) spawns the (10,76) fire orb via
            // `sub_65C20`, overrides the head's maxLife to 30 — a
            // brief burst vs the level's 80-life authored firestorm —
            // then LEADERS the hub (EF:63029) and stamps the owner id
            // onto all 25 satellites, swapping the local human's to
            // the star sprite 42 (`SetEntityIndex_49C90` — index
            // only, the row-340 extent quad stays). The hub's leader
            // is the PROJECTILE'S OWN LOCK, not the struck victim —
            // the stamp block at the tail of this fn carries the law
            // and the reason the two structure kinds behave
            // differently. ⚠ `docs/traces/mc2-class10-m76-fire-
            // spheres.md` §7 read the missing struck-write as a
            // remc2 transcription gap; that is SUPERSEDED as the
            // explanation (though not disproven — remc1's twin
            // `sub_52ED0_53210` really does carry a struck-write).
            (10, 76) => {
                let s = self.mc2_spawn_fire_orb(x, y, z);
                if let Some(s) = s {
                    self.ent[s].max_life = 30;
                    self.ent[s].act_life = 30;
                    let human = id == PLAYER_TARGET;
                    let mut n = self.ent[s].f54 as usize;
                    while n != 0 {
                        self.ent[n].id24 = id;
                        if human {
                            // `SetEntityIndex_49C90` (EF:32832-34)
                            // writes THREE fields: @0x5A, @0x5C = 0
                            // and @0x5D = the row's frame count.
                            let f = crate::mc2::mobs::mc2_sprite_frames(42);
                            let e = &mut self.ent[n];
                            e.type86 = 42;
                            e.frame88 = 0;
                            if !crate::mc2::mobs::no_mc2_frames89() {
                                e.frames89 = f;
                            }
                        }
                        n = self.ent[n].f54 as usize;
                    }
                }
                s
            }
            // Steal Mana (13): the (10,25) "steal" burst. `sub_662E0`'s
            // impact arm (EF:63531-63562) spawns the effect ONLY when
            // the struck victim is a class-3 model-0/1 wizard
            // (EF:63537; terrain / creature / expiry fizzle with no
            // burst), then stamps onto it `id`, `yaw`, `pitch`,
            // `word_0x96_150 = victim`, `subSpellIndex` and
            // `byte_0x46_70` = the TIER (EF:63552-59). The burst is a
            // real pool record — `sub_4F6A0` (EF:36110), maxLife 8,
            // extents 512 — and its own action `sub_33E20` (EF:24817)
            // area-stamps ch3 = the tier on every OTHER class-3 record
            // in its 5x5 window ONE TICK LATER, at the burst's slot
            // (`Gen::mc2_blast25_tick`). The consumers (`sub_61050`)
            // resolve `SPELLS[13].subspell[tier]` themselves.
            //
            // The port used to skip the record and poke the victim's
            // inbox by hand with the bolt's f44 as an AMOUNT — no
            // allocation (mc2l6-rsg t=8674: retail pops slot 102 for
            // the burst, the port's free stack keeps it and every
            // later pop shifts), no tick of delay (the port drained
            // at the impact tick, retail at the burst's first tick),
            // and the wrong unit on the channel (an amount where
            // retail carries a tier index).
            (10, 25) => {
                let wizard = victim == PLAYER_TARGET
                    || self
                        .ent
                        .get(victim as usize)
                        .is_some_and(|v| v.class64 == 3 && matches!(v.model65, 0 | 1));
                if !wizard {
                    None
                } else {
                    let tier = self.ent[i].f71;
                    let s = self.mc2_spawn_blast25(x, y, z);
                    if let Some(s) = s {
                        self.ent[s].f71 = tier; // byte_0x46_70 (EF:63559)
                        self.ent[s].f44 = dmg; // subSpellIndex copy (EF:63558)
                    }
                    s
                }
            }
            // Magic Mine (spell 23): the (9,29) carrier lands and places
            // a PERSISTENT proximity mine `(10,78)` (`sub_50840`), not a
            // fireball. The carrier arrives ~15 tiles ahead (maxLife 10 ×
            // speed 384 fuse) and the mine is born AT IT — `sub_50840`
            // copies the axis it is handed (EF:36968/36981); the old
            // ground snap was the port's own. The carrier's `f44` is the
            // TIER INDEX (`sub_6CAC0` EF:57993, the one arm that ships
            // the index rather than the payload) and the tail seats it
            // in the mine's `f44`; the owner rides `word_0x32_50` (f52),
            // never `id_0x1A_26`. docs/spell-audit/magic-mine.md.
            (10, 78) => {
                let tier = self.ent[i].f71;
                let s = self.mc2_spawn_magic_mine(x, y, z, tier, dmg as i32);
                // ⭐⭐⭐ AND THE MINE IS *SOLID*. `sub_50840`'s ctor
                // spells `event->struct_byte_0xc_12_15.byte[0] |= 8u`
                // (EF:36969) — it SETS the collide bit, where the
                // port's spawner CLEARS it (`flags & !0x2_0008`), an
                // INVERTED write. `sub_10780`'s first test is exactly
                // `byte[0] & 8` (EF:3763, the same law the castle
                // piece's `&= 0xF7` carries in `mc2/castle.rs`), so
                // with the bit down the victim probe walks straight
                // through the mine and `sub_68AC0` is never handed
                // it. mc2l6-rsg t=13544 slot 178: retail `flags` 12
                // (link|collide) against the port's 4.
                // ⚠ FOLDED INTO THE CTOR (`mc2_spawn_magic_mine`),
                // where retail spells it — it was landed here only
                // because effects.rs was another dig's file.
                s
            }
            // Summon Army (spell 19): the (9,24) carrier lands and spawns
            // a ring of allied class-5 creatures (`sub_51800`→`sub_3A5B0`
            // node ring, collapsed to a direct ring here). The creature
            // MODEL rides f71 (19/2 firefly-or-bee, 25 Cymmerian, 16
            // wyvern), which also sets the army size. NOT a quake — the
            // `byte_0x44_68 = 72` is a MODEL
            // (docs/spell-audit/summon-creatures.md Part B).
            (10, 72) => {
                let model = self.ent[i].f71;
                let head = self.mc2_spawn_summon_ring(x, y, model, id);
                // ⭐⭐⭐ …BUT THE ARM RETURNING `None` DOES NOT MEAN
                // RETAIL STAMPED NOTHING. `sub_65820`'s tail writes
                // the record `_4A190` returned — `sub_51800`'s HEAD —
                // and the bearing pair (EF:62989-90) is the half this
                // arm was throwing away. See
                // [`World::mc2_summon_head_bearing`] (mc2/roster.rs)
                // for the citation, the qmemcpy/head split and the
                // free-run witness. Kill switch
                // `MGC_NO_SUMMON_HEAD_BEARING`.
                self.mc2_summon_head_bearing(head, yaw, pitch);
                None
            }
            // Alliance (spell 24): the (10,74) conversion executor
            // (`sub_50800` → class-10 action 0x51 = `sub_3A650`,
            // EF:36945/29637) — a SAME-SPECIES AREA CHARM centered on
            // the struck creature. Radius = the tier charge f71
            // (16/26/32 tiles), duration = f44 (subSpell 610/1100/
            // 2710 ticks), owner = the caster. ZERO damage anywhere —
            // neither the flyer path nor the handler hurts anything.
            // A victimless detonation (terrain hit) fizzles, like
            // retail's executor with no `word_0x96_150`.
            //
            // ⭐⭐⭐ AND THE EXECUTOR IS A REAL POOL RECORD, EXACTLY LIKE
            // THE (10,25) BURST ABOVE. `sub_50800` (EF:36945) mints it
            // and `sub_3A650` (EF:29637) — its class-10 action 0x51 —
            // does the converting ONE TICK LATER, off the record's own
            // `word_0x96_150` / `byte_0x46_70` / `subSpellIndex`. The
            // port converted inline here, which cost an allocation (so
            // every later free-stack pop shifted) and put the whole
            // charm a tick early on every victim lane at once.
            // mc2l0-spells-galore t=23946 (the take's free-run horizon
            // after the whirlwind law): retail pops slot 70 for the
            // executor and its three (5,4) archers 559/621/627 keep
            // `sv2` 0 / `action` 34 / `speed` 0 / `owner28` 0 for one
            // more tick, taking the charm at t=23947 — the port had
            // already flipped all four lanes at 23946 and minted
            // nothing, so the pair read as a perfectly symmetric
            // one-tick-stale swap in both directions.
            (10, 74) => {
                if crate::mc2::mobs::no_mc2_alliance_record() {
                    let radius = self.ent[i].f71 as i32;
                    self.mc2_alliance_convert(victim, id, radius, dmg as i32);
                    None
                } else {
                    self.mc2_spawn_alliance_exec()
                }
            }
            (10, 23) => self.mc2_spawn_blast23(x, y, z),
            // Lightning L1/L2 storm burst (`sub_66FD0`'s hard-coded
            // `(10,38)` spawn, EF:58813). Full retail internals (the
            // chained second-order `(9,9)` beam, exact life/sprite) are
            // untraced — interim: a one-shot area-damage flash carrying
            // the tier's subSpell (via the f140 tail below), so the
            // storm is visible + damaging and the (9,9) misfit is gone.
            // ⭐⭐⭐ THE DUEL TETHER IS BORN ONLY ON A **WIZARD** HIT.
            // The (9,7) dart's flight worker is `sub_662E0`, not the
            // generic `sub_65820`, and its resolution arm forks three
            // ways (EF:63531-63545): shield-reflect → disable; **`!v5x
            // || v5x->class_0x3F_63 != 3 || (model != 0 && model != 1)`
            // → `sub_65780(a1x, 0, …)` and disable, NO EFFECT SPAWN**;
            // only a struck class-3 model-0/1 wizard reaches
            // `_4A190(pos, byte_0x43_67, byte_0x44_68)`. So a duel dart
            // that misses, expires or stops on terrain leaves nothing
            // at all — the (10,26) tether exists only where it caught
            // somebody. (This is the `sub_662E0` fork SESSION 75 banked
            // as "not reproduced"; the corpus settles it.)
            //
            // mc2l6-rsg carries nine (9,7) darts and grades the law
            // exactly: slots 807/857/617/379/172/208 strike rival 370
            // and each births a (10,26) on that same tick; slot 855
            // (t=1326-1347, the full 21-tick life expiry high in the
            // air) and slots 842/843 die with NOTHING allocated — the
            // free stack pops once at t=1347 and it goes to an
            // unrelated (10,66).
            //
            // The tether's own ctor is `sub_4F720` (EF:36129): action
            // 26, maxLife 8, `subSpellIndex_0x2A_42 = 200`, sprite
            // **213** (the July trace guessed 284) and
            // `SetEntityShiftRot_49EA0(event, 512, 512)` — the extents
            // quad retail records as apitch/aroll/afov 512 where the
            // bare sprite row gives 78/78/75. `dword_0x10_16` is left
            // at zero: it is the marker tick's own FRAME COUNTER, not
            // a radius (see `mc2_duel_tether_tick`). The generic tail
            // below stamps `id24`/`f30`/`f32`/`f146` = the victim, and
            // the `subSpellIndex` copy (EF:63544) overwrites the
            // ctor's 200 with the dart's payload.
            (10, 26) => {
                let wizard = victim == crate::mc1::mobs::PLAYER_TARGET
                    || self
                        .ent
                        .get(victim as usize)
                        .is_some_and(|v| v.class64 == 3 && matches!(v.model65, 0 | 1));
                if !wizard {
                    None
                } else {
                    let tier = self.ent[i].f71;
                    self.new_event().inspect(|&t| {
                        {
                            let e = &mut self.ent[t];
                            e.class64 = 10;
                            e.model65 = 26;
                            e.tick70 = 26;
                            e.max_life = 8;
                            e.f126 = 16;
                            e.f71 = tier; // byte_0x46_70 (EF:63545)
                            e.f44 = dmg; // subSpellIndex copy (EF:63544)
                            e.flags &= !8;
                        }
                        self.link(t, x, y, z);
                        self.refill_life(t);
                        self.mc2_set_sprite(t, 213);
                        self.mc2_shift_rot(t, 512, 512);
                    })
                }
            }
            // ⭐ `sub_66FD0` CHAINS ITS OWN DESCRIPTOR PAIR ONTO THE
            // STORM (EF:58825-26; EXE 0x8baa6-0x8bab0 copies
            // 0x1a/0x1c/0x1e/0x96/0x2a/0x43/0x44 onto the new record),
            // and the storm's own tick spawns every beam it rains as
            // `_4A190(pos, byte_0x43_67, byte_0x44_68)` (`sub_35640`
            // EF:25919-24). So the bolt's `(9,9)` is load-bearing, not
            // decoration — it is the storm's rain descriptor.
            (10, 38) => {
                let chain = (self.ent[i].f68, self.ent[i].f69);
                let s = self.mc2_spawn_lightning_burst(x, y, z);
                if let Some(s) = s {
                    self.ent[s].f68 = chain.0;
                    self.ent[s].f69 = chain.1;
                }
                s
            }
            (10, 65) => self.mc2_spawn_stagger(x, y, z),
            (10, 66) => self.mc2_spawn_paralyze(x, y, z),
            // The Cave-In ground effect: the action-31 wrapper's
            // post-impact fixup rides here (sub_67910 EF:59218-30 —
            // maxLife = the tier charge, phase reset to 0).
            (10, 89) => {
                let charge = self.ent[i].f71;
                let s = self.mc2_spawn_cave_in(x, y, z);
                if let Some(s) = s {
                    self.ent[s].max_life = charge as u32;
                    self.ent[s].f71 = 0;
                }
                s
            }
            _ => {
                self.note_misfit(fc as u16, fm as u16);
                let amt = dmg as u32;
                self.area_write(i, 0, amt, ctx, false, false);
                None
            }
        };
        // `if (!v11x) return 0;` (EF:62981). The pool ran dry INSIDE
        // this impact — leave the projectile alive and untouched.
        // (`spawned == None` on its own is NOT that: the possession,
        // aura, summon-ring and alliance arms deliberately return None
        // after minting their own children; only a failed allocation
        // moves `exhausted`.)
        if spawned.is_none() && self.exhausted != alloc_watermark {
            // ⭐⭐⭐ …EXCEPT THE LEVELED POSSESSION, WHICH DIES ON CONTACT
            // WHATEVER THE POOL SAYS. `sub_674C0` (action 18) gates its
            // two CHILDREN on their own allocations — `if (v12x) {…}`
            // for the claim pulse, `if (v14x) {…}` for the aura
            // (EF:59040-53) — but its `DisableEntityDrawing04_57F10(a1x)`
            // is the LAST statement of the `if (v16)` block, OUTSIDE
            // both (EF:59055). A struck bolt is spent even when the
            // pool hands back nothing; only the effect-existence-gated
            // workers folded above (`sub_65820` EF:62981, `sub_65C20`
            // EF:63183, `CastPosses_65F60` EF:63310) keep flying.
            //
            // WITNESS (mc2l22 t=8347, pool exhausted — free 0 / victim
            // 0): rival 557's Mana Lock (9,17) at slot 991 is minted at
            // its token's slot and snaps onto the (10,39) sphere at 484
            // the same tick, at (48691, 48938, 4256) on BOTH sides;
            // retail records it `flags` 0x406 (link + done2 + REAP) with
            // `life` 10 untouched and mints nothing, the port left it
            // flying (`life` 10 → 9 at 8348, x 48691 → 48944). The
            // tick-top reap at 8348 therefore freed 991 in retail and not
            // in the port, and 991 — the HIGHEST freed slot, the free
            // stack's top — went to the human turret's beam in retail
            // while the port's beam popped 959: an INHERITED head whose
            // whole signature was the pop order (retail 991/959/950/933/
            // 773 vs the port's 959/773/772).
            if act == 18 {
                self.ent[i].flags |= 0x400;
            }
            return;
        }
        if let Some(s) = spawned {
            // ⭐ THE LEADER STAMP IS NOT UNIVERSAL. Three retail impact
            // workers fold into this seam and only two of them
            // struck-stamp: `sub_65820` (the generic expiry, EF:62992)
            // and `CastPosses_65F60` (EF:63557) hand the effect the
            // STRUCK victim, but the FIREBALL worker `sub_65C20`
            // (EF:63057) never writes the effect's leader at all — it
            // only ZEROES the projectile's OWN homing lock when nothing
            // was struck (EF:63195-96). Its action-29 wrapper
            // `sub_65B50` then copies that lock onto the (10,76)
            // firestorm hub (EF:63027-29); action 0's wrapper
            // `CastPlayerFire_65B30` (EF:63005-09) copies nothing, so
            // the plain (10,0) splat keeps its memset 0.
            //
            // ⭐ That upstream lock IS the structural distinction
            // between a castle and a (10,45) building under a charged
            // fireball: `sub_67CB0` case 0x1C walks the class-3 list
            // (castles scored by `sub_685D0`, EF:54783/54790) and the
            // class-5 buckets, and NEVER the building list
            // `dword_38527` — which only the model 1/0x11 possession
            // arm reaches (EF:55047). A fireball can LOCK a castle; it
            // can never lock a building. [`Gen::mc2_aim_lists`] already
            // has this exactly (model 0x1C = wizards + creatures, no
            // buildings). Leader = a castle → the hub's phase-0 sizing
            // takes the 3392/640 bounds off `leader.f80` (`sub_339B0`
            // EF:24581-90) and the per-tick HARD SNAP re-centres the
            // ring on `leader.pos.z + leader.f78` (`sub_33C70`
            // EF:24722-45) = the engulf. Leader 0 → the AUTHORED
            // 192/480 compact ring (EF:35950-51) floats where the ball
            // died, riding the building's own stamped heightmap
            // (EF:27341) = "spins and runs above the flag".
            //
            // Damage is untouched either way (`sub_33C00` EF:24700-14,
            // 70 per satellite from the satellite's own quad).
            // ⭐⭐⭐ AND A MISS CLEARS THE LOCK BEFORE THE WRAPPER
            // READS IT — A PHASE LAW, NOT AN ARITHMETIC ONE.
            // `sub_65C20`'s LAST act inside `if (v18x)` is
            // `if (!v9x) a1x->word_0x96_150 = 0;` (EF:63196; shipped
            // `NETHERW.EXE` file 0x8A73E `test %esi,%esi` /
            // `jne 0x8a756` / `movw $0x0,0x96(%ebx)`, `esi` = the
            // struck victim `v9x`). Its action-29 wrapper `sub_65B50`
            // is `v2x = sub_65C20(a1x); if (v2x) { v3 =
            // a1x->word_0x96_150; … v2x->word_0x96_150 = v3; }`
            // (EF:63016-23; 0x8A362 `call 0x8a420` THEN 0x8A374
            // `mov 0x96(%edi),%bx`) — so the hub is stamped with the
            // POST-clear word. A charged fireball that expires or
            // lands on terrain therefore hands the firestorm leader
            // **0** and the ring floats where the ball died; only a
            // fireball that actually STRUCK something engulfs its
            // victim. The port read `lock` off the flyer at the top of
            // this fn, before any of that, and a stale lock makes
            // `sub_33C70`'s HARD SNAP teleport the hub and all 25
            // satellites onto a completely unrelated entity.
            // mc2l22 pair 47946→47947: the (9,28) at slot 445 expires
            // with `word_0x96_150` 181 (a (5,21) devil 30 tiles away)
            // and no victim; retail's hub 756 keeps the ball's
            // (7739,47743,4650), the port snapped the whole
            // constellation to (15,45132) — 26 records, 78 field rows.
            let leader = match act {
                29 if victim == 0 && !no_fireball_lock_clear() => 0,
                29 => lock,
                0 => 0,
                // ⭐⭐⭐ …AND THE TWO LIGHTNING WORKERS DO NOT TEST FOR
                // A NULL VICTIM AT ALL. `sub_66750` (action 9) and
                // `sub_66FD0` (action 12) divide the victim POINTER by
                // the record size and store the quotient BEFORE they
                // check whether there was a victim, so a terrain hit
                // stamps `-(EntitiesBase / 168)` truncated to 16 bits
                // where `sub_65820`'s guarded twin leaves the memset 0.
                // See [`mc2_null_victim_ghost`] for both disassemblies,
                // the census proof (1,037/1,037 and 7/7 rows) and why
                // this is OPT-IN.
                9 | 12 if victim == 0 && mc2_null_victim_ghost() => MC2_NULL_VICTIM_GHOST,
                _ => victim,
            };
            // ⭐ THE GENERIC CORE ALSO HANDS THE EFFECT ITS FUSE, AND
            // THE LIST IS ENUMERATED. `sub_65820` ends
            // `v11x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42;
            // v11x->byte_0x46_70 = a1x->byte_0x46_70;` (EF:62993-94) —
            // the port had the first line (`f140` below) and not the
            // second. It is the CORE's alone: `sub_65C20` (actions
            // 0/29, EF:63183-97), `CastPosses_65F60` (action 1,
            // EF:63310-19) and `sub_674C0` (action 18, EF:59029-58)
            // stamp id/yaw/pitch and stop. The `sub_65820` WRAPPERS
            // that re-zero it afterwards (`sub_677D0` EF:59130,
            // `sub_67760` EF:59113, …) are the port's own match arms
            // above, so they keep the last word.
            // mc2l22 t=10922..10933, the (10,17) meteor at slot 429
            // born off the action-3 shot: `b46` retail 10 / port 0 on
            // all 12 boundaries.
            let fuse = if matches!(act, 0 | 1 | 18 | 29)
                || matches!(
                    (fc, fm),
                    (10, 9) | (10, 25) | (10, 26) | (10, 67) | (10, 71) | (10, 89)
                ) {
                None
            } else {
                Some(self.ent[i].f71)
            };
            // ⭐ THE MAGIC-MINE CARRIER'S TAIL STAMPS NEITHER BEARING
            // WORD. `sub_67960`'s whole impact block (EF:59352-62) is
            //     resultx->word_0x32_50   = a2x->id_0x1A_26;
            //     resultx->byte_0x46_70   = 0;
            //     resultx->subSpellIndex  = a2x->subSpellIndex;
            //     DisableEntityDrawing04_57F10(a2x);
            // — an ENUMERATED list with no `yaw`/`pitch`/`word_0x96`
            // line anywhere, unlike every other impact worker
            // (sub_65C20 EF:63194-95, sub_65820 EF:62990-91,
            // CastPosses EF:63316-17). The (10,78) mine therefore
            // keeps `sub_50840`'s memset zeros. mc2l6-rsg pair
            // 13050→13051 slot 775: retail heading **0** / pitch **0**
            // where this tail wrote the carrier's 810 / 94.
            let bare_tail = act == 30 && !no_mine_carrier();
            // ⭐⭐⭐ THE LIGHTNING IMPACT PASSES ITS BLAST'S PAYLOAD
            // THROUGH THE STRUCK WIZARD'S REBOUND WINDOW. `sub_66750`
            // is NOT the generic `subSpell = subSpell` copy every other
            // impact worker runs (`sub_65820` EF:62993, `sub_65C20`
            // EF:63191, `CastPosses_65F60` EF:63558, `sub_66FD0`
            // EF:58836) — it runs its own ladder (EF:58421-52):
            // no victim / class != 3 / no `word[0] & 0x8010` window ⇒
            // FULL; else `byte[1] & 0x80` (= 0x8000 REBOUND) ⇒ QUARTER
            // if the caster's mana/4 fits the VICTIM's purse, else the
            // 0x10 PRECISE tier ⇒ HALF on the same test.
            //
            // ⚠⚠ THE SHIPPED EXE OUTRANKS THE LISTING, AND HERE IT
            // CORRECTS IT. EF:58436 reads the HALF arm's mana gate off
            // `a1x` — the CASTER — which is vacuous (`mana/8 <= mana`).
            // `NETHERW.EXE` 0x8B2F9 is `cmp eax,[esi+0x90]`, the
            // VICTIM's mana, exactly like the quarter arm at 0x8B2C5.
            // (Ladder disassembled at 0x8B292-0x8B324.)
            //
            // WITNESS mc2l22 t=11823: the human's tier-0 beam at slot
            // 878 (`f2a` 200) terminates on rival 557, whose `flags`
            // 0x800C carries F_REBOUND. Retail's (10,23) blast at slot
            // 279 records `f2a` 50 = 200 >> 2; the port wrote 200. The
            // blast bills 557 next tick and with the +20 regen gives
            // 10000 − 50 + 20 = 9970 (retail) vs 10000 − 200 + 20 =
            // 9820 (port) — the constant −150 standing at heads 11824,
            // 11890 and 11892.
            let payload = if no_lightning_rebound_absorb() || act != 9 {
                dmg
            } else {
                let cast_mana = self.ent[i].f140;
                let (vflags, vmana) = match victim {
                    0 => (0u32, 0i32),
                    crate::mc1::mobs::PLAYER_TARGET => {
                        let f = if self.player_rebound { 0x8000 } else { 0 }
                            | if self.mc2_rebound_precise.0 != 0 { 0x10 } else { 0 };
                        (f, ctx.pmana.min(i32::MAX as u32) as i32)
                    }
                    j if (self.ent[j as usize].class64 == 3) => {
                        let v = &self.ent[j as usize];
                        (v.flags, v.f140)
                    }
                    _ => (0, 0),
                };
                if vflags & 0x8010 == 0 {
                    dmg
                } else if vflags & 0x8000 != 0 {
                    if cast_mana / 4 > vmana { dmg } else { dmg / 4 }
                } else if cast_mana / 8 > vmana {
                    dmg
                } else {
                    dmg / 2
                }
            };
            let e = &mut self.ent[s];
            // ⭐⭐⭐ …AND THE (10,78) TAIL DOES NOT STAMP `id_0x1A_26`
            // EITHER. The enumerated block above has no `id` line, so
            // the mine keeps `NewEvent_4A050`'s OWN-SLOT default —
            // which is the whole reason `sub_10780`'s self-exclusion
            // (`a1x->id != v5x->id`, EF:3769) lets a caster's own bolt
            // reach his own mine. mc2l6-rsg t=13832 slot 767: retail
            // `f1a` **767** (its own slot) against the port's 343 (the
            // human), the same fused id that used to make the victim
            // probe skip it.
            if !bare_tail {
                e.id24 = id;
            }
            // ⭐⭐⭐ …AND THE ONE WORD IT *DOES* STAMP IS THE ONE THE
            // PORT NEVER WROTE. The very first line of that enumerated
            // block is `resultx->word_0x32_50 = a2x->id_0x1A_26`
            // (EF:59356) — the OWNER lane, sim `f52` (the importer
            // maps `r.f32` → `f52`). `sub_50840`'s ctor seeds it with
            // the mine's OWN slot (EF:36979) and this is the only
            // overwrite, which is why retail's mc2l6-rsg slot 178
            // reads `f32` **343** (the human) while the free-running
            // port read **0**.
            //
            // It is load-bearing, not cosmetic: `sub_68940`'s beacon
            // scan matches `mine->word_0x32_50 == bolt->id_0x1A_26`
            // (EF:55365) — HIS OWN mine — so with the lane unwritten
            // the beacon can never fire on a natively-laid mine and
            // every bolt the caster sends past it flies straight on.
            // (The PAIR lane hid this: the importer supplies `f52`,
            // so only free-run could see it.)
            if bare_tail && !no_mine_beacon() {
                e.f52 = id;
            }
            if !bare_tail {
                e.f30 = yaw;
                // Every impact worker stamps BOTH bearing words —
                // `v->yaw; v->pitch` (sub_65C20 EF:63194-95, sub_65820
                // EF:62990-91, CastPosses EF:63316-17); mc2l0 t=2817's
                // (10,0) records pitch 97 = the dying bolt's.
                e.f32 = pitch;
            }
            if bare_tail {
                // `resultx->byte_0x46_70 = 0; resultx->subSpellIndex =
                // a2x->subSpellIndex` (EF:59357-58) and NOTHING else —
                // no `word_0x96_150`, no `mana_0x90_144`. The mine's
                // `@0x2A` home is f44 (the lane `import_ent_mc2` seats
                // for a (10,78); `c10_2a_in_f140` excludes it), and it
                // is the TIER INDEX `sub_6CAC0` put on the carrier
                // (EF:57993), not a payload. mc2l6-rsg t=13832 slot
                // 767: retail `f2a` **1** / `mana` **0**, where the
                // generic tail wrote the tier into f140 (`mana` 1) and
                // left f44 to the arm step.
                e.f71 = 0;
                e.f44 = dmg;
            } else {
                e.f146 = leader;
                e.f140 = payload as i32; // subSpellIndex rides onto the effect
                if let Some(f) = fuse {
                    e.f71 = f; // byte_0x46_70 (EF:62994)
                }
            }
            // …and the clear itself, on the FLYER. `sub_65C20`'s mint
            // arm ends `if (!v9x) a1x->word_0x96_150 = 0;` (EF:63196;
            // 0x8A73E) — the write the `leader` arm above reads back
            // through `sub_65B50`. It belongs to `sub_65C20` alone, so
            // it fires for its two wrappers only: action 0
            // (`CastPlayerFire_65B30`) and action 29 (`sub_65B50`).
            // mc2l22 pair 47946→47947 slot 445: retail `target96` 0,
            // the port kept 181.
            if matches!(act, 0 | 29) && victim == 0 && !no_fireball_lock_clear() {
                self.ent[i].f146 = 0;
            }
        }
        // The impact XP award (`sub_6D8B0(id, spell, 1)` on a victim
        // hit — EF:63189 fireball, EF:58411 lightning, the §1.1
        // table): player casts carry their spell index in f40; the
        // world tick drains the mail into the book. Rival and
        // creature owners never award — sub_6D8B0's own guard is
        // `class == 3 && model == 0`, the HUMAN wizard only
        // (EF:58240-41); retail rivals have no spell-XP progression.
        if victim != 0 && id == PLAYER_TARGET && self.ent[i].flags & F_MC2PROJ != 0 {
            // `sub_6D8B0(id, Entities[a1x->word_0x26_38]->model, 1)`
            // (EF:62985): the lane is the TOKEN'S SLOT. The
            // `unwrap_or` keeps every pre-slot stamp (0 = no
            // back-ref, the castle turrets) reading exactly as before.
            let raw = self.ent[i].f40;
            let spell = self.mc2_token_model(raw).unwrap_or(raw);
            if spell < 26 {
                self.mc2_cast_xp.0.push((id, spell, 1));
            }
        }
        // ⭐ THE IMPACT RE-POINTS THE OWNER WIZARD AT WHAT IT STRUCK —
        // AND LEAVES THE TARGET SIGNATURE ALONE. `sub_686D0`
        // (EF:55192-55215) resolves `Entities[bolt->id_0x1A_26]`, and
        // if that owner is a wizard (class 3, model 0 or 1) writes
        // `owner->word_0x96_150 = victim`. It is the ONLY writer of
        // that word that does NOT also write `word_0x98_152` — every
        // AI picker (EF:6114/6140/6151/6174/6224/6259/6283/6332/6374/
        // 6394, the raid/attack picks EF:6224-6228 and the reactive
        // pick EF:7713) writes the pair. All four impact workers call
        // it right after their effect spawn: `sub_65820` (EF:62983),
        // `sub_65C20` (EF:63187), `sub_662E0` (EF:63549) and the
        // lightning `sub_66750` (EF:58408).
        //
        // ⭐ SO A LANDED SHOT USUALLY BRICKS THE SHOOTER'S CURRENT
        // GOAL. The AI's every target read goes through `sub_14C60`
        // (EF:5497), which compares `sub_14C40(Entities[+0x96])` —
        // id + model + (class<<7) — against the STORED `+0x98`. The
        // fresh victim almost never matches the sig of whatever the
        // picker chose, so the state handler falls out at its first
        // line: `sub_13890` (state 8) returns 0 without the tan2
        // roll, without `sub_14C90`'s approach/brake command and
        // without its z-step, leaving the wizard on the mover's bare
        // gravity (`sub_580E0`'s v_14) until the decision cadence
        // re-arbitrates. mc2l6-rival-spells-galore t=294: rival 370's
        // fireball reaches the HUMAN's castle (slot 63, id 343 model
        // 2 → sig 729) while 370 is chasing the human himself (sig
        // 727), so retail stamps 370's +0x96 = 63, freezes its roll
        // at 241 and lets z fall 332 → 328 → … → 280 for the 36
        // ticks up to the next think tick.
        //
        // ⭐ THE POSSESSION DELIVERY IS THE ONE THAT DOES NOT RE-POINT.
        // `sub_686D0` has exactly FOUR call sites (EF:58408 lightning
        // `sub_66750`, EF:62983 the generic `sub_65820`, EF:63187 the
        // fireball `sub_65C20`, EF:63549 `sub_662E0`) and NEITHER
        // possession worker is among them: `CastPosses_65F60` —
        // action **1**, the basic (9,1) bolt — runs its whole impact
        // block (EF:63304-19) as spawn → `sub_65780` → `sub_6D8B0` →
        // id/yaw/pitch copy → disable, with no re-point line at all,
        // and the leveled `sub_674C0` (action 18, EF:59029-58) is the
        // same shape twice over. So a possession hit leaves the
        // caster's `word_0x96_150` exactly where the picker put it.
        //
        // ⭐ AND THAT IS WHAT KEEPS A RIVAL'S POSSESS GOAL ALIVE. The
        // ball pick runs on the think cadence only (`sub_12E70`'s
        // `f63 % (64 − refl/4)` gate, EF:5522) while the state-6
        // handler `sub_135C0` casts every tick, so between two picks
        // a rival fires several bolts at the ball it chose. Re-pointing
        // on each hit re-targeted it at whatever the bolt struck and
        // then bricked the goal, because `sub_14C60` compares the
        // fresh victim's signature against the stored `+0x98` and the
        // state handler falls out at its first line. mc2l6-rsg t=3519
        // is the witness: rival 378 picked ball 851 at t=3507 (cadence
        // 52), its possess bolt at slot 803 struck ball 519 at 3519,
        // and retail's `target96` stays 851 through the claim pulse at
        // 3520 while the port's flipped to 519 and dropped the z servo
        // (`sub_135C0`'s ball-z+512 hover, EF:5854-64) — the t=3520
        // `(3,1)slot378:z` head, retail 132 / port 128.
        //
        // ⚠ ONE FORK STILL NOT REPRODUCED: `sub_662E0` reaches its
        // `sub_686D0` only down the branch that already required the
        // victim to be a class-3 model-0/1 wizard (EF:63541-49); the
        // other three stamp any victim. The folded seam stamps every
        // non-possession victim, which is right for the three.
        //
        // ⭐ AND NEITHER DOES THE LIGHTNING L1/L2 CARRIER. `sub_66FD0`
        // (action **12**, EF:58727) is not on that four-site list
        // either: its victim contact is `sub_68740(a1x, v6x, 0x2D, 22)`
        // (EF:58795) — mail + XP (`sub_6D8B0`) + the knock — and then
        // `return`; no `sub_686D0` anywhere in the worker. mc2l22
        // t=4689→4690: rival 584's carrier at slot 252 strikes the
        // (5,15) at 931 (retail: 252 reap-flagged, 931 mail 32→48) and
        // retail's 584 keeps `target96` 556 (its RaidCastle castle);
        // the port re-pointed 584 at 931, whose signature no longer
        // matched the stored 916, so the RaidCastle arm was skipped
        // for one tick — no aim-pitch stamp (91 vs 77), no whiff
        // hover (z 4642 vs 4646). That pair was the segment wall.
        let repoint = !matches!(act, 1 | 18 | 12);
        if repoint && victim != 0 && id != 0 && id != PLAYER_TARGET {
            let owner = id as usize;
            // The human owner is stamped by retail too, but his pool
            // record is the reserved hole here (out-of-pool carpet)
            // and nothing reads or grades a target word on it.
            if owner < self.ent.len()
                && self.ent[owner].class64 == 3
                && self.ent[owner].model65 <= 1
            {
                self.ent[owner].f146 = victim;
            }
        }
        self.ent[i].flags |= 0x400;
    }

    /// `sub_66750` (EF:58268) — the tier-0 LIGHTNING BEAM: a ONE-TICK
    /// hitscan, not a traveling ball. Retail re-aims ONCE at launch —
    /// `sub_66610` (EF:63583-99) runs the one-shot `sub_67CB0`
    /// acquisition and FULLY SNAPS yaw/pitch onto the pick — then
    /// walks a dead-STRAIGHT ray to the first blocker (no per-step
    /// homing anywhere in the beam) and detonates the `(10,23)` blast
    /// at the terminus, which for a victim hit IS the victim
    /// (position snap, EF:63604-08). The trail heading is saved AFTER
    /// the snap (EF:58306-08) — that is what makes the retail flash
    /// point at the locked target and jump target-to-target as each
    /// RAPID re-fire re-scans. The flyer core marches, probes, and
    /// applies the terrain/victim/impact law — run it to COMPLETION
    /// with the lock CLEARED so it flies straight; victim stamping
    /// rides the probe like retail's re-probe (EF:58401-21). Net:
    /// fire → instant aimed flash → gone, re-laid every RAPID tick
    /// (docs/spell-audit/lightning.md §5.A).
    pub(crate) fn mc2_lightning_beam_tick(&mut self, i: usize, ctx: &MobCtx) {
        // EF:58303 — the walk runs at minSpeed.
        self.ent[i].f126 = self.ent[i].f128;
        // The one-shot acquisition + full snap (`sub_66610`
        // EF:63586-98). A fresh cast always lands here (the a3==7
        // dispatch stamps no victim); no target = freeze the facing.
        //
        // ⭐⭐⭐ AND IT IS GATED ON HAVING NO TARGET, NOT ON A LATCH.
        // `sub_66610`'s whole acquire block sits inside
        // `if (Entities[0] >= Entities[a1x->word_0x96_150])`
        // (EF:63583) — a beam that arrives already carrying
        // `word_0x96_150` never re-aims and never even sets the
        // `byte[0] & 2` bit; it marches on the attitude its LAUNCHER
        // wrote. The port ran the acquisition off `F_AIMED` alone, so
        // a CASTLE TURRET's lightning re-acquired the same victim from
        // its own muzzle — and the muzzle is the turret's z plus the
        // sprite half-height, which `mc2_piece_fire` deliberately adds
        // AFTER the aim (EF:30296). Re-aiming from there hands back the
        // full box the turret's law had just given up.
        // mc2l6-rsg t=463: turret 2 at (61952, 23296, 2218) with `f78`
        // 100 fires on rival 383 at aim_z 932 — retail's beam keeps the
        // turret's pitch 207, the port re-acquired from z 2318 and got
        // 217. `sub_66750` builds ONE rounded step vector from that
        // pitch and ACCUMULATES it down all 49 trail nodes, so node k
        // landed exactly k units low and k units long.
        if self.ent[i].flags & F_AIMED == 0 && self.ent[i].f146 == 0 {
            self.ent[i].flags |= F_AIMED;
            if self.mc2_autoaim(i, ctx) {
                let e = &mut self.ent[i];
                e.f30 = e.f34;
                e.f32 = e.f36;
            } else {
                let e = &mut self.ent[i];
                e.f34 = e.f30;
                e.f36 = e.f32;
            }
        }
        // ⚠ AND THE LATCH STAYS OFF WHEN THE GATE REFUSED. `byte[0] |= 2`
        // lives INSIDE retail's no-target branch too, so a turret beam
        // reaches its death with the bit clear — a GRADED lane (mc2l6-rsg
        // t=463 slot 414 `flags` 1024, not 1026). The march's own re-aim
        // is suppressed by `mc2_beam_defer` instead (see
        // [`Gen::mc2_flyer_tick`]): the port stashes `f146` aside for the
        // walk, which would otherwise make the generic flyer read the
        // beam as targetless and re-acquire on its first step.
        let (sx, sy, sz, speed, id) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f126.max(384), e.id24)
        };
        // Straight march: the beam never homes per-step, so the lock
        // is held aside for the walk (F_AIMED is latched, so the
        // flyer neither re-acquires nor steers). maxLife (~9) bounds
        // the reach; the 64 cap is a pure safety backstop.
        let lock = self.ent[i].f146;
        self.ent[i].f146 = 0;
        let mut steps = 0i32;
        // ⭐⭐ THE STEP VECTOR IS BUILT ONCE AND ACCUMULATED, SO A
        // ONE-UNIT PITCH ERROR IS A ONE-UNIT ERROR **PER NODE**.
        // Retail reads `v5`/`v4` after the march, but only because it
        // restored them to the values its FIRST `sub_66610` left —
        // which, for a beam that already carries `word_0x96_150`, are
        // exactly the launcher's (that call takes the no-target gate
        // and touches neither axis). The port front-loads that first
        // acquisition into the block above, so the pre-march read here
        // is the same register.
        //
        // mc2l6-rsg t=463 lays 49 trail nodes off one turret shot, and
        // the port's `y`/`z` were off by exactly the node index — +1/−1,
        // node 3 by 3, node 27 by 27 — with `x` bit-exact throughout:
        // the signature of a slightly steeper shared delta (a larger
        // `dist·sin` and a smaller `dist·cos`, the latter too small to
        // move `x` at this yaw). pitch 205 gives retail's (10, −36, −28)
        // at spacing 48; the re-acquired 217 gives (10, −35, −29).
        let (yaw, pitch) = {
            let e = &self.ent[i];
            (e.f30, e.f32)
        };
        // The march is `sub_66610` — walk, blocker test, disable — and
        // NOTHING of the impact. Park it (see `Gen::mc2_beam_defer`).
        self.mc2_beam_defer.armed = true;
        for _ in 0..64 {
            steps += 1;
            self.mc2_flyer_tick(i, ctx);
            if self.ent[i].flags & 0x400 != 0 {
                break;
            }
        }
        self.mc2_beam_defer.armed = false;
        self.ent[i].f146 = lock;
        // Enhanced-lightning presentation feed: the resolved strike,
        // muzzle → walked terminus (hash-silent, drained by the
        // frontend).
        let end = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        if self.bolt_fx.0.len() < 256 {
            self.bolt_fx.0.push(crate::engine::features::BoltStrike {
                start: (sx, sy, sz),
                end,
                owner: id,
            });
        }
        // Lay the VISIBLE jagged flash: `sub_66750`'s cosmetic sprite-216
        // trail (EF:58320) along the AIMED heading, `steps·8` nodes at
        // `actSpeed/8` spacing (EF:58321-23) — its end coincides with
        // the walked terminus by construction. `i` is despawned here
        // but its fields are still live.
        let blast_at = self.mc2_lay_lightning_trail(i, (sx, sy, sz), steps, yaw, pitch, speed, id);
        // ⭐⭐ AND ONLY NOW THE IMPACT. mc2l3 t=18621 is the row: the
        // castle turret's beam terminates and retail allocates 148
        // records in one tick — 147 trail nodes and, LAST of all, the
        // (10,23) blast at slot 350. The port detonated during the
        // march, so the blast took the tick's FIRST pop (slot 114, the
        // record the tick-top reaper had just freed) and retail's node
        // for that slot fell off the end of the burst.
        if let Some((p, _march_victim)) = self.mc2_beam_defer.pending.take() {
            // …and at `v20x`, the position the trail loop left on the
            // node AFTER its last (EF:58401 hands `&v20x` straight to
            // the spawner). The beam record itself stays where the
            // march stopped.
            //
            // ⭐⭐⭐ THE IMPACT'S VICTIM IS A **FRESH PROBE AT THE
            // TERMINUS**, NOT THE MARCH'S CONTACT. `sub_66750` reads
            // `v13x = sub_10780(a1x)` (EF:58400) only AFTER the trail
            // loop, from the beam's committed position — the spot
            // `sub_66610`'s hit arm snapped it onto (EF:63609
            // `position = v2x->position`) — and every consumer of the
            // impact takes THAT record: `sub_65780(a1x, v13x, …)`, the
            // owner re-point `sub_686D0(a1x, v13x)` (EF:58408), the
            // mine test, the spell-7 XP (`if (v13x > Entities[0])`,
            // EF:58410-11) and the blast's own `word_0x96_150 = v13x`
            // (EF:58419). The march's blocker and the terminus probe
            // are the same predicate on the same tile chain, but they
            // run from DIFFERENT positions — the march from the ray's
            // step point, this one from the victim's exact position —
            // and `sub_10780` returns the FIRST overlapping record in
            // ring-then-chain order, so when two records overlap the
            // terminus the two probes can elect different ones. The
            // port carried the march's victim into the fold.
            //
            // WITNESS (mc2l22 t=7261, the free run's head after the
            // Upgrade gate): rival 530's tier-0 beam at slot 500,
            // homing on the dying (5,25) at 322, snaps onto it at
            // (51491, 11306, 4384); the terminus probe from there walks
            // the (5,25) at 924 FIRST (322 is that chain's tail —
            // `next16` 0, `prev18` 924) and retail's (10,23) blast at
            // slot 708 records `target96` **924**. So retail re-points
            // 530 at 924 — whose signature (1589) does not match the
            // stored 987 — and 530's Defense handler no-ops that tick
            // (roll holds 115, z takes only the mover's −4, the
            // selector's re-pick parks the command speed at 0 → 80→64).
            // The port re-pointed at 322 (signature 987 = a match), ran
            // the handler (roll 162, two z-steps −8, speed held 80).
            // Same shape as t=7713 on the same rival.
            let (bx, by, bz) = {
                let e = &self.ent[p];
                (e.x, e.y, e.z)
            };
            let hit = self.victim_scan_at(p, (bx, by, bz), ctx);
            let v = match self.mc2_proj_filter(p, hit) {
                Some(MailTarget::Pool(j)) => j as u16,
                Some(MailTarget::Player) => PLAYER_TARGET,
                None => 0,
            };
            self.mc2_proj_impact(p, v, ctx, Some(blast_at));
        }
    }

    /// `sub_66750`'s trail (EF:58320-58399): sprite-216 billboards along
    /// the beam every `actSpeed/8` (=48) units — `steps·8` of them, so
    /// the trail length equals the walked distance (EF:58321) —
    /// jittered by a ±1 random walk (amplitude clamp 8, tapering to 0
    /// at the far end). Each node is a 1-frame self-despawning
    /// class-9/model-9 billboard (action 14 = `sub_67410`). These ARE
    /// the visible flash.
    #[allow(clippy::too_many_arguments)]
    fn mc2_lay_lightning_trail(
        &mut self,
        src: usize,
        start: (u16, u16, i16),
        steps: i32,
        yaw: u16,
        pitch: u16,
        speed: i16,
        id: u16,
    ) -> (u16, u16, i16) {
        let (sx, sy, sz) = start;
        let spacing = (speed as i32 / 8).max(16); // v26 = actSpeed/8 = 48
        // ⭐ THE COUNTDOWN IS INCLUSIVE AND THE FIRST NODE SITS ON THE
        // MUZZLE. `v27` counts the march steps, is multiplied by 8, and
        // the loop runs `while ((v27 & 0x8000) == 0)` decrementing at
        // the BOTTOM — so it spawns at v27, v27−1, … 0 and stops on −1:
        // `steps * 8 + 1` nodes, the first at `v20x = v18x`, the beam's
        // own start position, BEFORE the first step is added
        // (EF:58316-19, :58398). The old `1..=steps*8` skipped the
        // muzzle node and laid one too few; the `96` ceiling stacked on
        // top of it was an invention — retail's only bound is the pool.
        let n = (steps * 8).max(0);
        let unit = (spacing / 4).max(1) as i16; // v26 >> 2 = 12
        let perp = yaw.wrapping_add(512) & 0x7FF; // HIBYTE(yaw) + 2 = +90°
        // `v22x`: the fixed per-step delta, built ONCE from the origin
        // (EF:58323) and ADDED each iteration. NOT the same as stepping
        // `k * spacing` from the start — the rounding accumulates.
        let mut delta = (0u16, 0u16, 0i16);
        Self::polar_step(&mut delta, yaw, pitch, spacing as i16);
        let (mut wz, mut wp) = (0i32, 0i32);
        let mut base = (sx, sy, sz); // predictedAxis
        let mut node = (sx, sy, sz); // v20x
        // ⚠ THE RANDOM WALK ONLY DRAWS INSIDE ITS BAND (EF:58346-58383).
        // Outside it the step is a bare ±1 back toward the band and NO
        // draw is taken; the old unconditional draw-then-clamp burned
        // two numbers per node whatever the amplitude, which
        // desynchronises the chain the moment the taper starts.
        let walk = |g: &mut Self, w: i32, amp: i32| -> i32 {
            if amp < w {
                w - 1
            } else if w < -amp {
                w + 1
            } else {
                let r = g.ent_rand(src);
                w + 2 * ((r % 0x9D) as i32 / 79) - 1
            }
        };
        for k in 0..=n {
            if let Some(nn) = self.mc2_spawn_lightning_node(node.0, node.1, node.2, src) {
                self.ent[nn].id24 = id;
            }
            // The amplitude tapers off the REMAINING count (`v27 / 2`
            // clamped to 8, EF:58346-50), so the flash narrows to a
            // point at the far end.
            let amp = ((n - k) / 2).clamp(0, 8);
            wz = walk(self, wz, amp);
            wp = walk(self, wp, amp);
            base = (
                base.0.wrapping_add(delta.0),
                base.1.wrapping_add(delta.1),
                base.2.wrapping_add(delta.2),
            );
            node = base;
            // ⚠⚠ BOTH OFFSETS ARE `v28`. EF:58390-58397 writes the z
            // lift and the perpendicular slide from the SAME walk
            // variable, and `v25` — the second walk, whose draws are
            // taken and whose value is maintained — is never read. That
            // reads like a transcription slip, and the corpus says it
            // is not: with `wp` on the perpendicular every node's `z`
            // matched and every node's `x`/`y` was wrong (mc2l3
            // t=18621, 298 field rows, all of them x/y); with `wz` on
            // both, the whole burst lands. `wp` is still WALKED because
            // its draws are in the beam's rand chain.
            let off = (wz as i16).wrapping_mul(unit);
            node.2 = node.2.wrapping_add(off);
            Self::polar_step(&mut node, perp, 0, off);
            let _ = wp;
        }
        node
    }

    /// One `sub_66750` trail billboard: class-9 model-9 sprite-216,
    /// action 14 (`sub_67410` = pure life-- decay). Born DEAD by the
    /// slot compare `maxLife = (node >= beam) - 1` (EF:58341): a node
    /// ahead of the beam's slot gets 0 (the ascending frame pass
    /// still ticks it this frame), one behind gets -1 — either way
    /// the disabled bit lands within a frame and the slot recycles
    /// on retail's schedule. No yaw write — the ctor leaves @0x1C 0.
    fn mc2_spawn_lightning_node(&mut self, x: u16, y: u16, z: i16, beam: usize) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 9;
            e.model65 = 9;
            e.tick70 = 14;
            e.max_life = if i >= beam { 0 } else { (-1i32) as u32 };
            // ⭐⭐⭐ NO FLAG-WORD WRITE — the node stays COLLIDABLE.
            // `sub_66750` (EF:58336-45) writes six fields and nothing
            // else; the `byte[0] &= 0xF7` every real bolt ctor spells
            // is absent, so a beam's trail billboards are legal
            // `sub_10780` victims. mc2l22 t=54920: a (9,3) meteor shot
            // (slot 904) homing on hydra 318 crosses node 886 at
            // (39722, 53682, 5645); retail freezes it, raises the reap
            // bit and snaps the shot onto the node's RAISED box centre
            // (+`ayaw` 25) per EF:62942-44, minting its (10,17) payload
            // there. The port's probe skipped it and flew on.
            e.flags = if no_lightning_node_collide() {
                (e.flags & !8) | F_MC2PROJ
            } else {
                e.flags | F_MC2PROJ
            };
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 216);
        Some(i)
    }

    /// `sub_67410` (EF:58906, action 14) — the inert trail-node tick:
    /// pure `life--`, despawn at `< 0`. No flight, no logic.
    pub(crate) fn mc2_lightning_node_tick(&mut self, i: usize) {
        // EF:58910-12 — the life test reads the PRE-decrement value.
        // Nodes are born at 0/-1 (the EF:58341 slot compare), so the
        // flash lives ~one frame before the disabled bit lands.
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
        }
    }

    /// `sub_65820` (EF:62882) — the shared class-9 flyer/projectile
    /// tick: per-tick homing with the behavior row's yaw/pitch caps
    /// (`sub_65610`, EF:62781 — caps v_2/v_6 via `sub_58350`), a ±2
    /// speed ramp toward minSpeed, the polar step, the tile-chain
    /// victim probe under the xtype/xsubtype filter, the per-state
    /// terrain law, the water splash, life expiry, and the
    /// (f68, f69) impact spawn.
    ///
    /// TERRAIN LAW (docs/traces/mc2-projectile-terrain-water.md):
    /// every ballistic state DETONATES on terrain contact
    /// (`getTerrainAlt > z`,
    /// EF:62950/63135 — the contact clamp only places the burst);
    /// POSSESSION (action 18) alone runs a PRE-move ground-raise
    /// (EF:63262-64) and therefore skims — and has NO water arm.
    /// The water test is NESTED inside the contact branch
    /// (EF:62956/63141): only a projectile flying AT the water
    /// surface splashes; flight over water never runs it.
    /// Would projectile `i`, standing at `at`, already be overlapping
    /// the victim the chord march just found? The muzzle-admission
    /// test (OPEN-7) — see the march in [`Self::mc2_flyer_tick`].
    fn mc2_hit_covers(
        &mut self,
        i: usize,
        at: (u16, u16, i16),
        h: MailTarget,
        ctx: &MobCtx,
    ) -> bool {
        let old = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
        self.ent[i].x = at.0;
        self.ent[i].y = at.1;
        self.ent[i].z = at.2;
        let v = match h {
            MailTarget::Pool(j) => self.ent_overlap(i, j),
            MailTarget::Player => self.player_overlap(i, ctx),
        };
        self.ent[i].x = old.0;
        self.ent[i].y = old.1;
        self.ent[i].z = old.2;
        v
    }

    /// `CastCastleProjectile_66B30` (EF:58461) + its create-arm body
    /// `sub_66D00` (EF:58556) — the MC2 (9,10) castle ball. NOT the
    /// generic flyer: it homes on the DEST POINT (create) or the
    /// bound castle entity (upgrade, `word_0x96_150` → f146), runs
    /// the castle-cast SITE TEST as a per-tick tripwire, has NO
    /// water arm (retail builds on whatever it lands on), and its
    /// landing spawns the descriptor pair (f68,f69) — (3,2) create /
    /// (10,43) upgrade — AT THE BALL'S POSITION, owner-stamped
    /// (mc2l3 t=241-244: cast at 241, ball armed-unmoved at its
    /// birth boundary, homing turn 789→810 / 205→183, terrain
    /// contact during tick 244, castle (3,2) born same tick at the
    /// landing tile (26368,49408) — the generic flyer's water arm
    /// was eating exactly this build).
    ///
    /// The ARM state (retail byte0&2 → the imported flags bit 1):
    /// the mint leaves the ball UNARMED (mc2l3 t=241's birth
    /// boundary carries flags byte0 = link alone) — the head below
    /// runs on the ball's own FIRST dispatch, one tick after the
    /// cast: arm bit + site test at the launch pose, no move
    /// (t=242), first flight step the tick after (243).
    pub(crate) fn mc2_castle_ball_tick(&mut self, i: usize) {
        let tgt = self.ent[i].f146 as usize;
        let upgrade_flight = tgt != 0 && tgt != PLAYER_TARGET as usize && tgt < self.ent.len();
        if !upgrade_flight && self.ent[i].flags & 2 == 0 {
            // sub_66D00's head: latch the arm bit, site-test the
            // launch pose, and do NOT move. A refusal despawns (the
            // sub_88D00 "can't build here" flash is app-side; the
            // cast lock is derived — `mc2_castle_lock_active`).
            self.ent[i].flags |= 2;
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            if !self.mc2_castle_cast_site_ok(x, y) {
                self.ent[i].flags |= 0x400;
            }
            return;
        }
        // ---- the shared flight: ease yaw/pitch toward the target
        // at the behavior row's caps (sub_58350's v_4 arg is dead —
        // the single-cap `turn_step` fold, the flyer's precedent),
        // ease speed ±2 toward min, one polar step. ----
        let (px, py, pz) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let (tx, ty, tz) = if upgrade_flight {
            let c = &self.ent[tgt];
            (c.x, c.y, c.aim_z())
        } else {
            let e = &self.ent[i];
            (e.dest_x, e.dest_y, e.site_z)
        };
        let tgt_yaw = Self::angle_between(px, py, tx, ty);
        let dh = Self::isqrt(Self::dist2_sq(px, py, tx, ty) as u32) as i32;
        let tgt_pitch = Self::pitch_toward(pz, tz, dh);
        let row = &BEHAVIOR[self.ent[i].row156 as usize];
        let (cy, cp) = (row.v_2, row.v_6);
        {
            let e = &mut self.ent[i];
            e.f34 = tgt_yaw;
            e.f36 = tgt_pitch;
            e.f30 = (e.f30 as i32 + Self::turn_step(e.f30, tgt_yaw, cy) as i32) as u16 & 0x7FF;
            e.f32 = (e.f32 as i32 + Self::turn_step(e.f32, tgt_pitch, cp) as i32) as u16 & 0x7FF;
            e.f126 += (e.f128 - e.f126).clamp(-2, 2);
        }
        let (yaw, pitch, speed) = {
            let e = &self.ent[i];
            (e.f30, e.f32, e.f126)
        };
        let mut pos = (px, py, pz);
        Self::polar_step(&mut pos, yaw, pitch, speed);
        let mut land = false;
        let mut refused = false;
        // Upgrade arrival: plain overlap with the target snaps the
        // ball onto it (EF:58496-99 / the sub_106C0 test).
        if upgrade_flight {
            let (ox, oy, oz) = (self.ent[i].x, self.ent[i].y, self.ent[i].z);
            self.ent[i].x = pos.0;
            self.ent[i].y = pos.1;
            self.ent[i].z = pos.2;
            let hit = self.ent_overlap(i, tgt);
            self.ent[i].x = ox;
            self.ent[i].y = oy;
            self.ent[i].z = oz;
            if hit {
                let c = &self.ent[tgt];
                pos = (c.x, c.y, c.z);
                land = true;
            }
        }
        if !land {
            // Terrain contact — floor, or on caves the ceiling at
            // ceiling − fov (EF:58637-48, the same comma arm as the
            // generic core). NO water arm (sub_66D00 has none).
            let ground = self.ground_z(pos.0, pos.1) as i16;
            if ground > pos.2 {
                pos.2 = ground;
                land = true;
            } else if self.is_cave() {
                let c = (self.ceiling_z(pos.0, pos.1) - self.ent[i].f84 as i32) as i16;
                if pos.2 > c {
                    pos.2 = c;
                    land = true;
                }
            }
        }
        if !land {
            // Airborne: life countdown, then the site tripwire
            // (create only — EF:58650-56); a refusal lands HERE with
            // the 180° back-step below.
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life < 0 {
                land = true;
            } else if !upgrade_flight && !self.mc2_castle_cast_site_ok(pos.0, pos.1) {
                land = true;
                refused = true;
            }
        }
        if refused {
            // The retreat step (EF:58662-69): re-step from the
            // committed position at yaw+0x400, live pitch and speed.
            let back = yaw.wrapping_add(0x400) & 0x7FF;
            Self::polar_step(&mut pos, back, pitch, speed);
        }
        self.move_relink(i, pos.0, pos.1, pos.2);
        if !land {
            return;
        }
        let own = self.ent[i].id24;
        let (fc, fm) = (self.ent[i].f68, self.ent[i].f69);
        // The stale-create guard (EF:58528-31): a (3,2) delivery
        // whose owner already holds a BOUND castle just despawns.
        if fc == 3 && self.mc2_castle_of(own).is_some() {
            self.ent[i].flags |= 0x400;
            return;
        }
        // `_4A190(&pos, byte67, byte68)` — the build. A pool-refused
        // spawn leaves the ball ALIVE to retry next tick (EF:58540-42
        // releases the caster's lock instead; ours is derived).
        let spawned = match (fc, fm) {
            (3, 2) => self.spawn_castle(pos.0, pos.1),
            (10, 43) => self.spawn_creator(43, pos.0, pos.1, pos.2),
            _ => None,
        };
        if let Some(c) = spawned {
            self.ent[c].id24 = own;
            self.ent[i].flags |= 0x400;
        }
    }

    /// The flyer's homing-target read — `Entities_EA3E4[@0x96] >
    /// Entities_EA3E4[0]` and NOTHING ELSE. All three flyer entry
    /// points spell the gate identically before handing the record to
    /// the `sub_65610` servo: `sub_65820` (EF:62899, the shared core),
    /// `sub_65C20` (EF:63084, the state-0 body) and
    /// `CastPosses_65F60` (EF:63241) — and `sub_65B50` reaches it
    /// through `sub_65C20`. It is a POINTER comparison against the
    /// sentinel record, so it reduces to `slot != 0`: no life test, no
    /// class test, no reap-mark test.
    ///
    /// That makes the read IDENTITY-BLIND, the same law the balloon's
    /// `AddBallon_60AB0` carries (2026-08-23) and the same reason a
    /// freed MC2 slot is not an empty slot — a target that DIED
    /// earlier in this very walk still steers the shot. The shared
    /// [`Gen::mc2_target`] cannot serve here: its `class64 == 0 ||
    /// act_life < 0 || flags & 0x400` guards belong to the creature
    /// and wizard AI sites that do re-test their quarry.
    ///
    /// Corpus row mc2l0 t=3999: the (9,0) at slot 149 homes on the
    /// (5,4) archer at slot 142, which dispatches FIRST in the same
    /// walk and dies there (life 0 → −250). Retail re-bears anyway —
    /// desired yaw 814 → 816, yaw 806 → 811 — where the port's
    /// life-guarded read dropped to the one-shot arm and flew on
    /// frozen.
    fn mc2_flyer_target(&self, slot: u16, ctx: &MobCtx) -> Option<(u16, u16, i16)> {
        if slot == PLAYER_TARGET {
            return Some((ctx.px, ctx.py, ctx.pz));
        }
        let j = slot as usize;
        if j == 0 || j >= self.ent.len() {
            return None;
        }
        let t = &self.ent[j];
        Some((t.x, t.y, t.z))
    }

    pub(crate) fn mc2_flyer_tick(&mut self, i: usize, ctx: &MobCtx) {
        // Homing / acquisition (EF:62902-21).
        match self.mc2_flyer_target(self.ent[i].f146, ctx) {
            Some((tx, ty, tz)) => {
                // `sub_65610` steers at the target RAISED to its z-box
                // CENTER (`sub_65580` EF:62750: z += f78 unless MODEL
                // 2 — [`Ent::aim_z`]; `model_0x40_64` IS the model
                // byte, per its own value key "2 - castle": castles
                // home at the FLAG, not 8192 under the base; restored
                // by `sub_655A0` after). The acquisition sites apply
                // the same raise. Without it the meteor aims a
                // half-box low every homing tick and grazes under
                // small high-altitude flyers. The PLAYER is a raised
                // victim too — retail's player is a boxed pool wizard
                // and `sub_65580` lifts it like any other; the
                // pose-only player's box center is pz + PLAYER_HH.
                let target = self.ent[i].f146;
                let tz = if target == PLAYER_TARGET {
                    tz + crate::mc1::combat::PLAYER_HH as i16
                } else {
                    self.ent[target as usize].aim_z()
                };
                let e = &self.ent[i];
                let (yaw, pitch) = (e.f30, e.f32);
                let f34 = Self::angle_between(e.x, e.y, tx, ty);
                let dh = Self::isqrt(Self::dist2_sq(e.x, e.y, tx, ty) as u32) as i32;
                let f36 = Self::pitch_toward(e.z, tz, dh);
                let row = &BEHAVIOR[e.row156 as usize];
                let (cy, cp) = (row.v_2, row.v_6);
                if std::env::var("MGC_H1_TRACE").ok().is_some_and(|v| v == i.to_string()) {
                    let e = &self.ent[i];
                    eprintln!(
                        "H1 slot={i} tgt={} self=({},{},{}) t=({tx},{ty},{tz}) dh={dh} f34={f34} f36={f36} cy={cy} cp={cp} pitch={pitch} row={}",
                        e.f146, e.x, e.y, e.z, e.row156
                    );
                }
                let e = &mut self.ent[i];
                e.f34 = f34;
                e.f36 = f36;
                e.f30 = (yaw as i32 + Self::turn_step(yaw, f34, cy) as i32) as u16 & 0x7FF;
                e.f32 = (pitch as i32 + Self::turn_step(pitch, f36, cp) as i32) as u16 & 0x7FF;
            }
            None => {
                // ⚠ `mc2_beam_defer.armed` = a lightning beam mid-march
                // with its lock stashed aside. Retail's per-step worker
                // there is `sub_66610`, whose acquire arm is gated on
                // `word_0x96_150 == 0` and so never fires for a beam
                // that arrived with a target; the port must not let the
                // stashed lock read as "targetless" and re-aim.
                if self.ent[i].flags & F_AIMED == 0 && !self.mc2_beam_defer.armed {
                    self.ent[i].flags |= F_AIMED;
                    // One-shot acquisition (`sub_67CB0`): the
                    // FIREBALL states nudge yaw ≤34 units toward the
                    // lock and snap pitch (EF:63106-19, the
                    // "assisted not locked" launch feel —
                    // docs/traces/mc2-mouse-aim.md §5; action 29 =
                    // the charged body, same law, provenance OPEN);
                    // every other state snaps both axes (the generic
                    // init law, EF:62907-13). No target = snapshot and
                    // fly straight (the retail else-arm).
                    //
                    // ⭐⭐⭐ THE MAGIC MINE BEACON RUNS FIRST. Retail
                    // spells the one-shot as
                    // `if (sub_68940(a1x) || sub_67CB0(a1x))` at
                    // EVERY flyer entry the port folds into this tick:
                    // `sub_65820` EF:62907 (the shared core),
                    // `sub_662E0` EF:63450, `sub_66610` EF:63589; and
                    // `sub_65C20` EF:63093 forks the SAME two arms with
                    // the identical 34-cap body, so the fold is exact.
                    // `sub_68940` bends a wizard-owned bolt onto HIS
                    // OWN armed (10,78) inside a ±0xAA yaw cone —
                    // 2.4× `sub_67CB0`'s 0x71 — which is the whole
                    // point of the Magic Mine spell and why the beacon
                    // wins locks the generic scan cannot even see.
                    // [`Gen::mc2_mine_beacon`] carries the gates.
                    if self.mc2_mine_beacon(i, ctx) || self.mc2_autoaim(i, ctx) {
                        let (yaw, dy, dp, act) = {
                            let e = &self.ent[i];
                            (e.f30, e.f34, e.f36, e.tick70)
                        };
                        let e = &mut self.ent[i];
                        if matches!(act, 0 | 29) {
                            // NO 11-bit mask here. `sub_65610`'s servo
                            // masks both axes (`HIBYTE(v5) &= 7u`
                            // EF:62798, `HIBYTE(v7) &= 7u` EF:62803);
                            // `sub_65C20`'s one-shot arm writes the sum
                            // RAW (`v4 = v3 * sub_582F0(yaw, roll) +
                            // yaw; a1x->yaw_0x1C_28 = v4;` EF:63117-19,
                            // same in the shielded arm at EF:63100-02).
                            // The mask was carried across from the
                            // servo. The fireball's yaw legally leaves
                            // 0..2047 and the capture records it:
                            // mc2l3 t=14913 slot 19 retail 2054
                            // (= 2048 + 6) where the masked port wrote
                            // 6, plus seven rows where retail is
                            // NEGATIVE as i16. Every consumer masks on
                            // read (`polar_step`, `angdist`, `arc_err`,
                            // the app's ghost draw) and the MC2
                            // importer stores `r.yaw` raw, so the round
                            // trip closes — the same RAW convention the
                            // MC1 (10,39) magnet aim already uses.
                            e.f30 = (yaw as i32 + Self::turn_step(yaw, dy, 34) as i32) as u16;
                        } else {
                            e.f30 = dy;
                        }
                        e.f32 = dp;
                    } else {
                        let e = &mut self.ent[i];
                        e.f34 = e.f30;
                        e.f36 = e.f32;
                    }
                }
            }
        }
        // Speed ramp toward minSpeed (EF:62923-31) — the shared
        // `sub_65820` core only. States 0 (`sub_65C20`, moves at
        // actSpeed verbatim EF:63126), 1 (`CastPosses_65F60`,
        // EF:63261), 18 (`sub_674C0`, EF:58991) and 29 (`sub_65B50`
        // = a charged-impact wrapper over the state-0 body,
        // EF:63023) have NO ramp line; folding them into this tick
        // must not synthesize one.
        // Corpus: retail (9,0)/(9,1) speed holds constant across
        // whole flights while the ramp pulled the port toward 384
        // (the mc2 takes' +2/−2 speed family, sign = speed vs 384).
        // ⭐ 18 IS THE SAME FAMILY AS THE RE-POINT ABOVE — BOTH
        // POSSESSION WORKERS ARE THE ODD ONES OUT. `sub_674C0`'s only
        // `actSpeed_0x82_130` reference in the whole body is the
        // `MoveEntity_57FA0` argument (EF:58991); the ±1/×2 fixup
        // that opens `sub_662E0` (EF:63464-72) and the generic core
        // is simply absent. mc2l6-rsg t=3543 is the witness: the
        // human's leveled (9,17) is born at slot 807 with speed 368
        // against a minSpeed of 384 and retail keeps 368 on its
        // first flight tick where the port ramped to 370 — a 2-unit
        // step that shows up as the x row 220.816 / 220.824.
        if !matches!(self.ent[i].tick70, 0 | 1 | 18 | 29) {
            let e = &mut self.ent[i];
            if e.f126 < e.f128 {
                e.f126 += 2;
            } else if e.f126 > e.f128 {
                e.f126 -= 2;
            }
        }
        // Polar step + victim probe. Retail's probe (`sub_10780`,
        // EF:3739) ray-marches the MAP CELLS along the flight — an
        // end-point-only test TUNNELS at cast speeds (the boost
        // clamp allows up to 0x2000/tick, and several projectile
        // sprites carry a zero-width box, e.g. the fireball's row
        // 340 speed_6 = 0). March the chord in ≤128-unit sub-steps
        // and probe each; the movement itself stays the single polar
        // step (trajectory unchanged).
        // Possession rides the CLAIM probe `sub_108B0`
        // (`claim_victim_scan`) instead of the generic `sub_10780` —
        // it detonates only on claimable targets (mana spheres,
        // possessable buildings, worms) and flies through everything
        // else (the un-possessable factory sinks / spires). Every
        // other spell uses the generic any-solid probe. This is the
        // ONE player spell with the whitelist behavior, and
        // `sub_108B0` has exactly TWO callers, both possession:
        // `CastPosses_65F60` (action **1**, the basic (9,1) bolt,
        // EF:63285) and `sub_674C0` (action 18, the leveled (9,17),
        // EF:59003). Action 1 was missing from this gate, so every
        // basic possession bolt — including the (9,1)s the corpus
        // importer replays — ran the generic any-solid probe and
        // detonated on the first thing it grazed: the mc2l30 (10,12)
        // claim-pulse family came out 714 extra against retail's 258
        // (and the port's bolt skipped the skim clamp below, so its
        // z ran high the whole flight).
        let is_possess = matches!(self.ent[i].tick70, 1 | 18);
        let e = &self.ent[i];
        let start = (e.x, e.y, e.z);
        let mut pos = start;
        Self::polar_step(&mut pos, e.f30, e.f32, e.f126);
        let dx = pos.0.wrapping_sub(start.0) as i16 as i32;
        let dy = pos.1.wrapping_sub(start.1) as i16 as i32;
        let dz = (pos.2 as i32) - (start.2 as i32);
        let dist = Self::isqrt((dx * dx + dy * dy) as u32) as i32;
        // Under `strict` (conformance replay) the march comes out and
        // the probe is retail's single endpoint test — `sub_65C20`
        // EF:63126-29 is MoveEntity → CopyEntityPosition → sub_10780,
        // once, at the committed position. The anti-tunnel deviation
        // and `probe_window`'s MC2 square are ONE compensating family
        // (DEVIATIONS.md) and come out TOGETHER: `n = 1` here, the
        // ring window there. mc2l0 t=3992 is the corpus row — a (9,0)
        // fireball whose chord grazes the (5,4) archer at slot 142
        // mid-step while neither endpoint overlaps it, so the port
        // burst where retail flew past and terrain-contacted at 3993.
        let n = if ctx.strict {
            1
        } else {
            ((dist + 127) / 128).max(1)
        };
        // Possession probes ONCE, at the committed endpoint, after
        // the skim clamps — retail's order is clamp → commit →
        // sub_108B0 (EF:63262-88). No march: every claim target
        // carries a real box (buildings ±2048), so the anti-tunnel
        // march — the documented deviation for zero-width GENERIC
        // sprite boxes — has nothing to close here, and marching
        // would admit mid-chord claims retail's single endpoint
        // probe never sees. The claim scan itself walks the retail
        // ring (see `claim_victim_scan`), not the march's square.
        if is_possess {
            let g = self.ground_z(pos.0, pos.1) as i16;
            if pos.2 < g {
                pos.2 = g;
            }
            // ⭐⭐⭐ THE CEILING PRE-CLAMP IS ACTION 1's ALONE — THE
            // LEVELED (9,17) DOES NOT HAVE IT. `CastPosses_65F60`
            // (action 1) spells both clamps before the commit:
            //   v3 = getTerrainAlt(&predicted);
            //   if (v3 > predicted.z) predicted.z = v3;        EF:63262-64
            //   if (isCaveLevel) { v4 = sub_10C60(&predicted)
            //                          - array_0x52_82.fov;
            //                      if (v4 < predicted.z)
            //                          predicted.z = v4; }      EF:63265-70
            // `sub_674C0` (action 18) has ONLY the floor arm —
            //   v3 = getTerrainAlt(&predictedAxis);
            //   if (v3 > predictedAxis.z) predictedAxis.z = v3;  EF:58993-95
            //   CopyEntityPosition_57CF0(a1x, &predictedAxis);   EF:58996
            // — an ENUMERATED absence: there is no `isCaveLevel` line
            // anywhere between the move and the commit. The ceiling
            // reaches action 18 only through the POST-move contact test
            // (EF:59008-19), whose FIRST disjunct `terrainAlt > pos.z`
            // is already false after the floor pre-clamp, so the cave
            // arm evaluates and the CEILING WINS.
            //
            // Pre-clamping it here inverted that: the port left the
            // bolt at the ceiling, and `mc2_proj_land`'s contact test —
            // where the FLOOR arm is tried first — then saw `z < ground`
            // and re-raised it to the floor. mc2l15 pair 6996→6997
            // slot 130: floor 5184, ceiling−fov 5034; retail parks the
            // dying (9,17) at **5034** and the port at 5184.
            let ceil_preclamp = self.ent[i].tick70 == 1 || no_a18_ceiling_preclamp();
            if ceil_preclamp && self.is_cave() {
                let c = (self.ceiling_z(pos.0, pos.1) - self.ent[i].f84 as i32) as i16;
                if pos.2 > c {
                    pos.2 = c;
                }
            }
            let hit = self.claim_victim_scan_at(i, pos);
            return self.mc2_proj_land(i, ctx, start, pos, hit);
        }
        // MUZZLE ADMISSION (fools-mana.md OPEN-7). The march is ours,
        // not retail's, and it can see something retail's single
        // end-of-step probe never can: an entity the projectile is
        // ALREADY inside at the START of the step. Retail resolves
        // such an entity at the step's END or not at all — if it had
        // been overlapping at the end of the PREVIOUS step, the
        // previous probe would have consumed the shot — so a victim
        // that already contains the launch point is admitted only at
        // `k == n`, retail's own probe point (EF:63126-29: MoveEntity
        // → CopyEntityPosition → sub_10780, once). Everything the
        // projectile ENTERS mid-chord still detonates at the sub-step,
        // which is the whole point of the anti-tunnel march.
        //
        // Nothing in the corpus exercises this: every launcher stamps
        // an owner the probe's `id24` gate already drops. It closes a
        // latent class — a projectile born co-located with a
        // targetable entity it does not own detonating on tick 1.
        let admit_muzzle = !no_muzzle_admission();
        let mut scanned = None;
        for k in 1..=n {
            let sub = (
                start.0.wrapping_add((dx * k / n) as u16),
                start.1.wrapping_add((dy * k / n) as u16),
                (start.2 as i32 + dz * k / n) as i16,
            );
            let found = self.victim_scan_at(i, sub, ctx);
            if admit_muzzle
                && k < n
                && let Some(h) = found
                && self.mc2_hit_covers(i, start, h, ctx)
            {
                continue; // retail probes this one at the endpoint
            }
            scanned = found;
            if scanned.is_some() {
                pos = sub;
                break;
            }
        }
        let hit = self.mc2_proj_filter(i, scanned);
        self.mc2_proj_land(i, ctx, start, pos, hit);
    }

    /// The shared landing tail of the MC2 flight: rebound gate,
    /// terrain/water contact, life countdown, impact/expiry. `hit`
    /// arrives ALREADY filtered — the xtype/xsubtype narrowing is
    /// retail's, but it lives INSIDE `sub_10780` (EF:3765-68);
    /// `sub_108B0` has no such filter (EF:3820-70, whitelist only),
    /// so the possession caller passes its claim hit through raw —
    /// the basic (9,1) bolt carries `xtype = 10` from its ctor
    /// (EF:34775), and running the generic filter over a CLAIM hit
    /// would swallow worm (5,22) and building (10,45) claims retail
    /// delivers.
    fn mc2_proj_land(
        &mut self,
        i: usize,
        ctx: &MobCtx,
        start: (u16, u16, i16),
        mut pos: (u16, u16, i16),
        hit: Option<MailTarget>,
    ) {
        let is_possess = matches!(self.ent[i].tick70, 1 | 18);
        // The Rebound gate: a shielded victim throws the bolt back at
        // its shooter — no impact, it flies on reversed. The window is
        // the WORKER's, not the tail's: actions 0 and 29 are
        // `sub_65C20`'s body (29 opens by calling it) and pass
        // `(0x5B, 45)` at EF:63162; everything else folded into this
        // tail is the shared `sub_65820` core at EF:62939, `(0x2D, 22)`.
        // Same split as the speed-ramp exclusion below.
        let window = if matches!(self.ent[i].tick70, 0 | 29) {
            (0x5B, 45)
        } else {
            (0x2D, 22)
        };
        if let Some(h) = hit
            && self.mc2_rebound_deflect(i, h, ctx, window)
        {
            return;
        }
        // ⭐⭐⭐ A SHIELDED VICTIM SWALLOWS THE HIT WHOLE — AND THE
        // FIREBALL WORKER'S REFUSED LEG IS A NO-OP WHERE ITS SIBLING'S
        // FALLS THROUGH TO THE STRIKE. Retail's strike arm is the
        // `else` of the `word[0] & 0x8010` WINDOW test, not the `else`
        // of `sub_68740`, and the two workers this tail serves disagree
        // about what a REFUSED deflection does. Read out of the shipped
        // `NETHERW.EXE` (file = runtime + 0x24800; `sub_65820` = file
        // 0x8A020, `sub_65C20` = file 0x8A420):
        //
        //   sub_65820 (the generic core, EF:62939-46 — window (0x2D, 22))
        //     8a12a  66 f7 40 0c 10 80  testw $0x8010,0xc(%eax)
        //     8a130  74 16              je    0x8a148   ; unshielded → STRIKE
        //     8a138  e8 03 2e 00 00     call  0x8cf40   ; sub_68740
        //     8a140  84 c0              test  %al,%al
        //     8a142  0f 85 d2 01 00 00  jne   0x8a31a   ; DEFLECTED → return 0
        //     8a148  …                  ; REFUSED FALLS THROUGH → STRIKE
        //
        //   sub_65C20 (the fireball body, EF:63160-63 — window (0x5B, 45))
        //     8a59f  66 f7 40 0c 10 80  testw $0x8010,0xc(%eax)
        //     8a5a5  74 1d              je    0x8a5c4   ; unshielded → STRIKE
        //     8a5ad  e8 8e 29 00 00     call  0x8cf40   ; sub_68740
        //     8a5b5  84 c0              test  %al,%al
        //     8a5b7  0f 84 f2 00 00 00  je    0x8a6af   ; REFUSED → the v20 join
        //     8a5bd  31 c0 / e9 …       xor %eax,%eax; return 0  ; DEFLECTED
        //     8a5c4  …                  ; the STRIKE arm sets v20 = 1
        //   and the join is `8a6af  80 7d fc 00  cmpb $0x0,-0x4(%ebp)` /
        //   `8a6b3  0f 84 9d 00 00 00  je 0x8a756` — v20 is still the
        //   prologue's 0 on the refused leg, so the whole impact block
        //   is skipped. The `if (!v8x)` terrain/water/life-countdown arm
        //   is skipped too (it is the no-victim branch), so the bolt
        //   keeps its life, its reap bit and its heading and simply
        //   flies on from the position the move already committed.
        //   ⭐ The polarity is INVERTED between the siblings — `jne` to
        //   the return in one, `je` past the impact in the other — which
        //   is exactly how the fused port arm came to be.
        //
        // The port ran `mc2_rebound_deflect` and, on ANY false, struck.
        // But that helper folds retail's THREE refusal points into one:
        // the WINDOW test (retail's, at the call site ⇒ strike), and
        // `sub_68740`'s own whitelist and mana gate (⇒ no-op here). So
        // the window is re-tested at the call site, where retail has it.
        //
        // WITNESS mc2l22 pair 28862→28863: the human's (9,0) fireball at
        // slot 135 (`mana` 50000, `target96` 557) probes onto rival
        // wizard 557 — flags 0x40800C, so `word[0] & 0x8010` = 0x8000 —
        // and 557's purse holds 9200 against the 12500 quarter-cost, so
        // `sub_68740` refuses. Retail leaves 135 at its plain moved
        // (18615, 31170, 4686) with `life` UNCHANGED at 12 and flies on;
        // the port struck, snapped onto 557, minted a (10,0) fire and
        // rotated every later allocation that tick — the census's
        // `extra in port: slot 384 (9,13)` + `slot 385 class 9/10` rows.
        //
        // ⚠ `& 0x8010`, not the helper's `& 0x8000`: retail's window is
        // both bits and the port's `0x10` (REBOUND tier-1 PRECISE) has
        // no writer anywhere in the tree yet (the standing `OPEN:` note
        // on `mc2_rebound_deflect`), so the two are identical TODAY and
        // this site is already right for the day that writer lands.
        if let Some(h) = hit
            && matches!(self.ent[i].tick70, 0 | 29)
            && !no_mc2_shield_swallow()
            && match h {
                MailTarget::Player => self.player_rebound,
                MailTarget::Pool(j) => self.ent[j].flags & 0x8010 != 0,
            }
        {
            // Retail committed the move before `sub_10780`
            // (EF:63125) and never touches the record again on this
            // leg; the port commits its landings, so commit here.
            self.move_relink(i, pos.0, pos.1, pos.2);
            return;
        }
        // ⭐⭐ THE FIREBALL WORKER CLOBBERS ITS OWN PAYLOAD ON A STRIKE,
        // AND ITS SIBLING DOES NOT. `sub_65C20`'s strike arm — the ELSE
        // of the `word[0] & 0x8010` rebound test — opens with
        //     if (v8x->dword_0xA0_160x->byte_160_0x20_32 & 0x10)
        //         a1x->subSpellIndex_0x2A_42 = 1;          EF:63166-67
        // BEFORE `sub_65580`, so the impact tail's
        // `v18x->subSpellIndex = a1x->subSpellIndex` (EF:63188) hands
        // the spawned effect a payload of ONE. The generic core
        // `sub_65820`, which this same tail also serves, has NO such
        // line (EF:62938-46) — a SIBLING SPLIT the port fused.
        // VERIFIED in the shipped NETHERW.EXE (`sub_65C20` = file
        // 0x8A420; bytes read by the main session 2026-09-04):
        //   8a59f  66 f7 40 0c 10 80   testw  $0x8010,0xc(%eax)
        //   8a5a5  74 1d               je     0x8a5c4
        //   8a5c4  8b 80 a0 00 00 00   mov    0xa0(%eax),%eax
        //   8a5ca  f6 40 20 10         testb  $0x10,0x20(%eax)
        //   8a5ce  74 06               je     0x8a5d6
        //   8a5d0  66 c7 43 2a 01 00   movw   $0x1,0x2a(%ebx)
        // EF:63167 is the ONLY reader of that bit in the decompile.
        // mc2l6-rsg t=28338: rival 378's (9,0) bolt at slot 171
        // (subSpell 250) strikes the human's row-84 (5,16) at slot 172
        // and retail's own `explain` records `f2a 250 -> 1`, so the
        // (10,0) fire it mints carries 1 and bills the human 1/tick
        // where the port carried the whole 250 — the constant -249 on
        // `slot 343 life` behind all 21 of that take's human-pose
        // census segments. Ledger ROUND 99 dig 99-9.
        // ⚠ BANKED, UNSETTLED: the gate is `flags & 0x10`, and the
        // port's BEHAVIOR table and the shipped EXE image AGREE on row
        // 84 but DISAGREE from row ~108 onward (the port has 0x10 on
        // row 133, the EXE image on 109/111/115/119/124/150). Only row
        // 84 is assigned by any ctor today, so the difference is inert
        // — but the table divergence itself is an open lead (round 98's
        // "the table the decompile shows is not the table the game
        // runs" class: `frames89` was filled at boot from the assets).
        if !no_mc2_strike_subspell_1()
            && matches!(self.ent[i].tick70, 0 | 29)
            && let Some(MailTarget::Pool(v)) = hit
            && crate::mc2::behavior::BEHAVIOR
                .get(self.ent[v].row156 as usize)
                .is_some_and(|b| b.flags & 0x10 != 0)
        {
            self.ent[i].f44 = 1;
        }
        // The MAGIC-MINE CARRIER's own worker (`sub_67960`, EF:59240)
        // is not the shared core — see the countdown note below.
        let mine_carrier = self.ent[i].tick70 == 30 && !no_mine_carrier();
        if hit.is_none() {
            let ground = self.ground_z(pos.0, pos.1) as i16;
            // Terrain CONTACT — floor, or on caves the CEILING at
            // ceiling − fov (the comma arm at EF:62951-53/63136-38/
            // 63281-88; floor wins when both, sealed-gap case). A
            // fireball reaching the ceiling detonates exactly as if
            // it hit ground. Possession's post-move test is the same
            // law (EF:63279-90) — after its pre-clamps it only fires
            // across a sealed gap.
            let contact_z = if pos.2 < ground {
                Some(ground)
            } else if self.is_cave() {
                let c = (self.ceiling_z(pos.0, pos.1) - self.ent[i].f84 as i32) as i16;
                (pos.2 > c).then_some(c)
            } else {
                None
            };
            // ⭐ THE LIGHTNING BEAM'S MARCH HAS NO CONTACT ARM AT ALL.
            // `sub_66610`'s whole terrain test is `if (terrainAlt >
            // pos.z) v7 = 1;` followed by `DisableEntityDrawing04`
            // (EF:63612-26) — no clamp, no x/y revert, no water splash.
            // The beam is LEFT at the overshot position, under the
            // ground it ran into: mc2l3 t=18621, retail's beam parks at
            // z −220 where the terrain reads 9. Clamping it was the last
            // field row of that burst.
            if contact_z.is_some() && self.mc2_beam_defer.armed {
                self.move_relink(i, pos.0, pos.1, pos.2);
                self.mc2_beam_defer.pending = Some((i, 0));
                self.ent[i].flags |= 0x400;
                return;
            }
            if let Some(cz) = contact_z {
                // Clamp z to PLACE the burst — offensive projectiles
                // never skim. The GENERIC core keeps the post-move
                // x/y under the clamped z (sub_65820 EF:62954), but
                // the FIREBALL body (sub_65C20, actions 0/29 — both
                // its dispatch wrappers, EF:63009/63023) commits the
                // saved PRE-move axis instead (`v16x`, EF:63139-40):
                // the burst REVERTS the dying move, x/y = tick entry,
                // z = the contact read at the post-move cell (mc2l0
                // t=2817 slot 172: retail parks at (14878,6442) 3083
                // where the port flew the full final step).
                pos.2 = cz;
                if matches!(self.ent[i].tick70, 0 | 29) {
                    pos.0 = start.0;
                    pos.1 = start.1;
                }
                // Water tile, nested in the contact branch
                // (EF:62956/63141): (10,5) splash, owner inherited,
                // despawn — no impact effect, no XP. Model gate: the
                // fireball/lightning states exempt 4 only, the
                // generic exempts {4,22,24,26}; models 22/24/26 fly
                // only generic states, so one set serves all.
                // Possession has NO water arm at all (EF:63279-95).
                if !is_possess
                    && !matches!(self.ent[i].model65, 4 | 22 | 24 | 26)
                    && self.cap_bit(pos.0, pos.1) == 1
                {
                    let own = self.ent[i].id24;
                    // ⭐ THE BURST POSITION IS ALREADY COMMITTED WHEN
                    // THE WATER TEST RUNS. Retail writes it BEFORE the
                    // tile read — the fireball body copies the whole
                    // clamped axis (`v16x.z = predictedAxis.z;
                    // CopyEntityPosition_57CF0(a1x, &v16x)`,
                    // EF:63139-40) and the generic core writes the z in
                    // place (`a1x->position_0x4C_76.z =
                    // predictedAxis_EB398ar.z`, EF:62954) — and the
                    // water arm BELOW it (EF:63141-47 / 62955-61) only
                    // spawns the splash and disables drawing. It never
                    // moves the projectile again, so a DROWNED bolt is
                    // LEFT at (tick-entry x/y, terrain z at the
                    // post-move cell). We returned without committing
                    // anything, so every splashing (9,0) kept its
                    // TICK-ENTRY z — and x/y matched only by accident,
                    // because retail's revert lands on the values we
                    // never left. mc2l3 t=879 slot 254 retail 0 / port
                    // 50; t=2431 slot 209 retail 453 / port 221 — the
                    // clamp is not always downward, it is the terrain
                    // read at the cell the bolt flew INTO. Placed
                    // BEFORE the splash to keep retail's order
                    // (EF:63140 then :63143): the splash links into the
                    // same cell chain.
                    self.move_relink(i, pos.0, pos.1, pos.2);
                    if let Some(s) = self.mc2_spawn_splash(pos.0, pos.1, pos.2) {
                        self.ent[s].id24 = own;
                    }
                    self.ent[i].flags |= 0x400;
                    return;
                }
                // ⭐ ACTION 30 DECREMENTS ON THE CONTACT TICK TOO.
                // The (9,29) magic-mine carrier does NOT run the
                // shared core: its own worker `sub_67960` (EF:59240,
                // str90 row 0x1E → 0x00248960) spells the terrain test
                // and the countdown as two STATEMENTS, not two arms —
                //     if (terrain) { v18 = 1; pos.z = predicted.z; }
                //     a2x->life_0x8 = a2x->life_0x8 - 1;   EF:59347-48
                //     if (a2x->life_0x8 < 0) v18 = 1;      EF:59349-50
                // where `sub_65820` (EF:62947-70) and `sub_65C20`
                // (EF:63131-56) both hang the decrement off the ELSE of
                // the contact test. So a carrier that lands still pays
                // its tick. mc2l6-rsg pair 13050→13051 slot 737: the
                // human's mine carrier terrain-contacts and retail
                // records life 5 → **4** where this tail left 5.
                if mine_carrier {
                    self.ent[i].act_life -= 1;
                }
                // Dry contact → fall through to the impact block
                // (v14/v20 = 1, EF:62964/63157).
            } else {
                // No contact: life countdown (EF:62966-70).
                self.ent[i].act_life -= 1;
                if self.ent[i].act_life >= 0 {
                    self.move_relink(i, pos.0, pos.1, pos.2);
                    return;
                }
            }
        }
        // Impact / expiry: land on the victim, spawn the effect.
        let victim = match hit {
            Some(MailTarget::Pool(v)) => {
                // Land at the victim's z-box CENTER, not its origin
                // (`sub_65580` raise → CopyEntityPosition → `sub_655A0`
                // restore, EF:62941-43): the impact effect spawns
                // where the box actually is, so its area write
                // (`sub_10C80`'s 3-D window) reaches the victim. At
                // the raw origin a tall-offset flyer (wyvern f78 ≈
                // 937 retail-derived) sat entirely above its own
                // burst. Model-2 exempt ([`Ent::aim_z`]) like every
                // sub_65580 site — castle hits land at the flag.
                //
                // ⚠ EXCEPT UNDER THE BEAM. The lightning walk's own
                // per-step worker is `sub_66610`, and its blocker arm is
                // a BARE `a1x->position_0x4C_76 = v2x->position_0x4C_76`
                // (EF:63605-08) — no `sub_65580` bracket anywhere in the
                // function, so the beam stops at the victim's RAW
                // origin. mc2l6-rsg t=463 slot 414: retail's beam ends
                // at rival 383's own 832, the raised 932 is this arm's.
                let (vx, vy, vz) = {
                    let t = &self.ent[v];
                    let tz = if self.mc2_beam_defer.armed {
                        t.z
                    } else {
                        t.aim_z()
                    };
                    (t.x, t.y, tz)
                };
                self.move_relink(i, vx, vy, vz);
                v as u16
            }
            Some(MailTarget::Player) => {
                // The player is a raised victim too (see the Pool arm:
                // retail's player wizard gets the same `sub_65580`
                // lift) — land at the box center so the burst's area
                // window brackets the player.
                //
                // ⚠ AND THE BEAM EXEMPTION IS THE POOL ARM'S, VERBATIM.
                // `sub_66610`'s blocker arm is one bare
                // `a1x->position_0x4C_76 = v2x->position_0x4C_76`
                // (EF:63605-08) and it does not ask who `v2x` is — the
                // human's wizard record is just another pool record
                // there, so a beam that stops on the PLAYER parks at
                // his raw origin exactly as it does on a rival.
                // The port carried the raise on this arm only, and the
                // whole `+PLAYER_HH` rode straight into the recorded
                // beam position.
                // mc2l22 t=9994: rival 530's tier-0 lightning at slot
                // 944 snaps onto the human and retail records z 5015 —
                // the carpet record's own z that tick — where the port
                // wrote 5115. Same row at t=9999/10000/10002/10006
                // (slots 892/888/872/980), 13666, 22166, 52931, 53161,
                // 53946 and 53948: eleven segment heads, one lift.
                let pz = if self.mc2_beam_defer.armed {
                    ctx.pz
                } else {
                    ctx.pz.wrapping_add(crate::mc1::combat::PLAYER_HH as i16)
                };
                self.move_relink(i, ctx.px, ctx.py, pz);
                PLAYER_TARGET
            }
            None => {
                self.move_relink(i, pos.0, pos.1, pos.2);
                0
            }
        };
        self.mc2_proj_impact(i, victim, ctx, None);
    }

    // ---- launch helpers ------------------------------------------------------

    /// `sub_5EF70` (EF:60598): poke the target wizard's danger timer.
    /// Pool wizards carry no reader yet (the rival MC2 column).
    pub(crate) fn mc2_danger_poke(&mut self, target: u16) {
        if target == PLAYER_TARGET {
            self.player_danger = 100;
        }
    }

    /// `sub_11900` (EF:4375) — MC2's POINT mailbox write, and ⚠ its
    /// two branches are the exact INVERSE of MC2's own area protocol
    /// at EF:4021-24: it OVERWRITES while a source is still pending
    /// and ACCUMULATES onto the stale amount once a reader has cleared
    /// it. Readers clear the SOURCE and never the amount (EF:5407), so
    /// MC2 point damage SNOWBALLS onto the residue exactly the way
    /// MC1's `sub_12B50` does — [`Gen::mail_write_single`] is that
    /// same law, and this is its MC2 face.
    ///
    /// Callers, all of them: the three melee thunks `sub_1CE80` /
    /// `sub_1CED0` / `sub_1CF20` (EF:9780/9794/9808) and the
    /// (10,65)/(10,66) debuff stamps' `sub_38F70` (EF:28440). Routing
    /// them through the AREA writer cost mc2l3 t=2704 the human's
    /// whole first lob: the inbox stood at (780, src 0) — consumed,
    /// residue standing — the fresh 780 OVERWROTE it under the area
    /// branch, and the drain took 780 where retail took 1560.
    ///
    /// (MC2 targets carry no per-channel mask; the human's inbox feeds
    /// the World intake.)
    pub(crate) fn mc2_melee_write(&mut self, target: u16, amt: u32, src: u16) {
        let tgt = if target == PLAYER_TARGET {
            MailTarget::Player
        } else {
            MailTarget::Pool(target as usize)
        };
        self.mail_write_single(tgt, 0, amt, src);
    }

    /// The target's (class, model) for the projectile filter bytes —
    /// the human is faithfully (3, 0).
    fn mc2_target_cm(&self, target: u16) -> (u8, u8) {
        if target == PLAYER_TARGET || target as usize >= self.ent.len() {
            (3, 0)
        } else {
            let t = &self.ent[target as usize];
            (t.class64, t.model65)
        }
    }

    /// `sub_582B0` (Sound.cpp:6569) — shortest-arc absolute angular
    /// distance between two 11-bit engine angles.
    pub(crate) fn arc_err(a: u16, b: u16) -> u16 {
        let d = a.wrapping_sub(b) & 0x7FF;
        d.min(0x800 - d)
    }

    /// `sub_68490` (EF:55101) — the acquisition scorer: reject
    /// outside the yaw/pitch cones or beyond 3-D distance 5120,
    /// else score = on-axis(cos)² terms + (4·sin(err))² terms — the
    /// off-axis angular error weighted ×16, so alignment dominates
    /// and distance tie-breaks. The castle variant `sub_685D0`
    /// (EF:55157) is the same law modulo term order — one body
    /// serves both (docs/traces/mc2-autoaim.md §2). Lower = better;
    /// None = rejected. Zero RNG.
    #[allow(clippy::too_many_arguments)]
    fn mc2_aim_score(
        &self,
        probe: &AimProbe,
        tx: u16,
        ty: u16,
        tz: i16,
        yaw_cone: u16,
        pitch_cone: u16,
    ) -> Option<u64> {
        use crate::mc2::sin_lut::SIN_DB750;
        let yaw_err = Self::arc_err(probe.yaw, Self::angle_between(probe.x, probe.y, tx, ty));
        if yaw_err > yaw_cone {
            return None;
        }
        let d2 = Self::dist2_sq(probe.x, probe.y, tx, ty) as i64;
        let dh = Self::isqrt(d2 as u32) as i32;
        let pitch_err = Self::arc_err(probe.pitch, Self::pitch_toward(probe.z, tz, dh));
        if pitch_err > pitch_cone {
            return None;
        }
        // 2-D: retail's `v8 = EuclideanDistXYZ_58490` (EF:55125 /
        // castle twin EF:55181) never reads z — both the 5120 gate
        // and the score's projection terms ride the HORIZONTAL
        // distance; reading z here double-weights altitude and
        // rejects high/low targets early. The candidate prefilters
        // stay genuinely 3-D (sub_583F0, below).
        let dist = dh as i64;
        if dist > 5120 {
            return None;
        }
        let sin = |a: u16| SIN_DB750[a as usize] as i64;
        let cos = |a: u16| SIN_DB750[0x200 + a as usize] as i64;
        let v9 = (dist * cos(yaw_err)) >> 16;
        let v10 = (4 * dist * sin(yaw_err)) >> 16;
        let v11 = (dist * cos(pitch_err)) >> 16;
        let v12 = (4 * dist * sin(pitch_err)) >> 16;
        Some((v11 * v11 + v9 * v9 + v10 * v10 + v12 * v12) as u64)
    }

    // `sub_67CB0` (EF:54710) — the auto-target acquisition, split
    // into the pure scan (`mc2_aim_scan`, shared with the crosshair
    // instrument) + the mutating first-tick lock (`mc2_autoaim`).
    // Best scorer result wins, first-scanned breaks ties.
    // Deliberate approximations (cited):
    // - the awake gate `byte_0x39_57` → f58 nonzero (retail's own
    //   truthiness on the byte);
    // - bucket 22 = the worm family, approximated as model-22
    //   heads + their f54 chains;
    // - the cave-in `sub_3A7F0` on-ground filter → z within one
    //   step of the terrain.
    // NOT an approximation (RETRACTED): the offensive branch's
    // class-3 range gate is the two-point `dist(cand.pos, self.pos)`
    // and it is LIVE. EF:54788 shows the same operand twice, but that
    // is a typo in the remc2 SOURCE — not retail, and not Hex-Rays.
    // The original decompiler output (remc2 `ab199daed`, the initial
    // upload, still raw) reads
    // `sub_583F0((x_WORD *)(i + 76), (x_WORD *)(a1 + 76))` — offset
    // 76 = 0x4C = position, two DISTINCT objects. remc2 commit
    // `22bf3758a5` ("190419-01", 2019-04-19) struct-ified the pointer
    // arithmetic and mistyped `a1` as `ix` here while converting the
    // three sibling gates (EF:54871 / 54897 / 54943) correctly in the
    // SAME hunk. 1 of 4 gates, 1 of 48 call sites.
    // ⚠ Reproduce with `git blame` + `git show ab199daed:...` INSIDE
    // reference/remc2 (that path is gitignored by this repo, so git
    // here answers "no such path" — that failure is the tell).
    // Two decoys on the way: `28b7c76336` (2026-02-02) is a mechanical
    // 3,291-line `axis_0x4C_76`→`position_0x4C_76` rename and
    // `3e041c9d30` (2025-12-22) is a 65,706-line file split; the
    // defect predates both.
    // ⚠ SCOPE, measured — this is NOT a load-bearing fork on the
    // recorded corpus. Over 15 full-pool censuses across five takes
    // the class-3 roster holds 1-5 entities and every one shares the
    // human's `id_0x1A_26`, so with EF:54785's same-id skip a
    // player-cast case-0 projectile has an EMPTY class-3 candidate
    // list and a monster-owned one has exactly the human. The two
    // readings could only diverge for a candidate in the thin shell
    // 3-D ∈ (4096, ~5450] with 2-D ≤ 5120 inside the ±0x71 cones,
    // and that shell is never occupied at an acquisition here.

    /// The model-keyed candidate lists + cones (trace §1/§8):
    /// (wizards, creatures, worms_always, spheres, buildings,
    /// yaw cone, pitch cone, wizard alarm, `sub_3A7F0`-only). None =
    /// a model with no acquisition switch arm.
    #[allow(clippy::type_complexity)]
    fn mc2_aim_lists(model: u8) -> Option<(bool, bool, bool, bool, bool, u16, u16, bool, bool)> {
        Some(match model {
            0 | 3 | 4 | 0x12 | 0x13 | 0x16 | 0x1A | 0x1C | 0x1E => {
                (true, true, false, false, false, 0x71, 0x71, true, false)
            }
            1 | 0x11 => (false, false, true, true, true, 0x71, 0x71, false, false),
            7 | 8 | 0xB | 0xC => (true, false, false, false, false, 0x71, 0x71, true, false),
            9 => (true, true, true, false, false, 0x71, 0x200, false, false),
            0x10 => (true, true, false, false, false, 0x100, 0x71, true, false),
            0x19 => (false, true, false, false, false, 0x71, 0x71, false, true),
            _ => return None,
        })
    }

    /// The class-3 lock range for a shot owned by `own`
    /// (`Entities[a1x->id_0x1A_26]->dword_0xA0_160x->word_160_0x1c_28`,
    /// EF:54783/54787).
    ///
    /// ⭐⭐⭐ WHOSE ROW A SITE READS IS A LAW. The port took the range
    /// off the PROJECTILE (class-9 rows are all 4096); retail hoists
    /// the OWNER's entity before the walk and reads its row, which for
    /// every wizard — human (`str_D7BD6[66]`, cave `[104]`) and rival
    /// (`[67]`) alike — is 8192. mc2l6 t=687 is the corpus row: rival
    /// 370's fresh (9,0) at (59264, 20095, 930) sits 5,065 from rival
    /// 383's carpet, inside 8192 and outside 4096, so retail locks it
    /// (`word_0x96_150` 383, desired 578/3 → yaw 515 + the hard-capped
    /// 34 = 549, pitch snapped to 3) where the port found nothing and
    /// flew on at the cast attitude 515/50.
    ///
    /// The human owner is out of the pool and has no `row156`; his
    /// carpet's row is a wizard row either way, and all three wizard
    /// rows carry the same 8192, so the constant is exact rather than
    /// an approximation. An owner-less shot (`id24` 0) reads retail's
    /// sentinel record — not a case the corpus reaches; it takes the
    /// same wizard default here.
    pub(crate) fn mc2_owner_lock_range(&self, own: u16) -> i64 {
        let row = match own {
            PLAYER_TARGET | 0 => crate::mc2::rivals::WIZARD_ROW,
            o if (o as usize) < self.ent.len() => self.ent[o as usize].row156,
            _ => crate::mc2::rivals::WIZARD_ROW,
        };
        BEHAVIOR[row as usize].v_28 as i64
    }

    /// The pure acquisition scan under an [`AimProbe`] — the scoring
    /// sweep of `sub_67CB0` with no writes (shared by the live
    /// first-tick lock and the crosshair instrument's preview).
    /// `human` = the human wizard candidate position (None = owner
    /// is the human, or invisible).
    pub(crate) fn mc2_aim_scan(
        &self,
        probe: &AimProbe,
        human: Option<(u16, u16, i16)>,
    ) -> Option<u16> {
        let (wizards, creatures, worms_always, spheres, buildings, yc, pc, _alarm, charm_eligible) =
            Self::mc2_aim_lists(probe.model)?;
        let own = probe.own;
        let range = probe.range;
        // Lightning's wizard range = the projectile's own reach
        // (minSpeed · maxLife, EF:54896).
        let wiz_range = if probe.model == 9 { probe.reach } else { range };
        let mut best: Option<(u16, u64)> = None;
        let consider =
            |g: &Self, best: &mut Option<(u16, u64)>, slot: u16, pos: (u16, u16, i16), yc, pc| {
                if let Some(sc) = g.mc2_aim_score(probe, pos.0, pos.1, pos.2, yc, pc) {
                    if best.is_none_or(|(_, b)| sc < b) {
                        *best = Some((slot, sc));
                    }
                }
            };
        if wizards {
            // The class-3 family list (wizards/castles/balloons),
            // own-owner and invisibles skipped; range-gated by the
            // owner row. The human is a candidate only for non-human
            // owners (player casts never self-target).
            // LIGHTNING's wizard scan runs a TIGHT pitch cone
            // (sub_67CB0 case 9: 0x71/0x71 for the wizard list vs
            // 0x71/0x200 for creatures, EF:54889-933 — the only
            // model where the two differ). The table pc stays
            // 0x200 for the creature/sphere branches below.
            let wiz_pc = if probe.model == 9 { 0x71 } else { pc };
            // ⭐⭐ THE WIZARD LIST IS THE TICK-TOP CLASS-3 ROSTER
            // `dword_38519` (EF:54783/54862/54892/54937), built by the
            // case-3 arm of the tick-top sweep under `life_0x8 >= 0`
            // ALONE (EF:39972-85). The walk re-asks nothing but
            // `id != own` and the invisibility bit (EF:54785), so a
            // wizard/castle/balloon that dies MID-tick stays a lock
            // candidate for the rest of the frame.
            for c in 0..self.wiz_chain.visible_len() {
                let v = self.wiz_chain.list[c] as usize;
                let e = &self.ent[v];
                if e.id24 == own || v as u16 == own || e.flags & 0x20 != 0 {
                    continue;
                }
                let dz = (e.z as i64) - (probe.z as i64);
                let d = Self::isqrt(
                    (Self::dist2_sq(probe.x, probe.y, e.x, e.y) as i64 + dz * dz)
                        .min(u32::MAX as i64) as u32,
                ) as i64;
                if d > wiz_range {
                    continue;
                }
                // Castles (3,2) score at the RAW z — the retail walk
                // routes model 2 through the raw-position castle
                // scorer sub_685D0 (EF:54790/54899/54945), same
                // cones/score as the bracketed sub_68490.
                let pos = (e.x, e.y, e.aim_z());
                consider(self, &mut best, v as u16, pos, yc, wiz_pc);
            }
            if let Some((hx, hy, hz)) = human {
                let dz = (hz as i64) - (probe.z as i64);
                let d = Self::isqrt(
                    (Self::dist2_sq(probe.x, probe.y, hx, hy) as i64 + dz * dz).min(u32::MAX as i64)
                        as u32,
                ) as i64;
                if d <= wiz_range {
                    consider(self, &mut best, PLAYER_TARGET, (hx, hy, hz), yc, wiz_pc);
                }
            }
        }
        if creatures {
            // The per-model buckets, worm family (22) excluded here
            // (offensive scans it only as the fallback); awake gate;
            // multipart segments skip like the census.
            // ⭐⭐ MODEL-MAJOR OVER THE TICK-TOP PER-MODEL ROSTER, not
            // the live pool: retail's `for (j = 0; j < 29; j++)` walks
            // `bytearray_38403x[j]` through `next_0`. The roster was
            // built by the case-5 arm of the tick-top sweep
            // (EF:39987-40008) under `life >= 0` and `actionIndex` not
            // in {0xB4, 0xE8, 0xEA} — so BOTH of those questions were
            // settled at the top of the frame and the walk never
            // re-asks them. A creature that dies mid-tick stays a lock
            // candidate for the rest of the frame; one already dead at
            // the top was never chained however alive it reads.
            for model in 0..29usize {
                if model == 22 && !worms_always {
                    continue;
                }
                for c in 0..self.mob_chains.visible(model).len() {
                    let v = self.mob_chains.visible(model)[c] as usize;
                    let e = &self.ent[v];
                    // The awake gate is retail's TRUTHINESS on the
                    // byte (`kx->byte_0x39_57`, EF:54811/54917/54964/
                    // 54992) — not a sign test. Read `<= 0` it
                    // rejected exactly the records the importer hands
                    // over carrying the −6 never-woken sentinel while
                    // admitting the natively-minted 250 that is the
                    // same byte.
                    if (e.f58 & 0xFF) == 0 || e.id24 == own {
                        continue;
                    }
                    // ⭐⭐⭐ `case 0x19`'s THIRD FILTER IS `sub_3A7F0`
                    // ITSELF (EF:54991) — the very charm-eligibility
                    // predicate `sub_3A650` asks before converting, not
                    // an on-ground test. The port had it as "z within
                    // one step of the terrain", cited as an
                    // approximation of a *"cave-in"* filter; it is
                    // nothing of the kind. The alliance carrier
                    // therefore locked onto creatures its own payload
                    // could never charm — above all creatures ALREADY
                    // CHARMED BY THIS CASTER (StageVar2 14), which the
                    // predicate bars outright. mc2l0-spells-galore
                    // t=25450: the human's tier-3 recast mints the
                    // (9,25) at slot 661 and the port locked slot 627,
                    // one of the three archers IT HAD JUST CHARMED,
                    // bending the launch to yaw 167 / pitch 46 where
                    // retail — finding no eligible candidate at all —
                    // returns 0 and fires straight down the caster's
                    // own post-tick pose (209 / 81).
                    if charm_eligible
                        && !no_mc2_aim_charm_filter()
                        && !self.mc2_charm_eligible(v)
                    {
                        continue;
                    }
                    if charm_eligible && no_mc2_aim_charm_filter() {
                        let g = self.ground_z(e.x, e.y) as i16;
                        if (e.z - g).unsigned_abs() > 256 {
                            continue;
                        }
                    }
                    let pos = (e.x, e.y, e.aim_z());
                    consider(self, &mut best, v as u16, pos, yc, pc);
                }
            }
        }
        if spheres {
            // The mana-sphere list: unowned or foreign spheres only,
            // AND AWAKE (`if (v26x->byte_0x39_57)`, EF:55024) — a
            // sleeping sphere never attracts the possession lock
            // (mc2l3 t=260: retail's bolt found no awake candidate
            // and flew straight at the cast attitude; the ungated
            // port snapped onto a dormant sphere). Ownership lane by
            // model (EF:55017-31): 39 reads playerEntityIndex@0x94
            // (f144), 57 the fused parentId (id24).
            // Membership is the TICK-TOP ball chain (`dword_38523`),
            // not the live pool: a sphere minted MID-tick is
            // invisible to this tick's acquisition (mc2l3 t=260: the
            // port locked a sphere a lower-slot kill had just
            // scattered; retail's chain predates it and the bolt
            // flew straight). The chain carries 39/40/57; the model
            // test below keeps 40 out like retail's `< 0x27` skip.
            //
            // ⭐⭐ …AND NOTHING PAST A SEVERED LINK. The sweep is
            // `v26x = dword_38523` (EF:54857) stepped by
            // `while (v26x > Entities_EA3E4[0]) … v26x = v26x->next_0`
            // (EF:55015/55045) — the list lives IN the pool, so a
            // member REALLOCATED earlier in this same tick, whose +0
            // the NewEvent ctor wiped, ends the walk one node past
            // itself. That is the `TickChain::cut` law, lowered in
            // `Gen::new_event`; `mc2_castle_absorb`'s retarget already
            // wears it on THIS SAME CHAIN (castle.rs, mc2l0 t=9953),
            // and so does MC1's own (9,1) lob (fixture mc1l0 t=604).
            //
            // mc2l0 t=9963 is the MC2 row: sphere 104 is swallowed by
            // the slot-82 burst and reborn as a (10,0) puff earlier in
            // the tick, cutting the chain 51…104. The human's (9,1)
            // bolt at slot 297 therefore sees ONE unowned in-range
            // candidate — slot 100 — and bears yaw 1575 / pitch 2003.
            // The un-cut walk reached slot 232 (134 units nearer,
            // score 23,027,966 v 24,765,478 — `sub_68490` is
            // distance-dominated) and flew 1581 / 1999.
            for k in 0..self.ball_chain.visible_len() {
                let v = self.ball_chain.list[k] as usize;
                let e = &self.ent[v];
                // ⭐⭐ THE WALK RE-ASKS MODEL, OWNER AND AWAKE — NOTHING
                // ELSE (EF:55016-31). No class test, no life test, no
                // hidden-bit test: membership was settled at the frame
                // top, so a sphere FREED earlier in this very tick is
                // still a lock candidate, exactly like the wizard and
                // creature lists above. mc2l3 t=10055 is the corpus
                // row — sphere 237 is freed before the human's (9,1)
                // at slot 232 dispatches (class 10 → 0, model/awake/
                // position untouched, and the free does not unlink it
                // from `dword_38523`), and retail locks it anyway
                // (`word_0x96_150` 237, yaw 248 / pitch 96) where the
                // guarded port fell through to the 119-units-farther
                // sphere 227 and bore 250 / 92.
                if !matches!(e.model65, 39 | 57) || (e.f58 & 0xFF) == 0 {
                    continue;
                }
                let owner_lane = if e.model65 == 57 { e.id24 } else { e.f144 };
                if owner_lane == own {
                    continue;
                }
                consider(self, &mut best, v as u16, (e.x, e.y, e.aim_z()), yc, pc);
            }
        }
        if buildings {
            // The buildings list: skip own, the un-possessable
            // (bldgprm byte_2 & 8, EF:55053) and the ASLEEP
            // (`if (i3x->byte_0x39_57)`, EF:55051).
            // ⭐⭐ THE TICK-TOP BUILDING ROSTER `dword_38527`, not the
            // pool — the same chain `Gen::bldg_chain` already backs the
            // ch0 broadcast pass. Membership (class 10, model 45, live)
            // was settled at the frame top; the walk re-asks only the
            // owner, the un-possessable bit and the awake byte.
            for c in 0..self.bldg_chain.visible_len() {
                let v = self.bldg_chain.list[c] as usize;
                let e = &self.ent[v];
                if (e.f58 & 0xFF) == 0 || e.f144 == own {
                    continue;
                }
                if self
                    .assets
                    .bldgprm
                    .get(e.f71 as usize)
                    .is_some_and(|b| b.flags & 8 != 0)
                {
                    continue;
                }
                consider(self, &mut best, v as u16, (e.x, e.y, e.aim_z()), yc, pc);
            }
        }
        // ⭐⭐⭐ `worms_always` DOES DOUBLE DUTY AND CASE 9 ONLY WANTS
        // HALF OF IT. For model 9 the flag has to keep bucket 22 in
        // the plain creature walk above (retail's case 9 is a bare
        // `for (jj = 0; jj < 29; ++jj)` over `bytearray_38403x[jj]`,
        // EF:54912-54927 — bucket 22 included, no special case), but
        // case 9 has NO worm HEAD/`f54` SEGMENT branch at all: the
        // segment expansion appears exactly twice in `sub_67CB0`, at
        // EF:54826 (the big case's no-candidate fallback) and at
        // EF:55072 (case 1/0x11's unconditional tail). Running it for
        // lightning scored worm SEGMENTS retail cannot see — segments
        // carry `actionIndex` 0xB4, which the tick-top per-model
        // roster build EXCLUDES (EF:39987-40008), so they are exactly
        // the records the roster walk was designed to keep out.
        let worm_chain = worms_always && (probe.model != 9 || no_lightning_worm_chain());
        if worm_chain
            || (best.is_none()
                && matches!(
                    probe.model,
                    0 | 3 | 4 | 0x12 | 0x13 | 0x16 | 0x1A | 0x1C | 0x1E
                ))
        {
            // The worm bucket: case 1 runs it UNCONDITIONALLY,
            // competing with the sphere/building candidates
            // (EF:55071 — no best-empty gate); the big case runs it
            // only as the no-candidate fallback (EF:54825).
            // An AWAKE model-22 HEAD admits
            // its f54 chain members as candidates — the head itself
            // is never scored and the members' own awake bytes are
            // not tested (EF:55071-85).
            // ⭐⭐ Bucket 22 off the TICK-TOP per-model roster, like the
            // main creature walk above — liveness and the reap action
            // were settled at the frame top.
            for c in 0..self.mob_chains.visible(22).len() {
                let v = self.mob_chains.visible(22)[c] as usize;
                let e = &self.ent[v];
                if (e.f58 & 0xFF) == 0 || e.id24 == own {
                    continue;
                }
                let mut j = self.ent[v].f54 as usize;
                while j != 0 {
                    let s = &self.ent[j];
                    consider(self, &mut best, j as u16, (s.x, s.y, s.aim_z()), yc, pc);
                    j = self.ent[j].f54 as usize;
                }
            }
        }
        best.map(|(target, _)| target)
    }

    /// `sub_67CB0` (EF:54710) — the ONE-SHOT acquisition on a live
    /// flyer's first tick: run the pure scan under the projectile's
    /// own probe, then apply the lock (`sub_655C0` — f146 + desired
    /// aim, target z raised by its half-height) and the "you are
    /// targeted" alarm on wizard locks (`sub_5EF70`).
    pub(crate) fn mc2_autoaim(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let probe = {
            let e = &self.ent[i];
            AimProbe {
                x: e.x,
                y: e.y,
                z: e.z,
                yaw: e.f30,
                pitch: e.f32,
                model: e.model65,
                own: e.id24,
                range: self.mc2_owner_lock_range(e.id24),
                reach: e.f128 as i64 * e.max_life as i64,
            }
        };
        let human = (probe.own != PLAYER_TARGET && !self.player_invisible)
            .then_some((ctx.px, ctx.py, ctx.pz));
        let Some(target) = self.mc2_aim_scan(&probe, human) else {
            return false;
        };
        let alarm = Self::mc2_aim_lists(probe.model).is_some_and(|l| l.7);
        // `sub_655C0`: the lock + the desired aim toward it (the
        // sub_65580 bracket — model-2 raw, [`Ent::aim_z`]).
        // ⭐ THE `sub_65580` RAISE IS NOT POOL-ONLY — IT LIFTS THE
        // HUMAN TOO. `sub_655C0` (EF:54862) brackets BOTH tan calls in
        // `sub_65580`/`sub_655A0`, which add and remove
        // `target->array_0x52_82.yaw` on any model but 2; retail's
        // player is an ordinary boxed pool wizard, so his carpet is
        // lifted by its own 100 like everything else. The servo arm in
        // [`Gen::mc2_flyer_tick`] already carries this (`tz +
        // PLAYER_HH`); the ONE-SHOT acquisition here read the raw pose
        // and aimed a full box low at the player alone.
        // mc2l6-rival-spells-galore t=309: rival 378's (9,0) at
        // (64750, 16685, 484) acquires the human at (62080, 18560,
        // 256) — retail's `fov_0x22_34` is 12 off the raised 356,
        // where the raw 256 gave 21, and `sub_65C20`'s arm copies that
        // word straight into `pitch_0x1E_30` (EF:63104/63118), so the
        // whole flight flew a box low (z 455 vs retail's 467).
        let (tx, ty, tz) = if target == PLAYER_TARGET {
            (
                ctx.px,
                ctx.py,
                ctx.pz + crate::mc1::combat::PLAYER_HH as i16,
            )
        } else {
            let t = &self.ent[target as usize];
            (t.x, t.y, t.aim_z())
        };
        let e = &self.ent[i];
        let yaw = Self::angle_between(e.x, e.y, tx, ty);
        let dh = Self::isqrt(Self::dist2_sq(e.x, e.y, tx, ty) as u32) as i32;
        let pitch = Self::pitch_toward(e.z, tz, dh);
        let e = &mut self.ent[i];
        e.f146 = target;
        e.f34 = yaw;
        e.f36 = pitch;
        // `sub_68BD0` (EF:55453, called from the lock at EF:54848 —
        // the big model case only): a lock onto a class-5 model-0
        // victim arms the dragon's 32-tick dodge-alert window
        // ([`Gen::m0_dodge`]).
        if matches!(
            probe.model,
            0 | 3 | 4 | 0x12 | 0x13 | 0x16 | 0x1A | 0x1C | 0x1E
        ) && target != PLAYER_TARGET
        {
            let v = target as usize;
            if self.ent[v].class64 == 5 && self.ent[v].model65 == 0 {
                self.ent[v].f46 = 32;
            }
        }
        if alarm {
            let is_wizard = target == PLAYER_TARGET
                || (self.ent[target as usize].class64 == 3
                    && self.ent[target as usize].model65 == 0);
            if is_wizard {
                self.mc2_danger_poke(target);
            }
        }
        true
    }

    /// Shared field arming every launch thunk performs after the
    /// creator (id, aim, target hand-off, filter bytes).
    pub(crate) fn mc2_arm_proj(&mut self, p: usize, i: usize, target: u16, tpos: (u16, u16, i16)) {
        let pz = self.ent[i].z;
        self.mc2_arm_proj_from(p, i, target, tpos, pz);
    }

    /// [`Gen::mc2_arm_proj`] with the AIM ORIGIN z named explicitly —
    /// the one thunk that measures from the lifted muzzle
    /// (`sub_1D260`, [`Gen::mc2_atk_heavy9`]) passes its own muzzle
    /// here; everybody else passes the caster's raw z.
    pub(crate) fn mc2_arm_proj_from(
        &mut self,
        p: usize,
        i: usize,
        target: u16,
        tpos: (u16, u16, i16),
        pz: i16,
    ) {
        let (own, f146) = (self.ent[i].id24, self.ent[i].f146);
        // ⭐ THE AIM IS MEASURED FROM THE CASTER, NOT FROM THE SPAWN
        // POINT. Every launch thunk holds `v2 = &a1x->position_0x4C_76`
        // — the CASTER's — and hands THAT to both `sub_581E0_maybe_tan2`
        // and `sub_58210_radix_tan` before touching the new record's z:
        // `sub_1CC20` (9,0) bolt EF:9697-99, `sub_1CCE0` (9,13)
        // EF:9725-27, `sub_1CDA0` arrow EF:9753-55, `sub_1D1A0` (9,21)
        // lob EF:9865-68 — which even SAVES the spawned z in `v7`,
        // computes the pitch, and only then writes `v7 + 128` back. The
        // doomsday launcher does the same from the AVATAR (EF:13497-501)
        // while spawning 640 ahead at z+768. So the z-lift is applied
        // AFTER the aim and must not tilt it.
        //
        // mc2l3 t=1410: the m20 at (31785, 28647, 927) lobs at the human
        // — retail's radix_tan from z 927 gives pitch 1907, the port's
        // from the lifted 1055 gave 1915. (The x/y are the caster's in
        // every thunk, so only the z ever differed; `m27_branch_bolt`
        // already lifted AFTER arming and is unaffected.)
        //
        // ⭐⭐⭐ …EXCEPT IN EXACTLY ONE OF THE SEVEN. `sub_1D260`
        // (EF:9887-99) — m23's (9,9) heavy bolt — builds the muzzle
        // FIRST (`predictedAxis = a1x->position; predictedAxis.z +=
        // a1x->array_0x52_82.fov`), SPAWNS at that point, and hands
        // **`&predictedAxis`** to both tan calls; it never touches the
        // new record's z afterwards. The other six all hold
        // `v2 = &a1x->position_0x4C_76` and lift the spawned record
        // after the aim (`sub_1CC20`:9697, `sub_1CCE0`:9725,
        // `sub_1CDA0`:9753, `sub_1D0E0`:9832, `sub_1D1A0`:9865,
        // `sub_1D460`:9963). The shipped EXE settles it at file
        // `0x41A80`: `mov 0x58(%eax),%ax; add %eax,%ebx;
        // mov %bx,0x1b39c` writes the lifted z into the
        // `predictedAxis` global BEFORE `push $0x1b398` /
        // `call 0x7c9e0` (`sub_581E0`) and `call 0x7ca10`
        // (`sub_58210`). So the aim origin is a PER-THUNK fact, not a
        // family rule — hence the explicit `pz`.
        let (px, py) = (self.ent[i].x, self.ent[i].y);
        self.ent[p].id24 = own;
        let yaw = Self::angle_between(px, py, tpos.0, tpos.1);
        let dh = Self::isqrt(Self::dist2_sq(px, py, tpos.0, tpos.1) as u32) as i32;
        self.ent[p].f30 = yaw;
        let pitch = Self::pitch_toward(pz, tpos.2, dh);
        self.ent[p].f32 = pitch;
        self.ent[p].f146 = f146;
        let (tc, tm) = self.mc2_target_cm(target);
        self.ent[p].f66 = tc;
        self.ent[p].f67 = tm;
    }

    // ---- the attack thunks (mc2_chase_attack-compatible) --------------------

    /// `sub_1CE80` (EF:9772): melee within 1024, damage = own f44.
    pub(crate) fn mc2_atk_melee_1024(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 1024)
    }

    /// `sub_1CED0` (EF:9786): melee within 768.
    pub(crate) fn mc2_atk_melee_768(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 768)
    }

    /// `sub_1CF20` (EF:9800): melee within 1536.
    pub(crate) fn mc2_atk_melee_1536(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        self.mc2_atk_melee(i, target, ctx, 1536)
    }

    fn mc2_atk_melee(&mut self, i: usize, target: u16, ctx: &MobCtx, range: u32) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let e = &self.ent[i];
        if Self::mc2_dist3((e.x, e.y, e.z), tpos) >= range {
            return false;
        }
        let (amt, src) = (self.ent[i].f44 as u32, self.ent[i].id24);
        self.mc2_melee_write(target, amt, src);
        true
    }

    /// `sub_1CC20` (EF:9680): the (9,0) bolt — impact (10,0) fire,
    /// row 65, subSpell 500, z-lift = own fov (f84), danger poke.
    pub(crate) fn mc2_atk_bolt(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let Some(p) = self.mc2_spawn_bolt(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 0;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 500;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D0E0` (EF:9814): the (9,20) lob — impact (10,65),
    /// row 65, subSpell 780, z-lift = own fov.
    pub(crate) fn mc2_atk_lob20(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let Some(p) = self.mc2_spawn_lob20(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 65;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 780;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D1A0` (EF:9847): the (9,21) arc — impact (10,66),
    /// row 65, subSpell 780, fixed z-lift 128.
    pub(crate) fn mc2_atk_lob21(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let Some(p) = self.mc2_spawn_lob21(x, y, z.wrapping_add(128)) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 66;
        self.ent[p].row156 = 65;
        self.ent[p].f44 = 780;
        self.mc2_arm_proj(p, i, target, tpos);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D260` (EF:9883): m23's (9,9) heavy bolt — spawned at
    /// pos + fov, impact (10,23), row 64, subSpell 4000.
    ///
    /// ⭐⭐⭐ AND IT IS THE ONE THUNK THAT AIMS FROM THE MUZZLE.
    /// EF:9887-90 lifts `predictedAxis.z` by the caster's
    /// `array_0x52_82.fov` BEFORE the spawn and then passes that same
    /// lifted point to `sub_581E0_maybe_tan2` and
    /// `sub_58210_radix_tan` (EF:9898-99); its six siblings aim from
    /// `&a1x->position_0x4C_76` and lift the spawned record
    /// afterwards. Verified in `NETHERW.EXE` at file `0x41A80`
    /// (`mov 0x58(%eax),%ax` / `add %eax,%ebx` /
    /// `mov %bx,0x1b39c`, then two pushes of `$0x1b398` into
    /// `call 0x7c9e0` and `call 0x7ca10`).
    ///
    /// The lift is large — a leviathan's fov is 366 on mc2l22 — so
    /// aiming from the caster's feet flattened the beam by ~47 angle
    /// units and the hitscan then flew clean OVER its victim.
    /// mc2l22 t=20222: slot 1's (5,23) fires on rival 584; retail's
    /// beam carries `pitch` 41 and stops at 7 march steps snapped onto
    /// 584, the port's carried 2042 and ran the full 10.
    pub(crate) fn mc2_atk_heavy9(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16)
        };
        let muzzle_z = z.wrapping_add(lift);
        let Some(p) = self.mc2_spawn_bolt9(x, y, muzzle_z) else {
            return false;
        };
        self.ent[p].f68 = 10;
        self.ent[p].f69 = 23;
        self.ent[p].row156 = 64;
        self.ent[p].f44 = 4000;
        let aim_z = if no_heavy9_muzzle_aim() { z } else { muzzle_z };
        self.mc2_arm_proj_from(p, i, target, tpos, aim_z);
        self.mc2_danger_poke(target);
        true
    }

    /// `sub_1D460` (EF:9918): m18's 5-shot fan — yaw offsets −226,
    /// −113, 0, +113, +226, each a (9,0) with impact (10,0), row 61,
    /// subSpell 800, z-lift 200.
    pub(crate) fn mc2_atk_fan(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z)
        };
        let mut fired = false;
        for off in [-226i32, -113, 0, 113, 226] {
            let Some(p) = self.mc2_spawn_bolt(x, y, z.wrapping_add(200)) else {
                continue;
            };
            self.ent[p].f68 = 10;
            self.ent[p].f69 = 0;
            self.ent[p].row156 = 61;
            self.ent[p].f44 = 800;
            self.mc2_arm_proj(p, i, target, tpos);
            // EF:9962 — the offset rides the yaw only; `sub_1D460`
            // writes no roll either (and lifts z by 200 after the aim,
            // which `mc2_arm_proj` now honours).
            //
            // ⭐⭐⭐ **AND THE FAN'S YAW IS NOT MASKED.** The line is
            // `v4x->yaw_0x1C_28 = v3 + Maths::sub_581E0_maybe_tan2(v9,
            // &a2x->position_0x4C_76);` — a RAW store into an
            // `int16_t`, with no `& 0x7FF` anywhere in `sub_1D460`.
            // The `+226`/`-226` wings therefore leave the 0..2047
            // bearing space on purpose and the flyer carries a yaw
            // ABOVE 0x7FF (or below 0) until whatever reads it folds
            // it at index time. The port's mask was INVENTED, and it
            // is false exactly on the two outer wings of every fan —
            // the classic invented-write shape (rounds 83-84).
            //
            // WITNESS — mc2l15 pair 15433→15434, slot 2, the +226 wing
            // of the (5,18) tank at slot 4 (`MGC_WRITE_TRACE=2:f30`
            // names the writer): the aimed bearing is 2035, retail
            // stores **2261**, and 2261 & 0x7FF = **213** — exactly
            // what the port wrote. The other four bolts of the same
            // volley (slots 7/225/265/268) were bit-exact, because
            // their offsets kept them inside the mask.
            //
            // `MGC_NO_MC2_FAN_YAW_UNMASKED=1` restores the mask.
            self.ent[p].f30 = if no_mc2_fan_yaw_unmasked() {
                (self.ent[p].f30 as i32 + off) as u16 & 0x7FF
            } else {
                (self.ent[p].f30 as i32 + off) as u16
            };
            fired = true;
        }
        if fired {
            self.mc2_danger_poke(target);
        }
        fired
    }

    /// `sub_1CDA0` (EF:9742): m9's (9,13) arrow — z-lift = the caster's
    /// `array_0x52_82` **fov/yaw** member (f84), subSpell 600 when owned
    /// (f144 set) else 400, the ctor's sprite 195 RE-STAMPED to 203
    /// (EF:9764), danger poke.
    ///
    /// ⚠⚠ **THE DECOMPILE SAYS `.roll` HERE AND THE MEASUREMENT SAYS IT
    /// CANNOT BE.** EF:9756 reads
    /// `v3x->position_0x4C_76.z += a1x->array_0x52_82.roll;` — `.roll`
    /// is +0x56, `.fov` +0x58, ONE MEMBER APART in a 4-field struct.
    /// mc2l3 t=5707 decides it with zero free parameters: the m9 at slot
    /// 225 stands at z 1124 wearing the quad 128/86/86/128, and the
    /// arrow's whole same-tick flight step is bit-exact on both sides
    /// (yaw 1111, pitch 1963, speed 384, x 47782, y 3240 all EQUAL), so
    /// the entire 42-unit delta is in the BIRTH z:
    /// `(384 * SIN[1963]) >> 16 = -100`, retail's 1352 ⇒ birth 1252 =
    /// 1124 + **128**; `.roll` = 86 gives the port's 1310.
    ///
    /// ⚠ THE VALUE IS MEASURED; THE NOUN IS A THREE-WAY TIE THIS CORPUS
    /// CANNOT BREAK, and it is deliberately not claimed to be settled:
    /// `SetEntityIndexAndRot_49CD0` (EF:32842-45) sets
    /// `.yaw == .fov == rot_speed_8 / 2`, and the m9 only ever wears
    /// rows 201/202 through that plain path, so `.yaw` (+0x52), `.fov`
    /// (+0x58) and a LITERAL 128 are indistinguishable here forever —
    /// and the immediate family does all three (`sub_1CC20` EF:9699
    /// `.fov`, `sub_1CCE0` EF:9728 `.fov / 2`, `sub_1D1A0` EF:9847 a
    /// bare literal). `.fov` is chosen because it is the thunk family's
    /// idiom: EF:9756 is the ONLY `z += ….roll` in all 63,636 lines,
    /// against 30+ `.fov` z-lifts. Provenance checked — no `//fix`
    /// marker, and the line entered `EventsFunctions.cpp` via a file
    /// SPLIT, so it is most likely original decompiler output (a
    /// struct-member mis-attribution) rather than a hand edit.
    pub(crate) fn mc2_atk_arrow(&mut self, i: usize, target: u16, ctx: &MobCtx) -> bool {
        let Some(tpos) = self.mc2_target(target, ctx) else {
            return false;
        };
        let (x, y, z, lift, owned) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f84 as i16, e.f144 != 0)
        };
        let Some(p) = self.mc2_spawn_arrow(x, y, z.wrapping_add(lift)) else {
            return false;
        };
        self.ent[p].f44 = if owned { 600 } else { 400 };
        self.mc2_arm_proj(p, i, target, tpos);
        // `sub_49E10(v3x, 203)` (EF:9764): the ctor stamps row 195
        // (`AddEvent09_0D_4DAB0` EF:35045) and the m9 thunk RE-STAMPS
        // 203 on top, AFTER the field writes and BEFORE `sub_5EF70`.
        // Geometry-safe: rows 195/203 point at sprite bases 105 and
        // 116, both 18x24 in the day bank, so `derive_sprite_extents`
        // gives both (45, 60) and the doubled quad stays 30/44/44/60 —
        // only the billboard moves. MC1's column has carried the same
        // re-skin since mc1/mobs.rs:2929.
        self.mc2_set_sprite_x2(p, 203);
        self.mc2_danger_poke(target);
        true
    }
}

#[cfg(test)]
mod debuff_knock_tests {
    use crate::chassis::ChassisParams;
    use crate::engine::features::{FeatureAssets, Gen, Planes};
    use crate::mc1::combat::MailTarget;
    use crate::mc1::mobs::{MobCtx, PLAYER_TARGET};
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
            px: 40 * 256,
            py: 40 * 256,
            pz: 400,
            pyaw: 1024, // a bearing the INVENTED write would have used
            pmana: 1000,
            pmana_max: 1000,
            pdead: false,
            pdead_top: false,
            strict: false,
            patches: crate::patches::WorldPatches::RETAIL,
            mc2_turn: 0,
        }
    }

    /// ⭐⭐⭐ A SPLIT IN AN ENUMERATED LIST IS THE LAW: `sub_1D260`
    /// AIMS FROM THE LIFTED MUZZLE, ITS SIX SIBLINGS FROM THE CASTER.
    ///
    /// `sub_1D260` (EF:9883) opens
    /// `predictedAxis = a1x->position; predictedAxis.z +=
    /// a1x->array_0x52_82.fov;` and then hands **that** pointer to
    /// both `sub_581E0_maybe_tan2` (EF:9898) and
    /// `sub_58210_radix_tan` (EF:9899); it never touches the spawned
    /// record's z afterwards. Every sibling launch thunk holds
    /// `v2 = &a1x->position_0x4C_76` and lifts the SPAWNED record
    /// after the aim — `sub_1CC20`:9697, `sub_1CCE0`:9725,
    /// `sub_1CDA0`:9753, `sub_1D0E0`:9832, `sub_1D1A0`:9865,
    /// `sub_1D460`:9963. The shipped `NETHERW.EXE` settles the order
    /// at file `0x41A80`: `mov 0x58(%eax),%ax` / `add %eax,%ebx` /
    /// `mov %bx,0x1b39c` writes the lifted z into the `predictedAxis`
    /// global BEFORE `push $0x1b398; call 0x7c9e0` and
    /// `push $0x1b398; call 0x7ca10`.
    ///
    /// A leviathan's fov is 366 on mc2l22, so aiming from its feet
    /// flattened the (9,9) hitscan by ~47 angle units and the beam
    /// flew over its victim: t=20222 slot 1 fires on rival 584 —
    /// retail's beam carries `pitch` 41 and stops at 7 march steps
    /// snapped onto 584, the port's carried 2042 and ran the full 10,
    /// laying a 22-node-longer trail on a flat bearing.
    /// `MGC_NO_MC2_HEAVY9_MUZZLE_AIM`. Ledger SESSION 97 dig D14.
    #[test]
    fn heavy9_aims_from_the_fov_lifted_muzzle_not_the_caster() {
        let mut g = flat_gen();
        let lev = g.new_event().expect("caster slot");
        {
            let e = &mut g.ent[lev];
            e.class64 = 5;
            e.model65 = 23;
            e.tick70 = 186;
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 1000;
            e.f84 = 366; // array_0x52_82.fov — the leviathan's lift
            e.act_life = 1000;
        }
        let vic = g.new_event().expect("victim slot");
        {
            let e = &mut g.ent[vic];
            e.class64 = 3;
            e.model65 = 1;
            e.x = 40 * 256 + 2400;
            e.y = 40 * 256;
            e.z = 1000;
            e.act_life = 100;
        }
        let before = g.ent.iter().filter(|e| e.class64 == 9).count();
        assert!(g.mc2_atk_heavy9(lev, vic as u16, &ctx()), "the bolt fires");
        let bolt = g
            .ent
            .iter()
            .position(|e| e.class64 == 9 && e.model65 == 9)
            .expect("a (9,9) beam was spawned");
        assert_eq!(
            g.ent.iter().filter(|e| e.class64 == 9).count(),
            before + 1,
            "exactly one record"
        );
        // The bolt is BORN at the lifted muzzle either way — only the
        // aim origin is in question.
        assert_eq!(g.ent[bolt].z, 1366, "spawned at position.z + fov");

        let tpos = g.mc2_target(vic as u16, &ctx()).expect("target resolves");
        let dh = Gen::isqrt(Gen::dist2_sq(40 * 256, 40 * 256, tpos.0, tpos.1) as u32) as i32;
        let from_muzzle = Gen::pitch_toward(1366, tpos.2, dh);
        let from_caster = Gen::pitch_toward(1000, tpos.2, dh);
        assert_ne!(
            from_muzzle, from_caster,
            "the fixture is only meaningful while the two origins disagree"
        );
        assert_eq!(
            g.ent[bolt].f32, from_muzzle,
            "sub_1D260 measures the pitch from `predictedAxis` AFTER \
             `predictedAxis.z += fov` (EF:9887-9899, NETHERW.EXE \
             0x41A80); reading {from_caster} here means the aim went \
             back to the caster's raw z like the other six thunks"
        );
        // The yaw is unaffected by the lift and must still be the
        // caster-to-target bearing.
        assert_eq!(
            g.ent[bolt].f30,
            Gen::angle_between(40 * 256, 40 * 256, tpos.0, tpos.1),
            "the lift is z-only, so the yaw is untouched"
        );
    }

    /// ⭐⭐⭐ AN INVENTED WRITE: THE DEBUFF STAMP SETS ONLY THE
    /// MAGNITUDE, AND SETS IT NEGATIVE.
    ///
    /// `sub_38F70` (paralyze, EF:28437) and `sub_38E70` (web-slow,
    /// EF:28411) each write `moveBoost_0x1E_30 = -80` and **nothing
    /// else** — the shipped EXE has the whole write as one
    /// instruction, `mov WORD PTR [eax+0x1e],0xffb0` at NETHERW.EXE
    /// 0x5d7b3, and nothing in either function touches `+0x20`. The
    /// heading is a SEPARATE register that keeps whatever bearing the
    /// last buffet left in it, and the mover consumes the two apart:
    /// EF:59695-711 is `MoveEntity_57FA0(&pred, ext->yaw_0x1E_30, 0,
    /// ext->moveBoost_0x1E_30)` with a ONE-SIDED clamp (`> 128` only,
    /// so a negative magnitude passes through by design) and a
    /// decay-toward-zero that zeroes below `|4|`. The sign goes
    /// straight into the polar step, so the kick lands 180 degrees off
    /// the stale bearing.
    ///
    /// The port used to fabricate BOTH halves — heading
    /// `pyaw + 0x400`, magnitude `+80` — which happens to look right
    /// only when the stale bearing already equals the carpet's own
    /// yaw.
    ///
    /// ⚠ NO FIXTURE HOME: the pair lane is CLEAN across the whole head
    /// window (mc2l22 pairs 405..416 grade 12 conforming, 0 field
    /// diffs) because the importer restores the knock register every
    /// pair — this law is visible only to the free run, which is why
    /// it sat under the horizon for two sessions. Corpus witness:
    /// mc2l22 t=412→413, where the human's mail does NO damage so no
    /// buffet overwrites the register first — retail steps (+66,+45)
    /// = polar(bearing 1729, mag −80), the port stepped (−77,+25)
    /// = polar(1434, +80). Free-run horizon 412 → 522.
    /// Ledger SESSION 95 dig 1.
    #[test]
    fn the_debuff_stamp_writes_only_a_negative_magnitude() {
        let mut g = flat_gen();
        let tok = g.new_event().expect("token slot");
        {
            let e = &mut g.ent[tok];
            e.class64 = 10;
            e.model65 = 66;
            e.tick70 = 71; // paralyze
            e.f146 = PLAYER_TARGET;
            e.id24 = 481;
            e.f140 = 0;
        }
        // A bearing an earlier buffet left in the register. Retail
        // does not touch it, so the kick must still ride THIS value,
        // not anything derived from `ctx.pyaw`.
        g.player_knock = (1729, 40);

        g.mc2_debuff_stamp_tick(tok, &ctx());

        assert_eq!(
            g.player_knock.1, -80,
            "the stamp writes moveBoost_0x1E_30 = -80 (EXE 0x5d7b3 \
             mov WORD PTR [eax+0x1e],0xffb0), NEGATIVE — a +80 here is \
             the invented magnitude and sends the carpet the wrong way"
        );
        assert_eq!(
            g.player_knock.0, 1729,
            "the heading register (+0x20) is STALE — nothing in \
             sub_38F70/sub_38E70 writes it. Seeing ctx.pyaw + 0x400 \
             (= 0) here means the invented heading write is back"
        );
    }

    /// ⭐⭐⭐ THE VICTIM GATE BELONGS TO THE WORKER, NOT TO THE PAYLOAD.
    ///
    /// `sub_674C0` — the LEVELED possession, class-9 **action 18** —
    /// wraps its ENTIRE spawn block in `if (v6x)`, the `sub_108B0`
    /// claim-probe result (EF:59029-58, with `v6x` bound at EF:59003).
    /// The shipped EXE leaves no room for doubt: at NETHERW.EXE
    /// 0x8be27, `test edi,edi` / `je 0x8bed4` jumps clean **past both
    /// `_4A190` spawns** to a bare
    /// `push ebx; call 0x7c710` = `DisableEntityDrawing04_57F10` — so a
    /// ground stop or a life expiry with NO claim victim disables
    /// drawing and **allocates nothing at all**. Its basic twin
    /// `CastPosses_65F60` (action 1, EF:63305-19) has the same block
    /// with NO such gate.
    ///
    /// The port carried the gate on the `(10,54)/(10,69)` **payload**
    /// arm instead, so an action-18 bolt holding any other payload
    /// minted its effect on a victimless expiry — one spurious
    /// `NewEvent` that re-seats every later pop in the tick.
    ///
    /// ⚠ NO FIXTURE HOME: the whole-take pair census is BYTE-IDENTICAL
    /// with and without this law; the entire gain is in the free run.
    /// Corpus receipt: mc2l22 t=523, where retail takes rival 611's
    /// bolt at slot 718 to `life 0 -> -1`, `flags 6 -> 1030` and
    /// allocates NOTHING (free stack 296 -> 296, next-pop still 729)
    /// while the port popped 729 for a `(10,12)` claim pulse — the
    /// take's horizon wall at 522, now 825.
    /// Ledger SESSION 95 wave 2 dig 2.
    #[test]
    fn an_action_18_bolt_with_no_claim_victim_allocates_nothing() {
        let mut g = flat_gen();
        let bolt = g.new_event().expect("bolt slot");
        {
            let e = &mut g.ent[bolt];
            e.class64 = 9;
            e.model65 = 17; // the LEVELED possession
            e.tick70 = 18; // action 18 -> sub_674C0
            e.id24 = 611;
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 400;
            e.f44 = 12;
        }
        let free_before = g.free.len();

        // victim = 0: the claim probe found nobody.
        g.mc2_proj_impact(bolt, 0, &ctx(), None);

        assert_eq!(
            g.free.len(),
            free_before,
            "an action-18 expiry with no claim victim must allocate \
             NOTHING — the whole spawn block sits inside `if (v6x)` \
             (EXE 0x8be27 test edi,edi / je past both _4A190 calls). \
             A shorter free list here means the port is minting a \
             claim pulse retail never mints, and every later pop in \
             the tick is re-seated."
        );
        assert_ne!(
            g.ent[bolt].flags & 0x400,
            0,
            "the bolt still reaps — retail's arm runs \
             DisableEntityDrawing04_57F10 and nothing else"
        );
    }

    /// ROUND 102 — THE LEVELED POSSESSION DIES ON CONTACT WHATEVER THE
    /// POOL SAYS. `sub_674C0` gates its two children on their own
    /// allocations (`if (v12x)` the claim pulse, `if (v14x)` the aura,
    /// EF:59040-53) but its `DisableEntityDrawing04_57F10(a1x)` is the
    /// last statement of the `if (v16)` block, OUTSIDE both (EF:59055).
    /// The generic "an impact that cannot mint its effect survives"
    /// rule belongs to the effect-existence-gated workers (`sub_65820`
    /// EF:62981, `sub_65C20` EF:63183, `CastPosses_65F60` EF:63310) —
    /// not to this one. Corpus receipt: mc2l22 t=8347 (free 0 / victim
    /// 0), rival 557's Mana Lock at slot 991 snaps onto sphere 484 on
    /// both sides; retail flags it (0x406) and mints nothing, the port
    /// flew on — so at 8348 retail's tick-top reap freed 991, the
    /// HIGHEST freed slot, and the human turret's beam popped it while
    /// the port's popped 959. ⚠ NO FIXTURE HOME (an INHERITED head).
    #[test]
    fn an_action_18_bolt_that_struck_dies_even_on_a_dry_pool() {
        let mut g = flat_gen();
        let victim = g.spawn_mana_ball(40 * 256, 40 * 256, 400).expect("the sphere");
        let bolt = g.new_event().expect("bolt slot");
        {
            let e = &mut g.ent[bolt];
            e.class64 = 9;
            e.model65 = 17; // the LEVELED possession
            e.tick70 = 18; // action 18 -> sub_674C0
            e.id24 = 557;
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 400;
            e.f68 = 10;
            e.f69 = 69; // Mana Lock: (10,70) pulse + (10,54) aura
        }
        // Run the pool dry: every remaining slot allocated, the victim
        // and the bolt untouched.
        while g.new_event().is_some() {}
        assert_eq!(g.free.len(), 0, "the free stack is empty");
        assert_eq!(g.ent[victim].class64, 10, "the sphere survives the drain");
        assert_eq!(g.ent[bolt].class64, 9, "the bolt survives the drain");
        let exhausted_before = g.exhausted;

        g.mc2_proj_impact(bolt, victim as u16, &ctx(), None);

        assert!(
            g.exhausted > exhausted_before,
            "the children could not mint (the pool is dry)"
        );
        assert_ne!(
            g.ent[bolt].flags & 0x400,
            0,
            "…and the struck bolt is STILL disabled: `sub_674C0`'s \
             DisableEntityDrawing04 sits outside the child gates \
             (EF:59055) — a surviving bolt is the generic worker's \
             law, not this one's"
        );
    }

    /// ⭐⭐⭐ THE MAGIC MINE SWALLOWS ITS OWNER'S SPELL, AND THE WHOLE
    /// IMPACT TAIL IS THE `else` OF THAT.
    ///
    /// `sub_68AC0` (EF:55397, shipped NETHERW.EXE 0x8D2C0): a
    /// qualifying wizard-owned bolt that contacts its owner's ARMED
    /// (10,78) spawns a BARE `(10,0)` and returns 1, and every flyer
    /// worker folded into [`Gen::mc2_proj_impact`] then runs
    /// `DisableEntityDrawing04_57F10` and NOTHING else — no `_4A190`
    /// effect, no `sub_65780` mail, no `sub_686D0` re-point, no XP
    /// (`sub_65820` EF:62974, `sub_65C20` EF:63177, `sub_662E0`
    /// EF:63533, `sub_66FD0` EF:58814).
    ///
    /// The armed gate `word_0x36_54 == -1` is a ONE-SHOT: the swallow
    /// writes the token's model into it, so the SECOND bolt detonates
    /// normally. Corpus witness mc2l6-rsg t=13555 — retail's slot 178
    /// goes `f36` 65535 → 9 as the human's first charged meteor lands
    /// on it and the rest of the 4-tick barrage flies past.
    #[test]
    fn the_magic_mine_swallows_its_owners_spell_once() {
        let mut g = flat_gen();
        const OWNER: u16 = 343;
        let mine = g.new_event().expect("mine slot");
        {
            let e = &mut g.ent[mine];
            e.class64 = 10;
            e.model65 = 78;
            e.tick70 = 85;
            e.f52 = OWNER; // `word_0x32_50` — HIS mine (EF:59356)
            e.f36 = 0; // armed (retail's −1)
            e.flags |= 8; // `sub_50840`'s collide bit (EF:36969)
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 400;
        }
        let bolt = g.new_event().expect("bolt slot");
        {
            let e = &mut g.ent[bolt];
            e.class64 = 9;
            e.model65 = 3; // in sub_68AC0's ladder
            e.tick70 = 3; // not 1 / 18 / 30
            e.id24 = OWNER;
            e.f40 = 9; // the port's @0x26 lane (spell index)
            e.f68 = 10;
            e.f69 = 17; // the meteor's impact effect (10,17)
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 400;
        }
        let count = |g: &Gen, cm: (u8, u8)| {
            g.ent
                .iter()
                .filter(|e| (e.class64, e.model65) == cm && e.flags & 0x400 == 0)
                .count()
        };
        g.mc2_proj_impact(bolt, mine as u16, &ctx(), None);
        assert_ne!(g.ent[bolt].flags & 0x400, 0, "the bolt is disabled");
        assert_ne!(g.ent[mine].f36, 0, "the mine disarmed itself");
        assert_eq!(count(&g, (10, 0)), 1, "sub_68AC0 spawns its bare (10,0)");
        assert_eq!(
            count(&g, (10, 17)),
            0,
            "and the impact effect is NOT minted — the tail is the else"
        );

        // The one-shot: a second bolt on the now-disarmed mine takes
        // the ordinary impact path.
        let bolt2 = g.new_event().expect("second bolt");
        {
            let e = &mut g.ent[bolt2];
            e.class64 = 9;
            e.model65 = 3;
            e.tick70 = 3;
            e.id24 = OWNER;
            e.f40 = 9;
            e.f68 = 10;
            e.f69 = 17;
            e.x = 40 * 256;
            e.y = 40 * 256;
            e.z = 400;
        }
        g.mc2_proj_impact(bolt2, mine as u16, &ctx(), None);
        assert_eq!(
            count(&g, (10, 17)),
            1,
            "a disarmed mine no longer swallows"
        );
    }

    /// ⭐⭐ THE BEAM'S RAW-ORIGIN EXEMPTION IS NOT A POOL-VICTIM
    /// EXEMPTION — IT IS THE WHOLE BLOCKER ARM'S, THE PLAYER
    /// INCLUDED.
    ///
    /// The generic flyer lands on its victim's z-box CENTRE
    /// (`sub_65580` raise → CopyEntityPosition → `sub_655A0` restore,
    /// EF:62941-43), and for the pose-only human that centre is
    /// `ctx.pz + PLAYER_HH`. A LIGHTNING BEAM does not run that body:
    /// its per-step worker is `sub_66610`, whose blocker arm is one
    /// bare `a1x->position_0x4C_76 = v2x->position_0x4C_76`
    /// (EF:63605-08) with no `sub_65580` bracket anywhere in the
    /// function — and no test on who `v2x` is, because the human's
    /// wizard record is just another pool record there.
    ///
    /// The port carried the exemption on the POOL arm only (landed for
    /// mc2l6-rsg t=463's turret beam) and left the raise standing on
    /// the Player arm, so every beam that terminated on the human
    /// recorded a z exactly 100 high. mc2l22 pair 9993→9994: rival
    /// 530's tier-0 Lightning fires and the (9,9) bolt at slot 944
    /// snaps onto the human — retail z 5015, slot 424's own z that
    /// tick; the port wrote 5115. Eleven segment heads shared the row
    /// (9994, 9999, 10000, 10002, 10006, 13666, 22166, 52931, 53161,
    /// 53946, 53948); mc2l22 135 → 124 segments. Ledger ROUND 103.
    #[test]
    fn a_beam_that_stops_on_the_player_parks_at_his_raw_origin() {
        let hh = crate::mc1::combat::PLAYER_HH as i16;
        let mut z = [0i16; 2];
        for (k, armed) in [false, true].into_iter().enumerate() {
            let mut g = flat_gen();
            let bolt = g.new_event().expect("bolt slot");
            {
                let e = &mut g.ent[bolt];
                e.class64 = 9;
                e.model65 = 9;
                e.tick70 = 9;
                e.id24 = 1;
                e.f68 = 10;
                e.f69 = 23;
                e.x = ctx().px;
                e.y = ctx().py;
                e.z = ctx().pz + 400; // anywhere but the answer
                e.act_life = 4;
            }
            let start = (g.ent[bolt].x, g.ent[bolt].y, g.ent[bolt].z);
            g.mc2_beam_defer.armed = armed;
            g.mc2_proj_land(bolt, &ctx(), start, start, Some(MailTarget::Player));
            g.mc2_beam_defer.armed = false;
            z[k] = g.ent[bolt].z;
        }
        assert_eq!(
            z[0],
            ctx().pz + hh,
            "the generic impact still lands at the player's box centre \
             (sub_65580, EF:62941-43)"
        );
        assert_eq!(
            z[1],
            ctx().pz,
            "sub_66610's blocker arm is a bare position copy \
             (EF:63605-08) — a beam parks at the player's RAW origin"
        );
    }
}
