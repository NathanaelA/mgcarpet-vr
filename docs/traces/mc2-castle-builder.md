# MC2 CASTLE BUILDER — port-ready verbatim trace

> **CORRECTIONS (see `mc2-castle-open-items.md` + the runtime doc's banner, 2026-07-11):**
> §1.4/§9 — the (10,32) seed's action 0x22 resolves through the CLASS-10 table to `sub_344A0`
> (a projectile), NOT BeginOfCastleCreation; never authored, dead path for the port. §3.2 —
> the (10,79) piece ctor is `sub_508E0_castle_defend_create` → action 0x56 =
> `sub_3AF00_castle_defend_event` (the defender launcher). §6 — `sub_60400` returns
> (balloons, guards): (1,0)/(1,0)/(1,4)/(2,6)/(2,14)/(3,18)/(3,34) by level (the "{2,2,2,2,
> 2,3,3} allowance" reading was a count/base misparse). §5/§9 — `sub_5F890` is the HUD
> build-ghost sync, not occupant ejection (there is none).

Port-ready verbatim trace of the **MC2-native castle column**: how a castle comes into being, gets
stamped onto terrain, and grows/shrinks through its stage pieces. The project currently runs MC1's castle
machinery (class-3 model-2 core, MC1 HP/CAP tables, MC1 leveler/painter creators) on MC2 worlds; THIS doc
is the foundation for the MC2-native swap. The Phase-0 survey (`docs/SURVEY-MC2.md` ~200-270) already
covers the runtime intake/regen/ladder IN SUMMARY — this doc is the BUILD/STAGE machinery it skipped.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`: EF = `engine/EventsFunctions.cpp`,
EV = `engine/Events.cpp`, Terrain = `engine/Terrain.cpp`. Read `mc2-class10-m67-flood-helpers.md` for
transcription conventions first. Trace date 2026-07-11.

---

## Headline findings (read first)

1. **THE CASTLE CORE IS `class 3, model 2` — SAME AS MC1.** The persistent castle entity a wizard owns is
   class-3 model-2, spawned via `IfSubtypeCallCreatingManaSphere_4A190(pos, 3, 2)` (EF:6833 runtime cast,
   EF:43779 level-load). The terrain-authored variant is `sub_4AA40` ("Bad Stone", EF:33362, `maxLife
   40000`, `byte_0x38_56 = 33`). So the project's class-3 model-2 assumption is **correct** — the delta is
   almost entirely in the CREATORS, the HP/CAP tables, and the guard, NOT the core class/model.

2. **THE BUILD-STATE FIELD MAPPING IS IDENTICAL to the project's MC1 column.** `dword_0x10_16` = **level**
   (project `f26`); `word_0x2E_46` = **build sub-state** (project `f59`); `maxMana_0x8C_140` = **capacity**
   (project `f136`); `axis_0x9A_154x` = **build datum / site position** (project `site_z`, but note MC2
   uses the full 3-axis `axis_0x9A_154x`). `byte_0x46_70` = **BUILD00 row index** (usually == level).
   `array_0x52_82.pitch/.roll` = the AABB half-extents driven from the build row (`SetShiftByCastle_49EC0`).

3. **THE CREATOR PAIR IS SHARED WITH MC1 IN MODEL NUMBER, DIFFERENT IN DISPATCH.** MC2's build cycle
   spawns **(10,42)** = the *painter/geometry* helper (`sub_5FBD0` EF:61182) and **(10,41)** = the *ground
   leveler* (`sub_5FC40` EF:61202) — the same m41/m42 pair the project's MC1 `castle_tick` already uses.
   MC2's `BeginOfCastleCreation_5FA70` (EF:61123) is a `word_0x2E_46` state machine (cases 0/2/3/4/5/1&6)
   that drives them; **port this as the MC2 castle_tick, diffing against `mc1/features.rs::castle_tick`.**

4. **THE ACTUAL STAGE PIECES ARE `class 10, model 79` (0x4F)**, spawned by `sub_613D0` (EF:62233) as a
   `word_0x34_52`-linked chain hanging off the castle, one per BUILD00 sub-part row. `RemoveCastleStage_385C0`
   (EF:28071) is a **terrain-restore + mana-scatter** routine that consumes the BUILD00 row cell-by-cell —
   it is NOT the sprite-piece remover. The (10,52)=0x34 anchor (`sub_50430`) is a SEPARATE permanent
   building entity, **not part of the castle chain**.

5. **THE GUARD IS `class 5, model 15` (`5,15`), spawned INSIDE the intake driver `sub_5FF50`** (EF:61488),
   one per `player_0x2FED9` allowance slot (`array_0x5C_92`), on a 16-tick respawn cooldown
   (`word_0x2C_44 = 16`). Confirmed: EF:61488 = `IfSubtypeCallCreatingManaSphere_4A190(&pos, 5, 15)`.

---

## 0. Entity taxonomy (the cast of the MC2 castle system)

| entity | class,model | ctor / spawner | action | role |
|---|---|---|---|---|
| **Castle core** | 3, 2 | `sub_4AA40` (EF:33362, authored) / `(3,2)` spawn (EF:6833,:43779) | 5 | the owned, persistent castle; holds level/cap/mana/HP |
| Castle-creation seed | 10, 32 (0x20) | `sub_4FA60` (EF:36292) | 0x22 → `BeginOfCastleCreation_5FA70` | short-lived "build me" order entity; par1→level |
| Build painter | 10, 42 (0x2A) | `sub_5FBD0`→`(10,42)` (EF:61188); ctor `sub_50370` (EF:36733) | 0x2C | paints geometry/heightmap for one build pass |
| Ground leveler | 10, 41 (0x29) | `sub_5FC40`→`(10,41)` (EF:61208) | — | levels/flattens terrain under the castle |
| Stage sprite piece | 10, 79 (0x4F) | `sub_613D0`→`(10,79)` (EF:62303) | — | the visible wall/tower sprites; `word_0x34_52` chain |
| Mana carrier | 10, 39 (0x27) | `sub_5FD00`→`(10,39)` (EF:61301) | — | scattered mana spheres on overflow/downgrade |
| Guard | 5, 15 | `sub_5FF50`→`(5,15)` (EF:61488) | 9 (idle) | castle militia; `array_0x5C_92` slots |
| Permanent anchor | 10, 52 (0x34) | `sub_50430` (EF:36771), sprite 205 | 0x38 (no-op) | standalone building landmark — **NOT castle-linked** |

---

## 1. CREATION SITES

### 1.1 Player cast — Create Castle (spell verb 2), `sub_15730` case 2 (EF:6820)

The spell dispatcher `sub_15730` (EF:7156) switches on the spell verb `a2`; **verb 2 = Create Castle**
(EF:6820). Transcribed:

```c
// EF:6820  (sub_15730, case 2u — Create Castle verb)
case 2u:
    v2x = sub_146C0(a1x, a2);                    // resolve the cast target/anchor point
    if (!v2x)
        return 0;
    if (a1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58)   // wizard ALREADY has a castle
    {
        if (sub_5F660(a1x, v2x, 0) != 1)         //   → re-cast = UPGRADE trigger (§4)
            return 0;
        a1x->dword_0xA4_164x->str_611.array_0x367_871x.SpellEnabled[a2] = x_WORD_D3F4C[a2];
        result = 1;
    }
    else                                          // wizard has NO castle yet → CREATE
    {
        v4x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->axis_0x9A_154x, 3, 2);   // spawn class-3 model-2
        if (v4x)
        {
            v4x->id_0x1A_26 = a1x->id_0x1A_26;                          // BIND owner (entity id)
            a1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58 = v4x - D41A0_0.struct_0x6E8E;  // wizard→castle link
        }
        result = 1;
    }
    break;
```

- **Position rule:** the castle spawns at `a1x->axis_0x9A_154x` — the wizard's **spell-anchor axis** (the
  landing-datum captured by `sub_146C0`), NOT the live flight position. This is `site_z` in the project.
- **Initial fields:** the raw `(3,2)` spawn from `IfSubtypeCallCreatingManaSphere_4A190` gives the ctor
  defaults; the level (`dword_0x10_16`) starts at 0. The FIRST build pass (`BeginOfCastleCreation_5FA70`
  case 0 → `sub_60480`, §4) raises it to level 1. Owner binding = **`CastleEntityIndex_0x3A_58`** on the
  wizard's `dword_0xA4_164x` (player struct) ↔ **`id_0x1A_26`** on the castle (== the owner wizard's id).
- **`sub_146C0`** (EF:6403) is the verb-2 caster preamble shared by several build verbs; it validates the
  cast and returns the anchor entity, else 0 (cast fails silently).

### 1.2 Level-authored starting castles — the THING/level-load path (EF:43779)

When a wizard (class 3) is instantiated at level load and `player_0x2FED9[color]` (the authored starting
castle **level**) is nonzero, the load code builds the castle inline. Transcribed:

```c
// EF:43775  (wizard spawn tail; v2x = the wizard entity)
if (v2x->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2])   // Create-Castle spell enabled
{
    if (D41A0_0.terrain_2FECE.player_0x2FED9[v2x->dword_0xA4_164x->playerColorIndex_0x38_56])  // authored level>0
    {
        v16x = IfSubtypeCallCreatingManaSphere_4A190(&v2x->position_0x4C_76, 3, 2);   // spawn class-3 model-2
        v39x = v16x;
        if (v16x)
        {
            v16x->id_0x1A_26 = v2x->id_0x1A_26;                                  // BIND owner
            v2x->dword_0xA4_164x->CastleEntityIndex_0x3A_58 = v16x - D41A0_0.struct_0x6E8E;
            PrepareEventSound_6E450(v2x - D41A0_0.struct_0x6E8E, -1, 30);        // build sound 30
            for (j = 0; ; j = v22 + 1)                                          // stamp EACH level 0..(level-1)
            {
                v23 = D41A0_0.terrain_2FECE.player_0x2FED9[...playerColorIndex...];  // authored level
                if (v23 <= j) break;
                *(&Entities_EA3E4[0]->position_0x4C_76) = v39x->axis_0x9A_154x;      // scratch entity [0]
                Entities_EA3E4[0]->model_0x40_64 = 0;
                Entities_EA3E4[0]->dword_0x10_16 = 0;
                Entities_EA3E4[0]->id_0x1A_26   = v39x->id_0x1A_26;
                Entities_EA3E4[0]->byte_0x46_70 = j;                                // BUILD00 row = this level
                sub_36FC0(Entities_EA3E4[0]);                                       // STAMP terrain for level j (§2)
            }
            v39x->dword_0x10_16 = v23 - 1;                                          // castle level = authored-1
            SetShiftByCastle_49EC0(v39x, v39x->dword_0x10_16);                      // AABB extent for level
            v39x->array_0x52_82.yaw = 0xe000;
            v39x->array_0x52_82.fov = 0x4000;
            sub_60810(v39x);                                                        // HP/CAP ladder (§4/§7)
            v27x->mana_0x90_144 = v27x->maxMana_0x8C_140;                           // start full mana
            // (AI personality: word_0x242/244/246_578/580/582 loaded EF:43764-43772; see survey)
        }
    }
}
```

- **Class/model records that make castles at load:** the same `(3,2)` spawn. There is no separate castle
  THING class; the castle is derived from the wizard's `player_0x2FED9[color]` authored **level** field, one
  per wizard color. Each level 0..(N-1) is stamped by a `sub_36FC0` call with `byte_0x46_70 = j`.
- **Binding to owner:** identical to the cast path — `CastleEntityIndex_0x3A_58` (wizard→castle) +
  `id_0x1A_26` (castle→owner). The castle's final level is `player_0x2FED9 - 1` (the loop stamps N passes,
  sets level to N-1).
- **par consumption:** the authored **level** comes from `player_0x2FED9[color]` (map header wizard record),
  NOT from a par1 on a THING row. (The separate model-32 **seed** DOES consume par1 into `byte_0x46_70` at
  trigger-spawn, EF:33200 — see §1.4.)

### 1.3 AI / rival castle creation

**Rivals use the SAME two paths, no separate AI creator.** At load an AI wizard with a nonzero
`player_0x2FED9` gets its castle via §1.2 (the `IsAiPlayer_0x009_2BE4_11239 == 1` branch at EF:43761 only
loads personality fields; the castle build above is shared). At runtime the AI brain casts Create Castle
through the same `sub_15730` verb-2 path (§1.1) when its brain decides to (survey: AI castle logic in
`sub_5EFA0` region). So: **no AI-specific castle ctor — port one creation column, both humans and rivals
drive it.**

### 1.4 The model-32 (0x20) castle-creation SEED (`sub_4FA60`, EF:36292)

```c
// EF:36292
type_entity_0x6E8E* sub_4FA60(axis_3d* position)//230a60
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->maxLife_0x4        = 0;
        event->actionIndex_0x45_69 = 0x22;      // action 34 → BeginOfCastleCreation_5FA70
        event->class_0x3F_63       = 0xA;       // class 10
        event->model_0x40_64       = 0x20;      // model 32
        event->position_0x4C_76    = *position;
        event->actSpeed_0x82_130   = 256;
        event->byte_0x46_70        = 2;         // default BUILD00 row 2
        event->struct_byte_0xc_12_15.byte[0] &= 0xF7;
        CopyMaxLifeToLife_49A20(event);
    }
    return event;
}
```

At **trigger-spawn** (`sub_4A310` case 0xB, EF:33198-33206) a model-0x20 seed consumes **par1 → byte_0x46_70**:
```c
// EF:33198
case 0xB:
    indexx->id_0x1A_26 = entity->stageTag_12;
    if (indexx->model_0x40_64 == 0x20)
        indexx->byte_0x46_70 = entity->par1_14;   // par1 selects the BUILD00 row (level)
    else
        SetEntityShiftRot_49EA0(indexx, entity->word_10 << 8, 4096);
    CopyMaxLifeToLife_49A20(v3x);
    v3x->struct_byte_0xc_12_15.byte[0] |= 1u;
    sub_58DA0(entity, v3x);
    return;
```
So an authored `(10,32)` THING row with `par1 = L` seeds a castle-build order at BUILD00 row L. The seed's
action 0x22 → `BeginOfCastleCreation_5FA70` immediately begins the build cycle (§2).

---

## 2. THE BUILD PROCESS — cast → standing castle

The build cycle is the `word_0x2E_46` (= project `f59`) state machine in **`BeginOfCastleCreation_5FA70`**
(EF:61123), the action handler for the castle-build family. VERBATIM:

```c
// EF:61123
void BeginOfCastleCreation_5FA70(type_entity_0x6E8E* locEvent)//240a70
{
    switch (locEvent->word_0x2E_46) {
    case 0:                                              // ── PRE-CLEAR + BUILD LEVEL ──
        sub_11960(locEvent);                             // house pre-clear (clear old footprint)
        if (!locEvent->dword_0x10_16 || sub_11A10(locEvent))   // level 0, OR space-check passes
        {
            if ((locEvent->struct_byte_0xc_12_15.byte[0] & 2) == 0) {
                locEvent->word_0x5A_90 += TransformPlayerColorIndex_616D0(          // apply owner color palette
                    Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x->playerColorIndex_0x38_56);
                locEvent->struct_byte_0xc_12_15.byte[0] |= 2u;                       // mark "colored"
            }
            sub_60480(locEvent);                          // ← LEVEL-UP (§4): raise level, spawn painter, ladder
        }
        else {                                            // space-check FAILED
            locEvent->word_0x2E_46 = 2;                   //   → state 2 (abort/retry)
            locEvent->struct_byte_0xc_12_15.byte[0] &= 0xBF;
            sub_88D00();                                  //   UI feedback ("no room")
        }
        break;
    case 1:
    case 6:
        locEvent->position_0x4C_76.z = getTerrainAlt_10C40(&locEvent->position_0x4C_76);  // settle on ground
        break;
    case 2:                                               // ── ABORT ──
        locEvent->actionIndex_0x45_69 = 4;                //   switch to steady-state action 4
        sub_5F890(locEvent, 0);
        locEvent->word_0x2E_46 = 0;
        break;
    case 3:                                               // ── SPAWN PAINTER ──
        sub_5F890(locEvent, 1);
        sub_5FBD0(locEvent);                              //   spawn (10,42) painter for current level (§2.1)
        break;
    case 4:                                               // ── WAIT for painter, then flip to leveler ──
        locEvent->position_0x4C_76.z = getTerrainAlt_10C40(&locEvent->position_0x4C_76);
        if ((locEvent->byte_0x3E_62 & 0x1F) == 0) {       //   every 32 ticks
            bool is10_42Type = false;                     //   is a (10,42) painter still alive?
            for (i = dword_38535; i > Entities_EA3E4[0] && !is10_42Type; i = i->next_0)
                if (i->class_0x3F_63 == 10 && i->model_0x40_64 == 42) is10_42Type = true;
            if (!is10_42Type)
                locEvent->word_0x2E_46 = 3;               //   painter done → go re-spawn (state 3)
        }
        break;
    case 5:                                               // ── SPAWN LEVELER ──
        sub_5F890(locEvent, 1);
        locEvent->position_0x4C_76.z = getTerrainAlt_10C40(&locEvent->position_0x4C_76);
        sub_5FC40(locEvent);                              //   spawn (10,41) ground leveler (§2.2)
        break;
    default:
        return;
    }
}
```

**Sequence:** cast/seed (state 0) → pre-clear (`sub_11960`) → space-check (`sub_11A10`) → `sub_60480`
raises level & fires the painter chain → the machine oscillates state 3↔4 spawning painters until geometry
is complete → state 5 spawns the leveler. The visible sprite pieces (10,79) are added separately by
`sub_613D0` (§3) which `sub_60480` and the downgrade both call.

### 2.1 Painter spawn `sub_5FBD0` (EF:61182) — VERBATIM

```c
// EF:61182
void sub_5FBD0(type_entity_0x6E8E* a1x)//240bd0
{
    indexx = IfSubtypeCallCreatingManaSphere_4A190(&a1x->axis_0x9A_154x, 10, 42);  // spawn (10,42) painter
    if (indexx) {
        indexx->byte_0x46_70   = a1x->dword_0x10_16;          // BUILD00 row = castle level
        indexx->id_0x1A_26     = a1x->id_0x1A_26;             // owner
        indexx->parentId_0x28_40 = a1x - D41A0_0.struct_0x6E8E;  // parent = castle
        a1x->word_0x2E_46 = 4;                                // castle → wait-for-painter state
        SetShiftByCastle_49EC0(indexx, a1x->dword_0x10_16);   // painter extent = build-row footprint
    }
}
```

### 2.2 Leveler spawn `sub_5FC40` (EF:61202) — VERBATIM

```c
// EF:61202
type_entity_0x6E8E* sub_5FC40(type_entity_0x6E8E* a1x)//240c40
{
    resultx = IfSubtypeCallCreatingManaSphere_4A190(&a1x->axis_0x9A_154x, 10, 41);  // spawn (10,41) leveler
    if (resultx) {
        resultx->byte_0x46_70   = a1x->dword_0x10_16;
        resultx->id_0x1A_26     = a1x->id_0x1A_26;
        resultx->parentId_0x28_40 = a1x - D41A0_0.struct_0x6E8E;
        a1x->word_0x2E_46 = 6;                                // castle → settle state 6
    }
    return resultx;
}
```

### 2.3 Terrain stamp `sub_36FC0` (EF:27030) — the heightmap/flatten writer (VERBATIM core)

Called at level-load per authored level (§1.2) and by the painter. Reads BUILD00 row `byte_0x46_70`,
stamps a `width × height` footprint. Key writes (EF:27077-27170):

```c
// EF:27077  (v1 = byte_0x46_70 = build row; v25 = position.z >> 5 = height DATUM)
v3   = posistruct[v1].height_5;                     // footprint height (tiles)
v17  = posistruct[v1].data;                         // 2 bytes/cell: [0]=sprite/angle, [1]=height delta
v4   = posistruct[v1].width_4;                       // footprint width (tiles)
if (!IsNextEvent0A_2A_37740(a1x)) {                  // skip if a build entity already pending here
    if (x_WORD_180660_VGA_type_resolution == 1) { v3>>=1; v4>>=1; }   // half-res halves the footprint
    v27x.x = v26 - (v4>>1);  v27x.y = v28 - (v3>>1);   // ORIGIN = centerTile - halfFootprint
    for (y=0; y<height; y++) for (x=0; x<width; x++) {
        sub_57390(cell, id);                          // register cell owner
        v7 = data[2*i+1];                             // height byte
        if (v7 != 0xff) {
            mapHeightmap_11B4E0[cell] = v7 + v25;     // HEIGHT WRITE: build-cell height = delta + datum
            if (!(mapAngle_13B4E0[cell] & 7)) {       //   if cell angle low3 == 0
                mapAngle_13B4E0[cell] = (angle & 0xF8) | 1;   //   set low3 = 1 (built-flag)
                sub_462A0(cell, cell);                //   RETILE the cell = project's mc2_retile_region
            }
        }
        // cave-level second-heightmap (ceiling) handling: EF:27114-27137 (skip on non-cave)
    }
    // second pass EF:27153-27169: for each non-0xff data[0], sub_45DC0(...) = sprite/shading paint
}
```

**Port note:** `sub_462A0` = the project's already-ported `mc2_retile_region`; `sub_45DC0` = the sprite
paint. The datum `v25 = position.z >> 5` is the `site_z`-derived build height. `data` is 2 bytes/cell:
byte[1] = height offset (0xff = skip), byte[0] = angle/sprite id (0xff = skip). `SetHeightmapByBuildingArea_48B50`
finalizes the shading over the box (called by `RemoveCastleStage`, §5).

### 2.4 BUILD00 row layout (`bitmap_pos_struct_t`, file index 8)

`filearrayindex_BUILD00DATTAB = 8` (Basic.cpp:152). The per-row struct exposes `.width_4`, `.height_5`,
and `.data` (2 bytes/cell). Indexed by `byte_0x46_70` (= level/build-row). Half-res (`x_WORD_180660 == 1`)
halves both dims. `str_D93C0_bldgprmbuffer[row].byte_2 & 1` = a per-row "scatter mana while building" flag
(consumed in `RemoveCastleStage` §5 and `AddHouse` EF:27998); `& 4` = "cave-eligible" flag (EF:27089).

### 2.5 Sounds & timing

- Build sound = `PrepareEventSound_6E450(..., -1, 30)` (EF:43786 load, EF:61627 downgrade). Upgrade tick
  plays sound **10** (`sub_60480` EF:61578). Downgrade plays sound **30** (`sub_605E0` EF:61627).
- Timing is state-machine paced (state 4 polls every 32 ticks via `byte_0x3E_62 & 0x1F`), not a fixed timer.

---

## 3. CASTLE STAGES

### 3.1 What a "stage" is

Two distinct things wear the word "stage":
- **Visible sprite pieces** = `class 10, model 79` (0x4F), created by **`sub_613D0`** (EF:62233) as a
  singly-linked chain rooted at the castle's `word_0x34_52`, each piece pointing back via `word_0x32_50`.
  These are the wall/tower graphics; they are re-generated on every level change.
- **The `RemoveCastleStage_385C0` routine** (EF:28071) — despite its name — is the **terrain-restore**
  routine that un-stamps one BUILD00 footprint (restores heightmap, scatters mana). It runs on downgrade
  and on the scratch entity [0].

### 3.2 Stage-add (visible pieces) `sub_613D0` (EF:62233) — VERBATIM core

```c
// EF:62257  (a1x = castle)
for (i1 = a1x->word_0x34_52; ...; i1 = v2x->word_0x34_52) {   // free the OLD piece chain first
    v2x = Entities_EA3E4[i1];  if (v2x == Entities_EA3E4[0]) break;
    sub_57F20(Entities_EA3E4[i1]);
}
a1x->word_0x34_52 = 0;
if (a1x->id_0x1A_26 && a1x->dword_0x10_16) {                  // owner set AND level > 0
    v4 = a1x->dword_0x10_16;
    while (v4 > 0) {                                          // find highest authored sub-part ≤ level
        v16 = player->array_0x24E_590.at(9 + v4);            // per-level part table (owner's array_0x24E_590)
        if (v16) break;
        v4--;
    }
    if (v4) {
        i1 = posistruct[v4].width_4;  v8 = posistruct[v4].height_5;   // footprint for this level
        v20 = centerTileX - (i1>>1);  v19 = centerTileY - (v8>>1);    // origin
        v14 = x_BYTE_DB038[2*v4];                             // part COUNT for this level  (DB038 table)
        v13 = &x_BYTE_DB038[18] + 2*x_BYTE_DB038[1 + 2*v4];   // part OFFSET-LIST base
        for (v15=0; v15 < v14; v15++) {                       // one (10,79) piece per part
            predictedAxis.x = (v20 + v13[0]) << 8;            // part tile offset from DB038 list
            predictedAxis.y = (v19 + v13[1]) << 8;
            i2x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis, 10, 79);   // spawn (10,79) piece
            if (!i2x) break;
            i2x->word_0x32_50 = a1x_idx;   a1x->word_0x34_52 = i2x_idx;  i2x->word_0x34_52 = 0;  // chain
            i2x->id_0x1A_26   = a1x->id_0x1A_26;              // owner
            i2x->byte_0x43_67 = v16;                          // part-type byte
            i2x->word_0x4A_74 = v4;                           // level tag
            v11 = getTerrainAlt_10C40(&i2x->position);
            i2x->position.z = (v4 <= 1) ? v11 + 384 : v11 + 224;   // piece height above ground by level
            v13 += 2;
        }
    }
}
```

- Stages are keyed to **level** (`dword_0x10_16`) via the owner's `array_0x24E_590[9+level]` part table and
  the `x_BYTE_DB038` offset table (count at `[2*level]`, offset-list index at `[1+2*level]`, base at `[18]`).
- `sub_613D0` is called by **`sub_60480`** (upgrade, EF:61595) and **`sub_605E0`** (downgrade, EF:61642) —
  it fully rebuilds the piece chain to match the current level.

### 3.3 `RemoveCastleStage_385C0` (EF:28071) — terrain restore + mana scatter — VERBATIM

```c
// EF:28071
void RemoveCastleStage_385C0(type_entity_0x6E8E* event)//2195c0 //remove castle stage
{
    uint8_t* locData = posistruct[event->byte_0x46_70].data;         // BUILD00 row cell data
    unsigned locHeight = posistruct[event->byte_0x46_70].height_5;
    unsigned locWidth  = posistruct[event->byte_0x46_70].width_4;
    if (x_WORD_180660_VGA_type_resolution == 1) { locHeight>>=1; locWidth>>=1; }
    locAxis1.x = ((event->position.x + 128) >> 8) - (locWidth  >> 1);   // origin = centerTile - halfW
    locAxis1.y = ((event->position.y + 128) >> 8) - (locHeight >> 1);
    if (!event->fontTypeIndex_0x3D_61) {                              // NORMAL restore branch
        if (event->model_0x40_64)  zKoef = GetTerrainHeightFromSquare_48DF0(x,y,H,W);  // 4-corner mean height
        else                       zKoef = event->position.z >> 5;    // model 0 (scratch): use datum
        locData2 = locData;
        int locIndex = 0;
        for (y=0; y<locHeight; y++) for (x=0; x<locWidth; x++) {
            if (locData2[1] != 0xff || locData2[0] != 0xff) {         // active cell
                predictedAxis.x = locAxis3.x << 8;
                predictedAxis.y = locAxis3.y << 8;
                predictedAxis.z = 32 * zKoef;
                if (!(++locIndex & 7)) predictedAxis.z = 32 * (zKoef - 10);   // every 8th cell 10 lower
                if (event->dword_0x10_16 > 0) {                       // ── MANA SCATTER while removing ──
                    event->dword_0x10_16--;
                    if (str_D93C0_bldgprmbuffer[event->byte_0x46_70].byte_2 & 1) {   // row scatters mana
                        if (event->dword_0x10_16) {
                            if (event->dword_0x10_16 >= 4)  tempEvent = GetRandManaSphere_38270(event);
                            else { tempEvent = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis, 5, 4);
                                   if (tempEvent) tempEvent->actionIndex_0x45_69 = 33; }
                        } else { tempEvent = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis, 5, 12);
                                 if (tempEvent) tempEvent->actionIndex_0x45_69 = 97; }
                        if (tempEvent) { tempEvent->str_0x5E_94.dword_0x5E_94 = 1;
                                         tempEvent->str_0x5E_94.word_0x62_98 = event->word_0x26_38; }
                    }
                }
                mapAngle_13B4E0[locAxis3.word] = (angle & 0x70) | 1;  // reset angle low3 = 1
                AddBuildingToTerrain_46570(locAxis3, locAxis3);       // = project mc2_add_building_region
                if (locData2[1] != 0xff) {                            // ── HEIGHT RESTORE ──
                    if (locData2[1] >= mapHeightmap_11B4E0[cell])  mapHeightmap_11B4E0[cell] = 0;
                    else {
                        event->rand = 9377*event->rand + 9439;
                        if (event->rand % 0x32 <= 20)  mapHeightmap[cell] -= locData2[1];   // full drop
                        else { event->rand = 9377*event->rand + 9439;                       // jittered drop
                               mapHeightmap[cell] -= locData2[1] - event->rand % 0x14; }
                    }
                }
            }
            locData2 += 2;  locAxis3.x++;
        }
        SetHeightmapByBuildingArea_48B50(locAxis1.x, locAxis1.y, locHeight, locWidth);  // recompute shading
        if (event->xtype_0x41_65)  sub_4A1E0(event->xtype_0x41_65, 1);
        if (event->byte_0x46_70 == 68) { D41A0_0.word_0x3654A = 0; DisableEntityDrawing04_57F10(event); return; }
        DisableEntityDrawing04_57F10(event);
        return;
    }
    // fontTypeIndex != 0 branch (EF:28183): convert to (10,45) mana-carrier instead of restoring
    tempEvent = IfSubtypeCallCreatingManaSphere_4A190(&event->position, 10, 45);
    if (!tempEvent) { event->fontTypeIndex_0x3D_61 = 0; DisableEntityDrawing04_57F10(event); return; }
    sub_49A30(tempEvent, event->fontTypeIndex_0x3D_61);
    tempEvent->position.z = event->axis_0x9A_154x.z;
    tempEvent->xtype_0x41_65 = event->xtype_0x41_65;
    if (event->playerEntityIndex_0x94_148) { /* re-color + SetEntityIndexAndRot 177, EF:28193-28203 */ }
    sub_59760(event, tempEvent);
    for (y=0;y<locHeight;y++) for (x=0;x<locWidth;x++) {              // clear built-flag bit7 per cell
        if (locData2[1]!=0xff || locData2[0]!=0xff) mapAngle_13B4E0[locAxis1.word] &= 0x7F;
        locData2 += 2;
    }
    DisableEntityDrawing04_57F10(event);
}
```

- **Two modes** keyed on `fontTypeIndex_0x3D_61`: (a) zero → **restore terrain** (drop heightmap back,
  retile, recompute shading) + optionally scatter mana; (b) nonzero → **convert to a (10,45) mana carrier**
  (the "castle became a mobile mana pickup" path).
- **Call sites of `RemoveCastleStage_385C0`:** EV:2679 (its own action dispatch, action index for
  `0x2195c0`), and **EF:61636 inside `sub_605E0`** (downgrade, §5) — where it is driven on the **scratch
  entity [0]** configured with `model = 0`, `dword_0x10_16 = 0`, `byte_0x46_70 = current level`,
  `parentId = castle`, so the `model 0` branch uses the datum and does NOT re-scatter (level already 0).
- `AddBuildingToTerrain_46570` = project's already-ported `mc2_add_building_region`;
  `GetTerrainHeightFromSquare_48DF0` = 4-corner mean sampler (see flood-helpers §6).

### 3.4 The (10,52) anchor (`sub_50430`, EF:36771) — NOT part of the castle

```c
// EF:36771
type_entity_0x6E8E* sub_50430(axis_3d* position)//231430
{
    event->actionIndex_0x45_69 = 0x38;    // action 56 = empty EV case (no-op tick)
    event->class_0x3F_63 = 0xA;  event->model_0x40_64 = 0x34;   // class 10, model 52
    event->maxLife_0x4 = 100000;  event->subSpellIndex_0x2A_42 = 500;
    event->dword_0x10_16 = 600;   event->mana_0x90_144 = 500;   event->maxMana_0x8C_140 = 2000;
    AddEventToMap_57D70(event, position);  CopyMaxLifeToLife_49A20(event);
    SetEntityIndexAndRot_49CD0(event, 205);   // sprite 205
}
```
- **Relationship to castles: NONE structural.** It is a standalone, near-invulnerable (100000 HP) building
  landmark (sprite 205) with its own mana pool (500/2000) and a 600-tick counter, action 0x38 = the empty
  EV case (passive). It is NOT in the castle piece chain, is not owned via `CastleEntityIndex_0x3A_58`, and
  is not created by any castle path. The project already ports it correctly as a permanent building anchor;
  **keep it independent of the castle column.**

---

## 4. THE UPGRADE PATH — `sub_60480` (EF:61563)

Triggered by: (a) the FIRST build (`BeginOfCastleCreation` state 0, EF:61136), and (b) a **re-cast** of
Create Castle when a castle already exists (`sub_15730` case 2 → `sub_5F660`, EF:6826). VERBATIM:

```c
// EF:61563
void sub_60480(type_entity_0x6E8E* a1x)//241480
{
    v1x = sub_50370(&a1x->axis_0x9A_154x);          // spawn (10,42)=0x2A build-painter at the castle datum
    v2x = v1x;
    if (v1x) {
        PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 10);   // upgrade sound 10
        a1x->struct_byte_0xc_12_15.byte[0] &= 0xBFu;
        a1x->dword_0x10_16++;                        // ── LEVEL UP ── (project f26++)
        a1x->actionIndex_0x45_69 = 5;                // castle → steady action 5
        a1x->word_0x2E_46 = 4;                       // castle → wait-for-painter (state 4)
        SetShiftByCastle_49EC0(a1x, a1x->dword_0x10_16);   // new AABB extent for the new level
        a1x->array_0x52_82.yaw = 0xe000;  a1x->array_0x52_82.fov = 0x4000;
        SetShiftByCastle_49EC0(v2x, a1x->dword_0x10_16);   // painter extent too
        v6x = Entities_EA3E4[a1x->id_0x1A_26];        // the OWNER wizard
        v6x->dword_0xA4_164x->CastleEntityIndex_0x3A_58 = a1x - D41A0_0.struct_0x6E8E;  // (re)bind
        v6x->dword_0xA4_164x->word_0x1C2_450 = a1x->dword_0x10_16;   // mirror level to player HUD
        v6x->dword_0xA4_164x->byte_0x1BE_446 = 0;
        sub_60810(a1x);                              // ── HP/CAP LADDER ── (§7)
        sub_613D0(a1x);                              // ── REBUILD visible piece chain (§3.2) ──
        sub_6D8B0(v6x - D41A0_0.struct_0x6E8E, 2u, 1);   // ── +1 CASTLE XP (effect id 2) ──  (EF:61596)
        v2x->parentId_0x28_40 = a1x - D41A0_0.struct_0x6E8E;   // painter parent = castle
        v2x->id_0x1A_26 = a1x->id_0x1A_26;
        v2x->byte_0x3B_59 = 0;
        v2x->byte_0x46_70 = a1x->dword_0x10_16;      // painter BUILD00 row = new level
        v2x->struct_byte_0xc_12_15.byte[2] |= 1;
    }
}
```

- **Trigger:** a re-cast (mana is spent by the spell system before this fires; there is no in-function mana
  threshold — the level-up is unconditional once the painter spawns). NOTE `sub_50370` (EF:36733) is the
  painter ctor: `class 10, model 0x2A (42), action 0x2C, maxLife 0, byte_0x3B_59 = 1`.
- **Per-level geometry/extent:** `SetShiftByCastle_49EC0` (EF:32881) sets the AABB half-extents from the
  BUILD00 row for the new level:
  ```c
  event->array_0x52_82.pitch = ((posistruct[level].width_4  << 8) + 1280) >> 1;
  event->array_0x52_82.roll  = ((posistruct[level].height_5 << 8) + 1280) >> 1;
  event->array_0x52_82.yaw = 0;  event->array_0x52_82.fov = 256;
  ```
- **Space-check:** performed UPSTREAM in `BeginOfCastleCreation` state 0 via `sub_11A10` (EF:61129) — level 0
  skips it; higher levels require it to pass (else state 2 abort + `sub_88D00` "no room"). `sub_60480`
  itself does no space test.
- **XP award:** `sub_6D8B0(ownerId, 2, 1)` = **+1 castle XP** (effect id **2**) per build/level-up
  (EF:61596). This is the "+1 castle XP on build" the survey noted.
- **HUD mirror:** the owner's `word_0x1C2_450` = level, `byte_0x1BE_446 = 0`.

---

## 5. THE DOWNGRADE / DESTRUCTION PATH — `sub_605E0` (EF:61612)

Fired when the castle takes a **lethal** hit (`sub_609E0` intake returns 2, routed via the castle's action
handler → `sub_5FCA0_destroy_castle_level` EF:61221 → `sub_605E0`), and directly by the "destroy castle
level" action. VERBATIM:

```c
// EF:61612
void sub_605E0(type_entity_0x6E8E* a1x)//2415e0
{
    if (a1x->dword_0x10_16 > 0)                          // ── ONE LEVEL DOWN (if level > 0) ──
    {
        v1 = 10 * a1x->maxMana_0x8C_140 / 100;           // 10% of capacity
        a1x->maxMana_0x8C_140 -= v1;                     //   temporarily drop cap by 10%
        sub_5FD00(a1x);                                  //   SCATTER overflow mana as (10,39) spheres (§5.1)
        v2 = a1x - D41A0_0.struct_0x6E8E;
        a1x->maxMana_0x8C_140 += v1;                     //   restore cap (net: 10% mana haircut via scatter)
        PrepareEventSound_6E450(v2, -1, 30);             //   downgrade sound 30
        Entities_EA3E4[0]->position_0x4C_76 = a1x->axis_0x9A_154x;   // ── configure SCRATCH entity [0] ──
        Entities_EA3E4[0]->byte_0x46_70 = a1x->dword_0x10_16;        //   BUILD00 row = current level
        Entities_EA3E4[0]->id_0x1A_26   = a1x->id_0x1A_26;
        Entities_EA3E4[0]->model_0x40_64 = 0;                        //   model 0 → datum-based restore
        Entities_EA3E4[0]->dword_0x10_16 = 0;                        //   level 0 → no re-scatter in Remove
        Entities_EA3E4[0]->parentId_0x28_40 = a1x - D41A0_0.struct_0x6E8E;
        RemoveCastleStage_385C0(Entities_EA3E4[0]);      //   ── RESTORE TERRAIN for the top level (§3.3) ──
        a1x->dword_0x10_16--;                            //   ── LEVEL-- ──
        SetShiftByCastle_49EC0(a1x, a1x->dword_0x10_16); //   shrink AABB to new level
        a1x->array_0x52_82.yaw = 0xE000;  a1x->array_0x52_82.fov = 0x4000;
        sub_60810(a1x);                                  //   HP/CAP ladder for the new (lower) level (§7)
        sub_613D0(a1x);                                  //   rebuild visible piece chain (§3.2)
        sub_5F890(a1x, 1);                               //   (occupant/guard bookkeeping)
    }
    if (!a1x->dword_0x10_16)                             // ── LEVEL 0 → CASTLE DEATH ──
    {
        v8x = Entities_EA3E4[a1x->id_0x1A_26];
        if (v8x->model_0x40_64 == 1)                     //   owner is a "player 1" body
        {
            if (D41A0_0.terrain_2FECE.byte_0x2FED2 & 4) {                 // team/mode flag
                v9 = v8x->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2];  // castle spell entity
                if (v9) { DisableEntityDrawing04_57F10(Entities_EA3E4[v9]);   // remove the spell entity
                          v8x->...SpellEnabled[2] = 0; }
            }
        }
        else { sub_5F890(a1x, a1x->dword_0x10_16); }     //   occupant EJECTION bookkeeping
        v8x->dword_0xA4_164x->CastleEntityIndex_0x3A_58 = 0;   //   UNBIND owner→castle link
        DisableEntityDrawing04_57F10(a1x);               //   ── DESPAWN the castle core ──
    }
}
```

- **Per lethal hit: exactly one level down** + a **10% capacity mana haircut** realized by scattering the
  overflow via `sub_5FD00` (§5.1). Occupant/guard ejection via `sub_5F890`.
- **Terrain restore** for the removed level is done by pointing the **scratch entity [0]** at the castle
  datum with `model = 0, level(dword_0x10_16) = 0, byte_0x46_70 = currentLevel` and calling
  `RemoveCastleStage_385C0` — the `model 0` branch uses `zKoef = position.z >> 5` (datum) and, because its
  own `dword_0x10_16 == 0`, does NOT run the mana-scatter sub-loop (that already happened via `sub_5FD00`).
- **Castle death (level 0):** unbind `CastleEntityIndex_0x3A_58 = 0`, despawn the core
  (`DisableEntityDrawing04_57F10`). The mana was already scattered on the way down; the piece chain is
  freed by the final `sub_613D0` (with level now 0 → empty chain). Death sound 30 already played per level.

### 5.1 Mana scatter `sub_5FD00` (EF:61240) — VERBATIM essentials

Scatters overflow mana as **(10,39)** spheres (`IfSubtypeCallCreatingManaSphere_4A190(pos, 10, 39)`,
EF:61301). Overflow `v14 = mana - maxMana` (or full mana if level 0). Split into `v16 = clamp(mana/1000,
1, 32)` spheres, each `mana/v16`, scattered ballistically (`MoveEntity_57FA0` with `yaw = rand&0x7ff`,
speed `rand%0x1400 + 3840`), owner = castle id. This is the standard corpse/overflow scatter (survey §
corpse pipeline).

### 5.2 Castle damage intake `sub_609E0` (EF:61733) — VERBATIM

```c
// EF:61733
int sub_609E0(type_entity_0x6E8E* locEvent)//2419e0
{
    int result = 0;
    if (locEvent->life_0x8 < 0) return 2;                     // already dead → 2 (trigger downgrade)
    if (locEvent->str_0x5E_94.word_0x62_98) {                 // pending damage in mailbox
        locEvent->life_0x8 -= locEvent->str_0x5E_94.dword_0x5E_94;   // apply straight subtract
        if (locEvent->life_0x8 < 0) {                         // LETHAL
            locEvent->word_0x24_36 = locEvent->str_0x5E_94.word_0x62_98;   // record killer
            locEvent->str_0x5E_94.word_0x62_98 = 0;
            return 2;                                         //   → 2 = one-level downgrade (§5)
        }
        locEvent->str_0x5E_94.word_0x62_98 = 0;
        locEvent->str_0x5E_94.dword_0x5E_94 = 0;
        result = 1;                                           // non-lethal hit absorbed
        Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x->byte_0x195_405 = 4;  // owner "castle hit" flag
    }
    if (locEvent->str_0x5E_94.word_0x80_128 == locEvent->id_0x1A_26) {   // self-referenced occupant
        if (locEvent->dword_0x10_16 < 7)  locEvent->struct_byte_0xc_12_15.byte[0] |= 0x40u;   // request rebuild
        locEvent->str_0x5E_94.word_0x80_128 = 0;
    }
    return result;
}
```
Straight subtract (survey confirmed). Returns **2** on a lethal hit → the action handler
(`EndOfCastleProjectile_5F8F0` EF:61082, or `sub_5FCA0`) flips to `sub_605E0` for the downgrade.

---

## 6. THE CASTLE GUARD HOOK — `(5,15)` in `sub_5FF50` (EF:61488) — CONFIRMED

The intake/regen driver `sub_5FF50` (EF:61342) walks the owner's guard-slot array `array_0x5C_92[0..v19]`
(size from `sub_60400`, EF:61523: {level1/2→2 slots at base 0, level3→base 4, level4→base 6, … level7→
base 34} — the guard allowance grows with level). For each EMPTY slot, if the castle's respawn cooldown
`word_0x2C_44 == 0`, it spawns a guard. VERBATIM (EF:61483-61510):

```c
// EF:61483
v14x = Entities_EA3E4[v18x->dword_0xA4_164x->array_0x5C_92[v20]];   // current guard in slot v20
if (v14x <= Entities_EA3E4[0]) {                                    // slot EMPTY
    if (!a1x->word_0x2C_44) {                                       // respawn cooldown elapsed
        v17x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 5, 15);   // ← SPAWN (5,15) GUARD
        if (v17x) {
            a1x->word_0x2C_44 = 16;                                 // 16-tick respawn cooldown
            v15 = a1x->id_0x1A_26;
            v17x->id_0x1A_26 = v15;  v17x->playerEntityIndex_0x94_148 = v15;   // guard owner = castle owner
            v18x->dword_0xA4_164x->array_0x5C_92.at(v20) = v17x_idx;           // register in slot
            v17x->yaw_0x1C_28 = 512;  v17x->roll_0x20_32 = 512;
            predictedAxis = v17x->position;
            predictedAxis.x += 128;  predictedAxis.y += 640;                   // offset spawn point
            predictedAxis.z = getTerrainAlt_10C40(&predictedAxis);
            CopyEntityPosition_57CF0(v17x, &predictedAxis);
        }
    }
}
else if (v14x->class_0x3F_63 != 5 || v14x->model_0x40_64 != 15 || v14x->actionIndex_0x45_69 == 125) {
    v18x->dword_0xA4_164x->array_0x5C_92[v20] = 0;                  // slot occupant died/invalid → clear
    a1x->word_0x2C_44 = 16;                                         //   restart cooldown
}
```

- **Guard = `class 5, model 15`, action 9** (idle), owner-bound to the castle owner. One per allowance slot.
- **Respawn cooldown = 16 ticks** (`word_0x2C_44`), decremented each tick at the top of `sub_5FF50` (EF:61446).
- **Allowance count** per level from `sub_60400` (EF:61522): level {1,2}→2, {3}→2 (base 4), {4}→2 (base 6),
  {5}→2 (base 14), {6}→3 (base 18), {7}→3 (base 34). (The `*a2` = count, `*a3` = base-offset into a shared
  guard table.)
- **No separate balloon machinery on the castle** — `AddBallon_60AB0` (EF:61763) is a MOB/tail entity (bees/
  balloons) tracked separately, not spawned by the castle. The castle's only militia is the (5,15) guard set.

---

## 7. CONSOLIDATED CONSTANTS + PORT NOTES

### 7.1 HP / CAP ladder `sub_60810` (EF:61695) — VERBATIM (8 levels)

```c
// EF:61695  number1 = (owner->word_0x24A_586 * ((owner->array_0x24E_590[level]<<8)+256)) >> 8   (Life-scaled)
switch (level) {  // sub_60780(castle, spellEnt, maxLife, maxMana)
  case 0: sub_60780(e, e2, 0,                    5000);       break;   // level 0: HP 0,   cap 5000
  case 1: sub_60780(e, e2, 20000*number1 >> 8,   8500);       break;
  case 2: sub_60780(e, e2, 40000*number1 >> 8,  18000);       break;
  case 3: sub_60780(e, e2, 40000*number1 >> 8,  38800);       break;
  case 4: sub_60780(e, e2, 60000*number1 >> 8,  78600);       break;
  case 5: sub_60780(e, e2, 60000*number1 >> 8, 158200);       break;
  case 6: sub_60780(e, e2, 80000*number1 >> 8, 317400);       break;
  case 7: sub_60780(e, e2, 80000*number1 >> 8, 300000000);    break;   // level 7: cap "infinite"
}
```
`sub_60780` (EF:61670): sets `maxLife = number1`, preserving negative-life haircut (`life = maxLife -
min(-life, maxLife/2)`), and `maxMana = number2`. HP is **Life-personality scaled** (`number1` factor);
CAP is flat. Matches survey: HP {—,20000,40000,40000,60000,60000,80000,80000}, CAP {5000,8500,18000,
38800,78600,158200,317400,∞}.

### 7.2 Constants table

| constant | value | meaning | cite |
|---|---|---|---|
| castle core | class 3, model 2 | the owned castle entity | EF:33362,:6833,:43779 |
| castle core action | 5 | steady-state | EF:61583,:33376 |
| authored core HP | 40000 | `sub_4AA40` maxLife | EF:33379 |
| authored core byte_0x38_56 | 33 | damageable flags | EF:33381 |
| creation seed | class 10, model 32 (0x20) | build-order entity | EF:36300 |
| seed action | 0x22 (34) → BeginOfCastleCreation | | EF:36298 |
| seed default row | byte_0x46_70 = 2 | par1 overrides at trigger | EF:36303,:33200 |
| painter | class 10, model 42 (0x2A), action 0x2C | geometry paint | EF:36739,:61188 |
| leveler | class 10, model 41 (0x29) | ground flatten | EF:61208 |
| stage piece | class 10, model 79 (0x4F) | visible walls/towers | EF:62303 |
| guard | class 5, model 15, action 9 | castle militia | EF:61488,:61390 |
| mana carrier (scatter) | class 10, model 39 (0x27) | overflow/downgrade mana | EF:61301 |
| anchor (unrelated) | class 10, model 52 (0x34), sprite 205 | standalone building | EF:36777 |
| **level field** | `dword_0x10_16` (**f26**) | 0..7 | EF:61581 |
| **build sub-state** | `word_0x2E_46` (**f59**) | 0/1/2/3/4/5/6 | EF:61125 |
| **capacity** | `maxMana_0x8C_140` (**f136**) | per-level | EF:61691 |
| **build datum / site** | `axis_0x9A_154x` (**site_z**) | spawn anchor | EF:61574,:6833 |
| BUILD00 row index | `byte_0x46_70` | usually == level | EF:27070 |
| owner→castle link | `dword_0xA4_164x->CastleEntityIndex_0x3A_58` | | EF:6837,:43785 |
| castle→owner link | `id_0x1A_26` = owner id | | EF:6836,:43784 |
| piece chain root | `word_0x34_52` (castle), `word_0x32_50` (piece→castle) | | EF:62308 |
| AABB extent (per level) | `pitch=((W<<8)+1280)>>1`, `roll=((H<<8)+1280)>>1` | `SetShiftByCastle` | EF:32890 |
| upgrade sound | 10 | | EF:61578 |
| downgrade/build sound | 30 | | EF:61627,:43786 |
| **XP on build** | `sub_6D8B0(owner, 2, 1)` = +1, effect id **2** | | EF:61596 |
| downgrade mana haircut | 10% of maxMana, scattered | | EF:61622 |
| lethal intake return | 2 → `sub_605E0` | | EF:61738,:61746 |
| guard respawn cooldown | `word_0x2C_44 = 16` ticks | | EF:61491 |
| guard slots per level | {1,2}:2 {3}:2 {4}:2 {5}:2 {6}:3 {7}:3 | `sub_60400` | EF:61530 |
| BUILD00 file index | 8 | `filearrayindex_BUILD00DATTAB` | Basic.cpp:152 |
| BUILD00 cell data | 2 bytes/cell: [1]=height, [0]=sprite/angle; 0xff=skip | | EF:27103,:28104 |
| terrain retile helper | `sub_462A0` = **mc2_retile_region** (ported) | | EF:27111 |
| terrain build helper | `AddBuildingToTerrain_46570` = **mc2_add_building_region** (ported) | | EF:28144 |
| shading finalize | `SetHeightmapByBuildingArea_48B50(x,y,H,W)` | | EF:28171 |
| corner-mean sampler | `GetTerrainHeightFromSquare_48DF0` (4-corner mean) | | EF:28093 |

### 7.3 Field mapping (retail → project class-3 castle convention)

| project field | retail field | note |
|---|---|---|
| `f26` (level) | `dword_0x10_16` | 0..7; ++ on upgrade, -- on lethal hit |
| `f59` (build sub-state) | `word_0x2E_46` | MC1 used +48 offset; MC2 raw 0/2/3/4/5/6 |
| `f136` (capacity) | `maxMana_0x8C_140` | set by `sub_60810` ladder |
| `site_z` (build datum) | `axis_0x9A_154x` | MC2 uses full 3-axis; `z>>5` = height datum |
| (HP) | `maxLife_0x4`/`life_0x8` | Life-scaled ladder |
| owner id | `id_0x1A_26` | == owner wizard id |
| owner→castle | `CastleEntityIndex_0x3A_58` | on wizard player struct |

---

## 8. MC1-vs-MC2 DELTA LIST

**SHARED (do NOT re-port):**
- Castle core class/model = **3,2** (identical).
- Build-state field layout (`f26`/`f59`/`f136`/`site_z`) — identical semantics.
- The creator PAIR model numbers: **(10,42) painter + (10,41) leveler** are the same m42/m41 pair MC1 uses.
- Terrain helpers `sub_462A0` (mc2_retile_region) and `AddBuildingToTerrain_46570` (mc2_add_building_region)
  — already ported, shared.
- BUILD00 table format (2 bytes/cell, `width_4`/`height_5`/`data`), file index 8.
- Downgrade shape: one level per lethal hit + occupant ejection + terrain restore + mana scatter.
- Intake = straight subtract (`sub_609E0`), lethal → downgrade.

**MUST SWAP per game (MC2 column):**
1. **HP/CAP ladder values** — MC2 `sub_60810`: HP {—,20000,40000,40000,60000,60000,80000,80000},
   CAP {5000,8500,18000,38800,78600,158200,317400,∞}; **Life-personality-scaled HP** (`number1` factor).
   The project's MC1 `CASTLE_HP`/`CASTLE_CAP` differ (MC1 has no Life scaling on castle HP).
2. **AABB extent formula** `SetShiftByCastle_49EC0`: `((dim<<8)+1280)>>1` from the BUILD00 row (MC2-specific
   padding of 1280). MC1's `SetShiftByCastle` counterpart differs — verify.
3. **Visible stage pieces = (10,79)** driven by the `array_0x24E_590[9+level]` part table + `x_BYTE_DB038`
   offset table. MC1's castle uses a different piece scheme (no (10,79) chain). **This is the biggest visual
   delta** — the project currently shows no MC2-native pieces.
4. **Guard = (5,15)** with the `array_0x5C_92` slot array and `sub_60400` per-level allowance
   {2,2,2,2,2,3,3}. Confirm MC1's guard model/allowance and swap.
5. **+1 castle XP on build** via `sub_6D8B0(owner, 2, 1)` — MC2-only (part of the XP system); MC1 has no
   castle XP.
6. **Build state machine** `BeginOfCastleCreation_5FA70` (word_0x2E_46 cases) — MC2's specific painter/
   leveler oscillation (state 3↔4↔5). Diff against `mc1/features.rs::castle_tick`.
7. **Level-authoring source** = `player_0x2FED9[color]` (map header wizard record) → stamp N passes at load.
   Confirm MC1's authored-castle field.
8. **Mana carrier conversion mode** (`RemoveCastleStage` `fontTypeIndex != 0` → (10,45)) — verify presence
   in MC1.

---

## 9. OPEN items

- **`sub_11960` (house pre-clear) and `sub_11A10` (space check)** transcribed only by name here; both need a
  verbatim pass before porting the state-0 arm of `BeginOfCastleCreation` (their exact footprint/space
  predicate matters for the "no room" abort).
- **`x_BYTE_DB038` table contents** (the (10,79) piece offset/count table) not dumped — the per-level part
  count is `DB038[2*level]`, offset-list index `DB038[1+2*level]`, base at `DB038[18]`, 2 bytes/(x,y) per
  part. Dump it from the retail data when baking piece geometry.
- **`array_0x24E_590` layout** (the owner's per-level build/part table; `[9+level]` = part id, `[level]` =
  the `sub_60810` HP scale factor) — needs a field map; it lives on the wizard's `dword_0xA4_164x` player
  struct, populated at load.
- **`sub_5F660` (re-cast upgrade router, EF:60874)** transcribed partially (model dispatch); the full
  case table (which target models accept the Create-Castle re-cast) should be finished before wiring the
  upgrade trigger.
- **`sub_5F890` occupant/guard bookkeeping** (EF:61028) — confirmed to touch `SpellEnabled[2]`'s entity's
  `word_0x2E_46`; exact ejection semantics on downgrade need a golden check.
- **Action-index → handler map** (e.g. seed action 0x22 = 34 → `BeginOfCastleCreation` at EV case 0x240a70)
  assumed from the EV dispatch by address; confirm the action-index table binds 0x22 to 0x240a70 at load.
