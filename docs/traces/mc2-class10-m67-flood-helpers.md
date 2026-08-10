# CLASS-10 Model 67 — flood/quake HELPER closure (port-ready supplement)

Port-ready verbatim supplement to `docs/traces/mc2-class10-tail-helper-closure.md` §1, which covers the
(10,67)=0x43 flood/quake phase machine (`sub_39040` / `sub_396A0` / `sub_396D0`) but leaves its HELPERS
*summarized*. This doc transcribes every helper VERBATIM with exact integer semantics (signedness,
truncation, clamp order) so the flood can be ported to Rust without re-reading the decompile.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
EF = `engine/EventsFunctions.cpp`, EV = `engine/Events.cpp`, Terrain = `engine/Terrain.cpp`,
Player = `engine/Player.cpp`, Maths = `utilities/Maths.cpp`. Trace date 2026-07-10.
Read the parent tail-helper-closure §1 first; this doc expands the black boxes it named.

---

## Headline findings (read first)

1. **`GetTerrainHeightFromSquare_48DF0` (EF:32605) is NOT a MAX and NOT a box scan.** It samples the
   **4 CORNERS** of the box `(x,y)…(x+width,y+height)` and returns their **AVERAGE** `(sum) >> 2`
   (arithmetic, truncating). The parent doc's phase-1 comment "sample 18×18 **max** height" is WRONG —
   it is the 4-corner mean of an 18×18-tile box. (The MIN-over-perimeter samplers are the *other*
   functions `sub_48F20`/`sub_48FD0` at EF:32647+, which the flood does NOT call.)

2. **`sub_439A0` (Terrain:1459) is a MEAN-of-8-neighbours restore, NOT sum-minus-extremes.** It sums the
   **8 surrounding cells** (a ring; the centre is excluded from the sum), `>>3` to average, then applies a
   flatness gate on `(center-min)`/`(max-center)` deltas and returns either the raw height, the average,
   or `(average+center)>>1`. Runs only when `mapAngle & 7` is nonzero; otherwise returns the cell's own
   height unchanged. `max`/`min`/`center` and the deltas are **uint8 wrap** comparisons.

3. **The shove distance tests use TWO different distance functions.** `EuclideanDistXY_584D0`
   (Maths:1043) returns the **SQUARED** XY distance (no sqrt) → the `< 0xA90000` window test is against
   `dist²`. `EuclideanDistXYZ_58490` (Maths:738) returns the **sqrt** but is XY-only (ignores z despite
   the name) → the `< 3840`, `< 3328`, `< 2304` tests are true tile-distances (in 1/256-tile units,
   i.e. `<< 8` fixed). So the flood's "radius 15 tiles" = `3840 = 15<<8`.

4. **The (10,67) spawn seam has THREE entry points, and one of them DOES consume par1 at level load.**
   The hardcoded ctor `sub_51730` (EF:37421) sets life=120, subSpell=20000 with NO SPELLS lookup. But
   the **triggered-spawn** path `sub_4A310` case 0xA (EF:33158, model 0x43 → LABEL_65) overwrites
   `life_0x8` from `SPELLS[20].subspell[par1].life_0x1A` and `subSpellIndex` from
   `SPELLS[20].subspell[par1].subSpellIndex_2`. The EV:387 dispatch's par1-override case list is indeed
   only `9/0xB/0xF` (+field cases 0x52–0x58), so EV:387 does NOT touch model 67 — but `sub_4A310` is a
   SEPARATE seam that fires authored `entity_0x30311` rows on trigger and DOES consume par1 for model 67.
   State this explicitly in the port: **model 67 authored + triggered ⇒ par1 selects SPELLS row 20.**

---

## 1. `sub_439A0` — neighbour-averaged height restore (Terrain:1459) — VERBATIM

Called by the finisher `sub_396D0` phase-2 settle loop (per cell) and by the "final snap" in the
every-4th-tick restore (`dword_0x10_16 < 3`). Restores a crater cell toward its 8-neighbour mean.

```c
// Terrain.cpp:1459
unsigned int sub_439A0(uint16_t index)//2249a0
{
    //    X          the "B" is the center (index); the 8 X's are the ring summed.
    //    |
    //  X-B-X
    //    |
    //    X
    uint8_t maxHeight, minHeight, centerPoint;
    unsigned int modSumaPoint;
    int sumaPoint = 0;
    unsigned int result = mapHeightmap_11B4E0[index];   // default = own height
    uaxis_2d uindex;  uindex.word = index;
    if (mapAngle_13B4E0[uindex.word] & 7)                // GATE: only if low-3 angle bits set
    {
        maxHeight   = mapHeightmap_11B4E0[uindex.word];  // seed max/min/center from CENTER
        minHeight   = maxHeight;
        centerPoint = mapHeightmap_11B4E0[uindex.word];
        // --- walk the 8-ring, y--, x++, y++, y++, x--, x--, y--, y-- (never revisits center) ---
        uindex._axis_2d.y--;  sumaPoint  = mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.x++;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.y++;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.y++;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.x--;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.x--;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.y--;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        uindex._axis_2d.y--;  sumaPoint += mapHeightmap_11B4E0[uindex.word];  /*upd max/min*/
        // (each "upd max/min" is: if h>maxHeight maxHeight=h; if h<minHeight minHeight=h;)
        modSumaPoint = sumaPoint >> 3;                   // MEAN of the 8 ring cells (int, truncating)
        if ((uint8_t)(centerPoint - minHeight) <= 4)     // center within +4 of the lowest neighbour?
        {
            if ((uint8_t)(maxHeight - centerPoint) <= 4)     // AND highest within +4 of center → flat
                return result;                               //   → keep own height (no smoothing)
            if ((uint8_t)(maxHeight - centerPoint) <= 10)    // mild upward step
                modSumaPoint = (centerPoint + modSumaPoint) >> 1;   // half-blend toward mean
        }
        else if ((uint8_t)(centerPoint - minHeight) <= 10)   // center a mild bump above lowest
        {
            return (modSumaPoint + centerPoint) >> 1;        // half-blend, EARLY return
        }
        result = modSumaPoint;                           // otherwise: full mean
    }
    return result;
}
```

**Exact semantics for the port:**
- **Gate:** if `mapAngle_13B4E0[index] & 7 == 0`, return the cell's own height unchanged.
- **Ring sum:** 8 cells around the center, center excluded. The 8 offsets in walk order (relative to
  center, x=east y=south): `(0,-1) (1,-1) (1,0) (1,1) (0,1) (-1,1) (-1,0) (-1,-1)`. `>>3` = mean.
- `max`/`min` seeded from the CENTER, then updated over the 8 ring cells. `centerPoint`, `maxHeight`,
  `minHeight`, and the `(a-b)` deltas are all **uint8** — the `(uint8_t)(x - y) <= K` tests are
  **modulo-256 wrap** comparisons (matters when `x < y`: e.g. `center-min` underflows to a large value,
  failing `<=4`/`<=10`, which is the intended "center is BELOW its neighbours" fall-through to full mean).
- **Return value is a MEAN (or half-blends of mean+center), NEVER sum-minus-extremes.** The four outcomes:
  1. flat (both deltas ≤4) → own height;
  2. small delta up (center-min ≤4, max-center in 5..10) → `(center + mean)/2`;
  3. center is a bump (center-min in 5..10) → `(mean + center)/2` (early return);
  4. everything else (incl. center below neighbours, or large deltas) → `mean`.
- All divides are C integer `/` on positive operands (truncating toward zero); `>>1`/`>>3` on
  non-negative ints. No rounding.

---

## 2. `sub_39FA0` — shove-victim filter (EF:29214) — VERBATIM

Called per candidate entity by `sub_39B60`. Returns 1 = shoveable, 0 = skip. Decision ladder keyed on
`class - 1` (so case N ⇒ class N+1).

```c
// EF:29214
char sub_39FA0(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x)//21afa0
{
    char result = 1;
    unsigned __int8 v3 = a2x->class_0x3F_63 - 1;
    if (v3 <= 0xE) {
        switch (v3) {
        case 0u:   // class 1
        case 3u:   // class 4
        case 5u:   // class 6
        case 6u:   // class 7
        case 7u:   // class 8
        case 0xAu: // class 11
        case 0xBu: // class 12
        case 0xCu: // class 13
        case 0xEu: // class 15
            return 0;                                   // NEVER shoveable
        case 1u:   // class 2
            return result;                              // ALWAYS shoveable (result=1)
        case 2u:   // class 3  (WIZARD / PLAYER)
            v7 = a2x->model_0x40_64;
            if (v7 < 1u) {                              // model 0 (the flying wizard body)
                if (v7) return result;                  //   (dead branch; v7<1 && v7 → never)
            } else {
                if (v7 > 1u) {                          // model >= 2
                    if (v7 != 2) return result;         //   model !=2 → shoveable
                    return 0;                           //   model 2 → NOT shoveable
                }
                if (a2x->struct_byte_0xc_12_15.byte[0] & 0x21)   // model 1 with flag 0x21 set
                    return 0;                                    //   → NOT shoveable
            }
            if (a1x->id_0x1A_26 == a2x->id_0x1A_26)     // model 0 (or model 1 no-flag): skip SELF
                result = 0;
            break;
        case 4u:   // class 5  (MOBS / spell effects)
            if (a2x->struct_byte_0xc_12_15.byte[0] & 0x21) return 0;   // flag 0x21 → skip
            if (a2x->actionIndex_0x45_69 == 232) return 0;             // action 232 → skip
            v5 = a2x->model_0x40_64;
            if (v5 < 0x16u) return result;              // model < 0x16 → shoveable
            if (v5 <= 0x16u) return 0;                  // model == 0x16 → NOT shoveable
            if (v5 != 27) return result;                // model in 0x17..? and != 27 → shoveable
            if (a2x->actionIndex_0x45_69 == 233) return 0;   // model 27 + action 233 → skip
            if (a2x->actionIndex_0x45_69 == 234) result = 0; // model 27 + action 234 → skip
            return result;
        case 8u:   // class 9
            v6 = a2x->model_0x40_64;
            if (!v6) return result;                     // model 0 → shoveable
            if (v6 < 0xDu) return 0;                     // model 1..12 → skip
            if (v6 > 0xEu) result = 0;                   // model >14 → skip
            return result;                               // model 13,14 → shoveable
        case 9u:   // class 10
            v8 = a2x->model_0x40_64;
            if (v8 < 0x27u)      v9 = (v8 == 6);          // model < 0x27: shoveable ONLY if model 6
            else { if (v8 <= 0x28u) return result;        // model 0x27,0x28 → shoveable
                   v9 = (v8 == 57); }                     // model > 0x28: shoveable only if model 57
            if (!v9) result = 0;
            return result;
        case 0xDu: // class 14
            if (a2x->struct_byte_0xc_12_15.byte[0] & 0x21 || a2x->model_0x40_64 == 1) return 0;
            return result;
        }
    }
    return result;                                       // classes 0 or >15 → default shoveable
}
```

**Port summary table** (class = case+1):

| class | rule |
|---|---|
| 0 | falls through → **1** (default) |
| 1,4,6,7,8,11,12,13,15 | **0** (never) |
| 2 | **1** (always) |
| 3 (wizard/player) | model 2 → 0; model 1 with `byte[0]&0x21` → 0; else 1, but **0 if it is SELF** (`a1x->id == a2x->id`) |
| 5 (mobs/effects) | `byte[0]&0x21` → 0; action 232 → 0; model 0x16 → 0; model 27 with action 233/234 → 0; else 1 |
| 9 | model 0 →1; model 1..12 →0; model 13,14 →1; model >14 →0 |
| 10 | shoveable only if model 6, 0x27, 0x28, or 57; else 0 |
| 14 | `byte[0]&0x21` or model 1 → 0; else 1 |
| >15 | **1** (default) |

`byte[0] & 0x21` = flag bits 0 and 5 of `struct_byte_0xc_12_15.byte[0]` (bit0 = "active/visible-ish",
bit5 = "immune/held"). No RNG, no distance — pure class/model/flag test.

---

## 3. `sub_39B60` — radius entity-shove (EF:29011) — VERBATIM

The physical push, called every tick while `dword_0x10_16 < 6` (phase 2), in phase 3, from action-73
(`sub_396A0`) each tick, and from the finisher phase-0.

```c
// EF:29011
void sub_39B60(type_entity_0x6E8E* a1x)//21ab60
{
    // window origin = centerTile - 13 (so a 26x26 tile scan window, x∈[cx-13,cx+12], same for y)
    v1        = ((unsigned __int16)(a1x->position_0x4C_76.y + 128) >> 8) - 13;   // origin Y
    LOBYTE(v15) = ((unsigned __int16)(a1x->position_0x4C_76.x + 128) >> 8) - 13; // origin X
    BYTE1(v15) = v1;   v17 = 0;   v16 = v15;
    do {                                              // 26 rows
        v18 = 0;  LOBYTE(v16) = v15;
        while ((signed __int16)v18 < 26) {            // 26 cols
            v12x.x = (unsigned __int8)v16   << 8;      // cell center in world units (tile<<8)
            v12x.y = HIBYTE(v16)            << 8;
            if ((unsigned int)Maths::EuclideanDistXY_584D0(&a1x->position_0x4C_76, &v12x) < 0xA90000)
            {                                          // DISC test: dist_XY² < 0xA90000 (= 2704<<10; ~13-tile radius)
                for (i = mapEntityIndex_15B4E0[v16]; ; i = v10x->oldMapEntity_0x16_22) {
                    v10x = Entities_EA3E4[i];
                    if (v10x == Entities_EA3E4[0]) goto LABEL_35;      // end of cell's entity list
                    if (sub_39FA0(a1x, Entities_EA3E4[i])) {           // victim filter (§2)
                        v3 = Maths::EuclideanDistXYZ_58490(&v10x->position_0x4C_76, &a1x->position_0x4C_76);
                        v4 = a1x->word_0x2C_44;                        // dome-top reference height
                        v5 = v10x->position_0x4C_76.z - v4;            // victim z above dome ref
                        v14 = v5;
                        if (v3 < 3328 && v5 < 4096) break;             // in-range AND below ceiling → shove it
                    }
                LABEL_25:                              // action-74 grab-RELEASE for entities NOT shoved
                    if (a1x->actionIndex_0x45_69 == 74 && v10x->struct_byte_0xc_12_15.byte[2] & 0x10) {
                        if (v10x->class_0x3F_63 != 3
                            || v10x->model_0x40_64
                            || v10x->dword_0xA4_164x->playerColorIndex_0x38_56 != D41A0_0.LevelIndex_0xc)
                            v10x->struct_byte_0xc_12_15.byte[0] &= 0xFEu;   // clear bit0 (hide) for non-local
                        else
                            v10x->struct_byte_0xc_12_15.byte[0] |= 1u;      // set bit0 (show) for LOCAL player
                        v10x->struct_byte_0xc_12_15.byte[2] &= 0xEFu;       // clear grab bit4
                    }
                }
                // ---- reached only when break'd out (a shoveable, in-range victim) ----
                if (v3 <= 32 || v5 <= 96) {            // very close OR barely above ref → DAMAGE pass
                    sub_3A200(a1x, v10x);              //   (§5) tag+damage, no positional push
                    goto LABEL_25;
                }
                predictedAxis_EB398ar = v10x->position_0x4C_76;         // work copy of victim pos
                v6 = ((3328 - v3) << 8) / 3328 << 7 >> 8;               // radial push force (see below)
                if (v6 < 4)   v6 = 4;                                   // clamp lo
                if (v6 > 128) v6 = 128;                                 // clamp hi
                if (v6 > v3)  LOWORD(v6) = v3;                          // never overshoot the center
                v7 = Maths::sub_581E0_maybe_tan2(&predictedAxis_EB398ar, &a1x->position_0x4C_76); // yaw victim→center
                MoveEntity_57FA0(&predictedAxis_EB398ar, v7, 0, v6);    // step v6 units toward center? see NOTE
                v8 = (signed __int16)getTerrainAlt_10C40(&predictedAxis_EB398ar);   // ground under new xy
                if (v10x->class_0x3F_63 == 3 && !v10x->model_0x40_64) { // wizard/player body: pull DOWN
                    v9 = (signed __int16)(predictedAxis_EB398ar.z
                         - (48 * ((((4096 - v14) << 8) - <arith-shift-round>) >> 12) >> 8));
                    predictedAxis_EB398ar.z -= 48 * ((((4096 - v14) << 8) - <arith-shift-round>) >> 12) >> 8;
                    goto LABEL_21;
                }
                if (v10x->dword_0xA0_160x->word_160_0xe_14 < -64) goto LABEL_40;    // deep sink: clamp to ground
                predictedAxis_EB398ar.z -= 48 * ((((4096 - v14) << 8) - <arith-shift-round>) >> 12) >> 8;
                v9 = predictedAxis_EB398ar.z;
            LABEL_21:
                if (v9 < v8) LABEL_40: predictedAxis_EB398ar.z = v8;    // never below terrain
                CopyEntityPosition_57CF0(v10x, &predictedAxis_EB398ar); // COMMIT new pos to victim
                goto LABEL_25;
            }
        LABEL_35:
            v18++;  LOBYTE(v16) = v16 + 1;
        }
        result = v17++ + 1;  HIBYTE(v16)++;
    } while ((signed __int16)v17 < 26);
}
```

**Exact semantics:**
- **Entity list walked:** the **map-cell entity index** `mapEntityIndex_15B4E0[cell]` → chased via
  `oldMapEntity_0x16_22` (the per-cell linked list). NOT `dword_38519/23/27`. So it's a spatial hash walk
  over a **26×26 tile window** (origin = centerTile-13), further gated by the **disc** test
  `EuclideanDistXY_584D0 < 0xA90000` (squared XY dist; `0xA90000 = 11075584 = 3328²`, i.e. a **13-tile
  radius disc**, since `3328 = 13<<8`).
- **Per-victim range gate (to actually shove):** `EuclideanDistXYZ_58490 < 3328` (13-tile true XY dist)
  **AND** `(victim.z - word_0x2C_44) < 4096` (victim within 16 tiles above the dome-top reference). Else
  the entity is skipped (only the action-74 release block at LABEL_25 runs for it).
- **Close-range → damage instead of push:** if `dist < 32` (v3≤32) OR `victim.z - ref ≤ 96`, call
  `sub_3A200` (§5) and do NOT reposition.
- **Radial push force `v6`:** `v6 = (((3328 - dist) << 8) / 3328) << 7 >> 8`, i.e.
  `v6 = ((3328 - dist) * 256 / 3328) * 128 / 256 = ((3328 - dist) * 128) / 3328` (0 at rim, ~128 at
  center), then **clamp [4,128]**, then **cap to `dist`** (`if v6 > v3, v6 = v3`, as a 16-bit `LOWORD`
  write). All ints, truncating.
- **Direction:** `sub_581E0_maybe_tan2(victimPos, centerPos)` = yaw from victim toward center; then
  `MoveEntity_57FA0(&pos, yaw, pitch=0, speed=v6)` advances the copy by `v6` along that yaw. NOTE: yaw
  points victim→center, so the "shove" actually walks the work-copy TOWARD center by v6; the strong
  downward z pull is what lifts-then-settles. (This matches the effect being a crater collapse pulling
  things into the pit, not blasting them out — confirm against ORIGINAL GAMEPLAY when porting.)
- **Z handling:** `z -= 48 * (((4096 - (victim.z-ref)) << 8) >> 12) >> 8` (a downward pull proportional to
  how far below 4096 the victim sits; the `<arith-shift-round>` is IDA's rendering of C signed `>>12` of a
  possibly-negative value, i.e. an arithmetic shift — for our port `(int32) x >> 12` suffices). For
  class-3 model-0 (player/wizard) this always applies. For others it applies unless
  `word_160_0xe_14 < -64` (already sinking → skip to ground clamp). Finally `if new_z < terrainAlt,
  new_z = terrainAlt` (never below ground). Commit via `CopyEntityPosition_57CF0`.
- **Action-74 mode extra block (grab release, LABEL_25):** ONLY when `actionIndex == 74` AND the victim
  has `byte[2] & 0x10` (was grabbed). It restores the LOCAL player's visibility bit and hides everyone
  else, then clears the grab bit:
  - `v10x->byte[0] |= 1` **iff** `class==3 && model==0 && dword_0xA4_164->playerColorIndex_0x38_56 ==
    D41A0_0.LevelIndex_0xc` (the LOCAL player) — sets **byte[0] bit0** (draw/visible).
  - **else** `v10x->byte[0] &= 0xFE` (clear bit0 — keep hidden).
  - always `v10x->byte[2] &= 0xEF` (clear **byte[2] bit4** = the 0x10 grab flag).
- **Victim fields WRITTEN:** position (via `CopyEntityPosition_57CF0`) for shoved entities; `byte[0]` bit0
  and `byte[2]` bit4 for grab-release in action-74; nothing else here (velocity/homes are NOT touched by
  this function — the damage/tag writes live in `sub_3A200`/`sub_3A090`).

---

## 4. `sub_3A090` — damage/grab pass (EF:29316) — VERBATIM

Called once at phase-2 countdown step 5, and at the top of finisher phase-0.

```c
// EF:29316
void sub_3A090(type_entity_0x6E8E* a1x)//21b909
{
    int v8 = 0;
    // ---- (a) kill overlapping BUILDINGS: list dword_38527 = (10,45) ----
    for (ix = x_D41A0_BYTEARRAY_4_struct.dword_38527; ix > Entities_EA3E4[0]; ix = ix->next_0) {
        if (CompareAxisWithShift_10750(a1x, ix)) {         // AABB overlap (§8)
            ix->life_0x8 = -1;                             // kill it
            ix->fontTypeIndex_0x3D_61 = 0;
        }
    }
    // ---- (b) grab overlapping model-2 OBJECTS: list dword_38519 ----
    for (jx = x_D41A0_BYTEARRAY_4_struct.dword_38519; jx > Entities_EA3E4[0]; jx = jx->next_0) {
        if (jx->model_0x40_64 == 2 && CompareAxisWithShift_10750(a1x, jx)) {
            v3 = x_DWORD_E9B90 + 1;
            jx->struct_byte_0xc_12_15.byte[2] |= 0x10u;    // set grab bit4
            x_DWORD_E9B90 = v3;                            // global "objects grabbed this tick" counter
            jx->word_0x30_48 = 30;                         // grab timer = 30
            jx->word_0x26_38 = a1x - D41A0_0.struct_0x6E8E;// owner = self (entity index)
            jx->str_0x5E_94.dword_0x5E_94 += a1x->subSpellIndex_0x2A_42;   // DAMAGE MAILBOX: += subSpell
            v8 += 2;
            jx->str_0x5E_94.word_0x62_98 = a1x->id_0x1A_26;               // mailbox source id
        }
    }
    // ---- (c) 30x30 terrain sweep: burnable-by-flags cells → lava-type 1 ----
    v11 = ((unsigned __int16)(a1x->position_0x4C_76.x + 128) >> 8) - 15;   // origin X = cx-15
    for (HIBYTE(v6) = ((unsigned __int16)(a1x->position_0x4C_76.y + 128) >> 8) - 15; ; ++HIBYTE(v6)) {
        if (v5 >= 30) break;
        LOBYTE(v6) = v11;
        for (k = 0; k < 30; k++) {
            if ((unsigned int)sub_10590_terrain_tile_type(mapTerrainType_10B4E0[v6]) & 0x7F0000) {
                v7 = mapAngle_13B4E0[v6] & 0xF8 | 1;
                mapTerrainType_10B4E0[v6] = 1;             // → lava
                mapAngle_13B4E0[v6] = v7;                  // low3 angle = 1
            }
            LOBYTE(v6) = v6 + 1;
        }
        v5 = v10 + 1;
    }
    if (v8)
        sub_6D8B0(a1x->id_0x1A_26, 0x14u, v8);            // spellbook report, effect id 0x14=20, count v8
}
```

**Exact semantics:**
- **Building kill (a):** walks `dword_38527` — the class-10 MODEL-45 list (builder EF:40043-51; the older "EFFECT list" reading was wrong, corrected 2026-08-11); overlap = `CompareAxisWithShift_10750`
  (AABB, §8); sets `life = -1` and `fontTypeIndex_0x3D_61 = 0`.
- **Object grab (b):** walks `dword_38519` (the OBJECT list); for **model 2** objects that AABB-overlap:
  sets **byte[2] |= 0x10** (grab bit4), `word_0x30_48 = 30` (grab timer), `word_0x26_38 = self index`
  (owner), **`str_0x5E_94.dword_0x5E_94 += a1x->subSpellIndex_0x2A_42`** — the DAMAGE MAILBOX write:
  channel = `str_0x5E_94.dword_0x5E_94` (accumulated pending damage), amount = **the flood's own
  `subSpellIndex_0x2A_42`** (20000 for the hardcoded spawn, or the SPELLS[20] value for a triggered one).
  Also `str_0x5E_94.word_0x62_98 = self.id` (mailbox source), `v8 += 2`, `x_DWORD_E9B90++`.
- **Terrain sweep (c):** 30×30 tiles (origin center-15). Uses `sub_10590_terrain_tile_type(type) & 0x7F0000`
  (see §7 for the table). Bit 0x7F0000 = types {10,11,12} (lava/water family) and {21,22,23,24,25,26,27}
  (bridge/wall family); NOT default (0x800000, bit 23) and NOT the low families. Those cells → type 1 (lava)
  with `mapAngle low3 = 1`.
- **Report:** if any object grabbed (`v8 != 0`), `sub_6D8B0(self.id, 0x14, v8)` — effect id **0x14 = 20**
  (= SPELLS row 20, the flood's own spell slot; this is the XP/spellbook accumulator, cf. m9-doc §1).

---

## 5. `sub_3A200` — per-entity shove callback (EF:29382) — VERBATIM

Called by `sub_39B60` for very-close victims (dist≤32 or z≤96 above ref). Tags + rolls damage.

```c
// EF:29382
void sub_3A200(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x)//21b200
{
    bool v2 = 0;  char v7 = 0;
    unsigned __int8 v3 = a2x->class_0x3F_63;
    a2x->struct_byte_0xc_12_15.dword |= 0x100001;              // FLAG WRITE (see decode below)
    if (v3 < 3u) goto LABEL_13;
    if (v3 > 3u) {
        if (v3 != 5) goto LABEL_13;                            // only class 5 handled specially
        v4 = a2x->model_0x40_64;
        if (v4 < 0x12u) { if (v4 != 12) goto LABEL_13; }       // class5 model 12 → v2=1
        else if (v4 > 0x12u) {                                 // class5 model >0x12
            if (v4 == 27) v7 = 1;                              //   model 27 → v7=1 (suppress damage)
            goto LABEL_13;
        }
        v2 = 1;                                                // class5 model 0x12 (or 12) → force-damage
        goto LABEL_13;
    }
    // v3 == 3  (class 3, wizard/player)
    if (!a2x->model_0x40_64) {                                 // model 0 (the wizard body): SPIN
        a2x->pitch_0x1E_30 = 512;                              //   pitch = 512
        a2x->dword_0xA4_164x->pitch_0x157_343 = 512;           //   mirror to the flight struct
    }
LABEL_13:
    if (!v2) {                                                 // not force-damaged → roll the dice
        a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;   // per-SOURCE RNG stream (a1x, the flood)
        v2 = a1x->rand_0x14_20 % 7u == 0;                      // 1-in-7 damage
    }
    if (v2 && !v7) {                                           // damage rolled AND not suppressed
        if (a2x->byte_0x38_56 & 1) {                           // victim gate: byte_0x38_56 bit0 set
            a2x->str_0x5E_94.dword_0x5E_94 += a2x->life_0x8 + 1;   // MAILBOX += (victim.life + 1)
            v6 = a1x->id_0x1A_26;
            a2x->str_0x5E_94.word_0x62_98 = v6;                //   mailbox source = flood id
            sub_6D8B0(v6, 0x14u, 1);                           //   report effect 0x14, count 1
        }
    }
}
```

**Exact semantics:**
- **Flag write `a2x->struct_byte_0xc_12_15.dword |= 0x100001`:** sets **byte[0] bit0** (0x000001 → draw/
  active) and **byte[2] bit0** (0x100000 = bit20 = byte2's bit0 → an "airborne/hit" marker). The commented
  original `byte[0] |= 1; byte[e]... |= 0x10` shows intent; the live code ORs the whole dword with
  `0x100001`, i.e. **byte[0] |= 0x01 and byte[2] |= 0x10** — WAIT: `0x100001` = byte0:0x01, byte1:0x00,
  byte2:0x10, byte3:0x00. So it sets **byte[0] bit0 AND byte[2] bit4 (the grab flag)**. (Decode:
  `0x100001` little-endian dword = bytes {01,00,10,00}.) The parent doc's "byte[c..f] |= 0x100001
  (airborne/grabbed)" is confirmed as **byte[0].bit0 + byte[2].bit4**.
- **Class-5 arms:** model 0x12 (or decimal 12) → `v2=1` (force damage this call, skip RNG); model 27 →
  `v7=1` (suppress the damage/report entirely).
- **Class-3 spin:** model 0 → `pitch_0x1E_30 = 512` and mirror `dword_0xA4_164->pitch_0x157_343 = 512`
  (a fixed pitch flip — NO roll write here; only pitch=512). (No roll term in this function despite the
  parent summary's "pitch/roll"; it is pitch only.)
- **RNG stream:** the **SOURCE** entity's `a1x->rand_0x14_20` (the flood's own per-entity RNG), advanced by
  the standard law `r = 9377*r + 9439`; `r % 7 == 0` = 1-in-7. NOT a global RNG, NOT the victim's.
- **Damage gate + mailbox:** requires `a2x->byte_0x38_56 & 1` (victim's bit0 — "damageable"). Amount added
  to the mailbox = **`a2x->life_0x8 + 1` (the VICTIM's own current life + 1)** — i.e. it deals damage
  equal to the victim's remaining life, a near-guaranteed kill on the roll. Mailbox source id = flood id;
  reports `sub_6D8B0(floodId, 0x14, 1)`.

---

## 6. `GetTerrainHeightFromSquare_48DF0` — the phase-1 sampler (EF:32605) — VERBATIM

```c
// EF:32605
__int16 GetTerrainHeightFromSquare_48DF0(char x, char y, char height, char width)//229df0
{
    uaxis_2d locAxis1, locAxis2, locAxis3, locAxis4;
    locAxis1 = (x,          y);            // NW corner
    locAxis2 = (x + width,  y);            // NE corner
    locAxis3 = (x + width,  y + height);   // SE corner
    locAxis4 = (x,          y + height);   // SW corner
    return (mapHeightmap_11B4E0[locAxis1.word] + mapHeightmap_11B4E0[locAxis2.word]
          + mapHeightmap_11B4E0[locAxis3.word] + mapHeightmap_11B4E0[locAxis4.word]) >> 2;
}
```

**Exact semantics:** NOT max, NOT min, NOT a box scan. Samples the **4 CORNERS ONLY** of the axis-aligned
box `[x, x+width] × [y, y+height]` and returns their **arithmetic mean** `sum >> 2` (truncating; sum ≤
4·255 = 1020 so no overflow in the int16 return). Phase-1 call is
`GetTerrainHeightFromSquare_48DF0(cx-9, cy-9, 18, 18)` → the mean of the four corners of an 18×18-tile box
centered on the entity (corners at ±9 tiles). Args are `char` (signed byte → the tile coords wrap in the
256×256 map word index, which is the intended toroidal addressing). **CORRECTION to parent doc:** it is a
4-corner MEAN, not an 18×18 max.

---

## 7. `sub_10590_terrain_tile_type` — the terrain flags table (Terrain:2067) — VERBATIM

The `& 0x7F0000` test in `sub_3A090` (§4) and the `& 0x7F0000` in `sub_57450`-adjacent code use this
table. It maps a terrain-type byte to a **1-hot (mostly) flags word**:

```c
// Terrain.cpp:2067
uint32_t sub_10590_terrain_tile_type(char tileType)//1f1590
{
    switch (tileType) {
    case 0:  return 1;            // 0x000001
    case 1:  return 2;            // 0x000002   (lava)
    case 2:  return 4;            // 0x000004
    case 3:  return 8;            // 0x000008
    case 4:  return 0x10;
    case 5:  return 0x20;
    case 8:  return 0x100;
    case 9:  return 0x200;
    case 10: return 0x100000;     // \
    case 11: return 0x200000;     //  } bits 20,21,22  ← caught by 0x7F0000
    case 12: return 0x400000;     // /
    case 13: case 14: return 0;   // (no flag — NOT burnable/convertible)
    case 15..20, 28..34: return 0x400;
    case 21: case 22: case 24: return 0x20000;   // \
    case 23: return 0x40000;                     //  } bits 16,17,18,19 ← caught by 0x7F0000
    case 25: case 27: return 0x80000;            //  /
    case 26: return 0x10000;                     // /
    default: return 0x800000;    // bit 23 — NOT caught by 0x7F0000
    }
}
```

**`0x7F0000` = bits 16..22.** So the "convert to lava" set in `sub_3A090` (c) is exactly:
- bits 16..19 (0x10000/0x20000/0x40000/0x80000) ⇒ types **26, 21/22/24, 23, 25/27** (the bridge/wall/
  scenery-floor family), plus
- bits 20..22 (0x100000/0x200000/0x400000) ⇒ types **10, 11, 12** (the water/lava-edge family).

Explicitly EXCLUDED from the `0x7F0000` test: types 0–9 (low bits), 13/14 (zero), 15–20/28–34 (0x400 =
bit10), and everything in `default` (0x800000 = bit23). So `sub_3A090`'s sweep converts a DIFFERENT set
than the phase-2/3 `sub_57450` predicate — keep both predicates distinct in the port.

---

## 8. `CompareAxisWithShift_10750 / _106F0` — the overlap geometry (EF:3733/3726) — VERBATIM

```c
// EF:3726
bool CompareAxisWithShift_106F0(axis_3d* a1, axis_4d* a2, axis_3d* a3, axis_4d* a4)//1f16f0
{
    return (abs((int16_t)((int16_t)a3->x - (int16_t)a1->x)) < (int16_t)a2->pitch + (int16_t)a4->pitch)
        && (abs((int16_t)((int16_t)a3->y - (int16_t)a1->y)) < (int16_t)a2->roll  + (int16_t)a4->roll);
}
// EF:3733
bool CompareAxisWithShift_10750(type_entity_0x6E8E* e, type_entity_0x6E8E* e2)//1f1750
{ return CompareAxisWithShift_106F0(&e->position_0x4C_76, &e->array_0x52_82,
                                    &e2->position_0x4C_76, &e2->array_0x52_82); }
```

**Exact semantics:** a 2-D **AABB overlap** on XY only. Overlap iff `|dx| < (a.pitch + b.pitch)` AND
`|dy| < (a.roll + b.roll)`, where `array_0x52_82.pitch` / `.roll` are each entity's **XY half-extents**
(the AABB is `[pos - halfextent, pos + halfextent]`; the test is the Minkowski-sum "sum of half-extents").
All int16, `abs` of a signed int16 delta, strict `<`. NO z term. This is the overlap `sub_3A090` uses for
both the effect-kill and object-grab passes.

---

## 9. Flood phase-2 inline shading (EF:28626-28654) — VERBATIM

The tail of the per-cell loop in `sub_39040` phase 2. The parent doc summarized it; here is the exact code.

```c
// EF:28624 (after the height write, still inside the 30x30 cell loop; v8x = current cell)
v8x._axis_2d.x++;                                   // move to (x+1, y+1)
v8x._axis_2d.y++;
v18x.word = v8x.word;                               // v18x = index of (x+1, y+1)  ← the SE neighbour
v8x._axis_2d.x -= 2;                                // move to (x-1, y-1)
v8x._axis_2d.y -= 2;
v19 = mapHeightmap_11B4E0[v8x.word];                // v19 = height at (x-1, y-1)   ← the NW neighbour
v8x._axis_2d.x++;                                   // restore x (now (x, y-1))
v20 = v19 - mapHeightmap_11B4E0[v18x.word] + 32;    // v20 = H(NW) - H(SE) + 32   ← NW minus SE!
v8x._axis_2d.y++;                                   // restore y (back to (x,y))
if (v20 >= 28) {
    if (v20 > 40) v20 = (v20 & 7) + 40;             // clamp high band: [40..47] via (v20&7)+40
}                                                    // (28 <= v20 <= 40: leave as-is)
else {
    v20 = (v20 & 3) + 28;                            // clamp low band: [28..31] via (v20&3)+28
}
if (D41A0_0.terrain_2FECE.MapType != MapType_t::Day)
    v21 = 32 - v20 + 32;                            // NIGHT flip: v21 = 64 - v20
else
    LOBYTE(v21) = v20;                              // DAY: v21 = v20
v22 = isCaveLevel_D41B6;
mapShading_12B4E0[v8x.word] = v21;                  // write shading for THIS cell (x,y)
if (v22) {                                          // cave: set/clear mapAngle bit3 (0x08)
    if (x_BYTE_14B4E0_second_heightmap[v8x.word] > mapHeightmap_11B4E0[v8x.word])
        mapAngle_13B4E0[v8x.word] &= 0xF7u;         //   ceiling above floor → clear bit3
    else
        mapAngle_13B4E0[v8x.word] |= 8u;            //   floor >= ceiling → set bit3
}
v8x._axis_2d.x++;                                    // advance loop to next cell
```

**Exact semantics:**
- The gradient is **`H(x-1, y-1) − H(x+1, y+1) + 32`** = **NW minus SE**. This is the SAME orientation as
  `AddBuildingToTerrain_46570`'s pass C (see `mc2-class10-m9-dome-open-closure.md §3`, which samples NW
  minus SE): identical formula, identical clamp bands, identical Day/Night flip. So the flood's phase-2
  cell shading is byte-for-byte the same shading recompute as the building/dome path. Port them from one
  shared helper.
- **Clamp bands (exact order):** compute `v20 = NW - SE + 32`; if `v20 >= 28`: if `v20 > 40` then
  `v20 = (v20 & 7) + 40` (→ 40..47), else keep (28..40); else `v20 = (v20 & 3) + 28` (→ 28..31). Then
  **Day:** `shade = v20`; **Night (MapType != Day):** `shade = 64 - v20` (`32 - v20 + 32`).
- `v21`/`v20` are effectively bytes written to `mapShading_12B4E0`.

**Finisher (`sub_396D0`) shading recompute (EF:28964-28995):** IDENTICAL formula and clamp/flip — the
finisher's per-cell tail (in the every-4th-tick restore loop) samples `v17 = H(NW)` at `(x-1,y-1)` and
`v18 = v17 - H(SE) + 32` at `(x+1,y+1)`, same `>=28 / >40` bands, same `32 - v18 + 32` night flip, same
cave bit3 handling. No difference from phase 2. (Verbatim confirmed at EF:28962-28992.)

---

## 10. `MoveEntity_57FA0` + the tan2 helper as used by the flood (Player:6, Maths:819) — VERBATIM

```c
// Player.cpp:6
void MoveEntity_57FA0(axis_3d* position, uint16_t yaw, int16_t pitch, int16_t speed)//238fa0
{
    if (speed) {
        pitch &= 0x7ff;  yaw &= 0x7ff;                        // wrap to [0,0x7ff]  (0x800 = quarter turn)
        if (pitch) {
            position->z -= (int)(speed * Maths::sin_DB750[pitch]) >> 16;     // z step (sin(pitch))
            speed         = (int)(speed * Maths::sin_DB750[0x200 + pitch]) >> 16;  // horiz speed *= cos(pitch)
        }
        position->x += (int)(speed * Maths::sin_DB750[yaw])         >> 16;   // x += speed*sin(yaw)
        position->y -= (int)(speed * Maths::sin_DB750[0x200 + yaw]) >> 16;   // y -= speed*cos(yaw)
    }
}
```

**Exact semantics:**
- **Yaw/pitch units:** masked to `& 0x7ff` — so a full circle is **0x800 = 2048 units per quarter turn**?
  No: the sin LUT `sin_DB750` is indexed with `[0x200 + angle]` for the cosine (quarter-turn offset =
  0x200 = 512). So **512 units = quarter turn ⇒ 2048 units = full turn**, and the `& 0x7ff` mask keeps the
  index in-range (0x800 entries of sin covering the full circle plus the 0x200 cosine offset window). This
  matches the `sin_DB750` LUT the flood also uses for the dome falloff (`sin_DB750[0x200 + ...]`).
- **`speed`/`dist` units:** world units (tile = 256 = `1<<8`). The flood passes `speed = 3840` (=15 tiles)
  when probing the rim, and `speed = v6` (4..128, capped to dist) for the shove step.
- **Writes:** `position->x`, `->y`, and (only if pitch≠0) `->z`. Pure `sin*speed >> 16` fixed-point
  (`sin_DB750` is a 16.16 sine table). The flood always calls with `pitch = 0`, so **z is never touched by
  MoveEntity in the flood** — z is handled separately by the caller.

**tan2 helper symbol/table:** the flood's angle calls are `Maths::sub_581E0_maybe_tan2(a1, a2)`
(Maths:819), which is `sub_72633_maybe_tan(a2->x - a1->x, a2->y - a1->y)` (Maths:764). That octant-decoder
uses the **`x_WORD_DE350` arctangent LUT** (Maths:788-814): quadrant/octant selection then
`x_WORD_DE350[(smaller<<8)/larger]` with per-octant base offsets (0x200/0x400/0x600/0x800…) to return a
16-bit yaw in the **same 2048-per-full-turn units** MoveEntity consumes. **Our port's existing arctangent
LUT is the equivalent of `x_WORD_DE350`; retail's symbol is `sub_581E0_maybe_tan2` → `sub_72633_maybe_tan`
→ `x_WORD_DE350`.** (Note: `sub_58210_radix_tan` is a DIFFERENT tan that uses the z-delta + XY-radix — the
flood does NOT use that one; it uses `sub_581E0` = pure XY yaw.)

---

## 11. Center-2×2 crater-floor block, phase 2 (EF:28672-28696) — VERBATIM

```c
// EF:28672 (after the 30x30 sweep, inside phase 2, if countdown>0)
v25 = 0;
v26x._axis_2d.y = v52x._axis_2d.y - 1;                 // v52 = CENTER tile; block spans (cx-1..cx, cy-1..cy)
do {
    v46 = 0;
    v26x._axis_2d.x = v52x._axis_2d.x - 1;
    while (v46 < 2) {                                   // 2x2 cells: (cx-1,cy-1),(cx,cy-1),(cx-1,cy),(cx,cy)
        v27 = mapHeightmap_11B4E0[v26x.word]
            - (signed int)mapHeightmap_11B4E0[v26x.word] / a1x->dword_0x10_16;   // h - h/countdown
        if (v27 < 0)    v27 = 0;
        if (v27 > 255)  LOBYTE(v27) = -1;              // clamp hi → 255 (LOBYTE -1 = 0xFF)
        mapHeightmap_11B4E0[v26x.word] = v27;          // HEIGHT WRITE: drop center
        v28 = a1x->dword_0x10_16;
        v29 = 31 / v28 + 32;                           // DAY shading = 31/countdown + 32
        if (D41A0_0.terrain_2FECE.MapType != MapType_t::Day)
            v29 = -31 / v28 + 32;                      // NIGHT shading = -31/countdown + 32
        mapShading_12B4E0[v26x.word] = v29;
        v26x._axis_2d.x++;  v46++;
    }
    v25++;  v26x._axis_2d.y++;
} while (v25 < 2);
```

**Exact semantics:**
- **Cells:** the 2×2 block with corners `(cx-1, cy-1) … (cx, cy)` — i.e. center-1..center in both axes
  (same layout as the dome cap in `mc2-class10-m9`).
- **Height drop:** `h -= h / countdown` (C signed integer divide, truncating toward zero; `countdown =
  dword_0x10_16 ∈ [1..11]` here). Clamp: `if <0 → 0`, `if >255 → 255` (the `LOBYTE(v27) = -1` is IDA for
  "store 0xFF"). So the very center sinks by `1/countdown` of its height each tick — a deepening pit.
- **Shading:** `(MapType != Day ? -31 : 31) / countdown + 32`. Signed `-31 / countdown` truncates toward
  zero (e.g. countdown=2 → -15). Written to `mapShading_12B4E0` for each of the 4 cells.

(Compare phase-3, EF:28728-28747: the 2×2 shading is forced to a CONSTANT `MapType != Day ? 1 : 63` — no
countdown term — because phase 3 is the final commit.)

---

## 12. Grab-release scan in finisher phase 2 (EF:28898-28908) — VERBATIM

```c
// EF:28898 (sub_396D0, resulty==2 branch, after the sub_439A0 settle loop)
for (jx = x_D41A0_BYTEARRAY_4_struct.dword_38519; jx > Entities_EA3E4[0]; jx = jx->next_0) {
    if (jx->model_0x40_64 == 2                                  // model-2 object
        && jx->struct_byte_0xc_12_15.byte[2] & 0x10             // grabbed (bit4)
        && jx->word_0x26_38 == a1x - D41A0_0.struct_0x6E8E)     // owned by THIS flood (self index)
    {
        v27 = jx->struct_byte_0xc_12_15.byte[2];
        jx->word_0x26_38 = 0;                                   // clear owner
        jx->struct_byte_0xc_12_15.byte[2] = v27 & 0xEF;         // clear grab bit4
    }
}
DisableEntityDrawing04_57F10(a1x);                             // despawn the flood
```

**Exact semantics:** walks the **`dword_38519` OBJECT list**; for each **model-2** object with
**byte[2] & 0x10** set (grabbed) **and** `word_0x26_38 == self entity index` (owned by this flood), clears
`word_0x26_38 = 0` (owner) and `byte[2] &= 0xEF` (grab bit4). Fields cleared: exactly those two
(`word_0x26_38` and byte[2] bit4). It does NOT clear the grab timer `word_0x30_48` (that expires on its
own) nor the mailbox. Then `DisableEntityDrawing04_57F10` despawns the flood entity.

---

## 13. The (10,67) ctor and spawn seam (EF:37421, EV:5158, EF:33158, EV:362) — VERBATIM

### 13.1 Hardcoded ctor `sub_51730` (EF:37421)
```c
// EF:37421
type_entity_0x6E8E* sub_51730(axis_3d* position)//232730
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->actionIndex_0x45_69 = 0x48;                 // action 72 = sub_39040 (flood driver)
        event->class_0x3F_63 = 0xA;                        // class 10
        event->model_0x40_64 = 0x43;                       // model 67
        event->struct_byte_0xc_12_15.byte[0] =
            event->struct_byte_0xc_12_15.byte[0] & 0xF6 | 1;   // clear bits1,3; set bit0
        event->life_0x8 = 120;                             // LIFE = 120
        event->subSpellIndex_0x2A_42 = 20000;              // subSpell (damage payload) = 20000
        AddEventToMap_57D70(event, position);
        SetEntityShiftRot_49EA0(event, 4352, 4352);        // AABB half-extents pitch=roll=4352 (=17 tiles)
    }
    return event;
}
```

- **actionIndex = 0x48** (72) → the flood driver `sub_39040`.
- **life = 120, subSpell = 20000, class 10, model 67, byte[0] = (…&0xF6)|1.**
- **maxLife is NOT set here** (unlike the fissure ctor `sub_51790` at EF:37451 which sets
  `maxLife = life`). So the flood's `maxLife_0x4` retains whatever `NewEvent_4A050` initialised it to
  (typically 0) — the flood never reads its own maxLife, so this is harmless, but note it for parity.
- **No sprite/HSPR set in the ctor** (the flood is a terrain effect, drawn via its heightmap writes; the
  entity itself is `DisableEntityDrawing04`-ready — `sub_39E40` may disable drawing immediately).
- **ShiftRot args = (4352, 4352):** `SetEntityShiftRot_49EA0(event, 4352, 4352)` sets
  `array_0x52_82.pitch = 4352` and `.roll = 4352` (= 17 tiles, `4352 = 17<<8`) — these are the AABB
  half-extents `CompareAxisWithShift` uses in `sub_3A090`. So the flood's overlap box is ±17 tiles in XY.

### 13.2 Dispatch: `pre_sub_4A190_axis_3d` → EV:5158 → `sub_51730`
The spawn-by-address dispatcher (EV:~5000, the big `case 0x232730: return sub_51730(a1_axis3d);` at
**EV:5158**) invokes `sub_51730` when the SPELLS/dome row's `address_6` = 0x232730. This is the CAST path
and the type-0x2D terrain-author path (EV:346 `pre_sub_4A190_axis_3d(temp_adress, …)`).

### 13.3 par1 consumption — the THREE seams (explicit)
1. **Hardcoded (cast via 0x2D author or direct):** `sub_51730` sets life=120/subSpell=20000 with **NO
   SPELLS lookup, NO par1.** (EV:362 case list is `9/0xB/0xF` + field cases `0x52..0x58` only — model 67's
   subtype/model is not among them, confirmed at EV:364-398.) So at the **EV:340 GenerateEvents /
   ApplyEvents authored-spawn pass, model 67 does NOT consume par1 / SPELLS row 20.**
2. **Triggered-spawn `sub_4A310` case 0xA (EF:33158):** this IS a par1 consumer. For a `type_entity_0x30311`
   authored row of class 10 model 0x43 fired on trigger (`DisId == a1`, EF:32982-32984), the code path
   `v8 == 0x43` → LABEL_65 sets:
   ```c
   // EF:33148  (subSpell, all class-10 in this branch)
   v3x->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(0x43)].subspell[entity->par1_14].subSpellIndex_2;
   // EF:33165 LABEL_65 → EF:33167 LABEL_69  (life, for model 0x43)
   v9 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(0x43)].subspell[entity->par1_14].life_0x1A;
   v3x->life_0x8 = v9;
   ```
   with `GetSpellIndex_6E020(67) == 20` (EF:44253). **So a TRIGGERED authored model-67 DOES consume par1
   via SPELLS row 20** (overrides the ctor's life & subSpell). This is the seam to wire for authored levels.
3. **In-game player cast:** goes through the same `sub_4A310`/apply path (the cast fills a
   `type_entity_0x30311` from the spellbook, par1 = charge level) → same SPELLS row-20 consumption.

**`GetSpellIndex_6E020` (EF:44240) — verbatim mapping:** `9→18, 11→16, 15→17, 17→9, 22→21, 67→20, 71→15`,
default 0. Confirms **model 67 → SPELLS row 20**.

---

## Consolidated constants table

| constant | value | meaning | cite |
|---|---|---|---|
| flood action (driver) | 0x48 = 72 | `sub_39040` | EF:37426 |
| flood action (intermed) | 0x49 = 73 | `sub_396A0` | EF:28750 |
| flood action (finisher) | 0x4A = 74 | `sub_396D0` | EF:28515 |
| ctor life | 120 | hardcoded spawn | EF:37430 |
| ctor subSpell | 20000 | damage payload (mailbox amount) | EF:37431 |
| ctor ShiftRot | (4352, 4352) | AABB half-extents = 17 tiles | EF:37433 |
| SPELLS row (model 67) | 20 | `GetSpellIndex_6E020(67)` | EF:44253 |
| phase-1 sampler box | 18×18 tiles, 4-corner MEAN | `GetTerrainHeightFromSquare_48DF0(cx-9,cy-9,18,18)` | EF:28539 / :32605 |
| phase-1 z threshold | 64 | `if height>64: pos.z = h-64` | EF:28542 |
| phase-1 word_0x2C_44 | `32*(h-80)` if `h-64>16` | dome-top ref | EF:28546 |
| dome countdown | 12 | `dword_0x10_16 = 12` | EF:28550 |
| sound | 64 | `PrepareEventSound_6E450(…,64)` | EF:28551 |
| morph radius | 3840 = 15<<8 | inner<2304, ring 2304..3840 | EF:28576/:28578 |
| lava-convert gate | `h≥z+6*countdown && h≤z+64 && sub_57450` | phase-2 burnable→lava | EF:28616 |
| damage pass step | countdown == 5 | `sub_3A090` fires | EF:28659 |
| shove-active gate | `dword_0x10_16 < 6` | `sub_39B60` every tick | EF:28699 |
| shove window | 26×26 tiles, origin cx-13 | `sub_39B60` scan | EF:29032-29041 |
| shove disc (squared) | `< 0xA90000` = 3328² | XY² gate | EF:29045 |
| shove range | `dist<3328 && (z-ref)<4096` | per-victim | EF:29058 |
| close→damage | `dist≤32 \|\| (z-ref)≤96` | → `sub_3A200` | EF:29077 |
| push force | `((3328-dist)*128/3328)`, clamp[4,128], cap dist | radial | EF:29083-29089 |
| z-pull factor | `48 * ((4096-(z-ref))<<8 >>12) >>8` | downward | EF:29108 |
| deep-sink skip | `word_160_0xe_14 < -64` | → clamp ground | EF:29106 |
| terrain-convert mask | `& 0x7F0000` (bits16..22) | `sub_3A090` sweep | EF:29363 |
| grab flag | byte[2] bit4 (0x10) | model-2 grab | EF:29344 |
| grab timer | word_0x30_48 = 30 | | EF:29346 |
| grab owner | word_0x26_38 = self index | | EF:29347 |
| grab damage | mailbox += subSpell | | EF:29348 |
| shove-callback flags | dword `\|= 0x100001` = byte[0].bit0 + byte[2].bit4 | | EF:29396 |
| shove-callback RNG | source `rand_0x14_20`, `%7==0` | 1-in-7 damage | EF:29427 |
| shove-callback damage | mailbox += `victim.life+1` | if `byte_0x38_56&1` | EF:29434 |
| class-3 spin | pitch = 512 | model-0 only | EF:29421 |
| finisher restore countdown | 16 | `dword_0x10_16 = 16` | EF:28842 |
| finisher settle divisor | every 4th tick (`life & 3 == 0`) | | EF:28914 |
| final-snap gate | `dword_0x10_16 < 3` → `sub_439A0` | | EF:28952 |
| yaw units | 2048 / full turn (512 = quarter) | `& 0x7ff` mask, cos offset 0x200 | Player:10-18 |
| shading gradient | `H(NW=x-1,y-1) - H(SE=x+1,y+1) + 32` | phase-2 & finisher | EF:28631 / :28969 |
| shading bands | `>40 → (v&7)+40; <28 → (v&3)+28` | | EF:28633-28640 |
| shading night flip | `64 - v` (`32-v+32`) | MapType != Day | EF:28643 |
| center-2×2 drop | `h - h/countdown`, clamp[0,255] | crater floor | EF:28680 |
| center-2×2 shade | `(±31)/countdown + 32` | Day +31 / Night -31 | EF:28687-28689 |
| RNG law | `r = 9377*r + 9439` | | (shared) |

---

## OPEN

- **`<arith-shift-round>` in `sub_39B60`'s z-pull** (EF:29097-29099, `__CFSHL__((4096-v14)<<8 >>31, 12) +
  ((4096-v14)<<8 >>31 << 12)`) is IDA's expansion of a signed `>> 12` on a possibly-negative int; the
  net expression equals `((4096 - v14) << 8) >> 12` as a C **arithmetic** right shift (sign-preserving).
  For the port use `((4096 - v14) * 256) >> 12` with `>>` on a signed i32. Confirmed by structure, not by
  a golden trace — flag for a state-hash check when porting.
- **`sin_DB750` LUT exact size/scale** is inferred from usage (`[0x200 + angle]` = cosine, `angle & 0x7ff`
  mask, 16.16 output). Not dumped here — our port already has the sine LUT (EPOCH-baked); confirm index
  base 0x200 = quarter turn against it.
- **`x_WORD_DE350` arctan LUT contents** not dumped (256-entry `atan((i)/256)` in 2048/turn units,
  per `sub_72633_maybe_tan` indexing `[(smaller<<8)/larger]`). Our port's arctangent LUT is the
  equivalent; a direct value comparison was not run.
- **`word_0x2C_44` in `sub_39B60` (`v4 = a1x->word_0x2C_44`, EF:29055)** is the same "dome-top reference"
  set in phase 1 (`32*(h-80)`); confirmed by field, but its exact fixed-point relationship to victim.z
  (both appear to be world-z units) should be checked against a running shove to be certain the `<4096`
  and `≤96` thresholds land where expected.
- **`NewEvent_4A050` default `maxLife_0x4`** (whether 0 or copied) not re-verified here; the flood never
  reads its own maxLife so it does not affect flood behaviour, but note the fissure ctor sets it and the
  flood ctor does not.
