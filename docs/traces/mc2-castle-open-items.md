# MC2 CASTLE — §9 OPEN-items closure (port-ready verbatim)

Closes the `docs/traces/mc2-castle-builder.md` §9 OPEN list and the `mc2-castle-runtime.md` §5 OPEN
tail with VERBATIM transcriptions and exact line citations. Read the two castle traces plus
`docs/traces/mc2-class10-m67-flood-helpers.md` (house style, uint8-wrap, RNG law `r = 9377*r + 9439`)
first. Trace date 2026-07-11.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
EF = `engine/EventsFunctions.cpp`, EV = `engine/Events.cpp`.

---

## Headline findings (read first — two are CORRECTIONS to the builder trace)

1. **CORRECTION to builder §1.4 and §9:** the class-10 model-32 (0x20) "castle-creation seed" does **NOT**
   run `BeginOfCastleCreation`. Its action `0x22` resolves through the **class-10** action table
   `x_DWORD_D4C52ar_strA0` (not the class-3 `str30`), and `strA0` action `0x22 = 0x2154A0 = sub_344A0`
   (EF:25052) — a **moving projectile** that decrements life, sheds (10,11) sparkle trails, and despawns.
   `0x240A70` (BeginOfCastleCreation) is a **class-3** action-5 address; a class-10 entity never dispatches
   into the class-3 table. So the seed is a spell SHOT, not the build driver. (§4)

2. **CORRECTION to builder §3.2 / runtime §4c:** the (10,79)=0x4F stage piece spawned by
   `sub_613D0`→`IfSubtypeCallCreatingManaSphere_4A190(pos,10,79)` gets its ctor from the class-10 **ctor**
   table `x_DWORD_D4C52ar_strA1`, whose model-0x4F entry is `0x2318E0 = sub_508E0_castle_defend_create`
   (EF:36987) → **action 0x56** → tick handler **`sub_3AF00_castle_defend_event`** (EF:30106, `strA0`
   action 0x56 = 0x21BF00). It is **NOT** `sub_3A5B0` (action 0x4F, EF:29590) — that handler belongs to
   **model 0x48 (72)** (a bee/meteor swarm piece, ctor `sub_51800` EF:37459), which merely shares the
   numeric action id 0x4F. So the castle "stage piece" is the **defender/townsfolk launcher**
   `sub_3AF00`, which lofts class-3/class-5 defenders out of the castle. (§5)

3. **`sub_11A10` (space check) is a footprint SCAN over the level+1 (next) BUILD00 footprint**, split into
   four bands (top/bottom/left/right rings) each tested cell-by-cell with `sub_11C80`, PLUS a class-10
   model-2 OBJECT overlap test. It aborts (returns 0) the moment ANY cell has `mapAngle & 0x80` (built/
   blocked) or, on cave levels, `mapAngle & 8` (ceiling-blocked). It always restores the AABB to the
   current level on exit. (§1)

4. **`sub_11960` (house pre-clear) KILLS overlapping BUILDINGS** (`dword_38527` = the class-10 MODEL-45 list, builder EF:40043-51 — NOT an "effect list"; corrected 2026-08-11) whose AABB
   overlaps the level+1 footprint (sets `life = -1`, `fontTypeIndex = 0`). It does NOT touch terrain and
   does NOT clear objects — it only nukes effect entities in the way. (§1)

5. **`sub_5F890(castle, a2)` is the build-anim GHOST sync**, not occupant/guard bookkeeping. It pokes the
   owner's `SpellEnabled[2]` entity (the Create-Castle spell-widget entity): `a2 != 0` → set its
   `word_0x2E_46 = word_0x30_48 - 1`; `a2 == 0` → set `word_0x2E_46 = 0` and call `sub_6D880(castle)`.
   The `a1x->dword_0x10_16` passed at the death site (EF:61662, `sub_5F890(a1x, a1x->dword_0x10_16)`) is
   just "nonzero" ⇒ the `a2 != 0` arm. (§2)

6. **`sub_5F660` (re-cast router) does NOT itself call `sub_60480`.** For an existing castle (model 2 =
   `case 2`) it plays the "can't upgrade yet / not enough mana" sound and, on success, calls `sub_5F7B0`
   which sets the castle's `word_0x2E_46 = word_0x30_48` — arming the **build state machine**; the actual
   level-up (`sub_60480`) fires on the next castle tick when `BeginOfCastleCreation` reaches `case 0`. The
   third arg `a3` (0 at EF:6826) is an OR-mask written into the target's `struct_byte_0xc_12_15.dword`
   (the spell-color/binding bits); it is forced to 0 when the CASTER is model 1. (§3)

---

## 1. `sub_11960` (pre-clear) + `sub_11A10` (space check) + `sub_88D00` (no-room UI) — VERBATIM

These run in `BeginOfCastleCreation_5FA70` **case 0** (EF:61128-61143):
```c
// EF:61127
case 0:
    sub_11960(locEvent);                                    // (§1.1) clear effect entities in the way
    if (!locEvent->dword_0x10_16 || sub_11A10(locEvent))    // level 0 skips the space test; else must PASS
    {
        // ... apply owner colour, then:
        sub_60480(locEvent);                                // LEVEL-UP (builder §4)
    }
    else
    {
        locEvent->word_0x2E_46 = 2;                         // → state 2 (abort/steady)
        locEvent->struct_byte_0xc_12_15.byte[0] &= 0xBF;    // clear bit6 (downgrade-armed)
        sub_88D00();                                        // "no room" help popup
    }
    break;
```

### 1.1 `sub_11960` (EF:4391) — house PRE-CLEAR — VERBATIM
```c
// EF:4391
void sub_11960(type_entity_0x6E8E* a1)//1f2960
{
    __int16 v1; int v5, v6; __int16 v4; type_entity_0x6E8E* v2x;

    SetShiftByCastle_49EC0(a1, a1->dword_0x10_16 + 1);      // set AABB to the NEXT level's footprint
    v6 = a1->array_0x52_82.pitch;                           // half-extent X (next level)
    v1 = a1->position_0x4C_76.x;
    v5 = a1->array_0x52_82.roll;                            // half-extent Y (next level)
    v2x = x_D41A0_BYTEARRAY_4_struct.dword_38527;           // the (10,45) BUILDING list
    v4 = a1->position_0x4C_76.y;
    while (v2x > Entities_EA3E4[0])
    {
        if (abs((signed __int16)(v2x->position_0x4C_76.x - v1)) <= v2x->array_0x52_82.pitch + v6
            && abs((signed __int16)(v2x->position_0x4C_76.y - v4)) <= v2x->array_0x52_82.roll + v5)
        {
            v2x->life_0x8 = -1;                             // KILL the overlapping effect
            v2x->fontTypeIndex_0x3D_61 = 0;
        }
        v2x = v2x->next_0;
    }
    SetShiftByCastle_49EC0(a1, a1->dword_0x10_16);          // restore AABB to CURRENT level
}
```
**Exact semantics:**
- **What it clears:** ONLY entities on the **(10,45) BUILDING list `dword_38527`** whose AABB (XY Minkowski-sum of
  half-extents, `|dx| <= pitchA+pitchB && |dy| <= rollA+rollB`) overlaps the castle's **level+1** footprint.
  Overlap ⇒ `life = -1` (queues despawn) and `fontTypeIndex_0x3D_61 = 0`.
- **NOT touched:** terrain (`mapHeightmap`/`mapAngle`), the OBJECT list `dword_38519`, guards, mana. It is
  purely "get any spell-effects out of the way before we stamp the next ring."
- **AABB juggling:** it temporarily sets the AABB via `SetShiftByCastle_49EC0(a1, level+1)`, uses
  `array_0x52_82.pitch/.roll` (the `((dim<<8)+1280)>>1` half-extents, builder §4/runtime §2), then restores
  it to the current level on exit. Note the comparison uses `<=` (inclusive), unlike the flood's strict `<`.

### 1.2 `sub_11A10` (EF:4421) — SPACE CHECK — VERBATIM
```c
// EF:4421
char sub_11A10(type_entity_0x6E8E* a1)//1f2a10
{
    uaxis_2d v3, v6, v8, v11; __int16 v4,v5,v7,v10, v12, v18,v19,v21,v22, j,k,l;
    int v9,v13,v17; unsigned __int16 v20; type_entity_0x6E8E* ix;

    v18 = a1->array_0x52_82.pitch >> 8;                     // CURRENT level half-extent (tiles)  [inner]
    v21 = a1->array_0x52_82.roll  >> 8;
    SetShiftByCastle_49EC0(a1, a1->dword_0x10_16 + 1);      // AABB → NEXT level footprint  [outer]

    // (A) OBJECT-overlap gate: any class-10 model-2 object (other than self) overlapping ⇒ NO ROOM
    for (ix = x_D41A0_BYTEARRAY_4_struct.dword_38519; ix > Entities_EA3E4[0]; ix = ix->next_0)
    {
        if (ix->model_0x40_64 == 2 && ix != a1 && sub_106C0(ix, a1))
        {
            SetShiftByCastle_49EC0(a1, a1->dword_0x10_16);  //   restore & fail
            return 0;
        }
    }

    v19 = a1->array_0x52_82.pitch >> 8;                     // NEXT level half-extent (tiles)  [outer]
    v12 = a1->array_0x52_82.roll  >> 8;
    LOBYTE(v20) = ((unsigned __int16)(a1->position_0x4C_76.x + 128) >> 8) - v19;   // outer origin X (tile)
    HIBYTE(v20) = ((unsigned __int16)(a1->position_0x4C_76.y + 128) >> 8) - v12;   // outer origin Y (tile)
    LOWORD(v17) = v19 - v18;                                // outer-minus-inner X margin (ring width)
    v3.word = v20;
    v22 = v12 - v21;                                        // outer-minus-inner Y margin (ring height)

    // (B) TOP band: v22 rows of full width (2*v19), starting at outer origin
    v4 = v22;
    while (v4) { for (j = 2 * v19; j; j--) { if (!sub_11C80(v3)) { SetShiftByCastle_49EC0(a1, a1->dword_0x10_16); return 0; } v3._axis_2d.x++; }
                v4--; v3.word = __PAIR__(v3._axis_2d.y, (unsigned __int8)v20) + 256; }   // next row, reset X

    // (C) BOTTOM band: v22 rows of full width, at the far (bottom) side
    LOBYTE(v6) = v20;  v5 = v22;  HIBYTE(v6) = 2 * v12 + HIBYTE(v20) - v22;
    while (v5) { for (k = 2 * v19; k; k--) { if (!sub_11C80(v6)) { SetShiftByCastle_49EC0(a1, a1->dword_0x10_16); return 0; } v6._axis_2d.x++; }
                v5--; v6.word = __PAIR__(v6._axis_2d.y, (unsigned __int8)v20) + 256; }

    // (D) LEFT band: (2*v12 rows) × v17 cols, at the left edge (the inner rows the top/bottom missed)
    LOBYTE(v8) = v20;  v7 = v22;  HIBYTE(v8) = v22 + HIBYTE(v20);
    while (v7) { v9 = v17; while (1) { v13 = v9; if (!(x_WORD)v9) break;
                    if (!sub_11C80(v8)) { SetShiftByCastle_49EC0(a1, a1->dword_0x10_16); return 0; } v9 = v13 - 1; v8._axis_2d.x++; }
                v7--; v8.word = __PAIR__(v8._axis_2d.y, (unsigned __int8)v20) + 256; }

    // (E) RIGHT band: symmetric, starting at (inner-right edge) = outer_origin_x + v19 + v20 - v17
    LOBYTE(v11) = v19 + v20 - v17;  v10 = v22;  HIBYTE(v11) = v22 + HIBYTE(v20);
    while (v10) { for (l = v17; l; l--) { if (!sub_11C80(v11)) { SetShiftByCastle_49EC0(a1, a1->dword_0x10_16); return 0; } v11._axis_2d.x++; }
                 v10--; v11.word = __PAIR__(v11._axis_2d.y, (unsigned __int8)v20) + 256; }

    SetShiftByCastle_49EC0(a1, a1->dword_0x10_16);          // restore AABB & PASS
    return 1;
}
```
**Exact semantics:**
- **The predicate is a per-cell TERRAIN-FLAG test plus an object test.** It scans the **RING** between the
  current-level footprint (inner, half-extents `v18/v21`) and the next-level footprint (outer, `v19/v12`) —
  i.e. only the newly-added border cells, not the whole footprint — in four bands (top `v22` rows, bottom
  `v22` rows, then left/right `v17`-wide columns of the middle `2*v12` rows). Each cell → `sub_11C80`.
- **`sub_11C80` (EF:4543) is the CELL predicate:**
  ```c
  // EF:4543
  char sub_11C80(uaxis_2d a1)//1f2c80
  {
      char result = 1;
      char v2 = mapAngle_13B4E0[a1.word];
      if (v2 < 0 || isCaveLevel_D41B6 && v2 & 8)   // bit7 (built/blocked) set, OR cave & bit3 (ceiling)
          result = 0;                              // → BLOCKED
      return result;
  }
  ```
  So a cell is FREE iff `mapAngle_13B4E0[cell]` has **bit7 clear** (`v2 >= 0`, i.e. not already built/
  reserved) AND, on cave levels, **bit3 (0x08) clear** (no ceiling overlap). Any blocked cell → return 0.
- **(A) OBJECT gate:** before the terrain scan, any **class-10 model-2 object** (`dword_38519` list) other
  than self overlapping the next-level box (`sub_106C0` = the ±0 AABB overlap, EF near 4557) ⇒ return 0.
  (This is why you can't upgrade a castle onto another building/object.)
- **Return:** 1 = room to grow, 0 = no room (any blocked ring cell or overlapping object). On EVERY exit it
  restores the AABB to the current level (`SetShiftByCastle_49EC0(a1, dword_0x10_16)`).
- **uint8 wrap:** the tile indices are packed as `uaxis_2d` (`.x`/`.y` bytes into one word); the
  `__PAIR__(y, (uint8)v20) + 256` row-advance and `(unsigned __int8)(...)` origin math are the intended
  256×256 toroidal addressing (same convention as the flood/dome samplers).

### 1.3 `sub_88D00` (EF:49261) — the "no room" feedback — VERBATIM
```c
// EF:49261
void sub_88D00()//269d00
{
    if (x_D41A0_BYTEARRAY_4_struct.showHelp_10)          // only if help/hints enabled
        str_unk_1804B0ar.word_0x88 = 93;                 // queue help-string 93 ("no room to build")
}
```
**Exact semantics:** pure UI. If the help system is on, it sets `str_unk_1804B0ar.word_0x88 = 93` (the
help-text index). No gameplay effect. **Port as a UI toast; safe to no-op if hints are off.**

---

## 2. `sub_5F890` (EF:61029) — build-anim GHOST sync (the "SpellEnabled[2] ghost") — VERBATIM
```c
// EF:61029
void sub_5F890(type_entity_0x6E8E* a1x, __int16 a2)//240890
{
    type_entity_0x6E8E* v2x; __int16 v3;

    // v3 = the OWNER wizard's Create-Castle spell-widget entity index (SpellEnabled[2])
    v3 = Entities_EA3E4[a1x->id_0x1A_26]->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2];
    if (v3)
    {
        if (a2)                                          // a2 != 0  → RUNNING-build arm
        {
            v2x = Entities_EA3E4[v3];
            v2x->word_0x2E_46 = v2x->word_0x30_48 - 1;   //   ghost sub-state = (ghost timer) - 1
        }
        else                                             // a2 == 0  → FINISH/RESET arm
        {
            Entities_EA3E4[v3]->word_0x2E_46 = 0;        //   ghost sub-state = 0
            sub_6D880(a1x);                              //   spellbook/HUD refresh for the castle owner
        }
    }
}
```
**Exact semantics:**
- **NOT occupant/guard bookkeeping.** `a1x->id_0x1A_26` = the castle's owner-wizard id; `SpellEnabled[2]`
  on that wizard's player struct = the **Create-Castle spell-widget ENTITY** (the little animated spell
  icon/ghost the HUD shows while a castle is building). `v3` is that entity's index; if the owner has no
  such widget (v3 == 0) the call is a no-op.
- **`a2` (second arg) meaning:** it is a **boolean "build in progress"** flag, not a count.
  - `a2 != 0` (build states 3/5, downgrade `sub_605E0` EF:61643, the projectile-animation branch of the
    standing tick EF:61076, and the death site EF:61662 which passes `dword_0x10_16` = "nonzero"): sets the
    ghost's `word_0x2E_46 = word_0x30_48 - 1` — i.e. slaves the ghost's displayed sub-state to its own
    animation timer, keeping the on-screen build animation in step.
  - `a2 == 0` (build ABORT state 2 EF:61151, EV-driven idle `sub_5F890(...,0)` EV:2934, and the (10,45)/
    misc sites EF:28234/28245/58542/58588): zeroes the ghost's `word_0x2E_46` and calls `sub_6D880` (the
    owner spellbook/HUD refresh) — i.e. "build finished/aborted, reset the widget."
- **What it touches:** ONLY the SpellEnabled[2] widget entity's `word_0x2E_46`, plus (a2==0) the
  `sub_6D880` HUD refresh. It does **not** eject occupants, does **not** spawn/remove guards, does **not**
  touch mana. (The guard roster is `sub_5FF50`/`array_0x5C_92` per runtime §4c; occupant ejection on death
  is elsewhere.) **CORRECTION to builder §9 / runtime notes: `sub_5F890` is the build-anim ghost sync.**
- **The death-site call `sub_5F890(a1x, a1x->dword_0x10_16)` (EF:61662)** passes the (still-positive)
  level as the boolean — so it takes the `a2 != 0` arm exactly like every other build-in-progress call.

---

## 3. `sub_5F660` (EF:60875) — the re-cast / spell-attach ROUTER — VERBATIM
```c
// EF:60875
char sub_5F660(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x, int a3)//240660
{
    int v3 = a3;                                          // a3 = OR-mask into target's byte struct (see below)
    char v5 = 0, v6 = 0, v7 = 0;                          // v5=caster-is-model-1, v6=not-enough, v7=success
    if (a2x > Entities_EA3E4[0])
    {
        if (a1x->model_0x40_64 == 1) { v5 = 1; v3 = 0; }  // caster model 1 (human wizard body) → force a3=0
        switch (a2x->model_0x40_64)                       // dispatch on the TARGET entity's MODEL
        {
        case 0:                                           // target model 0
            if (a2x->byte_0x46_70 < 2) break;             //   build-row < 2 → fall to mana branch
            goto LABEL_16;                                //   else → "already active?" gate
        case 1:                                           // target model 1
            if (a2x->word_0x2E_46 <= 0) break;
            a2x->byte_0x3C_60 = 1;
            a1x->struct_byte_0xc_12_15.byte[1] &= 0xFCu;  //   clear caster low-2 bits of byte[1]
            a1x->struct_byte_0xc_12_15.dword |= v3;       //   OR the a3 mask into caster's flags
            sub_5F7E0(a2x, a1x);
            v7 = 1;
            goto LABEL_23;
        case 2:                                           // ← target model 2 = THE CASTLE (re-cast Create-Castle)
            if (a2x->word_0x2E_46 <= 0) break;            //   castle mid-build (state>0) → fall to mana branch
            if (!v5)                                      //   caster not model-1 → play "busy" sound 29
                PrepareEventSound_6E450(0, a1x->dword_0xA4_164x->playerColorIndex_0x38_56, 29);
            goto LABEL_23;                                //   return v7=0 (no attach; build already running)
        case 4: case 6: case 8: case 0xB: case 0xC: case 0xE:
            if (a1x->model_0x40_64) goto LABEL_16;        //   non-body caster → active gate
            if (a2x->word_0x2E_46 <= 0) break;
            a2x->word_0x2E_46 = (a2x->model_0x40_64 == 4) ? 7 : 1;   // arm the target's SM
            goto LABEL_23;
        case 7:
            if (a2x->byte_0x46_70 < 1 || !a2x->word_0x2E_46) break;
            goto LABEL_23;
        case 9: case 0xA: case 0xD: case 0xF: case 0x10: case 0x11:
        case 0x12: case 0x13: case 0x14: case 0x15: case 0x16: case 0x17: case 0x18:
        LABEL_16:
            if (a2x->word_0x2E_46) goto LABEL_23;         //   already active → return v7 (0) without attach
            break;
        default:
            break;
        }
        // ---- mana branch (reached by the `break`s above) ----
        if (a1x->mana_0x90_144 < a2x->maxMana_0x8C_140)   // caster mana below target's capacity?
        {
            v6 = 1;                                        //   → NOT ENOUGH (play "can't" feedback below)
        }
        else
        {
            sub_5F7B0(a2x, a1x, v3);                       //   → ATTACH / ARM the target's build SM (§3.1)
            v7 = 1;
        }
    }
LABEL_23:
    if (v6 && !v5)                                        // not-enough AND caster isn't a model-1 body:
    {
        sub_88B60();                                      //   UI "not enough mana" popup
        PrepareEventSound_6E450(0, a1x->dword_0xA4_164x->playerColorIndex_0x38_56, 29);  // sound 29
    }
    return v7;                                            // 1 = attached/armed, 0 = busy or refused
}
```

### 3.1 `sub_5F7B0` (EF:60974) — the ATTACH that arms the build — VERBATIM
```c
// EF:60974
void sub_5F7B0(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x, int a3)//2407b0
{
    a1x->word_0x2E_46 = a1x->word_0x30_48;               // TARGET(castle).word_0x2E_46 = its word_0x30_48
    a2x->struct_byte_0xc_12_15.byte[1] &= 0xFCu;         // clear caster low-2 bits of byte[1]
    a2x->struct_byte_0xc_12_15.dword |= a3;              // OR the a3 mask into the CASTER's flags
    sub_5F7E0(a1x, a2x);
}
// EF:60983  sub_5F7E0: clears the target's byte[0] bit5 (0xDF) unless byte_0x1BF_447 >= 2
void sub_5F7E0(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x)//2407e0
{
    unsigned __int8 result = a2x->dword_0xA4_164x->byte_0x1BF_447;
    if (result < 2u || result <= 2u && a1x->model_0x40_64 != 1)
        a2x->struct_byte_0xc_12_15.byte[0] &= 0xDFu;     // clear bit5 (draw/hold)
}
```

**Exact semantics for the port:**
- **The CASTLE case is `case 2`** (target model 2). On a re-cast of Create-Castle onto an existing castle:
  - if the castle's `word_0x2E_46 > 0` (a build/repaint is already in progress) → play "busy" sound 29 (if
    caster isn't a model-1 body) and `goto LABEL_23` returning **v7 = 0** (refused; no level-up queued).
  - else (`word_0x2E_46 <= 0`, castle idle) → fall through to the **mana branch**: if caster mana <
    castle `maxMana_0x8C_140` → `v6 = 1` (not-enough popup + sound 29, return 0); else call `sub_5F7B0`
    which sets **`castle->word_0x2E_46 = castle->word_0x30_48`** and returns **v7 = 1**.
- **So `sub_5F660` NEVER calls `sub_60480` directly.** The upgrade is a two-step: (1) `sub_5F660`/`sub_5F7B0`
  arms `word_0x2E_46`; (2) on a later castle tick the standing handler routes to action 5 →
  `BeginOfCastleCreation_5FA70`, whose **`case 0`** calls `sub_60480` (the real level-up: `dword_0x10_16++`,
  painter spawn, HP/CAP ladder `sub_60810`, piece rebuild `sub_613D0`, +1 castle XP). See builder §4.
- **The third arg `a3` (0 at the EF:6826 Create-Castle call site):** it is an **OR-mask written into a
  `struct_byte_0xc_12_15.dword` field** — the caster's flags in `case 1`/`sub_5F7B0`, i.e. the spell
  hand/colour binding bits. At the Create-Castle site it is **0** (no extra bits), and it is **forced to 0
  anyway whenever the caster is model 1** (`if (a1x->model_0x40_64 == 1) v3 = 0`, EF:60888). Contrast the
  other `sub_5F660` sites at EF:60852/60855 which pass `256`/`512` (the left/right spell-hand mask bits) —
  those select which hand's flag bit gets ORed. For the castle re-cast, `a3 = 0` selects **no hand-bind
  mask** (the castle attach doesn't need the hand-flag; it just arms `word_0x2E_46`).
- **Model dispatch summary (which target models accept the router):** model 0 (row≥2), model 1, **model 2 =
  castle**, models {4,6,8,0xB,0xC,0xE} (arm SM to 7 or 1), model 7, and the `LABEL_16` "active-gate" band
  {9,0xA,0xD,0xF,0x10..0x18}. All others hit `default` → straight to the mana branch (attach if mana
  suffices). **The castle upgrade path is: model 2, castle idle, mana ≥ cap → `sub_5F7B0` arms the build.**

---

## 4. Action-index → handler bindings — CONFIRMED (with the two corrections)

### 4.1 Table selection (EF:2064-2071) — VERBATIM
```c
// EF:2064 (the per-class {action-table, ctor-table} pair array)
0x002A5C44,0x00000003,0x0000, x_DWORD_D4C52ar_str30, x_DWORD_D4C52ar_str31,   //class 3
0x002A5C44,0x00000005,0x0000, x_DWORD_D4C52ar_str50, x_DWORD_D4C52ar_str51,   //class 5
0x002A5C44,0x0000000A,0x0000, x_DWORD_D4C52ar_strA0, x_DWORD_D4C52ar_strA1,   //class 10
```
For each class the **first** table (`strN0`) is the **ACTION→handler** map (addresses `0x21xxxx`), the
**second** (`strN1`) is the **model→CTOR** map (addresses `0x22F/0x230/0x231xxx`). A class-N entity's
`actionIndex` indexes `strN0`; its `model` (at spawn) indexes `strN1`. **A class-10 entity therefore NEVER
resolves an action through the class-3 `str30` table.**

### 4.2 Class-3 action table `x_DWORD_D4C52ar_str30` (EF:1201) — VERBATIM rows
```c
// EF:1201  {tag, actionIndex, handlerAddress, enabled}
0x002A5C44,0x0004,0x002408F0,0x00000001,   // action 4 → 0x2408F0 = EndOfCastleProjectile_5F8F0 (standing tick)
0x002A5C44,0x0005,0x00240A70,0x00000001,   // action 5 → 0x240A70 = BeginOfCastleCreation_5FA70   (build SM)
0x002A5C44,0x0006,0x00240CA0,0x00000001,   // action 6 → 0x240CA0 = sub_5FCA0_destroy_castle_level (downgrade)
```
**CONFIRMED:** class-3 action 4 → 0x2408F0, 5 → 0x240A70, 6 → 0x240CA0 (matches runtime §1/§6). The EV
address dispatch has the matching cases `0x2408f0`/`0x240a70`/`0x240ca0` at **EV:2937/2941/2954**.

### 4.3 Class-10 seed action 0x22 — CORRECTED
The model-32 (0x20) seed ctor `sub_4FA60` (EF:36292) sets `actionIndex = 0x22`, `class = 0xA`. In the
**class-10** action table `strA0` (EF:1636):
```c
// EF:1636
0x002A5C44,0x0022,0x002154A0,0x00000001,   // class-10 action 0x22 → 0x2154A0 = sub_344A0
```
`sub_344A0` (EF:25052) is **a moving spell SHOT**, VERBATIM:
```c
// EF:25052
void sub_344A0(type_entity_0x6E8E* a1x)//2154a0
{
    int v1 = a1x->life_0x8;
    a1x->life_0x8 = v1 - 1;
    if (v1 < 0 || sub_104A0(&a1x->position_0x4C_76) & 1)   // life expired OR hit something → despawn
    { DisableEntityDrawing04_57F10(a1x); return; }
    type_entity_0x6E8E* v3x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 11); // trail
    if (v3x)
    {
        v3x->array_0x52_82.fov = a1x->array_0x52_82.fov;
        v3x->id_0x1A_26        = a1x->id_0x1A_26;
        v3x->life_0x8          = a1x->byte_0x46_70;
    }
    MoveEntity_57FA0(&a1x->position_0x4C_76, a1x->yaw_0x1C_28, 0, a1x->actSpeed_0x82_130);  // fly forward
}
```
**CORRECTION to builder §1.4/§9:** action 0x22 does **NOT** run `BeginOfCastleCreation`. The seed is a
projectile that flies (`MoveEntity`, speed `actSpeed_0x82_130 = 256` from the ctor), sheds (10,11) sparkle
trails carrying `byte_0x46_70` (the par1 build-row) as their life, and despawns on life-expiry or on
terrain contact (`sub_104A0 & 1`). The `0x240A70` address the builder trace named is the class-3 action-5
slot; a class-10 seed cannot reach it. **OPEN:** how the seed's impact turns into an actual `(3,2)` castle
(the trail (10,11) or a par1-consuming trigger) was not traced here — flag before wiring the seed spawn.

### 4.4 (10,42) painter action 0x2C — CONFIRMED
- ctor: model 0x2A → `strA1` `0x231370 = sub_50370` (EF:36734), sets `actionIndex = 0x2C`.
- action: `strA0` `0x2C = 0x218BC0 = AddTerrainMod0A_2A_37BC0` (EF:27648, "groove castle"). Tick handler
  transcribed in §5.1 below. EV dispatch case `0x218bc0` at EV:2665.

### 4.5 (10,41) leveler action 0x2B — CONFIRMED
- ctor: model 0x29 → `strA1` `0x231320 = sub_50320` (EF:36717), sets `actionIndex = 0x2B`.
- action: `strA0` `0x2B = 0x2187F0 = sub_377F0` (EF:27466). Tick handler transcribed in §5.2 below.
  (Note: `sub_377F0` reads the **MSPRD00** sprite table `filearrayindex_MSPRD00DATTAB`, i.e. it paints the
  sprite-piece footprint, whereas the painter reads **BUILD00**. See §5.2.)

### 4.6 (10,79) stage piece action 0x56 — CORRECTED
- ctor: model 0x4F → `strA1` `0x2318E0 = sub_508E0_castle_defend_create` (EF:36987), sets
  `actionIndex = 0x56`, `maxLife = 100000`, sprite 66, `fontTypeIndex = 1`.
- action: `strA0` `0x56 = 0x21BF00 = sub_3AF00_castle_defend_event` (EF:30106). EV case `0x21bf00` at
  EV:2787. Tick handler transcribed in §5.3.
- **CORRECTION to runtime §4c:** the (10,79) piece is the **defender/townsfolk launcher** run by
  `sub_3AF00`, NOT `sub_3A5B0`. `sub_3A5B0` (action 0x4F, EF:29590) is set only by ctor `sub_51800`
  (EF:37459) on **model 0x48 (72)** — a swarm piece — which merely shares the numeric id 0x4F.

---

## 5. The per-tick BUILD helpers (what runs each tick while building) — VERBATIM

### 5.1 `AddTerrainMod0A_2A_37BC0` (EF:27648) — the (10,42) PAINTER tick — VERBATIM core
```c
// EF:27648  // "groove castle"
void AddTerrainMod0A_2A_37BC0(type_entity_0x6E8E* a1x)//218bc0
{
    v30 = x_DWORD_E9C38_smalltit;                          // scratch delta-height buffer
    if (!(a1x->struct_byte_0xc_12_15.byte[0] & 0x2))       // FIRST tick: seed the countdown
    {
        a1x->dword_0x10_16 = 0x13;                         //   countdown = 19
        a1x->struct_byte_0xc_12_15.byte[0] |= 2;
    }
    if (a1x->dword_0x10_16 <= 0)                            // ── PHASE B: settle / finish (countdown<=0) ──
    {
        a1x->dword_0x10_16++;
        if (a1x->dword_0x10_16 == 0)                       // reached 0 → FINALIZE this pass
        {
            v50 = (unsigned __int16)(a1x->position_0x4C_76.x + 128) >> 8;   // centre tile X
            v23 = BUILD00[byte_0x46_70].width_4;  v25 = BUILD00[byte_0x46_70].height_5;  // (>>1 in VGA-1)
            if (!isCaveLevel_D41B6)                         //   non-cave: convert built cells' ceiling bit
            {
                v26x.y = centreTileY - (v25>>1);  v26x.x = v50 - (v23>>1);
                for (rows v25) for (cols v23) {
                    if (mapAngle_13B4E0[cell] & 8) {        //     bit3 (ceiling/built-marker) set:
                        mapAngle_13B4E0[cell] |= 0x80u;     //       set bit7 (built/blocked)  ← the space-check flag
                        mapAngle_13B4E0[cell] &= 0xF7u;     //       clear bit3
                    }
                }
            }
            Entities_EA3E4[a1x->parentId_0x28_40]->word_0x2E_46 = 2;   //   TELL the castle: pass done → state 2
            DisableEntityDrawing04_57F10(a1x);             //   DESPAWN the painter  ← this is how it "finishes"
        }
    }
    else                                                   // ── PHASE A: painting (countdown>0) ──
    {
        a1x->dword_0x10_16--;                              //   count down
        if (a1x->dword_0x10_16 == 0)
        {
            a1x->dword_0x10_16 = a1x->byte_0x3B_59 ? -25 : -1;   //   enter phase B (settle window)
        }
        else if (!Entities_EA3E4[a1x->parentId_0x28_40]->word_0x30_48)   // castle not mid-animation:
        {
            v49 = centreTileX;  v47 = centreTileY;  v40 = a1x->position_0x4C_76.z >> 5;   // datum height
            // (1) accumulate the target delta-height per cell across BUILD00 rows 1..byte_0x46_70:
            memset(v30, 0, 2 * height * width);
            for (i = 1; i <= a1x->byte_0x46_70; i++) {
                v10 = BUILD00[i].data;                     //   2 bytes/cell: [0]=sprite/angle, [1]=height
                for (rows) for (cols) {
                    if (a1x->struct_byte_0xc_12_15.byte[2] & 1)  sub_57390(cell, a1x->id_0x1A_26);  // own cell
                    v12 = v10[1];
                    if (v12 != 0xff)                       //   target = authored height + datum - current
                        v30[cell] = v12 + v40 - mapHeightmap_11B4E0[cell];
                    if ((!(a1x->dword_0x10_16 % 7) || a1x->dword_0x10_16 == 1) && v10[0] != 0xff)
                        sub_45DC0(7, cell, v10[0]);        //   paint sprite/shading every 7th tick (+ last)
                    v10 += 2;
                }
            }
            // (2) apply 1/countdown of each delta this tick (progressive rise):
            for (rows) for (cols) {
                if (v30[cell]) {
                    if (!mapHeightmap_11B4E0[cell] || sub_57450(mapTerrainType_10B4E0[cell])) {
                        mapAngle_13B4E0[cell] = mapAngle_13B4E0[cell] & 0xF8 | 1;   // low3 = built-flag 1
                        AddBuildingToTerrain_46570(cell, cell);   // = project mc2_add_building_region
                    }
                    mapHeightmap_11B4E0[cell] += (signed int)(int16_t)v30[cell] / a1x->dword_0x10_16;  // 1/countdown step
                    if (a1x->dword_0x10_16 == 1) {         //   LAST tick: clear bit7, (non-cave) set bit3
                        if ((char)mapAngle_13B4E0[cell] < 0) {
                            mapAngle_13B4E0[cell] &= 0x7F;
                            if (!isCaveLevel_D41B6) mapAngle_13B4E0[cell] |= 8;
                        }
                    }
                }
                // (cave ceiling maintenance on x_BYTE_14B4E0_second_heightmap: EF:27871-27894)
                if (!isCaveLevel_D41B6 && a1x->dword_0x10_16 == 2) mapAngle_13B4E0[cell] &= 0xF7u;  // clear bit3
            }
        }
    }
}
```
**Exact semantics for the port:**
- **Lifecycle:** first tick seeds `dword_0x10_16 = 19` (countdown). Phase A (countdown 19→1) each tick adds
  `delta/countdown` of the target height to every footprint cell (progressive rise; the divisor shrinks so
  the last ticks add the remainder). At countdown 1 it enters phase B (`-1`, or `-25` if `byte_0x3B_59`),
  which on reaching 0 flips built cells' `mapAngle` **bit3→bit7** (marking them "built/blocked" so the
  space-check `sub_11C80` sees them), signals the parent castle `word_0x2E_46 = 2`, and **despawns itself**
  via `DisableEntityDrawing04_57F10`.
- **How it "finishes" (answers the runtime "is any (10,42) alive" poll):** the painter despawns itself at
  phase-B end. The castle's build SM `case 4` (builder §2, EF:61147) scans `dword_38535` every 32 ticks for
  any live `class 10 && model 42`; once none remain it advances (`word_0x2E_46 = 3`). So "painter done" ==
  "the painter called `DisableEntityDrawing04` on itself after its 19-tick rise + settle."
- **Per-tick geometry write:** accumulate each cell's `target = authored_height + (z>>5) - current_height`
  into scratch `x_DWORD_E9C38_smalltit`, then add `target / countdown` to `mapHeightmap` (C signed divide,
  truncating). New/burnable cells (`!height || sub_57450(type)`) get `mapAngle low3 = 1` +
  `AddBuildingToTerrain_46570` (= `mc2_add_building_region`). Sprite/shading painted via `sub_45DC0` on the
  1st, every 7th, and last tick. It only paints when the castle is NOT mid-animation
  (`parent->word_0x30_48 == 0`). Cave levels additionally raise the ceiling heightmap
  `x_BYTE_14B4E0_second_heightmap` by `1/countdown` and toggle `mapAngle` bit3.

### 5.2 `sub_377F0` (EF:27466) — the (10,41) LEVELER / sprite-piece painter tick — VERBATIM core
```c
// EF:27466
void sub_377F0(type_entity_0x6E8E* a1x)//2187f0
{
    LOBYTE(v1) = centreTileX;  HIBYTE(v1) = centreTileY;
    v3 = MSPRD00[a1x->byte_0x46_70].width_4;              // ← reads the MSPRD00 (sprite) table, not BUILD00
    v5 = MSPRD00[a1x->byte_0x46_70].height_5;
    if (x_WORD_180660_VGA_type_resolution == 1) { v5 >>= 1; v3 >>= 1; }
    LOBYTE(v25) = v1 - (v3 >> 1);  HIBYTE(v25) = HIBYTE(v1) - (v5 >> 1);   // footprint origin
    v6 = a1x->struct_byte_0xc_12_15.byte[0];
    if (v6 & 2)                                           // (already-seeded) painting branch
    {
        if (!Entities_EA3E4[a1x->parentId_0x28_40]->word_0x30_48 && a1x->dword_0x10_16)  // castle idle & work left
        {
            v9 = a1x->subSpellIndex_0x2A_42 - a1x->word_0x2E_46;   // remaining amount
            v23 = v9 / a1x->dword_0x10_16;                          // this tick's slice = remaining/countdown
            a1x->word_0x2E_46 += v23;                               // advance the accumulator
            ... (per-cell flatten/paint over the MSPRD00 footprint, applying v23/countdown per cell) ...
        }
    }
    // (else: seed branch sets byte[0] bit1 and the countdown, symmetric to the painter)
}
```
**Exact semantics for the port:**
- The (10,41) "leveler" is the **sprite-piece footprint painter**: it reads the **MSPRD00** table
  (`filearrayindex_MSPRD00DATTAB`, the sprite-piece dat) indexed by `byte_0x46_70` (level), and progressively
  applies `remaining / countdown` per cell each tick (same progressive-slice scheme as the painter, but the
  accumulator is `word_0x2E_46` toward target `subSpellIndex_0x2A_42`).
- It only runs while the parent castle is not mid-animation (`parent->word_0x30_48 == 0`) and there is work
  left (`dword_0x10_16 != 0`). Like the painter it despawns itself when its countdown completes (the settle
  tail mirrors §5.1). **Its per-tick geometry pass flattens/paints the sprite footprint** (the visual
  "levelled ground under the walls"), complementing the painter's terrain-height rise.
- **Ctor `sub_50320` set-up (EF:36717):** action 0x2B, model 0x29, `maxLife = 0`, byte[0] bit3 cleared. The
  build-time `sub_5FC40` (builder §2.2) spawns it via the generic 4A190 (so it takes this ctor+action).

### 5.3 `sub_3AF00_castle_defend_event` (EF:30106) — the (10,79) DEFENDER/townsfolk launcher — VERBATIM core
The (10,79) piece is the persistent (maxLife 100000, sprite 66) castle defender. It is a state machine on
`byte_0x46_70` that periodically lofts class-3/class-5 defenders (townsfolk/turrets) OUT of the castle:
```c
// EF:30106
void sub_3AF00_castle_defend_event(type_entity_0x6E8E* a1x)//21bf00
{
    if (a1x->life_0x8 < 0) { DisableEntityDrawing04_57F10(a1x); return; }   // dead → despawn
    v2 = a1x->id_0x1A_26;
    if (!v2)               { DisableEntityDrawing04_57F10(a1x); return; }   // no owner → despawn
    v38x = Entities_EA3E4[v2];                                              // owner wizard
    v3.un_0x6E8E = Entities_EA3E4[v38x->dword_0xA4_164x->CastleEntityIndex_0x3A_58];  // owner's castle
    v37 = (a1x->word_0x4A_74 <= 1) ? 384 : 224;                            // height offset by level tag
    switch (a1x->byte_0x46_70)                                              // ── defender STATE MACHINE ──
    {
    case 0:  a1x->axis_0x9A_154x = a1x->position_0x4C_76; a1x->byte_0x46_70 = 1; goto LABEL_74;   // latch home
    case 1:  a1x->rand_0x14_20 = 9377*a1x->rand_0x14_20 + 9439;            // seed a random dwell
             a1x->dword_0x10_16 = (a1x->rand_0x14_20 % 0x30u) + 16; a1x->byte_0x46_70 = 2; goto LABEL_9;
    case 2: LABEL_9: if (!--a1x->dword_0x10_16) a1x->byte_0x46_70 = 3; goto LABEL_74;   // countdown → arm
    case 3:  if (a1x->byte_0x3E_62 & 0x3F) goto LABEL_74;                  // every 64 ticks:
             v3.unint = AddE7EE0x_10080(3, 12);                            //   need a free slot pair
             if (!v3.un_0x6E8E) goto LABEL_74;  v39 = 0; break;           //   fall to the target-scan (LABEL_33)
    case 4:  a1x->byte_0x46_70 = 5; a1x->dword_0x10_16 = 4; goto LABEL_37;
    case 5: LABEL_37: if (--a1x->dword_0x10_16) a1x->word_0x36_54 += 160;  // rising launch arc
             else { a1x->byte_0x46_70 = 6; a1x->word_0x36_54 = 0; } goto LABEL_74;
    case 6:  a1x->rand_0x14_20 = 9377*a1x->rand_0x14_20 + 9439;            // choose a defender kind by roll
             v13 = a1x->rand_0x14_20 % 0x64u; a1x->byte_0x46_70 = 7;
             a1x->word_0x2C_44 = (v13 ? (v13<=5 ? (a1x->byte_0x43_67==1)+2 : (a1x->byte_0x43_67!=1)) : 4);
             a1x->fontTypeIndex_0x3D_61 = (a1x->word_0x2C_44 <= 1) ? 6 : 1; goto LABEL_48;
    case 7: case 8: LABEL_48:                                              // LAUNCH the defender cast
             ... v17x = sub_6DCA0(castle, &a1x->position, v40, &SPELLS[v40].subspell[v42], 0, byte_0x46_70==7);
             ... (binds owner, aims via sub_655C0, decrements fontTypeIndex; when 0 → LABEL_104 done) ...
             a1x->byte_0x46_70 = (done) ? 1 : 8; goto LABEL_74;
    case 9:  goto LABEL_73;
    case 0xA: v22x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position, 10, 1);   // spawn a puff
              if (v22x) v22x->id_0x1A_26 = a1x->id_0x1A_26;
       LABEL_73: v41 = 1; goto LABEL_74;                                   // v41 → despawn at LABEL_74
    default: goto LABEL_74;
    }
LABEL_33: // target-scan: find a nearby enemy (class 3 model 0/1/3 not-own, or class 5 not-own not-model-22)
    ... if found: a1x->byte_0x46_70 = 4; a1x->word_0x96_150 = enemyIndex; ...
LABEL_74:
    if (v41) { sub_57F20(a1x); return; }                                   // v41 set → free the piece
    ... (byte_0x44_68 launch-arc offset table {0,115,230,334,368,384}, MoveEntity, ground-clamp z) ...
}
```
**Exact semantics for the port:**
- **Role:** the (10,79) piece is the **standing-castle defender turret**: it latches its home
  (`axis_0x9A_154x`), dwells a random 16..63 ticks, then every 64 ticks scans a small tile window for a
  nearby enemy (`LABEL_33` — class-3 model {0,1,3} not owned by this wizard, or class-5 not-owned and not
  model 22). On acquiring a target (`word_0x96_150`), it runs a launch arc (states 4/5/6) and casts a
  defender/projectile via `sub_6DCA0(castle, ..., &SPELLS[v40].subspell[v42], ...)`, the SPELL row chosen
  by a `% 0x64` roll off the castle's byte_0x43_67. It despawns (`sub_57F20`) when `v41` is set (state 9/0xA
  terminal) or draws to ground each tick otherwise.
- **RNG:** the standard `r = 9377*r + 9439` LCG on the piece's own `rand_0x14_20` (dwell time, defender
  kind). Modulo `0x30`, `0x64`, etc. are direct `%` on the unsigned LCG state.
- **Who runs it:** the EV action dispatch (`case 0x21bf00` → this function) once per tick per (10,79) piece.
  There is **no separate "child" spawner** — `sub_613D0` spawns the (10,79) piece directly (builder §3.2)
  and this handler is its tick. **CLARIFICATION to runtime §4c:** "sub_3AF00 = the (10,79) child" means
  exactly this: sub_3AF00 IS the (10,79) piece's own per-tick handler (action 0x56), and it in turn LAUNCHES
  class-3/class-5 defenders (the (5,15)-style militia) via `sub_6DCA0`. The (5,15) guard ROSTER slot machinery
  is the SEPARATE `sub_5FF50`/`array_0x5C_92` path (builder §6); do not conflate the two.

---

## 6. Consolidated bindings + constants

| entity | class,model | ctor (strA1) | action (strA0) | tick handler | cite |
|---|---|---|---|---|---|
| castle core | 3, 2 | (3,2) spawn | 4 = 0x2408F0 | EndOfCastleProjectile_5F8F0 | EF:1206,:61055 |
| castle core | 3, 2 | — | 5 = 0x240A70 | BeginOfCastleCreation_5FA70 | EF:1207,:61123 |
| castle core | 3, 2 | — | 6 = 0x240CA0 | sub_5FCA0_destroy_castle_level | EF:1208,:61222 |
| creation seed | 10, 32 (0x20) | 0x230A60 sub_4FA60 | **0x22 = 0x2154A0** | **sub_344A0 (projectile, NOT castle)** | EF:1636,:36292,:25052 |
| build painter | 10, 42 (0x2A) | 0x231370 sub_50370 | 0x2C = 0x218BC0 | AddTerrainMod0A_2A_37BC0 | EF:1746,:36734,:27648 |
| ground leveler | 10, 41 (0x29) | 0x231320 sub_50320 | 0x2B = 0x2187F0 | sub_377F0 (MSPRD00 painter) | EF:1745,:36717,:27466 |
| stage piece | 10, 79 (0x4F) | **0x2318E0 sub_508E0_castle_defend_create** | **0x56 = 0x21BF00** | **sub_3AF00_castle_defend_event** | EF:1783,:36987,:30106 |
| (swarm piece, unrelated) | 10, 72 (0x48) | 0x232800 sub_51800 | 0x4F = 0x21B5B0 | sub_3A5B0 | EF:37459,:29590 |

| constant | value | meaning | cite |
|---|---|---|---|
| pre-clear list | `dword_38527` ((10,45) BUILDING list) | overlap → life=-1, fontType=0 | EF:4403-4411 |
| pre-clear overlap | `\|dx\|<=pA+pB && \|dy\|<=rA+rB` (inclusive) | AABB Minkowski | EF:4407 |
| space-check object gate | `dword_38519` model-2 + `sub_106C0` overlap → 0 | EF:4449-4455 |
| space-check cell predicate | `mapAngle bit7 set` OR (cave & `bit3` set) → blocked | `sub_11C80` | EF:4549-4551 |
| space-check scan region | the RING between level & level+1 footprints (4 bands) | EF:4462-4535 |
| no-room UI | `str_unk_1804B0ar.word_0x88 = 93` (if help on) | `sub_88D00` | EF:49263-49264 |
| ghost sync target | owner `SpellEnabled[2]` entity | `sub_5F890` | EF:61036 |
| ghost sync a2!=0 | ghost `word_0x2E_46 = word_0x30_48 - 1` | build in progress | EF:61042 |
| ghost sync a2==0 | ghost `word_0x2E_46 = 0` + `sub_6D880` | finish/reset | EF:61046-61047 |
| re-cast castle case | target model 2, idle (`word_0x2E_46<=0`), mana≥cap → `sub_5F7B0` | EF:60908-60961 |
| re-cast arm | castle `word_0x2E_46 = word_0x30_48` | `sub_5F7B0` | EF:60976 |
| re-cast third arg a3 | OR-mask into flags dword; 0 for Create-Castle; forced 0 if caster model 1 | EF:6826,:60888 |
| re-cast busy sound | 29 | castle mid-build | EF:60912 |
| re-cast not-enough | `sub_88B60()` + sound 29 | mana < cap | EF:60966-60967 |
| painter countdown | 19 (0x13) seed | rise window | EF:27700 |
| painter step | `+= delta / countdown` per cell | progressive rise | EF:27858 |
| painter built-flag | phase-B: bit3 → bit7 (built/blocked) | drives space-check | EF:27742-27743 |
| painter finish | despawn self + parent `word_0x2E_46 = 2` | EF:27753-27754 |
| painter sprite paint | `sub_45DC0` on tick 1, every 7th, and last | EF:27832-27833 |
| leveler table | MSPRD00 (`filearrayindex_MSPRD00DATTAB`) | sprite footprint | EF:27506 |
| defender launch cast | `sub_6DCA0(castle, pos, SPELLS[v40].subspell[v42], ...)` | EF:30284 |
| defender scan period | every 64 ticks (`byte_0x3E_62 & 0x3F`) | EF:30195 |
| defender arc offsets | {0,115,230,334,368,384} by `abs(byte_0x44_68)` | EF:30407-30424 |
| RNG law | `r = 9377*r + 9439` | (shared) | — |

---

## 7. OPEN (could not confirm without further tracing)

- **How the (10,32) seed becomes a `(3,2)` castle.** `sub_344A0` (action 0x22) is confirmed a projectile
  shedding (10,11) trails; it does NOT create the castle itself. The impact/landing → castle-spawn path
  (via the (10,11) trail entity, or a par1-consuming trigger elsewhere) was not traced. **Do NOT port the
  seed as "spawns a castle on tick 0."** Flag: find where a landed seed/trail calls `(3,2)` spawn or
  `BeginOfCastleCreation`.
- **`sub_11A10` band geometry exact coverage.** The four-band ring scan (top/bottom full-width, left/right
  inner columns) is transcribed verbatim, but whether the corner cells are double-tested or exactly
  partitioned was reasoned from the offsets (`v17 = v19 - v18` col width, `v22 = v12 - v21` row height),
  not verified against a golden footprint. Confirm with a state-hash on a level-2→3 upgrade over a mixed
  terrain patch before relying on the exact abort boundary.
- **`sub_106C0` exact half-extent used in the space-check object gate** (EF:4451) — it is the `±0` AABB
  overlap (no margin), vs `sub_11CB0`'s `+2560` margin variant (EF:4557). Assumed the strict overlap;
  confirm the field passed if the port's "can't build next to a building" distance feels off.
- **`sub_377F0` (leveler) full per-cell body** — only the outer lifecycle + the `remaining/countdown`
  slice accumulator were transcribed; the inner MSPRD00 flatten/paint loop (EF:27536-27600) was summarized.
  Transcribe it verbatim if the sprite-piece ground-flatten needs exact parity.
- **`sub_3AF00` LABEL_33 target-scan window size** — the tile scan window and the exact enemy-eligibility
  ladder (class-3 model {0,1,3}, class-5 not-model-22) were transcribed at the branch level; the window
  extent (`byte_0x46_70`-derived) and the `sub_10130`/`sub_10080` slot helpers were not expanded. Expand
  before porting the defender AI's acquisition range.
