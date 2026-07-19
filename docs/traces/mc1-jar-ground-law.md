# MC1/HW spell-jar ground law — remc1 trace + port divergence (2026-07-15)

Investigating player-reported regression **PR-1**: spell jars don't
track the ground. They neither fall nor ride terrain changes — ground
destroyed under a jar leaves it hovering; on **HW level 00 a jar
spawned BURIED below the surface**. Expected: a jar rests at its
tile's ground altitude, re-settles on terrain change, clamps to ground
at spawn.

All `sub_main.cpp:LINE` citations are `reference/remc1` unless noted.
Read-only investigation; jar entity model per `docs/traces/mc1-blue-jars.md`.

---

## 1. Retail law — how a jar gets its z, and what happens after

### 1a. Spawn z = ground height at tile center — CONFIRMED

THING post-init `sub_37560_37920` (:43988) computes the spawn position
for every placed entity, jars included:

```c
v5[0] = (type1090->data_4 << 8) + 128;   // tile-center x  (:44003)
v5[1] = (type1090->data_6 << 8) + 128;   // tile-center y  (:44004)
v5[2] = sub_11F50((axis_3d*)v5);         // z = GROUND HEIGHT  (:44005)
v2 = sub_373F0_377B0((axis_3d*)v5, class, model);   // create at v5
```

`sub_11F50` (:17229) → `sub_724C0_729D0(x,y)` (:17231) is the bilinear
terrain-height sampler (port `Gen::ground_z`, features.rs:1062). The
jar is created AT ground height, **no half-height offset**. The class-12
arm at :44043-54 then only tweaks state/blue-flag (blue-jars trace §Q1).

The jar ctor `sub_3BF70` (:47981) links the entity through
`sub_41CF0_42030` (:52468): that routine copies the caller's position
verbatim (`a1->...72 = *a2`, :52480) and threads the entity into the
per-tile intrusive list `mapEntityIndex_10C1E0_10C1D0[tile]` — it does
**no** ground clamp of its own. So spawn z is exactly whatever
`sub_11F50` returned. **CONFIRMED: retail spawn-z == port spawn-z math.**

### 1b. Runtime: the jar tick NEVER updates z — CONFIRMED

Class dispatch row `//c` (index 12) of `dword_96902` (:5041) = state
table `str_2563D8` (:4957). The three placed-jar substates dispatch to
pure pickup polls:

- `sub_56250` → `sub_55DB0_562E0` (:64904) → `sub_55A40_55F70`
  (:64729) — the pickup/own conversion.
- `sub_56260` → `sub_55D30_56260` (:64875) — pickup variant.

Neither touches z. (The spell-specific `sub_56090_565C0` (:65029) /
`sub_56510_56A40` (:65203) entries in the same table are the OWNED-
manifestation firing states — `+48` burst counter, projectile spawn —
not placed jars.) The per-entity update loop (:52226-52354) only
buckets entities by class and calls `data6`; there is **no generic
per-tick ground/gravity pass** applied to jars. Gravity/band-settle
(`sub_42000_42340`, :52576) is velocity-driven and only run by moving
entities (rivals, projectiles, effects) — jars never call it.

**CONFIRMED: a placed jar's z is STATIC after spawn in retail.**

### 1c. Terrain-change handler ignores jars — CONFIRMED

`sub_40E20_41160(tile, exclude_id)` (:51729) walks the per-tile entity
list on a changed tile and, per class:

- class **2** (scenery): `sub_41E80_421C0` → sets `byte[1]|=4` (mark
  for re-eval/destroy).
- class **5** (creatures, except models 16/6/8): `life = -1` (kill).
- **everything else (incl. class 12 jars): no-op** (:51760 default).

So even the terrain-change notification path does **not** re-settle
jars. There is no z re-settle on terrain change anywhere in the traced
class-12 code.

### 1d. Render draws the billboard at the STORED z — CONFIRMED

The terrain-billboard path attaches an entity to its tile
(`haveBillboard_36 = mapEntityIndex[tile]`, :33991) and draws it with
`DrawSprite_2FC50_2FC90` (:37833). The sprite's world z is the
**entity's own stored z**, not the terrain vertex:

```c
int32 tempZ = -str_29795[result].var_u32_29867_72.z - posZ;   // :37854
```

So a jar whose stored z drifts from the ground *would* visibly hover
in retail too — the render offers no ground clamp to mask it.

### 1e. Net retail law (decompile) vs the player observation

Decompile: **spawn-at-ground, static-z thereafter, drawn at stored z.**
There is **no** decompile-confirmed per-tick or terrain-triggered jar
re-settle. Retail avoids visible hover only because terrain is fully
shaped *before* dis-0 THINGs spawn (load-time feature fixpoint) and
placed jars sit on terrain that rarely changes under them in MC1.

The player's "retail re-settles jars when terrain changes" claim is
**NOT reproduced in remc1's class-12 code** (marked INFERRED / OPEN).
The truncated decompile is a known limitation (recorded gameplay
outranks it), but no mechanism was found — flag for a targeted retail
re-check of the *runtime* re-settle specifically. What the decompile
DOES nail is the spawn-at-ground invariant, which is the concrete
HW-level-00 burial symptom.

---

## 2. Port divergence

The port already ground-clamps at BOTH spawn sites:

- THING spawn `spawn_from_thing` (mc1/world.rs:4751):
  `let z = self.g.ground_z(x, y) as i16;` → `spawn_inert` (…:5558
  `link(s, x, y, z)`). Matches retail :44005. **No divergence at spawn.**
- Death-scatter `player_land` (…:2179/2185): jar re-linked at
  `ground_z(x,y)`. Matches retail.

The divergence is that **nothing re-samples ground after spawn** and
the render (mgc-app) draws the jar at its stale stored z:

- `class12_tick` (mc1/world.rs:3399-3440) does prune / decay / pickup
  only — **no z update** for the placed (`tick70 < MANIFEST_BASE`)
  states. This is faithful to §1b in isolation.

So the port faithfully reproduces the decompile's static-z law. The
HW-level-00 burial is therefore a **terrain-under-jar height change
that the static-z law can't follow**: on HW level 00 the ground at the
jar's tile ends up higher than the jar's spawn-time sample (aggressive
load/early-runtime terrain shaping), and — exactly like retail's
untraced-but-observed behavior — the jar needs to re-settle. HW makes
this constant; MC1 masked it.

**Port divergence point:** `crates/mgc-sim/src/mc1/world.rs:3399`
(`class12_tick`) — the placed/dropped-jar arm never re-samples
`ground_z`. (Not a refactor regression: `git log -S class12_tick`
shows it has had this shape since the 745eef4 polymorphism refactor,
and the pre-refactor `world.rs` was identical — the ground-follow was
never implemented.)

---

## 3. Proposed minimal fix

Add an **idempotent per-tick ground clamp** for placed/dropped jars,
inside `class12_tick`, before the pickup poll, guarded to the placed/
dropped states (`tick70 < MANIFEST_BASE`; leave owned manifestations
alone):

```rust
// PR-1: a placed jar rests at its tile's ground altitude and
// re-settles when terrain changes under it (HW load/runtime terrain
// shaping buries the spawn-time sample). Idempotent: no-op wherever
// the jar already sits on current ground.
let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
let gz = self.g.ground_z(x, y) as i16;
if self.g.ent[i].z != gz {
    self.g.ent[i].z = gz;
    self.entities_dirty = true;
}
```

Mechanism = per-tick ground snap (NOT gravity/fall, NOT a terrain-write
notification — those don't exist for jars in retail). This is an
**INFERRED faithful reconstruction** of the observed ground-resting
behavior, extending the CONFIRMED spawn-at-ground law across the
jar's life. It fixes BOTH symptoms: burial self-corrects on the next
tick, and destroyed/raised terrain is followed.

Alternative (decompile-purist, burial-only): guarantee the jar's
spawn-time `ground_z` sample sees final HW terrain (load-order), and
accept static-z at runtime as faithful. Rejected as the primary fix
because it does not address the runtime "hover" the player reports and
does not self-heal early-runtime terrain shaping.

---

## 4. Golden impact — jars ARE hashed

`World::state_hash` (…:2352) → `Gen::hash` (`#[derive(Hash)]`,
features.rs:529, hashes `ent: Vec<Ent>`) → `Ent` (`#[derive(…, Hash)]`,
features.rs:373) with `z: i16` at features.rs:462. **Every active
entity's z, jars included, is in the golden hash.** CONFIRMED.

The per-tick clamp is **hash-neutral wherever a jar already sits on its
tile's current ground** — the write is skipped (or writes the identical
value), so the entity hash is byte-identical. It moves the hash ONLY on
frames where a jar's stored z differs from its tile ground — i.e.,
exactly the buggy states this fix targets. On MC1 golden fixtures with
no terrain change under a placed/scattered jar, the goldens **hold**.

**Must run the MC1 state-hash suite (`crates/mgc-sim/tests/state_hash.rs`)
to confirm zero movement; expected zero. Any movement localizes to a
fixture that genuinely destroys/raises terrain under a jar — a
justified, correct hash change.** The clamp is intrinsically
golden-safe by construction (idempotent on already-correct z), which is
the standard "hash-transparent while unchanged" discipline used across
this codebase (Mc2Ord / death_owned_blue precedents).
