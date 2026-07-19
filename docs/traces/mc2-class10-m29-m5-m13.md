# CLASS-10 Effect Models 29 (0x1D), 5 (0x05), 13 (0x0D) — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Trace date 2026-07-10.

**Headline findings (read before porting):**
1. **(10,29) is NOT the smoke column.** Its ctor creates an invisible, sprite-less entity with action 0x1F, and action 0x1F (`sub_34350`, EF:24996) is a bare `DisableEntityDrawing04_57F10` — the entity lives **exactly one tick**. Its only job is to exist long enough for the level-load stage binder (`sub_58DA0`) to capture it into stage/objective records. The playtest label "(10,29)=quest-beacon smoke column" does not match this decompile — see §3.5 and OPEN-1.
2. **The actual quest-beacon smoke column is model 0x3B (59)** (`ArriveCheckpoint_4EB50`, EV comment "in quest point"), which every tick spawns a **(10,13)** smoke cloud. The volcano-smoke variant is model 0x3C (60) (`AddSmoke_4EC10`, EV comment "1 instance in level 9"), which spawns **(10,14)** clouds. Both emitter chains traced verbatim in §4 because they are the gameplay-critical beacon.
3. **(10,13) is NOT a gib/debris chunk — it is the transparent smoke puff.** Creator is literally named `SetParticleSmoke3B_4E9E0`. It *is* spawned on destruction of scenery props (class-2 action 0x12, `sub_651B0`) — as the smoke poof accompanying the destruction, and by falling class-2 debris when it comes to rest. The flying debris chunks themselves are class-2 entities (actions 0x14/0x15, `sub_652C0`) — out of scope here, cited as callers.

---

## 0. Shared infrastructure

### 0.1 Dispatch tables
`str_D4C48ar[17]` (EF:2060) maps class → (action table `dword_10`, creator table `dword_14`). For class 0xA: `dword_10 = x_DWORD_D4C52ar_strA0` (EF:1601, indexed by **actionIndex**), `dword_14 = x_DWORD_D4C52ar_strA1` (EF:1703, indexed by **model**). Row layout `type_D4C52ar2 = {dword_0, word_4 (index), address_6, dword_10 (enabled)}`.

Creator rows of interest (strA1):
| model | EF line | address | EV case → ctor |
|---|---|---|---|
| 0x05 | EF:1709 | 0x22F570 | EV:4531 → `NewAdd0A05_4E570` ("begin of water splash") |
| 0x0D | EF:1717 | 0x22F9E0 | EV:4569 → `SetParticleSmoke3B_4E9E0` |
| 0x0E | EF:1718 | 0x22FA20 | EV:4573 → `SetParticleSmoke3C_4EA20` |
| 0x1D | EF:1733 | 0x230A00 | EV:4686 → `sub_4FA00` |
| 0x3B | EF:1763 | 0x22FB50 | EV:4586 → `ArriveCheckpoint_4EB50` ("//in quest point") |
| 0x3C | EF:1764 | 0x22FC10 | EV:4590 → `AddSmoke_4EC10` ("// 1 instance in level 9") |
| 0x57 | EF:1791 | 0x22FA60 | EV:4577 → `sub_4EA60` |
| 0x59 | EF:1793 | 0x231A20 | → `sub_50A20` (EF:37037, cave-only riser) |

Action rows of interest (strA0):
| action | EF line | address | EV case → handler |
|---|---|---|---|
| 0x05 | EF:1607 | 0x2128B0 | EV:2306 ("//end of water splash") → `AddAsh0A_05_318B0` |
| 0x0D | EF:1615 | 0x213160 | EV:2344 ("//in quest point3") → `sub_32160` |
| 0x0E | EF:1616 | 0x2132A0 | EV:2348 → `sub_322A0` |
| 0x1F | EF:1633 | 0x215350 | EV:2485 → `sub_34350` |
| 0x40 | EF:1666 | 0x2133E0 | EV:2352 ("//in quest point2") → `AddParticleSmoke0A_3B_323E0` |
| 0x41 | EF:1667 | 0x213400 | EV:2357 → `AddParticleSmoke0A_3C_32400` |
| 0x5E | EF:1696 | 0x213160 | same handler as 0x0D (`sub_32160`) |
| 0x60 | EF:1698 | 0x2121E0 | EV:2344-region case 0x2121e0 → `sub_311E0` |

Main tick dispatch (EF:40116-40180): for every live entity,
```c
if (mx->actionIndex_0x45_69 == str_D4C48ar[mx->class_0x3F_63].dword_10[mx->actionIndex_0x45_69].word_4)
    if (str_D4C48ar[...].dword_10[actionIndex].dword_10)
        pre_sub_4A190_0x6E8E(str_D4C48ar[...].dword_10[actionIndex].address_6, mx);   // EF:40171
mx->byte_0x3E_62++;
```
Universal spawner: `IfSubtypeCallCreatingManaSphere_4A190(position, type, subtype)` (EV:5186) routes through `dword_14[subtype]` (the strA1 creator) — every runtime `4A190(...,10,N)` call lands in the same ctor as a map THING of (10,N).

### 0.2 Level-load THING path `sub_4A310` (EF:32999)
```c
v11x.x = (entity->axis2d_4.x << 8) + 128;  v11x.y = (entity->axis2d_4.y << 8) + 128;
v11x.z = getTerrainAlt_10C40(&v11x);
indexx = IfSubtypeCallCreatingManaSphere_4A190(&v11x, entity->type_0x30311, entity->subtype_0x30311);  // EF:33017
```
then per-class/model post-processing (`case 0xA:` at EF:33033). Called from `sub_4A1E0` (EF:32950) for every THING whose `DisId` matches the fired disposition (dispositions re-fire during play, so THINGs can be (re)spawned mid-level; `if (a2) type_0x30311 = 0` consumes them, EF:32990-32991).

### 0.3 Stage binder `sub_58DA0` (EF:40650) — the ONLY thing THING-spawned effects feed
```c
for (i = 0; i < D41A0_0.stageIndex_0x36E01; i++)
  switch (stages_0x3654C[i].stages_3654C_byte0) {
  case 1: case 2: case 4:
    if (a1x == stages_0x3654C[i].str_36552_un.ptr0x30311)
      { stages_0x3654C[i].str_36552_un.ptr0x6E8E = a2x; str_3654D_byte1 |= 1; }  break;
  case 6:
    if (a1x == ...ptr0x30311) { ...un.dword = a2x - D41A0_0.struct_0x6E8E; byte1 |= 1; }  break;
  ...}
```
Stage records themselves come from `InitStages_58940` (EF:40567): stage types 1/2/4/6 store a pointer to a **THING** (`&terrain->entity_0x30311[stage_1]`, EF:40619-40622); when that THING spawns, `sub_58DA0` swaps in the live entity. Objective completion is evaluated in `sub_58F00_game_objectives` (EF:40693): type 1 = bound entity `life_0x8 <= -1` (EF:40763-40769); type 4 = a specific player within 768 of the bound entity's position words +76/+78 (EF:40787-40801); type 5 = player within 768 of `str_3654E_axis` (position-only, no entity, EF:40803-40813).

### 0.4 Common helpers
- `NewEvent_4A050` (EV:561): memset entity; defaults `maxLife=300; struct_byte dword=8; actSpeed=16; subSpellIndex=100; id_0x1A_26=selfIndex; xtype/xsubtype=-1; row=&str_D7BD6[59]; byte_0x43_67=10; byte_0x39_57=-6; byte_0x3E_62=selfIndex; rand_0x14_20 = selfIndex + D41A0_0.rand_0x8` (EV:578 — per-entity RNG seed). **Fallback** (EV:581-605): when the free pool is empty it **reclaims the tail of the `dword_0x11EA` list** — the list of `byte[2]&2` "reclaimable effect" entities. So `byte[2] |= 2` (set by splash/smoke ctors) marks the entity as sacrificial under entity pressure.
- `AddEventToMap_57D70` (EF:40315): inserts into the per-map-cell entity linked list, sets `byte[0]|=4`, copies position.
- `CopyMaxLifeToLife_49A20` (EF:54118): `life_0x8 = maxLife_0x4`.
- `SetEntityIndex_49C90` (EF:32830): `word_0x5A_90 = spriteIndex; animationFrame_0x5C_92 = 0; byte_0x5D_93 = x_BYTE_D8A2E[particlesParameters_D951C[idx].byte_12]` (frame cap from data table). `SetEntityIndexAndRot_49CD0` (EF:32838) and `SetHalfSpeedEntity_49DA0` (EF:32856) add the `array_0x52_82` rot/speed params from `particlesParameters_D951C[idx]`. **`word_0x5A_90` is the live sprite index** — the smoke tick mutates it directly to grow/shrink the cloud.
- `DisableEntityDrawing04_57F10` (EF:40332): `byte[1] |= 4` (mark for removal). `sub_57F20` (EV:5209) then unlinks from map, removes from the reclaim list if `byte[2]&2`, sets `class=0` and pushes the slot back to the free pool — removal is real, the slot is recycled.
- `sub_585A0` (EF:40438): `if (animationFrame_0x5C_92 < byte_0x5D_93) animationFrame++` — one-shot sprite animation advance.
- `sub_4A810_get_0x35plus` (EF:33254): `return D41A0_0.dword_0x35 + 1` — free-slot count (the emitters refuse to spawn below 32 free slots).
- RNG (everywhere): LCG `r = 9377*r + 9439`; per-entity state `rand_0x14_20`, global state `D41A0_0.rand_0x8`.

---

## 1. Model 5 (0x05) — water splash

### 1.1 Creator `NewAdd0A05_4E570` (EF:35436)
```c
type_entity_0x6E8E* NewAdd0A05_4E570(axis_3d* position)//22f570
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event)
    {
        event->maxLife_0x4 = 8;
        event->actionIndex_0x45_69 = 5;
        event->class_0x3F_63 = 10;
        event->model_0x40_64 = 5;
        event->subSpellIndex_0x2A_42 = 0;
        event->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;   // clear byte[0] bit3 (0x08) + byte[2] bit1
        event->dword_0x10_16 = 0;
        event->struct_byte_0xc_12_15.byte[2] |= 2;          // reclaimable-effect flag
        AddEventToMap_57D70(event, position);
        event->position_0x4C_76.z = getTerrainAlt_10C40(&event->position_0x4C_76); // snap to water surface
        CopyMaxLifeToLife_49A20(event);
        SetEntityIndexAndRot_49CD0(event, 244);             // sprite/particle row 244
    }
    return event;
}
```
**RNG draws: 0. Life: 8 ticks. Motion: none (pinned to terrain alt = water surface). Sprite 244.**

### 1.2 Action 5 `AddAsh0A_05_318B0` (EF:23169)
```c
void AddAsh0A_05_318B0(type_entity_0x6E8E* event)//2128b0
{
    if (event->life_0x8-- >= 0)
    {
        sub_585A0(event);                                   // animationFrame++ (up to byte_0x5D_93)
        if (!(event->struct_byte_0xc_12_15.byte[0] & 2))
        {
            event->struct_byte_0xc_12_15.byte[0] |= 2u;
            PrepareEventSound_6E450(event - D41A0_0.struct_0x6E8E, -1, 27);  // splash sound, once
        }
    }
    else
        DisableEntityDrawing04_57F10(event);
}
```
Pure one-shot: 8 ticks of frame animation at the water surface, **sound 27 on the first tick** (bit1 latch), then despawn. No RNG, no motion, no stage interaction.

### 1.3 Runtime callers of the ctor (grep `4A190(...,10, 5)`; all sites in EF, none in EV)
Common pattern everywhere: `sub_104D0_terrain_tile_is_water(&pos) == 1` → spawn (10,5), usually copy `id_0x1A_26`, then despawn the parent.
| EF line | Enclosing fn | Table slot | Context |
|---|---|---|---|
| 17131 | `sub_265A0` (0x2075A0) | class-5 action | walking mob touches water surface (`byte_0x46_70` latch value 10 = "in water"), splash at predicted pos (EF:17121-17133) |
| 21056 | `sub_2B260` (0x20C260) | class-5 action 0xE2 (str50, EF:1469) | creature death handler, `byte_0x46_70==1` branch: died over water (EF:21054-21058) |
| 21170 | `sub_2B260` | same | death case 6: sinks + splash (EF:21165-21171) |
| 23781 | `sub_32600` (0x213600) | class-10 action 0x10 (EF:1618) | falling body/object hits terrain below; if water tile → splash (inherits id) + despawn (EF:23771-23787) |
| 26516 | `sub_35FB0` (0x216FB0) | class-10 action 0x3E (EF:1664) | falling object lands on water: `word_0x2C_44=0`, splash, **spawner also plays sound 27 on the splash** (EF:26509-26520) |
| 26691 | `sub_36770` (0x217770) | helper (direct calls EF:26635/26650 from `sub_36680`) | child projectile spawned into water → splash + **sound 27** (EF:26683-26694) |
| 30082 | `sub_3A8B0` (0x21B8B0) | class-10 action 0x55 (EF:1687) | descend case 9: `z -= 32*t` until terrain; water → (10,5), land → (10,0) (EF:30073-30084) |
| 58794 | `sub_66FD0` (0x247FD0) | class-9 action 0x0C ("lighting II", EV:3322, str90 EF:1545) | bolt clamps to terrain/cave ceiling; water & model!=4 → splash (inherit id) + despawn (EF:58785-58798) |
| 62452 | `AddTree02_00_64E20` (0x245E20) | class-2 action 0 (EF:1163) | burning tree init/tile check: tree standing in water → splash + despawn (EF:62448-62456) |
| 62491 | `sub_64F60` (0x245F60) | class-2 action 1 (EF:1164) | burning tree countdown; same water check (EF:62487-62495) |
| 62509 | `sub_64FF0` (0x245FF0) | class-2 action 2 (EF:1165) | burnt-out tree; same water check (EF:62505-62513) |
| 62695 | `sub_652C0` (0x2462C0) | class-2 actions 0x14/0x15 via `sub_652A0`/`sub_65280` (EF:1183-1184) | falling debris comes to rest on a water tile (EF:62693-62698) |
| 62958 | `sub_65820` (class-9 core flight tick) | — | flyer (model not in {4,22,24,26}) crosses water → splash (inherit id) + despawn (EF:62952-62961; see class-9 doc §0.6) |
| 63143 | `sub_65C20` (0x246C20) | player-fire projectile mover (wrapped by `CastPlayerFire_65B30` EF:63006 "fire drop", `sub_65B50` EF:63023) | fire drop hits water (model!=4) → splash + despawn (EF:63137-63147) |
| 63513 | `sub_662E0` (0x2472E0) | class-9 action 8 (str90 EF:1541) | same terrain/water clamp pattern (EF:63505-63516) |

Plus map THINGs of (10,5) via `sub_4A310` → disposition default branch (model 5 hits `if (v4 != 0x9) { sub_58DA0; return; }`, EF:33052-33056 — **consumes no par1/par2/stageTag fields**).

---

## 2. Model 13 (0x0D) — smoke puff (the beacon/volcano cloud; NOT a gib)

### 2.1 Creator `SetParticleSmoke3B_4E9E0` (EF:35618) + core `SetSmoke4_4EAA0` (EF:35639)
```c
type_entity_0x6E8E* SetParticleSmoke3B_4E9E0(axis_3d* position)//22f9e0
{
    D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;                    // GLOBAL rng draw #1
    return SetSmoke4_4EAA0(position, 0xD, 0xD, 67, D41A0_0.rand_0x8 % 0x17u + 17);  // life 17..39, sprite 67
}
type_entity_0x6E8E* SetParticleSmoke3C_4EA20(axis_3d* position)//22fa20  (EF:35625 — model-14 sibling)
{
    D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;
    return SetSmoke4_4EAA0(position, 0xE, 0xE, 9, D41A0_0.rand_0x8 % 0x21u + 28);   // life 28..60, sprite 9
}
type_entity_0x6E8E* sub_4EA60(axis_3d* position)//22fa60  (EF:35632 — model-0x57 sibling, same sprite 67)
{
    D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;
    return SetSmoke4_4EAA0(position, 0x57, 0x5E, 67, D41A0_0.rand_0x8 % 0x17u + 17);
}
type_entity_0x6E8E* SetSmoke4_4EAA0(axis_3d* position, char a2, char a3, __int16 entityIndex, int a5)//22faa0
{
    type_entity_0x6E8E* tempevent = NewEvent_4A050();
    if (tempevent)
    {
        tempevent->actionIndex_0x45_69 = a3;
        tempevent->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;
        tempevent->model_0x40_64 = a2;
        tempevent->maxLife_0x4 = a5;
        tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;  // ENTITY rng draw #2
        tempevent->class_0x3F_63 = 0xA;
        tempevent->maxSpeed_0x86_134 = 30;                                // lateral drift speed
        tempevent->xtype_0x41_65 = 10;
        tempevent->xsubtype_0x42_66 = a2;
        tempevent->actSpeed_0x82_130 = tempevent->rand_0x14_20 % 0x35 + 51;  // rise speed 51..103
        tempevent->struct_byte_0xc_12_15.byte[2] |= 2;                    // reclaimable + transparent-effect list
        AddEventToMap_57D70(tempevent, position);
        SetHalfSpeedEntity_49DA0(tempevent, entityIndex);                 // sprite 67 (model 13)
        CopyMaxLifeToLife_49A20(tempevent);
    }
    return tempevent;
}
```
Note: yaw is **not** set (memset 0 by NewEvent) unless a spawner overrides it (volcano ring does, §2.3).

### 2.2 Action 0x0D `sub_32160` (EF:23572) — the rise/grow/shrink law (also action 0x5E)
```c
void sub_32160(type_entity_0x6E8E* entity)//213160
{
    if (entity->life_0x8-- < 0) { DisableEntityDrawing04_57F10(entity); return; }
    predictedAxis_EB398ar = entity->position_0x4C_76;
    entity->actSpeed_0x82_130 -= 4;                       // rise decel 4/tick
    if (entity->actSpeed_0x82_130 < 64)  entity->actSpeed_0x82_130 = 64;
    if (entity->actSpeed_0x82_130 > 128) entity->actSpeed_0x82_130 = 128;   // rise clamp [64,128]
    predictedAxis_EB398ar.z += entity->actSpeed_0x82_130;                    // RISE
    int tempAlt = getTerrainAlt_10C40(&entity->position_0x4C_76);
    if (predictedAxis_EB398ar.z < tempAlt) predictedAxis_EB398ar.z = tempAlt;
    entity->dword_0x10_16++;                              // age
    if (entity->dword_0x10_16 < 16)
    {
        MoveEntity_57FA0(&predictedAxis_EB398ar, entity->yaw_0x1C_28, 0, entity->maxSpeed_0x86_134); // drift
        entity->maxSpeed_0x86_134 -= 52;
        if (entity->maxSpeed_0x86_134 < 30)   entity->maxSpeed_0x86_134 = 30;
        if (entity->maxSpeed_0x86_134 > 1024) entity->maxSpeed_0x86_134 = 1024;
        if (!(entity->dword_0x10_16 & 1))
            if (entity->word_0x5A_90 < 74) entity->word_0x5A_90++;   // GROW: sprite 67→74 (every 2nd tick)
    }
    if (entity->life_0x8 < 6)
        if (entity->word_0x5A_90 > 67) entity->word_0x5A_90--;       // SHRINK back toward 67 near death
    CopyEntityPosition_57CF0(entity, &predictedAxis_EB398ar);
}
```
- Rise: `actSpeed` per tick, converging into [64,128] (¼–½ tile/tick).
- Drift: along `yaw` at `maxSpeed` for the first 16 ticks only; maxSpeed decays 52/tick to floor 30 (volcano ring starts at 512 → 512,460,408,…).
- Visual growth: sprite index `word_0x5A_90` walks 67→74 during the first 16 ticks (even ticks), and back down toward 67 when `life < 6`. **The cloud "vanishing" = shrink + life expiry — no fade field.** Transparency is the render-side property of the `byte[2]|=2` effect class and sprite rows 67–74 (data table, OPEN-4).
- Despawn: `life < 0` only. No water/stage checks (terrain clamp keeps it above ground).
- Sounds: none. RNG in tick: none.

Model-14 sibling action 0x0E `sub_322A0` (EF:23613): byte-for-byte the same law with sprite band **9..16** (`word_0x5A_90 < 16`/`> 9`, EF:23641/23647).

### 2.3 Runtime callers of (10,13)
1. **`AddParticleSmoke0A_3D_32420` (EF:23666)** — the emitter tick, direct call at EF:23681 (see §4).
2. **`sub_651B0` (EF:62548, class-2 action 0x12)** — scenery-prop destruction:
```c
v1 = a1x->str_0x5E_94.word_0x62_98;      // damage-event latch
if (v1) {
    v2 = a1x->life_0x8 - a1x->str_0x5E_94.dword_0x5E_94;   // apply accumulated damage
    a1x->life_0x8 = v2;
    if (v2 < 0) {
        v3 = a1x->word_0x5A_90;
        a1x->actionIndex_0x45_69 = 19;                      // → sub_65240 (idle stump)
        a1x->struct_byte_0xc_12_15.byte[0] = v4 & 0xF7;
        SetHalfSpeedEntity_49DA0(a1x, v3 + 4);              // swap to "destroyed" sprite (current+4)
        IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 13);  // EF:62570 — smoke poof
    }
    a1x->str_0x5E_94.word_0x62_98 = 0;
}
```
(One class-2 ctor using action 0x12 found: `sub_4AFE0` EF:33555 — cave plant, class 2 model 6, sprites `(rand&3)+324`.)
3. **`sub_652C0` (EF:62606, class-2 falling-debris mover, actions 0x14/0x15)** — `if (a1x->life_0x8 < 0) { IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 13); ... DisableEntityDrawing04(a1x); }` (EF:62688-62691): a settled debris chunk poofs into smoke. (Its water branch spawns (10,5) instead, EF:62693-62695. Its bounce RNG: `word_0x2C_44 = r % v7 + v7`, `actSpeed = r % (v8>>1) + 1`, `yaw = r & 0x7FF`, EF:62661-62681 — that is the *debris* behavior, class-2, not (10,13).)
4. **`sub_311E0` (EF:22860, class-10 action 0x60)** — cave terrain riser (ctor `sub_50A20` EF:37037: class 10 **model 0x59**, cave-only, `life=40`, action 0x60). When its column height passes the threshold it emits a **ring of 74 clouds**:
```c
if (!a1x->word_0x36_54 && a1x->word_0x2C_44 > 455) {
    a1x->word_0x36_54 = 1;
    for (k = 0; k < 2048; k += 28) {                       // yaw ring, 2048/28 ≈ 73 clouds
        v24 = (v39 << 8) - 768; clamp [256, 0x2000];
        predictedAxis_EB398ar = a1x->position_0x4C_76;
        MoveEntity_57FA0(&predictedAxis_EB398ar, k, 0, v24);
        v2x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 13);   // EF:23073
        if (v2x) { v2x->yaw_0x1C_28 = k; v2x->maxSpeed_0x86_134 = 512;                 // fast outward drift
                   predictedAxis_EB398ar.z = 32 * mapHeightmap_11B4E0[v25];
                   CopyEntityPosition_57CF0(v2x, &predictedAxis_EB398ar); }
    }
}
```
(EF:23061-23083.)
5. Map THINGs (10,13) via `sub_4A310` → disposition **default branch** (model 0xD hits `if (v4 < 0x11u) { if (v4 != 0xF) { sub_58DA0; return; } }`, EF:33060-33066 — no par consumption).

---

## 3. Model 29 (0x1D) — stage/quest marker (one-tick, invisible)

### 3.1 Creator `sub_4FA00` (EF:36274)
```c
type_entity_0x6E8E* sub_4FA00(axis_3d* position)//230a00
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event)
    {
        event->maxLife_0x4 = 0;
        event->actionIndex_0x45_69 = 0x1F;
        event->class_0x3F_63 = 0xA;
        event->model_0x40_64 = 0x1D;
        event->position_0x4C_76 = *position;
        event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;
        AddEventToMap_57D70(event, position);
        CopyMaxLifeToLife_49A20(event);       // life = 0
    }
    return event;
}
```
**No sprite is ever assigned** (no `SetEntityIndex*` call) — the entity is invisible. No RNG, no sound, no mana, maxLife/life = 0.

### 3.2 Action 0x1F `sub_34350` (EF:24996)
```c
void sub_34350(type_entity_0x6E8E* a1x)//215350
{
    DisableEntityDrawing04_57F10(a1x);
}
```
First tick → marked for removal → `sub_57F20` frees the slot. **Total lifetime: one tick.** It never despawns "in reaction" to anything — it always despawns immediately; it never reads stage variables (`StageVar1/2` are only assigned on class-5 mobs via the EF:5016 path).

Sibling markers for context (same one-shot pattern): model 0x1E = `AddPointToPath_4F9A0` (EV:4682 "2 instances in level 1") whose action `ApplyPointToPath_343F0` (EF:25027) stamps path bits into `mapAngle_13B4E0` then dies; models 0x1F/0x20 (ctors `sub_4FAC0` EF:36311 / `sub_4FA60` EF:36292, actions 0x21/0x22 = `sub_34390`/`sub_343C0` EF:25003/25015) probe the map cell for a class-14 model-1/2 entity via `sub_5B070` (EF:42497) and set its `life = 2`/`1`, then die — door/trigger pokers.

### 3.3 Level-load/disposition path (the exact v4 case)
In `sub_4A310` `case 0xA` (EF:33033): `v4 = 0x1D` → `v4 < 0x22` (EF:33035) → `v4 > 0xB` (EF:33058) → not `< 0x11` → `else if (v4 > 0x11 && v4 != 0x16) { sub_58DA0(entity, v3x); return; }` (EF:33068-33072).
**Model 29 consumes NO par1/par2/word_10/stageTag fields** — the only side effect is `sub_58DA0` stage binding (§0.3): if a stage record of type 1/2/4/6 points at this THING, the freshly created entity (or its index) is captured and `str_3654D_byte1 |= 1`.

### 3.4 Runtime callers
`grep sub_4FA00` → only EV:4687 (creator dispatch). `grep 4A190(...,10, 29 / 0x1D)` → none. **The ctor is reachable exclusively through map THINGs (10,29) via `sub_4A310`/dispositions.** No code path checks `model_0x40_64 == 0x1D` anywhere in EF/EV.

### 3.5 Reconciling with the playtest ("(10,29) = quest-beacon smoke column")
In this decompile a (10,29) THING produces **no visual at all** — it exists to donate its position/identity to a stage record for one tick. The visible homing-beacon smoke column is a **separate THING**, (10,0x3B), placed at quest points (remc2 author's comments: EV:4586 "in quest point", EV:2352 "in quest point2", EV:2344 "in quest point3" = the three links of the chain in §4). A caution for the binder semantics: stage type 4 reads the bound entity's position words +76/+78 *after* the marker's slot has been freed and possibly reused (memset by `NewEvent_4A050`) — how the original tolerates this is not answerable from this source (OPEN-2).

---

## 4. The smoke column chain (gameplay-critical beacon) — models 0x3B/0x3C → clouds 13/14

### 4.1 Emitter ctors
```c
type_entity_0x6E8E* ArriveCheckpoint_4EB50(axis_3d* position)//22fb50   (EF:35663, quest beacon)
{
    if (sub_4A810_get_0x35plus() < 32) return 0;            // entity budget gate
    tempevent = NewEvent_4A050();  if (!tempevent) return 0;
    tempevent->actionIndex_0x45_69 = 0x40;
    tempevent->class_0x3F_63 = 0xA;
    tempevent->model_0x40_64 = 0x3B;
    tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;
    tempevent->maxLife_0x4 = tempevent->rand_0x14_20 % 0x64u + 800;     // 800..899 ticks
    tempevent->struct_byte_0xc_12_15.byte[0] = (tempevent->struct_byte_0xc_12_15.byte[0] & 0xF6) | 1;
    tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;
    tempevent->actSpeed_0x82_130 = tempevent->rand_0x14_20 % 0x11u;     // 0..16 extra rise for its clouds
    tempevent->position_0x4C_76 = *position;                // NOT AddEventToMap — off the cell lists
    CopyMaxLifeToLife_49A20(tempevent);
    return tempevent;
}
```
`AddSmoke_4EC10` (EF:35685, volcano smoke, "1 instance in level 9") is **identical** except `actionIndex = 0x41; model = 0x3C`. Neither assigns a sprite — the emitters are invisible; only their clouds render.

### 4.2 Emitter tick (action 0x40/0x41 → shared body)
```c
void AddParticleSmoke0A_3B_323E0(type_entity_0x6E8E* event)//2133e0  { AddParticleSmoke0A_3D_32420(event); }   // EF:23654
void AddParticleSmoke0A_3C_32400(type_entity_0x6E8E* event)//213400  { AddParticleSmoke0A_3D_32420(event); }   // EF:23660
void AddParticleSmoke0A_3D_32420(type_entity_0x6E8E* event)//213420   // EF:23666
{
    type_entity_0x6E8E* tempentity = 0;
    if (event->life_0x8-- < 0) { DisableEntityDrawing04_57F10(event); return; }   // emitter expires!
    axis_3d position = event->position_0x4C_76;
    event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
    position.x += event->rand_0x14_20 % 0xA0u;              // +0..159 X jitter (positive-only)
    event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
    position.z += event->rand_0x14_20 % 0xA0u;              // +0..159 ALTITUDE jitter
    if (event->model_0x40_64 == 0x3Bu)
        tempentity = SetParticleSmoke3B_4E9E0(&position);   // cloud = (10,13), sprite 67  — EF:23681
    else if (event->model_0x40_64 == 0x3Cu)
        tempentity = SetParticleSmoke3C_4EA20(&position);   // cloud = (10,14), sprite 9
    if (tempentity)
    {
        event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
        tempentity->life_0x8 = 32;
        tempentity->maxLife_0x4 = 32;                       // OVERRIDES SetSmoke4's random life → 32 ticks
        tempentity->actSpeed_0x82_130 += event->actSpeed_0x82_130 + (event->rand_0x14_20 % 0x4Du);  // +0..16 +0..76
    }
}
```
**Cadence: one cloud per tick, every tick**, until the emitter's 800-899-tick life runs out (then it silently dies; dispositions re-fire it — §0.2). Per-cloud RNG draw order: (1) emitter x-jitter `%0xA0`, (2) emitter z-jitter `%0xA0`, (3) global life draw `%0x17+17` (discarded — overridden to 32), (4) cloud actSpeed draw `%0x35+51`, (5) emitter actSpeed-bonus draw `%0x4D`. Net cloud rise speed = `51..103 + 0..16 + 0..76` clamped to [64,128] from the first tick. 32-tick life, grow 67→74 for 16 ticks, shrink near death (§2.2). No sounds anywhere in the chain.

### 4.3 Third puff variant (10,0x57)
`sub_4EA60` (EF:35632) creates model 0x57, action 0x5E (= same `sub_32160` law, sprite 67, life `%0x17+17` — not overridden). Sole runtime spawner: `sub_31120` (EF:22826, class-10 action 0x5D): once, when `maxLife-5 == life`, at terrain level — plus a 50% chance sound `word_0x5A_90 - 282` (EF:22839-22845).

---

## 5. Consolidated constants

| Item | Value | Cite |
|---|---|---|
| RNG (all) | `r = 9377*r + 9439` (per-entity `rand_0x14_20`; global `D41A0_0.rand_0x8`) | EV:578, EF:35620 |
| (10,5) splash: action/model | 5 / 5 | EF:35441-35444 |
| (10,5) life | 8 ticks, no motion, z = terrain alt | EF:35441, 35450 |
| (10,5) sprite | `SetEntityIndexAndRot_49CD0(244)` | EF:35452 |
| (10,5) sound | **27**, once on first tick (bit-latch `byte[0]&2`); also played by spawners at EF:26518, 26693 | EF:23177 |
| (10,5) RNG | none | — |
| (10,13) cloud: action/model | 0xD / 0xD | EF:35621 |
| (10,13) sprite band | 67 base; grows to 74 (even ticks, age<16), shrinks to 67 (life<6) | EF:35621, 23600-23607 |
| (10,13) life | `glob%0x17 + 17` = 17..39 (beacon emitter overrides to **32**) | EF:35621, 23687-23688 |
| (10,13) rise speed | init `ent%0x35 + 51` = 51..103 (+emitter bonus), per tick `-4`, clamp **[64,128]**; z += actSpeed | EF:35653, 23580-23585 |
| (10,13) drift | along yaw (default 0) at `maxSpeed` 30 (volcano ring: 512), decay 52/tick floor 30, first 16 ticks only | EF:35650, 23592-23597 |
| (10,13) sounds | none | — |
| (10,14) cloud (volcano) | action/model 0xE/0xE, sprite band **9..16**, life `%0x21+28` = 28..60 | EF:35628, 23641-23648 |
| (10,0x57) cloud | action 0x5E (same law), sprite 67, life `%0x17+17` | EF:35635 |
| (10,29): action/model | 0x1F / 0x1D, maxLife 0, **no sprite**, lives 1 tick | EF:36279-36286, 24996-25000 |
| (10,29) disposition | default branch, no par fields; `sub_58DA0` stage-bind only | EF:33068-33072 |
| (10,0x3B) beacon emitter | action 0x40, life `%0x64+800` = 800..899, actSpeed `%0x11` = 0..16, invisible, gate: ≥32 free slots | EF:35663-35682 |
| (10,0x3C) volcano emitter | action 0x41, same numbers, clouds = (10,14) | EF:35685-35704 |
| Emitter cadence | 1 cloud **per tick**; x jitter `+ r%0xA0`, z(altitude) jitter `+ r%0xA0`; cloud life forced 32; `actSpeed += emitter.actSpeed + r%0x4D` | EF:23670-23690 |
| Volcano ring (model 0x59, cave) | at `word_0x2C_44 > 455`: yaw k = 0,28,…<2048 (73 clouds), radius `clamp((v39<<8)-768, 256, 0x2000)`, cloud yaw=k, maxSpeed=512, z = 32*heightmap | EF:23061-23083, 37037-37054 |
| Prop destruction (class-2 action 0x12) | damage latch `word_0x62_98`; on `life-dmg < 0`: sprite `+= 4`, action→19, spawn (10,13) | EF:62556-62572 |
| `byte[2] |= 2` | reclaimable-effect flag; NewEvent steals from this list when pool empty | EV:581-605, 5215-5235 |
| Removal | `DisableEntityDrawing04` = `byte[1]|=4`; `sub_57F20` unlinks + frees slot (class=0) | EF:40332, EV:5209-5239 |

## OPEN items
1. **(10,29) vs the playtest observation.** In remc2, (10,29) is invisible and one-tick; the beacon column is (10,0x3B)→(10,13). Verify against level-000 THING data (mgc-import) whether the observed smoke position carries a (10,0x3B)/(10,0x3C) THING alongside/instead of (10,29), and against original-binary behavior of creator 0x230A00/action 0x215350 (the decompile bodies are trivially small — plausible but worth a retail check given the stakes: it's the stage-goal marker).
2. **Stage-binding dangling pointer:** stage types 1/2/4 capture the (10,29) entity pointer (`sub_58DA0`), but the entity slot is freed on tick 1 and recycled by `NewEvent_4A050` (memset). How objective checks (EF:40763-40801) behave against a recycled slot is undefined from this source. The port should bind the THING **position/identity**, not the transient entity.
3. **Emitter death vs. permanent beacons:** the (10,0x3B) emitter lives 800-899 ticks and is not self-respawning; persistence must come from disposition re-firing (`sub_4A1E0`, EF:32950) or original data placing long-lived dispositions. Confirm respawn cadence against recorded gameplay.
4. **Sprite/particle data:** rows 244 (splash), 67-74 and 9-16 (smoke bands) index `particlesParameters_D951C[]`; frame counts come from `x_BYTE_D8A2E[...byte_12]` (EF:32830-32834). Transparency of the smoke sprites is data/renderer-side — extract from the particle table separately.
5. **Registration site of the `dword_0x11EA` reclaim list** (who appends `byte[2]&2` entities) was not located in the read window; only consumption (EV:581) and unlinking (EV:5230-5234) were traced.
6. **Class-2 debris origin:** which damage handler flips props into falling-debris actions 0x14/0x15 (`sub_652C0`) was not traced (out of scope); candidates include `sub_69250` (EF:5243, respawn-via-creator `actionIndex += 2`).
