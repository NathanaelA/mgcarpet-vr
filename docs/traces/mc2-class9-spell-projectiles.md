I have comprehensive data. The flight action functions (CastPlayerFire, sub_65820, 662E0, etc.) themselves play NO sounds during flight — sounds are played at cast time (sub_6DCA0, id=v6) and at ricochet (id 28 in sub_68740, id 26 in sub_68AC0). The impact effect entities (class 10) play their own sounds. I now have everything to write the complete report.

---

# CLASS-9 SPELL-PROJECTILE Family — Verbatim Trace (subtypes/states 0x00–0x0C + 0x1C)

All citations are `EventsFunctions.cpp` (EF) unless prefixed. Key file paths:
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/EventsFunctions.cpp`
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/Events.cpp` (E)
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/Level.cpp` (behavior-row data `str_D7BD6`)
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/Player.cpp` (`MoveEntity_57FA0`)
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/Sound.cpp` (`sub_582B0`/`sub_582F0`, `PrepareEventSound_6E450`)
- `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/global_types.h` (`type_str_160` behavior-row struct)

## Table anchors (confirmed)
- Creation table **str91** = `x_DWORD_D4C52ar_str91` (EF:1567). Maps `subtype → creator`. Entry format `{0x002A5C44, subtype, creator_addr, 1}`. Entry 0x1C = `0x0022E380` = `sub_4D380` (fireball clone), confirmed (EF:1596).
- Action table **str90** = `x_DWORD_D4C52ar_str90` (EF:1532). Maps `actionIndex → action_fn`. State→fn: 0=`0x246B30 CastPlayerFire_65B30`, 1=`0x246F60 CastPosses_65F60`, 2=`0x247160 sub_66160`, 3=`0x247180 sub_66180`, 4=`0x247250 sub_66250`, 5=`0x247280 sub_66280`, 6=`0x2472A0 sub_662A0`, 7=`0x2472C0 sub_662C0`, 8=`0x2472E0 sub_662E0`, 9=`0x247750 sub_66750`, 0x0A=`0x247B30 CastCastleProjectile_66B30`, 0x0B=`0x247FB0 sub_66FB0`, 0x0C=`0x247FD0 sub_66FD0` (EF:1533-1545). Dispatch in E:3260-3327.

## Shared primitives

**`MoveEntity_57FA0(pos, yaw, pitch, speed)`** (Player.cpp:6): polar 3D step, 11-bit angles (`&0x7FF`). If `pitch`: `z -= (speed*sin[pitch])>>16`, then `speed = (speed*sin[0x200+pitch])>>16` (cosine). Then `x += (speed*sin[yaw])>>16`, `y -= (speed*sin[0x200+yaw])>>16`. Sine LUT `Maths::sin_DB750`, `[0x200+θ]` = cosine. No gravity — z only changes via pitch or terrain clamp.

**Angle helpers** (Sound.cpp:6569-6604): `sub_582B0(a,b)` = shortest-arc absolute angular distance = `abs((a&0x7FF)-(b&0x7FF))`, folded to ≤1024. `sub_582F0(a,b)` = turn direction sign ∈ {-1,0,+1}.

**`sub_58350(cur, tgt, a3_UNUSED, cap)`** (EF:40391): the turn-step. `if cur==tgt return 0; v4=sub_582B0(cur,tgt); v5=sub_582F0(cur,tgt); v6=(v4>cap)?cap:v4; return v5*v6;` → **per-tick turn = min(angular_dist, cap) · sign**. Note a3 is discarded.

**`type_str_160` behavior-row** (global_types.h:75-94), fields used for homing (int16, byte offsets): `subtype_160_0x2_2` [field1] = **yaw turn cap**; `word_160_0x6_6` [field3] = **pitch turn cap**; `word_160_0x1a_26` [field12] = re-aim cadence divisor; `word_160_0x1c_28` [field13] = acquisition/lock range (squared 3D distance); `byte_160_0x20_32` [field15] = flag byte (bit 0x10 → subSpellIndex=1).

**Homing helper `sub_65610(self, target)`** (EF:62781): re-aims every tick when a live target is locked (`word_0x96_150` > 0):
1. `sub_65580(target)` (raise target z by box, EF:62750).
2. `self->roll = tan2(self.pos, target.pos)` (`Maths::sub_581E0`), `self->fov = radix_tan(...)` (`Maths::sub_58210`) — desired yaw/pitch.
3. `self->yaw += sub_58350(self.yaw, self.roll, word_160_0x4_4, subtype_160_0x2_2)`; `HIBYTE(yaw)&=7` (wrap to 11-bit). Cap = **subtype_160_0x2_2 (yaw cap)**.
4. `self->pitch += sub_58350(self.pitch, self.fov, word_160_0x8_8, word_160_0x6_6)`; `HIBYTE(pitch)&=7`. Cap = **word_160_0x6_6 (pitch cap)**.
5. `sub_655A0(target)` (lower target z back, EF:62761).

So MC2's per-tick turn caps are **behavior-row fields**, not a fixed 5. Re-aim runs **every tick** when locked (no cadence gate in the flight helpers — the `word_160_0x1a_26` cadence divisor is only consulted by the creature-AI cast-decision path at EF:9653, not by the projectile flight itself; OPEN whether any flight state uses it — none of 0x00–0x0C do).

**Behavior-row numeric data** (Level.cpp; array index N = file line 12+N):

| Row idx | line | yaw cap (fld1) | pitch cap (fld3) | cadence (fld12) | lock range (fld13) | flag byte (fld15) |
|---|---|---|---|---|---|---|
| 59 | 71 | 0x38=56 | 0x16=22 | 0x32=50 | 0x1000=4096 | 0x00 |
| 60 | 72 | 0x16=22 | 0x16=22 | 0x28=40 | 0x1000=4096 | 0x00 |
| 61 | 73 | 0x71=113 | 0x71=113 | 40 | 4096 | 0x00 |
| 62 | 74 | 0x11=17 | 0x16=22 | 40 | 4096 | 0x00 |
| 63 | 75 | 0x0B=11 | 0x16=22 | 40 | 4096 | 0x00 |
| 64 | 76 | 0x05=5 | 0x16=22 | 40 | 4096 | 0x00 |
| 65 | 77 | 0x00=0 | 0x16=22 | 40 | 4096 | 0x00 |

(Row 64 line = EF-equivalent Level.cpp:76; row 65 = Level.cpp:77.) Note **str_D7BD6[64] yaw cap = 5** (matches MC1's 5/tick), **str_D7BD6[65] yaw cap = 0** (no yaw homing — used by creature straight-shot fireballs). OPEN: row 59 (Level.cpp:71) is `D41A0_0.dword_0x36DF6` default (Level.cpp:189), yaw cap 56.

**Victim probe `sub_10780(self)`** (EF:3739): ray-marches map cells along `pitch`. For each entity in cell: must have `struct_byte_0xc_12_15.byte[0] & 8` (targetable flag), pass target-class filter, `self.id != entity.id`, and `sub_106C0` (AABB overlap). **Target-class filter** (EF:3765-3768): `self.xtype_0x41_65 == -1` (any) OR (`xtype == entity.class` AND `xsubtype_0x42_66 == -1`) OR (`xtype == entity.class` AND `xsubtype == entity.model`). Returns first hit. `sub_106C0(a,b)` (EF:3720) = `abs(dx) < a.box.pitch+b.box.pitch && abs(dy) < a.box.roll+b.box.roll` (2D AABB, EF:3726).

**Auto-aim `sub_67CB0(self)`** (EF:54710): one-shot initial target acquisition (guarded by `byte[0]&2`). Big `switch(model_0x40_64)`; models {0,3,4,0x12,0x13,0x16,0x1A,0x1C,0x1E} take the homing branch. Scans entity lists, uses `dword_0xA0_160x->word_160_0x1c_28` as squared range, scores with `sub_68490`/`sub_685D0`, writes best target into `word_0x96_150` and desired `roll`/`fov`. Returns nonzero if a target was found (caller then copies roll→yaw, fov→pitch). Full body EF:54769-~54900 (large; scoring detail OPEN but not needed for flight law).

**Homing-drone acquire `sub_68940(self)`** (EF:55315): only for models in {0,1,2,3,4,5,8,9,0xC,0x16,0x17,0x1A,0x1C,0x1E}. If owner (`id_0x1A_26`) is a class-3 player, searches for a friendly "drone/guide" entity (`model==78`, `word_0x32_50==self.id`, `word_0x36_54==-1`) within `owner->dword_0xA0_160x->word_160_0x1c_28`, front cone `sub_582B0(yaw,bearing) < 0xAA` (170/2048 ≈ 30°). On hit: sets `word_0x96_150`, calls `sub_655C0` (aim), returns 1.

**Impact spawner `IfSubtypeCallCreatingManaSphere_4A190(pos, type, subtype)`** (E:5186): if creation-table `str_D4C48ar[type].dword_14[subtype]` valid, spawns that entity at pos, else 0. Impact effects use `type=10` (class-10 particle/explosion). The projectile stores impact spell as `byte_0x43_67` (type) + `byte_0x44_68` (subtype); impact calls `_4A190(pos, byte_0x43_67, byte_0x44_68)`.

**Ricochet `sub_68740(self, victim, a3, a4)`** (EF:55221): shielded-target reflect. Preconditions: victim flag `word[0] & 0x8010`, `self.mana/4 <= victim.mana` (EF:55234), and self's impact spell (`byte_0x43_67==10` with specific `byte_0x44_68` values {1,2..8 range,9,0xB,0xF,0x11,0x16,0x43,0x47,0x59} or `model==13`) qualifies. On reflect (EF:55270+): **sound id 28** `PrepareEventSound_6E450(victim,-1,28)`; `sub_6D8B0(victim, 8, 1)` (shield XP); drains `victim.mana -= self.mana/4`; flips yaw by +π (`HIBYTE(yaw)+4 &7`), negates pitch; if victim `byte[0]&0x10` doubles subSpellIndex & keeps yaw else **RNG re-aim jitter** `rand=9377*rand+9439; yaw = roll + rand%a3 - a4` (a3=0x5B/45 for CastPlayerFire, 0x2D/22 for others); swaps id↔target, resets life=maxLife, sets `xtype/xsubtype` = old-owner class/model, repositions at victim. Returns 1 (projectile continues as reflected).

**Drone-collision `sub_68AC0(self, victim)`** (EF:55397): same model gate as sub_68940. If victim is the guide drone (`class==10, model==78, word_0x32_50==self.id, word_0x36_54==-1`): spawns effect `(10,0)`, **sound id 26** `PrepareEventSound_6E450(self,-1,26)`, marks drone consumed, returns 1 → caller kills projectile without normal impact.

**XP channel `sub_6D8B0(ownerId, spellIndex, amount)`** (EF:58228): awards spell experience (NOT damage) to a class-3 player owner. Damage itself is delivered by the spawned class-10 impact entity via its own subtype and the projectile's `subSpellIndex_0x2A_42` (passed through). OPEN: exact damage formula lives in the class-10 effect handler, out of scope.

**`sub_65780(self, victim, origTarget)`** (EF:62836): on impact, if owner is class-3 player, increments owner stat `dword_0x165_357` (shots) and, if victim==origTarget, `dword_0x169_361` (hits) — gated to model set {0-1,3,7-9,0xC,0x11,0x19,0x1C}. `sub_686D0(self,victim)` (EF:55193): if owner is class-3 player model 0/1, re-locks owner's `word_0x96_150` to victim (auto-retarget).

---

## SUBTYPE CREATORS (str91)

Common to every non-arrow creator: `NewEvent_4A050()`; `class_0x3F_63 = 9`; `struct_byte_0xc_12_15.byte[0] &= 0xF7` (clears bit 0x08 = clears targetable — projectile is not itself a target); `AddEventToMap_57D70(e,pos)`; `CopyMaxLifeToLife_49A20(e)` (`life = maxLife`, EF:54118); sprite via `SetEntityIndexAndRot_49CD0(e, spriteIdx)` (EF:32838: sets `word_0x5A_90=idx`, anim reset, and **collision box** `array_0x52_82.{pitch,roll,fov} = particlesParameters_D951C[idx].{speed/2, speed/2, rotSpeed/2}`, `yaw = rotSpeed/2`). `actSpeed=minSpeed=384`, `mana_0x90_144=50`, `maxLife = <dist>/actSpeed`. **No RNG draws in any creator.**

### Subtype 0x00 — `SummonFireball_4D2E0` (EF:34729), sprite 340
Field writes in order: `actionIndex=0`; `class=9`; `model=0`; `actSpeed=384`; `minSpeed=384`; `mana=50`; `maxLife = 0x2000/384 = 21`; `dword_0xA0_160x = &str_D7BD6[64]` (yaw cap 5 / pitch cap 22); `byte[0] &= 0xF7`; AddToMap; CopyMaxLifeToLife; `SetEntityIndexAndRot(340)`; `AddEvent2_847D0(e,128,1,0)` (attaches a trailing sub-effect, model 1). RNG: **0**.

### Subtype 0x1C — `sub_4D380` (EF:34752), sprite 340 (fireball clone)
Calls `SummonFireball_4D2E0(pos)` then overrides `actionIndex=29`, `model=28`. So it flies under action state **29** (not one of 0x00–0x0C), sprite 340, behavior row str_D7BD6[64]. Charged-fireball variant (spawned by sub_6DCA0 when `life_0x1A>=2`, EF:44081). RNG: **0**.

### Subtype 0x01 — `SummonManaPosession_4D3B0` (EF:34764), sprite 209
`actionIndex=1`; `class=9`; `model=1`; `actSpeed=384`; `minSpeed=384`; `mana=50`; `dword_0xA0_160x = &str_D7BD6[61]` (yaw/pitch cap 113); `maxLife = 4096/384 = 10`; `xtype_0x41_65 = 10` (targets class 10!); `byte[0] &= 0xF7`; AddToMap; CopyMaxLifeToLife; `SetEntityIndexAndRot(209)`; **`SetEntityShiftRot_49EA0(e, 2*pitch, 5*fov/2)`** (EF:32874: overrides box pitch/roll = 2× box-pitch, fov = 5·fov/2). RNG: **0**.

### Subtype 0x02 — `sub_4D470` (EF:34788), sprite 211
`actionIndex=2`; `model=2`; speeds 384/384; mana 50; `maxLife=0x2000/384=21`; `dword_0xA0_160x=&str_D7BD6[60]` (yaw/pitch cap 22); `byte[0]&=0xF7`; AddToMap; CopyMaxLifeToLife; `SetEntityIndexAndRot(211)`. RNG **0**.

### Subtype 0x03 — `sub_4D500` (EF:34810), sprite 76
Same as 0x02 but `actionIndex=3`, `model=3`, `SetEntityIndexAndRot(76)`, row str_D7BD6[60]. `maxLife=21`. RNG **0**.

### Subtype 0x04 — `sub_4D590` (EF:34832), sprite 210
`actionIndex=4`, `model=4`, sprite 210, row str_D7BD6[60], `maxLife=21`. (model 4 is special-cased throughout flight to skip water-splash / z-suppress, see states.) RNG **0**.

### Subtype 0x05 — `sub_4D620` (EF:34854), sprite 211
`actionIndex=5`, `model=5`, sprite 211, row str_D7BD6[60], `maxLife=21`. RNG **0**.

### Subtype 0x06 — `sub_4D6B0` (EF:34876), sprite 212
`actionIndex=6`, `model=6`, sprite 212, row str_D7BD6[60], `maxLife=21`. RNG **0**.

### Subtype 0x07 — `sub_4D740` (EF:34898), sprite 213
`actionIndex=7`, `model=7`, sprite 213, row str_D7BD6[60], `maxLife=21`. RNG **0**.

### Subtype 0x08 — `sub_4D7D0` (EF:34920), sprite 214
`actionIndex=8`, `model=8`, sprite 214, **row str_D7BD6[63]** (yaw cap 11 / pitch cap 22), `maxLife=21`. RNG **0**.

### Subtype 0x09 — `sub_4D860` (EF:34942), sprite 216
`actionIndex=9`, `model=9`, speeds 384/384, mana 50, `maxLife = 3584/384 = 9`, **row str_D7BD6[63]**, `byte[0]&=0xF7`, AddToMap, CopyMaxLifeToLife, `SetEntityIndexAndRot(216)`, `AddEvent2_847D0(e,128,9,0)` (trailing effect model 9). RNG **0**.

### Subtype 0x0A — `sub_4D900` (EF:34965), sprite 18
`actionIndex=10`, `model=10`, speeds 384/384, mana 50, `maxLife=0x2000/384=21`, row str_D7BD6[60], `SetEntityIndexAndRot(18)`. RNG **0**.

### Subtype 0x0B — `sub_4D990` (EF:34987), sprite 281
`actionIndex=11`, `model=11`, speeds 384/384, mana 50, `maxLife=21`, row str_D7BD6[60], `SetEntityIndexAndRot(281)`. RNG **0**.

### Subtype 0x0C — `sub_4DA20` (EF:35009), sprite 216
`actionIndex=12`, `model=12`, speeds 384/384, mana 50, `maxLife = 2048/384 = 5`, row str_D7BD6[60], `SetEntityIndexAndRot(216)`. RNG **0**.

### (Reference) Subtype 0x0D — `AddEvent09_0D_4DAB0` (EF:35031), sprite 195 — ALREADY PORTED
`actionIndex=0xD`, `model=0xD`, speeds 384/384, `maxLife=5120/384=13`, no mana/behavior-row set here, `byte[0]&=0xF7`, AddToMap, CopyMaxLifeToLife, **`sub_49E10(e,195)`** — the DOUBLE-box variant (EF:32865: calls SetEntityIndexAndRot then `box.{pitch,roll,fov} *= 2`). Contrast: subtypes 0x00–0x0C all use plain `SetEntityIndexAndRot` (half-speed box). Included for the sub_49E10 contrast only.

---

## Creature-attack / player-spell wiring (post-writes over creators)

Projectiles are usually spawned by a caster that post-writes impact fields, aim, homing target, behavior-row override, damage, and target filter. Verbatim examples:

**`sub_1CC20(attacker, victim)`** (EF:9680) — creature thunk, subtype 0: `_4A190(pos,9,0)` → `byte_0x43_67=10; byte_0x44_68=0` (impact effect (10,0)); `id = attacker.id`; `yaw=tan2(→victim); pitch=radix_tan(→victim)`; `pos.z += attacker.box.fov`; `word_0x96_150 = attacker.word_0x96_150` (inherit homing target); **`dword_0xA0_160x = &str_D7BD6[65]`** (yaw cap 0 — straight shot); `xsubtype = victim.model; xtype = victim.class` (filter); `subSpellIndex = 500` (damage); `sub_5EF70(victim)`.

**`sub_1D460(attacker,victim)`** (EF:9918) — 5-way fireball spread: loop v2=0..4 with yaw offset v3 ∈ {-226,-113,0,+113,+226}; each `_4A190(pos,9,0)`, `byte_0x43_67=10; byte_0x44_68=0`; **`dword_0xA0_160x=&str_D7BD6[61]`** (cap 113); `xsubtype/xtype` = victim; `subSpellIndex=800`; `yaw = v3 + tan2(→victim)`; `pos.z += 200`; inherit `word_0x96_150`. (Prompt's "800/1600" — 800 here; the doubling to 1600 is a charge multiplier applied elsewhere, OPEN exact site.)

**`sub_1D260(m23 attacker,victim)`** (EF:9883) — subtype 9 thunder: `_4A190(pos,9,9)`; `byte_0x43_67=10; byte_0x44_68=23`; aim toward victim; **`dword_0xA0_160x=&str_D7BD6[64]`** (yaw cap 5); `xsubtype/xtype`=victim; `subSpellIndex=4000`. Matches prompt.

**`sub_6DCA0(caster,pos,spellIdx a3, spellData a4, speedBoost a5, playSound a6)`** (EF:44020) — master player-spell dispatcher. Per spell index a3 → subtype spawn + impact fields + subSpell + sound id v6 (default 15):
- a3=0 **Fireball**: subtype 0 (or **28=0x1C if `a4.life_0x1A>=2` charged**), impact (10,0) or (10,76); v6=9 (EF:44068-44092).
- a3=7: subtype 12 impact (9,9) [if life_0x1A≥1] or subtype 9 impact (10,23) [else], subSpell=`a4.subSpellIndex_2`, v6=9 or 23 (EF:44052-44078).
- a3=8/9: subtype 3, impact (10,17), subSpell=`a4.subSpellIndex_2`, `byte_0x46_70=a4.life_0x1A`, v6=15 (EF:44099-44106).
- a3=0xD: subtype 8, impact (10,25) (EF:44112).
- a3=0x10: subtype 5, impact (10,11), subSpell=…, byte_0x46=life (EF:44138).
- a3=0x11: subtype 2, impact (10,15) (EF:44152).
- a3=0x12: subtype 4, impact (10,9) (EF:44164).
- (a3=0xF→subtype 23 arrow, 0x13→22, 0x15→26, 0x19→30: out of range 0x00–0x0C.)
- After spawn (EF:44226): `actSpeed = a5 + actSpeed`, clamped to [384, 0x2000]; if a6, **`PrepareEventSound_6E450(caster,-1,v6)`** (cast sound).

---

## ACTION STATES (str90)

Every flight state shares this skeleton: acquire-or-track → speed ramp → `MoveEntity` → victim probe → terrain/water/z clamp → life countdown → impact spawn. Sounds during flight: **none in the flight functions themselves** — only ricochet (28), drone-hit (26), and cast (v6) fire; the spawned class-10 impact entities carry their own sounds.

### State 0x00 — `CastPlayerFire_65B30` (EF:63005) → `sub_65C20` (EF:63057) — FIREBALL FLIGHT (creature bolts ride this)
`CastPlayerFire` (EF:63005): `if (sub_65C20(self)) DisableEntityDrawing04_57F10(self)` — i.e. run flight; if it returned an impact spawn (nonzero) despawn the projectile.

`sub_65C20(self)` full law:
1. `v1 = Entities[word_0x96_150]` (locked target).
2. **If target live** (`v1 > Entities[0]`): `sub_65610(self, v1)` (homing re-aim, caps from behavior row) then jump to move.
3. **Else (no live target)**, one-shot init if `!(byte[0]&2)`: set bit 2; then **`if sub_68940(self)`** (drone-lock): `v6=sub_582B0(yaw,roll)` clamp [0,34]; `yaw = v6*sub_582F0(yaw,roll) + yaw` (limited turn, cap 34), `pitch = fov`. **Else if !`sub_67CB0(self)`** (no auto-target): `roll=yaw; fov=pitch` (lock straight) and skip. **Else** (auto-target found): `v3=sub_582B0(yaw,roll)` clamp [0,34]; `yaw = v3*sub_582F0 + yaw`; `pitch=fov`. (Turn cap here is hard-coded **34**, not the row.)
4. **Move**: `pred = pos; MoveEntity_57FA0(&pred, yaw, pitch, actSpeed); CopyEntityPosition(self, &pred)`. (No actSpeed ramp in this state — flies at constant 384 unless changed by caster.)
5. **Victim probe** `sub_10780(self)`:
   - **Hit `v8`**: if `v8->word[0] & 0x8010` (shielded): `if sub_68740(self,v8,0x5B,45) return 0` (ricochet, **sound 28**, RNG jitter). Else: `if v8->dword_0xA0_160x->byte_160_0x20_32 & 0x10 → subSpellIndex=1`; `sub_65580(v8)`; snap pos to victim; `sub_655A0(v8)`; `v20=1` (impact).
   - **No hit**: `v11=getTerrainAlt(pos)`; if terrain above or (cave && z below `sub_10C60-box.fov`): clamp z; **if `model!=4` && water tile (`sub_104D0==1`)**: spawn splash `_4A190(pos,10,5)` (id=self.id), despawn, done (no normal impact). Else: `life--; if life>=0` continue (no impact); else `v20=1`.
6. **Impact `v20`** (EF:63174): `if sub_68AC0(self,v9)` (drone-collision, **sound 26**) → despawn. Else spawn impact `v18 = _4A190(pos, byte_0x43_67, byte_0x44_68)`; if spawned: `sub_65780`(stats), `sub_686D0`(retarget owner), `if v9>0 sub_6D8B0(id,0,1)` (fire-XP idx 0), then copy to impact entity: `subSpellIndex, id, yaw, pitch`; `if !v9 word_0x96_150=0`. Returns v18 (nonzero → CastPlayerFire despawns projectile).

Turn caps: initial-aim cap **34** (hard-coded); tracking cap from behavior row (str_D7BD6[64] for fireball → yaw 5/pitch 22). Homing target = `word_0x96_150`; filter via xtype/xsubtype. Gravity: none; z follows pitch + terrain clamp. **RNG in order**: only inside `sub_68740` on ricochet: 1 draw `rand=9377*rand+9439` (EF:55293) when target not `byte[0]&0x10`.

### State 0x01 — `CastPosses_65F60` (EF:63210) — POSSESSION PROJECTILE
1. `v1=Entities[word_0x96_150]`. If not live: init `byte[0]|=2`; `if sub_67CB0(self)` → `yaw=roll; pitch=fov` (no sub_68940 branch, no cap-34 partial turn — full snap). If live: `sub_65610` (row str_D7BD6[61], caps 113/113).
2. Move (constant actSpeed).
3. Z: clamp to terrain / cave.
4. Victim probe **`sub_108B0`** (EF:3783, NOT sub_10780) — a specialized probe accepting class-5/model-22, class-10/model{0x27,0x28,0x2D,57} targets (possession victims). No 0x8010 ricochet branch. On hit: `sub_65580`, snap pos, `sub_655A0`, `v15=1`.
5. Else terrain/life as usual.
6. Impact `v15`: spawn `_4A190(pos, byte_0x43_67, byte_0x44_68)`; if spawned: `sub_65780`; `if v7>0 sub_6D8B0(id,1,1)` (**possession-XP idx 1**); copy id/yaw/pitch; despawn self. **No dedicated sound; no RNG draws** (no ricochet path).

### States 0x02–0x08 — thin wrappers over generic flight `sub_65820`
- **0x02 `sub_66160`** (EF:63329): `r=sub_65820(self); if r r->life = byte_0x46_70` (impact entity gets charge-life).
- **0x03 `sub_66180`** (EF:63340): `v1=sub_65820(self)`; then if `class!=0` spawn TWO trailing sparks: `rand=9377*rand+9439; v4.x = rand%0x81 + pos.x -96-64`; `rand=…; v4.y = rand%0x81 + pos.y -96-64`; `v4.z=pos.z`; `_4A190(v4,10,0)` with flags |=0x10080, id, life=4, animFrame=3, yaw. Then if v1: `v1->maxLife=v1->life = byte_0x46_70`. **RNG: 2 draws** (x then y jitter).
- **0x04 `sub_66250`** (EF:63380): `r=sub_65820(self); if r { r->byte_0x46_70=0; r->maxLife = byte_0x46_70 }`.
- **0x05 `sub_66280`** (EF:63396): `r=sub_65820(self); if r r->life = byte_0x46_70`.
- **0x06 `sub_662A0`** (EF:63407): `return sub_65820(self)` (bare).
- **0x07 `sub_662C0`** (EF:63413): `sub_662E0(self)` (delegates to state-8 body).
- **0x08 `sub_662E0`** (EF:63419): the "generic-with-speed-ramp" variant (see below).

**Generic flight `sub_65820(self)`** (EF:62882) — used by states 2–6, 0x0B:
1. Track/init: if live target `sub_65610`; else init `byte[0]|=2`, `if sub_68940 || sub_67CB0 → yaw=roll,pitch=fov` else `roll=yaw,fov=pitch`.
2. **Speed ramp** (EF:62923): `v3 = sign(minSpeed - actSpeed)` (±1 or 0); `actSpeed += 2*v3` — ramps actSpeed toward minSpeed by ±2/tick (here min==act==384 so no change; matters when caster bumped actSpeed).
3. Move; `CopyEntityPosition`.
4. `sub_10780`: hit → `if word[0]&0x8010 && sub_68740(self,v4,0x2D,22) return 0` (**ricochet a3=0x2D/a4=22, sound 28, 1 RNG draw**); else `sub_65580`, snap, `sub_655A0`, `v14=1`.
5. No hit → terrain/cave clamp; **water: model ∉ {4,22,24,26}** && water → splash `_4A190(pos,10,5)`, despawn; else `life--`, impact if <0.
6. Impact: `if sub_68AC0` (sound 26) despawn; else spawn `_4A190(pos,byte_0x43_67,byte_0x44_68)`; `sub_65780`; `sub_686D0`; `if v5>0 && word_0x26_38 sub_6D8B0(id, Entities[word_0x26_38]->model, 1)`; **if `byte_0x44_68==34` impact.life = subSpellIndex**; copy id/yaw/pitch, `word_0x96_150=victim`, subSpellIndex, byte_0x46_70; despawn self. Returns impact.

**`sub_662E0(self)`** (state 8 body, EF:63419) — nearly identical to sub_65820 but: track branch checks `sub_68940 || sub_67CB0` then snap (EF:63450); same **speed ramp ±2**; `sub_10780` ricochet a3=0x2D/a4=22 (**sound 28**, 1 RNG on non-0x10 target); water clamp `model!=4` only; impact block (EF:63530) additionally handles owner class-3 model{0,1} (spawns child, `sub_6D8B0(id, Entities[word_0x26_38]->model, 1)`) vs else path (`sub_65780(self,0,orig)` + despawn).

### State 0x09 — `sub_66750` (EF:58268) — THUNDER/LIGHTNING BEAM (instant hitscan trail)
Not a flight loop — a one-tick beam:
1. `actSpeed = minSpeed`; `SetMapEntity_57E50`; run `sub_66610` repeatedly until `byte[1]&4` set, counting steps `v27` (traces path to first blocker); restore yaw/pitch.
2. Compute per-segment vector `v22` via `MoveEntity(v22, yaw, pitch, actSpeed/8)`; `v27 *= 8`.
3. **Loop v27 down to 0**: each iteration spawn a class-9 trail node (`NewEvent`: actionIndex=14, class=9, **model=9**, id=self.id, maxLife=(v6>=a1)-1, `SetEntityIndexAndRot(216)`, AddToMap). Jagged offset via **RNG**: `rand=9377*rand+9439; v9 = 2*((rand%0x9D)/79) -1 + prev` (offset chain, EF:58359) — **2 RNG draws per node** (v28 z-jag and v25 x/y-jag, EF:58359 & 58373). Advance `pred += v22`; `v20.z = v28*(v26/4)+pred.z`; `MoveEntity(v20, yaw±(2<<8), 0, v28*(v26/4))`.
4. **Impact at beam end**: `v13=sub_10780(self)`; `_4A190(v20, byte_0x43_67, byte_0x44_68)`; if spawned: `sub_65780`, `sub_686D0`, `sub_68AC0` (drone, sound 26), `if v13>0 sub_6D8B0(id, 7, 1)` (**lightning-XP idx 7**); copy id/yaw/pitch; `word_0x96_150 = victim` (or 0xae02 sentinel if none). Shielded-target mana handling (EF:58422): if victim shielded (`word[0]&0x8010`) and `byte[1]>=0`: if `self.mana/8 <= mana` → impact.subSpellIndex = self.subSpellIndex/2; else full; if `byte[1]<0`: if `self.mana/4 > victim.mana` full else impact.subSpellIndex = self.subSpellIndex>>2. RNG total: **2 per trail node** (variable count). No cast sound here.

### State 0x0A — `CastCastleProjectile_66B30` (EF:58461) — CASTLE/SIEGE PROJECTILE
1. `v1=Entities[word_0x96_150]`.
2. **If live target**: `sub_656D0(self,v1)` (EF:62809 — same as sub_65610 but WITHOUT the z raise/lower, straight homing with row caps); **speed ramp ±2**; Move; **if `sub_106C0(self, target)`** (in-box) → snap to target, impact `v11=1`. Else terrain/cave clamp or `life--` → impact.
   - Impact: **if `byte_0x43_67==3` and owner has castle** (`Entities[id]->dword_0xA4_164x->CastleEntityIndex_0x3A_58`) → just despawn. Else spawn `_4A190(pos, byte_0x43_67, byte_0x44_68)`; if spawned set id, despawn; **else `sub_5F890(owner, 0)`** (castle-damage callback).
3. **If NO live target**: `sub_66D00(self)` (EF:58559) — free-flight castle mode: one-shot `sub_11CB0` LOS check (if blocked: `sub_5F890(self,0)`, despawn, `sub_88D00`); else homes toward `axis_0x9A_154x` target point using row caps (`sub_58350` yaw/pitch, caps subtype_0x2/word_0x6), speed ramp ±2, Move, probe `sub_106C0(word_0x96_150)`, terrain/life; on impact optionally spawn `_4A190`. No sound in this state's flight; damage via `sub_5F890` or impact effect.

### State 0x0B — `sub_66FB0` (EF:58685)
`return sub_65820(self)` — bare generic flight (see sub_65820 above: ricochet a3=0x2D/a4=22 sound 28, water splash (10,5), impact via byte_0x43_67/byte_0x44_68). RNG: 1 on ricochet.

### State 0x0C — `sub_66FD0` (EF:58691) — LIGHTNING II
Generic flight + special drone-lock life extension:
1. If not live target, init `byte[0]|=2`: **if `sub_68940`** (drone lock): `life += 32; yaw=roll; life=maxLife=life+32; pitch=fov`; if `word_0x34_52` set, free that sub-entity (`sub_57F20`), clear it. **else if `sub_67CB0`** → snap yaw=roll,pitch=fov. **else** roll=yaw,fov=pitch. If live target: `sub_65610` (row str_D7BD6[60], caps 22/22).
2. **Speed ramp ±2** (EF:58751); Move.
3. `sub_10780`: hit → `if word[0]&0x8010 && sub_68740(self,v6,0x2D,22) return` (**ricochet, sound 28, 1 RNG**); else `sub_65580`/snap/`sub_655A0`, `v15=1`.
4. No hit → terrain/cave; water (`model!=4`) → splash `_4A190(pos,10,5)`; else `life--` impact.
5. Impact: `if sub_68AC0` (**sound 26**) despawn; else spawn **`_4A190(pos, 10, 38)`** (hard-coded impact effect (10,38), NOT byte_0x43/44); `sub_65780`; `if v7>0 sub_6D8B0(id, 7, 1)` (**lightning-XP idx 7**); copy id/yaw/pitch; `word_0x96_150=victim` (or 0xae02); copy subSpellIndex; **copy byte_0x43_67/byte_0x44_68 to impact** (chains); despawn self.

---

## Sounds (all ids, via `PrepareEventSound_6E450(entIdx, levelIdx, wavId)`, Sound.cpp:6254)
- **Cast sound**: `sub_6DCA0` v6 per spell (fireball=9, meteor/thunder=23, others=15 default, EF:44233).
- **Ricochet off shield** (`sub_68740`): **id 28** (EF:55272), applies to states 0/2/3/5/7/8/0xB/0xC and creature bolts.
- **Drone/guide collision** (`sub_68AC0`): **id 26** (EF:55437).
- **Water-splash impact** context: id 27 seen at EF:26693 (adjacent splash spawner, not in these flight fns; the flight water-splash just spawns effect (10,5) which self-sounds — OPEN whether (10,5) plays 27).
- Arrow (0x0D, ref only): `AddArcherArrow_672E0` plays **id 33 or 34** (`(rand&1)+33`, 1 RNG draw, EF:58866) — not in 0x00–0x0C.
- Impact class-10 effect entities carry their own sounds (out of this family's scope).

## RNG summary (LCG `rand = 9377*rand + 9439`, per-entity `rand_0x14_20`; some use global `D41A0_0.rand_0x8`)
- Creators 0x00–0x0C, 0x1C: **0 draws each**.
- Flight states 0,1,0x0A: 0 draws in nominal flight; **+1 draw only on ricochet** (`sub_68740`, states with 0x8010 branch = 0,2,3,5,7,8,0xB,0xC).
- State 0x03 (`sub_66180`): **2 draws/tick** (spark x,y jitter).
- State 0x09 (`sub_66750`): **2 draws per trail node** (z-jag, xy-jag), variable node count.

## OPEN / uncertain
- The `word_160_0x1a_26` re-aim cadence divisor (value 40/50 in rows) is consulted only by creature-AI cast-timing (EF:9653), **not** by any flight state 0x00–0x0C — flight re-aims every tick. Confirm no other state gates on it.
- Exact damage numbers: projectiles carry `subSpellIndex_0x2A_42` (500/800/4000/etc. set by casters) and impact effect subtype; the numeric damage formula lives in the spawned class-10 effect handler (out of scope). `sub_6D8B0` is XP accounting, not damage.
- `sub_67CB0` full scoring (which entity lists / `sub_68490`/`sub_685D0` weights) not fully transcribed (EF:54710-~54960) — only the model-case gate and `word_160_0x1c_28` range field are established; deeper scoring is likely not needed for a faithful flight port but flagged.
- "1600" subSpell for the 5-way fireball (`sub_1D460` sets 800): the ×2 charge site not located — OPEN.
- The charged-fireball flies under **action state 29** (subtype 0x1C sets actionIndex=29), which is outside 0x00–0x0C; another agent/scope covers state 29 if needed.