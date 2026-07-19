# MC2 class-5 model-27 — the HYDRA (3-tier body/heads/segments; "tree/kraken" in older notes) — BRANCH/SEGMENT machinery — VERBATIM trace

Player-identified 2026-07-16: this is MC2's Hydra — 5 fireball-spitting
HEADS (the 0xE9 "branches") that retract and re-grow when "killed"
(the body gauge `byte_0x3B_59` decrements at case 8, re-increments at
case 0xA), and a body that is attackable ONLY while the gauge is 0
(all heads suspended — `sub_2A6B0` returns the exposed code 2).

Permanent port reference. Fills the OPEN gaps left by `docs/traces/mc2-multipart-chains.md`
(which already pins the m27 ctor `sub_4D000` and finalizers `sub_2AC50/2AD40/2AE30`).

Addressing: `sub_XXXXX = address − 0x1E1000`. File cites: `EF:line` = `reference/remc2/remc2/engine/EventsFunctions.cpp`, `EV:line` = `.../engine/Events.cpp`. Fields named verbatim from the decomp (e.g. `byte_0x46_70`, `word_0x36_54`).

**RNG law (every advance MUST be byte-faithful in the port):** `ix->rand_0x14_20 = 9377 * ix->rand_0x14_20 + 9439;` — a *per-entity* LCG stored in `rand_0x14_20`. Note the branch entity `ix` advances ITS OWN rng, and helper `sub_2A340` / `sub_2A7F0` advance the rng of *their* argument (the branch `ix`, or in `sub_2A7F0` the branch `a1x`). Draw order below is exact.

---

## 0. Topology recap (from ctor `sub_4D000`, EF:34591)

A live m27 is ONE `word_0x34_52` linear list, 51 entities:

```
BODY(0xD9) → branch0(0xE9) → seg0..seg8(0xEA) → branch1(0xE9) → seg0..8 → … → branch4 → seg0..8
```

- BODY `actionIndex_0x45_69 = 0xD9` (217) — the only ticking entity.
- BRANCH `actionIndex = 0xE9` (233), `byte_0x3B_59 = branchIdx (0..4)`.
- SEGMENT `actionIndex = 0xEA` (234), `byte_0x3B_59 = branchIdx` (inherited).
- Every branch and segment: `word_0x32_50 = bodyIdx`, `id_0x1A_26 = bodyIdx`.
- 0xE9 and 0xEA have **NO independent tick dispatch** — the body drives them all via `sub_29A90`.

The body brain (`sub_29670` state 0xD9, EF:19556; `sub_29710` 0xDA; `sub_29400` 0xD8; `sub_29930` 0xDF) calls `sub_29A90(body)` every tick (EF:19567/19580/19587, and the fall-through at end of `sub_29670`). `sub_29A90` walks the `word_0x34_52` chain and processes **only entities with `actionIndex_0x45_69 == 233`** (the 5 branches). For each branch it (a) runs a 16-way state machine on `byte_0x46_70`, and (b) positions that branch's 9 trailing segments via `sub_2AA90`.

`byte_0x3B_59` overloading on the BODY: it is initialised to `5` by `sub_2AC50` (EF:20746) and is used by the machine as a **live branch-count / activation gauge** — case 8 does `a1x->byte_0x3B_59--` (EF:20098, `a1x` = body) and case 0xA does `a1x->byte_0x3B_59++` (EF:20108). On a branch entity `byte_0x3B_59` is the fixed branch index (0..4) used to index `str_D404C`.

---

## 1. `sub_29A90(body)` — the branch state machine (EF:19737) — CORE DELIVERABLE

Signature: `void sub_29A90(type_entity_0x6E8E* a1x)` where `a1x` = the BODY.

### 1.1 Walk + per-branch pre-roll (EF:19790–19838)

```c
for (ix = Entities_EA3E4[a1x->word_0x34_52];       // first chain member
     ix > Entities_EA3E4[0];
     ix = Entities_EA3E4[ix->word_0x34_52])        // walk word_0x34_52
{
    if (ix->actionIndex_0x45_69 != 233) continue;  // ONLY branches (skip body-none, segs 0xEA)

    v37 = 0;                                        // "spawn projectile this tick" flag (0/1/2)
    v4yy = &str_D404C[ix->byte_0x3B_59];            // branch's spline row (table §4)
    v5   = ix->byte_0x46_70;                        // current sub-state
    ix->byte_0x3E_62++;                             // per-branch phase counter (drives &7 / &1F gates)

    if (v5 <= 5u)                                    // states 0..5 get the "whip windup" pre-roll
    {
        // RNG DRAW #A  (branch ix's own lcg)
        ix->rand_0x14_20 = 9377 * ix->rand_0x14_20 + 9439;
        ix->byte_0x43_67 = ix->rand_0x14_20 % 0x14u;      // 0..19  (whip counter seed)
        sub_2A5B0(a1x, ix, 672);                           // reposition branch head (§3.1)
        sub_2A660(a1x, ix);                                // segment→body melee relay (§3.5)

        if (ix->byte_0x46_70 == 1)
        {
            // RNG DRAW #B
            ix->rand_0x14_20 = 9377 * ix->rand_0x14_20 + 9439;
            v6  = ix->rand_0x14_20 & 7;                    // 0..7
            v7  = ix->word_0x26_38;                        // attacker id (set by sub_2A660 on hit)
            v34x = (ix->rand_0x14_20 & 7);                 // saved for state-1 body of switch below
            if (v7)                                        // was hit → retaliate
            {
                if (v6 < 4)
                {
                    ix->word_0x26_38 = 0;
                    ix->byte_0x46_70 = 2;                  // → jump to "aim whip" chain
                    ix->word_0x96_150 = v7;                // target = attacker
                    ix->word_0x36_54 += 22;                // lengthen link
                    if (ix->word_0x36_54 > 0x44u)          // cap 68
                        ix->word_0x36_54 = 68;
                }
            }
            else if (v6 < 4 && a1x->word_0x96_150 && !(ix->byte_0x3E_62 & 7))
            {                                              // body has a target, every 8th phase
                ix->byte_0x46_70 = 2;
                ix->word_0x96_150 = a1x->word_0x96_150;    // inherit body's target
            }
        }
    }
```

`v34x` is initialised at function entry to `0x1000002b` (EF:19783 — a leftover from the `a1x` pointer, effectively "large nonzero" until overwritten by DRAW #B). It only matters inside `case 1`.

### 1.2 The 16-way switch on `ix->byte_0x46_70` (EF:19839–20185)

Each case verbatim. `v4yy = &str_D404C[branchIdx]`. Many cases fall-through via `goto LABEL_xx`.

**case 0 (idle→arm):** EF:19841
```c
ix->word_0x96_150 = 0;
ix->byte_0x46_70  = 1;
ix->word_0x2C_44  = 0;
ix->word_0x36_54  = 0;
ix->minSpeed_0x84_132 = 16;
goto LABEL_15;               // fall into case 1
```

**case 1 (scan / aim):** LABEL_15, EF:19848
```c
if (a1x->byte_0x39_57) {                         // body "active frames" flag
    if (!(ix->byte_0x3E_62 & 7)) {               // every 8th phase
        if (v34x) {                              // v34x = last DRAW #B & 7 (or the 0x..2b seed)
            if (v34x > 4) ix->byte_0x46_70 = 4;  // → "back-swing whip" branch
        } else {
            v10x = sub_2A6F0(ix);                // target scan (§3.4)
            v35x = v10x;
            if (v10x) {
                ix->byte_0x46_70   = 2;          // → aim
                ix->word_0x96_150  = v10x - D41A0_0.struct_0x6E8E;   // target id
            }
        }
    }
    if (!(ix->byte_0x3E_62 & 7) && !(v34x & 1)) {          // every 8th phase, v34x even
        // RNG DRAW #C
        ix->rand_0x14_20 = 9377 * ix->rand_0x14_20 + 9439;
        v4xx = a1x->yaw_0x1C_28 + v4yy->word_12 - ix->dword_0xA0_160x->word_160_0x1e_30;
        ix->yaw_0x1C_28 = v4xx + ix->rand_0x14_20 % ix->dword_0xA0_160x->word_160_0x1e_30;  // wander yaw
    }
}
break;
```

**case 2 (begin forward whip):** EF:19882
```c
ix->byte_0x46_70 = 3;
ix->byte_0x44_68 = 0;         // sub-phase
ix->word_0x2C_44 = 2;         // sub_2A340 mode 2 (accel toward 0)
ix->minSpeed_0x84_132 = 16;
goto LABEL_26;                // fall into case 3
```

**case 3 (forward whip strike, target-tracked):** LABEL_26, EF:19888
```c
v13x = sub_2A7B0(ix);         // validate target word_0x96_150 (§3.6) → target ptr or 0
v35x = v13x;
if (v13x) {
    if (ix->byte_0x44_68 < 1u) {
        if (!ix->byte_0x44_68 && !ix->actSpeed_0x82_130) {   // sub-phase 0, stalled
            ix->byte_0x44_68 = 1;
            ix->word_0x2C_44 = 1;                            // sub_2A340 mode 1 (accel to +192)
            ix->minSpeed_0x84_132 = 16;
            ix->yaw_0x1C_28   = Maths::sub_581E0_maybe_tan2(&ix->pos, &v13x->pos);   // face target
            ix->pitch_0x1E_30 = Maths::sub_58210_radix_tan(&ix->pos, &v13x->pos);
            ix->roll_0x20_32  = ix->yaw_0x1C_28   - a1x->yaw_0x1C_28;   // store yaw offset vs body
            ix->fov_0x22_34   = ix->pitch_0x1E_30 - a1x->pitch_0x1E_30; // store pitch offset vs body
        }
    } else if (ix->byte_0x44_68 <= 1u) {
        if (ix->actSpeed_0x82_130 == 192) {                  // whip reached full extension
            ix->word_0x2C_44 = 3;                            // sub_2A340 mode 3 (impact ramp)
            ix->byte_0x44_68 = 3;
            ix->dword_0x10_16 = 4;                           // impact countdown
            v37 = 1;                                         // ← SPAWN PROJECTILE, low (manaRegen path a3=1)
        }
    } else if (ix->byte_0x44_68 == 3) {
        v37 = 2;                                             // ← SPAWN PROJECTILE, high
        v14 = ix->dword_0x10_16 - 1;
        ix->dword_0x10_16 = v14;
        if (!v14) { ix->byte_0x46_70 = 0; ix->dword_0x10_16 = 1; }   // done → idle
    }
} else {
    ix->byte_0x46_70 = 0;      // target lost → idle
}
break;
```

**case 4 (begin back-swing whip):** EF:19937
```c
ix->byte_0x46_70 = 5;
ix->byte_0x44_68 = 0;
ix->word_0x2C_44 = 2;
ix->minSpeed_0x84_132 = 16;
goto LABEL_40;                // fall into case 5
```

**case 5 (back-swing whip, no target — inner switch on `byte_0x44_68`):** LABEL_40, EF:19943
```c
switch (ix->byte_0x44_68) {
case 0:
    if (!ix->actSpeed_0x82_130) {                 // stalled → launch back-swing
        ix->byte_0x44_68 = 1;
        ix->word_0x2C_44 = 1;
        ix->minSpeed_0x84_132 = -16;              // negative → swing back
        ix->roll_0x20_32 = v4yy->word_12;
        ix->fov_0x22_34  = v4yy->word_14;
        ix->yaw_0x1C_28   = ix->roll_0x20_32 + a1x->yaw_0x1C_28;
        ix->pitch_0x1E_30 = ix->fov_0x22_34  + a1x->pitch_0x1E_30;
    }
    break;
case 1:
    if (ix->actSpeed_0x82_130 == -192) {          // reached full back extension
        ix->byte_0x44_68  = 2;
        ix->dword_0x10_16 = 2;                     // countdown
    }
    break;
case 2:
    if (!--ix->dword_0x10_16) {
        ix->word_0x2C_44  = 4;                     // mode 4 (impact ramp back)
        ix->byte_0x44_68  = 6;
        ix->dword_0x10_16 = 1;
    }
    break;
case 5:
    if (--ix->dword_0x10_16 <= 4) {                // recovery finished
        ix->byte_0x46_70  = 0;                     // → idle
        ix->dword_0x10_16 = 4;
    }
    break;
case 6:
    PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 17);   // ← SOUND 17 (whip crack)  EF:19987
    if (++ix->dword_0x10_16 >= 4)
        ix->byte_0x44_68 = 5;
    break;
default:
    goto LABEL_94;
}
break;
```

**case 6 (retract → coil to segments):** EF:19997
```c
ix->dword_0x10_16 = 0;
ix->word_0x2C_44  = 2;
ix->byte_0x46_70  = 7;
ix->byte_0x44_68  = 0;
ix->minSpeed_0x84_132 = 80;
goto LABEL_52;                // fall into case 7
```

**case 7 (segment-extend animation — inner switch on `byte_0x44_68`):** LABEL_52, EF:20004
```c
v36 = 0;                                          // "apply yaw kick" flag
sub_2A5B0(a1x, ix, 672);                          // reposition branch head (§3.1)
switch ((uint8)ix->byte_0x44_68) {
case 0:
    if (!ix->actSpeed_0x82_130) {
        ix->byte_0x44_68 = 1;
        ix->word_0x2C_44 = 1;
        ix->roll_0x20_32 = v4yy->word_12;
        ix->fov_0x22_34  = v4yy->word_14;
    }
    break;
case 1:
    v36 = 1;
    if (ix->actSpeed_0x82_130 == 192) {
        ix->byte_0x44_68  = 7;
        ix->word_0x2C_44  = 5;                     // mode 5 (no sub_2A340 handler → noop)
        ix->dword_0x10_16 = 8;
    }
    break;
case 7:
    v36 = 1;
    if (!--ix->dword_0x10_16) {
        ix->byte_0x44_68  = 8;
        ix->word_0x2C_44  = 6;                     // mode 6 (z-descend, sub_2A340)
        ix->fov_0x22_34   = 0;
        ix->byte_0x43_67  = 0;
        ix->word_0x36_54  = 0;
        ix->minSpeed_0x84_132 = 12;
        ix->dword_0x10_16 = 0;
    }
    break;
case 8:                                            // walk the 9 segments, lift their draw bit progressively
    v18 = ix->dword_0x10_16;
    if (v18 > 10) {
        ix->byte_0x46_70 = 8;                      // → case 8 (drop / detach)
    } else {
        if (v18) {
            v19x = Entities_EA3E4[ix->word_0x34_52];       // first segment
            for (j = 0; ; j++) {
                v35x = v19x;
                if (j >= 9 - ix->dword_0x10_16) break;     // reach seg[9-n]
                v19x = Entities_EA3E4[v19x->word_0x34_52];
            }
        } else {
            v35x = ix;
        }
        if (v35x > Entities_EA3E4[0])
            v35x->struct_byte_0xc_12_15.byte[0] = (v35x->struct_byte_0xc_12_15.byte[0] | 1) & 0xF7;  // set draw bit0, clear bit3
        v22 = ix->byte_0x43_67 + 1;
        ix->dword_0x10_16++;
        v17 = 28 * v22;                            // link-length step
        ix->byte_0x43_67 = v22;
        ix->word_0x36_54 += v17;
    }
    break;
default: break;
}
if (v36) {                                         // yaw kick alternating by parity
    v23 = (ix->byte_0x3E_62 & 1) ? -204 : 204;
    ix->yaw_0x1C_28 = a1x->yaw_0x1C_28 + v4yy->word_12 + v23;
}
sub_2A340(ix);                                     // apply speed/rot mode (§3.7)
sub_2A940(a1x, ix);                                // reposition (§3.2)
break;
```

**case 8 (detach — decrement live-branch gauge, wait):** EF:20095
```c
ix->byte_0x46_70   = 9;
ix->dword_0x10_16  = 100;
a1x->byte_0x3B_59--;          // BODY gauge --  (branch went dormant)
goto LABEL_83;                // fall into case 9
```

**case 9 (dormant countdown):** LABEL_83, EF:20100
```c
if (!--ix->dword_0x10_16)
    ix->byte_0x46_70 = 10;
break;
```

**case 0xA (re-attach — increment gauge, start regrow):** EF:20107
```c
a1x->byte_0x3B_59++;          // BODY gauge ++
ix->byte_0x46_70   = 11;
ix->word_0x2C_44   = 5;
ix->dword_0x10_16  = 7;
v26 = ix->struct_byte_0xc_12_15.byte[0] & 0xF6;   // clear bits 0,3
ix->word_0x96_150  = ix->word_0x34_52;            // target-ref = first segment
ix->struct_byte_0xc_12_15.byte[0] = v26;
ix->struct_byte_0xc_12_15.byte[0] = v26 | 8;      // set draw bit3
ix->roll_0x20_32   = v4yy->word_12;
ix->actSpeed_0x82_130 = 156;
ix->fov_0x22_34    = v4yy->word_14;
goto LABEL_86;                // fall into case 0xB
```

**case 0xB (regrow countdown):** LABEL_86, EF:20123
```c
if (--ix->dword_0x10_16 <= 0) {
    ix->byte_0x46_70  = 12;
    ix->dword_0x10_16 = 0;
}
break;
```

**case 0xC (SEGMENT SEQUENTIAL SPAWN + set BRANCH LIFE):** EF:20133
```c
if (ix->dword_0x10_16 < 9) {                       // 9 segments to re-enable
    v1x = ix->word_0x34_52;
    for (k = 0; ; k++) {                            // walk to seg[dword_0x10_16]
        v4x = Entities_EA3E4[v1x];
        if (k >= ix->dword_0x10_16) break;
        v1x = (uint16)v4x->word_0x34_52;
    }
    v4x->struct_byte_0xc_12_15.byte[0] &= 0xFEu;    // clear draw bit0 of that segment
    v31 = ix->dword_0x10_16 + 1;
    ix->word_0x96_150 = v4x->word_0x34_52;          // next seg ref
    ix->dword_0x10_16 = v31;
    if (v31 >= 9) {                                 // all 9 done → LIFE ROLL
        // RNG DRAW #D  (branch ix's own lcg)
        ix->rand_0x14_20 = 9377 * ix->rand_0x14_20 + 9439;
        v32 = ix->rand_0x14_20 % 0x398u;            // 0..919
        ix->str_0x5E_94.word_0x62_98 = 0;           // clear pending hit
        ix->byte_0x46_70 = 0;                       // → idle (fully regrown)
        ix->life_0x8     = v32 + 920;               // BRANCH life = rand%0x398 + 920  (920..1839)
    }
}
break;
```

**VERIFIED (correction to prior trace §6):** the `life = rand%0x398 + 920` write is on **`ix`** = the BRANCH entity (EF:20155), reached only at the END of case-0xC regrow when all 9 segments are re-enabled (`v31 >= 9`). It is the branch's **respawn life** after a full detach(8/9/0xA)→regrow(0xB/0xC) cycle. It is NOT a segment life. This differs from the ctor's `sub_2AD40` life = `460*v2 + 920` (1380..3220, EF:20795) — the initial branch life; the regrow reset is the smaller 920..1839.

**case 0xD (begin die/whither):** EF:20159
```c
ix->byte_0x46_70  = 14;
ix->byte_0x43_67  = 10;
ix->dword_0x10_16 = 10;
goto LABEL_77;                // fall into case 0xE
```

**case 0xE (whither countdown):** LABEL_77, EF:20164
```c
if (!--ix->dword_0x10_16)
    ix->byte_0x46_70 = 15;
break;
```

**case 0xF (mass draw-bit set — 10 members):** EF:20171
```c
v35x = ix;
ix->byte_0x46_70 = 8;                              // → detach
v24 = 0;
do {
    v35x->struct_byte_0xc_12_15.byte[0] = (v35x->struct_byte_0xc_12_15.byte[0] | 1) & 0xF7;  // draw bit0 on, bit3 off
    v35x = Entities_EA3E4[v35x->word_0x34_52];
    v24++;
} while (v24 < 10);                                // branch + its 9 segments
break;
```

`default:` — nothing.

### 1.3 Post-state dispatch `LABEL_94` (EF:20186–20222)

After the first switch, a SECOND switch on the (possibly updated) `ix->byte_0x46_70` selects the positioning pipeline:

```c
LABEL_94:
switch (ix->byte_0x46_70) {
case 0: case 1: case 2: case 3: case 4: case 5:
    sub_2A340(ix);                                 // apply speed/rotation mode (§3.7)
    sub_2A940(a1x, ix);                            // reposition branch head via table (§3.2)
    sub_2AA90(a1x, ix);                            // position all 9 segments (§3.3 SPLINE)
    if (v37) {                                      // whip impact frames set v37=1 or 2
        sub_2A7F0(ix, v35x, v37 == 1);             // ← CLASS-9 PROJECTILE SPAWN (§3.8)  a3 = (v37==1)
        PrepareEventSound_6E450(a1x - D41A0_0.struct_0x6E8E, -1, 17);  // ← SOUND 17  EF:20202
    }
    goto LABEL_99;                                 // then sub_2A9F0

case 6: case 7:
    sub_2AA90(a1x, ix);                            // position segments only (no branch-head move)
    break;

case 0xB: case 0xC:
    sub_2A5B0(a1x, ix, 672);                        // reposition branch head (§3.1)
    sub_2A940(a1x, ix);
    sub_2AA90(a1x, ix);
    v35x = Entities_EA3E4[ix->word_0x96_150];       // the tracked segment
    predictedAxis_EB398ar = v35x->position_0x4C_76;
    CopyEntityPosition_57CF0(ix, &predictedAxis_EB398ar);   // branch head snaps onto that segment
LABEL_99:
    sub_2A9F0(a1x, ix);                             // final segment-follow pass (§3.9)
    break;

default:                                            // 8,9,0xA,0xD,0xE,0xF → no positioning
    continue;
}
```

**v37 spawn note:** `v37` is set to 1 in case 3 (`byte_0x44_68<=1` full extension) and 2 in case 3 (`byte_0x44_68==3`). It is consumed only in the `case 0..5` arm of LABEL_94. So a class-9 projectile spawns on the forward-whip impact frame; `v37==1` → low-power (`sub_2A7F0` a3=1), `v37==2` (byte_0x44_68==3) → the arm passes `v37 == 1` which is FALSE → a3=0 = re-fire using existing `manaRegen_0x88_136` without re-rolling.

---

## 2. `sub_29A90` RNG-draw order (byte-faithful)

Per branch (only if `actionIndex==233`), in order:
1. **DRAW #A** (EF:19807) — always, when `byte_0x46_70 <= 5`. → `byte_0x43_67 = rand%0x14`.
2. **DRAW #B** (EF:19813) — only if (after #A) `byte_0x46_70 == 1`. → `v6/v34x = rand & 7`.
3. **DRAW #C** (EF:19875) — inside case 1 body, if `byte_0x39_57 && !(byte_0x3E_62&7) && !(v34x&1)`. → yaw wander.
4. **DRAW #D** (EF:20150) — inside case 0xC, only when `dword_0x10_16+1 >= 9`. → branch life.

Additional rng advances happen INSIDE helpers on the branch's lcg:
- `sub_2A340` case 0 default (EF:20295): `maxSpeed = rand%0x1C` — advances branch lcg (reached via LABEL_94 case 0..5 and case-7 tail).
- `sub_2A7F0` (EF:20519,20521): advances the branch's lcg twice (once LCG, once `+= setting_30`) when `a3` (low path) is set.

The port must advance `rand_0x14_20` in EXACTLY this sequence per branch per tick, gated on the same conditions.

---

## 3. Positioning / relay helpers — verbatim

### 3.1 `sub_2A5B0(body, branch, a3=672)` — branch-head placement (EF:20374)
```c
v3x = &str_D404C[branch->byte_0x3B_59];
predictedAxis = body->position;
MoveEntity_57FA0(&pred, v3x->word_2 + body->yaw, 0, v3x->word_0);   // out along word_2 dir by word_0
pred.z += v3x->word_4;                                              // z lift
MoveEntity_57FA0(&pred, v3x->word_12 + body->yaw, v3x->word_14 + body->pitch, a3);  // + reach a3(=672)
CopyEntityPosition_57CF0(branch, &pred);
```

### 3.2 `sub_2A940(body, branch)` — branch move by table + global gate (EF:20570)
```c
if (x_DWORD_E9BA8) {                               // global "active" flag
    v3x = &str_D404C[branch->byte_0x3B_59];
    branch->roll_0x20_32 = v3x->word_12;
    branch->actSpeed_0x82_130 = 192;
    branch->byte_0x46_70 = 0;                       // (only when the global flag is on)
    branch->fov_0x22_34 = v3x->word_14;
    body->struct_byte_0xc_12_15.byte[1] |= 8u;
} else {
    body->struct_byte_0xc_12_15.byte[1] &= 0xF7u;
}
if (branch->actSpeed_0x82_130) {
    predictedAxis = branch->position;
    MoveEntity_57FA0(&pred, branch->roll + body->yaw, branch->fov + body->pitch, branch->actSpeed);
    CopyEntityPosition_57CF0(branch, &pred);
}
```
`x_DWORD_E9BA8` is a global; when set it force-resets `byte_0x46_70=0` and full speed — a "freeze/reset all branches" master switch (likely paused/debug). Port must read it.

### 3.3 `sub_2AA90(body, branch)` — the 9-segment SPLINE (EF:20632) — KEY

```c
v3x = &str_D404C[branch->byte_0x3B_59];
v12x = body->position;
MoveEntity_57FA0(&v12x, v3x->word_2 + body->yaw, 0, v3x->word_0);   // spline START anchor
v12x.z += v3x->word_4;

v14x = branch->position;                                            // spline END = branch head pos
v5  = (sub_583F0_distance_3d(&v12x, &v14x) - 468) / 24;             // curvature index
v18 = 16 - v5;   if (v18 > 15) v18 = 15;  else if (v18 < 0) v18 = 0; // clamp 0..15  → row into D40BC
v19 = Maths::sub_581E0_maybe_tan2(&v12x, &v14x);                    // yaw start→end
v17 = Maths::sub_58210_radix_tan(&v12x, &v14x);                     // pitch start→end

v6 = 0;
v7x = Entities_EA3E4[branch->word_0x34_52];                         // seg0
do {
    switch (v6) {                                                  // per-segment lateral offset
    case 0:            a1y = 0;                    break;           // seg0 sits at anchor
    case 1: case 8:    v9 = xx_DWORD_D40BC[v18][0]; a1y = -v9; break;
    case 2: case 7:    v9 = xx_DWORD_D40BC[v18][1]; a1y = -v9; break;
    case 3: case 6:    a1y =  xx_DWORD_D40BC[v18][1]; break;
    case 4: case 5:    a1y =  xx_DWORD_D40BC[v18][0]; break;
    }
    if (v6)   MoveEntity_57FA0(&v12x, v19, a1y + v17, 96);          // step 96 units, pitch += a1y
    CopyEntityPosition_57CF0(v7x, &v12x);                          // place segment
    if (branch->byte_0x46_70 == 7 && branch->byte_0x44_68 == 8) {  // "extend" phase → terrain clamp
        v10 = getTerrainAlt_10C40(&v7x->position);
        if (v7x->position.z <= v10) v7x->position.z = v10;
    }
    v6++;
    v7x = Entities_EA3E4[v7x->word_0x34_52];                        // next segment
} while (v6 < 9);                                                   // exactly 9 segments
```

**Spline shape:** segments 0..8 walk outward in fixed 96-unit steps from the body anchor, each bending in pitch by a symmetric offset pattern `{0, −c0, −c1, +c1, +c0, +c0, +c1, −c1, −c0}` where `(c0,c1) = D40BC[v18][{0,1}]`. This produces the drooping tentacle arc; `v18` (curvature index, driven by branch-head distance) selects the arc row. The two-column D40BC table IS the arc profile.

### 3.4 `sub_2A6F0(branch)` — target scan (EF:20451)
```c
v1 = branch->dword_0xA0_160x->word_160_0x1c_28;        // sight range
v9 = 0x10000000;   v2x = 0;
v3x = x_D41A0_BYTEARRAY_4_struct.dword_38519;          // head of a candidate list (players/creatures)
v8 = v1 * v1;
while (v3x > Entities_EA3E4[0]) {
    v4 = Maths::EuclideanDistXY_584D0(&branch->pos, &v3x->pos);
    if (v4 < v8) {                                     // within range²
        v5 = branch->dword_0xA0_160x->word_160_0x1e_30;    // FOV
        v6 = Maths::sub_581E0_maybe_tan2(&branch->pos, &v3x->pos);
        if ((uint16)sub_582B0(branch->yaw, v6) < v5 && v4 < v9) {   // within FOV and closest
            v2x = v3x;
            v9  = v4;
        }
    }
    v3x = v3x->next_0;
}
return v2x;                                             // nearest in-FOV target, or 0
```
Scans the `dword_38519` list (the per-frame player/target bucket), nearest-in-FOV wins. Returns entity ptr; caller stores `v10x - struct` as `word_0x96_150`.

### 3.5 `sub_2A660(body, branch)` — segment→body melee relay + branch death gate (EF:20395)
```c
v3 = branch->str_0x5E_94.word_0x62_98;                 // pending hit on this branch
if (v3) {
    body->str_0x5E_94.word_0x62_98   = v3;             // forward hit id to body
    v4 = branch->str_0x5E_94.dword_0x5E_94;            // damage amount
    body->str_0x5E_94.dword_0x5E_94  = v4;
    if (v4 > 76) v4 = 76;                              // ← CAP 76
    branch->life_0x8 -= v4;                            // apply capped dmg to BRANCH life
    v5 = branch->str_0x5E_94.word_0x62_98;
    branch->str_0x5E_94.word_0x62_98 = 0;
    branch->word_0x26_38 = v5;                         // remember attacker (→ case-1 retaliate)
    if (branch->life_0x8 < 0)
        branch->byte_0x46_70 = 6;                      // ← branch died → jump to case 6 (retract/coil)
}
```
**VERIFIED:** cap 76 confirmed (EF:20410); on branch `life<0` sets `byte_0x46_70 = 6` (EF:20418) — NOT an immediate delete. A dead branch enters the case-6→7 retract/coil animation, then detaches (case 8, decrement gauge), then may regrow (0xA→0xC, re-roll life 920..1839). So branches are effectively **regenerating limbs**, not permanently killable. The body (`life_0x8 = 1000000`, EF:20747) also absorbs the forwarded hit via its own `str_0x5E_94` (consumed by `sub_2A6B0`, §3.6).

### 3.6 `sub_2A6B0(body)` — body hit consume, returns 0/1/2 (EF:20423)
```c
v1 = body->str_0x5E_94.word_0x62_98;    v2 = 0;
if (v1) {
    v3 = body->byte_0x3B_59;                            // live-branch gauge
    body->word_0x26_38 = v1;                            // attacker
    if (v3) {                                           // gauge nonzero → a branch is alive → "branch hit"
        v2 = 1;
    } else {                                            // gauge 0 → all branches down → BODY exposed
        body->actionIndex_0x45_69 = 220;               // → state 0xDC path via caller? (actually 220)
        v2 = 2;
        body->word_0x24_36 = v1;
    }
    body->str_0x5E_94.word_0x62_98 = 0;
}
return v2;
```
Return codes: **0** = no hit; **1** = hit while branches alive (caller `sub_29670` sets actionIndex 218, `life=1000000`, target=attacker — EF:19570); **2** = hit with gauge 0 → sets `actionIndex=220` here and stores attacker in `word_0x24_36`. Note `byte_0x3B_59` (gauge) is the guard: the body is only vulnerable once every branch has detached (gauge decremented to 0 via case-8).

### 3.7 `sub_2A340(branch)` — speed / rotation integrator (EF:20233), switch on `word_0x2C_44`
- **mode 0** (EF:20252): compounds `roll += word_36 + maxSpeed + 73`, `fov += word_36 + maxSpeed + 62`; ramps `actSpeed += minSpeed` clamped ±192 (flips `minSpeed` sign on overflow, unless already ==192); decays `word_0x36_54` when `byte_0x3E_62` even; if `byte_0x43_67==3 && actSpeed==192` sets `minSpeed=-16`; else **RNG advance** `maxSpeed = rand%0x1C` (EF:20297).
- **mode 1** (EF:20300): `actSpeed += minSpeed` while `|actSpeed|<192`, clamp to ±192 by sign of minSpeed.
- **mode 2** (EF:20320): decelerate toward 0 by `minSpeed` (snap to 0 when `|actSpeed| < minSpeed`).
- **mode 3 / mode 4** (EF:20341): impact ramp by `dword_0x10_16`: {1→+192, 2→−130, 3→−23, 4→+192}.
- **mode 5**: default → no-op (falls to `default: return`).
- **mode 6** (EF:20359): `z -= word_0x36_54`; `actSpeed -= minSpeed`; clamp z to terrain floor via `getTerrainAlt_10C40`.

### 3.8 `sub_2A7F0(branch a1x, target a2x, char a3)` — CLASS-9 projectile spawn (EF:20507)
```c
v3x = 0;
if (a3) {                                              // low path re-rolls power
    // RNG advance (branch lcg) + setting perturb
    a1x->rand_0x14_20 = 9377 * a1x->rand_0x14_20 + 9439;
    v4 = a1x->rand_0x14_20 % 0xCu;                     // 0..11
    a1x->rand_0x14_20 += x_D41A0_BYTEARRAY_4_struct.setting_30;   // extra rng perturb
    a1x->manaRegen_0x88_136 = (v4 > 7) + 1;            // 1 or 2  (33% chance of 2)
}
v5 = a1x->manaRegen_0x88_136;
if (v5 >= 1) {
    if (v5 <= 1) {                                     // manaRegen == 1 → subtype-0 bolt
        if (!a3) goto LABEL_13;                        // a3==0 & regen 1 → NO new spawn (re-fire noop)
        v6x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->pos, 9, 0);   // class 9, subtype 0
        if (!v6x) goto LABEL_13;
        v6x->byte_0x43_67 = 10;
        v6x->byte_0x44_68 = 0;
        v7 = 15;                                       // ← SOUND 15
    } else {                                           // manaRegen == 2 → subtype-9 bolt
        if (v5 != 2) goto LABEL_13;
        v6x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->pos, 9, 9);   // class 9, subtype 9
        if (!v6x) goto LABEL_13;
        v6x->byte_0x43_67 = 10;
        v6x->byte_0x44_68 = 23;
        v7 = 23;                                       // ← SOUND 23
    }
    v6x->subSpellIndex_0x2A_42 = 850;                  // ← subSpellIndex 850  (VERIFIED EF:20551)
    PrepareEventSound_6E450(a1x->id_0x1A_26, -1, v7);  // SOUND 15 or 23 keyed on body id
}
LABEL_13:
if (v3x) {                                             // (v3x set = the new projectile)
    v3x->id_0x1A_26   = a1x->id_0x1A_26;               // owner = body id
    v3x->yaw_0x1C_28   = Maths::sub_581E0_maybe_tan2(&a1x->pos, &a2x->pos);   // aim at target
    v3x->pitch_0x1E_30 = Maths::sub_58210_radix_tan(&a1x->pos, &a2x->pos);
    v3x->position.z   += a1x->array_0x52_82.fov / 2;   // muzzle z lift
    v3x->word_0x96_150 = a1x->word_0x96_150;           // inherit target ref
    v3x->dword_0xA0_160x = &str_D7BD6[106];            // type row 106
    v3x->xsubtype_0x42_66 = a2x->model_0x40_64;
    v3x->xtype_0x41_65    = a2x->class_0x3F_63;
}
```
**VERIFIED against prior trace:** subSpellIndex **850** ✓ (EF:20551); manaRegen **1|2** ✓ via `(v4>7)+1` (EF:20522); sounds **15** (subtype-0/regen-1) and **23** (subtype-9/regen-2) ✓ (EF:20537/20549 → 20552). Correction: sound id equals `v7` = 15 or 23 keyed on `manaRegen`, and `PrepareEventSound` is keyed on `a1x->id_0x1A_26` (body id), not the branch. When `a3==0` and `manaRegen==1`, NO new projectile spawns (re-fire noop). When called from LABEL_94 with `v37==2`, `a3 = (v37==1) = 0` → this re-fire path.

### 3.9 `sub_2A9F0(body, branch)` — final segment-follow (EF:20608)
```c
predictedAxis = branch->position;
v3x = &str_D404C[branch->byte_0x3B_59];
predictedAxis.z += v3x->word_10;
MoveEntity_57FA0(&pred, v3x->word_12 + body->yaw, v3x->word_14 + body->pitch, v3x->word_6);
CopyEntityPosition_57CF0(branch, &pred);
```
Applies a small trailing offset (`word_6` reach, `word_10` z, `word_12/word_14` angle) to settle the branch head after the spline pass.

### 3.10 `sub_2AED0(entity, a2)` — pose/animation set (EF:20852)
```c
if (a2 != entity->word_0x5A_90) {                      // only on pose change
    entity->word_0x5A_90 = a2;
    entity->animationFrame_0x5C_92 = 0;
    entity->byte_0x5D_93 = x_BYTE_D8A2E[particlesParameters_D951C[a2].byte_12];
}
```
Called by body brain (`sub_29400`/`sub_29710`) with pose ids 315/337. No RNG, no sound.

### 3.11 `sub_2AE80(chainHead)` — chain hide/cleanup (EF:20830)
```c
if (a1x && a1x > Entities_EA3E4[0]) {
    for (ix = a1x->word_0x34_52; ; ix = v2x->word_0x34_52) {
        v2x = Entities_EA3E4[ix];
        if (v2x <= Entities_EA3E4[0]) break;
        DisableEntityDrawing04_57F10(v2x);             // hide every downstream member
    }
    DisableEntityDrawing04_57F10(a1x);                 // hide self
}
```
Single pass, hides the whole `word_0x34_52` chain (body→all branches→all segments). Used on spawn failure (ctor `v16` abort) and body death (`sub_298D0`).

### 3.12 `sub_2AF10(body, a2)` — m27 ground-move core (EF:20869), returns 1/2/3/4
Clears draw bits; if `byte[1]&8` set → skip move, return `v13=4`. Else predicts a step of `actSpeed` along yaw, snaps z to terrain; then:
- If moved but stuck in same 256-tile (`x>>8`,`y>>8` unchanged) → return **1** (arrived).
- Else if blocked (`sub_102D0`/`sub_1B830 >= 32`): if `yaw==roll` scan yaws ±91 stepping to 1024 for a free heading; found → set `roll=heading`, return **3**; not found → return **4** (fully blocked). If `yaw != roll` set collide bit, return **3**.
- Else clear path → return **2** (moved).
- Tail: if `v12` (moved/turn) rotate yaw toward `roll` via `sub_58350`; snap z to terrain.
- If `v13 == 4`: `body->actionIndex_0x45_69 = 216; body->dword_0x10_16 = 0;` and returns 4.

Return codes 1/2/3/4 confirmed (EF:20983). Caller `sub_29670` (EF:19575): `v3 >= 3`: ==3 → `sub_29A90` and return; ==4 → set actionIndex 216, dword_0x10_16=0, `sub_29A90`, return.

---

## 4. DATA TABLES — actual values

### 4.1 `str_D404C[5]` — per-branch spline parameters (source: `engine/Type_D404C.cpp`, static C array — compiled INTO the binary, not a loaded asset)

Struct `type_D404C` (11× int16, packed, 22 bytes) — `Type_D404C.h`:
`word_0, word_2, word_4, word_6, word_8, word_10, word_12, word_14, word_16, word_18, word_20`.

| idx | w0 | w2 | w4 | w6 | w8 | w10 | w12 | w14 | w16 | w18 | w20 |
|----|----|----|----|----|----|----|----|----|----|----|----|
| 0 | 0x0186 (390) | 0x0014 (20) | 0x0262 (610) | 0x001E (30) | 0x003C (60) | 0xFFB0 (−80) | 0x0009 (9) | 0x06EB (1771) | 0x0186 | 0x0121 | 0x0081 |
| 1 | 0x01B8 (440) | 0x006E (110) | 0x0258 (600) | 0x0000 | 0x0000 | 0xFF9C (−100) | 0x0197 (407) | 0x0695 (1685) | 0x01B8 | 0x0146 | 0x0092 |
| 2 | 0x01AE (430) | 0xFF9C (−100) | 0x0258 (600) | 0x0000 | 0x0000 | 0xFF9C (−100) | 0x0669 (1641) | 0x06AB (1707) | 0x01AE | 0x013F | 0x008E |
| 3 | 0x01A4 (420) | 0x0032 (50) | 0x01C2 (450) | 0x0000 | 0x0000 | 0xFFBA (−70) | 0x011C (284) | 0x0771 (1905) | 0x01A4 | 0x0137 | 0x008B |
| 4 | 0x01A4 (420) | 0xFFF6 (−10) | 0x01C2 (450) | 0x0028 (40) | 0x0B5E (2910) | 0xFFBA (−70) | 0x0302 (770) | 0x0485 (1157) | 0x01A4 | 0x0137 | 0x008B |

Fields USED by the machine:
- `word_0` = radial reach of the branch anchor (sub_2A5B0 3rd MoveEntity dist; sub_2AA90 anchor dist).
- `word_2` = yaw offset of the anchor direction (added to body yaw).
- `word_4` = z lift of the anchor.
- `word_6` = trailing reach (sub_2A9F0).
- `word_10` = z of the trailing settle (sub_2A9F0).
- `word_12` = branch yaw offset (the primary branch splay; used everywhere as `roll_0x20_32` seed).
- `word_14` = branch pitch offset (`fov_0x22_34` seed).
- `word_8, word_16, word_18, word_20` — NOT referenced by any m27 function traced here (word_8 only nonzero on idx4; likely used by the RENDERER — see GameRender*.cpp:3543/3147/3466 which also index `str_D404C[byte_0x3B_59]`).

### 4.2 `xx_DWORD_D40BC[17][3]` — spline arc profile (source: `engine/EventsFunctions.cpp:1092`, static C array in binary)

Only columns [0] and [1] are read by `sub_2AA90`; column [2] is always 0. Indexed by `v18` (0..15; row 16 is a zero guard).

| v18 | col0 | col1 |
|----|----|----|
| 0 | 0 | 0 |
| 1 | 0x6A (106) | 0x24 (36) |
| 2 | 0x97 (151) | 0x33 (51) |
| 3 | 0xBF (191) | 0x41 (65) |
| 4 | 0xDC (220) | 0x4B (75) |
| 5 | 0xF6 (246) | 0x54 (84) |
| 6 | 0x113 (275) | 0x5E (94) |
| 7 | 0x129 (297) | 0x66 (102) |
| 8 | 0x13E (318) | 0x6D (109) |
| 9 | 0x152 (338) | 0x74 (116) |
| 10 | 0x169 (361) | 0x7C (124) |
| 11 | 0x17C (380) | 0x82 (130) |
| 12 | 0x18E (398) | 0x88 (136) |
| 13 | 0x1A0 (416) | 0x8F (143) |
| 14 | 0x1B5 (437) | 0x96 (150) |
| 15 | 0x1C6 (454) | 0x9C (156) |
| 16 | 0 | 0 |

col0/col1 are the outer/inner pitch-bend magnitudes for the 9-segment arc; larger `v18` (branch head closer to anchor) → larger bend. Row selected by `v18 = clamp(16 − (dist3d−468)/24, 0, 15)`.

**Provenance:** BOTH tables are static C arrays compiled into remc2 (they are literal initializers in `.cpp`, NOT read from any GOG asset file). The port should hardcode these exact values.

---

## 5. Life & death semantics

- **BODY life:** `life_0x8 = 1000000` (`sub_2AC50` EF:20747; reset to 1000000 by `sub_29670`/`sub_29400`/`sub_29930` on any branch-guarded hit). `maxLife_0x4 = 36000`. The body is functionally **unkillable while `byte_0x3B_59` (gauge) > 0** — `sub_2A6B0` only exposes it (actionIndex 220, return 2) once every branch has detached (gauge==0).
- **BRANCH life (initial):** `sub_2AD40` EF:20795: `life = maxLife = 460*v2 + 920` where v2 = 1-based branch order → **1380, 1840, 2300, 2760, 3220**.
- **BRANCH life (regrow):** `sub_29A90` case 0xC EF:20155: `life = rand%0x398 + 920` → **920..1839**. Reached only after a full detach→regrow cycle.
- **BRANCH damage:** applied in `sub_2A660` (EF:20412), capped at **76**/hit. `life<0` → `byte_0x46_70 = 6` (retract/coil animation, then detach — NOT a delete).
- **SEGMENT life:** segments (0xEA) have NO life logic in this machinery — they are pure followers positioned by `sub_2AA90`; their `struct_byte_0xc[0]` draw bit is toggled by the branch machine (cases 7/0xC/0xF) to appear/disappear. A "segment death" is not modeled; segments hide/show with their branch's state.
- **BODY death:** handled by `sub_298D0` (state 0xDD, EF:19679 — see prior trace §6): `life=-1`, `TransformEntityToManaSphere_36BA0(body, true)`, optional class-10 subtype-1 mana sphere, then `sub_2AE80(body)` hides the entire chain.

**Contrast for the port:** branches are regenerating limbs (die → retract → detach → regrow with fresh 920..1839 life, gauge --/++); only when ALL branches are simultaneously detached (gauge 0) does a hit reach the body, which is otherwise 1000000-HP invulnerable. This is the "kill the kraken by clearing its tentacles" mechanic.

---

## 6. XP / kill-credit (`sub_6D8B0`) in this machinery

**None.** Grep of `sub_6D8B0` shows NO call inside `sub_29A90`, `sub_2A340/5B0/660/6B0/6F0/7B0/7F0/940/9F0/AA90/AC50/AD40/AE30/AE80/AED0/AF10`, or the m27 body brains. Kill credit for an m27 is granted the generic way when the whole entity dies — via the model-keyed callers at EF:62985 (`Entities[word_0x26_38]->model`) / EF:63551 (`model_0x40`), i.e. the killer's stored attacker model → `sub_6D8B0(id, model, delta)`. `word_0x26_38` (attacker) is threaded through `sub_2A660`→body, so the last attacker is credited on body death. No per-branch or per-segment XP is emitted by the branch machine itself.

---

## 7. Sound ids 37 / 59 / 7 / 62 — OWNERSHIP CORRECTION

The prior trace flagged these as "adjacent m27 sub-handlers sub_27950+". **They are NOT m27.** Verified by aggro-code base (base = model×8) and Events.cpp dispatch:

| sound | function | line | aggro base | ⇒ model | evidence |
|----|----|----|----|----|----|
| **7** | `sub_28570` | EF:18682 | `sub_1C310(a1x, **192**, sub_1CF20)` | **model 24** (192/8) | EF:18683 |
| **37** | `sub_28C60` | EF:19080 | `sub_1C310(a1x, **200**, sub_1CC20)` | **model 25** (200/8) | EF:19081 |
| **59** | `sub_27E00` | EF:18310 | (uses `word_0x96_150` chase; state block 0x27E00) | model 24/25 band | dispatched EV:1939 (0x208e00) |
| **62** | `sub_28FF0` | EF:19255 | (state handler in the 0x28xxx = model 24/25 band) | model 24/25 band | dispatched EV:2064 (0x209ff0) |

`sub_28570` (192) and `sub_28C60` (200) are unambiguously **model 24 and model 25** head-state handlers (thin wrappers over the shared `sub_1C310` chase-attack primitive with those aggro bases), dispatched by address in Events.cpp exactly like every other creature state — they are independent single-entity creatures, NOT part of the m27 body-driven branch machine. The only sounds the m27 branch machine actually emits are:

- **17** (whip crack) — `sub_29A90` case-5 sub-6 (EF:19987) and LABEL_94 case-0..5 on projectile spawn (EF:20202).
- **15 / 23** (bolt spawn low/high) — `sub_2A7F0` (EF:20552).
- **22** (body teleport) — `sub_29400` phase 9 (prior trace §6, EF:19504).

---

## 8. Field glossary (verbatim decomp names used above)

| field | offset | role in m27 machine |
|----|----|----|
| `actionIndex_0x45_69` | 0x45 | dispatch state (body 0xD9, branch 0xE9=233, seg 0xEA=234) |
| `byte_0x3B_59` | 0x3B | branch: fixed index 0..4 (indexes str_D404C). body: **live-branch gauge** (5 init, ±) |
| `byte_0x46_70` | 0x46 | branch sub-state 0..0xF (the 16-way switch) |
| `byte_0x44_68` | 0x44 | inner sub-phase (cases 3/5/7) |
| `byte_0x43_67` | 0x43 | whip counter / link-length multiplier (case 7) |
| `byte_0x3E_62` | 0x3E | per-branch phase counter (`++` each tick; gates `&7`, `&1`, `&0x1F`) |
| `word_0x2C_44` | 0x2C | sub_2A340 mode selector (0..6) |
| `word_0x36_54` | 0x36 | link length / reach (grows in cases 3/7/8) |
| `word_0x96_150` | 0x96 | branch: target id / tracked-segment ref |
| `word_0x26_38` | 0x26 | last attacker id (retaliate trigger) |
| `word_0x24_36` | 0x24 | body: attacker stored on exposure (sub_2A6B0 ret 2) |
| `dword_0x10_16` | 0x10 | per-state countdown / walk counter |
| `roll_0x20_32` / `fov_0x22_34` | 0x20/0x22 | branch splay yaw/pitch offsets (from str_D404C w12/w14) |
| `yaw_0x1C_28` / `pitch_0x1E_30` | 0x1C/0x1E | branch aim |
| `minSpeed_0x84_132` / `actSpeed_0x82_130` / `maxSpeed_0x86_134` | | swing speed integrator |
| `manaRegen_0x88_136` | 0x88 | sub_2A7F0 bolt power (1 or 2) |
| `str_0x5E_94.word_0x62_98` / `.dword_0x5E_94` | | pending-hit id / damage |
| `struct_byte_0xc_12_15.byte[0..3]` | 0x0C | draw/state flag bits (bit0 draw, bit3 group, byte[1]&8 freeze) |
| `rand_0x14_20` | 0x14 | per-entity LCG (`9377*x+9439`) |

---

## OPEN / uncertainties

1. **`x_DWORD_E9BA8`** (sub_2A940 gate, EF:20577) — a global flag that, when set, force-resets every branch to `byte_0x46_70=0` + full speed. Its writer was not traced; likely a global "reset/freeze animations" toggle (pause / level-transition). The port must replicate the read; behavior when 0 (normal) is fully specified above.
2. **`str_D404C` fields word_8/16/18/20** — not referenced by any m27 SIM function. word_16/18/20 mirror word_0-ish magnitudes; GameRender*.cpp also indexes `str_D404C[byte_0x3B_59]` (HD:3543 / NG:3147 / Orig:3466) so these are RENDER-only spline params. Confirm against the renderer if porting m27 visuals; not needed for sim.
3. **`sub_2A340` mode 0 compounding roll/fov** (EF:20256–20263) — the `+73`/`+62` constants and the `roll += word_36+maxSpeed+73` accumulation are verbatim but their geometric meaning (a spiralling wind-up) is inferred, not proven. Reproduce arithmetically.
4. **case-3 v37==2 re-fire** — when `byte_0x44_68==3` sets `v37=2`, LABEL_94 calls `sub_2A7F0(ix, v35x, v37==1)` = `sub_2A7F0(ix, v35x, 0)`. With `a3=0`, `sub_2A7F0` re-uses the existing `manaRegen`; if regen==1 it spawns NOTHING (LABEL_13 with v3x=0). So the second impact frame only fires a bolt if the first roll gave regen==2. Verified by reading, flagged because it is easy to mis-port as "always fires twice."
5. **`sub_2A6B0` return 2 sets actionIndex 220 in-place** (EF:20442) — the body then continues; the caller `sub_29670` only special-cases returns 0 and 1 (EF:19563), so on return 2 it proceeds to `sub_2AF10`+rng+`sub_29A90` with actionIndex already flipped to 220. State 220 (0xDC = `sub_298B0`, `life=-1`+PreKill) will be picked up next tick. Confirmed but the exact next-tick redispatch path (220 → death) relies on the actionIndex→address table (binary), consistent with prior trace §6 note.
6. **Sound 59 / 62 exact aggro base** — `sub_27E00`/`sub_28FF0` don't call `sub_1C310` with a literal base in the lines read, so their model (24 vs 25) is inferred from address adjacency to the 192/200 handlers. They are definitively in the model-24/25 band and definitively NOT m27; the 24-vs-25 split for these two specific handlers is the only residual uncertainty and does not affect the m27 port.
