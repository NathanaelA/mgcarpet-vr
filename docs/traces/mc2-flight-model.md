# MC2 Player Flight Model — verbatim sim trace (Phase 4.4)

Scope: the REAL MC2 player-carpet flight tick — `sub_5D530` (EF:59610),
its commit gate `moveTest_5D0A0` (EF:59429), the nudge `sub_5DD50`
(EF:59854), the leash `sub_5DE30` (EF:59889), the command integrator
`sub_5F380` (EF:60748) + the pre-move filter write (EF:38060), and the
per-entity tuning row (`str_D7BD6`) the carpet reads. This is the arm
that will land as `FlightVerb::Mc2`; today the player flies the MC1 arm
(`flight::mc1_move`) on MC2 levels.

Decompile citations: EF = `EventsFunctions.cpp`, TR = `Terrain.cpp`,
PI = `PlayerInput.cpp`, L = `Level.cpp`, otherwise `file:line`. Root
`reference/remc2/remc2/engine/`.

Cross-refs — build on, do NOT re-derive:
- `docs/traces/mc2-mouse-aim.md` — Channel A pose (`roll`/`pitch` →
  yaw-rate / absolute-pitch), Channel B free-look offset, device switch
  (device 7 = plain mouse leaves 180590/594 = 0). This doc reuses its
  §2 pose laws and does not re-trace the input device path.
- `docs/traces/mc2-cave-ceiling-sim.md` — the ceiling plane
  (`x_BYTE_14B4E0`), bit3 seal law, `sub_10C60` ceiling sampler, and the
  already-ported cave clamps. This doc traces how the flight mover
  *calls into* those; it does not re-trace the ceiling maintenance.
- `crates/mgc-sim/src/mc2/cave.rs` — `sub_11E20`/`cave_collide` (the
  384-margin collision primitive) is ALREADY ported; §2 here traces how
  the flight gate calls it, not the primitive itself.

---

## 0. Headline findings

1. **TRACE CORRECTION — the carpet's tuning row is 66 / 104, NOT 59.**
   Both the ROSTER survey (`SURVEY-MC2.md:448`, "wizard row0xa = 1792")
   and the constants sub-agent resolved the player's `str_D7BD6` row to
   index **59**. That is the GENERIC `NewEvent_4A050` default (Events.cpp:573)
   applied to every entity. `AddPlayer_4A920` (EF:33329-33332) then
   **explicitly overwrites** it: `&str_D7BD6[104]` on Cave maps, else
   `&str_D7BD6[66]`. The real carpet altitude constants are therefore:

   | field | offset | non-cave (row 66, L:78) | cave (row 104, L:116) | meaning |
   |---|---|---|---|---|
   | `word_160_0xa_10` | col5 0xa | `0x0400` = **1024** | `0x0C00` = **3072** | climb-ramp band / max-alt |
   | `word_160_0xc_12` | col6 0xc | `0x0100` = **256** | `0x0100` = **256** | ground clearance (floor) |
   | `word_160_0xe_14` | col7 0xe | `0xFFF0` = **−16** | `0xFFF8` = **−8** | buoyancy step |

   Row 59 (the mistaken value) has 0xa=`0x0700`=1792, 0xc=**0**, 0xe=−4.
   Using 1792 / clearance-0 in the port would put the soft ceiling too
   high AND drop the ground clearance to zero. **The band is 1024 above
   ground on open maps, 3072 in caves** (more headroom underground), and
   the carpet always keeps **256** clearance off the floor.

2. **The climb law is genuinely DIFFERENT from MC1** (confirmed, §1c).
   MC1 clamps `v5 = (z−ground−1024)` to ±256 and folds it into the polar
   step's *effective pitch* by `pitch·(−v5)/256`. MC2 does the SAME
   shape but the band constant is **`word_160_0xa_10` (row data)** and the
   ramp is normalized by it: `altDiff = ((z − ground − 0xa)·1024)/0xa`
   then clamped ±256 (EF:59645-59650). With row66 `0xa=1024` the divide
   is by 1024 so `altDiff ≈ (z−ground−1024)` — numerically MC1-like on
   open maps; in a cave `0xa=3072` stretches the band ×3. The pitch fold
   `pitch_0x24_36 = (tempPitch·−altDiff − sign·255) >> 8` (EF:59660/59665)
   is MC1's `(v6·−v5)/256` with a round-toward-zero bias. Result stored
   to a SEPARATE field `pitch_0x24_36` (the effective pitch), not back
   into aim.

3. **The vertical resolution is a two-part law** (EF:59745-59769, §3),
   NOT a single climb term. (a) The polar step applies the ramped
   `pitch_0x24_36` in `MoveEntity_57FA0`. (b) AFTER the commit gate, a
   post-move altitude clamp runs: mobilized → `z −= 51` (settle);
   else buoyancy `z += 0xe` while above `ground+0xc`; then hard floor
   clamp to `ground+0xc` if below, or cave-ceiling clamp to `ceiling−384`
   if above the clearance band. This is `sub_5D530`'s own vertical law —
   the "sub_5D530 branch order" from the prior note. **It only
   ceiling-clamps when z ≥ ground+clearance** (the `>= locAlt + 0xc`
   gate, EF:59757) — confirming the prior anchor.

4. **Speed model = MC1's, verbatim constants** (§4). Forward speed
   `speed_0xc_12` steps ±`speedIcrement_D4B84`=16/tick, clamps [−80,+80]
   (`x_DWORD_D4B88`/`D4B8C`), HOLDS on release (no stop key, no decay).
   `actSpeed_0x82_130` chases `speed_0xc_12` in ±16 sign steps
   (EF:59636-59644). Strafe `strafeSpeed_0x10_16` steps ±16
   (`x_WORD_D4BA4`), clamps [−80,+80] (`D4BA8`/`D4BAC`), decays −4
   (`x_WORD_D4BB0`) with a sign-flip snap on release (EF:60799-60849).
   These EQUAL MC1's 16/±80/−4 flight constants — the port can reuse the
   MC1 numbers directly.

5. **Extended channels MC1's carpet lacks** (§5): a **strafe** second
   polar step at `yaw+0x200` (yaw+90°); a **moveBoost** knockback impulse
   (cap 128, decays ±`moveBoosStep_D4B90`=−4, fired by hit-reaction and
   the possess-drag spell at ±80); a **slow** channel `moveSpeed_0x14C_332`
   (0..3, the spider-web SLOW) that scales roll/pitch delta AND actSpeed
   by `(4−moveSpeed)/4`; a **mobilize** channel `mobilizeCounter_0x14E_334`
   (the FULL-STOP web) that zeroes all speed and forces the −51 settle;
   and an external displacement **mailbox** `xAdd/yAdd/zAdd_0x1A6/8/A`
   added once then cleared (EF:59713-59718). None exist in `flight::mc1_move`.

6. **The commit gate zeroes TARGET speed on block** (EF:59602), unlike
   MC1 whose gate never touched speed. On any refusal (water dead-end,
   bit3 seal, cave both-sides-blocked) `moveTest_5D0A0` reverts position,
   sets `speed_0xc_12 = 0`, and clears the possess spell's `word_0x2E_46`
   (EF:59599-59605). The cave STEER-SEARCH (EF:59521-59591) tries the two
   diagonals off the intended bearing, picks the roomier UNSEALED one that
   passes `sub_11E20`, and turns yaw ±`(17·i)/6` toward it; only if BOTH
   fail does it refuse+stop.

7. **Water is MC2's hard barrier** (§2a, EF:59478-59511) — a wet
   predicted tile triggers two axis-aligned slide retries (project the
   move onto each cardinal), and if both land wet the move is refused.
   There is NO type-8 solid-wall gate for the player (unlike MC1's
   `sub_45410`) — walls are handled by the *terrain rising* + the cave
   bit3 seal, not a wall list. (SURVEY-MC2.md:426 called this "REWRITTEN"
   — confirmed.)

8. **Mouse pose integration is IN this function** (EF:59622-59652) and
   is identical to `mc2-mouse-aim.md` §2: roll/pitch delta accumulate
   into `roll_0x155_341`/`pitch_0x157_343`, yaw is a RATE
   `yaw += (roll_0x155 − sign·7)>>3`, pitch is ABSOLUTE
   `pitch_0x1E_30 = pitch_0x157 & 0x7FF`. Under the slow channel the delta
   is pre-scaled by `(4−moveSpeed)`. No correction needed to that trace.

9. **No collision damage, no bounce for the player** (§6). The carpet
   clamps at both floor and ceiling; it never takes contact damage from
   terrain and never reflects velocity (contrast m0-bob / rolling
   creatures which bounce). Damage/knockback arrives only via the hit
   mailbox (`str_0x5E_94`, EF:60671-60727) → the moveBoost impulse and
   the xAdd/yAdd/zAdd displacement. **stun/tint**: the tint is the SLOW
   palette-mod (`SetPaletteModification_5C830`, EF:60670/28415) driven by
   `moveSpeed`; the "stun" is the mobilize full-stop. Both are the
   `moveSpeed`/`mobilizeCounter` channels — the FlightVerb seam
   (proj.rs:292) should route incoming stun→`mobilizeCounter`,
   tint→`moveSpeed`.

---

## THE TICK: `sub_5D530` (EF:59610) statement order

Called once per entity-tick for the human (class 3 model 0) from
`AddPlayer03_00_5E010` (EF:59994) and `sub_5E7C0` (EF:60074). Full
statement order (the port must preserve it):

```c
// (pre) early-out: the "just teleported / frozen this tick" flag
if (byte[1] & 8) { byte[1] &= 0xF7; return; }          // EF:59616-59620
predictedAxis = position;                              // work on a copy

// (0) POSE INTEGRATION (mouse), slow-scaled  — see mc2-mouse-aim.md §2
if (moveSpeed_0x14C_332) {                             // SLOW active (0..3)
    t = rollDelta_0x4_4 * (4 - moveSpeed_0x14C_332);
    roll_0x155_341  += (t - sign(t)*3) >> 2;
    t = pitchDelta_0x6_6 * (4 - moveSpeed_0x14C_332);
    pitch_0x157_343 += (t - sign(t)*3) >> 2;
} else {                                               // normal
    roll_0x155_341  += rollDelta_0x4_4;                // EF:59631
    pitch_0x157_343 += pitchDelta_0x6_6;               // EF:59632
}
yaw_0x1C_28 = (yaw_0x1C_28 + ((roll_0x155_341 - sign*7) >> 3)) & 0x7FF;   // YAW RATE  EF:59635

// (1) ACTUAL SPEED chases TARGET in +-16 steps
sign = (speed_0xc_12 > actSpeed_0x82_130) ? 1 : (speed_0xc_12 != actSpeed_0x82_130 ? -1 : 0);
actSpeed_0x82_130 += sign * speedIcrement_D4B84;       // 16/tick   EF:59636-59644

// (2) CLIMB RAMP  -> effective pitch pitch_0x24_36
altDiff = ((predictedAxis.z - getTerrainAlt(pos) - word_160_0xa_10) << 10) / word_160_0xa_10;  // EF:59645
altDiff = clamp(altDiff, -256, 256);                   // EF:59647-59650
tempPitch = pitch_0x157_343 & 0x7FF;
pitch_0x1E_30 = tempPitch;                             // AIM pitch published (absolute)  EF:59651-59652
if (tempPitch > 1024) tempPitch -= 2048;               // signed
if (actSpeed >= 0 || tempPitch <= 0) {
    if (actSpeed < 0 && tempPitch < 0)  pitch_0x24_36 = pitch_0x157_343;              // reverse+climb: raw
    else if (actSpeed > 0 && tempPitch < 0) pitch_0x24_36 = (tempPitch*-altDiff - sign(tempPitch*-altDiff)*255) >> 8;  // fwd+climb: RAMPED
    else if (actSpeed > 0 && tempPitch > 0) pitch_0x24_36 = pitch_0x157_343;          // fwd+dive: raw
} else {                                                                              // reverse+dive: RAMPED
    pitch_0x24_36 = (tempPitch*-altDiff - sign(pitch_0x1E_30*-altDiff)*255) >> 8;
}
pitch_0x24_36 &= 0x7FF;                                 // EF:59666

// (3) forward polar step, slow-scaled
if (moveSpeed) locActSpeed = (actSpeed*(4-moveSpeed) - carry) >> 2;   // slow
else if (mobilizeCounter) locActSpeed = 0;                            // FULL STOP
else locActSpeed = actSpeed;
MoveEntity_57FA0(&predictedAxis, yaw_0x1C_28, pitch_0x24_36, locActSpeed);   // EF:59680

// (4) strafe polar step at yaw+90 deg, slow-scaled
if (strafeSpeed_0x10_16) {
    locStrafe = moveSpeed ? ((4-moveSpeed)*strafe - carry)>>2 : (mobilizeCounter ? 0 : strafe);
    MoveEntity_57FA0(&predictedAxis, yaw_0x1C_28 + 0x200, 0, locStrafe);    // EF:59693
}

// (5) knockback impulse along yaw_0x1E_30 (the hit bearing), decaying
if (moveBoost_0x1E_30) {
    if (moveBoost > 128) moveBoost = 128;                                   // cap
    MoveEntity_57FA0(&predictedAxis, yaw_0x1E_30, 0, moveBoost_0x1E_30);    // EF:59699
    moveBoost += sign(moveBoost) * moveBoosStep_D4B90;                      // decay by -4
    if (abs(moveBoost) < 4) moveBoost = 0;
}

// (6) external displacement mailbox (added once, cleared)
predictedAxis.x += xAdd_0x1A6_422;  predictedAxis.y += yAdd_0x1A8_424;  predictedAxis.z += zAdd_0x1AA_426;
xAdd = yAdd = zAdd = 0;                                                     // EF:59713-59718
if (waterCounter_0x262_610) waterCounter--;
sub_5DE30(a1x);                                        // (7) tornado/pull LEASH  (EF:59721, see below)

// (8) SLOW / MOBILIZE counter decrements
if (moveSpeed) { if (--moveSpeedCounter_0x14D_333 == 0) { if (--moveSpeed) {moveSpeedCounter=8; moveSpeedFlag=1;} else sub_5C800(a1x,1);} }
if (mobilizeCounter) { if (--mobilizeCounter2_0x150_336 == 0) mobilizeCounter--; }

// (9) COMMIT GATE + vertical resolution
if (moveTest_5D0A0(a1x)) {                              // EF:59745  (see section 2)
    locAlt = getTerrainAlt(&predictedAxis);
    if (mobilizeCounter) predictedAxis.z -= 51;         // settle while frozen  EF:59750
    else if (predictedAxis.z > locAlt + word_160_0xc_12) predictedAxis.z += word_160_0xe_14;  // BUOYANCY rise  EF:59755
    if (predictedAxis.z >= locAlt + word_160_0xc_12) {  // above the clearance band
        if (isCaveLevel) {                              // CEILING CLAMP (no bounce/damage)
            c = sub_10C60(&predictedAxis);
            if (predictedAxis.z > c - 384) predictedAxis.z = c - 384;      // EF:59762-59763
        }
    } else {
        predictedAxis.z = locAlt + word_160_0xc_12;     // FLOOR CLAMP to ground+clearance  EF:59768
    }
    CopyEntityPosition_57CF0(a1x, &predictedAxis);      // commit
} else {
    sub_5DD50(a1x);                                     // NUDGE out of the wall (see below)
}
// (10) trailing sound/music (cave ambient, water loop, building loops) EF:59776-59850 — not sim-critical
```

Notes on the vertical law (§3 detail):
- **`word_160_0xe_14 = −16` (open) / −8 (cave)** is the BUOYANCY step and
  it is NEGATIVE. The branch `predictedAxis.z += 0xe` runs only when
  `z > ground+clearance` — i.e. when ABOVE the clearance floor it eases
  the carpet DOWN by |0xe|/tick. This is the passive sink: MC2's carpet
  drifts toward `ground+clearance` at 16 (8 in caves) per tick when the
  pitch/speed aren't lifting it. (MC1 has an 8/tick sink only above the
  soft ceiling; MC2's is the row `0xe` step, always-on above clearance.)
- The floor clamp lands at **`ground + word_160_0xc_12` = ground+256**
  (MC1 clamps to ground+128). The MC2 carpet rides 256 units off the
  floor, not 128.
- The ceiling clamp is **`ceiling − 384`** (matches mc2-cave-ceiling-sim.md
  §2). No bounce, no damage. Only when `z ≥ ground+256` (so a carpet
  pinned to the floor is never yanked by the roof — the floor wins in a
  low-headroom pinch; this is the branch order the port's lib.rs
  ceiling clamp already mirrors, lib.rs:474-484).

---

## 1. THE POSE + CLIMB LAWS (details)

### 1a. Pose (mouse) — reuse `mc2-mouse-aim.md` §2
The delta write is at EF:38060-38066 (`PlayerEvents`, once/frame):
```c
rollEnv  = 2*playerInputs[i].roll  - roll_0x155_341;
rollDelta_0x4_4  = (rollEnv  - (sign(rollEnv)<<2)  + sign(rollEnv))  >> 2;   // ~= rollEnv/4 toward-zero
pitchEnv = 2*playerInputs[i].pitch - pitch_0x157_343;
pitchDelta_0x6_6 = (pitchEnv - (sign(pitchEnv)<<2) + sign(pitchEnv)) >> 2;
entityIndex_0x0  = playerInputs[i].entityIndex_0x6E3E_byte5;   // movement/fire BITFIELD (see §4)
nextEntity_0x18_24   = playerInputs[i].nextEntity_word6;       // Channel B yaw offset (=0 mouse)
entityIndex2_0x1A_26 = playerInputs[i].entityIndex2_word8;     // Channel B pitch offset (=0 mouse)
```
Then the per-tick integration is EF:59622-59652 (block (0)+(2) above).
Units 0..2048/turn (`& 0x7FF`). Identical to MC1's filter/rate/absolute
laws; the ONLY pose difference vs MC1 is the slow-scale `(4−moveSpeed)`
prefactor.

### 1b. `MoveEntity_57FA0` (Player.cpp:6) — the polar primitive
```c
void MoveEntity_57FA0(axis_3d* p, uint16 yaw, int16 pitch, int16 speed) {
    if (speed) {
        pitch &= 0x7FF; yaw &= 0x7FF;
        if (pitch) { p->z -= (speed*sin[pitch])>>16; speed = (speed*sin[0x200+pitch])>>16; }  // pitch first: z down, horiz *= cos
        p->x += (speed*sin[yaw])>>16;                // +sin
        p->y -= (speed*sin[0x200+yaw])>>16;          // -cos  (index+512 = cos)
    }
}
```
Same as the port's `Gen::polar_step`: `z -= s·sin(pitch)`, horizontal
scaled by `cos(pitch)`, `x += s·sin(yaw)`, `y −= s·cos(yaw)`. Pitch
applied BEFORE yaw (so climb steals horizontal speed exactly like MC1).

### 1c. The climb ramp — the piece that differs from MC1
EF:59645-59666 (block (2) above). The essential shape:
```
band       = word_160_0xa_10          # 1024 open, 3072 cave  (row 66/104)
altDiff    = clamp( (z - ground - band) * 1024 / band , -256, +256 )
# authority factor = -altDiff/256 in [-1, +1]:
#   z << band  -> altDiff = -256 -> factor +1  (full climb authority)
#   z == band  -> altDiff = 0    -> factor  0  (no climb, hover at band)
#   z >> band  -> altDiff = +256 -> factor -1  (INVERTED: pitch-up pushes down)
```
Fold into effective pitch (only when pitching TOWARD the band; pitching
AWAY passes raw):
```
pitch_0x24_36 = (tempPitch * -altDiff - sign(...)*255) >> 8   # = tempPitch * factor, round-toward-zero
```
The four-quadrant branch (EF:59655-59665) decides raw-vs-ramped by the
sign pairing of `actSpeed` and `tempPitch`:
- fwd + dive (`s>0, p>0`): RAW (dives always allowed).
- fwd + climb (`s>0, p<0`): RAMPED (climb authority-scaled).
- reverse + climb (`s<0, p<0`): RAW.
- reverse + dive (`s<0, p>0`): RAMPED.
This is MC1's `dive?raw:ramped` logic generalized to backwards flight.
**MC1 uses the fixed constant 1024 and divides by 256; MC2 uses the row
`0xa` for BOTH the band offset and the normalizer** — so in a cave the
whole authority window triples (you can climb to 3072 above the floor
before the ramp zeroes out, vs 1024 open). This is the "swappable climb
law" the survey (line 490) flagged; it is data-driven, not a constant.

---

## 2. THE COMMIT GATE — `moveTest_5D0A0` (EF:59429)

Runs on `predictedAxis` after all the polar steps. Returns true = commit.
**TRACE CORRECTION**: mc2-cave-ceiling-sim.md §4a labels this "the walker
steer-and-commit gate (verbs.rs)" — it is actually the PLAYER FLIGHT gate
(called only from `sub_5D530`). The creature walker gate is the separate
`sub_102D0`/`sub_1B8C0` path. The cave-steer logic described there is
correct; it just belongs to flight, not the creature walker.

### 2a. Water barrier (all levels) — EF:59478-59511
```c
if (terrain_is_water(&predictedAxis) == 256) {           // predicted tile is DEEP water (type 8)
    waterCounter_0x262_610++;
    // recover the intended bearing/dist, then SLIDE along the nearest cardinal:
    bearing = tan2(pos, predictedAxis);  dist = dist3d(...); azi = radix_tan(...);
    axis = pos;
    q = (bearing >> 9);                                  // snap bearing to the 4 cardinals (512 units each)
    MoveEntity(&axis, q<<9,      azi, dist*(512 - |bearing-(q<<9)|)/512 );   // slide on cardinal A
    if (terrain_is_water(&axis)==256) {
        axis = pos;
        MoveEntity(&axis, ((q+1)<<9)&0x7FF, azi, dist*(512 - |bearing-((q+1)<<9)|)/512 );  // slide on cardinal B
        if (terrain_is_water(&axis)==256) result = false;    // both wet -> BLOCKED
    }
    predictedAxis = axis;                                // committed slide
}
if (!isCaveLevel) return result;                         // non-cave: water is the ONLY gate
```
`terrain_is_water` (TR:2058) returns the tile-type bitmask; `==256` =
type-8 water (the barrier). `==1` (type 0 land) is used only for the
flight/water sound loop later. **There is no solid-wall list** — the only
XY blockers are water (open) and bit3-seal + `sub_11E20` (cave).

### 2b. Cave steer-search — EF:59515-59591
```c
tempAxis = predictedAxis;
headroom = word_160_0xc_12 + getTerrainAlt(tempAxis) + array_0x52_82.fov;   // occupied top
ceil     = sub_10C60(tempAxis);                          // ceiling sample
tile     = (tempAxis.x>>8) | (tempAxis.y>>8)<<8;
if (headroom < ceil - 576 && !(mapAngle[tile] & 8)) {
    /* plenty of room AND not sealed -> commit predictedAxis as-is */
} else {
    // widening probe: base bearing = tan2(pos, tempAxis);
    yawL = (bearing - 512) & 0x7FF_hi7;   yawR = (bearing + 2steps) & ...;   // two diagonals off the bearing
    for (i=0; i<6 && !found; i++) {
        r = 16*(i+1)+prev;                               // widening radius 16,32,64,...
        candL = tempAxis; MoveEntity(&candL, yawL, 0, r);   headroomL = sub_10C60(candL) - getTerrainAlt(candL);
        candR = tempAxis; MoveEntity(&candR, yawR, 0, r);   headroomR = sub_10C60(candR) - getTerrainAlt(candR);
        if (!(mapAngle[tileL]&8) || !(mapAngle[tileR]&8)) {
            if (headroomL > headroomR && !(mapAngle[tileL]&8) && !sub_11E20(a1x,&candL)) { pick=candL; side=1; }
            else if (headroomR > headroomL && !(mapAngle[tileR]&8) && !sub_11E20(a1x,&candR)) { pick=candR; side=2; }
        }
    }
    if (found) { predictedAxis = pick; yaw_0x1C_28 = (yaw + (side==1 ? -(17*i)/6 : (17*i)/6)) & 0x7FF; }  // TURN toward the open side
    else if (sub_11E20(a1x, &tempAxis)) result = false;  // both blocked -> REFUSE
    else predictedAxis = tempAxis;                       // neither better but not colliding -> commit straight
}
```
- `sub_11E20` = the 384-margin cave collision primitive (already ported,
  cave.rs). The gate calls it per candidate.
- Free-commit needs headroom `< ceiling − 576` AND target not bit3.
- The auto-steer turns yaw by `±(17·i)/6` (i = the probe iteration that
  found room) toward the clearer diagonal. This is a movement ASSIST
  unique to caves (MC1 has nothing like it).

### 2c. Final seal check + refusal — EF:59592-59605
```c
if (result && mapAngle[(predictedAxis.x>>8)|(predictedAxis.y>>8)<<8] & 8) result = false;  // landed on a sealed tile
if (!result) {
    predictedAxis = position;                            // revert XY+Z
    speed_0xc_12 = 0;                                    // ZERO TARGET SPEED  (EF:59602) -- MC1 never did this
    if (SpellEnabled[3]) Entities[SpellEnabled[3]]->word_0x2E_46 = 0;   // cancel the possess/leash spell
}
return result;
```
The `speed_0xc_12 = 0` on block is the "dead-stop into a wall" feel. Since
`actSpeed` chases `speed`, the carpet decelerates to rest over 5 ticks
after hitting a barrier (it does not instantly stop — `actSpeed` still
slews).

### 2d. `row156` / family indexing into the collide primitive
`sub_11E20` (EF:4620) reads `word_160_0xc_12` (clearance, EF:4625) and
tests floor+clearance+fov vs ceiling−384; the flight gate passes the
carpet entity `a1x` so the primitive reads the carpet's OWN row
(66/104). No family-table indexing happens in the flight path itself —
the "row156 per family" concern is a creature-mover detail; the player
always uses its single row. (If the port shares one collide fn across
MC1/MC2 movers, pass the carpet's clearance = 256.)

---

## 3. THE NUDGE — `sub_5DD50` (EF:59854)

Called ONLY when `moveTest_5D0A0` returned false (the carpet is stuck).
```c
stuck = (terrain_is_water(&position) == 256)                       // sitting in water
     || (isCaveLevel && mapAngle[tile] & 8);                       // OR sitting on a sealed tile
if (!stuck && isCaveLevel && byte_0x261_609 && sub_11E20(a1x,&position)) stuck = 1;  // OR already poking the ceiling and mid-nudge
if (stuck) {
    byte_0x261_609 = 1;                                            // latch "nudging"
    axis = position; MoveEntity(&axis, yaw_0x1C_28, 0, 128);       // shove 128 units forward along yaw
    CopyEntityPosition_57CF0(a1x, &axis);                          // commit the shove UNCONDITIONALLY (no gate!)
} else {
    byte_0x261_609 = 0;
}
```
The nudge is an un-gated 128-unit forward shove to push the carpet OUT of
a wall/water/ceiling it got wedged into (the gate refused the real move,
so this recovers). `byte_0x261_609` latches so the recovery persists until
clear. Port: when the gate refuses, apply the nudge instead of freezing.

---

## 4. SPEED MODEL & the command bitfield

### 4a. Command integration `sub_5F380` (EF:60748), called pre-move from
`AddPlayer03_00_5E010` (EF:59967). Reads `entityIndex_0x0` (the movement
bitfield copied from player inputs at EF:38064):
```c
word_0xe_14 = 0;
if (bits & 1 && speed_0xc_12 < D4B8C(80))  dir =  1;   // FORWARD
if (bits & 2 && speed_0xc_12 > D4B88(-80)) dir = -1;   // BACK
if (dir) { speed_0xc_12 += speedIcrement_D4B84(16)*dir; clamp[-80,80]; word_0xe_14=1; }   // EF:60783

sdir = (bits & 4) ? -1 : (bits & 8) ? 1 : 0;           // STRAFE L / R
if (sdir) { strafeSpeed_0x10_16 += D4BA4(16)*sdir; clamp[D4BA8(-80), D4BAC(80)]; }         // EF:60799
else if (strafeSpeed) { strafeSpeed += sign*D4BB0(-4); if sign flipped -> 0; clamp[-80,80]; }  // decay EF:60824-60849

if (bits & 0x10) sub_5F660(a1x, SpellEnabled[SpellIndexLeft], 256);    // FIRE LEFT  (hand offset 256)
if (bits & 0x20) sub_5F660(a1x, SpellEnabled[SpellIndexRight], 512);   // FIRE RIGHT (hand offset 512)
if (bits & 0x40) sub_5F660(a1x, SpellEnabled[spellIndex[..]], 256);    // CAST selected
```
Bit 0x80 (both-strafe "center/full-stop keybind", PI:2087) is consumed
upstream in the fly-assistant, not in `sub_5F380`. The command 0x27
"full-stop end-sequence" (SURVEY line 457) is the `sub_5E8C0` end-game
path (EF:60356-60359) that zeroes mobilize/moveSpeed/moveBoost/strafe —
NOT a normal-flight input.

### 4b. Movement bit map (PI:882-915, PI:2029-2087; via
`HandleButtonClick_191B0(6, bit)`):
| input bit | `entityIndex_0x0` bit | effect |
|---|---|---|
| arrow_keys & 1 | 0x01 | forward (throttle +) |
| arrow_keys & 2 | 0x02 | back (throttle −) |
| arrow_keys & 4 | 0x04 | strafe left |
| arrow_keys & 8 | 0x08 | strafe right |
| mouse L / fire-L | 0x10 | fire left-hand spell |
| mouse R / fire-R | 0x20 | fire right-hand spell |
| both mouse btns | 0x40 | cast the selected/centered spell |
| both strafe keys | 0x80 | center/full-stop (fly assistant) |

### 4c. Speed constants (all file-scope, no per-level scaling —
EF:1136-1143, agent-verified):
| symbol | value | role |
|---|---|---|
| `speedIcrement_D4B84` | 16 | forward accel step & actSpeed slew |
| `x_DWORD_D4B88` | −80 | min forward speed (full reverse) |
| `x_DWORD_D4B8C` | 80 | max forward speed; also `minSpeed_0x84_132` |
| `moveBoosStep_D4B90` | −4 | moveBoost decay step |
| `x_WORD_D4BA4` | 16 | strafe accel step |
| `x_DWORD_D4BA8` | −80 | min strafe |
| `x_DWORD_D4BAC` | 80 | max strafe |
| `x_WORD_D4BB0` | −4 | strafe decay step |

These are IDENTICAL to MC1's flight numbers (the port's `mc1_move` uses
16/±80/−4). So the SPEED half of the model is a straight reuse; only the
extended channels + climb law + gate differ.

---

## 5. EXTENDED CHANNELS (fields MC1's carpet lacks)

### 5a. Strafe `strafeSpeed_0x10_16` (dword_0xA4_164 +0x10)
Set by `sub_5F380` (§4a). Consumed as a second polar step at yaw+0x200
(yaw+90°, EF:59693). MC1 HAS strafe (`flight::mc1_move` st.strafe) with
the SAME 16/±80/−4 numbers — so this one is already in the MC1 arm; the
only MC2 twist is the `(4−moveSpeed)` slow-scale (EF:59685-59687).
Reset to 0 at EF:60359 (end-seq) and EF:43718 (respawn).

### 5b. moveBoost `moveBoost_0x1E_30` (+0x1E) + bearing `yaw_0x1E_30`
Knockback impulse. Written by:
- Hit reaction `sub_5EFA0` EF:60701: `moveBoost = str_0x5E_94.dmg/10`,
  clamp [0,80]; bearing `yaw_0x1E_30 = tan2(attacker, me)` and
  `fov_0x22_34 = radix_tan(...)` (EF:60699-60700).
- Possess-drag spell `sub_38E70`/`sub_38F70` EF:28411/28437: `= −80`
  (pulls the victim toward the caster).
Consumed EF:59695-59711: capped 128, applied along `yaw_0x1E_30`, decays
by `moveBoosStep_D4B90`=−4/tick toward 0, snaps to 0 below |4|. The camera
also reads it (EF:40261). MC1 has NO knockback channel in the mover (the
MC1 port takes `knock` as a caller-supplied buffet param instead). Reset
0 at EF:60264/60358/43717.

### 5c. Slow `moveSpeed_0x14C_332` (+0x14C), counter `+0x14D`
The spider-web SLOW (0..3). Set by `sub_38E70` (EF:28407-28417): each hit
`++moveSpeed` up to 3, sets a red palette-mod `171*moveSpeed/3+85`
(EF:28415, the TINT), `moveSpeedCounter=8`. Decremented in the tick
(EF:59722-59737): every 8 ticks `moveSpeed--`; at 0 calls `sub_5C800(a,1)`
(clear). While active it scales the pose delta (§0 block 0), the forward
speed and the strafe by `(4−moveSpeed)/4` — so moveSpeed=3 → quarter
speed, moveSpeed=1 → three-quarter. Palette re-applied each tick when
`moveSpeedFlag_181` set (EF:60669-60670). **This is the "tint" channel.**

### 5d. Mobilize `mobilizeCounter_0x14E_334` (+0x14E), counter `+0x150`
The FULL-STOP web (freeze). Set by `sub_38F70` (EF:28442-28443):
`mobilizeCounter=1, mobilizeCounter2=10`, and `moveBoost=−80`. Decremented
EF:59739-59743: every 10 ticks `mobilizeCounter--`. While active:
forward + strafe locActSpeed forced 0 (EF:59674/59690), and the post-gate
vertical does `z −= 51` (settle to ground, EF:59750). Also draws the
spider-web screen overlay (EF:21668). **This is the "stun" channel.**
Reset 0 at EF:60356. (proj.rs:292's stun/tint seam → route incoming
stun to `mobilizeCounter`, tint/slow to `moveSpeed`.)

### 5e. Displacement mailbox `xAdd/yAdd/zAdd_0x1A6/8/A_422/424/426`
One-shot world-space displacement added at EF:59713-59715 then zeroed.
External systems (wind, quake shove, teleport nudge) write these; the
mover applies once. MC1 has no such mailbox.

### 5f. Water counter `waterCounter_0x262_610` (+0x262)
Incremented in the gate on a wet predicted tile (EF:59480), decremented
in the tick (EF:59719-59720). Gates the water-flight sound + likely a
splash/drag; not a movement force itself.

---

## 6. `sub_5DE30` (EF:59889) — the leash/tractor (tornado-grab)

Runs every tick (EF:59721) BEFORE the gate. If the carpet is grabbed
(`word_0x146_326` != 0 = the grabber entity, set by `sub_5EFA0`
EF:60648) and the grabber's leash spell (`SpellEnabled[14]`) is live and
within `subspell.subSpellIndex_2` range:
```c
v12  = 3*minSpeed_0x84_132/2;                            // = 120 (minSpeed 80)
pull = (dist - dword_0x142_322) / (1024 / v12);          // proportional to over/under leash length
clamp pull to +-v12;
bearing = tan2(me, grabber);
yaw_0x1C_28 += sub_58350(yaw, bearing, 5, 0x82);         // turn toward grabber (rate 5, cap 130)
MoveEntity(&predictedAxis, bearing, pitch_0x1E_30, pull);// step toward/away to hold the leash length
// leash tiers drain the grabber's mana/life (life_0x1A subspell): tier>=1 drains mana, tier 2 drains life
```
`dword_0x142_322` = the target leash length (clamped 1024..3072 at
`sub_5EFA0` EF:60652-60656). The carpet is reeled to hold that distance.
This is the possess/tornado "grip" mechanic; it moves the carpet on top
of the player's own inputs. Port: an optional pre-gate pull step keyed on
the grab field.

---

## 7. FIELD HOMES (decompile name → offset → meaning)

Player entity `type_entity_0x6E8E` (fields off the entity) and its
`dword_0xA4_164x` sub-struct (the per-player mutable state):

| name | home | units | meaning | key sites |
|---|---|---|---|---|
| `yaw_0x1C_28` | entity +0x1C | 11-bit | heading (integrated rate) | EF:59635, MoveEntity |
| `pitch_0x1E_30` | entity +0x1E | 11-bit | published AIM pitch (absolute) | EF:59652 (write), cast §mouse-aim |
| `actSpeed_0x82_130` | entity +0x82 | u/tick | actual forward speed (chases target) | EF:59636-59680 |
| `minSpeed_0x84_132` | entity +0x84 | =80 | base speed (from D4B8C) | EF:33326, 59918 |
| `array_0x52_82.fov` | entity +0x52 | u | head clearance (collide margin) | EF:59517, cave |
| `str_0x5E_94` | entity +0x5E | — | incoming-hit mailbox (dmg/attacker) | EF:60671-60727 |
| `word_0x146_326` | 164 +0x146 | idx | leash grabber entity | EF:59906, 60648 |
| `dword_0x142_322` | 164 +0x142 | u | leash target length (1024..3072) | EF:59919, 60649-56 |
| `roll_0x155_341` | 164 +0x155 | 11-bit acc | roll accumulator (yaw-rate source, camera bank) | EF:59631-59635 |
| `pitch_0x157_343` | 164 +0x157 | 11-bit acc | pitch accumulator (aim source) | EF:59632-59651 |
| `rollDelta_0x4_4` | 164 +0x04 | — | filtered roll delta | EF:38061, 59624/59631 |
| `pitchDelta_0x6_6` | 164 +0x06 | — | filtered pitch delta | EF:38063, 59626/59632 |
| `pitch_0x24_36` | 164 +0x24 | 11-bit | EFFECTIVE pitch (ramped) → polar step | EF:59658-59680 |
| `speed_0xc_12` | 164 +0x0c | u/tick | TARGET forward speed (throttle) | EF:60783, 59602 (zeroed on block), 59636 |
| `strafeSpeed_0x10_16` | 164 +0x10 | u/tick | strafe speed | EF:60799, 59681-59693 |
| `moveBoost_0x1E_30` | 164 +0x1e | u/tick | knockback impulse (decaying) | EF:59695-59711, 60701 |
| `yaw_0x1E_30` | 164 +0x1e* | 11-bit | knockback BEARING (*aliased name; distinct field from moveBoost — see note) | EF:59699, 60699 |
| `fov_0x22_34` | 164 +0x22 | 11-bit | knockback pitch bearing | EF:59929, 60700 |
| `entityIndex_0x0` | 164 +0x00 | bitfield | movement/fire command bits | EF:38064, 60776-60857 |
| `nextEntity_0x18_24` | 164 +0x18 | 11-bit | Channel B yaw offset (=0 mouse) | EF:38065, 55867 |
| `entityIndex2_0x1A_26` | 164 +0x1a | 11-bit | Channel B pitch offset (=0 mouse) | EF:38066, 55868 |
| `moveSpeed_0x14C_332` | 164 +0x14c | 0..3 | SLOW level (tint) | EF:28407-28415, 59622-59692 |
| `moveSpeedCounter_0x14D_333` | 164 +0x14d | ticks | slow decay counter (8) | EF:59724-59730 |
| `mobilizeCounter_0x14E_334` | 164 +0x14e | flag | FULL-STOP (stun) | EF:28442, 59672-59750 |
| `mobilizeCounter2_0x150_336` | 164 +0x150 | ticks | mobilize decay counter (10) | EF:59741-59743 |
| `xAdd_0x1A6_422` | 164 +0x1a6 | u | displacement mailbox X | EF:59713-59716 |
| `yAdd_0x1A8_424` | 164 +0x1a8 | u | displacement mailbox Y | EF:59714-59717 |
| `zAdd_0x1AA_426` | 164 +0x1aa | u | displacement mailbox Z | EF:59715-59718 |
| `waterCounter_0x262_610` | 164 +0x262 | ticks | in-water counter | EF:59480, 59719 |
| `byte_0x261_609` | 164 +0x261 | flag | "nudging out of wall" latch | EF:59869-59881 |
| `word_160_0xa_10` | row +0x0a | u | climb band / max-alt (1024 open / 3072 cave) | EF:59645, 5482 |
| `word_160_0xc_12` | row +0x0c | u | ground clearance (256) | EF:59754-59768, 4625 |
| `word_160_0xe_14` | row +0x0e | u | buoyancy step (−16 open / −8 cave) | EF:59755 |

Note on the `moveBoost`/`yaw_0x1E_30` name collision: the decompile prints
both as "+0x1E" but they are different accesses — `moveBoost_0x1E_30` is
read as the impulse magnitude (EF:59695-59711) and `yaw_0x1E_30` as the
bearing (EF:59699); treat `moveBoost` (magnitude) and `moveBoostBearing`
(=yaw_0x1E_30) as two fields when porting. (The decompiler's field
numbering is ambiguous here; the SEMANTICS are unambiguous from the call
sites.) [OPEN-1: confirm the true byte offsets against global_types.h.]

---

## 8. DIFFERENCES VS MC1 FLIGHT (the port delta)

| aspect | MC1 (`flight::mc1_move`) | MC2 (`sub_5D530`) |
|---|---|---|
| pose (yaw rate / abs pitch) | same filter/rates | SAME + `(4−moveSpeed)` slow-scale on delta |
| speed step / clamp | ±16 / ±80, holds on release | IDENTICAL (same D4B* constants) |
| actSpeed slew | ±16 sign step | IDENTICAL |
| strafe | ±16/±80/−4 at yaw+90° | SAME + slow-scale |
| climb law | `v5=(z−ground−1024)` clamp±256, fold `pitch·−v5/256` | `altDiff=((z−ground−0xa)·1024)/0xa` clamp±256, fold `pitch·−altDiff/256` — **band = row 0xa (1024 open / 3072 cave)** |
| ground clearance (floor) | ground **+128** | ground **+256** (row 0xc) |
| passive sink | 8/tick only ABOVE soft ceiling | row **0xe** (−16 open / −8 cave) whenever above clearance |
| ceiling | (n/a on open) / cave clamp −384 | cave clamp **ceiling−384** (only when z≥ground+256) |
| wall gate | `sub_45410` solid-wall list | **NO wall list**; water barrier + cave bit3/`sub_11E20` steer |
| block → speed | gate never touches speed | **zeroes TARGET speed** (EF:59602) + cancels possess |
| stuck recovery | (revert, freeze) | **`sub_5DD50` 128-unit forward nudge** |
| cave steer-assist | none | **6-step widening diagonal probe, auto-turns yaw** |
| knockback | caller-supplied `knock` param | **`moveBoost` channel** (hit/possess, cap 128, decay −4) |
| slow/stun | none | **`moveSpeed` (tint) + `mobilizeCounter` (stun)** channels |
| displacement mailbox | none | **xAdd/yAdd/zAdd** one-shot |
| leash/tractor | none | **`sub_5DE30` grab reel** |
| flutter RNG (sound 46) | every 64th tick LCG roll | (MC2 replaces with cave-ambient / water-loop sounds, EF:59800-59818) |

**Bottom line for the port:** keep the MC1 speed+strafe+pose halves
verbatim (constants match); swap in (1) the row-`0xa` climb ramp, (2) the
ground+256 floor / row-`0xe` sink / ceiling−384 vertical resolution, (3)
the water/cave commit gate with speed-zeroing + nudge, (4) the four new
channels (moveBoost, moveSpeed, mobilizeCounter, xyzAdd) and the leash.

---

## 9. INTERACTION WITH ALREADY-PORTED PIECES

- **Cave ceiling clamp** (`mc2-cave-ceiling-sim.md` §2, lib.rs:474-484):
  the port's `player_cave_ceiling` clamp at `ceiling−384` with the
  `floor` (ground+128) max is the right shape, but the FlightVerb::Mc2
  floor is **ground+256** (row 0xc), not +128 — when the Mc2 arm lands,
  the clamp's floor-max must become ground+256 to match `sub_5D530`. The
  branch-order (floor wins in a pinch) is already correct.
- **`sub_11E20` / cave_collide** (cave.rs): the flight gate calls it
  per steer-probe candidate and once on the straight move; pass the
  carpet entity so it reads clearance 256. Already ported as the
  primitive; the flight arm just needs to CALL it in the §2b loop.
- **Extended-lift enhancement** (lib.rs:437-467): this is a DEVIATION
  layered outside `mc1_move`. On the Mc2 arm it should layer outside
  `mc2_move` the same way (vertical only, respects the ceiling). The
  faithful Mc2 vertical already sinks via row 0xe, so the "passive sink
  toward floor" the extended-lift adds (lib.rs:458-466) is partly
  redundant with row 0xe — decide whether ExtendedLift replaces or
  augments the row-0xe sink for the Mc2 arm.
- **`mc1_move` commit gate seam** (lib.rs:411 `player_wall_gate_fixed`):
  the Mc2 arm needs a DIFFERENT gate (`moveTest_5D0A0`, water+cave, no
  wall list) — a new `player_water_cave_gate` returning the steered
  candidate OR None (→ nudge). The verb telemetry at lib.rs:377-382
  already flags `FlightVerb::Mc2` / `CommitGateVerb::Mc2` fallbacks.
- **mouse-aim** (`mc2-mouse-aim.md`): the pose block (0) IS this trace's
  EF:59622-59652 — the port's pose integration is shared; the Mc2 arm
  only adds the `(4−moveSpeed)` prefactor.
- **proj.rs:292 stun/tint seam**: §5c/5d identify the retail homes —
  route stun → `mobilizeCounter` (full-stop + −51 settle + web overlay),
  tint → `moveSpeed` (0..3 slow + red palette-mod).

---

## 10. PORT WORKLIST (ordering)

1. **Tuning row**: bake `str_D7BD6[66]` (open) and `[104]` (cave) into
   the MC2 carpet params: `climb_band = 1024/3072`, `clearance = 256`,
   `buoyancy = −16/−8`. Select by `isCaveLevel` at spawn (mirror
   `AddPlayer_4A920`). **Do NOT use row 59.**
2. **`mc2_move` skeleton** = clone `mc1_move`, keep speed/strafe/pose;
   replace the climb block with the row-`0xa` ramp (§1c) writing an
   effective pitch; polar-step with it.
3. **Vertical resolution** (§3): after the gate, mobilize→−51,
   buoyancy `+0xe` above clearance, floor clamp ground+256, cave ceiling
   clamp ceiling−384 (reuse `player_cave_ceiling`, fix floor-max to +256).
4. **Commit gate** `moveTest_5D0A0` (§2): water two-slide barrier (open),
   cave steer-search (bit3 + `sub_11E20`, auto-turn), final seal check,
   speed-zero + possess-cancel on refusal; nudge (`sub_5DD50`) when
   refused. Wire as `CommitGateVerb::Mc2`.
5. **Command integrator** `sub_5F380` (§4): the bit map + speed/strafe
   stepping (numbers already match MC1) + fire/cast dispatch (already
   ported via the cast column — reuse).
6. **Extended channels** (§5): moveBoost (hit/possess knockback), the
   xyzAdd mailbox — cheap, additive. Slow/mobilize (§5c/5d) wire to the
   proj.rs stun/tint seam.
7. **Leash** `sub_5DE30` (§6): last — only matters once possess/tornado
   grab spells target the player.

---

## OPEN items

- **OPEN-1**: `moveBoost_0x1E_30` vs `yaw_0x1E_30` both print as "+0x1E".
  Semantics are clear (magnitude vs bearing) but the true distinct byte
  offsets need a global_types.h cross-check before baking the struct.
- **OPEN-2**: the water-slide azimuth term `azi = radix_tan(pos,pred)`
  (EF:59483, passed as MoveEntity pitch) — whether the slide preserves
  the vertical component or flattens it wasn't reduced to a clean form;
  verify against a live over-water skim.
- **OPEN-3**: `array_0x52_82.fov` (head clearance) value for the carpet
  — used in the cave headroom test (EF:59517) and the collide primitive;
  not extracted here (set at `SetEntityIndexAndRot_49CD0(event,44)`
  EF:33334). Pull it before the cave steer-search port.
- **OPEN-4**: the flutter-RNG divergence — MC1 rolls sound-46 every 64th
  tick (a state mutation the MC1 port replicates). MC2's `sub_5D530`
  instead runs cave-ambient / water-loop sound logic (EF:59800-59818)
  with its OWN LCG draw (EF:59802). If MC1 and MC2 goldens must share an
  RNG stream, confirm which draw the MC2 tick makes.
- **OPEN-5**: SURVEY-MC2.md:448 says "wizard row0xa = 1792" — this doc's
  §0.1 corrects it to 1024/3072 (rows 66/104). Update the survey row and
  any code that hard-coded 1792.
