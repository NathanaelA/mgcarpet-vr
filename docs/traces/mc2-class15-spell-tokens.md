I now have comprehensive coverage. Let me do one final verification on the `word_0x2E_46` check in the creator path — confirming that when SetSpell is called with word_0x2E_46==0 (fresh token), it writes byte_0x46_70, subSpellIndex, word_0x30_48 (fire duration), manaRegen, maxMana, mana. And confirm the strF1 table matches the AddSpellXX wrappers (0x2321A0 = AddSpell00_00_511A0). That's confirmed by the table at 2032 (0x002321A0) matching wrapper comment `//2321a0`. 

I have all deliverables. Let me compile the final report.

---

# CLASS-15 SPELL-TOKEN Verbatim Trace (remc2 / Magic Carpet 2)

All citations are `file:line` relative to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`. Files: `EventsFunctions.cpp` (EF), `Events.cpp` (E), `Level.cpp` (L), `Spells.h`, `global_types.h`.

## 0. Class-15 dispatch tables (context anchor)

The master class-dispatch table `str_D4C48ar` entry for class 15 (`EF:2076`):
```
0x002A5C44, 0x0000000F, 0x0000, x_DWORD_D4C52ar_strF0, x_DWORD_D4C52ar_strF1  //15
```
- `strF0` (`EF:1949`) = **action/effect table**, 79 live entries (index `0x00..0x4E`) + null terminator (`EF:2029`).
- `strF1` (`EF:2031`) = **creator table**, 26 entries `0x2321A0..0x2324C0` — these RVAs are exactly the `AddSpellNN_..._511xx` wrappers (`EF:2032` `0x002321A0` = `AddSpell00_00_511A0`//2321a0 at `EF:54145`). So both the CAST path and the level authoring path enter class-15 through the SAME 26 creator wrappers → the shared core `AddSpellXX_XX_51120`.

Field format of each table row `type_D4C52ar2` is `{magic 0x2A5C44, index, funcAddressRVA, enabled=1}`. The RVA in column 3 maps 1:1 to the `//24xxxx` address comment on the C++ function definitions.

---

## 1. Creator `AddSpellXX_XX_51120` (`EF:54124`) — FULL VERBATIM

```c
type_entity_0x6E8E* AddSpellXX_XX_51120(axis_3d* position, char type, char a3)  //232120
{
    type_entity_0x6E8E* event = NewEvent_4A050();
    if (event) {
        event->class_0x3F_63       = 0xF;          // class = 15
        event->maxLife_0x4         = 0;
        event->model_0x40_64       = type;         // model = spell index 0..25
        event->actionIndex_0x45_69 = a3;           // action state = type*3
        event->life_0x8            = 0;
        event->struct_byte_0xc_12_15.byte[0] &= 0xF7;   // clear bit 3 (0x08)
        AddEventToMap_57D70(event, position);
        SetEntityIndexAndRot_49CD0(event, 77);          // sprite/particle index = 77 (fixed)
        SetEntityShiftRot_49EA0(event, 768, 1280);      // extents pitch=roll=768, fov=1280
        CopyMaxLifeToLife_49A20(event);                 // life = maxLife = 0
        SetSpell_6D5E0(event, 0);                        // wire spell data, subspell=0
    }
    return event;
}
```

Field-write order and constants (all literal, no OPEN):
1. `class_0x3F_63 = 0x0F` (`EF:54129`).
2. `maxLife_0x4 = 0` (`EF:54130`).
3. `model_0x40_64 = type` (`EF:54131`).
4. `actionIndex_0x45_69 = a3` (`EF:54132`).
5. `life_0x8 = 0` (`EF:54133`).
6. `struct_byte_0xc_12_15.byte[0] &= 0xF7` — clears flag bit `0x08` (`EF:54134`). NewEvent had set `struct_byte_0xc_12_15.dword = 8` (`E:567`), so this clears that default bit; the `0x04` "added-to-map" bit is set later by AddEventToMap.
7. `AddEventToMap_57D70(event, position)` (`EF:54135`) — registers in the map linked list: sets `nextEntity_0x18_24=0`, `oldMapEntity_0x16_22 = mapEntityIndex_15B4E0[(y>>8<<8)+(x>>8)]`, links, copies `position_0x4C_76`, sets `byte[0] |= 4` (`EF:40315-40327`).
8. `SetEntityIndexAndRot_49CD0(event, 77)` (`EF:54136`) → `SetEntityIndex_49C90` sets `word_0x5A_90=77`, `animationFrame_0x5C_92=0`, `byte_0x5D_93 = x_BYTE_D8A2E[particlesParameters_D951C[77].byte_12]`; then extents from `particlesParameters_D951C[77]`: `yaw = rotSpeed_8/2`, `pitch = speed_6/2`, `roll = speed_6/2`, `fov = rotSpeed_8/2` (`EF:32830-32845`).
9. `SetEntityShiftRot_49EA0(event, 768, 1280)` (`EF:54137`) — **overwrites** extents just set: `pitch=768`, `roll=768`, `fov=1280` (leaves `yaw` from step 8) (`EF:32874-32879`). These 768/1280 are the pickup collision half-extents.
10. `CopyMaxLifeToLife_49A20` → `life_0x8 = maxLife_0x4 = 0` (`EF:54118-54120`).
11. `SetSpell_6D5E0(event, 0)` (`EF:54139`).

**How the 26 models differ:** There is **NO per-model data table inside the creator** and **NO switch**. The only per-model variance is the two args `type` and `a3=type*3`, supplied by the 26 thin wrappers `AddSpell00..25` (`EF:54145-54298`), each of form `return AddSpellXX_XX_51120(position, N, 3*N);`. The wrappers are collected into `arsub_2a881e[26]` (`EF:54300-54303`). Sprite is a FIXED particle index 77 for every model (step 8) — models do NOT get distinct sprite rows here; per-spell visual/behaviour data lives in `SPELLS_BEGIN_BUFFER_str[model]` consumed by `SetSpell` and by the effect states. No RNG is drawn in the creator.

Wrapper `type`→`a3` table (all 26): model N → `actionIndex = 3N` (0,3,6,…,75).

---

## 2. `SetSpell_6D5E0` (`L:1505`) — FULL VERBATIM

```c
void SetSpell_6D5E0(type_entity_0x6E8E* entity, int spellId)  //24e5e0
{
    int locSpellId = spellId;
    // clamp subspell to [0 .. byte_0-1]  (byte_0 = number of subspell tiers for this model)
    if (locSpellId > SPELLS_BEGIN_BUFFER_str[entity->model_0x40_64].byte_0 - 1)
        locSpellId = SPELLS_BEGIN_BUFFER_str[entity->model_0x40_64].byte_0 - 1;

    if (entity->word_0x2E_46) {                    // ACTIVE (cast in progress)
        entity->word_0x2C_44 = locSpellId + 1;     // stash pending tier only
    } else {                                       // IDLE / TOKEN / fresh
        entity->byte_0x46_70          = locSpellId;                                  // current subspell tier
        entity->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[m].subspell[locSpellId].subSpellIndex_2;
        entity->word_0x30_48          = SPELLS_BEGIN_BUFFER_str[m].subspell[locSpellId].word_0x18;   // fire duration/divisor
        entity->byte_0x3B_59          = (SPELLS_BEGIN_BUFFER_str[m].subspell[locSpellId].fontType_0x1B & 1) == 0;
        entity->byte_0x3C_60          = 0;
        entity->fontTypeIndex_0x3D_61 = 0;
        entity->manaRegen_0x88_136    = SPELLS_BEGIN_BUFFER_str[m].subspell[locSpellId].maxManaLimit_A;
        int mana = GetSpellManaCost_6D710(Entities_EA3E4[entity->parentId_0x28_40], m, locSpellId);
        entity->maxMana_0x8C_140      = mana;
        if (entity->word_0x30_48) mana /= entity->word_0x30_48;
        entity->mana_0x90_144         = mana;
        if (x_D41A0_BYTEARRAY_4_struct.OptionsSettingFlag_24 & 0x20) {  // "free mana" option
            entity->manaRegen_0x88_136 = 0;
            entity->mana_0x90_144      = 1;
        }
    }
}
```
(`m` = `entity->model_0x40_64`.) Semantics:
- It does **NOT** touch the `SpellEnabled[]` array itself — it only initialises the spell entity's per-cast data from the global spell-definition table `SPELLS_BEGIN_BUFFER_str[model]`. (The `SpellEnabled[]` wiring is done by the CALLERS, see §6.)
- `byte_0` of `SPELLS_BEGIN_BUFFER_str[model]` is the count of usable subspell tiers; `spellId`/`subSpellIndex` selects among `subspell[0..2]`.
- `word_0x2E_46 != 0` (active cast) short-circuits to just staging `word_0x2C_44 = tier+1`; the full re-init only happens when idle. This is the token/cast guard.
- `GetSpellManaCost_6D710` (`L:1714`): base = `subspell[tier].manaCost_6`; special-cased for `spellIndex==2` (castle) scaling by castle upgrade level `dword_0x10_16` → 1000/10000/…/320000/300000000 (`L:1729-1753`); `+3000` if `byte_0x1BE_446` and no castle (`L:1723-1726`).

`SPELLS_BEGIN_BUFFER_str` layout (`Spells.h:20-25`): `{ int8 byte_0; uint8 isEnabled_1; subspell[3] }`, each subspell (`Spells.h:7-18`): `{ subSpellIndex_2, manaCost_6, maxManaLimit_A, xpos1_E, xpos2_0x12, hintText_0x16x, word_0x18, life_0x1A, fontType_0x1B }`.

---

## 3. PICKUP MECHANICS — core `sub_68FF0` (`EF:55676`)

The pickup logic lives entirely on the **token side** (the class-15 entity's own action tick scans all wizards), NOT in the wizard tick. The wizard tick `AddPlayer03_00_5E010` (`EF:59955`) does regen/death only and never touches token collection (verified `EF:59955-60042`).

Entry: the token's idle action states call `sub_68FF0(a1x, model, actionIndex-1_or_2)` (see §4). Full pseudocode (`EF:55676-55760`):

```c
signed int sub_68FF0(type_entity_0x6E8E* a1x, char a2 /*model*/, char a3 /*target state*/) {
    v3 = a1x->life_0x8;
    v12 = 0;
    if (v3 && (a1x->life_0x8 = v3 - 1, v3 == 1)) {   // life>0: count down; hit 0 -> vanish
        DisableEntityDrawing04_57F10(a1x);            //   sets byte[1] |= 4 (stop drawing)
    } else {                                          // life==0 (authored token) OR still alive
        v4 = getTerrainAlt_10C40(&a1x->position_0x4C_76);
        sub_580E0(&a1x->position_0x4C_76, v4, 0, 0x2000, -128);   // snap-to-ground / bob (z toward terrain, step 0x2000, -128 offset)
        if (!(a1x->byte_0x3E_62 & 3)) {               // only on some frames (network stagger)
            for (ix = firstEntity; ix > Entities_EA3E4[0]; ix = ix->next_0) {   // scan entity list
                if (!ix->model_0x40_64 && ix->life_0x8 >= 0) {                  // ix is a wizard (model 0), alive
                    if (ix->dword_0xA4_164x->playerColorIndex_0x38_56 == D41A0_0.LevelIndex_0xc
                        && !(a1x->struct_byte_0xc_12_15.byte[0] & 1)
                        && ix->...SpellEnabled[a1x->model_0x40_64]) {
                        a1x->struct_byte_0xc_12_15.byte[0] |= 1u;               // mark "owned" (local player already has it)
                    }
                    if (sub_106C0(ix, a1x)) {          // PROXIMITY TEST (AABB overlap, extents 768/768/1280)
                        v7 = ix->...SpellEnabled[a2] ? 1 : 0;
                        if (!v7) {                     // wizard does NOT yet have this spell -> COLLECT
                            PrepareEventSound_6E450(ix - base, -1, 18);          // pickup SOUND id 18
                            a1x->struct_byte_0xc_12_15.byte[0] |= 1u;           // token flag: owned
                            a1x->struct_byte_0xc_12_15.byte[3] &= 0xFD;         // clear bit1 of byte[3]
                            a1x->parentId_0x28_40 = ix - base;                  // owner = this wizard
                            a1x->actionIndex_0x45_69 = a3;                      // advance token to next state
                            a1x->word_0x36_54 = 64;                             // timer = 64
                            ix->...SpellEnabled[a2] = a1x - base;               // GRANT: store token entity index
                            ix->...array_0x403_1027x.SpellIndex[a2] = 1;        // mark spell present
                            // assign to a quick-slot (left/right) if free:
                            if (a1x->word_0x4A_74) { ...word_0x4A_74 handling... }
                            else if (SpellIndexLeft==-1 || SpellIndexRight!=-1) v12=1;
                            if (v12) { SpellIndexLeft = model; SubSpellIndexLeft = SpellIndex[model]; }
                            else     { SpellIndexRight= model; SubSpellIndexRight= SpellIndex[model]; }
                            SetSpell_6D5E0(a1x, ix->...array_0x437_1079x.SpellIndex[model]);  // init to player's chosen tier
                            return 1;
                        }
                    }
                }
            }
        }
    }
    return 0;
}
```

Key answers:
- **Radius / proximity:** `sub_106C0(ix, a1x)` (`EF:3720`) → `sub_10630` (`EF:3712`): AABB test `|dx| < a2.pitch+a4.pitch && |dy| < a2.roll+a4.roll && |(z1+a2.yaw)-(z2+a4.yaw)| < a2.fov+a4.fov`. The token's half-extents are 768/768/1280 (set by creator step 9). So collection is an axis-aligned box overlap, not a Euclidean radius.
- **What gets granted:** `wizard->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[model] = tokenEntityIndex` (`EF:55726`) and `array_0x403_1027x.SpellIndex[model] = 1` (`EF:55727`); optionally bound to left/right quick-slot (`EF:55743-55749`). `SpellEnabled[]` is `int16_t[26]` per-player (`global_types.h:124,178`); `SpellIndex[]` is `uint8[26]` (`global_types.h:119`). The array is indexed by **model = spell index 0..25**.
- **Sound:** `PrepareEventSound_6E450(wizardIdx, -1, 18)` (`EF:55718`).
- **Despawn / re-enable semantics:** The token does NOT free its slot on pickup. It flips `byte[0] |= 1` (owned), rebinds `parentId` to the collector, advances `actionIndex` to the next state, and now belongs to the wizard as their live spell entity (its index is stored in `SpellEnabled[model]`). The token entity is REUSED as the wizard's castable spell object. Vanishing (when a life-timer expires) is only `DisableEntityDrawing04_57F10` = set draw-off flag `byte[1] |= 4` (`EF:40332-40334`) — the slot is NOT freed there either.

**Scatter re-enable confirmation (`sub_5E310`, `EF:60045`, loop `EF:60137-60162`):** on wizard death, for each spell `i` in 0..25:
```c
v19x = Entities_EA3E4[wizard->...SpellEnabled[i]];
if (v19x <= Entities_EA3E4[0]) wizard->...SpellEnabled[i] = 0;   // no such spell
else {
    wizard->...SpellEnabled[i] = 1;                 // sentinel marker (NOT an index anymore)
    v19x->struct_byte_0xc_12_15.byte[0] &= 0xFE;    // clear "owned" bit0  -> becomes collectible again
    v19x->actionIndex_0x45_69++;                    // advance token action state
    // scatter ±256 around wizard using the wizard LCG rand_0x14_20 = 9377*r + 9439:
    pos = wizard.pos;
    pos.x += (r & 0x1FF) - 256;
    pos.y += (r & 0x1FF) - 256;
    CopyEntityPosition_57CF0(v19x, &pos);
    v19x->life_0x8 = (9377*r+9439) % 0x5A + 200;     // life = 200..289
}
```
This **confirms** tokens toggle draw/live flags rather than freeing the slot: on death the wizard's owned spell entities are turned back into physical pickups by clearing `byte[0]&0x01` (owned), bumping `actionIndex`, scattering ±256, and setting `life` 200..289 (`0x5A=90` → `%90 + 200`). After the loop it spawns a `(10,40)` marker via `IfSubtypeCallCreatingManaSphere_4A190(&pos, 10, 40)` (`EF:60164`) and sets the wizard `actionIndex=3`, `dword_0x10_16=1200`, `byte[0]|=0x20`. The exact fields toggled per re-enabled token: `SpellEnabled[i]→1`, `byte[0]&=0xFE`, `actionIndex++`, `position` (scatter), `life` (200..289).

---

## 4. Class-15 IDLE/TOKEN action states

Each spell model M owns 3 consecutive `strF0` states (indices 3M, 3M+1, 3M+2). An **authored token** is created (§1) with `actionIndex = 3M` — i.e. it starts in the spell's ACTIVE/effect function (e.g. model 0 → `sub_693F0`//spell fire, `EF:55832`). But that effect function is gated on `word_0x2E_46 > 0` (`EF:55841`); a fresh token has `word_0x2E_46 == 0`, so the effect body is skipped and the token is inert in that state until picked up/cast.

The **pickup/idle behaviour** lives in states 3M+1 and 3M+2, which are thin wrappers:
- **State 3M+1** → `sub_692A0` (`EF:55768`): `return sub_68FF0(a1x, model, actionIndex-1)`. Pure pickup scan (see §3). The `a3=actionIndex-1` means on collection it sets the token BACK to state 3M (the active/cast slot), ready to be fired.
- **State 3M+2** → `sub_69250` (`E:5243`): `result = sub_68FF0(a1x, model, actionIndex-2)`; if collected, **also spawn a replacement token**:
  ```c
  resultx = arsub_2a881e[model](&a1x->position_0x4C_76);   // create fresh token via creator wrapper
  if (resultx) resultx->actionIndex_0x45_69 += 2;          // new token starts in state 3M+2 (spinning pickup)
  ```
  So state 3M+2 is the "self-replenishing pickup" state: when collected it drops a new token in its place (again in state 3M+2). This is the state a persistent authored/spinning pickup sits in.

Animation/bob/snap-z of the idle token (inside `sub_68FF0`, `EF:55696-55697`): every tick it reads terrain altitude `getTerrainAlt_10C40` and calls `sub_580E0(&pos, terrainAlt, 0, 0x2000, -128)` — snaps/eases the token's z toward ground (step magnitude `0x2000`, z-offset `-128`). The spinning visual comes from the particle entity index 77 rotation (`array_0x52_82.yaw` from `particlesParameters_D951C[77]`, set in creator step 8, NOT overwritten). Proximity check DOES live here (the entity scan + `sub_106C0`).

State transition helpers:
- `sub_692C0` (strF0[0x4E], `EF:55774`): if `sub_59DC0(a1x)` (owner-flight complete), set `actionIndex = 3*model+1`, `word_0x26_38=0`, `byte[3] |= 2` — returns a fired/possession spell to pickup state.
- `sub_69300` (state 78, `EF:55792`): despawn-to-owner — clears `SpellEnabled[model]=0`, `actionIndex=78`, repositions on owner, clears left/right slot bindings. This is the "spell consumed/returned" path.
- `sub_59DC0` (`EF:41199`): in-flight/return-to-caster interpolation used by possession-class effects.

---

## 5. `strF0` inventory (index → handler; NO deep tracing)

79 live entries. Pattern per spell model **M** = `{3M: effect fn, 3M+1: pickup wrapper→sub_692A0, 3M+2: pickup+respawn wrapper→sub_69250}`. Spell names per `spell_t` enum (`global_types.h:135-162`, model index == enum value). Address→fn mapping resolved from `strF0` column-3 RVAs against `//24xxxx` definitions:

| idx | addr | fn | spell (model) / role |
|----|------|-----|------|
| 0x00 | 24A3F0 | `sub_693F0` | model0 fireball — EFFECT (//spell fire) |
| 0x01 | 24A600 | `sub_69600`→sub_692A0 | model0 pickup |
| 0x02 | 24A620 | `sub_69620`→sub_69250 | model0 pickup+respawn |
| 0x03 | 24A640 | `sub_69640` | model1 possession — EFFECT (//spell posses) |
| 0x04 | 24AA70 | `sub_69A70`→sub_692A0 | model1 pickup |
| 0x05 | 24AA90 | `sub_69A90`→sub_69250 | model1 pickup+respawn |
| 0x06 | 24AAB0 | `sub_69AB0` | model2 castle — EFFECT |
| 0x07 | 24AD70 | `sub_69D70`→sub_692A0 | model2 pickup |
| 0x08 | 24AD90 | `sub_69D90`→sub_69250 | model2 pickup+respawn |
| 0x09 | 24ADB0 | `GetScroll_69DB0` | model3 speed_up — EFFECT |
| 0x0A | 24AFF0 | `AllCreaturesKilled_69FF0`→(pickup) | model3 pickup |
| 0x0B | 24B010 | `sub_6A010`→sub_69250 | model3 pickup+respawn |
| 0x0C | 24B030 | `sub_6A030` | model4 metamorph — EFFECT |
| 0x0D | 24B2C0 | `sub_6A2C0`→sub_692A0 | model4 pickup |
| 0x0E | 24B2E0 | `sub_6A2E0`→sub_69250 | model4 pickup+respawn |
| 0x0F | 24B300 | `sub_6A300` | model5 heal — EFFECT |
| 0x10 | 24B440 | `sub_6A440`→sub_692A0 | model5 pickup |
| 0x11 | 24B460 | `sub_6A460`→sub_69250 | model5 pickup+respawn |
| 0x12 | 24B480 | `sub_6A480` | model6 shield — EFFECT |
| 0x13 | 24B580 | `sub_6A580`→sub_692A0 | model6 pickup |
| 0x14 | 24B5A0 | `sub_6A5A0`→sub_69250 | model6 pickup+respawn |
| 0x15 | 24B5C0 | `sub_6A5C0` | model7 lightning — EFFECT |
| 0x16 | 24B9C0 | `sub_6A9C0`→sub_692A0 | model7 pickup |
| 0x17 | 24B9E0 | `sub_6A9E0`→sub_69250 | model7 pickup+respawn |
| 0x18 | 24BA00 | `sub_6AA00` | model8 rebound — EFFECT |
| 0x19 | 24BAC0 | `sub_6AAC0`→sub_692A0 | model8 pickup |
| 0x1A | 24BAE0 | `sub_6AAE0`→sub_69250 | model8 pickup+respawn |
| 0x1B | 24BB00 | `sub_6AB00` | model9 meteor — EFFECT |
| 0x1C | 24BD00 | `sub_6AD00`→sub_692A0 | model9 pickup |
| 0x1D | 24BD20 | `sub_6AD20`→sub_69250 | model9 pickup+respawn |
| 0x1E | 24BD60 | `sub_6AD60` | model10 teleport — EFFECT |
| 0x1F | 24C180 | `sub_6B180`→sub_692A0 | model10 pickup |
| 0x20 | 24C1A0 | `sub_6B1A0`→sub_69250 | model10 pickup+respawn |
| 0x21 | 24C1C0 | `sub_6B1C0` | model11 invisible — EFFECT |
| 0x22 | 24C2D0 | `sub_6B2D0`→sub_692A0 | model11 pickup |
| 0x23 | 24C2F0 | `sub_6B2F0`→sub_69250 | model11 pickup+respawn |
| 0x24 | 24C310 | `sub_6B310` | model12 beyond_sight — EFFECT |
| 0x25 | 24C3A0 | `sub_6B3A0`→sub_692A0 | model12 pickup |
| 0x26 | 24C3C0 | `sub_6B3C0`→sub_69250 | model12 pickup+respawn |
| 0x27 | 24C3E0 | `sub_6B3E0` | model13 steal_mana — EFFECT |
| 0x28 | 24C5D0 | `sub_6B5D0`→sub_692A0 | model13 pickup |
| 0x29 | 24C5F0 | `sub_6B5F0`→sub_69250 | model13 pickup+respawn |
| 0x2A | 24C610 | `sub_6B610` | model14 duel — EFFECT |
| 0x2B | 24C830 | `sub_6B830`→sub_692A0 | model14 pickup |
| 0x2C | 24C850 | `sub_6B850`→sub_69250 | model14 pickup+respawn |
| 0x2D | 24C870 | `sub_6B870` | model15 tremor — EFFECT |
| 0x2E | 24CA70 | `sub_6BA70`→sub_692A0 | model15 pickup |
| 0x2F | 24CA90 | `sub_6BA90`→sub_69250 | model15 pickup+respawn |
| 0x30 | 24CAB0 | `sub_6BAB0` | model16 crater — EFFECT |
| 0x31 | 24CCB0 | `sub_6BCB0`→sub_692A0 | model16 pickup |
| 0x32 | 24CCD0 | `sub_6BCD0`→sub_69250 | model16 pickup+respawn |
| 0x33 | 24CCF0 | `sub_6BCF0` | model17 earthquake — EFFECT |
| 0x34 | 24CEF0 | `sub_6BEF0`→sub_692A0 | model17 pickup |
| 0x35 | 24CF10 | `sub_6BF10`→sub_69250 | model17 pickup+respawn |
| 0x36 | 24CF30 | `sub_6BF30` | model18 volcano — EFFECT |
| 0x37 | 24D130 | `sub_6C130`→sub_692A0 | model18 pickup |
| 0x38 | 24D150 | `sub_6C150`→sub_69250 | model18 pickup+respawn |
| 0x39 | 24D170 | `sub_6C170` | model19 summon_army — EFFECT |
| 0x3A | 24D3A0 | `sub_6C3A0`→sub_692A0 | model19 pickup |
| 0x3B | 24D3C0 | `sub_6C3C0`→sub_69250 | model19 pickup+respawn |
| 0x3C | 24D3E0 | `sub_6C3E0` | model20 gravity_well — EFFECT |
| 0x3D | 24D5E0 | `sub_6C5E0`→sub_692A0 | model20 pickup |
| 0x3E | 24D600 | `sub_6C600`→sub_69250 | model20 pickup+respawn |
| 0x3F | 24D620 | `sub_6C620` | model21 whirlwind — EFFECT |
| 0x40 | 24D830 | `sub_6C830`→sub_692A0 | model21 pickup |
| 0x41 | 24D850 | `sub_6C850`→sub_69250 | model21 pickup+respawn |
| 0x42 | 24D870 | `sub_6C870` | model22 fools_mana — EFFECT |
| 0x43 | 24DA80 | `sub_6CA80`→sub_692A0 | model22 pickup |
| 0x44 | 24DAA0 | `sub_6CAA0`→sub_69250 | model22 pickup+respawn |
| 0x45 | 24DAC0 | `sub_6CAC0` | model23 magic_mine — EFFECT |
| 0x46 | 24DCE0 | `sub_6CCE0`→sub_692A0 | model23 pickup |
| 0x47 | 24DD00 | `sub_6CD00`→sub_69250 | model23 pickup+respawn |
| 0x48 | 24DD20 | `sub_6CD20` | model24 alliance — EFFECT |
| 0x49 | 24DF60 | `sub_6CF60`→sub_692A0 | model24 pickup |
| 0x4A | 24DF80 | `sub_6CF80`→sub_69250 | model24 pickup+respawn |
| 0x4B | 24DFA0 | `sub_6CFA0` | model25 cave_in — EFFECT |
| 0x4C | 24E1C0 | `sub_6D1C0`→sub_692A0 | model25 pickup |
| 0x4D | 24E1E0 | `sub_6D1E0`→sub_69250 | model25 pickup+respawn |
| 0x4E | 24A2C0 | `sub_692C0` | shared: return-fired-spell-to-pickup (state 78 helper) |

Note the last entry 0x4E is NOT a 27th spell — it's a shared terminal state reused across spells (`sub_692C0`, `EF:55774`); `actionIndex=78` (`sub_69300`) is out of the table (a special despawn state, not indexed). The 26 spell names above are from the `spell_t` enum (`global_types.h:135`); the two decompiler comments in-source confirm model0=fire (`EF:55832`) and model1=posses (`EF:55915`). OPEN: the `spell_t` enum ordering differs from the descriptive comment block in `Spells.h:28-54` (which is a different Bullfrog subtype numbering) — I rely on `spell_t` since it is indexed by model directly; treat individual names as best-effort, roles (EFFECT vs pickup) are certain.

---

## 6. How CASTING enters class-15 (survey's `Level.cpp:1313`)

Confirmed. The cast/spell-book wiring is `sub_55AB0` (`L:1304-1336`), which loops all 26 spells and, when a wizard has learned spell `i` but has no live entity yet:
```c
type_entity_0x6E8E* tempEvent =
    IfSubtypeCallCreatingManaSphere_4A190(&Entities_EA3E4[wizardIdx]->position_0x4C_76, 15, spellIndex_D94FF[i]);   // L:1313
if (tempEvent) {
    playStr->...SpellEnabled[i] = tempEvent - D41A0_0.struct_0x6E8E;   // register entity index
    tempEvent->parentId_0x28_40 = wizardEntity - base;                // owner = wizard
    tempEvent->struct_byte_0xc_12_15.byte[0] |= 1u;                    // mark OWNED (not a free pickup)
    SetSpell_6D5E0(tempEvent, playStr->...array_0x437_1079x.SpellIndex[i]);  // set chosen subspell tier
}
```
`IfSubtypeCallCreatingManaSphere_4A190(position, 15, subtype)` (`E:5186`):
```c
if (str_D4C48ar[15].dword_14[subtype].dword_10 && str_D4C48ar[15].dword_14[subtype].word_4 == subtype)
    return pre_sub_4A190_axis_3d(str_D4C48ar[15].dword_14[subtype].address_6, position);
```
`str_D4C48ar[15].dword_14` is the `strF1` creator table → `address_6` = one of the `AddSpellNN_..._511xx` wrappers → `AddSpellXX_XX_51120`. **So a CAST-created spell entity uses the identical creator as a level-authored TOKEN.**

**How the creator distinguishes CAST from TOKEN — it does NOT; the caller does, via post-creation field writes:**
- A **level-authored TOKEN**: created by the level loader calling an `AddSpellNN` wrapper directly. After creation it has `parentId=0` (NewEvent default), `byte[0]&0x01 == 0` (creator cleared 0x08 but never sets 0x01), `life=0`, `word_0x2E_46=0`. It sits in `actionIndex=3M` inert, then is scanned into pickup via its idle states (§4). It is collectible because `byte[0]&0x01` is clear.
- A **CAST/spell-book entity** (via `sub_55AB0`): the caller immediately sets `parentId = wizard` (`L:1317`), `byte[0] |= 1` = OWNED (`L:1318`), and records its index in `SpellEnabled[i]` (`L:1316`). The OWNED bit `0x01` is exactly what `sub_68FF0` checks (`EF:55706`, `EF:55714/55716`) to skip re-collection, and what the scatter clears (`EF:60150`) to convert it back to a pickup. Firing is triggered separately by `sub_5F7B0` (`EF:60976`) setting `word_0x2E_46 = word_0x30_48` (>0), which enables the effect state body (`EF:55841`).

So the discriminator is the pair of caller-set fields **`parentId_0x28_40` and `struct_byte_0xc_12_15.byte[0] & 0x01` (OWNED)** plus **`SpellEnabled[model]`** registration — none written by the creator, all written by the cast caller. The active-vs-idle sub-distinction within an owned entity is **`word_0x2E_46`** (0=idle/ready, >0=actively casting). There is no position-based or separate-flag TOKEN marker; position argument is the same channel for both.

---

### Summary of certain vs OPEN
- CERTAIN: creator body/constants; `SetSpell` body/semantics; pickup core `sub_68FF0` (radius=AABB 768/768/1280, grant into `SpellEnabled[]`, sound 18, no slot-free); scatter re-enable field toggles (SpellEnabled→1, byte[0]&=0xFE, actionIndex++, scatter ±256, life 200..289, marker (10,40)); 3-states-per-model `strF0` layout and full address→fn table; cast path uses same creator; cast/token discriminated by caller-set `parentId`+`byte[0]&0x01`+`SpellEnabled[]`, active via `word_0x2E_46`.
- OPEN: exact human-readable spell name per model beyond the two source-annotated ones (fire=model0, posses=model1) — I used the `spell_t` enum (`global_types.h:135`) which is model-indexed but conflicts with the alternate numbering comment in `Spells.h:28`; state-role assignment (EFFECT vs pickup) is not affected by this. The 80th nominal slot: table has 79 live entries (0x00–0x4E); `x_DWORD_D4C52ar_strF0[80]` is declared size 80 but entry 79 is the `{0,0,0,0}` terminator (`EF:2029`).