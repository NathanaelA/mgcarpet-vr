# ARM CENSUS — SUBSYSTEM LIFE (wizard damage, death, dead-wait, respawn, kill credit, XP, mana debit/regen, floors)

Read-only census. Line numbers are HEAD of branch `re_recording` (working tree, 2026-09-24).
File paths are relative to `crates/mgc-sim/src/`. "HUMAN/MC1" includes Hidden Worlds (HW) unless stated.
Retail citations: remc1 = `reference/remc1/sub_main.cpp` (`:NNNNN`), remc2 = `reference/remc2/remc2/engine/EventsFunctions.cpp` (`EF:`, line numbers
in THIS copy of the file, which differ from the `EF:` numbers quoted in port comments by ~+300..+400).

General shape of the subsystem: retail has ONE wizard intake (`sub_46540` MC1 / `sub_5EFA0` MC2), ONE death fall + landing
payout (`sub_45FC0` MC1 / `sub_5E310` MC2), ONE dead-wait dispatcher (`sub_46480` / `sub_5E7C0`, with an AI/human fork inside),
and ONE respawn (`sub_44D30` / `sub_5C950`). The port has a HUMAN arm (engine/world.rs, `Player` struct + `g.player_*` mirrors)
and a RIVAL arm per game (mc1/rivals.rs, mc2/rivals.rs, `Rival`/`Mc2Rival` + entity fields). The per-tick ALIVE housekeeping
(regen, lethal flip) really IS two retail routines per game (human `sub_45C90`/`AddPlayer03_00_5E010` vs AI `sub_132B0`/`sub_12A70`).

---

### LIFE-1: MC1 wizard damage intake (mailbox drain)
- **Retail routine(s):** `sub_46540_46880` (remc1 :55641-55740; HW twin `sub_46880`), called for EVERY class-3 wizard — human and AI. Drains ch4 (duel grip), ch3 (mana steal), ch0 (damage: shield quarter, life, knockback, killer latch).
- **Port arms:**
  - `engine/world.rs:11183 apply_player_damage` (MC1 branch) — HUMAN/MC1
  - `mc1/rivals.rs:2220 rival_damage_intake` — RIVAL/MC1
  - (third partial copy) `engine/world.rs:6374 player_mail_block`, the `if self.invincible` arm — HUMAN/CHEAT (see LIFE-3)
- **Status:** DRIFTED
- **Differences:**
  - **ch3 steal — THIEF CREDIT.** Retail credits the thief `+140 += amt` whenever the source record is class 3 (:55688-89), for any victim. RIVAL arm calls `credit_wizard_mana(steal_src, steal_amt)` (raw add, `MGC_NO_MC1_RAW_STEAL_CREDIT`). HUMAN arm has NO credit on MC1 at all — the credit block is inside `if self.game == GameId::Mc2` only; comment says "a class-3 thief would bank it (mob feeders aren't wizards)". MC1 rivals DO cast Steal Mana (spell 13 is in the rival cast/aim lists, `mc1/rivals.rs:5555/5650/5691`), and the (10,m11) steal flash's area write posts ch3 onto the human with `src = flash.id24` = the rival (`mc1/combat.rs:1553`). So a rival stealing from the human debits the human but the rival never gains the mana.
  - **ch4 duel grip — CASTER STAMP.** Retail writes the CASTER's `+314 = victim, +316 = 200, +318 = clamp(dist3d, 1024, 3072)` for whatever class-3 caster the letter names (:55663-77). RIVAL arm stamps only when the caster resolves to the human (`owner_slot_of_source(grip_src) == Some(0)` → `set_duel_latch`); a rival-cast tether landing on another rival stamps nothing. HUMAN arm stamps nothing ("no rival wizards cast it yet") — but MC1 rivals DO cast Tether (spell 11; `mc1_duel_dart_exact_off` rival dart arm at `mc1/rivals.rs:5877-5930`). See LIFE-12 for why it matters (the death fall reads it).
  - **Steal victim debit representation.** RIVAL: raw `wrapping_sub` on the purse immediately. HUMAN: `debit_mana` defers it into `player.mana_delta` (MC1 branch), landed raw only on the fatal tick. Documented as equivalent (mc1l42 t=349 rationale in `debit_mana` doc) — representational, not a known behaviour split.
  - **Shield flag home.** RIVAL reads the entity flag `flags & 0x4000` and clears it; HUMAN reads the `player.shield` bool (imported from `carpet.flags & 0x4000`, `conformance.rs:804`). Representational.
  - **Knock gate on a dead source.** RIVAL has `no_mc1_knock_from_dead_source` (switch restores a `class64 != 0` guard); HUMAN never had the guard. Same default behaviour.
  - **Knock gate for a self/`PLAYER_TARGET` source.** RIVAL maps `PLAYER_TARGET` to `human_pose`; HUMAN skips the knock when `src == PLAYER_TARGET`. Retail's only gate is `src > 0` (:55711). Probably unreachable (area writers skip the owner), noted for completeness.
  - HUMAN-only presentation writes: `hit_flash`, `player_alert = 4`, `player_danger = 100`, `player_damage` tally; RIVAL-only: none. Retail writes `+392 = 4` / danger for every wizard; for a rival they are unread HUD fields.
- **What reaches each arm in-game:** RIVAL arm: any rival wizard hit by anything, or hit by the human's Steal Mana / Tether. HUMAN arm: the human hit by anything; the drifted legs need a RIVAL casting **Steal Mana** at the human (ch3) or **Tether/Duel** at the human (ch4).
- **Already witnessed?:**
  - RIVAL arm: yes (mc1hwl0 fixture "THE CH3 THIEF BANKS THE FULL STEAL…", mc1l2 "AI LIFE REGEN HAS NO STALL GATE", mc1l2 "A DYING WIZARD SPENDS THE KILLING BLOW'S KNOCKBACK"; human-as-thief credit: mc1l48 "MC1's MANA-STEAL CREDIT HAS NO CEILING", victim slot 712 is a rival).
  - HUMAN arm ch0: yes (mc1hwl0 "A CORPSE IMPORTS AND HOLDS ITS WRAPPED-NEGATIVE PURSE"). HUMAN arm as ch3 victim of a rival thief: no evidence found. HUMAN arm as ch4 victim of a rival tether: no evidence found (the mc1l48 duel fixtures are the HUMAN's own tether on a rival).
- **Recording ask:** MC1 (or HW), a mid-campaign level whose rivals own Steal Mana and Tether (any level where a rival is seen casting the purple steal flash / the duel dart). Hover near an active rival with a healthy purse and let it hit you with Steal Mana several times (watch your mana drop); separately let it land a Tether on you, then kill that rival within ~20 tiles while it is still tethered. 5-10 min. The thief credit shows up in the rival's mana (graded wizard lane); the tether stamp only surfaces in the rival's death fall (LIFE-12).
- **Confidence / notes:** High for the ch3 credit (code read, both arms). The rival-cast steal reaching the human is inferred from the rival spell lists + the shared area writer; I did not find a recording of it.

---

### LIFE-2: MC2 wizard damage intake (mailbox drain)
- **Retail routine(s):** `sub_5EFA0` (remc2 EF:60974-61110, port comments cite EF:60613-60725), ONE routine for every class-3 wizard (human carpet and rivals). Prologue drops a dead target (`word_0x96_150`), then ch4 duel stamp, ch3 → `sub_61050` steal, ch0 damage (two-stage shield, life, knock, carpet LCG + wizard grunt, killer latch with (10,67) suppression, source clear on survive), cheat tail for the human.
- **Port arms:**
  - `engine/world.rs:11183 apply_player_damage` (MC2 branches) — HUMAN/MC2
  - `mc2/rivals.rs:3806 mc2_rival_intake` — RIVAL/MC2
  - `engine/world.rs:6374 player_mail_block` invincible arm — HUMAN/CHEAT (LIFE-3)
- **Status:** DRIFTED
- **Differences:**
  - **Killer latch (10,67) suppression.** Retail: `if (life < 0) { killer = src; if (Entities[src] is class 10 model 67) killer = 0; }` (EF:61094-98). RIVAL arm has it (`hearse` check). HUMAN arm writes `player.killer = src` unconditionally. The latch drives the human corpse's gaze and its z pin (`killer_pos`, `mc2_player_dead_wait`: killer → turn toward it at ≤22/tick, z = ground; no killer → spin +5/tick, z = ground+256). No kill switch.
  - **ch4 duel stamp.** RIVAL arm calls `mc2_duel_stamp(i)` (caster-generic, `engine/world.rs:15074`, stamps human OR rival caster + queues duel XP). HUMAN arm only sets regen stall/danger/alert and clears the source — never stamps the caster. Port comment: AI never picks spell 14 in MC2, so the human-victim leg is unreachable in single player.
  - **ch3 steal side-effects on the victim.** Retail `sub_5EFA0` ch3 is a bare `if (src) sub_61050(a1x)` — NO hit frame / regen stall / `sub_5EF70`. HUMAN arm (shared MC1 code) sets `regen_delay = 16`, `player_danger = 100`, `player_alert = 4` on MC2 too. RIVAL arm writes none of those (and MC2 rival regen has no stall anyway).
  - **ch3 steal victim clamp.** Retail `sub_61050` clamps the victim's purse to [0, maxMana] immediately, discarding any shortfall (EF:62570-80 in this copy). RIVAL arm: `(mana - amt).max(0)` then `.min(mana_max)` — matches. HUMAN arm: `debit_mana` (MC2 branch) saturates the purse at 0 but pushes the shortfall into `mana_delta -= owed`, so an over-steal eats the victim's NEXT regen.
  - **Shield-quarter overdraft / fatal-tick purse.** Retail: charged-shield quarter is a plain signed subtract, purse may go negative and a corpse keeps it (mc2l16 fixture). RIVAL arm: `pay = q` (overdraft allowed, `no_rival_mana_overdraft`). HUMAN arm: `debit_mana` floors at 0 with the shortfall in `mana_delta`; the MC1-only fatal-tick "land raw" fixup is gated `Mc1 | Mc1Hw`, so an MC2 human corpse reads purse 0 (retail negative) and the carried `-owed` survives into the respawned life (`mc2_player_respawn` never resets `mana_delta`).
  - **Attacker word `word_0x26_38`.** RIVAL arm zeroes `f40` every entry and sets it to `src` on ch0. HUMAN arm keeps no equivalent on the pooled carpet record. Ungraded (`debug_hit_words` doc).
  - **Hit grunt pick.** Both advance the victim's LCG once per physical intake (retail EF:61089). RIVAL picks sound `54 + (rand & 3)` (retail); HUMAN picks `54 + (src & 3)` and throws the draw away. Sound is ungraded.
  - **Dead-target prologue.** RIVAL-only by nature (the human has no AI target); includes `no_mc2_intake_drops_dead_human`. Not a twin difference.
  - Shield two-stage absorb, shield XP (`no_mc2_shield_xp_on_absorb`, human-only via the model-0 guard), knock raw bearing (`no_mc2_knock_dir_raw`, both arms), knock from dead source (`no_knock_from_dead_source` rival / no guard human), source-only clear on survive: equivalent in both arms today.
- **What reaches each arm in-game:** RIVAL: any MC2 rival hit (arrows, spells, creatures), the human's Steal Mana on a rival. HUMAN: the human hit by anything. Drift legs: human killed by the (10,67) Earthquake/Gravity-Well effect (whatever spawns (10,67) in MC2 — the quake/flood family); human with a CHARGED shield taking a lethal hit whose quartered amount exceeds his purse; human stolen from (probably only multiplayer/unreachable in SP).
- **Already witnessed?:**
  - RIVAL: yes (mc2l22 "THE SHIELD ABSORB STAGE LIVES ON THE WIZARD'S FLAGS WORD…", mc2l16 "RETAIL'S CHARGED-SHIELD QUARTER IS A PLAIN SIGNED SUBTRACT…", mc2l6 "STEAL MANA BILLS BY TIER…").
  - HUMAN: yes for shields (mc2l6 "AN ARMED SHIELD NULLS THE HIT OUTRIGHT…"). Human killed by a (10,67): no evidence found. Human lethal hit through a charged shield on an empty purse: no evidence found.
- **Recording ask:** MC2, a level with the Earthquake/Gravity-Well-type (10,67) effect near you (a rival or map object that casts it) — let it kill you and watch the corpse: retail should SPIN slowly and hover a tile up; then press Space. Separately (any MC2 level): cast Shield, spend your mana to ~0, then die to a big hit (a rival fireball volley) while the shield is charged; respawn and fly 10 s. ~10 min total.
- **Confidence / notes:** High on the (10,67) and steal-stall differences (read both bodies and retail). The MC2 human-victim steal is likely unreachable in SP (AI pick tables lack 13 and 14); it still matters for MP and for the collapse.

---

### LIFE-3: Invincibility-cheat intake (a third partial copy of the intake)
- **Retail routine(s):** MC2 `sub_5EFA0`'s cheat tail (EF:61103-09): the FULL intake runs, THEN `if (cheat & model == 0) { word_0x26 = 0; killer = 0; src = 0; life = 10000 }`. MC1 has no such tail in `sub_46540`.
- **Port arms:** `engine/world.rs:6374 player_mail_block` `if self.invincible` arm — HUMAN/CHEAT (both games) vs the normal arm `apply_player_damage`.
- **Status:** STRUCTURALLY-DIFFERENT
- **Differences:** the cheat arm re-implements ch3 (MC2 steal resolve + credit, but `mana.saturating_sub` instead of `debit_mana`, and no alert) and ch0 (damage tally uses `amt/4` when shielded but NEVER consumes the shield stage, never awards shield XP, never advances the MC2 carpet LCG, knock magnitude from the RAW amount not the quartered one), sets `player_danger` if ANY source is set, then wipes all six channels. Retail MC2 runs the whole intake first (shield promotion/consumption, XP, LCG, knock off the quartered amount) and only resets life/killer/source.
- **What reaches each arm in-game:** the invincibility cheat toggled on and the human taking hits.
- **Already witnessed?:** no evidence found.
- **Recording ask:** MC2 with the retail invincibility cheat on: take hits with and without a Shield running, ~2 min. Low priority (cheat).
- **Confidence / notes:** medium; cheats are player-ruled overlays in parts of this port — check the ruling before collapsing.

---

### LIFE-4: Lethal transition (alive → death fall)
- **Retail routine(s):** HUMAN: `sub_45C90` tail :55424-29 (MC1) / `AddPlayer03_00_5E010` EF:60035-41 (MC2): `+70 = 2`, fall speed `+46`/`word_0x2C_44 = 0`, sound 16. AI: `sub_132B0` :17980-83 (MC1) / `sub_12A70` lethal leg (MC2): `+70 = 2` ONLY, returns from housekeeping, brain still runs. Retail has two distinct routines (human vs AI) per game — this is a fact, not a collapse candidate across human/rival; the two RIVAL arms are cross-game twins.
- **Port arms:** `engine/world.rs:11379-11418` (fatal branch in `apply_player_damage`) + `engine/world.rs:6575 player_regen_block` death arm (mail-less corpse) — HUMAN/both games; `mc1/rivals.rs:1930 rival_alive_tick` lethal branch — RIVAL/MC1; `mc2/rivals.rs:3381 mc2_rival_alive` lethal branch — RIVAL/MC2.
- **Status:** IDENTICAL (rival arms vs each other; human arms vs each other)
- **Differences:** rival arms: both keep `f46` (MC2 behind `no_rival_fall_carry`, MC1 unswitched), both run the brain tail (`rival_dispatch_tail` / `mc2_rival_brain_tail` behind `no_rival_death_brain`), both emit sound 16 (MC1 comment flags it as possibly mis-owned). MC1 rival additionally has `death_keeps_rebound` (no rebound-bit clear); MC2 rival never clears it either. Human arm: identical code both games.
- **What reaches each arm in-game:** any wizard death.
- **Already witnessed?:** RIVAL/MC1 yes (mc1hwl0 +46 carry cited in code; mc1l49 t=6669 rebound); RIVAL/MC2 yes (mc2l6-rsg carried velocity, rival 378); HUMAN yes (mc1l0 "A MAIL-LESS CORPSE FLIPS…", mc2l0 "THE MC2 DEATH PAYOUT RUNS A TICK LATE").
- **Recording ask:** none.
- **Confidence / notes:** high.

---

### LIFE-5: Death fall (state 2) — fall integration, trail puff, tether tail
- **Retail routine(s):** MC1 `sub_45FC0` (:55434-55484, shared human/AI): full shared mover `sub_455D0` (speed-0 sink, strafe, knock, duel-lock tail, flutter, wall gate) then gravity, floor, (10,1) trail at the global scratch. MC2 `sub_5E310` (EF:60402-60440): `sub_5D530` mover (incl. `sub_5DE30` duel enforcement and `sub_5DD50` stuck nudge), gravity, floor, (10,1) puff at `predictedAxis`.
- **Port arms:**
  - `engine/world.rs:7147 step_player_flight` (flight::`mc1_move_duel` + `death_fall_step`) + `engine/world.rs:11435 mc1_mortality_pass` Falling arm — HUMAN/MC1
  - `mc1/rivals.rs:6016 rival_death_fall` (inline re-implementation of `sub_455D0`) — RIVAL/MC1
  - `engine/world.rs step_player_flight_mc2` (flight::`mc2_move`) + `engine/world.rs:12209 mc2_player_fall` — HUMAN/MC2
  - `mc2/rivals.rs:10074 mc2_rival_death_fall` (+ `mc2_rival_carpet_move`) — RIVAL/MC2
  - (pinned-pair variants inside the human arms: `fall_pre_z`/`mc1_fall_entry_z` reconstruction) — PINNED-POSE
- **Status:** DRIFTED
- **Differences:**
  - **Duel-lock tail during the fall.** MC2 RIVAL: the lock register is honoured in the fall (`no_mc2_rival_duel_death_tether`, pull + heading servo + drain). MC1 RIVAL: `rival_death_fall` has NO lock tail at all and there is no rival lock register (see LIFE-12). MC1 HUMAN: yes (the real mover). MC2 HUMAN: yes (the real mover).
  - **Wall gate.** MC1 RIVAL behind `rival_fall_wall_gate` (`MGC_NO_MC1_RIVAL_FALL_WALL_GATE`); MC1 HUMAN always gated (`player_wall_gate_fixed` in the mover).
  - **Speed-0 sink.** MC1 RIVAL behind `death_sink_runs` (`MGC_NO_DEATH_SINK`), reads the rival's row `v_10`; MC1 HUMAN hard-coded in `flight.rs` at the carpet row's 1024.
  - **Strafe/jink, knock decay, flutter** — MC1 RIVAL inline copies (jink constant, knock `min(128)` −4 decay, flutter every 64th `f63`); MC1 HUMAN via `flight.rs`. Landed separately (mc1l3 fixtures).
  - **Stuck nudge (MC2).** RIVAL behind `no_mc2_rival_stuck_nudge` in `mc2_rival_carpet_move`; HUMAN in `flight::mc2_move`.
  - **Puff seat (MC2).** Both arms read `g.mc2_pred_axis` under `no_mc2_fall_puff_pred_axis`; the HUMAN arm publishes it only `if mover_ran`, the RIVAL arm relies on `move_relink` having published it.
  - **Floor.** MC1 RIVAL `ground + row.v_12`; MC1 HUMAN `ground + 128` hard-coded; MC2 both `ground + row clearance`.
  - **Knock magnitude at fall entry** is armed by the fatal letter in both games (LIFE-1/2).
- **What reaches each arm in-game:** a wizard (human or rival) dying in the air. Drift legs: a rival that had tethered someone dying while its victim is within ~20 tiles (MC1); a rival corpse drifting into a wall tile (MC1); a corpse falling at zero speed from above its band (MC1).
- **Already witnessed?:** MC1 RIVAL yes (mc1l12 "A RIVAL CORPSE RUNS THE SHARED MOVER'S WALL GATE", mc1l3 "THE CORPSE KEEPS ITS JINK", mc1l3 "THE MOVER'S FLUTTER DRAW REACHES THE CORPSE", mc1l2 knockback; mc1l49 sink cited in code). MC1 HUMAN yes (mc1l42 "THE DEATH FALL TRAILS FIRE ONE GRAVITY STEP HIGH", mc1l32 "THE TOUCHDOWN BREAKS SETTLED-MINUS-STEP"). MC2 HUMAN yes (mc2l16 "THE MC2 WIZARD DEATH-FALL (10,1) PUFF…"). MC2 RIVAL yes (mc2l16 "…128-UNIT STUCK NUDGE…"). MC2 RIVAL duel tail: no (structurally unreachable in SP, per code). MC1 RIVAL duel tail: no evidence found.
- **Recording ask:** MC1: let a rival Tether YOU, then kill that rival while staying within ~20 tiles; watch whether its corpse is dragged toward you as it falls. ~5 min. (This is the only unwitnessed leg with a real behaviour split.)
- **Confidence / notes:** medium-high. The MC1 rival lock-tail gap depends on retail's `sub_455D0` reading `+314` for the AI's own wizext during `sub_45FC0` — the shared-mover citation in `rival_death_fall` and the MC2 twin's disassembly say it does. Heavy overlap with the FLIGHT subsystem (the mover itself).

---

### LIFE-6: Death landing payout — scatter, grave, victim pop, state flip
- **Retail routine(s):** MC1 `sub_45FC0` touchdown block (:55485-55570, shared): `sub_37220` rebuild, kill credit, notification, mailbox memset, 24-entry acquisition-list scatter (+ `var_916` bank write, which OVERFLOWS for model ≥ 30), grave (10,40) + ball re-point **and `+70 = 3` + respawn timer INSIDE `if (grave)`**, hide bit, `--recycle_top`. MC2 `sub_5E310` touchdown (EF:60441-60537): `sub_49F90`, kill credit, "has died.", memset, 26-token scatter (`actionIndex++`), grave + **`actionIndex = 3`, `dword_0x10 = 1200` INSIDE `if (grave)`**, hide, `--dword_0x11e6`.
- **Port arms:** `engine/world.rs:11534 player_land` — HUMAN/MC1; `mc1/rivals.rs:6174 rival_death_impact` — RIVAL/MC1; `engine/world.rs:12326 mc2_player_land` (+ `mc2/cast.rs:5903 mc2_scatter_spells`) — HUMAN/MC2; `mc2/rivals.rs:10123 mc2_rival_death_impact` — RIVAL/MC2.
- **Status:** DRIFTED
- **Differences:**
  - **Kill credit** — see LIFE-7 (split out).
  - **State flip on a full pool (grave spawn fails).** MC2 RIVAL: `tick70 = 3` / `f26 = 1200` only inside the grave success arm (retail). MC2 HUMAN: `state = Dead` and `mc2_respawn_timer = 1200` unconditionally. MC1 HUMAN: `state = Dead` unconditionally, documented as a deliberate benign deviation. MC1 RIVAL: `tick70 = 3` + timer unconditionally, undocumented.
  - **Roster overflow of the `var_916` bank (MC1).** RIVAL: a stale acquisition entry naming a model-39 record writes the NEXT player's husk-watch byte (`husk_watch_gate`, `MGC_NO_HUSK_WATCH`; mc1l49 t=39094). HUMAN: `death_owned_blue[s]` only for `s < SPELL_COUNT`; a human list entry naming a mana ball brands nobody. Retail's write is shared (`*(a1+160) + model + 916`), so a stale entry in the HUMAN's list would brand player 1 (the first rival) as human-driven.
  - **`var_916` bank for in-range models (MC1).** HUMAN banks `flags & BLUE_SPELL (0x40000)` per spell and the respawn restores it (LIFE-9). RIVAL keeps no bank for m < 30 (comment: "a register nothing else reads" — but the respawn reads it, :54908).
  - **Scatter state write (MC2).** RIVAL: `tick70 = tick70 + 1` (retail `actionIndex++`). HUMAN: `tick70 = spell*3 + 1` (assignment). Differ only if a token is not sitting at `3*spell` when its owner lands (e.g. mid-window).
  - **Scatter book-entry write for an empty slot (MC2).** RIVAL writes `book.ent[s] = 0`; HUMAN `continue`s. Same value.
  - Victim-list pop: both games both arms (MC2 HUMAN `human_payout_victim_pop_off`, MC2 RIVAL `no_death_payout_victims`, MC1 both unswitched). The victim-half REBUILD: `mc2_rebuild_free` (`engine/features.rs:6142`) already rebuilds it (behind `no_mc2_rebuild_victim_half`); MC2 RIVAL then calls `rebuild_recycle(0x2_0000)` a SECOND time behind `no_death_payout_victims` (idempotent, but the two arms answer to different kill switches — turning off `no_death_payout_victims` does not remove the victim half on the rival arm, and `human_payout_victim_pop_off` only gates the human's pop).
  - Grave seat: all four use the corpse's own z now (MC2 human comment claiming the rival passes `ground_z` is stale).
  - HUMAN-only: `player_death_clear_effects`, `teleport_return = None` (MC1), invisibility both homes (MC2). RIVAL-only: `rival_deaths` push + named toast.
  - Owned-register clear: MC1 HUMAN clears `owned[s]` unless `strict && owned_survives_scatter`; MC1 RIVAL clears only if `!owned_survives_scatter()` — same in strict mode, differs in NATIVE play (human native keeps the clear on purpose).
- **What reaches each arm in-game:** any wizard corpse touching down. Drift legs: a death while the entity pool is FULL (late MC1/HW levels with seizures — mc1hwl0, mc1l49 run dry); the human dying with a stale acquisition entry (after one of his spell tokens was recycled — long MC1 takes with many deaths, e.g. mc1l49); an MC2 death mid-spell-window.
- **Already witnessed?:** MC1 RIVAL yes (mc1l4 "THE JAR SCATTER WALKS THE +532 ACQUISITION LIST…", mc1l49 husk overflow cited in code). MC1 HUMAN yes (mc1l42 grave/scatter cited in code, mc1l42 "A CORPSE IS MANA DEAF…"). MC2 HUMAN yes (mc2l0 "THE MC2 DEATH PAYOUT RUNS A TICK LATE", mc2l22 t=10021 pop cited in code). MC2 RIVAL yes (mc2l6-rsg / mc2l22 cited in code). Full-pool landing: no evidence found for any arm. Human roster overflow: no evidence found.
- **Recording ask:** (a) MC1/HW late level with a saturated pool (lots of fire/creatures, e.g. where mc1hwl0 seized): die (and get a rival killed) while the pool is dry — look for the corpse staying in its fall/"no grave". Hard to force; a long chaotic take. (b) MC2: die while a long spell window (Rebound/Shield) is open. 5 min.
- **Confidence / notes:** high on the grave-gating split and the tick70 assignment; the full-pool legs are hard to stage deliberately.

---

### LIFE-7: Kill credit on a wizard death (per-victim tally + creature-kill counter)
- **Retail routine(s):** inside the landing payouts above: MC1 :55488-97 `if (killer && pool[killer].+64 == 3 && pool[killer].+65 <= 1) ++killer.wizext.+30[victim.+48]`; MC2 EF:60445-58 the same test (class 3, model 0/1) on `word_0x24_36`. Neither touches the human's creature counter (`+359` MC1).
- **Port arms:** `mc1/rivals.rs:6194-6206` (in `rival_death_impact`) — RIVAL-VICTIM/MC1; `engine/world.rs:11534 player_land` — HUMAN-VICTIM/MC1 (no credit code at all); `mc2/rivals.rs:10154-10166` — RIVAL-VICTIM/MC2; `engine/world.rs:12331-12343` — HUMAN-VICTIM/MC2.
- **Status:** DRIFTED
- **Differences:**
  - HUMAN-VICTIM/MC1: no kill credit — a rival that kills the human never gets its `+30[0]` tally.
  - RIVAL-VICTIM arms (both games): credit through `owner_slot_of_source(killer)`, which resolves a projectile/creature/castle killer to its OWNER's slot. Retail credits only when the killer record itself is a class-3 model-0/1 carpet. So a rival killed by a rival-summoned creature, a castle (3,2), or any source whose `+24` names a wizard gets credited in the port and not in retail.
  - HUMAN-VICTIM/MC2: literal retail test (class 3, model 0/1) — the only arm that matches retail's predicate.
  - RIVAL-VICTIM/MC2 additionally suppresses a (10,67) killer (redundant with the intake latch) and bumps `g.kills` when the human is the killer; RIVAL-VICTIM/MC1 bumps `g.kills` only under `MGC_NO_MC1_RIVAL_KILL_NO_TALLY` (round 154 law: retail never feeds `+359` from a wizard kill; mc1l49 t=9641). The MC1 law never crossed to MC2.
- **What reaches each arm in-game:** a rival killing the human (MC1); a rival dying to a creature/castle/owned effect whose owner is another wizard (both games); the human killing a rival (MC2 `kills` counter).
- **Already witnessed?:** RIVAL-VICTIM/MC1 `g.kills` law yes (mc1l49 t=9641, cited in `no_mc1_rival_kill_no_tally`). Per-victim tally: no fixture found in any arm (likely an ungraded lane — the stats-screen memo says the new counters are on graded paths; verify). MC2 `kills` is not imported by `retail_import_mc2` (only the MC1 importer seats `g.kills`), so the MC2 leg is ungraded.
- **Recording ask:** MC1: get killed by a rival (any), then open the retail stats screen at level end — note each wizard's kill column. MC2: kill a rival, check the retail stats screen's creature-kill figure before/after. Also useful: let a rival die to a rival's creature/castle. Probably needs a stats-screen or player-block lane to grade; otherwise UNGRADED.
- **Confidence / notes:** high on the code; grading status of the tally lane unverified.

---

### LIFE-8: Dead-wait (state 3) — knock wipe, gaze, z pin, countdown/elimination
- **Retail routine(s):** MC1 `sub_46480` (:55594-55625): first statement wipes knock magnitude `+22` for every wizard; then AI/human fork on the roster byte `13332_9`: AI → castle test / countdown / respawn / eliminate; human-driven → `sub_463B0` gaze at the killer. MC2 `sub_5E7C0` (EF:60254-305): first statement wipes `moveBoost`; AI arm counts 1200 / banishes; human arm `sub_5E6C0` gaze + z pin.
- **Port arms:** `engine/world.rs:11435 mc1_mortality_pass` Dead arm + gaze in `step_player_flight` (`engine/world.rs:7203-7231`) — HUMAN/MC1; `mc1/rivals.rs:1896 rival_entity_tick` (knock wipe) + `rival_dead_wait` + `rival_watch_track` (`sub_463B0` for a human-driven husk) — RIVAL/MC1; `step_player_flight_mc2` Dead branch (`engine/world.rs:~6752-6776`) + `engine/world.rs:12453 mc2_player_dead_wait` (pinned path) — HUMAN/MC2; `mc2/rivals.rs:10276 mc2_rival_dead_wait` — RIVAL/MC2.
- **Status:** DRIFTED (mostly switch-name / seat drift)
- **Differences:**
  - Knock-magnitude wipe: MC1 human + MC1 rival share `no_mc1_dead_wait_knock_clear`; MC2 rival `no_mc2_dead_wait_knock_clear`; MC2 human `no_mc2_knock_dir_keep` (different switch; its OFF arm wipes the bearing too). MC2 HUMAN's wipe lives only in the flight driver — the pinned-pair `mc2_player_dead_wait` does not wipe.
  - `sub_463B0` gaze twins: HUMAN/MC1 (driver) reads `ent[min(killer, len-1)]` raw and turns ≤22; RIVAL/MC1 husk `rival_watch_track` resolves `PLAYER_TARGET` to `human_pose`, returns (no turn) on an out-of-range killer, and also writes `f34`/`f36`/`f32 = 0`. HUMAN does not publish `+34`/`+36` onto the pooled carpet.
  - Killer-latch lifetime feeds the gaze (see LIFE-10).
  - AI arms: MC1 eliminates on an empty castle REGISTER; MC2 on `rival_castle()` none. Different predicates per game (fine per retail?) — not verified here.
- **What reaches each arm in-game:** any corpse on the ground; the husk-watch arm needs a rival row branded human-driven (LIFE-6 overflow).
- **Already witnessed?:** MC1 human yes (mc1l14 t=1397 cited in code; mc1l0-pd gaze cited in code). MC1 rival yes (mc1l49 husk watch cited). MC2 human yes (mc2l0 t=11193, mc2l24 "THE SELF-KILL WITNESS MUST SURVIVE A STICKY KILLER LATCH"). MC2 rival yes (knock wipe cited at EF:60660; mc2l6-rsg deaths).
- **Recording ask:** none beyond LIFE-10.
- **Confidence / notes:** medium; the switch-name drift is the main collapse hazard.

---

### LIFE-9: MC1 respawn (`sub_44D30`)
- **Retail routine(s):** `sub_44D30` (:54838-55060), ONE routine for human (Space, case 0xF) and AI (dead-wait countdown): rebuild, seat at castle (whole axis) else authored start +256, `u16_331 = 100`, `u32_351 = 2000`, zero `v_12/v_24/v_26/v_22/v_16`, 24-entry re-mint with `var_916` restore (`+86 = 280; +132 = 0; byte[2] |= 4`), `maxLife = 10000`, `+136 = +140 = 1000`, `sub_45C10`, `sub_47DD0` reprice, truce roster, **`u16_314 = 0; u16_316 = 0` (duel lock)**, AI-only hate/war/cooldown block, `u32_396/400 = 2048`, `v_46 = 0`, `memset(u8_333, 16, 8)`, `recycle_top = -1`. It never writes `+38` (killer) or `+383`/`+341`.
- **Port arms:** `engine/world.rs:11813 player_respawn` (+ `death_regrant`) — HUMAN/MC1; `mc1/rivals.rs:6543 rival_respawn` — RIVAL/MC1.
- **Status:** DRIFTED
- **Differences:**
  - **Duel lock clear.** Retail clears `+314/+316` for everyone. HUMAN never clears `self.duel` on respawn (only the mover's release tests do); RIVAL has no lock register.
  - **Killer latch.** HUMAN clears `player.killer = 0` on MC1 (the MC2 "sticky killer" law explicitly excludes MC1: `if !matches!(game, Mc2) || mc2_suicide_stale_killer_off()`); the MC1 Shift+K arm also clears it (`engine/world.rs:10455`) though retail's K handler is a bare `+12 = -1` (:20491-92). RIVAL never clears `f38`. Retail writes neither.
  - **`var_916` burst/blue restore.** HUMAN restores `flags |= BLUE_SPELL; type86 = 280` (no `+132 = 0`). RIVAL re-mints with no restore.
  - **Regen stall.** HUMAN keeps `regen_delay`/`life_rate` (`no_mc1_respawn_keeps_regen`, i.e. `+383`/`+341` untouched). RIVAL sets `regen_stall = 16`, citing `memset(u8_333, 16, 8)` — but the rival intake uses the same field as `+383`. Two arms map `regen_stall` to two different retail fields; harmless while the rival's `+383` sits at 16 forever after its first hit.
  - **Life ceiling.** HUMAN `life = PLAYER_LIFE_MAX` (10000). RIVAL `refill_life(i)` = the entity's existing `max_life`. Retail resets `maxLife = 10000` for everyone (:55023-27). If any MC1 rival carries a non-10000 `max_life` (per-level scaling, or retail's special `maxLife = 1000000` arm at :55017-20, keyed on `var_u32_13347_24 == 0xAE89E`, meaning unknown) the rival arm refills to the old ceiling and never writes `max_life` — the MC2 twin had exactly this bug and fixed it (`mc2_rival_respawn` writes `max_life = 10000`). Not verified whether MC1 rivals are ever spawned below 10000.
  - **Castle reprice.** HUMAN `no_mc1_human_respawn_reprice` + fallback gated on `player_castle_bound()` + `flags & 2`; RIVAL `no_mc1_respawn_reprice_owner` + fallback on the register. Default path identical (`mc1_respawn_reprice`).
  - **Truce.** Both `mc1_truce_roster` (`no_mc1_truce_roster`); HUMAN passes `(0, u16::MAX)`, RIVAL `(slot, own)`.
  - HUMAN-only: sound 14, `player_death_clear_effects`, `lost/won` castle-less arm, `no_spell_loss` native arm. RIVAL-only: `rival_owned_rebuild`, AI state reset, hate/war/poverty/cooldown[16], `vdes/jink/knock` clears (human flight registers are the app's).
- **What reaches each arm in-game:** HUMAN: Space after dying with a castle. RIVAL: a dead rival with a castle counting down. Drift legs: the human dies while HIS tether is gripping a rival, respawns near that rival; the human suicides (Shift+K) after an earlier death by a killer; a rival dies holding a blue/bursting token.
- **Already witnessed?:** HUMAN yes (mc1l42 t=17398, mc1l49 "A BANKED 0 IS FIREBALL…", mc1l0 "THE CASTLE-LESS RESPAWN…"). RIVAL yes (mc1hwl0 "THE MC1 RESPAWN WINDOW SACRIFICES…", mc1l35 "A STALE REGISTER STILL RESPAWNS THE RIVAL…"). Duel-lock-across-respawn, stale killer after suicide, blue restore on a rival: no evidence found.
- **Recording ask:** MC1: (1) get killed by a rival, respawn, then Shift+K suicide — watch which way the corpse turns (retail should look at the OLD killer). 2 min. (2) Tether a rival, get yourself killed while it is gripped, respawn at a castle within ~20 tiles of that rival — does the carpet get pulled? 5 min.
- **Confidence / notes:** high on the lock/killer code facts; the rival max-life leg needs a check of MC1 rival spawn life.

---

### LIFE-10: Killer latch lifetime (`+38` / `word_0x24_36`)
- **Retail routine(s):** written ONLY by the lethal branch of the intake (`sub_46540` :55729 / `sub_5EFA0` EF:61094-98, plus MC2's cheat tail). Never cleared by suicide, landing, or respawn in either game.
- **Port arms:** `player.killer` (HUMAN, both games: set `engine/world.rs:11388`; cleared at `:8525` MC2 suicide under switch, `:10455` MC1 suicide unconditionally, `:11928` MC1 respawn unconditionally / MC2 under switch, `:12542` MC2 respawn under switch); `ent.f38` (RIVAL/MC1 `mc1/rivals.rs:2321`, RIVAL/MC2 `mc2/rivals.rs` intake) — never cleared.
- **Status:** DRIFTED
- **Differences:** MC2 HUMAN carries the "sticky latch" law (`mc2_suicide_stale_killer_off`, 3 sites); MC1 HUMAN clears it at Shift+K and at respawn with no switch; both RIVAL arms never clear (retail). MC2 HUMAN also lacks the (10,67) suppression (LIFE-2).
- **What reaches each arm in-game:** readers are the corpse gaze (MC1 human driver, MC1 husk watch, MC2 human z pin/spin) and the kill credit (LIFE-7).
- **Already witnessed?:** MC2 HUMAN yes (mc2l24 "THE SELF-KILL WITNESS MUST SURVIVE A STICKY KILLER LATCH"). MC1 HUMAN: the mc1l0-pd suicide gaze witness is a FIRST death (latch 0 either way) — the stale-latch case: no evidence found.
- **Recording ask:** same as LIFE-9 (1): MC1, die to a rival, respawn, Shift+K, watch the corpse's facing for 3-4 s before pressing Space.
- **Confidence / notes:** high (retail K handler read at :20491-92).

---

### LIFE-11: MC2 respawn (`sub_5C950` reuse arm) + book re-mint (`sub_5CF40`)
- **Retail routine(s):** `sub_5C950` (EF:43630-43866), one routine for human (Space) and AI (1200 countdown): rebuild, seat at castle axis, `word_0x24A = 256`, `maxMana = 1000`, `maxLife = 10000`, zero-writes list (speed, knock yaw/boost, strafe, `str_0x1AC` memset, …), **`word_0x146/0x148 = 0` (duel lock)**, re-mint via `sub_5CF40` (every non-zero `SpellEnabled[i]` → fresh class-15, `parentId = wizard`, **`byte[0] |= 1`**, NULL → 0), life/mana refill, truce roster (all colours), AI-only hate/cooldown block, `dword_0x11e6 = -1`. Never writes `word_0x2C_44` (fall speed), `word_0x24_36`, `manaRegen`.
- **Port arms:** `engine/world.rs:12495 mc2_player_respawn` (+ `mc2_remint_book` `:12664`) — HUMAN/MC2; `mc2/rivals.rs:10350 mc2_rival_respawn` (inline re-mint) — RIVAL/MC2.
- **Status:** DRIFTED
- **Differences:**
  - **Duel lock clear.** RIVAL clears `duel = DuelLock(None)`; HUMAN never clears `mc2_duel` (it usually dies anyway when the enforcement sees book slot 14 no longer charged).
  - **Fall speed.** HUMAN writes `player.fall_speed = 0`; RIVAL keeps `f46` (the rival comment says retail's respawn writes no `@0x2C`). The human's lethal transition zeroes it anyway, so only the respawned life's value of `@0x2C` differs (it is the graded/imported `carpet.f2c`?).
  - **Re-mint in-book bit.** HUMAN sets `flags |= 1` (retail `byte[0] |= 1`); RIVAL re-mint (and the level-start `mc2_mint_rival_manifestation`) never sets bit 0. Also different SetSpell (`mc2_set_spell` vs `mc2_rival_set_spell`) and RIVAL-only `tick70 = 3*s`, `f26 = 0`, `f54` steal lock, marker switch `no_mc2_rival_remint_marker`.
  - **`mana_delta` carry.** Neither resets it (retail keeps `manaRegen`), but the HUMAN's may hold the `-owed` shortfall from LIFE-2.
  - **Life scale.** RIVAL `life_scale = 256`, `max_life = 10000`; HUMAN uses the constant. Equivalent.
  - **Truce.** HUMAN `mc2_respawn_truce(0)` (`no_mc2_human_respawn_truce`); RIVAL `mc2_truce_roster(slot, own)` (`no_mc2_rival_truce_roster`). Same roster routine, different switches.
  - HUMAN-only: wanted timer hold (`no_mc2_wanted_respawn_hold`), invisibility lift, stall latch, castle-less lost arm, `no_spell_loss`. RIVAL-only: brain/cooldown resets.
- **What reaches each arm in-game:** HUMAN: Space after an MC2 death. RIVAL: a dead rival with a castle after 1200 ticks.
- **Already witnessed?:** HUMAN yes (mc2l3 t=15315, mc2l16 t=7934 truce, mc2l19 wanted hold — cited in code). RIVAL yes (mc2l6 "A RESPAWNED MC2 RIVAL COMES BACK RAW…"). The in-book bit on a respawned rival book: covered in principle by mc2l6-rsg's 72 re-minted class-15s — if `flags` is graded on those tokens and the take is clean there, the bit is being set somewhere I did not find (verify before acting).
- **Recording ask:** none new beyond LIFE-2's "die with a charged shield on an empty purse, respawn".
- **Confidence / notes:** medium on the bit-0 claim (grep found no `flags |= 1` on the rival mint path; possible a later tick sets it); high on the duel/fall-speed facts.

---

### LIFE-12: Duel/tether lock register (stamp → use → clear)
- **Retail routine(s):** stamped by the VICTIM's intake (`sub_46540` :55663-77 / `sub_5EFA0` EF:61003-17) onto the CASTER; consumed only inside the shared mover (`sub_455D0` lock tail :55226-48 MC1; `sub_5DE30` MC2) — which an AI runs only during its DEATH FALL; cleared by the mover's release and by the respawn (both games, all wizards).
- **Port arms:** MC1 human-caster: `set_duel_latch` (`engine/world.rs:14934`) from `rival_damage_intake`, used in `step_player_flight`. MC1 rival-caster: NONE. MC2 human-caster: `mc2_duel_stamp` → `mc2_duel`, used in `mc2_duel_enforce`. MC2 rival-caster: `mc2_duel_stamp` → `Mc2Rival::duel`, used in the rival death fall (`no_mc2_rival_duel_death_tether`). Drain: `mc2/rivals.rs:4819 mc2_duel_drain` (one fn, human and rival victim arms — IDENTICAL arithmetic).
- **Status:** DRIFTED
- **Differences:** MC2 is complete for both casters (stamp at victim, death-fall tail, respawn clear on the rival). MC1 has only the human-caster/rival-victim leg: a rival-cast tether on the human (human intake) or on another rival (rival intake) stamps nothing, the MC1 rival death fall has no lock tail, and the MC1 human respawn does not clear the human's lock. MC2 human respawn does not clear `mc2_duel` either.
- **What reaches each arm in-game:** MC1: rivals cast Tether (spell 11) in native play → reachable. MC2: AI never picks spell 14 → rival-caster legs reachable only via import/MP.
- **Already witnessed?:** MC1 human-caster: yes (mc1l48 duel fixtures, t=59113-59119). MC2 human-caster: yes (mc2l6 "DUEL FIRES A (9,7) DART…", mc2l6-rsg t=1375/2165 cited). MC1 rival-caster: no evidence found. MC2 rival-caster: structurally none in SP.
- **Recording ask:** MC1, as in LIFE-5: let a rival Tether you, stay within ~20 tiles, kill it; also try tethering a rival yourself, dying, and respawning near it. 5-10 min.
- **Confidence / notes:** medium; the MC1 rival-caster consequence (corpse pulled during its fall) rests on the shared-mover citation.

---

### LIFE-13: Alive regen + life/mana floors (per-tick wizard housekeeping)
- **Retail routine(s):** HUMAN `sub_45C90` :55381-55421 (MC1) / `AddPlayer03_00_5E010` EF:59996-60033 (MC2): stall-gated life regen (`+383`), rates maxLife/250 home / /2000 afield, mana step + clamp, all inside `actLife >= 0`. AI `sub_132B0` :17990-18021 (MC1) / `sub_12A70` EF:5424-5459 (MC2): NO stall, rates /200 and /500, life clamp `[-1, max]`. Retail has distinct human and AI routines; the two AI routines are cross-game twins, as are the two human ones.
- **Port arms:** `engine/world.rs:6575 player_regen_block` + `:6558 mc2_mana_tail` (+ MC1 mana step elsewhere) — HUMAN/both; `mc1/rivals.rs:2079-2123` in `rival_alive_tick` — RIVAL/MC1; `mc2/rivals.rs:3606-3668` in `mc2_rival_alive` — RIVAL/MC2.
- **Status:** DRIFTED (rival twins), IDENTICAL (human twins)
- **Differences (RIVAL/MC1 vs RIVAL/MC2):**
  - Life floor −1: MC1 unswitched `clamp(-1, max)`; MC2 behind `no_mc2_rival_life_floor` (landed later, 5462778).
  - Purse home: MC2 re-seeds `Mc2Rival::mana` from the entity `f140` every tick (`no_mc2_wiz_purse_is_entity`, entity is master); MC1 pulls `f140` DOWNWARD only (`mirrored < mana`) and republishes at the tail (`no_mc1_rival_mana_wrap_publish` for the wrap) — an entity-side CREDIT written by another handler would be lost on MC1.
  - At-castle predicate: MC1 via the castle REGISTER (`rival_castle_register`); MC2 via `rival_castle()` + summed overlap.
  - Mana step clamp and rate recompute identical.
- **What reaches each arm in-game:** every live tick of every wizard.
- **Already witnessed?:** RIVAL/MC1 yes (mc1l2 "AI LIFE REGEN HAS NO STALL GATE"); RIVAL/MC2 yes (mc2l6 "THE RIVAL'S LIFE REGEN IS A STORED REGISTER…"); HUMAN yes (many; mc2l3 "THE MC2 MANA TAIL IS PART OF THE CARPET'S DISPATCH").
- **Recording ask:** none needed for the known split; an MC1 rival whose purse is credited from outside its own tick (e.g. it steals from someone) exercises the downward-only mirror — covered by LIFE-1's ask.
- **Confidence / notes:** medium.

---

### LIFE-14: Cast mana debit + mid-burst regen pin (`sub_55E80` MC1 / `sub_68DE0` MC2)
- **Retail routine(s):** `sub_55E80` (:64936-56) and `sub_68DE0` — one debit routine each: first-tick arm "overwrite `+132` negative or deepen", else-arm "pin a positive `+132` to 0"; called from each token body inside its afford gate.
- **Port arms:** `engine/world.rs:15539 mana_debit` + `:15557 suppress_regen` (+ `debit_mana` `:11171` for mail debits) — HUMAN/both; **seven inline copies** in mc1/rivals.rs (`:2873, :2932, :2986, :3033, :3106, :3210, :3371` in the rival token ticks) — RIVAL/MC1; **five inline copies** in mc2/rivals.rs (`:4937, :5035, :8472, :8756, :9052` + pins at `:5510, :8466, :8749, :8786`) — RIVAL/MC2.
- **Status:** DRIFTED
- **Differences:** RIVAL/MC2 pins are gated by `spell_pins_regen` (`NO_MID_BURST_REGEN_PIN` list, `MGC_NO_RIVAL_REGEN_PIN_LIST`) and by the afford gate (`MGC_NO_RIVAL_PIN_AFFORD`); the HUMAN/MC2 column reads the same list in `mc2/cast.rs`. HUMAN has a `dev_spells` exemption. The MC1 rival copies pin on every `f26 != count` tick with no per-spell list (the MC1 "earthquake never blocks regen" law, mc1l0, lives in the human's token machine — not checked whether each rival copy honours it).
- **What reaches each arm in-game:** any spell cast by any wizard.
- **Already witnessed?:** heavily (mc2l22 × 4 regen-pin fixtures, mc1l0 earthquake, mc1l49 token gate).
- **Recording ask:** none from LIFE; hand to the TOKENS/CAST census.
- **Confidence / notes:** low detail on purpose — this cluster belongs mostly to the manifestation-token subsystem (listed at the end).

---

### LIFE-15: MC2 XP awards triggered by the intake (shield / steal / duel)
- **Retail routine(s):** `sub_6D8B0` called synchronously from `sub_5EFA0` (shield absorb, duel stamp) and `sub_61050` (steal); model-0 guard makes it human-only.
- **Port arms:** direct `mc2_award_xp(PLAYER_TARGET, 6, 1)` in `apply_player_damage` (shield) — HUMAN-VICTIM; queued `g.mc2_cast_xp.push((…,13,1))` in `mc2_rival_intake` (human steals from rival) and `(…,14,1)` in `mc2_duel_stamp` (duel), drained at `engine/world.rs:10523` — RIVAL-VICTIM.
- **Status:** STRUCTURALLY-DIFFERENT (same award, two transports)
- **Differences:** direct call vs same-tick mail; the mail is applied later in the tick than retail's synchronous call. Visible only if something between reads the book (level-up notification / SetSpell of spell 13/14 mid-tick).
- **What reaches each arm in-game:** human's Shield absorbing a hit (direct); human's Steal Mana or Duel landing on a rival (queued).
- **Already witnessed?:** shield XP yes (mc2l24-crazy t=3209 cited in `no_mc2_xp_award_while_dead`); steal/duel XP no direct fixture found.
- **Recording ask:** MC2: level Steal Mana to a tier-up by stealing from rivals repeatedly (watch the tier-up tick). Low priority.
- **Confidence / notes:** low impact.

---

### LIFE-16: At-castle mailbox handling + spawn grace
- **Retail routine(s):** HUMAN: at own castle, FORWARD the ch0 letter into the castle (:55353-62 MC1; EF:59961 MC2), grace = 2, grace>0 → memset. AI: at own castle grace = 2 and DISCARD (:17971-79; EF:5397-5414). Retail asymmetry (two routines).
- **Port arms:** `engine/world.rs:6374 player_mail_block` — HUMAN/both; `rival_alive_tick` / `mc2_rival_alive` grace blocks — RIVAL/MC1, RIVAL/MC2.
- **Status:** IDENTICAL (rival twins match each other; human matches retail's human arm)
- **Differences:** at-castle predicate differs per rival arm (register vs `rival_castle()`), same as LIFE-13.
- **Already witnessed?:** yes (mc1l0 t=1827, mc1l5 t=11681, mc2l6-rsg t=148/t=3084 cited in code).
- **Recording ask:** none.

---

## Summary table

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| LIFE-1 | MC1 intake `sub_46540` | DRIFTED | human, rival (+cheat) | y — rival Steal Mana + Tether on the human (MC1) |
| LIFE-2 | MC2 intake `sub_5EFA0` | DRIFTED | human, rival (+cheat) | y — human killed by (10,67); charged shield lethal on empty purse (MC2) |
| LIFE-3 | Invincibility-cheat intake copy | STRUCTURALLY-DIFFERENT | human normal vs human cheat | y (low priority, cheat) |
| LIFE-4 | Lethal transition | IDENTICAL | human ×2, rival MC1, rival MC2 | n |
| LIFE-5 | Death fall | DRIFTED | human MC1, rival MC1, human MC2, rival MC2 (+pinned) | y — MC1 rival that tethered you dies nearby |
| LIFE-6 | Landing payout | DRIFTED | 4 | y (hard) — death on a full pool; MC2 death mid-window |
| LIFE-7 | Kill credit | DRIFTED | 4 | y — MC1 killed by rival + stats screen; lane possibly ungraded |
| LIFE-8 | Dead-wait | DRIFTED (switch/seat) | 4 (+pinned MC2) | n |
| LIFE-9 | MC1 respawn `sub_44D30` | DRIFTED | human, rival | y — stale killer after suicide; tether across respawn |
| LIFE-10 | Killer latch lifetime | DRIFTED | human MC1, human MC2, rival ×2 | y — MC1 die to rival, respawn, Shift+K |
| LIFE-11 | MC2 respawn `sub_5C950` | DRIFTED | human, rival | n (verify bit-0 first) |
| LIFE-12 | Duel lock register | DRIFTED | 4 caster/victim legs | y — MC1 rival tethers the human |
| LIFE-13 | Alive regen / floors | DRIFTED (rival twins) | human ×2, rival ×2 | n |
| LIFE-14 | Cast debit / regen pin | DRIFTED | human + 12 inline rival copies | n (TOKENS census) |
| LIFE-15 | Intake-side XP | STRUCTURALLY-DIFFERENT | direct vs queued | y (low) |
| LIFE-16 | At-castle mailbox / grace | IDENTICAL | human, rival ×2 | n |

## Clusters seen that belong to OTHER subsystems
- **FLIGHT:** the shared mover split — `mc1/rivals.rs rival_death_fall` inline `sub_455D0` vs `flight::mc1_move_duel`; `mc2/rivals.rs mc2_rival_carpet_move` vs `flight::mc2_move` (stuck nudge, sink, strafe, knock decay, flutter, wall gate). MC1 human duel-grip translation (`step_player_flight`) vs its kill-switch knock-channel copy (`engine/world.rs:~8355`).
- **TOKENS/CAST:** `sub_55E80`/`sub_68DE0` debit + mid-burst pin inlined 12× in the rival token ticks vs `mana_debit`/`suppress_regen`; `mc2_set_spell` vs `mc2_rival_set_spell`; `mc2_remint_book` vs `mc2_rival_respawn`'s inline loop vs `mc2_mint_rival_manifestation`; MC1 `death_regrant` vs `rival_respawn`'s inline re-grant vs `mint_manifestation`.
- **RIVAL AI:** truce roster call paths (`mc1_truce_roster`/`mc2_truce_roster` from human vs rival respawn, and from spawn); hate sweeps `proj_hate_sweep` vs `mc2_proj_hate_sweep`; `mc2_intake_drops_dead_human` target drop.
- **ALLOCATOR:** `mc1_rebuild_free`/`rebuild_recycle`/`mc2_rebuild_free` + victim pop at 4 landing sites and 4 respawn sites (switch names differ per site).
- **CASTLE:** respawn reprice `mc1_respawn_reprice` (human `no_mc1_human_respawn_reprice` vs rival `no_mc1_respawn_reprice_owner`, with different fallback predicates `player_castle_bound()` vs the register).
- **VILLAGE/AGGRO:** wanted-timer decay seats (`no_mc1_wanted_walk_seat`/`no_mc2_wanted_walk_seat`) and respawn hold — human only; rival `rival_wanted` counterpart not checked.
