# MC2 CASTLE DATA TABLES — port-ready verbatim closure of the four OPEN table items

Closes the data-table OPENs left by `docs/traces/mc2-castle-builder.md` (§9) and
`docs/traces/mc2-castle-runtime.md` (§OPEN): the `x_BYTE_DB038` stage-piece offset table, the
`array_0x24E_590` per-level part/HP-factor table + `word_0x24A_586` Life source, the authored
starting-castle-level header field `player_0x2FED9`, and the `BUILD0-0` file layout + row count.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
EF = `engine/EventsFunctions.cpp`, Basic = `engine/Basic.cpp`. House style per
`docs/traces/mc2-class10-m67-flood-helpers.md` (verbatim C, renamed-field comments, uint8-wrap,
RNG law `r = 9377*r + 9439`). Trace date 2026-07-11. Read the two castle docs first.

---

## Headline findings (read first)

1. **`x_BYTE_DB038` is a STATIC ARRAY in the decompile source** (EF:2594, `int8_t x_BYTE_DB038[52]`),
   NOT loaded from any retail file. It is FULLY DUMPED and DECODED below (§1) — the castle stage-piece
   (10,79) tile offsets for all 8 levels. The decode is self-consistent with the BUILD0-0 footprint
   dims (level 1 → 8×8 with 1 centred piece; L6/L7 → 48×48 with 8 pieces). **Port this verbatim.**

2. **`array_0x24E_590` is a 19-entry `int8_t` array on the WIZARD player struct (`dword_0xA4_164`),
   POPULATED AT RUNTIME from SPELLS.DAT** — NOT from `wizards.json` or personality constants. The single
   write site is `sub_69AB0` (EF:56120-56121), the per-level castle-research completion handler: it writes
   `array_0x24E_590[level] = SPELLS[model].subspell[byte_0x46_70].subSpellIndex_2` (the **part-type** used
   at `[9+level]` by `sub_613D0`) and `array_0x24E_590[level+9] = SPELLS[model].subspell[byte_0x46_70].life_0x1A`
   … **wait — the indices are crossed vs the two reader docs; see §2 for the exact correction.**

3. **`word_0x24A_586` (Life personality) = a 16.8 fixed-point scalar, DEFAULT 256 (= 1.0×)** set at wizard
   spawn (EF:43720), OVERRIDDEN for AI wizards from the map header `WizardMapSettings_0x360D2[color].Life_0x3612F`
   (EF:43768-43771). So the castle HP Life-scale is `256` (identity) for the human player and the authored
   per-wizard Life value for rivals. Its file source is the level header (§2.3).

4. **`player_0x2FED9[8]` is a level-header field at struct offset 0x2FED9 (17 bytes into the
   `terrain_2FECE` block), one signed byte per wizard color 0..7 = the authored starting-castle LEVEL.**
   Read at level load (EF:43777, EF:43789). File source: the compressed level file's `array_0x2FED9[8]`,
   copied by `DecompressLevel_2FECE` (ConvertMapInfo.cpp:10). §3.

5. **`BUILD0-0` = `DATA/BUILD0-0.DAT` (cells) + `DATA/BUILD0-0.TAB` (6-byte index rows)** inside the MC2
   `game.gog` CD image (Basic.cpp:271/273). The project already bakes it: **`baked/assets/mc2-day/build.tab.bin`
   has 77 rows** (462 bytes / 6). Row layout = `bitmap_pos_struct2_t {u32 offset, u8 width, u8 height}`;
   `CreateIndexes_6EB90` resolves it into `posistruct[i] {u8* data, u8 width_4, u8 height_5}`. Verified row
   count and dims from the baked file (§4). The castle build levels are rows 1..7 (8×8, 21×21, 21×21,
   35×35, 35×35, 48×48, 48×48).

---

## 1. `x_BYTE_DB038` — the (10,79) stage-piece offset table (EF:2594) — VERBATIM + DECODED

### 1.1 The definition (STATIC ARRAY, verbatim)

```c
// EF:2594
int8_t x_BYTE_DB038[52] = {
0x00,0x00,0x01,0x00,0x04,0x01,0x04,0x01,0x04,0x05,0x04,0x05,0x08,0x09,0x08,0x09,0x00,0x00,0x04,0x04,0x03,0x03,
0x11,0x03,0x03,0x11,0x11,0x11,0x03,0x03,0x1F,0x03,0x03,0x1F,0x1F,0x1F,0x03,0x03,0x18,0x03,0x2D,0x03,0x03,0x18,
0x2D,0x18,0x03,0x2D,0x18,0x2D,0x2D,0x2D
};
```

It is a plain compile-time constant in the decompiled source (the reference symbol comment `x_BYTE_DB038 - ok`
appears at EF:93). **Not loaded from any DAT/TAB — carry the 52 bytes verbatim.**

### 1.2 The consumer `sub_613D0` (EF:62293-62295) — VERBATIM

```c
// EF:62293  (v4 = the resolved level, see §2; v20/v19 = footprint origin = centerTile - (dim>>1))
v14 = (unsigned __int8)x_BYTE_DB038[2 * v4];                                   // part COUNT for level v4
v13 = (char*)&x_BYTE_DB038[18] + 2 * (unsigned __int8)x_BYTE_DB038[1 + 2 * v4];// OFFSET-LIST base for level v4
// ... loop v15 in [0, v14): one (10,79) piece per part ...
//     predictedAxis.x = (v20 + v13[0]) << 8;   predictedAxis.y = (v19 + v13[1]) << 8;   v13 += 2;
```

**Index math (verbatim):**
- **count** at `x_BYTE_DB038[2 * level]`, read as `unsigned __int8`.
- **offset-list index** at `x_BYTE_DB038[1 + 2 * level]`, read as `unsigned __int8`. This selects a slot in
  the pairs region.
- **pairs region base** = `&x_BYTE_DB038[18]`; the list for a level starts at `base + 2 * listindex`, and
  each part is a 2-byte `(x, y)` **tile offset** consumed as `v13[0]`, `v13[1]` (signed `char` reads, but
  all values here are in [0..0x2D] so unsigned == signed).
- Per part: world position `((origin.x + off.x) << 8, (origin.y + off.y) << 8)` where
  `origin = centerTile - (footprint_dim >> 1)` (footprint from `posistruct[level]`, §4). So the offsets are
  **tile coordinates measured from the top-left corner of the castle footprint.**

### 1.3 DECODED per-level piece list (level → count → tile offsets from footprint NW corner)

Bytes 0..15 = the 8 `(count, listindex)` pairs for levels 0..7; byte 16..17 = `(0x00,0x00)` = the first
`(x,y)` pair slot; bytes 18..51 = the pairs region (17 `(x,y)` pairs). Decoded (all offsets are tile units):

| level | count | listindex | piece tile offsets `(x,y)` | footprint (from BUILD0-0) |
|---|---|---|---|---|
| 0 | 0 | 0 | — (no pieces) | (row 0, 0×0) |
| 1 | 1 | 0 | `(4,4)` | 8×8 → 1 piece centred |
| 2 | 4 | 1 | `(3,3) (17,3) (3,17) (17,17)` | 21×21 → 4 corners |
| 3 | 4 | 1 | `(3,3) (17,3) (3,17) (17,17)` | 21×21 → 4 corners (same list as L2) |
| 4 | 4 | 5 | `(3,3) (31,3) (3,31) (31,31)` | 35×35 → 4 corners |
| 5 | 4 | 5 | `(3,3) (31,3) (3,31) (31,31)` | 35×35 → 4 corners (same list as L4) |
| 6 | 8 | 9 | `(3,3) (24,3) (45,3) (3,24) (45,24) (3,45) (24,45) (45,45)` | 48×48 → 8: 4 corners + 4 edge mids |
| 7 | 8 | 9 | `(3,3) (24,3) (45,3) (3,24) (45,24) (3,45) (24,45) (45,45)` | 48×48 → 8 (same list as L6) |

Pairs region raw (byte index 18 = pair slot 0):

```
slot  0: (0x00,0x00) = ( 0, 0)   <- listindex 0 uses this? NO: listindex 0 → base+0 = slot 0 = (4,4)?
```

**IMPORTANT indexing subtlety (verified by the decode):** the pairs base is `&x_BYTE_DB038[18]`, and
`listindex` is a **pair index** (multiplied by 2 to get a byte offset). Byte 16..17 (`0x00,0x00`) is the
`(0x00,0x00)` at the tail of the count/index block, NOT part of the pairs region. The pairs region proper is
bytes 18..51:

```
[18]=04 [19]=04 [20]=03 [21]=03 [22]=11 [23]=03 [24]=03 [25]=11 [26]=11 [27]=11
[28]=03 [29]=03 [30]=1F [31]=03 [32]=03 [33]=1F [34]=1F [35]=1F [36]=03 [37]=03
[38]=18 [39]=03 [40]=2D [41]=03 [42]=03 [43]=18 [44]=2D [45]=18 [46]=03 [47]=2D
[48]=18 [49]=2D [50]=2D [51]=2D
```

So pair slot 0 = `(0x04,0x04) = (4,4)` (level 1). Slot 1 = `(0x03,0x03) = (3,3)` … through slot 4 =
`(0x11,0x11) = (17,17)` (level 2/3's four corners). Slot 5 = `(3,3)`, slots 5..8 = level 4/5's four
`(±3,±31)` corners (0x1F = 31). Slots 9..16 = level 6/7's eight `(±3,±24,±45)` pieces (0x18=24, 0x2D=45).
**Confirmed:** `listindex` reads {L1:0, L2/3:1, L4/5:5, L6/7:9} → the pair-slot table exactly. Piece height
above ground = `v11 + 384` if `level <= 1` else `v11 + 224` (EF:62315, `v11 = getTerrainAlt`).

---

## 2. `array_0x24E_590` — per-level part-type + HP-factor table (populated from SPELLS.DAT)

### 2.1 Field definition

```c
// global_types.h:304  (on the wizard player struct dword_0xA4_164, at struct offset 0x24E)
std::array<int8_t, 19> array_0x24E_590; // size?? -> at least 12 in level 19. using the whole space of stubn now.
```

19 signed bytes at offset 0x24E on `dword_0xA4_164` (the per-player stat block
`array_0x2BDE[color].dword_0x3E6_2BE4_12228`; the game's own debug table names them
`array_0x2BDE[i].dword_0x3E6_2BE4_12228.array_0x24E_590[k]`, engine_support.cpp:710). Serialized 1 byte/entry
by `engine_support_converts.cpp:196` at output offset `0x24e + i`.

### 2.2 The SINGLE write site — `sub_69AB0` (EF:56086), the castle-research completion tick — VERBATIM

`sub_69AB0` (address 0x24aab0) is the per-tick handler of a research/production child entity that hangs off a
castle (`parentId_0x28_40` = the castle/wizard, `word_0x2E_46` counts down, `word_0x30_48` is its terminal
value). When `word_0x2E_46 == word_0x30_48` (research complete for a stage) it writes TWO table entries:

```c
// EF:56112
if (a1x->word_0x2E_46 == a1x->word_0x30_48)
{
    v2 = v1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58;      // v1x = the owner wizard (parent)
    v4 = 1;
    if (v2)
        v4 = Entities_EA3E4[v2]->dword_0x10_16 + 1;            // v4 = castle level + 1  (target stage)
    // EF:56120  part-type for stage v4  ←  SPELLS[model].subspell[byte_0x46_70].subSpellIndex_2
    v1x->dword_0xA4_164x->array_0x24E_590.at(v4)     = SPELLS_BEGIN_BUFFER_str[a1x->model_0x40_64].subspell[a1x->byte_0x46_70].subSpellIndex_2;
    // EF:56121  HP-factor for stage v4  ←  SPELLS[model].subspell[byte_0x46_70].life_0x1A
    v1x->dword_0xA4_164x->array_0x24E_590.at(v4 + 9) = SPELLS_BEGIN_BUFFER_str[a1x->model_0x40_64].subspell[a1x->byte_0x46_70].life_0x1A;
    ...
}
```

**Exact semantics:**
- `v4 = castleLevel + 1` (or `1` if no castle yet) = the STAGE this research grants.
- **`array_0x24E_590[v4] = SPELLS[model].subspell[row].subSpellIndex_2`** — call this the **`subSpellIndex_2`
  channel**.
- **`array_0x24E_590[v4 + 9] = SPELLS[model].subspell[row].life_0x1A`** — the **`life_0x1A` channel**.
- `model` = the research entity's `model_0x40_64`; `row` = its `byte_0x46_70` (which SPELLS subspell tier).

### 2.3 CORRECTION to the two castle reader docs (the `[9+level]` / `[level]` split)

The readers are in `sub_613D0` (piece builder) and `sub_60810` (HP ladder). Verbatim:

```c
// EF:62274  (sub_613D0, piece builder — v4 = level walked down from castle level)
v16 = v5x->dword_0xA4_164x->array_0x24E_590.at(9 + v4);        // PART-TYPE for the piece → i2x->byte_0x43_67
if (v5x->dword_0xA4_164x->array_0x24E_590.at(9 + v4)) break;   // stop at highest researched stage

// EF:61704  (sub_60810, HP ladder)
int number1 = (owner->word_0x24A_586 * ((owner->array_0x24E_590[locEvent->dword_0x10_16] << 8) + 256)) >> 8;
```

So the **actual channel mapping is:**
- **`array_0x24E_590[level]`** (the low 9 slots, indices 1..7 used) = the value written by EF:56120 =
  `SPELLS[model].subspell[row].subSpellIndex_2`. **This is the HP-scale FACTOR read by `sub_60810`** (EF:61704).
- **`array_0x24E_590[9 + level]`** (the high 9 slots) = the value written by EF:56121 =
  `SPELLS[model].subspell[row].life_0x1A`. **This is the PART-TYPE read by `sub_613D0`** (EF:62274) into the
  piece's `byte_0x43_67`, and its non-zero-ness is the "is this stage researched?" gate.

**This CORRECTS the builder-doc §3.2 comment "the per-level part table `array_0x24E_590[9+level]`" —
correct — but note the builder doc and runtime doc both listed `[9+v4]=part` and `[level]=HP-factor` as an
OPEN guess; it is now CONFIRMED, and additionally the writer proves the SOURCE:** the HP factor comes from
SPELLS' `subSpellIndex_2` field and the part-type from SPELLS' `life_0x1A` field, both keyed by the research
entity's `(model, byte_0x46_70)`. For the port: **both halves of `array_0x24E_590` are filled by the castle
research/production system from SPELLS.DAT, one stage at a time as research completes — they are NOT loaded
at level start.** A castle that has not completed research for stage L has `array_0x24E_590[L] == 0` (HP factor
contributes `(0<<8)+256 = 256` = 1.0×) and `array_0x24E_590[9+L] == 0` (no piece → `sub_613D0` walks down to
the highest researched stage).

**Port consequence:** for a freshly-cast or authored castle with no research children run yet, the whole array
is zero ⇒ HP factor 1.0× (identity, matches MC1 flat HP) and NO (10,79) pieces until research populates it.
The authored-load path (§1.2 of the builder doc) stamps terrain for N levels but does NOT populate
`array_0x24E_590` — so authored starting castles show terrain footprint but the piece chain / HP scaling only
fill in once the research entities (`sub_69AB0`) fire. **Flag: confirm whether authored castles ever get a
research pre-fill, or whether they legitimately start with factor=1.0 / no pieces** (OPEN below).

### 2.4 `word_0x24A_586` (Life personality) — the load site — VERBATIM

```c
// EF:43720  (wizard spawn — DEFAULT)
v2x->dword_0xA4_164x->word_0x24A_586 = 256;                   // 256 = 1.0x in 16.8 fixed point
v2x->maxMana_0x8C_140 = 1000;
v2x->maxLife_0x4 = 10000;

// EF:43768  (AI wizards only, IsAiPlayer_0x009_2BE4_11239 == 1)
v14 = D41A0_0.terrain_2FECE.WizardMapSettings_0x360D2[playerColorIndex].Life_0x3612F;   // authored Life
if (v14)
{
    v2x->dword_0xA4_164x->word_0x24A_586 = v14;                            // override Life scalar
    v2x->maxLife_0x4 = v2x->maxLife_0x4 * v2x->dword_0xA4_164x->word_0x24A_586 >> 8;  // scale wizard maxLife too
}
```

- **Default = 256** (identity). The HUMAN player keeps 256 unless… (only the AI branch overrides here — the
  human player's Life is always 256 at spawn in this code). So castle HP scaling by Life is an **AI/rival
  flavour**; the human's castle uses the flat HP ladder.
- **AI source = `WizardMapSettings_0x360D2[color].Life_0x3612F`** — the per-wizard Life stat in the MAP HEADER
  (`Type_WizardMapSettings_0x360D2`, BasicTerrain.h:20-34, at struct offset 0x3612F within the 110-byte
  per-wizard settings record; the same record carries Aggression_0x360D5, Reflexes_0x360D9,
  Perception_0x360DD, and the StartingSpells/BlockedSpells masks). Loaded per level via
  `WizardMapSettings_0x360D2[8]` in the level header (LevelStructs.h:318, copied by DecompressLevel
  ConvertMapInfo.cpp:37). It is a 16.8 fixed scalar (a value of 256 = 1.0×; larger = tougher).
- This is exactly the `word_0x24A_586` the runtime doc §2 calls "owner Life stat" — **CONFIRMED: it is the
  authored per-wizard Life from `WizardMapSettings_0x360D2[color].Life_0x3612F`, default 256.**

---

## 3. `player_0x2FED9` — authored starting-castle LEVEL per wizard color (level header) — VERBATIM

### 3.1 Field definition + file offset

```c
// BasicTerrain.h:77  (decompressed runtime level header, type Type_Level_2FECE, length 0x6604)
int8_t player_0x2FED9[8];      // one signed byte per wizard color 0..7

// context (BasicTerrain.h:69-78): the field sits at struct offset 0x2FED9, immediately after:
//   uint16 word_2FECE; uint16 levelID_2FED0; uint8 byte_0x2FED2; uint8 byte_0x2FED3;
//   MapType_t MapType (@0x2FED4); int16 word_0x2FED5; int16 word_0x2FED7;
//   int8 player_0x2FED9[8];   ← HERE (0x2FED9 = 17 bytes into the block)
```

Offset within the level block = **17 bytes** (`0x2FED9 - 0x2FEC8`? no — the block base symbol is
`str_2FECE`; the field's absolute label is 0x2FED9, and it is the 18th byte of the header, right after the two
`word_0x2FED5/7`). One `int8` per wizard color.

### 3.2 File source + copy

```c
// LevelStructs.h:291  (COMPRESSED on-disk level header, Type_CompressedLevel_2FECE)
int8_t array_0x2FED9[8];       // the raw file field

// ConvertMapInfo.cpp:10  (DecompressLevel_2FECE: file → runtime)
for (int i = 0; i < 8; i++) to->player_0x2FED9[i] = from->array_0x2FED9[i];
```

Round-tripped back to the compressed form by Basic.cpp:3107 (save) / read at Basic.cpp:3300.

### 3.3 Read site — level load (EF:43777, EF:43789) — VERBATIM

```c
// EF:43775  (wizard spawn tail; runs for BOTH human and AI when Create-Castle spell enabled)
if (v2x->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2])           // Create-Castle enabled
{
    if (D41A0_0.terrain_2FECE.player_0x2FED9[v2x->dword_0xA4_164x->playerColorIndex_0x38_56])  // authored level > 0
    {
        v16x = IfSubtypeCallCreatingManaSphere_4A190(&v2x->position_0x4C_76, 3, 2);    // spawn class-3 model-2 castle
        ...
        for (j = 0; ; j = v22 + 1) {
            v23 = D41A0_0.terrain_2FECE.player_0x2FED9[...playerColorIndex...];         // authored level (loop bound)
            if (v23 <= j) break;                                                       // stamp j = 0..(level-1)
            ...  Entities_EA3E4[0]->byte_0x46_70 = j;  sub_36FC0(...);  ...             // stamp BUILD00 row j
        }
        v39x->dword_0x10_16 = v23 - 1;                                                 // castle level = authored - 1
        ...
    }
}
```

**Exact semantics for wiring authored MC2 starting castles:**
- `player_0x2FED9[color]` = the authored castle **level (number of stages)** for that wizard color, read
  directly indexed by `playerColorIndex_0x38_56`. Value 0 = no starting castle.
- The load path spawns one `(3,2)` castle at the wizard's spawn position and STAMPS `player_0x2FED9[color]`
  BUILD00 footprints (rows 0..N-1) onto terrain, then sets `castle.dword_0x10_16 = player_0x2FED9[color] - 1`.
  (So an authored value of N ⇒ castle level N-1, N terrain passes — see builder-doc §1.2.)
- **Wiring note:** this is the ONLY consumer of `player_0x2FED9`. To honor authored starting castles, the
  project's MC2 level-load must read this 8-byte header field and drive the same spawn+stamp+level-set.

---

## 4. `BUILD0-0` file — layout, loader, row count (Basic.cpp:271, verified from baked bin)

### 4.1 File identity + loader

```c
// Basic.cpp:271  — the DAT (cell data) — inside game.gog
Pathstruct xadatabuild00dat = { "DATA/BUILD0-0.DAT\0", &BUILD00DAT_BEGIN_BUFFER, NULL, 0, 0 };
// Basic.cpp:273  — the TAB (index rows)
Pathstruct xadatabuild00tab = { "DATA/BUILD0-0.TAB\0", (uint8_t**)&BUILD00TAB_BEGIN_BUFFER, (uint8_t**)&BUILD00TAB_END_BUFFER, 0, 0 };
```

`filearrayindex_BUILD00DATTAB = 8` (Basic.cpp:152). ONE bank for all MC2 environments (fixed path, no
per-variant suffix). Loaded via the `filearray_2aa18c[8]` slot (Basic.cpp:236:
`{ &BUILD00TAB_BEGIN_BUFFER, &BUILD00TAB_END_BUFFER, &BUILD00DAT_BEGIN_BUFFER, &posistruct9 }`), indexed by
`CreateIndexes_6EB90` (PlayerInput.cpp:2578, EF:39360, EF:42892).

### 4.2 TAB row format + indexing — VERBATIM

Each `.TAB` row is a `bitmap_pos_struct2_t` (portability/bitmap_pos_struct.h:27), **6 bytes packed**:

```c
typedef struct {           // #pragma pack(1)
    uint32_t data_0;       // BYTE OFFSET into the .DAT buffer for this row's cell data
    uint8_t  width_4;      // footprint width in tiles
    uint8_t  height_5;     // footprint height in tiles
} bitmap_pos_struct2_t;
```

`CreateIndexes_6EB90` → `sub_9874D_create_index_dattab` (DatTabIndexes.cpp:32) resolves each row into the
runtime `posistruct[i]` = `bitmap_pos_struct_t {uint8_t* data, uint8_t width_4, uint8_t height_5}` (data =
`datbuffer + tabrow.data_0`, dims copied 1:1). In VGA-1/half-res mode the `_power` variant (DatTabIndexes.cpp:3)
doubles width/height instead — but for BUILD00 the base path is `sub_9874D` (1:1). **Row count = (TAB_END -
TAB_BEGIN) / 6** (the loop bound is `tabbufferend - tabbuffer` over `bitmap_pos_struct2_t*`, i.e. element
count).

### 4.3 Cell data layout (per builder-doc §2.3/2.4)

`posistruct[row].data` is **2 bytes per cell** for MC2 (MC1 = 1 byte), `width_4 * height_5` cells, row-major:
`data[2*i+0]` = angle/sprite paint code (0xFF = skip), `data[2*i+1]` = height offset delta (0xFF = skip). Read
by `sub_36FC0` (EF:27077-27170, the terrain stamp) and `RemoveCastleStage_385C0` (EF:28080). The per-row
"scatter mana while building" flag `& 1` and "cave-eligible" `& 4` live in a SEPARATE table
`str_D93C0_bldgprmbuffer[row].byte_2` (BLDGPRM.DAT), NOT in BUILD00.

### 4.4 VERIFIED row count + dims (read-only, from the baked bundle — gamedata not modified)

The raw `DATA/BUILD0-0.TAB` lives inside `gamedata/Magic Carpet 2/game.gog` (the CD image, 401 MB, not
directly opened here). The project's importer already extracts it verbatim to
`baked/assets/mc2-day/build.tab.bin` (462 bytes) and `build.dat.bin` (80064 bytes). Parsed as 6-byte rows:

- **`build.tab.bin` = 462 bytes = 77 rows.** (All MC2 variants share the same bank; mc2-cave/mc2-night-fog
  bake the identical file.)
- Row dims (first 20):

| row | .DAT offset | w | h | note |
|---|---|---|---|---|
| 0 | 0 | 0 | 0 | empty (level-0 castle footprint) |
| 1 | 0 | 8 | 8 | **castle level 1** (DB038 → 1 centred piece) |
| 2 | 128 | 21 | 21 | **castle level 2** (DB038 → 4 corners) |
| 3 | 1010 | 21 | 21 | **castle level 3** |
| 4 | 1892 | 35 | 35 | **castle level 4** |
| 5 | 4342 | 35 | 35 | **castle level 5** |
| 6 | 6792 | 48 | 48 | **castle level 6** (DB038 → 8 pieces) |
| 7 | 11400 | 48 | 48 | **castle level 7** (same 8-piece list as L6) |
| 8..16 | 16008.. | 1 | 1 | 1×1 rows (small buildings / markers) |
| 17 | 16026 | 48 | 48 | (large building) |
| 18 | 20634 | 15 | 15 | |
| 19 | 21084 | 6 | 4 | |

*[CORRECTED 2026-07-16, folding §Trace-bank corrections 5: the original table was OFF BY ONE from row 1
down — it read row N's dims against row N+1's offset, which produced the false "row 7 = 1×1 degenerate
castle" story. Re-read from baked/assets/mc2-day/build.tab.bin: rows 1-7 = 8/21/21/35/35/48/48 (exactly the
G9j code law, castle.rs), and the 1×1 building band starts at row 8.]*

**Note:** the castle build ladder uses rows **0..7** where the level-N castle stamps BUILD00 row N (via
`byte_0x46_70 = level`, builder-doc §1.2/§2.1). Row 7 is a full 48×48 footprint (offset 11400), sharing
row 6's dims exactly as DB038 gives L6 and L7 the same 8-piece list — there is NO degenerate level-7
footprint (that story was the off-by-one above). Rows 8..76 are the general building/scenery footprints
consumed by the `(10,45)` possessed-building and terrain-author paths, not the castle.

---

## 5. Consolidated constants + field map

| item | value / location | cite |
|---|---|---|
| `x_BYTE_DB038` | static `int8_t[52]`, verbatim §1.1 | EF:2594 |
| DB038 count | `[2*level]` (u8) | EF:62293 |
| DB038 list index | `[1 + 2*level]` (u8, = pair-slot index) | EF:62295 |
| DB038 pairs base | `&[18]`, 2 bytes/(x,y) tile offset | EF:62295 |
| piece level lists | L1:{(4,4)} L2/3:{4 corners of 21²} L4/5:{4 corners of 35²} L6/7:{8 of 48²} | §1.3 |
| piece z | `terrainAlt + 384` (L≤1) else `+224` | EF:62315 |
| `array_0x24E_590` | `int8_t[19]` on `dword_0xA4_164` @0x24E | global_types.h:304 |
| write site (both channels) | `sub_69AB0` research-complete | EF:56120-56121 |
| `[level]` channel | `= SPELLS[model].subspell[row].subSpellIndex_2` = **HP FACTOR** (read by ladder) | EF:56120 / :61704 |
| `[9+level]` channel | `= SPELLS[model].subspell[row].life_0x1A` = **PART-TYPE** (read by piece builder) | EF:56121 / :62274 |
| HP factor use | `number1 = (Life * ((factor<<8)+256))>>8`; factor 0 ⇒ 1.0× | EF:61704 |
| `word_0x24A_586` (Life) | default **256**; AI ← `WizardMapSettings[color].Life_0x3612F` | EF:43720 / :43771 |
| Life file source | map header `Type_WizardMapSettings_0x360D2[8].Life_0x3612F` @0x3612F | BasicTerrain.h:31 |
| `player_0x2FED9[8]` | authored starting-castle LEVEL per color, header @0x2FED9 | BasicTerrain.h:77 |
| `player_0x2FED9` file field | `array_0x2FED9[8]` in compressed header | LevelStructs.h:291 |
| `player_0x2FED9` copy | `DecompressLevel_2FECE` | ConvertMapInfo.cpp:10 |
| `player_0x2FED9` read | level-load castle spawn+stamp | EF:43777, :43789 |
| BUILD00 files | `DATA/BUILD0-0.DAT` + `.TAB`, file index 8 | Basic.cpp:271/273, :152 |
| TAB row | `bitmap_pos_struct2_t {u32 offset, u8 w, u8 h}` = 6 bytes | bitmap_pos_struct.h:27 |
| TAB → posistruct | `sub_9874D_create_index_dattab` (1:1) / `_power` (×2 half-res) | DatTabIndexes.cpp:32/3 |
| row count | **77** (462-byte TAB / 6) | baked/assets/mc2-day/build.tab.bin |
| castle rows | 0..7; L1=8×8, L2/3=21×21, L4/5=35×35, L6/7=48×48 | §4.4 |
| DAT cell data | 2 bytes/cell {[0]=paint 0xFF skip, [1]=height 0xFF skip} | EF:27103 |

---

## 6. OPEN

- ~~Level-7 castle footprint = 1×1~~ **CLOSED 2026-07-16** (§Trace-bank corrections 5): the "degenerate
  row 7" was an off-by-one read of the TAB (see the corrected §4.4 table). Row 7 = 48×48; L6 and L7 share
  the footprint exactly as DB038's shared 8-piece list implies. The code (G9j) already carries the correct
  8/21/21/35/35/48/48 ladder; its old defensive workaround was a harmless no-op.

- **`array_0x24E_590` is populated LAZILY by castle research (`sub_69AB0`), not at level load.** A cast or
  authored castle starts with the array all-zero ⇒ HP factor 1.0× (= MC1 flat HP) and NO (10,79) pieces until
  the research/production children complete. **Confirm whether authored MC2 starting castles are meant to show
  pieces / scaled HP immediately** (they would need a research pre-fill the load path §3.3 does NOT do), or
  whether they legitimately start piece-less at factor 1.0. This gates whether the port needs to synthesize
  `array_0x24E_590` at authored-castle load.

- **`sub_69AB0`'s full trigger chain** (what research/production entity has this as its action handler, what
  `model_0x40_64` / `byte_0x46_70` it carries so the SPELLS lookup resolves) was not walked end-to-end here —
  only the write itself is verbatim. The `model` selects the SPELLS row and `byte_0x46_70` the subspell tier;
  trace the spawner of the `sub_69AB0` entity (a class-9 `(9,10)` sphere is spawned at EF:56123 as a
  by-product) before wiring the research→table population. (This is the MC2 castle RESEARCH system, distinct
  from the build system this doc's siblings cover.)

- **`WizardMapSettings_0x360D2` full struct** (BasicTerrain.h:20, 110 bytes) carries Aggression/Reflexes/
  Perception/StartingSpells/BlockedSpells/Life — only `Life_0x3612F` was needed here; the rest is the rival
  personality block already handled by `wizards.json` per the mob-AI ledger. Confirm the project's
  `wizards.json` Life value maps to `word_0x24A_586` (16.8, 256=1.0×) for castle-HP parity on rivals.
