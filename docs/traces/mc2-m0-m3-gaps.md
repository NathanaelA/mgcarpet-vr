# MC2 Class-5 Models 0 (worm/hydra) & 3 (multipart flyer) — OPEN-Gap Verbatim Trace

Companion to `docs/traces/mc2-multipart-chains.md` (§1.0, §1.3, §2, §6). This closes the OPEN items. All `sub_XXXXX = address − 0x1E1000`. Cites: EF = `EventsFunctions.cpp`, EV = `Events.cpp`. Field names as in decomp.

---

## 0. DISPATCH TABLE — the ground truth (EF:1242 `x_DWORD_D4C52ar_str50[236]`)

The state→handler map for classes is a data table of 4-DWORD records `{groupKey, actionIndex, handlerAddress, enabledFlag}`. This IS the function-pointer table the engine calls via `str_D4C48ar[class].dword_10[actionIndex].address_6` (EV:435/466/…). Verbatim entries for m0 (states 0x00-0x07) and m3 (states 0x18-0x1F):

```
// m0 — states 0x00..0x07  (EF:1243-1250)
0x002A5BC8,0x0000,0x001FFF20,0x00000001   // sub_1EF20
0x002A5BC8,0x0001,0x001FFF40,0x00000001   // sub_1EF40
0x002A5BC8,0x0002,0x001FFF70,0x00000001   // sub_1EF70
0x002A5BC8,0x0003,0x001FFFD0,0x00000001   // sub_1EFD0
0x002A5BC8,0x0004,0x00200000,0x00000001   // sub_1F000
0x002A5BC8,0x0005,0x00200020,0x00000001   // sub_1F020
0x002A5BC8,0x0006,0x002002B0,0x00000001   // sub_1F2B0  <-- NO recovered body
0x002A5BC8,0x0007,0x00200300,0x00000001   // sub_1F300

// m3 — states 0x18..0x1F  (EF:1267-1274)
0x002A5BD4,0x0018,0x00200950,0x00000001   // sub_1F950
0x002A5BD4,0x0019,0x00200970,0x00000001   // sub_1F970
0x002A5BD4,0x001A,0x00200990,0x00000001   // sub_1F990
0x002A5BD4,0x001B,0x002009E0,0x00000001   // sub_1F9E0
0x002A5BD4,0x001C,0x00200A00,0x00000001   // sub_1FA00
0x002A5BD4,0x001D,0x00200A20,0x00000001   // sub_1FA20
0x002A5BD4,0x001E,0x00200A40,0x00000001   // sub_1FA40  <-- NO recovered body
0x002A5BD4,0x001F,0x00200A50,0x00000001   // sub_1FA50
```

Every slot's `enabledFlag = 1` (both 0x06 and 0x1E are LIVE states, not disabled). The child segment state is `0x00E8,0x001FC6B0` = `sub_1B6B0` (EF:1475), `enabledFlag = 1`. Table sentinels 0xE9/0xEA (`0x00000000` address, EF:1476-77) confirm m27 branch/segment have no self-dispatch.

**This resolves the state-0x06 / state-0x1E questions:** state 0x06 → address `0x2002B0`, state 0x1E → address `0x200A40`. Both are real, enabled binary functions. **The decompiler failed to lift either of them** — there is no C body at EF for `sub_1F2B0` or `sub_1FA40` (grep of the whole tree: the only hits for `2002b0`/`200a40` are the table entries themselves). The reimplementation's address-keyed dispatch switch (EV:1193-1323) has cases for every neighbouring address (`0x1fff20, 0x200000, 0x200020, 0x200300` for m0; `0x200950…0x200a50` for m3) but **no `case 0x2002b0` and no `case 0x200a40`** — so in the reimplementation these two states are silently no-ops (fall through the switch, which has no default body).

---

## 1. `sub_1F0C0` (EF:11260) — m0/m3 ATTACK/TETHER ("lasso") — FULL VERBATIM

Called from m0 states 0x01/0x02/0x03/0x07 and m3 (only via the missing 0x1E — see §4) plus `sub_1F300` case-dispatch. Arg is `a2x` = the head entity (the `a1x = 0` at top is a dead local the decomp keeps).

```c
void sub_1F0C0(type_entity_0x6E8E* a2x)                       // EF:11260
{
    type_entity_0x6E8E* a1x = 0;                               // scan cursor (reused local)
    v2 = a2x->fontTypeIndex_0x3D_61;                           // EF:11277  cooldown/gate byte
    if (v2) {
        a2x->fontTypeIndex_0x3D_61 = v2 - 1;                   // EF:11280  decrement each active tick
        if (a2x->byte_0x46_70) {                               // EF:11281  a hook is HELD
            if (a2x->word_0x2C_44) {                           // EF:11283  orbit timer > 0
                v7x = Entities_EA3E4[a2x->word_0x24_36];       // EF:11285  the ORBITED entity
                if (v7x <= Entities_EA3E4[0]
                    || v7x->life_0x8 < 0
                    || v7x->struct_byte_0xc_12_15.byte[1] & 4) // dead/despawning
                {
                    a2x->byte_0x46_70 = 0;                     // EF:11288  release hook
                    a2x->word_0x24_36 = 0;
                } else {
                    if (a2x->word_0x24_36 & 1)                 // EF:11293  parity of orbited id → side
                        v8 = v7x->yaw_0x1C_28 + 512;           //  +90°
                    else
                        v8 = v7x->yaw_0x1C_28 - 512;           //  -90°
                    predictedAxis_EB398ar = a2x->position_0x4C_76;
                    MoveEntity_57FA0(&predictedAxis_EB398ar,
                                     v8 & 0x7FF, 0,
                                     48 * a2x->word_0x2C_44);   // EF:11298  orbit radius = 48*timer
                    CopyEntityPosition_57CF0(a2x, &predictedAxis_EB398ar);
                    a2x->word_0x2C_44--;                        // EF:11300  wind orbit inward each tick
                }
            } else {                                            // orbit timer hit 0
                a2x->byte_0x46_70 = 0;                          // EF:11305  release hook
                a2x->word_0x24_36 = 0;
            }
        } else {                                                // NO hook held → SCAN for a projectile to grab
            v11 = (a2x->position_0x4C_76.x + 128) >> 8;         // EF:11311  self tile x (rounded)
            v12 = (a2x->position_0x4C_76.y + 128) >> 8;         // self tile y
            v3 = AddE7EE0x_10080(0, 4);                         // EF:11313  open a radius-4 tile spiral iterator
            if (v3) {
                v13 = 0;                                        // found flag
LABEL_11:    while (!v13 && sub_10130(v3, &v10, &v9) == 1) {    // EF:11318  next (dx=v10, dy=v9) tile
                    for (i = mapEntityIndex_15B4E0[((uint8)(v9+v12) << 8) + (uint8)(v10+v11)]; ;
                         i = a1x->oldMapEntity_0x16_22)          // walk that tile's entity list
                    {
                        a1x = Entities_EA3E4[i];
                        if (a1x == Entities_EA3E4[0]) break;
                        if (a1x->class_0x3F_63 == 9
                            && a1x->word_0x96_150 == a2x->id_0x1A_26) {  // EF:11327  a class-9 TARGETING me
                            v13 = 1;
                            goto LABEL_11;                       // break out with a1x = that projectile
                        }
                    }
                }
                ResetEvent08_10100(v3);                          // EF:11334  free iterator
                if (v13) {                                       // acquired
                    v5 = a1x - D41A0_0.struct_0x6E8E;            // projectile index
                    a2x->word_0x2C_44 = 5;                       // EF:11339  orbit timer = 5
                    a2x->byte_0x46_70++;                         // EF:11340  hook count++ (now >0)
                    a2x->word_0x24_36 = v5;                      // EF:11341  remember orbited projectile
                }
            }
        }
    }
}
```

### What this DOES (gameplay)
It is a **deflect-and-orbit ("lasso") of an incoming projectile**:

- `word_0x24_36` = the id of the class-9 entity being orbited. **In the m0 ctor it is left 0** (never assigned; see §8). It is set ONLY here (EF:11341) to a projectile the head captured.
- The scan (EF:11318-11333) sweeps a radius-4 tile spiral around the head and grabs the first **class-9** entity whose **`word_0x96_150 == self.id`**. Since `word_0x96_150` is the projectile's homing-TARGET id (confirmed: it is read as `Entities_EA3E4[a1x->word_0x96_150]` = the victim throughout the class-9 code, e.g. EF:5585/6810/7189/10578), a class-9 with `word_0x96_150 == head.id` is **a projectile flying AT the head**. The head snatches it, sets `word_0x2C_44 = 5` and `byte_0x46_70++`, and for the next 5 ticks whips it around itself at radius `48*word_0x2C_44` (240→48 as the timer decays), offset ±90° off the projectile's own yaw by the parity of its id.
- The mechanic is gated by `fontTypeIndex_0x3D_61` (a cooldown byte, 0 in ctor → gate initially CLOSED; something else must load it — not set in these handlers, so in practice this path is dormant until `fontTypeIndex` is nonzero from elsewhere).

### Which class-9 m0/m3 FIRE, and from where (the attack callback `sub_1CC20`)
The head's own attack is NOT this tether. It is `sub_1C310(head, aggro, sub_1CC20)` from state 0x02 (m0, EF:11205) / 0x1A (m3, EF:11595). `sub_1C310` (EF:9240) applies pending damage, walks the child chain for the min-life (tail-shot-kills-worm), and if the target is in range calls the callback:

```c
signed int sub_1CC20(type_entity_0x6E8E* a1x, type_entity_0x6E8E* a2x)   // EF:9680
{                                                                        // a1x=head, a2x=victim
    v4x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position, 9, 0);    // EF:9690  spawn (class 9, subtype 0)
    if (v4x) {
        v4x->byte_0x43_67 = 10;
        v4x->byte_0x44_68 = 0;
        v4x->id_0x1A_26   = a1x->id_0x1A_26;                              // proj.id = HEAD id
        v4x->yaw_0x1C_28  = sub_581E0_maybe_tan2(&a1x->pos, &a2x->pos);   // aim at victim
        v5x->pitch_0x1E_30= sub_58210_radix_tan(&a1x->pos, &a2x->pos);
        v5x->position.z  += a1x->array_0x52_82.fov;                       // muzzle raise = the head's fov metric
        v5x->word_0x96_150= a1x->word_0x96_150;                           // proj target = HEAD's target
        v5x->dword_0xA0_160x = &str_D7BD6[65];                            // anim/type row 65
        v5x->xsubtype_0x42_66 = a2x->model_0x40_64;                       // remember victim model
        v5x->subSpellIndex_0x2A_42 = 500;                                 // EF:9704  subSpellIndex = 500
        v5x->xtype_0x41_65 = a2x->class_0x3F_63;                          // victim class
        sub_5EF70(a2x);                                                   // flash victim's HUD if human
        return 1;
    }
    return 0;
}
```

So **both m0 and m3 fire a `(class 9, subtype 0)` mana-sphere projectile with `subSpellIndex_0x2A_42 = 500`**, launched from the head position, `+fov` z-raise, id = head id, homing target = head's current `word_0x96_150`. `IfSubtypeCallCreatingManaSphere_4A190(pos, 9, 0)` (EV:5186) only spawns if `str_D4C48ar[9].dword_14[0]` is a valid registered subtype.

**Cross-check / subtlety:** the tether scan grabs a class-9 whose `word_0x96_150 == head.id`, i.e. an incoming projectile aimed AT the head. `sub_1CC20`'s own projectile has `word_0x96_150 = head's target` (NOT head.id), so the head does **not** capture its own attacks — it lassoes projectiles fired at it (e.g. a player's fireball or another creature's shot). This is a defensive deflect, distinct from the (9,0) offensive shot.

RNG draws in `sub_1F0C0`: **none** (the tile-scan is deterministic). `sub_1CC20`: none directly (spawn may draw internally in `4A190`, out of scope). `sub_1C310`: none in the m0/m3 path shown. Sounds: none in `sub_1F0C0`/`sub_1CC20`.

---

## 2. `sub_1F040` (EF:11233) — vertical BOB — VERBATIM (summary VERIFIED, with exact clamps)

```c
void sub_1F040(type_entity_0x6E8E* a1x)                          // EF:11233
{
    a1x->position_0x4C_76.z += a1x->dword_0x10_16;               // EF:11238  apply velocity
    v1 = getTerrainAlt_10C40(&a1x->position_0x4C_76);            // EF:11239  ground under head
    a1x->dword_0x10_16 -= 5;                                     // EF:11240  gravity: velocity -= 5/tick
    result = v1 + 256;
    if (a1x->position_0x4C_76.z >= result) {                     // EF:11242  ABOVE ground+256
        if (isCaveLevel_D41B6) {                                 // EF:11244  only cave enforces a ceiling
            result = sub_10C60(&a1x->position_0x4C_76);          //  cave ceiling alt
            if (a1x->position_0x4C_76.z > (int16)result - 256)   // EF:11247  within 256 of ceiling
                a1x->dword_0x10_16 = -150;                       // EF:11248  slam DOWN
        }
    } else {                                                     // BELOW ground+256
        a1x->dword_0x10_16 = 150;                                // EF:11253  bounce UP
    }
}
```

Verified against the existing summary — exact. Points to note for the port:
- Gravity is `-5` **per tick, unconditionally** (EF:11240), applied every call regardless of altitude.
- Floor bounce sets velocity to `+150` whenever `z < terrain+256` (EF:11253).
- The ceiling clamp (`-150`) fires **only on cave levels** (`isCaveLevel_D41B6`, `sub_10C60` = cave ceiling) and only when `z > ceiling - 256`. On open levels there is NO upper clamp — the head keeps rising until gravity turns it over.
- No RNG, no sound.

---

## 3. `sub_1F2B0` — m0 state 0x06 — UNRECOVERED (address `0x2002B0`)

**The decompiler produced NO body for this function.** The table (EF:1249) proves it exists at `0x2002B0`, `enabledFlag = 1`, and the address-switch in the reimplementation (EV) has **no case for it**. There is no readable pseudocode, no comment stub, nothing at that address anywhere in the tree.

What we CAN infer strictly from structure (flag OPEN — do not treat as verbatim):
- By the m0 slot pattern, states are `patrol / idle / chase-attack / pack / prekill / kill / [0x06] / spawn`. Slots 0x04=prekill, 0x05=kill, 0x07=spawn are accounted. 0x06 sits between kill (0x05) and spawn (0x07).
- The parallel m1 (goat) table (EF:1251-1258) has, at the same relative offset (state 0x0E = base+6): `HitGoat_1F530` → `sub_1C980(entity, 8)` (the **FLEE** primitive) + actSpeed bump + a `(rand%0x2B)==0` SOUND 46. The parallel m3 slot (0x1E) is also unrecovered (§4). Given the goat's +6 slot is the flee/hit state (`sub_1C980`), **the most probable identity of m0 state 0x06 is the flee/hit variant wrapping `sub_1C980` with aggro-code 0**, i.e. `sub_1C980(head, 0)` plus an actSpeed bump. This matches the brief's hypothesis but **cannot be confirmed from source** — it is an educated structural guess.
- `sub_1C980` (EF:9572) is the flee primitive (takes `(entity, aggroCode)`), confirmed present.

**Port guidance:** since the retail binary DOES run this state (enabled) but we have no pseudocode, and the original reimplementation treats it as a no-op, the faithful-but-safe choice is to leave it a no-op OR implement `flee(aggro=0)` and gate it behind a telemetry flag. Recorded gameplay of the worm reaching state 0x06 is the only way to disambiguate.

---

## 4. m3 state 0x1E `sub_1FA40` — UNRECOVERED (address `0x200A40`) — resolves the aliasing question

**Definitive:** state 0x1E maps to `0x200A40` (EF:1273, `enabledFlag=1`). It is **NOT** an alias of `sub_1F990` (which is `0x200990`, state 0x1A). It is its own distinct function at `0x200A40` that **the decompiler failed to lift** — the EF source jumps straight from `sub_1FA20` (0x200A20, EF:11613) to `sub_1FA50` (0x200A50, EF:11619) with nothing between, and the reimplementation switch (EV:1316-1320) jumps `case 0x200a20 → case 0x200a50` with **no `case 0x200a40`**.

Structural inference (OPEN):
- m3 slots: `0x18 patrol / 0x19 idle / 0x1A chase-attack / 0x1B pack / 0x1C prekill / 0x1D kill / 0x1E [?] / 0x1F spawn`. The layout is **identical** to m0 (base 0x18 vs base 0x00). So m3's 0x1E is the same slot as m0's 0x06.
- Therefore m3 0x1E `sub_1FA40` is almost certainly the **flee/hit variant with aggro-code 24** (`sub_1C980(head, 24)`), mirroring m0 0x06's `sub_1C980(head, 0)` and the goat's `HitGoat_1F530`→`sub_1C980(entity,8)`.

**The earlier trace's claim "no sub_1FA40 exists / it aliases sub_1F990" is corrected:** it exists as a real, enabled, distinct handler at 0x200A40; it is simply unrecovered. Both m0 0x06 and m3 0x1E are the same unrecovered "flee" slot.

---

## 5. m0 states 0x00-0x07 — handler verification (all VERBATIM) + `sub_1F300`

All confirmed against the dispatch table (§0) AND the EF bodies:

| state | addr | handler | body (EF) |
|---|---|---|---|
| 0x00 | 0x1FFF20 | `sub_1EF20` | `sub_1BD90(a1x, 0)` — patrol, aggro 0 (EF:11189) |
| 0x01 | 0x1FFF40 | `sub_1EF40` | `sub_1BF90(a2x,0); sub_1F0C0(a2x); sub_1F040(a2x)` (EF:11195) |
| 0x02 | 0x1FFF70 | `sub_1EF70` | `if sub_1C310(a2x,0,sub_1CC20) PrepareEventSound_6E450(self,-1,8); sub_1F0C0(a2x); sub_1F040(a2x)` (EF:11203) — **SOUND 8** |
| 0x03 | 0x1FFFD0 | `sub_1EFD0` | `sub_1C560(a2x,0); sub_1F0C0(a2x); sub_1F040(a2x)` — pack (EF:11213) |
| 0x04 | 0x200000 | `sub_1F000` | `PreKillEntity_1C890(a1x, 0)` (EF:11221) |
| 0x05 | 0x200020 | `sub_1F020` | `KillEntity_1C930(a1x)` (EF:11227) |
| 0x06 | 0x2002B0 | `sub_1F2B0` | **UNRECOVERED** (§3) |
| 0x07 | 0x200300 | `sub_1F300` | see below (EF:11352) |

`sub_1F300` (state 0x07) — VERBATIM:
```c
void sub_1F300(type_entity_0x6E8E* a2x)                    // EF:11352
{
    sub_1D5D0(a2x, 0);                                     // spawn/appear primitive, aggro 0
    switch (a2x->StageVar2_0x49_73) {                      // EF:11358
    case 1: case 2: case 3: case 4: case 5:
    case 6: case 7: case 8: case 9: case 0xA:
    case 0xD: case 0xE: case 0x10:
        sub_1F0C0(a2x);                                    // EF:11373  tether
        goto LABEL_3;
    case 0x11:
    LABEL_3:
        sub_1F040(a2x);                                    // EF:11377  bob
        break;
    default:
        return;
    }
}
```
So StageVar2 in {1..0xA, 0xD, 0xE, 0x10} → tether **and** bob; StageVar2 == 0x11 → bob only; all else → nothing. No RNG; no sound (SOUND 8 is only in 0x02).

**m3 states 0x18-0x1F** (verified in EF:11581-11621, addresses from §0):
| state | addr | handler | body |
|---|---|---|---|
| 0x18 | 0x200950 | `sub_1F950` | `sub_1BD90(a1x, 24)` (EF:11581) |
| 0x19 | 0x200970 | `sub_1F970` | `sub_1BF90(a1x, 24)` (EF:11587) |
| 0x1A | 0x200990 | `sub_1F990` | `if sub_1C310(a1x,24,sub_1CC20) PrepareEventSound_6E450(self,-1,8)` (EF:11593) — **SOUND 8** |
| 0x1B | 0x2009E0 | `sub_1F9E0` | `sub_1C560(a1x, 24)` (EF:11601) |
| 0x1C | 0x200A00 | `sub_1FA00` | `PreKillEntity_1C890(a1x, 24)` (EF:11607) |
| 0x1D | 0x200A20 | `sub_1FA20` | `KillEntity_1C930(a1x)` (EF:11613) |
| 0x1E | 0x200A40 | `sub_1FA40` | **UNRECOVERED** (§4) |
| 0x1F | 0x200A50 | `sub_1FA50` | `sub_1D5D0(a1x, 24)` (EF:11619) |

Note: m3's head states do NOT call `sub_1F0C0`/`sub_1F040` (unlike m0). Only m0 states 0x01/0x02/0x03/0x07 run the tether+bob. m3 flies via the segment chain / different metrics; its heads are thin primitive wrappers. So the tether+bob is **an m0-only surface behavior** among the recovered states (m3 might reach it via the unrecovered 0x1E, unknown).

---

## 6. Sound 46 sites (EF:11391/11403/11423/11448/11457) — RESOLVED to **model 1 (goat)**

These lines lie between m0's 0x07 (`sub_1F300`, ends EF:11383) and m3's 0x18 (`sub_1F950`, EF:11581). The enclosing functions are **model-1 (goat) states 0x08-0x0F**, dispatch-table addresses `0x200340…0x2005B0` (EF:1251-1258), all aggro-code **8**:

| EF:line | enclosing fn | state | body | sound-46 condition |
|---|---|---|---|---|
| 11391 | `sub_1F340` (0x200340) | 0x08 | `sub_1BD90(a1x, 8)` + rand + `actionIndex==14→actSpeed=minSpeed` | `if !(rand % 0x4D)` |
| 11403 | `sub_1F3C0` (0x2003C0) | 0x09 | `sub_1BF90(a1x, 8)` + … | `if !(rand % 0x4D)` |
| 11423 | `sub_1F470` (0x200470) | 0x0B | `sub_1C560(a1x, 8u)` + … | `if !(rand % 0x4D)` |
| 11448 | `HitGoat_1F530` (0x200530) | 0x0E | `sub_1C980(a1x, 8)` + `actionIndex!=14→actSpeed=maxSpeed` | `if !(rand % 0x2B)` |
| 11457 | `AddGoat05_01_1F5B0` (0x2005B0) | 0x0F | `sub_1D5D0(event, 8)` + … | `if !(rand % 0x4D)` |

Each site first advances RNG: `rand_0x14_20 = 9377*rand + 9439` (one draw), then `PrepareEventSound_6E450(self, -1, 46)` when the modulus hits 0. **These belong to MODEL 1 (the goat/vulture slot), NOT model 0.** The decomp itself names them `HitGoat/PreKillGoat/KillGoat/AddGoat05_01` (EF:11429-11462). The prior trace's label "m0 goat variants" was wrong: they are model 1. State 0x08 is the goat's patrol; state 0x0E (`actionIndex==14`) is its flee/hit. (Note the missing goat states 0x0A/0x0C/0x0D → `sub_1F440` (0x200440, `actionIndex=14; HitGoat`), `PreKillGoat_1F4F0`, `KillGoat_1F510`.)

RNG draw count per site: exactly **one** advance before the modulus test.

---

## 7. Head death → children — CORRECTION: `PreKillEntity_1C890` IS chain-aware

The earlier trace claimed children only self-hide via `sub_1B6B0`. **That is incomplete.** `PreKillEntity_1C890` explicitly walks the child chain and cascades a kill-state:

```c
void PreKillEntity_1C890(type_entity_0x6E8E* entity, char state)      // EF:9533
{
    int i = entity->word_0x34_52;                                     // first child
    while (Entities_EA3E4[i] != Entities_EA3E4[0]) {                  // walk forward chain
        Entities_EA3E4[i]->actionIndex_0x45_69 = state + 5;           // EF:9538  child → kill state
        if (Entities_EA3E4[i]->word_0x24_36)                          // propagate any lasso ref up
            entity->word_0x24_36 = Entities_EA3E4[i]->word_0x24_36;
        i = Entities_EA3E4[i]->word_0x34_52;
    }
    tempEntity2 = Entities_EA3E4[entity->word_0x24_36];               // EF:9543  kill-credit target
    if (tempEntity2 > Entities[0] && tempEntity2->class==3 && !tempEntity2->model
        && entity->id != tempEntity2->id)
    {
        if (entity->model != 12 && != 13 && != 14 && != 15 && != 9)   // EF:9549  exclude those models
            tempEntity2->dword_0xA4_164x->creaturesKilledPercent_373++;
    }
    entity->actionIndex_0x45_69 = state + 5;                          // EF:9552  head → kill state
}
```

For m0: `state = 0` → children set to `actionIndex = 5` (= m0 kill state `sub_1F020` → `KillEntity_1C930`), head → 5. For m3: `state = 24` → children set to `actionIndex = 29 = 0x1D` (= m3 kill state `sub_1FA20` → `KillEntity_1C930`), head → 0x1D. So **head death DOES cascade a KillEntity state to every child**, walking `word_0x34_52` head→tail. This runs once, in a single call, the tick the head enters state 0x04 (m0) / 0x1C (m3).

But children are in state **0xE8** until this walk overwrites their `actionIndex`. Once overwritten to 5/0x1D, the main loop dispatches them to `KillEntity_1C930`:

```c
void KillEntity_1C930(type_entity_0x6E8E* entity)                    // EF:9556
{
    if (!(entity->byte_0x3E_62 & 7)) {                                // EF:9558  ONLY every 8th index
        TransformEntityToManaSphere_36BA0(entity, false);            // convert to mana sphere
        if (!(entity->struct_byte_0xc[2] & 0x10)) {
            tempEntity = IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 1);  // class-10 subtype-1 effect
            if (tempEntity) tempEntity->id = entity->id;
        }
        DisableEntityDrawing04_57F10(entity);
    }
}
```

**Consequence for children:** children carry `byte_0x3E_62 = childIndex` (0..15, set in ctor EF:33704). `KillEntity` only converts/hides when `!(childIndex & 7)` — i.e. **only child indices 0 and 8** drop a mana sphere and hide; indices 1-7, 9-15 do nothing in `KillEntity` and remain until list-rebuild culling drops them (they no longer dispatch to `sub_1B6B0` so they freeze in place until culled).

### `word_0x34_52` walk audit
- `PreKillEntity_1C890` (EF:9535) — walks `word_0x34_52`. **Chain-aware.** ✔
- `KillEntity_1C930` (EF:9556) — **no chain walk** (self only).
- `sub_1C310` (EF:9268-9284) — walks `word_0x34_52` to find the min-life child (damage propagation, "tail-shot kills the worm"): `if child.life < head.life: head.life = child.life; head.attacker = child.attacker`. This is how a segment hit registers on the head. Not a kill walk.

### Child segment life < 0 — who reaps it
`sub_1B6B0` (state 0xE8) applies `str_0x5E_94` damage to the segment's own `life_0x8`. Nothing in `sub_1B6B0` transitions the segment on `life<0`. The reaping is via the **list-rebuild skip** (multipart-chains §7, `case 0x5`: `if life_0x8 < 0: continue`) — a dead-life 0xE8 segment is simply never re-listed, so it's never scanned/collided. It is NOT explicitly killed or converted to mana. The head, meanwhile, absorbs the segment's low life via `sub_1C310`'s min-life walk (so damaging a tail segment enough drives the HEAD's life negative → head enters kill via `sub_1C310`'s `if life<0 → actionIndex = a2+4` path, EF:9285-9289/9338-9339).

**Net:** there IS an explicit chain-kill cascade for m0/m3 (via `PreKillEntity_1C890`) — but it only *converts/hides* children at indices 0 and 8; the rest are state-flipped and left for list-rebuild culling. The self-hide in `sub_1B6B0` (parent non-class-5) is a secondary safety net, not the primary path.

---

## 8. m0 `word_0x24_36`, `word_0x2C_44`, `dword_0x10_16` seeds & lifecycle (ctor EF:33642 verbatim-checked)

From the m0 ctor (EF:33662-33690):
- `word_0x2C_44 = 0` (EF:33685). Used as the **orbit timer** in the tether: set to 5 on hook acquire (EF:11339), decremented each orbit tick (EF:11300). Nowhere else written for m0. Radius = `48 * word_0x2C_44`.
- `byte_0x46_70 = 0` (EF:33686). The **hook-held flag/count**: `++` on acquire (EF:11340), `=0` on release (EF:11288/11305). (Distinct from m22 where 0x46 is tail length.)
- `word_0x24_36` — **NOT assigned in the ctor** → stays 0 from `NewEvent_4A050`'s `memset` (EV:565). Set ONLY in the tether (EF:11341) to the orbited projectile's index; cleared (EF:11289/11306) on release. Also used as the **kill-credit target** in `PreKillEntity_1C890` (EF:9543) — for m0 it's whatever projectile was last lassoed (or a child's propagated value, EF:9540), so worm kill-credit routes through the lassoed projectile's owner if any, else no credit.
- `fontTypeIndex_0x3D_61 = 0` (EF:33688). The tether's top-level gate (EF:11277). Zero in ctor ⇒ the tether is **dormant** until something writes a nonzero value; no recovered m0 handler writes it, so under the recovered code the lasso never activates (candidate mechanism lives in the unrecovered 0x06/0x1E or in a damage-intake path — OPEN).
- `dword_0x10_16 = (self − D41A0_0.struct_0x6E8E) % 100` (EF:33679) — the **bob phase/velocity seed**: the entity's slot index mod 100, used as the initial vertical velocity in `sub_1F040` (`z += dword_0x10_16`, then `-=5`/tick). So each worm starts its bob at a slot-dependent upward velocity in [0,99], desyncing multiple worms.
- `word_0x36_54 = 96` (EF:33677) — the segment **link length** (used by `sub_1B6B0` follow, and copied per-child; m3 first child gets 125%). Constant 96 for m0.
- RNG: the ctor advances **once** (EF:33672), feeding roll/yaw/pitch (all from the one draw). The child loop does NOT draw. The tether and bob draw nothing.

---

## OPEN / uncertainties

1. **m0 state 0x06 `sub_1F2B0` (addr 0x2002B0) and m3 state 0x1E `sub_1FA40` (addr 0x200A40) are UNRECOVERED.** Both are real, table-enabled (`flag=1`) binary functions the decompiler failed to lift; the reimplementation has no switch case for either (silent no-op). They occupy the same relative slot (base+6). Structural analogy to the goat's +6 slot (`HitGoat_1F530` = `sub_1C980` flee) strongly suggests both are **flee/hit variants** — `sub_1C980(head, 0)` for m0, `sub_1C980(head, 24)` for m3 — but this cannot be confirmed from source. Port options: (a) faithful no-op (matching the reimplementation), or (b) implement `flee(aggro)` behind a telemetry flag and validate against recorded worm/flyer flee behavior. Do NOT assert verbatim.

2. **`fontTypeIndex_0x3D_61` loader unknown.** The tether (`sub_1F0C0`) is gated on this byte being nonzero, but no recovered m0/m3 handler writes it (ctor sets 0). Whatever arms the lasso is either in the unrecovered 0x06/0x1E, in a damage-intake/aggro path outside this subsystem, or the byte is loaded from a spawn parameter. Until found, the recovered code path leaves the lasso dormant. Flag for a follow-up grep of writers to `fontTypeIndex_0x3D_61` on class-5 entities.

3. **Tether target semantics confirmed but rarely reachable:** the scan grabs a class-9 with `word_0x96_150 == head.id` (a projectile homing at the head). The head's own (9,0) attack (`sub_1CC20`) sets `word_0x96_150 = head's target`, so self-attacks are never lassoed — only incoming shots. Given item 2, this deflect is effectively inert under the recovered code.

4. **m3 heads don't run tether+bob among recovered states.** Only m0 states 0x01/0x02/0x03/0x07 call `sub_1F0C0`+`sub_1F040`. m3's recovered states (0x18-0x1D, 0x1F) are pure primitive wrappers. If m3 bobs/tethers, it's via the unrecovered 0x1E (item 1). Verify against recorded m3 flight whether it visibly bobs.

5. **Child kill cascade is partial:** `PreKillEntity_1C890` flips all children to the kill state, but `KillEntity_1C930` only converts/hides children whose `byte_0x3E_62 & 7 == 0` (indices 0 and 8 of 16). Indices 1-7/9-15 are state-flipped but not converted — they stop following and are culled by list-rebuild once off-list/negative-life. Confirm in play that a killed worm drops exactly the expected mana spheres (should be ~2 from children + head's own drop) and that stray segments vanish rather than freezing visibly.

6. **`str_D7BD6[65]` (proj anim row), `subSpellIndex 500`, `byte_0x43_67=10`** are the (9,0) projectile's registration — behavior of that projectile (speed/life/homing caps) lives in the class-9 subtype-0 tables; port keys it off `subSpellIndex_0x2A_42 = 500` and `xtype/xsubtype` = victim class/model. (Matches the already-ported `mc2_atk_bolt`.)

7. **RNG accounting for the port:** m0 ctor = 1 draw (EF:33672); child loop = 0; `sub_1F0C0`/`sub_1F040`/`sub_1F300` = 0 draws each; goat sound-46 sites = 1 draw each before the modulus test (§6). Preserve this exact order — the worm ctor's single draw feeds roll/yaw/pitch together.
