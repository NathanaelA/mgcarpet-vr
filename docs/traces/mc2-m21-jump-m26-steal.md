# m21 devil JUMP physics + m26 wraith SPELL-STEAL — verbatim traces (2026-07-17)

**ALL FOUR ITEMS LANDED 2026-07-18** (m22 castle wiring, m17 verbatim
dive-step, m26 steal round trip incl. detach arc + hand-hint
re-pickup, m21 frog-jump cycle). Opus re-extraction corrections folded
into the port (the tables below stand EXCEPT):

- **m21 state 7 is NOT settle-pinned** — v12 stays 0 (the fall
  integrator keeps running); only XY is frozen. Settle starts at
  state 8.
- **m21 ctor draws ONE entity-LCG draw** (EF:34353) feeding
  yaw/roll/pitch together — not one per angle. Ctor also zeroes
  `word_0x2C_44` (the old port misfiled `subSpellIndex_0x2A_42 = 400`
  into f44 — that field has no reader; the bolt thunk hard-sets 500).
  This ctor fix alone moved the mc2_cave checkpoint-C golden
  (level-014's m21s materialize stage-held — ctor state is their only
  hash contribution; re-pinned with bisect proof).
- **m21 state 9 draw count is CONDITIONAL**: cackle draw always; the
  rest draw only when `byte_0x43_67 != 0` (attack mode = one draw,
  rest 1 tick, no division).
- **m26: the `word_0x36_54 == 0` re-steal lock is checked INSIDE
  `sub_69300`**, after the %63 draw — the roll is spent even when
  locked (RNG parity preserved by the port's mail keying).
- **m26 detach (counter 0-5) anchors to the PLAYER**, 384 ahead of
  the player's aim at pitch `playerPitch − 16·n`; only the homing
  target (counter ≥ 6) is 384 ahead of the wraith (speed 32·(n−5)).
- **`word_0x4A_74` (hand hint, 1=R 2=L) and the lock live ON the
  jar entity.** Port homes: f38 = wraith, f26 = arc counter, f36 =
  hint, f54 = lock (the existing cast-tick cooldown decrement serves
  as the countdown). Both-hands edge: independent ifs clear both,
  the LEFT hint wins — covered by the round-trip unit test
  (`mc2_wraith_spell_steal_round_trip`).
- `byte[3] |= 2` (dropped-jar marker) and `byte[0] & 1` (in-hand
  tint) are write-only/presentation-side — unmodeled, documented in
  place.

**BANKED ITEM 3 — m22 mana-worm castle deposit (player report 2026-07-17,
smallest of the set: PURE WIRING, no research needed).** Retail law is
already verbatim in docs/traces/mc2-m22-worm-helpers.md §10: 0xB2
castle-acquire resolves the target player's `CastleEntityIndex_0x3A_58`,
banks toward the castle, and within 256 units on an aligned frame — IF
`worm.mana + castle.mana < castle.maxMana` — arms 0xB3 deposit (128-tick
cycles, tail shrinks by 2 per cycle, at length 1 dump mana capped at
maxMana + head despawn); castle FULL → LABEL_17 revert = drop owner
(player-observed "loses the color if the castle can't accept mana").
PORT BUG: `m22_target_castle` (multipart.rs:1059) is a hardcoded `None`
stub from the pre-castle era ("no MC2 level spawns a castle today" —
stale since the castle column landed 2026-07-11), so every worm takes
the castle-less revert and can never deposit. FIX: resolve the castle
like roster.rs's `mc2_castle_of` (class 3 model 2 keyed on the owner's
id24) and delete the stub comment; the 0xB2/0xB3 machines are already
ported to the trace. Player-visible contract: worm parks over the
castle, visibly shrinks in steps, castle mana rises, head vanishes.

**BONUS SIDE-FINDING (same family, bank with this work): m17's dive
z-curve diverges from the verbatim.** The port's `m17_dive_step`
(roster.rs — module-doc APPROX "reconstructed from the trace's shape
description") gives 8 rising ticks (+192..+1, gentle tail) then a
slow-starting 8-tick fall (−1..−192). Retail (EF:15726-44, verbatim in
this trace's agent report §3 of the m25 investigation):
`v14 = counter<=4 ? (192>>counter) : −(192>>(4−(counter−4)))`, clamp
−192 — i.e. **5 rising ticks (+192,+96,+48,+24,+12) then a SHARP fall
(−24,−48,−96,−192,−192…)**. The retail manticore leap is quicker and
snappier than ours. Fix `m17_dive_step` to the verbatim formula when
implementing the m21 cycle (cave goldens will move — level-014 has 21
manticores; DELIBERATE BEHAVIORAL re-pin).

BANKED FOR NEXT SESSION — retail-certified symptoms (player, mc2:08 basin,
2026-07-17), verbatim decompile specs (opus agent, same day). NOT yet
implemented. All cites EF = remc2 EventsFunctions.cpp.

**Identity correction:** the "devil" is **m21** (behavior row 96, sprites
305-312, trace mc2-class5-m10-21-23-24.md), NOT m25 — the earlier m25
identification (2026-07-17, from the 313/314 water pair) was wrong; m25 is
the castle-gnawing splitter. mc2:08's start basin authors 22× m21 + 5× m26.

Player-certified retail behavior: on LAND the devil frog-jumps (half-
sinusoid arc, lands, stands 1-2 s, jumps again — never walks); in WATER it
wades as a continuous walker, waist-deep. The zombie/wraith (m26) at close
range PULLS THE EQUIPPED SPELL JAR out of the player's hands; the jar drops
to the ground and can be re-picked.

---

## A. m21 jump cycle (`sub_265A0` EF:17010) — the whole trick

`byte_0x46_70` = the jump-cycle state; two frame-locals: `v12` = settle
(z −= 42 this tick), `v13` = moved (when CLEAR → `byte[1] |= 8`, and
`sub_1B8C0` EF:8786-91 consumes that flag and MOVES NOTHING that tick).
The walker is called every tick by idle (EF:16779) and attack (EF:16895)
handlers BEFORE `sub_265A0` — one-tick flag lag is authentic.

| state | phase | sprite | z/motion law |
|---|---|---|---|
| 0/1 | landed REST | 311/308 | v12=1,v13=0 (pinned); `byte_0x44_68--`; 0 → state 2 (EF:17030-39) |
| 2 | crouch | 308 | pinned (EF:17043) |
| 3 | launch | 308 | AIRBORNE (v13=1 → walker moves) |
| 4 | impulse seed | 309 | entity-RNG draw: `word_0x2C_44 = rand%100 + 140` → state 5 (EF:17048-53) |
| 5 | rise → apex | 310 | `z += imp; imp -= 42`; imp < 0 → state 6 (EF:17054-57) |
| 6 | fall | 305 | same integrator; `z − terrain < 230` → state 7 (EF:17058-62) |
| 7/8 | pre-land/settle | 306/307 | pinned, v12=1 (EF:17065-70) |
| 9 | landing | 308 | pinned; entity-RNG: cackle **sound 42 @ rand%0xB==0**; rest seed `byte_0x44_68 = rand % byte_0x43_67` (idle 64 → 0-63 ticks; attack byte_0x43_67=0 → rest 1 tick), land state = parity (EF:17072-91) |
| 0xA | WATER WADE | **312** | v12=1 ONLY, v13 stays 1 → **continuous walk**; enter on grounded water tile + spawn (10,5) splash; leave water/lift → state 0 (EF:17092-94, 17121-46) |

- z integrator (EF:17098-17110): settle `z -= 42`; else `z += word_0x2C_44;
  word_0x2C_44 -= 42`; TERRAIN FLOOR clamp `z = max(z, terrainAlt)`.
- Cave ceiling (EF:17111-20): `z > ceil − fov` → `imp = 0; z = ceil − fov`.
- Speeds (actSpeed, EF:17121-46 tail): land idle 60 / land attack 96 /
  wade idle 40 / wade attack 66.
- Yaw only changes while LANDED: `sub_26930` (EF:17234) true only at state
  ≥ 9 (or wading @ `!(byte_0x3E_62 & 7)`) — gates the wander RNG
  (EF:16782) and target-facing (EF:16869). Direction commits at landing.
- Attack (action 170, `sub_26220` EF:16838): jump cycle persists (1-tick
  rests = near-continuous hops); bolt (class-9 sub-0, subSpell 500, via
  `sub_1CC20` EF:9680) fires when in row-96 range on the 1-in-32 instance
  stagger (`byte_0x3E_62 & 0x1F`).
- RNG parity: ctor draws (EF:34353-57) + state-4 impulse + state-9
  sound+rest all on the ENTITY LCG (9377·r+9439) — keep exact draw order.
- PORT GAP: our m21 runs `mc2_move_core` unconditionally (continuous
  glide) + a summary-level hover — replace with the cycle above; the skip
  flag mechanism must veto XY in states 0,1,2,7,8,9.

## B. m26 spell-steal (`sub_69300` EF:55792) + dropped jar

Attack brain `sub_28FF0` (action 210, EF:19233):
- Every tick vs class-3 model 0/1 target: mana drain
  `mana −= (manaRegen + 14)`, clamp ≥ 0 (EF:19331-34). Sound 62 on the
  1-in-32 stagger (EF:19254-55).
- STEAL path gates (EF:19310-75): instance is a "stealer"
  (`byte_0x3E_62 & 3 == 0` — a fixed 1-in-4 partition; others only
  drain), target model 0 (HUMAN only), alive, dist ≤ row-99 range AND
  < 2048. Then exactly ONE **GLOBAL**-LCG draw (EF:19346-47):
  `v12 = (rand_0x8 := 9377·r+9439) % 63` — **4 = steal RIGHT hand,
  5 = steal LEFT hand, else nothing**. Hand empty (−1) or slot 0 → abort.
  ⚠ our port already consumes this draw (RNG parity) — key the effect off
  THAT SAME draw, never a second one.
- `sub_69300` (EF:55792-826), guarded on `word_0x36_54 == 0` (the 64-tick
  re-steal lock): spell entity → `actionIndex = 78`, `byte[0] &= ~1`,
  `word_0x26_38 = wraith`, `dword_0x10_16 = 0`, positioned ON the player;
  player book: `SpellEnabled[model] = 0` (unlearned!), equipped hand(s)
  `SpellIndexLeft/Right = −1`, `word_0x4A_74` = hand hint (1=R, 2=L).
  Per-spell TIER array (`array_0x437`) NOT touched → XP kept.
- Action 78 = shared table slot `strF0[0x4E]` → `sub_692C0` (EF:55774-89;
  dispatch proof EF:2028): runs `sub_59DC0` (EF:41199-252) — 6-tick detach
  arc off the hands, then homes toward the WRAITH at `32·(counter−5)`/tick
  to a point 384 ahead, drops to terrain (`< terrainAlt+64` → snap) — then
  flips to the ordinary ground-jar pickup state `3M+1` (+ byte[3] |= 2).
- Re-pickup (`sub_68FF0` EF:55676-760): sound 18, re-learn
  (`SpellEnabled[model]` restored, learned flag set), re-equip from
  `word_0x4A_74`, `word_0x36_54 = 64` re-steal lock,
  `SetSpell_6D5E0(jar, array_0x437[model])` → SAME TIER/XP as before.
- PORT GAP: the roll is drawn, the effect is skipped (the old "pending
  class-15 column" APPROX — the column has since landed). Implement steps
  1-4 of the checklist in the agent report; the human book fields map to
  `mc2_book.ent/left/right`; the 64-tick lock and the hand hint need
  homes on the manifestation entity (f54 candidate for the lock — check
  collision with the cooldown use).
