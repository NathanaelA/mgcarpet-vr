# CLASS-10 tail helper closure — (10,67) flood finisher, (10,71) fissure stamp, (10,22) whirlwind passes, sub_5C800 palette FX

Closes OPEN items 4, 5, and 8 from `mc2-class10-m50-chains-and-tail.md` (the "m50-doc"). All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, Terrain = `Terrain.cpp`). Trace date 2026-07-10.

Shared primitives already documented in the m50-doc / companion docs — **not** re-derived here: `NewEvent_4A050`, `AddEventToMap_57D70`, `sub_10C80` (AoE damage), RNG law `r = 9377*r + 9439`, `AddE7EE0x_10080`/`sub_10130` (radius-disc cell iterator; call `AddE7EE0x_10080(0, radius)` → repeatedly `sub_10130(handle,&dx,&dy)` yields cell offsets → `ResetEvent08_10100(handle)`). New shared helpers used below are documented inline in §5.

**Numbering reminder (from m50-doc):** prompt subtypes are DECIMAL. (10,67)=model 0x43 action 0x48; (10,71)=model 0x47 action 0x4E; (10,22)=model 0x16 action 0x16. The flood machine hands off to actionIndex **73 (0x49)** then **74 (0x4A)** — those are the finisher handlers `sub_396A0`/`sub_396D0`, confirmed from the strA0 table at EF:1675-1676 (`0x0049 → 0x0021A6A0 → sub_396A0`, `0x004A → 0x0021A6D0 → sub_396D0`).

---

## Headline findings (read first)

1. **(10,67)=0x43 flood/quake** is a **terrain-morphing "rising ground" effect**, not a damage burst. `sub_39040` (action 0x48) melts/raises the 30×30-tile heightmap toward a dome centered on the entity over a 12-step countdown, converting burnable terrain to lava-type 1, and — via `sub_39B60` — physically **shoves/lifts entities** out of the affected disc (and on the finisher pass, tags/lifts them for damage via `sub_3A200`). It does NOT call `sub_10C80`. Damage is dealt only indirectly through the mailbox writes of `sub_3A090`/`sub_3A200`. The life<=0 exit jumps straight to action **74** (`sub_396D0`, the ground-restoration finisher); the normal phase 3 path routes action 0x48→73 (`sub_396A0`, a life-countdown that then hands to 74).

2. **(10,71)=0x47 fissure** `sub_3A2D0` (action 0x4E) is a **pure heightmap displacer + periodic AoE**. Per affected cell it writes **±1 to `mapHeightmap_11B4E0`** (sign = life parity), producing a jittering trench/ridge; it does NOT raise a lava terrain-type and spawns no children. Every 4th tick (`life & 3 == 0`) it grows the sprite, plays sound 10, and fires **`sub_10C80(self, 0, subSpellIndex)`** for area damage (type-0 mask), reporting to the spellbook (id 0xF). The radius `v6` ramps up in the middle third of life and down at the ends, clamped to `[0, min(3*word_0x2C_44, 15)]`.

3. **(10,22)=0x16 whirlwind** has **two damage/pickup passes per tick**: `sub_33340` = the **lift-and-throw** pass (picks entities up in a radius-12 disc, spins them around the eye, drops them; deals damage via `sub_11900` while airborne), and `sub_33710` = the **contact/fireball-and-scenery** pass (every 8 ticks: damages overlapping fireballs via `sub_11900`, and knocks scenery model-2 objects). Both use the head's `subSpellIndex = 1000`. `sub_338D0` = teardown (clears the "grabbed" flags on every entity in the disc, then despawns the whole 12-node chain). It does **not** apply subSpellIndex=1000 as a subSpellIndex swap — 1000 is the head's own damage amount fed to `sub_11900`.

4. **`sub_5C800(entity, N)`** (EF:43576) is a **full-screen palette-effect trigger**, gated to the LOCAL player only. It sets `x_D41A0_BYTEARRAY_4_struct.paletteSubMod_180 = N`; the palette state-machine (EF:3, case-3 switch at EF:31937) reads that code next frame and re-tints the VGA palette. **Code 6 = a blue/cyan "cool" flash** (red←blue+48, green+32, blue+32) — the portal warp feedback. No sound, no gameplay state; purely a viewer-side screen tint.

---

## 1. (10,67)=0x43 — flood/quake completion (`sub_39040` phases 3+, `sub_39E40`, action-74 finisher)

### 1.0 The full state graph
```
action 0x48 sub_39040   byte_0x46_70 phase 0 → sub_39E40 probe → phase 1
                        phase 1 → 18×18 terrain sample, sound 64, arm dword_0x10_16=12 → phase 2 (fallthrough)
                        phase 2 → the 30×30 dome-morph loop; when countdown dword_0x10_16<=0 → phase 3
                        phase 3 → convert 30×30 lava cells + AddBuildingToTerrain + set action 73
   at life<=0 (any phase, top of sub_39040) → action 74, byte_0x46_70=0   [SHORTCUT to finisher]
action 0x49 sub_396A0   life-countdown; while life>0 calls sub_39B60 (entity shove); at life<=0 → action 74, byte=0
action 0x4A sub_396D0   the RESTORE finisher: re-lifts entities (sub_3A090 + sub_39B60 in action-74 mode),
                        settles heightmap back toward sub_439A0, then despawns
```
So there are TWO ways to reach action 74: the natural phase-3→73→74 route, and the life<=0 shortcut at the very top of `sub_39040`. Both land in `sub_396D0`.

### 1.1 `sub_39E40` init probe (EF:29133) — verbatim
```c
char sub_39E40(type_entity_0x6E8E* a1x)//21ae40
{
    v1 = (unsigned __int16)(a1x->position_0x4C_76.x + 128) >> 8;    // center tile X
    v2 = 0;
    HIBYTE(v4) = ((unsigned __int16)(a1x->position_0x4C_76.y + 128) >> 8) - 15;   // scan Y from -15
    while (v3 < 30) {                                  // 30×30 tile window
        LOBYTE(v4) = v1 - 15;
        while (v5 < 30) {
            if (!mapTerrainType_10B4E0[v4]) v2++;      // count terrain-type-0 (open ground) cells
            LOBYTE(v4)++;
        }
        ++HIBYTE(v4);
    }
    if (v2 >= 225)                                     // ≥225/900 open → ABORT (too much open ground)
        return 0;

    v6 = v1 - 27;
    HIBYTE(v8) = (...y...) - 27;                        // wider 54×54 tile window
    do {
        for (LOBYTE(v8) = v6; v9 < 54; LOBYTE(v8)++) {
            for (i = mapEntityIndex_15B4E0[v8]; ; i = v13x->oldMapEntity_0x16_22) {
                v13x = Entities_EA3E4[i];
                if (v13x == Entities_EA3E4[0]) break;
                if (v13x != a1x && v13x->class_0x3F_63 == 10) {
                    v11 = v13x->model_0x40_64;
                    if (v11 >= 0x2D) {
                        if (v11 <= 0x2D) {              // model 0x2D
                            v12 = v13x->actionIndex_0x45_69;
                            if (v12 == 48 || v12 == 51) return 0;   // another 0x2D flood/quake nearby → ABORT
                        }
                        else if (v11 == 67)             // another (10,67)=0x43 nearby → ABORT
                            return 0;
                    }
                }
            }
        }
    } while (v7 < 54);
    return 1;                                           // OK to proceed
}
```
**Semantics:** the flood refuses to start (`return 0` → creator's phase-0 despawns it) if the 30×30 footprint is mostly open ground (≥225 type-0 cells), OR if another flood/quake entity (class-10 model 0x2D in action 48/51, or another model-67) is already active within a 54×54 window. Prevents overlapping quakes.

### 1.2 `sub_39040` — phases 0,1,2 (already summarized in m50-doc §5.5, transcribed here for continuity)
Top of function (runs EVERY tick, before the phase switch):
```c
v54 = 0;
v2 = a1x->life_0x8 - 1;
x_DWORD_E9B90 = 0;                       // global "objects hit this tick" counter, reset
a1x->life_0x8 = v2;
if (v2 <= 0) { a1x->actionIndex_0x45_69 = 74; a1x->byte_0x46_70 = 0; }   // <-- shortcut to finisher
else {
    v3 = (pos.x + 128) >> 8;  v1 = (pos.y + 128) >> 8;   // center tile
    v53x = (v3-15, v1-15);   v52x = (v3, v1);             // window origin / center
    v4 = a1x->byte_0x46_70;
    switch (v4) {
      case 0: v1 = sub_39E40(a1x);
              if (v1) a1x->byte_0x46_70 = 1; else DisableEntityDrawing04_57F10(a1x); break;
      case 1: v5 = GetTerrainHeightFromSquare_48DF0(v3-9, v1-9, 18, 18);   // sample 18×18 max height
              a1x->position_0x4C_76.z = 0;  a1x->word_0x2C_44 = 0;
              if (v5 > 64) { a1x->position_0x4C_76.z = v5 - 64;
                             if (v5-64 > 16) a1x->word_0x2C_44 = 32*(v5-80); }
              a1x->mana_0x90_144 = 0;  a1x->byte_0x46_70 = 2;  a1x->dword_0x10_16 = 12;   // arm 12-step
              PrepareEventSound_6E450(..., 64);          // sound 64
              goto LABEL_11;   // fall into phase 2
      case 2: LABEL_11: ...    // the dome-morph loop, see §1.3
```

### 1.3 Phase 2 dome-morph loop (the core, EF:28553-28701) — verbatim
```c
case 2u:
LABEL_11:
    v6 = a1x->dword_0x10_16 - 1;  a1x->dword_0x10_16 = v6;   // step down the 12-countdown
    if (v6 <= 0) { a1x->byte_0x46_70 = 3; }                  // done → phase 3
    else {
        // ---- 30×30 tile sweep, y = v53.y .. +30, x = v53.x .. +30 (window centered on entity) ----
        for (v8x.y = v53.y; v7 < 30; ++v8x.y) {
            for (i = 0; i < 30; i++) {
                v40x = (v8x.x<<8, v8x.y<<8, -);
                v9 = EuclideanDistXYZ_58490(&pos, &v40x);      // 3D distance center→cell
                v48 = v9;
                if (v9 < 3840) {                               // inside 15-tile radius
                    if (v9 >= 2304) {                          // OUTER ring (9..15 tiles): fall toward rim
                        v10 = tan2(&pos, &v40x);
                        v42x = pos;  MoveEntity_57FA0(&v42x, v10, 0, 3840);   // point on rim
                        v11 = (int16)getTerrainAlt_10C40(&v42x) >> 5;         // rim terrain height
                        if (a1x->word_0x2C_44 < v11) a1x->word_0x2C_44 = v11;
                        // blend target height between rim and dome using a sin() falloff
                        v50 = v11 - (((0x10000 + sin_DB750[0x200 + ((v48-2304)<<10)/1536]) >> 1)
                                     * (v11 - (pos.z + 64)) >> 16);
                    } else {                                   // INNER disc (<9 tiles): raise to dome top
                        v50 = pos.z + 64
                              - ((0x10000 - sin_DB750[0x200 + ((2304 - v9)<<9)/2304]) << 6 >> 16);
                    }
                    // ease the cell's current height toward v50 by 1/countdown each tick
                    v12 = (v50 - mapHeightmap_11B4E0[v8x.word]) / a1x->dword_0x10_16
                          + mapHeightmap_11B4E0[v8x.word];
                    if (v12 < 1) v12 = 1;   if (v12 > 255) v12 = 255;
                    mapHeightmap_11B4E0[v8x.word] = v12;       // <-- HEIGHTMAP WRITE (raise/settle)
                    if (isCaveLevel_D41B6) {                   // cave: ease the CEILING heightmap too
                        v50 = min(v12 + 64, 254);
                        x_BYTE_14B4E0_second_heightmap[v8x.word]
                            = second - (second - v50) / a1x->dword_0x10_16;
                    }
                    // convert mid-slope burnable terrain to lava-type 1
                    v16 = pos.z;
                    if (v12 <= v16+64 && v12 >= v16 + 6*a1x->dword_0x10_16 && sub_57450(mapTerrainType_10B4E0[v8x.word])) {
                        v54 = 1;
                        mapTerrainType_10B4E0[v8x.word] = 1;                       // TERRAIN-TYPE WRITE (lava)
                        mapAngle_13B4E0[v8x.word] = (mapAngle & 0xF8) | 1;
                    }
                }
                // ---- recompute shading for this cell from the height gradient ----
                ... v20 = height(cell) - height(cell-1,-1) + 32; clamp bands; MapType flip;
                mapShading_12B4E0[v8x.word] = v21;
                if (isCaveLevel_D41B6) { set/clear mapAngle bit8 from second-heightmap vs heightmap; }
            }
        }
        if (a1x->dword_0x10_16 == 5) { sub_3A090(a1x); v54 = 1; }   // at step 5: the entity-damage pass
        if (v54) { sub_462A0(v52-15, v52+15); }                    // rebuild terrain/lighting mesh 30×30
        // ---- 2×2 center cells: drop the very center back down (the crater floor) ----
        for (v25=0; v25<2; ...) for (v46=0; v46<2; ...) {
            v27 = h - h/countdown;  clamp[0,255];  mapHeightmap[center] = v27;
            mapShading[center] = (MapType!=Day ? -31 : 31)/countdown + 32;
        }
    }
    if (a1x->dword_0x10_16 < 6) sub_39B60(a1x);   // <-- entity SHOVE pass every tick once countdown<6
    break;
```
**Phase-2 semantics:**
- Uses a `sin_DB750` LUT to raise the inner disc into a **dome** and blend the outer ring down to the terrain rim, easing `1/dword_0x10_16` per tick (so it converges over the 12→1 countdown).
- Writes: `mapHeightmap_11B4E0` (raise), `mapTerrainType_10B4E0=1` + `mapAngle` (lava conversion of burnable cells via the `sub_57450` predicate), `mapShading_12B4E0` (relit), plus cave ceiling `x_BYTE_14B4E0_second_heightmap`.
- **Entity damage is `sub_3A090` at countdown step 5** (one-shot) and **entity shove is `sub_39B60` every tick once countdown < 6** — no `sub_10C80`.
- `sub_462A0(min,max)` = rebuild the terrain render mesh for the 30×30 rect (visual commit of the height/terrain writes).

### 1.4 Phase 3 (case 3u, EF:28702-28751) — verbatim
```c
case 3u:
    // full 30×30: force every burnable cell (or terrain-type 8) to lava-type 1
    for (v31 = 0; v31 < 30; v31++, v30x.y++) {
        for (v32 = 0; v32 < 30; v32++, v30x.x++) {
            if (sub_57450(mapTerrainType_10B4E0[v30x.word]) || mapTerrainType_10B4E0[v30x.word] == 8) {
                mapTerrainType_10B4E0[v30x.word] = 1;
                mapAngle_13B4E0[v30x.word] = (mapAngle & 0xF8) | 1;
            }
        }
    }
    AddBuildingToTerrain_46570(v52-15, v52+15);   // re-integrate any building tiles in the rect
    // 2×2 center cells: force shading to 63 (Day) / 1 (night)
    for (v36=0; v36<2; ...) for (v37=0; v37<2; ...) {
        v45 = (MapType != Day) ? 1 : 63;
        mapShading_12B4E0[center] = v45;
    }
    sub_39B60(a1x);                      // final entity shove
    a1x->actionIndex_0x45_69 = 73;       // → sub_396A0
    break;
```
Phase 3 = the "finalize the crater" pass: everything burnable in the disc becomes lava, buildings re-stamped, center relit, then hands to action **73**.

### 1.5 Action 73 `sub_396A0` (EF:28764) — verbatim (the intermediate)
```c
void sub_396A0(type_entity_0x6E8E* a1x)//21a6a0
{
    v2 = a1x->life_0x8 - 1;  a1x->life_0x8 = v2;
    if (v2 > 0) { sub_39B60(a1x); return; }       // keep shoving entities while life remains
    a1x->actionIndex_0x45_69 = 74;  a1x->byte_0x46_70 = 0;   // → the restore finisher
}
```
A pure life-countdown that continues the entity-shove (`sub_39B60`) each tick, then hands to action 74.

### 1.6 ACTION-74 (0x4A) finisher `sub_396D0` (EF:28783) — verbatim (the RESTORE pass)
This is a second phase-machine on `byte_0x46_70` that **undoes** the crater — settling the heightmap back to natural terrain and lifting/tagging entities for the coup-de-grace.
```c
unsigned __int8 sub_396D0(type_entity_0x6E8E* a1x)//21a6d0
{
    v38 = centerY;  BYTE1(v37) = centerY - 15;  v39 = centerX;  LOBYTE(v37) = centerX - 15;   // window
    resulty = a1x->byte_0x46_70;

    if (resulty < 1) {                     // ---- phase 0 of the finisher (first tick) ----
        sub_39B60(a1x);                    // shove entities
        sub_3A090(a1x);                    // damage/tag entities (see §1.7)
        a1x->byte_0x46_70 = 1;
        a1x->dword_0x10_16 = 16;           // arm a 16-step restore countdown
        a1x->position_0x4C_76.z += 64;
        // 30×30: convert remaining burnable cells to lava-type 1
        for (v4=0; v4<30; v4++) for (i=0; i<30; i++)
            if (sub_57450(mapTerrainType_10B4E0[v6])) {
                mapTerrainType_10B4E0[v6] = 1;
                mapAngle_13B4E0[v6] = (mapAngle & 0xF8) | 1;
            }
        sub_462A0(v39-15..+15, v38-15..+15);       // relight rect
        PrepareEventSound_6E450(..., 64);          // sound 64 again
    }
    else if (resulty > 1) {
        if (resulty == 2) {                // ---- phase 2 of finisher: SETTLE heightmap & release grabs ----
            for (v22=0; v22<30; v22++) for (v23=0; v23<30; v23++)
                mapHeightmap_11B4E0[cell] = sub_439A0(cell);      // restore height from neighbours
            // release every entity this flood had grabbed (byte[2]&0x10, word_0x26==self)
            for (jx = dword_38519; jx > Entities[0]; jx = jx->next_0)
                if (jx->model==2 && (jx->byte[2] & 0x10) && jx->word_0x26_38 == self) {
                    jx->word_0x26_38 = 0;  jx->byte[2] &= 0xEF;
                }
            DisableEntityDrawing04_57F10(a1x);        // DESPAWN
        }
        return resulty;
    }
    // ---- phases 0 and 1 fall through here: every 4th tick, ease the dome back down ----
    if (!(a1x->life_0x8 & 3)) {
        v9 = a1x->dword_0x10_16 - 1;  a1x->dword_0x10_16 = v9;
        if (v9 <= 0) a1x->byte_0x46_70 = 2;           // countdown done → phase 2 (settle+despawn)
        else {
            for (v35=0; v35<30; v35++) for (k=0; k<30; k++) {
                v36 = EuclideanDistXYZ_58490(&pos, &cellpos);
                if (v36 < 3840) {                     // inside disc: ease height toward the natural rim
                    v11 = tan2(&pos, &cellpos);  v28x = pos;  MoveEntity(&v28x, v11, 0, 3840);
                    v12 = getTerrainAlt_10C40(&v28x);
                    v13 = (v12>>5) - (((0x10000 + sin_DB750[...]) >> 1) * ((v12>>5) - pos.z) >> 16);
                    a1x->rand = 9377*rand + 9439;
                    v14 = (rand & 3) + v13 - 2;        // + small random jitter
                    v34 = h + (v14 - h)/dword_0x10_16;  clamp[1,255];
                    mapHeightmap_11B4E0[cell] = v34;
                    if (dword_0x10_16 < 3) mapHeightmap_11B4E0[cell] = sub_439A0(cell);   // final snap
                    if (isCaveLevel_D41B6) ease second-heightmap toward height+64;
                }
                // recompute shading for the cell (same gradient formula as phase 2)
                mapShading_12B4E0[cell] = ...;
            }
        }
    }
    return resulty;
}
```
**Finisher semantics:**
- Phase 0: shove (`sub_39B60` in action-74 mode) + damage (`sub_3A090`) + finish lava conversion + sound 64, arm 16-step restore.
- Phases 0/1 (every 4th tick): ease the crater dome back DOWN toward natural terrain (`getTerrainAlt` rim + `sin` falloff + random jitter), snapping to `sub_439A0` (neighbour-averaged height) in the last 3 steps.
- Phase 2: fully settle the 30×30 heightmap to `sub_439A0`, **release all grabbed model-2 objects** (clears byte[2] bit4 and the `word_0x26_38` owner tag), then **despawn**.

### 1.7 The two entity passes the flood uses (shared with the finisher)

**`sub_3A090` (EF:29316) — the damage/grab pass.** Verbatim behavior:
- Iterates the "effect" list `dword_38527` (`CompareAxisWithShift_10750` overlap): sets `life=-1` (kills overlapping effect entities).
- Iterates the object list `dword_38519`: any **model-2 object** overlapping gets grabbed — sets byte[2] bit4 (0x10), `word_0x30_48 = 30`, `word_0x26_38 = self index`, **adds `subSpellIndex` to its damage mailbox `str_0x5E_94.dword_0x5E_94`**, `word_0x62_98 = self.id`, bumps a hit counter `v8 += 2`, and increments the global `x_DWORD_E9B90`.
- 30×30 tile sweep: cells whose terrain-type has bit `0x7F0000` in `sub_10590_terrain_tile_type` → forced to lava-type 1.
- If any objects were grabbed → `sub_6D8B0(id, 0x14, v8)` spellbook report (effect id 0x14 = 20).

**`sub_3A200(src, victim)` (EF:29382) — the per-entity shove callback** (invoked by `sub_39B60` for close entities): sets victim `byte[c..f] |= 0x100001` (airborne/grabbed flags); for class-5 model-0x12/0x27 and class-3 player it sets pitch/roll spin; on a `rand%7==0` roll (or if flagged) and the victim has `byte_0x38_56 & 1`, **adds `life+1` to the victim's damage mailbox** and reports `sub_6D8B0(id, 0x14, 1)`.

**`sub_39B60(src)` (EF:29011) — the radius entity-shove** (called every tick in phases 2/3/73 and the finisher): sweeps a 26×26 tile disc (`EuclideanDistXY < 0xA90000`); for each entity passing the `sub_39FA0` filter it pushes it radially outward from the center (force `((3328-dist)<<8)/3328 <<7 >>8`, clamped [4,128]) and lifts/clamps its z. In **action-74 mode** (`actionIndex==74`) it additionally, for grabbed entities (byte[2]&0x10): re-enables local-player visibility (byte[0] bit0) or hides others, and clears the grab bit — i.e. it releases the pickup as the crater settles.

**Damage delivery for the whole flood** = mailbox writes (`sub_3A090`/`sub_3A200` add to `str_0x5E_94.dword_0x5E_94`); the victim's own action reads and applies it. No `sub_10C80`.

---

## 2. (10,71)=0x47 — fissure completion (`sub_3A2D0` per-cell stamp)

Full verbatim function at EF:29443. The per-cell stamp loop (EF:29527-29582) resolved:

### 2.1 Init (phase 0, byte_0x46_70 == 0)
```c
a1x->word_0x2C_44   = a1x->maxLife_0x4 >> 3;             // ramp reference = maxLife/8
a1x->dword_0x10_16  = 0;                                 // current radius accumulator
a1x->byte_0x46_70   = 1;
a1x->subSpellIndex_0x2A_42 = 4 * (subSpellIndex / maxLife);   // per-hit damage = 4 * (20000/120) ≈ 664
```

### 2.2 Radius ramp `v6` (each tick)
```c
v4 = word_0x2C_44;  v5 = life;
if (maxLife - 3*v4 >= v5) {                 // early third of life:
    if (maxLife - 5*v4 > v5) v6 = --dword_0x10_16;      // very early: shrink
    else { v6 = 3*v4;  rand = 9377*rand+9439;           // mid-early: pin to 3*v4,
           if (rand % 5 == 0) byte_0x46_70 += 2; }      //   1-in-5 chance to jump phase (byte += 2 → >3 → tail-off)
} else v6 = ++dword_0x10_16;                            // late: grow
if (v6 < 0) v6 = 0;
if (v6 > 3*word_0x2C_44) v6 = 3*word_0x2C_44;
if (v6 < 0) v6 = 0;   if (v6 > 15) v6 = 15;             // clamp [0,15]
v8 = byte_0x46_70;  v20 = 0;
if (v8 > 1) { v20 = 1;  byte_0x46_70 = v8 - 1; }        // v20 = "do the half-radius second pass"
```
Note the m50-doc's "up in the middle third / down at the ends" summary is confirmed. `byte_0x46_70 > 3` is the terminal tail-off state that just decrements life (LABEL_51) with no stamping.

### 2.3 The per-cell stamp (verbatim) — **heightmap ±1, NO terrain-type write, NO child spawn**
```c
if (v6 > 0) {
    v21 = centerTileX;  v22 = centerTileY;
    v18 = AddE7EE0x_10080(0, v6);                       // disc of radius v6
    if (v18) {
        while (sub_10130(v18, &v17, &v16) == 1) {       // each cell offset (dx=v17, dy=v16)
            v9 = (centerX + v17, centerY + v16);        // absolute tile
            if (a1x->life_0x8 & 1) v10 = 1; else v10 = -1;       // <-- PARITY: odd life = +1, even = -1
            v11 = v10 + mapHeightmap_11B4E0[v9];
            if (v11 < 0) v11 = 0;   if (v11 > 255) v11 = 255;
            mapHeightmap_11B4E0[v9] = v11;              // <-- THE STAMP: ±1 heightmap displacement
        }
        ResetEvent08_10100(v18);
    }
    if (v20) {                                          // half-radius second pass (when byte>1 this tick)
        v12 = AddE7EE0x_10080(0, v6 >> 1);
        if (v12) {
            while (sub_10130(v12, &v17, &v16) == 1) {
                v13 = (centerX + v17, centerY + v16);
                if (a1x->life_0x8 & 1) v19 = 1; else v19 = -1;   // same parity
                v14 = v19 + mapHeightmap_11B4E0[v13];
                clamp[0,255];  mapHeightmap_11B4E0[v13] = v14;   // ±1 again on inner disc
            }
            ResetEvent08_10100(v12);
        }
    }
    if (!(a1x->life_0x8 & 3)) {                          // every 4th tick: the DAMAGE + visual beat
        SetEntityShiftRot_49EA0(a1x, (v6 << 8), 2048);  // grow sprite to current radius
        PrepareEventSound_6E450(..., 10);               // sound 10
        v15 = sub_10C80(a1x, 0, a1x->subSpellIndex_0x2A_42);   // AoE damage, type-0, amount ≈664
        if (v15) sub_6D8B0(a1x->id_0x1A_26, 0xF, v15);  // spellbook report id 0xF (15)
    }
}
a1x->life_0x8--;   // LABEL_51
```

### 2.4 Resolution of m50-doc OPEN-5
**The per-cell stamp writes `mapHeightmap_11B4E0` only, by ±1 (sign = `life & 1`).** This makes the ground **jitter/vibrate** (alternating raise/lower each tick) within a disc whose radius ramps 0→15→0 over the life, producing the "fissure/trench churn" look. It does **NOT**:
- write a lava terrain-type (unlike the flood, no `mapTerrainType`/`sub_57450`),
- spawn any child entity,
- write the ceiling/second-heightmap.

Damage is entirely via **`sub_10C80(self, 0, subSpellIndex)`** (type-0 AoE, ~664/beat) every 4th tick, reported to the spellbook as effect id 0xF. The `v10`/`v19` parity alternation is the raise-vs-lower toggle, confirmed verbatim.

---

## 3. (10,22)=0x16 — whirlwind damage/pickup passes

Driver `sub_33110` (EF:24155) each tick: `sub_331A0` (move+drag tail) → `sub_33340` (lift/throw pass) → `sub_33710` (contact pass) → loop sound 49. At life<0: `EndLoop_6EAB0(...,49)` then `sub_338D0` teardown.

### 3.1 `sub_331A0` (EF:24177) — head wander + tail drag (fuller than m50-doc §5.4)
```c
predictedAxis = pos;  a1x->word_0x30_48 = pos.z;      // remember eye z
if (!(byte_0x3E_62 & 0xF)) {                           // every 16 ticks:
    rand = 9377*rand+9439;
    if (!(rand & 1)) word_0x2E_46 = -word_0x2E_46;     // randomly flip lateral drift sign
}
roll = (roll + 11*word_0x2E_46) & 0x7FF;               // spin the roll axis
MoveEntity_57FA0(&predictedAxis, roll, 0, 32);         // 32-unit lateral wobble
a1x->axis_0x9A_154x = predictedAxis;                   // <-- axis_0x9A = the EYE center used for damage
yaw = (yaw + 341) & 0x7FF;                             // rotate forward heading 341/2048 ≈ 60°/tick
MoveEntity_57FA0(&predictedAxis, yaw, 0, 120);         // 120-unit forward advance
predictedAxis.z = getTerrainAlt_10C40(&predictedAxis); // clamp head to ground
CopyEntityPosition_57CF0(a1x, &predictedAxis);
// ---- drag each tail node toward the previous node, keeping a spacing gap ----
v7x = Entities[a1x->word_0x34_52];   resultx = a1x;    // word_0x34_52 = next-node link
while (v7x > Entities[0]) {
    predictedAxis = v7x->pos;
    v7x->yaw = tan2(&v7x->pos, &resultx->pos);
    v5 = EuclideanDistXYZ_58490(&resultx->pos, &v7x->pos);
    v6 = 72 - 4*(12 - v7x->word_0x2C_44);              // per-node spacing gap (nodes 1..11 → 28..68)
    if (v5 > v6) MoveEntity_57FA0(&predictedAxis, v7x->yaw, 0, v5 - v6);   // pull in to the gap
    predictedAxis.z = v7x->word_0x36_54 + a1x->pos.z;  // node z = head z + node's z-offset
    CopyEntityPosition_57CF0(v7x, &predictedAxis);
    resultx = v7x;  v7x = Entities[v7x->word_0x34_52];
}
```
Confirms: **the head wanders, the 11 tail nodes trail in a spiral pulled to per-node gaps.** `axis_0x9A_154x` is the eye-center that `sub_33340` measures distances from.

### 3.2 `sub_33340` (EF:24229) — the LIFT-AND-THROW pass (verbatim resolution)
This is the whirlwind's signature effect. It sweeps a **radius-12 disc** of tiles, and for each qualifying entity (`sub_33810` filter — creatures, players class-3 model-0, other class-5 fireball/effects, scenery model-2) it **lifts the entity into the vortex, spins it around the eye, and eventually flings it**, dealing damage while it is airborne.
```c
v32=centerTileX; v36=centerTileY;
v35 = AddE7EE0x_10080(0, 12);                          // radius-12 disc
while (sub_10130(v35, &v29, &v28) == 1) {              // each cell
  for (ix = map entities at cell; ix != Entities[0]; ix = next in cell) {
    if (sub_33810(a1x, ix)) {                          // grabbable?
      v40 = (ix->class==3 && ix->model==0);            // is it a player?
      v38 = v40 ? 56 : 204;                            // yaw step around the eye (players spin slower)
      v34 = v40 ? 384 : 768;                           // lift threshold
      v5 = EuclideanDistXY_584D0(&a1x->axis_0x9A_154x, &ix->pos);   // dist from EYE
      if (v5 >= 3211264) {                             // FAR ring (already flung out):
        if (ix->byte[3] & 0x10) {                      // if currently grabbed:
          ix->byte[1] |= 8;  v39 = 1;                  //   mark, will damage
          v37 = ix->dword_0xA0_160x->word_160_0xe_14;  //   its float bias
          v30 = 64;  ix->yaw = (v38 + ix->yaw) & 0x7FF;//   keep spinning, slow drift 64
          if (v5 >= 5308416) ix->byte[3] &= 0xEF;      //   too far → RELEASE the grab
        }
      } else {                                         // NEAR the eye:
        v6 = tan2(&a1x->axis_0x9A_154x, &ix->pos);
        if (ix->byte[3] & 0x10) {                      // already grabbed → spin fast & lift
          ix->byte[1] |= 8;  v30 = 128;  v39 = 1;
          predictedAxis.z += 114;  ix->yaw = (v38 + ix->yaw) & 0x7FF;
        } else {
          if (v40) { /* player: crank camera roll +28 (up to 256), actSpeed=80 */ }
          if (v33 >= 0x40000) {                        // mid-ring: swirl inward (yaw = bearing+591)
            v14 = (v6 + 591) & 0x7FF;  ix->word_0x30_48 = v14;  ix->yaw = v14;  v30 = 96;
          } else {                                     // inner: begin the LIFT
            ix->byte[1] |= 8;
            predictedAxis = a1x->axis_0x9A_154x;       // snap toward the eye
            v9 = ix->pos.z - a1x->word_0x30_48 + 57;   // height above eye
            predictedAxis.z = max(v9 + terrainAlt, terrainAlt);
            ix->yaw = (v38 + ix->yaw) & 0x7FF;
            ix->rand = 9377*ix->rand + 9439;
            if (v9 >= v34 + ix->rand % v34)            // lifted past threshold →
              { ix->byte[3] |= 0x10;  ix->word_0x30_48 = ix->yaw; }   // set GRABBED flag
          }
        }
      }
      MoveEntity_57FA0(&predictedAxis, ix->word_0x30_48, 0, v30);    // apply the swirl move
      if (isCaveLevel) clamp z under ceiling;
      // z-clamp between the entity's float band, then commit position
      sub_580E0(&predictedAxis, terrainAlt, word_160_0xc_12, word_160_0xa_10, v37);
      CopyEntityPosition_57CF0(ix, &predictedAxis);
      if (v39) {                                       // if the entity was airborne/grabbed this tick:
        v26 = a1x->subSpellIndex_0x2A_42;              // = 1000
        v31++;
        sub_11900(a1x, ix, 0, v26);                    // <-- DAMAGE: add 1000 to victim mailbox
      }
    }
  }
}
ResetEvent08_10100(v35);
if (v31) sub_6D8B0(a1x->id_0x1A_26, 0x15, v31);        // spellbook report id 0x15 (21)
```
**Resolution:** Yes — it **lifts entities (raises z toward/above the eye), spins them around the eye (yaw rotation, step 56/204), and flings them out** (release when dist ≥ 5308416). It **applies `subSpellIndex = 1000` damage** via `sub_11900` (mailbox add) **each tick the victim is airborne** (`v39`). `sub_580E0` z-clamps to the victim's float band. So the "1000" is the whirlwind's damage amount, not a subSpellIndex swap.

### 3.3 `sub_33710` (EF:24416) — the CONTACT pass (every 8 ticks)
```c
if (!(a1x->byte_0x3E_62 & 7)) {                        // only every 8th tick
    for (ix = dword_38527; ix > Entities[0]; ix = ix->next_0)      // "effect" list (fireballs etc.)
        if (CompareAxisWithShift_10750(a1x, ix))
            sub_11900(a1x, ix, 0, a1x->subSpellIndex_0x2A_42);     // damage overlapping effects (1000)
    for (jx = dword_38519; jx > Entities[0]; jx = jx->next_0)      // object list
        if (jx->model_0x40_64 == 2 && CompareAxisWithShift_10750(a1x, jx)) {   // scenery model-2
            jx->word_0x30_48 = 30;  jx->word_0x26_38 = self;
            jx->str_0x5E_94.dword_0x5E_94 += a1x->subSpellIndex_0x2A_42;        // damage scenery (1000)
            v1 += 2;  jx->str_0x5E_94.word_0x62_98 = a1x->id_0x1A_26;
        }
    if (v1) sub_6D8B0(a1x->id_0x1A_26, 0x15, v1);      // spellbook report id 0x15
}
```
**Resolution:** the second pass damages overlapping **effect entities** (fireballs/other spells) and **model-2 scenery objects** by adding `subSpellIndex=1000` to their mailboxes, once every 8 ticks. It does not lift them — that's `sub_33340`'s job; this pass is the collision/scenery-shred pass.

### 3.4 `sub_338D0` (EF:24518) — teardown
```c
v5 = AddE7EE0x_10080(0, 12);                           // radius-12 disc
while (sub_10130(v5, &v10, &v9) == 1)
    for (entities at cell)
        v7x->struct_byte_0xc_12_15.dword &= 0xEFFFF7FF;   // clear the "grabbed/airborne" flags
                                                          //   (bit 0x800 and bit 0x10000000)
ResetEvent08_10100(v5);
for (jx = a1x; jx > Entities[0]; jx = Entities[jx->word_0x34_52])
    DisableEntityDrawing04_57F10(jx);                  // despawn head + all 11 tail nodes
```
Clears every nearby entity's grabbed flags (so victims stop swirling), then walks the `word_0x34_52` chain despawning the whole 12-node whirlwind.

**Resolution of m50-doc OPEN-8 (whirlwind half):** the two passes lift/throw (`sub_33340`) and contact-damage effects+scenery (`sub_33710`), both using the head's `subSpellIndex=1000` as the `sub_11900` mailbox damage amount. There is no subSpellIndex=1000 "swap onto victims" — it is the whirlwind's own damage magnitude.

---

## 4. `sub_5C800(entity, N)` — screen palette-effect dispatcher

### 4.1 The helper (EF:43576) — verbatim
```c
void sub_5C800(type_entity_0x6E8E* a1x, char a2)//23d800
{
    if (D41A0_0.LevelIndex_0xc == a1x->dword_0xA4_164x->playerColorIndex_0x38_56)   // LOCAL player only
        x_D41A0_BYTEARRAY_4_struct.paletteSubMod_180 = a2;      // set the palette-effect code
}
```
`sub_5C800(e, N)` writes the palette-effect selector `paletteSubMod_180 = N`, but **only if entity `e` belongs to the local player** (`e->dword_0xA4_164x->playerColorIndex_0x38_56 == LevelIndex_0xc`). So when the portal calls `sub_5C800(warpedPlayer, 6)`, the flash only shows if the local player is the one warped. The companion `SetPaletteModification_5C830(e, sub, count)` (EF:43592) also sets `paletteCount_184w`.

### 4.2 What the N codes do — the palette state machine (EF:31937, mode 3 → switch on paletteSubMod_180)
The palette machine runs once per frame; it reads `paletteSubMod_180`, applies a VGA-palette transform to a scratch palette (`x_BYTE_F3FA0arx`), installs it (`sub_41A90_VGA_Palette_install`), and usually resets the code to 1 (the "fade back to normal" state). Enumerated:

| N | effect on the full-screen palette | typical trigger |
|---|---|---|
| 0 | (none — machine returns; steady state) | cleared |
| 1 | **Fade back to DefaultPal** (4-step fade-in); when done → 0 | tail of every flash |
| 2 | **Red-lean "hit" flash** (red +40, clamp 63) → 1 | taking damage |
| 3 | **Darken** (red&blue −= 56*paletteCount>>8) → 1 | dimming / storm |
| 4 | **Blue-max tint** (blue forced 63) → 1 | drowning/water |
| 5 | **Blackout** (palette zeroed) → 10 | blackout, then fade-in |
| **6** | **Blue/cyan "cool" flash** (red←blue+48, green+32, blue+32, clamp 63) → 1 | **portal warp / teleport** |
| 7 | **Black & white** (grayscale = (r+g+b)/3) → 1 | petrify/stun |
| 8 | **White-out brighten** (r,g,b +48) → then sets code 9 | strong flash, fade back via 9 |
| 9 | fade-in from black over 16 steps; when done → 0 | recovery from 5/8 |
| 0xA (10) | fade-in over 28 steps; when done → 0 | recovery from blackout(5) |

### 4.3 Resolution of m50-doc OPEN-8 (portal half)
**`sub_5C800(player, 6)` = a blue/cyan full-screen flash shown to the warped local player** as portal feedback — no sound (the portal's own sounds 21/22/20 are separate), no gameplay state change. It is purely a viewer-side palette tint that then auto-fades back (code 6 → 1 → 0). Other call sites confirm the semantics as a generic "screen FX by code": `sub_5C800(a1x,1)` fade-back, `(...,7)` grayscale (petrify), `(...,5)` blackout, `(...,2)` red hit-flash (EF:59735, 60076, 60304, 60354, 60516, 60708). For the (10,34) portal port, code 6 = the cyan warp flash.

---

## 5. Consolidated constants & Rust-port notes

### 5.1 Per-model constants (closing rows)

**(10,67)=0x43 flood/quake** (`sub_39040` :28452, `sub_39E40` :29133, `sub_396A0` :28764, `sub_396D0` :28783)
| item | value |
|---|---|
| window | 30×30 tiles (center ±15); probe also checks 54×54 for overlap |
| abort probe | ≥225/900 open (type-0) cells OR another class-10 model-0x2D(act 48/51)/model-67 within 54×54 |
| phase countdown | `dword_0x10_16 = 12` (morph), `= 16` (restore) |
| dome height | inner disc raised to `pos.z + 64`; outer ring (2304..3840) blended to rim via `sin_DB750` |
| terrain writes | `mapHeightmap` (ease 1/countdown toward dome), `mapTerrainType=1` (lava, `sub_57450` burnable cells), `mapShading`, cave `second_heightmap` |
| entity damage | `sub_3A090` at morph step 5 (mailbox += subSpellIndex, model-2 grab) + `sub_3A200` shove-damage; NO `sub_10C80` |
| entity shove | `sub_39B60` every tick once countdown<6 (radial push, 26×26 disc) |
| sounds | 64 (phase 1 arm, and finisher phase 0) |
| spellbook | id 0x14 (20) via `sub_3A090`/`sub_3A200` |
| exits | life<=0 → action 74; phase 3 → action 73 → action 74; action 74 phase 2 → despawn |

**(10,71)=0x47 fissure** (`sub_3A2D0` :29443)
| item | value |
|---|---|
| per-cell stamp | `mapHeightmap += (life&1 ? +1 : -1)`, clamp [0,255] — heightmap ONLY |
| second (half-radius) pass | when `byte_0x46_70 > 1` this tick, repeat ±1 on radius `v6>>1` |
| radius `v6` | ramp: shrink very-early, pin `3*word_0x2C_44` mid-early, grow late; clamp [0, min(3*word_0x2C_44,15)] |
| word_0x2C_44 | `maxLife >> 3` (= 15 for life 120) |
| per-hit damage | `subSpellIndex = 4*(subSpell/maxLife)` ≈ 664; via `sub_10C80(self,0,·)` every 4th tick |
| sprite | `SetEntityShiftRot(v6<<8, 2048)` every 4th tick |
| sound | 10 (every 4th tick) |
| spellbook | id 0xF (15) |
| phase-jump | 1-in-5 (`rand%5==0`) early → `byte_0x46_70 += 2` (tail-off); `byte>3` → just decrement life |
| NO | terrain-type write, child spawn, ceiling write |

**(10,22)=0x16 whirlwind** (`sub_33110` :24155 and children)
| item | value |
|---|---|
| head move | roll wobble 32, forward 120, yaw += 341/tick; lateral sign flip every 16 ticks (`byte_0x3E&0xF`) |
| eye center | `axis_0x9A_154x` (distances measured from here) |
| tail | 11 nodes, gap `72 - 4*(12 - word_0x2C_44)` (28..68), z = head.z + node.word_0x36_54 |
| lift pass `sub_33340` | radius-12 disc; grab (byte[3]|0x10) when lifted past `v34`(384 player/768 other); spin yaw += 56(player)/204; release at dist≥5308416; damage `sub_11900(·,·,0,1000)` while airborne |
| contact pass `sub_33710` | every 8 ticks: `sub_11900(·,·,0,1000)` to overlapping effect list + model-2 scenery (mailbox += 1000) |
| damage amount | `subSpellIndex = 1000` (fed to `sub_11900`, NOT swapped onto victims) |
| teardown `sub_338D0` | clear grabbed flags (`dword &= 0xEFFFF7FF`) in disc; despawn 12-node chain |
| sound | 49 loop while alive; `EndLoop 49` at death |
| spellbook | id 0x15 (21) |

**`sub_5C800`** (EF:43576) — palette-FX setter; code 6 = blue/cyan flash. Table in §4.2.

### 5.2 Shared primitives newly documented here
| helper | file:line | role |
|---|---|---|
| `sub_11900(src, victim, a3, amount)` | EF:4375 | mailbox damage: `victim[a3].str_0x5E_94.dword_0x5E_94 += amount` (overwrite if already tagged), stamp `word_0x62_98 = src.id`; returns `3*a3` |
| `sub_57450(terrainType)` | EF:39818 | "is burnable/lava-convertible terrain" predicate (type 0, 0x25-0x26, 0x2C-0x2F, 0x51, 0x53, 0x68-0x69…) |
| `sub_580E0(axis, floor, ceil, _, lift)` | EF:40372 | z-clamp: lift `axis.z += lift` if above floor, clamp to `floor+ceil` if below |
| `sub_439A0(cellIndex)` | Terrain.cpp:1459 | neighbour-averaged "natural" heightmap value (used to settle terrain back) |
| `sub_39B60(src)` | EF:29011 | radial entity-shove in a 26×26 disc; in action-74 mode also releases grabs |
| `sub_3A090(src)` | EF:29316 | flood damage/grab pass (kills effects, grabs model-2, mailbox += subSpell, lava sweep) |
| `sub_3A200(src, victim)` | EF:29382 | per-entity shove callback (sets airborne flags, rng-gated mailbox damage) |
| `paletteSubMod_180` machine | EF:31937 | per-frame VGA palette FX consumer for `sub_5C800` codes |

### 5.3 Rust-port notes
- **(10,67) flood:** port as a terrain-morph state machine, NOT a damage burst. It mutates the shared heightmap/terrain-type/shading grids — the port needs writable terrain layers and a `sub_462A0`-equivalent "relight/remesh rect". Damage is via the entity mailbox (`sub_11900` semantics), applied at morph-step 5 and every restore tick, plus a radial physics shove (`sub_39B60`). The dome uses a `sin` LUT (`sin_DB750`) — reuse the same LUT for bit-exact heights. State graph: 0x48(phase 0..3) → 0x49(countdown) → 0x4A(restore→despawn), with a life<=0 shortcut straight to 0x4A. This is the heaviest terrain effect in class-10.
- **(10,71) fissure:** trivial by comparison — a radius-ramp disc that toggles heightmap ±1 by life parity (ground vibration) + a periodic `sub_10C80` type-0 AoE (~664) every 4th tick. No terrain-type or child. Port = radius ramp + parity heightmap jitter + throttled AoE.
- **(10,22) whirlwind:** the lift/throw physics (`sub_33340`) is the interesting part — a per-entity grab state (byte[3] bit4), spin around `axis_0x9A_154x`, z-lift, and release at range, with `sub_11900` damage (1000) while airborne. The contact pass (`sub_33710`) is a throttled overlap-damage sweep. Reuse the mailbox-damage contract. `axis_0x9A_154x` = the eye center; keep it distinct from the head render position.
- **(10,34) portal:** `sub_5C800(player, 6)` = a cyan screen flash for the warped local player. Port as a client-side palette/post-fx tint (auto-fade), gated to the local player. It is cosmetic only; the warp itself is already covered by the m50-doc §2.

---

## OPEN items (remaining after this pass)
1. **`sub_439A0` exact formula** — used as the "natural height" restore target; body in Terrain.cpp:1459 not transcribed here (behavioral role — neighbour-averaged height — is sufficient for the flood port, but bit-exact restore needs it).
2. **`sub_3A090`/`sub_3A200` downstream damage application** — these write the victim mailbox `str_0x5E_94.dword_0x5E_94`; the victim's own action reads and converts it to HP loss. That conversion (per victim class) is the generic MC2 damage-mailbox contract, out of scope here.
3. **`GetTerrainHeightFromSquare_48DF0` / `sub_462A0` / `AddBuildingToTerrain_46570`** — terrain-sample and remesh helpers, treated as opaque (sample-max-height / rebuild-render-rect / re-stamp-building). Bodies not needed for behavior but would be for a fully faithful terrain pipeline.
4. **`sin_DB750` LUT** — the dome/falloff table; must match the original for bit-exact heightmap morph. Confirm the table is already extracted in the asset bank before porting the flood.

---

## Retail-check bank (for player-certified verification later)
- **(10,67) flood/quake:** cast on non-open terrain → a 30×30 dome rises (over ~12 ticks) with sound 64, burnable ground turns to lava, nearby creatures/objects are shoved outward and lifted, then the ground settles back and the effect despawns. Refuses to fire on mostly-open ground or near another active quake.
- **(10,71) fissure:** a growing/shrinking disc where the ground visibly vibrates (±1 jitter), sound 10 pulses every 4th tick, ~664 area damage/beat.
- **(10,22) whirlwind:** entities within ~12 tiles get sucked in, lifted, spun around the eye, and flung out; ~1000 damage/tick while airborne; scenery shredded every 8 ticks; wind loop sound 49.
- **(10,34) portal `sub_5C800(player,6)`:** on warp, the LOCAL player's screen gets a brief blue/cyan flash that fades back to normal (only if the warped player is the viewer).
