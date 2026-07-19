# CLASS-10 Model 76 (0x4C) — `AddFireSpheres_4F2A0` — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Trace date 2026-07-10. Closes **OPEN-3** of `mc2-class10-m50-chains-and-tail.md` §5.1. Shared helpers (`NewEvent_4A050`, `AddEventToMap_57D70`, `SetEntityShiftRot_49EA0`, `SetEntityIndexAndRot_49CD0`, `sub_10C80`, RNG law `r=9377*r+9439`, dispatch model) are documented in `mc2-class10-m29-m5-m13.md` §0 and `mc2-class10-m50-chains-and-tail.md` §0 — not re-derived here.

---

## Headline finding (read first)

**(10,76)=0x4C — the "expanding/contracting rotating FIRE-SPHERE ORB" (a 3D-lissajous ring of 25 fire particles pulsing around a central hub).** The creator `AddFireSpheres_4F2A0` (EF:35936) makes ONE invisible **head/hub** (model 76, action **0x53** = 83) and immediately spawns **25 satellite fire spheres** (model 77 = 0x4D, action 0x54 = 84) wired into a `word_0x32/word_0x34` doubly-linked chain, each given a sprite-340 fire particle + an attached `AddEvent2` child. The head's action `sub_339B0` (EF:24562, addr 0x2149B0) is the ONLY ticking handler — **the 25 satellites have NO action handler** (strA0[0x54] = NULL/disabled, EF:1686). Each tick the head rotates the whole constellation (spins yaw/pitch of every satellite via `sub_33B20`), breathes the ring radius in and out between `maxSpeed_0x86_134` and `minSpeed_0x84_132` (`sub_33AD0`), clamps to terrain, and deals **type-0 AoE damage amount 70** through the satellites in ring-row-0 (`sub_33C00`→`sub_10C80(sat,0,70)`, sound 3 on hit). It lives `maxLife=80` ticks, then enters a shrink/collapse phase and spawns a **(10,0)** ground-fire splat on final collapse before tearing down the whole chain (`sub_33D40`). **RNG draws in the creator: 25 (one per satellite, for its per-sphere roll/fov jitter).**

172 records / 25 levels — the highest-count unported tail model. It is a decorative-yet-damaging orbiting fireball hazard.

---

## 0. Dispatch rows

| entity | model (dec/hex) | strA1 creator | creator fn (EF) | actionIndex set | strA0 action | action fn |
|---|---|---|---|---|---|---|
| **head/hub** | 76 / 0x4C | 0x2302A0 (EF:1678\*) | `AddFireSpheres_4F2A0` :35936 | **0x53 (83)** | 0x2149B0 (EF:1685) | `sub_339B0` :24562 |
| **satellite** | 77 / 0x4D | **NULL/disabled** (EF:1679, strA1 row 0x4D `0x00000000,0x00000000`) | — (never created as a THING) | **0x54 (84)** | **NULL/disabled** (EF:1686, strA0 row 0x54 `0x00000000,0x00000000`) | — (never ticks) |

\* strA1 (creator, keyed by MODEL): model 0x4C → `0x002302A0, enabled=1` (EF, `x_DWORD_D4C52ar_strA1` row 0x4C). strA0 (action, keyed by ACTIONINDEX): row 0x53 → `0x002149B0, enabled=1`; row 0x54 → `0x00000000, enabled=0`.

**Numbering-trap resolution (closes the OPEN-3 alias worry):** the old §5.1 note said "strA0[0x4C]→0x219D80 aliases `sub_38D80`". That is the strA0 row indexed by 0x4C, and it is IRRELEVANT — the ctor writes **actionIndex 0x53**, not 0x4C, so the head dispatches to `sub_339B0` (0x2149B0), NOT `sub_38D80`. Key by the actionIndex the ctor sets, exactly as the convention warns.

---

## 1. Creator `AddFireSpheres_4F2A0` (EF:35936, addr 0x2302A0) — VERBATIM

```c
type_entity_0x6E8E* AddFireSpheres_4F2A0(axis_3d* position)//2302a0
{
    type_entity_0x6E8E* entity = NULL;
    if (sub_4A810_get_0x35plus() >= 26)                 // GATE: need >=26 free entity slots (1 head + 25 sats)
    {
        entity = NewEvent_4A050();
        if (entity)
        {
            entity->actionIndex_0x45_69 = 83;           // 0x53 -> sub_339B0
            entity->class_0x3F_63 = 10;
            entity->model_0x40_64 = 76;                 // 0x4C
            entity->maxLife_0x4 = 80;
            entity->subSpellIndex_0x2A_42 = 70;          // DAMAGE amount (type-0)
            entity->actSpeed_0x82_130 = 40;
            entity->maxSpeed_0x86_134 = 192;             // ring radius UPPER bound (breathe max)
            entity->minSpeed_0x84_132 = 480;             // ring radius LOWER bound target (see note)
            entity->actSpeed_0x82_130 = 40;              // (written twice, identical)
            entity->byte_0x38_56 = 1;
            entity->byte_0x43_67 = 0;
            entity->byte_0x44_68 = 0;
            entity->word_0x2C_44 = 0;                    // current ring radius (init 0 -> set by sub_4F440)
            entity->fontTypeIndex_0x3D_61 = 0;           // radius breathe step (set to 18 by sub_4F440)
            type_entity_0x6E8E* entity3 = entity;        // chain-tail cursor (starts at head)
            entity->struct_byte_0xc_12_15.byte[0] &= 0xf6; // clear bits 0,3
            entity->struct_byte_0xc_12_15.byte[0] |= 1;    // set bit0 (active/collidable)
            CopyMaxLifeToLife_49A20(entity);             // life = 80
            for (int i = 0; i < 25; i++)                  // spawn 25 satellites
            {
                type_entity_0x6E8E* entity2 = NewEvent_4A050();
                if (entity2)
                {
                    qmemcpy(entity2, entity, sizeof(type_entity_0x6E8E));  // clone head
                    entity2->model_0x40_64 = 77;         // 0x4D
                    entity2->actionIndex_0x45_69 = 84;   // 0x54 (NO handler -> never ticks alone)
                    entity2->word_0x32_50 = entity3 - D41A0_0.struct_0x6E8E;  // prev-link
                    entity3->word_0x34_52 = entity2 - D41A0_0.struct_0x6E8E;  // next-link (into prev)
                    entity2->byte_0x3E_62 = i;
                    entity2->byte_0x43_67 = i / 5;       // RING (0..4): the shape-class of the satellite
                    entity2->word_0x34_52 = 0;           // terminate its own next-link (tail)
                    entity2->byte_0x44_68 = i % 5;       // SLOT within ring (0..4)
                    AddEventToMap_57D70(entity2, position);
                }
                entity3 = entity2;                       // advance tail cursor
            }
            AddEventToMap_57D70(entity, position);
            SetEntityShiftRot_49EA0(entity, 640, 640);   // head collision/damage extents 640
            sub_4F440(entity);                           // lay out the ring geometry (below)
        }
    }
    return entity;
}
```

### Creator facts
- **Gate:** `sub_4A810_get_0x35plus() >= 26` (free-slot count ≥ 26). Below that, **spawns nothing** and returns NULL.
- **RNG draws (creator):** **0 in `AddFireSpheres` itself**, but `sub_4F440` draws **1 per satellite = 25** (each satellite's `roll/fov` jitter, §1.1). Head itself never draws.
- **Head:** invisible (no `SetEntityIndex*` on the head), maxLife 80, subSpell 70, extents 640, byte[0] = (…&0xF6)|1 (bit0 set = collidable/active, bits 1..3 cleared → **NOT targetable**, bit3=0x08 cleared).
- **Chain:** classic runtime `word_0x32_50`(prev) / `word_0x34_52`(next) doubly-linked list, head→sat0→sat1→…→sat24, same mechanism as the (10,22) whirlwind tail. Head's `word_0x34_52` points at sat0; each sat's `word_0x32_50` points back, `word_0x34_52` chains forward, last sat's `word_0x34_52 = 0` terminates.
- **Satellites:** model 77, action 0x54 (**no handler → inert on their own**), cloned from the head so they inherit maxLife 80 / subSpell 70 / extents / byte[0]. `byte_0x43_67 = i/5` = ring index 0..4; `byte_0x44_68 = i%5` = slot 0..4. So the 25 spheres form a **5×5 lattice** of (ring, slot) coordinates.

### 1.1 `sub_4F440` (EF:35989, addr 0x230440) — ring geometry layout — VERBATIM

Sets the head's breathe parameters, then walks the satellite chain (`word_0x34_52`) assigning each sphere a fixed `(yaw base, pitch base, roll spin, fov spin)` from its `(byte_0x43_67 ring, byte_0x44_68 slot)`, positions it, gives it sprite 340, and attaches an `AddEvent2` child.

```c
__int16 sub_4F440(type_entity_0x6E8E* a1x)//230440
{
    v1 = a1x->maxSpeed_0x86_134;          // 192
    a1x->fontTypeIndex_0x3D_61 = 18;      // radius breathe STEP = 18 (overrides ctor's 0)
    a1x->word_0x2C_44 = v1;               // current radius := maxSpeed (192)
    a1x->yaw_0x1C_28 = 0;
    v2 = v1; LOWORD(v2) = a1x->word_0x34_52;   // v2 = index of sat0 (head's next-link)
    a1x->pitch_0x1E_30 = 0;
    while (1)
    {
        v13x = Entities_EA3E4[v2];
        if (v13x <= Entities_EA3E4[0]) return v2;   // end of chain
        v4 = v13x->byte_0x44_68;                     // slot (0..4)
        v13x->struct_byte_0xc_12_15.byte[0] &= 0xFE; // clear bit0 (satellite NOT collidable itself)
        if (v4)                                       // slot != 0
        {
            v13x->struct_byte_0xc_12_15.byte[2] |= 0x80u;  // set byte[2] bit7 (render/transparency flag)
            v13x->struct_byte_0xc_12_15.byte[0] &= 0xF7;   // clear bit3 (not targetable)
        }
        else                                          // slot == 0 (the DAMAGING row members)
        {
            v13x->struct_byte_0xc_12_15.byte[0] |= 8;      // SET bit3 (targetable/damage-active)
        }
        v13x->rand_0x14_20 = 9377 * v13x->rand_0x14_20 + 9439;   // 1 RNG draw per satellite
        v6 = (v13x->rand_0x14_20 & 0x3F) + 84;        // spin rate base 84..147 (0x54..0x93)
        v7 = v13x->byte_0x43_67;                       // ring (0..4)
        switch ((x_BYTE)v7)
        {
        case 0:                                        // ring 0: yaw fans across slots
            LOWORD(v7) = v13x->byte_0x44_68;           // slot
            v13x->pitch_0x1E_30 = 0;
            v13x->fov_0x22_34   = 0;                    // no pitch-spin
            v8 = 512 - 96 * v7;                         // yaw base = 512 - 96*slot
            v13x->roll_0x20_32  = v6;                   // yaw-spin rate = v6
            BYTE1(v8) &= 7u; v13x->yaw_0x1C_28 = v8;
            break;
        case 1:                                        // ring 1: yaw fixed 512, pitch fans, pitch spins
            v9 = 96 * v13x->byte_0x44_68;
            v10 = 512; v13x->yaw_0x1C_28 = 512;
            goto LABEL_11;
        case 2:                                        // ring 2: yaw 0, pitch fans down
            v11 = v13x->byte_0x44_68;
            v13x->yaw_0x1C_28 = 0; v13x->roll_0x20_32 = 0;
            v12 = -96 * v11;
            goto LABEL_12;
        case 3:                                        // ring 3: yaw base 256, pitch fans, pitch spins
            v9 = 96 * v13x->byte_0x44_68; v10 = 256;
            v13x->yaw_0x1C_28 = 256; goto LABEL_11;
        case 4:                                        // ring 4: yaw base 768, pitch fans, pitch spins
            v9 = 96 * v13x->byte_0x44_68; v10 = 768;
            v13x->yaw_0x1C_28 = 768;
        LABEL_11:
            v13x->roll_0x20_32 = 0;                     // no yaw-spin
            v12 = v10 - v9;                             // pitch base = yawbase - 96*slot
        LABEL_12:
            v13x->fov_0x22_34 = v6;                     // pitch-spin rate = v6
            HIBYTE(v12) &= 7u; v13x->pitch_0x1E_30 = v12;
            break;
        default: break;
        }
        predictedAxis_EB398ar = a1x->position_0x4C_76;
        MoveEntity_57FA0(&predictedAxis_EB398ar, v13x->yaw_0x1C_28, v13x->pitch_0x1E_30, a1x->word_0x2C_44); // place at (yaw,pitch,radius)
        CopyEntityPosition_57CF0(v13x, &predictedAxis_EB398ar);
        SetEntityIndexAndRot_49CD0(v13x, 340);          // SPRITE 340 (the visible fire sphere)
        AddEvent2_847D0(v13x, 128, 1, 0);               // attach a secondary child (visual/particle, type 1)
        v2 = v13x->word_0x34_52;                         // next satellite
    }
}
```

Geometry summary: `roll_0x20_32` / `fov_0x22_34` are **per-satellite spin rates** (yaw-spin and pitch-spin respectively), each `= (rand&0x3F)+84`; `yaw_0x1C_28` / `pitch_0x1E_30` are the per-satellite **base angles** laid out by (ring, slot). The 5 rings each fan their 5 slots across a different great-circle → a spherical constellation. Sprite **340** is the fire-sphere particle; each also gets an `AddEvent2_847D0(sat,128,1,0)` attached secondary. Only **slot-0** satellites (5 of them, one per ring) keep byte[0] bit3 set = the damage-carriers; the other 20 are pure visual (byte[0] bit3 cleared, byte[2] bit7 set).

---

## 2. Action handler `sub_339B0` (EF:24562, addr 0x2149B0) — the head tick — VERBATIM

The head is a **3-phase state machine** keyed on `byte_0x46_70` (0 = init, 1 = alive/pulsing, 2 = collapsing).

```c
void sub_339B0(type_entity_0x6E8E* a1x)//2149b0
{
    LOBYTE(v1) = a1x->byte_0x46_70;
    if ((unsigned __int8)v1 < 1u)                  // ---- PHASE 0: one-time init ----
    {
        if ((x_BYTE)v1) return;                    // (byte<0 guard; unreachable for 0)
        v2 = a1x->word_0x96_150;                   // OPTIONAL leader/anchor entity index
        if (v2)                                     // (0 for a fresh spawn -> skipped)
        {
            v3x = Entities_EA3E4[v2];
            a1x->maxSpeed_0x86_134 = v3x->array_0x52_82.pitch >> 1;        // radius from leader size
            v4 = a1x->maxSpeed_0x86_134;
            a1x->minSpeed_0x84_132 = 6 * v3x->array_0x52_82.pitch >> 2;
            if (v4 < 128) a1x->maxSpeed_0x86_134 = 128;
            if (a1x->minSpeed_0x84_132 > 640) a1x->minSpeed_0x84_132 = 640;
        }
        a1x->byte_0x46_70 = 1;                      // -> phase 1
    }
    else if ((unsigned __int8)v1 > 1u)             // ---- PHASE 2: collapse ----
    {
        if ((x_BYTE)v1 == 2)
        {
            v6 = a1x->fontTypeIndex_0x3D_61;
            if (v6 < 0) a1x->fontTypeIndex_0x3D_61 = -v6;    // force breathe-step positive (shrink dir)
            sub_33B20(a1x);                          // keep spinning the ring while it collapses
            v1 = a1x->fontTypeIndex_0x3D_61;
            v7 = a1x->word_0x2C_44 - v1;             // radius -= step
            a1x->word_0x2C_44 = v7;
            if (v7 < 0)                              // fully collapsed:
            {
                IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 0);  // spawn (10,0) GROUND FIRE
                sub_33D40(a1x);                      // tear down whole chain + head
            }
        }
        return;
    }
    // ---- PHASE 1: alive/pulsing (falls through from phase-0 init on first tick) ----
    sub_33C70(a1x);        // clamp to terrain / cave ceiling; follow leader; detect leader death
    sub_33AD0(a1x);        // breathe the ring radius between min/max
    sub_33B20(a1x);        // rotate the whole constellation + reposition every satellite
    sub_33C00(a1x);        // DAMAGE pass (slot-0 satellites)
    v5 = a1x->life_0x8 - 1;
    a1x->life_0x8 = v5;
    if (v5 < 1) a1x->byte_0x46_70 = 2;              // life exhausted -> begin collapse
}
```

**Key: `word_0x96_150` (leader/anchor) is 0 for a fresh (10,76) spawn** (NewEvent memset, ctor never sets it, `sub_4A310`/`PrepareEvents` never set it — §3, §4). So the phase-0 leader branch and `sub_33C70`'s leader-follow branch are **skipped** — the orb is a **free-floating, terrain-clamped** hazard sitting where it was spawned. (The `word_0x96_150` machinery exists for a code path that would attach the fire-sphere shell to another entity as an aura — no such caller exists for model 76 in this decompile; see OPEN-1.)

### 2.1 `sub_33AD0` (EF:24623) — radius breathing — VERBATIM
```c
void sub_33AD0(type_entity_0x6E8E* a1x)//214ad0
{
    v2 = a1x->fontTypeIndex_0x3D_61 + a1x->word_0x2C_44;   // radius += step
    v3 = a1x->minSpeed_0x84_132;
    a1x->word_0x2C_44 = v2;
    if (v2 <= v3) {                                         // hit/passed the LOWER bound...
        v5 = a1x->maxSpeed_0x86_134;
        if (v2 < v5) { a1x->word_0x2C_44 = v5; a1x->fontTypeIndex_0x3D_61 = -a1x->fontTypeIndex_0x3D_61; }
    } else {                                                // hit/passed the UPPER bound...
        a1x->word_0x2C_44 = v3; a1x->fontTypeIndex_0x3D_61 = -a1x->fontTypeIndex_0x3D_61;  // reverse step
    }
}
```
Radius (`word_0x2C_44`) walks by ±`fontTypeIndex_0x3D_61` (=±18) between the two bounds and bounces (sign-flips the step) at each — the ring **pulses in and out**. (Bounds as authored: `maxSpeed_0x86_134=192`, `minSpeed_0x84_132=480`; note min>max numerically, so the clamp arithmetic ping-pongs the radius across [192,480] — see the constants note. The visual effect is a breathing orb regardless of which label is nominally "min"/"max".)

### 2.2 `sub_33B20` (EF:24656) — rotate the constellation — VERBATIM
```c
void sub_33B20(type_entity_0x6E8E* a1x)//214b20
{
    a1x->yaw_0x1C_28   += 22;  HIBYTE(&yaw)  &= 7;   // head global yaw spin +22/tick
    a1x->pitch_0x1E_30 += 16;  HIBYTE(&pitch)&= 7;   // head global pitch spin +16/tick
    for (i = a1x->word_0x34_52; (v9x=Entities_EA3E4[i]) > Entities_EA3E4[0]; i = v9x->word_0x34_52)
    {
        v9x->yaw_0x1C_28   += v9x->roll_0x20_32;  // each sat spins its own yaw by its roll-rate
        v9x->pitch_0x1E_30 += v9x->fov_0x22_34;   // ... and its pitch by its fov-rate
        predictedAxis = a1x->position_0x4C_76;
        MoveEntity_57FA0(&predictedAxis,
             (v9x->yaw_0x1C_28   + a1x->yaw_0x1C_28)   & mask,   // sat angle + head angle
             (v9x->pitch_0x1E_30 + a1x->pitch_0x1E_30) & mask,
             a1x->word_0x2C_44);                                  // at current breathing radius
        CopyEntityPosition_57CF0(v9x, &predictedAxis);            // reposition satellite
    }
}
```
Every tick: head spins (+22 yaw, +16 pitch), each satellite spins on its own `(roll,fov)` rates, and all 25 are re-placed at `head.pos + spherical(satAngle+headAngle, radius)`. This is what makes the whole orb tumble while pulsing. **No RNG in the tick.**

### 2.3 `sub_33C00` (EF:24700) — DAMAGE pass — VERBATIM
```c
void sub_33C00(type_entity_0x6E8E* a1x)//214c00
{
    for (result = a1x->word_0x34_52; (v2x=Entities_EA3E4[result]) > Entities_EA3E4[0]; result = v2x->word_0x34_52)
    {
        if (!v2x->byte_0x44_68) {                                    // slot 0 only (5 of the 25)
            if (sub_10C80(v2x, 0, a1x->subSpellIndex_0x2A_42))       // AoE type-0, amount 70, at the sat's cell
                PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 3);   // SOUND 3 on any hit
        }
    }
}
```
Each tick the **5 slot-0 satellites** each call `sub_10C80(self, 0, 70)` = type-0 (`1<<0`) area damage of magnitude **70** at that sphere's current cell. If any lands, the head plays **sound 3** once. The other 20 spheres are purely visual. Damage source = the moving fire spheres, so the hazard footprint sweeps as the orb tumbles.

### 2.4 `sub_33C70` (EF:24722) — terrain clamp + leader tracking — VERBATIM
```c
void sub_33C70(type_entity_0x6E8E* a1x)//214c70
{
    v1 = a1x->word_0x96_150;  v9 = 0;
    if (v1) {                                          // leader present (NOT for model 76)
        v2x = Entities_EA3E4[v1];
        predictedAxis = v2x->position_0x4C_76;
        predictedAxis.z += v2x->array_0x52_82.yaw;
        CopyEntityPosition_57CF0(a1x, &predictedAxis); // snap to leader
        if (v2x->life_0x8 < 0 || v2x->struct_byte_0xc_12_15.byte[1] & 4) v9 = 1;  // leader dead -> collapse
    }
    result = getTerrainAlt_10C40(&a1x->position_0x4C_76);
    v6 = a1x->word_0x2C_44 + (int16)result;            // floor = terrain + current radius
    if (a1x->position_0x4C_76.z < v6) a1x->position_0x4C_76.z = v6;   // keep the orb above ground
    if (isCaveLevel_D41B6) {                            // cave ceiling clamp
        result = sub_10C60(&a1x->position_0x4C_76);
        v8 = (int16)result - a1x->word_0x2C_44;
        if (a1x->position_0x4C_76.z > v8) a1x->position_0x4C_76.z = v8;
    }
    if (v9) a1x->byte_0x46_70 = 2;                      // leader dead -> collapse
}
```
For model 76 (leader index 0), this reduces to: **clamp the hub's z to terrain + radius** (and to cave ceiling − radius in caves). No leader → no early collapse.

### 2.5 `sub_33D40` (EF:24769) — teardown — VERBATIM
```c
void sub_33D40(type_entity_0x6E8E* a1x)//214d40
{
    for (i = a1x->word_0x34_52; (v2x=Entities_EA3E4[i]) > Entities_EA3E4[0]; i = v2x->word_0x34_52)
        DisableEntityDrawing04_57F10(v2x);   // dispose every satellite
    DisableEntityDrawing04_57F10(a1x);       // then the head
}
```
On final collapse (`word_0x2C_44 < 0`), the head first spawns **(10,0)** ground fire at its position (EF:24604), then disposes all 25 satellites + itself.

### 2.6 Lifecycle summary
1. **Tick 1 (phase 0→1):** init (leader branch skipped), byte_0x46_70=1, then immediately runs the phase-1 body.
2. **Ticks 1..80 (phase 1):** terrain-clamp, breathe radius (±18, bouncing), tumble the constellation, deal 5× `sub_10C80(,0,70)` from slot-0 spheres (sound 3 on hit), decrement life. At life<1 → byte_0x46_70=2.
3. **Phase 2 (collapse):** keep spinning (`sub_33B20`), shrink radius by |18|/tick; when radius<0 → spawn **(10,0)** ground fire + `sub_33D40` teardown of all 26 entities.
- **RNG per tick: 0** (all randomness is front-loaded in `sub_4F440`).
- **Despawn conditions:** life exhausted (→ collapse → radius<0 → dispose). No water/void check on the head. (A leader-death path exists but is inert for model 76.)

---

## 3. Level-load / THING-init path — `sub_4A310` case 0xA, model 0x4C

`sub_4A310` (EF:32999) `case 0xA` (EF:33033), `v4 = model = 0x4C`:
- `v4 < 0x22u`? no (0x4C ≥ 0x22).
- `v4 <= 0x22u`? no.
- `v4 < 0x43u`? no.
- `v4 > 0x43u`? **yes** (0x4C > 0x43) → EF:33118.
  - `v4 >= 0x53u`? no (0x4C < 0x53).
  - `if (v4 != 0x47) { sub_58DA0(entity, v3x); return; }` → **taken** (0x4C ≠ 0x47), EF:33141-33145.

**⇒ For a (10,76) THING, `sub_4A310` creates the orb via the ctor and then does ONLY `sub_58DA0` stage-binding and returns. It consumes NO `par1/par2/par3/stageTag/word_10` fields.** The orb is entirely self-parameterized (all constants hard-coded in the ctor). Contrast the teleporter (0x22, reads par1/par2) and the marker band (0x36, reads stageTag) — (10,76) reads nothing.

`PrepareEvents_49540` (EV:287) case 0xA: subtype 0x4C is **not** in the special-case list `{0x1C,0x1D,0x1F,0x32,0x50,0x2D}` (EV:325-352) → falls to the default creation path (EV:353-400), and is **not** in the par-consuming default sub-switch `{0x09,0x0B,0x0F,0x52,0x53,0x54,0x55,0x58}` (EV:362-399) → plain creation, no par consumption. Consistent with `sub_4A310`.

### 3.1 GenerateEvents passes — (10,76) is NOT load-generated
`GenerateEvents_49290` (EV:152) runs its passes over `DisId == -1` THINGs. Subtype **0x4C does not appear in ANY pass list** (passes cover 0x52; then {0x09,0x53,0x54,0x55,0x0B,0x0F,0x1E,0x1D,0x20,0x1F,0x33,0x32,0x58}; then {0x51,0x50}; then class-0x0E/2; then {0x1B,0x1C}; then 0x2D×2 — EV:162-277). **⇒ A load-time (DisId==-1) (10,76) THING is never spawned during level generation.** It is reachable only at RUNTIME via a fired disposition (`sub_4A1E0` EF:32950 → `sub_4A310`), i.e. it is a triggered/scripted hazard, not a settle-time entity.

### 3.2 ApplyEvents settle disable band — moot for (10,76)
The settle loop keys on `v4 = model` (EV:453). Model 0x4C: `v4 >= 0x32` and `v4 > 0x33 && v4 < 0x50` → **DISABLED** (`DisableEntityDrawing04` at EV:508-514). Same for the satellites (0x4D). So IF a (10,76) ever reached a settle pass it would be destroyed — but per §3.1 it never enters a load pass, so this band never actually fires on it. (Recorded for completeness / to warn against authoring it as a DisId==-1 THING.)

---

## 4. Runtime spawners / callers

- **Creator dispatch:** only `IfSubtypeCallCreatingManaSphere_4A190(...,10,76)` → EV:4634 `case 0x2302a0: return AddFireSpheres_4F2A0(...)`. That is the sole reference to the ctor.
- **grep `4A190(...,10,76)` / `(...,10,0x4C)`:** **no runtime caller** anywhere in EF/EV. No code emits a fire-sphere orb procedurally.
- **grep `AddFireSpheres_4F2A0`:** only the definition (EF:35936) + the dispatch case (EV:4635).
- **grep `model_0x40_64 == 0x4C` / `== 76`:** **none.** No code reads model 76 by hand.
- **grep model 77 (0x4D):** **none** — the satellite is never referenced by number; it exists only as chain nodes the head walks. Its strA1 creator row is NULL (never spawnable as a THING) and its strA0 action row is NULL (never ticks).

**⇒ (10,76) is spawned EXCLUSIVELY as a map THING / disposition of subtype 76 via `sub_4A310`.** It is an authored, position-placed, self-contained orbiting fire hazard. (This is why word_0x96_150 leader-attach is always 0 — no in-engine caller wires it to a host entity.)

---

## 5. Damage / targetability

- **Does it read its own mailbox `str_0x5E_94`?** **No.** Neither the head (`sub_339B0` chain) nor the satellites read `str_0x5E_94` — the orb is a pure **dealer**, not a receiver. It cannot be hurt through its mailbox.
- **Damage delivery:** the 5 slot-0 satellites each call `sub_10C80(sat, 0, 70)` per tick (type-0 mask, amount 70) at the satellite's swept cell; head plays sound 3 on any hit (§2.3).
- **byte[0] bit3 (0x08, targetable):**
  - **Head:** ctor does `byte[0] &= 0xF6` (clears bits 1,3, note 0x08 is bit3) then `|= 1` → **bit3 CLEARED = NOT targetable.**
  - **Satellites:** in `sub_4F440`, `byte[0] &= 0xFE` (clear bit0) for all; then **slot!=0 → clear bit3 (`&0xF7`) + set byte[2] bit7** (20 visual spheres, not targetable); **slot==0 → SET bit3 (`|=8`)** (5 damage spheres). So only the 5 slot-0 spheres carry the "damage-active/targetable" bit3 — matching them being the `sub_10C80` emitters. Their bit0 (collidable) is cleared, so they don't self-collide; damage flows purely through `sub_10C80`'s cell scan.

---

## 6. Consolidated constants

| item | value | cite |
|---|---|---|
| head class/model/action | 10 / 76 (0x4C) / **0x53 (83)** | EF:35944-35946 |
| satellite class/model/action | 10 / 77 (0x4D) / **0x54 (84)** — action handler NULL | EF:35968-35969; strA0 row 0x54 NULL |
| satellite count | **25** (5 rings × 5 slots) | EF:35962 |
| spawn gate | free slots `sub_4A810_get_0x35plus() >= 26` | EF:35939 |
| head life / maxLife | **80 ticks** | EF:35947, 35961 |
| damage (subSpellIndex) | **70**, via `sub_10C80(sat,0,70)` type-0 mask, 5 slot-0 spheres/tick | EF:35948, 24712 |
| ring radius bounds | maxSpeed 192, minSpeed 480; current `word_0x2C_44` breathes ±18 between them (ping-pong) | EF:35950-35951; 24633-24650 |
| radius breathe step | `fontTypeIndex_0x3D_61` = **18** (sign-flipped at each bound) | EF:36006, 24643/24650 |
| head spin/tick | yaw +22, pitch +16 | EF:24668-24672 |
| satellite spin rate | `(rand&0x3F)+84` = **84..147** per sphere (yaw-spin=roll for ring0, pitch-spin=fov for rings1-4) | EF:36031 |
| satellite sprite | **340** (fire sphere) + `AddEvent2_847D0(sat,128,1,0)` attached child | EF:36078-36079 |
| head extents | `SetEntityShiftRot_49EA0(head, 640, 640)` | EF:35981 |
| head sprite | **none** (invisible hub) | — (no SetEntityIndex on head) |
| on collapse | spawn **(10,0)** ground fire, then dispose all 26 | EF:24604-24605, 24769 |
| sound | **3** (on any damage hit) | EF:24713 |
| RNG draws (spawn) | **25** (1 per satellite in `sub_4F440`); head 0; per-tick 0 | EF:36030 |
| targetable (byte[0] bit3) | head: cleared; sats slot!=0: cleared; sats slot==0 (×5): **set** | EF:35959; 36020-36028 |
| level-load par consumption | **none** (sub_4A310 → sub_58DA0 only) | EF:33141-33145 |
| load generation | **not in any GenerateEvents pass** — runtime/disposition only | EV:162-277 |

### Rust-port note
Port (10,76) as a **self-contained authored orbiting fire hazard**, not a spell effect:
- Spawn 1 hub + 25 satellites in a `(ring 0..4, slot 0..4)` lattice; assign each satellite its base yaw/pitch from `sub_4F440`'s switch and a random spin rate `(rng&0x3F)+84` (draw the 25 RNGs at spawn to match state-hash goldens).
- Per tick: terrain-clamp hub z to `terrain+radius`; breathe radius by ±18 bouncing across the 192/480 bounds; advance hub yaw+22/pitch+16 and each satellite yaw+=its_yawspin / pitch+=its_pitchspin; re-place each satellite at `hub + spherical(satAngle+hubAngle, radius)`; for the 5 slot-0 satellites deal type-0 AoE 70 at their cell (sound 3 on hit); decrement life (start 80).
- On life exhaustion, shrink radius by 18/tick while still spinning; when radius<0 spawn a `(10,0)` ground-fire splat and dispose the whole constellation.
- Sprite 340 for satellites; hub invisible. Not targetable except the 5 damage spheres (which are non-collidable but carry the damage bit). Reads NO par fields and NO mailbox — purely position-placed.
- It is spawned only via a fired disposition of subtype 76 (never at level-generation, never procedurally), so wire it into the disposition/trigger path, not the settle path.

---

## 7. OPEN items
1. **`word_0x96_150` leader-attach path is dead for model 76** in this decompile (no caller sets it; the phase-0 leader-sizing and `sub_33C70` leader-follow branches never fire). Confirm no other MC2 model reuses `AddFireSpheres`/action 0x53 with a leader set (e.g. a boss aura) — grep found none, but the branch's existence hints a designed-but-unused "orb wrapped around a host" mode. Low risk for the port (implement leader==0 only).
2. **`AddEvent2_847D0(sat,128,1,0)` attached child (type 1)** — the secondary particle/visual attached to each of the 25 spheres was not transcribed (it is the same attach primitive used by (10,23)/other effects with a different type code). Pull `AddEvent2_847D0` when porting the visual layer; it does not affect damage/movement.
3. **min/max radius label inversion** — authored `maxSpeed_0x86_134=192` < `minSpeed_0x84_132=480`, and `sub_33AD0`'s clamp uses min as the "high" comparand and max as the "reset" value. The net motion is a bounce across [192,480], but confirm the exact per-tick radius sequence against a recorded orb (the sign-flip logic is subtle) before pinning a state-hash golden.
4. **Sprite 340 / `AddEvent2` type-1 particle** transparency + animation frames are render/data-table side (`particlesParameters_D951C[340]`), extract separately as with the smoke bands (m13 OPEN-4).
