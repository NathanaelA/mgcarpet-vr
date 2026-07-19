# CLASS-10 Models 59 (0x3B) & 60 (0x3C) — Verbatim Trace Report

All citations are to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`).

**Headline finding (read first):** (10,59) and (10,60) are **not spikes and not solid objects at all**. They are invisible, unkillable, non-collidable **smoke/particle-column emitters** (life ≈ 800–899 ticks, one particle per tick). The remc2 authors named their creators `ArriveCheckpoint_4EB50` and `AddSmoke_4EC10`, and annotated the (10,59) creator "in quest point" (EV:4586). The **indestructible tall spikes** the player saw are terrain, and the only terrain-raising machine in the codebase is **class-14 model-1 (`sub_59F60`, heightmap +48/tick writes)** — see §7. Both (10,60) emitters and (14,1) risers are spawned by the same **disposition-deferred THING mechanism** (`sub_4A1E0`), which is exactly what fires "between the first and second stage triggers" (§6).

---

## 0. Dispatch architecture (tables and switches)

### 0.1 Class/creator/action registry `str_D4C48ar` (EF:2060)
Row 10 (EF:2071): `0x002A5C44,0x0000000A,0x0000,x_DWORD_D4C52ar_strA0,x_DWORD_D4C52ar_strA1` — **strA0 = action table (field `dword_10`)**, **strA1 = creator table (field `dword_14`)**. Row 14 (EF:2075) likewise binds strE0/strE1.

- Creator path: `IfSubtypeCallCreatingManaSphere_4A190(pos, type, subtype)` (EV:5186) — verbatim:
  ```c
  if (str_D4C48ar[type].dword_14[subtype].dword_10 && str_D4C48ar[type].dword_14[subtype].word_4 == subtype)
      return pre_sub_4A190_axis_3d(str_D4C48ar[type].dword_14[subtype].address_6, position);
  return 0;
  ```
- Action path: main tick `UpdateEntities_57730` (EF:39928) — for every live entity, if `actionIndex == str_D4C48ar[class].dword_10[actionIndex].word_4` and `.dword_10`, dispatch `pre_sub_4A190_0x6E8E(str_D4C48ar[class].dword_10[actionIndex].address_6, mx)` then `mx->byte_0x3E_62++` (EF:40131-40172). `pre_sub_4A190_0x6E8E` (EV:610) is the giant switch on the original-binary address.

### 0.2 Table rows of interest

| role | table | index | addr | function | EF table line |
|---|---|---|---|---|---|
| **m59 creator** | strA1 | 0x3B | 0x22FB50 | `ArriveCheckpoint_4EB50` (EF:35663) | EF:1763 |
| **m60 creator** | strA1 | 0x3C | 0x22FC10 | `AddSmoke_4EC10` (EF:35685) | EF:1764 |
| **m59 action (0x40)** | strA0 | 0x40 | 0x2133E0 | `AddParticleSmoke0A_3B_323E0` (EF:23654) | EF:1666 |
| **m60 action (0x41)** | strA0 | 0x41 | 0x213400 | `AddParticleSmoke0A_3C_32400` (EF:23660) | EF:1667 |
| shared emitter body | — | — | 0x213420 | `AddParticleSmoke0A_3D_32420` (EF:23666) | (EV:2361 case) |
| particle (10,13) creator | strA1 | 0x0D | 0x22F9E0 | `SetParticleSmoke3B_4E9E0` (EF:35618) | EF:1717 |
| particle (10,14) creator | strA1 | 0x0E | 0x22FA20 | `SetParticleSmoke3C_4EA20` (EF:35625) | EF:1718 |
| particle action 0x0D | strA0 | 0x0D | 0x213160 | `sub_32160` (EF:23572) | EF:1615 |
| particle action 0x0E | strA0 | 0x0E | 0x2132A0 | `sub_322A0` (EF:23613) | EF:1616 |

EV creator-switch cases: 0x22fb50 at **EV:4586** (comment `//in quest point`), 0x22fc10 at **EV:4590** (comment `// 1 instance in level 9`), 0x22f9e0 at EV:4569, 0x22fa20 at EV:4573. EV action-switch cases: 0x2133e0 at **EV:2352** (`//in quest point2`), 0x213400 at EV:2357, 0x213160 at **EV:2344** (`//in quest point3`), 0x2132a0 at EV:2348.

### 0.3 Numbering trap — strA0 rows 0x3B/0x3C are NOT these models' actions
strA0[0x3B]→0x219D80 (`sub_38D80`, EF:28349) and strA0[0x3C]→0x219E40 (`sub_38E40`, EF:28393) belong to **models 0x36 and 0x37**: `AddAuxiliary_50500` (EF:36812) sets `actionIndex=0x3B; model=0x36` and `sub_50640` (EF:36864) sets `actionIndex=0x3C; model=0x37`. Model 59/60 ctors set actionIndex **0x40/0x41**. (strA0[0x4C] also aliases 0x219D80, EF:1678.) Any port keying handler by model number here would be wrong.

---

## 1. Creators — verbatim

### 1.1 `ArriveCheckpoint_4EB50` — (10,59) emitter (EF:35663-35684)
```c
type_entity_0x6E8E* ArriveCheckpoint_4EB50(axis_3d* position)//22fb50
{
	type_entity_0x6E8E* tempevent; // eax
	if (sub_4A810_get_0x35plus() < 32)      // EF:33254: return D41A0_0.dword_0x35 + 1;  (free-entity count)
		return 0;
	tempevent = NewEvent_4A050();
	if (!tempevent)
		return 0;
	tempevent->actionIndex_0x45_69 = 0x40;
	tempevent->class_0x3F_63 = 0xA;
	tempevent->model_0x40_64 = 0x3B;
	tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;
	tempevent->maxLife_0x4 = tempevent->rand_0x14_20 % 0x64u + 800;
	tempevent->struct_byte_0xc_12_15.byte[0] = (tempevent->struct_byte_0xc_12_15.byte[0] & 0xF6) | 1;
	tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;
	tempevent->actSpeed_0x82_130 = tempevent->rand_0x14_20 % 0x11u;
	tempevent->position_0x4C_76 = *position;
	CopyMaxLifeToLife_49A20(tempevent);
	return tempevent;
}
```

### 1.2 `AddSmoke_4EC10` — (10,60) emitter (EF:35685-35706)
**Byte-for-byte identical** to 1.1 except `model_0x40_64 = 0x3C` and `actionIndex_0x45_69 = 0x41`.

**Creator facts (both):**
- Gated on ≥32 free entity slots (`sub_4A810_get_0x35plus() < 32 → return 0`) — under entity pressure the emitter silently fails to spawn.
- **RNG draws: 2** (entity-local `rand_0x14_20`, LCG `r = 9377*r + 9439`): draw 1 → `maxLife = r%100 + 800` (800..899 ticks); draw 2 → `actSpeed = r%17` (0..16 — a per-emitter particle-speed bonus, see §2).
- Flags: `NewEvent_4A050` (EV:561) initializes `struct_byte_0xc_12_15.dword = 8` (bit 3 = target-eligible). Ctor does `(byte[0] & 0xF6) | 1` → **clears bit 3 (0x08, target-eligible) and sets bit 0 (0x01)**. Net byte[0] = 0x01.
- **No `AddEventToMap_57D70`** — the emitter is NEVER inserted into the map-entity grid (`mapEntityIndex_15B4E0`, EF:40315). `position_0x4C_76` is written directly.
- **No sprite**: no `SetEntityIndexAndRot_49CD0`/`SetHalfSpeedEntity_49DA0`/`SetEntityShiftRot_49EA0` call — the emitter has no particle-row index and no extents. It is pure invisible logic.
- No mana, no behavior row beyond the NewEvent default (`&str_D7BD6[59]`), no sound.

**NewEvent defaults inherited** (EV:561-579): `maxLife=300` (overwritten), `actSpeed=16` (overwritten), `subSpellIndex=100`, `id_0x1A_26 = self index`, `xtype=-1, xsubtype=-1`, `byte_0x43_67=10`, `byte_0x39_57=-6`, `byte_0x3E_62 = self index`, `rand_0x14_20 = self index + D41A0_0.rand_0x8`.

---

## 2. Action handlers — verbatim (the full "phase machine")

There is no multi-phase machine: one state per model, shared body.

### 2.1 `AddParticleSmoke0A_3B_323E0` / `AddParticleSmoke0A_3C_32400` (EF:23654-23664)
```c
void AddParticleSmoke0A_3B_323E0(type_entity_0x6E8E* event)//2133e0
{ AddParticleSmoke0A_3D_32420(event); }
void AddParticleSmoke0A_3C_32400(type_entity_0x6E8E* event)//213400
{ AddParticleSmoke0A_3D_32420(event); }
```

### 2.2 Shared emitter tick `AddParticleSmoke0A_3D_32420` (EF:23666-23693)
```c
void AddParticleSmoke0A_3D_32420(type_entity_0x6E8E* event)//213420
{
	type_entity_0x6E8E* tempentity = 0; // ecx

	if (event->life_0x8-- < 0)
	{
		DisableEntityDrawing04_57F10(event);
		return;
	}
	axis_3d position = event->position_0x4C_76;
	event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
	position.x += event->rand_0x14_20 % 0xA0u;
	event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
	position.z += event->rand_0x14_20 % 0xA0u;
	if (event->model_0x40_64 == 0x3Bu)
		tempentity = SetParticleSmoke3B_4E9E0(&position);
	else if (event->model_0x40_64 == 0x3Cu)
		tempentity = SetParticleSmoke3C_4EA20(&position);
	if (tempentity)
	{
		event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;
		tempentity->life_0x8 = 32;
		tempentity->maxLife_0x4 = 32;
		tempentity->actSpeed_0x82_130 += event->actSpeed_0x82_130 + (event->rand_0x14_20 % 0x4Du);
	}
}
```
Per-tick facts:
- **Despawn:** `life_0x8--` per tick; when it underflows past 0 → `DisableEntityDrawing04_57F10` (EF:40332: `byte[1] |= 4`; the corpse is unlinked by `sub_57F20` in `UpdateEntities`). Total lifetime 800..899 ticks; one particle per tick.
- **Jitter is verbatim `x` and `z` only** — `position.x += r % 0xA0` (0..159 units, +x-biased, never −x) and `position.z += r % 0xA0` (vertical base jitter). **`y` is never jittered.** (Faithful port must reproduce this asymmetry.)
- Spawned particle gets **life/maxLife force-set to 32** (overriding the random life the sub-creator rolled, §3) and `actSpeed += emitter.actSpeed(0..16) + r%0x4D(0..76)`.
- **RNG draws per tick: 3** on the emitter's `rand_0x14_20` (x-jitter, z-jitter, speed bonus) **+ 1 global draw** (`D41A0_0.rand_0x8`) inside SetParticleSmoke3B/3C **+ 1 draw on the new particle's own rand** inside `SetSmoke4_4EAA0`. Order: x-jitter → z-jitter → global life roll → particle actSpeed roll → emitter speed-bonus roll.
- **No sounds. No damage output. No collision test. No terrain writes. No stage-variable interaction.**

### 2.3 Indestructibility / damage-mailbox handling
- Damage in MC2 is delivered by `sub_11900(attacker, victim, slot, amount)` (EF:4375) which only **writes a mailbox** (`str_0x5E_94.dword_0x5E_94` += amount, `word_0x62_98` = attacker id); the *victim's own action handler* must read it to take damage. Neither the emitter tick (§2.2) nor the particle ticks (§2.4/2.5) ever read `str_0x5E_94` → **any damage written to them is ignored; they are damage-immune by omission.**
- The emitters additionally can never be *found*: they are not in the map grid (no `AddEventToMap`), so cell-walking probes (e.g. `sub_10780`) never reach them; and byte[0] bit 3 is cleared, so even list-based target scans that gate on `byte[0] & 8` skip them.
- The particles are in the map grid but have byte[0] = 0 after `dword &= 0xFFFDFFF7` (§3) — bit 3 clear → fail every probe's `byte[0] & 8` gate. **Nothing in the game can target, hit, or destroy any part of the column.** ("Indestructible" is true of the columns — but they are also intangible; they cannot be the retail *solid* spikes.)

### 2.4 Particle tick, model 13 (action 0x0D) — `sub_32160` (EF:23572-23611), verbatim
```c
void sub_32160(type_entity_0x6E8E* entity)//213160
{
	if (entity->life_0x8-- < 0)
	{
		DisableEntityDrawing04_57F10(entity);
		return;
	}
	predictedAxis_EB398ar = entity->position_0x4C_76;
	entity->actSpeed_0x82_130 -= 4;
	if (entity->actSpeed_0x82_130 < 64)  entity->actSpeed_0x82_130 = 64;
	if (entity->actSpeed_0x82_130 > 128) entity->actSpeed_0x82_130 = 128;
	predictedAxis_EB398ar.z += entity->actSpeed_0x82_130;
	int tempAlt = getTerrainAlt_10C40(&entity->position_0x4C_76);
	if (predictedAxis_EB398ar.z < tempAlt)
		predictedAxis_EB398ar.z = tempAlt;
	entity->dword_0x10_16++;
	if (entity->dword_0x10_16 < 16)
	{
		MoveEntity_57FA0(&predictedAxis_EB398ar, entity->yaw_0x1C_28, 0, entity->maxSpeed_0x86_134);
		entity->maxSpeed_0x86_134 -= 52;
		if (entity->maxSpeed_0x86_134 < 30)   entity->maxSpeed_0x86_134 = 30;
		if (entity->maxSpeed_0x86_134 > 1024) entity->maxSpeed_0x86_134 = 1024;
		if (!(entity->dword_0x10_16 & 1))
		{
			if (entity->word_0x5A_90 < 74)
				entity->word_0x5A_90++;
		}
	}
	if (entity->life_0x8 < 6)
	{
		if (entity->word_0x5A_90 > 67)
			entity->word_0x5A_90--;
	}
	CopyEntityPosition_57CF0(entity, &predictedAxis_EB398ar);
}
```
So each particle: **rises** `z += actSpeed` per tick where actSpeed starts high (51..195, §3+§2.2) and is clamped into [64,128] (−4/tick decay) — over life 32 that is a ~2000-4000-unit-tall column of drift; horizontal drift along its (zero-initialized — see OPEN-4) yaw at speed 30 for the first 16 ticks; grow rate word_0x5A_90 (draw scale, name-inferred) up to 74 on even ticks, shrink toward 67 in the last 6 ticks of life. z clamped to terrain alt from below. No RNG, no sound, no collision.

### 2.5 Particle tick, model 14 (action 0x0E) — `sub_322A0` (EF:23613-23652)
Byte-for-byte identical to §2.4 except the scale band: grow cap `word_0x5A_90 < 16` and end-of-life floor `> 9`. (m60's column is the **thin** one; m59's is the fat one.)

---

## 3. Particle sub-creators — verbatim

### 3.1 `SetParticleSmoke3B_4E9E0` (EF:35618-35623) / `SetParticleSmoke3C_4EA20` (EF:35625-35630)
```c
type_entity_0x6E8E* SetParticleSmoke3B_4E9E0(axis_3d* position)//22f9e0
{
	D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;
	return SetSmoke4_4EAA0(position, 0xD, 0xD, 67, D41A0_0.rand_0x8 % 0x17u + 17);
}
type_entity_0x6E8E* SetParticleSmoke3C_4EA20(axis_3d* position)//22fa20
{
	D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;
	return SetSmoke4_4EAA0(position, 0xE, 0xE, 9, D41A0_0.rand_0x8 % 0x21u + 28);
}
```
Note this uses the **global** RNG `D41A0_0.rand_0x8`. The rolled life (17..39 / 28..60) is **immediately overwritten to 32** by the emitter (§2.2) — the roll only survives when (10,13)/(10,14) are created directly (e.g. authored as THINGs; both are registered creators, strA1 EF:1717-1718). The names "3B/3C" are remc2-author labels for "serves model 0x3B/0x3C"; they *create* models 0x0D/0x0E. (`sub_4EA60` EF:35632 is the same body for (10,0x57) with action 0x5E, sprite 67 — not our family.)

### 3.2 `SetSmoke4_4EAA0` (EF:35639-35661), verbatim
```c
type_entity_0x6E8E* SetSmoke4_4EAA0(axis_3d* position, char a2, char a3, __int16 entityIndex, int a5)//22faa0
{
	type_entity_0x6E8E* tempevent = NewEvent_4A050();
	if (tempevent)
	{
		tempevent->actionIndex_0x45_69 = a3;
		tempevent->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;
		tempevent->model_0x40_64 = a2;
		tempevent->maxLife_0x4 = a5;
		tempevent->rand_0x14_20 = 9377 * tempevent->rand_0x14_20 + 9439;//mybe must fix
		tempevent->class_0x3F_63 = 0xA;
		tempevent->maxSpeed_0x86_134 = 30;
		tempevent->xtype_0x41_65 = 10;
		tempevent->xsubtype_0x42_66 = a2;
		tempevent->actSpeed_0x82_130 = tempevent->rand_0x14_20 % 0x35 + 51;
		tempevent->struct_byte_0xc_12_15.byte[2] |= 2;
		AddEventToMap_57D70(tempevent, position);
		SetHalfSpeedEntity_49DA0(tempevent, entityIndex);
		CopyMaxLifeToLife_49A20(tempevent);
	}
	return tempevent;
}
```
- `dword &= 0xFFFDFFF7` clears byte[0] bit 3 (0x08) and byte[2] bit 1 (0x20000); then `byte[2] |= 2` re-sets byte[2] bit 1 — membership in the `D41A0_0.dword_0x11EA` overflow/recycle list handled at unlink time (`sub_57F20`, EV:5209+).
- One RNG draw on the particle's own rand: `actSpeed = r%0x35 + 51` (51..103), to which the emitter then adds 0..92 more (§2.2).
- **Sprite/extents**: `SetHalfSpeedEntity_49DA0(ev, idx)` (EF:32856) = `SetEntityIndexAndRot_49CD0(ev, idx)` then `array_0x52_82.pitch = .roll = particlesParameters_D951C[idx].speed_6/2; .fov = particlesParameters_D951C[idx].rotSpeed_8/2`. Sprite rows: **67** for m13 (m59's particles), **9** for m14 (m60's particles).
- Particles ARE map-registered (`AddEventToMap_57D70`), unlike their emitters.

---

## 4. Level-load / disposition init (THING spawn path)

### 4.1 The spawner `sub_4A1E0(a1=DisId, a2=consume)` (EF:32950-32996)
Iterates all 0x4B0 authored THINGs; for each with `type_0x30311 != 0 && DisId == a1` calls `sub_4A310(&thing)`, and if `a2` clears the THING (one-shot). Called with **a1=0 at level load** (EF:39425, EF:39474 — twice inside the loader) and with **a1 = trigger id at runtime** (§6). So a THING's `DisId` field is a *deferred-spawn gate*: DisId=0 → spawns at load; DisId=N → spawns when disposition N fires.

### 4.2 `sub_4A310` (EF:32999) — per-THING spawn + class-specific post-init
Verbatim head: gate `if (!str_D4C48ar[type].dword_14[subtype].dword_10) return;` then
```c
v11x.x = (entity->axis2d_4.x << 8) + 128;
v11x.y = (entity->axis2d_4.y << 8) + 128;
v11x.z = getTerrainAlt_10C40(&v11x);
indexx = IfSubtypeCallCreatingManaSphere_4A190(&v11x, entity->type_0x30311, entity->subtype_0x30311);
```
(tile-center, z snapped to terrain). In the `case 0xA:` switch, models **0x37..0x3C** (which includes **0x3B and 0x3C**) hit EF:33107:
```c
if (v4 < 0x3Du)
{
	sub_58DA0(entity, v3x);
	return;
}
if (v4 <= 0x3Eu)
	goto LABEL_49;      // models 0x3D/0x3E only: par1→byte_0x46_70, par2→dword_0x10_16 (EF:33228)
```
**⇒ (10,59)/(10,60) THINGs consume NO par1_14/par2_16/stageTag_12/word_10 fields.** Their entire post-init is `sub_58DA0` (EF:40650): scan `D41A0_0.stages_0x3654C[0..stageIndex]`; for stage records of kind 1/2/4/6 whose `str_36552_un.ptr0x30311` points at this THING, bind the spawned entity into the stage slot and set `str_3654D_byte1 |= 1` — i.e. **they can be referenced as stage-objective targets** (kind-6 stores the entity index; kinds 1/2/4 store the pointer). Whether level 000 actually references them is data (OPEN-2).

Nearby contrast (for the port's data model): class-0xB THINGs get `id_0x1A_26 = entity->stageTag_12` (EF:33199) — that id is the **disposition id the trigger will fire** (§6); class-0xE model-1 gets `byte_0x46_70 = par1_14; dword_0x10_16 = par2_16` via the shared `LABEL_49` (EF:33228-33231).

### 4.3 The settle loop wipes them — `ApplyEvents_498A0` (EV:410)
This load-time "run all class-10 effects to completion" loop **disables** class-10 models `>0x33 && (<0x50 || (>0x55 && !=0x58))` instead of ticking them (EV:511-517) — 0x3B/0x3C fall in that band. So any emitter alive when `ApplyEvents_498A0` runs is destroyed rather than fast-forwarded.

---

## 5. Who creates model 59 (and 60) at RUNTIME

Exhaustive grep of both files for `4A190(..., 10, 59|0x3B|60|0x3C)` and for direct calls of `ArriveCheckpoint_4EB50`/`AddSmoke_4EC10`: **zero code call sites**. The only entries to these creators are the EV creator-dispatch cases (EV:4586, EV:4590), reached solely via `IfSubtypeCallCreatingManaSphere_4A190` with data-supplied type/subtype — i.e. **the THING table**. Therefore:

> The (10,59) that appears mid-play in level 000 is an **authored THING with a non-zero DisId**, spawned by `sub_4A1E0(DisId)` when a trigger fires (§6). Same for any of the nine (10,60)s that appear late rather than at load. There is no hardcoded runtime spawner.

(remc2's own runtime annotations agree: `//in quest point` on the m59 creator case, EV:4586 — they observed it spawning at a quest point; `// 1 instance in level 9` on m60, EV:4590.)

## 6. Stage gates → dispositions → deferred THINGs (the "appears between triggers" mechanism)

Class-11 trigger entities carry the disposition id in `id_0x1A_26` (copied from THING `stageTag_12`, EF:33199). Their action handlers fire `sub_4A1E0(id, ...)`:

| handler | EF | fires when | consume |
|---|---|---|---|
| `AddSwitch0B_00_6F030` | 44499 | player inside trigger extent (`InitSwitchChainZaxisAndSound_6F850(ev,1)`) | yes → despawns |
| `CheckpointArrived_6F070` | 44511 | player arrives (`...6F850(ev,0)`) | yes → despawns |
| `sub_6F100` / `sub_6F0B0` | 54306 / 54408 | arrival, with 10-tick re-arm (`dword_0x10_16 = 10`) — **repeating** trigger | no (a2=0) |
| `AddSwitch0B_04_6F150` | 54329 | any player entity with `IsLevelEnd_0` set | yes |
| `AddSwitch0B_20_6F1C0` | 54353 | **stage gate**: `struct_0x3659C[player].stage_0x3659F[byte_0x46_70] == 2` (stage `byte_0x46_70` completed; byte_0x46_70 = THING par1, EF:33201) | yes |
| `sub_6F300` | 54457 | class bucket(s) emptied, 16-tick debounce; plays **sound 41** | yes |

The proximity probe `InitSwitchChainZaxisAndSound_6F850` (EF:44523-44541): every 8th tick (`byte_0x3E_62 & 7`), scan the player list `dword_38519` for `model==0` with `CompareAxisWithShift_10750(event, ix) == a2`; on trip, **sound 41** if `event->model > 3`. Additionally `sub_28xxx`-region building-completion code fires `sub_4A1E0(event->xtype_0x41_65, 1)` (EF:28173).

**Level-000 story:** stage-1 completion (or an arrival trigger near the spire) fires `AddSwitch0B_20_6F1C0`/`CheckpointArrived_6F070` → `sub_4A1E0(N)` → every THING with `DisId == N` spawns at once: the (14,1) risers **and/or** the nine (10,60) smoke columns, and later the single (10,59). Which THINGs carry which DisId is level data, not code (OPEN-1).

## 7. Connection to the class-14 model-1 terrain riser (`sub_59F60`)

- (14,1) creator `sub_51660` (EF:37378): `actionIndex=6; class=0xE; model=1; byte[0]=(&0xF6)|1; maxLife=0; life=0; subSpellIndex=0; AddEventToMap`. strE0[6] → 0x23AF60 → `sub_59F60` (EF:41255; EV case 2864). THING init: `byte_0x46_70 = par1_14` (orientation: 0 = x-run, 1 = y-run), `dword_0x10_16 = par2_16` (length seed) — EF:33228-33231.
- `sub_59F60` phase sketch (verbatim anchors): with `life == 0` it computes the anchor tile from `position` (EF:41475-41487), `dword_0x10_16++`, then **writes the heightmap**: `mapHeightmap_11B4E0[tile] += 48` per affected cell unless already raised (`mapTerrainType != 8` or neighbor delta > 30 → raise; EF:41496-41508/41648-41666), stamps `mapTerrainType_10B4E0[...] = 8; mapAngle_13B4E0[...] = 1` (EF:41519-41531), handles cave second-heightmap, shading, and 0x80 dirty flags; then `subSpellIndex = 48; life = 3` (EF:41641-41642, 41792-41793). With `life <= 1 && subSpellIndex < 0x30`: **sound 47** (EF:41802, again EF:42145) and the finishing/wall-stamp pass. It is a **terrain wall/ridge extruder** — solid, unkillable (it edits the heightmap itself), exactly matching "indestructible tall spikes rising from the ground". Full phase inventory of this function is a separate trace (OPEN-3).
- **Code connection to m59/m60: NONE.** `sub_59F60` never spawns (10,59)/(10,60) or vice versa; nothing references model 0x3B/0x3C anywhere in the riser, and the emitters write no terrain. The only linkage is **data-level**: both THING kinds can share a DisId so one stage gate raises the rock and lights the smoke columns at the same instant. `sub_5B070` (EF:42497) shows collision code specifically looks up class-14 model-1/2 occupants of a map cell — the riser family, not class-10, is the solid one.

**Interpretation for the retail comparison (flagged, not verbatim):** the nine (10,60) THINGs are most plausibly the *smoke/dust dressing* at the spike sites (or free-standing beacons); the spike *geometry* must come from (14,1) THINGs (or authored heightmap) fired by the same disposition. If our importer/renderer drops (14,1) THINGs or ignores DisId deferral, retail shows rising rock + smoke and we show neither/only-smoke — matching the observed diff. Verify level-000 THING records: expect (14,1) entries with par1=orientation, par2=length and DisId equal to the (10,60) group's DisId (OPEN-1).

## 8. Passing note — the (10,58) "returns-0" discrepancy
strA1[0x3A] → 0x2310A0 = `CreateManaSphere2560_500A0` (EF:36601) = `CreateManaSphere_500C0(position, 2560)` (EF:36607): a **(10, model 0x27=39) mana sphere** (action 0x29, `mana=2560`, `word_0x2C_44=128`, `byte_0x38_56=3`, `byte_0x39_57=128`, sprite via `SetManaSphereColorAndRot_36920`). So creating subtype **58** yields an entity whose `model` is **39** — code that then re-derives the creator from the entity's model would take the (10,39) row, and any identity check `created->model == 58` fails. The creator itself does not return 0 when slots exist. Not the focus here; flagged for the x1509 campaign discrepancy investigation.

---

## Constants table (consolidated)

| item | value | source |
|---|---|---|
| m59 ctor: class/model/action | 10 / 0x3B / **0x40** | EF:35670-35672 |
| m60 ctor: class/model/action | 10 / 0x3C / **0x41** | EF:35692-35694 |
| emitter spawn gate | free entities ≥ 32 else NULL | EF:35666, 35689; EF:33254 |
| emitter maxLife | `r%0x64 + 800` (800..899 ticks) | EF:35675, 35697 |
| emitter actSpeed (particle-speed bonus) | `r%0x11` (0..16) | EF:35678, 35700 |
| emitter flags | byte[0] = (…&0xF6)\|1 → bit0 set, bit3 (targetable) clear; **not map-registered; no sprite** | EF:35676, 35698 |
| emitter tick jitter | `pos.x += r%0xA0`; `pos.z += r%0xA0` (**no y jitter**) | EF:23677-23680 |
| particle life (forced) | 32 / 32 (maxLife/life) | EF:23688-23689 |
| particle life (sub-creator roll, dead value here) | m13: `gr%0x17+17`; m14: `gr%0x21+28` (global RNG) | EF:35621, 35628 |
| particle actSpeed | `(own r%0x35+51) + emitter.actSpeed + (emitter r%0x4D)` → 51..195, tick-decay −4 clamped [64,128] | EF:35652, 23690, 23579-23582 |
| particle rise | `z += actSpeed` per tick, floor = terrain alt | EF:23583-23586 |
| particle drift | yaw-forward `maxSpeed`=30 (clamped [30,1024], −52/tick), first 16 ticks only (`dword_0x10_16 < 16`) | EF:23588-23594 |
| particle scale band m13 | grow to 74 (even ticks), decay floor 67 when life<6 | EF:23595-23606 |
| particle scale band m14 | grow to 16, decay floor 9 when life<6 | EF:23636-23647 |
| particle sprite rows | m13 → `particlesParameters_D951C[67]`; m14 → `[9]` (extents = speed_6/2, fov = rotSpeed_8/2) | EF:35621, 35628, 32856-32862 |
| particle model/action | (10,13)/action 0x0D → sub_32160; (10,14)/action 0x0E → sub_322A0 | EF:1615-1616, 23572, 23613 |
| RNG law | `r = 9377*r + 9439` everywhere (emitter-local, particle-local, global rand_0x8) | EF:23676 etc. |
| RNG draws | ctor: 2 (local). Tick: 3 local + 1 global + 1 new-particle-local, order §2.2 | §1, §2.2, §3 |
| sounds (this family) | **none** | §2 |
| adjacent sounds | trigger trip 41 (EF:44539, 54474, 54497); riser settle 47 (EF:41802, 42145) | §6, §7 |
| damage | none dealt, none receivable (no mailbox read; emitters unprobeable, particles bit3-clear) | §2.3 |
| THING init consumption | **none** (models 0x37..0x3C: plain `sub_58DA0`) | EF:33107-33110 |
| runtime spawn | data-only: `sub_4A1E0(DisId)` from class-11 triggers / stage gates / building completion | §5, §6 |
| (14,1) riser | ctor sub_51660 EF:37378 (action 6); tick sub_59F60 EF:41255: heightmap +48/cell/step, type 8, angle 1, life 3/segment, par1=orientation par2=length | §7 |
| settle-loop behavior | ApplyEvents_498A0 disables m59/m60 instead of ticking (band >0x33 <0x50) | EV:511-517 |

## OPEN items
1. **Level-000 THING data unread** (this trace is code-only): the DisId values of the nine (10,60) THINGs, the runtime (10,59)'s DisId, and whether (14,1) riser THINGs exist near the central spire with a matching DisId. This decides the "spike" attribution of §7. Check the mgc-import THING dump for level 000.
2. Whether any level-000 **stage record (kind 1/2/4/6)** points at the (10,60)/(10,59) THINGs (`sub_58DA0` binding, §4.2) — would make the columns quest-objective markers ("quest point" per remc2's runtime notes).
3. **`sub_59F60` full phase machine** (life/subSpellIndex decrement sites, terminal state, despawn — no `DisableEntityDrawing` found in its body; the entity may persist as a solid class-14 map occupant per `sub_5B070`) is only sketched here; it needs its own verbatim trace before porting (14,1).
4. Particle `yaw_0x1C_28` is **0** (NewEvent memset; neither `SetSmoke4_4EAA0` nor the emitter writes it), so the 16-tick drift is always in the yaw-0 world direction plus the emitter's +x-biased jitter. Confirm against retail column lean if fidelity of the lean matters.
5. `word_0x5A_90` = draw scale and byte[0] bit0 semantics are **name-inferred** (bit0 is set by several static/marker ctors and cleared once via `&0xFE` in `sub_6F2B0` EF:54438; renderer-side meaning unread).
6. `particlesParameters_D951C[67]` and `[9]` row contents (actual sprite/animation data + speed_6/rotSpeed_8 numbers) not extracted — pull from the table when wiring visuals.
7. remc2's EV creator-case note `// 1 instance in level 9` (EV:4590) suggests their level-9 run spawned one (10,60); they never annotated nine at once — our level-000 nine-at-load observation should be re-verified against DisId deferral (they may be authored to spawn mid-game, item 1).
