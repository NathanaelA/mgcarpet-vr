# MC2 RIVALS — SPAWN, DATA & MORTALITY (lifecycle) — port-ready verbatim trace

Port-ready verbatim trace of the **MC2 rival-wizard LIFECYCLE + DATA plumbing** for Phase 4.3b: how the 8
player slots are activated at level load, how each wizard entity is created and stamped with its authored
personality / spell tiers / starting castle, human-vs-AI asymmetries at spawn, the death/scatter/grave/
respawn sequence, and the intake/heal economy bits that are NOT the per-tick decision brain. The per-tick AI
BRAIN and casting arm are a SIBLING trace — deliberately out of scope here.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`: EF = `engine/EventsFunctions.cpp`,
EV = `engine/Events.cpp`, Level = `engine/Level.cpp`. House style per `mc2-castle-data-tables.md` (verbatim
C, renamed-field comments, RNG law `r = 9377*r + 9439`, uint8-wrap). Read `mc2-castle-builder.md` §1.2/1.3
first — the authored starting-castle stamp (EF:43775-43819) is ALREADY traced there and is only cross-cited
below. Trace date 2026-07-12.

---

## Headline findings (read first)

1. **THE 8-SLOT ACTIVATION LOOP IS `sub_53160` (EF:38088), a `do { … } while (v12 < 8)` over the player
   slots.** It runs at level init and for EVERY of the 8 wizard colors it: (a) resets the input mailbox, (b)
   snapshots+cleans the per-color stat block `array_0x2BDE[i]`, (c) sets `IsAiPlayer_0x009_2BE4_11239 = 1`
   for every non-human slot in single-player (EF:38157-38159), (d) **enqueues `playerInputs_0x6E3E[i].
   PlayerAction_byte0 = 1`** (EF:38154) — which on the NEXT input pump triggers the actual entity spawn — and
   (e) calls `InitialiseSpells_54A50(color, slot)` (EF:38287-region call at EF:38270 `InitialiseSpells_54A50
   (v12, v0index)`) which builds the wizard's book from the map header. **There is no "this slot is active"
   boolean in the header** — all 8 slots are always walked; a slot with no `array_0x2362` start marker simply
   spawns at (0,0)-derived terrain (its marker was never written). So slot activation is: *every color 0..7
   is spawned*; whether a color is the HUMAN is `color == D41A0_0.LevelIndex_0xc`, and every other color is an
   AI rival. `NumberOfPlayers_0xe` bounds the per-frame INPUT/tick loop (EF:37567), not the init loop.

2. **THE ACTUAL ENTITY SPAWN IS `sub_5C950` (EF:43600, address 0x23d950),** driven by `PlayerAction_byte0`
   in the input pump (EF:37592 switch: cases **1 / 3 / 0xF** all call `sub_5C950(&array_0x2BDE[i], actEvent)`).
   It (re)creates the `(3, IsAiPlayer)` carpet entity — **model 0 = human, model 1 = AI** — at the color's
   `array_0x2362[color]` start position (EF:43684), wires the player↔entity links, sets base stats, and for
   AI loads personality + authored starting castle. **This same function is the RESPAWN entry** (EF:60276).

3. **THE START POSITION SOURCE IS `array_0x2362[8]` (per-color `axis_3d`), written by the (3,4)..(3,11) THING
   marker ctors `sub_4A820`..`sub_4A900` (EF:33260-33313).** Each of the 8 ctors hard-writes one slot:
   `sub_4A820 → array_0x2362[0]`, `sub_4A840 → [1]`, … `sub_4A900 → [7]`. `sub_5C950` reads
   `array_0x2362[color]` and raises z to `getTerrainAlt + 0x100` (EF:43686-43688). **A color with no marker
   keeps the memset-0 position** (`array_0x2362` is zeroed at EF:39316) — it spawns at map origin. So the
   authoring contract is: place a (3,4+color) marker per participating color; missing marker = origin spawn.

4. **AI PERSONALITY + LIFE come from `WizardMapSettings_0x360D2[color]` (110-byte per-color header record),
   loaded ONLY in the `IsAiPlayer == 1` branch (EF:43761-43773):** Aggression→`word_0x242_578`,
   Perception→`word_0x244_580`, Reflexes→`word_0x246_582`, Life→`word_0x24A_586` (16.8, and it also scales
   `maxLife`). The three brain words and the Life scalar are the ONLY personality reads at spawn — everything
   else the brain reads live. (Note the field ORDER inside the struct is Aggression, **Reflexes**, Perception
   — see §2.1; the spawn code assigns them to 578/580/582 in Aggression/**Perception**/Reflexes order, so the
   name↔slot mapping is: `word_0x242`=Aggression, `word_0x244`=Perception, `word_0x246`=Reflexes.)

5. **THE SPELL BOOK + STARTING XP TIERS ARE BUILT IN `InitialiseSpells_54A50` (EF:38650), NOT at entity
   spawn.** It reads three parallel 26-byte masks from `WizardMapSettings_0x360D2[color]`:
   `StartingSpells_0x360E1x[26]` (grant flag), `byte_0x360FBx[26]` (**per-spell starting LEVEL 0..2**),
   `BlockedSpells_0x36115x[26]` (deny flag). For an **AI** it sets `SpellLevels[spell] = byte_0x360FBx[spell]`
   (clamped ≤2) directly (EF:38693) and grants the spell if not blocked (EF:38714). The book flag
   `SpellsEnabled_0x333_819x[spell]` starts as a **boolean 1**, then the per-tick `sub_55AB0` (Level:1305)
   turns each enabled flag into a live `(15, spell)` manifestation entity and stores its index back into the
   same field. **This is the rivals' starting spell-XP tier source the project's XP system needs.**

6. **DEATH → SCATTER → GRAVE → RESPAWN chain:** carpet tick sets action **2** when `life < 0` (EF:60037);
   action 2 = `sub_5E310` (EV:2882) does the death FALL + kill credit + **scatters the 26 SPELL manifestation
   entities** (class-15, not mana jars) + spawns the **(10,40) grave** + reassigns the dead wizard's (10,39)
   mana spheres to the grave + sets action **3** + `dword_0x10_16 = 1200` (respawn timer); action 3 =
   `sub_5E7C0` (EV:2895): **AI with a castle respawns** (counts `dword_0x10_16` down, then `sub_5C950`);
   **AI with NO castle is BANISHED = eliminated.** Rival elimination feeds objective **case 3** (kill enemy
   player) via the color's `byte_0x006_2BE4_11236` flag going false.

7. **CONFIRMED MC1 ASYMMETRY carries into MC2:** the HUMAN carpet tick (`AddPlayer03_00_5E010`, action 0)
   FORWARDS pending damage to its castle when at-castle (EF:59961-59977); the AI carpet tick (`sub_12A70`,
   action 1) does NOT forward — it applies `sub_5EFA0` intake to itself. Heal rates differ per tick function:
   human life `/250` home `/2000` afield (EF:60021/60029); AI life `/200` home `/500` afield (EF:5441/5449).
   **Corrects the survey's "at-castle redirect IDENTICAL" phrasing** — the redirect is HUMAN-only.

---

## 1. SLOT ACTIVATION — `sub_53160` (EF:38088), the 8-slot init loop

The level-init player loop. Runs once per level load. Bound is a literal `while (v12 < 8u)` (EF:38220) — NOT
`NumberOfPlayers`. Every color is initialised; the human is picked out by `LevelIndex_0xc`.

```c
// EF:38118  (v0index = slot 0..7, v11index = same, v12 = color 0..7)
v13 = 1;  v12 = 0;  v0index = 0;  v11index = 0;
do {
    // (a) reset this slot's input mailbox
    D41A0_0.playerInputs_0x6E3E[v11index].PlayerAction_byte0 = 0;  // + 7 more fields zeroed (EF:38124-38131)

    // (b) snapshot + clean the per-color stat block array_0x2BDE[color]
    x_D41A0_BYTEARRAY_4_struct.byteindex_256ar = D41A0_0.array_0x2BDE[v0index];
    clean_x_D41A0_BYTEARRAY_0_0x2BDE(v0index);
    D41A0_0.array_0x2BDE[v0index].dword_0x3E6_2BE4_12228.str_611.array_0x3E9_1001x =
        x_D41A0_BYTEARRAY_4_struct.byteindex_256ar. … .array_0x3E9_1001x;   // preserve the save-book flags
    if (setting_byte1_22 & MULTIPLAYER_MODE) v13 = 0;
    if (v13) sub_549A0(&array_0x2BDE[v0index]. … .str_611, &snapshot. … .str_611);
    array_0x2BDE[v0index].dword_0x018_2BDE_11254 = snapshot.dword_0x018_2BDE_11254;

    // (c) ENQUEUE SPAWN + mark AI
    D41A0_0.playerInputs_0x6E3E[v0index].PlayerAction_byte0 = 1;              // ← triggers sub_5C950 next pump
    D41A0_0.array_0x2BDE[v0index].word_0x007_2BE4_11237 = v12;
    if (!(setting_byte1_22 & MULTIPLAYER_MODE) && v12 != D41A0_0.LevelIndex_0xc)
        D41A0_0.array_0x2BDE[v0index].IsAiPlayer_0x009_2BE4_11239 = 1;        // ← every non-human color = AI

    // (d) menu/camera scaffolding
    array_0x2BDE[v0index].word_0x010_2BDE_11246 = 32;   // 32 saved camera slots
    array_0x2BDE[v0index].struct_0x1d1_2BDE_11695[0].rotation__2BDE_11701.fov = 128;
    array_0x2BDE[v0index].byte_0x3E1_2BE4_12223 = 2;
    array_0x2BDE[v0index].ActPlayerIndex_0x00e_2BDE_11244 = 32 - 1;
    for (v4=0; v4 < 32; ) { v4++; /* copy camera slot 0 → slot v4 */ }

    // (e) name + book
    strcpy(array_0x2BDE[v0index].WizardName_0x39f_2BFA_12157,
           WizardsNames_D93A0[GetTrueWizardNumber_61790(v12)]);
    InitialiseSpells_54A50(v12, v0index);                                    // ← §3

    v0index++;  v11index++;  v12++;
} while (v12 < 8u);
```

**Field homes:** `IsAiPlayer_0x009_2BE4_11239` (per-color byte on `array_0x2BDE[color]`), the human color =
`D41A0_0.LevelIndex_0xc`. `NumberOfPlayers_0xe` gates only the per-frame pump loop (EF:37567
`for (i=0; i < NumberOfPlayers_0xe; i++)`), so in single-player only the human's input is read live but ALL 8
colors were spawned + given AI brains at init.

**There is NO `player_0x2FED9`-style "active" gate for the wizard entity itself** — `player_0x2FED9[color]` is
purely the authored starting-castle LEVEL (0 = no castle, not "no player"). The 8-color loop always runs.

---

## 2. THE ENTITY SPAWN — `sub_5C950` (EF:43600, 0x23d950)

Signature: `void sub_5C950(type_str_0x2BDE* a1x, type_entity_0x6E8E* a2x)` — `a1x` = the color's stat block,
`a2x` = the existing carpet entity to reuse (or `Entities_EA3E4[0]` = "spawn fresh"). Dispatched by the input
pump on `PlayerAction_byte0` ∈ {1, 3, 0xF} (EF:37592, EF:37602/37617/37650/37672).

### 2.0 The fresh-spawn vs reuse fork (EF:43684-43706)

```c
v35x = D41A0_0.array_0x2362[a1x - &D41A0_0.array_0x2BDE[0]];   // ← START POS = array_0x2362[color]
v3 = getTerrainAlt_10C40(&v35x);  v3 += 0x100;  v35x.z = v3;   // sit 0x100 above ground
if (a2x == Entities_EA3E4[0]) {                                // FRESH SPAWN
    v2x = IfSubtypeCallCreatingManaSphere_4A190(&v35x, 3, a1x->IsAiPlayer_0x009_2BE4_11239 == 1);
    v37 = 1;                                                   //   class 3, model = (AI?1:0)
} else {                                                       // REUSE (respawn / re-anchor)
    a2x->actionIndex_0x45_69 = a1x->IsAiPlayer_0x009_2BE4_11239 == 1;   // action 0 human / 1 AI
    a2x->struct_byte_0xc_12_15.byte[0] &= 0xDFu;               //   clear hidden flag 0x20
    if (a2x->dword_0xA4_164x->CastleEntityIndex_0x3A_58)       //   if it has a castle,
        v35x = Entities_EA3E4[…CastleEntityIndex…]->position_0x4C_76;   //   respawn AT the castle
    CopyEntityPosition_57CF0(a2x, &v35x);
}
```

- **Carpet class/model = (3, IsAiPlayer)** → model 0 human, model 1 AI. This matches `AddPlayer_4A920`
  (model 0, action 0, EF:33322) and `sub_4A9C0` (model 1, action 1, EF:33346).
- **`actionIndex` = the AI flag** → action 0 = `AddPlayer03_00_5E010` (human tick), action 1 = `sub_12A70`
  (AI tick). The action index IS the human/AI split.
- **Respawn re-anchors at the castle** if one exists (EF:43700-43704); a fresh spawn uses `array_0x2362`.

### 2.1 `WizardMapSettings_0x360D2` — the 110-byte per-color header record (BasicTerrain.h:20-34) VERBATIM

```c
typedef struct {   // length 110  //word_0x360D2 ; #pragma pack(1)
    uint8_t  stuba[3];
    int16_t  Aggression_0x360D5;
    uint8_t  stubb[2];
    int16_t  Reflexes_0x360D9;
    uint8_t  stubc[2];
    int16_t  Perception_0x360DD;
    uint8_t  stubd[2];
    uint8_t  StartingSpells_0x360E1x[26];   // grant flag per spell
    uint8_t  byte_0x360FBx[26];             // per-spell STARTING LEVEL (0..2)
    uint8_t  BlockedSpells_0x36115x[26];    // deny flag per spell
    int16_t  Life_0x3612F;                  // 16.8 fixed HP/castle scalar (0 = "use default 256")
    uint8_t  stubf[15];
} Type_WizardMapSettings_0x360D2;
```

Loaded per level as `WizardMapSettings_0x360D2[8]` in the runtime level header (copied by `DecompressLevel`,
ConvertMapInfo.cpp:37 — cf. castle-data-tables §2.4). **Struct order is Aggression, Reflexes, Perception**;
the spawn code (below) assigns Aggression→578, **Perception→580, Reflexes→582**.

### 2.2 Base stat init — BOTH human and AI (EF:43708-43724)

```c
a1x->playerIndex_0x00a_2BE4_11240 = v2x - D41A0_0.struct_0x6E8E;      // color → entity link
v2x->dword_0xA4_164x = &a1x->dword_0x3E6_2BE4_12228;                  // entity → player-ext block
v2x->dword_0xA4_164x->playerColorIndex_0x38_56 = a1x - &array_0x2BDE[0];  // entity → color
v2x->dword_0xA4_164x->word_0x159_345 = 100;                          // ← GRACE = 100 (both)
v2x->dword_0xA4_164x->word_0x24C_588 = 0;
v2x->dword_0xA4_164x->dword_0x16D_365 = 2000;
memset(&v2x->dword_0xA4_164x->str_0x1AC_428, 0, 18);
v2x->dword_0xA4_164x->word_0x24A_586 = 256;                          // ← Life scalar DEFAULT 256 (1.0×)
v2x->maxMana_0x8C_140 = 1000;                                        // ← base cap 1000 (both)
v2x->maxLife_0x4 = 10000;                                            // ← base maxLife 10000 (both)
sub_5CF40(v2x, v37);
```

### 2.3 Fresh-spawn-only block (`v9 = v37`, EF:43726-43823)

Runs only on a FRESH spawn (`v37 == 1`). Zeroes the 26 per-spell XP counters, sets the carpet sprite by
color, then the AI-only branch:

```c
if (v9) {
    sub_58DA0(0, v2x);
    for (i = 0; i < 26; i++)
        v2x->dword_0xA4_164x->str_611.spellsExperience_0x2CB_715x.at(i) = 0;   // ← per-spell XP reset
    v2x->dword_0xA4_164x->time_393 = j___clock();
    switch (TransformPlayerColorIndex_616D0(color)) {                          // carpet sprite by color
        case 0: SetEntityIndexAndRot_49CD0(v2x, 44);  break;                   // human colour → 44
        case 1..7: … 273,274,275,276,277,278,279;                             // rival colours → 273..279
    }
    if (a1x->IsAiPlayer_0x009_2BE4_11239 == 1) {                               // ── AI ONLY ──
        v2x->dword_0xA4_164x->word_0x242_578 = WizardMapSettings_0x360D2[color].Aggression_0x360D5;
        v2x->dword_0xA4_164x->word_0x244_580 = WizardMapSettings_0x360D2[color].Perception_0x360DD;
        v2x->dword_0xA4_164x->word_0x246_582 = WizardMapSettings_0x360D2[color].Reflexes_0x360D9;
        v14 = WizardMapSettings_0x360D2[color].Life_0x3612F;
        if (v14) {                                                             // Life 0 ⇒ keep default 256
            v2x->dword_0xA4_164x->word_0x24A_586 = v14;                        // ← castle-HP + wizard-HP scalar
            v2x->maxLife_0x4 = v2x->maxLife_0x4 * word_0x24A_586 >> 8;         // wizard maxLife *= Life/256
        }
        if (…SpellsEnabled_0x333_819x.SpellEnabled[2]) {                       // Create-Castle known?
            if (D41A0_0.terrain_2FECE.player_0x2FED9[color]) {                 // authored starting-castle level>0
                // ─ AUTHORED STARTING CASTLE — already traced verbatim in mc2-castle-builder.md §1.2/1.3 ─
                // spawn (3,2), stamp player_0x2FED9[color] BUILD00 passes, set level = value-1,
                // sub_60810 HP/CAP ladder, start full mana (clamped ≤ 320000).  EF:43779-43819.
            }
        }
    }
    v2x->dword_0xA4_164x->creaturesKilledPercent_373 = 0;
}
```

**KEY:** the starting castle is AI-gated **and** Create-Castle-gated (`SpellEnabled[2]`). The HUMAN's starting
castle is NOT spawned here — the human never enters the `v9 && IsAiPlayer` branch. (The human's authored
castle, if any, is created through the same `player_0x2FED9` path but the decompile only wires it under the AI
branch; the human relies on casting Create-Castle. **Flag: verify whether the human ever gets an authored
starting castle in MC2 or must always cast one** — OPEN.)

### 2.4 Common spawn tail — BOTH human and AI (EF:43825-43863)

```c
if (D41A0_0.LevelIndex_0xc == color)  v2x->struct_byte_0xc_12_15.byte[0] |= 1u;   // human draw flag
v2x->life_0x8 = v2x->maxLife_0x4;                                                  // FULL LIFE
v2x->mana_0x90_144 = v2x->maxMana_0x8C_140;                                        // FULL MANA
v2x->dword_0xA4_164x->byte_0x150_336 = maxMana;
// hate-baseline seed toward THIS newcomer, written into EVERY other wizard's ledger:
for (kx = dword_38519; kx > Entities_EA3E4[0]; kx = kx->next_0)
    if (kx->id != v2x->id && (kx->model==0 || kx->model==1))
        kx->dword_0xA4_164x->array_0x1FC_508[4 + 4*color] = -24609;               // = 40927u post-spawn truce
v2x->dword_0xA4_164x->word_0x146_326 = 0;  v2x->dword_0xA4_164x->word_0x148_328 = 0;
if (v2x->model_0x40_64 == 1) {                                                    // AI-only ledger init
    memset(&…byte_0x1C1_449, 0, 1);
    for (l = 0; l < 8; l++) …array_0x1FC_508[4*l + 4] = 24607;                    // hate neutral 0x601F
    …str_611.array_0x367_871x.SpellEnabled[2] = 4 * color;
}
v2x->dword_0xA4_164x->dword_0x19A_410 = 2048;  maxDistance_0x19E_414 = 2048;
memset(v2x->dword_0xA4_164x->array_0x15B_347, 16, 8);
```

- **Human ALWAYS spawns life = maxLife = 10000, mana = 1000** (Life scalar 256, never overridden here).
- **AI spawns life = 10000·Life/256, mana = 1000** (castle full-mana clamp only in the authored-castle block).
- **Grace = 100** ticks for both (`word_0x159_345`), during which the damage mailbox is memset every tick.
- Team color = `playerColorIndex_0x38_56`; hate baseline 24607 (neutral) / 40927 (fresh-spawn truce) matches
  the MC1 `HATE_NEUTRAL`/`HATE_RESPAWN` constants in rivals.rs.

---

## 3. THE SPELL BOOK + STARTING XP — `InitialiseSpells_54A50` (EF:38650)

Runs per color in the §1 init loop. `playerIndex2` = color (source of map masks), `playerIndex` = slot
(target stat block). In single-player campaign both are the color; `setting_byte1_22 & 0x10` forces map-mask
source to color 0.

```c
// EF:38657  clear all 26 spell slots
for (i=0;i<26;i++){ array_0x437_1079x[i]=0; SpellsEnabled_0x333_819x[i]=0; array_0x403_1027x[i]=0; }
SpellIndexLeft_0x451_1105 = -1;  SpellIndexRight_0x453_1107 = -1;
tempPlayerIndex2 = (setting_byte1_22 & 0x10) ? 0 : playerIndex2;

// EF:38686  per REMAPPED spell slot (spellIndex_D94FF[i] = the book-order → spell-id remap)
for (i=0;i<26;i++) {
    result = spellIndex_D94FF[i];
    if (WizardMapSettings_0x360D2[c].byte_0x360FBx[result] > 2u)                     // clamp tier ≤ 2
        WizardMapSettings_0x360D2[c].byte_0x360FBx[result] = 2;

    if (IsAiPlayer == 1) {
        SpellLevels_0x41D_1053z.SpellIndex[result] = WizardMapSettings_0x360D2[c].byte_0x360FBx[result];  // ← AI STARTING TIER
    } else if (setting_byte1_22 & 8 || setting_byte1_22 & MULTIPLAYER_MODE) {         // human debug/MP branch
        int t = byte_0x360FBx[result];
        if (SpellLevels[result] < t) {
            t = clamp(t, 0, 2);
            SpellLevels[result] = t;
            SpellExperience_0x263_611x[result] = SPELLS[result].subspell[t].xpos2_0x12 + 1;  // XP to reach tier
        }
    }
    SpellIndexes_0x39B_923x.SpellIndex[i] = -1;
    setSpell = false;

    if (IsAiPlayer == 1) {                                                            // ── AI grant rule ──
        array_0x3CF_975x.SpellIndex[result] = WizardMapSettings_0x360D2[c].BlockedSpells_0x36115x[result];
        if (WizardMapSettings_0x360D2[c].StartingSpells_0x360E1x[result])
            if (array_0x3CF_975x.SpellIndex[result] == 0)  setSpell = true;           //   granted && !blocked
    }
    else if (!WizardMapSettings_0x360D2[c].BlockedSpells_0x36115x[result]) {          // ── human grant rule ──
        if (MULTIPLAYER_MODE) { if (StartingSpells[result] && Blocked[result]==0) setSpell = true; }
        else if (!(OptionsSettingFlag_24 & LEVEL_LOADED_FROM_ARG) && levelnumber_43w) {
            if (array_0x3E9_1001x.SpellIndex[result]) setSpell = true;                //   persistent save-book
        }
        else if (array_0x3E9_1001x.SpellIndex[result]) setSpell = true;              //   save-book (level 0)
        else if (StartingSpells[result] && !Blocked[result]) setSpell = true;        //   map default
    }
    if (setSpell) {                                                                  // EF:38751
        SpellsEnabled_0x333_819x.SpellEnabled[result] = 1;                           // ← BOOLEAN flag (not yet an entity)
        array_0x3E9_1001x.SpellIndex[result] = 1;
        if (SpellIndexLeft_0x451_1105 == -1) SpellIndexLeft_0x451_1105 = result;      // bind L/R hotkeys
        else if (SpellIndexRight_0x453_1107 == -1) SpellIndexRight_0x453_1107 = result;
        SpellIndexes_0x39B_923x.SpellIndex[index] = index;  index++;
    }
}
```

### 3.1 The book flag → live manifestation — `sub_55AB0` (Level:1305), per tick

`SpellsEnabled_0x333_819x[spell]` is a **boolean at grant time**, then reified into a class-15 entity:

```c
// Level:1305  per color, per remapped spell
for (i=0;i<26;i++) {
    if (array_0x3E9_1001x.SpellIndex[spellIndex_D94FF[i]] || array_0x403_1027x.SpellIndex[…]) {
        if (!SpellsEnabled_0x333_819x.SpellEnabled[spellIndex_D94FF[i]]) {
            tempEvent = IfSubtypeCallCreatingManaSphere_4A190(&carpet.position, 15, spellIndex_D94FF[i]); // (15, spell)
            SpellsEnabled_0x333_819x.SpellEnabled[spellIndex_D94FF[i]] = tempEvent - struct_0x6E8E;        // ← store ENTITY INDEX
            tempEvent->parentId_0x28_40 = carpet - struct_0x6E8E;
            SetSpell_6D5E0(tempEvent, array_0x437_1079x.SpellIndex[spellIndex_D94FF[i]]);                  // set level on the entity
        }
    } else if (SpellsEnabled_0x333_819x.SpellEnabled[spellIndex_D94FF[i]]) { … despawn … }
}
```

**Port consequence for the XP system:** an AI rival's starting book = `StartingSpells && !BlockedSpells`, and
its starting per-spell LEVEL (tier 0..2) = `byte_0x360FBx[spell]` (clamped ≤2). Those tiers feed straight into
`SpellLevels_0x41D_1053z` at load — the rival does NOT accrue XP up to them. The human's per-spell XP is
seeded from the save-book / `xpos2` threshold instead.

---

## 4. MORTALITY — death → scatter → grave

### 4.1 Death onset — carpet tick sets action 2 (EF:60035-60041, human tick; mirror in AI tick EF:5416-5419)

```c
// AddPlayer03_00_5E010 (human, action 0) — EF:60035
else {  // life_0x8 < 0
    a1x->actionIndex_0x45_69 = 2;                       // ← DEATH state
    a1x->word_0x2C_44 = 0;
    PrepareEventSound_6E450(a1x-…, -1, 16);             // death sound 16
    EventDispatcher::I->DispatchEvent(E_SCENE_CHANGE, Scene::DEAD);
}
// sub_12A70 (AI, action 1) — EF:5416
else if (sub_5EFA0(a1x) == 2) { a1x->actionIndex_0x45_69 = 2; return 0; }
```

### 4.2 Death fall + scatter + grave — `sub_5E310_multiplayer_test_die` (EF:60045, action 2, EV:2882) VERBATIM

```c
void sub_5E310_multiplayer_test_die(type_entity_0x6E8E* a1x) {
    sub_5D530(a1x);
    if (colorIndex_121[6]) sub_5C800(a1x, 7);
    if (array_0x2BDE[color].MenuState_0x3DF_2BE4_12221) SetMenuCursorPosition_52E90(&array_0x2BDE[color],0,false);
    v2 = a1x->word_0x2C_44 - 2;                          // ── DEATH FALL: −2/tick, floor −256 ──
    a1x->position_0x4C_76.z += a1x->word_0x2C_44;
    a1x->word_0x2C_44 = v2;
    if (v2 < -256) a1x->word_0x2C_44 = -256;
    if (a1x->word_0x2C_44 > 0) a1x->word_0x2C_44 = 0;
    v3 = a1x->dword_0xA0_160x->word_160_0xc_12;
    v4 = getTerrainAlt_10C40(&a1x->position);  v5 = v4;
    if (a1x->position.z < v3 + v4) a1x->position.z = v3 + v4;                 // clamp onto ground
    v6x = IfSubtypeCallCreatingManaSphere_4A190(&predictedAxis_EB398ar, 10, 1);   // (10,1) death FX puff
    if (v6x) { v6x->flags |= 0x80; v6x->id = a1x->id; }

    if (a1x->position.z == v5 + a1x->dword_0xA0_160x->word_160_0xc_12) {      // ── LANDED: do the payout ONCE ──
        sub_49F90();
        v8 = a1x->word_0x24_36;                                              // killer id (set by intake)
        if (v8) {
            v9x = Entities_EA3E4[v8];
            if (v9x->class_0x3F_63 == 3 && (v9x->model==0 || v9x->model==1)) {   // killer is a wizard
                v12 = a1x->dword_0xA4_164x->playerColorIndex_0x38_56;
                Entities_EA3E4[a1x->word_0x24_36]->dword_0xA4_164x->word_0x26_38[v12]++;   // ← KILL CREDIT (per-color tally)
            }
        }
        memset(&a1x->str_0x5E_94, 0, 36);                                    // clear damage mailbox
        strcpy(array_0x2BDE[color].CurrentNotificationText_…, lang[374]);    // "has died."
        array_0x2BDE[color].word_0x04f_2C2D_11309 = 1;
        array_0x2BDE[color].word_0x04d_2C2B_11307 = 100;                     // notification 100 ticks

        for (i = 0; i < 26; i++) {                                           // ── SCATTER THE 26 SPELL TOKENS ──
            v19x = Entities_EA3E4[…SpellsEnabled_0x333_819x.SpellEnabled[i]];
            if (v19x <= Entities_EA3E4[0]) { SpellEnabled[i] = 0; }
            else {
                SpellEnabled[i] = 1;                                         // book flag back to boolean
                v19x->flags &= 0xFE;                                         // detach (clear owned bit)
                v19x->actionIndex_0x45_69++;                                 // bump to "loose token" action
                predictedAxis = a1x->position;
                a1x->rand = 9377*a1x->rand + 9439;
                predictedAxis.x += (a1x->rand & 0x1FF) - 256;               // ± up to 256
                a1x->rand = 9377*a1x->rand + 9439;
                predictedAxis.y += (a1x->rand & 0x1FF) - 256;
                CopyEntityPosition_57CF0(v19x, &predictedAxis);
                a1x->rand = 9377*a1x->rand + 9439;
                v19x->life_0x8 = a1x->rand % 0x5A + 200;                     // token lifetime rand%90+200
            }
        }
        v24x = IfSubtypeCallCreatingManaSphere_4A190(&a1x->position, 10, 40); // ── SPAWN GRAVE (10,40) ──
        if (v24x) {
            a1x->actionIndex_0x45_69 = 3;                                    // ← carpet → DEAD-WAIT state 3
            a1x->dword_0x10_16 = 1200;                                       // ← RESPAWN TIMER = 1200
            for (jx = dword_38523; jx > Entities_EA3E4[0]; jx = jx->next_0)  // reassign dead wizard's mana spheres
                if (jx->model_0x40_64 == 39 && jx->playerEntityIndex_0x94_148 == a1x - struct_0x6E8E)
                    jx->playerEntityIndex_0x94_148 = v24x - struct_0x6E8E;   //   (10,39) spheres → the grave
        }
        a1x->struct_byte_0xc_12_15.byte[0] |= 0x20u;                         // hide the carpet husk
        D41A0_0.dword_0x11e6--;                                              // live-wizard count--
    }
}
```

- **Death fall:** `word_0x2C_44` accumulates −2/tick (floor −256), added to z, until the carpet lands on the
  terrain floor + `dword_0xA0_160x->word_160_0xc_12`. The payout runs ONCE, on landing.
- **Kill credit:** the killer wizard's per-color tally `word_0x26_38[deadColor]++` (an 8-wide array on the
  player-ext block, EF:60113; read for the scoreboard at EF:22345). Killer id was recorded by the intake
  (`word_0x24_36`).
- **Spell-token scatter:** the 26 class-15 manifestation entities (the book) are DETACHED from the wizard,
  bumped to a loose action, scattered ±256 tiles×256, given lifetime `rand%90+200`. **These are the SPELL
  TOKENS (not mana jars)** — a killer/other wizard can re-collect them. `SpellEnabled[i]` reverts to a
  boolean so `sub_55AB0` won't re-adopt them.
- **Grave = (10,40)**, respawn timer parked on the carpet's `dword_0x10_16 = 1200`, and the dead wizard's
  owned (10,39) mana spheres are re-pointed to the grave so their census mana flows to the grave, not a dead
  owner.

### 4.3 The world-mana census + corpse pipeline (context, already surveyed)

Corpse→mana for CREATURES is `KillEntity_1C930`→`TransformEntityToManaSphere_36BA0` (survey :9556/:26867):
(10,39) spheres, same 9377/9439 LCG, up to 16 spheres of ~1000 for big corpses, merge on contact. The wizard
death path above is the WIZARD-specific variant (spell tokens + grave), NOT the creature corpse path.

---

## 5. RESPAWN vs ELIMINATION — `sub_5E7C0_multiplayer_test_banished` (EF:60254, action 3, EV:2895) VERBATIM

```c
void sub_5E7C0_multiplayer_test_banished(type_entity_0x6E8E* a1x) {
    a1x->dword_0xA4_164x->moveBoost_0x1E_30 = 0;
    if (array_0x2BDE[color].IsAiPlayer_0x009_2BE4_11239 == 1) {          // ── AI ──
        if (a1x->dword_0xA4_164x->CastleEntityIndex_0x3A_58) {           //   HAS A CASTLE → respawn
            v4 = a1x->dword_0x10_16;
            if (v4)  a1x->dword_0x10_16 = v4 - 1;                        //   count the 1200 timer down
            else     sub_5C950(&array_0x2BDE[color], a1x);               //   → RESPAWN (reuse path, at castle)
        } else {                                                        //   NO CASTLE → BANISHED
            if (array_0x2BDE[color].byte_0x006_2BE4_11236) {
                strcpy(…CurrentNotificationText…, lang[HAS_BEEN_BANISHED]);   // "has been banished from the realm."
                array_0x2BDE[color].word_0x04f_2C2D_11309 = 1;
                array_0x2BDE[color].word_0x04d_2C2B_11307 = 200;
            }
            array_0x2BDE[color].byte_0x006_2BE4_11236 = 0;              // ← ELIMINATED FLAG cleared → objective case 3 fires
        }
    } else {                                                            // ── HUMAN ──
        sub_5C800(a1x, 7);
        sub_5E6C0(a1x);                                                 //   husk drift; no respawn (game-over via endgame seq)
    }
}
```

- **AI respawn condition = still owns a castle.** With a castle it decrements `dword_0x10_16` (the 1200 from
  §4.2) then calls `sub_5C950` (§2) which re-anchors at the castle, refills life/mana, re-grace 100, and
  `sub_55AB0` re-mints the book from `SpellsEnabled` (persisting across death — the flags were reset to
  boolean, not zeroed, for spells the wizard still "knows"). **Note the timer is checked ONCE per action-3
  tick; the castle presence is re-checked every tick — losing the castle mid-wait converts to elimination.**
- **AI banished = eliminated** (`byte_0x006_2BE4_11236 = 0`). This is the flag objective **case 3** reads.
- **Human never respawns** here (goes to the endgame sequence `sub_5E8C0` instead).

### 5.1 Win-condition interaction — objective engine `sub_58F00_game_objectives` (EF:40693)

Stage types (BasicTerrain.h:36-46). Verbatim of the two that rival elimination touches, plus the release-point
and mana ones the task named:

```c
// EF:40744  switch on stage type stages_3654C_byte0:
case 0:  // COLLECT MANA — banked % of level total from wizard's dword_0x13C_316 + castle mana (EF:40751)
case 1:  // KILL CREATURE — target entity life_0x8 <= -1  (EF:40764)
case 2:  // KILL FIXED ENTITY — life <= -1 AND fontTypeIndex_0x3D_61 == 0 (not a mana carrier) (EF:40772)
case 3:  // KILL ENEMY PLAYER — !array_0x2BDE[target.color].byte_0x006_2BE4_11236  (EF:40781) ← rival banished
case 5:  // RELEASE POINT — human within 768 of stage x/y  (EF:40803)
case 7:  // KILL ALL CREATURES OF TYPE — !bytearray_38403x[type] (EF:40829)
```

**So rival elimination feeds a `case 3` objective directly** through `byte_0x006_2BE4_11236` (set false by the
banish at EF:60299). A "kill all players" (type 8) is the same predicate over all colors. No 16-tick debounce
on these (unlike MC1's win check); the `ObjectiveDone_2` counter is the only pacing.

---

## 6. INTAKE / HEAL ECONOMY (non-brain) — `sub_5EFA0` (EF:60613) + the two carpet ticks

### 6.1 `sub_5EFA0` — the shared damage intake (returns 2 = lethal) VERBATIM essentials

```c
signed int sub_5EFA0(type_entity_0x6E8E* a1x) {
    a1x->word_0x26_38 = 0;  v1 = 0;
    …
    if (a1x->life_0x8 >= 0) {
        … duel-grip (word_0x7A_122) +1 duel XP via sub_6D8B0(…,0xE,1) …    // EF:60657
        … steal-mana (word_0x74_116) → sub_61050 …                          // EF:60666
        v8 = a1x->str_0x5E_94.word_0x62_98;                                 // pending-damage SOURCE
        if (v8) {
            a1x->word_0x26_38 = v8;
            if (shield flags) {                                             // SHIELD: quarter paid by mana
                sub_6D8B0(a1x-…, 6u, 1);                                    //   +1 shield XP
                if (byte[1] & 0x40) { v10 = dmg/4; mana -= v10; str_0x5E_94.dword_0x5E_94 = v10; … }
                else { str_0x5E_94.dword_0x5E_94 = 0; byte[1] |= 0x40; }    //   full-null one hit, re-arm
            }
            v14 = a1x->str_0x5E_94.word_0x62_98;
            a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;                // ── APPLY DAMAGE to SELF ──
            … knockback dmg/10 clamp 0..80, face the source, hit sound rand 54..57 …
            if (a1x->life_0x8 < 0) {
                a1x->word_0x24_36 = a1x->str_0x5E_94.word_0x62_98;          // record KILLER
                if (killer is (10,67) flood) a1x->word_0x24_36 = 0;         // flood = no credit
                v1 = 2;                                                     // ── LETHAL ──
            }
            if (v1 != 2) { v1 = 1;  sub_5EF70(a1x);  a1x->str_0x5E_94.word_0x62_98 = 0; }  // ← clears SOURCE, not AMOUNT
        }
    } else v1 = 2;
    if (setting_byte4_25 & 1 && !a1x->model_0x40_64) {                      // GOD-MODE cheat (human only)
        a1x->word_0x26_38 = 0;  a1x->word_0x24_36 = 0;  str_0x5E_94.word_0x62_98 = 0;
        a1x->life_0x8 = 10000;  v1 = 0;
    }
    return v1;
}
```

- **"Clears the source but NOT the amount":** on a non-lethal hit the SOURCE `word_0x62_98` is zeroed
  (EF:60725) but the AMOUNT `dword_0x5E_94` is left set (the shield branch may have rewritten it). Confirmed
  vs the survey claim (survey :204) and vs the project's `sub_46540` port — **verify our port zeroes source
  only.**
- **Killer credit** = `word_0x24_36 = word_0x62_98` on lethal (EF:60716); the (10,67) flood killer is
  suppressed (no credit).
- Shield/knockback/duel/steal are the "new intake channels" — those touch the XP system (sibling scope) but
  the mailbox mechanics are lifecycle.

### 6.2 Heal / regen rates — the divergent tail of each carpet tick

```c
// HUMAN — AddPlayer03_00_5E010 (action 0), EF:60018-60031
if (atCastle || byte[1]&0x10) { manaRegen = maxMana/200 (min 1000);  lifeRegen = maxLife/250; }
else                          { manaRegen = maxMana/2000 (min 100);  lifeRegen = maxLife/2000; }

// AI — sub_12A70 (action 1), EF:5438-5451
if (atCastle || byte[1]&0x10) { manaRegen = maxMana/200 (min 1000);  lifeRegen = maxLife/200; }
else                          { manaRegen = maxMana/2000 (min 100);  lifeRegen = maxLife/500; }
```

- **Human life regen: /250 home, /2000 afield.** **AI life regen: /200 home, /500 afield** — the AI heals
  ~4× faster afield (2000/500). Mana regen identical between them. Confirms survey :250-252.
- **At-castle damage handling** (EF:59961-59977, in `AddPlayer03_00_5E010`): the human's pending mailbox is
  copied into the castle's mailbox and the human's own grace set to 2, so the castle takes the hit. The AI
  path has no mailbox COPY, but it does NOT absorb on itself either: the brain's housekeeping pins
  `grace = 2` at the own castle and the grace branch memsets the whole mailbox (EF:5395-5414), so damage is
  DISCARDED — the AI is effectively immune at its own castle, the same observable as the human forward.
  [CORRECTED 2026-07-16, folding pedantic-review §Trace-bank corrections 1 (see the §8.6 correction below):
  this bullet previously claimed "the AI absorbs on itself", contradicting §6.3 one section down.]

### 6.3 Grace / spawn-grace wipe

`word_0x159_345` = grace counter, set 100 at spawn (§2.2), set to 2 in the at-castle human forward. While
`> 0`, the tick does `memset(&str_0x5E_94, 0, 36)` (clears the whole 36-byte damage mailbox) and decrements
it (EF:59982-59986 human, EF:5400-5414 AI). So a freshly (re)spawned wizard is damage-immune for 100 ticks.

---

## 7. FIELD HOMES (retail → meaning)

| field (retail) | where | meaning | cite |
|---|---|---|---|
| `array_0x2BDE[color]` | global | per-color STAT BLOCK (2124 B); `.dword_0x3E6_2BE4_12228` = player-ext | EF:38136 |
| `IsAiPlayer_0x009_2BE4_11239` | `array_0x2BDE[color]` | 1 = rival, 0 = human | EF:38159 |
| `LevelIndex_0xc` | global | the HUMAN's color | EF:37597,:43825 |
| `NumberOfPlayers_0xe` | global | bounds per-frame pump loop only | EF:37567 |
| `playerIndex_0x00a_2BE4_11240` | `array_0x2BDE[color]` | color → carpet ENTITY index | EF:43708 |
| `playerColorIndex_0x38_56` | carpet ext | carpet ENTITY → color | EF:43710 |
| `PlayerAction_byte0` | `playerInputs_0x6E3E[slot]` | 1/3/0xF → spawn/respawn via `sub_5C950` | EF:38154,:37592 |
| `array_0x2362[8]` | global | per-color START POSITION (`axis_3d`) | EF:33262,:43684 |
| (3,4)..(3,11) markers | THING | write `array_0x2362[0..7]` = `sub_4A820..sub_4A900` | EF:33260-33313 |
| carpet class/model | entity | class 3; model 0 human / 1 AI | EF:43691 |
| carpet action | entity | 0 human tick / 1 AI tick / 2 death / 3 dead-wait | EF:43696,:60037,:60167 |
| `WizardMapSettings_0x360D2[8]` | level header | 110-B per-color record | BasicTerrain.h:20 |
| `Aggression_0x360D5`→`word_0x242_578` | header→ext | brain hate/worth | EF:43764 |
| `Perception_0x360DD`→`word_0x244_580` | header→ext | brain spot checks | EF:43765 |
| `Reflexes_0x360D9`→`word_0x246_582` | header→ext | brain cadence/turn | EF:43766 |
| `Life_0x3612F`→`word_0x24A_586` | header→ext | 16.8 HP+castle scalar (0⇒256 default) | EF:43768 |
| `StartingSpells_0x360E1x[26]` | header | grant flag | EF:38714 |
| `byte_0x360FBx[26]` | header | per-spell STARTING LEVEL 0..2 (AI ⇒ SpellLevels) | EF:38693 |
| `BlockedSpells_0x36115x[26]` | header | deny flag | EF:38713 |
| `SpellsEnabled_0x333_819x[26]` | ext | book: bool at grant → class-15 entity idx after `sub_55AB0` | EF:38753,Level:1316 |
| `SpellLevels_0x41D_1053z[26]` | ext | per-spell level/tier | EF:38693 |
| `spellsExperience_0x2CB_715x[26]` | ext | per-spell XP (zeroed fresh spawn) | EF:43730 |
| `spellIndex_D94FF[26]` | const | book-order → spell-id remap | EF:38688 |
| `word_0x159_345` | ext | GRACE counter (100 spawn, 2 castle-fwd) | EF:43711,:59978 |
| `maxLife_0x4 / maxMana_0x8C_140` | entity | base 10000 / 1000; maxLife *= Life/256 (AI) | EF:43722-43772 |
| `word_0x2C_44` | entity | death-fall velocity (−2/tick, floor −256) | EF:60080 |
| `word_0x24_36` | entity | KILLER id (from intake) | EF:60716,:60102 |
| `word_0x26_38[8]` | ext | per-color KILL TALLY on the killer | EF:60113 |
| `dword_0x10_16` (on carpet) | entity | RESPAWN TIMER = 1200 in dead-wait | EF:60170,:60272 |
| grave | entity | class 10, model 40 | EF:60164 |
| death FX puff | entity | class 10, model 1 | EF:60092 |
| owned mana sphere | entity | class 10, model 39; `playerEntityIndex_0x94_148` = owner | EF:60173 |
| `byte_0x006_2BE4_11236` | `array_0x2BDE[color]` | ALIVE/active flag; 0 = banished (objective case 3) | EF:60299,:40781 |
| `CastleEntityIndex_0x3A_58` | ext | owner→castle; respawn condition | EF:60270 |
| `str_0x5E_94` | entity | 36-B damage mailbox; `.word_0x62_98` source, `.dword_0x5E_94` amount | EF:60671-60725 |
| hate ledger | ext | `array_0x1FC_508[4*color+4]`; 24607 neutral / 40927 truce | EF:43839,:43850 |

---

## 8. DIFFERENCES vs MC1 rivals (`crates/mgc-sim/src/mc1/rivals.rs`)

The port reuses the MC1 chassis; every divergence must be named.

**SHARED (do NOT re-port):**
- Carpet is class-3, model 0/1 (human/AI); action index IS the human/AI split.
- Start position from a per-color array written by (3,4+color) THING markers.
- Grace = 100 at spawn; the 36-byte mailbox memset while grace > 0.
- Death → fall (−2/tick, floor −256) → kill credit → **scatter the 26 SPELL manifestations (not jars)** →
  **(10,40) grave** → in-flight (10,39) spheres re-pointed to the grave → hide husk. `rivals.rs::
  rival_death_fall`/`rival_death_impact` already do this shape.
- Respawn condition = still owns a castle; castle-less = eliminated, re-checked every tick during the wait
  (`rivals.rs::rival_dead_wait` :1822-1833 matches EF:60270-60300 exactly).
- Hate baselines 24607 neutral / 40927 truce (rivals.rs `HATE_NEUTRAL`/`HATE_RESPAWN`).
- Intake: shield quarter-by-mana, knockback dmg/10 clamp 0..80, killer recorded, source-cleared-not-amount.

**MUST DIVERGE (MC2-specific — the port must branch on game):**
1. **Personality field ORDER + homes.** MC1 rivals.rs uses `aggression / accuracy / tempo` (u16_522/524/526).
   MC2 uses `Aggression_0x360D5 / Reflexes_0x360D9 / Perception_0x360DD` in the header (note struct order), and
   the LIFE scalar `Life_0x3612F` is a 4th personality field with no direct MC1 analogue on the rival config —
   in MC1 Life was `10000·L/256` flat; **in MC2 the same scalar ALSO drives castle HP** (castle-data-tables
   §2.4). Add `life: u16` (16.8, 256 default) to `RivalConfig`.
2. **Respawn timer.** MC1 = `32*((255-tempo)/8)+32` ticks (rivals.rs:1810). **MC2 = flat 1200 ticks**
   (`dword_0x10_16 = 1200`, EF:60170). Swap the timer formula for MC2.
3. **Starting spell TIERS.** MC1 rivals.rs `book: [bool;26]` + `allowed: [bool;26]` (two masks). **MC2 adds a
   THIRD 26-byte mask `byte_0x360FBx` = per-spell starting LEVEL 0..2**, written straight into `SpellLevels`
   for AI (EF:38693). The port's `RivalConfig` needs `start_level: [u8;26]` (clamped ≤2) so rivals begin at
   the authored tier without accruing XP. MC1 rivals had no per-spell starting tier.
4. **Spell book = class-15 world entities.** MC1's book was flags; **MC2 reifies each enabled spell into a
   `(15, spell)` manifestation entity via `sub_55AB0`, and death SCATTERS those entities as re-collectible
   tokens.** rivals.rs already models "die into jar-scatter" but the payload is SPELL tokens (class-15), not
   mana jars — confirm the project's `spawn_grave`/scatter uses the class-15 token entity for MC2.
5. **Heal rates.** MC1 AI heal was noted "4× the human afield" (rivals.rs module doc). **MC2 concrete rates:
   human life /250 home /2000 afield; AI life /200 home /500 afield; mana /200 home (min 1000) /2000 afield
   (min 100) for both.** Swap the MC2 numbers.
6. **At-castle damage handling.** MC1 doc says "AI at its castle DISCARDS damage" (:17975). **MC2: the HUMAN
   forwards to the castle (EF:59961); the AI DISCARDS** — the brain's housekeeping pins `grace = 2` at the own
   castle and memsets the whole mailbox while grace > 0 (EF:5395-5414), so damage never reaches the AI there.
   [CORRECTED 2026-07-15, pedantic review §Trace-bank corrections 1: this item previously read "AI keeps
   damage on ITSELF (no forward) — correct the port"; that was a misread. The port + brain trace are right;
   the MC2 AI *is* effectively immune at its own castle, same observable as MC1.]
7. **Slot activation source.** MC1 rivals were spawned from a config list; **MC2 always walks all 8 colors in
   `sub_53160`, marks non-human colors AI, and enqueues `PlayerAction_byte0 = 1`.** The starting castle is
   AI+Create-Castle gated and comes from `player_0x2FED9[color]` (castle-builder §1.2), NOT the rival config.
8. **Win-condition coupling.** MC1 had a monolithic win check. **MC2 elimination feeds the STAGED objective
   engine `sub_58F00` case 3/8 via `byte_0x006_2BE4_11236`** — the port must clear that flag on banish and let
   the objective engine (its own trace) read it, rather than hard-coding "all rivals dead = win."

---

## 9. PORT WORKLIST (Phase 4.3b)

1. **MC2 rival records loader.** Read `WizardMapSettings_0x360D2[8]` from the MC2 level header into a MC2
   `RivalConfig` variant: `aggression / reflexes / perception / life(16.8) / start[26] / start_level[26] /
   blocked[26]`. Wire the AI grant rule (EF:38714: granted && !blocked) and AI tier rule (EF:38693:
   `SpellLevels = clamp(byte_0x360FBx, 0..2)`). This is the missing "rivals spawn NO wizards" gap.
2. **8-color activation.** At MC2 level load, for every color 0..7: if `color != human`, mark AI and spawn a
   `(3,1)` carpet at `array_0x2362[color]` (from the (3,4+color) THING markers; origin if none). Human =
   `(3,0)`. Reuse the shared spawn tail (full life/mana, grace 100).
3. **AI Life → wizard maxLife + castle HP scalar.** On AI spawn, if `Life != 0`: `word_0x24A_586 = Life`,
   `maxLife = 10000·Life>>8`. This is the same scalar the ported castle HP ladder reads (castle-data-tables
   §2.4) — one field, two consumers.
4. **Authored starting castle** for AI colors with `player_0x2FED9[color] > 0` and Create-Castle known — reuse
   the already-ported castle-builder path (mc2-castle-builder §1.2). Resolve the OPEN: does the MC2 HUMAN get
   an authored starting castle, or must it cast one? (The decompile only wires it under the AI branch.)
5. **Death/respawn MC2 branch:** respawn timer 1200 (flat); scatter the class-15 spell tokens with lifetime
   `rand%90+200` at ±256; (10,40) grave; re-point owned (10,39) spheres; AI-with-castle respawns at castle,
   AI-castle-less → `byte_0x006 = 0` elimination.
6. **Heal-rate + at-castle branch** per §6.2/8-item-5/6 (game-conditioned).
7. **Objective hookup:** clear `byte_0x006`-equivalent on rival banish; feed the MC2 objective engine case 3/8
   (that engine is a separate trace — this trace only guarantees the elimination SIGNAL).
8. **Intake parity check:** confirm the project's `sub_46540` port clears damage SOURCE but not AMOUNT on
   non-lethal (§6.1), and suppresses kill credit for the (10,67) flood killer.

---

## 10. OPEN

- **Human authored starting castle in MC2.** `sub_5C950`'s starting-castle block (EF:43775) sits INSIDE the
  `IsAiPlayer == 1` branch, so only AI colors get an authored castle at spawn. Whether the MC2 human ever gets
  one from `player_0x2FED9[human]`, or always casts, needs a running check on a level authored with a human
  starting castle. (MC1's human got a free/instant first castle per the rivals.rs doc.)
- **`spellIndex_D94FF[26]` contents.** The book-order → spell-id remap array was referenced (EF:38688,
  Level:1309) but not dumped. Dump it from retail data before wiring the MC2 book so grant/tier indices land
  on the right spells.
- **`sub_61050` steal-mana intake** (EF:62082) — the steal-mana channel of `sub_5EFA0` (EF:60667) not walked;
  survey UNKNOWN. Needed only if MC2 rivals cast steal-mana (brain scope), but the mailbox channel is
  lifecycle-adjacent.
- **`word_0x26_38[8]` kill tally vs objective** — the per-color tally on the killer (EF:60113) is a scoreboard
  stat, distinct from the objective `byte_0x006` flag. Confirm the MC2 scoreboard/level-end stats read it
  (EF:22345) if porting the end-of-level summary.
