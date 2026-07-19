# Spell audit — Speed (index 3)

**TL;DR.** Retail's Speed spell overrides the carpet's travel speed to
`sign · minSpeed · subSpellIndex` for the armed window, where
`subSpellIndex` is **tier-dependent = {2, 3, 4}** (SPELLS.DAT row 3) and
`minSpeed = 80` for the carpet — i.e. sustained **160 / 240 / 320** at
tiers 0/1/2 (with a one-tick spike of `subSpellIndex+1` → 240/320/400 on
the first tick, then settle). It is a **fixed-duration armed window**
(`word_0x18` = 301/451/501 ticks), **not** a held/released channel — MC2
has no "hold the button for max speed" mechanic. The current port maps
spell 3 onto MC1's Accelerate channel with a **fixed 3.0 (held) / 2.0
(released) × 80** factor and **no tier scaling** — so the peak speed is
identical at all three tiers (~240 held / 160 released). The DURATION is
already tier-scaled correctly (port reads `word_0x18` into `f28`), which
is what makes the spell "feel" progressive; the **magnitude is the bug**.
Cast sound 19 is correct.

---

## 1. Spell identity / data fields (SPELLS.DAT row 3)

From `baked/assets/mc2-*/spells.bin`, row 3 (`byte_0 = 3` → 3 tiers,
`enabled = 12`), parsed by `crates/mgc-sim/src/mc2/spells.rs`:

| tier | `subSpellIndex_2` | `manaCost_6` | `maxManaLimit_A` | `xpos1` | `xpos2` | `word_0x18` | `life_0x1A` |
|------|-------------------|--------------|------------------|---------|---------|-------------|-------------|
| 0    | **2**             | 1000         | 0                | 0       | 0       | **301**     | 0           |
| 1    | **3**             | 2500         | 0                | 400     | 125     | **451**     | 0           |
| 2    | **4**             | 5000         | 20000            | 1200    | 370     | **501**     | 0           |

- `subSpellIndex_2 = {2,3,4}` — **the per-tier speed multiplier** (see §2).
- `word_0x18 = {301,451,501}` — the cast **duration** (tick count) and the
  mana-per-tick divisor; `SetSpell_6D5E0` copies it into `word_0x30_48`
  (Level.cpp:1518). Port maps this to `f28` (cast.rs:246). `life = 0` at
  all tiers (not used by this effect).

## 2. Retail law — `GetScroll_69DB0` (EventsFunctions.cpp, the model-3 EFFECT state)

The class-15 Speed manifestation's per-tick effect. `a1x` = spell entity,
`v1x` = caster (`parentId_0x28_40`). Armed timer `word_0x2E_46` counts
DOWN from `word_0x30_48` (= duration). Cites are `EF:` line numbers:

```
v10 = (caster.model==1) ? 64 : 2;                        // EF:56208-11 (2 for the human)
v2  = (caster.speed_0xc_12 >= 0) ? 1 : -1;               // EF:56212-15  direction = current travel sign
...
if (word_0x2E_46 == word_0x30_48) {                      // FIRST tick  EF:56241
    sub_6D8B0(parentId, 3, 1);                           //   XP += 1 for spell 3 (EF:58228, one-shot)
    caster.speed_0xc_12 = v2 * minSpeed * (subSpellIndex_2 + 1);   // spike  EF:56244
} else {
    caster.speed_0xc_12 = v2 * minSpeed *  subSpellIndex_2;        // sustain EF:56248
}
caster.actSpeed_0x82_130 = caster.speed_0xc_12;          // EF:56250 — hard override of BOTH speed and actSpeed
...
v8 = --word_0x2E_46;                                      // EF:56263-64
if (!v8) { caster.speed_0xc_12 = minSpeed * v2;          // LAST tick: restore to 1x  EF:56267-68
           caster.actSpeed = speed; byte[0] &= 0x7F; }
```

Numeric result with `minSpeed = 80` (`x_DWORD_D4B8C`, confirmed by
`docs/traces/mc2-flight-model.md:464` — "80 … also `minSpeed_0x84_132`"):

| tier | subSpell | **sustained** speed | first-tick spike |
|------|----------|---------------------|------------------|
| 0    | 2        | **160** (2×)        | 240 (3×)         |
| 1    | 3        | **240** (3×)        | 320 (4×)         |
| 2    | 4        | **320** (4×)        | 400 (5×)         |

Key retail properties:
- **Tier-scaled magnitude** via `subSpellIndex_2 = {2,3,4}`.
- **Armed-window, timed** — runs for `word_0x18` ticks (301/451/501),
  overwriting `speed_0xc_12`/`actSpeed` every tick, then auto-restores to
  1× `minSpeed` on the final tick. **No held/released 3/2 distinction.**
- **Direction follows current travel** (`v2 = sign(speed_0xc_12)`), so the
  boost applies forward or backward depending on how you were moving.
- Cast **sound 19** at first tick (EF:56230). Visual: `(10,2)` sphere
  (EF:56253). Boost flag `byte[0] |= 0x80` (EF:56229).
- Confirmed as the Speed effect by the trace bank:
  `docs/traces/mc2-player-cast-path.md:233` — "writes
  `speed_0xc_12 = ±minSpeed·(subSpellIndex+1)`, `actSpeed = speed` … XP
  idx 3; spawns visual (10,2) | sound 19".

## 3. Current port — how `accel` is applied

`crates/mgc-sim/src/mc2/cast.rs:747-752` — the direct-effect arm for spell 3:
```
3 => {
    self.player.accel = 1;
    self.player.accel_held = true;
    self.mc2_award_xp(PLAYER_TARGET, 3, 1);
    self.g.snd_player(19);
}
```
This drops onto MC1's Accelerate channel:
- `crates/mgc-sim/src/mc1/world.rs:1846-1848`:
  `speed_boost = (accel_held ? 3.0 : 2.0) · accel.signum()` — **fixed
  3.0 held / 2.0 released**, no tier input.
- `world.rs:3260` `accel_override()` exposes it; the sim feeds it as
  `accel_over` into flight.
- `crates/mgc-sim/src/flight.rs:476-480` (`mc2_move`, and the identical
  `mc1_move:200-204`): `let v = (k * 80.0) as i16; st.tgt_speed = v;
  st.act_speed = v;` — so peak = **240 (held) / 160 (released)**, base 80
  correct, multiplier fixed.
- Armed-window length is correct: `f26 = f28.max(1)` and `f28 =
  word_0x18` (cast.rs:246, :573); window expiry clears the channel
  (cast.rs:654 `3 => self.player.accel = 0`). So **duration scales
  per tier (301/451/501); magnitude does not.**
- The port additionally carries MC1-only semantics that MC2 lacks: the
  `accel_held` 3.0-vs-2.0 mouse-button gate, the brake `thrust_cancel`
  veto (`world.rs:3270`), and MC1's separate burst-count expiry.

## 4. Gap (quantified, minSpeed = 80)

| tier | retail sustained | port held (accel_held) | port released | delta at that tier |
|------|------------------|------------------------|---------------|--------------------|
| 0    | **160** (2×)     | 240 (3×)               | 160 (2×)      | held **+50%** too fast |
| 1    | **240** (3×)     | 240 (3×)               | 160 (2×)      | held matches; released 33% slow |
| 2    | **320** (4×)     | 240 (3×)               | 160 (2×)      | **25% too slow** (spike 400 → 40% slow) |

The port's peak speed is **the same at all three tiers** (240 held). It
should climb 160 → 240 → 320. Net: tier 0 is too fast, tier 2 is capped
short. The player's impression that "levels speed up progressively" is
being carried entirely by the **duration** ladder (correctly ported), not
by speed. Secondary gaps: (a) held/released 3-vs-2 is a fabricated MC2
mechanic — retail magnitude is constant across the window; (b) MC1's
brake-cancel/burst-expiry can end the boost differently than retail's
pure `word_0x18` countdown.

## 5. Fix data (exact law to port)

Replace the fixed 3.0/2.0 factor for MC2 spell 3 with the tier value:

```
speed = sign(current_travel) · minSpeed(80) · subSpellIndex[tier]
        where subSpellIndex[tier] = row.tiers[tier].sub_spell = {2, 3, 4}
```
- The tier row is already available in `mc2_spell_fire` as
  `sub.sub_spell` (cast.rs:699). Feed **`sub.sub_spell` (2/3/4)** as the
  accel factor instead of the fixed 3.0/2.0 — keep the `· 80` base and the
  sign. This lands the sustained 160/240/320 exactly.
- Magnitude is **constant for the whole armed window** — drop the
  `accel_held` 3.0-vs-2.0 branch for the MC2 path (retail has no
  button-held distinction; it is purely time-driven).
- Optional fidelity nicety: on the very first tick emit
  `subSpellIndex+1` (spike 240/320/400) for one tick, then settle to
  `subSpellIndex`. Low visual impact (1 tick); land the sustained value
  first.
- Restore to 1× `minSpeed` on window expiry (retail EF:56267; the port
  already zeroes `accel` at expiry — that reverts to normal thrust, which
  is close but not the explicit `speed = minSpeed` write).
- XP (`mc2_award_xp(…,3,1)`) and sound 19 are correct — keep.

## 6. Confidence + suggested test

**Confidence: HIGH** on the law and factors. `GetScroll_69DB0` is
unambiguous (explicit `speed = v2·minSpeed·subSpellIndex` writes at
EF:56244/56248/56267), `subSpellIndex = {2,3,4}` is read straight from
`spells.bin` row 3, `minSpeed = 80` is confirmed by the flight-model
trace (:464), and `sub_6D8B0` is verified as the spell-3 XP bump
(EF:58240-45). **Flag (medium):** confirm the MC1 accel channel's
burst/brake-cancel expiry doesn't fire before the `word_0x18` window ends
in the current port (two competing expiry timers) — the fix should route
spell 3 off the held-channel semantics entirely.

**Test:** cast Speed at each of the three tiers on the human carpet and
log peak `actSpeed_0x82_130` during the window. Expected retail:
**160 / 240 / 320** sustained (spike 240/320/400 on tick 1), window
length 301/451/501 ticks, then back to 80. Current port will show ~240 at
**all** tiers (or 160 after release). A state-hash golden over a scripted
"cast tier-2 Speed, sample speed at t+50" would pin it.

## 7. Interruptibility (player 2026-07-14) — brake cancels the window

The player reports MC2 Speed must be **interruptible**: as launched it
"flies way further than you need", and there was no way to stop it early
(unlike Shield/Invisibility, whose second cast toggles them off; Speed's
second cast merely re-casts). **Landed 2026-07-14:** a braking thrust
(`World::thrust_cancel`, thrust < 0) now terminates the Speed window —
it zeroes the manifestation burst timer `f26` and runs `mc2_cast_expire`
(stops the boost, lifts the mana-regen suppression that rides `f26`).
A forward press does not cancel; under the MC1 thrust model any press
reaches the `-1.0` call (matching MC1's "any Up/Down press cancels").
Test: `mc2_speed_window_interrupts_on_brake`.

**Faithfulness note:** the literal `GetScroll_69DB0` (§2) hard-overrides
speed every tick with **no** brake input, i.e. the decompile shows no
interrupt. This landed on the strength of the player's gameplay
observation (recorded gameplay is senior over the trace). If a retail
playthrough confirms Speed truly cannot be stopped early, reclassify this
as a P-class playability toggle (faithful default = runs to completion)
rather than the default behavior. **Open — player offered to verify.**
