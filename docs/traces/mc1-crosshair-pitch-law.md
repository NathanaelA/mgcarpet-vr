# MC1 CROSSHAIR / AIM-PITCH SCREEN LAW — Verbatim Trace

Player report (faithful throttle + altitude mode): the aim/camera-pitch disparity is in the
RIGHT DIRECTION but **too pronounced** — retail lands the aim marker at ~**1/3** screen-height
(full up) and ~**2/3** (full down); the port overshoots to ~**0.145 / 0.855**.

Retail citations are `sub_main.cpp:line` in `reference/remc1/` unless noted. Port citations are
`crates/…`. Every conclusion tagged **CONFIRMED** (decompile line) or **INFERRED**.

---

## 0. TL;DR — the two laws and the delta

- **Retail renders pitch as an AFFINE HORIZON SHEAR, not a perspective rotation.** The camera
  pitch (`= aim/2`, CONFIRMED :52434) shifts the eye-level row by `width·pitch/256` px
  (CONFIRMED :33872 / :38245), and a world point's own elevation adds `fowDist·Δz/dist`
  (CONFIRMED :36853). The shot (fired at the FULL aim) and the half-pitch horizon shear
  **nearly cancel**, leaving a small net offset.
- **The port renders pitch as a TRUE PERSPECTIVE ROTATION** (`world_to_screen`, `f = 1/tan(fov/2)`,
  FOV_Y 60°, CONFIRMED mgc-render/src/lib.rs:3231). The crosshair is placed at a world point on the
  FULL aim ray, viewed by a camera pitched to `aim/2`, so its net angular elevation above the view
  axis is `aim/2`, magnified by the narrow 60° FOV. No cancellation.
- **Both models agree on inputs** (CONFIRMED): camera = `aim/2`; aim clamp `±254` engine units
  (`±44.66°`); the port matches both. The divergence is **purely the vertical projection model**.
- **Delta at full aim (±254 units):** retail net offset from center ≈ **0.14–0.16·H** → crosshair
  at ~**0.34–0.36 / 0.64–0.66** (matches the player's 1/3–2/3). Port offset = **0.355·H** →
  **0.145 / 0.855**. Port overshoots by ~**2.4×**.

---

## 1. THE AIM PITCH — value, filter, clamp (both games agree)

### 1.1 Mouse → stick, clamped ±127 — CONFIRMED :19904-19920
`sub_169E0` maps absolute mouse-Y (offset from a 200-native screen center, `<<7`) to the pitch
stick `var_29715[player][4] = v2`, clamped `if (v2 < -127) v2 = -127; if (v2 > 127) v2 = 127;`
(:19913-16). So stick_y ∈ **[−127, 127]**.

### 1.2 Filter → published aim, settles at 2·stick — CONFIRMED :49019-49020, :55144, :55158-62
- Delta: `pitchEnv = 2·var_29715[player][4] − u16_329; word_0x6_6 = pitchEnv/4` (round-toward-zero,
  :49019-20).
- Integrate: `u16_329 += word_0x6_6` (:55144). Converges to `u16_329 = 2·stick_y`.
- Publish: `v6 = u16_329; HIBYTE(v6) &= 7` (mask 0x7FF); `var_u16_29827_32 = v6`;
  `if (v6 > 1024) v6 -= 2048` (:55158-62) — the signed aim.
- **Range: `2·(±127) = ±254` engine units = `±254·360/2048 = ±44.66°`.** CONFIRMED.

Port mirror (CONFIRMED): `flight.rs:207/209` `pitch_f += (2·stick_y − pitch_f)/4`;
`flight.rs:224` `aim_pitch = pitch_f & 0x7FF`; `flight.rs:123-126` `aim_signed()` = the same
`>1024 ⇒ −2048` unwrap; `lib.rs:537` `f.pitch = −aim_signed·(2π/2048)`. Same `±254 ⇒ ±0.7793 rad`.

### 1.3 Camera renders HALF the aim — CONFIRMED :52434
`str_13895_572[cam].pitch_8 = u16_329/2 + <buffet-kick terms>` — the render pitch is **half** the
published aim. Casts/projectiles launch on the FULL `u16_329` (aim). Port mirror (CONFIRMED):
`main.rs:2021-2025` `view_pitch = aim*0.5` under `ThrustModel::Mc1`; `main.rs:2031`
`cam.pitch = view_pitch − kick`. **The half-pitch split is faithful and correct in the port.**

---

## 2. THE RETAIL VERTICAL PROJECTION — affine horizon shear (the crux)

Magic Carpet is a voxel/heightfield raycaster; **pitch never rotates the view frustum — it shears
the horizon vertically**, and object elevation is an independent affine term.

### 2.1 Horizon shear — CONFIRMED :33872 (and :32641)
```
dword_B5CFC_B5CEC = (widthViewPort_93AD8 * pitch) >> 8;   // = width·pitch/256  (pitch = pitch_8 = aim/2)
```
Consumed as the eye-level screen row — CONFIRMED :38245:
```
v10 = heightViewPort_B5CE4_B5CD4 - ((cos(roll) * dword_B5CFC) >> 16);   // = H/2 − width·pitch/256  (roll 0)
```
`heightViewPort_B5CE4_B5CD4 = heightViewPort_93ADC >> 1` (:33787) = vertical center. So the
**eye-level / horizon row = H/2 − width·(aim/2)/256** px.

### 2.2 Object elevation — CONFIRMED :36853 (and :33977, :37263…)
```
wRotZY = tempZ * fowDist_B5D14_B5D04 / yRot + dword_B5CFC_B5CEC;   // screen row of a world vertex
```
`tempZ` = altitude relative to eye, `yRot` = yaw-rotated forward distance, `+dword_B5CFC` folds in
the horizon shear. A point at elevation angle α at horizontal distance d has `tempZ/yRot = tan(α)`
(**INFERRED** — the polar mover decomposes speed as `cos`/`sin` of pitch, so slope = `tan(aim)`;
empirically corroborated in §4). So elevation contributes **`fowDist·tan(α)`** px, added to center.

### 2.3 `fowDist` — CONFIRMED :33801-33804, :49157
```
fowDist = (fow * Distance(width² + height²)) >> 8;   // fow=fov_12; Distance ≈ screen diagonal
```
Default `fov_12 = 128` (CONFIRMED :49157). With `fow = 128 = 0x80`:
**`fowDist = 128·diag/256 = diag/2`**, `diag = √(W²+H²)`.

### 2.4 Viewport dims — CONFIRMED :38448-38455
Full screen (`viewPortSize = 40`): low-res `W=8·40=320, H=5·40=200` (8:5);
hi-res `W=16·40=640, H=12·40=480` (4:3). The shear/elevation scales use these `W,H`.

### 2.5 Net retail law for the aim marker (the SHOT)
Shot fired on the FULL aim `A` (units), angle `α = A·2π/2048`, camera pitched `A/2`:
```
screen_row(shot) = H/2  −  W·(A/2)/256   −  fowDist·tan(α)        (aim UP → both terms lift it)
net offset from center =  fowDist·tan(α)  −  W·(A/2)/256          (magnitude; a near-cancellation)
```

---

## 3. THE PORT — true-perspective predictor (the mismatch)

### 3.1 The crosshair predictor — mgc-app/src/main.rs:2229-2256
Places a world point on the **full aim ray**: `(sp,cp)=aim.sin_cos()` (aim = FULL pitch),
`point = cam + (sy·cp, sp, −cy·cp)·AIM_D`, `AIM_D=20` tiles (:2234-2245). Projects it with
`world_to_screen` using `cam.pitch = aim/2` (§1.3). Since point-pitch = `aim` and cam-pitch =
`aim/2`, the point's angular elevation above the view axis is exactly **`aim/2`**.

### 3.2 The projection — mgc-render/src/lib.rs:3174-3247
True perspective: `f = 1/tan(fov_y/2)`, `FOV_Y = 60°` (main.rs:32), `ndc_y = clip_y/w`,
`screen = (0.5 − ndc_y/2)·H`. A point at elevation `θ` above the axis lands at
**offset = 0.5·tan(θ)/tan(FOV_Y/2)·H**, `θ = aim/2`.

### 3.3 The mismatch (answering task 2)
- **Wrong projection model.** Port uses perspective-rotation (angular difference `aim/2` through a
  60° FOV). Retail uses affine shear where the half-pitch horizon (`W·(A/2)/256`) *cancels* most of
  the shot's own elevation (`fowDist·tan(α)`). The port predicts a **different point than retail
  projects** — it never applies the cancelling shear.
- Clamps and the `aim/2` split are **correct**; FOV/scale + the missing shear are the fault.

---

## 4. QUANTIFICATION (task 3) — full up / full down

`A = 254`, `α = 44.66°`, `tan α = 0.988`, aim/2 angle `= 22.33°`, `tan = 0.4103`.

**Port (perspective, FOV 60):** offset `= 0.5·0.4103/tan(30°) = 0.5·0.4103/0.5774 = 0.355·H`.
→ crosshair at **0.145 / 0.855**. (Independent of resolution.)

**Retail (affine), full screen:**
| res | W:H | `fowDist·tanα/H` | `W·(A/2)/256/H` | net offset | crosshair |
|----|----|----|----|----|----|
| lo 320×200 | 8:5 | 0.932 | 0.794 | **0.138** | 0.362 / 0.638 |
| hi 640×480 | 4:3 | 0.823 | 0.661 | **0.162** | 0.338 / 0.662 |

Both land at ≈ **1/3 and 2/3** — matching the player's retail measurement. The port's **0.145 /
0.855** overshoots by ≈ **2.4×** (offset 0.355 vs ~0.15).

*(The retail net is the small difference of two ~0.8–0.9 terms — a hallmark the port's single-term
perspective model cannot reproduce; the 1/3–2/3 landing validates the `tan(α)` slope of §2.2.)*

---

## 5. PROPOSED MINIMAL FAITHFUL FIX (task 4)

Replace the crosshair's **vertical** placement with the retail affine law (§2.5); keep the
horizontal (yaw) placement as-is. Constants all from the decompile:

```
// aim   : FULL aim pitch (rad, +up), port f.pitch     [flight.rs:537 / clamp ±0.7793]
// A     : aim in engine units = aim·2048/(2π)          [±254, §1.2]
// W,H   : retail viewport proportions — use 4:3 (hi-res), the shipped default   [§2.4]
// fowDist = √(W²+H²)/2                    (fov_12 = 128 ⇒ (128·diag)>>8)         [§2.3]
let alpha    = aim;                                   // radians, signed
let a_units  = aim * 2048.0 / TAU;                    // ±254
let diag     = (W*W + H*H).sqrt();
let fowdist  = diag / 2.0;
let offset   = fowdist * alpha.tan() - W * (a_units.abs()/2.0) / 256.0;   // px, magnitude
let cross_y  = H/2.0 - alpha.signum() * offset;       // aim up ⇒ above center
```
Expressed resolution-independently against the port surface height `Hs` (4:3 proportions):
`offset_frac = (√(1 + (W/H)²)/2)·tan(α) − (W/H)·(|A|/2)/256`, then `y = Hs·(0.5 − sign(α)·offset_frac)`.
At full aim this yields **0.162** (hi-res 4:3) ⇒ crosshair **0.338 / 0.662** — the retail target.

**Caveat / follow-up (flag, not in this fix's scope):** the port's *world camera* is perspective, so
actual projectiles currently render at the port's **0.145 / 0.855**, not retail's 1/3–2/3. Fixing
only the crosshair makes it disagree with where the port's own shots land. To make **shots** land
faithfully too, the renderer must adopt the same affine vertical model (horizon shear at `aim/2` +
`fowDist·tan` object elevation) — a larger `world_to_screen` / camera change. This trace fixes the
reported crosshair overshoot; the renderer-parity item is logged as the deeper follow-up.

---

## 6. CITATION INDEX

| Fact | Where | Tag |
|---|---|---|
| mouse-Y → stick, clamp ±127 | :19904-19920 | CONFIRMED |
| pitch filter `2·stick − aim`, `/4`; converges 2·stick | :49019-20, :55144 | CONFIRMED |
| aim publish + signed unwrap (`±254`) | :55158-62 | CONFIRMED |
| camera pitch = `aim/2` | :52434 | CONFIRMED |
| horizon shear `width·pitch/256` | :33872, :32641 | CONFIRMED |
| eye-level row `H/2 − width·pitch/256` | :38245 | CONFIRMED |
| object elevation `fowDist·Δz/dist + shear` | :36853, :33977 | CONFIRMED |
| `fowDist = fow·diag/256`, `fov=128 ⇒ diag/2` | :33801-04, :49157 | CONFIRMED |
| viewport dims 320×200 / 640×480 | :38448-55 | CONFIRMED |
| slope `Δz/dist = tan(aim)` | polar decomposition | INFERRED (validated §4) |
| port half-pitch split | main.rs:2021-2031 | CONFIRMED |
| port crosshair = full-aim world point | main.rs:2229-2256 | CONFIRMED |
| port perspective `f=1/tan(fov/2)`, FOV 60 | render lib.rs:3231, main.rs:32 | CONFIRMED |
| port aim clamp mirror | flight.rs:123-126,207-224; lib.rs:537 | CONFIRMED |
