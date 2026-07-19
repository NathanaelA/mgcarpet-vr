# MC2 (5,22) WORM — SEGMENT PATH-FOLLOW law, verbatim decompile trace

Answers the PLAYTEST-11 observation (player, retail-experienced, 2026-07-11): *in retail the ENTIRE worm follows
the SAME PATH the head took — like the classic snake game — and if the head loops on itself the rest of the worm
walks THROUGH the head, retracing the whole trail. In our port only the 3-4 segments near a short head-loop
moved/knotted while the rest of the worm FROZE.* The hypothesis put to this trace: **retail uses a breadcrumb /
position-history buffer, our port follows positionally.**

Companion to `mc2-m22-worm-steering.md` (head steering — the head's fixed-heading cruise) and
`mc2-m22-worm-helpers.md` / `mc2-multipart-chains.md` (the helper suite, ctor, tail spawn, hit relay, chain-kill).
Those cover the STEERING; this file re-examines the SEGMENT-FOLLOW law only, and adds the m0/m3 comparison.

All cites to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`:
`EF = EventsFunctions.cpp`, `GT = global_types.h`, `PL = Player.cpp`. Port cites: `multipart.rs`/`mobs.rs` =
`crates/mgc-sim/src/mc2/…`, `mc1/mobs.rs`/`mc1/world.rs` = `crates/mgc-sim/src/mc1/…`. Trace date 2026-07-11.

---

## TL;DR — THE LAW (read first)

**BREADCRUMB HYPOTHESIS: FALSE (decompile-confirmed).** There is NO position-history buffer anywhere. The
`type_entity_0x6E8E` struct carries exactly ONE live position (`position_0x4C_76`, GT:371) and one prediction
axis (`axis_0x9A_154x`, GT:387) — no ring buffer, no per-tick trail array, no `[512]`-style path store on any
entity (the only `path[512]` in the engine is a *file-path* string, GT:396 / EF:31332). A whole-file grep for
history/breadcrumb/trail/prevPos buffers returns nothing on the entity path.

**The m22 tail is NOT even a trailing follow-the-leader chain.** It is a RIGID SPIRAL COIL rebuilt every tick,
head-relative, by `sub_271D0` (EF:17685). Each tail segment positions itself at:

```
    seg.pos = grandparent.pos  +  polar( angle = head.subSpellIndex + spiral(seg.ringOffset, head.animFrame, head.tailLen),
                                          dist  = seg.linkPitch + grandparent.linkPitch )
```

— i.e. two links UP the chain (`word_0x32_50` twice), stepped out at an angle that is a pure function of the
HEAD's serpentine spin (`subSpellIndex_0x2A_42`) and the segment's fixed ring offset (`byte_0x46_70`). It does
**not** read the direction the chain is travelling and does **not** consult where the head has BEEN. The coil
shape is anchored to the head's CURRENT position and rotates with the head's spin; when the head moves, the
grandparent-chaining drags the whole coil along rigidly. **This does not trace the head's path.**

**Therefore the player's "whole worm retraces the head's exact trail like a snake" is NOT reproducible from the
recovered MC2 m22 code — and neither is it a property our port is missing. Our port (`m22_tail_follow`
multipart.rs:624-656) is a FAITHFUL transcription of `sub_271D0`.** The port and retail m22 follow are the same
positional-coil law.

**What this means for the playtest gap (the actionable finding):**
- The retail snake-trail-retrace behaviour the player remembers is the behaviour of a **follow-the-leader chain
  with a SHORT link length relative to speed** — that is the **m0/m3 worm** (`sub_1B6B0`, EF:8696), the MC1-lineage
  worm, whose segments DO chase their immediate parent along the bearing to it (a trailing chain that closely
  approximates the leader's recent path when links are tight). It is still NOT a stored breadcrumb — the "snake"
  look is an emergent property of a tight positional trail — but it is the code that LOOKS like the player's
  memory. The MC2 (5,22) worm is a DIFFERENT creature (a spinning castle-mana-thief coil), not the snake.
- The "3-4 segments move, the rest FREEZE" symptom the player saw in OUR port is the tell of a **one-link-per-tick
  propagation wave** — a follow chain where each segment reads its leader's STALE (previous-tick) position, so
  motion crawls down the body one segment per frame. That is NOT what `sub_271D0`/`m22_tail_follow` does (it reads
  the grandparent's CURRENT position; heads have lower slot indices than their segments and tick first, so the
  chain re-seats fully every tick — see §5). If the player genuinely saw a freeze, the creature under observation
  was the **m0/m3 worm** (or MC1's), whose follow is a trailing chain, AND some segments had `byte_0x39_57`
  (f58, the "awake" gate) == 0 so they only snap onto the leader every 4th tick (EF:8729) — a real
  intermittent-follow that reads as a partial freeze. See §3/§5 for the precise gate.

Bottom line: **no breadcrumb exists; do NOT add one.** The m22 coil is faithful. The snake-retrace + freeze
observation is about the m0/m3 (or MC1) follow-the-leader worm, whose law is §3 — and the freeze there is the
`byte_0x39_57` awake gate, not a missing trail.

---

## 1. There is no position-history buffer — the entity struct + grep evidence

### 1.1 The entity has ONE position

`type_entity_0x6E8E` (GT:346-392) — the only spatial fields:
```c
axis_3d position_0x4C_76;   //76  ACTUAL X Y Z            (GT:371)  ← the single live position
axis_4d array_0x52_82;      //82  {yaw,pitch,roll,fov}    (GT:372)  ← rotations + the LINK PITCH (see §4)
axis_3d axis_0x9A_154x;     //154 secondary/prediction    (GT:387)  ← used as a velocity/aim scratch, not a trail
```
`axis_3d` = `{uint16 x; uint16 y; int16 z;}` (axis_3d.h:4-8). No array of past positions. No `pos[N]` ring. The
follow reads only `link.position_0x4C_76` (a neighbour's CURRENT position), never a stored history.

### 1.2 Grep confirmation

- `grep -n "history|breadcrumb|trail|prevPos|posBuffer|path\[" EventsFunctions.cpp` → **only** `char path[512]`
  at EF:31332 (a filename buffer inside a file-open helper; `Pathstruct.path[512]` GT:396). No entity-position
  history anywhere.
- The m22 follow (`sub_271D0`), the m0/m3 follow (`sub_1B6B0`), and the tail spawn (`sub_4CB60`/`sub_274C0`)
  reference only `word_0x32_50` (parent link), `word_0x34_52` (child link), `word_0x96_150` (head ref), and
  neighbour `position_0x4C_76` — never a buffer.

**Verdict for Q1/Q2: there is NO breadcrumb buffer, no sampling interval, no wrap. The question's premise is
falsified by the struct + the grep.** The follow is positional.

---

## 2. The m22 tail follow `sub_271D0` — TRANSCRIBED COMPLETELY (EF:17685-17714)

This is the per-tick body handler for every m22 tail segment (state 0xB4 → `sub_26CA0` = `sub_271D0` +
`sub_26D20`, EF:17420-17424). It is ALSO run once per segment at spawn (`sub_276E0` EF:17920).

```c
void sub_271D0(type_entity_0x6E8E* a1x)//2081d0                             // EF:17685
{
    if (a1x->word_0x96_150) {                                               // has head ref
        v2x = Entities_EA3E4[a1x->word_0x96_150];                           // v2x = the WORM HEAD
        v3  = v2x->subSpellIndex_0x2A_42;                                   // the head's serpentine SPIN angle
        v4  = (v3 + sub_273C0(v2x->animationFrame_0x5C_92, v2x->word_0x36_54,//   + the ring's spiral offset
                              a1x->byte_0x46_70,           v2x->byte_0x46_70)) & 0x7FF;
        a1x->word_0x2C_44 = v4;                                             // remember this segment's orbit yaw
        v5x = Entities_EA3E4[a1x->word_0x32_50];                            // v5x = PARENT (one link up)
        if (v5x) {
            v6 = v5x->word_0x32_50;
            if (v6)
                v5x = Entities_EA3E4[v6];                                   // *** step to GRANDPARENT (two links up) ***
        }
        predictedAxis_EB398ar = v5x->position_0x4C_76;                      // START at the grandparent's position
        MoveEntity_57FA0(&predictedAxis_EB398ar, v4, 0,                     // step out at angle v4 (pitch arg = 0)
                         a1x->array_0x52_82.pitch + v5x->array_0x52_82.pitch);//   dist = own linkPitch + gp linkPitch
        predictedAxis_EB398ar.z =                                          // z set explicitly from the pitch diff:
            v5x->array_0x52_82.pitch - a1x->array_0x52_82.pitch + v5x->position_0x4C_76.z;
        CopyEntityPosition_57CF0(a1x, &predictedAxis_EB398ar);             // commit
    }
}
```

The critical facts (each load-bearing):

1. **Anchor = GRANDPARENT (2 links up), not the immediate parent.** `v5x` starts as the parent
   (`word_0x32_50`), and if the parent itself has a parent, `v5x` advances to the GRANDparent (EF:17701-17707).
   Because the tail is spawned as ordered ring pairs `+1,-1,+2,-2,…,+7,-7` (`sub_4CB60` EF:34420), the
   grandparent of `+n` is `+(n-1)`'s successor and of `-n` is `+n` — the coil pairs off two links back.

2. **Angle = head-relative spin, NOT chain travel direction.** `v4 = head.subSpellIndex_0x2A_42 +
   sub_273C0(head.animFrame, head.writhePhase, seg.ringOffset, head.tailLen)` (EF:17698-17699). Every term is a
   property of the HEAD's animation state or the segment's FIXED ring offset. Nothing here reads the direction the
   worm is moving or where the head has been. `sub_273C0` (EF:17780, transcribed in `mc2-m22-worm-helpers.md` §7)
   is a pure spiral function of `|ringOffset|`, the animation frame, and the offset sign/phase-bit-1 chirality.

3. **Distance = sum of two link pitches** (`seg.pitch + grandparent.pitch`, EF:17709). `array_0x52_82.pitch` is
   the per-link SPACING set by `sub_27610` (§4). `MoveEntity_57FA0` is called with its pitch argument = 0, so the
   step is purely horizontal at angle `v4`; z is then overwritten by the pitch-difference line (EF:17710).

4. **Consequence — the coil is rebuilt every tick, head-relative.** With the head at the origin of the coil and
   `subSpellIndex` advancing each tick (`sub_272C0` EF:17763: `subSpellIndex += word_0x2C_44`), the entire body
   RE-SEATS into a rotating spiral around the head's CURRENT position every frame. It does not trail behind the
   head along a path; it wraps around the head. The visible writhe IS this rotating coil.

**This is a positional, head-anchored spiral — categorically not a breadcrumb and not even a follow-the-leader
trail.** (Q3: the actual law is characterised here and in the TL;DR.)

### 2.1 `MoveEntity_57FA0` (PL:6-19) — the polar step primitive

```c
void MoveEntity_57FA0(axis_3d* position, uint16 yaw, int16 pitch, int16 speed) {   // PL:6
    if (speed) {
        pitch &= 0x7ff; yaw &= 0x7ff;
        if (pitch) {                                                    // pitch==0 in sub_271D0 → skipped
            position->z -= (speed * sin_DB750[pitch]) >> 16;
            speed        = (speed * sin_DB750[0x200 + pitch]) >> 16;
        }
        position->x += (speed * sin_DB750[yaw])         >> 16;          // x += dist·sin(yaw)
        position->y -= (speed * sin_DB750[0x200 + yaw])  >> 16;         // y -= dist·cos(yaw)
    }
}
```
2048-step angle tables, 16.16 fixed point. In `sub_271D0` the pitch arg is 0, so it is a pure XY step at angle
`v4`; the z comes from the explicit `grandparent.pitch − seg.pitch + grandparent.z` line.

---

## 3. The m0/m3 worm follow `sub_1B6B0` — the ACTUAL follow-the-leader chain (EF:8696-8734)

This is the DIFFERENT worm the player's "snake trail" memory matches. m0 (`sub_4B240`) and m3 (`sub_4B6F0`)
spawn 16 children in state 0xE8 (232); every child ticks through `sub_1B6B0`:

```c
void sub_1B6B0(type_entity_0x6E8E* a1x)//1fc6b0                             // EF:8696
{
    v1x = Entities_EA3E4[a1x->word_0x32_50];                                // v1x = PARENT (immediate leader)
    if (v1x->class_0x3F_63 != 5)
        DisableEntityDrawing04_57F10(a1x);                                 // leader dead/gone → orphan-hide
    if (a1x->byte_0x39_57) {                                                // *** AWAKE gate (f58 nonzero) ***
        a1x->yaw_0x1C_28   = sub_581E0_tan2 (&a1x->pos, &v1x->pos);         // face the LEADER
        a1x->pitch_0x1E_30 = sub_58210_radix(&a1x->pos, &v1x->pos);
        predictedAxis = v1x->position_0x4C_76;                             // START at the leader's position
        MoveEntity_57FA0(&pred, a1x->yaw, a1x->pitch, -a1x->word_0x36_54); // step BACK by ONE link length
        CopyEntityPosition_57CF0(a1x, &pred);                             // sit link-length behind the leader
        if (a1x->str_0x5E_94.word_0x62_98) { … apply damage to THIS seg … }
        else a1x->word_0x26_38 = 0;
    }
    else if (!(a1x->byte_0x3E_62 & 3)) {                                   // *** ASLEEP: every 4th child only ***
        CopyEntityPosition_57CF0(a1x, &v1x->position_0x4C_76);            // snap ONTO the leader
        a1x->yaw_0x1C_28 = v1x->yaw_0x1C_28;
    }
}
```

Key differences from m22, and why this is the "snake":

- **Anchor = IMMEDIATE parent** (`word_0x32_50` once, no grandparent hop), at distance `word_0x36_54` (the link
  length) along the bearing FROM the segment TO its leader (EF:8709-8713). This is a classic trailing chain: each
  segment chases the one ahead. With a SHORT link length relative to the head's step, the chain hugs the leader's
  recent path — the snake look. It is still positional corner-cutting, not a stored trail, but it VISUALLY
  retraces the path when links are tight.
- **The AWAKE gate `byte_0x39_57` (f58).** If f58 == 0, the full follow is SKIPPED; only every 4th child
  (`byte_0x3E_62 & 3 == 0`) snaps onto its leader (EF:8729-8733), and the other 3-of-4 do NOTHING that tick.
  **This is the mechanism that reads as "some segments move, the rest freeze."** f58 is driven by the awake
  pre-pass (`sub_68BF0`/`sub_68C70`; port `mc2_awake_pass` mobs.rs:2177) — the head arms 16, propagates 18 to
  followers, decrements one per tick. When a chain's f58 lapses to 0 (out of the player's proximity window
  0x2400000), the tail stops following and only the every-4th snap remains — a partial freeze.

**m22 has NO such freeze**: `sub_271D0` does not gate on `byte_0x39_57` at all (§2) — the coil always re-seats.
So if the player saw a FREEZE, they were watching the m0/m3 (or MC1) follow-the-leader worm, not the m22 coil.

---

## 4. Link spacing — the derived sprite-extent metric (Q4)

The per-link spacing lives in `array_0x52_82.pitch` (set by `SetEntityShiftRot_49EA0` EF:32874-78, which writes
`pitch = roll = shift, fov = fov`). For m22 the tail spacing is set by `sub_27610` (EF:17893):

```
    v = 550 * particlesParameters_D951C[ sub_278F0(colorIdx, headTailLen, segRingOffset) ].rotSpeed_8;
    SetEntityShiftRot_49EA0(seg, v/1000, v/1000);       // pitch = fov = 550·rotSpeed_8/1000
```
so **m22 link spacing = 550·rotSpeed_8/1000** of the segment's colorize row (`mc2_m22_shift_rot`
multipart.rs:586-600 ports this verbatim). The step distance in `sub_271D0` is `seg.pitch + grandparent.pitch`
(§2) — i.e. two of these spacings.

**rotSpeed_8 is DERIVED from the sprite bitmap at load** (`sub_...` EF:44870-44910). The loader walks the particle
table and, per row, decompresses the tmap to read its width (`*(v1+2)`) and height (`*(v1+4)`), then:
```c
if (speed_6) { if (!rotSpeed_8) rotSpeed_8 = height * speed_6 / width; }   // EF:44898
else         {  speed_6         = width  * rotSpeed_8 / height; }          // EF:44902
```
So **`speed_6` and `rotSpeed_8` are two views of the same sprite aspect** (one derives the other from the
bitmap's width/height). The m22 spacing reads the DERIVED `rotSpeed_8`; the m0/m3 chain link length
(`word_0x36_54`) reads the DERIVED `speed_6` (m3 child: `f56 = 65·speed_6/100`, first child 125%, EF:33846-51;
m0 child copies `array.pitch`). **This CONFIRMS the PLAYTEST-11 finding** ("link length = derived speed_6, 65%
multipart metric") — precisely: the *m0/m3* chain uses the 65%·`speed_6` derivation; the *m22* coil uses
`550·rotSpeed_8/1000`, and both `speed_6`/`rotSpeed_8` come from the same load-time sprite-extent derivation
(EF:44870-44910). The port's PLAYTEST-12 provenance note (multipart.rs:338-344) — derive from dims, floor 96 only
for dims-less unit fixtures — is the faithful reproduction.

---

## 5. Motion coupling, yaw, z, and dispatch order (Q5)

- **Does the body stop when the head stops?** m22: the coil is rebuilt from the head's CURRENT position every
  tick regardless of head motion, but `subSpellIndex` keeps advancing (`sub_272C0` EF:17763), so a stationary
  head still has a *rotating* coil (writhe continues). Lateral translation of the body only happens when the head
  translates. m0/m3: yes — a segment's position is `leader.pos − linkLen`, so if the leader is static the segment
  is static (once settled). This is the corner-cutting trailing chain.
- **Does each segment inherit yaw from a path direction?** m22: NO — the segment's orbit yaw `word_0x2C_44` is the
  head-relative spiral angle `v4` (EF:17700), not a travel bearing. m0/m3: the segment's `yaw_0x1C_28` is set to
  the bearing toward its LEADER (EF:8709), i.e. the local chain direction, not a stored path tangent.
- **z per segment:** m22: z = `grandparent.pitch − seg.pitch + grandparent.z` (EF:17710) — a pitch-driven coil
  offset from the grandparent's z; NOT an independent terrain-follow (only the HEAD terrain-follows, via
  `sub_26FF0` EF:17589, `mc2-m22-worm-steering.md` §1.3). m0/m3: z tracks the leader via the 3D `MoveEntity` with
  the radix-tan pitch (EF:8710-8713); no independent terrain clamp on the child.
- **Dispatch order (why the port does NOT wave-freeze):** the world loop iterates ascending slot index
  (`for i in 1..ent.len()`, mc1/world.rs:1229). Heads are `new_event`'d BEFORE their tail segments (m22 ctor
  spawns the head then `sub_4CB60` spawns the tail; port `mc2_spawn_m22` multipart.rs:466-508), so a head has a
  LOWER slot than its segments and ticks FIRST. Each segment therefore reads its grandparent's ALREADY-UPDATED
  position this tick — the coil re-seats fully in one frame, no one-link-per-tick wave. Retail dispatches in slot
  order too. So neither retail nor the port produces the "wave crawling down a frozen body" for m22.

---

## 6. PORT DELTA — where our follow law sits vs retail

**Headline: our m22 follow is FAITHFUL. There is no divergence to fix, and NO breadcrumb to add.**

| # | item | port site | retail | verdict |
|---|---|---|---|---|
| **m22 follow** | grandparent-anchored spiral coil, angle = head.subSpellIndex + spiral(off), dist = own+gp pitch, z = gp.pitch−own.pitch+gp.z | `m22_tail_follow` multipart.rs:624-656 | `sub_271D0` EF:17685-17714 | **FAITHFUL — arm-for-arm** (parent→grandparent hop multipart.rs:640-643; angle 631-638; dist 653; z 654) |
| **m22 link spacing** | `550·rotSpeed_8/1000` from the colorize row; step = own+gp pitch | `mc2_m22_shift_rot` multipart.rs:586-600 | `sub_27610` EF:17893 | **FAITHFUL** (Q4); rotSpeed_8 derived load-time EF:44870-44910 |
| **m0/m3 follow** | immediate-parent trail, step −f56 along bearing to leader; asleep = every-4th snap | `mc2_child_tick` multipart.rs:357-388 | `sub_1B6B0` EF:8696-8734 | **FAITHFUL** — the awake gate f58 (multipart.rs:364) and every-4th snap (384-387) both present |
| **dispatch order** | ascending slot; head before segments | mc1/world.rs:1229; `mc2_spawn_m22` alloc order multipart.rs:470/514 | slot-order dispatch, head allocated first | **FAITHFUL** — no wave-freeze introduced |
| **NO breadcrumb** | none | (no history buffer) | (no history buffer — §1) | **CORRECT — do NOT add one** |

### 6.1 Reconciling the player's observation (the real action)

The decompile proves retail has no path-history and the m22 coil does not trace the head's path. Two possibilities
remain for the playtest report, and BOTH point away from an m22 follow-law change:

1. **The player was recalling the m0/m3 (or MC1) worm** — the tight follow-the-leader trail (§3) — whose snake
   look is emergent, and whose partial-freeze is the `byte_0x39_57` awake gate (EF:8729), not a missing trail.
   Our m0/m3 port already reproduces that gate faithfully (multipart.rs:364/384). If the freeze looked WRONG in
   our port, the suspect is the **awake pre-pass f58 propagation** (`mc2_awake_pass` mobs.rs:2177-2213), not the
   follow: verify the head arms 16 and pushes 18 down the chain each tick so the whole chain stays awake inside
   the proximity window — if our f58 propagation is off, distant segments drop to the every-4th snap and appear
   to freeze exactly as described. **This is the single concrete thing to check.**

2. **The player was watching the m22 coil and read its rigid head-anchored writhe as "not following the path."**
   In that case the port is already correct and the perceived difference is either (a) the coil's *visual*
   spacing/writhe amplitude (governed by `rotSpeed_8` sprite derivation, §4 — verify the m22 sprite dims feed the
   derivation, else the 96 floor gives a wrong coil radius), or (b) a rendering/altitude read, not the follow law.

**Recommended action:** do NOT implement a breadcrumb trail — it would DIVERGE from retail. Instead: (a) confirm
which worm the player watched (m0/m3 vs m22 — they are visually very different: m0/m3 = a segmented trailing worm,
m22 = a spinning coiled castle-thief); (b) if m0/m3, audit `mc2_awake_pass` f58 propagation so no in-range segment
falls to the every-4th snap; (c) if m22, audit the `rotSpeed_8` sprite-dims derivation feeding the coil spacing.

---

## OPEN

1. **Which worm the player observed.** The playtest note says "worm"; MC2 has TWO segmented class-5 worms with
   DIFFERENT follow laws — m0/m3 (trailing chain, `sub_1B6B0`) and m22 (spinning coil, `sub_271D0`). The
   snake-retrace + freeze description matches m0/m3, not m22. A level census of which model is authored on the
   level the player was on would settle it. Until then this trace answers the m22 question definitively (no
   breadcrumb, faithful coil) and flags m0/m3 as the likely subject.
2. **`byte_0x39_57` (f58) awake propagation for chains.** The freeze symptom, IF on m0/m3, is the awake gate.
   The propagation walk (head→`word_0x34_52` chain, arm followers to 18) is in `sub_68BF0`/`sub_68C70`
   (:55469/:55494) and ported at `mc2_awake_pass` mobs.rs:2188-2210 — not re-diffed against retail this pass;
   flagged as the prime suspect for a port-side freeze. (MC1's worm uses the same idiom.)
3. **Head terrain-follow vs body z (m22).** Only the head terrain-follows (`sub_26FF0`, `mc2-m22-worm-steering.md`
   §1.3); body z is a pure coil offset from the grandparent (§5). Confirmed by reading; the visual result (body
   clipping terrain on slopes) is retail-authentic, not a port bug.
