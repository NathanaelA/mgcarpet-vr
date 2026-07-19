# MC2 Projectile TERRAIN + WATER contact laws — Verbatim Trace

All citations `file:line` in `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
EF = `EventsFunctions.cpp`, T = `Terrain.cpp`, E = `Events.cpp`.
Companion traces (flight skeleton, aim, possession delivery — do not re-derive):
`mc2-class9-spell-projectiles.md`, `mc2-autoaim.md`, `mc2-possession-delivery.md`.

Trace date 2026-07-11. Purpose: settle the PLAYTEST report that the FIREBALL wrongly follows/skims terrain in our port, and that water despawns projectiles flying *over* water.

---

## TL;DR — THE LAWS (each proven below with verbatim lines)

1. **FIREBALL DETONATES ON TERRAIN CONTACT. It does NOT skim.** The prior reading ("clamp z, then keep flying") was WRONG. In `sub_65C20` the ground-contact branch clamps z **only to position the burst**, then unconditionally sets the impact flag `v20 = 1` and detonates that same tick (spawns the class-10 impact effect + despawns). Fireball has **no pre-move terrain clamp**, so it flies a pure ballistic path and bursts the instant the terrain rises above its z. (EF:63130-63158.)

2. **POSSESSION IS THE ONE STATE THAT SKIMS.** `CastPosses_65F60` is the *only* flight state that runs a **PRE-move terrain clamp** (raise the just-moved z up to ground level *before committing*, EF:63262-63270). That pre-clamp is what makes the possession bolt hug the ground: the subsequent post-move terrain test then reads equal-to-ground and falls through to the life-countdown / continue path, so gentle terrain does **not** detonate it — it skims. (Steep rises / cave ceiling still stop it.)

3. **THE v20/v15 FLAG DOES NOT SELECT CLAMP-vs-DETONATE.** `v20` (fireball) / `v15` (possession/generic) / `v14` (sub_65820) is simply the **"impact this tick" flag**. It is set on: a victim hit, a terrain-contact-non-water event, OR life expiry. It is NOT a per-entity mode switch and there is no `v20=1`-selects-clamp reading — clamp always happens on contact; whether the projectile then continues is decided by which *branch* set (or didn't set) the flag. Fireball's terrain-contact branch ALWAYS sets it; possession's terrain branch sets it only when the pre-clamp already lifted the projectile into the ground (i.e. a real wall), otherwise it goes to life-countdown.

4. **WATER SPLASH IS NESTED INSIDE THE TERRAIN-CONTACT BRANCH.** The water test (`sub_104D0_terrain_tile_is_water == 1`, EF:63141 / 62956 / 63511 / 58792) sits *inside* the `if (terrainAlt > z || caveFloor)` block. A projectile flying **over** water at altitude never reaches it — the outer terrain-contact `if` is false, so the water branch is skipped entirely. Water only splashes when the projectile has actually descended to/into the water *surface tile*. **Our port's unconditional per-tick water test is the bug** for (b).

5. **WATER LAW = same for every state, model-gated.** On water contact: spawn `_4A190(pos, 10, 5)` (the splash effect, id inherited), then `DisableEntityDrawing04` (despawn) — **no normal impact effect, no impact-XP**. Model exemptions: fireball/lightning states exempt `model==4` only; the generic `sub_65820` exempts `model ∈ {4,22,24,26}`. There is **no** separate possession-vs-fireball water behavior at the flight level — but note `CastPosses_65F60` has **NO water branch at all** (it skims, never reaches the terrain-contact `if` on flat water), so possession simply flies over/into water and dies by contact-detonate or life, never by splash.

6. **NO minimum-height skim clamp exists for offensive projectiles.** The only "follow the ground" behaviour in the whole family is possession's pre-move clamp (EF:63262-63270). Every other player-castable state (fireball, lightning I/II, generic 2–6/8/0xB) moves ballistically and detonates on contact. The earlier "terrain-skim is shared retail flight law" note conflated possession's pre-clamp with the generic states — it is NOT shared; see §6.

---

## 0. Shared terrain primitives

**`getTerrainAlt_10C40(axis)`** (T:2146) `= sub_B5C60_getTerrainAlt2(x, y)` — interpolated GROUND altitude at world (x,y). Higher z = higher up; a projectile is "into the ground" when `groundAlt > proj.z`.

**`sub_10C60(axis)`** (T:2157) `= sub_B5D68(x,y)` — the SECOND heightmap (cave ceiling). Used only when `isCaveLevel_D41B6`.

**`sub_104D0_terrain_tile_is_water(axis)`** (T:2058):
```c
uint32_t sub_104D0_terrain_tile_is_water(axis_3d* axis3d) {          // T:2058
    axis2d.x = axis3d->x >> 8;  axis2d.y = axis3d->y >> 8;           // world → tile
    return sub_10590_terrain_tile_type(mapTerrainType_10B4E0[axis2d.word]);
}
```
`sub_10590_terrain_tile_type` (T:2067) is a `switch(tileType)`: **case 0 → returns 1** (T:2072-2074); every other tile type returns a different nonzero mask (1,2,4,8,0x10,…) or 0. So `== 1` is true **iff the tile type byte is 0 = WATER**. It is a per-tile map-cell type lookup, NOT a bit on the entity. (T:2067-2143.)

**`IfSubtypeCallCreatingManaSphere_4A190(pos, type, subtype)`** (E:5186): spawns the class-`type` model-`subtype` effect if the creation table has it, else 0. Terrain impact uses the projectile's stored `byte_0x43_67` (type) / `byte_0x44_68` (subtype); water uses the hard-coded `(10, 5)` splash.

---

## 1. Q1 — the shared generic flight helper `sub_65820` (EF:62882) terrain law

`sub_65820` backs states 2–6, 0xB. Move + terrain block VERBATIM:

```c
predictedAxis_EB398ar = a1x->position_0x4C_76;                                    // EF:62932
MoveEntity_57FA0(&predictedAxis_EB398ar, yaw, pitch, actSpeed);                   // EF:62933  ballistic step
CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);                            // EF:62934  COMMIT raw z (no pre-clamp!)
v4x = sub_10780(a1x);                                                             // EF:62935  victim probe
v5x = v4x;
if (v4x) { … shield ricochet / snap-to-victim … v14 = 1; goto LABEL_29; }         // EF:62937-62945  HIT
v6 = getTerrainAlt_10C40(&a1x->position_0x4C_76);                                 // EF:62947  ground at committed pos
v7 = a1x->position_0x4C_76.z;                                                     // EF:62948
predictedAxis_EB398ar.z = v6;
if (v6 > v7                                                                       // EF:62950  GROUND ABOVE PROJ → contact
    || isCaveLevel_D41B6
    && (predictedAxis_EB398ar.z = sub_10C60(&pos) - box.fov, pos.z > predicted.z))// EF:62952  cave-ceiling contact
{
    a1x->position_0x4C_76.z = predictedAxis_EB398ar.z;                            // EF:62954  clamp z to surface
    v8 = a1x->model_0x40_64;
    if (v8 != 4 && v8 != 22 && v8 != 24 && v8 != 26                               // EF:62956  water model-gate
        && sub_104D0_terrain_tile_is_water(&pos) == 1) {                          //           water tile?
        v9x = _4A190(&pos, 10, 5);  if (v9x) v9x->id = a1x->id;                    // EF:62958  SPLASH
        DisableEntityDrawing04_57F10(a1x);                                        // EF:62961  despawn, NO impact
        goto LABEL_29;
    }
    goto LABEL_28;                                                                // EF:62964  → v14=1 DETONATE
}
v10 = a1x->life_0x8 - 1;  a1x->life_0x8 = v10;                                    // EF:62966  NO contact → life--
if (v10 < 0) LABEL_28: v14 = 1;                                                   // EF:62968-62970  life spent → detonate
LABEL_29: if (!v14) return 0;                                                     // EF:62972  no impact → keep flying
… sub_68AC0 drone-hit … else _4A190(pos, byte_0x43_67, byte_0x44_68) …            // EF:62974-62996  IMPACT + despawn
```

**Terrain-contact condition:** `getTerrainAlt(pos) > pos.z` (ground risen above the ballistic z), OR on cave levels the ceiling test `pos.z > sub_10C60(pos) - box.fov`.
**The "z-clamp arm":** `a1x->position.z = predicted.z` (EF:62954) — this is NOT a keep-flying clamp; it is the position where the burst is placed. It is immediately followed (EF:62964 `goto LABEL_28`) by `v14 = 1`.
**The "impact/detonate arm":** `v14 = 1; goto LABEL_29` → spawn `_4A190(pos, byte_0x43_67, byte_0x44_68)` + `DisableEntityDrawing04` (EF:62979-62995).
**What selects between clamp-continue and detonate:** NOTHING selects "clamp then continue" here — a terrain hit ALWAYS detonates (unless it is water, which splashes-and-despawns). The only "continue flying" path is the `else` (EF:62966, no contact) when life remains. `v14` is the impact flag, not a mode.

---

## 2. Q2 — FIREBALL `sub_65C20` (EF:63058) vs POSSESSION `CastPosses_65F60` (EF:63210)

### 2.1 Fireball `sub_65C20` — DETONATES on terrain (verbatim)

Move (NO pre-clamp) then terrain block:
```c
v16x = a1x->position_0x4C_76;  predictedAxis_EB398ar = v16x;                      // EF:63124-63125
MoveEntity_57FA0(&predictedAxis_EB398ar, yaw, pitch, actSpeed);                   // EF:63126  ballistic step
CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);//move projectile?          // EF:63127  COMMIT raw z
v8x = sub_10780(a1x);  v9x = v8x;                                                 // EF:63128  victim probe
if (!v8x) {                                                                       // EF:63130  NO victim
    v11 = getTerrainAlt_10C40(&pos);                                              // EF:63132
    v12 = a1x->position_0x4C_76.z;                                                // EF:63133
    predictedAxis_EB398ar.z = v11;
    if (v11 > v12                                                                 // EF:63135  GROUND ABOVE → contact
        || isCaveLevel_D41B6
        && (predicted.z = sub_10C60(&pos) - box.fov, pos.z > predicted.z)) {      // EF:63137  cave ceiling
        v16x.z = predicted.z;  CopyEntityPosition_57CF0(a1x, &v16x);              // EF:63139-63140  clamp z (for burst)
        if (a1x->model_0x40_64 != 4 && sub_104D0_terrain_tile_is_water(&pos)==1){ // EF:63141  WATER? (nested!)
            v13x = _4A190(&pos, 10, 5);  if (v13x) v13x->id = a1x->id;            // EF:63143  SPLASH
            DisableEntityDrawing04_57F10(a1x);                                    // EF:63146  despawn, NO impact
            goto LABEL_35;
        }
        // NOT water → fall through …
    } else {                                                                      // EF:63150  NO terrain contact
        v14 = a1x->life_0x8 - 1;  a1x->life_0x8 = v14;                            // EF:63152  life--
        if (v14 >= 0) goto LABEL_35;                                             // EF:63154  life remains → KEEP FLYING (v20 stays 0)
    }
    v20 = 1;                                                                      // EF:63157  ← reached on {terrain-hit-non-water} OR {life spent}
    goto LABEL_35;                                                                // EF:63158
}
… (victim path sets v20 = 1) …                                                    // EF:63160-63173
LABEL_35: if (v20) { … _4A190(pos, byte_0x43_67, byte_0x44_68); … DisableEntityDrawing04; } // EF:63174-63200  DETONATE + despawn
```

**Control-flow proof the fireball detonates on ground:** on terrain contact the inner `if(water)` is false (dry ground), so control leaves the `if(v11>v12…)` block and reaches EF:63157 `v20 = 1` → `LABEL_35` → `_4A190(pos, byte_0x43_67, byte_0x44_68)` + `DisableEntityDrawing04_57F10(a1x)` (EF:63183-63195). The z-clamp at EF:63139-63140 is purely to place the explosion on the surface; the projectile is destroyed the same tick.
**Mechanism:** impact-effect spawn (`_4A190`) + `DisableEntityDrawing04` (despawn) — NOT a state transition, NOT health-zero. Fireball model is 0; `model != 4` → the water branch *does* apply to fireballs (fireballs splash on water). Fire-XP `sub_6D8B0(id, 0, 1)` fires only if a real victim was hit (`v9x > Entities[0]`, EF:63188-63189) — a pure ground burst gives NO XP.

**Why the fireball can't skim:** there is NO pre-move terrain clamp before EF:63127. The committed z is the raw ballistic `MoveEntity` output, so `getTerrainAlt > z` becomes true as soon as the ground rises past the flight line → immediate detonate.

### 2.2 Possession `CastPosses_65F60` — SKIMS (verbatim)

```c
predictedAxis_EB398ar = a1x->position_0x4C_76;                                    // EF:63260
MoveEntity_57FA0(&predictedAxis_EB398ar, yaw, pitch, actSpeed);                   // EF:63261  ballistic step
v3 = getTerrainAlt_10C40(&predictedAxis_EB398ar);                                 // EF:63262  ← PRE-MOVE terrain read
if (v3 > predictedAxis_EB398ar.z) predictedAxis_EB398ar.z = v3;                   // EF:63263-63264  RAISE to ground (SKIM)
if (isCaveLevel_D41B6) {                                                          // EF:63265
    v4 = sub_10C60(&predictedAxis_EB398ar) - box.fov;
    if (v4 < predictedAxis_EB398ar.z) predictedAxis_EB398ar.z = v4;               // EF:63268-63269  duck under ceiling
}
CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);                            // EF:63271  COMMIT clamped z
v5x = sub_108B0(a1x);  v6x = v5x;  v7x = v5x;                                     // EF:63272  possession victim probe
if (!v5x) {                                                                       // EF:63275  no victim
    v9 = a1x->position_0x4C_76.z;                                                 // EF:63278  (= just-clamped z)
    predictedAxis_EB398ar.z = getTerrainAlt_10C40(&a1x->position_0x4C_76);        // EF:63279  ground at committed pos
    if (predicted.z > v9                                                          // EF:63280  ground STILL above? (only if pre-clamp wasn't enough / steep)
        || isCaveLevel_D41B6 && (predicted.z = sub_10C60(&pos)-box.fov, pos.z > predicted.z)) {
        a1x->position_0x4C_76.z = predicted.z;                                    // EF:63289  final clamp
        // NO water branch here — see below
    } else {                                                                      // EF:63291  ground level reached → normal
        v11 = a1x->life_0x8 - 1;  a1x->life_0x8 = v11;                           // EF:63293  life--
        if (v11 >= 0) goto LABEL_19;                                             // EF:63295  KEEP FLYING (SKIM continues)
    }
    v15 = 1;  goto LABEL_19;                                                      // EF:63298  detonate (steep wall or life spent)
}
sub_65580(v5x); CopyEntityPosition(a1x, &v6x->pos); sub_655A0(v6x); v15 = 1;      // EF:63301-63304  victim → detonate
LABEL_19: if (v15) { _4A190(pos, byte_0x43_67, byte_0x44_68); … DisableEntityDrawing04; } // EF:63306-63319
```

**Possession's terrain arm SKIMS because of the PRE-move clamp (EF:63262-63264).** After that clamp the committed z equals the ground alt, so the *post-move* test `getTerrainAlt(pos) > pos.z` (EF:63280) is FALSE on flat/gently-rising ground (equal, not greater) → the `else` runs → `life--`, and if life remains → `goto LABEL_19` with `v15 == 0` → **no detonation, projectile continues at ground height**. This is the skim. It only sets `v15 = 1` (detonate) when the ground rises so fast that even after the pre-clamp the ground is *still* above (a true wall), or on the cave ceiling, or on life expiry.
**Key structural difference vs fireball:** fireball has NO EF:63262-63264-equivalent pre-clamp. Possession's pre-clamp is the entire mechanism of "possession skims, fireball detonates". Both otherwise share the `_4A190(pos, byte_0x43_67, byte_0x44_68)` detonation and possession-XP `sub_6D8B0(id, 1, 1)` (EF:63313-63314, victim-only).
**Possession has NO water splash branch** — because with the pre-clamp it never satisfies the post-move `terrainAlt > z` on flat water, it simply flies over/through water and dies by contact-detonate (steep bank) or life. (Verify in playtest: possession bolt does not splash.)

---

## 3. Q3 — WATER law, nesting confirmed

Verbatim, fireball case (representative; generic/state-8/state-0xC identical modulo model-gate):
```c
if (v11 > v12 || caveFloor) {                          // EF:63135  OUTER: terrain-contact only
    v16x.z = predicted.z; CopyEntityPosition(a1x, &v16x);                        // EF:63139-63140
    if (a1x->model_0x40_64 != 4 && sub_104D0_terrain_tile_is_water(&pos) == 1) { // EF:63141  INNER: water
        v13x = _4A190(&pos, 10, 5);                                              // EF:63143  splash effect (10,5)
        if (v13x) v13x->id_0x1A_26 = a1x->id_0x1A_26;                            // EF:63144-63145  inherit owner
        DisableEntityDrawing04_57F10(a1x);                                       // EF:63146  DESPAWN
        goto LABEL_35;                                                           // (LABEL_35 sees v20==0 → no normal impact)
    }
}
```

- **Nested inside terrain-contact?** YES. The water `if` (EF:63141) is inside the terrain-contact `if` (EF:63135). Confirmed for `sub_65820` (EF:62956, inside EF:62950), fireball (EF:63141 inside 63135), state-8 (EF:63511 inside 63498), state-0xC (EF:63792 inside 63783). **A projectile flying over water at altitude never runs the water test** → the (b) fix is: gate water on terrain-contact, exactly as retail does.
- **What is tested:** `sub_104D0_terrain_tile_is_water(pos) == 1`, i.e. the MAP CELL type byte `mapTerrainType_10B4E0[(x>>8) + (y>>8)*256]` equals 0 (= water tile). It is a per-tile map lookup, not an entity flag/bit.
- **On water contact:** spawn `(10, 5)` splash (owner id inherited), then `DisableEntityDrawing04` (despawn). **No** `byte_0x43_67/byte_0x44_68` impact effect, **no** `sub_6D8B0` XP, **no** distinct sound in the flight fn (the `(10,5)` effect self-sounds; splash sound id ≈ 27 seen at a sibling spawner EF:26693 — OPEN whether `(10,5)` plays it).
- **Possession vs fireball on water:** fireball (and the generic/lightning states) splash; **possession never reaches a water branch** (§2.2) so it does not splash — the only per-state divergence is possession's missing water arm, a consequence of its skim.

---

## 4. Q4 — per-state terrain/water table (player-castable states)

| State (fn) | model(s) | Pre-move clamp? | Terrain contact | Water contact | Cite |
|---|---|---|---|---|---|
| 0x00 fireball `sub_65C20` | 0 | **NO** | DETONATE (`_4A190(byte43,byte44)` + despawn) | model≠4 → splash `(10,5)` + despawn | EF:63124-63200 |
| 0x01 possession `CastPosses_65F60` | 1 | **YES (EF:63262-64)** — SKIMS | detonate only on steep wall/ceiling/life-out | **none** (skims; no water branch) | EF:63260-63319 |
| 0x02–0x06 generic `sub_65820` | 2,3,4,5,6,10,11 | NO | DETONATE | model ∉{4,22,24,26} → splash `(10,5)`+despawn | EF:62932-62996 |
| 0x08 `sub_662E0` (also 0x07) | 8 | NO | DETONATE (owner-model{0,1} vs else impact split) | model≠4 → splash `(10,5)`+despawn | EF:63475-63563 |
| 0x0B `sub_66FB0` | 11 | NO (`return sub_65820`) | DETONATE | model ∉{4,22,24,26} → splash | EF:58685-58688 |
| 0x0C lightning-II `sub_66FD0` | 12 | NO | DETONATE (impact hard-coded `_4A190(10,38)`) | model≠4 → splash `(10,5)`+despawn | EF:58760-58842 |
| 0x09 lightning-beam `sub_66750` | 9 | n/a — one-tick hitscan (not a flight loop) | beam traces to first blocker via `sub_66610`; impact `_4A190(byte43,byte44)` at beam end | n/a (no per-tick terrain descent) | EF:58268+ (see class-9 trace §0x09) |

Every ballistic state EXCEPT possession detonates on terrain contact. Only possession skims. Water is nested-in-terrain for all of them; possession has no water arm.

---

## 5. Q5 — the impact/detonation path

On any detonation (victim hit, dry-terrain contact, or life expiry — flag set), all states run at `LABEL_35`/`LABEL_26`/`LABEL_19`:
1. **Drone pre-check** `sub_68AC0(self, victim)` (EF:55397) — if the "victim" is the friendly guide drone: spawn `(10,0)`, sound 26, despawn self, return (no normal impact).
2. **Impact spawn** `v = IfSubtypeCallCreatingManaSphere_4A190(&pos, byte_0x43_67, byte_0x44_68)` (E:5186) — spawns the class-10 effect the caster stored (fireball `(10,0)`, possession `(10,12)`, lightning-II hard-codes `(10,38)`, EF:58821). This effect entity carries the damage/sound.
3. If spawned: `sub_65780` (accuracy stats), `sub_686D0` (owner auto-retarget), and **impact-XP `sub_6D8B0(ownerId, spellIdx, 1)` ONLY when a real victim was struck** (`v9x > Entities[0]`) — fireball idx 0 (EF:63188-63189), possession idx 1 (EF:63313-63314), lightning idx 7 (EF:58825-58826). A **pure terrain burst spawns the effect but awards NO XP** (the `if (v9x > Entities[0])` guard is false).
4. Copy `id/yaw/pitch/subSpellIndex` (and `byte_0x46_70`, and target index) onto the impact entity; `DisableEntityDrawing04_57F10(self)` (despawn projectile).

So terrain impact = same class-10 effect as an entity hit, minus the XP and minus victim bookkeeping. Water impact = a *different*, hard-coded `(10,5)` splash and skips this whole block.

---

## 6. Q6 — altitude / minimum-height clamp; reinterpreting the earlier "shared skim" reading

**There is NO minimum-height (skim) clamp for offensive projectiles.** Grep of all flight states: the only pre-move ground-raise is possession's EF:63262-63264. Fireball (EF:63127), generic `sub_65820` (EF:62934), state-8 (EF:63477), lightning-II (EF:58762) all `CopyEntityPosition` the **raw** `MoveEntity` output with no ground clamp, then treat `getTerrainAlt > z` as a *detonation* trigger, not a clamp-and-continue.

**Reinterpreting the earlier "terrain-skim is shared retail flight law (EF:62947-53)" reading.** Those lines are:
```c
v6 = getTerrainAlt_10C40(&a1x->position_0x4C_76);                                 // EF:62947
v7 = a1x->position_0x4C_76.z;                                                     // EF:62948
predictedAxis_EB398ar.z = v6;
if (v6 > v7 || isCaveLevel && (predicted.z = sub_10C60(&pos)-box.fov, pos.z > predicted.z)) // EF:62950-62952
{ a1x->position_0x4C_76.z = predictedAxis_EB398ar.z; …                            // EF:62954  clamp
```
This is the `sub_65820` **post-move terrain-CONTACT** test, not a skim. `a1x->position.z = predicted.z` (EF:62954) clamps the burst location, and the very next non-water step is `goto LABEL_28 → v14 = 1` (detonate, EF:62964/62970). The earlier trace mistook this contact-clamp for a "follow the ground" clamp and, worse, generalized it to the fireball. **Correct reading: EF:62947-62964 is a detonate-on-contact law shared by the generic/fireball/lightning states; it is NOT a skim.** The genuine skim is exclusively possession's PRE-move clamp (EF:63262-63264), which no other player-castable state has.

**Consequence for our port (the (a) bug):** whatever code terrain-clamps the fireball z and lets it continue is applying possession's pre-move law to the fireball. The faithful fireball must (i) commit the raw ballistic z with no pre-clamp, and (ii) on `getTerrainAlt(pos) > pos.z` detonate (`_4A190(byte43,byte44)` + despawn), splashing `(10,5)` only if the contact tile is water. Reserve the pre-move ground-raise for possession alone.

---

## OPEN / uncertain
- Splash sound: `(10,5)` self-sounds; splash id ≈ 27 seen at EF:26693 (a sibling spawner) — not confirmed emitted by `(10,5)` itself. APPROX.
- `sub_10780` (offensive victim probe, EF:3739) ray-marches along pitch and can itself register a terrain-blocked cell; its interaction with the altitude test is not re-derived here (it returns a victim or null; terrain handling is the block above). Not load-bearing for the terrain/water laws.
- Fireball charged variant flies under action state 29 (subtype 0x1C sets actionIndex=29) — outside 0x00–0x0C; its terrain law not re-verified here (suspected same detonate-on-contact as `sub_65C20`; CONFIRM if state 29 is ported).
- Lightning beam state 0x09 (`sub_66750`) is a hitscan trail, not a flight loop; its terrain handling (path trace via `sub_66610` until `byte[1]&4`) is in the class-9 trace §0x09 and out of scope for the per-tick descent law.
