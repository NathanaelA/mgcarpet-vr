# MC1 ambient-fire damage (burning trees, volcano lava)

Decompile trace of retail MC1 (`reference/remc1/sub_main.cpp`) establishing how
"ambient" fire — burning trees and volcano lava — deals damage, and the two
ways our port diverged. Cross-checked against the MC2 path (which was correct)
and the raw ctors.

Field naming: `f80/f82/f84` = AABB half-extents (x / y / z); `f78` = z-center
offset; `f44` = damage amount; `f28` = victim channel-vulnerability mask;
`id24` = owner (owner-equality is the only friendly-fire rule); `f66/f67` =
writer target filter (`0xFF` = wildcard).

## The entities

| entity | class/model | ctor | tick |
|---|---|---|---|
| ground/blast fire (fireball) | 10 / 0 | `sub_3A490` :46454 | `sub_24F60` :28047 (one-shot ch0) |
| **standing / tree fire** | **10 / 6** | **`sub_3A730` :46620** | **`sub_252D0` :28199 (per-tick ch0, /10 tree discount)** |
| lava bomb | 10 / 16 | `sub_3ACC0` :46958 | `sub_25A60` :28573 (ballistic; deposits a standing fire) |

## Retail spec (what each fire has at spawn)

`sub_37130_374F0(e, horiz, vert)` (:43790) sets `f80=f82=horiz, f84=vert`.

- **Standing fire `sub_3A730`** (:46620-47): `f44 = 50`, `maxLife = 240`,
  **`sub_37130_374F0(v2, 272, 1536)` → f80=f82=272, f84=1536** (:46643),
  sprite 228, flags clear `0x8`+`0x20000` then set the static-draw bit; `0x10000`
  (damage-suppress) stays clear; `id24`/`f66`/`f67` = `NewEvent` defaults
  (own-slot owner, `-1/-1` wildcard filter). Tick `sub_252D0` writes
  `sub_124F0(e, 0, f44=50)` **every tick** (:28255-56) — the /10 writer, so
  neighbor trees take 5/tick, the player and creatures take the full 50/tick.
  The tall `f84 = 1536` (6 tiles) is what lets a ground flame reach the flying
  carpet vertically.
- **Fireball ground fire `sub_3A490`** (control): `f44 = 400`, `maxLife = 8`,
  **`extents(128, 128)`** (:46476), tick `sub_24F60` writes once via `sub_120B0`
  (no tree discount), latched by `flags&2`.
- **Lava bomb `sub_25A60`**: **no in-flight area damage** anywhere in the tick.
  On landing (not water) it spawns a standing fire (`sub_3A730`) and overrides:
  `id24 = bomb.owner`, `actLife = 30`, **`f44 = 3 × the fresh fire's own ctor
  f44 = 3 × 50 = 150`** (:28642-45). The bomb ctor's `f44 = 200` is a dead
  store. Volcano source `sub_25EC0` sets the bomb's owner from the source
  (:28798), so a natural volcano's fire is not player-immune.

## Delivery (`sub_124F0` :17399 → our `area_write` combat.rs:132)

Per-tile grid scan of radius `ceil(f80/256)` (so `f80=0` ⇒ radius 0!). A pool
victim is hit iff: `id24 != fire.id24`, `flags&8` (hittable), `f28 & (1<<ch)`,
`filter_admits(f66,f67,…)`, AABB overlap (extents **summed** per axis — a
zero-extent attacker only hits a victim whose own box contains the fire's
center). Trees (class2/model0) take `amt/10`. The player carpet (class3/model0,
`f28` bit0 set, outside the pool) is reached by a separate probe gated only by
`id != PLAYER_TARGET` + filter + overlap.

## Our divergences (both fixed 2026-07-29)

1. **`spawn_effect(6)` never called `extents()`** → the standing fire had
   `f80=f82=f84=0` → overlapped nobody and its scan radius collapsed → burning
   trees and volcano lava dealt no damage (while the fireball fire, which set
   `extents(128,128)`, always worked). Fix: `self.extents(s, 272, 1536)`.
2. **Lava fire damage was `3 × bomb.f44 = 600`** (4× too high). Retail is
   `3 × the deposited fire's own f44 = 150`. Fix: `self.ent[f].f44 = 3 *
   self.ent[f].f44` after `spawn_effect`.

Regression test `a_standing_fire_burns_a_nearby_carpet` (engine/world.rs):
a carpet 250u from a standing fire (inside the 272+119 reach the extents grant,
outside the 0+119 a zero-extent fire would have) burns. ⚠ Set `player.grace = 0`
first — spawn grace (100 ticks) wipes the player mailbox and masks the damage.
