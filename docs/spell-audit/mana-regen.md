# Mana-regeneration blocking (general note 2 — BOTH games)

## TL;DR

Player report is **CONFIRMED and correct for both MC1 and MC2.** In retail, an
**active spell burst suppresses mana regeneration**: the caster's signed regen
accumulator (MC1 `+132`, MC2 `manaRegen_0x88_136`) is **clamped to 0 every tick
the burst is still alive**, so while you hold fire your mana does not refill and
the per-cast debits actually drain the pool. The port faithfully applies the
**negative debit stamp** (`mana_debit`) but **omits the mid-burst clamp**: every
tick, `on_tick` unconditionally recomputes `mana_delta` back to positive regen
(min 100 afield / 1000 at castle), so one tick after each fireball the regen
resumes at full rate and out-paces the debit. Net: holding L1 fireballs *gains*
mana instead of losing it.

The retail routines are structurally **identical across both games**
(`sub_55E80` for MC1, `sub_68DE0` for MC2): first-burst-tick stamps
`regen = -(cost)`; every subsequent burst tick clamps `regen = 0`. The one
historical wrinkle: **remc1 ships the MC1 negative stamp commented out** (a
maintainer "fix" for infinite fireballs) — but remc2 keeps the MC2 stamp live,
and our port already restores the debit for MC1. What is missing in **both** is
the clamp. The fix is one added step: after the positive regen recompute, if any
of the player's spell manifestations has a live-but-not-first-tick burst
(`f26 > 0`), clamp `mana_delta` to `min(0)`.

---

## MC1

### 1) Retail mechanism

**The per-tick wizard mana tick — `sub_main.cpp:55385` (remc1).** Regen is a
signed accumulator `+132` (`var_u16_29927_132`, Basic.h declares it u16 → must
be treated as **i16/signed**) that is *added to mana every tick* and then
*recomputed from max-mana*:

```
55385  a1x->mana_140       += a1x->regen_132;          // apply last frame's accumulator
55391  if (mana_140 < 0)    mana_140 = 0;               // clamp low
55394  if (mana_140 > max_136) mana_140 = max_136;      // clamp high
       ...
55407  if (at_own_castle || (flags & 0x10)) {           // fast-regen branch
55409      regen_132 = max_136 / 200;
55411      if (regen_132 < 1000) regen_132 = 1000;      // floor 1000 at castle
       } else {                                         // slow-regen branch (afield)
55417      regen_132 = max_136 / 2000;
55419      if (regen_132 < 100)  regen_132 = 100;        // floor 100 afield
       }
```

So each frame: **add the accumulator, then rebuild it as a positive regen
value.** On its own this refills mana forever.

**The suppression — `sub_55E80` (`sub_main.cpp:64936`, "SYNCHRONIZED WITH
REMC1").** Called during each spell EVENT's tick (`a1` = spell event, `a2` =
caster). `+48`/`+50` are the burst life counter and its initial value:

```
64941  v2 = a1->burst_48;
64942  if (v2 == a1->burstMax_50) {                      // FIRST tick of the burst
64944      v3 = a2->regen_132;
64948      // if (v3 >= 0) a2->regen_132 = -(a1->cost_136);   // stamp NEGATIVE
64950      // else        a2->regen_132 = v3 - a1->cost_136;  // deepen if already neg
           return 1;
       } else {                                          // MID-burst (every later tick)
64956      if (v2 && a2->regen_132 > 0) a2->regen_132 = 0; // *** SUPPRESS REGEN ***
           return 0;
       }
```

**The two halves:**
- First burst tick → `regen_132 = -(cost)` (or `-= cost`). Because the wizard
  tick adds `regen_132` next frame, this is the cast's mana deduction.
- Every subsequent tick the burst is alive → `regen_132` clamped to 0, so the
  positive value the wizard tick just recomputed at `:55409/:55417` is thrown
  away → **no regen while the spell is active.**

Order within a frame: wizard tick (recompute → positive) runs first; the spell
event tick (`sub_55E80`) runs later in the same frame and overwrites the
accumulator to negative (first tick) or 0 (mid-burst). Whatever it leaves is
what gets added next frame.

Effect on held fireballs: a fireball event lives ~4 ticks (click mode: 1 shot +
4-tick tail, `:65086`/`:8188`). While you hold fire you are continuously
mid-burst, so `regen_132` is pinned at 0 tick after tick and the per-shot
`-(cost)` stamps accumulate as real drain.

> **Historical note:** remc1's maintainer **commented out** the negative stamp
> (the `//fix` at `:64945-64951`), leaving only the mid-burst clamp live —
> which is why vanilla remc1 has effectively free-but-non-regenerating fireball
> spam. Recorded ORIGINAL gameplay (senior source) had **both** halves. Our port
> already re-enabled the debit; the clamp is the remaining gap.

### 2) Current port

`crates/mgc-sim/src/mc1/world.rs`:

- **Per-tick regen** (`on_tick`), `world.rs:1218-1229` — the faithful port of
  `:55385`:
  ```
  1218  let stepped = self.player.mana + self.player.mana_delta;   // apply last delta
  1219  self.player.mana = stepped.clamp(0, mana_max);
  1225  self.player.mana_delta = if at_castle { (mana_max/200).max(1000) }
  1228                           else         { (mana_max/2000).max(100) };  // ALWAYS positive
  ```
- **The debit** — `mana_debit`, `world.rs:2851-2861` (the port of `sub_55E80`'s
  *first-tick* half; dev_spells short-circuits):
  ```
  2856  if self.player.mana_delta >= 0 { self.player.mana_delta = -c; }
  2859  else                          { self.player.mana_delta -= c; }
  ```
  Called on the first-fire tick from `cast_spell` (`world.rs:2350, 2376, 2422,
  2432`), gated by the manifestation burst `f26` (the port of `+48`, comment at
  `world.rs:2059`).

**Why it out-regenerates spam:** the negative stamp *is* applied — but only the
`else` (mid-burst suppression) half of `sub_55E80` is missing. Line 1225
recomputes `mana_delta` back to a positive floor (≥100 afield, ≥1000 at castle)
**every tick, unconditionally**, including all the tail ticks of a live burst.
So the sequence per fireball is: spawn tick → `mana_delta = -cost`; next tick →
`mana += -cost` then `mana_delta` recomputed positive; the following tail/idle
ticks → `mana += +regen`. Over one click cycle the net is `-cost + k·regen`,
and with the regen floor the `k·regen` term dominates → mana climbs.

### 3) The gap (MC1)

Missing the mid-burst clamp `if (burst_alive && mana_delta > 0) mana_delta = 0`.
The port suppresses regen for **zero** ticks; retail suppresses it for **every**
tick a spell burst is live. Debit is present and correct; suppression is absent.

### 4) Fix data (MC1)

- **Regen law (already correct, keep):** `mana += mana_delta`; then
  `mana_delta = at_own_castle ? max(mana_max/200, 1000) : max(mana_max/2000, 100)`.
- **Suppression condition to ADD:** after the regen recompute (or, faithfully,
  after cast handling each tick), for the human caster:
  `if !dev_spells && any player manifestation has f26 > 0 (burst alive and past
  its first-fire tick this frame) && mana_delta > 0 { mana_delta = 0 }`.
  This is verbatim `sub_55E80:64956`. The `> 0` guard preserves a fresh
  first-tick negative debit; dev_spells stays exempt (infinite mana pin).
- **"Active spell" marker:** the manifestation entity's burst counter
  `f26` (retail `+48`), already tracked per spell in `cast_spell`.

---

## MC2

### 1) Retail mechanism

**The per-tick mana tick — `EventsFunctions.cpp:5426` (remc2).** Byte-identical
shape to MC1, on the MC2 field names:

```
5426  a1x->mana_144 += a1x->manaRegen_136;              // apply accumulator
      ...
5438  if (at_own_castle || (flags & 0x10)) {
5440      manaRegen_136 = maxMana_140 / 200;
5442      if (manaRegen_136 < 1000) manaRegen_136 = 1000;  // castle floor
      } else {
5448      manaRegen_136 = maxMana_140 / 2000;
5450      if (manaRegen_136 < 100)  manaRegen_136 = 100;    // afield floor
      }
5453  if (mana_144 < 0)   mana_144 = 0;
5456  if (mana_144 > max) mana_144 = maxMana_140;
```

**The suppression — `sub_68DE0` (`EventsFunctions.cpp:55569`).** Same structure
as MC1's `sub_55E80`, and here the negative stamp is **live** (not commented
out). `word_0x2E_46`/`word_0x30_48` are the burst counter/its initial value:

```
55569  sub_68DE0(a1x /*spell event*/, a2x /*caster*/):
       v2 = a1x->word_0x2E_46;
       if (v2 == a1x->word_0x30_48) {                    // FIRST tick
           if (a2x->manaRegen_136 >= 0) a2x->manaRegen_136 = -a1x->maxMana_140;  // stamp -(cost)
           else                         a2x->manaRegen_136 = v3 - a1x->maxMana_140; // deepen
       } else {                                           // MID-burst
           if (v2 && a2x->manaRegen_136 > 0) a2x->manaRegen_136 = 0;  // *** SUPPRESS ***
       }
```

`sub_68DE0` is invoked from **every** MC2 spell-effect-state handler, once per
tick per live effect (`EventsFunctions.cpp:55881, 55983, 56007, 56017, 56122,
56261, 56345, 56514, 56528, ...`). So exactly as in MC1: first tick debits the
cost, every later tick of a live spell pins `manaRegen` to 0 → no regen while a
spell is active.

### 2) Current port

MC2 shares the **same** player pool and the **same** per-tick regen as MC1 —
there is no separate MC2 regen path (`on_tick` at `world.rs:1218-1229` runs
regardless of `GameId`; only rivals have their own accumulator at
`mc2/rivals.rs:575`). The MC2 cast machinery is in
`crates/mgc-sim/src/mc2/cast.rs`:

- **Afford** — `mc2_afford`, `cast.rs:583-600` (port of `sub_68D50`): alive +
  castle-upkeep check + `player.mana >= e.max_life` on the first tick.
- **First-tick debit** — `mc2_cast_tick`, `cast.rs:614-620`:
  ```
  616  if f26 == f28.max(1) {                 // first tick
  617      self.mc2_spell_fire(...);
  618      let cost = self.g.ent[m].max_life;
  619      self.mana_debit(cost);             // shared world.rs:2851 negative stamp
  }
  626  self.g.ent[m].f26 -= 1;                 // burst countdown
  ```

**Why it out-regenerates spam:** identical root cause to MC1. `mana_debit`
applies the negative stamp on the first tick, but the shared regen recompute at
`world.rs:1225-1229` rebuilds `mana_delta` positive every tick, and
`mc2_cast_tick` never re-invokes the equivalent of `sub_68DE0`'s **else** branch
on the mid-burst ticks (it only decrements `f26`). So the port implements the
`if` (first-tick stamp) half of `sub_68DE0` but not the `else` (mid-burst clamp)
half.

### 3) The gap (MC2)

Same as MC1: the mid-burst regen clamp `if (burst_alive && mana_delta > 0)
mana_delta = 0` is absent. In MC2 the burst counter is `f26`
(`word_0x2E_46`); mid-burst = `f26 > 0 && f26 != f28.max(1)`.

### 4) Fix data (MC2)

- **Regen law (shared, keep):** same as MC1 (§MC1.4).
- **Suppression condition to ADD:** in `mc2_cast_tick` (`cast.rs:608`), for each
  spell whose manifestation `m` has `f26 > 0` after this tick's processing but
  was **not** first-fired this frame, apply `if mana_delta > 0 { mana_delta = 0 }`
  (dev_spells exempt). This is verbatim `sub_68DE0:else`
  (`EventsFunctions.cpp` mid-burst branch). Equivalent to running the
  suppression once per live player spell effect, as retail does.
- **"Active spell" marker:** the manifestation's `f26` burst counter
  (retail `word_0x2E_46 / +46`), already tracked in `mc2_book`/`mc2_cast_tick`.

---

## Shared implementation note

Because both games route through the **same** `player.mana_delta` accumulator
and the **same** `on_tick` regen recompute (`world.rs:1218-1229`) and the same
`mana_debit` (`world.rs:2851`), a **single** clamp step services both. Suggested
placement: after cast handling each tick (so first-tick debits stamped this
frame survive), add —

```
if !self.dev_spells && player_has_live_spell_burst && self.player.mana_delta > 0 {
    self.player.mana_delta = 0;
}
```

where `player_has_live_spell_burst` = any player manifestation with `f26 > 0`
that is not on its first-fire tick this frame (MC1: scan `player.owned[..]`;
MC2: scan `mc2_book.ent[..]`). This is faithful to both `sub_55E80:64956` and
`sub_68DE0` mid-burst branches. Keep the `> 0` guard so a same-frame first-tick
negative debit is not wiped.

---

## Confidence

**High** on the retail mechanism and the gap — both routines are quoted verbatim
from the vendored decompiles (`sub_main.cpp:55385/64936`,
`EventsFunctions.cpp:5426/55569`), both are structurally identical, and the port
demonstrably omits only the `else`/mid-burst branch while faithfully shipping the
`if`/first-tick debit.

## Open questions

1. **Exact "burst alive" window per spell.** The clamp keys on `+48`/`+46`
   (burst counter) being non-zero and past its first value. For multi-tick tail
   spells (fireball's 4-tick tail, streams, channels) this is clear; verify that
   *instantaneous* one-tick spells (`f28 == 1`) don't linger a spurious extra
   tick under the port's `f26 -= 1` ordering (they should immediately expire, so
   no suppression tick — matches retail where `v2 == burstMax` is the only tick).
2. **remc1 vs recorded original for MC1.** The negative stamp is commented out in
   remc1; we treat recorded gameplay as senior and keep the debit. Worth a
   recorded-capture cross-check that MC1 held fireballs *do* deplete personal
   mana (the report asserts they should).
3. **Rivals.** AI casters have their own `mana_delta` (`mc2/rivals.rs:183,575`
   and the MC1 rival path); confirm whether they need the same clamp or already
   avoid the spam pattern (they cast on cooldowns, rarely held).

## Suggested test

Add a sim regression (both games): spawn the player afield (slow-regen branch),
give a known-cost L1 fireball, drive the input as **held fire for N ticks**
(e.g. N = 120, spanning many click cycles), and **assert `mana_after <
mana_before`** — strictly monotone-ish net drain. Today this fails (net gain);
after adding the mid-burst clamp it should pass. A second assertion: with fire
**released**, mana strictly increases (regen resumes), guarding against
over-clamping. Pin/refresh the MC2 state-hash goldens after the change (MC1
goldens are MC1-gated by the clamp's `game`-agnostic but burst-gated condition,
so they should be unaffected when not firing — verify).
