# MC2 Cave Ceiling as Gameplay — verbatim sim trace

Scope: every runtime read/write of the **second heightmap**
(`x_BYTE_14B4E0_second_heightmap`, the ceiling plane) and every
`isCaveLevel_D41B6`-gated **sim** behaviour, so the port can wire its
deferred "Phase 4.5" cave arms. Roster spawn-gating is covered by a
sibling agent; this report covers **behaviour** (movement, collision,
terrain mutation, LOS, castle).

Decompile citations: EF = `EventsFunctions.cpp`, TR = `Terrain.cpp`,
otherwise `file:line`. Root:
`reference/remc2/remc2/engine/`.

Sweep basis: `grep isCaveLevel` = 87 hits (71 in EF, 16 elsewhere);
`grep x_BYTE_14B4E0` = ~110 hits. Both fully bucketed below.

---

## 0. Headline findings

1. **The ceiling is a full second heightmap.** `x_BYTE_14B4E0_second_heightmap`
   is a `uint8_t[65536]` (BasicTerrain.cpp:3, allocated engine_support.cpp:1084),
   sampled by **`sub_10C60`** (TR:2158) → `sub_B5D68` (TR:2164): the SAME
   bilinear-interp, `×32`-scaled sampler as the floor's `getTerrainAlt_10C40`,
   but reading the ceiling bytes. So "ceiling altitude" is in the same
   world-Z units as terrain altitude.

2. **`mapAngle_13B4E0[tile] & 8` (bit 3) = "SEALED tile"** — floor meets
   ceiling, no fly-through. Every ceiling write re-derives this bit with
   ONE invariant law (§1). In caves bit3 is a wall; in non-cave levels the
   same bit is repurposed (terraform "hole"/bit7 pairing, EF:27579).

3. **Three distinct collision margins against the ceiling** — pin these:
   - `sub_11E70` (TR:2151) = **poke test, margin 0**: `fov + terrainAlt + word_160_0xc > ceiling`.
   - `sub_11E20` (EF:4620) = **collision test, margin 384**: `terrainAlt + word_160_0xc + fov > ceiling − 384`.
   - Movers clamp z to **`ceiling − fov`** (the array_0x52_82.fov, the entity's own head clearance).
   - m0-bob (`sub_1F040`) bounces at **`ceiling − 256`** with velocity `−150`.
   - Player mover (`sub_5D530`) clamps at **`ceiling − 384`**.

4. **Player flight**: `sub_5D530` clamps (does NOT bounce, no damage) at
   `ceiling − 384` when already above floor+clearance; the walker
   `moveTest_5D0A0` runs a bit3/`sub_11E20` steer-and-refuse gate before
   committing any XY move (§4a, verbs.rs arm).

5. **Projectiles**: ballistic states DO test the ceiling — `ceiling − fov`.
   Two behaviours coexist: **detonate-on-ceiling** (identical to floor hit;
   EF:62951, 63136, 63281) and **glide-along-ceiling** (clamp only; EF:63265).
   A fireball reaching the ceiling detonates exactly as if it hit ground.

6. **Terrain mutators** (riser / flood / quake / dome / terraform / Cave-In)
   all eat the ceiling: raising the floor eases the ceiling DOWN toward it
   (`+64` clearance, clamp 254), toggling bit3 when they meet. Terraform
   primitive `sub_56F10` moves the ceiling by `−a3` (opposite the floor).

7. **Cave-In (spell index 25 / 0x19)** is a **cave-ONLY** spell (gated off
   outside caves in 4 places). It is an area creature-crush, not a terrain
   write (§5f).

8. **Castle in caves**: mana/pickup spheres and balloons WALK on the ceiling
   via `sub_60D50` (attach/detach on bit3 + `sub_11E70`); absorb radius is
   `1024` open → **`2048` in caves** (EF:61793); walkable-tile test
   `sub_11C80` fails on bit3 tiles; building placement flags `byte_2 & 4`
   footprints specially in caves.

9. **Boot**: level init runs `sub_43B40` (ceiling init: `ceiling = MapBasicHeight − floor`)
   + `sub_43BB0` (±3 fuzz) INSTEAD of the normal `sub_43D50` angle pass (TR:51).

10. **Render/UI**: caves force sky OFF and run a ceiling render pass
    (GameRenderNG/HD/Original), and the minimap has a cave variant
    (GameUI:2414/2725). Not sim; listed in §8.

---

## THE ONE INVARIANT: ceiling/bit3 maintenance law

Every terrain write in a cave re-runs this after changing the floor
(`mapHeightmap_11B4E0`) or ceiling. It appears **verbatim** at TR:1877-1913,
TR:2034-2042, EF:31061-31063, EF:31190-31192, EF:32531-32542, EF:41546-41554,
EF:41966-41983, EF:42018-42036, EF:42265-42304, EF:42401-42443, EF:27552-27562,
EF:27604-27614, EF:28650, and every riser stage:

```c
if (isCaveLevel_D41B6) {
    if (x_BYTE_14B4E0_second_heightmap[t] > mapHeightmap_11B4E0[t])
        mapAngle_13B4E0[t] &= 0xF7u;              // ceiling above floor  -> OPEN (clear bit3)
    else {
        x_BYTE_14B4E0_second_heightmap[t] = mapHeightmap_11B4E0[t] - 1; // clamp ceiling to floor-1
        mapAngle_13B4E0[t] |= 8u;                 // ceiling <= floor     -> SEALED (set bit3)
    }
}
```

Port note: any code that writes the floor in a cave MUST run this on the
touched tile(s). The non-cave twin just clears bit3 (`&= 0xF7`), TR:1916-1923,
EF:41578, EF:27547 (with the `|8` special).

---

## 1. Ceiling generation & maintenance (terrain_paint.rs, boot)

### 1a. Boot init — `sub_43B40` (TR:1157), called from level init TR:51-54
```c
locHeight = mapHeightmap_11B4E0[i]; if (locHeight > MapBasicHeight_D41B7) locHeight = MapBasicHeight_D41B7;
x_BYTE_14B4E0_second_heightmap[i] = MapBasicHeight_D41B7 - locHeight;   // mirror-fold floor about basic height
// then the INVARIANT (bit3 + floor-1 clamp)  TR:1169-1177
```
Ceiling starts as the floor mirror-folded about `MapBasicHeight_D41B7`
(`MapBasicHeight_D41B7` = TR:1166/1168, the cave's basic height). Then
`sub_43BB0` (TR:1545) roughens it: for each **open** (`!(bit3)`) tile add
`randSeed%7 − 3` fuzz (clamp 0..254), then re-run the invariant over the
whole map (TR:1563-1574).

Level-init call site (TR:45-56):
```c
sub_44580();  // set angle of terrain
if (isCaveLevel_D41B6) sub_43B40();   // ceiling init + fuzz  (replaces...)
else                   sub_43D50();   // ...the normal angle pass
```

### 1b. Retile / shade helpers (terrain_paint.rs :32531-32542, TR:1874/:2034)
- **`sub_462A0`** shade recompute (TR:1931): computes `tempShad = floor_dY + 32`
  banded to 28..40; **shade inversion** for non-Day maps at TR:2030-2033:
  `mapShading = 32 − tempShad + 32` (else `tempShad`); then the invariant
  at TR:2034-2042. The SAME shade-inversion + invariant twin is inline in
  the type painter at EF:31055-31063 and EF:31183-31192.
- **Type painter** (TR:1786-1926): after assigning terrain type, TR:1874
  `mapAngle |= 0x80`, then the cave branch (TR:1875-1913) runs the invariant
  over the tile's **2×2 quad** (self, +x, +x+y, +y); non-cave branch clears
  bit3 on the quad (TR:1914-1923).
- **`sub_48CB0`-family smoother** EF:32531-32542 (the retile arm named in
  terrain_paint.rs:37): floor = neighbour average, then invariant.
- **`sub_48D20`** EF:32548 = the **ceiling smoother twin** — averages
  neighbouring ceiling bytes whose |Δ| > a2, writes ceiling, then invariant
  (EF:32585-32599).
- Ceiling-indexed sampler twins: `sub_48EC0`/`sub_48EF0` (EF:32634/32640)
  = `sub_48E60`/`sub_48E90` but with `x_BYTE_14B4E0` as the heightmap arg
  (EF:32637/32643). Use these when a helper needs a ceiling sample.

---

## 2. PLAYER FLIGHT under the ceiling — `sub_5D530` (EF:59610)  (multipart.rs / flight)

The MC2 player/carpet mover. Floor + ceiling handling after `moveTest_5D0A0`
returns true (EF:59745-59770):
```c
locAlt = getTerrainAlt_10C40(&predictedAxis_EB398ar);
if (mobilizeCounter) predictedAxis.z -= 51;                 // freeze/settle
else if (predictedAxis.z > word_160_0xc + locAlt) predictedAxis.z += word_160_0xe;  // rise buoyancy
if (predictedAxis.z >= locAlt + word_160_0xc) {
    if (isCaveLevel_D41B6) {                                  // ==== CEILING CLAMP ====
        locIntTemp = sub_10C60(&predictedAxis_EB398ar);
        if (predictedAxis.z > locIntTemp - 384)
            predictedAxis.z = locIntTemp - 384;              // clamp to ceiling-384, NO bounce, NO damage
    }
} else {
    predictedAxis.z = word_160_0xc + locAlt;                 // floor clamp
}
```
**Verdict**: player is *clamped* (not bounced), no contact damage, at
`ceiling − 384`. Margin 384 (not fov) — the player keeps a fixed 384-unit
headroom. The floor side is a hard clamp to `floor + word_160_0xc`.

Cave ambient sound: EF:59800-59808 — in caves, ~5/131 chance/tick play a
random ambient (`rand%0x83 + 65`) for the owning player. Non-cave plays
water/flight loops instead. (Sound; listed here because it's in the mover.)

### 2b. m0 vertical bob — `sub_1F040` (EF:11233)  (multipart.rs m0_bob, :390)
```c
z += dword_0x10_16;  v1 = getTerrainAlt(&pos);  dword_0x10_16 -= 5;   // gravity
if (z >= v1 + 256) {
    if (isCaveLevel_D41B6) {
        result = sub_10C60(&pos);
        if (z > result - 256) dword_0x10_16 = -150;                    // CEILING BOUNCE at ceiling-256
    }
} else dword_0x10_16 = 150;                                            // FLOOR BOUNCE at floor+256
```
Pin: ceiling bounce is velocity **`−150`** when `z > ceiling − 256`. The
port's `m0_bob` has floor `+150`; add the cave arm: `if cave && z > ceil(x,y) − 256 { f26 = -150 }`.

---

## 3. PROJECTILES & the ceiling  (proj.rs, tail.rs)

Ballistic movers test BOTH floor and ceiling. Two laws:

### 3a. Detonate-on-ceiling (identical to floor hit)
`sub_10780`-collider movers, EF:62947-62964 and EF:63132-63149 and
EF:63279-63290:
```c
v6 = getTerrainAlt_10C40(&pos);
predictedAxis.z = v6;
if (v6 > z
    || isCaveLevel_D41B6 && (predictedAxis.z = sub_10C60(&pos) - fov, z > predictedAxis.z)) {
    a1x->position.z = predictedAxis.z;      // clamp to ceiling-fov
    ... goto LABEL_28; // v14=1  ->  impact / spawn IfSubtypeCallCreatingManaSphere, water=(10,5) splash
}
```
A fireball reaching `ceiling − fov` clamps its z and **detonates exactly as
if it hit terrain** (same `IfSubtypeCallCreatingManaSphere_4A190` impact,
same water-splash branch). No special ceiling FX.

### 3b. Glide-along-ceiling (clamp only, no detonate) — EF:63262-63270
```c
if (v3 > predictedAxis.z) predictedAxis.z = v3;              // floor clamp
if (isCaveLevel_D41B6) {
    v4 = sub_10C60(&predictedAxis) - fov;
    if (v4 < predictedAxis.z) predictedAxis.z = v4;          // ceiling clamp, then keep moving
}
```
Used by homing/tracking projectiles (they slide along the ceiling instead of
dying). Distinguish per-state which law applies (63b is the pre-move clamp
inside the same function whose post-move test 63a detonates).

### 3c. `sub_102D0` LOS/spawn-clear test (EF:3632) — cave ceiling blocks it
EF:3674-3683: after the floor test, in caves also return "blocked" if the
tile is bit3-sealed OR `sub_11E70` pokes the ceiling:
```c
if (isCaveLevel_D41B6) {
    if (mapAngle_13B4E0[tile] & 8) return 1;
    if (sub_11E70(a1x, &v16x)) return 1;
}
```

---

## 4. CREATURE ceiling arms  (mobs.rs, verbs.rs)

### 4a. Walker steer-and-commit gate — `moveTest_5D0A0` (EF:59429)  (verbs.rs :59566)
Runs after the XY move is proposed into `predictedAxis_EB398ar`. Non-cave:
returns after the water test (EF:59513-59514). Cave path (EF:59515-59606):
```c
axis_3d tempAxis = predictedAxis_EB398ar;
v8 = word_160_0xc + getTerrainAlt(&tempAxis) + fov;
v9 = sub_10C60(&tempAxis);                                    // ceiling
if (v8 < v9 - 576 && !(mapAngle_13B4E0[tile] & 8))
    ;                                                        // plenty of headroom & not sealed -> commit as-is
else {
    // search 6 steps, two candidate yaws (v43 = yaw-512 &7, v44 = yaw+2 &7), radius 16*(i+1)+...
    // for each candidate compute headroom v15/v17 = sub_10C60(cand) - getTerrainAlt(cand)
    // pick the side with MORE headroom whose tile is NOT bit3 AND !sub_11E20(cand):
    if (v15 > v17 && !(mapAngle[c1]&8) && !sub_11E20(a1x,&c1)) { commit c1; yaw turn -v23 }
    else if (v17 > v15 && !(mapAngle[c2]&8) && !sub_11E20(a1x,&c2)) { commit c2; yaw turn +v23 }
    else if (sub_11E20(a1x,&tempAxis)) result = false;       // both blocked -> REFUSE move
    else predictedAxis = tempAxis;
    // v23 = (17*i)/6 ; sign by which side ; yaw += v23 & 0x7ff  (EF:59578-59581)
}
if (result && mapAngle_13B4E0[final_tile] & 8) result = false; // final tile sealed -> REFUSE (EF:59594-59597)
if (!result) { predictedAxis = old pos; speed_0xc_12 = 0; SpellEnabled[3].word_0x2E_46 = 0; }
```
**What the walker tests before committing**: (1) headroom `< ceiling − 576`
AND target tile not bit3 → free commit; else (2) turn toward whichever
diagonal has more ceiling clearance and is neither bit3-sealed nor
`sub_11E20`-colliding (margin 384); else (3) refuse and zero speed. The
`sub_11E20` 384-margin and the bit3 wall are BOTH hard blockers.

### 4b. Fallback nudge — `sub_5DD50` (EF:59854), called when moveTest refuses
```c
if (water==256 || (isCaveLevel && mapAngle[tile]&8)) v3 = 1;          // stuck against seal
if (!v3 && isCaveLevel && byte_0x261_609 && sub_11E20(a1x,&pos)) v3 = 1; // still poking ceiling
if (v3) { byte_0x261_609 = 1; nudge forward 128; }                   // shove out of the wall
else byte_0x261_609 = 0;
```

### 4c. Generic creature-tick ceiling clamp (shared across roster movers)
Identical block in many movers — **clamp z to `ceiling − fov`**, some also
zero/reflect vertical velocity `word_0x2C_44`:
- `sub_265A0` EF:17111 — clamp + `word_0x2C_44 = 0`.
- `sub_30D50` EF:22752 — clamp only.
- `sub_33340` EF:24382 — clamp only (per-child).
- `sub_33C70` EF:24751 — clamp with `ceiling − word_0x2C_44`.
- `sub_3A8B0` EF:29872 — clamp only.
Pattern: `v = sub_10C60(&pos) − fov; if (z > v) z = v;`

### 4d. Creature bounce-off-ceiling (rolling / lobbed states)
EF:26192-26263 and EF:26479-26547 (two parallel state arms): if
`sub_11E70` pokes the ceiling, either restore old pos or sidestep-search a
yaw with clearance (`sub_11E70`==false), reset `word_0x2C_44 = -128`; then
clamp z to `ceiling − fov` and **reflect** velocity: `word_0x2C_44 = -abs(word_0x2C_44)`
(EF:26260, 26536). Distinct from the plain clamp in 4c because it reflects
(bounces down off the ceiling).

---

## 5. TERRAIN MUTATORS at runtime (riser.rs, flood.rs, morph.rs, spells)

### 5a. Riser (14,1) — `sub_59F60` (EF:41255) & pillar `sub_5B100` (EF:42530)
Deferred arms in riser.rs. After each floor edit the riser runs the
INVARIANT over the affected block. Verbatim arms (all = §0 invariant):
- EF:41535-41563 — 4×N block seal/clear (riser.rs:145).
- EF:41838-41857 — non-cave-only bit3 clear (riser.rs :280 vicinity).
- EF:41897-41916 — non-cave bit3 clear (second orientation).
- EF:41957-41983 — invariant after `mapHeightmap[..]++` ridge (riser.rs :280).
- EF:42012-42036 — invariant + non-cave twin (riser.rs :470).
- EF:42253-42304 — invariant over the four ridge lines.
- EF:42390-42443 — invariant, per-column (riser.rs :470 vicinity).
Pillar `sub_5B100` EF:42648-42764: eases the ceiling toward
`signLocKoef2 + ceiling` neighbour-blend (EF:42745-42761) then invariant
EF:42683/42764. The **non-cave** riser leaves bit3 cleared (holes), the cave
riser seals where floor overtakes ceiling — this is what makes a riser a
solid pillar in a cave.

### 5b. Flood / quake helpers  (flood.rs)
`sub_34540` (EF:25083), `sub_34910` (EF:25265), `sub_34C40` (EF:25419),
`sub_34EE0` (EF:25544) are the flood/quake terrain drivers. Ceiling writes:
- EF:25141 / EF:25251 — LOS/height sample uses **`(floor + ceiling)/2`**
  midpoint (a cave-aware terrain-Z sample for the wave shape).
- EF:25219-25232 — ease ceiling up to `v16`, clamp `> v18` → `v18 − 1`,
  set bit3 (`|8`, EF:25231).
- EF:25313-25323 — ease ceiling to `v7`, clamp, seal.
- EF:25504-25522 — ease ceiling `second − (second − v10)/life` toward
  target, then invariant.
- EF:25683-25710 — ease ceiling down `v37 − v21`, clamp `> v26 → v26−1`, seal.
`sub_11CB0` (EF:4557) is the flood/quake **spawn-clear** test: it scans the
class-3 castle list (`dword_38519`, EF:4571) and the model-45 list
(`dword_38527`, EF:4585) for overlaps within `pitch/roll + 2560`, then
checks an 8×8 tile block via `sub_11C80` (bit3-walkable). Matches the
`dword_38519`=castles / `dword_38527`=buildings finding in the m67 flood
trace.

### 5c. Dome / apocalypse-dome raise — `sub_31940` (EF:23193)  (morph.rs)
The named deferred arm (EF:23366-23387). Verbatim:
```c
sub_570F0(x, y, v14, 0, dist<=v34, 1);                       // raise floor
if (isCaveLevel_D41B6) {                                      // ==== ceiling caves in ====
    v43 += 64; if (v43 > 254) v43 = 254;                     // ceiling target = floor + 64 clearance
    v38 = x_BYTE_14B4E0_second_heightmap[t];
    if (v43 > v38) {                                         // ease ceiling DOWN toward target
        v15 = (v38 - v43) / life;
        x_BYTE_14B4E0_second_heightmap[t] = v38 - v15;
    }
}
if (isCaveLevel_D41B6) {                                      // then the INVARIANT
    if (second[t] > floor[t]) mapAngle[t] &= 0xF7;
    else                      mapAngle[t] |= 8;
}
```
The SAME `+64`/ease/invariant law appears in `sub_39040` EF:28602-28650
(dome-family, EF:28604 `v50 = v12 + 64 ... second = second − (second − v50)/life`)
and in `sub_396A0` EF:28959-28988 (`second -= (second − v15)/dword_0x10_16`).
Port: in caves the dome pushes the ceiling down to `floor + 64`, sealing when
they meet.

### 5d. Terraform brush — `sub_377F0` (EF:27466)  (terrain_paint.rs / verbs)
Raise/lower a block. Cave arms EF:27547-27562 (raise: clear/set bit3 via the
`< second` variant of the invariant, EF:27554) and EF:27604-27614 (positive
raise). Non-cave arm EF:27579-27582 repurposes bit3: `mapAngle |= 0x80; &= 0xF7`.

### 5e. Terraform primitive — `sub_56F10` (EF:39499)
Single-tile raise by `a3`. Cave arm EF:39534-39543:
```c
mapHeightmap[t] = clamp(floor + a3, 0, 200);
if (isCaveLevel_D41B6) {
    v6 = second[t] - a3;                                     // ceiling moves OPPOSITE the floor
    second[t] = (v6 >= 255) ? -1 : v6;
}
```
The core "cave terraform closes/opens the gap symmetrically" law: floor +a3,
ceiling −a3.

### 5f. Cave-In spell (index 25 / 0x19) — CAVE-ONLY, area creature crush
The spell is disabled outside caves in 4 places:
- EF:22470 `!isCaveLevel && spellIndex2 == 25` → grey out icon.
- EF:43883 `(SpellIndex||SpellEnabled) && (isCaveLevel || a3 != 25)` → castable.
- EF:48253 & PlayerInput.cpp:849 `!isCaveLevel && v7==25` → refuse cast.
Effect `sub_3A650` (EF:23637-.. wait, EF:29636): scans the footprint
(`2*byte_0x46_70` square) around a target, and for every entity that matches
class+model and passes the crush filter **`sub_3A7F0`** (EF:29701 —
class==5, excludes models {12..15,22,23,25..27 mid-anim}, StageVar2 not in
{13,14,16,17}, actionIndex != 232), sets `StageVar2 = 14` (collapse-death),
latches parent + `word_0x2E_46`, and re-points action (EF:29676-29687).
`sub_3A7F0` is also called at EF:54994. **It does NOT write the second
heightmap** — Cave-In crushes creatures, it does not literally lower the
ceiling. (OPEN-1: whether a separate ceiling-drop entity accompanies it.)

### 5g. Area fuzz / erosion — `sub_43C60` (EF:30953)
EF:30976-30997: `second[v6] += rand%7 − 3` (ceiling jitter), then clamp
`> v11 → v11 − 1` (EF:30991-30997). The runtime twin of the boot `sub_43BB0`.

### 5h. Warp/mound raise — `sub_34910`-adjacent (EF:24945-24985)
The mound spell: sets bit7 (`|0x80`) on a diagonal line (EF:24956/24981),
raises `mapHeightmap += 48` on non-type-8 tiles (EF:24971), calls
`sub_46180(tile, 8)` (retile, which runs the invariant). Cave sealing is
implicit via `sub_46180`.

---

## 6. CASTLE in caves (castle.rs)

### 6a. Balloon / pickup ceiling-walk — `sub_60D50` (EF:61872)  (castle.rs :402)
The named cave balloon-walking law. `byte[0] & 1` = "walking on ceiling":
```c
if (byte[0] & 1) {                                            // currently ceiling-walking
    if (!(mapAngle[tile]&8) && !sub_11E70(a2x,a1x)) { byte[0] &= ~1; transition=1; } // detach: open sky below
    actSpeed = 96;
} else {                                                     // flying
    if (mapAngle[tile]&8 || sub_11E70(a2x,a1x)) { transition=1; byte[0] |= 1; }      // attach: hit sealed/ceiling
    actSpeed = 48;
}
if (transition && !cooldown) { PlayEventSound(..,22); byte_0x46_70 = 32; }   // "clink" on transition
sub_580E0(a1x, getTerrainAlt(a1x), word_160_0xc, word_160_0xa, word_160_0xe); // floor-follow
if (!(byte[0]&1)) { v10 = sub_10C60(a1x) - fov; if (a1x->z > v10) a1x->z = v10; } // flying -> ceiling clamp
```
Speed 96 while ceiling-walking, 48 while flying. Sound 22 on attach/detach.
Called from the mana-sphere homing mover EF:61848-61850 (cave branch) —
non-cave branch does the plain `sub_580E0` floor-follow (EF:61854-61860).

### 6b. Absorb radius 1024 → 2048 in caves — EF:61793-61796  (castle.rs :729)
```c
if (isCaveLevel_D41B6) v4 = 2048; else v4 = 1024;
v5 = EuclideanDistXYZ(&pos, &sphere->pos);
if (v5 > v4) sphere &= ~0x40;                                 // out of range: clear "targeted"
else { sphere |= 0x40; sphere->word_0x96_150 = me; if (sub_106C0(...)) { absorb mana; kill sphere; } }
```
Cave castles vacuum spheres from twice as far.

### 6c. Walkway / walkable-tile test — `sub_11C80` (EF:4543)  (castle.rs sub_11C80 arm)
```c
result = 1; v2 = mapAngle_13B4E0[tile];
if (v2 < 0 || isCaveLevel_D41B6 && v2 & 8) result = 0;       // bit7 (impassable) OR (cave & bit3 sealed)
```
Bit 3 makes a tile non-walkable in caves — the walkway check refuses to
route the settler/walker over a sealed ceiling column. Also used by the
flood spawn-clear `sub_11CB0` (§5b).

### 6d. Building placement footprint flag — EF:27089, EF:27251
```c
if (isCaveLevel_D41B6 && !(str_D93C0_bldgprmbuffer[bldg].byte_2 & 4)) v29 = 1;
```
Buildings whose param `byte_2 & 4` is UNSET get flagged (`v29/v50 = 1`) in
caves — a placement/clearance restriction on the building footprint.
(OPEN-2: exact meaning of the flag downstream — needs the ghost-preview code.)

---

## 7. TELEPORT / WARP / SPAWN placement in caves

- **`sub_4A810`** (EF:33254, generic pickup/mound spawn): in caves calls
  `SetEntityShiftRot_49EA0(entity, 256, 768)` (EF:33426) — a placement
  shift/rotation the open version skips.
- **`sub_4F440`** (EF:35989): creates a class-14 model-2 entity ONLY in
  caves (EF:37400) — a cave terrain-author helper (the riser/ceiling seed).
- **`sub_58630`** (EF:40468, per-frame in caves via `UpdateEntities_57730`
  EF:40113): every 8 turns, `MoveEntity 2560` ahead of a random player,
  sample a 20×20 grid (step 11/11), and on the first **open** tile
  (`!mapTerrainType && !(mapAngle&8)`) spawn **`(10,86)`** (EF:40550) — the
  cave DRIP ambient. This is the (10,86) drip source.
- Cave-only ambient/effect factories (behaviour-gated; roster owns spawn):
  `sub_4FB20`→(10,0x51), `sub_4FB80`→(10,0x50), `sub_4FBE0`→(10,0x52),
  `sub_4FC30`→(10,83), `sub_4FCA0`→(10,0x54), `sub_4FCD0`→(10,0x55),
  `sub_4FD00`→class10 (EF:36341-36470). Each returns 0 outside caves.
- No warp-OUT z placement uses the ceiling directly (warp altitude derives
  from the floor); the movers' `ceiling − fov` clamp catches any spawn that
  lands too high.

---

## 8. Non-sim cave differences (render / UI / camera / sound) — one line each

- **Level init** TR:51 — cave runs `sub_43B40` (ceiling init) not `sub_43D50`.
- **Sky off** GameRenderNG:512, GameRenderHD:698, GameRenderOriginal:680 —
  cave fills the sky band with `keyColor1_D4B7C` instead of `DrawSky_40950`.
- **Ceiling render pass** GameRenderNG:526, HD:730, Original:704 — cave runs
  a second-heightmap raster pass ("cleaned screen", `21d3e3`).
- **Ceiling voxel column** GameRenderHD:820 — `inverse_alt_8 =
  (second[v36] << 15 >> 10) − posZ` (the ceiling column height for the HD
  raycaster). The `<<15>>10` = `×32` matches the sampler scale.
- **Minimap** GameUI:2414, GameUI:2725 — cave-variant map draw (`a10` branch).
- **Ambient sound** EF:59800-59808 — random cave ambient in the player mover
  (§2). `sub_60D50` plays sound 22 on ceiling attach/detach (§6a).
- **Persistence** Level.cpp:229/362, EF:38815/38867 — the second heightmap
  is saved/loaded (0x10000 bytes) alongside the floor. Port save format must
  include it for caves.
- **GL path** GameRenderGL.cpp:8 / .h:12 — `DrawWorld` takes `isCaveLevel`
  as a param; renderer-only.

---

## Constants table

| Constant | Value | Meaning | Cite |
|---|---|---|---|
| `x_BYTE_14B4E0` | `uint8[65536]` | second heightmap (ceiling) | BasicTerrain.cpp:3 |
| ceiling scale | `×32` (bilinear) | same units as floor | TR:2200/2220 |
| bit3 (`&8`) of mapAngle | sealed tile (floor≥ceiling) | fly/walk wall | §0 invariant |
| poke test margin | `0` | `sub_11E70` | TR:2154 |
| collision test margin | `384` | `sub_11E20` | EF:4627 |
| mover clamp margin | `fov` (per-entity) | `ceiling − fov` | EF:62952, 61924, 26257 |
| player clamp margin | `384` | `ceiling − 384` | EF:59762 |
| m0-bob ceiling | `ceiling − 256`, vel `−150` | bounce | EF:11247-11248 |
| m0-bob floor | `floor + 256`, vel `+150` | bounce | EF:11241-11253 |
| walker free-commit headroom | `< ceiling − 576` | skip steer search | EF:59521 |
| dome/flood ceiling clearance | `floor + 64` (clamp 254) | ease target | EF:23368, 28604 |
| terraform ceiling delta | `−a3` (floor `+a3`) | symmetric close | EF:39538 |
| absorb radius (open/cave) | `1024` / `2048` | sphere vacuum | EF:61793-61796 |
| ceiling-walk speed | `48` fly / `96` walk | balloon | EF:61896/61905 |
| ceiling init fold | `MapBasicHeight − floor` | boot | TR:1168 |
| ceiling fuzz | `±3` (rand%7−3) | roughen | TR:1555, EF:30976 |
| Cave-In spell index | `25` (0x19) | cave-only crush | EF:43883 |
| cave drip spawn | `(10,86)`, 8-turn | ambient | EF:40550 |

---

## OPEN items

- **OPEN-1**: Cave-In (spell 25) `sub_3A650`/`sub_3A7F0` crushes *creatures*
  (StageVar2=14) but writes no ceiling byte in the traced path. Whether a
  companion effect visibly drops the ceiling (or spawns falling-rock
  entities) is unconfirmed — need the spell-25 cast dispatch (the
  `sub_116A0`/`IfSubtypeCallCreatingManaSphere` chain that precedes
  `sub_3A650`). EF:29636/29701/54994.
- **OPEN-2**: Building-placement cave flag `!(byte_2 & 4) → v29/v50 = 1`
  (EF:27089/27251) — the downstream consumer of `v29`/`v50` (ghost preview
  colour? placement refusal?) wasn't traced to its use site. Needs the tail
  of `sub_36F30`-area placement.
- **OPEN-3** (machine-gen suspicion): the two parallel creature bounce arms
  EF:26479-26547 vs EF:26192-26263 are near-duplicates; the decompiler split
  one switch into two blocks with re-numbered temporaries (`v18` vs `v24`).
  Treat as ONE law (bounce = reflect `word_0x2C_44`, clamp `ceiling−fov`);
  verify against a live cave creature before trusting the split.
- **OPEN-4**: `sub_4F440` (EF:37400) creates a cave-only class-14 model-2 —
  presumed the ceiling/riser seed, but its tick was not traced here (riser
  agent territory). Flag for the (14,2) porter.
- **OPEN-5**: whether the player mover's `mobilizeCounter` settle (`z −= 51`,
  EF:59750) interacts with the ceiling clamp on the same tick (order:
  settle runs BEFORE the `>= floor+clearance` gate, so a settling player can
  still be ceiling-clamped) — logic is clear but untested in a cave.
