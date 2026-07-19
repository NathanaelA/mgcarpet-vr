# MC2 Spell-XP System — VERBATIM trace (port-ready)

Source tree: `reference/remc2/remc2/engine/`. Abbreviations: **EF** = `EventsFunctions.cpp`,
**E** = `Events.cpp`, **L** = `Level.cpp`, **GT** = `global_types.h`, **SP** = `Spells.h`,
**ES** = `engine_support.h`. Every constant/offset below is cited file:line against the vendored
decompile. This closes the RESEARCH-GATE that the castle traces (`mc2-castle-*.md`) left open on
"pieces / HP-factor keyed to spell level".

Cross-refs already in the bank (read for anchors, not redone):
`mc2-class9-spell-projectiles.md` (impact → `sub_6D8B0` accounting), `mc2-class15-spell-tokens.md`
(`SetSpell_6D5E0`, the pickup/grant layer), `mc2-castle-builder.md` / `-runtime.md` /
`-open-items.md` (the castle column, HP/CAP ladder `sub_60810`).

---

## 0. WHERE XP AND LEVEL LIVE (struct map)

The per-player spell substate is **`type_str_611`** (GT:174-216), embedded as `str_611` at **byte
offset 611** inside `Type_str_164` (the `dword_0xA4_164x` heavyweight player record, GT:308). All 26
spell arrays are keyed by **spell index 0..25** (the `spell_t` enum, GT:135-162 — `castle = 2`).

Relevant fields (GT:174-216):

| field | type | GT line | meaning |
|---|---|---|---|
| `SpellExperience_0x263_611x.SpellExperience[26]` | `int32[26]` | 175 | **banked / carried** XP (persists across levels — §5) |
| `spellsExperience_0x2CB_715x[26]` | `int32[26]` | 176 | **volatile / this-level** XP (the accumulator `sub_6D8B0` writes) |
| `SpellsEnabled_0x333_819x.SpellEnabled[26]` | `int16[26]` | 178 | per-spell **entity index** of the granted spell manifestation (0 = not owned) |
| `SpellLevels_0x41D_1053z.SpellIndex[26]` | `uint8[26]` | 204 | **current spell LEVEL 0/1/2** (derived, §2) |
| `array_0x437_1079x.SpellIndex[26]` | `uint8[26]` | 205 | **selected sub-spell tier** per spell (clamped ≤ level) |
| `array_0x3B5_949x` / `array_0x3E9_1001x` / `array_0x403_1027x` | `uint8[26]` | 189/202/203 | grant/availability flags (token layer) |
| `spellIndex_0x458_1112` / `subSpellIndex_0x459_1113` | `int8` | 211/212 | the actively-selected spell + sub-spell (CTRL pane) |

**Effective XP for a spell = `SpellExperience[i] + spellsExperience[i]`** (banked + volatile). This
sum is computed identically at every read site: EF:43889, EF:43939, EF:22649.

The spell TABLE is `SPELLS_BEGIN_BUFFER_str[26]` — 26 rows of `type_SPELLS_BEGIN_BUFFER_str`
(SP:20-25): `{int8 byte_0; uint8 isEnabled_1; subspell[3]}`, each tier
`{subSpellIndex_2, manaCost_6, maxManaLimit_A, xpos1_E, xpos2_0x12, hintText, word_0x18, life_0x1A,
fontType_0x1B}` (SP:7-18). This is `crates/mgc-sim/src/mc2/spells.rs::Mc2SpellRow` verbatim.
**`byte_0` = the tier COUNT = max level + 1** (used as the loop start in §2). `xpos1_E`/`xpos2_0x12`
are the XP ladder thresholds (single-player uses `xpos1_E`, multiplayer uses `xpos2_0x12`).

---

## 1. `sub_6D8B0` — THE XP-AWARD FUNCTION (VERBATIM, EF:58228)

```c
// EF:58228 (addr 0x24e8b0)
void sub_6D8B0(unsigned __int16 a1, unsigned __int16 a2, __int16 a3)
{
    type_entity_0x6E8E* v3x;
    int v5;
    if (!(x_D41A0_BYTEARRAY_4_struct.setting_38545 & 4))              // (A) global XP-disable gate
    {
        if (a1)                                                        // a1 = entity id (0 = noop)
        {
            v3x = Entities_EA3E4[a1];
            if (v3x->class_0x3F_63 == 3 && !v3x->model_0x40_64)        // (B) only class-3 model-0 = a PLAYER BODY
            {
                v5 = v3x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x.at(a2);
                v3x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x.at(a2) = a3 + v5;   // (C) += a3
                if (a2 == 2)                                           // (D) castle spell → resync manifestation tier
                    SetSpell_6D5E0(Entities_EA3E4[...SpellsEnabled_0x333_819x.SpellEnabled[2]],
                                   ...array_0x437_1079x.SpellIndex[2]);
                if (x_D41A0_BYTEARRAY_4_struct.setting_byte1_22 & Setting::MULTIPLAYER_MODE)
                {
                    if (a1 == D41A0_0.array_0x2BDE[D41A0_0.LevelIndex_0xc].playerIndex_0x00a_2BE4_11240)
                        sub_6DAD0(&...str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2);              // (E) MP level-up
                }
                else
                {
                    sub_6D9C0(&...str_611, &SPELLS_BEGIN_BUFFER_str[a2], a2, 0, 1);            // (F) SP level-up
                }
            }
        }
    }
}
```

Semantics:
- **`a1`** = entity **id** of the caster's body; **`a2`** = **spell index** 0..25; **`a3`** = XP delta.
- **(A)** `setting_38545 & 4` (ES:372) — a global flag that suppresses ALL XP accrual (recorded-run /
  no-progression mode). Port must gate on this.
- **(B)** award is a **no-op unless the id resolves to a class-3, model-0 entity** = a live player
  body (mob bodies are class-3 model≠0; castles are class-3 model-2 → excluded). So XP only ever
  lands on human/AI **wizard** records.
- **(C)** the accumulator that actually grows is **`spellsExperience_0x2CB_715x[a2]`** (the volatile
  array, NOT the banked one). `a3` is **added** (can be any amount; callers pass 1, or a computed
  batch). No cap here except the castle-specific clamp inside `sub_6D9C0`/`UpdateExperience` (§2).
- **(D)** for the **castle spell (a2 == 2)** the manifestation's active tier is immediately
  re-synced via `SetSpell_6D5E0` so the castle spell entity reflects the new selected tier.
- **(E)/(F)** — after adding, it **re-derives the level** for that one spell: single-player calls
  `sub_6D9C0` (threshold model, `xpos1_E`); multiplayer calls `sub_6DAD0` (incremental, `xpos2_0x12`).
  (E) also only re-levels for the local human player.

### 1.1 THE COMPLETE XP-AWARD TABLE (every caller of `sub_6D8B0`)

`a1` is the owner id in each case (`parentId_0x28_40` for a manifestation, `id_0x1A_26` for a
projectile carrying its owner). `spell_t` names per GT:135-162.

| a2 (spell idx) | a3 | event / call site | file:line |
|---|---|---|---|
| **model of impacted entity** | 1 | generic projectile impact → award to the spell whose model matches what was hit | EF:62985, EF:63551 |
| 0 = fireball | 1 | fireball hit | EF:63189 |
| 1 = possession | 1 | possession applied | EF:59052, EF:63314 |
| 2 = castle | 1 | **castle built / levelled one step** (`sub_60480` / `sub_605E0` create path) | EF:61596 |
| 3 = speed_up | 1 | speed-up spell resolved | EF:56243 |
| 4 = metamorph | 1 | metamorph resolved | EF:56321 |
| 5 = heal | 1 | heal resolved | EF:56453 |
| 6 = shield | 1 | shield resolved | EF:60678 |
| 7 = lightning | v2 / 1 | lightning hit (arc) | EF:24802, EF:58411, EF:58826 |
| 8 = rebound | 1 | rebound / ricochet | EF:55273 |
| 9 = meteor | v4 | meteor impact | EF:23871 |
| 0xA=10 teleport | 1 | teleport resolved | EF:56909 |
| 0xB=11 invisible | 1 | invisibility resolved | EF:57085 |
| 0xC=12 beyond_sight | 1 | beyond-sight resolved | EF:57146 |
| 0xD=13 steal_mana | 1 | mana-steal resolved | EF:62123 |
| 0xE=14 duel | 1 | duel resolved | EF:60657 |
| 0xF=15 tremor | v15 | tremor / fissure area | EF:29580 |
| 0x10=16 crater | v3 | crater / ground-fire area | EF:23521 |
| 0x11=17 earthquake | v3 | earthquake / fire-trail area | EF:23525 |
| 0x12=18 volcano | v39 | apocalypse-dome / raise-land area | EF:23395 |
| 0x13=19 summon_army | 1 | summon resolved | EF:10861 |
| 0x14=20 gravity_well | v8 / 1 | flood-quake / gravity area | EF:29374, EF:29437 |
| 0x15=21 whirlwind | v31 / v1 | whirlwind area | EF:24407, EF:24444 |
| 0x16=22 fools_mana | 1 | fool's-mana | EF:26636, EF:26646, EF:26663 |
| 0x17=23 magic_mine | 1 | magic-mine trip | EF:29979 |
| 0x18=24 alliance | 1 | alliance resolved | EF:10998 |

Notes:
- Several `a3` are **run-time counts** (v-vars) not literal 1 — e.g. area spells award per affected
  tile/target. The class-9 projectile trace's `(id,0,1)` / `(id,7,1)` / `(id,8,1)` observations are
  the fireball / lightning / rebound rows above.
- **EF:62985 / EF:63551** are the *interesting* generic form: `sub_6D8B0(ownerId,
  Entities_EA3E4[a1x->word_0x26_38]->model_0x40_64, 1)` — the projectile awards **+1 to whichever
  spell index equals the model of the thing it just hit** (the "the spell you hit a creature-type
  with levels up" rule). Port must map impacted-entity model → spell index.

### 1.2 BATCH awards (not via per-hit `sub_6D8B0`)

- **`UpdateExperience_6E090(spells, countXP)`** (EF:44262): adds `countXP` to **every enabled spell**,
  then applies the castle clamp. Used by the **mana-pool pickup** (EF:41183): `+4` XP single-player
  / `+50` multiplayer to all enabled spells + sound 63; multiplayer additionally runs `sub_6DBD0`
  (the MP full re-level, §2). Also the `case 0x24e910` dispatch entry (E:3906).
- **Spell-tome / global-boost pickup** (EF:24080-24089): loops all 26 spells and calls
  `sub_6D8B0(playerId, i, xpos1_E(tier2) >> 9)` — i.e. adds ≈1/512 of the spell's max threshold to
  each; effectively a small even bump toward the next level.
- **Cheat "More Spell XP"** (EF:37865-37869): `+100` to every spell's `spellsExperience_0x2CB_715x`,
  then `sub_6DB50(0,0)` (re-level all).

---

## 2. THE LEVEL-UP LAW

Two derivations, both read **effective XP = banked + volatile** and both write
`SpellLevels_0x41D_1053z.SpellIndex[a3]`.

### 2.1 Single-player — `sub_6D9C0` (VERBATIM, EF:43873)

```c
void sub_6D9C0(type_str_611* a1x, type_SPELLS_BEGIN_BUFFER_str* a2x, __int16 a3, char a4, char a5)
{
    int v5 = 0, v6, v7; char v10;
    if ((a1x->array_0x3E9_1001x.SpellIndex[a3] || a1x->SpellsEnabled_0x333_819x.SpellEnabled[a3])
        && (isCaveLevel_D41B6 || a3 != 25))
        v5 = 1;                                                        // v5 = "spell is owned/usable"
    if (x_D41A0_BYTEARRAY_4_struct.setting_byte2_23 >= 0
        && a1x->spellsExperience_0x2CB_715x.at((int)spell_t::castle) > 7)     // ── CASTLE XP CLAMP ──
        a1x->spellsExperience_0x2CB_715x.at((int)spell_t::castle) = 7;
    v6 = a2x->byte_0;                                                  // start at tier COUNT
    v7 = a1x->spellsExperience_0x2CB_715x.at(a3) + a1x->SpellExperience_0x263_611x.SpellExperience[a3];
    do
        v6--;
    while (v6 >= 0 && v7 < a2x->subspell[v6].xpos1_E);                 // highest tier whose xpos1_E is met
    if (v6 < 0) v6 = 0;
    if (v6 != a1x->SpellLevels_0x41D_1053z.SpellIndex[a3])            // level changed?
    {
        a1x->SpellLevels_0x41D_1053z.SpellIndex[a3] = v6;
        if (v5 && a5) sub_6DC40_improve_ability(a3);                   // notify (only if owned, a5=notify)
    }
    v10 = a1x->SpellLevels_0x41D_1053z.SpellIndex[a3];
    if (a1x->array_0x437_1079x.SpellIndex[a3] > v10)                   // clamp SELECTED sub-tier ≤ level
        a1x->array_0x437_1079x.SpellIndex[a3] = v10;
    if (v5 && a4)                                                      // a4 = "bank it"
    {
        if (v7 >= a2x->subspell[2].xpos1_E)
            a1x->SpellExperience_0x263_611x.SpellExperience[a3] = a2x->subspell[2].xpos1_E;   // cap banked at tier2 xpos1
        else
            a1x->SpellExperience_0x263_611x.SpellExperience[a3] = v7;
    }
}
```

- **The level = the highest tier index `v6` (searching down from `byte_0`) whose `xpos1_E` the
  effective XP `v7` reaches.** So a spell with `byte_0 == 3` and thresholds
  `xpos1_E = {t0,t1,t2}` gives: `XP < t1` → level 0; `t1 ≤ XP < t2` → level 1; `XP ≥ t2` → level 2.
  (Tier-0's own `xpos1_E` is the floor; if XP < it, `v6` still clamps to 0.)
- **`a4`** = "commit v7 into the banked `SpellExperience` array" (called with `a4=0` from `sub_6D8B0`,
  so per-hit awards do NOT bank; banking happens on the sweep paths §5). Banked XP is **capped at
  tier-2's `xpos1_E`** (can't over-bank).
- **`a5`** = "show the level-up notification". `sub_6D8B0` passes `a5=1` → per-award level-ups DO notify.
- **CASTLE XP CLAMP (EF:43885-43886):** the castle spell's volatile XP is **hard-capped at 7** when
  `setting_byte2_23 >= 0`. This matches the **7 castle build levels** — see §4.

### 2.2 Multiplayer — `sub_6DAD0` (VERBATIM, EF:43931)

```c
void sub_6DAD0(type_str_611* a1x, type_SPELLS_BEGIN_BUFFER_str* a2x, __int16 a3)
{
    if (a1x->SpellsEnabled_0x333_819x.SpellEnabled[a3]
        && a1x->SpellLevels_0x41D_1053z.SpellIndex[a3] < a2x->byte_0 - 1)         // not already max
    {
        int v3 = a1x->spellsExperience_0x2CB_715x.at(a3) + a1x->SpellExperience_0x263_611x.SpellExperience[a3];
        if (v3 < 0) v3 = 0;
        if (v3 > a2x->subspell[2].xpos2_0x12 + 2) v3 = a2x->subspell[2].xpos2_0x12 + 2;   // clamp
        if (v3 >= a2x->subspell[a1x->SpellLevels_0x41D_1053z.SpellIndex[a3] + 1].xpos2_0x12) // reach NEXT tier
        {
            a1x->SpellLevels_0x41D_1053z.SpellIndex[a3]++;                        // ONE step up
            sub_6DC40_improve_ability(a3);
        }
    }
}
```

- Multiplayer uses **`xpos2_0x12`** thresholds and steps up **at most one level per award** (checks
  only the *next* tier). Single-player jumps straight to the fully-earned tier.

### 2.3 The full re-level sweeps

- **`sub_6DB50(a1,a2)`** (EF:43957) — single-player: loops all 26 spells calling
  `sub_6D9C0(v5, &SPELLS_BEGIN_BUFFER_str[i], i, a1, a2)` for the **local player only**.
  `a1`=bank flag, `a2`=notify flag.
- **`sub_6DBD0()`** (EF:43979) — multiplayer counterpart: loops all 26 calling `sub_6DAD0`.

### 2.4 WHERE the comparison happens; notification; can levels drop?

- **At award time.** Every `sub_6D8B0` re-derives the level immediately (§1 E/F). The batch pickups
  call the sweep. The CTRL/spell-menu UI (§3.3) only *reads* `SpellLevels` for the XP bar; it does
  not re-derive.
- **Notification** = **`sub_6DC40_improve_ability(ability)`** (VERBATIM, EF:44007):
  ```c
  sprintf(printbuffer, langindexbuffer[159], langindexbuffer[160 + ability]);  // "Your ability to cast %s has improved."
  SetCurrentNotificationMessage_19760(printbuffer, 5u, 200);                    // on-screen msg, priority 5, 200 ticks
  PrepareEventSound_6E450(D41A0_0.LevelIndex_0xc, -1, 61);                       // sound 61
  ```
  Message string index **159** (format) + **160+spellIndex** (spell name); sound **61**. No hand-flash.
- **Can levels be LOST?** In single-player YES *in principle*: `sub_6D9C0` re-derives from the XP
  total each call, so if effective XP fell below a threshold the level would drop. In practice XP is
  monotonic within a level (only ever added), and the banked component is committed on the sweep — so
  levels do not regress during play. **Multiplayer never lowers** (`sub_6DAD0` only ++).
  The one deliberate DROP is the castle (§4): the castle *entity level* `dword_0x10_16` decrements on
  a lethal hit, but that is the castle build stage, not the spell level.

---

## 3. THE EFFECT OF LEVEL ON CASTING

### 3.1 Tier index = level — `SetSpell_6D5E0` (VERBATIM, L:1505)

```c
void SetSpell_6D5E0(type_entity_0x6E8E* entity, int spellId)
{
    int locSpellId = spellId;
    if (locSpellId > SPELLS_BEGIN_BUFFER_str[entity->model_0x40_64].byte_0 - 1)   // clamp to max tier
        locSpellId = SPELLS_BEGIN_BUFFER_str[entity->model_0x40_64].byte_0 - 1;
    if (entity->word_0x2E_46) { entity->word_0x2C_44 = locSpellId + 1; }
    else {
        entity->byte_0x46_70 = locSpellId;                                        // ← the ACTIVE TIER
        entity->subSpellIndex_0x2A_42 = SPELLS_BEGIN..subspell[locSpellId].subSpellIndex_2;   // damage/effect id
        entity->word_0x30_48          = SPELLS_BEGIN..subspell[locSpellId].word_0x18;
        entity->byte_0x3B_59          = (SPELLS_BEGIN..subspell[locSpellId].fontType_0x1B & 1) == 0;
        entity->manaRegen_0x88_136    = SPELLS_BEGIN..subspell[locSpellId].maxManaLimit_A;     // pool gate
        int mana = GetSpellManaCost_6D710(parent, entity->model_0x40_64, locSpellId);
        entity->maxMana_0x8C_140 = mana;
        if (entity->word_0x30_48) mana /= entity->word_0x30_48;
        entity->mana_0x90_144 = mana;
        if (OptionsSettingFlag_24 & 0x20) { entity->manaRegen_0x88_136 = 0; entity->mana_0x90_144 = 1; }  // free-cast cheat
    }
}
```

The manifestation's **`byte_0x46_70`** is the sub-spell tier and it indexes `subspell[tier]` for
**every** derived property. The tier passed in is the player's **selected** sub-tier
(`array_0x437_1079x.SpellIndex[spell]`), which §2.1 clamps to ≤ `SpellLevels`. So:

**level ↑ ⇒ the player may select a higher `subspell[tier]` ⇒:**

| property | source | effect |
|---|---|---|
| `subSpellIndex_2` | subspell[tier].subSpellIndex_2 | the **damage / area amount / effect-model id** (bigger at higher tier; the CD table's row-18 `{400,800,1200}` etc.) |
| `manaCost_6` / `maxMana_0x8C_140` | via `GetSpellManaCost_6D710` | mana per shot |
| `maxManaLimit_A` | subspell[tier].maxManaLimit_A | the **castle mana-pool gate** — a tier is castable only if the castle pool ≥ this (EF:22503-22505, 22604-22606) |
| `word_0x18` | subspell[tier].word_0x18 | shot divisor (`mana /= word_0x30_48`) |
| `life_0x1A` | subspell[tier].life_0x1A | selects **charged variants** in `sub_6DCA0` |

### 3.2 `life_0x1A` → charged sub-effects (`sub_6DCA0`, EF:44020)

The projectile spawner keys the effect model on the tier's `life_0x1A` (EF:44053-44091):
- **fireball** (a3==0): `life_0x1A >= 2` → `IfSubtypeCallCreatingManaSphere_4A190(pos, 9, 28)` +
  `byte_0x44_68 = 76` (the **charged fireball subtype 28**); else subtype 0 (EF:44080-44091). This is
  the "fireball subtype 28 when `life_0x1A >= 2`" that `spells.rs` flagged.
- **lightning** (a3==7): `life_0x1A > 2` → default; `life_0x1A != 0` → subtype 12 / model tweak
  (EF:44053-44078).

So the SPELLS.DAT `life` field per tier is the switch that upgrades the *shape* of the spell at high
levels, independent of the numeric `subSpellIndex_2`.

### 3.3 Cast-readiness + XP bar (the CTRL pane, EF:22450-22676)

The spell menu reads (not writes):
- `spellIndex3 = SpellLevels_0x41D_1053z.SpellIndex[spell]` (EF:22583) — sub-tiles above this are
  drawn dark/locked (`subSpellIndex2 > spellIndex3` → blank panel, EF:22611-22613).
- **XP progress bar** (EF:22633-22670): for the tier == current level, bar fill =
  `(effectiveXP − xpos[tier]) / (xpos[tier+1] − xpos[tier])`, using **`xpos1_E`** single-player /
  **`xpos2_0x12`** multiplayer (EF:22639-22649) — the same thresholds as the level law. Confirms
  `xpos1/xpos2` ARE the ladder.
- Castability per tile gated by `maxManaLimit_A ≤ castle mana` (EF:22503-22505).

---

## 4. CASTLE LINKAGE (closes `mc2-castle-open-items.md` §7 spell-level gate)

The **castle spell = index 2** (`spell_t::castle`, GT:138). Its level/XP feeds castle growth as
follows — three decoupled but linked facts, all decompile-cited:

1. **Castle build/level-up gives spell XP, not the reverse.** Each castle build step `sub_60480`
   (EF:61564) does `dword_0x10_16++` (castle entity level 0..7) then `sub_6D8B0(ownerId, 2, 1)` —
   **+1 castle spell XP per level built** (EF:61596). The downgrade path `sub_605E0` decrements the
   castle level but does not remove spell XP (builder trace §5).
2. **The castle spell XP is hard-capped at 7** (EF:43885-43886, mirrored EF:44269-44270) — i.e. it
   tracks the **7 buildable castle levels** exactly. So the castle "spell level"
   `SpellLevels[2] ∈ {0,1,2}` is derived from that capped XP via the same §2 law, but the *castle
   entity level* `dword_0x10_16 ∈ {0..7}` is the real growth counter and lives on the **castle
   entity**, not in `str_611`.
3. **HP / CAP are keyed to the castle ENTITY level `dword_0x10_16`, via `sub_60810` (EF:61695).**
   The re-cast/level-up and the downgrade both call `sub_60810(a1x)` (EF:61594 build, EF:61605
   downgrade) and `sub_613D0` (rebuild visible pieces). From the castle traces (verbatim there):
   - **CAP ladder** `[5000, 8500, 18000, 38800, 78600, 158200, 317400, 300000000]` indexed by
     `dword_0x10_16` (runtime trace §4a, EF:61705-61728).
   - **HP ladder** `{—,20000,40000,40000,60000,60000,80000,80000}`, **Life-personality-scaled** by a
     `number1` factor (builder §7.1, EF:61695+).
   - **Visible stage pieces = (10,79)** driven by `array_0x24E_590[9+level]` part table (builder §3.2
     / §7).
4. **Where the castle SPELL level is read at runtime:** `SpellLevels_0x41D_1053z.SpellIndex[2]` is
   consulted by the **AI cast/target loops** (EF:5591, 5629, 5698, 5703) as the ceiling of a
   `for (i = level; i >= 0; ...)` scan choosing which castle sub-tier the AI can invoke — it does NOT
   set HP/CAP. And `sub_6D8B0`'s `a2 == 2` branch re-syncs the castle **manifestation tier** via
   `SetSpell_6D5E0(SpellEnabled[2], array_0x437_1079x.SpellIndex[2])` (EF:58247).

**Conclusion for the RESEARCH-GATE:** castle **pieces and HP/CAP are functions of the castle ENTITY
level `dword_0x10_16` (0..7)**, which advances **one step per successful castle-spell re-cast**
(each re-cast = another `sub_60480` build step, gated by space `sub_11A10` and mana, not by spell
XP). Spell-XP for index 2 is a *shadow* of that (capped at 7) driving only the CTRL-pane bar and the
AI tier scan. The project's existing MC1 `sub_60810` port is HP/CAP-correct in shape; the OPEN delta
remains the **MC2 numeric HP/CAP values + Life-factor scaling** already listed in the castle traces,
NOT the spell-level plumbing (now closed).

---

## 5. PERSISTENCE ACROSS LEVELS (campaign carry-over)

- **The carry-over copy is `sub_549A0` (VERBATIM, L:1261):**
  ```c
  void sub_549A0(type_str_611* a1x, type_str_611* a2x) {
      a1x->array_0x3E9_1001x       = a2x->array_0x3E9_1001x;     // grant/availability flags
      a1x->SpellExperience_0x263_611x = a2x->SpellExperience_0x263_611x;   // ← BANKED XP carried
      a1x->SpellLevels_0x41D_1053z    = a2x->SpellLevels_0x41D_1053z;      // ← LEVELS carried
      a1x->array_0x3B5_949x        = a2x->array_0x3B5_949x;
      a1x->array_0x437_1079x       = a2x->array_0x437_1079x;     // selected sub-tiers carried
  }
  ```
  It copies **banked XP, levels, and selected tiers** from the campaign-save block
  (`x_D41A0_BYTEARRAY_4_struct.byteindex_256ar.…str_611`) into the live level record. It does **NOT**
  copy `spellsExperience_0x2CB_715x` (the volatile per-level accumulator) — that starts fresh each
  level and is folded into the bank only when a sweep runs with `a4=1`.
- **Callers:** `L:303` (level-start restore into the current level's player block) and
  `EF:38149` (per-player restore in the level-transition / new-level path, loop over players).
- **So:** spell **levels and banked XP persist across the campaign**; the per-level volatile XP does
  not. The `SpellsEnabled` manifestation indices are re-created per level by `sub_55AB0` (L:1305) from
  the carried `array_0x3E9_1001x`/`array_0x403_1027x` grant flags — tying back to the class-15
  token-grant layer (`mc2-class15-spell-tokens.md`): a token pickup sets the grant flag; level init
  re-spawns the manifestation and calls `SetSpell_6D5E0(entity, array_0x437_1079x.SpellIndex[i])`
  (L:1319) to restore its tier.

---

## 6. SUMMARY (port checklist)

1. `str_611` (offset 611 in the player record) holds per-spell: **volatile XP**
   `spellsExperience[26]`, **banked XP** `SpellExperience[26]`, **level** `SpellLevels[26]`, **selected
   tier** `array_0x437[26]`, **manifestation index** `SpellsEnabled[26]`.
2. **Effective XP = banked + volatile.** `sub_6D8B0(id, spellIdx, amount)` adds `amount` to volatile
   XP for a **class-3 model-0 body only**, gated by `setting_38545 & 4`, then re-derives level. Full
   award table in §1.1; batch awards (mana pickup +4/+50, tome, cheat +100) in §1.2.
3. **Level law:** SP `sub_6D9C0` = highest tier whose **`xpos1_E`** ≤ effective XP (scan down from
   `byte_0`); MP `sub_6DAD0` = one step when effective XP ≥ next tier's **`xpos2_0x12`**. Notify =
   `sub_6DC40_improve_ability` (msg 159/160+idx, sound 61). Selected tier clamped ≤ level.
4. **Level → cast:** `SetSpell_6D5E0` copies `subspell[tier]` into the manifestation
   (`byte_0x46_70`=tier): damage `subSpellIndex_2`, `manaCost`, `maxManaLimit_A` gate, `life_0x1A` →
   charged variants (fireball subtype 28 when life≥2).
5. **Castle:** castle spell (idx 2) XP capped at 7 = the 7 build levels; each **re-cast** advances the
   castle **entity** level `dword_0x10_16` (`sub_60480` +1 & +1 XP), which indexes the HP/CAP ladder
   `sub_60810` and piece table `sub_613D0`. Spell-level plumbing gate is now CLOSED; numeric HP/CAP +
   Life-factor remain the castle traces' open items.
6. **Persistence:** `sub_549A0` carries **banked XP + levels + selected tiers** across levels
   (L:303, EF:38149); volatile per-level XP resets each level.

---

## 7. OPEN ITEMS

- **`xpos1_E` vs `xpos2_0x12` values are per-row in SPELLS.DAT** — the actual thresholds are the
  imported `spells.bin` (already parsed in `spells.rs`). NOT re-tabulated here; confirm the ladder by
  dumping row `.tiers[t].xpos1/xpos2` at port time. The single-player ladder is `xpos1`, multiplayer
  `xpos2` — **verify the bake carries both** (parser does).
- **`setting_byte2_23 >= 0` as the castle-clamp guard** (EF:43885, 44269): its meaning (likely
  "campaign/objective mode present") is unconfirmed; the clamp value **7** is decompile-literal.
- **`a4` (bank) call sites:** `sub_6D8B0` always passes `a4=0`; the banking (`SpellExperience` commit)
  only happens through `sub_6DB50(…, a2)`/pickup sweeps. Which exact frame calls the sweep with
  `a4=1` (end-of-level fold) is inferred from `sub_549A0`'s copy-of-banked but the *fold* call site
  was not pinned — OPEN. Search `sub_6DB50(1, …)` at port time (seen at EF:44290 in the "max spells"
  cheat `sub_6E0D0`).
- **`word_0x26_38` → impacted model** in the generic award EF:62985/63551: confirm the index is the
  *hit* entity's `model_0x40_64` maps 1:1 to a spell index for all creature models (the table is the
  identity on 0..25 but creature models can exceed 25 — those awards are silently dropped by the
  `.at(a2)` bound / class-3 guard). Low-risk but note at port.
- **Life-personality HP scaling factor `number1`** for the castle — deferred to the castle traces
  (their §7 OPEN), not resolvable from the spell-XP path.
