# The conformance suite

The automated regression arm of the conformance program: failing (and
sampled passing) pairs from retail recordings, encoded as committed
fixture manifests and replayed against the current sim on every test
run. It sits BESIDE the unit/golden tests — goldens pin the port
against itself (refactor guard); the suite pins it against RETAIL
(fidelity guard). Divergence triage stays in
`docs/CONFORMANCE-FINDINGS.md`; this suite is how a triaged finding
becomes an enforced expectation.

## Shape

A **fixture** IS its pair of retail states — two consecutive tick
records out of a recording. Because takes get re-recorded (and a
superseded take must not orphan its curated suite), each manifest's
pairs are **frozen**: copied verbatim into a self-contained bundle
`conformance/fixtures/<take>-fixtures.mgcr` that the manifest's
`recording` field points at. The bundle is an ordinary `.mgcr` — per
fixture it carries the tick window `t-(input_delay+2) .. t+1` (the
leading lines warm the input-delay ring; window boundaries are
ordinary gaps), so the runner is unchanged: it streams the bundle
and replays exactly the manifest's pairs — import state@t, tick
once, diff obs@t+1 — through the same core as `verify-deltas`
(`verify::exec_pair`; one implementation, by construction).

Bundles are COMMITTED, via git-lfs (`.gitattributes` tracks
`conformance/fixtures/*.mgcr`) — a few hundred KB to a couple MB per
suite — so `conformance/` is self-contained: manifest + evidence
travel together and survive any re-record. Fullsize recordings NEVER
enter git — too large, and useless in their entirety; `/recordings`
stays ignored. The full take remains the source for `verify-deltas`
runs and re-extraction.

A **manifest** (`conformance/<take>.json`, committed) records per
fixture:

- `t` — the pair.
- `status` — the expected verdict:
  - `conforming`: passed at extraction; must stay green. The
    regression corpus.
  - `open`: known-failing PORT lead; expected to fail with the
    recorded signature until the law is fixed. Carries a ledger note.
  - `capture`: known capture-domain limitation (terrain closure,
    input latency) — expected to fail, NOT a port bug. Kept so the
    day the closure gap is fixed, the whole class flips visibly.
- `sig`/`atoms` — the pair's diff **signature**: the sorted, deduped,
  slot- and value-free atom set (`missing:c,m`, `extra:c,m`,
  `field:c,m:name`, `field:player.mana`, `rng`). It captures the
  STORY of the failure, not its exact numbers, so it is stable across
  incidental drift.
- `note` — free-form triage pointer (ledger entry, family name).

Fullsize recordings and `baked/` stay local corpus data (like the
goldens' baked tree); the fixture bundles are repo artifacts. The
cargo test SKIPS with a printed note when the baked tree (or a
bundle, e.g. an LFS-less checkout with pointer stubs) is absent, so
CI without the corpus stays green and honest.

## Verdicts

`mgc-conform fixtures conformance/mc1l0.json` replays every fixture
and classifies:

| manifest says | pair now | verdict |
|---|---|---|
| conforming | passes | ok |
| conforming | fails | **REGRESSION** — exit 1 |
| open / capture | fails, same signature | ok (expected) |
| open / capture | fails, different signature | drift — warning only |
| open / capture | **passes** | **FIXED** — exit 1 until acknowledged |

The FIXED case being red is deliberate: progress must be
acknowledged, never silently absorbed. `--promote` accepts it
(status → conforming, signature cleared) and refreshes drifted
signatures, rewriting the manifest — the diff then shows up in
review as the fix's conformance receipt.

## Sizing: generic corpus + curated failures

A suite is deliberately SMALL — two ingredients, ~50 fixtures per
take:

- **The generic per-game corpus**: conforming pairs sampled across
  the whole run (`--sample-every`, default 10). This is the broad
  regression net — it exercises ordinary simulation (motion, combat,
  economy, village life) without anyone choosing scenarios.
- **Specific failures, added when they happen**: ONE minimal
  exemplar per failure STORY (≈ per ledger entry), not per
  signature. When triage names a new family, its exemplar joins the
  manifest with a note citing the entry; when the family resolves,
  the fixture gets promoted and stays as a regression guard for that
  exact story. Do not bulk-import exemplars — a thousand variations
  of one bug is one fixture plus a ledger paragraph.

## Lifecycle

1. **Extract** — after a recording session:
   `mgc-conform extract recordings/mc1l0.mgcr --input-delay 2
   --out conformance/mc1l0.json`. Failing pairs dedup by signature,
   keeping minimal exemplars up to `--max-open` (default 24);
   conforming pairs sample as the generic corpus. The extract is a
   STARTING POINT — curate it down to one exemplar per story before
   committing.
2. **Triage** — everything failing extracts as `open`. Curate:
   collapse same-story exemplars, reclassify closure-domain ones to
   `capture`, write notes citing ledger entries. Statuses are
   ledger-governed; the manifest is the enforcement, the ledger is
   the argument.
3. **Freeze** — after curation:
   `conformance/freeze_fixtures.py conformance/mc1l0.json` copies
   every fixture's pair window out of the take into
   `conformance/fixtures/<take>-fixtures.mgcr` and repoints the
   manifest; commit the bundle (git-lfs) with the manifest. It
   verifies line coverage itself (the suite reports an unreachable
   pair as "not reached" WITHOUT failing, so an incomplete bundle
   must fail at freeze time). Adding a fixture by hand later? Run
   freeze again — it prefers the full take automatically and
   accepts it as an explicit second argument.
4. **Fix** — a port fix flips its fixtures to FIXED; run with
   `--promote` and commit the manifest with the fix.
5. **Append** — a NEW failure found later (a playtest report, a new
   verify-deltas family) gets its exemplar added by hand: run
   `verify-deltas --dump <t>` to pick the minimal pair, add the
   entry with status `open` and the measured signature (run the
   suite once; it will report the drift/signature to record — or
   add with an empty `sig` and let `--promote` fill it).
6. **Re-extract** — when a recording is superseded. The frozen
   bundle keeps the OLD suite fully replayable even after its take
   is deleted — archive the manifest+bundle pair if its exemplars
   are still earning their keep. Signatures make the old and new
   manifests comparable:
   `conformance/carry_curation.py` ports statuses + notes onto the
   fresh extract by exact signature match (and prints the new/vanished
   story reconciliation); `conformance/classify_fixtures.py` then
   auto-triages the still-noteless fixtures from the verify-deltas
   `--csv` rule column (all rows capture-explained → `capture`, else
   `open`, note = matched rule ids). Recording-side utilities
   (gap scan, level-transition boundary finder, conjoined-take cutter)
   live in `recordings/*.py`.

## Rules

- Never hand-edit `sig`/`atoms` — they are measured values; use
  `--promote` to refresh.
- Statuses may be hand-edited freely; that is what they are for.
  Every non-empty `note` should point at a ledger entry.
- The suite runs the manifest's own `input_delay`/`pin_pose` —
  reproducibility over CLI convenience.
- A `capture` fixture flipping to pass is as important as an `open`
  one: it means a closure gap (e.g. a terrain channel) landed —
  promote and note the mechanism in the ledger.
- Keep suites per take (`mc1l0.json`, `mc1hwl0.json`, …); a
  re-recorded take gets a fresh extract, not an edit of the old one.

## Current suites

All five takes are 2026-07-31 re-records with the monotonic-frame-
counter recorder (tickpatch mailbox on both games) — MC2 tearing is
GONE (0 torn frames on every MC2 take; the 2026-07-30 generation lost
a third of its pairs to the Turn++ park).

| manifest | take | fixtures | statuses at last commit |
|---|---|---|---|
| `conformance/mc1l0.json` | recordings/mc1l0.mgcr (2026-07-31, gapless; 5,873 pairs, all fixture-grade) | 68 | 44 conforming / 14 open / 10 capture |
| `conformance/mc1hwl0.json` | recordings/mc1hwl0.mgcr (2026-07-31; 39,716 pairs, 15 gaps + 517 torn under the meteor storms, 39,199 fixture-grade) | 29 | 5 conforming / 21 open / 3 capture |
| `conformance/mc2l0.json` | recordings/mc2l0.mgcr (2026-07-31, gapless; 8,626 pairs, all fixture-grade) | 41 | 17 conforming / 9 open / 15 capture |
| `conformance/mc2l4.json` | recordings/mc2l4.mgcr (2026-08-01 cut of the 2026-07-31 mc2:4→30 take at t=17713 — the take's single frame skip IS the level transition; 17,711 pairs, all fixture-grade) | 24 | 0 conforming / 24 open |
| `conformance/mc2l30.json` | recordings/mc2l30.mgcr (2026-08-01 cut, the hidden cave level, rebased t=0; gapless 9,337 pairs, all fixture-grade) | 24 | 0 conforming / 24 open — §l30-churn/rng mismatches rng on 9,328 of 9,337 pairs |

Runtime: ~8 s per suite (only selected pairs execute; the stream
decode dominates). The cargo hook is
`crates/mgc-conform/tests/suite.rs`.

## The known-deviation roster

`conformance/known-deviations.json` (loaded by `verify-deltas` unless
`--no-roster`) classifies every diff row into NAMED, ledger-tracked
families so the run's headline is the UNEXPLAINED residue, not the
gross row count. The goal state on a fully triaged take: unexplained
= 0 — everything either conforming or matched to a rule.

Rules carry `status`:
- `capture` — a closure limitation of the recording (terrain channel,
  input latency, mid-frame window); not a port bug.
- `deviation` — intentional port behavior registered in
  docs/DEVIATIONS.md.
- `open` — a real, ledger-tracked port lead awaiting its fix round
  (known ≠ resolved; these are the working backlog).

Rules match first-hit in order and scope on take stem, row kind
(field/missing/extra), class, model, field name, pair-tick window,
tile rect and slot list. Every rule's `note` MUST cite its
CONFORMANCE-FINDINGS.md entry — a rule without provenance is a
suppression, not a classification, and does not belong in the file.

Guard rails:
- The runner prints per-rule hit counts (rows / pairs) on every run;
  a rule whose count jumps an order of magnitude is the regression
  signal — the roster surfaces it rather than hiding it.
- The `--csv` output carries the matched rule id in its final `rule`
  column (empty = unexplained), so offline triage can both filter
  known families and audit what a rule actually swallowed.
- The FIXTURE suite ignores the roster entirely: signatures stay raw
  so drift detection keeps full resolution.
- When a fix or closure lands, retire or re-scope the rules it
  obsoletes in the same change (the ledger's Resolved entry is the
  cue), exactly like fixture promotion.

Report lines: `N pairs fully explained (conforming + explained = M)`
is the roster-aware conformance tier; `UNEXPLAINED rows: F field,
M missing, E extra` is the number a triage session works to zero.

## The pose-phase classifier

Retail's player pose is TWO-VALUED within a tick: the carpet moves at
its pool slot in the middle of the entity pass, so handlers at slots
below it read the pre-move pose and handlers above it the post-move
pose. The recording holds ONE sample per tick, so whichever
`--pin-pose` drives a pair, one side of the carpet-slot boundary sees
a pose that is one tick removed from what retail's same-slot handler
saw — aim yaw/pitch and pose-reactive steps diverge by exactly that
skew.

`verify-deltas` therefore re-runs every dirty pair under the OTHER
pose sample (`--no-pose-alt` disables): a row that is clean in either
run is tagged `pose-phase` — capture, not a lead. Row-level
either-matching is deliberately the union of both phases, which is
the slot-split semantics without needing the split point (below-slot
rows match the `n` run, above-slot rows the `n1` run).

Wiring mirrors the roster: `pose-phase` rows leave the UNEXPLAINED
headline and count toward `pairs fully explained`; the `--csv` rule
column carries the literal `pose-phase`; the report prints the
reclassified row/pair totals; FIXTURE signatures stay raw. The tag is
runner-built (no roster entry, no ledger rule) because it is derived
per pair from the recording itself, not from a triaged family. The
button channel is out of scope — cast-consume latency is genuinely
unobservable and stays `--input-delay`-modeled with cast-edge pairs
bucketed capture.
