# CLASS-10 High Band — Models 0x50/0x52/0x53/0x54/0x55/0x56 — Verbatim Trace Report

All citations are to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Format model: `docs/traces/mc2-class10-m59-m60.md` (read its §0 for the dispatch architecture; it is not repeated in full here).

---

## Headline finding (read first)

**The entire class-10 high band is the CAVE TERRAIN GENERATOR.** Models 0x50–0x55 are **invisible, one-shot, load-time heightmap sculptors** — authored THINGs that carve the cave floor/ceiling by writing `mapHeightmap_11B4E0` + the second heightmap `x_BYTE_14B4E0` + `mapAngle_13B4E0`/`mapTerrainType_10B4E0`, then self-destruct. **Every one of their creators is gated `if (!isCaveLevel_D41B6) return 0;`** (EF:36355, 36378, 36399, 36448) — they cannot exist outside cave levels. They are **run to completion during the load-time settle loop** `ApplyEvents_498A0` (the `>0x33 && (v4<0x50 || v4>0x55...)` disable-band deliberately *excludes* 0x50–0x55, so those are TICKED instead of disabled — EV:508). By the time the player has control, the sculptors are gone and only the terrain they wrote remains. This is why there are ~7000 authored records with no gameplay entity behind them: **they ARE the cave map geometry, expressed as spawn-time THINGs.**

Per-model identification:

- **(10,0x50=80)** — *degenerate/no-op sculptor.* Action 0x57 → `sub_34520` = pure `DisableEntityDrawing04` stub. Ctor adds to map, `maxLife=0`, no sprite. Writes NO terrain. Likely a **cave floor-marker / obsolete placeholder** (see OPEN-1). Largest count (3033) yet the tick does nothing — its effect, if any, is entirely map-registration at load.
- **(10,0x52=82)** — *flat-topped rectangular mesa/plateau block.* Action 0x59 → `sub_34910`. A fixed 6×6-tile box (byte_0x43=byte_0x44=3 half-extent) raised by `3·byte_0x46` (=6) height units in one tick. Ridge-outline finalize via `sub_34B00`+`sub_43C60`.
- **(10,0x53=83)** — *animated round dome/mound (rises over 16 ticks).* Action 0x5A → `sub_34C40`. Radius `axis_0x9A.x` (THING `word_10`), 3-phase machine (byte_0x46 0→1→2), sinusoidal profile (`sin_DB750`), height ramps in over `life` (starts 16).
- **(10,0x54=84)** — *animated round PIT/crater (digs the second-heightmap DOWN).* Action 0x5B → `sub_34EE0` (shared body, model-84 branch). Lowers `x_BYTE_14B4E0` inside a sinusoidal bowl. Depth = `par3_18` (position.z), radius `word_10`.
- **(10,0x55=85)** — *animated round HILL (raises mapHeightmap UP).* Action 0x5C → same `sub_34EE0`, model-85 branch. Raises `mapHeightmap_11B4E0` inside a sinusoidal dome. Peak = `par3_18`, radius `word_10`. (0x54/0x55 are the dig/raise pair of one function.)
- **(10,0x56=86)** — *cave ambient drip puff (RUNTIME-only, not load).* Action 0x5D → `sub_31120`, life 9, sprite `rand%3+332`, emits a (10,0x57) smoke particle at tick 4, occasional sound. **Spawned procedurally ahead of the player by `sub_58630` (EF:40114, cave-only, every-8th-turn) — NOT normally authored.** The "x7 in 1 level" authored records are hand-placed drips in one cave level. Disabled (not ticked) by the load settle loop, so authored ones do nothing until live play.

The sibling **(10,0x51=81)** (action 0x58 → `sub_34540`, ctor `sub_4FB80`… actually `sub_4FB20` EF:36329) is the **long swept ridge/tunnel-wall carver** — 32-sample cross-section swept along a heading for `EuclideanDist/0x55` steps. It has 0 records in the roster but is the 6th sculptor shape; documented in §7 for completeness because 0x54/0x55's shared `sub_34EE0` and the settle-band `!=0x58` clause reference it.

---

## 0. Dispatch table rows of interest

Registry `str_D4C48ar` row 10 (EF:2071): strA0 = action table (`dword_10`, keyed by actionIndex), strA1 = creator table (`dword_14`, keyed by model/subtype). Creator dispatch `IfSubtypeCallCreatingManaSphere_4A190` (EV:5186); action dispatch `UpdateEntities_57730` → `pre_sub_4A190_0x6E8E` giant switch (EV:610). See m59/m60 doc §0.

| model | strA1 creator | ctor fn (EF) | actionIndex set | strA0 action addr | action fn (EF) | EV creator case | EV action case |
|---|---|---|---|---|---|---|---|
| **0x50 (80)** | 0x230B80 | `sub_4FB80` (36352) | **0x57** | 0x215520 | `sub_34520` (25075) | EV:4702 | EV:2517 |
| 0x51 (81) | 0x230B20 | `sub_4FB20` (36329) | 0x58 | 0x215540 | `sub_34540` (25083) | EV:4698* | EV:2521 |
| **0x52 (82)** | 0x230BE0 | `sub_4FBE0` (36374) | **0x59** | 0x215910 | `sub_34910` (25265) | EV:4706 | EV:2525 |
| **0x53 (83)** | 0x230C30 | `sub_4FC30` (36397) | **0x5A** | 0x215C40 | `sub_34C40` (25419) | EV:4710 | EV:2539 |
| **0x54 (84)** | 0x230CA0 | `sub_4FCA0` (36421)→`sub_4FD00` (36445) | **0x5B** | 0x215EE0 | `sub_34EE0` (25544) | EV:4714 | EV:2543 |
| **0x55 (85)** | 0x230CD0 | `sub_4FCD0` (36433)→`sub_4FD00` (36445) | **0x5C** | **0x215EE0 (shared!)** | `sub_34EE0` (25544) | EV:4718 | (0x5C→same addr) |
| **0x56 (86)** | 0x231960 | `sub_50960` (37011) | **0x5D** | 0x212120 | `sub_31120` (22826) | EV:4848 | EV:2285 |

strA1 rows: EF:1784 (0x50), 1786 (0x52), 1787 (0x53), 1788 (0x54), 1789 (0x55), 1790 (0x56). strA0 rows: EF:1689 (0x57), 1691 (0x59), 1692 (0x5A), 1693 (0x5B), 1694 (0x5C→0x215EE0 same as 0x5B), 1695 (0x5D). **Numbering trap confirmed:** actionIndex 0x5B and 0x5C BOTH point at `sub_34EE0` — the function self-dispatches on `model_0x40_64` (84 vs 85) internally (EF:25608-25624, 25675-25699, 25722). No remc2 author comments on any of these EV cases.

---

## 1. Creators — verbatim

### 1.1 `sub_4FB80` — (10,0x50) degenerate sculptor (EF:36352-36371)
```c
type_entity_0x6E8E* sub_4FB80(axis_3d* position)//230b80
{
	type_entity_0x6E8E* event; // eax
	if (!isCaveLevel_D41B6)
		return 0;
	event = NewEvent_4A050();
	if (event)
	{
		event->maxLife_0x4 = 0;
		event->actionIndex_0x45_69 = 0x57;
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 0x50;
		event->position_0x4C_76 = *position;
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;   // clear bit3 (0x08 target-eligible)
		AddEventToMap_57D70(event, position);
		CopyMaxLifeToLife_49A20(event);                  // life = maxLife = 0
		return event;
	}
	return 0;
}
```
No RNG. No sprite. life=0 → first tick disables (§2.1).

### 1.2 `sub_4FBE0` — (10,0x52) box mesa (EF:36374-36394)
```c
type_entity_0x6E8E* sub_4FBE0(axis_3d* position)//230be0
{
	if (!isCaveLevel_D41B6) return 0;
	event = NewEvent_4A050();
	if (event)
	{
		event->maxLife_0x4 = 0;
		event->actionIndex_0x45_69 = 0x59;
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 0x52;
		event->position_0x4C_76 = *position;
		event->byte_0x46_70 = 2;   // height multiplier: raise = 3*2 = 6
		event->byte_0x43_67 = 3;   // half-extent X (box is 2*3=6 tiles wide)
		event->byte_0x44_68 = 3;   // half-extent Y (6 tiles deep)
		return event;
	}
	return 0;
}
```
No RNG, no sprite, **no AddEventToMap** (not grid-registered), no CopyMaxLifeToLife (life left at NewEvent default). byte[0] left at NewEvent default (0x08 set → but see §3: never read, entity disables tick 1).

### 1.3 `sub_4FC30` — (10,0x53) animated dome (EF:36397-36418)
```c
type_entity_0x6E8E* sub_4FC30(axis_3d* position)//230c30
{
	if (!isCaveLevel_D41B6) return 0;
	event = NewEvent_4A050();
	if (event)
	{
		event->life_0x8 = 16;                  // 16-tick animation
		event->actionIndex_0x45_69 = 90;       // 0x5A
		event->class_0x3F_63 = 10;
		event->model_0x40_64 = 83;
		event->position_0x4C_76 = *position;
		event->byte_0x46_70 = 0;               // phase 0 (init)
		event->axis_0x9A_154x.x = 2;           // radius default 2 (THING overrides w/ word_10, §4)
		event->axis_0x9A_154x.z = 0;
		event->struct_byte_0xc_12_15.byte[0] |= 1u;   // set bit0
		event->position_0x4C_76.z = 0;         // z=0 sentinel → phase0 randomises target height
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7; // clear bit3
		return event;
	}
	return 0;
}
```
No RNG in ctor (roll happens in phase-0 tick). No sprite, no AddEventToMap.

### 1.4 `sub_4FCA0`/`sub_4FCD0` → `sub_4FD00` — (10,0x54)/(10,0x55) pit & hill (EF:36421-36465)
```c
type_entity_0x6E8E* sub_4FCA0(axis_3d* position)//230ca0  (0x54 = pit)
{ type_entity_0x6E8E* event = sub_4FD00(position);
  if (event) { event->actionIndex_0x45_69 = 0x5B; event->model_0x40_64 = 0x54; }
  return event; }

type_entity_0x6E8E* sub_4FCD0(axis_3d* position)//230cd0  (0x55 = hill)
{ type_entity_0x6E8E* event = sub_4FD00(position);
  if (event) { event->actionIndex_0x45_69 = 0x5C; event->model_0x40_64 = 0x55; }
  return event; }

type_entity_0x6E8E* sub_4FD00(axis_3d* position)//230d00  (shared base ctor)
{
	type_entity_0x6E8E* event = NULL;
	if (isCaveLevel_D41B6) {
		event = NewEvent_4A050();
		if (event) {
			event->life_0x8 = 16;                     // 16-tick animation
			event->class_0x3F_63 = 0xA;
			event->position_0x4C_76 = *position;
			event->byte_0x46_70 = 0;                  // phase 0
			event->axis_0x9A_154x.x = 2;              // radius default (THING word_10 overrides)
			event->axis_0x9A_154x.z = 0;
			event->struct_byte_0xc_12_15.byte[0] |= 1u;
			event->position_0x4C_76.z = 0;
			event->struct_byte_0xc_12_15.byte[0] &= 0xF7;
		}
	}
	return event;
}
```
No RNG, no sprite, no AddEventToMap. Only difference between the two: model + actionIndex. (Note `maxLife` is left at NewEvent default 300 but never used; life=16 drives the tick.)

### 1.5 `sub_50960` — (10,0x56) cave drip puff (EF:37011-37034)
```c
type_entity_0x6E8E* sub_50960(axis_3d* position)//231960
{
	type_entity_0x6E8E* event = NewEvent_4A050();
	if (event)
	{
		event->maxLife_0x4 = 9;
		event->actionIndex_0x45_69 = 0x5D;
		event->class_0x3F_63 = 0xA;
		event->life_0x8 = event->maxLife_0x4;               // life = 9
		event->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;   // clear byte0 bit3 + byte2 bit1
		event->model_0x40_64 = 0x56;
		event->struct_byte_0xc_12_15.byte[2] |= 2;          // re-set byte2 bit1 (recycle-list membership)
		AddEventToMap_57D70(event, position);
		event->position_0x4C_76.z = getTerrainAlt_10C40(&event->position_0x4C_76);  // z snapped to floor
		event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;                    // 1 local RNG draw
		SetEntityIndexAndRot_49CD0(event, event->rand_0x14_20 % 3u + 332);          // sprite 332..334
		if (!(sub_104A0(&event->position_0x4C_76) & 1))                             // terrain-passability test
		{
			sub_57F20(event);                               // reject: unlink
			event = 0;
		}
	}
	return event;
}
```
**Only member of the band with a sprite** (rows 332/333/334) and the only one NOT `isCaveLevel`-gated at ctor time (its gate is the runtime spawner `sub_58630`, which is cave-only). RNG: 1 local draw (sprite pick). Rejected unless terrain cell passes `sub_104A0 & 1`.

**NewEvent_4A050 defaults inherited** (EV:561): `maxLife=300`, `actSpeed=16`, `subSpellIndex=100`, `id=self`, `xtype=-1, xsubtype=-1`, `byte_0x43_67=10, byte_0x44_68=?`, `byte_0x39=-6`, `byte_0x3E=self`, `struct_byte.dword=8` (byte0 bit3), `rand_0x14_20 = self + D41A0_0.rand_0x8`. (Overwrites per ctor above; note 0x52 ctor overrides byte_0x43/0x44 to 3.)

---

## 2. Action handlers — verbatim

### 2.1 `sub_34520` — (10,0x50) action 0x57 (EF:25075-25078)
```c
void sub_34520(type_entity_0x6E8E* a1x)//215520
{
	DisableEntityDrawing04_57F10(a1x);   // byte[1] |= 4  → unlinked next pass
}
```
**Does nothing but self-destruct.** No terrain write, no RNG, no children, no sound, no damage read. (See OPEN-1 for what (10,80) is meant to be — code-wise it is inert.)

### 2.2 `sub_34910` — (10,0x52) action 0x59, box mesa (EF:25265-25336)
```c
void sub_34910(type_entity_0x6E8E* a1x)//215910
{
	v13 = 3 * a1x->byte_0x46_70;                       // raise amount = 3*2 = 6
	v1 = a1x->byte_0x43_67;  v2 = 2 * v1;              // width  = 2*3 = 6 tiles
	v5x.x = (a1x->position_0x4C_76.x >> 8) - v1;       // origin = center - half-extent
	v3 = a1x->byte_0x44_68;  v4 = 2 * v3;              // depth  = 2*3 = 6 tiles
	v5x.y = (a1x->position_0x4C_76.y >> 8) - v3;
	v11 = sub_48E60(v5x.x, v5x.y, v2, 2*v3);           // MIN mapHeightmap over box perimeter
	v12 = sub_48E90(v5x.x, v5x.y, v2, v4);             // MAX mapHeightmap over box perimeter
	// for each cell in the v2 x v4 box:
	//   floor  = clamp(v11 - v13, 0, 254);   if mapHeightmap>floor: sub_570F0(...,floor,0,0,0)  -> LOWER floor
	//   ceil   = clamp(v13 + v12, 0, 254);   if ceil>second_heightmap: second_heightmap = ceil  -> RAISE ceiling
	//   sync mapAngle bit3 vs mapHeightmap/second_heightmap comparison
	sub_34B00(v5x.x-1, v5x.y-1, v2+1, v4+1);           // stamp ridge outline (terrainType=1, angle bit0)
	sub_43C60(v5x.x, v5x.y, v2, v4);                   // recompute shading/normals over box
	DisableEntityDrawing04_57F10(a1x);                 // one-shot: dies after the single stamp
}
```
**One-tick rectangular room-carve**: opens a 6×6 box by lowering the floor `mapHeightmap` by 6 and raising the ceiling `second_heightmap` by 6 relative to the perimeter min/max. `sub_570F0` (EF:39602) is the heightmap-write primitive. `sub_34B00` (EF:25339) stamps the wall-edge cells (`mapTerrainType=1`, angle bit0). No RNG, no sound, no children, no damage read.

### 2.3 `sub_34C40` — (10,0x53) action 0x5A, animated dome (EF:25419-25539)
```c
void sub_34C40(type_entity_0x6E8E* a1x)//215c40
{
	if (--a1x->life_0x8 <= 0) { DisableEntityDrawing04_57F10(a1x); return; }
	v2 = a1x->axis_0x9A_154x.x;  v3 = 2 * v2;          // radius (THING word_10), box side = 2r
	v4 = ((pos.x+128)>>8) - v2;  BYTE1 = ((pos.y+128)>>8) - v2;   // box origin
	v5 = a1x->byte_0x46_70;                            // PHASE
	if (v5 == 0) {                                     // PHASE 0: pick base+peak from terrain corners
		pos.z          = sub_48E60(v4,.., v3,v3);      //   pos.z = MIN mapHeightmap over box
		axis_0x9A.z    = sub_48EF0(v4,.., v3,v3);      //   axis.z = MAX second_heightmap over box
		byte_0x46_70   = (axis.z - pos.z <= 0) + 1;    //   → phase 1 (if range>0) else phase 2
	}
	else if (v5 == 1) {                                // PHASE 1: animate the dome
		v7 = v2 << 8;                                  //   radius in world units
		v14 = axis_0x9A.z - pos.z;                     //   full height range
		v17 = 192*v7 >> 8;                             //   inner-radius threshold (0.75r) for angle flag
		for each cell in 2r x 2r:
			d = EuclideanDistXYZ_58490(pos, cell);     //   3d distance from dome center
			if (d < v7) {
				h = v14 * ((0x10000 + sin_DB750[0x200 + (d<<10)/v7]) >> 1) >> 16;   // cosine dome profile
				lift = pos.z + h;   if>254: 254;
				if (lift > mapHeightmap[cell])
					sub_570F0(cell.x, cell.y, (lift-mapHeightmap)/life + mapHeightmap, 0, d<=v17, 1);  // ramp toward lift over `life`
				lower = clamp(0 + axis.z - h, >=0);
				if (lower < second_heightmap[cell])
					second_heightmap[cell] = second_heightmap - (second_heightmap-lower)/life;         // ramp ceiling down
				sync mapAngle bit3;
			}
	}
	else if (v5 == 2) { a1x->life_0x8 = 0; }           // PHASE 2: nothing to animate → die next tick
}
```
**Smooth circular dome** (floor rises, ceiling descends) with a cosine radial profile (`sin_DB750` LUT), incremented `/life` per tick so it grows over the 16-tick life. The angle-flag arg (`d<=v17`) marks the inner 75% as walkable. No RNG (base heights come from terrain, not random), no sound, no children, no damage read.

### 2.4 `sub_34EE0` — (10,0x54 pit) & (10,0x55 hill) action 0x5B/0x5C shared (EF:25544-25727)
```c
void sub_34EE0(type_entity_0x6E8E* a3x)//215ee0
{
	if (--a3x->life_0x8 <= 0) { DisableEntityDrawing04_57F10(a3x); return; }
	v4 = a3x->axis_0x9A_154x.x;  v38 = 2 * v4;          // radius, box side
	v39 = (box origin x,y)
	v7 = a3x->byte_0x46_70;                             // PHASE
	if (v7 == 0) {                                      // PHASE 0: choose target height at center
		if (model >= 0x54 && <= 0x54) {                 //   MODEL 84 (PIT):
			a1 = sub_48EF0(..)                          //     a1 = MAX second_heightmap over box
			a2 = a1 - sub_48E90(..) - 1;                //     a2 = range vs MAX mapHeightmap
		} else if (model == 85) {                       //   MODEL 85 (HILL):
			a1 = sub_48E60(..)                          //     a1 = MIN mapHeightmap over box
			a2 = sub_48EC0(..) - a1 - 1;                //     a2 = range vs MIN second_heightmap
		}
		if (a2 <= 0) byte_0x46_70 = 2;                  //   nothing to do → phase 2
		else {
			if (pos.z)  v10 = (51*pos.z*a2)/256;        //   depth/height from authored pos.z (par3_18)
			else { rand_0x14_20 = 9377*r+9439;          //   OR random if pos.z==0
			       v10 = r % a2; }
			axis_0x9A.z = a1;
			pos.z = (model==84) ? a1 - v10 : v10 + a1;  //   84: dig below; 85: raise above
			byte_0x46_70 = 1;
		}
	}
	else if (v7 == 1) {                                 // PHASE 1: animate the bowl/dome
		v35 = v4 << 8;                                  //   radius world units
		v14 = abs(pos.z - axis_0x9A.z);                 //   height range
		v30 = 49152*v4 >> 8;                            //   inner-radius threshold (0.75r)
		for each cell in 2r x 2r:
			d = EuclideanDistXYZ_58490(pos, cell);
			if (d < v35) {
				h = v14 * ((0x10000 + sin_DB750[0x200 + (d<<10)/v35]) >> 1);  // cosine profile
				v18 = h >> 16;
				if (model == 84) {                       //   PIT: lower second_heightmap toward (axis.z - v18)
					lo = max(axis_0x9A.z - v18, 0);
					cur = second_heightmap[cell];
					if (cur - (cur-lo)/life < cur) second_heightmap[cell] = cur - (cur-lo)/life;
				} else if (model == 85) {                //   HILL: raise mapHeightmap toward (axis.z + v18)
					hi = min(axis_0x9A.z + v18, 254);
					step = (hi - mapHeightmap[cell])/life;
					if (step + mapHeightmap > mapHeightmap)
						sub_570F0(cell.x, cell.y, step+mapHeightmap, 0, d<=v30, 1);
				}
				sync mapAngle bit3 vs mapHeightmap/second_heightmap;
			}
	}
	else if (v7 == 2) {                                 // PHASE 2: finalize
		if (model == 84) sub_43C60(v39, .., v38, v38);  //   pit re-shades its box
		a3x->life_0x8 = 0;                              //   die next tick
	}
}
```
**One function, two shapes by model:** 84 = circular **pit** carved into the ceiling/second-heightmap (goes DOWN), 85 = circular **hill/mound** raising the floor mapHeightmap (goes UP). Both cosine-profiled, ramped over 16-tick life. Depth/height authored via `par3_18` (position.z), or **randomised** (`rand_0x14_20 % a2`, one local RNG draw) when par3_18==0. No sound, no children, no damage read.

### 2.5 `sub_31120` — (10,0x56) action 0x5D, cave drip puff (EF:22826-22856)
```c
void sub_31120(type_entity_0x6E8E* a1x)//212120
{
	bool v4 = false;
	if (getTerrainAlt_10C40(&a1x->position_0x4C_76) != a1x->position_0x4C_76.z)
		goto LABEL_12;                                   // floor moved away → die
	if (a1x->maxLife_0x4 - 5 == a1x->life_0x8)            // at tick 4 (maxLife 9, life 4):
	{
		IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 87);  // spawn (10,0x57) smoke particle
		D41A0_0.rand_0x8 = 9377 * D41A0_0.rand_0x8 + 9439;                      // 1 GLOBAL RNG draw
		if (!(D41A0_0.rand_0x8 & 1))                                            //   50% chance:
			PrepareEventSound_6E450((a1x - D41A0_0.struct_0x6E8E), -1, a1x->word_0x5A_90 - 282);  // sound (id = word_0x5A - 282)
	}
	if (a1x->life_0x8-- < 0)
		LABEL_12: v4 = true;
	else
		sub_585A0(a1x);                                  // advance animationFrame (EF:40438)
	if (v4)
		DisableEntityDrawing04_57F10(a1x);
}
```
Short-lived (9 ticks) animated drip. At tick 4 it emits **one (10,0x57) smoke particle** (`SetSmoke4_4EAA0` sprite 67, life 17..39 — the same particle family as the m59/m60 columns, see m59/m60 doc §3.1) and plays a **water-drip sound** 50% of the time. `sub_585A0` (EF:40438) advances `animationFrame` up to `byte_0x5D_93`. Dies when its floor cell's terrain height changes or life underflows. No damage read.

---

## 3. Damage / targetability

**None of the six models reads the damage mailbox `str_0x5E_94`** (written by `sub_11900`, EF:4375). Grep of all six handlers' bodies for `0x5E_94`/`str_0x5E`/`sub_11900`: zero hits. They are **damage-immune by omission.** Beyond that:

- 0x50/0x52/0x53/0x54/0x55: byte[0] bit3 (0x08, target-eligible) is **cleared** in every ctor (`&0xF7` or `&0xFFFDFFF7`), and 0x52 is not even map-registered (`no AddEventToMap`). They can't be found by target scans that gate on `byte[0] & 8`, and they self-destruct within 1–16 ticks anyway.
- 0x56: byte[0] bit3 cleared too. It IS map-registered (sprite drip) but disables on the first floor-height mismatch and lives only 9 ticks.
- **No handler tests these models for collision or targeting** — the only model-number comparison anywhere is `sub_34EE0`'s internal `model == 84` self-dispatch (EF:25644, 25722) and `>= 0x54` (EF:25609, 25675). No collision code (e.g. `sub_5B070`) references class-10 models 0x50–0x56; the solid-occupant lookup is class-14 (see m59/m60 doc §7). **They are intangible logic that mutates the shared heightmap and then vanishes; the terrain they write is what the player collides with.**

---

## 4. Spawn paths — authored-THING post-init (`sub_4A310`, EF:32999)

`sub_4A310` spawns each THING at tile-center, z snapped to terrain (EF:33014-33017), then dispatches on class 10 model (EF:33033, `case 0xA:`). The relevant leaf branches for our band (EF:33118-33146):

```c
if (v4 > 0x43u) {
    if (v4 >= 0x53u) {
        if (v4 <= 0x53u) {                          // MODEL 0x53 (dome):
            v2x->axis_0x9A_154x.x = entity->word_10; //   radius = THING word_10
            sub_58DA0(entity, v3x);
        } else {
            if (v4 <= 0x55u) {                       // MODELS 0x54/0x55 (pit/hill):
                v6 = v2x->position_0x4C_76.y;
                v2x->position_0x4C_76.x -= 128;      //   recentre onto tile corner
                v2x->position_0x4C_76.y = v6 - 128;
                v2x->axis_0x9A_154x.x = entity->word_10; //   radius = word_10
                v2x->position_0x4C_76.z = entity->par3_18; //   depth/height = par3_18
            }
            sub_58DA0(entity, v3x);                  //   MODEL 0x56 falls here too: plain sub_58DA0
        }
        return;
    }
    // v4 in (0x43, 0x53): includes 0x50, 0x52
    if (v4 != 0x47) { sub_58DA0(entity, v3x); return; }   // MODELS 0x50, 0x52: PLAIN sub_58DA0, no par consumption
}
```

Per-model THING field consumption:

| model | THING fields consumed | meaning |
|---|---|---|
| **0x50** | none (plain `sub_58DA0`) | just spawn + stage-bind |
| **0x52** | none (plain `sub_58DA0`) | box size is ctor-fixed (6×6, height 6) |
| **0x53** | `word_10` → `axis_0x9A.x` | dome **radius** |
| **0x54** | `word_10` → radius; `par3_18` → `position.z` (depth); recentre −128,−128 | pit radius + depth |
| **0x55** | `word_10` → radius; `par3_18` → `position.z` (height); recentre −128,−128 | hill radius + height |
| **0x56** | none (plain `sub_58DA0`) | drip; but see §5 — authored 0x56 does nothing until live |

`sub_58DA0` (EF:40650) = the shared stage-objective binder (m59/m60 doc §4.2): scans `D41A0_0.stages_0x3654C` for stage records (kind 1/2/4/6) pointing at this THING and binds the spawned entity — i.e. any of these can be a **stage-objective target** if level data references it (unlikely for terrain sculptors; possible for a drip beacon).

### 4.1 The settle loop is what runs the sculptors — `ApplyEvents_498A0` (EV:410)
The load-time "run all class-10 effects to completion" loop (`while(runagain)`) TICKS class-10 models it doesn't disable. Its class-10 disable clause (EV:508):
```c
if (v4 > 0x33 && (v4 < 0x50 || v4 > 0x55 && v4 != 0x58))
    DisableEntityDrawing04_57F10(...);   // disabled (skipped)
else
    pre_sub_4A190_0x6E8E(...);           // TICKED, runagain = true
```
⇒ **models 0x50–0x55 (and 0x58) are the ONLY high-band models the settle loop ticks.** With `runagain=true` set every time one is ticked, the loop repeats until all their 1–16-tick animations complete — so the dome/pit/hill/box/ridge are fully carved into the heightmap **before gameplay starts**, then the sculptors self-destruct (`byte[1]&4 → sub_57F20`). Model **0x56 is >0x55 and !=0x58 → disabled at load** (authored drips never fire during the settle; they only run when re-spawned live).

---

## 5. Runtime creators (who calls these outside the THING table)

Exhaustive grep of both files for direct calls and `4A190(..,10,{80,82,83,84,85,86})`:

- **0x50, 0x52, 0x53, 0x54, 0x55: zero runtime call sites.** The only entries are the EV creator-dispatch cases (EV:4702/4706/4710/4714/4718), reached only via `IfSubtypeCallCreatingManaSphere_4A190` with data-supplied model — i.e. **the authored THING table.** They are pure level-geometry data.
- **0x56: one runtime spawner** — `sub_58630` (EF:40467) calls `IfSubtypeCallCreatingManaSphere_4A190(&v15x, 10, 86)` (EF:40550). `sub_58630` runs each frame inside `UpdateEntities` **only in cave levels** (`if (isCaveLevel_D41B6) sub_58630();`, EF:40113-40114), throttled to every 8th "turn" in single-player (`Turn_2BE0_11248 & 7`, EF:40497) or a random player in MP. It projects a point 2560 units ahead of the (throttled) player's facing, jitters it by `rand%0x14` in a 20×20 tile search, and spawns the drip on the first empty passable tile (`!mapTerrainType && !(mapAngle & 8)`, EF:40546). **This is the ambient cave-drip generator** — it makes drips appear procedurally in front of the flying player. This is why 0x56 has essentially no authored records: the 7 authored (10,86) in one level are hand-placed extras. (`sub_50960` itself has no direct callers besides the EV creator switch.)

The child (10,0x57=87) `sub_4EA60` (EF:35632) → `SetSmoke4_4EAA0(pos, 0x57, 0x5E, 67, gr%0x17+17)`: a smoke particle, sprite 67, life 17..39, action 0x5E → `sub_32160`-family riser (m59/m60 doc §2.4/§3). So the drip visibly emits a little smoke/vapor puff.

---

## 6. Cross-references

- No render/collision/AI function tests models 0x50–0x56 (only `sub_34EE0`'s internal 84/85 self-dispatch).
- `sin_DB750` (cosine LUT) is shared by the dome (0x53) and pit/hill (0x54/0x55) and by the class-14 riser family — the standard round-profile terrain primitive.
- Heightmap-write primitive `sub_570F0` (EF:39602) `(x, y, height, protectAngle, forceType1, edgeStamp)` is used by 0x52/0x53/0x55 (and the class-14 riser). The dual arrays: `mapHeightmap_11B4E0` (floor), `x_BYTE_14B4E0_second_heightmap` (ceiling/cave-roof), `mapAngle_13B4E0` (bit3 = ceiling-above-floor, bit0 = walkable/type1), `mapTerrainType_10B4E0`.
- Perimeter samplers: `sub_48E60`=min(mapHeightmap), `sub_48E90`=max(mapHeightmap), `sub_48EC0`=min(second_heightmap), `sub_48EF0`=max(second_heightmap) (EF:32623-32644, over `sub_48F20`/`sub_48FD0`).
- `Events.cpp:5676` has a commented-out `//result = sub_4A190(&v6, 10, 81)` debug line — remc2 authors were probing model 0x51 (the ridge carver, §7); confirms 0x51 is the same sculptor family, not our roster.

---

## 7. The sibling ridge carver (10,0x51=81) — `sub_34540` (context for the shared `!=0x58` clause)

Ctor `sub_4FB20` (EF:36329): `isCaveLevel`-gated, `maxLife=0`, action **0x58**, model **0x51**, `actSpeed_0x82_130 = 256`, `byte_0x46_70 = 2`, byte[0]&0xF7, CopyMaxLifeToLife. Action 0x58 → `sub_34540` (EF:25083-25261): builds a **32-sample terrain cross-section** (`x_BYTE_F01FEx`) sampling `(mapHeightmap+second_heightmap)/2` along a heading, then **sweeps it** for `EuclideanDist(pos, axis_0x9A)/0x55` steps (`MoveEntity 85 units/step`), stamping a parabolic-width channel via `sub_570F0`/second-heightmap writes and `sub_34B00` edge stamps. `byte_0x46_70` packs two 4-bit widths (`>>4<<8`, `&0xF<<8`) that taper start→end. **This is the long swept tunnel/ridge carver** — the one shape that connects points (uses `axis_0x9A` as an endpoint). It is excluded from the settle-loop disable (`!=0x58`) so it too runs to completion at load. **Not in the record roster (0 authored records)** — possibly unused in the shipped campaign, or used only in cut/HW content; flagged as OPEN-3.

---

## Constants table (consolidated)

| item | value | source |
|---|---|---|
| all ctors gate | `if (!isCaveLevel_D41B6) return 0` (except 0x56, gated by runtime spawner) | EF:36355,36378,36399,36448 |
| 0x50 ctor | action 0x57, maxLife/life 0, AddEventToMap, no sprite, byte0&0xF7 | EF:36352 |
| 0x50 tick | `DisableEntityDrawing04` only (inert) | EF:25075 |
| 0x52 ctor | action 0x59, byte_0x46=2 (raise 6), byte_0x43=byte_0x44=3 (6×6 box), no map, no sprite | EF:36374 |
| 0x52 tick | 1-tick box carve: floor −6, ceiling +6 over 6×6 tiles; edge stamp; then die | EF:25265 |
| 0x53 ctor | action 0x5A, life 16, radius default 2, byte_0x46=0 (phase0), pos.z=0 | EF:36397 |
| 0x53 tick | 3-phase cosine dome, radius `word_10`, floor↑/ceiling↓ ramped /life over 16 ticks | EF:25419 |
| 0x54 ctor | action 0x5B, life 16, phase0, radius 2, pos.z=0 (via sub_4FD00) | EF:36421,36445 |
| 0x54 tick | cosine **PIT** (second_heightmap DOWN), depth par3_18 or random, model-84 branch | EF:25544 |
| 0x55 ctor | action 0x5C, life 16, phase0, radius 2, pos.z=0 (via sub_4FD00) | EF:36433,36445 |
| 0x55 tick | cosine **HILL** (mapHeightmap UP), height par3_18 or random, model-85 branch | EF:25544 |
| 0x54/0x55 share | one function `sub_34EE0`, self-dispatch on model 84 vs 85 | EF:25608,25644,25722 |
| 0x56 ctor | action 0x5D, life 9, sprite `rand%3+332`, z=terrainAlt, reject if `!(sub_104A0&1)` | EF:37011 |
| 0x56 tick | at tick 4: spawn (10,87) smoke + 50% drip sound; sub_585A0 anim; die on floor mismatch/underflow | EF:22826 |
| 0x56 runtime spawner | `sub_58630` (EF:40467), cave-only, 2560 ahead of throttled player, empty passable tile | EF:40114,40550 |
| THING radius field | 0x53/0x54/0x55: `word_10 → axis_0x9A.x` | EF:33124,33134 |
| THING depth/height field | 0x54/0x55: `par3_18 → position.z` | EF:33135 |
| THING no-consume | 0x50, 0x52, 0x56: plain sub_58DA0 | EF:33141-33144, 33137 |
| settle-loop | ticks 0x50–0x55 to completion; disables 0x56 (band `>0x33 && (<0x50 \|\| >0x55 && !=0x58)`) | EV:508-521 |
| heightmap write | `sub_570F0(x,y,h,protectAngle,forceType1,edge)` | EF:39602 |
| perimeter samplers | 48E60 min(floor), 48E90 max(floor), 48EC0 min(ceil), 48EF0 max(ceil) | EF:32623 |
| radial profile LUT | `Maths::sin_DB750[0x200 + (d<<10)/r]` cosine dome | EF:25505,25672 |
| RNG (0x54/0x55 random depth) | `rand_0x14_20 = 9377*r+9439; v10 = r % a2` (1 local draw when par3_18==0) | EF:25639-25640 |
| RNG (0x56) | ctor: 1 local (sprite). tick: 1 global (sound coin-flip) | EF:37025, 22843 |
| sounds | 0x56 drip: `PrepareEventSound_6E450(self,-1, word_0x5A - 282)` 50% at tick 4; others: none | EF:22845 |
| damage | none dealt, none receivable (no `str_0x5E_94` read; all bit3-clear/short-lived) | §3 |
| sprites | only 0x56 (rows 332/333/334); 0x50–0x55 invisible | EF:37026 |

---

## OPEN items

1. **(10,80)=0x50 is code-inert** — its tick only self-destructs and it writes no terrain. With 3033 authored records it is by far the most common high-band THING, yet the decompile shows no effect beyond load-time map-registration (which `sub_58DA0` immediately un-links). Hypotheses: (a) a **cave floor/height marker** whose only role is to force `AddEventToMap` occupancy at load for some other pass to read; (b) an **obsolete/placeholder** shape whose handler was stubbed in the shipped build (compare vs an earlier build or the HW binary); (c) the terrain work is done by a DIFFERENT model the importer maps 0x50 onto. **Action: dump the level-000-style THING records for a cave level and check whether (10,80) THINGs carry par/word fields that a live build would consume — and whether removing them changes the baked heightmap.** This is the single biggest identification gap.
2. **par3_18 vs word_10 field names** for 0x54/0x55 are read straight from `sub_4A310` (EF:33134-33135); confirm the importer's THING struct maps these offsets to the same names before wiring radius/depth.
3. **(10,0x51=81) ridge carver** has 0 roster records — confirm it is genuinely unused in the shipped MC2 campaign (vs HW-only or cut). Its `sub_34540` is fully traced here (§7) if a level does reference it.
4. **Randomised vs authored depth (0x54/0x55):** when `par3_18==0` the depth/height is `rand_0x14_20 % a2` (one entity-local LCG draw). A faithful port must (a) seed `rand_0x14_20 = index + D41A0_0.rand_0x8` at NewEvent time and (b) draw in this exact order, or the load-time cave heights will diverge from retail. Since these run in the settle loop before any goldens are taken, this feeds the state-hash goldens directly.
5. **Second-heightmap semantics** (`x_BYTE_14B4E0`) as cave ceiling and `mapAngle` bit3 (`ceiling>floor`) / bit0 (walkable type-1) are inferred from the write patterns; verify against the renderer/collision side before trusting the pit (0x54) direction.
6. **(10,86) drip sound id** `word_0x5A_90 - 282` — `word_0x5A_90` is the draw-scale/sprite field; the `-282` offset implies the sound id is derived from the sprite row (332→50, etc.). Confirm the sample id when wiring audio.
7. **Settle-loop determinism:** the whole high band's terrain output depends on `ApplyEvents_498A0` ticking 0x50–0x55 to completion in entity-index order with `runagain` re-passes. The Rust port must reproduce that load-time fixpoint (order + per-entity RNG) exactly to match baked cave geometry.
