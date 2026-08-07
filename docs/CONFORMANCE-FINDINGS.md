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

Baseline corpus (2026-07-31 re-records on the MONOTONIC-frame-counter
`*_REC.EXE` recorder — the tickpatch mailbox latches the per-frame
clock on both games, so the MC2 Turn++-park tear is GONE; all pairs
at `--input-delay 2`; suites refreshed 2026-08-01 via fresh extract +
`carry_curation.py` + `classify_fixtures.py`):
- **mc1l0**: gapless full level-0 playthrough, 5,874 ticks, 5,873
  pairs, 0 torn, all fixture-grade, **450 conforming** (440 before
  the wake-law round); RNG (1,1) on every pair. Roster-aware
  (post corpse-flame spreader fix, 2026-08-02): **4,152
  conforming-or-explained**, UNEXPLAINED 12,129 field /
  98 missing / 211 extra rows. (The `mc1l0-village-regrade` rule
  hit 0 rows — its t/rect scope was the OLD take's regrade event;
  retire or re-scope on the next roster pass.)
- **mc1hwl0**: full HW take under meteor weather, ticks 0..39,800
  with 15 gaps (69 frames — heavy-animation skips; a skip-free HW
  run is not achievable) + 517 torn, 39,199 of 39,716 pairs
  fixture-grade, **49 conforming** (46 → 48 wake-law round → 49
  corpse-flame spreader 2026-08-02);
  RNG (1,1) on 39,171 pairs, retail >16-draw bursts on 28. Terrain
  closure still owns ~every pair (`mc1hwl0-terrain-z` explains
  2.12M rows / 39,133 pairs; 2.28M field rows unexplained — HW
  progress keeps reading from per-family totals + the story suite,
  not the pair headline).
- **mc2l0**: gapless 8,627 ticks, 8,626 pairs, **0 torn** (take-2
  on the rate-limited recorder tore 1,105 of 3,640), all
  fixture-grade, **479 conforming** (167 → 240 cave-rand round 2 →
  452 same-tick reap → 466 day-bank extents → 479 possession
  tier-0 gate + shared spreader, 2026-08-02); rng
  mismatch on **2 pairs only** (was 3 — reap-aligned seeds).
  Roster-aware: **6,066 conforming-or-explained**, UNEXPLAINED
  6,829 field / 123 missing / 21 extra (the reap converted most
  ghost-alias extras: gross extras 3,761 → 1,389, unexplained
  extras 198 → 22; gross missing 431 → 1,095 — dominated by
  re-labeled slot-alias rows, see the reap Resolved entry).
- **mc2l4 + mc2l30** (CUT 2026-08-01 from the single conjoined
  `mc2l4,30.mgcr` take at t=17713; the take's SINGLE frame skip
  17711→17713 is exactly the level transition — the tick fn never
  ran during the load — so both cuts are internally gapless, and
  the embedded level record flips at the cut as before): mc2l4 =
  17,711 pairs, 0 torn, all fixture-grade, 0 conforming raw but
  **13,698 of 17,711 pairs roster-explained (77%)**,
  rng mismatch **13** (163 before the fire-spray ring loop,
  2026-08-02); mc2l30 = 9,337 pairs, 0 torn, all
  fixture-grade, rng mismatch **19 of 9,337 pairs** (9,328 →
  cave-rand structure round 2 → 202 → 19 fire-spray ring loop +
  summit latch/frozen-z, 2026-08-02; session 4 REFUTED
  the per-entity `rand_0x14` hypothesis — the residual WAS the
  VOLCANO-CASCADE, §l30-churn (b) as re-written; of the last 19,
  one is the t=274 dome-import eruption-timing pair, 18 ride the
  slot-desync fire cascade), **6,686 pairs roster-explained**
  (was 1 → 6,320 → 6,658; reap collapsed the (10,0)/(10,14)
  extra side 5,590→346 / 917→36; UNEXPLAINED now 14,007 field /
  188 missing / 87 extra). Suite note: one mc2l4 exemplar's signature differed
  between the full extract pass and the sparse suite pass (the
  shared world instance leaks a trace of which pairs ran before —
  select-dependence, warning-grade); re-promoted to the
  suite-stable signature.
(Triage tooling on the runner: `--csv` per-diff TSV for offline
clustering, the POSE-PHASE classifier (2026-08-01, docs/CONFORMANCE.md
§pose-phase: every dirty pair re-runs under the other `--pin-pose`
sample; rows clean in either run tag `pose-phase` = within-tick pose
capture, leave the UNEXPLAINED headline, CSV rule column literal;
`--no-pose-alt` disables — mc1l0 claims 987 field rows/288 pairs,
mostly (5,x)/(9,0) aim+step; the (9,1)/(9,0) aim families that match
NEITHER pose stay open), `--dump <t> [--dump-port]`, `dump-state
<file> <t> <slot…|all>` — now also prints both free/recycle stack tails,
next-pop last — `trace <file> <slot> <t0> <t1>`, `--start <t>`
windowed triage on the MC1 arm too (announces pairs + the
free-stack fallback, wired through the MC1 import report), and
`ground-audit <file> [--dump t]` — retail rest-z vs the port's
generated plane per (class,model) + 16-tile site, the instrument
that refuted the HW generator-shortfall hypothesis.)
(History: the first takes ran 627 pairs / 73 conforming and 417 /
32; the fix rounds moved the like-for-like take to 34, and the full
recipe + fixes reached 117.) (History, 2026-07-30 corpus on the
rate-limited recorder, retired by the monotonic re-records: mc1l0
5,329 pairs / 385 conforming after the tick-top-reap round; mc1hwl0
40,586 fixture-grade / 1 conforming, entity-set misses 717,798 →
~33k after reap + rival re-anchor; mc2l0 take-2 7,762 fixture-grade
of 11,523 with 3,761 torn / 7→11 conforming, 5,242
conforming-or-explained; mc2l4 12,786 grade of 19,154 with 6,368
torn, mc2l30 10,021 grade of 15,428 with 5,407 torn — the per-take
triage sections below cite THAT corpus's tick numbers and counts.)
(History: the
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

0. **ENTITY-SET MISSING SIDE (post-reap map, 2026-08-01).** The
   MC2 same-tick reap LANDED (see Resolved — the extra side
   collapsed: l30 (10,0) 5,590→346, (10,14) 917→36; mc2l0
   unexplained extras 198→22) and the missing side is now the
   dominant entity-set lead. Post-reap gross missing (mostly
   roster-explained cast-timing, but the big non-(9,x) families
   are real): **mc2l30 (10,0) 1,693 + (10,14) 984** and **mc2l4
   (10,0) 2,005 + (10,14) 890 + (10,12) 717** — retail fire/riser
   spawns the port never makes (churn spawn cadence — the
   rand_0x14 suspicion is REFUTED, see §l30-churn (b): on l30 the
   family is the VOLCANO-CASCADE fire spread + summit re-erupt
   cadence; the l30 202 rng-mismatch pairs cluster exactly on the
   eruption windows); mc2l0 (10,13) 45 missing / 81
   extra (newborn churn into recycled slots — fixture t=737
   re-statused capture `mc2-fire-churn-m13`). Genuinely
   independent missing families still queued: **mc1l0 (10,0)
   fires 57 missing / 210 extra — MC1 has no reap excuse, fire
   spawn/expiry CADENCE, a real family**; mc1l0 (10,39)
   ball-merge edges (50/31); mc2l0 (2,0) trees 18 missing;
   (10,45) houses 7 missing (= §castle follow-up (c)
   build-window). Slot-mismatch stays MINOR (15 rows mc1l0 / 0
   mc2l0).

0b. **MC2L24 SCRIPTED CREATURE WAVES — SPAWN, BUT SLOT-DESYNCED
   (2026-08-02; dig B's "unported trigger" claim CORRECTED by the
   fixture signatures).** Two level-scripted spawn waves fire at
   t≈3569 ((5,3) worms + (14,1)×3 + (10,63) + (5,9) + (5,26)) and
   t≈13330 ((11,x) triggers + (5,17)/(5,20)/(5,26) + (10,71) +
   more). The t=3569/13330 fixture sigs show EXTRA and MISSING of
   the SAME models in the same pair, and whole-take totals are
   balanced ((5,3) 63/60, (14,1) 4/4, (5,9) 6/8) — the port DOES
   spawn the waves, at desynced slots: the ruled free-list
   slot-order infrastructure limit at mass-spawn ticks, not a
   missing trigger. **SESSION-6 UPDATE (2026-08-03): the ruling is
   now the computed `slot-desync` roster rule (dig F, see
   Resolved), and the re-census is DONE — (10,25) 37/0 and
   (10,75) 110/13 post-absorption are REAL unported-spawn leads
   (doomsday-pyramid effect + tail-drag chain); the (5,0) owner
   rows and the class-15 detach machine are RESOLVED (digs E + D —
   note (5,0) = pyramid-summoned worms, NOT hydra segments).**
   **SESSION-7 UPDATE (2026-08-03): (10,25) + (10,75) are now
   RESOLVED too — and the "doomsday" attribution was WRONG. Both
   are the (11,2) STORM-SWITCH disposition (whirlwind heads +
   funnel nodes + area blasts); the port's switch box dropped the
   human carpet's own 121-unit half-extent. See Resolved, "MC2
   SWITCH VOLUMES LOST THE HUMAN'S OWN HALF-EXTENT" —
   (10,25) 37/0→7/0, (10,75) 110/13→13/14, (10,22) 10/0→2/1,
   l24 missing 1,209→1,074. The residue is the same free-list
   slot-order limit this entry describes.**
   **SESSION-8 UPDATE (2026-08-03): the "free-list slot-order limit"
   was NOT an infrastructure limit — it was a PORT BUG. The MC2
   import double-pushed every ghost slot onto the free stack (once
   itself, once through `tick()`'s reap), so a spawn burst deeper
   than the ghost count re-allocated slots it had just filled. See
   Resolved, "THE MC2 CONFORMANCE IMPORT DOUBLE-PUSHED EVERY GHOST
   SLOT". A SECOND slot-order source survives and is now ROOT-CAUSED
   too (fix NOT landed — `mgc-formats`, own dig):
   **`mgcr::mc2_stack` recovers the pool base by assuming the stack's
   HIGHEST cell is slot 999.** It scans `cells[0] − s·168` from
   s=999 down and takes the first candidate under which every cell
   decodes in-range — every candidate keeps the cells stride-aligned
   (they are all pool pointers), so the only binding constraint is
   "max index < 1000", i.e. the lowest legal base = max cell ↦ 999.
   The moment the top pool slots are IN USE and therefore absent
   from the stack, every decoded slot is inflated by that many.
   Measured on mc2l24: t=53808 shift 0 (0 of 716 cells land on an
   occupied slot, census passes), **t=60101 shift 2** (197 of 576
   cells land on live (10,39)/(5,25)/(5,15)/(10,79) records),
   **t=62929 shift 4** (129 of 226) — a brute force over a constant
   k proves a unique k makes EVERY cell land class-0. The import's
   census catches the corruption (`live.len() != scan_free`) and
   falls back to the descending slot scan, which pops lowest-first
   and re-orders every spawn in the pair (t=60101: the pyramid's
   worm chain lands in slots 5/8/9/37… against retail's
   576/584/585…, 48 balanced missing/extra). FIX SHAPE: choose the
   base by VALIDATING against the pool image (the unique shift under
   which every cell lands on a `class3f == 0` slot) instead of the
   max-index guess; the runner's `free-stack fallback: live X !=
   scan Y` stderr line (under `--start`) is the ready-made
   instrument for counting how many pairs it costs per take.**
   Knock-on: mass-tick slot skew feeds the 52-63k epoch churn
   asymmetry and the lone rng residual at t=51556.
   **SESSION-9 UPDATE (2026-08-04): the `mc2_stack` half is FIXED —
   the base is now validated against the pool image (and the recycle
   stack shares it). See Resolved, "THE `.mgcr` MC2 DECODE GUESSED
   'THE HIGHEST STACKED CELL IS SLOT 999'": `free-stack fallback` is
   gone from all 69,207 mc2l24 pairs, the four shifted windows drop
   gross missing 9,402→469 / extra 10,134→1,241, and the computed
   `slot-desync` rule stops firing there entirely. WHAT SURVIVES: the
   EARLY-take desync (t=3569 / 13330 and the whole-take 208/208
   slot-desync residue) is a DIFFERENT cause — l24's first shifted
   snapshot is t=54932 and the 3500+300 window is byte-identical
   across the A/B, so this entry's early-wave rows need their own
   dig.**
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
2. **TERRAIN CLOSURE — the dominant residual family. ⚑ PLAN
   FINALIZED 2026-08-05 (player-directed), EXECUTE NEXT SESSION,
   ALL THREE GAMES — `docs/RECORDING-TERRAIN-V2.md` is the plan.**
   Decided shape: the recorder captures the height+type planes
   every record and stores **deltas relative to the PREVIOUS
   RECORDED TICK** (record 0 = full planes) — self-healing across
   recording gaps, near-zero size on quiet ticks. **Importer
   CARRY-FORWARD was evaluated and is a DEAD BRANCH (player-ruled
   same day): the recordings contain gaps (graphic-overload
   stalls), a carried terrain loses every edit inside a gap and
   silently poisons all downstream grading. Do not revisit.**
   Free instruments: record-0 planes = the stock-bake validator;
   per-pair terraform grading (port terrain writes vs retail's
   delta, cell-by-cell). The import-time reconstructions (pad
   replays, riser endcaps, prop-z inversion) demote to sensors
   once measurement lands. Rides the tickpatch + PP_CASTLE
   re-record round. Player's underlying theory, refined: every
   terraforming CAUSE is already in the corpus, so a continuous
   1:1 replay would need no channel — but the harness grades PAIR
   closures, and accumulated terrain state is not in the snapshot;
   the channel turns the capture bucket into measurement.** (proven on
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
   **FIX ROUND 2026-07-30 (opus dig + port)**: the "mound" = the
   player's CREATE-CASTLE MANIFESTATION (class 12 m16, ctor
   sub_3C060 → sub_3BF70 :48026: +136=1000, +140=1000/101, +50=101
   the divisor). sub_47DD0 (:56617-73) refreshes it EVERY tick from
   the wizard handler while a castle is bound: +136=cap[level],
   +140=cap/101 — now ported at the castle dispatch site
   (world.rs, strict-retail scoped, f144-owner join; the imported
   manifestation encodes +70<MANIFEST_BASE and never reaches
   manifestation_tick, so a manifestation-side fix is dead under
   conformance). castle_eject also gained retail's pool-headroom
   count cap `min(free+1, clamp(spill/1000,1,32))` (:56194-205) and
   continue-on-failed-alloc (:56213). Slot-28 mana_max 129→21 hits;
   350→384 conforming with the entry-5 clamp. wizext+50/+416: +416
   is WRITE-ONLY (no reader — no port action); the 627-vs-475
   binding is an allocation-order SYMPTOM of the remaining window
   divergence, not a missing write.
   **WORM-WINDOW DIG RESOLVED 2026-07-30 (opus agent + corpus
   A/B)**: the "(5,3) per-tick segment emission" hypothesis was
   WRONG — no emission law exists. The window is a WORM MASS-DEATH:
   heads 61/97 die at t≈472 (state 22), the death handler corpses
   the whole +54 chain in ONE tick (sub_1A6C0 :21828-39), and the
   corpses free one-per-phase-lane (`f63&7==0`, two lanes per worm
   8 slots apart = the four descending lanes). The port's death/
   corpse/segment handlers are faithful; the divergence is
   POOL-ORDER: the port frees 0x400 slots at once and its corpse
   ball-drops recycle the just-freed low slots, where retail
   allocates high (633+) and keeps low-ghost records. ⚠ EXTENDING
   MC2's strict-retail ghost/free deferral to MC1 was TRIED and
   MEASURED WORSE (384→377 conforming, both halves independently)
   — MC1's reaper evidently returns slots within the frame, unlike
   MC2's next-frame remove pass; the free-guard comment records the
   refutation. ALSO from the dig: the port's
   worm-segment ctor re-stamped id24 with the segment's own slot —
   retail's byte-copy KEEPS the head's +24 (corpus-pinned). Fixed
   (goldens re-pinned layout-only, OBSERVABLE holds); this also
   keeps kill credit head-only in native play.
   **CASCADE RESOLVED 2026-07-31 — it was the TICK-TOP REAP LAW,
   not pool order** (see Resolved). The trace showed the pop order
   was correct all along: retail's castle pop (627) was the
   stack top BECAUSE the flag-deferred frees land at the top of
   the next tick, before dispatch. With the reap law landed the
   worm-window substitutions, the phantom second castle at 475
   (a delivered (9,10) re-triggering), and the missing same-tick
   (10,42) painter all cleared. REMAINING (small): the same-tick
   painter's ctor fields differ (port max_life 30 vs retail 0,
   chase 627 vs 0, x off-by-one-tile at spawn) — a (10,42) ctor
   transcription pass vs sub_47020's spawn site is owed.
4. **Impact cluster around casts** — (9,1) 468/610 missing/extra,
   (10,12) hit-flash, (9,0), and the t=67-style substitution
   clusters: input-latency + unreconstructed aim (control aim_yaw/
   aim_pitch). Same ruling as before: partially mitigated by
   `--input-delay 2`; consider recording the control slot mid-tick.
   This is now the FIRST divergence family (t=58).
5. **Human mana regen cadence — RESOLVED 2026-07-30: THERE IS NO
   REGEN CLOCK.** Retail applies `mana += +132` EVERY frame behind
   only the pause gate, then recomputes +132 to its +100/+1000
   floor (:55385, :55407-21); the "drifting 3-4-tick cadence" is
   the NET of that regen against firing-driven suppression + costs:
   every live MID-burst spell event zeroes the caster's +132 before
   the next apply (sub_55E80 :64956 — the first burst tick,
   +48==+50, does not; remc2's twin sub_68DE0 runs the same shape
   with the cost stamp live), and the +90 mailbox debits land the
   same frame. Manifestation slots fall before/after the carpet's
   slot 630 as they churn, so suppression lands same-frame or
   next-frame — a one-frame stamp-then-apply jitter beating against
   the ~4-tick fire rhythm (the MC2 @0x88 "survives one frame" twin
   is the same phenomenon). Timer-domain and f63%4 hypotheses
   REFUTED (five consecutive +100 frames at t=121-125). **The
   port's every-tick regen is CORRECT — the long-standing "port
   regens 3-4× retail" concern is DISSOLVED; do not throttle it.**
   The conformance residual was import-side: the recorder samples
   +132 AFTER the recompute, so the importer seeded +100 on frames
   retail had suppressed — retail_import_mc1 now clamps the seed to
   0 when a live mid-burst manifestation exists (f48≠0, ≠f50,
   human-owned). player.mana 1276→580 pairs; the remainder is
   cast-latency compounds (capture) + the entry-3 window.
6. **wizard0 hand residuals** — 4 pairs: t=310 retail Some(3) port
   Some(16), t=409 hand_right Some(3) vs None. Pickup RESOLUTION
   differences (which jar/hand a mid-level acquisition lands in),
   not the old flicker; revisit with the quickselect-assign law
   (docs/traces/mc1-quickselect-assign-law.md).
9. **Small new families** (post-fix corpus): (9,0)/(9,1) flags
   0x2006-vs-0x6 (bit13, ~176 rows); (2,0) tree missing residue 53
   rows at exactly t=1056(×6)/t=1100(×47) — the hut-completion
   retile edge ticks; (10,39) flags 12→4 (port ball loses the 0x8
   default bit on some spawn path, 177 rows — likely the entry-3
   substitution family, see the worm-segment note there).
   **CORRECTED 2026-07-30: sub_1E810 is NOT a wizard path** — it is
   the GENIE's (m11) ball eat, called only from genie states
   0x43/0x44 (:24512/:24643), and it was ALREADY PORTED
   (`genie_eat_ball`). No retail wizard-flyover ball absorb exists;
   the wizard economy is the castle path (entry 3). mc1l0 has no
   genie, so nothing here rode on it. The dig did surface one real
   gap, now FIXED: the genie's (10,0) puff and the sparkle ring
   were missing retail's `+18 |= 1` stamp (:24793-94/:24377-78 —
   our `flags |= 0x10000`).
7. **HW terrain shortfall — GENERATOR CANDIDATE (a) REFUTED
   2026-07-31 by the `ground-audit` instrument** (mgc-conform mode:
   retail entities' rest-z vs the port's generated plane at their
   coordinates). At t=0 — before any runtime edit exists — every
   class-2 static and every (10,45) hut on the full HW take sits at
   **dz = 0 exactly** (99 samples map-wide; the mc1l0 control is
   identical, its (5,1)+512 rows being balloons at tether
   altitude). Late-tick audits localize ALL large dz to one
   contiguous castle-mound region (80-112, 160-208, ~+1000, the
   (3,2) at +2272) plus battle sites — runtime edits. So the HW z
   bulk is candidate (b) — TERRAIN CLOSURE (entry 2), capture
   domain per the standing deferral ruling; no generator fix
   exists to make, and the one-off live-DOSBox height-plane dump
   is no longer needed for this question (optional someday as a
   full-plane certifier — statics only sample where they stand).
   The old "~256 z around (56,246)" measurement came from the
   superseded partial take's walker families. Still open from the
   same triage: retail manifestations import with `+70 < 200`,
   below the port's `MANIFEST_BASE = 200` encoding, so imported
   manifestations take the resting-jar path instead of
   `manifestation_tick`.
8. **HW stat doubling — DISSOLVED (entity-substitution artifacts)**:
   "life 30 + flags 65536(0x10000)" = the port's (10,42) castle
   build painter occupying a slot it reaped (painter max_life=30,
   build bit 0x10000) — not a scaled (10,2); "mana_max 20000 vs
   10000" = the castle (3,2), whose stats are identical MC1↔HW —
   that is finding 3's castle column. retail flags 131073 = 0x20001
   (effect bit17 + active), port 65536 = 0x10000 (painter build
   bit): two different entities, no bit-shift exists. No stat-scale
   fix anywhere; closed into findings 1 and 3.

## MC1HW take-1 (2026-07-31 triage; suite conformance/mc1hwl0.json, 12 story fixtures)

41,488 pairs / 902 torn / 40,586 fixture-grade / 1 conforming (t=0)
after the reap law + the rival re-anchor. Terrain closure (entry 2's
HW face — the castle-mound region (80-112,160-208) raised ~+1000 by
late-run) z-poisons ~every pair, so progress reads from families +
the story suite. Post-fix field totals: z 1.72M (capture-dominated) ·
life 613k · flags 428k · x/y ~300k · rand 139k · heading 111k ·
mana_max 8.7k pairs · player.mana 7.6k · player.life 1.2k.

- **Rival carpet FREEZE — RESOLVED 2026-07-31 (importer defect, not
  a motion bug)**: `rival_entity_tick` keys on `self.rivals[i].ent`,
  which `retail_import_mc1` never re-anchored to the imported slots —
  every imported rival carpet was a frozen husk (obs@1 = state@0
  verbatim; the first divergence family, every pair). The port's
  motion law itself is verbatim sub_14EB0 (:18781; hand-computed one
  tick from state@0 → retail obs@1 EXACTLY: z band-settle quarter-rate
  −1, polar step sin/cos>>16, ±16/tick speed slew toward Type_160
  v_12, turn rate angdist/(8+(255−tempo)/16) clamped + overshoot
  snap). Fix: re-anchor `self.rivals` per pair (ent = play_index) +
  reseed vdes/jink/grace/mana lanes from the closure. First HW
  conforming pair.
- **Rival AI-STATE reconstruction — RESOLVED 2026-07-31** (the
  freeze entry's REMAINING): the AI record imported as state=Fresh
  with no target, so the decision cascade re-aimed f34 (target_yaw
  1477-vs-1825 on ~25k pairs) and cast choices diverged. Retail runs
  the state HANDLER before the selector (sub_13170 :17847), so state
  and a `target_alive`-surviving target must import TOGETHER or the
  tick falls back to Fresh. Decoded (all Type_160-relative, opus dig
  cited): +415 state byte (dispatch :17847; value map in
  `AiState::from_retail` — cut states 2/4/5/10 → Fresh), +404 burst
  (i16, negative lockout :17936-38), +406 poverty latch (:19468-91),
  +460/+462 hate/war per player slot (str_456, neutral 0x601F),
  +628 learn countdowns (:19409-12), +724 cooldown[24] (u16;
  triangulated: [16] = var_756 castle-build stagger). Target + site
  need NO decode: they ride the already-imported carpet entity
  (f146 tr-translated by import_ent, dest_x/dest_y); target_sig
  recomputed = retail's stored +148 exactly (sub_15420 :19041).
  Implemented as `reanchor_rival_ai` (rivals.rs) called from the
  importer's re-anchor loop. mc1hwl0: rival (3,1) target_yaw
  ~25k pairs → 320 rows (top slot 473); target_yaw total 25k→20.6k
  (rest = creature (5,x) share); rand 139k→128k (cast knock-ons);
  conforming 1→46; 8/12 story fixtures drifted shrinking (all lost
  their 3,1:target_yaw atom), promoted; mc1l0 47/47 + mc2l0 24/24
  unmoved; native goldens untouched.
- **§weather churn cadence** — the port under/over-spawns the ambient
  fire/meteor systems: (10,0) 11.6k missing / 1.4k extra (from
  t=355), (10,13) 9.1k missing (meteor showers, from t=9949), (9,9)
  3.8k/5.5k, (10,6) 2.7k/4.6k, (9,1) 1.9k/5.0k. Field-row bulk
  (life/flags/x/y) is the SAME churn as one-tick-offset lifecycle
  overlaps. Untraced; measure per model before patching.
- **(10,2) speed-token contrail — sub_56380 UNPORTED** (entry 1
  residual; 1,304 missing from t=1): the class-12 ACTIVE Accelerate
  token (state 6). Decoded 2026-07-31 (:65131-99): while +48>0 and
  `sub_55DD0` admits — owner cmd-speed v_12 = 3×(+128) on the first
  burst tick (+48==+50, also flags|=0x80 + notify 19) else 2×(+128),
  +126 = v_12, a (10,2) puff at the owner every 4th TOKEN f63 tick
  (id24 = owner's id24, act_life ×4), then sub_55E80 (the burst
  cost). At +48==0: restore v_12 = +128, clear 0x80. Port into the
  strict class-12 arm (world.rs class12_tick phase 0); sub_55DD0 and
  the owner Type_160 v_14 clamp lane still need transcription. The
  heal (sub_56270, state 3) and bolt (sub_56510, state 9 — its (9,1)
  share) arms ride the same dispatch.
- **§census 10000-vs-1000** (mana_max 8.7k pairs, from t=72): the
  claim census under a live rival castle; also rival.castle blink
  (the (3,2)@522 goes missing on 5-8 pairs — the castle state
  machine kills it) and one player.mana_max 58938 blowup (t=10705,
  a census overcount). Entangled with the rival AI-state gap.
- **§player-vitals**: player.mana 7.6k pairs (e.g. t=435 retail 0
  port 1000 — the regen floor applies while retail is suppressed
  mid-drain; the entry-5 clamp misfires on HW's token layout?);
  player.life 1.2k pairs (ambient damage share).
- **§token-blink** (t=3001-3013): the port drops the player's whole
  (12,x) owned-token roster for 13 pairs over the death window —
  the death path scatters/reaps what retail keeps banked.
- **Hands**: 61+18 pairs (quickselect law, mc1 entry 6's twin).
- **PLAYTEST (2026-07-31)**: (1) **HW SNOW GROUND — FIXED same day**
  (player report "reverted to mc1 plains"): the bundle chain was
  correct end-to-end (atlas/palette/shade-LUT all arctic; hiding
  the bundle errors; features switch with --tileset) — the defect
  was the TYPE-PAINT: the baked HW type plane was 94% type 3
  (temperate grass). Decompile-corroborated (opus dig): HIDDEN.EXE
  inserts a SNOW pass `sub_31C10(snlin, snflt)` between rock and
  majority (remc1hw :35792; height > snlin AND 4-neighbor relief
  < snflt AND land → class 6 = snow, then the shared basalt edge
  rule → class 1) AND its rock pass writes steep→class 1 not 6
  (sub_33570 :37269). CARPET.EXE never reads snlin (the old
  mc1_terrain.rs:48 claim was temperate-only truth). Ported
  arctic-gated into mc1_terrain::generate (rock steep param +
  snow pass; bake threads `Game::HiddenWorlds`); BAKE_EPOCH 22→23;
  HW:0 histogram flipped 61,452×type-3 → 63,284×type-6 (+80-83
  snowy-rock transitions), mc1:0 byte-identical, water untouched
  (snow never visits class 0 — water semantics = type 0 safe).
  Screenshot-verified snowfield; full workspace tests green;
  DDLEVELS snlin is a real per-level knob (5 on lvl0 = full snow,
  135 on lvl20 = peaks only). (2) The HOMING METEOR spell has
  always been wrong: wrong sprite (renders like a plain meteor)
  and far-too-weak combat law (retail: 3 guaranteed hits wreck
  Vodor, only rebound defends; the port's rival outheals it) —
  **RESOLVED 2026-07-31 (both defects, opus dig cited; PLAYTEST
  OWED)**. §3c dissolved: the sprite is a CODE LITERAL, not a
  descriptor-table lookup — HW swaps the m16 ctor sub_3A270
  (sprite 42, remc1:46353) for sub_3A5F0 (sprite 76 = the big
  meteor, hw:42451/:42474); SPRITE_STATS row 76's 420x350 extents
  also size the hitbox, so the port's hard-coded 42 was wrong
  look AND collision. Damage: the m16 bolt does NO direct damage —
  the state-17 handler sub_52770_52AB0 copies the bolt's +44 into
  the (10,53) cloud at delivery (hw:58859), so the cloud burns the
  ROW damage 5000 over its 6 ticks (833/tick) instead of the ctor
  3000; the port never copied → ~3000/hit vs Vodor's 10000 with
  ~20/tick regen between casts = outhealed; 3×5000 with the
  regen stall (+383=16/hit, hw:51748) = the retail 3-hit wreck.
  Both fixes HW-gated (spawn_firewall_bolt sprite,
  proj_firewall_tick copy_f44); test
  hidden_worlds_firewall_bolt_is_the_meteor_and_copies_damage
  pins both games; MC1 goldens + all 3 suites unmoved. Rebound
  uniquely defends because HW adds 53 to the model-53 reflect set
  {1,17} (hw:58806) — reflect itself still dormant in the port.
  CORRECTION: the earlier "(9,9) state-14 = meteor" guess was a
  MISID — that family is the Lightning beam segment swarm
  (spawn_zigzag one-frame segments, own=472); the meteor lane is
  (9,16) st17 + (10,53/58) + (10,0). SECOND CORRECTION (player
  push-back, same day): the meteor is RICHLY PRESENT in the take
  (3,005 (9,16) + 3,711 (10,53) diff rows from t≈727; first full
  engagement: cast t=798 slot 546 → homes on creature 183 →
  delivery t=801, cloud slot 512) — the "absent" reading came
  from the RUNNER replaying HW takes under base-MC1 law (next
  entry). CORPUS-CONFIRMED both fixes: bolt type86=76 and
  f44=5000 at birth; delivered cloud f44=5000 (the hw:58859
  copy, byte-for-byte). BANKED LEADS from the dig (INFERRED,
  verify before acting): ① base MC1's cloud should ALSO inherit
  +44 (=24464→191/tick) via the same sub_52770 copy — remc1's
  truncated class-9 table hid it; changing it moves MC1 combat
  balance + goldens, needs its own corpus/playtest pass. ②
  ~~rival at-castle grace mail-wipe "no retail basis"~~ —
  **REFUTED 2026-07-31 (the dig read the intake fn and missed
  the CALLER's gate)**: retail :17971-79 is verbatim the port's
  law (own-castle overlap sub_11950 → grace +331=2; while grace:
  memset the 36-byte mailbox, skip the intake). At-castle rival
  invincibility IS retail. The human's explicit ch0 redirect
  into the castle (:55353-62) is ported; retail has NO rival
  analog — a camping rival's castle takes damage as ordinary
  AREA-blast collateral (player testimony: "the damage is dealt
  to the castle instead"). See the playtest-round entry below
  for the REAL lead this resolves into.

## HW-LAW RUNNER FIX 2026-07-31 (the fall-through trap, new shape)

`verify::build_world` built EVERY MC1-family conformance world
with bare `World::new` = base-MC1 law — the game string selected
only the ASSET variant. **The whole mc1hwl0 triage to date ran
without SPELLS_HW, the m16 homing acquire, or the HW napalm
fork.** Fixed: `new_for_game(GameId::Mc1Hw)` for "mc1hw"
(verify.rs; serves verify, fixtures, and dump — MC2 has its own
builder). This is the mc1hw-survey durable-lesson trap in a new
shape: not an equality gate this time but a DEFAULT CONSTRUCTOR —
sweep `World::new(` call sites, not just `== Game::` tests, when
a per-game seam lands. New HW-law family baseline (full re-run):
z 1.641M · life 596k · flags 412k · x/y ~280k · rand 127k ·
max_life 112k (−16k) · heading 96.7k (−14k) · model 17.0k
(−48%: napalm life-6 fork + meteor lifecycles had been graded
against base law) · target_yaw 20.6k · player.mana 7,567
(unchanged).

Meteor engagement triage under real HW law (pairs 796-810):
- **Birth pair 797 now conforms on identity**: the acquire fires
  (chase 183, latch set, heading/pitch snap — retail 882/134 vs
  port 890/147; residue = pose-latency muzzle offset, capture
  domain). At the doctrine input-delay 2 the cast lands 2 ticks
  late (jitter — cast pairs are inherently capture).
- **Bolt f140 FIXED (both games)**: retail's emit copies the
  MANIFESTATION's +140 (hw:62371/:66151) = the ctor's
  cost-per-shot `a4/count` (:48005) — 5000/26=192 HW, 5000/51=98
  base; the port stamped the row total 5000. cast_firewall now
  computes the quotient (manifestations stay f140-unstamped =
  hash-quiet; nothing ever rewrites class-12 +140 — the castle
  ladder rewrites +136). Corpus row (mana 192-vs-5000) gone;
  the wall_of_fire test pins it.
- **NEW LEAD — cast DEBIT lands one tick late (suspected
  §player-vitals root)**: retail applies the −possess_mana
  regen-delta WITHIN the cast tick (obs: player.mana 10000→5000
  on the cast pair); the port's mana_debit writes mana_delta but
  the vitals application ran earlier in the tick, so the debit
  surfaces one pair late. MC1-wide ordering question (every
  spell, both games) — needs its own round with mc1l0 re-verify;
  candidate root for a chunk of player.mana 7.6k.
- **Fixtures**: t=797 (capture, cast story) + t=800 (open,
  delivery story: cloud delivered same-pair, f44 copy conforms;
  residue = free-stack allocation order 534-vs-512 + the
  jittered second cast) added; suite now 14 fixtures, all green;
  mc1l0 47/47 and mc2l0 24/24 untouched.

## METEOR PLAYTEST ROUND 2 (2026-07-31): mostly certified; the
## residual "Vodor tougher than retail" TRIAGED to ONE chain

Player: meteor sprite + 3-hit damage feel right; Vodor still
harder to kill than the retail playthrough ("starts healing
fast"), possess homing "feels broken" on unclaimed balls, and
respawn "way faster than retail". Adjudications:
- **Possess homing — RULED FAITHFUL, don't re-open**: retail's
  acquire case 1 gates BOTH candidate lists on `+58 != 0`
  (hw:60176/:60194) — identical to the port filter — and balls
  SETTLE to +58==0 forever after their 128-tick ballistic
  window. A settled unclaimed ball is never a homing target in
  retail either; the lob homes only on fresh still-bouncing
  balls, and old balls are claimed by aim + the possession
  flash's area-claim at the blast. Mid-flight steering is fully
  corpus-graded (every tick of every imported lob).
- **Respawn law — timer + cadence VERIFIED faithful** (formula
  32·((255−tempo)>>3)+32 at :55555-57 byte-identical; per-tick
  countdown + castle check + castle-less elimination :55601-30).
  The port's "fast respawn" is NOT a timer bug — see the chain.
- **THE CHAIN — RESOLVED 2026-07-31 (the CASTLE-COLLATERAL round,
  see Resolved for the fix inventory)**: the corpus lever paid off
  exactly as banked. Slot 522's record: castle born t=73 at life
  20000, UNTOUCHED until t=9330, then damage in runs of −833/tick
  (with −1666 overlap ticks), dead at t=9457 — 833 = 5000/6 = the
  meteor's (10,53) napalm cloud burn, and each burst is 7×833 per
  cloud (14 for two overlapped). The retail meteor bolt and cloud
  both carry chase=522: **the homing acquire locks the CASTLE
  itself** — that is how player fire aimed at a castle-camping
  Vodor fell his castle. Four port defects found and fixed (each
  corpus-validated on the 9325-9345 window; castle life diffs
  12→1): ① ent_overlap widened +78 UNSIGNED — the castle's 0xE000
  z-center marker read as +57344 instead of −8192, z-orphaning
  every castle out of the area-write pre-pass; ② the castle never
  carried the marker natively (the port skipped sub_37150's
  +78=0xE000 write *because* of ①); ③ the acquire candidate set
  lacked castles (retail's list-1 walk branches model 2 to a
  dedicated castle scorer in cases 0/3/4 AND 0x10); ④ the (10,53)
  cloud ran post-decrement — retail is class-10 PRE-decrement (7
  burns from a 6-life cloud, 5831 delivered not 5000). Remaining
  window rows = capture domain: the pair-9329 birth edge + both
  chase rows are cast-timing skew (the port's replayed cast fires
  a pair off, allocating its own bolt), and the 849 acquire miss
  is the terrain-closure z (port castle at pristine 5600 vs the
  raised mound's 7168 pushes the pitch bearing ~7 units outside
  the 0x71 cone). Exemplar fixture t=9331 added (castle intake
  CONFORMING inside it). Second-order Home/camp cadence: NOT
  re-checked this round — revisit only if a future take shows a
  camping-cadence divergence.

## MC2 take-2 (2026-07-30 re-record; FIX ROUND 1 LANDED 2026-07-30)

The re-recorded mc2l0: **11,524 ticks gapless, check-decode exact,
`channels.input: "raw"`** (the MC2 input frame validated live — mode 7,
arrow keybinds), spell upgrades + end-to-end level completion. 11,523
pairs → 7,762 fixture-grade (33% torn). Suite re-extracted per doctrine
(`conformance/mc2l0.json`, 24 exemplars, 23 open / 1 capture; sigs
re-promoted after the fix round). Post-triage it sat at 0 conforming
by construction: the §terraform capture family (village growth regrades
the hill at ~t=751; house z re-snaps both sides, ours to the pristine
plane) puts (10,45) z rows on every later pair — 186k of the 249k
then-remaining z hits. **The 2026-07-31 kinematics round moved it to
11 conforming + 8 rng-only** (total diff rows 329.9k → 300.2k).
Port-side conformance now lives in the t<751 window
and in the per-family totals, which the fix round moved hard:
player.mana 5,894→232 pairs · player.mana_max 5,939→458 · entity
mana_max 6,296→599 · player_ent_idx 6,759→out of top-20 · owner
2,724→~250 · rand 21,655→10,983 · player.castle (a fix-round
regression, then fixed) 6,083→0. Pair 0→1 = ONE row (the regen-cadence
lead below). Fix round (all in Resolved below): §class15 + spellbook
import, the @0x1A id-fusion/claim-census/economy block, the fire
activation bit + (10,0)/(10,6) field map, and the strict-retail MC2
sweep laws (newborn skip, disabled skip + ghost records, ghost slot
reallocation) — plus the tile-chain-cycle OOM guard (pair 9074's
100 GB allocation: a linked ghost's slot reallocated → chain cycle →
unbounded `area_write` victim walk).

Open leads, take-2 (verify with `--start <t> --limit <n>` windows; run
the full file under `ulimit -v` — see the pair-9074 note):

- **§effects per-model field-map grind — the dominant port residual**:
  the class-10 effect models keep per-model homes the uniform alias
  table misses, exactly like class-15 did. Landed: (10,0)/(10,6)
  (@0x2A amount → f140, @0x2C flicker/lift → f44, @0x90 dead-0).
  Remaining: small fire z residues (the sub_580E0 alt-core arg
  order?), smoke ±1-step tails, per-model rand rolls ((10,13)
  emitters, (10,12) hit-flash), (10,1) explosion cluster fields.
  Measure per model — the two-wrongs trap is real (the activation-bit
  fix EXPOSED the f44 aliasing; totals briefly rose).
  **SWEEP SLOT-ORDER LAW LANDED 2026-07-30 — the smoke families
  collapsed.** The universal "newborns never tick" gate was the t=0
  special case: retail's frame pass (EF:40116) is a bare ascending
  pointer walk — a mid-pass spawn ticks the SAME pass iff its slot
  lies ahead of the cursor. The chimney corpus pins it (9 births/
  tick, lives 31..−1, NO life-32 record ever). Gate removed; the
  natural loop serves both native and strict (DEVIATIONS.md entry
  updated — the dome guard is faithful, not a deviation). t<751:
  total rows 47.8k→34.1k, (10,14) life 6,060→308, y 6,029→1,061,
  (10,13) life/y gone. REMAINING smoke rows are capture-domain:
  newborn rand/actSpeed derive from the reused slot's STALE seed
  (SetSmoke4 steps the slot's leftover rand once; the slot's last
  ghost obs is 1-2 frames before the pair) and newborn drift reads
  stale yaw — not reachable from a single-pair closure. The extras
  (~9/pair) are the newborn capture tear (born after the recorder's
  window passes the slot; present in port's end-of-tick obs,
  absent from retail's mid-frame one).
- **§casts misfire — FIXED 2026-07-30 (the pane theory was WRONG)**:
  the recorded cursor sits dead-center (320,199) — no pane click.
  The real cause: the RIGHT BUTTON is already HELD on the
  recording's first frame (a hold crossing the level boundary), and
  the harness ring's default pre-fill read "released" →
  manufactured a press edge → the t≈3 phantom (9,17). verify_mc2
  now extends the first input frame's held state backward (retail
  latched the press before t=0; its first real edge is the t=5
  re-press). The substitution rows cleared. The --input-delay
  re-sweep 0..3 ran FLAT (<0.2% — this window barely casts);
  delay 2 stands.
- **Cross-pair StageVar leak — FIXED 2026-07-30**: the live
  StageVars2 rows @0x365F4 now decode (`RetailMc2::stagevars`, raw
  8-byte rows [kind, flags, chain, cadence, payload]) and overlay
  the port table's RUNTIME lanes per pair (kind/flags/chain/
  cadence + kind-6/7 param; loader-derived hold/watch fields stay
  from the build — the &2-clear payload can be a bound-entity
  guest POINTER (EF:4740), which the sv1 lanes already rebuild).
  The t=726 sv1/sv2 self-drift pair is now FULLY conforming. Note:
  mc2l0's recorded rows are byte-identical t=0..751+, so this
  overlay = a per-pair reset; a take with live trigger churn will
  exercise the lanes for real.
- **player.mana regen cadence — narrowed**: the pending delta @0x88
  applies mana@N+1 = mana@N + d88@N on almost every pair, EXCEPT a
  freshly-stamped −100 survives ONE extra frame before applying
  (measured pairs 0→1 and 16→17; the port applies immediately →
  ±100 on ~232 pairs). The MC1 entry-5 resolution EXPLAINS the
  mechanism (slot-order jitter between the stamping spell event and
  the carpet's apply — remc2's sub_68DE0 cost stamp is LIVE, so
  MC2 stamps −cost then applies next frame when the event's slot
  follows the carpet's). RE-MEASURED 2026-07-30: the stamp pends
  TWO recorded frames (d88=−100 at obs 0 AND 1, manifestation
  timer FROZEN between them = the recorder's mid-frame window
  catching pre-apply state), so a single-pair import cannot
  distinguish the hold from the apply — an f2e-first-tick clamp
  bought exactly one pair and was reverted. Reclassify toward
  capture unless a cleaner discriminator appears.
- **mana_max residual** (599 rows): the claim census within the tick
  — retail's t=64→65 jump (+187) lands mid-frame (a ball absorb the
  port's census sees one tick late?). Same family: owner retail-152
  rows (t=620 slot 49 — a just-learned manifestation's adopt path).
- **Completion arc** (t≈11,000+): still untriaged.
- Familiar carryovers: §terraform (capture), §wander turn law,
  §balloon (3,3) extra, §rng under load.

## MC2 open leads (mc2l0 take-1 triage 2026-07-30; fixtures retired with the take-1 recording — family shapes carry over)

Post-fix family table (2535 fixture-grade pairs, per-entity-torn
slots excluded from field comparison — see the MC2 capture caveat):
z 43128 (of which ~38k = the §terraform capture family) · rand 3874 ·
speed 2955 · x 2571 · y 2409 · mana_max 1114 · player.mana 945 ·
heading 764 · life 712.

- **§effects — the (10,13)/(10,14)/(10,0)/(10,60) fire-smoke band**
  (fixtures t=0/6/21/24): the dominant PORT residual. Lifecycle
  churn (5.7k missing / 3.1k extra (10,14); 2.3k/1.2k (10,13)) plus
  motion (speed ±4 = one decel step families beyond the torn
  residue) and draw cadence (retail (10,14) draws 0/tick with rare
  9-draw bursts, (10,60) draws 3/tick — measure per model before
  patching effects.rs). Entry point: `mc2_smoke_particle_tick` /
  `mc2_smoke_emitter_tick` vs EF:35618-35700.
- **§wander — (5,1)/(5,13) turn law: RE-RULED 2026-07-31** (see
  Resolved: KINEMATICS ROUND rulings): the law is byte-exact; the
  isolated ±22/±45 blips self-heal (capture). The REAL residual is
  the **held-state split**: on the sustained ±341 runs retail parks
  the goat in action 15 (+7 controlled, sv2=2) while the port
  wanders at 9 — the StageVar hold-gate isn't latching that
  creature (slot 81 exemplar, t≈2380-3096). Port lead, own dig.
- **§balloon — (3,3) extra-in-port** (t=1913): 549 extra balloons
  from t=1807 — the port's castle dispatches balloons retail does
  not send here (likely gated on economy state the importer seeds
  differently, or a cadence lead in `mc2_balloon`).
- **§rng — global-LCG divergence** (t=1520): 62/3640 pairs, all
  under load (draw counts high); likely a draw inside one of the
  §effects laws or spawn paths, will collapse with them.
- **§houses — (10,45) life deltas** (t=2181): +250 family (militia
  pop refund? repair?) on top of the terraform z (capture).
- **§castle — (3,2) player_ent_idx + z** (t=2204): the castle's
  sphere-owner field and z datum drift late in the run.
- **§player-vitals — player.life** (t=2347): 66 pairs of human life
  drift; partially entangled with the cast closure (capture) — the
  ambient-damage share is the port lead.
- **MC2 importer approximations (accepted, watch for families)**:
  the live StageVar table imports from LEVEL data, not the runtime
  rows @0x365F4 (FIRED/cadence bits stale across trigger ticks);
  `mc2_allied`/`mc2_aura_claim` clear at import; rival spell/XP
  columns (str_611) not imported (level 0 has no rivals); the
  scratch quartet f26/f36/f46/f50/f56 uses best-single-home
  mappings (conformance.rs `import_ent_mc2` doc) — f56 ← @0x36 was
  A/B-tested (the b38 mapping poisoned kinematics +14%).

## MC2 mc2l4 + mc2l30 triage (2026-07-31)

The first-cut triage of the two takes cut from the 2026-07-30 mc2:4
session. Four fixes landed during the round (Resolved below: worm-bob
import lane, lightning trail nodes, castle phantom-upgrade lane, the
cave ambient rand tail + turn anchor); the families here are what
remains, each with its dive verdict. Suites:
`conformance/mc2l4.json` + `conformance/mc2l30.json` (re-extracted
post-fix).

- **§l4-guard-terrain — the (5,15) castle-guard family (BOTH takes'
  #1 residual, ~170k rows l4 / ~130k l30): CAPTURE (terrain
  closure)**. (5,15) = the wizard-manager-spawned defensive archer
  guard (`sub_5FF50` EF:61488-502 stamps yaw=roll=512 + terrain-alt
  spawn z; behavior row 83 grounds it to `getTerrainAlt` every tick
  via the ported `mc2_alt_core`). Retail's guards walk up a
  runtime-terraformed castle-mound ramp (+15 z per +30 x, 512/544
  plateaus); the `.mgcr` has no terrain channel, so the port replays
  on pristine planes — z tracks the missing mound, the pristine tile
  TYPE trips the wander die-gate → action 121→124 (prekill), and the
  die-gate's early return freezes the guard's rand. One root, three
  fields; port laws verified faithful line-by-line. Rides the
  standing §terraform/terrain-channel remedy. The sv1/sv2 rows
  nearby are the SEPARATE mc2:04 death-watch/hold choreography;
  (9,13) arrow churn is part guard-downstream, part cast-timing.
- **§sphere — RESOLVED 2026-07-31** (see Resolved: KINEMATICS ROUND
  fix 4): the settle law (`byte@0x39 || kick`) + @0x2C z-vel import
  + latch imports + exact bounce/merge/rotation landed in the
  shared `ball_tick` MC2 arm. l4 (10,39) 37.9k → ~3.8k rows;
  residual = terrain-closure z + birth edges. The l30 sphere z bulk
  (−1169/−542 constants) was always the cave mound/plateau terrain
  closure — capture.
- **§l30-churn residual — the coupled fire/smoke draw+lifecycle
  family**: after the cave-tail fix ~22% of l30 pairs still
  mismatch rng, all count-mismatches on churn-heavy ticks. Two
  mechanisms: (a) ~~the MC2 per-tick reap lag~~ **RESOLVED
  2026-08-01 — the same-tick reap landed (see Resolved); the
  extra side collapsed but the 202 rng pairs survived UNCHANGED,
  so the rng residual is entirely (b)**; (b) ~~the per-ENTITY
  `rand_0x14 += counter` sites~~ **REFUTED as the l30/l4 driver
  2026-08-01 session 4** (the three sites EF:13140/13220/20521
  belong to the (5,10) doomsday pyramid and the (5,27) hydra
  branch bolt — NEITHER model exists on l30 or l4, censused
  across the takes; the perturb law itself LANDED anyway, see
  Resolved). The REAL (b) = the **VOLCANO CASCADE**: the human
  map-casts Volcano (spell 18) at t≈258-262 at (67.5,110.5) and
  (111.5,10.5) → (10,9) domes (both sides spawn them, ±2 ticks
  cast latency, slot-skewed) → dome life==3 beat → (10,18)
  summit controller (retail slot 134 @274) → (10,19) column +
  (10,16) + (9,0) + 4×(10,14) smoke ring → (10,0) fire cascade
  spreading tick-by-tick. The 202 rng pairs cluster EXACTLY on
  the eruption windows (274-468, 478-518, 2536-2776 — the SAME
  site re-erupting — plus singles 4359/4834/4866/6314-22/6490/
  7642-50/7762-78/7810/7934/7954/7970/8114-22/8330; a third site
  (201-206,0-11) at 2530-2537 emits (10,0)+(9,3)). The port
  erupts ONCE (under-sized cascade — gross missing 1,693 (10,0)
  vs 346 extras) and NEVER re-erupts. Two port bugs indicted:
  the (10,0) fire-entity spread law (also feeds l4's 2,005
  missing — no volcano there, combat ignitions) and the summit
  column re-erupt cadence. Dig round launched same session.
- **§l30-terrain — the (14,5) flat-512 plateau (CAPTURE, with a
  port-side check owed)**: 12 of the 14 (14,5) markers sit exactly
  −1664 (retail 2176 plateau at tiles (160-171,194-205), port flat
  512); nearby slots track terrain within ±32. Both sides ground-
  snap faithfully — the port's mc2:30 heightfield simply lacks the
  plateau. ~~OWED: load-time vs runtime~~ **ANSWERED 2026-08-03
  (session 6, dig C): RUNTIME-terraformed — `mc2_dome_tick`/
  `sub_31940` (EF:23193) direct heightmap writes; pure capture,
  nothing portable.** The l4 face of the same question: the (5,4) ARCHER
  family walks at a CONSTANT −192 z from t=0 (slot 210, byte-
  identical dynamics) — a pristine-plane datum gap at its site,
  present before any runtime edit can exist. (5,4) XP-scroll z, (14,3) −16, (15,19) token-fall
  (slot 92: port clamps up to its pristine 1296 floor while retail
  falls to 288) are the same terrain-closure story.
- **§castle follow-ups** (split from the resolved phantom-upgrade
  lane): (a) ~~painter @0x28 owner projection~~ **RESOLVED
  2026-08-03 (session 6, dig E — parent castle lane landed; see
  Resolved owner entry)**; (b) the (3,3) stage-piece −128 z residual post-rise —
  re-measure now that the phantom upgrade is gone; (c) the (5,1)
  at slot 92 killed at t=0 by `mc2_building_clear_tile` (build
  footprint clear) while retail's construction hasn't cleared that
  tile this tick — build-window timing; (d) player.mana_max
  claim-census within-tick (the standing mc2l0 lead, NOT a castle
  ripple — the mc2l4 castles are rival-owned).
- **§wander-drift residual — (5,0)/(5,3): RE-RULED 2026-07-31**
  (see Resolved: KINEMATICS ROUND rulings): the walker turn law is
  byte-exact — the smooth heading drift is capture (chaotic
  amplification, rand-matched). **SESSION-6 NOTE: the (3,3)
  BALLOON altitude half is RESOLVED (dig A — row-base import +
  sub_580E0 servo; see Resolved).** **SESSION-7 SCOUT (2026-08-03):
  the "multipart flyer z-bob" lead is CLOSED — capture (see
  Resolved "MULTIPART FLYER Z-BOB RULED CAPTURE"); no M0/M3
  altitude source exists to trace; roster mc2-flyer-drift-m0/m3
  flipped open→capture.** What survives as PORT-side work from
  that scout is the l4 **terrain datum gap** sizing (437 tiles at
  −23..+8 height bytes across five windows — a recording-format-v2
  terrain-channel / import question, not an entity law); plus the
  l4 t=17954 mass spawn-wave divergence (dozens of slots at once,
  unexamined).
- **§drip placement — (10,86)/(10,87) residual**: at the best
  cadence anchor (turn0&7==0, phase-scanned) the drip still lands
  9 missing/56 extra per 2000 pairs — the target-tile walk consumes
  the global stream, so any upstream rng divergence relocates the
  drip; expected to shrink with §l30-churn.
- **§lightning residual — (9,9)/(10,23) extras+missing**: the
  input-delay-2 cast-timing skew + retail's parked ghost husks vs
  the port's free-list reuse — capture-domain (the field families
  resolved, see Resolved).

## Resolved

- **⭐ SESSION-10 CLOSE (2026-08-05, THE LANDING ROUND) — authoritative
  full-take numbers on the final tree, all six takes, suites promoted
  203/203 as-expected, 0 regressions, `MGC_REQUIRE_GOLDENS=1` 0
  failures.** Every slate-A item landed or closed-by-refutation; three
  player-report digs landed on top (demon size law PLAYTEST CERTIFIED;
  camera EYE_LIFT; terrain keyframes decided). Numbers (conforming /
  unexplained field·missing·extra / rng pairs):
  - **mc2l24: 7,166 conf** (was 1,163 post-cast-phase) / 453,195·407·
    5,913 / **rng 4** (was 8). Conf-or-explained 19,579.
  - **mc2l0: 5,520 conf** (was 2,232) / 3,372·107·28 / rng 2.
  - **mc1l0: 506 conf** (was 501) / 10,585·87·101 / **rng 0**.
  - **mc2l4:** 14,709 of 17,711 fully explained (83.1%) / 10,523·116·
    15 / rng 5 (was 6). Rival split live: terrain-z 7,934 · ai-residual
    4,190 · mana-mirror 1,288 (purse-mirror decision still owed).
  - **mc2l30: 13 conf** / 6,428·70·40 / rng 18 (was 19).
  - **mc1hwl0: 49 conf** / 2.17M·27,418·9,844 / rng 28 — still
    terrain-dominated = the keyframe channel's first customer.
  - Landmark rule collapses: `mc2l24-static-terrain-z` **375,572 → 37
    rows** (prop-z inversion); `mc2-guard-terrain` residue 1,197 (l24
    doomsday ground, narrowed rule); `mc2-castle-pad-z` 17 l24 (sensor)
    + 166 l4 rival authored pads (real closure, keyframe territory);
    `mc2-terraform-houses` 1. `mc2-walker-ground-z` 233,822 = the next
    terrain lever, awaits keyframes.
  - The conforming jumps come from the compound of: 180° turn tie-break
    (both games, all takes), manifestation-slot cast order (every MC2
    cast was one tick early), prop-z terrain inversion, merge ring-walk
    + hard-free, muzzle endpoint admission, pool-base decode (session
    9), and the pad replays (session 8) grading together for the first
    time.
  - Instruments live corpus-wide: press-position fold (off by default),
    cycle-ring detector (0 hits all takes, as predicted), per-pair
    recycle/drop telemetry.

- **THE PORT FIRED EVERY MC2 CAST ONE TICK EARLY — the ARM and the
  LAUNCH are two different entities' ticks, and the port ran both at
  the caster's pool slot. The l24 "19 phantom possession bolts" are
  that one tick. LANDED 2026-08-04 (session 10, possession
  re-attribution dig).** Re-owns the divergence the session-9 entry
  parked on `mc2-rival-ai-lanes` and then had to give back when the
  corpus proved l24 is single-player ("(a) the l24 late-window
  divergence is NOT rival-attributable — look at the human/class-9
  path"). It is neither rivals nor input skew: it is retail's
  entity-walk order.
  - **THE CENSUS.** `verify-deltas --csv`, mc2l24 t=40000+4000 (the
    window the "19" was measured in): **24 extra (9,1), 0 missing**,
    all roster-swallowed by `mc2-cast-timing-extra`, at t=40123/29/34,
    40156/61/65/71/86, 40405/10, 41288/93/98, 41308/12/18/22/27/32/
    37/41/46/50/59 — a ~5-tick click cadence, and `MGC_CAST_TRACE`
    puts an aligned press edge on every one of them. So retail took
    the SAME presses. Probing the raw states says what it did with
    them: at t=40124 the right-hand manifestation (slot 9, class 15
    model 1, action 3) goes `word_0x2E_46` 0 → 3 with the pool still
    holding **zero** (9,1); at t=40125 the timer reads 2 and the bolt
    is there. **Retail armed on the press frame and launched on the
    next one.**
  - **THE LAW, AND ITS SPAWN-FREE ORACLE.** `sub_5F660` (EF:60874) and
    its arm `sub_5F7B0` (EF:60973) only stamp `word_0x2E_46 =
    duration`; they are called from the tail of `sub_5F380`
    (EF:60850-62), which is the HUMAN entity's own action
    (`AddPlayer03_00_5E010` EF:59954). The LAUNCH lives in the
    manifestation's own **class-15 action** (`sub_69640` EF:55915 for
    possess, dispatch EV:3491-92), run at ITS pool slot in the same
    ascending `UpdateEntities` walk (EF:40116). So the arm reaches the
    effect state in the same frame **iff the manifestation sits ABOVE
    the carpet**. That is measurable without looking at a single
    spawn: on the record where a timer leaves 0, `word_0x30_48 −
    word_0x2E_46` is 0 if the effect state has not run yet and 1 if it
    has. Over the whole corpus — **mc2l0 713/713 arms lag 1**
    (manifestations 153/154, carpet 152), **mc2l4 1,333/1,333 lag 1**
    (266..275 vs 265), **mc2l30 666/666 lag 1** (85/86/87/93 vs 83),
    **mc2l24 464 above → lag 1 and 3,184 below → lag 0**, with the
    only exceptions being the CASTLE spell, whose timer is an upgrade
    lock and never a countdown (castle-cost entry). Zero
    counter-examples in 6,360 arms. l24 is the odd take because its
    hands re-home into low slots after the opening (spell 1 at slot
    7/9/10, spell 0 at 6/78/79, spell 9 at 84) while carpet stays 116.
  - **THE FIX.** `World::mc2_player_cast_pass` (world.rs) now ARMS
    only; the per-manifestation effect state moved into
    `World::mc2_manifestation_pass`, called from the class-15 walk arm
    (`mc2_spell_token_tick`'s state-3M case, which used to `return`
    early because 3M "is not a jar"). `mc2_cast_tick`'s loop body split
    out as `World::mc2_manifestation_tick` (cast.rs) so both callers
    share one implementation. **Scoped to a POOLED human**
    (`mc2_carpet_slot != 0` — the conformance import): native MC2 keeps
    the human out of pool at slot 0, i.e. BELOW every manifestation,
    which is already the law's `above` arm, so the pre-walk combined
    pass stays exactly as it was and no golden moves.
    `MGC_NO_MANIFESTATION_ORDER=1` restores the pre-dig placement.
  - **A/B (one frozen binary, env-toggled, back-to-back).** mc2l24
    40000+4000: **(9,1) extra 24 → 0** with missing still 0, entity
    extras 691 → **632** — (10,12) 6 → 3, (9,28) 6 → 0, (10,75) 11 → 0,
    (10,0) 109 → 100, (9,9) 21 → 20, (9,3)/(9,26)/(10,22) 1 → 0 each —
    missing 957 → 959 ((10,23) +2), unexplained field 10,520 →
    **10,472**. mc2l30 0+2000: gross rows 39,184 → **39,038** (−146:
    134 `mc2-cast-timing-fields` `rand` rows on (9,1)/(9,10), 18
    fire-churn, 1 pose-phase; +2 unexplained (5,15) `rand`) — the
    launch moved two slots up the walk and the bolt's LCG seed now
    lands where retail's does, a second independent confirmation.
    mc2l0 0+2000: conforming 1,677 → **1,678**. **mc2l4 0+2000 and
    mc2l24 0+2000 are BYTE-IDENTICAL** — every manifestation is above
    the carpet there, which is exactly what the law predicts.
  - **THIS IS THE ±1 PHASE the session-9 entry measured**
    ("`retail_arm − port_fire` is +1 on 227 of 408 casts, +2 on 96, 0
    on 39") — that +1 was never input latency, it was this, and the
    39 zeros are the arms whose manifestation sat above the carpet.
    The proposed corpus-wide `--input-delay 3` knob is therefore NOT
    needed and must not be taken: it would model an entity-order law
    with a capture knob and would break every take where the
    manifestation is above the carpet (l0, l4, l30 entirely).
  - **IT IS NOT the mc2l0 "(10,12)-pulse vs (9,1)-bolt SUBSTITUTION"
    lead — CHECKED AND STILL OPEN.** l0's manifestations are all above
    the carpet, so this fix is a near-no-op there and the residue is
    untouched: mc2l0 0+2000 keeps **17 missing (10,12)** with zero
    (9,1) extras, byte-identical across the A/B but for one (9,10)
    extra row. That lead needs its own dig.
  - **ROSTER PROPOSAL (described, not applied).**
    `mc2-cast-timing-extra` / `mc2-cast-timing-missing` no longer carry
    a launch-phase family on any MC2 take; re-measure their hit counts
    and re-scope the notes to whatever residue survives (l24 40k keeps
    24 `mc2-cast-timing-missing` (9,0) rows, which are the RAPID
    fireball stream, a different story).
  - Test: `mc2_human_cast_arms_at_the_carpet_and_launches_at_the_manifestation`
    (world.rs — the old `..._pops_the_free_stack_after_lower_slots`
    rewritten to the two-tick geometry its own law now implies: tick 1
    arms and launches nothing, tick 2 launches from the LOW
    manifestation slot and therefore pops the free stack BEFORE the
    higher emitter's puff). Non-vacuous: under
    `MGC_NO_MANIFESTATION_ORDER=1` the first assert fails (the cast has
    already fired and expired inside tick 1). **No golden moved.**

- **THE (5,23) RETRY-LEG PAIR IS THE 180° TURN TIE-BREAK, NOT A RETRY
  ORDER BUG — retail's turn helper unwraps the angle delta only when
  it is STRICTLY past a half-turn, so an exact half-turn keeps the RAW
  sign. LANDED 2026-08-04 (session 10).** Closes NEW LEAD ② of the
  riser-endcap entry ("the 2 surviving (5,23) heading rows are exactly
  512 apart — a retry-3 / retry-2 leg disagreement worth one narrow
  dig"). The leg was never in doubt: both sides take retry 3. The
  512 is `2 × 256` = the commit turn applied in opposite directions.
  - **THE PAIR, RECONSTRUCTED TO THE UNIT.** mc2l24 t=15044 and
    t=15129, both slot 363, `field:5,23:heading` retail **1205/1287**
    vs port **1717/1799**. The dweller's stored yaw and its wander
    target (`roll_0x20_32`, the port's `f34`) are EQUAL at both ticks —
    437 and 519 — and the move core's third retry
    `(yaw0 + 0x400) & (0x700 + LOBYTE(yaw0))` (EF:8846) is the exact
    ANTIPODE for both (the mask clears nothing): 1461 and 1543. The
    commit then turns back toward the target, capped at row 91's
    `subtype_160_0x2_2 = 256` — from exactly 1024 away.
  - **RETAIL'S SIGN.** `sub_582F0` (Sound.cpp:6580; MC1's twin
    `sub_42240_42580` :52664 is the same body, and the decompile marks
    it SYNCHRONIZED): `v3 = (tgt & 0x7FF) − (cur & 0x7FF)`, unwrapped
    by ±2048 **only when `abs(v3) > 1024`** — strictly greater — then
    `v3 / abs(v3)`. At the tie `v3 = 437 − 1461 = −1024`, no unwrap,
    sign −1 → 1461 − 256 = **1205**. The magnitude helper `sub_582B0`
    (Sound.cpp:6569, MC1 `sub_42210_42550` :52652) folds on the same
    strict `> 0x400`, so it returns 1024 and the cap takes 256.
  - **PORT DEFECT.** `Gen::turn_step` derived the sign from the
    WRAPPED delta — `(tgt − cur) & 0x7FF <= 1024 → +1`. That agrees
    with retail on every delta except the one case `cur − tgt == 1024`
    exactly, where it turns +256 instead of −256: a full `2 × cap`
    error, and the antipodal retry lands on it every time it fires
    against a creature already facing its wander target. Fixed by
    porting the retail body as `Gen::turn_sign` (mc1/mobs.rs);
    `MGC_NO_TURN_TIE=1` restores the wrapped form.
  - **THE MOVE CORE IS STILL CLEAN** — the riser entry's ruling stands
    verbatim; nothing about the retry ORDER or the blocked test moved.
  - **A/B (one frozen binary, env-toggled).** mc2l24 t=14680+500:
    **(5,23) rows 2 → 0**, window gross 27,754 → **27,632**,
    unexplained field 13,429 → **13,307**, collateral ALL downward and
    all class-5 — (5,20) −74, (5,17) −39, (5,26) −7, nothing up.
    **mc1l0 0+2000: conforming 525 → 530**, conforming-or-explained
    1,428 → 1,441, unexplained field 3,391 → 3,371. mc2l0 0+2000:
    conforming 1,678 → **1,703**, gross 3,586 → 3,554. mc2l30 0+2000:
    explained 1,527 → 1,536, gross 39,038 → 39,024. mc2l4 0+2000:
    explained 1,313 → 1,318, unexplained field 4,059 → 4,049. Both
    games, every take, one direction.
  - **⚠ GOLDENS MOVE — THE RE-PIN RITUAL IS OWED AND NOT PERFORMED.**
    `turn_step` is shared creature-turn code, so fixing it is a
    deliberate behaviour change: `level_005_golden_state_hashes`,
    `flight_tier_golden_state_hashes`, `mc2_cave_behaviors_and_goldens`
    and `mc2_slice_behaviors_and_goldens` all diverge (level_005 from
    hash index 2 on). `MGC_NO_TURN_TIE=1` makes all four green again,
    and nothing else in the tree fails either way. The four re-pins are
    a player decision, exactly like the banked rival purse mirror; the
    fix is landed ON so the decision is visible rather than rotting in
    the backlog.
  - Test: `turn_step_breaks_the_exact_half_turn_toward_the_lower_angle`
    (mc2/mobs.rs) — replays both recorded pairs through the antipodal
    retry and the capped commit, pins the sign both ways round the tie,
    and asserts every non-tie delta is unchanged. Non-vacuous: under
    `MGC_NO_TURN_TIE=1` it fails reproducing the port's exact old 1717.

- **THE l24 FOUNTAIN "OVER-SPAWN" IS NOT AN OVER-SPAWN: the fountain
  is byte-exact and the extras are the MANA-SPHERE MERGE, which the
  port ran with the wrong SEARCH SET and the wrong TEARDOWN. LANDED
  2026-08-04 (session 10, fountain-over-spawn dig).** Closes the
  session-9 pool-base entry's parting call ("the (10,39) extras in the
  fountain window (673 after the fix) are a real over-spawn") — the
  premise is falsified and the real law is two decompile routines the
  port had never read.
  - **THE FOUNTAIN IS EXONERATED, BY LCG COUNT.** `sub_32CF0`
    (EF:24007, action 98) launches `for (i = 0; i < 3; i++)` spheres
    and spends **five** `9377·r + 9439` draws on each one that
    allocates (speed, apex, colour, mana, yaw — all inside `if (v1x)`).
    Probe over the corpus's own (10,91) spawner (mc2l24 slot 662,
    t=64490..64559): the spawner's `rand_0x14_20` advances by
    **exactly 15 steps on every one of the 70 ticks** — 3 successful
    creations per tick, never 2, never 0. An identity-keyed census
    ((slot, rand) over the whole pool) agrees: **3 (10,39) births per
    tick, every tick**. The port spawns 3 too. The window's 662 extra
    spheres are 2.8 retail DEATHS/tick of which the port reproduced
    only about half.
  - **DEFECT 1 — THE PARTNER SEARCH IS A MAP-TILE RING WALK, NOT A
    POOL SCAN.** `sub_10A50` (EF:3876) and its MC1 twin `sub_11D10`
    (:17127) are the same routine: base tile = `((pos + 128) >> 8)`
    — **ROUNDED**, not floored; ring count = `(applied_pitch + 255)
    >> 8` (the searcher's own +80 extent in tiles, and with **no**
    `.max(1)` — the area writers' `.max(1)` is a different routine);
    `sub_11410`/`sub_10080` seed a walker at ring 0 and
    `sub_114B0`/`sub_10130` yield each ring's tile offsets outwards;
    each tile's `mapEntityIndex` chain is walked and the FIRST hit
    admitted by (+66/+67 filter, `id != id`, AABB) wins. The port
    scanned all 999 slots by AABB alone, so it merged partners retail
    cannot see. Corpus proof, mc2l24 t=64509-11: the settled shore
    sphere slot 845 (55.973/228.99, +80 = 112 ⇒ ONE ring around tile
    56/229) does **not** absorb slot 795 when it steps to 54.98/227.98
    — the AABBs already overlap (Δ 255/260 < 112+153 = 265) but tile
    54/227 is two rings out — and absorbs it one tick later at
    55.23/228.23 (tile 55/228). Retail's mana ledger confirms the
    merge to the unit: 845 goes 141,653 → 143,966 (= +795's 2,313) at
    t=64511, then +78 as slot 828 vanishes at t=64512.
  - **DEFECT 2 — MC2's ABSORBED DONOR TAKES THE HARD FREE.**
    `sub_36D50` (EF:26919-96) is a ladder of owner-resolution arms and
    **every arm ends `return sub_57F20(a2x)`** — the hard free
    (Events.cpp:5209-39: tile unlink, recycle-stack swap-removal,
    `class = 0`, free-stack push). Nothing defers it to the disable
    sweep. The port kept MC1's hard free but soft-killed (0x400) on
    MC2 ("MC2's twin free is untraced"), so every MC2 merge left the
    donor in the pool for one more snapshot AND withheld its slot —
    which is both an extra-in-port AND a deeper free-stack pop that
    lands the tick's later spawns in slots retail never used.
  - **WHAT LANDED** (native + strict, one law, both games —
    `mc1::combat`): new `Gen::ball_merge_candidates` (combat.rs) is
    `sub_10A50`/`sub_11D10`'s ring walk, and the merge tail now calls
    `free_entity` (= `sub_57F20`) for MC2 as well. The explicit
    `class 10 / model 39` family test IS retail's +66/+67 filter —
    every ball ctor stamps `xtype/xsubtype` = (10,39) — so native
    balls, which carry no +66/+67, keep working. `MGC_NO_BALL_MERGE_FIX=1`
    restores both pre-dig halves for A/B.
  - **CONFORMANCE (windowed A/B, one frozen binary, env-toggled arms).**
    UNEXPLAINED field·missing·extra / gross missing / gross extra:
    l24 **64500+400 (the fountain)** 812·0·17 / 4 / **687** →
    809·0·17 / 15 / **574** ((10,39) extras **662 → 550**);
    l24 **61000+450 (the boss fight)** 7035·1·26 / 119 / **477** →
    6945·1·23 / 124 / **370** — the merge donors were feeding that
    window's slot pressure too, (9,9) extras **152 → 83**, (10,0)
    268 → 247; l24 30000+300 (control) 2250·0·0 / 3 / 18 → 17;
    **mc2l30 0+2000** 1765·13·41 / 65 / **149** → 1742·13·29 / 66 /
    **113** (explained pairs 1539 → 1549; (10,14) extras 37 → 21,
    (10,39) 26 → 15). Byte-identical in both arms: **mc2l0 0+2000**
    (523·17·17, 97/106) and **mc2l4 4300+300** (947·1·1, 6/20).
    **mc1l0 0+2000** is entity-set identical (75/52) and moves ONE
    unexplained field row (3390 → 3391) — the MC1 ring restriction is
    all but inert on that take. Fixture suites identical in both arms
    — mc1l0 68, mc1hwl0 29, mc2l0 41, mc2l4 24, mc2l30 24 all
    as-expected, and the mc2l24 17 (10 as-expected / 2 fixed / 5
    drifted) is the concurrent static-terrain-z dig's `field:2,2:z` /
    `field:2,3:z` drift, unchanged by this one.
  - **GOLDENS MOVED — ONE TEST, DELIBERATELY.**
    `mc2_cave_behaviors_and_goldens` checkpoints B-D (state hash and
    observable projection; the load checkpoint holds) — cave drips are
    mana spheres, so which ones coalesce and when their slots come
    back is exactly what this law changes. Re-pinned with the reason
    in place (mc2_cave.rs). Everything else green: 340 mgc-sim lib
    tests + every integration suite under `MGC_REQUIRE_GOLDENS=1`.
  - Pinned by
    `mc2_mana_merge_walks_the_tile_ring_and_hard_frees_the_donor`
    (world.rs — the corpus's own 845/795 geometry: an out-of-ring
    partner with overlapping AABBs is NOT absorbed, the same pair one
    tile closer IS, the donor ends `class == 0` and its slot is back
    on `free`). It FAILS under `MGC_NO_BALL_MERGE_FIX=1`, and the
    neutered arm reproduces the old port's exact wrong number
    (141,653 + 2,313 = 143,966 a tick early). Two existing merge tests
    (`mc2_mana_merge_takes_bigger_owner`,
    `mc2_mana_lock_survives_the_unclaimed_merge_arm_only`) now assert
    the donor by `class64`, not by the 0x400 soft-kill.
  - **WHAT IT IS NOT.** The 61k full-pool cluster is NOT one family
    with the fountain: its extras are **(10,0) 268 + (9,9) 152 of 477**
    (the boss's fire/blast churn, already captured by
    `mc2-fire-churn-m0` / `mc2-lightning-blast-churn`) against only 15
    (10,39). The merge fix helps it only through slot pressure.
  - **RESIDUALS / LEADS.** (a) The fountain window's remaining 550
    (10,39) extras are dominated by **summit merges the pristine
    terrain cannot host** — retail's early deaths cluster at
    (38-40, 213-216) z 2400..3300 on the doomsday mound, where the
    port's ball is 60 units off the ground and never grounds, so the
    merge branch is never entered. That is the `mc2l24-ball-terrain-roll`
    capture family and it belongs to the terrain-replay track; a
    60-pair join attributes 47 of 61 residual extras to slots retail
    never allocated (the downstream of those missed frees) and 11 to
    the one-snapshot decay-expiry linger. (b) 12 (10,39) missing
    remain: the port still merges a handful of SHORE-pile spheres a
    tick early — the order WITHIN one ring comes from retail's
    `bitmaps_E9980x` offset table, which the decompile does not
    carry, and raster order stands in. (c) Retail's decay expiry
    (`DisableEntityDrawing04` at life 0) leaves the record in the pool
    for exactly one snapshot with `byte[1] = 0x24` before the
    tick-top sweep frees it — the import carries that as 0x400 and the
    port's sweep matches, so the 11 `expiry(L0)` extras are pure
    slot-order downstream, not a law gap.

- **THE PRESS POSITION IS NOT THE CAST'S AIM, AND THE 0x40 BIT IS NOT
  A HAND — both landing-round cast-input items CLOSED ON THE
  DECOMPILE, 2026-08-04 (session 10, cast-input dig).** Two premises
  from the session-9 close (`mouse_press_pos recorded but UNUSED (= the
  aim the cast actually used)` and `cycle-ring hand bit 0x40`) were
  wrong about what retail does with either datum. Both lanes are now
  first-class in the format and measured; neither warrants the wiring
  that was proposed.
  - **③ `mouse_press_pos` = `x_WORD_E375C/E375E`, the ISR's
    cursor-at-press snapshot (EF:51478-97, written on the left, right
    AND middle press edges; nothing ever clears it). The poll copies it
    to `unk_18058C.x_DWORD_1805B8/1805BC` (EF:49664-65, and the three
    sibling control-mode arms 49703/49750/50423) and its ONLY consumer
    is `sub_1A7A0_fly_asistant` (PI:1988-2013) — the fly-assistant
    idle-recentre watchdog: 0x30 frames with the in-struct mouse
    unmoved and no pending action → `HandleButtonClick_191B0(39, 0)`.**
    The aim/attitude command is a different register: the input frame's
    `roll`/`pitch` (bytes 3/4) come from the LIVE cursor
    `x_DWORD_1805B0_mouse` ← `x_WORD_E3760` via
    `ComputeMousePlayerMovement_17060` (PI:643/1007/925 → PI:2100-41).
    And the cast itself takes NO aim at all: `sub_5F660` (EF:60874)
    is called `(caster, manifestation, hand-flag)` at all three sites,
    with the launch direction read off the caster entity's own pose —
    which `verify_mc2` already pins byte-exact from the recording
    (`carpet_pose_mc2`). **There is no aim to wire in.**
  - **③b The one place the datum could still bear on a cast — a
    sub-poll press oracle — was A/B'd and is STRICTLY WORSE.** The
    snapshot changes only on a press edge, so a change between records
    proves a press the latch lane may have missed. Measured against
    retail's own arm oracle (the equipped hand manifestation's
    `word_0x2E_46` 0 → nonzero) over the FULL mc2l0 take, 8,626 pairs,
    731 retail arms: **the landed latch law catches 728/731** (3 misses,
    t=3867/4144/4479; 201 armless edges = mana refusals, possess
    re-presses that raise `byte_0x3C_60` instead of the timer,
    cave-only refusals), **the press-position edge catches 480/731 with
    354 changes that arm nothing** — and it catches none of the latch
    law's 3 misses. End-to-end windows agree: mc2l0 0+2000 conforming
    **1,677 → 1,563** under the fold (unexplained field 523 → 513,
    conforming-or-explained 1,886 both ways — it trades 114 clean pairs
    for 10 roster-absorbed rows); mc2l4 3300+600 unexplained field
    **130 → 143** (extra 1 → 0, and two brand-new capture rules appear
    — `mc2-cast-timing-extra` 11 rows, `mc2-cast-timing-missing` 10 —
    i.e. manufactured casts); mc2l24 51500+600 headline unchanged
    (2,239 field / 1 missing / 140 extra both ways) but the fold adds
    270 rows of `mc2-fire-churn-m0` (3,481 → 3,751), the same
    manufactured-cast signature absorbed by a capture rule. LANDED OFF
    behind `MGC_PRESS_EDGE=1`
    (`verify_mc2::press_edge_mc2`, mirrored in `fixtures.rs` so a suite
    run under the toggle matches the triage run) purely to keep the
    result reproducible and to stand as the fallback if a recorder
    change ever costs us the latch.
  - **④ THE 0x40 BIT IS A THIRD CAST LANE, NOT A HAND SELECTOR.** The
    carpet's dispatch tail fires three lanes off `str_164->
    entityIndex_0x0` (EF:60851-62): `& 0x10` →
    `sub_5F660(carpet, SpellEnabled[SpellIndexLeft], 256)`, `& 0x20` →
    `…[SpellIndexRight], 512)`, and `& 0x40` →
    `…[spellIndex_D94FF[spellIndex_0x458_1112]], 256)`. So 0x40 casts
    the RING PANE's CATEGORY CURSOR through the LEFT hand-slot flag,
    consulting neither equipped hand — the shortcut that fires a ring
    spell without equipping it. `spellIndex_D94FF` (GameUI.cpp:59) is
    the identity over 0..25 (the three tail cells 0/3/0 are pane
    padding). It is raised at exactly one site, PI:880-84: the ring
    pane open (`MenuState_0x3DF` 5 or 8, the PI:806 branch), no equip
    pending (`byte_0x457_1111 == 0`, PI:836/842), no SHIFT (PI:856),
    and **BOTH press latches up** (`MouseButtonState & 1 && & 2` — bits
    0/1 are the ISR latches `x_WORD_180746`/`180744`, EF:49676-79). The
    dispatcher writes `spellIndex_0x458_1112 = byte1` first
    (EF:37626-27), so the cast reads the cursor the click just picked.
  - **④b UNREACHABLE ON THE WHOLE CORPUS — measured, not assumed.**
    Both press latches are never up in the same record in ANY MC2 take:
    mc2l0 0/8,626, mc2l4 0/17,711, mc2l24 0/69,220, mc2l30 0/9,337
    (both buttons are not even HELD together except 10 mc2l24 records,
    all outside the pane). The pane IS visited with the cursor moving
    (mc2l0 201 records, mc2l4 277, mc2l30 93; `hand_pending` reaching 1
    on all three), so the gate is live code this corpus simply never
    trips. LANDED as a DETECTOR, `verify_mc2::ring_cast_mc2`, default
    on / `MGC_NO_HAND_BIT=1` off: it reproduces retail's gate from the
    recorded state and prints the tick + spell if a take ever raises
    it, so the lane can never be silently dropped again. Verified a
    pure no-op — mc2l0 0+2000 is byte-identical with the toggle either
    way.
  - **PORT GAP, CONSCIOUSLY DOCUMENTED (not landed).** `mc2_cast_input`
    (cast.rs:1028) still models only the two hand bits, so a player who
    both-clicks inside the ring pane gets no cast. Wiring it needs a
    third `PlayerCommand` lane plus the ring CURSOR (the port carries
    `array_0x3B5` ring MEMBERSHIP, not `spellIndex_0x458_1112`), i.e. a
    real sim change with no corpus to verify it against — deferred
    until a take reaches the gate, at which point the detector above
    will say so. Related and separately open: mc2l0 0+2000 carries 2
    `player0.hand_left` rows (t=1750 retail `Some(2)` port `Some(0)`) —
    the ring pane's EQUIP path (`byte_0x457_1111` → PlayerAction 40 /
    31 / 32, PI:806-91) is unmodelled too.
  - **FORMAT.** `mgcr::Ext` gained `latch` and `press` (the `latch_b64`
    / `press_b64` raw registers the recorder has written since
    2026-07-30 and the decoder was dropping on the floor), and
    `RetailPlayerMc2` gained `menu_state` (+0x3DF), `hand_pending`
    (+998+1111) and `ring_cursor` (+998+1112) — the three fields the
    0x40 gate needs. Additive; `Ext` has no other consumer in the tree,
    so MC1 is untouched (mc1l0 0+2000 re-run unchanged: 525 conforming
    / 3,391 unexplained field / 48 missing / 27 extra, and the MC1 arm
    never reaches any of this dig's code).
  - **⚠ CORRECT THE FIELD MAP.** The recorder note and
    `tools/mc_dosbox_recorder.py:199-201/321-22/1452-54` both gloss
    `0xE375C` as "cursor-AT-PRESS (the cast's aim, game-snapped)". The
    provenance is right, the purpose is wrong — it is the
    fly-assistant watchdog's datum. `0xE3760` (live cursor) is the aim
    source.
  - Tests: `mc2_press_position_decodes_from_the_recorded_frame`,
    `mc2_press_move_can_carry_a_cast_the_latch_missed` (non-vacuous —
    the neutered `moved = false` arm must NOT manufacture a cast),
    `mc2_ring_cast_bit_needs_the_pane_and_both_latches` (nine
    coordinates, every neutered one refuses). Suites: the four
    non-mc2l24 MC2/MC1 manifests + mc1hwl0 ran as expected;
    conformance/mc2l24.json's 2 fixed + 5 drifted are the concurrent
    static-terrain-z / recycle-allocator sim work in the shared tree
    (the drifted atoms are `field:2,2:z` / `field:2,3:z` /
    `field:player.mana`), NOT this dig — nothing promoted.

- **THE STATIC GROUND PROBES — `.mgcr` has a terrain channel after
  all, hidden in the pool: every class-2 prop's `z` IS retail's own
  ground sample, and inverting the sampler ENDS
  `mc2l24-static-terrain-z`. LANDED 2026-08-04 (session 10,
  static-terrain-z dig).** Cashes session-9's NEW LEAD ①, and
  **refutes its pyramid hypothesis**: the l24 onset is not the (5,10)
  flatten, and the pyramid is not even in the pool at the onset tick.
  - **THE ONSET, TRIAGED FIRST (as instructed) AND IT IS FIRE.** The
    whole 375,572-row family is **8 entities**: the (2,2) dolmen at
    (128,112) and 7 of the 12 (2,3) props of the stone ring at
    (223-231, 233-241) — × the tail of a 69,207-pair take. The ring's
    props peel off one at a time, t=23411/25/26/27/27/35/37, as the
    player's METEOR barrage walks over them: `sub_32880` (EF:23834)
    seeds a ring of (10,0) fire children per tick out to ring 10
    (~6 tiles), and each fire's FIRST acting tick digs the ground once
    — `sub_30D50` EF:22741, `sub_572C0(fire, 0, 0, -(rand % 7), 1)`,
    the protected single-cell scorch (`sub_56F10` EF:39499). The
    dolmen's own −96 landed before t=3573 the same way. Every one of
    those fires despawned within ~30 ticks, so the dig history is
    **LOST BY CONSTRUCTION** — the pads entry's own "any crater whose
    caster despawned" class. There is nothing to replay.
  - **THE LAW THAT SAVES IT.** Three MC2 class-2 handlers end their
    tick with a bare ground read on an entity that NEVER MOVES:
    `AddStatue02_01_65040` (EF:62519, the (2,1) statue),
    `AddDolmen02_02_65080` (EF:62534, the (2,2) dolmen) and
    `sub_65110` (EF:62545, the (2,3) prop) all finish
    `position.z = getTerrainAlt_10C40(&position)` — our
    `Gen::ground_z` ≡ `sub_724C0` :81516. So each prop's recorded `z`
    is retail's interpolated ground height at a known position,
    sampled by retail's own sampler on the tick before the import: a
    **one-sample terrain channel per prop, which the recorder captured
    without knowing it**. `crates/mgc-sim/src/mc2/probes.rs` inverts
    that sampler (`mc2_static_ground_reconstruct`, :178) over the ≤4
    height cells the sample reads — one-cell solves by two binary
    searches on the branch-monotone corner, the uniform `32k` shift
    (`comp` is a form in height DIFFERENCES, so shifting every
    influential cell moves the sample exactly `32k`), then a bounded
    tilt search (:279). Hook: `world/conformance.rs:971-980`, LAST
    after the castle/building pads and the risers, so a prop standing
    on a replayed pad solves to a no-op. Toggle
    `MGC_NO_STATIC_TERRAIN_REPLAY=1`.
  - **THE BLAST-RADIUS RULE is the whole difference between this arm
    helping and hurting** — the point-sample descendant of the pad
    replay's off-footprint fence. A sample UNDER-determines its tile
    (2 corners on even parity, 3 on odd), so most exact assignments
    are wrong, and what decides is who else stands on those cells:
    `mc2_ground_reader_cost` (:120) scores every height cell by its
    live readers, an entity already at EXACTLY its own ground counting
    1,000 (a WITNESS that the cell is already right) and any other
    reader 1; the solve takes the smallest `(blast, edit)`. Measured
    on the dolmen, whose −96 can go −3/−3 across its corners or −6 on
    the far one alone: the naive split cost **38 new unexplained rows**
    at t=23300+300 (it broke the byte-exact (11,2) switch at (127,111),
    which shares the near corner) and **+981** at t=3560+440 (it moved
    a (5,3) body chain's head, and the chain amplified it across 12
    followers). The switch also PROVES the right answer independently:
    retail's digs only lower ground, so a witness reading the pristine
    `h(127,111)+h(128,112)` sum means neither moved, which forces the
    dolmen's whole −6 onto (129,113) — exactly what the reader cost
    picks with only the pool in hand.
  - **CONFORMANCE — windowed A/B, same binary, replay off/on.**
    mc2l24 t=60000+300: `mc2l24-static-terrain-z` **2400 → 0**, z rows
    3642 → 1242, every other rule and the entity sets byte-identical,
    UNEXPLAINED 2700 → 2700. t=40000+300: static-terrain-z **2400 →
    0**, z 4365 → 1965, nothing else moves. t=23300+300 (the onset):
    static-terrain-z **1512 → 12** (7 pairs — the ticks where the
    ground genuinely moves DURING the pair, which is now what the pair
    tests), `mc2-fire-churn-m0` 4103 → 4042, z 8086 → 6524,
    UNEXPLAINED 4257 → 4257. **THE ONE COST, reported honestly:**
    t=0+4000 conforming **1175 → 1212 (+37)**, conf+explained 3220 →
    3219, static-terrain-z **430 → 1**, fire-churn 3374 → 3315, z rows
    12992 → 12549 (pairs 2439 → 2224) — but UNEXPLAINED **5410 → 5538
    (+128)** and `mc2-walker-ground-z` 1225 → 1270. Row-level on
    t=3560+440: **519 rows fixed** (427 `(2,2) z`, 59 `(10,0) z`, 33
    (5,3)) against **206 new**, all of them the same (5,3) body chain
    reacting to the now-correct ground under the dolmen (48 z, 47 y, 37
    pitch, 35 x, 32 heading + 7 (5,9) z). Net −313 rows and +37
    conforming pairs. Regression probes **byte-identical** in both
    arms: mc2l0/mc2l4/mc2l30 t=0+2000, plus mc2l4 t=17000+300 and
    mc2l30 t=9000+300 (their class-2 props stand on unmodified ground
    for the whole take, so the arm is a no-op there — it is l24's dig).
    Runtime cost unmeasurable (t=60000+300: 60.7 s off, 62.6 s on).
  - **WHAT THIS IS NOT, stated plainly.** It is not a replay of the
    digs, and it recovers the RESULT at the sample points only — from
    the very field `mc2l24-static-terrain-z` grades. **That rule's row
    count is therefore not independent evidence for this arm, and it
    stops being a sensor for anything but the arm itself.** The
    independent evidence is the neighbours: 59 `(10,0) z` rows and 33
    (5,3) rows that the corrected plane FIXES, and the (11,2) witness
    that pins the dolmen's split. What the arm buys is not a number:
    without it every pair compares a STALE baseline — the port's prop
    stands on authored ground from the tick the dig landed to the end
    of the take, wrong by the same constant, testing nothing. With it
    the pair tests what a pair is for: whether the imported TICK moves
    the ground under the prop the way retail's did.
  - **SUITES.** mc2l0 41/41, mc2l4 24/24, mc2l30 24/24, mc1l0 68/68,
    mc1hwl0 29/29 all as expected, **0 regressions anywhere**. mc2l24
    17 ran, **0 regressions, 2 FIXED** (t=3588 Capture → conforming,
    t=10062 Open → conforming), 5 drifted — and **every drift is pure
    signature SHRINK**: `field:2,2:z` and `field:2,3:z` dropping out at
    t=13330/15288/51556/51751/64000, nothing added. Promotion owed,
    NOT applied. `cargo test -p mgc-sim` with `MGC_REQUIRE_GOLDENS=1`
    green, 339 lib tests, **no golden moved** (import-only).
  - Tests ×2, non-vacuous by construction:
    `static_ground_reconstruct_restores_the_sample_under_a_dug_prop`
    digs a 5×5 scorch bowl under a live prop, imports the prop's
    sample onto a pristine plane and demands the sample back, with the
    FENCE leg (every cell outside the sampled corners identical, plus a
    witness count so it cannot pass vacuously), a non-pristine
    assertion and an idempotence leg;
    `ground_probe_blast_radius_spares_a_witness_cell` runs the same
    deficit with and without a witness on the shared corner and demands
    the correction move to the OTHER corner — the no-witness leg is the
    non-vacuity.
  - **ROSTER RE-SCOPE PROPOSED (described, not applied — central
    re-scope owns it).** `mc2l24-static-terrain-z` should be re-noted
    as a post-reconstruct residual: whole-take it should fall from
    375,572 to the handful of pairs where the ground moves within the
    tick. `mc2-walker-ground-z` is **untouched** by this arm (its
    234k rows are creatures on scorched ground all over l24, not on
    the 8 probes) and gains ~45 rows in the early window.
  - **NEW LEADS.** ① `dig_scorch` (mc1/combat.rs:4214) digs the
    UNROUNDED cell `(x >> 8, y >> 8)`, but MC2's `sub_572C0`
    (EF:39712-39723) walks rings 0..0 around the ROUNDED cell
    `((x + 128) >> 8)` — the MC2 fire scorches a cell up to one off in
    each axis. It is shared MC1 code, so the fix needs an MC2 seam and
    MC1 evidence of its own; conformance-invisible per-pair (the plane
    is pristine every pair) but it deforms a NATIVE 69k-tick run's map.
    ② The same probe-inversion technique is the only handle on
    `mc2-walker-ground-z` and would need MOVING probes (the (5,3)
    heads are `z == ground_z` exactly, per `mc2-flyer-drift-m3`); a
    least-squares plane fit over a whole pair's ground-clamped
    population is the shape, and it is a much bigger and much more
    speculative arm than this one. ③ The l24 dolmen's ground OSCILLATES
    1184/1152 every tick from t≈13333 — something re-digs and re-raises
    (128,112)/(129,113) on alternate ticks for thousands of ticks;
    untriaged, and the reconstruct tracks it correctly either way.

- **THE MC2 ALLOCATOR'S FULL-POOL ARM: the port had NO recycle-victim
  path at all, and now has retail's — but the measurement that came
  with it FALSIFIES the lead that asked for it. LANDED 2026-08-04
  (session 10, recycle-victim dig).** Closes session-9's NEW LEAD (a).
  Law gap closed; **zero conformance movement, and that is the
  finding**: on the whole corpus retail never once takes this arm.
  - **THE RETAIL LAW (three routines, one stack).** `NewEvent_4A050`
    (Events.cpp:561-608) pops the FREE stack first (:563-79) and only
    with `dword_0x35 < 0` falls through to the recycle stack
    (:581-605) — the opposite priority of MC1. The victim arm is a
    **bare seizure, not a death**: `SetMapEntity_57E50` (tile unlink),
    `class = 0`, then the same 168-byte memset + `NewEvent` defaults
    the free arm runs (:589-604). No damage, no kill credit, no
    corpse, no parent notify, and the slot **never visits the free
    stack** — it goes straight to the new occupant. The ranking is
    `sub_49F90` (Level.cpp:1271-1302): reap every `byte[1] & 4`
    record through `sub_57F20` (:1276-80), then reset BOTH tops
    (:1281-83) and rebuild them in ONE descending 999→1 scan (:1284-
    1301) — live + `byte[2] & 2` (our `flags & 0x2_0000`) pushes to
    the recycle stack, class-0 pushes to the free stack. Descending
    pushes ⇒ **the stack top is the LOWEST-numbered victim**. The
    third routine is the removal: `sub_57F20` (Events.cpp:5209-39)
    pulls a dying sacrificable entity OUT of the stack by linear
    search + **swap-with-top** (:5220-34, order-destroying below the
    hole) before pushing its slot free — without it the allocator
    would hand one slot out twice. Refresh cadence: EF:39396 (level
    generate), EF:60101, EF:61275-79 (literally "free stack empty ⇒
    `sub_49F90` ⇒ retry"), and every save/load path *empties* it
    (`sub_49F90(); dword_0x11e6 = -1;` — Level.cpp:304-305/:423-424,
    EF:38829/:38874/:39467).
  - **THE MEASUREMENT THAT KILLS THE LEAD** (probe over all 104,898
    MC2 corpus snapshots). The 74 mc2l24 full-pool snapshots are real
    — and **every one of them has an EMPTY recycle stack**, so retail
    drops those spawns exactly like the pre-dig port did. The two
    conditions never meet anywhere: mc2l24 has 371 snapshots with a
    victim list and its **free stack never drops below 85** while one
    exists; mc2l4 has 2,851 and never below **696**; mc2l0/l30 have
    no victim list at all and never fill. Adjacent-tick recycle
    transitions on l24: 237 unchanged, 120 reordered (the `sub_57F20`
    swap-removal signature — e.g. mc2l4's stable tail
    `[146, 343, 368, 332, 249, 346]`, NOT ascending, so retail's own
    list is post-rebuild mutated), 14 clean shrinks (all to zero =
    the load/save `dword_0x11e6 = -1` reset), 14 grows. **The port's
    new fallback fires 0 times in every window measured.**
  - **WHAT LANDED** (native + strict, one law): `Mc2Recycle`
    (features.rs:940 — `stack` bottom-up, LAST pops first, hash-quiet
    while empty so no golden can move) on `Gen::mc2_recycle`
    (:604); `new_event` (:1228) is now
    `free.pop().or_else(mc2_recycle_pop)` — the shared body's
    existing `unlink` + `Ent::default()` already IS retail's victim
    teardown, so the fallback only chooses the slot;
    `mc2_recycle_pop` (:1294) skips cells that are no longer live
    victims and, natively, re-ranks once via `mc2_rebuild_recycle`
    (:1331 = `sub_49F90`'s victim half, verbatim); `free_entity`
    (:1414) gained `sub_57F20`'s swap-with-top removal. Native MC2
    arms the list in `World::new_full` (world.rs:1228); the strict
    import carries the RECORDED stack with `refill` clear
    (conformance.rs:685-700) so replay sacrifices retail's victims in
    retail's order and starves exactly where retail's list ran out.
    The refresh CADENCE is the one native gap — logged in
    docs/DEVIATIONS.md (`Gen::mc2_rebuild_recycle`). The stack is
    deliberately NOT saved (features.rs `snap_write`): retail's own
    load empties it.
  - **CONFORMANCE (windowed A/B, one frozen binary, env-toggled arms
    via `MGC_NO_RECYCLE_VICTIM=1`).** Every window is **byte-identical
    between arms**, as the measurement predicts — l24 61000+450 (the
    36-snapshot full-pool cluster; UNEXPLAINED 7035 field·1
    missing·26 extra, 119 missing-in-port / 477 extra-in-port), l24
    62950+300 (the 27-snapshot cluster; 4852·2·45, 40/293), l4
    4300+300 (the recycle window, and the import DOES carry 6 victims
    there from t≈4400; 947·1·1, 6/20). Cross-take regression probes
    mc2l0 and mc2l30 at 0+2000: byte-identical, and MC1/HW never
    touch the arm. Fixture suites identical in both arms — mc1l0 68,
    mc1hwl0 29, mc2l0 41, mc2l4 24, mc2l30 24 all as-expected; the
    mc2l24 17 (10 as-expected / 2 fixed / 5 drifted) belongs to the
    concurrent static-terrain-z dig (its rows are exactly the
    `field:2,2:z` / `field:2,3:z` lanes), not to this one. 337
    mgc-sim lib tests + every integration suite green under
    `MGC_REQUIRE_GOLDENS=1`; **no golden moved**.
  - **WHAT THE FULL-POOL PAIRS ACTUALLY SHOW.** The new telemetry
    (`World::take_recycle_seized` world.rs:3216, printed per pair by
    verify_mc2.rs:182 beside `take_pool_exhausted`) reports up to
    **238 dropped spawns in a single pair** around t=61117 — with
    **0 victims available**, i.e. retail was equally starved. And the
    window's entity sets run 477 extra-in-port against 119 missing:
    at a full pool the port is **over**-spawning relative to retail,
    never under. Pool starvation is not a missing-entity source on
    this corpus.
  - **WHAT IT IS NOT.** Not the l24 slot-desync residue (that lives
    in the early take, t=3569/13330). Not the (10,39) fountain
    extras. And **not** "the import filters live victims out so the
    port fails to spawn where retail sacrifices" — the import filter
    is correct and the premise's second half never happens.
  - Pinned by `mc2_full_pool_sacrifices_the_recorded_recycle_victims_in_order`
    (conformance.rs — a full imported pool sacrifices 300, 500, 700
    in the recorded order, the seized slot skips the free stack, the
    4th alloc returns None, and a normally-dying victim leaves the
    stack by swap-with-top; FAILS under `MGC_NO_RECYCLE_VICTIM=1`,
    which is the pre-dig port) and
    `mc2_full_pool_sacrifices_the_lowest_ranked_victim_natively`
    (world.rs — victims 640/210/480 are seized 210→480→640, and the
    in-line neutered arm returns None).
  - **INSTRUMENTS LEFT BEHIND.** `dump-state` finally prints the MC2
    free/recycle stack tails (main.rs:275 — the MC1 arm has printed
    them since session 4; the ledger's tooling note at the top of
    this file claimed both), and every MC2 verify pair now reports
    `N recycle victim(s), M spawn(s) dropped` when either is nonzero.
  - **NEW LEADS.** (a) The full-pool clusters (l24 t≈58707, 60188,
    61004-61434, 62963-63231) are a **479-entity over-spawn** window
    — the port fills the pool with entities retail does not have.
    That, not starvation, is what to dig there. (b) If a future take
    ever records a full pool WITH a live victim list, this arm
    becomes measurable for the first time; the seizure telemetry is
    already wired to say so.

- **THE MC2 RIVAL AI-LANE DECODE: the wizard-extension brain half is
  mapped and imported — and the (3,1) remainder turns out NOT to be
  decisions (2026-08-04).**
  - **The lane map.** The rival's brain lives in `type_str_164`,
    EMBEDDED in the per-player block at **+998** (`m2::PP_FLIGHT`;
    remc2 `dword_0x3E6_2BE4_12228`, and `str_611` is that struct's
    +611 — which is why the already-decoded book lanes are at
    block +0x649…+0x81D). Every remc2 field name carries its own
    offset twice (`word_0x159_345` = 0x159 = 345), all block-relative
    to +998. Decoded (`crates/mgc-formats/src/mgcr.rs`
    `RetailPlayerMc2`, verified against mc2l4):
    | +998+ | lane | writer/reader |
    |---|---|---|
    | 449 | `byte_0x1C1_449` **AI state** | dispatch `sub_12910` EF:5252; selector writes it EF:5517-70 |
    | 418 | `word_0x1A2_418` **burst** (counts shots up, goes NEGATIVE for the lockout) | gate EF:5947, walk-back EF:5358 |
    | 420 | `word_0x1A4_420` poverty latch | EF:7191-7205 |
    | 516+8c / 518+8c | **hate / war** per colour — 8-BYTE records based at 516, i.e. retail's `array_0x1FC_508[4c+4]`/`[4c+5]`, NOT a flat array at 508 | decay EF:5377-92, readers EF:6201/6257/7363, respawn truce EF:43839/43850 |
    | 871 (26×u16) | **AI recast cooldowns** `str_611.array_0x367_871x` — a SECOND per-spell array right after the manifestation table at 819 | EF:5364-70 |
    | 578/580/582/586 | aggression / perception / reflexes / Life scalar | think cadence EF:5460 |
    | 1116/1117 | combat-weave dir / phase | EF:7469 etc. |
    | 1118/1119 | water-steer FSM / exit | |
    | 58 | `CastleEntityIndex_0x3A_58` — see the DECODE BUG below | 21 brain sites |
    Target and site do NOT live here: they ride the wizard ENTITY
    (`word_0x96_150` + signature `word_0x98_152`, `axis_0x9A_154x`;
    EF:6114-15), already imported by `import_ent_mc2`.
  - **Landed** (import-only, no native law touched): `mgcr.rs`
    decodes the lanes; `World::reanchor_mc2_rival_ai`
    (`mc2/rivals.rs`) + the call in `retail_import_mc2`
    (`engine/world/conformance.rs`) seat state (via the new
    `Mc2AiState::from_retail`), target, RAW signature, site, burst,
    poverty, cooldowns, hate/war, weave, avoid, personality, Life —
    plus the rival SPELLBOOK (`spell_ent`/XP/levels/sel/ring), which
    was the frozen-husk bug's twin: `book.ent[s]` kept the fresh
    world's manifestation slots and after import pointed at whatever
    entity the closure had put there. The signature is imported RAW,
    not recomputed (MC1's re-anchor recomputes): it IS retail's
    staleness detector, so recomputing silently revives a dropped
    target. Death rewrites `SpellsEnabled[s]` to the boolean marker
    **1** (EF:60147) — imported verbatim, quirk included, because
    retail's own dead-window reads index the pool with that 1.
  - **Measured (mc2l4, the ONLY take in the corpus with rivals —
    see below).** A/B on identical code via a temporary import gate:
    window 8000-9000 `mc2-rival-ai-lanes` 1190 → 1057 rows / 733 →
    692 pairs, and inside it speed 85 → 44, heading 52 → 29, rand
    61 → 39; window 0-1500 3914 → 3896; window 5400-6600 1282 →
    1269 (x/y/heading were already clean there). Full take (with the
    round's other landings): 14,823 → **14,842 explained pairs**,
    unexplained field **11,184 → 10,247**, extra 72 → 64, missing
    87 → 90, rng 6 (flat). mc1l0 full take **501 conforming, rng 0 —
    unregressed**; mc2l0 full take 2,232 conf / 3,032 field / rng 2.
  - **THE HEADLINE NEGATIVE RESULT: the (3,1) bucket is not made of
    decisions.** Per-field census of the surviving (3,1) rows:
    window 5400-6600 → z 1019, mana 179, mana_max 26, pitch 24,
    everything else ≤ 4; window 8000-9000 → z 659, mana 90, life 50,
    speed 44, x/y 86, rand 39, heading 29, action 25. So **~80% is
    terrain-closure z** (the rival hovers over its own castle and the
    altitude clamp `EF:5482-86` reads the port's pristine+replayed
    heights: the l4 t=0 rival sits 2304 units off — the castle-pad
    stamp — while the 5400 window is a flat ±3 offset) and **~14% is
    a missing ENTITY MIRROR** (below). The decision lanes were worth
    ~11% of the bucket, and only in the combat window.
  - **TOP PROPOSAL (native, NOT applied — this dig was import-only):
    the MC2 rival's mana/regen never reaches its entity.** Retail
    keeps the wizard's purse ON THE ENTITY (`a1x->mana_0x90_144 +=
    a1x->manaRegen_0x88_136`, EF:5423, and every cast debits it
    there); the port keeps it in `Mc2Rival::{mana,mana_max}` and
    NOTHING writes `ent.f140`/`f136` back. The obs projects the
    ENTITY, so every rival's `mana` reads as "the imported value,
    frozen" — retail +1000/tick at its castle vs port +0, and a
    cast that drops retail 16840 → 1000 leaves the port at 16840.
    One mirror at the tail of `mc2_rival_alive` closes it; it WILL
    move goldens (an `Ent` field write), so it needs the re-pin
    ritual. Same shape, smaller: the `life` family (retail's rival
    takes damage at t≈8229 that the port's at-castle grace discards
    — check `rival_castle` resolution first, see the decode bug).
  - **DECODE BUG FOUND (documented, deliberately NOT moved):
    `m2::PP_CASTLE = 1080` is the WRONG LANE.** The real
    `CastleEntityIndex_0x3A_58` is at block **+1056** (= `PP_FLIGHT`
    + 58): on mc2l4 +1056 tracks the live (3,2) slots (297/304, and
    it follows raze/rebuild — p2 goes 304 → 318 → 0), while +1080 is
    dead 0 for every player of every sampled tick of every take. The
    RECORDER captures the same wrong offset, so the stored
    `obs.players[].castle` is a constant 0 AND `verify_mc2` PINS the
    port's projection from it — the compare is vacuous, a blind spot
    rather than a false positive. Moving `PP_CASTLE` would break
    `check-decode` against the whole corpus (RECORDING.md lockstep),
    so the truth is exposed as `PP_CASTLE_TRUE` /
    `RetailPlayerMc2::castle_ent` and the fix is owed to the recorder
    + a re-record. Retail's AI READS that stored index at 21 sites;
    the port re-derives it by owner scan (`rival_castle`).
  - **CORPUS FACT that re-attributes two banked leads: mc2l0, mc2l24
    and mc2l30 have NO rival wizards at all.** `player_count` is 1
    for every tick of all three takes (l0 8,627 / l24 69,221 / l30
    full scans) and none has a class-3 model-1 entity at t=0; only
    mc2l4 has AI players (colours 1-2, `is_ai` set). The importer
    eliminates every port-side rival record when the slot is absent
    (`reanchor_mc2_rival(ri, 0, …)`), and the pool import overwrites
    all 1000 slots, so a port-only rival cannot act. Therefore:
    (a) the "l24 late window: retail casts ZERO possession bolts,
    port casts 19" divergence is **NOT rival-attributable** — look
    at the human/class-9 path; (b) the "(3,1) frozen husk on mc2l0"
    label is a mis-file (the family is mc2l4, as the kinematics-round
    entry says); (c) the roster rule's `takes: [mc2l4, mc2l30]`
    deserves a re-check — l30 has no (3,1) at t=0.
  - **ROSTER PROPOSAL (described, not applied):**
    `mc2-rival-ai-lanes` is class/model-scoped, so it absorbs every
    (3,1) row and is now badly named for what it holds. Split it:
    keep an `mc2-rival-terrain-z` (the z majority, same family as
    `mc2-castle-pad-z`/`mc2-archer-ground-z`), add
    `mc2-rival-entity-mana-mirror` for the mana/mana_max/life rows,
    and let `mc2-rival-ai-lanes` shrink to the residual
    heading/speed/rand/action rows — or retire it if the mirror
    lands and the residue proves to be terrain only.
  - **Tests:** `mgcr::tests::mc2_wizard_ext_ai_lanes_decode_off_str_164`
    (synthetic image; the non-vacuity leg proves the hate array is
    NOT flat at +508 and that the per-player stride is respected) and
    `engine::world::tests::mc2_rival_ai_reanchor_replays_the_recorded_target`
    (two mana balls on opposite bearings; the re-anchored rival faces
    the RECORDED one and its imported recast cooldown ticks 10 → 9,
    while the un-anchored twin re-runs the cascade and aims
    elsewhere). `MGC_REQUIRE_GOLDENS=1 cargo test -p mgc-sim` green,
    no goldens moved.
  - **Open after this dig:** the ±3 z offsets (terrain closure) and
    the pitch family (`got(t) == want(t−1)` on the aim-pitch lane,
    ~25 rows/window — the rival skips retail's aim update on those
    ticks, EF:6803). Both are small and neither is a decision lane.
  - **Decompile lead, NOT resolved:** the hate decay reads
    `array_0x1FC_508[4*i]` (= 508+8i) as the addend while writing
    `[4*i+4]` (= 516+8i) — i.e. `hate[c] = agg + 1 + hate[c-1]` as
    literally transcribed. The port implements the sane
    `hate[c] += agg + 1`. Either remc2 mis-indexed one operand or
    retail has an off-by-one-record bug; the import now dominates it
    per pair, so it is conformance-quiet — but it must be settled
    against the raw binary before anyone trusts native hate pacing.

- **THE HUMAN'S MC2 CAST IS AN ENTITY-WALK EVENT, NOT A PRE-PASS —
  and that, not any "(10,14) re-arm", was the slot-order corruptor
  behind the whole mc2l0 `(10,14)` extra family (2026-08-04).**
  - **THE PRIOR HYPOTHESIS IS REFUTED, ON THE DATA.** The cast-phase
    entry below read mc2l0 t=28's specimen as "retail's dying (10,14)
    at slot 206 (`life -2`) RE-ARMS IN PLACE". `dump-state` says
    otherwise: retail's slot 206 at t=28 is at (78.56, 220.74, 7222)
    and at t=29 it is at (77.63, 218.38, 4866) with a fresh
    `life 31/32` — a DIFFERENT particle in a RECYCLED slot, not a
    re-arm. The retail handlers agree: `sub_32160`/`sub_322A0`
    (EF:23572/:23613, the (10,13)/(10,14) particle ticks) have ONE
    death path, `if (life-- < 0) { DisableEntityDrawing04_57F10;
    return; }` — byte[1] |= 4 and nothing else. **There is no re-arm
    arm anywhere in the class-10 smoke family**; the tick-top reap
    (`UpdateEntities_57730` EF:39948-56 → `sub_57F20`) frees it, and
    the port already matched that exactly.
  - **THE REAL LAW (EF-cited).** `sub_57F20` (Events.cpp:5209) pushes
    the freed slot onto the LIFO free stack (`pointers_0x246
    [++dword_0x35]`) and `NewEvent_4A050` (Events.cpp:561) pops it, so
    **who allocates FIRST inside the tick decides who gets the
    recycled slot**. Retail arms and fires the human's spells from the
    human's OWN class-3 dispatch, mid-walk:
    `AddPlayer03_00_5E010` (EF:59954) → **`sub_5F380` (EF:60748),
    whose tail IS the cast gate** — the three `sub_5F660` calls at
    **EF:60850/:60855/:60859** (left hand / right hand / the cycle-ring
    hand) — then `sub_5EFA0` (EF:59989), then the mover `sub_5D530`
    (EF:59994). The port ran `mc2_cast_input` + `mc2_cast_tick` as a
    PRE-PASS, ahead of the whole ascending walk, i.e. as if the human
    sat at slot 0.
  - **THE SPECIMEN, SOLVED TO THE SLOT.** mc2l0 t=28→29. Disabled at
    t=28: slots **122-129 and 206** (all `flags 0x20404`, `life -2`).
    Tick-top reap pushes them ASCENDING → stack top 206, then
    129…122. The nine chimney emitters (10,60) sit at slots
    **113-121** and each spawns one (10,14): emitter 113→**206**,
    114→129, 115→128, 116→127, 117→126, 118→125, 119→124, 120→123,
    121→**122** — every one confirmed by matching the particle's x/y
    to its emitter's tile. The human is slot **152**, so his bolt
    allocates AFTER all nine and lands on **453**, off the deep
    stack. The port's pre-pass cast took 206 for the bolt and shoved
    the ninth puff out to 453 — one slot of rotation across the whole
    ring, which is exactly the `(10,14) 0 / 125` shape.
  - **CHANGES.** `crates/mgc-sim/src/engine/world.rs`: new
    `World::mc2_player_cast_pass` (:1685) = `mc2_cast_input` +
    `mc2_cast_tick`, the human's own body; `tick()`'s MC2 spell block
    (:1902) now calls it only when `mc2_carpet_slot == 0`, and the
    ascending walk's carpet-slot hook (:2016) calls it at the human's
    slot, BEFORE `mc2_cave_carpet_tail` (retail's own order:
    sub_5F380 tail → sub_5EFA0 → sub_5D530). Pane selection
    (`mc2_select_spell`) stays pre-walk — it is PlayerInput's, not the
    entity's.
  - **NATIVE IMPACT: NONE, BY CONSTRUCTION.** `mc2_carpet_slot` is
    written only by `retail_import_mc2` (conformance.rs:556); native
    MC2 has no pooled human and leaves it 0, so native takes the
    unchanged pre-walk path (a human at slot 0). No `DEVIATIONS.md`
    entry covered the pre-pass placement — it was an unrecorded
    accident of harness ordering, not a ruled deviation.
    **Goldens: NONE moved** (`MGC_REQUIRE_GOLDENS=1 cargo test
    -p mgc-sim` = 333 lib + all integration green, 0 re-pins).
  - **A/B (one binary, arm neutered/restored in place, same tree,
    windowed 0+4000).** **mc2l0**: raw conforming **2,032 → 2,232**
    (+200); unexpl field 2,026 → **1,889**; rng 3 → **2**;
    `mc2-cast-timing-fields` **5,215 rows / 704 pairs → 1,301 / 457**;
    entity sets 101/140 → 101/**138**. **mc2l4**: unexpl field
    4,568 → **4,354**; extra 45 → **43**; entity sets 69/110 →
    70/**90**; cast-timing-fields 4,553/1,007 → **3,483/935**; rng 6
    (unmoved). **mc2l24**: conforming 1,163 → **1,175**; unexpl field
    5,454 → **5,410**; entity sets 538/813 → 538/**802**;
    cast-timing-fields 2,565/470 → **1,985/448**; `cast-timing-extra`
    99 → 111 (the one counter-move). **mc1l0 0+4000 UNCHANGED** — 501
    conforming / 6,524 unexpl field / rng 0 (MC1 cannot reach the
    changed code).
  - **FULL TAKE mc2l0** (8,626 pairs, vs the SESSION-8 post-cast-phase
    baseline): **2,232 conforming** (was 2,032) / **3,032** unexpl
    field (was 3,236) / 33 missing (was 32) / **42** extra (was 43) /
    **rng 2** (was 3).
  - **SUITES: 0 REGRESSIONS ANYWHERE.** mc2l0 41 fixtures — **1 FIXED
    (t=28, the specimen itself)**, 0 drifted. mc2l4 24 — 2 drifted,
    both SHRINKING: t=3449 loses its entire 14-row `field:9,1:*` slot
    substitution (→ `field:15,19:z field:3,3:z`), t=4233 loses
    `field:3,1:z`. mc2l24 17 — 1 drifted (t=3559 `extra:10,0` →
    `extra:9,0`). mc2l30 / mc1l0 / mc1hwl0 clean. **`--promote` NOT
    run** (conformance/*.json was re-frozen today; the promotion is
    owed to a central pass).
  - Pinned by `mc2_human_cast_pops_the_free_stack_after_lower_slots`
    (world.rs tests): a (10,60) chimney one slot BELOW the carpet slot
    must take the free stack's top and the bolt the next. Neutering
    the placement (cast back to pre-walk) flips both and the test
    fails — verified.
  - **NEW LEADS.** (a) Retail ticks each spell MANIFESTATION at its
    own pool slot (`sub_693F0` EF:55831, the class-15 3M action), not
    in book order from the wizard; the port still runs all 26 from
    `mc2_cast_tick`. Immaterial on today's corpus (manifestations are
    contiguous right above the human: mc2l0 153-154, mc2l4 266-291,
    mc2l24 117-135), but a jar picked up mid-level adopts the TOKEN's
    slot, which can be anywhere — worth a per-slot dispatch when a
    take shows a low-slot manifestation. (b) The surviving
    `mc2-cast-timing-fields` residue on mc2l0 is now dominated by
    slot substitutions where retail holds a (10,12) claim pulse and
    the port a (9,1) bolt (t=348 slot 165) — a possession-lane
    question, adjacent to the OPEN `mc2-claim-census-manifest`.
    (c) `sub_5F380`'s tail also fires the CYCLE-RING hand
    (EF:60859, `entityIndex_0x0 & 0x40` → `spellIndex_D94FF
    [spellIndex_0x458_1112]`); the port's `mc2_cast_input` models only
    the two hand bits (0x10/0x20).

- **SESSION-8 CLOSE (2026-08-04, authoritative full takes on the
  final tree; suites 6/6 green, 203/203 fixtures as-expected after
  a reviewed `--promote` pass — 10 promoted incl. mc2l0 t=737, the
  3 attributed mc2l0 regressions re-statused open with notes;
  NOTHING COMMITTED — player handles git).** Seven digs landed this
  session: (10,12) claim pulse + claim-probe gate, riser-endcap
  terrain replay, importer ghost double-push + (5,10) summon
  stride, tier-0 (9,1) possession bolt + fov launch lift (fool
  OPEN-5 closed), conformance cast-EDGE harness fix, `.mgcr`
  pool-base validated recovery (free+recycle stacks), BUILD00 pad
  replays (castle mound + village terrace); plus the mc1:49 O
  ruling (retail-confirmed). Corpus close:
  - **mc2l24** 67,391 grade: **1,030 conforming** (was 4) /
    19,266 conf-or-explained / 459,142 unexpl field (was 733,635)
    / 409 missing (was 980) / 6,034 extra / rng 10 (was 12).
  - **mc2l0** 8,626: **1,771 conforming** (was 479) / 7,414
    conf-or-explained / 3,206 unexpl field / 34 missing / 40
    extra / **rng 3**.
  - **mc2l4** 17,711: 0 raw-conf / **14,811 fully explained
    (83.6%)** / 11,283 unexpl field / 87 missing / 81 extra /
    rng 10. **mc2l30** 9,337: 13 conf / 7,224 conf-or-explained /
    7,322 unexpl field / 78 missing / 97 extra / rng 19.
  - **mc1l0** 5,873: **501 conforming** (was 450) / 4,214
    conf-or-explained / 10,632 unexpl field / **rng 0**.
    **mc1hwl0** 39,199 grade: 49 conf / 2.17M unexpl field
    (terrain-channel domination unchanged).
  **SAME-DAY ADDENDUM — CAST-PHASE LAW LANDED (player-ruled "build
  in the one correct mapping"; see its own Resolved entry): the
  pair takes its END record's command; the ISR press LATCH bit
  resolves frame ownership (`aligned = (held && !latch) ||
  latch(r−1)`; delta 0 on 4,814/4,815 casts corpus-wide; the old
  "+1 early" reading was nearest-arm ALIASING — the port was 3-4
  pairs LATE). Harness-only (verify_mc2 + fixtures; MC1 has no
  latch register, verify.rs untouched). Post-law close: mc2l0
  **2,032 conf** / rng 3 · mc2l24 **1,163 conf** / 405 missing /
  rng 8 · mc2l4 rng **6**, extra 72 · mc1l0 UNCHANGED to the
  digit. Suites re-promoted 203/203, 0 regressions (mc2l0 t=32/
  291 — this morning's re-statused pair — now genuinely
  conforming).**

- **THE BUILD00 PAD REPLAYS — the castle mound and the village
  terrace are pure functions of imported state, and replaying them at
  import ENDS the `mc2-guard-terrain` family. LANDED 2026-08-04
  (session 9, terrain-replay dig).** The riser entry's NEW LEAD ①
  ("point the same technique at the other terraform roots") cashes:
  MC2's two BUILD00 stampers both end their progressive lerp on a tick
  that divides by 1, so their terminal map is ABSOLUTE and depends only
  on the stamper's cell, its BUILD00 row and its build datum — all
  three of which the `.mgcr` already carries.
  - **THE CASTLE LAW.** `sub_5FBD0` (EF:61188) spawns the (10,42)
    painter AT the castle's `axis_0x9A_154` and copies the castle's
    `dword_0x10_16` (the LEVEL) into its `byte_0x46_70` (EF:61189).
    The ctor `sub_4AA40` fills `axis_0x9A_154.z = 32 *
    sub_48E60(...)` — the perimeter MINIMUM ground over the row-1
    footprint (EF:33399) — and it is the ONLY (3,2) ctor, so every
    castle carries its datum there. The painter reads it back off its
    own position (`v40 = position.z >> 5`, EF:27775) and writes
    `height += (pad + datum − height) / countdown` per cell of BUILD00
    rows `1..=level` (EF:27846-56); `countdown == 1` on the last rise
    tick makes the terminal height exactly `pad + datum`. Every
    level-up spawns a fresh painter over the same cumulative
    footprint, so the LAST one reproduces the whole history. Import
    homes: anchor `x`/`y`, datum `site_z` (@0x9A.z → `dest_z`), level
    `f26` (@0x10). **The tell**: mc2l4 slot 330 is the human's
    water-sited castle at (154,34) — `dest_z` 0 (perimeter min 0, it
    was built on a shore), level 5, retail z 4160; the port's castle
    ground-snapped to **0** because the pristine plane reads height 0
    there and nothing had ever stamped the mound.
  - **THE VILLAGE LAW.** `ApplyTerrainModification_37240` (EF:27181)
    is the same shape with `life` as divisor (EF:27341-44): 30 frames,
    the last dividing by 1. On the final frame it parks the building
    (action 51 → 52), stamps `axis_0x9A_154 = position` and only THEN
    overwrites `position.z` with the ground — so a parked building's
    build datum survives in `site_z` and its BUILD00 row in
    `byte_0x46_70` (`f71`). Both imported.
  - **THE FIX.** `crates/mgc-sim/src/mc2/pads.rs` — conformance-import
    only, native untouched: `Gen::mc2_castle_pad_reconstruct` (the
    terminal form of the painter, rows `1..=level` overlaid later-row-
    wins, first-tick flat-nibble promotion, `countdown 2`/`countdown
    1`/settle bit3↔bit7 dance, last-tick texture pass) and
    `Gen::mc2_building_pad_reconstruct` (drives the REAL
    `mc2_building_tick` for the frames already run — `max_life` when
    parked, `max_life − act_life` mid-construction — with the
    footprint kill suppressed and the entity row restored afterwards).
    `retail_import_mc2` runs castles, then buildings, then the risers
    (`world/conformance.rs`; a castle build purges the buildings inside
    its footprint, so a surviving building never overlaps a castle pad,
    and l24's risers sit in compounds no pad reaches). `MGC_NO_PAD_
    REPLAY=1|all|castle|building` is the A/B toggle.
  - **THE OFF-FOOTPRINT FENCE is the whole difference between this
    helping and hurting.** The building's final frame runs two
    pad-edge smoothing rings (`sub_48A20` EF:32348) anchored on the
    top-left corner MINUS the half extents, so they reach a full
    footprint-width PAST the pad; over ground the baseline plane had
    already settled that second 3×3 average is pure damage. Measured:
    without the fence, ONE 1-unit re-smooth at (71,166) — the top band
    of the 23×11 building at (82,180) — cost **all 291 conforming
    pairs** of mc2l0 t=700+400. Snapshotting the heights outside the
    footprint and putting them back turns the same window into **291 →
    377 conforming**. Inside the footprint the replay is idempotent by
    construction (the lerp lands the absolute target before the rings
    re-smooth). A majority-vote "is the terrace already there?" gate
    was tried and REJECTED — it gives back the 377 (291) without
    recovering the early-window cost.
  - **CONFORMANCE — windowed A/B, same binary, replay off/on.**
    mc2l4 t=4000+300: `mc2-guard-terrain` **2220 → 0**,
    `mc2-castle-pad-z` **900 → 0**, `mc2-terraform-houses` **300 → 0**,
    `mc2-balloon-z` **300 → 0**; raw z **3965 → 870**, rand 257 → 32,
    action 227 → 2, x 129 → 42, y 106 → 47, heading 39 → 10;
    UNEXPLAINED 112 → 112, rng unchanged, entity sets unchanged,
    **nothing up**. mc2l30 t=2400+300: guard-terrain **5095 → 0**,
    terraform-houses **900 → 0**, castle-pad-z **300 → 0**,
    archer-ground-z 4292 → 4231; UNEXPLAINED 369 → 369. mc2l24
    t=5000+300: castle-pad-z 300 → 0, balloon-z 66 → 0, UNEXPLAINED
    7 → 7. mc2l24 t=25000+300: guard-terrain **6605 → 0**,
    `mc2l24-castle-piece-terrain-z` **2217 → 5**, terraform-houses
    300 → 0, castle-pad-z 300 → 0, walker-ground-z 2532 → 2335,
    balloon-z 183 → 165, splash-churn 38 → 21; UNEXPLAINED field
    **4394 → 4019**, extra 2 → 1. mc2l0 t=700+400: conforming
    **291 → 377**, `mc2-terraform-houses` **1024 → 0**, fire-churn-m0
    319 → 70, UNEXPLAINED 103 → 103. **THE ONE COST**, reported
    honestly: mc2l0 t=0+400 conforming **241 → 219** (`conforming +
    explained` 398 both ways — the 22 pairs move into
    `mc2-cast-timing-fields`, 43 → 74 pairs). It is the BUILDING arm
    alone (castle-only measures 241) and it is the authored village:
    the baked `.mgcl` heightfield already carries those terraces, and
    the port's own construction law lands them a unit or two
    differently. Net on mc2l0 across both windows: **+64 conforming**.
  - **FULL TAKE mc2l24** vs the session-9 baseline (15 conf / 19,135
    conf-or-explained / 494,747 unexplained field / 464 missing /
    6,038 extra / rng 10): **1,030 conforming**, 19,266
    conf-or-explained, UNEXPLAINED field **459,142 (−35,605)**,
    missing **409**, extra **6,034**, rng **10**. Whole-take rule
    counts after: `mc2-guard-terrain` **1,199** (corpus 1.81M before),
    `mc2l24-castle-piece-terrain-z` **804** (367k before),
    `mc2-castle-pad-z` **17**, `mc2-terraform-houses` **1** (37k
    before), `mc2-walker-ground-z` 234,015, `mc2l24-static-terrain-z`
    375,572 (untouched — see the lost list). A concurrent
    possession dig shares the tree, so part of the full-take delta is
    not this fix; the windowed A/Bs above are the isolated
    measurement.
  - **FIXTURE DRIFT (promotion owed, NOT applied).** All of it is
    signature SHRINK — `field:3,2:z`, `field:10,45:z`, `field:5,15:*`
    dropping out. mc2l4 24/24 drifted (was 19 as-expected), mc2l30
    23/24 drifted + 1 fixed, mc2l24 12/17 drifted + 2 fixed, mc2l0
    7 fixed. ONE new regression: **mc2l0 t=138 `field:9,1:z`** (a
    projectile spawn-z on the authored village terrace — the same
    early-window cost above; mc2l0 t=32/t=291 were already regressed
    by the concurrent dig). MC1/MC1HW suites unmoved. `cargo test -p
    mgc-sim` with `MGC_REQUIRE_GOLDENS=1` green, 333 lib tests, **no
    golden moved** (import-only).
  - **PROPOSED ROSTER RE-SCOPES (described, not applied).** Retire or
    re-note `mc2-castle-pad-z` (17 rows left) and `mc2-terraform-
    houses` (1 row left); narrow `mc2-guard-terrain` to `mc2l24` and
    re-triage its 1,199-row residue (it clusters where
    `mc2l24-static-terrain-z` does — the doomsday family, not the
    castle mound); re-note `mc2l24-castle-piece-terrain-z` and
    `mc2-balloon-z` as post-replay residuals. Every one of these
    rules is now a REGRESSION SENSOR for the pad replay — a hit-count
    jump means the reconstruct stopped firing.
  - Tests ×2, both non-vacuous by construction (each asserts the
    replayed map is NOT pristine, so a neutered arm fails):
    `castle_pad_reconstruct_rebuilds_the_mound_two_painters_left`
    lives a castle through two level-up painters and demands the
    replay rebuild that map from the terminal row alone (plus an
    idempotence leg); `building_pad_reconstruct_rebuilds_the_hut_
    terrace` does the same for a parked hut and adds the FENCE leg
    (every off-footprint cell the live rings wrote must come back
    unchanged, with a witness count so the assertion cannot pass
    vacuously).
  - **LOST BY CONSTRUCTION** (triaged, not attempted — the source
    entity is gone, so the pool holds no evidence): a DEMOLISHED
    castle's un-stamp residue (`RemoveCastleStage_385C0` EF:28071
    subtracts the pad back with a per-cell entity-LCG jitter, and
    nothing anywhere saves the original ground); a FINALIZED (10,18)
    volcano dome (`mc2_dome_tick`/`sub_31940` EF:23193 — the l30
    summit plateau, already closed by session-6 dig C: at t=0 the only
    live dome is mid-grow at a DIFFERENT site while the summit already
    reads 2624); any crater whose caster despawned.
  - **NEW LEADS.** ① `mc2l24-static-terrain-z` (375k, untouched) is
    the DOOMSDAY family and it may yet be replayable: the pyramid's
    flatten (`mc2_pyramid_attack` → `sub_56F10` EF:39499) is a
    deterministic ring expansion driven to a FIXED POINT (repeat
    `radius = 15 − f26` passes until the radius-7 disc is all
    type 0), and the (5,10) pyramid persists in the pool with its
    phase bits (`f44 & 8` expanding, `& 4` done) — a finished crater
    is reproducible by iterating the same loop from the pristine
    plane. But the measured l24 (2,3) signature is a CONSTANT per-slot
    delta from t=23411, which is a one-shot edit, not the progressive
    flatten — triage the onset event first. ② The mc2l0 t=0 cost says
    the baked `.mgcl` village terraces and the port's own
    `mc2_building_tick` disagree by a unit or two; a direct
    plane-vs-replay diff at t=0 would say which is retail's, and
    would also settle whether the authored terraces belong in the
    generator or in the first 30 ticks. ③ `mc2-walker-ground-z`
    (234k on l24) survives the pad replay — its remaining root is the
    same doomsday/volcano ground as ①.

- **THE RECORDER'S SNAPSHOT STRADDLES RETAIL'S INPUT POLL, AND THE
  PRESS LATCH SAYS WHICH SIDE — so the ±1 cast phase is not a delay
  knob, it is a PER-PRESS bit the recording already carries. The MC2
  arm now derives the cast phase from it and ignores `--input-delay`
  entirely. LANDED 2026-08-04 (session 9, ±1 cast-phase dig).**
  Successor to the cast-EDGE entry below; **retires its "`--input-delay
  3` absorbs the dominant +1" reading, which was an ALIASING artifact**
  (nearest-arm matching against a 4-6-tick press cadence while the port
  actually fired 3-4 pairs LATE — traced live with `MGC_CAST_TRACE=1`).
  - **THE FRAME ORDER, CITED.** `DrawAndEventsInGame_47560`
    (EF:31724): `PaletteChanges` → `sub_715B0` →
    **`ReadGameUserInputs_89D10` (EF:31734)** → **`MouseAndKeysEvents_
    17A00` (EF:31763)** → **`PlayerEvents_51BB0` (EF:31796, `Turn++`)**
    → `UpdateEntities_57730` ×`speedIndex` → draw → the native limiter
    spin (`InGameLoop_47320`). **Both the poll and the button consume
    run at the TOP of the frame, before `Turn++` and before the entity
    pass.** The poll rebuilds `MouseButtonState_18059C` from the ISR
    registers (EF:49675-83: bit0/1 = the press LATCHES @0x180746/
    0x180744, bit2/3 = held @0x18074C/0x18074A);
    `HandleMouseButtons_18F80` consumes bit 0 and clears it
    (PI:2043-49, family `byte_0x3B_59 == 1`; PI:2050 = the
    `bit0 || (bit2 && armed)` repeat arm); the tail of
    `MouseAndKeysEvents_17A00` (**PI:1049-52**, LABEL_306) then drops
    the LATCH REGISTER itself whenever the matching MouseButtonState
    bit is down — i.e. **the latch dies in the very frame that
    consumes it**.
  - **THE RECORDER'S SAMPLING POINT.** `build_record`
    (tools/mc_dosbox_recorder.py) reads the input registers from the
    same parked window as the struct, and MC2 records are parked in the
    settled tail (after the entity pass, inside MC2's own limiter
    spin). So record `r`'s registers are read AFTER frame `r`'s poll
    and BEFORE frame `r+1`'s. A press visible at record `r` may
    therefore belong to EITHER frame — and the latch resolves it:
    **latch still up ⇒ frame `r` did not poll it ⇒ frame `r+1` will.**
  - **THE LAW.** The input frame `r` actually consumed is
    `aligned(r) = (held(r) && !latch(r)) || latch(r-1)`, and the pair
    `(r-1 → r)` — which IS frame `r`'s transition — must carry it, with
    `aligned(r-1)` as its edge predecessor. In harness terms the pair
    takes its **END** record's command, not a delayed copy of its start
    record's.
  - **THE MEASUREMENT (port-independent: recorded registers vs retail's
    OWN arm ticks, the hand manifestation's `word_0x2E_46` 0 →
    nonzero).** Raw held edges split **308 / 95** on mc2l4 0+4200
    between "arm on the same record" and "arm one record later" — and
    the split is EXACTLY the latch bit: **latch=0 ⇒ delta 0 (308/314),
    latch=1 ⇒ delta +1 (95/95, no exceptions)**. Under `aligned`, arm
    records land on a rising edge with **delta 0 on 4,814 of 4,815
    right-hand casts corpus-wide** (mc2l24 2,778/2,778, mc2l4
    1,074/1,075, mc2l0 556/556, mc2l30 406/407) and 1,558/1,607
    left-hand (the residue is retail's own repeat arm — a HELD button
    re-arming without a new press, which the aligned LEVEL still
    serves). Latch runs are **always exactly one record long** (584 +
    594 runs on l24), which is the PI:1049-52 clear seen from outside.
  - **CHANGES.** `verify_mc2.rs`: new `raw_input_mc2` (held/latch,
    unmerged) + `align_cmd_mc2` (the law, with the derivation in its
    doc-comment); the run loop computes `aligned` per record and hands
    each pair `(cmd_now, pcmd)` instead of `(pcmd, prev_cmd)`;
    `MGC_CAST_RING=1` restores the legacy `--input-delay` ring for A/B
    and `MGC_CAST_TRACE=1` prints the port's cast pairs.
    `fixtures.rs`: the MC2 loop reconstructs identically (no ring) —
    the suite MUST match `verify-deltas`. **No sim change; MC1
    (`verify.rs`) untouched — it has no latch register.** Fixture
    bundles need no re-extract: the `t-(input_delay+2)..t+1` window
    already carries the two leading records `aligned` needs.
  - **A/B (windowed, one binary, env-toggled arms, back-to-back).**
    mc2l4 0+4000: entity sets **559/532 → 179/169**, (9,1)
    **261/327 → 37/47**, (10,12) 141/59 → 37/18, unexpl field 4,620 →
    **4,591**, rng 10 → **6**, `cast-timing-missing`+`-extra` 663 →
    **125**. mc2l0 0+4000: **raw conforming 1,771 → 2,032**, entity
    sets 504/540 → **101/140** ((9,1) 241/131 and (10,14) 0/125 both
    off the board). mc2l24 0+4000: **conforming 1,030 → 1,163**,
    unexpl field 5,641 → **5,454**, extra 19 → 16. mc2l30 0+4000:
    unexpl field 3,636 → **3,321**, missing 30 → 22, extra 73 → 65,
    entity sets 504/771 → **200/487**.
  - **FULL TAKES (aligned, vs the session-8 close above).** **mc2l0**
    8,626: **2,032 conforming** (was 1,771) / 3,236 unexpl field
    (+30) / 32 missing (−2) / 43 extra (+3) / rng 3. **mc2l4** 17,711:
    **14,823 explained** (was 14,811) / **11,184** unexpl field (−99) /
    87 missing / **72 extra** (−9) / **rng 6** (was 10). **mc2l24**
    67,391 grade: **1,163 conforming** (was 1,030) / 19,263
    conf-or-explained / 459,635 unexpl field (+493) / **405 missing**
    (−4) / 6,103 extra (+69) / **rng 8** (was 10). **mc1l0
    UNCHANGED** — 501 conforming / 10,632 unexpl field / rng 0, the
    baseline to the digit.
  - **THE FIELD-ROW RISE IS AN ACCOUNTING ARTIFACT, and it exposed the
    next lead.** A cast that is now phase-correct but lands in a
    DIFFERENT POOL SLOT stops being 1 missing + 1 extra row and becomes
    ~15 field rows. Specimen mc2l0 t=28 (the take's first cast): the
    port's bolt is byte-identical to retail's — (9,1) life 9/10,
    pos (78.19, 221.04, 5160), mana 33, action 1 — but sits at slot
    **206** while retail's sits at **453**. Cause: retail's dying
    (10,14) at slot 206 (`life -2` at t=28) **re-arms IN PLACE** at
    t=29, while the port frees the slot and re-allocates, so the free
    stack hands the bolt 206 and the respawned (10,14) 453. That is a
    (10,14) respawn law, not a cast law — and plausibly the root of
    mc2l0's whole `(10,14) 0 / 125` extra family.
  - **RESIDUE.** The remaining (9,1) rows are RIVAL casts, not the
    human's: on mc2l4 the survivors cluster at map positions
    (118,164), (54,237), (132,188) — nowhere near the human carpet —
    and belong to `mc2-rival-ai-lanes` (the un-imported AI decision
    lanes), exactly as the l24 late-window finding predicted.
  - Pinned by `mc2_pending_latch_defers_the_cast_one_record`,
    `mc2_sub_poll_click_casts_once_on_the_consuming_record`,
    `mc2_consumed_press_casts_on_its_own_record` and
    `mc2_long_hold_is_one_aligned_edge` (verify_mc2.rs) — the first two
    assert the LEGACY merge's edge index alongside the aligned one, so
    they fail if the old mapping is restored.
  - **SUITE DRIFT owed to a central `--promote` (fixture JSON out of
    remit):** mc2l0 **4 FIXED** (t=32, 79, 156, 291 now conforming) +
    2 drifted (t=28 the slot-swap specimen above, t=60 loses its
    `extra:10,14`); mc2l4 4 drifted (t=491, 520, 3407 lose their
    `extra:9,1`/`missing:9,1`; t=3449 becomes the slot-swap shape);
    mc2l24 1 drifted (t=2868); mc2l30 / mc1l0 / mc1hwl0 **clean**;
    **0 regressions anywhere**.

- **THE POSSESSION OVER-FIRE WAS NOT A MISSING SUPPRESSION LAW — THE
  PORT'S CAST *EDGE* WAS DEAD IN THE HARNESS. `prev_cmd` was read from
  `prev` AFTER `prev.take()` had already emptied it, in BOTH verify
  loops and BOTH fixture loops, so it never left its seed and
  `edge = cmd.fire && !prev_fire` degenerated to the raw HELD level for
  every run ever measured. LANDED 2026-08-04 (session 9,
  possession-over-fire dig).** Closes the bolt-launch-lanes dig's
  parting lead ("THE RESIDUAL IS AN OVER-FIRE, NOT AN IDENTITY";
  full-take l24 (9,1) 355 missing / 3,794 extra) and **REFUTES the
  session-4 ruling's scope** — the residue it waved off as decode skew
  was mostly this, and the decode was never touched (nor does it need
  to be).
  - **THE RETAIL LAW, END TO END.** The two registers behind a cast are
    the two halves of `MouseButtonState_18059C`, rebuilt from scratch
    every input poll (EF:49675-83): **bit 0/1 = the ISR press LATCH**
    (`x_WORD_180746` / `x_WORD_180744`), **bit 2/3 = the HELD state**
    (`x_WORD_18074C` / `x_WORD_18074A`). `HandleMouseButtons_18F80`
    (PI:2027-76) then splits the spell families on `byte_0x3B_59`:
    **`== 1` fires off bit 0 ALONE** and clears it (PI:2043-49);
    everything else takes `bit0 || (bit2 && spell->word_0x2E_46 > 0)`
    (PI:2050) — the repeat arm, live only while the cast window is.
    The frame tail then drops the GLOBAL latch whenever bit 0 is down
    (PI:1049-52), so the latch is one-shot: **one cast per physical
    click, however long the button is held.** Possession's
    `byte_0x3B_59` is 1.
  - **THE CAST CHAIN ITSELF IS ALREADY VERBATIM — do not go looking
    for a lockout.** `sub_5F660` case 1 (EF:60900-07): armed
    (`word_0x2E_46 > 0`) → set `byte_0x3C_60 = 1`, stamp the hand, run
    `sub_5F7E0`, `goto LABEL_23`, **no re-arm, no mana gate, no buzz**;
    not armed → the mana gate then `sub_5F7B0` (EF:60973:
    `word_0x2E_46 = word_0x30_48`). `sub_69640` (EF:55915) fires only
    at `word_0x2E_46 == word_0x30_48` and counts down one per tick,
    expiring into `sub_6D880` (EF:58215 — a pending-tier apply, NOT a
    cooldown). `word_0x36_54` is decremented at the tail and read by
    **nothing** on this path. `byte_0x154_340` is the charge
    accumulator (incremented per frame to a 200 cap, EF:5423-25, spent
    as `dword_0x10_16` by the leveled arms and simply reset by
    `sub_69900` at EF:56058) — **not a suppression latch**. There is no
    cooldown, no in-flight cap, and no token gate. The only thing
    standing between two bolts is `word_0x2E_46` and the press latch.
  - **THE MEASUREMENT THAT PINNED IT.** Instrumented `mc2l4 0+4000`
    (the manifestation's `word_0x2E_46`/`word_0x30_48` are IMPORTED per
    pair, so retail's own arm cadence is readable straight off the
    trace): possession's `word_0x30_48` is **3**; retail arms
    (`f26` 0→2) **404** times; the recording's held-right register has
    **409 rising edges**; the port launched **883**. Every port
    `mc2_cast_input` sample in the window read `edge == held`, 2,058
    for 2,058 — the edge detector was structurally incapable of
    reporting anything else. After the fix the port launches **408**.
    (`mouse_clicks` — the recorded latch — is never set without
    `mouse_buttons` on this corpus, so the harness's `held || latch` OR
    is a no-op and the held register's own rising edge IS the press.)
  - **CHANGES.** `verify.rs` + `verify_mc2.rs`: `prev_cmd = pcmd` moved
    INSIDE the `prev.take()` arm (env `MGC_NO_FIRE_EDGE=1` restores the
    old behaviour for A/B); `fixtures.rs`: the same, both loops — the
    suite MUST reconstruct input exactly like `verify-deltas` or its
    signatures drift from the triage run. Port-side, two verbatim
    corrections on the same press path: `mc2_cast_input`'s repeat test
    is `f59 != 1`, not `f59 == 0` (PI:2043 tests `== 1`), and
    `mc2_cast_gate`'s armed possession arm raises `f56` FIRST and
    unconditionally — retail never reaches the sound-29 `v6` flag
    there, so a broke wizard re-pressing possession no longer buzzes.
  - **NUMBERS (A/B, one binary, env-toggled, back-to-back).** **mc2l0
    0+4000: raw conforming 682 → 705**, unexplained field 3,660 →
    **3,553**, entity sets 112/917 → **497/642**, (9,1) 12/168 →
    241/131, (10,14) 149 → **125**. **mc1l0 0+4000: raw conforming
    450 → 501**, unexplained field 7,440 → **6,524**, unexplained extra
    135 → **79**, entity sets 400/1,029 → **429/401**. mc2l4 0+4000:
    unexplained field 5,348 → **5,092**, extra 55 → **49**, (9,1)
    80/443 → 261/327. mc2l30 0+4000: explained 2,867 → **2,876**,
    unexplained field 4,826 → **4,519**, extra 89 → **78**, (9,1)
    23/213 → 103/143. mc2l24 0+4000: unexplained field 5,729 →
    **5,641**, extra 23 → **19**, (9,1) 11/138 → 79/100.
  - **FULL-TAKE mc2l24 (final tree, shared with the concurrent
    doomsday dig — absolutes, not attribution):** 69,207 pairs, 1,816
    torn, 67,391 fixture-grade, 15 conforming, **19,135**
    conforming-or-explained, UNEXPLAINED **494,747 field / 464 missing
    / 6,038 extra** (from 495,023 / 482 / 6,984 — the extra side is
    **−946**), rng **10** (was 12); (9,1) **140 / 1,733** (was
    355 / 3,794).
  - **THE RULING, RE-TESTED.** On every take where the HUMAN casts, the
    (9,1) family is now two-sided (l4 261/327, l30 103/143, l24-early
    79/100, l0 241/131) — the one-sided extra family the dig was
    chartered on is GONE. What is left is genuine ±phase: measured
    against retail's arm ticks, `retail_arm − port_fire` is **+1 on 227
    of 408 casts, +2 on 96, 0 on 39** at `--input-delay 2`, i.e. the
    port fires one tick EARLY. So the session-4 skew ruling **still
    holds for the residue, and only for the residue** — but it never
    covered the bulk, and the decode still needs no change.
    **`--input-delay 3` absorbs the dominant +1**: same window, mc2l4
    (9,1) 261/327 → **173/135**, (10,12) 141/60 → **68/60**, entity
    sets 571/542 → **407/318**, unexplained field 5,092 → **5,033**,
    explained pairs 3,064 → **3,069**. That is a corpus-wide knob and a
    roster decision, so it is measured here, NOT landed.
  - **THE l24 (9,1) RESIDUE IS NOT INPUT AT ALL.** Late window
    (40000+4000): retail casts **zero** possession bolts while the port
    casts 19 — these are RIVAL casts, and rivals' `cooldown[]`/`burst`
    AI lanes are the ones `retail_import_mc2` explicitly does not
    import ("the AI decision-lane decode is still owed"). They belong
    to `mc2-rival-ai-lanes` / open-leads, not to possession. The fix
    still halved them (56 → 19) by keeping the shared world instance
    closer to retail.
  - **SUITE ACTION OWED (not applied — fixture JSON is out of this
    dig's remit).** `mgc-conform fixtures conformance/*.json --promote`
    is needed: mc2l0 **5 fixed / 2 regressions / 2 drifted** (t=32
    `extra:10,14` and t=291 `missing:9,1` are both the ±1 phase),
    mc2l4 5 drifted, mc2l30 2 drifted, mc2l24 6 drifted, mc1l0 2
    drifted, mc1hwl0 clean, 0 regressions anywhere outside mc2l0.
    Also propose re-scoping `mc2-cast-timing-extra`'s note: it no
    longer carries "the possession fresh-arm input-reconstruction
    extras" as a bulk family.
  - **NEW LEAD (tier 1/2 possession re-fire, traced but NOT landed —
    no corpus coverage).** `sub_69640`'s else-branch (EF:55995-56013)
    is fully readable now: the `byte_0x3C_60` signal drives a 3-tick
    decay counter (1→2→3→4, reset at >3 with the trailing `sub_68DE0`
    SKIPPED via LABEL_26), the re-fire happens ONLY at counter == 1,
    and it calls **`sub_69900`** — i.e. tiers 1/2 re-fire the BASIC
    (9,1), not their own (9,17) — and it is **NOT mana-debited**
    (`sub_68DE0`'s `word_0x2E_46 != word_0x30_48` arm only pins regen,
    EF:55569-93). The port instead re-runs `mc2_spell_fire` (leveled
    entity + full debit) with no decay counter. Three wrong lanes,
    zero observables; needs a tier-1/2 take before it is touched.
  - Pinned by `mc2_possession_held_button_casts_exactly_once` (a
    24-tick hold on a 3-tick marker casts ONCE; the neutered
    level-trigger casts 8 — the over-fire's exact shape),
    `mc2_repeat_family_is_every_byte_3b_except_one` (fails under the
    old `f59 == 0`) and `mc2_possession_repress_while_armed_never_buzzes`
    (fails with the mana gate restored). All three verified failing
    against their neutered arms. 331 lib tests + every integration
    suite green under `MGC_REQUIRE_GOLDENS=1`; no golden moved.

- **THE `.mgcr` MC2 DECODE GUESSED "THE HIGHEST STACKED CELL IS SLOT
  999", so every snapshot with the TOP of the pool in use handed the
  import a free stack shifted by a constant — and the census then threw
  it away and replayed the pair on a descending slot scan, re-ordering
  every spawn. LANDED 2026-08-04 (session 9, mgcr pool-base dig).**
  Closes the SECOND slot-order source root-caused in open-leads 0b's
  session-8 update. Decode-side only: no gameplay law moved.
  - **WHY THE OLD RECOVERY WAS UNPINNED.** `D41A0_0` is a static, but
    DOS/4GW's load delta makes its guest address run-specific, so
    `mgcr::mc2_stack` recovered the pool base from the cells: scan
    `cells[0] − s·168` from s=999 down, take the first candidate under
    which every cell decodes in range. Every cell is a pool pointer, so
    **alignment is s-independent** — the in-range candidates form one
    contiguous interval and the only binding constraint is "max index
    < 1000", i.e. *the highest stacked cell is slot 999*. True only
    while the top of the pool is free. Occupy the top N slots (they
    then never appear on the free stack) and EVERY decoded slot
    inflates by N.
  - **THE MEASUREMENT (probe over every MC2 snapshot in the corpus).**
    mc2l24: **14,219 of 69,221 snapshots shifted** (20.5%), shifts
    1..993, e.g. t=56539 shift 3 (205 of 493 cells landing on LIVE
    records), t=60101 shift 2 (197/576), t=62929 shift 4 (129/226),
    t=64566 shift 5 (223/526). First shifted tick is **t=54932** — the
    take runs clean before it. mc2l0/l4/l30: shift 0 on all 35,677
    snapshots (their pools never fill), which is why the bug hid.
  - **THE FIX — VALIDATE THE BASE AGAINST THE POOL IMAGE**
    (`mgc-formats/src/mgcr.rs`: new `mc2_pool_base` /
    `mc2_base_from_cells` / `mc2_stack_cells`, `mc2_stack` now takes
    the recovered base). Retail's frame-top reap zeroes `class` before
    pushing (`sub_57F20`), so every free-stack cell must land on a
    `class3f == 0` record, and slot 0 is the reserved null that is
    never stacked. That base is **unique on all 104,824 corpus
    snapshots** with a non-empty free stack, and the decoded set is
    then EXACTLY the class-0 slots minus slot 0 on every one of them
    (= the import's own `scan_free` census, so the fallback can no
    longer fire). Ambiguity or no candidate ⇒ empty stack, i.e. the
    import's descending-scan fallback still rides.
  - **INDEPENDENT CORROBORATION.** The recovered base equals
    `base160 + 736_026` in **all 104,824** snapshots across four
    separate process runs (both are statics in the same image, so
    their distance is a build constant). Not used as the recovery —
    it is per-build — but it pins the validator's answer without
    reference to the class-0 criterion.
  - **THE RECYCLE STACK NEEDED IT TOO — the opposite validator.**
    Its cells are LIVE victims (`sub_49F90`'s sacrificable list), so
    recovering ITS base from "max index == 999" put mc2l4's victims on
    bogus FREE slots: 23,700 cells over 2,851 snapshots, every one of
    them decoding onto a class-0 record that the import then chained
    into the port's free list (and every one of those pairs took the
    fallback). Both stacks now share the one recovered base; under it
    **0 of 48,049 corpus recycle cells** land on a free record, so the
    import's `class64 == 0` filter drops them all — correct: a recycle
    victim is not a free slot.
  - **CONFORMANCE (windowed A/B, one frozen binary, env-toggled arms).**
    `free-stack fallback` stderr lines / gross missing / gross extra /
    UNEXPLAINED field·missing·extra / computed `slot-desync` rows:
    l24 60000+300 **296**/3271/3403/2503·2·49 →
    **0**/8/159/2635·1·28; l24 62800+300 **288**/3433/3288/6590·8·76 →
    **0**/411/286/6563·1·47; l24 56400+300 **300**/2395/2471/1169·4·61
    → **0**/46/123/1170·0·47; l24 64500+400 (the fountain window)
    **400**/303/972/759·1·52 → **0**/4/673/820·0·9; l4 4300+300 (the
    recycle window) **292**/142/138/946·2·2 → **0**/41/43/952·1·2.
    Over the four l24 windows: fallback **1,284 → 0**, gross missing
    **9,402 → 469 (−95%)**, gross extra **10,134 → 1,241 (−88%)**,
    unexplained extra 238 → 131, unexplained missing 15 → 2, and the
    computed `slot-desync` rule goes **124/124 rows across 47 pairs →
    ZERO** (those rows are gone, not re-labelled). UNEXPLAINED field
    rises 11,021 → 11,188 (+1.5%): entities that used to be absent are
    now present in retail's slot and get compared lane by lane.
    Cross-take regression probes mc2l0 / mc2l4 / mc2l30 at 0+2000 are
    **byte-identical** between arms (shift 0, empty recycle there), and
    the MC1/HW arm never touches this decode.
  - **FIXTURE SUITES (A/B, one binary, per manifest).** mc1l0 68 (2
    drift), mc1hwl0 29, mc2l0 41 (2 regressions / 5 fixed / 2 drift),
    mc2l4 24 (5 drift), mc2l30 24 (2 drift) — **identical in both
    arms** (the mc2l0 regressions/fixes belong to the concurrent
    possession dig, not this one). mc2l24 12 as-expected/5 drift →
    **11/6**: t=64000 LOSES `missing:10,39` and gains that ball's
    field lanes. Wants a `--promote` pass — NOT applied here.
  - **FULL-TAKE mc2l24** (whole tree, so it also carries the
    concurrent possession dig): 69,207 pairs, 1,816 torn, 67,391
    fixture-grade, **15 conforming** (=), 19,135 conforming-or-
    explained (was 19,131), **494,747 unexplained field** (was
    495,023), **464 missing** (was 482), **6,038 extra** (was 6,984,
    −14%), **rng 10** (was 12), and **zero `free-stack fallback`
    lines** in 69,207 pairs (was ~14,219 pairs' worth).
  - **WHAT IT IS NOT.** The t=3569 / t=13330 scripted-wave slot desync
    is NOT this bug — l24's first shifted snapshot is t=54932 and the
    t=3500+300 window is byte-identical in both arms. The whole-take
    `slot-desync` residue (208/208 rows across 21 pairs) lives in that
    early region and still wants its own cause. Likewise the (10,39)
    extras in the fountain window (673 after the fix) are a real
    over-spawn, no longer masked by a slot skew.
  - Pinned by `mc2_pool_base_is_pinned_by_the_free_records_not_the_max_index`,
    `mc2_recycle_stack_rides_the_free_stack_pool_base` and
    `mc2_ambiguous_pool_base_yields_no_stack` (mgcr.rs unit tests on
    synthetic snapshots; all three FAIL against the old recovery — the
    first prints the literal `[999, 400, 399, 304]` vs `[700, 101,
    100, 5]` shift).
  - **NEW LEADS.** (a) The port has NO recycle-victim allocation path:
    retail pops the recycle stack when the free stack is exhausted
    (74 mc2l24 snapshots have a FULL pool), the import filters those
    live slots out, so the port simply fails to spawn there.
    (b) `base160 + 736_026` could become a decode cross-check (or the
    recovery, if the recorder ever stamps the build).
    (c) The mc2l24 suite drift wants `--promote`, and the roster's
    computed `slot-desync` rule now absorbs far fewer rows — re-scope
    it to the early-take wave family.

- **THE TIER-0 POSSESSION BOLT IS (9,1), NOT (9,17) — the port had no
  subtype-1 creator AT ALL and launched the leveled entity for every
  tier — and FOOL'S-MANA OPEN-5's missing launch lift turned out to be
  the SAME retail law seen from the other end. BOTH LANDED 2026-08-03
  (session 8, bolt-launch-lanes dig).** Closes the claim-pulse dig's
  parting lead (full-take l24 "(9,1) 362 missing / 0 extra") and
  fools-mana.md OPEN-5.
  - **THE TIER GATE PICKS AN ENTITY, NOT A PAYLOAD.** `sub_69640`
    (EF:55946-49) branches on `SPELLS[model].subspell[tier].life_0x1A`:
    **0** → `sub_69900` (EF:56039) → `SummonManaPosession_4D3B0`
    (EF:34764) = class 9 model **1**, **action 1**, speed/minSpeed 384,
    `maxLife = 4096/384 = 10`, mana 50, row `str_D7BD6[61]`,
    **`xtype_0x41_65 = 10`**, sprite 209 +
    `SetEntityShiftRot_49EA0(2*pitch, **5*fov/2**)`; **1..3** → the
    inline (9,17) arm (EF:55950, `sub_4DDD0` EF:35132 — same row/sprite
    but action 18 and ShiftRot `2*fov`), `byte_0x44_68` 54 / 69 / the
    NewEvent 0; **>3** → the `<= 3` gate fails and NOTHING is cast.
    Row 1's baked `life` column is (0,1,2), so tier index ≡ life.
  - **`sub_69900`'s TAIL, pinned field by field against the recording**
    (mc2l4 t=13 slot 303, `dump-state`; full table in
    docs/traces/mc2-possession-delivery.md, Addendum 2026-08-03):
    `dword_0x10_16` = **200** (@0x10 → f26); `word_0x26_38` = the spell
    TOKEN's slot **267** (@0x26 → f40 — DELIBERATELY not ported: the
    port spends f40 on the spell INDEX, which is `mc2_proj_impact`'s XP
    back-ref, while retail hard-codes `sub_6D8B0(id, 1, 1)` per handler
    at EF:63314/59052; the lane is not compared); `mana_0x90_144` = the
    TOKEN's mana **33**, not the ctor's 50 (@0x90 → f140, a COMPARED
    lane); box `apitch/aroll` **180**, **afov 187** = 5·75/2 off sprite
    209's (speed_6 0, rotSpeed_8 150) — the leveled twin's is 150;
    impact (10,12); and `actSpeed` **336**.
  - **336 IS THE FIND INSIDE THE FIND.** Both possession arms add the
    carpet boost RAW — `v2x->actSpeed += a2x->actSpeed` (EF:56048 /
    EF:55953). The `[384, 0x2000]` clamp the port applied to every cast
    belongs to `sub_6DCA0` ALONE (EF:44226-31); on a REVERSING carpet
    retail genuinely launches a sub-384 bolt, and `mc2_launch` was both
    flooring it at 384 and dropping the negative term (`p.speed.max(0)`).
  - **A AND B ARE ONE LAW.** `position.z += <launcher>->array_0x52_82.fov`
    appears at EF:56054 / EF:55969 with the WIZARD as launcher and at
    EF:26688 (`sub_36770`) / EF:26718 (`sub_36850`) with the fool's-mana
    SPHERE as launcher: "leave from the top of the launcher's own box".
    The cast half was already carried — `World::muzzle` returns
    `p.z + PLAYER_HH`, and PLAYER_HH is exactly the MC2 wizard's fov
    (`AddPlayer_4A920` EF:33334 sets sprite row 44; MC2 row 44
    rotSpeed_8 = 200 → fov 100, MC1 row 44 height 200 → PLAYER_HH 100).
    The trap half was OPEN-5 and is now `e.z + e.f84` in
    `mc2_fools_bolt`.
  - **OPEN-5's "self-detonation" WAS NEVER A PROBE FILTER.**
    `sub_10780` (EF:3739-71) has no launcher exclusion — flags,
    xtype narrowing, `a1x->id_0x1A_26 != v5x->id_0x1A_26`, box. What
    keeps retail's bolt off its own sphere is (a) the sprung tier-0
    sphere is UNMAPPED and class-zeroed INSIDE ITS OWN TICK — the walk
    runs `sub_57F20` (Events.cpp:551; body :5209 = `SetMapEntity_57E50`
    + `class = 0` + free-stack push) the instant
    `DisableEntityDrawing04_57F10` latches `byte[1]&4` — and (b) retail
    probes ONCE, at the END of a full 384-unit step (`sub_65C20`
    EF:63126-29). Our soft kill leaves the sphere linked to the tick-top
    reap and our anti-tunnel chord march probes 128-unit sub-steps
    retail never visits, so the exclusion rides the shared owner gate
    instead: `mc2_fools_bolt` stamps `id24 = sphere.id24` and
    `victim_scan`'s `c.id24 != id` drops it, for an authored sphere
    (id24 = own slot on both sides) and a cast decoy (id24 = caster)
    alike. **NEW LEAD (fools-mana OPEN-7):** the chord march probes from
    the MUZZLE OUT where retail probes only the endpoint — any future
    launcher that does not inherit its host's id will detonate on tick 1.
  - **CHANGES.** `CREATORS` gained `(1, 1, 384, 10, 61, 209)`;
    `mc2_spawn_cast_proj` gained the possession pair's ShiftRot and the
    (9,1)'s `xtype = 10`; `mc2_flyer_tick` now SKIPS `mc2_proj_filter`
    on the claim arm (retail's narrowing lives inside `sub_10780`,
    EF:3765-68 — `sub_108B0` has none, so filtering a claim hit by
    xtype 10 would have swallowed worm (5,22) and building (10,45)
    claims); `mc2_spell_fire` spell 1 and `mc2_rival_emit` (which
    hardcoded 17 for every rival cast) both pick the entity off
    `life_0x1A`; `mc2_fools_bolt` lifts by the sphere's f84.
  - **NUMBERS (A/B, one binary, env-toggled, back-to-back).** mc2l4
    0+4000: explained 3065 → **3067**, unexplained field 5351 →
    **5348**, **(9,17) 443 extra → 0** with (9,1) 80/0 → 80/443, gross
    `action` 3706 → **3399**, `model` 628 → **322**, `mana` 1813 →
    **1551**, `speed` 1295 → **1030**, `applied_pitch` 1063 → **756**.
    mc2l30 0+4000: (9,17) 213 extra → 0 (all now (9,1)), gross `action`
    6142 → **5957**, `applied_pitch` 908 → **723**, `speed` 834 →
    **658**, `mana` 775 → **590**, `model` 642 → **459**; unexplained
    field 4824 → 4826. mc2l24 0+2000: gross `model`/`action` 786/783 →
    **617**, `speed` 1497 → **1329**, `mana` 409 → **245**,
    `applied_pitch` 878 → **709**, `z` 9600 → **9520** (the lift alone
    is −12 of those); unexplained field 1007 → 1010. Net: ~1,400 gross
    field rows per take stop being wrong, ±3 unexplained (the rows were
    already inside the class-9 capture rules, which are class-scoped and
    survive the relabel).
  - **FULL-TAKE mc2l24 (final tree, shared with the concurrent
    dweller/doomsday dig — absolutes, not attribution):** 69,207 pairs,
    1,816 torn, 67,391 fixture-grade, **15 conforming**, 19,131
    conforming-or-explained, UNEXPLAINED **495,023 field / 482 missing /
    6,984 extra**, rng **12**; (9,1) 355 missing / 3,794 extra, (9,17)
    2/24, (10,12) 540/1,114.
  - **THE RESIDUAL IS AN OVER-FIRE, NOT AN IDENTITY.** With the entity
    right, the (9,1) family reads ~5× (l4) to ~10× (l24) more port rows
    than retail rows — the port launches far more possession bolts than
    the recording does at the same input. That is the next possession
    lead, and it is orthogonal to everything above (it was hiding under
    the (9,17) extras before).
  - **RECORDER LEAD (concrete, two fields).** Retail aims BOTH bolts at
    `wizext.nextEntity_0x18_24 + yaw` / `entityIndex2_0x1A_26 + pitch`
    (EF:56060-66 / EF:55970-71) — the per-frame FREE-LOOK input deltas
    (`playerInputs_0x6E3E` word6/word8 → EF:38065-66; the camera adds the
    same pair at EF:40273-74). They live in `type_str_164` at **+24 /
    +26**, i.e. the very block `RetailPlayerMc2` already reads for
    `cmd_speed` (+12) and `strafe` (+16), and the `.mgcr` does NOT carry
    them — so a free-looking player's launch heading is unreproducible
    from the recording. Proposed: add `look_yaw`/`look_pitch` to
    `RetailPlayerMc2` on the next recorder + format pass.
  - Pinned by `mc2_tier0_possession_launches_the_basic_bolt_with_sub_69900s_tail`
    and `fools_trap_bolt_leaves_from_the_sphere_box_top_and_clears_its_own_muzzle`
    (world.rs lib tests; the first fails on the neutered arm with
    "tier 0 must NOT launch the leveled (9,17)", the second with the
    lift removed — and its CONTRAST arm shows a same-muzzle bolt with a
    foreign owner DOES detonate on the sphere, so the owner gate is
    load-bearing rather than lucky). The pre-existing
    `mc2_possession_tier0_does_not_refire_while_the_marker_runs` had to
    be flipped from counting model 17 to model 1 — the port's old
    behavior in one line. No golden moved; 328 lib tests + all
    integration suites green under `MGC_REQUIRE_GOLDENS=1`.

- **THE MC2 CONFORMANCE IMPORT DOUBLE-PUSHED EVERY GHOST SLOT ONTO
  THE FREE STACK — so any spawn burst deeper than the ghost count
  re-`NewEvent`ed a slot it had just filled. The (5,0) pyramid-summon
  "misses" were the doomsday worm chain RE-ALLOCATING ITS OWN HEAD.
  LANDED 2026-08-03 (session 8, (5,0) summon-cadence dig).** The
  session-7 hypothesis (a state-9 `count(m0) < 4` cap divergence) is
  **DISPROVEN** — the cap law is already verbatim (see below); the
  bug was in the import's free-list reconstruction and it was
  **global to every MC2 pair**, not a pyramid law at all.
  - **THE MEASUREMENT.** mc2l24 pair 53808→53809: retail spawns a
    17-record m0 chain (head slot **905** + 16 children 837, 813,
    796, 727, 690, 72, 65, 63, 62, 61, 625, 620, 423, 422, 420, 407
    — the head's `word_0x34_52` chain read straight off the corpus),
    the port spawned **nothing visible**. Instrumented pop order:
    905, 837, 813, 796, 727, 690, **905 again**, 837, 813, 796, 727,
    690, 72, 65, 63, 62, 61. The 7th pop re-entered `NewEvent_4A050`
    on the live head and `*e = Ent::default()` zeroed its class; the
    child loop then byte-copied that zeroed head into every
    subsequent child, so the whole chain projected as class 0 =
    16 `missing` rows in one pair.
  - **THE LAW.** Retail's frame top (`UpdateEntities` EF:39948-56)
    reaps every disabled record with `sub_57F20` (Events.cpp:5209-38:
    tile-unlink, `class = 0`, `dword_0x35++; pointers_0x246[top] =
    entity`) — ASCENDING slot order — and only then rebuilds the
    per-model buckets. So a `.mgcr` capture's recorded free stack is
    the **pre-reap** image and the ghosts are exactly the slots the
    next frame's reap will push. The port already runs that reap
    (`World::tick`'s strict-MC2 top pass, DEVIATIONS.md "World::tick
    (MC2 sweep: disabled dispatch)"), and `retail_import_mc2` was
    ALSO appending `ghost_slots` — the double push its own comment
    warns about ("appending them here too would double-push the
    slots"). The corpus confirms the reconstruction: at t=53808 the
    recorded free stack is 716 entries ending
    `…, 406, 407, 420, 422, 423, 620, 625, 61, 62, 63, 65, 72` and
    the 6 ghosts {690, 727, 796, 813, 837, 905} are absent — reap
    them once, ascending, and the top becomes 905, i.e. retail's
    exact allocation order for the chain. FIX: drop the
    `self.g.free.extend(ghost_slots)` (`conformance.rs`; the binding
    still feeds the census `scan_free`). Pinned by
    `mc2_import_leaves_the_ghost_free_push_to_the_tick_reap`
    (non-vacuous: restoring the extend fails both asserts).
  - **SECOND FIX — the pyramid's SUMMON STRIDE had no import home.**
    `sub_21850` stamps the ring stride into `word_0x4A_74` (@0x4A,
    682 for every creature pick / 256 for the m19 swarm,
    EF:13160/13173/13186/13199) and `sub_21AB0` fans the ring at
    `stride * repeat + yaw` (EF:13364). The uniform class-5 import
    read f50 ← @0x30, which is DEAD for the pyramid, so every
    replayed summon spawned stacked on the pyramid's own bearing
    (t=53808: retail x 7616 vs port 7936). f50 now imports @0x4A for
    (5,10) — the third (5,10) exception next to f26 ← @0x10 and
    f36 ← @0x28. Pinned by
    `mc2_pyramid_import_keeps_the_summon_stride`.
  - **THE CAP LAW IS ALREADY RIGHT (hypothesis retired).** `sub_223E0`
    (EF:13780-13808) recomputes four counts from the per-MODEL bucket
    lists rebuilt at frame top (class 5, `life >= 0`, action not
    0xB4/0xE8/0xEA — EF:39987-40007), and `mc2_pyramid_pick_summon`'s
    predicate matches that membership exactly. At the l24 summon the
    port and retail agree on the pick (both selector 3, both at
    state 8 t=53806). NOTE for a future dig: the decompile's three
    `bytearray_38403x[0]` loops (picks 3/4/6) vs the fourth
    `[100/4]` = bucket 25 (pick 5, m25) are internally inconsistent
    with the "cap counts the summoned model" reading — either the
    caps for picks 4 (m21, <12) and 6 (m19, <28) really are MODEL-0
    counts (kept verbatim) or the decompiler lost two bucket indices.
    Nothing at l24 discriminates; leave verbatim.
  - **CONFORMANCE (windowed A/B, same binary, env-toggled arms, 300
    pairs each).** UNEXPLAINED field / missing / extra:
    t=53700 3698/29/21 → **3098/1/31**; t=54700 3074/16/42 →
    **2532/0/45**; t=56400 1333/13/55 → **1169/4/61**; t=60000
    3069/41/42 → **2577/2/51**; t=62800 7212/40/56 →
    **6605/8/80**. Totals **18,386 → 15,981 field (−13%)**,
    **139 → 15 missing (−89%)**, 216 → 268 extra. Every named (5,0)
    miss tick is answered: 53808 (12/0 → gone), 54825 (14/0 → gone),
    56539 (9/0 → 2/3), 60101-60103 (50/16 → 48/49, now BALANCED =
    the computed slot-desync class), 62929 (12/0 → 0/3). Pair 53808
    alone: 77 field + 16 missing → **4 field, 0 missing, 1 extra**.
    The `extra` rise is the mirror of the same law — spawns that used
    to be clobbered into a shared slot now all survive.
  - **FIXTURE SUITES (A/B, same binary).** MC1/HW untouched (68/68,
    29/29). MC2 baseline → fixed: mc2l0 4 fixed/0 regressions/0 drift
    → **6 fixed / 1 regression / 3 drift**; mc2l4 3 → 4 drift;
    mc2l30 0 → 1 drift; mc2l24 2 → 5 drift. The drifts are
    IMPROVEMENTS (l0 t=449 loses `missing:10,0`, t=3449 loses
    `missing:10,12`, l4 t=39 loses `field:10,0:rand/x/y` +
    `missing:10,0`) and want a `--promote` pass. **NEW LEAD**: the
    one regression is an `extra:10,14` on mc2l0 (t=32, and the same
    row drifts into t=33/60/79) — a (10,14) the port over-spawns that
    the duplicate slot used to swallow.
  - **FULL-TAKE mc2l24 (end of session 8, whole tree — this dig plus
    the concurrent claim-pulse dig).** 69,207 pairs (13 gaps), 1,816
    TORN, 67,391 fixture-grade: **15 conforming** (was 10), 19,131
    conforming-or-explained, **495,023 unexplained field** (was
    500,845), **482 missing** (was 975, −51%), 6,984 extra (was
    6,227), **rng 12** (unchanged). (5,0) is no longer a headline
    family: 50 missing / 55 extra whole-take, near-balanced, first at
    t=56539 — all of it the `mc2_stack` shift lead above.

- **THE (10,12) POSSESSION CLAIM PULSE WAS NEVER AN ENTITY IN THE
  PORT — and the BASIC (9,1) bolt was missing from the claim-probe
  gate, so it detonated on everything it grazed. BOTH LANDED
  2026-08-03 (session 8, claim-pulse dig).** Open-leads entry 0
  banked "(10,12) missing 313 (l30) / 779 (l4) = possession
  WEAK-PULSE family"; the l24 `want=12 got=54` rows were the same
  machine seen as a slot skew.
  - **THE RETAIL LAW (already traced, now ported).** Possession is
    delivered by MAIL FROM A SEPARATE ENTITY, not by the hit
    (docs/traces/mc2-possession-delivery.md §1-§3). Both bolt
    handlers spawn it: `CastPosses_65F60` (EF:63306-19, class-9
    action **1**, the basic (9,1)) spawns `_4A190(&pos, byte_0x43,
    byte_0x44)` = (10,12) and copies id/yaw/pitch; `sub_674C0`
    (EF:59032-59058, action 18, the leveled (9,17)) spawns TWO
    children on a victim — the pulse FIRST ((10,12), or **(10,70)**
    when the payload is (10,69), EF:59036-39), then the (10,54)/
    (10,69) aura, with `sub_6D8B0(id, 1, 1)` = the possession-XP
    mail (EF:58228, `class==3 && model==0` guard = the human only)
    and `sub_65780` (EF:62836) = an ACCURACY-STATS counter on the
    caster's wizext, not the claim. Ctors `NewAdd0A0C_4E8C0` /
    `NewAdd0A46_4E950` (EF:35573/:35595) are byte-identical bodies:
    life 8, `subSpellIndex_0x2A_42` 64000, sprite 41, `byte[0] =
    (b & 0xF6) | 1`, box 512³, no RNG. Ticks
    `PossesHitMana_320E0` / `sub_32120` (EF:23546/:23559): bump
    @0x10, class-10 PRE-decrement, anim, then `sub_112D0(0|1)` —
    the ch1 broadcast, every tick of the 9-tick window.
  - **WHAT THE PORT DID.** `mc2_proj_impact`'s (10,12)/(10,54)/
    (10,69) arms ran ONE `area_write(i, 1, …)` from the BOLT and
    returned `None` — no entity, and the claim reach was the bolt's
    own box for one tick instead of the pulse's 512³ box for nine
    (retail's near-miss claims are exactly that reach). Fixed:
    `Gen::mc2_spawn_claim_pulse` (mc2/effects.rs) + the forced
    twin's tick `Gen::mc2_steal_pulse_tick` (action 0x4D; the weak
    action 12 was ALREADY equivalent to MC1's `possess_flash_tick`
    and keeps riding the shared class-10 band, world.rs).
  - **CTOR PINNED AGAINST THE RECORDING.** mc2l4 pair 22→23, slot
    309: the port now matches retail on EVERY projected lane —
    class/model/action 10/12/12, life 7 (it spawns ahead of the
    walk cursor and ticks the same pass), max_life 8, x/y/z,
    heading, pitch, applied_yaw **125**, applied_pitch **512**,
    speed 16, mana 0, rand. The applied_yaw/applied_pitch split is
    the ORDER fingerprint: `SetEntityIndexAndRot_49CD0(41)` writes
    all four lanes from the sprite row, then `SetEntityShiftRot_
    49EA0(512, 512)` overwrites pitch/roll/fov only — so row 41's
    half rot-speed survives at @0x52.
  - **THE SECOND BUG THE PULSE EXPOSED.** With the pulse invisible,
    the port's over-detonation was invisible too. `mc2_flyer_tick`
    gated the claim probe on `tick70 == 18`, but `sub_108B0` has
    exactly TWO callers and action **1** is the other one — its own
    comment said so. Every basic (9,1) bolt (including the ones the
    importer replays) therefore ran the generic any-solid
    `sub_10780` probe AND skipped possession's ground-skim clamp
    (EF:63262-64, likewise `is_possess`-gated). l30 went 258 retail
    pulses vs **714** port ones; fixed to `matches!(tick70, 1 | 18)`.
  - **NUMBERS (A/B, same binary, back-to-back, env-toggled).**
    mc2l4 0+4000: explained pairs 3,046 → **3,061**, unexplained
    field 6,734 → **6,652**, entity sets 819/1,183 →
    **627/694**, (10,12) 279 missing/0 extra → **89/79**.
    mc2l30 0+6000: explained 4,422 → **4,433**, unexplained field
    7,826 → 7,830, unexplained missing 65 → **58**, entity sets
    1,688/961 → **1,445**/1,027, (10,12) 258/0 → **55/72**.
    mc2l24 63900+5000: explained 994 → 994, unexplained field
    10,519 → 10,557, unexplained missing 26 → **23**, (10,12)
    71/0 → 71/**49**.
  - **WHY l24 KEEPS ITS 71.** On l24 the port's pulses are pure
    SLOT desync, not timing: same ticks (64566, 65040, 65140,
    65272, 65495, 65542, 65623, 65682, 65707 …), retail at high
    slots (423/485/397/247) and the port at low ones (7/9/18/24/27)
    — the free-list slot-order lead (open-leads 0b), which the
    runner already flags per run (`free-stack fallback: live 342 !=
    scan 564`). Nothing in the pulse's own law is left open.
  - Pinned by `mc2_possession_impact_spawns_the_claim_pulse_entity`
    and `mc2_basic_possession_bolt_rides_the_claim_probe` (world.rs
    lib tests; both verified to FAIL against the neutered arms).
    No golden moved.
  - **NEW LEAD OUT OF THIS DIG: the tier-0 bolt is the wrong
    ENTITY.** — **CLOSED 2026-08-03 by the bolt-launch-lanes dig; see
    the entry at the head of Resolved.** Retail's basic possession is
    `sub_69900` (EF:56039)
    spawning **(9,1)** `SummonManaPosession_4D3B0` (EF:34764 —
    action 1, speed 384/384, `maxLife = 4096/384 = 10`, row 61,
    sprite 209, `xtype = 10`, box ×2 / ×2.5 off the sprite row) and
    only tier `life_0x1A` 1..3 takes (9,17) (EF:55950). The port's
    cast arm (mc2/cast.rs spell 1) always launches (9,17), and
    `CREATORS` has no subtype-1 row at all: full-take l24 shows
    **(9,1) 362 missing / 0 extra**, mc2l4 0+4000 shows (9,1) 96
    missing vs (9,17) 393 extra. sub_69900's own tail is also
    unported — `dword_0x10_16 = 200`, `word_0x26_38` = the token
    slot, `mana_0x90_144` = the TOKEN's mana (recorded 33; the port
    ships the ctor default 50, and `mana` IS a projected lane), z +=
    caster fov, and the head-offset aim
    (`wizext.nextEntity_0x18_24 + yaw`, `entityIndex2_0x1A_26 +
    pitch`) at a 10240-unit designated point.

- **THE l24 m23 DWELLER STEERING RESIDUAL IS A MISSING TERRAIN
  REPLAY, NOT A MOVE-CORE BUG — the (14,1) riser's ENDCAP WALL
  outlives its own lowering, and the conformance import was landing
  retail's whole pool onto PRISTINE heights, so the fence the
  dwellers bounce off did not exist on our side. LANDED 2026-08-03
  (session 8, dweller-steering dig).** Session 7 closed the siphon
  dig with "the entire residual is x/y/heading on two dwellers
  cruising with retail's blocked-status byte[2]&4 toggling, i.e. the
  shared move-core fence reroute" — the right SITE, the wrong cause.
  - **THE RESIDUAL'S SHAPE.** `verify-deltas --csv`, mc2l24
    t=14680..15180: 665 (5,23) rows on three slots (230/363/364),
    211 heading + 208 y + 206 x + 39 z, one `action` row in 500
    pairs. Retail's heading advances in **exact ±85 steps** — and
    85 is not a law constant, it is `341 − 256`: the move core's
    retry-1 yaw offset (EF:8815) minus behavior row 91's turn cap
    `subtype_160_0x2_2 = 256`. Reconstructing the pairs by hand
    (slot 363, import (30962,26728) yaw 566 speed 24 → retail
    (30970,26751) yaw 651) shows retail stepping at yaw **566+341 =
    907** and then turning −256, i.e. **blocked on the first
    prediction and committing on retry 1**, every tick. The port
    stepped at the un-rerouted 566. Both sides carried the same
    `f34` aim, which is why `action` agreed everywhere.
  - **THE MOVE CORE IS CLEAN — do not re-open it.** `sub_1B8C0`
    (EF:8741-8938) sets `byte[2] |= 4` only on the first-prediction
    block (EF:8812) and clears it only on results 1 and 2
    (EF:8917/8933) — never on a successful retry (result 3) and
    never on the boxed-in result 4. `mobs.rs::mc2_move_core`
    matches that latch/clear pattern exactly, and the import already
    homes byte[2]&4 at `F_BLOCKED` (mobs.rs:81). The retry yaws
    match too (retry 2's byte-split `LOBYTE(v37−85) /
    HIBYTE(((v37−341)>>8)&7)` is provably `(v37−341) & 0x7FF` for
    ALL v37, since the two low bytes differ by exactly 256).
    `sub_102D0(_,_,1)` (EF:3632) cannot block an m23 at all outside
    a cave: row 91's `dword_160_0x14_20 = 0xFFFFFFFF` makes
    `~v_20 & water` identically 0. So the ONLY fence is
    `sub_1B7A0_tile_compare(pred) >= v_16 (=20)` — the heightmap
    second-difference, transcribed byte-exact in `Gen::roughness`.
  - **THE FENCE IS A RISER ENDCAP.** Instrumenting the candidate
    predicate: the port's pristine l24 map reads roughness **13** at
    the tile slot 363 tries to enter (121,104) and **9** at slot
    364's (123,104) — under the 20 gate, so the port walks through.
    Take-wide the pristine map has only **42 of 65536 tiles** at
    roughness >= 20: the walker fence essentially does not exist on
    l24's pristine planes. The imported state explains why — l24
    carries **20 (14,1) risers, ALL at `life = 4`** (idle-REMOVED),
    four of them boxing the compound the dwellers patrol (riser 3
    @(122,104) orient 1 len 15, riser 1 @(121,102) orient 0 len 15,
    riser 2 @(133,104), riser 4 @(121,119)). **A lowered riser is
    not a restored map**: the life-0 INSTANT build raises all `L`
    rows of its 2-wide strip by +48 (EF:41492-41513), while both
    animated phases only ever touch rows `3..L-3` (raise
    EF:41938-41955, lower EF:42159-42203) — so **the strip's 3-row
    ENDCAPS stand at +48 for the rest of the level**, no matter how
    often the (10,63)/(10,64) triggers cycle it. Riser 3's endcap is
    cells (122..123, 104..106) — exactly the fence. With the wall
    replayed, roughness at (121,104) reads **95** and at (123,104)
    **103**, and the port's retry chain reproduces retail's
    positions to the unit (slot 364: blocked at 1180 → retry 1 at
    1521 blocked → retry 2 at 839 → (31765,26686) = retail's want).
  - **THE FIX.** `mc2::riser::mc2_riser_reconstruct` (riser.rs) —
    conformance-import only — rebuilds a riser's cumulative terrain
    write from its own imported state (cell, `f71` orientation,
    `f26` length POST-increment, `act_life` phase, `f44` progress):
    life 3 = the instant build; life 4 = build + the full 48-tick
    lower (type restore included); life 2 = build + `48−f44` lower
    ticks; life 1 = build + full lower + `f44` raise ticks (forced,
    since `f44 < 48` is only reachable through a completed lower —
    the build and the raise both park at 48 and a raise trigger on a
    `f44 >= 0x30` riser is a no-op, EF:41934/42133). Junk
    orientations write nothing (EF:41487); the replayed loop-sound
    47 requests are truncated back off. `world/conformance.rs`
    `retail_import_mc2` runs it over every imported (14,1) after the
    pool lands. This is the ledger's own standing remedy for entry 2
    ("or replay the edit events in the importer") in its cheapest
    form: no recording-format change, the terraform is a pure
    function of state the `.mgcr` already carries.
  - **CONFORMANCE.** Windowed A/B (same binary, replay off/on),
    mc2l24 UNEXPLAINED field rows: t=1000+300 **1057 → 148**;
    t=5000+300 **4856 → 7**; t=14680+500 **16625 → 15536**;
    t=25000+300 **5540 → 4407**; t=45000+300 **227 → 189**. In the
    t=14680 window the (5,23) diff rows go **665 → 2** (both
    heading-only, both exactly 512 apart — a residual retry-leg
    disagreement, new lead) and the collateral is all downward:
    (5,21) −975, (5,17) −838, (5,20) −447, (5,26) −245, (11,2) −63,
    (5,18) −50, (5,25) −22, (9,0) −5, (10,0) −1, nothing up.
    FULL TAKE l24 vs the session-7 close: conforming pairs **4 →
    10**, conforming-or-explained **11,635 → 19,136**, UNEXPLAINED
    field **733,635 → 500,845 (−232,790, −31.7%)**, missing 980 →
    975, extra 6,167 → 6,227, rng 12 → 12. (The tree also carried a
    concurrent session-8 dig, so part of the full-take delta is not
    this fix; the windowed A/B pairs above are the isolated
    measurement.) Suites: **0 regressions, 6/6** — mc2l24 2 fixtures
    DRIFTED (both signature shrinks: t=15288 loses
    `field:5,23:{heading,x,y}`, t=3569 loses `field:10,39:z`),
    promotion owed.
  - **SCOPE.** l24 is the only take with (14,1) THINGs (l0/l4/l30
    have zero), and windowed probes on all three are byte-identical
    with the replay on — no cross-take risk. Gameplay is untouched:
    the reconstruction is reachable only from `retail_import_mc2`.
  - Test: `riser_reconstruct_rebuilds_the_endcap_wall_a_lowered_riser_leaves`
    (riser.rs) — lives one riser through build + trigger + 49 lower
    ticks, then demands the reconstruction rebuild that exact map
    from the terminal state alone, asserts the endcaps at +48 and
    the interior back at flank level, and asserts the map is NOT
    pristine. Proved non-vacuous (neutered arm fails on the
    height-plane equality). No golden moved.
  - **NEW LEADS.** ① The same technique should be pointed at the
    other terraform roots the roster currently rules as capture —
    `mc2-guard-terrain` (1.81M rows), `mc2l24-static-terrain-z`
    (376k), `mc2l24-castle-piece-terrain-z` (367k),
    `mc2-walker-ground-z` (250k), `mc2-terraform-houses` (37k): each
    is a deterministic edit whose source entity is in the imported
    pool (castle stamps, the (14,5) plateau, house regrades).
    ② The 2 surviving (5,23) heading rows are exactly 512 apart —
    a retry-3 / retry-2 leg disagreement worth one narrow dig.

- **MC2L24 HYDRA HEAVY-BOLT BARRAGE UNDER-FIRE — the (5,27) BOLT
  POWER @0x88 was never imported, so 4 of every 5 shots no-opped.
  LANDED 2026-08-03 (session 7, hydra dig).** The session-5 dig-D
  NEXT-LEAD ("a portable head-state bug under the capture noise")
  is REAL and is an IMPORT field-home bug, not a state-machine bug.
  - **RETAIL LAW.** The whip is a FIVE-SHOT burst, not one shot.
    `sub_29A90` case 3 (EF:19889-19928): at `actSpeed == 192` the
    branch sets `byte_0x44_68 = 3`, `dword_0x10_16 = 4` and raises
    `v37 = 1`; on each of the next four ticks sub-state 3 raises
    `v37 = 2` and steps `dword_0x10_16` 4→0 (0 → `byte_0x46_70 = 0`,
    counter re-armed to 1). LABEL_94 (EF:20197-20201) calls
    `sub_2A7F0(ix, v35x, v37 == 1)` on all five. `sub_2A7F0`
    (EF:20507-40): the a3=1 shot ROLLS the power
    `manaRegen_0x88_136 = (rand%12 > 7) + 1` (4/12 = power 2) and
    perturbs the branch LCG by `setting_30`; the four a3=0 re-fires
    only READ it back — power 2 spawns `(9,9)` on every one of the
    five, power 1 spawns one `(9,0)` and four no-ops
    (`if (!a3) goto LABEL_13`, EF:20524).
  - **PORT DEFECT.** `m27_branch_bolt` (multipart.rs) keeps the power
    in `f136` — but `import_ent_mc2`'s UNIFORM MC2 map spends f136 on
    `@0x8C` (`f136: r.mana_max`), and `@0x8C` is DEAD 0 on the whole
    (5,27) family. Under per-pair replay every pair therefore re-read
    the power as 0, the four re-fires fell into the `_ => return`
    arm, and each whip laid ONE arc instead of five. The obs
    `mana_max` lane took the same collision from the other side: the
    native low-roll wrote f136 = 1|2 mid-tick and the projection
    reported it where retail reads @0x8C = 0 (30 rows in window 1).
  - **CORPUS PROOF** (mc2l24 t=10650-11150, retail heads 16/26/36/
    46/56 + body 15). 30 whips in the window, 9 of them power-2.
    Per-tick `(9,9)` census, pre-fix: at every mid-burst tick the
    port reproduced the AGED generation (life −2) exactly and missed
    precisely the NEWBORN one (life −1) — e.g. t=10704/05/06/07
    missing 82/82/81/81 with the aged 78×(0,−2)+3×(−1,−2) matching
    row-for-row, and 0 missing on the FIRE tick (t=10703) and on the
    post-burst tick (t=10708). Retail's own churn confirms the
    5-shot burst: arcs born at 10703 (82 nodes) and 10704 (81), then
    new=1/gone=1 per tick through 10707 = a fresh 81-node arc every
    tick landing in the LIFO-freed slots. Family census over the
    whole take (every 37th tick, 87,210 (5,27) rows): `@0x8C` = 0 ×
    87,210; `@0x88` = 0/1/2 (6,876 non-zero) — the lane is free.
  - **FIX** (conformance-only, (5,27)-scoped; native untouched):
    `import_ent_mc2` `f136: if m27 { r.d88 } else { r.mana_max }`
    (conformance.rs:1367) + the (5,27) reverse-map `row.mana_max = 0`
    in `obs_project_mc2` (conformance.rs:977), matching the class-15
    and class-10 precedents. multipart.rs's field-map doc updated.
  - **NUMBERS**, `(9,9)` missing/extra. Window 1 (t=10650-11150):
    **1,530 / 94 → 89 / 175**; whole-window entity sets 1,727/122 →
    258/204; `mana_max` field rows 30 → **0**; rng 0/502 both sides.
    Window 2 (t=43100-43600): **1,686 / 0 → 166 / 0**. Full take on
    the current tree: `(9,9)` **46,241 / 1,967 → 13,632 / 3,052**
    (−71% missing); unexplained field 771,218 → 733,089, unexplained
    missing 1,209 → 1,006, pairs fully explained 11,127 → **11,634**.
    Suites 6/6 green, 0 regressions (the one l24 drift at t=15288 is
    the (5,23) dweller dig's, and improves). Goldens UNMOVED — the
    sim law is untouched, so no state hash changes.
  - **RESIDUAL, ruled capture.** What is left on `(9,9)` is BEAM
    GEOMETRY, not cadence: the arc node positions are a per-node
    `ent_rand` walk off a freshly-spawned beam (5,583 rows now
    classified `mc2-cast-timing-fields`), and the arc LENGTH is
    `steps·8` where `steps` is the beam's terrain walk — on l24 the
    port's terrain z runs high across the level (`mc2l24-static-
    terrain-z` 502/502 pairs in this window), so a beam that walks
    until it meets ground overshoots. The four extra-heavy fire
    ticks (t=10757/10797/10822/10940: retail 34/18/34/18 nodes vs
    port 63/40/57/33) are exactly the whips fired from the highest
    heads. Terrain-closure = the standing capture class; NO new
    roster rule needed (the existing `mc2-cast-timing-*` and
    `mc2-lightning-blast-churn` rules absorb it — window-1
    UNEXPLAINED went 6,807 → 6,810 field / 1 → 1 missing / 0 → 0
    extra).
  - Non-vacuous tests: `mc2_m27_import_field_homes`
    (conformance.rs — the five (5,27) homes with distinct
    sentinels, f136 = 2 not 999) and
    `mc2_m27_refire_needs_the_imported_bolt_power` (world.rs — a
    sub-state-3 branch lays a `(9,9)` at power 2, NOTHING at power 1
    (EF:20524) and nothing at power 0, which IS the pre-fix import).
  - NEXT: the m0/m3/m22 siblings share `sub_2A7F0`'s caller shape
    only through m27; no extension owed. The (10,23) impact family
    (826/5) shares the beam-walk root, not the cadence one.

- **THE (5,23) DWELLER'S MANA SIPHON IS A BALL-SIDE MECHANIC — the
  sphere flies to the collector, and that arm was unported. LANDED
  2026-08-03 (session 7, dig ④).** Player report: "the dwellers hover
  down to a mana ball but the ball is never attracted to them for
  pickup, so they never collect".
  - **RETAIL LAW, DWELLER SIDE** (`sub_27C10` EF:18211-93). The
    siphon does not move anything: it MARKS. Each tick it re-asserts
    `node.byte[0] |= 0x40` and `node.word_0x96_150 = self`
    (:18268-69), bumps its OWN `word_0x2C_44` by 10 (:18270, seeded
    to **18** on the arrival tick :18238 alongside the 64-tick
    `dword_0x10_16`), and swallows when `sub_106C0(self, node)` — the
    3-axis EXTENT overlap, not a radius — or `node.z > self.z`
    (:18271-76). Control flow is a **fall-through**: sub 0 seeds and
    then runs the body in the same tick (only sub ≥ 2 jumps to
    LABEL_24), so the grab, the first +10 and the first swallow test
    all land on the arrival tick.
  - **RETAIL LAW, BALL SIDE** (`TransformArcherToMana_35940`
    EF:26111-72; the (10,57) twin `sub_35FB0` EF:26385-447 is the
    same code). A sphere carrying `byte[0] & 0x40` runs NO physics
    and instead flies to `Entities[word_0x96_150]`, admitting exactly
    two collectors and dropping the grab for anything else
    (:26115-27): the **(3,3) balloon**, z step the constant 32, and
    the **(5,23) dweller**, z step = *the collector's own
    `word_0x2C_44`* (:26120) — i.e. the ramp above, so a siphoned
    sphere accelerates upward 28, 38, 48, … Then: own `word_0x2C_44
    = 128` (the release pop, :26135), yaw at the collector, and on
    the 2-D gap (`EuclideanDistXYZ_58490` sums X and Y only —
    utilities/Maths.cpp:738) either a 16/tick horizontal step
    (≥ 16), or an x/y SNAP plus the z servo into the band
    `[collector.z, +512]` (< 16), ground-clamped; past 1024 the ball
    releases itself (:26169).
  - **CORPUS PROOF** (mc2l24, dweller slot 363 / sphere slot 360).
    t=14512 act 187/0, `f2c` 8192, timer 0, sphere unmarked → t=14513
    act 187/1, `f2c` **28**, timer **63**, sphere `byte[0]` 0x0C →
    **0x4C** with `+150 = 363`: one tick, seed AND body — the
    fall-through. The sphere then walks 16/tick (28330,31292) →
    (28228,31242), snaps to the dweller's x/y at t=14521 and rises
    444 → 542 → 650 → 768 → 896, i.e. **+98, +108, +118, +128** —
    the dweller's `f2c` at each tick, exactly. Swallow at t=14524
    (mana 100 → 2300, sphere `byte[1] |= 4`): the extent test
    `|(1120+280) − (896+70)| = 434 < 384+70` fires that tick and not
    at t=14523 (562). All 14 siphon entries between t=14512 and
    t=15648 enter with `dz ∈ [588, 701]` and a 2-D gap ≤ 121.
  - **FOUR PORT DEFECTS.** ① the ball-side arm existed but admitted
    the balloon ONLY (mc1/combat.rs `ball_tick`) — a grounded sphere
    under a hovering dweller never rose, the swallow could never
    fire, and every siphon burned its 64 ticks into an eternal
    re-hunt: the reported symptom. ② the siphon arm returned on the
    arrival tick instead of falling through, and used
    `mc2_dist3 < 256` for `sub_106C0`. ③ the descend arm
    (`sub_27B20` :18250 → `sub_28390` :18580) handed over on a bare
    2-D 256 with NO altitude condition; retail station-keeps **640
    above the node within ±64** and inside a 2-D reach of **128**,
    runs the mover ONLY outside that reach, and aborts on
    `sub_28060` (:18415, the anti-stack lift) — all three unported.
    ④ the hunt arm re-aimed and range-tested every tick; retail rides
    the `byte_0x3E_62 & 3` cadence (:18140).
  - **THE IMPORTER COLLISION.** m23 is the second model (after the
    m27 hydra) whose machine runs on `word_0x2C_44` while our column
    homes `subSpellIndex_0x2A_42` at f44; the uniform map therefore
    handed every imported dweller a flat **500** where the ramp
    belongs, lifting its sphere 500/tick. `world/conformance.rs` now
    imports @0x2C for (5,23) too; @0x2A has no reader on our side
    (the (9,9) bolt launcher stamps its own payload).
  - **FOOL'S MANA.** `sub_28000` (:18384) and `sub_28420` (:18603)
    filter `model == 39` out of a list that DOES carry the (10,57)
    trap spheres (:40018-63 files models 39/40/57 into
    `dword_38523`), and the balloon fleet's `sub_5F810` (:61005)
    filters the same way — neither ever takes fool's mana. Our native
    m57 keeps the (10,39) family model and carries retail's action
    0x3E, so all three scans now exclude action 62.
  - **CONFORMANCE** (windowed verify-deltas, mc2l24). Pairs
    t=14500..14680 (180): **(5,23) rows 899 → 0, (10,39) rows
    90 → 0**, total diff rows 13880 → 12741, UNEXPLAINED 6251 →
    5648 (baseline = the session-5 CSV, same window). The importer
    fix alone took that window's (5,23)/(10,39) from 191/13 to 0/0.
    Pairs t=14680..15180 (500, ~12 siphon entries): (5,23) 2542 →
    665, (10,39) 309 → 29 (all `mc2l24-ball-terrain-roll`), and the
    siphon-dense half t=14680..15000 is **completely clean** — the
    entire residual is x/y/heading on two dwellers cruising in
    185/0 with retail's blocked-status byte[2]&4 toggling, i.e. the
    shared move-core fence reroute, not the siphon. OPEN LEAD.
  - Tests: `leviathan_siphon_lifts_a_grounded_sphere_and_swallows_it`
    (proved non-vacuous — without the collector admission the sphere
    never leaves the ground), `leviathan_stations_640_above_its_node_before_siphoning`
    (end to end from the ctor cruise altitude),
    `leviathan_never_siphons_a_fools_mana_sphere` (also non-vacuous).
    No golden moved.

- **THE l24 VISSULUTH ENDGAME — the demon is the (5,10) DOOMSDAY
  PYRAMID, and the port was drawing it through both invisible phases
  and running its death animation on the wrong clock. LANDED
  2026-08-03 (session 7, dig ③).** Player report: the demon sprite
  pops in the instant the pyramid is destroyed and then stands idle
  and indestructible until you fly into it; its death animation
  loops (keel over, get up, keel over) until the growing mana
  fountain buries it.
  **PLAYTEST 2026-08-04 (partial): keel-over/death CERTIFIED —
  "now OK, doesn't loop" (the one-shot anim-timer fix confirmed).**
  **RETAIL RE-VERIFY DONE (player, 2026-08-04) — the "second
  visibility gate" hypothesis is DEAD.** Retail DOES draw the demon
  body through the wait phase, exactly as the port does — but
  **scaled down TINY**, easily overlooked through the smoke; on slow
  approach it scales UP, becomes big and starts attacking BEFORE the
  narration stage trigger (attack trigger = the 0xA00 proximity
  activation, separate from the StageVar — both confirmed live).
  **⚠ THE "BLEND-ONLY, FULL SIZE" READING BELOW WAS REFUTED BY PLAYER
  FOOTAGE (2026-08-04). THE SIZE LAW IS REAL — see "THE VISSULUTH SIZE
  LAW" further down. Both effects are retail and simultaneous; the
  blend paragraphs stand, only the "there is NO size term" claim
  fell.**
  **⭐ PLAYTEST CERTIFIED (player, 2026-08-04, same day): "Confirmed
  through playthrough. The shrinking is now faithful." — the size law
  (20× wait-phase shrink + meter-driven growth ramp, with the
  render-side smooth-lerp deviation) is CLOSED. Remaining owed on this
  story: the 3-D wake-gate distance nit (doomsday.rs, blocked on the
  fountain dig's file ownership).**
  **WAIT-PHASE RASTER MODE TRACED + LANDED 2026-08-04 (session 8).**
  The suspect bit does mean translucency:
  - `DrawSprites_3E360` GRO:3779-3806: `byte[2] & 0x80` (flags bit
    23) takes the flag-override arm and, absent the player-colour
    bits, forces `str_F2C20ar.dword0x01_rotIdx = 2` (GRO:3805).
    Mode 2's inner loop (GRO:4525-4562, index at 4546/4559) is
    `if (src) *dst = T[0x4000 + (src<<8) | *dst]` — the TABLES
    256×256 blend matrix, empirically `nearest_palette(⅓·row +
    ⅔·col)`, so the sprite contributes ⅓ and the background ⅔:
    **33%-opaque, full size** (docs/traces/mc2-transparency-
    drawlist.md §4-§5).
  - There is no size term on the FLAG (this part still holds, and it
    is what sent the first pass wrong). Sprite height is
    `dword0x18 * particlesParameters_D951C[word_0x5A_90].rotSpeed_8
    / depth` (GRO:3770-3772) and nothing in `DrawSprite_41BD3` is
    keyed on `rotIdx` except the per-pixel writer — the mirroring/edge
    setup that owns `realWidth`/`realHeight` (GRO:4038-4400) is shared
    by all nine modes, and modes 0 and 2 step the source identically
    (`v53`/`v70` +2 dwords per pixel, GRO:4432-4490 vs 4526-4562), so
    mode 2 neither decimates nor blits raw. **The size instead moves
    because retail REWRITES THE TABLE — see below.**
  - Corroboration that mode 2 = the faint end of a fade RAMP: the
    generic effect tail at EF:26290-303 walks a dying entity
    opaque → `byte[3] |= 1` (mode 3, 67%) at life 12 → `byte[2] |=
    0x80` (mode 2, 33%) at life ≤ 6 → `DisableEntityDrawing` at 0.
  - **THE PORT BUG was a bad carve-out, not a missing law.**
    `World::live_poses_mc2` already mapped bit 23 → blend 2, but
    excluded `(5,10)` on the theory that the boss draws through
    `sub_3FD60` (GRO:2205-12), whose rotIdx comes from the static
    descriptor alone. That theory is WRONG: `sub_3FD60`'s only two
    call sites (GRO:1260, GRO:1327) sit inside the
    `m_Graphics.m_wReflections` block opened at GRO:1104 — it is the
    WATER-MIRROR pass. Every main-world per-tile billboard call is
    `DrawSprites_3E360` (GRO:900/1026/1775/1841), which reads bit 23.
    FIX: carve-out deleted (`engine/world.rs` `live_poses_mc2`), so
    the boss exports `blend = 2` from hide-clear (t=51645) until the
    0xA00 wake (t=51732) and `blend = 0` after. Scope is the plain
    bit-23 rule with no exception — nothing else changed lane.
  - Corpus confirmation of the lane the pose export reads (slot 7,
    `dump-state`): t=51650 and t=51731 flags `0x4880000C` (bit 23
    SET, hide bit clear, f2a `0x50` = the proximity-watch arm) →
    t=51733 flags `0x4800000C`, f2a `0x60` (doom-meter ramp). The
    native ctor stamps the same bit (`mc2::doomsday` `flags |=
    0x4880_0001`) and the same clear (`flags &= !(1 << 23)`), so
    replay and native runs read one storage.
  - Render side needed NO change: `LivePose.blend` → `Billboard.blend`
    → alpha ⅓ on `billboard_blend_pipeline` (mgc-render) was already
    wired for smoke. Presentation-only: `observable_digest` hashes
    only pose `type_index/x/z`, never `blend`, and `state_hash` never
    sees the export — `MGC_REQUIRE_GOLDENS=1 cargo test -p mgc-sim`
    green, no golden moved.
  - **★ THE VISSULUTH SIZE LAW — A SELF-MODIFYING SPRITE-PARAM ROW.
    TRACED + LANDED 2026-08-04 (session 8), after player retail
    FOOTAGE refuted the blend-only reading.** Player's two frames:
    (1) wait — the boss is a tiny handful of pixels at the base of the
    smoke column, gone entirely in lowres; (2) closing — it enlarges
    GRADUALLY and TICK-STEPPED ("several mid-step images getting
    bigger and bigger but nonetheless separate images") into the full
    demon bust, and the smoke stops. No entity lane carries it (corpus
    slot 7: box extents identical 1024/1024/1280 at t=51700 and
    t=51780; f5a 341→343; rows 341-345 all authored `rotSpeed_8`
    0x4B0). **The boss rewrites its own row in the STATIC TABLE.**
    Both writes decompile as stores into `x_BYTE_D9F50`, which is a
    MIS-SPLIT ALIAS of `particlesParameters_D951C`:
    `0xD9F50 − 0xD951C = 0xA34`, and remc2 itself flagged the symbol
    ("`x_BYTE_D9F50 - ? used only byte 0x87A,0x5b6,0x126 (error?)`",
    EventsFunctions.cpp:92) while its own data dump at EF:2422-24
    carries the D951C row constants `0x212C` / `0x002A` verbatim.
    Address arithmetic (14-byte rows, height at row offset 8):
    `&x_BYTE_D9F50[0x87a]` = **0xDA7CA** = row 341 (starts 0xDA7C2)
    `+8` = **`D951C[341].rotSpeed_8`** — the demon's own draw height.
    The decompile even leaves the breadcrumb `// DA7CA: using guessed
    type __int16 x_WORD_DA7CA;` at EF:12883/13097. The other two
    aliased offsets resolve the same way (0x5b6 → row 291 `word_0`,
    0x126 → row 207 `rotSpeed_8`), so the pattern is general.
    - **EF:12700** (state 0, the ritual start): row 341 height := 60.
      Against the authored 1200 that is a **20× linear shrink** —
      0.23 tiles, a few pixels, invisible at 320×200. This is the
      wait-phase floor, and it holds for the whole dormancy.
    - **EF:13041** (the `f44 & 0x20` doom-meter arm, armed the tick
      the 0xA00 proximity clears bit 23): row 341 height := the meter,
      which steps **+30/tick from 30 to 1200 over 40 ticks** — the
      filmed stepped growth. Corpus: scratch10 = 60 @ t=51733, 420 @
      t=51745, meter capped 1200 @ t=51772.
    - The ramp ENDS on the authored 1200 and never writes again (the
      meter's later reuse as a state timer — scratch10 = 3 @ t=51780 —
      lives in a different arm), so the attack rows 342-345 (never
      patched) and the post-attack idle back on row 341 all draw full
      size. That is why the demon does not re-shrink between attacks.
    - **PORT.** `LivePose.sprite_h_units: Option<f32>` (new,
      presentation-only) carries the patched row height; `live_poses`
      exports it for MC2 poses with `type_index == 341` from
      `World::mc2_doom_meter`, which is ALREADY the port's mirror of
      that field (`mc2::doomsday` writes 60 at state 0 and the meter
      each ramp tick — the module header already named it
      `x_BYTE_D9F50[0x87a]`, it was just never wired to the draw).
      `mgc-app::entities::billboards_from_poses` uses it in place of
      the baked `rot_speed_8`; the renderer re-derives width from the
      frame aspect exactly as retail does, so one field is the whole
      law. `mc2_doom_meter == 0` = never patched ⇒ the authored row
      stands. Hash-quiet: nothing new is hashed, `observable_digest`
      hashes only pose `type_index/x/z`, `mc2_doom_meter` was already
      sim state. `MGC_REQUIRE_GOLDENS=1 cargo test -p mgc-sim` green
      (340/340 lib + every suite), no golden moved.
    - **RENDER SMOOTHING — deliberate presentation deviation
      (player-requested).** Retail steps the size once per SIM TICK
      (the player can see the discrete images). The port lerps
      `sprite_h_units` on the same frame alpha as the transforms in
      `mgc-app::entities::lerp_poses`, so the growth is continuous at
      display rate. The sim law itself is untouched: +30/tick,
      exactly retail. Render-path only, no DEVIATIONS.md sim entry.
  - Tests (crates/mgc-sim/tests/mc2_slice.rs):
    `mc2_doomsday_is_tiny_until_the_proximity_wake_then_grows_to_full_size`
    — asserts (blend 2, height 60) through the wait, then a monotone
    ramp with mid-growth samples settling exactly on 1200; and
    `mc2_doomsday_growth_ramp_exports_lerpable_size_steps` — the tick
    pair differs by exactly +30 so a 0.5 alpha lands mid-step. Both
    proved non-vacuous (neutering the export fails them:
    `left: Some((2, None)), right: Some((2, Some(60.0)))`); the blend
    leg is separately non-vacuous (restoring the `(5,10)` carve-out
    fails it with `left: Some(0), right: Some(2)`).
  - **SMOKE-STOP: ALREADY FAITHFUL, nothing owed.** The wait-phase
    smoke is the (10,14) falling-rock ring the machine spawns every
    tick. Retail gates it on `v27`, cleared by
    `if (dword_0x10_16 >= 600) v27 = 0` inside the ramp arm
    (EF:13029) — i.e. the ring stops HALFWAY up the growth
    (meter 600 ≈ t=51752), not at ramp start. The port matches
    verbatim: `suppress_ring` at doomsday.rs:677 under the same
    `f26 >= 600` test, consumed at doomsday.rs:689. READ-ONLY check,
    nothing landed in doomsday.rs.
  - **ATTACK GATE RE-CHECKED, FAITHFUL.** `mc2_pyramid_attack`
    (doomsday.rs:650-673) arms only on `f44 & 0x10` + `f44 & 0x40`
    and the 0xA00 squared-distance test, then the caller flips state
    1 → 4 with `f44 |= 0x80` (doomsday.rs:337-340). No StageVar /
    narration row is consulted anywhere in the escalation. One nit
    left for the doomsday.rs owner: retail's gate is
    `EuclideanDistXYZ_58490` (3-D, EF:13010) while the port compares
    `dx²+dy²` only — a plan-distance approximation that ignores the
    player's altitude over the crater.
  - **WHAT THE DEMON IS.** l24's THING table: slot 373 = a (10,45)
    BUILDING (BLDGPRM id 68) at (40,212), dis-gated 28, `child` 29 =
    its ON-DEATH disposition; slot 379 = the (5,10) doomsday machine
    at (40,213), dis 29. So destroying the pyramid *building* fires
    dis 29 and spawns the boss — corpus t=51557 (slot 7, act 80,
    life 300000/300000). The "reach the centre" goal is a separate
    STAGE row (stages.json checkpoint `index 5, stage 0` at
    (40,212), objective kind 5 fly-to-point, `World::mc2_objectives`
    :40803-14) and the kill goal is `index 1, stage 379` (kind 1,
    bound to the boss THING) — both already ported, neither gates
    the spawn. The mana "fountain" is the state-0xF (10,9)
    APOCALYPSE dome → its life-3 (10,91) mana rain (t=63308), which
    RAISES the land over the corpse — hence "buries it".
  - **RETAIL DORMANCY LAW.** The ctor's `|= 0x48800001` (EF:33980)
    carries **byte[0] bit 0 = the billboard hide bit**: the MC2
    sprite pass skips `byte[0] & 0x21` (GameRenderOriginal.cpp:3157
    `DrawSprites_3E360`, mirrored NG:2838/HD:3235, plus the
    sub_3FD60 gather at GRO:1936). The boss is therefore INVISIBLE
    through its opening ritual (crater flatten + the 70-tick
    kill-all) and the kill-all exit clears the bit (EF:12983) —
    corpus slot 7: flags `0x4880000d` at t=51557 → `0x4880000c` at
    t=51645 (88 ticks). It then waits, still un-damageable (state 1
    never calls `sub_22190`), until the player closes inside 0xA00 =
    10 tiles, drops the ctor's raster-mode bit (`byte[2] &= 0x7F`,
    EF:13024 — corpus `0x4880000c` → `0x4800000c` at t=51732) and
    ramps the doom meter into the attack cycle at t=51772. **PORT
    BUG**: `live_poses` only honoured bit 5 (0x20), so the boss was
    billboarded from spawn. FIX: `World::live_poses_mc2` now skips
    `flags & 1` for the **class-5** column — the doomsday machine is
    the only MC2 class-5 ctor that writes bit 0, so the widening is
    provably scoped (see DEVIATIONS.md's multipart entry for why it
    is NOT global). `mc2::doomsday` also lands the missing
    `byte[2] &= 0x7F`.
  - **DEATH ANIMATION = ONE CYCLE.** `sub_221F0` (EF:13667-72): for
    sprites 343/344/345 (0x157..0x159) the state timer is re-seeded
    from the TMAPS animation's `CountOfFrames_16`, so those states
    last exactly one animation cycle; the cases' own seeds
    (16/16/32) are pre-override values. The sim carries no frame
    table, so the counts are PINNED FROM THE CORPUS (slot 7 b46
    dwell): **343 → 5** (states 6+7, t=51778..51782 and 3 more
    cycles), **344 → 15** (0xA+0xB, 51793..51807), **345 → 20**
    (state 0xE, 63201..63220). State 0xD is 32 and 0xF is 60 —
    both already right (63169..63200, 63221..63280, despawn 63281).
    With 0xE at 32 the death animation over-ran its cycle AND the
    port kept drawing the corpse through 0xF (retail re-sets the
    hide bit at the end of 0xE, EF:12846) — 92 visible ticks of a
    globally-looping FLC instead of 20. Both halves fixed.
  - **CONFORMANCE.** Windowed verify, pairs t=51500..52699 (1200):
    explained pairs **65 → 137**, UNEXPLAINED rows **8494 field →
    4536** (missing 12 → 14, extra 228 → 232, rng 1/1200 both).
    Gross families all down (x 7875→6021, y 7602→5744, heading
    2761→2388, action 3157→3129, model 584→566, class 478→464, life
    946→899). The timer pin is what moved it: with the wrong dwell
    the port's summon/ring cadence and per-tick LCG draws sat one
    phase off retail for the whole fight.
  - **SUMMON LIFE-LATCH FIELD HOME (report C, partial).** The
    pyramid's summon block stamps `word_0x2E_46 = 250` (EF:13419);
    the port wrote it to **f46**, which on a creature is
    `fontTypeIndex_0x3D_61` — for the selector-3 (5,0) worm that is
    the projectile-DODGE alert window (`m0_dodge`), so every worm
    summon was born with 250 ticks of phantom dodging armed, and the
    latch had no import home at all (the class-5 @0x2E lane is
    **f26**), so a replayed summon read ≈0 and puffed itself on its
    first ticked pair. Moved to f26 in `mc2::doomsday` +
    `mc2::mobs::mc2_doom_summon_{home,spinup}_tick`, with a
    conformance import arm so a StageVar2 16/17 (5,0)/(5,27) summon
    keeps @0x2E instead of the m0 bob velocity.
  - **~~STILL OPEN~~ → SUPERSEDED 2026-08-05. The earlier "the
    standing husk is RETAIL LAW" reading was HALF the law and the
    half it missed is the whole bug — see the next entry.** The
    latch write is real (`sub_1E700` v2==2 → `word_0x2E_46 = 1`,
    EF:10864-66; `sub_1E580`'s head only zeroes the latch when the
    PARENT pyramid is gone, EF:10699-10701), but retail NEVER
    STRANDS on it, because the caller runs an escape hatch after
    the core returns that the port `return`ed past — **FIXED and
    landed the same day** (see "VISSULUTH'S SUMMONS — THE 'FROZEN
    HUSK FOREVER' IS A PORT BUG"). Do not cite the old paragraph.
  - **NOT A BUG: the l24 "fountain sphere model 54".** The (10,54)
    rows at (41,212) z 2845..3600 over t=63971..68788 are NOT a
    wrong fountain model — they are `AddAuxiliary_50500` (EF:36812)
    MANA-MAGNET AURAS from the player's own possession casts on the
    raining spheres (life 128/128, act 59, speed 256, applied_yaw 0
    / applied_pitch 1024 = the ctor's ShiftRot(1024, 0x4000), mana
    0 — an exact ctor fingerprint), spawned at each possess impact
    point above the fountain. The fountain itself rains (10,39):
    `sub_32CF0` (EF:24030) calls `_4A190(&pos, 10, 39)` verbatim and
    the port matches. The slot skew is the port's missing (10,12)
    claim-pulse ENTITY — retail spawns BOTH the pulse and the aura
    per magnet impact (EF:59036-59054) while the port only runs the
    `area_write` — which shifts every subsequent slot by one (the
    4 `want=12 got=54` rows). Ported pulse entity = a separate
    lead.

- **THE l24 START SPHERES ARE ALL FOOL'S MANA AND THE PORT HANDED
  THEM OVER — the (10,57) claim intake was gated on a CAST-DECOY
  marker retail does not have — LANDED 2026-08-03 (session 7, dig
  ②).** Player report: on mc2l24 every mana ball on the ground at
  level start is fool's mana in retail — possess one and it fires
  back; the port let you collect them.
  **PLAYTEST FOLLOW-UP (same day): "fires the trap fireball in the
  wrong direction — not at the player" — FIXED.** `mc2_fools_bolt`
  only aimed at POOL-entity claimers; the human sentinel fell back
  to the sphere's stale launch heading (junk for an authored ground
  sphere) on the assumption the flyer autoaim would re-acquire.
  Retail `sub_36770` aims `sub_655C0` at the CLAIMER entity — the
  human included (retail humans are in-pool). Fix: ctx threaded
  through `mc2_fools_retaliate`/`mc2_fools_bolt` (cast.rs) and the
  human claimer resolved via the ctx pose exactly as every creature
  attack aim does (`Gen::mc2_target` convention). Non-vacuous lib
  test `fools_trap_fireball_aims_at_the_human_claimer` (world.rs;
  neutered arm leaves at yaw 5 vs expected 295 → fails). NOTE
  found while testing: a GROUND-level muzzle detonates the bolt on
  its first step — that is OPEN-5's deferred `+fov` launch lift
  (the l24 bait hangs at z 1280..3840, so live traps fire fine). **LAW.** (10,57) is retail's
  RANDOM-VALUE sphere: ctor `sub_50130` (EF:36631) stamps action
  **0x3E**, whose handler `sub_35FB0` (EF:26318, strA0 row 62) is
  the (10,39) ball's twin EXCEPT in the claim intake. The ball
  transfers ownership + chimes sound 4
  (`TransformArcherToMana_35940` EF:26069-94); the m57 instead runs
  `else if (word_0x68_104 && sub_36680(a1x)) { _4A190(&pos,10,0);
  DisableEntityDrawing04(a1x); }` (EF:26362-66) — the FOOL'S-MANA
  trap. `sub_36680` (EF:26615) has **no owner precondition**: its
  only skip is `parentId == claimer` (EF:26623), so a sphere with
  the NewEvent defaults (`parentId` 0, `byte_0x46_70` 0) is a live
  TIER-0 trap for everyone → one homing `(9,0)` fireball
  (`sub_36770` EF:26672, `word_0x96_150` = claimer, sound 9),
  `sub_6D8B0(parentId,22,1)`, consume. **CORPUS PROOF (this is what
  resolved the audit doc's OPEN-2).** t=0 census: 21 authored
  (10,57), slots 67-87, all `own=0 pe=0 act=62 flags=0x2000c`, raw
  **b46=0, owner28=0, f2a=100**. All 21 die in t=0..1836 and **every
  one dies by the trap, none by damage or collection**: the tick
  before each death the ch1 mail SOURCE (= `word_0x68_104`) flips to
  116 = the human, written by a co-located (10,12) possess pulse
  (`PossesHitMana_320E0` → `sub_112D0` EF:4199); the next state has
  the sphere at `flags |= 0x400` with life still 300/300, a **(10,0)
  poof at its exact position** and a **(9,0) fireball with tgt=116**.
  Slot→last-m57-tick→poof/fireball: 67→1322→569/589 ·
  68→1355→489/589 · 69→1358→622/402 · 70→1422→589/75 ·
  71→1402→75/622 · 72→406→539/627 · 73→854→524,618 · 74→294→524/599
  · 75→786→430/524 · 76→1132→432/620 · 77→998→145/144 ·
  78→1452→75,179 · 79→1531→228/271 · 80→1649→280,340 ·
  81→1718→326/342 · 82→1700→345/363 · 83→1693→161/285 ·
  84→1573→155,322 · 85→1515→310/363 · 86→1835→469/478 ·
  87→959→609/73. **PORT BUG.** `ball_tick`'s fool arm gated on
  `is_fool = Mc2 && f52 != 0` (mc1/combat.rs) — a marker only the
  spell-22 cast wrote — so authored spheres (f52 = 0) fell through
  to the ownership-transfer arm: `f144 = 116`, sound 4, sphere kept.
  Worse, the whole round-1 trap used PORT-PRIVATE lanes the importer
  never feeds (f50 tier, f136 payload, f146 claimer, f56 counter, f52
  owner) — `f136` is the observed `mana_max` lane and `f146` is the
  balloon-tether target. **FIX.** (a) gate = the (10,57) identity
  `model65 == 57 || tick70 == 62`; (b) every trap lane re-homed onto
  the RETAIL field the importer already carries — parentId `id24`
  (@0x28), tier `f71` (@0x46), payload `f44` (@0x2A), counter `f26`
  (@0x10), and the claim latch IS `mail[1].1` (@0x68), never cleared
  except on the owner arm; (c) the missing **(10,0) consume poof**
  (`mc2_spawn_fire`) + `flags |= 0x400` soft kill, and a claimed
  sphere runs no physics that tick (retail's `else if`); (d) tier > 3
  = no trap, no transfer, latched forever (EF:26665); (e) NATIVE arm:
  `mc2_spawn_mana_sphere(57, …)` now stamps `tick70 = 62`
  (`sub_50130`'s action) so the l24 authored spheres are trap-armed
  in real play — this is a gameplay fix, not only a conformance one.
  World-mana census now excludes CAST decoys only (action-62 with a
  real caster in `id24`); authored spheres count exactly as before.
  **NUMBERS (windowed t=0..2200, the whole life of the 21 spheres;
  before/after measured 20 min apart with only this change between).**
  UNEXPLAINED rows **8,007 → 7,963 (−44)**, and the entire delta is
  the (10,57) family: **227 → 183** (`player_ent_idx` 23 → **0**, x
  80→71, y 67→60, z 57→52); every other family byte-identical (5,19
  6935, 10,16 463, 10,17 168, 10,18 113, 10,19 65, 3,3 13, 10,42 13).
  Entity sets in-window: missing **2,086 → 2,046**, extra 487 → 491;
  **(9,0) missing 12 → 3**, (10,0) missing 284 → 270, (10,12) missing
  119 → 117, (9,1) missing 17 → 14, `mc2-cast-timing-missing` at the
  21 spring pairs **10 → 1**. Every newly-spawned (9,0)/(10,0) row is
  absorbed by the existing `mc2-cast-timing-fields` /
  `mc2-fire-churn-m0` rules — **zero new unexplained rows**. Whole
  take (l24, after): 69,207 pairs, 11,135 explained, unexplained
  773,459 field / 1,074 missing / 6,085 extra, **(10,57) 0 missing /
  0 extra** — identical to the figures dig ① banked, which were
  measured with this change already in the shared tree, so the
  whole-take attribution lives in the windowed run above. All six
  fixture suites 0 regressions / 0 drifted; mgc-sim 313 + 28 green
  (one new test), MC1 goldens unmoved (`level_005_golden_state_hashes`,
  `flight_tier_golden_state_hashes` under `MGC_REQUIRE_GOLDENS=1`).
  Test `mc2_authored_ground_sphere_is_a_tier0_trap` is two-sided and
  non-vacuity-proven (restore the `f52 != 0` gate → 0 fireballs).
  RESIDUE: the 183 remaining (10,57) rows are the sphere SETTLE
  PHYSICS (x/y/z drift on the authored spheres' first ~300 ticks),
  a different machine; and two DEFERRED items recorded in
  docs/spell-audit/fools-mana.md §6 — the bolt's `+= array_0x52_82.fov`
  launch lift (OPEN-5, ~42 units; our victim probe admits the
  launcher sphere) and the port's `model65 = 39` on natively-spawned
  spheres (OPEN-6; the action lane carries the law, the model residual
  would need a sweep of every `model65 == 39` sphere gate).

- **MC2 SWITCH VOLUMES LOST THE HUMAN'S OWN HALF-EXTENT — the
  (10,25)+(10,75) "unported doomsday spawns" were a SWITCH-BOX
  MISS — LANDED 2026-08-03 (session 7, dig ①).** The paired lead
  was misattributed: neither family belongs to the doomsday
  pyramid. On l24 every one of the seven bursts is a **(11,2)
  repeating enter-switch** releasing its disposition — a storm of
  `AddWind_4F040` whirlwinds (each 1 head (10,22) + 11 (10,75)
  funnel nodes) plus a scatter of `sub_4F6A0` (10,25) area blasts
  at authored tile centres (corpus proof: at t=34041 slots
  8/122/135 are three fresh heads maxLife 40 = 8×tier-1 charge,
  `word_0x2A_42`=100, plus 8 blasts maxLife 8 / subSpell 2000 /
  action 25 — and switch slot 93 at (144.5,109.5) steps
  `dword_0x10_16` 0→10, the 10-count rearm, on exactly that tick).
  **LAW.** `sub_6F0B0` (:54408) → `InitSwitchChainZaxisAndSound_
  6F850` (:44523) walks the wizard list `dword_38519` for a
  class-3 **model-0** entity (AI wizards are model 1,
  `sub_4A9C0` — the port's human-only probe IS faithful) and tests
  `CompareAxisWithShift_10750` → `_106F0` (:3726), which SUMS BOTH
  boxes: `|dx| < a.pitch + b.pitch`. The human carpet's own
  half-extent is `particlesParameters_D951C[44].speed_6 / 2`
  (`AddPlayer_4A920` :33317 → `SetEntityIndexAndRot_49CD0`
  :32841). **PORT BUG.** Row 44 AUTHORS `speed_6 = 0` — retail
  fills it in at BOOT from the TMAPS geometry
  (`speed_6 = width * rotSpeed_8 / height`, the table pass at
  EF:44898-903), giving 242 → half-extent **121** (verified in the
  corpus: the l24 and l30 human entities both carry
  `apitch=aroll=121, afov=ayaw=100` from t=0). `mc2_switch_overlap`
  read the RAW static row (0) and shrank every MC2 switch volume
  by 121 units, so l24's marginal trips never happened: at t=34041
  the human sits 1588 from switch 93, inside 1536+121 but outside
  1536. FIX = `world.rs:7632` `pw = self.g.mc2_params_ext(44).0/2`
  (the derived table the port already builds,
  `mc2::derive_sprite_extents`). Native + conformance; no
  deviation involved. **HARNESS TWIN (needed, else the fix
  regresses other takes):** a one-shot disposition ZEROES the
  records it releases (`sub_4A1E0(id,1)`) and that consumption is
  NOT in the captured `D41A0_0` closure, so it could not be
  re-imported per pair — one mis-timed trip disarmed the
  disposition for the whole rest of the run (l30: the port tripped
  the (11,0) at (201.5,204.5) at t=3234, one phase period early
  under the `--pin-pose n1` sample, and the real t=3242 release
  then spawned NOTHING). Added `World::thing_table_clone` /
  `restore_thing_table` (conformance.rs, opaque `ThingTable`) and
  re-imprint it per pair in `exec_pair_mc2` next to
  `restore_planes` — the same modelling choice already made for
  terrain. **NUMBERS (whole take, unexplained).** l24 family:
  (10,25) **37 missing/0 extra → 7/0**, (10,75) **110/13 → 13/14**,
  (10,22) **10/0 → 2/1** — 109 of the newly-spawned rows now pair
  off as `slot-desync`. l24 totals: missing **1,209 → 1,074**,
  pairs fully explained **11,127 → 11,135**, extra 6,067→6,085,
  field 771,218→773,459 (the port now CREATES 133 entities it
  never did — they land on desynced free-list slots, which is what
  the field/extra ticks buy). l30 missing 126→124, explained
  7,018→7,023; mc2l0 missing 97→80, explained 6,700→6,692; mc2l4
  missing 199 (unchanged). All six fixture suites: **0
  regressions, 0 drifted** (no re-promote needed); mgc-sim 313+
  green, goldens unmoved. Test
  `mc2_switch_box_sums_the_human_carpet_half_extent` (world.rs) is
  two-sided and non-vacuity-proven (pw=0 → fails). RESIDUE (22
  rows): t=13338/13378/13379 (5 (10,25) missing + 9 (10,75) extra)
  = free-list ordering inside one already-firing burst; 1-2 rows
  per burst at t=34041/34324/48007 = the same; t=57223 is a
  DIFFERENT machine — a projectile-impact whirlwind (`sub_678E0`,
  maxLife 24, subSpell 20, `id_0x1A`=7 stamped by the impact tail
  EF:63183) from a non-human caster, i.e. the `mc2-cast-timing-*`
  family, not a switch.

- **MULTIPART FLYER Z-BOB RULED CAPTURE — the "untraced M0/M3
  altitude source" DOES NOT EXIST — 2026-08-03 (session 7, scout;
  read-only, no code changes; roster mc2-flyer-drift-m0/m3 flipped
  open→capture).** Six windowed l4 re-measures post-session-6
  prove the family unchanged (the dig-A servo commits touch
  castle.rs only; the multipart servo `mc2_alt_core` mobs.rs:169
  was already the retail 2-branch shape). Mechanism, decompile-
  pinned: MC2 creature move core `sub_1B8C0` (EF:8741) calls the
  `sub_580E0` servo at :8804 with row args and `MoveEntity` at
  :8805 with **pitch literal 0** — multiparts never fly along
  pitch; m3 row 74 has v_12=0 so the head z ≡ ground_z every tick
  (measured: retail free-descent branch 0% over 6,728 rows —
  BOTH sides clamp); m3 has NO bob state (multipart.rs:548-565
  bare wrappers); m0's bob `sub_1F040` (EF:11233-55) is ported
  byte-equivalent incl. the `ground+256` bounce gate (its 1-4-tick
  z bursts = the bounce firing a tick apart across the terrain
  gap). Family decomposition (t=8000 window, (5,3) 7,077 rows):
  heads med|off| 259 vs segments 14 (rigid `sub_1B6B0` follow —
  all signal in the head); 92-94% of z rows carry a same-tick x/y
  diff with |dz| monotone in |dpos| = wander capture drift
  SAMPLING terrain; the byte-identical-position residue (444
  rows, 423 = the four heads) is the pristine-plane terrain datum
  gap (retail z pinned at 0/2624 for 40+ ticks while the port
  tracks its own heightmap; deltas −23..+8 height bytes; probe
  validated by re-deriving the (5,15) +256 castle-pad raise,
  16,365/17,500 rows). REMAINING from the scout: the l4 terrain
  datum sizing (437 tiles) = recording-format-v2 terrain channel /
  import work, and the untouched t=17954 mass spawn-wave lead. The
  near-universal (3,3) z family (66,845 of 67,391 pairs, the reason
  l24 had zero raw-conforming pairs) clustered position-independent
  → not terrain. ① IMPORT: `mc2_balloon_tick` is the ONE MC2 tick
  that indexes its servo row RELATIVE to `ROW_BASE`
  (`BEHAVIOR[ROW_BASE + row156]`; native spawn sets row156=9 → abs
  68). Retail's ctor `sub_4ABA0` pins `&str_D7BD6[68]` (EF:33422),
  so the generic import produced row156=68 and the tick read
  `BEHAVIOR[127]` — v14=−128 — sinking every imported balloon
  128/tick (= the whole original histogram: floor +128, climb
  +258/+353, descent +112). Fixed conformance-import-scoped,
  (3,3)-only row rebase (conformance.rs:558-579). ② NATIVE LAW: the
  port reused MC1's 3-branch `alt_clamp` (25%·v14 through the band);
  retail MC2 uses `sub_580E0` (EF:40372) — 2-branch, `z>ground →
  z+=v14; floor at ground+v12`, ceiling arg DEAD → open-sky descent
  −4 vs retail −16 = the −12 residual. Fixed both branches
  (castle.rs:910-924), decompile-proven; ZERO goldens moved.
  Numbers: balloon-z rows 163,717→52,517 (−68%), afflicted pairs
  66,845→31,430; windows mid(t20k) −84%, late(t58k) −95%; **l24
  raw-conforming 0→4 (first ever)**; rng untouched. Residual 52.5k
  = balloon DOCKED over the terraformed castle pad (retail floor
  pad≈1536+512=2048; pristine replay descends one servo step) —
  capture, roster `mc2-balloon-z` flipped open→capture with the fix
  provenance. Non-vacuous test
  `mc2_balloon_servo_descends_full_v14_in_band`. EF: sub_4ABA0
  :33422, sub_580E0 :40372, AddBallon_60AB0 :61857-61, sub_60D50
  :61933-35. **PLAYTEST OWED (native hover rate now retail).**
- **MC2 (10,79) "ENDGAME MOVER" = the CASTLE DEFENDER PIECE, eleven
  import mis-homes — LANDED 2026-08-03 (session 6, dig B).** Ctor
  `sub_508E0` (EF:36987), tick `sub_3AF00` (EF:30106): max_life
  100000, action 0x56, sprite 66; 4 pieces per castle upgrade in a
  2×2 grid, three cohorts on l24 (t=15288..69273, ~8-12 live). The
  port already had the FULL machine (mc2/castle.rs) — the ~730k-row
  family was pure import: the piece is minted with a fresh layout
  and the uniform alias table mis-read ELEVEN homes. Killer: recoil
  `f68 ← @0x43` (part-type, nonzero) instead of @0x44 → every
  imported piece re-applied a 115-unit launch displacement per pair
  (slot 619 t=30000: y retail 173.0 vs port 173.449 = 115/256
  exact) = the entire y family. Fixed homes: f44←@0x10, f30←@0x2C,
  f69←@0x3D, f68←@0x44, f54←@0x36, f28←@0x96, f34←@0x1C, f36←@0x1E,
  f26←@0x4A, f67←@0x43; + obs override: (10,79) heading projects
  from f34 (f30 now holds the fire-mode selector). All
  conformance-import scoped, no native change, no golden. Full-take:
  y 335,570→534, heading 593→22, x 1,263→541, **total 732,243→
  368,779 (−49.6%)**, zero collateral. Residual z ~367k = terrain
  closure (pieces on terraformed mounds; want/got bob in lockstep,
  constant per-slot ~16) → rule `mc2l24-castle-piece-terrain-z`.
  Test `mc2_castle_piece_import_field_homes`.
- **MC2 (10,57) FROZEN FALLING SPHERE + VOLCANO-LANE z + the
  l30-terrain RULING — LANDED 2026-08-03 (session 6, dig C).**
  ① The fixture's "z+16 constant" was a pair-0 artifact; real diff =
  −f2c(retail@t): the port FROZE a falling sphere. (10,57) = the
  random-value mana sphere (`sub_50130` EF:36631, action 0x3E=62);
  its tick `sub_35FB0` (EF:26318, settle EF:26526-46, bounce
  EF:26567-77) is byte-identical to the (10,39) ball law ball_tick
  already serves. Importer was ALREADY right — the class-10
  effect-tick whitelist just never listed action 62, so imported
  spheres fell to the terrain catch-all. Fix: `| 62` in the effect
  gate (world.rs:2270) + `62 => ball_tick` (mc1/combat.rs:2999);
  native-inert (native spawns m57 as model-39/action-41). t=0..500:
  (10,57) z 1438→22, all rows 5233→72 (−98.6%); whole-window
  unexplained field 10,175→1,753. ② (10,16) boulder vz home:
  `sub_32600` EF:23765 reads vz from @0x2C; uniform import homed
  f44←@0x2A (=200 always) → +200 relaunch per pair; scoped
  f44←f2c block (conformance.rs:1373). ③ (10,19) column z-snap
  strict-gated frozen-z (tail.rs:1578), mirroring summit18.
  ④ **RULING — the §l30-terrain OWED check is ANSWERED: the summit
  plateau is RUNTIME-terraformed, pure capture.** `mc2_dome_tick`/
  `sub_31940` (EF:23193) writes the heightmap directly
  (`mc2_dome_cap` EF:23300-18); decisive: at t=0 exactly one dome
  exists mid-grow at a DIFFERENT site while the summit already
  reads 2624 — an earlier finalized dome the recording's
  entity channel cannot carry. Nothing further portable; rules
  `mc2-summit-fire6-z-capture` + boulder/column re-eruption landed.
  BANKED: extending frozen-z-under-strict to the shared
  `standing_fire_tick` would recover summit-slope (10,6) fires but
  touches MC1 terrain goldens — own dig. Tests ×2, non-vacuous.
- **MC2 (5,19) FIREBUG LUNGE + CLASS-15 DETACHED-JAR ARC — LANDED
  2026-08-03 (session 6, dig D).** ① (5,19) = the FIREBUG; retail
  oscillates actSpeed 76↔8 with the `byte_0x46` sub-state and rolls
  the entity LCG only on the fast leg. `HitFirebug_25610` case 1
  (EF:16386-16407) sets b46=2 and RETURNS; case 2's drop to
  maxSpeed + its own roll (EF:16409-16416) run the NEXT tick. The
  port's `continue` fell through into case 2 SAME-tick → speed
  dropped a tick early AND the LCG double-advanced. One-word fix
  `continue`→`break` (mc2/roster.rs:2119-28); native change, NO
  golden moved. t=1960+1500: speed 109→8, rand 129→28; fixture
  sigs dropped 5,19:rand+speed in TWO corpora (mc2l24 t=2157 AND
  mc2l4 t=76). Residual heading/x/y = ruled wander-turn capture.
  ② Class-15 detach: the lead's premise CORRECTED — slot 73's
  20k-tick idle is FAITHFUL (0 rows, not torn); the family is a
  ~15-tick FLING at t=15080-95: the m26-wraith spell-steal jar is
  a moving projectile (z 251→344→0, action 78/pitch 5) the port
  dropped on frame 1. Retail arc `sub_59DC0` (EF:41198-41243) runs
  off homes the class-15 import never mapped: arc counter
  dword@0x10 (`sub_69300` EF:55807 zeroes it at the steal) + wraith
  slot word@0x26. Fix: `action45 == 78` arm in the class-15 import
  (conformance.rs:1359) + 1-line native pitch-copy each rising tick
  (EF:41216-18; world.rs:6911). t=14990+130: 64→12 rows,
  unexplained 64→0 (residual = pre-existing pose-phase). Tests ×2,
  non-vacuous, no re-pin.
- **MC2 `owner` OBS-SCHEMA GAP + the (5,0) identity CORRECTION —
  LANDED 2026-08-03 (session 6, dig E).** Per-class @0x28 truth
  table (EF cites): (10,42) build painter @0x28 = parent CASTLE
  (repaint `sub_5FBD0` EF:61192-93; level-up `sub_60480`
  EF:61596-97); (5,{0,19,21,25}) pyramid-SUMMONED creatures @0x28 =
  the PYRAMID (summon EF:13420/13413); (5,10) = ring-spin angle
  (f36 arm intact); class-15 = wizard (unchanged). ⚠ PREMISE
  CORRECTIONS: the session-5 "(5,0) = hydra segments" handoff was
  WRONG — on l24 the hydra is (5,27); the (5,0) owner=7 family is
  the pyramid's summoned WORMS in the apocalypse window t≈52-68k.
  And the pyramid was POISONING its own children: its repurposed
  @0x28 (spin angle) was fused into id24, which the summon copies —
  excluding (5,10) from the fusion makes pyramid id24 = @0x1A = its
  own index, the identity that makes retail parentId ≡ port own_id.
  ⚠ TRAP (the transient mid-session `field:5,0:owner` atom): model
  0 is ALSO the generic worm body whose id24 is its body slot — a
  naive `id24 != slot` guard over-projected 261,555 wild rows; the
  final discriminator is "referenced entity IS a live (5,10)
  pyramid". Numbers: whole-file owner mismatches **47,083→143
  (−99.7%)**; painter window t=10060-70 →0; apocalypse t=52000-500
  →0 with 2,509 summoned-creature rows present; non-owner rows
  byte-identical before/after (no ripple); exactly ONE sig atom
  changed anywhere (mc2l24 t=10062 lost 10,42:owner). Residue: 29
  class-15 want=116 rows (one per spellbook model — adjacent lead).
  Tests ×2 (gate non-vacuity proven).
- **SLOT-DESYNC CLASSIFIER + village-regrade RETIREMENT + WAVE
  RE-CENSUS — LANDED 2026-08-03 (session 6, dig F).** The
  session-4 free-list ruling + open-leads 0b are now a COMPUTED
  roster rule (literal id `slot-desync`, pose-phase mechanism;
  `--no-slot-desync` opt-out): within one pair, still-unexplained
  missing/extra of the same (class,model) pair up by nearest x/y,
  tagging only min(missing,extra) per side — one-sided residue
  stays open. Ordering is LOAD-BEARING: runs after the roster,
  BEFORE pose-phase (at a wave the port extras are pose-phase but
  the retail missing are not; pose-first orphans the balanced
  family). Field rows untouched — proven byte-identical OFF/ON on
  all four takes. Fires on l24 at 236/67,391 pairs (0.35%),
  exactly the two scripted waves + the apocalypse epoch. Impact
  (missing unexplained): l24 1,688→1,209, l30 188→126, l4 249→199,
  mc1l0 98→83. `mc1l0-village-regrade` RETIRED (0 hits post
  re-record; region absorbed by mc1l0-terrain-z). **RE-CENSUS
  VERDICTS — both REAL unported-spawn leads:** (10,25) 37 missing
  / 0 extra, 100% one-sided — a short-lived doomsday-pyramid
  effect (action 25, life 7/8) the port never spawns; (10,75) 128/31
  → post-absorption **110 missing / 13 extra** — the doomsday
  TAIL-DRAG segment chain (tail.rs:448 model65=75) under-produced.
  Small one-sided residues: (5,3)+3, (5,26)+2, (14,1)+2 missing.
  ⚠ SESSION-7 CORRECTION: the census counts stand but the
  ATTRIBUTION was wrong — neither family is doomsday. Both come
  from the (11,2) storm-switch disposition (whirlwind heads +
  their 11 funnel nodes + `sub_4F6A0` area blasts); see the
  session-7 switch-box entry above.
- **SESSION-6 CLOSE-OUT (2026-08-03).** Post-six-digs full l24:
  unexplained field 1.58M→**771,218** (−51%), missing 1,721→1,209,
  extra 6,432→6,067; pairs fully explained 4,517→**11,127**;
  conforming 0→**4**; rng 12 singles unchanged. All six suite
  manifests promoted green, 0 regressions (l24: 1 fixed + 12
  drifted-improved; fixes propagated to l4 balloon atoms + l30
  firebug atoms). Workspace sweep 42 bins green --no-fail-fast,
  fmt clean. Roster 48 rules. **Hydra bolt-cadence RE-MEASURE
  (the dig-D-session-5 NEXT-LEAD precondition): t=10650-11150
  reads 1,529/94 — BYTE-IDENTICAL to the mid-fix session-5
  measurement ⇒ the barrage under-fire is independent of the
  import homes; the portable head-state slice (multipart.rs:1732)
  is a LIVE dig.**

- **MC2 HYDRA (5,27) — FOUR IMPORT FIELD-HOME BUGS froze the whole
  machine — LANDED 2026-08-02 (session 5 mc2l24 intake, dig A).**
  The m27 hydra branch machine homes four struct words where the
  uniform MC2 importer spent other lanes. remc2 `sub_2AD40`
  (EF:20770-800) writes `fov_0x22_34`; the branch integrator
  `sub_2A340` (EF:20233) switches on `word_0x2C_44` and reads
  `dword_0x10_16`; the branch index / body live-branch gauge is
  `byte_0x3B_59`. `import_ent_mc2` had the uniform homes: `f36:0`
  (dropped @0x22), `f44:@0x2A`, `f50:@0x30`, `f26:@0x2E`. Corpus
  proof (dump-state slot 16, t=0→1): `m27_integrate` mode 0 =
  roll+73 / fov+62 / speed+16 verbatim (2461→2534, 1433→1495,
  160→176). f36←@0x22 = per-branch spline PITCH
  (1433/2595/1709/1905/1985 — imported 0 collapsed all 5 branch
  heads to one z=2951); f44←@0x2C = integrate MODE (@0x2A=100 hit
  the no-op arm → roll/fov/speed frozen; branch 46 mode-1 −64+−16
  =−80 = the "|speed|=port+16" symptom); f50←@0x3B = branch index
  0..4 + body gauge 5 (@0x30=0 collapsed every branch onto
  `D404C[0]`); f26←@0x10 = whip counter (steps 1→2→3→4 in lockstep
  with crack speeds −192/−130/−23/192 @ t=180 slot 46; the m0
  `(5,0)` arm extended to `(5,0|27)`). Fix conformance-only in
  `import_ent_mc2`, (5,27)-scoped; native spawn untouched.
  Numbers ((5,27) rows): t=0..2000 **34,923→1,180 (−96.6%)**
  (speed 7357→0); t=40000..41500 **67,269→19,012 (−71.7%)**
  (speed 13356→3); non-(5,27) rows +0.06% (noise). RULED not-bugs:
  death-window z residual 11,110 = terrain-crater non-closure
  (bodies are ground-walkers, `m27_move` z=`ground_z`; x/y match,
  z differs ~370); t=40565 missing 78/extra 58 = free-stack
  slot-order desync on a mass spawn/death tick; window-1 residual
  ~1,180 = body-brain wander phase drift (shared move-core).
  NEXT: m0/m3/m22 siblings likely need the same f44/f26 homes if
  their families ever surface — extend the arms, don't re-derive.
- **MC2 DOOMSDAY PYRAMID — RNG LEAK + owner FIELD-MAP — LANDED
  2026-08-02 (session 5, dig B).** The `got[t]==want[t+4]` rng
  window t=51751-70 SOLVED, and the "blind-landed perturb arm
  draws global" hypothesis REFUTED: retail's (10,14) ring-rock
  ctor DOES draw the global LCG; the window was the port failing
  to SUPPRESS the ring. The importer restored the pyramid's `f26`
  from @0x2E (charm lane ≈0) instead of `dword_0x10_16`/@0x10, so
  the 0..1200 doom-meter reset to 0 every pair, re-ramped to only
  30, and never crossed the 600 gate (`sub_21490` EF:13031) that
  stops the `for k in 0..4` (10,14) spawn ring (EF:13070-90) — 4
  spurious global draws/tick. Fix: `f26` match `+ (5,10) =>
  r.scratch10`. rng 51500-52100: 21→1 (window 20→0); whole-file
  32→~12; death window 0/100. ALSO: the pyramid repurposes
  `parentId_0x28` as its ring-spin angle (+96 & 0x7FF per
  un-suppressed tick, EF:13072) — imported f36=0 mis-angled the
  ring and pinned the `owner` obs at 0 (11,721 rows); fix =
  import `f36 (5,10)←owner28`, project `owner (5,10)←f36`
  (owner diffs 51500-52100: 874→334, all in-window pyramid rows
  gone). `sub_21030` case 0xF verified ALREADY PORTED
  (doomsday.rs:432-453, session 4) and faithful to EF:12857-80 —
  retail reaches state 0xF at t≈63289 and the apocalypse window
  grades clean. Pyramid heading (6,969 rows) = pose-phase noise.
- **MC2 PLAYER DEATH/RESPAWN — RULED FAITHFUL (first player-death
  corpus) + class-15 heading gate — 2026-08-02 (session 5, dig
  C).** mc2l24 holds 14 human deaths (respawns t=2609/4462/6093/
  8977/11243/34931/39200/39895/41232/43087/46127/54046/60451/
  61490). Every respawn re-inits in ONE tick (trace slot 116
  t=2608→2609): life→maxLife via `CopyMaxLifeToLife_49A20`
  (template `AddPlayer_4A920` EF:33317-38), mana→full refill,
  z→respawn pad, scratch d88→1000, action 3→0, flags clear
  0x1020, class-15 spellbook (slots 161-191) re-granted — and
  both sides AGREE at t+1 at every death. Residual death-window
  rows (1-tick mana/spellbook/life/hand blips + 7 slot-79 swaps
  at t=2608 = transient slot-alloc of the 22 re-granted book
  records) are the input-delay-2 boundary, NOT a port bug; no
  native change needed. FIXED: 25,334 class-15 `heading`
  false-divergences → 0 — the port repurposes the class-15
  world-yaw lane @0x1C for the subSpellIndex payload and projects
  heading 0 (conformance.rs:890-94); the "@0x1C dead on
  manifestations" premise is REFUTED (a detached spell jar, model
  0 action 78, slot 73, holds its fling yaw ~1634 for 20k ticks);
  facing is cosmetic (cast reads f30/f34) → skip class-15 heading
  in `compare_mc2_gated` (verify_mc2.rs), twin of the human
  applied_yaw skip. RULED capture: player.mana 24,465 +
  player.life 4,667 = regen-cadence drift (the stored
  `lifeRegen_0x163_355`/+132 deltas live in the un-recorded
  wizext; life onset t=299: retail holds post-damage ~16 ticks
  then +5/tick, port regens one quantum early, heals at cap);
  mana_max 5,053 + player_ent_idx 1,462 = class-10 effects/
  slot-desync lanes, not vitals. OPEN: class-15 detach state
  machine (slot 73 pitch 5→0, action 78→1 — real unmodeled state
  diff, still compared).
- **MC2 FOUNTAIN + TEMP MANA + BALLOON-REFUSAL — 2026-08-02
  (session 5, dig F).** ① BALLOON-REFUSAL LAW PORTED (the
  player-observed law): retail's balloon sphere-acquisition scan
  `sub_5F810` (EF:60994-61023) skips any (10,39) carrying the
  decay channel `byte[1]&0x20` (port flag 0x2000) — fountain/
  mana-rain spheres are 140-tick TTL and carry it, so retail
  balloons never take off for temporary mana. Port scan omitted
  the gate; fixed in `mc2_castle_roster` (castle.rs:737,
  `|| e.flags & 0x2000 != 0`), MC2-only by construction, pinned
  by non-vacuous test `mc2_balloon_refuses_a_decaying_fountain_
  sphere`. Faithful port, NOT a deviation. ② NATIVE FOUNTAIN ARC
  fixed: `mc2_summit91_tick` discarded the apex
  (`word_0x2C_44=(rand&0x7F)+128`, EF:24052) → balls sprayed
  flat; now `e.f46 = apex` (morph.rs:563); conformance-neutral.
  ③ TEMP-BALL TTL LAW PROVEN already byte-exact: source
  `AddManaRain` sub_32CF0 (EF:24007, 3 spheres/tick, 5-draw
  arming [speed/apex/color r%9−1/mana/yaw], maxLife=140,
  byte[1]|0x20, z=ground+96); mover `TransformArcherToMana`
  (EF:26173-307: z+=v, v−=16 clamp≥−128, bounce −v/4 zero≤16,
  roll+friction 250/256, decay tail fade@12/ghost@6/expire@0);
  corpus slots 133/168 match to the unit. ④ (10,39) FOUNTAIN BULK
  (2.5M rows) RULED terrain-closure capture + slot desync: x
  diffs 99.8% ≤1 tile, z 88% ≤16 — balls rest on the
  doomsday-terraformed mound the pristine replay lacks; early/mid
  rows = terrain-roll (worm-death mana + wake-law downhill).
  Ball laws byte-exact throughout. NOTE: the retail minimap draws
  ALL fountain balls ORANGE (player-observed retail one-off hack)
  — port colors by logic; standing map-presentation deviation
  ruling applies, never "fix" toward retail. HANDOFF: (5,0) on
  this take = HYDRA SEGMENTS (not balloons); their owner rows
  (16,333, constant want=7 got=0) = obs-schema gap (@0x28
  projected only for class-15 and (5,10)) — open lead.
- **MC2L24 LIGHTNING (9,9) 46k-MISSING — RULED CAPTURE 2026-08-02
  (session 5, dig D; mechanism fully proven, no code landed).**
  The take's #1 missing family — (9,9) 46,241 missing / 1,967
  extra (42% of all missing), max_life 64,987 rows (63,865 =
  want −1 got 0). Caster identified: the HYDRA (dump-state 10703
  id=15, (5,27), 1e6 life) — the trail is its (9,9) heavy bolt
  (`mc2_spawn_bolt9` = `sub_4D860`/`sub_1D260`, EF:9883/34942,
  impact (10,23) id=15); the seven (9,0) id=116 are the PLAYER's
  bolts, a separate no-trail family — don't conflate. Born-dead
  law CONFIRMED byte-correct (EF:58341 ≡ proj.rs:883; ahead node
  lives 2 recorded frames, behind 1); "reaped a tick early"
  REFUTED (import census t=10703→05: nodes 81→162→162, disabled
  0→81→81 = retail exactly); node-cap REFUTED (clamp 96/beam,
  beams ~80; retail's 326-node frames are multi-volley stacking).
  Residual = two capture mechanisms: ① UNDER-FIRING — port lays
  18 trails vs retail ~38 birth-ticks per 500; the hydra's
  multi-head barrage cadence comes up ~half (the attack GATE is
  faithful: `sub_27E00` EF:18297 ≡ roster.rs:2877-87; divergence
  is upstream head-state/rand); ② maxLife −1/0 — both engines
  pop a LIFO free stack; a sustained barrage drifts the beam
  slot upward (measured 160→183→254 across t=10703/04/05) so
  retail's steady nodes fall below the beam; per-pair replay
  cannot reproduce multi-frame free-stack drift. Both = the
  cast-timing-skew + free-list-reuse class already ruled
  capture; l24 amplifies the mc2l4 residual ~30×. (10,23) 826/5
  shares the root. Windows: 10650-11150 = 1529/94; 43100-43600
  = 1686/0. NEXT-LEAD: trace the hydra multipart bolt path
  (multipart.rs:1732) at t=10704/05 for any PORTABLE
  head-state bug under the capture noise — note dig A's
  field-home fixes landed mid-measurement; the post-fix cadence
  may already differ, re-measure before digging.
  ⚠ SESSION-7 CORRECTION: mechanism ① (UNDER-FIRING) was NOT
  capture — the NEXT-LEAD paid out. The whip is a five-shot
  burst and the port dropped four of the five because
  `manaRegen_0x88_136` (the bolt power) was never imported; see
  the session-7 Resolved entry "MC2L24 HYDRA HEAVY-BOLT BARRAGE
  UNDER-FIRE". Mechanism ② (maxLife −1/0 free-stack drift) and
  the beam GEOMETRY stand as capture. Windows now 89/175 and
  166/0; full take (9,9) 46,241/1,967 → 13,632/3,052.
- **MC2 SUMMIT RE-ERUPTION TRIO — LANDED 2026-08-02 (session 4,
  opus dig; complements the fire-spray ring loop).** Retail law
  (EF cites): the (10,18) summit vortex controller (`sub_32A70`
  EF:23906) is a PERSISTENT invisible singleton latched by the
  GLOBAL `D41A0.word_0x31` (the (10,19) column latches
  `word_0x33`); tick-0 eruption spawns column + one (9,0) bolt
  (impact (10,17)) + one (10,16) boulder, controller yaw +=1280
  UNMASKED (only the bolt copy gets the &7FF mask, EF:23976-87);
  pulse rolls (`dword<128 && dword&0xF && rand%5==0`) each spawn
  a (10,16); despawn ONLY on ground-move (`z != getTerrainAlt`)
  or dword>=127, releasing the latch. RE-ERUPTION CADENCE
  (EF:23921-35): at dword>2500, a 1-in-100 per-tick roll resets
  dword=0 — ONLY while `word_0x31==0` (latch free). mc2l30
  corroborated tick-exact: site-118's controller (slot 134)
  erupts t=274, site-114's (slot 195) steals the latch t=279 and
  despawns t=281 (its still-growing dome moved the ground under
  it), slot 134 idles to dword=2507 and re-erupts EXACTLY at
  t=2536 (roll r1=28800 %100==0), then holds the latch forever —
  no further re-eruptions (recorded word_0x31 stream matches:
  0@2535, 134@2537+). THREE port bugs fixed: ① PHANTOM
  GROUND-MOVE DESPAWN — the port compared the imported plateau z
  (3296) against pristine ground_z (1232) and killed the
  controller; strict-gated FROZEN-Z law (no re-snap/despawn
  under replay; native keeps the exact check) + regression test
  `mc2_summit_vortex_frozen_z_under_strict`; ② controller yaw
  wrongly &0x7FF-masked (heading 512-vs-2560); ③ the eruption
  LATCH was not imported → over-eruption (~13 phantom eruptions
  5037-8330 once frozen-z landed) — `word_0x31`/`word_0x33` ARE
  in the recorded D41A0 header: decoded as `RetailMc2.vortex`/
  `fire_col` (mgcr.rs, additive) and imported into erupting/
  plume (conformance.rs). Both halves load-bearing (frozen-z
  without the latch import regresses to 37 rng / 1,152 extra).
  Net on current tree: l30 rng 20→19, missing −28 (the recovered
  t=2536 re-eruption records), extras flat, mc2l0/l4 provably
  inert (no (10,18) there). Goldens unmoved (dome-trap
  mc2_slice passes); check-decode clean ×3. Residual 19 rng: 1 =
  t=274 dome-import eruption-timing (open, dome life decrement
  on import), 18 = the slot-desync fire cascade (ruled, fire
  entry).

- **MC2 FIRE-SPRAY RING LOOP — LANDED 2026-08-01/02 (session 4,
  opus dig; closes the l30/l4 RNG residual).** Retail's (10,19)
  ground-fire-spray column tick `sub_32F40` (EF:24095) wraps its
  (10,14) smoke emission in a walk of the RING-0 SPLAT TEMPLATE
  (`while (sub_10130(AddE7EE0x_10080(0,0)) == 1)`, EF:24112-40)
  — ring 0 has 4 cells (baked search.bin value-0 count), last
  dropped as the stop code ⇒ THREE emission cells per tick, each
  with the ~50% gate roll (`2*((r%0x9D)/79)-1 > 0`), 2 jitter
  draws offset `192*(dx,dy)`, and the odd-life 4-puff (10,14)
  ring. The port's `mc2_fire_spray_tick` (mc2/tail.rs) emitted
  ONCE (no ring loop) → ~1/3 the smoke AND under-drew the GLOBAL
  stream (each smoke ctor draws lcg32 — retail-matching), which
  WAS the l30 rng residual. Fix: ring_cells(0,0) loop, native+
  strict (unconditional retail law). Numbers: mc2l30 rng
  **202→19** pairs, (10,14) missing 990→390; mc2l4 rng
  **163→13**, (10,14) missing 873→326; mc2l0 untouched (479
  conforming — no volcano, fix inert); extras FLAT everywhere
  (no over-production); no golden moved. Same dig VERIFIED
  BYTE-FAITHFUL (no fix): (10,0) fire tick sub_30D50 (fire does
  NOT spread — one damage pulse + burn + flicker + z + anim),
  (10,6) sub_31760 incl. 1/7 smoke-on-shrink, (10,17) meteor
  ring-seeding sub_32880, (10,1) big-explosion sub_30F60,
  meteor-shot spark sub_66180, (10,16) boulder→(10,6) sub_32600,
  and the ring template mapping. Port re-eruptions CONFIRMED
  PRESENT and matched at t>2400 (the "never re-erupts" reading
  of the missing rows was wrong). RULED on the residual: the (10,0)
  missing/extra bulk (l30 1,659, l4 1,975) = FREE-LIST
  SLOT-ORDER DESYNC, not law — proven at l4 t=9082: missing and
  extra fires have IDENTICAL x/y, differing only in slot (and
  hence flicker/z, since rand_0x14 seeds from slot+global_rand)
  — single-snapshot import can't recover retail's within-tick
  free-then-reuse LIFO order; matcher pairs by slot. NEW LEAD
  banked: (10,12) missing 313 (l30) / 779 (l4) = possession
  WEAK-PULSE family (cast lane, not fire).

- **MC1 CORPSE-FLAME SPREADER CADENCE — LANDED 2026-08-01
  (session 4, opus dig; the "mc1l0 (10,0) fires 57/210" family).**
  The (10,0) ground-fire family is rung out by the (10,1)
  fire-spreader (`sub_25130`, sub_main:28161-70): per ring cell
  ONE draw is the skip test — spawn iff `v5 % 157 >= 79` (~50%) —
  and the x/y jitter PAIR is drawn only on the SPAWN branch (a
  skipped cell costs a single draw). Spawned fire inherits id24
  (:28175), f30 (:28176), `flags |= 0x80 | (spreader & 0x10000)`
  (:28177-79). Port bugs (mc1/combat.rs `spreader_tick`): skip
  test was `rand & 1`, and both jitter draws ran on EVERY cell —
  the 3-draws-vs-1 skew desynced the spreader's per-entity stream
  so the whole ring's fire SET diverged (the free-stack census
  passes on every pair, so this was a genuine tick-law bug, not
  drift). Fixed + f30 inherit. mc1l0 (10,0) 57/211 → 32/166 (the
  t=564-583 worm-death burst ~130 rows → 1); mc1hwl0 48→49
  conforming, (10,0) 2754/1571 → 2455/1222, (10,1) 385/691 →
  261/586. Residual = within-tick slot substitution (fires
  faithful in pose+tick but landing in different slots at
  free/reuse boundaries) = capture, matching the MC2 ruling.
  L005 GOLDEN+OBSERVABLE re-pinned D-E ONLY (post-init/A/B/C
  hold byte-for-byte — behavior change localized to the combat/
  aftermath stages, by design). ⚠ SHARED TICK: `spreader_tick`
  dispatches for BOTH games (engine/world.rs effect_tick) — the
  MC1 law also collapsed MC2 corpse flames: mc2l0 fixtures t=58
  and t=77 flipped open→conforming (promoted same session). If
  MC2's retail spreader ever proves a different skip law, split
  per-game then — empirically the MC1 law fits MC2.

- **MC2 (9,17) POSSESSION RE-FIRE — LANDED 2026-08-01 (session
  4, opus dig; the biggest EXTRA family, mis-swept under
  `mc2-cast-timing-extra`).** The port's `mc2_cast_gate`
  re-pressed an already-armed possess manifestation into a FULL
  new (9,17) delivery bolt + mana debit every press, all tiers.
  Retail law: the armed-possess press only sets `byte_0x3C_60`
  (sub_5F660 case 1, EF:60902) and the consumer `sub_68DE0`
  (EF:55987-56013) is TIER-gated — tier 0 just CLEARS the signal
  (no bolt, no debit); only tiers 1/2 spawn (a different class-9
  subtype-1 via sub_69900, 3-tick cadence, untraced/unexercised).
  Corpus proof: MISSING (9,17)=0 vs EXTRA=452 — symmetric timing
  skew would balance; retail emits exactly ONE bolt per arm.
  The old cast.rs "//player retail-verified, all tiers" comment
  was a misreading (likely a higher-tier Mana-Magnet
  observation). Port: cast.rs re-press gated on `f71 > 0`;
  test renamed → `mc2_possession_tier0_does_not_refire_...`.
  Numbers: mc2l0 466→479 conforming ((9,17) extras 445→312);
  mc2l30 extras 452→355, rng 202 UNCHANGED (the residual rides
  the volcano windows, not casts); mc2l4 (9,17) 1393→1208. The
  remaining fresh-arm extras are genuine input-reconstruction
  skew (the recorded held-register toggles per frame; retail
  arms ~1 tick before the recorded button) — correctly
  roster-swept, NOT a sim bug; don't chase them into the input
  decode, changing it regresses the other takes.

- **PER-ENTITY `rand_0x14 += setting_30` PERTURB — LANDED
  2026-08-01 (session 4), corpus-invisible by census.** Retail
  has exactly THREE per-entity perturb sites (whole-tree grep):
  the pyramid pick rolls (sub_21850, EF:13140/13220) and the m27
  branch bolt (sub_2A7F0, EF:20521); pattern = LCG → modulo draw
  → `rand_0x14 += setting_30` (next roll starts shifted). The
  counter: `setting_30` increments beside `Turn++` in
  `PlayerEvents_51BB0` (EF:37557) and zeroes at level init
  (EF:31290/38455/39339/43327) → during the entity pass it
  EQUALS the post-increment turn — the same value the cave
  carpet tail's corpus solve anchored (EF:59803 is the one
  GLOBAL-stream perturb site, already ported). remc2's
  `uint8_t setting_30` typing and Level.cpp:340's "0x3D after
  load" are both remc2 artifacts (the latter is their own debug
  reseed `//fix`), not retail law. Port: `Gen::mc2_rand_perturb`
  (mc2/mobs.rs) + `MobCtx::mc2_turn` (the sanctioned no-Gen-field
  channel, same rationale as `strict`); applied at the three
  sites. BONUS FIX found in the same read: the port short-
  circuited pyramid roll 1 under the bit7 escalation — retail
  draws UNCONDITIONALLY (EF:13137-39) and only overrides the
  ROLL to 0 (:13141-45); the draw+perturb now always land.
  Zero corpus effect (no (5,10)/(5,27) on any graded take — this
  is why the old "top lever" claim was wrong); prepares future
  doomsday/hydra takes. Suites green, goldens unmoved.

- **MC2 SAME-TICK REAP — LANDED 2026-08-01 (session 3; the
  player-chosen top lever, opus dig corroborated).** Retail MC2's
  death path only SETS the disable bit (`DisableEntityDrawing04`
  EF:40332-35 = `byte[1] |= 4`, nothing else); the reap is
  `sub_57F20` (Events.cpp:5209-39: tile-unlink → recycle-list
  scrub if byte[2]&2 → class-zero → free-stack push, atomic) and
  the per-tick site is the TOP of `UpdateEntities_57730`
  (EF:39948-56): after the single global LCG draw (EF:39947), one
  unconditional ascending pass frees every record already
  disabled at tick entry, BEFORE bucketing and dispatch. (The
  ledger's old "Events.cpp:548" cite is `ApplyEvents_498A0` —
  LOAD-TIME only, sole caller GenerateEvents; it shows the same
  disable→sub_57F20 idiom but is not the per-tick mechanism.) So
  a record disabled during tick T's dispatch survives EXACTLY ONE
  end-of-tick snapshot and is gone before T+1's dispatch — the
  measured ghost law and the MC1 tick-top reap are the SAME LAW
  seen from the two ends. Slot reuse: earliest at T+1 dispatch
  (NewEvent pops free-FIRST, LIFO; reap pushes ascending →
  reused slots pop highest-first). Disabled-but-unreaped records
  draw NO rand (already dispatched their death tick, skipped
  after). PORT (strict-scoped; native MC2 keeps its in-loop free
  pending the native sweep-law port — DEVIATIONS.md updated): ①
  world.rs `tick()` tick-top reap gate extended to
  `Mc2 && strict_retail` (runs after the tick-top draw, before
  bucket counts — ghosts stop inflating class buckets); ② the
  importer's ghost free-stack pre-push DELETED
  (conformance.rs — the reap owns the push now; keeping both
  would double-push; `ghost_slots` stays for the census, ghosts
  still import class≠0/unlinked = retail's end-of-T state).
  NUMBERS (A/B on identical build): mc2l0 conforming 240→452,
  gross extras 3,761→1,389, unexplained extras 198→22, rng
  mismatch 3→2 pairs; mc2l30 (10,0) extras 5,590→346, (10,14)
  917→36, roster-explained 6,320→6,658; mc2l4 explained
  13,330→13,698. The l30 202 rng pairs survived UNCHANGED ⇒ the
  rng residual is entirely §l30-churn (b) per-entity rand sites.
  Two-wrongs exposure measured and accepted: mc2l0 unexplained
  field +232 rows ((10,39)/(10,1) slot-occupancy collateral,
  model co-diverges — different entity in the slot) and gross
  missing 431→1,095 (mostly re-labeling: slots that used to hold
  a port ghost aliasing retail's record now sit empty = cleaner
  missing atoms). Fixture t=737 re-statused conforming→capture
  (`mc2-fire-churn-m13`: a newborn (10,13) churn spawn into
  recorded-free slot 464 — pre-reap it "conformed" only because
  the ghost extra masked the family). 7 mc2l0 fixtures FIXED
  (several were mis-bucketed capture — they rode the reap), all
  suites promoted green, sim tests green, native goldens
  UNMOVED (strict-scoped).

- **MC2 +0x54 `applied_pitch` STATICS FAMILY — SPLIT 2026-08-01
  (opus dig): one real ASSET-BAKE lead + two downstream slices;
  no f80-law bug anywhere.** +0x54 = `array_0x52_82.pitch` (port
  f80) and means different things per family:
  ① **(10,45) dwellings, 41 rows, retail 194 → port 184 —
  RESOLVED 2026-08-01 (opus dig + fix landed): NOT a bake trim.
  The night/cave art is GENUINELY 36 wide** (RNC-decompressed
  payload headers read straight from the retail files: TMAPS0-0
  day = (38,39), TMAPS1-0 night/night-fog = (36,39), TMAPS2-0
  cave = (36,39); 240 of 504 entries differ across banks, in
  both directions — no uniform trim exists, and the port's
  decode/bake reads the header dims verbatim). The REAL law:
  retail derives `particlesParameters` ONCE AT BOOT from the DAY
  bank — `sub_71410_process_tmaps`' sole caller is Initialize
  (EF:42885), the boot-active tmaps file is TMAPS0-0
  (TextureMaps.cpp:595), and the per-level TAB swap
  (ReadAndDecompress.cpp:55/110/137) never recomputes the table —
  so night/cave levels run day-art extents session-wide. The port
  re-derived per level from the level's own bundle. FIX:
  `Bundle::mc2_extent_dims` (mgc-formats bundle.rs) day-sources
  the dims for ALL MC2 extents derivations (app loader,
  conformance runner, every test/example world recipe — 11
  sites); rendering stays on the level's variant bank; no rebake,
  BAKE_EPOCH unchanged. 52 SPRITE_PARAMS rows shift on
  night/cave/fog levels; mc2_cave + mc2_slice STATE goldens
  re-pinned as behavior (OBSERVABLE goldens HELD). mc2l0 452→466
  conforming, the 41-row family gone, fixtures t=223/t=291
  promoted.
  ② **(10,39) spheres, 123 rows = DOWNSTREAM of the open sphere
  mana economy; rotation law CONFIRMED byte-exact** (thresholds
  BALL_SIZES + ROT quads match retail's own values 140→13,
  300→28, 1028→56, 2250→70; re-sprite gate EF:26742 ==
  combat.rs:2904). 28 rows are slot-occupancy mismatches, 92
  co-diverge on f140 mana (death-burst fractionation: port slots
  carry 280=2·140, 420=3·140 where retail dropped 140-mana
  balls). No sphere fix here — rides the open l4/l30 sphere
  economy + AI-lane work. Size-threshold-off-by-one REJECTED.
  ③ **(10,1)/(10,42), ~38 rows = pure slot-occupancy collateral**
  (model co-diverges on every row — different entities in the
  slot; comparing +0x54 is meaningless). Rides the population/
  timeline divergence; no per-family fix.

- **MC2 (5,13) VILLAGER FAMILY — TERRAIN-CLOSURE CAPTURE, RULED
  2026-08-01 (opus dig).** Model 13 = the Townie/Villager
  (`AddVilliger_4BF40` EF:34037, behavior row 100, flags 0x9 =
  die-on-water + flee-on-hit). The dominant mc2l0 unexplained
  creature family (575 heading + 530 x + 503 y + 255 life + 55
  rand + 26 speed + 22 extra) splits in two, both capture:
  ① **the DROWN family** — every life row is retail-alive →
  port-dead(−1); the dying slots cluster on the EASTERN approach
  (154-173, 206-227), exactly the region MC2's village-growth
  construction paint terraforms to land at runtime (ledger
  §terraform); the pristine replay reads deep water there, so the
  port's FAITHFUL all-four-blocked die law (`mc2_move_core`
  mobs.rs:318-24 = EF:8855-62, row flag bit 0) drowns them —
  confirmed live at pair 1473 (slot 76: retail life 1000, port −1,
  port heading 1918 = the blocked-retry-yaw signature). Per-pair
  reseeding re-imports the live villager and re-drowns it every
  tick (slot 76 alone = 171 rows). Zero deaths on the stamped
  y=212 causeway strip — the load-time stamp is fine; the missing
  edits are the RUNTIME house-cluster regrades. Chaotic heading/
  x/y/rand/speed/extra = downstream of the deaths. ② **the +44
  family** (39 heading rows, got = want+44 ≈ 2× the v_2=22 turn
  cap, alive west-side villagers) — rides the RULED-byte-exact
  wander law's ±22/±45 blip capture (hypothesis-grade within an
  established ruling). Port walker/move/drown/retile/stamp laws
  verified faithful line-by-line — do NOT touch. Disposition:
  roster rule `mc2-walker-drown-terrain` (capture, mirrors
  mc2-guard-terrain); real remedy = the deferred terrain channel.

- **MC1 CLASS-9 AIM SKEW — SPLIT 2026-08-01 (opus dig); the (9,1)
  slice was a LAW BUG — FIX LANDED 2026-08-01 (session 3).**
  Refines the SPAWN-ARM entry's "aim skew stays open": (9,0) =
  fireball, (9,1) = the POSSESS LOB (`spawn_spell_lob(1)`,
  combat.rs:388). Split of the post-pose class-9 residue:
  (c) **LAW BUG, FIXED — (9,1) target_yaw, 525 rows → 0**:
  retail's possess handler `sub_52ED0` (:62970) homes through the
  SHARED `sub_52550_52890` (:62534) which writes +34 =
  angle_between and +36 = pitch_toward EVERY tick
  (:62543/:62546); the port's separate `home_possess`
  (combat.rs:1251) updated only f30/f32 and never wrote f34/f36.
  Proven by the one-tick-lag signature under the re-seed runner:
  361/363 rows-with-predecessor were exact got[t]==want[t-1].
  LANDED: `e.f34 = yaw; e.f36 = pitch;` in home_possess per
  :62543-46 — write-only for the lob (proj_m1_tick reads only
  f30/f32/f126), zero gameplay change, no golden moved (goldens
  never fire the lob); mc1l0 fixtures t=620/t=1158 signatures
  promoted (the target_yaw atom vanished); mc1l0
  conforming-or-explained 4,026 → 4,150. LATENT
  (decompile-corroborated, invisible on this corpus, do NOT
  bundle): home_possess hardcodes yaw cap 34 vs retail row-0
  v_2=56, and snaps f32=pitch instantly vs retail's v_6=22
  turn-step — the pitch one WOULD move trajectory if pitch ever
  steps >22/tick; separate behavior change with its own re-verify.
  (a) **(9,0) fireball = TARGET-DRIVEN, law faithful**: 229/250
  target_yaw rows co-flag `chase` (port acquires a different
  target — 152 port-acquires/94 reverse; exemplar slot 555 retail
  flew straight f146=0 while port acquired the pose-phase creature
  556); same-target cases show ±1-3 noise only. Stays
  open/capture-leaning; optional roster rule "target_yaw co-flags
  chase+heading ⇒ homing a divergent target".
  (a2) (9,1) pitch 144 rows = target BALL z via terrain-z —
  re-triage after the f34 fix. (b) ~56 (slot,t) slot-alias records
  (port holds a different class/model — lob born a tick off) ride
  mc1l0-cast-impacts. The (9,x) flags bit13 lead stays its own
  item.

- **MC1 (5,15) CASTLE-GUARD FAMILY — TERRAIN-CLOSURE CAPTURE,
  RULED 2026-08-01 (opus dig; the MC1 twin of §l4-guard-terrain).**
  Model 15 = the castle guard (behavior row 24, `v_20 = 0x20000` —
  terrain-locked to castle-pad tile types 21/22/24 + 13/14). The
  whole post-pose unexplained family (1,288 rand + 154 heading +
  547 x + 374 y rows on mc1l0) is ONE root: `grid_walk`'s vote-tick
  die-gate (`sub_20480` :25934-40, port mobs.rs:2600-04) reads
  `cap_bit & !v_20` — on retail's recording the castle build had
  regraded those tiles to pad type, so retail runs the 4-draw
  quadrant vote + 1-draw move coin (5 per-entity LCG steps,
  measured); the pristine-plane replay reads the original tile
  type, trips the die-return, and draws ZERO (proven: port rand
  `got` == seed on vote ticks, incl. the unaligned t=3415 case
  that disproves any phase/order theory). Heading = pure ±512
  knock-on (the vote's candidates are f30 + 512k, :25945-71 — a
  frozen +30 vs a re-vote); x/y = the movement knock-on; 154/154
  heading rows co-locate with a rand row (zero heading-alone → the
  uniform vote-weight stand-in contributes nothing here). All six
  diverging slots cluster in one tile box x17-26/y19-28 = one
  castle site, co-tiled with `mc1l0-terrain-z` want-512/got-256
  rows. Port laws verified byte-identical line-by-line — do NOT
  touch grid_walk. Disposition: roster rule `mc1-guard-terrain`
  (capture, mirrors `mc2-guard-terrain`); real remedy = the
  deferred terrain channel, which retires both games' guard
  families at once.

- **SPAWN-ARM f34 MIRROR — RULED DEVIATION (player, 2026-08-01).**
  The post-pose-filter mc1l0 target_yaw residue splits two ways: the
  (9,0)/(9,1) rows are both-sides-nonzero aim skew (targets that
  themselves diverged + cast latency — capture-flavored, stays
  open), but (10,0)+170 / (10,39)+73 / (10,1)+22 rows have retail
  +52 == 0 on EVERY row with the port nonzero — all BIRTH pairs.
  Mechanism: the port's spawn arms (`arm_projectile`, `corpse_drop`,
  payload/eruption/storm) mirror `f34/f36 = f30/f32` on every spawn;
  retail writes +52 only on homing paths, so corpse balls and
  Wall-of-Fire bolts (and the standing fires they convert into
  in-place) are born 0. The lane is WRITE-ONLY for those families in
  the port (readers: class-9 homing `proj_tick`, class-5 multipart)
  — no gameplay bearing; un-stamping would mean splitting the shared
  arm per-spell and risking the faithful homing paths. Ruled a
  deliberate deviation: DEVIATIONS.md "spawn arms (universal f34/f36
  target mirror)" + roster rules `mc1-spawn-arm-f34-{fire,ball,
  flame}` (status deviation). Guard: the rules' hit counts are
  birth-pair-bounded — a jump means a NEW divergence hiding behind
  the lane, re-triage.

- **MC2 CAVE RAND STRUCTURE, ROUND 2 (2026-08-01) — the mc2l30
  headline closed: rng mismatches 9,328 → 202 of 9,337 pairs.**
  The clean corpus offline-solve (recordings→`--csv` rng rows,
  scratch solver) fits `R' = LCG^k(R) + (turn+1)` — the additive
  lands AFTER every draw of the carpet's position, and the counter
  is the POST-increment Turn (solved s = recorded-turn@t + 1 on
  every fitting pair): k=2 on quiet ticks (6,806), k=4 exactly on
  drip ticks t≡5 mod 8 (1,001), +1 activity draw variants (846),
  367 "sandwich" pairs with draws after the additive (activity in
  slots ABOVE the carpet — proving the tail runs AT the carpet's
  pool slot inside the frame walk, not pre/post-pass), and 258
  pure-LCG pairs with NO tail at all: t=3257-3267 (possession
  holds the byte[1]&8 stall every tick, carpet flags 0x1000_0A0D)
  and t=9090..end (carpet action45 = 12, the level-end arm — the
  mover `sub_5D530` is only called from the flying arm EF:59994
  and the death-test arm EF:60074). Source-corroborated: exactly
  ONE unconditional global draw/tick at the frame-function top
  (EF:39947; the parked-carpet window measures precisely one
  draw), the drip reads the post-Turn++ player Turn (&7,
  EF:40501), the tail is `sub_5D530`'s late body (EF:59800-08).
  Port restructure (world.rs): tick-top draw unconditional for MC2
  (post-pass baseline deleted), drip gate → incremented mc2_turn,
  tail moved INTO the frame walk at the imported carpet slot (new
  `mc2_carpet_slot`; native = post-pass fallback), additive =
  post-increment counter, importer folds the mover-less action
  arms into `mc2_carpet_stall`. SUPERSEDES round 1's
  [drip→tail→pass→baseline] order and the tick-entry drip anchor
  (both fit on the TORN corpus under the wrong additive position).
  Numbers: mc2l30 roster-explained pairs 1 → 6,320, rng residual
  202 pairs (all churn-tick draw-count skew — rides §l30-churn);
  mc2l0 167 → **240 conforming** (the tick-top draw re-phases
  every mid-pass stream consumer on non-cave levels too); mc2l4
  roster-explained 13,325 → 13,330, rng 162 → 160; mc1/HW
  untouched. Goldens re-pinned as behavior (mc2_cave B-D + obs D,
  mc2_slice A-E + obs A-E); suites 0 regressions (mc2l0 t=737
  fixed, all mc2l30 sigs re-promoted).

- **MANA-BALL WAKE LAW (2026-08-01) — the ⓪b banked lead, ported
  verbatim; "mana rolls away downhill when approached" is now port
  behavior. ⭐ PLAYTEST CERTIFIED 2026-08-05: "Mana roll now
  faithful. It stops moving outside of the awake range, starts
  moving when moving in range."** Decompile dig closed every open
  question: the writer of 16 into +58 is `sub_54F80` :64361 — the
  SAME per-tick maintenance pass that decrements (:64321), called
  from the bucket walk `sub_54F00_55430` :64266. Law: +58 nonzero
  → decrement, mirror down the +54 chain; else if +59 nonzero →
  decrement it (DEAD branch — nothing ever writes +59 > 0); else
  2D squared distance (sub_42410 :52748, x/y only) to the LOCAL
  HUMAN's wizard entity (:64352 — single scalar index, rivals
  never wake balls) `< 37748736` (= 6144² = 24.0 tiles, strict) →
  +58 = 16, chain members 18 (:64364), +48 stamped with an
  isqrt-of-index artifact (not ported — flagged low-load-bearing).
  The corpus 17-tick period is emergent: 16 decrements + 1
  observe-zero re-arm tick (duty 16/17). Ctor 128 = `sub_3B5A0`
  :47465; HW twin `sub_554B0` byte-identical (hw:60542/:60576/
  :60582). Retail has NO class gate in the pass (bucket membership
  beyond balls/creatures = open); the port scopes to the
  corpus-proven rows: balls (10, state 41) now ride
  `mob_awake_pass` alongside class-5 (mobs.rs — counter handled as
  a raw BYTE, the i8-import −128 trap), and ball_tick's private
  decrement fold is REMOVED: the ballistic gate reads the
  post-maintenance value (retail handler order — this also fixes
  the old 1→0-edge quirk: a fresh ball's window ends at the
  counted zero, and each wake cycle moves 16 / freezes 1).
  Native + strict both (retail law); the settled-ball ground-track
  deviation still applies to out-of-radius balls only. Acceptance:
  `settled_ball_wakes_within_24_tiles_on_a_17_tick_cycle`
  (features.rs) pins the strict boundary + exact period. Corpus:
  mc1l0 440 → **450 conforming** ((10,39) fixture x/y atoms gone,
  t=882 fixture now conforming), mc1hwl0 46 → 48. Goldens
  re-pinned as behavior (flight A-C both modes, L005 B-E + obs
  B-E); suites 0 regressions, drifts promoted.

- **KINEMATICS ROUND 2026-07-31 (the banked coordinate+speed
  deep-dive) — four port/import fixes + three capture rulings.**
  Fixes (all decompile-corroborated, corpus-A/B'd; mc1l0 385
  conforming UNCHANGED, all five suites 0 regressions / 9 drifted
  sigs promoted; mc2l0 7 → 11 conforming + 8 rng-only):
  1. **Class-9 spurious speed ramp (the retail-+2 family, ~14k rows
     across takes)**: retail states 0 (`sub_65C20` EF:63126), 1
     (`CastPosses_65F60` EF:63261) and 29 (`sub_65B50` EF:63023 — a
     charged-impact wrapper over the state-0 body) fly at CONSTANT
     actSpeed; only the shared `sub_65820` core ramps ±2 toward
     minSpeed (EF:62923-31). The port's `mc2_flyer_tick` ramped
     every state toward 384 — the corpus proof was the delta sign
     flipping exactly at 384 and (9,0) (whose launcher floors speed
     at 384, EF:44224) being 100% one-signed. Gated out for
     tick70 0|1|29 (proj.rs). mc2l0 class-9 speed 6,426 → 342 rows,
     l4 ±2 rows 10,055 → 289 — residue = birth-pair cast-timing
     (the slots are free in retail at state-N). The original
     "ramp one step out of phase" hypothesis was WRONG — order was
     always right; the ramp itself was spurious.
  2. **(3,3) balloon ceiling-walk latch (the l30 retail-+48
     family)**: `sub_60D50` walks the cave ceiling at actSpeed 96
     with `byte[0]|=1` (EF:61896/61903) vs 48 flying (EF:61905),
     ceiling clamp flying-only (EF:61921) — the port law was
     verbatim (castle.rs) but the importer dropped bit 0, so every
     imported walker re-took the flying branch each pair. (3,3)-
     scoped bit-0 import (port bit 0 is per-class overloaded).
     l30 954 speed rows → 0 + the z/x/y cascade.
  3. **(3,1) = the MC2 RIVAL WIZARD, replayed as a frozen husk**
     (NOT a balloon — the banked label was a misID): every (3,1)
     field satisfied got(t)=want(t−1) for the wizard's whole life —
     the ±16 "dither" was a one-tick lag on retail's ±16/tick speed
     slew (EF:6484, port-verbatim rivals.rs). `retail_import_mc2`
     never re-anchored `self.mc2_rivals` (the MC1 rival-freeze
     twin); `reanchor_mc2_rival` now points the brain at the
     imported slot + reseeds vdes/strafe/grace/mana lanes from the
     closure. l4 (3,1) rows 46.6k → 24.3k; the REMAINDER is the
     AI decision-lane reconstruction (state/target/hate/burst —
     needs the MC2 wizard-ext decode; the same split as MC1's fix).
  4. **(10,39) sphere mover — the ledger's §sphere spec, ported**
     (TransformArcherToMana EF:26015; behavior change toward
     retail, cave + slice GOLDEN&OBSERVABLE re-pinned, post-init/A
     hold): the MC2 settle law is MC1's shape at different homes —
     moving only while `byte@0x39 || fresh-kick` (EF:26173; ctor
     seeds 128, EF:36617; corpus: b39 counts ~1/tick to 0, f2c
     parks at −16, frozen forever) with z-velocity @0x2C. The port
     had opted the MC2 arm out of the settle gate entirely →
     always-on physics dropped every authored sphere to bare
     ground (the l4 z family) and re-rolled/merged settled spheres
     forever. Landed: settle gate (f58 was already imported from
     b39), f46 ← @0x2C import, absorb-chase (b0&0x40) + decay
     (b1&0x20 → bit-13 tail) + stall-skip (b1&8 → bit 26) latch
     imports, unconditional moving-mode gravity, the EXACT bounce
     `−impact/4 zeroed at ≤16` (EF:26244-52, replacing the
     untraced −32 floor), grounded-ONLY merge (EF:26265-69 —
     the always-scan was port invention), and the per-size
     ROTATION quad on re-sprite (EF:26744-77: 14·(size+1), 13 at
     size 0 — the port stamped MC1 art extents into the applied
     lanes instead: the 16k applied_yaw/pitch family). l4 (10,39)
     37.9k rows → ~3.8k (residual = terrain-closure z + birth
     edges); applied 16,004 → 270. MC1 arm byte-untouched.
     OPEN note: retail l4 renders a RIVAL-claimed sphere in the
     NEUTRAL family (sprite 56 with live class-3 owner 298) while
     mc2l0's human spheres color 105+size — the wizard spawn
     stamps ext color = slot for both (EF:43710), so the neutral
     mechanism is unresolved; conformance-invisible (sprite lane
     uncompared, rotation is size-only), port keeps team colors
     natively pending a colored-rival-sphere take.
  Rulings from the same round:
  - **§wander turn law = BYTE-EXACT, capture** (opus dig): turn
    clamp `sub_58350`, alt core `sub_580E0`, polar step, wander
    nudge, block-retry chain incl. the precedence quirk, move-
    then-nudge order, goat per-tick sound draw — all verified
    verbatim. The ±v_2/±341 heading blips are self-healing chaotic
    amplification through position-fed branches (rand streams
    match); the hypothesized 24-31-unit "binary branch divergence"
    DOES NOT EXIST (torus wraps + one t=17954 spawn wave). Two
    real leads split out: the HELD-STATE SPLIT (retail parks a
    goat in action 15 = +7 controlled, port wanders at 9 — a
    StageVar hold-gate miss, drives the sustained ±341 runs) and
    the (5,0)/(5,3) FLYER Z-BOB (±8..56 airborne offset — the
    multipart altitude source, untraced; NOT the walker path).
  - **§effects (10,0)/(10,6)/(10,14) = CAPTURE** (opus dig +
    substitution-split measurement): every fire/smoke motion law
    verified byte-exact (smoke `actSpeed−4 clamp[64,128]`, fire
    flicker `rand%0x41−32`, emitter `rand%0x4D` bonus). The
    "64-quantum" was the SHARED clamp; the one-sided spikes were
    100% slot-substitution rows. The standing "sub_580E0 alt-core
    arg order?" lead is CLOSED — exact, dead a4 correctly dropped.
  - **The lightning-trail TESTS were stale, not the law**: the two
    mc2_spell_channels lightning tests asserted live end-of-tick
    (9,9) nodes — under the certified born-dead law the trail
    decays within the cast tick (which node survives is pool-
    layout noise; they failed at the PRIOR commit too, hidden by
    cargo's per-bin fail-fast — run `--no-fail-fast` for truth).
    Re-asserted on laid RECORDS. ⚠ presentation question for the
    playtest: retail's crackle renders from the mid-frame draw;
    the port draws end-of-tick — verify lightning still reads
    visually.
  - NEW LEAD (out of family): the l4 (5,4) ARCHER walks at a
    CONSTANT −192 z from t=0 with byte-identical dynamics — a
    pristine-plane datum gap at its site, i.e. the LOAD-TIME
    terrain-edit question (the (14,5) plateau entry's l4 face),
    not an entity law.

- **MC2 CAVE AMBIENT RAND TAIL + the turn anchor (the mc2l30
  ">16 draws/tick" banked lead)** — RESOLVED 2026-07-31.
  ⚠ PARTIALLY SUPERSEDED by "MC2 CAVE RAND STRUCTURE, ROUND 2"
  above: the tail's EXISTENCE, LCG constants and full-counter
  addend stand, but this round's tick ORDER
  ([drip→tail→pass→baseline]), the pre-increment additive/drip
  anchor, and the "s = t" solve were artifacts of the TORN corpus
  — the clean re-record pins additive-last at the carpet's slot
  with the POST-increment counter. Level 30
  is a CAVE (`map_type: "cave"` + ceiling plane), and retail's
  carpet handler `sub_5D530` runs a cave-only tail (EF:59800-08):
  `rand_0x8 = 9377·rand + 9439 + counter` — a NON-LCG perturbation
  of the GLOBAL stream, once per carpet, which the port omitted
  (the runner bucketed the unreachable values as ">16 draws";
  they are a small step count + an additive). Three
  corpus-solved laws beyond the decompile: (1) the addend is the
  FULL per-tick counter (= the local player's Turn, reset at level
  load) — solved s=304/305/309/310 at ticks 304/305/309/310;
  remc2's `uint8_t setting_30` typing is refuted (the counter
  passes 255); (2) intra-tick op order solved from the recorded
  stream: [cave-drip draws (8th ticks)] → [carpet tail] → [frame
  pass draws] → [baseline ApplyEvents draw] — the human carpet
  updates in a PRE-pass phase (tail-first fits r=k−1 on ~all
  solved ticks; drip ticks fit (k=4,r=1)), so the MC2 baseline
  draw moved post-pass (count-preserving on non-cave takes —
  mc2l0/mc2l4 parity untouched, suites green); (3) the drip
  cadence anchors on the TICK-ENTRY counter (turn0&7==0; phase
  scan 442-vs-535+ rng mismatches per 2000). Wiring: the importer
  anchors `mc2_turn` from the recorded player Turn (also fixing
  the drip cadence which previously re-anchored to 0 every pair)
  and arms the carpet's byte[1]&8 one-shot stall skip (EF:59616 —
  the handler early-return that also skips the tail; the retail
  (1,1) stall pairs pinned it). mc2l30 rng mismatches 63% → ~22%
  (all residuals = churn-tick count mismatches, §l30-churn), first
  fully-conforming mc2l30 pairs appeared (t=6 promoted FIXED).
  Runner tooling: `--csv` now emits a per-pair `rng` row (retail,
  port) — the offline solver's input. ⚠ the ambient-loop SOUND
  gate (%0x83<5) reads the perturbed value without stepping it —
  presentation, owed to the audio layer, NOT the sim `sounds` vec.
- **MC2 m0 worm/hydra DEAD-BOB import lane** — RESOLVED 2026-07-31
  (the mc2l4 triage round's dominant family: 2.6M diff rows —
  (5,0) z/pitch/x/y/heading on 140 slots, every pair). The class-5
  arm of `import_ent_mc2` mapped port f26 ← retail @0x2E (the
  charm/armed lane, the mc2l0-era A/B choice), but the m0
  worm/hydra keeps its BOB VELOCITY in @0x10 (`dword_0x10_16`:
  the multipart ctor seeds it, `sub_1F040` integrates z += f26,
  f26 −= 5/tick, bounce +150 at terrain+256) — so every imported
  worm head had a dead bob and sank while retail undulated
  (corpus: slot 2 climbs +136/tick to ~2400, ~60-tick arcs, rand
  FROZEN — pure deterministic ballistics; the port's bob law
  already reproduced the arc exactly once seeded). Fix: the f26
  import is model-aware — (5,0) takes scratch10 (@0x10), other
  class-5 keep @0x2E (conformance.rs). mc2l4 z 899k→340k, x
  596k→100k, y 594k→98k, pitch 556k→51k, heading 516k→75k rows;
  all three prior suites green. RESIDUAL (5,0)/(5,3): smooth
  ±1..6 heading / ~±25 z accumulated drift (wander/bob phase
  detail, own open entry below). ⚠ the f26 dual-homing is
  per-MODEL, not per-class — m2's attack countdown and the
  doomsday timers are ALSO @0x10-homed in the port; if their
  families surface in a future corpus, extend the match arm, do
  not re-litigate the class-wide A/B.
- **MC2 castle PHANTOM-UPGRADE import lane (the mc2l4 build-out
  block)** — RESOLVED 2026-07-31, the MC2 twin of MC1's
  phantom-upgrade family. `import_ent_mc2` filled f59 from @0x3A —
  DEAD for (3,2) castles, whose build sub-state lives in @0x2E
  (`word_0x2E_46` → f59, docs/traces/mc2-castle-builder.md §2) —
  so every imported castle sat in f59=0 (level-up commit) and the
  port re-ran `mc2_castle_upgrade` each pair: level 1→2 → the
  HP/CAP ladder one level high (max_life 9375-vs-4687 = exactly
  `40000·Life60>>8` vs `20000·Life60>>8`; mana_max 18000-vs-8500 =
  CAP[2] vs CAP[1]), z frozen for the tick (the upgrade path never
  writes z → the rigid one-step rise lag on (3,2)), and one
  phantom (10,42) painter spawned per pair (the slot-304 squat
  where retail spawns a second rival castle head at t=5). One
  model-aware import remap (conformance.rs f59 ← @0x2E for (3,2))
  cleared max_life/mana_max/model/class/player.mana_max and the
  painter extras from the pairs-0..300 window; suites green.
  FOLLOW-UPS split out as their own entries (below): the (10,42)
  painter's parent @0x28 is NOT projected by obs_project_mc2
  (owner retail-297-vs-0 — the "@0x28 nonzero only on class-15"
  comment is false for painters), and the (3,3) stage-piece
  −128 z residual + the player.mana_max claim census are separate
  families to re-measure post-fix.
- **MC2 lightning trail-node born-dead law + phantom yaw stamp**
  — RESOLVED 2026-07-31 (the mc2l4 (9,9) window t=2517..8494:
  34k extra-in-port + 16k max_life + 14k life + 14k heading
  rows). The (9,9) swarm is the tier-0 Lightning beam's cosmetic
  trail (sub_66750 lays steps·8 sprite-216 billboards per cast,
  action 14 = sub_67410 pure pre-decrement decay). Retail births
  each node DEAD: `maxLife = (node_slot >= beam_slot) - 1`
  (EF:58341, so 0 ahead of the beam / −1 behind; life copied
  from maxLife; the ascending frame pass drives both to the
  disabled bit within a frame) and never writes the node's yaw.
  The port hardcoded max_life=1 (born-alive → 3 enabled frames →
  slot-recycle skew accumulating extras) and stamped the beam
  yaw into f30 (the heading family, retail 0). Both fixed in
  proj.rs (max_life encodes −1 as wrapped u32 — refill_life and
  the obs projection both cast through i32). Window t=2517+200:
  max_life 972→154, life 1087→273, yaw-stamp heading family
  gone; suites green. RESIDUAL (9,9)/(10,23) extras+missing =
  the input-delay-2 cast-timing skew + retail's parked ghost
  husks vs the port's free-list reuse — capture-domain, rides
  the standing input-latency + free-stack rulings.
  (docs/spell-audit/lightning.md §trail updated — the old "life
  1, self-despawning" note was the refuted reading.)

- **CASTLE COLLATERAL DAMAGE (the mc1hw playtest-round-2 chain:
  "Vodor tougher than retail" + "fast respawn")** — RESOLVED
  2026-07-31, opus decompile dig + corpus (mc1hwl0 slot 522, life
  20000→dead t=9457 at −833/tick; window 9325-9345 castle-life
  diffs 12→1 after the round; mc1l0 385 conforming UNCHANGED; all
  three suites green, L005 GOLDEN re-pinned A-E with OBSERVABLE
  holding — layout-only in that window). One chain, five laws:
  1. **+78 is SIGNED** in sub_118C0's z test (`ent_overlap`,
     `player_overlap`, the app-side `overlap`): the decompile
     types it `uint16_t` with a 32-bit `abs32` — a movsx artifact;
     the 0xE000 literal and the corpus overlap only reconcile as
     −8192. Port previously widened unsigned, so any entity with
     a negative z-center was orphaned from every AABB test.
  2. **Castle extents quad** (sub_37150 :43798, HW 40191-203):
     `+78=0xE000, +80/+82=((dim<<8)+1280)>>1, +84=0x4000` — now
     written at the level-up commit, the downgrade, castle_extents,
     AND re-applied in the settled tick's every-other-tick block
     (sub_46DB0 :52083, level VERBATIM) with the every-settled-tick
     `+144 = +24` owner echo (:52080). The port had deliberately
     skipped the marker ("would z-orphan our AABB overlaps" — true
     only because of defect 1).
  3. **Castles are homing-acquire candidates**: sub_54520's list-1
     walk (the significant-entity list: wizard models 0/1 + castle
     model 2) branches `+65==2` to the dedicated castle scorer
     sub_54BD0 in the base cases 0/3/4 (cone 0x71) and HW's meteor
     case 0x10 (cone 0x100) alike. The castle scorer is the
     generic scorer minus the sub_524C0 z-lift bracket (which
     itself skips model 2): castles are aimed at the RAW flag
     position. Ported into aim_assist_mc1_cone + the crosshair
     preview (Creatures set) + the victim-teleport lift skip +
     the AimLock alt. NOTE the sub_524C0 guard is MODEL-only (any
     class's model 2 skips the lift) — ported verbatim.
  4. **(10,53) cloud joins the class-10 PRE-decrement family**: 7
     burns from a 6-life cloud (pre-values 6..0), 5831 delivered
     per cloud — the corpus bursts are 7×833 (14 for two
     overlapped clouds), and the burst arithmetic is the proof
     (the decompile's C shows post-decrement; the batch law +
     corpus overrule it). Terminal act_life = −2, matching every
     class-10 ghost record in the corpus.
  5. **The sub_52770 explode stamps the child's +146 with the
     struck victim's SLOT** (:58859-64 `v20[73]`) — states 3/17
     ONLY; the m0/m1 explode blocks (:59015/:59092) write
     owner/yaw/pitch alone. First landed unconditionally, which
     put a foreign chase lane on (10,0) children of m0 explodes
     (suite drift caught it at t=355/366) — re-scoped via a
     stamp_victim parameter. Mechanically inert (no handler reads
     it; the cloud's damage is pure position overlap) but it is
     an observable lane.
  Death chain verified retail-equivalent: demolition clears the
  owner wizext `var_50` (:52598) where the port's `rival_castle`
  id24-scan needs no stored binding; the castle-less elimination
  (:55601-30) was already verified byte-identical. Intake law
  confirmed: the ch0 castle pre-pass gates ONLY model==2 + owner
  +24 differs + sub_11950 overlap — NO damageable-flag or
  +28-mask check for castles — and the general ch0 pass excludes
  (3,2) (both already ported). Gameplay effect: meteors aimed at
  a castle-camping rival now lock and fell the castle → castle-
  less death → ELIMINATION, replacing infinite camp-heal-respawn.
  PLAYTEST OWED. Banked adjacent leads: retail's list-1 walk also
  gates candidates on the OWNER's row v_28 rooted range BEFORE the
  scorer (port keeps only the scorer's 5120 — unexercised by
  corpus so far); base-MC1 napalm_tick never decrements act_life
  (retail does — inert under the 15-wave cap, but the obs lane
  drifts; fold into the banked base-17 +44-copy pass).
- **MC1 TICK-TOP REAP LAW (the castle-window "pool-order cascade" +
  the HW linger families)** — RESOLVED 2026-07-31, decompile-
  corroborated remc1:52226-31 / remc1hw:48276-81: retail has ONE
  unconditional reap pass at the TOP of every sub-step (after the
  LCG draw, before the awake build and dispatch) freeing every
  `class≠0 && flags&0x400` record via sub_41E90. Death paths only
  SET the flag (single setter sub_41E80 :52508, ~100 callers) or
  hard-free inline. Consequences that all fall out of the one pass:
  a record flagged mid-tick persists through that tick's snapshot
  (the delivered create-castle projectile's 0x406 one-frame linger;
  there is NO separate delivery latch — reap-before-dispatch IS the
  latch), corpse records persist MULTIPLE frames because the corpse
  HANDLER (sub_1A800 :21855-71) gates its own flagging on
  `f63 & 7 == 0` (the worm lanes), and same-tick spawns pop the
  PRE-EXISTING stack rather than the dying slots (the castle → 627
  = stack top, then the same-tick (10,42) painter → 481). The
  port's same-iteration free lost the linger AND recycled dying
  slots; the MC2-style next-frame deferral (the refuted 384→377
  experiment) re-ticked flagged records — the correct move was the
  FRONT of the tick, not the back. Landed MC1-scoped (native +
  strict; MC2 keeps its measured next-frame ghost law pending the
  owed sweep-law port). mc1l0 367→385 conforming (18 fixed, 0
  regressed; missing (10,0) 735→58, (10,12) 288→57, (9,1) 468→192);
  mc1hwl0 missing rows 717,798→33,379, phase-clock rows 67,378→
  2,257 (the (1,9) pattern was linger records vs respawned slots).
  Native goldens re-pinned as a BEHAVIOR change (flight-tier leg B
  both models; L005 GOLDEN+OBSERVABLE D-E — death records live one
  more snapshot and slot reuse shifts; post-init..C hold). The t=136
  mc1l0 capture fixture flipped conforming; t=470's phantom-castle
  atoms cleared.
- **§class15 manifestation aliasing + spellbook import** — RESOLVED
  2026-07-30 (take-2 fix round): `import_ent_mc2` now applies the
  cast.rs class-15 map (EIGHT fields — the ledger's seven plus the
  cadence flag `@0x3B → f59`, which gates rapid-fire): @0x2E→f26 ·
  @0x30→f28 · @0x2A→f30 · @0x2C→f44 · @0x36→f54 · @0x88→f136 ·
  @0x8C→max_life · @0x3B→f59; the projection reverse-maps heading=0,
  max_life=0, mana_max←max_life (measured constants; applied/speed/z
  ride through untouched). `action = 3·model` CONSTANT even across
  tier upgrades (measured — the "state" term never moves in this
  take), so the uniform tick70 lane round-trips. ALSO: the human's
  str_611 spellbook (banked/volatile XP @+0x649/+0x6B1, manifestation
  slots @+0x719, ring @+0x79B, levels @+0x803, sel @+0x81D — offsets
  validated against the pool roster) now imports per pair; before, the
  cast machinery ticked the WORLD-BUILD slots and the book's XP was a
  cross-pair leak of its own.
- **MC2 economy block: @0x1A id fusion + claim census + regen seed +
  castle echo** — RESOLVED 2026-07-30: retail's `id_0x1A` is the LIVE
  owner-or-self lane (census over the take: caster on projectiles,
  owner on castles/balloons/charmed (5,15), watch target on class-11,
  self elsewhere) while `parentId_0x28` is nonzero ONLY on class-15 —
  the fusion now imports `tr(owner28 ∥ f1a ∥ slot)` and the obs owner
  lane projects class-15-only (detached manifestations excepted).
  This fixed the claim census (`recompute_mana`: mana_max = 1000 +
  Σ f140 of claimed via f144/id24) → player.mana_max, entity
  mana_max, player_ent_idx, and the ball-claim stamps (bolt id24 =
  caster, not its own slot). `player.mana_delta` seeds from the
  carpet's @0x88 (the MC1 f132 law's twin). `player.castle` is now
  ECHOED from the recorded per-player word (+1080 = the AUTHORED
  castle binding; a runtime-BUILT castle never fills it — 0 across
  this take with the castle live; deriving it from the pool was a
  6,083-pair regression, briefly).
- **(10,0)/(10,6) fire aliasing + the activation bit** — RESOLVED
  2026-07-30: retail's `byte0&2` (the one-shot-done latch) imported
  only to the port's bit-25 mirror while the fire/explosion ticks
  latch on POSITIONAL bit 1 — every imported active fire re-ran its
  activation (area damage + flicker draw + scorch + sound) each pair
  (the fire-band rand churn, 19k→11k on the fix). The fire field
  map: @0x2A subSpellIndex = the area AMOUNT → f140, @0x2C = the z
  flicker/lift → f44, @0x90 mana lane dead-0 (projection override) —
  the uniform @0x2A→f44 alias fed the flicker a 400-unit constant
  (masked until the activation fix — the two-wrongs trap).
- **MC2 sweep laws (strict-retail scoped) + the ghost-record law** —
  RESOLVED 2026-07-30, measured on the take: (a) NEWBORNS never tick
  in their birth pass (the phase byte stamps at spawn; a fresh
  emitter particle surfaces at t+1 with life 32 and spawn z/speed
  untouched — the port's same-pass tick skewed all nine opening
  smoke columns every pair); (b) DISABLED entities (byte[1]&4) never
  run again, but their pool records PERSIST until slot reuse (the
  (10,1) death record sits a frame at life −2; the recorded obs
  carries ghosts, so the projection must too); (c) ghost slots are
  NOT in the recorded free stack — the next frame's remove pass
  pushes them (ascending scan; measured via the reused-slot ↔
  emitter mapping: 129←113 … 122←120, LIFO) — the importer appends
  ghost slots ascending atop the recorded stack; (d) ghosts NEVER
  tile-link (their link bit is stale bytes), and `new_event`
  defensively unlinks any still-linked record it reallocates —
  without (d), a reallocated linked ghost leaves a dangling chain
  pointer and the tile-chain WALK CYCLES: pair 9074 grew a 100 GB
  `area_write` victim list (use `ulimit -v` on full runs; `--start`
  + the per-pair announce found it in seconds). Laws (a)/(b) are
  STRICT-RETAIL ONLY for now: the native MC2 dome/eruption chain
  relies on the same-pass tick (the (10,19) summit column dies
  unspawned under the gate — mc2_slice caught it), so the native
  port of the sweep laws is OWED together with that timing fix;
  native goldens unmoved, DEVIATIONS.md entry added.

- **MC2 held-goat idle BLEAT draw (the mc2l0 rand family)** —
  RESOLVED 2026-07-30: retail's phase-7 goat wrapper
  (`AddGoat05_01_1F5B0` EF:11452) rolls the per-entity u16 stream
  once EVERY held tick (bleat on `% 0x4D == 0`); the port's held
  seam deliberately skipped the sound rolls (stagevars APPROX
  register), silently freezing every held goat's rand stream. The
  mc2l0 corpus measured it: 82,353 of 86,947 rand hits (95%) were
  held goats. `mc2_held_tick` now runs `goat_snd(i, 0x4D)` for
  model 1 between the 1D5D0 legs and the speed tail (retail order);
  rand family 86,947 → 3,874, first conforming MC2 pairs (0 → 7).
  MC2 slice goldens re-pinned (GOLDEN A-E + OBSERVABLE — a real
  behavior change toward retail; post-init holds). Other models'
  wrapper rolls remain skipped (APPROX, per-model transcription
  owed as §effects narrows).
- **MC2 importer wiring findings (landed with the importer)**:
  (a) class-9 projectiles must carry the port's `F_MC2PROJ` marker
  (bit 29, collidable bit cleared — the ctor convention) or they
  fall into the MC1 fallback arm and index MC1's 31-row BEHAVIOR
  table with an MC2 row (a panic, not a family); (b) the port fuses
  retail's own-id (`id_0x1A` = slot) and `parentId_0x28` into
  `id24` — import owner-if-nonzero-else-slot, project owner as 0
  when `id24 == slot`; (c) behavior rows derive from `ptr_a0` via
  retail's own load fixup `(ptr − base160@0x36DF6)/34 + 59`
  (validated: every live mc2l0 entity converts, creatures land on
  their model rows); (d) the free stack lives at top@0x35 (the
  0x242 dword is DEAD in remc2) + pointer cells @0x246, recycle
  @0x11E6/@0x11EA, allocation pops free-first (opposite of MC1's
  recycle-first) — g.free = recycle ++ free so the Vec pop matches.

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

## The known-deviation roster (2026-07-31)

`conformance/known-deviations.json` + `verify-deltas` classification
(docs/CONFORMANCE.md §roster): every diff row is tagged against
scoped, ledger-cited rules (capture / deviation / open) and the
report's headline is the UNEXPLAINED residue + per-rule hit counts;
`--csv` carries the rule id per row (`--no-roster` = raw). Seeded
from this ledger's ruled families (33 rules). The player-stated
goal: a fully triaged take runs to unexplained = 0 — everything
conforming or known. Baseline at seeding (2026-07-31, post-
kinematics-round): mc2l0 **5,242 of 7,762 pairs conforming-or-
explained**, 7,512 unexplained rows (gross was ~300k); mc2l4
8,398/12,786 + 14,136; mc2l30 3,434/10,021 + 13,878; mc1l0
1,196/5,329 + 44,523 (the walker x/y/heading terrain knock-on is
DELIBERATELY unexplained until a terrain channel exists — only the
direct z family carries the ledger's whole-take ruling); mc1hwl0
800/40,586 + 2.17M (only the z closure seeded — the §weather/
token/census families await their own triage rounds). Notable: the
roster instantly SIZED the undug (3,3) balloon-z lead at 40k rows
on l4 / 19k on l30 (tagged open, not hidden).

**Capture-window clarification (player question, same day)**: the
read-consensus scheme (N byte-identical neighboring reads ⇒ the
guest is between ticks) IS the recorder's mechanism — but identical
reads prove only that the guest was FROZEN, and DOSBox regularly
parks MID-entity-loop, so a perfectly stable consensus image can be
a mid-tick state (RECORDING.md "Capture tearing" — the original 75%-
torn corpus). Higher snapshot frequency cannot fix this (it is an
alignment problem, not a sampling-rate problem); the by-construction
fix is the tickpatch MAILBOX window (`in_window` raised during the
pacing spin = a guaranteed quiescent window), which is why mc1l0
runs 0 torn. The MC2 takes run the pacer but not the windowed
mailbox — they fall back to the phase-byte tear gate (~33% torn,
plus the per-entity torn-slot exclusion). **The owed MC2 tickpatch
mailbox/emit gate would reclaim those pairs the same way —
PLAYER-APPROVED 2026-07-31 as the next session's headline ("the
final piece for proper recording"): a NETHERW_REC.EXE arm hooking
MC2's OWN frame limiter (no pacer needed) at the true frame
boundary — after the post-pass ApplyEvents baseline draw, before
the next PlayerEvents Turn++ — so the Turn++-park tear mode
becomes unobservable by construction. New mailbox magic + its own
window-open counter (Turn advances mid-frame, unusable as a
continuity token); recorder grows the MC2 windowed path. Pays on
re-recorded takes only.**

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
- **MC2 tear law (measured, supersedes the old "Turn + LCG parity"
  guess)**: Turn advances on EVERY adjacent pair (it increments in
  `PlayerEvents` BEFORE the entity pass) and the global LCG draw
  count is activity-dependent, so neither discriminates. The gate is
  phase-byte step-1 DOMINANCE (`byte_0x3E_62`, RECORDING.md); 1105 of
  mc2l0's 3640 pairs (30%) are torn. WITHIN accepted pairs,
  minority entities can still be individually torn (0- or 2-pass) —
  the runner excludes them from field comparison per slot
  (`verify_mc2::torn_slots`); their signature was the perfectly
  balanced ± families (life ±1, z ±64, speed ±4, y ±30) that no sim
  law produces. A recorder-side MC2 emit gate is still owed.
- **MC2 input closure (mc2l0 §casts)**: the 2026-07-29 take carries
  `channels.input: "none"` — the human's casts are invisible
  (control commands consumed+zeroed mid-tick). Every human cast
  surfaces as missing (9,x) projectiles + player.mana spend families
  (fixtures t=425/1410, `capture`). **CLOSURE FIX LANDED 2026-07-30**
  (recorder-side, no exe patch): the MC2 raw-input register frame
  (held buttons + press latches + cursor + cursor-at-press +
  pressedKeys) is now mapped and validated — RECORDING.md "input" —
  and `verify_mc2`/the fixture loop consume it (`fire = held ∥
  latch` through the `--input-delay` ring). A RE-RECORDED take gets
  the channel automatically; these fixtures stay `capture` for the
  old take and retire with it.
- **MC2 terrain closure (mc2l0 §terraform)**: village growth
  terraforms the hill under the (157..173, 205..209) house cluster
  at ~t=751; house ticks re-snap z to terrain both sides, so every
  later pair shows the (10,45) z family against the pristine plane
  (fixture t=1447, `capture`) — the MC2 face of the mc1 ledger's
  dominant TERRAIN CLOSURE residual. Same fix direction: a terrain
  channel in .mgcr v2.

## mc1:49 — the map "O" that triggers nothing: RULED FAITHFUL
## ⭐ RETAIL-CONFIRMED BY PLAYER 2026-08-04 (ruling closed, top tier)

Player report (2026-08-03, MC1 level 049, the last campaign level):
after the final genie wave dies an "O" map marker appears, attached
to nothing — flying over it does nothing. Investigated without a
recording (decompile + level data only). **Verdict: the port is
byte-faithful; retail does the same. No fix, no deviation.**
**Player replayed the level in retail (2026-08-04, cheat-assisted)
and confirmed: the O appears there too, inert — a known community
oddity, apparently a Bullfrog placeholder for an ending sequence
that was never built. Ruling stands at the highest evidence tier
(disasm + retail replay agree); do not re-open.**

**The level's trigger graph** (`baked/mc1/level-049.mgcl`, 114
class-11 THINGs — the densest in either game):

- 104 x (11,0) proximity one-shots, whose dispositions hold 103
  (10,9) growing-hill/volcano creators + 27 (5,6) creatures. This is
  the "most triggers spawn volcanoes" the player saw.
- (11,6) @ (123,152), box 64 — the leave-polarity one-shot that
  opens the level: fires dis 1 = the main wave (19x(5,2), 4x(5,3),
  14x(5,5) crabs, 17x(5,8), 10x(5,16), 1x(5,6), 1x(5,9)) plus four
  kill triggers.
- Kill chains (state = 13 + watched class-5 bucket; state 30 = the
  -1 "all buckets" variant):
  (11,15)@(45,159) bucket 2 -> dis 101 -> (11,15)@(57,74) -> dis 102
  -> (11,21)@(90,31) -> dis 103 = EMPTY;
  (11,18)@(134,251) bucket 5 -> dis 46 = 8x(10,52) crab eggs;
  (11,21)@(32,222) bucket 8 -> dis 6 = 9x(5,8);
  (11,30)@(230,86) ALL -> dis 87 = 5x(5,11) GENIES + (11,24)@(118,85)
  -> dis 106 = 6 more genies + (11,24)@(153,128)
  -> dis 107 = **the (11,31) at tile (0,0)** -> its own dis 108 has
  ZERO member THINGs. Terminal.

So the O is authored, placement is faithful (LEVELS.DAT entry 49,
sha `b6d6c6ff…`, slot 1633, x=y=0 -> the map's corner), it appears
exactly when the last genie dies, and even a hypothetical trip could
spawn nothing because disposition 108 is empty.

**Retail law for class-11 state 31** (remc1 + CARPET.EXE, both):
`str_256038[31]` (remc1 sub_main.cpp:4953) is a LIVE entry
(`data4 = 0x1F`, `data10 = 1`) pointing at `sub_5A080`. In
CARPET.EXE that data row sits at VA 0x981EA (`F4 68 00 00 1F 00 80
A0 04 00 01 00 00 00`) and `sub_5A080` is **one byte: `C3`** — the
state-30 thunk at 0x5A070 falls through a `90` pad into that shared
`ret`. The dispatch site (VA 0x41A0A-0x41A7C: `movsx ecx,[ebx+0x46]`
/ `imul edi,ecx,0x0E` / `call [eax+0x6]`) has **no state bound
compare** — index 31 is genuinely dispatched, and does nothing.
Whole-image scan: the 12 callers of the proximity probe 0x5A090 are
states 0-3/5-12, the 18 callers of the kill helper 0x59E40 are
states 13-30, and 0x5A080 has **zero** callers besides the table
slot. WAV 41 (inside the probe, at 0x5A0E9) is therefore unreachable
from state 31. Model 31's only other consumer in the entire binary
is the map draw `sub_48710` (model jump table VA 0x4868C: models
9-12 -> sprite 83 "X", model 31 -> sprite 84 "O", all else nothing).
MC1 has no exit-marker win path at all — the level ends through the
mana-share latch `sub_415C0` (bit 1 of +13325) — so MC1's O is NOT
MC2's ending switch despite sharing the sprite and the model number.

**Port**: `World::trigger_tick`'s `_ => {}` arm == retail's `ret`
(the dispatcher's `f63++` at :52406 is applied by the caller for
every entity, so even the phase clock matches);
`World::advertised_marker_poses` plots models 9..=12|31 exactly like
case 0xB. Comments at both sites now carry the proof.

**Retail-replay checklist** (to confirm on the player's next run):
the O should appear at the moment the last genie of the second
(11,24) wave dies; it should sit in the map's (0,0) corner, i.e.
diagonally opposite/wrapped from wherever the player is, never
moving; flying through that corner should produce NO chime (sound
41), NO spawn and NO level end; and it should persist unchanged
until the level is won on mana share. If retail instead chimes,
spawns, or ends the level there, `str_256038[31]` is being reached
by some path this dig did not find — reopen.

## SESSION-9 LANDING ROUND, BUNDLE 4 (2026-08-05): fool's-mana OPEN-6/OPEN-7, the hate-decay from-binary check, the Vissuluth wake metric

Four backlog items, one dig. Two closed as NO-BUG with citations, two
landed; the only corpus mover is OPEN-7, and it moved the corpus the
right way.

### 1. OPEN-7 — the chord march probed sub-steps retail never visits, and it WAS costing us pairs

Retail's flight states run the victim probe ONCE, at the END of a full
step (`sub_65C20` EF:63126-29: MoveEntity → CopyEntityPosition →
`sub_10780`). Our anti-tunnel march walks the chord in ≤128-unit
sub-steps and probed **every one from the muzzle out**, so a projectile
born co-located with a targetable entity it does not own detonated on
its first sub-step.

**Landed law** (`crates/mgc-sim/src/mc2/proj.rs`, `mc2_flyer_tick` +
the new `mc2_hit_covers`): a victim whose box already contains the
step's START is admitted only at `k == n`, retail's own probe point.
Mid-chord ENTRIES still detonate at the sub-step, so anti-tunnelling is
intact. Chosen over "skip `k == 1`" because a PARKED projectile's only
probe IS the endpoint and retail detonates that one — which is why the
existing pin
`fools_trap_bolt_leaves_from_the_sphere_box_top_and_clears_its_own_muzzle`
keeps its contrast arm unchanged (it pins the endpoint LAW, not the
residual). New pin:
`a_projectile_born_inside_a_foreign_box_flies_clear_of_its_muzzle`
(engine/world.rs). A/B toggle `MGC_NO_MUZZLE_ADMISSION=1`.

**The audit called this latent. It was not.**

- **mc2l0 0+2000: 1703 → 1704 conforming.** The whole delta is t=618
  slot 165, a (9,1) possession bolt the port blew up in its own muzzle:
  `life 2` vs retail 3, position frozen at 82/180/3616 instead of
  retail's 84.57/181.94/3335, plus a phantom (10,12) possess flash at
  slot 123. `--csv` row diff: **5 rows removed, 0 added.**
- **mc2l24 51500+600: pair verdicts unchanged** (51 conforming / 549
  field-diff / 27 explained, both arms). At t=51500 a (9,3) stops
  self-detonating (slot 720 life/x/y/z all become retail's) and three
  phantom (10,0) impact puffs disappear; entity-set extras 284 → 281.
  Downstream the freed slots reshuffle the free list and the row detail
  wobbles (+8 field, +2 missing) in an epoch whose FIRST pair was
  already deeply divergent. Net entity mismatches 387 → 386.

### 2. OPEN-6 — the native fool's sphere wore the wrong model, and four gates were on the wrong side of it

`spawn_mana_ball` stamps `model65 = 39` for the whole MC2 sphere line,
so a natively-spawned (10,57) read model 39 where retail's `sub_50130`
builds a real model-57 entity. Now stamped
(`crates/mgc-sim/src/mc2/effects.rs`); the action-62 discriminator
stays as belt-and-braces. Full gate audit —
`docs/spell-audit/fools-mana.md` §7, table of eleven laws with cites.

The organising fact: retail's class-10 chain `dword_38523` is built
from models **39, 40 AND 57** (EF:40023-40062). Laws that walk it with
no model test include m57; laws that test `model == 39` exclude it; the
census is a third thing.

Port changes the stamp forced:

- **awake pass** (`mc2_awake_pass`, mc2/mobs.rs) — retail's sphere loop
  has no model test (EF:55489); 57 ADDED, else native fools stop waking.
- **mana-magnet aura** (`mc2_aura_tick`, mc2/tail.rs) — no model test
  either (EF:28362); 57 ADDED. (Model 40 rides retail's chain too and
  the port has never pulled it — pre-existing residual, noted in place,
  deliberately not changed here.)
- **world-mana census** (`recompute_mana`, engine/world.rs) — retail's
  MC2 census `sub_61F50` is a MODEL SWITCH: 39 and 58 count, 45 banks,
  **everything else falls through** (EF:62012-35). So (10,57) never
  enters the type-0 castle-share denominator — cast decoys AND authored
  ground spheres alike. The port's decoy-only special case is deleted;
  the match list is the filter. (§3 of the audit said authored spheres
  "keep counting exactly as they did" — that was wrong against retail.)
- **possess whitelist** (`claim_admits`, mc1/combat.rs) — the `(10,57)`
  arm read `f40`. Retail reads `parentId_0x28` (EF:3846), whose port
  home is `id24` (the importer's `owner28` fuse). Invisible while only
  IMPORTED spheres reached that arm (both lanes read 0); with the model
  native it would have let a caster's own possess bolt detonate on his
  own trap. Lane corrected.
- **castle absorb** (mc2/castle.rs) needed no edit but had a live bug
  the stamp fixes: retail filters `model != 39` (EF:61105), so a native
  fool's sphere touching a castle used to be eaten as real mana.
- **rival mana hunt** already walked 39/40 then 57 under the Perception
  break (EF:6544-49) — the native m57 was simply in the wrong pass.
- **map dot** (mgc-app/src/entities.rs) keeps (10,57) on the (10,39)
  arm: a decoy that looks different is not a decoy.

**Corpus: byte-identical** on both windows (proved by running the whole
change set with `MGC_NO_MUZZLE_ADMISSION=1` — output matched the
pre-change baseline exactly). Expected: `verify-deltas` rebuilds
entities from the recording, where an m57 already carried model 57, so
only NATIVE play is affected.

### 3. Rival hate decay — FROM-BINARY VERDICT: **SANE. remc2's shifted index is a decompiler artifact.**

remc2 EF:5377-93 writes `array_0x1FC_508[4·i+4]` from
`array_0x1FC_508[4·i]` — eight bytes lower, i.e.
`hate[p] = agg + 1 + hate[p−1]`, an accumulator that would leak hate
across pairs. Disassembled the shipped NETHERW.EXE (`sub_12A70`, linear
0x12A70 → file 0x37270 by the banked LE recipe `0x34800 + (linear −
0x10000)`; pristine copy at
`/home/rain/games/dosgames/carpet2/patched/netherw.exe.orig`):

```
12AE6  lea  ecx,[ecx*8+0x0]        ; 8·i
12AF6  lea  esi,[ecx+eax]          ; playerRec + 8·i
12AF9  mov  cx,[esi+0x204]         ; READ hate[i]
12B00  cmp  cx,0x601f
12B05  jnc  .above                 ; unsigned >= neutral
12B07  mov  ax,[eax+0x242]         ; aggression — per-PLAYER, NOT indexed
12B0E  inc  eax
12B0F  add  ecx,eax
12B11  mov  [esi+0x204],cx         ; WRITE hate[i]  ← SAME element
12B23  cmp  word [eax+0x204],0x601f / jna / mov 0x601F   ; clamp DOWN
.above:
12B54  cmp  word [esi+0x206],0x0   ; war flag → pin
12B5E  mov  ecx,0x100 / sub cx,[eax+0x242] / sub [esi+0x204],ax
12B85  cmp  word [eax+0x204],0x601f / jnc / mov 0x601F   ; clamp UP
```

Read and write are the same element; both compares are unsigned and
strict. remc2's `[4·i+4]` for hate and `[4·i+5]` for the war flag are
BOTH right (0x204+8i and 0x206+8i) — only the right-hand-side operand
is mistyped. `mc2_rival_hate_decay` (mc2/rivals.rs) already implements
exactly this. **No port change; annotated with the disassembly.**
`cargo test -p mgc-sim --test mc2_rivals`: 17 passed.

### 4. Vissuluth wake gate — NO-BUG, metric pinned

`Maths::EuclideanDistXYZ_58490` (Maths.cpp:738) is
`radix = (int16)(dx)² + (int16)(dy)²` and nothing else — **Z is never
read**, confirming the banked "the name lies" trap; and it is a true
2-D EUCLIDEAN, not Manhattan: the return is
`sub_7277A_radix_3d(radix)` (Maths.cpp:744), a Heron integer sqrt
seeded from `x_WORD_727B0[bsr]` terminating on `radix / i >= i` — an
exact FLOOR sqrt. So retail's `>= 0xA00` and the port's `>= 0xA00²` are
the same predicate, boundary included. `doomsday.rs` already had the
squared 2-D form: **the session-7 "already faithful" ruling stands.**
Comment now carries the derivation; arithmetic widened to i64 for the
same reason retail accumulates into a `uint32_t` (two i16 legs reach
2³¹ and the i32 form wrapped negative there). Last Vissuluth crumb —
closed.

### Suites

`MGC_REQUIRE_GOLDENS=1 cargo test -p mgc-sim --no-fail-fast`: **0
failures** (342 lib + all integration; the three fool's-mana channel
tests updated to count (10,57), which is the OPEN-6 pin). Workspace
minus mgc-conform: 0 failures. `cargo fmt --all --check` clean; clippy
warnings all pre-existing (probes.rs / roster.rs doc lists).

`mgc-conform fixtures conformance/*.json`: **0 regressions** on all six
manifests. The 9 FIXED + 9 drifted fixtures it reports are
**pre-existing, not from this dig**, on three independent grounds:
(a) re-running the identical command under `MGC_NO_MUZZLE_ADMISSION=1`
gives byte-identical output, so OPEN-7 moves no fixture; (b) five of
the nine fixes are MC1 (mc1l0 t=112/178, and mc1hwl0's drift), and
every remaining change in the bundle is MC2-only — `model65 = 57` and
`tick70 = 62` are stamped in exactly one place, `mc2_spawn_mana_sphere`,
so the `(10,57)` claim arm and the deleted census `tick70 == 62` test
are unreachable on the MC1 column; (c) the two mandated windows are
byte-identical to the pre-change baseline under that same toggle. The
manifests are stale relative to the uncommitted session-8 work.
**NOT promoted** — the promote decision is the orchestrator's.

## THE EYE LIFT — "docked at my castle I sit lower than retail" was a MISSING +128 IN THE CAMERA (player report 2026-08-05, FIXED)

**⭐ PLAYTEST CERTIFIED (player, 2026-08-05, same day): "Eye-height
playtest confirmed, it looks much better in all situations."**

**Symptom (player, native MC2 side-by-side vs retail):** parked on their
own castle the port puts the view CONSISTENTLY LOWER than retail, while
ambient creature/guard placement reads 1:1 exact. Three hypotheses were
offered; the corpus + both decompiles convict the third, in its
player-specific variant.

**RETAIL LAW, measured then read.** The corpus pins the sim half:
`mc2l0` parks the human at z **256** over sea-level ground for
t=5683..5758, and its spawn pose is z **5024** over a 149-byte cell
(149·32 + 256) — clearance **256**, exactly `sub_5D530`'s floor
`z = getTerrainAlt + word_160_0xc_12` (EF:59768). `mc1l0`'s spawn pose
is z **2080** over a 61-byte cell (61·32 + 128) — MC1 clearance
**128** (:55151). The castle mound is ordinary terrain under that law:
`mc2l0`'s human castle at tile (48,34) walks its z 1644 → **2336**
across t=2564..2583 as its (10,42) painter stamps the BUILD00 pad, and
holds there — 2336 = 73 height bytes × 32 IS the pad top (the castle
entity re-pins to live ground every tick, both games). `mc1l0`'s own
castle at (117,101) does the same over t=562..607: z 1022 → **2656** =
83 bytes × 32, then flat.

The half the port never had is the RENDER half: **retail hands its
world draw `axis.z + 128`, never the raw carpet z** — MC2
`DrawWorld_411A0` (remc2 EventsFunctions.cpp:21575, mirrored :21606 /
:21868 / :21899), MC1 `DrawWorld_30D90_30DD0` (remc1
sub_main.cpp:26406, :26589). Same literal, both games. The per-frame
view record is otherwise a verbatim copy of the entity position
(EF:40250-54 — it even calls `getTerrainAlt_10C40` and throws the
result away), and `array_0x52_82.fov` (the head clearance 100) never
touches the camera. So retail's docked eye over that mound is
`2336 + 256 + 128` = **2720**; the port rendered from **2592**, a flat
half-tile low — everywhere, but only judgeable where a structure of
known height stands next to you, which is why the castle dock is where
the player saw it.

**The other two hypotheses, refuted.**
- *Castle taller than its bounds* — NO. The visible castle is painted
  terrain in both engine and port; the only drawn (3,2) art is the
  owner's flag billboard, anchored at the entity z, which
  `castle_tick` re-pins to `ground_z` every tick
  (features.rs:3639, mc2/castle.rs:105-172). The terrain MESH samples
  the same plane at the same scale (`HEIGHT_SCALE` 1/8 =
  32/256, terrain.wgsl:121) with matching triangulation parity, so art
  and collision share one datum. Guards standing right is not a
  coincidence — there is no second datum for them to stand on.
- *Per-resolution perspective* — NO. `GameRenderOriginal` / `NG` / `HD`
  are constant-identical in every projection field (camera z, screen
  centre, focal `7·isqrt(W²+H²)·fov >> 11`, horizon `pitch·W >> 8`);
  NG/HD only parameterize fog and draw distance, and reproduce Original
  exactly at scale 1. There IS a real resolution effect, but it is
  ASPECT, not a constant: focal scales with the diagonal while the
  screen centre scales with H, so with the default `fov` 128
  (EF:38163) the vertical FOV is ≈62.3° at 320×200 (16:10) and ≈68.7°
  at any 4:3 hires mode. It rescales the whole picture, creatures
  included, so it cannot produce a player-only offset — and the port's
  fixed 60° sits within ~4% of retail lores. BANKED, not landed:
  deriving `FOV_Y` from retail's aspect formula would close the last
  few degrees.

**Landed (presentation layer, hash-quiet, zero sim law touched):**
`mgc_sim::EYE_LIFT = 128.0 / 256.0` (crates/mgc-sim/src/lib.rs:47-68,
carrying the citations), applied at the one live-gameplay camera —
`crates/mgc-app/src/lib.rs:5464-5480` (`y: carpet_y + EYE_LIFT`,
:5474). The
`Flyer` pose stays the CARPET plane deliberately: it round-trips
through `sync_carpet_from_flyer` and feeds the world its pose, so the
lift belongs to whoever builds the camera. The debug coordinate
overlay (lib.rs:6142) backs the lift out again so its floor/band
readout stays carpet-relative.

Test `docked_on_a_castle_pad_floors_and_lifts_the_eye`
(flight.rs) pins the whole chain on the measured mound: MC2 floors at
2336+256 and renders from 2720, MC1 at 2336+128 and renders from 2592,
with a non-vacuity clause on the 128 itself and on the two games
docking at different heights. No golden moved and no fixture is
reachable — the conformance harness pins the human pose from the
recording and compares sim fields only, so a camera constant is
invisible to it (no probe run, none applicable).
**PLAYTEST OWED:** docked on your MC2 castle the view should now sit
half a tile higher — same carpet, same mound, eye raised 128 engine
units; MC1 gets the identical lift (its carpet floors half as high, so
the change is proportionally more visible there).

## VISSULUTH'S SUMMONS — THE "FROZEN HUSK FOREVER" IS A PORT BUG: ONE `return` SKIPS RETAIL'S ESCAPE HATCH — **DIAGNOSED AND LANDED 2026-08-05, PLAYTEST OWED** (l24 windowed unexplained −208 / −245, `sv2` mismatches → 0, no golden moved)

Player report (sharpened 2026-08-05): on mc2l24, Vissuluth's summoned
creatures killed at a specific very early moment after flying out
remain behind as frozen husks — forever. Cyclone shoves them (physics
still applies) but they never move or die. Incidence dropped after the
session-8 latch-home fix; in the last playthrough it happened ONLY to
"fireflies". Player adds: **they have never seen this in retail**,
across substantial play of the level.

**VERDICT: (b) THE PORT DIVERGES.** The ledger's earlier reading —
"the standing husk is retail law" — was half the law. The latch write
is real, but retail has an escape hatch on the very next statement and
the port `return`s past it. The corpus contains **no standing-husk
specimen at all**: every doom summon that dies in the recorded take
dies in an ordinary model death state, never in the summon slot. The
player's "never seen it in retail" is exactly right.

### The latch lifecycle, both sides

Retail `sub_1E580` (EF:10689-10746) is the StageVar2 13/16 body; a2 =
`8 * model` (`sub_1D5D0` call sites, EF:11356/11454/11567/…). Its
target-valid branch is:

```c
sub_1E700(a1x, a2);                                    // EF:10734
if (!(a1x->byte_0x3E_62 & 7)) {                        // EF:10735
    v4 = a1x->dword_0xA0_160x->word_160_0x1c_28;       // EF:10737
    if (sub_583F0_distance_3d(&a1x->position, &v3x->position) < v4)
        a1x->actionIndex_0x45_69 = a2 + 2;             // EF:10739
}
```

`sub_1E700` (EF:10753-10871) is the shared damage head plus three
arms:

- **v2 == 0** (quiet) — move core `sub_1B8C0`, face target, crowd
  steer-away (EF:10806-40).
- **v2 == 1** (damaged, survived) — move core, then RETARGET: lock
  the attacker and hand to the model's `+2` (or `+6` for flee rows),
  plus the parent-XP mail (EF:10844-61). **This leaves the doom lane
  permanently** — `8m+2 & 7 == 2`, so the next tick dispatches to the
  model's own handler, whose head sends `life < 0` to `a2 + 4`, the
  ordinary death animation (EF:9008-9013 and its 30-odd twins).
- **v2 == 2** (dead) — `word_0x2E_46 = 1` (EF:10864-66). No state
  change, no move, **no early return.** Control falls back to
  EF:10735, so a DEAD husk still runs the engage check and still
  converts to `a2 + 2` the moment it is inside `v_28` of its target.

So retail's husk is a ≤ 8-tick freeze (the `byte_0x3E_62 & 7`
throttle) whenever the player is within the row's engage reach, and
the reach is enormous: `v_28` = 5120 (20 tiles) for m0 and m19, 4608
for m21, 3072 for m25 (behavior.rs rows 71/88/96/92). In a fight
fought inside that radius the husk is invisible; the creature simply
freezes for a beat and then plays its death animation.

The port (`crates/mgc-sim/src/mc2/mobs.rs`,
`mc2_doom_summon_home_tick`) mirrors all three arms — and then:

```rust
match self.mc2_state_head(i) {
    2 => {
        self.ent[i].f26 = 1;
        return;                      // mobs.rs:2598  ← THE BUG
    }
    ...
}
// The engage handoff (EF:10735-40) — mobs.rs:2659-2669, unreachable
// from the dead arm.
```

`f26 = 1` and "do not move" are both correct. The `return` is not in
retail. With it, a port husk can never leave `8m+7`, so it stands
until the parent scan (mobs.rs:2552-58) fails — i.e. until Vissuluth
himself dies (t=63221 in the corpus). That is the player's "forever".

### Corpus specimens (recordings/mc2l24.mgcr) — retail never husks

`f2e` in the trace output is `word_0x2E_46`, the latch; `flags` bit 10
(0x400) is retail's `byte[1] & 4` teardown bit.

- **slot 573, (5,0) worm.** Spin-up t=60104-60140 (`act=7`, speed
  312 → 24 at −8/tick), enters the home lane t=60141 at the m0 cruise
  30. **t=60142: first hit, 1600 damage, life 4000 → 2400, `act 7 →
  2` in the SAME tick** — the v2==1 retarget (EF:10855-60), one tick
  into the lane. t=60143 → 800, t=60144 → −800 and `act 4` (prekill),
  t=60145-49 `act 5` (kill), t=60150 flags 0x40c. Latch `f2e = 250`
  constant the whole time — never decremented, because StageVar2 16
  skips the `--` (EF:10703-06), exactly as the port models it.
- **slot 772, (5,19) firefly.** Home lane from ≈t=60138. **t=60153:
  `act 159 → 154` at FULL life (600/600)** — the engage handoff,
  15 ticks of exposure. Killed t=60193 by a single 1600 hit
  (600 → −1000) while in `act 154` → `act 156` (prekill) →
  `act 157` t=60194-60200 → torn down t=60201. A ONE-SHOT KILL, and
  still a full 8-tick death animation.
- **slot 820, (5,19) firefly.** Born t=60068, home lane ≈t=60105,
  **`act 159 → 154` at t=60161 at full life** (56 ticks of exposure),
  one-shot t=60192 → 156 → 157 → slot recycled t=60202.

Every one of them left `8m+7` alive-or-instantly, by either the
v2==1 retarget or the engage handoff. **The port's exposure window is
the home-lane dwell before the engage handoff fires: 15 and 56 ticks
in these two specimens (0.6-2.2 s).** A one-shot kill inside that
window strands the husk permanently in the port and costs retail
≤ 8 ticks.

### Why only fireflies — HP, not a lane home

Not a per-model field home; the four summon models share one lane and
the session-8 f26 fix covers all of them. It is max_life against the
player's 1600-damage fireball:

| pick | model | max_life | per burst | roll weight |
|------|-------|----------|-----------|-------------|
| 3 | (5,0) worm | 4000 | 3 | 10/70 |
| 4 | (5,21) | 1000 | 3 | 1/70 (roll 69 only) |
| 5 | (5,25) | 7500 | 3 | 10/70 |
| 6 | (5,19) firefly | **600** | **8** | 9/70 |

(`mc2_spawn_m*` in roster.rs/multipart.rs; repeats + weights from
`mc2_pyramid_pick_summon`, doomsday.rs:786-813.) m0 and m25 can never
be one-shot, so their first hit is always a v2==1 retarget that pulls
them out of the lane — specimen 573 is that, one tick in. m21 is
one-shottable but is the rarest pick. Fireflies are ≈96% of the
one-shottable summon population, and they arrive eight at a time.

### NOT a divergence: visibility. There is no hide lane to honour

Ruled out explicitly, because the reconciliation hypothesis was that
retail hides its husks. It does not. Retail's sprite pass gates only
on `byte[0] & 0x21` (`DrawSprites_3E360` GameRenderOriginal.cpp:3157,
mirrored NG:2838/HD:3235; gather `sub_3FD60` GRO:1936) — no life gate
anywhere. The corpus agrees: specimens 573/772/820 hold `flags=0xc`
(bit 0 CLEAR = drawn) through prekill and kill, and only gain 0x400
(`byte[1] & 4`) at teardown. The port draws exactly what retail draws
and `live_poses_mc2`'s class-5 `flags & 1` gate (world.rs:1662) is
already right. The floating red bar over the husk is the port's opt-in
`render.debug.health_bars` overlay, not a retail element. **The
divergence is DURATION, not visibility.**

### Second divergence, same family: the spin-up lane eats the first hit

`sub_1E320` (EF:10566-10604), StageVar2 17, calls **only** the move
core and then tests life directly:

```c
sub_1B8C0(a1x);                       // EF:10572 — move core, no
if (a1x->life_0x8 < 0) {              // EF:10573   damage intake
    DisableEntityDrawing04_57F10(a1x); return; }
```

Damage in MC2 reaches an entity solely through the accumulate-mailbox
(`dword_0x5E_94` += / `word_0x62_98` = src, EF:4023-25 and ~30 twins;
the port's `Gen::mail_write`, mc1/combat.rs:85-95, matches) and is
applied only by a state handler's head. So retail applies NOTHING
during the ~37-tick spin-up flight: a hit taken in flight is carried
into the home lane and consumed there, where it becomes either the
v2==1 retarget (the escape) or the husk.

The port's `mc2_doom_summon_spinup_tick` opens with
`if self.mc2_state_head(i) == 2 { flags |= 0x400; return; }`
(mobs.rs:2505-2508), which DRAINS the mailbox and applies the damage
in the spin-up lane. Two consequences: (a) a non-fatal in-flight hit
is swallowed, so the creature enters the home lane pristine and loses
the retarget that would have taken it out of the husk-prone lane on
tick 1; (b) a fatal in-flight hit makes it vanish outright — no death
animation, no puff — where retail would have flown it into the home
lane and given it the full `+4`/`+5` death.

### ~~Proposed fix shape — DEFERRED~~ → **LANDED 2026-08-05** (player go-ahead: "Let's fix this")

All three parts landed in `crates/mgc-sim/src/mc2/mobs.rs`, one
function each side of the summon chain:

1. **The `return` in the dead arm is gone**
   (`mc2_doom_summon_home_tick`, mobs.rs:2617-2619). `f26 = 1` stays,
   `mc2_move_core` still is NOT called (retail's v2==2 arm does
   neither), and the arm now falls through to the engage handoff at
   mobs.rs:2680-2688 exactly as EF:10864-66 → EF:10735-39 does. This
   is the whole player-visible fix.
2. **The spin-up no longer drains the mailbox**
   (`mc2_doom_summon_spinup_tick`, mobs.rs:2517-2521): move core, then
   a BARE `act_life < 0` test, mirroring EF:10572-76. The queued hit
   now survives the launch flight and is consumed by the home lane —
   non-fatal → the tick-1 `v2==1` retarget, fatal → the husk arm and
   thence the normal death animation.
3. **The drift latch read-back** (mobs.rs:2599-2611): the dead head
   stamps `f26 = 1` before the `-= 4`, so an unlocked dead summon
   lands on −3 and expires next tick (EF:10727-30) instead of draining
   its live 250 by 4s.

The `mc2_doom_summon_home_tick` doc comment carries the corrected law
(the dead arm's fall-through and the three corpus specimens); the
stale "a KILL leaves the corpse standing until the pyramid's death"
line in `mc2_pyramid_summons_release_fight_and_expire`'s header
(world.rs) is corrected to note that test sits its summon ~90 tiles
out, far beyond m21's `v_28` 4608, which is why its husk legitimately
stands.

**Tests** (world.rs, all four green, each proven non-vacuous by
neutering its own arm and watching only that test fail):

- `mc2_doom_husk_converts_to_the_death_handoff_in_reach` — a firefly
  one-shot (1600 vs 600) in `8m+7` six tiles from the player leaves
  the lane inside the 8-tick throttle and finishes `154 → 156 → 157 →
  reaped`. Neutered (`return` restored): *"the husk converts out of
  the summon lane inside the 8-tick throttle (it used to stand in
  8m+7 forever)"*.
- `mc2_doom_husk_out_of_reach_still_stands_at_latch_one` — the same
  kill 60 tiles out still stands at `tick70 = 159`, `f26 = 1`, in the
  pool. The non-vacuity PARTNER: it pins that the conversion is gated
  on EF:10738's `v_28` test and that the standing corpse itself is
  retail law, so the engage check cannot be made unconditional.
- `mc2_doom_spinup_keeps_the_hit_queued_for_the_home_lane` — a fatal
  in-flight hit leaves `act_life` untouched, the mailbox still
  charged and the summon still in the pool, then lands once the home
  lane owns it; the non-fatal twin (a worm) becomes the tick-1
  retarget to `8m+2`. Neutered: `left: -1000, right: 600` on *"the
  spin-up applies NO damage"*.
- `mc2_doom_unlocked_dead_summon_drains_to_minus_three` — `f26 == -3`.
  Neutered: `left: 246, right: -3` (246 = the old 250 − 4).

**Goldens: NOTHING MOVED — no re-pin.** `MGC_REQUIRE_GOLDENS=1 cargo
test -p mgc-sim --no-fail-fast` = 347 lib + every integration binary
green, 0 failed. As predicted: no golden world authors a (5,10), so
the doom-summon lanes are simply not reachable from them.

**Windowed A/B** (`verify-deltas recordings/mc2l24.mgcr`, pre-fix vs
post-fix release binaries, `ulimit -v 2000000`, one at a time). The
baseline was rebuilt from the SAME tree with only these three arms
reverted, so the comparison isolates this law and nothing else — it
reproduced the first baseline's CSV byte-identically in both windows:

| window | UNEXPLAINED field | rows fixed | rows NEW | rng | conforming |
|--------|-------------------|-----------|----------|-----|------------|
| 51500 +600 | 2247 → **2039** (−208) | 224 | **0** | 1/600 → 1/600 | 51 → 51 |
| 60000 +300 | 2625 → **2380** (−245) | 281 | **0** | 0/300 → 0/300 | 0 → 0 |

Missing/extra rows unchanged in both (1/137 and 1/27). **Every moved
row is a pyramid summon and the change introduces no new diff
anywhere:** the only (class,model) touched are **(5,25)** in the
first window and **(5,0) / (5,19) / (5,25)** in the second — the
fireflies the player named are the largest block there (slots 974,
959, 960, 908). Per-family, every count fell and none rose: w1 x
2042→2005, y 2035→1998, z 925→909, heading 824→787, speed 418→381,
life 182→126, action 66→64; w2 y 1632→1587, x 1631→1585, z
1209→1173, speed 612→566, heading 540→494, life 270→216, action
100→96. **`sv2` mismatches go to ZERO in both windows (2→0, 4→0)** —
the spin-up's early mailbox drain had been shifting the
StageVar2 17→16 handover a tick off retail, and dropping it puts the
handover tick back on retail's. The `life` family is the direct hit:
retail holds an in-flight hit QUEUED, so the port's early application
had every damaged summon reading a wrong life for the rest of its
spin-up and a wrong track thereafter.

**Fixture suite: no drift, nothing promoted.** `cargo test -p
mgc-conform` green — all 6 manifests, 203 fixtures, `0 regressions,
0 fixed, 0 drifted, 0 not reached` (mc2l24: 17/17 as expected). The
l24 manifest does not sample these summon ticks, so no fixture status
moved; **`--promote` was NOT run.** `cargo fmt --all --check` clean.

**PLAYTEST OWED — what the player should see:** a summon killed
instantly right after it flies out now freezes for **at most a third
of a second** (the 8-tick engage throttle, and only while you are
within its engage reach — 20 tiles for a firefly) and then plays its
normal death animation and disappears, instead of standing there for
the rest of the fight. Summons shot during their launch flight now
fly on and die properly instead of blinking out of existence. A
corpse left standing far from you is still retail-correct — fly
within ~20 tiles and it will drop.

---

## MC1/HW RIVAL REBOUND — the arm existed, the BIT was never published (2026-08-06)

**Player report (retail-observed, mc1hw:0):** the sole rival wizard, once
taken down by meteor, puts Rebound (spell 14) up and keeps re-upping it
for the rest of the level. The port's MC1/HW rivals had never been seen
casting it.

**Retail law (corroborated line-for-line in BOTH trees).**

1. *Trigger* — `sub_132B0` (remc1 :18024-34 / remc1hw :16156-66), inside
   the per-tick brain `sub_13170` (remc1 :17842 / hw :15974). Every
   decision tick (`ent+63 % (64 - tempo/4) == 0`): `sub_16800` picks the
   NEAREST class-9 whose `+146` (chase target) is my id, 3-D distance²
   under `0x1900000` (= 5120²), walking bucket[3] — the class-9 list
   rebuilt at :52277-84; a hit sets strafe 80 (`sub_16870`) and runs
   `sub_16890`. (The same block also self-heals: `if (+12 < +8)
   sub_155F0(a1, 1)`.)
2. *Reactive pick* — `sub_16890` (remc1 :19808-54 / remc1hw :17940-90),
   byte-identical between the trees. Inside `0x100000` (= 1024²), a
   three-way switch on the THREAT's `+65`:
   models {0, 3, 16} → `if (sub_15A00(a1,0xE)) sub_155F0(a1,0xE); else if
   (sub_15A00(a1,4)) sub_155F0(a1,4);` — a LADDER, Rebound first and
   Shield as the fallback; models {4, 9} → Shield only; **every other
   model casts NOTHING** (1/2 fall out of the `< 4` branch, 5..8 out of
   the `>= 9` branch, `!= 16` returns outright).
3. *Readiness* — `sub_15A00` case 4/0xC/0xE (remc1 :19289-99 / hw
   :17422-34): the manifestation must exist, its burst `+48` must be
   **ZERO** (no re-cast while the buff runs), the AI cooldown
   `+724[2*s]` must be zero, and wizard mana `+140 >= +136`.
4. *Commit* — `sub_155F0` case 1/4/5/0xE (remc1 :19140-48 / hw
   :17271-81): `+48 = +50` (= `count` = 101 for Rebound) and
   `+724[2*s] = word_90034[s]` (= 1). No projectile, and **no castle
   check at cast time**.
5. *Token tick + THE BIT* — class-12 handler `0x2A` (= 3 × spell 14) of
   `str_2563D8` (:4996) is `sub_573F0_57920` (remc1 :65774 / remc1hw
   :61996): while `+48 > 0` run `sub_55DD0_56300` (remc1 :64910 — the
   stored-mana ladder: owner mana/life >= 0 and, since the ctor
   `sub_3C210` :48080 = `sub_3BF70(a1, 14, 42, 1000, 101, 1, 0, 8000,
   100)` sets `+132` = 8000, the owner's castle entity `+140` must hold
   >= 8000), then **`owner->+17 |= 0x80`** (our `flags & 0x8000`) and
   `sub_55E80` (regen pin); on failure `+48 = 1` (dies next decrement,
   buzz 29). `+48 <= 0` → `owner->+17 &= ~0x80`. Then `--+48`.
6. *The effect* — that bit is what the projectile-vs-victim step reads
   to deflect (`proj_move_and_hit`, remc1 :62848-90) — already ported,
   already reading `ent[j].flags & 0x8000` for pool victims.

**Corpus proof (`recordings/mc1hwl0.mgcr`, 50,150 ticks).** The retail
rival is slot 473 (class 3 model 1). Its Rebound bit is directly
observable in the obs `flags` lane, and the take contains **seven**
windows, each exactly `count` = 101 ticks, ON→OFF at
5591→5692, 7578→7679, 8506→8607, 12247→**12550**, 13442→13543,
17858→17959, 21372→21473. The 12247 window is 303 ticks = 3 × 101: the
rival re-upped twice, so "permanent" is a re-up loop, not a long token.
The port missed the ON edge of all seven and the OFF edge of six — it
never wrote the bit at all.

**Root cause: the arm was there; the PUBLISH was not.** `rival_defense`
already ported `sub_16800/70/90` and, driven against the corpus, fires
correctly (instrumented windowed replay at 20800..21500 caught the port
casting spell 14 at t=21371 with `owned=484, mana=24906, castle_stored=
13768` — one tick off retail's t≈21372, inside the take's own class-9
divergence). But `rival_refresh_buffs` mirrored ONLY the invisibility
cloak (0x20) onto the wizard entity; the Rebound token's `f26` never
reached `flags & 0x8000`, so `class12_tick` (which deliberately skips
rival-owned manifestations) plus the missing mirror meant NOTHING could
ever bounce off an AI wizard. `docs/DEVIATIONS.md` already flagged the
symptom for MC2 ("rival Rebound windows are not yet mirrored onto their
entities"); MC1/HW had the same hole.

**Fixed (all in `crates/mgc-sim/src/mc1/rivals.rs`).**
- `rival_refresh_buffs` publishes the token: `flags |= 0x8000` while the
  burst is live, `&= !0x8000` when it lapses (`sub_573F0_57920`). The
  wizard's death transition drops the bit too — retail's token keeps
  ticking through death states 2/3, which the port's rival lanes never
  reach, so without it a corpse would deflect forever.
- `rival_cast_ready` gains retail's ALREADY-ACTIVE gate (`+48 != 0` →
  not ready) for the self-buff group whose burst the port actually
  decrements for rivals: 2/4/12/14. Before it, `AI_RECAST[14] = 1` let
  the port re-cast Rebound every other tick for as long as the trigger
  held, paying 1000 mana each time — the instrumented window shows the
  port firing 3× in 12 ticks where retail fired once. Retail applies the
  same gate to the aimed group (3/7/8/17/20, :19265-68) and to Castle
  (:19305); **BANKED**, because the port never runs a per-tick countdown
  on a rival's OFFENSIVE manifestations, so gating them would freeze the
  picker after one shot.
- `rival_defense` is now verbatim `sub_16890`: the default arm casts
  nothing (was `_ => 4`, burning Shield + 2000 mana on threats retail
  ignores) and the fire-spell arm is the Rebound→Shield LADDER (which
  matters precisely because the new readiness gate makes Rebound unready
  for the 100 ticks after it arms).
- New `rival_token(ri, spell)` validates the `owned[]` binding
  (class 12 / matching model / `tick70 >= MANIFEST_BASE` / `f144` = the
  rival / not removed) before any burst lane touches it.

**⚠ NEW FINDING — the rival spell book is NOT re-anchored at import, so
no MC1 rival cast/buff lane is conformance-measurable.**
`import_state` (conformance.rs :324-350) rebinds a rival's entity, mana,
flight and brain lanes but NOT `owned[]`/`known[]`: the port's book still
points at the slots the FRESH world minted, which the import has since
overwritten with unrelated entities. Retail's token carries its owner in
`+42`, a field the port's `Ent` does not model, so the importer *cannot*
re-anchor today. Consequences: (a) the pre-existing code was decrementing
`f26` on whatever entity happened to occupy a stale slot — `rival_token`
stops that; (b) publishing the Rebound bit from a stale token actively
DESTROYED retail's own imported bit (measured: +100 `flags` rows on slot
473 per Rebound window, mc1hwl0 window 5500/+400 went 828 → 926
unexplained field rows), which is why the mirror is gated on the port
actually driving the token. Fixing this properly needs `Ent` to carry the
manifestation owner (+42) — a hashed-field change. **Banked.**

**Numbers.** Windowed A/B on `mc1hwl0` (`--input-delay 2`), before vs
after, identical in every window: 5500/+400 → 828 field / 25 missing / 73
extra both ways; 7500/+400 → 5533 / 117 / 55 both ways; 21300/+400 →
16310 / 148 / 201 both ways. **Conformance-neutral by construction** —
the rival token lanes are inert under a delta-verify import (see above),
so the corpus validates the RETAIL law and the free-run arm, not the
port's own casts. Full take after: 7,464 conforming / 49,765
fixture-grade, 800,222 unexplained field rows (the drop from the 2.03 M
baseline is the concurrent (10,13) smoke-puff landing, not this change).

**Goldens.** `level_005_golden_state_hashes` moves — attributed by
per-sub-change toggle to the ALREADY-ACTIVE gate alone (the mirror and
the reactive-arm corrections leave it pinned). Level 005's rival casts
Shield/Accelerate, and retail refuses those re-casts while the burst
runs. Behavior change toward retail by design. NOTE: the pin now on disk
was refreshed by the concurrent (10,13) smoke-puff work from a tree that
already contained this change, so the recorded value covers both; with
this change alone reverted the test fails.

**Tests.** `rival_rebound_arms_publishes_expires_and_re_ups` (arm →
publish → 101-tick countdown with no re-arm → expire → clear → re-up on
a fresh threat) and `unlisted_threat_models_provoke_no_reactive_cast`
(model 2 provokes nothing; model 9 controls that the Shield arm is live).
Both NON-VACUOUS — verified by temporarily disabling each sub-fix behind
an env toggle: the mirror off fails "did not publish the 0x8000
deflection bit", the gate off fails "the live Rebound token was re-armed
(100 -> 101)", the `_ => 4` fallback restored fails "a model-2 threat
provoked the Shield cast retail ignores".

**PLAYTEST OWED — what the player should see:** fireballs and meteors
now BOUNCE off a rival wizard (with the deflection twang, sound 28) for
about four seconds after he takes fire at close range, and the deflected
bolt turns around and homes on whoever fired it. A rival needs a castle
storing 8000+ for it — a poor or homeless rival still eats everything.
And the rival no longer stands there re-casting Rebound into itself:
once it is up, a second incoming bolt makes him raise Shield instead.

## LIGHTNING STORM + THE (10,13) SMOKE PUFF — the two biggest mc1hw families, both LANDED 2026-08-06

Player report (retail-observed): *"Lightning Storm is the safest way to
kill griffin flocks in mc1"*, but in the port *"it barely kills one and
the rest kill you"*, with the bolts visibly sitting **just above** the
creatures. Dig target also covered the take's `(9,9)` / `(10,13)`
families and the 33 rng over-draw pairs.

### 1. FAMILY SPLIT — what those rows actually are

- **`(10,13)` is NOT storm and NOT weather.** It is the class-10
  **RISING SMOKE PUFF**: ctor `sub_3AAA0` (`str_255D0C[13]`, remc1
  :46817 / remc1hw :44xxx — identical), tick `sub_257B0` (:28443,
  remc1hw :26987 — identical). Life `rand%23+17`, rise speed
  `rand%53+51`, sprite 67, filter pair `(10,13)`, `+18` bit1. The old
  ledger note "(10,13) 9.1k missing (meteor showers)" was a mis-ID —
  **retire it**. Exactly TWO creators in the whole binary, both
  unported:
  - the STANDING FIRE's exhaust (`sub_252D0` :28224, remc1hw :26774):
    inside the shrink window (`life < 12 && +26 > 0`) and with `+16`
    bit7 clear, one entity-LCG draw and on `d % 7 == 0` a puff at the
    fire with `+26 = 100` (which parks it past the tick's 16-tick
    drift window → fire smoke rises dead straight), `life = 15`
    (`max_life` keeps the ctor roll), the fire's owner, sprite `+2`;
  - the VOLCANO PLUME's ring spray (`sub_26140` :28834 = class-10
    state 19, remc1hw :27419) — see §3.
- **`(9,9)` is the lightning family and nothing else** — the m9 BEAM
  (state 9) plus its born-dead state-14 SEGMENTS. In mc1hwl0 it is
  driven by the storm windows (the `(9,12)` carrier fires at
  t≈21976 / 26669 / 34573 / 41015 / 41338 / 42212) and by kraken/creature
  beams elsewhere. The balanced missing/extra is segment churn, as the
  standing law says — count RECORDS.

### 2. THE STORM LAW, line-cited

`sub_26D20` (:29279 / remc1hw :27823 — **byte-identical**), class-10
state 40, reached from the `(9,12)` carrier's `sub_53DC0` (:63628),
which stamps owner / `+30` / `+32` / `+44` / `+146` / `+68=9` / `+69=9`
onto the `(10,38)` cloud (:63767-83):

1. `v2 = ground_z(pos)`. `z < v2+1024` → `z += 64`, **skip the tick**.
   `z > v2+1024` → `z = v2+1024`, **skip the tick too** (`HIBYTE(v2)+=4`
   is `+1024`). The port fired on the clamp-DOWN tick — fixed.
2. Pre-decrement life (33 firing ticks from the ctor's 32).
3. ONE entity-LCG draw → `+30 = d & 0x7FF`; `+32 = 56` **fixed pitch**.
4. Twice: `+30 ^= 0x400` (180° flip) then create `(+68, +69)` =
   `(9, 9)` **AT THE CLOUD'S OWN POSITION**. Retail builds a
   `z + <+78>` point in the shared temp `word_AE454_AE444` and then
   passes `(axis_3d*)(a1+72)` — **the `+78` lift is a DEAD STORE**. The
   port was adding it, laying every bolt a sprite half-height high.
   Child gets owner, `life /= 3`, `+30`/`+32` (NOT `+34`/`+36` — retail
   leaves those at NewEvent 0 for the beam's own acquire to fill),
   `+68=10`, `+69=23`, `+44` = the storm damage.
5. Thunder 23, positioned at the LAST bolt (the port plays it at the
   cloud — presentation, left alone).

Dropping the launcher's `+34`/`+36` pre-seed exposed a second missing
store: `sub_534C0`'s acquire gate (:63232-40) copies `+30`/`+32` into
`+34`/`+36` on a MISSED acquire, and the port only ever wrote them on a
HIT. The storm's pre-seed had been standing in for that copy. The miss
branch now lives in `proj_m9_tick` where the original puts it — worth
−93 / −155 unexplained field rows in the two storm windows on its own,
and no golden moves.

**Reach arithmetic: the storm CANNOT hit anything on its own.** The
`(9,9)` ctor (`sub_39EC0` :46135) is speed 384 / life 9, so a storm bolt
gets `9/3 = 3` → **4 steps of 384 = 1536 units** of travel, fired at
pitch 56 (≈9.8°) from **1024 above the terrain**. It descends 262 units
over its whole flight. Every single storm kill therefore comes from the
BEAM'S ACQUIRE snapping it down onto a victim.

**⭐ THE BUG IS ONE CONSTANT.** `sub_54520` (:63943 / remc1hw :60053)
switches on `+65`, and **case 9 is its own lane** (:64125 / remc1hw
:60256, identical):

- the wizard/castle list: cones `(0x71, 0x71)`, range gate
  `+128 * +8` (speed × max_life = the beam's own reach), 2-D distance;
- the **CREATURE buckets: `sub_54A90(a1, ii, 0x71u, 0x200u)`** — yaw
  ±0x71 as usual, **pitch ±0x200 = ±90°**, no range gate beyond the
  scorer's 3-D 5120.

No other subtype does this (cases 0/3/4 and 1 are `0x71/0x71`; HW's
meteor case 0x10 is `0x100/0x71` on BOTH lists). The port ran one shared
`0x71/0x71` cone for every subtype, so a flock 1024 below the cloud sat
outside the pitch wedge: nothing locked, the bolt held pitch 56, and it
sailed out level — **"the bolts hover just above the monsters"**,
exactly as reported. Restoring `0x200` on the creature lane makes the
storm the flock-killer it is remembered as.

### 3. THE PLUME (class-10 state 19) WAS UNTRACED — now transcribed

`sub_26140` (:28834): pre-decrement life; `+26 = 0`; walk the radius-0
ring (`sub_11410(0, +26)` → the 2×2 recentre block, 3 cells after the
iterator's dropped-last quirk); per cell one draw for the spreader's
~50% skip test (`d % 0x9D >= 79`) and, on a pass, the ±64 jitter pair;
then on ODD post-decrement life ticks each passing cell emits **four**
`(10,13)` puffs at yaws `{v, v+0x200, v+0x400, v+0x600}` with
`v = ((life/2) & 1) << 8` — the column corkscrews. Re-seat on ground. NO
animation step (sprite 228 is static). The port's stub drew **zero**
entity-LCG values a tick where retail draws **3 / 5 / 7 / 9** — that is
the entire `(10,19)` `rand` column in mc1hwl0 (3,822 rows on slot 767).

**OPEN (banked, deliberately NOT ported):** `sub_26140` closes with an
UNCONDITIONAL `sub_120B0(a1, 0, +44)` — 200 ch0 per tick over the
ctor's 512 extents for the plume's whole 240-tick life, i.e. the volcano
plume is a persistent damage field. No corpus take erupts a volcano, so
this is left for a measured eruption window rather than guessed in.

### 4. THE 33 rng OVER-DRAW PAIRS — DIAGNOSED, fix is OUTSIDE this lane

Located by windowed bisection: they are **contiguous runs of ticks
around the human's death**, e.g. t=21468..21481 (14 pairs) and
t≈6xxx / 18xxx. On MC1 the port has exactly one site that can step the
GLOBAL LCG more than once a tick: `player_land` (world.rs :3122-24),
three draws per owned spell for the jar scatter.

**Root cause is the conformance IMPORT, not a sim arm.**
`import_state` (`crates/mgc-sim/src/engine/world/conformance.rs` :392)
reads the human's life state from **`carpet.f66`**:
```
state: match carpet.f66 { 2 => Falling, 3 => Dead, _ => Alive }
```
Retail keeps the wizard's life state in **`+70`** (`sub_46B00`'s death
tail writes `*(_BYTE*)(a1+70) = 3` at :55550). Measured on mc1hwl0 slot
472: `f66 = 255` at t=100, 21400, 21470 and 21600 — *always* — while
`f70` is 0 alive and 3 during the dead-waiting window. So a dead retail
player imports as **Alive with a stale negative life**; the next damage
mail re-runs the whole death (`life < 0` → `Falling` → the same tick's
landing test → `player_land`) on EVERY imported tick of the window.

**Measured probe** (applied, measured, reverted — the file is the
harness-side import and outside this dig's territory): switching that
match to `carpet.f70` on window 21440/+80 takes rng **14 → 1**
mismatched pairs, unexplained field rows **5,385 → 4,775** and extra
**36 → 11**, with all 203 fixtures still `as expected`. **Handed to the
lead to land deliberately.**

### 5. NUMBERS

Full take `mc1hwl0.mgcr --input-delay 2`, before → after:

| | before | after |
|---|---|---|
| conforming | 7,355 | **7,464** |
| UNEXPLAINED field rows | 2,030,967 | **799,629** (−60.6 %) |
| UNEXPLAINED missing | 32,097 | **17,142** (−46.6 %) |
| UNEXPLAINED extra | 14,210 | 18,117 |
| `(10,13)` missing / extra | 10,135 / 0 | **422 / 1,782** |
| `(10,6)` missing / extra | 3,582 / 875 | **501 / 2,882** |
| `(9,9)` missing / extra | 12,017 / 11,344 | 11,785 / 11,106 |
| phase-clock disagreements | 1,844 | **898** ((10,13,13) 1,277 → 370) |
| `mc1hwl0-terrain-z` rule rows | 609,374 | **159,071** |
| rng mismatched pairs | 33 | 33 (see §4) |

`(10,13)` alone carried **1,752,673** of the take's diff rows — 86 % of
the whole field-row mass — because the state had no tick arm at all:
every imported puff fell through world.rs's class-10 catch-all (the
terrain-feature dispatch) and self-killed one tick after import
(`flags` `0x20004` → `0x20404`, `life` +1, `z` −64 on every row).

Windowed A/B, before → after (unexplained field / missing / extra):
- storm window **41000/+120**: 40,450 / 183 / 82 → **19,018 / 6 / 161**
- storm window **26690/+120**: 22,189 / 156 / 311 → **20,170 / 39 / 52**
- storm window **41340/+120**: 5,428 / 4 / 5 → **5,079 / 4 / 5**

`mc1l0` (regression guard): 8,834 / 126 / 98 → **8,520 / 119 / 99**,
rng 0/7097, no new families. All six suites `as expected`
(29 + 68 + 41 + 24 + 17 + 24 = 203, 0 regressions, 0 drifted).

### 6. GOLDENS + TESTS

`level_005_golden_state_hashes` B–E re-pinned (post-init and A hold —
nothing burns before B): every burning tree and crater now emits smoke
entities, so pool population and free-list order move. **Behavior change
toward retail by design.** The layout-independent OBSERVABLE companion
golden **held**. NOTE: the pin on disk was taken from a tree that also
contained the concurrent MC1/HW rival-Rebound work, so the recorded
value covers both changes.

New NON-VACUOUS tests (`engine::world::tests`):
- `storm_bolt_locks_the_flock_below_and_strikes_at_creature_z` — asserts
  in-fixture that the target's pitch delta is `> 0x71` (so the shared
  cone provably cannot see it) and `<= 0x200`, then that the bolt locks,
  that the acquire SNAPS `+30/+32` onto the pick, that the beam ends at
  flock altitude instead of ~760 above the deck, and that the `(10,23)`
  endpoint flash carries and delivers the storm's 2000. Verified by
  temporarily restoring the `0x71` creature cone: fails with
  "the storm bolt must lock the creature under it — left 0, right 2".
- `burning_fire_emits_rising_smoke_that_survives_its_own_tick` — the
  fire's 1-in-7 exhaust, `+26 = 100` / `life = 15` / owner inheritance,
  the `[64,128]` speed decay, the rise, the absence of lateral drift,
  pre-decrement life, and expiry on its own clock (it used to be flagged
  dead on the spot).

### 7. PLAYTEST OWED — what the player should see

Lightning Storm should once again be **the safest way to clear a griffin
flock**: the cloud parks 1024 up and its bolts now *snap down* onto the
birds under it instead of drifting overhead, two strikes a tick at
2000 apiece. The same constant governs every m9 beam, so kraken beams
also stop skimming past low targets. Separately, burning trees, fire
walls and craters now trail rising smoke, and volcano plumes churn out
a corkscrewing smoke column instead of standing there inert.

## THE MC2 HUMAN DEATH LAW — the corpse, the token scatter and the SPACE reset, LANDED 2026-08-06 (mc2l3 both reset pairs now conform outright)

The mc2l3 dig. The take carries **two human deaths** (t≈15280 and
t≈20560) and the port failed both: it kept the corpse alive (regening
life AND applying the frozen `manaRegen`, and clamping the corpse's
mana to `mana_max`), never respawned, and never re-minted the
spellbook — the 26 class-15 tokens retail creates at the reset were
the take's whole `(15,*)` missing census. Everything below is
`EventsFunctions.cpp` (`EF:`) unless marked.

### 1. THE STATE MACHINE IS THE RIVALS' — THE FORK IS INSIDE ACTION 3

The human wizard is a class-3 pool entity running the SAME action
machine as the AI wizards. There is no separate human death path; the
AI/human fork sits *inside* the action-3 body.

| action | body | what it does |
|---|---|---|
| 0 | `AddPlayer03_00_5E010` (EF:60040-44) | `life < 0` → action = 2, `word_0x2C_44` = 0, sound 16, DEAD scene |
| 2 | `sub_5E310` (EF:60074-99) | mover, then z-only gravity; floor = ground + row clearance; one `(10,1)` puff/tick; EXACT floor contact = the payout |
| — | the payout (EF:60101-77) | `sub_49F90`, kill credit, mailbox wipe, "has died.", the 26-token SCATTER, the `(10,40)` grave + sphere re-point, action = 3, `dword_0x10_16` = 1200, hide |
| 3 | `sub_5E7C0` (EF:60254) | **AI**: count 1200 down → castle respawn / banish. **HUMAN** (EF:60303-05): `sub_5C800(7)` + `sub_5E6C0` — face the killer at ≤22/tick, pin z to the ground, wait for SPACE forever |
| reset | `sub_5C950` (EF:43630) | PlayerAction 0xF, accepted by PI:1102 only while `life < 0 && action == 3` |

Two things the corpse does NOT do, both because the whole regen block
in the action-0 body is inside `if (life >= 0)` (EF:59996-60033):
it never heals and it never touches `mana`/`manaRegen`. mc2l3 holds
life at **−3060** and mana at **16,957** for the entire 15-tick corpse
window, and `mana` stays 95,951 at death 2 even though `mana_max` is
2,559 — the corpse never runs the clamp either.

Measured constants: floor − ground = **256** = the carpet tuning row's
`word_160_0xc_12` (`Mc2Row::CAVE`/`OPEN` clearance, both 256) — mc2l3
t=15300 lands at z 2125 over ground 1869. The corpse yaw walks
585 → 563 → 541 → 519 → 497 → 482 in exact 22s, then wobbles ±3 as
the killer moves: `sub_58350(yaw, bearing, 5, 0x16)`, cap 22.

### 2. THE SCATTER KEEPS THE BOOK — `SpellEnabled[i] = 1`, NOT 0

`sub_5E310` EF:60137-62 walks all 26 book slots. A live manifestation
is detached in place — `byte[0] &= ~1`, `actionIndex++` (3M → 3M+1,
the loose-pickup state), position ±256 around the corpse, life
`rand%90 + 200` — and **the book entry becomes a boolean 1**. That
marker is the wizard's whole memory of what he knew; the reset
re-mints exactly the non-zero entries. mc2l3 t=15300 shows all 26
tokens (slots 168-204) flipping to state 3M+1 with lives in [200,289]
scattered over the death tile, and both hands (`SpellIndexLeft/Right`
= 0/1) unchanged across both deaths — the scatter never touches them.

The three draws per token come off the DYING WIZARD's private LCG
(`a1x->rand_0x14_20`), never the world stream and never the token's
own `rand` (mc2l3 keeps all 26 token seeds at their allocation values
through the scatter).

### 3. THE RESET — WHAT `sub_5C950` WRITES, AND WHY THE MANA RESIDUE IS 750

For a human (`IsAiPlayer != 1`, so `actionIndex = 0` and the
fresh-join block EF:43744-822 is skipped), in order:

1. `sub_49F90` — **both** stacks rebuilt (descending 999→1 scan).
2. position ← the own castle's FULL position (x, y **and z**);
   castle-less single player = the level's authored start point at
   ground + 0x100, plus `byte[2] |= 0xC` (lost + level-over, PI's
   own flags) — retail respawns you anyway.
3. grace (`word_0x159_345`) = 100, wanted = 0, `dword_0x16D_365` =
   2000, commanded speed/yaw/fov/boost/strafe = 0, life-scale = 256.
4. **`maxMana` = 1000, `maxLife` = 10000.**
5. `sub_5CF40` (EF:59374) — the token RE-MINT (see §4).
6. **`life = maxLife`; `mana = maxMana`; and the mana-census BASE
   `byte_0x150_336` = maxMana.**
7. every other wizard's hate toward this colour → −24609; the AI
   target lanes cleared; the recycle stack emptied
   (`dword_0x11e6 = −1`, EF:43857).

`maxMana = 1000` is invisible one tick later: `sub_60F00` (EF:61976)
recomputes every wizard's `maxMana` from `byte_0x150_336` plus the
`mana` of every entity they own (castles, balloons, creatures,
`(10,39)` spheres) **before the entity pass**, which is why mc2l3's
`mana_max` reads 82,157 straight across the reset while `mana` lands
on 750. (It is also why `mana_max` collapsed 82,157 → 2,559 between
the deaths: the castle's stored mana went with the castle.)

**THE 750/2000 RESIDUE IS THE FRAME ORDER, NOT A PENALTY.** The reset
runs in `PlayerEvents` — ahead of the census, ahead of the entity pass
and (measured) ahead of even the frame's one unconditional LCG draw —
so the same frame's wizard body applies the `manaRegen` the corpse had
frozen:

```
mana(reset) = 1000 + manaRegen_held
  mc2l3 death 1: 1000 + (−250) =  750   (a cast debit was pending)
  mc2l3 death 2: 1000 + 1000   = 2000   (at-castle regen floor)
```

mc2l24's 14 human deaths confirm it on a second take: resets land on
1000 (delta 0, regen suppressed by a live cast), 1100 (the afield
floor 100), **1295** = 1000 + 590060/2000, **1310** = 1000 +
621060/2000 and 0 (a −1000 debit pending). The `maxMana/2000` afield
rate reproduces the odd residues exactly.

The RNG dating: retail's 26 fresh tokens are seeded `slot + rand` off
**29,590**, which is the recorded global LCG at t=15314 *unadvanced* —
so the reset allocates before the frame's own draw. That single fact
places the whole reset in the input phase.

### 4. `sub_5CF40` — the re-mint, and why the slots are 99, 100, 102…

For every non-zero book entry, spawn a fresh class-15 token at the
wizard's (already moved) position, `parentId` = the wizard,
`byte[0] |= 1`, then re-apply `SetSpell_6D5E0` at the stored tier. An
allocation failure zeroes that book entry — retail's own silent loss.

Because `sub_49F90` rebuilt the free stack first, the mint takes the
LOWEST free slots in ascending spell order: mc2l3 t=15315 puts spell 0
at slot **99**, spell 1 at 100, spell 2 at 102 (101 is live), … spell
25 at 133, all at the castle's exact x/y/z, with the same
cost/upkeep payloads as the jars they replace. The same rebuild is why
the graves land on slots **3** and **1**.

The OLD scattered jars stay out in the world, still collectible — a
death therefore doubles the class-15 census for a few hundred ticks.

### 5. WHAT LANDED (port)

- `retail_import_mc2`: `action45` 2/3 → `LifeState::Falling`/`Dead`
  (it pinned `Alive` unconditionally, which is what let the corpse
  regen and clamp).
- `World::tick`: the MC2 reset is processed FIRST, ahead of the frame
  draw, the census and the wizard body — the ordering IS the residue
  law. `MGC_NO_MC2_DEATH=1` restores the pre-dig behaviour for A/B.
- the mana step + delta recompute are gated off for an MC2 corpse.
- `mc2_player_fall` / `mc2_player_land` / `mc2_player_dead_wait` /
  `mc2_player_respawn` / `mc2_remint_book` — the human column;
  `Gen::mc2_rebuild_free` is `sub_49F90`'s free half.
- `mc2_scatter_spells` was written but never wired (and zeroed the
  book, which would have made every death a permanent wipe); it now
  leaves the boolean marker, keeps the hands, drops the jars at the
  corpse's z and does NOT mutate the token's `rand`.
- the book-driven manifestation loops now test the OWNED action state
  (`tick70 == 3M`) — retail's own dispatch condition, which also keeps
  the boolean marker from aliasing a real slot-1 manifestation.
- harness: the recorded SPACE key (scancode 57) is decoded into
  `PlayerCommand::respawn`. Dating it needs a witness — the key
  registers carry no press latch and mc2l3 shows both sides of
  retail's poll (SPACE first at record 15314 with the reset in frame
  15315; at record 20612 with the reset in frame 20612) — so the rule
  is `space(end) && (space(start) || recentred(end))`, where
  "recentred" is the cursor jumping to a point that equals the
  press-position snapshot: retail's 0xF handler runs
  `SetCenterScreenForFlyAssistant_6EDB0` (EF:37653). Over the whole
  take 348 records carry the recentre shape and exactly two of them
  also have SPACE down — the two reset frames.

### 6. THE `(10,2)` FAMILY — the Speed spell's slipstream, never ported

175 missing rows, first at t=15499, right after respawn 1: an
owner-stamped `(10,2)` appearing every 4th tick along the boosted
flight path, life counting 31 → −2. It is `GetScroll_69DB0`'s
contrail (EF:56251-59): on every live tick of the Speed window with
`byte_0x3E_62 & 3 == 0` — the TOKEN's phase byte, not the caster's —
spawn `NewAdd0A02_4E430` at the caster and **quadruple** the ctor's
life (8 → 32). The ctor is four writes: maxLife/life 8, action 2,
`dword_0x10_16` = 0, flags masked to `byte[0]|1`, `byte[0]&~8`,
`byte[2]|2` (= the recorded 0x20001) — no sprite, and deliberately NO
map link (it assigns `position_0x4C_76` instead of calling
`AddEventToMap_57D70`), which is why the trail hangs where the carpet
was. The MC1 twin of the same puff is documented in
docs/traces/mc1-class12-spell-tokens.md.

### 7. NUMBERS (windowed A/B, `MGC_NO_MC2_DEATH=1` = before)

unexplained field / missing / extra:

| window | before | after |
|---|---|---|
| mc2l3 **15255 +70** (death 1: fall, corpse, reset) | 375 / 28 / 7 | **215 / 2 / 7** |
| mc2l3 **20595 +30** (death 2 reset) | 98 / 26 / 0 | **30 / 0 / 0** |
| mc2l24 **2580 +40** (death 1) | 226 / 18 / 2 | 233 / **2** / 2 |
| mc2l24 **11205 +45** (death 5) | 610 / 22 / 4 | 610 / **0** / 4 |

- mc2l3 death-1 reset pair **15314→15315: 0 unexplained rows and the
  rng matched** — life 10000, mana 750, all 26 tokens at retail's own
  slots with retail's own seeds. (The rng needed one more thing: the
  importer arms `mc2_carpet_stall` off the corpse's action 3, and the
  reset must clear it — the resurrected carpet runs the mover, and
  with it the cave tail's draw, that same frame.)
- mc2l3 death-2 reset pair **20611→20612**: life/mana/tokens all
  conform; the window's residue is the pre-existing grave-census
  family (`slot 1 mana_max`, 12/12 pairs, present before the reset).
Full take (`verify-deltas recordings/mc2l3.mgcr`, 27,489 pairs):

| | before | after |
|---|---|---|
| conforming | 21,534 | **21,597** |
| conforming + explained | 21,865 | **21,947** |
| UNEXPLAINED field | 28,450 | **27,650** |
| UNEXPLAINED missing | 440 | **209** |
| UNEXPLAINED extra | 144 | 145 |
| entity sets missing / extra | 559 / 304 | **328** / 305 |
| rng mismatched pairs | 2 | **0** |
| `(15,*)` reset rows | 52 / 0 | **0 / 0** |
| `(10,2)` | 175 / 0 | **0 / 1** |
| `player.life` / `player.mana` pairs | 454 / 1,447 | **357** / **1,351** |

The two `(>16, 1)` rng pairs in the census were the two resets; the
take now has none.

### 8. RESIDUE + LEADS (all measured, none blocking)

1. **The landing pair keeps ~104 rows** (26 tokens × life/x/y/z): the
   scatter's three draws belong to the dying wizard's private LCG,
   which this port has no home for (the human owns no pool record). A
   copy of the token's own seed stands in. The recorded carpet DOES
   carry `rand` — importing it would close this exactly.
2. **Loose class-15 jars do not snap z.** Retail's scattered tokens
   read 1997/2063/3194/3887 one tick after the scatter (cave terrain);
   the port leaves them at the corpse's z. That is the class-15
   pickup-state arm, not the death law — a clean follow-up, and the
   take's `z` family is the biggest one left.
3. **The death puff ticks one frame early.** The port's player column
   runs before the pool walk, so a `(10,1)` allocated below the carpet
   slot seeds its `(10,0)` fires in the same frame; retail's puff is
   spawned at slot 167 inside the walk and waits. 6 extra rows on the
   landing pair (`mc2_manifestation_pass` is the existing cure shape).
4. **The mid-burst regen suppression is worth a dig.** `sub_68DE0`
   zeroes the caster's `manaRegen` on every non-first burst tick, and
   the manifestation slot decides whether the wizard body already
   applied it: mc2l3's spell-3 token sits at slot 103 < carpet 167, so
   retail's mana stays FLAT while the recorded `d88` reads the
   post-recompute 100. The port applies the imported delta and
   over-regens by exactly one quantum. This is the take's largest
   single family (`player.mana` 1,447 pairs) and the MC1 import lane
   already has the twin clamp (`conformance.rs`, the `f48 != f50`
   probe) — the MC2 version wants `any live human class-15 token with
   `f26 != 0 && f26 != f28` below the carpet slot`.
5. `mc2_rival_death_impact` spawns the rival grave at `ground_z`, not
   at the corpse's floor — the same one-line law this dig fixed on the
   human side, left alone because no rival death was measured here.
6. The port's respawn queues `pending_respawn` but the human pose
   belongs to the app, so the same frame's scans still run from the
   corpse (retail moves the entity mid-frame). Harmless — you collect
   your own jars a tick early — but it is a real one-tick divergence
   in native play.

### 9. TESTS + GOLDENS

New, non-vacuous, `engine::world::tests`:
- `mc2_death_scatters_the_book_and_the_reset_re_mints_it` — the
  landing scatters every manifestation into a loose jar on the
  `rand%90+200` clock, leaves the **boolean 1** marker (not 0), raises
  the grave, then a corpse tick that changes NOTHING (no heal, no
  regen, no delta recompute), then SPACE: alive, life 10000, mana
  **750** = 1000 + the held −250, a castle teleport, fresh tokens in
  the owned state at NEW slots, the old jars still out in the world,
  hands unchanged.
- `mc2_speed_window_trails_a_puff_every_fourth_tick` — one puff per
  four ticks, `maxLife` 8 with `life` 4×8, action 2, the caster stamp
  and the exact flag word; off-cadence shifts WHICH tick drops, never
  how many.

No golden was re-pinned by this work: the MC2-native-visible changes
(the Speed puff, the owned-state guard) were A/B'd against
`mc2_cave_behaviors_and_goldens` and left its hashes bit-identical.
That test IS red in the shared tree — its divergence is in `hashes[0]`,
the freshly built world before any tick, which no death path can
reach; it belongs to the concurrent `(10,13)` smoke-puff work
(§"LIGHTNING STORM + THE (10,13) SMOKE PUFF"), whose own re-pin note
says pool population and free-list order move from the build settle on.

## MC1/HW SPEED-TOKEN CONTRAIL + TOKEN-OWNER IMPORT + PLAYER
## LIFE-STATE LANE (session lead, dig/re-triage round, 2026-08-06)

**(10,2) = the direction tokens' contrail, BOTH directions.** The
ledger's old attribution (Accelerate `sub_56380` alone) was
incomplete: the reverse token's arm `sub_57F00_58410` (remc1hw
:62390-451; v_12 written −3×/−2× base, otherwise the same body)
emits the same `(10,2)` puff every 4th token `+63` tick, and the
hw:0 rival brakes/boosts near-permanently — every one of the take's
1,847 missing rows was RIVAL contrail (`id24` 473; the human never
cast a direction spell all take). Landed:

1. **Strict class-12 phase-0 arm** (world.rs `class12_tick`): for
   spells 2|21 with the imported burst counter live (f26 ← retail
   +48) and `f63 & 3 == 0`, spawn the `(10,2)` at the OWNER's pose,
   `id24` = owner, `act_life ×4` (ctor 8 → 32 — the t=3 corpus puff
   reads 31). Admission per `sub_55DD0_56300` (:61132-55): both
   direction ctors author no castle-store req; only first-burst-tick
   mana ≥ 1000 gates. Cadence corroborated tick-exact (rival token
   f63 244 & 3 == 0 at the t=2 spawn).
2. **Token-owner import** (conformance.rs `import_ent`): class-12
   `f144 ← tr(f42)` — retail keeps the token's owner wizard carpet
   slot in +42 and +144 is authored 0 on every token (corpus-proven),
   so the lane is free. This is the re-anchor lane the Rebound dig
   banked as lead 1; MC1 obs never projects f144, and
   `rival_token()` requires native encoding, so both stay unaffected.
3. **Native contrail** (`manifestation_tick` arm 2): same law off the
   native f26 burst; test
   `accelerate_burst_emits_the_contrail_on_the_4_tick_cadence`
   (non-vacuous: off-phase and lapsed-burst ticks spawn nothing).
4. **Player life-state import fix** (the storm dig's diagnosed lead,
   landed verbatim): `state:` read `carpet.f66` = sCLASS (255
   always) — every dead player imported Alive-with-negative-life and
   re-ran the death cascade per pair. Real lane = the tick-handler
   byte `+70` (`*(_BYTE*)(a1+70) = 3`, :55550).

**Full-take mc1hwl0 (`--input-delay 2`), post-storm baseline →
after:** conforming 7,464 → **7,810**; rng mismatched pairs 33 →
**4**; missing 17,142 → **15,035**; unexplained field 799,629 →
796,115. `(10,2)` 1,847/0 → **150/220** — the residue is (a)
burst-START pairs (the rival casts mid-tick: token 0 → 251 inside
the pair, e.g. t=70 — cast-timing capture) and (b) free-stack
slot-allocation capture from t≈278 (both sides spawn the same puff
at the same tick, different slots; the retail slot is port-occupied
so the balance degrades to field rows + one-sided extras — the
extras run 278-326 at exactly the retail cadence). Suites: 1 FIXED
(hw t=4 → conforming), 3 hw drifts + 1 mc2l4 drift all
signature-shrinking, promoted; 0 regressions.

**Leads:** ① the extras/missing (10,2) residue wants a slot-capture
roster rule scoped to burst windows; ② the strict arm skips retail's
`Type_160 +14` force-end clamp (`+48 = 1` when the owner's v_14 lane
is pinned) — unmodeled, invisible on this corpus; ③ the regen seed
clamp (conformance.rs :385) counts ANY mid-burst class-12 with
`f144 == 0` as the human's — with rival tokens now stamped via f42
that heuristic could be tightened to `f144 == PLAYER_TARGET` (it
currently keys retail rows, where +144 = 0 for rivals too — worth a
re-measure against the player.mana family, 9,983 rows).

## MC2 CAVE STOCK-BAKE DIG — THE GENERATOR IS BYTE-PERFECT; THE
## LOAD-TIME SPAWN DATUM WAS THE BUG (dig agent, 2026-08-06)

**Premise falsified.** The dig opened on "the MC2 cave terrain
GENERATOR chain diverges from retail's t=0 bake". It does not.
`mgc-import/src/mc2_terrain.rs` reproduces retail's
`GenerateLevelMap_43830` **byte for byte on every plane**: over
mc2l3's 65,536 cells, the count of cells that neither retail's nor
our load pass touched and that still disagree is **0 on all five
planes** (type/height/shading/angle/ceiling). Every divergence the
record-0 validator saw lived in MC2's LOAD-TIME cave-sculptor pass
(`GenerateEvents_49290` + the `ApplyEvents_498A0` settle), which
runs after the bake and before tick 0.

The three-way split that proved it: `mgc-conform terrain-diff --out`
gives retail's measured record-0 planes and our POST-LOAD planes;
the new `mgc-import --example tmp_mc2gen` dumps the generator's
output BEFORE the load pass. Generator-vs-`.mgcl` matched exactly;
`.mgcl`-vs-post-load differed by 12,465 height cells — i.e. the
carve, not the bake.

### The three laws (all decompile-line-cited)

1. **LOAD-TIME SPAWNS SIT ON THE TILE CORNER, NOT ITS CENTRE.**
   `PrepareEvents_49540` builds every one of its three spawn
   positions as the bare `entity->axis2d_4.x << 8`
   (Events.cpp:307 class 2/0x0E, :339 the 0x2D building arm, :353 the
   generic class-10 arm). Only the RUNTIME disposition spawn
   `sub_4A310` adds the half tile (`(axis2d_4.x << 8) + 128`,
   EF:33014). `World::spawn_from_thing` added +128 unconditionally,
   so the MC2 at-load pass placed every cave sculptor half a tile
   SE of retail's. That half tile is load-bearing twice over: each
   sculptor derives its box origin from `(position + 128) >> 8`
   (EF:25599-25602) — which rounds a whole tile the other way — and
   its radial profile from `EuclideanDistXYZ` to each tile CORNER
   `(i << 8, j << 8)` (EF:25666-68), so retail's cell-centred cone
   (`d == 0` at the centre cell) became a 2x2-symmetric one with no
   `d == 0` sample at all. Fix: `spawn_from_thing_at(ti, corner)`,
   with `mc2_generate_events` passing `corner = true` and
   `fire_disposition` keeping the centre. MC1 is untouched (its
   load pass runs `load_time_pass`, not this seam) and stayed
   byte-perfect on mc1l0/mc1hwl0.

2. **THE PIT/HILL −128 RECENTRE BELONGS TO THE DISPOSITION PATH
   ONLY.** `sub_4A310` subtracts 128 from x and y for models
   0x54/0x55 (EF:33129-31) — which exactly CANCELS the +128 that
   same function applied at EF:33014, landing the sculptor on the
   corner. `PrepareEvents_49540`'s 0x54/0x55 case (Events.cpp:384-88)
   consumes `word_10` and `par3_18` and does NO position fixup,
   because it never added the half tile. We ran the −128 on both
   paths, so after fix 1 the load-time pits/hills sat half a tile NW.
   Fix: gate the recentre on `r.dis_id != 0xFFFF`.

3. **THE RELIEF-SHADE INVERSION IS LIVE DURING THE LOAD SETTLE.**
   `sub_462A0` / `AddBuildingToTerrain_46570` write `32 - s + 32`
   whenever `MapType != Day` (Terrain.cpp:2030-33, EF:31185-88) — a
   LEVEL property retail holds long before `GenerateEvents`. Our
   `mc2_night_shade` is a post-construction setter, so every repaint
   the load carve fired baked DAY shading into a cave level (the
   plane came out `64 - correct` on ~15k cells). Fix: derive the
   cave half of the flag inside `World::new_for_game` from the
   ceiling plane (`Gen::is_cave`) before `mc2_generate_events`.
   REMAINING GAP: a NIGHT non-cave level has no ceiling plane, so
   its load-time painters (roads/rivers/beams) still repaint under
   the Day law until the flag reaches construction properly.

### Numbers (mc2l3, record-0 stock-bake validator)

| plane | before | after | capture-domain | real |
|---|---|---|---|---|
| type | 2,244 (3.42%) | **131 (0.20%)** | 0 | 131 |
| height | 4,483 (6.84%) | **140 (0.21%)** | 85 | 55 |
| shading | 15,432 (23.55%) | **61 (0.09%)** | 0 | 61 |
| angle | 9,640 (14.71%) | **5,373 (8.20%)** | 0 | 5,373 |
| ceiling | 4,770 (7.28%) | **132 (0.20%)** | 80 | 52 |

The "one-cell X-shift at map edges" in the original report was law 1
seen edge-on, not a wrap/clamp bug: nothing is shifted globally
(rolling the port ±1 in x or y makes every plane WORSE by 3-4x).

### The capture-domain share is now MEASURED, not guessed

Level 3 was recorded twice (the take was re-recorded mid-dig). The
two takes' record-0 bases are **byte-identical on type, shading and
angle, and differ on exactly 85 height + 80 ceiling cells** — all six
`(14,2)` CAVE PILLAR footprints, and monotonically (old take floor
46 / ceiling 91, new take 48 / 89). That is a pillar mid-ANIMATION:
each pillar carries a co-located `(10,64)` riser trigger at
disposition 0 (slots 646-651), which `fire_disposition(0)` arms at
load and which drives the pillar's RETRACT arm on the first live
ticks — after the load settle, before record 0. Our stock bake is a
zero-tick world, so it cannot show it. **Those cells are capture
domain, not a generator or carve bug** — the two-take A/B is the
free discriminator, and it should become standard practice: record
any stock-bake level twice and the cells that MOVE between takes are
exactly the pre-record runtime edits.

### Remaining (all attributed, none unknown)

- **One cave-tube leg, x 158..170 / y 17..28** (55 height, 51
  ceiling, 61 shading, 72 type cells): chain 21→22→23 around node
  (165,22). Both sides carve it; ours runs 2-3 units LOW in both
  floor AND ceiling, i.e. the `sub_34540` rolling midline baseline
  (`x_BYTE_F01FEx[2+0]`, the 32-sample `(floor+ceiling)/2` buffer) is
  2 low for those steps. Ruled out by reading: the chain walk and
  FROM/TO order (Events.cpp:5352-59), the packed-radii nibbles
  (EV:5348), `MoveEntity_57FA0` ≡ our `polar_step` (Player.cpp:6-19,
  pitch 0), the box/abs-wrap arithmetic, the `sub_34B00` wall ring,
  and the buffer shift order. Only 1 of ~55 legs diverges.
- **A 59-cell TYPE-only cluster at x 145..152 / y 79..88**, adjacent
  to the `(152,85)` pillar — downstream of the same capture-domain
  pillar difference (the classes under a retracted pillar differ, so
  the blend retile picks other textures).
- **angle 5,373 = the ORIENTATION NIBBLE ONLY** (bits 4-6; class,
  seal bit 3 and lock bit 7 all agree to within 225/0/0 cells).
  `sub_462A0`/`AddBuildingToTerrain_46570` draw it from the shared
  terrain LCG `rand2_17B4E0` (Terrain.cpp:1995-99, EF:31142-45),
  whose post-generation state we reconstruct with
  `post_generation_pseudo_rand` — and that reconstruction is CORRECT
  (a brute-force solve over all 65,536 seeds against retail's own
  nibbles picks our value, 777). The stream is a per-draw one: an
  offline alignment of our 314,196 load-time draws against retail's
  captured nibbles agrees **1.000 through draw ~154,500** (all
  mesas, domes, pits, hills and most tube legs) and then goes random
  — one missing/extra draw in a tube carve near the y-wrap at
  x 196..201 / y 253..1 desyncs everything after it. Presentation
  only (texture rotation), so it costs no sim fidelity, but it is
  the single remaining lead with a known first-divergence index.

### Instruments added

- `mgc-conform terrain-diff --out <dir>` — dumps both sides of every
  plane as raw 256x256 byte images (`<plane>.retail`/`.port`) for
  offline clustering.
- `mgc-conform terrain-diff --baseline <dir>` — reads the MEASURED
  planes from an earlier `--out` dump instead of the take's own
  record-0 base, so an attribution stays reproducible after a
  re-record (and so two takes can be A/B'd against each other).
- `mgc-import --example tmp_mc2gen <index> <out>` — the generator's
  planes before the load carve; the third side of the triangle.

**Goldens re-pinned (behavior change toward retail):**
`mc2_cave_behaviors_and_goldens` GOLDEN and OBSERVABLE, all four
checkpoints including the load checkpoint — as it must be, since the
terrain plane the projection hashes differs before tick 0.
Provenance comments carry the citations above. `cargo test -p
mgc-sim` 18/18 suites green, `cargo test -p mgc-conform` suite green
(no fixture regressions), `mgc-import` generator golden unmoved.

## THE MC2 BURST MANA LANE — THE RECORDED `manaRegen` IS ONLY THE APPLIED ONE WHEN THE MANIFESTATION SITS ABOVE THE CARPET (dig agent, 2026-08-06; mc2l3 take-2's #1 family, 3,738 pairs)

`player.mana` + the carpet's `mana` were 3,710 pairs each on the fresh
mc2l3 — the take's largest family — with the dominant shape
`port = want + 100`: one un-suppressed regen quantum per casting tick.
The same lane also swallowed a 40,000-mana Create Castle whole
(t=8445, want 1,359 got 41,359).

### 1. THE LAW

Both engines run the wizard's mana as applied-then-recomputed:
`AddPlayer03_00_5E010` does `mana += manaRegen`, then recomputes
`manaRegen` to the regen floor (EF:59996-60033). The CAST machinery
writes the same word from somewhere else entirely — `sub_68DE0`
(EF:55569), called from the manifestation's OWN class-15 action:

```c
if (a1x->word_0x2E_46 == a1x->word_0x30_48) {          // FIRST tick
    v3 = a2x->manaRegen_0x88_136;                      // a2x = the CASTER
    a2x->manaRegen_0x88_136 = v3 >= 0 ? -a1x->maxMana_0x8C_140
                                      : v3 - a1x->maxMana_0x8C_140;
} else if (a1x->word_0x2E_46 && a2x->manaRegen_0x88_136 > 0)
    a2x->manaRegen_0x88_136 = 0;                       // the mid-burst PIN
```

Both live in the SAME ascending entity walk, so **the manifestation's
pool slot against the carpet's decides which of the two writes the
recorder's frame-tail snapshot catches**:

- **token ABOVE the carpet** — wizard applies, recomputes, then the
  token overwrites. The record holds the token's stamp and applying it
  next frame is exactly right. (mc2l24 slot 118 vs carpet 116:
  `d88` −100 with mana flat, then mana −100 with `d88` 0, then flat.)
- **token BELOW the carpet** — the token stamps first, the wizard
  applies it and then recomputes. The record holds the RECOMPUTED
  FLOOR; what the next frame applies is whatever the token stamps
  then. (mc2l3 t=9033-36: `d88` pinned at 100 while mana goes
  41359 → −100 → flat → flat → +100.)

Two exceptions, both retail's own dispatch:

- **The CASTLE (spell 2)** never re-enters `sub_68DE0` while its timer
  is parked: that timer is an upgrade LOCK, not a countdown. mc2l3
  t=8446+ holds `word_0x2E_46` at 100 while mana climbs +1000/tick.
- **Any action but 0** skips the regen block outright, because the
  block lives in the action-0 body. Actions 2/3 are the corpse (see
  the death-law entry — their HELD delta is the reset's 750/2000
  residue). Action **12**, the level-end sequence
  (`sub_5E8C0_endGameSeq`), freezes mana for its whole run: 176 of
  mc2l3's 177 action-12 ticks apply 0 against a recorded 100.

### 2. THE FAMILY DECOMPOSES EXACTLY

Census over all 22,798 pairs (recorded `d88` vs what retail's mana
actually did, clamped pairs excluded):

| shape | pairs |
|---|---|
| a live burst BELOW the carpet | **3,407** (2,186 first-tick, 1,221 mid-burst) |
| the carpet in a non-action-0 body (death fall / corpse / end seq) | **330** |
| a live burst ABOVE the carpet | 1 |
| **total mismatched** | **3,738** |

which is the reported `player.mana` 3,710 plus the handful the clamp
filter dropped. Nothing else is in there.

### 3. WHAT LANDED

- `conformance::mc2_applied_mana_delta` — the importer now seeds the
  word the wizard body will APPLY, not the one the recorder caught:
  replay `sub_68DE0` over the human's book manifestations that sit
  below the carpet, in slot order. Non-action-0 carpets seed 0.
- `World::mc2_same_frame_debit` — the FIRST-tick debit is deliberately
  NOT pre-applied by the importer. The port stamps it in the
  manifestation pass exactly like retail and lands it there, which
  keeps retail's ORDERING: `mc2_afford` reads the purse BEFORE the
  debit. Pre-applying it instead (tried first, measured) made the gate
  see 1,359 against a 40,000 cost at t=8445 and the entire Create
  Castle cast vanished — the build ball and its (10,43) painter both
  went missing. Native play is untouched (the human owns no pool slot,
  so the stamp pends a tick like retail's).
- `MGC_NO_MC2_BURST_DELTA=1` restores the pre-dig import (both halves)
  for A/B.

The book entry must still BE that manifestation in its owned action
state `3M`: the death scatter parks a boolean 1 marker in the book and
a wraith-stolen jar runs action 78 — neither reaches `sub_68DE0`.

### 4. NUMBERS (windowed A/B, `MGC_NO_MC2_BURST_DELTA=1` = before)

conforming | unexplained field / missing / extra:

| window (mc2l3 take-2) | before | after |
|---|---|---|
| **13500 +500** (burst-dense) | 109 conf, 1,700 / 0 / 31 | **237 conf, 946 / 0 / 31** |
| **14500 +500** | 169 conf, 1,481 / 0 / 10 | **290 conf, 851 / 0 / 10** |
| **10000 +500** | 148 conf, 4,954 / 5 / 32 | **208 conf, 4,344 / 5 / 32** |
| **22620 +178** (the level-end sequence) | 354 / 0 / 0 | **2 / 0 / 0** |
| **9028 +20** (possess re-arms) | 44 / 0 / 0 | **26 / 0 / 0** |
| **8443 +6** (Create Castle, 40k) | `player.mana` want 1,359 got 41,359 | **conforms** |

Full take (`verify-deltas recordings/mc2l3.mgcr`, 22,798 pairs):

| | before | after |
|---|---|---|
| conforming | 14,276 | **15,706** (+1,430) |
| conforming + explained | 14,546 | **15,999** |
| UNEXPLAINED field | 42,414 | **34,986** (−7,428) |
| UNEXPLAINED missing / extra | 335 / 340 | 335 / **337** |
| **`player.mana` pairs** | **3,710** | **10** |
| carpet+entity `mana` hits / pairs | 5,606 / 4,632 | **1,904 / 1,626** |
| rng mismatched pairs | 4 | 4 (see §5) |

Cross-take regression guard: `mc2l24` @45000 +400 conforming
140 → **155**, unexplained unchanged; `mc2l24` @20000 +400 and
`mc2l0` @3000 +400 both unchanged. All six fixture suites `as
expected`, including the newly frozen `conformance/mc2l3.json`
(1,452 fixtures, 0 regressions, 0 drifted).

### 5. THE FOUR RNG PAIRS (mc2l3 take-2)

- **22621** was ours: `player.mana` +100 in the level-end sequence,
  now clean. Its rng lane stays mismatched `(1, >16)` — retail's
  action-12 frame draws ONLY the frame-top LCG step (the mover, and
  with it the cave tail, is parked), while the port still ticks
  something through the end sequence. That is the end-sequence lane,
  not the mana one.
- **14275 / 14395 / 14491** are one shape and NOT this dig's: slot 96
  reads `action 97` in retail against the port's `105`, with a stray
  (10,0)/(10,45) spawned in the port each time. A creature state
  machine one step ahead — worth its own probe.

### 6. RESIDUE + LEADS

1. **The 10 `player.mana` pairs left are the FATAL-HIT frame** (first
   at t=7884, the frame that ends with `action45` 2). Retail takes the
   killing damage in `sub_5EFA0`, which runs BEFORE the `life >= 0`
   gate, so the whole regen block is skipped on the way into the death
   fall; the port applies its imported delta at the top of `tick()`,
   ahead of its own damage intake, and lands a −250 retail never
   applied. Same root as lead 3.
2. `mc2_afford`'s castle-store gate (`f136` against the castle's
   stored mana) is evaluated against the port's own castle bank, which
   carries its own residual — a starved bank there would now suppress
   a cast the importer already zeroed the regen for. Not observed on
   any take in the corpus.
3. The port still applies the wizard's mana at the top of `tick()`
   rather than at the carpet's slot in the walk. `mc2_same_frame_debit`
   is the targeted repair for the one observable that ordering breaks;
   a general fix (running the human's mana body from the walk at
   `mc2_carpet_slot`, where `mc2_player_cast_pass` and the cave tail
   already run) would retire this whole class — including the death
   column's own one-frame puff artifact.

## LIGHTNING/FIRE FIELD-ROW TRIAGE + VISUAL-ONLY DOCTRINE
## (session lead, 2026-08-06, player-ruled)

Post-dig triage of mc1hwl0's 796k unexplained field rows; player
ruling generalized into docs/CONFORMANCE.md §roster "Visual-only
families". All claims verified against consumers before
classification:

1. **(9,9) lightning trail nodes (~331k rows) = capture.** Born-dead
   by law (maxLife=(node>=beam)-1), no victim scan — storm kills
   ride the beam's acquire exclusively (storm dig). Row shapes:
   life −1 vs −2 corpse stamps; max_life 0-vs-300 = different record
   in slot; x/y bimodal — >60-tile (different record) or <0.12-tile
   (one-node index shift along the beam). Rules
   mc1hw-lightning-node-{life,maxlife,x,y}. REAL signal = record
   counts (harness metric still owed) + missing/extra atoms, which
   stay unexplained.
2. **(10,0)/(10,6) fire churn** — fire_tick moves only the z flicker
   and never reads f30: heading = write-only spawn stamp → capture
   (mc1hw-fire-churn-heading, 67k). Same-slot x/y/rand = a DIFFERENT
   fire in the slot, knock-on of the ambient spawn-cadence
   divergence (the undug weather-churn lead) → classified **open**
   citing the parent (mc1hw-fire-churn-{x,y,rand} 75k,
   mc1hw-standing-fire-churn-{x,y} 11.7k) — explained, still on the
   books.
3. **(10,13) smoke puffs = fully visual entity** (smoke_puff_tick:
   rise + 16-tick drift + sprite step, no damage, nothing scans it)
   → blanket field rule mc1-smoke-puff-fields (50k hw + 15 mc1l0).
4. **Boundary case proving the bar: (10,39) ball heading is NOT
   visual** — re-derived per tick and fed to the merge-walk's
   polar_step (:4119) → rows KEPT (ball-merge lead symptoms).

**Close: mc1hwl0 unexplained field 796,115 → 260,818 (−67%);
missing/extra untouched by design.** Remaining top families: (5,15)
guards (~42k — now live leads on measured terrain), mana cadence
(~26k incl. the :385 clamp over-fire lead), (10,40)/(10,39) known
leads, long tail. Doctrine: visual-only = capture with consumer-read
proof cited; knock-ons of real leads = open citing the parent; field
rows only; hit-count jump = re-verify.

**CROSS-GAME SWEEP ADDENDUM (same day):** the visual-only pass was
run over base-MC1 and MC2 as well. MC2 fire tick verified
(mobs.rs mc2_fire_tick): positions never move, z = the f44 flicker
roll, PITCH never read = write-only spawn stamp — but MC2 needed no
new rules: the existing mc2-fire-churn-m0/m6/m13 blanket rules
(cadence dug-to-completion and RULED capture in the session-4/8
rounds) merely lacked the mc2l3 take scope — extended. MC1 fire
rules extended to mc1l0 (same stationary-fire law; its open cadence
lead stands). **Post-sweep: mc2l3 unexplained 34,986 → 29,386
field / 212 missing / 144 extra · mc1l0 8,505 → 8,054.** Everything
left in both games' top families is gameplay-bearing: MC2 (10,40)
aura-pull, (10,39) ball physics, (9,0)/(9,1) bolt aim, (5,19)
firebug stagevars; MC1 (10,39) merge-walk heading, mana cadence,
(9,x) aim. No further visual-only candidates above the noise floor.

## HW:0 LIVE REBOUND — playtest "still no rebound" TRIAGED: mechanism WORKS live, the gate is the 8000-stored era (2026-08-06)

**Player report (post-fix playtest):** hw:0 Vodor still never shows
Rebound in the port, "even though he has it pretty much the entire
time in retail."

**Method.** Temporary live-game probe (deleted; recipe: build hw:0
from `baked/mc1hw/level-000.mgcl` + `mc1-arctic` via
`World::new_for_game(Mc1Hw)` + `set_wizards`, drive scripted
`PlayerPose`/`PlayerCommand` ticks, watch `debug_rival_ai()` +
`debug_pool()` — the pool view now carries `f26/f140/f136/f144/f146`
lanes for exactly this kind of dig). Retail side read straight off
the corpus with `mgc-conform dump-state recordings/mc1hwl0.mgcr <t>
<slots>`.

**Findings.**

1. **Config is right.** hw:0 wizards.json slot 1 (Vodor): Rebound
   pregranted+allowed (book[14] set, manifestation minted at spawn),
   `castle_level` 0 — and retail t=10 confirms NO rival castle
   exists (slot 473 wizard only), so the authored-castle question is
   closed: retail Vodor bootstraps homeless too.
2. **The whole arm works end-to-end in a live game.** With his
   castle storing >= 8000, EVERY fireball volley opened a window
   within ~5 ticks: trigger scan (class-9 f146 = him, <= 1024),
   ladder pick, `rival_cast_ready`, commit past the castle gate,
   f26 = 101 countdown, `flags|0x8000` published, deflection
   returned the volley (and eventually killed the probe player).
   A realistic 25k-tick session (64-tick hit-and-run volleys every
   1500 ticks) produced 19 windows / 1,900 bounce-ticks, including
   the re-up chatter retail shows, and even reproduced the
   rebuild arc: stray volley fire collapsed his castle (lvl 4 ->
   gone), he re-planted at a new site, re-banked 10,000 in ~1k
   ticks, and went back to bouncing.
3. **The gate that hides it is the Rebound castle_req 8000 — an
   ERA, not a bug.** Both engines bank the same curve: first haul
   4,490 by t~1-2k, stall while the map's ball supply is dry, then
   cross 8,000 on the next wave. Retail crossed at ~5.5k (first
   corpus window 5,591); the port crossed at 6.8k/9.6k/15.8k with a
   PASSIVE player (variance = ball scarcity) and at **t=750** with
   an ACTIVE one (the player's own fighting feeds the world's
   balls). Corpus cross-check: the identical 4,490 first-haul
   appears in retail slot 522 f140 at t=1000 — the economy port is
   faithful; wealth split during ferry (balloon cargo vs banked)
   lags retail ~1.2k ticks at worst.
4. **Retail "permanent" decoded.** The take has just SEVEN windows
   (~1.4% of ticks): Rebound is REACTIVE — the player only observes
   it when attacking, and after t~5.5k every attack provokes it,
   hence "permanently up." Retail's era ended at ~21-26k when the
   player wrecked castle 522, Vodor re-planted, died castle-less,
   and was ELIMINATED (slot 473 corpse states from ~31k on).

**Verdict: no port defect found.** What the playtest needs in order
to SEE it: engage Vodor a few minutes in (after his castle banks
8,000 — faster the more the level is actually being played), don't
eliminate him first, and expect bounces only WHILE provoking him
(4-second windows, re-upped per threat). If a session with those
conditions still shows nothing, the next suspect is the app binary
being stale (the fix tree must be rebuilt), not the sim.

**Tree delta:** `DebugEvent` (debug_pool) gained the f26/cargo/cap/
owner/chase lanes (diagnostics-only, hash-silent). Suites: rivals
unit tests, level-005 goldens, fmt — all green.

## HW REBOUND ROUND 2 — the deflection itself was broken: sound-only "rebounds" (2026-08-06)

**Player report (second playtest, rich Vodor confirmed):** the rebound
SOUND plays, but the meteor explodes on him, hurts no one, and no
projectile ever comes back. Reproduced the triage conclusion from
round 1 (the arm/bit/economy are fine) and found the real remaining
defect in the deflection reader.

**Retail law (sub_52B30 :62858-90, re-read from source).** On a bolt
striking a victim whose `+17` bit 7 is set: quarter = the bolt's
`+140 / 4` (signed); **afford gate** `quarter <= victim->+140` — the
victim's +140 is his MANA (retail keeps a wizard's mana ON the
entity). On success: sound 28 positional at the victim (**inside**
the branch, :62861), `victim->+140 -= quarter`, heading reversed +
rand%0x5B−45 scatter, pitch negated, chase = the original shooter,
owner = the deflector, life refreshed, bolt relinked at the victim
LIFTED by `victim->+84`, and NO explosion. On afford-fail: **nothing
at all** — no sound, no debit, no hit; the bolt flies straight
through (the :62859 false arm leaves the explode flag clear).

**Port defects found.**
1. **The rival wizard entity's `f140` was never written** — rival
   mana lives in `Rival::mana` and the entity field stayed 0 for
   life, so the afford gate compared against 0 and EVERY real bolt
   failed it. (`RivalAiDebug`'s doc comment even claimed the mirror
   existed.)
2. The port played sound 28 BEFORE the afford gate — the reported
   sound-but-no-rebound.
3. On afford-fail the port fell through to the normal
   teleport-onto-victim EXPLODE — retail flies through.
4. The deflected bolt relinked at the victim's z without the `+84`
   lift.

**Fixed.**
- `crates/mgc-sim/src/mc1/rivals.rs`: the +140 mana mirror —
  seeded at `spawn_rival`/`rival_respawn` (1000), published at the
  end of every `rival_alive_tick` (= `Rival::mana`), and reconciled
  DOWNWARD at the tick top so combat-side quarter debits land in the
  pool. Downward-only: every port-side credit writes `Rival::mana`
  first and re-publishes the same tick.
- `crates/mgc-sim/src/mc1/combat.rs` (`proj_move_and_hit`): sound 28
  moved inside the afford branch (positional at the deflector);
  afford-fail = silent fly-through (advance + return, no explode);
  deflect relink lifted by the victim's `f84`.

**Verification.**
- New unit test `rebound_deflection_bounces_debits_and_is_silent_when_poor`
  (rivals.rs): world maintains the mirror; a direct `proj_tick` on a
  parked bolt against the rebounding wizard deflects (no explode,
  owner swap, re-homed on shooter), debits exactly quarter (1000 →
  900 for a 400-mana bolt), twangs; the poor arm (50 mana vs quarter
  100) flies through silently with no debit. NON-VACUOUS: verified by
  toggling each sub-fix off — the mirror off fails the mirror assert,
  the old explode/sound restored fails the fly-through + silence
  asserts. Test trap for posterity: park the encounter AWAY from the
  starting castle — its 0x2000-tall envelope scan-resolves the CASTLE
  instead of the wizard hovering on it.
- Live hw:0 probe (temporary, deleted): rich era at t=6,839; volley
  → bit ON at +5; **returned bolt (class-9 owned by Vodor, chase =
  PLAYER_TARGET) at +10**. The player-visible loop is closed.
- Goldens: `level_005_golden_state_hashes` re-pinned — attributed by
  toggle to the f140 mirror ALONE (a deliberate hashed-field change;
  the deflection restructure is golden-silent). Observable goldens
  unmoved. All 18 mgc-sim test bins green; fmt clean.
- Conformance: mc1l0 492 / mc1hwl0 852 / mc2l3 1,452 fixtures — all
  as expected, 0 regressions, 0 drift (the rival lanes are inert
  under import; the mirror writes retail's own value back).

**BANKED — MC2 twin:** `mc2/proj.rs` has the same quarter-debit
pattern against entity f140. MC2 rival Rebound windows are not yet
mirrored onto their entities (existing DEVIATIONS note), so the gate
cannot mis-fire there today — but when the MC2 rival rebound lands,
it needs the same mana-mirror treatment. Also banked: the PLAYER
deflection arm keeps its INTERIM no-debit (sound placement now
matches retail; the debit needs the player pool reachable from Gen).

### ROUND-2 ADDENDUM — playtest PASSED; retail "stuck flag" lead OPEN (2026-08-06)

Player confirms deflection now works live. New lead from the same
report: retail Vodor remembered as rebounding AT ALL TIMES, even
castle-less — suspicion that the flag gets stuck. Corpus second look
(dump-state slot 473 `flags` at 5600/7000/21400/24000/26000/31000/
42000/49000): **this take is clean** — bit 15 ON only inside the
seven windows, OFF between them, OFF through the final death and the
entire corpse era; and window 7 (21,372→21,473) is law-consistent —
he had REBUILT (slot 233, level-3 castle, stored 13,768 — the number
the round-1 instrumentation saw). The take never TESTS the stuck
hypothesis though: he never died mid-window. Mechanism is credible:
retail clears `+17` bit 7 only from the token's burst tick
(sub_573F0_57920), and rival death scatters the manifestations into
decaying ground jars (:55519-49) whose handlers never touch the bit
— a death with the window live should orphan the bit ON permanently
(castle-less, across respawns). The port deliberately deviates (the
death transition drops the bit — see the ROUND-1 fix notes).
**Discriminator for a future retail take: kill the rival within ~4 s
of a deflection, then check whether everything bounces off him for
the rest of the level (flags lane stuck 0x8000 through state 3).**
If confirmed: player ruling — keep the sane deviation (DEVIATIONS.md
entry) or port the stuck flag faithfully.

## V2 CORPUS INTAKE — mc2l0 / mc2l4 / mc2l30 / mc2l24 (2026-08-07)

The 2026-08-06 evening re-records intaken and FROZEN per the
freeze-at-intake law; these four sections supersede the "Baseline
corpus" block's numbers for their levels. Pipeline per level:
check-decode → terrain-diff → full verify-deltas (`--csv` at repo
root) → extract → carry_curation → classify_fixtures → freeze →
suite gate. ALL SEVEN suites green at close.

**check-decode (all 100% clean, terrain base present):** mc2l0
22,696 ticks (659 deltas / 37,913 cells) · mc2l4 17,819 (626 /
128,685) · mc2l30 12,555 (803 / 118,614) · mc2l24 67,268 (3,821 /
722,177). The mc2l4/mc2l30 cut (t=17,820) with the materialized
part-B base decodes clean on both halves.

**Stock-bake terrain-diff (first ever on these four levels; cells
of 65,536):**

| take | type | height | shading | angle | note |
|---|---|---|---|---|---|
| mc2l0 | 34 | 15 | **3,676 (5.61%)** | 33 | shading = SUM-64 signature |
| mc2l4 | 28 | 1,357 (2.07%) | 583 | 940 (1.43%) | unattributed, first look |
| mc2l30 | 8 | 288 | 88 | 345 | local clusters (x≈182-206) |
| mc2l24 | 1,086 (1.66%) | 729 | 429 | 3,088 (4.71%) | ONE region x≈120-134,y≈101-105 |

- **mc2l0 shading: every example sums to 64 (retail = 64 − port)** —
  this is the banked night-shading gap made measurable (mc2:0 is
  MapType Night; port bakes Day relief shading — the flag must reach
  construction, the known ~60 `World::new` sites item). 3,676 cells =
  the relief-shaded population. The corpus now grades the fix.
- **mc2l24 (Night+doom): NOT the inversion** — diffs cluster in one
  region (the citadel footprint): type retail 8 vs port 77/79,
  shading a constant −8, angle 4.71%. Reads as an authored
  citadel-stamp bake difference, one story.
- **mc2l30 = MapType CAVE (level.json: cave, basic_height 173) and
  the take has NO ceiling plane** — the conjoined take started on
  non-cave l4, so the recorder never declared ceiling (the cut
  caveat, as predicted). Ceiling runs PRISTINE-GENERATED in verify;
  in-level cave carves are unshielded there. The floor planes'
  small residues cluster locally (eruption/spawn-stamp shaped, e.g.
  height ±2..4 at x=197-203,y=16-17). A future native-l30 take
  would close the ceiling.

**verify-deltas headlines (terrain MEASURED everywhere):**

| take | pairs | torn | conforming raw | conf+explained | UNEXPL field/miss/extra | rng |
|---|---|---|---|---|---|---|
| mc2l0 | 22,695 | 0 | **17,153** | **21,872 (96.4%)** | 2,288 / 33 / 26 | 1 |
| mc2l4 | 17,818 | 0 | 9,716 (was 0!) | 14,898 (83.6%) | 9,406 / 57 / 43 | 4 |
| mc2l30 | 12,553 | 0 | 2,657 | 10,986 (87.5%) | 4,221 / 37 / 42 | 1 |
| mc2l24 | 67,233 | 329 | 13,251 | 27,740 (41.3%) | 367,145 / 445 / 569 | **3** |

RNG stays essentially locked corpus-wide (9 mismatched pairs across
120k) — even through the l24 endgame frenzy (old take: 1,816 torn;
new: 329).

**mc2l24 freakshow scoping REFUTES the convenient story:** the
unexplained bulk is MID-GAME — t=5k-25k holds ~254k of the 367k
field rows; the post-victory stress window (t=50k-70k) only ~78k.
Biggest single family in the GROSS entity-set table: 22,093 missing
(10,39) mana spheres from t=2,857 on — **CORRECTED at triage: all
22,093 are already claimed by the `mc2l24-ball-terrain-roll` capture
rule** (fountain/merge/summit-grounding closure, 2026-08-04 dig) and
none reach the unexplained headline (445 total unexplained missing).
A spot dump (slot 357, t=2,857→2,858: alive 300/300 both ticks in
retail, port kills it within ONE tick of import) is consistent with
that rule's merge/grounding story rather than a spawn miss; the
port-side "N spawn(s) dropped" bursts live in the LATE stress
window only (pairs 36k+) and remain the entity-cap story. Second
family: (9,9) lightning 9,527 missing / 2,539 extra from t=8,222.

**Cross-take unexplained families (the joint-triage roster; all
takes agree, exemplars in the repo-root TSVs mc2l0/l4/l30[-v2].tsv):**

1. **player.life +5 skew** — (3,0).life + player.life mirrored, all
   four takes (111/426/443 rows + l24): after a damage event the
   port sits EXACTLY +5 above retail persistently (retail 9500 /
   port 9505 for hundreds of flat ticks), damage tick itself one
   frame apart (t=265 l30: retail 10000 port 9220). Reads as one
   extra +5 life-regen application around the damage frame — the
   frame-ORDER class again (cf. the burst-mana debit law).
2. **(9,9) lightning missing** — l4 1,355 missing from t=928 (the
   3-player rival level), l24 9,527 from t=8,222. Port draws far
   less lightning than retail in rival fights.
3. **(10,17) spawn pitch −2 + seeded mana** — CORRECTED by the
   want/got distribution: retail applied_pitch walks multiples of
   192 (192/384/576/…) and the port is EXACTLY −2 on every rung
   (not compounding) → a spawn-time applied_pitch offset of −2 with
   an identical 192/tick rate. Same family also shows port seeding
   mana 8000/16000 where retail keeps 0 (l30: 103 of 106 mana
   rows). (l30 965 + l4 417 pitch rows.)
4. **(10,39) mana-sphere speed** — port pins 16, retail varies
   (17/24/28/63); all takes (472/122/108 rows + l24's missing mass).
   Sphere speed law, not a constant.
5. **(5,9) action 72→73 at import** — l4 t=0 (354 rows over the
   whole starting population), l30 a t=2,298 burst: retail parks at
   action 72 (39 rows also 79) where the port writes 73 — the port
   collapses a 72..79 state band onto 73. Import/state-mapping
   off-by-one class (the f66→f70 lesson).
6. **(10,79) spawn z** — port spawns at flat z=1760 where retail has
   terrain-varied 800-1152 (l30 t=1,114 burst; l4 437 z + 253 x
   rows). Spawn-z law.
7. Lesser recurring: (10,1).mana, (10,45).life, (5,x).sv1 families,
   (3,3) rival pose (x/y/heading), (10,40).mana_max l0 t≈11.2k
   (the aura-pull lead's family).

**Suites (frozen at intake, gates green):** mc2l0 1,740 fixtures
(1 carried + 23 stories; classify 10 capture/13 open; one
select-dependence FIXED at t=11,322 promoted) · mc2l4 996 (15 open /
8 capture; t=2,278 (15,9) select-dependence demoted open) · mc2l30
290 (t=3,586 (15,21) ditto) · mc2l24 1,349 (0 carried — the
freakshow shares no stories with the old take; 16 capture / 8 open;
green first run). The (15,x) select-dependence pair = the known
warning-grade shared-world leak (2026-08-01 suite note), now with
two more exemplars, both class 15.

### mc1hwl0 gate: 5 rival-mana fixtures parked on the REBOUND-PENDING pile (2026-08-07)

The all-suite pass found mc1hwl0 at 846/852: five fixtures
(t=21,039 / 24,145 / 24,163 / 24,264 / 25,922), ALL `field:(3,1)
mana`, frozen as conforming at session-11 close, now +100
port-over-retail. Full-take windowed re-verify REPRODUCES it (not
suite-context): retail's rival drains ~100-150/tick in these
stretches while the port, re-importing retail state each pair,
re-creates a +100 skew within ONE tick — i.e. retail is PAYING for
something per-tick that the imported port rival doesn't resume
(rival cast/channel state not reconstructed at import — the f66→f70
lesson's rival sibling). This is rebound/re-up territory and the
player owes the retail triple-check (ROUND-2 addendum above), so:
demoted open with a note, t=140 FIXED promoted, suite green
852/852. Revisit WITH the rebound ruling. (Why it was green at the
session-11 freeze and not now is unexplained — the tree is the
committed db356a7 state; suspect the freeze predated the last
rival-arm edits of that session.)

### JOINT-TRIAGE ANSWERS (player, 2026-08-07) — three redirects

1. **l24 mana spheres: "only after victory" did mana lie around** —
   the player collected as they went mid-game. This REFUTES the
   lying-mana/entity-cap framing for the 22,093 missing (10,39)
   rows from t=2,857: the early/mid-game missing spheres need a
   different story (sphere spawn/lifetime law, not cap pressure).
   The port-side "spawns dropped" bursts (pairs 36k+) remain the
   late-game cap story only.
2. **Life regen (player concept, TO VALIDATE FROM DECOMPILE):**
   "life regenerates extremely slowly; speeds up at dolmen/shrine/
   castle; outside of that it's so slow it feels disabled." The +5
   skew = port applying a regen quantum around the damage frame
   that retail gates. Dig authorized.
3. **(9,9) lightning is NOT wizard-cast: "primarily castle
   defenses; I do not recall players using it. In l24 it's also the
   hydra and to some extent the final boss (Vissuluth)."** So the
   under-firing arm is the CASTLE-DEFENSE lightning (and hydra m27 /
   Vissuluth breath arms on l24) — not the rival reactive-cast
   ladder. l4's t≈928 onset = when rival castles first stand.

**Dig order (player selected ALL four lanes):** ① life +5 frame
order (decompile validation of the regen concept) → ② l24
mana-sphere drop (now a spawn/lifetime story) → ③ castle-defense
lightning arm → ④ the constants batch ((10,17) 384-vs-382 rate,
(10,39) sphere speed, (5,9) import 72→73, (10,79) spawn z 1760).

### ⭐ LANE ① LANDED SAME DAY — the life+5 family was the UNSEEDED REGEN-STALL COUNTER (2026-08-07, FIXED)

Opus decompile dig, spot-verified, then landed. **The port's live sim
law was already correct** (world.rs `regen_delay`: armed 16 on
hit/grip/steal, damage-before-regen order, 10000/2000 = +5 afield vs
10000/250 = +40 at castle/dolmen — validating the player's concept
exactly: slow ambient, 8× at castle/dolmen, and the 16-tick stall
re-armed under sustained fire is the "feels disabled"). The
divergence was VERIFICATION-DOMAIN: retail's stall counter
(MC2 `dword_0x18D_397` = wizext +397, EF:60000-60003, armed
EF:60662/60710/62222; MC1 `u32_383` = +383, sub_main.cpp:55387-90 —
the agent's report said MC1 +397, corrected against remc1 before
landing) was never decoded by the recorder structs, so BOTH
importers seeded `regen_delay = 0` every pair → each pair inside
retail's stall window applied exactly one heal quantum retail
withheld → the dead-flat retail+0/port+5 (or +40) runs, ~15 pairs
per hit, on every MC2 take.

**Fix (pure decoder+importer, no sim change, no re-record — the
bytes were always in the recorded struct images):**
`mgc_formats::mgcr` decodes `regen_stall` (MC1 t+383 u32, MC2 t+397
i32); both conformance imports seed `regen_delay` from it.
PROOF: the l0 t=4103-4160 and l30 t=265-271 flat runs are GONE;
survivors are only the one-frame damage-tick pairs themselves
(capture-window artifact — the attacker's mail is not in the
imported closure; roster-able as capture). All 7 suites green, 0
regressions 0 fixed; state_hash goldens pass (import-side change —
goldens can't see it). **Post-fix full mc2l0 re-verify: 17,494
conforming (was 17,153), conf+explained 22,249/22,695 = 98.0% (was
96.4%), UNEXPLAINED field 1,482 (was 2,288)** — ~800 rows retired on
the cleanest take; l4/l30/l24 carry proportional retirements
(re-verify at next full pass; intake headline table above still
shows PRE-fix numbers).

Banked from the same dig (second-order, masked until now): ① retail
consumes the STORED stale rate `lifeRegen_0x163_355` (+355; port
computes inline — only differs ±35 on castle/dolmen entry/exit
ticks; +355 is in the recorded bytes, seed when it shows up); ② the
l24 roster rules mc2l24-player-life-regen / mc2l24-wizard-life-regen
(and the life half of the mana twins) = this same root cause —
expect their hits to collapse on the next full l24 verify; retire
then; ③ human maxLife = 10000 × header scalar/256 — port hardcodes
10000 (all four takes are scalar 256; a non-256 level would bite);
④ AI wizards have NO stall in retail (EF:5426-5433) and heal
maxLife/200 home / /500 afield — unverified against the port's AI
arm, separate lane.

### LANE ③ MAPPED — the (9,9) lightning deficit IS the castle-defense turret burst arm (2026-08-07, Opus dig; player-redirected)

The player's triage answer ("primarily castle defenses; I do not
recall players using it; in l24 also the hydra and Vissuluth")
aimed this dig straight past the rival-cast ladder — and the
decompile confirms every element.

**The retail arm:** castle pieces (10,79) (ctor sub_508E0 EF:36987,
brain sub_3AF00 EF:30106-472) rebuilt on every castle level change
by sub_613D0 (EF:62234); tower TYPE byte_0x43_67 = the castle spell
tier's life_0x1A stamped by the research child sub_69AB0 (EF:56121)
— type 1 = fireball tower, any other non-zero = LIGHTNING tower.
Scan once per 64 ticks (byte_0x3E_62 & 0x3F), ring band 3..12, no
LOS/invisibility test; weapon roll: 94% → 6-shot burst, one shot
per tick, via the SHARED Lightning path sub_6DCA0 a3==7 → (9,9).
**Row multiplier:** the (9,9) beam (sub_4D860, speed 384, life 9) is
a one-tick hitscan laying steps*8+1 trail nodes (sub_66750
EF:58268-400) → one bolt ≈ 1+81 records, one burst ≈ 490 (9,9)
records. Secondary (9,9) sources CONFIRMED as the player said:
hydra (5,27) whip bolt ≈33% power roll (EF:20543), Vissuluth
doomsday pyramid case-2 volley, selector-gated ~10/29 (EF:13345);
plus one UNRESOLVED site EF:26708 (owner not in any action table).

**Corpus evidence both onsets = this arm:** mc2l4 missing (10,23)
impacts arrive in ~6-wide consecutive-tick runs (925-930, 1156-63,
…) — 48 retail burst windows, ~19 with NO port burst at all; (9,0)
balanced so the port is not rolling fireball instead. mc2l24: 392
missing-impact windows, ALL t≥8,223, starts 63-65 ticks apart —
the 64-tick re-scan clock verbatim.

**Port state: the machine is PORTED and largely faithful**
(castle.rs mc2_castle_piece_tick/scan/fire; proj.rs beam+trail) —
divergence list, ranked: **D7** one-tick aim-phase lag (every
(10,79) pitch row = port carrying retail's previous-tick value;
fits every burst; root walk-order vs fire-delay UNDUG — highest
value next step); **D-19-bursts** ~19/48 l4 bursts absent entirely
(candidates: D5 target-choice deviation, or the f67 tower-type
stamp path — port stamps SPELLS[2].tiers[tier].life at cast time
vs retail's research-child stamp at castleLevel+1 — compare f67
against the recording); **D1** aim order (retail aims from unlifted
muzzle at box-raised target, lifts z AFTER; port aims from lifted
muzzle at raw target); **D2** trail one node short + one spacing
downrange (drops retail's muzzle node — the mc1hw-lightning-node-x
shift); **D3** lateral jag accumulator (possible IDA artifact);
**D4** charged lightning doesn't chain (documented deviation —
also kills turret mode 2, 5% of bursts); **D5/D6** documented
deviations (target order, friendly exemption).

**⚠ ROSTER MASKING WARNING:** mc2-cast-timing-missing/extra swallow
ALL class-9 set rows — their own notes warn "ONE-SIDED counts = a
real port bug hiding here", and 1,355/21 (l4), 9,527/2,539 (l24)
is exactly that. mc2-lightning-blast-churn likewise masks the
strongest signal (217/0, 1,660/14 (10,23) missing). When the
turret fixes land, re-scope those rules or the win is invisible.

Also corrected: docs/traces/mc2-castle-runtime.md:422 conflates
(10,79) with the (5,15) guard — different brains (sub_3AF00 vs
sub_23C40); trace doc NOT edited this session, note carried here.

### REBOUND ROUND 4 — CLOSED BY PLAYER RETAIL PLAYTEST (2026-08-07)

Player ran the retail check: **reduced the rival's castle to rubble
→ the rebound went away.** That is the ported law exactly (reactive
re-up gated on castle stores ≥8000; castle-less = no re-up, window
lapses) — the remembered "rebounding at all times, castle-less" did
NOT reproduce. Player ruling: "everything is the way it should be
now; if I ever spot a deviation, I'll reopen." Topic CLOSED. (The
narrow die-mid-window jar-orphan discriminator from the ROUND-2
addendum was not the path tested; it stays moot under this ruling —
the port's death-drops-the-bit guard stands as the sane behavior,
no DEVIATIONS.md entry needed since no retail divergence was ever
observed.)

The five mc1hwl0 (3,1)-mana fixtures parked on this pending check
are hereby RE-SCOPED, not un-parked: with the rebound arm vindicated,
their +100-per-pair skew is a pure IMPORT-DOMAIN lead — retail's
rival is paying ~100-150/tick for something in those stretches and
the fixture import doesn't reconstruct the paying state (the exact
class the regen-stall fix just closed for the human). Banked as
"rival per-tick payment state unseeded"; fixture notes updated; dig
when convenient, no urgency.

### mc2l30 NATIVE RE-RECORD INTAKEN — THE CEILING CHANNEL IS CLOSED (2026-08-07)

Player recorded l30 natively — CORRECTED MECHANISM (player,
same day): NOT a savestate load. MC2 makes a reached hidden level
revisitable from the campaign map (red portal; flag once
completed). The player unlocked it via l4, exited, saved the
CAMPAIGN, restarted, and launched l30 directly — a NORMAL level
start, so the recorder pinned MapType=Cave and declared the
ceiling. (Mid-level save-load recording remains UNTESTED; player
rates it low priority.) Take REPLACES the cut one at
recordings/mc2l30.mgcr per player instruction (cut preserved as
mc2l30-cut.mgcr; regenerable from mc2l4,30.mgcr anyway).

- check-decode: 12,562 ticks 100% clean, 763 deltas / 119,257
  cells.
- **terrain-diff: FIRST MEASURED l30 CEILING — 0.13% (84 cells)**
  vs the port generator; floor planes 0.23-1.03% — MORE than the
  cut take's (8/288/88/345 cells), which a fresh map-launch would
  not obviously explain. OPEN QUESTION: does MC2 PERSIST hidden-
  level terrain across revisits (campaign state), or does the
  revisit re-bake with different spawn stamps? Until answered,
  treat the native base as ceiling evidence only; the cut take
  remains the t0-bake instrument for the floor planes.
- verify: 12,561 pairs, 0 gaps 0 TORN, 2,024 raw conforming,
  10,541 conf+explained (83.9%), UNEXPL 5,199 field / 98 / 22 —
  ceiling now INSTALLS measured per pair. ⚠ **rng 39/12,561** (cut
  take: 1) — cause unknown (save-load theory RETIRED with the
  mechanism correction; different gameplay or revisit state are the
  remaining candidates), long-tail item.
- Suite re-frozen: 227 fixtures (14 carried via sig-bridge from the
  cut-take suite, 10 new stories), gate green; ALL 7 suites green.
  Select-dependence claimed a THIRD exemplar (t=3,362, again
  (15,21) action/owner) — demoted+sig-refreshed like the others.

## PORTAL WARP-OUT ALTITUDE — mc2l24 enclosure deviation, the WHOLE
## teleport family re-pinned to retail z + speed laws (player-spotted,
## LANDED 2026-08-07, PLAYTEST OWED)

Player report: the mc2l24 start enclosure's pad warps the port flyer
at its PRE-portal altitude (x/y only, snap-up-if-buried), where retail
emerges ON the ground — and the too-high flyer drags the enclosure
monsters' aggro/aim upward (live-play cascade; conformance replays pin
the recorded pose, so the suites never saw it).

**Retail law (both games, resolved from the decompiles):** the warp
tick recomputes `dest.z = row->word_0xc + terrainAlt(dest)` on every
warp — MC2 `sub_35390` EF:25785-86, MC1 vortex `sub_26A60` :29212-13 —
and the pad keeps the NewEvent-default behavior row (`str_D7BD6[59]` /
`unk_98F38[0]`, byte-identical), whose word12 = **0**: the wizard
arrives exactly on the destination ground. This closed trace OPEN-2 in
docs/traces/mc2-class10-m50-chains-and-tail.md.

**The teleport SPELLS carry their own z laws** (read while in there —
MC1 `sub_56E50_57380` :65554, MC2 `sub_6AD60` EF:56860):
- castle hop → the castle entity's FULL axis (`CopyEntityPosition`;
  MC2 offsets −448 along yaw−204 pitch-0, MC1 has no stand-off);
- T1/return toggle → the SAVED axis restored verbatim (x, y AND z);
  the toggle is castle-gated, and the no-castle random hop CLEARS an
  armed return (:65585);
- random hop → pitch-0 `MoveEntity`, altitude rides along untouched;
- EVERY resolve arm zeroes the caster's flight TARGET speed
  (`Type_160 v_12` :65583/:65601; MC2 `speed_0xc_12` EF:57029), and
  the burst EXPIRY repeats the zero (:65614 / EF:57046) — the
  formerly-banked "flight-speed zero on resolve" follow-up landed
  with this round.

**Port shape:** `pending_teleport` widened to carry the arrival
altitude (`None` = keep, the pitch-0 arms); new `pending_speed_zero`
channel (`take_speed_zero`) → `carpet.tgt_speed = 0` (target only —
the actual chases down 16/tick, retail's glide-out); the consumer
re-seeds `lift_desired` at the arrival pose so a ground emergence
doesn't auto-climb back; `teleport_return` widened to the full saved
axis and cleared at the MC1 death scatter (retail's +154 dies with
the recycled manifestation). SNAPSHOT_VERSION 7→8. Tests: level-032
vortex + (10,34) pad asserts extended with the ground-z law;
`mc1_teleport_spell_z_and_speed_laws` covers the three spell arms.
Full workspace + ALL 7 conformance suites green (bit-unchanged, as
expected for a live-play-only fix).

Still owed on the pad: the rival-warp arm (retail warps EVERY player
in the list) and the `sub_5C800(player, 6)` palette flash — both
pre-registered in the (10,34) APPROX register, unchanged this round.

## TRANSIT-CLUSTERING PROBE (2026-08-07): the pin does NOT alias
## portal warps — and one dated whirlwind lead at t=31121

Cheap pre-test for the proposed player-pose channel: do mc2l24's
unexplained rows cluster after recorded player-position jumps (the
mid-tick-warp aliasing hypothesis — retail moves the player DURING the
entity walk, our pin holds one pose per tick)? Instrument:
`mgc-conform --example pose_dump` (new) + scratch clustering script
over `mc2l24-v2.tsv`.

**Verdict: NO broad aliasing.** 9 true pad transits found (>30-tile
one-tick jumps, flying both sides). All-warp windows 0.92x baseline;
transit-only windows 1.77x but the excess is ONE event — 7 of 9
transits sit at zero-to-baseline rows (the three early enclosure
entries are completely clean: 0 rows before AND after). Mechanism
retired as a deviation source at current corpus scale; the pose
channel's case rests on coverage, not on explaining existing rows.

**The one event — t=31121 (enclosure re-entry, dz=−3184): a 410-row
burst in 8 ticks, ~90% (10,75) WHIRLWIND FUNNEL rows** — full-motion
mismatches (x/y/z/pitch/yaw/rand/speed/applied_*) on ~4 funnel
entities plus ~11 wrong-record rows (model/action/life/max_life/mana
— mis-slotted spawns?). A whirlwind engages right at the re-entry and
the port's funnel column diverges wholesale. (10,75) is the (10,22)
head's funnel child; the tail-band creators around it are the trace's
under-transcribed models. DATED, POSITIONED LEAD — parked for player
triage (mc2l24 carries the late-game freakshow ruling; this is
mid-take t=31121).

**Bonus corpus corroboration of the portal-z law:** every enclosure
transit in the recording lands with dz ≈ −3200 (−12.4 tiles) — retail
measurably DROPS the player to the destination ground on warp. The
row-59 word12=0 law now has recording-side proof independent of the
decompile.

Incidental: pose data alone cleanly classifies the player-motion
event families — pad warps (>30-tile jumps), respawns (act3→0,
+~1700dz, back to castle), knock throws (~80u/tick decaying
displacement runs, the buffet law visible passively). Exactly the
signal set a tier-1 pose channel would formalize.

## METEOR TRAIL SOUND MACHINE-GUN — the dispatchers' SILENT-EMITTER
## gate was unported (player report 2026-08-07, FIXED, PLAYTEST OWED)

Player report: the port repeats the meteor's shoot/trail sound at
every step of the flight path until impact, clipping the audio engine;
retail (BOTH games) sounds once at fire + once at impact. Not a
regression — born with the sound wiring (git bisect found nothing,
correctly).

**Cause:** both retail sound dispatchers refuse any request whose
EMITTER wears flags byte[0] bit 0x80 — the no-damage/decorative bit —
before the request-slot table: remc1 `sub_55370` :64473
(`byte[+16] < 0 → return`), remc2 `PrepareEventSound_6E450`
Sound.cpp:6291-92. The meteor trail is built ENTIRELY of 0x80-stamped
entities (MC1: the per-tick (10,1) seeder + its ring children, all
`|= 0x80`; MC2: the per-tick (10,0) sparks, `|= 0x10080`), and every
fire's activation tick requests sound 3 (:28118/:28152 / EF-side
sub_30D50) — faithful emissions the port then PLAYED because
`Gen::snd()` had no gate: one sound-3 restart per flight tick =
the machine-gun + mode-1 restart clipping.

**Fix:** the gate applied at DRAIN time in `World::take_audio`
(retain: player-sourced OR emitter `flags & 0x80 == 0`), beside the
existing drain-time owner resolution — the sim-side `sounds` vec is
hashed and stays byte-stable (the standing "audio fixes go in
mgc-audio/drain, not snd()" law). Regression test
`silent_emitter_gate_drops_decorative_trail_sounds`. Workspace + all
7 suites green (no hashed state touched).

**Noted while in there (small open lead, not dug):** MC2's dispatcher
keys the channel on the emitter's OWN id (`id_0x1A_26`,
Sound.cpp:6299), not the owner — retail MC2 gives concurrent
same-owner emitters separate channels where our drain-time id24
resolution folds them onto one (restart/keep-running granularity).
MC1 keys on +24, which effect ctors owner-stamp — our resolution is
exact there. Audible-difference cases look rare (the 0x80 gate now
silences the bulk emitters); parked.

## THE POSE CHANNEL LANDED (2026-08-07): the player-motion column
## verified over the whole corpus — ~196k pairs, 99.3% bit-exact,
## two sim fixes and one positioned MC2 gate lead out of round one

The tier-1 ticks-replay design from the transit-clustering session
(§TRANSIT-CLUSTERING PROBE) is implemented and graded. `verify-deltas`
pins the human pose, so the player's own motion column was the one
lane the diff never verified. The POSE CHANNEL
(`crates/mgc-conform/src/pose_lane.rs`, docs/CONFORMANCE.md §"The
pose channel") shadow-steps the faithful mover beside every
fixture-grade pair: flight state seeded from the recorded closure at
N, input recovered from the recorded flight column, one
`flight::mc1_move`/`mc2_move` step against the imported world,
stepped pose diffed bit-exact against N+1. World lanes untouched;
fixture signatures cannot drift (`exec_pair` not involved).

**The decode key (both games, decompile-verified):** the whole flight
column lives in the recorded wizard/player block and most of it was
simply undecoded. New `RetailWizardMc1` lanes (Type_160): `dw_0`
move/fire byte @+0, filter deltas @+4/+6, `v_28` eff_pitch @+28.
New `RetailPlayerMc2` lanes (Type_str_164): move byte @+0, deltas
@+4/+6, knock `moveBoost` @+30 + direction @+32 (remc2's `yaw_0x1E_30`
name is stale), eff_pitch @+36, stick accumulators @+341/+343,
web-slow ladder @+332/+333, paralyze @+334/+336, nudge latch @+609,
water counter @+610.

**Input reconstruction is exact, not modeled:**
- The move byte is stamped by the consume loop and SURVIVES to the
  settled snapshot (unlike the memset 10-byte command). Phase is
  per-game and was corpus-measured: MC1 stamps post-pass (:49018-22
  runs after the entity pass — pair N→N+1 reads record N; mb@56's
  strafe bit moves the recorded strafe across 56→57), MC2 stamps in
  PlayerEvents (EF:38064) — read record N+1.
- The stick reaches the mover only through the IIR filter, and the
  accumulators are recorded at both ends: `recover_stick` inverts
  `(2·stick − acc)/4` per pair (truncation-aware; any solution is
  downstream-equivalent). Map-screen ticks self-classify (retail
  zeroes the command; the accumulators decay; recovery returns a
  centered stick) — no gate needed.
- Mid-pass knock arms reconstruct by un-decaying the N+1 channel
  (mc1hwl0 t=371: kmag 0→76 = 80 armed − 4).
- Terrain probes run on MEASURED terrain@N+1 (terraform writers sit
  below the carpet's slot; on mc1l0 every eff_pitch/z residue row sat
  on a live terraform window; the port-evolved planes are no
  substitute where port terraform itself diverges, e.g. the t=562-570
  castle build).

**Round-one grades (% of stepped pairs bit-exact):**

| take | offered | stepped | bit-exact | % |
|---|---|---|---|---|
| mc1l0 | 7,097 | 7,097 | 7,092 | 99.93 |
| mc1hwl0 | 49,765 | 49,121 | 49,059 | 99.87 |
| mc2l0 | 22,695 | 22,402 | 22,376 | 99.88 |
| mc2l4 | 17,818 | 17,523 | 17,517 | 99.97 |
| mc2l30 | 12,561 | 12,172 | 12,141 | 99.75 |
| mc2l3 | 22,798 | 22,371 | 21,706 | 97.04 |
| mc2l24 | 66,904 | 65,342 | 64,775 | 99.13 |

Gates (death/respawn, warp, accel/Speed-domain — importer lacks a
`speed_boost` seed, MC2 debuffs, stick-unrecoverable) hold 2-3% of
offered pairs; every gated class is counted in the report.

**FIXED toward retail (sim, corpus-proven, goldens re-pinned):**
1. **Flutter draw phase** — retail tests the +63 clock BEFORE the
   tick's bump (:55294 tests the settled value, no increment in the
   handler); the port tested post-increment, so EVERY draw landed one
   pair late (the whole 220-row rand lane on mc1l0 was adjacent-pair
   swaps; f63 191/192 at the t=57/58 exemplar). `flight.rs` now tests
   then increments; rand lane 220→0.
2. **Both-strafes-held resolves RIGHT** — retail's strafe bit tests
   are SEQUENTIAL (:55783-86; MC2 EF:60793-96 identical), so 0xC in
   the move byte steps +16; the port's match treated it as release
   and decayed. Fixed in BOTH movers + regression test
   `both_strafes_held_resolve_right_not_release`. mc1l0 exemplars
   t=3189/3305 under move byte 0xE/0x2E.

**Emulated in the harness, small sim lead parked:** `sub_46840`
short-circuits WHOLE when the move byte is exactly 48 (both fires, no
move — :55759), freezing a held strafe's decay while double-firing.
`flight::mc1_move` cannot see fire state (Mc1Input has no fire bits),
so the harness pre-feeds one decay quantum on dw==48 pairs (18 on
mc1l0). Wire the fire bits into Mc1Input when convenient; MC2's
sub_5F380 has NO such short-circuit.

**OPEN LEAD (positioned, the one real family): the MC2 commit gate
refuses water-skim moves retail allowed.** mc2l3's 3% residue is one
story: 566 `tgt_speed want 80 got 0` rows + ~650 x/y/z rows, first
exemplar t=1156-1172 — the player skims z≈1276 over deep water toward
a rising shore, our `moveTest_5D0A0` port (`mc2_flight_gate`,
mc2/cave.rs) refuses where retail passed, the deep-water wedge then
legitimately fires `sub_5DD50`'s 128-unit nudge + zero_speed. The
stuck law itself is verified cave-gated correctly (water-only
off-cave, :59854-81). Likely live-play visible as phantom shoves when
grazing terrain near water on Day levels. Dig target: the gate's
water-slide arm vs retail moveTest_5D0A0 (EF:59429).

**Corroborations and small families:**
- l24's dirty windows cluster at t≈7686-7757 and t≈30-31k —
  whirlwind-funnel grabs and the enclosure portal transits: the
  funnel full-motion override corroborates the parked t=31121 lead
  (§TRANSIT-CLUSTERING), now also visible in the player's OWN pose.
  mc2l30's t≈2972-2990 yaw burst is the same signature.
- Teleport-resolve `tgt_speed` zeroes (HW t=20088, 3 rows; l24 8
  rows) — the spell-arm speed-zero law, spell-domain, not gated yet.
- Residual z/eff_pitch rows (≤ a few dozen per take, |d| ≤ ~100) sit
  on mid-pass terraform fractions (castle carve, meteor craters) —
  neither terrain@N nor @N+1 captures intra-pass edits; irreducible
  capture noise at current closure.
- mc1hwl0 stick-unrecoverable 0 / l24 155 — heavy-combat stretches
  where something else wrote the accumulators mid-pair (suspect the
  paralyze/possession writers); un-dug.

**Instruments kept:** `--example flight_dump_mc1` / `flight_dump_mc2`
(the recorded flight column per tick — the microscope every finding
above came from), CSV `kind=pose` rows. Golden re-pin:
`flight_tier_golden_state_hashes` FAITHFUL A-C (flutter phase +
strafe law; ENHANCED holds). Workspace + all 7 suites green.

**Tier 2 (banked, design ready):** chained gap-free segments — seed
once, recover input per pair from the recorded accumulators, step
continuously, report first divergence per scalar; the same loop as
tier 1 minus the reseed. Becomes the stepping stone to the full
`--replay` verifier.

## THE REPLAY VERIFIER LANDED (2026-08-07): pure input replay —
## seed once, free-run on the recovered input stream, report (never
## correct) divergence; the MC1 ±1-tick cast caveat is RETIRED

Player-directed scope cut: no resync machinery — gaps are recording
artifacts (re-anchor a segment), divergence is the finding. New mode
`mgc-conform replay <take>` (crates/mgc-conform/src/replay.rs;
docs/CONFORMANCE.md §"The replay verifier"): world imported ONCE from
the first closure, the human's flight state chained and stepped by
the same integer laws as the app (`Simulation::step`'s faithful path:
dead/falling override, Accelerate expiry edge, knock drain, mover,
death fall + dead-camera turn, `World::tick(pose, cmd)`, then the
respawn/teleport/speed-zero/end-pose mailboxes). `--pose-only` =
tier 2 (flight chained, world re-imported per pair). Headline metric:
the BIT-EXACT HORIZON.

**dw_0 IS the cast ground truth (decompile + corpus, both games).**
Writer: MC1's poller sends press-edge OR held-while-reloading/charging
via `MakeControlCommand_188A0(6, 16|32)` (:20590-20632), the consume
loop stamps `T160->dw_0` post-pass (:49021) — record N's byte is the
byte tick N+1 acts on. Readers: bit 0x10 = LEFT hand (+940), 0x20 =
RIGHT (+944), dispatched EVERY set tick (:55825/:55830) — retail cast
dispatch is LEVEL-triggered through the manifestation reload ladder
(`f48 := f50` arm at :55893; launch at `f48==f50`), autorepeat = the
held level itself. No edge detection exists in retail; replaying the
byte as a per-tick level is exact by construction. Corpus: mc1l0
312/312 and mc1hwl0 2,056/2,056 single-shot casts at exactly
byte-record +2 (zero off-phase; the +2 is pool pass order); MC2
560/560 arms carry the byte on the same record, and the press-latch
law's extra edges are UI clicks the byte correctly omits — the byte
outranks the latch. RECORDING.md updated; `--input-delay` stays only
for the legacy raw-externals path.

**Fixed in this round (all three caught BY the chain):**
1. `RetailPlayerMc2.water_ctr` decoded u16 at +610 — retail's
   `n_0x262_610` is int8_t (remc2 global_types.h); the read polluted
   the value with the 0xE0 neighbor byte on every take. mgc-formats
   now reads the byte.
2. Measured-terrain install order: the importer's terrain-replay pass
   DOUBLE-APPLIES state-derived edits on already-measured planes.
   The replay driver installs measured planes AFTER the import (the
   pose channel's proven order); mc2l0 pose horizon 0 → 597 on the
   fix. (`exec_pair`'s install-before-import is unchanged — its world
   lanes absorb the same double-apply; candidate cleanup.)
3. MC2 importer gap: `g.player_knock` now seeds from the recorded
   `moveBoost` channel (the MC1 arm's :295 twin).

**Grades (2026-08-07 corpus). Pose-only horizon = boundaries
bit-exact from the anchor; world horizon = whole-world raw compare
(NO roster — known-open families are chain-breakers by design):**

| take | pose-only horizon | first pose break | world horizon | first world break |
|---|---|---|---|---|
| mc1l0 | 567 | t=568 z−3 (castle-build terraform window) | 1 | t=2 (10,39,41) statics z +32..35 re-snap |
| mc1hwl0 | 339; seg1 **14,053 ZERO-DIVERGENCE**; seg7 1,021 | t=340 z+21 | 1 | t=2 rival col: mana 0→100, target_yaw, speed |
| mc2l0 | 597 | t=598 spell-arm tgt_speed-zero (known, un-gated) | 64 | t=65 (10,12) missing + slot 418 life/xyz |
| mc2l3 | **1,156** | t=1157 x/y/z/yaw — THE WATER-GATE exemplar (t=1156-1172), exact hit | 4 | t=5 slot 124 z −128 |
| mc2l4 | 1,656 | t=1657 z−1/eff_pitch | 0 | t=1 slots 99/100/135 action 72→73 cohort |
| mc2l30 | 246 | t=247 z−10 | 14 | t=15 slot 30 life 3→1 |
| mc2l24 | **7,686** (~5.3 min) | t=7687 y−2 | 1 | t=2 slot 16 heading |

MC1's global LCG holds LOCKSTEP across entire takes even after
entity-set divergence (rng 0 mismatches on all MC1 runs — one
unconditional draw + per-entity private LCGs); MC2's
activity-dependent draw count breaks within ~35-2,600 ticks of the
first entity divergence, as expected.

**Positioned world-chain leads (NOT dug — categorize-and-ask):**
mc1l0 (10,39,41) z re-snap on the SECOND free tick (+32..35, port
lands on ×16 grid; survives the terrain-order fix — per-pair
verification structurally cannot see it, only tick 2+ of a chain
does); mc1hwl0 rival-column mana/aim at t=2 (corroborates the banked
hw-mana import-domain lead); mc2l4's t=1 action-72→73 cohort. The
pose chain's breaks all land on ALREADY-KNOWN families — the
recovery+mover chain itself is sound.

**Notes for the next round:** replay compares RAW; a roster-aware
replay tier (skip known families when counting the horizon) is the
obvious lever on MC2 world horizons. The app/driver step the mover
BEFORE `World::tick` (player probes last tick's terrain) where
retail's carpet moves mid-pass after the terraform writers — the
structural skew is now measurable here if it ever matters. MC1
respawn stays keyboard-±1 (SPACE, no latch). Runtime: the whole
7-take × 2-mode sweep ≈ 3 min wall.

**Follow-ups agreed with the player:** (a) the in-app PORT recorder
(`source:"port"`, `input:"exact"`, optional hash channel) + `--replay`
playback — one consumer plays both port demos and transcoded retail
takes; the hash channel dates any post-fix drift for free re-record
decisions. (b) For watchable retail replays the app should derive the
faithful-tier pose from the integer carpet, not the accumulated float
yaw (a quantization risk over long sessions the integer driver
sidesteps).
