# Class-5 Model-10 — DOOMSDAY PYRAMID — Verbatim Trace Report

All citations are to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, other files cited path:line).

## ⚠ CORRECTIONS (2026-07-15 re-verification pass — read BEFORE the sections below)

A dedicated decompile re-verification (Session C of the pedantic-review fix plan) overturned several readings in this file. The sections below are left as written; where they conflict with this list, THIS LIST WINS:

1. **bit0 (`subSpellIndex & 1`) is NOT "the death branch."** The `sub_21F60` trip forces the ATTACK chain (case 3 → 6 → 7 → 8; `sub_21850`'s bit0 path picks the case-7 hurl-away beam and re-arms bit1). Death starts ONLY via `life < 10` in case 3.
2. **§2.5 devour walks the wrong list in the old reading.** `dword_38531` = the **class-9 PROJECTILE chain** (builder EF:40010-17, no filters); the model tests are class-9 SUBTYPES {2,4,5,22,23,25,30}. The pyramid devours incoming spell projectiles (an anti-magic zone) — NOT creatures/wizards — and there is **no player-proximity trip**.
3. **"pyramid-vs-pyramid" is wrong:** the `model == 10` case is class-9 subtype 10 = the **castle-build projectile**; devouring it zeroes `SpellEnabled[2]` (the CASTLE spell's manifestation window). The extents branch exists only for subtype 10.
4. `SpellEnabled[8]` in the trip tail = **REBOUND** — the laser trips whenever the local player's Rebound is live (EF:13616-18).
5. **"Unkillable by damage" is wrong.** `sub_22190` floors life at 8, and case 3's `life < 10` check IS the damage-death trigger; the mailbox is read in states 2..0xB while life ≥ 10. Only states 0/1 and 0xC-0xF ignore damage (300000 HP at ≤300/tick ⇒ ≥1000 damage ticks to kill).
6. **§2.8 pick-table fixes:** `rand += setting_30` perturbs the LCG STATE after each roll, not the roll value. All three creature ranges 49-58/59-68/69 write f26=8/f38=3/f50=682 **BEFORE** the cap test (persisting on cap failure); 40-48 writes 8/8/256; roll-2 picks 8/9 are **f38=1, f26=5** (ONE shot — the old "count 5" conflated timer with repeat); pick 1 = f38=10/f26=10, pick 2 = 8/8.
7. **The population caps are the verbatim bucket-0 quirk:** `sub_223E0`'s counters for picks 4 (<12) and 6 (<28) count the class-5 **MODEL-0** bucket, not their own kinds; only pick 3 (m0 <4) and pick 5 (m25 <6, excluding actionIndex 200) count their own.
8. **§2.9 creature summons** skip the shared aim tail; `actionIndex` is written LAST over the creators' defaults (m0 1→7, m21 169→175, m25 201→207, m19 153→159); the aim bearing uses the ALREADY-decremented repeat; only the first f38 ticks of state 9 spawn.
9. **§6's "drags the player in" is wrong** — the case-7 beam HURLS AWAY (bearing pyramid→player applied outward, ramp 1024→10 at −80/tick; see the corrected §2.9 case 7).
10. **§2.10:** the skipped bucket 10 = the class-5 **model-10** bucket (the pyramid spares itself); bucket 27 = the model-27 branch heads.

Bonus facts from the same pass: the setup cases 0/2/4/6/8/0xA/0xC fall THROUGH to their successors in the same tick (goto, no break); `word_0x36548` has NO reader anywhere in remc2 (savegame/debug only); case 0xF does NOT zero the doom meter (left at 1200); the kill-all arm zeroes `countStageVars_0x36E00` at countdown tick 70 (EF:12996-98); building type 68 (`byte_0x46_70 == 68`, `word_0x3654A`) reuses `sub_21F60` as a projectile-devouring structure every tick (EF:40181-83); `sub_56F10` carries a cave arm (EF:39534-43) that counter-shifts the SECOND heightmap by the delta (saturate 255, no low clamp).

**Headline finding (read first):** (5,10) is the campaign's **spell-of-extinction endgame device** — a ground-clamped, stationary boss "pyramid" (`sub_4BD00`, sprite 341) that runs a 16-phase scripted `byte_0x46_70` state machine (`sub_21030`). It is NOT a fighter. Its "doomsday attack" (`sub_21490`) literally **flattens the terrain in an expanding disc** (calls `sub_56F10(tile, -1, 0)` = lower heightmap to 0 over a growing circle rasterized by `AddE7EE0x_10080`/`sub_10130`), while spawning **falling mana rocks** (class-10 subtype 14) in a ring, and at climax it **`KillAllCreatures_1B5F0()` twice, sets every entity's life to 140 (a global reset), spawns the two doomsday mana spheres (10,17) and (10,9)**, sets the global `byte_0x36E03` extinction flag, and removes itself. Between phases it summons real creatures (`sub_4B240`/`sub_4C8F0`/`sub_4CE00`/`sub_4C6B0`) and fires class-9 projectiles at the player via `sub_21AB0`. It reads its own damage mailbox in `sub_22190` (immortal-clamped: `life` never falls below 8 until the script itself sets `life=-1`). **All 8 previously-OPEN helpers are now traced and closed** — see §2 and the OPEN-items list (empty).

The `byte_0x2FED2 & 2` spawn gate is a level palette flag (Night maps with bit1 load `PALF-0.DAT`, the doom palette, EF:31905) — i.e. the pyramid spawns only on the flagged "extinction" level(s).

---

## 0. Dispatch (how the pyramid is reached)

Class-5 binds its own creator/action tables in registry `str_D4C48ar` (EF:2060); the class-5 row's creator table holds `sub_4BD00` for subtype 10, and the action table is keyed by **actionIndex** (0x50..0x57 = 80..87). Per the class-5 model-21/23/24 trace (`docs/traces/mc2-class5-m10-21-23-24.md`), dispatch is by `actionIndex_0x45_69` through the table at EF:1290-1450, then Events.cpp switches on the IDA address (EV:620+); any address not in that switch hits `default → 157/0 → exit(1)`.

| state | actionIndex | function | EF | role |
|---|---|---|---|---|
| +0 | 80 (0x50) | `sub_21030` | 12654 | **the whole machine** (byte_0x46_70 sub-states) |
| +1 | 81 | `sub_22530` | 13844 | `actionIndex = 80` (reset) |
| +2 | 82 | `sub_22540` | 13854 | `actionIndex = 80` |
| +3 | 83 | `sub_22550` | 13864 | `actionIndex = 80` |
| +4 | 84 | `sub_22560` | 13874 | `PreKillEntity_1C890(a1x, 80)` |
| +5 | 85 | `sub_22580` | 13880 | `KillEntity_1C930(a1x)` |
| +6 | 86 | `sub_225A0` | 13886 | `actionIndex = 80` |
| +7 | 87 | `sub_225B0` | 13896 | `actionIndex = 80` |

All seven non-+0 handlers are trivial (verbatim in §5); the entire behavior lives in the +0 `byte_0x46_70` machine. Unlike m21/m23, **there is no missing/crashing +6 handler** for m10 — all eight addresses resolve.

---

## 1. Creator `sub_4BD00` (EF:33965-33996) — verbatim

```c
type_entity_0x6E8E* sub_4BD00(axis_3d* position)//22cd00
{
	type_entity_0x6E8E* v1x; // eax
	type_entity_0x6E8E* v2x; // ebx
	if (!(D41A0_0.terrain_2FECE.byte_0x2FED2 & 2))   // MAP GATE — doom-palette level only
		return 0;
	v1x = NewEvent_4A050();
	v2x = v1x;
	if (!v1x)
		return 0;
	v1x->actionIndex_0x45_69 = 80;                    // +0 state
	v1x->class_0x3F_63 = 5;
	v1x->model_0x40_64 = 10;
	v1x->maxLife_0x4 = 300000;                        // huge HP pool
	SetEvent144_49C70(v1x);
	v2x->struct_byte_0xc_12_15.dword |= 0x48800001u;  // OR (not assign) — static/marker flags
	v2x->subSpellIndex_0x2A_42 = 0;                   // the state-bit field the machine drives
	v2x->byte_0x38_56 = 1;
	v2x->dword_0xA0_160x = &str_D7BD6[107];           // behavior row 107
	v2x->byte_0x39_57 = 64;                           // wake-init / awake flag
	v2x->xtype_0x41_65 = 3;
	v2x->dword_0x10_16 = 0;                           // phase countdown
	v2x->byte_0x46_70 = 0;                            // internal state machine = 0
	v2x->byte_0x3E_62 = D41A0_0.array_0x10[10]++;     // per-model instance counter
	AddEventToMap_57D70(v2x, position);
	v2x->position_0x4C_76.z = getTerrainAlt_10C40(&v2x->position_0x4C_76);  // GROUND-CLAMPED
	CopyMaxLifeToLife_49A20(v2x);                     // life = 300000
	SetEntityIndex_49C90(v2x, 341);                   // sprite row 341
	v2x->array_0x52_82.yaw = 512;                     // extent yaw
	SetEntityShiftRot_49EA0(v2x, 1024, 1280);         // ShiftRot (1024,1280) — large extents
	return v2x;
}
```

**Creator facts:**
- **Map gate:** `byte_0x2FED2 & 2` must be set or NULL. This is a level palette/atmosphere bit (EF:31905 uses the same bit to pick `PALF-0.DAT`). Effectively: the pyramid only exists on the flagged extinction level(s).
- **No RNG draw in the ctor.** (First entity-local `rand_0x14_20` draw happens in the machine, §2 case 2.)
- **No speed fields** (minSpeed/maxSpeed/actSpeed) — stationary. **No mana field.** `subSpellIndex_0x2A_42 = 0` here is repurposed as the **phase bitfield** driven by the machine (bits 1/4/8/0x10/0x20/0x40/0x80), NOT a spell id.
- Flags OR'd: `0x48800001` = bits {0, 0x800000, 0x8000000, 0x40000000} — marker/static/no-cull bits (renderer + target-filter side; exact bit meanings name-inferred, see retail-check bank).
- Sprite 341; ShiftRot (1024,1280); extent yaw 512. Ground z via `getTerrainAlt_10C40`.

`NewEvent_4A050` defaults inherited (EV:561): `id_0x1A_26 = self index`, `xtype/xsubtype = -1` (xtype overwritten to 3), `byte_0x43_67 = 10`, `rand_0x14_20 = self index + D41A0_0.rand_0x8`, `struct_byte dword = 8` (then OR'd with 0x48800001).

---

## 2. The machine `sub_21030` (EF:12654-12880) — full body + phase table

### 2.1 Per-tick prologue (runs every tick, all states)
```c
v24 = 0;                                    // sound-flag (fires sound 63 at LABEL_48)
sub_223E0();                                // recount creature populations (§2.4)
v1x = <local player entity>;                // Entities_EA3E4[... playerIndex ...]
if (sub_21F60(a1x))                          // §2.5 — proximity-devour + spell-active test
	a1x->subSpellIndex_0x2A_42 |= 1u;        //   sets phase bit0 (forces the death branch)
v2 = a1x->byte_0x46_70;
if (v2 > 1 && (v2 < 0xC || v2 > 0xF) && a1x->life_0x8 >= 10)
	sub_22190(a1x);                          // §2.6 — read damage mailbox (immortal clamp)
switch (a1x->byte_0x46_70) { ... }
```

### 2.2 Phase table (`byte_0x46_70` sub-states) — verbatim semantics

`dword_0x10_16` = phase countdown; `word_0x2C_44` = turn-rate/altitude param fed to `sub_222B0`; `byte_0x44_68` = a facing-mode selector read by `sub_222B0`; `subSpellIndex_0x2A_42` = **phase bitfield**.

| state | does (verbatim) |
|---|---|
| **0** (init) | `D41A0_0.word_0x36548 = 1` (global "doomsday active" flag ON); →1; `subSpellIndex = 8` (set bit3 → arms the terrain-flatten attack in `sub_21490`); `dword_0x10_16 = 15`; `word_0x2C_44 = 22`; `word_0x96_150 = playerIndex` (target = player); `*(int16*)&x_BYTE_D9F50[0x87a] = 60` (HUD/UI meter init); `sub_22490(a1x)` (§2.7 clear 38×38 tile grid around pyramid); fall to LABEL_10. |
| **1** / LABEL_10 | `if (sub_21490(a1x))` (§2.3 — the attack; returns nonzero only when NO phase bit is set → "idle"): →4, `subSpellIndex |= 0x80`. Then LABEL_48. |
| **2** | **RNG draw #1** `rand = 9377*rand+9439`; `dword_0x10_16 = 26*life/maxLife - (rand & 7)` clamped [3,26]; →3; `byte_0x44_68 = 0`; `word_0x2C_44 = 22`; `sub_221F0(a1x, 341)` (sprite 341). LABEL_17. |
| **3** / LABEL_17 | if `life < 10` → **12** (begin death), LABEL_48. If `subSpellIndex & 1` → jump **6** (LABEL_26). Else `dword_0x10_16--`; if `EuclideanDistXYZ(player,self) < 0x2000` AND countdown ≤ 0: **RNG draw #2** `rand%0xC`, if `< 9` → **4**, else → **6**. LABEL_48. |
| **4** | →5; `dword_0x10_16 = 6`; `byte_0x44_68 = 2`; `word_0x2C_44 = 113`. LABEL_25. |
| **5** / LABEL_25 | `dword_0x10_16--`; if ≤0 → **6** (LABEL_26). LABEL_48. |
| **6** | →7; `dword_0x10_16 = 16`; `byte_0x44_68 = 0`; `word_0x2C_44 = 113`; `sub_221F0(a1x, 343)` (sprite 343). LABEL_28. |
| **7** / LABEL_28 | `dword_0x10_16--`; if ≤0 → **8**. LABEL_48. |
| **8** | →9; `dword_0x10_16 = 0`; `byte_0x44_68 = 3`; `word_0x2C_44 = 22`; `sub_221F0(a1x, 342)` (sprite 342); `sub_21850(a1x)` (§2.8 — pick which creature/projectile to summon). LABEL_31. |
| **9** / LABEL_31 | `sub_21AB0(a1x)` (§2.9 — execute the summon/fire); `dword_0x10_16--`; if ≤0 → **10**. LABEL_48. |
| **0xA** | →11; `dword_0x10_16 = 16`; `word_0x2C_44 = 22`; `sub_221F0(a1x, 344)` (sprite 344). LABEL_34. |
| **0xB** / LABEL_34 | `dword_0x10_16--`; if ≤0 → **2** (loops the 2→3→…→0xB charge/attack/summon cycle). LABEL_48. |
| **0xC** (death start) | →13; `dword_0x10_16 = 32`; **spawn doomsday sphere** `v13x = IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 17)`; if v13x: `z=0`, `maxLife=70`, `id = player.id`, `life=70`. LABEL_38. |
| **0xD** / LABEL_38 | `v24=1`; `dword_0x10_16--`; if ≤0 → **14**, `dword_0x10_16=32`, `sub_221F0(a1x,345)` (sprite 345). LABEL_48. |
| **0xE** | `v24=1`; **sound 10** `PrepareEventSound_6E450(idx,-1,10)`; `dword_0x10_16--`; if ≤0 → **15**, `dword_0x10_16=60`, `life = -1`, `byte[0] |= 1`, **`KillAllCreatures_1B5F0()`** (§2.10), then iterate ALL entities: `maxLife=140`, `byte[1] |= 0x20`, `life=140` (global life reset). LABEL_48. |
| **0xF** (final) | **`KillAllCreatures_1B5F0()`** again; `v24=1`; `dword_0x10_16--`; if ≤0: **spawn doomsday sphere** `IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 9)`, if ok: `life=32`, `maxLife=11`, `id=player.id`, **`D41A0_0.byte_0x36E03 = 1`** (global extinction flag); `word_0x36548 = 0` (doomsday-active OFF); `DisableEntityDrawing04_57F10(a1x)` (pyramid removes itself). LABEL_48. |
| default | LABEL_48. |

**LABEL_48 (shared tail):** `if (v24 && !(byte_0x3E_62 & 3)) PrepareEventSound_6E450(idx,-1,63)` (sound 63 on ~1/4 of ticks during death phases); then `sub_22270(a1x)` (§2.11 — ground-clamp z + turn toward player).

**RNG draws in the machine:** draw #1 (case 2, mask `&7`), draw #2 (case 3, `%0xC`). All entity-local `rand_0x14_20` LCG `9377*r + 9439`. (Helpers draw more — see each §.)

### 2.3 `sub_21490` — THE DOOMSDAY ATTACK + summon-ring (EF:12886-13094) — verbatim body

Returns `v26` (nonzero only when NO phase bit set → machine reads it as "idle, escalate"). `v1 = subSpellIndex` is the phase bitfield; the function branches on which bit is set:

- **`v1 & 8` (bit3, the terrain-flatten attack, armed in case 0):**
  ```c
  v25 = (pos.x + 128) >> 8;  v22 = (pos.y + 128) >> 8;   // center tile
  if (dword_0x10_16 < 0) {                                 // "expansion done" branch
      v28 = 1;                                             // scan a radius-7 disc for any non-flat terrain
      v4 = AddE7EE0x_10080(0, 7);                          // circle rasterizer radius 7
      while (sub_10130(v4,&dx,&dy)==1 && v28)
          if (mapTerrainType_10B4E0[tile(dx+v25, dy+v22)]) v28 = 0;
      ResetEvent08_10100(v4);
      if (v28) { subSpellIndex = (subSpellIndex | 4) & 0xF7; dword_0x10_16 = 70; }  // clear bit3, set bit2 (kill-all phase), 70-tick timer
      else       dword_0x10_16 = 15;                       // still terrain left → keep going
  } else {                                                 // ACTIVE FLATTEN
      PrepareEventSound_6E450(idx,-1,10);                  // sound 10
      v3 = AddE7EE0x_10080(0, 15 - dword_0x10_16);         // disc radius grows as countdown falls (0..15)
      while (sub_10130(v3,&dx,&dy)==1)
          sub_56F10(dx+v25, dy+v22, -1, 0);               // LOWER heightmap toward 0 on every tile in disc
      dword_0x10_16--;
  }
  ```
  → The pyramid **carves an ever-widening crater** to sea level. `sub_56F10(tx,ty,-1,0)` (EF:39499): `heightmap[tile] += -1` clamped [0,200], stamps angle/shading, cave second-heightmap decrement — i.e. sinks terrain by 1 unit/tile/tick over the disc. When countdown underflows past 0 (radius maxed), it verifies a radius-7 ring is fully flat, then flips to the kill-all bit.

- **`v1 & 4` (bit2, KILL-ALL countdown, `dword_0x10_16` from 70):**
  ```c
  KillAllCreatures_1B5F0();                                // every tick
  v7 = dword_0x10_16--;
  if (v7 == 70) D41A0_0.countStageVars_0x36E00 = 0;        // reset stage-var count once
  else if (v7 == 0x23) v29 = 1;                            // → set byte[3]|=1, clear byte[2] bit7 on ALL entities
  else if (v7 == 0x11) v29 = 2;                            // → set byte[3]|=0x80, clear byte[2] bit0 on ALL
  else if (v7 == 1)   v29 = 3;                             // → DisableEntityDrawing04 on ALL entities (wipe)
  else if (v7 == 0) { subSpellIndex = (…|0x10)&0xFB; dword_0x10_16=1; byte[0] &= 0xFE; }  // clear bit2, set bit4, clear byte0 bit0
  ```
  So the kill-all phase repeatedly annihilates creatures, then at three checkpoints (0x23/0x11/1) applies escalating byte-flag mutations to every entity (fade/vanish stages), and finally hands off to bit4.

- **`v1 & 0x10` (bit4, wind-down):** if `dword_0x10_16==1` → clear countdown, clear bit0x40. Else if `v1 & 0x40`: distance-gate on player — if `EuclideanDist(player) >= 0xA00` clear bit0x40; else `dword_0x10_16=30`, set bit0x20, clear byte[2] bit7, clear bit0x10 (arms the fade timer bit5).

- **`v1 & 0x20` (bit5, screen-meter ramp):** `if (dword_0x10_16>=600) v27=0` (suppress ring); `dword_0x10_16 += 30` clamped 1200 (then clear bit5); write `*(int16*)&x_BYTE_D9F50[0x87a] = dword_0x10_16` (drives the HUD doom meter 0..1200).

- **else (no bit set):** `v26 = 1` (return "idle" → machine case 1 escalates to state 4).

- **`v29` post-pass** (when set by bit2 checkpoints): iterate all entities and apply the flag mutation (1: `byte[3]|=1, byte[2]&=0x7F`; 2: `byte[3]|=0x80, byte[2]&=0xFE`; 3: `DisableEntityDrawing04`).

- **`v27` summon-ring post-pass** (default on, unless bit5 & countdown≥600): rotate `parentId_0x28_40 += 96 & 0x7FF`, then **4 iterations** at 90° apart (`+512 &0x7FF`): `MoveEntity_57FA0(&p, angle, 0, 192)` then `IfSubtypeCallCreatingManaSphere_4A190(&p, 10, 14)` — **spawns class-10 subtype-14 falling-rock particles** in a spinning ring; each gets **RNG draw** `life = (rand & 7) + 8`.

### 2.4 `sub_223E0` — population recount (EF:13779-13809) — verbatim
Sets four `D41A0_0` counters by walking entity buckets:
- `word_0x3653E` = count of `bytearray_38403x[0]` (a class bucket).
- `word_0x36540` = same bucket recount (identical loop — decompile artifact; both read bucket 0).
- `word_0x36544` = same bucket recount again.
- `word_0x36542` = count of `bytearray_38403x[25]` (=100/4) entries whose `actionIndex != 200`.

These four counters are read by `sub_21850` (§2.8) as summon-population caps (don't over-summon a creature class).

### 2.5 `sub_21F60` — proximity-devour + spell-active test (EF:13518-13620) — verbatim
Returns `v19` (→ sets phase bit0 in the prologue, which forces the machine into the death branch). Walks entity bucket `dword_38531`:
- `v17 = (class != 5 || model != 10)` — TRUE for the pyramid itself (it is class5/model10) → `v17 = 0`. (So the pyramid path takes the `!v17` branches.)
- For each entity of certain models (2, 4-5, 0x16-0x17, 0x19, 30 = creature/wizard models): if within `EuclideanDistXYZ <= 0xC00` → `v19 = 1`, mark `v18` (devour it).
- For low models (<0xA, the wizard/castle set): uses castle position via `dword_0xA4_164x->CastleEntityIndex` and `CompareAxisWithShift_106F0` extents (0xC00-ish box); on hit → `v19 = 1`, `v18`.
- `v18` (devour) side effects: if the devoured `model == 10` (another pyramid), zero out a spell slot (`SpellEnabled[2]`'s `word_0x2E_46 = 0`); spawn `IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 0)` (a mana absorb sphere, id=self), then `DisableEntityDrawing04(devoured)`.
- Finally: if the player's spell-slot `SpellEnabled[8]` entity has `word_0x2E_46 > 0` → `v19 = 1`.

So `sub_21F60` = "if the player (or a specific spell effect) is in the danger zone, or a devourable creature is adjacent, trip the death sequence and eat nearby creatures each tick." This is why the pyramid consumes the battlefield.

### 2.6 `sub_22190` — DAMAGE MAILBOX read (EF:13625-13659) — verbatim
```c
v1 = 0;
if (byte_0x39_57) {
    if (str_0x5E_94.word_0x62_98) {                        // attacker id present in mailbox
        v2 = str_0x5E_94.dword_0x5E_94;                    // queued damage
        if (v2 < 1) v2 = 1;  if (v2 > 300) v2 = 300;       // clamp 1..300 per tick
        life_0x8 -= v2;
        word_0x26_38 = str_0x5E_94.word_0x62_98;           // remember attacker
        str_0x5E_94.word_0x62_98 = 0;                      // clear mailbox
        v1 = 1;
    } else word_0x26_38 = 0;
}
if (life_0x8 < 10) { v1 = 2; life_0x8 = 8; }               // IMMORTAL CLAMP: life pinned to 8
return v1;
```
The mailbox is written by `sub_11900(attacker, victim, slot, amount)` (EF:4375): `str_0x5E_94.dword_0x5E_94 += amount; word_0x62_98 = attacker.id`. So the pyramid **does take damage** (≤300/tick), but `sub_22190` clamps `life` up to 8 whenever it would drop below 10 — **the pyramid cannot be killed by damage.** It only "dies" when the script sets `life=-1` (case 0xE) or when `sub_21F60`/case-3 low-life pushes it into the death branch. Note: `sub_22190` is only called for states 2..0xB (the active cycle), not during the death sequence (0xC-0xF) — during death the clamp is off so the scripted `life=-1` sticks.

### 2.7 `sub_22490` — clear the 38×38 tile footprint (EF:13814-13841) — verbatim
Runs once at case 0. Loops a 38×38 tile block centered at pyramid tile − (19,19): for each tile calls `sub_57390(tile, self.id)`. `sub_57390` (EF:39746) walks the map-cell entity chain and **disables every class-2 entity and applies death (`life=-1`) to select class-5 creature models** (6, <8, ≠10, 0x16-0x17, 0x19, 27) whose id ≠ self.id — i.e. **it wipes the ground area the pyramid stands on** (clears buildings/creatures under it) at activation. No RNG.

### 2.8 `sub_21850` — choose the summon (EF:13100-13265) — verbatim
Sets `byte_0x43_67` (which thing to summon), `word_0x24_36` (repeat count), `dword_0x10_16` (timer). Two RNG draws.
- Clears bit1 of `subSpellIndex`; if bit0 set → clear it, and if awake (`byte_0x39_57`) set `v3=1` (→ the "big attack" case 7). Else (bit0 clear):
  - set bit1; **RNG draw** `v4 = rand % 0x46` (0..69) `+= setting_30`; branch on `v4`:
    - 3..6 → `v3 = 1` (case-7 laser).
    - 40..48 → summon **case 6** (heavy, `sub_4C6B0`) if population `word_0x36544 < 28`; sets `word_0x24_36=8, dword_0x10_16=8, word_0x4A_74=256`.
    - 49..58 → **case 3** (`sub_4B240`) if `word_0x3653E < 4`; `word_0x24_36=3, word_0x4A_74=682`, `byte_0x43_67=3`.
    - 59..68 → **case 5** (`sub_4CE00`) if `word_0x36542 < 6`; `byte_0x43_67=5`.
    - >68 → **case 4** (`sub_4C8F0` = model-21 floating caster!) if `word_0x36540 < 12`; `byte_0x43_67=4`.
    - none matched → `v1=1` (fall to the projectile picker).
  - `v1` picker: **RNG draw** `v10 = rand % 0x1D` (0..28): ≤7 → `byte_0x43_67=1` (class-9 sub-0 bolt, count 10); 8..17 → `=2` (class-9 sub-9, count 8); 18..25 → `=9` (class-9 sub-3, count 5); 26..27 → `=8` (class-9 sub-26, count 5); >27 → `v3=1` (laser).
  - `v3` (laser/beam) → `byte_0x43_67=7, word_0x24_36=24, dword_0x10_16=32`.

So state 8 rolls a weighted table: sometimes summon a real creature (respecting per-class population caps from §2.4), sometimes queue a burst of class-9 projectiles, sometimes the sustained player-beam (case 7).

### 2.9 `sub_21AB0` — execute the summon / fire (EF:13270-13511) — verbatim
Runs each tick of state 9 while `word_0x24_36 > 0` (decrementing it). Gated on player alive & visible. Fires **sound `v34`** at the end if set. `byte_0x43_67` selects:
- **1:** spawn `(9,0)` projectile — `byte_0x43_67=10, byte_0x44_68=0, str_D7BD6[62], subSpellIndex=800`; sound 15. (id/aim set in the shared tail.)
- **2:** spawn `(9,9)` — `byte_0x44_68=23, subSpellIndex=800`; sound 23.
- **3-6:** the CREATURE SUMMON. Aim offset `v32 = (word_0x4A_74*word_0x24_36 + yaw) & 0x7FF`, `MoveEntity(1792)`. Spawn creature by case: 3→`sub_4B240`, 4→`sub_4C8F0` (model-21 caster), 5→`sub_4CE00`, 6→`sub_4C6B0`. If created: `str_0x364D2.dword_0x364D2++` (spawn tally); set `actionIndex = v30` (3→7, 4→175, 5→207, 6→159) and sound `v34` (3→8, 4→42, 5→37, 6→44). Bind `word_0x96_150 = playerIndex`, `id = self.id`, `StageVar2_0x49_73 = 17`, `word_0x2E_46 = 250`, `actSpeed = 320`, `parentId = self`, `yaw = roll = v32`.
- **7:** the PLAYER BEAM. If `subSpellIndex & 2`: `sub_5C800(player, 6)` (palette flash 6), `word_0x36546 = 1024`, sound 19, clear bit1. Then ramp `word_0x36546 -= 80` clamped [10,1024]; the angle is `tan2(pyramid → player)` applied OUTWARD — `MoveEntity(player.pos, angle, word_0x36546)` steps the player AWAY from the pyramid — and if `moveTest_5D0A0(player)` passes, commit (`CopyEntityPosition_57CF0`), floor-clamped to terrain + behavior-row height. → a HURL-AWAY beam that flings the player out (hard at first, 1024/tick, decaying to 10). [CORRECTED 2026-07-15, pedantic review §Trace-bank corrections 3: previously summarized as a tractor beam that "drags the player in" — the sign was misread; retail pushes the player AWAY. Port fix tracked as C3.]
- **8:** spawn `(9,26)` — `byte_0x44_68=22, subSpellIndex=20, byte_0x46_70=3`; sound 15.
- **9:** spawn `(9,3)` — `byte_0x44_68=17, subSpellIndex=6000, byte_0x46_70=10`; sound 15.
- **shared tail (v33x set = a projectile was made):** `id = self.id`, aim `yaw`/`pitch` at player (`sub_581E0_maybe_tan2`/`sub_58210_radix_tan`), `xsubtype = player.model`, `xtype = player.class`, `sub_5EF70(player)`.

### 2.10 `KillAllCreatures_1B5F0` (EF:8669-8693) — verbatim
Loops buckets 0..28. Skips bucket 10. Bucket 27: set each `actionIndex = 221`, `word_0x24_36 = playerIndex`. All other buckets: set each `life_0x8 = -1`, `word_0x24_36 = playerIndex`. → mass extinction of all creatures/wizards (bucket 10 = presumably the player/spell bucket is spared; bucket 27 gets a special teardown action).

### 2.11 `sub_22270` → `sub_222B0` — ground-clamp + face player (EF:13683-13774) — verbatim
`sub_22270`: `z = getTerrainAlt(pos)` (re-clamp to ground each tick); if `life >= 10` call `sub_222B0`.
`sub_222B0`: turn logic toward player. Computes relative-yaw bucket `v2 = (((yaw - player.yaw) >> 3) & 0xF0) >> 4`. If bucket ≤2 → `yaw = player.yaw + 384`; if 0xD..0xF → `yaw = player.yaw + 6`; else switch `byte_0x44_68` (facing mode): 0 → `roll = angle-to-player`; 1 → turn only; 2 → `roll = ±512 + player.yaw+…` (alternating by frame parity), set mode 1; 3 → `roll = yaw`. Then if turning: `yaw += sub_58350(yaw, roll, behaviorRow.word_0x4, word_0x2C_44)` (rate-limited turn). `yaw &= 0x7FF`. No RNG.

### 2.12 `sub_221F0` — set sprite + animation length (EF:13661-13679) — verbatim
`SetEntityIndex_49C90(a1x, a2)`; if `a2` in [0x157,0x159] (343-345), look up `particlesParameters_D951C[a2].word_0`, `sub_71AB0`, and set `dword_0x10_16 = animation frame count` — i.e. sprites 343/344/345 (the death-sequence anims) auto-size the phase timer to the animation length.

---

## 3. Damage path (summary)
- **Intake:** `sub_11900` (EF:4375) writes mailbox `str_0x5E_94` (`dword_0x5E_94 += amount`, `word_0x62_98 = attacker.id`); the pyramid reads it in `sub_22190` (§2.6), clamped ≤300/tick, and **`life` is pinned to ≥8** during the active cycle → **unkillable by damage.**
- **Death is scripted only:** case 3 (`life<10` after clamp release, but clamp is active in 2..0xB so this rarely trips) → 12; `sub_21F60` proximity → bit0 → death; case 0xE sets `life=-1`.
- **byte[0] targetability:** ctor OR's `0x48800001` (bit0 set); the machine clears byte[0] bit0 in `sub_21490` bit2-phase (`byte[0] &= 0xFE`) and sets `byte[0] |= 1` in case 0xE. Combined with the `0x08` (bit3 = list-target-eligible) state from NewEvent, the pyramid IS targetable while active (players can shoot it, damage is just clamped away).

## 4. Spawn paths (data)
- THING spawn: `sub_4A310` (EF:32999) tile-centers + `IfSubtypeCallCreatingManaSphere_4A190(&pos, 5, 10)` → `sub_4BD00`. Class-5 post-init in the `case 5:` arm of `sub_4A310` (not the class-0xA arm); model 10 consumes no par1/par2 beyond the ctor (no LABEL_49 field wiring for class 5). Disposition-deferred via `sub_4A1E0(DisId)` like all THINGs.
- **Gate:** ctor returns NULL unless `terrain_2FECE.byte_0x2FED2 & 2` — the doom-palette level bit. So even if a THING authors (5,10) on the wrong level, no pyramid spawns. This bit is a level-data flag; which campaign levels set it is data (retail-check).
- Which levels author (5,10): x41 records across 6 levels per the task brief; not code-determinable here (THING table data).

## 5. The seven thin handlers (EF:13844-13903) — verbatim
```c
sub_22530/22540/22550/225A0/225B0:  a1x->actionIndex_0x45_69 = 80; return a1x;   // reset to +0
sub_22560:  PreKillEntity_1C890(a1x, 80);
sub_22580:  KillEntity_1C930(a1x);
```
All resets funnel back into the +0 machine; +4/+5 are the standard prekill/kill primitives (targeting actionIndex 80).

## 6. Interactions / who kills it / special-cases of (5,10) elsewhere
- **Targets:** the local player (`playerIndex`), and — via `sub_21F60`/`sub_22490` — every nearby creature/wizard/building (devours or wipes them). Via `sub_21490` bit1/bit2 phases it flattens **terrain** and mass-kills **all creatures**. Via `sub_21AB0` case 7 it HURLS the **player** AWAY (the "tractor beam" reading was a misread — corrected per §Trace-bank corrections 3; the corrected passages above have the hurl law).
- **What can kill it:** nothing via damage (immortal clamp). It self-terminates through the scripted death sequence (states 0xC-0xF) triggered by player proximity (`sub_21F60`) or a low-life release. `KillAllCreatures_1B5F0` skips it (it is class 5 but the machine controls its own life).
- **Special-cases of model 10 elsewhere:**
  - EF:13542 `sub_21F60`: `v17 = class!=5 || model!=10` — the pyramid identifies itself.
  - EF:13608 `sub_21F60`: devouring another `model==10` zeroes a spell slot (pyramid-vs-pyramid).
  - EF:39779 `sub_57390`: model 10 is EXCLUDED from the footprint-wipe (the pyramid doesn't wipe itself).
  - EF:58205 `CopyAxisForSpellWithLife_6D830`: for a spawned **(class-10) model-10** mana entity (NOT this pyramid — different class), copies parent axis. (Class-10 model-10 is unrelated; guard is model-only, harmless collision of model numbers.)
- **Downstream doomsday spheres:** the (10,17) [case 0xC] and (10,9) [case 0xF] spheres run action-handler around EF:23392-23430; when `byte_0x36E03` (the extinction flag the pyramid set) is on, they spawn `(10,91)` instead of `(10,18)` and play sound 63 — the "extinction accomplished" terminal effect + a heightmap stamp at `life==3`. `byte_0x36E03` is reset per-level at EF:35527.
- **Global flags the pyramid drives:** `word_0x36548` (doomsday active, 1 in case 0 → 0 in case 0xF), `byte_0x36E03` (extinction done, case 0xF), `countStageVars_0x36E00` (reset in `sub_21490` bit2), `x_BYTE_D9F50[0x87a]` (HUD doom meter, 0..1200).

---

## 7. Sounds (consolidated)
| sound | where | cadence |
|---|---|---|
| 10 | case 0xE (EF:12837); `sub_21490` active-flatten (EF:12955) | every tick of those phases |
| 63 | LABEL_48 when `v24 && !(byte_0x3E_62 & 3)` (EF:12751) | ~1/4 ticks during death phases |
| 15 | `sub_21AB0` cases 1/8/9 (`v34=15`) | on projectile spawn |
| 23 | `sub_21AB0` case 2 | on spawn |
| 8/42/37/44 | `sub_21AB0` cases 3/4/5/6 | on creature summon |
| 19 | `sub_21AB0` case 7 first fire | once when beam starts |

---

## 8. Porting notes (field map + draw order + math)

**Ctor field writes (order):** map-gate `byte_0x2FED2&2`; actionIndex=80; class=5; model=10; maxLife=300000; SetEvent144; `byte[0..3] dword |= 0x48800001`; subSpellIndex=0 (phase bitfield, NOT a spell); byte_0x38=1; behaviorRow=&str_D7BD6[107]; byte_0x39=64; xtype=3; dword_0x10_16=0; byte_0x46_70=0; instanceCounter; AddEventToMap; z=terrainAlt; life=300000; sprite=341; extent.yaw=512; ShiftRot(1024,1280). **No RNG in ctor.**

**State field semantics:**
- `byte_0x46_70` = 16-state machine cursor (0..0xF).
- `subSpellIndex_0x2A_42` = phase bitfield: bit0(1)=death-trigger, bit2(4)=kill-all phase, bit3(8)=terrain-flatten armed, bit4(0x10)=wind-down, bit5(0x20)=meter-ramp, bit6(0x40)=player-near gate, bit7(0x80)=set on entering state 4.
- `dword_0x10_16` = phase countdown / attack radius seed.
- `word_0x2C_44` = turn-rate fed to `sub_222B0`.
- `byte_0x44_68` = facing mode (0=aim, 1=hold, 2=alternate, 3=lock).
- `byte_0x43_67` = summon selector (set in `sub_21850`, consumed in `sub_21AB0`).
- `word_0x24_36` = summon repeat counter; `word_0x4A_74` = summon aim-spread stride; `word_0x96_150` = target (player).

**RNG draw order (entity-local `rand=9377*rand+9439`):** none in ctor. Per full cycle: case 2 (`&7`), case 3 (`%0xC`) → `sub_21850` (`%0x46`, then `%0x1D`) → `sub_21AB0` (none) → `sub_21490` summon-ring (one `&7` per of 4 rocks) → `sub_21850` again next cycle. Global `D41A0_0.rand_0x8` not used by the machine directly (spawned spheres/particles use it internally).

**The attack math (bit3 flatten):** disc radius = `15 - dword_0x10_16` (grows 0→15 as timer falls), rasterized by `AddE7EE0x_10080(0, radius)` + `sub_10130` (precomputed circle bitmap `bitmaps_E9980x[radius]` yields (dx,dy) tile offsets); each tile `sub_56F10(cx+dx, cy+dy, -1, 0)` sinks heightmap by 1 (clamp [0,200]). When `dword_0x10_16 < 0`, scan radius-7 disc; if all `mapTerrainType==0` (flat) → switch to bit2 kill-all (timer 70). Center tile = `(pos.{x,y}+128)>>8`.

**Contract:** MC2 ctor sets the cross-column damage `f28=1` contract per project convention (mirror the `struct_byte` init). The pyramid is a stationary, non-flying, ground-clamped structure — no move/steer beyond `sub_222B0` yaw-turn; z re-clamped to terrain every tick in `sub_22270`.

**Behavior row:** str_D7BD6[107] (0x1c_28 range / 0x1e_30 FOV cone / 0x1a_26 cadence / 0x4_4 turn-rate / 0xc_12 height). **Sprites:** 341 (idle/charge), 342/343/344 (cycle anims), 345 + 342-344 (death anims); 341 also re-set in case 2.

---

## 9. The summoned creatures' RELEASE CHAIN (added 2026-07-19, playtest item 11)

The §2.9 summons spawn with `StageVar2 = 17` and their `base+7`
action — and every class-5 `+7` wrapper simply calls
`sub_1D5D0(self, base)` (EF:9977), the held/StageVar2 dispatcher.
Its cases 16 (0x10) and 17 (0x11) are the pyramid-summon states:

- **17 → `sub_1E320` (EF:10566), spin-up:** `sub_1B8C0` intake+move
  (life < 0 → immediate despawn, no puff); aim `roll` at
  `word_0x96_150` (the player); `actSpeed -= 8` from the summon's
  320; at ≤ 16 set the per-model cruise (m0 → 30, m19 → 76,
  m21 → 96, m25 unchanged, EF:10588-601) and `StageVar2 = 16`.
  Invalid target → straight to 16 (speed untouched).
- **16 → `sub_1E580` (EF:10689), home:** the Summon-Army twin
  WITHOUT the case-13 life decrement (EF:10703-06) — summons persist
  while the pyramid lives; parent death zeroes `word_0x2E_46` →
  expire with the (10,0) puff (the (10,73) jar is case-13-only).
  The `sub_1E700` core: intake (a KILL sets `word_0x2E_46 = 1` —
  the corpse stands until parent death, EF:10864-67); on a hit,
  retaliate at the attacker unless parent or same-species (flee
  rows → `+6`, else `+2`, parent-XP `sub_6D8B0(parent,0x13,1)`);
  quiet path: 8-tick aim (unless `byte[2]&4`), 64-tick jink,
  same-model different-id crowd steer-away within `array.pitch`
  (EF:10829-38 — the anti-pile-up). Engage: 8-tick throttle,
  `dist3d < row.word_0x1c_28` → `actionIndex = base + 2`
  (StageVar2 stays 16).

Port: `mc2_doom_summon_spinup_tick` / `mc2_doom_summon_home_tick`
(mc2/mobs.rs), dispatched from the `+7` site_z match; parent link is
scan-resolved (the level authors exactly one (5,10);
`parentId_0x28_40` has no port home). Before 2026-07-19 these two
cases were unported — summons froze unkillable at the `_ => {}` arm.
Wrapper-tail residues accepted as APPROX: m19's `action==154 →
byte_0x46_70 = 0` handoff reset (moot for fresh summons) and m21's
`sub_268F0` anim tail.

## OPEN items
None. All eight previously-OPEN helpers (`sub_21490`, `sub_21850`, `sub_21AB0`, `sub_22190`, `sub_22270`, `sub_22490`, `sub_223E0`, `sub_21F60`) are traced verbatim above. Minor name-inferred (not blocking a port): the exact renderer meaning of the `0x48800001` flag bits and `x_BYTE_D9F50[0x87a]` (HUD doom-meter address); the downstream (10,17)/(10,9) sphere handler (EF:23392) is sketched (own trace if the spheres are ported separately).

## Retail-check bank
1. Which campaign levels set `terrain_2FECE.byte_0x2FED2 & 2` (the doom-palette gate) — those are the only levels where a (5,10) THING actually spawns. Cross-check against the x41-records/6-levels count from the survey.
2. Terrain-flatten disc: confirm retail shows the pyramid **sinking terrain to sea level in a growing circle** (crater) with sound 10 looping — not raising or exploding.
3. Climax: confirm **all creatures die twice + a global life=140 reset + two mana spheres (10,17 then 10,9) + the pyramid vanishing** (sound 63, screen palette flash via `sub_5C800`/case-7 flash 6).
4. Player hurl-away beam (case 7 / `sub_21AB0`): the pyramid **flings the player AWAY from it** (`word_0x36546` ramp 1024→10, `moveTest_5D0A0` push) with palette flash and sound 19. [Corrected 2026-07-15 — was "drags the player toward it"; see §2.9 case 7.]
5. Summon weights (`sub_21850`): confirm the pyramid **spawns real creatures** (including model-21 casters via `sub_4C8F0`) and class-9 projectile bursts between charge phases, respecting the population caps counted in `sub_223E0`.
6. Damage: confirm the pyramid is **effectively unkillable** by player weapons during its run (life clamp to 8) and only ends via its own script/proximity.
