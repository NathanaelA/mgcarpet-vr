The prompt's given addresses (24830, 24DE0, 25540, 25D40, 26010) don't appear in the actual dispatch — the real handlers per role are what I traced. The prompt gave nominal +N addresses; actual decompile has the kill-alt/spawn at the addresses in the dispatch. I'll map by role and note discrepancies. Let me verify m16's full state set by checking what handles states 0x80-0x87 (128-135). m16 base=128, actionIndex=129. Handlers: 24420(+0 patrol/128), 24440(+1 idle/129), 24510(+2 attack/130), 247D0(+3 pack/131), 247F0(+4 prekill/132), 24810(+5 kill/133), 24840(+? spawn), 24400(+?). 

Note 24400 = sub_1D5D0(a1x,120) — that's a DIFFERENT base (120), likely not m16. And the prompt's m16 +6/+7 = 24830/24840. Only 24840 exists (sub_1D5D0 spawn, base 128). Let me check 247F0 prekill uses base 128 → transitions +5=133 kill=24810. There's no explicit +6 hit handler for m16 in dispatch (24830 absent) — m16 has no hit/flee state (boss). Let me confirm by checking m16 idle routing: sub_1BF90... wait, m16 idle is sub_24440 which calls sub_1BF90(a1x,128) → routes to 128+2=130 (attack) since row 84 byte_0x20&8=0. And +6 would be 134 but no handler. Since row84 byte&8=0, +6 never reached. Good.

I have all data. Let me verify m17's `str_D7BD6[87]` (row 87, line 99) used in chase-swap, and m18 attack sub_1D460 spawn, and confirm sub_25E40 (m20 attack) uses sub_1D1A0. Everything is confirmed. Let me write the report.

Now compiling the complete report.

---

# MC2 Class-5 Models 16–20 — Verbatim Trace

**Global notes (apply to all five):**
- Handler symbol rule confirmed against `Events.cpp` dispatch (`case 0x2054xx → sub_24xxx`), file `reference/remc2/remc2/engine/Events.cpp:1632–1794`. Dispatch case value = handler VA (e.g. `0x205420` ⇒ `sub_24420`), and `sub_XXXXX` = VA − 0x1E1000.
- **RNG LCG** everywhere: `rand = 9377*rand + 9439` (`rand_0x14_20`). Each occurrence below is one draw.
- **Behavior-row struct** `type_str_160` (34 bytes), fields at `global_types.h:75–94`: `word_160_0x1a_26` (attack-cadence modulus / `word_0x1a`), `word_160_0x1c_28` (engage/acquire range `word_0x1c`), `word_160_0x1e_30` (FOV cone `word_0x1e`), `byte_160_0x20_32` (flag byte). Rows live in `Level.cpp:11` `str_D7BD6[157]`; **row N is at source line 12+N** (line 12 = row 0).
- **`byte_160_0x20_32 & 8`**: in shared idle/patrol/pack (`sub_1BD90` `EventsFunctions.cpp:8998–9017`, `sub_1BF90` :9170–9173/:9227–9231, `sub_1C560` :9496–9524) this flag routes target-acquire to state **+6** instead of **+2**. `&4` disables pack-follow acquire; neither is a day/night gate. **No day/night/cave gate exists in any of these five ctors or handlers.**
- **`sub_1B8C0`** (move core, `:8741`) hugs terrain via `getTerrainAlt_10C40` + behavior-row z-envelope `word_160_0xa/0xc/0xe`. Handlers that instead write `position.z` directly are flying/hovering.
- **`SetEntityShiftRot_49EA0(e,shift,fov)`** (`:32874`): sets `array_0x52_82.pitch = .roll = shift`, `.fov = fov`. Called AFTER `SetEntityIndexAndRot_49CD0` (`:32838`, which pre-seeds pitch/roll/fov from `particlesParameters_D951C[idx]/2`), so the ShiftRot args are final. `.pitch`/`.roll` = collision half-extent (used in flock-spacing `abs(dx)<pitch`); `.fov` = projectile spawn z-lift.
- **Attack primitives** (all in `EventsFunctions.cpp`):
  - `IfSubtypeCallCreatingManaSphere_4A190(pos, 9, N)` (`Events.cpp:5186`) spawns a **class-9 projectile** of subtype N from spell-def table `str_D4C48ar[9].dword_14[N]` (returns NULL if that slot empty). `(…,10,1)` on death spawns a class-10 corpse/manasphere.
  - `sub_11900(atk,tgt,0,dmg)` (`:4375`) = **melee mailbox write**: accumulates `dmg` into target's `str_0x5E_94.dword_0x5E_94` and stamps attacker id into `word_0x62_98`. `dmg` = attacker's `subSpellIndex_0x2A_42`.
  - `sub_1CE80` (`:9772`): if dist3d < **1024**, melee `sub_11900(...,subSpellIndex)`, return 1.
  - `sub_1CED0` (`:9786`): if dist3d < **768**, melee `sub_11900(...,subSpellIndex)`, return 1.
  - `sub_1CC20` (`:9680`): spawn class-9 **subtype 0**, `byte_0x43_67=10,byte_0x44_68=0`, aim yaw/pitch at target, `z+=array_0x52_82.fov`, row=`str_D7BD6[65]`, **subSpell=500**, copy target class/model, `sub_5EF70(target)`.
  - `sub_1D0E0` (`:9814`): class-9 **subtype 20**, `byte_0x44_68=65`, `z+=fov`, row `str_D7BD6[65]`, **subSpell=780**.
  - `sub_1D1A0` (`:9847`): class-9 **subtype 21**, `byte_0x44_68=66`, `z = z+128`, row `str_D7BD6[65]`, **subSpell=780**.
  - `sub_1D460` (`:9918`): **5-shot fan** (yaw offsets −226,−113,0,+113,+226), each class-9 **subtype 0**, `byte_0x44_68=0`, `z=z+200`, row `str_D7BD6[61]`, **subSpell=800**.

---

## MODEL 16 — Boss (60000 life). CTOR `sub_4C310` (`EventsFunctions.cpp:34163`)

**Ctor field writes, in order** (`:34165–34196`):
1. `NewEvent_4A050()`; guard `if(v1x)`.
2. `actionIndex_0x45_69 = 129` (base 0x80=128, starts at +1 idle).
3. `class_0x3F_63 = 5`
4. `model_0x40_64 = 16`
5. `minSpeed_0x84_132 = 60`
6. `maxSpeed_0x86_134 = 20`
7. `maxLife_0x4 = 60000`
8. `actSpeed_0x82_130 = minSpeed(60)`
9. `SetEvent144_49C70(v1x)` (mana init helper; no numeric mana literal here)
10. **RNG draw #1** `:34176`. Then `roll_0x20_32 = (rand&0x7FF)-1`, `yaw_0x1C_28 = (rand&0x7FF)-1`, `pitch_0x1E_30 = roll`. (**One draw; three fields all derive from it.**)
11. `fov_0x22_34 = 0`
12. `subSpellIndex_0x2A_42 = 500`
13. `byte_0x38_56 = 1`
14. `dword_0x10_16 = (index)%100` — **immediately overwritten to 0** at `:34187`.
15. `dword_0xA0_160x = &str_D7BD6[84]` (**behavior row 84**, `Level.cpp` line 96: `word_0x1a=0x28=40`, `word_0x1c=0x1200=4608`, `word_0x1e=0x0200=512`, `byte_0x20=0x11` ⇒ `&8=0`).
16. `byte_0x39_57 = 64` (wake/scan enable)
17. `xtype_0x41_65 = 3`
18. `dword_0x10_16 = 0`
19. `byte_0x3E_62 = D41A0_0.array_0x10[16]++` (per-model instance counter → per-instance phase)
20. `AddEventToMap_57D70`; `CopyMaxLifeToLife_49A20` (life=60000).
21. `SetEntityIndexAndRot_49CD0(v1x, 207)` — **sprite/particle row 207**.
22. `array_0x52_82.yaw = (5*word[D9F50+294] …)>>3` (a scaled global; verbatim expression `:34192–34194`).
23. `SetEntityShiftRot_49EA0(v1x, 128, 128)` ⇒ pitch=roll=128, fov=128.

- **No `byte_0x3A_58` write.** No sound calls in ctor. No map-type gate, no early despawn.

**State handlers (base 128 = 0x80):**
- **+0 patrol 128 `sub_24420`** (`:15333`): thin wrapper `sub_1BD90(a1x, 128)`. No deviations.
- **+1 idle/wake-scan 129 `sub_24440`** (`:15339`) — **model-specific**: calls `sub_1BF90(a1x,128)` (wander+wizard-scan), THEN if `actionIndex==129` and `byte_0x3E_62 % (word_0x1a+1)==0`: scan all entities in `D41A0.dword_38527` list for nearest within `word_0x1c²` (4608²); if found set `actionIndex=130` (attack) and `word_0x96_150 = target index`. No RNG, no sound. (This is a second, wider target acquire on top of `sub_1BF90`'s cone scan.)
- **+2 attack/chase 130 `sub_24510`** (`:15389`) — **model-specific, full trace**:
  - Reads engage range² `v13 = word_0x1c²`.
  - Damage-inbox drain: if `str_0x5E_94.word_0x62_98` set, `life -= dword_0x5E_94`; sets `word_0x26_38`, `v3=1`. Follow child chain `word_0x34_52` for a lower-life child (`v3=1`). If `life<0` ⇒ `v3=2`, `word_0x24_36=word_0x26_38`.
  - If `v3==0` (alive, no new damage): `sub_1B8C0` move; `sub_1ED30(self, target)` LOS/resolve. If target valid:
    - Every 8 ticks (`!(byte_0x3E_62 & 7)`): if `class==3` OR dist3d≥0x200, face target (`roll_0x20_32 = tan2`).
    - If target dead/flagged ⇒ `actionIndex=129`.
    - Else if `dword_0x10_16>0`: decrement, and **launch a class-9 projectile** via `IfSubtypeCallCreatingManaSphere_4A190(pos, 9, 0)` (`:15474`): sets `byte_0x43_67=10, byte_0x44_68=0`, row `str_D7BD6[61]`, `xsubtype=target.model`, `xtype=target.class`, `id=self.id`, yaw=`tan2(pos,tgt)`, pitch=`radix_tan`, `z += 6*array_0x52_82.fov` (= 6*128), **`subSpellIndex_0x2A_42 = 1600`**, **`mana_0x90_144 = 50000`**, `word_0x96_150 = self.word_0x96_150`. (This is m16's homing bolt.)
    - Cadence gate `byte_0x3E_62 % word_0x1a == 0` (:15494): compute planar dist²; if `< v13`: if `byte_0x3E_62 % (2*word_0x1a)==0` play **sound 39** (`PrepareEventSound_6E450(idx,-1,39)`); if angle-to-target `sub_582B0(yaw, tan2) < 0xE3`: set `dword_0x10_16 = 15` (arms the 15-tick bolt burst above) and `sub_5EF70(target)`. Else (out of range) ⇒ `actionIndex=129`.
  - `v3==1` ⇒ retarget `word_0x96_150 = word_0x26_38`. `v3==2` ⇒ `actionIndex=132` (prekill). **No RNG draws in this handler.**
- **+3 pack 131 `sub_247D0`** (`:15536`): thin `sub_1C560(a1x, 0x80)`. No deviations.
- **+4 prekill 132 `sub_247F0`** (`:15542`): thin `PreKillEntity_1C890(a1x, 128)` (propagates children to state+5, credits kill).
- **+5 kill 133 `sub_24810`** (`:15548`): thin `KillEntity_1C930(a1x)` (transform→manasphere, spawn class-10 subtype1 corpse if not flagged).
- **+7 spawn hook `sub_24840`** (`:15554`): thin `sub_1D5D0(a1x, 128)` (StageVar2-dispatched add hook).
- **+6 hit/flee: NO HANDLER** in dispatch (nominal 0x24830 absent; `Events.cpp` has no 0x205830 case). Because row-84 `byte_0x20 & 8 == 0`, acquire always routes to +2, so +6 is unreachable — OPEN but consistent (boss has no flee state).

**Sound ids:** 39 (attack, gated by `%word_0x1a` then `%(2·word_0x1a)`). No others.

---

## MODEL 17 — subSpell 350, 10000 life. CTOR `sub_4C460` (`:34201`)

**Ctor, in order** (`:34203–34231`):
1. `NewEvent_4A050`; guard.
2. `actionIndex_0x45_69 = 137` (base 0x88=136, +1 idle).
3. `class=5`, `model=17`.
4. `minSpeed_0x84_132 = 68`, `maxSpeed_0x86_134 = 20`, `maxLife = 10000`, `actSpeed = minSpeed(68)`.
5. `SetEvent144_49C70`.
6. **RNG draw #1** `:34214` → `roll=(rand&0x7FF)-1`, `yaw=(rand&0x7FF)-1`, `pitch=roll`. (**One draw.**)
7. `fov_0x22_34 = 0`.
8. `dword_0x10_16 = index%100` (kept; not re-zeroed).
9. `subSpellIndex_0x2A_42 = 350`.
10. `byte_0x38_56 = 1`.
11. `dword_0xA0_160x = &str_D7BD6[85]` (**row 85**, line 97: `word_0x1a=0x14=20`, `word_0x1c=0x0F00=3840`, `word_0x1e=0x0200`, `byte_0x20=0x01` ⇒ `&8=0`).
12. `byte_0x39_57 = 64`. `xtype_0x41_65 = 3`. `dword_0x10_16 = 0` (overwrites step 8 → 0).
13. `byte_0x3E_62 = array_0x10[17]++`.
14. `AddEventToMap`; `CopyMaxLifeToLife` (10000).
15. `SetEntityIndexAndRot_49CD0(v1x, 285)` — **sprite row 285**.
16. `SetEntityShiftRot_49EA0(v1x, 128, 128)` ⇒ pitch=roll=128, fov=128.
- No `byte_0x3A_58`, no ctor sound, no map gate.

**State handlers (base 136 = 0x88):**
- **+0 patrol 136 `sub_24860`** (`:15560`): `sub_1BD90(a1x,136)`; **deviation**: if resulting `actionIndex==138`, validate `word_0x96_150` target is class-3 model 0/1 (else clear it) and `byte_0x46_70 = 0`.
- **+1 idle 137 `sub_248C0`** (`:15579`): `sub_1BF90(a1x,136)`; same +138 validation/`byte_0x46_70=0` deviation.
- **+2 attack/chase 138 `sub_24930`** (`:15596`) — **model-specific, full trace** (this is a dive-bomber):
  - `PrepareEventSound_6E450(idx,-1,58)` **every tick** (idle-loop sound **58**).
  - Damage-inbox drain (same v1 pattern): `v1=1` on hit / child; `life<0` ⇒ `v1=2`. `v1==2` ⇒ `actionIndex=140` (prekill). `v1==1` ⇒ `word_0x96_150=word_0x26_38`.
  - `v13 = sub_1B8C0(a1x)` (move; returns 3 when blocked/redirected). `sub_1ED30` resolve target `v15x`. If target invalid/dead ⇒ reset: row=`str_D7BD6[85]`, `word_0x96_150=0`, `actionIndex=137`, `actSpeed=minSpeed`.
  - Else: every 4 ticks (`!(byte_0x3E_62 & 3)`), when `byte_0x46_70∈{0,4}`, face target and do flock-spacing scan over `bytearray_38403x[17]` (turn away from crowded same-model neighbor within `array_0x52_82.pitch`).
  - **`switch(byte_0x46_70)`**:
    - **case 0** (approach): every `%word_0x1a` tick, `d=dist3d`; if `d≥word_0x1c(3840)` ⇒ `actionIndex=137` (disengage); elif `d≥0x700(1792)` ⇒ **`sub_1D0E0(self,target)`** = class-9 **subtype 20**, subSpell **780** (long-range lob); else ⇒ `byte_0x46_70=1` (commit dive).
    - **case 1** (begin dive): face target (`roll=yaw=tan2`); `actSpeed = 3*minSpeed(=204)`; row=`str_D7BD6[87]` (line 99: dive row, `word_0x1c=0x0F00`, `byte_0x20=0x01`); `dword_0x10_16=0`; `byte_0x46_70=2`.
    - **case 2/3** (diving): if `v13!=3` keep heading (`yaw=roll`). Vertical ballistic: `dword_0x10_16` indexes a `192>>n` fall curve (ascending then `-192>>` descending, clamp −192) `:15730–15744`; `dword_0x10_16++`; decel `actSpeed-=8` down to maxSpeed(20). Predict z; if next z ≤ terrain alt ⇒ `byte_0x46_70=4`, `dword_0x10_16=18` (pull-up/impact). Else apply `z += v14`; in case 2 only, `sub_1CED0(self,target)` melee (dist<768, dmg=subSpell 350) — on hit ⇒ `byte_0x46_70=3`.
    - **case 4** (recover): **ORDER MATTERS (errata 2026-07-17):** retail reads the OLD `dword_0x10_16`, THEN decrements, and compares the OLD value (`v1 = ctr; ctr = v1-1; if (v1) { if (v1==18) …restore row 85… } else …`, EF:15771-88) — so row `str_D7BD6[85]` + `actSpeed=maxSpeed(20)` restore on the FIRST recover tick, and the `else` (old==0) arm ⇒ `byte_0x46_70=0`, `actSpeed=minSpeed(68)`. The port's decrement-first transcription made the `==18` row-85 restore unreachable — the leaper stayed on dive row 87 (v_14=0, no ground-follow) forever after its first leap and ran on air at the chase altitude (player 2026-07-17). Fixed in `m17_tick`.
  - **RNG draws in +2: none.** Sounds: **58** (per-tick).
- **+3 pack 139 `sub_24D40`** (`:15803`): `sub_1C560(a1x,0x88)`; deviation: if `actionIndex==138` validate class-3 model 0/1 target, `byte_0x46_70=0`.
- **+4 prekill 140 `sub_24DA0`** (`:15821`): thin `PreKillEntity_1C890(a1x,136)`.
- **+5 kill 141 `sub_24DC0`** (`:15827`): thin `KillEntity_1C930`.
- **+7 spawn/add hook `sub_24DF0`** (`:15833`): `sub_1D5D0(a1x,136)`; deviation: if `actionIndex==138`, `byte_0x46_70=0`.
- **+6 hit/flee:** unreachable (row-85 `&8=0`); nominal 0x24DE0 absent from dispatch. OPEN/consistent.

**Attack linkage:** long-range = class-9 subtype 20 (subSpell 780) via `sub_1D0E0`; close = melee `sub_11900` dmg=350 (subSpell) via `sub_1CED0` (<768). **Sounds:** 58.

---

## MODEL 18 — Slow tank, 36000 life. CTOR `sub_4C590` (`:34236`)

**Ctor, in order** (`:34238–34265`):
1. `NewEvent`; guard.
2. `actionIndex_0x45_69 = -109` (= **0x93 = 147** as unsigned byte; base 0x90=144, +3). **Note: starts at +3, not +1.**
3. `class=5`, `model=18`.
4. `minSpeed = 10`, `maxSpeed = 6`, `maxLife = 36000`, `actSpeed = minSpeed(10)`.
5. `SetEvent144_49C70`.
6. **RNG draw #1** `:34249` → `roll=(rand&0x7FF)-1`, `yaw=(rand&0x7FF)-1`, `pitch=roll`. (**One draw.**)
7. `fov_0x22_34=0`; `dword_0x10_16 = index%100`.
8. `subSpellIndex_0x2A_42 = 500`; `byte_0x38_56 = 1`.
9. `dword_0xA0_160x = &str_D7BD6[86]` (**row 86**, line 98: `word_0x1a=0x04=4`, `word_0x1c=0x1900=6400`, `word_0x1e=0x0200`, `byte_0x20=0x07` ⇒ `&8=0`).
10. `byte_0x39_57 = 64`; `xtype_0x41_65 = 3`.
11. `byte_0x3E_62 = array_0x10[18]++`.
12. `dword_0x10_16 = 100` (overwrites step 7 → **100**).
13. `AddEventToMap`; `CopyMaxLifeToLife` (36000).
14. `SetEntityIndexAndRot_49CD0(v1x, 286)` — **sprite row 286**.
15. `SetEntityShiftRot_49EA0(v1x, 512, 512)` ⇒ pitch=roll=512 (big collision box), fov=512.
- No `byte_0x3A_58`, no ctor sound/gate.

**Shared sub-routine `sub_252E0`** (`:16092`, used by all m18 non-spawn states): pins entity to ground (`position.z = getTerrainAlt`), then the standard damage-inbox drain; returns 0 alive / 1 new-target / 2 dead (2 ⇒ `actionIndex=148` prekill). **Ground-locking every tick ⇒ m18 does NOT fly.**

**Shared `sub_253B0`** (`:16155`) is the m18 state-transition/timer setter (arms `dword_0x10_16` with RNG-jittered durations; **each branch draws RNG once** — e.g. `%400+400`, `%0x190+400`, `%200+200`, or fixed 10/12/14). **Shared `sub_254E0`** (`:16232`) = turn-toward-target by `a3` step. These count as m18 primitives; RNG draws happen inside `sub_253B0`.

**State handlers (base 144 = 0x90):**
- **+0 patrol 144 `sub_24E20`** (`:15841`) — **model-specific**: `r=sub_252E0`. If `r==1` ⇒ `sub_253B0(a1x,2,0)`. If `r==0`: switch `byte_0x46_70`:
  - `!=0` path (`==1` & has target): `d=EuclideanDist`; if `d<word_0x1c(6400)`: `sub_254E0(...,4)` face; **RNG draw** `:15876`, if `rand%0x31==0` ⇒ `sub_253B0(a1x,2,0)`. Else drop target, `sub_253B0(a1x,0,0)`.
  - `==0` path (roam timer): `dword_0x10_16--`; if still >0 and `byte_0x39_57` set: **RNG draw** `:15896`; if `rand&1==0` scan `dword_38519` list for nearest visible non-flagged class-3 within `word_0x1c²` and inside FOV `word_0x1e` ⇒ set `word_0x96_150`, `sub_253B0(a1x,0,1)`. If timer hit 0 ⇒ `sub_253B0(a1x,1,0)`.
- **+1 idle 145 `sub_25050`** (`:15952`) — model-specific: `r=sub_252E0`; `r==1` ⇒ `sub_253B0(a1x,0,1)`; `r==0` ⇒ clear target, `sub_1B8C0` move, `dword_0x10_16--`, at ≤0 ⇒ `sub_253B0(a1x,0,0)`.
- **+2 attack/chase 146 `sub_250B0`** (`:15976`) — **model-specific, full trace**; `v2=sub_252E0`; if `v2≤1` switch `byte_0x46_70`:
  - **case 0**: `sub_254E0(...,4)` face; if `v2==1`: `dword_0x10_16 -= 47`, if <0 ⇒ `sub_253B0(a1x,2,1)`. Else **RNG draw** `:16005`; if `rand%0x29!=0`: `dword_0x10_16--`, if<0 ⇒ `sub_253B0(a1x,2,2)`; else ⇒ `sub_253B0(a1x,2,1)`.
  - **case 1** (wind-up→strike): `dword_0x10_16--`, ≤0 ⇒ `sub_253B0(a1x,2,2)`. Else every `%word_0x1a(4)`: resolve target; if dead ⇒ `sub_253B0(a1x,2,2)`; else face + `yaw += sub_58350(yaw,roll,5,0x400)` (turn), `yaw &= 0x7ff`, then **`sub_1D460(self,target)`** = **5-shot class-9 fan** (subtype 0, subSpell **800**, z+200).
  - **case 2**: `dword_0x10_16--`, ≤0 ⇒ `sub_253B0(a1x,2,3)`.
  - **case 3**: `dword_0x10_16--`; <0 ⇒ `sub_253B0(a1x,1,0)`; elif ≥8 ⇒ spin `yaw += 170; yaw&=0x7ff; roll=yaw`.
- **+3 pack `sub_25280`** (`:16074`): thin `sub_253B0(a1x,0,0)` (m18's "pack" slot just re-enters roam; **no `sub_1C560`** — deviation from the generic pack primitive).
- **+4 prekill 148 `sub_252A0`** (`:16080`): thin `PreKillEntity_1C890(a1x,144)`.
- **+5 kill 149 `sub_252C0`** (`:16086`): thin `KillEntity_1C930`.
- **+7 spawn hook 150 `sub_25550`** (`:16247`): `sub_1D5D0(a1x,144)`, then `position.z = getTerrainAlt` (ground-lock), and if `actionIndex==146` ⇒ `sub_253B0(a1x,2,0)`.
- **+6 hit/flee:** unreachable (row-86 `&8=0`); nominal 0x25540 absent. OPEN/consistent.

**Attack linkage:** 5-shot class-9 fan, subtype 0, **subSpell 800** (via `sub_1D460`; NOT self subSpell 500). **Sounds:** none found in m18 handlers (OPEN — no `PrepareEventSound` in 24E20/25050/250B0/253B0).

---

## MODEL 19 — Fast/fragile flyer (600 life), level-000 final-wave. CTOR `sub_4C6B0` (`:34271`)

**Ctor, in order** (`:34273–34301`):
1. `NewEvent`; guard.
2. `actionIndex_0x45_69 = 0x99 = 153` (base 0x98=152, +1 idle).
3. `class=5`, `model=19`.
4. `minSpeed = 76`, `maxSpeed = 8`, `maxLife = 600`, `actSpeed = minSpeed(76)`.
5. `SetEvent144_49C70`.
6. **RNG draw #1** `:34284` → `roll=(rand&0x7FF)-1`, `yaw=(rand&0x7FF)-1`, `pitch=roll`. (**One draw.**)
7. `fov_0x22_34 = 0`; `subSpellIndex_0x2A_42 = 300`; `xtype_0x41_65 = 3`; `dword_0x10_16 = index%100`; `xsubtype_0x42_66 = 0`; `byte_0x38_56 = 1`.
8. `dword_0xA0_160x = &str_D7BD6[88]` (**row 88**, `Level.cpp` line 100: `type_0x0=0x1D`, `word_0x1a=0x23=35`, `word_0x1c=0x1400=5120`, `word_0x1e=0x02AA`, `byte_0x20=0x01` ⇒ `&8=0`; row bytes `{0x00,0x06}` and z-envelope `word_0xa/0xc/0xe = 0x0700,0x0033,0xFFF8` differ from ground rows).
9. `byte_0x3E_62 = array_0x10[19]++`; `xtype_0x41_65 = 3` (again).
10. **`byte_0x39_57 = word_0x1a − (byte_0x3E_62 % word_0x1a) + 4`** — **model-specific wake init**: `byte_0x39_57 = 35 − (instance%35) + 4`, i.e. a staggered per-instance wake counter (unlike the flat `=64` of m16/17/18/20). No extra RNG.
11. `AddEventToMap`; `CopyMaxLifeToLife` (600).
12. `SetEntityIndexAndRot_49CD0(v1x, 287)` — **sprite row 287**.
13. `SetEntityShiftRot_49EA0(v1x, 85, 51)` ⇒ pitch=roll=**85** (small box), fov=**51**.
- **No `dword_0x10_16` re-zero** (keeps `index%100`). No `byte_0x3A_58`, no ctor sound/gate. **The survey's "class-9-in-ctor" claim is REFUTED: the ctor spawns nothing — no `IfSubtypeCallCreatingManaSphere` call anywhere in `sub_4C6B0`.**

**State handlers (base 152 = 0x98):**
- **+0 patrol 152 `sub_25590`** (`:16261`): `sub_1BD90(a1x,152)`; deviation: if `actionIndex==154` ⇒ `byte_0x46_70=0`.
- **+1 idle 153 `sub_255C0`** (`:16273`): `sub_1BF90(a1x,152)`; deviation: if `actionIndex==154` ⇒ `byte_0x46_70=0`. (Row `&8=0` ⇒ acquire routes to **+2=154**.)
- **+2 attack/chase 154 `HitFirebug_25610`** (`:16281`) — **THE flying attack handler, model-specific, full trace.** Despite the "Hit" name it is the +2 engage state (dispatch `0x206610`, `Events.cpp:1745`). Damage-inbox drain first (`v1`): `v1==1` ⇒ `byte_0x46_70=7, word_0x96_150=word_0x26_38`; `v1==2` ⇒ `actionIndex=156` (prekill). If `v1==0`:
  - `sub_1B8C0` move; `sub_1ED30` resolve `v34x`. If dead/invalid ⇒ `LABEL_92`: `actionIndex=153`, `actSpeed=minSpeed`.
  - **`switch(byte_0x46_70)`** (attack-run state machine):
    - **case 0**: `actSpeed=minSpeed(76)`, `byte_0x46_70=1`, fall through.
    - **case 1** (approach flank point): predict `predictedAxis = target.pos`; **RNG draw** `:16379`; `MoveEntity_57FA0(&pred, (target.yaw − 256 + (rand%0x5A<<11)/360)&0x7FF, 0, 2048)` (a random-jittered flank position 2048 ahead of target's facing). If `dist3d(self,pred) ≤ 0x500(1280)` ⇒ `byte_0x46_70=2`. Else face `pred` (`roll=tan2`); every 4 ticks flock-spacing turn-away over `bytearray_38403x[19]`.
    - **case 2** (climb decision): `actSpeed=maxSpeed(8)`; **RNG draw** `:16411`; `dword_0x10_16 = (rand&0x3FF) + target.z` (**sets a target hover altitude** above the target). `byte_0x46_70=3`, fall to LABEL_37.
    - **case 3** (**hover/strafe over target — the flight core**): face target; every 4 ticks flock-spacing. Predict point 2048 ahead of target's yaw; if `dist3d>0x500` ⇒ `byte_0x46_70=0` (restart run). Else **RNG draw** `:16447`; `v16 = rand%0x11F`; sub-mode rolls: `!(v16&0x3F)`⇒`byte_0x46_70=6`; `!(v16&0x1F)`⇒`=7`; `!v16`⇒`=4`; and `!(v16&3)` ⇒ **vertical bob**: `position.z += (z≤dword_0x10_16 ? +64 : −64)` (climbs toward the RNG hover altitude). **This direct `position.z += ±64` with no terrain-lock is the definitive flying evidence.**
    - **case 4→5** (strafe-fire pass): case 4 sets `actSpeed=minSpeed`, `byte_0x46_70=5`. case 5: every 4 ticks flock-spacing; then LABEL_89: every `%word_0x1a(35)` tick, if `dist3d < word_0x1c(5120)` ⇒ **`sub_1CC20(self,target)`** = launch **class-9 subtype 0**, subSpell **500**, `z+=fov` (the firebug's ranged bolt). Else `byte_0x46_70=6`.
    - **case 6**: ⇒ LABEL_92 (break off, `actionIndex=153`).
    - **case 7** (dive-strike w/ sound): **RNG draw** `:16507`; `actSpeed=3*minSpeed(228)`; `sound = (rand&1)+43` (**sound 43 or 44**) via `PrepareEventSound_6E450(idx,-1,v19)`; `dword_0x10_16=24`, `byte_0x46_70=8`, fall to LABEL_59.
    - **case 8/9** (diving melee): `dword_0x10_16--`, at 0 ⇒ `byte_0x46_70=0`. Face target if `dword_0x10_16>16`; flock-spacing. LABEL_70: vertical approach `z += clamp(target.z − z, ±64)`; in case 8, `sub_1CED0(self,target)` melee (dist<768, dmg=subSpell **300**) — on hit ⇒ `byte_0x46_70=9`. Then every `%word_0x1a`: if `dist3d ≥ word_0x1c` ⇒ `byte_0x46_70=6`.
  - `v1==1` branch (took damage) ⇒ `byte_0x46_70=7` (flip straight into a dive-strike). `v1==2` ⇒ `actionIndex=156`.
- **+3 pack 155 `sub_25CD0`** (`:16582`): `sub_1C560(a1x,0x98)`; deviation: if `actionIndex==154` ⇒ `byte_0x46_70=0`.
- **+4 prekill 156 `sub_25D00`** (`:16593`): thin `PreKillEntity_1C890(a1x,152)`.
- **+5 kill 157 `sub_25D20`** (`:16599`): thin `KillEntity_1C930`.
- **+7 spawn hook `AddFirebug05_13_25D50`** (`:16605`): `sub_1D5D0(a1x,152)`; deviation: if `actionIndex==154` ⇒ `byte_0x46_70=0`.
- **+6 hit/flee:** unreachable (row-88 `&8=0`); nominal 0x25D40 absent from dispatch. The hit behavior is instead folded into +2 case 7 (see `v1==1`). OPEN/consistent.

**MODEL 19 VERDICT (special question):** **m19 DOES fly.** Evidence in handler `25610` (`sub_255C0`→`25610` per the survey mapping): case 3 does un-terrain-locked vertical bob `position.z += ±64` toward an RNG-chosen hover altitude `dword_0x10_16 = (rand&0x3FF)+target.z` (`:16413–16461`), and case 8 free vertical approach `z += ±64` (`:16542–16551`). Its ground move-core `sub_1B8C0` runs only in the between-pass approach. Its **attack** is dual-mode: ranged **class-9 subtype 0** projectile (subSpell **500**, via `sub_1CC20`, `:16500`) fired during the hover/strafe pass, plus a **melee** strike (dmg = subSpell **300**, via `sub_1CED0` <768, `:16552`) during the dive. **The ctor launches no class-9 and does not itself set a "flying" flag — flight is entirely handler-driven z-writes.**

**Sounds:** 43/44 (dive, `(rand&1)+43`). No idle-loop sound.

---

## MODEL 20 — subSpell 100, 5500 life. CTOR `sub_4C7F0` (`:34307`)

**Ctor, in order** (`:34309–34334`):
1. `NewEvent`; guard.
2. `actionIndex_0x45_69 = -95` (= **0xA1 = 161** unsigned; base 0xA0=160, +1 idle).
3. `class=5`, `model=20`.
4. `minSpeed = 32`, `maxSpeed = 20`, `maxLife = 5500`, `actSpeed = minSpeed(32)`.
   - (Ctor comment in prompt says maxLife 100; **actual `maxLife_0x4 = 5500`** at `:34317`. The "100" is `subSpellIndex`.)
5. `SetEvent144_49C70`.
6. **RNG draw #1** `:34320`. Order here: `fov_0x22_34=0` first, then `roll=(rand&0x7FF)-1`, `yaw=(rand&0x7FF)-1`, `pitch=roll`. (**One draw.**)
7. `subSpellIndex_0x2A_42 = 100`; `byte_0x38_56 = 1`.
8. `dword_0xA0_160x = &str_D7BD6[89]` (**row 89**, `Level.cpp` line 101: `type_0x0=0x1E`, `word_0x1a=0x23=35`, `word_0x1c=0x1400=5120`, `word_0x1e=0x02AA`, `byte_0x20=0x00` ⇒ `&8=0`).
9. `byte_0x39_57 = 64`; `xtype_0x41_65 = 3`.
10. `byte_0x3E_62 = array_0x10[20]++`.
11. `AddEventToMap`; `CopyMaxLifeToLife` (5500).
12. `SetEntityIndexAndRot_49CD0(v1x, 288)` — **sprite row 288**.
13. `SetEntityShiftRot_49EA0(v1x, 384, 512)` ⇒ pitch=roll=**384**, fov=**512**.
- **No `dword_0x10_16` write in ctor at all** (left as whatever `NewEvent` zeroed). No `byte_0x3A_58`, no ctor sound/gate.

**State handlers (base 160 = 0xA0):**
- **+0 patrol 160 `sub_25D80`** (`:16613`): `sub_1BD90(a1x,160)`; deviation: if `actionIndex==162` validate class-3 model 0/1 target (else clear), `byte_0x46_70=0`.
- **+1 idle 161 `sub_25DE0`** (`:16632`): `sub_1BF90(a1x,160)`; same +162 validation deviation. (Row `&8=0` ⇒ acquire → +2=162.)
- **+2 attack/chase 162 `sub_25E40`** (`:16649`) — **model-specific, full trace**:
  - `v1x = target`; if invalid (`≤Entities[0]`) ⇒ `actionIndex=161`, jump to tail.
  - `PrepareEventSound_6E450(idx,-1,32)` **every tick** (idle-loop **sound 32**).
  - **`switch(byte_0x46_70)`**:
    - **case 0** (approach+ranged): `v3 = sub_1C310(a1x, 160, sub_1D1A0)` — the shared chase primitive using **`sub_1D1A0`** as its in-range fire fn = class-9 **subtype 21**, subSpell **780**, `z+128` (arcing lob). Engage-commit test: if target is class-3 model 0 use `mobilizeCounter_0x14E_334==0`, else `v3`. If satisfied ⇒ `byte_0x46_70=1`.
    - **case 1**: `byte_0x46_70=2`; `dword_0x10_16 = 32`; `actSpeed = 2*minSpeed(64)`; fall through.
    - **case 2** (melee rush): `sub_1C310(a1x,160, sub_1CE80)` — chase with **`sub_1CE80`** = melee `sub_11900` (dist<1024, dmg = subSpell **100**); on hit `v8=1`. `--dword_0x10_16`; at 0 `v8=1`. If `v8`: `byte_0x46_70=0`, `actSpeed=minSpeed(32)`.
  - Tail (`LABEL_21`): if `actionIndex!=162` ⇒ `actSpeed=minSpeed`.
  - **No RNG draws in this handler.**
- **+3 pack 163 `sub_25F70`** (`:16716`): `sub_1C560(a1x,0xA0)`; deviation: if `actionIndex==162` validate class-3 model 0/1 target, `byte_0x46_70=0`.
- **+4 prekill 164 `sub_25FD0`** (`:16734`): thin `PreKillEntity_1C890(a1x,160)`.
- **+5 kill 165 `sub_25FF0`** (`:16740`): thin `KillEntity_1C930`.
- **+7 spawn hook `sub_26020`** (`:16746`): `sub_1D5D0(a1x,160)`; deviation: if `actionIndex==162` ⇒ `byte_0x46_70=0`.
- **+6 hit/flee:** unreachable (row-89 `&8=0`); nominal 0x26010 absent from dispatch. OPEN/consistent.

**Attack linkage:** ranged = class-9 **subtype 21** (subSpell **780**, `z+128`) via `sub_1D1A0`; melee = `sub_11900` dmg **100** (self subSpell) via `sub_1CE80` (<1024). **Sounds:** 32 (per-tick during +2).

---

## Cross-model summary table

| model | ctor fn / life / minSpd/maxSpd | subSpell | row / `byte_0x20` | sprite (IdxAndRot) | ShiftRot(shift,fov) | +2 handler | attack mechanism | sounds |
|---|---|---|---|---|---|---|---|---|
| 16 | `4C310` / 60000 / 60,20 | 500 | 84 / 0x11 | 207 | 128,128 | `24510` | class-9 subtype0, **subSpell 1600 + mana 50000** homing bolt (15-tick burst) | 39 |
| 17 | `4C460` / 10000 / 68,20 | 350 | 85 / 0x01 | 285 | 128,128 | `24930` | dive-bomber: class-9 subtype20 (780) far / melee 350 (<768) | 58 |
| 18 | `4C590` / 36000 / 10,6 | 500 | 86 / 0x07 | 286 | 512,512 | `250B0` | 5-shot class-9 fan subtype0, **subSpell 800**; ground-locked | none found (OPEN) |
| 19 | `4C6B0` / 600 / 76,8 | 300 | 88 / 0x01 | 287 | 85,51 | `25610` | **FLYER**: class-9 subtype0 (500) strafe + melee 300 (<768) dive | 43/44 |
| 20 | `4C7F0` / 5500 / 32,20 | 100 | 89 / 0x00 | 288 | 384,512 | `25E40` | class-9 subtype21 (780,z+128) arc / melee 100 (<1024) | 32 |

**OPEN / flags:**
- The prompt's nominal +6/+7 addresses for m16 (0x24830), m17 (0x24DE0), m18 (0x25540), m19 (0x25D40), m20 (0x26010) **do not exist** in the decompile; `Events.cpp` dispatch has no cases for them. Every one of these five models has `byte_160_0x20_32 & 8 == 0`, so the +6 "hit/flee" slot is never reached; the actual per-model +6-equivalent hit response is folded into +2 (notably m19 case 7 / `v1==1`). Reported handler→role mapping follows the real dispatch (`Events.cpp:1632–1794`).
- m18 (`sub_24E20/25050/250B0/253B0`) contains **no `PrepareEventSound` call** — no attack/idle sound id located. Flagged OPEN.
- m16 has no `mana_0x90_144` literal in ctor (uses `SetEvent144_49C70`); the 50000 mana seen for m16 is written onto its *spawned projectile*, not the boss.
- m19/m20 leave `dword_0x10_16` un-normalized in ctor (m19 keeps `index%100`; m20 never writes it) — noted verbatim, not an error.