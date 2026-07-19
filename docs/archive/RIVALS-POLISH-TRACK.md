# Rivals-polish track — kickoff worklist (banked 2026-07-14)

Player-spotted issues while playtesting MC2 levels with rivals, traced + banked at
the end of the notification/music session so the next session hops straight in.
Four issues; **2 are real code fixes, 1 is faithful (playtest-only), 1 is
cosmetic/optional.** All decompile findings are from three Opus traces (cited
inline). Retail cites = `reference/remc2/remc2/engine/EventsFunctions.cpp` (EF).

Priority order for the next session: **#1 (trivial) → #2 (the meaty, high-value
fix) → #3 (cosmetic decision) → #4 (playtest, maybe no code)**.

---

## #1 — Wrong rival death name ("Vodoor/Vodor death") — REAL BUG, trivial fix

**Symptom:** killing Nyphur (MC2 level 01) prints the wrong name to the console.

**Root cause (VERIFIED):** the death broadcast hardcodes the **MC1** name table for
every game. Nyphur = MC2 wizard **slot 1**; `mc1::rivals::RIVAL_NAMES[1]` = **"Vodor"**.
Slot 0 is "Zanzamar" in both tables, which is why only non-zero slots show the bug.
It is NOT a creature-name issue — MC2 death messages are wizard-only and never touch
a monster-name table.

- Bug site: `crates/mgc-app/src/main.rs:1152` (`sync_world`) —
  `mgc_sim::mc1::rivals::RIVAL_NAMES` used unconditionally.
- `take_rival_deaths()` returns bare slots for BOTH games (`rival_deaths: Vec<u8>`,
  pushed by MC2 at `mc2/rivals.rs:2242` and MC1 at `world.rs:595`) — the consumer has
  no game context.
- The correct MC2 table already exists and is decompile-verified:
  `mc2::rivals::MC2_RIVAL_NAMES` (= retail `WizardsNames_D93A0`, GameUI.h:39;
  `GetTrueWizardNumber` is identity in single-player). Slot→name:
  0 Zanzamar, 1 **Nyphur**, 2 Rahn, 3 Belix, 4 Jark, 5 Elyssia, 6 Yragore, 7 Prish.
- The codebase already branches correctly elsewhere: `world.rs:3646` (MC1) vs
  `world.rs:3666` (MC2 `MC2_RIVAL_NAMES`) for the map labels — only the console log
  was never updated.

**Fix:** in `sync_world` select the table by `self.game` (already available, used at
main.rs:86). ~5 lines, no bake needed. Optional later polish: match retail wording
(MC1 lang 54 "%name% is dead" / MC2 lang 374 "has died.") — that's the
notification/messaging surface, and now that the toast surface exists, these death
events are a natural first customer of it (route the death broadcast through
`World::set_notification` instead of `eprintln!`).

---

## #2 — Dead wizard's corpse can't be possessed / mana never claimed — REAL BUG, the main fix

**Symptom:** when a wizard (rival or player) dies, the corpse that carries their
loose (uncollected) mana can't be hit or aimed at by Possession, and it stays forever.

**How it works in retail (VERIFIED):** there are **TWO separate entities** at the
death site, grabbed by two different mechanics —
1. **The GRAVE `(10, model 40)`** = the corpse; carries the dead wizard's MANA.
2. **N SPELL JARS `(15, spell)`** = one per held spell (see #3).

The grave (ctor `sub_501D0` EF:36659: `f28/byte_0x38=2`, targetable bit kept,
action `0x2A`=42) is raised by the death handler `sub_5E310` at EF:60164, which then
re-points every `(10,39)` mana sphere owned by the dead wizard to the grave
(EF:60173-77) — the mana becomes "unclaimed" (owned by the grave). The grave's own
action tick **`sub_36AE0` (EF:26835)** reads the possession claim channel; when a
class-3 wizard claims it, it reassigns **every entity the grave owns** to the
claimant, then despawns (EF:26847-58). So possessing the grave transfers the mana.

Two faithful behaviors that are NOT bugs, worth knowing:
- **The grave persists indefinitely** — `sub_36AE0` has no life decrement. It only
  vanishes when possessed. So "corpse remains forever" is correct.
- **Auto-aim does NOT lock graves** — `sub_67CB0` case 1 accepts only models 39 and 57,
  explicitly skipping 40. You must MANUALLY aim the possess bolt into the grave (the
  flight probe `sub_108B0` accepts models 39 AND 40, so a bolt flown into it detonates).
  So "possession doesn't aim toward the corpse" is *partly faithful* — the crosshair
  magnetism isn't meant to grab graves.

**The real bug = our grave is inert.** `mc2_spawn_grave` (`mc2/rivals.rs:2385`) diverges
from the WORKING MC1 grave (`features.rs:3519 spawn_grave`) in three ways + a dispatch
shadow:

| | retail / MC1 `spawn_grave` | MC2 `mc2_spawn_grave` |
|---|---|---|
| targetable bit 8 | kept | **`e.flags &= !8`** (cleared → bolt flies through) |
| possess channel | `f28 = 2` | **absent** (f28=0 → claim mail dropped) |
| action | `tick70 = 42` → `grave_tick` | **`tick70 = 0x29`** + a no-op dispatch arm |

- `e.flags &= !8` (rivals.rs:2392) fails the `possess_victim_at` targetable gate
  (combat.rs:1081) — the bolt passes through.
- No `f28 = 2` → `possess_flash_tick`'s ch1 `area_write` drops the claim mail (same
  root cause as the already-fixed MC2-building possession bug,
  `mc2-possession-delivery.md` §7).
- The grave dispatches to a **no-op** (`10 if Mc2 && model65 == 40 => {}`,
  `world.rs:1642`) whose comment wrongly calls the retail action "untraced no-op,
  provenance OPEN." It is `sub_36AE0`, the mana transfer — and we ALREADY have a
  byte-exact port of it: **`grave_tick` (`features.rs:3540`)**, just never reached in MC2.

**Fix (concrete, low-risk — reuse the MC1 machinery):**
1. In `mc2_spawn_grave`: `e.tick70 = 42;` (not 0x29), `e.f28 = 2;`, **delete**
   `e.flags &= !8;` (keep bit 8), `e.f26 = (s % 11) as i16;` (cosmetic, matches MC1).
2. In `world.rs`: **delete the `10 if Mc2 && model65 == 40 => {}` no-op arm** (line 1642)
   so a `tick70==42` grave falls through to the shared `effect_tick` → `grave_tick`.
   Verified safe: grave is the only class-10 model-40 in MC2, and action 42 collides
   with no other MC2 class-10 arm. `grave_tick` already does transfer + despawn — no
   new logic.
3. Re-pin the MC2 state-hash goldens (grave's f28+flags now differ; MC1 untouched).

Optional exact-faithfulness: keep model 40 OUT of the possess auto-aim candidate set
(retail `sub_67CB0` excludes it) — already the case since the fix doesn't set `f58`.

---

## #3 — Uncollectable owned-spell jars — INTENTIONAL UNFAITHFUL IMPROVEMENT (player-directed, both games)

**Scope (player-clarified 2026-07-14): this is NOT just death-scattered jars — it's
ALL jars, chiefly the AUTHORED/PLACED ones across the level.** Example: level 01 has a
placed **speed-spell** jar, but speed is already collectable in level 00; if you took
it there, the level-01 jar is permanent dead weight you can never pick up. The authors
scattered redundant spell sources as a safety net for players who missed one, but the
result is uncollectable, **unidentifiable** clutter (you can't tell what's in a jar
without our `expose_jar_spells` debug option) — and the player confirms this is
frustrating in BOTH retail MC1 and MC2.

**PLAYER DIRECTIVE — an intentional deviation from retail, flagged as an unfaithful
improvement (authenticity matrix): eliminate any jar whose spell the local player
already owns (and therefore cannot pick up).** "One that I'm sure no one will ever
complain about." Applies to BOTH games' jar systems (each has its own pickup gate).

### The retail baseline (so we know what we're deviating from) — VERIFIED
- Pickup gate `sub_68FF0` (EF:55710-16, MC2): a wizard collects a jar only if it does
  NOT already own that spell. Already-owned jars are flown through, never taken —
  retail leaves them in the world.
- **Authored/placed jars carry `life = 0` → they NEVER decay** (permanent). Only
  DEATH-scattered jars carry `life = rand%0x5A + 200` (200-289 ticks) and self-cull.
  So the persistent-clutter problem is the placed jars; the death-scatter jars already
  disappear on their own (that half is faithful and already correct in our port — see
  below).
- MC1 has the equivalent placed-jar + already-own-gate system (its own pickup path).

### The improvement to build (next session)
When the (local/human) player already owns spell `s`, remove every jar of `s` from the
world — both at level load (sweep placed jars for already-owned spells) and dynamically
the instant the player gains `s` (sweep the level for `s` jars). Criterion = the exact
pickup gate ("player owns s" = "can't pick it up").
- **Authenticity-matrix option** (P-class enhancement toggle; faithful default = keep
  the jars). Given the player's confidence it's universally wanted, default-on is
  reasonable, but keep it toggleable for purists. Mark clearly as an unfaithful
  improvement in the option registry + docs/FIDELITY.md.
- **Applies to BOTH MC1 and MC2** — wire it into each game's jar tick/pickup path.
- **MP caveat (from the trace):** removing the ENTITY affects all wizards, and in MP a
  *different* wizard may still need spell `s`. So: gate entity-removal on single-player
  (`human owns s`), OR in MP make it PRESENTATION-ONLY (suppress the human's billboard
  for jars it can't take, leaving the entity live for other wizards). SP entity-removal
  is the clean default; decide MP behavior when we get there.
- MC2 jar entities = `(15, spell)` state 3M+1; tick `mc2_spell_token_tick`
  (world.rs:4794), owned-gate `mc2_spell_tokens.0 & (1<<model)` (world.rs:4827). The
  removal sweep hooks here + on grant (`mc2_adopt_manifestation`, `mc2_award`/pickup).
  MC1 jars = its own class/model; find the parallel pickup gate.

### The death-scatter sub-case (the original #3 symptom) — already faithful
Nyphur's death dropping fireball/possess jars: rivals scatter ALL held spells incl.
starters (`mc2_rival_death_impact` rivals.rs:2243-68, faithful EF:60137), but those
carry the 200-289 tick life and SELF-CULL (`mc2_spell_token_tick` despawns at 1,
world.rs:4803); the already-owned gate blocks pickup (world.rs:4827). So the jars the
player saw already expire in seconds — brief cosmetic litter, not persistent. The
improvement above (remove owned-spell jars) subsumes this too: an owned-spell death
jar would just be removed immediately instead of lingering its few seconds.

**Related gap (Phase 4.6, separate):** the HUMAN's MC2 death does NOT scatter its
`mc2_book` — `mc2_scatter_spells` (`cast.rs:1325`) is `#[allow(dead_code)]` and
uncalled; the human dies via `player_land` (world.rs:2054), which scatters the empty
MC1 `player.owned` list. Wiring `mc2_scatter_spells` into the MC2 human-death path
(+ re-mint from a `known` mask on respawn) is the banked "4.6 corpse/economy pass."

---

## #4 — Nyphur "flies around like an idiot and dies" in level 01 — FAITHFUL, playtest only

**Verdict (VERIFIED): deliberate, data-driven, correctly ported. No code fix.**

Level-001 `WizardMapSettings` gimps Nyphur (slot 1) on purpose:
`aggression 243, reflexes 39, perception 90, life 79` (≈0.31× → maxLife ~3100 vs
~10000), `starting_spells = {0,1,3}` but `blocked_spells = {2..25}` — so with the
`start && !blocked` grant rule his effective book is **just {fireball, possess}**;
spell 2 (castle) is blocked AND `players[1]=0` (no authored castle seed).

The retail brain's fallback for a castle-less, attack-poor, unprovoked wizard IS
cruise-and-die (selector cascade `sub_12E70` EF:5495; idle fallback `sub_14630`
EF:6383 → CRUISE state 12; flee `sub_13DC0` EF:6163 returns 0 without a castle, so a
hurt castle-less wizard can never retreat/heal). Our `mc2_rival_selector`
(rivals.rs:1072-1145) mirrors this exactly. Every OTHER rival slot carries the castle
spell + no block list — Nyphur is the author-designated trivial tutorial punching-bag.
The player's hypothesis is exactly right.

**Playtest to confirm (not code):**
- (a) level-001 spawns pickable mana balls near Nyphur's start, so he engages the
  possess/mana loop and "does something" before dying.
- (b) our APPROX hate-feed doesn't make retaliation feel noticeably late.

**Optional low-priority polish (ONLY if playtest flags passivity):**
- Move the hate feed from damage-intake to the per-projectile scan `sub_159E0`
  (retail builds hate when merely *shot at*; ours only when *hit* — flagged in the
  `rivals.rs:35-40` header). Ours retaliates slightly later → marginally more passive.
- Restore the inline possess cast in the Cruise handler `sub_13270` (EF:5680); ours
  (rivals.rs:1679-81) only sets `vdes`. Harmless on level-01 (possess still covered by
  selector step 8, Nyphur owns no scroll), so low priority.

---

## Entity/handler quick reference (for #2/#3)

| entity | class/model | action | retail | re-impl |
|---|---|---|---|---|
| grave (corpse) | 10 / 40 | 42 | `sub_501D0` / `sub_36AE0` EF:26835 | `mc2_spawn_grave` rivals.rs:2385 (BROKEN); `grave_tick` features.rs:3540 (correct, unreached) |
| loose mana sphere | 10 / 39 | 0x29 | re-pointed EF:60173 | rivals.rs:2278 |
| claim pulse | 10 / 12 | 0x0C | `sub_112D0` EF:4162 | `possess_flash_tick` combat.rs:2123 |
| spell jar | 15 / spell | 3M+1 | scatter EF:60137 / pickup `sub_68FF0` EF:55676 | `mc2_spell_token_tick` world.rs:4794 (correct) |
| possess bolt probe | — | — | `sub_108B0` EF:3839 (m39,m40) | `possess_victim_at` combat.rs:1066 |
| possess auto-aim | — | — | `sub_67CB0` EF:55015 (m39,57 — NOT 40) | `aim_assist_possess` combat.rs:562 |

## OPEN / verify at implementation
- After the #2 grave fix, re-pin MC2 state-hash goldens (mc2_cave + mc2_slice); MC1 untouched.
- Human MC2 death scatter + respawn re-mint (`mc2_scatter_spells` wiring) is the separate Phase-4.6 corpse pass.
- retail `sub_36AE0` uses `DisableEntityDrawing04` (draw-off) where MC1 `grave_tick` uses `free_entity` — benign, but note if any census pass expects the husk to linger.
