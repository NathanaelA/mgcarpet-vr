# PARALLEL-DIG PROTOCOL (hand this to every dig; it is short)

Written for round 138's four-dig wave and kept as the standing brief preamble.
Build the sandbox with `tools/mksandbox.sh <digid>`.

You are one of several parallel digs in the mgcarpet conformance campaign. The project is a
bit-exact remake of Magic Carpet 1 + 2. A `.mgcr` flight recording is retail DOS memory captured
every tick; `mgc-conform replay` free-runs the port on the recorded input stream and reports
every field where the port's state differs from retail's at a recorded boundary. **A divergence
is a defect in the PORT unless proven to be a registered deviation.**

## YOUR SANDBOX — USE IT, NEVER TOUCH THE MAIN TREE
Your sandbox is named in your brief: `.claude/sb/<digid>/`.
It is a **private full copy of the source** with `recordings/ gamedata/ gamedataX/ baked/
assets/ reference/` symlinked in, and its **own `CARGO_TARGET_DIR`**.

- `cd` into it. Build with `./build`. Run with `./conform <mode> ...`.
  Both wrappers pin your OWN target dir and nice themselves. **Never** run
  `cargo` or `./tools/conform` from `/home/rain/projects/mgcarpet` — that is the main
  session's binary and swapping it under a live comparison invalidates everyone's numbers
  (this cost round 99 about 30 bad measurements).
- Kill switches go **inside argv**: `./conform --env MGC_NO_FOO=1 replay recordings/x.mgcr ...`
  — never as a shell prefix.
- ⚠⚠ **ONE replay at a time, and never during a build.** Replays are heavy and the machine is
  the player's daily driver. `./build` first, wait, *then* measure.
- Your `conform` binary at handout time is byte-identical to the main session's
  (the brief quotes the md5). If you rebuild, only your copy moves.

## THE INSTRUMENTS
```
./conform replay recordings/<take>.mgcr --segmented --classify --max-diffs 40
      the graded run: segments, heads, first divergence per segment
./conform explain recordings/<take>.mgcr <t> [<slot>…]
      RETAIL's OWN t-1 → t changelog: what CHANGED in retail, not what differs.
      Births, deaths, class/owner moves, the focus slot expanded with its pointees,
      the human's input deltas, the rng draw count.  ⭐ THE HIGHEST-VALUE INSTRUMENT.
./conform dump-state recordings/<take>.mgcr <t> <slot>…            retail's raw fields
./conform dump-state … --port                    the PORT's lanes beside retail's, ≠-marked
./conform dump-state … --port --start <t0>       free-run the port from t0 instead of the seed
./conform dump-state … --at-slot <n>             sample MID-WALK, as the tick reaches slot n
./conform slice recordings/<take>.mgcr --from <t0> --to <t1> --out $TMPDIR/<x>.mgcr
      ⭐ CUT A SLICE for any head past ~5,000 ticks. Original tick numbers are kept.
      A 2,000-tick slice replays in ~1 s instead of ~20 s. See docs/PERF-CONFORM.md.
cargo run --release --example head_census_mc2  -- <mgcr> <replay-report>  species census
cargo run --release --example slot_census_mc2  -- <mgcr> <replay-report>  PER-SLOT census
      ⭐⭐⭐ RUN BOTH BEFORE BRIEFING. In round 138 the species census said
      "136 (5,25) heads" and the slot census said "SIX slots in one 2,000-tick
      window", which named the root cause (a castle split) in twenty seconds.
cargo run --release --example field_walk_mc2 -- <mgcr> <slot> <t0> <t1> [field…]
      one slot's chosen fields over a window, printed only on CHANGE
      (run it as `CARGO_TARGET_DIR=../.cargo-target-<digid> nice -n 10 cargo run …`)
```

## THE EVIDENCE STANDARD — THIS IS THE WHOLE JOB
1. **`reference/remc2/` is a DECOMPILATION, and it is not trustworthy on its own.**
   It has been edited by humans, has commented-out arguments that are DELETED FACTS, and
   carries a broken `__CFSHL__` carry macro. `EF:<n>` means
   `reference/remc2/remc2/engine/EventsFunctions.cpp` line n.
   ⚠⚠⚠ **EVERY `EF:` LINE NUMBER IN THE TREE IS STALE — DO NOT TRUST ONE, RESOLVE IT.**
   Measured in round 141 over the 1,286 citations that name exactly one `sub_`: **six** still
   land on their target. 993 point *below* it (the reference file has grown above them), and the
   drift is **not uniform** — clusters at −13, −22, −51 and a long tail around −300…−400, because
   citations were written against different upstream commits of the submodule. Two digs in round
   140 lost time to this independently.
   ⭐ **RESOLVE BY ADDRESS, NOT BY NAME.** remc2 **renames functions in place** — `sub_508E0` is
   now `sub_508E0_castle_defend_create` — so grepping the bare symbol reports "not found" for a
   function that is right there. Grep the IDA address banner instead, which is stable:
   `grep -n '000508E0' <EF>` → `//----- (000508E0) ---`, and the signature is the next line.
   ⚠ The tree cites BOTH the banner line and the signature line depending on the round, so a
   mechanical renumber would have to pick one and would silently move the other half. Round 141
   built and dry-ran such a sweep (`tools/ef_recite.py`) and **deliberately did not apply it** for
   exactly that reason. Cite the signature line in new work, and resolve old ones by address.
2. **THE SHIPPED EXE OUTRANKS THE LISTING.** `NETHERW.EXE` (MC2) is in the repo root.
   **file offset = virtual address + 0x24800.** Disassemble the actual bytes
   (`objdump -D -b binary -m i386 --start-address=… NETHERW.EXE`, or python + capstone) for
   every constant and every branch you rely on. MC1 is `CARPET.EXE`, file = VA + 0x187F8.
3. ⚠⚠ **A NEAR-IDENTICAL SIBLING WILL EAT YOUR CONSTANT.** Round 137: `sub_5FBD0` (model 42,
   stores 4) vs `sub_5FC40` (model 41, stores 6) — the port's comment cited the wrong one.
   When two retail functions differ only in immediates, disassemble BOTH and discriminate on an
   **argument**, never on shape.
4. ⭐⭐⭐ **THE HIGHEST-YIELD TELL IN THE PORT** is `.max()` / `.min()` / `saturating_*` /
   `.clamp()` where the decompile has plain (wrapping, signed, or 16-bit) arithmetic.
   Second highest: a `.max()`-style guard, a life/class/reap test, or an `if let Some` that
   retail's loop simply does not have.
5. ⭐⭐⭐ **A LAW ON ONE CALL PATH IS NOT LANDED.** Enumerate every caller of the retail function
   from the binary (scan for `e8 <rel32>` targeting it) and every caller in the port. Round 137's
   law 2 was ONE retail function reached by TWO port call paths with the field home wrong on one.
6. ⭐ **COUNT DECISIONS, NOT ROWS.** A 3-row signature is often a 1-bit divergence.

## WHAT YOU DELIVER — DIAGNOSE-ONLY, ANCHORED HUNKS
You are a **diagnosis** dig. Do NOT edit the main tree. In your own sandbox, implement and
measure freely. Then report to the main session:

- **The law**, in one paragraph: what retail does, what the port does, why they differ.
- **The citation**: decompile file:line AND the EXE bytes you disassembled to confirm it.
- **The hunk(s)**, each as `path:line` + the exact before/after text, small enough to apply by
  hand. Name every file you touch.
- **A kill switch** `MGC_NO_MC2_<NAME>` guarding the new behaviour, following the convention
  already in the tree (grep `MGC_NO_MC2_CLASS3_SCAN_ROSTER` for a recent example, including its
  doc comment style — the doc comment is where the citation lives).
- **The measured A/B in your own sandbox**: `--segmented --classify` horizon + segment count +
  head census, with the switch OFF (law on) and ON (law off). Report the take's
  **horizon, segments, and remaining head census** in both arms. If it regresses, say so.
- ⚠ **If your law needs a new `World`/`Ent` field or otherwise cannot be a self-contained hunk,
  SAY SO EXPLICITLY and name every anchor.** That is an expected, fully acceptable outcome.
  ⚠⚠ `Gen` is `#[derive(Hash)]` — a bare new field moves EVERY golden; wrap it `HashSilent<T>`.

## WRITE YOUR BRIEF'S HYPOTHESIS OFF IF IT IS WRONG
**All three of the main session's round-137 hypotheses were refuted by the digs, correctly.**
The value handed to you is the **witness shape** — which pair, which slot, which fields, which
direction — not the guess. Refuting the brief in your first paragraph is a good result.

## ⚠⚠⚠ STAY IN YOUR SLICE — THE FULL SWEEP IS THE MAIN SESSION'S, AT SESSION END (player, 2026-09-17)

**Measure your own take(s), not the corpus.** Your A/B, your head census and your kill-switch
reversion probe run on the take you were briefed on (and, if your law plainly touches a named
sibling take, that one too). **Do not run a `--segmented --brief` sweep of the whole corpus.** The
main session runs ONE full 60-take regression sweep at the END of the session, after every dig has
landed; every row it finds moved then gets its own reversion probe, which is how cross-take
fixture candidates are still caught.

The player's reasoning, in their words: *"Just because the takes are now fast and the CPUs aren't
going to get throttled doesn't mean we should blow the performance out the window … regressions
haven't really been much of a thing, so it's completely sensible to only do a full regression sweep
of everything at the end of a session, with all digs and implementations primarily minding their
own business and slice of the world."*

**There is no fixed dig cap any more.** The old "two concurrent digs" rule was set for the
player's 35 W laptop, which round 143 made "completely unusable" (load 5.94, three `rustc` plus
three replays). The project now lives on a 40-core development box. Size a wave to the work, not
to the core count, and keep the habits that were never about CPU:

- ⭐⭐⭐ **NICE IS NOT A LICENCE, AND NEITHER ARE 40 CORES.** Don't loop replays you don't need.
  ⭐ `./conform slice` a head past ~5,000 ticks instead of replaying the whole take.
- ⚠ A dig's cost is not one replay: each one BUILDS (rustc takes every core on its own) and then
  replays, often in a loop. Build first, then measure; never replay during your own build.
- ⚠ Never point two runs at one output file or directory, and namespace every temporary under
  `$TMPDIR` with your dig id — `$TMPDIR` is shared.
- ⚠ Each sandbox symlinks `reference/` (482 C/C++ files) back into the workspace; an editor
  C/C++ indexer re-walks it per sandbox unless `.vscode/settings.json` excludes it.
- 🔧 Toolchain: the repo pins rustc 1.96.1 through `rustup`. If `cargo` reports 1.85.1 you have
  the distro binary — run `. ~/.cargo/env` first.

## ⚠⚠⚠ CLEAN UP WHEN THE ROUND CLOSES — THIS IS THE MAIN SESSION'S JOB

`tools/mksandbox.sh --clean` removes every sandbox and its target dir. **Run it at the end of
every round.** Until round 138 nobody owned this and `/tmp` had quietly collected **35 GB of dead
dig sandboxes going back three weeks** — the partition was at 93% before the round started, and
four new sandboxes took it to 100% mid-session.

- A sandbox is **scratch**: reproducible from the script in seconds, never worth keeping past its
  round.
- Digs: put your own temporaries under `$TMPDIR` and do not create repo copies of your own.
- ⚠ Large one-off artifacts (pool dumps, pickles, full-corpus reports) are the other half of the
  problem — three forgotten pickles were 1.8 GB. Delete them with the sandbox.

## HOUSE RULES
- Never `cargo fmt`. Never stage or commit anything (git is the player's).
- Never `pkill -f` (it kills the agent's own shell). Kill by PID, or `pkill -x`.
- `./tools/conform fixtures conformance/*.json` exits 2 — the glob catches
  `known-deviations.json`, which is not a manifest.
- Registered deviations live in `conformance/known-deviations.json` and
  `docs/DEVIATIONS.md` — **check there before "fixing" a divergence toward retail.**
- Recordings are the ground truth and outrank the decompile.
