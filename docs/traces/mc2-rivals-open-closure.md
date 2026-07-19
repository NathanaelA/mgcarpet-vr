# MC2 RIVALS — OPEN-ITEM CLOSURE — port-ready verbatim trace

Closes the untranscribed OPEN items left dangling by the two MC2 rival traces:
`mc2-rivals-brain.md` (§1.4, §2.2, §4, §9) and `mc2-rivals-spawn-mortality.md` (§10). Every item below is
transcribed verbatim from the decompile with renamed-field comments, and the doc ends with a PORT
CONSEQUENCES section. Read `mc2-rivals-brain.md` first for the brain map, and `mc2-castle-builder.md` for
house-style conventions (verbatim C, renamed-field comments, RNG law `r = 9377*r + 9439`, uint8 wrap).

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`: EF = `engine/EventsFunctions.cpp`,
GameUI = `engine/GameUI.cpp`, Maths = `utilities/Maths.cpp`, Spells = `engine/Spells.h`,
gtypes = `engine/global_types.h`, BasicTerrain = `engine/BasicTerrain.h`. Trace date 2026-07-12.

---

## Headline findings (read first)

1. **THE "SPELL-PICKUP ACQUIRE CHAIN" (brain §4) IS A MIS-READ — it is a REACTIVE ANTI-PROJECTILE DEFENSE.**
   `sub_15CB0` scans the **class-9 chain** (`dword_38531`, projectiles/effects) for the nearest entity
   *targeted at this wizard* (`word_0x96_150 == self.id`), within `EuclideanDistXY < 0x1900000` (= 5120²).
   `sub_15D20` gives a **strafe jink** (`strafeSpeed = 80`). `sub_15D40` — if the threat is within `0x100000`
   (=1024²) — **casts a reactive spell by threat model**: model 0 or 3 → try spell **8** (shield), and cast
   spell **6** ONLY as the else-fallback if the 8-cast did not land; model 4 → cast spell **6**; models 1/2 →
   nothing. [CORRECTED 2026-07-16 to match the corrected body below — the old "0..2 → 8 then 6" headline
   pre-dated §Trace-bank corrections 2.] **No jar, no scroll, no XP, no despawn, no ownership grant.** The
   brain-trace §4 "auto-pick a jar/scroll keyed to self / learn by pickup" reading is WRONG on all counts.
   The MC2 AI learns its book ONLY at load (`InitialiseSpells_54A50`, tiers from `byte_0x360FBx`); there is
   no runtime spell acquisition in the brain at all.

2. **`sub_583B0` IS CHEBYSHEV (max-axis) RAW distance, NOT squared, NOT supercell-scaled.**
   `return max(|dx|, |dy|)` in world units (EF:40408). So the scout-site gate `sub_583B0(nearest,cand) >
   0x3000` (EF:6087/6093) means **the nearest foreign castle must be > 12288 world units away on its
   dominant axis** — a plain Chebyshev radius, not a squared threshold. The project's `12288*12288`
   Euclidean-squared gate is WRONG; re-derive as `max(|dx|,|dy|) > 12288` (or, if keeping Euclidean, note
   it is a looser test than the original).

3. **`spellIndex_D94FF[29] = {0..25, 0,3,0}` — an IDENTITY map for spells 0..25**, then three trailing
   book-icon aliases (idx 26→0, 27→3, 28→0). The book-order → spell-id remap is a **no-op for the 26 real
   spells** (icon i = spell i); only the 3 extra UI icon slots alias back to fireball(0)/speed-up(3)/
   fireball(0). So AI grant/tier indexing by `spellIndex_D94FF[i]` for i<26 is just `i`. (GameUI:59.)

4. **`sub_61050` (steal-mana intake) DRAINS the VICTIM's CASTLE (not the victim's own mana) by a per-tier
   PERCENTAGE, spawns the drained mana as (10,39) spheres, and credits the thief.** The steal channel is
   `str_0x5E_94.word_0x74_116` (thief entity id) + `.dword_0x70_112` (spell tier 0..2) on the victim's damage
   mailbox. It reads `SPELLS[13]` (spell 0xD = the steal-mana spell): `subspell[tier].subSpellIndex_2` = the
   **percent** of the victim-castle mana to remove, `subspell[tier].life_0x1A` = the **destination mode**
   (1 = scatter as neutral spheres, 2 = scatter as thief-owned spheres). Fallback (no castle) just moves a
   flat mana amount thief↔victim. Also awards duel XP `sub_6D8B0(thief, 0xD, 1)`.

5. **`dword_0x16D_365` IS A DEAD/VESTIGIAL COUNTDOWN.** Init 2000 at spawn (EF:43713); decremented every
   tick in BOTH carpet ticks (AI EF:5436, human EF:60014) *if nonzero*; and (de)serialized in the save
   block. **There is NO reader anywhere that branches on it** (exhaustive grep: only the two decrements, the
   init, and save I/O). It is a save-persisted timer with no live consumer in this build. Safe to model as a
   plain decrementing counter or omit entirely.

6. **THE MC2 HUMAN GETS NO AUTHORED STARTING CASTLE.** The ONLY two runtime reads of
   `player_0x2FED9[color]` are EF:43777 and EF:43789 — **both strictly inside `if (IsAiPlayer == 1)`** in
   `sub_5C950`. Exhaustive grep of `player_0x2FED9` finds no other runtime consumer (the rest are
   save/convert copies). So a level that authors `player_0x2FED9[human] = 7` (e.g. level-067) **does nothing
   for the human** — the human must cast Create-Castle to get a castle. The MC1 "free instant first castle"
   is not present for the MC2 human.

7. **THE WATER-STEER `x_WORD_D3FCE`/`x_WORD_D3FE8` TABLES + the 4-neighbor probe + the micro-FSM are fully
   transcribed below (§1).** The probe is a 4-bit water mask (bit0=N, bit1=side-fwd, bit2=S, bit3=side-back),
   the "case" is a packed 16-bit `(escape_index<<8 | mask)` word, and the FSM on `byte_0x45E_1118`/`_1119`
   walks a raycast (`sub_16E70`) + 40-step ray-march (`sub_16CA0`) to pick the shorter detour around water.

---

## 1. THE WATER / OBSTACLE STEER SUPPORT (brain §2.2) — VERBATIM

The caller `sub_16580` (EF:7879) is already transcribed in `mc2-rivals-brain.md` §2.2. This section
transcribes its three support subs + the four static tables it (and they) index, and settles the mask
semantics + the micro-FSM law.

### 1.0 The static tables (EF:1074-1079) — VERBATIM

```c
// EF:1074  four-neighbour ray-march STEP deltas (signed, 0xff = -1), indexed by the mask/exit code
char    x_BYTE_D3F96[14] = { 0x00,0xff,0x00,0xff,0x01,0xff,0x00,0x00,0x00,0x00,0xff,0x00,0x00,0x00 }; // dx (probe A)
char    x_BYTE_D3FA4[14] = { 0x00,0x00,0xff,0x00,0x00,0x00,0xff,0xff,0x01,0x01,0x00,0x01,0x00,0x00 }; // dy (probe A)
char    x_BYTE_D3FB2[14] = { 0x00,0x01,0x00,0x00,0xff,0x01,0xff,0x00,0x00,0x01,0x01,0x00,0x00,0x00 }; // dx (probe B)
char    x_BYTE_D3FC0[14] = { 0x00,0x00,0x01,0x01,0x00,0x00,0x00,0x01,0xff,0x00,0x00,0x01,0xff,0x00 }; // dy (probe B)

// EF:1078  YAW to snap to, indexed by the low byte of the packed exit word (0..12 / 0..13)
__int16 x_WORD_D3FCE[13] = { 0, 1536,    0, 1536,  512, 1536,    0,    0, 1024, 1024, 1536, 1024,  512 };       // LEFT-turn table
__int16 x_WORD_D3FE8[14] = { 1024,  512, 1024, 1024, 1536,  512, 1536, 1024,    0,  512,  512, 1024,    0, 0 }; // RIGHT-turn table
```

The yaw values are the game's 0..2047 (0x800 = 360°) heading units: 0 = +X (east), 512 = +Y, 1024 = −X,
1536 = −Y. Both tables are indexed by `v8` = the LOW BYTE of `sub_16730`'s packed return word (see §1.2).

### 1.1 `sub_16730` (EF:7955) — the FOUR-NEIGHBOUR WATER PROBE — VERBATIM

Signature `int16_t sub_16730(type_entity_0x6E8E* a2x, char a3)`. `a3` selects the probe HANDEDNESS
(0 = probe the LEFT side, 1 = probe the RIGHT side). It returns a **packed 16-bit word**: if a direct
neighbour is water it returns the 4-bit MASK (low byte, high byte 0); otherwise it consults the FSM memory
`byte_0x45E_1119` and returns a full `(exit_index<<8 | mask)` word for a diagonal escape.

```c
// EF:7955   uaxis_2d.word packs {.x = tileX(low byte), .y = tileY(high byte)}; mapTerrainType_10B4E0[word]
//           reads the terrain-type map at that (x,y); type 8 == WATER.
int16_t sub_16730(type_entity_0x6E8E* a2x, char a3)
{
    v14 = a2x->position_0x4C_76.y >> 8;          // wizard tile Y (>>8 = pixel→tile)
    v3x.x = a2x->position_0x4C_76.x >> 8;         // wizard tile X
    v3x.y = v14 - 1;                              // ── neighbour NORTH (y-1) ──
    a1y.word = 0;                                 // mask accumulator (the return value)
    if (mapTerrainType_10B4E0[v3x.word] == 8)     //   N is water →
        a1y.x = 1;                                //     set BIT 0
    else {
        v3x.y = v14 + 1;                          // ── neighbour SOUTH (y+1) ──
        if (mapTerrainType_10B4E0[v3x.word] == 8) //   S is water →
            a1y.x = 4;                            //     set BIT 2
    }
    v3x.y = a2x->position_0x4C_76.y >> 8;         // back to own row
    if (a3) v3x.x++; else v3x.x--;                // ── neighbour FORWARD-SIDE (a3? +x : -x) ──
    if (mapTerrainType_10B4E0[v3x.word] == 8) {   //   water →
        if (a3) { a1y.x |= 2; goto LABEL_18; }    //     right probe: set BIT 1
    LABEL_17:
        a1y.x |= 8; goto LABEL_18;                //     left  probe (or fallthrough): set BIT 3
    }
    v13 = a2x->position_0x4C_76.x >> 8;
    if (a3) v3x.x = v13 - 1; else v3x.x = v13 + 1;// ── neighbour BACK-SIDE (a3? -x : +x) ──
    if (mapTerrainType_10B4E0[v3x.word] == 8) {   //   water →
        if (!a3) { a1y.x |= 2; goto LABEL_18; }   //     left  probe: set BIT 1
        goto LABEL_17;                            //     right probe: set BIT 3
    }
LABEL_18:
    if (a1y.x) return a1y.word;                   // ── any direct neighbour water → RETURN THE MASK ──

    // ── no direct water: consult FSM memory byte_0x45E_1119, probe the DIAGONAL, return a packed exit ──
    if (a3) {                                     // RIGHT probe
        switch (a2x->dword_0xA4_164x->str_611_byte_0x45E_1119) {
        case 1: case 9:                           //   diag NW (x-1,y-1) water → 1544 = (6<<8)|8
            ...check (x-1, y-1) == 8 → return 1544;
        case 2: case 3:                           //   diag NE (x+1,y-1) water → 3073 = (12<<8)|1
            ...check (x+1, y-1) == 8 → return 3073;
        case 4: case 6:                           //   diag SE (x+1,y+1) water → 2306 = (9<<8)|2
            ...check (x+1, y+1) == 8 → return 2306;
        case 8: case 0xC:                         //   diag SW (x-1,y+1) water → 772  = (3<<8)|4
            ...check (x-1, y+1) == 8 → a1y.word = 772; return a1y.word;
        default: return a1y.word;                 //   (0)
        }
    }
    switch (a2x->dword_0xA4_164x->str_611_byte_0x45E_1119) {   // LEFT probe
    case 1: case 3:  ...check (x+1, y-1) → 770  = (3<<8)|2;    //   diag NE water
    case 2: case 6:  ...check (x+1, y+1) → 1540 = (6<<8)|4;    //   diag SE water
    case 4:          ...check (x-1, y+1) → 3080 = (12<<8)|8;   //   diag SW water
    case 8: case 9:  ...check (x-1, y-1) → 2305 = (9<<8)|1;    //   diag NW water
    default: return a1y.word;
    }
}
```

**MASK SEMANTICS (the low byte, 4 bits):**
- **bit 0 (1)** = the NORTH neighbour (y−1) is water.
- **bit 1 (2)** = the FORWARD-SIDE neighbour is water (right-probe: +x; left-probe: −x).
- **bit 2 (4)** = the SOUTH neighbour (y+1) is water.
- **bit 3 (8)** = the BACK-SIDE neighbour is water (right-probe: −x; left-probe: +x).

The terrain check is **`mapTerrainType_10B4E0[tile] == 8` ONLY — water is the sole obstacle type**. No other
terrain type is tested anywhere in the steer path (grep-confirmed: the four subs test `== 8` exclusively).

**PACKED RETURN:** the low byte (0..12) indexes `x_WORD_D3FCE`/`x_WORD_D3FE8` for the escape yaw; the high
byte is the diagonal exit code. The direct-neighbour path returns just the mask (high byte 0). The diagonal
values are `(hi<<8 | lo)`: 1544=(6,8), 3073=(12,1), 2306=(9,2), 772=(3,4), 770=(3,2), 1540=(6,4),
3080=(12,8), 2305=(9,1) — i.e. the low byte is always a table index into the yaw arrays.

### 1.2 `sub_169C0` (EF:8111) — the SITUATION CLASSIFIER — VERBATIM

Returns 0 = clear / 1 = commit-left / 2 = commit-right / 3 = stay-in-arc, and updates `byte_0x45E_1118`.
On the FIRST tick of an obstacle (`byte_0x45E_1118 == 0`) it ray-marches BOTH detour directions 40 steps
and picks the one whose exit point is nearer the wizard's steering target (`word_0x96_150`).

```c
// EF:8111
char sub_169C0(type_entity_0x6E8E* a1x)
{
    v12.x = a1x->position_0x4C_76.x >> 8;  v12.y = a1x->position_0x4C_76.y >> 8;   // wizard tile
    v11 = v12;
    v1x = Entities_EA3E4[a1x->word_0x96_150];                                       // ← the STEER TARGET
    v9.x = v1x->position_0x4C_76.x >> 8;  v9.y = v1x->position_0x4C_76.y >> 8;      // target tile
    HIBYTE(v2) = a1x->dword_0xA4_164x->str_611_byte_0x45E_1118;                     // FSM state
    switch (byte_0x45E_1118) {
    case 0:                                                                         // ── FRESH obstacle ──
        v15a = sub_16730(a1x, 0);                                                   //   probe LEFT
        v14a = sub_16730(a1x, 1);                                                   //   probe RIGHT
        if (!v14a && !v15a) return 0;                                              //   both clear → no steer
        if (v15a) {                                                                //   march LEFT detour 40 steps
            for (v5 = 0; v5 < 0x28u; v5++) {
                v12.x += x_BYTE_D3F96[v15a];  v12.y += x_BYTE_D3FA4[v15a];          //     step by table delta
                v16 = v12;                                                          //     remember exit
                v15a = sub_16CA0(&v12, v15a, 0);                                    //     re-probe from new cell
            }
        }
        if (v14a) {                                                                //   march RIGHT detour 40 steps
            for (v6 = 0; v6 < 0x28u; v6++) {
                v11.x += x_BYTE_D3FB2[v14a];  v11.y += x_BYTE_D3FC0[v14a];
                v13 = v11;
                v14a = sub_16CA0(&v11, v14a, 1);
            }
        }
        if (v15a && v14a) {                                                        //   both still blocked →
            if (abs(v9.y-v16.y)*abs(v9.x-v16.x) > abs(v9.x-v13.x)*abs(v9.y-v13.y)) //   pick exit NEARER target
                v8 = 2;                                                            //     (rect-area proxy for dist)
            else
                v8 = 1;
            goto LABEL_21;
        }
        if (!v15a) { v8 = 2;                                                       //   left cleared out → go RIGHT
        LABEL_21:
            byte_0x45E_1118 = v8; return v8;
        }
        byte_0x45E_1118 = 1; return 1;                                             //   else → go LEFT
    case 1:                                                                        // ── committed LEFT ──
        v15b.word = sub_16730(a1x, 0);
        if (!v15b.y || sub_16E70(&v12, &v9))                                       //   exit clear OR LOS to target →
            result = 1;                                                            //     keep going left
        else { byte_0x45E_1118 = 3; result = 3; }                                  //   else → freeze arc (state 3)
        break;
    case 2:                                                                        // ── committed RIGHT ──
        v14b.word = sub_16730(a1x, 1);
        if (!v14b.y || sub_16E70(&v12, &v9)) result = 2;
        else { byte_0x45E_1118 = 3; result = 3; }
        break;
    default: return v2;                                                            // states 3..8 handled by caller
    }
    return result;
}
```

- `sub_16E70` (EF:8403) is a **Bresenham line-of-sight raycast** from the wizard tile to the target tile:
  it walks the integer line and returns `>0` (the step index+1) if it crosses a water tile, `0` if the path
  to the target is water-free. Used to decide whether to STOP steering (clear LOS → resume direct flight).
- `sub_16CA0` (EF:8245) is the same 4-neighbour probe as `sub_16730` but operating on an arbitrary tile
  cursor `baxis_2d* a2x` (the marching cursor), returning the same packed mask/exit word so the 40-step
  march can chain.

### 1.3 THE MICRO-FSM LAW on `byte_0x45E_1118` / `_1119`

`str_611_byte_0x45E_1118` = the avoidance FSM STATE; `str_611_byte_0x45E_1119` = the LAST chosen exit code
(the packed low-byte the yaw came from). The whole cycle, driven by `sub_16580` (§2.2 of the brain trace):

| `byte_0x45E_1118` | meaning | who sets it | transition |
|---|---|---|---|
| 0 | idle / no obstacle | `sub_16580` case 0, `sub_169C0` when clear | `sub_169C0` re-probes each tick |
| 1 | committed LEFT detour | `sub_169C0` case 0 → left | caller reads `x_WORD_D3FCE[exit]`; `sub_169C0` case 1 keeps or → 3 |
| 2 | committed RIGHT detour | `sub_169C0` case 0 → right | caller reads `x_WORD_D3FE8[exit]`; `sub_169C0` case 2 keeps or → 3 |
| 3..8 | FROZEN arc (coast) | `sub_169C0` (→3), `sub_16580` (++ each tick) | `sub_16580` cases 3..8 just `byte_0x45E_1118++`, no re-probe; at 8 → `sub_169C0` re-runs |
| ≥8 | re-evaluate | wraps back via case-`<=7` gate in `sub_16580` | `sub_16580` calls `sub_169C0` again (state 0 path) |

`sub_16580`'s gate `if (byte_0x45E_1118 <= 2 || byte_0x45E_1118 >= 8)` → call `sub_169C0` (states 0/1/2 and
the 8+ wrap); `else` (states 3..7) → `v4 = 3` = "stay frozen". So the FSM is: **detect (0) → commit L/R
(1/2) → hold the turn for ~5 ticks (3..7, incrementing) → re-detect (8→0).** On any tick where the yaw
actually changed, `sub_16580` **zeroes speed** (`actSpeed = 0; speed_0xc_12 = 0; word_0xe_14 = 1`) and
realigns the steering setpoint `roll_0x20_32 = yaw`. `_1119` is written whenever the chosen exit differs, so
the diagonal-escape switches in `sub_16730`/`sub_16CA0` recall the last commit direction.

---

## 2. `sub_583B0` — THE SCOUT-SITE DISTANCE METRIC (brain §1.4) — VERBATIM

```c
// EF:40408
int sub_583B0(axis_3d* a1, axis_3d* a2)
{
    v2 = abs(a2->x - a1->x);
    v3 = abs(a2->y - a1->y);
    if (v2 < v3) v2 = v3;
    return v2;                    // ← CHEBYSHEV: max(|dx|, |dy|), RAW world units (NOT squared, NOT scaled)
}
```

**UNITS: raw world units, Chebyshev (L∞) distance** — the larger of |Δx| and |Δy|, no square, no supercell
scaling, no z. Contrast the neighbours in the same file: `sub_583F0_distance_3d` (EF:40421) = true 3-D
radix distance, `sub_58440` (EF:40430) = 3-D squared, `EuclideanDistXY_584D0` (Maths:1043) = XY squared.
`sub_583B0` is the cheap axis-aligned one.

So the scout-site accept (EF:6087/6093) `sub_583B0(nearestForeignCastle, candidate) > 0x3000` means
**the nearest OTHER castle must be more than `0x3000` = 12288 world units away on its dominant axis** from
the candidate build site. In tile units (>>8) that is 48 tiles. **The project's `12288*12288` squared-Euclid
gate is WRONG**; port as `max(|dx|,|dy|) > 12288`.

---

## 3. THE "SPELL-PICKUP ACQUIRE CHAIN" — actually a REACTIVE ANTI-PROJECTILE DEFENSE (brain §4)

The three subs the housekeeping calls on the decision-cadence tick (EF:5462-5467, caller confirmed):

```c
// EF:5460  (decision-cadence gate: byte_0x3E_62 % (64 - Reflexes/4) == 0)
v26x = sub_15CB0(a1x);              // find nearest CLASS-9 entity aimed at me
if (v26x) { sub_15D20(a1x);         //   jink
            sub_15D40(v2, a1x, v26x); }  //   reactive cast by threat model
```

### 3.1 `sub_15CB0` (EF:7435) — SCAN FOR AN INCOMING THREAT — VERBATIM

```c
// EF:7435
type_entity_0x6E8E* sub_15CB0(type_entity_0x6E8E* a2x)
{
    a1 = 0;  v2 = -1;                                         // v2 = best (min) distance
    for (ix = dword_38531; ix > Entities_EA3E4[0]; ix = ix->next_0) {   // ← the CLASS-9 chain (projectiles/effects)
        if (ix->word_0x96_150 == a2x->id_0x1A_26) {          //   this entity is AIMED AT me
            v4 = EuclideanDistXY_584D0(&a2x->position, &ix->position);   //   XY-squared distance
            if (v4 < v2) { v2 = v4; a1 = ix; }               //   keep the nearest
        }
    }
    if (v2 >= 0x1900000) result = 0;                         // ← range gate: 0x1900000 = 5120² → ignore if farther
    else result = a1;
    return result;
}
```

- **What is scanned:** the **class-9 entity chain** `dword_38531` (built by the entity re-link switch case
  `0x09`, EF:40010-40017 — the same chain the projectile hate-feed `sub_159E0` walks, EF:7332). Class 9 is
  the homing-projectile / effect class.
- **The ownership key:** `word_0x96_150` on a class-9 entity holds the id of the wizard it is HOMING ON /
  aimed at. `word_0x96_150 == self.id` means **"this projectile is targeting ME."** (It is NOT a pickup
  ownership tag; it is the projectile's victim id.)
- **Range:** `EuclideanDistXY < 0x1900000` = **5120² world units** (dx²+dy²; `EuclideanDistXY_584D0`,
  Maths:1043 = raw XY-squared). Nearest qualifying threat wins.

### 3.2 `sub_15D20` (EF:7469) — JINK — VERBATIM

```c
// EF:7469
void sub_15D20(type_entity_0x6E8E* a1x)
{
    if (!a1x->dword_0xA4_164x->str_611_byte_0x45E_1118)     // only if NOT mid water-steer
        a1x->dword_0xA4_164x->strafeSpeed_0x10_16 = 80;     // ← impulse the strafe channel (dodge)
}
```

Confirms the strafe-channel impulse value **80** (the brain §2.1 note that the project's flat-80 jink is an
MC1 approximation is thus HALF-right: 80 IS the value used for the *reactive dodge*, while the *combat
weave* uses `3·minSpeed·Reflexes/255` — two different writers of `strafeSpeed_0x10_16`).

### 3.3 `sub_15D40` (EF:7480) — REACTIVE CAST BY THREAT MODEL — VERBATIM

```c
// EF:7480  a2x = self (the AI wizard), a3 = the incoming class-9 threat entity
char sub_15D40(__int16 a1, type_entity_0x6E8E* a2x, type_entity_0x6E8E* a3)
{
    result = EuclideanDistXY_584D0(&a2x->position, &a3->position);
    if (result >= 0x100000) return result;                  // ← react only within 1024² (0x100000)
    result = a3->model_0x40_64;                             // ── branch on the THREAT's model ──
    if (result < 3u) {                                      //   model 0/1/2 (wizard-fired families) →
        if (result) return result;                         //     model 1 or 2 → (no-op, return)
        /* model 0 falls through to the shield+recover block below */
    }
    else if (result > 3u) {                                 //   model 4+ →
        if (result == 4) {                                 //     model 4 → cast spell 6 (recover/counter)
            v6 = SpellLevels[6];
            if (v6 >= 0) { while (1) { if (sub_15F20(a2x,v6,6)==6) break; if (--v6<0) return; }
                           sub_14E10(a2x, 6u); }            //       cast spell 6 if owned+ready+level-usable
        }
        return result;
    }
    /* model 0 OR model 3: cast spell 8 (shield), then spell 6 */
    for (i = SpellLevels[8]; i >= 0; i--) {                 //   ── cast spell 8 (SHIELD) ──
        if (sub_15F20(a2x, i, 8) == 8) { a1 = sub_14E10(a2x, 8u); break; }
    }
    if (a1 != 8) {                                          //   if shield wasn't castable, cast spell 6
        v5 = SpellLevels[6];
        if (v5 >= 0) { while (1) { if (sub_15F20(a2x,v5,6)==6) break; if (--v5<0) return; }
                       sub_14E10(a2x, 6u); }
    }
    return result;
}
```

- **What it does:** casts a **defensive/counter spell** in reaction to an incoming projectile, chosen by the
  projectile's MODEL: model 0 or 3 → try **spell 8** first, and cast **spell 6** ONLY IF the 8-cast did not
  land (`a1` captures the cast chain's result and the `a1 != 8` test is an ELSE-fallback, not a sequence);
  model 4 → cast **spell 6**; models 1/2 → nothing. `sub_15F20(self, level, spell)` is the
  owned+affordable+level-usable probe; `sub_14E10(self, spell)` is the standard AI cast executor.
  [CORRECTED 2026-07-15, pedantic review §Trace-bank corrections 2: the earlier "always casts 6 after 8"
  summary was a misread of the `a1 = ...` capture; the port's else-fallback is faithful.]
- **Range:** only within `EuclideanDistXY < 0x100000` = **1024² world units** (threat imminent).
- **NO acquisition, NO XP, NO level grant, NO despawn** — the chain never touches the threat entity, never
  reads a jar/scroll, and never mutates ownership. **The brain-trace §4 "spell pickup / learn-by-pickup"
  reading is fully retracted by this.** MC2 AI spell learning is 100% load-time (`InitialiseSpells_54A50`,
  tier = `byte_0x360FBx`, per the lifecycle trace §3); there is no runtime brain acquisition path.

---

## 4. `spellIndex_D94FF[29]` — THE BOOK-ORDER → SPELL-ID REMAP (lifecycle §10) — VERBATIM

```c
// GameUI:59
char spellIndex_D94FF[29] = { 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,  0,3,0 };
//                            └──────────────── identity map for the 26 real spells ────────────────┘  └26,27,28┘
```

- **The 26 real spells are an IDENTITY map** (icon i → spell i). Book-order equals spell-id for i∈[0,25].
- **Indices 26/27/28** are three extra UI icon slots that ALIAS: 26→spell 0 (fireball), 27→spell 3
  (speed-up), 28→spell 0 (fireball). These are the CTRL-pane duplicate icons, not distinct spells.
- **Consequence:** every `spellIndex_D94FF[i]` for i<26 in `InitialiseSpells_54A50` (EF:38688) and
  `sub_55AB0` (Level:1309) is just `i`; the AI grant/tier masks (`StartingSpells`, `byte_0x360FBx`,
  `BlockedSpells`) are indexed by the raw spell id with no permutation. The port's book wiring needs no
  remap table for the 26 spells (only reproduce the 3 icon aliases if it mirrors the CTRL pane).

---

## 5. `sub_61050` — STEAL-MANA INTAKE (lifecycle §10) — VERBATIM

Called from the shared intake `sub_5EFA0` (EF:60666-60667) when the victim's damage mailbox has a steal
channel set: `if (a1x->str_0x5E_94.word_0x74_116) sub_61050(a1x);`. **`a1x` = the VICTIM.**

**The steal channel fields** (on the victim's 36-byte damage mailbox `str_0x5E_94`, gtypes:96-111):
- `word_0x74_116` = **the THIEF's entity id** (`Entities_EA3E4[v1]` = the thief `v34x`).
- `dword_0x70_112` = **the spell TIER** (subspell index 0..2) used to look up `SPELLS[13].subspell[tier]`.

**The spell it reads** is `SPELLS_BEGIN_BUFFER_str[13]` = **spell id 0xD = the steal-mana spell**
(Spells.h subspell fields: `subSpellIndex_2`, `life_0x1A`).

```c
// EF:62076  a1x = VICTIM
type_entity_0x6E8E* sub_61050(type_entity_0x6E8E* a1x)
{
    v33 = 0; v35 = 0;
    v1  = a1x->str_0x5E_94.word_0x74_116;             // thief id
    v34x = Entities_EA3E4[v1];                         // thief entity
    if (v34x < Entities_EA3E4[0]) goto LABEL_35;
    if (v34x->class_0x3F_63 == 3) {                    // ── thief is a WIZARD ──
        sub_6D8B0(v1, 0xD, 1);                         //   +1 steal-mana XP to the thief
        v4 = SPELLS[13].subspell[tier].life_0x1A;      //   life_0x1A = DESTINATION MODE (1 or 2), 0 = flat path
        if (v4) {
            if (v4 > 2u) goto LABEL_23;
            v6x = Entities_EA3E4[a1x->CastleEntityIndex];   // VICTIM's castle
            v7x = Entities_EA3E4[v34x->CastleEntityIndex];  // THIEF's castle (v31x)
            if (v6x <= 0 || v7x <= 0 || v6x->mana <= 0)     // no victim castle / no thief castle / empty →
                v35 = 1;                                     //   fall back to the flat mana move (LABEL_23)
            else {
                percent = SPELLS[13].subspell[tier].subSpellIndex_2;   // ← subSpellIndex_2 = the STEAL PERCENT
                v33 = v6x->mana * percent / 100;             // amount = percent% of VICTIM-CASTLE mana
                v6x->mana -= v6x->mana * percent / 100;      // ── DRAIN the victim's CASTLE ──
                v29 = radix(thiefCastle.pitch² + thiefCastle.roll²);   // scatter radius from thief-castle extents
                while (v33 > 0) {                            // scatter the drained mana as (10,39) spheres
                    v30 = min(v33, 500);  v33 -= v30;        //   ≤500 mana per sphere
                    v26x = thiefCastle.position;
                    a1x->rand = 9377*a1x->rand + 9439;  yaw = rand & 0x7FF;
                    MoveEntity_57FA0(&v26x, yaw, 0, v29);    //   random bearing at castle-radius
                    v26x.z = getTerrainAlt(&v26x) + (4<<8);
                    v17x = IfSubtypeCallCreatingManaSphere_4A190(&v26x, 10, 39);   // (10,39) mana sphere
                    if (v17x) {
                        ... give it a random yaw/speed/arc (9377/9439 LCG), word_0x2C_44 = 128 ...
                        v17x->mana = v30;
                        if (SPELLS[13].subspell[tier].life_0x1A == 2)
                            v17x->playerEntityIndex = v34x;   // mode 2 → spheres OWNED BY THIEF
                        else
                            v17x->playerEntityIndex = 0;      // mode 1 → NEUTRAL spheres
                    }
                }
                if (!v35) goto LABEL_23;
                v5 = SPELLS[13].subspell[tier].subSpellIndex_2;
            }
            v33 = v5;
        }
        else { v5 = SPELLS[13].subspell[tier].subSpellIndex_2; v35 = 1; v33 = v5; }  // life_0x1A==0 → flat move
    }
LABEL_23:
    if (v35 && v33) {                                  // ── FLAT FALLBACK: move v33 mana thief←victim ──
        v34x->mana += v33;                             //   thief GAINS
        a1x->mana  -= v33;                             //   victim LOSES  (the wizard's own mana)
    }
    clamp(v34x->mana, 0, v34x->maxMana);               // clamp both
    clamp(a1x->mana,  0, a1x->maxMana);
    a1x->PlayerHitFrameTime_406 = 4;                   // victim hit-flash
    a1x->dword_0x18D_397 = 16;
    a1x->word_0x24C_588 = 64;
LABEL_35:
    sub_5EF70(a1x);                                    // clear the mailbox source
    a1x->str_0x5E_94.word_0x74_116 = 0;                // ← clear the steal channel
    return a1x;
}
```

**WHAT IT DOES:**
- **To the VICTIM:** if the victim owns a castle and `life_0x1A != 0`, it drains `subSpellIndex_2`% of the
  **victim's CASTLE mana** (not the wizard's carried mana) and scatters it as (10,39) spheres around the
  THIEF's castle. If there is no castle (or `life_0x1A == 0`), it instead moves a flat `subSpellIndex_2`
  amount directly off the victim's own carried `mana_0x90_144`. Applies a hit-flash + palette effect.
- **To the THIEF:** gains the scattered spheres' mana (mode 2 = spheres tagged `playerEntityIndex = thief`
  so they census to him; mode 1 = neutral spheres anyone can grab), OR the flat amount added straight to
  `mana_0x90_144`. Also earns +1 steal-mana XP (`sub_6D8B0(thief, 0xD, 1)`).
- The channel is consumed (`word_0x74_116 = 0`) so it fires once per hit.

**Field homes:** `subSpellIndex_2` (Spells.h:8) = the steal **percent/amount** per tier;
`life_0x1A` (Spells.h:15) = the **destination mode** (0 flat / 1 neutral-scatter / 2 thief-owned-scatter).

---

## 6. `dword_0x16D_365` — the 2000-init countdown (brain §9) — SETTLED: VESTIGIAL

Exhaustive grep (`dword_0x16D_365`, all .cpp/.h):

| site | code | role |
|---|---|---|
| gtypes:265 | `int32_t dword_0x16D_365;` | field decl |
| EF:43713 | `v2x->…->dword_0x16D_365 = 2000;` | **init 2000** at spawn (both human+AI, common stat block) |
| EF:5436-5437 | `if (…dword_0x16D_365) …dword_0x16D_365--;` | **AI tick** decrement (housekeeping `sub_12A70`) |
| EF:60014-60015 | `if (…dword_0x16D_365) …dword_0x16D_365--;` | **human tick** decrement (`AddPlayer03_00_5E010`) |
| engine_support.cpp:688 | `S164SC(dword_0x16D_365, 4);` | save serialize |
| engine_support_converts.cpp:122 | `memcpy(output + 0x16d, …, 4)` | save convert |

**There is NO reader that branches on its value** — nothing compares it, gates on it, or consumes it beyond
the self-decrement. It counts 2000→0 once after spawn and then sits at 0, persisted in saves. It is a
**dead/vestigial timer** in this build (plausibly a cut post-spawn immunity window; the live grace is the
separate `word_0x159_345 = 100`). Model it as a plain counter or omit it — no behavior depends on it.

---

## 7. HUMAN AUTHORED STARTING CASTLE (lifecycle §10) — SETTLED: THE HUMAN GETS NONE

Exhaustive grep of `player_0x2FED9` (all .cpp/.h):

| site | context | consumer? |
|---|---|---|
| BasicTerrain.h:77 | `int8_t player_0x2FED9[8];` | decl (the per-color authored starting-castle LEVEL) |
| ConvertMapInfo.cpp:10, Basic.cpp:3107/3300, engine_support_converts.cpp:565 | map load / save copies | data plumbing, no behavior |
| **EF:43777** | `if (player_0x2FED9[color])` | **inside `if (IsAiPlayer == 1)`** (EF:43761) |
| **EF:43789** | `v23 = player_0x2FED9[color]` (the BUILD00 stamp loop bound) | **same AI branch** |

Both runtime reads are inside the `IsAiPlayer == 1` gate of `sub_5C950`. The human's spawn path
(`v9 && IsAiPlayer==0`) never enters this branch, and no other function anywhere reads `player_0x2FED9`.

**CONCLUSION:** a level that authors `player_0x2FED9[human] = 7` (level-067) gives the HUMAN **nothing** —
the byte is loaded and copied but only the AI branch acts on it. **The MC2 human must cast Create-Castle to
obtain a castle; there is no authored/free starting castle for the human.** (This corrects the MC1 relic
where the human got a free instant first castle.) The level-067 human `player_0x2FED9 = 7` value is either
inert dead data or is authored expecting the human to build up to it, but the code plants no castle for it.

---

## PORT CONSEQUENCES

1. **Water steer (§1):** `rival_movement` must gain the full 4-neighbour `mapTerrainType == 8` probe with the
   verbatim bit-mask (N=1, fwd-side=2, S=4, back-side=8), the packed `(exit<<8|mask)` return, the two yaw
   tables `x_WORD_D3FCE`/`x_WORD_D3FE8` and the four `x_BYTE_D3F9x` step-delta tables, the 40-step ray-march
   (`sub_16CA0`) + Bresenham LOS (`sub_16E70`), and the `byte_0x45E_1118`/`_1119` micro-FSM (detect→commit
   L/R→hold ~5 ticks→re-detect) that zeroes speed on any yaw change. Copy the tables byte-for-byte.

2. **Scout-site metric (§2):** `sub_583B0` is Chebyshev `max(|dx|,|dy|)` RAW world units — the site gate is
   `> 12288` world units (48 tiles), NOT `12288²` squared-Euclid. Fix `rival_scout_site`'s foreign-castle
   distance test.

3. **Reactive defense, not pickup (§3):** DELETE the project's `rival_learn_tick` (MC1 200-tick jar timer)
   for MC2 AND do not model any runtime spell acquisition. Instead add a decision-cadence reactive-defense:
   scan class-9 entities aimed at self (`word_0x96_150 == self.id`) within 5120², jink (strafe=80), and if
   within 1024² cast shield(8)+recover(6) by threat model. AI book/tiers are load-time only (lifecycle §3).

4. **Book remap (§4):** `spellIndex_D94FF` is identity for the 26 spells — the MC2 book grant/tier masks
   index by raw spell id, no permutation needed. (Only the 3 CTRL-pane icon aliases 26/27/28 → 0/3/0.)

5. **Steal-mana (§5):** if MC2 rivals/human cast spell 0xD, port `sub_61050`: it drains
   `SPELLS[13].subspell[tier].subSpellIndex_2`% of the **victim's CASTLE** mana (fallback: flat off carried
   mana), scatters it as (10,39) spheres at the thief's castle radius (mode `life_0x1A`: 1=neutral,
   2=thief-owned), and awards the thief +1 XP. Channel = victim mailbox `word_0x74_116` (thief id) +
   `dword_0x70_112` (tier).

6. **`dword_0x16D_365` (§6):** vestigial 2000→0 countdown with no reader — model as a plain counter or omit;
   nothing observable depends on it. Do NOT invent a behavior for it.

7. **Human starting castle (§7):** the MC2 human gets NO authored starting castle — `player_0x2FED9` is
   consumed only in the AI branch of `sub_5C950`. The port must NOT plant a human castle at level load from
   `player_0x2FED9[human]`; the human casts Create-Castle like MC2 AI does at runtime.
