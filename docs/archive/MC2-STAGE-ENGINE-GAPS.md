# MC2 stage/objective engine — remaining gaps worklist

A **data-driven** census of what's still unported in the MC2 stage engine,
so we grind the gaps in a planned session instead of discovering them one
at a time in playtests (the level-01 "raze-the-vaults never completes" bug
was the trigger — objective **type 9** turned out to be unported).

Regenerate this census any time:
```
cargo run -p mgc-sim --example tmp_objcensus     # types × levels
cargo run -p mgc-sim --example tmp_stages [lvl]  # one level's board
```
Reference trace: `docs/traces/mc2-stage-engine-completion.md` (§5 = the
full objective-type switch; §2 = the stage-var subsystem).

Scanned **165 baked MC2 levels** (2026-07-14).

---

## A. Objective TYPES (the stage board) — the "level can't complete" class

A level authoring an unported objective type can **never finish that stage
→ soft-lock**, exactly like the vault bug. Completion predicates live in
`objective_mc2` (`crates/mgc-sim/src/mc1/world.rs`).

| type | name | levels using | status |
|---|---|---|---|
| 0 | collect mana (banked %) | 84 | ✅ ported |
| 3 | kill enemy player (by color) | 8 | ✅ ported |
| 5 | fly-to / release point | 64 | ✅ ported |
| 7 | kill creature by model | 63 | ✅ ported |
| 8 | kill all players | 38 | ✅ ported |
| 9 | **destroy building** | 17 | ✅ **ported 2026-07-14** (chain-walk) |
| **1** | **kill named entity (entity-bound)** | **21** | ✅ **ported 2026-07-14** (bind seam) |
| **2** | **kill named entity, transform-aware** | **27** | ✅ **ported 2026-07-14** (bind seam) |
| 4 | fly-to (bound-entity anchored / escort) | **0** | ❌ unported — port for completeness (shares the binding seam) |
| 6 | collect item (8-slot inventory scan) | **0** | ❌ unported — blocked on the item-inventory model |

### Types 1 & 2 — LANDED 2026-07-14
The entity-binding seam is ported (`Mc2Stage.bound: Option<u16>` +
`World::mc2_bind_stage_target` = `sub_58DA0`, the template match reduced
to `thing_slot == target`). Two bind sites cover the port's inverted load
order: the **spawn-time hook** in `spawn_from_thing` (disposition-fired /
stage-gated waves) and a **retroactive pass** in `set_mc2_stages` (the
dis-0 targets already live when the app registers the checkpoints, since
`new_for_game` fires disposition 0 inside the ctor). Completion
(`World::mc2_bound_gone`) anchors on the `thing_slot` identity rather than
a raw life read, because our slots recycle through a LIFO free list.

**Trace corrections found while porting** (the original census indexed
`things[]` by ARRAY position; `build_table` indexes by the THING `slot`
field, and `pos != slot`):
- EVERY shipped type-2 target is a NAMED BUILDING `(10,45)` — never a
  plain creature. **Session H1 (2026-07-16) corrected the earlier
  "type 2 reduces to type 1" claim**: building DEGRADATION is the slot
  swap — `mc2_house_collapse`'s chain branch spawns the `bldgprm.chain`
  (= `byte_3` = `fontTypeIndex`) successor in a fresh slot and now
  RE-POINTS every active type-2 row's `bound` to it (`sub_59760`,
  EF:40921-54); the completion predicate carries retail's
  `!fontTypeIndex` term (a dead building with a successor pending does
  not count — only the final chain stage completes, EF:40771-79).
- Also from Session H (H7v): `sub_58DA0` re-points type-1/2 rows
  UNCONDITIONALLY on EVERY matching spawn (no first-bind latch) — the
  row tracks the newest instance of the named template.
- Type-1 targets are class-5 creatures (some dis-0, some dis-gated).
- Tests: `mc2_level008_kill_named_creature_objective_completes` (type 1,
  retroactive bind) + `mc2_level008_kill_named_building_type2_completes`
  (type 2, spawn-time hook, entity-specific bind) in `tests/mc2_rivals.rs`.
  Cave + slice goldens re-pinned (the new `bound` field joins the hash;
  neither golden level authors a type-1/2 row, so no behavior changed).

### Type 4 vs type 5 (the green-radius fly-to we already have)
They are **not** the same. Type 5 (ported) tests the **player's own**
position against a fixed authored point (EF:40803-14). Type 4 (EF:40787-
802) tests a **bound entity's** position and additionally requires that
entity be **player-owned** — an *escort-a-specific-entity-to-a-point*
objective. It reads the row's bound `ptr0x6E8E` (offset +40 owner must be
the player, +76/+78 = the entity's position) and so **depends on the
entity-binding machinery** — the same seam types 1/2 need. So type 4 is
NOT a standalone fly-to variant; it lands **with** the 1/2 binding port.

### Why 4 & 6 can't be "speculatively ported" in isolation
Porting them faithfully requires subsystems we don't yet have, so a
standalone predicate would reference always-empty state (never completes,
or completes vacuously) — worse than the honest `_ => false`:
- **Type 4** needs the entity-binding seam (`sub_58DA0` bind-at-spawn +
  the `& 1` bound bit + `ptr0x6E8E`). ⇒ port it **together with 1/2**.
- **Type 6** scans the player's 8-slot inventory `word_2BDE_12658`, but
  the trace found **nothing in the decompile ever WRITES that inventory**
  (read at EF:40819, zeroed at EF:43719, no producer) — so type 6 is a
  **dead objective in retail itself**: it can never complete. Authors
  never use it (0 levels), which is consistent. ⇒ do NOT invent a
  completion path; if ever needed, it's blocked on an item carry-slot
  subsystem that has no reference to port. Effectively **skip**.

Both are authored in **zero** shipped levels (reachable only via a
hypothetical level editor), so they are low-priority-but-wanted for a
complete implementation. Full port recipe banked (next session):
**`docs/traces/mc2-objective-types-1-2-4-6.md`**.

**Distinct levels formerly soft-locked by an unported objective type
(1 or 2): 41 — now CLOSED (types 1 & 2 ported 2026-07-14).** With types
4 & 6 authored in zero levels, EVERY objective type used by a shipped
level is now ported: the objective-board soft-lock class is fully closed.
The 41 formerly-at-risk levels were:
`level-005, 008, 013, 015, 017, 019, 021, 022, 024, 027, 038, 039, 046,
048, 074, 085, 087, 090, 097, 100, 117, 118, 119, 120, 124, 128, 137, 139,
140, 143, 144, 150, 155, 157, 159, 162, 163, 172, 173, 175, 199`.

### The work (types 1 & 2) — DONE (recipe kept for 4/6)
Both are "kill a specific bound creature". Unlike type 7 (kill-by-MODEL,
already ported and needs no binding), types 1/2 name a **specific entity
instance** and so need the **entity-binding machinery** we haven't ported:
- `sub_58DA0` (EF:40650-90) — at every class-5/A spawn, bind the row's
  `ptr0x30311` template to the live entity + set flag bit 0 (`& 1` "target
  live"). `sub_59760` (EF:40922-54) re-points the binding when the target
  transforms (keeps type-2's chain alive).
- Completion (§5): type **1** = bound entity `life <= -1`; type **2** =
  `life <= -1` **AND** not transformed (`!fontTypeIndex_0x3D_61` — genuinely
  dead, not morphed into a successor).
- The `InitStages` payload for 1/2 is a template pointer, not a model.
  **Session H4 (2026-07-16, LE-binary verified):** retail's "drop typed
  rows with stage==0" guard is DEAD CODE (its switch selector reads the
  memset-zero destination row, EF:40589 — confirmed by disassembling
  NETHERW.EXE), so retail registers EVERY `index != -1` row, active.
  The port's literal drop was removed: 13 levels' stage0-typed rows now
  register (fixing level-198's severed m32 chain); the type-1/2 stage0
  rows (5 levels) bind the empty record 0 and are FAITHFULLY
  un-completable, exactly like retail — those levels end by other
  paths (the model-31 X-marker latch).
- Effort: **Medium.** One shared bind seam + two thin predicates. Model
  it on the type-9 session: pick an exemplar level, write a driven test.

---

## B. The STAGE-VAR subsystem — ✅ LANDED 2026-07-14

Distinct from the objective board. StageVars are the level's **triggered-
spawn / hold-gate layer**: a gated creature spawns HELD (at its phase-7
wait action, `8*model+7`) until its gate fires — proximity, a timer, a
referenced model going extinct, a bound entity dying, or a disposition
firing — then drops to its active action (`8*model+1`). The whole
subsystem is ported in `crates/mgc-sim/src/mc2/stagevars.rs`.
**Session H6/E16 (2026-07-16):** HELD is no longer a hard freeze — a
held creature runs `sub_1D5D0`'s per-kind held head every tick
(killable via the damage-inbox drain; a foreign hit breaks the hold
into aggro `8m+2`/`8m+6`; the kind-3 AMBUSH arm aggros on the watched
entity when it nears `v_28`; kind-4 joins the watch's fight), the m9
imp defers its hold until its 16-tick materialize completes, and the
m27 kraken body runs its full 0xDF stage-command state (`sub_29930` —
pose select, the 0xDA MASS-ATTACK tentacle broadcast, the
0xD8→StageVar2=15 inert marker). See stagevars.rs's module doc for the
deliberate APPROX register (idle facing/physics choreography).

Failure mode was a fidelity issue, not a soft-lock: gated content
appeared too early / all at once (scripted reinforcements active from
spawn instead of waiting for their trigger).

| kind | meaning | levels | status |
|---|---|---|---|
| 1 | release on the creature's own proximity (≤8 tiles) | 22 | ✅ |
| 2 | fly-point store (held, released by chain/cadence) | 23 | ✅ |
| 3 | release when a bound entity dies | 19 | ✅ |
| 6 | release after an N-tick timer | 10 | ✅ |
| 4 | release when the referenced model extinct / bound dead | 4 | ✅ |
| 9 | bound-death (retail's "proximity fallback" reads pointer bytes via a union — unreachable garbage, not reproduced; H7i) | 3 | ✅ |
| 7 | release when disposition D fires | 2 | ✅ |
| 5 | (bound-subtype) | 1 | ✅ |

**Distinct levels touching the stage-var subsystem: 61.** A scan of all
165 baked levels finds **38 that actually HOLD a creature** (18 at load,
20 after a disposition) across kinds 1/2/3/4/5/6/7/9 — the mechanism
fires.

### What landed
The full pipeline, verified against the decompile (port-spec banked in
the completion trace + the agent verification 2026-07-14):
`InitStageVars_11EE0` (loader) → `Mc2StageVar[11]` table →
`mc2_stagevar_attach` (`sub_12100`/`sub_12330` — hold + cadence + the
death-watch bind, plus a retroactive pass for load-time dis-0 spawns) →
`mc2_stagevar_tick` (`sub_12780` fired-bit scan + `sub_12500` per-entity
reaction) → `mc2_stagevar_release` (`sub_12410`/`sub_12470` — chain
re-arm or full release) → `mc2_stagevar_arm_disposition`/
`_rearm_watchers` (`sub_122C0`/`sub_12870`, from `fire_disposition`).

### Key decisions / findings
- **HELD = freeze.** A phase-7 class-5 entity with `site_z != 0` already
  early-returns in `mc2_creature_tick` (the metamorph/summon path), so a
  StageVar-held creature (`site_z` = kind 1..9) runs no per-model
  behaviour. Retail also runs the model's phase-7 action while held, but
  for these gated creatures it is a wait/idle, and with no retail hash to
  match, "held until the gate, then active" is the reproduced behaviour.
- **Hash + `Ent` discipline.** The state lives in two `World` vecs
  (`mc2_stagevars`, `mc2_sv_held`) hashed only when populated — MC1 and
  StageVar-less MC2 levels are byte-identical. Retail's per-entity
  `StageVar1`/`word_0x4A_74` are kept in the side `Mc2Held` binding so
  `Ent` (and the MC1 goldens) never change.
- **Data-layout corrections** (the census originally indexed `things[]`
  by array position; `build_table` indexes by the THING `.slot`): the
  hold-match template = `str_0x3647A_2.word` (level x|y<<8); the
  death-watch template = `data.lo`; the kind-1/2 fly-point survives only
  its low byte after the loader's `<<8` u16 truncation.
- **Golden impact.** level-000's three kind-1 vars are inert (word=0, no
  matching THING → nothing held); level-014 holds exactly the one kind-9
  model-18 creature (its gate never fires in the golden run — it stays
  dormant, faithfully). Both cave + slice goldens re-pinned; the held
  binding is asserted in the cave test.
- Tests: `mc2_level019_stagevar_holds_until_bound_death` (kind 3) +
  `mc2_level104_stagevar_timer_releases` (kind 6) in `tests/mc2_rivals.rs`.

### OPEN / banked
- The kind-3/4/5 `&2`-mode `word_0x4A_74` handle-tracking branches are
  DORMANT in retail too (nothing writes the handle) — not ported; flagged
  in the verification spec.
- Per-model phase-7 wrapper EXTRAS during hold: the "presentation
  nicety" reading was WRONG — two player reports (2026-07-18) traced
  to dropped +7 physics: m21's jump cycle (`sub_26470`, floating +
  silent devils) and m0's vertical bob (`sub_1F300`, flat "crippled"
  dragons). Both now run in the held seam (kinds 1-10). REMAINING:
  systematically sweep the OTHER class-5 phase-7 wrappers in the EF
  dispatch table for more ambient tails (sound rolls, speed refresh,
  ground re-snap — e.g. the goat bleat, m18's re-snap) the held seam
  still drops.
- PLAYTEST OWED: the 38 holding levels (esp. level-014's now-dormant
  model-18) want an eyeball to confirm the gated reinforcements feel
  right in play.

---

## Suggested grind order
1. ✅ **Objective types 1 & 2** — DONE 2026-07-14 (the bind seam; 41
   levels' soft-lock class closed). Recipe: `mc2-objective-types-1-2-4-6.md`.
2. **Type 4** (0 levels) — the bind seam now exists; add the player-owner
   check (resolve the `f40`/`f144` ownership-field ambiguity against the
   possession/gift path first, §3e). Completeness only.
3. **Type 6 — SKIP** (0 levels, and the trace shows it's un-completable in
   retail too: nothing writes the inventory it scans). Keep `_ => false`.
4. **Stage-var subsystem** (MEDIUM/LARGE — 61 levels; content presence).
   Its own session — now the largest remaining stage-engine gap.

Each item: pick an exemplar level from its list, trace the retail arm
(citations in `mc2-stage-engine-completion.md`), port, add a level-driven
regression test (model on `mc2_level001_destroy_building_objective_completes`
in `tests/mc2_rivals.rs`), re-run the full suite (goldens must hold).
