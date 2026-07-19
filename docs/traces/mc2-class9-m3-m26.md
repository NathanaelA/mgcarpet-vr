# CLASS-9 models 3 & 26 — Verbatim Port Trace (doomsday-pyramid summons)

Scope: the two class-9 creators the (5,10) doomsday pyramid summons that are currently
misfit-ledgered — **class-9 model 3** (subtype 0x03) and **class-9 model 26 / 0x1A** (subtype 0x1A).

Citations: EF = `engine/EventsFunctions.cpp`, EV = `engine/Events.cpp`, under
`/home/rain/projects/mgcarpet/reference/remc2/remc2/`.

**This doc EXTENDS the banked family traces — do not re-read them into a port:**
- Shared flight law `sub_65820`, homing `sub_65610`, turn-cap `sub_58350`, ricochet `sub_68740`,
  victim-probe `sub_10780`, impact spawner `_4A190`, sound ids, behavior-row struct →
  `docs/traces/mc2-class9-flyers.md` (§0.x) and `mc2-class9-spell-projectiles.md`.
- **Both creators AND both flight states are ALREADY transcribed verbatim** in those two docs.
  This doc adds ONLY: (a) the pyramid summon sites verbatim, (b) the consolidated per-model
  constants for a port, (c) the full spawn-site inventory, (d) the exact flight law each needs.

---

## 1. The pyramid summon sites — VERBATIM

The (5,10) doomsday pyramid's spawner mover is **`sub_21AB0`** (EF:13270), called from the
class-5/model-10 action tick (EF:12791 inside `sub_21030`, and EV:1427). One outer
`switch (a1x->byte_0x43_67)` selects the doomsday *phase*: case 1 → (9,0), case 2 → (9,9),
cases 3–6 → land creatures (`sub_4B240`/`sub_4C8F0`/`sub_4CE00`/`sub_4C6B0`), case 7 → tremor,
**case 8 → (9,26)**, **case 9 → (9,3)**.

### 1.1 Shared launch preamble (EF:13313–13325) — applies to cases 8 and 9
```c
v2x = Entities_EA3E4[D41A0_0.array_0x2BDE[D41A0_0.LevelIndex_0xc].playerIndex_0x00a_2BE4_11240];
v31x = v2x;                                        // v31x = the level's PLAYER AVATAR
if (v2x > Entities_EA3E4[0] && v2x->life_0x8 >= 0 && !(v2x->struct_byte_0xc_12_15.byte[1] & 4))
{
    v3 = a1x->word_0x24_36;                         // spawn-budget counter
    if (v3)
    {
        a1x->word_0x24_36 = v3 - 1;
        v28x = a1x->position_0x4C_76;               // launch pos = pyramid pos ...
        MoveEntity_57FA0(&v28x, a1x->yaw_0x1C_28, 0, 640);   // ... stepped 640 fwd along pyramid yaw
        v4 = a1x->position_0x4C_76.z;
        HIBYTE(v4) += 3;                            // z raised by 3<<8 = 768
        v28x.z = v4;
        switch (a1x->byte_0x43_67) { ... }
    }
}
```
So **launch position `v28x`** = pyramid position, offset **640 units** forward along the pyramid's
`yaw`, with **z raised by 0x300 (768)**. Case 8/9 use this base `v28x` directly (they do NOT take
the extra `MoveEntity(...,1792)` scatter that cases 3–6 apply). Guarded by a per-tick spawn budget
`word_0x24_36` and by the avatar being alive & visible.

### 1.2 Case 8 → (9,26) VERBATIM (EF:13457–13472)
```c
case 8:
    v8x = IfSubtypeCallCreatingManaSphere_4A190(&v28x, 9, 26);
    v33x = v8x;
    if (v8x)
    {
        v8x->byte_0x43_67 = 10;      // impact effect class 10
        v8x->byte_0x44_68 = 22;      // impact effect model 22  -> effect (10,22)
        v8x->subSpellIndex_0x2A_42 = 20;    // damage payload = 20
        v8x->byte_0x46_70 = 3;              // fuse/charge byte = 3
        v34 = 15;                    // sound id 15
    }
    break;
```

### 1.3 Case 9 → (9,3) VERBATIM (EF:13473–13488)
```c
case 9:
    v7x = IfSubtypeCallCreatingManaSphere_4A190(&v28x, 9, 3);
    v33x = v7x;
    if (v7x)
    {
        v7x->byte_0x43_67 = 10;      // impact effect class 10
        v7x->byte_0x44_68 = 17;      // impact effect model 17  -> effect (10,17)
        v7x->subSpellIndex_0x2A_42 = 6000;  // damage payload = 6000
        v7x->byte_0x46_70 = 10;             // fuse/charge byte = 10
        v34 = 15;                    // sound id 15
    }
    break;
```

### 1.4 Shared post-spawn arming (EF:13492–13509) — applies to `v33x` (cases 1,2,8,9)
```c
if (v33x)
{
    v33x->id_0x1A_26 = a1x->id_0x1A_26;                    // owner = pyramid's id (owner-immunity)
    v33x->yaw_0x1C_28  = Maths::sub_581E0_maybe_tan2(&a1x->position_0x4C_76, &v31x->position_0x4C_76);
    v33x->pitch_0x1E_30 = Maths::sub_58210_radix_tan(&a1x->position_0x4C_76, &v31x->position_0x4C_76);
    v33x->xsubtype_0x42_66 = v31x->model_0x40_64;          // target-class filter: model = avatar's
    v33x->xtype_0x41_65    = v31x->class_0x3F_63;           //                      class = avatar's (3)
    sub_5EF70(v31x);                                        // danger-timer poke on avatar
}
if (v34 >= 0)
    PrepareEventSound_6E450(a1x->id_0x1A_26, -1, v34);      // sound 15 at pyramid
```
**Target = the player avatar `v31x`** (NOT `word_0x36DFC`). The flyer's homing lock
`word_0x96_150` is **NOT** set here → both fly with no pre-locked target; they acquire via
`sub_67CB0` on the first tick (model-3 IS in the acquisition set; model-26/0x1A IS too — see §4).
Aim is seeded straight at the avatar; the xtype/xsubtype filter restricts the victim probe to the
avatar's class(3)/model. Note `sub_581E0`/`sub_58210` are the tan2/radix-tan helpers already in the
banked traces.

---

## 2. The two creators — VERBATIM (confirm; already in flyers/spell-projectiles docs)

str91 dispatch (EF:1567 table): subtype `0x03 → 0x22E500 → sub_4D500` (row EF:1571;
EV:4392–4393); subtype `0x1A → 0x22F180 → sub_4E180` (row EF:1594; EV:4495–4496). No RNG in
either creator. Launch yaw/pitch/owner are set by the caller (§1.4), not the creator.

### 2.1 (9,3) creator `sub_4D500` (EF:34810), sprite 76
```c
event->actionIndex_0x45_69 = 3;
event->class_0x3F_63 = 9;
event->model_0x40_64 = 3;
event->actSpeed_0x82_130 = 384;
event->minSpeed_0x84_132 = 384;
event->mana_0x90_144 = 50;
event->maxLife_0x4 = 0x2000 / 384;            // = 21
event->dword_0xA0_160x = &str_D7BD6[60];      // behavior row 60 (yaw cap 22 / pitch cap 22)
event->struct_byte_0xc_12_15.byte[0] &= 0xF7; // clear targetable bit
AddEventToMap_57D70(event, position);
CopyMaxLifeToLife_49A20(event);
SetEntityIndexAndRot_49CD0(event, 76);        // sprite 76; box = particlesParameters[76].{speed/2,rotSpeed/2}
```

### 2.2 (9,26) creator `sub_4E180` (EF:35266), sprite 320
```c
event->actionIndex_0x45_69 = 27;
event->class_0x3F_63 = 9;
event->model_0x40_64 = 26;
event->actSpeed_0x82_130 = 384;
event->minSpeed_0x84_132 = 384;
event->mana_0x90_144 = 50;
event->maxLife_0x4 = 0x2000 / 384;            // = 21
event->dword_0xA0_160x = &str_D7BD6[60];      // behavior row 60 (yaw cap 22 / pitch cap 22)
event->struct_byte_0xc_12_15.byte[0] &= 0xF7;
AddEventToMap_57D70(event, position);
CopyMaxLifeToLife_49A20(event);
SetEntityIndexAndRot_49CD0(event, 320);       // sprite 320
```

Both use **behavior row str_D7BD6[60]**: yaw turn cap **22**, pitch turn cap **22**, re-aim cadence
divisor 40, acquisition range² **4096** (per the row table in `mc2-class9-spell-projectiles.md`).
maxLife = 21 ticks. Speed constant 384 (min==act → no ramp).

---

## 3. The two flight/tick handlers — VERBATIM

Both delegate to the **core flight tick `sub_65820`** (EF:62882, fully documented in
`mc2-class9-flyers.md` §0.6): polar step by actSpeed along (yaw,pitch); homing re-aim every tick
via `sub_65610` with the row's 22/22 caps once a target is locked; acquire via `sub_67CB0`; ricochet
off shielded (`word[0]&0x8010`) targets via `sub_68740(self,v,0x2D,22)` (sound 28, 1 RNG jitter draw);
water (model ∉{4,22,24,26}) → splash (10,5); expire on life<0/terrain; impact spawns
`(byte_0x43_67, byte_0x44_68)` carrying `subSpellIndex` damage. Owner immunity via `id_0x1A_26`.

### 3.1 (9,3) state 3 `sub_66180` (EF:63340) VERBATIM
```c
void sub_66180(type_entity_0x6E8E* a1x)//247180
{
    v1x = sub_65820(a1x);                                  // <-- core flight tick
    if (a1x->class_0x3F_63)                                // (always true for a live class-9)
    {
        a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;              // RNG draw 1
        v4x.x = a1x->rand_0x14_20 % 0x81u + a1x->position_0x4C_76.x - 96 - 64;
        a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;              // RNG draw 2
        v4x.y = a1x->rand_0x14_20 % 0x81u + a1x->position_0x4C_76.y - 96 - 64;
        v4x.z = a1x->position_0x4C_76.z;
        resultx = IfSubtypeCallCreatingManaSphere_4A190(&v4x, 10, 0);     // trailing spark (10,0)
        if (resultx)
        {
            resultx->struct_byte_0xc_12_15.dword |= 0x10080;
            resultx->id_0x1A_26 = a1x->id_0x1A_26;
            resultx->life_0x8 = 4;
            resultx->animationFrame_0x5C_92 = 3;
            resultx->yaw_0x1C_28 = a1x->yaw_0x1C_28;
        }
        if (v1x)                                           // if core flight produced an impact effect
        {
            v1x->maxLife_0x4 = a1x->byte_0x46_70;          // pass fuse (pyramid=10) into impact life
            v1x->life_0x8 = a1x->byte_0x46_70;
        }
    }
}
```
**Flight law (9,3):** core homing flyer (row 60, yaw/pitch cap 22, speed 384, life 21) that **lays a
random-jittered trailing spark (10,0) every tick** (±64 box around pos; **2 RNG draws/tick**), and
on impact stamps `byte_0x46_70` (=10 from the pyramid) into the spawned impact effect's max/current
life. Impact effect (from pyramid arming) = **(10,17)**, damage `subSpellIndex` = **6000**.

### 3.2 (9,26) state 27 `sub_67890` (EF:59181) VERBATIM
```c
type_entity_0x6E8E* sub_67890(type_entity_0x6E8E* a1x)//248890
{
    resultx = sub_65820(a1x);                             // <-- core flight tick
    if (resultx)                                          // if it hit / detonated this tick
    {
        resultx = Entities_EA3E4[a1x->id_0x1A_26];        // the OWNER entity
        if (resultx > Entities_EA3E4[0] && resultx->class_0x3F_63 == 3)
        {
            v2 = resultx->model_0x40_64;
            if (!v2 || v2 == 1)                            // owner is a player avatar (model 0 or 1)
                resultx->word_0x96_150 = 0;               // clear owner's homing lock (release target)
        }
    }
    return resultx;
}
```
**Flight law (9,26):** core homing flyer (row 60, yaw/pitch cap 22, speed 384, life 21). No trailing
particle. On impact, if the owner is a **class-3 player avatar (model 0/1)** it clears that owner's
`word_0x96_150` (releases the avatar's homing lock). For the pyramid launch the owner is the pyramid
(class 5) so this clause is a no-op; it only fires when a player casts the same subtype. Impact
effect (from pyramid arming) = **(10,22)**, damage `subSpellIndex` = **20**, fuse `byte_0x46_70` = 3
(passed to the (10,22) effect by `sub_65820`'s impact block only when `byte_0x44_68==34`; here 22≠34,
so byte_0x46_70 rides along as a field but is not force-copied into impact life by the core — the
(10,22) effect handler consumes it).

**Neither flight function plays a sound during flight.** Sound 15 is played once at *spawn* by the
pyramid (§1.4). Ricochet (sound 28) and drone-hit (sound 26) can fire inside `sub_65820`. The
class-10 impact effects (10,17)/(10,22) carry their own sounds (out of scope).

---

## 4. ALL other spawn sites for (9,3) and (9,26)

Exhaustive grep of `_4A190(...,9,3)` and `_4A190(...,9,26)` across the engine, plus the str91
creator-address dispatch (EV:4392 / EV:4495) and the EV action-dispatch tables. Complete inventory:

| Model | Site | Fn / context | Impact | subSpell (dmg) | byte_0x46_70 | Notes |
|---|---|---|---|---|---|---|
| (9,3) | EF:13474 | `sub_21AB0` case 9 (**pyramid**) | (10,17) | 6000 | 10 | this doc §1.3 |
| (9,3) | EF:44099 | `sub_6DCA0` a3≤9 (**player spell** 8/9) | (10,17) | `a4->subSpellIndex_2` | (LABEL_59 path) | player fireball-family cast |
| (9,26) | EF:13458 | `sub_21AB0` case 8 (**pyramid**) | (10,22) | 20 | 3 | this doc §1.2 |
| (9,26) | EF:44191 | `sub_6DCA0` a3≤0x15 (**player spell** 21) | (10,22) | `subSpellIndex_2 / life_0x1A` | — | player spell cast |

- **No monster-thunk launcher** spawns (9,3) or (9,26) (unlike (9,20)/(9,21)/(9,9) which have
  `sub_1D0E0`/`sub_1D1A0`/`sub_1D260`).
- **No level-authored / terrain / other-creature site.** Level-placed instances would enter through
  the str91 subtype dispatch (EV:4392 `sub_4D500`, EV:4495 `sub_4E180`), which is the same creator;
  no additional arming path exists.
- (9,3)'s player path (EF:44099) is documented in `mc2-class9-flyers.md` §3.4 as "a3≤9 → (9,3),
  byte 10/17"; (9,26)'s player path (EF:44191) as "a3≤0x15 → (9,26) subtype 0x1A, byte 10/22,
  subSpellIndex = subSpellIndex_2/life_0x1A". This doc confirms both verbatim.

---

## 5. Consolidated constants + port notes + OPEN items

### Constants (pyramid-launched)
| | (9,3) model 3 | (9,26) model 26 (0x1A) |
|---|---|---|
| creator | `sub_4D500` EF:34810 | `sub_4E180` EF:35266 |
| action state | 3 → `sub_66180` EF:63340 | 27 → `sub_67890` EF:59181 |
| sprite | 76 | 320 |
| behavior row | str_D7BD6[60] | str_D7BD6[60] |
| yaw / pitch turn cap | 22 / 22 | 22 / 22 |
| acquisition range² | 4096 | 4096 |
| actSpeed = minSpeed | 384 (constant) | 384 (constant) |
| maxLife | 21 ticks | 21 ticks |
| mana | 50 | 50 |
| launch pos | pyramid + 640·yaw, z+768 | pyramid + 640·yaw, z+768 |
| owner (id) | pyramid id | pyramid id |
| target seed | aim straight at avatar; xtype/xsub = avatar class(3)/model | same |
| homing lock word_0x96_150 | NOT preset (acquire via sub_67CB0) | NOT preset (acquire via sub_67CB0) |
| impact effect | (10,17) | (10,22) |
| damage subSpellIndex | 6000 | 20 |
| byte_0x46_70 | 10 (→ impact life via state-3) | 3 |
| RNG / tick | 2 (trailing-spark x,y) + ricochet | 0 + ricochet |
| flight sound | none (spawn plays 15) | none (spawn plays 15) |
| special | lays (10,0) spark trail each tick; stamps impact life=10 | on impact clears owner-avatar lock (no-op for pyramid owner) |

### Port recommendation (flight law in one line each)
The project's MC2 class-9 port has only (9,13) arrows native; other class-9 states fall back to MC1
handlers. Both m3 and m26 need the **`sub_65820` core homing-flyer law** (already required by the
whole flyer band — port it once): polar step at 384/tick, per-tick re-aim toward the locked target
with **±22 yaw / ±22 pitch caps** (row 60), initial acquisition via `sub_67CB0` (both models are in
its homing case set), owner-immunity by `id_0x1A_26`, ricochet off shielded targets, water→(10,5),
life-21 expiry, impact spawns the armed `(byte_0x43_67,byte_0x44_68)` effect carrying `subSpellIndex`.

- **(9,3)**: `sub_65820` + a per-tick trailing spark `(10,0)` (±64 jitter box, 2 RNG draws) + stamp
  `byte_0x46_70` into the impact effect's life. Pyramid variant: impact (10,17), damage 6000.
- **(9,26)**: `sub_65820` verbatim + on-impact "clear owner-avatar lock" clause (inert when the
  pyramid is the owner — can be a no-op arm for the doomsday case; needed only for the player-cast
  spell-21 path). Pyramid variant: impact (10,22), damage 20.

Because the pyramid does NOT preset `word_0x96_150`, both flyers rely on `sub_67CB0` acquisition —
ensure the ported acquisition includes models 3 and 0x1A in the homing branch (they are, per
`mc2-class9-flyers.md` §0.11 model set {0,3,4,0x12,0x13,0x16,0x1A,0x1C,0x1E}).

### OPEN items
1. **Sprite-76 / sprite-320 collision-box extents**: `SetEntityIndexAndRot_49CD0` copies
   `particlesParameters_D951C[76|320].{speed/2, rotSpeed/2}` into the box — the numeric values live
   in the baked `particlesParameters_D951C[]` table (Type_WORD_D951C.h; 347 entries), not readable
   from source. Extract from baked data if pixel-exact hit boxes matter.
2. **Impact effects (10,17) and (10,22)** are the actual damage/visual carriers (damage formula in
   the class-10 effect handler, out of scope here). (9,3) passes fuse life 10 into (10,17); (9,26)
   passes payload 20 + fuse 3 into (10,22). Trace the class-10 handlers separately for the full
   detonation behavior.
3. **byte_0x46_70 on (9,26)**: `sub_65820`'s impact block only force-copies `byte_0x46_70`→impact
   life when `byte_0x44_68==34`; for (10,22) it is carried but consumed by the effect handler — worth
   confirming when (10,22) is traced.
