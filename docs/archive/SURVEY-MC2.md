# Phase-0 survey: remc2 vs the MC1 port (2026-07-09)

The ROADMAP "MULTI-GAME ARCHITECTURE" Phase-0 deliverable: the
chassis diff and the tier-5 verb inventory, produced by four parallel
survey passes over `reference/remc2/remc2/engine/` against our
`crates/mgc-sim` port (and `reference/remc1` where the original MC1
was the better comparison side). remc2 citations are
`EventsFunctions.cpp` unless noted.

**Kill-criterion verdict: PASSED, decisively.** The MC2 developers
demonstrably reused MC1's chassis themselves — same LCG constants
(9377/9439), same two-stack allocator, byte-identical 6-channel
damage-mailbox protocol, same tile chains, same disposition machinery.
The ChassisParams surface is ~16 items, half of them table sizes and
struct offsets that disappear in a native Rust struct. The one-sim
superset architecture stands.

---

## Survey 1: CHASSIS

| # | Subsystem | Verdict |
|---|-----------|---------|
| 1 | Entity pool & allocator | **IDENTICAL** (1000 slots, slot-0 sentinel, 999→1 build, LIFO, two-stack reclaim, silent-null exhaustion) |
| 2 | RNG | **IDENTICAL** constants (9377/9439, one global draw/tick, seed = slot + global); per-entity state width u32@4 → u16@20 |
| 3 | Entity struct | **PARAMETERIZED** (164→168 bytes; every MC1 field survives at an offset shifted 0–8) |
| 4 | Damage mailboxes | **IDENTICAL** protocol (6 ch × {u32 amount, u16 source}, accumulate-vs-overwrite, owner immunity, accept mask, grace memset 36 bytes); base 90→94 |
| 5 | Tick loop | **PARAMETERIZED** (same 4-phase order; + 1/4/8× speed multiplier, 3 extra pre-pass subsystems, buckets 20→29) |
| 6 | Triggers/dispositions | **PARAMETERIZED core + EXTENDED** (disposition spawn + class-11 volumes verbatim incl. the 10-tick debounce; new stage/objective layer) |
| 7 | Tile chains | **IDENTICAL** (256×256 heads, intrusive doubly-linked u16 chains, flag 0x4; link fields shifted +2) |

### 1. Entity pool

remc2: pool = `D41A0_0.struct_0x6E8E[1000]`, 168 bytes each
(LevelStructs.h:237), pointer LUT `Entities_EA3E4[1001]` (Basic.h:195,
filled EventsFunctions.cpp:43372), index 0 = chain sentinel, loops run
`1..0x3e8`. Allocator `NewEvent_4A050` (Events.cpp:561-607) pops a
primary free stack; on exhaustion falls back to a second
"reclaimable" stack (entities flagged `byte[2]&2`), forcibly
unlinking and stealing one (Events.cpp:581-605); both empty →
**return 0 silently** (Events.cpp:606), callers null-check. Free
stacks built in `sub_49F90` (Level.cpp:1271-1301): **999 down to 1**
(allocation order 1, 2, 3, …). Frees are LIFO (`sub_57F20`,
Events.cpp:5209-5240, which also linearly removes from the reclaim
stack).

MC1 original is the same two-stack design: `str_29795[1000]` +
`var_u32_593[1000]` + `var_u32_4597[1000]` (remc1 Basic.h:562,550,552);
`NewEvent_372C0_37680` (sub_main.cpp:43865-43911) is line-for-line the
MC2 allocator — same defaults (maxLife 300, flags 8, speed 16,
strength 100, id = own slot, xClass/xModel −1, +58/57 = −6, +68/67 =
10), same fallback, same return 0.

Our port omits the reclaim second stack (never hit in MC1 practice) —
a known simplification, and now a CHASSIS item since MC2 has it too:
grow it once, then it's shared.

Clarification: the "2000-entry table / 1999 usable" figure is the
level THING table (MC1 `str_1072[2000]`, our `TABLE_SLOTS`), not the
runtime pool. MC2 shrank it to **0x4B0 = 1200**
(`Type_Level_2FECE.entity_0x30311`).

### 2. RNG

- Global LCG `x = 9377·x + 9439` in both. MC2 `D41A0_0.rand_0x8`
  (u32), drawn exactly once at the top of every tick
  (`UpdateEntities_57730` :39947) before the pause gate — same as MC1
  (:52223) and our port (world.rs `tick`). Handlers also draw the
  global stream ad hoc in both games.
- Per-entity streams seeded at allocation `slot + global_rand`
  WITHOUT advancing the global — MC2 Events.cpp:578, MC1 :43881,
  ours features.rs new_event.
- **One real delta:** MC2 stores the per-entity seed as **u16 at
  offset 20** (layout byte-verified against DOS memory snapshots,
  engine_support.cpp:1287) where MC1 uses u32 at offset 4. 9377 ≡ 1
  (mod 4) so still full-period mod 2^16, but sequences differ — a
  true behavioral parameter, not just an offset.

### 3. Entity struct (field map, MC1 → MC2)

164 → 168 bytes; the MC1 shape survives completely recognizably:

| Field | MC1 (ours features.rs Ent) | MC2 |
|---|---|---|
| chain scratch ptr | @0 | @0 |
| per-entity rand | @4 u32 | **@20 u16** |
| maxLife / life | @8 / @12 | @4 / @8 |
| flags dword (dead 0x400, linked 0x4, accepts-damage 0x8) | @16 | @12 — same bit meanings |
| tile-chain next/prev | @20 / @22 | @22 / @24 |
| id/owner (disposition fired) | @24 | @26 |
| damage-accept channel mask | @28 u16 | **@56 u8** |
| phase counter (slot at alloc, ++/tick) | @63 | @62 |
| class / model | @64 / @65 | @63 / @64 |
| target class/model filter | @66 / @67 | @65 / @66 |
| state / tick-handler index | @70 | **@69** (`actionIndex`) |
| position x/y/z (8.8) | @72/74/76 | @76/78/80 |
| damage mailboxes | @90..126 | @94..130 |
| act/min/max speed | @126/128/130 | @130/132/134 |
| mana regen/max/act | @136/140/144 | @136/140/144 |
| behavior row + player extension ptrs | @156 / @160 | @160 / @164 |

The per-player wizard extension grew from MC1's Type_160/164 into
MC2's 1136-byte `type_str_164` with the 26-spell XP/level tables
(global_types.h:222-336) — per-game payload, not chassis.

### 4. Damage mailboxes

MC2's area writer `sub_10C80` (:3953-4030) is MC1's writer
(:17240-17330) verbatim: channel mask `1 << ch` vs accept mask; owner
immunity by id equality only; optional xtype/xsubtype filter; write
protocol source-pending → `amount += dmg` else `amount = dmg`, then
`source = attacker`; delivery walks per-tile chains in a radius
square. Six channels, 6-byte stride, indexing `entity + 6·ch`.
Drained in the per-entity state handlers, not centrally — readers
clear the source but NOT the amount. Wizard spawn-grace wipe = the
same `memset(&mail, 0, 36)` gated on a grace counter.

### 5. Tick loop

MC2 `UpdateEntities_57730` (:39928-40185): (1) one global draw;
(2) dead sweep (`byte[1]&4` → free); (3) bucket build — per-category
intrusive chains via @0: class 3 alive, class 5 by model into **29**
heads (skipping states 0xB4/0xE8/0xEA), class 9, class 10/11 subsets
into 5 registers; (4) if unpaused: `sub_12780`, stage-var reaction
pass over creature chains (`sub_12500`), AI `sub_68BF0`
(single-player), `sub_159E0`, cave `sub_58630`, `sub_60F00`, then
main dispatch in slot order 1..999: handler =
`str_D4C48ar[class].dword_10[actionIndex]`, guarded by a state-id
echo check (`word_4 == actionIndex`, mismatch HIDES in MC2 where MC1
DELETES), call, `byte_0x3E_62++`; finally `sub_585D0`.

MC1 (:52197-52420) identical phases with 20 heads + 4 registers,
dispatch on byte-70.

Frame orchestration (MC2 :31763-31822): input → `PlayerEvents_51BB0`
(wizard tick, ONCE per frame) → `UpdateEntities_57730` × **1/4/8 by
game speed** → lighting → objectives → sounds → draw. The player
tick lives OUTSIDE the multiplied loop.

### 6. Triggers/dispositions

Dispositions fully survive: `sub_4A1E0(dis, consume)` (:32950-32997)
= MC1's sub_37440_37800 = our fire_disposition; dis 0 = level init.
Class-11 volumes survive with the same constants: fly-into/fly-out
probing player AABBs, firing the id26 disposition with the same
10-tick rearm debounce; kill trigger tests the per-model chains with
a 16-tick confirm. EXTENSION: an objective/stage layer MC1 lacks —
`stages[8]`, `StageVars2[11]`, per-record `stageTag_12`,
objective-gated switches, per-creature stage reaction pass
`sub_12500` — plus multiplayer-aware probes.

### 7. Tile chains

`mapEntityIndex_15B4E0[65536]`, tile = `(x>>8) + ((y>>8)<<8)`,
intrusive doubly-linked u16 chains, linked-flag 0x4 — identical
design; fields shifted +2.

### Proposed ChassisParams (MC1 → MC2)

Pool/tables: (1) level_table_slots 2000→1200; (2) pool_slots
1000→1000 (parametric anyway); (3) reclaim second stack — in BOTH
originals, our port must grow it once (shared, not a param);
(4) entity stride 164→168 (moot in Rust).

RNG: (5) ent_rand_width u32→u16 (true behavioral param); LCG
constants/seeding/one-draw: shared.

Dispatch: (6) bucket_models 20→29; (7) bucket predicates + excluded
states ({120} → {0xB4,0xE8,0xEA}); (8) state-mismatch policy
delete→hide; (9) the per-class × per-state handler table (THE
swap-wholesale surface; keyed @70 vs @69 + echo check);
(10) ticks_per_frame 1 → {1,4,8} with the player tick outside the
loop; (11) tick pre-pass hook list (MC1 awake pass → MC2 stage
reactions/AI/cave/census hooks).

Damage: (12) per-channel mailbox SEMANTICS per game (MC1 ch1 = mana
claim, ch4 = grip, ch5 = balloon recall; MC2 assignments need their
own trace); (13) accept-mask default + wizard grace-counter location
(per-game wizard-extension layout).

Triggers: (14) trigger-condition state sets (MC1 set ∪ MC2
objective-stage + multiplayer probes; debounce 10 / confirm 16
shared); (15) the MC2 stage machine as an optional module.

Allocator defaults (maxLife 300, flags 8, speed 16, strength 100,
awake −6, explode-class 10): identical in both binaries — shared.

---

## Survey 2: DAMAGE APPLICATION, MANA ECONOMY, CORPSE PIPELINE

Big picture: all three verbs keep the MC1 protocol shape; the
differences are constants plus XP hooks bolted onto existing events.
An XP "decorator" over the events our combat.rs already emits covers
the flagship MC2 feature.

### Verb: damage application — PARAMETERIZED

- Mailbox search-apply identical (writers sub_10C80 :3953,
  sub_11400 :4208, sub_116A0 :4310, single-target sub_11900 :4375;
  ~30 self-apply sites in creature handlers). New constants:
  class-2 model-0 buildings take amt/10 (:4282-4286); castles exempt
  from generic appliers (:4294).
- Wizard intake `sub_5EFA0` (:60613) vs our sub_46540 port:
  - Shield quarter paid by mana — IDENTICAL formula (:60682-60688):
    `dmg/4`, `mana -= dmg/4`. Armed by the Shield SPELL's flag, not
    passive; a second flag fully nulls one hit then re-arms quarter
    mode (:60693-60698). Each absorbed hit = +1 shield XP (:60678).
  - Knockback dmg/10 IDENTICAL, new clamp 0..80, direction recorded.
  - Regen stall 16 ticks IDENTICAL (:60664, :60716; life only, mana
    keeps flowing).
  - Grace IDENTICAL protocol: spawn value 100 (:43711), while >0 the
    36-byte mailbox memset (:59984-59987).
  - At-castle redirect IDENTICAL: forward inbox to castle + grace=2
    (:59968-59978); castle intake sub_609E0 (:61735) straight
    subtract; downgrade sub_605E0 (:61612) one level per lethal +
    10% haircut + ejection.
  - Changed: hit sound = random 54-57 via the LCG (vs MC1 17/16);
    new intake channels: duel grip (+1 duel XP per grip tick,
    :60644-60668), steal-mana handler sub_61050. Death sound 16
    matches MC1.
- Reflexes/Life personality consumed in the AI BRAIN, not the
  intake (loaded :43764-43775, AI only): Aggression → hate pacing +
  target worth `50000 − maxMana/10·aggr/255` (:6559); Perception →
  `rand%255 < p` spot checks (scrolls, :6537); Reflexes → brain
  cadence `tick % (64 − r/4)` (:5459) + turn rate (:6489); Life →
  `maxLife = 10000·L/256` (base 10000 flat, same as MC1). Human
  intake has zero personality coupling.

### Verb: mana economy — census IDENTICAL-in-shape; win REWRITTEN
(objective engine); unlock ladder REWRITTEN (the XP system); regen
PARAMETERIZED

- World-mana census survives: `sub_60F00` (:61959) per tick
  immediately before the entity loop (:40115) — same placement as
  our recompute_mana. Owned (10,39) spheres + castles add to owner
  maxMana; (10,45) carriers into the in-transit bank; world total
  kept.
- Win check demoted into a staged objective engine
  `sub_58F00_game_objectives` (:40693; types at BasicTerrain.h:36-46:
  0 collect mana, 1/7 kill creature, 2 kill fixed entity, 3 kill
  enemy player, 5 release point, 8 kill all players, 9 destroy
  building). Type 0 = exactly MC1's banked-% rule (:40745-40760) but
  castle-gated with NO 16-tick debounce.
- Castle ladders PARAMETERIZED (sub_60810 :61695, 8 levels): HP
  {—,20000,40000,40000,60000,60000,80000,80000} scaled by Life
  personality + per-level factor (top matches MC1 6/7 = 80000);
  capacity {5000,8500,18000,38800,78600,158200,317400,∞}.
- Regen nearly IDENTICAL (human :60022-60040): life /250 home,
  /2000 afield (same as MC1); mana /200 home (min 1000), /2000
  afield (min 100) — MC1's flat /200 became dual-rate with floors.
  AI wizards: life /200 home /500 afield (:5440-5452).
- Spell-unlock economy REPLACED by two mechanisms: (1) spells are
  world entities (15, spell_idx); the book (`SpellsEnabled[26]`)
  stores entity indices; they scatter on wizard death and are
  collectible. (2) The XP system: per-spell XP `spellsExperience`
  + persistent cross-level bank `SpellExperience` (global_types.h:
  174-216; banked sub_6D9C0 :43910-22, copied across levels
  Level.cpp:1264); level = threshold scan vs the spells-table
  `xpos1_E` (:43873); 3 levels per spell; campaign clamps castle XP
  to 7 (:43885).
- XP awards per successful EFFECT, not per cast (primitive
  sub_6D8B0 :58228): fireball +1 per HIT (:63189); possession +1
  (:59052); heal/speed-up/metamorph +1 per use; teleport/invisible/
  beyond-sight/steal-mana +1; lightning +1/variable; TERRAIN spells
  award XP = magnitude (meteor/crater/quake/volcano/tremor/gravity-
  well/whirlwind pass tiles-altered counts); shield +1 per absorbed
  hit INSIDE the intake (:60678); duel +1 per grip tick; castle +1
  on build (:61596); generic impact handlers +1 keyed by impacting
  model (:62985, :63551). Scroll pickup (model 57): +4 XP campaign /
  +50 multiplayer to ALL enabled spells (:41158-41190); AI hunts
  scrolls gated on Perception. Cheats: +100 all (:37867), grant-all
  sub_6E0D0 (:44273).
- Coupling flag: XP hooks live INSIDE the damage intake (shield,
  duel) and impact/corpse handlers — porting XP = decorating exactly
  the events our combat.rs already emits, plus one new event
  (terrain-tiles-altered count).

### Verb: corpse/mana-ball pipeline — PARAMETERIZED

- Kill counter identical concept: `PreKillEntity_1C890` (:9533)
  increments killer's `creaturesKilledPercent_373`, excluding models
  9/12/13/14/15 (townsfolk); level-end stats add spells-collected %,
  hit-accuracy %, mana-collected %, time (:43440-43570).
- Corpse → mana same pipeline: `KillEntity_1C930` (:9556) →
  `TransformEntityToManaSphere_36BA0` (:26867): corpse mana becomes
  (10,39) spheres using the same 9377/9439 LCG, ballistic scatter
  (yaw ±56 of facing, speed rand%48+16, vertical (1024−h)/8);
  `useManaFraction` splits into up to 16 spheres of ~1000 for big
  corpses (:19684); spheres merge on contact (sub_36D50 :26920);
  ownership carries into the census.
- Wizard death same skeleton as our jar scatter + m40 grave
  (sub_5E310 :60052): death fall −2/tick cap −256, kill tally,
  scatters the 26 SPELL entities (not mana jars) at pos ±
  (LCG&0x1FF −256) with life rand%90+200, spawns a (10,40) grave
  timer 1200 and reassigns the dead wizard's spheres to it.

### UNKNOWNs (survey 2)

- sub_61050 (:62082) assumed steal-mana intake — body not fully read.
- Whether cast mana costs scale per spell level — not traced.
- Shield full-absorb flag transition (:60693) reads "first hit free,
  then quarter mode" — unverified against retail.

---

## Survey 3: PERCEPTION / TARGETING / LOS

### Verb: awake / sight-aggro — PARAMETERIZED (same shared two-scan
skeleton, new per-type flag bits; "perception" is NOT a creature input)

- Awake pre-pass: remc2 sub_68BF0/sub_68C70 (:55469/:55494) is the
  structural twin of remc1 sub_54F00/sub_54F80 (:64191-64375) and our
  mob_awake_pass: dead → 250; awake → decrement + segment mirror;
  asleep → grace byte, re-arm 16 (segments 18) inside dist² <
  0x2400000 — the identical threshold. Deltas: remc2 adds an
  early-out flag gate `byte[0]&1` (:55516); segment mirroring order
  flipped (propagate-then-decrement — segments lag one tick); 29
  class lists vs 20; dead sentinel 0xFA identical.
- **PORT BUG FOUND AND FIXED (2026-07-09): the awake distance is 2D
  in BOTH engines.** sub_42410_42750 (:52748, SYNCHRONIZED) reads
  only x/y; our port shipped a 3D test mis-reading :64353. Fixed in
  mobs.rs the same day (fixture goldens unchanged — the scripted run
  flies low). remc2's EuclideanDistXY confirms.
- Shared two-scan (IDLE sub_1BD90 :8945 / WANDER sub_1BF90 :9064)
  mirrors remc1 and our pack_scan/wizard_scan line for line: same
  inbox prologue, same wander-turn LCG constants (9377/9439,
  `&0xFF + 85`, `% 0x9D / 79`), awake gate, Scan A wizard list with
  v_28²/v_30 range/cone + cloak skip `&0x20`, Scan B same-model
  leaderless pack fallback, cadence % v_26. **MC2 confirms the
  universal design — still no per-model aggro list.** Deltas: new
  type-flag byte bits (4 = disable pack scan :9022; 8 = alternate
  chase state base+6 :9170); retaliation vs ANY attacker class
  (remc1 only class-3, :21372 vs :9226-9235).
- MC2 `perception` = rival-BRAIN input only (word_0x244_580, set
  :43765): `rand%255 < p` notice rolls (invisible model-57 spotting
  :5692/:6546), reaction delays on 255−p (:6969-7211). The creature
  verb needs no perception parameter; the rival-brain verb does.

### Verb: targeting / autoaim — PARAMETERIZED (identical scoring &
steer, extended subtype key, one new acquire source)

- Miss-metric scoring: remc2 sub_68490/sub_685D0 (:55101/:55157) =
  remc1 sub_54A90/sub_54BD0 = our aim scorer: yaw/pitch cone gates,
  3D distance ≤ 5120, score = summed squared sin/cos projections
  (one sin table + `4·… >>16` ≡ remc1's two tables `>>14`). Cones
  0x71/0x71 everywhere, same 0x71/0x200 pitch-loosened class-9
  creature scan (:54920 = :64157).
- Subtype-keyed acquire: remc2 sub_67CB0 (:54710) = remc1 sub_54520.
  Key extended: MC1 {0,3,4 | 1 | 7,8,B,C | 9} → MC2 {0,3,4,0x12,
  0x13,0x16,0x1A,0x1C,0x1E | 1,0x11 | 7,8,B,C | 9 | 0x10 | 0x19};
  new case 0x10 = asymmetric wide cone 0x100/0x71; case 0x19 =
  creatures through an eligibility filter. 29 lists (skip 22) + a
  segment-chain fallback + a StageVar exclusion. Case 1/0x11 adds a
  BUILDINGS target source (:55040-55070) + model-39/57 ownership
  cases. MC1's `+26 > 16 → 16` clamp has no remc2 counterpart.
  Class-9 range = speed × maxLife in both. On acquiring a human:
  same notify (sub_5EF70 = sub_46520).
- **New acquire source sub_68940** (:55315), tried BEFORE the
  subtype scan in several movers: a player-designated target-MARKER
  entity (model 78) owned by the shooter, 3D range vs caster v_28,
  fixed cone 0xAA. MC2-only interface input.
- In-flight homing: identical per-tick re-bearing + clamped step;
  sub_58350 (:40391) is byte-identical to remc1 sub_422A0 (:52689);
  caps live in the TYPE TABLES (+2/+4 yaw, +6/+8 pitch) — our
  5/tick cap is data, MC2's cap VALUES are unknown from source
  (table extraction item). Untargeted-acquire gate `byte[0]&2`
  unchanged, so the re-acquire cadence carries over structurally.

### Verb: LOS / height sampling — IDENTICAL algorithm + one MC2-only
surface (cave ceiling)

- Height: remc2 sub_B5C60 (Terrain.cpp:113) is line-for-line remc1
  sub_724C0 / our ground_z: same (cx+cy)&1 diagonal split, same
  three sub-triangle branches, same `(comp>>3) + 32·p1`.
- MC2-only: a SECOND heightmap — the cave ceiling (sub_B5D68 /
  sub_10C60; fit test sub_11E70 = extent + floor vs ceiling−384);
  movers clamp z into [floor, ceiling−extent] on cave levels. The
  shared height verb needs a floor/ceiling pair (MC1: floor only).
- LOS: NEITHER engine ray-marches terrain for perception — "sight"
  = range + cone + flags in both; proximity = the same
  summed-extents AABB (sub_106C0 = remc1 sub_118C0) + 3D radix
  distance. The novelties are flag/roll gates, not geometry.

---

## Survey 4: MOVEMENT / FLIGHT / TICK ORCHESTRATION

### Verb: creature movement core + state primitives — PARAMETERIZED
(same interpreter, extended table, +1 primitive)

- MC2 tuning table `str_D7BD6[157]` (34 bytes/row; behavior rows
  proper start at index 59, 48 model-keyed rows 0x00-0x2F, then an
  all-zero row + an unexplained duplicate 48-row copy — only 59-106
  referenced). NewEvent defaults every entity to row 59; spawns
  overwrite with 59+model. **The rows are byte-for-byte MC1's
  unk_98F38 schema** (our BehaviorRow, 32 bytes) + a trailing flags
  byte (byte_160_0x20_32) padding to 34. Values match for shared
  models: MC2 model 0 ≡ our BEHAVIOR[0] EXACTLY; model 9 identical;
  model 7 differs only v_12 128→256, v_14 −4→−16. MC1 31 rows, MC2
  48.
- Movement core sub_1B8C0 (:8741) = MC1's creature move verbatim in
  structure: vertical adjust from rows 0xa/0xc/0xe, polar step,
  block test, the same retry ladder (yaw+341, yaw−341, +180°),
  walled → die if row flag 0x20&1 or on water; turn-toward with row
  0x4/0x2 rates.
- State primitives: same mailbox prologue, same base+n encoding.
  IDLE/WANDER/CHASE/PACK-FOLLOW/DEATH all ≡ ours including the
  wander jitter constants. CHASE's attack thunk is passed as a
  FUNCTION-POINTER parameter (vs MC1's inline per-model switch) —
  MC2 itself moved toward our tier-3/4 dispatch design.
- **PACK-FOLLOW retains retail MC1's catch-up line verbatim**
  (:9482: actSpeed = LEADER's maxSpeed + LEADER's actSpeed) — the
  line our 2026-07-05 fix bounded. A shared implementation must
  keep that gate as the faithful-MC1/MC2 behavior column (the
  runaway-pack mis-fix history stands).
- **NEW primitive: FLEE** sub_1C980 (:9572) — bearing-to-aggressor
  +180°, exits once distance ≥ row 0x1c; entered as base+6 where
  MC1 went base+2, gated by row flag bit 8. MC1's 6 primitives + a
  7th, per-kind gated by the widened row.

### Verb: wall gate + movement commit — **REWRITTEN** (player);
PARAMETERIZED (creatures)

The ONE genuinely rewritten verb in the whole survey.
- Player commit moveTest_5D0A0 (:59429): NO type-8 wall gate.
  Blockers: (1) WATER — predicted tile wet → two axis-aligned slide
  attempts, both wet → blocked (water is MC2's absolute barrier);
  (2) blocked-cell flag mapAngle&8; (3) cave clearance — headroom
  test vs ceiling−576 + a 6-step widening side probe that
  AUTO-STEERS yaw ±17i/6 toward the roomier side. On failure:
  position reverts AND target speed zeroes (MC1's gate never killed
  speed). Vertical: floor = ground + row 0xc (wizard +0, vs MC1's
  hard ground+128), gravity −4/tick, cave ceiling clamp −384.
- Creature gate sub_102D0 (:3632) = MC1's design (terrain permission
  mask row v_20 sampled across the radius + slope tests) + added
  cave clauses. PARAMETERIZED.

### Verb: player flight — PARAMETERIZED (same filter/rates/speed
constants, different altitude law + additions)

- IDENTICAL to our sub_455D0 port: input low-pass `s += (2·in−s±3)/4`
  (split across PlayerEvents :38060 + flight :59631); yaw = rate
  from roll `(roll − 7·sign)/8`; pitch = absolute aim; speed impulse
  ±16/tick held, clamp ±80, HOLDS on release, actSpeed slews 16/tick.
  All constants match MC1. The human is still class 3 model 0.
- DIFFERENT climb law: altDiff = clamp(((z − ground − row0xa)·1024)
  /row0xa, ±256) (wizard row0xa = ~~1792~~ CORRECTED 2026-07-12:
  1024 open / 3072 cave — rows 66/104, explicitly overwritten by
  AddPlayer_4A920 EF:33329-32; 1792 is the generic row-59 default
  the player never flies. docs/traces/mc2-flight-model.md §0.1);
  pitch pointing AWAY from
  the band is replaced by pitch·(−altDiff)/256 — zero authority at
  g+1792, inverted above, ramp below; pitching toward the band
  applies raw. Same soft-ceiling concept as MC1's piecewise window,
  as a linear row-driven ramp on the pitch ANGLE, band from the
  behavior row instead of constants.
- Additions: strafe channel (step 16, clamp ±80, decay −4, at
  yaw+90°); moveBoost knockback impulse (cap 128, decay 4); slow-
  effect scaling (4−moveSpeed)/4; external displacement mailbox;
  command 0x27 "full stop" (zeroes roll/pitch/speed, recenters the
  fly assistant) — the "MC2 normalize" feel, together with the
  ever-herding altitude ramp. Camera ROLL is rendered (sub_57B20)
  — our MC1 port leaves roll unrendered.

### Verb: tick orchestration — IDENTICAL skeleton, parameterized
cadence

Frame: palette → sprite anim → input → PlayerEvents_51BB0 (player
commands + filter deltas, ONCE per frame) → UpdateEntities_57730
×{1,4,8 by game speed} → lighting → objectives → sounds → draw.
Inside the tick: one global draw → dead sweep → rebuild per-tick
chains (wizards, 29 creature buckets, projectiles, class-10 subsets,
houses) → pre-passes → single class×action dispatch with per-entity
age++ → post hooks. Same architecture as our World::tick; deltas are
the pre-pass roster and the speed multiplier (confirms the banked
"take MC2 constants" tick-rate design decision).

---

## Consolidated tier-5 verb inventory (the Phase-2 interface input)

| Verb | Verdict | Interface must admit |
|---|---|---|
| Tick orchestration | IDENTICAL skeleton | pre-pass hook list; ticks-per-frame; bucket count/predicates |
| Awake / sight-aggro | PARAMETERIZED | type-flag byte (bits 4/8); retaliation class policy |
| Targeting / autoaim | PARAMETERIZED | extended subtype key; model-78 designated-target pre-acquire; buildings source; homing caps as table data |
| Damage application | PARAMETERIZED | XP decorator hooks; new intake channels (duel grip, steal); per-game sounds |
| Mana economy | PARAM + objective engine | census shared; win check behind an objective module; XP system as decorators |
| Corpse pipeline | PARAMETERIZED | scatter payload (mana jars vs spell tokens); sphere split/merge |
| LOS / height | IDENTICAL | floor/ceiling PAIR (cave second heightmap) |
| Movement core | PARAMETERIZED | widened behavior row (+flags byte); FLEE primitive; attack thunk as parameter |
| Movement commit gate | **REWRITTEN** | the one true per-game verb swap: MC1 wall gate vs MC2 water/cave-steer |
| Player flight | PARAMETERIZED | climb-authority law as the swappable piece; strafe/boost/slow channels; full-stop command |

Re-verify against retail before locking constants: MC2 homing-cap
table values (data extraction); the shield full-absorb flag
transition; whether MC2 cast costs scale with spell level.
