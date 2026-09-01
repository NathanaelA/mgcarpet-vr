# Duel (spell 14) — effect spec + port notes (2026-07-17)

Player report: "duel does nothing, all 3 tiers" (tested WITH a rival
nearby — an owed playtest). Root cause: the effect body was the
`note_misfit` stub from the Phase-4.2 deferral; only the cast gate +
mana drain ran. Decompile spec traced 2026-07-17 (opus agent, verbatim
citations); implemented same day.

## Retail machinery (remc2, EF = EventsFunctions.cpp)

- **Cast** (`sub_6B610` EF:57291): first live tick of the class-15
  model-14 manifestation fires a **(9,7) DUEL DART** — `_4A190(&caster
  .position, 9, 7)`, the `sub_4D740` creator (EF:34898): action 7,
  speed/min_speed 384 with **NO caster-speed boost anywhere in the
  function**, maxLife 21 = `0x2000/384`, behaviour row 60, sprite 213 —
  stamped with the hand muzzle (`sub_68E50`), the token slot
  (`word_0x26_38`), impact pair `byte_0x43_67 = 10 / byte_0x44_68 =
  26`, and the token's `subSpellIndex` / `mana` / `byte_0x46_70`; its
  `axis_0x9A_154x` aim point is the caster's position stepped a flat
  10240 along the launch bearing. Cast sound 9. Abort arm (EF:57277-85):
  `word_0x2E_46 <= word_0x30_48 - 28 && !wizext->word_0x146_326` →
  `LABEL_19`, which sets the charge to 1 **and skips the whole else,
  `sub_68DE0` included** — so the mana-regen clamp does not run on the
  collapse tick.
- **Flight + impact** (`sub_662E0`, EF:63419-63549 — *not* the generic
  `sub_65820`): the resolution arm forks three ways and the middle one,
  `!v5x || v5x->class_0x3F_63 != 3 || (model != 0 && model != 1)`, runs
  `sub_65780(a1x, 0, …)` + disable with **NO effect spawn**. Only a
  struck class-3 model-0/1 **WIZARD** reaches `_4A190(pos, 10, 26)`. A
  duel dart that misses, expires or stops on terrain leaves nothing at
  all.
- **The tether** — ctor `sub_4F720` (EF:36129): class 10, model/action
  26, maxLife 8, `subSpellIndex_0x2A_42 = 200`, sprite **213**,
  `SetEntityShiftRot_49EA0(event, 512, 512)`. `sub_662E0`'s tail then
  overwrites `subSpellIndex` with the dart's payload and copies
  `id_0x1A_26` / yaw / pitch / `word_0x96_150` = the victim /
  `byte_0x46_70` = the tier.
- **The tether's tick** is `sub_33E80` (EF:24985), and it is a CHANNEL-4
  AREA WRITE that renews its own life:

  ```c
  v1 = life; dword_0x10_16++; life = v1 - 1;
  if (v1 < 0) { DisableEntityDrawing04(a1x); return; }
  sub_585A0(a1x);
  if (sub_10C80(a1x, 4u, a1x->byte_0x46_70)) life = 0;
  ```

  `sub_10C80` is the area write; the ch4 pair `word_0x76_118` /
  `word_0x7A_122` at `str_0x5E_94 + 6*4` is `mail[4]` (amount = the
  TIER, source = the marker's OWNER). So the marker is a stationary
  8-tick area effect that lives as long as it keeps catching somebody
  and only starts dying once the victim leaves its box.
- **Grip** (victim-side resolve `sub_5EFA0` EF:60643-63): the ch4 mail
  read off the victim sets the caster's LOCK — `word_0x146_326` = opponent,
  `dword_0x142_322` = dist(caster,victim) clamped **[1024,3072]**,
  `word_0x14A_330` = tier — plus **+1 duel XP** (`sub_6D8B0(…,0xE,1)`
  EF:60657) and victim recoil `word_0x36_54 = 100` (`sub_5EF70`
  EF:60598). Gripped CREATURES take the yank path instead
  (EF:26097/26369) — never a duel.
- **Enforcement** (`sub_5DE30` EF:59889-947), per caster tick while
  locked: break when the manifestation charge dies, the opponent dies,
  or dist ≥ `SPELLS[14].subspell[tier].subSpellIndex_2`; else
  force-fly the caster toward the opponent holding the tether
  distance (speed cap 3·minSpeed/2, EF:59918-29) and DRAIN per the
  tier's `life_0x1A` mode: `1` = mana −(manaRegen+8)/tick, `2` =
  also life −(lifeRegen+2)/tick (EF:59930-43).
- **Tier data** (shipped SPELLS row 14): range/mode = **5170/0**
  (tier 1 = pure leash, NO drain), **7720/1** (mana), **7720/2**
  (mana + life).

## Port (landed 2026-07-17)

- cast.rs `mc2_cast_duel` (the 0xE arm) + the no-grip fizzle in
  `mc2_cast_tick` + the expiry lock-clear in `mc2_cast_expire`.
- world.rs `mc2_duel: Option<(opp, hold, tier)>` (hash tag 0xE2,
  transparent when None), `mc2_duel_tether_tick` (grip),
  `mc2_duel_enforce` (per-tick beside the MC1 duel pull; caster
  force-fly rides the established `player_knock` transport).
- rivals.rs `mc2_duel_drain` (mana via the recomputed regen rate + 8;
  life via the afield /500 rate + 2).
- Test: `mc2_rivals.rs::mc2_duel_locks_drains_and_breaks`.

## ⚠ 2026-09-01 CORRECTION (SESSION 80)

The 2026-07-17 trace above was wrong in three connected places and the
port carried all three until `mc2l6-rival-spells-galore` graded them:

1. **The cast was read as planting the tether at the caster.** It fires
   a projectile; the tether is the projectile's impact. Every duel in
   the port therefore gripped whatever stood next to the CASTER.
2. **`sub_38D80` (EF:28348) was named as the tether tick.** It is a
   roster scanner that writes `word_0x7A_122` = its OWN SLOT and
   decrements life every tick. The real tick is `sub_33E80`. mc2l6-rsg
   slot 827 (t=1375) refutes the old reading directly: `life` holds 0
   for twenty-four ticks with `scratch10` counting 1..26 before falling
   to −1/−2 and freeing at t=1401, and rival 370 takes `mail4.src`
   0 → 343 — the human's entity id, not the marker slot.
3. **The tether ctor was invented** (sprite 284, no shift-rot).
   `sub_4F720` is real and gives sprite 213 + ShiftRot 512/512.

The fizzle predicate also carried an invented `f26 > 1` guard, which
blocked it exactly when the retrigger family (`sub_5F660` case
`{4,6,8,0xB,0xC,0xE}`, EF:60914-27 — a cast press CANCELS one of those
six spells' own live window instead of refusing) had already pinned the
counter to 1.

## APPROX register

- The victim-side one-tick grip-mailbox hop is collapsed onto the
  tether tick (same observable, one tick earlier), and only the HUMAN
  column owns a duel lock register — a rival-cast duel stays unported.
- Life-drain uses the afield /500 life-regen rate (retail reads the
  stored rate, which differs only while the rival sits at its castle).
- The caster force-fly transport is the knock channel (MC1-pull
  precedent), magnitude formula shared, not retail's MoveEntity call.
- Rival-CAST duel (a rival dueling the human) stays unported — the
  rival AI's cast table never picks spell 14 today; note for the
  rival-polish track.

## Player-visible contract (for the re-test)

Tier 1 = a pure LEASH: you get dragged toward the rival, no drain —
"did nothing" at tier 1 without watching your own movement is
expected-retail. Tiers 2/3 drain the rival's mana (bar visibly sinks);
tier 3 also their life. Cast sound 9 + a brief tether sprite at the
caster; breaks past ~20/30/30 tiles.
