# MC2 Load-Time TERRAIN-AUTHORING Painters (sub_49090 chain family) — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, TR = `Terrain.cpp`). Trace date 2026-07-10. Closes **item 4 of `docs/traces/mc2-class10-m6-m9-m11-m28-m31.md` §4/§OPEN** (the (10,28)/(10,31) painter output models) and the **(10,80) OPEN item** in `mc2-class10-high-band.md`. Companion to `mc2-class10-m50-chains-and-tail.md` (§1 chain-walk conventions reused verbatim; the `sub_49090` walk, `sub_48690` waterpath, `sub_48880` beam stamper, the (10,51) beam, `NewEvent_4A050`, the RNG law `r=9377*r+9439`, `AddE7EE0x_10080`, `sub_10C80` — all documented there and NOT re-derived).

**Numbering: prompt subtypes are DECIMAL.** (10,28)=0x1C, (10,31)=0x1F, (10,27)=0x1B, (10,50)=0x32, (10,80)=0x50, (10,81)=0x51.

---

## Headline findings (read first — one line per painter)

1. **(10,28)=0x1C is the ROAD/PATH RIDGE painter.** `sub_49090`→`sub_48400` (EV:5365) walks a coarse **Bresenham** line between chain-node endpoints and, at each step, spawns **(10,27)=0x1B segment-walker** entities with `actionIndex` **27/28/29** (=0x1B/0x1C/0x1D) and a `dword_0x10_16` run-length. Those walkers (`sub_34110`/`sub_34000`/`sub_34210`, EF:24897/24863/24929) each **RAISE `mapHeightmap_11B4E0` by +48** along a strip, set `mapAngle_13B4E0 |= 0x80` (the "authored/locked" edge flag), and re-shade the strip via **`sub_46180(cell, 8)`** — writing terrain-type **8** (the road/ridge surface material). This is a fully-working painter. Water cells are protected by `sub_33F70` (TR:1744).

2. **(10,31)=0x1F is the RIVER/stroke marker — and its endpoint entity is INERT in remc2.** `sub_49090`→`sub_487D0` (EV:5558) computes yaw+length between endpoints, seeds `z = 32 * mapHeightmap[...]`, and spawns a **(10,50)=0x32** entity with `life = dist>>8`, `yaw`, `byte_0x46_70 = width` (the `par3` remap 0→2/1→6/2→16/3→32). **BUT the (10,50)=0x32 ctor (`sub_4FDE0`, EF:36488) sets action 0x36, and action 0x36 (`sub_352A0`, EF:25732) is a bare one-tick `DisableEntityDrawing04` self-destruct that never reads life/yaw/byte_0x46_70.** So in this decompile the river's spawned marker despawns without carving. Contrast the near-identical (10,50) beam-chain stamper `sub_48880` which spawns a **(10,51)=0x33** entity whose action 0x37 (`sub_352C0`) DOES travel and paint — the river path was wired to a self-destruct model, not the traveling one. See §3.4 (a real, citable OPEN discrepancy).

3. **(10,80)=0x50 is the CAVE TUNNEL/CAVERN carver — resolving the high-band OPEN item.** `sub_49090`→`sub_48930` (EV:5621) packs `par3&0xF | 16*(prev.par3&0xF)` into `byte_0x46_70` and spawns a **(10,81)=0x51** entity (NOT 0x50 — the handler explicitly targets subtype **81**, EV:5684/5686). The (10,81) ctor (`sub_4FB20`, EF:36329) is **cave-only** (`if(!isCaveLevel) return 0`) and sets action **0x58**. Action 0x58 (`sub_34540`, EF:25083) is a substantial **variable-radius tube carver**: it walks a stroke from `position`→`axis_0x9A_154x`, **LOWERS the floor `mapHeightmap_11B4E0`** (via `sub_570F0`) and **RAISES the ceiling `x_BYTE_14B4E0_second_heightmap`** across a disc whose radius eases from the start-nibble to the end-nibble of `byte_0x46_70`. **This IS cave-related and IS the real work behind the (10,80..86) band.** The (10,80)/(10,81) x3033 records are cave-tunnel authoring chains.

4. **Settle:** every model these painters spawn — (10,27)=0x1B, (10,28)=0x1C, (10,31)=0x1F, the river's (10,50)=0x32, and the cave (10,81)=0x51 — is in the **settle-TICK band, NOT the disable band** (EV:454-526). They run their one-shot stamp during the `ApplyEvents_498A0` settle loop after their GenerateEvents pass. Ordering: rivers (0x1F) run in **pass 2**, cave carvers (0x50/0x51) in **pass 3**, roads (0x1C) in **pass 5** — roads are painted last, over the river and cave terrain.

---

## 0. Dispatch rows

Registry `str_D4C48ar` row 10 (EF:2071) → `dword_10 = strA0` (action table keyed by **actionIndex**) + `dword_14 = strA1` (creator table keyed by **model**). Creator dispatch = `IfSubtypeCallCreatingManaSphere_4A190(pos,type,subtype)` (EV:5186); action dispatch = `UpdateEntities_57730`→`pre_sub_4A190_0x6E8E` switch. **Key handlers by the actionIndex the ctor writes, never by model.**

strA0 (action) rows, verbatim (EF:1627-1634, 1655-1657, 1688-1690):

| actionIndex | address | fn (EF) | role |
|---|---|---|---|
| 0x1B (27) | 0x215110 | `sub_34110` :24897 | road walker — Y+2 offset strip |
| 0x1C (28) | 0x215000 | `sub_34000` :24863 | road walker — parity/life-shifted strip |
| 0x1D (29) | 0x215210 | `sub_34210` :24929 | road walker — run-length + tail-lock strip |
| 0x1E (30) | 0x215330 | `sub_34330` :24989 | (10,28) marker: 1-tick despawn |
| 0x21 (33) | 0x215480 | `sub_34480` :25046 | (10,31) marker: 1-tick despawn |
| 0x36 (54) | 0x2162A0 | `sub_352A0` :25732 | (10,50) marker: **1-tick despawn** |
| 0x37 (55) | 0x2162C0 | `sub_352C0` :25739 | (10,51) traveling beam (paints) |
| 0x57 (87) | 0x215520 | `sub_34520` :25075 | (10,80) worker: 1-tick despawn |
| 0x58 (88) | 0x215540 | `sub_34540` :25083 | (10,81) **cave tube carver** |

strA1 (creator) rows, verbatim (EF:1730-1734, 1784-1785):

| model (dec) | address | ctor fn (EF) | action set | notes |
|---|---|---|---|---|
| 0x1B (27) | 0x2307A0 | `sub_4F7A0` :36151 | 0x1B | road segment-walker; overridden to 27/28/29 by sub_48400 |
| 0x1C (28) | 0x230800 | `sub_4F800` :36170 | 0x1E | road chain MARKER |
| 0x1F (31) | 0x230AC0 | `sub_4FAC0` :36311 | 0x21 | river chain MARKER |
| 0x32 (50) | 0x230DE0 | `sub_4FDE0` :36488 | 0x36 | river endpoint model (self-destruct) |
| 0x33 (51) | 0x230D70 | `sub_4FD70` :36468 | 0x37 | beam-chain endpoint model (paints) |
| 0x50 (80) | 0x230B80 | `sub_4FB80` :36352 | 0x57 | **cave-only** worker (1-tick) |
| 0x51 (81) | 0x230B20 | `sub_4FB20` :36329 | 0x58 | **cave-only** tube carver |

---

## 1. `sub_48400` (EV:5365) — the (10,28)=0x1C ROAD/PATH ridge painter

### 1.1 Full verbatim (EV:5365-5489)
```c
void sub_48400(uint16_t posX2, uint16_t posY2, uint16_t posX, uint16_t posY, uint8_t a5)//229400
{
	v4 = shortestLenght_48370(posX2, posX, MAP_SIZE);   // signed shortest X delta (torus-aware)
	v5 = v4;  v6 = v4;
	result = shortestLenght_48370(posY2, posY, MAP_SIZE);  // signed shortest Y delta
	v8 = result;
	if (v5 || result)                                   // non-degenerate segment
	{
		if (v6 < 0) {                                   // ensure walk goes +X: swap endpoints
			v9 = posX2; v6 = -v6; v8 = -v8;
			posX2 = posX; posX = v9;
			v10 = posY2; posY2 = posY; posY = v10;
		}
		if (v6 <= abs(v8))                              // Y-major segment
		{
			v17 = abs(v8 / 10);  v18 = v17 + 1;         // ~10 steps along the LONGER axis
			v19 = v8 / (v17 + 1); v20 = v19;            // per-step Y advance
			v21 = v8 - v18 * v19;                       // Y remainder
			v30 = v6 / v18;                             // per-step X advance
			result = v18 * (v6 / v18);
			v28 = v21;
			for (i = v6 - result; v18; v28 = 0)         // v18 steps
			{
				sub_483A0(posX2, posY2, posX, posY);    // set predictedAxis to (posX2,posY2)@maxHeight
				v22x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 27);
				if (v20 >= 0) { v22x->actionIndex_0x45_69 = 28; v23 = v20 + v28; }  // +Y strip
				else          { v22x->actionIndex_0x45_69 = 27; v23 = -v20 - v28; } // -Y strip
				v22x->dword_0x10_16 = v23;              // run-length = |Y advance this step|
				sub_483A0(posX2, (uint16_t)(v20 + v28 + posY2), posX, posY);
				v18--;
				posY2 += v20 + v28;
				v24x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 27);
				v24x->actionIndex_0x45_69 = 29;          // X-run cap walker
				v24x->dword_0x10_16 = i + v30;           // run-length = X advance this step
				result = 0;  posX2 += i + v30;  i = 0;
			}
		}
		else                                             // X-major segment
		{
			v11 = v6 / 10 + 1;                           // ~10 steps
			v29 = v6 / v11;  v12 = v8 / v11;
			result = v11 * (v8 / v11);
			v25 = v6 - v11 * (v6 / v11);  v27 = v8 - result;
			if (v6 / 10 != -1) {
				do {
					sub_483A0(posX2, posY2, posX, posY);
					v13x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 27);
					v13x->actionIndex_0x45_69 = 29;      // X-run walker
					v13x->dword_0x10_16 = v25 + v29;     // run-length = X advance
					posX2 += v25 + v29;
					sub_483A0(posX2, posY2, posX, posY);
					v14x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 27);
					if (v12 >= 0) { v14x->actionIndex_0x45_69 = 28; v15 = v12 + v27; }  // +Y
					else          { v14x->actionIndex_0x45_69 = 27; v15 = -v12 - v27; } // -Y
					v14x->dword_0x10_16 = v15;
					v11--;  result = v12 + v27;  v25 = 0;
					v16 = v12 + v27 + posY2;  v27 = 0;  posY2 = v16;
				} while (v11);
			}
		}
	}
}
```

### 1.2 What it does
- **`shortestLenght_48370` (EV:5753)** returns the torus-shortest signed delta on a 256-wide map: `d = p2-p1; if (d > mapSize/2) d-=mapSize; if (d < -mapSize/2) d+=mapSize;`. So roads wrap the world edge correctly.
- The walk is a **coarse Bresenham** subdivided into ~`|major|/10 + 1` steps. At each step it lays down **two (10,27) segment-walkers**: one that runs a strip in ±Y (action **27** for -Y, **28** for +Y) with `dword_0x10_16 = |Y advance|`, and one that runs a strip in +X (action **29**) with `dword_0x10_16 = |X advance|`. Together they trace a staircased ridge along the segment.
- **`sub_483A0` (EV:5763)** sets the shared spawn point `predictedAxis_EB398ar` to `(x<<8, y<<8, 32*max(heightmap[from],heightmap[to]))` — i.e. the (10,27) is spawned at the higher of the two endpoint heights (so the ridge sits on top of terrain).
- `a5` (the chain style byte) is **NOT consumed** by the road painter.
- **Each (10,27) walker's `dword_0x10_16` is the run-length** (how many tiles that strip covers this step); its **`life_0x8` = 2** (from the (10,27) ctor, §2.1) is the strip WIDTH (the inner `for(i=life;i;i--)` loop count).

---

## 2. The (10,27)=0x1B segment-walker model — creator + actions 27/28/29

### 2.1 Creator `sub_4F7A0` (EF:36151), verbatim
```c
type_entity_0x6E8E* sub_4F7A0(axis_3d* position)//2307a0
{
	event = NewEvent_4A050();
	if (event) {
		event->maxLife_0x4 = 2;                              // life = 2 = STRIP WIDTH
		event->actionIndex_0x45_69 = 0x1B;                  // default (sub_48400 overrides to 27/28/29)
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 0x1B;
		event->subSpellIndex_0x2A_42 = (position->z >> 5) + 48;  // (unused by the walkers)
		event->dword_0x10_16 = 10;                          // default run-length (overridden)
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7;       // not targetable
		AddEventToMap_57D70(event, position);
		CopyMaxLifeToLife_49A20(event);                     // life_0x8 = 2
	}
	return event;
}
```
**Ctor facts:** RNG 0. `life = maxLife = 2` (strip width). `dword_0x10_16 = 10` and action `0x1B` are **defaults that sub_48400 overwrites** with the real run-length and the 27/28/29 variant. `subSpellIndex = (z>>5)+48` is set but never read by the walkers. Not targetable; map-registered.

### 2.2 Action 28 = 0x1C `sub_34000` (EF:24863), verbatim — parity/life-shifted +Y strip
```c
void sub_34000(type_entity_0x6E8E* a1x)//215000
{
	LOBYTE(v3) = (a1x->position_0x4C_76.x + 128) >> 8;   // tile X
	HIBYTE(v3) = (a1x->position_0x4C_76.y + 128) >> 8;   // tile Y (packed into v3 hi byte)
	if (v3 % 2) LOBYTE(v3) = v3 + 1;                     // snap X to even parity
	LOBYTE(v3) = v3 - a1x->life_0x8 + 1;                 // back up X by (width-1)
	v5 = a1x->life_0x8 + a1x->dword_0x10_16;             // total rows = width + run-length
	while (v5) {
		HIBYTE(v1) = HIBYTE(v3);
		LOBYTE(v1) = v3 - 1;
		mapAngle_13B4E0[v1] |= 0x80u;                    // lock the left border cell
		LOBYTE(v1) = v3;
		for (i = a1x->life_0x8; i; i--) {                // stamp `width` cells across
			if (mapTerrainType_10B4E0[v1] != 8 || sub_33F70(v1))
				mapHeightmap_11B4E0[v1] += 48;           // RAISE ridge +48
			sub_46180(v1++, 8);                          // paint terrain-type 8 + reshade
		}
		mapAngle_13B4E0[v1] |= 0x80u;                    // lock the right border cell
		v5--;
		HIBYTE(v3)++;                                    // advance +Y
	}
	DisableEntityDrawing04_57F10(a1x);                   // ONE-SHOT
}
```

### 2.3 Action 27 = 0x1B `sub_34110` (EF:24897), verbatim — +Y strip (no parity snap), Y descends
```c
void sub_34110(type_entity_0x6E8E* a1x)//215110
{
	v3 = (a1x->position_0x4C_76.x + 128) >> 8;           // tile X
	v4 = ((a1x->position_0x4C_76.y + 128) >> 8) + 2;     // tile Y + 2
	v6 = a1x->life_0x8 + a1x->dword_0x10_16;             // rows = width + run-length
	while (v6) {
		HIBYTE(v1) = v4;  LOBYTE(v1) = v3 - 1;
		mapAngle_13B4E0[v1] |= 0x80u;
		LOBYTE(v1) = v3;
		for (i = a1x->life_0x8; i; i--) {
			if (mapTerrainType_10B4E0[v1] != 8 || sub_33F70(v1))
				mapHeightmap_11B4E0[v1] += 48;
			sub_46180(v1++, 8);
		}
		mapAngle_13B4E0[v1] |= 0x80u;
		v6--;  v4--;                                     // advance -Y
	}
	DisableEntityDrawing04_57F10(a1x);
}
```

### 2.4 Action 29 = 0x1D `sub_34210` (EF:24929), verbatim — X-run with tail-locked borders
```c
void sub_34210(type_entity_0x6E8E* a1x)//215210
{
	LOBYTE(v12) = (uint16_t)(a1x->position_0x4C_76.x + 128) >> 8;   // X
	HIBYTE(v12) = (uint16_t)(a1x->position_0x4C_76.y + 128) >> 8;   // Y
	v1 = ((uint8_t)v12 + BYTE1(v12)) % 2;
	if (v1) LOBYTE(v12) = v12 + 1;                       // parity snap on (X+Y)
	v2 = v12;
	LOWORD(v1) = a1x->dword_0x10_16;                     // run-length
	--BYTE1(v2);
	while ((x_WORD)v1) {                                 // lock the top border row over the run
		v3 = v2; --v1;
		v4 = mapAngle_13B4E0[(uint16_t)v2++] | 0x80;
		mapAngle_13B4E0[v3] = v4;
	}
	for (i = a1x->life_0x8; i; i--) {                    // `width` rows
		LOWORD(v1) = a1x->dword_0x10_16;
		v5 = v12;
		while (1) {                                       // `run-length` cells across in +X
			v11 = v1; if (!(x_WORD)v1) break;
			if (mapTerrainType_10B4E0[v5] != 8 || sub_33F70(v5))
				mapHeightmap_11B4E0[v5] += 48;
			sub_46180(v5++, 8);
			v1 = v11 - 1;
		}
		++BYTE1(v12);                                    // next row +Y
	}
	v6 = v12;
	v7 = a1x->dword_0x10_16;
	while (v7) {                                          // lock the bottom border row
		v8 = v6; v7--;
		v9 = mapAngle_13B4E0[(uint16_t)v6++] | 0x80;
		mapAngle_13B4E0[v8] = v9;
	}
	DisableEntityDrawing04_57F10(a1x);
}
```

### 2.5 What each walker stamps per tick (they run exactly ONCE, then despawn)
| write | value | meaning |
|---|---|---|
| `mapHeightmap_11B4E0[cell] += 48` | +48 | raise the road ridge, **unless** the cell is already road-type-8 AND `sub_33F70` says it would flood a lower water body |
| `mapTerrainType_10B4E0[cell] = 8` (via `sub_46180`, on a 2×2 block) | 8 | the road/path surface material |
| `mapShading_12B4E0[3×3 nbhd]` (via `sub_46180`) | slope-derived | re-shade around each painted cell (Day = raw, else inverted `32-v+32`); cave also bumps `x_BYTE_14B4E0` second-heightmap |
| `mapAngle_13B4E0[border cells] |= 0x80` | bit7 set | mark the strip borders as "authored/locked" (prevents later generation from re-classifying them) |

- **Run-length countdown:** `dword_0x10_16` is the number of tiles the strip runs (set by `sub_48400`). Actions 27/28 iterate `v5/v6 = life + dword_0x10_16` ROWS along the walk axis, each row `life`(=2) cells wide. Action 29 iterates `life`(=2) rows, each `dword_0x10_16` cells long, with a locked border row before and after. There is NO tick-by-tick decrement — the whole strip is stamped in one `sub_XXXXX` call, then `DisableEntityDrawing04` despawns the walker.
- **`life_0x8` = 2 = strip width** (the `for(i=life;i;i--)` count).
- **Children spawned:** none. The walkers are leaf painters.
- **`sub_33F70` (TR:1744):** returns 0 only when the cell is water-type-8 and lower than all 4 neighbors + 30 — protecting the interior of a water body from being raised while still letting the road climb its banks.

### 2.6 Settle: the walkers are TICKED, not disabled
In `ApplyEvents_498A0` (EV:453-505): for class 0xA, `v4 = model`. Model **0x1B (27)** is not `< 0x1B`, so it falls to the `else` branch; `v4 <= 0x20` is TRUE (0x1B ≤ 0x20) → `runagain=true`, action dispatched (EV:493-504). **⇒ (10,27) walkers run their stamp during settle.** They are one-shot (each ends in `DisableEntityDrawing04`), so they're gone after the pass-5 `ApplyEvents`. They are NOT in the disable band (EV:508 `v4 > 0x33 && ...`).

---

## 3. `sub_487D0` (EV:5558) — the (10,31)=0x1F RIVER/stroke painter

### 3.1 Full verbatim
```c
void sub_487D0(uint16_t posX2, uint16_t posY2, uint16_t posX, uint16_t posY, uint8_t a5)//2297d0
{
	v8x.x = posX2 << 8;
	v8x.y = posY2 << 8;
	v8x.z = 32 * mapHeightmap_11B4E0[256 * posY2 + posX2];   // FROM node, z = 32*heightmap seed
	v11x.x = posX << 8;
	v11x.y = posY << 8;
	v5 = Maths::sub_581E0_maybe_tan2(&v8x, &v11x);          // yaw FROM->TO
	v6 = Maths::EuclideanDistXYZ_58490(&v8x, &v11x);        // segment length
	resultx = IfSubtypeCallCreatingManaSphere_4A190(&v8x, 10, 32);  // spawn (10,50)=0x32 at FROM
	if (resultx) {
		resultx->yaw_0x1C_28 = v5;
		resultx->life_0x8 = (signed int)v6 >> 8;            // life = length/256
		resultx->byte_0x46_70 = a5;                         // width (par3 remapped 0->2/1->6/2->16/3->32)
	}
}
```

### 3.2 Facts
- **`byte_0x46_70 = a5` = width.** `a5` is the `par3_18` remap from `sub_49090` case 0x1F (EV:5329-5346): **0→2, 1→6, 2→16, 3→32**.
- **`z = 32 * mapHeightmap[...]`** — the river source snaps to 32× the terrain height at the FROM tile.
- **`life = length >> 8`** — sized to the segment length (one life-unit per 256 world-units).
- Spawns subtype **0x32 = (10,50)**.

### 3.3 What the spawned (10,50)=0x32 ACTUALLY paints
The (10,50) ctor `sub_4FDE0` (EF:36488) sets `actionIndex = 0x36`. Action **0x36** = `sub_352A0` (EF:25732), verbatim:
```c
void sub_352A0(type_entity_0x6E8E* a1x)//2162a0
{
	DisableEntityDrawing04_57F10(a1x);   // ONE-TICK SELF-DESTRUCT. Nothing else.
}
```
**⇒ It paints NOTHING.** `sub_352A0` never reads `life`, `yaw`, or `byte_0x46_70`. On its first settle tick (model 0x32 → `v4 >= 0x32 && !(v4 > 0x33)` → TICKED, EV:506-525) it simply despawns.

### 3.4 The discrepancy (OPEN-1) — river vs. beam model mixup
The river painter `sub_487D0` and the beam-chain stamper `sub_48880` (m50 doc §1.3) are structurally identical, differing only in the subtype they spawn:
| painter | spawns | ctor action | that action's tick |
|---|---|---|---|
| `sub_48880` (beam chain, 0x32-THING) | **(10,51)=0x33** | 0x37 `sub_352C0` | **travels + `sub_10C80` damage + terrain probe** (EF:25739) |
| `sub_487D0` (river, 0x1F-THING) | **(10,50)=0x32** | 0x36 `sub_352A0` | **self-destruct** (EF:25732) |

The (10,51) beam (action 0x37, `sub_352C0`) advances 1024/tick along its yaw, probes ahead with `sub_572C0`, and calls `sub_10C80` — a traveling stroke. The river's (10,50) has `life`/`yaw`/`width` set with the SAME intent (a traveling carve), but is wired to the self-destruct model. **In this remc2 decompile the (10,31) river authoring is therefore inert: it produces a marker that despawns without carving.** Two readings: (a) remc2 has not finished porting river carving through this path (most likely — the fields are set but the consumer is a stub); (b) the river's actual water is baked into the heightmap/`river`/`lriver` header fields at level-gen time (`level_mc2.rs` carries `river`/`lriver`), not stamped by (10,31) THINGs at all. **Do NOT port (10,31) as a working carver on the strength of this trace alone** — verify against a level with authored (10,31) river THINGs and recorded gameplay before implementing. The width remap and z-seed are real; the carve consumer is missing.

---

## 4. `sub_48930` (EV:5621) — the (10,80)=0x50 CAVE-CARVE painter (resolves the high-band OPEN)

### 4.1 Full verbatim (comments/debug scaffolding elided)
```c
void sub_48930(uint16_t posX2, uint16_t posY2, uint16_t posX, uint16_t posY, uint8_t a5)//229930
{
	v8x.z = 0;
	v6 = posX2 << 8;                       // FROM node (packed into v6/v7 as __int16)
	v7 = posY2 << 8;
	v8x.x = posX << 8;                     // TO node
	v8x.y = posY << 8;

	v3x = &str_D4C48ar[10].dword_14[81];   // <-- creator row for subtype 81 (0x51), NOT 0x50
	if (v3x->dword_10 && v3x->word_4 == 81)
	{
		axis_3d a1x;  a1x.x = v6;  a1x.y = v7;  a1x.z = 0;
		result = pre_sub_4A190_axis_3d(v3x->address_6, &a1x);   // spawn (10,81) at FROM
	}
	else result = 0;

	if (result) {
		result->axis_0x9A_154x = v8x;      // destination = TO node (carver walks FROM->TO)
		result->byte_0x46_70 = a5;         // packed radii: par3&0xF | 16*(prev.par3&0xF)
	}
}
```

### 4.2 Facts
- **The `par3` packing** from `sub_49090` case 0x50 (EV:5348-5352) is `v8 = tempEntity->par3_18 & 0xF | 16 * (v8 & 0xF)` — i.e. low nibble = the NEXT node's par3, high nibble = the PREVIOUS node's par3. This `byte_0x46_70` gives the carver a **start radius (high nibble) and end radius (low nibble)** so the tube can taper along the segment.
- **It spawns subtype 81 (0x51), NOT 0x50** — the handler explicitly indexes `str_D4C48ar[10].dword_14[81]` and checks `word_4 == 81`. This is the load-bearing detail resolving the (10,80) puzzle: (10,80)=0x50 is the CHAIN-authoring subtype (routed here by `sub_49090`), and the WORKER it produces is (10,81)=0x51.
- **`axis_0x9A_154x` = the TO endpoint**; `position` = the FROM endpoint. The carver walks between them.

### 4.3 (10,81)=0x51 worker — ctor + the carve
Ctor `sub_4FB20` (EF:36329): **cave-only** (`if(!isCaveLevel_D41B6) return 0`), action **0x58**, `byte_0x46_70 = 2`, `actSpeed = 256`, life 0, not targetable, NOT map-registered.

Action 0x58 `sub_34540` (EF:25083) — the tube carver, verbatim spine:
```c
void sub_34540(type_entity_0x6E8E* a1x)//215540
{
	v28 = (a1x->byte_0x46_70 >> 4 << 8) + 512;             // START radius (high nibble)
	v27 = ((a1x->byte_0x46_70 & 0xF) << 8) + 512;          // END radius (low nibble)
	v29 = EuclideanDistXYZ_58490(&position, &axis_0x9A_154x) / 0x55;   // number of steps
	v34 = sub_581E0_maybe_tan2(&position, &axis_0x9A_154x);            // yaw FROM->TO
	// --- prime a 32-sample rolling floor-height buffer along the stroke ---
	v33 = 0;  v25x = a1x->position_0x4C_76;
	while (v33 < 32) {
		v1x = (mapHeightmap_11B4E0[cell] + x_BYTE_14B4E0_second_heightmap[cell]) / 2;   // mid of floor+ceiling
		clamp v1x [0,254];
		x_BYTE_F01FEx[2 + v33++] = v1x;
		MoveEntity_57FA0(&v25x, v34, 0, 85);
	}
	// --- walk the segment, carving a disc at each step ---
	v30 = 0;  v23x = a1x->position_0x4C_76;
	while (v30 < v29) {
		v2 = v28 + v30 * ((v27 - v28) / v29);              // radius eased START->END
		v36 = v2 * v2;                                     // radius^2
		... establish a (v4 x v4) bounding box of tiles around the stroke point ...
		for each tile (v39x) in the box:
			if (dx^2 + dy^2 <= radius^2) {
				v12 = radius^2 - dx^2 - dy^2;
				v14 = sub_7277A_radix_3d(v12) >> 5;        // sqrt-ish falloff -> half-height
				v15 = clamp(baseline - v14, 0, 254);
				if (mapHeightmap_11B4E0[cell] > v15)
					sub_570F0(x, y, v15, 0, 0, 1);          // LOWER the FLOOR (carve down)
				v16 = clamp(v14 + baseline, 0, 254);
				if (x_BYTE_14B4E0_second_heightmap[cell] < v16)
					x_BYTE_14B4E0_second_heightmap[cell] = v16;   // RAISE the CEILING (carve up)
			}
			// maintain the cave floor<->ceiling relationship + mapAngle bit3 (cave-shade)
			if (second_heightmap[cell] > heightmap[cell]) mapAngle_13B4E0[cell] &= 0xF7;
			else { second_heightmap[cell] = heightmap[cell]-1; mapAngle_13B4E0[cell] |= 8; }
		sub_34B00(box);                                    // retile/reshade the carved box
		MoveEntity_57FA0(&v23x, v34, 0, 85);               // advance the stroke
		shift the 32-sample rolling buffer, append the next floor sample
		v30++;
	}
	DisableEntityDrawing04_57F10(a1x);                     // ONE-SHOT
}
```
- **This carves a cave passage:** it LOWERS `mapHeightmap_11B4E0` (the cave floor) and RAISES `x_BYTE_14B4E0_second_heightmap` (the cave ceiling) across a disc that follows the stroke, radius eased from the start radius (`(hi>>4)<<8 + 512`) to the end radius (`(lo&0xF)<<8 + 512`). `sub_34B00` re-tiles/re-shades the carved box; `mapAngle bit3 (0x08)` = the cave-lit flag.
- **It IS cave-related.** The `(10,80..86)` band = the cave terrain generator (confirmed §4.1 headline of the 4.3 sweep). The (10,80)/(10,81) x3033 records are cave-tunnel authoring chains, resolvable to this carver.
- **`byte_0x46_70 = 2`** default from the ctor (both nibbles → radii ~2), overwritten by `sub_48930` with the packed par3 radii.
- The sibling **(10,80)=0x50 worker** (`sub_4FB80`→action 0x57 `sub_34520`, EF:25075) is a **1-tick despawn** — (10,80)=0x50 is purely the chain-authoring subtype, and its own (rarely-spawned) worker is inert; the real carve is (10,81)=0x51's action 0x58.

### 4.4 Settle
(10,81)=0x51 → in `ApplyEvents`, `v4 = 0x51` → `v4 >= 0x32` and `v4 > 0x33 && (v4 < 0x50 || ...)` — 0x51 is NOT `< 0x50`, and the second clause `v4 > 0x55 && v4 != 0x58` is false for 0x51, so the whole disable predicate is **false** → **TICKED** (EV:516-525). The carver runs its one-shot during the pass-3 settle.

---

## 5. Ordering & settle facts

### 5.1 GenerateEvents passes (EV:152-282) — DisId==-1 THINGs only, `ApplyEvents_498A0` between each
| pass | EV | subtypes | our painters |
|---|---|---|---|
| 1 | 162 | 0x52 | — |
| **2** | 176 | 0x09, 0x53, 0x54, 0x55, 0x0B, 0x0F, 0x1E, 0x1D, 0x20, **0x1F**, 0x33, 0x32, 0x58 | **(10,31) river**; also 0x1D waterpath marker |
| **3** | 209 | **0x51, 0x50** | **cave carvers** ((10,80)/(10,81)) |
| 4 | 226 | class 0x0E subtype 2 | (14,2) risers |
| **5** | 242 | 0x1B, **0x1C** | **(10,28) road** (paired with 0x1B) |
| 6 | 256/269 | class 0x2D buildings (split on `bldgprm.byte_2 & 0x10`) | — |

**Cross-pass ordering that matters:**
- **Rivers (pass 2) → cave carvers (pass 3) → roads (pass 5).** Roads are painted LAST, on top of whatever the river/cave passes left. A road stamped over an already-carved cave floor sees the lowered heightmap.
- `ApplyEvents` after each pass fully settles that pass's one-shot painters before the next pass's THINGs are prepared.
- The (10,50)=0x32 in **pass 2** is the river-endpoint self-destruct (§3.3). A (10,50) that is a beam-chain THING would also be pass 2, but note (10,50)=0x32 is in the `sub_49090` divert list (EV:330), so a stageTag≠0 (10,50) never spawns a runtime entity — it walks the chain via `sub_48880` (beam) instead. Whether a given (10,50) THING becomes a river endpoint or a beam depends on WHICH painter routed to it: `sub_487D0` (from a 0x1F river THING) or `sub_48880` (from a 0x32 beam THING).

### 5.2 PrepareEvents divert (EV:323-336)
```c
case 0x0A:
  switch (subtype) {
    case 0x1C: case 0x1D: case 0x1F: case 0x32: case 0x50:
      if (entity->stageTag_12)   // only 1c,1d,1f, 32 and 50
        sub_49090(terrain, entity);
      return;                    // NO ordinary entity spawned for these
```
So (10,28)=0x1C, (10,31)=0x1F, (10,80)=0x50 (and 0x1D, 0x32) with `stageTag_12 != 0` route to `sub_49090` and spawn NO marker entity. With `stageTag_12 == 0` they fall through to the ordinary spawn → 1-tick despawn (no-op). **Note (10,81)=0x51 is NOT in the divert list** — it is only ever spawned by `sub_48930` (the 0x50 painter), never authored directly as a chain THING; a (10,81) THING would take the generic spawn path (EV:353) and produce the cave worker directly if `isCaveLevel`.

### 5.3 `sub_49090` chain-walk (EV:5261) — recap (full body in m50 doc §1.2)
`par1_14` = prev-node link, `par2_16` = next-node link, `par3_18` = per-segment style. Climb `par1` to the head, walk `par2` forward, invoke the per-subtype stamper on each consecutive `(fromX,fromY)→(toX,toY)` pair, zero each node's `stageTag_12` (one-shot). Style remaps: **0x1F** → width enum (0/1/2/3 → 2/6/16/32); **0x50** → `par3&0xF | 16*(prev.par3&0xF)` (packed start/end radii). **0x1C/0x32 pass the style byte through unmodified** (and the road painter ignores it entirely).

---

## 6. Consolidated constants + Rust-port plan

### 6.1 Constants
| item | value | source |
|---|---|---|
| `shortestLenght_48370` | torus-shortest signed delta, mapSize 256 | EV:5753 |
| `sub_483A0` road spawn-point | `(x<<8, y<<8, 32*max(hmap[from],hmap[to]))` | EV:5763 |
| (10,27)=0x1B ctor | class/model/action 10/0x1B/0x1B; life=maxLife=**2** (width); dword_0x10_16=10 (run, overridden); subSpell=(z>>5)+48 | EF:36156-36164 |
| road walker override | action 27(-Y)/28(+Y)/29(X-run); dword_0x10_16 = run-length | EV:5429-5443, 5463-5478 |
| road walker writes | `hmap += 48`; `mapTerrainType = 8` (2×2, via sub_46180); `mapShading` reshade; `mapAngle |= 0x80` on borders | EF:24884-24888 etc. |
| road water guard | raise only if `mapTerrainType != 8 || sub_33F70(cell)` | EF:24884; TR:1744 |
| `sub_46180(cell,8)` | writes type 8 on 2×2 + slope-shade 3×3 + cave 2nd-hmap | EF:31007 |
| (10,31)=0x1F river painter | z=32*hmap; life=len>>8; yaw; **byte_0x46_70=width** (0/1/2/3→2/6/16/32) | EV:5569-5582 |
| (10,31) spawned model | **(10,50)=0x32**, action 0x36 = **self-destruct** (paints nothing) | EF:36494, 25732 |
| (10,80)=0x50 cave painter | packs par3 radii into byte_0x46_70; spawns **(10,81)=0x51** at FROM, axis_0x9A=TO | EV:5637-5748 |
| (10,81)=0x51 ctor | **cave-only**; action 0x58; byte_0x46_70=2; actSpeed 256 | EF:36329-36345 |
| (10,81) carve | LOWER `mapHeightmap` (floor, sub_570F0) + RAISE `x_BYTE_14B4E0` (ceiling); radius eased hi→lo nibble; step 85 | EF:25129-25260 |
| (10,80)=0x50 worker | action 0x57 `sub_34520` = 1-tick despawn (inert) | EF:25075 |
| settle TICK band (class 0xA) | model `0x1B..0x20` (≤0x20), and 0x32/0x33, and 0x50..0x55/0x58 | EV:493, 506-521 |
| settle DISABLE band | `v4 > 0x33 && (v4 < 0x50 || (v4 > 0x55 && v4 != 0x58))` | EV:508 |
| RNG law | `r = 9377*r + 9439` | universal |

### 6.2 Port-plan — which painter maps onto which existing primitive
The port (`crates/mgc-sim/src/mc2/terrain_paint.rs`) currently has:
- `Gen::mc2_paint_cell` = `sub_45DC0` (TR:1783) — the per-cell blend/retile applicator (codes ≥ 8).
- `Gen::mc2_retile_region` = `sub_462A0` (TR:1931) — region retile+shade.
- The 0x1D **waterpath** stamp (`sub_48690`) and a **generalized chain walk** (per the prompt).

Mapping:
1. **(10,28) road painter → NEW primitive.** The road walkers use **`sub_46180`** (EF:31007), NOT `sub_45DC0`/`sub_462A0` — a distinct "raise +48, write type 8 on a 2×2, slope-shade 3×3, lock mapAngle bit7" primitive. Port `sub_46180` as `Gen::mc2_ridge_stamp(cell, type)` and drive it from a Bresenham strip walker keyed by (action 27/28/29, width=2, run-length). The chain walk from `sub_49090` case 0x1C feeds `sub_48400` → strip segments. **Reuse the generalized chain walk; add the Bresenham line + the ridge stamp.** This is the highest-value port here (roads are the one fully-working painter).
2. **(10,31) river painter → DO NOT port as a carver yet.** The remc2 consumer (action 0x36) is a self-destruct stub (§3.4). Port only the marker recognition + width/z seeding; leave the carve OPEN pending a level+gameplay check (river geometry may be baked from the `river`/`lriver` header fields instead).
3. **(10,80)/(10,81) cave carver → maps onto `sub_570F0` (floor) + the `x_BYTE_14B4E0` second-heightmap (ceiling).** The port needs a **cave second-heightmap** and `sub_570F0`-equivalent floor writer; `sub_34B00` (the carved-box retile) is the reshade. Gate on cave levels. This is the terrain machinery behind the (10,80..86) cave band → schedule with the Phase 4.5 cave generator work, reusing the chain walk + a new tube-carve primitive.
4. The **generalized chain walk** already planned covers `sub_49090`'s par1/par2/par3 traversal for all five subtypes; each painter is a per-subtype callback exactly as in remc2.

---

## OPEN items
1. **(10,31) river carve is a stub in remc2 (§3.4).** The river painter sets life/yaw/width on a (10,50)=0x32 whose action self-destructs without reading them — either an unfinished remc2 port or the river is baked from the `river`/`lriver` header at level-gen. **Resolve by dumping a level with authored (10,31) THINGs (grep the x3033 records for class 10 subtype 31) and comparing recorded gameplay river placement to the heightmap the header produces.** Do not implement a river carver on this trace alone.
2. **(10,80)/(10,81) THING density + par3 radii.** Confirm the mgc-import x3033 dump emits (10,80)/(10,81) chains on cave levels and that par3 carries the 4-bit radii; the carver's radius formula `((nibble)<<8)+512` and the `sub_7277A_radix_3d` falloff need a golden before porting the exact tube profile.
3. **`sub_34B00`** (cave carved-box retile, called by `sub_34540`) and **`sub_570F0`** (floor writer with dirty flags) bodies not transcribed here — pull when porting the cave carver (both are the terrain-write primitives the carve depends on).
4. **Road `sub_46180` slope-shading exact table** (`unk_D4A30`-free path — it computes shading inline from neighbor height diffs, EF:31047-31060) — the Day vs. non-Day inversion (`32-v+32`) and the cave second-heightmap coupling need a shading golden.
5. **Does mgc-import currently emit (10,28)/(10,31)/(10,80) chain THINGs with par1/par2/par3 intact?** The importer must carry the linked-list links and the style bytes; verify against level-000/001 x3033 dumps before wiring the chain walk.
6. **`sub_483A0`'s `predictedAxis_EB398ar` global** is shared across the two (10,27) spawns per step — confirm the second spawn's Y (`v20+v28+posY2`) is intended as the X-run walker's start (it is, per the code) when porting the staircase geometry.
