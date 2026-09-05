# mgc-conform PERFORMANCE — measured in round 105, TO REVISIT AFTER mc2l22

## ✅ STATUS 2026-09-05 — LANDED (the session after 105)
- **`mgc-conform slice <take> --from t0 [--to t1] --out <slice.mgcr>`** — the
  player's slice idea, built as specified below: header + `capture.slice`
  provenance, terrain re-based at the first record, ORIGINAL tick numbers,
  every other record byte for byte, a `⚠ SLICE` banner on every seeding
  instrument. Cutting 2,200 ticks of mc2l24 at t=7381: 9 s, 15 MB.
- **`Recording::skip_to`** — option 1 (byte-scan to SELECT, serde to parse)
  under `dump-state`, `explain`, `trace`, `ground-audit`, `replay --start`,
  `verify-deltas --start`. `dump-state mc2l22 54000`: 70 s → 37 s; the
  ~19 s zstd inflate is the floor, so option 2 (an offset index) is moot
  once slices exist — a slice IS the index. Option 3 (binary sidecar) stays
  deferred until the full-take sweep is the bottleneck.
- **Doctrine** — docs/CONFORMANCE.md "Recording slices": NO dig reads a
  full take past t≈2000; the main session keeps one `H-200..H+2000` slice
  in `$TMPDIR`; re-cut when the horizon moves (a slice's content never goes
  stale). The full take is for the sweep, the suite and the horizon query.
- **Proof** — slice vs full-take window byte-identical on `replay` and
  `--segmented --classify`; `verify-deltas` identical modulo `--start`'s
  own `pair N` announcements; sweep + suite byte-identical vs the committed
  baseline (see the session note).
- ⚠ Measured while validating: **a `target/release` binary is NOT HEAD** —
  the one preserved as "before" predated the day's commit by 5 h and
  differed from HEAD in MC2 laws (one pool-slot-0 byte at the anchor).
  The true BEFORE is `git archive HEAD | tar -x` + build.


## THE MEASUREMENT (not a guess)
`perf record` on `replay recordings/mc2l22.mgcr --stop-at-divergence`:
```
~50%  serde_json  (skip_to_escape / ignore_value / ignore_integer / ignore_str)
~20%  base64 internal_decode
~11%  ZSTD_decompressSequences
 1.4% mgc_sim::mc2::roster::m15_tick   <- the TOP simulation function
```
Process shape: **RSS 23.6 MB, 0 major page faults, 1 voluntary context switch.**
100% CPU-bound, single-threaded. It is NOT a memory or I/O problem, and NOT a bug.

## WHY
`recordings/mc2l22.mgcr` is **532 MB compressed → 29,068,288,390 bytes (29 GB) of JSONL**,
a **54.5x** ratio. That is ~443 KB of JSON parsed per simulated tick. Whole corpus:
5.53 GB compressed → **~302 GB uncompressed** (605 GB free on /home, so an uncompressed
mirror would fit but eat half the free disk).

## THE OPTIONS, RANKED BY WHAT THEY ACTUALLY BUY
1. ⭐⭐ **BYTE-SCAN TO SELECT, SERDE TO PARSE** (player's idea, round 105).
   For TARGETED reads — `dump-state <t> <slot>`, `verify-deltas --dump <t>`,
   `--start <t>` — scan the inflated stream for the tick key as a BYTE PATTERN and hand
   only matching lines to serde. Kills most of the ~70% JSON+base64 share on every line
   we do not want. `dump-state 54238` currently parses 54,238 ticks of JSON to read ONE.
   ⚠ **Frame it as SELECT, not EXTRACT.** A regex that pulls values out of JSON is
   fragile (escapes, key order, nesting); a scan that only decides "is this line worth
   parsing" keeps serde as the validator and cannot silently mis-read a record.
   ⚠ Does NOT help full-take `replay` / `verify-deltas` — those need every tick.
2. **A TICK OFFSET INDEX** (sidecar). The only thing that fixes SEEKING: zstd is a
   stream, so reaching tick N today means inflating everything before it. Pairs with (1).
3. **A COMPACT BINARY SIDECAR** — pre-decoded records, no JSON, no base64. Attacks all
   three costs including full-take sweeps. Should land far below 302 GB because base64 is
   a 4/3 expansion and JSON field names are pure overhead.
4. ⛔ **AN UNCOMPRESSED JSON MIRROR — REJECTED.** ~302 GB for only the 11% zstd share;
   the ~70% JSON+base64 cost survives untouched.

## WHY IT MATTERS ENOUGH TO DO
The campaign re-reads the same 29 recordings hundreds of times. A full-corpus
`replay --segmented --brief` sweep is ~25 min and is run several times per session
(every law needs one for its neutrality proof); a full-take `verify-deltas` census is
~7m30s. Even 5x turns a 25-minute sweep into 5 minutes.

## ⚠ ANY CHANGE HERE IS A TOOL CHANGE
**THE NEUTRALITY PROOF IS MANDATORY: whole-corpus sweep + the fixture suite,
byte-identical before and after.** A decoder that is subtly wrong would silently
corrupt every measurement the campaign makes.

## RELATED, ALREADY BANKED THIS ROUND
- `nohup … &` inside a tool call does NOT survive the call returning (measured).
  Use the Bash tool's `run_in_background: true`. Not an OOM, not a crash.
- 7 concurrent `mgc-conform` from 5 digs is normal — digs run >1 measurement at once.
  Each pegs one core for minutes.

---

# ⭐⭐⭐ THE BEST IDEA: RECORDING SLICES FOR DIGS (player, round 105)

> *"The digs get progressively slower and slower as we move the horizon further. The first
> few digs took 30 minutes, now a session with a couple of digs can take 8 hours. All of
> this is because all workers have to skip through the first 40K or so ticks. But they
> also don't have to, because at that point the free-run horizon is there. So what we need
> is the ability to make a slice of a recording and throw that to the subagents to only
> work on a range of ticks. Those can be a few hundred ticks at most and stored in temp
> for the agent to pick them up. It'll make the digs an order of magnitude faster and the
> CPU churn practically nonexistent. We need to make sure the slice contains all the
> necessary information, and the agent has info on how to get more if it becomes necessary."*

**This beats every option above, because it attacks the cost that GROWS.** Options 1-3 give a
constant factor; this one removes work proportional to the horizon. And the horizon is
exactly what the campaign keeps pushing outward — mc2l22's went 4,661 → 15,668 → 54,238
this round alone. **The tool gets slower precisely as the campaign succeeds.**

## WHY IT IS CHEAP TO BUILD — THE FORMAT ALREADY SUPPORTS IT
Every tick record carries **full state**, which is why `verify-deltas` can import state@N and
tick, and why `dump-state --start <t0>` already works. So a slice is mostly a **copy of a
contiguous run of records plus the header**, not a reconstruction.

## WHAT A SLICE MUST CARRY (the player's "all the necessary information")
1. The recording **header** — game, level, human slot, seed, input-delay.
2. **The record-0 terrain base** (the MEASURED planes). ⚠ NOT optional: `terrain=measured`
   appears on every take's BRIEF line, and terrain laws (three of round 105's five!) are
   unreadable without it. A slice that drops it silently changes the answer.
3. The **tick records** for `[t0-margin, t1+margin]`.
4. The **input stream** for that range (`replay` is a pure input replay).
5. **Provenance metadata**: source take, original tick numbers, the margin used, and the
   command that cut it — so a head at slice-local t maps back to the real tick, and so a
   stale slice can be detected after a law lands.

## ⚠⚠⚠ THE CAVEAT THAT MUST SHAPE THE DESIGN — DO NOT SKIP THIS
**A slice is a `--start`-shaped view, and this campaign has ALREADY BANKED that
`--port --start <t0>` ≠ `--port --segmented`.** `anchor_mc2` (`replay.rs` ~644) re-imports
the pool into the **same reused `World`**, so a segmented reset carries un-imported
**port-side side channels** across, while `--start` builds a fresh one. Round 104 lost time
to two "tool bugs" that were exactly this.

**Consequence for slicing:**
- ✅ **LOCAL / pair-dirty heads slice safely** — the cause is inside the pair.
- ⚠ **INHERITED / pair-clean heads DO NOT.** Their cause is upstream state, by definition.
  A slice starting at t0 seeds from t0's recorded state and therefore **cannot see a
  divergence born before t0** — it will look clean and the dig will conclude "no bug".
  Round 105's own t=15668 was INHERITED and its true cause was **26 ticks earlier**; W1-B's
  head at t=21509 was caused **243 ticks earlier**; W1-D's t=47659 was caused **5,223 ticks
  earlier** (t=42436). **A few-hundred-tick slice would have MISSED the last one entirely.**
- ⇒ **The margin is not a constant.** Size it from the head's classification, and make the
  slice tool REFUSE (or loudly warn) when asked for a slice too tight around an INHERITED
  head. Better: let the dig widen it — hence the player's "the agent has info on how to get
  more if it becomes necessary".

## THE INTERFACE THE DIG NEEDS
- `mgc-conform slice <take.mgcr> --from <t0> --to <t1> [--margin N] --out <slice.mgcr>`
- The slice's BRIEF/classify output must **print original tick numbers**, never slice-local
  ones, or every citation in the ledger becomes ambiguous.
- Every dig brief must state: **which take and range the slice came from, that it IS a
  slice, and the exact command to cut a wider one.**
- ⚠ A slice must be **re-cut after a law lands** — a stale slice is the same hazard as the
  stale census that steered three digs wrong in round 99.

## VALIDATION BEFORE IT IS TRUSTED
**The neutrality proof for a slice tool is not the corpus sweep — it is an EQUIVALENCE
proof:** for a set of known heads, `replay`/`verify-deltas` on the slice must produce
**byte-identical** rows to the same range of the full take. Prove it on at least one LOCAL
head and one INHERITED head with adequate margin, then on one INHERITED head with a
deliberately-too-small margin to demonstrate the failure mode is loud, not silent.

## EXPECTED WIN
A dig on the t=54238 head currently inflates and parses ~24 GB of JSON to reach its tick.
A 500-tick slice with a 5,000-tick margin is well under 1% of that. **Order-of-magnitude is
if anything conservative for late-horizon heads, and it is 5 digs' worth of CPU at once.**
