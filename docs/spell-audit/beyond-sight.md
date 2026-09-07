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
  enemy MAGIC MINES in the viewport** (`sub_3A8B0` at `EF:29749` is the class-10 model-78
  Magic Mine tick — the port's `mc2_mine_tick` — NOT a creature tick as the first pass of this
  audit claimed; its draw-bit block `EF:29848-58` clears the mine's hidden bit for the human's
  own mines, and for every other wizard's only while the local Beyond Sight is armed at tier
  ≥ 2. Bit 0 gates the billboard pass `& 0x21` (GameRenderOriginal.cpp:1936/3157) and the
  mouse pick (PlayerInput.cpp:1626); the mine's MAP dot is own-only regardless, GameUI:1320-27).
  Nothing in the engine reveals creatures for Beyond Sight.
- **Duration scales by tier too:** the armed window is `word_0x18` = **151 / 261 / 361** ticks
  for tiers 0/1/2 (SPELLS.DAT row 12). The port already reproduces this (via `f28`).
- The effect state handler `sub_6B310` (`EF:57132`) sets **no player flag at all** — it only
  awards XP + drains mana while armed. All reveal logic lives on the READER side (the minimap
  draw and the creature tick), which look up the local player's Beyond-Sight manifestation and
  read its armed timer + tier directly.
- **Port (2026-09-07):** `beyond_sight_tier()` exposes the live tier; `entities::rival_reveals`
  draws the retail NAME labels (font-glyph map stamps, per-game offset/colour) with MC2's
  position pixel, gated by `sub_63570`'s law (Invisible + the new `RivalView::metamorphed`);
  `live_poses_mc2` hides a rival's (10,78) mine from the viewport unless the tier is 2. See §3.
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

### 2d. The enemy-mine reveal (tier ≥ 2 ONLY) — `sub_3A8B0` (`EF:29848-58`)

`sub_3A8B0` (`EF:29749`, state address `0x21b8b0`) is the **Magic Mine** tick (class 10, model
78, action 0x55; the port's `mc2_mine_tick`), not a creature tick. Inside its countdown/float
block each mine re-computes its hidden bit:

```
if (LevelIndex == Entities[word_0x32_50]->playerColorIndex)   // the human's own mine
    byte[0] &= 0xFE;                                           //   always visible
else if (!(byte_0x3E_62 & 7)) {
    v9x = Entities[localPlayer.SpellsEnabled[12]];             // local Beyond Sight
    if (v9x valid && v9x->word_0x2E_46 && v9x->byte_0x46_70 >= 2)
        byte[0] &= 0xFE;                                       // tier 2 armed → REVEALED
    else
        byte[0] |= 1;                                          // HIDDEN
}
```

`byte[0] & 1` is the generic hidden bit: the billboard gather/draw skips `& 0x21`
(GameRenderOriginal.cpp:1936, :3157; NG/HD mirrors), the mouse pick skips it
(PlayerInput.cpp:1626), and the map's class-5 / class-12 / class-15 arms skip it (GameUI:1220,
:1398, :1861, :2037). The mine's own map arm (10, 0x4E) is own-only anyway (GameUI:1320-27),
so tier 2's visible effect is **a rival's mine appears in the 3-D view** (and becomes
targetable). It is the ONLY site in the engine that keys on `SpellEnabled[12] && tier ≥ 2`.

### 2e. MC1 — `sub_48710` (remc1 :57143-46, :57232-35, :57413-48)

One tier. `v59` = the spell-5 token's burst counter (`+48` of the entity at player data
`+686`). While it runs: (a) rival BALLOONS (3,3) stamp `[66 + colour]` (:57232-35; castles
stamp always); (b) for every other player whose wizard entity has `+12 >= 0`, `DrawText(name,
x + 2, y)` (hires `2x + 2, 2y`) in `byte_99B58[1 + 2·colour]` — NO position dot and NO
invisibility test (:57413-48). Nothing else reads `v59`. The names are `off_99B68[slot]` —
Zanzamar (the human's default), Vodor, Gryshnak, Mahmoud, Syed, Raschid, Alhabbal,
Scheherazade — copied at level init (:49158).

**Duration / armed window:** `word_0x30_48` (= `word_0x18` = 151/261/361 by tier) is loaded as
the initial cast-timer at arm; the reveal is live only while `word_0x2E_46 > 0`. Higher tiers
stay revealed **longer** as well as **deeper**. Mana is drained per tick over the window
(cost / `word_0x18`). **No sound** is played on cast.

## 3. Port (landed 2026-09-07)

- **State:** `World::beyond_sight_tier() -> Option<u8>` — MC2 reads the class-15 token
  `mc2_book.ent[12]`'s `f71` while `f26 > 0`; MC1's bool maps to tier 0.
  `RivalView::metamorphed` (MC2: the rival's `book.ent[4]` token armed) joins `invisible`.
- **Map reveal:** `entities::rival_reveals(game, rivals, tier, env, palette, icons)` replaces
  the interim 2×2 marker dot. MC1: each alive rival's NAME at `+2 px` in `TEAM_COLORS[slot].1`,
  no cloak test. MC2: `sub_63570`'s gate (tier 0 hides Invisible or Metamorphed, tier 1 hides
  Metamorphed, tier 2 hides nothing), a 1-px `playersColors[slot][0]` position pixel and the
  name at `+4 px` in the same colour. Names are runs of `mgc_render::MapStamp`s built from the
  messaging font (`UiAssets::map_glyph` → `MapIcons::glyphs`); `MapStamp` grew `offset`
  (screen-space, post-projection, so a label never rotates with the map) and `tint`.
- **Balloons:** unchanged — `map_stamps_from_poses` stamps `[66 + colour]` for own-or-armed.
- **Enemy mines:** `live_poses_mc2` skips a (10,78) whose `f52` owner is neither the human nor
  unresolved unless `beyond_sight_tier() == Some(2)` — presentation-side, so the hashed flag
  word (which also feeds the flood's shove filter `sub_39FA0`) is untouched.
- **Duration** 151/261/361 and the silent cast were already right.

## 6. Confidence, open questions, test

**Confidence:** HIGH on the mechanism. `EF:57132` (effect state), `GameUI:2219-2252` +
`:1085/1198/1492/1748/2143` (wizard reveal + tier gate), and `EF:29856-29861` (tier-2 monster
reveal) are unambiguous and mutually consistent; SPELLS.DAT row 12 values are read straight from
the baked bundle. The player's fuzzy memory ("L1 extended vision, L2 players, L3 monsters") lines
up with retail (L1 = base wizard reveal, L2 = see through Invisible, L3 = see through Metamorph +
reveal monsters) once "extended vision" is read as "the map now shows enemy positions."

**Open questions:**
1. ~~Which models route through `sub_3A8B0`~~ — resolved: it is the Magic Mine tick (class 10
   model 78), see §2d. Unverified against the shipped EXE: `byte_0x3E_62 & 7` (the re-check
   cadence) — the port evaluates every tick.
2. **"Extended vision" (render draw-distance):** no 3D-render path keys on `SpellEnabled[12]`
   (grepped GameRenderOriginal/NG/HD/GL + ViewPort — zero hits). So retail Beyond Sight does
   **not** extend the 3D view distance; it is purely a minimap reveal. If a recording shows a
   view-distance change, that would contradict the decompile and should be recorded.
3. **Metamorph interaction** is currently untestable in the port (Metamorph effect unported);
   the tier-2 "see through Metamorph" rung can only be verified once spell 4 lands.

**Playtest conditions (what each tier changes on screen):** tier 1 vs 0 — a rival under
Invisibility gains its name/pixel on the map; tier 2 vs 1 — a rival under Metamorph gains
them, AND a rival's Magic Mine becomes visible in the viewport. With no rival cloaked and no
enemy mine in view, the three tiers look identical by design (only the armed window differs).

**Suggested test:** on an MC2 level with a rival wizard, cast Beyond Sight at each tier (use the
dev-spells instrument to select tiers) while the rival is (a) plain, (b) Invisible. Assert on the
minimap: tier 0 shows the plain rival but NOT the invisible one; tier 1 shows both; tier 2 shows
both plus any nearby enemy ground creatures that are absent at tiers 0/1. A sim-level golden can
pin `beyond_sight_tier` transitions and the resulting revealed-entity count per tier without the
renderer.
