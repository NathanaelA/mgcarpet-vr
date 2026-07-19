# MC1 Speed (Accelerate) — re-cast + hold, and the PR-2 refusal

Trace for player-reported regression **PR-2 / fix-plan J4** (MC1 + MC1HW):
*"the Speed spell allows REPEATED casting (stacking/refreshing) and
CAST-AND-HOLD for a stronger effect; the port refuses the re-cast and the
hold."* Read-only investigation; every claim tagged **CONFIRMED** (with a
`file:line`) or **INFERRED**.

The MC1 "Speed" spell is **Accelerate** — id **2** (forward) / id **21**
(backward). CONFIRMED: `crates/mgc-sim/src/mc1/spells.rs:44,63` (the name
table), ctors `sub_3C0C0`/`sub_3C420` (spells.rs:111,149).

---

## 1. Retail law (reference/remc1)

### 1a. The cast trigger — no armed-gate for Accelerate; held reloads every tick
`sub_46B00_46E40` (`reference/remc1/sub_main.cpp:55851`), called twice per
frame by the carpet fire tick `sub_46840_46B80` (:55830-45, once per hand).

- With the cast button **held** (`a4 & dw_0`), the spell-id switch on
  `var_u8_..._65`: id **16 (Castle)** is the ONLY spell hard-gated on a live
  burst — `if (a2x->var_48) { buzz 29; return; }` (**:55895-99**). Every other
  hand spell (incl. Accelerate 2/21) falls through to **`LABEL_32`**
  (**:55893**): `a2x->var_48 = a2x->var_50;` — i.e. the burst counter is
  **reloaded to `count` (=251) EVERY held tick**, regardless of whether it was
  already live. CONFIRMED :55880-93.
- Accelerate's arms are mutually exclusive: firing 2 clears manif[21]'s
  `var_48`, firing 21 clears manif[2]'s (**:55873 / :55910**). CONFIRMED.
- Accelerate's `charge_flag` (+62) is 0 (spells.rs:112,150), so the
  hold-accumulator branch at :55877-89 is not taken — it goes straight to the
  `var_48 = var_50` reload. CONFIRMED.

So retail imposes **no "already armed" refusal** on re-casting Accelerate; it
re-arms freely on every press and holds by reloading each tick.

### 1b. The speed magnitude — 3× held, 2× decaying after release
`sub_56380_568B0` (`:65131`), the Accelerate manifestation/effect tick:

```
if (var_48 == var_50)   v4 = 3 * caster.actSpeed_128;   // :65167-69  → 3×
else                    v4 = 2 * caster.actSpeed_128;   // :65175     → 2×
caster.mgr.v_12 = v4;  caster.actSpeed_126 = v_12;       // target+actual speed
```
- `var_48 == var_50` means the burst is **at full** — true on exactly the ticks
  the cast function just reloaded it (§1a), i.e. **while the button is held**.
  Result: **held = 3×** (`accel_held` in the port). CONFIRMED :65167-69.
- After release the cast function stops reloading, so `var_48` counts **down**
  from 251 (`< var_50`): **released = 2×**, sustained for the full 251-tick
  drain, then `var_48==0` restores the base speed (:65189-96). CONFIRMED.
- `actSpeed_128` (base) is pinned to **80** at the top of the player update
  every frame (`a1x->actSpeed_29923_128 = dword_93A90; dword_93A90 = 80`,
  **:55343 + :4209**). So the speeds are 3·80=240 (held) / 2·80=160 (released).
  CONFIRMED.

### 1c. The cancel — RESISTING thrust only (this is the crux of PR-2)
The effect tick cancels the boost via the **speed-touched flag `v_14`**:
```
if ( !gate || caster.mgr.v_14 ) { if (caster.mgr.v_14) var_48 = 1; }   // :65145-50
```
`v_14 = 1` clamps the burst to 1 → the boost expires next tick.

`v_14` is set by the command integration `sub_46840_46B80` **only when a thrust
key actually moves `v_12` inside the ±80 band** (:55764-83):
```
v3=+1 if (forward held  && v_12 < dword_93A90 /*  80 */);   // :55766
v3=-1 if (backward held && v_12 > dword_93A8C /* -80 */);   // :55769
if (v3) { v_12 += 16*v3; clamp ±80; v_14 = 1; }             // :55772-83
```
While Accelerate is active, `v_12` is pinned at **160–240 (> 80)**, so:
- **Forward** thrust: `v_12 < 80` is **false** → v3=0 → **v_14 stays 0 → NO cancel.**
- **Backward** (braking) thrust: `v_12 > -80` is true → v3=−1 → **v_14=1 → cancel.**

CONFIRMED :55766/:55769 + constants :4207-09. **Retail cancels Accelerate on
the RESISTING (braking) thrust only; pushing forward while Speed is active does
nothing** — you are already clamped above the forward max. (Symmetric for
backward Accelerate: forward thrust brakes it.) The manual's "press the down
cursor to cancel" is literal — it is the *resisting* input, not "any input".

### 1d. Expiry quirk
On expiry the target+actual speed snap to **+80 max forward** even out of
backward flight (:65189-96). CONFIRMED. Ported at `lib.rs:415-423`.

### 1e. Hidden Worlds delta
**None for Accelerate.** remc1hw carries the same constants (16/−80/80,
`sub_main.cpp:3911-13`), the same resisting-only cancel conditions (:51833/
:51836), the same base=80 (:51411) and the same 3×/2× effect law (:61391/
:61397). `SPELLS_HW` diverges from base MC1 in **exactly one** row — spell 20,
not 2/21 (spells.rs:165-169). CONFIRMED.

---

## 2. Port divergence

### 2a. The World-level toggle logic is FAITHFUL (not the bug)
`crates/mgc-sim/src/mc1/world.rs:2604-2627` reloads `f26 = count` on every held
tick, sets `accel_held` (→ 3.0) each tick, and re-casts refresh the burst
without re-debiting mana — matching §1a/§1b exactly. **CONFIRMED intact**: the
unit test `accelerate_directions_are_mutually_exclusive` (world.rs:8584) drives
`fire_left` held and asserts `Some(3.0)` held / `Some(2.0)` released / re-cast
refresh — and it **passes**. Without a thrust input, hold and re-cast work.

### 2b. c44021a is NOT the cause
`git show c44021a^:.../world.rs` — the id-2/21 toggle block is **byte-identical**
to the current one. c44021a only removed the `(armed && id != 22)` clause on the
*generic edge-trigger* gate at world.rs:2641, which is reached **after** the
2/21 block returns (2626). So the suspected commit never touched the Speed path.
CONFIRMED.

### 2c. THE BUG — the MC1 thrust-model over-cancels on FORWARD thrust
`crates/mgc-sim/src/lib.rs:274-279`, inside `Simulation::step`, **before** the
world turn:
```rust
ThrustModel::Mc1 => {
    if input.thrust != 0.0 {
        w.thrust_cancel(1.0);    // → accel_veto.1, stop_accel(21)
        w.thrust_cancel(-1.0);   // → accel_veto.0, stop_accel(2)   ← kills FORWARD Speed
    }
}
```
`thrust_cancel` (world.rs:3711) sets the veto and `stop_accel` (zeroes `f26`,
`accel`, `speed_boost`); the cast gate then refuses re-arm
(`if (id==2 && accel_veto.0) return;`, world.rs:2608). So **ANY** thrust — including
holding **forward**, the instinctive "go fast" input while using Speed —
triggers `thrust_cancel(-1.0)`, which cancels and refuses forward Accelerate on
**every tick**. To the player: the Speed spell won't hold and won't re-cast.

This contradicts §1c: retail's forward thrust is **inert** during Speed (v_12
pinned above +80). The `if thrust != 0 { cancel both }` shape was introduced by
commit **1012805** ("Faithful flight controls polish") on the mistaken premise
(comment lib.rs:266-271) that "faithful MC1 = ANY Up/Down press cancels". The
correct retail law is **resisting-only** — which the **Enhanced** branch
(lib.rs:280, `w.thrust_cancel(input.thrust)`) already implements correctly.

The bug fires only under `ThrustModel::Mc1` and only when a thrust key is held.
App thrust is clean keyboard `axis(back, forward)` (mgc-app main.rs:1120) — 0
when idle — so the refusal is exactly "hold forward while casting Speed".

---

## 3. Proposed minimal faithful fix

Collapse the MC1 branch to the same resisting-only call the Enhanced model uses
(retail's true law, §1c) and correct the comment:
```rust
// The Accelerate cancel reads the tick's raw thrust BEFORE the move.
// Faithful MC1 (sub_56380 :65145-50 + sub_46840 :55766/:55769): only the
// RESISTING thrust cancels — while Speed is active v_12 is pinned above the
// ±80 band, so the non-resisting (forward) press can't set v_14 and is inert.
if let Some(w) = &mut self.world {
    w.thrust_cancel(input.thrust);
}
```
This restores forward-thrust-while-Speed (hold + re-cast) and keeps the braking
cancel. `thrust_cancel` already dispatches on sign, so passing `input.thrust`
directly is sufficient; the `ThrustModel` split here becomes unnecessary.
(Optional stricter fidelity — gate the cancel on `|v_12| < 80` to model the
first-cast edge — is not needed for PR-2 and adds state; resisting-only matches
retail steady-state and the player-observed feel.)

## 4. Regression tests to pin it

- **PR-2 (Simulation-level, the missing coverage):** `Simulation::with_world`,
  `thrust_model = Mc1`, dev-spells, equip Accelerate (id 2); step with
  `FlightInput { fire_left: true, thrust: 1.0, .. }` for several ticks and assert
  `world.accel_override() == Some(3.0)` each tick (pre-fix: `None` — refused).
  Then step with `thrust: -1.0` and assert `None` (the faithful brake still
  cancels). Release and confirm it drains through `Some(2.0)`.
- **J4 refire-gate (World-level, c44021a's blind spot):** drive an edge spell
  (Fireball id 0) with `fire_left` held across ticks and assert re-fire happens
  while the burst is live (the `(armed && id != 22)` gate is gone), and that
  Global Death (22) still stacks. Plus a World-level hold/re-cast-refresh
  assertion for Accelerate (extend world.rs:8584).

## 5. Golden impact — NONE

`thrust_cancel` is invoked **only** from `Simulation::step` (lib.rs:276-280);
grep confirms no other src call site. The `state_hash` goldens drive
`World::tick` **directly** (tests/state_hash.rs:93) — they never construct a
`Simulation` and never call `thrust_cancel` — and the scripted run equips
Fireball/Rapid-Fireball, not Accelerate (state_hash.rs:120-131). The World hash
also gates `accel`/`accel_held`/`speed_boost` behind "armed" (world.rs:301,304).
So the fix cannot move any MC1 golden. The pre-existing World test
`accelerate_directions_are_mutually_exclusive` (which manually calls
`thrust_cancel(1.0)` to brake **backward** accel = resisting = faithful) stays
valid.
