# Phase-3 research bank (2026-07-09)

Condensed porting notes from three background research passes over
`reference/remc2/remc2/engine/` (all `:N` line cites =
EventsFunctions.cpp unless named). Companion to ROADMAP "Phase 3".
The Rust ports must cite these sites; verify at the source when a
detail is load-bearing.

## A. Behavior rows + sprite params (extracted — mgc_sim::mc2)

- `str_D7BD6[157]` (Level.cpp:11) = MC1 Type_156 schema + trailing
  FLAGS byte (bit 1 die-on-water :8855, bit 4 pack-disable :9022,
  bit 8 flee/alt-chase :9003). Base pointer = index 59
  (Events.cpp:573/599); indices 0-58 = a typed non-row section,
  108-155 = in-source duplicate. Ctors hand-pick ABSOLUTE rows:
  Vulture 98, Archers 75, Villager 100 (:33739/:33899/:34058).
  MC2[59] ≡ MC1 BEHAVIOR[0] byte-for-byte (anchor test in
  mc2/mod.rs). Extracted → mc2/behavior.rs by
  tools/extract-remc2-tables.py.
- `particlesParameters_D951C[347]` (Type_WORD_D951C.cpp:3): ctor
  passes row index to SetEntityIndexAndRot_49CD0 (:32837; stores in
  entity +90, derives rot params /2 — no RNG); word_0 = TMAPS sprite
  base, byte_12 = draw type. WORLD EXTENTS COME FROM THE CTOR's
  SetEntityShiftRot_49EA0 ARGS, NOT THIS TABLE (unlike MC1
  sprite_stats). Extracted → mc2/sprite_params.rs.
- Slice sprite type indices: Vulture 238 (:33745); Archers 0
  (:33905); Villager RNG one of 242/271/241/239 via `rand % 9` →
  0-2:242, 3-5:271, 6-7:241, 8:239 (:34066-34087).
- Awake pre-pass twins: sub_68BF0 (:55469 driver) + sub_68C70
  (:55494), called Events.cpp:3438/3442. Radius 2D
  `EuclideanDistXY < 0x2400000` (:55526) — SAME constant as MC1.
  Re-arm 16 (:55529), segment mirror +2 (:55535), dead sentinel
  byte_0x39_57=0xFA (:55484). MC2-only early-out:
  `struct_byte_0xc_12_15.byte[0] & 1` → skip (:55515) — an ENTITY
  type-flag byte, distinct from the row flags byte.
- Entity rand seed is `uint16_t rand_0x14_20` (global_types.h:331)
  — ChassisParams::MC2 RandWidth::U16 CONFIRMED at the type.

## B. Init orchestration + stage/objective engine

Struct homes: THING record `type_entity_0x30311` (20B,
BasicTerrain.h:7-18); level blob has `entity_0x30311[1200]` +
`stages_0x36442[8]` checkpoints (BasicTerrain.h:102/105); objective
type legend BasicTerrain.h:36-46.

### Level init (order)
`sub_49270_generate_level_features` (Level.cpp:430) →
`SetStagetagForTermod_49830` (Level.cpp:1388 — pre-sets stageTag=1
for class-10 subtypes {0x1C,0x1D,0x1F,0x32,0x50}) →
`GenerateEvents_49290` (Events.cpp:152-282) → `InitStages_58940`
(:39406) → `InitStageVars_11EE0`, `Init0x3664C_84790` →
**`sub_4A1E0(0, 1)` = disposition 0** (:39425).

GenerateEvents passes (each = one slot scan 1..0x4b0 ascending over
records with `DisId == -1`, then a queue drain `ApplyEvents_498A0`):
A: (10,0x52). B: (10, {9,0x53,0x54,0x55,0xB,0xF,0x1E,0x1D,0x20,
0x1F,0x33,0x32,0x58}). C: (10,{0x51,0x50}). D: (14,2).
E: (10,{0x1B,0x1C}). F: buildings (10,0x2D) with
`bldgprm[par1].byte_2 & 0x10`. G: (10,0x2D) without. Consume =
`type_0x30311 = 0` after each spawn (Events.cpp:168 etc.). Port
note: our pool spawns directly (no queue); direct spawn in pass
order = APPROX equivalent for creators that only create entities.

There is NO MC1-style load-time terrain-feature fixpoint — MC2
terrain is pre-generated (GenerateLevelMap_43830 :39462; our baked
planes come from the mc2-genlevel oracle).

### Record states (the class-0 resolution)
- Runtime consume = `type_0x30311 := 0` (:32991, Events.cpp:168…).
- `DisId == -1` (0xFFFF) = spawn-at-load (GenerateEvents).
- `DisId >= 0` = disposition-gated; consumer = `sub_4A1E0(dis,
  consume)` (:32950): scan `type != 0 && DisId == dis` →
  `sub_4A310(entity)`; dis 0 = level init. A stage-var pre-pass
  `sub_122C0(dis)` (:32967→:4961) reacts to fired dispositions —
  slice models don't touch it (their ctors set fixed actionIndex).
- Class-0 ON DISK = Conditional Spawn CONTENT (consumed by the
  stage machinery, not the disposition scan) — never treat class 0
  in the authored table as empty at decode; runtime emptiness is
  its own consumed state.

### Creator dispatch
`IfSubtypeCallCreatingManaSphere_4A190(pos, class, subtype)`
(Events.cpp:5186): guard `str_D4C48ar[class].dword_14[subtype]`
`.dword_10 != 0 && .word_4 == subtype` else None (FAIL-SOFT — the
misfit path). Enabled-but-unmapped address → deliberate crash
(Events.cpp:5176-82) = engine bug, port as unreachable!().
`sub_4A310` (:32999): center position `axis<<8 + 128`, z from
terrain, create, then per-class post + `sub_58DA0` stage binding.

### Stages (single-stage subset; level-000 = [5,5,7,0,7])
- Registry: `stages_0x3654C[8]` global + `stageIndex_0x36E01`
  count; per-player `struct_0x3659C[8]` {IsLevelEnd_0,
  ObjectiveText_1 (current stage), ObjectiveDone_2 (pause),
  stage_0x3659F[8] (0 inactive/1 active/2 done)}
  (LevelStructs.h:146-196, 247-263).
- `InitStages_58940` (:40567): per used checkpoint (index_0 != -1):
  drop entity-typed {1,2,4,6,7,9} with stage_1==0; register type;
  ACTIVATE for all players (stage_0x3659F[idx]=1); fill per type —
  0: target % = stage_1; 5: point = axis<<8, 7: store TARGET
  MODEL = `entity_0x30311[stage_1].subtype` (:40628-30).
- `sub_58DA0` (:40650) binds spawned entities ONLY for types
  1/2/4 (pointer match → live entity ptr, flag |=1), 3 (player
  color), 6 (index). **Types 0/5/7 need NO binding** — level-000
  needs none.
- `sub_58F00_game_objectives` (:40693), EVERY tick (:31817), per
  player (single-player v22=1): skip stages not active; type 0:
  `100*(player.dword_0x13C_316 + castle.mana_0x90_144) /
  str_index_242ar.dword_4 >= target%` (needs castle; denominator =
  world total); type 5 (CURRENT stage only): `|dx| <= 768 &&
  |dy| <= 768` vs stored point (768 = 3 tiles in 8.8); type 7
  (CURRENT stage only): `bytearray_38403x[model] == 0` — the
  per-model live-list head is null ⇒ "kill THING" really = "model
  extinct". On satisfy: stage=2; advance ObjectiveText_1 to next
  active stage, or set `IsLevelEnd_0 = 1` (:40898) when none
  remain; transient `byte_0x36E02 = 1` (:40910) drives
  presentation.
- Win consumption: IsLevelEnd_0 = the durable per-player latch (MC2
  analog of MC1's +13325 bit 2); byte_0x36E02 = transient. Both in
  the save blob.
- Single-player collapse of `struct_0x3659C[8]` → 1 slot: SAFE
  (only LevelIndex slot is read; types 3/8 unreachable
  single-player).

## C. The three slice creature machines (class 5)

Dispatch: per-entity update `sub_57730` (:40116): row =
`str_D4C48ar[class].dword_10[actionIndex]` (:40130, must match
word_4 + dword_10 valid, else disable-draw), call handler, then
`byte_0x3E_62++` (phase counter — increments AFTER the handler,
same as our f63). Class-5 handler table `x_DWORD_D4C52ar_str50[236]`
(:1242-1479); handler address = 0x1E1000 + sub offset.

Block structure (base = 8*k), slot roles: +0 patrol (sub_1BD90),
+1 idle/SPAWN state (sub_1BF90), +2 chase-attack (sub_1C310, takes
attack callback), +3 pack-follow (sub_1C560), +4 PreKill
(PreKillEntity_1C890), +5 Kill (KillEntity_1C930), +6 FLEE/hit
(sub_1C980), +7 re-enter (sub_1D5D0 by StageVar2).

Entity-field mapping to our Ent (established for the port):
actionIndex_0x45_69→tick70; byte_0x3E_62→f63; yaw_0x1E_30→f30;
roll_0x20_32 (target yaw)→f34; word_0x96_150 (target)→f146;
word_0x32_50 (leader)→f52; byte_0x39_57 (awake, 0xFA dead)→f58;
mana_0x90_144→f140; subSpellIndex_0x2A_42→f44; dword_0x10_16
(scratch/invis)→f26; id_0x1A_26→id24; rand u16→rand (U16 chassis);
melee mailbox str_0x5E_94 {damage, attacker} → mail ch0 + tag.

### Vulture (5,1) — block 8, ctor AddCreature_4B490 :33720
Ctor: action=9, min/maxSpeed=54/18 (act=18), maxLife=600,
SetEvent144_49C70 → mana = maxLife>>1 = 300 (:32826), yaw/roll/
pitch=0 — **NO ctor RNG draw**; f26-analog=(slot)%100; row 98;
awake=row.v_26+1; sprite 238; NO ShiftRot call. Ground creature
(hoverHeight v_12=0).
States: 8 patrol (sub_1BD90(8); 1 LCG + snd 46 if rand%0x4D==0;
action==14→actSpeed=minSpeed); **9 idle/spawn** (sub_1BF90(8); 1
LCG + snd 46 %0x4D :11401); 10 → set action 14 + actSpeed=min +
HitGoat; 11 pack (sub_1C560(8) + LCG/snd); 12 PreKill(8); 13 Kill;
14 FLEE (sub_1C980(8); on exit actSpeed=max; LCG %0x2B snd 46);
15 re-enter (sub_1D5D0(8) + LCG/snd + speed by action).
Aggro: flags bit 8 → target found ⇒ action = base+6 = 14 (FLEE) —
NEVER attacks (no base+2 in block 8). Kill-credit: model 1 COUNTS.
Death: 12→13, Kill gated `!(f63 & 7)`; mana 300 → 1 sphere
(TransformEntityToManaSphere_36BA0 :26867, class-10 m39) + corpse
sphere (10,1) unless byte[2]&0x10; disable-draw.

### Archers (5,4) — block 32, ctor AddArchers_4BA10 :33878
Ctor: action=33, minSpeed=30, **maxSpeed=0 (stationary)**,
act=min=30, maxLife=1000, mana=500; **ONE ctor LCG draw** :33891 →
roll=yaw=(rand&0x7FF)-1, pitch=roll; f44=500; row 75; awake =
(v_26 - f63 % v_26) + 4 (:33902); sprite 0; ShiftRot(128,256).
States: 32 patrol (sub_1BD90(32); action==34→sub_20060);
**33 idle/spawn** = CUSTOM brain sub_1FAA0 (:11635): scans the
all-entities list within v_28²=5120² and FOV cone (v_30=0x200);
candidate must be model 0/1 with wizard `word_0x248_584` set
(:11799) → f146=target, action=34; own-model pack scan → 35; life<0
or dead linked sub → 36. 34 attack: sub_1C310(32, cb sub_1CCE0)
(:9240: gated f63 % v_26==0 + range) → cb :9713 spawns class-9
MODEL-13 ARROW via (9,13) creator (AddEvent09_0D_4DAB0 :35031:
speed 384, life ≈ 13, sprite 195); aim yaw=tan2, pitch, z += fov/2;
arrow f44=250; also sets target building morale 200 (:11900).
Cadence: one arrow per 30-tick window. 35 pack (→34 possible);
36 PreKill (skip if f26-invis); 37 Kill; 38 hit-slot sub_20130 (NOT
sub_1C980 — Archer does NOT flee); 39 re-enter (clears invis).
sub_20060 aim-recover: 1 LCG, actSpeed=0, sprite 206 or 1 by
rand%0x14<=10, ShiftRot(128,256), copies target class/model.
Kill-credit: model 4 COUNTS. Death: mana 500 → 1 sphere + corpse.

### Villager (5,13) — block 104, ctor AddVilliger_4BF40 :34037
Ctor: action=105, min/max=54/18 (act=18), **TWO ctor LCG draws**:
#1 :34048 → roll=yaw=(rand&0x7FF)-1; then maxLife=1000, **mana=0**
(no SetEvent144), f44=500, row 100, awake=64, f26=2; after map-add:
draw #2 :34065 → `rand % 9` sprite pick (0-2:242, 3-5:271, 6-7:241,
8:239); ShiftRot(128,128).
States: 104 → set 105 + run; **105 idle/spawn** = CUSTOM townie
brain sub_23340 (:14506): mailbox/life bookkeeping; shared move
sub_1B8C0; on period (f63 % v_26==0): rally to class-10 m45 flag
entity within 0x800 (→108 + morale) OR pick nearest building from
the buildings list whose `bldgprm[f71-analog].byte_2 & 1` (:14617)
and walk toward it (actSpeed = maxSpeed + 12 :14637); wander turn =
two-draw idiom. On damage: set attacker's building morale 200
(:14561), → action 110 (flee) or 108 (die). 106/107 → set 105 +
run. 108 PreKill (+ morale hit :14678); 109 Kill; 110 FLEE
(sub_1C980(104); on exit clear f146, actSpeed=max); 111 re-enter
(110→minSpeed else max). NO attack state (no sub_1C310).
Kill-credit: model 13 EXCLUDED (:9549 set {9,12,13,14,15}).
Death: mana=0 → no mana spheres; corpse sphere only.

### Shared movement core sub_1B8C0 (:8741) — the MC2 creature gate
1. predicted pos: terrain-follow sub_580E0 (:40372: `if z > alt,
   z += v_14 (negative); if z <= alt + v_12, z = alt + v_12`;
   all slice rows v_12=0 → ground clamp), then MoveEntity_57FA0
   (yaw, pitch 0, actSpeed).
2. COMMIT GATE (:8806-8810): if tile changed AND (sub_102D0 entity/
   obstacle collision (:3632) OR sub_1B7A0_tile_compare(pred) >=
   v_16 blocked-height threshold (Terrain.cpp:1578)) → REJECT, turn
   +341 and retry (4 candidates: +341, −341/−85, mirrored :8843).
3. ALL FOUR BLOCKED (:8855): `if (row.flags & 1) || tile is water
   (sub_104D0 == 1, Terrain.cpp:2058) → life = -1` + global
   creature counter decrement — the water/blocked-flag suicide,
   the survey's "creature commit gate".
4. Commit + turn toward f34 capped ±v_4 via sub_58350 (:8868),
   mask 0x7FF. Result code 4 = blocked/killed.
Special: struct_byte[1] & 8 → force result 4 (:8786).

### Shared primitives
- sub_1BD90 (:8945) / sub_1BF90 (:9064) — patrol/idle two-scan:
  read melee mailbox (damage, attacker :8966/:9097), propagate
  weakest linked life, target scan; transition = flags bit 8 ?
  base+6 : base+2; life<0 → base+4; pack-mate → base+3; else move +
  altitude commit sub_1EEE0 (:11172).
- Wander-turn idiom (in sub_1BF90 :9136-38, sub_1FAA0 :11756-59,
  sub_23340 :14607-11): draw v = rand; draw rand again; `f34 +=
  ((rand & 0xFF) + 85) * (2*((v % 0x9D)/79) - 1); f34 &= 0x7FF`.
- sub_1C310 (:9240) chase-attack: validate target sub_1ED30, aim,
  fire callback when f63 % v_26 == 0 in range.
- sub_1C560 (:9345) pack-follow: follow f52 leader, match speed.
- sub_1C980 (:9572) FLEE: every `f63 & 3` re-aim AWAY from threat
  (`f34 = tan2(self, threat); HIBYTE += 4; & 0x7FF` — the +4 high
  byte = 180° flip), then sub_1B8C0.
- PreKillEntity_1C890 (:9533): chain subentities to state+5;
  kill-credit gate :9549 (player-killer class 3 model 0, victim
  model NOT in {9,12,13,14,15}) → creaturesKilledPercent_373++.
- KillEntity_1C930 (:9556): gated `!(f63 & 7)`; mana spheres
  (:26867: only if mana>0, split ≤16 spheres, LCG per sphere
  :26902-10) + (10,1) corpse sphere + disable-draw.
- Melee mailbox write sub_11900 (:4375): accumulate damage +
  attacker id (none of the three slice creatures emit it).
- Common LCG moduli: %0x4D(77), %0x2B(43), %0x9D(157)/79, %0x14,
  %9, &0x7FF, &0xFF.

### Flight (Phase-4 bank, from the spec review)
MC2 climb law = row-driven linear ramp (:59645:
`((z − ground − row.v_10) << 10) / row.v_10` clamp ±256, authority
= −that), commit gate zeroes target speed on block (:59602,
moveTest_5D0A0) + resets Shield entity field; mover reads player
extension {moveSpeed_0x14C_332 slow, mobilizeCounter_0x14E_334
full-stop, strafeSpeed_0x10_16, moveBoost_0x1E_30} (:59610-99) —
a real port with its own state struct, NOT a re-parameterization.
