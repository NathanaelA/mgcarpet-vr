# MC2 Cave Level TERRAIN FOUNDATION — Verbatim Trace Report

The load-time pipeline that turns a cave level's data into **floor + ceiling geometry**, for Phase 4.5. All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, TR = `Terrain.cpp`); other files cited by full name:line. Trace date 2026-07-11.

**Builds on** (does NOT re-derive): `docs/traces/mc2-class10-high-band.md` (the (10,80..86) sculptor band), `docs/traces/mc2-terrain-author-painters.md` §4 (the (10,80)/(10,81) tube carver), `docs/traces/mc2-night-environment.md` (night/cave palette + sky). This report covers the *foundation those sculptors run on top of*: the two-heightmap build, the floor↔ceiling invariant, the renderer's ceiling path, and the load order.

---

## Headline findings (read first)

1. **A cave map is a DUAL heightmap.** `mapHeightmap_11B4E0` = the cave FLOOR (walkable ground you fly above), `x_BYTE_14B4E0_second_heightmap` = the cave CEILING (rock roof you fly under). Both are 256×256 bytes; both render at world height **32× their byte value**. Non-cave levels leave `x_BYTE_14B4E0` unused (it doubles as the sky buffer, `off_D41A8_sky`, engine_support.cpp:1008/1084).

2. **`isCaveLevel_D41B6` is set once, from the level-header MapType byte.** MapType 0/1/2 = Day/Night/Cave (ConvertMapInfo.cpp:7). `LevelInit_56C00` clears the flag, then on `MapType::Cave` sets `isCaveLevel_D41B6 = 1` AND `MapBasicHeight_D41B7 = levelData->byte_0x2FED3` (LevelInit.cpp:11/32-36). Non-cave leaves `MapBasicHeight_D41B7` at its constant default **44** (TR:15).

3. **`sub_43B40` (TR:1158) is the cave ceiling build; `sub_43D50` (TR:1183) is the non-cave angle build.** They are the mutually-exclusive last step of `GenerateTerrainAngles` (TR:51-54). `sub_43B40` **inverts the floor into a ceiling**: `ceiling = MapBasicHeight − min(floor, MapBasicHeight)`, then adds ±3 noise (`sub_43BB0`, TR:1546) and enforces the floor↔ceiling invariant. `sub_43D50` never touches the second heightmap — it only classifies water-edge/coast angles (`mapAngle & 8` there = *open-sea* flag, a different meaning from cave).

4. **The floor↔ceiling INVARIANT is: `ceiling` must stay `> floor`; where it isn't, `ceiling = floor − 1` and the cell is SEALED (`mapAngle |= 8`).** Every writer re-asserts this immediately after touching either array: `sub_43B40` (TR:1169-1177), `sub_43BB0` (TR:1565-1573), `sub_45DC0` retile (TR:1877-1912), the (10,8x) sculptors, `sub_570F0` callers, the building-placement raise (EF:27127-27136). **`mapAngle` bit3 (0x08) = "sealed / uncarved solid rock"** on cave levels (opposite polarity to non-cave, where bit3 = open sea).

5. **The ceiling renders in a CAVE-ONLY pass** in each `GameRender*`. The renderer computes floor projection from `alt_4 = 32*mapHeightmap − posZ` and ceiling projection from `inverse_alt_8 = (second_heightmap << 5) − posZ` (GameRenderNG.cpp:817-818), then draws the ceiling triangles FIRST using a **fixed texture tile (DDF50 index 1)** and the floor triangles second using the per-cell material tile (GameRenderNG.cpp:626/676). **Caves draw NO sky** — the sky band is memset to a flat key color (GameRenderNG.cpp:512).

6. **The ceiling is a hard flight ceiling.** `sub_10C60` (TR:2158) bilinearly samples `32*second_heightmap` = the ceiling world height. Flight/collision clamps the flyer to stay ≥576 below it and treats a SEALED cell (`mapAngle & 8`) exactly like water — impassable (EF:59513-59521, EF:59861-59865).

7. **BLDGPRM flag 4 = "no-cave-raise" CONFIRMED.** When a building is placed on a cave level and flag 4 is CLEAR (`!(bldgprm.byte_2 & 4)`), the placement raises the ceiling to `max(floor, footprint_base) + 80` over the footprint so the building has headroom / is enterable (EF:27089-27137, EF:27251). Flag 4 SET = leave the ceiling alone (building stays sealed in rock).

8. **47 of 165 shipped MC2 levels are caves** (baked-data census, §9). All carry the (10,80..86) sculptor band plus cave-only mobs (5,24)/(2,6)/(14,2).

**OPEN count: 6** (see last section).

---

## 1. `isCaveLevel` derivation + everything keyed off MapType at load

### 1.1 Header byte → MapType enum
```c
// ConvertMapInfo.cpp:7
to->MapType = (from->MapType == 2) ? MapType_t::Cave
            : (from->MapType == 1) ? MapType_t::Night
            :                        MapType_t::Day;
```
(Same law, Basic.cpp:3104.) The stored header byte is 0/1/2 = Day/Night/Cave. The reverse (`Basic.cpp:3297`) writes the underlying ordinal back.

### 1.2 `LevelInit_56C00` (LevelInit.cpp:9) — the ONLY isCaveLevel setter
```c
void LevelInit_56C00(Type_Level_2FECE* levelData) {
    isCaveLevel_D41B6 = 0;
    /* ... spell-row patching for freeze/... , Day vs non-Day ... */
    if (levelData->MapType == MapType_t::Day) {
        ... transparency=0; LoadSounds_84300(0); CURSOR_SPRITE_INDEX_D419E = 1;
    } else if (levelData->MapType == MapType_t::Night) {
        ... transparency=0; LoadSounds_84300(1); CURSOR_SPRITE_INDEX_D419E = 9;
    } else if (levelData->MapType == MapType_t::Cave) {
        D41A0_0.m_GameSettings.str_0x2196.transparency_0x2198 = 1;
        isCaveLevel_D41B6 = 1;
        MapBasicHeight_D41B7 = levelData->byte_0x2FED3;   // <-- cave sea/basic level
        LoadSounds_84300(2u);
        CURSOR_SPRITE_INDEX_D419E = 10;
    }
    ...
}
```
`isCaveLevel_D41B6` is a plain global (BasicTerrain.cpp:2). It is READ in ~80 sites (grep above); the load-bearing ones for terrain: TR:51 (which angle build), TR:1875/2034 (retile invariant), EF:27089/27251 (building ceiling-raise), EF:36332-36448 (all cave-sculptor ctors gate on it), EF:40113 (ambient drip spawner), and the flight/collision ceiling clamps EF:59513/59861.

### 1.3 Everything else keyed off MapType at load
- **Palette** (`LoadFixedMenuGraphics`, Level.cpp:900): `DATA/PALC-0.DAT` (cave) vs PALD/PALN.
- **HSPR sprite bank** (Level.cpp:460): `HSPRC0-0.DAT`.
- **Folder** (Level.cpp:449): `CAVE`.
- **Terrain texture atlas** (`LoadTextureData`, Level.cpp:1444): `HWEBC0-0.DAT`/`.TAB` (cave) vs HWEBD/HWEBN.
- **TMAPS block set** (`sub_71A70_setTmaps`, Level.cpp:1604): `x_DWORD_DB748_tmaps20file` — cave = tmaps **2**0 (day=00, night=10, cave=20). This confirms the *TMAPS digit = MapType ordinal* rule already noted in mgc-import bundle.rs:142.
- **Sounds** (`LoadSounds_84300(2)`), **cursor** (sprite 10).
- **Freeze spell** (spell 25) is cave-only usable (`isCaveLevel || spell != 25`, EF:22470/43883).

---

## 2. `sub_43B40` (cave) vs `sub_43D50` (non-cave) — the second-heightmap build

### 2.1 Where they run (TR:19-55, `GenerateTerrainAngles`)
```c
void GenerateLevelMap_43830(Type_Level_2FECE* a2x) {   // TR:19 — the master terrain pipeline
    rand2_17B4E0 = a2x->seed_0x2FEE5;  D41A0_0.rand_0x8 = a2x->seed_0x2FEE5;
    memset(mapEntityIndex_15B4E0, 0, 0x20000);
    sub_B5E70_decompress_terrain_map_level(seed, offset, raise, gnarl);   // decompress + fractal-gen the FLOOR heightmap
    sub_44DB0_truncTerrainHeight(...);                 // trunc/create
    memset(mapEntityIndex_15B4E0, 0, 0x20000);
    sub_44E40(river, lriver);                          // add river/lake fields into the floor
    sub_45AA0_setMax4Tiles(); sub_440D0(snLin); sub_45060(snFlt,bhLin);
    sub_44320(); sub_45210(snFlt,bhLin);
    sub_454F0(source, rkSte); sub_45600(bhFlt);
    sub_43FC0();
    memset(mapTerrainType_10B4E0, 0, 0x10000);
    sub_43970();   // smooth terrain
    sub_43EE0();   // add rivers
    sub_44580();   // set angle of terrain (builds building_F2CD0x tile LUT + per-cell angles)
    if (isCaveLevel_D41B6)
        sub_43B40();   // CAVE: build the ceiling second-heightmap
    else
        sub_43D50();   // non-cave: coast/open-sea angle classify (leaves second-heightmap untouched)
    sub_44D00();
}
```
So at the point `sub_43B40` runs, `mapHeightmap_11B4E0` (the FLOOR) is fully built (fractal gen + rivers + smoothing), `mapTerrainType`/`mapAngle` are classified, and `x_BYTE_14B4E0` (ceiling) is still whatever was left over from the sky buffer — **`sub_43B40` initializes the ceiling from scratch every cave load.**

### 2.2 `sub_43B40` verbatim (TR:1158)
```c
void sub_43B40() {
    uint8_t locHeight;  uaxis_2d index;
    for (int i = 0; i < 256 * 256; i++) {
        index.word = i;
        locHeight = mapHeightmap_11B4E0[index.word];
        if (locHeight > MapBasicHeight_D41B7)
            locHeight = MapBasicHeight_D41B7;              // clamp floor to the basic height
        x_BYTE_14B4E0_second_heightmap[index.word] = MapBasicHeight_D41B7 - locHeight;   // CEILING = mirror of floor about MapBasicHeight
        if (MapBasicHeight_D41B7 - locHeight > mapHeightmap_11B4E0[index.word])
            mapAngle_13B4E0[index.word] &= 0xF7u;          // ceiling ABOVE floor => OPEN cell (clear bit3)
        else {
            x_BYTE_14B4E0_second_heightmap[index.word] = mapHeightmap_11B4E0[index.word] - 1;  // pin ceiling just below floor
            mapAngle_13B4E0[index.word] |= 8;              // SEALED cell (set bit3)
        }
    }
    sub_43BB0();
}
```
**Initial ceiling law:** `ceiling(x,y) = MapBasicHeight − min(floor(x,y), MapBasicHeight)`. Where the floor is low, the ceiling is high → a tall open cavern. Where the floor rises to/above `MapBasicHeight`, `ceiling ≤ floor` → the cell is SEALED (solid rock: ceiling pinned to `floor−1`, bit3 set). **This is the "sealed rock everywhere, carve caverns down into it" starting state.** No RNG in this pass. Uses `MapBasicHeight_D41B7` (the header `byte_0x2FED3`, §4) as the mirror pivot.

### 2.3 `sub_43BB0` verbatim (TR:1546) — ceiling noise + re-assert invariant
```c
void sub_43BB0() {
    int fuzzyHeight;
    unsigned int randSeed = 37487429;                    // FIXED seed (not the level seed!)
    for (int i = 0; i < 256 * 256; i++) {
        if (!(mapAngle_13B4E0[i] & 8)) {                 // only OPEN cells get roughened
            randSeed = 9377 * randSeed + 9439;           // the standard LCG
            fuzzyHeight = randSeed % 7 - 3 + x_BYTE_14B4E0_second_heightmap[i];   // +/- 3 jitter
            if (fuzzyHeight < 0)   fuzzyHeight = 0;
            if (fuzzyHeight > 254) fuzzyHeight = 254;
            x_BYTE_14B4E0_second_heightmap[i] = fuzzyHeight;
        }
    }
    for (int i = 0; i < 256 * 256; i++) {                // re-assert the invariant after jitter
        if (x_BYTE_14B4E0_second_heightmap[i] > mapHeightmap_11B4E0[i])
            mapAngle_13B4E0[i] &= 0xF7u;                 // still open
        else {
            x_BYTE_14B4E0_second_heightmap[i] = mapHeightmap_11B4E0[i] - 1;
            mapAngle_13B4E0[i] |= 8;                     // became sealed
        }
    }
}
```
**RNG law is the standard `r = 9377*r + 9439`, BUT the seed is the hard constant `37487429`, NOT the level seed** — so the ceiling roughness is deterministic and identical across every cave level (only the open/sealed mask, which gates which cells advance the LCG, varies). The port MUST use this exact constant and only advance the LCG on open cells, in row-major order, to match baked ceiling goldens. Jitter is `±3` on open ceiling cells only.

### 2.4 `sub_43D50` (non-cave) by contrast (TR:1183)
`sub_43D50` **never writes `x_BYTE_14B4E0`.** It walks every cell, clears `mapAngle & 8` (TR:1198), and for zero-height (sea-level) cells does an 8-neighbour + terrain-type scan to decide whether the cell is open sea vs a coast/inlet, setting `mapAngle` bit3 accordingly. On non-cave levels bit3 therefore means **open sea** (used by water rendering + die-on-water), a completely different semantic from the cave "sealed rock" meaning. This is the polarity trap: same bit, opposite meaning by MapType.

---

## 3. The floor↔ceiling invariant + its enforcers

**Invariant (cave levels only):** for every cell, either
- `ceiling > floor` → **OPEN**: `mapAngle & 8 == 0`, or
- `ceiling ≤ floor` → **SEALED**: force `ceiling = floor − 1` and set `mapAngle |= 8`.

It is a coupled write: you never write floor or ceiling without re-running this two-line check on the touched cell. Enforcers found:

| enforcer | site | what it does |
|---|---|---|
| `sub_43B40` | TR:1169-1177 | initial build |
| `sub_43BB0` | TR:1565-1573 | after ceiling jitter |
| `sub_45DC0` retile | TR:1875-1912 | after any per-cell terrain reclassify, on the cell + its 3 forward neighbours (a 2×2 block) — `if(isCaveLevel)` guarded |
| `sub_44D00` bulk | TR:2034-2036 | `if(isCaveLevel && ceiling<=floor) ceiling=floor-1` |
| (10,52) box mesa | EF:25219-25232 | after raising ceiling / lowering floor |
| (10,53) dome / (10,55) hill | EF:25522, 25703-25710 | after ramped floor raise |
| (10,54) pit | EF:25313-25323 | after ceiling dig |
| (10,81) tube carver | EF:25231-25232 (and see mc2-terrain-author-painters §4) | after each disc: `if(ceiling>floor) clear bit3 else {ceiling=floor-1; set bit3}` |
| building raise | EF:27127-27136 | after ceiling +80 |

### 3.1 `sub_570F0` (EF:39602) — the FLOOR write primitive
Signature `char sub_570F0(x, y, height, protectAngle a4, forceType1 a5, edgeStamp a6)`. Body (spine):
```c
    if (a3 > 255) { a3 = 255; if (!a1&&!a2) v8=1; }      // clamp height 0..255
    if (a3 < 0)   { a3 = 0;   if (!a1&&!a2) v8=1; }
    if (a4 && mapAngle_13B4E0[v9x.word] < 0) return 1;   // a4 => skip if bit7 (authored/locked) set
    mapHeightmap_11B4E0[v9x.word] = a3;                  // WRITE THE FLOOR
    if (a5 || sub_57450(mapTerrainType_10B4E0[v9x.word]))
        mapAngle_13B4E0[v9x.word] = mapAngle & 0xF8 | 1; // a5/water-type => mark walkable type-1
    if (!a3) { ... a6 edge-stamp neighbours via sub_56EE0 ... }   // height 0 + a6 => coast/edge flood
```
**Note `sub_570F0` writes ONLY the floor.** It does NOT itself re-assert the ceiling invariant — the *caller* does that on the surrounding cells (this is why every sculptor pairs `sub_570F0` with an inline `ceiling>floor?` block). `protectAngle` (a4) respects the `mapAngle bit7` authored-lock; `forceType1` (a5) forces walkable-type-1; `edgeStamp` (a6) triggers the coast neighbour flood when height hits 0.

### 3.2 `sub_34B00` (EF:25339) — the carved-box retile
Given box (a1,a2,w=a3,h=a4), walks the box top edge, bottom edge, left column, right column; for each border cell that is currently open (`mapAngle & 8`... **note: here `&8` reads as "still sealed"** — the carve set bit3 clear on interior, so a border cell still `&8` is the cavern wall) it stamps `mapTerrainType = 1` (wall material), forces `mapAngle & 0xF8 | 1` (walkable-type-1), and calls `sub_462A0(cell,cell)` to re-tile+re-shade. **This paints the CAVE WALL RING around a carved region** — the visible rock face where the carved cavern meets sealed rock. Used by the box mesa (10,52), the tube carver (10,81), and the pit finalize (10,54).

---

## 4. Cave sea level / water / lava

### 4.1 `MapBasicHeight_D41B7` = the cave "basic height" (header `byte_0x2FED3`)
- Default **44** (TR:15), used unchanged on non-cave levels.
- On cave load, `= levelData->byte_0x2FED3` (LevelInit.cpp:36).
- Consumed **only in `sub_43B40`** (TR:1166-1169) as the mirror pivot for the ceiling build. It is NOT a water-plane level: there is no cave water-plane render keyed to it. It sets *how much headroom* the cavern gets (ceiling = pivot − floor) and the sealed/open threshold (`floor ≥ pivot` ⇒ sealed).

### 4.2 Do caves have water / lava?
- **Water type-8 still exists in caves.** `sub_44E40(river, lriver)` runs unconditionally in the pipeline (TR:29) → cave levels can carry rivers/lakes baked into the FLOOR heightmap (terrain type 8). The (10,80)/(10,81) carvers explicitly guard water cells (`mapTerrainType != 8 || sub_33F70`, see painters trace §2.5). So a cave can have water on its floor.
- **`sub_104D0_terrain_tile_is_water` == 256** is the water-tile test used for die-on-water; it fires on cave floors with water tiles the same as open levels (EF:59500, 59861).
- **Die-on-water in caves is EXTENDED:** `sub_5DD50` (EF:59861) — a mover "dies/reflects" if `is_water==256` **OR** (`isCaveLevel && mapAngle & 8`) i.e. the cell is SEALED rock. **A sealed cave cell is treated exactly like water for movement collision** (bounces the flyer back, EF:59873-59876). So "solid rock wall" and "water" share the impassable path.
- **Lava** ("16. Darklava", EF:2319) is a level *name*, not a distinct cave terrain array — lava tiles are terrain-type material codes (10/11/12 handled in `sub_45DC0` cases, TR:1832 type 0xF→11), not cave-specific. No separate cave-lava machinery.

### 4.3 Wave rendering in caves
The floor water wobble is computed in the tile projector: `tempSinXSin = sin_DB750[...]^2` applied to `alt_4` for cells with `mapTerrainType == 0` (GameRenderNG.cpp:820-826). This runs in the cave path too (the cave block reuses the same per-tile projector loop, GameRenderNG.cpp:802-849) — cave floor water animates identically. There is no separate cave water plane.

---

## 5. CEILING RENDERING

### 5.1 The projector fills both floor + ceiling projected coords (GameRenderNG.cpp:814-847)
```c
    Str_E9C38_smalltit[i].alt_4         = 32 * mapHeightmap_11B4E0[yawXY.word] - posZ;               // FLOOR world height
    Str_E9C38_smalltit[i].inverse_alt_8 = (x_BYTE_14B4E0_second_heightmap[yawXY.word] << 15 >> 10) - posZ;  // CEILING world height
```
`<<15 >>10` = `<<5` = **×32**, so ceiling world height = `32 * second_heightmap − posZ`, **same scale/offset as the floor**. The two are then perspective-projected to screen Y: `pnt2_20` (floor) and `pnt4_28` (ceiling) via `dword0x22 + dword0x18 * alt / tempY` (GameRenderNG.cpp:838-839). `pnt1/pnt3` are the two X columns (floor/ceiling share X).

### 5.2 The cave-only ceiling triangle pass (GameRenderNG.cpp:526-690)
`if (isCaveLevel_D41B6)` opens a dedicated render block that, per quad, draws **TWO** triangle passes:
- **Ceiling pass FIRST** (GameRenderNG.cpp:590-638): uses `pnt3/pnt4` (the ceiling-projected corners) + `pnt5_32` shading, gated by `triangleFeatures & 0x80` (the sealed-cell flag propagated from `mapAngle & 8` at GameRenderNG.cpp:836-837). Texture is hard-wired: `x_DWORD_DE55C_ActTexture = x_DWORD_DDF50_texture_adresses[1]` (GameRenderNG.cpp:626). Winding is flipped vs the floor (`B6253(&p1,&p4,&p2); B6253(&p4,&p3,&p2)`), so the roof faces down.
- **Floor pass SECOND** (GameRenderNG.cpp:641-690): uses `pnt1/pnt2`, texture `x_DWORD_DDF50_texture_adresses[textIndex_41]` (per-cell material, GameRenderNG.cpp:676) — the ordinary ground.

The non-cave render path (below, GameRenderNG.cpp:850+) draws only the floor pass and a water plane; it never touches `inverse_alt_8`/`pnt4`. So the ceiling pass is strictly cave-gated. (GameRenderOriginal.cpp:1052 and GameRenderHD.cpp:820/1231/1322 mirror this — index [1] ceiling texture, `<<15>>10` scale.)

### 5.3 Ceiling texture source + shading
- **Texture:** `x_DWORD_DDF50_texture_adresses` is a flat 256-entry LUT of pointers into the current TMAPS atlas (`BLOCK32DAT_BEGIN_BUFFER`), tiled `texture_size`×`texture_size` (EF:42800-42807). **Index [1] = the 2nd tile of the atlas row** — i.e. a single fixed ceiling tile from the cave TMAPS (tmaps20). The floor uses `textIndex_41` (the per-cell `mapTerrainType`), so the ceiling is uniform rock while the floor varies.
- **Shading:** the quad's `pnt5_32` = `(mapShading << 16) + 8*tempSinXSin` blended by fog (GameRenderNG.cpp:828-835). Both passes share the same `pnt5` (the cell's floor shading) — the ceiling reuses the floor cell's shade value. The night/cave shade INVERSION (`32−v+32`) noted in the prompt lives in the terrain-shade primitive `sub_46180`/`sub_462A0` when it writes `mapShading` (see painters trace §2.5), not in the renderer — the renderer just reads `mapShading_12B4E0` which was already inverted at bake time for non-Day maps.

### 5.4 Above/outside carved areas + no sky
- Sealed cells have `ceiling = floor − 1` and `mapAngle & 8` set → `triangleFeatures |= 0x80` (GameRenderNG.cpp:836) → the ceiling and floor collapse to nearly the same plane and the `& 0x80` gating (GameRenderNG.cpp:604/655) suppresses/degenerates the quad, producing the "solid rock, no cavern here" look (you can't fly there — collision blocks it per §4.2).
- **No sky:** GameRenderNG.cpp:512 `if (!m_wSky || isCaveLevel) memset the sky band to flat keyColor1_D4B7C` instead of `DrawSky_40950`. Confirmed in all three renderers (NG:512, Original:680, HD:698).

---

## 6. `mapAngle` bit3 (0x08) at load — all set/clear/read sites

| site | file:line | SET / CLEAR / READ | meaning on cave |
|---|---|---|---|
| `sub_43B40` | TR:1171 (clear) / 1176 (set) | both | initial open/sealed |
| `sub_43BB0` | TR:1567 (clear) / 1572 (set) | both | post-jitter re-assert |
| `sub_44D00` | TR:2034-2036 | set | bulk re-assert |
| `sub_45DC0` retile | TR:1879/1889/1899/1907 | both | per-cell 2×2 re-assert |
| (10,81) carver | EF (painters §4) | both | per carved disc |
| (10,52/53/54/55) | EF:25xxx | both | per sculpted cell |
| building raise | EF:27129/27133 | both | footprint headroom |
| renderer | GameRenderNG.cpp:836 READ | — | `&8 ⇒ triangleFeatures|=0x80` (sealed quad) |
| flight ceiling clamp | EF:59521 READ | — | `&8 ⇒ blocked` |
| die/reflect | EF:59865 READ | — | `&8 ⇒ impassable like water` |
| `sub_43D50` (NON-cave) | TR:1198 clear + coast sets | both | OPEN-SEA flag (different meaning) |

**A SEALED cell (bit3 set) differs from a carved cell in DATA:** `ceiling = floor−1` (no headroom) and it is impassable + renders as collapsed rock. **A carved (OPEN) cell** has `ceiling > floor` (real headroom), renders both floor + ceiling tiles, and is flyable. The `mapAngle bit3` is the single source of truth the renderer + collision both consult.

Note there is also a separate **`mapAngle bit7` (0x80) = authored/locked** flag (set by the road/river/carve painters, respected by `sub_570F0` arg a4) — distinct from bit3. Don't conflate them.

---

## 7. LOAD ORDER end-to-end (cave level)

Master entry `sub_56D60`/`LoadLevel` (EF:39380-39441, mirrored EF:39444-39464). Numbered, with the terrain sub-steps expanded:

1. **`LevelInit_56C00`** (EF:38286) — sets `isCaveLevel_D41B6=1`, `MapBasicHeight_D41B7 = byte_0x2FED3`, palette/sound/cursor mode. (LevelInit.cpp:32-38)
2. **`GenerateLevelMap_43830`** (EF:39385) — the terrain pipeline (TR:19):
   1. seed RNG from `seed_0x2FEE5` (TR:21-22)
   2. `sub_B5E70` decompress + fractal-gen the FLOOR heightmap (TR:24)
   3. `sub_44E40(river,lriver)` bake rivers/lakes into floor (TR:29)
   4. smoothing/gnarl passes (TR:31-45)
   5. `sub_43970` smooth, `sub_43EE0` rivers, `sub_44580` build angle LUT + per-cell angles (TR:47-50)
   6. **`sub_43B40`** build the CEILING from the floor + invariant (cave branch, TR:52)
   7. `sub_43BB0` ceiling ±3 jitter (seed 37487429) + re-assert invariant (TR:1179)
   8. `sub_44D00` final bulk pass (TR:55)
3. **`sub_49F30`** (EF:39386) — prepare event pointers (`PrepareEvents_49540` per THING, EV:166 — the `sub_49090` chain-divert for authored terrain THINGs, painters trace §5.2).
4. **`sub_49270_generate_level_features`** (EF:39392) → **`GenerateEvents_49290`** (Level.cpp:437, EV:152) — the 6 numbered GenerateEvents passes, each followed by `ApplyEvents_498A0` (EV:171/204/225/238/255/265/281). The cave sculptor band (10,80..86) + (14,2) risers spawn here and, in the **settle loop `ApplyEvents_498A0`** (EV:410), are TICKED to completion (the `>0x33 && (<0x50 || >0x55 && !=0x58)` disable-band deliberately excludes 0x50-0x55/0x58 — high-band trace §4.1). **This is where the cave floor/ceiling get sculpted into their final shape**, on top of the `sub_43B40` foundation. Cave-carver pass ordering (painters trace §5.1): rivers pass 2 → cave carvers pass 3 → roads pass 5.
5. **`sub_71A70_setTmaps(MapType)`** (EF:39402) — select cave TMAPS (tmaps20).
6. **`InitStages` / model init / sound / `sub_60F00`** (EF:39406-39437) — post-terrain.

**Port determinism requirements:** (a) `sub_43B40` is pure (no RNG); (b) `sub_43BB0` uses the FIXED seed `37487429` and advances the LCG only on open cells in row-major order; (c) the settle loop's per-entity RNG (the (10,54)/(10,55) random depth `rand_0x14_20 % a2` when `par3==0`, and the drip sprite pick) must reproduce entity-index seeding + draw order exactly (high-band trace OPEN-4/7). All three feed the baked cave-geometry state hash.

---

## 8. BLDGPRM flag 4 (no-cave-raise) — VERIFIED

The building placement/finalize routine (`ApplyTerrainModification_37240`, EF:27181, and its sibling at EF:27080) stamps the building footprint into the floor, then:
```c
    // EF:27089  (and identically EF:27251 in the life-countdown finalize path)
    if (isCaveLevel_D41B6 && !(str_D93C0_bldgprmbuffer[v1].byte_2 & 4))
        v29 = 1;                       // v29 = "raise the cave ceiling over this building"
    ...
    // per footprint cell, when v29:  EF:27114-27137
    if (v29) {
        v9  = mapHeightmap_11B4E0[cell];
        v10 = (v9 >= v25) ? v9 : v25;          // max(current floor, footprint base height v25)
        v11 = v10 + 80;  if (v11 > 255) v11 = 255;      // +80 headroom
        if (v11 > second_heightmap[cell]) second_heightmap[cell] = v11;   // RAISE CEILING
        // re-assert invariant:
        if (second_heightmap[cell] > mapHeightmap[cell]) mapAngle[cell] &= 0xF7;   // open
        else { second_heightmap[cell] = mapHeightmap[cell]-1; mapAngle[cell] |= 8; } // sealed
    }
```
**Confirmed reading:** `bldgprm.byte_2 & 4` = **no-cave-raise**. CLEAR (0) ⇒ raise the ceiling +80 above the building footprint so there's headroom to fly in and enter it (a building embedded in cave rock gets a carved bubble). SET (4) ⇒ skip the raise; the building stays buried in sealed rock with no headroom. The port's current bldgprm reading ("8 no-mana, 4 no-cave-raise, 1 enterable") is correct for flag 4. (Flag 1 "enterable" and flag 8 "no-mana" are gated elsewhere; not re-verified here — see OPEN-6.)

---

## 9. Shipped MC2 cave levels (baked-data census)

Ran `crates/mgc-sim/examples/tmp_cavecensus.rs` over `baked/mc2/*.mgcl` (165 levels), filtering `header.map_type == Cave` and counting cave-band THINGs. **47 of 165 levels are caves.** Full list (level file : gfx_type : total THINGs : cave-band {(class,model): count}):

```
level-003 gfx=0  918  {(2,6):114,(10,80):60,(10,82):5,(10,83):55,(10,84):75,(10,85):62,(10,86):7,(14,2):6}
level-005 gfx=0  746  {(2,6):6,(10,80):104,(10,82):6,(10,83):161,(10,84):29,(10,85):25,(14,2):3}
level-007 gfx=0  545  {(2,6):8,(10,80):54,(10,82):5,(10,83):44,(10,84):40,(10,85):29,(14,2):3}
level-011 gfx=0  836  {(2,6):56,(10,80):68,(10,82):8,(10,83):163,(10,84):34,(10,85):59,(14,2):12}
level-014 gfx=0  650  {(2,6):92,(5,24):61,(10,80):24,(10,82):2,(10,83):33,(10,84):5,(10,85):3,(14,2):32}
level-015 gfx=0  731  {(2,6):15,(5,24):69,(10,80):77,(10,82):13,(10,84):10,(10,85):8,(14,2):35}
level-020 gfx=0  918  {(2,6):80,(5,24):105,(10,82):21,(10,83):3,(10,84):17,(10,85):13}
level-023 gfx=0  503  {(2,6):66,(5,24):3,(10,80):33,(10,82):3,(10,83):5,(10,85):35,(14,2):2}
level-030 gfx=0  566  {(10,80):4,(10,82):6,(10,83):13,(10,84):2,(10,85):5,(14,2):3}
level-032 gfx=0  472  {(5,24):12,(10,80):12,(10,82):3,(10,83):9,(10,84):4,(10,85):1,(14,2):1}
level-033 gfx=0  676  {(2,6):118,(10,80):14,(10,82):10,(10,83):16,(10,84):19}
level-055 gfx=1  790  {(5,24):15,(10,80):171,(10,82):6}
level-066 gfx=1  811  {(10,80):215,(10,82):6,(10,83):19,(10,84):11,(10,85):4}
level-067 gfx=1  770  {(5,24):87,(10,80):177,(10,83):11,(10,84):7,(10,85):7,(14,2):4}
level-073 gfx=0  608  {(5,24):45,(10,80):57,(10,82):15,(10,83):27,(10,84):82,(10,85):18,(14,2):18}
level-074 gfx=0  538  {(5,24):12,(10,80):39,(10,82):13,(10,84):10,(10,85):8,(14,2):35}
level-077 gfx=1  567  {(2,6):72,(5,24):52,(10,80):21,(10,82):1,(10,83):2,(10,84):3,(10,85):2}
level-082 gfx=0  187  {(10,80):86,(10,82):30,(10,83):8}
level-085 gfx=0  677  {(10,80):206,(10,82):4,(10,83):19,(10,84):11,(10,85):4}
level-087 gfx=0  348  {(10,80):35,(10,82):5,(10,84):20,(10,85):25}
level-094 gfx=0  700  {(2,6):76,(10,80):71,(10,82):8,(10,83):164,(10,84):34,(10,85):59,(14,2):11}
level-095 gfx=0  538  {(2,6):6,(10,80):34,(10,82):8,(10,83):150,(10,84):53,(10,85):80,(14,2):11}
level-097 gfx=0  634  {(2,6):3,(10,80):80,(10,82):8,(10,83):155,(10,84):49,(10,85):45,(14,2):4}
level-105 gfx=1  621  {(5,24):8,(10,80):171,(10,82):6}
level-106 gfx=0    2  {}          <-- near-empty cave stub
level-107 gfx=0  466  {(5,24):4,(10,80):12,(10,82):3,(10,83):9,(10,84):4,(10,85):1,(14,2):1}
level-111 gfx=0  728  {(2,6):70,(10,80):68,(10,82):8,(10,83):164,(10,84):34,(10,85):59,(14,2):12}
level-113 gfx=1  686  {(5,24):57,(10,80):177,(10,83):11,(10,84):7,(10,85):7,(14,2):5}
level-114 gfx=0   84  {(10,80):83}
level-115 gfx=0   89  {(5,24):9,(10,80):10,(10,83):3,(10,84):1,(10,85):1}
level-116 gfx=1  472  {(2,6):107,(5,24):13,(10,80):81,(10,82):5,(10,83):10,(10,84):21,(10,85):17}
level-117 gfx=0  678  {(10,80):206,(10,82):4,(10,83):19,(10,84):11,(10,85):4}
level-123 gfx=1  567  {(2,6):72,(5,24):52,(10,80):24,(10,82):2,(10,83):33,(10,84):3,(10,85):2,(14,2):32}
level-125 gfx=0  575  {(2,6):118,(10,80):14,(10,82):10,(10,83):16,(10,84):19}
level-127 gfx=1  448  {(2,6):38,(10,80):60,(10,82):19}
level-131 gfx=0  196  {(10,80):4,(10,82):17,(10,84):3,(10,85):4}
level-132 gfx=1  514  {(2,6):38,(10,80):60,(10,82):18}
level-135 gfx=0  378  {(2,6):21,(10,80):16,(10,82):9,(10,83):2,(10,84):13,(10,85):7,(14,2):4}
level-137 gfx=0  443  {(10,80):35,(10,82):5,(10,83):35,(10,84):75,(10,85):47}
level-142 gfx=0  919  {(2,6):97,(10,80):35,(10,82):5,(10,83):65,(10,84):75,(10,85):47}
level-143 gfx=0  621  {(2,6):3,(10,80):80,(10,82):8,(10,83):152,(10,84):49,(10,85):45,(14,2):4}
level-144 gfx=0  620  {(10,80):80,(10,82):8,(10,83):44,(10,84):49,(10,85):45,(14,2):4}
level-146 gfx=0  918  {(2,6):97,(10,80):35,(10,82):5,(10,83):65,(10,84):75,(10,85):47}
level-147 gfx=0  327  {(2,6):8,(10,80):28,(10,82):4,(10,83):3,(10,84):40,(10,85):29,(14,2):2}
level-155 gfx=0  265  {(10,80):43,(10,82):3,(10,83):4,(10,84):3}
level-157 gfx=0  266  {(10,80):43,(10,82):3,(10,83):4,(10,84):3}
level-164 gfx=0   74  {(10,80):26,(10,82):5}
```
(All 47 rows now enumerated — the original 38-row list was a truncated-terminal artifact (the first 8 rows had scrolled off `tail -40`), CLOSED as OPEN-5. Note level-003 — the THIRD campaign level — is a cave carrying the campaign's only seven authored (10,86) drips and 114 cave bees; levels 020 and 023 author NO (10,80) tunnel chains at all or only a handful, so the foundation mirror + non-tunnel sculptors must stand alone there.) Note the campaign-selection law (D/N/C filename suffix, Level.cpp:449-460, `DAY`/`NIGHT`/`CAVE` + `HSPRD/N/C`) is what the importer already keys `map_type` off of (bake.rs:303-306). **level-106 is a 2-THING near-empty cave stub** (likely a broken/placeholder level — flag when grinding).

---

## Constants table

| item | value | source |
|---|---|---|
| MapType byte | 0 Day / 1 Night / 2 Cave | ConvertMapInfo.cpp:7 |
| `isCaveLevel_D41B6` setter | only in `LevelInit_56C00` cave branch | LevelInit.cpp:35 |
| cave basic/sea height | `MapBasicHeight_D41B7 = byte_0x2FED3` (cave), else const **44** | LevelInit.cpp:36; TR:15 |
| floor array | `mapHeightmap_11B4E0` (256×256 u8) | TR:7 |
| ceiling array | `x_BYTE_14B4E0_second_heightmap` (256×256 u8; = sky buffer off-cave) | BasicTerrain.cpp:3; engine_support.cpp:1008 |
| initial ceiling law | `ceiling = MapBasicHeight − min(floor, MapBasicHeight)` | TR:1166-1168 |
| invariant | `ceiling>floor ⇒ open (bit3 clear); else ceiling=floor−1, bit3 set` | TR:1169-1177 |
| ceiling jitter | `±3`, LCG seed **37487429** (fixed, NOT level seed), open cells only, row-major | TR:1549-1560 |
| LCG law | `r = 9377*r + 9439` | TR:1554 (universal) |
| floor world height | `32 * mapHeightmap` | GameRenderNG.cpp:817 |
| ceiling world height | `32 * second_heightmap` (`<<15>>10`); bilinear `sub_10C60` | GameRenderNG.cpp:818; TR:2158-2222 |
| ceiling texture | `DDF50[1]` (fixed atlas tile 1) | GameRenderNG.cpp:626 |
| floor texture | `DDF50[textIndex_41]` (per-cell material) | GameRenderNG.cpp:676 |
| cave TMAPS | tmaps**20** (day 00 / night 10 / cave 20) | Level.cpp:1604 |
| cave textures/palette | HWEBC0-0 / PALC-0 / HSPRC0-0 | Level.cpp:1444/902/460 |
| sky in cave | none — flat `keyColor1_D4B7C` memset | GameRenderNG.cpp:512 |
| `mapAngle` bit3 (cave) | 0x08 = SEALED rock (open = clear) | §6 |
| `mapAngle` bit3 (non-cave) | 0x08 = OPEN SEA (opposite meaning) | TR:1198 |
| `mapAngle` bit7 | 0x80 = authored/locked (distinct from bit3) | EF:39650 |
| floor write primitive | `sub_570F0(x,y,h,protectAngle,forceType1,edge)` — floor only | EF:39602 |
| carved wall ring | `sub_34B00` → type-1 wall + `sub_462A0` reshade | EF:25339 |
| flight ceiling clamp | blocked if within 576 of `sub_10C60` or cell `&8` | EF:59521 |
| die/reflect in cave | water==256 OR (`isCaveLevel && mapAngle&8`) | EF:59861-59865 |
| bldgprm flag 4 | no-cave-raise: clear ⇒ ceiling +80 over footprint | EF:27089-27137 |
| shipped cave levels | 47 / 165 (list §9) | census `tmp_cavecensus.rs` |
| load order | LevelInit → GenerateLevelMap(→43B40→43BB0) → GenerateEvents/settle → setTmaps | EF:39380-39441 |

---

## OPEN items

1. **`sub_43B40` runs BEFORE the settle-loop sculptors — order confirmed, but the interaction is layered, not verified end-to-end.** `sub_43B40` builds the *foundation* ceiling (mirror of floor); the (10,80..86) band then carves specific caverns into it during `ApplyEvents`. I have both halves cited (§2, §7) but have NOT produced a baked golden proving the composite floor+ceiling matches retail. **A cave-level state-hash golden after the settle loop is the acceptance test for the whole port.**

2. **`sub_43BB0` fixed seed 37487429 — provenance.** It's a hard constant in the decompile (TR:1549), NOT derived from the level seed. Plausible it's a genuine shipped constant, but the decompile could have inlined a global. Confirm against a second cave level's baked ceiling (the jitter pattern should be identical up to the open/sealed mask). If it diverges, the seed is really level-derived.

3. **Ceiling shade inversion.** §5.3 asserts the `32−v+32` night/cave inversion happens at bake time in `sub_46180`/`sub_462A0` (mapShading write) and the renderer just reads it. I did NOT transcribe that shade-write inline here (it's in the painters-trace scope). Verify the ceiling's reused-floor-shade actually looks right (the ceiling reads the FLOOR cell's `mapShading`, GameRenderNG.cpp:804/828 — it may want a distinct ceiling shade the retail engine doesn't compute; confirm visually or against a screenshot before judging faithfulness).

4. **`MapBasicHeight_D41B7` as anything beyond the ceiling pivot.** I found it consumed ONLY in `sub_43B40`. If a cave "water/lava sea level" exists it is NOT keyed to this byte in the terrain code — but I did not exhaustively grep the *gameplay* water-death for a cave-specific sea constant. Low risk (die-on-water uses the type-8 tile test, not a height), but flagged.

5. ~~**The census printed 38 rows but reports 47 caves.**~~ CLOSED same day: stdout truncation (`tail -40` ate the first 8 rows). Full 47-row list now in §9; the 47th row is the `level-106` stub already flagged.

6. **BLDGPRM flags 1 (enterable) and 8 (no-mana) not re-verified here** — only flag 4 was traced to its consumer. The prompt's flag-4 reading is confirmed; the other two ride on the same byte and should be cross-checked against their own consumers before trusting the full bldgprm bit layout for cave building placement.
