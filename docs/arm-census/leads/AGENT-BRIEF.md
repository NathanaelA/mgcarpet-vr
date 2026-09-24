# ARM CENSUS — shared brief for every subsystem agent (READ-ONLY research)

## Why this exists
Retail (MC1 CARPET.EXE / HW HIDDEN.EXE / MC2 NETHERW.EXE) usually has ONE routine where the Rust
port (`crates/mgc-sim/src`) has several "arms": human vs rival, MC1 vs HW vs MC2, flight vs pool,
free-run vs pinned-pose, per-model/per-class copies. Each arm was ported and fixed on its own, so a
law (a conformance fix, usually behind a `MGC_NO_*` kill switch) often lands in ONE arm and not the
others. We plan to eventually COLLAPSE twins to one routine, but we don't know which arm is right.
The player will make RETAIL RECORDINGS that exercise each arm, so the collapse isn't blind.
**Your job: map the twins in your subsystem, show exactly how they differ, and say what in-game
situation reaches each arm, so the player can record it.**

## ⛔ Hard rules
- READ-ONLY. Do not edit any file in the repo. Do not run `cargo` (the main session owns it).
  Write ONLY your output file (path given in your task).
- Do NOT rule on which arm is correct. Describe the differences neutrally; the recordings will decide.
  (You may note "retail has two distinct routines here" if the decompile citation shows two different
  sub_ addresses — that is a fact, not a verdict.)
- Prefer the CODE over comments; comments and function names in this repo sometimes lie.
- Verify each claimed difference by reading both bodies. No guessing from names.

## Inputs (in the scratchpad dir `/tmp/claude-1001/-home-rain-projects-mgcarpet/6acea123-2be0-41bd-8a95-3f6d96a358d5/scratchpad`)
- `census.json` — per retail routine (`sub_XXXXX`), the port fns citing it: `docfns` = fns whose
  doc-comment cites it (likely IMPLEMENTATIONS), `fns` = any citation (many are just calls, or
  kill-switch fns named `no_*`). Noisy — use it as a lead list, not a result.
- `killswitches.json` — every `MGC_NO_*` kill switch: its fn, where defined, and the port fns that
  consult it (`uses`). ⭐ Strongest drift signal: a law whose switch is consulted in arm A of a retail
  routine but NOT in its twin arm B. Read the switch's doc comment — it usually names the retail
  routine, the take that witnessed it, and sometimes explicitly says "the rival twin" etc.
- Repo docs: `docs/ROADMAP.md` (section "POST-CONFORMANCE REFACTOR — COLLAPSE THE PER-COLUMN ARMS"
  has the known examples), `docs/DIG-PROTOCOL.md` §5, `docs/CONFORMANCE-FINDINGS.md`,
  `conformance/known-deviations.json`.
- Fixture corpus: `conformance/<take>.json` manifests; each fixture has a `note` naming the law and
  often the retail routine / slot / entity. Grep these notes to decide whether an arm is ALREADY
  WITNESSED by a recording (e.g. a note about a RIVAL's knock => rival arm witnessed in that take).
  Take names: mc1lN (MC1 level N+1, 0-based map), mc1hwlN (Hidden Worlds), mc2lN (MC2).
- `git log -S'<symbol>'` / `git log --oneline -- <file>` can show when a law landed in one arm.
- Retail decompile listings live under `reference/` (remc1, remc1hw, remc2) if you need to confirm
  that two port arms really are one retail routine. Addresses: MC1 and HW differ (HW fn = MC1 VA
  shifted; dual names like `sub_44D30_45070` mean MC1 0x44D30 / HW 0x45070).

## What counts as a "cluster"
A set of ≥2 port code bodies that implement the SAME retail routine (or the same retail
behaviour), duplicated rather than shared. Include:
- whole twin functions (e.g. `mc2_rival_carpet_move` vs `flight::mc2_move`),
- duplicated inline hunks inside bigger functions (e.g. the same damage-intake math pasted into two
  tick functions),
- MC1-vs-MC2 twins ONLY where retail MC1 and MC2 really share the routine's shape (same logic, both
  games) — note it, but they are lower priority than same-game twins.
Skip: shared helpers that are already ONE function called from many places (that's already unified).

## Output — write Markdown to your output file, this exact shape per cluster:

### <SUBSYS>-<n>: <short name>
- **Retail routine(s):** sub_… (game/binary), what it does in game terms (one line)
- **Port arms:** each as `file:line fn_name` — column label (e.g. HUMAN/MC2, RIVAL/MC2, PINNED-POSE)
- **Status:** IDENTICAL | DRIFTED | STRUCTURALLY-DIFFERENT | UNKNOWN
  (DRIFTED = a landed law / kill switch / hunk exists in some arms, not others.
   STRUCTURALLY-DIFFERENT = different enough that it's unclear they're one routine; say why.)
- **Differences:** bullet per difference: what arm A does vs arm B, with the kill switch name if
  one gates it, and the commit/round that landed it if you found it cheaply.
- **What reaches each arm in-game:** plain player-level description per arm ("a rival wizard gets
  knocked back by a fireball while carrying a spell token", "the human carpet flies into a
  wall in MC2"). Be concrete: which game(s), which entity/spell/creature, what must happen.
- **Already witnessed?:** per arm: `yes (<take> fixture "<note fragment>")` / `no evidence found`.
- **Recording ask:** the concrete thing the player should do in retail to exercise the UNWITNESSED
  arm(s) *under the condition where the arms differ* (the difference must actually fire — e.g. if
  the drift is in the stuck-nudge, the rival must actually get stuck). Game + suggested level if
  one is natural + what to do + roughly how long. If the difference is in a lane nobody can see
  (ungraded), say so.
- **Confidence / notes:** anything uncertain.

End your file with a short summary table: id | name | status | arms | recording needed (y/n).
Also list at the end any clusters you saw that belong to ANOTHER subsystem (so we don't lose them).

Aim for completeness in your subsystem over polish. Big files: `engine/world.rs` is 60k lines —
use grep, read by line range. It's fine to take a long time.
Return to the caller only a 10-line summary + the output path (the file is the deliverable).
