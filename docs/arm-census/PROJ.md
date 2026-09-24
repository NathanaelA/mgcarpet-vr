# ARM CENSUS — PROJ (projectiles, aiming, hit/overlap resolution)

Paths are relative to `crates/mgc-sim/src/`. Line numbers are as of 2026-09-24 (branch `re_recording`).
"Human" means the port's out-of-pool carpet (`PLAYER_TARGET` / `ctx.p*` / `human_pose`). In retail the human is an
ordinary pool record, so most of the drift below comes from the port re-deriving that record's box, z raise,
roster membership or chain seat by hand at each site.

---

### PROJ-1: Human candidacy in the one-shot projectile acquire (MC1 `sub_54520` / MC2 `sub_67CB0`)
- **Retail routine(s):** MC1/HW `sub_54520` (:63943, HW `sub_54520_548B0`); MC2 `sub_67CB0` (EF:54710) + lock `sub_655C0`. These are the same routine in both games: on a projectile's first flight tick it picks the best target in a ±0x71 cone and bends the shot onto it. Both walk the TICK-TOP class-3 roster (MC1 `var_u32_36462[0]`, MC2 `dword_38519`). That roster only takes a record whose `life >= 0` at the top of the frame, and a mid-tick seizure blank hides it.
- **Port arms:**
  - `mc1/combat.rs:2131 aim_assist_mc1_cone2`: MC1/HW, all casters (human, rival, creature). The human candidate is tested at :2244-2258.
  - `mc2/proj.rs:4388 mc2_autoaim` + `mc2/proj.rs:4071 mc2_aim_scan`: MC2, all casters. The human is passed in as `human: Option<pos>` (:4402-4403) and scored after the wizard chain (:4135-4172).
- **Status:** DRIFTED
- **Differences:**
  - **Dead human.**
    - MC1 drops the human while he is dead at tick top (`!ctx.pdead_top`). Switch `MGC_NO_ACQUIRE_HUMAN_BUCKET0`, commit 1e0a191.
    - MC2 gates the human only on `own != PLAYER_TARGET && !player_invisible`, with no death gate. After the human dies, any rival or creature projectile acquiring in MC2 can still lock his corpse and bend onto it. Retail's `dword_38519` is built under `life >= 0` (the port's own comment at proj.rs:4107-4112 says so for pool wizards).
  - **Chain visibility.** MC1 also requires `mc1_human_on_wiz_chain()`: a same-tick seizure blank or sever below his seat hides him (mc1l19 t=14743). MC2 has no equivalent for the human, although the MC2 pool wizards are walked through `wiz_chain.visible_len()`, which does honour the blank.
  - **Scan order / tie-break.** MC1 scores the human FIRST (before the pooled class-3 chain, the "Scan-A tie-break ruling"). MC2 scores him AFTER the whole wizard chain. Both use strict `<`, so on an exact score tie MC1 picks the human and MC2 picks the pool wizard.
  - These are cross-game differences in shape only. The score math, the cone, the 5120 2-D range, the 3-D sig gate at the raw position, the human `+PLAYER_HH` raise and the danger arm are the same in both.
- **What reaches each arm in-game:**
  - MC1 arm: any projectile's first flight tick in MC1/HW while the human is a potential target (rival fireball, creature bolt, storm/meteor child), including the ticks right after the human dies.
  - MC2 arm: the same in MC2. The divergent case is **the human is dead (falling or on the ground) and a rival wizard or a shooting creature launches a fresh projectile toward where he is.** The port can lock the corpse; retail should not.
- **Already witnessed?:**
  - MC1 dead-human arm: yes (mc1l49 t=34531, "THE CLASS-3 ACQUIRE DROPS THE DEAD HUMAN AT TICK TOP").
  - MC2 dead-human arm: no evidence found.
  - MC2 live-human raise: yes (mc2l24 t=45473, mc2l6 t=309 per the code comment).
- **Recording ask:** MC2, any level with an aggressive rival (mc2l6 or mc2l22 are natural). Let the rival or a missile creature (archers are enough) kill you in the open, then stay dead through the respawn wait. Keep rivals or creatures in range and facing the corpse so they keep firing. About 2-3 minutes, 2-3 deaths. The divergence shows on each `(9,x)` born after the death tick: `target96` / heading bent onto the corpse vs straight.
- **Confidence / notes:** High that the gate is missing in the MC2 port (read both bodies). Medium that retail MC2 excludes the dead human: it follows from the roster-build predicate cited in the port's own comments, but I found no MC2 fixture for it. Whether rival AI even fires at a dead human is a brain question. Creature shots (archers mid-volley) are the likelier trigger.

---

### PROJ-2: The `sub_65580` / `sub_655C0` z-raise applied to the out-of-pool human (MC2 sites)
- **Retail routine(s):** MC2 `sub_65580` / `sub_655A0` (the in-place +`array_0x52_82.yaw` bracket, model 2 exempt) and `sub_655C0` (lock + desired aim toward the target, bracketed). MC1 twin: `sub_524C0` / `sub_524E0`. It is ONE bracket in retail, and retail's human is a boxed pool record, so it is raised like everyone else (+100).
- **Port arms** (pool targets use `Ent::aim_z`, `engine/features.rs:880`; each human site re-adds `PLAYER_HH` by hand):
  - `mc2/proj.rs:4388 mc2_autoaim` (lock, :4428-4431): HUMAN raised, no switch.
  - `mc2/proj.rs:4071 mc2_aim_scan` (scorer, :4161-4165): HUMAN raised, `MGC_NO_MC2_AIM_HUMAN_RAISE`.
  - `mc2/proj.rs:3178 mc2_flyer_tick` (homing servo, :3193-3197): HUMAN raised, no switch.
  - `mc2/castle.rs:3405 mc2_piece_fire` (the (10,79) turret, :3414-3424): HUMAN raised, `MGC_NO_MC2_PIECE_HUMAN_RAISE`.
  - `mc2/cast.rs` `mc2_fools_bolt` (:2104-2154, (10,57) retaliation): HUMAN raised, `MGC_NO_FOOLS_AIM_PHASE`.
  - `mc2/proj.rs:3543 mc2_proj_land` (strike landing, :3849-3862): HUMAN raised except under the beam (the beam exemption was added to both arms).
  - `mc2/mobs.rs:2765 mc2_arrow_tick` (arrow landing, :2847-2853): HUMAN raised.
  - **`mc2/effects.rs:997 mc2_mine_detonate`** (the Magic Mine relaunch, `sub_3A8B0` case 5 → `sub_655C0(bolt, tripper)`): tripper resolution at :1001-1011 gives POOL `t.aim_z()` but HUMAN **raw `ctx.pz`**. **The raise is missing on this arm.**
  - MC1 counterparts, all raised: `mc1/combat.rs:2131` acquire, `:2467 home`, `:6343 proj_move_and_hit` snap, `:4578 proj_m8_tick` snap, `:5030` bolt, `:5120` payload.
- **Status:** DRIFTED (one arm: `mc2_mine_detonate`). The switches are uneven too: four arms carry a kill switch, three carry none.
- **Differences:**
  - `mc2_mine_detonate` aims the relaunched spell at the human's raw z. Pool trippers are aimed at their box centre, which is the same code's other arm. This is the same shape as round 139's `castle.rs` miss (fixture mc2l16 t=17681, "castle.rs was the one arm that never did").
  - Kill-switch coverage is uneven, which matters for the future collapse: the lock and the servo cannot be A/B'd, the scorer and the turret can.
- **What reaches each arm in-game:**
  - `mc2_mine_detonate` human arm: a **Magic Mine owned by someone else (a rival)** that has swallowed a spell, and the **human flies into its trip radius**. The mine relaunches the swallowed spell at the human. Human trippers only qualify for non-human-owned mines (effects.rs:942 `own != PLAYER_TARGET`).
  - Other arms: any MC2 auto-aimed shot, castle turret fire, fool's-mana retaliation, arrows, or bolt strikes on the human.
- **Already witnessed?:**
  - scorer: yes (mc2l24 t=45473, "handed mc2_aim_scan's OUT-OF-POOL human arm his RAW pose").
  - turret: yes (mc2l16 t=17681, mc2l12 t=10905).
  - fools: yes (mc2l24 t=1024 per the code comment).
  - beam landing: yes (mc2l22 t=9993).
  - lock: yes (mc2l6 t=309 per the code comment).
  - arrow: yes (mc2l0 t=4104 per the code comment).
  - `mc2_mine_detonate` human tripper: **no evidence found** (the only mine takes are mc2l0-spells-galore, mc2l6-rsg and mc2l24, all with human-owned mines).
- **Recording ask:** MC2, a level where a rival learns Magic Mine (spell 0x17). The rival must arm a mine and feed it a spell by firing into it, and the human must then trip it. This is hard to force through AI play and possibly unreachable in practice. If the rival brain never feeds its own mine, say so and this arm stays ungraded. Otherwise record ~5 min near a rival's mine and fly through it after it glows charged.
- **Confidence / notes:** High on the code difference. Whether a rival ever charges a mine in retail is unknown (rival brain).

---

### PROJ-3: The per-tick homing servo — MC1 `home` vs the MC2 flyer servo (self-alias lift)
- **Retail routine(s):** MC1 `sub_52550` (:62534) and MC2 `sub_65610` (EF:62750-62803, reached from `sub_65820` / `sub_65C20` / …). Both re-bear a flying projectile on `+146` each tick through the in-place raise bracket.
- **Port arms:**
  - `mc1/combat.rs:2467 home`: MC1/HW, all shooters (also reached by MC2-fallback projectiles through `proj_tick`).
  - `mc2/proj.rs:3178 mc2_flyer_tick` (Some-target arm, :3180-3216): MC2 native flyers.
  - `mc2/proj.rs:2825 mc2_castle_ball_tick` (upgrade flight, :2851-2854): MC2 castle ball.
  - `mc1/combat.rs:4196 castle_ball_homing_tick`: MC1 castle ball.
- **Status:** DRIFTED
- **Differences:**
  - **Self-alias.** MC1 `home` lifts the SHOOTER when `+146 == its own slot` (`fz = aim_z()` when `tgt == i`, :2494). Because retail's bracket is in-place on the measured record, homing at yourself is a no-op. MC2 `mc2_flyer_tick` always measures from the raw `e.z` against the lifted target, so a self-aliased MC2 flyer would pitch straight up (1536). There is no switch. The MC1 law came from mc1l2 slot 317.
  - **Target resolution.** MC1 `home` returns without steering on `t == 0`. MC2 `mc2_flyer_target` returns None on 0, which routes to the one-shot acquire arm, a different branch. Neither re-validates liveness (the same law in both).
  - **Castle ball target z.** MC1 `castle_ball_homing_tick` reads the target's raw `c.z`; MC2 reads `aim_z()`. The target is always a (3,2) castle, where `aim_z == z`, so this is equivalent in practice.
- **What reaches each arm in-game:** a homing projectile (fireball, lob) whose `+146` target slot gets freed and recycled INTO THE PROJECTILE ITSELF (a lob born into a freed ball's slot that its own acquire then scored). MC1 has a corpus row; MC2 has none.
- **Already witnessed?:**
  - MC1 self-alias: yes (mc1l2 t=2301, "homing at yourself is a NO-OP").
  - MC2 self-alias: no evidence found.
- **Recording ask:** Cannot be forced deliberately; it needs a pool slot recycled onto the homer. The best odds are a long MC2 possession-heavy take (possess bolts at mana spheres being scattered by kills) with the pool near full, 10+ minutes. Treat it as opportunistic and effectively ungraded.
- **Confidence / notes:** High on the code difference. MC2 retail's `sub_65580` is in-place per the port's own comments (proj.rs:3182-3186), so the alias should behave as in MC1, but that is not verified.

---

### PROJ-4: Rebound deflection — human-victim vs pool-victim arms (MC1 and MC2)
- **Retail routine(s):**
  - MC1 deflect blocks inside `sub_52770` (generic, :62705-50) and `sub_52B30` (fireball, :62847-88), plus the dead `sub_530C0` / `sub_53DC0` copies.
  - MC2 `sub_68740` (EF:55221-55308), called from `sub_65820` (window 0x2D/22), `sub_65C20` (0x5B/45), `AddArcherArrow_672E0` (EF:58892) and `sub_66FD0`.
  - All are one routine per game. The victim can be ANY pool record, and the human is one.
- **Port arms:**
  - `mc1/combat.rs:6343 proj_move_and_hit`, Pool arm (:6397-6446): MC1 POOL victim (rival carpet).
  - `mc1/combat.rs:6343 proj_move_and_hit`, Player arm (:6447-6508): MC1 HUMAN victim.
  - `mc2/proj.rs:838 mc2_rebound_deflect`: one fn with inline `MailTarget::Player` / `MailTarget::Pool` splits (active test, precise, cost, XP, landing).
  - `mc2/proj.rs:3543 mc2_proj_land` (:3563-3634): the call site, adding the `sub_65C20` refused-leg swallow (actions 0/29 only).
  - `mc2/mobs.rs:2765 mc2_arrow_tick` (:2818-2823): the arrow's call site, with no swallow (`sub_65820`-like).
- **Status:** DRIFTED
- **Differences:**
  - **MC2 precise tier is human-only.** The Player arm reads `mc2_rebound_precise` (yaw = roll exactly, `+44 *= 2`, no LCG draw). The Pool arm hard-codes `precise = false` and tests `flags & 0x8000`, where every call site tests `word[0] & 0x8010`. The source itself flags this as OPEN (proj.rs:823-836): there is no pool writer of the `0x10` bit (`import_ent_mc2` skips it; `mc2_rival_publish_buff` tier 1 is an empty arm).
  - **MC2 shield-swallow call site** (`MGC_NO_MC2_SHIELD_SWALLOW`) tests `self.player_rebound` for the human and `flags & 0x8010` for pool. The pool side can never see 0x10 (same missing writer).
  - **MC1 sound position.** Pool arm `snd(28, j)` (at the deflector); Player arm `snd(28, i)` (at the bolt). MC2 plays at the bolt for both. Sound is ungraded.
  - **Pitch mirror.** MC1 is gated by `MGC_NO_MC1_DEFLECT_PITCH_MIRROR` (72be2ce, round 154) in both MC1 arms. MC2 writes `f36 = f32` unconditionally with no switch.
  - **XP.** MC2 mails spell-8 XP only when the deflector is the human. This is consistent with retail: `sub_6D8B0` is human-only, per proj.rs:2293-2296 and effects.rs. Not a drift.
  - **Purse.** MC1/MC2 human cost reads `ctx.pmana - player_deflect_debit` (tick-head purse proxy); pool reads the live `f140`. This is a documented approximation (combat.rs:6450-6462).
- **What reaches each arm in-game:**
  - Human arm: the human has Rebound up and a rival or creature bolt reaches him.
  - Pool arm: a rival wizard has Rebound up and the human (or anyone) fires a fireball or bolt into him.
  - MC2 precise sub-arm: the victim's Rebound is the precise tier (per mc2l6 t=5564, the tier whose `life_0x1A` is 1).
- **Already witnessed?:**
  - MC1 human: yes (mc1hwl0 t=38739, "THE REBOUND DEFLECTION QUARTER DEBITS THE DEFLECTOR"; mc1l14 t=20607).
  - MC1 pool: yes (mc1l49 t=19557, "the Rebound deflection quarter sub_52B30").
  - MC2 human incl. precise: yes (mc2l6 t=5564, "THE PRECISE REBOUND TIER DRAWS NO LCG").
  - MC2 pool non-precise: yes (mc2l22 t=3628, t=27566).
  - **MC2 pool precise: no evidence found.**
- **Recording ask:** MC2, a level where a rival casts Rebound at the precise tier (mc2l6 / mc2l22 rivals cast Rebound). While its Rebound is up, fire fireballs (tier 0 and charged) straight into it repeatedly, ~2-3 min. The difference shows on the returned bolt: exact reverse yaw, doubled `f2a`, no `rand` step vs scatter. Note: the port cannot even represent a rival precise window today (missing writer), so this recording is also the witness for the missing publish.
- **Confidence / notes:** High that the arms differ. The retail behaviour for a rival precise deflect is inferred from the caster-generic `sub_68740`.

---

### PROJ-5: MC2 debuff stamps (10,65) stagger / (10,66) paralyze — human arm vs rival arm
- **Retail routine(s):** MC2 `sub_38E70` (action 0x46, stagger) and `sub_38F70` (action 0x47, paralyze), EF:28404-43. They are caster-generic: they read the victim wizard's `dword_0xA4_164x` (moveSpeed / mobilizeCounter). Game terms: a creature's web or arc lob lands on a wizard and slows or stuns him, with a backward kick, a grunt, an LCG step and (paralyze) a point-damage mail, all only when he is not already debuffed.
- **Port arms:**
  - `mc2/proj.rs:1037 mc2_debuff_stamp_tick`, `victim == PLAYER_TARGET` branch (:1043-1196): HUMAN.
  - Same fn, `else` branch (:1197-1210): RIVAL (pool class-3 model-0).
- **Status:** DRIFTED
- **Differences:**
  - **Already-debuffed gate.** Human: the kick, the LCG draw, the grunt and (paralyze) the `mc2_melee_write` all sit inside `!already_stunned` (mobilize latch / `mc2_slow` level, switches `MGC_NO_MC2_STAGGER_LEVEL_GATE` and the paralyze law). Rival: ALWAYS draws the LCG + grunt, ALWAYS mails on paralyze, with no gate at all.
  - **Backward kick.** Human gets `player_knock.1 = -80` (`MGC_NO_STALE_DEBUFF_KNOCK`). Rival: no kick written anywhere.
  - **Latch / level.** Human updates `mc2_mobilize` / `mc2_slow` (saturating at 3) plus the flight-ext queue. Rival writes no stun or slow register at all. Whether rival flight reads a stun is a separate MOVE question.
  - The switches (5462778, the mc2l42 commit) landed on the human arm only.
- **What reaches each arm in-game:**
  - Human arm: a (5,17) creature's (9,20) lob or a (5,20)'s (9,21) arc (roster.rs:3492 / :4357) hits the human.
  - Rival arm: the same lobs hit a RIVAL wizard. The difference fires when a rival is hit a second time while already slowed or stunned (two lobs within the stun window).
- **Already witnessed?:**
  - human: yes (mc2l22 t=375, "A PARALYZE RE-STAMP ON AN ALREADY-STUNNED WIZARD DRAWS NOTHING"; mc2l24 t=10898, "THE (10,65) STAGGER STAMP ONLY BITES A WIZARD WHO IS NOT ALREADY SLOWED").
  - rival: no evidence found.
- **Recording ask:** MC2 level with the m17 / m20 lob creatures and an active rival (mc2l22 or mc2l24; mc2l3 has m20). Lead the creatures onto a rival wizard, or fight near a rival's castle where they roam, so the rival takes repeated webs. ~3-5 min. Look for the rival's carpet `rand` / life on the second hit in the window.
- **Confidence / notes:** High on the code. Whether retail rivals keep a `str_164` that these stamps write is inferred from "caster-generic"; not verified in bytes.

---

### PROJ-6: Hand-rolled `sub_118C0` / `sub_106C0` boxes against the out-of-pool carpet
- **Retail routine(s):** MC1 `sub_118C0` (:16963, via `sub_11950`) and MC2 `sub_106C0` / `sub_10630` (EF:3712); the XY-only `CompareAxisWithShift_10750` for the switch probes. All are one summed-extents AABB with `movswl` extents: an extent above 0x7FFF is negative. Game terms: "is this entity touching the wizard".
- **Port arms** (entity vs carpet):
  - `mc1/combat.rs:940 player_overlap`: signed extents (`aabb_ext`), per-game HW (119/121), ctx pose. Used by victim_scan, area_write, MC1 regen_boost, native try_pickup.
  - `mc1/combat.rs:885 ent_overlap`: pool-pool, signed.
  - `engine/world.rs:22101 World::overlap`: triggers/portals/pads. UNSIGNED `e.f80 as i32`; per-game HW via `MGC_NO_MC2_TRIGGER_HW` (5462778).
  - `engine/world.rs:5769 mc2_regen_boost` + `:5838 mc2_regen_boost_warp`: UNSIGNED, const 121/100/100, pre-move pose.
  - `mc2/rivals.rs:6232 mc2_owner_at_castle` (human leg): UNSIGNED, `human_pose`, alive-gated.
  - `mc1/rivals.rs:4343-4351` inline `parked_on`: UNSIGNED, MC1 HW, `human_pose`.
  - `engine/world.rs:16588-16598` strict MC1 jar poll: UNSIGNED, `human_pose`. Its native twin `try_pickup` path (world.rs:16777-16780) uses `player_overlap` (signed, ctx).
  - `engine/world.rs:20598-20611` MC2 jar pickup: UNSIGNED, `MGC_NO_MC2_JAR_PICKUP_BOX` (5462778).
  - `mc2/roster.rs:5248-5252` `m23_owner_scan` `parked`: XY-only, UNSIGNED.
  - `engine/world.rs:21853 mc2_switch_overlap`: XY-only, HW = `mc2_params_ext(44)/2`.
  - Others: `mc2/flood.rs:894 mc2_overlap_xy` (pool-pool XY, UNSIGNED); `engine/features.rs:9410-9420` MC1 upgrade-space check (signed, inline).
- **Status:** DRIFTED
- **Differences:**
  - **Signed extents.** `MGC_NO_MC1_AABB_SIGNED_EXTENTS` (6a71a1f) reached only `ent_overlap` / `player_overlap` and the one inline copy in features.rs. Every other hand-rolled copy widens `f80/f82/f84` unsigned, so an entity with an extent past 0x7FFF still "touches" the wizard there.
  - **Half-width constant.** It is picked per site:
    - `player_overlap` / `World::overlap`: per game.
    - MC2 regen / jar / owner-at-castle: the 121 constant.
    - switch probe: `params_ext(44)/2`.
    - MC1 inline copies: `PLAYER_HW`.
    
    These agree today, but they are four independent homes for one lane (the "fourth reader" comment at world.rs:22078 already caught one miss).
  - **Pose.** Current ctx vs `human_pose` vs pre-move pose. Each is documented per site (walk-slot laws), not a drift in itself, but it means these sites cannot share one call without a pose argument.
  - **Axes.** Switch and m23 are XY-only. Retail uses different primitives there, so this is a legitimate split.
- **What reaches each arm in-game:** the signed-extent difference needs an entity whose `+80/+82/+84` exceeds 0x7FFF, e.g. a (10,17) blast ring carrying a stale-alias `+26 = 250` (the mc1l26 witness), overlapping a trigger, a jar, the carpet at a castle or dolmen, or a switch.
- **Already witnessed?:**
  - signed-extent law on `ent_overlap`: yes (mc1l26-froze t=25647 per the switch doc).
  - MC2 HW=121 on `player_overlap`: mc2l0 t=4104 (code comment).
  - `World::overlap` MC2 HW: mc2l24 t=2457 (code comment).
  - MC2 jar box: mc2l24 t=5219 (code comment).
  - Signed extents on any hand-rolled copy: no evidence found.
- **Recording ask:** Not practically recordable: the >0x7FFF extent only arises from a stale-alias write on a full pool. Treat it as a structural hazard for the collapse (unify on `aabb_ext`) rather than a recording target. **Ungraded in practice.**
- **Confidence / notes:** High on the code. Low practical impact.

---

### PROJ-7: Castle / dolmen fast-regen probe — MC1 `regen_boost` vs MC2 `mc2_regen_boost`
- **Retail routine(s):** MC1 :55346-50 + dolmen leg :55407 (`sub_11950` against the bound castle). MC2 `AddPlayer03_00_5E010` `locIsOk` (EF:59957-64) + the dolmen latch in `AddDolmen02_02_65080` (EF:65086-92). Same shape in both games: the wizard gets castle or dolmen regen rates while overlapping his bound castle or a dolmen.
- **Port arms:**
  - `engine/world.rs:5677 regen_boost`: MC1 human.
  - `engine/world.rs:5769 mc2_regen_boost` and `:5838 mc2_regen_boost_warp`: MC2 human.
- **Status:** STRUCTURALLY-DIFFERENT
- **Differences:**
  - **Castle box.** MC1 uses `player_overlap` (signed, ctx = current pose). MC2 uses an inline unsigned box on the PRE-move record.
  - **Dolmen.** MC1 has a latch (`mc1_at_shrine`, pre-latch live scan under `MGC_NO_MC1_SHRINE_LATCH`). MC2 runs a live pool probe on the pre-move pose. The latch is modelled only on the warp tick, by slot order (`MGC_NO_MC2_WARP_DOLMEN_PHASE`). The MC2 comment admits "a dolmen chained ABOVE the wizard slot would read the fresh pose — unmodeled".
  - **Register.** Both read the bound register (`player_castle_bound`) under their own switches (`MGC_NO_CASTLE_BIND_REGISTER` / `_LEVEL` for MC1, `no_mc2_castle_register` for MC2).
- **What reaches each arm in-game:**
  - MC1: hovering at your castle or a dolmen in MC1/HW.
  - MC2: the same in MC2. The unmodelled MC2 case is a dolmen whose pool slot is ABOVE the carpet's (e.g. a dolmen created after the wizard: respawned or late-authored).
- **Already witnessed?:**
  - MC1 castle box: yes (mc1l0 t=1827, mc1l6 t=3907, per the code comments).
  - MC2 dolmen below carpet: yes (mc2l0 t=342, mc2l24 t=2457, per the code comments).
  - MC2 castle register: mc2l12 t=18304 (code comment).
  - MC2 dolmen above carpet: no evidence found.
- **Recording ask:** MC2, a level where you die and respawn (so your carpet slot changes) and then sit on a dolmen. Park on and off the dolmen edge a few times, ~2 min. It only matters if the respawn puts the carpet BELOW the dolmen's slot. Possibly not controllable. Also belongs to the MANA subsystem.
- **Confidence / notes:** Medium. Pose-timing semantics are per-game by design; the only open arm is the MC2 above-carpet dolmen.

---

### PROJ-8: MC1 class-9 emission — human `cast_projectile` vs rival `rival_emit`
- **Retail routine(s):** The MC1 manifestation token machines (`sub_56090` … `sub_58240`, :65029-66420). These are ONE routine per spell for the human and the AI: the token mints the bolt and stamps owner, aim, launch boost, `+44`, `+140`, detonation pair `+68/+69`, dest triple `+150/+152/+154` and the charge move `+26`.
- **Port arms:**
  - `engine/world.rs:14105 cast_projectile`: HUMAN/MC1 (spells 3,6,7,8,9,11,13,15,17,19; fireball and firewall elsewhere).
  - `mc1/rivals.rs:5820 rival_emit`: RIVAL/MC1 (spells 0,23,3,7,8,11,13,15,17,20).
  - `mc1/combat.rs:1702 mc1_stamp_bolt_dest`: the shared dest helper.
- **Status:** DRIFTED (kill-switch coverage). At default flags the values agree for every shared spell.
- **Differences:**
  - **Dest triple, meteor (7).** Human is gated by `MGC_NO_MC1_METEOR_DEST_STAMP` (inline, 6a71a1f). Rival is gated by `MGC_NO_MC1_BOLT_DEST_STAMP` (via the helper, 72be2ce).
  - **Dest triple, possess (3) and lightning (15).** Human is inline and UNGATED. Rival goes through the helper and is gated by `BOLT_DEST_STAMP`.
  - **Dest triple, duel (11).** Both inline under `MGC_NO_MC1_DUEL_DART_EXACT`.
  - **Dest source.** Human reads `PlayerPose (p.x,p.y,p.z)`; rival reads the caster pool record. Both are "caster raw".
  - **Detonation pair (`MGC_NO_MC1_EMIT_DETONATION_PAIR`).** Human stamps {6,8,9,17}; rival stamps {8,17}. Rivals never cast 6/9 in `rival_emit`.
  - **Charge bank.** Human is fixed {6,7,8,9,13,15,17,19} (3 zeroes). Rival is under `rival_charge_family()` {0,6,7,8,9,13,15,17,18,19,20,22,23}, and the switch-off shape is {0,7,8,20}.
  - **`f36` pitch mirror.** Rival only (`no_mc1_rival_emit_pitch_mirror`); the human never writes `f36`. These agree at default.
  - **Launch speed.** Human adds `p.speed`; rival adds the carpet `f126`. Same retail read (`caster.+126`).
- **What reaches each arm in-game:** the human casting any bolt spell in MC1/HW, or a rival wizard casting the same spells.
- **Already witnessed?:**
  - human meteor dest: yes (mc1l15 t=44456).
  - rival fireball/possess dest: yes (mc1l2 t=2 / t=2380 per the helper doc).
  - duel: mc1l48 t=59114 (code comment).
  - lightning charge: mc1hwl0 t=22112 (code comment).
- **Recording ask:** None needed. The arms produce identical values today; the drift is only in which switch reverts which arm. Flag for the collapse: merge the switches.
- **Confidence / notes:** High. Overlaps the CAST subsystem (the token machine is the caster's); kept here because every field is a projectile record lane.

---

### PROJ-9: MC1 `sub_54520` case split — creature/wizard cone vs possess/magnet vs crosshair preview
- **Retail routine(s):** MC1/HW `sub_54520`: cases 0/3/4/9/0x10 (creatures + class-3), 7/8/B/C (class-3 only), 1 (possess), 0x11 (magnet, HW listing).
- **Port arms:**
  - `mc1/combat.rs:2131 aim_assist_mc1_cone2` (+ wrappers `aim_assist_mc1` :2105, `_cone` :2121, `aim_assist_wizards_mc1` :2433).
  - `mc1/combat.rs:3425 aim_assist_possess_mc1`: cases 1/0x11.
  - `mc1/combat.rs:2328 aim_preview_scan`: the crosshair INSTRUMENT (read-only).
- **Status:** STRUCTURALLY-DIFFERENT (cone vs possess are distinct retail cases). The preview is DRIFTED from the live scans.
- **Differences:**
  - **Snap.** Possess snaps `f30/f32` AND writes `f34/f36`; the cone writes only `f146/f34/f36`. Retail's case bodies differ, so this is expected.
  - **Self-alias lift.** Possess lifts the scorer's own z when the candidate is itself (`fz = aim_z()` if `j == i`, :3477-3479); the cone scan has no such guard (it skips `id24 == own` instead).
  - **Dwelling list.** Possess walks a LIVE pool with `0x400` / class gates ("chain semantics unmodeled", :3525-3530). The ball list is the tick-top chain.
  - **Preview drift.** The preview walks the LIVE pool with `act_life` / `0x420` / `tick70 != 120` gates, while the live scans walk tick-top chains. It has no dead-human gate (it is the human's own preview). It is display-only, not graded.
  - **Danger music.** Only the cone path arms it (cases 0/3/4, 7/8/B/C; not 9).
- **What reaches each arm in-game:** casting fireball, meteor, lightning, duel, steal, undead or volcano (cone) vs possess or magnet (possess) in MC1/HW. The preview is the crosshair option.
- **Already witnessed?:**
  - cone: yes (mc1l4 t=6864, mc1l1 t=4129, mc1l49 t=34531).
  - possess: yes (mc1l0 t=76 / pair 604, mc1l2 t=2301 alias).
  - magnet: yes (mc1l27 t=37366, mc1l0 t=2723).
  - possess dwelling-list chain semantics: no evidence found.
- **Recording ask:** Low value. To exercise the possess dwelling list: an MC1 level with villages, possess houses (m45) while houses are being destroyed and rebuilt nearby (record reuse mid-tick), ~3 min. The preview is ungraded.
- **Confidence / notes:** Medium on the dwelling-list relevance (the port notes that m45 records are never reused mid-tick in the corpus).

---

### PROJ-10: Out-of-pool human's seat in the victim probe's tile-chain walk (`sub_11980` / `sub_10780`)
- **Retail routine(s):** MC1 `sub_11980` (:16999-17027), MC2 `sub_10780` (EF:3739-71). The shot's collision probe walks tile chains in SEARCH.DAT ring order and takes the FIRST admissible overlapping record. The human is a linked record at his chain position.
- **Port arms** (all inside `mc1/combat.rs:2582 victim_scan`):
  - SEAT arm: MC1/HW (`player_chain` seat, `MGC_NO_PLAYER_CHAIN_SEAT`).
  - TAIL arm: MC2 strict (`MGC_NO_PLAYER_CELL_TAIL`, defined in `mc2/mobs.rs:380`).
  - POST-PASS arm: MC2 native play and the `MGC_NO_PROBE_WINDOW_PLAYER` instrument.
- **Status:** STRUCTURALLY-DIFFERENT (documented). The MC2 import keeps the carpet in-pool, so a seat would double-count. MC2 native uses the inflated window + chord march deviation.
- **Differences:** where the human sits relative to pool records in the same cell.
  - MC1: at his remembered gap.
  - MC2 strict: behind everything.
  - MC2 native: after all cells.
- **What reaches each arm in-game:** a projectile whose probe window holds the human AND another admissible target in the same tile (or an earlier ring cell). For example, a rival fireball reaching you while you hover over your own castle or balloon.
- **Already witnessed?:**
  - MC1 seat: yes (mc1l48 t=6593 per the code; mc1hwl0-pd t=451).
  - MC2 tail: witness in the `no_player_cell_tail` doc (MC2 strict).
  - MC2 native post-pass: ungraded (native play).
- **Recording ask:** None beyond what exists. The MC2 native arm is by definition not replay-graded.
- **Confidence / notes:** The arms are deliberate. Listed so the collapse knows three orderings exist.

---

### PROJ-11: Move-before-probe commit across the flight movers
- **Retail routine(s):**
  - MC1 flight handlers `sub_52770` / `sub_52B30` / `sub_530C0` (move with `sub_41C70`, then probe `sub_11980`).
  - MC2 `sub_65C20` / `sub_65820` (EF:63121-29), `CastPosses_65F60` / `sub_674C0`, `sub_66D00` castle ball (EF:59018-63).
  - Exceptions: `AddArcherArrow_672E0` probes PRE-move; `sub_66610` beam uses a raw write.
- **Port arms:**
  - `mc1/combat.rs:6343 proj_move_and_hit`: MC1 generic/fireball, relink first, no switch.
  - `mc1/combat.rs:4578 proj_m8_tick`: relink first, no switch.
  - `mc2/proj.rs:3178 mc2_flyer_tick`: `MGC_NO_MC2_PROJ_STEP_COMMIT` (6f55e65).
  - `mc2/proj.rs:2825 mc2_castle_ball_tick`: `MGC_NO_MC2_CASTLE_BALL_STEP_COMMIT` (6f55e65, double commit on refusal).
  - `mc2/mobs.rs:2765 mc2_arrow_tick`: probes the CURRENT position, commits after (documented "do not unify").
  - `mc1/combat.rs:4701 proj_m9_tick`, `mc2/proj.rs:2451 mc2_lightning_beam_tick`: raw writes.
  - `mc1/combat.rs:5120 proj_payload_tick`: probes via `victim_scan_at` (endpoint, NO pre-commit). The mc1l42 t=525 fixture names exactly one MC1 handler that does not relink before it probes; I believe this is the payload lob.
- **Status:** IDENTICAL law, applied per retail routine. Only the switch coverage differs (MC1 arms have none).
- **Differences:** no behavioural drift found; each arm cites its own retail commit order.
- **What reaches each arm in-game:** any projectile crossing a tile edge and then colliding or terrain-contacting in the same tick.
- **Already witnessed?:**
  - MC2 flyer: yes (mc2l9 pair 2073 per the switch doc).
  - MC2 castle ball: mc2l24-crazy t=15604 (per the doc).
  - MC1: mc1l32 t=7785 lineage (code comment), mc1l42 t=525.
- **Recording ask:** None.
- **Confidence / notes:** I did not verify which MC1 handler the mc1l42 t=525 fixture names.

---

### PROJ-12: Class-9 speed servo (`2·sign(min − act)`)
- **Retail routine(s):** MC1/HW: five sites (0x52802 / 0x53152 / 0x539DA / 0x53C2A / 0x53E52 and HW twins). MC2: the `sub_65820` ramp, `CastCastleProjectile_66B30` and `sub_66D00`.
- **Port arms:**
  - `mc1/combat.rs:693 flight_speed_step`: the helper, `MGC_NO_MC1_FLIGHT_SPEED_STEP`, used by 5 MC1 sites.
  - `mc1/combat.rs:4621-4626 proj_m8_tick`: double-gated by `MGC_NO_M8_ACQUIRE`.
  - `mc2/proj.rs:3464-3471 mc2_flyer_tick`: inline ±2, no switch, excluded for actions 0/1/18/29.
  - `mc2/proj.rs:2888-2894 mc2_castle_ball_tick`: `MGC_NO_MC2_CASTLE_BALL_SPEED_STEP`.
- **Status:** IDENTICAL at default (switch coverage differs).
- **Differences:** Three independent copies: the MC1 helper, the MC2 flyer inline and the MC2 castle-ball inline. Only the per-action exclusion list is MC2-specific (retail-cited).
- **What reaches each arm in-game:** any launched bolt with a caster-speed boost, or a castle ball.
- **Already witnessed?:** yes for all (mc2l4 t=7249 castle ball; mc2l6-rsg t=3543 action 18; MC1 per the switch doc).
- **Recording ask:** None.
- **Confidence / notes:** High.

---

### PROJ-13: Area writer's out-of-pool human arm vs its pool arm (`sub_120B0` family / `sub_10C80` / `sub_116A0` / `sub_11400`)
- **Retail routine(s):** MC1 `sub_120B0` / `sub_124F0` / `sub_127E0`; MC2 `sub_10C80` (+ ch3/ch4 arm EF:4034-60), `sub_11400`, `sub_116A0`. Area damage and claim mail to every overlapping record in the scan window. The human is just a linked record in retail.
- **Port arms** (inside `mc1/combat.rs:1012 area_write_opt`):
  - POOL tile-ring arm: :1400-1470.
  - HUMAN post-arm: :1500-1560.
  - MC2 ch3/ch4 POOL arm: :1254-1275.
  - MC2 ch3/ch4 HUMAN arm: :1307-1320.
- **Status:** DRIFTED (minor)
- **Differences:**
  - **Pool gates.** The pool arm tests the unfused id (`probe_self_id`, `MGC_NO_MC2_AREA_ID_UNFUSE`), damageable `flags & 8`, the vulnerability mask `f28 & (1<<ch)`, the filter, the castle exclusion and the building-footprint exclusion. The human arm tests only `id != PLAYER_TARGET`, the window (`MGC_NO_MC2_AREA_WINDOW` / `_CH34`), `filter_admits(3,0)` and `player_overlap`. There is **no `f28` channel-mask test and no damageable-bit test for the human.** If retail's carpet record lacks a channel bit, retail would skip him and the port bills him.
  - **ch3/ch4 pool arm id.** It compares the FUSED `id24` (`self.ent[j].id24 != id`). The ch0/ch1+ arm uses `probe_self_id`. It is inert today because ch3/ch4 victims are class 3 and the unfuse families are class 15/10.
- **What reaches each arm in-game:** any area effect (fire, blast, claim flash, steal burst, duel grip) around the human vs around a pool record.
- **Already witnessed?:**
  - human window gate: yes (mc1l0 t=565-570; mc2l24 t=134 and t=7913 per the code).
  - pool unfuse: mc2l6-rsg t=11664.
  - Human `f28` mask case: no evidence found.
- **Recording ask:** Only if the human carpet's `+28` mask is not all-ones in retail. First check with `dump-state` on any MC2/MC1 take (read the carpet record's `+28` / `@0x38`). If some channel bit is 0, record that channel's area effect bursting on the human (e.g. a possession claim flash (ch1) on your own carpet). No recording is needed if the mask is full.
- **Confidence / notes:** Low-medium. The human carpet's mask value was not checked.

---

## Summary table

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| PROJ-1 | Human candidacy in one-shot acquire (MC1 vs MC2) | DRIFTED | 2 | y (MC2 dead human) |
| PROJ-2 | `sub_655C0` human z-raise sites (MC2) | DRIFTED (`mc2_mine_detonate`) | 8 + MC1 | y (rival mine tripped by human; possibly unreachable) |
| PROJ-3 | Homing servo self-alias (MC1 `home` vs MC2 flyer) | DRIFTED | 4 | y (opportunistic; practically ungraded) |
| PROJ-4 | Rebound deflection human vs pool (MC1+MC2) | DRIFTED | 5 | y (MC2 rival precise Rebound) |
| PROJ-5 | MC2 debuff stamps human vs rival | DRIFTED | 2 | y (rival double-webbed) |
| PROJ-6 | Hand-rolled carpet AABBs (signed extents) | DRIFTED | ~11 | n (ungraded in practice) |
| PROJ-7 | Castle/dolmen regen probe MC1 vs MC2 | STRUCTURALLY-DIFFERENT | 3 | y (MC2 dolmen above carpet slot; maybe uncontrollable) |
| PROJ-8 | MC1 emit human vs rival | DRIFTED (switches only) | 2 + helper | n |
| PROJ-9 | MC1 `sub_54520` case split + preview | STRUCTURALLY-DIFFERENT | 3 | n (low value) |
| PROJ-10 | Human seat in victim probe | STRUCTURALLY-DIFFERENT | 3 | n |
| PROJ-11 | Move-before-probe commit | IDENTICAL | 7 | n |
| PROJ-12 | Speed servo copies | IDENTICAL | 4 | n |
| PROJ-13 | Area writer human arm vs pool arm | DRIFTED (minor) | 4 | check `+28` first |

## Clusters seen that belong to OTHER subsystems
- **CAST:**
  - MC2 human `mc2_spell_fire` (`mc2/cast.rs:4706`) vs rival `mc2_rival_emit` (`mc2/rivals.rs:9132`) vs `mc2_mine_detonate` vs `mc2_piece_fire`: four callers of the `sub_6DCA0` dispatch, each with its own stamp tail (possession tier table via `no_rival_possess_tier`, fool's-mana six-sphere throw, metamorph puppet, `f126` boost/clamp).
  - Rebound window lifecycle: `MGC_NO_MC1_REBOUND_BIT_LAG` (human, `manifestation_tick`) vs `rival_rebound_token_tick`, and `MGC_NO_RIVAL_REBOUND_CLEAR_DEFER` (rival only, mc2).
- **TRIGGER/SWITCH:** MC1 `balloon_probe` / `one_shot` / `rearm_probe` (`World::overlap`, 3-axis) vs MC2 `mc2_switch_probe` / `mc2_switch_rearm_probe` (`mc2_switch_overlap`, XY-only). The polarity and player-table laws are split across `MGC_NO_MC2_SWITCH_REARM_POLARITY` / `MGC_NO_MC1_SWITCH_REARM_PLAYER_TABLE`.
- **SPELL PICKUP / JARS:** MC1 strict jar poll (world.rs:16588, `human_pose`, unsigned) vs native `try_pickup` (`player_overlap`, ctx, signed). Same `sub_55A40`; the native arm is not replay-graded.
- **STATS:** MC1 has shot/hit counters (`mc1_shot_stats` + two inline copies in `proj_m9_tick` :4925/:4974, with `MGC_NO_MC1_HIT_STAT_AIM_LATCH` / `_MODEL_GATE` / `_ALLOC_GUARD`). I found no MC2 shots/hits writer at all (`grep self.shots` hits `mc1/combat.rs` only) — for the stats-screen session.
- **MOBS:** MC2 non-native class-9 records fall back into the MC1 `proj_tick` (`mc2/mobs.rs:6405-6410`, `note_verb_fallback`), so MC1 flight laws run on MC2 records there. There are also creature-thunk aim-at-raw-z sites (`mc2_atk_*` via `mc2_target`, MC1 `attack_thunk`).
- **MANA/REGEN:** PROJ-7 overlaps it.
