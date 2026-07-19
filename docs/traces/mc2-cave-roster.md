# MC2 Cave-Gated Content — Roster & Existence-Gate Verbatim Trace

All decompile citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`
(EF = `EventsFunctions.cpp`, EV = `Events.cpp`, TR = `Terrain.cpp`, LI = `LevelInit.cpp`,
Spells = `Spells.cpp`, PI = `PlayerInput.cpp`). EVERY claim carries a `file:line`. Trace date 2026-07-11.
Companions (read for architecture, not re-derived here): `mc2-class10-high-band.md` (the (10,80..86) sculptor band + drip ctor),
`mc2-terrain-author-painters.md` (the (10,80)/(10,81) cave-tunnel carver), `mc2-class5-m10-21-23-24.md` (the (5,24) ctor),
`mc2-class11-switches-class14.md` (class-14 architecture + the (14,2) action 7), `mc2-night-environment.md` §5.1 (the 50-slot dynamic-light system).

**Scope split:** this report owns EXISTENCE / ROSTER gates (ctors returning 0 off/on cave, GenerateEvents branches, runtime spawners).
Behavior-modifying gates (steering/ceiling clamps) are the sibling agent's; they are noted for existence only, never traced.

---

## Headline findings (read first)

1. **The cave gate is decided at load in `LevelInit_56C00` (LI:32-38):** `MapType == Cave` sets `isCaveLevel_D41B6 = 1`, `MapBasicHeight_D41B7 = levelData->byte_0x2FED3` (the cave-ceiling base height, TR:15/1166-1169), `transparency = 1`, sound bank 2, cursor 10. Every ctor gate below reads either `isCaveLevel_D41B6` or `D41A0_0.terrain_2FECE.MapType == MapType_t::Cave` (the same fact via two globals).

2. **Six ctors are HARD existence gates (return 0 on the wrong map type).** The complete list of roster gates:
   | (class,model) | ctor | gate | polarity |
   |---|---|---|---|
   | **(2,6)** cave bee | `sub_4AFE0` EF:33555 | `if (MapType != Cave) return 0` (EF:33561) | **cave-only** |
   | **(5,24)** cave brute | `sub_4CCF0` EF:34487 | `if (MapType != Cave) return 0` (EF:34490) | **cave-only** |
   | **(14,2)** cave pillar | `sub_516C0` EF:37397 | `if (isCaveLevel_D41B6){…}` else return 0 (EF:37400) | **cave-only** |
   | **(10,89)** cave-in effect | `sub_50A20` EF:37037 | `if (MapType != Cave) return 0` (EF:37040) | **cave-only** (runtime-only, never authored) |
   | **(2,7)** day/night bee | `sub_4B0F0` EF:33587 | `if (MapType == Cave) return 0` (EF:33590) | **cave-EXCLUDED** |
   | **(2,8)** day/night bee | `sub_4B120` EF:33601 | `if (MapType == Cave) return 0` (EF:33601) | **cave-EXCLUDED** |
   | **(5,2)** day creature | `sub_4B590` EF:33751 | `if (MapType != Day) return 0` (EF:33758) | **DAY-only** (absent in caves AND night) |
   | **(5,27)** multipart | `sub_4D000` EF:34591 | cave → `v16=1`, whole spawn skipped, returns 0 (EF:34608-34690) | **cave-EXCLUDED** |
   Plus the six sculptor/carver ctors already banked in `mc2-class10-high-band.md`/`mc2-terrain-author-painters.md`:
   (10,80/81) `sub_4FB80`/`sub_4FB20` and (10,82/83/84/85) all `if(!isCaveLevel) return 0` (EF:36355/36332/36378/36399/36448) — cave-only terrain sculptors.

3. **(10,89) IS the Cave-In spell's ground effect** — the loose end resolves. Spell 25 (cave-only, greyed off-cave at EF:22470/43883/48253, PI:849) casts a flying manifestation (9,30) `sub_4E210` EF:35288 (the exact "(30,31,384,21,60,211)" row), which on floor-OR-ceiling impact spawns (10,89) `sub_50A20`, whose tick `sub_311E0` (EF:22860) SLAMS THE CAVE CEILING (`x_BYTE_14B4E0_second_heightmap`) DOWN onto the floor (`mapHeightmap_11B4E0`), burying ground entities in terrain and flagging tiles `mapAngle |= 8` (collapsed/solid). **The terrain is the weapon — no direct HP damage is applied by the cave-in.**

4. **The (14,2) pillar** is a cave-only floor-to-ceiling COLUMN machine: on first tick it finds the two flanking cave walls, sets its base z to their midpoint, then each tick raises the floor and lowers the ceiling toward each other until they seal, setting the solid bit `mapAngle |= 8` — a real collision occupant. Sprite: NONE (pure terrain machine). Consumes THING `par1→orientation`, `par3→half-width`.

5. **Census (47 cave levels of 165):** cave-EXCLUSIVE authored roster = **(5,24) x604, (14,2) x244, (10,80..86) sculptor band, (11,40) x1**, plus decoration models (0,31/36/64/80/82/83/84/85). Cave-ABSENT authored roster (never on any cave level) = **(2,0) trees x3953, (5,1) x1491, (5,13) x945, (5,14) x240, (5,23) x224, (5,27) x43, (5,10) x41, (5,12) x67, (10,8/23/52/71)**, and class-11 switch models (11,14/15/29/30/38/39/43). The (2,6) bee is de-facto cave-only (24 cave levels; 1 stray Day authoring that never spawns because the ctor returns 0 off-cave).

6. **Dynamic lights (Night+Cave only):** 7 registration sites for `AddEvent2_847D0` (the 50-slot light system, banked `mc2-night-environment.md` §5.1). None are cave-EXCLUSIVE — they are fire/explosion effects that light up wherever they appear; caves just make them visible. Roster in §8.

---

## 1. (2,6) CAVE BEE — cave-only, ctor `sub_4AFE0` (EF:33555)

### 1.1 Constructor (cave gate + 4 RNG draws)
```c
type_entity_0x6E8E* sub_4AFE0(axis_3d* position)//22bfe0 //Spawn Creture Bee
{
	if (D41A0_0.terrain_2FECE.MapType != MapType_t::Cave)   // EF:33561 CAVE-ONLY
		return 0;
	v1x = NewEvent_4A050(); ...
	v1x->actionIndex_0x45_69 = 18;                          // action 18 (0x12)
	v1x->class_0x3F_63 = 2;  v1x->model_0x40_64 = 6;
	v2x->byte_0x38_56 = 1;                                  // damage-eligibility bit0
	v2x->maxLife_0x4 = v4 % 0x50u + 100;                   // life 100..179  (RNG #1)
	v6x.x += (rand & 0x3F) - 32;                           // ±32 X scatter  (RNG #2)
	v6x.y += (rand & 0x3F) - 32;                           // ±32 Y scatter  (RNG #3)
	SetHalfSpeedEntity_49DA0(v2x, (rand & 3) + 324);       // sprite 324..327 (RNG #4)
}
```
No behavior-table row set (`dword_0xA0_160x` untouched) → NO aggro/attack profile.

### 1.2 Live tick — action 18 → `sub_651B0` (EF:62548)
Dispatch: class-2 actions go through `str_D4C48ar[2] → x_DWORD_D4C52ar_str20[actionIndex]` (EF:2063, table EF:1162), switch at EV:610/EV:3170-3239. Action 18 (0x12) → 0x2461B0 = `sub_651B0`. **All bee actions present in the switch — no truncation.**

```c
void sub_651B0(a1x) {                                       // EF:62556
	v1 = a1x->str_0x5E_94.word_0x62_98;                    // pending-damage attacker id
	a1x->struct_byte_0xc_12_15.byte[2] |= 2u;              // "on ground/static" draw flag
	if (v1) {                                              // took damage this frame
		a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;
		if (a1x->life_0x8 < 0) {                          // DEATH
			a1x->actionIndex_0x45_69 = 19;                // -> settle/corpse state
			a1x->struct_byte_0xc_12_15.byte[0] &= 0xF7;   // clear solid/collidable bit3
			SetHalfSpeedEntity_49DA0(a1x, word_0x5A_90 + 4);  // death sprite 328..331
			IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 13);  // death puff (10,13)
		}
		a1x->str_0x5E_94.word_0x62_98 = 0;                // consume damage record
	}
	a1x->position_0x4C_76.z = getTerrainAlt_10C40(&pos);  // SNAP to floor every frame
	if (sub_104D0_terrain_tile_is_water(&pos) == 1)
		DisableEntityDrawing04_57F10(a1x);                // despawn over water
}
```

**Behavioral facts (the surprising part):** the cave bee is a **passive, ground-pinned, non-flying, non-attacking, SILENT** sprite.
- **Movement/flight AI: NONE** — no `MoveEntity`, no bob, no wander. Pinned to `getTerrainAlt` each frame.
- **Sight-aggro: NONE** — no target scan, no aim, no behavior row.
- **Sting/attack: NONE** — its handler never damages the player or spawns a projectile. It is only a damage *target*.
- **Damage-IN:** external. Projectile/spell collision writes `str_0x5E_94` when `(1<<a2) & byte_0x38_56` matches (EF:4258-4290). Class-2 **model-0 gets damage/10** (EF:4273 `class!=2 || model`), but **model-6 takes FULL damage** (model!=0).
- **Death:** action 19 (`sub_65240` EF:62582, snaps z + despawns over water — inert corpse), death sprite +4, one (10,13) death puff.
- **Sounds: NONE** anywhere in the class-2 bee family (sub_651B0/65240/652C0).

### 1.3 The surface siblings (2,7)/(2,8) — the FLYING bees the cave bee is a stripped variant of
Ctors `sub_4B0F0` (model 7, action 20, sprite 322, **`if(MapType==Cave) return 0`** EF:33590) and `sub_4B120` (model 8, action 21, sprite 323, EF:33601), both via shared `sub_4B150` (EF:33609): `maxLife = rand%0x7D0+400` (400..2399, far tankier), `word_0x2C_44=-128` (bob seed), `actSpeed=0`. Both actions route to `sub_652C0` (EF:62606) — the REAL flight AI: forward flight via `MoveEntity`, obstacle/altitude avoidance `sub_654B0` (EF:62704, 8-direction terrain sampling), vertical bob clamp `[-192,+192]`, random hop/yaw re-roll on landing, aim helpers `sub_655C0`/`sub_65610`. Same death path ((10,13) puff; water → (10,5) splash). **So the cave bee = the ground-only, silent, defenseless flavor; the surface wasp/hornet = the full flyer.**

---

## 2. (5,24) CAVE BRUTE — cave-only melee patroller, ctor `sub_4CCF0` (EF:34487)

Ctor gate: `if (D41A0_0.terrain_2FECE.MapType != MapType_t::Cave) return 0;` (EF:34490). Full ctor + state map banked in `mc2-class5-m10-21-23-24.md` (spawns at action 193 idle; states 192..199; behavior row str_D7BD6[102]; sprites 335/336; minSpeed 80/maxSpeed 24/maxLife 16000; melee dmg 1500 @ range 1536). **Correction to the banked note:** its ONLY sound is **snd 7** (chase, `sub_28570` EF:18682) — the "snd 59" attributed to (5,24) is a MISATTRIBUTION (snd 59 belongs to a different class-5 settler `sub_27C10`/`sub_27E00`).

### 2.1 Handlers (verbatim spines)
- **+0/192 PATROL `sub_28490` (EF:18636):** `sub_1BD90(a1x,192)`; if still 192, every 8 ticks (`!(byte_0x3E_62&7)`) roll `rand%3==0` → idle(193); else `sub_28690` (acquire). If primitive changed state → force flee(198). Then `sub_287B0`.
- **+1/193 IDLE `sub_28500` (EF:18658):** `sub_1BF90(a1x,192)`; mirror — roll back to patrol(192) or acquire; else flee(198); `sub_287B0`.
- **+2/194 CHASE-ATTACK `sub_28570` (EF:18680):** **snd 7** every tick; `sub_1C310(a1x,192,sub_1CF20)`; if it returns nonzero (hit landed) → flee(198); `sub_287B0`.
- **+3/195 `sub_285D0` (EF:18690):** `actionIndex=193; sub_1BD90(a1x,192)` (the friend-assist/collision-nudge state; NO sprite setter).
- **+4/196 `sub_285F0`:** `PreKillEntity_1C890(a1x,192)`. **+5/197 `sub_28610`:** `KillEntity_1C930`.
- **+6/198 FLEE `sub_28630` (EF:18709):** `sub_1C980(a1x,192)` + `sub_28690` (re-acquire mid-flee) + `sub_287B0`.
- **+7/199 WAKE `sub_28660` (EF:18717):** `sub_1D5D0(a1x,192)` + `sub_287B0`.

### 2.2 Shared primitives (port-grounding summaries)
- **`sub_1BD90` PATROL (EF:8945):** drains damage mailbox (`str_0x5E_94`), walks child-chain adopting min child life; **v2==1 took-damage-survived** → `word_0x96_150 = attacker` (retarget) + action `a2+6` (if `byte_160_0x20_32 & 8`) else `a2+2`; **v2==2 dead** → `a2+4`; **v2==0** → on cadence `!(byte_0x3E_62 % word_160_0x1a_26)` scan own-model peers within `word_160_0x1c_28`+FOV → `a2+3`. Always `sub_1EEE0` (move).
- **`sub_1BF90` IDLE (EF:9064):** same preamble; no-damage path brakes (`sub_1B8C0`) + random yaw jitter (`roll += ((rand&0xFF)+85)*sign; roll&=0x7ff`); if sight (`byte_0x39_57`) scans the **class-3 building/castle list `dword_38519`** within range+FOV → `word_0x96_150 = building` + action `a2+2` (CHASE). **This is the primary aggro seam: (5,24) aggros CASTLES/BUILDINGS, not the player.**
- **`sub_1C310` CHASE (EF:9240):** same preamble; brake, resolve `word_0x96_150` via `sub_1ED30` (drop to idle if gone); every 4 ticks aim + peer-avoidance; on cadence if dist ≥ range → idle, else call the passed callback `sub_1CF20`.
- **`sub_1C980` FLEE (EF:9572):** same preamble; aim AWAY (`tan2 + 1024° flip`) + peer-avoid; return to idle when target lost/out-of-range.
- **`sub_1D5D0` WAKE (EF:9977):** switch on `StageVar2_0x49_73` → the shared stage/leash sub-behaviors (patrol-node sampling etc.). Dormant for a plain cave patroller (spawns at 193, not 199).

### 2.3 Melee fire + retaliation
```c
signed int sub_1CF20(a1x, a2x) {                            // EF:9800
	if (sub_583F0_distance_3d(&a1x->pos, &a2x->pos) < 1536) {
		sub_11900(a1x, a2x, 0, a1x->subSpellIndex_0x2A_42);  // deal 1500 dmg to mailbox
		return 1;
	}
	return 0;
}
```
`sub_11900` (EF:4375) writes `dword_0x5E_94 += a4` and stamps `word_0x62_98 = attacker id`. **Retaliation** is the mailbox loop shared by all class-5 primitives: victim drains mailbox next tick, sets `word_0x96_150 = word_0x26_38` (target := attacker) and jumps to CHASE `a2+2`. No `isCaveLevel` branch in any (5,24) handler (gate is ctor-only). `sub_287B0` (EF:18778): state 192→sprite 336/speed 0, 194(chase)→minSpeed/sprite 335, 198(flee)→`2*maxSpeed` (double-speed flee)/sprite 335, else maxSpeed/sprite 335.

---

## 3. (14,2) CAVE PILLAR — cave-only floor-to-ceiling column, ctor `sub_516C0` (EF:37397)

Creator table: class-14 `str_x_DWORD_D4C52ar_0x2F22[7]` (EF:2082) maps model 2 → 0x2326C0 = `sub_516C0`.
```c
type_entity_0x6E8E* sub_516C0(axis_3d* position) {          // 2326c0
	if (isCaveLevel_D41B6) {                               // EF:37400 CAVE-ONLY (else return 0)
		event = NewEvent_4A050();
		event->actionIndex_0x45_69 = 7;                   // class-14 action 7 (sub_5B100)
		event->class_0x3F_63 = 0xE; event->model_0x40_64 = 2;
		event->struct_byte_0xc_12_15.byte[0] &= 0xF6u; event->byte[0] |= 1;
		event->life_0x8 = 0;                              // life 0 => case-0 measure on first tick
		event->subSpellIndex_0x2A_42 = 0;
		event->word_0x2C_44 = 0;                          // ORIENTATION (0=long Y, nonzero=long X)
		event->word_0x96_150 = 0;                         // HALF-WIDTH koef
		AddEventToMap_57D70(event, position);
	}
}
```
**No sprite** (no `SetEntityIndexAndRot`) — it is a pure terrain machine, not a drawn entity. No `byte_0x43/0x44`. No `maxLife`.

### 3.1 Action 7 → `sub_5B100` (EF:42529), the column carver
Extent: `locKoef2 = 2*word_0x96_150 + 4`; orientation `word_0x2C_44` sets which axis is `locKoef2` long vs 2 wide; growth rate `signLocKoef2 ≈ locKoef2/4`. Phases via `life_0x8`:
- **case 0 (measure):** default `life = 4` (removed). Scan −/+ along the long axis for flanking solid cells (`mapAngle & 8`); if none found → `sub_57F20` DESPAWN; else `position.z = (wall1+wall2)/2` and stamp footprint `mapAngle |= 0x80` (dirty).
- **case 1 (grow):** loop **snd 47**. Each tick RAISE `mapHeightmap_11B4E0` (floor) UP and LOWER `x_BYTE_14B4E0_second_heightmap` (ceiling) DOWN by `signLocKoef2` across the footprint. When ceiling ≤ floor at a cell → `mapAngle_13B4E0[cell] |= 8` (**solid collision bit**); when nothing more moves → `life = 3` (built, idle) + end loop.
- **case 2 (retract):** mirror — relax floor down + ceiling up toward the surrounding cave terrain; when settled → `life = 4` (removed).

**Builds:** a floor-to-ceiling solid column over a `koefX×koefY` footprint; writes BOTH heightmaps; registers as a solid occupant via `mapAngle |= 8`. Raising is externally triggered (like the (14,1) riser).

### 3.2 THING field consumption — `sub_4A310` case 0xE, model-2 arm (EF:33236-33241)
```c
if (v10 == 2) {                                            // class-14 model-2
	v2x->word_0x2C_44  = entity->par1_14;                  // par1 -> ORIENTATION
	v2x->word_0x96_150 = entity->par3_18;                  // par3 -> HALF-WIDTH koef
	sub_58DA0(entity, v3x); return;
}
```
Does NOT consume `word_10`/`par2`/`stageTag`. GenerateEvents pass (EV:226-233) — the ONLY `type==0x000e` case, bracketed by `ApplyEvents_498A0` settles: for each `DisId==-1` THING with class 14 subtype 2, `PrepareEvents_49540` case 0x0E (EV:305-320) spawns via `sub_516C0` and writes the same two fields (par1→word_0x2C_44, par3→word_0x96_150). Both spawn paths agree.

---

## 4. (10,86) AMBIENT DRIP — runtime spawner `sub_58630` (EF:40468)

Load-time ctor `sub_50960` (EF:37011, life 9, sprite `rand%3+332`) is banked in `mc2-class10-high-band.md`. The RUNTIME spawner:
```c
sub_60F00(); if (isCaveLevel_D41B6) sub_58630(); ...       // EF:40113-40114 cave-only, each frame
```
`sub_58630` (EF:40468) verbatim spine:
- **Cadence:** MP → `rand%NumberOfPlayers` picks a random player, NO throttle (attempted every frame). SP → `if (Turn_2BE0_11248 & 7) goto skip` — only fires when `(Turn & 7)==0`, i.e. **every 8th turn**. Chosen player must be active (`byte_0x006_2BE4_11236 != 0`).
- **Placement:** `v15 = player.pos; MoveEntity_57FA0(&v15, player.yaw, 0, 2560)` → **2560 units ahead** of facing. Cell = `(v15+128)>>8`. Window = 20×20 tiles centered on that cell (origin `cell-10`). OUTER loop = rows (y) step +1 from `(cy-10)+rand%20`; INNER = cols (x) step +11 from `(cx-10)+rand%20`. First tile with `!mapTerrainType_10B4E0[cell] && !(mapAngle_13B4E0[cell] & 8)` (empty, passable, not solid) → `IfSubtypeCallCreatingManaSphere_4A190(&center, 10, 86)` (EF:40549), then break.
- **RNG order (`D41A0_0.rand_0x8`, LCG 9377/9439):** MP draw #0 (player pick); draw #1 → `v17 = rand%20` (X jitter); draw #2 → row-start `rand%20`. **SP = 2 draws, MP = 3 draws** per attempt. Port must preserve this order for hash fidelity.
- **Lifetime:** spawned drip life 9 (ctor). Ctor also self-aborts if `!(sub_104A0(&pos) & 1)` (extra terrain guard).

---

## 5. (5,2) DAY-ONLY & (5,27) CAVE-EXCLUDED

### 5.1 (5,2) — DAY-only, ctor `sub_4B590` (EF:33751)
`if (D41A0_0.terrain_2FECE.MapType != MapType_t::Day) return 0;` (EF:33758). Sets class 5, model 2, action 17, minSpeed 64, maxLife 3000, subSpellIndex 200, behavior row str_D7BD6[73], sprite 238. **Gate polarity confirmed: DAY-only ⇒ absent in caves AND night.** Census: authored on 45 Day + 6 Night + 7 Cave levels, but the Night/Cave authored records NEVER spawn (ctor returns 0). Also confirmed by `mc2-night-environment.md` §6 (EF:33758-59).

### 5.2 (5,27) MULTIPART — cave-EXCLUDED, ctor `sub_4D000` (EF:34591)
```c
if (D41A0_0.terrain_2FECE.MapType == MapType_t::Cave) v16 = 1;   // EF:34608
else { if (sub_4A810_get_0x35plus() >= 51) { /* spawn head (5,27) action 0xD9
        + 5 x (5,27) action 0xE9 sub-parts + 9 x (5,27) action 234 leaves */ } }
if (!a1x || v16) sub_2AE80(a1x); else { sub_2AC50; sub_2AD40; sub_2AE30; }
if (v16) a1x = 0; return a1x;                                     // cave => whole spawn returns 0
```
On cave levels `v16=1` skips the entire multi-part construction and returns 0 — a class-5 model-27 multipart creature that does NOT exist in caves. (It also has a `get_0x35plus >= 51` mission-progress precondition off-cave.)

---

## 6. Cave-In SPELL (index 25) — full effect chain

### 6.1 Descriptor + gate
`SPELLS_BEGIN_BUFFER_str[25]` (Spells:103-106, the LAST entry): `byte_0 = 3` (3 charge levels); subSpellIndex 100/130/390; manaCost 11000/13000/26000; `life_0x1A` 0/1/2 (divisor); hintText 0x107/0x108/0x109. **No verbatim "Cave-In" string** — name inferred from the cave gate (Spells.h comment lists only subtypes 0..0x17). Cave-only: greyed/blocked at EF:22470, 43883, 48253, PI:849.

### 6.2 Cast chain (three entities)
1. **Emitter** (model 25, tick `sub_6CFA0` EF:58123, dispatch EV:3799). Each tick while `isCaveLevel && word_0x2E_46 > 0` (EF:58134) it calls `sub_6DCA0(...,0x19,...)`.
2. **Flying manifestation** `sub_6DCA0` a3==25 branch (EF:44204): `IfSubtypeCallCreatingManaSphere_4A190(a2x, 9, 30)` → **(9,30) ctor `sub_4E210` (EF:35288)** = action 31, class 9, model 30, actSpeed/minSpeed 384, maxLife 21, str_D7BD6[60], sprite 211 (the "(30,31,384,21,60,211)" row). Sets `byte_0x43=10, byte_0x44=89` (impact-spawn keys), `subSpellIndex = subSpellIndex_2 / life_0x1A`. Launched 4096 ahead, z=terrain. **Cast sound 15** (EF:44233). Tick `sub_67910`→`sub_65820` (EF:62882): flies, and **on cave levels also collides with the CEILING** (`isCaveLevel && z > sub_10C60(pos) - fov`, EF:62950). On impact (EF:62979): spawn `(byte_0x43=10, byte_0x44=89)` = (10,89), copy subSpellIndex, despawn.
   > Note: `sub_3A7F0` (EF:29701) is NOT on this path — it is an unrelated class-5 filter for a possession/grab spell. The prompt's "(30,31,…)" row is the (9,30) manifestation, not sub_3A7F0.
3. **Cave-in ground effect** (10,89) `sub_50A20` (EF:37037, `if (MapType != Cave) return 0`) → action 0x60, class 10, model 89, life 40. **Zero authored records anywhere** (census) — spawned ONLY by the spell impact.

### 6.3 The collapse — `sub_311E0` (EF:22860), action 0x60
Phased radial collapse (`byte_0x46_70` 0=init, 2=running, 3=done; ~40+ ticks; `word_0x2C_44` wave 227 start, +22/tick, thresholds 455/1024). Per-tick, 6 concentric rings with `sin_DB750` falloff, per affected tile:
- **Floor raises** (rubble): `if (mapHeightmap_11B4E0[ix] < v45) sub_570F0(ix, ..., v45, 0,1,1)` (EF:22995).
- **Ceiling drops** (the cave-in): `if (second_heightmap[ix] > v45) x_BYTE_14B4E0_second_heightmap[ix] = v45` (EF:22997).
- **Entity burial** (EF:23003-23037): walks `dword_38519`, filtered `!model_0x40_64` (ground creatures/wizards), within radius² ≤ 0x64000 → pushes floor UP + ceiling DOWN onto them (dome via `sub_7277A_radix_3d`).
- **Solid flag** (EF:23040): `if (second_heightmap[ix] > mapHeightmap[ix]) mapAngle &= 0xF7; else { second_heightmap[ix] = mapHeightmap[ix]; mapAngle |= 8; }` — sealed tile marked collapsed/blocked.
- **Debris** (EF:23061): ~73 rock particles `(10,13)` sprite 67 + dust `(10,14)` sprite 9.

**Damage model:** `sub_311E0` calls NO HP-decrement. The kill is geometric — ceiling meets floor, ground entities are entombed and tiles marked solid (`mapAngle | 8`); the crush-death happens in the shared terrain-collision path (`Terrain.cpp:2034` invariant + `sub_65820` ceiling test). **The terrain IS the weapon.** Arrays touched: `mapHeightmap_11B4E0` (floor↑), `x_BYTE_14B4E0_second_heightmap` (ceiling↓), `mapAngle_13B4E0` (bit3 collapsed).

---

## 7. Full `isCaveLevel`/`MapType==Cave` gate sweep (roster hits only)

Behavior-modifying hits (steering/clamps/rendering — sibling agent / presentation) are EXCLUDED. Roster/existence hits:

| site | (class,model) or system | effect | polarity |
|---|---|---|---|
| EF:33561 `sub_4AFE0` | (2,6) cave bee | ctor returns 0 off-cave | cave-only |
| EF:33590 `sub_4B0F0` | (2,7) day/night bee | ctor returns 0 in cave | cave-excluded |
| EF:33601 `sub_4B120` | (2,8) day/night bee | ctor returns 0 in cave | cave-excluded |
| EF:33758 `sub_4B590` | (5,2) day creature | ctor returns 0 off-Day | day-only |
| EF:34490 `sub_4CCF0` | (5,24) cave brute | ctor returns 0 off-cave | cave-only |
| EF:34608 `sub_4D000` | (5,27) multipart | cave → skip whole spawn, return 0 | cave-excluded |
| EF:37040 `sub_50A20` | (10,89) cave-in effect | ctor returns 0 off-cave | cave-only (runtime) |
| EF:37400 `sub_516C0` | (14,2) cave pillar | ctor gated on isCaveLevel | cave-only |
| EF:36332 `sub_4FB20` | (10,81) tunnel carver | ctor returns 0 off-cave | cave-only (sculptor) |
| EF:36355 `sub_4FB80` | (10,80) worker | ctor returns 0 off-cave | cave-only (sculptor) |
| EF:36378 `sub_4FBE0` | (10,82) box mesa | ctor returns 0 off-cave | cave-only (sculptor) |
| EF:36399 `sub_4FC30` | (10,83) dome | ctor returns 0 off-cave | cave-only (sculptor) |
| EF:36448 `sub_4FD00` | (10,84)/(10,85) pit/hill | ctor returns 0 off-cave | cave-only (sculptor) |
| EF:33329 `sub_...` | (3,0) player-castle base | behavior row 104 (cave) vs 66 (else) — NOT a spawn gate | (behavior) |
| EF:33426 `sub_4ABA0` | (3,3) | cave-specific `SetEntityShiftRot(256,768)` — NOT a spawn gate | (behavior) |
| EV:— | GenerateEvents | NO `isCaveLevel` branch in Events.cpp (grep empty) — cave roster is entirely ctor-gated | — |

`sub_58630` (EF:40113 gate, EF:40468 body) = the (10,86) drip runtime spawner (§4) — cave-only, but a spawner not a ctor gate.

---

## 8. Dynamic-light-registering entities (roster for Phase 4.9)

The 50-slot Night/Cave light system (`AddEvent2_847D0(event, radius, flickerSpan, byte1)`, EF:47172; banked `mc2-night-environment.md` §5.1). **7 registration sites — none cave-EXCLUSIVE** (all are fire/explosion effects; caves merely make the light visible). One line each:

| (class,model) | ctor | radius | flickerSpan | byte1 | EF |
|---|---|---|---|---|---|
| (9,0) fireball projectile | `SummonFireball_4D2E0` | 128 | 1 | 0 | EF:34746 |
| (9,9) | `sub_4D860` | 128 | 9 | 0 | EF:34959 |
| (10,0) muzzle/spawn puff (sprite 7) | `NewAdd0A00_4E320` | 128 | 7 | 1 | EF:35349 |
| (10,1) big explosion | `NewAdd0A01_4E3B0` | 128 | 7 | 1 | EF:35370 |
| (10,6) real standing fire | `NewAdd0A06_4E5F0` | 80 | 11 | 1 | EF:35477 |
| (9,x) multi-fireball child (sprite 340) | `sub_4F5C0`-body | 128 | 1 | 0 | EF:36079 |
| (10,23=0x17) (sprite 7) | `sub_4F5F0` | 128 | 9 | 0 | EF:36104 |

(`byte1 & 1` caps the added light to the source's remaining life, per §5.1. Radius feeds the 5×5 cell add/undo cycle.)

---

## 9. LEVEL-SIDE CENSUS — 47 cave levels of 165

Read read-only from `baked/mc2/*.mgcl` (`mgc_formats::mgcl::read`, header `map_type == Cave`). Instrument written+run at trace time (a throwaway example, since removed); reproduce with the standing `crates/mgc-sim/examples/tmp_cavecensus.rs` or the command in OPEN-2.

**47 cave levels:** gfx=0 (37): 003 005 007 011 014 015 020 023 030 032 033 073 074 082 085 087 094 095 097 106 107 111 114 115 117 125 131 135 137 142 143 144 146 147 155 157 164; gfx=1 (10): 055 066 067 077 105 113 116 123 127 132. (`gfx_type` = the cave tileset variant; `MapBasicHeight_D41B7` from `byte_0x2FED3` sets the ceiling base.)

### 9.1 (class,model) authored ONLY on cave levels (cave-EXCLUSIVE roster)
| (c,m) | total | note |
|---|---|---|
| **(5,24)** | 604 | cave brute (16 levels) — matches ctor gate |
| **(14,2)** | 244 | cave pillar (23 levels) — matches ctor gate |
| **(10,80)** | 3033 | tunnel-chain authoring |
| **(10,82)** | 333 | box-mesa sculptor |
| **(10,83)** | 1696 | dome sculptor |
| **(10,84)** | 1000 | pit sculptor |
| **(10,85)** | 854 | hill sculptor |
| **(10,86)** | 7 | hand-placed drips (1 level; rest spawned by sub_58630) |
| **(11,40)** | 1 | a cave-only switch model (1 level) |
| (0,31)(0,36)(0,64)(0,80)(0,82)(0,83)(0,84)(0,85) | 13/1/20/21/1/127/10/6 | cave decoration/scenery models |

### 9.2 (class,model) authored on cave levels but ctor-gated OFF (spawn to nothing)
- **(5,2)** on 7 cave levels (83 things) — `sub_4B590` returns 0 off-Day.
- **(2,7)** on 2 cave levels (18), **(2,8)** on 1 (11) — ctors return 0 in cave.
- **(2,6)** 1 stray Day authoring (1 thing) — `sub_4AFE0` returns 0 off-cave.
These authored records should be treated as no-ops on the wrong map type (faithful behavior).

### 9.3 (class,model) NEVER authored on any cave level (cave-ABSENT roster)
`(2,0)` trees x3953, `(5,1)` x1491, `(5,13)` x945, `(5,14)` x240, `(5,23)` x224, `(5,27)` x43, `(5,10)` x41, `(5,12)` x67, `(10,8)` x11, `(10,23)` x10, `(10,52)` x6, `(10,71)` x21, `(11,14)` x12, `(11,15)` x9, `(11,29)` x27, `(11,30)` x10, `(11,38)` x3, `(11,39)` x11, `(11,43)` x5, plus `(0,7/10/16/23/27/28/35/42/43/50/71)`. `(2,0) = AddTree_4AC40` (EF:33433) is the day/night TREE — no ctor gate, but data-absent from caves (caves author (2,6) bees instead of (2,0) trees).

### 9.4 Cave stage / class-11 switch machinery
Per-level switch counts (class-11 THINGs) range 0..46; markers = 0 on all cave levels (all cave THINGs are Entity-kind). The class-11 switch family (chain-fire, slot-watch, level-end objective) is generic — banked in `mc2-class11-switches-class14.md` — with NO cave-specific switch handler beyond the cave-only model (11,40) (1 record, behavior OPEN-4). No cave-specific mission/objective machinery found in GenerateEvents (Events.cpp has zero `isCaveLevel` branches). Cave levels use the same stage engine as day/night.

---

## Constants table (consolidated)

| item | value | source |
|---|---|---|
| cave gate set | `MapType==Cave → isCaveLevel_D41B6=1, MapBasicHeight=byte_0x2FED3` | LI:32-38 |
| (2,6) bee ctor | cave-only; action 18; life rand%0x50+100; sprite 324..327; ±32 scatter; 4 RNG | EF:33555 |
| (2,6) bee tick | passive ground-pinned; die→(10,13) puff+sprite+4; despawn over water; NO move/aggro/attack/sound | EF:62548 |
| (2,7)/(2,8) bees | cave-EXCLUDED; sprite 322/323; action 20/21→`sub_652C0` full flyer | EF:33587/33601 |
| (5,24) brute ctor | cave-only; action 193; minSpeed 80/maxSpeed 24/maxLife 16000; melee 1500@1536; row 102; sprite 335/336; snd 7 ONLY | EF:34487 |
| (5,24) aggro | scans class-3 building list `dword_38519` (aggros castles, not player) | EF:9167 |
| (14,2) pillar ctor | cave-only; action 7; life 0; par1→word_0x2C_44 orient, par3→word_0x96_150 half-width; NO sprite | EF:37397 |
| (14,2) pillar tick | measure walls → grow floor↑+ceiling↓ → seal `mapAngle|=8`; snd 47; life 3=built/4=removed | EF:42529 |
| (14,2) THING fields | par1→orientation, par3→half-width koef (`locKoef2 = 2·par3 + 4`) | EF:33236 |
| (14,2) GenerateEvents | pass 4, only `type==0x000e subtype 2`, ApplyEvents-bracketed | EV:226 |
| (10,86) drip spawner | cave-only, every 8th turn (SP) / random player (MP); 2560 ahead; 20×20 search; empty passable tile | EF:40468 |
| (10,86) RNG | SP 2 draws / MP 3 draws; order = [player],Xjitter,rowstart | EF:40525-40536 |
| (5,2) day creature | DAY-only (absent cave+night); action 17; row 73; sprite 238 | EF:33751 |
| (5,27) multipart | cave-EXCLUDED (v16=1 skips); head action 0xD9 + 5×0xE9 + 9×234 sub-parts | EF:34591 |
| Cave-In spell 25 | 3 levels; subSpell 100/130/390; mana 11000/13000/26000; cave-only | Spells:103 |
| Cave-In manifest | (9,30) `sub_4E210`; action 31; actSpeed 384; maxLife 21; row 60; sprite 211; snd 15 | EF:35288 |
| Cave-In ceiling hit | cave: `z > sub_10C60(pos) - fov` also triggers impact | EF:62950 |
| Cave-In effect | (10,89) `sub_50A20` cave-only; action 0x60; life 40; ZERO authored records | EF:37037 |
| Cave-In collapse | `sub_311E0`: ceiling↓ onto floor↑, bury `!model` entities, `mapAngle|=8`, 73 rock (10,13) + dust (10,14); NO HP damage | EF:22860 |
| light registration | `AddEvent2_847D0(event, radius, flickerSpan, byte1)`; Night/Cave only, <50 slots | EF:47172 |
| cave levels | 47 of 165 (37 gfx=0, 10 gfx=1) | census §9 |

---

## OPEN items

1. **(10,89) crush-death path.** `sub_311E0` applies NO HP decrement — buried mobs die via the shared terrain-collision invariant (`Terrain.cpp:2034`, `sub_65820` ceiling test). The exact life-zeroing line for a mob entombed by the collapse was not traced to a single citation. Confirm the crush mechanic when wiring Cave-In lethality.
2. **Level-census reproduction.** The full per-level roster was produced by a throwaway `crates/mgc-sim/examples/tmp_caveroster.rs` (removed after the run). Reproduce with: `cargo run --release --example tmp_cavecensus` (the standing instrument, though it only counts the cave band), or re-add a census example that iterates `baked/mc2/*.mgcl`, reads `mgc_formats::mgcl::read`, filters `header.map_type == MapType::Cave`, and tallies `(t.class, t.model)`. Figures in §9 are from that run (2026-07-11 baked set).
3. **(2,6) death-VFX & attacker channel.** `(10,13)` death puff and `(10,5)` water splash are class-10 spawns; their visuals live in those handlers. The cave bee's damage-eligibility is only `byte_0x38_56 = 1` (bit0) — which weapon channels set `a2==0` to reach it is an attacker property, not the bee's. Confirm which player weapons can hit ground bees.
4. **(11,40) cave-only switch.** 1 authored record on 1 cave level; its behavior (which class-5 model it watches / what it chain-fires) not traced. It is the only class-11 model in the cave-exclusive set — trace before booting that level's objectives.
5. **Sound/sprite asset ids.** Cave-In cast sound 15, drip sound `word_0x5A - 282`, pillar loop 47, brute chase 7 — all are `PrepareEventSound` slot indices; concrete WAV names are in the sound-index tables (bank 2 for caves, LI:37), not resolved here.
6. **Cave-In `word_0x2C_44` wave tuning** (227 start, +22/tick, 455/1024 thresholds) and the `sin_DB750` falloff are verbatim (EF:22926-23088) but their gameplay feel wants a recorded-gameplay check before pinning the collapse radius/speed.
7. **`sub_57F20` / `sub_104A0` / `sub_104D0`** (pillar despawn, drip terrain-qualifier, bee water-test) bodies not transcribed — standard terrain/entity helpers; pull when porting.
8. **(2,7)/(2,8)/(5,2) stray cave authorings.** These authored records exist on some cave levels but their ctors return 0 there. Confirm the importer keeps them (faithful) and the sim treats them as no-ops rather than erroring, so the census hash matches retail.
