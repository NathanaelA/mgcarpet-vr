# AUDIT — authored THING-field consumption vs the port (2026-07-17)

Trigger: the mc2:23 spawn-embedded-in-rock report. Root cause was a whole
CLASS of bug: retail consumes authored THING par fields in per-model
creation arms, and any arm the port didn't wire silently drops authored
data ((10,82) room carves ran at ctor defaults 3/3/2 instead of their
authored extents). This audit asks: **how many other record types are
ignored the same way?**

Method, three legs cross-checked:
1. **Retail consumption matrix** from the decompiles (remc1/remc1hw/remc2),
   per (class, model) × creation path — background opus agents, citations.
2. **Port consumption matrix** read out of `mc1/world.rs` (the spawn
   dispatch + post-init) and the chain/table subsystems.
3. **Authored-data census** over every baked level: which records actually
   carry nonzero fields (severity weighting — a gap on a never-authored
   field is theoretical; a gap on 300 records is a bug).

A field being AUTHORED does not mean retail CONSUMES it (editor junk is
real — e.g. ~540 (10,83) domes carry a nonzero par1 no engine path reads).
Only retail-consumed ∧ port-ignored ∧ data-authored = a finding.

---

## MC1 + MC1HW — VERDICT: CLEAN, no gaps

Verified by full-body decompile walk (agent report, 2026-07-17):

- ALL authored-field consumption at MC1 entity creation happens in ONE
  function, `sub_37560_37920` (remc1 sub_main.cpp:43988-44058), shared by
  the load path (`sub_37440_37800(0,1)` :51570/:51605, rows with
  `data_8 == 0`) and the disposition path (same walker, 15 trigger call
  sites :67230-67502). One law, both paths — unlike MC2.
- Per-class creators receive ONLY the position `axis_3d`
  (`sub_373F0_377B0` :43914-19) — no creator can read record fields.
- The port's post-init match (world.rs:5361-5554) reproduces every arm
  with the same constants: (10,4) swi_id→id24 + swi_sz²→extents; (10,34)
  child/parent→dest tile centers; (10,45) par1+16→build id; class 11
  swi_id→id24 + (swi_sz<<8, 4096) extents + flag bit0; class 12 swi_id
  state bump incl. the ≥3 blue-jar split (−3, sprite 280, flag |4).
- Confirmed nothing-consumed: classes 0–9 (all models), class 10 except
  4/34/45, class ≥13. The MC1 census's nonzero swi_id on creatures/
  creators is retail-ignored at creation too → the port ignoring it is
  FAITHFUL.
- MC1HW `sub_37560_37920` is byte-for-byte identical → same verdict.
- Out of scope, separately owned: `GenerateFeatures_36430_367F0` (the
  load-time TERRAIN pass) consumes map records for terrain models — a
  different subsystem, already ported as the feature pass.

## MC2 — port consumption matrix (leg 2, read out of the code)

Spawn dispatch (world.rs `spawn_from_thing`) + post-init, MC2 arms:

| (class, model) | fields consumed | destination |
|---|---|---|
| (5,22) worm | par1 | tail length |
| (10,4) spawner vol | swi_id, swi_sz | id24, extents (MC1-shaped arm) |
| (10,9)/(10,11)/(10,15) | par1 | SPELLS tier → subspell + maxLife/life |
| (10,22) whirlwind | par1 | tier → 8×life |
| (10,34) portal | parent, child | dest tile centers |
| (10,45) building | par1; par2 | BLDGPRM id (raw); f66 |
| (10,54) aura | swi_id (stageTag) | range + life |
| (10,67) flood | par1 (DIS-fired only) | SPELLS row-20 tier |
| (10,82) room carve | par1, par2, par3 (LOAD only) | f67/f68 half-extents, f71 depth — **ADDED 2026-07-17, the mc2:23 fix** |
| (10,83) dome | swi_sz (word_10) | radius |
| (10,84)/(10,85) pit/hill | swi_sz; par3 | radius; depth/height seed (+ tile-corner recentre) |
| (11,32) stage switch | swi_id; par1 | id24; stage row |
| (11,\_) switches | swi_id; swi_sz | id24; extents (h=4096) |
| (12,\_) | swi_id | state bump (MC1-shaped arm) |
| (14,1) riser | par1; par2 | orientation; length |
| (14,2) cave pillar | par1; par3 | orientation; half-width koef |
| (15,\_) spell tokens | swi_id (stageTag) | state bump (≥3 → junk 253) |
| chains (10, 28/29(0x1D)/31/50/80) | par1/par2 links; swi_id guard; par3 (0x1F/0x50) | `mc2_author_chain` = sub_49090 port — road/path/river/fence stampers + (10,80)→(10,81) tube carver with packed par3 radii |
| stage/StageVar binds | swi_id + stage rows | `mc2_bind_stage_target` / `mc2_stagevar_attach` |

Everything else spawns from (x, y, ground-z) only.

## MC2 — retail matrix (leg 1, decompile-verified)

Structural facts (agent walk, 2026-07-17, all cited):
- THING = `type_entity_0x30311`, 20 bytes (BasicTerrain.h:7-18);
  `entity_0x30311[1200]`; no field remap on load.
- TWO paths, DIFFERENT laws: LOAD = GenerateEvents_49290 →
  PrepareEvents_49540 (only DisId == −1, classes 0x0A/0x0E); DIS =
  sub_4A1E0 → sub_4A310 (every DisId ≥ 0 group — **dis 0 fires at
  init**, it is NOT a load-path marker).
- CTOR-DIRECT consumption: NONE — every creator takes only `axis_3d*`;
  all customization happens after the ctor returns. ApplyEvents settle
  consumes nothing. (Same shape as MC1.)
- PrepareEvents' class-0x02 arm is DEAD CODE on the load path (no
  GenerateEvents pass feeds class 2) → class-2 scenery pars are
  retail-unconsumed at creation. The census's constant par1 = 1 on
  7343 class-2 records is editor junk. Port ignoring it = FAITHFUL.
- LOAD consumers: (0x0E,2) par1/par3; (0x0A) 0x2D par1→bldg;
  9/0xB/0xF par1→SPELLS tier; 0x52 par1/par2/par3→carve box;
  0x53 word_10; 0x54/0x55 word_10+par3; 0x58 par1/par2→pitch/roll;
  chains {0x1C,0x1D,0x1F,0x32,0x50} via sub_49090.
- DIS consumers: everything in the port matrix above PLUS
  **0x11 (meteor) par1→subspell+maxLife+life**, **0x16 (whirlwind)
  par1→subspell+8×life**, **0x43 (flood)/0x47 (fissure)
  par1→subspell+life**, 0x3D/0x3E par1/par2 (never authored),
  (0x0A,0x2D) par2→xtype (DIS only; no load record authors par2),
  (0x0A,0x22=34 portal) par2/par1→dest, class 0x0B/0x0C/0x0E/0x0F arms.
- `sub_49090` chain walk: the type/subtype guard at EV:5316-19 tests
  the loop-INVARIANT seed, NOT the walked node — **retail never
  class-checks chain nodes**; termination is par2 == 0 only. Chains
  freely cross passive rows of any class (incl. class 0).
- CLASS 0: can NEVER materialize (guard EF:32982 `type_0x30311 &&`;
  creator-table null check EF:33012). "Conditional Spawn" is a
  misnomer — class-0 rows are PASSIVE DATA addressed by slot index
  from active records: chain endpoints (x/y/par3 reads, stageTag
  writes), objective targets (type-9 reads the slot's par1),
  StageVar subtype resolution, texture-preload census.

## FINDINGS (leg 1 ∩ leg 2 ∩ leg 3)

| # | record | gap | shipped data | status |
|---|---|---|---|---|
| F0 | (10,82) room carve | LOAD par1/par2/par3 → carve box dropped | 333 records, all authored | **FIXED 2026-07-17** (the mc2:23 trigger) |
| F1 | (10,17) meteor | DIS par1 → subspell + maxLife + life dropped | 69 records, 50× par1 = tier 1-2 | **FIXED** (post-init arm + test) |
| F2 | (10,71) fissure | DIS par1 → subspell + life dropped | 21 records, all par1 = 2 | **FIXED** (post-init arm + test) |
| F3 | (10,22) whirlwind | subspell (f140) missing from the tier arm | 52 records par1 = 2 | **FIXED** (f140 added; 8×life both paths stays — the documented player-ruled unification) |
| F4 | chain walk | per-node class/model break = decompile misread (guard tests the seed) | 2 chains in level 151 (stageTag zeroing only; zero terrain legs differ) | **FIXED** (check removed, cycle guard kept) |
| F5 | (10,67) flood gate | `dis_id != 0` — wrong sentinel (load = 0xFFFF; dis 0 fires at init) | shipped-data neutral (all 49 are disN) | **FIXED** (gate = `!= 0xFFFF`; flood test re-pinned to the retail law) |

Verified NON-findings (port already correct / faithful):
- MC1 + MC1HW: complete and byte-exact, zero gaps.
- Class 0: never spawns in retail either; our slot-indexed `build_table`
  keeps class-0 rows addressable, so all passive-row reads (type-9
  objective par1 world.rs:4639, StageVar `mc2_table_model`, chain hops)
  already work. `fire_disposition`'s `class != 0` skip ≡ retail EF:32982.
- Class-2 par1 = 1 (7343 records) and (10,83) par1 (≈540 records):
  authored-but-unconsumed in retail → correctly ignored.
- Chains/portal/building/switches/tokens/jars/worm/aura/domes/pits:
  arm-for-arm match (see the port matrix).
- (10,84)/(10,85) anchor parity: LOAD spawns at tile corner (predicted
  axis has no +128), DIS spawns center then −128 — both end at the
  corner; our center-spawn + unconditional −128 matches. ✔

Theoretical-only (retail consumes, ZERO shipped records — port only if
custom levels ever need them; all three are outside `known_thing` today):
- (10,0x3D=61)/(10,0x3E=62): DIS par1→f71, par2→f26.
- (10,0x58=88): LOAD par1/par2 → array pitch/roll.

WATCH (side finding, not chased): the (10,83) dome LOAD position law —
PrepareEvents' predicted axis is the tile CORNER (x<<8, no +128) while
our dispatch spawns everything at center; the dome tick's
`(x + 128) >> 8` anchor math expects corners. Whether our domes sit a
tile off on the load path deserves a dedicated look (cave levels are
player-certified, so if it's off it's subtle). Same question does NOT
apply to 0x54/0x55 (resolved above) or 0x53's DIS path (no recentre in
retail either).

## MC2 — authored-data census highlights (leg 3)

Sentinel-aware census over all baked MC2 levels (65535 = none; class-2
records carry swi_sz = 0xFFFF sentinel, dis 0):

- Chains (10,28)/(10,29)/(10,31)/(10,50)/(10,80): par1/par2 links on
  ~75-90% of records, (10,31)/(10,80) par3 heavily authored → ALL
  consumed by `mc2_author_chain`. ✔
- (10,82): 333 records, par1/par2 on ALL, par3 on 307 → was 100% dropped
  until the 2026-07-17 fix. ✘→✔
- (10,45) buildings: par1 on 3495 records, par2 on 278 → consumed. ✔
- (10,84)/(10,85): par3 on ~75% → consumed (z seed). ✔
- (10,83): par1 nonzero on ~540 records (values 1..4) — retail consumes
  only word_10 on BOTH paths → par1 is editor junk; ignoring = faithful.
- class 2 (scenery/bee/falling, 7343 records): par1 = 1 constant —
  PrepareEvents' class-0x02 arm is dead code (no load pass feeds it) and
  the DIS default is stage-bind only → junk; ignoring = faithful.
- class 0 "Conditional Spawn": ~7000 records — retail can NEVER
  materialize them (guard EF:32982); they are passive slot-indexed data
  (chain endpoints, objective/StageVar targets), which our slot-indexed
  table already serves. Resolved — see findings.
- (10,17)/(10,71)/(10,22): see findings F1-F3.
- (14,1): par2 on all 158 (consumed ✔), par1 on 75 (consumed ✔).
- (5,22) worm: par1 on all 191 → consumed. ✔
