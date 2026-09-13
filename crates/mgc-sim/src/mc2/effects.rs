//! MC2 class-10 effects band: the smoke-column emitters
//! (10,59)/(10,60) and their (10,13)/(10,14)
//! smoke particles, plus the (10,6) standing ground fire. Trace
//! bank: docs/traces/mc2-class10-m59-m60.md +
//! mc2-class10-m6-m9-m11-m28-m31.md (`EF:` = remc2
//! EventsFunctions.cpp).
//!
//! The emitters are retail's "quest point" smoke columns: invisible,
//! never map-linked, untargetable logic entities that shed one rising
//! smoke particle per tick for 800..899 ticks. The particles carry
//! the visuals (sprite rows 67 / 9, growing through the row band as
//! they rise). Nothing in the emitter family collides, damages, or
//! sounds; the standing fire is the band's damage dealer (per-tick
//! ch0 area heat).

use crate::engine::features::{Gen, lcg32};
use crate::mc1::mobs::MobCtx;

/// A/B toggle for the whole Magic-Mine law cluster (the `sub_67960`
/// carrier shape in `mc2/proj.rs` and `sub_3A8B0`'s arm step here):
/// set `MGC_NO_MC2_MINE` to restore the pre-dig behaviour, where
/// `sub_50840`'s ctor carried the tier lifespan and rolled the arm
/// delay, and the mine never ran an arm sub-state.
pub(crate) fn no_mc2_mine() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_MINE").is_some())
}

/// ⭐⭐⭐ A/B toggle for THE MAGIC MINE'S RETAIL DETONATION — the
/// player ruling that retired `docs/DEVIATIONS.md`'s three "better
/// than retail" choices (the forced 1024 blast box, the `(9,0)` bolt
/// stand-in and the hand-off to the lifespan teardown) together with
/// the `gameplay.patches.mc2_magic_mine` option that gated them.
/// Set `MGC_NO_MINE_RELAUNCH` and the newly-live column goes back to
/// what the RETAIL arm did before the ruling: the owner-death entry
/// guard, the recoil bob, the arm countdown, the proximity scan and
/// `sub_3A8B0` case 5's relaunch are all skipped, so a swallowed mine
/// wakes into state 3 and parks there until its lifespan runs out.
pub(crate) fn no_mine_relaunch() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_RELAUNCH").is_some())
}

/// A/B toggle for the mine ctor's BIRTH POSITION (`sub_50840`
/// EF:36968/36981 — a straight copy of the carrier's own axis): set
/// `MGC_NO_MINE_SPAWN_POS` to restore the port's invented ground snap.
fn no_mine_spawn_pos() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_SPAWN_POS").is_some())
}

/// A/B toggle for the mine's SINK STEP (`sub_3A8B0` case 9): set
/// `MGC_NO_MINE_SINK_STEP` to restore the post-increment counter the
/// remc2 hand-conversion reads, against its own raw Hex-Rays and the
/// recording.
fn no_mine_sink_step() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MINE_SINK_STEP").is_some())
}

/// ⭐⭐⭐ A/B toggle for DIG 98-Q25 — **`sub_28860`'s `case 8` EXPIRY
/// WRITES THE ACTION AND KEEPS RUNNING.**
///
/// The Cymmerian brain (`sub_28860`, EF:18828) has TWO places that
/// send the creature to state 204 and they are NOT the same
/// statement:
///
/// * the death arm — `if (v2 == 2) { a1x->actionIndex_0x45_69 = 204; }`
///   (EF:18899-18902) — is the `if` half of an `if/else`, so it skips
///   the whole `byte_0x46_70` switch, the wander draw, `sub_1B8C0`
///   and the water-sprite swap. The port models this correctly.
/// * `case 8`'s countdown expiry — `v22 = a1x->dword_0x10_16 - 1;
///   a1x->dword_0x10_16 = v22; if (v22 < 0) a1x->actionIndex_0x45_69
///   = 204; break;` (EF:19003-19008) — is inside the `else` arm and
///   ends in a plain `break`. **The wander, the move, the water swap
///   and the speed reset all still run on that very tick.**
///
/// The port had copied the death arm's early `return` onto the
/// countdown arm, so an expiring `case 8` Cymmerian froze in place
/// for the tick retail spends moving. mc2l22 t=2669 is the witness:
/// retail's slots 957 and 971 both step `scratch10 0 -> -1`,
/// `action45 200 -> 204` **and** x/y/z, while the port kept x/y/z at
/// their t=2668 values.
///
/// Set `MGC_NO_M25_C8_FALLTHROUGH` to restore the invented `return`.
pub(crate) fn no_m25_c8_fallthrough() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_M25_C8_FALLTHROUGH").is_some())
}

/// ⭐⭐⭐ ROUND 98 — **THE RECORDED MOVE BYTE IS RETAIL'S OWN
/// POST-LATCH COMMAND WORD, SO THE PORT MUST NOT RE-DERIVE A PRESS
/// EDGE FROM IT.**
///
/// `sub_5F380`'s whole cast tail is three flat `testb`/`call` pairs
/// on `dword_0xA4_164x->entityIndex_0x0` — the consumed command word
/// — with **no press latch, no previous-frame register and no edge
/// test of any kind**. Byte-for-byte in the shipped `NETHERW.EXE`
/// (file `0x83D7D-0x83DED`, linear `0x5F57D-0x5F5ED` by the code rule
/// `0x34800 + linear − 0x10000`; the call target `0x83E60` is
/// `sub_5F660` and `0x1a3e4` is `Entities_EA3E4`):
///
/// ```text
/// 83d53  f6 00 10           testb $0x10,(%eax)   ; LEFT  fire bit
/// 83d56  74 25              je    0x83d7d
/// 83d75  e8 e6 00 00 00     call  0x83e60        ; sub_5F660(caster, token, 256)
/// 83d7d  8b 83 a4 00 00 00  mov   0xa4(%ebx),%eax
/// 83d83  f6 00 20           testb $0x20,(%eax)   ; RIGHT fire bit
/// 83d86  74 25              je    0x83dad
/// 83da5  e8 b6 00 00 00     call  0x83e60        ; sub_5F660(caster, token, 512)
/// 83dad  8b 83 a4 00 00 00  mov   0xa4(%ebx),%eax
/// 83db3  f6 00 40           testb $0x40,(%eax)   ; the CYCLE-RING bit
/// 83de1  e8 7a 00 00 00     call  0x83e60
/// 83ded  c3                 ret
/// ```
///
/// The press/repeat decision lives one layer up, in
/// `HandleMouseButtons_18F80` (PI:2027-76), whose OUTPUT is that word
/// — and that word is exactly what the recorder captures as
/// `players[].move_bits` (`recover_pair_mc2`'s `mb`, bits 0x10/0x20).
/// So when the port is driven by a recording it must call the gate on
/// the RAW BIT and let `sub_5F660`'s own per-model refusal do the
/// rest; re-running the latch on top of an already-latched word is
/// DOUBLE LATCHING, and it silently drops every press that lands on
/// the frame after another press.
///
/// mc2l22 t=3424 is the witness. The human's possession token (slot
/// 426, `byte_0x3B_59 == 1`, the CLICK-ONLY family) records the
/// command word carrying `0x20` on t=3423 **and** t=3424; at 3423 the
/// token is still armed (`word_0x2E_46` 1) so retail's gate refuses,
/// and at 3424 it is 0 so retail arms (`word_0x2E_46` 0 → 2 after the
/// same-tick countdown) and launches the `(9,1)` at slot 935. The
/// port's `edge = fire && !prev_fire` was true at 3423 and **false at
/// 3424**, so it never cast: `wiz 0 charge` retail 0 / port 3, one
/// allocation short, and the whole tick's slot assignment slid — the
/// arrow took 935, the trail's scorch ring took 874 and retail's
/// `(10,11)` at 847 was never minted, which IS the wall signature
/// `missing(10,11)slot847x1`.
///
/// Scoped to `strict_retail` — the conformance import/replay seat — so
/// NATIVE play keeps modelling `HandleMouseButtons_18F80` itself (it
/// owns a real mouse there, not a recorded command word).
///
/// Set `MGC_NO_MC2_COMMAND_WORD_CAST` to restore the double latch.
pub(crate) fn no_mc2_command_word_cast() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_COMMAND_WORD_CAST").is_some())
}

impl Gen {
    // ---- ctors ---------------------------------------------------------------

    /// `ArriveCheckpoint_4EB50` / `AddSmoke_4EC10` (EF:35663/:35685)
    /// — the (10,59)/(10,60) emitter, byte-identical bodies. Gated on
    /// ≥32 free pool slots (`sub_4A810`); TWO entity-RNG draws (life
    /// 800..899, particle-speed bonus 0..16); NEVER map-linked and
    /// carries no sprite — invisible by construction, like the
    /// class-11 volumes.
    pub(crate) fn mc2_spawn_smoke_emitter(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        if self.free.len() < 32 {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = if model == 59 { 0x40 } else { 0x41 };
        }
        let d = self.mc2_rand(i);
        self.ent[i].max_life = d % 0x64 + 800;
        // byte[0] = (&0xF6)|1: bit 0 set, bit 3 (targetable) cleared.
        self.ent[i].flags = (self.ent[i].flags & !0x8) | 1;
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            e.f126 = (d % 0x11) as i16; // actSpeed = the speed bonus
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        Some(i)
    }

    /// `SetSmoke4_4EAA0` (EF:35639) — the shared smoke-particle body:
    /// state = model (13/14), ONE entity-RNG draw (actSpeed 51..103),
    /// maxSpeed 30, xtype 10/xsubtype = model, map-linked, half-speed
    /// sprite. Flag ops ≡ the (10,0) fire ctor (`dword &= 0xFFFDFFF7`
    /// then `byte[2] |= 2`).
    pub(crate) fn mc2_spawn_smoke_particle(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
        life: u32,
        sprite: u16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = model;
            e.max_life = life;
            e.f130 = 30; // maxSpeed
            e.f66 = 10; // xtype
            e.f67 = model; // xsubtype
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        let d = self.mc2_rand(i);
        self.ent[i].f126 = (d % 0x35 + 51) as i16; // actSpeed = rise rate
        self.link(i, x, y, z);
        self.mc2_set_sprite(i, sprite);
        self.refill_life(i);
        Some(i)
    }

    /// `SetParticleSmoke3B_4E9E0` / `SetParticleSmoke3C_4EA20` /
    /// `sub_4EA60` (EF:35618/:35625/:35632) — the per-model wrapper:
    /// ONE global-RNG draw for the life roll (m13: 17..39 sprite 67;
    /// m14: 28..60 sprite 9; m87 = the THIRD PUFF, m13's roll and
    /// sprite under its own action 0x5E —
    /// docs/traces/mc2-class10-m29-m5-m13.md §4.3). The roll only
    /// survives on direct (authored) spawns — the emitter overwrites
    /// it to 32.
    pub(crate) fn mc2_spawn_smoke_particle_for(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let g = lcg32(&mut self.rand);
        let (life, sprite) = match model {
            13 | 87 => (g % 0x17 + 17, 67),
            _ => (g % 0x21 + 28, 9),
        };
        let i = self.mc2_spawn_smoke_particle(model, x, y, z, life, sprite)?;
        if model == 87 {
            self.ent[i].tick70 = 0x5E; // action != model for the third puff
        }
        Some(i)
    }

    /// `NewAdd0A05_4E570` (EF:35436) — the (10,5) water splash: life
    /// 8, sprite 244, snapped to the water surface, no RNG, no
    /// motion. Flag ops ≡ the fire/smoke ctors.
    pub(crate) fn mc2_spawn_splash(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 5;
            e.tick70 = 5;
            e.max_life = 8;
            e.f44 = 0; // subSpellIndex = 0
            e.f26 = 0;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(lx, ly) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 244);
        Some(i)
    }

    /// `NewAdd0A0C_4E8C0` / `NewAdd0A46_4E950` (EF:35573/:35595) —
    /// the POSSESSION CLAIM PULSE: byte-identical ctors under two
    /// models, (10,12) action 12 = the WEAK claim and (10,70) action
    /// 0x4D = the FORCED steal. This is the entity that actually
    /// DELIVERS possession (docs/traces/mc2-possession-delivery.md
    /// §2): the bolt only spawns it, and the pulse then broadcasts
    /// the claim channel from its own tick over its whole 9-tick
    /// window (life 8 under the class-10 pre-decrement).
    ///
    /// `subSpellIndex_0x2A_42` = 64000 (f44), sprite 41, box 512³ via
    /// `SetEntityShiftRot_49EA0(512, 512)` — that box is what gives
    /// `sub_112D0` its ±2-tile cell scan, so a near miss still
    /// claims. `byte[0] = (b & 0xF6) | 1` clears the targetable bit
    /// and sets bit 0; the map link then ors bit 2 (recorded flags =
    /// 5). No RNG draw.
    ///
    /// Ordering matters for the field lanes: `SetEntityIndexAndRot_
    /// 49CD0(41)` runs BEFORE the ShiftRot, so the row-41 half
    /// rot-speed survives in the applied_yaw lane (@0x52) while
    /// pitch/roll/fov take 512 — the recorded fingerprint (mc2l4
    /// t=23 slot 309: ayaw 125, apitch/aroll/afov 512, max_life 8,
    /// @0x2A 64000, flags 5, sprite 41).
    pub(crate) fn mc2_spawn_claim_pulse(
        &mut self,
        x: u16,
        y: u16,
        z: i16,
        forced: bool,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = if forced { 70 } else { 12 };
            e.tick70 = if forced { 0x4D } else { 12 };
            e.max_life = 8;
            e.f44 = 64000; // subSpellIndex_0x2A_42
            e.flags = (e.flags & !9) | 1;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        self.mc2_set_sprite(i, 41);
        self.mc2_shift_rot(i, 512, 512);
        Some(i)
    }

    /// `sub_32120` (EF:23559, action 0x4D) — the (10,70) FORCED claim
    /// pulse's tick: `PossesHitMana_320E0`'s twin, differing only in
    /// `sub_112D0`'s force flag (1 = steal past the `byte[2]&0x20`
    /// claim lock and set it). Body order is load-bearing and shared
    /// with the weak pulse: the @0x10 counter bumps BEFORE the life
    /// test (so it counts on the death tick too), then the class-10
    /// pre-decrement, then anim + broadcast. The weak (10,12) pulse
    /// rides the shared `possess_flash_tick` (mc1/combat.rs) — the
    /// same law with force 0 — via the class-10 action-12 dispatch.
    pub(crate) fn mc2_steal_pulse_tick(&mut self, i: usize, ctx: &MobCtx) {
        self.ent[i].f26 = self.ent[i].f26.wrapping_add(1);
        let life = self.ent[i].act_life;
        self.ent[i].act_life = life - 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        self.anim_advance(i);
        // EF:4200 — the ch1 mail AMOUNT carries retail's
        // `dword_0x64_100` force flag; no consumer reads a ch1 damage.
        self.area_write(i, 1, 1, ctx, false, false);
    }

    /// `CreateManaSphere512_50080` / `CreateManaSphere2560_500A0`
    /// (EF:36595/:36601, both thunks into `CreateManaSphere_500C0`
    /// EF:36607) — the authored ground mana economy: (10,39) = the
    /// 512-mana sphere, (10,58) = the 2560 variant (strA1 rows
    /// 0x27/0x3A; the created entity is ALWAYS model 39, action 0x29
    /// — the m59-m60 §8 numbering note) — plus (10,57) = the
    /// RANDOM-VALUE sphere (`sub_50130` EF:36631, its own model with
    /// action 0x3E; docs/traces/mc2-class10-m57.md): mana = one draw
    /// of the sphere's own stream `% 0x7D0` = 0..1999. Unowned (the
    /// neutral 52 sprite family). All ride the shared MC1 ball
    /// machinery exactly like the death-drop spheres (mobs.rs
    /// module-doc APPROX: the MC2 action-0x29/0x3E tick columns are
    /// unported; the MC1 (10,39) ball tick rests/flies/claims them —
    /// m57's AI-avoidance gate `word_0x244_580` rides the same
    /// APPROX).
    pub(crate) fn mc2_spawn_mana_sphere(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.spawn_mana_ball(x, y, z)?;
        self.ent[i].f140 = match model {
            58 => 2560,
            57 => (self.ent_rand(i) % 0x7D0) as i32,
            _ => 512,
        };
        if model == 57 {
            // `sub_50130` builds its OWN entity: model **57** with
            // action **0x3E**, where every other sphere ctor takes
            // model 39 / action 0x29 (m57 trace §1.2 / the strA0 row
            // 62 → `sub_35FB0` EF:26318). `spawn_mana_ball` stamps
            // the (10,39) family for the whole sphere line, so the
            // m57 arm has to put its own model back — retail's
            // per-model sphere gates all key on it:
            //
            // - the class-10 scan chain `dword_38523` is built from
            //   models 39, 40 AND 57 (EF:40023-40062), so every
            //   consumer that walks it without a model test sees an
            //   m57 (awake pass EF:55489, the aura pull EF:28362, the
            //   Vissuluth ritual broadcasts EF:12848/13049);
            // - the consumers that DO test `model == 39` are exactly
            //   the ones a fool's sphere must dodge: castle absorb
            //   (EF:61105), the balloon fleet target (EF:61011), the
            //   m23 siphon's find/validate (EF:18396), the dead
            //   wizard's sphere re-point (EF:60174);
            // - the census admits 39/45/58 and drops 57 outright
            //   (EF:62012-62035);
            // - the rival mana hunt walks 39/40 FIRST and the 57s in
            //   a second pass, where a Perception roll breaks the
            //   whole walk (EF:6544-49).
            //
            // With the model native the action is belt-and-braces
            // (an IMPORTED m57 has always carried model 57), but it
            // stays: `ball_tick`'s trap gate accepts either lane and
            // the action is what the m57 physics column keys on.
            self.ent[i].model65 = 57;
            self.ent[i].tick70 = 62;
            // …AND ITS OWN `xsubtype`. `sub_50130` stamps
            // `xtype_0x41_65 = 10` / **`xsubtype_0x42_66 = 57`**
            // (EF:36638-39; shipped EXE file 0x7494f/0x74953 `mov
            // BYTE PTR [eax+0x41],0xa` / `mov BYTE PTR [eax+0x42],
            // 0x39`), where the shared `CreateManaSphere_500C0` takes
            // 39 (EF:36615). That pair is the PARAMETER the merge
            // partner search `sub_10A50` matches candidates against
            // (EF:3908-15), so leaving the shared 39 here made a
            // natively-spawned fool's sphere hunt m39 balls and be
            // hunted by them. An IMPORTED m57 has always carried 57
            // on this lane (`b42` → `f67`); this is the native half.
            // (under the same `MGC_NO_M57_MERGE` A/B arm as the merge
            // law it feeds — `crate::mc1::combat::no_m57_merge`.)
            if !crate::mc1::combat::no_m57_merge() {
                self.ent[i].f67 = 57;
            }
            // ⭐ THE BANKED HEAD, PAID (round 99, dig 99-3).
            // `sub_50130` also stamps `byte_0x43_67 = 10` /
            // `byte_0x44_68 = 1` (EF:36642-44), the two statements the
            // shared `CreateManaSphere_500C0` (EF:36607) does NOT have
            // — the sibling-pair split. The port homes them in
            // `f68`/`f69`, and the (10,57) never reaches the class-9
            // impact tail that reads that pair, so the lane is inert
            // in the port and pure conformance state: `b44` was the
            // ONLY ≠ row on the mc2l6-rsg free-run dumps of BOTH
            // walls (slot 462 at t=26502, slot 614 at t=26510).
            // `MGC_NO_M57_IMPACT_PAIR=1` reverts.
            if !crate::mc1::combat::no_m57_impact_pair() {
                self.ent[i].f68 = 10;
                self.ent[i].f69 = 1;
            }
            // ⭐ AND THE THIRD STATEMENT OF THE SPLIT: the RECLAIMABLE
            // bit. `sub_50130` closes with `byte[2] |= 2` — the
            // recycle-victim membership the allocator's `sub_49F90`
            // rebuild scans for (`Gen::rebuild_recycle(0x2_0000)`) —
            // and `CreateManaSphere_500C0` does NOT have it, so every
            // authored/economy sphere is unreclaimable and only the
            // FOOL'S sphere is sacrificeable. Shipped `NETHERW.EXE`:
            //     749a1: 8a 63 0e  mov  0xe(%ebx),%ah
            //     749a9: 80 cc 02  or   $0x2,%ah
            //     749ad: 88 63 0e  mov  %ah,0xe(%ebx)
            // (no byte[0] bit-3 clear — retail's live m57 reads
            // flags 0x2000C, bits 2/3/17, confirmed on the rsg
            // free-run dumps of slots 462 and 614).
            // `MGC_NO_M57_RECLAIM_BIT=1` reverts.
            if !crate::mc1::combat::no_m57_reclaim_bit() {
                self.ent[i].flags |= 0x2_0000;
            }
            // The m57 ctor's own zero (sub_50130 EF:36643), not the
            // shared (10,39) ctor's 32 (EF:36618) — mc2l30 t=216:
            // five m57 births at speed 0 where the port kept 32.
            self.ent[i].f126 = 0;
        }
        self.ent[i].f144 = 0;
        self.ball_resize(i);
        Some(i)
    }

    /// `NewAdd0A06_4E5F0` (EF:35458) — the (10,6) STANDING GROUND
    /// FIRE, the real damaging self-sustaining flame: 240-tick life,
    /// per-tick ch0 area heat of 50 (subSpell home = f140,
    /// the class-10 effect column's amount field like the (10,0)
    /// fire), sprite 228 with ShiftRot(272, 1536), z snapped to
    /// terrain + the `word_0x2C_44` lift (f44 — runtime spawners
    /// raise it; the ctor zeroes it, overriding NewEvent's 100).
    /// NOT targetable (byte[0] bit 3 cleared — fire cannot be
    /// attacked); byte[2] bit 1 set (reclaimable). No RNG.
    ///
    /// APPROX register: `AddEvent2_847D0(80, 11, 1)` — the
    /// Night/Cave dynamic light registration — is presentation,
    /// unported (the same note as the (10,1) big explosion).
    pub(crate) fn mc2_spawn_fire6(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 6;
            e.tick70 = 6;
            e.f140 = 50; // subSpellIndex = the per-tick ch0 amount
            e.max_life = 240;
            e.f44 = 0; // word_0x2C_44 = the z lift
            e.f26 = 0; // dword_0x10_16 = the grow/shrink step
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(lx, ly) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 228);
        self.mc2_shift_rot(i, 272, 1536);
        Some(i)
    }

    /// `sub_50840` (EF:36960) — the Magic Mine (spell 23) persistent
    /// proximity mine `(10,78)`: sprite 66, SOLID, born exactly where
    /// the (9,29) carrier died. Placed by the carrier's landing
    /// (`mc2_proj_impact` `(10,78)`); the owner is stamped by the
    /// impact tail into `word_0x32_50` (f52), NOT into `id_0x1A_26` —
    /// the mine's own `@0x1A` stays `NewEvent`'s own-slot default.
    /// Ticks via [`Gen::mc2_mine_tick`] (action 0x55) —
    /// docs/spell-audit/magic-mine.md.
    ///
    /// ⭐⭐ **THE MINE'S LIFESPAN IS A FLAT 1000 — THERE IS NO TIER
    /// LIFESPAN.** `sub_50840` writes `maxLife_0x4 = 1000` as a literal
    /// (EF:36965) and `CopyMaxLifeToLife_49A20` mirrors it; the only
    /// per-cast values its caller `sub_67960` passes down are
    /// `word_0x32_50` (owner), `byte_0x46_70 = 0` and
    /// `subSpellIndex_0x2A_42` (EF:59356-59). Feeding the carrier's
    /// `subSpellIndex` in as a "1000/5000/10000 tier lifespan" was an
    /// INVENTED WRITE: on mc2l6-rsg pair 13050→13051 the carrier's
    /// payload is 0, so slot 775 was born `max_life 1 / life 0` against
    /// retail's 1000 / 1000.
    ///
    /// ⭐⭐⭐ **AND IT IS BORN AT THE CARRIER'S POSITION, NOT ON THE
    /// GROUND.** `sub_50840`'s ctor is `event->position_0x4C_76 =
    /// *position` followed by `AddEventToMap_57D70(event, position)`
    /// (EF:36968/36981) — a straight copy of the axis its caller
    /// `sub_67960` hands it, which is the CARRIER's own
    /// `position_0x4C_76` after that tick's move (EF:59354). The port
    /// looked the ground height up and linked there instead, so every
    /// mine started ~2000 units low and the float above spent the rest
    /// of the take climbing. mc2l6-rsg t=13832 slot 767 is the witness
    /// and it is that take's whole free-run wall: retail records the
    /// new mine at `z` **3664** (the carrier's 3712 less this same
    /// tick's one −48 float step), the port at **1718** (ground 1670
    /// plus one +48 step). `MGC_NO_MINE_SPAWN_POS` restores the snap.
    ///
    /// ⭐ AND THE CTOR ROLLS NO RNG. `sub_50840` is fourteen straight
    /// field writes with no `rand` anywhere; the arm delay is rolled by
    /// sub-state **2** off the record's own stream (EF:29926-31).
    ///
    /// ⚠ FIELD HOMES. `byte_0x43_67` (the burst budget) is **f68** and
    /// `byte_0x44_68` (the recoil counter) is **f69** — the `b43`/`b44`
    /// lanes `port_ent_lanes_mc2` publishes and `import_ent_mc2` seats.
    /// `subSpellIndex_0x2A_42` (the TIER INDEX) is **f44**, which is
    /// what the importer seats for a (10,78) (`c10_2a_in_f140` excludes
    /// it on purpose) — the native path used to write the tier into
    /// f140 and the burst budget into f44, i.e. both lanes inverted
    /// against the importer. Same witness: retail `f2a` 1 / `b43` 2,
    /// port `f2a` 2 / `b43` 10 (the `NewEvent` default, never written).
    pub(crate) fn mc2_spawn_magic_mine(
        &mut self,
        x: u16,
        y: u16,
        z: i16,
        tier: u8,
        lifespan: i32,
    ) -> Option<usize> {
        let i = self.new_event()?;
        let revert = no_mc2_mine();
        let at_z = if no_mine_spawn_pos() {
            self.ground_z(x, y) as i16
        } else {
            z
        };
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 78;
            e.tick70 = 85; // action 0x55 = sub_3A8B0
            e.max_life = if revert {
                lifespan.max(1) as u32
            } else {
                1000 // EF:36965, a literal
            };
            // `byte[0] |= 8` (EF:36969) — the mine is SOLID, which is
            // what lets `sub_10780` hand it to `sub_68AC0`.
            e.flags = (e.flags & !0x2_0000) | 0x2_0008;
        }
        self.link(i, x, y, at_z);
        self.refill_life(i);
        if revert {
            self.ent[i].f44 = 1u16 << tier.min(3);
            let r = self.ent_rand(i);
            self.ent[i].f26 = ((r % 0x32) + 16) as i16;
            self.mc2_set_sprite(i, 66);
            return Some(i);
        }
        {
            let e = &mut self.ent[i];
            e.f44 = 0; // `subSpellIndex_0x2A_42 = 0` (EF:36970)
            e.f36 = 0; // `word_0x36_54 = -1` — ARMED (port: f36 == 0)
            e.f46 = 1; // `fontTypeIndex_0x3D_61 = 1` (EF:36973)
            e.f68 = 1; // `byte_0x43_67 = 1` (EF:36974)
            e.f69 = 0; // `byte_0x44_68 = 0` (EF:36975)
            e.f52 = i as u16; // `word_0x32_50 = own slot` (EF:36976)
        }
        self.mc2_set_sprite(i, 66);
        Some(i)
    }

    /// `sub_3A8B0` (EF:29749), class-10 action 0x55 — the Magic Mine
    /// tick, a nine-state machine on `byte_0x46_70` (our `f71`).
    ///
    /// ⭐⭐⭐ **THE TRIGGER IS NOT DEAD AND THE DETONATION IS A
    /// RELAUNCH OF THE SWALLOWED SPELL.** `docs/DEVIATIONS.md` used to
    /// carry three "better than retail" inventions here — a forced
    /// 1024-unit blast box, a `(9,0)` bolt, and a hand-off to the
    /// lifespan teardown — all adopted on the ruling that retail ships
    /// this lane dead. It does not: `sub_68AC0` writes the armed gate
    /// (`Gen::mc2_mine_swallow`), and case 5 below is retail's own
    /// detonation. **A mine fires back WHATEVER SPELL YOU SHOT INTO
    /// IT, at that spell's own tier**, owned by the mine's owner and
    /// aimed at whoever tripped it. The three inventions are retired.
    ///
    /// The states, verbatim:
    /// - **entry** (EF:29793-98) — the owner (`Entities[word_0x32_50]`)
    ///   dead or reap-flagged ⇒ `DisableEntityDrawing04_57F10`;
    /// - **the recoil bob** (EF:29800-38) — with `byte_0x44_68` live
    ///   the mine sits `{0,153,307,445,491,512}[|b44|]` off its anchor
    ///   along its own yaw, and the counter ramps 1..6 then flips to −5;
    /// - **the countdown + float** (EF:29840-72), skipped in 7 and 9:
    ///   `life--` into state 6 at zero, clamp up out of the ground,
    ///   then ±48 toward ground+1024 with a 96-unit deadband;
    /// - **0** (EF:29886-908) — re-arm to the TIER's lifespan, latch
    ///   the anchor, seed the burst budget, → 1;
    /// - **1** (EF:29910-16) — PARK until something writes
    ///   `word_0x36_54`; `sub_68AC0`'s swallow is that writer;
    /// - **2** (EF:29918-32) — go non-solid, take the owner off
    ///   `@0x32`, size the burst from the swallowed tier's
    ///   `fontType_0x1B`, roll the 16..65 arm delay, → 3;
    /// - **3** (EF:29933-39) — the arm countdown, → 4;
    /// - **4** (EF:29940-59) — every 16th of the record's OWN frames,
    ///   the nearest class-3 model ≤ 1 within 3584 (3-D) on the
    ///   tick-top roster `dword_38519`, excluding the owner, → 5;
    /// - **5** (EF:29960-30042) — the relaunch, below;
    /// - **6/7/9** (EF:30043-86) — hang, pause, sink, puff.
    ///
    /// ⚠ NOT PORTED (cosmetic, and ungraded on the class-10 lanes):
    /// the draw-bit block at EF:29850-61, which hides an enemy's mine
    /// unless the viewer holds a live `SpellsEnabled[12]`.
    pub(crate) fn mc2_mine_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // EF:29793-98 — the mine belongs to its owner's life. A mine
        // whose owner has died or been reap-flagged stops drawing and
        // goes; `word_0x32_50` is the ctor's own slot until the
        // carrier's tail stamps the caster, so a 0 here is not a
        // resolvable owner and the guard sits out.
        if !no_mine_relaunch() {
            let own = self.ent[i].f52;
            let gone = if own == crate::mc1::mobs::PLAYER_TARGET {
                ctx.pdead
            } else if own == 0 {
                false
            } else {
                self.ent
                    .get(own as usize)
                    .is_none_or(|o| o.act_life < 0 || o.flags & 0x400 != 0)
            };
            if gone {
                self.ent[i].flags |= 0x400;
                return false;
            }
            // EF:29800-38 — the recoil bob. Off the LATCHED anchor
            // (`axis_0x9A_154x`, our dest_x/dest_y/site_z) along the
            // mine's own yaw, keeping the live z; the counter ramps
            // 1..6 and then flips to −5 for the return swing.
            let b44 = self.ent[i].f69 as i8;
            if b44 != 0 {
                let step: i16 = match b44.unsigned_abs() {
                    1 => 0,
                    2 => 153,
                    3 => 307,
                    4 => 445,
                    5 => 491,
                    _ => 512,
                };
                let (ax, ay, yaw, z) = {
                    let e = &self.ent[i];
                    (e.dest_x, e.dest_y, e.f30, e.z)
                };
                let mut pos = (ax, ay, z);
                Self::polar_step(&mut pos, yaw, 0, step);
                // `CopyEntityPosition_57CF0` — a relinking move, not a
                // `link` (which no-ops on an already-linked record).
                self.move_relink(i, pos.0, pos.1, z);
                let n = b44.wrapping_add(1);
                self.ent[i].f69 = if n > 6 { -5i8 as u8 } else { n as u8 };
            }
        }
        // EF:29840-45 — sub-states 7 and 9 skip the lifespan countdown.
        // The expiry test is post-decrement `<= 0`, and it enters the
        // teardown rather than despawning; the switch below then runs
        // sub-state 6 on this SAME tick.
        if self.ent[i].f71 != 7 && self.ent[i].f71 != 9 {
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life <= 0 {
                self.ent[i].f71 = 6;
            }
            // EF:29862-72 — the mine clamps UP out of the ground, then
            // FLOATS toward ground + 1024 in +/-48 steps with a 96-unit
            // deadband (gated on f69 == 0). Player-observed in retail:
            // the mine rises to roughly castle-tower height rather than
            // resting on the ground, which is where ours sat because
            // this whole block was missing.
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            let g = self.ground_z(x, y);
            if (self.ent[i].z as i32) < g {
                self.ent[i].z = g as i16;
            }
            let target = g + 1024;
            let delta = self.ent[i].z as i32 - target;
            if self.ent[i].f69 == 0 && delta.abs() > 96 {
                let step = if delta <= 0 { 48 } else { -48 };
                self.ent[i].z = (self.ent[i].z as i32 + step) as i16;
            }
        }
        // ⭐⭐ SUB-STATE 0 IS THE ARM STEP, AND IT IS WHERE THE TIER
        // LIFESPAN LIVES — NOT IN THE CTOR. `sub_3A8B0` case 0
        // (EF:29886-908) runs on the mine's VERY FIRST tick (retail's
        // ascending walk reaches the fresh slot in the same frame the
        // carrier minted it) and writes
        //     maxLife = life = SPELLS[23].subspell[subSpell].subSpellIndex_2
        //     axis_0x9A = position
        //     byte_0x43_67 = {1,2,4,8}[ SPELLS[23].subspell[..].life_0x1A ]
        //     byte_0x46_70 = 1
        // So a mine is born at `sub_50840`'s flat 1000 and re-armed to
        // its tier's lifespan one tick later — which is exactly what
        // the recording shows: mc2l6-rsg t=13832, slot 767 surfaces at
        // life **5000** / max_life **5000**, having already paid the
        // countdown above and been re-armed by this block, with
        // `b43` **2** (tier 1's `life_0x1A` is 1) and its anchor
        // `dest_*` latched to the birth position.
        if self.ent[i].f71 == 0 && !no_mc2_mine() {
            let sub = (self.ent[i].f44 as usize).min(2);
            if let Some(row) = self.assets.spells.get(23).copied() {
                let span = row.tiers[sub].sub_spell.max(0) as u32;
                self.ent[i].max_life = span;
                self.ent[i].act_life = span as i32;
                self.ent[i].f68 = match row.tiers[sub].life {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    3 => 8,
                    _ => self.ent[i].f68,
                };
            }
            // `a1x->axis_0x9A_154x = a1x->position_0x4C_76` (EF:29890)
            // — the anchor the recoil bob swings around.
            let e = &mut self.ent[i];
            e.dest_x = e.x;
            e.dest_y = e.y;
            e.site_z = e.z;
            e.f71 = 1;
        }
        match self.ent[i].f71 {
            // EF:29910-13 — sub-state 1 PARKS until something writes
            // the armed gate; `sub_68AC0` (the swallow) is that
            // writer. Before the swallow was ported, nothing ever
            // wrote it and the mine never left this state.
            // ⚠ Gated on the SWALLOW's own switch: parking here with
            // no writer for `f36` would strand the mine forever, so
            // the two halves must arm and disarm together.
            1 if !crate::mc2::proj::no_mine_swallow() => {
                if self.ent[i].f36 != 0 {
                    self.ent[i].f71 = 2;
                }
                false
            }
            // EF:29918-32 — the mine WAKES: drop the collide bit, take
            // the owner id off `@0x32`, size the burst from the
            // SWALLOWED tier's `fontType_0x1B`, and roll the arm delay.
            // The draw is `(rand % 0x32) + 16` off the record's own
            // stream and it is the mc2l6-rsg free-run wall to the
            // digit: 9377*50718 + 9439 mod 65536 = 62909 = retail's
            // slot-178 `rand` at t=13557.
            2 if !crate::mc2::proj::no_mine_swallow() => {
                self.ent[i].flags &= !8;
                self.ent[i].id24 = self.ent[i].f52;
                let (spell, tier) = self.mc2_mine_charge(i);
                let ft = self
                    .assets
                    .spells
                    .get(spell)
                    .map_or(0, |r| r.tiers[tier].font_type);
                self.ent[i].f46 = if ft & 1 != 0 { 6 } else { 1 };
                let r = self.ent_rand(i);
                self.ent[i].f26 = ((r % 0x32) + 16) as i16;
                self.ent[i].f71 = 3;
                false
            }
            // EF:29933-39 — the arm countdown.
            3 if !no_mine_relaunch() => {
                let v = self.ent[i].f26.wrapping_sub(1);
                self.ent[i].f26 = v;
                if v == 0 {
                    self.ent[i].f71 = 4;
                }
                false
            }
            // EF:29940-59 — the proximity scan, every 16th of the
            // record's OWN frames (`byte_0x3E_62 & 0xF`, our f63 — NOT
            // the lifespan, which is what the port's reconstruction
            // threw at it). The candidate set is the tick-top class-3
            // roster `dword_38519` filtered to `model_0x40_64 <= 1`
            // (wizards, never castles), the owner excluded BY SLOT, the
            // metric `sub_583F0`'s 3-D distance under 3584, NEAREST
            // wins with a strict `<` off a 0x10000 seed.
            4 if !no_mine_relaunch() => {
                if self.ent[i].f63 & 0xF == 0 {
                    if let Some(v) = self.mc2_mine_scan(i, ctx) {
                        self.ent[i].f71 = 5;
                        self.ent[i].f146 = v;
                    }
                }
                false
            }
            // EF:29960-30042 — the detonation.
            5 if !no_mine_relaunch() => {
                self.mc2_mine_detonate(i, ctx);
                false
            }
            // EF:30043-54 — clear the draw bit and wait for f69.
            6 => {
                self.ent[i].flags &= !1;
                if self.ent[i].f69 == 0 {
                    self.ent[i].f71 = 7;
                    self.ent[i].f26 = 10;
                }
                false
            }
            // EF:30055-62 — the 10-tick pause before the sink.
            7 => {
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 == 0 {
                    self.ent[i].f71 = 9;
                    self.ent[i].f26 = 3;
                }
                false
            }
            // EF:30073-85 — the accelerating sink, then the puff.
            //
            // ⭐⭐⭐ THE SINK STEPS BY THE **PRE**-INCREMENT COUNTER, AND
            // THE DECOMPILE'S CONVERTED LINE HAS IT BACKWARDS. remc2's
            // hand-conversion reads `dword_0x10_16++; z -= 32 *
            // dword_0x10_16;` but its own commented-out raw Hex-Rays
            // above it (EF:30074-76) reads
            //     `v12 = dword_0x10_16; dword_0x10_16 = v12 + 1;
            //      LOWORD(v12) = z - 32 * v12;`
            // — the OLD value. The recording settles it: mc2l6-rsg
            // t=14025→14026, slot 772 steps `scratch10` 3 → 4 while
            // `z` goes 3466 → **3370**, a −96 = 32×3 step, not the
            // −128 the converted line predicts. (THE DECOMPILE IS NOT
            // RAW DECOMPILER OUTPUT — remc2-source-corruption-class.)
            //
            // ⭐ AND THE SHIPPED EXE SETTLES IT INDEPENDENTLY.
            // `NETHERW.EXE` file `0x5F654` (linear `0x3AE54`):
            //     8b 43 10   mov  0x10(%ebx),%eax   ; the counter
            //     89 c2      mov  %eax,%edx         ; edx = the OLD value
            //     40         inc  %eax
            //     89 43 10   mov  %eax,0x10(%ebx)   ; store the new one
            //     c1 e2 05   shl  $0x5,%edx         ; edx = OLD * 32
            //     0f bf 43 50 movswl 0x50(%ebx),%eax ; z
            //     29 d0      sub  %edx,%eax         ; z -= OLD * 32
            // The copy to `edx` precedes the `inc`. THE SHIPPED EXE
            // OUTRANKS THE LISTING.
            9 => {
                let n = self.ent[i].f26;
                self.ent[i].f26 = n + 1;
                let step = 32 * if no_mine_sink_step() { n + 1 } else { n } as i32;
                self.ent[i].z = (self.ent[i].z as i32 - step) as i16;
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                let g = self.ground_z(x, y);
                if self.ent[i].z as i32 >= g {
                    return false;
                }
                self.ent[i].z = g as i16;
                let model = if self.on_water_pub(x, y) { 5 } else { 0 };
                let z = self.ent[i].z;
                self.spawn_effect(model, x, y, z);
                self.ent[i].flags |= 0x400;
                false
            }
            _ => false,
        }
    }

    /// The spell a mine has SWALLOWED and the tier it was cast at —
    /// `word_0x36_54` / `word_0x34_52`, written together by
    /// `sub_68AC0` (EF:55438-40). The port's armed gate carries the
    /// retail value with a forced high bit (armed ⟺ `f36 == 0`), so
    /// that swallowing spell 0 still disarms; mask it back off here.
    fn mc2_mine_charge(&self, i: usize) -> (usize, usize) {
        let e = &self.ent[i];
        ((e.f36 & 0x7FFF) as usize, (e.f54 as usize).min(2))
    }

    /// `sub_3A8B0` case 4's proximity scan (EF:29942-56).
    fn mc2_mine_scan(&self, i: usize, ctx: &MobCtx) -> Option<u16> {
        let (mx, my, mz, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f52)
        };
        let d3 = |x: u16, y: u16, z: i16| -> i32 {
            let dz = (z as i32) - (mz as i32);
            Self::isqrt(
                (Self::dist2_sq(mx, my, x, y) as i64 + (dz as i64) * (dz as i64))
                    .min(u32::MAX as i64) as u32,
            ) as i32
        };
        let mut best: Option<(u16, i32)> = None;
        // The out-of-pool human is a class-3 model-0 record on retail's
        // `dword_38519` like any other; the roster's entry test is
        // `life_0x8 >= 0` (EF:39975), which he takes here as `pdead_top`.
        if own != crate::mc1::mobs::PLAYER_TARGET && !ctx.pdead_top {
            let d = d3(ctx.px, ctx.py, ctx.pz);
            if d < 3584 {
                best = Some((crate::mc1::mobs::PLAYER_TARGET, d));
            }
        }
        for c in 0..self.wiz_chain.visible_len() {
            let j = self.wiz_chain.list[c] as usize;
            let e = &self.ent[j];
            // `ix->model_0x40_64 <= 1u && ix != v33x` — the owner is
            // excluded by RECORD IDENTITY, not by owner tag.
            if e.model65 > 1 || j as u16 == own {
                continue;
            }
            let d = d3(e.x, e.y, e.z);
            if d < 3584 && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        best.map(|(s, _)| s)
    }

    /// ⭐⭐⭐ `sub_3A8B0` case 5 (EF:29960-30042) — THE DETONATION IS A
    /// RELAUNCH OF THE SWALLOWED SPELL, NOT A BLAST. Retail hands
    /// `sub_6DCA0` the mine's OWNER as the caster, the mine's own
    /// position as the muzzle, the swallowed spell index
    /// (`word_0x36_54`) and the swallowed tier's subspell row
    /// (`word_0x34_52`), with `a5 = 0` (no caster speed boost) and
    /// `a6 = 1` (the cast sound, positioned at the OWNER). Then:
    /// ```text
    ///     v19x->id_0x1A_26   = a1x->word_0x32_50;   // the mine's owner owns the bolt
    ///     v19x->word_0x96_150 = a1x->word_0x96_150; // aimed at the tripper
    ///     sub_655C0(v19x, Entities[v17]);           // desired roll/fov at the tripper
    ///     v20x->yaw_0x1C_28   = v20x->roll_0x20_32; // ...applied immediately
    ///     v20x->pitch_0x1E_30 = v20x->fov_0x22_34;
    ///     v20x->position.z   += a1x->array_0x52_82.yaw;   // the MINE's muzzle lift
    ///     a1x->yaw_0x1C_28    = bolt.yaw + 0x400;         // the mine kicks 180 off
    /// ```
    /// `word_0x36_54 == 7 && life_0x1A == 2` (Lightning III) fires
    /// **two** bolts, fanned ±113 exactly like the cast site's own
    /// twin. Each landed bolt spends one `fontTypeIndex_0x3D_61`; at
    /// zero the burst ends, `byte_0x43_67` drops one, and the mine
    /// either RE-ARMS (back to state 2, a fresh delay and a fresh
    /// burst) or enters the teardown. The XP award is retail's —
    /// `sub_6D8B0(owner->id, 0x17u, 1)` (EF:29979), routed through
    /// `mc2_cast_xp` so the world tick re-applies the human-only guard.
    ///
    /// ⚠ RETIRED INVENTIONS (docs/DEVIATIONS.md): the forced
    /// `f80/f82/f84 = 1024` blast box around one `area_write`, the
    /// `mc2_atk_bolt` `(9,0)` stand-in, the `mc2_spawn_big_explosion`
    /// and the `f71 = 6` hand-off to the lifespan teardown. Retail
    /// does none of them: no area write anywhere in case 5, and a
    /// spent mine reaches state 6 only through `byte_0x43_67`.
    fn mc2_mine_detonate(&mut self, i: usize, ctx: &MobCtx) {
        // EF:29962 — `dword &= 0xFF7FFFFE`: the draw bit and @0x17.
        self.ent[i].flags &= !0x0080_0001;
        let v17 = self.ent[i].f146;
        // EF:29965-70 — the tripper must still be there, alive and
        // un-reaped, or the burst is over.
        let tripper = if v17 == 0 {
            None
        } else if v17 == crate::mc1::mobs::PLAYER_TARGET {
            (!ctx.pdead).then_some((ctx.px, ctx.py, ctx.pz))
        } else {
            self.ent
                .get(v17 as usize)
                .filter(|t| t.act_life >= 0 && t.flags & 0x400 == 0)
                .map(|t| (t.x, t.y, t.aim_z()))
        };
        let mut done = tripper.is_none();
        if let Some((tx, ty, tz)) = tripper {
            let owner = self.ent[i].f52;
            // `sub_6D8B0(v33x->id_0x1A_26, 0x17u, 1)` (EF:29979) — a
            // wizard record's `@0x1A` is its own slot, so the owner tag
            // IS the award target. Rival awards are filtered by
            // `sub_6D8B0`'s own human-only guard in the mail drain.
            self.mc2_cast_xp.0.push((owner, 23, 1));
            let (spell, tier) = self.mc2_mine_charge(i);
            let row = self.assets.spells.get(spell).copied();
            let mut sub = row.map_or(crate::mc2::spells::Mc2SubSpell::default(), |r| {
                r.tiers[tier]
            });
            // The 0x15/0x19 arms divide the payload by the charge
            // (`subSpellIndex_2 / life_0x1A`, EF:44189-219).
            if matches!(spell, 21 | 25) && sub.life > 0 {
                sub.sub_spell /= sub.life as i32;
            }
            // `v34 = word_0x36_54 == 7 && life_0x1A == 2` ⇒ TWO bolts.
            let twin = spell == 7 && sub.life == 2;
            let shots = usize::from(twin) + 1;
            // The `sub_6DCA0` cast sound (EF:44232-33), keyed to the
            // OWNER's slot: fireball 9, lightning charged 9 /
            // uncharged 23, everything else 15.
            let v6 = match spell {
                0 => 9u8,
                7 if matches!(sub.life, 1 | 2) => 9,
                7 => 23,
                _ => 15,
            };
            for k in 0..shots {
                let Some(arm) = crate::engine::world::World::mc2_dispatch_arm(spell, sub.life)
                else {
                    break; // no band arm ⇒ `sub_6DCA0` returns null
                };
                let (mx, my, mz) = {
                    let e = &self.ent[i];
                    (e.x, e.y, e.z)
                };
                let Some(p) = self.mc2_spawn_cast_proj(arm.subtype, mx, my, mz) else {
                    break; // pool full: retail's `if (v19x)` skips it all
                };
                let yaw = Self::angle_between(mx, my, tx, ty);
                let dh = Self::isqrt(Self::dist2_sq(mx, my, tx, ty) as u32) as i32;
                let pitch = Self::pitch_toward(mz, tz, dh);
                {
                    let e = &mut self.ent[p];
                    e.f68 = arm.impact.0;
                    e.f69 = arm.impact.1;
                    // The mine's post-launch writes are `id_0x1A_26` +
                    // `word_0x96_150` only (EF:29999-30000) — the same
                    // `@0x2A` absence the castle turret has.
                    if crate::mc2::cast::mc2_band_arm_writes_2a(spell) {
                        e.f44 = sub.sub_spell.clamp(0, u16::MAX as i32) as u16;
                    }
                    if arm.charge {
                        e.f71 = sub.life.max(0) as u8;
                    }
                    // a5 = 0: no caster boost, the clamp only
                    // (EF:44226-31).
                    e.f126 = e.f126.clamp(384, 0x2000);
                    e.id24 = owner; // EF:29988
                    e.f146 = v17; // EF:29989
                    e.f34 = yaw; // `sub_655C0` — the DESIRED aim
                    e.f36 = pitch;
                    e.f30 = yaw; // EF:29991-92 — applied at once
                    e.f32 = pitch;
                }
                // EF:29994 — the muzzle lift is the MINE's own
                // `array_0x52_82.yaw`, and it lands AFTER the aim.
                let lift = self.ent[i].f78 as i16;
                self.ent[p].z = self.ent[p].z.wrapping_add(lift);
                // EF:29993/29996 — the mine kicks a half-turn off the
                // shot and steps its recoil counter (1..5).
                self.ent[i].f30 = yaw.wrapping_add(0x400) & 0x7FF;
                let b44 = self.ent[i].f69 as i8;
                self.ent[i].f69 = if b44 == 0 {
                    1
                } else {
                    b44.wrapping_add(1).min(5) as u8
                };
                // EF:30013-19 — Lightning III's pair fans ±113.
                if twin {
                    let e = &mut self.ent[p];
                    let y2 = if k != 0 {
                        e.f30.wrapping_sub(113)
                    } else {
                        e.f30.wrapping_add(113)
                    };
                    e.f30 = y2 & 0x7FF;
                }
                self.mc2_mine_snd(v6, owner);
                // EF:30021-24 — one shot off the burst budget.
                let c = (self.ent[i].f46 as u8).wrapping_sub(1);
                self.ent[i].f46 = c as i16;
                if c == 0 {
                    done = true;
                }
            }
        }
        if done {
            // EF:30028-40 — drop the lock, spend one `byte_0x43_67`,
            // and either RE-ARM (state 2 rolls a fresh delay and a
            // fresh burst) or enter the teardown.
            self.ent[i].f146 = 0;
            let b43 = self.ent[i].f68.wrapping_sub(1);
            self.ent[i].f68 = b43;
            self.ent[i].f71 = if b43 != 0 { 2 } else { 6 };
        }
    }

    /// `PrepareEventSound_6E450(a1x - Entities, -1, v6)` where `a1x` is
    /// the OWNER wizard `sub_6DCA0` was handed — the cast sound rides
    /// the owner's slot, not the mine's.
    fn mc2_mine_snd(&mut self, id: u8, owner: u16) {
        if owner == crate::mc1::mobs::PLAYER_TARGET {
            self.snd_player(id);
        } else if (owner as usize) < self.ent.len() {
            self.snd(id, owner as usize);
        }
    }

    /// `sub_4FE40` (EF:36506) — the (10,34) MC2 TELEPORTER pad
    /// (docs/traces/mc2-class10-m50-chains-and-tail.md §2): a
    /// self-contained player-only warp, NOT the MC1 paired-portal
    /// arm. Visible sprite 223, extents 256, hovers 640 above
    /// terrain, targets class 3 (players), persistent (maxLife 0).
    /// ONE entity-RNG draw whose fling of the launch axis is dead —
    /// the THING post-init overwrites the destination with the
    /// par-authored tile (par1 = dest Y / par2 = dest X, EF:33077 —
    /// the shared (10,34) post-init in the spawn seam); the draw
    /// stays for RNG-stream parity.
    pub(crate) fn mc2_spawn_portal(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 34;
            e.tick70 = 36; // actionIndex 0x24
            e.max_life = 0;
            e.f66 = 3; // xtype: class-3 players only
            e.f67 = 0xFF;
            e.flags &= !8;
        }
        self.mc2_set_sprite(i, 223);
        self.mc2_shift_rot(i, 256, 256);
        self.refill_life(i);
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = (self.ground_z(lx, ly) as i16).wrapping_add(640);
        let _ = self.mc2_rand(i); // the dead launch-axis fling draw
        Some(i)
    }

    /// `sub_4FD70` (EF:36468) — the (10,51) traveling RIDGE/DAMAGE
    /// BEAM, the (10,50) chain's per-segment child
    /// (docs/traces/mc2-class10-m50-chains-and-tail.md §1.4). Not
    /// map-linked, no sprite (invisible), extents 768, actSpeed
    /// 1024/tick, life = the chain stamper's dist/1024. The damage
    /// amount stays NewEvent's subSpell default 100 (neither the ctor
    /// nor sub_48880 overrides it), homed in f140 like the rest of the
    /// class-10 effect column.
    pub(crate) fn mc2_spawn_load_beam(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 51;
            e.tick70 = 0x37;
            e.max_life = 0;
            e.f26 = 256; // dword_0x10_16
            e.f126 = 1024; // actSpeed
            e.f140 = 100; // subSpellIndex (the NewEvent default)
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.mc2_shift_rot(i, 768, 768);
        self.refill_life(i);
        Some(i)
    }

    /// `sub_352C0` (EF:25739) — the (10,51) beam tick: post-decrement
    /// despawn OR a class-0 (water/void) cell under it (`sub_104A0 &
    /// 1`, the unrounded cell); otherwise ONE entity-RNG draw feeds
    /// the terrain RAISE — `sub_572C0(0, 1024, r%0xF+10, 0)` walks
    /// the disc of radius pitch/256 = 3 tiles applying the
    /// unprotected +delta cell write (`sub_56F10` ≡ the shared
    /// chassis `dig_cell` — same clamp 0..200, angle-nibble → 1,
    /// water-conversion, (0,0) latch); the walk always exhausts
    /// (nothing refuses in unprotected mode, sub_572C0 → 0) so the
    /// `sub_10C80` ch0 area damage + sound 10 fire EVERY tick; then
    /// advance 1024 along yaw. Returns terrain-dirty.
    pub(crate) fn mc2_load_beam_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let raw = crate::engine::features::tile((x >> 8) as u8, (y >> 8) as u8);
        if life < 0 || self.t.angle[raw] & 0xF == 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let d = self.mc2_rand(i);
        let delta = (d % 0xF + 10) as i16;
        let r = (self.ent[i].f80 as i32) >> 8;
        let (cx, cy) = (
            (x.wrapping_add(128) >> 8) as i16,
            (y.wrapping_add(128) >> 8) as i16,
        );
        for (dx, dy) in self.ring_cells(0, r) {
            self.dig_cell_pub(
                cx.wrapping_add((dx as i8) as i16),
                cy.wrapping_add((dy as i8) as i16),
                delta,
                false,
            );
        }
        let amt = self.ent[i].f140 as u32;
        self.area_write(i, 0, amt, ctx, false, false);
        self.snd(10, i);
        let (yaw, spd) = (self.ent[i].f30, self.ent[i].f126);
        let mut pos = (x, y, self.ent[i].z);
        Self::polar_step(&mut pos, yaw, 0, spd);
        {
            let e = &mut self.ent[i];
            e.x = pos.0;
            e.y = pos.1;
            e.z = pos.2;
        }
        true
    }

    /// `sub_4FA00` (EF:36274) — the (10,29) stage/quest marker:
    /// INVISIBLE (no sprite), life 0, lives exactly one tick
    /// (action 0x1F = DisableEntityDrawing). Its whole job is
    /// donating position/identity to the stage binder at spawn —
    /// our stage engine reads the authored checkpoint rows directly,
    /// so the entity is pure churn, exactly like retail.
    pub(crate) fn mc2_spawn_stage_marker(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.mc2_spawn_stage_marker_for(29, 0x1F, x, y, z)
    }

    /// The shared one-tick marker ctor shape ((10,29) `sub_4FA00`
    /// EF:36274, (10,50) `sub_4FDE0` EF:36488 — byte-identical
    /// bodies modulo model/action): invisible, life 0, untargetable,
    /// map-registered, gone on the first tick.
    pub(crate) fn mc2_spawn_stage_marker_for(
        &mut self,
        model: u8,
        state: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = state;
            e.max_life = 0;
            e.flags &= !0x8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        Some(i)
    }

    // ---- ticks ---------------------------------------------------------------

    /// `AddAsh0A_05_318B0` (EF:23169) — the splash tick: 8 ticks of
    /// frame animation at the water surface, sound 27 once (the
    /// flags-bit-2 latch), then despawn.
    pub(crate) fn mc2_splash_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        // sub_585A0 (EF:23173) — capped by the sprite's own count.
        self.mc2_anim_step(i);
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.snd(27, i);
        }
    }

    /// `AddParticleSmoke0A_3D_32420` (EF:23666) — the shared emitter
    /// tick: post-decrement despawn, THREE entity-RNG draws (x-jitter
    /// 0..159, z-jitter 0..159 — retail jitters x and z only, NEVER
    /// y — and the particle speed bonus 0..76), one particle per tick
    /// with life forced to 32.
    pub(crate) fn mc2_smoke_emitter_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (ex, ey, ez, model) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.model65)
        };
        let d = self.mc2_rand(i);
        let px = ex.wrapping_add((d % 0xA0) as u16);
        let d = self.mc2_rand(i);
        let pz = ez.wrapping_add((d % 0xA0) as i16);
        let pm = if model == 59 { 13 } else { 14 };
        if let Some(p) = self.mc2_spawn_smoke_particle_for(pm, px, ey, pz) {
            let d = self.mc2_rand(i);
            self.ent[p].act_life = 32;
            self.ent[p].max_life = 32;
            self.ent[p].f126 += self.ent[i].f126 + (d % 0x4D) as i16;
        }
    }

    /// `sub_32160` / `sub_322A0` (EF:23572/:23613) — the particle
    /// tick, identical except the sprite-row band (m13: grow to 74,
    /// end-of-life floor 67; m14: 16/9). Rise by actSpeed (−4/tick,
    /// clamped [64,128]) with the terrain floor, drift yaw-forward
    /// for the first 16 phase ticks (maxSpeed −52/tick clamped
    /// [30,1024]), grow the sprite row on even ticks, shrink it when
    /// life < 6. No RNG, no sound.
    pub(crate) fn mc2_smoke_particle_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        // m87 (the third puff, action 0x5E) shares `sub_32160` — the
        // m13 handler and its 67..74 band (docs/traces/
        // mc2-class10-m29-m5-m13.md §2.2; the model-13-only test
        // dropped it into m14's 9..16 band and every m87 row read one
        // low — the mc2l3 (10,87) f5a family, 4,123 rows).
        let (grow_cap, shrink_floor) = if matches!(self.ent[i].model65, 13 | 87) {
            (74, 67)
        } else {
            (16, 9)
        };
        {
            let e = &mut self.ent[i];
            e.f126 = (e.f126 - 4).clamp(64, 128);
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let mut pos = (x, y, self.ent[i].z.wrapping_add(self.ent[i].f126));
        let alt = self.ground_z(x, y) as i16;
        if pos.2 < alt {
            pos.2 = alt;
        }
        self.ent[i].f26 += 1;
        if self.ent[i].f26 < 16 {
            let (yaw, spd) = (self.ent[i].f30, self.ent[i].f130);
            Self::polar_step(&mut pos, yaw, 0, spd);
            let e = &mut self.ent[i];
            e.f130 = (e.f130 - 52).clamp(30, 1024);
            if e.f26 & 1 == 0 && e.type86 < grow_cap {
                e.type86 += 1;
            }
        }
        if self.ent[i].act_life < 6 && self.ent[i].type86 > shrink_floor {
            self.ent[i].type86 -= 1;
        }
        self.move_relink(i, pos.0, pos.1, pos.2);
    }

    /// `sub_31760` (EF:23099) — the (10,6) standing-fire tick.
    /// Post-decrement despawn WITH one last damage pulse; the
    /// grow/shrink sprite machine on `word_0x5A_90` (type86: 6-step
    /// ramp up while life >= 12, ramp down under 12 with a ~1/7
    /// (10,14) smoke puff per shrink tick — life forced 15, drift
    /// phase disabled, sprite row +2, id inherited); z = f44 lift +
    /// terrain each tick; extinguished by water; ch0 area heat of
    /// `subSpell` EVERY tick (`sub_11400` — the void mailbox writer;
    /// trees take a tenth, hence `building_tenth`), gated only on
    /// byte[2] bit 0 which nothing here sets.
    ///
    /// APPROX register: `sub_5C870` (EF:43602, the player
    /// nearest-hazard distance for HUD/audio proximity) has no
    /// ported consumer — skipped, no gameplay observable.
    pub(crate) fn mc2_fire6_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            if self.ent[i].flags & 0x1_0000 == 0 {
                let amt = self.ent[i].f140 as u32;
                self.area_write(i, 0, amt, ctx, true, false);
            }
            return;
        }
        if self.ent[i].act_life < 12 {
            if self.ent[i].f26 > 0 {
                self.ent[i].f26 -= 1;
                self.ent[i].type86 = self.ent[i].type86.wrapping_sub(1);
                if self.ent[i].flags & 0x80 == 0 {
                    let d = self.mc2_rand(i);
                    if d % 7 == 0 {
                        let (x, y, z, id) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z, e.id24)
                        };
                        if let Some(p) = self.mc2_spawn_smoke_particle_for(14, x, y, z) {
                            let e = &mut self.ent[p];
                            e.f26 = 100;
                            e.act_life = 15;
                            e.id24 = id;
                            e.type86 += 2;
                        }
                    }
                }
            }
        } else if self.ent[i].f26 <= 6 {
            self.ent[i].type86 += 1;
            self.ent[i].f26 += 1;
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        self.ent[i].z = (self.ent[i].f44 as i16).wrapping_add(ground);
        if self.cap_bit(x, y) == 1 {
            self.ent[i].flags |= 0x400;
        }
        if self.ent[i].flags & 0x1_0000 == 0 {
            let amt = self.ent[i].f140 as u32;
            self.area_write(i, 0, amt, ctx, true, false);
        }
    }
}
