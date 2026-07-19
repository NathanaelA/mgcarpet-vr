# MC1/HW castle transform freeze under meteor bombardment — investigation

Player report (MC1HW; suspected retail bug too): under heavy multi-meteor
bombardment of an enemy castle that is being repeatedly upgraded/downgraded,
the castle sometimes gets **stuck** — frozen mid-transformation, neither
upgradeable nor destroyable. Player hypothesis: entity-pool overload makes a
spawn fail at a step of the castle-transform sequence, breaking the state
machine.

**Verdict: hypothesis CONFIRMED, and the port is STRICTLY WORSE than retail.**
Retail guards every transform-spawn with `if(spawn_result)` and *retries next
tick* (a temporary stall while the pool is saturated). The port **dropped that
guard** and advances the castle sub-state unconditionally, so a failed spawn
sends the castle into a wait state that waits forever for an entity that was
never created — a permanent deadlock that does NOT self-heal even after the
pool drains.

---

## 1. The castle transform state machine (port)

`Gen::castle_tick` — `crates/mgc-sim/src/mc1/features.rs:3154`. Sub-state field
`f59` (retail `+48`). The transform is driven by two helper entities the castle
spawns and then WAITS on:

- painter, `spawn_creator(42)` → `tick_castle_painter` (features.rs:2577); on
  finish it writes the castle's `f59 = 5` (features.rs:2622).
- leveler, `spawn_creator(41)` → `tick_castle_leveler` (features.rs:2644); on
  finish it writes the castle's `f59 = 2` (features.rs:2708).

Nominal flow:

```
0 (level-up)  -- spawn painter(42)+killbit --> wait 1
   painter finishes ------------------------> 5
5             -- spawn leveler(41)         --> wait 6
   leveler finishes ------------------------> 2
2 ------------------------------------------> 4 (ESTABLISHED)
3 (damage repaint) -- spawn painter(42)     --> wait 1  (then 5 -> 6 -> 2 -> 4)
```

Only state **4 (established)** processes the damage mailbox `mail[0]`, the
downgrade/demolish (`act_life < 0`), and the upgrade mailbox `mail[5]`
(features.rs:3257-3305). States **1 and 6 are pure waits** — the catch-all
`_ => {}` arm at features.rs:3310. A castle parked in 1 or 6 processes NO
damage, NO upgrade, NO demolish.

## 2. The break points — three unguarded spawns (CONFIRMED)

Every place the port spawns a transform helper, the sub-state advance to the
wait state is placed OUTSIDE the `if let Some(...)` success arm:

| state | spawn call | site | unconditional advance | stuck in |
|-------|-----------|------|-----------------------|----------|
| 0 (level-up)   | `spawn_creator(42)` | features.rs:3194 | `f59 = 1` at **features.rs:3211** | **1** |
| 5 (leveler)    | `spawn_creator(41)` | features.rs:3222 | `f59 = 6` at **features.rs:3228** | **6** |
| 3 (repaint)    | `spawn_creator(42)` | features.rs:3242 | `f59 = 1` at **features.rs:3248** | **1** |

`spawn_creator(41|42)` reaches the pool allocator `new_event()`
(features.rs:1588 → :954), which **fail-opens to `None`** when the free stack
is empty (features.rs:955-963, incrementing `exhausted`). Models 41/42 are not
in the early-return effect set, so they genuinely go through the pool.

**Exact stuck sequence** (state 0, the common case): pool is full → painter
`spawn_creator(42)` returns `None` (the `if let Some(p)` body is skipped) →
line 3211 sets `f59 = 1` anyway → the castle sits in wait-state 1 → the painter
that would write `f59 = 5` was never spawned → nothing else touches `f59` (state
1 is `_ => {}`) → **permanent freeze**. Damage/upgrade/demolish mail all accrue
untouched (only state 4 reads them), matching "neither upgraded nor destroyed."
States 5 and 3 are identical with `f59 = 6` / `f59 = 1`.

Aggravating factor in state 0 specifically: the level increment (`f26+1`), HP/
cap/extents commit and the build gong `snd(10)` all run BEFORE the spawn and
unconditionally (features.rs:3173-3186), so the port's level-up is also **not
atomic** with the painter spawn — a second regression vs retail (see §5).

## 3. Retail does NOT do this — it guards + retries (CONFIRMED)

remc1 `sub_46F10_47250` dispatch (`reference/remc1/sub_main.cpp:56051`) →

- case 0 level-up → `sub_47960_47CA0` (:56461): spawns painter via
  `sub_3B7B0` FIRST (`v1 = ...`), and the **entire** commit — sound, level++
  (`+26`), extents, capacity ladder, AND `*(a1+48) = 4` — is inside
  `if ( v1 )` (:56471-93). Spawn fails → nothing changes → next tick retries.
- case 3 repaint → `sub_47020_47360` (:56100): `if ( result ) { … *(a1+48)=4; }`
  (:56107-14). Fail → stays in case 3, retries.
- case 5 leveler → `sub_47080_473C0` (:56119): `if ( result ) { … *(a1+48)=6; }`
  (:56126-33). Fail → stays in case 5, retries.

remc1hw is **byte-identical** at all three sites:
`reference/remc1hw/sub_main.cpp` `sub_47960` :52525-58 (`if ( v1 )` :52535),
`sub_47020` :52164-80 (`if ( result )` :52171), `sub_47080` :52183-99
(`if ( result )` :52190).

So retail's failure mode is a **livelock/stall**: while the pool stays
saturated the castle sits in its active state retrying and is unresponsive, but
it **self-heals** the instant a slot frees. Under *sustained* multi-meteor fire
that never lets a slot free, that stall can look permanent — which is the "known
retail bug" the player recalls. The port converted this recoverable stall into
an **unrecoverable deadlock**: it is a port regression layered on top of the
retail limitation.

## 4. Why meteors trigger it — pool math

Pool size = **1000 slots** in BOTH retail engines and the port
(`ChassisParams::MC1.pool_slots = 1000`, chassis.rs:57; slot 0 unused, so 999
usable). Meteors are the heaviest transient consumer in the game:

- **Meteor is m3 with `fire_trail = true`** (`proj_generic_tick`,
  combat.rs:1398/1417): it drops a fire-seeder (`spawn_effect(1)`, combat.rs:
  1416) **every flight tick**. A meteor crossing the field flies for dozens of
  ticks → dozens of seeders; each seeder (`spreader_tick`, combat.rs:3028)
  spawns a ring of ground fires (`spawn_effect(0)`).
- **Ground fire model 0** (combat.rs:2321) has `max_life = 8` but `fire_tick`
  only decrements life every 4th tick (`f26 & 3` gate, combat.rs:2957), so each
  fire occupies its slot for ~32 ticks.
- **The explosion** spawns a growing **blast ring** (`blast_ring_tick`,
  combat.rs:3070): a full ring of `spawn_effect(0)` fires **per tick**, radius
  cycling `(r+2)%11` (combat.rs:3112) — ring cell count ≈ 8·r, so the larger
  radii alone push tens of fires/tick, each living ~32 ticks.
- Fires that land on flammable terrain/trees ignite **standing fires** (model
  6, `max_life = 240`, combat.rs:2349) which torch neighbours (~5/tick,
  combat.rs:2837) → **forest chain-burn** can hold hundreds of slots.

Order-of-magnitude: a single meteor's trail + explosion + terrain ignition
holds ~100-200 concurrent effect slots at peak. Meteor's burst count is **11**
(`SPELLS[7]`, spells.rs:122) — a full charge looses up to 11 meteors; several
casters/casts bombarding at once trivially exceed the 999-slot pool once the
resident creatures, jars, rival spawns and the castle's own mana balls are
subtracted. When the pool is saturated **at the same tick** the enemy castle
needs its painter/leveler slot, `spawn_creator` returns `None` → §2 freeze.

## 5. Entity pool: size, configurability, and the hash

- **Default = 1000** (`chassis.rs:57`), fail-open exhaustion counted in
  `Gen::exhausted` (features.rs:962) and surfaced by the app
  ("entity pool exhausted … fail-open, as retail", main.rs:1972).
- **Already player-configurable, offline only**: `--pool-slots N` (CLI,
  main.rs:2548) and `cfg.sim.parameters.entity_pool_size` (config.rs:93). CLI
  wins over config (main.rs:2917). Clamp **`2..=60000`** (main.rs:2554) — the
  ceiling is because slot indices are `u16` (`free: Vec<u16>`). Applied at
  world build (main.rs:455) with a stderr note that it is a "limit-removing
  override; G-class — not a faithful run". Default in `mgcarpet.json.defaults`
  is `null` → 1000. Settings registry marks it `Mutability::Startup`
  (settings.rs:187), class `Cheat` (settings.rs:222).
- **Pool size DOES feed the state hash (CONFIRMED)** — contrary to the usual
  assumption. `Gen` is `#[derive(Hash)]` (features.rs:529) and its fields
  `ent: Vec<Ent>` (:537), `free: Vec<u16>` (:539), `exhausted` (:627) and
  `chassis: ChassisParams` (:630) are ALL hashed; `state_hash` calls
  `g.hash()` (world.rs:2395). So:
  - the full `ent` vec (length = `pool_slots`) and the `free` stack (length
    `pool_slots-1`) are hashed → **changing `pool_slots` changes the hash even
    with zero gameplay divergence** (more trailing default entries);
  - `chassis.pool_slots` is hashed directly as well;
  - `exhausted` is hashed → the number of dropped spawns feeds the hash, and
    the LIFO `free` ordering feeds it too.
  Consequence: **raising the pool default would re-pin every MC1 golden.** This
  matches the chassis module doc (chassis.rs:14-18): a bumped pool is the
  divergent "G-class" option, replays must record the set. Keep the default at
  1000.

## 6. Recommendations

### (a) Deadlock fix — retail-faithful, bug-fix class, no opt-out

The port's unconditional state advance is an **unfaithful regression**: retail
already retries. Restore retail semantics so a failed spawn leaves the castle in
its active state to retry next tick. This is a pure faithfulness correction
(not an enhancement / not G-class) — default-on, no toggle.

- **State 5 and state 3** (minimal): move the `f59` write INSIDE the
  `if let Some(...)` arm (features.rs:3228 → inside the :3222 block;
  features.rs:3248 → inside the :3242 block). On failure `f59` stays 5/3 and
  retries — exactly `sub_47080`/`sub_47020`.
- **State 0** (must also become atomic, matching `sub_47960`): spawn the
  painter FIRST; only if it succeeds do the level increment, HP/cap/extents
  commit, `snd(10)` and `f59 = 1`. On failure make NO mutation (no level++, no
  gong) and leave `f59 = 0` so the next tick retries cleanly. The current
  pre-spawn commit (features.rs:3173-3186) would otherwise re-increment the
  level and re-ring the gong every retry tick.

Golden impact: in any run that never exhausts the pool *during a castle
transform*, `spawn_creator` always succeeds and behaviour is identical — hash
unchanged. Only the exact exhaustion-during-transform corner changes, and it
changes toward retail-correct. Treat as bug-fix class: re-pin only if a golden
level (e.g. 032, the known pool-ceiling level) happens to exhaust while a
castle is mid-transform. This does NOT remove retail's stall-while-saturated
behaviour (still faithful) — it only removes the port-specific permanent
deadlock.

Optional hardening (belt-and-braces, still faithful because it can only fire in
a state retail also can't reach cleanly): give wait-states 1/6 a sanity check —
if the awaited painter/leveler helper (the entity whose `f146` points back at
this castle) is absent, fall back to the active state to re-spawn. Not required
if §6a is done, since the castle never enters 1/6 without a live helper.

### (b) Pool size — mitigation, opt-in only

Do **not** raise the default (breaks goldens per §5). The root-cause mitigation
the player wants already exists: `--pool-slots N` / `entity_pool_size`
(2..=60000). Recommend documenting it as the bombardment mitigation and
suggesting e.g. 4000-8000 for heavy-combat play; that removes the exhaustion
events (and un-drops the spawns retail silently discarded) at the cost of
retail-divergence and a re-pinned hash for that run. With §6a in place a full
pool is no longer catastrophic — it degrades to a brief stall — so the bump
becomes a comfort/economy option rather than a correctness requirement.

---

### Confidence

- §1, §2 break points and stuck sequence: **CONFIRMED** (features.rs:3154-3312,
  :3194/3211, :3222/3228, :3242/3248, :3310; helper hand-back :2622/:2708).
- §3 retail guards all three sites + HW identical: **CONFIRMED**
  (remc1 :56100/:56119/:56461; remc1hw :52164/:52183/:52525).
- §4 meteor as heavy pool consumer, freeze causation: mechanism **CONFIRMED**
  (combat.rs trail/blast/standing-fire cites); exact peak slot count is a
  **HYPOTHESIS** (order-of-magnitude estimate, not measured — a live capture of
  `pool_dropped_total` under bombardment would quantify it).
- §5 pool size feeds the hash: **CONFIRMED** (`#[derive(Hash)]` on Gen with
  ent/free/exhausted/chassis fields; world.rs:2395).
