# MC2 Day/Night/Cave Environment System — Classification, Sky, Fog, Map, Night Consequences

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, GRO = `GameRenderOriginal.cpp` — the faithful software renderer; RD = `ReadAndDecompress.cpp`, LI = `LevelInit.cpp`, PI = `PlayerInput.cpp`). Data analysis performed on the actual retail CD image (`gamedata/Magic Carpet 2/game.gog`, ISO9660 walked directly: `DATA/SKY{D,N,C}0-0.DAT`, `PAL{D,N,C,F}-0.DAT`) and on our correctly-carved (post shade-LUT fix) `baked/assets/mc2-{day,night,night-fog,cave}/shade-lut.bin` + `palette.bin`. Trace date 2026-07-10.

**Headline findings (read first):**

1. **Classification source of truth = a byte in the per-level header inside `LEVELS/LEVELS.DAT`.** Raw byte at header offset 6 (`x_D41A0_BYTEARRAY_0[196308]` = struct offset 0x2FED4): 0=Day, 1=Night, 2=Cave (`DecompressLevel_2FECE`, Basic.cpp:3104; struct LevelStructs.h:284-288). Byte at offset 4 (`byte_0x2FED2`, "type of level graphics") bit 1 = the **FOG night sub-variant** (swaps atlas BL32F + palette PALF, keeps TABLESN/TMAPS1/SKYN — RD:63-92, Level.cpp:886-899). **Level 000 = Night, gfx_type 0** (data-verified from retail LEVELS.DAT via our importer; header field `map_type:"night"` in `baked/mc2/level-000.mgcl`). Retail tally over all 165 entries: 77 day (3 carry an inert bit-1 — the fog check lives inside the Night case only), 36 night, 5 night-fog (024, 027, 046, 062, 122), 47 cave.
2. **The retail sky is a 256×256 8bpp CLOUD-PLANE BITMAP, not a flat fill and not LUT row 0** — `DrawSky_40950` (GRO:258-373) texture-maps `off_D41A8_sky` (SKYD0-0.DAT day / SKYN0-0.DAT night, RD:40-42/89-91) across the whole viewport with roll rotation, yaw scroll and pitch offset, drawn FIRST, terrain painted over. **Cave loads no sky** (RD:115-137) and, like sky-option-off, **flat-fills with `keyColor1_D4B7C`** (GRO:680-700): day 0xFE = pale blue (176,188,252), night 0x00 = black, cave 0xFE = black in PALC (RD:153-168 + palette data). **Our "shade-LUT row-0 mode" model is numerically EXACT as the sky's base color** — measured: day LUT row 0 is 254/256× palette index 254, and SKYD is 74% that same index (plus white cumulus); night row 0 is 232/256× index 64 = (0,0,0), and SKYN is 94.5% that index (plus faint moonlit cloud rims — **no stars, no moon sprite**; brightest pixel lum < 150). The faithful upgrade = bake the SKY bitmaps.
3. **Fog constants are IDENTICAL in all environments** — set every frame in `DrawTerrainAndParticles_3C080` (GRO:668-679): FogStart = 14745600 = 3840², FogEnd = 23658496 = 4864², Thickness = FogEnd−FogStart, tile cutoff = 26214400 = 5120². The entire day/night/cave look difference is **table + palette DATA**. The ramp is **linear in squared distance** (not exponential): factor = clamp((4864²−d²)/(4864²−3840²), 0, 1).
4. **The shade LUT's brightness polarity INVERTS between day and night/cave (measured):** TABLESD rows darken monotonically 0→63 (row 0 = pale-blue fog, mean lum 194; row 32 identity; row 63 black), TABLESN/F/C rows brighten monotonically (row 0 = black fog, lum 9; row 32 identity; row 63 brightest, lum 132). Retail code depends on this: terrain shading stores `64 − shade` on non-Day (Terrain.cpp:1281-1288, 2030-2033 — double inversion keeps the physical light direction identical), scorch = row 63 day / row 1 night (EF:23307-23315), sprite shadows go `0x2000 + fog/4` day vs `0x2000 − fog/4` night (`notDay_D4320`, GRO:585, 3443-3446), and **dynamic lights ADD rows toward 63 — meaningful only where up = brighter, i.e. night/cave**.
5. **Big-map law confirmed verbatim:** non-cave pixel = `tables[mapShading[cell]<<8 | tables[0x14000 + mapTerrainType[cell]]]` (`sub_63670_draw_minimap_a`, GameUI.cpp:2544-2554) — exactly our `palette[shade_lut[shade][tile_colors[type]]]`. Sea noise is dark at night for no special reason: flat cells (all open sea) get random shading rows 28..36 (`sub_44D00`, Terrain.cpp:1242-1290 — the night `64−x` maps [28..36] onto itself), and TABLESN rows 28..36 keep the already-dark PALN sea colors near identity.
6. **Night/Cave gameplay ledger (beyond visuals):** the 50-slot dynamic-light system (Night/Cave only, EF:47182-47195/47213/47385/47612); per-environment sound bank 0/1/2 (LI:23/29/37) and music program 2/1/3 (EF:31440-31451); per-environment user brightness settings (PI:2495-2505); spell 25 Cave-In selectable only in caves (PI:849); spells 4/19 subspell-0 life 2 (day) vs 19 (night/cave) + hint swap (LI:12-21); (5,2) creature refuses to spawn off-Day (EF:33758-59); day-only GTD2.DAT mid-level table swap (EF:60449-60453); no occluding water surface off-cave (Terrain.cpp:50-55) — the roadmap note stands.

---

## 1. Classification — the exact source of truth

### 1.1 Level load chain
`LevelDecompress_533B0(levelIndex, levelData, customLevelPath)` (EF:38225-38290):
- opens `CLEVELS/LEVELS.DAT|TAB` (game dir) falling back to `LEVELS/LEVELS.DAT|TAB` (CD) (EF:38229-38251);
- LEVELS.TAB = 4000 bytes = 1000 `u32` offsets; entry `[levelIndex] .. [levelIndex+1]` is one RNC-compressed level blob (EF:38252-38259);
- decompressed header (`Type_CompressedLevel_2FECE`, 0x6604 bytes) is field-copied by `DecompressLevel_2FECE` (Basic.cpp:3100-3116);
- then `LevelInit_56C00(levelData)` (LI:9-52) applies the per-environment side effects (§1.3).

### 1.2 The header fields (LevelStructs.h:284-296)

| offset | field | meaning |
|---|---|---|
| 0 | `word_2FECE` | (header word) |
| 2 | `levelID_2FED0` | level id |
| **4** | `byte_0x2FED2` | gfx flags — **bit 1 (&2) = FOG variant**, tested only on Night (RD:63/78/95; Level.cpp:888; EF:31905) |
| 5 | `byte_0x2FED3` | **cave base/ceiling height** → `MapBasicHeight_D41B7` (LI:36; default 44 = the off-cave "sea level", Terrain.cpp:14) |
| **6** | `MapType` | **0 = Day, 1 = Night, 2 = Cave** (mapped to the enum at Basic.cpp:3104) |

There is no level-number table and no filename convention — the byte in the level file is the whole law. (Our importer reads exactly these: `level_mc2.rs:229-231` — `gfx_type: data[0x04]`, `map_type: from_byte(data[0x06])`.)

**Level 000 specifically: MapType = 1 (Night), gfx_type = 0** — plain night: PALN-0 + BL32N0-0 + TABLESN + TMAPS1-0 + SKYN0-0. Black sky is correct.

### 1.3 Per-environment side effects at level init (`LevelInit_56C00`, LI:9-52)

| effect | Day | Night | Cave | cite |
|---|---|---|---|---|
| `isCaveLevel_D41B6` | 0 | 0 | 1 | LI:11/35 |
| `MapBasicHeight_D41B7` | (44 default) | (44) | `byte_0x2FED3` | LI:36, Terrain.cpp:14 |
| map transparency forced | 0 | 0 | 1 | LI:22/28/34 |
| sound bank (`LoadSounds_84300`) | 0 | 1 | 2 | LI:23/29/37; Sound.cpp:2299-2359 (seeks `bank*96` in SOUND.DAT) |
| cursor sprite | 1 | 9 | 10 | LI:24/30/38 |
| spells 4/19 `subspell[0].life_0x1A` | 2 | 19 | 19 | LI:12-21 (hint text 198/244 day vs 199/245) |

Spell indices are `spell_t` models (the spells buffer is indexed by `model_0x40_64` of class-15 entities, Level.cpp:1508-1523), i.e. 4 = Metamorph, 19 = Summon Army per our pinned grid order (`MC2_SPELL_NAMES`, mgc-app ui.rs:1332).

### 1.4 Per-environment data files

| file | Day | Night | Night-fog | Cave | loader |
|---|---|---|---|---|---|
| terrain atlas | BLOCK32.DAT | BL32N0-0 | **BL32F0-0** | BL32C0-0 | RD:20-140 (`sub_54660`) |
| sky bitmap | SKYD0-0 | SKYN0-0 | SKYN0-0 | **none** | RD:40-42/89-91/115-137 |
| TMAPS | TMAPS0-0 | TMAPS1-0 | TMAPS1-0 | TMAPS2-0 | RD:54/110/138 |
| TABLES | TABLESD | TABLESN | TABLESN | TABLESC | RD:145-170 (`sub_54800`) |
| palette | PALD-0 | PALN-0 | **PALF-0** | PALC-0 | EF:31895-31930 (`PaletteChanges_47760`), Level.cpp:878-906 |
| minimap CLR LUT | CLRD-0 | **CLRN-0** | CLRN-0 | **CLRC-0** | EF:31905-31925 |
| keyColor1/2 | 0xFE/0x00 | 0x00/0xFF | 0x00/0xFF | 0xFE/0xFF | RD:153-168 |

- **CORRECTION to a banked note:** the in-game palette path (`PaletteChanges_47760`, EF:31873+) loads the per-environment **CLRN-0/CLRC-0** minimap LUTs; only the menu-time `PaletteFadeIn_480A0` (EF:32161-32172) hardcodes PALD/CLRD.
- On the CD, TABLESD/N/C are stored **uncompressed** (83456 bytes each); SKY files are raw 65536-byte 8bpp bitmaps; **SKYC0-0.DAT exists (a brown rock texture) but no code path loads it**; there is **no TABLESF / SKYF** — fog reuses the night tables and night sky. The unsuffixed `DATA/TABLES.DAT` is read by nothing in the engine.
- "Cave = night + wizard-0 override" (roadmap shorthand) is precisely the players-colors block `sub_48120` (EF:32180-32262): Night and Cave share every entry except wizard 0 (night {0xA4,0xAA,0x7B} vs cave {0xE0,0x58,0x7B}); Day is a fully separate set.

## 2. The sky — retail mechanism

### 2.1 The pass (GRO:680-703, inside `DrawTerrainAndParticles_3C080`)
```c
if (!m_GameSettings.m_Graphics.m_wSky || isCaveLevel_D41B6)
    /* flat memset32 fill of the viewport with keyColor1_D4B7C */   // GRO:682-700
else
    DrawSky_40950(roll);                                            // GRO:702
```
Sky first, terrain triangles painted over it (painter renderer, no z-buffer). Cave never draws a sky — the fill IS its "sky" (and the cave ceiling geometry covers most of it, §5.2).

Flat-fill color check against the retail palettes: PALD[0xFE] = (176,188,252) pale blue; PALN[0x00] = (0,0,0); PALC[0xFE] = (0,0,0). `keyColor2_D4B7E` is set alongside (RD:153-168) but has no consumer in GRO (HD-renderer use only).

### 2.2 `DrawSky_40950` (GRO:258-373) — the cloud plane
- Samples the 256×256 bitmap `off_D41A8_sky` with a 16-bit wrap index `tex[v_hi<<8 | u_hi]` (GRO:344-355) — both axes wrap, so it tiles infinitely.
- Per-pixel deltas = cos/sin of **roll** scaled by 1/viewport-width (prepared table, GRO:290-313) → the cloud plane rotates with the carpet.
- Row seed `v23 = (yaw << 15) − f(pitch, camera)` (GRO:315-323): **yaw scrolls** the plane (a full 2048-unit turn = 4 texture wraps), **pitch** offsets it via `str_F2C20ar.dword0x22` (pitch·width>>8, set at GRO:672).

### 2.3 The bitmaps (data, measured)
| | dominant | features |
|---|---|---|
| SKYD0-0 | 74% idx 254 = (176,188,252) | white cumulus band (idx 224 white etc.), 29 distinct colors, everything lum>150 |
| SKYN0-0 | 94.5% idx 64 = (0,0,0) | faint blue moonlit cloud rims (max color ~(16,84,116)); **zero pixels lum>150 — no stars/moon** |
| SKYC0-0 | 70% idx 211 = (72,52,32) brown rock | never loaded by the engine |

### 2.4 Verdict on our row-0 model
- Day shade-LUT row 0 = 254×/256 the single index 254 — the SAME index as SKYD's 74%-dominant base. Night row 0 = 232×/256 index 64 — SKYN's 94.5% base. **Row-0 mode through the palette = the retail sky's base color, exactly.** Our current clear-color/fog-target derivation is data-correct.
- The full retail mechanism additionally shows the cloud texture; matching it faithfully = bake SKY{D,N}0-0.DAT (65536 raw bytes each; day also has the 1024² `skyd1024.data` enhanced variant in remc2's own asset pack, RD:47-50 — not retail). **BANKED** (§6).

## 3. Distance fog — the retail law

### 3.1 Constants (GRO:668-679; every frame, every environment)
```
FogStart  = 14745600 = 3840²   (= 15 tiles of 256)
FogEnd    = 23658496 = 4864²   (= 19 tiles)
Thickness =  8912896 = FogEnd − FogStart
cutoff    = 26214400 = 5120²   (= 20 tiles; tiles beyond are not drawn, GRO:1042-1046)
```
No environment (or weather/spell) modifies them — the only writers in the tree are these lines (plus the HD/NG rewrites). All per-environment fog character is in the TABLES data + palette.

### 3.2 Terrain (per-vertex, GRO:1038-1074 / 1560-1590 / 1647-1668)
```c
v33 = (mapShading[cell] << 8) + 128;                    // shade row, 8.8 fixed
if (type == 0) v37 = wave(cell, Turn);                  // animated sea sparkle, GRO:1053-1061
v39 = (v33 << 8) + 8*v37;
if (d² > FogStart)
    v39 = d² < FogEnd ? v39 * (FogEnd − d²) / Thickness  // linear in d²
                      : 0;                               // row 0 = full fog
pnt5_32 = v39;                                           // gouraud-interpolated
```
The span rasterizer indexes `tables[row<<8 | texel]` with `row` = the interpolated high byte (inner loop `*v341 = x_BYTE_F6EE0_tablesx[v31]`, GRO:8868-8905 and siblings). So terrain fog = **multiplicative pull of the shade row toward row 0**; shading and fog are one axis of the same LUT.

### 3.3 Sprites (GRO:3499-3511; executed by `DrawSprite_41BD3` modes 1/6/7)
```c
if (d² <= FogStart)      fogrow = 32;                         // identity
else if (d² < FogEnd)    fogrow = 32*(FogEnd − d²)/Thickness; // 31..0
else                     fogrow = 0;
```
Applied AFTER the blend for translucent modes 6/7 (see mc2-transparency-drawlist.md §4).

### 3.4 The LUT polarity (measured on the fixed bake: shade-lut.bin rows through palette.bin)

| row | mc2-day lum | mc2-night lum | mc2-night-fog lum | mc2-cave lum |
|---|---|---|---|---|
| 0 (fog) | 193.6 (mode idx254 pale blue) | 9.4 (mode idx64 black) | 21.1 (idx64 (12,12,12)) | 14.4 (black) |
| 32 (identity) | 130.4 (81.6% identity) | 82.4 (90.6% id) | 92.3 | 85.2 (78.9% id) |
| 63 | 11.4 (black) | 131.6 (brightest) | 139.0 | 129.1 |

**Day darkens with row; night/fog/cave brighten with row.** Both are monotonic through row 32 ≈ identity. Retail behaviors that hinge on this:
- slope shading writes `64 − shade` off-Day (Terrain.cpp:1281-1288 in `sub_44D00`; same flip in every terrain-deform event: EF:28642, 28688, 28980, 31058 (`AddBuildingToTerrain` env), 31185, 41613, 41766, 42080, 42121, and Terrain.cpp:2030-2033) — the index flip and the table flip cancel, so **the physical light direction is the same day and night**; the flip exists so row 0 can stay "fog color" while rows >32 stay "added light";
- scorch/crater floors = row **63 on Day, row 1 off-Day** (EF:23307-23315, 23412-23420, 28736-28744) — both are the near-black end;
- entity ground shadows: `fogrow' = 0x2000 ± fog/4` — **plus on Day, minus off-Day** (`notDay_D4320` set GRO:585, used GRO:3443-3446, shadow raster mode 8);
- dynamic lights ADD rows (clamped ≤63) — only bright-upward tables make that a light (§5.1).

### 3.5 Port tuning (for our exponential approximation)
Faithful targets: fog color = LUT row 0 (already done); onset 3840 world units, **full fog at 4864**, hard geometry cutoff 5120; ramp linear in d² (equivalently: factor = (FogEnd−d²)/Thickness). Identical in all four bundles.

## 4. The big map under night

### 4.1 The terrain painters
`DrawMinimap_63600` (GameUI.cpp:2256) dispatches to `sub_63670_draw_minimap_a` (:2265, draws to screen) or `sub_63C90_draw_minimap_b` (:2605, also captures into `x_DWORD_E9C3C`). Identical color law; per-pixel, non-cave (GameUI.cpp:2544-2554 opaque / 2570-2590 blend-capture):
```c
color  = tables[0x14000 + mapTerrainType[cell]];   // tile-type → map color (the +0x14000 table)
pixel  = tables[(mapShading[cell] << 8) | color];  // shade LUT (+0x0000), row = per-cell shading
// translucent map variants additionally: pixel = tables[0x4000 + (pixel<<8) | dst]  (:2581-2588)
```
**This IS our `palette[shade_lut[shade][tile_colors[type]]]` — retail law confirmed verbatim.** Deltas worth knowing: (a) cave maps mask `mapAngle & 8` cells (solid rock) to pixel 0 = black (:2436-2445, :2529-2534); non-cave never tests bit 8 on the map — open sea draws normally; (b) the translucent map modes add the ⅓/⅔ blend through +0x4000 (cave forces `transparency_0x2198 = 1` at LI:34, day/night default opaque).

### 4.2 Why sea noise is dark at night
`GenerateLevelMap_43830` (Terrain.cpp:19-56) ends with `sub_44D00` (Terrain.cpp:1242-1290): per cell, shade = `h(x−1,y−1) − h(x+1,y+1) + 32` clamped into [28..47]; **flat cells (diff = 0 — all open sea) get random rows 28..36** (`rand%9 + 28`, the sea "noise"). Off-Day it stores `64 − x`, and 64−[28..36] = [28..36] — the noise band maps onto itself. So night sea noise occupies the same rows as day; it reads dark purely because PALN's sea colors are dark and TABLESN's rows 28..36 hold them near identity. No night special-case anywhere in the map path. (In the 3D view the same sea cells additionally get the turn-animated sine wave on height+shade, GRO:1053-1061, and raster mode 26 via the open-sea flag, §5.2.)

### 4.3 Cross-refs
Entity dots, the per-MapType fill colors `v90/v91/v92` (GameUI.cpp:1043-1063: night v92 = CLR[4095], v91=0xE8, v90=0x84), castle rope, blink phases — all in the banked MC2 MINIMAP LAW (ROADMAP "PLAYTEST-2"/mob sessions; `DrawMinimapEntities_B_61A00` GameUI.cpp:951+). Dynamic lights mutate the same `mapShading` array the map reads — **night fires visibly brighten the map**, for free (§5.1).

## 5. Other night/cave-classified behavior (ledger)

### 5.1 Dynamic lights (Night/Cave ONLY)
- Registration `AddEvent2_847D0(event, radius, a3, a4)` (EF:47182-47195): gated on `m_wDynamicLighting` **and MapType ∈ {Night, Cave}** and <50 active; appends `{byte_1=a4, byte_2=radius, byte_3=a3(flicker span), pos, entity}` into `D41A0_0.str_0x3664C[50]`, sets entity byte[2] bit 3. Freed by `sub_84880` (EF:47250).
- Frame order in `DrawAndEventsInGame_47560` (EF:31724): `sub_848A0` (EF:47213) **subtracts** last frame's stored 5×5 contributions from `mapShading` → entity updates → `sub_84B80` (EF:47385) recomputes: flicker `rand % byte_3 − byte_3/2`, then per covered cell `sub_84EA0` (EF:47612-47670): skip if shade ≥63; 3D dist² to the cell's terrain point; if < 0x48000: `add = flicker + (31·(0x48000−d²)/0x48000 · radius) >> 7`, capped at 31 (or the source entity's remaining life when `byte_1&1`), clamped so shade ≤ 63; **added** to `mapShading` and remembered in `array_E[25]` for next frame's undo.
- Registered sources traced so far: (10,6) real standing fire, radius 80 (EF:35475; trace mc2-class10-m6-m9-m11-m28-m31.md §5.4). This is the roadmap's "(10,0) night light source" — the light belongs to the REAL fire (10,6), not the (10,0) stand-in.
- Because lights ride the shade row axis, they brighten terrain in the 3D view AND the big map, and they do nothing on Day maps by construction.

### 5.2 Water / second surface
- `GenerateLevelMap_43830` (Terrain.cpp:50-55): cave → `sub_43B40` (:1158-1180, second heightmap = ceiling `MapBasicHeight − h`, + fuzz `sub_43BB0` :1546-1575); **day AND night** → `sub_43D50` (:1183-1240) which only sets `mapAngle` bit 3 (`|= 8`) on height-0 cells fully surrounded by open sea. No second-heightmap water plane exists off-cave — night is not special vs day here; the roadmap note ("no occluding water surface; floor + wave dip only") stands for BOTH.
- Renderer: bit 3 → `triangleFeatures |= 0x80` (GRO:1075-76, 1590) → sea raster mode `x_BYTE_E126D = 26` instead of 5/7 (GRO:1376-1433) — the animated open-sea fill. The cave branch (`isCaveLevel_D41B6`, GRO:704+) is a separate two-surface (floor+ceiling) traversal.

### 5.3 Ambience/UX
- **Music:** `maptypeMusic_0x235` = 2 (Day) / 1 (Night) / 3 (Cave) (EF:31440-31451).
- **Sound bank** 0/1/2 per environment (§1.3) — SOUND.DAT holds per-environment sample banks (Sound.cpp:2299-2359).
- **Per-environment user brightness:** three separate settings `brightness_11/12/13` (day/night/cave), applied at palette install (PI:2495-2505; adjusted by the brightness keys PI:1252+). One player setting per environment, remembered independently.
- **HUD:** spell-power-bar backdrop color 16 off-Day vs 48 on Day (GameUI.cpp:400-403); cursor sprite per environment (§1.3).

### 5.4 Gameplay gates
- **Spell 25 (Cave-In) selectable only when `isCaveLevel_D41B6`** (PI:849).
- **Spells 4/19 (Metamorph/Summon Army) subspell-0 life 2 on Day vs 19 off-Day** + hint-text ids 198/244 vs 199/245 (LI:12-21).
- **(5,2) creature ctor returns 0 off-Day** (`sub_4B590`, EF:33758-59) — matches the roster's DAY-ONLY note (SURVEY-MC2-ROSTER.md:455).
- **GTD2.DAT scripted swap is Day-only** (EF:60449-60453): overwrites tables from +0x4000 up (`x_BYTE_FAEE0_tablesx_pre = tablesx+0x4000`, Basic.cpp:123) — blend matrix + map colors, not the shade LUT.
- Every terrain-deforming event recomputes shading with the day/night flip and scorches with 63/1 (§3.4 list).

## 6. RETAIL CHECKS BANKED

1. **Sky bitmaps:** bake `SKY{D,N}0-0.DAT` (raw 65536 B, no RNC) into mc2-day / mc2-night(+fog reuses N) bundles and implement the cloud-plane pass (wrap-tiled 256², roll-rotated, yaw-scrolled ×4/turn, pitch offset, drawn behind terrain — GRO:258-373). Until then the row-0-mode base color is data-exact. Cave: keep the flat black (retail flat-fills `keyColor1`; SKYC is dead data).
2. **Fog ramp:** replace/tune the exponential approximation to retail's linear-in-d² ramp — onset 3840, full fog 4864, geometry cutoff 5120, all environments identical; target color = LUT row 0 (already landed).
3. **Night terrain shading + lights:** when MC2 terrain shading ports, carry (a) the `64−x` off-Day storage flip (self-cancelling for light direction, load-bearing for scorch/lights/fog polarity), (b) the flat-cell 28..36 noise (`rand%9+28`, LCG 9377/9439 from the level seed chain), (c) the 50-slot dynamic-light add/undo cycle (Night/Cave only).
4. **Minimap CLR LUTs:** in-game retail loads CLRN-0/CLRC-0 per environment (EF:31905-31925) — our "CLRD only" note came from the menu path; bake/route per-environment when the map-dot quantization matters on night/cave maps.
5. **Per-environment brightness settings** (three sliders) — authenticity-matrix candidate when the options menu lands.
6. **Sea raster mode 26** (the open-sea animated fill) — joins the water-surface track; the open-sea flag (`mapAngle&8` → 0x80) is already load-bearing for it.
