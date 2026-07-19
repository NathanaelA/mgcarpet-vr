# MC2 Translucency — Blend LUT, Draw Modes, and the (Non-)Existence of a Transparent Draw List

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, GRO = `GameRenderOriginal.cpp` — the faithful software renderer; GameRenderHD/NG are remc2's rewrites and are cited only where noted). MC1 citations to `/home/rain/projects/mgcarpet/reference/remc1/sub_main.cpp` (MC1) and `/home/rain/projects/mgcarpet/reference/remc1hw/sub_main.cpp` (MC1HW). Data analysis performed on the actual retail `DATA/TABLES{D,N,C}.DAT` + `PAL{D,N,C}-0.DAT` (extracted from gamedata via mgc-import's gamedata/rnc modules, 2026-07-10). Trace date 2026-07-10.

**Headline findings (read first):**

1. **CORRECTION to a banked claim: `byte[2] |= 2` is NOT a "transparent-effect draw list".** It marks membership in the 1000-slot temp-entity recycle pool `D41A0_0.dword_0x11EA` (alloc EV:588-605, unlink scan `sub_57F20` EV:5215-5233, save/load Level.cpp:1289). It has nothing to do with rendering. There is **no separate transparent draw pass at all** — translucent sprites draw inline, in painter order, right after their own tile's terrain triangles (§2).
2. **The real mechanism is a per-sprite raster MODE (`str_F2C20ar.dword0x01_rotIdx`, 0..8) selected in `DrawSprites_3E360` and executed by `DrawSprite_41BD3`'s inner-loop switch (GRO:4429).** Modes 2/3 (and their fogged twins 6/7) blend each pixel through a **256×256 lookup table at TABLES +0x4000**: `out = tables[0x4000 + row*256 + col]` (§4).
3. **The table is DATA, per environment** — `DATA/TABLESD.DAT` (day) / `TABLESN.DAT` (night) / `TABLESC.DAT` (cave), RNC-compressed, 83456 (0x14600) bytes, loaded whole at level load (`sub_54800_read_and_decompress_tables`, ReadAndDecompress.cpp:145-170; called EF:39379/39458). Empirically the blend matrix is `T[a][b] = nearest_palette(⅓·rgb(a) + ⅔·rgb(b))` — an **asymmetric one-third/two-thirds mix** (best-fit w = 0.31, w=⅓ within noise; 50/50 is 2× worse; §5.2). Row = the ⅓ contributor. That asymmetry is the whole "two strengths" system: mode 2 = 33%-opaque sprite, mode 3 = 67%-opaque sprite.
4. **"Tint" = palette-nearest quantization of that ⅓/⅔ mix, per-environment.** There is no per-effect color ramp and no additive table; day/night/cave each bake their own matrix against their own palette, so the same smoke reads warm at day and murky at night. Fogged modes 6/7 apply the 64-row fog/shade LUT (TABLES +0x0000) **after** the blend (§4, §5.1).
5. **Smoke columns are translucent via static sprite-descriptor data, not entity flags:** `particlesParameters_D951C[i].byte_10 == 2` → mode 2/6 through decode table `x_BYTE_D4750` (GRO:3808-3814, GameRenderOriginal.h:48). The (10,13)/(10,14) clouds use descriptor rows 67/9, both `byte_10=2` (§3.2, §6.1).
6. **MC2's per-entity override bits (flags dword):** bit 23 (`byte[2]&0x80`) = mode 2 (33% ghost), bit 24 (`byte[3]&1`) = mode 3 (67%), bit 25/26 (`byte[3]&2`/`&4`) = player-color recolor modes 4/5 (GRO:3779-3806). This override gate is **MC2-new** — MC1's selection is descriptor-only (§8).
7. **MC1's engine has the identical rasterizer modes and the identical TABLES layout** (fog LUT +0x0000, blend matrix +0x4000; remc1 `strPal.fog_B7934_B7924` / `strPal.byte_BB934_BB924`, sub_main.cpp:35540-35800). MC1 world translucency is engine-supported; whether any MC1 world sprite descriptor selects it is unconfirmed (data arrays truncated in the remc1 decompile). The player's "MC1 = alpha-mask only" observation stands for content, not capability (§8).
8. **RETAIL CHECK BANKED — our bake carves MC2's shade LUT from the wrong offset.** `bundle.rs` (`MC2_SHADE_OFFSET = 0x4000`) and FORMAT.md:335 claim MC2's shade LUT lives at +0x4000. TRACED + measured: MC2's fog/shade LUT is at **+0x0000, exactly like MC1** (retail indexes `tables[fogrow<<8 | color]` with fogrow ≤ 0x20 — GRO:3176/3501-3511 sprites, GRO:8869+ terrain spans; row 32 of region 0 is 79-91% identity in all three MC2 variants, row 0 = the sky/fog color). What we currently ship as MC2 `shade-lut.bin` is rows 0..63 **of the blend matrix** (row 32 there is only 15% identity — it's "⅓ pull toward palette color 32"). See §9.

---

## 1. The flags dword `struct_byte_0xc_12_15` — render-relevant bits

Bit numbering: `byte[0]`=bits 0-7, `byte[1]`=8-15, `byte[2]`=16-23, `byte[3]`=24-31.

| bit | byte form | meaning | citation |
|---|---|---|---|
| 0, 5 | `byte[0] & 0x21` | **not drawn** (DrawSprites entity gate) | GRO:3157/1936 |
| 3 | `byte[0] & 8` | (cleared by many effect ctors via `dword &= 0xFFFDFFF7`) list-target eligibility | EF:35343 etc. |
| 17 | `byte[2] & 2` | **temp-pool member** (recycle list `dword_0x11EA`) — NOT rendering | EV:588-605, EV:5215, Level.cpp:1289 |
| **23** | `byte[2] & 0x80` | **translucent mode 2** — 33% sprite / 67% background | GRO:3798-3805 |
| **24** | `byte[3] & 1` | **translucent mode 3** — 67% sprite / 33% background | GRO:3798-3801 |
| **25** | `byte[3] & 2` | recolor mode 4 — ⅓ player-color + ⅔ texel (subtle team tint) | GRO:3784-3789 |
| **26** | `byte[3] & 4` | recolor mode 5 — ⅓ texel + ⅔ player-color (strong recolor) | GRO:3791-3796 |
| 29 | `byte[3] & 0x20` | draw-x offset from mount parent's yaw (`str_D404C` widths) | GRO:3464-3497 |
| 30 | `byte[3] & 0x40` | fold 16 view sectors through `x_BYTE_D4750[44..59]` | GRO:3570-3572 |
| 31 | `byte[3] & 0x80` | draw 160 units lower (sinking corpses/devoured) | GRO:3458-3461 |

The override gate is decompiled as `v91 = byte[2]; if (v91 & 0x380)` (GRO:3779-3781, same in HD:3852/NG:3460) — a lift artifact (`int8` sign-extension makes `byte[2]&0x80` satisfy `& 0x380`). Semantics recovered from the branch bodies: **if bit 23, 24, 25 (or 26) is set, take the flag-override path; else the descriptor default path** (§3.2). Priority inside the override: mode 4 > mode 5 > (bit 23 clear ? mode 3 if bit 24 : mode 2) (GRO:3782-3806). Modes 4/5 fetch the tint color from `playersColors_E88E0x[...][2]` of the **parent** entity's wizard (`parentId_0x28_40`, GRO:3786-3795).

## 2. Draw ordering — no lists, no z-buffer: painter per tile

- Frame: `DrawWorld_411A0` (GRO:9) → `DrawTerrainAndParticles_3C080` (GRO:373) which draws sky, then traverses terrain tile strips, and **for each tile after emitting its triangles**: `if (Str_E9C38_smalltit[jx].haveBillboard_36) DrawSprites_3E360(jx)` (GRO:899-900, 1025-1026, 1435-1436, 1529-1530, 1774-1775, 1840+ — one call per traversal quadrant/stage).
- `haveBillboard_36` = head of the per-map-cell entity chain, filled from `mapEntityIndex_15B4E0[cell]` during traversal (GRO:1091, 1597, 1664); `DrawSprites_3E360` walks it via `oldMapEntity_0x16_22` (GRO:3822-3826).
- Terrain traversal is far-to-near (painter overdraw, no depth buffer), so when a translucent sprite reads the frame buffer, everything behind it — terrain, farther tiles' sprites — is already there. **Translucency correctness falls out of the traversal order**; there is no sorting pass and no transparency list. (TRACED for the interleave-with-own-tile; far-to-near overall order is the painter invariant the blend reads require — inference, consistent with the no-z-buffer renderer.)
- Consequence: two translucent sprites on the SAME tile blend in chain order (spawn-list order), not depth order. Retail doesn't care; neither should a faithful port.

## 3. Mode selection in `DrawSprites_3E360` (GRO:3050)

### 3.1 Fog row (`str_F2C20ar.dword0x00`)
Per entity, from squared camera-plane distance `v51` (GRO:3499-3511):
```c
if (v51 <= FogStart)        dword0x00 = 0x2000;                                  // row 32 = identity
else if (v51 < FogEnd)      dword0x00 = 32 * (FogEnd - v51) / FogThickness << 8; // rows 31..0
else                        dword0x00 = 0;                                       // row 0 = full fog color
```
`dword0x00` is `fogrow << 8`, index base into the **+0x0000** fog/shade LUT. `0x2000` (row 32, no fog) doubles as the "full bright" discriminator below.

### 3.2 Descriptor path (the default; how smoke is translucent)
Every drawable entity carries `word_0x5A_90` = row into the static 347-entry `particlesParameters_D951C` (Type_WORD_D951C.h:8-22, .cpp:3; original exe data). Fields: `word_0` sprite id, `rotSpeed_8` size, **`byte_10` material class**, `byte_12` draw type (0/1/17/18..36 switch, GRO:3521+). Mode:
```c
if (dword0x00 == 0x2000) rotIdx = x_BYTE_D4750[    byte_10];   // full-bright   GRO:3811
else                     rotIdx = x_BYTE_D4750[6 + byte_10];   // fog-shaded    GRO:3813
```
`x_BYTE_D4750` (GameRenderOriginal.h:48-53) decodes:

| `byte_10` | full-bright mode | fogged mode | material |
|---|---|---|---|
| 0 | 0 (opaque raw) | 1 (opaque, fog-shaded) | normal |
| 1 | 0 | 0 | **fullbright** (never fogged — glows/fire) |
| 2 | **2** (33% blend) | **6** (blend then fog) | **translucent, heavy** |
| 3 | **3** (67% blend) | **7** (reverse blend then fog) | **translucent, light** |
| 4 | 4 | 4 | player-recolor subtle |
| 5 | 5 | 5 | player-recolor strong |

(Bytes 12..27 / 28..43 / 44..59 of the same array are the 16-sector view-fold maps for draw types 18/19/`byte[3]&0x40`.)

**All translucent descriptor rows in MC2** (scan of Type_WORD_D951C.cpp):
- rows **9-16**: sprite 0x3F, growth ramp 0x32..0x190, `byte_10=2` — the **(10,14)** volcano-smoke puff band (m60's particles start at row 9; trace mc2-class10-m59-m60.md §3).
- rows **67-74**: sprite 0x39, same ramp, `byte_10=2` — the **(10,13)** quest-beacon smoke band (m59's particles start at row 67; ibid).
- row **78**: sprite 0, `byte_10=3` — used by `NewAdd0A07_4E6A0` (10,7) via `SetHalfSpeedEntity_49DA0(event, 78)` (EF:35500).
- row **209**: sprite 0x90, `byte_10=3` — `SummonManaPosession_4D3B0` (EF:34781) and `sub_4DDD0` (EF:35148).
- row **216**: sprite 0x97, `byte_10=3` — `sub_4D860` (EF:34958/35025), `sub_66750` (EF:58342).
- row **224**: sprite 0xA8, `byte_10=3` — no direct `SetEntityIndexAndRot(…,224)` call found (likely reached by ramp growth from a lower row).
- rows **293-304**: sprite 0x147, ramp 0x46..0x168, `byte_10=2` — a second large translucent growth band (no direct base-row caller found; reached via ramp/growth, same pattern as smoke).

### 3.3 Shadow blobs = mode 8
The `shadows_F2CC7` first half of DrawSprites projects the sprite at terrain altitude and forces `rotIdx = 8` with the fog row biased off identity: `notDay ? 0x2000 - fog/4 : 0x2000 + fog/4` (GRO:3441-3448) — mode 8 ignores the texel colors and **re-shades the ground through the fog LUT under the sprite's mask** (day rows >32 darken toward black; night rows <32 darken toward the dark sky color — §5.1). Shadow strength scales with fog distance.

## 4. `DrawSprite_41BD3` inner loops — the exact per-pixel math

Switch on `rotIdx` at GRO:4429 (upright) and GRO:5004 (the rolled/mirrored variant, same modes). `src` = sprite texel (0 = skip, the color-key — GRO:4463 etc.), `dst` = frame-buffer byte. `T` = `x_BYTE_F6EE0_tablesx` (83456-byte blob, Basic.h:197; `x_BYTE_FAEE0_tablesx_pre` aliases `T+16384`, Basic.cpp:123).

| mode | inner loop (verbatim index math) | semantics | citation |
|---|---|---|---|
| 0 | `if (src) *dst = src` | opaque, color-keyed | GRO:4430-4488 |
| 1 | `if (src) *dst = T[(dword0x00 & 0xFF00) \| src]` | opaque + fog row | GRO:4490-4524 |
| **2** | `if (src) *dst = T[16384 + (src<<8) \| *dst]` | **33% sprite + 67% background** | GRO:4525-4562 (idx GRO:4546/4559) |
| **3** | `if (src) *dst = T[16384 + (*dst<<8) \| src]` | **67% sprite + 33% background** | GRO:4564-4601 (idx GRO:4585/4598) |
| 4 | `if (src) *dst = T[16384 + (pc<<8) \| src]` | ⅓ player-color + ⅔ texel (opaque write) | GRO:4603-4635 (pc = `dword0x07`, GRO:3786) |
| 5 | `if (src) *dst = T[16384 + (src<<8) \| pc]` | ⅓ texel + ⅔ player-color (opaque write) | GRO:4637-4668 |
| 6 | `t = T[16384 + (src<<8)\|*dst]; *dst = T[(dword0x00&0xFF00)\|t]` | mode 2, then fog **after** blend | GRO:4670-4691 (GRO:4684-4685) |
| 7 | `t = T[16384 + (*dst<<8)\|src]; *dst = T[(dword0x00&0xFF00)\|t]` | mode 3, then fog after blend | GRO:4693-4714 (GRO:4707-4708) |
| 8 | `if (src) *dst = T[(dword0x00&0xFF00) \| *dst]` | src as mask only; re-shade ground (shadows) | GRO:4716-4744 (GRO:4737/4750) |

Note the row/column orientation: **the row (high byte) is the ⅓ contributor** (§5.2). Mode 2 puts the sprite in the row (sprite faint); mode 3 puts the background in the row (sprite dominant).

## 5. The TABLES blob — files, layout, measured semantics

### 5.1 Loading and layout
`sub_54800_read_and_decompress_tables(MapType)` (ReadAndDecompress.cpp:145-170): Day → `DATA/TABLESD.DAT` (also sets colorkeys `keyColor1=0xFE, keyColor2=0x00`), Night → `TABLESN.DAT` (0x00/0xFF), Cave → `TABLESC.DAT` (0xFE/0xFF). One RNC file, decompressed to 0x14600 bytes over `x_BYTE_F6EE0_tablesx`:

| offset | size | contents | consumers |
|---|---|---|---|
| +0x0000 | 0x4000 | **fog/shade LUT**, 64 rows × 256: `T[row][c]`; **row 32 ≈ identity** (81.6% day / 90.6% night / 78.9% cave / 95.3% MC1), rows 31→0 converge on the **sky/fog color** (day ≈ RGB(176,188,252) pale blue; night ≈ RGB(12,9,6) black; MC1 row 0 literally constant), rows 33→63 converge the other way (day/MC1/cave → black = darkness; night → a moonlit gray ≈ RGB(140,135,115)) | sprite modes 1/6/7/8; terrain span rasterizer `T[light<<8 \| texel]` (GRO:8869-8975, light byte gouraud-interpolated); wall case GRO:5083 |
| +0x4000 | 0x10000 | **blend matrix**, 256×256: `T[a][b] = nearest(⅓a + ⅔b)` | sprite modes 2-7; motion blur GRO:168-231; full-screen smoothing GRO:225-231; crosshair GameUI.cpp:1537-1583/2185-2210; map/HUD blits GameUI.cpp:1105, 2437-2585; MC1 UI blitters (remc1 sub_main.cpp:27444/27564) |
| +0x14000 | 0x100 | tile-type → flat map color (our `tile-colors.bin`) | GameUI.cpp:2437 etc. |
| +0x14100 | 0x200 | two more map-draw helper rows (0xFF row, 0x00 row) | (map compositing masks) |
| +0x14300 | 0x300 | map lens/brightness curves | GameUI.cpp:2386, 2702 (`T[0x14300 + frac]` as a multiplier) |

### 5.2 Measured blend-matrix semantics (retail data, all four variants)
Least-squares fit of `rgb(T[a][b]) = w·rgb(a) + (1−w)·rgb(b)` over all 65536 cells (6-bit palettes scaled ×4):

| variant | best w | rmse @ best | rmse @ w=⅓ | rmse @ w=½ | diag identity | symmetric? |
|---|---|---|---|---|---|---|
| MC2 day | 0.31 | 12.8 | 13.0 | 21.9 | 85.2% | 2.1% |
| MC2 night | 0.31 | 10.6 | 10.8 | 20.4 | 95.3% | 1.8% |
| MC2 cave | 0.32 | 18.6 | — | — | 94.9% | 1.7% |
| MC1 temperate | 0.32 | 8.9 | 8.9 | 18.6 | 95.3% | 2.2% |

So: **⅓ row + ⅔ column, nearest-palette-quantized, per environment.** Per-row w is flat (0.30-0.36) — no per-color special rows, no additive region, no darkening region. `T[a][a] = a` (diagonal ≈ identity). The visible **tint** of retail translucency = (a) the ⅓/⅔ weighting itself and (b) quantization into that environment's palette (day palettes pull smoke warm-gray, night pulls it blue-black). The residual (rmse ≈ 10-19 of 255) is quantization noise, largest in the cave palette.

### 5.3 Curiosities banked
- **Motion blur** (option `xxxx_0x2191`): previous frame in the row (⅓ old + ⅔ new) per pixel — GRO:160-205.
- **Full-screen smoothing** (option `str_0x2192.xxxx_0x2192`): three chained lookups mixing right and below neighbors — GRO:214-233.
- **Crosshair** is drawn translucent through the same matrix, colour in the row: `T[0x4000 + 256*CentreCrossColour + *dst]` (GameUI.cpp:1537-1583).
- **`DATA/GTD2.DAT` hot-swap:** the banishment sequence (`sub_5E7C0_multiplayer_test_banished`, EF:60254) on Day maps loads GTD2.DAT **over `T+0x4000`** (EF:60450-60453) — replacing the blend matrix so the blur/blend paths repaint the world in the "banished" tint until `sub_54800` reloads the real tables. A whole full-screen effect done by swapping the LUT.
- `BlendAndBlit_40F80` (EF:30829) uses separate additive tables `x_BYTE_F0220..F0920` (interlaced half-res upscale) — different mechanism, out of scope.

## 6. Who is translucent in MC2 (complete enumeration)

### 6.1 Via descriptor `byte_10` (static data — §3.2 table)
Smoke puffs (10,13)/(10,14) (rows 67/9 + growth ramps), (10,7) steam (row 78, light), possession/summon glows (rows 209/216, light), the 0x147 ramp band (rows 293-304), row 224. These are translucent **always**, no flag needed. (This corrects the earlier reading that the smoke's `byte[2] |= 2` was the transparency routing — the smoke ctors' `dword &= 0xFFFDFFF7; byte[2] |= 2` lines (EF:35654 etc.) are pool bookkeeping; their translucency rides entirely on descriptor rows 67/9.)

### 6.2 Via entity flag bits (dynamic)
| setter | entity / meaning | bits | citation |
|---|---|---|---|
| `sub_4BD00` ctor `dword \|= 0x48800001` | **(5,10) doomsday pyramid** — spawns translucent-33% + view-folded (bit23 + bit30 + bit27 + bit0) — closes the doomsday trace's OPEN "renderer meaning of 0x48800001" | 23, 27, 30, 0 | EF:33980 |
| `sub_21490` (doomsday devour, v29 stages) | victims: stage 1 → mode 3 ghost (+clear bit23); stage 2 → `byte[3] \|= 0x80` sink-160 (+clear bit16); stage 3 → hidden | 24, 31 | EF:13049-13066 |
| `sub_29400` (m27 kraken-tree body sequencer) | whole chain `word[1] &= 0xFE7F` then phase-keyed: mode 3 or mode 2 — the tree's **emerge/teleport ghost fade** (closes "draw-flag groups" note in mc2-multipart-chains.md §0xD8) | 23, 24 | EF:19455-19462, 19531-19543 |
| `TransformArcherToMana_35940` (class-10 action 0x29, EF:1665; EV:2578 "end of mana sphere making") | **3-step death fade**: life 12 → mode 3 (67%), life ≤ 6 → mode 2 (33%) (+`byte[3]&0xFE`), life 0 → hidden | 24 then 23 | EF:26290-26308 |
| `sub_3A8B0` (class-10 action 0x55, EF:1687) | **owner-only ghost**: for the owning player's view → unhide + mode 2; other players → `byte[0] \|= 1` hidden unless their `SpellsEnabled[12]` entity is active (a see-invisible counter) | 23, 0 | EF:29845-29862 |
| `sub_4F440` ← `AddFireSpheres_4F2A0` ctor (10,76)+25×(10,77) | 5×5 fire-sphere wall: segments with `byte_0x44_68 != 0` (grid col 1-4) → mode 2 ghost; col 0 → opaque `byte[0] \|= 8` | 23 | EF:35989-36032 (ctor EF:35936) |
| `sub_293D0` / `sub_293B0` (m26 wake/calm) | **(5,26) mana-leech wraith** — outside attack state 210: clear target, full speed, `byte[2] = v2 \| 0x80` → **ghost-33% while hunting**; in attack state 210: `byte[2] &= 0x7F`, min speed → **solid while draining**. Called at the tail of every m26 movement handler, as post-init (EF:34585), and as dispatched actions (EV:2086/2091) | 23 | set EF:19425-19439 (:19436), clear EF:19411-19423 (:19419) |

**Reconciliation — flags bit 23 is dual-purpose by retail design (m26 "full-speed" vs render ghost).** Our port reads `sub_293D0`'s `byte[2] |= 0x80` as an AI wake/full-speed marker (mgc-sim mc2/roster.rs:2810-2825, `flags |= 1 << 23`). Both readings are the SAME storage: entity struct offset **0xE mask 0x80** = bit 23 of the `struct_byte_0xc_12_15` dword (offset 0xC) — exactly the byte/bit `DrawSprites_3E360`'s override gate reads for mode 2 (GRO:3779-3805; setter EF:19436, reader GRO:3798/3805). There are not two fields; retail deliberately gives the bit both effects: the wraith turns 33%-translucent while it hunts at full speed and solidifies while it drains (the state marker IS the ghost look). Verdict: the port's bit-23 folding is correct and `m26_wake`/`m26_calm` must keep writing it; only the roster.rs comment ("byte[2] bit 7 = flags bit 23", an *AI* signal) is incomplete — it should note the render side effect, and the future billboard blend-mode export (§10 step 1) must map bit 23 → `Ghost33` for creatures too, which gives m26 its retail look for free. For clarity: mode-3 (67%) override = offset **0xF mask 0x01** (bit 24); m26 never touches it.

**Modes 4/5 (player recolor):** no `byte[3] |= 2/4` setter exists anywhere in the decompile (swept EF/EV/Level/Terrain/GameUI). Engine-supported, content-unused in MC2 single-player as decompiled — possibly a multiplayer/team path or flags loaded verbatim from level THING data (inference; the flags dword round-trips through saves, Level.cpp:363/EF:38816).

## 7. What does NOT use the blend matrix
Terrain triangles and water spans never touch +0x4000 — the span rasterizer (`DrawTriangleInProjectionSpace_B6253`, GRO:5319+) uses only `T[light<<8 | texel]` in the +0x0000 region (GRO:8869-8975). Water transparency in MC2 is faked by palette/texture, not by blending. The blend matrix is exclusively: world sprites (modes 2-7), shadows-by-fog (mode 8, region 0), full-screen blur/smoothing, and 2D UI (crosshair, map, HUD).

## 8. MC1 counterpart — same machine, different content
- MC1's sprite rasterizer has the **identical mode set**: case 1 = fog LUT (`strPal.fog_B7934_B7924[fogrow|src]`), case 2 = `strPal.byte_BB934_BB924[src<<8|dst]`, case 3 = reverse, cases 6/7 = blend-then-fog, case 8 = mask-re-shade (remc1 sub_main.cpp:35545-35800+, in `DrawSkyTerrainParticles_2A700`). Same TABLES blob layout (fog at +0x0000 — `B7934`, blend at +0x4000 — `BB934`), same 83456-byte file (`DATA/TABLES.DAT`/`DTABLES.DAT`, verified by extraction), same measured ⅓/⅔ weights (§5.2).
- MC1 mode selection is **descriptor-only**: `dword_B5CAC_B5C9C = (fog==0x2000 ? byte_906DC : byte_906E2)[desc->var_10]` (remc1hw sub_main.cpp:33783-33788) — the D4750[0..11] split, but **no per-entity flag override gate** (no `&0x380` analog anywhere in remc1/remc1hw). The override bits (§1 bits 23-26) are the MC2 addition.
- MC1's descriptor data arrays are truncated in the decompile (`byte_906DC[] = {0}`, remc1 sub_main.cpp:2456-2457), so whether any MC1 world sprite ships with material 2/3 is **unverifiable from source**; player gameplay recording shows none. MC1 definitely uses the blend matrix for its HUD/UI blits (sub_main.cpp:27444/27564 — already ported in `mgc-app/src/ui.rs`) and mode-8 shadows.

## 9. Retail checks banked (for our tree)
1. **MC2 `shade-lut.bin` offset is wrong** (crates/mgc-import/src/bundle.rs:186 `MC2_SHADE_OFFSET = 0x4000`, comment at :60-63, FORMAT.md:335): retail MC2 fog/shading indexes +0x0000 (§5.1). Current MC2 terrain/sprite shading runs through blend-matrix rows 0..63 (≈ "⅓ pull toward palette colors 0..63"), i.e. row 32 is NOT identity — fog and darkness curves are off, most visibly at night/cave and at fog distance. Fix: `shade_offset: 0` for all variants, delete the "pixel-remap at +0x0000" comment, FORMAT.md line 335, bump BAKE_EPOCH. (MC1 goldens untouched; MC2 state-hash goldens unaffected — render-only.)
2. **Correct the m59/m60 trace + memory claim** that `byte[2] |= 2` routes to a transparent draw list (§1; the mc2-class10-m59-m60.md:224 wording "overflow/recycle list" was already right — the transparency attribution came later and is wrong).
3. Doomsday trace OPEN item "renderer meaning of `0x48800001`" — closed (§6.2).
4. mc2-multipart-chains.md §0xD8 "phases set draw-flag groups" — now concrete: emerge ghost fade via bits 23/24 (§6.2).
5. `blend-lut.bin` is already baked per variant (+0x4000..+0x14000 slice, bundle.rs:691-702) and already consumed by the MC1 UI compositor — the world-sprite consumer below is new.

## 10. Implementation sketch (crates/mgc-render + mgc-sim)

Current state: billboards are opaque instanced quads; fragment resolves `palette[shade_lut[shade][index]]` + RGB fog (billboard.wgsl fs_main); painter-ish depth from anchor-tile distance (`anchor_depth`). Sim exports `LivePose` → `Billboard` → `BillboardInstance` (`flags = [mirror, shade_row]`).

**Faithful-in-spirit default (recommended): RGB alpha approximation.**
The retail result IS `⅓src + ⅔dst` (or reverse) up to palette quantization (§5.2); in our true-color pipeline the ideal mix is the same math without the quantization noise. So:
1. **mgc-sim**: add `blend: BlendMode` (`Opaque | Ghost33 | Ghost67`) to `LivePose` (mc1/world.rs:531 and the mc2 pose path). Sources, in priority order per §1/§3: entity flag bit 23 → `Ghost33`, bit 24 → `Ghost67`, else descriptor `byte_10` (2 → `Ghost33`, 3 → `Ghost67`). Port the 347-row `particlesParameters_D951C` `byte_10` column (or just the nonzero rows from §3.2) with the mc2 tables; MC1 poses stay `Opaque`.
2. **mgc-render**: carry `blend` through `Billboard` into `BillboardInstance.flags` (a third flag word replaces `_pad`). Split the billboard pass: opaque instances first (current pipeline, depth-write on), then translucent instances **sorted by `anchor_depth` descending**, alpha-blend enabled, depth-test on / depth-write off, fragment emitting `alpha = 1/3` (Ghost33) or `2/3` (Ghost67). Fog stays as-is — retail applies fog after blend (modes 6/7), and `mix(base, fog, f)` then alpha-over is the same composite to first order.
3. **Recolor modes 4/5** (when content ever needs them): not translucency — implement as an RGB `mix(texel, player_color, ⅓ or ⅔)` with opaque write.
4. **Death fades**: no renderer work — the sim flag flips (bit 24 then bit 23) drive Ghost67→Ghost33 exactly like retail (§6.2).
5. **Shadows (mode 8)** stay out of scope here; if/when ported, they're a darken-decal keyed by the fog LUT, not the blend matrix.

**Faithful-LUT alternate (authenticity-matrix opt-in, "palette-exact translucency"):** the bundle already ships `blend-lut.bin` (65536 B) per variant — upload as a 256×256 `r8uint` texture alongside the colormap. Exact retail output requires the *destination* palette index, which our RGB target no longer has; the honest exact path is an 8-bit indexed offscreen for the world pass (a bigger project, aligned with the "software-faithful renderer" alternate column). Do NOT fake it by nearest-matching the RGB dst back to a palette index per fragment — that's neither faithful nor cheap. Bank the LUT texture plumbing until/if the indexed pipeline lands.

**Bake changes**: none required for translucency (blend-lut.bin exists in every variant, FORMAT.md:344 entry already documents the slice — reword it to note it is a *world+UI* blend matrix, `nearest(⅓row + ⅔col)`, not UI-only). Required independently: the §9.1 shade-offset fix (BAKE_EPOCH bump).

## OPEN items
- **OPEN-1**: modes 4/5 setters unfound (§6.2) — check level THING flag dwords or multiplayer paths if team-colored effects ever matter.
- **OPEN-2**: MC1 descriptor table contents (does any MC1 world sprite ship material 2/3?) — needs the MC1 exe data segment, not present in remc1; player recordings suggest no.
- **OPEN-3**: night/cave fog-LUT rows 33..63 converge toward a light gray (§5.1) — which night-time effects index rows >32 (lightning? the mode-8 `notDay` branch uses rows <32) is untraced; harmless for the port (we index computed rows, whatever they are, once §9.1 lands).
- **OPEN-4**: `GTD2.DAT` (banishment LUT swap, §5.3) — contents unanalyzed; bake + trace when the banishment sequence is ported.
