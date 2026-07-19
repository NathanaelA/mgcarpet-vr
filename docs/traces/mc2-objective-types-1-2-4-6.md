# MC2 Objective Types 1 / 2 / 4 / 6 — Port-Ready Decompile Trace

BANKING trace of the four remaining unported MC2 objective types, so a
future session can implement them without re-deriving. Companion to
`docs/traces/mc2-stage-engine-completion.md` (the stage engine as a
whole); this file drills only into types **1, 2, 4, 6** and the
**entity-binding** machinery they share.

All file:line citations are into `reference/remc2/remc2/engine/`
(EF = EventsFunctions.cpp, E = Events.cpp, LS = LevelStructs.h,
BT = BasicTerrain.h, GT = global_types.h, GU = GameUI.cpp). Line numbers
are as of this session. Port references are into
`crates/mgc-sim/src/mc1/world.rs` unless noted.

Status at time of writing: types **0, 3, 5, 7, 8, 9** are PORTED in
`objective_mc2` (world.rs:4154). Types **1, 2, 4, 6** are `_ => false`
(world.rs:4281). None of 1/2/4/6 is exercised by the certified level set;
this doc is for completeness.

---

## 0. The shared machinery: entity binding (`sub_58DA0` / `sub_59760`)

Types 1, 2, 4 (kill-a-specific-creature / escort) and 6 (collect item)
cannot test a raw model — they name **one specific authored entity** by
its index into the level's THING/entity table. That entity is not live at
load; it materializes later (via disposition-fire / a stage-gated m32
switch). So each such row must **bind** to the live entity when it spawns.
This is the piece not yet ported.

### 0a. What InitStages stores (the pre-bind payload)

`InitStages_58940` (EF:40611-42) — for types 1/2/4/6 the payload is a
pointer into the **source** THING table, i.e. an index `stage_1`:

```cpp
case 1:
case 2:
case 6:
    D41A0_0.stages_0x3654C[...].str_36552_un.ptr16u =
        &terrain->entity_0x30311[terrain->stages_0x36442[result].stage_1].type_0x30311;   // EF:40619
    break;
case 4:
    D41A0_0.stages_0x3654C[...].str_36552_un.ptr16u =
        &terrain->entity_0x30311[terrain->stages_0x36442[result].stage_1].type_0x30311;   // EF:40622
    D41A0_0.stages_0x3654C[...].str_3654E_axis.x = terrain->stages_0x36442[result]._axis_2d.x << 8;  // EF:40623
    D41A0_0.stages_0x3654C[...].str_3654E_axis.y = terrain->stages_0x36442[result]._axis_2d.y << 8;  // EF:40624
    break;
```

So `str_36552_un` initially holds a **pointer to `entity_0x30311[stage_1]`**
(the authored THING record). `stage_1` is the checkpoint's `stage`
parameter = **the THING-table index of the named entity**. Type 4 also
stores a fixed fly-to point in `str_3654E_axis` (`<< 8` = engine units).

**Drop-guard (EF:40589-602):** rows of type {1,2,4,6,7,9} whose
`stage_1 == 0` are dropped (`index_0 = -1`). The port reproduces this
in `set_mc2_stages` (world.rs:4105: `matches!(index, 1|2|4|6|7|9) &&
stage == 0 → continue`). *(The decompile switch at :40589 reads the
DEST row's byte0 (still 0 pre-store) rather than the source `index_0`,
so the exact trigger is subtle — the port reproduces the OBSERVED drop.
See stage-engine trace OPEN item #3.)*

### 0b. The bind at spawn — `sub_58DA0` (EF:40650-90)

Called at every class-5 / class-A spawn (28 call sites, EF:33031-33247;
the dispatcher context at EF:33022-33031 shows `entity` = the THING
template `type_entity_0x30311*`, `v3x`/`indexx` = the freshly-created
live entity `type_entity_0x6E8E*`). Full body:

```cpp
void sub_58DA0(type_entity_0x30311* a1x, type_entity_0x6E8E* a2x) {   // EF:40650
    for (int i = 0; i < D41A0_0.stageIndex_0x36E01; i++) {
        switch (D41A0_0.stages_0x3654C[i].stages_3654C_byte0) {
        case 1:
        case 2:
        case 4:
            if (a1x == D41A0_0.stages_0x3654C[i].str_36552_un.ptr0x30311) {   // EF:40659
                D41A0_0.stages_0x3654C[i].str_36552_un.ptr0x6E8E = a2x;       // EF:40661 — payload becomes the LIVE entity ptr
                D41A0_0.stages_0x3654C[i].str_3654D_byte1 |= 1;              // EF:40662 — set the "bound" bit
            }
            break;
        case 3:                                                              // (already ported: bind by wizard color)
            if (a2x->class_0x3F_63 == 3) {
                if (!a2x->model_0x40_64 || a2x->model_0x40_64 == 1) {
                    if (a2x->dword_0xA4_164x->playerColorIndex_0x38_56 ==
                        D41A0_0.stages_0x3654C[i].str_36552_un.dword) {
                        D41A0_0.stages_0x3654C[i].str_36552_un.ptr0x6E8E = a2x;
                        D41A0_0.stages_0x3654C[i].str_3654D_byte1 |= 1;
                    }
                }
            }
            break;
        case 6:
            if (a1x == D41A0_0.stages_0x3654C[i].str_36552_un.ptr0x30311) {   // EF:40679 — SAME template match as 1/2/4
                D41A0_0.stages_0x3654C[i].str_36552_un.dword = a2x - D41A0_0.struct_0x6E8E;  // EF:40681 — payload becomes the ENTITY INDEX
                D41A0_0.stages_0x3654C[i].str_3654D_byte1 |= 1;             // EF:40682
            }
            break;
        default:
            continue;
        }
    }
}
```

Key facts:
- **Types 1/2/4** bind by **template-pointer equality**: `a1x` (the THING
  record being spawned) equals the stored `&entity_0x30311[stage_1]`.
  After the match, `str_36552_un` holds the **live entity pointer** and
  `str_3654D_byte1 |= 1` (the `& 1` "bound" bit the predicates gate on).
- **Type 6** binds by the SAME template match but stores a different
  payload: `a2x - struct_0x6E8E` = the **live entity INDEX** (pointer
  minus the pool base). Its predicate then hunts that index inside the
  inventory (§4).
- **Type 3** binds by player color (already ported via `mc2_rivals`); not
  our concern here, quoted for contrast only.
- `a1x == 0` at EF:43728 (the player-spawn path) matches nothing for
  types 1/2/4/6 (they require a non-null template equal to their stored
  pointer), so player spawn never mis-binds them.

**Port equivalent of "template match":** the port's `spawn_from_thing(ti)`
stamps every live entity with `thing_slot = ti` (world.rs:4570, run
unconditionally on the normal spawn path; also :4386 for placeholders).
So `a1x == &entity_0x30311[stage_1]` reduces to
**`new_entity.thing_slot == stage_1`**, and `stage_1` is exactly what
`set_mc2_stages` already stores in `st.target` for these types (the `_ =>
stage as u32` arm, world.rs:4126). No new payload plumbing is needed for
the pre-bind value — only a live-slot binding field.

### 0c. The re-point on transform — `sub_59760` (EF:40922-54)

Type 2's target may transform into a successor entity (morph/hatch); the
bound pointer must follow. Called from EF:28204 (`sub_59760(event,
tempEvent)` — the transform path, old entity → new entity):

```cpp
void sub_59760(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x) {   // EF:40922  a1x=old, a2x=new
    ... for each player, if !IsLevelEnd_0:
        for (j = 0; j < stageIndex_0x36E01; j++)
            if (struct_0x3659C[ix].substr_3659C.stage_0x3659F[j] == 1)      // row still active
                if (D41A0_0.stages_0x3654C[j].stages_3654C_byte0 == 2       // EF:40944 — TYPE 2 ONLY
                    && D41A0_0.stages_0x3654C[j].str_36552_un.ptr0x6E8E == a1x)   // bound to the OLD entity
                    if (D41A0_0.stages_0x3654C[j].str_3654D_byte1 & 1)      // and actually bound
                        D41A0_0.stages_0x3654C[j].str_36552_un.ptr0x6E8E = a2x;   // EF:40947 — re-point to the NEW entity
}
```

Only **type 2** re-points. This is the machinery that keeps an "escort/
kill this creature through its whole transform chain" objective alive: if
you must kill the boss and the boss morphs, the row now tracks the morph.
Type 1 does NOT re-point (see the predicate difference in §1 vs §2).

**Port note:** the port's transform paths are `mc2::morph` and the
class-10 multipart/hatch chains. Wherever a live entity is replaced by a
successor (old slot dies / new slot spawns as the "same" logical
creature), a type-2 bound row must swap `bound` from the old slot to the
new. Because there is no MC2 level authoring a type-2 chain on the
certified set, this re-point is a completeness requirement, not a
correctness blocker today. Flag: the port's morph path needs a hook that
`objective_mc2` (or a helper) can observe.

---

## 1. Type 1 — kill creature (no transform-follow)

### 1a. InitStages payload
`str_36552_un.ptr16u = &entity_0x30311[stage_1]` (EF:40619). Pre-bind =
THING index `stage_1`. Dropped if `stage_1 == 0` (§0a). No axis, no
dword.

### 1b. Completion predicate (EF:40763-70)
```cpp
case 1:
    if (D41A0_0.stages_0x3654C[v3].str_3654D_byte1 & 1                     // EF:40764 — must be bound
        && D41A0_0.stages_0x3654C[v3].str_36552_un.ptr0x6E8E->life_0x8 <= -1) {   // EF:40765 — bound entity dead
        achievedGoal = true;
        D41A0_0.struct_0x3659C[v0x].substr_3659C.stage_0x3659F[v3] = 2;
    }
    break;
```
- Reads the **bound entity's `life_0x8` (port `act_life`)**; done when
  `<= -1`.
- Requires the `& 1` bound bit (so it cannot fire before the target
  spawns).
- **NOT** current-stage gated — `case 1` does not test `v3 ==
  ObjectiveText_1`. It completes in ANY row position (a background
  objective).

### 1c. Binding
Bind-at-spawn by template match (§0b, case 1). No re-point (type 1 is not
in `sub_59760`'s `== 2` filter) — if the target transforms, the row is
left pointing at the dead husk, which reads `life <= -1` and completes.
That is the intended semantics: "the named creature is gone."

### 1d. Port recipe
```rust
// Type 1 (kill named creature): done when the bound entity is dead.
// NOT current-stage gated (background objective). Requires bound.
1 => st.bound.is_some_and(|b| {
    self.g.ent.get(b).is_some_and(|e| e.act_life <= -1)
}),
```
New machinery: a per-row `bound: Option<usize>` slot + set it at the
binding seam (§5).

---

## 2. Type 2 — kill creature, genuinely dead (transform-aware)

### 2a. InitStages payload
Identical to type 1: `str_36552_un.ptr16u = &entity_0x30311[stage_1]`
(EF:40619, shares the `case 1: case 2: case 6:` arm). Pre-bind = THING
index. Dropped at `stage_1 == 0`.

### 2b. Completion predicate (EF:40771-79)
```cpp
case 2:
    if (D41A0_0.stages_0x3654C[v3].str_3654D_byte1 & 1                          // bound
        && D41A0_0.stages_0x3654C[v3].str_36552_un.ptr0x6E8E->life_0x8 <= -1    // dead
        && !(D41A0_0.stages_0x3654C[v3].str_36552_un.ptr0x6E8E->fontTypeIndex_0x3D_61)) {  // EF:40774 — NOT mid-transform
        achievedGoal = true;
        D41A0_0.struct_0x3659C[v0x].substr_3659C.stage_0x3659F[v3] = 2;
    }
    break;
```
The type-1 test PLUS `!fontTypeIndex_0x3D_61`. `fontTypeIndex_0x3D_61`
(offset 61, `int8_t fontTypeIndex_0x3D_61`, GT:355) is nonzero while the
entity is in a transform/morph state. So type 2 = "dead AND not merely
transforming into a successor" — it will not accept a `life<=-1` that is
actually a morph handoff; combined with `sub_59760`'s re-point (§0c) the
row follows the chain until a genuine death. Not current-stage gated.

**What makes 2 distinct from 1:** type 1 accepts any `life<=-1` (husk
after morph counts as done); type 2 rejects a death that is a transform
(`fontTypeIndex != 0`) and instead re-points to the successor via
`sub_59760`. Type 2 = "kill it for real, through every transformation";
type 1 = "the original instance is gone."

### 2c. Binding
Bind at spawn (§0b, case 2) + **re-point on transform** (§0c, the only
type in `sub_59760`). The port must swap `bound` at its morph/hatch
handoff.

### 2d. Port recipe
```rust
// Type 2 (kill for real, transform-aware): bound entity dead AND not
// in a transform pose. Re-point `bound` across morph handoffs
// (mirrors sub_59760). NOT current-stage gated.
2 => st.bound.is_some_and(|b| {
    self.g.ent.get(b).is_some_and(|e| {
        e.act_life <= -1 && e.f61_morph == 0   // f61 = fontTypeIndex_0x3D_61
    })
}),
```
New machinery: the `bound` slot (shared with 1/4) + a morph-pose field
(`fontTypeIndex_0x3D_61`, port offset 61) if not already surfaced + a
re-point hook on the transform path. **Gotcha:** `fontTypeIndex_0x3D_61`
is the port's offset-61 byte — verify it is modeled (the port's morph
code, `mc2::morph`, may key transform pose off a different field; confirm
which port field is nonzero during a morph before trusting this test).

---

## 3. Type 4 — escort a player-owned entity to a point

### 3a. InitStages payload
`str_36552_un.ptr16u = &entity_0x30311[stage_1]` **AND**
`str_3654E_axis = _axis_2d << 8` (EF:40622-24). Pre-bind = THING index
`stage_1` (the entity to escort) + a fixed fly-to point (engine units).
Dropped at `stage_1 == 0`.

### 3b. Completion predicate (EF:40787-802)
```cpp
case 4:
    if (D41A0_0.stages_0x3654C[v3].str_3654D_byte1 & 1) {                       // EF:40788 — bound
        v20 = D41A0_0.stages_0x3654C[v3].str_36552_un.dword;                    // EF:40790 — bound live-entity ptr (as int)
        if (v17x == Entities_EA3E4[*(unsigned __int16*)(v20 + 40)]) {           // EF:40791 — player == Entities[bound.parentId]
            v11 = D41A0_0.stages_0x3654C[v3].str_3654E_axis.x - (int16_t)*(int16_t*)(v20 + 76);  // EF:40793 — dx = point.x - bound.pos.x
            if ((abs)(v11) <= 768                                               // EF:40794 — |dx| <= 768 (3 tiles)
                && abs(D41A0_0.stages_0x3654C[v3].str_3654E_axis.y - *(int16_t*)(v20 + 78)) <= 768) {  // EF:40795 — |dy| <= 768
                achievedGoal = true;
                D41A0_0.struct_0x3659C[v0x].substr_3659C.stage_0x3659F[v3] = 2;
            }
        }
    }
    break;
```
`v17x` is the **player entity** (`Entities_EA3E4[array_0x2BDE[LevelIndex_0xc]
.playerIndex_0x00a_2BE4_11240]`, set at EF:40733). The offsets read off
the bound entity (`v20`):

| offset | struct field (GT) | meaning |
|---|---|---|
| +40 | `parentId_0x28_40` (uint16, GT:342 "WHO OWNS ME") | owner entity index |
| +76 | `position_0x4C_76.x` (axis_3d at 0x4C, GT:371) | escort X |
| +78 | `position_0x4C_76.y` | escort Y |

So: **the bound entity's owner (`parentId_0x28_40`) must resolve to the
PLAYER entity, AND the bound entity must be within 3 tiles (768 engine
units) of the fixed point.**

Not current-stage gated (`case 4` has no `v3 == ObjectiveText_1`). It can
complete as a background row — but only while the escort is player-owned
and at the point.

### 3c. **Type 4 vs type 5 (the framing question)**
Confirmed from EF:40787-814:
- **Type 5** (EF:40803-14) tests **the PLAYER's OWN position**:
  `str_3654E_axis - (int16_t)v17x->position_0x4C_76.x/.y <= 768`, gated
  `v3 == ObjectiveText_1`. It is a plain "fly YOURSELF to the beacon."
- **Type 4** tests a **BOUND entity's position** (`v20 + 76/78`, NOT the
  player's) AND additionally requires that entity's owner
  (`*(u16*)(v20+40)`) to be the player. It is **an escort**: "get this
  (player-owned) creature to the point." It is **not** merely a
  fixed-point fly-to with a different subject — the owner gate
  (`v17x == Entities[bound.parentId]`) is what makes it distinct: the
  escorted entity must be under the player's control at the moment it
  reaches the point. If the player loses ownership (never possessed it,
  or it changed hands), the row does not complete even if the entity sits
  on the point.

### 3d. Binding
Bind at spawn by template match (§0b, case 4 shares the `1/2/4` arm). No
re-point (only type 2 is in `sub_59760`).

### 3e. Port recipe
```rust
// Type 4 (escort a player-owned entity to a point). NOT current-stage
// gated. Requires bound + player-owned + within 3 tiles of st.point.
4 => st.bound.is_some_and(|b| {
    self.g.ent.get(b).is_some_and(|e| {
        e.owner_is_human()                        // parentId+40 resolves to the human
            && (st.point.0 as i32 - e.x as i16 as i32).abs() <= 768
            && (st.point.1 as i32 - e.y as i16 as i32).abs() <= 768
    })
}),
```
(Mirror type-5's existing wrapping-min distance form at world.rs:4192-96
for the ±32768 wrap correctness.)

**Gotcha / open risk (the owner field):** the decompile reads
`parentId_0x28_40` (offset 40). The port's offset-40 field is `f40`
("attacker latch", features.rs:395) — a DIFFERENT semantic than the
remc2 "WHO OWNS ME" guess; the two projects diverge on this byte. The
port models entity ownership elsewhere (`f144` = owner tag, 0 on the
human's — rivals.rs:285; `f66` = team). Because the human is out-of-pool
(`crate::mc1::mobs::PLAYER_TARGET`), "owned by the player" most likely
means the escort's owner tag equals the human's (candidate: `f144 == 0`,
the human tag). **Do not assert this** — no shipped level authors type 4
(§6), and the escort-ownership path (how a possessed/gifted creature gets
its owner set) is itself unported. Resolve at implementation time by
tracing how a possessed creature's owner is stamped, then pick the
matching port field. Flag it a per-port decision, not a settled mapping.

---

## 4. Type 6 — collect a named item into the 8-slot inventory

### 4a. InitStages payload
`str_36552_un.ptr16u = &entity_0x30311[stage_1]` (EF:40619, shares the
`1/2/6` arm). Pre-bind = THING index. Dropped at `stage_1 == 0`.

### 4b. The bind rewrites the payload to an ENTITY INDEX
Unlike 1/2/4 (which store a live *pointer*), `sub_58DA0` case 6 stores
`str_36552_un.dword = a2x - struct_0x6E8E` (EF:40681) = the **live entity
INDEX** of the item. So after binding, `payload.dword` is a pool index,
not a model and not a pointer.

### 4c. Completion predicate (EF:40815-27)
```cpp
case 6:
    if (v3 == D41A0_0.struct_0x3659C[v0x].substr_3659C.ObjectiveText_1        // EF:40816 — CURRENT stage only
        && D41A0_0.stages_0x3654C[v3].str_3654D_byte1 & 1) {                   // AND bound
        v13 = 0;
        while (D41A0_0.array_0x2BDE[v24].dword_0x3E6_2BE4_12228.str_0x1AC_428.word_2BDE_12658[v13]
               != D41A0_0.stages_0x3654C[v3].str_36552_un.dword) {            // EF:40819 — scan inventory for the entity index
            if (++v13 >= 8)
                goto LABEL_72;                                                 // not found → not done
        }
        achievedGoal = true;                                                   // found → done
        D41A0_0.struct_0x3659C[v0x].substr_3659C.stage_0x3659F[v3] = 2;
    }
    break;
```
- **Current-stage gated** (`v3 == ObjectiveText_1`) AND requires the
  `& 1` bound bit.
- Scans the player's **8-slot inventory** `word_2BDE_12658[0..8]`
  (GT:115, inside `type_str_0x1AC_428` at player struct offset 0x1AC,
  LS:131 `dword_0x3E6_2BE4_12228`) for a slot equal to the bound
  **entity index**. Found → objective done.

### 4d. **Type 6 (the framing question): what the inventory holds, and what writes it**
- The inventory `word_2BDE_12658[8]` holds **live entity INDICES** of
  collected items (that is exactly what `sub_58DA0` case 6 puts into the
  comparand — `a2x - struct_0x6E8E`). So a type-6 objective is
  "**pick up the specific placed item whose entity ends up in your
  carry-slots**."
- **What writes the inventory: NOTHING in the decompile.** An exhaustive
  search finds `word_2BDE_12658[...]` **read** only at EF:40819 (this
  predicate) and **cleared** only at EF:43719 (`memset(&...->str_0x1AC_428,
  0, 18)` — the player-init zero of the whole 18-byte `type_str_0x1AC_428`
  = `word_0` + 8×`int16`). There is **no store site** anywhere in
  `engine/`. The pickup path that would populate the carry-slots is absent
  from the decompile.
- **Therefore:** in the shipped engine type 6 **can never complete** — the
  array it scans is always all-zero, and a bound entity index is nonzero,
  so the scan always falls through to `LABEL_72`. Type 6 is effectively
  vestigial / dead in remc2.
- **Subsystem to model first:** an item-pickup/carry-slot system that
  writes an item's entity index into `word_2BDE_12658` on collection.
  Since the decompile contains no such writer, there is **no reference to
  port from** — it would be an invented mechanism. Given that and zero
  authored levels (§6), type 6 is the lowest priority.

### 4e. Payload-semantics OPEN risk (model vs entity index)
The BT:36-46 legend leaves type 6 blank. The payload is **NOT a model**:
`sub_58DA0` case 6 overwrites it with a live **entity index**
(`a2x - struct_0x6E8E`, EF:40681), and the predicate compares that index
against the inventory. This differs from type 7 (which stores a model)
and from 1/2/4 (which store a live pointer). A port must therefore:
store the THING index pre-bind (like 1/2/4), then on bind store the
**live pool slot** (not a model, not a pointer), then match that slot
against whatever a carry-slot system records. Do not reuse the type-7
"model extinct" shape.

### 4f. Port recipe (skeleton — blocked on the carry-slot subsystem)
```rust
// Type 6 (collect named item): current-stage gated, requires bound.
// BLOCKED: needs an item-carry inventory that records collected items'
// pool slots — no writer exists in the decompile (see §4d). Until then,
// this stays `_ => false`.
6 => idx == self.mc2_stage_current
    && st.bound.is_some_and(|b| self.human_inventory_slots().contains(&b)),
```

---

## 5. The binding seam (minimal new machinery)

Everything types 1/2/4/6 need reduces to **one new per-row field** plus
**one hook** the port already has a natural home for.

### 5a. Data
Add to `Mc2Stage` (world.rs:495):
```rust
/// Live pool slot the row's named target bound to (sub_58DA0,
/// EF:40650-90). `None` until the referenced THING (index = `target`
/// for types 1/2/4/6) spawns. The decompile's `str_3654D_byte1 & 1`
/// bound bit is `bound.is_some()`.
bound: Option<usize>,
```
`target` already carries the THING index for these types (the `_ => stage
as u32` arm, world.rs:4126). No payload-store change is needed in
`set_mc2_stages` beyond initializing `bound: None`.

### 5b. The bind hook
`spawn_from_thing(ti)` sets `thing_slot = ti` at world.rs:4570 (the
normal path) — the exact analog of `sub_58DA0`'s template match. Right
after that line, walk the stages:
```rust
// sub_58DA0 (EF:40650-90): bind a named-target objective row to the
// entity it just spawned. Template match = thing_slot == target.
for st in &mut self.mc2_stages {
    if matches!(st.kind, 1 | 2 | 4 | 6) && st.state == 1
        && st.bound.is_none() && st.target as usize == ti {
        st.bound = Some(s);
    }
}
```
This serves **all four** types (1/2/4 read the slot's fields; 6 matches
the slot against the inventory). Note `sub_58DA0` binds every matching
row each spawn even if already bound; the port's `bound.is_none()` guard
binds once (the first spawned instance), which matches "one authored
named entity" — verify against a real type-1/2 level if one ever surfaces
(a template shared by multiple THINGs would bind only the first).

### 5c. The type-2 re-point hook (`sub_59760`, EF:40922-54)
Only type 2 needs it. At the port's transform/morph handoff (old slot →
new slot), swap the bound slot:
```rust
// sub_59760 (EF:40922-54): follow a type-2 target across a transform.
for st in &mut self.mc2_stages {
    if st.kind == 2 && st.state == 1 && st.bound == Some(old_slot) {
        st.bound = Some(new_slot);
    }
}
```
Place this wherever `mc2::morph` (and any hatch/multipart succession)
replaces a logical creature. Low urgency (no authored type-2 chain).

### 5d. Hash discipline
`mc2_stages` is already hashed only when populated (world.rs:518 note);
adding `bound: Option<usize>` to the struct is covered by the existing
`Mc2Stage` hashing at world.rs:2262-63 area — confirm the new field is
included in the state hash so goldens catch binding regressions (follow
the existing `mc2_stage_current`/`mc2_objective_pause` hashing, and the
"hash-only-when-pending / when-populated" pattern from prior sessions).

---

## 6. Exposure + priority

Census counts (authored objective rows across the shipped level set):

| type | levels authoring it | shares binding? | priority |
|---|---|---|---|
| **2** | 27 | yes (bind + type-2 re-point) | **HIGH — do first** |
| **1** | 21 | yes (bind only) | **HIGH — falls out with 2** |
| **4** | 0 | yes (bind + owner check) | LOW (completeness) |
| **6** | 0 | yes (bind) + needs carry-slot subsystem | LOWEST (blocked) |

Distinct levels authoring 1 or 2 = **41**. So the binding seam (§5) is the
single highest-leverage piece: it unlocks types 1 and 2 (41 levels) at
once, and lays the exact groundwork for 4 and 6.

**Recommended order:**
1. **Land the binding seam (§5a/§5b) + types 1 and 2.** One `bound` field
   + one hook at world.rs:4570 covers both predicates. Type 2 also wants
   the morph re-point (§5c) and the offset-61 morph-pose field (§2d);
   land the re-point but it can trail if no type-2 transform level is on
   the immediate test list.
2. **Type 4** — same `bound` field, add the player-owner check. Resolve
   the owner-field ambiguity (§3e) first. Zero authored levels → do it for
   completeness / a future level editor, low urgency.
3. **Type 6** — needs the item carry-slot inventory
   (`word_2BDE_12658`), which **no decompile code writes** (§4d). It is
   both zero-authored and blocked on an unreferenced subsystem; defer
   until an item-pickup model exists (or a level authors type 6 and
   forces the question).

Shared pieces: the **binding slot + bind hook (§5b)** serves 1/2/4/6; the
**type-2 re-point (§5c)** is 2-only; the **owner check (§3e)** is 4-only;
the **carry-slot inventory (§4d)** is 6-only. Types 4 and 6 are authored
in **zero shipped levels** (reachable only via a hypothetical level
editor) — wanted for completeness, not for any certified level.

---

## 7. The one-line gotcha per type

- **Type 1:** completes on ANY `life <= -1` (a morph husk counts) and is
  **not** current-stage gated — a background kill row. Needs only `bound`.
- **Type 2:** the extra `!fontTypeIndex_0x3D_61` term rejects a death that
  is really a transform; it demands the **`sub_59760` re-point** to follow
  the chain. Verify which port field is the morph-pose byte before
  trusting the `f61 == 0` test.
- **Type 4:** it is an **escort, not a fly-to** — the distinguishing gate
  is `player == Entities[bound.parentId+40]` (the escort must be
  player-OWNED). The owner field is genuinely ambiguous between the
  port's `f40` (byte-offset match) and its semantic owner (`f144`/`f66`);
  resolve against the possession/gift path, do not assume.
- **Type 6:** the inventory it scans (`word_2BDE_12658`) is **written by
  nothing in the decompile** (only zeroed at EF:43719) — type 6 can never
  complete in retail and is blocked on an item carry-slot subsystem that
  has no reference implementation to port; the payload after bind is a
  live **entity index**, not a model.
