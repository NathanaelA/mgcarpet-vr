I now have full understanding. `SetEntityIndexAndRot_49CD0(e, idx)` sets word_0x5A_90 = the sprite/particle index (the "sprite row"/animation), and derives collision box from particlesParameters. Let me quickly check SetEntityIndex_49C90 to confirm word_0x5A_90 is the sprite row, and read the m26 remaining handlers I haven't fully seen (sub_29300 = sub_1C560 wrapper, sub_29380 = sub_1D5D0 wrapper — these are the thin ones). I have enough. Let me write the report.

Let me verify one detail: sub_1BF90/sub_1BD90 signatures (the base action = 200/208/224 passed). Those are already ported per CONTEXT. I have everything.

---

# MC2 Engine Trace — Class-5 models 25/26/28 + Class-2 scenery models 3–8 & tree burn

All citations are `EventsFunctions.cpp:line` unless prefixed `Events.cpp:`. Address→symbol confirmed via dispatch table `str_D4C48ar[class].dword_10[actionIndex]` (Events.cpp:435). Handler bodies live in EventsFunctions.cpp; the per-address `case` dispatch is in Events.cpp.

Shared helper semantics used below (verified):
- `SetEntityIndexAndRot_49CD0(e, idx)` (:32838) → `SetEntityIndex_49C90` sets **sprite/particle row = `word_0x5A_90 = idx`**, then sets collision box halves `array_0x52_82.{pitch,roll} = speed_6/2`, `{yaw,fov} = rotSpeed_8/2` from `particlesParameters_D951C[idx]`.
- `SetHalfSpeedEntity_49DA0(e, idx)` (:32856) = same as above then re-halves pitch/roll/fov (net identical values; class-2 scenery uses this).
- `SetEntityShiftRot_49EA0(e, shift, fov)` (:32874) sets collision `array_0x52_82.pitch = roll = shift`, `fov = fov` (overrides the box from the index call — this is the **ShiftRot** the CONTEXT refers to).
- `IfSubtypeCallCreatingManaSphere_4A190(pos, type, subtype)` (Events.cpp:5186) spawns entity from prototype `str_D4C48ar[type].dword_14[subtype]` at `pos`; returns entity or 0. "(10,N)" = class-10 model N; "(9,N)" = class-9 subtype N projectile.
- **Burn/damage mailbox**: `str_0x5E_94.dword_0x5E_94` = accumulated damage, `str_0x5E_94.word_0x62_98` = attacker entity index (nonzero ⇒ "hit pending"). Written by area/melee damage (see Part B tree section).

---

## PART A — Class-5 (models 25, 26, 28), base action = model*8

### Model 25 — creator `sub_4CE00` (:34523), states 0xC8–0xCF = actions 200–207

**CTOR `sub_4CE00` (:34523), field writes in order:**
- `actionIndex_0x45_69 = 201` (:34528) — spawns *into* idle-brain state (+1), not +0.
- `class_0x3F_63 = 5` (:34529); `model_0x40_64 = 25` (:34530).
- `byte_0x46_70 = 0` (:34531) — sub-state.
- `minSpeed_0x84_132 = 60` (:34532); `maxSpeed_0x86_134 = 20` (:34533); `maxLife_0x4 = 7500` (:34534).
- `actSpeed_0x82_130 = minSpeed (60)` (:34535).
- `SetEvent144_49C70(v1x)` (:34536).
- **RNG draw #1** (:34537): `rand = 9377*rand + 9439`. `fov_0x22_34 = 0` (:34538); `roll_0x20_32 = (rand & 0x7FF)-1` (:34539); `yaw_0x1C_28 = (rand & 0x7FF)-1` (:34540) [same draw]; `pitch_0x1E_30 = roll` (:34541). **Only 1 RNG draw**; roll/yaw share it.
- `subSpellIndex_0x2A_42 = 300` (:34542) — CONTEXT-confirmed "subSpell 300"; used both as attack damage magnitude AND as a lifetime countdown in the brain (see handler 200).
- `byte_0x38_56 = 1` (:34543) — flammable bit0 set (burnable).
- `dword_0xA0_160x = &str_D7BD6[92]` (:34544) — behavior-row index 92.
- `byte_0x39_57 = 64` (:34545); `xtype_0x41_65 = 3` (:34546).
- `byte_0x3E_62 = D41A0_0.array_0x10[25]++` (:34547) — per-model instance counter (used as phase mask `& 7`/`& 0x1F` for staggering AI).
- `AddEventToMap_57D70`; `CopyMaxLifeToLife_49A20` (life=7500) (:34548–34549).
- `SetEntityIndexAndRot_49CD0(v1x, 290)` (:34550) — **sprite row 290**.
- `SetEntityShiftRot_49EA0(v1x, 384, 384)` (:34551) — collision box pitch=roll=384, fov=384.

**Handlers (m25 actions 200–207):**

- **200 (0xC8) = `sub_28860` (:18828) — patrol/brain (model-specific, full):**
  - `v1 = byte_0x46_70`. If not in death sub-states (1 or 2) (:18859):
    - If `word_0x62_98` (hit pending): `life -= dword_0x5E_94`; save attacker into `word_0x26_38`; clear flag; `v2=1` (:18861–18867). Else `word_0x26_38=0`.
    - Follow `word_0x34_52` linked-list of sub-entities: adopt the minimum life among them into own life, and its `word_0x26_38` (:18873–18889).
    - If `life < 0`: `word_0x24_36 = word_0x26_38`; `v2=2` (death) (:18890–18894).
  - `subSpellIndex_0x2A_42--` (:18896); if it reaches 0 ⇒ `v2=2` (**lifetime expiry: 300 ticks**).
  - If `v2==2` ⇒ `actionIndex = 204` (prekill) (:18902).
  - Else sub-state machine on `byte_0x46_70`:
    - **case 1**: `dword_0x10_16 = 52`; `byte_0x46_70 = 2`; fall to case2.
    - **case 2** (LABEL_20): reset `life = maxLife`; clear hit flag; `dword_0x10_16--`; if <0 → LABEL_21 (`byte_0x46_70=3`, v26=1); if >13, `roll` HIBYTE = `(HIBYTE+1)&7` (spin) (:18912–18928).
    - **case 3**: acquire target `word_0x24_36`; if invalid/not class-3 model 0|1 → `byte_0x46_70=8, dword_0x10_16=100` (:18929–18936). If target's `CastleEntityIndex_0x3A_58` set → `byte_0x46_70=5`, `word_0x96_150=target` (:18937–18942). Else **RNG draw** `rand%100`; `byte_0x46_70=4`, `dword_0x10_16 = rand%100 + 100` (:18945–18948).
    - **case 4**: `dword_0x10_16--`; if <0 → LABEL_35 (`byte_0x46_70=3`) (:18956–18960).
    - **case 5**: read castle index; if `!(byte_0x3E_62 & 7)` (1-in-8 stagger), aim `roll = tan2(self,castle)` and if `CompareAxisWithShift_10750` in-range → `byte_0x46_70=6` (:18962–18972). If no castle → LABEL_35 (`=3`).
    - **case 6**: `byte_0x46_70=7`, `v26=1`; fall to case7.
    - **case 7** (LABEL_41): read castle; if in range → **ATTACK: `sub_11900(a1x, castleEntity, 0, 0x3C)`** (:18992) — writes 60 damage to castle mailbox (see below). Else `byte_0x46_70=5`, v26=1. No castle → LABEL_21 (`=3`).
    - **case 8**: `dword_0x10_16--`; if <0 → `actionIndex=204` (prekill) (:19007–19011).
  - Wander steering (if `!(byte_0x3E_62 & 7)`): **2 RNG draws** (:19018,:19020) → `roll += (2*(rand%157/79)-1)*(rand%381)`, `roll &= 0x7FF` (:19019–19023).
  - `sub_1B8C0(a1x)` (move) (:19025).
  - Water/terrain sprite swap (:19026–19047): if on water and `word_0x5A_90==314` and above terrain → sprite **313**; else on water → sprite **314**, `minSpeed=35`, v26; else if sprite≠313 → sprite **313**, `minSpeed=60`, v26.
  - If v26: `actSpeed = minSpeed`, and if sub-state==2, `actSpeed = minSpeed+50` (:19048–19055).
  - **m25 attack payload = `sub_11900(...,0,0x3C)`: 60 damage to the castle via mailbox** (drains a castle, not a projectile).

- **201 (0xC9) = `sub_28C30` (:19062):** thin — `sub_1BF90(a1x, 200)` then clamp `life≥0`. (base action 200.)
- **202 (0xCA) = `sub_28C60` (:19076):** sound `PrepareEventSound_6E450(idx, -1, 37)` (**sound id 37**); then `sub_1C310(a1x, 200, sub_1CC20)` — the **class-9 subtype-0 attack projectile**; clamp life≥0. `sub_1CC20` (:9680): spawns `(9,0)` mana-sphere/projectile, `byte_0x43_67=10`, aims yaw/pitch at target, `subSpellIndex=500`, behavior-row `str_D7BD6[65]`, tags target class/model.
- **203 (0xCB) = `sub_28CC0` (:19089):** `actionIndex = 201`; clamp life≥0 (pack/re-arm).
- **204 (0xCC) = `sub_28CE0` (:19103) — prekill (model-specific, full):**
  - If `byte_0x46_70` nonzero → `PreKillEntity_1C890(a1x, 200)` and return (:19118–19122).
  - Else if `sub_4A810_get_0x35plus() <= 1` (entity pool nearly full) → `TransformEntityToManaSphere_36BA0(a1x, false)` (:19125).
  - Else split into **3 child model-25 spawns** (loop i<3, :19131): each gets `actionIndex=200`, class5/model25, copied position, `byte_0x46_70=3`, `minSpeed=35,maxSpeed=60,actSpeed=85`, `mana = parentMana/3`, **1 RNG draw** per child (:19150) → `roll=(rand&0x7FF)-1`, `yaw=roll`; `maxLife=80`, `subSpellIndex=15000`, `byte_0x38_56=1`, instance counter, behavior-row `str_D7BD6[95]`, `byte_0x39_57=64`, `xtype=3`; sprite row **314** (:19167), ShiftRot(32,32); `byte_0x46_70=1`; `word_0x24_36 = parent.word_0x26_38`. Leftover mana added to last child (:19173).
  - Spawn `(10,1)` mana sphere and set its id (:19176). `actionIndex = 205`; `D41A0_0...dword_0x364D2 += 3` (:19181).
- **205 (0xCD) = `sub_28EC0` (:19187) — kill/score:** if `byte_0x46_70`: bump player kill counter if `word_0x24_36`==player, then `KillEntity_1C930`. Else bump counter, `life=-1`, `DisableEntityDrawing04_57F10` (:19187–19201).
- **206 (0xCE) = `sub_28F40`:** *not present as a body.* Dispatch has no `0x209f40` case; **OPEN — action 206 (0xCE) has no handler function; likely an unused/empty slot** (Events.cpp jumps to whatever `str_D4C48ar` holds; the code base has no `sub_28F40`).
- **207 (0xCF) = `sub_28F50` (:19205) — hit/flee:** if `byte_0x46_70`: `sub_12470(a1x, 200)` (resets StageVars + actionIndex=200) then `byte_0x46_70=3`. Else `sub_1D5D0(a1x, 200)`.

**Note on state roster:** m25 spawns at action 201, and its brain (`sub_28860`) is at 200, so the "+0 patrol / +1 idle brain" labels are swapped for this model vs. the generic roster — flag as intentional deviation.

---

### Model 26 — creator `sub_4CF00` (:34557), states 0xD0–0xD7 = actions 208–215; post-init `sub_293D0`

**CTOR `sub_4CF00` (:34557):**
- `actionIndex_0x45_69 = 209` (:34562) — spawns into +1 (idle brain).
- `class=5, model=26` (:34563–34564).
- `minSpeed=25, maxSpeed=25` (:34565–34566) — no speed range; `maxLife=4400` (:34567); `actSpeed=maxSpeed(25)` (:34568).
- `SetEvent144_49C70` (:34569).
- **RNG draw #1** (:34570): `roll=(rand&0x7FF)-1` (:34572), `yaw=(rand&0x7FF)-1` (:34573, same draw), `pitch=roll`, `fov=0`. **1 RNG draw.**
- `subSpellIndex_0x2A_42 = 300` (:34575) — CONTEXT said "300" (matches; the "2000" in header note belongs to m28).
- `byte_0x38_56 = 1` (:34576) burnable; `dword_0xA0_160x = &str_D7BD6[99]` (:34577) — **behavior-row 99**; `byte_0x39_57=64`, `xtype=3`; `byte_0x3E_62 = array_0x10[26]++`.
- `AddEventToMap`; `CopyMaxLifeToLife` (life=4400); `SetEntityIndexAndRot_49CD0(v1x, 318)` (:34583) — **sprite row 318**; `SetEntityShiftRot_49EA0(v1x, 256, 384)` (:34584).
- **`sub_293D0(v1x)` post-init (:34585).**

**Non-standard post-init `sub_293D0` (:19425), fully traced:**
```
if (actionIndex != 210) {           // 210 == 0xD2 == attack state
    v2 = byte[2] of struct_byte_0xc;
    word_0x96_150 = 0;              // clear target
    byte[2] = v2 | 0x80;            // set "awake/active" flag bit7
    actSpeed_0x82_130 = maxSpeed;   // full speed
}
```
This is a "wake" primitive: it clears the current target and forces full speed with the active bit set, UNLESS the entity is already in the attack state (210). It is dispatched as a normal action too (Events.cpp:2090, addr 0x20a3d0). Its inverse `sub_293B0` (:19411): if `actionIndex==210`, clear bit7 and set `actSpeed=minSpeed` (slows during attack). Both are called at the tail of every m26 movement handler.

**Handlers (m26 actions 208–215):**
- **208 (0xD0) = `sub_28F90` (:19219):** `sub_1BD90(a1x, 208)` then `sub_293B0` (patrol +0).
- **209 (0xD1) = `sub_28FC0` (:19226):** `sub_1BF90(a1x, 208)` then `sub_293B0` (idle-brain wrapper +1).
- **210 (0xD2) = `sub_28FF0` (:19233) — attack brain (model-specific, full):**
  - If `!(byte_0x3E_62 & 0x1F)` (1-in-32 stagger): sound `PrepareEventSound_6E450(idx,-1,62)` (**sound id 62**) (:19255).
  - Hit-flag/child-min-life absorption identical to m25 (:19257–19290): `v1=0/1/2`; on hit `life-=dword_0x5E_94`, save attacker; on `life<0` → `v1=2`, `word_0x24_36=attacker`.
  - `v1==0`: skip to move. `v1==1`: `word_0x96_150 = word_0x26_38` (target = attacker). `v1==2`: `actionIndex=212` (prekill), `sub_293D0`, return (:19301–19307).
  - `sub_1B8C0` (move) (:19310).
  - Target `v6x = Entities[word_0x96_150]`; if valid class-3 model 0|1, alive, not flagged (:19312–19315):
    - If `!(byte_0x3E_62 & 3)`: aim `roll=tan2(self,target)`; scan same-model neighbors within `array_0x52_82.pitch` box and re-aim away from crowding (:19317–19330).
    - **Drain: `target.mana -= (target.manaRegen + 14)`, clamp 0** (:19331–19334) — m26's "attack" is a **mana leech**, not a projectile.
    - If `byte_0x3E_62 & 3` → `sub_293D0` return.
    - `v10 = distance_3d(self,target)`; if `v10 <= behaviorRow.word_160_0x1c_28` (attack range): if `v10 >= 2048` → break off; if target has a model (not player model 0) → break off; else **RNG draw** `D41A0.rand%63` (:19346–19347):
      - `<4`: no cast. `==4`: cast target's **right spell** via `sub_69300(Entities[rightSpellEntity], a1x)`. `==5`: cast target's **left spell** via `sub_69300(...)`. Otherwise break off. (:19348–19373)
  - Else `actionIndex = 209` (:19378); always tail-call `sub_293D0`.
  - **m26 attack linkage**: m26 IS a spell-carrier entity — `sub_69300` (:55792) hijacks the *player's own* left/right spell slot (`SpellIndexLeft/Right`), sets that spell entity's `actionIndex=78`, positions it on the player, and disables the slot. Net effect: forces the player to discharge their equipped spell. It also drains the player's mana directly (the `-14` above).
- **211 (0xD3) = `sub_29300` (:19386):** `sub_1C560(a1x, 0xD0)` then `sub_293B0` (pack/regroup +3).
- **212 (0xD4) = `sub_29330` (:19393):** `PreKillEntity_1C890(a1x, 208)` (prekill +4).
- **213 (0xD5) = `sub_29350` (:19399):** `KillEntity_1C930(a1x)` (kill +5).
- **214 (0xD6) = `sub_29370`:** *no body present.* Dispatch has no `0x20a370` case in the block. **OPEN — m26 action 214 (0xD6) has no handler function (empty slot).**
- **215 (0xD7) = `sub_29380` (:19405):** `sub_1D5D0(a1x, 208)` then `sub_293B0` (hit/flee +7 wrapper).

---

### Model 28 — creator `sub_4D1D0` (:34695), states 0xE0–0xE7 = actions 224–231

**CTOR `sub_4D1D0` (:34695):**
- `actionIndex_0x45_69 = 225` (:34700) — spawns into +1.
- `class=5, model=28` (:34701–34702).
- `minSpeed=120, maxSpeed=64` (:34703–34704) — **fastest** (CONTEXT "120/64"); `maxLife=8000` (:34705).
- `SetEvent144_49C70` (:34706).
- **`struct_byte_0xc_12_15.byte[3] |= 8`** (:34707) — CONTEXT-confirmed flag.
- **RNG draw #1** (:34708): `roll=(rand&0x7FF)-1`, `yaw=(rand&0x7FF)-1` (same draw), `pitch=roll`, `fov=0`. **1 RNG draw.**
- `subSpellIndex_0x2A_42 = 2000` (:34713) — CONTEXT "subSpell 2000" (melee damage magnitude).
- `byte_0x38_56 = 1` (:34714) burnable; instance counter `array_0x10[28]++` (:34715); `dword_0xA0_160x = &str_D7BD6[93]` (:34716) — **behavior-row 93**; `byte_0x39_57=64`, `xtype=3`.
- `actSpeed_0x82_130 = maxSpeed + (minSpeed-maxSpeed)/2 = 64 + 28 = 92` (:34719).
- `AddEventToMap`; `CopyMaxLifeToLife` (life=8000); `SetEntityIndexAndRot_49CD0(v1x, 292)` (:34722) — **sprite row 292**; `SetEntityShiftRot_49EA0(v1x, 85, 42)` (:34723).

**Handlers (m28 actions 224–231):**
- **224 (0xE0) = `sub_2B1D0` (:20990):** `sub_1BD90(a1x, 224)`; if resulting `actionIndex==226` → `sub_2B840` (patrol +0).
- **225 (0xE1) = `sub_2B200` (:21002):** `sub_1BF90(a1x, 224)`; if `actionIndex==226` → `sub_2B840` (idle-brain +1).
- **226 (0xE2) = `sub_2B260` (:21010) — attack brain (model-specific, full):**
  - `v1 = sub_2B9A0(a1x)` (:21045): hit/child-min-life absorption (:21356–21413) identical pattern; on death sets `actionIndex=228` and returns 2.
  - If `v1 <= 1`, switch on `byte_0x46_70` (:21049):
    - **case 0**: `sub_2B860(a1x, 3)` (sprite/state config mode 3 — see below); `sub_2BA50(a1x, 1)` (set sub-state 1, dword_0x10_16=0). return.
    - **case 1**: spawn `(10,5)` blood/mana sphere at pos (`IfSubtypeCallCreatingManaSphere_4A190(pos,10,5)`); `sub_2BA50(a1x, 2)` (sub-state 2, dword_0x10_16=32). return.
    - **case 2 (chase)**: validate `word_0x96_150` target (alive, not flagged). If invalid or `--dword_0x10_16 <= 0` → `v29=1`, go LABEL_31. Else predict target pos `MoveEntity_57FA0(&pred, target.yaw,0,768)`; if `!(byte_0x3E_62&3)` aim `roll=tan2(self,pred)`; neighbor-crowding re-aim (:21082–21098). `sub_1B8C0` move; if returns 3 (blocked?) → `sub_2BA50(a1x,7)`. Else if `!(byte_0x3E_62&3)` and `dword_0x10_16<14`: `EuclideanDistXY_584D0 < 2768896` → try **melee `sub_2B7E0(a1x)`**; if it returns 0 → `sub_2BA50(a1x,3)` (windup). LABEL_31: if v29, try `sub_2B7E0`; else `sub_2BA50(a1x,3)`. return.
    - **case 3 (windup)**: `sub_2BA50(a1x,4)` (sub-state 4, dw=0); `sub_2B860(a1x,2)` (sprite mode 2, sound anim); `word_0x30_48 = yaw`; sound `PrepareEventSound_6E450(idx,-1,38)` (**sound id 38**); fall to LABEL_35.
    - **case 4 / case 5 (strike)** (LABEL_35): if `dword_0x10_16<=0` → `sub_2BA50(a1x,6)` return. Restore `yaw=roll=word_0x30_48`. If sub-state==4: re-validate target; if `!(byte_0x3E_62&7)` and `EuclideanDistXY>802816` re-aim; if `word_0x2C_44-3 > dw > 3` and **`sub_1CED0(a1x, target)` succeeds → `byte_0x46_70=5`** (:21157–21158). `dword_0x10_16--`; if `!(byte_0x3E_62&3)` neighbor re-aim (:21166–21224). LABEL_58: `sub_1B8C0`; `word_0x30_48=yaw`; **turn: `yaw += (dword_0x10_16 & 4)? +56 : -56; yaw &= 0x7FF`** (swinging arc) (:21227–21235).
    - **case 6**: `sub_2B860(a1x,3)`; spawn `(10,5)`; validate target; if within `behaviorRow.word_160_0x1c_28` → `sub_2BA50(a1x,2)` (re-engage) else `sub_2BA50(a1x,7)`.
    - **case 7 (reposition)**: **RNG draw** `roll = rand & 0x7FF` (:21193–21194); `sub_2BA50(a1x,8)` (dw=16); fall to LABEL_76.
    - **case 8** (LABEL_76): `sub_1B8C0`; `dword_0x10_16--`; if <=0 → `sub_2BA50(a1x,9)`.
    - **case 9**: `sub_2B860(a1x,1)` (restore normal sprite); `actionIndex=225`; `word_0x96_150=0` (disengage).
  - **`sub_2B860(a1x, mode)` (:21308)** — sprite/state config:
    - mode 1: behavior-row `str_D7BD6[93]`, sprite **292**, ShiftRot(85,42), `actSpeed=maxSpeed`.
    - mode 2: behavior-row `str_D7BD6[93]`, `word_0x2C_44=0`, `actSpeed=minSpeed`, sprite **291**, ShiftRot(384,768), plays anim `x_BYTE_D9F50[0x5b6]` and sets `dword_0x10_16 = anim frame count`.
    - mode 3: `byte_0x39_57=0`, behavior-row `str_D7BD6[94]`, `actSpeed = minSpeed-28 (=92)`, clears sprite-visible bit.
  - **`sub_2BA50(a1x, n)` (:21416)**: `byte_0x46_70=n`; `dword_0x10_16 = 32` if n==2, `=16` if n==8, else `0`.
  - **`sub_2B7E0(a1x)` (:21273)**: scans model-28 roster for another m28 in strike sub-states (3/4/5); returns 1 if one is already striking (prevents simultaneous attacks) — a **pack-attack gate**.
  - **m28 attack payload = `sub_1CED0(a1x, target)` (:9786): if `distance_3d < 768`, `sub_11900(a1x, target, 0, subSpellIndex=2000)` → 2000 melee damage to target's mailbox.** Pure melee, no projectile.
- **227 (0xE3) = `sub_2B750` (:21244):** `actionIndex = 225` (pack/reset +3).
- **228 (0xE4) = `sub_2B760` (:21254):** `PreKillEntity_1C890(a1x, 224)` (prekill +4).
- **229 (0xE5) = `sub_2B780` (:21260):** `KillEntity_1C930(a1x)` (kill +5).
- **230 (0xE6) = `sub_2B7A0`:** *no body present.* No `0x20c7a0` case in dispatch. **OPEN — m28 action 230 (0xE6) has no handler function (empty slot).**
- **231 (0xE7) = `sub_2B7B0` (:21266):** `sub_1D5D0(a1x, 224)`; if `actionIndex==226` → `sub_2B840` (hit/flee +7).
- **`sub_2B840` (:21297):** `actionIndex=226; byte_0x46_70=0` — the "enter attack brain" transition used by the wrappers.

**PART A cross-cutting notes:** All three models spawn at base+1, run their real brain at base+2 (m28) or base+0/+2 (m25/m26), share the identical hit-mailbox absorption prologue, and are burnable (`byte_0x38_56=1`). RNG generator is always `rand = 9377*rand + 9439`.

---

## PART B — Class-2 scenery models 3–8 + tree (2,0) burn ticks

### Common class-2 ctor pattern
Each: `NewEvent_4A050`; set `class=2`; `dword_0x10_16 = (self - struct_0x6E8E) % 11` (stagger, tie-breaks update phase); `AddEventToMap`; `CopyMaxLifeToLife`; then a sprite-row setter. `struct_byte_0xc_12_15.byte[0] &= 0xF7` clears the "solid/collidable draw" bit (bit3) on some; models that omit it stay collidable.

### Model 3 — `sub_4AE80` (:33503)
- `byte[0] &= 0xF7` (:33508) — clears bit3. `dword_0x10_16 = idx%11` (:33509). `actionIndex=9` (:33510). `class=2, model=3`. `SetHalfSpeedEntity_49DA0(v1x, 270)` (:33515) — **sprite row 270**. Life = default from `CopyMaxLifeToLife` (maxLife not set here ⇒ inherits pooled default — **OPEN: maxLife not explicitly written; value comes from NewEvent init**).
- **Action 9 = `sub_65110` (:62536):** `byte[2] |= 2` (draw flag); `z = getTerrainAlt_10C40(pos)` (snap Z). Pure static prop.

### Model 4 — `sub_4AF00` (:33521)
- `actionIndex=12` (:33526); `class=2, model=4`; `dword_0x10_16=idx%11`; `SetHalfSpeedEntity_49DA0(v1x, 48)` (:33532) — **sprite row 48**. No bit-clear ⇒ stays collidable. Pure static.
- **Action 12 (0xC):** dispatch addr range `0x246130–0x2461a0` are **empty `break;` cases** (Events.cpp:3199–3207) → **no-op tick** (pure static, never updates). Confirmed.

### Model 5 — `sub_4AF70` (:33538)
- `actionIndex=15` (:33543); `class=2, model=5`; `dword_0x10_16=idx%11`; `SetHalfSpeedEntity_49DA0(v1x, 48)` (:33549) — **sprite row 48**. No bit-clear.
- **Action 15 (0xF):** also within the empty `0x246130–0x2461a0` range → **no-op tick**. Pure static.

### Model 6 — cave-bee `sub_4AFE0` (:33555) — CAVE-ONLY, burnable
- **`if MapType != Cave return 0`** (:33561) — cave-only gate.
- `actionIndex=18` (:33567); `class=2, model=6`; **RNG draw #1** (:33570) → `maxLife = rand%0x50 + 100` (100–179) (:33573). `byte_0x38_56=1` (:33572) — burnable.
- Jitter position: **RNG draw #2** (:33575) `x += (rand&0x3F)-32`; **RNG draw #3** (:33577) `y += (rand&0x3F)-32` (:33576–33578).
- `AddEventToMap` at jittered pos; `CopyMaxLifeToLife`; **RNG draw #4** (:33581) `SetHalfSpeedEntity_49DA0(v2x, (rand&3)+324)` — **sprite row 324–327** (randomized among 4). **4 RNG draws total.**
- **Action 18 (0x12) = `sub_651B0` (:62548):** on burn-hit (`word_0x62_98`): `life -= dword_0x5E_94`; if `life<0`: clear hit flag; clear draw bit `byte[0] &= 0xF7`; `actionIndex=19`; `SetHalfSpeedEntity_49DA0(a1x, word_0x5A_90 + 4)` (**sprite += 4** = death sprite); **spawn `(10,13)`** blood/gib `IfSubtypeCallCreatingManaSphere_4A190(pos,10,13)` (:62570). Always clear hit flag, snap Z. Water ⇒ `DisableEntityDrawing04_57F10` (despawn) (:62576).
- **Action 19 (0x13) = `sub_65240` (:62582):** snap Z; water ⇒ despawn. (death-idle terminal.)

### Model 7 — `sub_4B0F0` (:33587) → `sub_4B150` (:33608); Model 8 — `sub_4B120` (:33598) → `sub_4B150`
- Both **non-cave**: `sub_4B0F0`/`sub_4B120` return 0 in caves (:33590/:33601). Delegate to `sub_4B150(pos, model, action, spriteRow)`:
  - m7: `sub_4B150(pos, 7, 20, 322)` (:33593) — action **20 (0x14)**, sprite **322**.
  - m8: `sub_4B150(pos, 8, 21, 323)` (:33604) — action **21 (0x15)**, sprite **323**.
- **`sub_4B150` (:33608):** `actionIndex=a3`; `model=a2`; **RNG #1** (:33621) `maxLife = rand%0x7D0 + 400` (400–2447); `class=2`; `byte_0x38_56=1` (burnable); `byte_0x46_70=0`; **`word_0x2C_44 = -128`** (initial downward velocity for falling physics); `actSpeed=0`; jitter x/y with **RNG #2, #3** (:33630,:33632) `(rand&0x3F)-32`; `AddEventToMap`; `CopyMaxLifeToLife`; `SetHalfSpeedEntity_49DA0(v5x, a4)` (sprite row). **3 RNG draws.**
- **Actions 20/21 = `sub_652A0`/`sub_65280` (:62599/:62593) → both call `sub_652C0` (:62606) — falling physics (fully traced):**
  - Clear-and-return branch: if `byte[1] & 8` set → clear it and **return** (frozen this tick) (:62627–62631).
  - Else: predict pos. If `pos.z > terrainAlt`: if `actSpeed`, `MoveEntity_57FA0(&pred, yaw, 0, actSpeed)` (horizontal drift) (:62635–62640). Else (at/under ground): `actSpeed=0`; `sub_654B0(a1x)` (slide toward lowest adjacent terrain — anti-clip, :62704).
  - `CopyEntityPosition`. `actSpeed--` if >0. **Gravity: `word_0x2C_44 -= 24` each tick, clamped to [-192, +192]; `pos.z += word_0x2C_44`** (:62650–62657). Clamp `pos.z` up to terrain floor `v6` (:62658–62660).
  - **Burn/impact on hit-flag** (`word_0x62_98`, :62661): if grounded (`z<=v6`): compute bounce `v7 = dword_0x5E_94 >> 2`, clamp [2,192]; **RNG** `word_0x2C_44 = rand%v7 + v7` (upward kick); **RNG** `actSpeed = rand%(v8>>1) + 1`; **RNG** `yaw = rand & 0x7FF` (random bounce direction); `pos.z += word_0x2C_44` (:62665–62681). Then `life -= dword_0x5E_94`; clear hit flag (:62683–62687).
  - **Terminal states:** if `life < 0` → spawn **`(10,13)`** gib, `DisableEntityDrawing04` despawn (LABEL_27) (:62688–62691). If grounded AND on water → spawn **`(10,5)`** splash, despawn (:62693–62698). Otherwise persists (still falling/settling).
  - **Re: CONTEXT "terminal states 19/27 question":** In this decompile the falling handler does **not** transition to a distinct actionIndex 19/27; it terminates by `DisableEntityDrawing04_57F10` (despawn) at the two LABELs (life<0 → `(10,13)`; water → `(10,5)`). The "19/27" appear to be internal `LABEL_27` (goto label) and unrelated. **OPEN — no action-index 19 or 27 transition exists in `sub_652C0`; terminal behavior is despawn, not a state change.**

### TREE (2,0) — creator `AddTree_4AC40` (:33433) + 3-stage burn
**Creator `AddTree_4AC40` (:33433):** `actionIndex=0`; `class=2, model=0`; `dword_0x10_16=idx%11`; **RNG #1** (:33442) `life = rand%0x1388 + 2500` (2500–7499); `byte_0x38_56=1` (burnable); jitter x/y **RNG #2,#3** (:33445,:33447); `CopyMaxLifeToLife`; **RNG #4** (:33451) picks sprite: `rand&1` ⇒ sprite **84**, else sprite **83** (`SetHalfSpeedEntity_49DA0`) — the two tree variants. **4 RNG draws.**

- **Action 0 = `AddTree02_00_64E20` (:62399) — healthy tree, burn-hit check:**
  - `byte[2] |= 2` (draw flag) (:62414).
  - If `word_0x62_98` (burn-hit pending) (:62415):
    - `life -= dword_0x5E_94` (:62417). If `life < 0` (tree dies → becomes burning tree):
      - Spawn **`(10,6)`** `IfSubtypeCallCreatingManaSphere_4A190(pos,10,6)` = fire/flame effect (:62421); set its id from the attacker (`Entities[word_0x62_98].id`); `word_0x2C_44 = (3*array_0x52_82.fov) * 9/16` approx (the `>>2` of `(v6 - (…))` ⇒ effectively `(3*fov*3)>>? ` — exact: `v6 = 3*fov; word_0x2C_44 = (v6 - (v6>>4*... )) >> 2`, i.e. `(3*fov)*13/16 >> ...`; **the literal expression is `(int)(3*fov - ((HIDWORD(3*fov)<<2)+4*HIDWORD(3*fov))) >> 2`** — for positive fov this is `(3*fov)>>2`); set flame's z = `treeZ - 128` (or 0 if treeZ≤128) (:62427–62433).
      - **Re-seed life: RNG** `life = rand%0x3C + 130` (130–189) — burning duration; write to both flame entity and tree (:62434–62438).
      - `struct_byte_0xc_12_15.dword &= 0xFFFDFFF7` (clear bits), then `actionIndex=1` (burning), `byte[2] |= 2` (:62439–62442); `sub_57D40(a1x, pos)` (re-register).
    - Clear `word_0x62_98` (:62446).
  - Snap Z (:62448). If on water → spawn **`(10,5)`** splash, set id, `DisableEntityDrawing04` despawn (:62450–62456).
- **Action 1 = `sub_64F60` (:62462) — burning tree tick:**
  - `life -= 1` (:62469). If `life < 60`: `actionIndex=2` (charred); sprite swap based on `word_0x5A_90`: if `== 0x53 (83)` → sprite **226**; if `== 84` → sprite **227** (`SetHalfSpeedEntity_49DA0`) — charred-stump sprites (:62471–62486).
  - Snap Z; water → spawn `(10,5)`, despawn (:62487–62495).
- **Action 2 = `sub_64FF0` (:62500) — charred stump terminal:**
  - Snap Z; water → spawn `(10,5)`, despawn. No life logic — permanent stump (:62505–62514).

### How fire damage reaches a tree (burn-flag writer)
The `word_0x62_98` / `dword_0x5E_94` mailbox is written by the **area-damage functions `sub_11400` (:4207) and `sub_116A0` (:4305)** (NOT `sub_10C80` as the CONTEXT hypothesized — **correction/OPEN**). Mechanism (`sub_11400`, element index `a2`):
- When `a2 == 0` (fire element): iterate **all class-2/model-2 entities** and every entity in the affected map tiles where **`(1 << a2) & byte_0x38_56`** is set (bit 0 ⇒ flammable — exactly the bit every tree/scenery ctor sets via `byte_0x38_56=1`) (:4227, :4264, :4351).
- For a qualifying target: `if (word_0x62_98) dword_0x5E_94 += a3; else dword_0x5E_94 = a3;` then `word_0x62_98 = a1x->id_0x1A_26` (attacker id) (:4275–4289). Trees (class 2, model 0) take **full** `a3` damage; class-2 model-2 (dolmen) takes `a3/10` (:4273–4287).
- Element/class filtering via `xtype_0x41_65`/`xsubtype_0x42_66` (:4268–4271). `sub_116A0` is the identical structure but also sets `word_0x30_48 = 30` on hit model-2 entities and counts hits (:4323).
- The melee/single-target equivalent is `sub_11900` (:4375): writes `dword_0x5E_94`/`word_0x62_98` to one entity (used by m25/m28 attacks and the melee helpers `sub_1CE80/1CED0/1CF20`).

**So the damage→burn chain is:** a fire spell/area effect calls `sub_11400(attacker, 0, dmg)` → sets tree's `word_0x62_98`+`dword_0x5E_94` → next tick `AddTree02_00_64E20` reads the flag, subtracts damage, and if lethal spawns flame `(10,6)`, re-seeds 130–189 life, and advances to burning (action 1) → charred (action 2).

---

## OPEN / uncertain items
1. **m25 action 206 (0xCE) = `sub_28F40`**, **m26 action 214 (0xD6) = `sub_29370`**, **m28 action 230 (0xE6) = `sub_2B7A0`** — none of these three functions exist in the decompile and none appear as `case` labels in the Events.cpp dispatch. They are empty/unused action slots (the +6 role in the roster). Flag for the port as no-op or verify against the original binary's jump table.
2. **`sub_10C80`** named in CONTEXT as the burn-flag writer is **not** the correct symbol; the actual writers are `sub_11400` (:4207), `sub_116A0` (:4305), and single-target `sub_11900` (:4375).
3. **`sub_652C0` "terminal states 19/27":** no `actionIndex` 19 or 27 transition occurs; the handler despawns via `DisableEntityDrawing04_57F10`. "27" is the internal `LABEL_27` goto. Re-confirm expected behavior.
4. **Model 3 (`sub_4AE80`) maxLife** is never explicitly written; `CopyMaxLifeToLife_49A20` copies whatever `NewEvent_4A050` initialized (likely 0 or pooled default). Verify the intended tree/prop HP.
5. `word_0x2C_44` re-seed formula in `AddTree02_00_64E20` (:62429) is an obfuscated `>>2`-of-`3*fov` expression; for the port treat as `word_0x2C_44 = (3 * array_0x52_82.fov) >> 2` for non-negative fov (matches the compiler's signed-div-by-4 idiom), but confirm sign handling.