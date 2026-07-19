# Duel (spell 14) — effect spec + port notes (2026-07-17)

Player report: "duel does nothing, all 3 tiers" (tested WITH a rival
nearby — an owed playtest). Root cause: the effect body was the
`note_misfit` stub from the Phase-4.2 deferral; only the cast gate +
mana drain ran. Decompile spec traced 2026-07-17 (opus agent, verbatim
citations); implemented same day.

## Retail machinery (remc2, EF = EventsFunctions.cpp)

- **Cast** (`sub_6B610` EF:57258): first live tick of the class-15
  model-14 manifestation spawns a **(10,26) DUEL TETHER** at the caster
  — life 8, sprite row 284, `+44 = 200`, owner + TIER stamped
  (EF:57297-57316), cast sound 9. Abort arm (EF:57280): 28 ticks into
  the window with NO lock → charge collapses to 1 (fizzle).
- **Grip** (victim-side resolve `sub_5EFA0` EF:60643-63): a gripped
  RIVAL WIZARD sets the caster's LOCK — `word_0x146_326` = opponent,
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

## APPROX register

- The retail tether's own grip-write instruction is not isolable in
  the symbolic decompile (the only symbolic writer `sub_38D80` is a
  creature-pull spell sharing the grip fields). The port grips the
  nearest live rival wizard within the TIER'S ENFORCEMENT RANGE of
  the tether — a farther grip would dissolve on the next enforcement
  pass, so the observable law matches.
- The victim-side one-tick grip-mailbox hop is collapsed onto the
  tether tick (same observable, one tick earlier).
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
