# ARM CENSUS: CAST (spell casting and spell tokens, human vs rival, MC1/HW + MC2)

Read-only research. Line numbers are against the working tree as of 2026-09-24 (branch `re_recording`).
Paths are relative to `crates/mgc-sim/src/`. "Witnessed" means a fixture note in `conformance/<take>.json`
names the law for that arm. A `(code cites …)` entry means the port source names a take/tick but I found
no fixture note for it.

Two structural facts matter for every cluster below:

- **MC1 human tokens have two encodings.** In native play a human token is `tick70 = MANIFEST_BASE + spell`.
  In a conformance import it is `tick70 = 3*spell`. `class12_tick` (engine/world.rs:16172) sends both
  to `manifestation_tick`, with two exceptions. Under the strict encoding, spell 1 goes to
  `mc1_heal_token_tick` and spell 4 goes to `mc1_shield_token_tick` (world.rs:16240-16251). In native play
  spell 4 goes through the `manifestation_tick` skeleton. So the human Shield has a native-only arm that
  no replay can ever grade.
- **MC2 rival tokens have up to four bodies**, and which one runs depends on the token's pool slot relative
  to its wizard and on whether the carpet is pooled.
  - Replay, level-load book (tokens above the wizard): the caster-slot stand-in `mc2_rival_buffs` handles
    the countdown, pin, publish and expiry. `mc2_rival_token_fire_at_slot` fires at the token's own slot.
  - Replay, respawn-minted book (tokens below the wizard): `mc2_rival_token_tick` runs the whole body.
  - Speed (spell 3) always runs in `mc2_rival_manifestation_tick`, and Castle (spell 2) always runs in
    `mc2_rival_castle_token_tick`.
  - Native MC2 has no pooled carpet, so `mc2_token_slot_dispatch()` is false. Only the `mc2_rival_buffs`
    stand-in runs there (see CAST-12, native hole).

---

### CAST-1: MC1 token gate `sub_55DD0_56300` (the "may this burst tick run" predicate)
- **Retail routine(s):** sub_55DD0_56300 (MC1 CARPET.EXE; HW twin in HIDDEN.EXE). One predicate that every
  class-12 token machine calls: purse ≥ 0, caster `actLife ≥ 0`, token castle requirement ≤ bound castle
  store, and on the FULL tick only, purse ≥ token cost.
- **Port arms:**
  - `engine/world.rs:6225 mc1_token_gate`: HUMAN, full-tick form (also called on mid ticks by heal)
  - `engine/world.rs:6263 mc1_token_gate_hard`: HUMAN, mid-burst "hard legs only" form
  - `engine/world.rs:15517 spell_gate`: HUMAN, command-site copy (legacy `cast_spell`, used for 12 Invisible and other non-token spells)
  - `engine/world.rs:16080 mc1_shield_token_tick`: HUMAN/STRICT, an inline copy
  - `engine/world.rs:16907 manifestation_tick` spell-16 arm: HUMAN, inline `hard_fail` copy
  - `mc1/rivals.rs:3398 rival_token_gate`: RIVAL
  - `mc1/rivals.rs:3143 rival_castle_token_tick`: RIVAL, inline copy ("the inlined twin of rival_token_gate")
  - `mc1/rivals.rs:3066 rival_heal_token_tick`: RIVAL, extra inline afford leg
- **Status:** DRIFTED
- **Differences:**
  - **Death leg.** HUMAN tests `player.state != LifeState::Alive`. RIVAL tests `ent.act_life < 0`, which is
    retail's `a2[3] < 0`. On the fatal tick the human is still `Alive` (entry-state law), so a human token
    dispatched after the fatal hit in the same tick keeps running where retail's `actLife < 0` leg refuses it.
  - **Negative-purse leg.** RIVAL tests the owner record's live `f140 < 0` on every tick (switch
    `MGC_NO_MC1_RIVAL_TOKEN_GATE_LIVE_PURSE`, witnessed mc1l49 t=19557). HUMAN
    `mc1_token_gate_hard` tests `(player.mana as i32) < 0`. HUMAN `mc1_token_gate` (full tick) has no
    negative leg: a u32 purse that wrapped negative passes `mana >= cost`. The strict-arm inline copy in
    `mc1_shield_token_tick` also lacks it.
  - **Per-tick hard gate.** On the HUMAN arm this sits behind `MGC_NO_MC1_TOKEN_GATE_PER_TICK`, plus
    `MGC_NO_MC1_SPEED_TOKEN_HARD_GATE` for 2/21. The RIVAL arm always runs per tick and has no switch.
    `mc1_shield_token_tick` (human strict) ignores both switches and never runs the hard legs mid-burst:
    `(!full || mana >= cost)` with no negative-purse leg.
  - **Castle requirement source.** HUMAN uses `spell_castle_req(id)`, which returns 0 when the owned token
    carries `BLUE_SPELL` (blue jar). RIVAL uses the table `spells()[spell].castle_req` with no blue test.
  - **Castle store resolution.** HUMAN uses `player_castle_bound()` (switch `MGC_NO_CASTLE_BIND_REGISTER`).
    RIVAL uses `rival_castle_reg()` (switch `MGC_NO_MC1_HOME_CASTLE_REGISTER`). Both read the register by
    default, but through different kill switches.
  - **Full-tick cost operand.** HUMAN uses `spell_cast_cost(id)`: the table cost, except spell 16, which reads
    the live `+136`. RIVAL uses the table `possess_mana` (spell 16 is handled by the castle arm's
    `rival_castle_price`). HUMAN Heal and the strict Shield also test the token's own `f136`.
  - **Purse operand.** HUMAN uses `player.mana`. RIVAL uses live `ent.f140` by default (mirror `r.mana` under
    `MGC_NO_MC1_RIVAL_TOKEN_GATE_LIVE_PURSE`).
  - **Refusal.** HUMAN buzzes sound 29 at every token-side refusal. RIVAL is silent (retail's buzz is the
    local player's channel).
  - **Dev bypass.** `dev_spells` short-circuits every human copy. There is no rival equivalent (by design).
- **What reaches each arm in-game:**
  - HUMAN: any MC1/HW human spell burst (fireball, shield, rebound, speed, castle…) while that token's counter runs.
  - RIVAL: any rival burst. The rival arm refuses on castle store (mc1hwl0 Wall of Fire) and on live purse.
- **Already witnessed?:**
  - HUMAN per-tick hard leg: yes (mc1l49 fixture "THE TOKEN GATE IS PER TICK, NOT PER ARM", t=9646, a
    castle downgrade cancels Global Death).
  - RIVAL castle-store refusal: yes (mc1hwl0 "THE CASTLE-STORED LADDER IS THE TOKEN'S GATE, NOT THE COMMIT'S").
  - RIVAL live purse: yes (mc1l49 t=19557).
  - HUMAN death leg during a live burst: no evidence found.
  - HUMAN blue-jar castle-req 0 at the token: no fixture (unit test `blue_jar_unrestricts_its_spell_and_survives_death` only).
- **Recording ask:**
  1. MC1 (any level with rivals, e.g. level 3): cast Shield or Rebound (long bursts), then get killed while
     the burst is still counting. Repeat 3-4 times, about 10 min. This exercises the human "Alive vs
     actLife<0" leg on the fatal tick.
  2. MC1 with a castle: cast a castle-gated spell (Global Death / Wall of Fire) and let an enemy knock the
     castle down a level mid-burst. The human arm is already witnessed, so this is optional.
  3. Blue jar: pick up a blue jar of a castle-gated spell, cast it with no or small castle, and keep casting
     while the castle changes level.
- **Confidence / notes:** High on the code differences. The human purse is a `u32`, so the "negative purse"
  leg only matters if the human purse can wrap (the rival's does on its fatal tick). Whether the human's
  can is unverified.

### CAST-2: MC1 launcher skeleton (`sub_56090` fireball machine and its twins)
- **Retail routine(s):** sub_56090_565C0 (fireball) and its byte-identical siblings for 3/7/8/11/13/15/17/20/23
  (plus 6/9/10/18/19/22, which only the human casts). Per burst tick: gate, then on the FULL tick emit plus
  `sub_55E80` debit, on mid ticks pin regen, then decrement last.
- **Port arms:**
  - `engine/world.rs:16907 manifestation_tick` (the `launcher` set + per-tick gate block, :17077-17120): HUMAN
  - `mc1/rivals.rs:2825 rival_manifestation_tick`: RIVAL
- **Status:** DRIFTED
- **Differences:**
  - **Spell sets.** HUMAN launcher = {0,3,6,7,8,9,10,11,13,15,17,18,19,20,22,23}. RIVAL = {0,3,7,8,11,13,15,17,20,23}.
    Every other spell returns immediately on the rival side. The rival AI never casts 6/9/10/18/19/22
    (`rival_cast` refuses 18-23; no picker offers 6/9/10), so this is latent.
  - **Mid-burst gate switch.** HUMAN mid-burst uses `mc1_token_gate_hard`, behind
    `MGC_NO_MC1_TOKEN_GATE_PER_TICK`. RIVAL always runs `rival_token_gate(.., full=false)` (CAST-1 leg
    differences apply).
  - **Extra rival full-tick leg.** After `rival_token_gate` passes, the RIVAL full tick re-tests
    `ent.tick70 == 1 && rivals[ri].mana >= cost`. That second test reads the cached MIRROR purse, which the
    live-purse law deliberately stopped using in the gate. On failure it sets `f26 = 1` (no emit). HUMAN has
    no second test.
  - **Emit pose.** HUMAN emits from `mc1_cast_pose` (the record pose at walk time) with the lateral hand muzzle
    (`mc1_fire_hand`, ±256 units, `dual_wield_muzzle` patch). RIVAL emits from the owner's live `(x, y, z+f78)`
    and its yaw/pitch, with no lateral muzzle.
  - **Refusal.** HUMAN buzzes 29. RIVAL is silent.
  - **Earthquake (6).** On the HUMAN side, spell 6 skips the mid-burst regen pin (`spell != 6`). The rival
    has no 6 arm at all.
- **What reaches each arm in-game:**
  - HUMAN: the player casts any projectile spell in MC1/HW.
  - RIVAL: a rival wizard fires fireball / possess / meteor / volcano / duel / steal mana / lightning /
    undead army / wall of fire at you, a castle, a balloon or mana.
- **Already witnessed?:**
  - HUMAN: yes (mc1l4 "REPEAT FIREBALLS (23) IS A LAUNCHER"; mc1hwl0 lightning t=22112; mc1l49 per-tick gate).
  - RIVAL: yes (mc1hwl0 "THE CASTLE-STORED LADDER IS THE TOKEN'S GATE…", Wall of Fire 26→1→0; the code cites
    mc1l2 token 301, dying-owner double drop).
  - The rival mirror-purse extra leg: no evidence found.
- **Recording ask:** The difference that matters is the rival's mirror-purse leg: a rival whose live purse
  was debited mid-tick by combat just before its own launcher token fires. In MC1, fight a rival up close
  with rapid hits while it casts fireballs (a level with an aggressive rival, e.g. MC1 level 5 Vodor).
  About 10 min of close duelling. The emit-pose difference is exercised by any rival cast and is already graded.
- **Confidence / notes:** The emit-pose difference probably reflects retail. Rival hand bits are cleared at
  commit (:19110), so the muzzle offset is a no-op, and the rival's lift is `f78` where the human's is
  `PLAYER_HH`.

### CAST-3: MC1 Heal token `sub_56270_567A0`
- **Retail routine(s):** sub_56270_567A0 (MC1 :65091 / HW :61313, byte-identical). Heals the OWNER 5% per
  tick. Admission is gate AND `actLife < maxLife` AND purse ≥ `+136`, on every tick. Debits the full cost
  on every admitted tick. A refusal releases the burst.
- **Port arms:**
  - `engine/world.rs:16029 mc1_heal_token_tick`: HUMAN (both encodings)
  - `mc1/rivals.rs:3066 rival_heal_token_tick`: RIVAL
  - Dead code: `manifestation_tick`'s `match spell { 1 => … }` tail (world.rs ≈17243) is unreachable
    because spell 1 returns early. It is a stale 5%-per-tick / cost-per-count reconstruction.
- **Status:** DRIFTED (small)
- **Differences:**
  - **Admission.** HUMAN = `mc1_token_gate(1)`, which applies the table full-price leg on every tick
    (`mana >= spell_cast_cost(1)`, not full-only), plus `life < PLAYER_LIFE_MAX`, plus `mana >= f136`.
    RIVAL = `rival_token_gate(full)` (price leg on the full tick only), plus `act_life < max_life`, plus
    live `f140 >= f136`.
  - **Life ceiling.** HUMAN uses the constant `PLAYER_LIFE_MAX`. RIVAL uses the owner's `max_life`.
  - **Death leg** as in CAST-1: HUMAN `state != Alive`, RIVAL `act_life < 0`. HUMAN has no negative-purse leg.
  - **Sound 25.** HUMAN uses `snd_player`. RIVAL uses `snd(25, owner)`.
  - HUMAN sets `player.heal_active`. RIVAL has no mirror.
  - RIVAL with `i == 0` (no owner) still decrements. HUMAN has no such guard.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Heal while hurt, or at full life (instant release).
  - RIVAL: a hurt rival casts Heal on a think tick.
- **Already witnessed?:**
  - HUMAN: no fixture found (code cites mc1l42 t=10669 and mc1l0-sg t=5447, free-run only).
  - RIVAL: yes (mc1hwl0 t=17050 "THE HEAL TOKEN HEALS ITS OWNER AT ITS OWN POOL SLOT").
- **Recording ask:** MC1, any level. Get hurt, cast Heal and let it run. Cast Heal again at full life.
  Cast Heal with mana just above and just below 1000. Also cast Heal just as you drain to under the cost
  mid-burst. About 5 min. This gives the human arm its first fixture.
- **Confidence / notes:** The every-tick table price leg on the human side is redundant when `f136` equals
  the table cost, so the arms differ only for a re-priced heal token (unlikely in MC1).

### CAST-4: MC1 Shield token `sub_566C0` (three arms)
- **Retail routine(s):** sub_566C0 (MC1 :65266). The Rebound skeleton with the owner bit +17 0x40 (set-only,
  cleared per absorb by the damage intake).
- **Port arms:**
  - `engine/world.rs:16080 mc1_shield_token_tick`: HUMAN/STRICT (conformance encoding only)
  - `engine/world.rs:16907 manifestation_tick` with `skeleton` (4) plus the `4 => player.shield = true` tail: HUMAN/NATIVE
  - `mc1/rivals.rs:3019 rival_shield_token_tick`: RIVAL
- **Status:** DRIFTED
- **Differences:**
  - **Full-tick cost.** HUMAN/STRICT uses the token's `f136` (falls back to the table). HUMAN/NATIVE uses
    `spell_cast_cost(4)`, which is the table cost. RIVAL uses the table `possess_mana`.
  - **Mid-burst legs.** HUMAN/STRICT runs `state == Alive && castle req` only: no negative-purse leg and no
    per-tick switch. HUMAN/NATIVE runs `mc1_token_gate_hard` behind `MGC_NO_MC1_TOKEN_GATE_PER_TICK` (this
    includes the negative-purse leg). RIVAL runs `rival_token_gate(full=false)`.
  - **Shield bit.** HUMAN/NATIVE sets `player.shield` in the tail only when `active` (post-decrement f26 > 0).
    On the tick the counter drops 1→0 it does NOT set the bit. STRICT and RIVAL set it on every admitted tick,
    including the last. Retail sets it on every tick the gate passes.
  - **Refusal.** Both human arms buzz 29. RIVAL is silent.
- **What reaches each arm in-game:**
  - HUMAN/STRICT: only conformance replay of a human Shield.
  - HUMAN/NATIVE: the real game (player's own casts). It is never graded.
  - RIVAL: a rival's defense ladder picks Shield (projectile models 4/9 homing on it).
- **Already witnessed?:**
  - HUMAN/STRICT: yes (mc1l3 t=4275 "SHIELD IS A LAUNCHER-FAMILY SPELL", t=4334).
  - HUMAN/NATIVE: impossible to witness by replay (native-only arm).
  - RIVAL: yes (mc1hwl0 t=5592 "THE SHIELD TOKEN IS THE REBOUND SKELETON WITH A SET-ONLY BIT").
- **Recording ask:** No recording can reach the native arm. It needs a code-side unification or a unit
  test. To watch the last-tick bit: MC1, cast Shield and arrange to be hit exactly as it expires (hard to
  aim). Low value. Any Shield take already covers the other arms.
- **Confidence / notes:** High. This is exactly the "native arm invisible to replay" class
  (memory: REPLAY IS BLIND TO A NATIVE-ONLY HOLE).

### CAST-5: MC1 Rebound token `sub_573F0_57920`
- **Retail routine(s):** sub_573F0_57920 (MC1 :65774 / HW :61996). The bare skeleton: per-tick gate, the
  owner's +17 0x80 deflection bit, `sub_55E80`, decrement. The clear arm runs only when a tick is ENTERED
  with `+48 <= 0`.
- **Port arms:**
  - `engine/world.rs:16907 manifestation_tick` (skeleton + the `14 =>` tail): HUMAN
  - `mc1/rivals.rs:2914 rival_rebound_token_tick`: RIVAL
- **Status:** DRIFTED (converged in behaviour; different switch coverage)
- **Differences:**
  - **Bit-lag law.** HUMAN computes the bit after the decrement with an explicit lag law, behind
    `MGC_NO_MC1_REBOUND_BIT_LAG` (mc1l14). RIVAL has the same shape natively (it clears at entry when
    `f26 <= 0`) and has no switch.
  - **Mirror republish.** HUMAN republishes `g.player_rebound` mid-walk. RIVAL writes owner `flags |= 0x8000`.
  - **Gate differences** as in CAST-1 (mid-tick switch on the human only; death leg; purse operand).
  - **Refusal.** HUMAN buzzes 29.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Rebound (MC1 / HW).
  - RIVAL: a rival's defense reflex (incoming fireball / possess / castle ball) casts Rebound.
- **Already witnessed?:**
  - HUMAN: yes (mc1l14 t=20607 "THE REBOUND BIT OUTLIVES THE COUNTER BY ONE TICK"; mc1l32 t=18013/18014).
  - RIVAL: yes (mc1hwl0 t=5591 "THE REBOUND TOKEN PUBLISHES THE OWNER BIT AT ITS OWN POOL SLOT").
- **Recording ask:** None needed for the default arms. The only open cell is the human death leg (CAST-1 ask 1).
- **Confidence / notes:** High.

### CAST-6: MC1 Invisible token `sub_571B0_576E0`
- **Retail routine(s):** sub_571B0_576E0 (MC1 :65675 / HW :61899). The skeleton with the owner's +16 0x20
  cloak bit. The FULL tick sets the bit and zeroes the spawn grace (`wizext+331 = 0`). A mid tick whose bit
  was externally broken sets `+48 = 1`. Gate refusal does the same. The debit/pin is `sub_55E80`. The bit
  clears when the counter lands on 0.
- **Port arms:**
  - HUMAN, split across two places:
    - `engine/world.rs:13765 cast_spell` (command-site `spell_gate` + `mana_debit(possess_mana)` + arm)
    - `engine/world.rs:16907 manifestation_tick` spell 12 (neither `launcher` nor `skeleton`: pin-only
      countdown, `12 => player.invisible = active`)
    - plus `break_cloak` (world.rs ≈13910)
  - RIVAL: `mc1/rivals.rs:2965 rival_invis_token_tick`
- **Status:** STRUCTURALLY-DIFFERENT / DRIFTED
- **Differences:**
  - **Debit site.** HUMAN pays the cost at the COMMAND site (`cast_spell`, pre-step purse, with the table
    castle-req via `spell_gate`). RIVAL pays on the token's FULL tick through `sub_55E80`. Every other MC1
    skeleton spell (4, 5, 14) was moved token-side for the human (mc1l3, mc1l48, mc1l32 fixtures). Spell 12
    was not: `mc1_cast_command`'s launcher list excludes 12, and `manifestation_tick`'s `skeleton` excludes 12.
  - **Gate.** HUMAN runs no gate at all on the token, so a castle downgrade or death mid-burst does not
    cancel the cloak. RIVAL runs the full `sub_55DD0` gate every tick (refusal = the fizzle inside the commit
    tick, mc1hwl0 t=20697).
  - **Broken-cloak collapse.** HUMAN `break_cloak` (casting another spell) zeroes the window to `f26 = 0`
    immediately at the command site. RIVAL: a mid tick finding the owner's 0x20 cleared sets `f26 = 1`, then
    the shared decrement ends it at the token's slot.
  - **Spawn grace.** HUMAN does not zero the spawn grace on the full tick. RIVAL zeroes `grace`.
  - **Regen pin.** HUMAN pins on every live tick including the arm tick (the delta is already negative from
    the command debit). RIVAL pins only on mid ticks after a passing gate.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Invisible in MC1/HW, then casts something else (breaks the cloak), loses a castle
    level, or respawns and gets hit during grace.
  - RIVAL: a fleeing rival casts Invisible.
- **Already witnessed?:**
  - HUMAN: partially (mc1l0 t=4677 "THE INVISIBLE TOKEN RUNS, AND A LIVE BURST FREEZES THE CASTER'S MANA"
    witnesses only the regen pin).
  - RIVAL: yes (mc1hwl0 t=20697).
- **Recording ask:** MC1 (or HW) with a castle, with Invisible owned.
  1. Cast Invisible with mana just above cost and watch where the debit lands. The command-site vs token
     debit is one tick apart.
  2. Cast Invisible, then within a few seconds cast a fireball (breaks the cloak).
  3. Cast Invisible, then let a rival knock your castle below Invisible's castle requirement mid-burst.
  4. Die with Invisible up, respawn, and cast Invisible during the spawn-grace window.
  About 15 min.
- **Confidence / notes:** High on the code. That retail's human uses the same `sub_571B0` is stated by the
  port's own comments (dispatch row 0x24 serves every wizard).

### CAST-7: MC1 Speed tokens `sub_56380_568B0` / `sub_57F00_58410` (Accelerate forward/back)
- **Retail routine(s):** sub_56380 (spell 2) and its negated twin sub_57F00 (spell 21). The v_14 kill,
  the +16 bit 7 active bit, the 3×/2× speed override on the record, the (10,2) contrail every 4th tick,
  `sub_55E80`, and the expiry snap to ±base.
- **Port arms:**
  - `engine/world.rs:16907 manifestation_tick` (`speed_token` block ≈:17135-17215 + `2 | 21 =>` tail): HUMAN
  - `mc1/rivals.rs:3317 rival_speed_token_tick`: RIVAL
- **Status:** DRIFTED
- **Differences:**
  - **Mid-tick gate.** HUMAN uses `mc1_token_gate_hard`, behind two switches
    (`MGC_NO_MC1_TOKEN_GATE_PER_TICK`, `MGC_NO_MC1_SPEED_TOKEN_HARD_GATE`). RIVAL uses `rival_token_gate`
    and has no switch.
  - **Base speed.** HUMAN uses the literal 80 ("human's +128 measured 80"). RIVAL uses the owner's `f128`.
  - **Speed write.** HUMAN posts a mail (`pending_speed_base`) plus `mc1_cast_pose.speed`, behind
    `MGC_NO_MC1_SPEED_TOKEN_CAST_POSE`. RIVAL writes `vdes` and `f126` directly on the record.
  - **Contrail.** HUMAN puffs only while `player.state == Alive`. RIVAL puffs regardless of owner life.
    Retail has no life test at that line. The puff happens inside the sustain arm, so a dead owner is refused
    by the gate first on the rival side (act_life < 0). The human's extra `Alive` test matters only on the
    fatal tick.
  - **Owner-gone.** RIVAL with no owner record returns without decrementing (the counter stalls). HUMAN has no
    such case.
  - **Expiry.** HUMAN also clears `player.accel` and `speed_boost`. The rest (`0x80` clear, base restore) is
    the same.
  - **Refused full tick.** HUMAN still decrements, with no debit and no pin. RIVAL is the same.
  - **Chime 19.** `snd_player` vs `snd(19, owner)`.
- **What reaches each arm in-game:**
  - HUMAN: the player casts or holds Accelerate / Accelerate Backwards.
  - RIVAL: a rival cruising to a far target casts spell 2 (rivals never cast 21).
- **Already witnessed?:**
  - HUMAN: yes (mc1l1 t=8748 "THE SPEED-TOKEN JAR-SIDE COUNTDOWN"; mc1l48 t=13554 "THE SPEED TOKEN WRITES +126 ON THE RECORD").
  - RIVAL: yes (mc1l4 t=1; mc1l49 t=21961 v_14 kill).
  - HUMAN death mid-burst (hard gate): no fixture found (code cites the switch only).
- **Recording ask:** MC1: hold Accelerate and die while the burst runs (fly into a rival's fireball stream
  or a volcano). Repeat 3 times, about 5 min. This is the human hard-gate / contrail-on-fatal-tick
  difference.
- **Confidence / notes:** The 80 literal vs `f128` difference is invisible unless the human's `+128` ever
  differs from 80. The port comment says it was measured 80 on every take.

### CAST-8: MC1 Create-Castle token `sub_57610_57B40` (+ ball mint)
- **Retail routine(s):** sub_57610_57B40 (MC1 :65862-923; HW :62103, byte-identical), shared by human and
  rival owners. While `+48 > 0` it gates. On FULL it debits (`sub_55E80`), then mints the ball
  (`sub_373F0(9,10)`). Only if a ball was minted: latch `+48 = +50 − 1`, bank the charge, sound 15. It
  splits create vs upgrade on the `wizext+50` register.
- **Port arms:**
  - HUMAN: `engine/world.rs:16907 manifestation_tick` spell-16 arm, plus `engine/world.rs:15586 cast_castle` (the ball mint, shared with MC2's human)
  - RIVAL: `mc1/rivals.rs:3143 rival_castle_token_tick` (gate + mint inlined)
- **Status:** DRIFTED
- **Differences:**
  - **Debit before mint.** HUMAN always debits before the mint (no switch). RIVAL does too, but behind
    `MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER`.
  - **Dry pool.** HUMAN keeps `+48` full and retries next tick, behind
    `MGC_NO_MC1_CASTLE_TOKEN_DRY_POOL_RETRY` (mc1l25). RIVAL: an allocation failure returns and stays full,
    with no switch.
  - **Mid-latch gate.** HUMAN refuses on `state != Alive || (mana as i32) < 0`, with the full leg via
    `mc1_token_gate(16)`. RIVAL refuses on `act_life < 0 || f140 < 0`, with the full leg `f140 >= price`.
    Both release by setting `f26 = 0`. HUMAN buzzes 29.
  - **Create-vs-upgrade register.** HUMAN uses `player_castle_bound()` (switch `MGC_NO_CASTLE_BIND_REGISTER`).
    RIVAL uses `wiz_castle_reg` (switch `MGC_NO_MC1_RIVAL_CASTLE_TOKEN_REGISTER`).
  - **Ball position.** HUMAN mints at the hand muzzle (`muzzle(p, right)`, or the carpet under the
    `castle_latch_bug` patch in native), with z = `p.z + PLAYER_HH`. RIVAL mints at the owner's `(x, y, z)`
    then adds `z += f84`.
  - **Ball stamps.** HUMAN stamps `f44 = SPELLS[16].damage` and `f140 = spell_cast_cost(16) / count`.
    RIVAL copies the TOKEN's `f44` and `f140`. Retail: `*(v3+44) = *(a1+44)`, `*(v3+140) = *(a1+140)`.
  - **Upgrade dest triple.** HUMAN leaves it unwritten, behind `MGC_NO_MC1_CASTLE_UPGRADE_BALL_NO_DEST`.
    RIVAL never writes it (no switch).
  - **Create ball `site_z`.** Both arms gate it on `MGC_NO_MC1_BOLT_DEST_STAMP`.
  - **Sound 15.** HUMAN uses `snd_player`. RIVAL uses `snd(15, ball)`.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Create Castle (first castle or upgrade).
  - RIVAL: a rival plants, then upgrades its keep.
  - Dry pool: a crowded level (many creatures and effects) at the moment of the cast.
- **Already witnessed?:**
  - HUMAN: yes (mc1l0 t=559 latch; mc1l25 t=7014 "THE CASTLE TOKEN'S LATCH IS INSIDE `if (v3)`"; mc1l48 t=27413).
  - RIVAL: yes (mc1l5 t=5151; mc1l26 t=25107; mc1l49 t=15551 "THE CREATE-CASTLE TOKEN BILLS BEFORE IT ASKS THE POOL").
- **Recording ask:** The ball `f44`/`f140` stamp differs by arm. The human's table stamp equals the token's
  own stamp unless the token was re-priced (castle levels 1+). MC1, build a castle and upgrade it 2-3 times;
  each upgrade ball's `f140` is graded. About 10 min. This is probably already in the corpus.
- **Confidence / notes:** Medium-high. The human `f140 = cost/count` vs the rival token copy agree whenever
  the token's `+140` is `+136/101`, which the ladder writers keep true.

### CAST-9: MC1 token emission (the per-spell emit halves of the class-12 machines)
- **Retail routine(s):** the emit blocks inside sub_56090 (0/23), sub_56510 (3), sub_56950 (7), volcano,
  crater, sub_57040 (11), steal (13), sub_57470 (15), undead (17), sub_57D40 (20). One set of functions
  for human and AI (`str_2563D8`, state = 3*spell).
- **Port arms:**
  - HUMAN: `engine/world.rs:13923 emit_spell`, `:14069 cast_fireball`, `:14105 cast_projectile`, `cast_firewall` (:15894), `cast_storm`, `cast_bomb`
  - RIVAL: `mc1/rivals.rs:5820 rival_emit`
- **Status:** DRIFTED
- **Differences:**
  - **Charge bank.** RIVAL has a "full family" list behind `MGC_NO_MC1_RIVAL_CHARGE_FAMILY`
    (`0|6|7|8|9|13|15|17|18|19|20|22|23`). HUMAN banks in each emitter separately (fireball in
    `cast_fireball`, `6|7|8|9|13|15|17|19` in `cast_projectile`, 20/18/22 in their own casts). There is no
    switch on the human side.
  - **Meteor (7) dest triple.** HUMAN writes it inline, behind `MGC_NO_MC1_METEOR_DEST_STAMP`. RIVAL routes it
    through `mc1_stamp_bolt_dest` (switch `MGC_NO_MC1_BOLT_DEST_STAMP`).
  - **Possess (3) and lightning (15) dest.** HUMAN writes them inline with no switch. RIVAL goes through
    `mc1_stamp_bolt_dest`, behind `MGC_NO_MC1_BOLT_DEST_STAMP`.
  - **Bolt `+36`.** RIVAL writes it only under `MGC_NO_MC1_RIVAL_EMIT_PITCH_MIRROR` (a pre-dig
    invention). HUMAN never writes it.
  - **Shared switches.** `MGC_NO_MC1_DUEL_DART_EXACT` and `MGC_NO_MC1_EMIT_DETONATION_PAIR` are consulted
    in BOTH arms (converged).
  - **Muzzle.** HUMAN uses the lateral hand muzzle. RIVAL uses the owner's position at z + `f78` (see CAST-2).
  - **Sounds.** The per-spell tables differ only for spells the rival never casts.
- **What reaches each arm in-game:**
  - HUMAN: the player casts each projectile spell.
  - RIVAL: the rival casts the same spell.
- **Already witnessed?:**
  - HUMAN: yes for most (mc1l42 crater, mc1hwl0 possess t=7853, mc1l15 meteor t=44457, mc1l48 duel).
  - RIVAL: yes (mc1l49 charge family 766 rows; mc1l2 possess `+36`).
- **Recording ask:** None. Default arms agree. The differences are switch coverage only.
- **Confidence / notes:** Overlaps PROJECTILE. Listed here because it is the token machine's emit half.

### CAST-10: MC1 spell-book mint (level start + respawn re-grant)
- **Retail routine(s):** sub_3DD50 (:49213-54) fills the `+532` acquisition list in `byte_99B88` (display)
  order for every wizard. sub_44D30 (:54882-923) mints tokens by walking that list with the class-12
  ground-jar ctor at the wizard's position, stamps `+42`/owned, and re-mints in place on respawn.
- **Port arms:**
  - HUMAN: `engine/world.rs:13687 grant_level_book` → `grant_spells` → `grant_spell` → `:12813 mint_spell_token`; respawn: `:12050 death_regrant`
  - RIVAL: `mc1/rivals.rs:1557 spawn_rival` (book loop) → `:1702 mint_manifestation`; respawn: `:6543 rival_respawn` (re-grant loop)
- **Status:** DRIFTED (switch coverage + respawn details)
- **Differences:**
  - **Book order.** HUMAN always walks `DISPLAY_ORDER` (no switch). RIVAL does too, behind
    `MGC_NO_MC1_RIVAL_BOOK_ORDER`.
  - **Mint ctor.** HUMAN switches: `MGC_NO_MC1_HUMAN_TOKEN_CTOR` (bare `new_event`) and
    `MGC_NO_MC1_HUMAN_TOKEN_STRICT_STATE`. RIVAL always uses the ctor and follows the world's encoding.
  - **Mint position.** HUMAN mints at `human_pose`. RIVAL mints at the owner record.
  - **Respawn empty entry.** HUMAN (strict) has an extra entry==0 disambiguator behind
    `MGC_NO_MC1_REGRANT_ZERO_ENTRY`. RIVAL maps `entry<0 || entry>=24` → 0.
  - **Respawn blue marker.** HUMAN restores BLUE (`death_owned_blue` → `BLUE_SPELL`, `type86 = 280`).
    RIVAL restores no blue marker.
  - **Respawn relink.** HUMAN relinks each token to the seat after the mint. RIVAL mints after moving the
    wizard to the castle, so it is already there.
  - **Native deviation.** HUMAN in native play takes the registered player-favouring deviation (a starved
    grant retries). RIVAL always loses the spell.
  - **Owned rebuild.** RIVAL runs `rival_owned_rebuild` after the re-grant (`owned_survives_scatter`). HUMAN's
    rebuild lives elsewhere.
- **What reaches each arm in-game:**
  - Level start for every wizard.
  - A castle-owning wizard (human or rival) dying and respawning at the castle.
  - The blue difference needs a wizard holding a BLUE-granted spell to die.
- **Already witnessed?:**
  - Level-start order: `init-check` lanes, round 154 (no fixture note found).
  - Human respawn: mc1l0 t=1710.
  - Rival respawn: mc1hwl0 t=25318, mc1l35 t=23647.
  - Rival-with-blue respawn: no evidence (rivals may never own blue spells; unverified).
- **Recording ask:** Low priority. If rivals can pick up blue jars at all: MC1, drop blue jars near a rival
  and kill it after it collects one. Otherwise nothing is needed.
- **Confidence / notes:** Overlaps DEATH (respawn). Rivals learn through `rival_learn_tick` (a different
  retail routine, sub_15EC0) and through jar pickups in some cases, which I did not verify.

---

### CAST-11: MC2 effect-body gate `sub_68D50` (afford)
- **Retail routine(s):** sub_68D50 (EF:55548-66). Caster purse < 0 → false, caster `life < 0` → false,
  the token's upkeep word (`+136`) must be covered by the caster's castle store, and on the FIRST tick only
  the purse must cover the full cost.
- **Port arms:**
  - `mc2/cast.rs:3360 mc2_afford`: HUMAN
  - `mc2/rivals.rs:5173 mc2_rival_afford`: RIVAL
  - `mc2/rivals.rs:8619 mc2_rival_token_fire_at_slot`: RIVAL, inline copy on the pre-decrement window
- **Status:** DRIFTED (small)
- **Differences:**
  - **Death leg.** HUMAN tests `player.state != Alive`. RIVAL tests `act_life < 0`. Same fatal-tick gap as
    CAST-1.
  - **Upkeep castle.** Both arms use a pool SCAN (`player_castle()` / `rival_castle(ent)`), not the
    `CastleEntityIndex_0x3A_58` register. This contrasts with the MC2 price path, which reads the register for
    the human (CAST-18), and with the MC1 gates, which read registers. Neither MC2 arm has been moved.
  - **Dev bypass.** HUMAN full-tick leg has a `dev_spells` bypass. RIVAL has none (by design).
- **What reaches each arm in-game:** every MC2 spell window tick, for the human and for rivals.
  The upkeep leg needs a spell whose tier has a nonzero `maxManaLimit` (castle-upkeep spells, e.g. Cave-In's
  100k, crater tier…).
- **Already witnessed?:**
  - RIVAL upkeep refusal: yes (mc2l22 t=6434 "A REFUSED SPELL BODY PAYS NOTHING AND PINS NOTHING").
  - HUMAN upkeep refusal: no evidence found.
  - Scan-vs-register split (a human with two castles, or a register that has moved): no evidence for this lane.
- **Recording ask:** MC2 with a castle split (two castles; mc2l12-style) or with a castle knocked to level 0
  while still standing. Cast an upkeep-gated spell (a high-tier crater/earthquake or similar) whose
  `maxManaLimit` sits between the two castles' stores. About 15 min, fiddly.
- **Confidence / notes:** Medium. Whether retail `sub_68D50` reads the register or a scan is inferred from
  sibling routines; I did not check the EXE.

### CAST-12: MC2 generic manifestation skeleton (`sub_693F0` and its 20 siblings: first-tick fire + `sub_68DE0` debit/pin + countdown + `sub_6D880` expiry)
- **Retail routine(s):** sub_693F0 (EF:55832) and siblings, one caster-generic body per spell, run at the
  class-15 token's own pool slot. `sub_68DE0` (EF:55569) does the debit on the first tick and the pin on
  later ticks, for the 20 spells not in `NO_MID_BURST_REGEN_PIN`.
- **Port arms:**
  - `mc2/cast.rs:3475 mc2_manifestation_tick` (+ `:3388 mc2_cast_tick` loop in native play): HUMAN
  - `mc2/rivals.rs:5271 mc2_rival_buffs` (caster-slot stand-in): RIVAL, level-load book, and ALL rival tokens in NATIVE play
  - `mc2/rivals.rs:8682 mc2_rival_token_tick`: RIVAL, respawn-minted book below the wizard, replay only
  - `mc2/rivals.rs:8619 mc2_rival_token_fire_at_slot`: RIVAL, fire half for a level-load book, replay only
  - `mc2/rivals.rs:8265 mc2_rival_cast` (the first-tick debit pre-consume when `token_later`): RIVAL
  - (death collapse walk in `mc2_rival_corpse_tokens` :3076: RIVAL corpse, DEATH overlap)
- **Status:** DRIFTED
- **Differences:**
  - **First-tick debit site.**
    - HUMAN: at the token, after `mc2_afford` evaluated at the token.
    - RIVAL level-load: pre-consumed at the COMMIT in `mc2_rival_cast`, with afford evaluated at the caster
      slot before the brain's steer.
    - RIVAL respawn book: at the token (`mc2_rival_token_tick`).
  - **First-tick test.** HUMAN uses `f26 == f28.max(1)`. RIVAL `mc2_rival_token_tick`,
    `mc2_rival_publish_buff` and the stand-in pin use `f26 == f28` (no `.max(1)`). The arm writes
    `f28.max(1)`, so a zero-duration tier (`word_0x18 == 0`) is "first" on the human side and never "first"
    on those rival paths: no fire and no debit in `mc2_rival_token_tick`. The fire in the stand-in and in
    `fire_at_slot` does use `.max(1)`.
  - **Mid-burst pin list.** HUMAN always uses `NO_MID_BURST_REGEN_PIN`. RIVAL uses the same list, behind
    `MGC_NO_RIVAL_REGEN_PIN_LIST` (both rival paths).
  - **Afford-gated pin.** HUMAN is always afford-gated. RIVAL is gated behind `MGC_NO_RIVAL_PIN_AFFORD`.
  - **Shield III pre-decrement.** HUMAN uses `MGC_NO_MC2_SHIELD3_PREDECREMENT`. RIVAL uses
    `MGC_NO_RIVAL_SHIELD3_PREDECREMENT`, with its own copy in `mc2_rival_cast`'s pre-consume. Both are
    landed; the switches are separate.
  - **Duel fizzle (spell 14, LABEL_19).** HUMAN only. Rivals never cast 14, so this is latent.
  - **Possess re-press release signal (`f56`, tier re-fire).** HUMAN only (rivals re-arm through
    `sub_5F660(.., 0)`, see CAST-19).
  - **Speed brake collapse (`v14`).** HUMAN collapses in-skeleton. RIVAL does it in
    `mc2_rival_manifestation_tick` (CAST-16).
  - **Expiry.** HUMAN runs `mc2_cast_expire` (tier drain + human flag teardown). RIVAL has an inline tier drain
    plus `clear_buff` / metamorph / invis teardown, duplicated in both the stand-in and `mc2_rival_token_tick`.
  - **Fire pose.** HUMAN uses the caster pose `p` at the token's walk point. RIVAL uses the caster's live
    `f30/f32` at the stand-in slot (after the brain) or at the token slot.
  - **⚠ Suspected native-only hole (not a recording question).** In NATIVE MC2 (`mc2_carpet_slot == 0`),
    `mc2_manifestation_pass` returns immediately, so `mc2_rival_token_tick` never runs. Meanwhile
    `mc2_rival_buffs` and `mc2_rival_heal_stand_in` both `continue`/return for tokens with `m < own_slot`.
    A rival respawn re-mints its book onto the LOWEST free slots (`mc2_rival_respawn` :10350+), usually below
    the wizard. So in native play a respawned MC2 rival's non-speed, non-castle tokens appear to get no body
    at all: the window never counts down, no first-tick fire from the stand-in, and `mc2_rival_cast` does not
    emit because `token_later` is false. Speed (3) and Castle (2) have explicit native stand-ins.
    Needs confirmation by a native run.
- **What reaches each arm in-game:**
  - HUMAN: any MC2 spell the player casts.
  - RIVAL stand-in: any rival that has never died casting in MC2 (level-load book).
  - RIVAL token_tick: a rival that has died and respawned at its castle, then casts.
- **Already witnessed?:**
  - HUMAN: yes, many (mc2l6-rsg t=6267 pin list; mc2l3 t=1496 afford gate).
  - RIVAL stand-in: yes (mc2l22 t=2513 "THE MANIFESTATION WINDOW COUNTS DOWN AT THE TOKEN'S OWN POOL SLOT";
    mc2l22 t=2562 fire at token slot; mc2l22 t=6434 refused body).
  - RIVAL respawn book: yes (mc2l6 t=3083 "A RESPAWNED MC2 RIVAL COMES BACK RAW…"; code cites mc2l6-rsg t=3258).
  - Zero-duration tier on a rival: no evidence found.
- **Recording ask:** MC2, a level with several aggressive rivals (the mc2l22 map). Kill one rival at least
  twice so it respawns with a low-slot book, then let it fight you for several minutes: fireballs,
  lightning, possession, shield, rebound. About 20 min. This adds more coverage of the `token_tick` arm for
  shield/rebound/invis teardown, which is thinner than the stand-in's.
- **Confidence / notes:** High on the code. The zero-duration first-tick mismatch depends on whether any
  SPELLS.DAT tier has `word_0x18 == 0` (not checked). The native hole is inferred from reading the code, not run.

### CAST-13: MC2 Heal `sub_6A300`
- **Retail routine(s):** sub_6A300 (EF:56430-77). Admits on `sub_68D50` && purse ≥ cost on every tick.
  Heals `maxLife*sub/100` and debits the full cost only on ticks that actually heal. A full-life tick does
  nothing and keeps the window. Sound on the first admitted tick. XP (human only) on the first healing tick.
- **Port arms:**
  - `mc2/cast.rs:4004 mc2_heal_token_tick`: HUMAN
  - `mc2/rivals.rs:5015 mc2_rival_heal_tick`: RIVAL. It is called from `:5069 mc2_rival_heal_stand_in`
    (level-load book, runs after the brain) and from `mc2_rival_token_tick` (respawn book), and each caller
    decrements for itself.
- **Status:** DRIFTED
- **Differences:**
  - **Sound 25 and heal flag.** HUMAN plays sound 25 on the first admitted tick and sets `heal_active`.
    RIVAL plays NO sound.
  - **XP.** HUMAN awards XP on the first healing tick. RIVAL has no call, which matches retail's own model-0
    guard: `mc2_award_xp` ignores non-human owners. Not a drift.
  - **Life ceiling.** HUMAN uses `PLAYER_LIFE_MAX`. RIVAL uses the owner's `max_life`.
  - **Death leg.** As in CAST-11.
  - **Release placement.** HUMAN releases by setting `f26 = 1` inside the body, then decrements.
    RIVAL `mc2_rival_heal_tick` releases by setting `f26 = 1` and returns; the caller decrements.
    - Stand-in: same result.
    - `mc2_rival_token_tick` for s==5: the heal body runs AFTER that function's own afford collapse, and
      `first` is evaluated before. Equivalent in effect, but the admission is tested twice (afford in the
      token_tick head, then again in the heal body).
  - **Expiry tier drain.** HUMAN uses `mc2_cast_expire(5)`. RIVAL does it inline, and in the `token_tick`
    path it rides the generic expiry.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Heal (hurt, or at full life, or runs dry mid-window).
  - RIVAL: a hurt rival re-casts Heal on every think tick (recast cooldown 1).
- **Already witnessed?:**
  - HUMAN: yes (mc2l0 t=22777 "MC2 HEAL IS `sub_6A300`…").
  - RIVAL: yes (mc2l22 t=728 "A RE-ARMED HEAL WINDOW IS STILL ONE TOKEN BODY"). Note that the rival fn's
    doc comment still says "UNWITNESSED ON THIS COLUMN", which is stale.
  - RIVAL respawn-book heal: no evidence found.
- **Recording ask:** The rival heal sound (sound is in no graded lane). For the respawn-book path: MC2
  mc2l22 map, kill a Heal-owning rival once, then damage it without killing it after it respawns.
  About 10 min.
- **Confidence / notes:** High.

### CAST-14: MC2 Shield `sub_6A480` / Rebound `sub_6AA00` buff publish and teardown
- **Retail routine(s):**
  - sub_6A480 (Shield, EF:56496-541): tier `life_0x1A` 0 re-stamps CHARGED (`byte[1] |= 0x40`) on every
    afforded tick. `life == 1` stamps ARMED (`byte[2] |= 0x40`) on the first tick only. The clear
    (`&= 0xFFBFBFFF`) runs on the tick the counter hits 0.
  - sub_6AA00 (Rebound, EF:56721-51): `life 0` stamps `byte[1] |= 0x80` (scatter), `life 1` stamps
    `byte[0] |= 0x10` (precise). The clear `&= 0x7FEF` runs only when a pass is ENTERED with `+2E <= 0`,
    which is one pass after the counter hits 0.
- **Port arms:**
  - HUMAN:
    - `mc2/cast.rs:3475 mc2_manifestation_tick` spell-6 / spell-8 blocks
    - `mc2_spell_fire` spell 6/8 arms (cast.rs ≈4880-4920)
    - `:4569 mc2_cast_expire` 6/8 teardown
  - RIVAL:
    - `mc2/rivals.rs:5216 mc2_rival_publish_buff`
    - `:5256 mc2_rival_clear_buff`, called from `mc2_rival_buffs` and from `mc2_rival_token_tick`
- **Status:** DRIFTED
- **Differences:**
  - **⭐ Rebound clear timing.** HUMAN clears `player.rebound` and `mc2_rebound_precise` inside
    `mc2_cast_expire(8)`, on the same tick the counter reaches 0. RIVAL defers the clear to the next pass's
    `<= 0` arm (retail's shape), behind `MGC_NO_RIVAL_REBOUND_CLEAR_DEFER` (code cites mc2l22 t=23732; no
    fixture note found). The human arm never received the deferral. On the human column the flag drops one
    tick early.
  - **Rebound precise tier.** HUMAN models `life == 1` via `mc2_rebound_precise`. RIVAL publishes only the
    `life == 0` scatter bit (per-rival precise is "not modelled"). A tier-1 rival rebound sets NO bit, so it
    deflects nothing.
  - **Shield CHARGED re-stamp.**
    - HUMAN: behind `MGC_NO_SHIELD_RESTAMP`.
    - HUMAN ARMED stage: behind `MGC_NO_MC2_SHIELD_ARMED`, plus a first-tick latch in `mc2_spell_fire` for
      every tier, left as a stand-in.
    - RIVAL: both stages together behind one switch, `MGC_NO_RIVAL_BUFF_BITS`, with no extra first-tick latch.
      RIVAL also keeps its own `shield_state` register.
  - **XP.** HUMAN shield and rebound XP are paid on absorb and on deflect
    (`MGC_NO_MC2_SHIELD_XP_ON_ABSORB`, `MGC_NO_MC2_REBOUND_XP_ON_DEFLECT`). Rivals accrue no XP (retail guard).
- **What reaches each arm in-game:**
  - HUMAN: the player casts Shield (I/II/III) or Rebound (I/II), and a bolt arrives exactly on the tick the
    window expires.
  - RIVAL: a rival's reactive defense casts Rebound (fireball/possess incoming) or Shield.
- **Already witnessed?:**
  - HUMAN rebound: yes (mc2l6 t=4847).
  - HUMAN shield armed: yes (mc2l6 t=22705; mc2l24 t=40238).
  - RIVAL rebound bit: yes (mc2l22 t=3628).
  - RIVAL shield: yes (mc2l22 t=3637, t=19865).
  - HUMAN rebound on its expiry+1 tick: no evidence found.
  - RIVAL tier-1 (precise) rebound: no evidence found.
- **Recording ask:**
  1. MC2: cast Rebound and stand in a rival's fireball stream so a bolt is likely to land within a tick of the
     window expiring. Repeat several windows, about 10 min. This is the human expiry+1 clear.
  2. MC2, a late level where rivals have Rebound tier 1 (precise), e.g. the mc2l22 map. Fire at a rival while
     its rebound is up, about 10 min. This is the rival precise tier.
- **Confidence / notes:** High on the code. The human "one tick early" claim assumes the human deflect gate
  reads `player_rebound` live, which the code comment at mc2l6 t=4847 says it does.

### CAST-15: MC2 Invisibility `sub_6B1C0` + cast cloak-break `sub_5F7E0`
- **Retail routine(s):**
  - sub_6B1C0 (EF:57068-110): the first tick sets the strength register `byte_0x1BF_447`, raises the
    caster cloak and zeroes the spawn grace. A mid tick whose caster bit was cleared collapses the window.
    Expiry clears the caster 0x20 and the strength.
  - sub_5F7E0 (EF:60983-90): called from every arm (sub_5F7B0). It clears the caster's 0x20 unless
    strength ≥ 3, or strength == 2 while the armed token is model 1.
- **Port arms:**
  - HUMAN:
    - `mc2/cast.rs:3343 mc2_arm_invis_break` (the break)
    - `mc2_spell_fire` 0xB arm (cast.rs ≈4930)
    - `mc2_cast_expire` 0xB / 4 teardown
    - skeleton countdown in `mc2_manifestation_tick`
  - RIVAL:
    - `mc2/rivals.rs:707 mc2_invis_cloak_edge`
    - `:5158 mc2_rival_invis_strength` (derived)
    - `mc2_rival_cast` cloak clear (≈:8540)
    - expiry arms in `mc2_rival_buffs` / `mc2_rival_token_tick`
- **Status:** DRIFTED
- **Differences:**
  - **Break effect.** HUMAN `mc2_arm_invis_break` sets `invisible = false`, `strength = 0`, AND zeroes the
    invis window (`f26 = 0`) at the cast site, so the window dies instantly with no further body tick. RIVAL
    `mc2_rival_cast` clears only the caster's 0x20. The window then collapses at its next body tick via
    `mc2_invis_cloak_edge` (`f26 = 1` → 0, retail's shape), behind `MGC_NO_RIVAL_INVIS_CLOAK_EDGE`.
  - **External break (metamorph expiry).** HUMAN `mc2_cast_expire(4)` clears `player.invisible` but leaves
    the invis window running (still pinning regen), with no collapse. RIVAL collapses it through the cloak edge.
  - **Strength.** HUMAN stores `invis_strength` at the first tick. RIVAL derives it from the live token.
    These should be equivalent.
  - **Grace zero.** Both honour `MGC_NO_MC2_INVIS_CLEARS_GRACE` (converged).
- **What reaches each arm in-game:**
  - HUMAN: the player goes invisible (tier 0/1/2) and then casts another spell, or has a metamorph running
    that expires while invisible.
  - RIVAL: a rival in its defense or flee state casts Invisibility and then casts again.
- **Already witnessed?:**
  - RIVAL: code cites mc2l22 t=13105 (tier 1) and t=28013 (tier 2). I found no fixture note naming
    `sub_6B1C0`.
  - HUMAN: no fixture found.
- **Recording ask:** MC2 with Invisibility (all three tiers if possible) and Metamorph.
  1. Go invisible, then cast a fireball 2-3 s later. Repeat at each tier. At tier 2, also cast Possession
     while invisible.
  2. Cast Metamorph, then Invisibility, and let the metamorph expire first.
  About 15 min. Both are graded through the mana lane (the regen pin).
- **Confidence / notes:** Medium-high. The exact tick on which retail's human window dies after a break (the
  cast tick vs the token's next body tick) is what the recording would decide.

### CAST-16: MC2 Speed `GetScroll_69DB0`
- **Retail routine(s):** GetScroll_69DB0 (EF:56205-68), caster-generic (it has a model-1 arm).
  `if (!afford || brake) { if (brake) +2E = 1 } else` it sets `speed = sign*minSpeed*(sub + first)`, lays a
  puff every 4th tick, and calls `sub_68DE0`. It decrements. At 0 it writes `speed = ±minSpeed` and drains
  the tier.
- **Port arms:**
  - `mc2/cast.rs:3475 mc2_manifestation_tick` spell-3 blocks: HUMAN
  - `mc2/rivals.rs:4894 mc2_rival_manifestation_tick`: RIVAL. Replay dispatches it at the token slot; native
    play dispatches it from the `mc2_rival_buffs` stand-in, behind `MGC_NO_RIVAL_SPEED_STANDIN`.
- **Status:** DRIFTED
- **Differences:**
  - **Direction sign.** HUMAN takes it from `mc2_cmd_speed`, behind `MGC_NO_MC2_SPEED_SIGN_CMD`. RIVAL takes
    it from `vdes` (its command register). Equivalent in concept.
  - **Base speed.** HUMAN uses the literal 80. RIVAL uses the owner's `f128`.
  - **Speed write.** HUMAN posts a `pending_speed_base` mail. RIVAL writes `vdes` and `f126` directly.
  - **Broke tick.** HUMAN does not collapse on an unaffordable tick (behind
    `MGC_NO_MC2_SPEED_BROKE_SURVIVES`), and its puff is gated `afford || switch`. RIVAL has the same shape
    natively, with no switch.
  - **First-tick test.** HUMAN uses `f28.max(1)`. RIVAL uses `f28`.
  - **Expiry.** RIVAL clears the token's `flags & 0x80`. HUMAN never touches it. HUMAN resets `accel` and
    `accel_mc2_factor`.
  - **Debit/pin.** Both happen inside the afford arm.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Speed-up (all tiers; forward or while decelerating through 0).
  - RIVAL: a rival boosting toward a far target, and braked by the water-steer turn or the defense engage.
- **Already witnessed?:**
  - HUMAN: yes (mc2l19 t=3256; mc2l20 t=31826; code cites mc2l33 t=8896 sign, galore).
  - RIVAL: yes (mc2l6 t=4, t=7061; mc2l22 t=1245, t=1675).
- **Recording ask:** None essential. The `0x80` token flag on the human's token at expiry is the only
  unwitnessed difference, and it is ungraded unless the token flags lane is graded.
- **Confidence / notes:** High.

### CAST-17: MC2 Create-Castle token `sub_69AB0` + lock release `sub_5F890(a2=0)`
- **Retail routine(s):**
  - sub_69AB0 (EF:56086-158): at `+2E <= 0` it runs `sub_6D880` (tier drain) only. Otherwise, if afford fails
    it releases `+2E = 0`. If `+2E == +30` it does the research write, then `sub_68DE0`, then mints the ball at
    the WIZARD (AI) or the hand muzzle (human), latches `+30 − 1`, banks the charge and plays sound 15.
  - sub_5F890 (EF:61029): the release. It applies `sub_6D880` to the CASTLE (the wrong record), so the tier
    lands one tick later.
- **Port arms:**
  - HUMAN:
    - `mc2/cast.rs:4053 mc2_castle_spell_tick`
    - `engine/world.rs:15586 cast_castle` (the ball mint, shared with MC1)
    - `mc2/cast.rs:4187 mc2_castle_lock_release`
  - RIVAL:
    - `mc2/rivals.rs:9009 mc2_rival_castle_token_tick`
    - `:9030 mc2_rival_castle_mint`
    - `mc2/cast.rs:4513 mc2_rival_castle_lock_release`
- **Status:** DRIFTED
- **Differences:**
  - **Tier defer lag: separate switches.** HUMAN uses `MGC_NO_MC2_CASTLE_TIER_DEFER_LAG` (mc2l7).
    RIVAL uses `MGC_NO_MC2_RIVAL_CASTLE_TIER_DEFER_LAG` (mc2l12 t=7529).
  - **Cast latch.** HUMAN has the `MGC_NO_MC2_CASTLE_CAST_LATCH` re-derive arm. RIVAL has none.
  - **Upgrade vs create castle.**
    - HUMAN `cast_castle` uses `player_castle_bound()` (the REGISTER).
    - RIVAL mint uses `rival_castle(own)` (a pool SCAN).
    - RIVAL lock release also uses `rival_castle(own)` for the re-price, while HUMAN's release uses
      `player_castle()` (also a scan).
  - **Research stamp.**
    - HUMAN stamps `mc2_research_stamp(stage, mc2_book.sel[2])` (the SELECTED tier) inside `cast_castle`,
      AFTER a successful spawn only.
    - RIVAL stamps the token's `f71` tier BEFORE the spawn, so a pool-full mint still researches. That
      matches retail's order (EF:56118-21).
  - **Ball position.** HUMAN uses `muzzle_side(p, hand)` at `p.z + PLAYER_HH`. RIVAL uses the wizard axis plus
    `z += f84` (retail: `sub_68E50` is dead for AI because the hand bits are cleared).
  - **Latch condition.** HUMAN pins `dur − 1` only if `mc2_castle_ball_aloft()` (a pool scan for a flying
    human ball). RIVAL pins when the spawn returned `Some`.
  - **Ball `f44`.** HUMAN: token `f30`, behind `MGC_NO_MC2_BALL_TOKEN_2A_HOME`. RIVAL: token `f30` always.
  - **Upgrade dest absence.** HUMAN: behind `MGC_NO_MC2_BALL_UPGRADE_AXIS_ABSENCE`. RIVAL: always absent.
  - **Sound 15.** HUMAN plays it via `mc2_spell_fire`'s arm sound; RIVAL plays `snd(15, ball)`.
- **What reaches each arm in-game:**
  - HUMAN: the player casts Castle (create, upgrade, tier 0/1/2).
  - RIVAL: a rival upgrades its keep, or builds after losing one.
  - The register-vs-scan and research differences need a castle SPLIT (two castles owned) or a register that
    moved (a castle razed to level 0 while standing).
- **Already witnessed?:**
  - HUMAN: yes (mc2l0 t=12777; mc2l6 t=323; mc2l7 t=25587; mc2l3 t=15905).
  - RIVAL: yes (mc2l12 t=7529, t=10015).
  - Rival research-before-spawn on a dry pool: no evidence.
  - Human tier-selected vs token-tier research mismatch: no evidence.
- **Recording ask:** MC2:
  1. Select castle tier 2 while the castle token is mid-window (so the selected tier ≠ the token tier), then
     cast the upgrade. About 5 min.
  2. Build a second castle while the first still stands (the mc2l12 split recipe), then cast upgrades.
     About 15 min.
- **Confidence / notes:** Overlaps CASTLE. Only the cast-side twins are listed.

### CAST-18: MC2 `SetSpell_6D5E0` / `GetSpellManaCost_6D710` (tier pricing)
- **Retail routine(s):** SetSpell_6D5E0 (Level.cpp:1505). It defers when the window is live
  (`+2C = t+1`). Otherwise it writes tier, sub, duration, cadence, upkeep, cost and cost/duration.
  GetSpellManaCost_6D710 (L:1714-85) prices spell 2 off `CastleEntityIndex_0x3A_58`. The global cheat flag
  `OptionsSettingFlag_24 & 0x20` zeroes upkeep and sets cost 1 (L:1531-35).
- **Port arms:**
  - `mc2/cast.rs:2402 mc2_set_spell_at` + `:2259 mc2_spell_mana_cost_at`: HUMAN
  - `mc2/rivals.rs:2814 mc2_rival_set_spell_at` (inline price): RIVAL
- **Status:** DRIFTED
- **Differences:**
  - **Pricing castle.** HUMAN uses `mc2_price_castle()` = the REGISTER, behind
    `MGC_NO_MC2_SPELL_PRICE_REGISTER` (mc2l12 43127). RIVAL uses the `rival_castle(own)` pool SCAN, plus a
    `fallback` castle behind `MGC_NO_MC2_RIVAL_LADDER_PRICE_DYING_CASTLE`. The round-136 register law never
    reached the rival column.
  - **Cheat flag.** HUMAN applies `dev_spells || mc2_free_spells` (retail's own recorded cheat flag) and
    zeroes `f136`, `f140 = 1`. RIVAL never applies it, although retail's flag is global inside SetSpell.
  - **Recast surcharge.** HUMAN adds +3000 (`MGC_NO_MC2_RECAST_SURCHARGE`). RIVAL is provably 0 (only the
    human's demolish writes the latch). Legitimate.
- **What reaches each arm in-game:**
  - Every tier change, level-up and castle ladder step, for human and rivals.
  - The scan-vs-register split needs a rival with two castles, or a rival castle at level 0 that is still
    standing after its register cleared.
  - The cheat difference needs a recording made with retail's free-spells cheat on.
- **Already witnessed?:**
  - HUMAN register: yes (mc2l12).
  - RIVAL tier multiply: yes (mc2l22 "THE CASTLE SPELL'S READINESS GATE SPENDS THE TOKEN'S STAMPED PRICE").
  - Rival scan-vs-register: no evidence.
  - Cheat on a rival: no evidence.
- **Recording ask:**
  1. If feasible: MC2 with retail's spell cheat enabled, on a level with rivals. Let rivals cast castle-upkeep
     spells, about 10 min.
  2. A rival castle split is hard to force (rivals rarely hold two castles). Skip unless it happens naturally.
- **Confidence / notes:** Medium. Whether retail's rival SetSpell honours the global cheat flag follows from
  the flag being global in L:1531. Not EXE-verified.

### CAST-19: MC2 cast gate / arm `sub_5F660` → `sub_5F7B0`
- **Retail routine(s):** sub_5F660 (EF:60874-967): a per-model re-arm/retrigger switch, then the mana gate,
  then the arm `sub_5F7B0` (`+2E = +30`, hand bits, `sub_5F7E0`). It is called by the human command pass
  and by the AI executor `sub_14E10`. The switch itself branches on the caster model: the retrigger family
  4/6/8/0xB/0xC/0xE refuses for a rival caster and extends for the human.
- **Port arms:**
  - `mc2/cast.rs:3232 mc2_cast_gate` (+ `:3327 mc2_stamp_hand`, `:3343 mc2_arm_invis_break`): HUMAN
  - `mc2/rivals.rs:8265 mc2_rival_cast`, the executor window-gate block (≈:8370-8395) plus the arm
    (≈:8480-8560): RIVAL
- **Status:** STRUCTURALLY-DIFFERENT (part of the difference is retail's own caster-model branch)
- **Differences:**
  - **Retrigger family (4/6/8/0xB/0xC/0xE).** HUMAN extends (`f26 = 1`, or 7 for metamorph). RIVAL refuses.
    Retail itself branches here.
  - **Cave-In (25) cave-only refusal.** HUMAN has it. RIVAL has none (rivals do not cast 25).
  - **Possess (1) re-press.** HUMAN raises the release signal with no mana gate. RIVAL: not modelled (the
    rival arm is reached only through readiness).
  - **Castle (2) armed buzz.** HUMAN buzzes 29. RIVAL takes a separate route (`mc2_rival_cast_castle`).
  - **Mana gate.** HUMAN checks `mana < token.max_life` → buzz. RIVAL checks readiness `mc2_rival_cast_ready`
    (purse and ceiling), which is a different routine (`sub_15170`).
  - **Cloak break.** HUMAN applies the strength-gated break AND zeroes the invis window (see CAST-15). RIVAL
    applies the strength-gated 0x20 clear only (`MGC_NO_RIVAL_INVIS_CLOAK_EDGE` restores always-clear).
  - **Hand stamp.** HUMAN writes `hand_bits`. RIVAL writes none (a3 = 0 for AI).
- **What reaches each arm in-game:**
  - HUMAN: every MC2 cast press.
  - RIVAL: every AI cast.
- **Already witnessed?:**
  - HUMAN: yes (mc2l22 t=3424/t=63318 command word/ring; mc2l6-rsg t=1347 duel retrigger).
  - RIVAL: yes (mc2l22 t=2513 "THE EXECUTOR HAS ITS OWN WINDOW GATE").
- **Recording ask:** Covered by CAST-15's invisibility break ask. There is nothing else to record.
- **Confidence / notes:** High. The retrigger split is retail's own branch, not port drift.

### CAST-20: MC2 first-tick emission (`sub_6DCA0` band + direct arms)
- **Retail routine(s):** the fire blocks of sub_693F0 / sub_69640 / sub_6A5C0 … feeding sub_6DCA0 (the
  class-9 spawn). They are caster-generic.
- **Port arms:**
  - `mc2/cast.rs:4706 mc2_spell_fire` + `:5393 mc2_launch` + `:5050 mc2_possess_launch`: HUMAN
  - `mc2/rivals.rs:≈9170 mc2_rival_emit`: RIVAL
- **Status:** DRIFTED (switch coverage)
- **Differences:**
  - **Shared reach table.** Both use `mc2_launch_axis_reach` (shared).
  - **Band extension.** HUMAN has an extra `MGC_NO_MC2_LAUNCH_AXIS_BAND` (spells 0/7/9/13). RIVAL has its own
    `MGC_NO_RIVAL_LAUNCH_AXIS`, and HUMAN has `MGC_NO_MC2_LAUNCH_AXIS`.
  - **2a absence.** `MGC_NO_RIVAL_LAUNCH_2A_ABSENCE` (rival) vs `…LAUNCH_2A_ABSENCE` (human): separate switches.
  - **Mine carrier.** `no_mine_carrier_tier_byte` and `no_mc2_mine` exist on the human only.
    `mc2_rival_emit` does have a 0x17 → subtype 29 arm, so a rival Magic Mine would take the other path.
    No rival pick list I read offers 0x17, so this may be latent.
  - **Possess tier table.** RIVAL behind `MGC_NO_RIVAL_POSSESS_TIER`. HUMAN has it natively.
  - **Launch speed.** HUMAN uses `mc2_caster_act_speed`, behind `MGC_NO_MC2_SPEED_TOKEN_LIVE_ACTSPEED`.
    RIVAL uses the raw `f126`.
- **What reaches each arm in-game:**
  - HUMAN: each player projectile cast.
  - RIVAL: each rival projectile cast.
  - The mine difference needs a rival casting spell 0x17 (Magic Mine), if any rival book holds it.
- **Already witnessed?:** both extensively (mc2l6, mc2l22 fixtures on `(9,x) dest`, 2a, possess tier).
- **Recording ask:** Only if some MC2 level gives rivals Magic Mine (0x17): let a rival lay mines, about
  10 min. Otherwise none.
- **Confidence / notes:** Overlaps PROJECTILE. The mine-carrier tier byte on the rival arm is unverified.

---

## Summary table

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| CAST-1 | MC1 token gate sub_55DD0 | DRIFTED | human ×5 copies / rival ×3 copies | y (human dies mid-burst; blue jar) |
| CAST-2 | MC1 launcher skeleton sub_56090 | DRIFTED | human / rival | y (rival mirror-purse leg, close duel) |
| CAST-3 | MC1 Heal sub_56270 | DRIFTED (small) | human / rival | y (human heal has no fixture) |
| CAST-4 | MC1 Shield sub_566C0 | DRIFTED | human-strict / human-native / rival | n (native arm is unreachable by replay; code fix) |
| CAST-5 | MC1 Rebound sub_573F0 | DRIFTED (switch coverage) | human / rival | n |
| CAST-6 | MC1 Invisible sub_571B0 | STRUCTURALLY-DIFFERENT | human (command + pin-only token) / rival | y |
| CAST-7 | MC1 Speed sub_56380/57F00 | DRIFTED | human / rival | y (human dies holding Accelerate) |
| CAST-8 | MC1 Create-Castle token sub_57610 | DRIFTED | human (token + cast_castle) / rival | n (optional upgrade take) |
| CAST-9 | MC1 token emission | DRIFTED (switch coverage) | human emitters / rival_emit | n |
| CAST-10 | MC1 book mint / respawn re-grant | DRIFTED | human / rival | n (low; blue-jar rival) |
| CAST-11 | MC2 afford sub_68D50 | DRIFTED (small) | human / rival / inline | y (upkeep + castle split, fiddly) |
| CAST-12 | MC2 manifestation skeleton | DRIFTED | human / rival stand-in / rival token_tick / fire_at_slot / cast pre-consume | y (respawned rivals fighting) |
| CAST-13 | MC2 Heal sub_6A300 | DRIFTED | human / rival (2 callers) | y (low; respawn-book heal) |
| CAST-14 | MC2 Shield/Rebound publish | DRIFTED | human / rival | y (human rebound expiry; rival precise tier) |
| CAST-15 | MC2 Invisibility + cloak break | DRIFTED | human / rival | y |
| CAST-16 | MC2 Speed GetScroll_69DB0 | DRIFTED | human / rival | n |
| CAST-17 | MC2 Castle token sub_69AB0 / sub_5F890 | DRIFTED | human / rival | y (tier change mid-window; castle split) |
| CAST-18 | MC2 SetSpell / GetSpellManaCost | DRIFTED | human / rival | y (cheat-on take, if feasible) |
| CAST-19 | MC2 cast gate sub_5F660 | STRUCTURALLY-DIFFERENT (partly retail) | human / rival | n (covered by CAST-15) |
| CAST-20 | MC2 first-tick emission | DRIFTED (switch coverage) | human / rival | n (unless rivals own Magic Mine) |

## Clusters seen that belong to another subsystem
- **DEATH:**
  - MC1 respawn re-grant `death_regrant` (world.rs:12050) vs the `rival_respawn` re-grant loop (mc1/rivals.rs:6543). This is sub_44D30 and is summarised in CAST-10.
  - MC2 rival corpse token collapse `mc2_rival_corpse_tokens` (mc2/rivals.rs:3076), a third rival body arm (the "death-collapse walk") duplicating the rebound/shield clear split.
- **CASTLE:**
  - The `mc2_castle_lock_stamp` owner-generic pin (cast.rs:4253) vs `mc2_rival_castle_lock_release`.
  - MC1 `cast_castle` is shared by the MC1 human and the MC2 human, while each game's rival has its own mint (CAST-8 / CAST-17).
  - The MC2 castle ladder re-price `mc2_drain_ladder_sync` vs the rival ladder (`MGC_NO_MC2_RIVAL_LADDER_PRICE_DYING_CASTLE`).
- **PROJECTILE:**
  - CAST-9 / CAST-20 emission halves.
  - The rebound deflection quarter debit (mc1hwl0 t=38739), which touches both human and rival purses.
- **AI:**
  - `rival_rebound_roll` / `mc1_invis_notice` read the human's or the rival's token `+48` via two different register paths (`player.owned[14]` vs `r.owned[14]`).
  - `rival_invis_window_off` in the MC2 picks.
- **MOBS:** `mc2_rival_leech_apply` carries an inline human branch (already one routine; noted only).
- **Suspected native-only hole (MC2, CAST-12):** a respawned MC2 rival's below-wizard book gets no token body
  in native play. Needs a native run to confirm. Replay cannot see it.
