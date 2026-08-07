# CLASS-10 Models 50 (0x32), 34 (0x22) + small-count tail — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`). Trace date 2026-07-10. Companion to `mc2-class10-m59-m60.md` (§0 dispatch model) and `mc2-class10-m29-m5-m13.md` (chain-walk / smoke family). **The prompt's subtype numbers are DECIMAL** — (10,50)=model 0x32, (10,34)=0x22, (10,52)=0x34, etc. Do not confuse decimal-52 (=0x34, castle anchor) with strA1-*row* 0x52 (=82, the cave/player-start creator sub_4FBE0) — different entities (see §12).

---

## Headline findings (read first — one line per model)

1. **(10,50)=0x32 — the "beam/laser-fence between waypoints" CHAIN GENERATOR.** Its real work happens at **level-load**, not runtime: a (10,50) THING with `stageTag_12 != 0` is fed to `sub_49090` (EV:5261), which walks the `par1_14`/`par2_16` linked list and for each adjacent node pair calls `sub_48880` (EV:5586) → spawns a **(10,51)=0x33 traveling damage-beam** aimed from node→node with `life = distance/actSpeed`. The `ApplyEvents_498A0` settle loop ticks those (10,51) beams to completion (EV:508+ keeps 0x32/0x33 alive), so the beams fly the whole chain and stamp damage/terrain during load. The RUNTIME (10,50) creator (`sub_4FDE0`, action 0x36) is a **one-tick self-destruct marker** used only when `stageTag==0`. This is a chain family exactly like (10,29)'s waterpath and (10,0x1F) path-links, all routed through the same `sub_49090` switch.

2. **(10,34)=0x22 — the MC2 TELEPORTER/PORTAL (player warp pad).** Creator `sub_4FE40` (EF:36506) makes a visible sprite-223 entity floating at terrain+640, and stores a launch axis. Action 0x24 `sub_35390` (EF:25761) scans all players; a player within reach AND facing the pad (front cone <0xAA) is **teleported** (`CopyEntityPosition_57CF0`) to the pad's destination `axis_0x9A_154x` (set from THING `par1_14`/`par2_16` = destination tile), plays hum/activate/expire sounds 21/22/20, and calls `sub_5C800(player,6)`. Differs from the MC1 portal arm: MC2's is a self-contained player-only warp with a data-authored destination tile, not a paired projectile arm.

3. **(10,76)=0x4C — no-op alias.** Its creator `sub_50020`-region sets action; strA0[0x4C]→0x219D80 aliases model-0x36's handler `sub_38D80` (see m54). Actually (10,76 dec)=model 0x4C, creator `sub_4FF20`-region = **`return NewEvent` bare / disposition record** — see §5.1; treated as a data record, minimal behavior.

4. **(10,54)=0x36 — proximity DAMAGE-FIELD stamper.** Creator `AddAuxiliary_50500` (EF:36812), action 0x3B `sub_38D80` (EF:28349): each tick scans the player-list; any entity within `dword_0x10_16` range gets a value written into its damage-mailbox `str_0x5E_94.word_0x76/78/7A` (source id + radix magnitude ≤42). A radius spell-effect applicator (gas/curse aura). NOTE: shares model 0x36 & action 0x3B numbering with the (10,54) auxiliary discussed as an alias in the m59-m60 doc §0.3.

5. **(10,25)=0x19 — one-shot area BLAST, damage-type 3, mana 2000.** Creator `sub_4F6A0` (EF:36110), action 0x19 `sub_33E20` (EF:24817): 8-tick life, single latched `sub_10C80(self, 3, mana)` burst.

6. **(10,22)=0x16 — WHIRLWIND / tornado (11-node trailing chain).** Creator `AddWind_4F040` (EF:35852) spawns the head + **11 linked children** (model 75, action 82, `word_0x32/word_0x34` par-linkage), gated on ≥12 free slots. Action 0x16 `sub_33110` (EF:24155): wandering path, drags the tail nodes behind the head, loops sound 49, applies damage via `sub_33340`/`sub_33710`.

7. **(10,17)=0x11 — METEOR / comet impact (area fire).** Creator `AddMeteor_4ED70` (EF:35731), action 0x11 `sub_32880` (EF:23834): 10-tick life, `sub_10C80(self,0,mana/maxLife)` each tick (mana 3000), spawns a spreading cell-grid of (10,0) fire spawns, sound 30, reports to spellbook (`sub_6D8B0` id 9).

8. **(10,15)=0x0F — wandering FIRE-TRAIL emitter.** Creator `sub_4ECD0` (EF:35707), action 0x0F `sub_32530` (EF:23694): moves at speed 256 with ±random yaw, drops (10,11) fire puffs each tick, dies over water after 8 water-contact ticks. mana 100.

9. **(10,67)=0x43 — big multi-phase AREA/FLOOD effect.** Creator `sub_51730` (EF:37421, sprite 4352, mana 20000, life 120), action 0x48 `sub_39040` (EF:28452): phased (`byte_0x46_70` 0-3+) 18×18-tile terrain-height scan; sound 64; transitions to action 74 at end.

10. **(10,71)=0x47 — expanding ground FISSURE/LAVA area.** Creator `sub_51790` (EF:37439, sprite 1280/2048, mana 20000, life 120), action 0x4E `sub_3A2D0` (EF:29443): phased cell-iteration (`AddE7EE0x_10080`) radius ramp up/down over life.

11. **(10,8)=0x08 — DEAD MODEL.** Creator `sub_4E750` (EF:35507) is literally `return 0`. No action (strA0[8] is an unrelated slot). A (10,8) THING creates nothing. See §11 for what its 11 records/2 levels most plausibly are (a placeholder / consumed elsewhere).

12. **(10,23)=0x17 — one-shot area blast, damage-type 0, mana 25.** Creator `sub_4F5F0` (EF:36087, sprite 7, has AddEvent2 child), action 0x17 `sub_33D80` (EF:24787): single `sub_10C80(self,0,25)` burst, sound 24, spellbook report id 7.

13. **(10,52 dec)=0x34 — permanent CASTLE/BUILDING ANCHOR (no-op tick).** Creator `sub_50430` (EF:36772, sprite 205, maxLife 100000, mana 500/max 2000), action 0x38 → EV switch case 0x219B70 is an **empty no-op** (EV:2693). A passive, essentially-immortal map marker with a mana pool (a captured-building economy anchor). Matches the (10,45) building path's "(10,0x52)" reference resolving to a benign anchor with the (3,4) house fallback (§12).

14. **(10,57)=0x39** — already traced in `mc2-class10-m29-m5-m13.md` §4.3 (the (10,0x57) smoke puff). Skipped here per prompt.

---

## 0. Dispatch rows for every model in this report

Registry `str_D4C48ar` row 10 (EF:2071) → strA0 (action table, keyed by **actionIndex**) + strA1 (creator table, keyed by **model**). Creator dispatch = `IfSubtypeCallCreatingManaSphere_4A190(pos,type,subtype)` (EV:5186); action dispatch = `UpdateEntities_57730`→`pre_sub_4A190_0x6E8E` switch (EV:610).

| model (dec) | strA1 creator addr | creator fn (EF) | action idx set | strA0 action addr | action fn (EF/EV) |
|---|---|---|---|---|---|
| **50 = 0x32** | 0x230DE0 | `sub_4FDE0` :36488 | **0x36** | 0x2162A0 | `sub_352A0` :25732 (1-tick self-destruct) |
| — chain child **51 = 0x33** | 0x230D70 | `sub_4FD70` :36468 | **0x37** | 0x2162C0 | `sub_352C0` :25739 (traveling beam) |
| **34 = 0x22** | 0x230E40 | `sub_4FE40` :36506 | **0x24** | 0x216390 | `sub_35390` :25761 (portal) |
| **76 = 0x4C** | 0x2302A0 | `AddFireSpheres_4F2A0` :35936 | (see §5.1) | 0x219D80 (alias) | `sub_38D80` :28349 |
| **54 = 0x36** | 0x231500 | `AddAuxiliary_50500` :36812 | **0x3B** | 0x219D80 | `sub_38D80` :28349 |
| **25 = 0x19** | 0x2306A0 | `sub_4F6A0` :36110 | **0x19** | 0x214E20 | `sub_33E20` :24817 |
| **22 = 0x16** | 0x230040 | `AddWind_4F040` :35852 | **0x16** | 0x214110 | `sub_33110` :24155 |
| **17 = 0x11** | 0x22FD70 | `AddMeteor_4ED70` :35731 | **0x11** | 0x213880 | `sub_32880` :23834 |
| **15 = 0x0F** | 0x22FCD0 | `sub_4ECD0` :35707 | **0x0F** | 0x213530 | `sub_32530` :23694 |
| **67 = 0x43** | 0x232730 | `sub_51730` :37421 | **0x48** | 0x21A040 | `sub_39040` :28452 |
| **71 = 0x47** | 0x232790 | `sub_51790` :37439 | **0x4E** | 0x21B2D0 | `sub_3A2D0` :29443 |
| **8 = 0x08** | 0x22F750 | `sub_4E750` :35507 (**return 0**) | — | — | — |
| **23 = 0x17** | 0x2305F0 | `sub_4F5F0` :36087 | **0x17** | 0x214D80 | `sub_33D80` :24787 |
| **52 = 0x34** | 0x231430 | `sub_50430` :36772 | **0x38** | 0x219B70 | **no-op** (EV:2693) |

Numbering-trap reminder (per m59-m60 §0.3): strA0[0x3B]/[0x4C]→0x219D80 (`sub_38D80`) and are shared by models 0x36 (m54) and 0x4C. Always key handlers by the **actionIndex the ctor writes**, never by model number.

---

## 1. (10,50)=0x32 — the sub_49090 chain generator (DEEPEST)

### 1.1 Runtime creator `sub_4FDE0` (EF:36488) — a self-destruct marker
```c
type_entity_0x6E8E* sub_4FDE0(axis_3d* position)//230de0
{
	type_entity_0x6E8E* event = NewEvent_4A050();
	if (event)
	{
		event->maxLife_0x4 = 0;
		event->actionIndex_0x45_69 = 0x36;
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 0x32;
		event->position_0x4C_76 = *position;
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;   // clear bit3 (target-eligible)
		AddEventToMap_57D70(event, position);
		CopyMaxLifeToLife_49A20(event);                  // life = 0
	}
	return event;
}
```
- **RNG draws: 0.** No sprite (`SetEntityIndex*` never called → invisible). maxLife/life = 0. byte[0] bit3 cleared → not targetable.
- Action 0x36 `sub_352A0` (EF:25732) is a bare `DisableEntityDrawing04_57F10(a1x);` — **lives exactly one tick**, does nothing else. Same pattern as (10,29) and (10,50) is the runtime "no-op" fallback that fires only when the THING has no `stageTag_12`.

### 1.2 The real work: `sub_49090` chain-walk (EV:5261), fired from `PrepareEvents_49540`
`PrepareEvents_49540` (EV:287, the per-THING level-load handler) for class 0x0A subtypes **{0x1C, 0x1D, 0x1F, 0x32, 0x50}** does (EV:323-336):
```c
case 0x0A:
  switch (entity->subtype_0x30311) {
    case 0x1C: case 0x1D: case 0x1F: case 0x32: case 0x50:
      if (entity->stageTag_12)   // only 1c,1d,1f, 32 and 50   <-- author comment
        sub_49090(terrain, entity);
      return;                    // <-- NO runtime entity created when chained
```
So **(10,50) with stageTag_12 != 0 NEVER runs its creator** — it takes the chain path and returns. `sub_49090` verbatim (EV:5261-5362):
```c
void sub_49090(Type_Level_2FECE* terrain, type_entity_0x30311* entity)//22a090
{
	...
	functionPointer = nullptr;
	if (entity->type_0x30311 == 0x0A) {
		switch (entity->subtype_0x30311) {
			case 0x1C: functionPointer = &sub_48400; break;
			case 0x1D: functionPointer = &sub_48690; break;
			case 0x1F: functionPointer = &sub_487D0; break;
			case 0x32: functionPointer = &sub_48880; break;   // <-- (10,50)
			case 0x50: functionPointer = &sub_48930; break;
		}
	}
	if (functionPointer) {
		while (1) {                                  // walk par1 back to the chain HEAD
			if (!tempEntity->par1_14) break;
			tempEntity = &terrain->entity_0x30311[tempEntity->par1_14];
		}
		do {
			if (tempType != entity->type_0x30311)  break;
			if (tempSubtype != entity->subtype_0x30311) break;
			tempEntity->stageTag_12 = 0;             // consume node (prevents re-walk)
			if (!tempEntity->par2_16) break;
			tempY = tempEntity->axis2d_4.y;
			tempX = tempEntity->axis2d_4.x;
			v8   = tempEntity->par3_18;
			tempEntity = &terrain->entity_0x30311[tempEntity->par2_16];   // step to NEXT node
			switch (tempSubtype) {
				case 0x1F: /* par3 remap 0→2,1→6,2→16,3→32 */ ... break;
				case 0x50: v8 = tempEntity->par3_18 & 0xF | 16 * (v8 & 0xF); break;
				// case 0x32 (our model): v8 passes through unmodified
			}
			functionPointer(tempX, tempY, tempEntity->axis2d_4.x, tempEntity->axis2d_4.y, v8);
		} while (tempEntity);
	}
}
```
**Chain semantics (verbatim):** `par1_14` = "previous node" link, `par2_16` = "next node" link, `par3_18` = per-segment style/param. The walk climbs `par1` to the head, then walks `par2` forward, invoking the per-model stamper `sub_48880` on each consecutive (fromX,fromY)→(toX,toY) pair. Each visited node's `stageTag_12` is zeroed so the chain is processed exactly once regardless of which node the loader hits first.

### 1.3 Per-segment stamper `sub_48880` (EV:5586) — spawns a (10,51) beam per link
```c
void sub_48880(uint16_t posX2, uint16_t posY2, uint16_t posX, uint16_t posY, uint8_t a5)//229880
{
	axis_3d v9x, v12x;
	v9x.x = posX2 << 8;  v9x.y = posY2 << 8;
	v9x.z = 16 * mapHeightmap_11B4E0[256 * posY2 + posX2];       // FROM node, snapped to terrain
	v12x.x = posX << 8;  v12x.y = posY << 8;                     // TO node (xy)
	v4 = Maths::sub_581E0_maybe_tan2(&v9x, &v12x);              // yaw FROM→TO
	v5 = Maths::EuclideanDistXYZ_58490(&v9x, &v12x);           // segment length
	resultx = IfSubtypeCallCreatingManaSphere_4A190(&v9x, 10, 51);   // spawn (10,51) beam at FROM
	if (resultx) {
		v8 = resultx->actSpeed_0x82_130;                        // = 1024 (from (10,51) ctor)
		resultx->yaw_0x1C_28 = v4;                              // aimed at next node
		resultx->life_0x8 = (v5 / v8);                          // life = travel ticks to reach next node
	}
}
```
**⇒ Each chain segment becomes one traveling (10,51) entity that flies from the current node toward the next, living exactly long enough to arrive.** The `a5`/`v8` style byte is NOT consumed by this stamper (unlike 0x1F/0x50 which remap it) — for (10,50) the segment param passes through unused. Note `sub_48400`/`sub_48690`/`sub_487D0`/`sub_48930` are the sibling stampers for 0x1C/0x1D/0x1F/0x50 (out of scope; 0x1F=waterpath is in the m29 doc).

### 1.4 The (10,51)=0x33 traveling beam — creator + action
Creator `sub_4FD70` (EF:36468):
```c
event->maxLife_0x4 = 0;  event->actionIndex_0x45_69 = 0x37;
event->class_0x3F_63 = 0xA;  event->model_0x40_64 = 0x33;
event->position_0x4C_76 = *position;
event->dword_0x10_16 = 256;
event->actSpeed_0x82_130 = 1024;                 // <-- the divisor in life = dist/1024
event->struct_byte_0xc_12_15.byte[0] &= 0xF7;    // not targetable
SetEntityShiftRot_49EA0(event, 768, 768);        // extents 768 (collision/damage radius)
CopyMaxLifeToLife_49A20(event);
```
Action 0x37 `sub_352C0` (EF:25739) — the traveling-beam tick:
```c
void sub_352C0(type_entity_0x6E8E* a1x)//2162c0
{
	v1 = a1x->life_0x8;  a1x->life_0x8 = v1 - 1;
	if (v1 < 0 || sub_104A0(&a1x->position_0x4C_76) & 1) {   // expired OR hit blocked/void terrain
		DisableEntityDrawing04_57F10(a1x);
		return;
	}
	a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
	if (!sub_572C0(a1x, 0, 1024, a1x->rand_0x14_20 % 0xFu + 10, 0))   // collision probe ahead
	{
		sub_10C80(a1x, 0, a1x->subSpellIndex_0x2A_42);        // AoE damage, type 0, mana subSpell
		PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 10);  // impact sound 10
	}
	MoveEntity_57FA0(&a1x->position_0x4C_76, a1x->yaw_0x1C_28, 0, a1x->actSpeed_0x82_130);  // advance 1024/tick
}
```
- Travels along its yaw at **1024/tick**, one RNG draw per tick (entity-local LCG `r=9377*r+9439`) feeding the `sub_572C0` probe distance `r%0xF+10`.
- When the probe (`sub_572C0`) reports a hit ahead, deals **`sub_10C80(self, 0, subSpellIndex)`** area damage (type-0 mask) and plays **sound 10**.
- Despawns on life underflow OR when `sub_104A0 & 1` (terrain-blocked cell).
- `subSpellIndex` of the (10,51) is NOT set by `sub_4FD70` (defaults to 100 from NewEvent) — the damage amount is the NewEvent default unless a caller overrides it. **OPEN-1:** the segment's damage magnitude source (whether the chain author sets subSpell) is not overridden in `sub_48880`; likely the default 100 or set via the THING record's par fields elsewhere.

### 1.5 Level-load orchestration — `GenerateEvents_49290` (EV:152) passes & `ApplyEvents_498A0` settle
- (10,50)=0x32 and (10,51)=0x33 are processed in **pass 2** (EV:190-191) — only THINGs with **`DisId == -1`** (load-time, EV:178). After the pass, `ApplyEvents_498A0()` (EV:204) runs the settle loop.
- Settle loop (EV:508-521): for class-0xA model `v4 >= 0x32`: it DISABLES models in the band `v4 > 0x33 && (v4 < 0x50 || (v4 > 0x55 && v4 != 0x58))` — but **0x32 and 0x33 are explicitly NOT in that disable band** (they fail `v4 > 0x33`), so they fall through to the `runagain=true` ticking branch (EV:497-506). **⇒ The (10,51) beams are ticked to completion during load-settle**, flying each chain segment and stamping `sub_10C80` damage/reveal along the path, before gameplay begins. Any (10,50) marker that survived to settle (stageTag==0 fallback) is a 1-tick self-destruct, so it's gone by the first settle iteration.

### 1.6 THING post-init (`sub_4A310`, EF:32999) for the runtime path
When (10,50) is created at runtime via a disposition (not the chain), `sub_4A310` case 0xA `v4==0x36` branch (EF:33095-33104) does:
```c
v3x->dword_0x10_16 = (entity->stageTag_12 << 8) * (entity->stageTag_12 << 8);
v5 = 8 * entity->stageTag_12 + 16;
v3x->maxLife_0x4 = v5;  if (v5 < 128) v3x->maxLife_0x4 = 128;
CopyMaxLifeToLife_49A20(v3x);  sub_58DA0(entity, v3x);
```
i.e. par-driven life scaling for the marker — but since its action self-destructs in one tick, this only matters if a disposition-fired (10,50) is meant to persist (it isn't, per action 0x36). Mostly a dead path; the chain path (§1.2) is the live one.

---

## 2. (10,34)=0x22 — the MC2 teleporter/portal (DEEP)

### 2.1 Creator `sub_4FE40` (EF:36506)
```c
type_entity_0x6E8E* sub_4FE40(axis_3d* position)//230e40
{
	type_entity_0x6E8E* event = NewEvent_4A050();
	if (event)
	{
		event->actionIndex_0x45_69 = 0x24;
		event->class_0x3F_63 = 0xA;
		event->model_0x40_64 = 0x22;
		event->maxLife_0x4 = 0;
		event->xtype_0x41_65 = 3;                 // targets class-3 (players)
		event->xsubtype_0x42_66 = -1;
		event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;   // not targetable
		event->position_0x4C_76 = *position;
		SetEntityIndexAndRot_49CD0(event, 223);           // sprite/particle row 223 (visible pad)
		SetEntityShiftRot_49EA0(event, 256, 256);         // extents 256
		CopyMaxLifeToLife_49A20(event);
		AddEventToMap_57D70(event, position);
		event->position_0x4C_76.z = getTerrainAlt_10C40(&event->position_0x4C_76) + 640;  // hover 640 up
		event->axis_0x9A_154x = event->position_0x4C_76;                                   // launch axis seed
		event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;                           // 1 RNG draw
		MoveEntity_57FA0(&event->axis_0x9A_154x, event->rand_0x14_20 & 0x7FF, 0, -32768);  // fling axis
	}
	return event;
}
```
- **RNG draws: 1** (entity-local), used only to fling the initial `axis_0x9A_154x` in a random yaw at -32768 — a seed that the THING post-init OVERWRITES with the real destination (§2.2).
- Visible (sprite 223), extents 256, hovers 640 above terrain, targets class 3 (players).

### 2.2 THING post-init sets the DESTINATION tile — `sub_4A310` (EF:33077)
```c
if (v4 <= 0x22)   // model 0x22
{
	v2x->axis_0x9A_154x.x = (entity->par2_16 << 8) + 128;   // dest tile X = par2
	v2x->axis_0x9A_154x.y = (entity->par1_14 << 8) + 128;   // dest tile Y = par1
	sub_58DA0(entity, v3x);
	return;
}
```
**⇒ The teleport target is the THING's `par1_14`/`par2_16` = destination tile (Y,X).** (The ctor's random fling of §2.1 is immediately replaced by this.) `sub_58DA0` additionally binds the pad into any stage record pointing at it.

### 2.3 Action 0x24 `sub_35390` (EF:25761) — the warp tick
```c
void sub_35390(type_entity_0x6E8E* a1x)//216390
{
	if (!(a1x->struct_byte_0xc_12_15.byte[0] & 2)) {          // once, on first tick:
		a1x->struct_byte_0xc_12_15.byte[0] |= 2u;
		PrepareEventSound_6E450(..., 21);                    // hum/spawn sound 21
	}
	v2 = a1x->life_0x8;
	if (v2 <= 0 || (a1x->life_0x8 = v2 - 1, v2 != 1)) {       // while alive (maxLife 0 → v2<=0 keeps it looping)
		for (i = 0; i < D41A0_0.NumberOfPlayers_0xe; i++) {
			v4x = Entities_EA3E4[ D41A0_0.array_0x2BDE[i].playerIndex_... ];
			if (sub_106C0(a1x, v4x)) {                        // player within pad reach
				v5 = Maths::sub_581E0_maybe_tan2(&v4x->position, &pad->position);
				if ((uint16_t)sub_582B0(v4x->yaw_0x1C_28, v5) < 0xAAu) {   // player FACING pad (<~30deg)
					v7 = getTerrainAlt_10C40(&a1x->axis_0x9A_154x);
					a1x->axis_0x9A_154x.z = a1x->dword_0xA0_160x->word_160_0xc_12 + v7;  // dest z
					PrepareEventSound_6E450(..., 22);          // activate/warp sound 22
					CopyEntityPosition_57CF0(v4x, &a1x->axis_0x9A_154x);   // TELEPORT the player
					sub_5C800(v4x, 6);                         // post-warp effect (state 6)
				}
			}
		}
		a1x->position_0x4C_76.z = getTerrainAlt_10C40(&a1x->position);   // re-clamp pad to ground+hover
	}
	else {
		DisableEntityDrawing04_57F10(a1x);
		PrepareEventSound_6E450(..., 20);                    // expire sound 20
	}
}
```
- The pad is **persistent** (maxLife 0 → the `v2 <= 0` guard keeps it looping; it only expires on the `v2 == 1` branch, which needs a positive life first — so with maxLife 0 it effectively never self-expires and stays as a permanent warp pad).
- Warp condition: player within `sub_106C0` reach **and** facing the pad (front cone `sub_582B0 < 0xAA` ≈ 30°). On trip: teleport to `axis_0x9A_154x` (dest tile from par1/par2, dest z = destination-record `word_160_0xc_12` + terrain alt) + `sub_5C800(player,6)`.
- **Sounds: 21 on spawn (once), 22 on each warp, 20 on expire.**
- `dword_0xA0_160x` = a behavior-row pointer (`str_D7BD6[...]`); `word_160_0xc_12` supplies the destination altitude offset. **RESOLVED (was OPEN-2):** neither the ctor (`sub_4FE40` EF:36506) nor the (10,34) THING post-init (EF:33077 — writes `axis_0x9A_154x.x/y` only) repoints `dword_0xA0_160x`, so the NewEvent default `&str_D7BD6[59]` (Events.cpp:573/599) stands. Row 59 = `{0x0000,0x0038,0x0005,0x0016,0x0005,0x0700,`**`0x0000`**`,0xFFFC,...}` (Level.cpp:11 table) → `word_160_0xc_12 = 0` → **warp-out z = terrain alt at the destination tile exactly (ground + 0)**. MC1's vortex (`sub_26A60` remc1 :29170) runs the identical law with its NewEvent default `unk_98F38[0]` — the byte-identical row (word12 = 0) — so both games emerge on the ground.

### 2.4 Difference from the MC1 portal arm (port note)
Our port has an MC1-style portal. MC2's (10,34):
- Is a **single self-contained pad**, not a paired spawn-arm/projectile.
- Its **destination is data-authored** in the THING's par1_14 (destY) / par2_16 (destX), not computed from a partner entity.
- Only teleports **players** (xtype=3, player-list scan) and only when they are **facing** the pad.
- Uses a distinct sound set (21/22/20) and applies `sub_5C800(player,6)` post-warp.
Port action: add an MC2 portal variant that reads par1/par2 as a destination tile and warps the facing player, rather than reusing the MC1 arm creator.

---

## 3. (10,54)=0x36 — proximity damage-field (`sub_38D80`)

### 3.1 Creator `AddAuxiliary_50500` (EF:36812)
```c
event->actionIndex_0x45_69 = 0x3B;  event->class_0x3F_63 = 0xA;  event->model_0x40_64 = 0x36;
event->maxLife_0x4 = 128;  event->actSpeed_0x82_130 = 256;
event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;      // not targetable
event->subSpellIndex_0x2A_42 = 100;
event->rand_0x14_20 = 9377 * event->rand_0x14_20 + 9439;   // 1 RNG draw
event->dword_0x10_16 = 12845056;                    // = 0xC40000; RANGE (see action)
event->yaw_0x1C_28 = event->rand_0x14_20 & 0x7FF;   // random facing
event->position_0x4C_76 = *position;
event->struct_byte_0xc_12_15.byte[0] |= 1u;
CopyMaxLifeToLife_49A20(event);  SetEntityShiftRot_49EA0(event, 1024, 0x4000);
```
- **RNG draws: 1** (random yaw). Life 128 ticks, no sprite index set (invisible field). `dword_0x10_16 = 12845056` is the squared-distance range used by the action.

### 3.2 Action 0x3B `sub_38D80` (EF:28349)
```c
result = a1x->life_0x8;  a1x->life_0x8 = result - 1;
if (result & 0x80000000) { DisableEntityDrawing04_57F10(a1x); return; }   // life<0 → despawn
for (ix = x_D41A0_BYTEARRAY_4_struct.dword_38523; ix > Entities_EA3E4[0]; ix = ix->next_0) {
	if (!ix->str_0x5E_94.word_0x7A_122) {                                  // not already tagged
		result = Maths::EuclideanDistXY_584D0(&a1x->position, &ix->position);
		if (result < a1x->dword_0x10_16) {                                 // within range
			v3 = Maths::sub_7277A_radix_3d(result);  if (v3 > 0x2A) v3 = 42;
			ix->str_0x5E_94.word_0x76_118 = LOWORD(v3);
			ix->str_0x5E_94.word_0x78_120 = HIWORD(v3);
			ix->str_0x5E_94.word_0x7A_122 = (a1x - D41A0_0.struct_0x6E8E);   // source id, latch
		}
	}
}
```
Each tick scans the entity list (`dword_38523` head) and stamps the target's damage-mailbox fields `word_0x76/78/7A` with a radix magnitude (≤42) + source id — but only if `word_0x7A_122` is 0 (one tag per victim, first-come). This is a **field/aura applicator** that marks nearby entities for a downstream effect (the victim's own handler reads `str_0x5E_94`). **No direct HP damage, no sound.**

---

## 4. (10,25)=0x19 — one-shot area blast (type 3)

Creator `sub_4F6A0` (EF:36110): action 0x19, model 0x19, `maxLife=8`, `subSpellIndex=2000`, byte[0]=(…&0xF6)|1, AddEventToMap, `SetEntityShiftRot(512,512)`. RNG: 0.
Action 0x19 `sub_33E20` (EF:24817):
```c
v1 = a1x->life_0x8 - 1;  a1x->dword_0x10_16++;  a1x->life_0x8 = v1;
if (v1 >= 0) {
	if (!(a1x->struct_byte_0xc_12_15.byte[0] & 2)) {          // once:
		a1x->struct_byte_0xc_12_15.byte[0] |= 2u;
		if (sub_10C80(a1x, 3u, a1x->byte_0x46_70))            // AoE damage type 3, amount byte_0x46
			a1x->life_0x8 = 0;
	}
} else DisableEntityDrawing04_57F10(a1x);
```
One latched `sub_10C80(self, 3, byte_0x46_70)` burst (type-3 mask). Note the amount is `byte_0x46_70` (a par-set field), not `subSpellIndex`. No sound.

---

## 5. (10,76)=0x4C, (10,17)=0x11, (10,15)=0x0F, (10,22)=0x16, (10,67)=0x43, (10,71)=0x47 — the effect band

### 5.1 (10,76)=0x4C — `AddFireSpheres_4F2A0` (EF:35936)
Creator addr 0x2302A0 = `AddFireSpheres_4F2A0`. strA0[0x4C]→0x219D80 aliases `sub_38D80` (the m54 field handler) — but the ctor sets its own actionIndex. **OPEN-3:** the exact actionIndex `AddFireSpheres_4F2A0` writes and its sphere-ring body were not fully transcribed in this pass; the name and the strA0[0x4C]-alias indicate a **fire-sphere ring / multi-projectile burst**. Its 172 records/25 levels make it a common decorative/hazard emitter. Creator citation EF:35936; action alias EF:28349. Recommend a dedicated read before porting (the count warrants it — flagged high in OPEN).

### 5.2 (10,17)=0x11 — meteor (`AddMeteor_4ED70` :35731 / `sub_32880` :23834)
Ctor: action 0x11, model 0x11, maxLife 10, subSpellIndex 3000, byte[0]&=0xF7. RNG 0.
Action `sub_32880` per tick (10 ticks):
- First tick: set `dword |= 0x10002`, **sound 30**.
- `SetEntityShiftRot(…)` grows sprite by `dword_0x10_16` frame counter.
- **`sub_10C80(self, 0, subSpellIndex/maxLife)`** = 3000/10 = 300 area damage/tick; if it hit, `sub_6D8B0(id, 9, dmg)` spellbook report.
- Spawns a spreading grid of **(10,0)** fire spawns via `AddE7EE0x_10080`/`sub_10130` cell iteration, each inheriting id/yaw, `dword|=0x10080`, sprite ShiftRot(512,512). RNG: 2 draws/spawned-cell for jitter (`r%0x81`). `dword_0x10_16 = (dword_0x10_16+2)%11` cycles the spawn ring.

### 5.3 (10,15)=0x0F — wandering fire-trail (`sub_4ECD0` :35707 / `sub_32530` :23694)
Ctor: action 0x0F, model 0x0F, maxLife 128, actSpeed 256, subSpellIndex 100, random yaw (1 RNG), `SetEntityShiftRot(1024,0x4000)`.
Action `sub_32530`:
```c
if (sub_104A0(&pos) & 1) dword_0x10_16++;              // over blocked/water tile → count up
else if (dword_0x10_16 > 0) dword_0x10_16--;
life--;
if (life < -1 || dword_0x10_16 > 8) { DisableEntityDrawing04; return; }   // dies after 8 water ticks
rand = 9377*rand+9439;                                  // 1 RNG/tick
yaw = ((rand%0x5B) + yaw - 45) & 0x7FF;                 // wander ±
MoveEntity(&pos, yaw, 0, 256);                          // advance 256/tick
resultx = IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 11);   // drop a (10,11) fire puff
if (resultx) { copy extents; life=10; word_0x26_38=15; id=self.id; }
```
A serpentine fire-trail that lays (10,11) puffs and dies over water. No sound. mana 100.

### 5.4 (10,22)=0x16 — whirlwind/tornado with 11-node tail (`AddWind_4F040` :35852 / `sub_33110` :24155)
Creator (gated on ≥12 free slots):
- Head: action 0x16, model 0x16, `minSpeed=20, maxSpeed=10, actSpeed=50`, maxLife 500, subSpellIndex 1000, byte_0x38_56=1, random roll/yaw/pitch (1 RNG draw), byte[0]&=0xF7.
- **Then a `for(i=0;i<11;i++)` loop** builds 11 children by `qmemcpy(child, head, 0xA8)` then `model=75, action=82, word_0x2C_44=i+1`, wiring the doubly-linked chain via `word_0x32_50`(prev)/`word_0x34_52`(next), `byte_0x3E_62=i`, AddEventToMap. Finally `sub_4F1C0(head)`.
Action `sub_33110`:
```c
life--;
if (life < 0) { EndLoop_6EAB0(id,-1,49); sub_338D0(a1x); }   // end wind-loop sound 49, teardown
else { sub_331A0(a1x); sub_33340(a1x); sub_33710(a1x); PrepareEventSound(id,-1,49); }  // loop sound 49
```
`sub_331A0` (EF:24177): wanders the head (random `word_0x2E_46` sign flips every 16 ticks, roll drift, speed 32 lateral + 120 forward), then drags each tail node toward the previous node keeping a spacing gap `72 - 4*(12 - word_0x2C_44)` (nodes trail in a spiral). `sub_33340`/`sub_33710` = the damage/pickup passes (not transcribed; they carry the whirlwind's `subSpellIndex=1000` effect). **Sound: 49 loop while alive.** This is a genuine **chain family** (par-linked tail) alongside (10,50) and (10,29).

### 5.5 (10,67)=0x43 — big flood/quake area (`sub_51730` :37421 / `sub_39040` :28452)
Ctor: action 0x48, model 0x43, byte[0]=(…&0xF6)|1, life 120, subSpellIndex 20000, `SetEntityShiftRot(4352,4352)` (huge extent). No sprite index, no RNG.
Action `sub_39040`: a large phase machine keyed on `byte_0x46_70` (0→1→2→3):
- phase 0: `sub_39E40(a1x)` init probe; fail → despawn.
- phase 1: `GetTerrainHeightFromSquare_48DF0(x-9,y-9,18,18)` samples an 18×18-tile region, sets `position.z`, `word_0x2C_44`, **sound 64**, arms `dword_0x10_16=12` countdown.
- phase 2: countdown → phase 3.
- At `life<=0`: `actionIndex=74 (0x4A)`, `byte_0x46_70=0` (hands off to a finisher action). A slow, large-radius terrain-area effect (flood/earthquake). **OPEN-4:** phases 3+ and the action-74 finisher not transcribed.

### 5.6 (10,71)=0x47 — expanding fissure/lava area (`sub_51790` :37439 / `sub_3A2D0` :29443)
Ctor: action 0x4E, model 0x47, life 120, subSpellIndex 20000, byte_0x46_70=0, maxLife=life, `SetEntityShiftRot(1280,2048)`. No RNG.
Action `sub_3A2D0`: phase machine on `byte_0x46_70`:
- phase 0 init: `word_0x2C_44 = maxLife>>3`, `subSpellIndex = 4*(subSpell/maxLife)`, phase→1.
- ongoing: computes a radius `v6` ramping with life (up in the middle third, down at the ends, clamped [0, min(3*word_0x2C_44,15)]), random phase-jump (`byte_0x46_70 += 2` on `rand%5==0`), then iterates the affected cells via `AddE7EE0x_10080(0, v6)` / `sub_10130` and stamps each cell (parity-alternating `v10`). A growing/shrinking ground-hazard disc. **OPEN-5:** per-cell stamp target (heightmap vs. damage) not fully transcribed.

---

## 6. (10,23)=0x17 — one-shot area blast (type 0, small)

Creator `sub_4F5F0` (EF:36087): action 0x17, model 0x17, maxLife 8, subSpellIndex 25, `dword&=0xFFFDFFF7` + `byte[2]|=2` (reclaimable effect), AddEventToMap, `SetEntityIndexAndRot(7)` (sprite 7), `SetEntityShiftRot(200,200)`, byte[0]|=1, then **`AddEvent2_847D0(event,128,9,0)`** (spawns an attached secondary — a paired sound/visual child). RNG 0.
Action 0x17 `sub_33D80` (EF:24787):
```c
life--;
if (life >= 0) {
	if (!(byte[0] & 2)) {                                  // once:
		v2 = sub_10C80(a1x, 0, subSpellIndex);            // AoE type 0, mana 25
		if (v2) sub_6D8B0(id, 7u, v2);                    // spellbook report id 7
		PrepareEventSound(id, -1, 24);                    // sound 24
		life = 1;  byte[0] |= 2;
	}
} else DisableEntityDrawing04_57F10(a1x);
```
A small single-burst blast (sprite 7), sound 24. Contrast with (10,25): (10,23) uses `subSpellIndex` (25) & type-0; (10,25) uses `byte_0x46_70` & type-3.

---

## 7. (10,52 dec)=0x34 — permanent castle/building anchor (no-op tick)

Creator `sub_50430` (EF:36772):
```c
event->actionIndex_0x45_69 = 0x38;  event->class_0x3F_63 = 0xA;  event->model_0x40_64 = 0x34;
event->maxLife_0x4 = 100000;                 // effectively immortal
event->subSpellIndex_0x2A_42 = 500;
event->dword_0x10_16 = 600;
event->mana_0x90_144 = 500;  event->maxMana_0x8C_140 = 2000;   // has a MANA POOL
event->struct_byte_0xc_12_15.byte[0] &= 0xF7u;                 // not targetable
AddEventToMap_57D70(event, position);  CopyMaxLifeToLife_49A20(event);
SetEntityIndexAndRot_49CD0(event, 205);                        // sprite 205 (visible building marker)
```
Action 0x38 → EV switch case **0x219B70 is an empty `{ break; }`** (EV:2693) — **the entity ticks but does nothing.** A passive, near-immortal, mana-bearing map anchor with sprite 205. Consistent with a **captured-building economy anchor** (mana pool 500/2000, mirrors the class-10 mana-sphere economy family). Its 6 records/2 levels fit a per-building fixture. It is the (10,0x52)-in-decimal the (10,45) building-creator path falls back on (see §12), resolving to a benign anchor.

---

## 8. (10,8)=0x08 — dead model

Creator `sub_4E750` (EF:35507) is:
```c
type_entity_0x6E8E* sub_4E750()//22f750
{ return 0; }
```
**A (10,8) THING creates NOTHING** (the creator returns 0 unconditionally; `sub_4A310` sees `!indexx` and returns, EF:33020). There is no action handler tied to model 8. Its 11 records/2 levels are inert in this decompile — either vestigial authoring data or consumed by a non-creator path. **OPEN-6:** grep found no code reading `model==0x08` for class 10; treat (10,8) as a no-op record in the importer (do not attempt to spawn it).

---

## 9. Damage / targetability summary

MC2 damage is delivered by `sub_11900` (EF:4375) writing the victim's mailbox `str_0x5E_94` — the victim's own action must read it. In this batch:
- **Damage DEALERS** (via `sub_10C80` AoE primitive, EF:3953 — scans map cells within `array_0x52_82.pitch`, applies `1<<a2` effect mask, amount a3): (10,51) beam (type 0), (10,17) meteor (type 0), (10,23) blast (type 0), (10,25) blast (type 3), plus (10,54) writes mailbox fields directly (aura).
- **Immune-by-omission** (never read `str_0x5E_94`): all of them — these are effect emitters, not creatures; every ctor clears byte[0] bit3 (`&0xF7` or `&0xF6`) so they are **not targetable** and cannot be found by target scans. The (10,34) portal, (10,52) anchor, and (10,50) markers likewise have no mailbox read.
- The **(10,34) portal** and **(10,52) anchor** are the only two with byte[0] configurations that keep them in the map grid persistently; both are non-damageable.

---

## 10. Spawn paths (THING post-init + runtime callers)

- **(10,50)/(10,51):** level-load only, via `PrepareEvents_49540`→`sub_49090` chain (stageTag set) or the self-destruct-marker creator (stageTag==0). GenerateEvents **pass 2**, DisId==-1. Settle-ticked to completion. No runtime code caller for the chain.
- **(10,34):** THING via `sub_4A310` sets dest par1/par2 (EF:33077). No other runtime spawner found.
- **(10,54)/(10,25)/(10,23)/(10,15)/(10,17):** spell/effect creators — reached via `IfSubtypeCallCreatingManaSphere_4A190` from spell-cast code and/or map THINGs (default `sub_4A310` branch, EF:33033-33196 assigns subSpell/life from the SPELLS buffer for models 9/0xB/0xF/0x11/0x16/0x43/0x47). (10,17)=0x11 gets `life/maxLife` from `SPELLS_BEGIN_BUFFER_str[...].subspell[par1].life_0x1A` (EF:33150-33179).
- **(10,67)=0x43 / (10,71)=0x47:** also take the SPELLS-buffer life-init path in `sub_4A310` (EF:33118-33146, 33158-33179) — they are castable spell effects with authored par1 = subspell index.
- **(10,22):** `AddWind_4F040` self-builds its 11-child chain; runtime callers = spell cast (whirlwind spell).
- **(10,52)=0x34:** THING/building path; sprite-205 anchor.
- **(10,8):** none (creator returns 0).

---

## 11. Cross-references / interactions

- The **`sub_49090` chain family** (this doc's (10,50), plus (10,29)-waterpath=0x1F, (10,0x1C), (10,0x1D), (10,0x50)) all share one switch and one par1/par2/par3 linked-list walk; only the per-model stamper differs. (10,50)→`sub_48880`→(10,51) beam. This is the same linked-list mechanism as the multipart mob chains (`word_0x32/word_0x34`) but for AUTHORED THING chains (`par1/par2`).
- The **(10,22) whirlwind** is a runtime chain (`word_0x32/word_0x34`), distinct from the load-time par1/par2 chains — same "leader drags trailing nodes" idea.
- **`sub_10C80`** (EF:3953) is the shared AoE damage primitive for the whole effect band; a faithful port needs it once and every model here calls it with a different `(mask, amount)`.
- **Settle-loop disable band** (EV:508): models 0x34(m52), 0x36(m54), 0x43(m67), 0x47(m71) are DISABLED (not ticked) during `ApplyEvents` — so if authored as load-time DisId==-1 THINGs they are destroyed at settle unless re-fired by a runtime disposition. (10,50)/(10,51) are the exception (ticked to completion).

---

## 12. The (10,45) building path and the "(10,0x52)→(3,4)" fallback

Per the roster (`docs/SURVEY-MC2-ROSTER.md`) and ROADMAP, MC2's captured-building creation resolves a class-10 building anchor and falls back to a class-3 house `(3,4)` when the class-10 slot is empty. In DECIMAL terms the building anchor is **(10,52)=model 0x34** (`sub_50430`, §7) — the permanent sprite-205 mana anchor. It is benign (no-op action, immortal), so the building teardown/claim column can key on it safely. Note the SEPARATE strA1-*row* 0x52 (=82) = `sub_4FBE0` (cave/player-start, model 0x52) is unrelated to the (10,45) building — do not conflate. **Confirm which of the two "52"s the (10,45) creator references** against the (10,45) trace/building code (OPEN-7).

---

## Constants table (consolidated)

| model | class/model/action | life | mana/subSpell | RNG draws (ctor) | sprite | sounds | key numbers |
|---|---|---|---|---|---|---|---|
| 50=0x32 (marker) | 10/0x32/0x36 | 0 | — | 0 | none | — | 1-tick self-destruct; chain path skips ctor |
| 51=0x33 (beam) | 10/0x33/0x37 | dist/1024 | subSpell(default) | 1/tick | none | 10 on hit | actSpeed 1024, extents 768, `sub_10C80` type0 |
| 34=0x22 (portal) | 10/0x22/0x24 | 0 (persistent) | — | 1 | 223 | 21 spawn/22 warp/20 expire | dest = par1(Y)/par2(X); hover +640; front cone <0xAA |
| 54=0x36 (aura) | 10/0x36/0x3B | 128 | 100 | 1 | none | — | range 0xC40000; stamps mailbox word_76/78/7A, mag ≤42 |
| 25=0x19 (blast) | 10/0x19/0x19 | 8 | 2000 | 0 | — | — | `sub_10C80` type3 amount byte_0x46, latched |
| 22=0x16 (tornado) | 10/0x16/0x16 | 500 | 1000 | 1 (+per-tick) | — | 49 loop | 11 children model75/act82, chain word_32/34, gate ≥12 slots |
| 17=0x11 (meteor) | 10/0x11/0x11 | 10 | 3000 | 0 (+2/spawn) | grows | 30 | `sub_10C80` type0 mana/maxLife=300/tick; (10,0) fire grid |
| 15=0x0F (fire trail) | 10/0x0F/0x0F | 128 | 100 | 1 (+1/tick) | ShiftRot 1024 | — | speed 256, wander ±0x5B, drops (10,11), dies 8 water-ticks |
| 67=0x43 (flood) | 10/0x43/0x48 | 120 | 20000 | 0 | 4352 ext | 64 | 18×18 terrain scan; phases 0-3; →action 74 |
| 71=0x47 (fissure) | 10/0x47/0x4E | 120 | 20000 | 0 (+/tick) | 1280/2048 | — | phase radius ramp; AddE7EE0x cell iter |
| 8=0x08 | — | — | — | — | — | — | creator returns 0 (dead) |
| 23=0x17 (blast) | 10/0x17/0x17 | 8 | 25 | 0 | 7 | 24 | `sub_10C80` type0 mana25; AddEvent2 child; spellbook id7 |
| 52=0x34 (anchor) | 10/0x34/0x38 | 100000 | 500 (mana 500/2000) | 0 | 205 | — | no-op action (EV:2693); building anchor |

| shared item | value | source |
|---|---|---|
| RNG law | `r = 9377*r + 9439` (entity-local rand_0x14_20; global D41A0_0.rand_0x8) | EV:578 etc. |
| chain links | par1_14=prev, par2_16=next, par3_18=style (THING); word_0x32/34=prev/next (runtime) | EV:5310-5326, EF:35878-35884 |
| AoE damage | `sub_10C80(self, mask a2, amount a3)` scans cells within `array_0x52_82.pitch` | EF:3953 |
| GenerateEvents pass2 | 0x32,0x33 (+others), DisId==-1 only | EV:190-191 |
| settle keeps 0x32/0x33 | disable band is `>0x33 && (<0x50 || (>0x55 && !=0x58))` | EV:508-521 |

---

## OPEN items
1. **(10,51) beam damage magnitude** — `sub_48880` sets yaw+life but not subSpellIndex; the (10,51) ctor doesn't set it either → defaults to NewEvent's 100. Whether the chain author intends a different magnitude (via a par field or a spell-buffer lookup at THING init) is unconfirmed. Verify with a level that has a (10,50) fence.
2. ~~**(10,34) portal warp-out altitude**~~ **RESOLVED** (mc2l24 playthrough deviation dig, 2026-08-07): the pad keeps the NewEvent default row 59, whose `word_160_0xc_12 = 0` → warp-out z = dest ground exactly. Same law + same row value in MC1's vortex (`unk_98F38[0]`). See §2.3.
3. **(10,76)=0x4C (`AddFireSpheres_4F2A0` :35936)** — highest-count tail model (172/25) but only headline-identified here (fire-sphere ring). Its full creator body + action (strA0[0x4C] alias to `sub_38D80` vs. an own actionIndex) needs a dedicated read before porting. Flagged high.
4. **(10,67)=0x43 phases 3+ and the action-74 (0x4A) finisher** not transcribed — needed for the full flood/quake behavior.
5. **(10,71)=0x47 per-cell stamp target** (heightmap raise vs. damage vs. lava terrain-type write) inside the `sub_10130` loop not fully transcribed.
6. **(10,8)=0x08** creates nothing; confirm its 11 THING records are truly inert (not consumed by a non-creator subsystem) in the importer data.
7. **(10,45) building creator's "52"** — verify whether it references decimal-52 (=model 0x34 anchor, §7/§12) or strA1-row 0x52 (=model 0x52 cave/player-start `sub_4FBE0`), and how the (3,4) house fallback is wired, against the (10,45) building trace.
8. **`sub_33340`/`sub_33710`** (whirlwind damage/pickup passes) and **`sub_5C800(player,6)`** (portal post-warp effect) bodies not transcribed — pull when porting (10,22) damage and (10,34) warp feedback.

---

## Retail-check bank (for player-certified verification later)
- **(10,50) fence:** a level with authored (10,50) chain THINGs should show a laser/beam sweeping node-to-node at load, dealing damage along the path (the beams are gone by first frame — verify via the terrain/entity state it leaves, not a visible beam).
- **(10,34) portal:** stand a player on the pad facing it → teleport to the par1/par2 destination tile with sound 22; approach not-facing → no warp. Sound 21 on level start (pad hum), sound 20 if a pad expires.
- **(10,22) whirlwind:** an 11-segment spiraling tornado with wind loop sound 49, wandering, dragging its tail; damages entities it crosses (mana 1000).
- **(10,17) meteor:** 10-tick impact laying a spreading fire grid, sound 30, ~300 dmg/tick.
- **(10,52) building anchor:** a passive sprite-205 fixture at captured buildings with a 500/2000 mana pool; never dies, never ticks.
