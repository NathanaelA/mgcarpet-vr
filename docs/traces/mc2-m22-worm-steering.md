# MC2 (5,22) segmented WORM — HEAD STEERING & dispersal, verbatim decompile trace

Answers the PLAYTEST question: *worms stay heavily condensed in a small area — does retail steer the worm
somewhere we missed, or do retail worms genuinely mill in place?* Companion to
`docs/traces/mc2-m22-worm-helpers.md` (the helper suite) and `mc2-multipart-chains.md` (ctor/tail).

All cites to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`:
`EF = engine/EventsFunctions.cpp`, `EV = engine/Events.cpp`, `LVL = engine/Level.cpp`,
`GT = engine/global_types.h`. Port cites: `multipart.rs`/`mobs.rs`/`roster.rs` =
`crates/mgc-sim/src/mc2/…`. RNG law = per-entity uint16 LCG `rand = 9377*rand + 9439`. Trace date 2026-07-11.

---

## HEADLINE VERDICT (read first)

**Retail worm heads genuinely mill / cruise on a FIXED spawn heading. There is NO wander-turn law for the
m22 head, and NO steering block our port missed.** The idle head (state 0xB0) never re-writes its target
heading `roll_0x20_32` — the ctor seeds `roll = yaw = (rand & 0x7FF) − 1` once (EF:34392-34393), and the
head cruises in that ONE direction until a hit (`sub_26F10`/`sub_26D20`) or castle-acquire (`sub_26AA0`)
overwrites `roll`. The port replicates all of this faithfully (ctor `mc2_ctor_facing` roster.rs:85, move
core `mc2_move_core` mobs.rs:209, turn clamp v_2=227). **So the "condensed" symptom is NOT a missing steer.**

The dispersal that DOES happen in retail comes from three things, all present in our port:
1. **the fixed-heading straight cruise** at `actSpeed` 16/tick from a *random per-worm* spawn direction — worms authored at nearby points fan OUT along their distinct headings;
2. **block-retry yaw kicks** (+341, −85, reverse) when the cruise hits water / steep terrain (`sub_1B8C0`), after which yaw decays back toward the fixed `roll` at up to **227/tick** (v_2, very fast) — a hard, fast turn that makes the worm graze along terrain edges;
3. **the anti-stack z-hop** (`sub_27120`) which only separates worms *vertically*, never laterally.

If the port's worms look MORE condensed than retail, the cause is almost certainly one of:
(a) the port's authored spawn points are clustered and the RNG facing draws are not diverging the headings (verify the spawn RNG stream advances per worm — §2), or
(b) the effective-sim-time / frame-rate calibration issue already documented for walkers (`mc2-walker-wander-ai.md` FU.1/FU.6 — our fixed high tick rate covers proportionally MORE ground, which would make ours LESS condensed, not more), or
(c) all worms authored at literally ONE point (retail also starts condensed there and disperses only by the straight cruise — §2).

**This is an APPROX / faithful-behavior confirmation, not a bug in the movement port.** The one concrete
port-fidelity gap found is minor and in the spin field, not the heading (§5, D-SPIN).

---

## 1. Who steers the m22 head?

### 1.1 The class-5 action table binding (0xB0..0xB7)

The m22 head band 0xB0..0xB7 dispatches through the `pre_sub_4A190_0x6E8E(row.address_6, mx)` address
switch (EV, the `x_DWORD_D4C52ar_str50` action-table region near EV:1236). The m22 bindings
(from `mc2-m22-worm-helpers.md` state roster, verified against EV:1847-1922):

| state | dispatch (EV) | handler | role |
|---|---|---|---|
| 0xB0 176 | EV:1847 `0x207960` | `sub_26960` EF:17247 | **IDLE head** |
| 0xB1 177 | EV:1851 `0x207990` | `sub_26990` EF:17255 | chase + colorize sweep |
| 0xB2 178 | EV:1855 `0x207aa0` | `sub_26AA0` EF:17313 | castle acquire |
| 0xB3 179 | EV:1859 `0x207bd0` | `sub_26BD0` EF:17373 | castle deposit / shrink |
| 0xB4 180 | EV:1863 `0x207ca0` | `sub_26CA0` EF:17420 | tail-segment tick |
| 0xB5 181 | EV:1867 `0x207cc0` | `sub_26CC0` EF:17427 | chain-kill |
| 0xB6 182 | — | — (interior of `sub_27720`) | no unique body |
| 0xB7 183 | EV:1921 `0x208930` | `sub_27930` EF:18046 | spawn/appear |

### 1.2 The IDLE head handler `sub_26960` (0xB0) — TRANSCRIBED COMPLETELY (EF:17247-17253)

```c
void sub_26960(type_entity_0x6E8E* a1x)//207960
{
    sub_26FF0(a1x);   // move + altitude core  (§1.3)
    sub_272C0(a1x);   // writhe anim + serpentine SPIN advance  (§3)
    sub_26F10(a1x);   // damage-turn + retarget + death→0xB5  (§1.5)
    sub_27880(a1x);   // 1024-tick tail-grow + mana regen
}
```

**There is NO `sub_1B8C0` call here, NO rand draw, NO write to `roll_0x20_32`, NO wander-turn nudge.** The
only movement is inside `sub_26FF0` → `sub_1B8C0`, which consumes the ALREADY-SET `yaw`/`roll`. Grepping the
entire m22 head region (EF:17247-17600) for `roll_0x20_32` / `rand_0x14_20` / `yaw_0x1C_28` writes yields
ONLY:
- `sub_26AA0` (0xB2, EF:17348): `roll = tan2(self→castle)` — castle-acquire bank, not idle;
- `sub_26D20` (relay, EF:17473-17474): `yaw = roll = tan2(attacker→self)` — on a SEGMENT hit;
- `sub_26F10` (EF:17567-17568): `yaw = roll = tan2(attacker→self)` — on a HEAD hit.

**In pure idle (0xB0, no hits, no castle), the head's `roll` is NEVER touched.** It keeps the ctor's random
value forever. (Verified: `awk 'NR>=17247 && NR<=17600' | grep roll_0x20_32/rand/yaw` → only the three
sites above.)

### 1.3 `sub_26FF0` — the head move core, TRANSCRIBED (EF:17589-17651)

```c
void sub_26FF0(type_entity_0x6E8E* a1x)//207ff0
{
    v1 = a1x->actSpeed_0x82_130;
    if (v1 > a1x->maxSpeed_0x86_134)                       // decay actSpeed toward cruise (16)
        a1x->actSpeed_0x82_130 = v1 - 2;                   //   −2/tick, floors at maxSpeed via the >test
    v2 = a1x->array_0x52_82.fov;                           // save fov
    v3 = a1x->array_0x52_82.pitch;                         // save pitch
    SetEntityShiftRot_49EA0(a1x, a1x->byte_0x46_70 << 8, a1x->array_0x52_82.fov);  // shift = tailLen<<8
    sub_1B8C0(a1x);                                        // *** THE MOVE *** (§1.4) — consumes yaw, chases roll
    if (!(a1x->byte_0x3E_62 & 0xF))                        // every 16th frame
        sub_27120(a1x);                                    //   anti-stack z-push (VERTICAL only)
    v11x = a1x; v12 = 0;
    SetEntityShiftRot_49EA0(a1x, v3, v2);                  // restore pitch, fov
    while (v11x != Entities_EA3E4[0]) {                    // walk head + entire word_0x34_52 chain
        v4 = getTerrainAlt_10C40(&v11x->position_0x4C_76);
        if ((int16)v4 > (int16)v12) { v12 = v4; v9x = v11x->position_0x4C_76; }  // highest terrain alt + where
        v11x = Entities_EA3E4[v11x->word_0x34_52];
    }
    v12 += 384;                                            // ceiling = highest terrain + 384
    v5x = a1x->position_0x4C_76.z;
    if ((int16)v5x >= (int16)v12) {                        // at/above ceiling → SINK
        v7 = a1x->word_0x24_36;
        if (v7) a1x->word_0x24_36 = v7 - 1;                //   burn rise budget, hold z
        else    a1x->position_0x4C_76.z = v5x - 2;         //   sink −2
    } else {                                               // below ceiling → RISE
        v6 = a1x->dword_0xA0_160x->word_160_0x10_16;       //   row-90 baseline (=20)
        v5x = sub_1B7A0_tile_compare(&v9x);                //   roughness at the highest-alt cell
        if (v5x > v6) a1x->position_0x4C_76.z += 0x100;    //   steep terrain → +256
        else          a1x->position_0x4C_76.z += 0x40;     //   gentle → +64
        a1x->word_0x24_36 = 0x40;                          //   refill rise budget = 64
    }
}
```

**Confirmed: `sub_26FF0` steers NOTHING laterally.** It only (a) runs the shared move core, (b) declumps
vertically, (c) does the terrain-follow z clamp. There is NO steering block our port skipped — `multipart.rs`
`m22_move` (lines 760-807) transcribes this arm-for-arm (actSpeed decay, the shift-rot bracket, the every-16th
anti-stack, the whole-chain ceiling + rise-budget). **PORT FAITHFUL.**

### 1.4 `sub_1B8C0` — the shared walker move core, the yaw↔roll relationship (EF:8741-8939)

This is the SAME move core the ground-walkers use (`mc2-walker-wander-ai.md §2`). Key excerpts (the
non-cave, non-model-27 else-branch):

```c
predictedAxis_EB398ar = a1x->position_0x4C_76;
sub_580E0(&pred, getTerrainAlt(&pred), v_12, v_10, v_14);          // altitude core
MoveEntity_57FA0(&pred, a1x->yaw_0x1C_28, 0, a1x->actSpeed_0x82_130);   // *** step along YAW ONLY ***
if (pos.x>>8 != pred.x>>8 || pos.y>>8 != pred.y>>8) {              // crossing a tile boundary
    if (sub_102D0(a1x,&pred,1) || sub_1B7A0_tile_compare(&pred) >= v_16) {   // BLOCKED?
        a1x->yaw_0x1C_28 = (yaw0 + 341) & 0x7ff;                   // RETRY 1: kick yaw +341 (~+60°)
        … re-probe …
        //   RETRY 2: LOBYTE=yaw0-85, HIBYTE=((yaw0-341)>>8)&7
        //   RETRY 3: (yaw0+0x400) & (0x700 + (uint8)yaw0)   ← reverse
        //   all four blocked → die-on-water/boxed-in: life = -1
    }
}
// on ANY successful commit (results 1/2/3):
CopyEntityPosition_57CF0(a1x, &pred);
v = a1x->yaw_0x1C_28
  + sub_58350(a1x->yaw_0x1C_28,               // a1 = current yaw
              a1x->roll_0x20_32,               // a2 = TARGET heading (roll)
              a1x->dword_0xA0_160x->word_160_0x4_4,       // a3 = v_4 (DEAD in sub_58350)
              a1x->dword_0xA0_160x->subtype_160_0x2_2);   // a4 = v_2 = the turn clamp
a1x->yaw_0x1C_28 = v & 0x7ff;                  // yaw chases roll at ≤ v_2/tick
```

`sub_58350` (EF:40391-40405) VERBATIM:
```c
int sub_58350(uint16 a1_yaw, int16 a2_roll, int /*a3 DEAD*/, uint16 a4_cap)
{
    if (a1 == a2) return 0;
    v4 = sub_582B0(a1, a2);          // angular distance yaw→roll
    v5 = sub_582F0(a1, a2);          // sign (±1) toward roll
    v6 = v4; if ((int16)v4 > (int)a4) v6 = a4;   // clamp magnitude to a4 = v_2
    return v5 * v6;                  // signed step toward roll, |step| ≤ v_2
}
```

**So `roll` = the target heading, `yaw` = the actual travel heading chasing `roll` at ≤ v_2 per tick.**
For the m22 head, **v_2 (`subtype_160_0x2_2` of row 90) = 227** (see §1.6) — an extremely fast slew
(≈40°/tick), so yaw locks onto roll almost instantly. With `roll` fixed (idle), the worm travels dead
straight; when a block-retry kicks yaw off, it snaps back to the fixed `roll` within ~1 tick. **PORT
FAITHFUL** — `mc2_move_core` (mobs.rs:209-275) uses `turn_cap = BEHAVIOR[row].v_2` (the D1 fix; mobs.rs:221)
and `turn_step` (mc1/mobs.rs:594) = `sub_58350` exactly (`min(angdist, cap)` signed toward target).

### 1.5 `sub_26F10` — the ONLY idle-state heading writer, and it is HIT-driven (EF:17542-17583)

```c
void sub_26F10(a1x):
    if a1x->byte_0x39_57:                            // awake
        if a1x->str_0x5E_94.word_0x62_98:            // a HIT is pending on the HEAD mailbox
            actSpeed += dmg/4  (clamp [maxSpeed,minSpeed])         // accelerate (does NOT subtract life)
            v4 = tan2(&Entities[word_0x62_98].pos, &self.pos)      // face the ATTACKER (attacker→self)
            a1x->str_0x5E_94.word_0x62_98 = 0
            a1x->yaw_0x1C_28 = v4                     // *** the only idle-path yaw write ***
            a1x->roll_0x20_32 = v4                    // *** the only idle-path roll write ***
        v5 = a1x->str_0x5E_94.word_0x68_104           // TARGET-tag
        if v5 && v5 != playerEntityIndex:
            playerEntityIndex = v5; actionIndex = 177 (0xB1); dword_0x10_16 = 0; SOUND 4
    if a1x->life_0x8 < 0: a1x->actionIndex_0x45_69 = 181 (0xB5)   // death → chain-kill
```

So the ONLY way an idle worm changes heading is by being HIT (then it turns to face and charges the
attacker). **Absent any hit, `roll` and hence `yaw` are frozen at the ctor's random spawn angle.** The head
is also effectively melee-immune (the mailbox accelerates but never subtracts life — `mc2-m22-worm-helpers.md`
§3/§12), so `sub_26F10` mostly just redirects the worm at whoever poked it. **PORT FAITHFUL** —
`multipart.rs` `m22_dmg` (859-902) transcribes this including the "turn to face attacker" yaw/roll write.

### 1.6 Behavior row 90 (`str_D7BD6[90]`), field-decoded (LVL:101 + GT:75-94)

`str_D7BD6[90]` (the m22 head's `dword_0xA0_160x`, set in the ctor EF:34397) is LVL line 11+90 = **101**:
```
{0x001E,0x00E3,0x0005,0x0016,0x0005,0x0100,0x0000,0xFF80,0x0014,0x0200,0xFFF080FE,{0x00,0x09},0x0023,0x1400,0x02AA,0x00,{0x00}}
```
Decoded against `type_str_160` (GT:75-94):

| field | off | value | meaning |
|---|---|---|---|
| type_160_0x0_0 | 0x0 | 0x1E = 30 | type |
| **subtype_160_0x2_2 (v_2)** | 0x2 | **0xE3 = 227** | **yaw→roll turn clamp / tick (≈40°/tick)** |
| word_160_0x4_4 (v_4) | 0x4 | 5 | `sub_58350` a3 — **DEAD** |
| word_160_0x6_6 | 0x6 | 0x16 = 22 | (pitch-turn clamp, unused by walker path) |
| word_160_0x8_8 | 0x8 | 5 | — |
| word_160_0xa_10 (v_10) | 0xa | 0x100 = 256 | alt-core |
| word_160_0xc_12 (v_12) | 0xc | 0 | alt-core hover |
| word_160_0xe_14 (v_14) | 0xe | 0xFF80 = −128 | alt-core z-step (sink toward ground) |
| **word_160_0x10_16 (v_16)** | 0x10 | 0x14 = 20 | slope-refusal threshold (block test) + the `sub_26FF0` steep-rise gate |
| word_160_0x12_18 | 0x12 | 0x200 = 512 | — |
| dword_160_0x14_20 (v_20) | 0x14 | 0xFFF080FE | terrain permission mask (blocks water/village/rough) |
| flags | 0x18 | {0x00,0x09} | **flags = 9** = die-on-water(bit0) + flee-on-hit(bit3) |
| word_160_0x1a_26 (v_26) | 0x1a | 0x23 = 35 | decision-period baseline (used only for `byte_0x39_57` wake seed) |
| word_160_0x1c_28 (v_28) | 0x1c | 0x1400 = 5120 | scan radius (unused by m22 head — no wander scan) |
| word_160_0x1e_30 (v_30) | 0x1e | 0x2AA = 682 | scan cone (unused) |
| byte_160_0x20_32 | 0x20 | 0 | flag byte |

The critical value is **v_2 = 227**: the head turns toward its target heading fast. Combined with "idle never
changes the target heading", the worm cruises dead straight. (Our `behavior.rs` extracts the same
`str_D7BD6` initializer → row 90 v_2 = 227, v_16 = 20; PORT FAITHFUL.)

---

## 2. Spawn dispersal — how worms are authored and where they start

### 2.1 The ctor `sub_4CA00` (EF:34377-34417) — the random spawn heading

```c
v1x->actionIndex_0x45_69 = 176;                  // 0xB0 idle
v1x->class_0x3F_63 = 5;  v1x->model_0x40_64 = 22;
v1x->minSpeed_0x84_132 = 128;  v1x->maxSpeed_0x86_134 = 16;
v1x->actSpeed_0x82_130 = 16;                     // cruise speed 16/tick
v1x->rand_0x14_20 = 9377 * v1x->rand_0x14_20 + 9439;   // ONE RNG draw
v1x->roll_0x20_32 = (v1x->rand_0x14_20 & 0x7FF) - 1;   // *** RANDOM target heading ***
v1x->yaw_0x1C_28  = (v1x->rand_0x14_20 & 0x7FF) - 1;   // *** = same value (yaw seeded to roll) ***
v1x->pitch_0x1E_30 = v1x->roll_0x20_32;
v1x->maxLife_0x4 = 2000;
v1x->dword_0xA0_160x = &str_D7BD6[90];           // behavior row 90 (§1.6)
v1x->byte_0x3E_62 = D41A0_0.array_0x10[22]++;    // per-model spawn ordinal (phase-staggers ticks)
v1x->word_0x2C_44 = 11;                          // spin rate (COSMETIC — §3)
v1x->word_0x96_150 = 1024;                       // grow timer
v1x->byte_0x46_70 = 15;                          // default tail length (map overrides, §2.2)
// head placed at getTerrainAlt(pos) + 384
```

**Every worm gets a distinct random heading** (the per-entity `rand_0x14_20` stream advances once per ctor).
So a herd of worms authored at *distinct* points fans out along distinct straight cruises; a herd authored at
ONE point starts stacked and disperses only via those distinct headings + the vertical anti-stack + terrain
deflections. **PORT FAITHFUL** — `mc2_ctor_facing` (roster.rs:85-92) does `f34 = f30 = f32 = (rand&0x7FF)−1`
with one `mc2_rand(i)` draw; `mc2_spawn_m22` (multipart.rs:454-497) sets actSpeed 16, row 90, spin 11, etc.

### 2.2 The authored / trigger spawn seam `sub_4A310` (EF ~33025)

The authored-record path (per `type_entity_0x30311` row, one head per record):
```c
if (indexx->model_0x40_64 == 0x16) {              // model 22
    indexx->byte_0x46_70 = entity->par1_14 & 0xff;   // tail length = par1 (map author sets it)
    sub_4CB60(indexx);                               // spawn the tail rings (§2.3)
}
```
So **one authored (5,22) record ⇒ one head at that record's position, with par1-controlled tail length.**
There is **no per-record placement jitter** in the ctor or this seam — worms start exactly where authored.
**Whether several worms are authored at one point vs spread out is a per-LEVEL data question** (OPEN — a
level census of (5,22) record coordinates would settle whether retail *also* starts condensed). Our port's
`mc2_spawn_m22(par1)` (multipart.rs:494) mirrors: `f71 = par1 & 0xFF` then `mc2_m22_spawn_tail`. **PORT
FAITHFUL** for the seam; no jitter to add.

### 2.3 Tail spawn `sub_4CB60` (EF:34420-34451)

Spawns `(tailLen/2)` rings × 2 segments (+n,−n) all COPYING the head's fields (so segments inherit the head
position + a spiral offset applied by `sub_274C0`/`sub_271D0`), then colorize + shift-rot + one follow pass.
Segments follow the head; they do not add lateral dispersal of their own. **PORT FAITHFUL**
(`mc2_m22_spawn_tail` multipart.rs:502-521).

**Conclusion for §2:** worms do NOT self-jitter at spawn. If several are authored at one point they START
condensed; retail disperses them ONLY by the fixed-heading cruise (§1) + vertical declump. This matches the
port. If the port looks more condensed than retail, check that the port's per-worm RNG facing stream actually
diverges the headings (each worm must draw its own `rand`), and re-read the effective-sim-time note (§4/HEADLINE).

---

## 3. The serpentine spin — does it feed the movement heading? NO.

### 3.1 `sub_272C0` (EF:17720-17776) advances the spin

```c
// (writhe animation frames, tailLen>=11, SOUND 48 — omitted; unchanged from helper doc §2)
a1x->subSpellIndex_0x2A_42 += a1x->word_0x2C_44;   // advance SERPENTINE ANGLE by the spin rate
a1x->subSpellIndex_0x2A_42 &= 0x7ff;               // wrap to 11-bit angle
if (!(a1x->byte_0x3E_62 & 3)) {                    // every 4th frame: decay spin magnitude toward ±11
    v6 = abs(a1x->word_0x2C_44) - 5;
    if ((int16)v6 < 11) v6 = 11;
    a1x->word_0x2C_44 = (a1x->word_0x2C_44 <= 0) ? -v6 : v6;   // sign preserved
}
```

`subSpellIndex_0x2A_42` = the "serpentine spin angle"; `word_0x2C_44` = its per-tick rate (starts 11, bumped
by `sub_26D20` on a hit up to ±227, decays back to ±11).

### 3.2 Where the spin angle is CONSUMED — segment orbit only, NOT the head move

`subSpellIndex_0x2A_42` is read in exactly ONE place: `sub_271D0` (EF:17685-17712), the tail-segment
spiral-follow, via `sub_273C0`:
```c
// sub_271D0 (a segment), reading its HEAD (Entities[word_0x96_150]):
v3 = head->subSpellIndex_0x2A_42;                         // the head's spin angle
v4 = (v3 + sub_273C0(head->animationFrame, head->word_0x36_54,
                     seg->byte_0x46_70, head->byte_0x46_70)) & 0x7FF;
seg->word_0x2C_44 = v4;                                   // the segment's orbit yaw
… MoveEntity_57FA0(&pred, v4, 0, …) …                     // step the SEGMENT around the head's position
```

So the spin angle drives where each TAIL SEGMENT sits in its coil around the head — it is the *visual writhe*
of the body. **It is NEVER read by the HEAD's move core.** The head move (`sub_1B8C0`, §1.4) steps along
`yaw_0x1C_28` only; `MoveEntity_57FA0(&pred, a1x->yaw_0x1C_28, 0, actSpeed)`. **The serpentine spin does NOT
feed the head heading and is NOT the mechanism of dispersal.** (Dispersal = the straight cruise, §1.)

**Move-core consumption of yaw vs roll for the m22 head:** the head uses `yaw` for the polar step and chases
`roll` at v_2/tick; `roll` is the target and is idle-frozen; `subSpellIndex`/spin is orthogonal (body
cosmetics). `MoveEntity_57FA0` (`Player.cpp:6`) is `x += speed·sin(yaw); y −= speed·cos(yaw)` (2048/turn,
16.16 tables), speed = actSpeed = 16. **PORT FAITHFUL** — `m22_anim` (multipart.rs:827-857) advances `f46`
(subSpellIndex) by `f44` and decays `f44`; the head move uses `f30` (yaw). Spin feeds only `m22_tail_follow`.

---

## 4. The burrow / underground arm — NONE (m22 does not burrow)

The old MC1 worm burrows underground; **MC2's (5,22) does NOT.** The m22 head's only z handling is in
`sub_26FF0` (§1.3): a *terrain-follow ceiling* — it flies at `highest-chain-terrain + 384`, rising +64/+256
below the ceiling (with the `word_0x24_36` rise-budget hysteresis) and sinking −2 at/above it. There is NO
underground state, NO terrain-below z, NO lateral movement modulation from z. The block test refuses steep
tiles (`sub_1B7A0_tile_compare >= v_16=20`) and impassable terrain (mask `0xFFF080FE` = water/village/rough),
which is a HORIZONTAL wall, not a burrow. So the worm is an above-ground flyer-crawler that hugs the terrain
ceiling. **No underground z handling changes its lateral movement.** **PORT FAITHFUL** (`m22_move`
multipart.rs:772-806 transcribes the ceiling/rise-budget; no burrow arm exists to port).

---

## 5. PORT DELTA — what multipart.rs m22 movement is missing or doing differently

**Bottom line: the m22 HEAD MOVEMENT is a faithful port. No steering block is missing.** The worm-condensation
symptom is expected retail behavior (fixed-heading cruise). Concrete items:

| # | item | port site | retail | severity |
|---|---|---|---|---|
| **NONE (heading)** | idle head has no wander-turn; `roll` frozen at spawn | multipart.rs:1026-1031 (0xB0 calls move/anim/dmg/grow, no roll nudge) | EF:17247-17253 — same four calls, no nudge | **Correct — do NOT add a wander law** |
| **D-SPIN (minor fidelity)** | spin decay sign uses `if spin <= 0 {-mag} else {mag}` on the DECAYED `f44`; retail computes `v6=|f44|-5` clamped to ≥11 then re-signs from the ORIGINAL `word_0x2C_44 <= 0` | multipart.rs:852-856 | EF:17766-17773 — `if (word_0x2C_44 <= 0) v7=-v6 else v7=v6` (sign from pre-decay value) | The port already reads `spin` (pre-decay) for the sign — **matches**. Re-verify the `>=11` clamp is `.max(11)` on the magnitude (multipart.rs:854 `.max(11)` ✓). No fix needed; listed for completeness. |
| **CHECK-1** | per-worm RNG facing must actually diverge headings | `mc2_ctor_facing` roster.rs:85-92 (one `mc2_rand(i)` draw) | EF:34391-34393 (one `rand` draw per worm) | Confirm each worm advances its OWN `rand_0x14_20` stream (i.e. the seed is per-entity, not shared) so the herd fans out. If the port seeds all worms with the same RNG state at spawn, they'd all pick the SAME heading → over-condensation. **This is the single most likely real cause if the port is worse than retail — verify the per-entity RNG seed.** |
| **CHECK-2** | effective sim-time / tick-rate | (engine tick loop) | `mc2-walker-wander-ai.md` FU.1/FU.6 (frame-locked 30 Hz ceiling) | Our fixed high tick rate makes worms cover MORE ground/min than dosbox retail (→ LESS condensed), so this can't cause over-condensation — but if the port ticks worms SLOWER than 30 Hz relative to render, they'd move less. Sanity-check the m22 head participates in the normal per-tick dispatch at the same rate as walkers. |

**Recommended action:** do NOT add heading steering to the idle worm — that would DIVERGE from retail. Instead:
1. **Verify CHECK-1** (per-worm RNG facing divergence) — most likely the real over-condensation cause.
2. Instrument a headless level with the authored (5,22) records: log each worm's spawn `f34` heading and its
   XY displacement over N ticks. Retail law predicts each worm travels a straight line at 16/tick from a
   distinct random heading (fanning out), deflecting only at water/steep edges. If the port's worms show
   IDENTICAL headings → CHECK-1 is the bug. If distinct headings but still condensed → confirm the block
   test isn't over-refusing (compare `mc2_path_blocked` extent probe, already fixed as D2 for walkers).
3. If retail-vs-port still differs with distinct headings and correct blocks, the worms are likely authored
   at one point on that level and retail ALSO starts condensed (§2) — a playtest expectation mismatch, not a
   port bug.

---

## OPEN

1. **Per-level (5,22) authored coordinates** — not censused here. Needed to answer "do several worms spawn at
   ONE point?" definitively (§2). A level-record census (like the mc2census instrument) keyed on class 5
   model 22 would list spawn XY spread and par1 tail lengths.
2. **CHECK-1 (per-entity RNG seed at m22 spawn)** — the decompile draws one `rand_0x14_20` per worm from the
   entity's own evolving stream (EF:34391). Confirm `mc2_rand(i)`/`new_event` gives each worm a distinct,
   advancing seed. Flagged as the prime suspect; not verified in the sim this pass.
3. **`sub_1B7A0_tile_compare` exact metric** — the steep-rise gate (v_16=20) and the block roughness use the
   cross-corner gradient (`mc2-walker-wander-ai.md §2.4`, TER:1578-1600). Assumed identical to the walker port
   (`roughness`); a divergence would over-refuse worm moves and could deepen condensation. Not re-diffed here.
4. **`sub_26D20` spin bump feeding writhe** — the hit relay bumps `word_0x2C_44` to ±227 (EF:17475-17494),
   which only speeds up the serpentine writhe (via `sub_272C0` → `subSpellIndex`), NOT the head heading
   (which `sub_26D20` sets separately to face the attacker, EF:17473-17474). Confirmed orthogonal; noted so
   nobody mistakes the spin bump for a movement steer.
