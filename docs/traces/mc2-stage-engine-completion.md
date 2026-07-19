# MC2 Stage / Mission Engine — Completion Trace (Phase 4.1)

VERBATIM trace of the remaining MC2 stage/mission machinery from the
remc2 decompile, port-ready. Cross-references the already-landed
single-stage core (Phase 3.5 + LEVEL-000 MISSION-CHAIN) in
`crates/mgc-sim/src/mc1/world.rs` (`objective_mc2`, `Mc2Stage`,
`set_mc2_stages`, `mc2_objective_pause`) and regression test
`mc2_level000_mission_chain` (`crates/mgc-sim/tests/mc2_slice.rs`).

All file:line citations are into
`reference/remc2/remc2/engine/` (EF = EventsFunctions.cpp,
E = Events.cpp, GU = GameUI.cpp, LS = LevelStructs.h,
BT = BasicTerrain.h). Line numbers are as of this session.

---

## 0. The struct map (authoritative field names)

### Objective (stage) row — `type_str_3654C` (LS:146-151), 8 rows
`D41A0_0.stages_0x3654C[8]` — the LIVE objective rows.

| field | type | meaning |
|---|---|---|
| `stages_3654C_byte0` | int8 | **objective TYPE** (0..9), the switch key at EF:40744 |
| `str_3654D_byte1` | int8 | flag byte: **bit0 (`&1`) = entity bound** (target spawned, set by `sub_58DA0`); **bit1 (`&2`) = external force-complete** (EF:40736) |
| `str_3654E_axis` | axis_2d | fly-to point (types 4/5), engine units (tile<<8) |
| `str_36552_un` | union | per-type payload: `.dword` (mana% target / kill-model / building-model) or `.ptr0x30311`/`.ptr0x6E8E` (bound entity for 1/2/3/4/6) — see InitStages |

### Per-player objective state — `type_substr_3659C` (LS:190-196), 8 players
`D41A0_0.struct_0x3659C[player].substr_3659C`:

| field | meaning |
|---|---|
| `IsLevelEnd_0` (uint8) | **all objectives done** → level complete for this player (= our `World.completed`) |
| `ObjectiveText_1` (uint8) | **the CURRENT-stage cursor** (0-based row index) — types 5/6/7/8/9 test only while `v3 == ObjectiveText_1` |
| `ObjectiveDone_2` (uint8) | **the pause countdown** (= our `mc2_objective_pause`) |
| `stage_0x3659F[8]` (uint8[8]) | per-row STATE: **1 = active, 2 = done** (0 = unused row) |

### Global counters (LS:261-267, in `type_D41A0_BYTESTR_0`)
- `stageIndex_0x36E01` — **count of registered objective rows** (loop bound everywhere).
- `countStageVars_0x36E00` — count of registered StageVars (§3).
- `byte_0x36E02` — "temp objective" = the **objective-message trigger** (§4).
- `byte_0x36E03` — the APOCALYPSE latch (already ported as `mc2_apocalypse`; EF:12871/35527) — NOT an objective field.
- `byte_counter_current_objective_box_0x36E04` — the on-screen objective-box display countdown (200 ticks).
- `byte_0x36E0B` — bit0 = "this is a SWITCH/beacon objective, use the switch chime + secret-track path" (§4).

### Checkpoint SOURCE (level file) — `type_str_0x36442` (BT:48-53), 8 rows
`terrain_2FECE.stages_0x36442[8]` — the package's checkpoint rows:
- `index_0` (int8) — objective type, or `-1` = unused.
- `stage_1` (int16) — the type-dependent param (mana%, entity index, …).
- `_axis_2d` — the fly-to tile point.

### StageVar SOURCE (level file) — `type_str_0x3647Ac` (BT:62-67), 11 slots
`terrain_2FECE.StageVars_0x3647A[11]`; live copy `D41A0_0.StageVars2_0x365F4[11]` (LS:249). Fields per §3.

### Objective-type legend (verbatim, BT:36-46)
```
//0 - collect mana
//1 - kill creature2 - must fix entites
//2 -
//3 - kill enemy player
//4 -
//5 - release point
//6 -
//7 - kill creature - must fix entites
//8 - kill all players
//9 - destroy building - must fix entites
```

---

## 1. Multi-stage progression + checkpoint chaining

### 1a. Registration — `InitStages_58940` (EF:40567-40647)
Called from level load (EF:39406/39471). Zeroes `stageIndex_0x36E01`,
`struct_0x3659C[0..8]`, `stages_0x3654C[0..8]`, and `word_0x2FED5`
(the type-0 HUD bar, §4). Then either:

- **Multiplayer** (EF:40574-40582): one synthetic row, type 8 ("kill
  all players"), every player's `stage_0x3659F[0] = 1`.
- **Single-player** (EF:40585-40645): walk the 8 source checkpoints
  `stages_0x36442[result]`; skip `index_0 == -1`.
  - **Entity-typed rows {1,2,4,6,7,9} with `stage_1 == 0` are DROPPED**
    (EF:40589-40602: their `index_0` is forced to -1). *(NB the switch
    at :40589 reads the DEST row's byte0, which is still 0 at that
    point — effectively the guard runs against type 0; the port's
    `set_mc2_stages` reproduces the observed drop of {1,2,4,6,7,9}
    when `stage==0`.)*
  - Each surviving row: `stages_3654C_byte0 = index_0`, and **every
    player's `stage_0x3659F[stageIndex] = 1`** (EF:40607-40610) — so
    ALL rows are ACTIVE from load; `ObjectiveText_1` starts at 0.
  - Per-type payload store (EF:40611-42):
    | type | payload store |
    |---|---|
    | 0 | `.dword = stage_1` (mana % target) |
    | 1,2,6 | `.ptr16u = &entity_0x30311[stage_1].type_0x30311` (bind by index; resolved to live ptr later) |
    | 4 | same ptr **+** `str_3654E_axis = _axis_2d << 8` |
    | 5 | `str_3654E_axis = _axis_2d << 8` **+** `.dword = stage_1` |
    | **7** | `.dword = entity_0x30311[stage_1].subtype_0x30311` (**the target MODEL/subtype**, EF:40632) |
    | 9 | `.dword = entity_0x30311[stage_1].par1_14` (building-model tag) |
    | default | `.dword = stage_1 - 1` |

**Port match:** `set_mc2_stages` (`world.rs`:3407) mirrors this: skips
-1, drops {1,2,4,6,7,9} at stage==0, type-7 stores the target's model,
`state=1` for all, `mc2_stage_current=0`.

### 1b. The advance law — `sub_58F00_game_objectives` (EF:40693-40919)
Runs once per tick (called EF:31817). Per player `v0x` (SP: only
player `LevelIndex_0xc`):

1. **Pause head** (EF:40724-27): if `ObjectiveDone_2 != 0`, decrement
   and **skip the whole pass this tick**. (= our `mc2_objective_pause`.)
2. Else if NOT `IsLevelEnd_0`, walk rows `v3 = 0 .. stageIndex_0x36E01`:
   - **Force-complete first** (EF:40736-40741): `if (str_3654D_byte1 & 2)` →
     `stage_0x3659F[v3] = 2`, `achievedGoal = true`, and **clear bit1**
     (`&= 0xFD`). (= our `Mc2Stage.force`.)
   - Else if `stage_0x3659F[v3] == 1` → the **type switch** (§5).
   - `LABEL_72` (EF:40881): `if (achievedGoal && v3 == ObjectiveText_1) v23 = 1`
     — `v23` records "the CURRENT row just completed".
3. **On any `achievedGoal`** (EF:40885-40912):
   - `sub_88B20()` (a sound/UI ping).
   - **Recompute the cursor** (EF:40889-96): scan rows 0..stageIndex; the
     FIRST still-`== 1` row becomes the new `ObjectiveText_1`; if NONE,
     `v14 = 1`.
   - `IsLevelEnd_0 = v14` (EF:40898) — **level completes only when NO
     active row remains.**
   - `if (v23 || v14)` (EF:40899): fire the objective-message trigger —
     MP writes the "Has Completed Objective." / "…All Objectives."
     notification (langindex 430/431, EF:40906/08); SP sets
     **`byte_0x36E02 = 1`** (EF:40911) → drives `PresentObjective_59820`
     (§4). So the message/chime fires **when the CURRENT row completes OR
     the level ends**, NOT on a background row completing out of turn.

### 1c. Level-completion vs stage-completion
- **Stage done** = `stage_0x3659F[row] = 2`. Advancing the cursor to the
  next active row is stage progression.
- **Level done** = `IsLevelEnd_0 = 1`, set ONLY when the cursor scan
  finds no `== 1` row (EF:40889-98). This is the distinction the port
  encodes as: advance `mc2_stage_current` to `position(state==1)`, else
  `self.completed = true` (`world.rs`:3525-27).

### 1d. Inter-stage timers / pauses
- **The one-tick `ObjectiveDone_2` pause** — the ONLY inter-stage pause.
  Set to 1 by the m32 stage-gated switch `AddSwitch0B_20_6F1C0` when it
  fires (EF:54371, §1e). Already ported. No other inter-stage timer
  exists in `sub_58F00`; the "60/4" writes at EF:40903-04 are the MP
  on-screen notification's display timer, not a gameplay pause.
- The objective-BOX display counter (`byte_...0x36E04 = 200`,
  EF:41054) is presentation only (§4).

### 1e. The stage-gated switch (m32) — `AddSwitch0B_20_6F1C0` (EF:54353-54380)
The class-0xB, model-32 switch entity. Per tick (dispatched E:4005):
`if (byte_0x46_70 < stageIndex_0x36E01)` scan players; if
`stage_0x3659F[switch.byte_0x46_70] == 2` (the gated row is DONE):
```
D41A0_0.struct_0x3659C[player].substr_3659C.ObjectiveDone_2 = 1;   // :54371 the pause
sub_4A1E0(a1x->id_0x1A_26, 1);                                       // :54372 FIRE THE DISPOSITION
DisableEntityDrawing04_57F10(a1x);                                  // :54373 consume the switch
```
`byte_0x46_70` = the switch's gated STAGE INDEX. `id_0x1A_26` = the
disposition id it fires. So a completed stage → this switch spawns the
NEXT stage's targets (via disposition-fire, §3d) AND sets the 1-tick
pause so the objective pass re-scans only after the spawns land. This
is exactly the port's `(11,32)`/m32 handling + `mc2_objective_pause = 1`
(`world.rs`:4338-4347). **The similar switch at EF:54329-54347 fires its
disposition only when `IsLevelEnd_0` is set (a level-END-gated switch,
e.g. the exit-point release) — same shape, gate = `IsLevelEnd_0`
instead of a row state.**

### 1f. Entity binding — `sub_58DA0` (EF:40650-40690)
Called at every class-5/class-A spawn (EF:33031, :33041, …). For rows of
type {1,2,4} whose bound `ptr0x30311` == the just-spawned template, it
sets `str_36552_un.ptr0x6E8E = <live entity>` and **`str_3654D_byte1 |= 1`**
(bit0 = "target is now live"). Type 3 binds by player-color; type 6 binds
by entity-index. This is why types 1/2/4/6 gate on `& 1` in the switch —
a row cannot complete before its target has spawned and bound. The mirror
`sub_59760` (EF:40922-40954) **re-points** a bound target to a successor
entity when the original transforms (type 2 keeps the chain alive).

---

## 2. The stage-var reaction pass (the roadmap's "~:4961")

The roadmap's "(:4961)" is **EventsFunctions.cpp**, function
`sub_122C0` (EF:4961) — a stage-var TRIGGER, one member of the
stage-var machinery. The full pass is three functions + a loader.
**This whole subsystem is NOT yet ported** (grep confirms only the
class-10 multipart dispatch references `StageVar2`, not the gate table).

### 2a. Loader — `InitStageVars_11EE0` (EF:4631-4681)
`countStageVars_0x36E00` = highest slot `1..10` whose source
`index_0x3647A_0 & 0xF != 0`. Per slot, unpack the source byte:
- low nibble `& 0xF` → `index_0x3647A_0` = the **StageVar KIND** (1..9).
- high bits → `stage_0x3647A_1` flag bits: `0x80→bit0(&1)`,
  `0x40→bit1(&2)`, `0x10→bit5(0x20)`, `0x20→bit6(0x40)` (EF:4646-53).
- payload by kind (EF:4654-77): kinds 1/2 store a fly point `<<8`;
  kinds 3/4/5/8/9 store (if `&2`) the referenced entity's SUBTYPE in
  `str_0x3647C_4.axis.x`; kinds 6/7 store the raw param.
- `str_0x3647A_2._axis_2d.x = source.stage_0x3647A_1` (the retrigger
  cadence seed).

### 2b. Per-tick global scan — `sub_12780` (EF:5135-5211)
Runs each tick from `UpdateEntities_57730` (EF:40095), BEFORE the
per-model reaction loop. Walk `StageVars2_0x365F4[1..countStageVars]`:
- **kinds {3,4,5,8,9}** (EF:5164-5193): if flag `& 4` already set →
  latch `v2=1`; else if `& 2` → satisfied when the referenced model's
  live-list head `bytearray_38403x[axis.x]` is **empty** (target extinct);
  else → satisfied when the bound entity `pointer_0x6E8E` is dead
  (`life < 0`) or being-removed (`byte[1] & 4`). On satisfy: **set
  `stage_0x3647A_1 |= 4`** (the "fired" bit).
- **kind 7** (EF:5194-5204): if `& 0x18` set, consume it (clear 0x10 or
  0x08) — the disposition-trigger auto-clears after one tick.

### 2c. Manual fire — `sub_122C0(a1)` (EF:4961-4968) [the roadmap anchor]
```
for (index = 1; index <= countStageVars; index++)
  if (StageVars2[index].index_0x3647A_0 == 7 && a1 == StageVars2[index].str_0x3647C_4.axis.x)
    StageVars2[index].stage_0x3647A_1 |= 0x18u;   // arm the kind-7 gate
```
**Called from `sub_4A1E0` (EF:32967)** — the disposition-fire path. So
firing disposition `a1` also arms every kind-7 StageVar keyed to `a1`.
`sub_12870` (EF:5214-5240) is the inverse: for kinds {3,4,5,8,9} with
`&4` set, clear the `&2` bit (re-arm bookkeeping) — called from
`sub_12500` case 7 (EF:5123).

### 2d. Per-ENTITY reaction — `sub_12500` (EF:5045-5131)
Runs for every class-5 live entity whose `StageVar1_0x48_72` or
`StageVar2_0x49_73` is nonzero (dispatched EF:40096-40103, the
`bytearray_38403x` walk). Gated by `(actionIndex & 7)` being outside
{4,5}. Switches on the entity's `StageVar2_0x49_73` (the bound KIND):
- kind 0xA: unless action-phase is 2/6, re-arm via `sub_12330`.
- kinds 0xD/0xE/0x10/0x11: jump to `actionIndex = 8*model + 7` (the
  "held/waiting" action).
- kind 0xF: if `actionIndex & 7` set, re-arm.
- **default** → dispatch on the SLOT's `index_0x3647A_0` (EF:5070):
  - kind 1 (EF:5072): proximity — if `|point−pos| ≤ 2048` on both axes →
    `sub_12410(entity, 8*model+1)` = **RELEASE** (spawn/animate). 2048 = 8 tiles.
  - kind 3 (EF:5077): if slot flag `& 4` (fired) → release; else track
    the `word_0x4A_74` handle (clear if the referenced entity died).
  - kinds 4/5/8/9 (EF:5092): if `& 4` → release; if `& 2` track handle;
    else (kind 9 only) proximity `≤ 3072` (12 tiles) → release.
  - kind 6 (EF:5115): **timer** — decrement `word_0x4A_74`; at 0 → release.
  - kind 7 (EF:5120): if slot flag `& 0x18` (disposition-armed) →
    `sub_12870()` + release.

### 2e. Attach at spawn — `sub_12100` (EF:4684-4750)
Called when a class-5 entity spawns (EF:33031). Finds the StageVar slot
matching this entity's template index (`str_0x3647A_2.word`) or subtype,
and either sets `event->word_0x4A_74 = slot` (deferred) or
`sub_12330(event, slot)` (immediate arm). `sub_12330` (EF:4970-5021)
computes the retrigger cadence (`stage_0x3647A_1 & 0x60` → every
1/2/4 ticks) then sets `actionIndex = 8*model+7`, `StageVar1 = slot`,
`StageVar2 = slot.index_0x3647A_0`. This is the mechanism that puts a
freshly-spawned creature into a **HELD** state until its gate fires.

**Summary (§2):** StageVars are the level's TRIGGERED-SPAWN / hold-gate
layer, distinct from the objective rows. Classes 3/4/5/8/9 = "release
when target-model extinct / bound-entity dead"; kind 1 = "release on
player/pos proximity"; kind 6 = "release after N-tick timer"; kind 7 =
"release when disposition D fires". The gate KIND is the level-file low
nibble; the flag bits are the mode (`&1` template-match, `&2` bound-by-
entity, `&4` fired, `0x60` retrigger cadence).

---

## 3. Class-0 "conditional spawn" machinery

**Finding: there is NO live class-0 dispatch.** The per-tick entity
update loop `UpdateEntities_57730` (EF:39948-40091) opens with
`if (jx->class_0x3F_63)` (EF:39971) and the class switch (EF:39973) has
cases only for 3/5/9/0xA/0xB (`default: continue`). **Class 0 entities
are never ticked, never bucketed, never drawn** — they are the inert
package records. So "class-0 conditional spawn" = the **THING/disposition
records** (class 0 in the package table) consumed by disposition-fire,
gated by the m32 switch (§1e) and StageVars (§2). This is exactly the
port's `fire_disposition` + `Mc2Stage`/`StageVar` seams.

### 3a. The census/bucket lists built each tick (EF:39957-40091)
`UpdateEntities_57730` rebuilds the per-class fast lists that the
objective + stage-var passes read:
- `bytearray_38403x[model]` (EF:40005) — **class-5 per-MODEL live-list
  heads**, skipping actionIndex ∈ {0xB4, 0xE8, 0xEA} (the corpse/
  multipart phases). **This is the type-7 kill test's oracle** (EF:40829)
  and the StageVar {3,4,5,8,9} extinction test (EF:5178). Already
  mirrored in the port's type-7 predicate (`world.rs`:3501-3510,
  the `!matches!(tick70, 0xB4|0xE8|0xEA)` filter).
- `dword_38519` (EF:39981) — class-3 (wizard/player) list.
- `dword_38531` (EF:40014) — class-9 (projectiles) list.
- `dword_38523`/`dword_38527`/`dword_38535` (EF:40028/40048/40038,
  70) — class-0xA model buckets: `38523` = models 0x27-0x28 & 0x39,
  `38527` = models 0x2D (**the type-9 destroy-building list**, EF:40854),
  `38535` = models 0x2A, 0x43, 0x4E, and class-0xB 0x0C/0x1F.

### 3b. Disposition-fire — `sub_4A1E0(disId, flag)` (EF:32950-…)
The class-0 conditional-SPAWN executor:
- `disId == 0` (EF:32952-64): full reset — recount spell tallies,
  rebuild the F5538 table.
- `sub_49F90()` + **`sub_122C0(disId)`** (EF:32967, arms kind-7
  StageVars) + walk all 1200 package entities: any with `.DisId == disId`
  → `sub_4A310(&entity)` = **materialize it into the live pool**
  (EF:32982-84). Class-5 subtypes update the mana-tally accounting
  (EF:32985-88). This is the port's `fire_disposition` (`world.rs`:3537).

### 3c. Interaction with census / misfit accounting
Because class-0 records never enter the tick loop, they never appear in
`bytearray_38403x` and never count toward type-7 extinction — correct
(they are unspawned). They DO count in the package census (the
`examples/mc2census` 69381-record sweep). When `sub_4A310` materializes
one, it enters as its real class (3/5/9/0xA/0xB) and thereafter buckets
normally. **No class-0 model enumeration is needed for the tick engine**
— the switch at EF:39973 proves class 0 is a no-op there; the models
that matter are enumerated per §3a by the CLASSES they materialize into.

---

## 4. Objective messages

### 4a. Trigger — `byte_0x36E02` set to 1 (EF:40911) on current-row
completion or level-end (§1b). Consumed by `PresentObjective_59820`
(EF:40957-41066), called each tick.

### 4b. Presentation state machine — `PresentObjective_59820`
- `byte_counter_current_objective_box_0x36E04` counts down; at 1 calls
  `sub_88BA0()` (tear down the box) (EF:40966-70).
- If `paletteMod_51 >= 3` and `byte_0x36E02` set:
  - **SPEECH enabled** (EF:40984): staged CD-voiceover ramp
    (`byte_0x36E02` steps 1→8), `PlayCDTrackSegmentNumber_86EB0(level,
    objectiveIndex, 1)` (EF:41038) — the spoken objective. On the
    switch/beacon path (`byte_0x36E0B & 1`) it plays the secret-level
    segment instead (EF:41012).
  - **SPEECH disabled** (EF:41052-63): `byte_...0x36E04 = 200` (show the
    text box 200 ticks), then the CHIME:
    - `if (byte_0x36E0B & 1)` → **`PrepareEventSound_6E450(level, -1, 41)`**
      (EF:41058) — sound **41 = `Switch_41`** (SoundInGameIndexes.h:45),
      the SWITCH/beacon acknowledgement.
    - else if `ObjectiveText_1 != 0` → **`PrepareEventSound_6E450(level,
      -1, 61)`** (EF:41063, also :41019) — sound **61 = `Success2_61`**
      (SoundInGameIndexes.h:65), **the objective-advance chime**.

  **PORT CORRECTION:** `objective_mc2` currently plays `snd_player(41)`
  on every advance (`world.rs`:3524). Retail's advance chime is **61
  (Success2)**; **41 (Switch)** is the beacon/switch-objective variant
  and only when `byte_0x36E0B & 1`. Also note the chime is suppressed
  when `ObjectiveText_1 == 0` (the very first objective at load).

### 4c. Text lookup — `DrawCurrentObjectiveTextbox_30630` (GU:532-601)
The objective string index:
```
result = ObjectiveText_1 + IndexLevelText_DB4EE[levelnumber_43w];   // GU:573-574
text   = langindexbuffer[result];                                    // GU:579 "Fly towards my beacon."
```
- `IndexLevelText_DB4EE[level]` = the per-level BASE into the language
  string table `x_DWORD_E9C4C_langindexbuffer[]`; `ObjectiveText_1`
  (current cursor) is the offset. So **objective N of level L =
  `langindexbuffer[ IndexLevelText_DB4EE[L] + N ]`**.
- On level-end: `LevelEndText_DB507[level]` (GU:571).
- Levels 30-34 (0x1E-0x22) are special-cased (GU:556-562).
- Help-toggle mode: `x_BYTE_DB520[ObjectiveText_1]` (GU:547).

So the per-row "message id" is NOT stored in the objective struct — it
is **implicit: the row's ordinal position + the level's base index.**
The struct carries no hint/text field; the CURSOR is the message
selector. (Fields available for a port: `ObjectiveText_1` = the index;
`IndexLevelText_DB4EE` + `LevelEndText_DB507` = the two base tables;
`langindexbuffer` = the string table.)

### 4d. The type-0 HUD progress bar — `word_0x2FED5`
Separate from the text: for a type-0 (mana-share) CURRENT row, the pass
publishes the target% into `terrain_2FECE.word_0x2FED5` (EF:40760-61),
cleared to 0 when the row completes (EF:40756) or at init (EF:40573).
The HUD draws it as a bar at `barStartXPos + (word_0x2FED5<<6)/100`
(GU:214-217) — the "collect mana" progress marker. Banked for the HUD
track (4.9); not gameplay-critical.

---

## 5. The FULL objective-type switch (EF:40744-40878)

The complete `switch (stages_3654C_byte0)` inside `sub_58F00`. Each
case, its completion predicate, verbatim citation:

| type | name | gate | completion predicate | cite |
|---|---|---|---|---|
| **0** | collect mana | ANY row | player has a castle AND world-mana total > 0 AND `100*(banked_0x13C + castle.mana_0x90)/worldMana >= target%`; on success clears the HUD bar | EF:40746-58 |
| **1** | kill creature | `& 1` (bound) | bound entity `ptr0x6E8E->life_0x8 <= -1` | EF:40763-70 |
| **2** | kill creature (no transform) | `& 1` | `life <= -1` **AND** `!fontTypeIndex_0x3D_61` (the target hasn't morphed into a successor — i.e. genuinely dead, not transformed) | EF:40771-79 |
| **3** | kill enemy player | — | `!array_0x2BDE[targetPlayerColor].byte_0x006_2BE4_11236` (that wizard's alive-flag is clear) | EF:40780-86 |
| **4** | fly-to (bound-entity anchored) | `& 1`, current only | the bound entity is the PLAYER (`v17x == Entities[*(u16*)(payload+40)]`) AND `|point.x − *(payload+76)| <= 768` and `|point.y − *(payload+78)| <= 768` (768 = 3 tiles) | EF:40787-802 |
| **5** | release point | current only | `|point.x − player.pos.x| <= 768` and `|point.y − player.pos.y| <= 768` | EF:40803-14 |
| **6** | (collect item?) | current AND `& 1` | scan the player's 8-slot inventory `word_2BDE_12658[0..8]` for `== payload.dword`; found → done | EF:40815-27 |
| **7** | kill creature (by model) | current only | `!bytearray_38403x[payload.dword]` — **the target MODEL's live-list is empty** (all instances dead) | EF:40828-34 |
| **8** | kill all players | current only | for every OTHER player i, `!array_0x2BDE[i].byte_0x006` (all rivals dead) | EF:40835-50 |
| **9** | destroy building | current, every 16th tick (`!(FrameTimingIndex_26 & 0xF)`) | walk the class-0xA model-0x2D list `dword_38527`; follow the building's `str_D93C0_bldgprmbuffer[j].byte_3` chain (≤8 links) for a live piece with `byte_0x46_70 == payload.dword`; **none found → destroyed** | EF:40851-75 |

Notes:
- Types **5,6,7,8,9** gate on `v3 == ObjectiveText_1` (**current-stage
  only**) — the CURSOR gate the port added at :40827 for type 7 applies
  to all of these. Types **0,1,2,3,4** test in ANY position (background
  rows) — but type 4 additionally requires the bound player anchor.
- Type 0 uses **`>=`** (port note at `world.rs`:3474-75: MC1's banked
  check is strictly `>`).
- **Port status:** types 0, 3, 5, 7, 8, and **9** landed. Type 9
  (destroy building, ported 2026-07-14 for level-001's fifth objective
  = raze the two `par1=21` vaults by Pyahandra's tower): current-stage
  gated, done when no live `(class10,model45)` building with `act_life
  >= 0 && flags & 0x400 == 0` carries `f71 == payload`, where the
  payload = `table[stage].par1` (the referenced THING's par1 = the
  building-type tag the ctor stamps into `f71`). The m32 stage-gated
  switch fires the vault-spawn disposition + the 1-tick objective pause
  on the prior row's completion, so the current-stage gate prevents a
  vacuous latch (test `mc2_level001_destroy_building_objective_completes`).
  The retail bldgprm `byte_3` ≤8-link chain walk (EF:40851-75) reduces
  to the per-entity `f71` match for standalone vaults; multi-piece
  building groups would need the chain follow. Types 1/2/4/6 remain
  `_ => false` (entity-binding; none on the certified set).

---

## OPEN items (unconfirmed — do not guess in the port)

1. **Type-6 semantics.** The BT legend leaves type 6 blank; the switch
   (EF:40815-27) reads the player's 8-slot `word_2BDE_12658` inventory
   for a model id — consistent with "collect a specific item/spell",
   but no level-000 exemplar. Confirm against a level that authors a
   type-6 row before porting. `sub_58DA0` case 6 binds it by
   entity-index → `.dword = entity − struct_0x6E8E` (EF:40681), so the
   payload is a LIVE entity index, not a model — re-verify the switch's
   `payload.dword` meaning against that.

2. **Type-4 payload layout.** The switch reads `*(u16*)(payload+40)`,
   `*(s16*)(payload+76/78)` off the bound entity (EF:40791-95) — offsets
   into `type_entity_0x6E8E` (40 = a player/owner index, 76/78 =
   position). Confirm these offsets map to `dword_0xA4`/`position_0x4C`
   in the port's entity before porting type 4.

3. **The `InitStages` drop-guard switch at EF:40589.** It reads the
   DEST row's `stages_3654C_byte0` (still 0 pre-store) rather than the
   source `index_0`, so the {1,2,4,6,7,9} guard's exact trigger is
   subtle. The port reproduces the OBSERVED drop (skip {1,2,4,6,7,9}
   when `stage==0`); re-derive if a level regresses on stage
   registration.

4. **StageVar retrigger cadence (`0x60` bits).** `sub_12330`
   (EF:4986-5008) implements every-1/2/4-tick gating via
   `str_0x3647A_2._axis_2d.y & 3`. Ported nowhere yet; verify the
   cadence counter's persistence across save/load if StageVars land.

5. **`sub_88B20` (EF:40887) / `sub_88BA0` (EF:40969).** The advance
   ping and box-teardown helpers — not traced to their sound/UI ids.
   Distinct from the 41/61 chime path in §4b. Trace if the objective
   UI is ported.

6. **`byte_0x36E0B` origin.** Set by `AddSwitch31atyp_50FF0` (EF:37308,
   `|= 1`) — the type-31 beacon switch. Confirms bit0 = "beacon/switch
   objective" (→ sound 41). Other bits of `byte_0x36E0B` unread here.

---

## Port-completion checklist (Phase 4.1)

- [ ] **Advance chime = 61 (Success2), not 41** (§4b), suppressed at
      `ObjectiveText_1 == 0`; 41 only for beacon rows (`byte_0x36E0B & 1`).
- [ ] **Objective text index** = `IndexLevelText_DB4EE[level] +
      ObjectiveText_1` into `langindexbuffer` (§4c) — for the HUD track.
- [x] **StageVars subsystem** (§2): loader `InitStageVars_11EE0`, the
      `StageVars2_0x365F4[11]` table, `sub_12780` per-tick scan,
      `sub_12500` per-entity reaction, `sub_122C0` disposition-arm,
      `sub_12100` attach-at-spawn + held action-index. LANDED 2026-07-14
      in `crates/mgc-sim/src/mc2/stagevars.rs` (see
      docs/MC2-STAGE-ENGINE-GAPS.md §B). HELD = freeze (the phase-7
      `site_z` early-return); state in two hash-gated `World` vecs so
      `Ent`/MC1 goldens are untouched. **§2 CORRECTIONS from the port**:
      source byte1 = a CHAIN/re-arm slot (not a cadence seed);
      `sub_12410` re-arms-or-clears (not a bare actionIndex set); `&1`
      selects subtype-vs-index spawn match; `sub_12870` has two call
      sites (EF:5123 + EF:32994); the kind-3/4/5 `&2` handle-tracking
      branches are dormant in retail (no writer). Verified: kind-3 (bound
      death) + kind-6 (timer) release end-to-end.
- [ ] **Remaining objective types** 1/2/3/4/6/8/9 (§5) — currently
      `_ => false`. Types 3/8 need the rival alive-flags (present via
      `rivals`); type 9 needs the class-0xA building list; types 1/2/4/6
      need entity binding (`sub_58DA0`).
- [ ] **Type-0 HUD bar** `word_0x2FED5` (§4d) — banked for 4.9.
- [ ] **The level-END-gated switch** (EF:54329, exit-point release) as a
      sibling of the m32 stage-gated switch (§1e) — verify the exit
      demon-mouth / trigger-point release routes through it.
