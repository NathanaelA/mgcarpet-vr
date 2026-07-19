# MC2 class-5 model-22 (segmented worm / castle-mana thief) — HELPER SUITE Verbatim Trace

Companion to `docs/traces/mc2-multipart-chains.md` (§1.22, §4, §6). That file pins the ctor `sub_4CA00`,
tail spawn `sub_4CB60`, segment-spawn primitive `sub_274C0`, spiral-follow `sub_271D0`, hit relay `sub_26D20`,
and chain-kill `sub_26CC0`. THIS file traces the SUMMARIZED / OPEN helpers of the m22 head + tail suite.

Addressing: `sub_XXXXX` = address − 0x1E1000. All `EF:line` = `reference/remc2/remc2/engine/EventsFunctions.cpp`,
`EV:line` = `.../Events.cpp`. Field names are the decomp names (`word_0x2C_44`, `dword_0x10_16`, …).
Every RNG advance (`rand = 9377*rand + 9439`) is called out because the port must reproduce draw ORDER byte-for-byte.

State roster for m22 (base 176 = 0xB0 = 22×8; `sub_268F0` maps `actionIndex = a2 − 88`, so "target" 264/0xB1... etc.):

| state | handler | dispatch | role |
|---|---|---|---|
| 0xB0 (176) | `sub_26960` EF:17247 | EV:1847 (0x207960) | idle head tick |
| 0xB1 (177) | `sub_26990` EF:17255 | EV:1851 (0x207990) | chase player + segment-colorize scan |
| 0xB2 (178) | `sub_26AA0` EF:17313 | EV:1855 (0x207aa0) | castle acquire (mana-drain arming) |
| 0xB3 (179) | `sub_26BD0` EF:17373 | EV:1859 (0x207bd0) | castle DRAIN deposit / tail shrink |
| 0xB4 (180) | `sub_26CA0` EF:17420 | EV:1863 (0x207ca0) | TAIL-SEGMENT tick (follow + relay) |
| 0xB5 (181) | `sub_26CC0` EF:17427 | EV:1867 (0x207cc0) | CHAIN-KILL (mana-sphere the whole worm) |
| 0xB6 (182) | — (no unique body) | — | mislabel; interior of `sub_27720` (EF:17938) |
| 0xB7 (183) | `sub_27930` EF:18046 | EV:1921 (0x208930) | spawn/appear → `sub_1D5D0(event,176)` |

CTOR (`sub_4CA00`, EF:34377) recap of the fields the helpers read: `actionIndex=176`, `minSpeed=128`, `maxSpeed=16`,
`actSpeed=16`, `maxLife=2000`, `dword_0xA0_160x=&str_D7BD6[90]`, `word_0x2C_44=11`, `subSpellIndex_0x2A_42=0`,
`word_0x36_54=0`, `word_0x96_150=1024`, `word_0x24_36=0`, `byte_0x46_70=15`, `byte_0x38_56=3`,
`animationFrame_0x5C_92=0`, `array_0x52_82.yaw=0`, `playerEntityIndex_0x94_148=0`; head placed at `terrainAlt+384`.
One RNG draw (EF:34390). `byte_0x46_70` is later overwritten from map `par1_14 & 0xff` at `sub_4A310` (EF:33025-28)
before `sub_4CB60` spawns the tail. Head is later `SetEvent144_49C70`'d (mana table) + `CopyMaxLifeToLife`.

---

## 1. `sub_26FF0` (EF:17589) — HEAD move + altitude core

Called by 0xB0/0xB1/0xB2 (NOT 0xB3). Handles actSpeed decay, the base move (`sub_1B8C0`), the every-16th-frame
anti-stack push (`sub_27120`), and the whole-chain max-terrain-altitude clamp with rise/sink authority.

```c
void sub_26FF0(a1x):                                      // EF:17589
    v1 = a1x->actSpeed_0x82_130                            // EF:17603
    if v1 > a1x->maxSpeed_0x86_134:                        // decay one step toward max cruise
        a1x->actSpeed_0x82_130 = v1 - 2                    //   actSpeed -= 2  (EF:17605)

    v2 = a1x->array_0x52_82.fov                            // save fov  (EF:17606)
    v3 = a1x->array_0x52_82.pitch                          // save pitch (EF:17607)
    // temporarily set shift = tailLen<<8, fov = current fov, for the move step:
    SetEntityShiftRot_49EA0(a1x, a1x->byte_0x46_70 << 8, a1x->array_0x52_82.fov)   // EF:17608
    sub_1B8C0(a1x)                                         // base move core (EF:17609)

    if !(a1x->byte_0x3E_62 & 0xF):                         // every 16th frame (per instance counter)
        sub_27120(a1x)                                     //   anti-stack z-push (EF:17610-11)

    SetEntityShiftRot_49EA0(a1x, v3, v2)                   // restore pitch,fov (EF:17614)

    // --- whole-chain highest terrain altitude ---
    v11x = a1x; v12 = 0                                    // EF:17612-13
    while v11x != Entities_EA3E4[0]:                       // walk head + entire word_0x34_52 chain
        v4 = getTerrainAlt_10C40(&v11x->position_0x4C_76)  // EF:17617
        if (int16)v4 > (int16)v12:
            v12 = v4                                        // remember highest terrain alt…
            v9x = v11x->position_0x4C_76                    // …and the position where it occurred (EF:17621)
        v11x = Entities_EA3E4[v11x->word_0x34_52]          // next chain link
    v12 += 384                                             // ceiling = highest terrain + 384 (EF:17625)

    v5x = a1x->position_0x4C_76.z                          // EF:17626
    if (int16)v5x >= (int16)v12:                           // head at/above ceiling → SINK
        v7 = a1x->word_0x24_36                              // pending "rise budget"
        if v7:
            a1x->word_0x24_36 = v7 - 1                      // burn rise budget, DON'T move z (EF:17633)
        else:
            a1x->position_0x4C_76.z = v5x - 2               // sink slowly (z -= 2)  (EF:17637)
    else:                                                   // head below ceiling → RISE
        v6 = a1x->dword_0xA0_160x->word_160_0x10_16         // type baseline alt threshold (row 90)
        v5x = sub_1B7A0_tile_compare(&v9x)                  // terrain-type code at the highest-alt cell
        if v5x > v6:
            a1x->position_0x4C_76.z += 0x100                // steep rise +256 over "high" terrain (EF:17645)
        else:
            a1x->position_0x4C_76.z += 0x40                 // gentle rise +64 otherwise (EF:17647)
        a1x->word_0x24_36 = 0x40                            // refill rise budget = 64 (EF:17648)
```

Exact conditions (for the port):
- **actSpeed decay:** if `actSpeed > maxSpeed(16)` then `actSpeed -= 2` each tick (never below maxSpeed, no floor test here — floor comes from `sub_26F10`/`sub_26AA0`).
- **anti-stack push:** fires when `(byte_0x3E_62 & 0xF) == 0`, i.e. every 16th frame keyed on the per-instance
  spawn counter `byte_0x3E_62` (so different worms phase-offset).
- **ceiling:** `max(terrainAlt over {head ∪ all word_0x34_52 segments}) + 384`.
- **at/above ceiling:** if `word_0x24_36 > 0` decrement it and hold z; else `z -= 2` (slow sink).
- **below ceiling:** rise `+0x100` if the highest-terrain tile's type code `sub_1B7A0_tile_compare(v9x) > word_160_0x10_16`, else `+0x40`; and reset `word_0x24_36 = 0x40` (a 64-tick "keep rising even if we clip the ceiling" hysteresis budget).
- The double `SetEntityShiftRot_49EA0` brackets the move so `sub_1B8C0` sees `shift = byte_0x46_70<<8` (tail length as a spacing shift) but the persistent `array_0x52_82.pitch/fov` are restored afterward.

RNG: none. Sounds: none.

---

## 2. `sub_272C0` (EF:17720) — animation / spin

Called by ALL of 0xB0/0xB1/0xB2/0xB3. Steps the writhe animation, emits the thrash sound, advances the
serpentine spin angle `subSpellIndex_0x2A_42`, and jitters the per-segment spin rate `word_0x2C_44`.

```c
void sub_272C0(a1x):                                       // EF:17720
    if a1x->byte_0x46_70 >= 11:                            // only worms with tailLen >= 11 animate/thrash
        v1 = sub_27430(a1x->animationFrame_0x5C_92)         // frame-step size from banding table (EF:17733)
        v2 = a1x->animationFrame_0x5C_92
        v9 = v1
        if v2 && v2 < 0x10:                                 // frame in (0, 16) → THRASH SOUND
            PrepareEventSound_6E450(a1x - struct, -1, 48)   // SOUND 48  (EF:17737)

        if a1x->word_0x36_54 & 1:                           // phase bit 0 set = counting UP
            v3 = v9 + a1x->animationFrame_0x5C_92
            a1x->animationFrame_0x5C_92 = v3                //   frame += step
            if v3 > 0x64:                                   //   clamp at 100 and flip to DOWN
                a1x->animationFrame_0x5C_92 = 100
                a1x->word_0x36_54 &= 0xFE                    //   clear bit0 (start counting down)
        else if a1x->animationFrame_0x5C_92 > v9:           // counting DOWN
            a1x->animationFrame_0x5C_92 -= v9               //   frame -= step
        else:                                               // hit floor → flip to UP, clear bit1
            v5 = a1x->word_0x36_54 | 1
            a1x->word_0x36_54 = v5
            a1x->animationFrame_0x5C_92 = 0
            a1x->word_0x36_54 = v5 ^ 2                      //   toggles bit1 each floor bounce (EF:17758)

    // --- spin, always (regardless of tailLen) ---
    a1x->subSpellIndex_0x2A_42 += a1x->word_0x2C_44         // advance serpentine angle by spin rate
    a1x->subSpellIndex_0x2A_42 &= 0x7FF                     // wrap to 11-bit angle (EF:17763)

    if !(a1x->byte_0x3E_62 & 3):                            // every 4th frame (per-instance phase)
        v6 = abs(a1x->word_0x2C_44) - 5                     // shrink spin magnitude by 5
        if (int16)v6 < 11: v6 = 11                          //   floor magnitude at 11
        if a1x->word_0x2C_44 <= 0: v7 = -v6                 //   preserve sign
        else:                     v7 = v6
        a1x->word_0x2C_44 = v7                              // (EF:17773)
```

Notes:
- `sub_27430(frame)` (EF:17806) is the step-size band: `frame>=96 → 2`, `>=87 → 3`, `>=60 → 4`,
  else `5 + (frame<30 ? 1 : 0)` (i.e. `<30 → 6`, `30..59 → 5`).
- SOUND **48** whenever the anim frame is in the open interval (0, 16) AND `byte_0x46_70 >= 11`.
- `word_0x36_54` is a 2-bit anim phase state (bit0 = up/down direction, bit1 = flip toggled each floor bounce).
- Spin magnitude `word_0x2C_44` decays by 5 every 4th frame toward a floor of ±11 (sign preserved). It is
  BUMPED back up by the aggro relay in `sub_26D20` (steer) and reset to 11 in the CTOR.

RNG: none. Sounds: 48.

---

## 3. `sub_26F10` (EF:17542) — damage-turn + death transition

Called by 0xB0/0xB2 (NOT 0xB1/0xB3). Consumes the head's own damage/target mailbox to STEER and RETARGET,
and transitions to the chain-kill state when head life goes negative.

```c
void sub_26F10(a1x):                                       // EF:17542
    if a1x->byte_0x39_57:                                   // active frame count nonzero
        if a1x->str_0x5E_94.word_0x62_98:                   // a HIT is pending on the head mailbox
            // boost speed proportional to damage, clamp to [maxSpeed, minSpeed]:
            v1 = (a1x->str_0x5E_94.dword_0x5E_94 >> 2) + a1x->actSpeed_0x82_130   // actSpeed += dmg/4
            a1x->actSpeed_0x82_130 = v1
            if (int16)v1 < a1x->maxSpeed_0x86_134: a1x->actSpeed_0x82_130 = maxSpeed(16)
            if a1x->actSpeed_0x82_130 > a1x->minSpeed_0x84_132: a1x->actSpeed_0x82_130 = minSpeed(128)
            v4 = tan2(&Entities[word_0x62_98].pos, &self.pos)  // face the attacker
            a1x->str_0x5E_94.word_0x62_98 = 0               // consume the hit
            a1x->yaw_0x1C_28 = v4
            a1x->roll_0x20_32 = v4
        v5 = a1x->str_0x5E_94.word_0x68_104                  // a TARGET-tag mailbox
        if v5:
            if v5 != a1x->playerEntityIndex_0x94_148:
                a1x->playerEntityIndex_0x94_148 = v5         // adopt new target
                a1x->actionIndex_0x45_69 = 177 (0xB1)        // → chase state
                a1x->dword_0x10_16 = 0
                PrepareEventSound_6E450(v5, -1, 4)           // SOUND 4  (EF:17577)
            a1x->str_0x5E_94.word_0x68_104 = 0

    if a1x->life_0x8 < 0:                                    // *** DEATH ***
        a1x->actionIndex_0x45_69 = 181 (0xB5)                // → chain-kill  (EF:17583)
```

**CRITICAL FINDING — the head is damage-IMMUNE through its own mailbox.** `sub_26F10` reads `dword_0x5E_94`
only to *accelerate* the head (dmg/4) and never subtracts it from `life_0x8`. None of the m22 head states
(0xB0-0xB3) call `sub_26830` (the chain-life-min aggregator at EF:17154 that DOES subtract). So a melee/projectile
hit landing on the HEAD mailbox never lowers head life. Head `life_0x8` reaching `< 0` (→ state 181) must come
from an external write (see §12). Sound **4** at EF:17515/17524 belongs to `sub_26D20`; the SAME sound 4 fires
here at EF:17577 when the head adopts a new target-tag.

RNG: none. Sounds: 4 (retarget).

---

## 4. `sub_27880` (EF:18012) — tail-grow timer + mana regen

Called by 0xB0/0xB2. A countdown on `word_0x96_150`; on expiry, grows the tail (up to 15) and regenerates mana.

```c
void sub_27880(a1x):                                       // EF:18012
    v1 = a1x->word_0x96_150
    if v1:
        a1x->word_0x96_150 = v1 - 1                         // tick the 1024-step timer down
    else:
        v2 = a1x->byte_0x46_70                              // current tail length
        a1x->word_0x96_150 = 1024                           // reload timer
        if v2 <= 13:
            sub_27720(a1x, v2 + 2)                          // grow tail by 2 (→ ≤15)
        v3 = a1x->mana_0x90_144
        if v3 < 50000:
            a1x->mana_0x90_144 = v3 + 1000                  // regen +1000, cap 50000
```

Note the dual use of `word_0x96_150`: in the HEAD it is this 1024-tick grow timer; in a SEGMENT it is the head
back-reference (set by `sub_274C0` to `event1 - struct`). Grow condition is `tailLen <= 13` (so it steps 13→15,
never overshoots 15). Mana caps at 50000 and grows +1000 per 1024-tick cycle. `sub_27720(v2+2)` internally
`| 1`'s the length (odd tail lengths), so real lengths are 1,3,5,…,15.

RNG: none. Sounds: none.

---

## 5. `sub_27720` (EF:17938) — tail GROW / SHRINK (spans 0x27720–0x27880; contains the mislabeled "0xB6 handler" at 0x27920)

Grows or shrinks the tail to a target odd length `a2`. Grow = spawn a +/- segment PAIR via `NewEvent`+`sub_274C0`;
shrink = hide segment PAIRS from the tail end and re-link. Rebalances via recolor/rot afterward.

```c
void sub_27720(a1x, a2):                                   // EF:17938  (a2 = desired length)
    a2 |= 1                                                 // force ODD length (low byte OR 1)  (EF:17952)
    v11 = 0                                                 // "spawn failed" flag
    if a2 >= 1 && a2 <= 15 && a1x->byte_0x46_70 != a2:      // in range and actually changing
        // walk to the TAIL end; v3 ends = last segment's word_0x34_52 (== 0):
        for ix = a1x; ; ix = Entities[ix->word_0x34_52]:
            v3 = ix->word_0x34_52
            if !v3: break                                   // ix = last live segment, v3 = 0

        if a1x->byte_0x46_70 >= a2:                          // ---- SHRINK ----
            while v3 < (a1x->byte_0x46_70 - a2) / 2:         // remove ((cur-target)/2) ring PAIRS
                v6x = Entities[ix->word_0x32_50]             // one link up (the -offset twin)
                v7x = v6x
                ix  = Entities[v6x->word_0x32_50]            // two links up (new tail anchor)
                ix->word_0x34_52 = 0                         // sever chain there
                v8x = Entities[v7x->word_0x34_52]            // the +offset twin
                DisableEntityDrawing04_57F10(v7x)            // hide -offset seg
                v3++
                DisableEntityDrawing04_57F10(v8x)            // hide +offset seg
        else:                                                // ---- GROW ----
            v4x = NewEvent_4A050()                            // spawn +offset seg
            if v4x:
                v9x = NewEvent_4A050()                        // spawn -offset seg
                if v9x:
                    sub_274C0(a1x, v4x, ix, abs(ix->byte_0x46_70) + 1)     // + segment, offset = |lastOff|+1
                    v5 = -(abs(ix->byte_0x46_70) + 1)
                    ix = v9x
                    sub_274C0(a1x, v9x, v4x, v5)                            // - segment, offset = -(|lastOff|+1)
                else:
                    DisableEntityDrawing04_57F10(v4x)          // rollback partial spawn
                    v11 = 1
            else:
                v11 = 1

        if !v11:                                              // commit
            a1x->byte_0x46_70 = a2                             // record new length
            sub_27590(a1x)                                     // recolor whole chain
            sub_27610(a1x)                                     // reset per-seg shift-rot from particle table
```

Details:
- Grows/shrinks in PAIRS of segments (one `+off`, one `-off`), consistent with `sub_4CB60`'s ring topology.
- GROW appends at the tail: new `+` seg copies `ix` (last seg) then `sub_274C0` re-links `ix → v4x`, offset `|ix.byte_0x46_70|+1`; new `-` seg links `v4x → v9x`, offset `−(|ix.byte_0x46_70|+1)`. So each grow adds ONE ring (+n,−n).
- SHRINK walks `word_0x32_50` (child→parent) twice per removed pair, severs `word_0x34_52` at the new tail, and `DisableEntityDrawing04`'s both twins (does NOT free the slots — just hides; `NewEvent` reclaims later).
- **The address 0x27920 ("0xB6 handler" in the old roster) is INSIDE this function** — it is the GROW-branch `sub_274C0` calls, not a standalone state handler. State 0xB6 (182) has no unique body.
- On any commit, `sub_27590` (colorize) + `sub_27610` (shift-rot) re-run across the whole chain.

RNG: none. Sounds: none.

---

## 6. `sub_27120` (EF:17655) — anti-stack z-push vs other model-22

Every 16th head frame (gated in `sub_26FF0`), pushes this worm's head UP if it is stacked on another worm's body.

```c
void sub_27120(a1x):                                       // EF:17655
    v1 = 2 * a1x->array_0x52_82.fov + 32                    // vertical proximity window
    v2 = 2 * a1x->array_0x52_82.pitch                       // horizontal proximity window
    v3x = bytearray_38403x[a1x->model_0x40_64]              // head of the per-model-22 bucket list
    if v3x <= Entities_EA3E4[0]: return
    do:
        if v3x->id_0x1A_26 != a1x->id_0x1A_26               // different worm instance
           && abs(a1x.pos.x - v3x.pos.x) < v2
           && abs(a1x.pos.y - v3x.pos.y) < v2
           && abs(a1x.pos.z - v3x.pos.z) < v1:              // overlapping in all 3 axes
            v5 = a1x->position_0x4C_76.z
            if v5 >= v3x->position_0x4C_76.z:               // if we're at/above the other
                a1x->position_0x4C_76.z = v5 + 64           // hop up 64 to declump
        v3x = v3x->next_0
    while v3x > Entities_EA3E4[0]
```

- Iterates the model-22 bucket (`bytearray_38403x[22]`) — which by the list-rebuild rules contains only the
  HEADS (state 0xB0-0xB3); tail segments in state 0xB4 are excluded from the bucket (see multipart-chains §7).
- `id_0x1A_26` distinguishes worm instances; a worm never pushes against its own segments.
- Only pushes UP (+64) and only when this head is the upper/equal one — so two stacked worms deterministically
  separate (lower one stays, upper one climbs).

RNG: none. Sounds: none.

---

## 7. `sub_273C0` (EF:17780) — spiral angle for `sub_271D0`

Pure function computing the yaw a segment orbits the head at. Args (from `sub_271D0` EF:17699):
`a1 = head->animationFrame_0x5C_92`, `a2 = head->word_0x36_54` (anim phase byte),
`a3 = seg->byte_0x46_70` (SIGNED segment ring offset), `a4 = head->byte_0x46_70` (head tail length).

```c
int sub_273C0(int16 a1, char a2, int16 a3, int16 a4):      // EF:17780
    v4 = abs(a3)                                            // |ring offset|
    result = ((15 - a4) * v4 + v4 * a1) & 0x7FF             // base spiral = v4*(15 - tailLen + animFrame), 11-bit
    if a3 >= 0:                                             // + side (right of spine)
        if a2 & 2: return result                            //   phase bit1 set → as-is
        v6 = 2048 - result                                  //   else mirror
    else:                                                   // - side (left of spine)
        if a2 & 2: v6 = 1024 - result                       //   phase bit1 → 90°-mirror
        else:      v6 = result + 1024                       //   else + 180°
    return v6 & 0x7FF
```

So the ring offset `a3` sign selects which half-plane the segment spirals in, `head->word_0x36_54 bit1`
(toggled every anim floor-bounce in `sub_272C0`) flips the spiral chirality, and the magnitude grows with
`|offset|` and the head animation frame. The `(15 - tailLen)` term tightens the coil as the tail grows.

RNG: none.

---

## 8. Colorize / shift-rot / init chain walkers

### 8a. `sub_278F0` (EF:18036) — color/particle index lookup (leaf)

```c
int sub_278F0(int a1, int16 a2, int16 a3):                 // a1=baseColorIdx, a2=headTailLen, a3=segOffset
    v3 = abs(a3)                                            // |segment ring offset|
    v3 = (uint8) x_BYTE_D400C[a2 >> 1][v3]                  // 8x8 triangular offset table (EF:18041)
    return a1 + v3
```

`x_BYTE_D400C[8][8]` (EF:1080) is a lower-triangular ramp indexed `[tailLen>>1][|offset|]`:
```
row0: 0 0 0 0 0 0 0 0
row1: 1 0 0 0 0 0 0 0
row2: 2 1 0 0 0 0 0 0
row3: 3 2 1 0 0 0 0 0
row4: 4 3 2 1 0 0 0 0
row5: 5 4 3 3 1 0 0 0     ← note [5][3]=3 (not 2) and [5][2]=3
row6: 6 5 4 3 2 1 0 0
row7: 7 6 5 4 3 2 1 0
```
So the returned particle index = `baseColorIdx + D400C[tailLen>>1][|offset|]` — segments nearer the head
(larger |offset| for a given length) get lower deltas; the head itself (offset 0) always gets `baseColorIdx`.
The base color index is `GetManaSphereColorIndexFromEntityId_369F0(head->playerEntityIndex_0x94_148)` — i.e.
the worm recolors to the OWNER's mana-sphere palette when it acquires a player target.

### 8b. `sub_27590` (EF:17867) — colorize the whole chain

```c
void sub_27590(a2x):                                       // a2x = head
    v2 = GetManaSphereColorIndexFromEntityId_369F0(a2x->playerEntityIndex_0x94_148)  // owner color
    v3 = v2
    v4 = sub_278F0(v2, a2x->byte_0x46_70, 0)               // head's own index (offset 0)
    sub_49D50(a2x, v4)                                      // apply to head
    for each seg in a2x->word_0x34_52 chain:
        v6 = sub_278F0(v3, a2x->byte_0x46_70, seg->byte_0x46_70)
        sub_49D50(seg, v6)                                  // apply per-segment index
```

### 8c. `sub_49D50` (EF:32847) — what the color index actually does

```c
void sub_49D50(event, entityIndex):                        // EF:32847
    event->word_0x5A_90 = entityIndex                       // sprite/particle row index
    event->byte_0x5D_93 = x_BYTE_D8A2E[particlesParameters_D951C[entityIndex].byte_12]   // palette/shade byte
    event->array_0x52_82.yaw = particlesParameters_D951C[entityIndex].rotSpeed_8 / 2      // spin from particle row
```
So the "color index" is really a `particlesParameters_D951C[]` ROW selector: it sets the draw sprite row
(`word_0x5A_90`), a palette-shade byte via the `D8A2E` LUT, and the yaw spin to half the row's `rotSpeed_8`.

### 8d. `sub_27610` (EF:17893) — per-segment shift-rot from the particle table

```c
void sub_27610(a2x):                                       // a2x = head
    v2 = GetManaSphereColorIndexFromEntityId_369F0(a2x->playerEntityIndex_0x94_148)
    v3 = 550 * particlesParameters_D951C[ sub_278F0(v2, a2x->byte_0x46_70, 0) ].rotSpeed_8
    SetEntityShiftRot_49EA0(a2x, v3/1000, v3/1000)          // head: shift=fov=550*rotSpeed/1000
    for each seg in a2x->word_0x34_52 chain:
        v5 = 550 * particlesParameters_D951C[ sub_278F0(v2, a2x->byte_0x46_70, seg->byte_0x46_70) ].rotSpeed_8
        SetEntityShiftRot_49EA0(seg, v5/1000, v5/1000)      // per-seg shift=fov=550*rotSpeed/1000
```
`SetEntityShiftRot_49EA0(e, shift, fov)` sets `array_0x52_82.pitch = shift` and `array_0x52_82.fov = fov`
(EF:32874+). The **550·rotSpeed_8/1000** formula sizes each link's spacing (`pitch`) and coil radius (`fov`)
from the same particle row that colored it — so nearer-head segments (lower D400C delta → lower row) get a
different spacing than tail segments, giving the tapered coil.

### 8e. `sub_276E0` (EF:17920) — position-init the whole chain

```c
void sub_276E0(a1x):                                       // a1x = head
    for each seg in a1x->word_0x34_52 chain:
        sub_271D0(seg)                                      // run the spiral-follow once to snap positions
```
Called once by `sub_4CB60` after spawning the tail, to seat every segment on its spiral before the first tick.

---

## 9. `sub_27470` (EF:17822) — segment-by-offset finder (used by state 0xB1)

```c
type_entity* sub_27470(a1x, a2):                           // a1x=head, a2=target signed offset
    resultx = a1x
    if a2:                                                  // a2==0 → return head itself
        while 1:
            resultx = Entities[resultx->word_0x34_52]       // walk head→tail
            if resultx == Entities[0]: break                // end of chain
            if resultx->byte_0x46_70 == a2: return resultx  // matching signed ring offset
        resultx = 0                                         // not found
    return resultx
```
Returns the tail segment whose signed ring offset `byte_0x46_70` equals `a2`, or the head if `a2==0`,
or NULL if no such segment. Used by 0xB1 to pick a segment to recolor as it "swallows" mana toward the head.

---

## 10. The castle-mana-drain chain: 0xB1 / 0xB2 / 0xB3

### 10a. `sub_26990` (EF:17255) — state 0xB1 (177): chase + colorize-inward scan

```c
void sub_26990(a1x):                                       // EF:17255
    v10 = 1                                                 // "found nothing" flag
    sub_26FF0(a1x)                                          // move+alt
    sub_272C0(a1x)                                          // anim/spin
    v8 = a1x->dword_0x10_16 >> 8                            // HI byte = fixed center offset
    v1 = a1x->dword_0x10_16 & 0xFF                          // LO byte = current sweep radius (0..)
    v2 = 0
    v9 = GetManaSphereColorIndexFromEntityId_369F0(a1x->playerEntityIndex_0x94_148)   // owner color base

    while v2 < ((v1 != 0) + 1):                             // 1 iter if v1==0, else 2 iters (± pair)
        if v2: v4 = -v1                                     // second pass = negative side
        else:  v4 = v1                                      // first pass = positive side
        v7 = v4 + v8                                        // signed target offset = center ± radius
        if abs((int16)(v4 + v8)) <= a1x->byte_0x46_70 / 2:  // within the tail's ring range
            resultx = sub_27470(a1x, v7)                     // find the segment at that offset
            if resultx:
                v5 = sub_278F0(v9, a1x->byte_0x46_70, v7)    // owner-color index for it
                sub_49D50(resultx, v5)                       // recolor that segment (mana "traveling in")
                v10 = 0                                       // found at least one

    if v10:                                                 // swept past the whole tail → nothing left
        if a1x->playerEntityIndex_0x94_148:
            a1x->actionIndex_0x45_69 = 178 (0xB2)            // → castle-acquire (has a target)
        else:
            a1x->actionIndex_0x45_69 = 176 (0xB0)            // → idle (no target)
    else:
        a1x->dword_0x10_16 = (int16)(v1 + 1) | ((int16)v8 << 8)   // advance sweep radius, keep center
```

**`dword_0x10_16` packing (verbatim):** HI 8 bits = `v8` = a fixed CENTER ring offset (set once, e.g. by
`sub_26D20`: `head->dword_0x10_16 = seg.byte_0x46_70 << 8`). LO byte = `v1` = the current sweep radius,
incremented each tick. Each tick recolors the segment(s) at offset `center ± radius` to the owner palette
(a visual "mana crawling inward along the worm"); when the radius exceeds the tail's `byte_0x46_70/2` on both
sides (nothing found → `v10` stays 1), the worm advances to 0xB2 (if it has a player target) or 0xB0.

### 10b. `sub_26AA0` (EF:17313) — state 0xB2 (178): castle acquire (arm the drain)

```c
void sub_26AA0(a1x):                                       // EF:17313
    v7 = 0
    sub_26FF0(a1x); sub_272C0(a1x); sub_26F10(a1x); sub_27880(a1x)   // move/anim/damage/grow
    if !(a1x->byte_0x3E_62 & 0x1F):                         // every 32nd frame (per-instance phase)
        v1 = a1x->playerEntityIndex_0x94_148                // the targeted PLAYER entity index
        if !v1: goto LABEL_17 (v7=1)                        // no target → revert
        if a1x->actSpeed_0x82_130 > a1x->maxSpeed_0x86_134: // still accelerated (recently hit) → hold, skip
            goto LABEL_13
        v2x = Entities[v1]                                  // the player entity
        if v2x->class_0x3F_63 != 3:      goto LABEL_17      // not a player → revert
        if v2x->life_0x8 < 0:            goto LABEL_17      // player dead → revert
        if v2x->struct_byte_0xc[1] & 4:  goto LABEL_17      // player flagged (out) → revert
        v3 = v2x->dword_0xA4_164x->CastleEntityIndex_0x3A_58   // the player's CASTLE entity index
        if !v3:                          goto LABEL_17      // player has no castle → revert
        v4x = Entities[v3]                                  // the castle entity
        v5 = tan2(&self.pos, &castle.pos)
        a1x->roll_0x20_32 = v5                              // bank toward the castle
        if (byte_0x3E_62 & 3) || EuclideanDistXYZ_58490(self.pos, castle.pos) > 0x100:
            goto LABEL_13                                    // not aligned frame OR too far (>256) → hold
        if a1x->mana_0x90_144 + v4x->mana_0x90_144 < v4x->maxMana_0x8C_140:   // castle has room
            a1x->dword_0x10_16 = 128                        // drain countdown = 128
            a1x->actionIndex_0x45_69 = 179 (0xB3)           // → DEPOSIT state
        else:
            LABEL_17: v7 = 1                                // castle full → revert
    LABEL_13:
        if v7:
            a1x->dword_0x10_16 = 0
            a1x->playerEntityIndex_0x94_148 = 0             // drop target
            a1x->actionIndex_0x45_69 = 177 (0xB1)           // → back to chase/sweep
```

**"targets a player CASTLE" resolved verbatim:** the worm's `playerEntityIndex_0x94_148` names a PLAYER entity
(`class==3`). It reaches through that player's `dword_0xA4_164x` (the player's control/state block) to
`CastleEntityIndex_0x3A_58` — the entity index of THAT player's castle. The distance test `0x100 (256)` is a
3D Euclidean distance (`EuclideanDistXYZ_58490`) between the WORM head and that CASTLE. Only when the worm is
within 256 units of the castle, on an aligned frame (`byte_0x3E_62 & 3 == 0`), and the castle has spare mana
capacity, does it arm the deposit (state 0xB3) with a 128-tick countdown. The whole check is gated to every
32nd frame (`byte_0x3E_62 & 0x1F`). Any failing condition reverts to 0xB1 (dropping the target) except the
"still accelerated" and "not-aligned/too-far" cases which merely hold in 0xB2.

### 10c. `sub_26BD0` (EF:17373) — state 0xB3 (179): deposit / shrink

```c
void sub_26BD0(a1x):                                       // EF:17373
    sub_272C0(a1x)                                          // anim/spin ONLY (no move — it's parked at the castle)
    v1 = a1x->dword_0x10_16
    if v1:
        a1x->dword_0x10_16 = v1 - 1                        // count the 128-tick timer down
    else if !(a1x->byte_0x3E_62 & 1):                       // on expiry, only on even-phase instances
        v2 = a1x->byte_0x46_70                              // tail length
        if v2 > 1:
            sub_27720(a1x, v2 - 2)                          // SHRINK tail by 2 (worm consumes its own mass)
        else:                                               // tail exhausted → final deposit + self-destruct
            v3x = Entities[a1x->playerEntityIndex_0x94_148]  // the player
            if v3x->class==3 && v3x->life >= 0 && !(v3x->struct_byte_0xc[1] & 4):
                v4 = v3x->dword_0xA4_164x->CastleEntityIndex_0x3A_58
                if v4:
                    v5x = Entities[v4]                        // castle
                    v6 = v5x->maxMana_0x8C_140
                    v7 = a1x->mana_0x90_144 + v5x->mana_0x90_144
                    if v7 >= v6: v5x->mana_0x90_144 = v6      // deposit worm mana, cap at castle maxMana
                    else:        v5x->mana_0x90_144 = v7
            DisableEntityDrawing04_57F10(a1x)                // worm HEAD vanishes (self-destruct)
```

The deposit is a REPEATED shrink: each 128-tick cycle (only on `byte_0x3E_62 & 1 == 0` "even" instances) the
worm shrinks its tail by 2 via `sub_27720(len-2)`. When the tail is down to length 1, it dumps its accumulated
`mana_0x90_144` into the target castle (`castle.mana += worm.mana`, capped at `castle.maxMana`) and
`DisableEntityDrawing04`'s the head — the worm is consumed delivering the stolen mana.

Note 0xB3 does NOT call `sub_26FF0`/`sub_26F10`/`sub_27880` — no movement, no damage intake, no growth while
depositing. Only `sub_272C0` (writhe) runs. The tail segments (still in 0xB4) keep spiral-following the parked head.

---

## 11. `sub_265A0` (EF:17011) — belongs to MODEL 21, NOT model 22

**Conflict resolved.** The multipart-chains trace's §8 sound table cites "42 | EF:17075 (m22 `sub_265A0` case9)".
That attribution is WRONG. `sub_265A0` is the **model-21 hover-fly physics core** and m22 never calls it.

Evidence:
- `sub_265A0` is called ONLY from `sub_26070` (EF:16780), `sub_26220` (EF:16896), and `sub_26470` (EF:16952).
- Those three handlers key off states in the 0xA0-0xAF band and use `sub_268F0` (`actionIndex = a2 - 88`),
  i.e. base 160/168. The model-21 CTOR `sub_4C8F0` (EF:34340) sets `actionIndex = 169 (0xA9)`, `model = 21`,
  `dword_0xA0_160x = &str_D7BD6[96]`, and calls `sub_26500` + `sub_268F0(v1x, 1u)` at spawn — wiring it into
  exactly this state family. (The model-20 CTOR `sub_4C7F0` at EF:34307 sets `actionIndex = -95 (0xA1)`,
  `str_D7BD6[89]`, and shares the same `sub_265A0`/`sub_26500` helpers via the 0xA0 band.)
- `sub_265A0` runs a `byte_0x46_70` 0..0xA bob/dive state machine (rise to `word_0x2C_44`, dive to
  `terrainAlt+230`, water-surface transition to sub-state 10 spawning a class-10 subtype-5 mana sphere at
  EF:17131, actSpeed set to 40/60/66/96 by water+actionIndex), and `sub_26500` maps `byte_0x46_70` → sprite
  rows 305-312. This is hovering-bird / floater physics, incompatible with the m22 worm state set (0xB0-0xB7).
- `sub_26FF0` is m22's terrain-follow core (§1), a DIFFERENT function from `sub_265A0`. The similar naming
  (`sub_265A0` vs `sub_26FF0`) is likely what produced the mis-cite.

The SOUND 42 at EF:17075 (`case 9: if (rand%0xB == 0) PrepareEventSound(...,42)`) therefore belongs to
model 20/21, NOT model 22. m22's only sounds are **48** (thrash, `sub_272C0`) and **4** (retarget, `sub_26D20`
EF:17515/17524 and `sub_26F10` EF:17577).

RNG note (model 20/21 `sub_265A0`, for completeness): case 4 draws once (`word_0x2C_44 = rand%100 + 140`);
case 9 draws once for the sound roll, then if `byte_0x43_67` draws AGAIN for `byte_0x44_68 = rand % byte_0x43_67`.

---

## 12. m22 damage intake — how segment hits reach the head, and head death

### 12a. Segment tick 0xB4 (`sub_26CA0` = `sub_271D0` + `sub_26D20`)

Verified against the existing `sub_26D20` trace (multipart-chains §4). Confirmed behavior:
- `sub_271D0` (spiral follow) does **not** touch life or the damage mailbox.
- `sub_26D20` (EF:17447) reads the SEGMENT's `str_0x5E_94` mailbox and RELAYS to the head:
  - Gate: only if `seg->byte_0x39_57` AND head `actionIndex >= 0xB0 && (<= 0xB0 || == 178)` (i.e. head in
    0xB0 or 0xB2 — the ambush/acquire states).
  - On a pending hit (`seg->str_0x5E_94.word_0x62_98`): boosts HEAD `actSpeed = ((minSpeed-maxSpeed)>>2)+maxSpeed`,
    turns head yaw/roll toward the attacker, steers head `word_0x2C_44` by `±56*|seg.byte_0x46_70|/(head.byte_0x46_70>>1)`
    clamped to `[11,227]`, then CLEARS the `word_0x62_98` mailbox on EVERY segment in the chain.
  - On a target-tag (`seg->str_0x5E_94.word_0x68_104` ≠ head's `playerEntityIndex`): sets head `actionIndex=177`,
    `head->dword_0x10_16 = seg.byte_0x46_70 << 8` (the sweep CENTER for 0xB1), head target = attacker,
    SOUND **4** (EF:17515 or 17524, split on `dword_0x64_100` and flag bit 0x20), sets `struct_byte_0xc[2] |= 0x20`,
    then clears `word_0x68_104` on every segment.

**Neither `sub_26D20` nor `sub_271D0` subtracts life** — not from the segment, not from the head. The relay
consumes the damage mailbox purely for AGGRO/STEER, discarding the damage amount `dword_0x5E_94`.

### 12b. Where head life actually drops — the OPEN question

- The head handlers 0xB0-0xB3 call `sub_26F10` (0xB0/0xB2 only), which reads the head's OWN mailbox but (as
  shown in §3) uses it only to accelerate/retarget and NEVER does `life -= dword_0x5E_94`.
- No m22 state calls `sub_26830` (the EF:17154 aggregator that would set `head.life = min(head.life, min segment life)`
  and subtract the head mailbox). `sub_26830` is used by the model-20/21 family (`sub_26070`/`sub_26220`), not m22.
- Therefore, **through the traced handlers, the m22 head life never decreases.** Head `life_0x8 < 0` (→ state 181
  chain-kill, EF:17583) can only be produced by a damage path OUTSIDE this suite that writes `head.life_0x8`
  or `head.str_0x5E_94.dword_0x5E_94` in a way a NON-traced routine subtracts.

Two candidate external paths (NOT confirmed in this pass — flagged OPEN):
1. A projectile/spell hit that targets the head's chain and writes directly into `head.life_0x8` (some spells
   bypass the mailbox and decrement life on impact).
2. The damage-writer at EF:4023-4386 that stamps `str_0x5E_94.dword_0x5E_94 += a3` on entities it hits —
   if it can hit the HEAD (id match) and a *different* per-frame pass subtracts head mailboxes for class-5.

The practical consequence for the PORT: **m22 as coded is effectively melee-immune** (segment hits only enrage
it), and dies only when some external effect drives `head.life_0x8` negative, at which point `sub_26F10`
flips it to 0xB5 and `sub_26CC0` converts the entire `word_0x34_52` chain (segments first, head last) into
mana spheres via `TransformEntityToManaSphere_36BA0(_, false)` + `DisableEntityDrawing04`.

---

## OPEN / uncertainties

1. **Head life-subtract path (§12b).** Not found within the m22 helper suite. The head has no traced code that
   lowers `life_0x8`; state 181 is reachable only via `life < 0`. The external writer (direct-life spell impact
   vs a global class-5 mailbox-drain pass) must be located before the port can make m22 killable. Until then a
   faithful port reproduces "melee only enrages; head immune" — which may itself be the authentic behavior
   (verify against retail: is the worm killable by fireball, or only by draining/expiring?).
2. **`sub_26D20` gate `v2 <= 0xB0u || v2 == 178`.** The relay only acts when head is in 0xB0 or 0xB2. In 0xB1/0xB3
   segment hits are ignored (mailboxes NOT cleared by the relay, but also not applied). Confirm this asymmetry
   is intended (segments accumulate stale hit flags while the head chases/deposits).
3. **`word_0x24_36` rise budget** in `sub_26FF0` — value 0x40 (64) refilled on each rise; consumed one-per-tick
   while clamped at ceiling. Interaction with the `+0x100/+0x40` rise is a hysteresis; validated by reading only,
   not by simulation.
4. **`sub_1B7A0_tile_compare` vs `word_160_0x10_16`** — the "steep vs gentle rise" branch compares a terrain-type
   code at the highest-alt cell against the row-90 baseline. The exact semantics of the tile code are external
   to this suite (shared terrain helper).
5. **`x_BYTE_D400C[5][2]=3, [5][3]=3`** (EF:1087) — the row-5 ramp is non-monotone (…4,3,3,1…). Reproduced verbatim;
   assumed intentional (a hand-tuned color ramp), not a decompiler artifact.
6. **`dword_0x10_16` HI byte origin.** In 0xB1 the sweep CENTER (`v8`) comes from `dword_0x10_16 >> 8`, which
   `sub_26D20` seeds as `seg.byte_0x46_70 << 8` when a segment relays a target-tag. If the head enters 0xB1 by
   another route (e.g. `sub_26F10` sets `dword_0x10_16 = 0` on retarget), the center is 0 and the sweep is
   symmetric about the spine. Confirmed both writers; no third writer found.
7. **Sound-42 re-attribution (§11).** High confidence `sub_265A0` is model 20/21, not 22, but the state→handler
   binary table (`off_D697E`, indexed `actionIndex*14`) is not readable source; the binding is inferred from the
   three call sites + the model-20/21 CTORs. m22's dispatch (EV:1847-1922) is directly verified from Events.cpp.
