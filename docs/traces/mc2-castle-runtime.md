# MC2 CASTLE RUNTIME — port-ready verbatim trace (DELTA vs the MC1 castle column)

> **CORRECTIONS (port session 2026-07-11, verified against the decompile — see
> `mc2-castle-open-items.md` + `mgc_sim::mc2::castle`):**
> 1. **`word_0x80_128` is the UPGRADE-request channel, not a downgrade.** Its writer is the
>    delivered castle-cast entity `sub_389F0` (EV "cast castleII", EF:28240), which also writes
>    `word_0x7C_124 = 10` — the exact MC1 ch5 `(10, owner)` token protocol. The armed
>    `byte[0] |= 0x40` routes the standing tick to action 5 case 0 → `sub_60480` = LEVEL-UP,
>    and the intake gate is `dword_0x10_16 < 7` (an upgrade cap, not a downgrade floor). The
>    sell/downgrade path is player input 0x2A → `life = -1` (§5, correct as written).
> 2. **§5 flood/quake:** `dword_38519` is the **class-3 live list** (EF:39975-39984), so the
>    flood grab DOES target castles — the project's flood port (grab + `f50=30` + ch0 damage)
>    is retail-faithful. The grab damage processes only after the settle timer expires (branch
>    A skips intake), the same "mailbox accrues during the shake" shape as MC1.
> 3. **Headline 4 "NO balloons" is wrong.** Class-3 action 9 = `AddBallon_60AB0` (EF:61763) =
>    the MC2 balloon tick, and `sub_5FF50` maintains an `array_0x3C_60` fleet of (3,3)
>    balloons per `sub_60400` quota — (1,0)/(1,0)/(1,4)/(2,6)/(2,14)/(3,18)/(3,34) by level,
>    byte-identical to MC1's fleet table. §4c's "townsfolk" ARE the balloons.

Port-ready verbatim transcription of the **standing MC2 castle**: its per-tick action handler,
the HP/capacity ladder, damage intake, mana economy, and world-effect consumption — expressed as an
exact DELTA against the MC1 castle column the project currently runs on MC2 worlds
(`crates/mgc-sim/src/mc1/features.rs`, `castle_tick` ~line 2960).

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
EF = `engine/EventsFunctions.cpp`, EV = `engine/Events.cpp`. Trace date 2026-07-11.
House style per `docs/traces/mc2-class10-m67-flood-helpers.md`. Read that first for conventions
(field-name suffixes, uint8-wrap semantics, RNG law `r = 9377*r + 9439`).

The BUILDER side (stage add/remove, `RemoveCastleStage_385C0` EF:28071, `AddBuildingToTerrain`,
the footprint checks `sub_11A10`/`sub_11960`) is being traced in parallel — this doc covers only the
STANDING castle's per-tick behaviour and numbers, plus the intake→downgrade *trigger* (their doc
transcribes the teardown body of `sub_605E0`).

---

## Headline findings (read first)

1. **The MC2 castle is `class 3, model 2`, and it is NOT one action with internal sub-states — it is
   THREE class-3 actionIndices.** The class-3 action table `x_DWORD_D4C52ar_str30` (EF:1201) maps
   actionIndex → function address:
   - **action 4 = `0x2408F0` = `EndOfCastleProjectile_5F8F0` (EF:61055) = the STANDING-castle per-tick
     handler** (regen, ground track, intake dispatch, absorption).
   - **action 5 = `0x240A70` = `BeginOfCastleCreation_5FA70` (EF:61123)** = the build/repaint/upgrade
     state machine (keyed on `word_0x2E_46`, cases 0..6).
   - **action 6 = `0x240CA0` = `sub_5FCA0_destroy_castle_level` (EF:61222)** = the downgrade/destroy
     entry (calls `sub_605E0` when `sub_4A810_get_0x35plus()` says spheres can spawn).
   So MC1's single `castle_tick` with `f59` sub-states **maps to MC2's `actionIndex` field itself**
   (4/5/6), NOT to a byte inside the tick. `word_0x2E_46` is MC2's within-action-5 sub-state (the
   equivalent of MC1's `f59` repaint states). This is the single biggest structural delta.

2. **The ladder capacities DIFFER at every level.** MC1 CAP = `[5000,10000,20000,40000,80000,160000,
   320000,30M]`; **MC2 CAP = `[5000,8500,18000,38800,78600,158200,317400,300000000]`** (`sub_60810`
   EF:61705). The HP *shape* `[—,20000,40000,40000,60000,60000,80000,80000]` matches MC1's tail, but
   MC2 **scales HP by a Life-personality × per-level factor** (`number1`, EF:61704) — MC1 uses flat HP.
   The top-level capacity sentinel is **300000000** (MC2) vs **30000000** (MC1) — a 10× difference.

3. **The intake `sub_609E0` (EF:61734) is a straight subtract with a `word_0x62_98`-gated single
   channel** — NOT a per-channel mail sweep. It reads ONE mailbox (`str_0x5E_94`), subtracts
   `dword_0x5E_94` from `life`, and on lethal returns 2 (→ the tick sets `actionIndex = 6` = destroy).
   It ALSO handles a second, independent channel `word_0x80_128` (the self-referential "arm downgrade
   flag" that sets `byte[0] |= 0x40` when `dword_0x10_16 < 7`). There is **no per-hit hit-grunt sound in
   the castle intake** — the survey's "random 54-57" grunts are the WIZARD intake (`sub_5EFA0`), not the
   castle. The castle plays sound **30** only on downgrade (`sub_605E0`, EF:61627).

4. **MC2 has NO balloons carrying mana to the castle.** `AddBallon_60AB0` (EF:61763) exists but is a
   general mana-absorption helper for class-10 carriers, not a castle feeder. The castle's mana INFLOW
   is: (a) the standing tick absorbs **model-39 mana spheres** that AABB-overlap it (EF:61101-61116),
   and (b) the possessed-building census `sub_60F00` (EF:61959) adds owned-building mana into the
   owner's `dword_0x13C_316` bank (NOT into the castle's own `mana`). The castle's stored mana is
   grown by absorbing spheres; the "banked %" objective adds `dword_0x13C_316 + castle.mana`.

5. **The shake counter is `word_0x30_48`, but on the castle it is the DOWNGRADE/PROJECTILE timer, not a
   30-tick blast shake.** `EndOfCastleProjectile_5F8F0` decrements `word_0x30_48` while >0 (running a
   projectile/settle animation, EF:61062-61078); MC1's `f50 == 30` blast-shake convention does NOT
   have a castle-side counterpart in the standing tick — MC1's `word_0x30_48 = 30` writes are the
   flood/quake GRAB timer on model-2 OBJECTS (EF:29346), a *different* use. **Confirm what the project's
   ported quake/whirlwind castle-grab actually writes** (see §5 OPEN).

---

## 1. `EndOfCastleProjectile_5F8F0` (EF:61055) — the STANDING castle tick (action 4) — VERBATIM

```c
// EF:61055  — class-3 model-2 actionIndex 4
void EndOfCastleProjectile_5F8F0(type_entity_0x6E8E* a1x)//2408f0
{
    if (a1x->word_0x30_48)                                   // (A) projectile/settle animation running
    {
        if (a1x->word_0x30_48 == 1)
        {
            if (!(a1x->struct_byte_0xc_12_15.byte[2] & 0x10)) // not grabbed (bit4)
            {
                a1x->actionIndex_0x45_69 = 5;                //   → hand back to the build/repaint SM
                a1x->word_0x2E_46 = 3;                       //   word_0x2E_46 = 3 (the '10,42 gate' path)
                a1x->word_0x30_48 = 0;
            }
        }
        else
        {
            a1x->word_0x30_48--;                             //   count the animation down
            sub_5F890(a1x, 1);                               //   keep the build-anim ghost in sync
            a1x->position_0x4C_76.z = getTerrainAlt_10C40(&a1x->position_0x4C_76);  // ground-track z
        }
    }
    else                                                     // (B) NORMAL STANDING TICK
    {
        if (sub_609E0(a1x) == 2)                             //   INTAKE (§3): 2 = lethal / already dead
            a1x->actionIndex_0x45_69 = 6;                    //     → destroy-level handler (action 6)
        else if (a1x->struct_byte_0xc_12_15.byte[0] & 0x40)  //   byte[0] bit6 = "downgrade one level" armed
        {
            a1x->word_0x2E_46 = 0;
            a1x->actionIndex_0x45_69 = 5;                    //     → build SM (case 0 rebuilds footprint)
        }
        a1x->position_0x4C_76.z = getTerrainAlt_10C40(&a1x->position_0x4C_76);   // ground-track z
        a1x->playerEntityIndex_0x94_148 = a1x->id_0x1A_26;   //   owner = self.id (census key)
        if (!(a1x->byte_0x3E_62 & 1))                        //   (odd/even tick gate: run heavy work on even)
        {
            sub_5FD00(a1x);                                  //   MANA OVERFLOW EJECTOR (§4a) — spew spheres
            SetShiftByCastle_49EC0(a1x, a1x->dword_0x10_16); //   refresh AABB half-extents from BUILD00DAT
            a1x->array_0x52_82.yaw = -8192;                  //   (0xE000) sprite yaw
            a1x->array_0x52_82.fov = 0x4000;                 //   sprite fov
            sub_5FF50(a1x);                                  //   TOWNSFOLK + TURRET census/spawn (§4c)
            if (a1x->mana_0x90_144 < a1x->maxMana_0x8C_140)  //   room for more stored mana?
            {
                v3x = x_D41A0_BYTEARRAY_4_struct.dword_38523;//     walk the SPHERE list
                if (v3x > Entities_EA3E4[0])
                {
                    while (v3x->model_0x40_64 != 39               // model 39 = mana sphere
                        || v3x->playerEntityIndex_0x94_148 != a1x->id_0x1A_26   // owned by castle's wizard
                        || !CompareAxisWithShift_10750(a1x, v3x))               // AABB overlaps castle
                    {
                        v3x = v3x->next_0;
                        if (v3x <= Entities_EA3E4[0])
                            return;
                    }
                    a1x->mana_0x90_144 += v3x->mana_0x90_144;    //   DEPOSIT: absorb the sphere's mana
                    DisableEntityDrawing04_57F10(v3x);           //   despawn the sphere
                }
            }
        }
    }
}
```

**Exact semantics / processing order (the STANDING branch B):**
1. `sub_609E0` intake first (§3). Returns 2 → set actionIndex 6 (destroy). This is the ONLY lethal exit.
2. Else if `byte[0] & 0x40` (bit6, the "downgrade armed" flag set by intake §3) → actionIndex 5,
   `word_0x2E_46 = 0` (the build SM rebuilds the now-lower footprint).
3. **Ground-track:** `position.z = getTerrainAlt(position)` EVERY tick (both A and B branches).
4. `playerEntityIndex_0x94_148 = self.id` every tick — this is what the census keys on.
5. **Heavy work gated by `!(byte_0x3E_62 & 1)`** — `byte_0x3E_62` is a free-running tick/frame counter,
   so heavy work (overflow ejector + townsfolk census + sphere absorption) runs on **even ticks only**.
   Light work (intake, ground-track) runs every tick.
6. **Sphere absorption:** ONE sphere per (even) tick, first model-39 sphere owned by this wizard that
   AABB-overlaps the castle, iff `mana < maxMana`. `mana += sphere.mana; despawn(sphere)`.
   `CompareAxisWithShift_10750` = the XY-only AABB Minkowski test (see m67-helpers §8).

**MC1 DELTA:** MC1 `castle_tick` runs a `f59` state machine *inside* one action; MC2 branches on
`word_0x30_48` (animation timer) vs a normal tick, and delegates level-change to a DIFFERENT actionIndex
(5 or 6). MC1's "30-tick blast shake on f50" has NO analogue here — `word_0x30_48` is the build/projectile
animation timer, decremented to 0, at which point it hands to action 5 word_0x2E_46=3.

---

## 2. `sub_60810` (EF:61695) — THE LADDER — VERBATIM

```c
// EF:61695
void sub_60810(type_entity_0x6E8E* locEvent)//241810
{
    type_entity_0x6E8E* locEvent2 = nullptr;

    // build-anim ghost (SpellEnabled[2]) — only when the OWNER is a live wizard (action <= 1)
    if ((Entities_EA3E4[locEvent->id_0x1A_26]->actionIndex_0x45_69 <= 1u) &&
        (Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2]))
    {
        locEvent2 = Entities_EA3E4[Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x
                        ->str_611.SpellsEnabled_0x333_819x.SpellEnabled[2]];
    }

    // === THE LIFE-PERSONALITY × PER-LEVEL SCALE FACTOR ===
    // number1 = (Life_personality * ((per_level_factor << 8) + 256)) >> 8
    int number1 = (Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x->word_0x24A_586
                   * ((Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x
                        ->array_0x24E_590[locEvent->dword_0x10_16] << 8) + 256)) >> 8;

    switch (locEvent->dword_0x10_16) {              // dword_0x10_16 = castle STAGE / LEVEL (0..7)
    case 0: sub_60780(locEvent, locEvent2, 0,                 5000);       break;
    case 1: sub_60780(locEvent, locEvent2, 20000 * number1 >> 8, 8500);    break;
    case 2: sub_60780(locEvent, locEvent2, 40000 * number1 >> 8, 18000);   break;
    case 3: sub_60780(locEvent, locEvent2, 40000 * number1 >> 8, 38800);   break;
    case 4: sub_60780(locEvent, locEvent2, 60000 * number1 >> 8, 78600);   break;
    case 5: sub_60780(locEvent, locEvent2, 60000 * number1 >> 8, 158200);  break;
    case 6: sub_60780(locEvent, locEvent2, 80000 * number1 >> 8, 317400);  break;
    case 7: sub_60780(locEvent, locEvent2, 80000 * number1 >> 8, 300000000);break;
    }
}
```

`sub_60780` (EF:61670) is what the ladder pushes those two numbers through:

```c
// EF:61670
void sub_60780(type_entity_0x6E8E* locEvent, type_entity_0x6E8E* locEvent2, int number1, int number2)
{
    if (number1) {                                  // number1 = the scaled maxLife (HP)
        int number3 = 0;
        locEvent->maxLife_0x4 = number1;
        if (locEvent->life_0x8 < 0) {               // if currently dead-negative, cap the debt
            number3 = -locEvent->life_0x8;
            if (-locEvent->life_0x8 > number1 / 2)  //   debt clamped to half the new maxLife
                number3 = number1 / 2;
        }
        locEvent->life_0x8 = locEvent->maxLife_0x4 - number3;   // new life = maxLife - (clamped debt)
    }
    if (locEvent2) {                                // build-anim ghost, if present
        short originalWord46 = locEvent2->word_0x2E_46;
        locEvent2->word_0x2E_46 = 0;
        SetSpell_6D5E0(locEvent2, locEvent2->byte_0x46_70);
        locEvent2->word_0x2E_46 = originalWord46;
    }
    locEvent->maxMana_0x8C_140 = number2;           // number2 = the capacity (maxMana)
}
```

**Exact semantics for the port:**
- **STAGE field:** `dword_0x10_16` (0..7) is the castle level (MC1's `lvl`). Level 0 = footprint only:
  `maxLife = 0` (so `sub_60780` skips the life write — life is left as-is), `maxMana = 5000`.
- **HP array (pre-scale):** `[0, 20000, 40000, 40000, 60000, 60000, 80000, 80000]`.
- **CAPACITY array:** `[5000, 8500, 18000, 38800, 78600, 158200, 317400, 300000000]`. **Verbatim, and
  DIFFERENT from MC1 at every level ≥1.** Note level-0 cap = 5000 (same as level 1's HP-less base).
- **The scale factor `number1`:** `number1 = (LifePers * ((factor[level] << 8) + 256)) >> 8` where
  `LifePers = dword_0xA4_164->word_0x24A_586` (the owner wizard's Life personality, `maxLife = 10000·L/256`
  base per survey) and `factor[level] = dword_0xA4_164->array_0x24E_590[level]` (a per-level 0x24E table
  on the wizard's stat block). Then scaled HP = `HP_base[level] * number1 >> 8` (integer, truncating).
  So a Life=256 wizard with factor=0 gives `number1 = (256 * 256) >> 8 = 256`, and `HP_base * 256 >> 8 =
  HP_base` (identity) — i.e. **factor=0, Life=256 reproduces MC1's flat HP exactly**. Non-zero factor or
  non-256 Life scales it. `<< 8` then `+256` then `>> 8` means factor `f` contributes `(f + 1)×` before the
  Life multiply; the whole thing is `(Life × (factor+1)) >> 8 ... ` → all integer truncation, no rounding.
- **life-on-downgrade preservation:** if the castle is currently `life < 0` (mid-lethal), the new life is
  `maxLife - min(-life, maxLife/2)` — carries at most half the new maxLife as damage debt into the lower
  level. (Relevant during the `sub_605E0` downgrade, which calls `sub_60810` after decrementing stage.)
- **What else the ladder sets:** ONLY `maxLife_0x4`, `life_0x8`, `maxMana_0x8C_140` (via sub_60780).
  It does NOT set extents/sprite/stage-count — the AABB extents come from `SetShiftByCastle_49EC0`
  (§ below) called separately by the tick and by `sub_605E0`/`sub_60480`. The stage count is
  `dword_0x10_16` itself, set by the builder (`sub_60480` increments it, EF:61581) or `sub_605E0`
  (decrements it, EF:61637).

**`SetShiftByCastle_49EC0` (EF:32882) — the AABB extents source:**
```c
// EF:32882
void SetShiftByCastle_49EC0(type_entity_0x6E8E* event, int16_t a2)//22aec0
{
    bitmap_pos_struct_t posistruct = (*filearray_2aa18c[filearrayindex_BUILD00DATTAB].posistruct)[a2];
    if (x_WORD_180660_VGA_type_resolution == 1) { posistruct.height_5 >>= 1; posistruct.width_4 >>= 1; }
    event->array_0x52_82.pitch = ((posistruct.width_4  << 8) + 1280) >> 1;   // XY half-extent (pitch)
    event->array_0x52_82.yaw   = 0;
    event->array_0x52_82.fov   = 256;
    event->array_0x52_82.roll  = ((posistruct.height_5 << 8) + 1280) >> 1;   // XY half-extent (roll)
}
```
`a2` is the STAGE. So the castle's collision/absorption box is `((BUILD00DAT[stage].width<<8)+1280)>>1`
in pitch and `((height<<8)+1280)>>1` in roll — i.e. a stage-sized footprint plus a fixed +1280/2 = +640
world-unit margin. (VGA-1 mode halves the sprite dims first.)

---

## 3. `sub_609E0` (EF:61734) — THE DAMAGE INTAKE — VERBATIM

```c
// EF:61734
int sub_609E0(type_entity_0x6E8E* locEvent)//2419e0
{
    int result = 0;
    if (locEvent->life_0x8 < 0)                         // already dead → 2 (caller → destroy)
        return 2;
    if (locEvent->str_0x5E_94.word_0x62_98)             // MAIL CHANNEL armed (source id present)
    {
        locEvent->life_0x8 -= locEvent->str_0x5E_94.dword_0x5E_94;   // STRAIGHT SUBTRACT (no /10, no shield)
        if (locEvent->life_0x8 < 0)                      // this hit was lethal
        {
            locEvent->word_0x24_36 = locEvent->str_0x5E_94.word_0x62_98;  // remember the KILLER's id
            locEvent->str_0x5E_94.word_0x62_98 = 0;
            return 2;                                    //   → 2 (caller sets actionIndex 6)
        }
        locEvent->str_0x5E_94.word_0x62_98 = 0;          // consume the mail
        locEvent->str_0x5E_94.dword_0x5E_94 = 0;
        result = 1;                                      // 1 = took non-lethal damage this tick
        Entities_EA3E4[locEvent->id_0x1A_26]->dword_0xA4_164x->byte_0x195_405 = 4;  // HUD "castle hit" flag
    }
    if (locEvent->str_0x5E_94.word_0x80_128 == locEvent->id_0x1A_26)   // self-referential DOWNGRADE channel
    {
        if (locEvent->dword_0x10_16 < 7)                 // only if not at max level
            locEvent->struct_byte_0xc_12_15.byte[0] |= 0x40u;   // ARM the one-level downgrade (bit6)
        locEvent->str_0x5E_94.word_0x80_128 = 0;
    }
    return result;
}
```

**Exact semantics for the port:**
- **Channels read:** the single 36-byte mailbox `str_0x5E_94`. Two logical sub-channels:
  - **`word_0x62_98` (damage source id) + `dword_0x5E_94` (damage amount):** if source id nonzero,
    `life -= amount` (STRAIGHT SUBTRACT — no shield quarter, no /10, no knockback; those are wizard-only).
  - **`word_0x80_128` (downgrade request id):** if it equals self.id, arm `byte[0] |= 0x40` (bit6) which
    the tick reads to bump to actionIndex 5. This is how a wizard's "sell/downgrade one level"
    (player input 0x2A, EF:37991) and the flood/quake grab (`word_0x80_128` writes, EF:28240) request a
    single level-down without dealing lethal HP damage.
- **Downgrade trigger into `sub_605E0`:** NOT direct. Two paths reach `sub_605E0`:
  1. **Lethal** (`life < 0` after subtract, or already `<0`) → return 2 → tick sets `actionIndex = 6` →
     next tick `sub_5FCA0_destroy_castle_level` (EF:61222) calls `sub_605E0` (one level off + 10% mana
     haircut + eject, transcribed by the builder-side agent).
  2. **Armed bit6** → tick sets actionIndex 5 word_0x2E_46=0 → build SM `case 0` rebuilds; the actual
     level decrement for the *sell* path also flows through `sub_5FCA0`→`sub_605E0`.
- **Return codes:** 0 = nothing, 1 = took non-lethal damage (sets HUD flag `byte_0x195_405 = 4`),
  2 = dead/lethal.
- **SOUNDS:** `sub_609E0` plays **NO sound**. The only castle sound in this subsystem is the **downgrade
  sound 30** in `sub_605E0` (`PrepareEventSound_6E450(v2, -1, 30)`, EF:61627) and the **upgrade sound 10**
  in `sub_60480` (EF:61578). The survey's "random 54-57 hit grunts" belong to the WIZARD intake
  (`sub_5EFA0`), NOT the castle — confirmed: no grunt in `sub_609E0`.
- **`byte_0x195_405 = 4`** is the owner-wizard's "castle-under-attack" HUD/notify flag (set on any
  non-lethal castle hit) — port as a UI event, no gameplay effect.
- **No shake counter armed here.** The MC1 port's `f50 = 30` blast-shake on castle hit has no counterpart
  in `sub_609E0`. (See §5 OPEN — decide whether to keep the shake as an improved-column extra.)

---

## 4. The MANA ECONOMY — VERBATIM

### 4a. `sub_5FD00` (EF:61241) — stored-mana home + CAP-enforcement OVERFLOW EJECTOR

Called every even tick from the standing handler. Ejects mana above cap as scattered model-39 spheres.

```c
// EF:61241 (core)
v14 = 0;
if (Entities_EA3E4[id]->dword_0xA4_164x->dword_0x13C_316 + a1x->mana_0x90_144 > a1x->maxMana_0x8C_140)
    v14 = a1x->mana_0x90_144 - a1x->maxMana_0x8C_140;   // OVERFLOW = stored mana above the cap
if (!a1x->dword_0x10_16)                                // level 0 castle: eject ALL stored mana
    v14 = a1x->mana_0x90_144;
if (v14 > 0) {
    v3 = v14 / 1000;                                    // number of spheres ~ overflow/1000
    v16 = sub_4A810_get_0x35plus();                     // free sphere-slot budget this frame
    // ... if none free, force v3=8 and set D41A0.dword_0x11e6=-1 ...
    if (v3 < 1) v3 = 1;  if (v3 > 32) v3 = 32;          // clamp sphere count [1,32]
    if (v16 > v3) v16 = v3;
    v15 = v14 / v16;                                    // mana per sphere
    for (result = v16; v16 > v13; ...) {
        v4x = IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 39);   // spawn (10,39) sphere
        if (v4x) {
            v4x->mana_0x90_144 = v15;
            v4x->playerEntityIndex_0x94_148 = a1x->id_0x1A_26;       // owner = castle's wizard
            v4x->rand_0x14_20 = 9377 * v4x->rand_0x14_20 + 9439;
            v4x->actSpeed_0x82_130 = v4x->rand_0x14_20 % 0x30u + 16; // scatter speed rand%48 + 16
            v4x->word_0x2C_44 = (1024 - (z - terrainAlt) - my_sign32(...)*7) >> 3;   // vertical arc
            v10 = a1x->rand_0x14_20 % 0x1400u + 3840;                // scatter dist rand%5120 + 3840
            MoveEntity_57FA0(&pos, a1x->rand_0x14_20 & 0x7FF, 0, v10);// random yaw
            CopyEntityPosition_57CF0(v4x, &pos);
            a1x->mana_0x90_144 -= v4x->mana_0x90_144;                // debit the castle
            ...
        }
    }
}
```

**Exact semantics:**
- **Cap enforcement (the "13C law"):** overflow = `(dword_0x13C_316 + castle.mana) - maxMana`, i.e. the
  cap is enforced against **possessed-building bank + castle stored**, not castle stored alone. Above cap,
  the excess is EJECTED as spheres (owner-tagged), not clamped. **Level-0 castle ejects its entire
  stored mana** (a footprint holds no mana).
- **Sphere spray:** count = `clamp(overflow/1000, 1, 32)` further capped to the free-slot budget; each
  sphere gets `overflow/count` mana, random yaw, scatter dist `rand%5120 + 3840`, speed `rand%48 + 16`,
  vertical arc from `word_0x2C_44`. Same 9377/9439 LCG as the corpse→sphere pipeline.

### 4b. `sub_60F00` (EF:61959) — the per-tick WORLD-MANA CENSUS (banked-% numerator)

```c
// EF:61959 (core)
// (1) reset every player's maxMana to base, zero the possessed-building bank
for each player p:
    p.maxMana_0x8C_140 = p->dword_0xA4_164->byte_0x150_336;   // base maxMana from stat block
    p->dword_0xA4_164->dword_0x13C_316 = 0;                    // ZERO the possessed-building bank
// (2) world mana total starts at 1
x_D41A0_BYTEARRAY_4_struct.str_index_242ar.dword_4 = 1;
// (3) walk all entities; for contributing kinds, call sub_61000
//     contributing kinds:  class 3 model 2/3 (CASTLE + creature3),  class 5,
//                          class 10 model 39 (mana sphere), 0x2D=45 (possessed building), 58
//     class-10 model 0x2D (45, possessed building): ALSO adds its mana to the OWNER's 0x13C bank:
//         v3c->dword_0xA4_164->dword_0x13C_316 += v4x->mana_0x90_144;
```

`sub_61000` (EF:62061):
```c
// EF:62061
type_entity_0x6E8E* sub_61000(type_entity_0x6E8E* a1x)//242000
{
    if (a1x->playerEntityIndex_0x94_148)                       // owned?
        Entities_EA3E4[a1x->playerEntityIndex_0x94_148]->maxMana_0x8C_140 += a1x->mana_0x90_144;
    x_D41A0_BYTEARRAY_4_struct.str_index_242ar.dword_4 += a1x->mana_0x90_144;   // world total += mana
    return Entities_EA3E4[a1x->playerEntityIndex_0x94_148];
}
```

**Exact semantics:**
- Runs once per tick immediately before the entity loop (survey: EF:40115), same slot as MC1's
  `recompute_mana`. Resets each player's `maxMana` to `byte_0x150_336` (base) then ADDS every owned
  contributor's `mana` into the owner's `maxMana` and into `dword_4` (world total).
- **Possessed buildings (class 10 model 45=0x2D) also credit `dword_0x13C_316`** on the owner — this is
  the "possessed-building mana bank" that the objective's numerator adds to castle stored mana.
- **World total `dword_4` starts at 1** (avoids /0 in the objective percentage).

### 4c. `sub_5FF50` (EF:61343) — TOWNSFOLK + DEFENSE-TURRET census (runs from the standing tick)

Per stage, maintains `array_0x3C_60` (up to 3 townsfolk, class 3 model 3, action 9) and `array_0x5C_92`
(up to `v19` class-5 model-15 defense turrets). Dead members are turned to mana spheres
(`TransformEntityToManaSphere_36BA0`) and respawned; the townsfolk mana totals feed
`dword_0x12E_302`/`dword_0x12A_298`. Turret spawn is throttled by `a1x->word_0x2C_44` (16-tick cooldown,
EF:61491). `sub_60400` (EF:61523) gives (townsfolk_count, turret_count) per stage:
stage 1/2 → (1,0); 3 → (1,4); 4 → (2,6); 5 → (2,14); 6 → (3,18); 7 → (3,34).
**This is the standing castle's "population" — MC1 has no equivalent per-stage townsfolk/turret roster in
`castle_tick`; port as an MC2-only arm.** (Defense-turret AI = `sub_3AF00_castle_defend_event` EF:30106,
the (10,79)/(5,15) child spawned by `sub_613D0`.)

### 4d. Wizard AT-CASTLE redirect + grace=2 (`AddPlayer03_00_5E010` EF:59955-59993) — VERBATIM

```c
// EF:59961
if (a1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58)
    if (sub_106C0(a1x, Entities_EA3E4[...CastleEntityIndex...]))   // wizard body overlaps its castle?
        locIsOk = true;
sub_5F380(a1x);
if (!PAUSED && locIsOk) {
    if (a1x->str_0x5E_94.word_0x62_98) {                          // wizard has pending inbox damage
        // FORWARD the wizard's inbox damage into the CASTLE's mailbox:
        if (castle->str_0x5E_94.word_0x62_98)
            castle->str_0x5E_94.dword_0x5E_94 += a1x->str_0x5E_94.dword_0x5E_94;
        else
            castle->str_0x5E_94.dword_0x5E_94  = a1x->str_0x5E_94.dword_0x5E_94;
        castle->str_0x5E_94.word_0x62_98 = a1x->str_0x5E_94.word_0x62_98;
    }
    a1x->dword_0xA4_164x->word_0x159_345 = 2;                     // GRACE = 2
}
if (!PAUSED) {
    if (a1x->dword_0xA4_164x->word_0x159_345) {                   // while grace > 0:
        memset(&a1x->str_0x5E_94, 0, 36);                        //   WIPE the wizard's own inbox
        a1x->dword_0xA4_164x->word_0x159_345--;                  //   (so the intake sees nothing)
    } else {
        sub_5EFA0(a1x);                                          //   else: normal wizard damage intake
    }
    ...
}
```

**Exact semantics:** when the wizard body overlaps its own castle (`sub_106C0`), its pending inbox damage
is **forwarded into the castle's mailbox** (so the castle takes the hit via `sub_609E0`) and the wizard's
grace `word_0x159_345` is set to **2**; while grace > 0 the wizard's own 36-byte inbox is `memset` to 0
each tick (nulling the wizard intake `sub_5EFA0` for those 2 ticks). Grace also decrements once here per
tick. Spawn value of grace is 100 (survey :43711). **IDENTICAL protocol to MC1** (the project already
matches this per the memory ledger); the only MC2 flavour is that castle intake is the straight-subtract
`sub_609E0`, not a wizard-style intake.

---

## 5. Castle vs the WORLD — area effects, owner death, transfer

- **Area effects reach the castle through the mailbox `str_0x5E_94`, same as any entity.** The flood/quake
  (`sub_3A090`, EF:29346) grabs model-2 objects (the castle IS class 3 not class 10, so it is NOT grabbed
  by the class-10 flood's model-2 grab — that grab targets class-10 model-2 objects on `dword_38519`). The
  quake/whirlwind castle-grab the project ported writes `word_0x80_128 = self.id` (the DOWNGRADE channel)
  — which `sub_609E0` consumes to arm `byte[0] |= 0x40` = one level-down. **Confirm the project's port
  writes `word_0x80_128` (downgrade request), not `f50=30`+ch0 damage:** the AUTHENTIC quake→castle
  interaction is a *level knock-down*, not raw HP damage. The single downgrade-request site in the world is
  `RemoveCastleStage`/sell (EF:37991 `word_0x80_128`-adjacent) and the terrain author. (See OPEN.)
- **Owner (wizard) death:** the castle is NOT directly torn down when its wizard dies. The **only** place
  `CastleEntityIndex_0x3A_58 = 0` is cleared is `sub_605E0`'s `!dword_0x10_16` branch (EF:61664) — i.e.
  the castle unlinks from its owner only when it is destroyed down to stage 0. A dead wizard's castle keeps
  standing (running action 4) until an enemy grinds its levels off via the intake→downgrade chain. The
  wizard's death scatters its spellbook (survey), but does not despawn the castle.
- **Sell / player-command downgrade:** player input **0x2A** (EF:37991) sets the owner's castle
  `life_0x8 = -1` directly (→ next castle tick `sub_609E0` returns 2 → actionIndex 6 → `sub_605E0`
  destroy-one-level) and if the castle is at stage 1 sets `byte_0x1BE_446 = 1`.
- **Claim / transfer:** NONE at the standing-tick level — a castle belongs to `id_0x1A_26` for life; there
  is no capture. Ownership only ends via destruction (stage → 0, CastleEntityIndex cleared).

---

## 6. Consolidated constants + MC1-vs-MC2 DELTA + OPEN

### Constants table

| constant | value | meaning | cite |
|---|---|---|---|
| castle taxonomy | class 3, model 2 | standing castle entity | EF:33378, EV:2937 |
| STANDING tick | actionIndex 4 = `0x2408F0` `EndOfCastleProjectile_5F8F0` | per-tick handler | EF:1206, :61055 |
| build/repaint SM | actionIndex 5 = `0x240A70` `BeginOfCastleCreation_5FA70` | word_0x2E_46 cases 0..6 | EF:1207, :61123 |
| destroy-level | actionIndex 6 = `0x240CA0` `sub_5FCA0_destroy_castle_level` | → `sub_605E0` | EF:1208, :61222 |
| stage field | `dword_0x10_16` ∈ [0..7] | castle level | EF:61705 |
| HP array (pre-scale) | `[0,20000,40000,40000,60000,60000,80000,80000]` | maxLife per level | EF:61707-61728 |
| CAP array | `[5000,8500,18000,38800,78600,158200,317400,300000000]` | maxMana per level | EF:61707-61728 |
| HP scale | `number1 = (LifePers * ((factor[lvl]<<8)+256)) >> 8`, HP=`base*number1>>8` | Life-personality scale | EF:61704 |
| LifePers field | `dword_0xA4_164->word_0x24A_586` | owner Life stat | EF:61704 |
| level factor table | `dword_0xA4_164->array_0x24E_590[lvl]` | per-level HP factor | EF:61704 |
| AABB extents | `pitch=((w<<8)+1280)>>1`, `roll=((h<<8)+1280)>>1` | from BUILD00DAT[stage] | EF:32890-32893 |
| intake | `sub_609E0` straight subtract | EF:61734 |
| intake damage channel | `str_0x5E_94.{word_0x62_98=src, dword_0x5E_94=amt}` | `life -= amt` | EF:61741 |
| intake downgrade channel | `str_0x5E_94.word_0x80_128 == self.id` → `byte[0]|=0x40` | one level-down | EF:61753-61757 |
| lethal → destroy | intake returns 2 → actionIndex 6 | EF:61082, :61738/:61746 |
| killer memory | `word_0x24_36 = damage src id` on lethal | EF:61744 |
| HUD hit flag | `dword_0xA4_164->byte_0x195_405 = 4` on non-lethal hit | EF:61751 |
| heavy-work gate | `!(byte_0x3E_62 & 1)` (even ticks) | EF:61094 |
| sphere absorb | model 39 + owned + AABB overlap, `mana += sphere.mana` | EF:61101-61116 |
| overflow ejector | `sub_5FD00`: eject `(bank+mana)-cap` as spheres | EF:61263-61264 |
| level-0 ejects all | `if !dword_0x10_16: v14 = mana` | EF:61265-61268 |
| sphere count | `clamp(overflow/1000, 1, 32)` capped to free slots | EF:61274-61295 |
| census | `sub_60F00` per tick, world total `dword_4` starts at 1 | EF:61959, :61997 |
| possessed-building bank | class-10 model-45 mana → owner `dword_0x13C_316` | EF:62028 |
| objective type-0 | `100*(dword_0x13C_316 + castle.mana)/dword_4 >= threshold`, castle-gated, NO debounce | EF:40751 |
| upgrade sound | 10 (`sub_60480`) | EF:61578 |
| downgrade sound | 30 (`sub_605E0`) | EF:61627 |
| downgrade mana haircut | `10 * maxMana / 100` (10%) | EF:61622 |
| at-castle redirect | forward wizard inbox → castle mailbox; grace=2 | EF:59970-59978 |
| grace field / spawn | `word_0x159_345`, =2 at-castle, =100 on spawn | EF:59978, :43711 |
| townsfolk/turret roster | `sub_60400` per stage (see §4c) | EF:61523 |
| CastleEntityIndex clear | ONLY in `sub_605E0` stage-0 branch | EF:61664 |
| sell command | player input 0x2A sets castle `life = -1` | EF:37991-37996 |

### MC1 ↔ MC2 DELTA (verb-column terms)

| aspect | MC1 column (project today) | MC2 | shared or swapped? |
|---|---|---|---|
| tick structure | one `castle_tick`, sub-states on `f59` | THREE actionIndices 4/5/6; standing = 4 | **SWAPPED** (f59 → actionIndex) |
| repaint sub-state | `f59` (0 level-up, 4 standing, repaint states) | `word_0x2E_46` (cases 0..6) inside action 5 | SWAPPED |
| HP array | flat `[40000,20000,40000,40000,60000,60000,80000,80000]` | same *shape* but `[0,20000,...]` at lvl0 + **Life×factor scale** | **SWAPPED numbers** |
| capacity array | `[5000,10000,20000,40000,80000,160000,320000,30M]` | `[5000,8500,18000,38800,78600,158200,317400,300M]` | **SWAPPED** (differ every level; 10× top) |
| intake | mail-channel damage intake | `sub_609E0` straight subtract, single mail + downgrade channel | shared shape, MC2 = straight subtract |
| shake | 30-tick blast shake on `f50` (`word_0x30_48`) | **NONE** — `word_0x30_48` = projectile/settle timer | **SWAPPED** (no shake) |
| mana deposit | balloon roster → castle | **NO balloons**; model-39 spheres absorbed on overlap + census | **SWAPPED** carrier |
| overflow | cap clamp | overflow EJECTED as scattered spheres (`sub_5FD00`) | MC2 richer |
| objective numerator | `possessed + castle stored`, banked-% | IDENTICAL formula, castle-gated, no debounce | **SHARED** |
| population | none | per-stage townsfolk (class3/model3) + turrets (class5/model15) | **MC2-only arm** |
| grace redirect | grace protocol identical | forward inbox → castle, grace=2 | **SHARED** |
| owner death | (verify project) | castle survives; unlink only at stage 0 | check project |

### OPEN

- **`array_0x24E_590` level-factor table contents** (the per-level HP multiplier) not dumped — it lives on
  the wizard stat block (`dword_0xA4_164`), likely loaded from `wizards.json`/personality. For a Life=256,
  factor=0 wizard the scale is identity (= MC1 flat HP). **Port must locate this table's source** (probably
  the same personality block that sets `word_0x24A_586` Life); confirm with a state-hash on a scaled castle.
- **The project's ported quake/whirlwind castle grab:** the task says it "writes f50=30 + ch0 mail". The
  AUTHENTIC MC2 castle-vs-quake interaction is a **downgrade request via `word_0x80_128`** (one level off),
  NOT raw ch0 HP damage or a shake. Reconcile: either the project's port models the quake as HP damage
  (works, but not authentic) or should write the downgrade channel. Flag for a fidelity pass. The
  castle-side consumption of `word_0x80_128` is confirmed verbatim here (EF:61753).
- **`byte_0x3E_62` exact period** (the even/odd heavy-work gate) not dumped — it is a free-running counter;
  the `& 1` gate means heavy work every OTHER tick. Confirm the field increments once per frame (it is used
  as `% v21` in `sub_5FF50` too, EF:61409, so it is a general per-castle frame counter).
- **`sub_605E0` teardown body** (10% haircut, ejection, `RemoveCastleStage`, stage decrement, re-ladder)
  is the builder-side agent's deliverable — only the intake→downgrade TRIGGER is transcribed here (§3).
  The trigger→`sub_605E0` path is: intake returns 2 → actionIndex 6 → `sub_5FCA0_destroy_castle_level`
  (EF:61224) gates on `sub_4A810_get_0x35plus()` then calls `sub_605E0`.
- **`word_0x2C_44` on the castle** doubles as the turret-spawn cooldown (16) AND (in `sub_5FD00`) the
  sphere vertical-arc seed — confirm no aliasing bug in the port (they run in different sub_5FF50/sub_5FD00
  passes on the same field within one tick).
