# CLASS-10 Model 57 (0x39) — Verbatim Trace Report (the RANDOM-VALUE mana sphere)

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Trace date 2026-07-11.

**Headline finding (read before porting):**
Class-10 model **57 (0x39)** IS a mana-sphere-family object — the context hypothesis is correct. It is the **random-value ground mana sphere**: creator `sub_50130` (EF:36631) is a near-twin of the mana ball's `CreateManaSphere_500C0` (EF:36607) but grants a **random `rand % 0x7D0` = 0..1999 mana** instead of a fixed 512/2560, uses a **different action index (0x3E vs 0x29)**, sets **`byte_0x39_57 = -128`** (the ball uses +128), and sets two extra flag bytes (`byte_0x43_67 = 10`, `byte_0x44_68 = 1`) plus the reclaimable-effect bit `byte[2] |= 2`. It shares the mana-sphere list `dword_38523`, the color/size sprite routine `SetManaSphereColorAndRot_36920`, the sphere-merge routine `sub_36F30`, and the class-10 shove family, with the 0x27/0x28 balls.

Two subtle behavioural differences from the plain 0x27 ball, both keyed on `model == 57`:
1. **AI collection is skippable.** The flyer's mana-target picker `sub_148E0` (EF:6518) `break`s out of the sphere scan with probability `word_0x244_580/255` when it hits a model-57 sphere (EF:6544-6549), and its fallback loop excludes model 57 (EF:6597). Model-57 spheres are *avoided* by the collection AI in proportion to a per-wizard personality value.
2. **Physics uses a distinct bounce law (action 0x3E `sub_35FB0`, EF:26318)** vs the ball's action 0x29 (`TransformArcherToMana_35940`, EF:26015). The 0x3E handler has a `word_0x68_104`→spawn-(10,0) despawn branch (EF:26362-26366) that the ball's collection-oriented handler does not; the ball handler instead carries the full owner-transfer/sound-4 collection code (EF:26069-26095).

---

## 0. Shared infrastructure (see mc2-class10-m29-m5-m13.md §0 for the full version)

- Dispatch tables `str_D4C48ar[17]` (EF:2060); class 0xA creator table `x_DWORD_D4C52ar_strA1[93]` (EF:1703, indexed by **model**), action table `x_DWORD_D4C52ar_strA0` (indexed by **actionIndex**).
- `NewEvent_4A050` (EV:561): memset + defaults (`maxLife=300`, `subSpellIndex=100`, `byte_0x43_67=10`, `byte_0x39_57=-6`, per-entity RNG seed `rand_0x14_20`, …).
- `AddEventToMap_57D70` (EF:40315): links into per-cell list, `byte[0]|=4`, copies position.
- `CopyMaxLifeToLife_49A20` (EF:54118): `life_0x8 = maxLife_0x4`.
- RNG (all): LCG `r = 9377*r + 9439`; per-entity `rand_0x14_20`, global `D41A0_0.rand_0x8`.
- Removal: `DisableEntityDrawing04_57F10` (EF:40332) = `byte[1]|=4`; `sub_57F20` (EV:5209) unlinks + frees slot.

### 0.1 The mana-sphere size/color table
```c
int manaSphereSizeTable_DB538[8] = { 256, 512, 1024, 2048, 4096, 9192, 18384, 36768 }; // EF:2600
```
`SetManaSphereColorAndRot_36920` (EF:26735) walks this table to pick sprite size 0..7 by `mana`, then adds the owner color index (`GetManaSphereColorIndexFromEntityId_369F0`, EF:26782). For model 57's random mana 0..1999 the size is 0..3 (2048 is the size-3 boundary).
```c
void SetManaSphereColorAndRot_36920(type_entity_0x6E8E* entity)//217920
{
    int sphereSize;
    for (sphereSize = 0; sphereSize < 7 && entity->mana_0x90_144 > manaSphereSizeTable_DB538[sphereSize]; sphereSize++);
    int colorIndex = GetManaSphereColorIndexFromEntityId_369F0(entity->playerEntityIndex_0x94_148);
    if (colorIndex + sphereSize != entity->word_0x5A_90)
    {
        SetEntityIndex_49C90(entity, sphereSize + colorIndex);
        int16_t rotation; // 13,28,42,56,70,84,98,112 for size 0..7  (EF:26745-26773)
        ...
        entity->array_0x52_82.pitch = rotation; .roll = rotation; .fov = rotation; .yaw = rotation;  // EF:26774-26777
    }
}
```

---

## 1. Level-load THING path and creator dispatch

### 1.1 `sub_4A310` `case 0xA`, `v4 == 0x39` (EF:33033)
Exact branch walk for `v4 = model = 0x39` (57):
- `v4 < 0x22u`? **No** (57 ≥ 34) → else block (EF:33075).
- `v4 <= 0x22`? No.
- `v4 < 0x43u`? **Yes** (57 < 67) (EF:33084).
- `v4 < 0x36u`? No (57 ≥ 54).
- `v4 <= 0x36u`? No (57 > 54) → else (EF:33105).
- `v4 < 0x3Du`? **Yes** (57 < 61) →
```c
if (v4 < 0x3Du)
{
    sub_58DA0(entity, v3x);   // EF:33109 — plain stage-bind, no par consumption
    return;
}
```
**Model 0x39 consumes NO par1/par2/par3/word_10/stageTag fields.** It takes the same plain stage-bind default as model 5 does. It does NOT reach the `subSpellIndex`/`life` spell-lookup block at EF:33148 (that block is fallen through only from the branches that do not `return`; the 0x39 case returns). Only side effect: `sub_58DA0` (stage binder, §0.3 of the m29 doc) captures the entity into any stage record pointing at this THING.

*(Note: `byte_0x39_57` at struct offset 0x39 is a completely different field from the model value 0x39; do not conflate. The `v4 == 0x39` test above is on `indexx->model_0x40_64`.)*

### 1.2 Creator dispatch (strA1 model→address, and EV address→function)
```
x_DWORD_D4C52ar_strA1 rows (EF):
  model 0x27 (39) -> 0x00231080  (EF:1743)  CreateManaSphere512_50080  -> 512 mana
  model 0x28 (40) -> 0x002311D0  (EF:1744)  sub_501D0                  -> collector/"archer" sphere
  model 0x39 (57) -> 0x00231130  (EF:1761)  sub_50130                 -> RANDOM 0..1999 mana   <-- THIS
  model 0x3A (58) -> 0x002310A0  (EF:1762)  CreateManaSphere2560_500A0 -> 2560 mana
```
EV dispatch (`IfSubtypeCallCreatingManaSphere_4A190` routes `4A190(pos,10,N)` through `dword_14[N]` → these addresses):
```c
case 0x231080: { return CreateManaSphere512_50080(a1_axis3d); }   // EV:4755  "//creating mana sphere"
case 0x2310a0: { return CreateManaSphere2560_500A0(a1_axis3d); }  // EV:4759
case 0x2310c0: { return CreateManaSphere_500C0(a1_axis3d, 0); }   // EV:4763  core, 0-mana
case 0x231130: { return sub_50130(a1_axis3d); }                   // EV:4767  <-- model 0x39 ctor
case 0x2311d0: { return sub_501D0(a1_axis3d); }                   // EV:4771  model 0x28
```

---

## 2. Creator `sub_50130` (EF:36631) VERBATIM — with the ball for comparison

```c
type_entity_0x6E8E* sub_50130(axis_3d* position)//231130
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event)
    {
        event->actionIndex_0x45_69 = 0x3E;                 // <-- action 0x3E (ball = 0x29)
        event->class_0x3F_63 = 0xA;
        event->model_0x40_64 = 0x39;
        event->xtype_0x41_65 = 10;
        event->xsubtype_0x42_66 = 57;                      // cross-column damage twin = (10,57)
        event->word_0x2C_44 = 128;                         // initial vertical velocity / gravity accumulator
        event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;   // RNG draw #1 (entity)
        event->actSpeed_0x82_130 = 0;                      // <-- 0 (ball = 32)
        event->byte_0x38_56 = 3;
        event->byte_0x39_57 = -128;                        // <-- -128 (ball = +128)
        event->byte_0x3A_58 = 0;
        event->byte_0x43_67 = 10;                          // <-- extra (ball does not set)
        event->mana_0x90_144 = event->rand_0x14_20 % 0x7D0;// <-- RANDOM 0..1999 mana
        event->byte_0x44_68 = 1;                           // <-- extra (ball does not set)
        event->struct_byte_0xc_12_15.byte[2] |= 2;         // <-- reclaimable-effect flag (ball does not set)
        AddEventToMap_57D70(event, position);
        CopyMaxLifeToLife_49A20(event);                    // life = maxLife = 300 (NewEvent default)
        SetManaSphereColorAndRot_36920(event);             // sprite/rot by mana+ownercolor
    }
    return event;
}
```
For reference the plain ball:
```c
type_entity_0x6E8E* CreateManaSphere_500C0(axis_3d* position, __int16 mana)//2310c0  (EF:36607)
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event)
    {
        event->actionIndex_0x45_69 = 0x29;
        event->class_0x3F_63 = 0xA;
        event->model_0x40_64 = 0x27;
        event->xtype_0x41_65 = 10;
        event->xsubtype_0x42_66 = 39;
        event->word_0x2C_44 = 128;
        event->actSpeed_0x82_130 = 32;
        event->byte_0x38_56 = 3;
        event->byte_0x39_57 = 128;
        event->byte_0x3A_58 = 0;
        event->mana_0x90_144 = mana;
        AddEventToMap_57D70(event, position);
        CopyMaxLifeToLife_49A20(event);
        SetManaSphereColorAndRot_36920(event);
    }
    return event;
}
```

**RNG draws in `sub_50130`: exactly 1**, and it is consumed twice-over in one statement — `rand_0x14_20` is advanced once (EF:36642) then read for `mana = rand % 0x7D0` (EF:36648). No sprite is set explicitly other than via `SetManaSphereColorAndRot_36920`. `maxLife/life = 300` (from `NewEvent_4A050`, never overridden here → the sphere is long-lived, matching a persistent ground pickup).

Note field decode: `word_0x2C_44` is the vertical-velocity / gravity accumulator (initial 128 = downward settle). `byte_0x39_57 = -128` is read in the bounce law's `else if (a1x->byte_0x39_57 || v31)` branch (EF:26526) → because `-128 != 0`, model-57 spheres **always take the bounce/settle-with-random-drift branch**, whereas a ball with `byte_0x39_57=+128` also takes it (non-zero). The functional distinction here is the value's sign is irrelevant to the branch test (just truthiness); `byte_0x39_57` is not otherwise consumed inside `sub_35FB0`.

---

## 3. Tick action handler — action 0x3E `sub_35FB0` (EF:26318) VERBATIM

Table row: `0x002A5C44, 0x003E, 0x00216FB0, 0x00000001` (EF:1664) → address 0x216FB0 → `sub_35FB0`.

Full function (verbatim, exactly as decompiled):
```c
void sub_35FB0(type_entity_0x6E8E* a1x)//216FB0
{
    char v1; unsigned __int16 v2, v5, v8; type_entity_0x6E8E* v6x; char v7;
    signed int v9; type_entity_0x6E8E* v10x; __int16 v11, v12, v13, v14, v15;
    type_entity_0x6E8E* v16x; __int16 v17; int v18; __int16 v20, v21, v22, v23, v24;
    int v25; type_entity_0x6E8E* v27x; int v28, v29; signed __int16 v30; char v31;

    v1 = a1x->struct_byte_0xc_12_15.byte[1];
    v31 = 0;
    if (v1 & 8)                                             // byte[1]&8 = "just spawned / skip one tick"
    {
        a1x->struct_byte_0xc_12_15.byte[1] = v1 & 0xF7;    // clear it
    }
    else if (a1x->str_0x5E_94.word_0x68_104 && sub_36680(a1x))  // collected-into-mailbox path
    {
        IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 0);  // spawn (10,0) poof
        DisableEntityDrawing04_57F10(a1x);                 // consume the sphere
    }
    else
    {
        if (a1x->str_0x5E_94.word_0x7A_122)                // homing target assigned (by sub_38D80 vacuum)
        {
            v2 = a1x->str_0x5E_94.word_0x7A_122;
            a1x->actSpeed_0x82_130 = 0;
            v31 = 1;                                        // <-- forces the drift branch below
            a1x->yaw_0x1C_28 = Maths::sub_581E0_maybe_tan2(&a1x->position_0x4C_76, &Entities_EA3E4[v2]->position_0x4C_76);
            predictedAxis_EB398ar.x = 0; predictedAxis_EB398ar.y = 0; predictedAxis_EB398ar.z = 0;
            MoveEntity_57FA0(&predictedAxis_EB398ar, a1x->yaw_0x1C_28, 0, a1x->str_0x5E_94.word_0x76_118);
            a1x->axis_0x9A_154x.x = predictedAxis_EB398ar.x;
            a1x->axis_0x9A_154x.y = predictedAxis_EB398ar.y;
            a1x->str_0x5E_94.word_0x7A_122 = 0;
        }
        if (a1x->struct_byte_0xc_12_15.byte[0] & 0x40)     // being attracted to a collector (word_0x96_150)
        {
            v5 = a1x->word_0x96_150;
            a1x->actSpeed_0x82_130 = 0;
            v6x = Entities_EA3E4[v5];
            v7 = 1;
            if (v6x->class_0x3F_63 != 3 || 3 != v6x->model_0x40_64)    // collector is not a castle
            {
                if (v6x->class_0x3F_63 == 5 && v6x->model_0x40_64 == 23) // collector is a "mailbox"(5,23)
                {
                    v7 = 0;
                    v30 = v6x->word_0x2C_44;               // approach speed from mailbox
                }
            }
            else { v7 = 0; v30 = 32; }                     // castle: approach speed 32
            if (v7)
            {
                a1x->struct_byte_0xc_12_15.byte[0] &= 0xBFu; // give up attraction
            }
            else
            {
                v8 = a1x->word_0x96_150;
                a1x->word_0x2C_44 = 128;
                a1x->yaw_0x1C_28 = Maths::sub_581E0_maybe_tan2(&a1x->position_0x4C_76, &Entities_EA3E4[v8]->position_0x4C_76);
                v9 = Maths::EuclideanDistXYZ_58490(&a1x->position_0x4C_76, &Entities_EA3E4[a1x->word_0x96_150]->position_0x4C_76);
                if (v9 <= 1024)
                {
                    if (v9 >= 16)
                    {
                        predictedAxis_EB398ar = a1x->position_0x4C_76;
                        MoveEntity_57FA0(&predictedAxis_EB398ar, a1x->yaw_0x1C_28, 0, 16);  // step 16 toward collector
                    }
                    else                                    // v9 < 16: snap-lerp toward collector, converge z
                    {
                        predictedAxis_EB398ar = a1x->position_0x4C_76;
                        v10x = Entities_EA3E4[a1x->word_0x96_150];
                        predictedAxis_EB398ar.x = v10x->position_0x4C_76.x;
                        predictedAxis_EB398ar.y = v10x->position_0x4C_76.y;
                        v11 = v10x->position_0x4C_76.z;
                        if (predictedAxis_EB398ar.z >= v11) { if (predictedAxis_EB398ar.z > v11 + 512) predictedAxis_EB398ar.z -= v30; }
                        else                                  predictedAxis_EB398ar.z += v30;
                    }
                    v12 = getTerrainAlt_10C40(&predictedAxis_EB398ar);
                    if (v12 > predictedAxis_EB398ar.z) predictedAxis_EB398ar.z = v12;   // clamp above ground
                    CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);
                }
                else
                {
                    a1x->struct_byte_0xc_12_15.byte[0] &= 0xBFu;   // collector too far (>1024): give up
                }
            }
        }
        else
        {
            v13 = a1x->actSpeed_0x82_130;
            if (v13)                                        // lateral thrown motion (spheres knocked by flood/shove)
            {
                if (v13 <= 0) { if (v13 < -4) a1x->actSpeed_0x82_130 = v13 + 4; }   // decel toward 0, step 4
                else if (v13 > 4) a1x->actSpeed_0x82_130 = v13 - 4;
                predictedAxis_EB398ar = a1x->position_0x4C_76;
                MoveEntity_57FA0(&predictedAxis_EB398ar, a1x->yaw_0x1C_28, a1x->pitch_0x1E_30, a1x->actSpeed_0x82_130);
                v14 = a1x->word_0x2C_44 - 16;               // GRAVITY: velocity -= 16/tick
                a1x->word_0x2C_44 = v14;
                if (v14 < -128) a1x->word_0x2C_44 = -128;   // terminal fall speed clamp -128
                predictedAxis_EB398ar.z += a1x->word_0x2C_44;
                if (isCaveLevel_D41B6 && (unsigned __int8)sub_11E70(a1x, &predictedAxis_EB398ar))   // cave wall block
                {
                    predictedAxis_EB398ar = a1x->position_0x4C_76;
                    a1x->actSpeed_0x82_130 = 0;
                    a1x->word_0x2C_44 = -128;
                }
                else { CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar); }
                v15 = getTerrainAlt_10C40(&predictedAxis_EB398ar);
                if (v15 <= (int16_t)predictedAxis_EB398ar.z)   // still airborne
                {
                    if (isCaveLevel_D41B6)                    // cave-ceiling bounce
                    {
                        v17 = sub_10C60(&predictedAxis_EB398ar) - a1x->array_0x52_82.fov;
                        if (v17 < predictedAxis_EB398ar.z)
                        {
                            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;   // RNG (cave only)
                            v18 = (a1x->rand_0x14_20 & 3) - 2;
                            a1x->actSpeed_0x82_130 = v18;
                            if (!(_WORD)v18) a1x->actSpeed_0x82_130 = 1;
                            a1x->word_0x2C_44 = -abs(a1x->word_0x2C_44);
                            predictedAxis_EB398ar.z = v17;
                        }
                    }
                }
                else                                          // hit terrain
                {
                    a1x->actSpeed_0x82_130 = 0;
                    a1x->position_0x4C_76.z = v15;
                    if (sub_104D0_terrain_tile_is_water(&a1x->position_0x4C_76) == 1)   // landed in water
                    {
                        a1x->word_0x2C_44 = 0;
                        v16x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 5);  // splash
                        if (v16x) PrepareEventSound_6E450(v16x - D41A0_0.struct_0x6E8E, -1, 27);      // splash sound 27
                    }
                    else { a1x->word_0x2C_44 = 128; }         // reset gravity accumulator on land
                }
            }
            else if (a1x->byte_0x39_57 || v31)              // resting/settle-with-jitter (model-57 always enters: byte_0x39_57=-128)
            {
                if ((int16_t)a1x->axis_0x9A_154x.x < -64) a1x->axis_0x9A_154x.x = -64;   // clamp jitter x [-64,64]
                if ((int16_t)a1x->axis_0x9A_154x.x >  64) a1x->axis_0x9A_154x.x =  64;
                if ((int16_t)a1x->axis_0x9A_154x.y < -64) a1x->axis_0x9A_154x.y = -64;   // clamp jitter y [-64,64]
                if ((int16_t)a1x->axis_0x9A_154x.y >  64) a1x->axis_0x9A_154x.y =  64;
                predictedAxis_EB398ar = a1x->position_0x4C_76;
                predictedAxis_EB398ar.x = a1x->axis_0x9A_154x.x + a1x->position_0x4C_76.x;   // drift by residual velocity
                predictedAxis_EB398ar.y = a1x->axis_0x9A_154x.y + a1x->position_0x4C_76.y;
                v20 = a1x->word_0x2C_44;
                predictedAxis_EB398ar.z += v20;
                a1x->word_0x2C_44 = v20 - 16;                 // gravity again
                if ((int16_t)(v20 - 16) < -128) a1x->word_0x2C_44 = -128;
                if (isCaveLevel_D41B6 && (unsigned __int8)sub_11E70(a1x, &predictedAxis_EB398ar))
                {
                    if (v31) { predictedAxis_EB398ar = a1x->position_0x4C_76; }
                    else { a1x->axis_0x9A_154x.x = 64; v21 = a1x->position_0x4C_76.x; v22 = a1x->axis_0x9A_154x.x;
                           a1x->axis_0x9A_154x.y = 64; v23 = a1x->axis_0x9A_154x.y;
                           predictedAxis_EB398ar.x = v22 + v21; predictedAxis_EB398ar.y = v23 + a1x->position_0x4C_76.y; }
                    a1x->word_0x2C_44 = -128;
                }
                v24 = getTerrainAlt_10C40(&predictedAxis_EB398ar);
                if (v24 > (int16_t)predictedAxis_EB398ar.z)   // below ground -> bounce
                {
                    v25 = -((a1x->word_0x2C_44 - (my_sign32(a1x->word_0x2C_44) * 4) + my_sign32(a1x->word_0x2C_44)) >> 2);
                    a1x->word_0x2C_44 = v25;                  // reflect+damp velocity (quarter, sign-corrected)
                    if ((signed __int16)v25 <= 16) a1x->word_0x2C_44 = 0;   // small bounce -> stop
                    predictedAxis_EB398ar.z = v24;
                }
                if (isCaveLevel_D41B6)                         // cave ceiling
                {
                    v24 = sub_10C60(&predictedAxis_EB398ar) - a1x->array_0x52_82.fov;
                    if (v24 < predictedAxis_EB398ar.z) { a1x->word_0x2C_44 = -abs(a1x->word_0x2C_44); predictedAxis_EB398ar.z = v24; }
                }
                CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);
                if (v24 == predictedAxis_EB398ar.z)           // came to rest on the surface this tick
                {
                    v27x = sub_10A50(a1x);                     // any other sphere sharing this cell?
                    if (v27x)
                    {
                        sub_36F30(a1x, v27x);                  // MERGE: this.mana += that.mana, free that (EF:27002)
                        SetManaSphereColorAndRot_36920(a1x);   // re-size sprite for new mana total
                    }
                    sub_58030(&a1x->position_0x4C_76, &predictedAxis_EB398ar);
                    a1x->axis_0x9A_154x.x += predictedAxis_EB398ar.x;
                    v28 = ((250 * (int16_t)a1x->axis_0x9A_154x.x) - (my_sign32(250 * (int16_t)a1x->axis_0x9A_154x.x) * 256) + my_sign32(250 * (int16_t)a1x->axis_0x9A_154x.x)) >> 8;  // residual *= 250/256 ~= 0.977 friction
                    a1x->axis_0x9A_154x.y += predictedAxis_EB398ar.y;
                    v29 = a1x->axis_0x9A_154x.y;
                    a1x->axis_0x9A_154x.x = v28;
                    a1x->axis_0x9A_154x.y = ((250 * (int16_t)v29) - (my_sign32(250 * (int16_t)v29) * 256) + my_sign32(250 * (int16_t)v29)) >> 8;
                }
            }
        }
    }
}
```

**Integer semantics that matter for the port:**
- `word_0x2C_44` is the signed vertical velocity accumulator; gravity is `-16`/tick, clamped to `-128` (terminal). On landing it resets to `128` (land) or `0` (water/rest).
- Bounce reflection `v25 = -((v - sign(v)*4 + sign(v)) >> 2)` = arithmetic shift right by 2 with sign correction (≈ reflect and quarter). `<= 16` → snap to 0 (comes to rest). `my_sign32` in the port must reproduce IDA's `>> 31` sign trick (the commented-out original at EF:26570-26572 shows the exact ancestry).
- Horizontal-residual friction after settle: `x = (250*x - sign(250*x)*256 + sign(250*x)) >> 8` ≈ `x * 250/256` truncated toward zero. Applied per tick once at rest.
- Jitter clamp `[-64, 64]` on `axis_0x9A_154x` is applied as `(int16_t)` (signed) compare — sign-preserving.
- `sub_10A50(a1x)` returns another sphere in the same map cell; `sub_36F30` (§4.3) then **absorbs** it: `a1x->mana += other->mana; free(other)`. This is how two settled spheres coalesce into a larger one (visible as the sprite growing via `SetManaSphereColorAndRot`).
- No `life--`/despawn on age exists inside `sub_35FB0` — the sphere persists (life stays at NewEvent's 300 and is never decremented here). It is removed only by (a) collection into a mailbox (`word_0x68_104` path, EF:26362-26366), (b) merge (as the absorbed `a2x`), or (c) the external doomsday/apocalypse sweeps that overwrite life (§4.4).

---

## 4. Who else reads/writes model 0x39 entities

### 4.1 Per-tick list builder → `dword_38523` (EF:39964-40075)
Class 0xA models are bucketed; model 0x39 joins the SAME list as 0x27/0x28:
```c
case 0x0A:
    if (jx->model_0x40_64 < 0x2D) {
        if (jx->model_0x40_64 < 0x27) continue;
        if (jx->model_0x40_64 <= 0x28) {                    // models 0x27, 0x28
            ... push to dword_38523 ...  continue;           // EF:40024-40031
        }
        if (jx->model_0x40_64 != 0x2A) continue;
        ... push to dword_38535 ...  continue;
    }
    if (jx->model_0x40_64 <= 0x2D) { ... dword_38527 ... continue; }
    if (jx->model_0x40_64 < 0x43) {
        if (jx->model_0x40_64 != 0x39) continue;             // <-- ONLY 0x39 in [0x2E,0x42]
        ... push to dword_38523 ...  continue;               // EF:40057-40063 — SAME list as 0x27/0x28
    }
    ...
```
So `dword_38523` = {all model 0x27, 0x28, and 0x39 spheres}. Every consumer below iterates it.

### 4.2 The AI collection target picker `sub_148E0` (EF:6518) — model 57 is AVOIDED
```c
type_entity_0x6E8E* sub_148E0(type_entity_0x6E8E* a1x)//1f58e0
{
    ...
    v2x = x_D41A0_BYTEARRAY_4_struct.dword_38523;
    v15x = Entities_EA3E4[a1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58];
    while (v2x > Entities_EA3E4[0])
    {
        if (a1x == Entities_EA3E4[v2x->playerEntityIndex_0x94_148]) goto LABEL_22;   // skip own spheres
        if (v2x->model_0x40_64 == 57)                                                // <-- MODEL 0x39
        {
            v3 = a1x->dword_0xA4_164x->word_0x244_580;      // wizard "avoid random spheres" personality 0..255
            if (watcomrand() % 255 < v3) break;             // probabilistically ABANDON the whole scan
        }
        ... distance-nearest selection into resultx ...
    LABEL_22: v2x = v2x->next_0;
    }
    if (!resultx) {                                          // fallback: nearest sphere in a different bucket
        for (ix = ...bytearray_38403x[88/4]; ...; ...)
            if (ix->model_0x40_64 != 57 && ...) { ... }      // <-- fallback also EXCLUDES model 57
    }
    return resultx;
}
```
Callers: `sub_13CE0` (EF:6137/6148) — the flyer/wizard "go fetch mana" AI, which on success sets `word_0x96_150 = target` and `word_0x98_152 = sub_14C40(target)` (the homing lock that later makes the sphere set `byte[0]&0x40` and fly to the collector). Net effect: **model-57 spheres are collected less eagerly** than plain balls, controlled by the per-wizard `word_0x244_580`.

The SAME `model == 57` gate appears in the mana-economy scan at EF:6544 (function starting EF:6518 is `sub_148E0`; the identical pattern at the EF:6538/6544 region is inside it — one function).

### 4.3 Sphere merge `sub_36F30` (EF:27002) — mana coalescing
```c
void sub_36F30(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x)//217f30
{
    __int16 v2; LOBYTE(v2) = 0;
    if (a1x->byte_0x46_70 >= a2x->byte_0x46_70) {
        if (a2x->parentId_0x28_40 != a1x->parentId_0x28_40) {
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
            v2 = a1x->rand_0x14_20 & 1;                     // coin-flip ownership on tie
        }
    } else { LOBYTE(v2) = 1; }
    if ((x_BYTE)v2) {                                        // adopt a2's owner/spell identity
        a1x->parentId_0x28_40      = a2x->parentId_0x28_40;
        a1x->playerEntityIndex_0x94_148 = a2x->playerEntityIndex_0x94_148;
        a1x->byte_0x46_70          = a2x->byte_0x46_70;
        a1x->subSpellIndex_0x2A_42 = a2x->subSpellIndex_0x2A_42;
    }
    a1x->mana_0x90_144 += a2x->mana_0x90_144;               // ABSORB mana
    return sub_57F20(a2x);                                  // free the absorbed sphere
}
```
Called from `sub_35FB0` when a settled sphere finds another in its cell (EF:26594) — model-57 and model-27 spheres freely merge (both in dword_38523, both settle via their handlers).

### 4.4 Bulk sweeps over `dword_38523` (apply to model 57 too)
- **Doomsday/dome (class-5 (5,10) & class-10 (10,9))**: `sub_21360`-region at EF:12848 sets every dword_38523 sphere `maxLife=140; byte[1]|=0x20; life=maxLife` (EF:12851-12853) — force-expires all ground mana when the apocalypse triggers. Another sweep at EF:13049 flips `byte[3]` flags / despawns based on `v29` (EF:13051-13067).
- **Vacuum/attractor `sub_38D80` (EF:28362)**: scans dword_38523, and for each sphere without a homing target computes range and assigns `word_0x76_118/word_0x78_120/word_0x7A_122` (the homing vector consumed at EF:26369-26383). This is the tornado/whirlwind-style mana attractor and treats model 57 identically to 0x27.
- **`sub_28000` (EF:18384)** finds the nearest dword_38523 entity but **filters `model == 39` (0x27) only** — this "nearest mana BALL" helper deliberately excludes model 0x39/0x28.

### 4.5 Shove/collision filter `sub_39FA0` (EF:29214), class-10 case (EF:29291)
```c
case 9u:   // class 0xA (index = class-1 = 9)
    v8 = a2x->model_0x40_64;
    if (v8 < 0x27u) { v9 = (v8 == 6); }
    else {
        if (v8 <= 0x28u) return result;                     // models 0x27,0x28 -> shoveable (result=1)
        v9 = (v8 == 57);                                    // model 0x39 -> shoveable
    }
    if (!v9) result = 0;                                    // everything else class-10 -> not shoveable
    return result;
```
Model 6, 0x27, 0x28, and 57 (0x39) are the four physically-shoveable class-10 models (confirms the (10,67) flood filter note). A moving body pushes model-57 spheres.

### 4.6 Collection completion (into castle/mailbox)
The `word_0x68_104` field is the "collected by" latch. `sub_35FB0` reads it (EF:26362): if set and `sub_36680(a1x)` (EF:26615) confirms delivery (the sphere reached `parentId`'s mailbox), it spawns a (10,0) poof and despawns the sphere — the mana is credited elsewhere by the collector path, not inside this handler. (Contrast: the plain ball's action 0x29 `TransformArcherToMana_35940`, EF:26069-26095, carries the owner-transfer + sound-4 code inline; the 0x3E handler relies on `sub_36680`.) `word_0x68_104` is written by the wizard-collection path (e.g. EF:4199 sets a sphere's `word_0x68_104 = a1x->id_0x1A_26` when a collector claims it).

---

## 5. Consolidated constants

| Item | Value | Cite |
|---|---|---|
| Creator | `sub_50130` @ 0x231130 | EF:36631, EF:1761, EV:4767 |
| class / model | 0xA / 0x39 (10 / 57) | EF:36637-36638 |
| xtype / xsubtype (damage twin) | 10 / 57 | EF:36639-36640 |
| action index | **0x3E** (handler `sub_35FB0` @ 0x216FB0) | EF:36636, EF:1664, EF:26318 |
| mana granted | **`rand_0x14_20 % 0x7D0` = 0..1999** (random) | EF:36648 |
| maxLife / life | **300** (NewEvent default, not overridden) | EV:578, EF:36652 |
| word_0x2C_44 (grav accum) | init 128; `-16`/tick; clamp `-128`; reset 128 on land, 0 on water/rest | EF:36641, 26474-26478, 26522 |
| actSpeed_0x82_130 | init **0** (ball=32); lateral throw, decel 4/tick toward 0 | EF:36643, 26461-26469 |
| byte_0x38_56 | 3 | EF:36644 |
| byte_0x39_57 | **-128** (ball=+128); truthiness only → always enters settle branch | EF:36645, 26526 |
| byte_0x43_67 | 10 (ball does not set) | EF:36647 |
| byte_0x44_68 | 1 (ball does not set) | EF:36649 |
| byte[2] \|= 2 | reclaimable-effect flag (ball does not set) | EF:36650 |
| sprite / rot | `SetManaSphereColorAndRot_36920`: size 0..7 by `mana` vs `manaSphereSizeTable`, +owner color; rot 13/28/42/56/70/84/98/112 | EF:36653, 26735-26777 |
| size table | {256,512,1024,2048,4096,9192,18384,36768} → mana 0..1999 = size 0..3 | EF:2600 |
| list membership | `dword_38523` (SAME as 0x27/0x28) | EF:40055-40063 |
| RNG draws in ctor | 1 (advance, then `%0x7D0`) | EF:36642, 36648 |
| RNG in tick | only cave-ceiling bounce (`(rand&3)-2`) and merge tie-break (`rand&1`) | EF:26499, 27011 |
| water landing | spawn (10,5) splash + sound 27 | EF:26516-26518 |
| collected/delivered | `word_0x68_104` set + `sub_36680` true → spawn (10,0), despawn | EF:26362-26366 |
| merge | `sub_36F30`: `mana += other.mana`, free other | EF:26594, 27002 |
| AI avoidance | `sub_148E0` breaks scan w.p. `word_0x244_580/255` on model 57; fallback excludes it | EF:6544-6548, 6597 |
| shoveable | yes (case 9: models 6/0x27/0x28/57) | EF:29291-29304 |
| THING par consumption | **none** (case 0xA, v4=0x39 → plain `sub_58DA0` at EF:33109) | EF:33107-33110 |
| authored | x89 across 11 campaign levels (levels 009,020,024,030 among them) | context (verify via mgc-import) |

---

## 6. Port notes — retail fields → project entity-field conventions

Using the doc-bank convention keys (subSpell→f140, byte_0x46_70→f71, dword_0x10_16→f26, word_0x2C_44→f44, actionIndex→tick70):

| Retail field | Project field | Value for (10,57) |
|---|---|---|
| `mana_0x90_144` | mana / f144 (mana grant) | `rng() % 2000` — RANDOM per instance (the defining trait) |
| `actionIndex_0x45_69` | tick70 | select the (10,57) bounce/settle tick = `sub_35FB0` law (§3) |
| `word_0x2C_44` | f44 | vertical velocity accumulator; init 128, grav -16, clamp -128 |
| `byte_0x39_57` | (offset-0x39 byte) | -128 (only truthiness matters → always take settle branch) |
| `byte_0x43_67` | f67 | 10 |
| `byte_0x44_68` | f68 | 1 |
| `subSpellIndex_0x2A_42` | f140 (subspell) | 100 (NewEvent default; not overridden by (10,57) ctor) |
| `byte_0x46_70` | f71 | NewEvent default (used in merge priority `sub_36F30`) |
| `dword_0x10_16` | f26 | NewEvent default 0 (not used by (10,57) tick; used by 0x28's ctor only) |
| — (list) | mana-sphere collision list | put (10,57) in the SAME list as (10,39)/(10,40) balls |
| — (color/size) | sphere sprite | drive size from mana via the 8-entry size table; add owner-color base |
| xsubtype = 57 | cross-column damage twin | MC2 ctors set the f28=1 cross-column contract as usual |

**One-line port recommendation:** Implement (10,57) as a mana-sphere twin of the existing (10,39)/(10,40) balls — reuse `CreateManaSphere`/`SetManaSphereColorAndRot`/the settle-merge tick — but (a) grant `rng()%2000` mana instead of a fixed amount, (b) give it action 0x3E's own bounce-settle handler (nearly identical to the ball's 0x29 but with the `sub_36680` mailbox-delivery early-out rather than inline owner transfer), (c) register it in the same collision list so it merges with balls, and (d) honor the AI-avoidance gate (`word_0x244_580` probability skip in the collector's target picker). It is NOT the "third puff" (that is model 0x57 = 87).

---

## 7. OPEN items (not fully verified from this source)

1. **`byte_0x39_57` sign semantics.** In `sub_35FB0` only `if (a1x->byte_0x39_57 || v31)` reads it (truthiness), so -128 vs +128 is behaviourally identical *inside this handler*. Whether some OTHER reader distinguishes the sign for model-57 spheres was not found (grep of `byte_0x39_57` reads showed only per-entity assignments and the truthiness test). If the original relies on the sign elsewhere, verify against recorded gameplay. The 12 assignment sites at EF:33690-34473 are other entities' ctors, not sphere consumers.
2. **`sub_36680` (EF:26615) delivery predicate** was read only at its head (checks `parentId_0x28_40 == word_0x68_104`); the full body / what "delivered" means for the mana credit (which wizard's mana is incremented, and by how much) was not transcribed end-to-end here. Trace it before wiring the collection credit if the ball's path differs.
3. **`word_0x244_580` provenance.** It is a wizard `dword_0xA4_164x` (brain) field used as an "avoid random spheres" probability and also in cast-decision math (EF:5692, 6969-6974, 7004-7009). Where it is initialized per-wizard (wizards.json/AI-personality import) was not located in EF/EV — confirm the value range/source in the rivals/AI data import.
4. **x89 authored-count / affected levels.** The context states x89 across 11 levels (009,020,024,030 among them); verify via mgc-import THING dump that these are class 10 / model 57 records and that none carry non-zero par fields (the THING path consumes none, so any authored par bytes would be inert — worth confirming they are indeed zero in the data).
5. **`sub_10A50` cell-neighbor query and `sub_58030`** (used in the settle/merge, EF:26591/26597) were treated as opaque helpers here (return "another sphere in this cell" and a position blend respectively); if the merge produces visible artifacts in the port, transcribe them.
6. **`byte[1] & 8` "skip-one-tick" bit** (EF:26358) — its setter for model-57 spheres was not located; likely `AddEventToMap`/spawn-frame housekeeping shared with other effects. Benign if the port simply clears it on the first tick.
