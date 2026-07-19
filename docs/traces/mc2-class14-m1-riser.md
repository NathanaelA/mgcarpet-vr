# MC2 class-14 model-1 terrain riser (`sub_59F60`) — full verbatim trace + trigger web

All cites: `EF:<line>` = `reference/remc2/remc2/engine/EventsFunctions.cpp`, `EV:<line>` = `.../Events.cpp`. Companion docs: `mc2-class11-switches-class14.md` (§4 partial summary this replaces), `mc2-class10-m59-m60.md` (beacons + dispatch architecture).

---

## 0. HEADLINE FINDINGS (read first)

1. **The riser is a three-phase terrain machine** on `life_0x8`: `0` = INSTANT build (+48 in one tick, no sound), `1` = ANIMATED raise (+1/tick × 48 ticks, loop sound 47), `2` = ANIMATED lower (−1/tick until flank-level, then terrain-type restore), `3` = idle-built, `4` = idle-removed. Orientation `byte_0x46_70`: `0` = strip along +X, `1` = strip along +Y, anything else = counters tick but NO terrain writes.
2. **The runtime raise/lower triggers are class-10 models 0x40(64)/0x3F(63)** — actions `0x45`→`sub_343C0` (EF:25015, sets riser `life=1`) and `0x44`→`sub_34390` (EF:25003, sets `life=2`) — NOT models 68/69 (the strA0 numbering trap, same as m59/m60). They find the riser via **`sub_5B070`** (EF:42497) — same-map-cell entity-chain lookup. **724 (10,63)/(10,64) THINGs across the baked campaign** (e.g. level 002: riser at (105,139) dis 0 + (10,63) lower at the same cell dis 2; level 003/005/007: (10,63)+(10,64) pairs stacked on (14,2) cave pillars with distinct dis ids = stage-scripted open/close).
3. **LEVEL-000 CORRECTION — the missing water path is NOT this riser.** Level-000's baked `things.json` contains **zero (14,1) and zero (10,63)/(10,64)** records. The retail "narrow straight land path" at row y=212 is authored as a **linked pair of (10,29) waypoint THINGs** at (174,212)→(113,212) (slots 62/63, `DisId=-1`, `stageTag=1`, par1/par2 = prev/next slot links), consumed **at level-generation time** by `GenerateEvents_49290`'s jx-loop (EV:178-198, subtype list includes 0x1D) → `PrepareEvents_49540` case 0xA/0x1D (EV:333) → **`sub_49090`** (EV:5261, walks the chain head-first, zeroes stageTags) → **`sub_48690`** (EV:5493, per leg spawns two **(10,30) `AddPointToPath_4F9A0`** segment entities carrying `dword_0x10_16`=length, `yaw/pitch`=unit step) → one-shot action **`ApplyPointToPath_343F0`** (EF:25027, actionIndex 0x20 = strA0[0x20]=0x2153F0):
   ```c
   while (v2) {                       // v2 = dword_0x10_16 (length)
       mapAngle_13B4E0[v1.word] = mapAngle_13B4E0[v1.word] & 0xF0 | 1;  // class nibble := 1 (clears deep-water bit 3!)
       v2--;
       sub_462A0(v1, v1);             // the MC2 retile (already ported: mgc_sim::mc2::terrain_paint)
       v1.x += event->yaw_0x1C_28;    // unit step (here -1, 0)
       v1.y += event->pitch_0x1E_30;
   }
   DisableEntityDrawing04_57F10(event);
   ```
   For level-000: chain head slot 62 (174,212) → slot 63 (113,212); `sub_48690` computes Xdist=−61, Ydist=0 → first (10,30) at (174,212) len 0 (diagonal share), second at (174,212) len 61, yaw=−1, pitch=0 → stamps cells **(114..174, 212)**. Baked terrain there is height 0 / type 0 / angle nibble 8 (deep water); the stamp turns each cell to class-nibble 1 and the retile rewrites `mapTerrainType` to a land tile — a flat, sea-level, 1-cell-wide causeway. That is exactly where the (5,13) villager row (x118-145, y212) and the two beacons sit. (10,30)'s ctor `AddPointToPath_4F9A0` EF:36256 sets actionIndex 0x20; model 0x1E is inside `ApplyEvents_498A0`'s run-to-completion band 0x1B..0x20 (EV:493-504), so the path is fully stamped during generation settle. **Port fix for the drowning settlers = this waypoint-path pass**, while `sub_59F60` (this doc) fixes the walls/pillar-risers of levels 002/003/005/007/008/….

---

## 1. Riser state machine (sub_59F60, EF:41255-42492)

Dispatch: class-14 action 6, `strE0[6]`=0x23AF60 (EF:1932, EV:2864). Runs once per tick while the entity is live.

| `life_0x8` | phase | terminal |
|---|---|---|
| < 0 | dead — no branch taken (falls through `v1 < 1` with `!v1` false) → return | — |
| 0 | **instant build**; `dword_0x10_16++` first (length = par2+1) | `subSpellIndex=48; life=3` |
| 1 | **animated raise**: gate `subSpellIndex_0x2A_42 < 0x30`; sound 47 every tick; first tick (sub==0) stamps type/angle; every tick +1 height on strip interior; on the tick sub reaches 0x30 recompute shading; NEXT tick takes the gate's else | `life=3; EndLoop_6EAB0(idx,-1,47)` (EF:42133-42135) |
| 2 | **animated lower**: if sub==0 → `life=4; EndLoop 47` (EF:42140-42143); else sound 47, −1 toward flank average, `sub--`; when sub hits 0, terrain-type restore + endcap + dirty | `life=4; EndLoop 47` |
| 3 / 4 | idle built / idle removed (`v1 != 2` → return, EF:42138) | — |

Externals: `sub_343C0` sets life=1, `sub_34390` sets life=2 (§6). No RNG anywhere; the only masking is the deterministic `&7`/`&3` shade folds.

Fields consumed: `position_0x4C_76` (base cell), `byte_0x46_70` (orientation), `dword_0x10_16` (length L; **++'d once by the life-0 pass only**), `subSpellIndex_0x2A_42` (0..48 progress), `life_0x8`. `word_0x2C_44`/`word_0x96_150` are **model-2 (cave pillar) fields only** — the riser never reads them.

**Cell derivation** (all phases): `cx = (u16)(pos.x + 128) >> 8`, `cy = (u16)(pos.y + 128) >> 8` (round-to-nearest; entity is spawned at tile center `(tile<<8)+128`, EF:33014-33015). Base cell `B=(bx,by)`:
- life 0 (EF:41479-41486): `v208=(cx,cy)`; then orientation 1 → `by=cy-1` (byte dec), orientation 0 → `bx=cx-1` (byte dec).
- life 1/2 (EF:41803-41812, 42146-42155): orientation 1 → `(cx, cy-1)`; orientation 0 → `v207=(cx,cy)` then **`v207.word--`** (16-bit dec — equals `(cx-1,cy)` except x==0 borrows into y; life-0 wraps within the byte). Port faithfully.

All map arrays are `[65536]`, index = `(y<<8) | x` (uaxis_2d: low byte x, high byte y; `word±1`⇒x±1 w/ carry, `±256`⇒y±1, `±512`⇒y±2, `±768`⇒y±3).

---

## 2. Creation + parameter wiring

### 2.1 ctor `sub_51660` (EF:37378-37394) — verbatim
```c
type_entity_0x6E8E* sub_51660(axis_3d* position) {   // 232660; strE1[1], EV:5150
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->actionIndex_0x45_69 = 6;
        event->class_0x3F_63 = 0xE;
        event->struct_byte_0xc_12_15.byte[0] &= 0xF6u;   // clear bits 0,3
        event->model_0x40_64 = 1;
        event->struct_byte_0xc_12_15.byte[0] |= 1;       // active
        event->maxLife_0x4 = 0;
        event->life_0x8 = 0;                             // ⇒ instant build on first tick unless retargeted first
        event->subSpellIndex_0x2A_42 = 0;
        AddEventToMap_57D70(event, position);            // inserts into mapEntityIndex_15B4E0 cell chain (⇒ sub_5B070 finds it)
    }
    return event;
}
```
The ctor sets **neither** orientation nor length — the THING spawn path does:

### 2.2 THING spawn `sub_4A310` case 0xE (EF:33219-33244) — verbatim
```c
case 0xE:
    v10 = indexx->model_0x40_64;
    if (v10 < 1u)  { sub_58DA0(entity, v3x); return; }
    if (v10 <= 1u) {                      // MODEL 1 — the riser
    LABEL_49:                             // (shared with class-10 models 0x3D/0x3E, EF:33112)
        v2x->byte_0x46_70   = entity->par1_14;   // par1 → ORIENTATION (0=X, 1=Y)
        v2x->dword_0x10_16  = entity->par2_16;   // par2 → LENGTH
        sub_58DA0(entity, v3x);
    } else {
        if (v10 == 2) {                   // model 2 — cave pillar
            v2x->word_0x2C_44  = entity->par1_14;
            v2x->word_0x96_150 = entity->par3_18;
            sub_58DA0(entity, v3x); return;
        }
        sub_58DA0(entity, v3x);
    }
    return;
```
THING fields (BasicTerrain.h:7-18 / things.json): `par1_14`=`parent`, `par2_16`=`child`, `par3_18`=`par3`, `stageTag_12`=`swi_id`, `word_10`=`swi_sz`, `DisId`=`dis_id`.

### 2.3 Creation paths (all of them)
1. **Disposition scan** `sub_4A1E0(dis, consume)` (EF:32950-32996): every THING with `type!=0 && DisId==dis` → `sub_4A310`; `dis=0` fired at load (twice: EF:39425, 39474), `dis=N` fired by class-11 switch actions (`sub_4A1E0(ev->id_0x1A_26, 1)`; id = THING `stageTag_12`, EF:33199). Campaign (14,1)s all use `dis_id=0` (levels 002/008) — **built at load** by the life-0 instant path. A stage-var pre-pass `sub_122C0(dis)` (EF:4961-4968) marks StageVars of kind 7 watching that dis (`stage_0x3647A_1 |= 0x18`) — creature-wave reactions, not the riser.
2. **Generation pass** `GenerateEvents_49290` (EV:226-234): only class-14 **subtype 2** (cave pillar) with `DisId==-1` spawns at generation, via `PrepareEvents_49540` case `0x02/0x0E` (EV:304-321) which dispatches the class-14 sub-creator table `str_x_DWORD_D4C52ar_0x2F22` (EF:2081: idx1=0x232660=sub_51660) and writes `word_0x2C_44=par1, word_0x96_150=par3` — the **model-2** wiring. A hypothetical `DisId==-1` (14,1) would get orientation/length **zeroed** (this case writes the model-2 fields, not `byte_0x46_70`/`dword_0x10_16`); no campaign level authors one.
3. No hardcoded code call sites of `sub_51660` exist (grep: only EV:5150 creator dispatch).

**Lifecycle nuance (port-relevant):** because the ctor leaves `life=0`, a dis-spawned riser instant-builds on its first tick. `dword_0x10_16++` happens **only** in the life-0 branch, so a riser that instant-built has effective L=par2+1 forever after; a riser whose life was set to 1 before its first tick (possible only if a same-tick trigger ran first) would keep L=par2. Campaign data always lets life-0 run first (riser dis 0; raise triggers with dis 0 act on the *next* tick order — see §6 ordering note).

---

## 3. LIFE 0 — instant build (EF:41470-41796), verbatim

Entry: `v1 = life; if (v1 < 1) { if (!v1) { ... } return; }`. Let `L = ++dword_0x10_16` (EF:41481). `v3 = byte_0x46_70`.

### 3.1 Orientation 1 (strip along +Y; base B=(bx,by)=(cx,cy−1)); EF:41488-41643

**(a) RAISE +48** (EF:41492-41513) — 2 columns × L rows:
```c
v33 = B; v201 = 0;
do {                                          // columns c = bx, bx-1  (v33.word--)
    v34 = v33;
    for (i = 0; i < L; i++) {                 // rows by .. by+L-1
        if (mapTerrainType_10B4E0[v34] != 8
            || (v35 = (v33.x, v34.y + 1),     // neighbor +Y (along strip)
                abs(mapHeightmap_11B4E0[v34] - mapHeightmap_11B4E0[v35]) > 30))
            mapHeightmap_11B4E0[v34] += 48;
        v34.y++;
    }
    v33.word--; v201++;
} while (v201 < 2);
```
Skip rule: a cell already type 8 with a smooth (≤30) step to its +Y neighbor is **not** re-raised (idempotence for overlapping ridges); type-8 cells at a >30 discontinuity DO raise.

**(b) TYPE/ANGLE STAMP** (EF:41514-41531) — 3 columns × (L+1) rows:
```c
v36 = B;
for (v37 = 0; v37 < 3; v37++, v36.word--) {   // cols bx, bx-1, bx-2
    v38 = -1; v39 = v36 - 256;                // row by-1
    while (v38 < L) {                         // rows by-1 .. by+L-1
        v40 = v39; v38++; v39.y++;
        mapTerrainType_10B4E0[v40] = 8;       // ridge/wall tile type
        mapAngle_13B4E0[v40] = 1;             // class nibble 1, ALL other bits cleared
    }
}
```

**(c) CAVE ceiling fixup** (EF:41535-41563, `isCaveLevel_D41B6`) — 4 columns × (L+2) rows:
```c
v46 = B + 1;                                   // col bx+1
for (v47 = 0; v47 < 4; v47++, v46.x--) {       // cols bx+1, bx, bx-1, bx-2
    v202 = -1; v48 = v46 - 256;                // row by-1
    while (L + 1 > v202) {                     // rows by-1 .. by+L   (L+2 cells)
        v49 = mapHeightmap_11B4E0[v48];
        if ((u8)x_BYTE_14B4E0_second_heightmap[v48] > v49)
            mapAngle_13B4E0[v48] &= 0xF7u;     // ceiling above floor: clear solid bit 3
        else {
            x_BYTE_14B4E0_second_heightmap[v48] = v49 - 1;  // pin ceiling below floor
            mapAngle_13B4E0[v48] |= 8u;        // solid column
        }
        v48.y++; v202++;
    }
}
```
**(d) NON-CAVE** (EF:41564-41583): same 4×(L+2) footprint, only `mapAngle &= 0xF7` (clear bit 3 = deep-water/solid flag).

**(e) SHADING** (EF:41584-41622) — L+1 cells `(bx, by-1+k)`, k=0..L:
```c
v50 = -1; v51 = B - 256;                       // (bx, by-1)
while (v50 < L) {
    v51.x++; v51.y++;      v52 = v51;          // (bx+1, by+k)
    v51.x -= 2; v51.y -= 2; v53 = v51;         // (bx-1, by+k-2)
    v51.x++;                                   // (bx, by+k-2)
    v54 = mapHeightmap_11B4E0[v53] - mapHeightmap_11B4E0[v52] + 32;
    v51.y++;                                   // (bx, by+k-1)
    if (v54 >= 28) { if (v54 > 40) v54 = (v54 & 7) + 40; }
    else            v54 = (v54 & 3) + 28;      // fold into [28..47]
    v55 = (MapType != MapType_t::Day) ? (32 - v54 + 32) : v54;   // non-Day: 64 - s
    v56 = v51; v50++; v51.y++;
    mapShading_12B4E0[v56] = v55;              // shade (bx, by-1+k)
}
```
**(f) DIRTY-MARK** (EF:41623-41639) — 9 columns × (L+6) rows:
```c
v57 = -3; v58 = B - 3;                          // col bx-3
while (v57 < 6) {                               // cols bx-3 .. bx+5
    v59 = -3; v60 = v58 - 768;                  // row by-3
    while (v59 < L + 3) {                       // rows by-3 .. by+L+2
        v61 = v60; v59++; v60.y++;
        mapAngle_13B4E0[v61] |= 0x80u;          // renderer dirty/lock bit
    }
    v57++; v58.word++;
}
```
Then (for **any** v3≠0, even junk orientations that skipped all writes): `subSpellIndex = 48; life = 3;` (EF:41641-41642).

### 3.2 Orientation 0 (strip along +X; base B=(cx−1,cy)); EF:41644-41794 — exact mirror
- **(a')** EF:41646-41666: cols `bx..bx+L-1` × rows `by, by-1` (`v5.y--`); neighbor test `abs(h[v5] - h[v5+1]) > 30` (**+X**, along strip); `+= 48` same rule.
- **(b')** EF:41667-41685: cols `bx-1..bx+L-1` (L+1) × 3 rows `by, by-1, by-2`: `type=8; angle=1`.
- **(c')** cave EF:41686-41713: cols `bx-1..bx+L` (L+2) × 4 rows `by+1, by, by-1, by-2` (start `v12+256`, `y--`): identical ceiling fixup. **(d')** non-cave EF:41714-41733: same footprint `angle &= 0xF7`.
- **(e')** EF:41734-41774: L+1 cells `(bx-1+k, by)`; `v24 = h[(bx-2+k, by-1)] - h[(bx+k, by+1)] + 32`; same folds/Day; write `mapShading[(bx-1+k, by)]`.
- **(f')** EF:41775-41791: cols `bx-3..bx+L+2` (L+6) × 9 rows `by+3` **down to** `by-5` (start `v29+768`, `y--`): `|= 0x80`.
- Then `subSpellIndex = 48; life = 3;` (EF:41792-41793).

Note the deliberate asymmetries the port must copy: dirty rows/cols run `-3..+5` on one side (orient-1 cols ascending from bx−3; orient-0 rows descending from by+3), and the strip's second row/col is on the **−** side of the base cell.

---

## 4. LIFE 1 — animated raise (EF:41798-42136), verbatim

Gate (EF:41800): `if (subSpellIndex_0x2A_42 < 0x30)` else → `life = 3;` + `EndLoop_6EAB0(idx, -1, 47)` (EF:42133-42135, LABEL_292) and return.

Every raise tick: `PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 47)` (EF:41802). Base B per §1. L = `dword_0x10_16` (no ++ here).

### 4.1 First tick only (`subSpellIndex == 0`, EF:41813-41933) — stamp pass
Orientation 1 (v62==1):
- **STAMP** (EF:41820-41837): 3 cols `bx, bx-1, bx-2` (`v78.word--`) × rows `by+2 .. by+L-4` (`v80=2; v81=v78+512; while (v80 < L-3)`): `type=8; angle=1`. (Interior only — endpoints excluded, count L−5.)
- **NON-CAVE CLEAR** (EF:41838-41857, `!isCaveLevel`): 4 cols `bx+1..bx-2` (`v84=B+1`, `word--`) × rows `by+2 .. by+L-3` (`v85=2; while (v85 < L-2)`): `angle &= 0xF7`. (Cave levels do their ceiling fixups per-tick in §4.2 instead.)
- **DIRTY** (EF:41858-41874): identical 9×(L+6) block as §3.1(f) (`v89 = B-3`, `v91 = v89-768`, rows ascending).

Orientation 0 (v62==0):
- **STAMP** (EF:41879-41896): cols `bx+2 .. bx+L-4` (`v63=2; v64=B+2; while (v63 < L-3)`) × 3 rows `by, by-1, by-2`: `type=8; angle=1`.
- **NON-CAVE CLEAR** (EF:41897-41916): cols `bx+2 .. bx+L-3` × 4 rows `by+1..by-2` (`v71=v69+256`, `y--`): `angle &= 0xF7`.
- **DIRTY** (EF:41917-41931): cols `bx-3..bx+L+2` × 9 rows `by+3..by-5` (`v76=j+768`, `y--`): `|= 0x80`.

### 4.2 Every raise tick — `subSpellIndex++` (EF:41934) then +1 height on the strip interior
Orientation 1 (EF:41938-41991):
```c
v104 = (bx, by+3); v103 = 3;
while (v103 < L - 3) {                          // rows by+3 .. by+L-4
    mapHeightmap_11B4E0[(bx,  row)] += 1;       // v105/v106
    mapHeightmap_11B4E0[(bx-1,row)] += 1;       // v107/v108
    v103++; row++;
}
if (isCaveLevel_D41B6)                          // EF:41957-41991
    for (row = by+3; row < by + (L-3); row++)   // same rows
        for (col of {bx, bx-1}) {               // v110 then v112 = v110-1
            v = mapHeightmap_11B4E0[(col,row)];
            if ((u8)x_BYTE_14B4E0_second_heightmap[(col,row)] > v)
                 mapAngle_13B4E0[(col,row)] &= 0xF7u;
            else { x_BYTE_14B4E0_second_heightmap[(col,row)] = v - 1;
                   mapAngle_13B4E0[(col,row)] |= 8u; }
        }
```
Orientation 0 (EF:41996-42043): mirror — cols `bx+3 .. bx+L-4` (`v95=B+3`), rows `by` and `by-1` each `+= 1`; cave fixup for the same two rows per col (`kx`, `kx.y--`, restore).

48 ticks × +1 = **+48 total**, matching the instant path. Interior-only: rows/cols `±3` from both ends never animate (they were stamped type-8 but only rise via… nothing — the ends stay low; retail ramps the ends implicitly since STAMP covered `+2..L-4/L-3` and heights only move on `+3..L-4`).

### 4.3 Last raise tick (`subSpellIndex >= 0x30` after ++, EF:42045-42130) — shading
- Orientation 1 (EF:42050-42089): **only 3 cells** `(bx, by), (bx+1, by), (bx+2, by)` — diagonal shade `v127 = h[(x-1,y-1)] - h[(x+1,y+1)] + 32`, same 28/40/`&7`/`&3` folds, same non-Day `64 - s` inversion (EF:42080-42083), write `mapShading`.
- Orientation 0 (EF:42094-42128): **L+1 cells** `(bx-1+k, by)`, k=0..L — `v119 = h[(bx-2+k, by-1)] - h[(bx+k, by+1)] + 32`, folds, Day inversion (EF:42121-42124), write `mapShading[(bx-1+k, by)]`.
(The 3-vs-L+1 asymmetry is verbatim retail decompile — keep it.)

Then return; the **next** tick hits the `>= 0x30` else → `life=3` + `EndLoop(…,47)`.

---

## 5. LIFE 2 — animated lower + restore (EF:42138-42491), verbatim

`if (v1 != 2) return;` (EF:42138 — life 3/4/n do nothing).
`if (!subSpellIndex) { life = 4; goto LABEL_292; }` → `EndLoop_6EAB0(idx,-1,47)` (EF:42140-42143).
Else `PrepareEventSound(idx, -1, 47)` (EF:42145); base B per §1.

### 5.1 Every lower tick — sink toward flank average
Orientation 1 (EF:42159-42181), rows `by+3 .. by+L-4`:
```c
v141 = h[(bx, row)];
v142 = (h[(bx-2, row)] + h[(bx+1, row)]) >> 1;   // average of the two FLANK columns
if (v142 < v141) h[(bx, row)]  = v141 - 1;
v144 = h[(bx-1, row)];
if (v142 < v144) h[(bx-1, row)] = v144 - 1;
```
Orientation 0 (EF:42185-42203), cols `bx+3 .. bx+L-4`:
```c
v137 = (h[(col, by-2)] + h[(col, by+1)]) >> 1;   // flank rows
if (v137 < h[(col, by)])   h[(col, by)]--;
if (v137 < h[(col, by-1)]) h[(col, by-1)]--;
```
Then `subSpellIndex--` (EF:42205-42206). Cells clamp at flank level (no underswing); after ≤48 ticks the ridge is flush.

### 5.2 Final tick (`subSpellIndex` decremented to 0, EF:42207-42490) — terrain-type restore
Orientation 1 (`v146 == 1`; **`v146 >= 2` returns immediately with NO cleanup**, EF:42212-42213):
- **RESTORE** (EF:42214-42252), rows `by+3 .. by+L-5` (`v173=3; while (v173 < L-4)`):
  ```c
  v175 = (bx+2, row);                            // source = east flank cell
  if (mapTerrainType_10B4E0[(bx+2,row)] == 8)    // flank still ridge? jump the source
      v175.y += L >> 1;                          //   +halfL ALONG THE STRIP (+Y)
  for (cell of {(bx+1,row), (bx,row), (bx-1,row), (bx-2,row)}) {
      mapTerrainType_10B4E0[cell] = mapTerrainType_10B4E0[v175];
      mapAngle_13B4E0[cell]       = mapAngle_13B4E0[v175];
      mapShading_12B4E0[cell]     = 32;
      mapAngle_13B4E0[cell]      |= 0x80u;
  }
  ```
  (The decompile interleaves the last cell's writes through saved registers v180-v183/v153-v156 — net effect exactly as above, EF:42227-42251.)
- **CAVE** (EF:42253-42315, `isCaveLevel && L-4 > 3`): same rows, ceiling fixup (§3.1(c) rule) at the 4 cells `(bx+1..bx-2, row)`; then `goto LABEL_286` dirty.
- **NON-CAVE ENDCAP** (EF:42317-42325): clear bit 3 at the 4 cells of row `by+3`: `(bx+1, by+3), (bx, by+3), (bx-1, by+3) [v197=B+767], (bx-2, by+3)`.
- **DIRTY** (EF:42326-42344, LABEL_286): 9 outer iterations (`v194 -3..5`) × rows `by-3 .. by+L+2` (`v199 = v195-768`, `y++`): `|= 0x80`. ⚠ **remc2 transcription suspect**: the outer step here is `v195x._axis_2d.y++` with the original `//++v195;` left commented out (EF:42342-42343) — every other dirty block in this function steps `word++` (col+1). As written it repaints column bx−3 nine times shifting in Y. Port the symmetric `word++` (cols bx-3..bx+5) and flag for asm verification (OPEN-1).

Orientation 0 (`v146 == 0`, EF:42347-42490):
- **RESTORE** (EF:42347-42389), cols `bx+3 .. bx+L-5`:
  ```c
  v149 = (col, by+2);                            // source = south flank cell
  if (mapTerrainType_10B4E0[(col,by+2)] == 8)
      v149.word += L >> 1;                       //   +halfL ALONG THE STRIP (+X — word +=, not y +=)
  for (cell of {(col,by+1), (col,by), (col,by-1), (col,by-2)}) {
      type[cell] = type[v149]; angle[cell] = angle[v149];
      shading[cell] = 32; angle[cell] |= 0x80;
  }
  ```
- **CAVE** (EF:42390-42454, `L-4 > 3`): same cols, ceiling fixup at the 4 rows `by+1..by-2`; then LABEL_261.
- **NON-CAVE ENDCAP** (EF:42456-42472): clear bit 3 at col `bx+3`, rows `by+1, by, by-1, by-2` (`v165 = B+259`; then `v165.y = (u16)(B.word+3) >> 8` = by, then `y--` twice).
- **DIRTY** (EF:42473-42490, LABEL_261): cols `bx-3 .. bx+L+2` (`v164.word++`) × 9 rows `by+3 .. by-5` (`v171 = v164+768`, `y--`): `|= 0x80`.

Life stays 2; the **next** tick sees `subSpellIndex == 0` → `life = 4` + `EndLoop 47`.

Result: the strip sinks flush and its tiles are **re-typed from the flank terrain** — over water flanks it becomes water again (walkers drown again); over land it becomes that land.

---

## 6. Runtime raise/lower triggers — class-10 models 0x3F(63)/0x40(64)

**Numbering trap** (same as m59/m60): strA0 rows 0x44/0x45 (EF:1670-1671 → 0x215390/0x2153C0) are the actions of the models whose **ctors set actionIndex 0x44/0x45**:
- `sub_4F900` (EF:36222-36236, strA1[0x3F]=0x230900, EF:1767): `maxLife=1; actionIndex=0x44; class=0xA; model=0x3F;` byte0 `&0xF6|1`; `AddEventToMap`; `CopyMaxLifeToLife`.
- `sub_4F950` (EF:36238-36254, strA1[0x40]=0x230950, EF:1768): identical but `actionIndex=0x45; model=0x40`.

Actions (EV:2495-2502):
```c
void sub_34390(type_entity_0x6E8E* a1x) {   // 215390, action 0x44 — LOWER
    type_entity_0x6E8E* v1x = sub_5B070(a1x);
    if (v1x) v1x->life_0x8 = 2;
    DisableEntityDrawing04_57F10(a1x);      // one-shot
}
void sub_343C0(type_entity_0x6E8E* a1x) {   // 2153c0, action 0x45 — RAISE
    type_entity_0x6E8E* v1x = sub_5B070(a1x);
    if (v1x) v1x->life_0x8 = 1;
    DisableEntityDrawing04_57F10(a1x);
}
```

**`sub_5B070`** (EF:42497-42526) — find the riser/pillar in MY map cell:
```c
v1 = mapEntityIndex_15B4E0[ ((u8)((u16)(pos.y - 128) >> 8) << 8)
                          + ((u16)(pos.x - 128) >> 8) ];      // NOTE: −128 bias here (vs +128 in the riser)
while (Entities_EA3E4[v1] > Entities_EA3E4[0]) {
    e = Entities_EA3E4[v1];
    if (e->class_0x3F_63 == 14 && (e->model_0x40_64 == 1 || e->model_0x40_64 == 2))
        return e;
    v1 = e->oldMapEntity_0x16_22;                             // cell chain
}
return 0;
```
So a trigger THING must be authored **in the same map cell** as the riser THING. Spawn wiring: models 0x3F/0x40 take the plain `sub_58DA0` path in `sub_4A310` (EF:33107-33114 band, no par consumption). Campaign patterns (baked scan, 724 records): riser (14,1)/(14,2) + a (10,63) with `dis_id = <switch id>` at the same cell (lower on stage progress), often plus a (10,64) with another dis (re-raise), and dis-0 (10,64)s that animate structures up at load (levels 003/005/007).

**Ordering:** the trigger acts on its first tick (via the class-10 action dispatch), i.e. after the whole `sub_4A1E0` spawn sweep of that disposition — the co-located riser (created in the same or an earlier sweep) is guaranteed present in the cell chain. If the riser is idle-built (life 3, sub 48), a RAISE trigger is a no-op (`sub >= 0x30` → life 3 + EndLoop). If lowered (life 4, sub 0), RAISE re-runs the full animation including the sub==0 stamp. LOWER on a built ridge runs §5. LOWER on a never-built riser (life 0 not yet ticked): life-0 wins only if it ticks first — data does not author this.

---

## 7. Water interaction — what makes the raise walkable

- **MC2 water tiles are `mapTerrainType == 0`** — the renderer's wave displacement keys `if (!mapTerrainType_10B4E0[v36])` (GameRenderOriginal.cpp:1054-1062); walkers drown on `sub_104D0_terrain_tile_is_water(...) == 1` (mask of type 0; EF:8855; ported as `cap_bit == 1`); baked level-000 water = height 0, type 0, angle nibble 8 (deep-water bit 3 set).
- `sub_104D0` (Terrain.cpp:2058) maps `mapTerrainType` through `sub_10590_terrain_tile_type` (Terrain.cpp:2067: type 0→1, 8→0x100, …) — the same mask table as walker terrain permission (`v_20`).
- **The riser rewrites everything itself** — height (+48), `mapTerrainType = 8`, `mapAngle = 1` (class nibble 1, clears deep-water bit 3), shading, cave ceiling, dirty bits. Raising height alone would NOT stop drowning (type stays 0) and would not retile; **type must be rewritten too, and the routine does it** — port all five arrays.
- Walkability of the ridge: type 8 ⇒ mask 0x100; a creature can stand on it iff its behavior row `v_20` has bit 0x100 (ported `cap_bit` law). The **player carpet** is blocked/deflected by type-8 tiles in `moveTest_5D0A0` (EF:59478/59500/59509 `== 256` checks) — i.e. the raised wall is a barrier to the player, which is the point of the level-002/008 wall rectangles. ⚠ The SURVEY-MC2 line "water is MC2's absolute barrier" describes this `==256` gate; per the tile-type table that gate keys **type 8 (ridge/wall)**, not type 0 (water) — reconcile when porting moveTest (OPEN-3).
- The flat sea-level **path** (level 000) is the different, cheaper mechanism (§0.3): angle nibble := 1 (clears bit 3) + `sub_462A0` retile; height untouched.
- Related standalone stampers (same family, used by (10,0x1C) chains via `sub_48400`, EV:5365): `sub_34000` (EF:24863), `sub_34110` (EF:24897), `sub_34210` (EF:24929) — heightmap `+= 48` with the same `type != 8 || sub_33F70(cell)` skip and `sub_46180(cell, 8)` type-set helper. Not needed for the riser but cite-adjacent.

---

## 8. Index math cheat sheet

- Entity pos → cell: `cx = (u16)(x + 128) >> 8`, `cy = (u16)(y + 128) >> 8` (riser, EF:41479, 41803, 42146). `sub_5B070` uses `(pos − 128) >> 8` instead (EF:42504-42505).
- Array index: `idx = (cy << 8) | cx` into `mapTerrainType_10B4E0`, `mapHeightmap_11B4E0`, `mapShading_12B4E0`, `mapAngle_13B4E0`, `x_BYTE_14B4E0_second_heightmap`, `mapEntityIndex_15B4E0` (all 65536).
- `uaxis_2d`: `word++` = x+1 (carry into y at 255→0); `_axis_2d.x±`/`.y±` = byte-wrapping component steps; `word ± 256/512/768` = y ± 1/2/3.
- `getTerrainAlt_10C40(axis)` (Terrain.cpp:2146) = `sub_B5C60_getTerrainAlt2(x, y)`: two-triangle interpolation of `mapHeightmap` (32× scale; diagonal picked by `(xhigh + yhigh) & 1` — twin of the ceiling variant `sub_B5D68`, Terrain.cpp:2164-…). The riser itself never calls it; the spawn path does (z snap, EF:33016).

---

## 9. Constants table

| item | value | cite |
|---|---|---|
| action index / dispatch | class 14, action 6, `strE0[6]`=0x23AF60 | EF:1932, EV:2864 |
| orientation field | `byte_0x46_70`: 0=+X strip, 1=+Y strip, ≥2 no-writes | EF:41487, 41815, 42156 |
| length field | `dword_0x10_16` = THING par2 (`child`); `++` once in life-0 | EF:33230, 41481 |
| instant/total raise | **+48** (instant) / **+1 × 48 ticks** | EF:41506/41657, 41947-41955/42001-42010 |
| progress counter | `subSpellIndex_0x2A_42` 0→0x30 (raise), 0x30→0 (lower) | EF:41800, 41934, 42045, 42205 |
| already-ridge skip | `type == 8 && |Δh along strip| <= 30` → no raise | EF:41499-41504, 41654-41655 |
| ridge tile type / angle | `type = 8`, `angle = 1` | EF:41526-41527, 41679-41680, 41832-41833, 41891-41892 |
| deep-water/solid bit | angle bit 3: cleared (`&0xF7`) non-cave, set + ceiling=h−1 when cave column closes | EF:41548-41553, 41578, 41699-41704 |
| dirty/lock bit | angle `|= 0x80` over strip ±3 (9 wide) × L+6 long | EF:41635, 41787, 41870, 41928, 42339, 42486 |
| shade fold | `>=28`: `>40 → (s&7)+40`; else `(s&3)+28` (range 28..47) | EF:41604-41612, 41757-41765, 42071-42079, 42112-42120 |
| shade day law | Day: `s`; non-Day: `64 − s` (`32 − s + 32`) | EF:41613-41616, 41766-41769, 42080-42083, 42121-42124 |
| restore shading | flat **32** | EF:42227/42232/42237/42246, 42359-42384 |
| restore source | flank +2 across-strip; if type 8, `+ (L>>1)` along-strip | EF:42220-42222, 42352-42353 |
| loop sound | **47 (0x2F)** each raise/lower tick; `EndLoop_6EAB0(idx,−1,47)` on 1→3 and 2→4 | EF:41802, 42145; 42135, 42140-42143 |
| life transitions | 0→3 (instant), 1→3 (sub≥0x30), 2→4 (sub==0) | EF:41642/41793, 42133, 42142 |
| lower rule | sink 1/tick while `h > (flankA+flankB)>>1` | EF:42167-42175, 42191-42199 |
| RAISE trigger | (10, **0x40**=64), ctor `sub_4F950`, action 0x45 `sub_343C0` → `life=1` | EF:36238, 25015; EV:2499 |
| LOWER trigger | (10, **0x3F**=63), ctor `sub_4F900`, action 0x44 `sub_34390` → `life=2` | EF:36222, 25003; EV:2495 |
| trigger lookup | `sub_5B070`: same cell (`(pos−128)>>8`), chain `oldMapEntity_0x16_22`, class 14 model 1|2 | EF:42497-42526 |
| par wiring | par1→orientation, par2→length (`LABEL_49`) | EF:33228-33231 |
| level-000 path stamp | angle `= (a & 0xF0) | 1` + `sub_462A0` retile per cell; (10,29)→`sub_49090`→`sub_48690`→(10,30) action 0x20 | EF:25027-25043; EV:5261-5362, 5493-5555 |
| water tile | type **0** (waves, drowning mask 1); ridge type **8** (mask 0x100, player moveTest barrier) | GameRenderOriginal.cpp:1054; Terrain.cpp:2067; EF:59478 |
| RNG | **none** (deterministic `&7`/`&3` folds only) | — |

---

## 10. OPEN items

1. **Life-2 orient-1 dirty loop step** (EF:42342-42343): remc2 has `v195x._axis_2d.y++` with the original `//++v195` commented out — asymmetric vs every other dirty block (`word++`). Port the symmetric `word++` (cols bx−3..bx+5); verify against the retail binary at 0x23C0xx if fidelity disputes arise.
2. **Level-000 retail timing**: the (10,29) path pass runs at level **generation** (DisId=−1 jx-loop), so the causeway should exist from level start; the player's "appears between stage triggers" memory may conflate it with the beacons/smoke. Retail-check when dosbox is available. Also confirm the port's bake/runtime split for generation-time terrain edits (path must land in the terrain the port loads — either bake it or run the pass at load).
3. **moveTest `== 256` semantics**: SURVEY-MC2 calls the player commit-gate blocker "water", but the mask 256 = tile type 8 = the ridge type this riser writes (open water is type 0). Reconcile in the moveTest port; both behaviors (can't fly through raised walls; something about water) hang on this.
4. **`sub_46180(cell, 8)` / `sub_33F70`** (used by the sibling stampers `sub_34000/34110/34210`, not by the riser): bodies not read — needed only if porting the (10,0x1C) ridge-chain builders.
5. **Class-0 (0,13) conditional-spawn records** at (115-117, 212) in level-000 (three, all-zero pars, right at the path's west end): their consumer is the still-untraced conditional-spawn machinery (Phase 4.1 OPEN) — presumably three more villagers, unrelated to terrain.
6. `EndLoop_6EAB0` internals (stop-looping-sound helper) not read; name-inferred, matches `PrepareEventSound` pairing everywhere else.
