# MC1/MC1HW "monsters & dwellings randomly instantly die" — investigation

Investigated 2026-07-15 (read-only). Player report: in **MC1 + MC1HW (not MC2)**,
monsters and dwellings sometimes INSTANTLY die for no visible reason; the only
observed correlation is that it *might* happen when **balloons fly over them
collecting nearby mana**.

Bottom line: the strongest root cause is a **stale raw entity-slot reference
across a despawn + LIFO slot reuse**, and the balloon mana-collection path is the
concrete carrier that despawns **dwellings** (class-10 houses). Monsters (class 5)
cannot be hit by the balloon path; they are explained by the *same root class*
through a different slot-holder (secondary hypotheses below). The balloon code
itself is a *faithful* port (it is actually stricter than retail) — the bug is the
architecture-wide "raw slot index, no generation guard" pattern, not a
mis-scoped filter.

---

## The mechanism (CONFIRMED in code)

### 1. Slots recycle LIFO and are reaped mid-frame
- `new_event` pops a slot off the free stack (`self.free.pop()`) —
  `crates/mgc-sim/src/mc1/features.rs:954-983`. On reuse it resets the entity,
  and sets `e.id24 = idx` (the SLOT INDEX) — **there is no generation/version
  counter** (`features.rs:973`). So `id24` cannot distinguish "this slot, this
  incarnation" from a later reuse.
- `free_entity`/`free_slot` push the freed slot back (`features.rs:1051-1055`,
  `world.rs:5574`) — a LIFO stack, so a just-freed slot is the **next** one handed
  out.
- The runtime tick loop reaps despawned entities **immediately, inside the
  per-entity loop** (`world.rs:1843-1845`: `if flags & 0x400 { free_slot(i) }`).
  So a slot freed while entity *i* ticks can be reused by a later entity's spawn
  **within the same frame**.

This is **faithful to retail**: the reference main loop
`sub_36620_369E0` reaps in-loop too (`reference/remc1/sub_main.cpp:43264-43265`,
again `43355-43356`: `if (byte[1] & 4) sub_41E90_421D0(...)`), and the chassis
survey recorded the retail allocator as "two-stack, 999→1 build, LIFO"
(`crates/mgc-sim/src/chassis.rs:6`). So the *exposure* exists in the original as
well — this is a latent retail bug the port reproduces, not a port regression in
the reap/alloc machinery.

### 2. The balloon holds a raw ball-slot index and validates it only by class
MC1/MC1HW balloons run `balloon_tick`→`balloon_move`
(`features.rs:3048-3137`); MC2 runs the separate but structurally identical
`mc2_balloon_tick` (`crates/mgc-sim/src/mc2/castle.rs:758-887`). Dispatch split:
`world.rs:1805-1809` (MC1/HW → `balloon_tick`, MC2 → `mc2_balloon_tick`).

The dispatcher `castle_balloons` picks the **nearest own (10,39) mana ball** and
stores its **slot index** in the balloon's `f146`
(`features.rs:3017-3035`; filter `class64 != 10 || model65 != 39 || … || f144 != own`
at `:3019`). So the picked target is always a real mana ball *at pick time*.

Each tick, `balloon_move` re-reads that raw index and validates it with **only**:
```
let t = self.ent[i].f146 as usize;              // features.rs:3072 — RAW index
if t == 0 || self.ent[t].flags & 0x400 != 0 { return; }   // :3073 — despawn bit only
...
if self.ent[t].class64 == 10 {                  // :3086 — CLASS only, NOT model
    if self.ent[t].f144 != own { step = false }  // :3087 — owner gate
    else { ... if self.ent_overlap(i, t) {        // :3096
        self.ent[i].f140 += self.ent[t].f140;     // :3097-3099 absorb cargo
        ...
        self.ent[t].flags |= 0x400;               // :3103 — DESPAWN the target
    } } }
```
There is **no `model65 == 39` check and no identity/generation check**. So if slot
`t` was reused between the dispatcher pick and this tick by a **class-10 dwelling
(house `(10,45)`, `f144 == own`)** — or any owned class-10 entity: grave `(10,40)`,
mana magnet `(10,54)`, etc. — the balloon flies to it and, on overlap, "absorbs"
it and sets `flags |= 0x400` → the dwelling **instantly despawns with no visible
cause**. Exactly the reported symptom, exactly the reported correlation.

The staleness window is real: `castle_balloons` re-picks only every other tick and
only while the balloon has cargo room (`features.rs:3005-3011`), and mana balls
churn constantly (collection + merge), so a ball slot is freed and LIFO-reused
frequently. The house→same-slot coincidence is *rare*, matching "random."

### 3. This is faithful to retail (the port is even safer here)
Reference `sub_47F90_482D0` (`reference/remc1/sub_main.cpp:56717-56811`):
- `:56735-36` dereferences the target from `*(a1+146)` and only checks it is
  non-zero — it does **not** even check the target's despawn flag (the port added
  that guard at `:3073`).
- `:56742` `if (v2 == 10)` — CLASS 10 only, no model check (identical to the port).
- `:56766` overlap `sub_11950` → `:56768` absorb cargo → `:56773`
  `sub_41E80_421C0(v1)` despawns the target.

So the retail balloon is *less* guarded than the port. The port did not introduce
the class-only filter — it inherited it. Fixing it is a **P-class (better-than-
faithful) safety improvement**, not a fidelity correction.

### 4. Why MC2 doesn't show it
The MC2 balloon code (`mc2/castle.rs:772,790`) has the **same** class-only check
and the same `flags |= 0x400` despawn — so MC2 is *equally* exposed *in code*. The
asymmetry is the **economy, not the balloon code**: MC1/MC1HW have player-owned,
stationary class-10 dwellings `(10,45)` sitting in balloon range (the huts that
generate mana); MC2's mana/building model does not park owned `(10,45)` houses in
the balloons' collection field, so the reused-slot target is almost always another
sphere and the despawn is unobservable. MC1HW shares MC1's economy → shows it.
(HYPOTHESIS — economy-level, not code-cited; confirm by checking whether MC2
levels spawn owned `(10,45)` entities.)

---

## What was RULED OUT (CONFIRMED)

- **Mis-scoped balloon *pick*** — the dispatcher filters to `(10,39)` mana balls
  (`features.rs:3019`); it never targets a house directly. Only slot reuse gets a
  non-ball into `f146`.
- **Mana-ball merge** — filtered to `class64 == 10 && model65 == 39`
  (`combat.rs:3305-3317`); cannot despawn a house or monster.
- **Jar prune (RIVALS-POLISH #3, the default-on enhancement)** — runs only inside
  `class12_tick` and only sets `flags |= 0x400` when `model65` is a spell the
  player already owns (`world.rs:3416-3422`, dispatch `world.rs:1829`). It is
  **class-12-gated**, so it cannot kill houses (class 10) or monsters (class 5).
  Its only side effect on this bug is *more* despawn/free churn (more LIFO reuse).
- **Damage mail / `area_write`** — recipients are found by a fresh live-tile scan
  and gated by owner-immunity + damageable + vulnerability mask + class/model
  filter + AABB (`combat.rs:130-207`); mailboxes are cleared by `new_event`
  (`features.rs:967`), so a reused slot never inherits stale damage. Not a
  cross-tick stale-ref vector.
- **Recent commits (sessions A-D, 2026-07-15)** — the review commits touched only
  `crates/mgc-sim/src/mc1/world.rs` (+387 lines, additive/tests). No change to the
  reap loop, the balloon, or the allocator. This bug is **pre-existing**, not a
  session A-D regression.

---

## Monsters (class 5) — secondary hypotheses

Monsters and villagers/settlers are **class 5** (`mobs.rs:398`; villagers =
`(5,12/13/14)`), so the balloon's class-10 branch **cannot** despawn them (a
class-5 target falls into the `else`/castle branch — deposit, no kill;
`features.rs:3112-3128`). "Monster deaths" therefore need a different carrier of
the *same* root class (stale raw slot index over a LIFO-reused slot):

- **HYPOTHESIS M1 (identity-vs-index confusion):** several sites re-validate a
  stored slot with `f146 == ent[j].id24 || f146 == j as u16` (`combat.rs:877`,
  `:1606`). Because a fresh entity has `id24 == slot index` (`features.rs:973`),
  the two arms are indistinguishable for a reused slot, so any code that *targets*
  off such a stored index can lock onto the new occupant. NOTE: the two cited
  sites are accuracy-stat/effect-anchor only — the projectile's lethal `hit`
  comes from a *fresh* proximity scan, so those particular lines do **not** deliver
  damage to a stale slot. The hypothesis is that a *different* holder that DOES
  drive a lethal write off a stored index exists; it needs to be found. Not
  confirmed — the concrete monster-kill carrier is still OPEN.
- **HYPOTHESIS M2:** rival-AI target slot references held across ticks
  (`rivals.rs` state targets) landing an attack on a reused slot. Lower priority,
  unverified.
- **Also plausible:** the player is loosely labeling despawned class-10 denizens
  (e.g. the huts' output, graves, or other owned class-10 effects the balloon can
  eat) as "monsters," in which case candidate #1 covers both symptoms and there is
  no separate monster vector. Worth confirming with the player.

These share the fix direction with the dwelling case: **validate entity references
by identity, not raw index.**

---

## Ranked root-cause candidates

1. **Balloon mana-collection despawns a LIFO-reused class-10 dwelling** (dwellings).
   Mechanism CONFIRMED in code (`features.rs:3072-3103`) and shown faithful to
   retail (`sub_47F90` `:56742-56773`). The specific reused-slot coincidence is a
   HYPOTHESIS pending a repro. **This is the top candidate and directly matches the
   player's balloon correlation.**
2. **A yet-unfound stale-index lethal write** (monsters). The class-5 monster
   deaths are NOT explained by any confirmed path — the balloon cannot reach class
   5, and the two `f146 == id24 || f146 == index` sites (`combat.rs:877,1606`) are
   stat/anchor only, not lethal. OPEN: either an unfound raw-index kill carrier, or
   the player is labeling despawned owned class-10 entities as "monsters"
   (candidate #1 would then cover everything).
3. **General class:** any raw-slot entity reference surviving a despawn, because
   the port (faithfully) has no per-slot generation counter (`features.rs:973`).

---

## Recommended fix

**Minimal / targeted (top candidate, goldens-safe):** in `balloon_move`
(`features.rs`) and `mc2_balloon_tick` (`mc2/castle.rs`), require the tether/absorb
target to be an actual mana ball before entering the class-10 branch — add
`&& self.ent[t].model65 == 39` alongside the `class64 == 10` test (`features.rs:3086`,
`castle.rs:772`). The dispatcher only ever assigns `(10,39)` balls to `f146`, so
this changes **no legitimate behavior**; it only prevents the balloon from
"absorbing" a reused-slot house/grave/magnet. This is a P-class improvement (retail
lacks the guard), so it must be variant-gated exactly like RIVALS-POLISH #3 if the
MC1 goldens turn out to touch it — but because the guard only fires in the
already-broken reused-slot case, the goldens should be unaffected (verify by
re-running the MC1 state-hash goldens; if unchanged, it can be unconditional).

**Robust / root fix (covers monsters too):** add a `u16` **generation** field to
`Ent`, bump it in `free_slot`/`new_event`, and store `(slot, gen)` wherever a raw
slot index currently survives across ticks — balloon `f146`, projectile `f146`,
rival target refs. Validate `ent[t].gen == expected_gen` before use; treat a
mismatch as "target gone." This closes the entire stale-ref class. Higher effort
and it must be hash-excluded (the generation field must not feed the state hash, or
it re-pins every golden) — follow the established "hash-feeding fields go last"
discipline.

## Suggested repro / diagnostic

- **Repro sim test (deterministic):** spawn an own castle + one balloon + one own
  `(10,39)` mana ball; capture the ball's slot; force the ball to despawn
  (`flags |= 0x400`) and immediately spawn a `(10,45)` house with `f144 == own` so
  `new_event` hands back the SAME slot (LIFO) at the balloon's target position;
  tick to overlap; assert the house is **not** `flags & 0x400`. Without the fix the
  house despawns; with the `model65 == 39` guard it survives. This both proves the
  mechanism and locks the fix.
- **Cheap in-app diagnostic for a playtest build:** in `balloon_move`/
  `mc2_balloon_tick`, right before the absorb (`features.rs:3096`), assert/log when
  `self.ent[t].class64 == 10 && self.ent[t].model65 != 39` (or, more broadly,
  when the target `model65` differs from the value the dispatcher last stored).
  Any hit is a smoking gun and will name the victim's class/model.
- For the monster (M1) hypothesis: add the analogous log at the projectile
  explode site (`combat.rs:1606`) when the raw-index arm (`f146 == j`) matches but
  the id24 arm does not — that flags a stale-slot re-lock.
