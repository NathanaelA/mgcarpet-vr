I have enough to write the report. Let me verify a couple of remaining items: SetHalfSpeedEntity_49DA0, SetEntityShiftRot_49EA0, AddEventToMap_57D70 signatures, and check the sub_59C40 (getTerrtoZ) which is a no-op-ish for models 8. Also confirm CopyMaxLifeToLife and the byte_0x46_70/f71 meaning. Let me check the model→action for the plain ctors 16/17. The context said models 16/17 have their own plain ctors at :37137/:37143 — but I read those and they just call AddSwitchXX_50A90(pos,16,16) and (pos,17,17). So the context's claim is slightly off. Let me verify there aren't different ctors. Also I should double check EndLoop_6EAB0 vs PrepareEventSound.

I have everything needed. Let me compile the report.

# Class-11 Switches + Class-14 Special Map Objects — remc2 trace

All cites are `EventsFunctions.cpp` (EF) or `Events.cpp` (EV) under `reference/remc2/remc2/engine/`.

---

## 0. Correction to CONTEXT premise (models 16/17 ctors)

The CONTEXT states "Models 16/17 have their own plain ctors (:37137/:37143)." **This is not what the code shows.** `AddSwitch16_50C90` (EF:37137-37140) and `AddSwitch17_50CB0` (EF:37143-37146) are ordinary thin wrappers identical in form to every other one — they just call `AddSwitchXX_50A90(position, 16, 16)` / `(position, 17, 17)`. There is no distinct ctor body for 16/17. **OPEN:** if the CONTEXT meant something special about models 16/17, it is not in these ctors. (Model 17's *action* is the scroll pickup `AddSwitch0B_11_6F4A0` — see §3.)

---

## 1. Class-11 creator `AddSwitchXX_50A90` (EF:37059-37074) — verbatim field writes

```c
type_entity_0x6E8E* AddSwitchXX_50A90(axis_3d* position, char a2, char a3) {   // 231a90
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->class_0x3F_63        = 0xB;                 // class 11
        event->model_0x40_64        = a2;                  // par1 -> model
        event->actionIndex_0x45_69  = a3;                  // par2 -> action index
        event->struct_byte_0xc_12_15.byte[0] &= 0xF6u;     // clear bits 0,3 (mask ~0x09)
        event->dword_0x10_16        = 0;                   // countdown/state = 0
        event->struct_byte_0xc_12_15.byte[0] |= 1;         // set bit0 (draw/active flag)
        event->position_0x4C_76     = *position;           // copy full 3D pos
        CopyMaxLifeToLife_49A20(event);                    // life = maxLife
    }
    return event;
}
```

Mapping / notes:
- **par1 (`a2`) → `model_0x40_64`**, **par2 (`a3`) → `actionIndex_0x45_69`**. In every wrapper the two are **equal** (model N always paired with action N): `AddSwitch00`→(0,0) … `AddSwitch32`→(32,32), etc. (EF:37077-37311). So the per-model difference is **only the model/action number** — there are **no per-model extents, id24, stageTag, or f71 writes** in this ctor. Those are all left at whatever `NewEvent_4A050` initialized.
- `struct_byte_0xc_12_15.byte[0]`: cleared with `&=0xF6` (clears bits 0 and 3), then `|=1` (bit0 set) → net effect bit0=1, bit3=0, others preserved. Bit0 is the "drawing enabled / active" flag (cleared later by `DisableEntityDrawing04_57F10` and by the X-marker draw-flag clears; see §3/§4).
- `dword_0x10_16 = 0` is the **countdown/phase counter** used by `sub_6F300` (see §2) and by `InitSwitchChainZaxisAndSound_6F850` fallthrough.
- `CopyMaxLifeToLife_49A20` copies `maxLife`→`life` (**OPEN:** not read in full, name-inferred; standard init helper).

**Wrapper enumeration** (all EF): `AddSwitch00_50AE0`(0,0):37077 · `01_50B00`(1,1):37083 · `02_50B20`(2,2):37089 · `03_50B40`(3,3):37095 · `04_50B60`(4,4):37101 · `32_50B80`(32,32):37107 · `12_50C10`(12,12):37113 · `13_50C30`(13,13):37119 · `14_50C50`(14,14):37125 · `15_50C70`(15,15):37131 · `16_50C90`(16,16):37137 · `17_50CB0`(17,17):37143 · `18_50CD0`(18,18):37149 · `19_50CF0`(19,19):37155 · `20_50D10`(20,20):37161 · `21_50D30`(21,21):37167 · `22_50D50`(22,22):37173 … continuing through 0x2C (contiguous in the file to :37311). Model 0x1E(30) ctor and model 0x1F(31) ctor are the two specials:
- **Model 31 ctor** (EF:37299-37312, addr `002321F0`… actually the `sub` just before `sub_514E0`): guarded — only creates the switch if **NOT** multiplayer (`setting_byte1_22 & Setting::MULTIPLAYER_MODE`) **and** `(setting_38545 & 8)==0`; on success sets `byte_0x36E02=1` and `byte_0x36E0B |= 1` (EF:37302-37309). This is the level-end objective flag.

The **creator dispatch table** is `x_DWORD_D4C52ar_strB1[46]` (EF:1846-1892): model→ctor-address. Enumerated indirectly via the class table `str_D4C48ar[11] = {strB0, strB1}` (EF:2072). The EV creator switch (EV:4864-5017 range) dispatches on these addresses.

---

## 2. `sub_6F300` (EF:54457-54505) — FULL slot-condition core

```c
unsigned int sub_6F300(type_entity_0x6E8E* a1x, signed int a2) {   // 250300
    unsigned int result = a2;
    int v3;
    if (a2 == -1) {                                   // ANY-SLOT variant (model 0x1E)
        for (result = 0; (signed int)result <= 16; ++result) {
            if ((result <= 0xB || (result >= 0x10 && result <= 0x1C))
                 && x_D41A0_BYTEARRAY_4_struct.bytearray_38403x[result])
                return result;                        // some watched slot still OCCUPIED -> wait
        }
        v3 = a1x->dword_0x10_16;
        if (!v3) { a1x->dword_0x10_16 = 16; return result; }   // arm countdown = 16
        if (v3 == 1) {                                          // countdown expired -> FIRE
            PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 41);
            sub_4A1E0(a1x->id_0x1A_26, 1);
            DisableEntityDrawing04_57F10(a1x);
            return 1;
        }
    } else {                                          // SINGLE-SLOT variant
        if (x_D41A0_BYTEARRAY_4_struct.bytearray_38403x[a2])
            return result;                            // slot a2 OCCUPIED -> wait
        v3 = a1x->dword_0x10_16;
        if (!v3) { a1x->dword_0x10_16 = 16; return result; }   // arm countdown = 16
        if (v3 == 1) {                                          // countdown expired -> FIRE
            PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 41);
            sub_4A1E0(a1x->id_0x1A_26, 1);
            DisableEntityDrawing04_57F10(a1x);
            return 1;
        }
    }
    result = v3 - 1;                                  // otherwise decrement countdown
    a1x->dword_0x10_16 = v3 - 1;
    return result;
}
```

### `bytearray_38403x` — what it is / who fills it
`x_D41A0_BYTEARRAY_4_struct.bytearray_38403x[model]` is the **per-class-5-model live-entity list head** — an array of head pointers (`type_entity_0x6E8E*`), one per class-5 model number. It is **rebuilt every frame** in the big list-rebuild loop at EF:39969-40009: iterating all entities `Entities_EA3E4[1..999]`, for `class_0x3F_63 == 5` (case `0x5`, EF:39987-40009):
- skips dead (`life_0x8 < 0`), and skips certain reserved action indices: `actionIndex==0xB4`, `==0xE8`, `==0xEA` are excluded (the `v8` test EF:39990-39999);
- otherwise **links the entity into the singly-linked list keyed by its `model_0x40_64`** (EF:40002-40007): `bytearray_38403x[model]` = list head, `next_0` chaining via `v22x[model]` tail tracker.

So `bytearray_38403x[slot] != 0` ⟺ **at least one live class-5 entity of model==slot currently exists**. The switch's "slot" is therefore a class-5 model number it is watching. **CONFIRMED** it is the per-class-5-model live-list head (built at EF:39969-40009, class-5 branch 39987+).

### Semantics
- **Occupied test:** `bytearray_38403x[slot] != NULL` → return immediately, do nothing (wait).
- **Countdown field:** `dword_0x10_16`, **start value 16** (EF:54473, 54491). When the watched slot(s) become empty, first empty frame arms it to 16; each subsequent empty frame decrements (EF:54502-54503) until it reaches 1.
- **Chain-fire ("fire"):** at `dword_0x10_16 == 1` (EF:54494-54500 / 54476-54482): plays **sound 41** via `PrepareEventSound_6E450(entityIndex, -1, 41)`, then `sub_4A1E0(a1x->id_0x1A_26, 1)`. **`sub_4A1E0(id, activate)`** (EF:32950) is the **terrain-object disposition activation** by DisId: it clears the objective struct if id==0, then walks `D41A0_0.terrain_2FECE.entity_0x30311[1..0x4AF]` and for every terrain entity whose `.DisId == id` calls `sub_4A310` to spawn/activate it; with `a2!=0` (here `1`) it **removes** the terrain slot after activating (`.type_0x30311 = 0`, EF:32990-32991). The id used is **`a1x->id_0x1A_26`** (the switch's own disposition id). So "fire" = **activate all terrain dispositions tagged with this switch's id**. Then `DisableEntityDrawing04_57F10(a1x)` despawns/hides the switch.
- **Despawn:** `DisableEntityDrawing04_57F10` (EF:8706 et al., name-inferred: clears draw + marks for removal).
- **Any-slot variant (`a2==-1`):** watches every slot in `0..0xB` and `0x10..0x1C` (EF:54467); fires only when **all** those slots are empty.

**Sound: id 41 (0x29).** Same sound also emitted by `InitSwitchChainZaxisAndSound_6F850` (EF:44539) and by `sub_6F850`'s use for model>3.

---

## 3. Each model 0x05–0x2C: dispatcher case, slot passed, and specials

### Model → action-address, from `x_DWORD_D4C52ar_strB0[46]` (EF:1798-1844)
Format rows are `{tag, model, action_addr, 1}`. The EV **action** switch (keyed on address) is at EV:4009-4144. Mapping:

| Model (hex/dec) | action addr | handler | slot passed to `sub_6F300` |
|---|---|---|---|
| 0x00–0x04 | 250030..250150 | (ALREADY PORTED: proximity models 0-3, level-end 4) | — |
| 0x05 | 250240 | AddSwitch0B..(porting range start) EV | — (own ctor path; **OPEN**, addr 0x250240 not among the 6F3xx set — it is `sub_6F240`, a separate handler not in scope's sub_6F300 family) |
| 0x06–0x0B | 250250..2502A0 | `sub_6F250..6F2A0` (separate, not sub_6F300) | — **OPEN** (these 6 are NOT sub_6F300 wrappers) |
| **0x0C (12)** | **2502B0** | **`sub_6F2B0`** (EV:4009) | — X-MARKER (see below) |
| 0x0D (13) | 250420 | `sub_6F420` (EV:4017) | **0** |
| 0x0E (14) | 250440 | `sub_6F440` (EV:4021) | **1** |
| 0x0F (15) | 250460 | `sub_6F460` (EV:4026) | **2** |
| 0x10 (16) | 250480 | `sub_6F480` (EV:4030) | **3** |
| **0x11 (17)** | **2504A0** | **`AddSwitch0B_11_6F4A0`** (EV:4034, `//get scroll4`) | **4** |
| 0x12 (18) | 2504C0 | `sub_6F4C0` (EV:4038) | **5** |
| 0x13 (19) | 2504E0 | `sub_6F4E0` (EV:4042) | **6** |
| 0x14 (20) | 250500 | `sub_6F500` (EV:4046) | **7** |
| 0x15 (21) | 250520 | `sub_6F520` (EV:4050) | **8** |
| 0x16 (22) | 250540 | `sub_6F540` (EV:4054) | **9** |
| 0x17 (23) | 250560 | `sub_6F560` (EV:4058) | **0xA (10)** |
| 0x18 (24) | 250580 | `sub_6F580` (EV:4062) | **0xB (11)** |
| 0x19 (25) | 2505A0 | `sub_6F5A0` (EV:4067) | **0xC (12)** |
| 0x1A (26) | 2505C0 | `sub_6F5C0` (EV:4071) | **0xD (13)** |
| 0x1B (27) | 2505E0 | `sub_6F5E0` (EV:4075) | **0xE (14)** |
| 0x1C (28) | 250600 | `sub_6F600` (EV:4079) | **0xF (15)** |
| 0x1D (29) | 250620 | `sub_6F620` (EV:4083) | **0x10 (16)** |
| **0x1E (30)** | **2507C0** | **`sub_6F7C0`** (EV:4139) | **-1 (ANY-SLOT)** |
| **0x1F (31)** | **2507E0** | **`sub_6F7E0`** (EV:4143) | — X-MARKER/level-end (see below) |
| 0x20 (32) | 2501C0 | `AddSwitch0B_20_6F1C0` (EV:4005) | — (stage-gated, ALREADY PORTED) |
| 0x21 (33) | 250640 | `sub_6F640` (EV:4089) | **0x11 (17)** |
| 0x22 (34) | 250660 | `sub_6F660` (EV:4093) | **0x12 (18)** |
| 0x23 (35) | 250680 | `sub_6F680` (EV:4097) | **0x13 (19)** |
| 0x24 (36) | 2506A0 | `sub_6F6A0` (EV:4101) | **0x14 (20)** |
| 0x25 (37) | 2506C0 | `sub_6F6C0` (EV:4105) | **0x15 (21)** |
| 0x26 (38) | 2506E0 | `sub_6F6E0` (EV:4109) | **0x16 (22)** |
| 0x27 (39) | 250700 | `sub_6F700` (EV:4113) | **0x17 (23)** |
| 0x28 (40) | 250720 | `sub_6F720` (EV:4117) | **0x18 (24)** |
| 0x29 (41) | 250740 | `sub_6F740` (EV:4121) | **0x19 (25)** |
| 0x2A (42) | 250760 | `sub_6F760` (EV:4125) | **0x1A (26)** |
| 0x2B (43) | 250780 | `sub_6F780` (EV:4130) | **0x1B (27)** |
| 0x2C (44) | 2507A0 | `sub_6F7A0` (EV:4134) | **0x1C (28)** |

Each wrapper body (EF:54510-54687) is a one-liner `return sub_6F300(a1, <slot>)`. Full slot table above; wrapper defs: `sub_6F420`→0 (:54510), `6F440`→1, `6F460`→2, `6F480`→3, `6F4A0`→4, `6F4C0`→5, `6F4E0`→6, `6F500`→7, `6F520`→8, `6F540`→9, `6F560`→0xA, `6F580`→0xB, `6F5A0`→0xC, `6F5C0`→0xD, `6F5E0`→0xE, `6F600`→0xF, `6F620`→0x10, `6F640`→0x11, `6F660`→0x12, `6F680`→0x13, `6F6A0`→0x14, `6F6C0`→0x15, `6F6E0`→0x16, `6F700`→0x17, `6F720`→0x18, `6F740`→0x19, `6F760`→0x1A, `6F780`→0x1B, `6F7A0`→0x1C (:54677-54681), `6F7C0`→ -1 (:54684-54687).

> **Note on "slot" semantics:** slots are contiguous class-5 model numbers 0..28. Model 0x0D(13) watches class-5-model 0; there is a fixed **offset of 13** between switch model and watched slot for the 0x0D-0x1C run (switch 0x0D→slot0 … switch 0x1C→slot0xF), then continuing 0x1D→0x10, and the 0x21-0x2C block continues 0x21→0x11 … 0x2C→0x1C. (Model 0x11's slot 4 is the "scroll4" pickup — see note below.)

### Model 0x0C (12) — `sub_6F2B0` (EF:54431-54452) — X-MARKER single-fire

```c
void sub_6F2B0(type_entity_0x6E8E* a1x) {   // 2502b0
    type_entity_0x6E8E* result = InitSwitchChainZaxisAndSound_6F850(a1x, 1);
    if (result) {
        result->actionIndex_0x45_69 = 12;
        result->byte_0x46_70 = 0;
        DisableEntityDrawing04_57F10(a1x);          // despawn the marker
        if (D41A0_0.word_0x36DFE) {                 // linked class-14 model-3 object
            Entities_EA3E4[D41A0_0.word_0x36DFE]->struct_byte_0xc_12_15.byte[0] &= 0xFEu;  // clear draw bit0
        }
    }
}
```
- Passes `a2=1` to `InitSwitchChainZaxisAndSound_6F850` → searches the class-3 list for a class-3 entity (`model==0`, i.e. a player/marker) at shift-distance `==1` (`CompareAxisWithShift_10750`, EF:44535). **This is the proximity test** — the marker fires when a player is adjacent.
- On hit: sets the **found chain target's** `actionIndex = 12`, `byte_0x46_70 = 0` (arms it as a checkpoint), despawns the X-marker, and **clears the draw flag (bit0) of the linked class-14 model-3 object pointed to by `word_0x36DFE`** (making the on-map "X" graphic disappear). **This ARMS A CHECKPOINT** — matches CONTEXT.

### Model 0x11 (17) — the "scroll4" pickup, `AddSwitch0B_11_6F4A0` (EF:54534-54537)
```c
unsigned int AddSwitch0B_11_6F4A0(type_entity_0x6E8E* a1) { return sub_6F300(a1, 4u); }
```
- **It is NOT a distinct pickup handler** — it is an ordinary slot-4 switch delegating to `sub_6F300`. The EV comment `//get scroll4` (EV:4033) and the name are descriptive: this switch waits until all live class-5-model-4 entities ("scroll4" objects) are gone, then chain-fires (sound 41, `sub_4A1E0(id,1)`, despawn). **It grants nothing itself, has no proximity radius, no distinct sound** beyond the shared sound 41 — the "pickup" behaviour lives in the class-14 scroll object (`UpdateScroll_59C80`, §4), which is what actually grants XP. **OPEN/CLARIFICATION:** the CONTEXT's framing of 0x11 as "the scroll pickup: what does it grant / radius / sound" is a mismatch; the granting object is the class-14 scroll (§4), and this class-11 switch is just the empty-list gate for it.

### Model 0x1E (30) — `sub_6F7C0` (EF:54684-54687)
`return sub_6F300(a1, 0xFFFFFFFF);` → the **any-slot** variant. Same fire mechanics (sound 41, `sub_4A1E0(id,1)`, despawn).

**CORRECTION 2026-07-18 (binary-verified):** the watched set is slots
**0..=0xB and 0x10 ONLY** — the scan loop's bound is `<= 16`
(`for (result = 0; result <= 16; ++result)`, and NETHERW.EXE @0x93BA6
= `cmp eax,0x10; jng`), so the condition's `0x10..0x1C` arm is dead
past slot 0x10. High models (0x11..=0x1C — manticores, hydra, etc.)
NEVER gate the any-slot switch. This is the same effective law as
MC1's -1 variant (buckets 0..=11 and 16). Load-bearing on level 024:
the authored wandering (5,27) hydra must not block the opening
gauntlet's (11,30) wall gates. The earlier reading here ("every
watched slot 0..0xB, 0x10..0x1C") transcribed the dead condition, not
the shipped loop.

### Model 0x1F (31) — `sub_6F7E0` (EF:54690-54705) — X-MARKER / level-end
```c
void sub_6F7E0(type_entity_0x6E8E* entity) {   // 2507e0
    if (x_D41A0_BYTEARRAY_4_struct.setting_38545 & 8)
        DisableEntityDrawing04_57F10(entity);
    type_entity_0x6E8E* entity2 = InitSwitchChainZaxisAndSound_6F850(entity, 1);
    if (entity2) {
        entity2->actionIndex_0x45_69 = 11;                 // target action = 11
        entity2->byte_0x46_70 = 0;
        DisableEntityDrawing04_57F10(entity);              // despawn
        if (D41A0_0.word_0x36DFC) {                        // linked class-14 model-4 object
            Entities_EA3E4[D41A0_0.word_0x36DFC]->struct_byte_0xc_12_15.byte[0] &= 0xFEu;   // clear draw bit0
        }
    }
}
```
- If `setting_38545 & 8` (level-end-suppressed flag), immediately hides itself.
- Otherwise proximity-chain via `6F850(entity,1)`; on hit sets chain target `actionIndex = 11` (level-end), `byte_0x46_70 = 0`, despawns, and clears the draw flag of the **class-14 model-4** object at `word_0x36DFC`.

### `InitSwitchChainZaxisAndSound_6F850` (EF:44523-44540) — the proximity/chain helper
```c
type_entity_0x6E8E* InitSwitchChainZaxisAndSound_6F850(type_entity_0x6E8E* event, int a2) {  // 250850
    if (event->byte_0x3E_62 & 7)                          // PHASE GATE: bits 0-2 of byte_0x3E
        return 0;
    for (ix = x_D41A0_BYTEARRAY_4_struct.dword_38519; ; ix = ix->next_0) {   // class-3 live list
        if (ix <= Entities_EA3E4[0]) {                    // end of list -> no hit
            event->position_0x4C_76.z = getTerrainAlt_10C40(&event->position_0x4C_76);
            return 0;
        }
        if (!ix->model_0x40_64 && CompareAxisWithShift_10750(event, ix) == a2)  // model0 & dist==a2
            break;
    }
    if (event->model_0x40_64 > 3u)
        PrepareEventSound_6E450(ix - D41A0_0.struct_0x6E8E, -1, 41);   // sound 41 for models >3
    return ix;
}
```
- **Phase gate `byte_0x3E_62 & 7`** (EF:44526): if any of low 3 bits set, do nothing this frame.
- Walks the **class-3 live list head `dword_38519`** (rebuilt at EF:39975-39986, class-3 branch) — these are the player/checkpoint entities.
- **Proximity = `CompareAxisWithShift_10750(event, ix) == a2`** (a2 passed as 1 from both X-markers) — a box/shift-distance comparison (**OPEN:** exact box size inside `CompareAxisWithShift_10750`, not read; name-inferred grid-cell shift comparison).
- Emits **sound 41** if `model > 3`. Fallthrough (no hit) sets the switch's z to terrain altitude.

### `word_0x36DFE` / `word_0x36DFC` — what they point at / who writes them
Both are **entity-array indices** (`event - D41A0_0.struct_0x6E8E`) into `Entities_EA3E4`/`struct_0x6E8E`, stored in the global `D41A0_0`:
- **`word_0x36DFE`** = index of the **class-14 model-3** special object; **written by `sub_51570`** (EF:37346): `sub_51570` creates a class-14 model-3 object via `sub_514E0(pos, 3, 8, 338)` and records its index. Read/cleared by X-marker model 0x0C (`sub_6F2B0`, EF:54444-54448) to hide the "X" graphic.
- **`word_0x36DFC`** = index of the **class-14 model-4** special object; **written by `sub_515C0`** (EF:37358): creates class-14 model-4 via `sub_514E0(pos, 4, 9, 339)`. Read/cleared by level-end marker model 0x1F (`sub_6F7E0`, EF:54700-54702).
- Both initialized to 0 at level start (`LevelInit.cpp:42-43`), alongside `byte_0x36E02=1`, `byte_0x36E0B &= 0xFC` (LevelInit.cpp:41,46). Persisted in save/load (Basic.cpp:3181-3182, 3374-3375; engine_support_converts.cpp:798-800).
- `byte_0x36E02` (temp objective flag) and `byte_0x36E0B` (bit0 read in GameUI.cpp:549,811; bit1 in PlayerInput.cpp:420) are set by the model-31 ctor (EF:37307-37308) and in the objective-update loop (EF:40911, 40975-41007).

---

## 4. Class-14 special map objects

### Creator `sub_514E0` (EF:37315-37329) — verbatim
```c
type_entity_0x6E8E* sub_514E0(axis_3d* position, char a2, char a3, __int16 a4) {  // 2324e0
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->class_0x3F_63       = 0xE;          // class 14
        event->byte_0x46_70        = 0;
        event->actionIndex_0x45_69 = a3;           // par2 -> action index
        event->model_0x40_64       = a2;           // par1 -> model
        AddEventToMap_57D70(event, position);      // insert into map cell grid
        CopyMaxLifeToLife_49A20(event);            // life = maxLife
        SetHalfSpeedEntity_49DA0(event, a4);       // a4 -> sprite/anim table id (half-speed)
    }
    return event;
}
```
- `a2`→model, `a3`→**actionIndex** (this is what selects the class-14 action from `strE0`), `a4`→passed to `SetHalfSpeedEntity_49DA0` (sets the sprite/animation resource id at half update rate — **OPEN:** exact field, name-inferred). Note class-14 does NOT clear/set the draw bit here (unlike class-11 ctor); draw bit is set by the specific sub-creators.

**Sub-creators** (each picks model, actionIndex, sprite-id):
- `sub_51530` (EF:37332-37338): `sub_514E0(pos, 0, 0, 77)`; then `SetEntityShiftRot_49EA0(event, 384, 384)`. Model 0, action 0 (no-op action).
- **`sub_51570` (EF:37341-37350) — model 3 → `word_0x36DFE`:** `sub_514E0(pos, 3, 8, 338)`; sets `D41A0_0.word_0x36DFE = event - struct_0x6E8E` (EF:37346) and `byte[0] |= 1` (draw on). **Action index 8** → per `strE0[8]` = `0x0023AC40` = `sub_59C40_getTerrtoZ` (the "X" checkpoint graphic; static, just snaps z to terrain).
- **`sub_515C0` (EF:37353-37361) — model 4 → `word_0x36DFC`:** `sub_514E0(pos, 4, 9, 339)`; sets `D41A0_0.word_0x36DFC = event - struct_0x6E8E` (EF:37358) and `byte[0] |= 1`. **Action index 9** → `strE0[9]` = `0x0023AC60` = `sub_59C60` (level-end graphic; snaps z to terrain).
- `sub_51610` (EF:37365-37375): `sub_514E0(pos, 5, 10, 280)`; `SetEntityShiftRot_49EA0(event,768,1280)`; if `setting_38545 & 4` sets draw bit0. **Action 10** → `strE0[10]` = `0x0023AC80` = `UpdateScroll_59C80` (the pickup scroll).
- `sub_51660` (EF:37378-37394): model 1, **action 6** (`sub_59F60`); zeroes maxLife/life/subSpellIndex; clears draw bits (`&=0xF6`) then `|=1`.
- `sub_516C0` (EF:37397-37418): only if `isCaveLevel_D41B6`; model 2, **action 7** (`sub_5B100`); zeroes life/subSpellIndex/word_0x2C/word_0x96.

### Class-14 action dispatch: `x_DWORD_D4C52ar_strE0[12]` (EF:1926-1938)
| action idx | addr | handler |
|---|---|---|
| 0 | 0023AD90 | **no-op** (empty EV case, EV:2857-2859) |
| 1 | 0023AD90 | **no-op** |
| 2 | 0023AD90 | **no-op** |
| 3 | 0023AD90 | **no-op** |
| 4 | 0023AD90 | **no-op** |
| 5 | 0023AD90 | **no-op** |
| 6 | 0023AF60 | `sub_59F60` (EV:2864) |
| 7 | 0023C100 | `sub_5B100` (EV:2873) |
| 8 | 0023AC40 | `sub_59C40_getTerrtoZ` (EV:2845) |
| 9 | 0023AC60 | `sub_59C60` (EV:2849) |
| 10 | 0023AC80 | `UpdateScroll_59C80` (EV:2853) |

**CONFIRMED: actions 0-5 are all no-ops** — they map to `0x23AD90`, whose EV case (EV:2857-2859) is an empty `break;`. (Creator table `strE1[7]` at EF:1940-1947 gives the six class-14 sub-creator addresses.)

#### Action 8 / 9 — `sub_59C40_getTerrtoZ` (EF:37138-37145 wait, 41137-41145) and `sub_59C60` (41147-41155)
Both are trivial: `position.z = getTerrainAlt_10C40(&position)`. `sub_59C40` returns void, `sub_59C60` returns the alt. These are the static "X" checkpoint (model 3) and level-end (model 4) markers — they just keep their sprite pinned to the terrain height each frame. No sound, no RNG, no draw manipulation (their draw bit is cleared externally by the class-11 X-markers via `word_0x36DFE`/`word_0x36DFC`).

#### Action 10 — `UpdateScroll_59C80` (EF:41158-41196) — the pickup scroll
```c
int UpdateScroll_59C80(type_entity_0x6E8E* entity) {   // 23ac80
    if (x_D41A0_BYTEARRAY_4_struct.setting_38545 & 4) {          // pickups-disabled flag
        entity->struct_byte_0xc_12_15.byte[0] |= 1u;
        DisableEntityDrawing04_57F10(entity);                    // remove it
    } else {
        entity->position_0x4C_76.z = getTerrainAlt_10C40(&entity->position_0x4C_76);  // pin to terrain
        for (entity2 = dword_38519; entity2 > Entities_EA3E4[0]; entity2 = entity2->next_0) {  // class-3 list
            if (!entity2->model_0x40_64 && entity2->life_0x8 >= 0) {                  // live player
                if (sub_106C0(entity2, entity)) {                                     // proximity/overlap test
                    int playerIndex = entity2->dword_0xA4_164x->playerColorIndex_0x38_56;
                    if (playerIndex == D41A0_0.LevelIndex_0xc) {                      // only local player
                        int countXP = (setting_byte1_22 & MULTIPLAYER_MODE) ? 50 : 4; // XP grant
                        UpdateExperience_6E090(&entity2->dword_0xA4_164x->str_611, countXP);
                        PrepareEventSound_6E450(playerIndex, -1, 63);                 // sound 63
                        if (setting_byte1_22 & MULTIPLAYER_MODE) sub_6DBD0();
                        else                                       sub_6DB50(0, 1);
                    }
                    DisableEntityDrawing04_57F10(entity);                            // consume scroll
                }
            }
        }
    }
    return 1;
}
```
- **What UpdateScroll grants:** experience — **50 XP in multiplayer, 4 XP in single-player** (EF:41180-41183) via `UpdateExperience_6E090` on the touching player's `str_611`. Also fires a UI/notify (`sub_6DBD0` MP / `sub_6DB50(0,1)` SP, EF:41186-41188).
- **Pickup proximity:** `sub_106C0(player, scroll)` (EF:41173) — overlap/box test (**OPEN:** exact box, name-inferred). Iterates the **class-3 player list** (`dword_38519`), only players with `model==0` and `life>=0`.
- **Only the local player** (`playerColorIndex_0x38_56 == D41A0_0.LevelIndex_0xc`) receives XP/sound, but the scroll is consumed (`DisableEntityDrawing04_57F10`) on **any** player overlap.
- **Sound: id 63 (0x3F)** (EF:41184).
- **Draw-flag mechanic:** if `setting_38545 & 4` (scroll pickups suppressed this level), the scroll sets bit0 and immediately removes itself (EF:41161-41165). This is the same `setting_38545 & 4` gate seen in `sub_51610` creator (EF:37371) which conversely *shows* the scroll's draw bit — i.e. the flag flips scroll visibility/behaviour.
- **Interaction with the class-11 switches:** scrolls are class-14, but the **class-11 slot-4 switch (model 0x11)** watches class-5-model-4 live objects. **OPEN:** whether the scroll object here (class-14 model 5) is the same entity counted in `bytearray_38403x[4]` — it is not (that array is class-5-only, EF:39987). The "scroll4 gate" and the "scroll pickup" are two distinct object families; the linkage between them is via level-data disposition, not a direct pointer. Flag this as **OPEN** for the porter.

#### Action 6 — `sub_59F60` (EF:41255-42492) — terrain-raising "wall/ridge" animation (model 1)
Large (~1240 lines) procedural terrain-deformation routine driven by `life_0x8` phases and orientation `byte_0x46_70` (0 = X-axis, else Y-axis). Key structure & constants:
- Gated on `life_0x8`: `<1` branch (init/grow, EF:41470+), value `2` tail branch (EF:42138+). Uses `dword_0x10_16` as a per-frame growth counter and `subSpellIndex_0x2A_42` as a length accumulator (`>= 0x30u` termination check, EF:41255-region line 791).
- Writes directly into terrain arrays: `mapHeightmap_11B4E0[]` (raises by **+48**, EF:41506; +30 threshold checks EF:41504, 42-region), `mapAngle_13B4E0[]` (flags `|= 0x80`, `|= 8`, `&= 0xF7`), `x_BYTE_14B4E0_second_heightmap[]`, `mapTerrainType_10B4E0[]` (checks `== 8`, EF:41499).
- **Height clamp / RNG-like folding:** `if (v54 >= 28) if (v54 > 40) v54 = (v54 & 7) + 40;` (EF:41-region lines 350-353, 503-506, 817-820, 858-861) — a `&7` fold to keep raised heights in 40..47. **This `&7` is the only "RNG"/masking** — no `Random()` call; it's deterministic modular folding of heightmap values.
- **Day-vs-non-Day branch:** `if (terrain_2FECE.MapType != MapType_t::Day) v121 = 32 - v119 + 32; else v121 = v119;` (EF:42121-42124) — inverts a shade/height coefficient off the daytime map type.
- **Sound: id 47 (0x2F)** — emitted via `PrepareEventSound_6E450(idx, -1, 47)` at EF:41-region line 548 and 891, and via `EndLoop_6EAB0(idx, -1, 47)` at end-of-phase (`LABEL_292`, EF:42135, and 42145 uses PrepareEventSound 47).
- **Life transitions:** sets `life_0x8 = 3` on completion of a growth phase (EF:41-region 388, 539, 879; 42133), and `life_0x8 = 4` (fully done) at EF:42142 / 42-region 888. `EndLoop_6EAB0(...,47)` stops the looping sound.
- **No `Random()` draws, no distance-box thresholds** other than the `>30` height-discontinuity gate (EF:41504 etc.) and the 28/40/`&7` folds.

#### Action 7 — `sub_5B100` (EF:42530-42781) — cave stalactite/pillar carving (model 2, cave levels only)
Phase machine on `locEvent->life_0x8` (only runs while `<= 2`, EF:42547). Orientation from `word_0x2C_44` (0/nonzero → X vs Y span); span coefficient from `word_0x96_150`:
- `locKoef2 = (2*((word_0x96_150 << 8) + 512) + 128) >> 8` (EF:42550) — the pillar footprint size; assigns `koefX/koefY` = {locKoef2, 2} depending on orientation (EF:42551-42560).
- `signLocKoef2 = (8*locKoef2 - my_sign32(8*locKoef2)*31) >> 5` (EF:42565) — signed rounding of the vertical carve step.
- **case 0 (`life==0`, EF:42568):** sets `life=4` provisionally, scans outward (± along the span axis) for the two nearest `mapAngle_13B4E0 & 8` boundary cells to read floor/ceiling heights `mapHeight1/2`; if either not found → `sub_57F20(locEvent)` (abort/despawn); else sets `position.z = (mapHeight1+mapHeight2)>>1` and marks the footprint cells `mapAngle |= 0x80` (EF:42636).
- **case 1 (EF:42644):** plays **sound 47**; raises floor (`mapHeightmap_11B4E0`) up toward `koefZ1` and lowers ceiling (`x_BYTE_14B4E0_second_heightmap`) down toward `koefZ2`, clamping to `[0, locEventZ]` and `[locEventZ, 254]` (EF:42649-42656). Sets/clears `mapAngle & 8` per cell (solid if ceiling ≤ floor). When no cell changed (`finded1` stays true) → `life=3`, `EndLoop_6EAB0(...,47)` (EF:42691-42695).
- **case 2 (EF:42698):** plays **sound 47**; smooths the pillar edges using neighbour cells (±1/+2/+3 offsets) with a `<=4` height-difference blend (EF:42726-42763). When settled → `life=4`, `EndLoop_6EAB0(...,47)` (EF:42772-42776).
- **Sound: id 47 (0x2F)** at EF:42646, 42700; `EndLoop_6EAB0(...,47)` at 42694, 42775. **No RNG.** Distance-ish constants: `<= 4` smoothing threshold (EF:42726, 42745), ceiling clamp `254` (EF:42656), and `my_sign32(...)*31 >> 5` rounding (EF:42565).
- Helper `sub_5B070` (EF:42497-42526): looks up the map cell at the event's `(y-128, x-128)>>8` and returns a class-14 model-1-or-2 entity occupying it (used elsewhere to detect existing pillars). Cell index via `mapEntityIndex_15B4E0[]`, chain via `oldMapEntity_0x16_22`.

---

## 5. Consolidated constants: sounds, RNG, thresholds, phase gates

| Item | Value | Cite |
|---|---|---|
| Chain-fire sound (class-11 switch) | **sound 41 (0x29)** | EF:54478, 54496 (sub_6F300); EF:44539, EF:37... 6F850 |
| Terrain-anim looping sound (class-14 model 1 & 2) | **sound 47 (0x2F)** | EF:41802/…548, 42145 (59F60); 42646, 42700 (5B100) |
| Scroll pickup sound | **sound 63 (0x3F)** | EF:41184 (UpdateScroll_59C80) |
| Switch countdown start value | **16** | EF:54473, 54491 |
| Switch fires at countdown | **1** | EF:54476, 54494 |
| Scroll XP grant | **50 (MP) / 4 (SP)** | EF:41180-41183 |
| Proximity dist arg (X-markers) | `CompareAxisWithShift == 1` | EF:44535; callers EF:54436, 54694 |
| Phase gate (chain helper) | **`byte_0x3E_62 & 7`** (bits 0-2) | EF:44526 |
| Model-31 create guards | `!MULTIPLAYER_MODE && !(setting_38545 & 8)` | EF:37302 |
| Level-end marker suppress | `setting_38545 & 8` | EF:54692 |
| Scroll-suppress / show flag | `setting_38545 & 4` | EF:41161 (hide), 37371 (show), 37302 (model31) |
| Terrain height raise amount | **+48** | EF:41506 (59F60) |
| Height discontinuity gate | **> 30** | EF:41504, 42-region (59F60) |
| Height fold constants | `>= 28`, `> 40` → `(v & 7)+40` | EF:41-region 350-353, 503-506, 817-820, 858-861 |
| Cave carve ceiling clamp | **254** | EF:42656 (5B100) |
| Cave smoothing threshold | **<= 4** | EF:42726, 42745 (5B100) |
| Cave carve rounding | `>>5`, `* 31` | EF:42565 (5B100) |
| Day-map inversion | `MapType != Day` → `64 - v` | EF:42121-42124 (59F60) |

**No `Random()`/RNG draws** appear in any of the traced switch or class-14 handlers — the only stochastic-looking operations are the deterministic `& 7` height folds (EF:41-region 353 etc.). If the porter expects RNG in these paths, there is none — **flag as confirmed-absent**.

---

## OPEN items for the porter
1. **Models 16/17 "own ctors"** — none exist; they are plain `AddSwitchXX_50A90(pos,16/17,16/17)` wrappers (EF:37137-37146). CONTEXT premise appears wrong.
2. **Models 0x05-0x0B action handlers** (`sub_6F240`..`sub_6F2A0`, addrs 250240-2502A0 in `strB0`) are **outside** the `sub_6F300` family and were **not** in scope/read — trace separately if porting the full class-11 set.
3. **Model 0x11 "scroll pickup" framing** — the class-11 slot-4 switch grants nothing and has no radius; it is a plain empty-list gate. The actual scroll grant is `UpdateScroll_59C80` on a class-14 model-5 object. The linkage (which class-5 model feeds `bytearray_38403x[4]` vs the class-14 scroll) is via level disposition data, not a code pointer — unresolved here.
4. **`CompareAxisWithShift_10750`, `sub_106C0`, `SetHalfSpeedEntity_49DA0`, `SetEntityShiftRot_49EA0`, `AddEventToMap_57D70`, `CopyMaxLifeToLife_49A20`, `getTerrainAlt_10C40`** — referenced but bodies not read; names inferred. Read these before finalizing the box-distance / sprite-id / z-snap semantics.
5. **`sub_59F60` full verbatim** — ~1240 lines (EF:41255-42492); I captured the phase machine, sounds (47), height constants (+48, >30, 28/40/&7), and life transitions (3,4) but did not transcribe every line. A verbatim Rust port must copy the whole body.