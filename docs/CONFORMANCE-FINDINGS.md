# Conformance findings ledger

Divergences surfaced by `mgc-conform verify-deltas` (docs/RECORDING.md)
against the retail recordings. This file records LEADS, not verdicts:
each entry needs decompile corroboration before any port change, and
several may resolve into capture caveats or DEVIATIONS.md entries.
Add new findings here as runs are triaged; move resolved ones to a
`RESOLVED` section with the outcome.

Enforcement lives in the FIXTURE SUITE (docs/CONFORMANCE.md,
`conformance/*.json`): triaged pairs replay on every `cargo test` with
expected statuses (`conforming`/`open`/`capture`); fixture notes cite
the entries below. Fixing an entry flips its fixtures — promote them
(`mgc-conform fixtures … --promote`) in the same change that moves the
entry to Resolved.

Baseline corpus (2026-07-30, the rate-limited `*_REC.EXE` recorder,
all pairs at `--input-delay 2`):
- **mc1l0**: a FULL gapless level-0 playthrough, 5330 ticks,
  check-decode 5330/5330 exact, 5329/5329 pairs fixture-grade, 0
  torn. **59 conforming at capture → 350 conforming after the
  2026-07-30 fix round** (castle f59 import, ball settle/roll/
  hard-free, strict-retail jar pickup, tree water gate, regen
  seed — see Resolved). First divergent pair t=11 → t=58 (an
  input-latency cast, entry 4's domain). RNG (1,1) on all 5329.
- **mc1hwl0**: 2026-07-30 partial HW level-0 take (173MB) — NOT
  yet triaged; run verify-deltas next (expect entry 7's terrain
  shortfall plus the same now-fixed families).
- **mc2l0**: 2026-07-29 full take, 0 gaps — complete, awaiting the
  MC2 importer.
(Triage tooling on the runner: `--csv` per-diff TSV for offline
clustering, `--dump <t> [--dump-port]`, `dump-state <file> <t>
<slot…|all>`, `trace <file> <slot> <t0> <t1>`.)
(History: the first takes ran 627 pairs / 73 conforming and 417 /
32; the fix rounds moved the like-for-like take to 34, and the full
recipe + fixes reached 117.) (History: the
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
2. **TERRAIN CLOSURE — the dominant residual family** (proven on
   the 2026-07-30 mc1l0 corpus): the recording has no terrain
   channel and every pair replays on pristine planes, so retail's
   runtime terrain edits are invisible. The z diffs cluster at
   event sites with CONSTANT per-entity offsets from an onset tick
   to end-of-run: fireball craters (t=109+, −1..−4), a large edit
   field around (112,88) from t=472, the rival castle raise at
   (44,40) +256 from t≈1028, and the village regrade at
   (152-164,44-56) from t≈992-1139 (heights AND tile TYPES — the
   construction paint pass, sub_27D30 :30184-248). After the fix
   round this family is z 134k hits / 4801 pairs plus the walker
   x/y/heading knock-on — roughly all remaining bulk noise. Fix
   direction: a terrain channel in recording format v2 (height +
   type planes, delta-coded or hashed-with-keyframes), or replay
   the edit events in the importer.
3. **Castle build WINDOW + economy** (narrowed from the old "castle
   column" entry — the settled-castle half is RESOLVED below): the
   t≈469-513 initial-build window still diverges (transform ball
   slots, (10,39)/(10,0)/(10,1) around the build, castle binding
   retail 627 vs port 475, upgrade mana lumps −5000/−10000). Known
   unported pieces: the wizard's castle slot binding (retail
   player+50, sub_47960 :56484), the mound-mana write at level-up
   (sub_47BD0's mound arm :56561-66 — the slot-28 mana_max
   10000-vs-1000 rows, now only 129 hits), and the ball-economy
   cap-fill (sub_47130 :56160). ~100 pairs.
4. **Impact cluster around casts** — (9,1) 468/610 missing/extra,
   (10,12) hit-flash, (9,0), and the t=67-style substitution
   clusters: input-latency + unreconstructed aim (control aim_yaw/
   aim_pitch). Same ruling as before: partially mitigated by
   `--input-delay 2`; consider recording the control slot mid-tick.
   This is now the FIRST divergence family (t=58).
5. **Human mana regen cadence — OPEN, port regens 3-4× retail**:
   retail's human +100 regen quanta land at drifting ~3-4-tick
   intervals (trace t=1200-1240: mostly period 4 with adjacent
   doubles; NOT f63%4-clocked — the phase walks). The port applies
   `mana_delta` every tick (long-standing gameplay law, goldens
   locked). The wizard tick's `mana += +132` (:55385) sits behind
   only the pause gate (var_u8_2 bit0), so the cadence source is
   elsewhere (a timer-domain clock? the walk's class-3 rate nibble?
   kill/collect +100s interleaved?). ~1276 player.mana pairs
   (−100/−150/−300 compounds with cast latency). Needs its own
   decompile pass before touching the gameplay regen.
6. **wizard0 hand residuals** — 4 pairs: t=310 retail Some(3) port
   Some(16), t=409 hand_right Some(3) vs None. Pickup RESOLUTION
   differences (which jar/hand a mid-level acquisition lands in),
   not the old flicker; revisit with the quickselect-assign law
   (docs/traces/mc1-quickselect-assign-law.md).
9. **Small new families** (post-fix corpus): (9,0)/(9,1) flags
   0x2006-vs-0x6 (bit13, ~176 rows); (2,0) tree missing residue 53
   rows at exactly t=1056(×6)/t=1100(×47) — the hut-completion
   retile edge ticks; (10,39) flags 12→4 (port ball loses the 0x8
   default bit on some spawn path, 177 rows); wizard Path-A ball
   collection (sub_1E810 instant absorb) still unported — the
   mana-economy track's (10,0) puff + ball-removal rows ride on it.
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

- **Castle phantom upgrade (the settled-castle half of old entry
  3)** — RESOLVED 2026-07-30: retail castles keep their macro-state
  in the JOB byte +70 (4 settled / 5 transforming / 6 building,
  sub_46DB0/sub_46F10) with the transform sub-state in +48; the
  importer wrote retail's dead +59 byte into the port's fused `f59`
  machine, parking every settled castle in f59=0 = the level-up
  commit — one phantom upgrade per pair (stats one level ahead,
  1612 extra (10,42) painters, castle life reset). Importer now
  maps (3,2) f59 from (+70,+48); `castle_tick` case 4 additionally
  honors the retail upgrade-request bit (+16 & 0x40, :56007-11,
  cleared at commit :56475) and `castle_absorb` takes ONE ball per
  absorb tick (:56030-42). max_life hits 5736 → 695, mana_max
  3627 → 129, (10,42) extra 1612 → 11. The retail ladder
  (CASTLE_HP/CASTLE_CAP) was already correct. Retail castles have
  NO life regen (only the ladder snap + damage) — confirmed, and
  the port's case 4 has none either.
- **Mana-ball laws (old entry 2 + the slot-103 insta-kill)** —
  RESOLVED 2026-07-30 from sub_27030 (:29416-571) + sub_54F80
  (:64318-20): a ball is ballistic — gravity, grounded downhill
  roll (sub_41F50 = the 2×2 forward difference), 250/256 friction,
  and the grounded-only MERGE scan — only while its +58 settle
  countdown (ctor 0x80, −1/tick via the global anim pass) is
  nonzero; at 0 it freezes at rest FOREVER (no TTL — max_life 300
  is inert). Retail's merge donor is HARD-freed (sub_41E90), gone
  from the same snapshot. The port merged/rolled resting balls
  forever (a settled ball beside the castle was re-merged on every
  pair for 3000 ticks, timeline-matching spawn+128), ran MC1
  friction unconditionally with no roll (contradicted by its own
  cite), and soft-killed donors into extra-in-port rows. All
  MC1-scoped; MC2's sphere twin untraced and untouched. Ball x/y
  hits 56k/62k → 9.7k/9.7k, (10,39) missing 1621 → 315. MC1
  goldens re-pinned (behavior change by design; OBSERVABLE moved
  A-E, post-init holds).
- **Jar pickup under strict_retail (the t=11 first divergence)** —
  RESOLVED 2026-07-30: the strict arm was fully inert, so retail's
  jar-pickup poll (sub_55A40 :64729-872 — every-4th-tick, AABB,
  grant = in-place convert to the owned token + LEFT auto-equip +
  the jar's own bit0 stamp; already-owned = pure no-op, NO
  jar→mana path exists) never ran. Ported into the strict arm with
  retail's encoding (tick70 = spell*3). The old "port converts
  pickup to a mana ball" reading was wrong — the extra (10,39) at
  t=11 was retail's grounded ball-merge hard-free (see above).
- **Village-tree reap ("(2,0) hut" family)** — RESOLVED 2026-07-30:
  the reaped entities are TREES (class-2 model-0; retail huts are
  class-10 model-45, ctor sub_3B690 :47501-18). Retail's village
  construction PAINTS tile types under them (sub_27D30 :30184-248);
  on pristine replay planes those tiles still read water and the
  tree's own splash-die (:57703-11) fired in one tick. Strict-retail
  now suppresses the tree water arm (capture-domain, same pattern
  as the class-12 frozen-z law). 1960 rows → 53 (the completion
  retile edges, entry 9). Gameplay unchanged.
- **player.mana regen seed** — RESOLVED-as-import 2026-07-30: the
  importer now seeds `player.mana_delta` from the carpet's +132
  (the applied-then-recomputed pipeline both engines share). The
  remaining divergence is entry 5's cadence gap (port every-tick vs
  retail ~every-4th) — the family flipped sign from +100 (missing
  regen) to −100 (over-regen).

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
