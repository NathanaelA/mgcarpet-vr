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
  the wake-law round); RNG (1,1) on every pair. Roster-aware:
  3,042 conforming-or-explained, UNEXPLAINED 17,246 field / 124
  missing / 260 extra rows. (The `mc1l0-village-regrade` rule hit
  0 rows — its t/rect scope was the OLD take's regrade event;
  retire or re-scope on the next roster pass.)
- **mc1hwl0**: full HW take under meteor weather, ticks 0..39,800
  with 15 gaps (69 frames — heavy-animation skips; a skip-free HW
  run is not achievable) + 517 torn, 39,199 of 39,716 pairs
  fixture-grade, **48 conforming** (46 before the wake-law round);
  RNG (1,1) on 39,171 pairs, retail >16-draw bursts on 28. Terrain
  closure still owns ~every pair (`mc1hwl0-terrain-z` explains
  2.12M rows / 39,133 pairs; 2.28M field rows unexplained — HW
  progress keeps reading from per-family totals + the story suite,
  not the pair headline).
- **mc2l0**: gapless 8,627 ticks, 8,626 pairs, **0 torn** (take-2
  on the rate-limited recorder tore 1,105 of 3,640), all
  fixture-grade, **240 conforming** (167 before the cave-rand
  round-2 tick-top draw); rng mismatch on 3 pairs only.
  Roster-aware: 5,508 conforming-or-explained, UNEXPLAINED 8,846
  field / 70 missing / 281 extra.
- **mc2l4 + mc2l30** (CUT 2026-08-01 from the single conjoined
  `mc2l4,30.mgcr` take at t=17713; the take's SINGLE frame skip
  17711→17713 is exactly the level transition — the tick fn never
  ran during the load — so both cuts are internally gapless, and
  the embedded level record flips at the cut as before): mc2l4 =
  17,711 pairs, 0 torn, all fixture-grade, 0 conforming raw but
  **13,330 of 17,711 pairs roster-explained (75%)**, rng mismatch
  on 160; mc2l30 = 9,337 pairs, 0 torn, all fixture-grade, rng
  mismatch **202 of 9,337 pairs** (9,328 before the cave-rand
  structure round 2 — see Resolved; the residual is churn-tick
  draw-count skew riding §l30-churn), **6,320 pairs
  roster-explained** (was 1). Suite note: one mc2l4 exemplar's signature differed
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
  family (PORT-LAW, two specs banked)**: after the cave-tail fix
  (Resolved) ~22% of l30 pairs still mismatch rng, all
  count-mismatches on churn-heavy ticks. Two mechanisms: (a) the
  MC2 per-tick reap lag — retail's fire/riser disable
  (`DisableEntityDrawing04` byte[1]|=4) is class-zeroed within the
  SAME tick by the ApplyEvents fallthrough (Events.cpp:548 →
  sub_57F20), the port frees next-tick → +1 extra-in-port per fire
  death (the bulk of (10,0) 8.7k extras + the 3 (10,64) riser
  extras at t=1) — the MC2 face of the MC1 tick-top-reap law; (b)
  the per-ENTITY `rand_0x14 += counter` sites (EF:13140/13220/
  20521), unmodeled (multipart.rs/doomsday.rs notes) — now
  implementable since the counter is anchored. Downstream of (a)+
  (b): divergent `new_event` seeds → the (10,0)/(10,14)
  missing-in-port spawns.
- **§l30-terrain — the (14,5) flat-512 plateau (CAPTURE, with a
  port-side check owed)**: 12 of the 14 (14,5) markers sit exactly
  −1664 (retail 2176 plateau at tiles (160-171,194-205), port flat
  512); nearby slots track terrain within ±32. Both sides ground-
  snap faithfully — the port's mc2:30 heightfield simply lacks the
  plateau. OWED: check whether the plateau is load-time (the dis-0
  (10,64) riser raise — then the port's conformance world-build
  skips a load-time raise = fixable) or runtime-terraformed (pure
  capture). The l4 face of the same question: the (5,4) ARCHER
  family walks at a CONSTANT −192 z from t=0 (slot 210, byte-
  identical dynamics) — a pristine-plane datum gap at its site,
  present before any runtime edit can exist. (5,4) XP-scroll z, (14,3) −16, (15,19) token-fall
  (slot 92: port clamps up to its pristine 1296 floor while retail
  falls to 288) are the same terrain-closure story.
- **§castle follow-ups** (split from the resolved phantom-upgrade
  lane): (a) the (10,42) painter's parent @0x28 is NOT projected by
  `obs_project_mc2` (owner retail-297-vs-0 rows; the "@0x28 nonzero
  only on class-15" comment is wrong for painters) — obs-schema
  gap; (b) the (3,3) stage-piece −128 z residual post-rise —
  re-measure now that the phantom upgrade is gone; (c) the (5,1)
  at slot 92 killed at t=0 by `mc2_building_clear_tile` (build
  footprint clear) while retail's construction hasn't cleared that
  tile this tick — build-window timing; (d) player.mana_max
  claim-census within-tick (the standing mc2l0 lead, NOT a castle
  ripple — the mc2l4 castles are rival-owned).
- **§wander-drift residual — (5,0)/(5,3): RE-RULED 2026-07-31**
  (see Resolved: KINEMATICS ROUND rulings): the walker turn law is
  byte-exact — the smooth heading drift is capture (chaotic
  amplification, rand-matched). The remaining PORT lead here is the
  **flyer z-bob** (±8..56 airborne altitude offset on the multipart
  chains) — the M0/M3 altitude source, untraced, own item; plus the
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
  behavior (PLAYTEST OWED).** Decompile dig closed every open
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
