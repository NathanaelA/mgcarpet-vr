# CLASS-10 Model 9 — Raise-Land / Apocalypse Dome: FULL Geometry & SPELLS-table Closure

Closes OPEN items 2, 3, 5 of `docs/traces/mc2-class10-m6-m9-m11-m28-m31.md`. All citations to
`/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`)
and `/home/rain/projects/mgcarpet/reference/remc2/remc2/utilities/` (Maths.cpp). Trace date 2026-07-10.
Read the parent doc §2 first; this doc only expands the geometry/table/latch that were cited-but-not-fully-expanded there.

**Headline closures:**

1. **The dome footprint radius is FIXED, not accumulated.** `array_0x52_82.pitch` is set once in the init phase
   to `(maxLife|1)<<8` (EF:23251) and only READ thereafter. There is no per-tick radius increment anywhere in
   `sub_31940` or `sub_22190`. What grows over life is the **height** of each cell (eased `(target-current)/life`),
   not the disc. The ctor's `pitch=7` seed (EF:35525) is a throwaway placeholder overwritten on the first tick.
2. **`sin_DB750` is a 2560-entry (`0x200+0x800` usable) sine table in signed 16.16 fixed point.** Index i = `sin(i * 360/2048 deg)*0x10000`. `sin_DB750[0x200 + phase]` = **cos(phase)**. The dome uses `(0x10000 + cos(phase))>>1` = raised cosine, phase `= (dist<<10)/radius` ∈ [0, 0x400] ⇒ profile 1.0 at center → 0.0 at rim. Table def: `Maths.cpp:3` (`int32_t Maths::sin_DB750[2560]`).
3. **Model 9 → spell index 18.** `GetSpellIndex_6E020` (EF:44240) `case 9: return 18`. subspells (from `DATA/SPELLS.DAT`, fallback baked in `Spells.cpp:80-82`, entry 18): par1_14 selects tier — `{subSpell=120,life=7}`, `{240,9}`, `{480,11}`. Model 9 override writes **`maxLife_0x4`** (not life). Field offsets: element stride 26, `.subSpellIndex_2`=+0, `.life_0x1A`=+24.
4. **Apocalypse latch ordering confirmed** (EF:12864-12872): create (10,9) [ctor zeroes `byte_0x36E03`] → set life/maxLife/id → **THEN** `byte_0x36E03=1`. First tick sees the variant.

---

## 1. The three-phase machine (`sub_31940`, EF:23193-23433) — verbatim spine

`byte_0x46_70` is the phase: `0`=init (runs once), `1`=grow (per tick), `2`=finalize (runs once, then despawn).
Tile-center is computed every entry: `v45x.x = (pos.x+128)>>8`, `v45x.y = (pos.y+128)>>8` (EF:23241-23242).
`uaxis_2d` (global_types.h:69) = `union { struct{uint8_t x,y;} ; uint16_t word; }` — so `.word = x | (y<<8)` is the
direct index into the 256×256 terrain arrays `mapHeightmap_11B4E0[65536]`, `mapShading_12B4E0[65536]`,
`mapAngle_13B4E0[65536]` (Terrain.h:20-22) and `x_BYTE_14B4E0_second_heightmap` (BasicTerrain.h:111).

### 1.1 Phase 0 — INIT (`byte_0x46_70 == 0`, EF:23245-23261)
```c
if (a1x->byte_0x46_70 < 1u) {
    if (a1x->byte_0x46_70) return;                 // (guards the signed-<1 corner; byte is 0 here)
    v2 = a1x->maxLife_0x4;  LOBYTE(v2) = v2 | 1;    // radius tiles = maxLife | 1  (odd, so a center tile exists)
    SetEntityShiftRot_49EA0(a1x, (x_WORD)v2 << 8, 0x4000);   // pitch = roll = (maxLife|1)<<8 ; fov = 0x4000
    v44x.x = v45x.x - v2;  v44x.y = v45x.y - v2;             // bounding-box origin (unused past here)
    v3 = (uint8_t)(v45x.y - v2);                             // box top-left tile (y)
    v4 = (uint8_t)(v45x.x - v2);                             // box top-left tile (x)
    LOWORD(v2) = 2 * v2;                                     // box side = 2*(maxLife|1)
    a1x->position_0x4C_76.z = sub_48E60(v4, v3, v2, v2);     // dome BASE z = MIN terrain height over the box
    a1x->word_0x2C_44 = v2 + 100;                            // dome HEIGHT = 2*(maxLife|1) + 100
    if (a1x->position_0x4C_76.z + a1x->word_0x2C_44 > 255)   // clamp so summit <= 255
        a1x->word_0x2C_44 = 255 - a1x->position_0x4C_76.z;
    a1x->byte_0x46_70 = 1;
}
```
- **`SetEntityShiftRot_49EA0`** (EF:32874) is a pure setter: `pitch=roll=shift`, `fov=rotation`. So `array_0x52_82.pitch = (maxLife|1)<<8`. With the endgame `maxLife=11` ⇒ pitch = `12<<8 = 0x0C00 = 3072` (radius = 12 tiles). NOTE: `maxLife|1` uses the maxLife AFTER the SPELLS-table override (§3), so a par1-authored dome uses maxLife∈{7,9,11}.
- **`sub_48E60`** (EF:32623) = `sub_48F20(a1,a2,a3,a4, mapHeightmap_11B4E0)`. `sub_48F20` (EF:32647) walks the **perimeter** of an `a3 x a4` tile box from (a1,a2) and returns the **MINIMUM** height found (init `result=250`, keeps smaller). Two-arm loop: top+bottom rows if `a4!=0`, then left+right columns via `LABEL_9` if `a3!=0`. So the dome base is planted at the lowest ground under its footprint. (Siblings: `sub_48F20`→min on mapHeightmap; `sub_48FD0`→**max**; `sub_48EC0/48EF0`→same on the second heightmap.)
- After init, `execution falls through` to the grow body (there is no `return` in phase-0) — so the tick that sets `byte_0x46_70=1` ALSO does a first grow pass.

### 1.2 Phase 1 — GROW (per tick, EF:23324-23431)
```c
v5 = a1x->life_0x8 - 1;  a1x->life_0x8 = v5;
if (v5 <= 0) { a1x->byte_0x46_70 = 2; }         // life exhausted -> finalize next entry
else {
    v6 = a1x->array_0x52_82.pitch;              // FIXED radius accumulator (fixed-point, tiles<<8)
    v7 = v6 >> 7;                               // loop count = pitch/128 = 2*(maxLife|1)  (box side in tiles)
    v44x.x = v45x.x - BYTE1(v6);                // box origin x = center - (pitch>>8) = center - (maxLife|1)
    v34 = v6 - ((((v6>>8) - 7) >> 1 << 8) + 512);   // INNER-flat threshold (see below)
    v44x.y = v45x.y - BYTE1(v6);
    // disc walk: ROW-MAJOR over the (v7 x v7) bounding box, y outer, x inner
    for (v9x.y = v45x.y - BYTE1(v6), v8=0; v8 < v7; ++v9x.y, ++v8)
    for (v9x.x = v44x.x,            v10=0; v10 < v7; ++v9x.x, ++v10) {
        v31x.x = v9x.x << 8;  v31x.y = v9x.y << 8;
        v11 = Maths::EuclideanDistXYZ_58490(&a1x->position_0x4C_76, &v31x);   // 3D dist center->cell (world units)
        v33 = v11;
        if (v11 < v6) {                                                       // inside radius (both in <<8 units)
            // ---- COSINE DOME PROFILE ----
            v12 = (a1x->word_0x2C_44
                   * ((0x10000 + (int)Maths::sin_DB750[0x200 + (v11 << 10) / v6]) >> 1) >> 16)
                  + a1x->position_0x4C_76.z;                                  // target summit-height for this cell
            v42 = mapHeightmap_11B4E0[v9x.word];                              // current terrain height
            v14 = v42;
            if (v12 > v42) v14 = (v12 - v42) / a1x->life_0x8 + v42;           // EASE: 1/life of the gap this tick
            sub_570F0(v9x.x, v9x.y, v14, 0, v33 <= v34, 1);                   // write height (+ dirty/normals)
            if (isCaveLevel_D41B6) {                                          // cave: raise the CEILING too
                v43 = v14 + 64;  if (v43 > 254) v43 = 254;
                v38 = x_BYTE_14B4E0_second_heightmap[v9x.word];
                if (v43 > v38) {
                    v15 = (v38 - v43) / a1x->life_0x8;
                    x_BYTE_14B4E0_second_heightmap[v9x.word] = v38 - v15;     // ease ceiling up by same 1/life
                }
            }
        }
        if (isCaveLevel_D41B6) {                                             // cave open/closed angle flag
            if (x_BYTE_14B4E0_second_heightmap[v9x.word] > mapHeightmap_11B4E0[v9x.word])
                mapAngle_13B4E0[v9x.word] &= 0xF7u;    // clear bit3: floor below ceiling (open)
            else
                mapAngle_13B4E0[v9x.word] |= 8u;       // set bit3: floor meets/exceeds ceiling (sealed)
        }
    }
    // ---- combat + audio pulse ----
    if (!D41A0_0.byte_0x36E03)
        v39 = (int16_t)sub_116A0(a1x, 0, a1x->subSpellIndex_0x2A_42);        // area dmg (ch0); returns hit count
    if (v39) sub_6D8B0(a1x->id_0x1A_26, 0x12u, v39);                         // earthquake/shake event kind 0x12
    PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 10);            // rumble sound 10 EVERY tick
    if (D41A0_0.byte_0x36E03 && !(a1x->byte_0x3E_62 & 3))
        PrepareEventSound_6E450(..., -1, 63);                               // apocalypse: sound 63 every 4th tick
    if (a1x->life_0x8 == 3) { /* summit cap + child, see §1.4 */ }
}
```

**Iteration order (closed):** ROW-MAJOR, `y` outer / `x` inner, over the square bounding box
`[center - (maxLife|1) .. center - (maxLife|1) + 2*(maxLife|1) - 1]` in BOTH axes (side = `pitch>>7` =
`2*(maxLife|1)` tiles). Per cell a Euclidean-distance disc test `v11 < v6` clips the box to a disc; cells outside
the disc are skipped for height (but on cave levels the mapAngle open/sealed flag is still written for every box cell).

**Easing (closed):** each in-disc cell moves `(target - current)/life` toward the cosine target THIS tick (integer
division, truncating). Because `life` decreases each tick, the step is `1/life` of the remaining gap — a classic
"ease-out over remaining ticks" that lands the cell exactly on `target` when `life==1`. Only raises (`v12>v42`);
never lowers terrain during grow.

**`v34` inner-flat threshold (closed):** `v34 = pitch - ((((pitch>>8) - 7) >> 1 << 8) + 512)`. With `pitch>>8 = R`
tiles this is `v34 = (R<<8) - ((((R-7)>>1)<<8) + 512)` world-units. Cells with `dist <= v34` pass `a5=1` to
`sub_570F0`, which force-marks their `mapAngle` low-nibble = flat/terrain-typed (see §2). So the dome's inner core
is flagged as walkable/flat terrain, the outer annulus keeps its slope shading.

### 1.3 Phase 2 — FINALIZE (`byte_0x46_70 == 2`, EF:23263-23319)
```c
if (a1x->byte_0x46_70 > 1u) {
  if (a1x->byte_0x46_70 == 2) {
    v43 = a1x->position_0x4C_76.z + a1x->word_0x2C_44 - 24;   // summit plateau height (dome top minus 24)
    v20 = a1x->array_0x52_82.pitch; v22 = v20 >> 7;  v20 >>= 8; // v22 = box side (2R), v20 = R
    // clamp the whole footprint DOWN to the plateau (only cells above it):
    for (v23x.y = v45x.y - v20, v24=0; v24 < v22; ++v23x.y, ++v24)
      for (v23x.x = v45x.x - v20, v27=0; v27 < v22; ++v23x.x, ++v27)
        if (mapHeightmap_11B4E0[v23x.word] > v43) mapHeightmap_11B4E0[v23x.word] = v43;
    // stamp a 2x2 summit CAP at plateau-16 with bright shading:
    for (v25=0, v26x.y = v45x.y - 1; v25 < 2; ++v25, ++v26x.y)
      for (v28=0, v26x.x = v45x.x - 1; v28 < 2; ++v28, ++v26x.x) {
        mapHeightmap_11B4E0[v26x.word] = v43 - 16;
        mapShading_12B4E0[v26x.word] = (MapType != Day) ? 1 : 63;
      }
    DisableEntityDrawing04_57F10(a1x);       // despawn
  }
  return;
}
```
Finalize FLATTENS the dome to a plateau (`summit_z - 24`), then presses a 2×2 pit-cap at `-16` with shading
63 (Day) / 1 (Night/Cave). Then it despawns. Note the flatten writes `mapHeightmap` DIRECTLY (not via `sub_570F0`)
and only lowers cells that are above the plateau.

### 1.4 The `life==3` summit-cap + child (inside grow, EF:23400-23430)
Identical 2×2 cap stamp as finalize (`v43 = pos.z + word_0x2C_44 - 24`, height `v43-16`, shading 63/1), THEN:
```c
predictedAxis_EB398ar = a1x->position_0x4C_76;
predictedAxis_EB398ar.z = getTerrainAlt_10C40(&predictedAxis_EB398ar);
v1x = byte_0x36E03 ? IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 91)   // apocalypse child (10,91)
                   : IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 18);  // normal child (10,18)
if (v1x) v1x->id_0x1A_26 = a1x->id_0x1A_26;
```
So the child (10,18)/(10,91) births at `life==3`, one tick before finalize begins (life reaches 0 → phase 2).

---

## 2. `sub_570F0` — the heightmap writer + dirty/normal recompute (EF:39602-39709, verbatim)

Signature: `char sub_570F0(int16 x, int16 y, int16 h, char a4, char a5, char a6)`. Called by the dome as
`sub_570F0(cx, cy, easedHeight, a4=0, a5=(dist<=v34), a6=1)`.
```c
v9x.x = x; v9x.y = y; v8 = 0;
if (h > 255) { h = 255; if (!x && !y) v8 = 1; }       // clamp high; flag if origin cell
if (h < 0)   { h = 0;   if (!x && !y) v8 = 1; }       // clamp low
if (a4 && mapAngle_13B4E0[v9x.word] < 0) return 1;     // a4: skip cells whose mapAngle high-bit (0x80) is set (locked)
mapHeightmap_11B4E0[v9x.word] = h;                     // WRITE the height
if (a5 || sub_57450(mapTerrainType_10B4E0[v9x.word]))  // a5 OR terrain-type flat-eligible ...
    mapAngle_13B4E0[v9x.word] = mapAngle_13B4E0[v9x.word] & 0xF8 | 1;   // ... force low-nibble = 1 (flat/typed)
if (!h) { /* a6: 8-neighbour water-seal cleanup when height hit 0 -> clears mapAngle low nibble */ }
if (a4) sub_462A0(v9x, v9x);              else AddBuildingToTerrain_46570(v9x, v9x);   // recompute normals/light
return v8;
```
**a4=0** in the dome path ⇒ (a) no lock-skip, (b) the terrain refresh goes through
`AddBuildingToTerrain_46570(cell,cell)` which recomputes the tile's shading/normal from the new heights. **a5** is the
"force this tile flat/terrain-typed" bit — the dome sets it only for the inner core (`dist <= v34`). **a6=1** enables
the h==0 water-seal neighbour walk (never triggers for the dome since it only raises). `sub_57450` tests whether a
terrain-type id is one of the "auto-flatten" types.

**Dirty-flag semantics (closed):** there is no separate dirty bitmap; the "dirty flag" IS `mapAngle_13B4E0`'s
low nibble (`&0xF8 | 1` = flat-authored) plus the immediate `AddBuildingToTerrain_46570` normal/shading recompute
per written cell. The renderer reads `mapHeightmap`/`mapShading`/`mapAngle` live; writing them + recomputing the
tile normal is the whole "dirty" contract.

---

## 3. SPELLS-table indexing — full closure

### 3.1 `GetSpellIndex_6E020` (EF:44240, verbatim)
```c
int GetSpellIndex_6E020(int entitySubtype) {
    switch (entitySubtype) {
        case 9:  return 18;   // MODEL 9  -> spell row 18   <-- the dome
        case 11: return 16;   // (10,11)/0x0B
        case 15: return 17;   // (10,15)/0x0F
        case 17: return 9;
        case 22: return 21;
        case 67: return 20;
        case 71: return 15;
    }
    return 0;                 // everything else -> row 0
}
```
So the model→spell map for the par1-consuming class-10 effects: **9→18, 0x0B(11)→16, 0x0F(15)→17.** The other
queried models (0x0F handled, 0x11=17→9, 0x16=22→21, 0x43=67→20, 0x47=71→15) map elsewhere and default 0 is used
for anything unlisted.

### 3.2 Struct layout (`Spells.h`, `#pragma pack(1)`)
```c
typedef struct {                    // subspell element, stride = 26 bytes
    int32_t subSpellIndex_2;        // +0   <- copied into entity.subSpellIndex_0x2A_42
    int32_t manaCost_6;             // +4
    int32_t maxManaLimit_A;         // +8
    int32_t xpos1_E;                // +12
    int32_t xpos2_0x12;             // +16
    int16_t hintText_0x16x;         // +20
    int16_t word_0x18;              // +22
    int8_t  life_0x1A;              // +24  <- copied into maxLife (model 9) / life (0x0B/0x0F)
    uint8_t fontType_0x1B;          // +25
} type_SPELLS_BEGIN_BUFFER_str_sub;

typedef struct {                    // spell row, stride = 80 bytes
    int8_t  byte_0;                 // +0
    uint8_t isEnabled_1;            // +1
    type_SPELLS_BEGIN_BUFFER_str_sub subspell[3];   // +2 .. +79  (3 x 26)
} type_SPELLS_BEGIN_BUFFER_str;     // SPELLS_BEGIN_BUFFER_str[26]
```
The `_2 / _0x1A` suffixes are offsets WITHIN THE 80-BYTE ROW (subSpellIndex at row+2, life at row+26=+2+24).
Within the sub-element the offsets are +0 and +24.

### 3.3 Where it loads from (importer must carry this)
- `Basic.cpp:334`: `Pathstruct xadataspellsdatx = { "DATA/SPELLS.DAT", &SPELLS_BEGIN_BUFFER_ptr, ... }` —
  the authoritative table is read from **`DATA/SPELLS.DAT`** (26 rows × 80 bytes = 2080 bytes) into the buffer.
- `Spells.cpp:2-107` holds a full baked-in fallback of the same 26×80 table (identical semantics, used before/if
  the .DAT isn't loaded). `LevelInit.cpp:12-21` patches two rows (4 and 19) per difficulty at level init.
- Our importer must ship the same 26×80 table (from `DATA/SPELLS.DAT`) so that par1-authored (10,9)/(10,11)/(10,15)
  THINGs resolve to the same subspell/life values.

### 3.4 Model-9 spell row (entry 18) — actual subspell values (from `Spells.cpp:80-82`, = DATA/SPELLS.DAT fallback)
| par1_14 | subSpellIndex_2 (=area dmg) | life_0x1A (→ **maxLife**) | hintText |
|---|---|---|---|
| 0 | **120** (0x78) | **7** | 241 |
| 1 | **240** (0xF0) | **9** | 242 |
| 2 | **480** (0x1E0) | **11** | 243 |

So a level-authored (10,9) with `par1_14=k` gets `subSpellIndex = {120,240,480}[k]` and `maxLife = {7,9,11}[k]`.
`maxLife` then drives BOTH the dome radius (`(maxLife|1)` tiles) and the dome height (`2*(maxLife|1)+100`) and the
grow-phase duration. The endgame runtime spawner ignores par1 and hard-forces `maxLife=11` (⇒ tier-2 geometry) but
NOT the subspell (it keeps the ctor's `subSpellIndex=2000`, and the apocalypse variant suppresses damage anyway).

For cross-reference: model 0x0B (spell 16) tiers `{50/6, 100/12, 300/24}` write **life**; model 0x0F (spell 17)
tiers `{80/16, 120/32, 320/64}` write **life**.

### 3.5 The two override sites (verbatim)
Generic THING post-init `sub_4A310` case 0xA (EF:33148-33195):
```c
v3x->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(v3x->model_0x40_64)].subspell[entity->par1_14].subSpellIndex_2;
...
// model 9 falls to the bottom block:
v3x->maxLife_0x4 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(v3x->model_0x40_64)].subspell[entity->par1_14].life_0x1A;   // EF:33195
```
GenerateEvents `PrepareEvents` (EV:367-371):
```c
event->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(subtype)].subspell[par1_14].subSpellIndex_2;
if (subtype == 9) event->maxLife_0x4 = SPELLS[...].subspell[par1_14].life_0x1A;   // model 9: maxLife
else              event->life_0x8    = SPELLS[...].subspell[par1_14].life_0x1A;   // 0x0B/0x0F: life
```

---

## 4. Apocalypse-latch ordering (OPEN item 5) — CONFIRMED

`sub_21030` case 0xF (EF:12857-12876), verbatim of the relevant block:
```c
case 0xF:
    KillAllCreatures_1B5F0();
    v20 = a1x->dword_0x10_16 - 1;  a1x->dword_0x10_16 = v20;
    if (v20 <= 0) {
        v21x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 9);   // (1) CREATE dome
        if (v21x) {                                                                     //     ctor zeroes byte_0x36E03
            v21x->life_0x8   = 32;                                                      // (2) force life/maxLife/id
            v21x->maxLife_0x4 = 11;
            v21x->id_0x1A_26 = v1x->id_0x1A_26;
            D41A0_0.byte_0x36E03 = 1;                                                   // (3) SET latch AFTER create
        }
        D41A0_0.word_0x36548 = 0;
        DisableEntityDrawing04_57F10(a1x);
    }
```
Order is exactly: **create → set fields → set `byte_0x36E03=1`**. The (10,9) ctor `NewAdd0A09_4E760`
(EF:35527) writes `byte_0x36E03 = 0`, so the endgame MUST re-set it after — which it does, inside the `if(v21x)`
block. The dome runs on the NEXT frame's `UpdateEntities` tick, by which time `byte_0x36E03 == 1`. Therefore the
dome's very first tick already: (a) SUPPRESSES `sub_116A0` damage (grow-phase `if(!byte_0x36E03)`), (b) plays sound
63 on 4-tick cadence, (c) will spawn (10,91) not (10,18) at life==3. Confirmed load-bearing: the endgame world-rise
deals NO area damage; only the earthquake path is dead too (v39 stays 0 → no `sub_6D8B0`).

---

## 5. Consolidated constants table

| item | value | source |
|---|---|---|
| ctor class/model/action | 10 / 9 / 9 | EF:35516-35518 |
| ctor maxLife / life / subSpell | 11 / 17 / 2000 | EF:35519-35523 |
| ctor pitch seed (throwaway) | `SetEntityShiftRot(7,0x4000)` → pitch=roll=7 | EF:35525, 32874 |
| ctor clears latch | `byte_0x36E03 = 0` | EF:35527 |
| init: radius (tiles) | `maxLife \| 1` (odd); pitch = `(maxLife\|1)<<8` | EF:23250-23251 |
| init: box side (tiles) | `2*(maxLife\|1)` = `pitch>>7` | EF:23256, 23273/23333 |
| init: base z | `sub_48E60` = MIN terrain height over the `2R x 2R` box | EF:23257, 32623/32647 |
| init: dome height | `word_0x2C_44 = 2*(maxLife\|1) + 100`, clamped `base+ht <= 255` | EF:23258-23260 |
| grow: loop order | ROW-MAJOR (y outer, x inner) over `2R x 2R` box, disc-clipped `dist<v6` | EF:23338-23391 |
| grow: cell target | `word_0x2C_44 * ((0x10000 + cos(phase))>>1) >> 16 + base_z` | EF:23355-23357 |
| grow: phase | `(dist<<10)/radius` ∈ [0,0x400]; `cos = sin_DB750[0x200+phase]` | EF:23356 |
| grow: easing | `(target-current)/life + current` (raise-only) | EF:23362-23363 |
| grow: inner-flat threshold | `v34 = pitch - ((((pitch>>8)-7)>>1<<8)+512)`; `dist<=v34` → a5=1 | EF:23335, 23365 |
| grow: heightmap write | `sub_570F0(x,y,h,0,dist<=v34,1)` | EF:23365, 39602 |
| grow: cave ceiling | `second_heightmap += (target+64 - cur)/life`, cap 254; mapAngle bit3 open/sealed | EF:23366-23387 |
| grow: damage | `sub_116A0(ch0, subSpell)` (only if `!byte_0x36E03`) → hits → `sub_6D8B0(id,0x12,hits)` | EF:23392-23395 |
| grow: sound | 10 every tick; 63 every 4th tick (apocalypse only) | EF:23396-23398 |
| grow: child @ life==3 | (10,18) normal / (10,91) apocalypse; +2x2 summit cap | EF:23400-23429 |
| finalize: plateau | flatten footprint to `base+height-24` (lower-only) | EF:23267-23291 |
| finalize: 2x2 cap | height `plateau-16`, shading 63(Day)/1(else) | EF:23300-23318 |
| finalize: despawn | `DisableEntityDrawing04_57F10` | EF:23319 |
| radius growth per tick | **NONE — fixed at init** | EF:23251 (only write); grow/finalize read-only |
| spell index (model 9) | **18** | EF:44243 |
| spell entry 18 subspells | par1 {0,1,2} → subSpell {120,240,480}, maxLife {7,9,11} | Spells.cpp:80-82 |
| SPELLS source file | `DATA/SPELLS.DAT` (26×80 bytes) | Basic.cpp:334 |
| subspell element stride / offs | 26 bytes; subSpellIndex_2=+0, life_0x1A=+24 | Spells.h:7-18 |
| sin table | `Maths::sin_DB750[2560]`, signed 16.16, i=`sin(i*360/2048°)*0x10000` | Maths.cpp:3 |
| cos lookup | `sin_DB750[0x200 + phase]`; `[0x200]`=0x10000=+1.0, `[0x600]`=-1.0 | Maths.cpp:132, 145 |
| latch order | create(zeroes) → fields → `byte_0x36E03=1` | EF:12864-12871 |
| endgame spawn forced | `life=32, maxLife=11` (subSpell stays 2000, unused in variant) | EF:12868-12869 |

---

## 6. Rust-port pseudocode (dome deformer)

```rust
// One entity; `phase: u8` (0 init, 1 grow, 2 finalize); operates on the shared 256x256 terrain maps.
// heightmap/shading/angle : [u8; 65536] indexed by (x as u16) | ((y as u16) << 8).
// second_heightmap: cave ceiling. is_cave / map_type from level.
// COS table: sin_db750[2560] i32 (16.16). cos(phase) = sin_db750[0x200 + phase].

fn tick(e: &mut Entity, t: &mut Terrain) {
    let cx = ((e.pos.x as i32 + 128) >> 8) as u8;   // center tile
    let cy = ((e.pos.y as i32 + 128) >> 8) as u8;
    let mut hits = 0i32;

    if e.phase == 0 {
        let r = (e.max_life | 1) as i32;                 // radius in tiles (odd)
        e.pitch = (r << 8) as i16;                        // FIXED for the whole grow
        let ox = cx.wrapping_sub(r as u8);
        let oy = cy.wrapping_sub(r as u8);
        e.pos.z = sample_min_height(t, ox, oy, (2*r) as i16, (2*r) as i16); // sub_48E60
        e.dome_height = (2*r + 100) as i16;              // word_0x2C_44
        if e.pos.z as i32 + e.dome_height as i32 > 255 {
            e.dome_height = (255 - e.pos.z as i32) as i16;
        }
        e.phase = 1;
        // FALL THROUGH into grow (no early return)
    }

    if e.phase == 2 {                                    // FINALIZE (one-shot)
        let plateau = e.pos.z as i32 + e.dome_height as i32 - 24;
        let r = (e.pitch >> 8) as i32;
        let side = (e.pitch >> 7) as i32;                // 2R
        for j in 0..side { for i in 0..side {
            let x = cx.wrapping_sub(r as u8).wrapping_add(i as u8);
            let y = cy.wrapping_sub(r as u8).wrapping_add(j as u8);
            let w = idx(x, y);
            if t.height[w] as i32 > plateau { t.height[w] = plateau as u8; }
        }}
        for j in 0..2 { for i in 0..2 {                  // 2x2 cap
            let w = idx(cx.wrapping_sub(1).wrapping_add(i), cy.wrapping_sub(1).wrapping_add(j));
            t.height[w]  = (plateau - 16) as u8;
            t.shading[w] = if t.map_type != Day { 1 } else { 63 };
        }}
        e.despawn = true;
        return;
    }

    // ---- GROW (phase 1) ----
    e.life -= 1;
    if e.life <= 0 { e.phase = 2; return; }

    let radius = e.pitch as i32;                          // <<8 world units
    let side   = radius >> 7;                             // 2R tiles
    let r_tiles = (radius >> 8) as i32;                   // R
    let v34 = radius - ((((r_tiles) - 7) >> 1 << 8) + 512);   // inner-flat threshold
    for j in 0..side {
        let y = cy.wrapping_sub(r_tiles as u8).wrapping_add(j as u8);
        for i in 0..side {
            let x = cx.wrapping_sub(r_tiles as u8).wrapping_add(i as u8);
            let w = idx(x, y);
            let dist = euclid_xyz(e.pos, (x as i32) << 8, (y as i32) << 8);   // 3D, <<8 units
            if dist < radius {
                let phase = ((dist << 10) / radius) as usize;                 // 0..0x400
                let cosv = sin_db750[0x200 + phase];                          // 16.16
                let target = ((e.dome_height as i64
                              * (((0x10000 + cosv as i64) >> 1))) >> 16) as i32
                             + e.pos.z as i32;
                let cur = t.height[w] as i32;
                let h = if target > cur { (target - cur) / e.life as i32 + cur } else { cur };
                write_height(t, x, y, h.clamp(0,255), /*a4*/false, /*a5*/ dist <= v34); // sub_570F0
                if e.is_cave {
                    let mut c = (h + 64).min(254);
                    let cur_c = t.second[w] as i32;
                    if c > cur_c { let d = (cur_c - c) / e.life as i32; t.second[w] = (cur_c - d) as u8; }
                }
            }
            if e.is_cave {
                if t.second[w] > t.height[w] { t.angle[w] &= 0xF7; } else { t.angle[w] |= 0x08; }
            }
        }
    }
    if !e.apocalypse { hits = area_damage_ch0(e, e.subspell); }     // sub_116A0
    if hits != 0 { earthquake_event(e.id, 0x12, hits); }           // sub_6D8B0
    play_sound(e, 10);
    if e.apocalypse && (e.byte_0x3E & 3) == 0 { play_sound(e, 63); }
    if e.life == 3 {
        let plateau = e.pos.z as i32 + e.dome_height as i32 - 24;
        for j in 0..2 { for i in 0..2 {
            let w = idx(cx.wrapping_sub(1).wrapping_add(i), cy.wrapping_sub(1).wrapping_add(j));
            t.height[w]  = (plateau - 16) as u8;
            t.shading[w] = if t.map_type != Day { 1 } else { 63 };
        }}
        let child_model = if e.apocalypse { 91 } else { 18 };
        if let Some(c) = spawn(10, child_model, e.pos_snapped_to_terrain()) { c.id = e.id; }
    }
}

// write_height (sub_570F0), height already clamped:
fn write_height(t: &mut Terrain, x: u8, y: u8, h: i32, a4: bool, a5: bool) {
    let w = idx(x, y);
    if a4 && (t.angle[w] & 0x80) != 0 { return; }
    t.height[w] = h as u8;
    if a5 || terrain_type_auto_flat(t.ttype[w]) { t.angle[w] = (t.angle[w] & 0xF8) | 1; }
    // h==0 water-seal neighbour walk omitted (dome only raises)
    recompute_tile_normal_shading(t, x, y);   // AddBuildingToTerrain_46570
}
```

---

## OPEN items (remaining after this pass)

1. **`sub_6D8B0(id, 0x12, hits)` body.** Confirmed as an earthquake/shake queued event, gated `!(setting_38545 & 4)`,
   but its actual effect (camera shake vs. knockback vs. damage propagation) is unread. Only fires in the NON-apocalypse
   dome (endgame variant never hits). Low priority for the endgame cinematic; needed for a player-cast (10,9).
2. **`AddBuildingToTerrain_46570` / `sub_462A0` normal-recompute.** The per-cell terrain-normal/shading recompute
   that `sub_570F0` calls is treated as a black box here. Our renderer already derives shading from heights, so a
   port can recompute lighting for the whole dirty rect once rather than per-cell — but if we want byte-exact
   `mapShading` goldens we must reproduce `AddBuildingToTerrain_46570` exactly.
3. **`EuclideanDistXYZ_58490` exact fixed-point form.** Used as the disc test; assumed standard `sqrt(dx²+dy²+dz²)`
   in `<<8` world units (z included — the dome center z rises as it grows, so late-tick discs are slightly tighter).
   Worth a one-function verify before pinning geometry goldens, since the z-term subtly clips the rim.
4. **Player-cast entry for (10,9).** This trace covers level-authored (par1 → SPELLS) and endgame (forced) spawns.
   Whether MC2 lets the player CAST spell 18 (crater/raise-land) in normal play, and via which cast bridge, is not
   traced here — relevant only if we expose it in the spell selector.
