# Conformance findings ledger

Divergences surfaced by `mgc-conform verify-deltas` (docs/RECORDING.md)
against the retail recordings. This file records LEADS, not verdicts:
each entry needs decompile corroboration before any port change, and
several may resolve into capture caveats or DEVIATIONS.md entries.
Add new findings here as runs are triaged; move resolved ones to a
`RESOLVED` section with the outcome.

Baseline corpus: tear-gated recordings — **0 torn snapshots**. mc1l0
takes: 627 pairs / **73 conforming**, then 417 pairs / 32 conforming
(34 after the 2026-07-29 HW fixes; half cycles; both at
`--input-delay 2`). mc1hwl0 (churn-refined gate): **192/192 pairs
fixture-grade**, 1 gap (a saturation streak — see RECORDING.md
"Gaps"), 0 conforming. **mc1hwl0-test** (pinned cycles, saturation-
aware recorder): 290 ticks, **0 gaps, 289/289 fixture-grade** — the
certification-grade HW take; still 0 conforming pending the
remaining entries below (the walker x/y/z drift of entry 7 touches
nearly every pair). (History: the
first flat-tolerance gate starved HW — ambient spawn churn rewrites
+63 with spawn ordinals every tick and was read as tearing, 85-tick
rejection streaks; the gate now counts only `dv±1` steps as tear
suspects.) Every open entry below reproduced across all takes,
including the 75%-torn pre-gate corpus.

## Confirmed conforming (worth naming)

- **Global LCG draw law**: 627/627 MC1 pairs and 48/48 HW pairs draw
  exactly one `9377x+9439` step per tick, matching the port's
  tick-top draw. (The previously-banked "12.5% draw-driven stall"
  was a capture artifact — see RECORDING.md "Capture tearing".)
- **The +63 phase clock**: 12 stray entity-ticks over 627 MC1 pairs,
  all spawn-edge (ordinal overwrites, projectile birth/death). The
  port's "step every dispatched entity" matches retail's static
  state table (`data10` is 1 on every live row —
  docs/traces/mc1-state-table.md, sub_main :52356/:52406).
- **Free-list discipline**: with the LIVE free-stack imported, port
  spawns land on retail's slots (verified: the fireball at slot 627
  both sides).

## Open leads (port vs retail, unfixed by ruling 2026-07-29)

1. **HW ambient-family population loss — ROOT-CAUSED, mostly fixed**
   (see Resolved; residuals on the 289-pair mc1hwl0-test take): the
   port lacked generic MC1 handlers that HW's content exercises. The
   engine is byte-identical MC1↔HW (no data-table delta); the
   "weather" was (a) (10,2)/(10,3) puffs the port reaped via the
   terrain-dispatch self-kill catch-all — ctors+ticks now ported —
   and (b) rivals' class-12 owned-spell TOKENS the port decayed as
   scatter jars — docs/traces/mc1-class12-spell-tokens.md, fixed via
   strict_retail. REMAINING: 57 (10,0) + 3 (10,1) corpse-cascade
   under-spawn (retail `sub_1A800`: corpse slot → (10,1) puff, ball
   via `sub_27690` only on carried mana — port's mob_corpse differs
   at the boundary; ~33 (10,39) ball diffs ride the same chain);
   39 (10,2) from the UNPORTED active speed-token emitter
   (`sub_56380`, puff every 4th token-tick); a few (9,1) from the
   spell-3 bolt token (`sub_56510`).
2. **Mana-ball merge** — mc1l0 first divergent clean pair t=26:
   port merges balls (slot 627 mana 512→1024, slot 484 flagged dead
   0x400) where retail keeps both at 512. Merge-condition law
   differs (distance? owner? tick phase?).
3. **Castle upgrade window (t≈605-632)** — a cluster: port's (10,42)
   build painter persists across ticks where retail's never appears
   at a snapshot boundary (29 extra entity-ticks — possibly retail
   spawns+works+frees it within one tick); castle max_life retail 8
   vs port 21 (36 pairs); the port's castle-binding scan picks slot
   475 vs retail's 629; slot-28 mound mana_max 10000 vs 1000. The
   whole castle build/upgrade path needs a trace-backed pass.
4. **Impact cluster around casts** — (10,12) hit-flash and (9,1)
   fireball timing jitter, consistent with the ±2-3 tick input
   latency (retail mouse ISR → control command → consume) that cast
   reconstruction cannot see. Partially mitigated by
   `--input-delay 2` (9→12 conforming pairs). Aim (control aim_yaw/
   aim_pitch) is not reconstructed at all — projectile trajectories
   diverge accordingly. Consider recording-side capture of the
   control slot mid-tick, or accept as input-domain noise.
5. **Residual creature motion noise** — x: 313 / y: 160 / z: 212
   entity-ticks over 153 clean pairs (was ~34k before the tear gate;
   the bulk was tearing). What remains clusters around combat/impact
   ticks; re-measure on a tear-gated corpus before reading laws into
   it.
6. **wizard0 hand flicker** — a few pairs per run where retail's
   hand reads empty (0xFFFF) while the port holds the equip. Retail
   appears to blank the hand transiently (book UI? cast window?).
   Harmless-looking; not modeled.
7. **HW terrain shortfall + walker drift** (narrowed from "systematic
   z −64" — the class-12 part is RESOLVED below): the port's HW
   ground sits below retail's in places (~8 height-bytes ≈ 256
   z-units around (56,246) on level 0), driving residual z hits
   (1349/192 pairs after the jar fix, was 5189) and the broad
   x/y/heading drift on ground-following walkers. Candidates: (a)
   `mc1_terrain::generate` not bit-exact on HW/DDLEVELS gen-map
   parameter ranges (validated for temperate only), (b) retail
   runtime terrain edits (castle/wizard pads) that are non-closure
   state the importer resets. Also open from the same triage:
   retail manifestations import with `+70 < 200`, below the port's
   `MANIFEST_BASE = 200` encoding, so imported manifestations take
   the resting-jar path instead of `manifestation_tick`.
8. **HW stat doubling — DISSOLVED (entity-substitution artifacts)**:
   "life 30 + flags 65536(0x10000)" = the port's (10,42) castle
   build painter occupying a slot it reaped (painter max_life=30,
   build bit 0x10000) — not a scaled (10,2); "mana_max 20000 vs
   10000" = the castle (3,2), whose stats are identical MC1↔HW —
   that is finding 3's castle column. retail flags 131073 = 0x20001
   (effect bit17 + active), port 65536 = 0x10000 (painter build
   bit): two different entities, no bit-shift exists. No stat-scale
   fix anywhere; closed into findings 1 and 3.

## Resolved

- **(10,2)/(10,3) puff reaping (the bulk of old entry 1)** — RESOLVED
  2026-07-29: ctors (`str_255D0C[2/3]` = `sub_3A570`/`sub_3A5D0`) and
  tick handlers (`str_255998[2/3]` = `sub_252B0`/`sub_253F0`, bare
  pre-decrement) ported un-gated (generic MC1 code, HW just exercises
  it). Missing (10,2) entity-ticks 1090-scale → 39 (only the unported
  speed-token emitter remains).
- **(12,1) loss (162-scale → 0)** — RESOLVED 2026-07-29: those were
  rivals' class-12 owned-spell TOKENS (retail encodes tick70 =
  spell*3+phase; state 3 = the idle HEAL token), which the port's
  DROPPED_JAR=3 decay reaped. Under `strict_retail`, imported
  class-12 entities follow retail's law (inert; active handlers
  still open) — docs/traces/mc1-class12-spell-tokens.md. Phase-clock
  disagreements 224 → 44 per 289 pairs; MC1 conforming pairs rose
  32 → 34.

- **HW systematic z −64 (the class-12 half of old entry 7)** —
  RESOLVED 2026-07-29: the port re-snapped resting class-12
  jars/manifestations to ground every tick (`class12_tick`), an
  unregistered cosmetic workaround; retail's terrain-reshape walk
  re-snaps class 2 and kills class 5 but default-skips class 12
  (remc1/remc1hw `sub_40E20_41160` :51745-65), leaving jars
  hovering/buried at their spawn z — confirmed by the recording
  (slot 161 holds z=3408 for hundreds of ticks over lowered
  ground). True sign was port-LOWER, magnitude the local terrain
  gap (64/80/256), not a uniform datum. Resolution (player-ruled):
  the snap STAYS for gameplay — it is what keeps HW's authored
  jars pickable and earthquake aftermath grounded — but is now a
  registered deviation (DEVIATIONS.md "World::class12_tick (jar
  ground-snap)") disabled in strict-retail mode: `retail_import_mc1`
  sets `World::strict_retail`, under which imported retail worlds
  evolve by retail's frozen-z law. Tests pin both behaviors;
  goldens unmoved; HW z hits 5189 → 1349 (all class-12 diffs gone;
  the rest is entry 7's terrain shortfall).

## Capture caveats (not port bugs)

- Pre-gate recordings: mid-pass tearing (75% of mc1l0 pairs) — see
  RECORDING.md. The runner's `capture_clean` re-classifier is
  authoritative for old files.
- The human carpet's +63/rand/flags have no port counterpart (the
  human lives outside the pool); the comparator restricts the pinned
  slot to life/mana fields.
- `owner_ptr` (guest pointer) is never compared; behavior rows are
  compared via the derived index (`(ptr − base)/32`, base anchored on
  the carpet's canonical row 7).
- MC2 verify-deltas is not wired (obs decode + check-decode are); the
  MC2 tear gate is open (no per-entity clock — needs Turn + LCG-step
  parity at minimum).
