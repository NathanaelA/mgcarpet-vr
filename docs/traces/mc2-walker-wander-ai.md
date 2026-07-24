# MC2 ground-walker wander AI — the goat herd ("sheep") + the townie settlers, retail trace vs port

All cites: `EF:<line>` = `reference/remc2/remc2/engine/EventsFunctions.cpp`, `EV:<line>` = `.../Events.cpp`, `TER:<line>` = `.../Terrain.cpp`, `SND:<line>` = `.../Sound.cpp`, `PLY:<line>` = `.../Player.cpp`. Port cites: `mobs.rs:<line>` = `crates/mgc-sim/src/mc2/mobs.rs`, `mc1/mobs.rs:<line>` = `crates/mgc-sim/src/mc1/mobs.rs`. RNG = the per-entity 16-bit LCG `rand = 9377*rand + 9439`.

Investigates the level-000 playtest deviation: (a) the sheep flock south of the start disperses map-wide in the port but mills in place in retail; (b) the causeway settlers scatter brownianly in the port but file along the causeway toward the spire in retail.

> **READ THE §FOLLOW-UP AT THE BOTTOM FIRST if you're here post-PLAYTEST-3**: it supersedes headline #2's "mills in place" mechanism (no pen exists — the discrepancy is effective sim time, FU.1/FU.3), corrects the §2.6/§7 die-on-water summary (flag-1 = die on ANY box-in, FU.5), and closes Q1-Q5 of the five addenda.

---

## 0. HEADLINE FINDINGS (read first)

1. **The "sheep" are (5,1) — remc2's GOAT** (`AddGoat05_01_1F5B0` EF:11452, `HitGoat_1F530` EF:11441, `PreKillGoat_1F4F0`/`KillGoat_1F510` EF:11429/11435; ctor `AddCreature_4B490` EF:33720, sprite 238, behavior row 98). Our port calls it "Vulture" — an early-survey misnomer worth renaming. Level-000 authors 33 of them in two herds at (40-77, 20-39) and (98-110, 31-39) — on the torus map that IS "just south of" the start (74,212): 212→20 wraps in ~64 tiles. The settlers are the traced **(5,13) townie/Villager** (`AddVilliger_4BF40` EF:34037, row 100), 29 authored on the causeway (118-145, 212).
2. **Retail walkers have NO home anchor, tether radius, or downhill bias.** The flock stays put through THREE mechanisms: (i) a **fast yaw slew** — yaw chases the wander heading at up to **row `v_2` per tick** (goat 45 ≈ 7.9°/tick, EF:8868-8875 + `sub_58350` EF:40391-40405 — the 3rd arg is DEAD, the clamp is `subtype_160_0x2_2` = **v_2**), so the ±(85..340) heading nudge every `v_26` ticks (EF:9136-9141) produces a tight, kinked random walk, not long arcs; (ii) **leader-follow chains** (state +3, `sub_1C560` EF:9345): an awake goat latches the nearest same-model leaderless goat and steers at it with catch-up speed `leader.max + leader.act` (EF:9482), with a fixed **256-unit (1-tile) separation** shove (EF:9470-9480); (iii) **slope refusal walls** — a move into a tile whose corner-height cross metric ≥ `v_16` (goat 20, villager 15) is BLOCKED outright (EF:8809-8810, metric TER:1578-1600), which fences herds into their basin. There is **no uphill speed penalty** — slopes either pass at full speed or refuse.
3. **Retail settlers are never in free wander: they are permanently marching at the nearest ENTERABLE building.** The townie brain (`sub_23340` EF:14506) scans the global (10,45) list every `v_26`=40 ticks for buildings whose **`bldgprm[template].byte_2 & 1`** is set (EF:14619) — no range limit, nearest wins — then walks at it at maxSpeed+12 (EF:14637), steering every 40 ticks (EF:14589) and entering it within 0x800 if it has capacity (EF:14592-14598). Level-000 data (probe, this session): **every at-load building on the level is enterable (flags 0x03)** — the western village (x46-97) and eastern ring (x172-233) — while the route-gated spire/obelisk buildings (dis 13..32, par1 37/17) are NOT (flags 0x08/0x18). Settlers on the water causeway therefore beeline east/west along it toward real village houses; the water on both flanks and the `v_16`=15 slope wall at the shore hills produce the observed "files along the causeway, stalls on hills" — **an emergent bias, exactly as the playtest guessed**.
4. **THE PORT'S TOP BUG: the move-commit turn is clamped with row `v_4` (always 5) instead of `v_2`** (`mobs.rs:208/212` vs retail EF:8868-8875) — goats turn 45→5 (9× too slow), villagers 22→5 (4.4×). Yaw can no longer catch the wander heading inside one decision period (goat max slew 5×32=160 < mean nudge 212), so the walk turns ballistic — long straight runs = fast dispersal — and every steering correction (follow the leader, flee, walk at the building) is equally crippled. MC1's own `creature_move` does it right (`mc1/mobs.rs:674`, `row.v_2`).
5. **Port bug #2: the passability probe checks one extra 256-step point.** Retail `sub_102D0` a3&1 probes `while (v8 <= max(pitch,roll))` — for walker extents (goat 0, villager 128) that is exactly ONE check at the predicted position (EF:3659-3686). The port's loop (`mobs.rs:165-181`) tests the predicted point AND predicted+256 — walkers bounce off water/village paint a full tile early. On the 1-tile causeway this converts retail's clean along-path march into constant false blocks → ±60°/180° retry spins → the "brownian" scatter.
6. **Port bug #3: the villager building scan is missing the `bldgprm byte_2 & 1` enterable gate** (`mobs.rs:1407-1424`, the stale "no (10,45) spawns yet" comment predates the building creator). Benign at level-000 load (all at-load buildings happen to be enterable) but wrong the moment the route fires: the dis-13 obelisks at (157-161, 209) — NON-enterable par1 37 — sit right next to the causeway row and would magnetize the port's settlers away from the real villages.

---

## 1. Identification + level-000 authored data

- **(5,1) GOAT** — ctor `AddCreature_4B490` EF:33720-33748: initState 9 (= base 8 **+1**, the wander state), min/max speed 54/18, `actSpeed = maxSpeed = 18` (:33731; 54 is the RUN speed used by flee), maxLife 600, `yaw = roll = pitch = 0` (:33733-33735 — the whole herd spawns facing the same way; NO ctor RNG), `dword_0x10_16 = slot % 100` (:33736), behavior row **`str_D7BD6[98]`** (:33739), `byte_0x3E_62 = per-model spawn ordinal` (:33740 — phase-staggers the herd's decision ticks), `byte_0x39_57 = v_26+1 = 33` (:33741, the awake countdown), xtype 3, sprite 238 (:33745). State handlers EF:11386-11462 — all eight are thin wrappers over the shared primitives plus a sound-46 roll (`% 0x4D`, `% 0x2B` in flee) and the "flee runs at minSpeed(54)" speed fixup (:11392-11393). State +2 (chase) REDIRECTS to flee (+6) (:11412-11414) — goats never fight.
- **(5,13) TOWNIE** — ctor EF:34037-34091 (already ported): initState 0x69 (+1), 54/18 with `actSpeed=18`, ONE RNG facing draw (:34048-34051), `subSpellIndex=500`, `byte_0x39_57 = 64` (:34059), `dword_0x10_16 = 2` (:34061 — the death-mode latch, see §6), spawn-ordinal `byte_0x3E_62` (:34062), row **100** (:34058), `%9` sprite pick (:34066-34087), ShiftRot(128,128) (:34088).
- Level-000 probe (this session, baked things): 33× (5,1) dis 0 in the two southern herds; 29× (5,13) dis 0 at y=212, x=118..145 (the causeway); buildings as in §6.

## 2. The movement core — `sub_1B8C0` (EF:8741-8939)

Runs once per tick from every walking state (+1/+3/+6 and chase; the +0 patrol does NOT move, §3). Verbatim sequence:

1. **Stop-flag**: `byte[1] & 8` → consume, return 4 (EF:8786-8791).
2. **Predict**: copy position; altitude core `sub_580E0(pred, getTerrainAlt, v_12, v_10-unused, v_14)` (EF:8798-8804; body EF:40372-40388: `if z > alt → z += v_14`, then `if z <= alt+v_12 → z = alt+v_12`; goat/villager v_12=0, v_14=−256 — ground snap); polar step `MoveEntity_57FA0(pred, yaw, 0, actSpeed)` (EF:8805; body PLY:6-20 — `x += speed·sin(yaw)`, `y −= speed·cos(yaw)`, 16.16 tables).
3. **Same-tile shortcut**: if `pos>>8 == pred>>8` on both axes → commit unconditionally, result 1 (EF:8806, 8923-8936).
4. **Block test** on tile crossing (EF:8809-8810): blocked iff `sub_102D0(self, pred, 1)` **or** `sub_1B7A0_tile_compare(pred) >= v_16`.
   - `sub_102D0` a3&1 (EF:3659-3686): steps from the PREDICTED point along yaw in 256 increments `while (v8 <= max(array.pitch, array.roll))`; each point is blocked when `~v_20 & sub_104D0_terrain_tile_is_water(pt)` ≠ 0 (EF:3669-3673) — `sub_104D0` returns `1 << f(tileType)` per the TER:2067-2100 bit table (type 0 water → bit 0, types 8/9 village bands → 0x100/0x200, roads 10-12 → 0x100000..0x400000, rough 15-20/28-34 → 0x400 …). Cave-only extras: `mapAngle & 8` walls + `sub_11E70` (EF:3674-3682). **For extents ≤ 255 this is exactly ONE probe** (v8=0 passes, v8=256 exits the loop).
   - `sub_1B7A0_tile_compare` (TER:1578-1600): the destination tile's 4 heightmap corners; returns `max(|p1+p4−p2−p3|, |p1+p2−p4−p3|)` — the cross-corner gradient. **≥ v_16 refuses the move entirely** (goat 20, villager 15, raw height bytes). This is the ONLY slope treatment — no speed scaling, no explicit downhill preference; the bias is emergent (steep = wall).
5. **Retries** (each repeats steps 2-4 at the mutated yaw, which STAYS mutated): +341 (≈+60°, EF:8813-8817); then the byte-split `LOBYTE = yaw0−85, HIBYTE = ((yaw0−341)>>8)&7` (EF:8829-8831); then the precedence-quirk reverse `(yaw0+0x400) & (0x700 + (uint8)yaw0)` (EF:8843).
6. **All four blocked** (EF:8855-8862): if row flags bit 1 (die-on-water — set for goat AND villager) or the CURRENT tile is open water → `life = −1`. Result 4.
7. **Commit** (EF:8867-8875 and twins): write position, then `yaw = (yaw + sub_58350(yaw, roll, v_4, v_2)) & 0x7FF`. `sub_58350` (EF:40391-40405): **a3 (v_4) is commented out/dead; the turn magnitude is `min(angdist(yaw,roll), a4 = subtype_160_0x2_2 = v_2)`** signed toward roll (helpers `sub_582B0`/`sub_582F0` SND:6569-6600). Goat v_2 = **45**/tick, villager **22**/tick.

So the walker architecture is: `roll` = target heading (written by brains/steering), `yaw` = actual heading chasing roll at ≤ v_2/tick, blocks rotate `yaw` directly.

## 3. Wander (+1, `sub_1BF90` EF:9064-9234) and patrol (+0, `sub_1BD90` EF:8945-9058)

Both open with the shared damage head (mailbox + weakest-subentity life + death latch, EF:9094-9128): hit → target=attacker, state +6 (flags bit 8 — both rows have it) else +2 (EF:9226-9231); dead → +4 (EF:9221).

**+1 WANDER, the quiet path** (EF:9131-9213):
- `sub_1B8C0` move every tick (EF:9133).
- On `byte_0x3E_62 % v_26 == 0` (goat 32, villager-class rows 40; the counter is the u8 per-tick age, incremented in the dispatch loop EV:461-549, seeded with the spawn ordinal):
  - **The wander-turn law** (EF:9136-9141): two LCG draws;
    `roll += ((rand2 & 0xFF) + 85) * (2*((rand1 % 0x9D)/79) − 1); roll &= 0x7FF`
    — magnitude 85..340 (15°..60°, mean ≈ 37°), sign −1 with p=79/157. Yaw then catches up at v_2/tick (§2.7) — for the goat within ≤ 8 ticks of the 32-tick period, i.e. **the retail track is a kinked, quasi-Brownian walk at speed 18 (0.07 tile/tick)**.
  - **Awake gate** `byte_0x39_57 != 0` (EF:9140-9142): maintained by the per-tick awake pass `sub_68BF0`/`sub_68C70` (EF:55469-55543) — countdown, refreshed to 16 while the player is within `dist² < 0x2400000` (≈ 24 tiles), dead entities parked at 0xFA. While awake:
    - **Wizard scan** (EF:9144-9174): nearest live class-3 (list `dword_38519`, rebuilt per tick in `UpdateEntities_57730` EF:39927/39975-39985) within `v_28` (goat 1536 = 6 tiles) AND inside the facing cone `angdist(yaw, bearing) < v_30` (goat 853 ≈ ±150°), skipping invisibles (`byte[0]&0x20`). Found → target it, state +6 (flee; both rows flag-8). **This is the only human-avoidance walkers have.**
    - **Pack fallback** (EF:9176-9209, gated on row flags bit 4 CLEAR — true for goat and villager rows): nearest same-model creature with **no leader of its own** (`!word_0x32_50`) within v_28/cone (per-model list `bytearray_38403x[model]`, EF:39987-40009) → `word_0x32_50 = leader`, state **+3 follow**.

**+0 PATROL** (EF:9019-9057): NO movement call at all — a stationary graze that only runs the damage head and (on cadence, flags bit 4 clear) the same pack scan → +3. Quirk: its cone test uses the REVERSED bearing `tan2(candidate → self)` (EF:9038), unlike +1's `tan2(self → candidate)` (EF:9194). Goats/townies start in +1 and nothing routes them to +0, so this state is practically vestigial for them.

## 4. Flock cohesion — the follow state (+3, `sub_1C560` EF:9345-9528)

- Leader validity: alive, not despawned, same class AND model (EF:9382-9386); invalid → drop, back to +1 (EF:9430-9434).
- Moves via `sub_1B8C0` every tick (EF:9429). On the row cadence (EF:9437), switch on the LEADER's state − base (EF:9439-9469):
  - leader +0/+1 (grazing/wandering) → **steer**: `roll = tan2(self → leader)` (EF:9450-9455);
  - leader +3 → **adopt the leader's leader** (`word_0x32_50 = leader.word_0x32_50`, EF:9450-9451) — chains collapse toward a single head;
  - leader +2/+6 → copy its target, join that state (EF:9444-9449, 9458-9463);
  - anything else → drop leader, +1 (EF:9464-9468).
- **Separation** (EF:9470-9480): first same-model packmate within a FIXED 256-unit (1-tile) box → `roll = tan2(packmate → self)` (steer directly away) — overrides the leader steer.
- **Catch-up speed** (EF:9482): `actSpeed = leader.maxSpeed + leader.actSpeed` (goat: 18+18 = 36 while the leader ambles) — followers close distance between decisions.
- Damage while following also re-targets the LEADER onto the attacker before transitioning (EF:9487-9525).

Net retail flock dynamics: awake goats condense into follow-chains around grazers/wanderers, hold ~1-tile spacing, and the head does a tight random walk — **the herd mills in place**. TRACED: there is no spawn-point memory or radius tether anywhere in these handlers.

## 5. Flee (+6, `sub_1C980` EF:9572-9675)

- Move every tick; validate threat via `sub_1ED30`.
- Every 4th tick (`byte_0x3E_62 & 3`): `roll = tan2(self → threat) + 0x400` (the `HIBYTE += 4` at EF:9636 = +1024 = dead away), then the same same-model separation but with the box = `array_0x52_82.pitch` (sprite extent; goat 0 — no-op, villager 128) (EF:9641-9650).
- On the row cadence: threat farther than `v_28` (goat 1536, villager 3584) → back to +1 (EF:9653-9657). Goat flee speed = minSpeed 54 (state wrappers §1); recovery resets to maxSpeed 18 (EF:11444-11445).

## 6. The townie brain — `sub_23340` (EF:14506-14648) + friends

States: 104 +0/+2/+3 all re-enter the brain at 105 (EF:14499-14503, 14654-14665); +4 `KillTownie_23680` (EF:14668-14681 — `dword_0x10_16 != 0` → just DisableEntityDrawing; the ctor sets it to 2, so townie deaths NEVER credit/PreKill via this path — entering a house sets it to 1 first); +6 `HitTownie_23710` = shared flee + speed exit (EF:14691-14704).

Brain, quiet path (per tick): move `sub_1B8C0` (EF:14577); everything else on `byte_0x3E_62 % 40` (EF:14579).

- **Damage** → stamp the wizard's wanted timer `word_0x248_584 = 200` (EF:14561-14563), target attacker, state 110 (+6 flee at minSpeed 54, EF:14641-14646).
- **Have a target `word_0x96_150`** (EF:14581-14605): if it is a live (10,45): `dist3d > 0x800` → `roll = tan2(self → building)` (EF:14589) and nothing else; within 0x800 and `building.minSpeed > building.dword_0x10_16` → ENTER: own `dword_0x10_16 = 1`, state 108 (vanish via KillTownie), building occupant count ++ (EF:14592-14598). Capacity = `minSpeed = footprint w·h >> 4` (`sub_49A30` EF:32770), initial occupants = 2 (EF:32768). Otherwise drop the target, `actSpeed = maxSpeed` (EF:14601-14604).
- **No target** → the standard two-draw wander turn (EF:14607-14611), then the **building scan** (EF:14613-14640): walk `dword_38527` — the global list of ALL (10,45)s (list build EF:40043-40052, no life gate) — keep the nearest whose **`str_D93C0_bldgprmbuffer[building.byte_0x46_70].byte_2 & 1`** (ENTERABLE) is set (EF:14619); no range limit, no cone. Found → target it, `actSpeed = maxSpeed + 12 = 30` (EF:14637).
- NO wizard-proximity flee for townies — they only flee when actually hit.

**Level-000 bldgprm ground truth** (probe, this session; template = par1 raw, flags = byte_2):

| group | where | templates | flags | enterable |
|---|---|---|---|---|
| western village (at load, dis −1) | x46-97, y180-250 — around the start | 26/29/30/31/34/42/47/50 | 0x03 | **YES** |
| eastern ring (at load, dis −1) | x172-233, y183-234 | 26/29/30 | 0x03 | **YES** |
| route obelisk/spire chain (dis 13..32) | x156-237 incl. (157,209),(161,209) beside the causeway | 37 | 0x08 | no |
| spire center (slot 3) | (198,213) | 17 | 0x18 | no |

So retail causeway settlers (x118-145, y212) ALWAYS have an enterable target — west (96,215)/(96,205) for the row's west end, the eastern ring for its east end — and the march is channeled by water (mask bit 0) on both flanks and the `v_16`=15 slope wall at the shores. **The playtest's "settlers travel the causeway toward the spire" is this march; the "went backwards up hills but got nowhere" is the slope refusal.** (INFERENCE from traced code + data; the exact retail split east-vs-west depends on live distances.)

## 7. Behavior rows (absolute indices; port `mc2/behavior.rs` extraction verified)

| field (offset) | GOAT row 98 (v_0=39) | TOWNIE row 100 (v_0=41) | consumed at |
|---|---|---|---|
| v_2 turn clamp/tick | **45** | **22** | EF:8868-8875 (§2.7) |
| v_4 (dead 58350 arg) | 5 | 5 | — |
| v_10 / v_12 / v_14 alt | 256 / 0 / −256 | 256 / 0 / −256 | EF:8798-8804 |
| v_16 slope refusal | **20** | **15** | EF:8809-8810 |
| v_20 terrain mask | 0xfff080fe → blocks types 0, 8-9+rough band, 21-27 | 0xfffffefe → blocks type 0 (water) + type 8 only | EF:3669-3673 |
| v_26 decision period | 32 | 40 | EF:9134/14579 |
| v_28 scan/flee radius | 1536 (6 tiles) | 3584 (14 tiles) | EF:9144/9653 |
| v_30 scan cone | 853 (±150°) | 512 (±90°) | EF:9159 |
| flags | 0x9 = die-on-water + flee-on-hit | 0x9 | EF:8855/9003 |

Note the mask asymmetry: **goats refuse village texture bands and rough types** (they cannot enter painted towns at all); villagers walk everything but open water and the type-8 band.

## 8. What retail does NOT have (question checklist)

- **No uphill speed penalty / no downhill steering** — slope handling is the binary v_16 refusal only (§2.4). TRACED.
- **No deep-water special case for walkers** beyond the v_20 bit-0 block; night maps have no second water heightmap (Terrain.cpp:52 cave-only). A walker boxed in on all four retry headings dies if flag-1 or standing on water (EF:8855-8862) — that is the entire drowning law. TRACED.
- **No home/spawn anchor, no tether radius** — cohesion is purely leader-follow + separation + terrain walls (§4). TRACED (absence, all five shared primitives read).
- **No building avoidance field** — buildings repel only via the painted terrain types in v_20; they ATTRACT townies via the bldgprm scan (§6). No other magnet/effector reads these walkers.
- **Human avoidance** = the awake-gated wizard flee scan (§3), goats only; townies flee only on damage.

## 9. PORT DEVIATION LEDGER (crates/mgc-sim/src/mc2/mobs.rs)

The port's state machines (`mc2_idle`/`mc2_patrol`/`mc2_pack`/`mc2_flee`/`villager_brain`), wander-turn law, cadences, awake pass, ctors, rows, masks, slope metric (`roughness`, mc1/mobs.rs:538-548 ≡ TER:1578-1600) and boxed-in death were all verified faithful. Three concrete deviations found:

| # | deviation | port site | retail law | playtest symptom |
|---|---|---|---|---|
| **D1** | move-commit turn clamped with `row.v_4` (=5) instead of **`row.v_2`** | mobs.rs:208 (`row_v4`), used at :212/:219 | EF:8868-8875 + EF:40391-40405 (clamp = `subtype_160_0x2_2`); MC1 twin correct at mc1/mobs.rs:674 | goats turn 9× too slowly (45→5), villagers 4.4× (22→5): wander becomes ballistic → **herd disperses map-wide**; follow/flee/building steering can't bite → settlers orbit instead of marching |
| **D2** | passability probe tests ONE extra 256-step point (`walked > reach` checked AFTER the probe) | mobs.rs:165-181 loop order | EF:3659-3686 `while (v8 <= v7)` — extent 0/128 ⇒ exactly one probe at the predicted point | walkers false-block a tile early; on the 1-tile causeway ⇒ constant ±60°/180° retry spins = **brownian settlers**; goats bounce around their basin harder |
| **D3** | building scan missing the **`bldgprm.byte_2 & 1`** enterable gate (stale pre-building-creator comment) | mobs.rs:1407-1424 | EF:14619 | benign at level-000 LOAD (all at-load buildings enterable — §6 table) but wrong mid-route: the dis-13 obelisks (157/161, 209) beside the causeway would capture the port's settlers |
| D4 (minor) | `mc2_patrol` pack-scan cone uses self→candidate bearing; retail +0 uses the REVERSED `tan2(candidate→self)` | mobs.rs:372-402 via :477 | EF:9038 (vs EF:9194 for +1) | none for goat/townie (they never occupy +0); fidelity nit for models that do |
| D5 (cosmetic) | (5,1) named "Vulture" throughout port + survey | mobs.rs:86,807-840; SURVEY-MC2-ROSTER | remc2 authors: GOAT | reader confusion (it is the sheep/herd animal) |

**Fixes** (in impact order):
1. `mc2_move_core`: `let cap = BEHAVIOR[row].v_2;` — one line (mobs.rs:208). Expect the mc2 goldens to move (deliberate re-pin); MC1 goldens untouched (separate verb column).
2. `mc2_path_blocked`: restructure to retail shape — `while walked <= reach { if mask-blocked(pos) { return true; } walked += 256; polar_step(...); } return false;` (probe count = ⌊reach/256⌋+1, matching EF:3667).
3. `villager_brain` scan: gate on `self.assets.bldgprm.get(c.f71 as usize).is_some_and(|p| p.flags & 1 != 0)` (the table is live since the building creator landed).
4. Optional D4 arg swap in `mc2_patrol`'s scan; D5 rename `VULTURE_BASE`→`GOAT_BASE` etc. when convenient.

**Suggested verification probe** (fits the state-hash golden method): fixed-seed headless level-000, 5000 ticks, player parked at the start; assert (a) goat-herd RMS displacement from the authored centroid stays under ~8 tiles (currently blows up), (b) causeway villagers' mean |Δx| progress along y=212 is monotone toward a village (currently ~0), (c) no goat enters a tile whose type ∈ {8,9} band.

## 10. Retail checks banked / OPEN

1. **BANKED**: retail level-000 observation — do the causeway settlers split east/west by row position (nearest-enterable prediction, §6) or all head one way? Nearest-building math says split near x≈134; a from-memory or dosbox check would confirm the scan is truly unlimited-range/nearest.
2. **BANKED**: retail herd radius over ~2 minutes with the player hovering 10+ tiles off (awake but not fleeing) — expected: milling within a handful of tiles (v_2=45 + follow chains). Calibrates the D1 fix.
3. OPEN: `sub_1ED30` target-validation corner cases (StageVar2 ≠ 0 spawn variants) — not consumed by goat/townie paths traced here.
4. OPEN: whether any level authors goats/townies in state +0 (patrol) via the conditional-spawn machinery (`sub_1D5D0` EF:9977 dispatches on StageVar2; 0 = no-op for these ctors) — would make D4 observable.

---

# §FOLLOW-UP (PLAYTEST-3 addenda 1-5) — the dispatch loop, the "pen" mystery, and the march that isn't a march

Written after D1-D4 landed and five rounds of retail re-observation (goats still spread vs retail's tight peak blob; settlers = "pure random walk", never converging on dwellings; claiming every building changes nothing). Verdicts below marked **TRACED** (file:line), **MEASURED** (probe on our baked/oracle data), or **INFERENCE**.

## FU.0 Headline answers

1. **Q1 — there is NO time-slicing, distance culling, or brain-freeze in the retail dispatch.** The per-tick loop runs EVERY live entity's handler EVERY sim tick (full chain of citations in FU.1). The only distance-sensitive machinery remains the awake byte, and it gates SCANS only. **BUT: one retail sim tick = one rendered frame** (`UpdateEntities_57730` is called once per frame at game-speed 0, ×4/×8 at speeds 1/2 — EF:31800-31815), and **the whole world pass is inside `if (!(OptionsSettingFlag & GAME_PAUSED))`** (EF:40093) — pause/menus (and possibly the fullscreen map — banked) freeze all walkers. Retail wall-clock motion rate = DOS frame rate (~12-20 fps) × unpaused fraction, which for a screenshot-and-map-heavy observation session is a small fraction of our port's fixed tick rate.
2. **Q2/Q3 — the traced wander law is confirmed law-exact, and it CANNOT produce the retail blob at face-value tick counts.** Raw `str_D7BD6` rows re-verified against the initializer (Level.cpp:110/112 — goat v_2=45, v_16=20, v_26=32, v_28=1536, v_30=853, flags 9; villager 22/15/40/3584/512/9). No home anchor, no territory radius, no mean reversion exists anywhere in the walker paths (FU.2). Oracle-exact terrain flood-fill: **no closed pen** — 15,920 tiles reachable from the herd center under goat rules (FU.3). A faithful simulation of the complete traced law (exact 16-bit LCG, exact draw pattern, cadence 32, clamp 45, speed 18) yields **median displacement ≈19 tiles @500 ticks, ≈70 @4000** — statistically identical to OUR port's measured behavior and irreconcilable with a multi-minute few-tile blob. ⇒ **Our port is running the traced law correctly; the retail observations are of far fewer effective sim ticks** (pause + frame rate), and/or the shipped binary differs from remc2's reconstruction (version-skew note in FU.3). INFERENCE, with the decisive experiments banked.
3. **Q4 (superseded by the pure-random-walk verdict) — the townie building-march branch is REAL and UNGATED for authored settlers; no authored-vs-homeless gate exists** (every gate enumerated in FU.4). The player's claim-every-building test changing nothing is CONSISTENT: the scan reads the STATIC `bldgprm byte_2 & 1` bit, never ownership. The retail "pure random walk" look is explained by (i) march steering being a once-per-40-ticks roll write that terrain blocks dominate on the causeway, (ii) the within-0x800 regime degenerating to a steer-less drop/re-acquire cycle, (iii) settlers who DO complete a march vanishing silently into the house (survivor bias — you only ever watch the ones still wandering), and (iv) the small effective sim time. Our port's visible beeline = same law at many× the effective tick rate.
4. **Q5 — the boxed-in death law was mis-summarized in the main doc: row flag bit 1 means "die on ANY all-four-blocked tick, even on land"** (`flags&1 || standing-on-water` — EF:8855-8862). Goat AND villager have it. A blocked walker commits NO movement on all-fail (verified; port matches). Retail causeway settlers WILL also drown/vanish given enough unpaused sim time — the watch windows were too short. Banked retail check.

## FU.1 Q1 — the real per-tick creature dispatch, fully traced

- **Main frame function** (EF:31796-31815): `PlayerEvents_51BB0(); sub_848A0();` then by `speedIndex`: 0 → ONE `UpdateEntities_57730()`, 1 → FOUR, 2 → EIGHT. So the sim tick is frame-locked, with a game-speed multiplier.
- **`UpdateEntities_57730`** (EF:39927-40185), per call:
  1. Global rand step + reap pass (`byte[1] & 4` → `sub_57F20`) (EF:39947-39956).
  2. Per-model/class list rebuild (EF:39958-40085): class-3 → `dword_38519` (:39975-39985, life ≥ 0 only), class-5 per-model → `bytearray_38403x[model]` (:39986-40009, life ≥ 0, excluding multipart states 0xB4/0xE8/0xEA), class-9 → `dword_38531`, class-10 m0x27/28/0x39 → `dword_38523`, **m45 → `dword_38527` with NO life/state gate** (:40018-40052), class-11 m12/31 → `dword_38535`.
  3. Inside `if (!(OptionsSettingFlag_24 & GAME_PAUSED))` (EF:40093):
     - `sub_12780()` stage-var pass + `sub_12500` for entities with StageVars (EF:40095-40103 — authored level-000 walkers have none, verified pars all zero in the baked things);
     - awake pass `sub_68BF0` (EF:40107-40108, skipped in multiplayer!);
     - mana census `sub_60F00` (EF:40115);
     - **the dispatch loop (EF:40116-40180): `for (mx = Entities[1]; mx < Entities[1000]; mx++)`** — every live entity, no cursor, no stride, no budget, no distance test. Gate = the STATIC action-table row: `actionIndex == row.word_4` (self-consistency) and `row.dword_10` (compile-time enable) (EF:40130-40132). Handler via `pre_sub_4A190_0x6E8E(row.address_6, mx)` (EF:40171 — a direct address→function switch, e.g. `case 0x2003c0: sub_1F3C0(...)` EV:1236-1239). **`byte_0x3E_62++` at EF:40172 — once per dispatch call** — so the v_26 cadence is measured in handler calls ≡ sim ticks ≡ frames.
- **Awake-byte interaction** (`sub_68BF0`/`sub_68C70` EF:55469-55543): pure countdown refreshed to 16 when the player is within `dist² < 0x2400000` (24 tiles), propagated to subentities (+2 for chains), dead entities parked at 0xFA; gates only the wizard/pack scans (EF:9140-9142). Goat ctor seeds it 33, villager 64 — an initial one-shot awake window.
- **GAME_PAUSED** = bit 1 of OptionsSettingFlag, toggled by `PauseUnpauseGame_18BB0` (PlayerInput.cpp:406-417) and managed around menu-state transitions (GameUI.cpp:662-721). **CONFLICTED 2026-07-24**: a decompile reading (map-mana-roster.md trace) had `SetPausedMenuOpen_41AF0` (GameUI.cpp:651) fire for any nonzero map menu state — but the PLAYER's retail recollection is that the MC2 fullscreen map keeps NORMAL CONTROLS live (you can fly and even cast, HUD-less) — gameplay outranks the decompile, so either the plain map state (6) skips the pause call or SetPausedMenuOpen doesn't set GAME_PAUSED for it. Re-read the GU:641-721 dispatch before relying on either version; retail map-dot observations may or may not sample a frozen world.

**Port consequence:** our sim ticking at a fixed high rate makes every walker cover proportionally more ground per wall-clock minute than dosbox retail. This is a CALIBRATION issue, not a law issue — see FU.6.

## FU.2 Q2 — wander-turn law re-verification + the unexplained row fields

- Raw initializer rows (Level.cpp:11+, row N at line 12+N): row 98 = `{0x27, 0x2D, 5, 0x16, 5, 0x100, 0, 0xFF00, 0x14, 0x200, 0xFFF080FE, {00,09}, 0x20, 0x600, 0x355, 0x09}` and row 100 = `{0x29, 0x16, 5, 0x16, 5, 0x100, 0, 0xFF00, 0x0F, 0x200, 0xFFFFFEFE, {00,09}, 0x28, 0xE00, 0x200, 0x09}` — every port value confirmed (v_2 45/22, v_16 20/15, v_26 32/40, v_28 1536/3584, v_30 853/512, flags 9).
- The nudge is ADDITIVE on `roll` from its current value (EF:9139, villager twin :14610), sign −1 with p=79/157; NOTHING mean-reverts roll: its only other writers in these states are the follow steer (EF:9453), the flee flip (EF:9635-9640), the separation shove (EF:9476, 9647) and the townie building steer (EF:14589). The per-entity RNG state is **uint16_t** (global_types.h:331) — our RandWidth::U16 is exact.
- Decompile ambiguity note: the raw IDA comments at :14578/:14612 show a DIVISION (`byte_62 / v_26`) where the author wrote `%`; the `%` reading matches the :9134 idiom and all other creature cadences. Low confidence risk only; flagged.
- **The row struct has NO territory field**: offset 0x18 (our v_22 = 2304) is literally declared dead pad `stuba[2]` (global_types.h:87) — ZERO readers in the tree. v_18/offset 0x12 is read only in `sub_102D0`'s a3&4 pitch-clamp branch (EF:3703, flyer moves). v_6/v_8 are pitch-turn clamps for flying paths (EF:58608, 62802, 62829). All 17 row fields now accounted for in the walker paths.
- The goat's `dword_0x10_16 = slot % 100` (EF:33736) is WRITE-ONLY boilerplate — no reader in any goat state (the models that use the field overwrite it in their ctors). No spawn-anchor storage exists in the entity for these walkers.
- Goat state machine re-verified: ctor initState 9 = +1 (EF:33725); NO transition into +0 exists from +1/+3/+6 (sub_1BF90 exits only to +2/+3/+4/+6; sub_1C560 drops to +1; flee releases to +1) — retail goats are never in the stationary patrol; our always-+1/+3 port matches. (Addendum-5 hypothesis (d) CLOSED.)

## FU.3 Q3 — the pen does not exist in code or data; the law's dispersal quantified

- **MEASURED (oracle-exact terrain, level-000)**: flood fill under goat rules (mask 0xFFF080FE + tile-metric < 20, 8-connected) from the herd center (48,29): **15,920 reachable tiles** spanning the entire landmass; same from the second herd. The peak/valley feature is a SOFT pocket (partial high-metric ring with many sub-threshold corridors), not a corral. Under villager rules from the west-village edge: 13,089 tiles. The "eastern hill" band the port settlers climbed (155-180 × 210-230): **every tile's corner metric < 10** — the climb is legal under the retail law too.
- **MEASURED (law simulation)**: the complete traced wander (16-bit LCG `x*9377+9439`, sign `(v%157)/79`, magnitude `(r&255)+85`, cadence 32 with per-model phase stagger, one per-tick sound draw interleaved, yaw→roll clamp 45/tick, speed 18/tick) gives median displacement **18.7 tiles @500 / 46 @2000 / 70 @4000 ticks** (33 walkers; true-random RNG gives the same — the LCG adds no anti-correlation). Our port's in-sim measurements (17.8 @500 / 33 @2000, terrain reflections included) sit exactly on this law. **A handful-of-tiles blob over minutes of UNPAUSED play is impossible under this law** — the reconciliation must be effective sim time (FU.1) and observation pattern, or a binary/version difference:
- **Version-skew caveat (banked)**: the GOG `NETHERW.EXE` (extracted from game.gog; LE, code object base 0x10000) does NOT match remc2's IDA offsets (function layout differs — remc2 was made from a different build). Direct binary verification of the shipped wander brain was not completed in this session (the constant-signature scan surfaced the terrain shader and the class-9 zigzag-path builder `sub_5xxxx` EF:58270-58399 — which, notably, is where the ±8-bounded random-walk idiom lives, NOT in walker AI). remc2's creature brains carry regression instrumentation (`add_compare`/DoDebugSequences), so the reconstruction is probably faithful — but "probably" is doing work here; the banked timelapse check (FU.6) falsifies or confirms cheaply.

## FU.4 Q4 — the march branch: really ungated, really invisible

Every gate on the `sub_23340` path, checked for an authored-spawn bypass:
- `word_0x96_150` starts 0 (NewEvent memset; ctor doesn't set it) — the march arm (:14581) is skipped until the SCAN sets it.
- The scan (:14613-14640) has exactly ONE filter: `str_D93C0_bldgprmbuffer[bldg.byte_0x46_70].byte_2 & 1` (:14619) — a static per-template bit (loader `sub_539A0` reads 76×4-byte records, EF:38328). **No ownership/claim/disposition/awake/StageVar/`dword_0x10_16` gate exists** — the claim-everything experiment changing nothing is exactly what the code predicts.
- The authored spawn path adds nothing: `sub_4A310` (EF:32999) → ctor → `sub_12100` (EF:4684-4750, stage-var attach — level-000 walker records carry no stage vars; pars verified all-zero) → `sub_58DA0` (EF:40650, objective-pointer wiring only). actionIndex stays the ctor's 105. There is NO "homeless vs authored" distinction anywhere in the townie brain.
- `byte_2 & 1` is the POPULATED-HOUSE bit: the same bit gates the house's militia pop (EF:27998-28010), its townie emission (EF:28043-28061, requires capacity `minSpeed > 5` AND occupants == capacity), and its collapse evacuation (EF:28112-28127). Level-000 capacities (`w·h >> 4`, BUILD00 footprints — MEASURED): templates 26→6, 29→8, 30→12, 31→15, 34→30, 42→7, 47→7, 50→3, spire 17→144; route obelisk 37→1 (< occupants-init 2 — NEVER enterable in practice).
- Why retail READS as a pure random walk (INFERENCE from traced pieces): (i) the march writes `roll` once per 40 ticks; on the causeway the bearing points into water, so per-tick motion is dominated by the block-retry churn; (ii) within 0x800 of the target the branch does NO steering at all — it alternates {capacity-fail → drop target + speed 18} / {wander-nudge + re-acquire + speed 30} every 40 ticks (:14592-14604 vs :14607-14637) — a leash-free random walk near the village; (iii) every settler that DOES enter disappears (state 108 → `KillTownie` silent despawn via `dword_0x10_16` != 0, EF:14668-14676) — the visible population is always the not-yet-arrived; (iv) effective sim time (FU.1).

## FU.5 Q5 — the drowning law, corrected

- **Correction to §2.6/§7 of the main doc**: `if (row.byte_0x20 & 1 || tile_is_water(current) == 1) life = -1` (EF:8855-8856) — flag bit 1 (set on BOTH goat and villager rows) means **die whenever all four move candidates are blocked, water or not**; bare water-death applies to everyone else. The port implements the corrected reading (mobs.rs mc2_move_core all-blocked arm).
- All-four-blocked commits NO position change (EF:8863 returns 4 with no CopyEntityPosition on that path) — port identical. A causeway settler pointing across the strip can genuinely box in (both waters + the byte-quirk retries) → our 8/28 deaths by t=1000 are the law working; retail settlers would die too given equal UNPAUSED sim time. BANKED: leave retail running unpaused near the causeway ~5 min and count splashes/vanishes.

## FU.6 Port fix ledger #2 (post-D1-D4)

| # | action | basis |
|---|---|---|
| **F1** | **Calibrate the MC2 sim tick to retail's frame-locked rate**: one sim tick per retail frame, DOS-era ~12-20 fps (measure in dosbox — banked); expose the ×4/×8 game-speed multiplier as the authentic speed option (EF:31800-31815). Our fixed high tick rate is the single biggest source of "everything covers too much ground per minute". | FU.1 |
| **F2** | **Pause semantics**: verify our map/menu screens freeze the world exactly where retail's GAME_PAUSED does (EF:40093; PlayerInput.cpp:406-417, GameUI.cpp:662-721). If retail's fullscreen map pauses (banked check), ours must too — map-watching is how herd drift gets judged. | FU.1 |
| F3 | No further law changes for goats/settlers: rows, masks, thresholds, RNG width, dispatch shape, death law all verified law-exact; the D1-D4 fixes stand. | FU.2-FU.5 |
| F4 | Optional fidelity: multiplayer skips the awake pass entirely (EF:40107) — note for the eventual MP column. | FU.1 |

**Banked retail checks (decisive, cheap):**
1. Dosbox fps during normal MC2 play on the reference rig (calibrates F1).
2. Does the fullscreen map pause the sim? (Watch a fleeing goat, open map 10 s, close — did it move?)
3. **The falsifier for the whole reconstruction**: unpaused 3-minute close-range timelapse of the goat herd (~2,500-3,500 retail ticks). The traced law predicts median per-goat drift ≈ 20-50 tiles. If retail goats verifiably stay within a few tiles over that window while UNPAUSED, the shipped binary's brains differ from remc2's reconstruction and a from-binary retrace of the wander brain is required (the GOG NETHERW.EXE is version-skewed vs remc2 — FU.3).
4. Retail causeway settlers over the same window: expect slow eastward/westward drift, occasional drownings, occasional silent house-entries.

## FU.7 (sixth addendum) — MC2's timing architecture, pinned

The player challenged FU.0's "one sim tick = one rendered frame" as needing hardening (MC2 reputedly ran a constant tick rate unlike MC1's infamous frame lock). Verdict after tracing the full pacing chain: **the frame-lock reading STANDS, and it is even looser than FU.0 implied — MC2's world tick is frame-locked and UNPACED by software.** There is no constant world-tick rate anywhere in the engine; the only fixed clock is a 120 Hz service timer that never paces the world.

1. **The world tick is called once per frame, free-running (architecture (a)).** The in-game outer loop (EF:31621-31659) is a bare `while(1) { ...; DrawAndEventsInGame_47560(GameTimerTurn_17DB54); }` with no wait, no delay, no timer test. `DrawAndEventsInGame_47560(turn)` (EF:31724) runs input, then the world pass ×1/×4/×8 by `speedIndex` (EF:31800-31815), then render — its `turn` argument (the raw timer counter) feeds ONLY `MouseAndKeysEvents_17A00` → `AdjustVolume_1A070(turn)` (volume-fade pacing) and the frame-time debug HUD (`byteindex_196 = GameTimerTurn − byteindex_196`, EF:31865-31867). No wait-on-timer, no accumulator, no reset of the counter in the game path.
2. **The ×4/×8 multiplier is the in-game SPEED KEY, not a catch-up accumulator.** `speedIndex` is cycled 0/1/2 by a player key (PlayerInput.cpp:1356-1385, prints "speed normal"; disabled in multiplayer/recording) — a fast-forward option. Its plain-loop structure (EF:31806-31815) is itself evidence there is no timer-accumulated tick decoupling: if the engine had a fixed-rate accumulator, the multiplier would scale a target, not loop the whole world pass.
3. **The only constant clock is 120 Hz, and it never paces the world.** At sound init the game registers `SimpleTimer_46820` (Sound.cpp:5929-5933, `GameTimerTurn_17DB54++`) on the Miles/AIL timer service at **120 Hz** (`AilSetTimerFrequency_92930(TimerIdx_F42A4, 120)` — EF:43027-43029; the frequency converts to a period as `1,000,000 µs / hertz`, Sound.cpp:1769-1774, → remc2 runs it as an 8 ms SDL_AddTimer, port_sdl_sound.cpp:712-722). The NO-SOUND fallback reprograms the PIT directly with **divisor 10022** (`SetProgrammableIntervalTimer_6FDA0`, EF:43050-43075: the commented original `outb(0x43,0x36); outb(0x40,div)` + int-8 hook; 1,193,182/10022 = **119.06 Hz** ≈ the same 120). Consumers of this clock: input repeat/volume fades, the FLC/cutscene frame pacer (`sub_75CB0` waits `GameTimerTurn < x_DWORD_E3844` then resets — Animation.cpp:588-601 — cutscenes ONLY), and the frame-time debug readout. **The world pass never waits on it.**
4. **No vsync pacing in the game frame either.** The VGA retrace wait (`sub_9A0FC_wait_to_screen_beam`, port 0x3DA bit 3 busy-wait — Animation.cpp:613-625) is used by the FLC player and the palette-fade loops (original commented at EF:51970/52009); the in-game blit `sub_90478_VGA_Blit320` (Basic.cpp:1378-1419, original VA 0x271478) contains NO original wait — remc2 ADDED `LockFps(maxFps)` there.
5. **The nominal number: there isn't one.** Retail MC2's effective world rate = whatever fps the software renderer achieved on the machine (period hardware at 320×200: roughly 15-25 fps, half that at 640×480 or high detail; dosbox: cycles-dependent — measurement still banked). The player's "MC2 runs a constant tick rate" is false at the software level; it FELT constant because the renderer load is fairly steady in-game.
6. **remc2's own calibration (the Q4 cross-check): 30 fps.** The reimplementation caps the in-game frame (and therefore the world tick, 1:1) with `maxGameFps`, **default 30** (read_config.cpp:24), applied via `LockFps` at every game blit (Basic.cpp:1617-1630; call sites EF:30868-30883, 31488-31490); menus 30, FMV 20 (read_config.cpp:25-26). That is the remc2 authors' retail-feel constant, validated by their regression stream and user base.
7. **Version-skew caveat: applies here too.** This chain is read from remc2's translated source (regression-instrumented against their base binary); the 120 Hz/divisor-10022 constants are data-like and unlikely to vary across builds, but the GOG NETHERW.EXE's timing path was not independently disassembled — it falls under the same banked binary-verification item as FU.3.

**F1, finalized:** implement the MC2 world tick as **frame-locked with a 30 Hz ceiling** (remc2's calibrated constant; one world pass per rendered frame, never more without the speed key) — i.e. if our port already ticks at a constant 30 Hz, F1 reduces to "correct at the ceiling", and the remaining retail-vs-port motion delta comes from (a) dosbox's real sub-30 fps (banked measurement — if it measures ~15-18, an optional "period hardware" tick-cap knob in the authenticity matrix is the faithful default candidate) and (b) the FU.1 pause fraction during observation. Expose ×4/×8 as the authentic speed-key option (EF:31806-31815). FU.0's frame-lock verdict is CONFIRMED, with the correction that the lock is unpaced-free-running, not "capped by a timer".
