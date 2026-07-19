I have thoroughly traced the subsystem. Let me write the complete structured report.

---

# MULTIPART / SEGMENT-CHAIN Subsystem — Verbatim Trace (remc2 / Magic Carpet 2)

All addresses use `sub_XXXXX = address − 0x1E1000`. All file:line refer to `reference/remc2/remc2/engine/EventsFunctions.cpp` (EF) or `.../Events.cpp` (EV). Every claim is cited. Fields named as in the decomp (e.g. `word_0x34_52`).

Key chain-link fields:
- `word_0x32_50` = link to PARENT (index into `D41A0_0.struct_0x6E8E` = `Entities_EA3E4`).
- `word_0x34_52` = link to NEXT child (0 = end of chain).
- `parentId_0x28_40` = owner/creator id used for XP credit (NOT the chain link).
- `word_0x96_150` = target/head-ref (varies by model).
- `byte_0x3E_62` = per-child index / phase counter.
- `byte_0x46_70` = tail length param (m22) / sub-state (m27 segments) / child index.

---

## 1. CONSTRUCTORS (CTORS)

### 1.0 — Model 0 (worm/hydra "class-5 model 0"): `sub_4B240` (EF:33642, addr 0x4B240 / "22c240")

Free-slot gate: `if (sub_4A810_get_0x35plus() < 16) return 0;` (EF:33655) — needs ≥16 free entity slots.

Head init (EF:33660-33690):
- `actionIndex_0x45_69 = 1` (state 0x01), `class_0x3F_63 = 5`, `model_0x40_64 = 0`.
- `minSpeed_0x84_132 = 80`, `maxSpeed_0x86_134 = 16`, `actSpeed_0x82_130 = 30`.
- `maxLife_0x4 = 4000`; `mana_0x90_144 = 4500` (EF:33668-69).
- `maxMana_0x8C_140 = mana (4500)`; then `mana_0x90_144 = 4500/2 = 2250` (EF:33670-71).
- RNG draw #1: `rand = 9377*rand + 9439`; `roll_0x20_32 = (rand & 0x7FF) - 1`; `yaw_0x1C_28 = (rand & 0x7FF) - 1` (same draw); `pitch_0x1E_30 = roll` (EF:33672-75). Only ONE RNG advance.
- `fov_0x22_34 = 0`, `word_0x36_54 = 96`, `byte_0x38_56 = 1`.
- `dword_0x10_16 = (self - struct_0x6E8E) % 100` (EF:33679).
- `array_0x10[model]++` → v6y (the model-0 instance counter), stored later as `byte_0x3E_62` (EF:33680-82).
- `dword_0xA0_160x = &str_D7BD6[71]` (animation/type row 71) (EF:33681).
- `v6z = (int16)byte_0x3E_62 % word_160_0x1a_26` (EF:33683).
- `xtype_0x41_65 = 3`, `word_0x2C_44 = 0`, `byte_0x46_70 = 0`, `fontTypeIndex_0x3D_61 = 0`.
- `byte_0x39_57 = word_160_0x1a_26 - v6z + 4` (sprite/frame base) (EF:33690).

CHILD SPAWN LOOP — 16 children, `v15 = 0..15` (EF:33691-33712):
```
for v15 in 0..=15:
    v8x = NewEvent_4A050()            # NO free-slot re-check; if null, child skipped
    if v8x:
        qmemcpy(v8x, v12x/*head*/, 0xA8)          # copy 168 bytes of head into child
        v8x->word_0x32_50 = prev(v14x) - struct   # child.PARENT = previous link (head first, then prior child)
        v14x->word_0x34_52 = v8x - struct          # prev.NEXT = child
        v8x->word_0x34_52 = 0                       # child.NEXT = 0 (tail)
        v8x->actionIndex_0x45_69 = 232 (0xE8)       # segment tick state
        v12x->mana_0x90_144 = (maxMana - ...) >> 5  # head mana set to maxMana/32 ≈ 4500/32 = 140 (recomputed each iter!)
        v8x->byte_0x3E_62 = v15
        SetEntityIndexAndRot_49CD0(v8x, v15 + 19)   # sprite/rot row = 19 + childIndex
        v9x->word_0x36_54 = v9x->array_0x52_82.pitch
        AddEventToMap_57D70(v9x, position)
        CopyMaxLifeToLife_49A20(v9x)
    v14x = v13x (the just-created child, becomes prev)
```
- Chain topology: HEAD ← child0 ← child1 ← … ← child15. Each child's `word_0x32_50` points at its PREDECESSOR; each predecessor's `word_0x34_52` points at the child. So `word_0x34_52` walks head→tail, `word_0x32_50` walks child→parent. HEAD's `word_0x32_50` is whatever the qmemcpy copied (0 unless head had one) — head keeps its own; only children get 0x32 rewritten to prev.
- The `mana >> 5` write (EF:33703) writes to `v12x` (the HEAD) every loop iteration (bug-compatible: head mana ends at maxMana/32).
- Head finalize (EF:33713-15): `AddEventToMap`, `CopyMaxLifeToLife`, `SetEntityIndexAndRot_49CD0(head, 40)` (head sprite row 40).

RNG draws in order: exactly ONE (EF:33672). Sound ids: none in CTOR.

### 1.3 — Model 3 (multipart flyer): `sub_4B6F0` (EF:33797, addr 0x4B6F0 / "22c6f0")

No free-slot gate (relies on NewEvent null-checks).

Head init (EF:33808-33835):
- `actionIndex_0x45_69 = 25` (0x19 = base 0x18 + 1 idle), `class = 5`, `model = 3`.
- `minSpeed = 64`, `maxSpeed = 16`, `actSpeed = 30`, `maxLife = 9000` (EF:33815-18).
- `SetEvent144_49C70` (sets mana from a table).
- `maxMana_0x8C_140 = mana`; `mana = mana/2` (EF:33820-21).
- RNG draw #1 (EF:33822-25): one advance; `roll`, `yaw` = `(rand&0x7FF)-1`; `pitch = roll`.
- `fov = 0`, `word_0x36_54 = 96`, `byte_0x38_56 = 1`, `dword_0x10_16 = (self-struct)%100`.
- `dword_0xA0_160x = &str_D7BD6[74]` (EF:33830).
- `byte_0x3E_62 = array_0x10[3]++`.
- `xtype_0x41_65 = 3`.
- `byte_0x39_57 = word_160_0x1a_26 - (byte_0x3E_62 % word_160_0x1a_26) + 4` (EF:33835).

CHILD SPAWN LOOP — 16 children `v12 = 0..15` (EF:33836-33865):
```
for v12 in 0..=15:
    v5x = NewEvent_4A050()
    if v5x:
        qmemcpy(v5x, head, 0xA8)
        v5x->word_0x32_50 = prev(v11x) - struct     # child.PARENT = prev
        v11x->word_0x34_52 = v5x - struct            # prev.NEXT = child
        v5x->word_0x34_52 = 0
        v5x->actionIndex_0x45_69 = 232 (0xE8)
        v5x->mana_0x90_144 = (head->maxMana - ...) >> 5   # ≈ maxMana/32
        v5x->byte_0x3E_62 = v12
        SetEntityIndexAndRot_49CD0(v5x, v12 + 89)    # SPRITE ROW = 89 + childIndex  (m3's "89+i" pattern)
        # per-child particle-driven rotation from particlesParameters_D951C[89+i]:
        v6x->array_0x52_82.pitch = 65 * particlesParameters_D951C[v12+89].speed_6   / 100
        v6x->array_0x52_82.roll  = 65 * particlesParameters_D951C[v12+89].speed_6   / 100
        v6x->array_0x52_82.fov   = 65 * particlesParameters_D951C[v12+89].rotSpeed_8/ 100
        v6x->word_0x36_54 = pitch
        if v12 == 0: v6x->word_0x36_54 = 125 * pitch / 100   # first child link 25% longer
        AddEventToMap_57D70(child, position)
        CopyMaxLifeToLife_49A20(child)
    v11x = child (prev)
```
- Chain topology identical to m0: HEAD ← c0 ← … ← c15 via `word_0x32_50`; forward via `word_0x34_52`.
- Head finalize (EF:33866-71): `AddEventToMap`; `CopyMaxLifeToLife`; `SetEntityIndexAndRot_49CD0(head, 88)` (HEAD sprite row 88); then head's own segment metrics: `pitch/roll = 60*particlesParameters_D951C[88].speed_6/100`, `fov = 60*particlesParameters_D951C[88].rotSpeed_8/100` (EF:33869-71).

RNG draws: exactly ONE (EF:33822). Child mana `>>5` written to CHILD (v5x) here, not head (differs from m0). Sounds: none.

### 1.22 — Model 22 (segmented worm): head `sub_4CA00` (EF:34377), tail spawned separately

Free-slot gate: `if (sub_4A810_get_0x35plus() < 15) return 0;` (EF:34380) — needs ≥15 free slots.

Head init (EF:34385-34410):
- `actionIndex_0x45_69 = 176` (0xB0), `class = 5`, `model = 22`.
- `minSpeed = 128`, `maxSpeed = 16`, `actSpeed = 16`.
- RNG draw #1 (EF:34391-94): one advance; `roll`,`yaw` = `(rand&0x7FF)-1`; `pitch=roll`.
- `maxLife = 2000`, `fov = 0`.
- `dword_0xA0_160x = &str_D7BD6[90]`.
- `byte_0x3E_62 = array_0x10[22]++`.
- `xtype_0x41_65 = 3`, `byte_0x38_56 = 3`, `playerEntityIndex_0x94_148 = 0`.
- `array_0x52_82.yaw = 0`, `animationFrame_0x5C_92 = 0`, `word_0x2C_44 = 11`, `subSpellIndex_0x2A_42 = 0`, `word_0x36_54 = 0`, `word_0x96_150 = 1024`, `word_0x24_36 = 0`, `byte_0x46_70 = 15` (default tail length) (EF:34401-09).
- `byte_0x39_57 = word_160_0x1a_26 - ((int16)byte_0x3E_62 % word_160_0x1a_26) + 4`.
- **Position `alt+384`**: `predictedAxis = *position; predictedAxis.z = getTerrainAlt_10C40(&pred) + 384;` then `AddEventToMap_57D70(head, &predictedAxis)` (EF:34411-13).
- `SetEvent144_49C70`, `CopyMaxLifeToLife` (EF:34414-15). NO children spawned in the CTOR.

**TAIL SPAWN** — happens at map-placement (`sub_4A310`, EF:32998) when a class-5/model-0x16 entity is created from a level-editor tile: `indexx->byte_0x46_70 = entity->par1_14 & 0xff; sub_4CB60(indexx);` (EF:33025-28). So `byte_0x46_70` (tail length) is overridden by the map par1.

`sub_4CB60(event)` (EF:34420):
```
v1 = 1; v5x = event (head)
while v1 <= event->byte_0x46_70 / 2:      # (tailLen/2) rings
    v2 = 0
    while v2 < 2:                          # 2 segments per ring
        v6x = NewEvent_4A050()
        if v6x:
            v3 = (v2 ? -(int16)v1 : v1)     # signed offset: +v1 then -v1
            sub_274C0(event/*head*/, v6x/*new*/, v5x/*prev*/, v3)
        v2++; v5x = v6x                     # prev = new
    v1++
sub_27590(event); sub_27610(event); sub_276E0(event)   # colorize / shift-rot / init
```
So for tailLen=15 → 7 rings × 2 = 14 tail segments, with `byte_0x46_70` values `+1,-1,+2,-2,…,+7,-7`.

`sub_274C0(event1=head, event2=new, event3=prev, a4=offset)` (EF:17845) — the SEGMENT SPAWN primitive:
- `*event2 = *event3` (full struct copy from previous link).
- `event2->word_0x32_50 = event3 - struct` (new.PARENT = prev).
- `event3->word_0x34_52 = event2 - struct` (prev.NEXT = new).
- `event2->word_0x34_52 = 0`.
- `event2->byte_0x3E_62 = abs((int8)a4) & 1` (parity: alternates 0/1 by ring).
- `event2->struct_byte_0xc_12_15.byte[0] &= 0xFB` (clear draw bit 2).
- `event2->byte_0x46_70 = a4` (SIGNED segment offset — this is the chain "distance/side" key).
- `event2->actionIndex_0x45_69 = 180` (0xB4 — tail segment tick state).
- `event2->word_0x2C_44 = 0`, `playerEntityIndex_0x94_148 = 0`, `mana_0x90_144 = 0`.
- `event2->word_0x96_150 = event1 - struct` (segment's HEAD reference = the worm head).
- `predictedAxis = event1->position; AddEventToMap_57D70(event2, &pred); CopyMaxLifeToLife(event2)` (EF:17859-61).

Chain topology (m22): HEAD ← seg0 ← seg1 ← … via `word_0x32_50`; forward via `word_0x34_52`; every segment carries `word_0x96_150 = head`.

Post-spawn helpers:
- `sub_27590` (EF:17867): colorize head + all `word_0x34_52` chain via `sub_278F0(colorIdx, headLen, segOffset)` → `sub_49D50`.
- `sub_27610` (EF:17893): sets each segment `SetEntityShiftRot_49EA0(seg, v/1000, v/1000)` where `v = 550 * particlesParameters_D951C[sub_278F0(...)].rotSpeed_8`.
- `sub_276E0` (EF:17920): for each chain member calls `sub_271D0` (position-follow init).

### 1.27 — Model 27 (3-tier tree/kraken): `sub_4D000` (EF:34591)

Cave gate + free-slot gate (EF:34608-14):
- `if MapType == Cave: v16 = 1` (abort flag).
- else `if (sub_4A810_get_0x35plus() >= 51)` proceed (needs ≥51 free slots).

BODY (tier 0) (EF:34616-24): `NewEvent`; `actionIndex = 0xD9 (217)`, `class = 5`, `model = 27`; `AddEventToMap(body, position)`.

TIER-1 BRANCH LOOP — 5 branches `v15 = 0..4` while `!v16` (EF:34628-34674):
```
for v15 in 0..4:
    v5x = NewEvent()
    if v5x:
        v5x->actionIndex = 0xE9 (233)      # BRANCH tick-state (NULL dispatch)
        v5x->class = 5; v5x->model = 27
        v5x->byte_0x3B_59 = v15            # branch index (0..4), also drives str_D404C[branchIdx]
        v5x->id_0x1A_26   = v4 (=body idx) # id = body index
        v5x->word_0x32_50 = v4             # branch.PARENT = body
        v13x->word_0x34_52 = v5x - struct  # prev.NEXT = branch  (prev = body first iter, else prior branch)
        v5x->word_0x34_52 = 0
        AddEventToMap(branch, position)
        # TIER-2 SEGMENT LOOP — 9 segments per branch, i = 0..8 while !v16:
        for i in 0..8:
            v7x = NewEvent()
            if v7x:
                v7x->actionIndex = 234 (0xEA)   # SEGMENT tick-state (NULL dispatch)
                v7x->class = 5; v7x->model = 27
                v7x->byte_0x3B_59 = v15         # inherits branch index
                v7x->id_0x1A_26   = v4          # id = body
                v7x->word_0x32_50 = v4          # seg.PARENT = body (NOT the branch!)
                v14x->word_0x34_52 = seg-struct # prev.NEXT = seg  (chain: branch→seg0→…→seg8)
                v7x->word_0x34_52 = 0
                AddEventToMap(seg, position)
            else: v16 = 1
            v14x = seg
    else: v16 = 1
    v13x = last-created (branch or its last seg)
    v15++
```
- Total entities = 1 body + 5 branches + 45 segments = 51 (hence ≥51 free slots).
- Chain is a SINGLE linear `word_0x34_52` list: body → branch0 → seg0..8 → branch1 → seg0..8 → … The `word_0x32_50` of every branch AND every segment points at the BODY (all share id_0x1A_26 = body). `byte_0x3B_59` tags branch membership (0..4).

Init dispatch (EF:34678-87):
- `if (!a1x || v16) sub_2AE80(a1x)` (cleanup: disables drawing on whole chain — EF:20831).
- else `sub_2AC50(a1x); sub_2AD40(a1x); sub_2AE30(a1x)`.
- `if (v16) a1x = 0` (spawn fails, returns null).

`sub_2AC50(body)` (EF:20730) — BODY finalize:
- `roll=yaw=pitch = 0`; `minSpeed=64`, `maxSpeed=0`, `actSpeed=30`; `byte_0x3B_59 = 5`; `life_0x8 = 1000000`; `maxLife_0x4 = 36000`; `mana_0x90_144 = 20000` (EF:20739-49).
- `dword_0x10_16 = (self-struct)%100`; `fov=0`; `byte_0x38_56 = 1`; `dword_0xA0_160x = &str_D7BD6[97]`; `byte_0x3E_62 = array_0x10[27]++`; `xtype_0x41_65 = 3`; `struct_byte_0xc[3] |= 0x80`; `byte_0x39_57 = word_160_0x1a_26 + 1` (EF:20750-62).
- `SetEntityIndexAndRot_49CD0(body, 315)`; `SetEntityShiftRot_49EA0(body, 1024, 1536)` (EF:20763-64).

`sub_2AD40(body)` (EF:20770) — BRANCH finalize (walks `word_0x34_52`, only `actionIndex==233`):
- `SetEntityIndexAndRot_49CD0(branch, 316)`; `struct_byte_0xc[3] |= 0xA0`.
- TWO RNG draws per branch: draw#1 → `roll = rand & 0x7FF`; draw#2 → `fov = rand & 0x7FF` (EF:20786-90).
- `minSpeed = 16`, `actSpeed = 16`; `dword_0xA0_160x = &str_D7BD6[103]`; `byte_0x38_56 = 1`.
- `v5 = 460*v2 + 920` (v2 = 1-based branch counter); `life_0x8 = maxLife_0x4 = v5` (EF:20795-97). So branch lives: 1380, 1840, 2300, 2760, 3220.
- `sub_2A940(body, branch)` (positions branch — EF:20798).

`sub_2AE30(body)` (EF:20808) — SEGMENT finalize (walks `word_0x34_52`, only `actionIndex==234`):
- `SetEntityIndexAndRot_49CD0(seg, 317)`; `struct_byte_0xc[3] |= 0xA0`. (No life set here — segments inherit from struct copy? Actually segments were NewEvent'd fresh; their life comes from later `sub_29A90` case 0xC: `life = rand%0x398 + 920`.)

RNG draws in m27 CTOR: 0 in the spawn loops; 2 per branch (10 total) in `sub_2AD40`. Sounds: none in CTOR.

---

## 2. SEGMENT TICK `sub_1B6B0` (EF:8696, state 0xE8=232, dispatched at EV:1021 addr 0x1fc6b0)

Used by m0 and m3 children (both set children to actionIndex 232).

```c
void sub_1B6B0(a1x):                      // a1x = the segment
    v1x = Entities_EA3E4[a1x->word_0x32_50]   // v1x = PARENT (previous link)
    if v1x->class_0x3F_63 != 5:
        DisableEntityDrawing04_57F10(a1x)      // if parent isn't a creature (dead/gone) → hide this segment
    if a1x->byte_0x39_57:                       // "active frame count" nonzero → full follow
        a1x->yaw_0x1C_28   = tan2(&a1x.pos, &v1x.pos)        // face parent
        a1x->pitch_0x1E_30 = radix_tan(&a1x.pos, &v1x.pos)
        predictedAxis = v1x->position                          // start at PARENT position
        MoveEntity_57FA0(&pred, yaw, pitch, -word_0x36_54)     // step BACK by link length word_0x36_54
        CopyEntityPosition_57CF0(a1x, &pred)                    // segment sits behind parent
        if a1x->str_0x5E_94.word_0x62_98:                       // pending damage-hit record
            v3 = str_0x5E_94.dword_0x5E_94    // damage amount
            v4 = a1x->life_0x8
            v5 = str_0x5E_94.word_0x62_98     // attacker id
            str_0x5E_94.word_0x62_98 = 0
            a1x->word_0x26_38 = v5            // remember attacker
            a1x->life_0x8 = v4 - v3           // APPLY damage to this segment's life
        else:
            a1x->word_0x26_38 = 0
    else if !(a1x->byte_0x3E_62 & 3):          // inactive: every 4th child snaps to parent
        CopyEntityPosition_57CF0(a1x, &v1x->position)
        a1x->yaw_0x1C_28 = v1x->yaw_0x1C_28
```

Mechanics:
- **Follow = distance-chain, not interpolation.** Each segment positions itself at its parent's position moved backward along the parent-facing direction by `word_0x36_54` (the per-link length, set in CTOR; m3 first child gets 125% length). This produces a rigid trailing chain, head→tail.
- **Z handling:** implicit — `MoveEntity_57FA0` with pitch moves in 3D, so z tracks parent's pitch/altitude. No explicit terrain clamp here.
- **Death/despawn:** if PARENT is not class 5 (killed/converted), the segment self-hides via `DisableEntityDrawing04_57F10`. Damage lands on the segment's own `life_0x8` from the `str_0x5E_94` hit record; life going negative is handled elsewhere (segment doesn't self-transition here — head death cascade hides it).
- **Draw flag:** `DisableEntityDrawing04_57F10(a1x)` on parent-loss. There is no per-tick redraw enable here; segments are drawn as long as parent is alive.
- Segments in state 0xE8 are SKIPPED in the per-model list rebuild (see §7) and in collision (see §7), so they never independently collide or appear in AI target lists.

---

## 3. m27 states 0xE9 (233) / 0xEA (234) — NULL dispatch confirmation

Both branch entities (actionIndex 233 = 0xE9) and tier-2 segments (actionIndex 234 = 0xEA) have NO independent tick handler — they are NOT dispatched. Instead the BODY's head-brain (`sub_29400/29670/29710/…`) calls `sub_29A90(body)` (EF:19737) every tick, which walks `Entities_EA3E4[body->word_0x34_52]` and processes ONLY entities with `actionIndex_0x45_69 == 233` (EF:19796). For each branch it runs a large sub-state machine on `byte_0x46_70` (0..0xF), and its tier-2 segments (the 9 in `word_0x34_52` after the branch) are moved by `sub_2AA90(body, branch)` (EF:20632, iterates exactly 9 via `do…while(v6<9)`), `sub_2A5B0`, `sub_2A940`, `sub_2A9F0`.

Confirmed: **0xE9 and 0xEA entities do not tick on their own; they are driven entirely from the body via `sub_29A90`.** Segments (0xEA) are additionally skipped in list-rebuild and collision (§7). The list-rebuild (EF:39996) also `continue`s past 0xE8 and skips 0xB4/0xEA from the model list, so branches (0xE9) ARE still added to the per-model list (only 0xE8/0xB4/0xEA are v8-skipped; 0xE9 is not in the skip set — see §7 note).

---

## 4. CHAIN-KILL state 0xB4 (m22 tail segment) = `sub_26CA0` (EF:17420) + walker `sub_26CC0` (EF:17427)

Dispatched: EV:1864 (0x207ca0 → sub_26CA0), EV:1868 (0x207cc0 → sub_26CC0).

`sub_26CA0(a1x)` (EF:17420) — the per-tick handler for a tail SEGMENT in state 0xB4:
```c
sub_271D0(a1x)   // position-follow (see below)
sub_26D20(a1x)   // hit/aggro propagation to the head
```
It is NOT itself the "death walk"; 0xB4 is the *normal tail-segment tick*. The actual chain-KILL walker is `sub_26CC0`:

`sub_26CC0(a1x)` (EF:17427) — CHAIN DESTRUCTION walker:
```c
for i = a1x->word_0x34_52; Entities[i] != Entities[0]; i = v2x->word_0x34_52:
    v2x = Entities[i]
    TransformEntityToManaSphere_36BA0(v2x, false)   // convert each downstream segment to a mana sphere
    DisableEntityDrawing04_57F10(v2x)                 // hide it
TransformEntityToManaSphere_36BA0(a1x, false)         // convert the head/self last
DisableEntityDrawing04_57F10(a1x)
```
- **Cadence:** single pass, entire `word_0x34_52` chain destroyed in ONE call (no per-frame stagger). Walks head→tail.
- **Mana drops:** each segment `TransformEntityToManaSphere_36BA0(seg, false)` — the `false` = no explicit mana-sphere creation trigger flag; conversion follows the segment's own subtype. (Compare m27 `sub_298D0` uses `true`.)
- **Kill credit:** none granted here directly — credit for m22 comes through `sub_26D20`→`sub_6D8B0`-style aggro (see below) and the segment/head `word_0x26_38` attacker tracking.

`sub_271D0(a1x)` (EF:17685) — SEGMENT FOLLOW for m22 (also used by `sub_276E0`):
```c
if a1x->word_0x96_150:                       // has head reference
    v2x = Entities[a1x->word_0x96_150]        // = worm head
    v3 = v2x->subSpellIndex_0x2A_42
    v4 = (v3 + sub_273C0(head->animationFrame, head->word_0x36_54, seg->byte_0x46_70, head->byte_0x46_70)) & 0x7FF
    a1x->word_0x2C_44 = v4                     // angle around head
    v5x = Entities[a1x->word_0x32_50]          // PARENT link
    if v5x && v5x->word_0x32_50: v5x = Entities[v5x->word_0x32_50]   // skip one more up (2 links back)
    predictedAxis = v5x->position
    MoveEntity_57FA0(&pred, v4, 0, a1x->array_0x52_82.pitch + v5x->array_0x52_82.pitch)
    pred.z = v5x->array_0x52_82.pitch - a1x->array_0x52_82.pitch + v5x->position.z
    CopyEntityPosition_57CF0(a1x, &pred)
```
So m22 tail follows a computed spiral angle (`sub_273C0`) around the head, positioned two links back with pitch-based z offset — a spiral/serpentine chain, unlike m0/m3's straight trail.

`sub_26D20(a1x)` (EF:17447) — segment→head aggro/damage relay:
- Reads `Entities[a1x->word_0x96_150]` (the head). If head actionIndex ≥0xB0 (≤0xB0 or ==178):
  - If segment has pending hit (`str_0x5E_94.word_0x62_98`): boosts head `actSpeed` to `((minSpeed-maxSpeed)>>2)+maxSpeed`, turns head toward attacker, steers head `word_0x2C_44` by `±56*abs(seg.byte_0x46_70)/(head.byte_0x46_70>>1)` clamped to [11,227]; clears all segments' hit records.
  - If segment has target flag (`str_0x5E_94.word_0x68_104`) different from head's `playerEntityIndex`: sets head actionIndex=177, `head->dword_0x10_16 = seg.byte_0x46_70 << 8`, head target = attacker, `PrepareEventSound_6E450(target, -1, 4)`, sets flag bit 0x20; clears all segments' target flags.

---

## 5. Death propagation `sub_6D8B0(a1=parentId, a2=code, a3=amount)` (EF:58228)

This is the **spell-XP grant** routine (NOT a chain-death walker). Full pseudocode:
```c
void sub_6D8B0(a1 /*owner/parent id*/, a2 /*spell/code index*/, a3 /*XP delta*/):
    if !(setting_38545 & 4):                 // XP-tracking enabled
        if a1:
            v3x = Entities[a1]
            if v3x->class == 3 && v3x->model == 0:      // owner is a human player (class 3, model 0)
                slot = v3x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x[a2]
                v3x->...spellsExperience[a2] = a3 + slot          // add XP to spell 'a2'
                if a2 == 2:
                    SetSpell_6D5E0(Entities[...SpellsEnabled[2]], ...array_0x437[2].SpellIndex[2])
                if MULTIPLAYER_MODE:
                    if a1 == local player index:
                        sub_6DAD0(&str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2)
                else:
                    sub_6D9C0(&str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2, 0, 1)
```
So: when a creature/spell entity dies or lands a kill, it credits the owning player's spell-experience for spell index `a2` by `a3`. `a2` is the **spell/monster-type code**, not a chain command.

Callers and their codes (EF grep):
| line | caller code (a2) | id source | meaning |
|---|---|---|---|
| 10861 | 0x13 | parentId | class-9/10 projectile lock-on granted |
| 10998 | 0x18 | parentId | projectile re-target |
| 23395 | 0x12 | id | (spell effect) |
| 23521/23525 | 0x10/0x11 | id | spell hits |
| 23871 | 9 | id | |
| 24088 | v15 | local player | generic |
| 24407/24444 | 0x15 | id | |
| 24802 | 7 | id | |
| 26636/26646/26663 | 0x16 | parentId | mana-sphere spell kill credit (`sub_36680`) |
| 29374 | 0x14 | id | |
| 29437 | 0x14 | v6 | |
| 29580 | 0xF | id | |
| 29979 | 0x17 | id (`v33x`) | |
| 55273 | 8 | a2x index | |
| 56243 | 3 | parentId | |
| 56321 | 4 | parentId | |
| 56453 | 5 | parentId | |
| 56909 | 0xA | parentId | |
| 57085 | 0xB | parentId | |
| 57146 | 0xC | v1 | |
| 58411/58826 | 7 | id | |
| 59052 | 1 | id | |
| 60657 | 0xE | word_0x7A_122 | |
| 60678 | 6 | self | |
| 61596 | 2 | self | |
| 62123 | 0xD | v1 | |
| 62985 | `Entities[word_0x26_38]->model` | id | killer's model → grants XP for that monster type |
| 63189 | 0 | id | |
| 63314 | 1 | id | |
| 63551 | `*(char*)(Entities[word_0x26_38]+64)` (= model_0x40) | id | kill-by-model credit |

**OPEN / correction to brief:** The context's premise "Death propagates via parentId_0x28_40 + sub_6D8B0(parentId, code, 1)" is only partially correct — `sub_6D8B0` is a per-spell XP crediting function keyed by the killed thing's type code, called with `parentId` (owner) as the recipient. It does **not** walk the segment chain. The chain destruction itself is done by `sub_26CC0` (m22, §4), `sub_2AE80` (m27 cleanup, §1.27), and `sub_298D0` (m27 body death, §6). The code `a2` at lines 62985/63551 equals the killed entity's model, which for a segmented monster would be 22 or 27 — that's how a multipart kill grants type-specific XP.

---

## 6. The 8 model-specific head/state handlers (per model)

Note: state→handler is via a data table; I verified every handler BODY. Base state = model×8.

### Model 0 (states 0x00–0x07): handlers 1EF20,1EF40,1EF70,1EFD0,1F000,1F020,1F2B0,1F300

- **0x00 `sub_1EF20`** (EF:11189): thin wrapper → `sub_1BD90(a1x, 0)` (patrol primitive, arg0=aggro-code 0).
- **0x01 `sub_1EF40`** (EF:11195): `sub_1BF90(a2x,0)` (idle) + `sub_1F0C0(a2x)` + `sub_1F040(a2x)`.
- **0x02 `sub_1EF70`** (EF:11203): `if sub_1C310(a2x,0,sub_1CC20)` (chase-attack; callback sub_1CC20) → `PrepareEventSound_6E450(self,-1,8)` (SOUND 8 = attack); then `sub_1F0C0` + `sub_1F040`.
- **0x03 `sub_1EFD0`** (EF:11213): `sub_1C560(a2x,0)` (pack) + `sub_1F0C0` + `sub_1F040`.
- **0x04 `sub_1F000`** (EF:11221): → `PreKillEntity_1C890(a1x, 0)`.
- **0x05 `sub_1F020`** (EF:11227): → `KillEntity_1C930(a1x)`.
- **0x06 `sub_1F2B0`** (not shown; addr 0x1F2B0 — flee/spawn variant; OPEN: body not read, but from context it is the flee primitive `sub_1C980`-based).
- **0x07 `sub_1F300`** (EF:11352): `sub_1D5D0(a2x,0)` (spawn/appear) then by `StageVar2_0x49_73`: cases 1-0xA/0xD/0xE/0x10 → `sub_1F0C0`+`sub_1F040`; case 0x11 → `sub_1F040`.

`sub_1F0C0` (EF:11260) — model-0/3 **ATTACK/tether behavior**: manages a hooking/lasso mechanic — decrements `fontTypeIndex_0x3D_61`; if `byte_0x46_70` (hooked target set) and `word_0x2C_44>0`, orbits target `word_0x24_36` by `MoveEntity(..., 48*word_0x2C_44)`; else scans nearby map tiles for a **class-9** entity whose `word_0x96_150 == self.id` (its own projectile) and hooks it (`byte_0x46_70++`, `word_0x2C_44=5`). `sub_1F040` (EF:11233): vertical bob — `z += dword_0x10_16; dword_0x10_16 -= 5`; bounces at terrain+256 (and cave ceiling −256 → `dword_0x10_16 = -150`; floor → `= 150`).

Sounds m0: 8 (attack, EF:11206).

### Model 3 (states 0x18–0x1F): 1F950,1F970,1F990,1F9E0,1FA00,1FA20,(1FA40?),1FA50

- **0x18 `sub_1F950`** (EF:11581): → `sub_1BD90(a1x, 24)`.
- **0x19 `sub_1F970`** (EF:11587): → `sub_1BF90(a1x, 24)`.
- **0x1A `sub_1F990`** (EF:11593): `if sub_1C310(a1x,24,sub_1CC20)` → `PrepareEventSound_6E450(self,-1,8)` (SOUND 8).
- **0x1B `sub_1F9E0`** (EF:11601): → `sub_1C560(a1x,24)`.
- **0x1C `sub_1FA00`** (EF:11607): → `PreKillEntity_1C890(a1x, 24)`.
- **0x1D `sub_1FA20`** (EF:11613): → `KillEntity_1C930(a1x)`.
- **0x1E — OPEN:** context lists `1FA40`, but **no `sub_1FA40` exists** (verified: EF:11616 jumps 0x1FA20→0x1FA50 with no function between). The m3 chase-attack handler is `sub_1F990` (0x1F990). The 8th slot's true address is unresolved from the source — FLAG OPEN. Likely the same `sub_1F990` or an alias; the aggro-code base is 24 for all m3 handlers.
- **0x1F `sub_1FA50`** (EF:11619): → `sub_1D5D0(a1x, 24)`.

All m3 head states are thin wrappers over shared primitives with aggro-code **24** (= model 3 × 8). Sounds m3: 8 (attack). No class-9/10 spawns in the head states themselves; attacks route through `sub_1C310`/`sub_1CC20`.

### Model 22 (states 0xB0–0xB7): 26960,26990,26AA0,26BD0,26CA0,26CC0,27920(→27720),27930

- **0xB0 `sub_26960`** (EF:17247): `sub_26FF0(a1x)`(move+alt), `sub_272C0`(anim/spin), `sub_26F10`(damage-accel+death→181), `sub_27880`(tail-grow timer).
- **0xB1 `sub_26990`** (EF:17255): `sub_26FF0` + `sub_272C0` then a **castle-mana-drain attack** scan: iterates `dword_0x10_16` hi/lo, `sub_27470` finds a segment with matching offset, colors it via `sub_278F0`→`sub_49D50`; if none found sets actionIndex to 178 (with target) or 176 (idle).
- **0xB2 `sub_26AA0`** (EF:17313): `sub_26FF0`+`sub_272C0`+`sub_26F10`+`sub_27880`; then (every 0x1F frames) targets a player CASTLE (`class==3`, `CastleEntityIndex_0x3A_58`): if within 0x100 dist and its mana < maxMana it begins draining → `dword_0x10_16 = 128; actionIndex = 179`; else reverts to 177 (chase player).
- **0xB3 `sub_26BD0`** (EF:17373): `sub_272C0`; countdown `dword_0x10_16`; when 0 and `!(byte_0x3E_62 & 1)`: if `byte_0x46_70>1` shrinks tail via `sub_27720(a1x, byte_0x46_70-2)`; else deposits accumulated mana into the target castle (`mana += seg.mana`, capped at castle maxMana) and `DisableEntityDrawing04` (self-destructs the drainer). This is the mana-theft payoff.
- **0xB4 `sub_26CA0`** (EF:17420): TAIL-SEGMENT TICK = `sub_271D0`(spiral follow) + `sub_26D20`(hit relay to head). See §4.
- **0xB5 `sub_26CC0`** (EF:17427): CHAIN-KILL walker (mana-sphere conversion of whole chain). See §4.
- **0xB6 (0x27920):** address 0x27920 falls **inside `sub_27720`** (EF:17938, spans 0x27720–0x27880) — the tail add/remove routine. **OPEN:** "27920" as a standalone handler is a mislabel; it is the interior of `sub_27720(a1x, len)` which grows/shrinks the tail by spawning (`NewEvent`+`sub_274C0` topology) or hiding pairs of segments and rebalancing `byte_0x46_70`, then `sub_27590`/`sub_27610` recolor.
- **0xB7 `sub_27930`** (EF:18046): → `sub_1D5D0(event, 176)` (spawn/appear, base 176).

`sub_26FF0` (EF:17589) — m22 head move+altitude: decays `actSpeed`; `SetEntityShiftRot`; `sub_1B8C0`(move core); every 16th frame `sub_27120`(anti-stack z-push vs other model-22); computes max terrain alt across the whole `word_0x34_52` chain +384 and clamps head z (rises `+0x100`/`+0x40`, sinks `-2`). `sub_272C0` (EF:17720): animation frame stepping — if `byte_0x46_70>=11` plays SOUND 48 when frame in (0,0x10); spins `subSpellIndex_0x2A_42 += word_0x2C_44`, jitters `word_0x2C_44`. `sub_27880` (EF:18012): every `word_0x96_150==0` cycle, grows tail via `sub_27720(len+2)` if len≤13 and regens mana (+1000, cap 50000).

Sounds m22: 4 (aggro/target, EF:17515/17524/17577), 48 (anim/thrash, EF:17737), 42 (EF:17075, in `sub_265A0`).

### Model 27 (states 0xD8–0xDF): 29400,29670,29710,29890,298B0,298D0,29920(→?),29930

- **0xD8 `sub_29400`** (EF:19443): BODY emerge/attack sequencer on `dword_0x10_16++` phases: phase0 `sub_2AED0(a1x,337)`+`life=1000000`; phase9 teleports the whole tree to a valid random tile (RNG ×2 for offset, up to 128 probe steps of 768) + SOUND 22 (EF:19504); phase 0xF/12/6/3 set draw-flag groups; phase18 → actionIndex 218, `byte_0x46_70=1`; finally `sub_29A90(a1x)` (drives all branches/segments).
- **0xD9 `sub_29670`** (EF:19556): BODY main brain. `sub_2A6B0`(consume hit → 0/1/2; on 1 sets life=1000000, target=word_0x26_38); `sub_2AF10(a1x,1)` (m27 move core → 3=stuck-rotate, 4=blocked→actionIndex 216); every 0x3F frames RNG re-roll of `roll_0x20_32`; `sub_29A90`.
- **0xDA `sub_29710`** (EF:19600): BODY chase-target brain. Similar to 0xD9 but with target `word_0x96_150`: computes `sub_581E0` angle, if within FOV `word_160_0x1e_30` sets `byte_0x46_70=1`+`sub_2AED0(337)` (attack pose) else `byte_0x46_70=0`+`sub_2AED0(315)`; if beyond range `word_160_0x1c_28` → actionIndex 217 (return); `sub_29A90`.
- **0xDB `sub_29890`** (EF:19665): `actionIndex = 217; sub_29670(a1x)` (return-then-idle).
- **0xDC `sub_298B0`** (EF:19672): `life_0x8 = -1; PreKillEntity_1C890(a1x, 216)`.
- **0xDD `sub_298D0`** (EF:19679): BODY DEATH — `life=-1`; `TransformEntityToManaSphere_36BA0(a1x, true)`; if not flag 0x10, spawns a **class-10 subtype-1** mana sphere (`IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 1)`) inheriting id; then `sub_2AE80(a1x)` hides the entire chain; returns 1.
- **0xDE (0x29920):** **OPEN:** no `sub_29920` exists — grep shows 0x298D0 followed directly by 0x29930. 0x29920 is the tail of `sub_298D0`. Mislabel in brief; the 7th m27 handler slot has no distinct function.
- **0xDF `sub_29930`** (EF:19696): BODY spawn/appear — `sub_1D5D0(a1x,216)`; sets attack/idle pose by `StageVar2`; `life=1000000`; on actionIndex −38 (0xDA) walks chain setting branch `byte_0x46_70 2` + head target; on −40 sets `StageVar2=15`; `sub_29A90`.

`sub_2A6B0` (EF:20424): consume `str_0x5E_94.word_0x62_98` hit — returns 0 (none), 1 (branch hit, `byte_0x3B_59≠0`), or 2 (body direct hit → actionIndex 220). `sub_2AF10` (EF:20870): m27 ground-move core → return code 1/2/3/4 (4=fully blocked).

m27 branch/segment sub-state machine (`sub_29A90`, EF:19737, driving 0xE9/0xEA): per branch (`actionIndex==233`), 16-way `byte_0x46_70` state machine — spawns **class-9 subtype-0/9 projectiles** via `sub_2A7F0`(EF:20507): `sub_2A7F0` RNG-picks `manaRegen_0x88_136` (1 or 2), then creates a class-9 (model 9) mana sphere with `subSpellIndex_0x2A_42 = 850`, plays SOUND 15 or 23; segment melee/whip damage via `sub_2A660` (EF:20395): relays segment hit to body, caps applied damage at **76** (`if v4 > 76: v4 = 76`) then `body.life -= v4`, and if segment life<0 sets `byte_0x46_70 = 6` (death sub-state). Segment positions: `sub_2A5B0`/`sub_2A9F0`/`sub_2AA90` (9-per-branch spline via `str_D404C[branchIdx]` table + `xx_DWORD_D40BC` offsets).

Sounds m27: 22 (teleport, EF:19504), 17 (branch strike/whip, EF:19987/20202), 15 & 23 (projectile spawn low/high, EF:20537/20549 → id in `PrepareEventSound` EF:20552), 37 (EF:19080), 59 (EF:18225/18310), 7 (EF:18682), 62 (EF:19255) [latter four are in adjacent m27 sub-handlers `sub_27950`+].

---

## 7. Collision skip (EF:24486) and list-rebuild skip (EF:39969-39998)

### Collision filter `sub_33810(a1x, a2x)` (EF:24452)
Switches on `a2x->class_0x3F_63 - 2`. **Case 3u = class 5 (creatures)** (EF:24484-98):
```c
case 3u:
    if a2x->actionIndex_0x45_69 == 232 || a2x->actionIndex_0x45_69 == 180:
        return 0;                          // 0xE8 (m0/m3 child) and 0xB4 (m22 tail seg) → NO collision
    v5 = a2x->model_0x40_64
    if v5 < 0xF:  if v5==10 result=0
    else if v5<=0xF || (v5>=0x12 && (v5<=0x12 || (v5>=0x1B && v5<=0x1C))): return 0
    return result   // (1)
```
So class-5 entities in **state 0xE8 (232) or 0xB4 (180) are collision-transparent**. (m27's 0xEA/0xE9 are not in this list — but they are class 5 with model 27; model 27 is not in the excluded model set here, so branch/segment collision falls through to the model check. **OPEN:** m27 segments aren't explicitly excluded by actionIndex in `sub_33810`; they rely on the list-rebuild skip to never be enumerated as collision targets.)

### List rebuild (`sub_...` at EF:39929+, per-frame entity bucketing)
For `case 0x5` (class 5) (EF:39987-40009):
```c
if jx->life_0x8 < 0: continue                       // dead → not listed
if jx->actionIndex_0x45_69 < 0xE8:
    v8 = (actionIndex == 0xB4)                        // 0xB4 flagged for skip
else:
    if actionIndex == 0xE8: continue                  // 0xE8 → NEVER added to per-model list
    v8 = (actionIndex == 0xEA)                         // 0xEA flagged for skip
if !v8:                                               // only if NOT 0xB4 and NOT 0xEA (and passed 0xE8 continue)
    add jx to per-model bucket bytearray_38403x[model]  (linked via next_0)
continue
```
Result: **0xE8 (child), 0xB4 (m22 tail seg), 0xEA (m27 tier-2 seg)** are all excluded from the per-model AI/collision iteration list. **0xE9 (m27 branch) IS added** (not in skip set) — branches appear in the model-27 bucket (used by `sub_2A6F0` target scans). Class-3 (players, EF:39975) and class-9 (EF:40010) get their own buckets.

---

## 8. Sound ids (complete, subsystem-wide)

| id | where | context |
|---|---|---|
| 4 | EF:17515,17524,17577 (m22 sub_26D20/sub_26F10) | tail→head target-acquire |
| 7 | EF:18682 (m27 adjacent) | segment event |
| 8 | EF:11206 (m0 0x02), 11596 (m3 0x1A) | melee/chase attack hit |
| 12+ (12/13) | EF:11483,11524 (m0 goat-variant `sub_1F660/sub_1F6D0`) | `(rand&1)+12` |
| 15 | EF:20537→20552 (m27 sub_2A7F0) | class-9 projectile spawn (low) |
| 17 | EF:19987 (m27 sub_29A90 case5/6), 20202 (sub_29A90 tail strike) | branch whip/strike |
| 22 | EF:19504 (m27 sub_29400 phase9) | body teleport |
| 23 | EF:20549→20552 (m27 sub_2A7F0) | class-9 projectile spawn (high) |
| 37 | EF:19080 (m27 region) | |
| 42 | EF:17075 (m22 `sub_265A0` case9) | `(rand%0xB)==0` ambient |
| 46 | EF:11391,11403,11423,11448,11457 (m0 goat variants) | `(rand%0x4D)==0` / `(rand%0x2B)==0` ambient |
| 48 | EF:17737 (m22 sub_272C0) | thrash/anim (frame in 0..0x10) |
| 59 | EF:18225,18310 (m27 adjacent handlers) | |
| 62 | EF:19255 (m27 adjacent) | |

(m0/m3 CTORs and the m22/m27 CTORs emit NO sounds.)

---

## OPEN / uncertainties

1. **m0 state 0x06 `sub_1F2B0`** — body not read in this pass (flee/spawn variant, aggro-code 0). Reads as `sub_1C980`-family per context; verify separately.
2. **m3 8th handler "1FA40"** — no such function exists (EF:11616 gap). The m3 chase-attack is `sub_1F990`. Slot 0x1E's true handler unresolved from source; likely a table alias to `sub_1F990` or an unused slot. FLAG OPEN.
3. **m22 "27920"** — not a function; interior of `sub_27720` (tail grow/shrink). The real m22 states are 0xB0–0xB7 = `sub_26960/26990/26AA0/26BD0/26CA0/26CC0/…/27930`; the 7th slot maps into `sub_27720`.
4. **m27 "29920"** — not a function; tail of `sub_298D0`. The distinct m27 handlers are `sub_29400/29670/29710/29890/298B0/298D0/29930` (7 distinct for 8 states 0xD8–0xDF; one slot has no unique body).
5. **`sub_6D8B0`** is a spell-XP grant, NOT a chain-death walker (brief's framing corrected in §5). Chain destruction is `sub_26CC0` (m22), `sub_2AE80`/`sub_298D0` (m27), and parent-loss auto-hide in `sub_1B6B0` (m0/m3).
6. **State→handler-address table** is a binary data table (indexed `actionIndex*14`, base off_D697E) not present as readable source; all handler *bodies* were confirmed via the address-keyed switch in Events.cpp (e.g. EV:1021 0x1fc6b0→sub_1B6B0, EV:1864 0x207ca0→sub_26CA0). The actionIndex→address correspondence relies on the brief's roster plus CTOR-assigned `actionIndex` values, which I cross-checked (m0=1, m3=25, m22 head=176, m22 seg=180, m27 body=0xD9, m27 branch=0xE9, m27 seg=0xEA, m0/m3 child=0xE8).
7. **m27 collision** — segments (0xEA) not explicitly excluded in `sub_33810`; they avoid collision only by absence from the rebuilt model list (§7). Branches (0xE9) ARE listed and collidable.