# CLASS-10 Middle-Band Models 6, 9, 11, 28, 31 — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Trace date 2026-07-10. Format model: `docs/traces/mc2-class10-m59-m60.md` (§0 dispatch architecture reused). Shared helpers (`NewEvent_4A050`, `AddEventToMap_57D70`, `CopyMaxLifeToLife_49A20`, `DisableEntityDrawing04_57F10`, `SetEntityIndexAndRot_49CD0`, `SetHalfSpeedEntity_49DA0`, `SetEntityShiftRot_49EA0`, `sub_58DA0` stage-binder, RNG law `r = 9377*r + 9439`) are documented in `mc2-class10-m29-m5-m13.md` §0 and not re-derived.

**Headline identification (read first):**

1. **(10,6) is the STANDING GROUND FIRE.** `NewAdd0A06_4E5F0` (EF:35458), action 6 `sub_31760` (EF:23099). It is a real, damaging, self-sustaining flame: sprite 228, 240-tick life, a **dynamic light source** (`AddEvent2_847D0`, radius 80 — Night/Cave only), and **every tick it deals area heat damage on channel 0** (`sub_11400(a1x, 0, subSpellIndex=50)`, EF:23111/23152) and randomly puffs **(10,14)** smoke. It grows in (life 12..6) then shrinks, and self-extinguishes over water. **The current Rust `(10,0)` stand-in in `scenery.rs` is wrong on damage, light, life-shape and sprite — this is the real creator to port.** Runtime-spawned by burning trees (EF:62421) and falling objects landing on dry ground (EF:23792).

2. **(10,9) is the APOCALYPSE / RAISE-LAND CLIMAX effect** — a giant `sin`-domed heightmap deformer. `NewAdd0A09_4E760` (EF:35513), action 9 `sub_31940` (EF:23193). Over its life it raises `mapHeightmap_11B4E0` into a cosine dome, rewrites `mapShading`/`mapAngle`, deals area damage (`sub_116A0`), fires an earthquake event (`sub_6D8B0(...,0x12,...)`), plays sound 10 (and 63 in the `byte_0x36E03` variant), and at `life==3` births a **(10,18)** (or **(10,91)** in the `byte_0x36E03` variant). **This is the model whose level-load consumes `par1_14` as a subspell selector** (§4.2) — confirmed. Its sole runtime spawner is the level-ending state machine `sub_21030` case 0xF (EF:12864, after `KillAllCreatures`), which sets `byte_0x36E03 = 1`.

3. **(10,11) (decimal 0x0B) is a persistent GROUND-FIRE-SPRAY SPELL entity.** Its creator `NewAdd0A0B_4E840` (EF:35553) **remaps the model to 0x13 (19) and action to 0x13 (19)** — so a `(10,11)` THING becomes an entity of model 19. Action 0x13 `sub_32F40` (EF:24095) walks a `AddE7EE0x_10080` splat template each tick and, on odd life-ticks, sprays rings of **(10,14)** smoke; it applies area damage (`sub_10C80`, subSpell 200) and clears the `word_0x33` singleton latch on death. Level-load also consumes `par1_14` as a subspell selector (0x0B is in the pass-2 list). Runtime spawner: `sub_344A0` (EF:25064, a travelling fire-drop) and EF:23716.

4. **(10,28) (0x1C) and (10,31) (0x1F) are LOAD-TIME TERRAIN-AUTHORING MARKERS, not runtime effects.** Their action handlers (`sub_34330` EF:24989 / `sub_34480` EF:25046) are bare one-tick `DisableEntityDrawing04_57F10` — identical to the (10,29) marker family. Their real work happens **during `GenerateEvents_49290`**: when a marker THING has `stageTag_12 != 0`, `PrepareEvents_49540` diverts it to **`sub_49090`** (EV:5261), which chains linked markers via `par1_14`/`par2_16` and paints terrain. **(10,28)→`sub_48400`** draws Bresenham **road/path lines** spawning **(10,27)** segment-walkers; **(10,31)→`sub_487D0`** drops **(10,50)=0x32** river/terrain-stroke entities with a `par3_18`-remapped width. This is the "special par writes" the task flagged.

---

## 0. Dispatch rows (this band)

Registry `str_D4C48ar` row 10 at EF:2071 binds `dword_10 = strA0` (action table, keyed by **actionIndex**) and `dword_14 = strA1` (creator table, keyed by **model/subtype**). Creator dispatch `IfSubtypeCallCreatingManaSphere_4A190` (EV:5186); action dispatch via `UpdateEntities_57730` → `pre_sub_4A190_0x6E8E` switch (EV:610).

| model (THING subtype) | creator addr | creator fn | EV creator case | ctor-set action | action addr | action fn | EV action case |
|---|---|---|---|---|---|---|---|
| **6** | 0x22F5F0 | `NewAdd0A06_4E5F0` (EF:35458) | EV:4535 | **6** | 0x212760 | `sub_31760` (EF:23099) | EV:2294 |
| **9** | 0x22F760 | `NewAdd0A09_4E760` (EF:35513) | EV:4548 | **9** | 0x212940 | `sub_31940` (EF:23193) | EV:2315 |
| **11 (0x0B)** | 0x22F840 | `NewAdd0A0B_4E840` (EF:35553) | EV:4557 | **0x13** (sets model=0x13) | 0x213F40 | `sub_32F40` (EF:24095) | EV:2389 |
| **28 (0x1C)** | 0x230800 | `sub_4F800` (EF:36170) | EV:4662 | **0x1E** | 0x215330 | `sub_34330` (EF:24989) | EV:2481 |
| **31 (0x1F)** | 0x230AC0 | `sub_4FAC0` (EF:36311) | EV:4694 | **0x21** | 0x215480 | `sub_34480` (EF:25046) | EV:2509 |

strA1 rows: EF:1709 (m6), EF:1712 (m9=row 0x09), EF:1714 (m11=row 0x0B), EF:1732 (m28=row 0x1C), EF:1734 (m31=row 0x1F). strA0 rows: EF:1607 (a6), EF:1610 (a9), EF:1619 (a0x13), EF:1631 (a0x1E), EF:1634 (a0x21).

**Numbering trap:** (10,11)'s creator sets `model_0x40_64 = 19 (0x13)` and `actionIndex = 19 (0x13)` — a `(10,11)` THING produces an entity that identifies as **model 19**. All model-based cross-refs for this effect key on **0x13/19**, not 0x0B. Runtime code (EF:23962) tracks it as the `word_0x33` singleton (also referred to via the (10,19) creator directly). Do not port (10,11) as a distinct model — it IS (10,19).

---

## 1. Model 6 (0x06) — standing ground fire

### 1.1 Creator `NewAdd0A06_4E5F0` (EF:35458), verbatim
```c
type_entity_0x6E8E* NewAdd0A06_4E5F0(axis_3d* position)//22f5f0
{
	type_entity_0x6E8E* event = NewEvent_4A050();
	if (event)
	{
		event->actionIndex_0x45_69 = 6;
		event->class_0x3F_63 = 10;
		event->model_0x40_64 = 6;
		event->subSpellIndex_0x2A_42 = 50;
		event->maxLife_0x4 = 240;
		event->word_0x2C_44 = 0;
		event->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;   // clear byte[0] bit3 (0x08=targetable) + byte[2] bit1
		event->struct_byte_0xc_12_15.byte[2] |= 2u;         // reclaimable-effect flag
		AddEventToMap_57D70(event, position);
		event->position_0x4C_76.z = getTerrainAlt_10C40(position);  // snapped to terrain
		CopyMaxLifeToLife_49A20(event);
		SetEntityIndexAndRot_49CD0(event, 228);             // sprite/particle row 228
		SetEntityShiftRot_49EA0(event, 272, 1536);          // draw shift 272, rot 0x600
		event->dword_0x10_16 = 0;
		AddEvent2_847D0(event, 80, 11, 1);                  // dynamic light: radius 80, params (11,1)
	}
	return event;
}
```
**Creator facts:** RNG draws **0**. `maxLife = 240` ticks. `subSpellIndex = 50` = the per-tick area-damage amount. **Sprite 228** with rot/shift. **byte[0] bit3 cleared → NOT itself targetable** (fire cannot be attacked). Map-registered. `AddEvent2_847D0(event,80,11,1)` (EF:47172): if `m_wDynamicLighting` and map is Night/Cave and < 50 lights active, appends a light source at radius 80 into `D41A0_0.str_0x3664C[]` and sets **byte[2] bit3 (0x08 = has-light)**. Day maps: no light entry.

### 1.2 Action 6 `sub_31760` (EF:23099), verbatim
```c
void sub_31760(type_entity_0x6E8E* a1x)//212760
{
	if (a1x->life_0x8-- < 0)
	{
		DisableEntityDrawing04_57F10(a1x);
		if (!(a1x->struct_byte_0xc_12_15.byte[2] & 1))
			sub_11400(a1x, 0, a1x->subSpellIndex_0x2A_42);   // one last damage pulse
		return;
	}
	sub_5C870(a1x);                                          // record player's min-distance to this fire
	if (a1x->life_0x8 < 12)
	{
		if (a1x->dword_0x10_16 > 0)
		{
			a1x->dword_0x10_16--;
			v4 = a1x->struct_byte_0xc_12_15.byte[0];
			a1x->word_0x5A_90--;                            // SHRINK sprite (die-down phase)
			if (v4 >= 0)                                     // byte[0] bit7 clear
			{
				a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
				if (!(a1x->rand_0x14_20 % 7u))              // ~1/7 chance
				{
					v5x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position_0x4C_76, 10, 14); // smoke puff
					if (v5x)
					{
						v6 = a1x->id_0x1A_26;
						v5x->dword_0x10_16 = 100;
						v7 = v5x->word_0x5A_90;
						v5x->life_0x8 = 15;
						v5x->id_0x1A_26 = v6;
						v5x->word_0x5A_90 = v7 + 2;
					}
				}
			}
		}
	}
	else if (a1x->dword_0x10_16 <= 6)                        // life >= 12: RAMP-UP phase
	{
		v2 = a1x->dword_0x10_16 + 1;
		a1x->word_0x5A_90++;                                // GROW sprite
		a1x->dword_0x10_16 = v2;
	}
	a1x->position_0x4C_76.z = a1x->word_0x2C_44 + getTerrainAlt_10C40(&a1x->position_0x4C_76); // z = terrain + word_0x2C_44 offset
	v8 = sub_104D0_terrain_tile_is_water(&a1x->position_0x4C_76);
	if (v8 == 1)
		DisableEntityDrawing04_57F10(a1x);                  // extinguished by water
	if (!(a1x->struct_byte_0xc_12_15.byte[2] & 1))
		sub_11400(a1x, 0, a1x->subSpellIndex_0x2A_42);      // AREA DAMAGE, channel 0, every tick
}
```
Per-tick facts:
- **Damage OUTPUT every tick** via `sub_11400(a1x, 0, 50)` (gated only on `!(byte[2]&1)` — a "damage-suppressed" flag never set by the ctor, so it always damages). See §5.1 for what `sub_11400` does.
- **Sprite phase machine on `word_0x5A_90`** (draw scale): while `life >= 12` and `dword_0x10_16 <= 6`, `word_0x5A_90++` and `dword_0x10_16++` (ramp up 6 steps); while `life < 12` and `dword_0x10_16 > 0`, `word_0x5A_90--` and `dword_0x10_16--` (ramp down) plus a ~1/7 puff of **(10,14)** smoke (life forced 15, scale +2, id inherited).
- **z = `word_0x2C_44` + terrain alt** each tick (ctor sets `word_0x2C_44=0`; runtime spawners can lift it — the tree flame sets `word_0x2C_44 = (3*fov)>>2`, EF:62428).
- **Water extinguish:** `sub_104D0_terrain_tile_is_water == 1` → despawn.
- `sub_5C870` (EF:43602): writes the player's minimum 3D distance to this fire into `player->dword_0xA4_164x->dword_0x19A_410` — a "nearest hazard" tracker (used for HUD/audio proximity). No RNG in this path except the ~1/7 smoke roll.

### 1.3 Damage/targetability
- **Deals** channel-0 area damage (`sub_11400`, subSpell 50) — see §5.1. **Never reads** its own mailbox `str_0x5E_94` and has byte[0] bit3 cleared → **not targetable/killable by anything**; it only dies of old age (240 ticks) or water. Immune-by-omission.

### 1.4 Runtime spawners of (10,6)
| EF | enclosing | context |
|---|---|---|
| 23792 | `sub_32600` (0x213600, class-10 action 0x10 falling-object mover) | object lands on **dry** terrain: guarded by `!sub_10B70(&pos,10,6)` (no fire already there); spawns (10,6), `life=30`, `subSpellIndex = 3*50 = 150`, inherits id (EF:23790-23800) |
| 62421 | `AddTree02_00_64E20`-family (class-2 tree burn) | lethal burn hit → spawns (10,6) flame at the tree, id from attacker, `word_0x2C_44 = (3*fov)>>2`, z lifted; re-seeds burn (EF:62421-62456). **This is the flame the current Rust `mc2_tree_tick` approximates with `(10,0)`.** |
| load | `sub_4A310` case 0xA `v4=6` → `!=9` branch → `sub_58DA0` only (EF:33051-33054) | (10,6) THINGs consume **no par fields**; stage-bind only |

### 1.5 Rust port note (closes the `scenery.rs` APPROX)
Replace the `(10,0)` stand-in in `mc2_tree_tick`/`mc2_falling_tick` with a real `mc2_spawn_fire6`: action 6, sprite 228, `SetEntityShiftRot(272,1536)`, maxLife 240, subSpell 50, byte3-clear+byte1-set(byte2), map-registered, z=terrain, dynamic-light (Night/Cave only). Tick = §1.2: damage `sub_11400(ch0, 50)` each tick (into the same ch0 mailbox our combat column writes), the 6-step grow/shrink `word_0x5A_90` machine, the ~1/7 (10,14) puff, water despawn. Runtime spawners already set `life=30`+`subSpell=150` (dry-land) or `word_0x2C_44` lift (tree).

---

## 2. Model 9 (0x09) — raise-land / apocalypse dome

### 2.1 Creator `NewAdd0A09_4E760` (EF:35513), verbatim
```c
type_entity_0x6E8E* NewAdd0A09_4E760(axis_3d* position)//22f760
{
	type_entity_0x6E8E* event = NewEvent_4A050();
	if (event)
	{
		event->actionIndex_0x45_69 = 9;
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 9;
		event->maxLife_0x4 = 11;
		event->life_0x8 = 17;                               // life > maxLife on purpose
		event->position_0x4C_76 = *position;
		event->subSpellIndex_0x2A_42 = 2000;                // area-damage amount
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;      // clear bit3 (targetable)
		SetEntityShiftRot_49EA0(event, 7, 0x4000);          // radius seed pitch=7, rot 0x4000
		D41A0_0.byte_0x36E03 = 0;                           // clear apocalypse-variant flag
	}
	return event;
}
```
**Creator facts:** RNG **0**. `maxLife=11`, `life=17` (life deliberately exceeds maxLife; `maxLife` is later reset per subspell during GenerateEvents). subSpell 2000. Not map-registered (no `AddEventToMap`). `array_0x52_82.pitch = 7` (the dome radius accumulator, grown in the tick). **Not targetable.** Clears the global `byte_0x36E03` (apocalypse variant selector).

### 2.2 Action 9 `sub_31940` (EF:23193) — the dome deformer, structural summary
The full body is ~180 lines (EF:23193-23434); here is its verbatim spine. It is a three-phase machine on `byte_0x46_70` (0 = init, 1 = active grow, 2 = finalize).

- **byte_0x46_70 == 0 (init, EF:23245-23262):** compute tile center; `SetEntityShiftRot_49EA0(a1x, (maxLife|1)<<8, 0x4000)`; sample base terrain via `sub_48E60`; `word_0x2C_44 = 2*maxLife + 100` (dome height), clamped so `z + word_0x2C_44 <= 255`; set `byte_0x46_70 = 1`.
- **byte_0x46_70 == 2 (finalize, EF:23263-23319):** flatten the summit: write `mapHeightmap_11B4E0[...] = (z + word_0x2C_44 - 24)` over the radius, stamp a 2×2 cap at `-16` with `mapShading = 63` (Day) / `1` (else), then `DisableEntityDrawing04_57F10` (despawn). `return`.
- **active grow (else, EF:23323-23430):** `life--`; when `life <= 0` set `byte_0x46_70 = 2`. Otherwise walk the disc of radius `array_0x52_82.pitch`:
```c
v11 = Maths::EuclideanDistXYZ_58490(&a1x->position_0x4C_76, &v31x);
if (v11 < v6) {                                          // inside radius
	v12 = (a1x->word_0x2C_44
		* ((0x10000 + Maths::sin_DB750[0x200 + (v11 << 10) / v6]) >> 1) >> 16)  // COSINE DOME profile
		+ a1x->position_0x4C_76.z;
	v42 = mapHeightmap_11B4E0[v9x.word];
	if (v12 > v42) v14 = (v12 - v42) / a1x->life_0x8 + v42; // ease toward target over remaining life
	sub_570F0(x, y, v14, 0, dist<=v34, 1);              // write heightmap + dirty flags
	// cave: also raise x_BYTE_14B4E0_second_heightmap
}
```
  Then the **combat/audio pulse** (EF:23390-23430):
```c
if (!D41A0_0.byte_0x36E03)
	v39 = sub_116A0(a1x, 0, a1x->subSpellIndex_0x2A_42);  // AREA DAMAGE (returns hit count)
if (v39)
	sub_6D8B0(a1x->id_0x1A_26, 0x12u, v39);              // earthquake / shake event, kind 0x12
PrepareEventSound_6E450(a1x, -1, 10);                    // sound 10 (rumble) every tick
if (D41A0_0.byte_0x36E03 && !(a1x->byte_0x3E_62 & 3))
	PrepareEventSound_6E450(a1x, -1, 63);                // apocalypse variant: sound 63 every 4th tick
if (a1x->life_0x8 == 3) {                                 // at life==3: cap + spawn child
	// stamp a 2x2 summit cap (heightmap -16, shading 63/1) as in finalize
	predictedAxis.z = getTerrainAlt(&predictedAxis);
	v1x = D41A0_0.byte_0x36E03 ? IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 91)  // apocalypse child
	                           : IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 18); // normal child
	if (v1x) v1x->id_0x1A_26 = a1x->id_0x1A_26;
}
```

Per-tick facts:
- **Terrain write:** raises `mapHeightmap_11B4E0` into a **cosine dome** (`sin_DB750[0x200 + ...]`) of height `word_0x2C_44` (≈ `2*maxLife+100`) and radius `array_0x52_82.pitch`, easing each cell toward target over remaining `life` ticks. Also `mapShading_12B4E0`, `mapAngle_13B4E0` (cave), and `x_BYTE_14B4E0_second_heightmap` (cave ceiling). Uses `sub_570F0` (heightmap+dirty), and direct writes.
- **Damage:** `sub_116A0(a1x, 0, subSpell=2000)` every tick (unless `byte_0x36E03` apocalypse variant, which suppresses damage). If it hits, fires `sub_6D8B0(id, 0x12, hitcount)` — an earthquake/knockback event, kind 0x12.
- **Sounds:** 10 every tick; +63 every 4th tick in the apocalypse variant.
- **Child:** at `life==3` spawns **(10,18)** (normal) or **(10,91)** (apocalypse) at the dome center, id inherited.
- **Not map-registered**, byte[0] bit3 clear → not targetable.

### 2.3 Level-load: (10,9) consumes `par1_14` as a SUBSPELL selector (the "special par writes")
Two independent paths both read `par1_14`:

**(a) `sub_4A310` case 0xA** (EF:33033) — the generic THING post-init. For `v4 = model = 9`: it falls through all early returns (`<0xB`, not `<4`, not `<=4`, `==9` so skips the `!=9` return) into the bottom subspell block (EF:33163-33170):
```c
v3x->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(model)].subspell[entity->par1_14].subSpellIndex_2;
...
if (v8 != 0x9) { sub_58DA0(...); return; }
v3x->maxLife_0x4 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(model)].subspell[entity->par1_14].life_0x1A;
sub_58DA0(entity, v3x); return;
```

**(b) `PrepareEvents_49540`** (EV:387) — the GenerateEvents pass (§4). For subtype 0x09:
```c
case 0x09: case 0x0B: case 0x0F:
	event->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(subtype)].subspell[par1_14].subSpellIndex_2;
	if (subtype == 0x09)  event->maxLife_0x4 = SPELLS[...].subspell[par1_14].life_0x1A;   // model 9 sets maxLife
	else                  event->life_0x8  = SPELLS[...].subspell[par1_14].life_0x1A;     // 0x0B/0x0F set life
```
So a level-authored (10,9) reads `par1_14` → picks a subspell row → overrides `subSpellIndex` and **`maxLife`** from the spell table `SPELLS_BEGIN_BUFFER_str`. (10,11)=0x0B and (10,15)=0x0F share this, but write **`life_0x8`** instead of maxLife.

### 2.4 Runtime spawner
Sole runtime spawn: **`sub_21030` case 0xF** (EF:12864), the level-ending state machine (`byte_0x46_70` phases; case 0xF runs `KillAllCreatures_1B5F0` each tick, and on its countdown expiry spawns (10,9), forces `life=32, maxLife=11`, inherits id, and sets **`D41A0_0.byte_0x36E03 = 1`** → selecting the apocalypse variant: no damage, sound 63, (10,91) child). Also `word_0x36548 = 0`. This is the endgame "the land rises / world ends" cinematic.

---

## 3. Model 11 (0x0B → entity model 0x13/19) — ground-fire-spray singleton

### 3.1 Creator `NewAdd0A0B_4E840` (EF:35553), verbatim
```c
type_entity_0x6E8E* NewAdd0A0B_4E840(axis_3d* a1x)//22f840
{
	type_entity_0x6E8E* v1x = NewEvent_4A050();
	if (v1x)
	{
		v1x->actionIndex_0x45_69 = 19;      // 0x13
		v1x->class_0x3F_63 = 10;
		v1x->model_0x40_64 = 19;            // 0x13  — NOTE: model != subtype 0x0B
		v1x->subSpellIndex_0x2A_42 = 200;
		v1x->maxLife_0x4 = 240;
		v1x->struct_byte_0xc_12_15.dword &= 0xFFFDFFF7;   // clear bit3 + byte2 bit1
		v1x->struct_byte_0xc_12_15.byte[2] |= 2u;         // reclaimable
		AddEventToMap_57D70(v1x, a1x);
		v1x->struct_byte_0xc_12_15.byte[0] |= 1u;         // set bit0
		CopyMaxLifeToLife_49A20(v1x);
		SetEntityIndexAndRot_49CD0(v1x, 228);             // sprite 228 (same as fire)
		SetEntityShiftRot_49EA0(v1x, 512, 512);           // shift/rot 512,512
	}
	return v1x;
}
```
**Creator facts:** RNG **0**. `maxLife=240`, subSpell 200, sprite 228, byte[0] bit0 set / bit3 clear (not targetable). Map-registered. **model & action forced to 0x13/19** — the entity is (10,19), tracked at runtime as the `word_0x33` singleton (EF:23962).

### 3.2 Action 0x13 `sub_32F40` (EF:24095), verbatim
```c
void sub_32F40(type_entity_0x6E8E* a1x)//213f40
{
	v1 = a1x->life_0x8; a1x->life_0x8 = v1 - 1;
	if (v1 >= 0)
	{
		a1x->dword_0x10_16 = 0;
		v2 = AddE7EE0x_10080(0, a1x->dword_0x10_16);        // splat/area template iterator
		if (v2) {
			while (sub_10130(v2, &v9, &v8) == 1) {          // walk template cells
				a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
				if (2 * ((a1x->rand_0x14_20 % 0x9Du) / 79) - 1 > 0) {   // ~ chance gate
					a1x->rand_0x14_20 = 9377 * ...;
					v5x.x = a1x->rand_0x14_20 % 0x81u + a1x->position.x - 96 + 192*v9 - 64;  // jittered cell pos
					a1x->rand_0x14_20 = 9377 * ...;
					v5x.y = a1x->position.y - 96 + 192*v8 + a1x->rand_0x14_20 % 0x81u - 64;
					v5x.z = a1x->position.z;
					if (a1x->life_0x8 & 1) {                // only on odd life ticks
						LOWORD(v10) = (a1x->life_0x8 / 2 & 1) << 8;
						while ((uint16_t)v10 < 0x800u) {    // ring of smoke, yaw step 0x200
							v3x = IfSubtypeCallCreatingManaSphere_4A190(&v5x, 10, 14);  // (10,14) smoke
							if (v3x) { v3x->id_0x1A_26 = a1x->id_0x1A_26; v3x->yaw_0x1C_28 = v10; }
							BYTE1(v10) += 2;
						}
					}
				}
			}
			ResetEvent08_10100(v2);
		}
		a1x->position_0x4C_76.z = getTerrainAlt_10C40(&a1x->position_0x4C_76);  // snap to terrain
	}
	else {
		DisableEntityDrawing04_57F10(a1x);
		D41A0_0.word_0x33 = 0;                              // release the singleton latch
	}
	sub_10C80(a1x, 0, a1x->subSpellIndex_0x2A_42);          // AREA DAMAGE channel 0, subSpell 200, EVERY tick
}
```
Per-tick facts:
- **Area damage** `sub_10C80(a1x, 0, 200)` every tick (channel 0), continuing even during the final despawn tick.
- **Smoke spray:** on odd `life` ticks, over the `AddE7EE0x_10080` template cells (a fixed splat shape), a probabilistic gate spawns rings of **(10,14)** smoke at jittered positions (RNG: 1 gate roll + up to 2 position rolls per cell). Yaw ring step `0x200`, up to `0x800`.
- **z snapped to terrain** each tick.
- On death: releases `word_0x33` (the "one active fire-spray at a time" singleton, EF:23962-23964 shows a prior instance being disabled when a new one registers).

### 3.3 Damage/targetability
- **Deals** channel-0 area damage (`sub_10C80`); **never reads** its own mailbox; byte[0] bit3 clear → not targetable. Immune-by-omission.

### 3.4 Level-load & runtime spawners
- **Load:** `PrepareEvents_49540` case 0x0B (§4.2 / §2.3b): consumes `par1_14` → subspell row → overrides `subSpellIndex` and **`life_0x8`** from `SPELLS_BEGIN_BUFFER_str`. Generated in **GenerateEvents pass 2** (0x0B is in the pass-2 subtype list, EV:181). `sub_4A310` case 0xA path: model is 0x13 at that point (`v4=0x13` → `>0x11 && !=0x16` → `sub_58DA0` only), so the generic path does not re-derive the subspell — the GenerateEvents path is the authoritative one for authored (10,11).
- **Runtime:** EF:23716 (a class-10 tick that drops (10,11) at a landing point, `life=10`) and EF:25064 `sub_344A0` (a travelling fire-drop that emits (10,11) each tick with `fov`/id copied and `life = byte_0x46_70`). `sub_344A0` verbatim (EF:25046):
```c
void sub_344A0(type_entity_0x6E8E* a1x)//2154a0
{
	v1 = a1x->life_0x8; a1x->life_0x8 = v1 - 1;
	if (v1 < 0 || sub_104A0(&a1x->position) & 1) { DisableEntityDrawing04_57F10(a1x); return; }
	v3x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position, 10, 11);
	if (v3x) { v3x->array_0x52_82.fov = a1x->array_0x52_82.fov;
	           v3x->id_0x1A_26 = a1x->id_0x1A_26;
	           v3x->life_0x8 = a1x->byte_0x46_70; }
	MoveEntity_57FA0(&a1x->position, a1x->yaw_0x1C_28, 0, a1x->actSpeed_0x82_130);  // travel
}
```

---

## 4. Models 28 (0x1C) & 31 (0x1F) — load-time terrain-authoring markers

### 4.1 Creators (near-identical marker ctors)
```c
type_entity_0x6E8E* sub_4F800(axis_3d* position)//230800  (m28, EF:36170)
{
	event->maxLife_0x4 = 0;  event->actionIndex_0x45_69 = 0x1E;  event->class_0x3F_63 = 0xA;
	event->model_0x40_64 = 0x1C;  event->position_0x4C_76 = *position;
	event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;   // clear bit3
	AddEventToMap_57D70(event, position);  CopyMaxLifeToLife_49A20(event);   // life = 0
}
type_entity_0x6E8E* sub_4FAC0(axis_3d* position)//230ac0  (m31, EF:36311)
{
	event->maxLife_0x4 = 0;  event->actionIndex_0x45_69 = 0x21;  event->class_0x3F_63 = 0xA;
	event->model_0x40_64 = 0x1F;  event->position_0x4C_76 = *position;
	event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;
	AddEventToMap_57D70(event, position);  CopyMaxLifeToLife_49A20(event);   // life = 0
}
```
RNG 0, life 0, not targetable, map-registered. Actions are bare one-tick despawns:
```c
void sub_34330(type_entity_0x6E8E* a1x)//215330  (m28, EF:24989) { DisableEntityDrawing04_57F10(a1x); }
void sub_34480(type_entity_0x6E8E* a1x)//215480  (m31, EF:25046) { DisableEntityDrawing04_57F10(a1x); }
```
These siblings match the (10,29)/(10,30) one-tick marker family (`mc2-class10-m29-m5-m13.md` §3). **The spawned entity does nothing** — the work is all at load time.

### 4.2 The real behavior: `GenerateEvents_49290` → `PrepareEvents_49540` → `sub_49090`
`GenerateEvents_49290` (EV:152) is the **multi-pass level-load event generator**. It only processes THINGs with `DisId == -1` (spawn-at-start), in ordered passes with `ApplyEvents_498A0()` between them. The subtype pass lists (verbatim, EV:161-282):

| pass | EV | subtypes generated |
|---|---|---|
| 1 | 161 | 0x52 only |
| **2** | 178 | 0x09, 0x53, 0x54, 0x55, **0x0B**, 0x0F, 0x1E, 0x1D, 0x20, **0x1F**, 0x33, 0x32, 0x58 |
| 3 | 218 | 0x51, 0x50 |
| 4 | 235 (class 0x0E) | (14,2) risers |
| **5** | 248 | 0x1B, **0x1C** |
| 6 | 269 (class 0x2D bit set) / EV:269 else | building subtypes 0x2D |

So **(10,9), (10,11)=0x0B, (10,31)=0x1F are generated in pass 2**; **(10,28)=0x1C is generated in pass 5** (paired with 0x1B). The ordering matters: roads (pass 5) are painted after the pass-2 spell/marker terrain and after rivers (0x32 in pass 2). (10,6) is **not** in any special pass — (10,6) THINGs load via the generic `sub_4A1E0(0)` sweep.

`PrepareEvents_49540` case 0x0A (EV:319-405) diverts the markers **before** creating a normal entity:
```c
case 0x1C: case 0x1D: case 0x1F: case 0x32: case 0x50:
	if (entity->stageTag_12)          // only when stageTag != 0
		sub_49090(terrain, entity);
	return;                           // NO ordinary entity is spawned for these
```
So **(10,28) and (10,31) with `stageTag_12 != 0` never spawn the one-tick marker entity at all** — they route straight to `sub_49090`. (With `stageTag_12 == 0` they fall through to the ordinary spawn → one-tick despawn, effectively a no-op.)

`sub_49090` (EV:5261) picks a terrain painter by subtype:
```c
case 0x1C: functionPointer = &sub_48400;   // (10,28) → road/path line
case 0x1D: functionPointer = &sub_48690;   // (10,29 sibling)
case 0x1F: functionPointer = &sub_487D0;   // (10,31) → river/stroke
case 0x32: functionPointer = &sub_48880;   // (10,50)
case 0x50: functionPointer = &sub_48930;
```
It then **walks a linked chain**: `par1_14` points to the chain start, `par2_16` to the next node. For each consecutive pair it calls `functionPointer(x1, y1, x2, y2, param)` and zeroes `stageTag_12` (one-shot). For **0x1F** it remaps `par3_18` → param: **0→2, 1→6, 2→16, 3→32** (a width/scale enum). For 0x50 it packs `par3_18 & 0xF | 16*(v8 & 0xF)`.

### 4.3 The painters
- **(10,28) → `sub_48400`** (EV:5365): Bresenham line between the two endpoints; along the segments it spawns **(10,27)** entities with `actionIndex` 27/28/29 and a `dword_0x10_16` run-length, e.g.:
  ```c
  v22x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 27);
  v22x->actionIndex_0x45_69 = 28;  v22x->dword_0x10_16 = v23;   // segment walker
  ```
  So (10,28) authors a **road/path** as a chain of (10,27) segment-drawing effects.
- **(10,31) → `sub_487D0`** (EV:5558): at the endpoint tile it spawns a **(10,50)=0x32** entity at `z = 32 * mapHeightmap[...]` and sets `byte_0x46_70 = a5` (the remapped width 2/6/16/32). So (10,31) authors a **river / terrain stroke** as a (10,50) generator, with `par3_18` selecting the width.

### 4.4 Damage/targetability
Markers: life 0, byte[0] bit3 clear, one-tick despawn, never read/write damage. **The generated (10,27)/(10,50) painters are separate models (out of scope; cross-referenced).** No damage in this family.

### 4.5 Generic `sub_4A310` path (for THINGs that reach it — stageTag==0 or non-DisId=-1)
`case 0xA`, `v4 = 0x1C` or `0x1F`: both hit `v4 > 0x11 && v4 != 0x16` → **`sub_58DA0` only, no par consumption** (EF:33068-33072). Confirms: outside the GenerateEvents special pass, these are inert stage-bind markers.

---

## 5. Shared damage/terrain helpers (verbatim anchors)

### 5.1 `sub_11400` (EF:4208) — void area-damage-mailbox writer (used by (10,6))
Channel `a2=0`: (1) walks the **player list** `dword_38519` writing `str_0x5E_94` on model-2 (players/props?) entities with a different id via `sub_106C0`; (2) walks map cells in the radius `array_0x52_82.pitch` and, for each occupant with `(1<<a2) & byte_0x38_56` set (damage-channel enrolled) **and** `byte[0] & 8` (targetable) **and** the xtype/xsubtype filter passing, writes `str_0x5E_94.dword_0x5E_94 += a3` (or `= a3` if fresh) and `word_0x62_98 = attacker id`. Class-2 model-0 victims (trees) take **a3/10** (reduced). Returns void — fire does not count hits.

### 5.2 `sub_116A0` (EF:4305) & `sub_10C80` (EF:3953) — int-returning area-damage variants
Same mailbox-write pattern but **return a hit count** (used by (10,9) to gate `sub_6D8B0` earthquake and by (10,11) each tick). `sub_10C80` additionally handles cross-channel/knockback bookkeeping (larger body). Both write channel-0 `str_0x5E_94` on eligible targetable victims — identical mailbox contract to our ch0 combat column.

### 5.3 `sub_6D8B0(id, kind, amount)` (EF, def ~) — event/shake dispatcher
Called by (10,9) as `sub_6D8B0(id, 0x12, hitcount)` (earthquake/knock kind 0x12). Gated on `!(setting_38545 & 4)`. Not a damage primitive — a queued gameplay event.

### 5.4 `AddEvent2_847D0(event, radius, a3, a4)` (EF:47172) — dynamic light registration
Only active when `m_wDynamicLighting` and map is Night/Cave and `< 50` lights. Appends `{byte_0=1, byte_1=a4, byte_2=radius, byte_3=a3, axis3d = event.pos, pointer = event}` into `D41A0_0.str_0x3664C[]`, sets **byte[2] bit3 (0x08)** on the entity, bumps `word_0x36DFA`. Freed by `sub_84880` when the entity dies. (10,6) registers `(80, 11, 1)`.

---

## 6. Constants table (consolidated)

| item | value | source |
|---|---|---|
| **(10,6)** ctor: class/model/action | 10 / 6 / 6 | EF:35462-35464 |
| (10,6) maxLife / subSpell(=dmg) / word_0x2C_44 | 240 / 50 / 0 | EF:35465-35467 |
| (10,6) sprite / shiftrot | `SetEntityIndexAndRot(228)` + `SetEntityShiftRot(272,1536)` | EF:35472-35473 |
| (10,6) dynamic light | `AddEvent2_847D0(80,11,1)` (Night/Cave only) | EF:35475, 47172 |
| (10,6) flags | byte[0] bit3 clear (untargetable); byte[2] bit1 set (reclaimable) + bit3 (light) | EF:35468-35469 |
| (10,6) tick damage | `sub_11400(ch0, 50)` **every tick** (+1 pulse on despawn) | EF:23111, 23152 |
| (10,6) sprite machine | life>=12 & dword_0x10_16<=6: grow (6 steps); life<12: shrink + ~1/7 (10,14) puff | EF:23115-23145 |
| (10,6) z | `word_0x2C_44 + terrainAlt` per tick | EF:23148 |
| (10,6) water | `terrain_is_water==1` → despawn | EF:23149-23151 |
| (10,6) RNG | ~1/7 smoke-puff roll only (`r%7`) | EF:23124 |
| (10,6) runtime spawn | dry-land object landing (life 30, dmg 150), tree burn (word_0x2C_44 lift) | EF:23792, 62421 |
| **(10,9)** ctor: class/model/action | 10 / 9 / 9 | EF:35516-35518 |
| (10,9) maxLife / life / subSpell(=dmg) | 11 / 17 / 2000 | EF:35519-35521, 35523 |
| (10,9) radius seed | `SetEntityShiftRot(7, 0x4000)` (pitch=7) | EF:35525 |
| (10,9) dome height | `word_0x2C_44 = 2*maxLife + 100`, capped z+ht<=255 | EF:23256-23260 |
| (10,9) profile | cosine `sin_DB750[0x200 + (dist<<10)/radius]`, eased over `life` | EF:23355-23360 |
| (10,9) terrain writes | `mapHeightmap`, `mapShading`(63/1), `mapAngle`, cave 2nd heightmap; `sub_570F0` | EF:23361-23386 |
| (10,9) tick damage | `sub_116A0(ch0, 2000)` (suppressed in apocalypse variant) → `sub_6D8B0(id,0x12,hits)` | EF:23390-23395 |
| (10,9) sounds | 10 every tick; 63 every 4th tick (apocalypse) | EF:23396-23398 |
| (10,9) child at life==3 | (10,18) normal / (10,91) apocalypse | EF:23425-23427 |
| (10,9) par consumption | `par1_14` → subspell row → `subSpellIndex` + **`maxLife`** from SPELLS table | EV:387-391, EF:33163-33170 |
| (10,9) gen pass | pass 2 (subtype 0x09) | EV:178 |
| (10,9) runtime spawn | `sub_21030` case 0xF endgame (sets byte_0x36E03=1, life 32) | EF:12864-12873 |
| **(10,11=0x0B)** ctor: class/model/action | 10 / **0x13 (19)** / **0x13** | EF:35557-35559 |
| (10,11) maxLife / subSpell(=dmg) / sprite | 240 / 200 / 228 | EF:35560-35561, 35566 |
| (10,11) flags | bit3 clear (untargetable), bit0 set, byte2 bit1 set | EF:35562, 35564 |
| (10,11) tick damage | `sub_10C80(ch0, 200)` **every tick** incl. despawn tick | EF:24148 |
| (10,11) smoke | odd life ticks: (10,14) rings over `AddE7EE0x_10080` template, jittered | EF:24107-24140 |
| (10,11) singleton | releases `D41A0_0.word_0x33 = 0` on death | EF:24149 |
| (10,11) par consumption | `par1_14` → subspell → `subSpellIndex` + **`life_0x8`** from SPELLS table | EV:387-390 |
| (10,11) gen pass | pass 2 (subtype 0x0B) | EV:178 |
| (10,11) runtime spawn | EF:23716 (drop, life 10), `sub_344A0` EF:25064 (travelling drop) | EF:23716, 25064 |
| **(10,28=0x1C)** ctor: class/model/action | 10 / 0x1C / 0x1E | EF:36174-36176 |
| (10,28) life / flags | 0 / bit3 clear; map-registered; action = one-tick despawn | EF:36173, 24989 |
| (10,28) load behavior | `stageTag!=0` → `sub_49090` → `sub_48400` road line → (10,27) segment walkers | EV:325, 5285, 5365 |
| (10,28) gen pass | pass 5 (subtype 0x1C, paired 0x1B) | EV:248 |
| **(10,31=0x1F)** ctor: class/model/action | 10 / 0x1F / 0x21 | EF:36315-36317 |
| (10,31) life / flags | 0 / bit3 clear; map-registered; action = one-tick despawn | EF:36314, 25046 |
| (10,31) load behavior | `stageTag!=0` → `sub_49090` → `sub_487D0` → (10,50)=0x32 river, `byte_0x46_70=width` | EV:328, 5303, 5558 |
| (10,31) par3_18 width remap | 0→2, 1→6, 2→16, 3→32 | EV:5327-5340 |
| (10,31) gen pass | pass 2 (subtype 0x1F) | EV:178 |
| RNG law | `r = 9377*r + 9439` (per-entity `rand_0x14_20`, global `D41A0_0.rand_0x8`) | universal |
| smoke child (10,14) | volcano/thin cloud — traced in `mc2-class10-m59-m60.md` §2.5/§3, sprite 9 | (existing doc) |

---

## OPEN items

1. **(10,6) `word_0x5A_90` sprite band bounds.** The grow/shrink machine bumps `word_0x5A_90` ±1 for 6 steps but the min/max clamp is implicit (starts at 228 from `SetEntityIndexAndRot`). Confirm the visible sprite range (228..234?) against retail and against `particlesParameters_D951C[228]` contents. Renderer-side meaning of `SetEntityShiftRot(272,1536)` (draw offset + rotation) unread.
2. **(10,9) full dome geometry.** The `sub_48E60` base sampler, `sub_570F0` heightmap writer, and `sin_DB750` table were cited but not fully expanded; the exact per-cell height easing (`(v12-v42)/life`) and the cave second-heightmap path (`x_BYTE_14B4E0`) need a dedicated pass before porting the terrain deformation faithfully. Also confirm `array_0x52_82.pitch` growth per tick (the radius accumulator) — the disc radius `v6` derives from it but the increment site is in the init/`sub_22190` path not fully read.
3. **(10,9)/(10,11) SPELLS table indexing.** `SPELLS_BEGIN_BUFFER_str[GetSpellIndex_6E020(model)].subspell[par1_14]` — need the actual spell-index mapping for model 9 and model 0x0B and the `.subSpellIndex_2`/`.life_0x1A` field offsets to reproduce the par1-driven overrides. The importer must carry `par1_14` on these THINGs.
4. **(10,28)/(10,31) painter output models.** (10,27) segment-walkers (actions 27/28/29) and (10,50)=0x32 river entities are the terrain that actually appears — they are separate models, not traced here. The importer must (a) recognize (10,28)/(10,31)/(10,29)/(10,50) markers, (b) resolve the `par1_14`/`par2_16` chain, (c) honor the pass ordering (pass 2 before pass 5). Whether mgc-import currently emits (10,28)/(10,31) THINGs at all, and whether the road/river geometry is otherwise baked into the heightmap, needs a level-000/001 THING-dump check (remc2 notes "47 instances in level 1" for the neighbor 0x2D at EV:4776 — high marker density expected).
5. **`byte_0x36E03` apocalypse latch lifetime.** Set to 1 by `sub_21030` case 0xF (EF:12871), cleared to 0 by the (10,9) ctor (EF:35527) and at EF:12873/23392-context. Since the ctor clears it, the flag must be set **after** the (10,9) is created — confirm the ordering: `sub_21030` creates (10,9) then sets `byte_0x36E03=1` (EF:12864-12872) so the first tick already sees the variant. This is load-bearing for whether the endgame dome deals damage (it does NOT in the apocalypse variant) — verify against recorded endgame footage.
6. **(10,11) `word_0x33` / (10,19) singleton.** EF:23962 disables a prior `word_0x33` entity when a new (10,19) registers — confirm whether authored (10,11) THINGs also participate in the singleton (they set model 0x13/19 but the registration at EF:23964 happens in a different action's context). If two authored (10,11)s coexist, retail may keep both (no registration) while a ported singleton would cull one.
7. **`sub_11400` player-list first loop (EF:4222-4231).** It writes `str_0x5E_94` on `model_0x40_64 == 2` entities in the player list unconditionally (no targetable gate) — clarify what class/model 2 in the player list is (props? player-owned?) vs the cell-walk's `class!=3||model!=2` exclusion, so the (10,6) fire's damage target set is exact.
