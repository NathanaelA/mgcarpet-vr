# MC2 MOUSE FREE-AIM — Verbatim Trace (raw mouse → view + launch angle)

All `file:line` citations relative to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
Files: `EventsFunctions.cpp` (EF), `Events.cpp` (E), `PlayerInput.cpp` (PI), `Sound.cpp` (S),
`Level.cpp` (L), `sub_main_old.cpp`, `EventsFunctions.h`, `global_types.h`.

Cross-refs (do NOT re-derive):
- `docs/traces/mc2-cast-input.md` — button bitfield / cadence / charge / §4 flagged the WRITE side of
  `x_DWORD_180590/594` as OPEN. **THIS doc closes that OPEN item** and traces the whole aim path.
- `docs/traces/mc2-autoaim.md` — projectile-side one-shot acquisition (`sub_67CB0`), the scorers, the
  "no HUD reticle" finding, `sub_68E50` = muzzle-position (not aim) helper.
- `docs/traces/mc2-player-cast-path.md` — the cast gate + `sub_6DCA0` spawn + the effect-state skeleton.

---

## 0. TL;DR — the aim laws (READ THIS FIRST)

**The headline correction to the playtest hypothesis:** the mouse-offset globals
`x_DWORD_180590` (yaw) / `x_DWORD_180594` (pitch) — which become the player-record fields
`nextEntity_0x18_24` / `entityIndex2_0x1A_26` that are added at launch — are **only written by the
PERIPHERAL device paths** (VR head-tracker `sub_8B790`, VFX/CyberPuck `sub_75910`, and the
joystick post-scale at EF:49687-49688). **For the plain 2-D mouse (device 7, the default) they are
NEVER written each frame and stay 0** (§1). So on a normal mouse the launch offset term is **zero** —
the projectile launches on the wizard's **pose** (`yaw_0x1C_28` / `pitch_0x1E_30`), and the ENTIRE
aim is the wizard's pose, which the mouse drives through a *different* channel (§2).

1. **Two independent mouse channels.** `ComputeMousePlayerMovement_17060(x,y)` (PI:2100) turns the
   **absolute mouse position** (offset from screen center, clamped ±127) into `playerInputs.roll` /
   `.pitch` (PI:2114-2138). SEPARATELY it copies `x_DWORD_180590/594` into
   `playerInputs.nextEntity_word6/word8` (PI:2139-2140). Channel A (`roll`/`pitch`) **steers the
   wizard's body**; channel B (`180590/594`) is a **free-look/aim offset added on top** — but B is
   only nonzero on peripherals.

2. **Channel A → wizard pose (the real aim on a mouse).** `roll`→`rollDelta_0x4_4`,
   `pitch`→`pitchDelta_0x6_6` (EF:38060-38063, a spring-damped delta toward `2·roll`). Then per tick
   in `sub_5D530` (EF:59610): `roll_0x155_341 += rollDelta`, `pitch_0x157_343 += pitchDelta`
   (EF:59631-59632), and **`yaw_0x1C_28 += (roll_0x155_341 − sign·7) >> 3`** (EF:59635) — mouse-X
   **turns the wizard's yaw** (a rate, integrating), while **`pitch_0x1E_30 = pitch_0x157_343 & 0x7FF`**
   (EF:59651-59652) — mouse-Y **sets the wizard's pitch directly** (absolute, wrapped to 0..2047).
   Units: **0..2048 per full turn** (`& 0x7FF`). So on a plain mouse, aim = wizard pose = X-offset
   integrated into yaw + Y-offset mapped to pitch.

3. **Channel B (180590/594) drives BOTH the camera AND the projectile — a shared free-look offset.**
   The camera rotation is `view.yaw = wizard.yaw` **`+ nextEntity_0x18_24`** (EF:40256, 40273) and
   `view.pitch = wizard.pitch_0x157_343/2 + …` **`+ entityIndex2_0x1A_26`** (EF:40267, 40274). The
   projectile launch is `proj.yaw = nextEntity_0x18_24 + wizard.yaw` / `proj.pitch =
   entityIndex2_0x1A_26 + wizard.pitch` (EF:55867-55868). **Same offset, same sign, added to both** —
   the view and the shot share the aim offset; the projectile does NOT diverge from the view. There is
   NO separate aim-vs-view offset. (This is the free-look on a head-tracker/joystick; on a mouse the
   offset is 0 and both view and shot follow the pose.)

4. **No on-screen cursor/crosshair in flight.** Confirmed by `mc2-autoaim.md` §4 (no HUD reticle) and
   here: nothing draws a moving cursor at the 180590/594 offset in flight view — the offset is a pure
   angular quantity, never a screen position. The only in-flight "aim" sprite is the local projectile
   sprite `SetEntityIndex_49C90(proj, 42)` (EF:55877-55878). Mouse *position* (`x_DWORD_1805B0_mouse`)
   is used only for MENU/HUD hit-testing (spellbook, map), never as a flight cursor.

5. **Fireball 34-cap initial turn (EF:63106-63119, verbatim §5).** On the FIRST flight tick, if
   `sub_67CB0` finds a target, the projectile turns toward it capped at **34 angle-units on YAW only**,
   one-shot: `v3 = clamp(sub_582B0(yaw, roll), 0, 34); yaw += v3 · sub_582F0(yaw, roll)`. **Pitch is
   snapped, not capped**: `pitch_0x1E_30 = fov_0x22_34` (the desired pitch). With **no** target it
   locks straight ahead: `roll = yaw; fov = pitch` (EF:63108-63109) — no turn at all.

6. **Complete list of terms in a player-cast projectile's initial yaw/pitch** (§6):
   `proj.yaw = wizard.yaw_0x1C_28 + nextEntity_0x18_24` (offset=0 on mouse);
   `proj.pitch = wizard.pitch_0x1E_30 + entityIndex2_0x1A_26` (offset=0 on mouse);
   PLUS (charged lightning tier-2 only) a **±113** yaw fork (EF:56631/56633);
   PLUS (fireball/most flight states) the **one-shot ≤34 yaw turn toward autoaim** on tick 1 (§5).
   **There is NO random spread, NO per-shot RNG, and NO ±512 hand-side term in the launch ANGLE** —
   the ±512 hand offset is a muzzle-POSITION step in `sub_68E50` (autoaim trace §7), not an angle.

**Port implication.** If the port launches on wizard-pose only, that is **correct for a plain mouse**
(offset term = 0). If aiming "feels off", the bug is almost certainly in **Channel A** — the pose
integration: mouse-X must integrate into yaw as a *rate* (`>> 3` of accumulated `roll_0x155`), and
mouse-Y must map to pitch *absolutely* (`pitch_0x157 & 0x7FF`), with the spring-damped
`rollDelta`/`pitchDelta` from `2·roll − roll_0x155`. A port that turns yaw *absolutely* from the mouse,
or that omits the `/2` and damping, will feel wrong. See §2 for the exact laws. (Also verify the port
isn't reading raw mouse *deltas* — retail uses the **absolute** screen-center offset, §1.2.)

---

## 1. WHERE `x_DWORD_180590` / `x_DWORD_180594` GET WRITTEN

### 1.1 The device switch — `ReadGameUserInputs_89D10` (EF:49586)

Per-frame input read dispatches on the active device `x_WORD_1805C2_joystick` (EF:49653):

| case | device | writes 180590/594? | where |
|---|---|---|---|
| 1, 8, 12 | i-Glasses / VR head-tracker | **YES** via `sub_8B790` (EF:49659) then `*4` / `<<11/360` post-scale (EF:49687-49688) | §1.3 |
| 2, 9, 13 | VFX1 CyberPuck | **YES** via `sub_75910` (EF:49698) + `180590 = -2048·raw/0xFFFF & 0x7FF` (EF:49726), `180594=0` (EF:49733) | §1.4 |
| 4 | (stub) | no | LABEL_54 |
| 6 | (stub) | reads `xx_array_E36C4[4]/[8]` (EF:49743-49745) | — |
| **7** | **plain 2-D mouse (DEFAULT)** | **falls to `default: break`** — 180590/594 **NOT written** | — |

**Device 7 is the default mouse** (selected by the menu at `MenusAndIntros.cpp:5095` and the fallback
`x_WORD_1805C2_joystick = 7` at `MenusAndIntros.cpp:5236`; `sub_main_old.cpp:306-309` documents case 7
= the only live case). In `ReadGameUserInputs_89D10`'s switch the live cases are **1/8/12, 2/9/13, 4, 6**
— **there is no `case 7`**, so device 7 hits `default: break` (EF:49563-49564 in the sibling
setting-applier; the read-switch has no case 7 either → default). **Therefore on a plain mouse,
`x_DWORD_180590/594` are only ever set to 0** (at device-init / setting-apply, EF:49504-49505;
`sub_89B60_aplicate_setting`) and **never updated per frame**. They remain **0** throughout mouse play.

> VERIFICATION: the only per-frame writers of 180590/594 are EF:49687-49688 (joystick post-scale, inside
> case 1/8/12), EF:49726/49733 (case 2/9/13), EF:49743-49745 (case 6), EF:50563-50589 (`sub_8B790`
> head-tracker), and the zero-inits at EF:49504-49505 / `sub_main_old.cpp:270-271`. `grep` for
> `180590 =` / `180594 =` returns exactly this set — none in a plain-mouse code path.

### 1.2 The mouse position → `roll`/`pitch` (Channel A) — `ComputeMousePlayerMovement_17060` (PI:2100)

This is what the plain mouse actually feeds. Called every flight tick with the **current absolute mouse
position** (`ComputeMousePlayerMovement_17060(unk_18058Cstr.x_DWORD_1805B0_mouse.x, .y)`, PI:643 /
PI:1007; the recorded-input path passes `position_backup_20.x/y` at PI:925). VERBATIM (PI:2105-2141):
```c
void ComputeMousePlayerMovement_17060(int16_t x, int16_t y) {
    if (CommandLineParams.DoMouseOff2()) { x = 0x140; y = 0xc8; }   // 320,200 = dead center
    if (!x_D41A0_BYTEARRAY_4_struct.speedIndex) {
        if (x_WORD_180660_VGA_type_resolution == 1) {               // 320x200 mode
            roll  = ((x << 7) - 40960) / 320;                       // (x·128 − 160·128)/160
            pitch = ((y << 7) - 25600) / -200;                      // (y·128 − 100·128)/-100 … note /-200
        } else {
            roll  = ((x << 7) - ((gameResWidth  / 2) << 7)) / (gameResWidth  / 2);   // offset from center X
            pitch = ((y << 7) - ((gameResHeight / 2) << 7)) / -(gameResHeight / 2);  // offset from center Y (neg)
        }
        if (roll  < -127) roll  = -127;   if (roll  > 127) roll  = 127;   // clamp ±127
        if (pitch < -127) pitch = -127;   if (pitch > 127) pitch = 127;
        if (!invertYAxis) pitch = pitch * -1;                      // default: flip Y back to +
        if ( invertXAxis) roll  = roll  * -1;
        playerInputs[LevelIndex].roll  = roll;                     // Channel A: view/turn intent
        playerInputs[LevelIndex].pitch = pitch;
        playerInputs[LevelIndex].nextEntity_0x6E3E_word6   = x_DWORD_180590;   // Channel B: aim offset (=0 on mouse)
        playerInputs[LevelIndex].entityIndex2_0x6E3E_word8 = x_DWORD_180594;   //   (=0 on mouse)
    }
}
```

**Units/scale/semantics of Channel A (`roll`/`pitch`):**
- **Absolute offset from screen center**, NOT a per-tick delta. `roll = (x − centerX)·128 / (width/2)`.
  So at the horizontal screen edge `roll = ±127` (`(width·128/2)/(width/2) = 128`, clamped to 127); at
  center `roll = 0`. Same for `pitch` from y, **negated** (mouse-down = look-down by default), with
  `invertYAxis`/`invertXAxis` options.
- Range **±127**. This is a **normalized aim-stick deflection**, not an angle in 2048 units yet — it
  becomes an angle only after the pose integration (§2).
- **No decay/recentering of `roll`/`pitch` themselves** — they are recomputed from scratch each frame
  from the live mouse position. The decay/spring lives downstream in the pose integrator (§2).
- **`DoMouseOff2()`** debug flag forces center (320,200) → zero aim.
- **Guard:** only runs when `speedIndex == 0` (not in some menu/pause substate).

### 1.3 Head-tracker path (device 1/8/12) — `sub_8B790` (EF:50534) + post-scale (EF:49687-49688)

For completeness (this is where 180590/594 ARE nonzero). `sub_8B790` reads the tracker's angular
registers, applies `flt_D1F40` scale, and does **recentering on key 0x2E** (EF:50570-50575: latch
`1805A4/A8/AC = 180590/594/598`) then **subtracts the latched center** each frame
(EF:50581-50585) — i.e. an **absolute angle minus a recenter datum**, then negates both
(EF:50587-50589). After return, EF:49687-49688 rescales: `x_DWORD_180594 *= 4;
x_DWORD_180590 = (x_DWORD_180590 << 11) / 360` — **converts the yaw from degrees to 2048-units**
(`<<11 / 360` = `×2048/360`), and multiplies pitch ×4. So on a head-tracker, 180590 is a **2048-unit
yaw offset** and 180594 a scaled pitch offset — a genuine free-look that both the view and the shot
inherit (§3). **On a plain mouse none of this runs.**

### 1.4 VFX/CyberPuck path (device 2/9/13) — `sub_75910` (EF:45584) + EF:49726/49733

`x_DWORD_180590 = -2048 * x_WORD_17D6CCar[0] / 0xFFFFu & 0x7FF;` (EF:49726) — a full 2048-unit yaw from
the puck's 16-bit axis, **and `x_DWORD_180594 = 0`** (EF:49733, pitch offset forced 0 for this device).
Again, not the mouse.

### 1.5 Summary answer to Q1
`x_DWORD_180590/594` = **an absolute free-look angular offset from "straight ahead", in 2048-unit yaw
(post-scale) and a scaled pitch**, **written only by peripheral devices** (head-tracker recenters on
key 0x2E; no clamp beyond the device masks; no per-frame decay — it is the live tracker reading minus a
recenter datum). **The plain 2-D mouse leaves them 0.** The mouse instead feeds **Channel A**
(`roll`/`pitch`, absolute ±127 screen-center offset), which drives the wizard's pose (§2). Mouselook vs
pointer mode **does** matter: the "mouselook" here is the peripheral free-look via 180590/594; the plain
mouse is always "pose-steer" via roll/pitch.

---

## 2. CHANNEL A → THE WIZARD POSE (the real mouse aim)

### 2.1 `roll`/`pitch` → `rollDelta`/`pitchDelta` (EF:38060-38066)

In the input-dispatch (`sub_5DFB0`-adjacent), VERBATIM:
```c
int rollEnv = 2 * playerInputs[i].roll - dword_0xA4_164x->roll_0x155_341;                 // EF:38060
dword_0xA4_164x->rollDelta_0x4_4  = (rollEnv  - (my_sign32(rollEnv)  << 2) + my_sign32(rollEnv))  >> 2;   // EF:38061
int pitchEnv = 2 * playerInputs[i].pitch - dword_0xA4_164x->pitch_0x157_343;              // EF:38062
dword_0xA4_164x->pitchDelta_0x6_6 = (pitchEnv - (my_sign32(pitchEnv) << 2) + my_sign32(pitchEnv)) >> 2;   // EF:38063
dword_0xA4_164x->entityIndex_0x0      = playerInputs[i].entityIndex_0x6E3E_byte5;         // fire bits
dword_0xA4_164x->nextEntity_0x18_24   = playerInputs[i].nextEntity_0x6E3E_word6;          // Channel B offset (=0 mouse)
dword_0xA4_164x->entityIndex2_0x1A_26 = playerInputs[i].entityIndex2_0x6E3E_word8;        // Channel B offset (=0 mouse)
```
`rollDelta = (2·roll − roll_0x155) · ¾-ish` — a **spring toward `2·roll`**: the delta is proportional to
how far the accumulator `roll_0x155_341` is from twice the desired offset. `(x − sign·4 + sign)>>2` is
`≈ (x − 3·sign) >> 2 = x/4` biased toward zero (a rounding-toward-zero divide by 4). So
**`rollDelta ≈ (2·roll − roll_0x155)/4`**. Same for pitch. This is a **critically-damped follow** of the
mouse offset, not a raw copy.

### 2.2 The per-tick pose integrator — `sub_5D530` (EF:59610)

Runs every wizard tick. VERBATIM (EF:59622-59652):
```c
if (moveSpeed_0x14C_332) {                              // slowed/mobilizing → scale the delta
    locIntTemp = rollDelta_0x4_4 * (4 - moveSpeed_0x14C_332);
    roll_0x155_341 += (locIntTemp - (my_sign32(locIntTemp) * 3)) >> 2;
    locIntTemp = pitchDelta_0x6_6 * (4 - moveSpeed_0x14C_332);
    pitch_0x157_343 += (locIntTemp - (my_sign32(locIntTemp) * 3)) >> 2;
} else {                                                // normal flight
    roll_0x155_341  += rollDelta_0x4_4;                 // accumulate roll delta
    pitch_0x157_343 += pitchDelta_0x6_6;                // accumulate pitch delta
}
locIntTemp = roll_0x155_341;
yaw_0x1C_28 = (yaw_0x1C_28 + ((locIntTemp - (my_sign32(locIntTemp) * 7)) >> 3)) & 0x7FF;   // EF:59635  YAW turn-RATE
…
int16_t tempPitch = pitch_0x157_343 & 0x7ffu;           // EF:59651
pitch_0x1E_30 = tempPitch;                               // EF:59652  PITCH set ABSOLUTE
```
**The two aim laws on a mouse:**
- **YAW (`yaw_0x1C_28`) is a RATE.** Each tick `yaw += (roll_0x155_341 − sign·7) >> 3 ≈ roll_0x155/8`,
  wrapped `& 0x7FF` (0..2047). `roll_0x155_341` is the accumulated roll delta (banking). So **holding
  the mouse to one side keeps turning** — the further off-center the mouse, the larger `roll` →
  `roll_0x155` grows → faster turn. This is the "lean to turn" flight feel. `roll_0x155_341` also feeds
  the visible **bank/roll** of the carpet (EF:40258).
- **PITCH (`pitch_0x1E_30`) is ABSOLUTE.** `pitch_0x1E_30 = pitch_0x157_343 & 0x7FF`, where
  `pitch_0x157_343` springs toward `2·pitch` (mouse-Y offset). So mouse-Y **directly points the nose
  up/down** to a pitch proportional to the vertical mouse offset (spring-followed). (`roll_0x155_341`
  self-decays via EF:60577 `roll_0x155 -= (roll_0x155 − sign·7)>>3` when input stops → the turn
  eases out and the carpet levels.)

**Units:** yaw/pitch are **0..2048 per full circle** (`& 0x7FF` = mask to 0..2047), confirming the
codebase's 2048-unit angle convention.

### 2.3 Why this is the likely "aiming feels off" culprit

On a plain mouse the launch offset (§3) is 0, so aim = wizard pose. The pose is: **yaw = integrated
turn-rate from mouse-X, pitch = absolute from mouse-Y (spring-damped, ÷2-ish scaling via the `2·roll`
spring and `>>3`/`>>2` divides).** A port that (a) sets yaw absolutely from the mouse, or (b) omits the
`roll_0x155` accumulator / the `>>3` rate divide, or (c) uses a raw mouse *delta* instead of the
*absolute screen-center offset*, will not reproduce this feel. The pitch mapping in particular is a
spring toward `2·(mouse-Y-offset)` then `& 0x7FF` — not a linear 1:1.

---

## 3. CHANNEL B (180590/594) AFFECTS BOTH VIEW AND SHOT — the same offset

### 3.1 The camera/view (EF:40256-40274)

Building the render camera for the local player, VERBATIM:
```c
view.rotation.yaw   = a2x->yaw_0x1C_28;                                    // EF:40256  wizard yaw
view.rotation.roll  = a2x->dword_0xA4_164x->roll_0x155_341;                // EF:40258  carpet bank
view.rotation.pitch = a2x->dword_0xA4_164x->pitch_0x157_343 / 2 + …bob…;   // EF:40267  wizard pitch (/2!) + head-bob
view.rotation.yaw   += a2x->dword_0xA4_164x->nextEntity_0x18_24;           // EF:40273  + free-look yaw offset
view.rotation.pitch += a2x->dword_0xA4_164x->entityIndex2_0x1A_26;         // EF:40274  + free-look pitch offset
```
### 3.2 The projectile launch (EF:55867-55868, and every effect state)
```c
v6x->yaw_0x1C_28   = v1x->dword_0xA4_164x->nextEntity_0x18_24   + v1x->yaw_0x1C_28;    // EF:55867
v6x->pitch_0x1E_30 = v1x->dword_0xA4_164x->entityIndex2_0x1A_26 + v1x->pitch_0x1E_30;  // EF:55868
```

### 3.3 Answer to Q3
**The mouse/free-look offset is added identically to BOTH the camera view and the projectile** — same
field, same sign, same magnitude. **The view turns with the offset, and the projectile simply inherits
that same offset.** There is **no aim-vs-view divergence** — you shoot exactly where the free-look
points. Note one asymmetry in the VIEW-only path: the camera uses `pitch_0x157_343 / 2` (half the raw
pose pitch, EF:40267) plus head-bob, whereas the projectile uses `pitch_0x1E_30` (the full pose pitch,
which is `pitch_0x157_343 & 0x7FF`, EF:59652). So the **rendered camera pitch is half the aim pitch** (a
cosmetic "look-down is gentler than you aim" — the shot goes steeper than the camera tilts). The **yaw**
is identical between view and shot. **Port consequence:** since retail's VIEW turns with the mouse via
the *pose* (yaw/pitch, §2) AND the offset applies to both, if your port already turns the view+wizard
with the mouse, you have the aim — you do **not** need a separate 180590/594 term for a plain mouse (it
is 0). If aim still feels off, re-check the pose math (§2) and the **view pitch ÷2** asymmetry.

---

## 4. IS THERE A MOVING CURSOR/CROSSHAIR IN FLIGHT? — NO

- **No HUD reticle at all** in flight view (re-confirmed from `mc2-autoaim.md` §4: GameUI/GameRender
  draw no crosshair/lock sprite; the player's `word_0x96_150` is never read by UI).
- **The 180590/594 offset is a pure ANGLE, never a screen position** — it is added to yaw/pitch
  (§3), never used to place a cursor. There is no "draw cursor at offset" anywhere.
- **`x_DWORD_1805B0_mouse` (the raw mouse SCREEN position)** is read only for **menu/HUD hit-testing** —
  spellbook selection (`SelectSpell_6D4F0` PI:2175, `SelectSpellCategory_6D420` PI:2226), the spell/map
  panel bounds tests (PI:715-845), options-menu buttons (PI:2315+), and the menu-cursor draw
  `SetMousePositionInMemory_5BDC0` (GameUI:3908, fed from `position_backup_20` GameUI:3921-3926).
  **None of these run in flight view** — they are gated on `MenuState`. So in flight there is **no
  cursor and no reticle**; the "aim" is entirely the camera direction (which the mouse steers via the
  pose). The only in-flight aim visual is the local projectile's own sprite `SetEntityIndex_49C90(proj,
  42)` (EF:55878) — an in-world muzzle bolt, not a screen-anchored cursor.

**Answer to Q4:** No moving cursor, no crosshair, no reticle in flight. POINTERS.DAT cursors are drawn
only in menus (via `x_DWORD_1805B0_mouse` screen position); the flight aim has no on-screen indicator.

---

## 5. THE FIREBALL 34-CAP INITIAL TURN — verbatim (`sub_65C20`, EF:63080-63122)

Fireball flight state, FIRST-tick acquisition branch. VERBATIM:
```c
v2 = a1x->struct_byte_0xc_12_15.byte[0];
if (!(v2 & 2)) {                                         // FIRST flight tick only (init gate)
    a1x->struct_byte_0xc_12_15.byte[0] = v2 | 2;
    if (sub_68940(a1x)) {                                // guide-drone lock (rare) …
        v6 = sub_582B0(a1x->yaw_0x1C_28, a1x->roll_0x20_32);   // |yawErr| to desired
        if (v6 < 0)  v6 = 0;
        if (v6 > 34) v6 = 34;                            // CAP = 34
        v7 = v6 * sub_582F0(a1x->yaw_0x1C_28, a1x->roll_0x20_32) + a1x->yaw_0x1C_28;   // yaw += ±min(err,34)
        v5 = a1x->fov_0x22_34;
        a1x->yaw_0x1C_28 = v7;
    } else {
        if (!sub_67CB0(a1x)) {                           // NO autoaim target →
            a1x->roll_0x20_32 = a1x->yaw_0x1C_28;        //   lock straight: desiredYaw = yaw
            a1x->fov_0x22_34  = a1x->pitch_0x1E_30;      //   desiredPitch = pitch  (NO turn)
            goto LABEL_18;
        }
        v3 = sub_582B0(a1x->yaw_0x1C_28, a1x->roll_0x20_32);   // |yawErr| to acquired target
        if (v3 < 0)  v3 = 0;
        if (v3 > 34) v3 = 34;                            // CAP = 34
        v4 = v3 * sub_582F0(a1x->yaw_0x1C_28, a1x->roll_0x20_32) + a1x->yaw_0x1C_28;   // yaw += ±min(err,34)
        v5 = a1x->fov_0x22_34;
        a1x->yaw_0x1C_28 = v4;
    }
    a1x->pitch_0x1E_30 = v5;                             // PITCH snapped to desired fov (NOT capped)
}
```
Helpers (S:6569-6604): `sub_582B0(a,b) = |(a&0x7FF)−(b&0x7FF)|`, folded to the **shortest arc**
(`if >1024 → 2048−`) — the **absolute yaw error in 2048-units** (0..1024). `sub_582F0(a,b)` = the
**sign of the shortest turn** (+1 / −1 / 0).

**The exact law (Q5):**
- **What is capped:** **YAW ONLY.** `yaw += clamp(|yawErr|, 0, 34) · turnSign`.
- **Cap value/units:** **34 angle-units** in the 2048-per-turn convention (≈ 34/2048 ≈ **6.0°**).
- **Per-tick or one-shot:** **ONE-SHOT, on the first flight tick only** (gated by
  `byte[0] & 2` init flag — set on entry, so this whole block never runs again; thereafter
  `sub_65610` homes with the behavior-row turn caps, EF:63086). So the 34-cap is the **initial
  "nudge toward target" on launch**, not a per-tick homing rate.
- **Pitch:** **snapped, not capped** — `pitch_0x1E_30 = fov_0x22_34` (the desired pitch from
  `sub_655C0`). No 34-limit on pitch.
- **No target:** `roll_0x20_32 = yaw; fov_0x22_34 = pitch` (EF:63108-63109) → desired = current →
  the fireball flies **straight on the launch angle** (which already = wizard pose + offset, §3), no
  turn. (Contrast possession `CastPosses_65F60` which **snaps** `yaw = roll; pitch = fov` with no cap,
  per autoaim trace §1.6.)

**Why 34 and not a full snap:** the fireball only *slightly* curves toward a nearby target at launch
(≤6° yaw), then homes each subsequent tick — it does not teleport its heading onto the target. This is
the "assisted but not locked" feel. If the target is within 6° it points exactly at it; beyond that it
launches 6° toward it and the homing (`sub_65610`) closes the rest over following ticks.

---

## 6. COMPLETE ENUMERATION OF INITIAL YAW/PITCH TERMS (Q6)

For a **player-cast projectile**, the initial `yaw_0x1C_28` / `pitch_0x1E_30` at spawn is:

| term | yaw | pitch | source | on plain mouse |
|---|---|---|---|---|
| **wizard pose** | `wizard.yaw_0x1C_28` | `wizard.pitch_0x1E_30` | pose integrator §2 (mouse-driven) | **the whole aim** |
| **free-look offset** | `+ nextEntity_0x18_24` | `+ entityIndex2_0x1A_26` | `= x_DWORD_180590/594` (EF:55867-68) | **0** (peripheral-only, §1) |
| **muzzle z-lift** | — (position, not angle) | — | `proj.pos.z += wizard.array_0x52_82.fov` (EF:55866) | (position) |
| **charged-lightning fork** | `± 113` (tier-2 only) | — | `v5 = yaw ± 113` (EF:56631/56633), only `life_0x1A==2` | ±113 if charged lightning |
| **autoaim initial turn** | `± min(|err|, 34)` on tick 1 | pitch snapped to desired | fireball §5 (and other flight states) | if a target is acquired |
| **hand-side ±512** | **NOT an angle term** | — | `sub_68E50` muzzle POSITION step (autoaim §7) | (position offset only) |
| **random spread** | **NONE** | **NONE** | — zero RNG in launch (autoaim §2, cast-path §2) | none |

**So on a plain mouse a single fireball launches at exactly `wizard.yaw / wizard.pitch`** (offset = 0),
optionally nudged ≤34 yaw toward an autoaim target on tick 1. The wizard pose is 100% mouse-driven via
Channel A (§2). There is **no hand-side yaw offset in the ANGLE** (the left/right muzzle is a *position*
step in `sub_68E50`, ±512 yaw applied to a 256-unit *MoveEntity translation*, not to `yaw_0x1C_28`) and
**no per-shot random spread anywhere**.

### 6.1 The `axis_0x9A_154x` aim-point (secondary — not the heading)
The effect state also writes `proj.axis_0x9A_154x = wizard.pos` then
`MoveEntity_57FA0(&axis, nextEntity_0x18_24 + wizard.yaw, entityIndex2_0x1A_26 + wizard.pitch, 0x4000)`
(EF:55871-55876) — a **point 0x4000 (16384) units ahead along the same launch vector**. This is a target
waypoint some flight states steer toward; it uses the **identical** yaw/pitch (pose + offset), so it adds
no new aim term.

---

## 7. OPEN / uncertain

- **Plain-mouse 180590/594 = 0 is inferred** from device 7 having no `case` in the
  `ReadGameUserInputs_89D10` switch (falls to `default: break`) and from the zero-init at EF:49504-49505.
  I did not find a hidden per-frame writer for device 7 (grep of all `180590/594 =` writes is exhausted
  in §1.1). If a golden/replay shows nonzero 180590 under a mouse, re-examine — but the decompile shows
  no such path. **The mouse aim is Channel A (pose), not Channel B.** [CONFIDENT but flagged.]
- **`ComputeMousePlayerMovement_17060` uses ABSOLUTE mouse position** (`x − centerX`), i.e. the game
  parks the OS cursor and reads its screen offset — classic DOS "the cursor position IS the stick." A
  modern port must emulate a virtual absolute pointer that re-centers, or convert relative mouse motion
  into an absolute deflection with the same ±127 clamp. Using raw relative deltas will NOT match. [Port
  hazard, HIGH.]
- **`speedIndex` guard** (PI:2110): `ComputeMousePlayerMovement` only writes roll/pitch when
  `speedIndex == 0`. What sets `speedIndex` nonzero (a menu/transition substate?) not traced — assume
  flight = 0. [Minor.]
- **VIEW pitch ÷2** (EF:40267) vs projectile full pitch (EF:55868): confirmed the rendered camera tilts
  at half the aim pitch. Whether the port should mirror this cosmetic asymmetry is a presentation call;
  the SHOT uses full `pitch_0x1E_30`. [Cosmetic, flagged for view fidelity.]
- **`moveSpeed_0x14C_332` / `mobilizeCounter`** scaling of the pose delta (EF:59624-59627) — active only
  when slowed/mobilizing; normal flight uses the raw accumulate (EF:59631-59632). Port can implement the
  normal branch first. [Minor.]
- The **head-tracker `flt_D1F40` scale and `x_WORD_17D6CC` raw units** (EF:50561, 49726) are device data
  not needed for a mouse/keyboard port. [Out of scope.]
