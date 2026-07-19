# CLASS-10 Model 9 — Apocalypse-Dome OPEN-item Closure (shake event, dist form, shading recompute, SPELLS.DAT authority)

Closes the four OPEN items left by `docs/traces/mc2-class10-m9-dome-geometry.md` (§OPEN 1-4) plus the
low-priority player-cast question (§OPEN 5). Every claim carries a `file:line` citation to
`/home/rain/projects/mgcarpet/reference/remc2/remc2/` (EF = `engine/EventsFunctions.cpp`,
EV = `engine/Events.cpp`, Maths = `utilities/Maths.cpp`, Terrain = `engine/Terrain.cpp`,
Basic = `engine/Basic.cpp`, Spells = `engine/Spells.cpp`, LevelInit = `engine/LevelInit.cpp`).
Trace date 2026-07-10. Read the parent geometry doc first; this doc only expands the black boxes it named.

**Headline corrections / closures:**

1. **`sub_6D8B0(id, 0x12, hits)` is NOT an earthquake/shake — it is a SPELL-EXPERIENCE accumulator.**
   It adds `hits` to `wizard.spellsExperience[0x12]` (= spell row 18's XP counter), gated by
   `!(setting_38545 & 4)` and requiring `id` to point at a wizard (`class==3 && model==0`). The name
   "earthquake event kind 0x12" in the parent doc is wrong: `0x12` is the **spell-slot index**, not an
   event opcode. The dome awards spell-18 XP proportional to how many entities its area damage hit.
   (EF:58228-58262.)

2. **`EuclideanDistXYZ_58490` is 2-D, not 3-D, in remc2.** Despite the name it computes
   `isqrt(dx² + dy²)` — the **z term is absent**. So the dome disc test never reads `v31x.z`; the
   `v31x.z` stack-garbage question is MOOT (the field is written by nobody and read by nobody). Exact
   integer sqrt via Newton iteration seeded from a bit-scan LUT. (Maths:738-742, 744-755.)

3. **`AddBuildingToTerrain_46570` / `sub_462A0` mapShading formula is byte-reproducible.**
   `mapShading[cell] = f(height[x-1,y-1] − height[x+1,y+1])` through a clamp ladder, negated for
   non-Day maps. mapAngle low-3-bits come from a 4-neighbour `building_F2CD0x` LUT lookup. Full spine
   below. (EF:31158-31205; Terrain:2005-2048.)

4. **The CD `DATA/SPELLS.DAT` WINS at runtime — it overwrites the baked `Spells.cpp` fallback in place.**
   `SPELLS_BEGIN_BUFFER_ptr` literally aliases `SPELLS_BEGIN_BUFFER_str` (Basic:244), and the loader
   `ReadFileAndDecompress(".../DATA/SPELLS.DAT", &SPELLS_BEGIN_BUFFER_ptr)` decompresses the file straight
   onto that memory (EF:42903-42908), UNLESS `OptionsSettingFlag_24 & 8` is set. `SetDefaultSpells_5C0A0`
   runs AFTER and only touches `isEnabled_1`/`fontType`/one row's `maxManaLimit` — it never rewrites
   `subSpellIndex_2` or `life_0x1A`. So retail plays the CD's `subSpellIndex` values (e.g. row 18
   `{400,800,1200}`), and the `Spells.cpp` baked table `{120,240,480}` is used ONLY as the pre-load
   default / when the DAT is suppressed.

5. **The `LevelInit` patch of rows 4 & 19 is keyed to MapType (Day vs. non-Day), NOT difficulty.**
   (LevelInit:11-21.) Correcting the parent doc's "per difficulty" wording.

---

## 1. `sub_6D8B0` — spell-XP accumulator (OPEN item 1), verbatim (EF:58228-58262)

```c
void sub_6D8B0(unsigned __int16 a1, unsigned __int16 a2, __int16 a3)   // a1=entity id, a2=spell slot, a3=amount
{
    if (!(x_D41A0_BYTEARRAY_4_struct.setting_38545 & 4))            // GATE: only when flag-4 clear
    {
        if (a1)                                                     // id 0 = "no owner" -> no-op
        {
            v3x = Entities_EA3E4[a1];
            if (v3x->class_0x3F_63 == 3 && !v3x->model_0x40_64)     // owner must be a WIZARD (class 3, model 0)
            {
                v5 = v3x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x.at(a2);
                v3x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x.at(a2) = a3 + v5;   // XP += amount
                if (a2 == 2)                                        // spell 2 special: re-arm SetSpell
                    SetSpell_6D5E0(Entities_EA3E4[...SpellEnabled[2]], ...SpellIndex[2]);
                if (setting_byte1_22 & MULTIPLAYER_MODE) {          // MP: only the local player
                    if (a1 == D41A0_0.array_0x2BDE[...].playerIndex_0x00a_2BE4_11240)
                        sub_6DAD0(&v3x->...str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2);
                } else {
                    sub_6D9C0(&v3x->...str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2, 0, 1);       // SP: recompute spell level
                }
            }
        }
    }
}
```

- **`spellsExperience_0x2CB_715x`** is a `std::array<int32_t, NUMBER_OF_SPELLS>` — one XP dword per spell
  row, 26×4 = 104 bytes (global_types.h:176). So `a2 = 0x12 = 18` addresses **spell row 18's** XP.
- **The dome caller** (EF:23392-23395): `v39 = sub_116A0(a1x, 0, subSpell)` returns the area-damage HIT
  COUNT; `if (v39) sub_6D8B0(a1x->id_0x1A_26, 0x12, v39)` credits that many XP points to spell 18 for the
  wizard whose id the dome carries (`id_0x1A_26` was copied from the caster). This is a **feedback/XP**
  pulse, not a physics shake. There is **no camera shake and no knockback** in this path.
- **`setting_38545 & 4`** (the gate) is a game-mode/rules flag set in the menus
  (MenusAndIntros:1013, 3348, 3372 all `|= 4u`) and cleared as part of the `&= 0x43` mask at
  EF:32291. When bit-2 is set the whole XP-credit is suppressed (matches the parent doc's note that the
  ENDGAME apocalypse variant never even reaches here, because `sub_116A0` is skipped when
  `byte_0x36E03` is set → `v39` stays 0 → no `sub_6D8B0` call at all, EF:23392-23394).
- **The consumer of the XP** is `sub_6D9C0` / `sub_6DAD0` (SP / MP), invoked right here in the same
  function — it re-derives the caster's spell level/enablement from the new XP against
  `SPELLS_BEGIN_BUFFER_str[a2]` (the SPELLS table row). There is no separately-queued "earthquake event";
  the effect is entirely spell-progression bookkeeping and is consumed synchronously.

**Other `sub_6D8B0` call sites confirm the interpretation** — every caller passes a *spell-slot* number as
`a2`: `0x13`, `0x18`, `0x12`, `0x10`, `0x11`, `9`, `0x15`, `7`, `0x16`, `0x14`, `0xF`, … and a small hit
count as `a3` (EF:10861, 10998, 23395, 23521, 23525, 23871, 24407, 24802, 26636, 29374, 29580, 55273,
56243, …). These are the per-spell "you used spell N and it connected" XP awards scattered across all the
spell-effect state machines.

---

## 2. `EuclideanDistXYZ_58490` — the dome disc test is 2-D integer sqrt (OPEN item 3)

### 2.1 Verbatim (Maths:738-755)
```c
unsigned int Maths::EuclideanDistXYZ_58490(axis_3d* a1, axis_3d* a2)   // "XYZ" but z is UNUSED
{
    uint32_t radix =   ((int16_t)(a2->x - a1->x)) * ((int16_t)(a2->x - a1->x))
                     + ((int16_t)(a2->y - a1->y)) * ((int16_t)(a2->y - a1->y));   // dx² + dy²  (NO dz²)
    return Maths::sub_7277A_radix_3d(radix);
}

unsigned int Maths::sub_7277A_radix_3d(unsigned int a1)                // integer sqrt(a1)
{
    if (!a1) return 0;
    x_BitScanReverse(&v1, a1);                                          // v1 = index of highest set bit
    for (i = x_WORD_727B0[v1]; (signed int)(a1 / i) < (signed int)i; i = (a1 / i + i) >> 1)
        ;                                                              // Newton/Heron iteration
    return i;                                                          // floor-ish sqrt
}
```

- **The deltas are truncated to `int16_t` before squaring.** Inputs are in `<<8` world units. Products
  accumulate in a `uint32_t`.
- **`sub_7277A_radix_3d` is a Heron (Newton) integer square-root.** It seeds from
  `x_WORD_727B0[bit_index]` — a power-of-√2 seed table — then iterates `i ← (a1/i + i)/2` until
  `a1/i ≥ i`. Result is the floor integer sqrt of `dx²+dy²`.
- **Seed LUT `x_WORD_727B0`** (Maths:647-676): the FIRST 32 entries are the valid seeds
  `round(2^(n/2))` for n = 0..31:
  `{1,2,2,4,5,8,0xB,0x10,0x16,0x20,0x2D,0x40,0x5A,0x80,0xB5,0x100,0x16A,0x200,0x2D4,0x400,0x5A8,0x800,0xB50,0x1000,0x16A0,0x2000,0x2D41,0x4000,0x5A82,0x8000,0xB504,0xFFFF}`.
  Entries from index 32 onward (`0x8B55, 0x50EC, …`) are disassembled *code bytes* that bled into the
  array in the decompile — but they are never indexed: the dome's radix `dx²+dy²` is bounded (see 2.3),
  so `x_BitScanReverse` returns an index ≤ 24, well inside the valid 0..31 range. **Not a bug for the
  dome.**

### 2.2 The `v31x.z` question is moot (EF:23349-23351)
```c
v31x.x = v9x._axis_2d.x << 8;      // cell x, <<8 units
v31x.y = v9x._axis_2d.y << 8;      // cell y, <<8 units
v11 = Maths::EuclideanDistXYZ_58490(&a1x->position_0x4C_76, &v31x);
```
`v31x` is `axis_3d v31x; // [esp+0h] [ebp-3Ch]` (stack local, EF:23225). Only `.x` and `.y` are written;
`.z` is left as stack garbage. Because `EuclideanDistXYZ_58490` reads **only** `->x` and `->y`, the garbage
`.z` is never touched — the disc test is a pure horizontal (2-D) distance. So the parent doc's worry that
"the dome center z rises, tightening late-tick discs" does **not** apply: **z never enters the disc test.**
(This may be a remc2 fidelity fix vs. the original x86, which the decompile comment `//239490` cannot
confirm either way — see OPEN.)

### 2.3 Range bound (why the seed LUT never overflows)
Dome radius `R = maxLife|1 ≤ 12` tiles (endgame maxLife=11 → R=12). Max delta ≈ `R<<8 = 3072`, so
`dx²+dy² ≤ 2·3072² ≈ 1.89e7 < 2²⁵`. `x_BitScanReverse` ⇒ index ≤ 24. Only seeds `x_WORD_727B0[0..24]`
are ever read; all are valid.

---

## 3. mapShading / mapAngle recompute — `AddBuildingToTerrain_46570` (a4=0 path) & `sub_462A0` (a4=1) (OPEN item 2)

`sub_570F0` (the height writer) calls **`AddBuildingToTerrain_46570(cell,cell)` when a4==0**
(the dome path) and **`sub_462A0(cell,cell)` when a4==1** (EF:39592-39595, 39704-39707). Both take
`(axis1, axis2)` = inclusive tile box corners; the dome passes the **same** cell for both, so the box is
1×1 and the loops below all run for the single written cell plus its immediate neighbours.

### 3.1 `AddBuildingToTerrain_46570` — three passes (EF:31080-31206, verbatim spine)

**Pass A — seed mapTerrainType (EF:31096-31115):** for each box cell, force `mapTerrainType=1` on the cell
and 3 of its neighbours (the 2×2 quad `(x,y),(x-1,y),(x-1,y-1),(x,y-1)`). (Unconditional here — this is
the ONLY behavioural difference from `sub_462A0`, which gates it on `mapAngle>=0`.)

**Pass B — mapTerrainType + mapAngle low-nibble from the `building_F2CD0x` LUT (EF:31123-31157):**
```c
for each cell in the (xAdd+1) x (yAdd+1) expanded box, if mapTerrainType[cell]==1:
    // 4-neighbour angle key (each & 7), packed base-7:
    buildingIndex = 343*(angle[x,  y  ]&7)     // this cell
                  +  49*(angle[x+1,y  ]&7)      // +x
                  +   7*(angle[x+1,y+1]&7)      // +x+y
                  +     (angle[x,  y+1]&7);     // +y
    mapTerrainType[cell] = building_F2CD0x[buildingIndex][0];
    if (mapTerrainType[cell] >= 8)
        mapAngle[cell] = building_F2CD0x[buildingIndex][1] + (mapAngle[cell] & 0x87);   // keep bits 7,2,1,0
    else {
        rand2_17B4E0 = 9377*rand2_17B4E0 + 9439;                                        // LCG
        mapAngle[cell] = (mapAngle[cell] & 0x87) + 16*(rand2_17B4E0 % 7);               // random hi-nibble
    }
```

**Pass C — mapShading from neighbour height DELTA (EF:31162-31205, verbatim):**
```c
for each cell in the (xAdd+2) x (yAdd+2) box:
    char zTemp = -mapHeightmap[x+1,y+1] + 32;      // start: 32 - height(SE)
    zTemp +=      mapHeightmap[x-1,y-1];            // + height(NW)      => zTemp = NW - SE + 32
    if (zTemp >= 28) { if (zTemp > 40) zTemp = (zTemp & 7) + 40; }   // clamp high band -> [40..47]
    else            { zTemp = (zTemp & 3) + 28; }                    // clamp low  band -> [28..31]
    if (MapType != Day) mapShading[cell] = 32 - zTemp + 32;          // = 64 - zTemp  (invert for Night/Cave)
    else                mapShading[cell] = zTemp;                    // Day: direct
    // cave floor/ceiling seal:
    if (isCave && second_heightmap[cell] <= mapHeightmap[cell]) {
        second_heightmap[cell] = mapHeightmap[cell] - 1;  mapAngle[cell] |= 8;   // sealed
    } else {
        mapAngle[cell] &= 0xF7;                                                  // open (clear bit3)
    }
```

### 3.2 `sub_462A0` — identical shading, gated terrain-type seed (Terrain:1931-2049, verbatim of the load-bearing block)
Pass A gates the `mapTerrainType=1` seed on `(int8_t)mapAngle >= 0` (i.e. high-bit/lock clear)
(Terrain:1949-1961). Pass B is byte-identical to `AddBuildingToTerrain`'s Pass B
(Terrain:1975-1996 — same `343/49/7/1` base-7 key over the same 4 neighbours, same
`building_F2CD0x[k][0]`/`[1]` writes, same `& 0x87` mask, same LCG `9377*x+9439` and `%7`).
Pass C is byte-identical to Pass C above (Terrain:2005-2048 — same `NW - SE + 32`, same `>=28 / >40`
clamp ladder, same `64 - zTemp` Night inversion, same cave seal).

**Net: the ONLY difference between the two is Pass A's guard** (`AddBuildingToTerrain` seeds
`mapTerrainType=1` unconditionally; `sub_462A0` only on unlocked cells). For a port targeting **byte-exact
`mapShading` goldens**, Pass C is the whole story and is the same in both:

```
mapShading[x,y] = shade( height[x-1,y-1] - height[x+1,y+1] )
where shade(d):
    z = d + 32
    if z >= 28: if z > 40: z = (z & 7) + 40
    else:               z = (z & 3) + 28
    return (MapType == Day) ? z : (64 - z)
```

**Caveat for goldens:** Pass B mutates `mapAngle`'s HIGH nibble via `rand2_17B4E0` (an LCG) whenever the
looked-up `building_F2CD0x[k][0] < 8`. To reproduce `mapAngle` byte-exact you must also model `rand2_17B4E0`
(same LCG constants `9377`, `9439`) and its call ordering. `mapShading` itself (Pass C) is deterministic
and RNG-free. `building_F2CD0x` is a `[?][2]` byte LUT (the "building/terrain-transition" table) that a port
must ship verbatim; its contents were not dumped in this pass (see OPEN).

---

## 4. SPELLS.DAT authority — the CD file wins (OPEN item 4)

### 4.1 The buffer aliases the baked table (Basic:244, 334)
```c
uint8_t* SPELLS_BEGIN_BUFFER_ptr = (uint8_t*)SPELLS_BEGIN_BUFFER_str;                 // Basic:244
Pathstruct xadataspellsdatx = { "DATA/SPELLS.DAT\0", &SPELLS_BEGIN_BUFFER_ptr, NULL, 0, 0 };  // Basic:334
```
`Pathstruct.colorPalette_var28` is the 2nd field (`uint8_t**`, global_types.h:397). So
`xadataspellsdatx.colorPalette_var28 == &SPELLS_BEGIN_BUFFER_ptr`, and
`*colorPalette_var28 == SPELLS_BEGIN_BUFFER_ptr == (uint8_t*)SPELLS_BEGIN_BUFFER_str`. **The DAT target
memory IS the baked table.**

### 4.2 The loader runs at init and decompresses the file onto that memory (EF:42903-42908, verbatim)
```c
if (!(x_D41A0_BYTEARRAY_4_struct.OptionsSettingFlag_24 & 8))       // unless option-8 suppresses it
{
    char spellDataPath[MAX_PATH];
    sprintf(spellDataPath, "%s/%s", cdDataPath.c_str(), "DATA/SPELLS.DAT");
    DataFileIO::ReadFileAndDecompress(spellDataPath, xadataspellsdatx.colorPalette_var28);   // -> overwrites SPELLS_BEGIN_BUFFER_str
}
SetDefaultSpells_5C0A0();                                          // EF:42911, runs AFTER the load
```
- `ReadFileAndDecompress(path, out)` reads+decompresses the file into `*out` (same call signature used for
  every other CD asset, e.g. Level:883-903). Here `*out == SPELLS_BEGIN_BUFFER_str`, so the 2080-byte
  (26×80) file lands directly on the baked table.
- **Ordering:** load (42907) → `SetDefaultSpells_5C0A0` (42911). The default-setter runs on the ALREADY-
  loaded buffer.

### 4.3 `SetDefaultSpells_5C0A0` does NOT clobber subSpellIndex/life (Spells:110-153, verbatim of the writes)
```c
for (i = 0; i < 26; i++) {
    SPELLS_BEGIN_BUFFER_str[i].isEnabled_1 = 0;                       // flags only
    SPELLS_BEGIN_BUFFER_str[i].subspell[0..2].fontType_0x1B &= 0xFE;  // flag bit only
    switch (i) { case 0: subspell[1].fontType |= 1; ...
                 case 3/4/6/8/11/12/14: isEnabled_1 |= 4; ...
                 case 7: subspell[0].fontType |= 1; ...
                 case 23: subspell[0/1/2].maxManaLimit_A = 50000/70000/90000; }   // ONLY row 23 mana
    // then derive isEnabled_1 bits 3/4/5 from maxManaLimit_A / manaCost_6
}
```
It writes **only** `isEnabled_1`, `fontType_0x1B`, and (row 23 only) `maxManaLimit_A`. It **never** writes
`subSpellIndex_2` or `life_0x1A`. Therefore **the CD DAT's `subSpellIndex`/`life` values survive untouched
into gameplay.** ⇒ Retail plays the CD values; the `Spells.cpp` baked numbers are the pre-load default
(and the value used only if `OptionsSettingFlag_24 & 8` suppresses the load).

### 4.4 Confirming the divergent rows (baked fallback, for the importer's diff)
`Spells.cpp` is `SPELLS_BEGIN_BUFFER_str[26]`, each row = `{byte_0, isEnabled_1, {sub0,sub1,sub2}}`, each
sub = `{subSpellIndex, manaCost, maxManaLimit, xpos1, xpos2, hintText, word_0x18, life, fontType}`.
Row r's three subspells are on source lines `4+4r … 6+4r`. Baked values:

| row | line | baked subSpellIndex {t0,t1,t2} | baked life {t0,t1,t2} | CD DAT subSpellIndex (per prompt) |
|---|---|---|---|---|
| 16 | Spells.cpp:68-70 | {0x32=**50**, 0x64=**100**, 0x12C=**300**} | {0x06=6, 0x0C=12, 0x18=24} | {**250,400,900**} |
| 17 | Spells.cpp:72-74 | {0x50=**80**, 0x78=**120**, 0x140=**320**} | {0x10=16, 0x20=32, 0x40=64} | {**300,500,1000**} |
| 18 | Spells.cpp:76-78 | {0x78=**120**, 0xF0=**240**, 0x1E0=**480**} | {0x07=7, 0x09=9, 0x0B=11} | {**400,800,1200**} |

The baked column matches the parent doc's §3.4 table. The `life` values match the CD (per prompt), so only
`subSpellIndex` diverges — and per §4.2-4.3 the **CD values are authoritative at runtime**. Our importer
must ship the CD DAT numbers (400/800/1200 for row 18, etc.), NOT the `Spells.cpp` fallback, for
subSpellIndex parity. (Note: for the (10,9) dome specifically, `subSpellIndex` feeds `sub_116A0` area
damage — so the CD's bigger row-18 numbers mean a level-authored dome does MORE area damage than the baked
fallback would suggest; the endgame apocalypse variant suppresses damage regardless.)

### 4.5 The `LevelInit` rows-4-&-19 patch — MapType-keyed, not difficulty-keyed (LevelInit:11-21, verbatim)
```c
isCaveLevel_D41B6 = 0;
SPELLS_BEGIN_BUFFER_str[4].subspell[0].life_0x1A     = 19;    // default (non-Day)
SPELLS_BEGIN_BUFFER_str[4].subspell[0].hintText_0x16x = 199;
SPELLS_BEGIN_BUFFER_str[19].subspell[0].life_0x1A     = 19;
SPELLS_BEGIN_BUFFER_str[19].subspell[0].hintText_0x16x = 245;
if (levelData->MapType == MapType_t::Day) {                   // Day override
    SPELLS_BEGIN_BUFFER_str[4].subspell[0].life_0x1A     = 2;
    SPELLS_BEGIN_BUFFER_str[4].subspell[0].hintText_0x16x = 198;
    SPELLS_BEGIN_BUFFER_str[19].subspell[0].life_0x1A     = 2;
    SPELLS_BEGIN_BUFFER_str[19].subspell[0].hintText_0x16x = 244;
}
```
Exact patched values, **fields `subspell[0].life_0x1A` and `subspell[0].hintText_0x16x` of rows 4 and 19
ONLY (tier 0)**:

| map | row 4 life / hint | row 19 life / hint |
|---|---|---|
| non-Day (Night/Cave) | 19 / 199 | 19 / 245 |
| Day | 2 / 198 | 2 / 244 |

This runs at every `LevelInit_56C00`, i.e. it re-patches per level after the DAT is loaded, keyed on the
level's `MapType`, **not** on a difficulty setting. (Corrects the parent doc's §3.3 "per difficulty"
phrasing. No difficulty branch touches these rows in `LevelInit`.)

---

## 5. Player-cast entry for spell 18 (OPEN item 5) — partial

- **Row 18 is present in the player spellbook index map.** `spellIndex_D94FF[29]` (GameUI.cpp:59) is the
  identity-ish slot→row table `{0,1,…,25,0,3,0}`; slot 18 → row 18. Player input reads spells through this
  table (PlayerInput.cpp:847, 1876, 1918, 2214, 2275). So spell 18 is addressable by the normal
  spellbook/cast UI — it is **not** structurally excluded from player casting.
- **Whether the player can LEARN/enable row 18** depends on `isEnabled_1` bits (set by
  `SetDefaultSpells_5C0A0` + the CD DAT's per-row `isEnabled` byte) and the level's spell grants; row 18 is
  NOT in the `SetDefaultSpells` case-list that force-sets `isEnabled_1 |= 4` (rows 3,4,6,8,11,12,14 only,
  Spells:123-131), so its enablement comes from the mana-limit derivation (Spells:141-151) and/or level
  grants — not force-enabled by default.
- **The cast → (10,9) materialization** goes through the class-15 spell-token path
  (`IfSubtypeCallCreatingManaSphere_4A190(&pos, 15, spellIndexRow)`, Level.cpp:1313) which then dispatches
  to the class-10 effect entity. That token→effect dispatch is the subject of
  `docs/traces/mc2-class15-spell-tokens.md` and was NOT re-traced here.

**Verdict (bounded):** spell 18 is *reachable* by the player-cast machinery (it's in `spellIndex_D94FF`
and routed through the class-15 token like every other castable spell), so if a level grants/enables it the
player can cast the raise-land/crater. The exact enable-gating byte and the token→(10,9) subtype mapping
are out of scope here.

---

## 6. Consolidated constants / formula table (this pass)

| item | finding | source |
|---|---|---|
| `sub_6D8B0(id,a2,a3)` semantics | `wizard.spellsExperience[a2] += a3` (spell-XP), not a shake | EF:58243-58245 |
| `sub_6D8B0` owner gate | `class==3 && model==0` (wizard) | EF:58240 |
| `sub_6D8B0` mode gate | skip whole body if `setting_38545 & 4` | EF:58235 |
| `sub_6D8B0` consumer | `sub_6D9C0`(SP)/`sub_6DAD0`(MP) recompute spell level from new XP | EF:58252, 58257 |
| dome caller a2/a3 | `a2 = 0x12` (spell row 18), `a3 = sub_116A0 hit count` | EF:23393-23395 |
| `spellsExperience` array | `std::array<int32_t,26>` (one XP dword per spell) | global_types.h:176 |
| `EuclideanDistXYZ_58490` form | `isqrt(dx² + dy²)` — **2-D, z ignored** | Maths:740-741 |
| delta typing | deltas cast to `int16_t` before squaring | Maths:740 |
| integer sqrt | Heron/Newton, seed `x_WORD_727B0[bsr(radix)]` | Maths:744-755 |
| sqrt seed LUT valid range | first 32 entries = `round(2^(n/2))`; rest is code-bleed, never indexed for dome | Maths:649-652 |
| `v31x.z` | never written meaningfully, never read → moot | EF:23349-23351 |
| shading formula | `shade(height[x-1,y-1] − height[x+1,y+1])`, `z=d+32`, clamp `[28..31]`/`[40..47]`, Night ⇒ `64−z` | EF:31170-31188; Terrain:2014-2033 |
| mapShading Day vs Night | Day: `z`; non-Day: `64 − z` | EF:31185-31188 |
| mapAngle low-nibble | `building_F2CD0x[k][*]` via base-7 key `343·a0+49·a1+7·a2+a3` of 4 neighbours (&7) | EF:31133-31145; Terrain:1985-1996 |
| mapAngle hi-nibble (when LUT<8) | `16·(rand2 % 7)`, `rand2 = 9377·rand2 + 9439` | EF:31148-31149; Terrain:1993-1994 |
| `AddBuildingToTerrain` vs `sub_462A0` | identical except Pass-A seed guard (`sub_462A0` gates on `mapAngle>=0`) | EF:31101-31107 vs Terrain:1949-1959 |
| SPELLS buffer alias | `SPELLS_BEGIN_BUFFER_ptr == (uint8_t*)SPELLS_BEGIN_BUFFER_str` | Basic:244 |
| SPELLS.DAT loader | `ReadFileAndDecompress(".../DATA/SPELLS.DAT", &ptr)`, skip if `OptionsSettingFlag_24 & 8` | EF:42903-42908 |
| load ordering | DAT load → `SetDefaultSpells_5C0A0` (flags only) | EF:42907, 42911 |
| `SetDefaultSpells` writes | `isEnabled_1`, `fontType`, row-23 `maxManaLimit` — NOT subSpellIndex/life | Spells:114-151 |
| authority | **CD DAT subSpellIndex/life WIN at runtime** | §4.1-4.3 |
| baked row 18 subSpellIndex | {120,240,480} (fallback only) | Spells.cpp:76-78 |
| baked row 16/17 subSpellIndex | {50,100,300} / {80,120,320} (fallback only) | Spells.cpp:68-70 / 72-74 |
| LevelInit rows 4&19 patch | tier-0 `life`+`hintText`; MapType-keyed (non-Day 19/199,19/245; Day 2/198,2/244) | LevelInit:11-21 |
| row 18 player-castable | in `spellIndex_D94FF`; routed via class-15 token; enable-gating out of scope | GameUI.cpp:59; Level.cpp:1313 |

---

## OPEN items (remaining after this pass)

1. **`building_F2CD0x` LUT contents.** Pass B of the shading recompute indexes a `[?][2]`-byte
   building/terrain-transition table (`building_F2CD0x[key][0..1]`) to set `mapTerrainType` and the
   `mapAngle` low-nibble. The formula and access pattern are fully traced (§3), but the LUT *bytes* were
   not dumped here. A byte-exact `mapAngle` port must ship this table verbatim (find its definition/loader
   — it is one of the `DATA/*.DAT`/`BUILD0-0.DAT`-family assets; `CreateIndexes_6EB90(BUILD00DATTAB)` at
   EF:42892 is the likely builder). `mapShading` (Pass C) is fully closed and RNG-free.

2. **`rand2_17B4E0` call-ordering for `mapAngle` goldens.** `mapShading` is deterministic, but the
   `mapAngle` hi-nibble uses the `rand2_17B4E0` LCG (only when `building_F2CD0x[k][0] < 8`). Reproducing
   `mapAngle` byte-exact requires modelling that LCG's global state and exact call sequence across the
   whole frame, not just the dome. Deferred unless we pin `mapAngle` goldens (we currently do not for the
   dome; `mapHeightmap`/`mapShading` are the load-bearing outputs).

3. **Original-x86 z-term in the distance test.** remc2's `EuclideanDistXYZ_58490` is 2-D. Whether the
   retail x86 binary's `sub_239490` actually summed a `dz²` term (making the name literal) cannot be
   settled from the decompile alone (the `//239490` address comment is not disassembly). If a recorded-
   gameplay disc edge ever disagrees with a 2-D model on sloped terrain, revisit — but for parity with
   remc2 (our senior source for structure) the 2-D form is what ships.

4. **`ReadFileAndDecompress` failure fallback.** §4 confirms the DAT overwrites the baked table when the
   file is present and `OptionsSettingFlag_24 & 8` is clear. The return value of `ReadFileAndDecompress`
   is ignored at EF:42907, so behaviour when `DATA/SPELLS.DAT` is ABSENT (does the buffer keep the baked
   `Spells.cpp` values, or get zeroed?) was not traced. For our importer this is moot (we always ship the
   CD DAT), but note it if we ever run without the file.

5. **Spell-18 enable-gating + token→(10,9) subtype.** §5 bounds the player-cast question (reachable via
   `spellIndex_D94FF` + class-15 token) but does not pin the exact `isEnabled_1` requirement to cast row 18
   nor the class-15-token→class-10-model-9 subtype dispatch (that lives in
   `docs/traces/mc2-class15-spell-tokens.md`).
