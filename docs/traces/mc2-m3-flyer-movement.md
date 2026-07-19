# MC2 (5,3) MULTIPART FLYER — head movement, altitude, water & chain string-out — verbatim trace

Answers the PLAYTEST bug: *the (5,3) 16-segment "worm" (level-000) sits CONDENSED at its spawn point over
shallow water — the head never moves, so the 16 children stay stacked on it.* Companion to
`docs/traces/mc2-m0-m3-gaps.md` (the m0/m3 dispatch/tether/bob trace), `docs/traces/mc2-walker-wander-ai.md`
(the walker move-core laws), and `docs/traces/mc2-m22-worm-steering.md` (the (5,22) fixed-heading confirmation
— a close analog, but the (5,3) head is a DIFFERENT beast: it wanders, and it is a terrain-grounded walker).

All cites to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
`EF = engine/EventsFunctions.cpp`, `EV = engine/Events.cpp`, `LVL = engine/Level.cpp`,
`GT = engine/global_types.h`. Port cites: `multipart.rs`/`mobs.rs`/`behavior.rs` =
`crates/mgc-sim/src/mc2/…`. RNG law = per-entity uint16 LCG `r = 9377*r + 9439`. Trace date 2026-07-11.

---

## HEADLINE VERDICT (read first)

**The (5,3) head is NOT a flyer. Behavior row 74 makes it a TERRAIN-GROUNDED WALKER: it snaps to terrain
altitude every tick (`sub_580E0`, v_14 = −64), it REFUSES water (v_20 = 0xFFF080FE blocks tile-type 0), and
it DIES if boxed in on water (flags byte_160_0x20_32 & 1 = die-on-water, SET on row 74).** It runs the exact
same shared move core `sub_1B8C0` the goat/villager use — there is no altitude "flyer arm", the head grazes
the ground like a snake, at `actSpeed` 30 (ctor) decaying toward maxSpeed 16.

**The head DOES wander** (unlike the (5,22) worm, which is provably steer-less): row-74 idle (`sub_1BF90`,
state 0x19) applies the standard two-draw wander-turn to `roll_0x20_32` every `v_26 = 30` ticks, magnitude
85..340, and `yaw` chases `roll` at up to `v_2 = 34` per tick. So a lone, awake head on open, walkable
terrain wanders and its 16 children string out behind it at `-word_0x36_54` along the exact 3D bearing.

**Therefore the CONDENSED symptom has exactly two retail-faithful causes, and BOTH are already in our port:**

1. **ASLEEP.** While the player is FAR, the head's `byte_0x39_57` (f58) counts to 0. When asleep the head
   STILL moves (`sub_1B8C0` is unconditional in `sub_1BF90`) — but the CHILDREN's follow arm
   (`sub_1B6B0`) only runs when `byte_0x39_57 != 0`; asleep, each child instead **snaps ONTO the parent's
   exact position every 4th phase** (`if (!(byte_0x3E_62 & 3)) CopyEntityPosition(child, parent)`, EF:8727).
   ⇒ **an asleep worm is legitimately a stack of 16 sprites on one point.** It only strings out once the
   player comes within ≈24 tiles (`dist² < 0x2400000`). This is retail; the port matches (`mc2_awake_pass`,
   `mc2_child_tick`).
2. **BLOCKED ON/NEAR WATER.** The screenshot is a worm over shallow water at a shore. Row 74 refuses water
   AND steep terrain. If the head's fixed-ish heading points at water on all four move candidates (the
   original + 3 retry yaws), `sub_1B8C0` commits NO movement and — because flags bit0 (die-on-water) is set —
   sets `life = −1` (boxed-in suicide) the moment it is standing on water or fully surrounded. A head parked
   on a water tile at spawn cannot walk off it in any tried direction ⇒ it sits stock-still (children snap
   onto it) until it either finds a walkable retry yaw or dies. This is ALSO retail-faithful; the port's
   `mc2_move_core` all-blocked arm transcribes it.

**No missing "flyer altitude arm" exists to add — row 74 is a grounded walker. The one real fidelity risk is
whether the LEVEL authors this worm ON a water tile (a data question) and whether the head can ever escape.**
Concrete port deltas are minor (§5): the port is faithful on move-core, altitude, water refusal, wander, and
the awake/child string-out. The prime suspects are (a) spawn-on-water data + (b) the block-retry/roughness
over-refusal already fixed for walkers (D2) — re-verify it bites here too.

---

## 1. The m3 head's action handlers — the class-5 action table (EF:1242) & the IDLE state

### 1.1 Dispatch (from `mc2-m0-m3-gaps.md` §0/§5, re-confirmed)

The m3 head band is states **0x18..0x1F** in `x_DWORD_D4C52ar_str50` (EF:1267-1274). The ctor `sub_4B6F0`
(EF:33797) spawns the head in **`actionIndex = 25 = 0x19`** (= M3_BASE+1, the IDLE/wander state). The
recovered handlers (EF:11581-11621), aggro base 24:

| state | addr | handler | body |
|---|---|---|---|
| 0x18 | 0x200950 | `sub_1F950` | `sub_1BD90(a1x, 24)` — PATROL (no move) |
| **0x19** | **0x200970** | **`sub_1F970`** | **`sub_1BF90(a1x, 24)` — IDLE/WANDER (the spawn state)** |
| 0x1A | 0x200990 | `sub_1F990` | `if sub_1C310(a1x,24,sub_1CC20) SOUND 8` — chase/attack |
| 0x1B | 0x2009E0 | `sub_1F9E0` | `sub_1C560(a1x, 24)` — pack-follow |
| 0x1C | 0x200A00 | `sub_1FA00` | `PreKillEntity_1C890(a1x, 24)` |
| 0x1D | 0x200A20 | `sub_1FA20` | `KillEntity_1C930(a1x)` |
| 0x1E | 0x200A40 | `sub_1FA40` | **UNRECOVERED** (gaps §4 — held inert) |
| 0x1F | 0x200A50 | `sub_1FA50` | `sub_1D5D0(a1x, 24)` — spawn/appear |

**Critical: m3's head states are thin wrappers over the SHARED walker primitives — NO tether, NO bob** (bob
`sub_1F040` and tether `sub_1F0C0` are m0-ONLY; the m3 wrappers do not call them, gaps §5). So the m3 head's
only movement is inside `sub_1BF90` → `sub_1B8C0`. **`M3_BASE+1` maps to `sub_1F970` = `sub_1BF90(_, 24)`.**

### 1.2 `sub_1F970` (state 0x19) — VERBATIM (EF:11587-11592)

```c
void sub_1F970(type_entity_0x6E8E* a1x)//200970
{
    sub_1BF90(a1x, 24);
}
```

That is the ENTIRE idle handler. `sub_1BF90` is the identical wander core the goat/villager run (transcribed
in `mc2-walker-wander-ai.md §3` and re-read below for the m3's row-74 params).

### 1.3 `sub_1BF90` — the wander body, m3-relevant excerpts (EF:9064-9234) — VERBATIM

(Full body in `mc2-walker-wander-ai.md §3`; the load-bearing arms for m3:)

```c
// --- damage/life head (EF:9086-9128): inbox → life, weakest-child life, death latch → v2 ---
// v2==1 (hit): a1x->word_0x96_150 = word_0x26_38; state = a2 + (flags&8 ? 6 : 2); sub_1EEE0(a1x);
//   row 74 flags byte_0x20_32 = 0x01 (bit3 CLEAR) ⇒ a HIT sends the head to a2+2 = 0x1A (CHASE), not flee.
// v2==2 (dead): state = a2 + 4 = 0x1C (PreKill → chain cascade).

if (v2 < 1) {                                     // QUIET path (no hit, alive)
    if (!v2) {
        sub_1B8C0(a1x);                            // *** THE MOVE — EVERY TICK, unconditional ***  (§1.4)
        if (!(a1x->byte_0x3E_62 % a1x->dword_0xA0_160x->word_160_0x1a_26)) {   // every v_26 = 30 ticks
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;   // draw 1
            v9 = a1x->rand_0x14_20;
            a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;   // draw 2
            a1x->roll_0x20_32 += ((a1x->rand_0x14_20 & 0xFF) + 85) * (2*((v9 % 0x9D)/79) - 1);   // WANDER TURN
            a1x->roll_0x20_32 &= 0x7ff;
            if (a1x->byte_0x39_57) {               // *** SCAN gate = AWAKE only ***
                // wizard scan within v_28 = 5120 & cone v_30 = 170 → target + state a2+2 (flags&8? +6)
                // else pack scan (flags&4 clear ⇒ ENABLED for row 74): leaderless same-model → +3 follow
            }
        }
    }
}
```

**Two things the port must get exactly right (it does):**
- **`sub_1B8C0` runs UNCONDITIONALLY** in the quiet path (EF:9133) — the head moves whether awake or asleep.
  The awake byte gates only the wander-*scan* (wizard/pack), NOT the move and NOT the wander-turn nudge.
  (So a far-away head still cruises; only its children condense — see §4.)
- **The wander-turn writes `roll` (target heading) every 30 ticks;** `yaw` (travel heading) chases `roll` at
  `v_2 = 34`/tick in the move core (§1.4). ⇒ the head genuinely wanders (contrast (5,22): NO wander law).

### 1.4 `sub_1B8C0` — the SHARED walker move core (EF:8741-8939) — the altitude & step, VERBATIM

The head's model is 3, not 27, so it takes the generic else-branch:

```c
predictedAxis_EB398ar = a1x->position_0x4C_76;
v5 = a1x->dword_0xA0_160x->word_160_0xe_14;      // v_14  (row 74 = −64)  ← z-step
v6 = a1x->dword_0xA0_160x->word_160_0xa_10;      // v_10  (row 74 = 256)  ← DEAD arg (a4 unused, §1.5)
v7 = a1x->dword_0xA0_160x->word_160_0xc_12;      // v_12  (row 74 = 0)    ← hover
v8 = getTerrainAlt_10C40(&predictedAxis_EB398ar);
sub_580E0(&predictedAxis_EB398ar, v8, v7, v6, v5);          // *** ALTITUDE CORE — grounds the head ***
MoveEntity_57FA0(&predictedAxis_EB398ar, a1x->yaw_0x1C_28, 0, a1x->actSpeed_0x82_130);   // step along YAW
if (pos.x>>8 != pred.x>>8 || pos.y>>8 != pred.y>>8) {       // crossing a tile boundary → BLOCK TEST
    if (sub_102D0(a1x, &pred, 1)                            // water/permission-mask refusal  (§2)
        || sub_1B7A0_tile_compare(&pred) >= a1x->dword_0xA0_160x->word_160_0x10_16)   // slope ≥ v_16 = 20
    {
        a1x->struct_byte_0xc_12_15.byte[2] |= 4u;           // set the "blocked" status bit
        // RETRY 1: yaw += 341 (~+60°), re-probe
        // RETRY 2: LOBYTE=yaw0-85, HIBYTE=((yaw0-341)>>8)&7, re-probe
        // RETRY 3: (yaw0+0x400) & (0x700 + (uint8)yaw0)  [reverse], re-probe
        // ALL FOUR BLOCKED (EF:8855):
        if (a1x->dword_0xA0_160x->byte_160_0x20_32 & 1                          // *** DIE-ON-WATER flag ***
            || sub_104D0_terrain_tile_is_water(&a1x->position_0x4C_76) == 1)    // OR standing on water
            a1x->life_0x8 = -1;                                                 // *** SUICIDE ***
        result = 4;   // NO position commit
    }
    // any successful commit: CopyEntityPosition; yaw += sub_58350(yaw, roll, DEAD, v_2=34) & 0x7ff
}
```

**`sub_580E0` — the altitude core (EF:40372) — VERBATIM:**
```c
void sub_580E0(axis_3d* a1x, signed int a2 /*terrainAlt*/, int a3 /*v_12 hover*/, int /*a4 DEAD*/, __int16 a5 /*v_14 zStep*/) {
    if (a1x->z > a2)               a1x->z += a5;              // ABOVE ground → step by v_14
    if ((int16)a1x->z <= a2 + a3)  a1x->z = a3 + a2;         // AT/BELOW ground+hover → SNAP to it
}
```

**Row-74 servo params (LVL:86 = `str_D7BD6[74]`, decoded against GT:75-94):**

| field | off | value | meaning |
|---|---|---|---|
| type_160_0x0 | 0x0 | 0x0F = 15 | the row's type label (the "B(15,…)" row) |
| **subtype_160_0x2 (v_2)** | 0x2 | **0x22 = 34** | **yaw→roll turn clamp / tick (`sub_58350` a4)** |
| word_160_0x4 (v_4) | 0x4 | 5 | `sub_58350` a3 — **DEAD** |
| word_160_0x6 | 0x6 | 0x55 = 85 | (pitch-turn clamp, walker path unused) |
| word_160_0x8 | 0x8 | 5 | — |
| word_160_0xa (v_10) | 0xa | 0x100 = 256 | `sub_580E0` a4 — **DEAD** |
| **word_160_0xc (v_12)** | 0xc | **0** | altitude hover offset (0 ⇒ sits exactly on terrain) |
| **word_160_0xe (v_14)** | 0xe | **0xFFC0 = −64** | altitude z-step: **SINK −64/tick toward ground** |
| **word_160_0x10 (v_16)** | 0x10 | **0x14 = 20** | slope-refusal threshold (`sub_1B7A0_tile_compare ≥ 20`) |
| word_160_0x12 | 0x12 | 0x155 = 341 | — |
| **dword_160_0x14 (v_20)** | 0x14 | **0xFFF080FE** | terrain permission mask (blocks water/village/rough) |
| word_160_0x18 pair | 0x18 | {0x00,0x14} | `word_160_0x18_18` = 0x1400 (NOT the flags) |
| **word_160_0x1a (v_26)** | 0x1a | **0x1E = 30** | wander/decision period + wake seed |
| word_160_0x1c (v_28) | 0x1c | 0x1400 = 5120 | wizard/pack scan radius (20 tiles) |
| word_160_0x1e (v_30) | 0x1e | 0x00AA = 170 | scan cone (≈ ±30°) |
| **byte_160_0x20_32 (flags)** | 0x20 | **0x01** | **die-on-water(bit0)=SET; flee(bit3)=CLEAR; pack-disable(bit2)=CLEAR** |

**Move core = WALKER core, NOT flyer.** The Z law is: sink −64/tick while above terrain, then hard-snap to
terrain height (`hover = 0`). There is NO cave ceiling, NO bob, NO aerial hover — the head grazes the ground.
(This is why it looks like a "worm on the shore" and not a bird.) **The passability/slope/water refusal that
pins WALKERS applies to this head IN FULL** — same `sub_102D0(_,_,1)` water mask and same
`sub_1B7A0_tile_compare ≥ v_16` slope wall. **The flyer name is a misnomer; row 74 is a grounded creature.**

**IMPORTANT flags-field correction (applies to the whole family):** every runtime flag test —
die-on-water `& 1` (EF:8855), pack-disable `& 4` (EF:9022/9178), flee `& 8` (EF:9003/9170/9227) — reads
**`byte_160_0x20_32`** (the offset-0x20 byte), which for row 74 = **0x01**. It is NOT the `{0x00,0x14}` word
pair at offset 0x18. Our `behavior.rs` extracts this correctly: **port BEHAVIOR[74].flags = 0x1** (verified;
DIE_ON_WATER=1, FLEE=8 clear, PACK_DISABLE=4 clear). Do not confuse the two fields when auditing.

### 1.5 The `sub_58350` turn clamp (EF:40391) — VERBATIM (confirms v_2 = 34 governs the head)

```c
int sub_58350(uint16 a1_yaw, int16 a2_roll, int /*a3 v_4 DEAD*/, uint16 a4_v2) {
    if (a1 == a2) return 0;
    v4 = sub_582B0(a1, a2);                        // |angdist yaw→roll|
    v5 = sub_582F0(a1, a2);                        // sign toward roll
    v6 = v4; if ((int16)v4 > (int)a4) v6 = a4;     // clamp to a4 = v_2 = 34
    return v5 * v6;
}
```

`roll` = target heading; `yaw` = travel heading chasing `roll` at ≤ 34/tick (≈6°/tick — a moderate slew, not
the (5,22) worm's fast 227). The move steps along `yaw`; `MoveEntity_57FA0` = `x += speed·sin(yaw);
y −= speed·cos(yaw)` (2048/turn, 16.16 tables), speed = `actSpeed`. Ctor `actSpeed = 30`; there is no
`actSpeed` decay in `sub_1BF90`/`sub_1B8C0` (unlike the m22 head's `sub_26FF0`), so the m3 head cruises at 30
until a chase/flee wrapper changes speed. **PORT: `mc2_move_core` uses `turn_cap = BEHAVIOR[row].v_2` (the D1
fix; mobs.rs:221), `mc2_alt_core` uses `row.v_14`/`row.v_12` (mobs.rs:141-148) — FAITHFUL.**

---

## 2. Water — does die-on-water apply to the m3 head? Can it cross water?

**YES the die-on-water law applies (flags & 1 = 0x01 for row 74), and NO the head CANNOT cross water.**

### 2.1 The refusal — `sub_102D0` a3&1 (EF:3659-3686) — VERBATIM

```c
if (a3 & 1) {
    v16x = *a2;                                                  // predicted point
    v6 = max(a1x->array_0x52_82.pitch, a1x->array_0x52_82.roll); // reach = XY half-extent
    v8 = 0;
    while (v8 <= v6) {                                            // step along yaw in 256-unit hops
        v18 = ~a1x->dword_0xA0_160x->dword_160_0x14_20;          // ~v_20 = ~0xFFF080FE = 0x000F0F01
        v9  = sub_104D0_terrain_tile_is_water(&v16x);            // 1 << f(tileType); water(type0) → bit0 = 1
        result = v18 & v9;                                       // 0x0F0F01 & 1 = 1 for water → BLOCKED
        if (result) return result;
        if (isCaveLevel_D41B6) { … }                            // level-000 is a night map, not cave — skipped
        v8 += 256;
        MoveEntity_57FA0(&v16x, a1x->yaw_0x1C_28, 0, 256);
    }
}
```

`~v_20 = ~0xFFF080FE = 0x000F0F01`; ANDing with `sub_104D0`'s 1-hot tile bit: **water (type 0 → bit 0)
gives 1 → nonzero → BLOCKED.** (The mask also blocks the village bands 8/9 and the rough types, same as the
walkers — `mc2-walker-wander-ai.md §7`.) So **the m3 head refuses to step onto a water tile**, exactly like a
goat. It cannot cross water in retail.

### 2.2 Spawned over/near water (the screenshot case)

The ctor places the head at `getTerrainAlt(pos) + …` (via `AddEventToMap_57D70`) — if the AUTHORED spawn
coordinate is a water tile, the head starts standing ON water. Then per tick:
- Its wander cruise tries to step off. If the current heading (and all 3 retry yaws) point into more water
  (a worm parked in a shallow inlet/at a shore points at water on most bearings), **all four candidates are
  blocked** → `sub_1B8C0` returns 4, commits NO move.
- On the all-blocked tick, **`byte_160_0x20_32 & 1` is TRUE (flags = 0x01) OR the head is standing on water
  (`sub_104D0(...) == 1`)** → **`life = −1`** (boxed-in suicide, EF:8855-8862). The head dies.
- Until it dies (or finds one walkable retry yaw), the head does not move ⇒ **the 16 children (asleep, or even
  awake but with the head stationary) sit condensed on it.** That is the screenshot.

So the retail behavior for "worm spawned on/at water" is: **sit still, then die** (if truly boxed) — or wander
off the moment one retry yaw finds land. It never "sits condensed forever" unless the level authors it fully
surrounded by water AND it happens never to be standing on a water tile itself (impossible if surrounded).
**The most likely real cause of the exact screenshot is the head being authored on a shallow-water tile
and dying/refusing.** The children then condense because the head is not moving. This is retail-faithful IF
the level data really places it there — see §5 OPEN (level census).

### 2.3 Contrast with the m22 head

(5,22) uses the SAME move core and the SAME row-style water/slope refusal (its row 90 also v_20 = 0xFFF080FE,
also die-on-water byte_0x20_32 = 0x05 → bit0 SET). BUT the m22 head flies at `terrain + 384` (its
`sub_26FF0` z-ceiling, `mc2-m22-worm-steering.md §1.3`), whereas the m3 head is GROUNDED (v_14 = −64, snap to
terrain). So the m3 head is MORE exposed to the water refusal than the m22 head — a grounded creature standing
on a water tile boxes in immediately, while a flyer skims above. **This is the salient difference and the
likely reason (5,3) shows the shore-parked bug where (5,22) does not.**

---

## 3. The wander cadence — confirming the head DOES wander

**CONFIRMED: the m3 head wanders (unlike the (5,22) worm, which provably has no steering).**

Driving row-74 fields:
- **`v_26 = 30`** (`word_160_0x1a_26`) — the wander-turn (and scan) PERIOD: the nudge fires when
  `byte_0x3E_62 % 30 == 0` (EF:9134). `byte_0x3E_62` (f63) is the per-tick age, phase-staggered by the
  per-model spawn ordinal in the ctor, so a herd of worms nudges on different ticks.
- **Turn magnitude 85..340** (`((rand2 & 0xFF) + 85)`) with sign −1 at p = 79/157 (`2*((rand1 % 0x9D)/79) − 1`,
  EF:9139). ≈15°..60° per nudge, mean ≈37°.
- **`v_2 = 34`** (`subtype_160_0x2_2`) — `yaw` catches `roll` at ≤34/tick (§1.5); within ~4-10 ticks of the
  30-tick period `yaw` locks onto the new `roll`. ⇒ a kinked, quasi-Brownian walk at `actSpeed` 30.

**Why (5,3) wanders but (5,22) does not:** the m3 head runs the GENERIC idle handler `sub_1BF90`, which OWNS
the wander-turn law (EF:9136-9141). The m22 head runs its OWN handler `sub_26960`, which calls
`sub_26FF0`/`sub_272C0`/`sub_26F10`/`sub_27880` and **never touches `roll`** except on a hit/castle-acquire
(`mc2-m22-worm-steering.md §1.2`). So the m22 head cruises a fixed spawn heading; the m3 head genuinely
random-walks. **PORT: `mc2_idle` calls `mc2_wander_turn` on the `f63 % v_26 == 0` cadence (mobs.rs:524-525) —
FAITHFUL.**

Note: the head starts with `roll = yaw = (rand & 0x7FF) − 1` from ONE ctor draw (EF:33818-33820) — a random
initial heading, then the wander law bends it. (Port `mc2_ctor_facing`, roster.rs.)

---

## 4. The chain string-out — children (state 0xE8, `sub_1B6B0` EF:8696) & the awake gate

### 4.1 `sub_1B6B0` — the child tick — VERBATIM (EF:8696-8735)

```c
void sub_1B6B0(type_entity_0x6E8E* a1x)//1fc6b0
{
    v1x = Entities_EA3E4[a1x->word_0x32_50];                 // PARENT (link toward head)
    if (v1x->class_0x3F_63 != 5)
        DisableEntityDrawing04_57F10(a1x);                  // parent gone/not-a-creature → orphan reap
    if (a1x->byte_0x39_57)                                   // *** AWAKE ***
    {
        a1x->yaw_0x1C_28   = sub_581E0_maybe_tan2(&a1x->pos, &v1x->pos);     // face parent (XY)
        a1x->pitch_0x1E_30 = sub_58210_radix_tan(&a1x->pos, &v1x->pos);      // face parent (Z)
        predictedAxis_EB398ar = v1x->position_0x4C_76;                        // start at PARENT pos
        MoveEntity_57FA0(&predictedAxis_EB398ar, a1x->yaw_0x1C_28, a1x->pitch_0x1E_30,
                         -a1x->word_0x36_54);                                 // step BACK by −word_0x36_54
        CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);               // COMMIT: rigid trail
        if (a1x->str_0x5E_94.word_0x62_98) {                                 // own damage intake
            a1x->word_0x26_38 = a1x->str_0x5E_94.word_0x62_98;
            a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;
            a1x->str_0x5E_94.word_0x62_98 = 0;
        } else { a1x->word_0x26_38 = 0; }
    }
    else if (!(a1x->byte_0x3E_62 & 3))                       // *** ASLEEP — every 4th phase only ***
    {
        CopyEntityPosition_57CF0(a1x, &v1x->position_0x4C_76);   // SNAP onto the parent → CONDENSED
        a1x->yaw_0x1C_28 = v1x->yaw_0x1C_28;
    }
}
```

**Awake gate semantics for CHILDREN (`byte_0x39_57` = f58):**
- **Awake (f58 ≠ 0):** rigid follow — the child sits exactly `word_0x36_54` behind the parent along the exact
  3D bearing parent→child. Each child trails the one ahead of it (`word_0x32_50` = the toward-head link), so
  the 16 segments string out into a line. `word_0x36_54` = 96 on the head, and per-child overridden to the
  particle-row spacing (EF:33846-51: `65% of speed_6`, child 0 = 125% of that). ⇒ the chain has real length.
- **Asleep (f58 == 0):** the child does NOTHING except, every 4th phase (`byte_0x3E_62 & 3 == 0`), **teleport
  onto the parent's exact position.** ⇒ **all 16 children collapse onto the head = CONDENSED.**

### 4.2 The awake pass — head & child f58 lifecycle (`sub_68BF0`/`sub_68C70` EF:55469-55542) — VERBATIM

```c
int sub_68C70(type_entity_0x6E8E* a1x) {
    if (a1x->byte_0x39_57) {                                 // head already awake:
        for (i = a1x->word_0x34_52; Entities[i] > Entities[0]; i = child->word_0x34_52)
            child->byte_0x39_57 = a1x->byte_0x39_57;         //   push head's f58 to EVERY child
        a1x->byte_0x39_57--;                                 //   then head decrements
        return 0;
    }
    …                                                        // asleep: wait out byte_0x3A_58 delay, then:
    if (EuclideanDistXY_584D0(&a1x->pos, &player->pos) < 0x2400000) {   // player within ≈24 tiles
        a1x->byte_0x39_57 = 16;                              //   wake head to 16
        for (v2 = a1x->word_0x34_52; Entities[v2] > Entities[0]; v2 = child->word_0x34_52)
            child->byte_0x39_57 = a1x->byte_0x39_57 + 2;     //   wake children to 18 (+2)
    }
}
```

**So: while the player is FAR the whole worm is asleep (f58 → 0).** The HEAD still MOVES every tick (§1.3,
`sub_1B8C0` is unconditional and does NOT read `byte_0x39_57`), but the CHILDREN only snap onto the head
(§4.1). ⇒ **an asleep worm is a moving head with 16 sprites stacked on it — condensed by design.** When the
player closes to ≈24 tiles, the head wakes to 16 (children to 18), and thereafter the children string out
behind the moving head. This is the intended "the worm uncoils when you approach" behavior.

**PORT: `mc2_awake_pass` (mobs.rs:2162-2198) transcribes this exactly** — head f58 pushed to children then
decremented while awake; on proximity head→16, children→18 (mobs.rs:2189-2194). `mc2_child_tick`
(multipart.rs:345-376) transcribes `sub_1B6B0`: awake = rigid `-f56` trail along the 3D bearing + damage
intake; asleep = every-4th-phase snap onto parent + yaw copy. **FAITHFUL.**

### 4.3 Does the retail HEAD move while asleep? YES.

Confirmed above: `sub_1B8C0` (the only mover) has no `byte_0x39_57` gate. **A far, asleep m3 head wanders;
its children stay condensed on it until the player approaches.** So "asleep worms legitimately sit condensed"
is CORRECT for the CHILDREN, but the HEAD is drifting under them the whole time (you just can't tell — one
sprite-stack drifting). The stack only STOPS drifting if the head is BLOCKED (water/steep, §2) — which is the
screenshot's likely second ingredient.

---

## 5. PORT DELTA — `multipart.rs` `m3_tick` / generic primitives vs retail

**Bottom line: the (5,3) head movement, altitude, water refusal, wander, awake gate and child string-out are
a FAITHFUL port. There is NO missing flyer altitude arm — row 74 is a grounded walker, and our `mc2_idle`
grounding it via v_14 is CORRECT, not a bug. Do NOT add an aerial/hover arm.** The condensation is retail
behavior (asleep children and/or a water-blocked head). Concrete items:

| # | item | port site | retail | verdict |
|---|---|---|---|---|
| **NONE (altitude)** | m3 idle grounds the head (v_14=−64 sink-to-terrain, hover=0) | `mc2_move_candidate`/`mc2_alt_core` mobs.rs:141-203 (uses `row.v_14`/`row.v_12`) | `sub_580E0` EF:40372 with row-74 v_14=−64, v_12=0 | **CORRECT — row 74 IS a grounded walker; the "flyer" name is a misnomer. Do NOT add a flyer altitude arm.** |
| **NONE (water)** | head refuses water + die-on-water on all-blocked | `mc2_path_blocked` mobs.rs:165-187 (~v_20 & cap_bit) + all-blocked suicide mobs.rs:268-273 (flags & DIE_ON_WATER, cap_bit==1) | `sub_102D0` EF:3659-3686 + EF:8855-8862 (`byte_0x20_32 & 1`) | **CORRECT — water refusal SHOULD block the head; that is the retail law. flags=0x1 extracted correctly (behavior.rs[74]).** |
| **NONE (wander)** | head wanders every v_26=30, clamp v_2=34 | `mc2_idle`→`mc2_move_core`+`mc2_wander_turn` mobs.rs:511-545, turn_cap=v_2 | `sub_1BF90` EF:9133-9141 + `sub_58350` v_2 | **CORRECT — the head DOES wander (contrast (5,22)). FAITHFUL.** |
| **NONE (string-out)** | awake child trail `-f56` / asleep snap; head moves regardless of awake | `mc2_child_tick` multipart.rs:345-376 + `mc2_awake_pass` mobs.rs:2162-2198 | `sub_1B6B0` EF:8696-8735 + `sub_68C70` EF:55494 | **CORRECT — asleep = condensed by design; head drifts under the stack. FAITHFUL.** |
| **CHECK-A (data)** | is this worm AUTHORED on a water tile? | (level-000 data, not code) | ctor `AddEventToMap` uses the authored coord | **The prime suspect.** If (5,3) is authored on/at a shallow-water tile, it boxes in / dies AND its children condense — RETAIL-faithful, but visually the reported bug. **Census the level-000 (5,3) record coordinate & its terrain type (§OPEN).** If it is genuinely on water, the "bug" is retail data (a doomed worm), not a port defect. |
| **CHECK-B (retry/roughness over-refusal)** | does the block-retry / roughness wall over-refuse the head near the shore, pinning it where retail would step off? | `mc2_path_blocked` loop shape (D2 fix, mobs.rs:171-186) + `roughness ≥ v_16` mobs.rs:201 | `sub_102D0` `while(v8<=reach)` EF:3667 + `sub_1B7A0_tile_compare ≥ 20` EF:8810 | **VERIFY the D2 walker fix bites here.** The head's XY reach = `max(array.pitch, array.roll)` = `60% of particle-row speed_6` (ctor EF:33869-71) — likely > 255, so the probe is MULTI-STEP (unlike the goat's single probe). Confirm `mc2_path_blocked` steps the FULL reach and does not test one extra tile (D2). An over-refusal here would pin a shore worm that retail would walk off, DEEPENING the condensation beyond retail. |
| **CHECK-C (per-worm RNG facing)** | each worm draws its own ctor facing so a herd fans out | `mc2_ctor_facing` roster.rs + `mc2_spawn_m3` multipart.rs:271 (one `mc2_rand`) | EF:33818-33820 (one draw per head) | Confirm each head advances its OWN `rand_0x14_20` stream (per-entity seed). If all heads seed identically they pick identical headings and over-condense (same as the (5,22) CHECK-1). |

**Recommended actions (impact order):**
1. **Census the level-000 (5,3) authored record: coordinate + terrain-type at that cell** (CHECK-A). If it is
   a water tile, the worm is a doomed/blocked retail data case — the "bug" is faithful; note it and move on
   (or flag it as a broken retail level datum, cf. MC1 level 039). If it is LAND, proceed to 2/3.
2. **Instrument a headless run:** park the player NEXT TO the worm (so it wakes, f58 set) and log (a) the head's
   XY displacement over N ticks, (b) each child's distance from the head. Expected once awake: the head
   random-walks at 30/tick and the children string out to a ~16·spacing line. If the head does NOT move while
   awake on walkable land → a block/roughness over-refusal (CHECK-B) or a facing-RNG collapse (CHECK-C).
3. **Re-verify the D2 block-probe fix** for the m3 head's larger XY reach (multi-step probe) — it was tuned on
   walker extents (goat 0, villager 128); the m3 head's reach is bigger and multi-step, so re-read
   `mc2_path_blocked`'s loop against `sub_102D0`'s `while(v8 <= reach)` for the >255 case.
4. **Do NOT** add a flyer altitude arm, remove the water refusal, or add "make the head move when blocked" —
   all three would DIVERGE from retail. The head is a grounded, water-refusing walker by row-74 law.

---

## OPEN

1. **Level-000 (5,3) authored coordinate + terrain type** — not censused here. THE decisive datum: is the
   worm authored on/at a water tile (⇒ retail-faithful box-in/condensation) or on land (⇒ a real port
   over-refusal to hunt)? A record census keyed on class 5 model 3 (like `mc2census`) with the terrain-type
   at each spawn cell settles it. **Prime suspect.**
2. **CHECK-B: the m3 head's XY reach (`array.pitch`/`.roll`) and whether `mc2_path_blocked` handles its
   multi-step probe exactly** — the head's extent = `60% of particlesParameters_D951C[88].speed_6` (ctor
   EF:33869); if > 255 the `sub_102D0` a3&1 loop probes several 256-hops, and the D2 fix (tuned for
   single-probe walkers) must be re-confirmed for the multi-probe case. Not re-diffed in the sim this pass.
3. **m3 state 0x1E `sub_1FA40` UNRECOVERED** (gaps §4) — held inert in the port. Structural guess = flee slot.
   Does not affect the idle/condensation path (the head spawns and stays in 0x19 absent a hit). Banked.
4. **`sub_1B7A0_tile_compare` exact metric** — the slope-refusal (v_16 = 20) uses the cross-corner gradient
   (`mc2-walker-wander-ai.md §2.4`, TER:1578-1600), assumed identical to the port `roughness`. A divergence
   would over-refuse the head at shore hills and deepen condensation. Not re-diffed here.
5. **`v_10 = 256` (word_160_0xa) is the DEAD `sub_580E0` a4 arg** — confirmed unused (EF:40372 signature has
   `int /*a4*/`). Noted so nobody wires it into the altitude core. Port `mc2_alt_core` correctly ignores it.
