# CLASS-10 Models 18 (0x12) & 91 (0x5B) — Raise-Land Dome SUMMIT CHILDREN

The two entities the `(10,9)` raise-land / apocalypse dome births at its summit on the `life==3` grow beat.
**Model 18 (0x12) = the NORMAL summit eruption** (a spinning ground-vortex that drags creatures + throws fire
columns + lightning bolts). **Model 91 (0x5B) = the APOCALYPSE summit variant** (rains collectible mana spheres
and floods every wizard's spell-XP). All citations point at
`/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`).
Trace date 2026-07-11.

Read `docs/traces/mc2-class10-m9-dome-geometry.md` (parent dome machine) and
`docs/traces/mc2-class10-m9-dome-open-closure.md` (`sub_6D8B0` = spell-XP, not earthquake; `EuclideanDistXYZ` is 2-D)
first — this doc only expands the CHILDREN, and reuses those banked closures.

**Headline corrections to the task's priors:**

1. **The apocalypse ctor is `sub_4EF30` (0x22FF30, EF:35797), NOT `sub_32A70`.** The task hint conflated
   two different symbols. `sub_32A70` (0x213A70, EF:23906) is the *action/tick handler* of model **18** — it is
   model 18's eruption machine, not any ctor. The normal ctor is `sub_4EED0` (0x22FED0, EF:35777) as the prior
   guessed; the apocalypse ctor is `sub_4EF30`.
2. **The spawn site does NOT call any ctor by name.** It calls `IfSubtypeCallCreatingManaSphere_4A190(&pos,10,18|91)`
   which dispatches through the **class-10 ctor table** `str_x_DWORD_D4C52ar_0x1D26[93]` (EV:34-127), indexed by
   model. Row 0x12 → ctor 0x22FED0 (`sub_4EED0`); row 0x5B → ctor 0x22FF30 (`sub_4EF30`). (EV:53, EV:126.)

---

## 1. The child-spawn call site (inside the dome grow phase, EF:23400-23430)

Reached once, on the tick where `life_0x8 == 3` (one tick before the dome enters finalize). Verbatim
(`a1x` = the dome entity; `v45x` = its center tile `(pos.x+128)>>8, (pos.y+128)>>8`):
```c
if (a1x->life_0x8 == 3)
{
    // ... (a 2x2 summit-cap stamp identical to finalize: height v43-16, shading 63 Day / 1 else) ...
    predictedAxis_EB398ar = a1x->position_0x4C_76;                       // COPY dome position (x,y,z)
    predictedAxis_EB398ar.z = getTerrainAlt_10C40(&predictedAxis_EB398ar); // then SNAP z to terrain altitude
    v1x = D41A0_0.byte_0x36E03
        ? IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 91)   // apocalypse latch set -> model 91
        : IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 18);  // normal            -> model 18
    if (v1x)
        v1x->id_0x1A_26 = a1x->id_0x1A_26;                              // child INHERITS the dome's owner id
}
```
- **Position:** dome's `(x,y)` at the summit tile, **z snapped to terrain height** (`getTerrainAlt_10C40`) — i.e.
  the child sits on the newly-raised ground, NOT at the dome's z-center. (EF:23424-23425.)
- **Owner:** `id_0x1A_26` copied from the dome. That id is the wizard who cast/owns the dome (for the endgame it is
  the endgame-trigger's `id`, EF:12868). Everything the child does (damage credit, spell-XP) is billed to that id.
- **Only field copied in:** `id_0x1A_26`. Everything else comes from the child ctor. No position/rot/life copy.
- **Selector:** `D41A0_0.byte_0x36E03` — the apocalypse latch. Set to 1 by the endgame trigger AFTER the dome is
  created (parent doc §4, EF:12871). The dome's grow ticks all see it set, so at `life==3` it spawns model 91.
  A par1-authored / player-cast `(10,9)` (latch clear) spawns model 18.
- **Spawn helper:** `IfSubtypeCallCreatingManaSphere_4A190(axis*, type, subtype)` (EV:5186) is a thin table
  dispatcher: `if (str_D4C48ar[type].dword_14[subtype].dword_10 && ...word_4==subtype) return pre_sub_4A190_axis_3d(...address_6, pos);` — it returns NULL if the (type,subtype) slot is unregistered. Not a "mana sphere"
  function despite its name; it is the generic entity spawner.

**Cadence:** exactly ONE child per dome, at `life==3`. Not per-tick. The dome then finalizes and despawns two
ticks later (life 3→2→1→0→phase 2). The child then runs its own long lifetime independently.

---

## 2. The two ctors — VERBATIM

### 2.1 Model 18 ctor `sub_4EED0` (0x22FED0, EF:35777)
```c
type_entity_0x6E8E* sub_4EED0(axis_3d* position)//22fed0
{
    type_entity_0x6E8E* v1x = NewEvent_4A050();
    if (v1x)
    {
        v1x->actionIndex_0x45_69 = 18;          // -> action table row 0x12 -> 0x213A70 = sub_32A70 (§3.1)
        v1x->class_0x3F_63       = 10;
        v1x->model_0x40_64       = 18;
        v1x->subSpellIndex_0x2A_42 = 200;       // damage subspell for its fire/lightning children
        v1x->dword_0x10_16       = 0;           // tick counter (drives the whole state machine)
        v1x->maxLife_0x4         = 10000;       // effectively "immortal"; sub_32A70 self-terminates instead
        v1x->struct_byte_0xc_12_15.byte[0] &= 0xF7;   // clear draw-list bit 3
        v1x->position_0x4C_76 = *position;      // NO terrain snap here (position already snapped at spawn site)
        CopyMaxLifeToLife_49A20(v1x);           // life_0x8 = maxLife_0x4 = 10000
    }
    return v1x;
}
```
No RNG draws in the ctor. No sprite/extents set here (model 18 draws nothing itself — it is an invisible
controller; its visible effects are the children it spawns). `subSpellIndex=200`, `maxLife=life=10000`.

### 2.2 Model 91 ctor `sub_4EF30` (0x22FF30, EF:35797)
```c
type_entity_0x6E8E* sub_4EF30(axis_3d* a1x)//22ff30
{
    type_entity_0x6E8E* v1x = NewEvent_4A050();
    if (v1x)
    {
        v1x->actionIndex_0x45_69 = 98;          // 0x62 -> action table row 0x62 -> 0x213CF0 = sub_32CF0 (§3.2)
        v1x->class_0x3F_63       = 10;
        v1x->model_0x40_64       = 91;
        v1x->subSpellIndex_0x2A_42 = 200;
        v1x->dword_0x10_16       = 0;
        v1x->maxLife_0x4         = 10000;       // "immortal"; sub_32CF0 never despawns itself (see OPEN-1)
        v1x->struct_byte_0xc_12_15.byte[0] &= 0xF6u;   // clear bits 1 and 3 ...
        v1x->struct_byte_0xc_12_15.byte[0] |= 1;       // ... then SET bit 0
        v1x->position_0x4C_76 = *a1x;
        CopyMaxLifeToLife_49A20(v1x);           // life_0x8 = maxLife_0x4 = 10000
    }
    return v1x;
}
```
No RNG draws in the ctor. No sprite/extents. Byte-0 flag decode: `&0xF6 |1` = clear bits 1,2? — bit-math is
`0xF6 = 1111_0110`, so it clears bits 0 and 3 (the `&0xF7` in model 18 clears only bit 3; here bit 0 is also
cleared) then bit 0 is force-set. Net byte[0]: bit0=1, bit3=0, others preserved. (Contrast model 18 sets no bit0.)
Same `subSpellIndex=200`, `maxLife=life=10000` as model 18.

---

## 3. The two tick handlers — VERBATIM

Action-table resolution (`x_DWORD_D4C52ar_strA0[100]`, EF:1601, indexed by `actionIndex_0x45_69`):
`0x12 → 0x213A70` (`sub_32A70`, EF:1620) and `0x62 → 0x213CF0` (`sub_32CF0`, EF:1700). Confirmed against the
Events.cpp dispatch switch: EV:2381-2382 (`case 0x213a70: sub_32A70(...)`) and
EV:2385-2386 (`case 0x213cf0: sub_32CF0(...)`).

### 3.1 Model 18 action `sub_32A70` (0x213A70, EF:23906) — the ground-vortex eruption
```c
void sub_32A70(type_entity_0x6E8E* a1x)//213a70
{
    if (a1x->dword_0x10_16 > 2500)                              // after ~2500 ticks: random self-teardown
    {
        a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;   // RNG draw 1
        if (!(a1x->rand_0x14_20 % 0x64u) && !D41A0_0.word_0x31)  // ~1/100 chance, when no active-vortex latch
        {
            v1 = a1x->position_0x4C_76.z;
            v2 = getTerrainAlt_10C40(&a1x->position_0x4C_76);
            a1x->position_0x4C_76.z = v2;
            if (v1 != v2) { DisableEntityDrawing04_57F10(a1x); return; }   // terrain moved under it -> despawn
            a1x->dword_0x10_16 = 0;                            // else reset the tick counter (restart)
        }
    }
    if (a1x->dword_0x10_16 < 128
        && a1x->dword_0x10_16 & 0xF
        && (a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439, !(a1x->rand_0x14_20 % 5u))   // RNG draw 2, ~1/5
        || !a1x->dword_0x10_16)                                 // OR always on the very first tick
    {
        v3 = a1x->position_0x4C_76.z;
        v4 = getTerrainAlt_10C40(&a1x->position_0x4C_76);
        a1x->position_0x4C_76.z = v4;
        if (v3 != v4) { DisableEntityDrawing04_57F10(a1x); D41A0_0.word_0x31 = 0; return; }  // ground moved -> despawn

        if (!a1x->dword_0x10_16)                                // FIRST tick only: seize the vortex + smoke-column latches
        {
            v5x = Entities_EA3E4[D41A0_0.word_0x31];
            if (v5x > Entities_EA3E4[0]) v5x->dword_0x10_16 = 250;   // fast-expire the PREVIOUS vortex's controller
            D41A0_0.word_0x31 = a1x - D41A0_0.struct_0x6E8E;         // register THIS entity as the active vortex
            v6x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 19);  // spawn (10,19) fire-spray column
            if (v6x)
            {
                v6x->id_0x1A_26 = a1x->id_0x1A_26;                   // inherit owner
                if (Entities_EA3E4[D41A0_0.word_0x33] > Entities_EA3E4[0])
                    DisableEntityDrawing04_57F10(Entities_EA3E4[D41A0_0.word_0x33]);   // kill the previous fire column
                D41A0_0.word_0x33 = v6x - D41A0_0.struct_0x6E8E;     // register new fire column in singleton latch
            }
        }

        v8x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 16);  // spawn (10,16) tornado-drag entity
        if (v8x)
        {
            v8x->id_0x1A_26 = a1x->id_0x1A_26;
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;    // RNG draw 3
            v8x->rand_0x14_20 = a1x->rand_0x14_20;                  // seed the child's RNG from the parent
        }

        v10 = a1x->dword_0x10_16;
        a1x->yaw_0x1C_28 += 1280;                                   // spin the emitter yaw each pulse
        if (!v10)                                                   // FIRST tick only: fire a (9,0) lightning/spark bolt
        {
            v12x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 9, 0);  // class-9 model-0 projectile
            if (v12x)
            {
                v12x->id_0x1A_26 = a1x->id_0x1A_26;
                v11 = a1x->yaw_0x1C_28;
                v12x->pitch_0x1E_30 = -386;                         // steep upward pitch
                v12x->byte_0x43_67 = 10;                            // xtype  (class-10 origin)
                v12x->byte_0x44_68 = 17;                            // xsubtype 17
                HIBYTE(v11) &= 7u;                                  // wrap yaw to 11-bit
                v12x->life_0x8 = 1;
                v12x->yaw_0x1C_28 = v11;
                v12x->axis_0x9A_154x = a1x->position_0x4C_76;
                MoveEntity_57FA0(&v12x->axis_0x9A_154x, v12x->yaw_0x1C_28, 0, 1536);   // aim point 1536 units ahead
                v12x->axis_0x9A_154x.z = getTerrainAlt_10C40(&v12x->axis_0x9A_154x);
            }
        }
        if (a1x->dword_0x10_16 >= 127)                              // >=127 ticks into a pulse window -> despawn
        {
            DisableEntityDrawing04_57F10(a1x);
            D41A0_0.word_0x31 = 0;                                  // release the vortex latch
        }
    }
    a1x->dword_0x10_16++;                                           // advance tick counter every call
}
```
**What model 18 spawns per pulse** (a "pulse" fires when `dword_0x10_16 < 128 && (dword_0x10_16 & 0xF) && rand%5==0`,
plus unconditionally on tick 0):
- **(10,16)** — a tornado-drag entity (§4.1) EVERY pulse. Seeded from the parent RNG. This is the thing that
  physically sucks/whirls nearby creatures and deals the melee damage.
- **(10,19)** — one persistent fire-spray column (§4.2, = the already-banked ground-fire-spray, `sub_32F40`),
  ONCE on tick 0. Singleton via `word_0x33` — a new vortex kills the old column.
- **(9,0)** — one class-9 model-0 projectile (banked in `mc2-class9-spell-projectiles.md`) ONCE on tick 0,
  pitched steeply up (`pitch=-386`), `xtype/xsubtype = 10/17`, `life=1`, aimed 1536 units along the emitter yaw.
- **Vortex singleton:** `word_0x31` holds the active model-18; `word_0x33` holds the active (10,19) column.
  A newborn model-18 fast-expires the previous one (`dword_0x10_16 = 250`) so only one vortex runs at a time.
- **Damage:** model 18 itself deals NONE directly (no `sub_11900`/`sub_11400`/mailbox call in this handler).
  All damage is delegated to its (10,16) and (10,19) children. Its own `subSpellIndex=200` is passed DOWN? — no,
  the children carry their own subspells; model 18 only copies `id`. (Verified: grep of `sub_32A70` for
  `sub_11900|sub_11400|sub_116A0|str_0x5E` = zero hits.)
- **Sound:** NONE emitted by `sub_32A70` itself (no `PrepareEventSound_6E450`). Sounds come from the children.
- **Lifetime / despawn:** self-terminates on any of: (a) ground height changed under it (2 sites), (b)
  `dword_0x10_16 >= 127` within a pulse window, (c) the >2500-tick random-teardown branch. `maxLife=10000` is a
  backstop that rarely bites first. On despawn it clears `word_0x31`.
- **RNG order per full pulse tick:** up to 3 draws — (1) the >2500 teardown roll [only when applicable],
  (2) the `%5` pulse gate, (3) the (10,16) child-seed draw. `r = 9377*r + 9439` on `a1x->rand_0x14_20`.

### 3.2 Model 91 action `sub_32CF0` (0x213CF0, EF:24007) — the apocalypse mana-rain
```c
void sub_32CF0(type_entity_0x6E8E* a1x)//213cf0
{
    for (i = 0; i < 3; i++)                                          // spawn THREE mana spheres this tick
    {
        v1x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 39);   // (10,39) collectible mana sphere
        if (v1x)
        {
            v4 = v1x->struct_byte_0xc_12_15.byte[1] | 0x20;
            v1x->maxLife_0x4 = 140;
            v1x->struct_byte_0xc_12_15.byte[1] = v4;                 // set draw-flag bit 5
            v1x->life_0x8 = v1x->maxLife_0x4;                        // life = 140
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;     // RNG 1: launch speed
            v5 = a1x->rand_0x14_20 % 0x300u;
            v1x->actSpeed_0x82_130 = v5;
            if (v5 < 64)  v1x->actSpeed_0x82_130 = 64;               // clamp [64, 768]
            if (v1x->actSpeed_0x82_130 > 768) v1x->actSpeed_0x82_130 = 768;
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;     // RNG 2: arc height
            v6 = a1x->rand_0x14_20 & 0x7F;
            v1x->playerEntityIndex_0x94_148 = 0;
            v1x->word_0x2C_44 = v6 + 128;                            // ballistic apex 128..255
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;     // RNG 3: sprite variant selector
            v7 = a1x->rand_0x14_20 % 9u - 1;                         // -1..7 (fed to color-index picker)
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;     // RNG 4: mana amount
            v8 = 0;
            v1x->mana_0x90_144 = a1x->rand_0x14_20 % 0xA00u + 1;     // mana value 1..2560
            while (v8 < 7 && v1x->mana_0x90_144 > manaSphereSizeTable_DB538[v8]) v8++;   // size bucket 0..7
            v9 = GetManaSphereIndexFromId_36A50(v7);                 // base sprite index by color (§4.4)
            SetEntityIndexAndRot_49CD0(v1x, v8 + v9);                // sprite = base + size bucket
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;     // RNG 5: launch yaw
            v10 = a1x->rand_0x14_20 & 0x7FF;
            v1x->pitch_0x1E_30 = 0;
            v1x->yaw_0x1C_28 = v10;
            v1x->roll_0x20_32 = v1x->yaw_0x1C_28;
            v1x->axis_0x9A_154x = a1x->position_0x4C_76;
            MoveEntity_57FA0(&v1x->axis_0x9A_154x, v1x->yaw_0x1C_28, 0, v1x->actSpeed_0x82_130);  // launch vector
            v1x->axis_0x9A_154x.x -= v1x->position_0x4C_76.x;        // store as a DELTA (velocity), not absolute
            v1x->axis_0x9A_154x.y -= v1x->position_0x4C_76.y;
            v1x->position_0x4C_76.z = getTerrainAlt_10C40(&v1x->position_0x4C_76) + 96;   // start 96 above ground
        }
    }
    if (!(a1x->byte_0x3E_62 & 1))                                    // every OTHER tick: flood spell-XP
    {
        v13 = 0;
        while (v13 < 26)                                             // for all 26 spell rows
        {
            v14 = (SPELLS_BEGIN_BUFFER_str[v13].subspell[2].xpos1_E
                   - (my_sign32(...xpos1_E) << 9) + my_sign32(...xpos1_E)) >> 9;   // = xpos1_E / 512 (round-to-0)
            v15 = v13++;
            sub_6D8B0(D41A0_0.array_0x2BDE[D41A0_0.LevelIndex_0xc].playerIndex_...11240, v15, v14);  // add XP row v15
        }
    }
}
```
**What model 91 does per tick:**
- **Spawns THREE `(10,39)` collectible mana spheres** (§4.3, ctor `CreateManaSphere512_50080` region — action 0x29,
  the ordinary pick-up mana orb) with fully randomized velocity (`actSpeed` 64..768), ballistic apex (128..255),
  color/size sprite, mana value **1..2560**, and random launch yaw. They arc out and land as collectible mana. This
  is the "apocalypse rains mana/gold" cinematic.
- **Every other tick (`!(byte_0x3E_62 & 1)`), floods spell experience:** loops all 26 spell rows and awards
  `sub_6D8B0(player, row, xp)` where `xp = SPELLS[row].subspell[2].xpos1_E / 512` (tier-2 xpos1 field, signed
  divide-toward-zero by 512). So the level's designated player is instantly maxed across every spell. (sub_6D8B0
  is the spell-XP accumulator, banked in `mc2-class10-m9-dome-open-closure.md §1`, gated on `!(setting_38545 & 4)`
  and the target being a wizard.)
- **Damage:** NONE. No `sub_11900`/area-damage call — the apocalypse variant is non-lethal by design (matches the
  parent doc's finding that the endgame dome deals no area damage).
- **Sound:** NONE emitted by `sub_32CF0` itself. (The mana spheres and their pickups carry their own audio.)
- **Lifetime / despawn:** `sub_32CF0` NEVER despawns itself — no `DisableEntityDrawing04` path. It runs until the
  entity is torn down externally (level end / KillAllCreatures). See OPEN-1. `maxLife/life = 10000`.
- **RNG order per spawned sphere:** 5 draws in order — speed, apex, color-selector, mana-amount, yaw. Three spheres
  ⇒ **15 draws per tick**, all `r = 9377*r + 9439` on `a1x->rand_0x14_20`.

---

## 4. Helper functions the two handlers call (that aren't already banked)

Already banked, do NOT re-transcribe: `MoveEntity_57FA0`, `getTerrainAlt_10C40`, `EuclideanDistX*`,
`sub_11900` (damage mailbox `str_0x5E_94`), `sub_6D8B0` (spell-XP), `IfSubtypeCallCreatingManaSphere_4A190`,
`sub_32F40` (= (10,19), the ground-fire-spray, in `mc2-class10-m6-m9-m11-m28-m31.md §3`), `(9,0)` projectile
(`mc2-class9-spell-projectiles.md`), `(10,39)` mana-sphere pickup machine (referenced across `mc2-class9-flyers.md`
/ `mc2-multipart-chains.md`).

### 4.1 `(10,16)` ~~tornado-drag~~ VOLCANO BOULDER — ctor `sub_4EDC0` (0x22FDC0, EF:35749) + action `sub_32600` (0x213600, EF:23729)

> **ERRATA (2026-07-17, player-reported "boulders act as cyclones"):**
> the original trace read `actionIndex = 16` as `0x214110 = sub_33110`
> (the whirlwind driver). That is a DECIMAL/HEX confusion: the class-10
> `strA0` action table (EF:1601-1701) maps row `0x0010` (= 16 decimal,
> this ctor's value) to `0x213600 = sub_32600` — a ballistic
> rolling/burning BOULDER (gravity, ±80 velocity clamp, bounce
> `vz=-(vz/4)`, water splash-out, lights a 30-tick (10,6) fire with
> subSpell×3, `sub_58030` slope roll + 250/256 friction; NO sound, NO
> player interaction, NO XP). `0x214110 = sub_33110` belongs to row
> `0x0016` (= 22 decimal), the (10,22) whirlwind head only. The
> `word_0x2C_44 = 256` below is the launch VERTICAL IMPULSE (vz), not a
> "radius/height param", and `MoveEntity_57FA0` onto the zeroed
> `axis_0x9A` stores the launch VELOCITY DELTA. The `sub_33110` body
> quoted further down documents the (10,22) whirlwind, NOT this model.
> Ported 2026-07-17 (`mc2_boulder16_tick`).

**Ctor (verbatim):**
```c
type_entity_0x6E8E* sub_4EDC0(axis_3d* position)//22fdc0
{
    type_entity_0x6E8E* v1x = NewEvent_4A050();
    if (v1x)
    {
        v1x->actionIndex_0x45_69 = 16;                    // -> 0x214110 = sub_33110
        v1x->class_0x3F_63 = 10;  v1x->model_0x40_64 = 16;
        v1x->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;   // clear draw bits 3 and 17
        v1x->subSpellIndex_0x2A_42 = 200;                 // DAMAGE subspell (relayed by sub_331A0/33340/33710)
        v1x->rand_0x14_20 = 9377 * v1x->rand_0x14_20 + 9439;    // RNG 1
        v1x->maxLife_0x4 = v1x->rand_0x14_20 % 0x64u + 100;     // life 100..199
        v1x->rand_0x14_20 = 9377 * v1x->rand_0x14_20 + 9439;    // RNG 2
        v1x->actSpeed_0x82_130 = (v1x->rand_0x14_20 % 0x32u) + 52;   // wander speed 52..101
        v1x->rand_0x14_20 = 9377 * v1x->rand_0x14_20 + 9439;    // RNG 3
        v1x->word_0x2C_44 = 256;                          // radius/height param
        v1x->yaw_0x1C_28 = v1x->rand_0x14_20 & 0x7FF;     // random heading
        v1x->struct_byte_0xc_12_15.byte[2] |= 2;
        AddEventToMap_57D70(v1x, position);
        v1x->position_0x4C_76.z = getTerrainAlt_10C40(position) + 64;   // hover 64 above ground
        MoveEntity_57FA0(&v1x->axis_0x9A_154x, v1x->yaw_0x1C_28, 0, v1x->actSpeed_0x82_130);
        CopyMaxLifeToLife_49A20(v1x);
        SetEntityIndexAndRot_49CD0(v1x, 210);             // sprite 210
    }
    return v1x;
}
```
**Action `sub_33110` (verbatim):**
```c
void sub_33110(type_entity_0x6E8E* a1x)//214110
{
    v1 = a1x->life_0x8 - 1;  a1x->life_0x8 = v1;
    if (v1 < 0) {
        EndLoop_6EAB0(a1x - D41A0_0.struct_0x6E8E, -1, 49);   // stop looping sound 49
        sub_338D0(a1x);                                       // death: clear grabbed-creature flags + despawn chain
    } else {
        sub_331A0(a1x);                                       // move self + drag the grabbed-creature tail chain
        sub_33340(a1x);                                       // grab/whirl creatures on adjacent tiles + damage
        sub_33710(a1x);                                       // damage overlapping trees/entities every 8th tick
        PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 49);   // looping tornado sound 49
    }
}
```
- **`sub_331A0` (EF:24177):** advances the tornado along a slowly-precessing path (roll += 11*wobble, moved 32
  units; yaw += 341, sampled 120 units for the drag anchor), snaps z to terrain, then walks the **grabbed-creature
  linked list** (`Entities_EA3E4[word_0x34_52]` chain) and re-positions each captive around the vortex using
  `sub_581E0_maybe_tan2` bearing + `EuclideanDistXYZ_58490` distance, holding them at `72 - 4*(12-word_0x2C_44)`
  radius. No damage in `sub_331A0`; pure kinematics. RNG: one draw when `!(byte_0x3E_62 & 0xF)` (flips a wobble
  sign). (EF:24187-24222.)
- **`sub_33340` (EF:24229):** the GRAB + DAMAGE core. Over a 3×3-ish tile splat template (`AddE7EE0x_10080(0,12)`)
  around the vortex, for each entity on those tiles that passes `sub_33810` (target filter: creatures classes
  4..10, birds model 7-8, not same-id, not model-2 buildings — EF:24451), it either whirls the captive
  (state bit `byte[3]&0x10`) or reels it inward. On each captive it just grabbed this tick (`v39`) it calls
  **`sub_11900(a1x, ix, 0, subSpellIndex_0x2A_42=200)`** — channel-0 damage of subspell 200 into the victim's
  mailbox `str_0x5E_94` — and accumulates a hit count. At loop end, if any hits: `sub_6D8B0(id, 0x15u, hits)` →
  spell-row **0x15 (21)** XP. So the vortex's damage subspell is **200**, XP credited to spell 21. (EF:24272-24408.)
- **`sub_33710` (EF:24416):** every 8th tick (`!(byte_0x3E_62 & 7)`) sweeps two global entity lists: village
  BUILDINGS (`dword_38527` = the class-10 MODEL-45 list, builder EF:40043-51 — the earlier "creatures" reading
  was wrong, corrected 2026-08-11) get `sub_11900(a1x, ix, 0, 200)`; class/model-2 objects (`dword_38519`, model==2) get a knockback
  stamp (`word_0x30_48=30`, damage added into `str_0x5E_94.dword_0x5E_94 += 200`, owner id stamped, +2 hits).
  Hits → `sub_6D8B0(id, 0x15, v1)`. (EF:24424-24445.)
- **`sub_338D0` (EF:24518, death):** clears the "being-whirled" flags (`struct_byte...dword &= 0xEFFFF7FF`) on every
  entity in the splat tiles, then despawns the whole tornado + its captive chain via
  `DisableEntityDrawing04_57F10` walking `word_0x34_52`. (EF:24531-24557.)

**Net (10,16):** a self-wandering tornado, life 100..199, sprite 210, hovering 64 above ground, that grabs and
whirls creatures (and slams buildings), dealing **subspell-200 channel-0 damage** each tick via `sub_11900`, crediting
**spell-21 XP**, playing looping **sound 49**.

### 4.2 `(10,19)` fire-spray column — ctor `sub_4EF90` (0x22FF90, EF:35824), action `sub_32F40` (0x213F40)
Already banked as the (10,11)→model-19 ground-fire-spray in `mc2-class10-m6-m9-m11-m28-m31.md §3`. Ctor here
(verbatim, for completeness — it's the direct-model-19 entry the vortex uses, sprite 228, life 240):
```c
type_entity_0x6E8E* sub_4EF90(axis_3d* a1x)//22ff90
{
    v1x = NewEvent_4A050();
    if (v1x) {
        v1x->actionIndex_0x45_69 = 19;  v1x->class_0x3F_63 = 10;  v1x->model_0x40_64 = 19;
        v1x->subSpellIndex_0x2A_42 = 200;  v1x->maxLife_0x4 = 240;
        v1x->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;  v1x->struct_byte_0xc_12_15.byte[2] |= 2u;
        AddEventToMap_57D70(v1x, a1x);  v1x->struct_byte_0xc_12_15.byte[0] |= 1u;
        CopyMaxLifeToLife_49A20(v1x);  SetEntityIndexAndRot_49CD0(v1x, 228);  SetEntityShiftRot_49EA0(v1x, 512, 512);
    }
    return v1x;
}
```
Its `sub_32F40` machine (banked) sprays **(10,14)** smoke rings on odd life-ticks and applies area damage
(subspell 200) — see the banked doc; NOT re-transcribed here.

### 4.3 `(10,39)` mana sphere — ctor `CreateManaSphere512_50080` (0x231080, EF:36595)
```c
type_entity_0x6E8E* CreateManaSphere512_50080(axis_3d* position) { return CreateManaSphere_500C0(position, 512); }
type_entity_0x6E8E* CreateManaSphere_500C0(axis_3d* position, __int16 mana)//2310c0
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->actionIndex_0x45_69 = 0x29;   // action 41 = the collectible-mana-sphere machine
        event->class_0x3F_63 = 0xA;  event->model_0x40_64 = 0x27;   // (10,39)
        event->xtype_0x41_65 = 10;  event->xsubtype_0x42_66 = 39;
        event->word_0x2C_44 = 128;  event->actSpeed_0x82_130 = 32;
        event->byte_0x38_56 = 3;  event->byte_0x39_57 = 128;  event->byte_0x3A_58 = 0;
        event->mana_0x90_144 = mana;
        AddEventToMap_57D70(event, position);  CopyMaxLifeToLife_49A20(event);
        SetManaSphereColorAndRot_36920(event);
    }
    return event;
}
```
`sub_32CF0` calls the spawn through the dispatch table (subtype 39 → this ctor) then OVERWRITES `maxLife=140`,
`life=140`, `actSpeed`, `word_0x2C_44`, `mana`, sprite, yaw etc. as in §3.2 — so the ctor's 512-mana/word_0x2C_44=128
seeds are placeholders. The (10,39) action (0x29) is the ordinary pick-up mana-orb machine (banked). It is a
collectible: creatures/players walking over it gain its `mana_0x90_144`.

### 4.4 `GetManaSphereIndexFromId_36A50` (0x217A50, EF:26794) — mana-sphere base sprite by color
```c
int GetManaSphereIndexFromId_36A50(char index) {
    int result = 0;  char index2 = index;
    if (index >= 0) index2 = TransformPlayerColorIndex_616D0(index);
    switch (index2 + 1) {
        case 0: result = 52;  break;  case 1: result = 105; break;  case 2: result = 113; break;
        case 3: result = 121; break;  case 4: result = 129; break;  case 5: result = 137; break;
        case 6: result = 145; break;  case 7: result = 153; break;  case 8: result = 161; break;
    }
    return result;
}
```
In `sub_32CF0` it is called with `v7 = rand%9 - 1` (range -1..7). `index >= 0` gates the palette remap: for the
apocalypse rain the argument is a raw -1..7 selector, so `index2 = v7` when `v7 < 0` (only -1 → case 0 → 52),
else remapped. Final sprite = `GetManaSphereIndexFromId(...) + sizeBucket(mana)`.
`manaSphereSizeTable_DB538[8] = {256,512,1024,2048,4096,9192,18384,36768}` (EF:2600) — the 7-way size threshold.

---

## 5. Level-authored vs. runtime-only (task item 4)

**Both models are reachable via level THINGs but there is no evidence any shipped level authors them; they are
effectively dome-born only.**

- **Registration:** class-10 rows 0x12 and 0x5B are BOTH populated in the ctor table `str_x_DWORD_D4C52ar_0x1D26`
  (EV:53 → 0x22FED0; EV:126 → 0x22FF30) and in the action table `x_DWORD_D4C52ar_strA0` (EF:1620 → 0x213A70;
  EF:1700 → 0x213CF0). So a `(class=10, subtype=18|91)` THING would NOT be rejected by the spawner
  (`IfSubtypeCallCreatingManaSphere_4A190` returns non-NULL for both) — unlike the sparse holes (e.g. rows with
  `0x00000000` address at EV:72/81-84/110/112).
- **`sub_4A310` case-0xA path (EF:32999-33197):** the generic class-10 THING post-init. For model 18 (`v4=0x12`):
  `<0x22`, `>0xBu`, `<0x11u`? no (`0x12>0x11`), so `v4>0x11 && v4!=0x16` → falls straight to `sub_58DA0(entity,v3x)`
  and returns — a plain finalize with NO special field init (no SPELLS-table lookup, no par1 consumption; model 18
  is not in `GetSpellIndex_6E020`'s case list, so it would default subSpell to row-0 anyway). For model 91
  (`v4=0x5B`): `>0x22`, `>0x43`, `>=0x53`, `<=0x53`? no (`0x5B>0x53`), `<=0x55`? no → `sub_58DA0` return. So both
  models take the generic no-op init path — confirming they were never DESIGNED to be par1-authored spell effects;
  they are pure runtime children. (EF:33068-33073 for m18; EF:33118-33146 for m91.)
- **Endgame path:** the only production spawner is the dome (§1), driven by the apocalypse latch. Model 18 also has
  the vortex singleton machinery (`word_0x31`/`word_0x33`) which only makes sense as a dome child.
- **CONCLUSION:** treat (10,18) and (10,91) as **exclusively dome-born**. A port need not expose them as authorable
  THING subtypes; wire them only as the two dome-summit children. (If a future level file is found placing subtype
  18/91, they'd spawn functionally — but with the generic init, which is exactly the ctor defaults §2.)

---

## 6. Consolidated constants table

| item | model 18 (0x12) NORMAL | model 91 (0x5B) APOCALYPSE | source |
|---|---|---|---|
| ctor | `sub_4EED0` (0x22FED0) | `sub_4EF30` (0x22FF30) | EF:35777 / EF:35797 |
| ctor sets action | 18 (0x12) | 98 (0x62) | EF:35783 / EF:35807 |
| action handler | `sub_32A70` (0x213A70) | `sub_32CF0` (0x213CF0) | EF:23906 / EF:24007 |
| ctor subSpellIndex | 200 | 200 | EF:35786 / EF:35810 |
| ctor maxLife=life | 10000 | 10000 | EF:35788,35791 / EF:35814,35818 |
| ctor dword_0x10_16 | 0 | 0 | EF:35787 / EF:35812 |
| ctor byte[0] flags | `&0xF7` (clear bit3) | `&0xF6 \|1` (clear 0,3; set 0) | EF:35789 / EF:35815-16 |
| ctor sprite / extents | none (invisible controller) | none (invisible controller) | — |
| ctor RNG draws | 0 | 0 | — |
| per-tick spawns | (10,16) every pulse; (10,19)+( 9,0) on tick 0 | 3× (10,39) mana spheres EVERY tick | EF:23967,23957,23979 / EF:24031-24073 |
| damage (self) | NONE (delegated to children) | NONE | grep-verified |
| child damage | (10,16): `sub_11900` ch0 subspell 200 → spell-21 XP; (10,19): area subspell 200 | none | EF:24400,24429 |
| spell-XP flood | no | every other tick: all 26 rows, `xp = SPELLS[row].sub[2].xpos1_E/512` | EF:24075-24089 |
| sounds (self) | none (children play them) | none | grep-verified |
| child sounds | (10,16) looping sound 49; (10,19) fire sfx | mana-sphere pickup sfx | EF:24171 |
| lifetime | self-terminates (ground-move / ≥127-tick / >2500 random) | NEVER self-despawns (external teardown only) | EF:23931,23997,23921 / (no despawn) |
| RNG per tick | ≤3 draws (teardown / %5 gate / (10,16) seed) | 15 draws (5×3 spheres) | §3 |
| singleton latches | `word_0x31` (vortex), `word_0x33` (fire column) | none | EF:23956,23964 |
| owner inheritance | `id_0x1A_26` from dome | `id_0x1A_26` from dome | EF:23429 |
| spawn position | dome (x,y), z=`getTerrainAlt` (on raised ground) | same | EF:23424-25 |
| spawn cadence | 1 child @ dome life==3 | 1 child @ dome life==3 | EF:23400 |
| child-of-child sprites | (10,16)=210, (10,19)=228, (9,0)=projectile | (10,39)=`GetManaSphereIdx+bucket` | EF:35771,35845 / EF:24060-61 |

`manaSphereSizeTable_DB538[8] = {256,512,1024,2048,4096,9192,18384,36768}` (EF:2600).

---

## 7. Port notes

- **Wire both as dome-summit children only** (§5). In `mgc_sim::mc2::morph`, at the `life==3` beat, spawn
  `(10,18)` normally / `(10,91)` when the apocalypse latch is set, at the summit tile with z=terrain-alt, id
  inherited. That call already exists in the parent-dome port — this doc supplies the two child machines.
- **Model 18 = an invisible EMITTER/controller.** It draws nothing. Port it as a short-lived spawner state that:
  (1) on entry seizes the vortex singleton + spawns one (10,19) fire column + one (9,0) bolt, (2) each pulse
  (`tick<128 && tick&0xF && rand%5==0`) spawns a (10,16) tornado, (3) self-despawns on ground-move / tick≥127 /
  the >2500 random teardown. The **(10,16) tornado** is the real gameplay object — a wandering creature-grabber
  dealing subspell-200 channel-0 damage via the damage mailbox and crediting spell-21 XP. Port (10,16) fully
  (ctor §4.1 + the 4 sub-handlers); it reuses banked `sub_11900`/`MoveEntity`/`EuclideanDist`.
- **Model 91 = a pure cinematic + progression grant.** No damage. Port it as: spawn 3 collectible mana spheres/tick
  with the exact 5-draw RNG sequence (speed/apex/color/mana/yaw), and every-other-tick award all-26-spell XP to the
  level's player. Match the `rand%0xA00 + 1` mana range and the `xpos1_E/512` XP grant exactly if XP goldens matter.
- **RNG determinism:** both handlers draw on `entity.rand_0x14_20` (per-entity), `r = 9377*r + 9439`. Model 91's
  15-draws-per-tick order is load-bearing for sphere layout goldens. Model 18 seeds each (10,16)'s RNG from its own
  stream (EF:23971-73), so tornado wander is deterministic given the dome's seed.
- **Reuse banked pieces:** (10,19) fire-spray (`sub_32F40`), (9,0) projectile, (10,39) pickup machine, `sub_6D8B0`
  spell-XP, `sub_11900` mailbox — all already traced; do not re-port from scratch.

---

## 8. OPEN items

1. **Model 91 (`sub_32CF0`) has NO self-despawn path.** It relies on external teardown (level end /
   `KillAllCreatures_1B5F0` — which the endgame `case 0xF` calls every frame, EF:12858). CONFIRM the endgame
   frame-loop actually reaps model 91, or it would spawn mana forever. The dome that births it despawns at
   `life==0` (two ticks after `life==3`), so model 91 outlives its parent — its lifetime is bounded only by the
   surrounding endgame sequence. Flagged, not guessed.
2. **`sub_331A0` captive-chain acquisition.** This doc traces how (10,16) *drags* already-grabbed creatures and how
   `sub_33340` *grabs* them (sets `byte[3]&0x10` + links via `word_0x34_52`), but the exact linked-list insertion
   (where `word_0x34_52` is first written to enroll a victim) is inside `sub_33340`'s inner branches and only
   summarized here. Transcribe fully before pinning (10,16) capture goldens.
3. **`(10,39)` action 0x29 ballistics.** `sub_32CF0` stores the launch vector as a position DELTA in
   `axis_0x9A_154x` and sets `word_0x2C_44` as an apex — the actual arc integration lives in the (10,39) action
   machine (referenced-but-not-verbatim-here; banked indirectly). Verify the apex/gravity math if sphere landing
   positions need to be byte-exact.
4. **`GetManaSphereIndexFromId_36A50` argument sign for the rain.** `sub_32CF0` passes `rand%9 - 1` (can be -1);
   `TransformPlayerColorIndex_616D0` only runs for `index >= 0`. The -1 case (→ `switch(0)` → sprite 52) is a real
   but rare path; confirm 52 is the intended "wild/grey" mana sprite vs. a decompiler artifact.
5. **`sub_33810` full target filter.** Transcribed partially (classes 4..10, bird models 7-8, skip same-id / model-2).
   The remaining `case` arms (EF:24483+) weren't all read; confirm which creature classes the tornado can/can't grab
   before pinning damage goldens.
