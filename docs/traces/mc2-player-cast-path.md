# MC2 PLAYER CAST PATH — Verbatim Trace (input → sub_6DCA0 → spawn)

All citations are `file:line` relative to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
Files: `EventsFunctions.cpp` (EF), `Events.cpp` (E), `Level.cpp` (L), `Player.cpp`, `Sound.cpp`, `Spells.h`, `global_types.h`.

Cross-refs (do not re-derive):
- `docs/traces/mc2-class9-spell-projectiles.md` — the class-9 flight states + partial `sub_6DCA0` sketch (this doc supersedes/completes it).
- `docs/traces/mc2-class15-spell-tokens.md` — class-15 spell tokens, `SetSpell_6D5E0`, pickup, the strF0/strF1 tables, the 3-states-per-model layout.
- `crates/mgc-sim/src/mc2/spells.rs` — SPELLS.DAT (26 rows × 80 bytes, 3 subspell tiers each).

---

## 0. Big picture — TWO tables, ONE cast core

A MC2 spell that a wizard has learned lives as a **class-15 entity** (its index stored in `player->…SpellEnabled[model]`, model = spell index 0..25). That entity is the "spell object". Casting is **not** a one-shot call: it is a multi-tick *cast-in-progress* driven by the spell entity's own **EFFECT action-state** (`strF0` index `3·model`), which fires every tick while `word_0x2E_46 > 0`.

Chain:
```
player button / rival AI
  → sub_5F660(caster, spellEntity, buttonBit)      [the CAST GATE]  EF:60874
    → mana check (EF:60953) + per-model precondition switch
    → sub_5F7B0(spellEntity, caster, bit): word_0x2E_46 = word_0x30_48   EF:60973  (arm the cast)
  → next tick(s): spellEntity's EFFECT state (strF0[3·model]) runs, gated on word_0x2E_46>0
    → sub_68D50(spellEntity, caster)   [per-tick mana affordability]  EF:55548
    → if word_0x2E_46 == word_0x30_48 (FIRST tick of cast) → spawn:
        projectile spells: sub_6DCA0(caster, &caster.pos, a3=spellClass, &subspell[tier], caster.actSpeed, 1)   EF:44020
        direct spells:     write caster flags / IfSubtypeCallCreatingManaSphere_4A190(...)
    → sub_68DE0(spellEntity, caster)   [commit mana drain]  EF:55569
    → word_0x2E_46-- each tick; at 0 → sub_6D880 (apply pending tier change)  EF:58215
```

The single dispatcher `sub_6DCA0` (EF:44020) is shared by: the class-15 effect states (player + rival cast), the class-9 creature/duel casters (EF:29990), and MC1-fireball creature bolts. **`sub_6DCA0` only spawns class-9 flight projectiles**; the non-projectile spells never reach it.

---

## 1. THE FULL CHAIN — input to `sub_6DCA0`

### 1.1 Current-spell selection state (which spell, which TIER)

Per-player spell state lives in `dword_0xA4_164x->str_611` (the player-context struct `str_611`, `global_types.h`):
- **`SpellsEnabled_0x333_819x.SpellEnabled[model]`** = int16[26]: entity index of the learned spell object (0 = not learned). Indexed by **model = spell index 0..25**.
- **`array_0x437_1079x.SpellIndex[model]`** = uint8[26]: the player's **chosen tier** (0..2) for each spell. THIS is the per-spell level.
- **`SpellIndexLeft_0x451_1105` / `SpellIndexRight_0x453_1107`** = the spell index currently bound to the left / right fire button. `SubSpellIndexLeft_1109 / SubSpellIndexRight_1110` mirror the chosen tier.
- **`spellIndex_0x458_1112`** = the "quick-select" spell for the 0x40 button (indexed through `spellIndex_D94FF[]`).

Tier selection input — **PlayerAction 0x1F / 0x20 "Change Spell"** (EF:37898):
```c
spellIndex = spellIndex_D94FF[ playerInputs.byte1 ];                                   // EF:37904
str_611.array_0x437_1079x.SpellIndex[spellIndex] = playerInputs.byte2;                 // chosen tier  EF:37905
// bind to left(0x1F) / right(0x20) quick-slot:
str_611.SpellIndexLeft_0x451_1105  = spellIndex;   SubSpellIndexLeft_1109  = byte2;    // EF:37910-37911
str_611.SpellIndexRight_0x453_1107 = spellIndex;   SubSpellIndexRight_1110 = byte2;    // EF:37916-37917
SetSpell_6D5E0(Entities[…SpellEnabled[spellIndex]], byte2);                            // EF:37921  apply tier
CopyAxisForSpellWithLife_6D830(…, byte2);                                              // EF:37922
```
On the local player it also shows the hint text (§4) and plays sound **14** (EF:37902). So the **TIER of `SPELLS_BEGIN_BUFFER_str[model].subspell[tier]` = `str_611.array_0x437_1079x.SpellIndex[model]`**, applied into the spell entity's own **`byte_0x46_70`** by `SetSpell_6D5E0` (class-15 trace §2: idle branch writes `byte_0x46_70 = clamp(spellId, byte_0-1)`). The effect state later reads `subspell[a1x->byte_0x46_70]` — i.e. **the entity's `byte_0x46_70` holds the live tier**.

`GetSpellManaCost_6D710(caster, spellIndex, tier)` (L:1714): base = `subspell[tier].manaCost_6`; castle (spellIndex==2) rescales by castle upgrade `dword_0x10_16` → 1000/10000/20000/40000/80000/160000/320000/3e8 (L:1729-1755), plus `+3000` if `byte_0x1BE_446` and no castle (L:1723-1726). `SetSpell` copies this into the entity's `maxMana_0x8C_140`, `manaRegen_0x88_136 = subspell[tier].maxManaLimit_A`, and `mana_0x90_144 = maxMana / word_0x30_48` (class-15 trace §2). `word_0x30_48 = subspell[tier].word_0x18` = the **cast duration** (# of ticks; also the divisor).

### 1.2 The CAST GATE — `sub_5F660` (EF:60874) and the fire trigger

The player tick `sub_5F380` (EF:60748, called from `AddPlayer03_00_5E010` EF:59967 and E:2913) reads the held-button bitfield `dword_0xA4_164x->entityIndex_0x0` and, for each armed button, calls the gate with the spell object bound to that slot:
```c
if (…entityIndex_0x0 & 0x10)  sub_5F660(player, Entities[…SpellEnabled[SpellIndexLeft ]], 256);  // EF:60851-60852
if (…entityIndex_0x0 & 0x20)  sub_5F660(player, Entities[…SpellEnabled[SpellIndexRight]], 512);  // EF:60854-60855
if (…entityIndex_0x0 & 0x40)  sub_5F660(player, Entities[…SpellEnabled[spellIndex_D94FF[spellIndex_0x458_1112]]], 256);  // EF:60857-60862
```
`sub_5F660(caster a1x, spellEntity a2x, buttonBit a3)` (EF:60874):
- `if (a1x->model_0x40_64 == 1) { v5=1; v3=0; }` — a **possessed/model-1 caster** forces bit 0 and marks v5 (suppresses fail-sound), EF:60888-60892.
- `switch (a2x->model_0x40_64)` — **per-spell cadence/precondition** (EF:60893-60952). Each case decides whether re-casting is allowed *right now* (mostly: skip if a cast is already active `word_0x2E_46>0`, or spell not ready). E.g.:
  - model 0 fireball: `if (byte_0x46_70 < 2) break; else LABEL_16` — tier<2 can't re-fire while airborne (EF:60895-60898).
  - model 1 posses: `if (word_0x2E_46 <= 0) break; … sub_5F7E0; v7=1` (EF:60899-60907) — already-active branch returns without re-arming.
  - models {4,6,8,0xB,0xC,0xE}: if non-wizard caster → LABEL_16; else if `word_0x2E_46<=0` break; else set `word_0x2E_46 = (model==4? 7 : 1)` and return (EF:60914-60928) — these RETRIGGER an active cast instead of re-arming.
  - models {9,0xA,0xD,0xF,0x10..0x18} LABEL_16: `if (word_0x2E_46) goto LABEL_23 (no re-arm); else break` (EF:60946-60948).
- **THE MANA GATE** (EF:60953):
  ```c
  if (a1x->mana_0x90_144 < a2x->maxMana_0x8C_140)   // caster mana < spell tier cost
      v6 = 1;                                        //   → INSUFFICIENT
  else
  { sub_5F7B0(a2x, a1x, v3); v7 = 1; }               //   → ARM THE CAST
  ```
- On insufficient (`v6 && !v5`, EF:60964): `sub_88B60()` (UI flash) + **`PrepareEventSound_6E450(0, playerColorIndex_0x38_56, 29)`** = the "not enough mana" fail sound (id **29**). `v5` (model-1 caster) suppresses it.

`sub_5F7B0(spellEntity a1x, caster a2x, bit a3)` (EF:60973):
```c
a1x->word_0x2E_46 = a1x->word_0x30_48;         // ARM: cast timer = duration  → effect state now fires
a2x->…byte[1] &= 0xFC;  a2x->…dword |= a3;     // record which button on the caster
sub_5F7E0(a1x, a2x);                            // clear caster byte[0]&0x20 unless model-1 guard (EF:60982)
```

### 1.3 Per-tick affordability + drain — `sub_68D50` / `sub_68DE0`

`sub_68D50(spellEntity locEvent1, caster locEvent2)` (EF:55548) — returns whether the cast may proceed THIS tick:
```c
if (locEvent2->mana_0x90_144 < 0) return false;                         // caster bankrupt
if (locEvent2->life_0x8 < 0)      return false;                         // caster dead
if (locEvent1->manaRegen_0x88_136) {                                    // spell has upkeep
    if (!caster.…CastleEntityIndex_0x3A_58
        || locEvent1->manaRegen > Entities[castle]->mana_0x90_144)
        return false;                                                   // no castle / castle too poor
}
if (mana >= maxMana && word_0x2E_46==word_0x30_48) return true;         // first tick, can afford full
if (word_0x2E_46==word_0x30_48) return false;                          // first tick, cannot afford
return true;                                                            // mid-cast ticks always proceed
```
`sub_68DE0(spellEntity a1x, caster a2x)` (EF:55569) — **the mana deduction**:
```c
if (a1x->word_0x2E_46 == a1x->word_0x30_48) {          // FIRST tick of cast
    if (a2x->manaRegen_0x88_136 >= 0) a2x->manaRegen = -a1x->maxMana_0x8C_140;   // stamp negative upkeep = full cost
    else                              a2x->manaRegen -= a1x->maxMana_0x8C_140;
    return 1;
} else {
    if (word_0x2E_46 && a2x->manaRegen > 0) a2x->manaRegen = 0;
    return 0;
}
```
So the tier's `maxMana` (the total mana cost) is charged **once, on the first cast tick**, by pushing it as a *negative manaRegen* on the caster; the caster's regen loop applies it. Note: `mana_0x90_144` (the entity's own field) is the per-tick amount; `maxMana_0x8C_140` is the full cost. `word_0x30_48` = duration & divisor.

### 1.4 Cast cadence / repeat-rate

- **`word_0x36_54`** on the spell entity is a per-spell cooldown counter, decremented each tick at the tail of every effect state (e.g. EF:55895, 56541, 56168). Set to 64 on pickup (class-15 trace §3).
- The precondition switch in `sub_5F660` (§1.2) is the actual repeat-rate gate: it refuses to re-arm while `word_0x2E_46>0` for most spells, so the button must be released & the cast finish (`word_0x2E_46→0`) before another cast arms.
- **Multi-shot within one cast** (charged tiers): the effect state loops `v17 = (life_0x1A != 1) + 1` (1 or 2 shots) when `subspell[tier].life_0x1A <= 2` (e.g. fireball twin-shot, lightning fork at ±113 yaw — EF:56604-56656).

### 1.5 Hand/wand aim & the a4/a5/a6 argument derivation (canonical effect-state skeleton)

Every projectile effect state (e.g. fireball `sub_693F0` EF:55832, lightning `sub_6A5C0` EF:56561) has this skeleton and derives the args identically:
```c
if (a1x->word_0x2E_46 > 0) {                         // cast in progress
  v1x = Entities[a1x->parentId_0x28_40];             // the CASTER (wizard)
  if (v1x > Entities[0] && sub_68D50(a1x, v1x)) {
    if (a1x->word_0x2E_46 == a1x->word_0x30_48) {    // FIRST tick → spawn
      proj = sub_6DCA0(
                v1x,                                  // a1 = caster (for sound + owner)
                &v1x->position_0x4C_76,               // a2 = spawn pos = caster pos
                <SPELL_CLASS_CONST>,                  // a3 = fixed per effect fn (§2 table)
                &SPELLS_BEGIN_BUFFER_str[a1x->model_0x40_64].subspell[a1x->byte_0x46_70],  // a4 = TIER row
                v1x->actSpeed_0x82_130,               // a5 = caster's current speed (speedBoost)
                1);                                   // a6 = playSound = 1
      if (proj) {
        sub_68E50(v1x, proj, a1x);                    // register/aim helper
        proj->word_0x26_38 = a1x - base;             // back-ref to spell entity
        proj->id_0x1A_26 = v1x->id_0x1A_26;          // owner id
        proj->subSpellIndex_0x2A_42 = subspell[tier].subSpellIndex_2;  // damage payload
        proj->mana_0x90_144 = a1x->mana_0x90_144;
        proj->position_0x4C_76.z += v1x->array_0x52_82.fov;            // muzzle height
        proj->yaw   = …nextEntity_0x18_24 + v1x->yaw;                 // hand yaw offset + facing
        proj->pitch = …entityIndex2_0x1A_26 + v1x->pitch;            // hand pitch offset + facing
        proj->dword_0x10_16 = …byte_0x154_340;  …byte_0x154_340 = 0; // charge counter, consumed
        proj->axis_0x9A_154x = v1x->pos;  MoveEntity_57FA0(&axis, yaw, pitch, 0x4000);  // aim target point
        if (…playerColorIndex == D41A0_0.LevelIndex_0xc) SetEntityIndex_49C90(proj, 42); // local-player HUD sprite
      }
      sub_68DE0(a1x, v1x);                            // COMMIT mana
    }
  } else a1x->word_0x2E_46 = 1;                       // can't afford → collapse cast to 1 tick
  a1x->word_0x2E_46--;                                // count down
  if (!word_0x2E_46) sub_6D880(a1x);                  // apply pending tier change (word_0x2C_44)
}
if (a1x->word_0x36_54) a1x->word_0x36_54--;           // cooldown tick
```
The **wand/hand animation** is the caster's own animation (`nextEntity_0x18_24` / `entityIndex2_0x1A_26` are the caster's per-frame hand yaw/pitch offsets sampled from `dword_0xA4_164x`), applied to the projectile's launch angle; no separate anim call in the effect state. The local-player muzzle-flash sprite is `SetEntityIndex_49C90(proj, 42)`.

**`a5` (speedBoost)** is always the caster's `actSpeed_0x82_130` from the class-15 effect states (so a faster-flying wizard throws faster projectiles); `sub_6DCA0` adds it to the base `actSpeed` and clamps to [384, 0x2000] (EF:44226-44231). **`a6` (playSound)** is `1` from all class-15 effect states, and `a1x->byte_0x46_70 == 7` from the class-9 creature caster (EF:30284) — i.e. creatures only play the cast sound for one specific state.

---

## 2. `sub_6DCA0` COMPLETE DISPATCH (EF:44020–44236)

Signature: `type_entity* sub_6DCA0(caster a1x, axis_3d* pos a2x, uint16 a3, subspell* a4x, int16 a5, char a6)`.

`a3` is **NOT the model index** — it is a spell-CLASS selector 0..25 with its own numbering. Locals: `v6 = 15` (default cast-sound id), `v7x = spawned projectile = 0`. Every arm calls `IfSubtypeCallCreatingManaSphere_4A190(pos, 9, <subtype>)` (E:5186 — spawns a class-9 flight projectile via the str91 creator table; see class-9 trace §"SUBTYPE CREATORS"), then post-writes impact fields. **The impact spell is stored as `byte_0x43_67` (impact class) + `byte_0x44_68` (impact subtype)**, later consumed by the projectile's impact via `_4A190(pos, byte_0x43_67, byte_0x44_68)`.

`byte_0x46_70 = a4x->life_0x1A` is written on several arms via `LABEL_59` (the charge/level byte carried to the projectile). `a4x->life_0x1A` is the tier's charge level (0/1/2/3); `a4x->subSpellIndex_2` is the tier's damage payload.

Full per-`a3` table (verbatim):

| a3 | dec | spawn subtype (class 9) | byte_0x43_67 / byte_0x44_68 (impact) | subSpellIndex src | byte_0x46_70 | v6 sound | charge special | EF |
|----|-----|------|------|------|------|------|------|-----|
| 0 | 0 | **28** if `a4.life_0x1A>=2` else **0** | 10 / (76 if charged else 0) | — | — | **9** | charged fireball = subtype 28, impact (10,76) | 44080-44093 |
| 7 | 7 | **12** if `a4.life_0x1A∈{1,2}` else **9** | (9/9) if charged else (10/23) | `a4.subSpellIndex_2` | — | 9 (charged) / 23 | life>2 → LABEL_60 no-spawn | 44050-44078 |
| 8 | 8 | **3** | 10 / 17 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | — | 44097-44106 |
| 9 | 9 | **3** | 10 / 17 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | (same arm as a3=8, `a3<=9`) | 44097-44106 |
| 0xD | 13 | **8** | 10 / 25 | — | — | 15 | (`a3<0xD`→no-op; `a3==0xD`) | 44108-44118 |
| 0xF | 15 | **23** | 10 / 71 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | — | 44120-44131 |
| 0x10 | 16 | **5** | 10 / 11 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | (`a3<=0x10`) | 44136-44146 |
| 0x11 | 17 | **2** | 10 / 15 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | (`a3<=0x11`) | 44150-44160 |
| 0x12 | 18 | **4** | 10 / 9 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | (`a3==18`) | 44162-44172 |
| 0x14 | 20 | **22** | 10 / 67 | `a4.subSpellIndex_2` | `a4.life_0x1A` | 15 | (`a3<0x14` no-op; `<=0x14`) | 44175-44185 |
| 0x15 | 21 | **26** | 10 / 22 | `a4.subSpellIndex_2 / life_0x1A` (÷ if charged) else raw | `a4.life_0x1A` | 15 | subSpell = payload / charge | 44189-44202 |
| 0x19 | 25 | **30** | 10 / 89 | `a4.subSpellIndex_2 / life_0x1A` (÷ if charged) else raw | `a4.life_0x1A` | 15 | subSpell = payload / charge | 44204-44219 |

**No-op a3 values** (fall through `LABEL_60` with `v7x==0`, spawn nothing): 1,2,3,4,5,6,0xA,0xB,0xC,0xE,0x13,0x16,0x17,0x18,0x1A..0x19 gaps, and any a3≥0x1A. The internal range branches: `a3<0x10` block handles {0,7,8,9,0xD,0xF}; `a3<=0x10`→0x10; `a3<0x14 & <=0x11`→0x11; `==0x12`; `<=0x14`→0x14; `<=0x15`→0x15; `==25`→0x19. Everything else spawns nothing (used by direct-effect spells and by the AI passing spell classes that this table ignores). **`a3=0x1A..0x18` and non-listed values are legitimately no-op** — those spells produce their effect without a class-9 projectile.

Tail (all arms, EF:44223-44234):
```c
if (v7x) {
    v24 = a5 + v7x->actSpeed_0x82_130;          // add speedBoost
    v7x->actSpeed = clamp(v24, 384, 0x2000);    // EF:44228-44231
    if (a6) PrepareEventSound_6E450(a1x - base, -1, v6);   // CAST SOUND
}
return v7x;
```

**RNG draws in `sub_6DCA0`: ZERO.** (The div in the 0x15/0x19 arms is `subSpellIndex_2 / life_0x1A`, deterministic.)

### 2.1 Mapping model → a3 (which player spell reaches which arm)

Resolved from the class-15 EFFECT states (each `strF0[3·model]`) that call `sub_6DCA0`:

| model | spell (class-15 trace §5) | effect fn | a3 passed | → spawns |
|-------|------|------|-----|------|
| 0 | fireball | `sub_693F0` (EF:55832) | **0** (hardcoded EF:55853) | fireball / charged-28 |
| 7 | lightning | `sub_6A5C0` (EF:56561) | **7** (EF:56610,56661) | thunder subtype 12/9 |
| 9 | meteor | `sub_6AB00` (EF:56784) | **9** (EF:56804) | subtype 3, impact (10,17) |
| 15 | tremor | `sub_6B870` (EF:57348) | **0xF** (EF:57368) | arrow subtype 23, impact (10,71) |
| 16 | crater | `sub_6BAB0` (EF:57421) | **0x10** (EF:57441) | subtype 5, impact (10,11) |
| 17 | earthquake | `sub_6BCF0` (EF:57494) | **0x11** (EF:57513) | subtype 2, impact (10,15) |
| 18 | volcano | `sub_6BF30` (EF:57566) | **0x12** (EF:57585) | subtype 4, impact (10,9) |
| 20 | gravity_well | `sub_6C3E0` (EF:57717) | **0x14** (EF:57736) | subtype 22, impact (10,67) |
| 21 | whirlwind | `sub_6C620` (EF:57789) | **0x15** (EF:57810) | subtype 26, impact (10,22) |
| 25 | cave_in | `sub_6CFA0` (EF:58123) | **0x19** (EF:58144) | subtype 30, impact (10,89) |

`GetSpellIndex_6E020` (EF:44240) is the inverse: impact subtype → spell index (9→18, 11→16, 15→17, 17→9, 22→21, 67→20, 71→15) — confirms the model↔a3↔impact-subtype triples above.

### 2.2 Direct-effect spells (do NOT call `sub_6DCA0`)

These effect states write caster state or spawn a non-class-9 entity directly. Verbatim behaviors:

| model | spell | effect fn | mechanism | sound |
|-------|------|------|------|------|
| 1 | posses | `sub_69640` (EF:55915) | spawns class-9 subtype **17** via `_4A190(pos,9,17)` (possession projectile) — **not** through sub_6DCA0; guarded on `life_0x1A<=3` | 40 |
| 2 | **castle** | `sub_69AB0` (EF:56086) | writes `dword_0xA4_164x->array_0x24E_590[v4]` (castle piece + level) then spawns class-9 **subtype 10** `_4A190(pos,9,10)` (CastCastleProjectile state 0x0A); impact **(3,2)** build if no castle yet, else **(10,43)** attack; `word_0x30_48-1` re-tick | **15** |
| 3 | speed_up | `GetScroll_69DB0` (EF:56189) | writes `dword_0xA4_164x->speed_0xc_12 = ±minSpeed·(subSpellIndex+1)`, `actSpeed = speed`; sets caster `byte[0]|=0x80` (boosting); XP idx 3; spawns visual **(10,2)** | 19 |
| 4 | metamorph | `sub_6A030` (EF:56294) | morph/transform caster state; no projectile | 60 (×2) |
| 5 | heal | `sub_6A300` (EF:56432) | direct life restore on caster | 25 |
| 6 | shield | `sub_6A480` (EF:56496) | sets caster `byte[1]|=0x40` (charged) / `byte[2]|=0x40`; cleared `dword &= 0xFFBFBFFF` when cast ends; NO spawn | — |
| 8 | rebound | `sub_6AA00` (EF:56721) | caster-state flag; no sub_6DCA0 | — |
| 10 | teleport | `sub_6AD60` (EF:56860) | relocates caster; no projectile | 22 |
| 11 | invisible | `sub_6B1C0` (EF:57068) | sets caster `byte[0]|=0x20` (invis) on first tick, clears `&=0xDF` at end; NO spawn | — |
| 12 | beyond_sight | `sub_6B310` (EF:57132) | vision/scry state on caster; no projectile | — |
| 13 | steal_mana / mana-magnet | `sub_6B3E0` (EF:57177) | drains nearby mana to caster; no sub_6DCA0 | — |
| 14 | duel | `sub_6B610` (EF:57258) | duel-marker state; no projectile | — |
| 19 | summon_army | `sub_6C170` (EF:57638) | spawns class-9 **subtype 24** `_4A190(pos,9,24)` directly (a summoned flyer, not sub_6DCA0) | 9 |
| 22 | fools_mana | `sub_6C870` (EF:57868) | spawns **(10,57)** random-sphere directly | 11 |
| 23 | magic_mine | `sub_6CAC0` (EF:57960) | spawns class-9 **subtype 29** `_4A190(pos,9,29)` (a laid mine) directly | 15 |
| 24 | alliance | `sub_6CD20` (EF:58039) | spawns class-9 **subtype 25** `_4A190(pos,9,25)` directly | 9 |

So of 26 models: **10 route through `sub_6DCA0`** (0,7,9,15,16,17,18,20,21,25); **16 are direct-effect** (1,2,3,4,5,6,8,10,11,12,13,14,19,22,23,24). Models 1,2,19,22,23,24 spawn class-9/class-10 entities **without** sub_6DCA0 (they use `_4A190` directly with the specific subtype); 3,4,5,6,8,10,11,12,13,14 are pure caster-state writes (+ optional cosmetic effect).

**The castle spell routes elsewhere** (model 2 → `sub_69AB0` → `_4A190(pos,9,10)` → CastCastleProjectile state 0x0A, plus writes into `array_0x24E_590` castle-build queue) — it does NOT touch `sub_6DCA0`. See `docs/traces/mc2-castle-*.md`.

---

## 3. CREATURE / RIVAL cast entry vs player entry

**Rival wizards use the IDENTICAL cast core.** The rival AI cast-decision dispatcher is `sub_14E10(wizard a1x, uint8 spellClass a2)` (EF:6759). Per spell class it retrieves the wizard's spell entity via `sub_146C0(a1x, a2)` and calls the **same gate**:
```c
case 0/1/7/0x16: … if (sub_5F660(a1x, v6x, 0) != 1) return 0;                       // EF:6816
case 2 (castle):  … if (…CastleEntityIndex) { sub_5F660(a1x, v2x, 0) } else _4A190(&axis,3,2)  // EF:6826/6833
case 3:           … sub_5F660(a1x, v5x, 0)                                          // EF:6844
case 4/9/0xD/0xE/0x12/0x13/0x15: … aim pitch then sub_5F660(a1x, v8x, 0)            // EF:6861
case 5/6/8/0xA/0xB/0xC/0x10/0x11/0x14: … sub_5F660(a1x, v9x, 0)                     // EF:6875
```
Argument differences vs player:
- **`a3` (button bit) = 0** for all rival casts (players pass 256/512/256). The bit only records which caster button armed; 0 = AI.
- After a successful `sub_5F660`, the AI stamps a **cooldown** into `dword_0xA4_164x->str_611.array_0x367_871x.SpellEnabled[a2] = x_WORD_D3F4C[a2]` (EF:6818, 6828, 6846, 6863, 6877) — the players' path never touches `array_0x367`. This is the only cast-execution difference; from `sub_5F660` onward (mana gate, `sub_5F7B0`, effect state, `sub_6DCA0`) the paths are byte-identical.
- The AI also pre-aims (`pitch = radix_tan(→target)`, EF:6810/6860) and enforces facing cones (`sub_582B0(yaw,roll) < 0xAA` for straight shots, `< 0xE3` for lobbed, EF:6806/6858) before arming; players aim via crosshair.

**Creatures (non-wizard mobs)** cast via `sub_6DCA0` too, but from a **class-9 creature state** (EF:29963 `case 5`), not the class-15 effect state. There, `a3 = a1x->word_0x36_54` (creature's stored spell class), `a4 = &SPELLS_BEGIN_BUFFER_str[word_0x36_54].subspell[word_0x34_52]` (creature tier), `a5 = 0`, `a6 = 1` (EF:29990). Charged twin-shot loop `v35 = (word_0x36_54==7 && subspell.life_0x1A==2) + 1` (EF:29984-29988). Some simple mobs instead call the hand-rolled creature thunks (`sub_1CC20`, `sub_1D260`, `sub_1D460`) that spawn class-9 directly (class-9 trace §"Creature-attack wiring") — those never touch `sub_6DCA0`.

Also note `sub_6DCA0(a1_6E8E, 0, 0, 0, 0, 0)` at E:3893 — a **degenerate/null cast** (a4=NULL, a6=0 no sound). With a3=0 and a4=0 it would deref `a4x->life_0x1A`; guarded because it is in a code path that is effectively a stub/never-live call (flagged OPEN).

---

## 4. Cast SOUND + UI feedback

**Cast sound** (on successful spawn, `a6=1`): `PrepareEventSound_6E450(caster - base, -1, v6)` (EF:44233), `v6` per §2 table: fireball **9**, meteor/thunder-charged **9**, thunder-uncharged **23**, all others **15** (default). Direct-effect spells play their own: posses 40, castle 15, speed_up 19, metamorph 60, heal 25, teleport 22, fools_mana 11, alliance 9. **Fail (insufficient mana)**: sound **29** at EF:60967 (+ `sub_88B60` UI flash). **Spell-select**: sound **14** at EF:37902. **Pickup**: sound 18 (class-15 trace §3).

**Hint / message text** — shown at TIER-SELECT time, not at cast-fire time. In the "Change Spell" handler (EF:37923-37928), for the local player (`i == LevelIndex_0xc`):
```c
strcpy(array_0x2BDE[i].CurrentNotificationText_0x01c_2BFA_11258,
       x_DWORD_E9C4C_langindexbuffer[ SPELLS_BEGIN_BUFFER_str[spellIndex].subspell[tier].hintText_0x16x ]);  // EF:37925
array_0x2BDE[i].word_0x04d_2C2B_11307 = 20;   // display 20 ticks
array_0x2BDE[i].word_0x04f_2C2D_11309 = 3;
```
i.e. `subspell[tier].hintText_0x16x` indexes the localized string table `x_DWORD_E9C4C_langindexbuffer`. The spellbook UI also reads all three tiers' `hintText_0x16x` (EF:49347-49349). There is **no per-cast message** at fire time — only the select-time hint.

---

## 5. Field-name quick reference (spell entity, class 15)

| field | meaning |
|-------|---------|
| `model_0x40_64` | spell index 0..25 |
| `byte_0x46_70` | live tier (0..2), set by `SetSpell` from `array_0x437.SpellIndex[model]` |
| `parentId_0x28_40` | owning wizard's entity index |
| `word_0x2E_46` | cast-in-progress timer (0 = idle; armed to `word_0x30_48`) |
| `word_0x30_48` | cast duration & mana divisor = `subspell[tier].word_0x18` |
| `word_0x2C_44` | pending tier+1 (applied by `sub_6D880` when cast ends) |
| `maxMana_0x8C_140` | full mana cost of current tier (from `GetSpellManaCost`) |
| `manaRegen_0x88_136` | upkeep = `subspell[tier].maxManaLimit_A` |
| `mana_0x90_144` | per-tick mana = `maxMana / word_0x30_48` |
| `subSpellIndex_0x2A_42` | damage payload = `subspell[tier].subSpellIndex_2` |
| `word_0x36_54` | per-spell cooldown counter |

On the spawned class-9 projectile: `byte_0x43_67`/`byte_0x44_68` = impact effect (class,subtype); `byte_0x46_70` = charge level; `subSpellIndex_0x2A_42` = damage; `word_0x26_38` = back-ref to spell entity; `id_0x1A_26` = owner; `dword_0x10_16` = charge counter from `byte_0x154_340`.

---

## 6. OPEN / uncertain

- **E:3893 `sub_6DCA0(a1_6E8E,0,0,0,0,0)`** — a null-arg call (a4=NULL). If reached with a3=0 it would deref `a4x->life_0x1A` (EF:44080) and crash. It sits in an Events.cpp path (near a `SetSpell_6D5E0(a1_6E8E,0)` at E:3835) that appears to be a stub/never-taken branch. NOT the live player/rival cast site (those are the class-15 effect states + EF:29990). Flagged: confirm this branch is dead before porting.
- **Rival tier choice**: `sub_146C0`/`sub_14E10` decide the SPELL to cast (spell class a2) and use the wizard's stored `array_0x437.SpellIndex[model]` tier via the normal `SetSpell` state — the AI *decision* logic (when/which spell) is out of scope per the task; only the cast-execution seam (`sub_5F660`, a3=0, `array_0x367` cooldown stamp) was traced.
- **`array_0x367_871x.SpellEnabled[a2]` cooldown value** `x_WORD_D3F4C[a2]` — per-spell AI recast delay table; not transcribed (values in the D3F4C data table). Not needed for the cast-execution port.
- **Charge counter `byte_0x154_340` / `dword_0x10_16`**: copied caster→projectile and zeroed each cast (EF:55869-55870, 56153-56154). Its accumulation site (how a held button builds charge into `byte_0x154_340`) was not traced here — likely in the player-input/charge path; flagged for the charge-mechanic port.
- **`word_0x18` (duration) exact units vs tick rate**: `word_0x30_48 = subspell.word_0x18` used both as cast length and mana divisor; confirmed as tick count by the `word_0x2E_46 == word_0x30_48` first-tick test, but the SPELLS.DAT values themselves (per tier) are data-driven — see `crates/mgc-sim/src/mc2/spells.rs`.
- **a3 values 8 vs 9**: both hit the same arm (subtype 3, impact (10,17)); which player spell emits a3=8 (vs a3=9=meteor) is not among the class-15 effect states surveyed — a3=8 may be a creature/AI-only spell class. Flagged.
- **`sub_68E50`** (EF:55595, the aim/registration helper called right after every sub_6DCA0 spawn) full body not transcribed — it copies position and does target-list bookkeeping; not on the critical arg-derivation path.
