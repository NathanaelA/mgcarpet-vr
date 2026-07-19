# Project retrospective: one engine, two (three) games — 2026-07-19

A mid-project wrap, written after the MC2 campaign became playable end-to-end
and the playtest bank largely cleared. Not a final accounting — fixes continue —
but the picture of what varies between the games is now essentially complete,
which makes this the right moment to judge the architecture and decide on
refactors. Sources: the Phase-0 survey (docs/archive/SURVEY-MC2.md), the ROADMAP
multi-game design + phase ledgers, the pedantic review + fix-plan ledgers, the
THING-pars and stage-engine audits, and a fresh code survey of every crate at
HEAD.

---

## 1. The verdict on the bet

**The one-superset-sim decision was correct, and it was correct for the reason
the Phase-0 survey gave: Bullfrog made it first.** MC2 reuses MC1's chassis —
same LCG constants (9377/9439), same two-stack allocator, byte-identical
6-channel damage-mailbox protocol, same tile chains and disposition machinery.
The kill criterion ("if most verbs are rewritten, the superset dies") passed
decisively on day one and was never reopened. Of the ten tier-5 engine verbs,
exactly ONE was genuinely rewritten (the player movement-commit gate: MC1 walls
vs MC2 water/cave-steer), exactly as predicted. The flagship MC2 feature —
spell XP — reduced to decorators over events combat.rs already emitted, exactly
as predicted. MC2 itself passes CHASE's attack thunk as a function pointer:
the original was already moving toward our tier-3/4 dispatch design.

The hard proof is the fidelity ledger: **MC1 bit-identity survived the entire
MC2 port.** The level-005 state-hash goldens moved only on audited, deliberate
re-pins; the misfit ledger was driven to a 100.0% census (69,381/69,381 THING
records admitted across all 165 MC2 levels, zero panicking levels).

Scale, for the record: 74 commits over 16 days (2026-07-03 → 07-18), ~104K
lines of Rust. The multi-game decision landed on day 7; the entire MC2 port —
~45.7K insertions across 110 files — plus review, audits, and nine playtest fix
rounds happened in the back nine days. The port was reviewed by 11 parallel
decompile-anchored reviewers (5 P0 / ~23 P1 findings), fixed across ten
lettered sessions, and left behind a 71-file trace bank and 19 spell-audit
files. Test count grew to ~284.

## 2. How the seams deviated from the initial analysis

The consistent pattern: **the survey predicted WHAT diverges correctly, but
repeatedly drew the seam one layer too shallow.** The chassis-level claims all
held; the trouble was always in per-record and per-model substructure the
survey's ~16 chassis params couldn't see. None of these broke the
architecture — each was absorbed as a chassis param, a tier-3 wiring split, or
a "two handlers, not one `if mc2`" port — but each cost a trace, a fix round,
or (twice) a formal audit.

The full deviation ledger, in discovery order:

- **Parent/child reused as context params** (Phase 2). MC2 overloads MC1's
  portal parent/child fields into chain context; cyclic links livelocked the
  shared `walk_chain` (4e9 failed allocs overflowed the exhaustion counter).
  The frankenstein checkpoint flushed it out. Fixed with a table-len hop cap.
  The same seam resurfaced in the chain-walk seed-guard misread
  (AUDIT-THING-PARS F4): retail's guard tests the loop-invariant SEED, not the
  walked node.
- **Class-0 collision** (Phase 2 → resolved 07-17). MC2's on-disk class-0
  "Conditional Spawn" collides with MC1's class-0 empty-slot sentinel. Final
  law (better than the interim plan): class-0 rows are PASSIVE slot-indexed
  data — chain endpoints, objective targets, StageVar resolution — and can
  never materialize; runtime class==0 = consumed.
- **Table base off-by-one**: MC1's 1999-record file = engine slots 1..=1999,
  MC2's 1200-record file IS the engine table base 0 (checkpoint targets index
  it raw).
- **Cross-column damage contract (f28)** (Phase 3). MC2 has no per-channel
  vulnerability mask, so MC2 ctors must set f28 at the seam or MC1-shaped
  writers silently drop mail — and the per-creature DEFAULT is itself a seam
  (the m22 worm shipped f28=1 vs retail 3, deadening a whole machine).
- **Hitboxes live on PROJECTILE quads in MC2**, not creatures (creatures are
  faithful zero-extent targets; area/fire is the kill path). Predicted
  nowhere; discovered in Phase 3 and again in the pyramid-devour review
  finding (walked the wrong class list).
- **spawn_effect SEAM RULE**: under game-keyed dispatch, fallback MC1-shaped
  spawns fed to MC2 handlers are silently damage-less — cross-column
  constructors must resolve to NATIVE ctors.
- **Load sentinel law** (AUDIT-THING-PARS F5): spawn-at-load = DisId −1
  (0xFFFF) ONLY; disposition 0 fires at level init and is not a load marker.
  The port had drawn the load/disposition boundary in the wrong place.
- **THING-par consumption is per-model, not uniform** (the mc2:23
  spawn-in-rock class of bugs). Retail consumes authored par fields in
  per-model post-ctor arms; ctors never read THINGs (all take only position).
  Every unwired arm silently drops authored data — bounded only by the
  three-legged audit (retail matrix ∩ port matrix ∩ authored census), 6
  findings, all fixed. MC1's equivalent surface is ONE shared function —
  audited CLEAN.
- **MC2 flight was NOT parameterized** after all — the row-driven climb ramp +
  player-extension fields needed a real port (and the wizard's rows are 66/104,
  overwritten by AddPlayer, not the row-59 default the first trace assumed).
- **The stage/objective layer was far deeper than "an optional module"**: 10
  objective types (a missing one = campaign SOFT-LOCK, 41 levels were at
  risk), 9 StageVar kinds, the per-spawn bind seam, and the discovery that
  HELD ≠ frozen — retail ticks held creatures through per-kind heads
  (killable, aggro-breakable, kind-3 ambush), and held models additionally
  need their retail +7 physics wrappers (jump-cycle devils, m0 dragons).
- **Retail's own dead code is part of the spec**: the InitStages stage-0 drop
  guard is dead in NETHERW.EXE; porting the decompile literally severed a
  level's chain. Verified by disassembly.

### The recurring error classes (what porting-by-decompile actually costs)

1. **Decompile names LIE** — EuclideanDistXYZ is 2-D; "Conditional Spawn"
   never spawns; "tractor beam" hurls the player AWAY. Standing rule: read the
   body.
2. **Case-table collapse** — cascading independent `if`s transcribed as
   exclusive `else-if` chains kill states (m19 bolt-strafe, doomsday
   fall-throughs, pyramid release cases hitting `_ => {}`).
3. **Field-width/offset misreads** — u8-vs-u16 accept masks, a dropped nibble
   turning 60 tiles into 15, 32-bit LCG draws where MC2 entity rand is u16.
4. **Dead-third-arg traps** — the wrong parameter passed to a shared primitive
   (turn caps in degrees vs angle units; BEHAVIOR v_4 vs the live v_2 clamp).
5. **The trace bank itself errs** — 8 trace-corrections where the CODE was
   right and the research memo wrong. Agent conclusions get re-verified
   against the decompile; that discipline caught all of these.
6. **Verification blind spots** — the golden guard once self-skipped (PASS
   with no baked data); the rival brain had zero coverage when both P0 rival
   bugs shipped. Fixed wholesale in session J (MGC_REQUIRE_GOLDENS, hash field
   tags, observable-projection goldens).

## 3. How the taxonomy fared in the code (measured at HEAD)

The five-tier taxonomy is not just documentation — it is measurably intact:

- **`Gen` (the low-level engine, mc1/features.rs) has NO game field** — only
  `chassis` + `verbs`. `GameId` lives on `World` alone.
- **The "no `if mc2` inside a handler" rule held at essentially zero
  violations.** The only non-dispatch game checks outside world.rs are a
  spell-table selector and one pub-API guard.
- **Tier 1/2 are clean**: ChassisParams pairs, paired extracted tables
  (behavior 31↔157 rows, sprite stats, spells).
- **Tier 5 held**: flight.rs is the family-column template it was meant to be.
- **Mc1Hw held as designed**: chassis = MC1 wholesale, VerbSet::MC1HW = the
  MC1 column with exactly one arm flipped (Fire Storm targeting), data deltas
  ride the spells table, and the enum variant is deliberately last so the
  hash-feeding discriminants never moved. One banked caveat: `known_thing`
  still lacks the per-game override point HW's THING delta will need.
- **The state-hash discipline shaped the code more than any other force.**
  Exhaustive `let World {…}` destructuring makes new fields compile errors
  until deliberately hashed; MC2 per-entity state lives in
  transparent-when-empty side channels (`Mc2SlotMap<TAG>`, NightShade,
  conditional tag bytes) precisely so MC1 goldens never moved. This is why
  `World` is a fat struct with ~17 `mc2_*` fields rather than a clean
  sum type — a deliberate, correct trade.

**The measured debt is concentrated in exactly two places:**

1. **`mc1::world.rs` (14K lines, 37% of the sim) is misnamed** — it is the
   shared runtime for all three games, filed under `mc1::` for historical
   reasons. The "mc1 = verbatim remc1 port" claim in its module doc is no
   longer true of the directory.
2. **A second dispatch layer grew below the VerbSet.** The 8 verb enums cover
   the coarse verbs, but per-model routing (class-10 effects, spawn column)
   is ~80 inline `matches!(self.game, Mc2)` guards concentrated in
   `World::tick()` (~865 lines, 51 sites) and `spawn_from_thing()` (~490
   lines, 16 sites). These honor the taxonomy's SPIRIT (they are dispatch,
   not logic; handler bodies stay clean) while violating its letter (wiring
   should be declared, not inline). Plus one genuinely contorted shared
   function: `live_poses()` interleaves both games' presentation rules.

### The non-sim layers (no formal taxonomy — did fine anyway)

The app/import/render/audio layers converged on three informal patterns:
Option-field unions on ONE schema, spec-driven variant tables, and a single
`is_mc2()` funnel. Notable:

- **"One schema, per-variant bundles" HELD** — `bake_variant` is a single
  spec-driven pass over all six variants of both games with exactly one hard
  game comparison in the body. BAKE_EPOCH (content) vs FORMAT_VERSION
  (schema) separation works.
- **mgc-render is the ideal**: per-game variance = two tiny data enums
  (~6 branch sites in the whole crate).
- **The app funnels through one bool**: `App::is_mc2()` has 11 call sites;
  most of main.rs's 348 "mc2" mentions are retail-law comments, not branches.
- **HW never forked the pipeline**: it is a second archive row in a 2-entry
  table, mirroring the sim's treatment.
- The remaining duplication (twin menus, twin save codecs, twin sound-policy
  tables, twin frontend action enums) is **faithful mirroring of genuinely
  different retail artifacts**, not stretched abstraction — the globe menu
  and temple menu are different rendering paradigms in retail, and the mixer
  policy tables mirror two different decompiled switches.

## 4. Refactor decisions

Ranked by payoff-to-risk under the two standing constraints: goldens move only
deliberately, and fidelity outranks beauty. "Verify by unchanged goldens" is
the acceptance test for every sim item.

### Recommended (safe, high-clarity)

- **S1 — Split `World::tick()` into per-class dispatch helpers**
  (`tick_class5_mc2`, `tick_class10_mc2`, …). Pure code motion; must preserve
  exact iteration/side-effect order. Retires the worst hot spot (865 lines,
  51 game guards). Same treatment for `spawn_from_thing()`.
- **S2 — Rename the shared-engine modules out of `mc1::`** (world, features,
  and the shared parts' namespace) into `core::`/`engine::`. Removes the
  biggest conceptual lie in the tree; mechanical path churn, zero hash
  impact; `flight.rs` at top level is the precedent.
- **S3 — Split `live_poses()` into per-game bodies.** The one contorted
  shared function, and it is presentation-side (hash-excluded) — the safest
  possible split.
- **A1 — `CampaignRun`: replace the `mc1: Option` XOR `mc2: Option` pair with
  a real `enum CampaignSave`.** Kills a class of invalid states and the forks
  in start/persist/complete. Pure app layer.
- **A2 — Dedupe the GM-normalize block** shared verbatim by the two audio
  bake paths into one helper (output-identical, guarded by manifest SHAs).

### Conditional / deferred

- **S4 — Lift per-model class-10/spawn routing into a declared dispatch
  table** `(GameId, class, model) → handler`. Only AFTER S1, and only if
  provably order-equivalent — the inline guards encode subtle tick70-state
  precedence. Nice-to-have, not debt that hurts today.
- **A3 — `WizardConfig` as a real per-game enum** instead of 8 optional
  fields. Serialized schema → bundle it with the next forced FORMAT_VERSION
  bump, not before.
- **A4 — One conversion point for the `Game`/`CampaignId`/`GameId`
  trichotomy** (currently stringly `tag()` round-trips). Cheap housekeeping,
  do opportunistically.

### Rejected (deliberately)

- **Extracting `World`'s mc2_* fields into a sub-struct.** Touches the hash
  destructure; near-certain golden movement for a purely cosmetic win. The
  transparent-field pattern already makes the runtime cost zero.
- **Collapsing mc1/mc2 handler pairs into generics/traits.** The taxonomy
  WANTS two handlers; retail behaviors diverge. Duplication-by-design.
- **Folding the mixer's `if self.mc2` cases into the policy tables.** The
  request order is decompile-verbatim and NOT guarded by goldens; fidelity
  risk with no functional payoff.

## 5. Lessons worth keeping (beyond this project)

1. **Promote the original's own seams; don't invent abstractions.** The whole
   architecture worked because the plugin boundary was Bullfrog's own
   table-dispatch design, promoted to Rust. Where we guessed a seam ourselves
   (flight parameterization, uniform THING consumption), we were wrong.
2. **Bit-identity goldens are the refactor guard that makes a superset engine
   tractable at all.** Every carve, namespace move, and MC2 addition was
   proven MC1-neutral within minutes. The corollary discipline — hash
   transparency for new state, exhaustive destructures, deliberate audited
   re-pins — is the real architecture.
3. **A truthful degradation surface beats predictions.** The misfit ledger +
   verb-fallback telemetry turned "what did we miss?" from speculation into a
   worklist, and its drive to zero is the completion proof.
4. **Surveys buy the decision, not the port.** Phase 0 was right about every
   architectural question and still under-priced the per-model substructure by
   an order of magnitude (16 chassis params vs 71 traces). Budget accordingly:
   the survey validates the bet; the grind finds the seams.
5. **Verify the verifier.** The two near-misses that mattered most were a
   golden guard that self-skipped and a subsystem with zero coverage — both
   found by reviewing the test infrastructure, not the code.
