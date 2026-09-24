# MOB census — creatures and non-wizard actors (MC1/HW + MC2)

Read-only arm census per `BRIEF.md`. Three sections: **MC2 creature core** (MOB-1..12: `mc2/mobs.rs`, `mc2/roster.rs`), **MC1/HW creatures** (MOB-13..24: `mc1/mobs.rs`, creature intake in `mc1/combat.rs`, buffet in `mc1/rivals.rs`), **MC2 multipart + held/controlled** (MOB-25..30: `mc2/stagevars.rs`, `mc2/multipart.rs`, creature side of `mc2/tail.rs`). Paths are under `crates/mgc-sim/src/` unless absolute; line numbers are HEAD `52e5b11` + working tree. Take names: mc2lN / mc1lN = map N (0-based).

Cross-section overlaps (kept separately, different angles): MOB-9 ⊂ MOB-27 (`sub_12500` homes); MOB-3 ↔ MOB-28 (class-3 roster; MOB-28 = the human's entry test); MOB-11 ≡ MOB-24 (awake pass MC1 vs MC2); MOB-16 (MC1 roster-vs-pool) is the MC1 twin of MOB-3/4/5.

⭐ Headline for the player: **MC1 and HW share one creature arm in the port** (no HW branch in `mc1/mobs.rs`; spot-checked retail HW creature routines are line-identical), so MC1-vs-HW creature recordings are not needed. The big MC2 drift families are the **charm resolver `sub_1ED30`** (MOB-2), the **tick-top roster walks** (MOB-3/4/16/28), the **summon handler 13 vs 16** (MOB-1), and the **phase-7 wrapper tails across held/controlled/free seams** (MOB-25).

---

## Part A — MC2 creature core (mobs.rs / roster.rs)

### MOB-1: Controlled-summon handler `sub_1E580` + core `sub_1E700` (Summon Army SV2 13 vs pyramid summon SV2 16; Alliance SV2 14 reaches the same core)
- **Retail routine(s):** `sub_1E580` (NETHERW.EXE, EF:10689) — ONE handler for StageVar2 13 (Summon Army creature) and 16 (doomsday-pyramid summon parked in its home slot); both call the core `sub_1E700` (EF:10755). `sub_1E9C0` (SV2 14, Alliance) also calls `sub_1E700`. In game terms: a summoned/controlled creature follows its caster, locks the nearest enemy wizard every 8 ticks, and hands off to its attack state inside the row reach.
- **Port arms:**
  - `crates/mgc-sim/src/mc2/mobs.rs:5306 mc2_summon_creature_tick` + `:5221 mc2_summon_core` — SUMMON-ARMY/SV2 13
  - `crates/mgc-sim/src/mc2/mobs.rs:5680 mc2_doom_summon_home_tick` (inline copy of both handler and core; helpers `:5511 mc2_doom_hit_retarget`, `:5576 mc2_doom_target_probe`) — PYRAMID/SV2 16
  - `crates/mgc-sim/src/mc2/mobs.rs:6217 mc2_alliance_creature_tick` → calls `mc2_summon_core` — ALLIANCE/SV2 14 (shares the 13 core, so it is unified with 13, not with 16)
  - (not a twin: `:5388 mc2_summon_creature_tick_legacy`, `:6307 mc2_alliance_creature_tick_legacy` are kill-switch pre-fix bodies behind `MGC_NO_SUMMON_CORE` / `MGC_NO_MC2_ALLIANCE_CORE`)
- **Status:** DRIFTED
- **Differences:**
  - Lock-staleness test: 13 uses `mc2_summon_lock_ok` (`act_life > 0 && !reap`; human = live `!ctx.pdead`, no switch). 16 uses `mc2_doom_target_probe(home=true)` which has the same `<= 0` test only while `MGC_NO_DOOM_DEAD_TARGET` is unset (dig 125-B, commit 5462778); with the switch set it falls back to `< 0` and treats the human as never dead. The switch is consulted only on the 16 arm.
  - Re-acquire (`sub_16FC0(parent,parent)` in retail for both): 13 = `mc2_nearest_rival_wizard` (`:5162`), a LIVE-POOL sweep over class 3 model<=1 with `life>=0 && !reap`, human via LIVE `ctx.pdead`. 16 = hardcoded `PLAYER_TARGET` unless `ctx.pdead_top` (the TICK-TOP sample, gated by `MGC_NO_DOOM_DEAD_TARGET`) — 16 can NEVER lock a rival wizard, 13 can. 13 reads the live human death, 16 reads the tick-top one.
  - Quiet-arm aim and engage lookup: 13 (and 14) aim and measure reach through `mc2_summon_lock_pos` (RAW, no life test — doc: "`sub_1E580`/`sub_1E700` test the LOCK POINTER only"); 16 aims and measures through `mc2_doom_target_pos_home` (life `<= 0` / reap → no aim, no engage).
  - Crowd steer-away: 13/14 always call `mc2_avoid_packmate` (tick-top per-model roster). 16 calls it only under `MGC_NO_MC2_DOOM_CROWD_ROSTER` unset; with it set, 16 runs a hand-rolled live-pool sweep (commit 5462778).
  - No-lock (parentward) arm: 13 runs the FULL core with `f146 = parent` (move, 8-tick aim + 64-tick jink + crowd steer, hit retarget), then clobbers the lock and spends 4 lease. 16 runs a reduced copy: no crowd steer (explicit "APPROX" comment), the hit-retarget only when `MGC_NO_MC2_DOOM_DRIFT_HIT_ARM` unset, aim order only when `MGC_NO_DOOM_PARENT_AIM_ORDER` unset. Those two switches exist only on 16.
  - Hit-arm retarget: 16 uses helper `mc2_doom_hit_retarget` (parent test = parent SLOT equality); 13/14 inline the same logic in `mc2_summon_core` with parent test `atk != e.id24` (the parent's id) — same `MGC_NO_MC2_SUMMON_NULL_ATTACKER` switch on both copies.
  - Parent-alive probe: 13 = `mc2_target(parent)` (pool record by slot/`PLAYER_TARGET`, `life>=0`); 16 = scan for ANY live `(5,10)` (spawn-block APPROX, no parent pointer).
  - Enumerated retail differences, NOT drift: 13 spends 1 lease/tick and drops the `(10,73)` puff on expiry; 16 does neither (EF:10703-06, EF:10745-46).
- **What reaches each arm in-game:** 13 — MC2, the human (or a rival) casts Summon Army; the summoned creatures (fireflies/bees/wyverns/(5,25)) roam with the caster. 16 — MC2, a doomsday pyramid `(5,10)` hurls creatures that land (after the SV2-17 spin-up) in the home slot. 14 — MC2, Alliance charm on a creature group. The arms differ when (a) a RIVAL WIZARD is the nearest enemy of a pyramid summon (16 cannot lock it), (b) the locked target dies or sits at exactly 0 life within the tick, (c) two summons of the same model crowd while the summon has no lock, (d) the human dies while summons are out.
- **Already witnessed?:** 13 — yes (mc2l6 fixtures t=15660 "THE SUMMON-ARMY AND PYRAMID-SUMMON SHARE ONE HANDLER", t=20748 controlled-slot snap, t=20587 lease split). 16 — yes (mc2l24 t=45558 "THE DOOM SUMMON DROPS A DEAD TARGET AND MOVES BEFORE IT AIMS", t=45560 drift hit arm, t=49248 null attacker). 14 via core — yes (mc2l0 t=24000 "THE ALLIANCE SLOT RUNS THE SHARED `sub_1E700` CORE"). A pyramid summon locking a RIVAL wizard: no evidence found. A 16 summon crowding while unlocked: no evidence found.
- **Recording ask:** MC2 doomsday-pyramid level (the one mc2l24 is from) WITH A LIVE RIVAL WIZARD near the pyramid: keep the human far away (or invisible) so the pyramid summons' nearest non-pyramid wizard is the rival; let summons hunt ~2-3 min. Separately: cast Summon Army and then die (or let the human sit at low life and die) while the creatures are out and several of them cluster with no target, ~1-2 min.
- **Confidence / notes:** High on the structural diff (read both bodies). The "16 only ever locks the human" is an explicit port shortcut ("the out-of-pool player") — retail would run the same wizard walk as 13.

### MOB-2: Charm lock resolver `sub_1ED30` — 10 retail callers, 2 port call sites; the head charm clock covers the rest
- **Retail routine(s):** `sub_1ED30` (NETHERW.EXE file 0x43530, EF:11060) — every class-5 attack/flee handler asks "may I keep pursuing my lock?"; for a CHARMED creature (SV2 14) it decrements the charm clock `word_0x2E_46` and answers null when the clock is out, when the lock is the parent, or when the parent is fighting someone else. Retail callers (EventsFunctions.cpp): `sub_1C310` (shared chase-attack, m0/m2/m3/m4/m20/m24/m25), `sub_1C980` (shared FLEE, used by goat/townies/m12/m14/m24), `sub_20C50` (m9 hive imp chase+volley), `sub_24510` (m16 wyvern burst), `sub_24930` (m17 dive), `sub_250B0` (m18 tank barrage case 1), `HitFirebug_25610` (m19 attack run), `sub_26220` (m21 devil attack), `sub_27E00` (m23 ranged retaliation), `sub_28690` (m24 acquire).
- **Port arms:**
  - `crates/mgc-sim/src/mc2/mobs.rs:2008 mc2_chase_attack` — calls `mc2_ally_resolve` (sub_1C310 users)
  - `crates/mgc-sim/src/mc2/roster.rs:3248` inside `m16_tick` state 2 — calls `mc2_ally_resolve` (sub_24510)
  - `crates/mgc-sim/src/mc2/mobs.rs:1973 mc2_flee` — plain `mc2_target`, no resolver (sub_1C980)
  - `crates/mgc-sim/src/mc2/roster.rs:~1487` m9 chase (sub_20C50), `:~3470` m17 dive (sub_24930), `:~3890` m18 barrage case 1 (sub_250B0), `:4033 m19_attack` (HitFirebug_25610), `:~4850` m21 attack (sub_26220), `:~5500` m23 retaliation (sub_27E00), `:5686 m24_acquire` (sub_28690) — all plain `mc2_target`/`mc2_class3_scan`, no resolver
  - `crates/mgc-sim/src/mc2/mobs.rs:6099 mc2_alliance_clock` — the port's HEAD clock (runs at the class-5 dispatch head in every state), which stands down only for m16 state 2 and `CHASE_ATTACK_MODELS` state 2
- **Status:** DRIFTED
- **Differences:**
  - Clock owner: for the 7 `sub_1C310` species and m16 the charm clock in the attack state is counted by the resolver on the arm that reaches it (a damaged/dying tick or a wrapper that bails early counts nothing) — `MGC_NO_MC2_CHASE_ALLY_RESOLVE` (commit f7b8080, round ~142). For m9/m17/m18/m19/m21/m24 attack states and every species' flee state, `mc2_alliance_clock` keeps counting at the head every tick regardless of which arm runs (its own comment: "🏦 OWED … the head clock is one tick early there").
  - Species whose attack handler never calls `sub_1ED30` in retail (e.g. m28 melee brute, which is charmable) — retail's clock should not move at all outside state 7 (only `sub_1E9C0`, the state-7 body, and `sub_1ED30` decrement it); the port's head clock counts in every state.
  - Null answers: in the unported arms a charmed creature keeps fighting a target its caster is not fighting, and can keep attacking the caster itself — retail's resolver would null the lock (m9/m17/m18/m19/m21/m24 → exit to idle; flee → return to patrol).
  - m24 acquire: retail validates the resolver's answer with `life >= 0 && !reap` before committing action 194 (EF:18712-16); port `m24_acquire` commits whatever `mc2_class3_scan` returned. Since `MGC_NO_MC2_CLASS3_SCAN_ROSTER` made that scan a tick-top roster walk with no life/reap test, a class-3 record that died or was reap-flagged earlier in the tick is now committed by m24 where retail would reject it (the doc comment still claims "already in the scan").
- **What reaches each arm in-game:** MC2, human casts ALLIANCE on a group of one species. Ported arms: archers, m0 worms, m2 pack hunters, m3, m20 skirmishers, m24/m25 in their chase-attack state; m16 wyverns. Unported: charmed m9 hive imps, m17 dive-bombers, m18 tanks, m19 firebugs, m21 devils, m24 cave brutes (acquire), m28 melee brutes; goats / m24 brutes when they FLEE. The arms differ when the charmed creature is in its attack/flee state and (a) the caster is fighting someone else or nobody, (b) the charm clock runs out mid-attack, (c) the creature is hit or dying on a tick (no count in retail).
- **Already witnessed?:** `sub_1C310` arm — yes (mc2l17 slot 65 (5,20), doc on `no_mc2_chase_ally_resolve`; mc2l0 t=25983 charmed archers). m16 arm — yes (mc2l17 t=26941 "A CHARMED WYVERN WITH NO LOCK NEVER LEAVES ITS ATTACK STATE"). m9/m17/m18/m19/m21/m24/m28 charmed-in-attack and any charmed flee: no evidence found.
- **Recording ask:** MC2, a level with firebugs (take mc2l3 / mc2l6), tanks (mc2l5 / mc2l15), devils (mc2l10 / mc2l22), dive-bombers (mc2l8 / mc2l16 / mc2l18) or cave brutes (mc2l15 / mc2l32): cast Alliance (tier 1 is fine) on a group, then (1) fly near a rival or a hostile creature WITHOUT attacking it so the allies engage something you are not fighting, (2) then attack a different target, (3) stay near them until the charm expires while they are fighting (~610 ticks at tier 1 ≈ 30-40 s). Also charm goats and hit one so it flees. ~3-4 min per species; firebugs + tanks would cover the two largest unported bodies.
- **Confidence / notes:** High that the port lacks the call on those arms (grep: `mc2_ally_resolve` has exactly two call sites). The exact retail arm each caller sits on (quiet arm vs hit arm) differs per handler and was not individually mapped.

### MOB-3: Class-3 roster walk (`dword_38519`, tick-top wizard/castle/balloon list) — roster walkers vs live-pool walkers
- **Retail routine(s):** every walk of `dword_38519`: `sub_1BF90` wizard scan, `sub_1FAA0` archer Scan A, `sub_28690` m24 acquire, `sub_1DBF0` held kind-2 watch, `sub_23C40` m15 guard, `sub_203D0` m9 prey, m9 awake cone scan (:12159-93), `sub_282D0` m23 owner scan, `sub_16FC0` nearest-other-team wizard (summon re-acquire), `sub_29xxx` m27 scan. The chain is rebuilt at the tick top with `life >= 0` only; walkers test only range/cone/invisibility (+ id). A record that dies mid-tick stays a member, a record born mid-tick is not one yet.
- **Port arms:**
  - ROSTER (`wiz_chain`): `crates/mgc-sim/src/mc2/mobs.rs:1413 mc2_class3_scan` (`MGC_NO_MC2_CLASS3_SCAN_ROSTER`), `:1435 mc2_wizard_scan` (no switch), `mc2/roster.rs:2519 m15_scan` + `:2647 m15_scan_tests_any` (`MGC_NO_MC2_M15_SCAN_ROSTER`), m9 prey seek inside `mc2/roster.rs:1235 m9_tick` (~:1413, `MGC_NO_M9_PREY_ROSTER`), `mc2/multipart.rs:2008 m27_wizard_scan` (`MGC_NO_M27_SCAN_ROSTER`, see MP part), `mc2/effects.rs:948 mc2_mine_scan`
  - LIVE POOL (life/reap re-tested): `crates/mgc-sim/src/mc2/roster.rs:1201 m9_cone_scan` (its own doc says it walks `dword_38519`; the code walks `self.ent` with `act_life >= 0 && !0x400`), `mc2/roster.rs:5237 m23_owner_scan` (doc says `dword_38519`; code walks the pool), `mc2/mobs.rs:5162 mc2_nearest_rival_wizard` (`sub_16FC0`, pool), `mc2/mobs.rs:6280 mc2_alliance_parent_attacker` (port APPROX, pool)
- **Status:** DRIFTED
- **Differences:**
  - Membership: roster walkers see a class-3 that died earlier this tick and do not see one born this tick (mc2l10 t=12683 / mc2l32 t=4341 witnesses); pool walkers do the opposite. Four walkers still on the pool with no kill switch (the law never reached them).
  - Human entry test: `mc2_mine_scan` and the doom-home re-acquire use the TICK-TOP `ctx.pdead_top`; `mc2_class3_scan`, `mc2_wizard_scan`, `m15_scan`, `m9_cone_scan`, `m23_owner_scan`, `mc2_nearest_rival_wizard` use the LIVE `ctx.pdead` (`mc2_class3_scan` doc: "🏦 OWED — the out-of-pool human still enters on the LIVE `ctx.pdead`").
  - Model filter: `mc2_wizard_scan` dropped its `model <= 1` filter (castles/balloons returnable, mc2l3 t=10222); `mc2_nearest_rival_wizard` keeps `model65 <= 1` — retail `sub_16FC0` per its doc filters model 0/1 itself, so this one may be right; noted, not ruled.
  - Downstream: `m24_acquire` lost its life/reap validation when `mc2_class3_scan` switched to the roster (see MOB-2).
- **What reaches each arm in-game:** MC2. Roster arms: most creature acquisitions of wizards/castles/balloons. Pool arms: m9 hive imps awakening and choosing a target (`m9_cone_scan`), m23 mana leviathans picking a wizard/balloon owner to siphon (`m23_owner_scan`), Summon-Army creatures re-acquiring a rival wizard (`mc2_nearest_rival_wizard`), charmed allies of the human choosing an attacker of the human (`mc2_alliance_parent_attacker`). The arms differ only on a tick where a class-3 record (a wizard carpet, a castle, a balloon) is born or dies/gets reap-flagged mid-tick close to the scanning creature, or the human dies.
- **Already witnessed?:** roster arms — yes (mc2l10 t=12683 "THE DEATH DIRECTION", mc2l22 t=4858 m15 guard, mc2l5 t=32530 m9 prey). `m9_cone_scan`, `m23_owner_scan`, `mc2_nearest_rival_wizard` under a mid-tick class-3 birth/death: no evidence found.
- **Recording ask:** MC2, m9 hive level (take mc2l4/mc2l5/mc2l15): let an awake hive sense you while a rival wizard dies or a castle is being built/demolished nearby — e.g. kill a rival right next to an imp hive, and build a castle next to one. m23: level with mana leviathans (mc2l22/mc2l24): fill a balloon / have a wizard die while a leviathan is choosing its owner. Summon Army: cast it, then kill the nearest rival wizard while the summons are re-acquiring. Each ~2 min; the exact tick is luck, so longer takes help.
- **Confidence / notes:** High for the pool-vs-roster split (read all bodies). The human-entry (`pdead` vs `pdead_top`) half is expanded in MOB-28. The mid-tick window is narrow, so a recording may need several attempts to hit it.

### MOB-4: Building roster walk (`dword_38527`, tick-top (10,45) list) — roster walkers vs live-pool walkers
- **Retail routine(s):** walks of `dword_38527`: `sub_24440` m16 wide building sweep, m12 builder site/seek (`sub_22760`), `sub_23340` villager dwelling seek (EF:14570), `sub_237B0` trader far-dwelling seek (EF:14797). Per-node test is `bldgprm[byte_0x46_70].byte_2 & 1` + distance only (no reap, no life).
- **Port arms:**
  - ROSTER (`bldg_chain`): `crates/mgc-sim/src/mc2/roster.rs:3188 m16_tick` state 1 (`MGC_NO_MC2_M16_SWEEP_ROSTER`), `mc2/roster.rs:1644 m12_tick` / `:1898 mc2_m12_build`
  - LIVE POOL: `crates/mgc-sim/src/mc2/mobs.rs:2921 villager_brain` (dwelling seek, `class==10 && model==45 && !0x400 && build_tab.get(f71).is_some() && bldgprm&1`), `mc2/roster.rs:2207 m14_brain` (far trade building, `!0x400 && bldgprm&1 && d2 > 0xE100000`)
- **Status:** DRIFTED
- **Differences:**
  - Villager and trader seek walk the live pool with a reap test; retail walks the tick-top building roster with no reap test (a house reaped earlier in the tick is still a candidate; a house born this tick is not). The m16 sweep got this law in commit f7b8080; the two townie brains did not.
  - Villager seek carries an extra `self.assets.build_tab.get(c.f71).is_some()` gate that neither retail `sub_23340` nor the port's trader copy has.
  - (IDENTICAL, for the record) the target-HOLD test `class==10 && model==45` with reap-blindness (`townie_target_reap_blind`, `MGC_NO_MC2_TOWNIE_TARGET_REAP_BLIND`) is on all three: `archer_brain`, `villager_brain`, `m14_brain` (+ `m12_site_reap_blind` on m12).
- **What reaches each arm in-game:** MC2. Villagers `(5,13)` and traders `(5,14)` choosing a house to walk to; the wyvern (m16) choosing a house to attack; builders (m12). They differ on the tick a house is destroyed (reap-flagged) or finished/born, when a villager/trader is on its scan cadence tick and has no target.
- **Already witnessed?:** m16 roster — yes (mobs.rs test `the_wyvern_sweep_locks_a_house_reaped_earlier_this_tick`; mc2l22 fixture t=17804 m12 vetoes). Villager/trader seek at a house birth/death: no evidence found.
- **Recording ask:** MC2, an early village level (mc2l0 / mc2l1): with villagers and traders wandering, destroy a house (fireball) and let a village build a new one, staying near the village for ~2-3 min so many scan ticks overlap a house death/birth.
- **Confidence / notes:** High (retail listing lines 14570 and 14797 both load `dword_38527`).

### MOB-5: Per-model roster walk (`bytearray_38403x[model]`, tick-top class-5 lists) — coverage census
- **Retail routine(s):** pack scan `sub_1BF90` tail (EF:9183), chase/flee packmate avoidance `sub_1C310`/`sub_1C980`, m15 wander, summon crowd-steer `sub_1E700`, m22 anti-stack, archer Scan B, m9 food scan `sub_203D0`/`sub_20940`, m23 lift-off, m28 strike-taken, model-extinct test, pyramid summon pick, awake pre-pass `sub_68BF0`.
- **Port arms:** roster-walking: `mc2/mobs.rs:1526 mc2_pack_scan`, `:1618 mc2_avoid_packmate_at` (called from chase/flee/summon core/held walk/doom home), `mc2/roster.rs:1137 m9_consume_scan`, archer Scan B in `mc2/mobs.rs:2518 archer_brain`, `m15_wander`, `m23_lift_off_packmate`, `m28_strike_taken`, `mc2/stagevars.rs mc2_model_extinct`, `mc2/doomsday.rs mc2_pyramid_pick_summon`, `mc2/mobs.rs:6420 mc2_awake_pass` (roster ORDER, `MGC_NO_MC2_AWAKE_ROSTER_ORDER`). Pool-walking same-model loops remaining: `mc2/mobs.rs:5947 mc2_alliance_convert` (see MOB-6; retail walks map tile buckets, not the roster).
- **Status:** IDENTICAL (all arms read `mob_chains` via one switch `MGC_NO_MC2_MOB_CHAIN_PREDICATE`, except archer Scan B `MGC_NO_MC2_ARCHER_SCANB_M9_ONLY` and doom crowd `MGC_NO_MC2_DOOM_CROWD_ROSTER` which carry their own pre-fix bodies)
- **Differences:** None found on the default path. The only per-arm variation is the box test sign-cast (`cast16`), which is `packbox_span_law()` on every caller but the held walk hard-codes `true` (`stagevars.rs:1620 mc2_avoid_packmate_at(i, true)`) — i.e. `MGC_NO_PACKBOX_SPAN` does not reach the held walk. Behaviour equal on default.
- **What reaches each arm in-game:** any MC2 creature pack.
- **Already witnessed?:** yes (mc2l3 t=253/257 firebug roster; mc2l24 crowd; mc2l4 archer Scan B).
- **Recording ask:** none needed.
- **Confidence / notes:** Listed so the collapse knows these are already one law.

### MOB-6: Charm eligibility `sub_3A7F0` — faithful predicate vs the Alliance sweep's inline copy (and the sweep geometry `sub_3A650`)
- **Retail routine(s):** `sub_3A7F0` (EF:29676) the charm-eligibility predicate; `sub_3A650` (EF:29611) the (10,74) executor's Alliance conversion, which walks the MAP TILE BUCKETS (`mapEntityIndex_15B4E0`, `oldMapEntity_0x16_22` chains) over a `2r × 2r` tile square from `((victim+128)>>8) - r` and calls `sub_3A7F0` on each same class+model record.
- **Port arms:**
  - `crates/mgc-sim/src/mc2/mobs.rs:5917 mc2_charm_eligible` — faithful predicate, used by the (9,25) alliance carrier's auto-target (`mc2/proj.rs:4232`)
  - `crates/mgc-sim/src/mc2/mobs.rs:5947 mc2_alliance_convert` — inline re-implementation inside the conversion sweep (called from `mc2_alliance_exec_tick` :6019 and `mc2/proj.rs:1899`)
- **Status:** DRIFTED
- **Differences:**
  - Inline copy adds `flags & 0x400 == 0` and `act_life >= 0` (predicate has "no life test and no reap test anywhere in it").
  - Inline copy admits only `site_z` 0 or 10 (skips stage-HELD kinds 1..9, 15 — port "APPROX"); the predicate (and retail) excludes only 13/14/16/17, and retail CONVERTS held creatures (saves StageVar1 to `word_0x4A_74`, zeroes StageVar1) — the port has no save/restore.
  - Model bar tested once on the VICTIM in the inline copy; predicate tests per candidate (same model, so equivalent), but the `m25 && f71 != 0` exclusion is per-candidate in both. Equivalent.
  - Sweep geometry: port walks the whole pool with `|(x>>8) - (vx>>8)| <= radius` (a `2r+1` square, no +128 rounding, no byte wrap); retail walks tile buckets `[(vx+128>>8) - r, +2r)` with u8 wrap — a different square (off by one on the far edge, rounding at tile halves) and bucket order instead of slot order (order matters only for the per-conversion sound).
- **What reaches each arm in-game:** MC2, human casts Alliance on a creature. Inline copy: every conversion. Predicate: the (9,25) Alliance carrier choosing its target before it flies. They differ when the charm area contains a same-species creature that is stage-HELD (a guardian parked by a level trigger — StageVar1/2 kinds 1-10), dead-but-not-reaped (in its death animation), or sits on the square's edge tile.
- **Already witnessed?:** predicate — yes (mc2l0 t=25449 "`case 0x19`'S CREATURE FILTER IS `sub_3A7F0` ITSELF"). Conversion of a held / dying / edge-tile creature: no evidence found (mc2l0 t=23945/23947, mc2l17 t=26938 are ordinary conversions).
- **Recording ask:** MC2, a level with trigger-held guardian creatures of a charmable species standing near free ones of the same species (e.g. held devils on mc2l10 / mc2l22 per MOB-3 witnesses): cast Alliance on a free one next to the held group; also cast it on a group where one member is mid-death (hit it with a spell first). ~2 min.
- **Confidence / notes:** High on the code diff. Which stage-held kinds exist on which level is best found from `mc2_held_tick` witnesses in the MP part.

### MOB-7: Townie hit head with the "wanted" arm (`sub_22C80`/`sub_22E60`/`sub_23020`/`sub_23340`/`sub_237B0`)
- **Retail routine(s):** five sibling state-handler heads (m12 builder ×3, m13 villager, m14 trader) — on a HIT (`v1 == 1`) arm the attacking wizard's wanted timer (`word_0x248_584 = 200`), latch the attacker, go to flee.
- **Port arms:**
  - `crates/mgc-sim/src/mc2/roster.rs:1588 mc2_head_wanted` — used by m12 (`:1664/:1696/:1780`) and m14 (`:2208`)
  - `crates/mgc-sim/src/mc2/mobs.rs:2921 villager_brain` — inline copy (state-head arm 1)
- **Status:** DRIFTED (switch coverage only; default behaviour equal)
- **Differences:**
  - `MGC_NO_MC2_HEAD_WANTED_HIT_ONLY` (commit 6f55e65) is consulted only in `mc2_head_wanted`; the villager copy was always hit-only.
  - `MGC_NO_WANTED_ANY_WIZARD` (commit 4b5efc4) is consulted in the villager copy (arm on any wizard vs only the human); `mc2_head_wanted` arms any wizard unconditionally via `mc2_is_wizard`.
  - The villager copy sets `f146`/`tick70` itself; `mc2_head_wanted` leaves that to its callers (each caller writes the same `+6`).
- **What reaches each arm in-game:** MC2, a wizard (human or rival) hits a villager (m13) vs a builder/trader (m12/m14).
- **Already witnessed?:** yes both (mc2l19 t=13663 m12 fatal head; mc2l0 villagers).
- **Recording ask:** none — default paths agree; this is a collapse note (one helper with one switch).
- **Confidence / notes:** High.

### MOB-8: `sub_583F0` 3-D distance — creature copy vs rival-wizard copy
- **Retail routine(s):** `sub_583F0_distance_3d` (NETHERW.EXE file 0x7CBF0): three 16-bit wrapping deltas, sum of squares, isqrt.
- **Port arms:** `crates/mgc-sim/src/mc2/mobs.rs:1756 Gen::mc2_dist3` — CREATURE (and many non-creature callers: roster, multipart, stagevars, proj, tail, cave, world); `crates/mgc-sim/src/mc2/rivals.rs:1340 mc2_sub_583f0` — RIVAL WIZARD
- **Status:** DRIFTED
- **Differences:** `mc2_dist3` takes `dz` as a full `i32` difference (`b.2 as i32 - a.2 as i32`, NOT wrapped to 16 bits) and sums in wrapping i32; `mc2_sub_583f0` wraps `dz` to 16 bits like dx/dy and sums in i64 narrowed to u32. The rival side also has a "rank in squared space" switch `MGC_NO_MC2_DIST3D_ISQRT` that the creature side does not. Results differ only when `|dz| > 32767` (a z far out of range, e.g. an OOB/sentinel z) — the rivals.rs doc names the overflow; the mobs.rs doc names a wrap witness on mc2l22 (`m12_tick` building scan against a far-z record).
- **What reaches each arm in-game:** creature arm — every creature reach/engage test; rival arm — rival AI target ranking. Divergence needs a record with an extreme z (ungraded in practice).
- **Already witnessed?:** no evidence found for the `|dz|>32767` case on either.
- **Recording ask:** none practical (needs an out-of-range z); collapse to one helper with the retail 16-bit `dz`.
- **Confidence / notes:** High on the arithmetic difference; low practical impact.

### MOB-9: Controlled-slot snap `sub_12500` (tick-top) vs the Alliance clock's mis-sited copy
- **Retail routine(s):** `sub_12500`'s arm shared by StageVar2 13/14/16/17 (NETHERW.EXE 0x36d8b-0x36da2), run from `UpdateEntities_57730`'s tick-top roster walk: phases 0,1,3,7 → `actionIndex = 8*model+7`.
- **Port arms:** `crates/mgc-sim/src/mc2/mobs.rs:4671 mc2_controlled_slot_snap` — TICK-TOP (13/14/16/17); `crates/mgc-sim/src/mc2/mobs.rs:6099 mc2_alliance_clock` tail (`if tick70 & 7 < 2 { tick70 = 8m+7 }`) — MID-WALK, SV2 14 only
- **Status:** DRIFTED (the second copy is explicitly "🏦 BANKED" in its comment)
- **Differences:** tick-top vs at the creature's own dispatch; phase gate "not 2/4/5/6" vs "< 2"; the mid-walk copy fires on the SAME tick a charmed creature drops into phase 0/1 during the walk, retail waits for the next frame's tick-top pass. `MGC_NO_SV_CONTROLLED_SLOT_SNAP` gates only the tick-top one.
- **What reaches each arm in-game:** MC2, a charmed (Alliance) creature whose fight resolves mid-tick (its target dies or leaves range, dropping it to phase 0/1) — the port re-slots it one tick early.
- **Already witnessed?:** tick-top snap — yes (mc2l6 t=20748 "A CONTROLLED CREATURE IS DRAGGED BACK INTO ITS OWN 8*model+7 SLOT"). Mid-walk copy firing where retail does not: no evidence found.
- **Recording ask:** MC2, charm a group (Alliance) and lead them into a fight with weak targets that die quickly (villagers/goats) so the allies drop out of attack often; ~2 min. (Overlaps MOB-27, which maps all four port homes of `sub_12500`.)
- **Confidence / notes:** Medium — whether any creature enters phase 0/1 mid-walk while charmed (without the attack handler immediately re-entering) depends on the species.

### MOB-10: Flee core `sub_1C980` vs chase-attack core `sub_1C310` (shared-shape twins)
- **Retail routine(s):** two distinct retail routines with the same skeleton (inbox head → move → resolver → re-aim every 4th phase with packmate avoidance → cadence range test).
- **Port arms:** `crates/mgc-sim/src/mc2/mobs.rs:1973 mc2_flee`; `crates/mgc-sim/src/mc2/mobs.rs:2008 mc2_chase_attack`
- **Status:** DRIFTED (retail has two routines; the drift is the resolver call, MOB-2)
- **Differences:** chase-attack routes the lock through `mc2_ally_resolve` (`MGC_NO_MC2_CHASE_ALLY_RESOLVE`); flee does not, though retail `sub_1C980` calls `sub_1ED30` (EventsFunctions.cpp:9578). Flee aims away (`+0x400`), chase aims toward and fires the thunk. Retail flee re-aims on `byte_0x3E_62 & 3` with its own packmate walk — port uses the shared `mc2_avoid_packmate`, equivalent.
- **What reaches each arm in-game:** MC2 goats, villagers, builders, traders and m24 brutes fleeing; everything else chasing. Differs only for a CHARMED fleeing goat or m24 brute.
- **Already witnessed?:** flee — yes (townies flee in mc2l0); charmed flee — no evidence found.
- **Recording ask:** covered by MOB-2 (charm goats, hit one).
- **Confidence / notes:** High.

### MOB-11: Awake pre-pass — MC1 `mob_awake_pass` vs MC2 `mc2_awake_pass`/`mc2_awake_one`
- **Retail routine(s):** MC1 awake pass (CARPET.EXE :64353, 0x6D7F8) vs MC2 `sub_68BF0`/`sub_68C70` (EF:55811). Retail has two different routines: MC2 propagates the counter down the sub-entity chain THEN decrements and has a hidden-bit skip; MC1 decrements then propagates, stamps `+48` (dy) on wake, no hidden skip.
- **Port arms:** `crates/mgc-sim/src/mc1/mobs.rs:4401 mob_awake_pass` — MC1/HW; `crates/mgc-sim/src/mc2/mobs.rs:6420 mc2_awake_pass` + `:6517 mc2_awake_one` — MC2
- **Status:** STRUCTURALLY-DIFFERENT (two retail routines; each port arm follows its own)
- **Differences:** MC2 walks the 29 per-model rosters in model order (`MGC_NO_MC2_AWAKE_ROSTER_ORDER`), skips tick-top-dead records; MC1 walks the pool in slot order with a `life >= 0 && state != 120` gate and writes `raw48` (`MGC_NO_MC1_WAKE_DY48`). Wake radius: MC2 hard-coded 0x2400000, MC1 `chassis.awake_gate_sq`. Both share the `map_wide_ball_rolling` patch leg.
- **What reaches each arm in-game:** any creature near the human, per game.
- **Already witnessed?:** yes both (mc2l24 t=49448 roster order; MC1 fixtures for dy48).
- **Recording ask:** none — not a collapse candidate beyond sharing the patch leg.
- **Confidence / notes:** Same pair as MOB-24 (MC1-part view, which rates it DRIFTED on the MC2 roster-order law); kept both for their different angles.

### MOB-12: Creature (9,13) arrow launchers — archer thunk `sub_1CCE0` vs m15 guard's inline volley
- **Retail routine(s):** `sub_1CCE0` (archer fire thunk, :9713) and the m15 guard's inline volley in `sub_23E60` (EF:15154-66) — two retail code paths; m15 is "the archer's launch minus the `sub_200F0` overrides".
- **Port arms:** `crates/mgc-sim/src/mc2/mobs.rs:2722 archer_fire`; `crates/mgc-sim/src/mc2/roster.rs:2685 m15_fire`
- **Status:** STRUCTURALLY-DIFFERENT (retail-enumerated differences only)
- **Differences:** archer writes `f44 = 250` and target class/model into f66/f67; m15 keeps f44, copies the GUARD's f66/f67. archer resolves the target itself via `mc2_target` (life-guarded); m15 takes a pre-resolved position. Both poke `player_danger = 100` on a human target.
- **What reaches each arm in-game:** MC2 archers `(5,4)` firing vs castle guards `(5,15)` firing.
- **Already witnessed?:** yes both (mc2l0 archers; mc2l22 t=4858 guard).
- **Recording ask:** none.
- **Confidence / notes:** Candidate for a shared spawn+aim helper, not a behaviour question.


---

## Part B — MC1 / HW creatures

### MOB-13: Shared chase core `sub_1A120` and its sister chase handlers (target deref / "scratch target" / lost-test order)
- **Retail routine(s):** `sub_1A120` (MC1 :21580 / HW :20211), the shared creature CHASE core: move, re-bear every 4th tick off the raw `&pool[+146]`, then the lost test `+12<0 || +17&4`, then the v_26 range / attack thunk. Used by m0,1,2,3,5,7,8 and m4 (via `sub_1BB20` = `sub_1A120(a1x,24,sub_1A990)`), plus m10 (`:24295`). Retail also has **separate** chase handlers that repeat the same shape: kraken `sub_1C4F0` (:23135), mound `sub_1DA60` (:24130ff), genie `sub_1E380` (:24554), castle guard `sub_201D0` (:25771), wyvern `sub_207E0` (:26062). These are different sub_ addresses, so retail really has distinct routines here.
- **Port arms:**
  - `mc1/mobs.rs:1538 mob_chase`: SHARED CORE (`sub_1A120`). Also hosts the folded-in kraken `sub_1C4F0` (`model == 6` branches) and mound `sub_1DA60` (`model == 9` branches).
  - `mc1/mobs.rs:2591 militia_chase_body`: a HAND COPY of `sub_1A120` for m4.
  - `mc1/mobs.rs:1944 wyvern_chase`: `sub_207E0`.
  - `mc1/mobs.rs:4312 guard_chase`: `sub_201D0`.
  - `mc1/mobs.rs:2310 genie_chase`: `sub_1E380`.
- **Status:** DRIFTED. The law has the same *effect* everywhere, but the kill switches sit on some copies and not on others.
- **Differences:**
  - **Scratch-target law (`+146 == 0` reads slot 0, no class test in the lost verdict):**
    - `mob_chase` carries it with **no kill switch**. The bound `t >= len` is memory safety only.
    - `militia_chase_body` carries it behind `MGC_NO_M4_SCRATCH_TARGET`. With the switch set, `t==0` gives an early return and the lost test gets a `class64==0` conjunct.
    - `wyvern_chase` and `guard_chase` carry it behind `MGC_NO_M15M16_SCRATCH_TARGET`. With the switch set, the wyvern adds `class64==0`, and the guard substitutes its OWN position as the target.
    - `genie_chase` has no switch. Its lost test *keeps* `class64 == 0`, which the comment says is retail's own `+64` test at :24636. On `t>=len` it runs `genie_home`.
    - All the switches landed in commits 4b5efc4 / fc4aee8. You can A/B the law on m4/m15/m16 only. The shared core (m0,1,2,3,5,7,8,10 + m6 + m9) has no A/B switch.
  - **Order of mover vs target read:**
    - `mob_chase`, `wyvern_chase` and `genie_chase` call `creature_move` FIRST.
    - `militia_chase_body` resolves the target first and moves afterwards. It returns *before* moving only on its `t>=len` / switch-on scratch exit. Retail `sub_1A120` moves first (:21654).
    - `guard_chase` never moves, and neither does retail `sub_201D0`.
  - **Re-aim cadence / range metric per arm (retail-distinct, listed so the collapse keeps them):**
    - Shared core: `&3`, un-squared 3-D `isqrt >= v_28`.
    - m9: `%10` re-aim AFTER the lost test, and `v_28 += castle +80` on a (3,2) target.
    - Wyvern: `&7` re-aim gated on `class==3 || d>=0x200`, 2-D SQUARED range.
    - Guard: `&3`, no mover.
    - Genie: `&7`, break-off by life fraction.
  - **Thunk return:** `mob_chase` returns the thunk result (m7 consumes it). The militia copy discards it, and so does retail `sub_1BB20`.
- **What reaches each arm in-game:**
  - Every chase needs a creature in CHASE state (role 2) whose `+146` is 0. The practical source is the **pack-death handoff**: a PACK follower (role 3) dies from damage that did not come through its own `+94` mailbox slot on that tick. The leader is then handed `+146 = dier.+40 = 0`.
  - Shared core: bees, crabs, vultures, m0/m3 worms, m7 throwers, griffons, m10, kraken, mounds.
  - Militia copy: m4 villager militia.
  - Wyvern arm: MC1 wyverns (5,16).
  - Guard arm: MC1 castle guards (5,15).
  - Genie arm: genies (5,11).
- **Already witnessed?:**
  - Shared core: yes (mc1l3 t=497 fixture "A +146 OF 0 IS THE SCRATCH RECORD, AND IT STEERS THE EXIT BEARING").
  - m4 militia: yes (mc1hwl1 t=345 "A +146 OF 0 IS THE SCRATCH RECORD (HIDDEN WORLDS column…)"; mc1l37 t=2734; mc1l2 t=8282 "THE m4 CHASE TAKES ITS LOST VERDICT BELOW THE RE-BEAR").
  - Wyvern: no evidence found. mc1l49 t=3058 witnesses only the dead-target (pdead) half.
  - Castle guard: no evidence found for the scratch half. mc1l49 t=4654 witnesses a different law, see MOB-21.
  - Genie: no evidence for `+146 == 0` (mc1hwl0 t=23461 covers the dead-human half).
- **Recording ask:**
  - MC1 wyvern level (mc1l34 / mc1l49 area): get wyverns to form a PACK (two or more near each other), then kill the FOLLOWER with an area / non-mailbox death (walled-in, or a same-tick kill) while the leader is chasing. Keep doing it for ~10 min. The leader must then keep hunting toward map origin (0,0) for a v_26 window.
  - The same for castle guards on a level where rival castles field guards (mc1l48/49): kill guard pack followers.
  - Low odds per attempt. The scratch condition is emergent.
- **Confidence / notes:**
  - High on the code comparison.
  - The "+40 == 0 at handoff" condition is taken from the code comment (:21716), not re-derived.

### MOB-14: Chase-entry "arm" trailers that stamp the bolt filter off `+146` (militia `sub_1BC50`, mound `sub_1DCD0`) and the two trailer tables
- **Retail routine(s):**
  - `sub_1BC50` (:22744), the militia shoulders his dart: one LCG draw, speed 0, filter = target class/model.
  - `sub_1DCD0` (:24236), the mound's chase entry: owner test, then speed 0, sprite 202, filter.
  - Also the per-model entry trailers in the idle/wander/pack wrappers: bee `sub_1B350/1B370/1B4C0`, m7 `sub_1C900/1CA00`, guard `sub_20410`.
- **Port arms:**
  - `mc1/mobs.rs:2536 militia_arm`: m4 (idle/wander/pack promotion).
  - `mc1/mobs.rs:3766 m9_enter_chase`: m9.
  - `mc1/mobs.rs:4282 chase_entry_trailer`: HIT-ARM table (promotion by damage).
  - Inline QUIET-dispatch copies in `mc1/mobs.rs:4885-5068 creature_tick`: `(4,0)` mob_idle + militia_arm, `militia_idle`, `(4,3)`, `(2,0|1|3)` `m2_lunge_arm`, `(7,1|3)` `m7_arm`, `(9,3)` `m9_enter_chase`, `m9_hidden`, `guard_wander → guard_enter_chase`.
- **Status:** IDENTICAL for the scratch law: both arms are gated by the same `MGC_NO_M4M9_ARM_SCRATCH_TARGET`. For the two trailer tables, the Hit table vs the Quiet dispatch agree on every (model, role) pair except the one noted below.
- **Differences:**
  - `chase_entry_trailer` lists `(15, 0)` → `guard_enter_chase`. The Quiet dispatch has `(13 | 14 | 15, 0) => {}` (state 90 does nothing).
  - Retail state 0x5A = `sub_1FF50`, which lives inside the corrupt `sub_1FF40` stub ("positive sp value"). Its body is probably an idle wrapper over `sub_19B10`, but I could not confirm it. So m15 IDLE (state 90) is UNKNOWN in the port on quiet ticks: it is a no-op there, while retail may run the shared idle and pack scan. I found nothing that puts a guard into state 90 in the port. Flag, don't rule.
- **What reaches each arm in-game:**
  - militia_arm: a villager militiaman acquires a target from idle/wander/pack.
  - m9_enter_chase: a hidden or packed mound acquires. The `+146 == 0` case needs the pack handoff (MOB-13).
- **Already witnessed?:**
  - militia: yes (mc1hwl1 t=3683 "A +146 OF 0 STAMPS THE SCRATCH RECORD'S FILTER").
  - mound: no evidence found for the `+146 == 0` arm.
  - m2 damage-promoted lunge arm: cited in code as mc1l42 t=19459 / t=21217. No fixture found.
- **Recording ask:** For the mound arm, an MC1 level with burrowers/mounds (mc1l5, mc1l32) where two mounds pack, and the pack follower dies non-mailbox. Rare, emergent. Otherwise nothing is needed.
- **Confidence / notes:** The m15 state-90 item is uncertain because of the decompile corruption.

### MOB-15: Wrapper head/tail work around the shared damage prologue (pre-work hoist + hit/death trailers)
- **Retail routine(s):**
  - Every per-model wrapper calls a shared core (`sub_19B10` idle, `sub_19D70` wander, `sub_1A120` chase, `sub_1A390` pack), and the damage prologue lives INSIDE that core.
  - So wrapper statements BEFORE the call run on hit and death ticks too, and so do wrapper statements AFTER it: a non-lethal hit and a lethal hit both `return` back into the wrapper.
  - The port centralises the prologue (`inbox()` above dispatch), so every wrapper head and tail has to be re-listed by hand in up to three places.
- **Port arms:**
  - `mc1/mobs.rs:4485-4530 creature_tick` pre-work block (runs above the intake): `(2,2)` m2_chase_prework, `(7,1)` m7_wander_prework, `(7,2)` m7_chase_prework, `(8,2)` griffon_chase_prework, `(9,1)` m9_hidden_prework, `(4,1)` the `+26 = 0` zero.
  - `mc1/mobs.rs:4537 Inbox::Dead` arm: trailers chase_exit_trailer(role 2), m0 flyer_bob, `(1,0)` m1_idle_trailer, m5_regen.
  - `mc1/mobs.rs:4673 Inbox::Hit` arm: trailers m0 flyer_bob, `(1,0)` m1_idle_trailer, m5_regen, `(4,2)` militia_chase_wanted_tail, `(16,1)` wyvern_house_hunt.
  - Quiet dispatch `mc1/mobs.rs:4889-5071`, including `if model == 6 && role == 2 { f126 = 30 }` BELOW the intake (:4885) and `if model == 6 && role 1|3 { f126 = 30 }` AFTER dispatch (:5069).
- **Status:** DRIFTED. The law has landed for most wrappers, each behind its own switch, but not for all of them.
- **Differences** (what runs on a hit or death tick, retail vs port):
  - LANDED:
    - m0 bob: Hit and Dead, no switch.
    - m1 IDLE trailer: Hit (`MGC_NO_HIT_TRAILERS`) and Dead (`MGC_NO_MC1_VULTURE_DEATH_MOVER`).
    - m5 regen: Hit and Dead.
    - m4 chase wanted tail: Hit (`MGC_NO_HIT_TRAILERS` + `MGC_NO_M4_CHASE_WANTED_TRAILER`).
    - m16 wander house hunt: Hit (`MGC_NO_MC1_WYVERN_HOUSE_HUNT_ON_HIT`).
    - m6 chase dive-clock `+26 = -10`: Hit (`MGC_NO_HIT_TRAILERS`).
    - Chase-exit trailers on death.
    - Pre-work hoists for m2, m7 (×2), m8, m9 and m4 (these have no switches of their own).
  - **NOT landed — m1 vulture WANDER trailer (`sub_1B200` :22255-83):** retail runs the GRAVE HUNT after `sub_19D70` returns, *unconditionally* on the v_26 cadence, on hit ticks AND on the death tick. The port runs `m1_grave_hunt` only on Quiet ticks (`(1,1)` dispatch). On a DEATH tick, retail's grave hunt can overwrite the fresh `+70 = 10` (death) with `+70 = 6` (idle, `sub_424F0(a1x,6)`) and set `+146` to the grave. The next tick's prologue then re-kills it with `+38 = +40`, and `+40` is normally 0 on a quiet tick. This can lose the human's kill credit.
  - **NOT landed — m6 kraken CHASE head `+126 = 30` (`sub_1C4F0` :23146, the first statement, above the prologue):** the port runs it below the intake. The code comment admits this is deliberate: "no corpus row has demanded it yet".
  - **NOT landed — m6 kraken WANDER/PACK tails (`sub_1C4A0` :23111 / `sub_1C880` :23270):** `+126 = 30` plus growl sound 37 when the core promoted to chase (`+70 == 38`). The port writes `+126 = 30` only on Quiet ticks and never plays sound 37 for this promotion, not even on quiet ticks.
  - **NOT landed — m8 griffon CHASE tail (`sub_1CE30` :23563):** the sound-38 screech every v_26 runs on hit ticks in retail. The port plays it only on quiet ticks. This is sound only, an ungraded lane.
- **What reaches each arm in-game:**
  - Vulture grave hunt: MC1, a vulture (5,1) in WANDER within v_28 of a GRAVE (10,40, left by a dead wizard) takes damage, or is killed, on its v_26 think tick.
  - Kraken tails: a kraken (5,6) wandering or packing is hit (by a wizard's attack, which promotes it) or killed.
  - Griffon screech: a griffon in chase is hit on a v_26 tick.
- **Already witnessed?:**
  - m0 bob: yes (mc1l1 t=3809 "THE m0 DEATH-TICK BOB"; mc1l5 bob note).
  - m1 idle death mover: yes (mc1l35 t=4521).
  - m5 regen: yes (mc1l32 t=29840 / 30954).
  - Wyvern hunt on hit: yes (mc1l34 t=14668).
  - m2 pre-work: yes (mc1l2 t=570 "WRAPPER PRE-WORK OUTLIVES THE DAMAGE PROLOGUE").
  - m7 chase head: yes (mc1l5 t=23064).
  - m8 griffon pre-work: cited in code as mc1l42 t=20162. No fixture found.
  - m4 wanted tail: cited in code as mc1l37 t=9270. No fixture found.
  - m1 wander grave hunt on hit/death: no evidence found.
  - m6 head/tails on hit: no evidence found.
- **Recording ask:**
  - (a) MC1 early level with vultures (mc1l0 / mc1l4 / mc1l35 have them): let a rival die (or kill one) so a grave exists, then fly among the vultures circling near it and shoot them repeatedly. Kill several of them within v_28 of the grave. ~10 min.
  - (b) A kraken level (mc1l14 / mc1l42): hit a WANDERING (not yet chasing) kraken with a spell so it promotes. The observable is `+126` and sound 37, and `+126` is probably already 30, so this is likely invisible in graded lanes.
  - The m8 screech is sound, an ungraded lane.
- **Confidence / notes:**
  - m1: high. I read `sub_1B200` myself: the hunt block has no gate except the cadence.
  - m6: high on the code. The observable impact is probably nil, because f126 is already 30.

### MOB-16: Tick-top roster chains vs live-pool scans (the "chain-vs-pool" law) across creature scans
- **Retail routine(s):** every retail creature scan walks a TICK-TOP roster chain (`str_36382[model]` per-model, `+36462` class-3 bucket[0], `+36466` ball chain, `+36470` house chain). Chain membership (sampled at tick top) is the only life filter.
- **Port arms (landed, walk the chain):**
  - `pack_scan` :1184.
  - `mob_pack` separation :3499.
  - `nearest_wizard_target` :1272: bucket[0] with the human on `pdead_top`.
  - `m1_grave_hunt` :1500.
  - `m5_wander` ball pick :2411.
  - `wyvern_house_hunt` :2037.
  - `militia_idle_body` burrower rung :2745.
  - `nearest_building_3d` :2887.
  - `feeder_wander` acquire :3156.
  - `m12_build` veto :2998 (`MGC_NO_MC1_BUILD_SITE_CHAIN`).
  - `m9_convert` :3829.
  - m9 castle hunt in `m9_hidden_body` :3933.
  - `first_wizard_with_mana` :2261, HUMAN leg only (`pdead_top`).
- **Port arms (NOT landed, live pool with live filters):**
  - `mc1/mobs.rs:4142 grid_walk`: the castle-guard same-model repulsion (`:25984`). Retail walks `str_36382[+65]` exactly like `sub_1A390`'s separation. The port walks `1..ent.len()` with `class64==5 && model==own && tick70!=120 && act_life>=0`. The port's own comment calls it "the exact lifted twin of mob_pack's :21796 walk", but the pack-walk chain fix (mc1hwl0 t=42648 seizure-blank witness) was never carried over.
  - `mc1/mobs.rs:2171 genie_eat_ball` (`sub_1E810` :24769): retail walks the BALL chain `+36466` with the per-node test `+65 == 39` alone. The port walks the pool with `class64==10 && model==39 && !(flags&0x400)`.
  - `mc1/mobs.rs:2261 first_wizard_with_mana`, the RIVAL leg (:24527): retail walks bucket[0] with `+65 <= 1 && +140 && !(+16&0x20)`. The port walks the pool with `class==3 && model==1 && act_life>=0 && !0x20 && +140`. So the human leg follows the chain law and the rival leg in the SAME function does not.
- **Status:** DRIFTED.
- **Differences:** The three unlanded arms differ from retail only when the population changes MID-TICK before the scanner's slot, or after a pool seizure blank:
  - (a) a member that died or was soft-killed earlier in the tick. Retail still sees it, the port skips it.
  - (b) a record born earlier in the tick. Retail can't see it, the port can.
  - (c) a NewEvent seizure blank (pool exhaustion). Retail sees an EMPTY chain for every later slot, the port sees everything.
- **What reaches each arm in-game:**
  - grid_walk: two castle guards of DIFFERENT owners within one tile of each other (same-owner guards skip each other through the `+24` test, in both), while one of them dies or is (re)spawned by its castle mid-tick. Or any seizure-blank tick near guards.
  - genie_eat_ball: a genie below max mana, on its v_26 tick, near a mana ball that another eater (a second genie, a crab, or a claim) consumed earlier in the same tick: retail double-credits. Or a ball dropped by a creature corpse earlier in the same tick: the port lets the genie eat it and retail doesn't.
  - Mana hunt, rival leg: a genie above 3/4 life wandering while a RIVAL wizard dies earlier in the same tick. Retail still blinks onto the dying rival.
- **Already witnessed?:**
  - Landed arms: pack separation (mc1hwl0 t=42647), militia rung (mc1l5), m12 veto (mc1hwl1 t=17541), wyvern house hunt (mc1hwl0), grave hunt (mc1l4 t=6887), bucket[0] (mc1l4 t=1016, mc1l48 t=4891), genie human leg (mc1hwl0 t=23461), seizure blank (mc1l48 t=34326, mc1l19 t=14742).
  - Unlanded arms: no evidence found.
- **Recording ask:**
  - Genie arms: mc1l42 (the genie level). Fight near genies while mana balls lie around, and kill creatures and rivals next to genies so balls drop and rivals die mid-fight. ~15 min.
  - Guard repulsion: a level where two wizards' castles both field guards close together (mc1l48/l49), with guards dying in the melee. Rare; a seizure-blank tick (huge spell spam that exhausts the pool) near guards is the more reliable trigger.
- **Confidence / notes:** High on the code. The genie ball scan also lacks nothing else (range test v_28² is present in both).

### MOB-17: Owner (`+24`) exclusions: one port scan serves retail scans that differ; the guard's hit-retarget owner test is missing
- **Retail routine(s):**
  - Class-3 bucket[0] scans with NO `+24` test: shared wander `sub_19D70` :21519, militia `sub_1B5D0` :22587, crab `sub_1BF60` :22876, griffon `sub_1CA50` :23483, genie aggro :24485, genie mana hunt :24527.
  - Scans WITH `j+24 != own+24`: m9 castle hunt :23753, m9 wizard scan :23802, guard wander `sub_1FF60` :25738.
  - Hit-intake retarget with an owner test: ONLY guard wander `sub_1FF60` :25727 (hw :24284): `if (attacker.+64 != 3 || attacker.+24 == own.+24) skip`. Every other prologue retargets on `class == 3` alone.
- **Port arms:**
  - `mc1/mobs.rs:1272 nearest_wizard_target`: ONE function for all these callers. It always applies `owner != PLAYER_TARGET` (human leg) and `owner == c.id24 → skip` (pool leg). The doc says "verbatim for the m9/m15 scans and a kept extension … for the rest".
  - `mc1/mobs.rs:4673` `Inbox::Hit` arm: `attacker_is_wizard(src)` only, with no owner test, for every model including m15 role 1.
  - `mc1/mobs.rs:2261 first_wizard_with_mana`: the human leg has `id24 != PLAYER_TARGET`; retail has no owner test.
- **Status:** DRIFTED. The hit arm is missing a per-handler test. The scans are STRUCTURALLY-DIFFERENT: one port function stands for several retail loops with different gates.
- **Differences:**
  - Guard hit: in retail, a castle guard (spawned with `id24 = castle owner`, `engine/features.rs:9947`) that takes non-lethal damage FROM ITS OWN OWNER while WANDERING (state 91) keeps wandering. In the port it retargets onto its owner and goes to chase (92), so it fires at its own wizard.
  - Scans: the port's owner skip on the shared-wander, militia, crab, griffon and genie callers is inert unless a creature on those paths carries a wizard's id in `+24`. I found none: the Undead Army mints m9 mounds with `f144 = own`, not `id24`, and m9 has retail's owner test anyway. So the extension is probably unobservable.
- **What reaches each arm in-game:** An MC1 castle with guards (owner = human or a rival). The owner's own damage has to reach a guard in WANDER state. Direct projectiles probably don't do that (the `+24` owner immunity is noted in `attack_thunk`'s doc), so it takes an area or effect writer that ignores the owner, e.g. the owner's fire / explosion puff spreading onto his own guard. Whether any such MC1 writer bills same-owner creatures is UNVERIFIED.
- **Already witnessed?:** Guard wander hit: no evidence found. Guard wander scan owner skip: no fixture found that isolates it.
- **Recording ask:** MC1, build a castle high enough to get guards, then cast area spells (fire wall, meteor / crater, fireball explosions) onto your OWN wandering guards, and also let a rival's own spells hit his guards. Watch whether a hit guard turns on its owner. ~10 min. If owner damage can never reach a guard, this arm is unreachable, and the recording will show that too.
- **Confidence / notes:** The retail owner test is certain (read at :25727). Whether same-owner damage can reach a guard is not established.

### MOB-18: Creature damage prologue: one centralised port intake vs per-handler inline prologues in retail
- **Retail routine(s):**
  - The inline mailbox block (`if +58 { apply ch0; walk +54 chain for a lower life } if life<0 …`) is textually repeated at the head of `sub_19B10`, `sub_19D70`, `sub_1A120`, `sub_1A390`, `sub_1CA50`, `sub_1B5D0`, `sub_1C4F0`, `sub_1DA60`, `sub_1E380`, `sub_1FF60`, `sub_201D0`, `sub_207E0`, settler `sub_1EED0`-family (:25040) and more.
  - The per-handler outcomes differ: retarget-and-promote, retarget-only, freeze, death-only abort (guard chase), wanted-mark (m12/13/14), owner test (guard wander), and deaf (no prologue at all).
- **Port arms:**
  - `mc1/combat.rs:1564 inbox` + `:1602 attacker_is_wizard`: the shared intake.
  - `mc1/mobs.rs:4537-4735` Dead / Hit match in `creature_tick`: the per-(model, role) outcome table.
  - `mc1/mobs.rs:1769 state_is_damage_deaf` (`MGC_NO_DEAF_STATES`).
  - `mc1/mobs.rs:4084 segment_follow`: a separate intake AFTER the move (`:21127`, a distinct retail routine).
  - Guard-chase exception `(15,2)` via `MGC_NO_M15_CHASE_HIT_FALLTHROUGH`.
- **Status:** STRUCTURALLY-DIFFERENT. The port already collapsed retail's N copies into one intake plus an exception table, so drift shows up as missing exceptions.
- **Differences:**
  - (1) Guard-wander owner test missing (MOB-17).
  - (2) `attacker_is_wizard` returns false for `src == 0`. Retail reads `pool[+40].+64` raw, so it reads the scratch record's class when `+40 == 0`. That is reachable only through the segment-chain path with `f40 = 0` (the scratch class is 0 unless staged), so it is probably inert.
  - (3) The per-handler hit outcomes are re-encoded as `match` arms. Any retail handler whose outcome isn't the default "class-3 → retarget and promote for roles 0/1, retarget for role 2, leader handoff for role 3" needs its own port exception. Present exceptions: m12/13/14 (wanted, no chase), m9 hidden (any attacker), m11 (ambush), m6 chase (`+26 = -10`), m15 chase (fall through). I did not audit every retail handler's hit arm. Not audited: sub_1CFF0 deaf ✓ (listed), m13/m14 idle (13|14,0) no-op, m15 idle (state 90).
- **What reaches each arm in-game:** Any creature taking damage in a given state. The unaudited ones are rare states.
- **Already witnessed?:**
  - Deaf states: yes (mc1l32 "THE DAMAGE-DEAF STATES").
  - Guard chase fallthrough: yes (mc1l49 t=4654).
  - Pack handoff: yes (mc1l2 t=8289).
- **Recording ask:** None specific beyond MOB-17. A broad "hit every creature type in every state" session on mc1l42 (the largest bestiary) would exercise the table.
- **Confidence / notes:** This cluster is here to map the seam, not to claim drift beyond items (1) and (2).

### MOB-19: Two-draw wander yaw jitter, inlined six times
- **Retail routine(s):** The same `rand` pair `+34 += (b(d2)+85)·(2·(d1%157/79)−1)` inlined in every wander-like handler (`sub_19D70` :21506, `sub_1B5D0` :22572, `sub_1DFE0` :24518, `sub_1EED0`, `sub_1F640`/`sub_1FAC0`, `sub_1D060`).
- **Port arms:** `mc1/mobs.rs` `mob_wander` :1466, `genie_wander` :2212, `militia_idle_body` :2745, `m12_wander` :2904, `feeder_wander` :3156, `m9_hidden_body` :3933 (castle-less else).
- **Status:** IDENTICAL. All six bodies are character-for-character the same four lines.
- **Differences:** none.
- **What reaches each arm in-game:** Any wandering creature of those models.
- **Already witnessed?:** Yes, broadly: every certified take with wandering creatures.
- **Recording ask:** none.
- **Confidence / notes:** Safe to collapse into a helper.

### MOB-20: 3-D distance `sub_42340_42680`, inlined everywhere
- **Retail routine(s):** `sub_42340_42680` (:52721): int16-truncated dx, dy AND dz, squared, summed, then `Distance_410CE` (isqrt).
- **Port arms:**
  - Inline in `mc1/mobs.rs`: `mob_chase` :1686, `militia_chase_body`, `guard_chase`, `genie_chase`, `attack_thunk` melee, `m9_convert`, m9 castle hunt, `m12_approach` :2978, `nearest_building_3d` :2887, `feeder_wander` :3156, `player_wall_slide` :1103.
  - `mc1/combat.rs:1954 dist3d`.
  - `mc1/rivals.rs:5231 rival_approach`.
  - `flight.rs:302 mc1_duel_dist`: WIZARD duel.
- **Status:** DRIFTED (one arm).
- **Differences:**
  - All the mob copies and `dist3d` / `rival_approach` truncate `dz` to i16 (`wrapping_sub`) and use wrapping sums.
  - `mc1_duel_dist` computes `dz = vpos.2 as i32 - spos.2 as i32` (NOT i16-truncated) and uses non-wrapping `+`. It diverges only when |dz| > 32767.
  - The wyvern chase's range test is 2-D squared, not this routine; that is retail-distinct, correctly.
- **What reaches each arm in-game:** `mc1_duel_dist`: a Duel tether between two bodies more than 32767 z-units apart. Practically unreachable.
- **Already witnessed?:** Mob copies yes (many). The duel copy's overflow arm: no evidence.
- **Recording ask:** none (unreachable). Belongs to the WIZARD/duel subsystem.
- **Confidence / notes:** High.

### MOB-21: Kraken buffet: human target vs rival target (creature-vs-wizard twin)
- **Retail routine(s):** `sub_1C4F0` :23215-31, the kraken tether: on ON-ticks of the `+26` cycle it writes the TARGET record's wizext `+24 = bearing+0x400`, `+26 = 256`, `+22 = 80` through `+146`'s `+160`, with no class test.
- **Port arms:**
  - `mc1/mobs.rs:1646-1668 mob_chase` (model 6), HUMAN arm: writes `self.player_knock = (dir, 80)` and sound 42 inline.
  - RIVAL arm: posts `mc1_buffet_post`, drained by `mc1/rivals.rs:1885 mc1_buffet_apply` from `engine/world.rs:10847 tick_arm_creature` right after the kraken's dispatch.
- **Status:** IDENTICAL in formula. The two arms live in different homes, and the switch is on one arm only.
- **Differences:**
  - The rival arm is gated by `MGC_NO_MC1_KRAKEN_BUFFET_RIVAL`; the human arm has no switch.
  - The human arm derives the bearing from `ctx.px/py`, the rival arm from the target record. They are the same source for each respective target.
  - Neither arm models the `+26 = 256` store.
  - A NON-wizard `+146` (castle, balloon) gets retail's write through a garbage `+160`; both port arms do nothing.
- **What reaches each arm in-game:** A chasing kraken tethering the human, or tethering a rival wizard (MC1 water levels with a kraken, e.g. mc1l14, mc1l42).
- **Already witnessed?:**
  - Rival: cited in code as mc1l14 t=2262..2276 wiz 1, plus the unit test `the_kraken_buffet_drags_a_rival_wizard_too`. No fixture found.
  - Human: unit test `kraken_beam_lays_segments_and_the_buffet_arms_the_knock`. No fixture found.
- **Recording ask:** Optional. A kraken level where a kraken chases the human for a full 132-tick cycle and later chases a rival. It only strengthens existing evidence.
- **Confidence / notes:** Belongs partly to WIZARD knock (other subsystem).

### MOB-22: Retail-distinct chase handlers folded INTO the shared port core (m6 `sub_1C4F0`, m9 `sub_1DA60`)
- **Retail routine(s):** Kraken chase `sub_1C4F0` and mound chase `sub_1DA60` are their own functions in retail (own prologue, own body). They are not `sub_1A120`.
- **Port arms:** `mc1/mobs.rs:1538 mob_chase` with `model == 6` (buffet, growl, burst) and `model == 9` (`%10` re-aim after the lost test, castle `+80` range extension), plus `creature_tick` dispatch `(9,2)` + `chase_exit_trailer(9)`.
- **Status:** IDENTICAL as far as I read. I compared `sub_1DA60` line by line: lost test before re-aim, castle extension, `sub_1AA40` thunk, exit trailer `if +70 != 56 → sub_1DD50`. The one known gap is the m6 head `+126 = 30` (MOB-15).
- **Differences:** Structural only. Any future law landed in `mob_chase` automatically reaches m6/m9 even though retail's routines are separate, and the reverse can happen too. Worth knowing for the collapse.
- **What reaches each arm in-game:** A kraken or mound in chase.
- **Already witnessed?:** Yes (mc1l42 kraken growl / dive-clock fixtures; mc1l32/mc1l4 mound fixtures).
- **Recording ask:** none.
- **Confidence / notes:** High.

### MOB-23 (MC1-vs-MC2, low priority): Corpse mana drop `sub_27690` vs MC2 `TransformEntityToManaSphere_36BA0`
- **Retail routine(s):**
  - MC1 `sub_27690` (:29674): one ball from a corpse.
  - MC2 `TransformEntityToManaSphere_36BA0` (EF:26867): N spheres. Same draw shape: one pre-draw, then `yaw = rand%0x71 + heading − 56`, `speed = rand%0x30 + 16`, fall `(1024 − dz)/8`.
- **Port arms:**
  - `mc1/combat.rs:9690 corpse_drop_mc1`: MC1 (and the MC2 fallback through `CorpseVerb`).
  - `mc2/mobs.rs:2109 mc2_mana_spheres`: MC2.
- **Status:** DRIFTED (MC1-only law). Low priority because these are two games' routines.
- **Differences:**
  - MC1 carries the self-seize re-read (`mc1_self_seized`, `no_mc1_self_seize`): all operands are re-read off the ball after allocation. MC2 has no equivalent.
  - MC2 splits the purse (`use_fraction`).
  - Velocity: MC1 uses direct `SIN/COS >> 16`, MC2 uses `polar_step` from origin. These are equivalent.
  - ⚠ Stale comment in `mc2_mana_spheres` ("MC1 clamps ≥ 0; MC2 does not"): MC1's clamp was removed (its own comment says "the old `.max(0)` flattened…").
- **What reaches each arm in-game:** Any creature corpse (MC1) or death (MC2).
- **Already witnessed?:** MC1 corpse drops: yes (mc1l0 corpus per code). MC2: yes (mc2l3 t=245 per code).
- **Recording ask:** none.
- **Confidence / notes:** Only the stale comment is a concrete finding.

### MOB-24 (MC1-vs-MC2, low priority): Awake pass: MC1 `sub_54F00_55430`/`sub_54F80` vs MC2 `sub_68BF0`/`sub_68C70`
- **Retail routine(s):**
  - MC1 `sub_54F00_55430` (:64268): walks the 20 per-model roster chains in MODEL order. For a member with life < 0 it stamps `+58 = 0xFA, +59 = 0`, otherwise `sub_54F80`. Then it walks the ball chain `+36466`.
  - `sub_54F80` DECREMENTS its own `+58` first and then propagates the new value down `+54`. The wake arm stamps `+48`, `+58 = 16`, and segments get 18.
  - MC2's pair propagates first and decrements after.
- **Port arms:**
  - `mc1/mobs.rs:4401 mob_awake_pass`: MC1/HW, walks the POOL in slot order with balls interleaved, filter `act_life>=0 && tick70!=120`.
  - `mc2/mobs.rs:6420 mc2_awake_pass`: MC2, walks the roster in model order, `MGC_NO_MC2_AWAKE_ROSTER_ORDER`.
- **Status:** DRIFTED (the roster-order law is in the MC2 arm only). Retail MC1 is roster-ordered too.
- **Differences:**
  - MC1 port uses pool order where retail uses per-model chain order. The dead-member `0xFA` arm is skipped in both ports (only reachable for a member that died between the tick-top rebuild and this pass).
  - The `+48` stamp is MC1-only (`MGC_NO_MC1_WAKE_DY48`).
- **What reaches each arm in-game:** Order matters only when one record hangs off two different heads' `+54` chains (MC2's hydra witness). MC1's multipart creatures (worms m0/m3, kraken m6) have one head per chain, so the drift is probably unobservable in MC1.
- **Already witnessed?:** MC1 awake: yes, broadly. MC1 order-sensitive case: none exists that I know of.
- **Recording ask:** none.
- **Confidence / notes:** Retail MC1 caller order read at :64268-64297.

---

## Minor notes (not clusters)
- `m12_build` anchor test (`mc1/mobs.rs:2998`) requires `class64 == 10 && model == 45 && a != 0`. Retail `sub_1EA40` (:24865-66) tests `!+64 || +65 != 45`, i.e. any nonzero class with model 45. The two differ only for a non-class-10 record carrying model 45. Probably unreachable.
- m13/m14 feeders: retail has two routines (`sub_1F640` / `sub_1FAC0`) that differ ONLY by the migrant `> 0xE100000` filter (diffed). The port already unifies them as `feeder_wander(distant)`. This is the retail→port direction of unification, fine.
- The three MC1 multipart ctors (`sub_38030` m0, `sub_384B0` m3, `sub_389E0` m6) are unified in the port as `spawn_worm` with model branches. `MGC_NO_MC1_KRAKEN_HEAD_F56` is on the m6 leg only (the head `+56 = 96` stamp): the law landed as a shared statement, and the switch just restores the old m6 skip. Nothing to record.
- The village-wanted `+528` writers (house `sub_28DC0` in `engine/features.rs:~11170`, the m12/13/14 hit arm, the death-state handlers in `mob_death`/`mob_corpse`, the griffon pounce and beam, and the militia chase tail) each gate on "attacker record `+65 <= 1`" in retail. The port gets the same effect through `flag_village_wanted`'s wizard lookup plus explicit model tests in two sites (house, militia tail). They look equivalent; I found no drift.


---

## Part C — MC2 multipart + held/controlled creatures

Scope: `mc2/stagevars.rs` (held seam, watch resolution, sv walk), `mc2/multipart.rs`
(m0/m3 chain, m22 worm, m27 hydra/kraken), the creature side of `mc2/tail.rs`, and the
seams in `mc2/mobs.rs::mc2_creature_tick` that are the other half of the same retail routine.
All paths below are relative to `crates/mgc-sim/src/`. Line numbers are as of HEAD `52e5b11`
plus the working tree.

The key fact behind most of this part: retail `sub_1D5D0(a1x, 8m)` (EF:9977, NETHERW VA
0x1D5D0) is ONE switch over `StageVar2`. It covers the held kinds 1..=0xA and the controlled
kinds 0xC/0xD/0xE/0x10/0x11. Every per-model phase-7 wrapper calls it first and then runs its
own tail. The port splits that switch into two seams:
- **HELD** = `World::mc2_held_tick` (stagevars.rs:898), for `site_z` 1..=10 | 15. It is
  intercepted at `World::tick_arm_creature` (engine/world.rs:10865).
- **CONTROLLED** = the `action & 7 == 7 && site_z != 0` arm of `Gen::mc2_creature_tick`
  (mc2/mobs.rs:4719-4926), for `site_z` 12/13/14/16/17.

The port also has a third column. Each per-model tick's own phase-7 arm (the **FREE** arm)
runs with `StageVar2 == 0`, where `sub_1D5D0` is a no-op but the wrapper tail still runs.

---


### MOB-25: The per-model phase-7 WRAPPER TAILS after `sub_1D5D0` (held vs controlled vs free)
- **Retail routine(s):** One wrapper per model, each `sub_1D5D0(a1x, 8m); <tail>` (MC2 NETHERW):
  - `sub_1F300` m0: the StageVar2 dodge+bob table.
  - `sub_1F8A0` m2: jiggle + lunge re-arm.
  - `AddScroll05_04_20140` m4: `@0x10 = 0` head + aim test.
  - `sub_20FC0` m9: engage pose.
  - `AddGoat05_01_1F5B0` m1: bleat draw + speed tail. The townie wrappers share the FLEE speed shape.
  - `sub_24DF0`/`AddFirebug05_13_25D50`/`sub_26020`/`sub_2B7B0` m17/m19/m20/m28: `byte_0x46_70 = 0`.
  - `sub_25550` m18: ground snap + `sub_253B0(2,0)` aim timer.
  - `sub_26470` m21: jump + mode re-apply.
  - `sub_28660` m24: pose.

  In game terms, this is what a creature does on the tick a hold or control handler releases or
  promotes it.
- **Port arms:**
  - `mc2/stagevars.rs:898 World::mc2_held_tick`: HELD (StageVar kinds 1..=10, 15).
  - `mc2/mobs.rs:4701 Gen::mc2_creature_tick` (controlled arm, lines 4719-4926): CONTROLLED
    (12 metamorph, 13 Summon Army, 14 Alliance, 16/17 pyramid summon).
  - The per-model FREE phase-7 arms: `mc2/mobs.rs:2385 goat_tick` `_` arm, `mc2/mobs.rs:2452
    archer_tick` `_` arm, `mc2/roster.rs:3726 m18_tick` `_` arm (ground snap), `mc2/roster.rs:4772
    m21_tick` +7 arm (`m21_wrapper_tail`), `mc2/roster.rs:5761 m24_tick` `_ => m24_pose`, and
    `mc2/multipart.rs:809 m0_tick`. FREE only matters for the UNCONDITIONAL tails, because with
    `StageVar2 == 0` no leg can promote the action.
- **Status:** DRIFTED
- **Differences** (HELD vs CONTROLLED; the order of the tails inside each seam also differs, but
  only one model's tail applies per entity):
  - **m18 aim timer** (`if action == 146 → m18_timer(2,0)`: one entity draw, `dword_0x10_16 =
    rand%200+200`):
    - HELD: present, `MGC_NO_MC2_M18_HELD_AIM` (stagevars.rs:1183).
    - CONTROLLED: absent. A controlled tank that is promoted to 146 takes no draw.
    - Landed in 3346b53 (mc2l15).
  - **m9 engage pose** (`if action == 74 → m9_engage_pose`: speed 0, sprite 202, quad, quarry
    class/model):
    - HELD: present, `MGC_NO_MC2_M9_HELD_ENGAGE` (stagevars.rs:1218).
    - CONTROLLED: absent. Also 3346b53.
  - **Goat bleat draw** (`goat_snd(0x4D)`, one per-entity LCG draw on every wrapper run):
    - HELD: present, unconditional for m1 (stagevars.rs:1320).
    - CONTROLLED: absent.
    - FREE: present (`goat_tick` `_` arm).
  - **FLEE speed tail** (`8m+6 → f126 = f128`, else phase 7 → `f126 = f130`) for FLEE-flagged
    rows (goats/townsfolk):
    - HELD: present (stagevars.rs:1323-1330).
    - CONTROLLED: absent. The code comment at mobs.rs:4757-4761 says both of these are "OWED …
      NOT landed".
  - **m4 archer aim tail** (`if action == 34 → archer_aim`):
    - HELD: present and NOT switch-gated (stagevars.rs:1097).
    - CONTROLLED: present under `MGC_NO_MC2_CONTROLLED_WRAPPER_TAIL` (mobs.rs:4763). A913653-era.
    - FREE: present (archer_tick).
  - **m17/m19/m20/m28 sub-state reset** (`action == 8m+2 → f71 = 0`):
    - HELD: present, NOT switch-gated (stagevars.rs:1295).
    - CONTROLLED: present under `MGC_NO_MC2_SUBSTATE_RESET_CONTROLLED` (mobs.rs:4859). Commit
      6f55e65.
  - **m21 jump + mode re-apply:**
    - HELD: `m21_jump` for `site_z` 1..=10 only, then `m21_wrapper_tail`. Gated only by
      `m21_wrapper_tail`'s own `MGC_NO_MC2_M21_WRAPPER_TAIL`.
    - CONTROLLED: `m21_jump` for 1..=10, and ALSO for 13/14/16 with `f68 = 0` first (the
      0xD/0xE/0x10 "zero rest base" arm). Everything is under an extra switch,
      `MGC_NO_MC2_M21_CONTROLLED_TAIL` (mobs.rs:4895).
    - The held seam cannot reach 13/14/16 after its legs, so the 1..=10-only case list is
      consistent there. The difference is the gating switch.
  - **m0 dodge+bob:** one shared fn, `Gen::m0_phase7_physics` (multipart.rs:646), called from both
    seams with the full `sub_1F300` table. The switch `MGC_NO_MC2_M0_PHASE7_CONTROLLED` reverts
    BOTH seams to the held-only 1..=10 arm. IDENTICAL.
  - **m2 jiggle + lunge re-arm:** both seams call the same `m2_wrapper_jiggle` /
    `m2_wrapper_lunge_rearm`. IDENTICAL, but the controlled arm is unwitnessed per its own comment.
  - **m18 ground snap, m24 pose, m4 `@0x10 = 0` head:** both seams, same switches
    (`MGC_NO_MC2_M18_GROUND_SNAP`, `MGC_NO_MC2_M24_WRAPPER_POSE`, `MGC_NO_MC2_ARCHER_10_SEED`).
    IDENTICAL.
- **What reaches each arm in-game:**
  - HELD: an MC2 level whose StageVar table holds a spawn in its phase-7 wait. Examples: the
    grazing goat herd (kind 2), the mc2:04 skeleton/archer guards (kinds 3/4), and the mc2l15 tank.
    The tail fires on the release tick, when one of these happens: the wizard walks into the
    kind-2 watch cone; a guardian's charge comes within `v_28`; the creature takes a hit from a
    different class or model; or it re-raises out of kind 10.
  - CONTROLLED: a creature under the human's (or a rival's) **Alliance/charm** (14), a **Summon
    Army** creature (13), a creature spawned by a **doomsday pyramid** (16/17), or a **Metamorph**
    puppet (12). The action-test tails fire when the controlled handler promotes the creature from
    `8m+7` to `8m+2`. That happens when a charmed or summoned creature engages an enemy (the
    `sub_1E700` core finds a target).
  - FREE: any ordinary creature (StageVar2 0) sitting at `8m+7`.
- **Already witnessed?:**
  - HELD m18 aim: yes (mc2l15 fixture "m18's PHASE-7 WRAPPER ENDS IN AN AIM TEST, AND THE HELD
    SEAM PRE-EMPTED IT").
  - HELD m9 engage: yes (mc2l15 fixture "m9's PHASE-7 WRAPPER ENDS IN THE ENGAGE POSE").
  - HELD m24 pose: yes (mc2l32 fixture "m24'S PHASE-7 WRAPPER ENDS IN AN UNCONDITIONAL POSE").
  - HELD m18 snap: yes (mc2l5 fixture "A STAGE-HELD (5,18) TANK SNAPS TO THE GROUND").
  - HELD m4 aim: yes (mc2l0 fixture "THE ARCHER'S PHASE-7 TAIL IS AN AIM RE-TEST").
  - HELD m2 jiggle: yes (mc2l1 fixture "A HELD WALKER STILL RUNS ITS WRAPPER TAIL").
  - HELD goat bleat: measured in the code comment (mc2l0 corpus); no dedicated fixture found.
  - HELD sub-state reset: mc2l3 slot 146 is cited in the code; no fixture note found.
  - CONTROLLED m4 aim: yes (mc2l0 fixture "THE CONTROLLED SEAM RUNS THE MODEL WRAPPER'S TAIL").
  - CONTROLLED m0: yes (mc2l24 fixture "THE m0 PHASE-7 WRAPPER'S StageVar2 JUMP TABLE RUNS ON THE
    CONTROLLED SEAM TOO").
  - CONTROLLED m21: yes (mc2l24 fixture "THE m21 PHASE-7 WRAPPER RUNS ITS JUMP CYCLE ON THE
    CONTROLLED SEAM TOO").
  - CONTROLLED sub-state reset: mc2l24-crazy is cited in the code; no fixture note found.
  - CONTROLLED m18 snap / m24 pose / m2 jiggle / m2 re-arm: no evidence found (each is marked
    UNWITNESSED in its code comment).
  - CONTROLLED m18 aim / m9 engage / goat bleat / FLEE tail: not implemented; no evidence found.
- **Recording ask** (MC2; each is a few minutes):
  1. Cast **Alliance** on a group of **(5,18) tanks** next to an enemy (a rival or hostile
     creatures), so the charmed tanks ENGAGE. This covers the missing m18 aim-timer draw on the
     controlled seam.
  2. Walk the charmed tanks across ground that is changing, for example a castle you demolish or
     a crater. This covers the m18 snap on the controlled seam.
  3. Charm **(5,9) imps/skeletons** and let them engage. This covers the missing m9 engage pose.
  4. Charm a **goat herd** or **townsfolk** (FLEE rows). If the charm filter admits them, let them
     sit charmed for about 30 s, then have them flee. This covers the missing bleat draw and the
     FLEE speed tail. If Alliance cannot target goats, say so; the arm may then be unreachable.
  5. Charm **(5,24) troglodytes** and **(5,2) walkers** and let them idle and engage. This covers
     the controlled m24 pose and the m2 jiggle and re-arm.
  6. A doomsday-pyramid level (mc2 level 25, the mc2l24 map) that lets tanks, imps or troglodytes
     spawn from the pyramid covers 16/17 as well.
- **Confidence / notes:** High. Both bodies were read in full. The action-test tails only fire when
  the controlled handler writes `8m+2` itself (engage), which is why "engage" is the recording
  condition. The bleat and FLEE tails are unconditional per wrapper run, so a charmed goat simply
  sitting in `8m+7` exercises them.

---

### MOB-26: The m27 (hydra/kraken) HELD head vs the shared `sub_1D5D0` legs; m27 0xDF held vs free
- **Retail routine(s):** `sub_29930` (EF:19644-19684, m27 action 0xDF) opens with a plain
  `sub_1D5D0(a1x, 216)`. This is the SAME shared leg switch every other held model runs:
  `sub_1DDA0` kind 1, `sub_1DBF0` kind 2, `sub_1D7C0`/`sub_1D700`/`sub_1D8C0` kinds 3/4/5, and
  `sub_1E1C0` kinds 6-9. Each of those opens with the inbox head. The head's chain-inherit is
  gated `model != 27` inside each leg (EF sub_1D8C0 / sub_1DDA0 / sub_1E1C0 all read
  `word_0x34_52 && model_0x40_64 != 27`). The legs call `sub_1B8C0`, which for model 27
  dispatches `sub_2AF10(a1x, 1)` AFTER its own `byte[1] & 8` forced-stop test. In game terms,
  this is what a StageVar-held hydra does while it waits.
- **Port arms:**
  - `mc2/stagevars.rs:2083 World::mc2_m27_held_tick`: HELD/m27. An inline head plus
    `m27_move` plus `mc2_held_watch`.
  - `mc2/stagevars.rs:898 World::mc2_held_tick` + `mc2_held_move` (1342) + `mc2_held_wizard_scan`
    (1697) + `Gen::mc2_state_head` (mobs.rs:1210): HELD/all other models.
  - `mc2/multipart.rs:3774 Gen::m27_tick`, `_` (0xDF) arm at ~3979: FREE/m27, StageVar2 0.
- **Status:** STRUCTURALLY-DIFFERENT. The port hand-inlines the shared leg switch for m27 instead
  of routing it through the same legs.
- **Differences:**
  - **Inbox head:** m27 uses an inline copy with NO weakest-link chain inherit. The generic seam
    calls `mc2_state_head`, which has NO `model != 27` gate. It is only correct because m27 never
    reaches it. That is the same law with two homes.
  - **Kind 1 (walk to point):**
    - Generic: `mc2_sv_walk`, which is the move, then every 8 ticks an aim at the point, the
      ±jitter every 64 ticks, and the packmate box.
    - m27: `m27_move` only. There is NO aim write toward the authored point, so the hydra turns
      toward whatever `f34` already holds.
  - **Kind 2 (graze leash):**
    - Generic: the 3072 leash box (`MGC_NO_MC2_LEASH_SEAM`), walk home or graze, plus the kind-2
      wizard watch `mc2_held_wizard_scan` → StageVar2 10.
    - m27: `physics = kind ∈ {1,3,4,5} || row.flags & 2 == 0`. Row 97 has flag 2, so kind 2 does
      NOTHING. It does not walk home from outside the leash (retail `sub_1DBF0 → sub_1DDA0 →
      sub_1B8C0 → sub_2AF10` would), and there is NO wizard-watch scan.
  - **Kinds 3/4/5 (shadow):**
    - Generic: move, then the 8-tick-gated watch resolve (`MGC_NO_SV_WATCH_CADENCE`,
      `MGC_NO_SV_WATCH_POST_MOVE_RESOLVE`), then the toward-aim, jitter, packmate box, and
      personal-space back-off (`MGC_NO_SV_BACKOFF`), with the graze on no watch
      (`MGC_NO_SV_SHADOW_GRAZE`).
    - m27: `m27_move` only. None of those five laws exist on this path. The watch handle is still
      resolved, via `mc2_held_watch(resolve=true)`.
  - **Kinds 6-9:** both skip movement for m27 because of the row flag. This matches retail's
    `sub_1E1C0`.
  - **Forced stop (`F_STOP`, retail `byte[1] & 8`):**
    - Retail `sub_1B8C0` consumes it and returns 4 BEFORE reaching `sub_2AF10`, so the m27 mover
      never runs that tick.
    - The port's held m27 path calls `m27_move` directly. `m27_move` consumes the bit itself and
      ALSO writes `tick70 = 216, f26 = 0` (its own EF:20890-94 arm). The held tick's `216` arm
      then converts that to `site_z = 15`, which marks the hydra inert to the stage machinery.
    - Whether retail's `sub_1B8C0`-first path can reach 216 here is unconfirmed. The port path
      certainly does.
  - **Guardian-after-head:** both seams, same `MGC_NO_MC2_HELD_GUARD_AFTER_HEAD`. The m27 arm is
    marked "UNWITNESSED on this seam".
  - **Free vs held 0xDF:**
    - Free `m27_tick` runs `m27_v34_enter(i)` at its head. That adjacency-scoped consume of
      `m27_v34_slot.2/.3` drops a stale publication.
    - The held path never calls `m27_v34_enter`. It only clears `.0/.1` before `m27_drive`, so a
      `.3` publisher tag is neither consumed nor dropped on held ticks. The v34 lane is stack
      residue, a seed-parity model.
- **What reaches each arm in-game:**
  - HELD/m27: an MC2 level that authors a StageVar hold on a (5,27) hydra template. I did not find
    one.
  - FREE/m27: every hydra, for example mc2l22/mc2l24 (levels 23 and 25).
  - Generic held: see MOB-25.
- **Already witnessed?:**
  - HELD/m27: no evidence found. No fixture note mentions `sub_29930` or a held hydra.
  - FREE/m27 0xDF: yes (mc2l22/mc2l24 m27 v34 fixtures, e.g. "THE M27 MOVER'S TURN-SEARCH FLAG IS
    THE BRANCH MACHINE'S v34").
  - Generic held: yes (see MOB-25; mc2l4/mc2l15 guardian fixtures).
- **Recording ask:**
  - First check whether ANY shipped MC2 level binds a StageVar to a (5,27). If none does, this arm
    is unreachable in campaign play, which is itself the answer.
  - If one does, record that level: approach the held hydra from outside its leash or watch radius,
    hit it once with a different class of attacker, and if possible catch it in a whirlwind (that
    sets F_STOP). Allow about 5 minutes.
- **Confidence / notes:** High on the code facts. The F_STOP ordering claim rests on the retail
  `sub_1B8C0` listing (EF ~8735-8742: the `byte[1] & 8` test precedes the `model == 27` dispatch).

---

### MOB-27: `sub_12500`, the per-creature StageVar REACTION pass, split into four port homes
- **Retail routine(s):** `sub_12500` (EF:5045-5131, NETHERW file 0x36D00), called from
  `UpdateEntities_57730`'s tick-top walk of all 29 class-5 roster chains (file 0x7C1A9-0x7C1E4)
  for every member with StageVar1 or StageVar2 non-zero. It is one jump table:
  - kind 1: arrival box → release.
  - kinds 3/4/5/8/9: fired bit → release, plus the dead-watch scrub.
  - kind 6: timer.
  - kind 7: disposition.
  - **case 0xA**: re-leash when not in phase 2/6.
  - **0xD/0xE/0x10/0x11**: snap `actionIndex = 8m+7` unless in phase 2/4/5/6.
  - Outer gate: phases 4/5 never react.

  In game terms, this is the start-of-frame check that releases held creatures and pulls
  controlled or charmed creatures back into their controlled slot.
- **Port arms:**
  - `mc2/stagevars.rs:601 World::mc2_stagevar_tick` (the per-entity loop from ~665): HELD kinds +
    kind 10 for entities in `mc2_sv_held`. It walks the side table, filtered by the tick-top
    roster under `MGC_NO_MC2_STAGEVAR_REACT_ROSTER`.
  - `mc2/mobs.rs:4671 Gen::mc2_controlled_slot_snap`: CONTROLLED 13/14/16/17, over the roster
    chains, `MGC_NO_SV_CONTROLLED_SLOT_SNAP`.
  - `mc2/stagevars.rs:542 World::mc2_kind10_resume_snap`: kind 10 for ANY roster member,
    `MGC_NO_SV_KIND10_RESUME`.
  - `mc2/mobs.rs:6099 Gen::mc2_alliance_clock`, lines ~6163-6175: an ALLIANCE-only mid-walk
    paraphrase, `if tick70 & 7 < 2 { tick70 = 8m+7 }`. The code itself flags it "MIS-SITED
    PARAPHRASE … 🏦 BANKED".
- **Status:** DRIFTED
- **Differences:**
  - **Walk and order.** Retail makes ONE walk, in roster-chain order, with each entity taking
    exactly one case. The port makes three passes in sequence: the stagevar side table, then the
    controlled snap over the chains, then the kind-10 snap over the chains. A kind-10 creature is
    therefore reacted to by `mc2_stagevar_tick` if it is in the held side table, and by
    `mc2_kind10_resume_snap` otherwise. For held creatures the order is side-table order, not
    roster order.
  - **Early return.** `mc2_stagevar_tick` returns early when the level has no StageVars. Kind 10
    for non-held creatures (for example an alliance creature whose charm expired and fell to
    `site_z = 10`) is then served only by the kind-10 snap.
  - **Roster gate.** The roster gate is switchable on the held pass
    (`MGC_NO_MC2_STAGEVAR_REACT_ROSTER`) and hard-wired on the other two.
  - **Alliance paraphrase.** It fires MID-WALK, at the creature's own dispatch, for phase < 2
    only. Retail does it at the tick top for phases 0/1/3/7. So a charmed (14) creature that
    drops to phase 0/1 during the walk is snapped the SAME tick in the port and the NEXT frame in
    retail. Its phase-3 case is covered only by the real snap. 13/16/17 have no paraphrase.
- **What reaches each arm in-game:**
  - Held pass: any StageVar-held creature (see MOB-25).
  - Controlled snap: a Summon-Army (13), charmed (14) or pyramid-summoned (16/17) creature whose
    fight ended (its target died), so it fell to idle.
  - Kind-10 snap: a held creature that broke into aggro (hit or wizard seen) and then calmed
    down, or a charm that expired (StageVar2 → 10).
  - Alliance paraphrase: a **charmed** creature whose target dies DURING the tick, before the
    creature's own dispatch, so it lands in phase 0/1 mid-walk.
- **Already witnessed?:**
  - Held pass: yes (mc2l8 fixture "THE KIND-1 ARRIVAL BOX SIGN-EXTENDS"; mc2l18 fixture "A
    CREATURE DEAD AT THE TICK TOP GETS NO STAGEVAR REACTION").
  - Controlled snap: yes (mc2l6 fixture "A CONTROLLED CREATURE IS DRAGGED BACK INTO ITS OWN
    8*model+7 SLOT", Summon-Army wyverns).
  - Kind-10: yes (mc2l0 fixture "STAGEVAR2 10 IS A ONE-TICK TRANSIT").
  - Alliance paraphrase: no evidence found for the mid-walk-vs-tick-top difference.
- **Recording ask:** MC2, any level with plenty of creatures (mc2l17's wyverns worked for the
  charm fixtures). Cast **Alliance** on a pack and send it at a weak enemy, so the enemy DIES
  WHILE several allies are attacking it. Repeat about 10 times. The condition that separates the
  arms is: an ally's target dies in the same tick, earlier in walk order than the ally. Allow
  5-10 minutes.
- **Confidence / notes:** Medium-high. I did not prove that the side-table order and the roster
  order differ observably. The alliance paraphrase is explicitly owed in the code.

---

### MOB-28: Tick-top class-3 roster scans — the out-of-pool human's ENTRY test (`pdead` vs `pdead_top`)
- **Retail routine(s):** Several distinct retail walkers of `dword_38519`, the tick-top class-3
  roster, whose membership test is `life_0x8 >= 0` at the tick top (EF:39975):
  - `sub_1BF90` (the shared wizard scan).
  - The inline class-3 scans in `sub_1FAA0` (archer Scan A), `sub_28690` (m24 acquire) and
    `sub_1DBF0` (held kind-2 wizard watch).
  - `sub_2A6F0` (the m27 branch scan).
  - The m9 cone scan.
  - `sub_23C40` (m15).

  They are DIFFERENT retail subs. The shared fact is the human's membership in the list. In game
  terms, this decides whether a creature "sees" the human on the tick he dies.
- **Port arms:**
  - `mc2/mobs.rs:1385 Gen::mc2_class3_scan`: ARCHER/M24/HELD-KIND-2. Human test: live
    `ctx.pdead`.
  - `mc2/mobs.rs:1435 Gen::mc2_wizard_scan`: GENERIC CREATURE. Live `ctx.pdead`.
  - `mc2/roster.rs:1201 Gen::m9_cone_scan`: M9. Live `ctx.pdead`.
  - `mc2/roster.rs:2519 Gen::m15_scan`: M15. Live `ctx.pdead` (under `roster`).
  - `mc2/multipart.rs:1980 Gen::m27_wizard_scan`: M27 BRANCH. Tick-top `ctx.pdead_top`, under
    `MGC_NO_M27_SCAN_ROSTER_LIFE`.
- **Status:** DRIFTED. The retail routines are distinct, but the human-membership law was landed
  on the m27 walker only. The `mc2_class3_scan` doc says it is "🏦 OWED — the out-of-pool human
  still enters on the LIVE `ctx.pdead` where the roster's own sample is `ctx.pdead_top`".
- **Differences:**
  - **Human entry:** m27 reads the tick-top life (a human who dies mid-tick is still a member).
    The other four read live death (a human who dies mid-tick vanishes from later walkers).
  - **Retail-real, not drift** (the retail subs differ; noted so a collapse does not merge them):
    - m27 uses strict `<` on range and has no invisibility filter (it honours only the port's
      ghost cheat).
    - class3/wizard/m9 use `<=` and skip invisible targets.
    - `mc2_wizard_scan` has `wanted_only`.
  - **Roster walks:** all five now walk `wiz_chain`. The class3 walk is gated by
    `MGC_NO_MC2_CLASS3_SCAN_ROSTER`, m27 by `MGC_NO_M27_SCAN_ROSTER`, and the m15 walk has its own.
- **What reaches each arm in-game:** in MC2 the human carpet DIES during a tick, killed by
  something dispatched before the scanning creature in that tick's walk. On that one tick, a
  creature that runs later in the walk scans for wizards:
  - an archer or troglodyte acquiring;
  - a StageVar-held grazer on the kind-2 watch;
  - an imp;
  - an m15;
  - a hydra branch.
- **Already witnessed?:**
  - m27 pdead_top: probably yes (mc2l22/mc2l24 hydra fixtures). No note names the life law
    specifically.
  - Others with the live test: mc2l3 t=11757 is cited in the `mc2_wizard_scan` code for a human
    dead for about 60 ticks. That case is not the death tick itself; no fixture for the death tick.
- **Recording ask:** MC2, a level with archers, troglodytes, grazing held herds, or a hydra.
  Deliberately die several times INSIDE creature scan cones (let archers or creatures finish you
  while other creatures face you at close range). Each death tick is one sample. Around 5 deaths
  is enough.
- **Confidence / notes:** Medium. `pdead` vs `pdead_top` semantics are taken from the code
  comments (world.rs:8303 builds `pdead_top` from `player.life < 0` at the tick top). I did not
  trace where `ctx.pdead` refreshes mid-walk. The m9/m15 bodies were only grep-checked.

---

### MOB-29: Seam-safe (sign-extended) coordinate BOX tests — landed in some creature walks, not others
- **Retail routine(s):** An idiom, not one routine. Every `abs(pos.x - other.x) < span` box test in
  NETHERW loads the coordinates with `movswl`: both are sign-extended to 32 bits, then subtracted,
  so the difference never wraps across the 0x8000 map-centre seam. The annotated remc2 source at
  `sub_1D8C0` states: "there is no movzwl of 0x4c/0x4e anywhere in the shipped exe". In game
  terms, this decides whether two creatures straddling the map-centre line count as "close".
- **Port arms:**
  - SEAM-SAFE, law landed:
    - `mc2/stagevars.rs:1342 mc2_held_move` kind 2 leash (`MGC_NO_MC2_LEASH_SEAM`).
    - `mc2/stagevars.rs:776 mc2_stagevar_tick` kind 1 arrival (`MGC_NO_MC2_KIND1_SEAM`).
    - `mc2/stagevars.rs:1664 mc2_sv_walk_after_move` back-off (unconditional).
    - `mc2/multipart.rs:1237 m22_antistack` (`MGC_NO_M22_ANTISTACK_SPAN`).
    - `mc2/mobs.rs:1656 mc2_avoid_packmate_at` (`cast16`: always true from the held walk,
      `MGC_NO_PACKBOX_SPAN` elsewhere).
    - `mc2/mobs.rs:1918 mc2_pack` (unconditional).
  - STILL 16-BIT WRAPPING (`a.wrapping_sub(b) as i16`), all live code, unswitched:
    - `mc2/roster.rs:2479/2493 m15_wander` (|d| < 256 packmate test).
    - `mc2/roster.rs:5342/5355 m23_lift_off_packmate`.
    - `mc2/roster.rs:6094/6116 m25_brain` (castle-near tests).
    - `mc2/roster.rs:2029/2086 mc2_m12_build`.
    - `mc2/doomsday.rs:1063 mc2_pyramid_devour` and `:1189 mc2_citadel_devour`.
    - Also outside MOB: `castle.rs:1180/1279/1332`, `cave.rs:662`, `flood.rs:897`,
      `rivals.rs:6239`, `mobs.rs:3964`.
- **Status:** UNKNOWN for the wrapping sites. I did NOT verify each site's own retail bytes; the
  "no movzwl" claim is remc2's annotation. It is DRIFTED as a class: the same law was landed in 5+
  walks by separate digs, each named "A LAW ON ONE CALL PATH IS NOT LANDED".
- **Differences:** In the landed arms, two entities straddling x or y = 0x8000 are about 65,000
  apart and never "adjacent". In the wrapping arms, the same pair reads as a few hundred apart and
  the box fires (packmate steer, lift-off, castle-near, devour).
- **What reaches each arm in-game:** creatures of the named models, or a pyramid devour, operating
  right at the MAP CENTRE line (tile 128 on either axis):
  - m15 packmates wandering across it;
  - m23 dwellers stacking;
  - m25 Cymmerians near a castle that sits on the centre line;
  - m12 builders on a site across it;
  - a doomsday pyramid devouring across it.
- **Already witnessed?:**
  - Landed arms: yes (mc2l9 leash, mc2l8 kind-1, mc2l22 m22 anti-stack, mc2l0-pd packmate).
  - Wrapping arms: no evidence found.
- **Recording ask:** MC2. On a level whose centre line has creatures (check which maps put
  m15/m23/m25 or a pyramid near tile 128), lure those creatures to fight or pack right on the
  centre line for a couple of minutes. The difference fires only while a pair straddles 0x8000
  within the box span.
- **Confidence / notes:** Low-medium on whether each wrapping site is wrong. Each needs its own
  byte check. Listed so the collapse does not unify them blindly. The roster.rs sites belong to
  the parent's per-model brains.

---

### MOB-30: The graze idiom (`+rand%0x71+142` every 16 ticks, fence-gated)
- **Retail routine(s):** `sub_1E1C0` quiet path (EF:10520-45, held kinds 6-9 and kind-2 inside
  the leash), and the `else` arm of `sub_1D8C0`'s cadence body (file 0x423a4) when the watch does
  not resolve. Two retail copies of the same instructions. In game terms, this is a held creature
  circling lazily.
- **Port arms:**
  - `mc2/stagevars.rs:1677 World::mc2_sv_graze`: GRAZE LEG (with move_core and the HOLD_STILL
    gate).
  - `mc2/stagevars.rs:1590-1596` in `mc2_sv_walk_after_move`: SHADOW NO-WATCH (move already done;
    `MGC_NO_SV_SHADOW_GRAZE`).
- **Status:** IDENTICAL (same draw, gate and arithmetic). The only differences are the ones
  retail itself has: the shadow arm runs after an 8-tick gate and without the HOLD_STILL
  row test.
- **Differences:** None beyond retail's own. The shadow arm has a kill switch; the graze leg has
  none.
- **What reaches each arm in-game:**
  - Graze leg: StageVar-held creatures of kinds 2/6-9.
  - Shadow no-watch: a kind-3/4/5 guardian whose watched entity does not resolve.
- **Already witnessed?:**
  - Graze leg: yes (mc2l9 leash witness, mc2l0 herd).
  - Shadow no-watch: yes, per the code (mc2l6 t=15670 slot 408); no fixture note found.
- **Recording ask:** None needed.
- **Confidence / notes:** High. It is a trivial duplicate and safe to share.

---

---

## Summary table (all parts)

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| MOB-1 | summon handler SV2 13 vs 16 (+14 via shared core) | DRIFTED | 3 | y (pyramid summons with a live rival; summons out when the human dies) |
| MOB-2 | `sub_1ED30` charm resolver: 10 retail callers, 2 port sites + head clock | DRIFTED | 11 | y (charmed firebugs/tanks/devils/dive-bombers/brutes fighting; charm expiring mid-attack) |
| MOB-3 | class-3 roster (`dword_38519`) walkers vs live-pool walkers | DRIFTED | 10 | y (class-3 born/dying mid-tick near m9 hives, m23 leviathans, Summon Army) |
| MOB-4 | building roster (`dword_38527`) walkers vs pool walkers | DRIFTED | 4 | y (house destroyed/built next to villagers and traders) |
| MOB-5 | per-model roster (`bytearray_38403x`) walkers | IDENTICAL | 12 | n |
| MOB-6 | charm eligibility: predicate vs Alliance sweep's inline copy | DRIFTED | 2 | y (Alliance over held / dying same-species creatures) |
| MOB-7 | townie hit-head wanted arm | DRIFTED (switch coverage only) | 2 | n |
| MOB-8 | `sub_583F0` distance: creature vs rival copy | DRIFTED | 2 | n (needs an out-of-range z) |
| MOB-9 | controlled-slot snap vs alliance-clock copy | DRIFTED | 2 | y (see MOB-27) |
| MOB-10 | flee core vs chase-attack core | DRIFTED (resolver) | 2 | covered by MOB-2 |
| MOB-11 | awake pass MC1 vs MC2 | STRUCTURALLY-DIFFERENT | 2 | n |
| MOB-12 | archer vs castle-guard arrow launch | STRUCTURALLY-DIFFERENT | 2 | n |
| MOB-13 | MC1 shared chase core `sub_1A120` + sister chase handlers | DRIFTED (switches on some copies) | 5 | y (wyvern/guard pack whose follower dies) |
| MOB-14 | MC1 chase-entry arm trailers + trailer tables | IDENTICAL (m15 state-90 UNKNOWN) | 4 | n |
| MOB-15 | MC1 wrapper head/tail work vs damage prologue | DRIFTED | 4 | y (vulture hit/killed near a grave; kraken promoted by a hit) |
| MOB-16 | MC1 roster chain vs live-pool scans | DRIFTED | 16 | y (mc1l42 genies among balls and dying rivals; guard melee) |
| MOB-17 | MC1 owner (+24) exclusions / guard hit-retarget | DRIFTED / STRUCTURALLY-DIFFERENT | 3 | y (own area damage on own wandering castle guards) |
| MOB-18 | MC1 damage prologue: central vs per-handler | STRUCTURALLY-DIFFERENT | 5 | n |
| MOB-19 | MC1 wander yaw jitter | IDENTICAL | 6 | n |
| MOB-20 | MC1 3-D distance `sub_42340_42680` | DRIFTED (duel copy only) | ~15 | n (unreachable) |
| MOB-21 | MC1 kraken buffet human vs rival | IDENTICAL (switch on rival arm only) | 2 | n (optional) |
| MOB-22 | MC1 m6/m9 chase folded into shared core | IDENTICAL | 1 | n |
| MOB-23 | corpse mana drop MC1 vs MC2 | DRIFTED | 2 | n |
| MOB-24 | awake pass MC1 vs MC2 (MC1-part view of MOB-11) | DRIFTED (MC2 only) | 2 | n |
| MOB-25 | MC2 phase-7 wrapper tails: held vs controlled vs free | DRIFTED | 3 seams × models | y (charmed/summoned tanks, imps, goats, troglodytes, walkers engaging) |
| MOB-26 | m27 held head vs shared `sub_1D5D0` legs | STRUCTURALLY-DIFFERENT | 3 | y only if a level holds a hydra (maybe unreachable) |
| MOB-27 | `sub_12500` reaction pass in 4 port homes | DRIFTED | 4 | y (charmed pack whose target dies mid-fight) |
| MOB-28 | class-3 scans: human entry test `pdead` vs `pdead_top` | DRIFTED | 5 | y (die inside creature scan cones) |
| MOB-29 | sign-extended box tests across the 0x8000 seam | UNKNOWN (class-drifted) | 11 | y (creatures on the map-centre line) |
| MOB-30 | MC2 graze idiom | IDENTICAL | 2 | n |

## Clusters seen that belong to OTHER subsystems
- **SPELL/EFFECT — whirlwind `sub_33340`**: one retail victim body split into the pool-victim loop and the human-victim arm inside `mc2/tail.rs:1819 mc2_whirlwind_lift`; the human arm carries ~10 human-only switches (`MGC_NO_MC2_WW_HUMAN_*`, `WW_CRANK_*`, `WW_TAIL_*`, `WW_MIDRING*`). Pool arm laws not diffed against them. (The `v40` wizard constants apply to class 3 model 0 only, so rivals (3,1) taking creature constants is NOT drift.)
- **SPELL/EFFECT — `mc2_whirlwind_contact`** (`tail.rs:2729`, `sub_33710`): retail bills the tick-top BUILDING and CASTLE lists; port walks the live pool with `0x400` / castle `act_life >= 0` guards — likely the roster law, unlanded.
- **SPELL/PROJ — MC1 vs HW**: `mc1/combat.rs` `is_hidden_worlds` branches (manifestation reach world.rs:15944, firewall bolt sprite / napalm :7552, 53-state blast :6966, 20-case reach :1715, cone :3898) — per-binary arms, not creature code.
- **SPELL — Alliance impact**: `mc2/proj.rs:1899` calls `mc2_alliance_convert` inline with `dmg` as the duration, a second entry beside the (10,74) executor `mc2_alliance_exec_tick` (see MOB-6).
- **WIZARD/DUEL**: `flight.rs:302 mc1_duel_dist`, the one `sub_42340` copy without i16 dz truncation (MOB-20). `mc2/rivals.rs:1340 mc2_sub_583f0` vs `Gen::mc2_dist3` (MOB-8).
- **WIZARD KNOCK**: kraken buffet — human `player_knock` vs rival `knock_dir/knock_mag` homes (MOB-21); the `+26 = 256` store is unmodelled in both.
- **BUILDINGS**: MC1 house damage intake / defender pop / wanted arm in `engine/features.rs:~11100-11180` (`sub_28DC0`) with its own inline intake; MC2 villager dwelling rally vs m14 trader vs archer shrine hold tests (MOB-4) touch building capacity `f26`.
- **MOVE CORE**: `mc2_move_core` (`sub_1B8C0`, mobs.rs:1139) vs `m27_move` (multipart.rs:3557, `sub_2AF10`) — `m27_move` has a grab/tossed-latch release (`MGC_NO_M27_MOVE_FLAGS`) that `mc2_move_core` lacks; retail `sub_1B8C0` equivalence unchecked. `mc2_rival_movement` (rivals.rs:4366) is `sub_146F0`, a different routine (not a twin of `sub_1B8C0` despite citing it).
