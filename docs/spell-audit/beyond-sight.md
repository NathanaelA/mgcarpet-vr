# Spell audit — Beyond Sight (index 12 / 0xC)

**Method:** recorded gameplay is senior; the vendored remc2 decompile is the reference of
record here (no recording was available for this pass). Cites are `EF:` = `EventsFunctions.cpp`,
`GameUI:` = `GameUI.cpp`, `L:` = `Level.cpp` line numbers in `reference/remc2/remc2/engine/`.
Port cites are `crates/…:line`.

## TL;DR

- **Beyond Sight is a pure map-reveal spell, armed-window only, and its three tiers reveal
  PROGRESSIVELY MORE — the player's memory is essentially correct.** The reveal *depth* is
  driven by the manifestation's **tier byte `byte_0x46_70`**, not by any per-tier SPELLS.DAT
  data field.
- **Tier 0 (L1):** while armed, reveal enemy WIZARDS on the minimap (their balloon blip +
  their name), *except* wizards who are Invisible or Metamorphed.
- **Tier 1 (L2):** additionally see through **Invisible** (reveal cloaked wizards too).
- **Tier 2 (L3):** additionally see through **Metamorph** (reveal all wizards) **AND reveal
  enemy ground CREATURES/monsters on the map** (the `sub_3A8B0` class-5 unit tick clears its
  own map-hidden bit only when the local player holds Beyond Sight at tier ≥ 2, `EF:29857`).
- **Duration scales by tier too:** the armed window is `word_0x18` = **151 / 261 / 361** ticks
  for tiers 0/1/2 (SPELLS.DAT row 12). The port already reproduces this (via `f28`).
- The effect state handler `sub_6B310` (`EF:57132`) sets **no player flag at all** — it only
  awards XP + drains mana while armed. All reveal logic lives on the READER side (the minimap
  draw and the creature tick), which look up the local player's Beyond-Sight manifestation and
  read its armed timer + tier directly.
- **GAP:** the port collapses everything to a single `player.beyond_sight: bool` with **no
  tier**. The app reveals enemy balloons and rival markers on any cast but **always filters out
  Invisible rivals** (= retail tier 0 only) and **never reveals monsters** (retail tier 2). So
  all three levels look identical, exactly as reported.
- **No cast sound** (silent) — both retail (`sub_6B310` has no `PrepareEventSound`) and the
  port (cast.rs 0xC arm) agree; nothing to change there.

## 1. Identification

- **Spell index:** 12 (0xC), name "Beyond Sight" — port table `crates/mgc-app/src/ui.rs:1412`
  (`MC2_SPELL_NAMES`). (The `Spells.h:35` "05 : Beyond Sight" comment is a stale/guessed
  *subtype* list, NOT the `spell_t` index — ignore it; every live code path keys on `[12]`.)
- **Class-15 model:** the learned spell is a class-15 manifestation whose `model_0x40_64 = 12`;
  the wizard's `str_611.SpellsEnabled[12]` holds its pool slot (read at `GameUI:1085`, `:1748`,
  `:2228`, `EF:29856`).
- **Effect-state handler:** `sub_6B310` at **`EF:57132`** (dispatched via `Events.cpp` by the
  state address `0x24c310`; `strF0[3·model]`). It is the tick that runs while the cast window is
  armed.
- **SPELLS.DAT row 12** (from `baked/assets/mc2-cave/spells.bin`; `byte_0` = 3 tiers,
  `isEnabled_1` = 12):

  | tier | manaCost_6 | maxManaLimit_A | xpos1_E (unlock XP) | word_0x18 (armed ticks) | life_0x1A |
  |------|-----------|----------------|---------------------|-------------------------|-----------|
  | 0    | 10000     | 0              | 0                   | 151                     | 0         |
  | 1    | 20000     | 20000          | 360                 | 261                     | 0         |
  | 2    | 30000     | 40000          | 1080                | 361                     | 0         |

  `subSpellIndex_2 = 0` and `life_0x1A = 0` for every tier — Beyond Sight spawns **no
  projectile and carries no charge byte**. The only tier-varying data is cost, unlock threshold,
  and **duration** (`word_0x18`). The *reveal depth* is NOT a data field — it is the raw tier
  number `byte_0x46_70` read by the map/creature code.

## 2. Retail behaviour per tier

### 2a. The effect state — `sub_6B310` (`EF:57132`)

```
if (word_0x2E_46 > 0) {                         // armed
    owner = Entities[parentId_0x28_40];
    if (owner valid) {
        if (word_0x2E_46 == word_0x30_48)       // first tick
            sub_6D8B0(owner, 0xC, 1);           // award 1 XP to spell 0xC
        if (sub_68D50(this, owner)) sub_68DE0(this, owner);  // afford → commit mana
        else word_0x2E_46 = 1;                  // can't afford → collapse to 1 tick
    }
    if (--word_0x2E_46 == 0) sub_6D880(this);   // expiry → apply pending tier
}
if (word_0x36_54) word_0x36_54--;               // cooldown tick
```

Note what is **absent**: no `struct_byte_0xc` bit is set, no `byte_0x1BF`, no player field.
Contrast the Invisible handler `sub_6B1C0` (`EF:57068`) directly above it, which DOES set
`byte_0x1BF_447 = tier.life_0x1A` and the `0x20` invisibility bit. Beyond Sight is entirely
passive on the caster side; the reveal is done by whoever draws the map.

### 2b. The map reader (enemy WIZARD reveal) — `GameUI.cpp`

Both minimap variants capture the armed timer of the local player's Beyond-Sight manifestation:

- `GameUI:1085-87` (`sub_63670_draw_minimap_a`): `v16 = SpellEnabled[12]; if (v16) v83 =
  Entities[v16]->word_0x2E_46;` → **`v83` = armed?**
- `GameUI:1748-50` (`sub_63C90_draw_minimap_b`): same into `v93`.

Then, **gated by `if (v83)` / `if (v93)` (i.e. only while armed):**

1. **Enemy balloons** (`GameUI:1196-1209`, class-1 / model-3): drawn `if (jy->id == self->id ||
   v83)` — an enemy carpet/balloon blip (sprite `66+color`, symbol 26) appears only if it is
   yours OR Beyond Sight is armed.
2. **Enemy wizard positions + NAMES** (`GameUI:1492-1529`, and `:2143-…` for minimap B): loop
   over all other players; for each alive one draw its blip + `DrawText(WizardName…)` **iff
   `sub_63570(self, other)` returns 1**.

### 2c. The tier gate — `sub_63570` (`GameUI:2219-2252`)

```
v2x = viewer.SpellEnabled[12]                 // beyond-sight manifestation
v4x = target.SpellEnabled[4]                  // target's METAMORPH manifestation (index 4)
v5  = (v4x valid && v4x->word_0x2E_46) ? 1:0  // target Metamorph armed?
if (v2x valid) {
    v6 = v2x->byte_0x46_70;                    // BEYOND-SIGHT TIER
    if (v6 < 1) {                              // tier 0
        if (target->byte_0x1BF_447) return 0;  //   Invisible target → hidden
    } else if (v6 > 1) {                       // tier 2
        return 1;                              //   see EVERYTHING
    }
    // tier 1, or tier 0 that passed the invis check:
    if (v5) return 0;                          // Metamorphed target → hidden
}
return 1;                                       // visible
```

`byte_0x1BF_447` is the **Invisible** strength byte (set by `sub_6B1C0`, `EF:57089`; cleared at
`:57109`). `SpellEnabled[4]` is **Metamorph** (`MC2_SPELL_NAMES[4]`). So per tier, for wizards:

| tier | reveals enemy wizard… | through Invisible? | through Metamorph? |
|------|-----------------------|--------------------|--------------------|
| 0 (L1) | yes                 | no                 | no                 |
| 1 (L2) | yes                 | **yes**            | no                 |
| 2 (L3) | yes                 | yes                | **yes**            |

### 2d. The creature/monster reveal (tier ≥ 2 ONLY) — `sub_3A8B0` (`EF:29856-29861`)

Inside the ground-unit tick `sub_3A8B0` (dispatched from `Events.cpp:2783`; the class-5 mobile
creature/settler state), each tick an enemy (non-owned) unit re-computes its map-visibility bit:

```
else if (!(a1x->byte_0x3E_62 & 7)) {
    v9x = Entities[localPlayer.SpellsEnabled[12]];          // local player's beyond-sight
    if (v9x valid && v9x->word_0x2E_46 && v9x->byte_0x46_70 >= 2)
        a1x->struct_byte_0xc_12_15.byte[0] &= 0xFE;         // clear hidden bit → REVEALED
    else
        a1x->struct_byte_0xc_12_15.byte[0] |= 1;            // set hidden bit → HIDDEN
}
```

`byte[0] & 1` is the **map-hidden flag**: the minimap draw skips any entity with it set
(`GameUI:1220`, `:1398`, `:1861`, `:2037`; it also drives a class-15 render tint at
`GameRenderOriginal.cpp:2294`). This is the ONLY site in the engine that keys on `SpellEnabled[12]
&& tier ≥ 2` — i.e. **L3 is the only tier that reveals enemy monsters/units**, matching the
player's "L3 also monsters."

**Duration / armed window:** `word_0x30_48` (= `word_0x18` = 151/261/361 by tier) is loaded as
the initial cast-timer at arm; the reveal is live only while `word_0x2E_46 > 0`. Higher tiers
stay revealed **longer** as well as **deeper**. Mana is drained per tick over the window
(cost / `word_0x18`). **No sound** is played on cast.

## 3. Current port

- **State:** one `player.beyond_sight: bool` (`crates/mgc-sim/src/mc1/world.rs:150`). Set true
  on cast (`crates/mgc-sim/src/mc2/cast.rs:784`, spell 0xC arm) + XP award; cleared at cast
  expiry (`cast.rs:652`). **No tier is stored anywhere.** The MC1 legacy channel mirror at
  `world.rs:3096` (`5 => beyond_sight = active`) is the same single bool.
- **Duration:** DOES scale by tier — `mc2_set_spell` loads `e.f28 = word_0x18` (151/261/361) and
  `mc2_cast_gate` arms `f26 = f28`, expiring via `mc2_cast_tick`/`mc2_cast_expire`. Only the
  *reveal depth* is flat.
- **App consumption** (`crates/mgc-app/src/main.rs:1206`, `:1218`):
  - `entities::map_stamps_from_poses(…, w.beyond_sight(), …)` → balloons (class 3 / model 3)
    stamped `if p.team == Some(0) || beyond_sight` (`entities.rs:610`). Correct shape for the
    wizard-balloon reveal, but ungated by tier.
  - `entities::rival_markers(&w.rival_views(), w.beyond_sight())` (`entities.rs:644-661`) →
    a 2×2 team-colour dot per rival, but **`.filter(|r| r.alive && !r.invisible)`**: it
    *unconditionally hides Invisible rivals*. That is retail **tier 0** behaviour applied to
    every tier.
- **Monsters:** the port has **no** creature map-reveal path at all — nothing consumes Beyond
  Sight to un-hide enemy class-5 units.
- `w.beyond_sight()` returns the bare bool (`world.rs:3437`).

## 4. Gap

| aspect | retail | port | verdict |
|--------|--------|------|---------|
| reveal enemy wizard on map | all tiers | any cast | OK (untiered but present) |
| reveal Invisible wizard | tier ≥ 1 | **never** (`!r.invisible` filter) | **WRONG** — L2/L3 don't work |
| reveal Metamorphed wizard | tier 2 | n/a (Metamorph unported) | latent — no cloak to pierce yet |
| reveal enemy monsters/units | tier 2 | **never** | **MISSING** — L3's headline effect |
| armed-window duration by tier | 151/261/361 | 151/261/361 | OK |
| cast sound | none | none | OK |

Root cause: a single boolean discards the tier. All three levels are indistinguishable on the
map exactly as the player reports; the only tier-dependent behaviour that survives is the
(barely visible) duration.

## 5. Fix data

**State (sim):** replace/augment the bool with the tier. Two viable shapes:
- add `player.beyond_sight_tier: u8` (0/1/2) alongside a live bool, OR
- expose the manifestation's live tier directly: `beyond_sight()` returns `Option<u8>` = the
  tier of `mc2_book.ent[12]` while `f26 > 0`, else `None`.

Set it at the cast-fire site (`cast.rs:783-786`) from `self.g.ent[m].f71` (the live tier), and
clear at `mc2_cast_expire` (`cast.rs:651-652`). Keep the MC1 `apply_effect` channel
(`world.rs:3096`) as tier-0/off for MC1 (MC1 has its own Beyond-Sight semantics; do not disturb
the MC1 goldens — gate the tier field additions behind the same hash-transparency pattern used
for the spellbook).

**Reveal law (app), by tier `t` = live Beyond-Sight tier (None = not armed):**
- `None`: reveal only own team (unchanged base map).
- `t ≥ 0` (armed, any tier): reveal every alive enemy **wizard** balloon + marker/name.
  - `t == 0`: exclude rivals that are Invisible OR (once ported) Metamorphed.
  - `t == 1`: **include Invisible rivals**; still exclude Metamorphed.
  - `t == 2`: include all rivals unconditionally.
- `t == 2` **only**: additionally reveal enemy **creatures/monsters** — i.e. the class-5 ground
  units that are otherwise culled from the map. In the port this means: when building the map
  pose/dot list, stop hiding enemy mobs iff `beyond_sight_tier == 2`.

Concretely:
- `entities::rival_markers` — change the filter from unconditional `!r.invisible` to
  `tier >= 1 || !r.invisible` (and drop the metamorph exclusion until Metamorph lands).
- `entities::map_stamps_from_poses` balloon branch — fine as-is for wizards; no tier change
  needed for L1 wizard reveal, but the invisible-rival exclusion must live in the marker path
  (above), keyed on tier.
- Add a tier-2 branch wherever enemy creatures are excluded from the minimap pose set, un-hiding
  them (mirror of `EF:29857` clearing `byte[0] & 1`). This is the biggest new wiring.

**State fields referenced (retail):** manifestation `word_0x2E_46` (armed timer),
`word_0x30_48`/`word_0x18` (duration 151/261/361), `byte_0x46_70` (tier — THE reveal knob);
target `byte_0x1BF_447` (Invisible) and `SpellEnabled[4]`/`word_0x2E_46` (Metamorph); entity
`struct_byte_0xc.byte[0] & 1` (map-hidden bit).

**Sounds:** none — Beyond Sight casts silently in retail and in the port; do not add one.

## 6. Confidence, open questions, test

**Confidence:** HIGH on the mechanism. `EF:57132` (effect state), `GameUI:2219-2252` +
`:1085/1198/1492/1748/2143` (wizard reveal + tier gate), and `EF:29856-29861` (tier-2 monster
reveal) are unambiguous and mutually consistent; SPELLS.DAT row 12 values are read straight from
the baked bundle. The player's fuzzy memory ("L1 extended vision, L2 players, L3 monsters") lines
up with retail (L1 = base wizard reveal, L2 = see through Invisible, L3 = see through Metamorph +
reveal monsters) once "extended vision" is read as "the map now shows enemy positions."

**Open questions:**
1. **Exactly which entity models route through `sub_3A8B0`** (i.e. which "monsters/units" the L3
   reveal covers). It is the class-5 ground-unit tick, but the full model set behind state
   address `0x21b8b0` was not enumerated in this pass — cross-check against the MC2 class-5
   roster docs before wiring the app's creature-reveal so the un-hide set matches retail.
2. **"Extended vision" (render draw-distance):** no 3D-render path keys on `SpellEnabled[12]`
   (grepped GameRenderOriginal/NG/HD/GL + ViewPort — zero hits). So retail Beyond Sight does
   **not** extend the 3D view distance; it is purely a minimap reveal. If a recording shows a
   view-distance change, that would contradict the decompile and should be recorded.
3. **Metamorph interaction** is currently untestable in the port (Metamorph effect unported);
   the tier-2 "see through Metamorph" rung can only be verified once spell 4 lands.

**Suggested test:** on an MC2 level with a rival wizard, cast Beyond Sight at each tier (use the
dev-spells instrument to select tiers) while the rival is (a) plain, (b) Invisible. Assert on the
minimap: tier 0 shows the plain rival but NOT the invisible one; tier 1 shows both; tier 2 shows
both plus any nearby enemy ground creatures that are absent at tiers 0/1. A sim-level golden can
pin `beyond_sight_tier` transitions and the resulting revealed-entity count per tier without the
renderer.
