I have everything needed. Let me compile the complete report.

---

# CLASS-9 Creature-Flyer Family — Verbatim Trace Report

All citations are to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, GT = `global_types.h`).

## 0. Shared infrastructure (read this first — every subtype/state references it)

### 0.1 Creator dispatch & subtype→creator map (str91, EF:1567)
The str91 table maps subtype→creator address (3rd dword of each row). Dispatch is in EV around 4436-4500 (`switch` on that address). The subtypes of interest 0x0E–0x1E resolve as:

| Subtype | str91 addr | Creator fn | EF line | action set | model set |
|--------|-----------|-----------|--------|-----------|-----------|
| 0x0E | 0x22EBC0 | `sub_4DBC0` | 35051 | 15 | 14 |
| 0x0F | 0x22ED50 | `sub_4DD50` | 35112 | 16 | 15 |
| **0x10** | 0x22EDC0 | **`return NULL`** (EV:4454) | — | — | — |
| 0x11 | 0x22EDD0 | `sub_4DDD0` | 35132 | 18 | 17 |
| **0x12** | 0x22EE80 | **`return NULL`** (EV:4462) | — | — | — |
| **0x13** | 0x22EE90 | **`return NULL`** (EV:4466) | — | — | — |
| 0x14 | 0x22EC40 | `sub_4DC40` | 35071 | 21 | 20 |
| 0x15 | 0x22ECC0 | `sub_4DCC0` | 35091 | 22 | 21 |
| 0x16 | 0x22EEA0 | `sub_4DEA0` | 35155 | 23 | 22 |
| 0x17 | 0x22EFC0 | `sub_4DFC0` | 35199 | 24 | 23 |
| 0x18 | 0x22F050 | `sub_4E050` | 35221 | 25 | 24 |
| 0x19 | 0x22F0F0 | `sub_4E0F0` | 35244 | 26 | 25 |
| 0x1A | 0x22F180 | `sub_4E180` | 35266 | 27 | 26 |
| 0x1B | 0x22EF30 | `sub_4DF30` | 35177 | 28 | 27 |
| 0x1C | 0x22E380 | `sub_4D380` | 34752 | 29 | 28 |
| 0x1D | 0x22F2A0 | `sub_4E2A0` | 35310 | 30 | 29 |
| 0x1E | 0x22F210 | `sub_4E210` | 35288 | 31 | 30 |

**OPEN:** subtypes **0x10, 0x12, 0x13** are stubbed to `return NULL` in remc2 (EV:4454/4462/4466). Their original creators (addresses 0x22EDC0/0x22EE80/0x22EE90) have no body in this decompile — the `actionIndex`/`model`/behavior-row cannot be recovered from this source. By the sequential pattern they would be actions 17/19/20 and models 16/18/19, but that is inference, not verbatim. Flag for original-binary recovery.

### 0.2 str_D7BD6 behavior-row struct (`type_str_160`, GT:75-94, length 34 bytes)
Fields consumed by the flight law:
- `subtype_160_0x2_2` (off 2) — yaw turn **target-select mode** arg for `sub_58350`
- `word_160_0x4_4` (off 4) — yaw turn **cap** arg for `sub_58350`
- `word_160_0x6_6` (off 6) — pitch turn mode arg
- `word_160_0x8_8` (off 8) — pitch turn **cap** arg
- `word_160_0xc_12` (off 12) — terrain-clearance min height (used in sub_21AB0 style movers)
- `word_160_0x1a_26` (off 26) — re-aim cadence divisor / fire-period mask (`byte_0x3E_62 % word_0x1a`)
- `word_160_0x1c_28` (off 28) — **acquisition/engage max range²** (compared against `sub_583F0_distance_3d`)

Behavior-row indices used by creators (all `&str_D7BD6[N]`):
- **60** (`0x7f8`): subtypes 0x16, 0x17, 0x18, 0x1A, 0x1B, 0x1D, 0x1E (default flyer row)
- **61** (`0x81a`): subtypes 0x11, 0x19
- (0x0E, 0x0F, 0x14, 0x15 leave `dword_0xA0_160x` **unset** by their creators — see per-subtype notes)

### 0.3 SetEntityIndexAndRot_49CD0(event, idx) (EF:32838)
Sets sprite/particle index `idx` and copies `particlesParameters_D951C[idx]`: `array_0x52_82.yaw = rotSpeed_8/2`, `.pitch = speed_6/2`, `.roll = speed_6/2`, `.fov = rotSpeed_8/2`. `sub_49E10` (EF:32865) = same then ×2 on pitch/roll/fov. `SetEntityShiftRot_49EA0(event,shift,fov)` (EF:32874): `.pitch=.roll=shift; .fov=fov`.

### 0.4 Flag op `struct_byte_0xc_12_15.byte[0] &= 0xF7` — clears bit 3 (0x08) on every flyer creator (disables a draw/collision flag). Bit 1 (0x02) is the "aim-acquired" latch set in the tick (§0.6). Bit 3 of `byte[0]` (0x08) is also the target-eligibility flag tested in the victim probe (§0.5).

### 0.5 Victim probe `sub_10780` (EF:3739) — ray-march along yaw
Marches a ray (`AddE7EE0x_10080` seeded from `array_0x52_82.pitch+255`), walks the map-entity linked list per cell, and returns the first entity matching **all** of:
- `struct_byte_0xc_12_15.byte[0] & 8` (target eligible), AND
- **target-class filter** (EF:3766-3768): `xtype_0x41_65 == -1` (any) OR (`xtype==class` AND `xsubtype_0x42_66==-1`) OR (`xtype==class` AND `xsubtype==model`), AND
- **owner immunity** (EF:3769): `a1x->id_0x1A_26 != v5x->id_0x1A_26`, AND
- `sub_106C0(a1x,v5x)` (line-of-fire/proximity confirm).

`sub_108B0` (EF:3783) is a variant used by the descend/impact state 0x12 (sub_674C0): same march but filters to class-5 model-22, class-10 models 0x27/0x28/0x2D/57 (buildings/shields), with owner immunity on both `id_0x1A_26` and `playerEntityIndex_0x94_148` (EF:3863).

### 0.6 Core flight tick `sub_65820` (EF:62882) — used by nearly every flyer action state
Full pseudocode:
1. `v1 = Entities[a1x->word_0x96_150]` (current locked target).
2. **If no target** (`v1 <= Entities[0]`): if bit1 of byte[0] not yet set → set it, then **initial acquisition**: `if (sub_68940(a1x) || sub_67CB0(a1x))` → adopt aim (`yaw=roll; pitch=fov`), else snapshot current (`roll=yaw; fov=pitch`). (EF:62902-62917)
   **Else** (have target): `sub_65610(a1x, v1)` = **homing re-aim toward the target every tick** (§0.7).
3. **Speed ramp** (EF:62923-62931): step `actSpeed_0x82_130` by `±2` toward `minSpeed_0x84_132`.
4. **Polar step**: `predictedAxis = position; MoveEntity_57FA0(&predicted, yaw, pitch, actSpeed); CopyEntityPosition`. (EF:62932-62934)
5. **Victim probe** `v4 = sub_10780(a1x)` (§0.5). If hit:
   - **Shielded-target ricochet** (EF:62939): `if (v4->struct_byte_0xc_12_15.word[0] & 0x8010 && sub_68740(a1x,v4,0x2D,22)) return 0;` — deflect off shield (§0.8).
   - else land on victim position, `v14=1` → impact.
6. **No victim → terrain test** (EF:62947-62965): if terrain alt above z (or cave ceiling below z), clamp z; then if `model != {4,22,24,26}` and tile is water (`sub_104D0_terrain_tile_is_water==1`) → spawn **(10,5)** splash (`IfSubtypeCallCreatingManaSphere_4A190(pos,10,5)`, id inherited), `DisableEntityDrawing04_57F10`, done.
7. **Life countdown** (EF:62966-62970): `if (--life_0x8 < 0) v14=1` (expiry → impact).
8. **Impact/expiry block** (v14==1, EF:62972-62996):
   - `sub_68AC0(a1x, victim)` (§0.9 — detonate-on-owner-shield): if true, disable & return 0.
   - Spawn the **impact effect**: `v11 = IfSubtypeCallCreatingManaSphere_4A190(pos, a1x->byte_0x43_67, a1x->byte_0x44_68)` — the impact class/model come from the flyer's `byte_0x43_67`/`byte_0x44_68` (armed by launcher). If null → return 0.
   - `sub_65780` / `sub_686D0` bookkeeping (danger scoring on class-3 owner).
   - `if (word_0x26_38) sub_6D8B0(...)` (weapon-hit stat).
   - `if (byte_0x44_68 == 34) v11->life_0x8 = subSpellIndex_0x2A_42` (damage passed via life for that impact model).
   - Copy `id_0x1A_26, yaw, pitch, word_0x96_150(=victim), subSpellIndex_0x2A_42, byte_0x46_70` onto the impact entity, then `DisableEntityDrawing04_57F10(a1x)`.

**Damage channel:** these flyers carry damage in `subSpellIndex_0x2A_42` (set by launcher, e.g. 780/4000/spell subSpellIndex_2) and pass it to the spawned impact effect (class from `byte_0x43_67`, model `byte_0x44_68`). The impact entity is what actually applies damage.

### 0.7 Homing re-aim `sub_65610` (EF:62781) — the turn law
Each tick with a live target: `roll_0x20_32 = tan2(self,target)` (desired yaw), `fov_0x22_34 = radix_tan(...)` (desired pitch), then:
- `yaw += sub_58350(yaw, roll, row->word_160_0x4_4, row->subtype_160_0x2_2)`
- `pitch += sub_58350(pitch, fov, row->word_160_0x8_8, row->word_160_0x6_6)`
- both `HIBYTE(...) &= 7` (angle wrap).

`sub_58350(cur, desired, a3_ignored, cap)` (EF:40391): returns `sign(delta)*min(|delta|, cap)` where delta via `sub_582B0`(magnitude)/`sub_582F0`(sign). **So turn cap per tick = `word_160_0x4_4` (yaw) / `word_160_0x8_8` (pitch)** — read from the behavior row. `sub_656D0` (EF:62809) is the identical law without the `sub_65580/A0` z-bob wrapper.

### 0.8 Shielded-target ricochet `sub_68740` (EF:55220)
Gate: only if `a2x` (victim) has enough mana (`(mana*3/4) > victim.mana` → return 0), and impact model in a set (models {byte_0x44_68 in 1..9,0xB,0xF,0x11,0x16,0x43,0x47,89} when byte_0x43==10, or self model==13). On success: plays sound 28 (`PrepareEventSound_6E450(victim,-1,28)`), `sub_6D8B0(victim,8,1)`, drains victim mana by `mana*3/4`, **reflects heading** (`roll = yaw+180°` via `HIBYTE+4&7`; pitch inverted), then if victim byte[0]&0x10 doubles subSpellIndex, else applies a **random yaw jitter**: `rand_0x14_20 = 9377*rand + 9439; yaw = roll + rand % a3(=0x2D) - a4(=22)`. Then re-owns: `word_0x96_150 = id_0x1A_26; id_0x1A_26 = victim.id; life = maxLife`, sets `xtype/xsubtype` from old owner, repositions to victim + victim.fov.

### 0.9 Detonate-on-owner-shield `sub_68AC0` (EF:55396) — OWNER-immunity/anti-shield
Only for models in flyer set {0,1,2..5,7..9,0xB,0xC,0x16,0x17,0x1A,0x1C,30}. If `victim.class==10 && victim.model==78 (shield) && victim.word_0x32_50==self.id && victim.word_0x36_54==-1`: spawn **(10,0)**, sound 26, and if `word_0x26_38` set, poke the caster entity (`word_0x36_54=model, word_0x34_52=byte_0x46_70, word_0x2E_46=1`). Returns 1 → flyer disabled.

### 0.10 Homing-toward-shield `sub_68940` (EF:55314) — pulls flyer to friendly shield model-78
Same model set. Scans `x_D41A0_BYTEARRAY_4_struct.dword_38535` for `model==78 && word_0x32_50==self.id && word_0x36_54==-1` within `owner.row->word_160_0x1c_28` range and within ~0xAA yaw cone; nearest becomes `word_0x96_150` and `sub_655C0` aims at it.

### 0.11 Initial acquisition `sub_67CB0` (EF:54710) — **keyed on `model_0x40_64`**
Returns 1 and sets `word_0x96_150` when a target is found; scoring via `sub_68490` (EF:55100, yaw/pitch cone `a3/a4`, range ≤5120, returns squared miss-distance, -1 if outside cone) or `sub_685D0` for model-2 victims. Cases (model → behavior):
- **models {0,3,4,0x12,0x13,0x16,0x1A,0x1C,0x1E}** (EF:54771): full sweep — dynamic list `dword_38519` within `owner.row->word_160_0x1c_28`, then all 29 class buckets (skip bucket 22) requiring `byte_0x39_57` and not-own-parent, then a fallback building sweep (bucket 88/4). On lock: `sub_68BD0`, `sub_655C0`, and **if target is class-3 player-avatar (`class==3 && model==0`) → `sub_5EF70(target)` danger-timer poke**.
- **models {1,0x11}** (EF:54853): scan `dword_38523`, cone 0x71/0x71, plus buildings/shield sweep.
- **models {7,8,0xB,0xC}** (EF:54859): only `dword_38519` within range; poke `sub_5EF70` if class-3 avatar.
- **model 9** (EF:54889): range = `minSpeed*maxLife`, dynamic + all 29 buckets (cone widened to 0x200 pitch for bucket targets).
- **model 0x10** (EF:54934): cone 0x100/0x71; poke `sub_5EF70` if avatar.
- **model 0x19** (EF:54983): all buckets, requires `sub_3A7F0(target)`.
- **default: return 0** (no acquisition).

**OWNER immunity everywhere:** the `id24-equivalent` is **`id_0x1A_26`** — every acquisition/probe excludes `target.id_0x1A_26 == self.id_0x1A_26` (and for buildings also `playerEntityIndex_0x94_148`/`parentId_0x28_40`). Launchers set `flyer->id_0x1A_26 = caster->id_0x1A_26` so the flyer never targets/hits its own caster.

### 0.12 `IfSubtypeCallCreatingManaSphere_4A190(axis*, class, subtype)` — the universal spawner (returns the created entity or NULL). Used both to create flyers (class 9) and their impact effects (class 10).

---

## PART 1 — Creators (subtypes 0x0E–0x1E)

Every creator: `NewEvent_4A050()`; if non-null set fields (order below); then `struct_byte_0xc &= 0xF7`; `AddEventToMap_57D70(event,position)`; `CopyMaxLifeToLife_49A20`; sprite via `SetEntityIndexAndRot_49CD0`. **No RNG draws occur in any creator** (RNG only in the ricochet path §0.8). Launch yaw/pitch are **not** set by the creator — they are set by the launcher after creation (Part 3).

### Subtype 0x0E — `sub_4DBC0` (EF:35051)
`actionIndex=15; class=9; model=14; actSpeed=128; minSpeed=128; maxLife=4096/128 (=32); &0xF7; AddToMap; CopyLife; SetEntityIndexAndRot_49CD0(196)`. No `dword_0xA0_160x` set, no mana, no ShiftRot. Sprite row 196. **RNG draws: 0.**

### Subtype 0x0F — `sub_4DD50` (EF:35112)
`maxLife=80; actionIndex=16; class=9; model=15; actSpeed=128; minSpeed=128; &0xF7; ...; SetEntityIndexAndRot_49CD0(215)`. No behavior row, no mana. Sprite 215. **RNG: 0.**

### Subtype 0x10 — **OPEN** (EV:4454 `return NULL`). Not recoverable from this source.

### Subtype 0x11 — `sub_4DDD0` (EF:35132)
`actionIndex=18; class=9; model=17; actSpeed=384; minSpeed=384; mana=50; maxLife=4096/384 (=10); dword_0xA0_160x=&str_D7BD6[61]; &0xF7; AddToMap; CopyLife; SetEntityIndexAndRot_49CD0(209); SetEntityShiftRot_49EA0(2*array.pitch, 2*array.fov)`. Sprite 209, behavior **row 61**. **RNG: 0.**

### Subtype 0x12 — **OPEN** (EV:4462 `return NULL`).
### Subtype 0x13 — **OPEN** (EV:4466 `return NULL`).

### Subtype 0x14 — `sub_4DC40` (EF:35071)
`actionIndex=21; class=9; model=20; actSpeed=394; minSpeed=actSpeed(394); maxLife=7680/394 (=19); &0xF7; ...; SetEntityIndexAndRot_49CD0(196)`. No behavior row, no mana. Sprite 196. **RNG: 0.**

### Subtype 0x15 — `sub_4DCC0` (EF:35091)
`actionIndex=22; class=9; model=21; actSpeed=394; minSpeed=394; maxLife=7680/394 (=19); &0xF7; ...; SetEntityIndexAndRot_49CD0(319); SetEntityShiftRot_49EA0(256, 512)`. No behavior row, no mana. Sprite 319. **RNG: 0.**

### Subtype 0x16 — `sub_4DEA0` (EF:35155)
`actionIndex=23; class=9; model=22; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384 (=21); dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(211)`. Sprite 211, **row 60**. **RNG: 0.**

### Subtype 0x17 — `sub_4DFC0` (EF:35199)
`actionIndex=24; class=9; model=23; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(211)`. Sprite 211, **row 60**. **RNG: 0.**

### Subtype 0x18 — `sub_4E050` (EF:35221)
`actionIndex=25; class=9; model=24; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; &0xF7; dword_0xA0_160x=&str_D7BD6[60]; maxLife &= 0xFC (clear low 2 bits → 20); ...; SetEntityIndexAndRot_49CD0(281)`. Sprite 281, **row 60**. Note the extra `maxLife &= 0xFC` at EF:35235. **RNG: 0.**

### Subtype 0x19 — `sub_4E0F0` (EF:35244)
`actionIndex=26; class=9; model=25; actSpeed=384; minSpeed=384; mana=50; maxLife=4096/384 (=10); dword_0xA0_160x=&str_D7BD6[61]; &0xF7; ...; SetEntityIndexAndRot_49CD0(321)`. Sprite 321, **row 61**. **RNG: 0.**

### Subtype 0x1A — `sub_4E180` (EF:35266)
`actionIndex=27; class=9; model=26; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(320)`. Sprite 320, **row 60**. **RNG: 0.**

### Subtype 0x1B — `sub_4DF30` (EF:35177)
`actionIndex=28; class=9; model=27; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(215)`. Sprite 215, **row 60**. **RNG: 0.**

### Subtype 0x1C — `sub_4D380` (EF:34752) — wraps `SummonFireball_4D2E0`
Calls `SummonFireball_4D2E0(position)` (EF:34729) which sets: `actionIndex=0; class=9; model=0; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; dword_0xA0_160x=&str_D7BD6[64]; &0xF7; AddToMap; CopyLife; SetEntityIndexAndRot_49CD0(340); AddEvent2_847D0(event,128,1,0)`. Then `sub_4D380` **overrides**: `actionIndex=29; model=28`. Net: action 29, model 28, **row 64**, sprite 340, plus a trailing particle (`AddEvent2_847D0(...,128,1,0)`). **RNG: 0.**

### Subtype 0x1D — `sub_4E2A0` (EF:35310)
`actionIndex=30; class=9; model=29; actSpeed=384; minSpeed=384; maxLife=10 (literal, before mana); mana=50; dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(66)`. Sprite 66, **row 60**. **RNG: 0.**

### Subtype 0x1E — `sub_4E210` (EF:35288)
`actionIndex=31; class=9; model=30; actSpeed=384; minSpeed=384; mana=50; maxLife=0x2000/384; dword_0xA0_160x=&str_D7BD6[60]; &0xF7; ...; SetEntityIndexAndRot_49CD0(211)`. Sprite 211, **row 60**. **RNG: 0.**

---

## PART 2 — Action states 0x0E–0x1C (`sub_67410`..`sub_67940`)

str90 (EF:1532) maps action→address: 0x0E→0x248410, 0x0F→0x248430, 0x10→0x248450, 0x11→0x248470, 0x12→0x2484C0, 0x13→0x2486F0, 0x14→0x248740, 0x15→0x248760, 0x16→0x248780, 0x17→0x2487A0, 0x18→0x2487D0, 0x19→0x248800, 0x1A→0x248890, 0x1B→0x2488E0, 0x1C→0x248940. EV dispatch at 3332-3362.

### State 0x0E — `sub_67410` (EF:58906) — pure timer
`result = life_0x8; life_0x8 = result-1; if (result < 0) DisableEntityDrawing04_57F10(a1x)`. **No flight, no probe, no spawn.** This is a static/decorative expiry entity (paired with creator subtype 0x0E, model 14, actSpeed 128). **RNG: 0. Sounds: none.**

### State 0x0F — `sub_67430` (EF:58918) — `return sub_65820(a1)`. Full flyer flight (§0.6). Turn caps/re-aim from behavior row (but subtype 0x0F creator sets **no** row → uses whatever row is later assigned by a launcher; if none, `dword_0xA0_160x` is whatever NewEvent zeroed — **OPEN**: state 0x0F flight law depends on an externally-set behavior row).

### State 0x10 — `sub_67450` (EF:58924) — `return sub_65820(a1)`. Identical to 0x0F.

### State 0x11 — `sub_67470` (EF:58930)
`sub_65820(a1x)` (full flight), then **on-tick secondary spawn**: `if (class_0x3F_63) { r = IfSubtypeCallCreatingManaSphere_4A190(pos, 10, 0); if(r){ r->byte[0]|=0x80; r->byte[2]|=1; r->id_0x1A_26 = a1x->id; } }`. So it lays a **(10,0)** trail/marker each tick (owner-tagged). Flight law per §0.6. **RNG: only via ricochet path inside sub_65820. Sounds: only via sub_65820 impact helpers.**

### State 0x12 — `sub_674C0` (EF:58952) — descend / terrain-detonate variant (uses `sub_108B0` probe)
Pseudocode:
1. Target lock via `word_0x96_150`. If none & bit1 unset → set bit1, `if (sub_67CB0(a1x))` adopt aim. Else `sub_65610` homing (§0.7).
2. Polar step `MoveEntity_57FA0(&predicted, yaw, pitch, actSpeed)`; clamp z to terrain alt if below (EF:58994-58997).
3. **Probe `sub_108B0(a1x)`** (§0.5 building/shield-oriented variant). If hit: `sub_65580`, snap to victim pos, `sub_655A0`, mark impact.
4. Else terrain/cave-ceiling clamp; else `--life_0x8 < 0` → expiry.
5. **Impact block** (EF:59031): spawn **two** effects when a victim exists:
   - primary: `(10,12)` normally, or `(10,70)` if `byte_0x43_67==10 && byte_0x44_68==69` (EF:59036-59039); inherits id/yaw/pitch.
   - secondary: `(byte_0x43_67, byte_0x44_68)` (the armed impact class/model), `sub_65780` scoring, `sub_6D8B0(id,1,1)`, inherits id/yaw/pitch/`dword_0x10_16`.
   - then `DisableEntityDrawing04_57F10`.
No RNG in this body (ricochet not invoked here — uses sub_108B0 not sub_10780). **This is the "mana-possession II"/descend-and-detonate flyer (EV:3349 comment "possess mana ii").**

### State 0x13 — `sub_676F0` (EF:59069) — immediate relay/detonate
`r = IfSubtypeCallCreatingManaSphere_4A190(pos, byte_0x43_67, byte_0x44_68); if(r){ inherit id/yaw/pitch/subSpellIndex; DisableEntityDrawing04(a1x); }`. **No flight** — spawns its armed impact effect on the first tick and dies. Returns the spawned entity. **RNG: 0. Sounds: 0.**

### State 0x14 — `sub_67740` (EF:59086) — `return sub_65820(a1)`. Full flight (§0.6).
### State 0x15 — `sub_67760` (EF:59092) — `return sub_65820(a1)`. Full flight.
### State 0x16 — `sub_67780` (EF:59098) — `return sub_65820(a1)`. Full flight.

### State 0x17 — `sub_677A0` (EF:59104)
`r = sub_65820(a1x); if(r){ v2 = a1x->byte_0x46_70; r->byte_0x46_70 = 0; r->life_0x8 = v2; }`. Full flight; on impact-spawn, transfers `byte_0x46_70` into the spawned effect's **life** (fuse length). **RNG/sounds via sub_65820.**

### State 0x18 — `sub_677D0` (EF:59120)
`r = sub_65820(a1x); if(r){ v2 = byte_0x46_70 & 0xF0; r->byte_0x46_70=0; r->maxLife=v2; r->life_0x8=v2; }`. Full flight; passes `(byte_0x46_70 & 0xF0)` as the spawned effect's max/current life.

### State 0x19 — `sub_67800` (EF:59138) — sets global detonation-radius then flight
Chooses `x_D41A0_BYTEARRAY_4_struct.byteindex_224` (blast tier) from `byte_0x46_70`: value 2 or in {0x11..0x13}→8; ==25→4; else→2 (EF:59143-59162). Then `r = sub_65820(a1x)`; if hit, **walks the spawned-effect chain** (`word_0x34_52` list) copying `byte_0x46_70`, `parentId_0x28_40`, `id_0x1A_26` onto each (EF:59167-59173). Full flight + multi-fragment impact.

### State 0x1A — `sub_67890` (EF:59181)
`r = sub_65820(a1x); if(r){ r = Entities[a1x->id_0x1A_26]; if(r class==3 && model in {0,1}) r->word_0x96_150 = 0; }`. Full flight; on impact clears the owner-avatar's lock (releases target). 

### State 0x1B — `sub_678E0` (EF:59202)
`r = sub_65820(a1x); if(r){ v2 = 8 * byte_0x46_70; r->maxLife=v2; r->life=v2; }`. Full flight; spawned effect life = `8×byte_0x46_70`.

### State 0x1C — `sub_67940` (EF:59234) — `return sub_65820(a1)`. Full flight (§0.6).

**Summary of flight law for all "full-flight" states (0x0F,0x10,0x11,0x14–0x1C):** polar step by `actSpeed` along (`yaw`,`pitch`); re-aim every tick toward locked target with per-axis turn caps `word_160_0x4_4`/`word_160_0x8_8` and re-aim modes `subtype_160_0x2_2`/`word_160_0x6_6` from the behavior row; acquire via `sub_67CB0` keyed on model; expire on `life<0`, terrain/cave ceiling, or water (spawns (10,5)); impact spawns `(byte_0x43_67,byte_0x44_68)` carrying `subSpellIndex` damage. Ricochet off shielded targets via `sub_68740` (RNG jitter, sound 28). Owner immunity via `id_0x1A_26`.

---

## PART 3 — WHO LAUNCHES WHAT (complete caller map)

### 3.1 Thunk `sub_1D0E0` → **(9,20)** subtype 0x14 (EF:9814)
Arming: `byte_0x43_67=10; byte_0x44_68=65; id_0x1A_26=caster.id; yaw=tan2(caster,target); pitch=radix_tan(caster,target); position.z += caster.array.fov; word_0x96_150=caster.word_0x96_150; dword_0xA0_160x=&str_D7BD6[65]; xsubtype_0x42_66=target.model; xtype_0x41_65=target.class; subSpellIndex_0x2A_42=780 (damage); sub_5EF70(target)` (danger poke).
**Launch geometry:** yaw/pitch = direct line to target; z-offset = `+caster.fov`.
**Caller:** `sub_24930` (class-5 monster attack tick) at **EF:15708** — gated: `byte_0x46_70==0` branch, fires only when `byte_0x3E_62 % row->word_160_0x1a_26 == 0`, and distance in `[0x700, row->word_160_0x1c_28)` (EF:15698-15713). Also the generic EV:1093 dispatch (`0x1fe0e0`) with null target.

### 3.2 Thunk `sub_1D1A0` → **(9,21)** subtype 0x15 (EF:9847)
Arming: `byte_0x43_67=10; byte_0x44_68=66; id=caster.id; yaw=tan2; pitch=radix_tan; position.z += 128 (fixed); word_0x96_150 inherited; dword_0xA0_160x=&str_D7BD6[65]; xsubtype=target.model; xtype=target.class; subSpellIndex=780; sub_5EF70(target)`.
**Caller:** `sub_25E40` (class-5 attack tick) at **EF:16673**, via wrapper `sub_1C310(a1x, 160, sub_1D1A0)` (EF:9240 — the standard "line-up-and-fire" wrapper that checks life/target validity, advances actionIndex to a2+1 on target loss). Plays **sound 32** (EF:16667). Also EV:1098.

### 3.3 Thunk `sub_1D260` → **(9,9)** subtype 0x09 (EF:9883)
Note: this launches **subtype 9 (model 9, the "AddEvent09_..." style, row 64)** — not one of 0x0E–0x1E, but it is one of the three thunks you named. Arming: `predicted = caster.position; predicted.z += caster.array.fov; spawn (9,9) at predicted; byte_0x43_67=10; byte_0x44_68=23; id=caster.id; yaw=tan2(predicted,target); pitch=radix_tan(predicted,target); word_0x96_150 inherited; dword_0xA0_160x=&str_D7BD6[64]; xsubtype=target.model; xtype=target.class; subSpellIndex=4000 (damage); sub_5EF70(target)`.
**Caller:** `sub_27E00` (class-5 attack tick) at **EF:18341** — gated `!(row->word_160_0x1a_26 & byte_0x3E_62)` and distance `< row->word_160_0x1c_28` (EF:18336-18343). Plays **sound 59** on entry (EF:18310). Also EV:1102.

### 3.4 Player spell-fire dispatcher `sub_6DCA0` (EF:44020) — a3 = spell index selects subtype
This is the primary **player** launch path. `f44/damage arming` = `byte_0x43_67`/`byte_0x44_68` (impact target) + `subSpellIndex_0x2A_42 = a4x->subSpellIndex_2` + `byte_0x46_70 = a4x->life_0x1A`. Trailing (EF:44224): `actSpeed += a5`, clamped `[384, 0x2000]`; if `a6` play sound `v6`. Map by spell index a3:
- a3==7 & life_0x1A in {1,2}: **(9,12)** subtype 0x0C, byte_0x43/44 = 9/9, sound v6=9.
- a3==7 & life_0x1A≥2 (top block): **(9,28)** subtype **0x1C**, byte_0x44=76; else **(9,0)**, byte_0x44=0.
- a3≤9: **(9,3)**, byte 10/17.
- a3==0xD: **(9,8)**, byte 10/25.
- a3==15: **(9,23)** subtype **0x17**, byte 10/71, subSpellIndex set, byte_0x46_70=life.
- a3≤0x10: **(9,5)**, byte 10/11.
- a3≤0x11: **(9,2)**, byte 10/15.
- a3==18: **(9,4)**, byte 10/9.
- a3==0x14: **(9,22)** subtype **0x16**, byte 10/67, subSpellIndex+life.
- a3≤0x15: **(9,26)** subtype **0x1A**, byte 10/22, subSpellIndex = `subSpellIndex_2 / life_0x1A`.
- a3==25: **(9,30)** subtype **0x1E**, byte 10/89, subSpellIndex = `subSpellIndex_2 / life_0x1A`.

### 3.5 Other launch sites (grep of `4A190(..,9,N)`, N in 14..30)
- **(9,26)** subtype 0x1A — EF:13458 in `sub_21AB0` (a large spell/effect mover, case 8): `byte_0x43/44=10/22; subSpellIndex=20; byte_0x46_70=3`; sound v34=15.
- **(9,17)** subtype 0x11 — EF:55950 in `sub_69640` (spell **possess** tick, "spell posses"): spawned when possessed target's `SPELLS_BEGIN_BUFFER_str[...].subspell[...].life_0x1A` in {1..3}; `actSpeed += caster.actSpeed`; then `sub_68E50` sets ownership.
- **(9,24)** subtype 0x18 — EF:57659 in `sub_6C170` (spell tick): `byte_0x43/44=10/72; id=caster.id; parentId inherited; byte_0x46_70=subspell.life_0x1A; dword_0x10_16` from `caster.dword_0xA4_164x->byte_0x154_340`; launch yaw/pitch = `caster.dwordA4->nextEntity_0x18_24 + caster.yaw` / `...entityIndex2_0x1A_26 + caster.pitch`; `MoveEntity_57FA0(...,0x4000)` sets `axis_0x9A_154x`; **sound 9**.
- **(9,29)** subtype 0x1D — EF:57981 in `sub_6CAC0`: `byte_0x43/44=10/78; id=caster.id; position.z += caster.fov; mana = caster.mana; subSpellIndex = caster.byte_0x46_70; dword_0x10_16` from `byte_0x154_340`; yaw = `dwordA4->nextEntity + caster.yaw`, pitch = `entityIndex2 + caster.pitch`; `axis_0x9A_154x.z = terrainAlt`; **sound 15**.
- **(9,25)** subtype 0x19 — EF:58062 in `sub_6CD20`: `byte_0x43/44=10/74; id=caster.id; parentId inherited; subSpellIndex = subspell.subSpellIndex_2; byte_0x46_70 = subspell.life_0x1A; dword_0x10_16` from `byte_0x154_340`; `axis_0x9A_154x` via MoveEntity (sound/rest continues past read window).
- **(9,28)** subtype 0x1C, **(9,23)** subtype 0x17, **(9,22)** subtype 0x16, **(9,30)** subtype 0x1E — all in `sub_6DCA0` (§3.4).

**Caller map summary (subtype → launcher(s)):**
- 0x14 ← `sub_1D0E0` (monster, sub_24930)
- 0x15 ← `sub_1D1A0` (monster, sub_25E40 via sub_1C310)
- 0x16 ← `sub_6DCA0` a3==0x14 (player spell)
- 0x17 ← `sub_6DCA0` a3==15 (player spell)
- 0x18 ← `sub_6C170` (spell tick)
- 0x19 ← `sub_6CD20` (spell tick)
- 0x1A ← `sub_6DCA0` a3≤0x15 (player spell) **and** `sub_21AB0` case 8
- 0x1B ← **no launch site found** with N=27 in the grep — **OPEN**: subtype 0x1B (model 27, `sub_4DF30`) has a creator but I found no `4A190(...,9,27)` launch call. Likely launched only via a creator-address dispatch not matching the grep pattern, or unused. Flag.
- 0x1C ← `sub_6DCA0` a3==7 top branch (player spell)
- 0x1D ← `sub_6CAC0` (spell tick)
- 0x1E ← `sub_6DCA0` a3==25 (player spell)
- 0x0E, 0x0F, 0x11 — 0x11 ← `sub_69640` possess; **0x0E/0x0F: no `4A190(...,9,14/15)` site found** — **OPEN** (0x0E is likely a static effect given its timer-only action state; may be spawned via creator-address dispatch only).

---

## PART 4 — Owner immunity & shielded-target ricochet (consolidated)

- **Owner-immunity field (id24-equivalent): `id_0x1A_26`.** Set on every flyer at launch to the caster's `id_0x1A_26`. Enforced in: `sub_10780` (EF:3769), `sub_108B0` (EF:3862-3863, also `playerEntityIndex_0x94_148`), `sub_67CB0` acquisition (EF:4785,4810,4868,4894,4916,4963,4990 — every bucket excludes `target.id==self.id`, plus `parentId`/`StageVar2` self-checks). The impact-spawn copies `id_0x1A_26` forward so daughter effects share immunity.
- **Shielded-target ricochet `sub_68740`** (§0.8) is invoked **once**, from `sub_65820` (EF:62939) when the probed victim's `struct_byte_0xc.word[0] & 0x8010` (shielded/invuln flags), with args `(a3=0x2D, a4=22)` feeding the RNG jitter `yaw = roll + rand%0x2D - 22`. On success returns 0 and the flyer survives, re-owned to the shield's holder (`id_0x1A_26 = victim.id`). This is the **only RNG draw** in the flyer runtime (`rand_0x14_20 = 9377*rand + 9439`, EF:55293).
- **Anti-shield detonate `sub_68AC0`** (§0.9) and **homing-to-shield `sub_68940`** (§0.10) handle friendly class-10 model-78 shields; both keyed to `id_0x1A_26 == shield.word_0x32_50`.

---

## OPEN ITEMS (flagged)
1. **Subtypes 0x10, 0x12, 0x13** — creators stubbed `return NULL` (EV:4454/4462/4466). No recoverable body. Original action/model/row unknown from this source.
2. **State 0x0F/0x10 (sub_67430/50)** flight uses a behavior row that their creators (0x0F = sub_4DD50) do **not** set — `dword_0xA0_160x` must be assigned by a launcher for the turn law to be defined. No such launcher found for 0x0F. Flag.
3. **Subtype 0x1B (model 27)** — creator exists (`sub_4DF30`) but no `4A190(...,9,27)` launch site found. Possibly unused/dead in remc2.
4. **Subtypes 0x0E, 0x0F** — no direct `4A190(...,9,14/15)` launch site found. 0x0E's action state is timer-only (`sub_67410`), consistent with a decorative/secondary-spawn entity created via creator-address dispatch rather than the (9,N) helper. Confirm against original if their runtime behavior matters.
5. `SetEntityIndexAndRot_49CD0` sprite indices (196/209/211/215/281/319/320/321/340/66) index `particlesParameters_D951C[]` — the actual model/animation data lives in that table (not read here; extract separately if needed for the port).