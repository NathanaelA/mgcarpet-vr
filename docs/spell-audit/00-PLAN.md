# MC2 Spell Fidelity Track — master plan

Synthesis of the 17 read-only verification audits in this directory
(one per note in the player's 2026-07-13 kickoff list). Each row's
detail + citations live in the named `docs/spell-audit/<slug>.md`.
Method reminder: recorded original gameplay is the senior source; the
vendored decompile (`reference/remc2/.../engine/`, `EF:`/`L:` cites)
is the reference. Baked `spells.bin` (SPELLS.DAT) values WIN over the
decompile fallback.

## Progress

**Landed 2026-07-13 (Phase A + most of B; all tested, 165/165 sim
tests green, NO golden re-pin — fixtures don't cast these spells and
the mana clamp is dormant in them):**
- **G2 mana-regen** — mid-burst clamp (`suppress_regen`, both games);
  new test `active_spell_burst_blocks_mana_regen`.
- **G3 XP scrolls** — full 4 XP to every owned spell (no split).
- **Speed (3)** — per-tier factor 2/3/4 (`accel_mc2_factor`).
- **Meteor (9)** — charge fuse (`f71`→maxLife) at `(10,17)`.
- **Tremor (15) / Earthquake (17) / Volcano (18) / Gravity well (20)**
  — impact-route arms in `mc2_proj_impact` (handlers already existed).
- **Castle cost 2B (MC2)** — LADDER top rung 300M + tier ×1.25/×1.5.

**Also landed (Phase B/C, same session, tested, 165/165):**
- **Beyond Sight (12)** — sim `beyond_sight_tier()` + app reveal keyed
  on tier (T0 wizards, T1 through Invisible). T2 creature-reveal owed.
- **Possession (1)** — Mana Magnet (T1) + Mana Lock (T2) now spawn the
  `(10,54)/(10,69)` attract aura (range 15/20) on the possess impact;
  the aura drags/merges mana under the caster. T2 building-**lock**
  bit (forced-claim + `byte[2]&0x20`) still owed (mail-flag wiring).
- **Castle Teleport (10)** — real 3-tier `mc2_cast_teleport`: T0 to
  own castle, T1 save/return toggle, T2 cycle own→rival castles;
  `-448`/(yaw-204) offset, sound 22 on castle-success only. Flight
  speed-zero on resolve owed (minor).

**Deferred with reason:**
- **Metamorph (4)** — the faithful transform (player flies AS the
  spawned class-5 creature, carpet hidden) collides with the human-
  out-of-pool architecture; needs control-routing work, not a data
  fix. Model ladder (19/25/16, +2 Day) is known when we do it.

**Still open in Phase A/B/C:** G4 spell-name import (bake pipeline);
Beyond-sight T2 creature reveal; Fool's mana (19) trap+retaliation;
Steal mana (11); Lightning (5) beam + `(10,38)` storm. Phase D
(magic-mine `(10,78)`, castle towers `(10,79)`); Phase E rival track.

**Player decisions (2026-07-13):** Summon Army (19) HELD pending an
in-game re-test (decompile says quake, not creatures). MC1 castle
cost deferred to the castle track — it has a runtime hack that bumps
the flat cost when a level is built/destroyed (the real gate = can't
afford the higher level); only the MC2 ladder was fixed here.

**Playtest follow-ups LANDED (2026-07-13, next session; tested,
165/165, NO golden re-pin — fixtures don't cast these):**
- **Speed (3) brake-decoupling** — retail MC2 Speed is a FIXED-DURATION
  window with NO brake-cancel (speed.md §5); the port rode MC1's
  held-channel whose brake-cancel stopped the boost early while the MC2
  `f26` window kept counting, pinning regen off. FIX: `thrust_cancel`
  (world.rs) is now a no-op while `accel_mc2_factor != 0` — the MC2
  window runs to completion, so effect and burst (and thus the regen
  block) stay coupled. Test `mc2_speed_window_survives_brake`.
- **Invisibility (11) break-law** — ported `sub_5F7E0` (EF:60987): new
  `player.invis_strength` (= tier `life` {1,2,3}) set on the invis
  first tick, zeroed at expiry; in `mc2_cast_gate` after the arm, if
  `s && (s<2 || (s<=2 && spell!=1))` → clear the cloak flag + strength
  AND zero the invis window's `f26` (functional termination clears the
  burst → regen lifts with the cloak, per the player directive). T0 any
  cast breaks, T1 all-but-possess, T2 nothing. Test
  `mc2_invisibility_break_law_per_tier`.
- **Castle-cost gate (2)** — the gate + first-tick debit read the cached
  `max_life`, stale after a castle level-up (retail re-syncs via the +1
  castle XP on upgrade → SetSpell; our upgrade lives in `Gen`, off the
  book). FIX: `mc2_cast_gate` re-runs `mc2_set_spell(m, sel[2])` for
  spell 2 before the mana gate, so BOTH gate and debit charge the live
  tier-scaled ladder cost. Test `mc2_castle_cost_gate_tracks_live_level`
  (level-1 live cost 10000; 5000 mana refused). New `set_player_mana`
  debug hook; new test file `tests/mc2_spell_channels.rs`.
- **STILL OWED:** Speed magnitude timing (player will report peak/window
  per tier); the OTHER armed-window channels (heal/shield/rebound/
  beyond-sight) run their full window with no early-break condition in
  retail — audited, no `f26`-decoupling, no change needed.

**Playtest round 2 (player, 2026-07-13, same day — verifications +
one fix LANDED):**
- **Speed (3) — CONFIRMED WORKING.** Durations 10/15/17 s ≈ 301/451/501
  ticks (30 tps), speed progressive (T0 ~0.8× map-wrap, T2 ~2.8×). Item
  CLOSED.
- **Cave-In (25) — sim MEASURED to scale, NOT a flat bug.** Player saw
  "identical all 3 tiers"; a direct measurement (cast T0/T1/T2 in a
  cave, count collapsed cells) shows **ceiling cells = 1283 / 1498 /
  1731** (rings 3/5/7 → box 15/17/19), i.e. a real ~35 % growth per
  tier; the tier charge (`f71 = 0/1/2`) reaches the effect correctly.
  So this is PRESENTATION/perception, not the sim. Floor-raise cells are
  constant (1264) across tiers — possibly faithful (the collapse is
  mostly a ceiling drop) or a minor floor-scaling gap; flagged for a
  visual re-check, no code change made.
- **Summon Army (19) — CONFIRMED broken + re-traced.** Player: boomerang
  hits ground, puffs, nothing; NOT primarily a tremor. Re-trace (agent,
  HIGH confidence) overturns the "quake" verdict: it's a real
  **creature army** — model-72 ≠ action-72 was a conflation. `(9,24)` →
  flight `sub_67800` → ring of `(10,72)` nodes (`sub_51800`) → each
  `sub_3A5B0` spawns a class-5 creature: **T0 8× firefly(19)/bee(2 Day),
  T1 4× Cymmerian(25), T2 2× wyvern(16)**, 250-tick lifespan, allied.
  Same `{19,25,16}` roster as Metamorph. Recipe in
  `summon-creatures.md` (corrected). PORT = an L-sized item, blocked on
  the shared `8·model+7` controlled-creature action (Metamorph's
  dependency) — newly scoped, not yet done.
- **Possession (1) magnet — FIXED (LANDED, tested).** The Mana Magnet /
  Lock aura (`(10,54)/(10,69)`) now spawns ONLY when the possession bolt
  actually CLAIMS a mana sphere (`(10,39)/(10,40)/(10,57)`); an
  empty-space / terrain detonation "evaporates without trace" (no more
  redundant free-floating magnet). Building/worm possession does NOT
  magnet either (player suspects buildings never did — gated to spheres
  pending a retail trace). Test `mc2_possession_magnet_needs_a_mana_claim`.
- **Beyond Sight (12) + Mana Lock (T2 building lock)** — DEFERRED to the
  rival track (need a level with enough enemies to verify).

**SPELLS LANDED 2026-07-13 (session 3; tested 12 binaries green, no
golden re-pin — fixtures don't cast these):**
- **Lightning (7)** — L0 (subtype 9) is now a ONE-TICK hitscan BEAM
  (`mc2_lightning_beam_tick` runs the tested flyer to completion → an
  instant `(10,23)` flash, the authentic RAPID crackle, not the old
  slow-bolt "stream"). L1/L2 (subtype 12) detonate into the `(10,38)`
  STORM burst (new `mc2_spawn_lightning_burst`, reuses the blast23
  one-shot-damage tick; the misfit `(9,9)` is gone). The `(10,38)`→
  second-order `(9,9)` chain is the deferred tail (its class-10
  internals untraced). Tests `mc2_lightning_l0_is_a_one_tick_beam` /
  `mc2_lightning_storm_spawns_the_burst`.
- **Steal Mana (13)** — was a `note_misfit` stub; now a class-9
  subtype-8 homing bolt whose `(10,25)` impact stamps the struck
  wizard's channel-3 "steal" inbox (`mail[3] = (sub_spell, caster)`).
  The ch3 consumers + `credit_wizard_mana` ALREADY existed (rivals.rs
  `mc2_rival_intake`, world.rs `apply_player_damage`) — only the
  emitter was missing. L1/L2 flat drain (2000/4000) is faithful; L3's
  castle-% + `(10,39)` re-emit is the deferred tail (lands as the flat
  10-point fallback). Gated to a direct class-3 model-0/1 hit; AoE
  spread deferred. Test `mc2_steal_mana_casts_a_projectile_not_a_stub`
  (economy not cleanly observable headless — dev masks the pool, rivals
  self-spend). New `set_player_mana`/`set_player_mana_max` debug hooks.
- **Earthquake (17)** — PLAYER-REPORTED entity-pool FLOOD + "explosions
  not a carve": `mc2_fire_trail_tick` was laying the `(10,19)`
  ground-fire SPRAY (240-life, spews `(10,14)` smoke every odd tick →
  ~823 entities/cast) instead of the `(10,11)` SCORCH RING (the
  earth-carve, 10-tick life, ~11 concurrent). One-line fix
  (`mc2_spawn_fire_spray` → `mc2_spawn_scorch_ring`) — measured
  +823→+1 entity footprint. The same `(10,11)`-vs-`(10,19)` numbering
  trap the cave column hit. RE-TRACE CONFIRMED the whole chain (agent,
  HIGH conf): every-tick `(10,11)` child IS correct, the flood was the
  spray misport, tiers don't scale the trail geometry. Test
  `mc2_earthquake_carves_without_flooding_the_pool` + updated the stale
  `mc2_fire_trail_drops_scorch_rings` unit test.

**SESSION 4 LANDED 2026-07-14** (Fool's Mana + Magic Mine + G4 names +
the creature pair; 14 channel tests + 12/12 suites green, MC1 + MC2
goldens hold, no bake-epoch bump):
- **Fool's Mana (22)** — the inverted single real sphere → the retail
  6-decoy neutral-fake cone (`sub_6C870`) + possess-claim retaliation
  (`sub_36680`, T0 fireball / T1 fireball×8 every-other / T2 lightning,
  homing the possessor). Trap rides free ball fields (f52 owner, f50
  tier, f136 payload, f146 claimer, f56 counter); hook in `ball_tick`
  (MC2-gated); traps excluded from the merge. **Economy fix (player-
  flagged):** decoys carry real mana for the disguise but you can't trip
  your OWN trap, so they are EXCLUDED from `world_mana` (else the
  banked/world-mana goal denominator inflates uncollectably →
  unreachable). Tests: decoy-trap / tier-2-lightning / world-mana.
- **Magic Mine (23)** — inert (10,0) contact-fireball → the carrier
  (~15-tile fuse) lands a persistent `(10,78)` proximity mine
  (`sub_50840`, effect_tick STATE 85): arm 16–65, scan every 16 for
  class-3 model≤1 within 14 tiles (excl. owner; + the human for
  rival-owned), detonate = ch0 blast ×tier-intensity (APPROX; exact
  `sub_6DCA0` relaunch + detonation-XP deferred). Tests: placement/
  persistence + detonate-on-approach. NB had to add state 85 to the
  class-10 `effect_tick` whitelist or the catch-all despawns it.
- **G4 spell names** — embedded the verbatim `L1.TXT` table (lang
  159..265, `spells::MC2_LANG`/`lang()`); `World::mc2_spell_name`
  resolves the LIVE `hint_text` (Day/non-Day auto) + `mc2_relevel_message`;
  the app hover now shows per-TIER names. CD bake / locale + the level-up
  banner rendering deferred (helper provided). **The spell SELECT + LEVEL-UP
  messages (top-of-screen red notification) are surveyed + banked** — they
  belong to the GENERIC messaging/font surface (unported both games, blocked
  on bitmap-font text rendering; HFONT3/FONT1 = HSPR format, `hspr::decode`
  works). Full survey + impl sketch in the `messaging-font-system` memory;
  the `mc2_spell_name` / `mc2_relevel_message` helpers + sound 61 are ready.
- **Metamorph (4) + Summon Army (19)** — the "blocked on control-routing"
  verdict was WRONG (Opus trace of `8·M+7`/`sub_1D5D0`): **Metamorph is a
  cosmetic pose-PUPPET** (`sub_1E4D0`, StageVar2=12) — copies the parent
  pose each tick, wizard keeps control + casting, carpet just hidden; the
  out-of-pool `human_pose` model is a clean fit. Summon Army (`sub_1E580`,
  StageVar2=13) = self-contained allied AI (acquire enemy wizard by
  team-id / follow-parent / move / hand off to the landed `+2` combat /
  250-tick). Ported: marker = free `site_z` (12/13; 0 no-ops = retail),
  intercept `action&7==7 && site_z!=0` in `mc2_creature_tick`. Metamorph
  cast spawns `(5,life)`, hides the carpet (`player.metamorph`, hash-
  skipped), teardown on the window. Summon Army delivery: cast → `(10,72)`
  impact → `mc2_spawn_summon_ring` (N by model 19/2→8, 25→4, 16→2). The
  carpet→creature swap in first-person is deferred presentation. Tests:
  transform-and-revert / allied-ring.

## FINAL-PASS VALIDATION FLAG — per-tier "charge override" audit (player 2026-07-14)

The Earthquake (17) travel bug is almost certainly an INSTANCE OF A CLASS, not a
one-off. The pattern: a CHARGED spell (dispatch arm `charge = true`) carries the
tier's `life_0x1A` in the projectile's `f71`, and retail's per-action flight
WRAPPER overrides the SPAWNED impact-effect's geometry with a function of that
charge — `sub_678E0` whirlwind = `8 * byte_0x46_70`, meteor `sub_66180` = the
charge as maxLife, cave-in `sub_67910` likewise, and Earthquake = `sub_66160` =
the charge at **1×** (life 16/32/64 → trail 16/32/64, ~2×/level; the earlier
"8 * charge" reading here was wrong — §Trace-bank corrections 6, folded
2026-07-16; only the whirlwind is 8×). The PORT must re-apply that override in
the `(10,x)` impact arm (mc2_proj_impact); when it doesn't, the effect runs the
ctor's fixed default and **every tier looks identical**. Earthquake shipped with
`max_life` hard-coded to 128 (8× the tier-0 charge) — so the port's quake was
both tier-flat AND 8× too long; the faithful 1× per-tier law landed Session F.
**Recorded gameplay is senior for CATCHING the divergence; the decompile
resolves the exact law** (the first agent trace concluding "no scaling" was
wrong, and so was the 8× correction of it — read the wrapper body).

**Final-pass task (defer; the nittiest nitpick pass):** for EVERY charged spell,
cross-check the baked `life_0x1A` per tier against the OBSERVED in-game scaling,
and confirm the `(10,x)` impact arm applies the charge override. Candidates to
re-verify beyond the ones already fixed (meteor 9 ✓, whirlwind 21 ✓, earthquake
17 ✓, cave-in 25 — has a charge arm, re-confirm the multiplier): Tremor (15,
`(10,71)` fissure — NO charge override in the arm today), Crater (16, `(10,11)`
scorch — audit says geometry tier-independent, re-confirm), Volcano (18, `(10,9)`
dome — NO charge override today), Gravity Well (20, `(10,67)` flood — NO charge
override today). Any of these that retail scales per tier but the port spawns at a
fixed default is the same bug. Method: read each spell's baked `life_0x1A` triple;
if it's a clean geometric ladder (×2 or similar) the effect almost certainly
scales and the arm must apply it.

## Playtest feedback (player, 2026-07-13 end of session — NEXT-SESSION follow-ups)

- **Meteor (7)** — CONFIRMED fixed.
- **Mana-regen (G2)** — CONFIRMED, but a **follow-up** (player
  clarified): the regen-block correctly tracks the burst (`f26 > 0`),
  but when a channel spell *functionally deactivates early* the burst
  timer keeps running, so the regen-blocking "highlight" (active
  state) does NOT clear when it should. Root cause: the effect and
  the burst timer are decoupled — e.g. Speed's boost stops on brake
  but `f26` keeps counting the full 301/451/501-tick window, pinning
  regen off. **Fix: functional termination must ZERO `f26` (clear the
  burst), not merely stop the effect** — then the regen-block lifts
  with it. Cases:
  - **Speed (3)** — braking/stopping deactivates the spell; must also
    clear the burst so regen resumes (currently the timed window keeps
    `f26` alive after the boost has stopped).
  - **Invisibility (11)** should terminate when you take the action
    that breaks it (offensive cast) — this IS the per-tier break-law
    already traced in `rival-spells.md` (T0 any cast, T1 all-but-
    possess, T2 nothing). Porting the break-law fixes both the invis
    behavior AND the regen coupling. Do them together.
  - General: audit every armed-window channel (heal/shield/rebound/
    beyond-sight/speed/invis) for its real termination condition.
- **Castle cost (gen #1)** — CONFIRMED numbers now correct in the
  display, BUT the player can still **cast below the shown cost**. The
  cast GATE (LMB/RMB fire → `mc2_cast_gate`, uses cached `f26.max_life`)
  is NOT using the same live tier-multiplied cost as the display
  (`mc2_book_view` → `mc2_spell_mana_cost`). For the castle spell the
  cost is dynamic (rises with castle level), but `max_life` is only
  re-synced on XP award / select, not on castle level-up → stale/low
  gate. **Fix:** make the cast affordability gate use the live
  `mc2_spell_mana_cost` (recompute for spell 2, or re-sync `max_life`
  when the castle level changes) so the multiplier applies to BOTH the
  gate and the display.
- **Speed (3)** — player will time the tiers and report back (peak
  speed / window length); factors are 2/3/4 × 80 = 160/240/320.

## Spell index map (resolved by the audits)

| idx | spell | idx | spell | idx | spell |
|----|-------|----|-------|----|-------|
| 0 | fireball | 9 | meteor | 18 | volcano |
| 1 | possess | 10 | teleport | 19 | summon army |
| 2 | castle | 11 | invisible | 20 | gravity well |
| 3 | speed | 12 | beyond sight | 21 | whirlwind (tornado)\* |
| 4 | metamorph | 13 | steal mana | 22 | fool's mana |
| 5 | heal | 14 | duel | 23 | magic mine |
| 6 | shield | 15 | tremor | 24 | alliance |
| 7 | lightning | 16 | crater | 25 | cave-in |
| 8 | rebound | 17 | earthquake | | |

\* whirlwind/tornado (21) already handled in the island-effects
session (2026-07-13); not in this audit list.

## Verdict table

Legend — **Fix size**: XS = ≤ a few lines / one match arm · S = one
function / data wire · M = new handler + per-tier law · L = new
entity/state-machine · DATA = asset import · DEFER = rival track ·
NONE = faithful, no code change.

| # | Item | idx | Verdict | Fix size | Blocked by |
|---|------|-----|---------|----------|-----------|
| G2 | **Mana regen blocks while casting** (MC1+MC2) | — | real bug, missing mid-burst clamp | XS | — |
| G4 | **Spell name tables** | — | not wired (UI hand-authored) | DATA | — |
| G3 | **XP scrolls** | — | awards 0 XP (counter only) | S | — |
| 3 | Speed tier factor | 3 | peak not tiered (fixed 3.0/2.0) | S | — |
| 13 | Tremor effect absent | 15 | missing impact route `(10,71)` | XS | — |
| 15 | Earthquake effect absent | 17 | missing impact route `(10,15)` | XS | — |
| 16 | Volcano effect absent | 18 | missing impact route `(10,9)` | XS | — |
| 18 | Gravity well no effect | 20 | missing impact route `(10,67)` | XS | — |
| 7 | Meteor tiers identical | 9 | missing charge fuse (`f71`→maxLife) | XS | — |
| 10 | Beyond sight tiers identical | 12 | collapsed to one bool + app filter | S | — |
| 2B | Castle cost mis-scaled | 2 | top rung + tier multiplier bugs | S | — |
| 1 | Possession tiers identical | 1 | magnet/lock not wired to cast | M | — |
| 11 | Steal mana absent | 13 | `note_misfit` stub | M | (10,39) for L3 |
| 4 | Metamorph misfit | 4 | `note_misfit` stub | M | — |
| 19 | Summon army no monsters | 19 | mis-armed `(10,0)`; real=`(10,72)` quake | M | **OPEN Q1** |
| 8 (teleport) | Teleport tiers identical | 10 | reuses MC1 single toggle | M | — |
| 19 (fool) | Fool's mana = real mana | 22 | inverted (real sphere vs trap) | M | — |
| 5 | Lightning stream / storm misfit | 7 | subtypes served by wrong tick | M | **(10,38)** port |
| 20 | Magic mine = projectile | 23 | fireball flyer, no mine | L | new `(10,78)` |
| 2A | Castle defensive towers | 2 | part-type hardcoded 0, AI stubbed | L | — |
| 14 | Crater tiers identical | 16 | **faithful** — geometry tier-indep; dmg scales | NONE | — |
| 22 | Cave-in scale identical | 25 | **faithful** — already {3,5,7}; likely tier-select | NONE | playtest |
| 6 | Rebound | 8 | tier-blind bool | DEFER | — |
| 9 | Invisibility break-law | 11 | break conditions absent | DEFER | — |
| 12 | Duel | 14 | `note_misfit` stub; real subtype-7 tether | DEFER | — |
| 21 | Alliance | 24 | mis-armed `(10,0)`; real=`(10,74)` | DEFER | (10,74) untraced |

## Corrections to the kickoff hypotheses (surface to player)

- **Crater (14)** is NOT a per-tier bug: retail carve geometry is
  tier-independent by design; only burn damage scales (250/400/900),
  which already propagates via the shared `subSpell→f140` tail. No
  code change; "same on all levels" is faithful.
- **Cave-in (22)** already scales rings {3,5,7} per tier, verbatim
  vs retail. The "identical" observation is most likely the recently
  fixed tier-selection regression / the XP unlock gate — needs a
  playtest with the all-spells toggle, not a code change.
- **XP scrolls (G3)**: the equal-split / round-robin hypothesis is
  **disconfirmed** — retail adds the FULL `countXP` (4 SP) to EVERY
  owned spell, no split. Port awards zero (only bumps a counter).
- **Spell names (G4)**: there is **no roman-numeral generator** —
  "Crater II", "Mana Magnet" etc. are all literal distinct strings
  in `LANGUAGE/L1.TXT` (base 160..185, per-tier hint 186..265). Pure
  data import.
- **Summon Army (19)**: the decompile path is NOT a firefly/gargoyle/
  wyvern creature summon — it's a `(10,72)` terrain-raise + castle-
  grab quake. The creature-ladder the player expects is **Metamorph
  (4)** (transform into class-5 model 19/25/16 per tier). See OPEN Q1.
- **MC1 castle cost**: decompile = flat 1000 constant; the port is
  faithful to it. The player recalls MC1 static cost ≈ what the
  previous castle holds — a possible recorded-gameplay vs decompile
  discrepancy. See OPEN Q2.

## Open questions (need the player)

1. **Summon Army (spell 19)** — the decompiled cast raises terrain &
   grabs castles (a quake), no creatures. What does it actually LOOK
   like in the real game when you cast it at each level? (If it truly
   spawns firefly/gargoyle/wyvern, the trace missed a branch and we
   re-investigate; if it's a quake, we port `sub_39040`.)
2. **MC1 castle spell cost** — keep the decompile-faithful flat 1000,
   or match your recollection (cost ≈ previous castle capacity)?
   Recorded gameplay is senior, so your call decides it.
3. **Alliance `(10,74)`** effect is untraced anywhere in the bank —
   deferred to the rival track regardless.

## Proposed execution order

**Phase A — cross-cutting foundations (do first; unblock verifying
everything else).** These change the *feel* the player uses to judge
the rest.
- G2 mana-regen mid-burst clamp (both games) — 1 line.
- G4 spell-name table import + resolve at hover & level-up message.
- G3 XP-scroll award loop over owned spells.

**Phase B — trivial routing + param fixes (one batch, cheap, high
count).**
- Tremor / Earthquake / Volcano / Gravity-well impact-route arms.
- Meteor charge fuse.
- Speed tier factor.
- Beyond-sight tier storage + app reveal filters.
- Castle cost top-rung + tier multiplier.

**Phase C — medium ports (per-spell, each with a cast→effect test).**
- Possession tiers (magnet aura @T1, lock @T2 — machinery exists).
- Metamorph transform.
- Teleport per-tier destination law.
- Fool's mana trap + retaliation.
- Steal mana (bolt→(10,25)→marker→drain; L3 needs (10,39)).
- Lightning (subtype-9 beam + subtype-12 storm; needs (10,38) port).

**Phase D — heavy / new entities.**
- Magic mine `(10,78)` proximity-mine state machine.
- Castle defensive towers `(10,79)` AI states 3–8 + part-type wire.

**Phase E — rival track (separate, baseline captured here).**
- Rebound, Invisibility break-law, Duel tether, Alliance.

Regression posture: each ported spell gets a headless cast→effect
test (the `mc2_*` test pattern); MC2 state-hash goldens re-pin as
needed (MC1 goldens must stay untouched — MC2-gate every change).
Author's-note: several fixes touch the shared class-9→class-10
`mc2_proj_impact` router and the SPELLS payload tail; land Phase B as
one reviewed batch so the goldens re-pin once.
