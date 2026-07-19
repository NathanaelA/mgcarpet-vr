# MC2 spell audit — Gravity Well (20) & Cave-In (25)

Fidelity verification. Recorded gameplay = senior; vendored decompile
(`reference/remc2/remc2/engine/*`) = reference. `EF:` =
EventsFunctions.cpp. Port = `crates/mgc-sim/src/mc2/*`.

## TL;DR

- **Gravity Well (spell 20) — REAL BUG, one-line fix.** The spell
  launches its projectile fine (subtype 22 → impact class/model
  `(10,67)`), but the port's impact dispatcher **has no `(10,67)`
  arm** — it falls through to the misfit branch (`proj.rs:406`) and
  degrades to a bare, invisible area-damage ping. The `(10,67)`
  flood/quake ground-collapse effect **is fully ported**
  (`flood.rs`); the impact is simply never routed to it. → "no
  effect." **Fix: add `(10, 67) => self.mc2_spawn_flood(x, y, z),`
  to the `match (fc, fm)` in `mc2_proj_impact`.** Per-tier damage
  then scales for free via the existing generic post-spawn block.

- **Cave-In (spell 25) — NO constant bug found; the port already
  scales per tier.** Contrary to the working hypothesis, ring count
  IS tiered: `mc2_cave_in_tick` returns rings `{0→3, 1→5, 2→7}` keyed
  on `maxLife` (`cave.rs:523-527`), which is set from the projectile's
  tier `life` at impact (`proj.rs:397-404`) — a **verbatim** match to
  retail `sub_311E0` (EF:22910-22922) and `sub_67910` (EF:59218-30).
  Row-25 tier lives are `0/1/2`, so tier 0/1/2 → 3/5/7 rings, box
  15/17/19 tiles. The likely reason a player sees "the same every
  cast" is the **XP tier-unlock gate** (tiers 1/2 need 400/1600
  spell-XP; without earning them, or without the dev/all-spells
  toggle, only tier 0 = 3 rings is castable). Needs a playtest at
  distinct tiers to confirm, not a code change.

---

## (A) Gravity Well — spell 20

### 1. Identity
- **Spell index** 20; SPELLS.DAT **row 20**.
- **Effect state** `sub_6C3E0` (EF:57717), which calls `sub_6DCA0`
  with **a3 = 0x14** (EF:57736).
- **Projectile** class-9 **subtype 22 / 0x16**, creator `sub_4DEA0`
  (EF:~35160: `actionIndex=23`, `model=22`, actSpeed 384, sprite
  211). Mirrored in the port's `CREATORS` table
  (`cast.rs:158`: `(22, 23, 384, 21, 60, 211)`).
- **Impact model `(10,67)`** — the flood/quake ground-collapse.
  Confirmed both ways: `sub_6C3E0` arms it, and the inverse
  `GetSpellIndex_6E020` maps impact model 67 → spell 20 (EF:44240;
  port `spells.rs:126`).
- **Row 20 tier fields** (baked `spells.bin`, verified):

  | tier | subSpell | mana | maxManaLimit | xpos1 | life |
  |------|----------|------|--------------|-------|------|
  | 0 | 750  | 15000 | 300000 | 0    | 16 |
  | 1 | 1500 | 20000 | 350000 | 1500 | 26 |
  | 2 | 3000 | 30000 | 400000 | 3000 | 40 |

### 2. Retail effect + per-tier law
The projectile flies (auto-aim homing — model 22 is in the homing
set, class9 trace §auto-aim), then detonates on terrain/victim
contact. The generic flyer detonation (`sub_65820`, EF:62979-94)
spawns the impact effect via `_4A190(pos, byte_0x43_67,
byte_0x44_68)` = **class 10, model 67**, and copies onto it:
`id`, `yaw`, `pitch`, target, **`subSpellIndex` (= the tier's
subSpell 750/1500/3000)** (EF:62993), and **`byte_0x46_70`
(= the tier's life 16/26/40)** (EF:62994).

The `(10,67)` flood/quake (ctor `sub_51730`, EF:37421; action-0x48
phase machine, EF:28509+) is a **fixed-size** ground collapse:
life 120, AABB ±17 tiles, and per-tick over a 30×30 box it —
- **grabs** class-3 model-2 **castles** (flags a quake-grab, mails
  `subSpellIndex` as damage — EF/port `flood.rs:395-400`),
- **destroys** class-10 model-45 **buildings** (`act_life = -1`),
- **burns** burnable/lava-edge terrain to lava.

**Per-tier scaling: DAMAGE ONLY.** The ctor geometry (radius,
duration) is constant across tiers; only the mailed `subSpellIndex`
(750 → 1500 → 3000) scales. There is no per-tier radius law.

**RETAIL QUIRK / OPEN.** EF:62994 stamps the flood's phase field
`byte_0x46_70` with the projectile's value (tier life 16/26/40). The
flood's phase switch is guarded `if (v4 <= 3u)` (EF:28527), so a
phase of 16/26/40 **skips the entire machine** — the flood would sit
inert until its 120 life expires. Either (a) retail's player-cast
gravity-well flood is a partial no-op, or (b) the action-23 wrapper
resets `byte_0x46_70` to 0 before/at spawn (analogous to Cave-In's
`sub_67910`). I could not locate the action-23 dispatch to confirm.
Because recorded gameplay is senior and the effect is clearly meant
to fire, the correct port behavior is a **working** flood (phase 0).

### 3. Current port
Launch is correct: `mc2_dispatch_arm(20, ..)` → `arm(22, (10,67),
charge=true)` (`cast.rs:686`); `mc2_launch` sets `f44 = tier
subSpell`, `f68/f69 = (10,67)`, `f71 = tier life`. The projectile
is even water-exempt (`proj.rs:606`, model 22 in `{4,22,24,26}`) so
it detonates on terrain like retail.

**The break is at impact.** `mc2_proj_impact` (`proj.rs:339-412`) has
arms for `(10,0/1/11/12/17/22/23/65/66/76/89)` but **not `(10,67)`**,
so it hits `_ =>` (`proj.rs:406-410`): `note_misfit(10,67)` + a bare
`area_write` of the carried damage on channel 0. No flood/quake
entity is ever created → the player sees nothing (only an invisible
damage ping if something is right at the impact point). The flood
effect itself is fully ported and reachable
(`flood.rs:94 mc2_spawn_flood`, `flood.rs:559 mc2_flood_tick`).

### 4. Gap
A single missing dispatch arm. The impact is routed to the misfit
fallback instead of the ported `mc2_spawn_flood`.

### 5. Fix data
In `crates/mgc-sim/src/mc2/proj.rs`, `mc2_proj_impact`'s
`match (fc, fm)`, add:

```rust
(10, 67) => self.mc2_spawn_flood(x, y, z),
```

Rationale / expected wiring:
- The existing generic post-spawn block (`proj.rs:413-419`) then sets
  `id/yaw/victim` and **`f140 = dmg`** where `dmg = projectile.f44 =
  tier subSpell** → per-tier damage (750/1500/3000) reaches
  `flood_damage_pass` (`flood.rs:373` reads `f140`). No extra work.
- **Do NOT** copy the projectile's `f71` onto the flood. The flood
  ctor leaves `f71 = 0` and the generic block does not touch it, so
  the phase machine runs. (Copying `f71` would reproduce the
  inert-retail quirk above.)
- No per-tier radius/duration change is needed (retail ctor is fixed:
  life 120, ±17-tile AABB).
- Per-tier params (row 20): tier0 dmg 750, tier1 1500, tier2 3000;
  geometry constant.

### 6. Confidence / open / test
- **Confidence: HIGH** on the gap and the fix (impact simply isn't
  routed to the ported effect).
- **Open (MEDIUM):** the `byte_0x46_70`→phase copy quirk (EF:62994 vs
  the `<=3` guard EF:28527) — whether retail's cast flood runs at all;
  the action-23 wrapper is unlocated. The recommended port fix
  side-steps it by keeping phase 0.
- **Open (naming):** "Gravity Well" ↔ flood/quake ground-collapse.
  The model↔spell mapping (67↔20) is verbatim retail, so the identity
  is firm; the label just describes a terrain-collapse, not a literal
  pull.
- **Suggested test:** cast Gravity Well at a rival castle + a cluster
  of buildings; expect the ground-collapse (buildings erased, castle
  grab-shake, terrain → lava in a ~30-tile box). Compare tier 0 vs
  tier 2 castle damage (750 vs 3000). Confirm the flood entity
  actually spawns (misfit ledger should no longer log `(10,67)`).

---

## (B) Cave-In — spell 25

### 1. Identity
- **Spell index** 25; SPELLS.DAT **row 25**. **CAVE-ONLY** (gate
  `cast.rs:523`).
- **Effect state** `sub_6CFA0` (EF:58123) → `sub_6DCA0` with
  **a3 = 0x19** (EF:58144).
- **Projectile** class-9 **subtype 30 / 0x1E**, action **31 =
  `sub_67910`**, sprite 211 (port `CREATORS` `cast.rs:165`:
  `(30, 31, 384, 21, 60, 211)`).
- **Impact model `(10,89)`** — the cave-in collapse (ctor
  `sub_50A20` EF:37037; tick `sub_311E0` EF:22860). Never authored;
  only the spell spawns it.
- **Row 25 tier fields** (baked `spells.bin`, verified):

  | tier | subSpell | mana | maxManaLimit | xpos1 | life |
  |------|----------|------|--------------|-------|------|
  | 0 | 100  | 11000 | 100000 | 0    | 0 |
  | 1 | 480  | 13000 | 150000 | 400  | 1 |
  | 2 | 1200 | 26000 | 250000 | 1600 | 2 |

### 2. Retail per-tier scale law
`sub_311E0` (EF:22910-22922), verbatim:
```
maxLife < 1  → rings = 3
maxLife == 1 → rings = 5
maxLife == 2 → rings = 7        (else → 3)
box_r = rings + 12
```
Then 6 concentric rings from `ring_r = rings`, +2 tiles each, carve
floor-up/ceiling-down on a sine profile; debris burst radius =
`(rings<<8) - 768` clamped `[256, 0x2000]`. **`maxLife` is written by
the action-31 wrapper `sub_67910`** (EF:59218-30): `spawned.maxLife =
projectile.byte_0x46_70` (= the tier's `life`), and the spawned
entity's own `byte_0x46_70` (its phase) is reset to 0.
**No direct HP** — the terrain crush is the weapon (`subSpell`/damage
is unused by cave-in).

So the per-tier law is: **tier 0 → 3 rings / box 15**, **tier 1 → 5 /
box 17**, **tier 2 → 7 / box 19**, with proportionally larger debris.

### 3. Current port — already tiered
- `mc2_cave_in_tick` (`cave.rs:522-527`): `rings = match max_life {
  1 => 5, 2 => 7, _ => 3 }`, `box_r = rings + 12`. **Exact match** to
  retail.
- Impact arm (`proj.rs:397-404`): `(10,89) =>` spawn cave-in, then
  `max_life = charge` where `charge = projectile.f71`, and `f71 = 0`
  — **verbatim `sub_67910`**.
- Launch (`cast.rs:687` + `mc2_launch`): `arm(30,(10,89),charge=true)`
  → projectile `f71 = sub.life` = the selected tier's life (0/1/2).
  (The `sub_spell /= life` division at `cast.rs:702` touches only the
  damage payload, not `life`, so the ring key is intact.)

Trace end-to-end: select tier T → manifestation `f71 = T` → fire
reads tier T → projectile `f71 = life(T)` (0/1/2) → impact
`max_life = 0/1/2` → 3/5/7 rings. **The port does NOT use a
constant.**

### 4. Gap
No constant-scale bug in current code — ring count, box size and
debris radius all scale per tier and match retail. The plausible
source of a "same every level" perception:
- **XP tier-unlock gate.** Without the dev/all-spells toggle, the
  selectable tier is capped at the XP-earned level
  (`mc2_select_spell`, `cast.rs:426-435`). Row-25 `xpos1` = 0 / 400 /
  1600, so a fresh player can only cast **tier 0 = 3 rings** until
  they bank 400+ spell-XP. Every early cast is identical by design.
- This was worsened by the (now-fixed, 2026-07-13) "can't select
  higher spell levels" regression, where the sim silently cast tier 0
  while the selector showed tier N.

### 5. Fix data
**No handler change required** — the scaling law is correct and
data-faithful. Action items are verification, not code:
- Confirm the player is actually reaching tiers 1/2 (earn 400/1600
  spell-XP, or enable the all-spells/dev toggle which lifts the
  selection cap, `cast.rs:426`).
- If a genuine constant is still observed **at distinct selected
  tiers**, the fault would be upstream `f71` delivery — instrument
  the `(9,30)` projectile's `f71` at the impact tick and the spawned
  `(10,89)` `max_life`. Current static trace shows both correct.

### 6. Confidence / open / test
- **Confidence: HIGH** that the port scales rings 3/5/7 per tier and
  matches retail `sub_311E0`/`sub_67910` exactly.
- **Open:** the player's phrase "different between levels" — if
  "levels" = spell tiers, the port already does this (gated by XP);
  if "levels" = game maps, note that cave-in is never authored, so
  scale is purely a function of the cast tier.
- **Suggested test:** in a cave with the all-spells toggle ON, cast
  Cave-In at tier 0 then tier 2 over flat ground. Expect a visibly
  larger collapse footprint (box 15 → 19 tiles) and a wider debris
  ring at tier 2. If identical, capture the projectile `f71` and the
  `(10,89)` `max_life` at impact.
