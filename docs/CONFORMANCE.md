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

A **fixture** is a reference, not a copy: `(recording, pair t)`. The
`.mgcr` tick records are self-contained (RECORDING.md), so the runner
streams the source recording once and replays exactly the manifest's
pairs — import state@t, tick once, diff obs@t+1 — through the same
core as `verify-deltas` (`verify::exec_pair`; one implementation, by
construction).

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

Recordings and `baked/` stay local corpus data (like the goldens'
baked tree); the cargo test SKIPS with a printed note when either is
absent, so CI without the corpus stays green and honest.

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
3. **Fix** — a port fix flips its fixtures to FIXED; run with
   `--promote` and commit the manifest with the fix.
4. **Append** — a NEW failure found later (a playtest report, a new
   verify-deltas family) gets its exemplar added by hand: run
   `verify-deltas --dump <t>` to pick the minimal pair, add the
   entry with status `open` and the measured signature (run the
   suite once; it will report the drift/signature to record — or
   add with an empty `sig` and let `--promote` fill it).
5. **Re-extract** — when a recording is superseded. Signatures make
   the old and new manifests comparable.

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

| manifest | take | fixtures | statuses at last commit |
|---|---|---|---|
| `conformance/mc1l0.json` | recordings/mc1l0.mgcr (2026-07-30, gapless 5329 pairs) | 47 | 35 conforming / 8 open / 4 capture |

Runtime: ~8 s per suite (only selected pairs execute; the stream
decode dominates). The cargo hook is
`crates/mgc-conform/tests/suite.rs`.
