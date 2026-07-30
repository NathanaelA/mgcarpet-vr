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
- **mc2l0**: 2026-07-29 full take, 3641 ticks, 0 gaps, check-decode
  3641/3641 exact. **IMPORTER WIRED + TRIAGED 2026-07-30** (no
  input delay — the take carries no input channel): 3640 pairs,
  1105 torn by the MC2 phase-byte gate (see the MC2 section below),
  2535 fixture-grade, **7 conforming** after the first fix (the
  goat bleat draw). Global LCG near-exact out of the box: 62/3640
  pairs mismatched, draw counts matching pairwise across the whole
  0..16+ histogram.
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

## MC2 take-2 (2026-07-30 re-record; FIX ROUND 1 LANDED 2026-07-30)

The re-recorded mc2l0: **11,524 ticks gapless, check-decode exact,
`channels.input: "raw"`** (the MC2 input frame validated live — mode 7,
arrow keybinds), spell upgrades + end-to-end level completion. 11,523
pairs → 7,762 fixture-grade (33% torn). Suite re-extracted per doctrine
(`conformance/mc2l0.json`, 24 exemplars, 23 open / 1 capture; sigs
re-promoted after the fix round). **Still 0 conforming — by
construction**: the §terraform capture family (village growth regrades
the hill at ~t=751; house z re-snaps both sides, ours to the pristine
plane) puts (10,45) z rows on every later pair — 186k of the 249k
remaining z hits. Port-side conformance now lives in the t<751 window
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
- **§casts misfire**: the port casts when retail does not (t≈3: port
  spawns (9,17) into slot 208 where retail has smoke; `mana: port 50`
  = `mc2_spawn_cast_proj`). Suspect: `sample_cmd_mc2` maps raw
  buttons with no PANE hit-test (clicks on the control pane are UI,
  not casts — the input frame carries cursor + cursor-at-press for
  exactly this). The input-delay re-sweep 0..3 is UNBLOCKED now
  (class-15 noise gone) and owed after the pane fix.
- **Cross-pair StageVar leak** (suite self-drift): still owed —
  import the live StageVars2 rows @0x365F4 (in the closure) per
  pair. The BOOK half of the leak is fixed (str_611 imports).
- **player.mana regen cadence — narrowed**: the pending delta @0x88
  applies mana@N+1 = mana@N + d88@N on almost every pair, EXCEPT a
  freshly-stamped −100 survives ONE extra frame before applying
  (measured pairs 0→1 and 16→17; the port applies immediately →
  ±100 on ~232 pairs). Needs the EF wizard-regen/castle-drain order
  trace (the MC2 twin of the mc1 entry-5 cadence).
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
- **§wander — (5,1)/(5,13) turn law** (t=1705/1587): heading ±22
  and ±45 step families on grazing goats and villagers — a turn-step
  or turn-cadence mismatch in the held graze/walk legs, NOT rand
  (their streams now match post-bleat-fix).
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

## Resolved

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
