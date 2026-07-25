# mc2:04 StageVar ground truth — the archer/skeleton skirmish, to the bone

**Date:** 2026-07-25. **Question:** what does the level-004 map data actually
say about the scripted battle, what does the *shipped* engine actually do
with it, and how much of the port's behavior (commit `ec9d2295`) is fact vs
interpretation? Prompted by community reports that retail is
non-deterministic: in some runs the human archers leave the crater and
march, in others they stay put.

**Sources, by authority (strongest first):**

1. **Shipped `NETHERW.EXE`** — disassembled from `gamedata/Magic Carpet
   2/game.gog` (Mode-1 2352 sector carve → ISO → 7z; LE at file
   `0x34800 + (linear − 0x10000)`; calibrated on `sub_68C70`@0x8D470,
   `sub_1BF90`@0x40790, cross-calls `sub_1D8C0`/`sub_583F0` resolve exactly).
2. **The authored level file** — `GAME/NETHERW/CLEVELS/LEVELS.DAT` entry 4,
   RNC-unpacked via the crate's own `dattab`/`rnc` (26116 bytes exact).
3. **The retail memimage** — `reference/remc2/.../memimages/regressions/`
   `level5/` (1-BASED: level5 = archive index 4), 20 frames ≈ 20 ticks from
   load, a real retail-engine run's memory.
4. **remc2 reconstruction** — used for structure/typing only; its three
   `//fix` guards in this subsystem are **confirmed absent from the shipped
   binary** (instruction-level byte scans; `0xae02` appears nowhere in the
   code section).

---

## 1. The authored data (raw bytes — FACT)

StageVar block at file offset 0x65AC, 11×8 bytes, 4 live rows (0 and 5–10
all-zero). Loaded **byte-identical** into the retail engine (memimage RAW
block +0x3647A matches the file exactly).

```
sv[1] @0x65B4: c3 00 46 00 6f 00 00 00   kind 3, byte0 flags 0x80|0x40, chain 0, hold=70,  data=111
sv[2] @0x65BC: 89 03 80 00 46 00 00 00   kind 9, byte0 flags 0x80,      chain 3, hold=128, data=70
sv[3] @0x65C4: 43 00 00 00 31 00 00 00   kind 3, byte0 flags 0x40,      chain 0, hold=0,   data=49
sv[4] @0x65CC: c4 00 47 00 41 00 00 00   kind 4, byte0 flags 0x80|0x40, chain 0, hold=71,  data=65
```

Referenced THINGs (table at 0x443, 20 B/slot; all with zero
dis/stage/par fields):

```
thing   0: NULL (class 0, all zero)
thing  49: (5,9) skeleton (175,8)      thing  65: (5,9) skeleton (174,8)
thing  70: (5,9) skeleton (173,13)     thing  71: (5,3) worm    (177,1)
thing 111: (5,4) archer  (130,58)      thing 128: (5,4) archer  (129,60)
```

Cast census: **33 archers** (5,4) in the crater (126-133, 53-60);
**39 skeletons** (5,9) north (171-182, 5-13); **10 worms** (5,3) map-wide
(the prior session's "2 worms" undercounted — only the northern few are
relevant; sv[4] subtype-binds exactly 4 of the 10).

The port's importer copies all 11 rows verbatim and slot-aligned; nothing
is skipped or reinterpreted at import time. **The port's kind/flag decode
(`0x80→&1 subtype`, `0x40→&2 watch-model`) is vindicated by retail's own
live tables** — the memimage live flags are 0x03/0x01/0x02/0x03 exactly as
the port computes.

## 2. What the shipped binary does with a watch row (instructions — FACT)

The live-row union at `StageVar+4` is **polymorphic by flag &2**
(`sub_12780` @ file 0x36F80, kinds 3/4/5/8/9):

- **&4 set** → already FIRED (latched).
- **&2 set** → the union's low 16 bits are read as an **index** into the
  per-model chain-head array (`cmp dword [esi+ecx*4+0x9603],0`): **fire
  when no live entity of that model exists**. Pass-1 of the loader stores
  the watched thing's **model** there. This is a clean, bounded,
  deterministic **model-extinction watch**. No garbage anywhere.
- **neither** → the union is read as a **raw 32-bit pointer** and
  dereferenced behind only a null check: `life_0x8 < 0` or byte
  `+0x0D & 4` (being-removed) → fire. **No pool-range guard exists in the
  shipped binary** (remc2's range guard and the `sub_1D700` `0xae02` bail
  are both `//fix` additions, absent from retail — verified instruction by
  instruction and by byte-scan).

Binding (pass-3, `sub_12100` @ 0x36900, re-run on **every class-5 spawn**,
caller EF:33030): when the spawning entity matches the raw row's 16-bit
key, the row's union is overwritten with the **live entity pointer** and
the FIRED bit is cleared. The savegame relocator (`sub_55100`) serializes
exactly this pointer as `slot × 0xA8` — the pointer store is genuine
retail, not a remc2 repair.

The union writers are exactly: pass-1 init, pass-3 bind, save/load
relocation, and struct mirror copies. **There is no per-tick gameplay
writer** — the kind-9 "graze point" hypothesis is refuted; `sub_12500`'s
kind-9 proximity arm only *reads* the union (as coordinates, vs the held
entity's own position, ≤12 tiles — for level 004 values this never
matches; it is not a rescue path).

## 3. The four rows, decoded against that machinery

| row | semantics (data + binary) | grade |
|---|---|---|
| sv[1] | kind-3 hold of all m9 skeletons (subtype of thing 70); **&2 model-extinction watch on m4 (archers)** — fires when archers extinct. Memimage: binds all 39 skeletons at tick 16 (cadence), watch_model=4 live. | FACT |
| sv[2] | kind-9 hold of all m4 archers (subtype of thing 128), **chain → slot 3**. No &2: its release gate is the **raw-pointer deref arm**, keyed on the authored 16-bit value 70. | FACT (gate identity) |
| sv[3] | kind-3, hold_word=0 (holds nothing at load), **&2 model-extinction watch on m9** — the chain **target**: archers land here when sv[2] fires and shadow-march the skeletons; releases survivors at m9 extinction. | FACT for the row itself; "chain target" is inferred from sv[2].chain=3 + confirmed chain machinery (memimage cannot confirm — nothing fires in its 20 ticks) |
| sv[4] | kind-4 hold of the northern worms (subtype of thing 71; binds 4 of 10), &2 watch on m9. | FACT |

## 4. sv[2]'s release gate — THE REVERSAL: the death watch WORKS in retail

This is the row that decides whether the archers leave the crater, and
the audit's headline finding: **the prior session's "garbage deref /
fire-at-bind" mechanism story is refuted at every step.**

- **The pass-3 match key is the THING INDEX** — shipped `sub_12100`
  computes `(spawning_thing_ptr − D41A0_0+0x30311) / 20` (divisor 0x14,
  template base; instructions quoted in the audit transcripts) and
  compares it against the raw row's 16-bit key. For sv[2], key = 70:
  **when thing 70 spawns, the row binds a live pool pointer to it.**
- **The memimage proves the binding worked in a real retail run.** The
  live union reads `0x003640F6` (frame 0) and `0x00007230` (frame 19) —
  which look like garbage until you notice `0x3640F6 − (0x6E8E + 174×0xA8)
  = 0x356038` = the D41A0 struct's base address in that run (it is the
  second number in the memimage filename). Both values denote **pool slot
  174** — frame 0 as an absolute pointer, frame 19 in relocated
  slot-offset form (a dump-serialization artifact, not a game-state
  change). And pool slot 174 at both frames contains: **class 5, model 9,
  life 1000, position (173.5, 13.5) — thing 70, the vanguard skeleton,
  alive.** Verified directly from the dump bytes.
- **So the shipped semantics of sv[2] are the authored intent,
  deterministic**: *archers hold in the crater until the vanguard
  skeleton actually dies; then the gate fires and chains the flock onto
  sv[3], the march row.* Before the bind the union is zeroed by pass-1
  and the deref is null-guarded — no coin at load. The memimage's
  "nothing fires in 20 ticks" is exactly right: the vanguard is alive.
- **Where the raw-pointer fragility is real but latent**: the deref has
  no pool-range guard (only a null check), pass-3 re-runs on **every**
  class-5 spawn (a runtime spawn whose template arithmetic accidentally
  matches the key would silently REBIND the watch), and a stale pointer
  after slot reuse is dereferenced unchecked. These are genuine bug
  surfaces — but on level 004 at load, none of them is active.

### So where does the observed retail nondeterminism come from?

Not from memory. A dedicated instruction-level sweep for load-time coins
proved four negatives in the shipped binary:

1. `InitStageVars_11EE0` **memsets the whole live table** (88 bytes,
   file 0x366E0 — remc2's EF:4636 memset is genuine) and re-zeroes the
   union per row — no cross-level leakage through the static struct.
2. `sub_12780` has **exactly one caller**: the per-tick
   `UpdateEntities_57730`, scanning **all rows every gameplay tick** —
   no per-objective row selection, no load-time evaluation. (The port's
   active-stage gating is a port-side model, not retail.)
3. The load sequence is `InitStages → InitStageVars → spawn+bind →
   settle` with **no scan window** before the bind; the bind clears
   FIRED, so every bound row enters tick 1 clean.
4. `sub_12780` is the **only** FIRED-bit writer in the code section;
   the stage/objective counters are freshly zeroed each load.

The gate is therefore fully deterministic *given its input* — and the
input on level 004 is the **living vanguard skeleton** (memimage-proven
bind). The retail bimodality (player observation: the archers either
march more-or-less immediately, or never — no mid-battle mode) then
falls out of the *release condition quantizing on the battle*:

- The gate fires when **thing 70's skeleton — one specific creature in
  the marching horde — dies.** Retail combat is strongly
  non-deterministic (player-proven, ~12-13 survivor spread).
- **He dies early** (he is the ford-nearest vanguard walking point-first
  into a 33-archer kill zone): the gate fires while the rest of the
  horde is still crossing — the archers visibly rise and march out to
  meet them → the "march more or less immediately" mode.
- **He dies late** (arrow collateral spreads the volleys; he survives
  the approach): by the time he falls, the skeletons are at/in the
  crater — and if m9 is extinct or nearly so at that moment, the chain
  lands the archers on sv[3] whose model-extinction watch releases them
  instantly → **no visible march ever**; the archers fight from the
  crater and stand down → the "never march" mode.

One creature's time-of-death, quantized by the choreography into two
observable outcomes. (Second-order caveat, unresolved: pass-3 re-runs
on every runtime class-5 spawn and could in principle rebind a row —
no runtime class-5 spawns are known to matter on L004's opening.)

### But what kills the vanguard EARLY? (the player's objection)

The crater is ~60 tiles from the skeleton spawn and arrow range is 20
tiles — in a clean run **nothing can kill the vanguard early**, so the
vanguard-death-timing model cannot by itself produce a
march-at/near-load mode. Port probe evidence (headless mc2:04,
39 skeletons tracked): every skeleton spawns at full life 1000 (the
load settle does not wound them — deterministic early death ruled
out); zero deaths during the ~260-tick approach; and — a fidelity
bonus — the port allocates the vanguard to **pool slot 174, the same
slot as retail's memimage** (allocation-faithful).

Candidates for the early kill, ranked by evidential fit:

1. **The kind-4 worm join-arm misfire (leading).** sv[4]'s held worms
   run `sub_1D700`'s watch arm: resolve the nearest watched m9
   skeleton and adopt **its target** — read from `word_0x96_150`,
   which on a held/idle skeleton is **uninitialized pool garbage**.
   This is not hypothetical on this level: remc2's own level-5
   regression replay hit exactly this junk (the literal
   `if (v4 == 0xae02) return;` bandaid was added FOR this level's
   replay). The shipped binary has no such guard — the worm adopts a
   garbage entity reference. Depending on what the junk resolves to
   (pool-state / play-path dependent — the community's "conventional
   memory" instinct lands HERE, one row over from where it was first
   aimed), the worms either stay inert (the player's own retail runs:
   "worms just crawl along") or open fire on something — and the two
   northern worms sit 8-12 tiles from the vanguard. A worm shelling
   the column kills thing-70's skeleton near spawn → the gate fires →
   the archers march "more or less immediately."
2. **Ford jam / water death** (the player's terrain hypothesis): the
   boxed-in suicide and off-lane water lethality are real
   (byte-verified move core), and under the hold — archers absent
   from the ford — all 39 skeletons pack the 3-lane funnel, unlike
   the fire-at-bind probe where the armies met mid-ford. Plausible
   but unsupported by the one probe run (0 crossing deaths); needs a
   literal-watch port run to test.
3. **Player interference** — a human strafing the column early kills
   the vanguard trivially; varies per run by definition.

Falsifiable retail observables: in "march immediately" runs, watch
the WORMS in the opening seconds (do they fire? does a skeleton near
spawn die first?) — mode 1 predicts worm shots precede the archer
march, and the march starts seconds in, not frame 1. If retail
archers rise at literal tick 1 with no creature death anywhere, none
of the death-watch mechanisms explain it and the hunt reopens.

## 7. RESOLVED — the live DOSBox capture (2026-07-25, player's retail)

The player captured the live D41A0 struct from their running retail
game (DOSBox, `comparison/tools/mc2_dosbox_capture.py`, paused within
the first seconds of level entry). The result closes the case:

```
sv[2] kind=9 flags=0x05 (FIRED) chain=3 union=0x00007230
model 9 alive: 39            (ALL skeletons alive)
slot 174: (5,9) life=1000 at (173.5,13.5)   (vanguard untouched)
archers: all 33 held on (3,3)               (chained to the march row)
```

The gate fired **with every skeleton alive** — and the union holds
**`0x7230 = 174 × 0xA8`: the SERIALIZED OFFSET form** of the vanguard
pointer (the exact output of `sub_55100`'s pointer→offset
relocation), live in game memory. The mechanism:

- `sub_55100`'s pool-bounds precheck reads the union **as a pointer
  in both directions**. On deserialize the union holds an offset
  (0..167832) — always below the pool's virtual base — so the check
  fails and **the offset is never converted back to a pointer**. Any
  pass through the save machinery permanently leaves the raw offset
  in the live union.
- The death-watch then dereferences `0x7230` as an address → **DOS
  low memory** (IVT/BIOS data area). Whether byte `0x723B` has bit 7
  set (life<0) or `0x723D` bit 2 (dying) is **fixed per DOS/DOSBox
  configuration** — hence per-machine deterministic: the player's
  setup always fires (archers always march at load, zero skeleton
  deaths — player-verified); other configs never fire (crater camp).

**Both retail modes are therefore real and reproducible:**

| path into the level | union at tick 1 | behavior |
|---|---|---|
| fresh bind, no serialize touched the row | absolute pool pointer (memimage frame 0: `0x3640F6`) | literal death watch — archers hold until the vanguard dies |
| any save/load (or autosave) in the path | raw offset (`0x7230`) | low-memory deref → per-config coin → march at load, or never |

The community's "conventional memory" theory is vindicated — with
the precise entry point being the save system's un-deserializable
pointer relocation, not the watch logic.

### Final binary verification (all instruction-PROVEN)

- **`sub_55100` (file 0x79900)**: ONE write (`add ecx,edx` — a raw
  byte-displacement add of ±pool_base, not index arithmetic), reached
  only through a pool-bounds gate (`cmp` vs `Entities_EA3E4[0]` and
  `[1000]`) that reads the union **as a pointer in both directions**.
  On deserialize the offset always fails the gate → **restore is
  structurally impossible for pointer-mode StageVar rows**. (The
  stages arm relocates unconditionally — the bug is specific to the
  death-watch rows.)
- **Six call sites**: every save path is serialize(1) → write →
  deserialize(2), where the (2) pass is the proven-unable restore,
  and nothing re-converts afterwards — the live union keeps the
  offset.
- **The trigger is AUTOMATIC**: `sub_57640` — a one-shot, non-multiplayer
  gated **in-level checkpoint autosave** (`SaveLevel_55080(1,
  levelnumber, "")`) — fires early in every level with no player
  action. The memimage even caught it in the act: frame 0 holds the
  absolute pointer, frame 19 the offset — the corruption happened
  live between ticks ~0–19 of that retail run.

### The complete shipped behavior of level 004 (final statement)

1. Load: rows built clean; sv[2] binds a live pointer to the vanguard
   skeleton. For the first few seconds the authored death watch is
   genuinely armed.
2. Seconds in (~tick 5–15), the one-shot checkpoint autosave
   serializes the pointer to `0x7230` **in place**, and the paired
   restore cannot undo it. The watch is now permanently severed from
   the skeleton.
3. Every subsequent scan derefs `0x7230` into DOS low memory. Per
   machine/DOS configuration, those bytes read either "dead" — the
   gate FIRES at ~the autosave moment and the archers chain onto the
   march row ("march more or less immediately"; the player's setup,
   every run) — or "alive" — the gate can **never** fire again (the
   vanguard's real death is invisible to it) and the archers camp the
   crater forever ("never march"; other setups, every run).

The authored intent (march on the vanguard's death) survives for only
the first seconds of any retail campaign run; the observed retail
behavior is a **per-configuration constant**, not a per-run roll. The
"middle mode" is impossible in shipped retail — by the time the
vanguard could die, the union no longer points at him. All ~30
&2-clear rows across ~20 levels are severed the same way by the same
autosave in every level.

**Port ruling (player, 2026-07-25): DATA-FAITHFUL — the authored
literal death watch.** "No matter what the retail does here, that's
what the level script says." Implemented in `mc2_stagevar_tick`
(replacing fire-at-bind): held while unbound or while the bound
entity lives; fires on its actual death. Test renamed to
`mc2_bound_death_watch_fires_on_death_and_chains`; mc2_cave goldens
re-pinned (level 014's kind-9 m18 hold now stands — B-D moved, load
checkpoint holds, OBSERVABLE moved last-checkpoint-only, the correct
signal); DEVIATIONS.md entries rewritten to the proven mechanism.

**The new mc2:04 choreography (probed)**: skeletons march the full
~40 tiles (~36 s); battle erupts AT the crater (first death t≈872);
the vanguard falls INSIDE the crater at t≈914 (18th of 39) and the
chain fires there — 24 archers flip to the march row with nowhere
left to march, exactly the ruling's prediction. The melee is far
bloodier than under fire-at-bind: held archers absorb the approach
volleys, ending ~0/33 archers vs 1/39 skeletons (mutual
near-annihilation) where fire-at-bind gave a 16-archer win.
PLAYTEST OWED on the new battle feel.

## 5. Re-grading the port's choices (commit `ec9d2295` and successors)

| port behavior | grade after this audit |
|---|---|
| Row decode, kind/flag mapping, holds, chain=3 | **FACT** — vindicated byte-for-byte by retail's live tables |
| sv[1]/sv[3]/sv[4] model-extinction watches (release at archer/skeleton extinction) | **FACT** — the binary's &2 arm is exactly this |
| Arrow collision filter (projectile's own xtype/xsubtype) | FACT (decompile-traced, unchanged by this audit) |
| **Fire-at-bind** (`bound ⇒ fired`, sv[2] fires tick 1) | **NOW EVIDENCE-CONTRADICTED as a mechanism.** The shipped binary binds a live pointer to the authored watched thing (thing-index key, proven) and fires **on its actual death** (valid deref, null-guarded before bind). The retail memimage run shows sv[2] validly watching the living vanguard skeleton with nothing fired through tick 20. The port firing at tick 1 releases the archers **before any retail run would**. The DEVIATIONS.md:180 rationale ("never a live entity reference in the shipped engine") is refuted. NOTE: the port's *observable* on mc2:04 still lands inside the retail envelope loosely (archers do march early in many retail runs — after first vanguard contact), which is why the 2026-07-24 player certification passed. Decision owed (see §6). |
| kind-9 proximity fallback dropped | **FACT-BACKED** — it reads the same union as coordinates vs the held archer's own position; for the actual bound-pointer values it essentially never matches. Correctly not a rescue path. |
| kind-4 worm join disabled | **Needs re-examination in light of the reversal** — the "reads uninitialized garbage" rationale came from the same refuted theory. The watched reference is (now proven) a valid pointer; the `word_0x96_150` read may be genuine. The player-observed behavior (worms never join) remains the retail datum; the *mechanism* claim in DEVIATIONS.md:181 is now suspect. |
| Archer worm-hunt fallback [9→3] | **INVENTION** (player-observed design reading; no decompile path). Unchanged. |
| f63 rescan-on-release nudge | **INVENTION** (timing match; mechanism unrecovered). Unchanged. |

## 6. Decision owed (player adjudication)

The binary-proven retail law for &2-clear watch rows is the **literal
death watch**: held while the union is null (watched thing not yet
spawned) or while the bound entity lives; fire when it dies (or its
being-removed bit sets). Restoring it would:

- put mc2:04's archers back in the crater until the vanguard skeleton
  actually falls (he marches into 33 archers, so they still release
  early in most runs — but *after first contact*, not at tick 1), and
  reproduce retail's run-to-run variance in the archers' march timing
  from genuine battle variance;
- change the ~30 other &2-clear rows across ~20 levels from "release
  when X appears" back to "release when X dies" — incl. L120's
  five-slot chain (the fire-at-bind survey list; all owed re-checking
  under the corrected law);
- make the port's row semantics match the shipped instructions with no
  determinization layer at all.

The literal death watch **naturally reproduces the player's sharpened
bimodal observation** (march at/near first contact when the vanguard
falls early; no march ever when he falls late) — fire-at-bind reproduces
only a tick-1 march, which is arguably earlier than either retail mode.
The remaining calibration question for the player: in the "march" runs,
do the archers rise **before the skeletons come into view/range at all**
(true tick-1 behavior — would need a mechanism beyond the death watch)
or **around first contact** (vanguard-death timing — the literal watch)?
And in the "never" runs, is the crater fight followed by any late
mop-up march, or full stand-down?

Against restoring: the 2026-07-24 player certification was made against
fire-at-bind and the whole battle was tuned/validated under it; the ~30
surveyed rows would all need re-checking under the literal law.

Residual open items: byte-verifying the frame-19 relocated-offset form
(cosmetic); the runtime-rebind chaos vector (pass-3 re-runs on every
class-5 spawn — audit which levels spawn class-5 at runtime near a live
&2-clear row).

## 8. The blast-radius audit — what else the save relocation breaks

Full enumeration of `sub_55100` (binary-verified 2026-07-25): the
function has exactly **three** relocation arms —

| arm | gating | verdict |
|---|---|---|
| stages board (`stages_0x3654C` objective union) | unconditional add both ways | **round-trips** |
| **StageVar death-watch union** | pool-bounds gate read as-pointer BOTH directions | **one-way broken** |
| lighting-event ring (`str_0x3664C.event_A`) | unconditional add both ways | **round-trips** |

The corruption is surgically confined to the death-watch union — the
objective/stage board ("kill entity X" tracking) and the map-lighting
ring survive saves correctly. Three consumers of the broken field:

1. `sub_12780` death-watch scan → the per-config march-or-never coin
   (the documented case; all ~30 &2-clear rows / ~20 levels).
2. `sub_1D700` kind-4 join arm → post-severance the offset FAILS the
   lower-bound guard → guaranteed early return → held kind-4
   creatures shadow-walk forever and never join a fight. **This is
   the recovered true mechanism of the mc2:04 worms' inertness** (the
   `word_0x96` garbage-read story applies only to the pre-autosave
   window).
3. `sub_12500` kind-9 proximity release → coords collapse to
   `(slot·168, 0)` — provably inert pre- and post-severance.

Port cross-check: the port stores slot indices and serializes via
Snap — structurally immune; no new divergence candidates beyond the
documented (player-ruled) death-watch law. One flag: a clean-semantics
port of the kind-4 join arm WOULD genuinely fire under the port's
data-faithful union — kept unported per the certified retail
observable (worms never join).
