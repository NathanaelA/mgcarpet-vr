# MC2 CLASS-9 CREATORS — LOW BAND (subtypes 0x00–0x0D) — Verbatim Trace

Companion to `docs/traces/mc2-class9-flyers.md` (which covers 0x0E–0x1E). Same
Part-1 format. All citations are to
`/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`
(EF = `EventsFunctions.cpp`, EV = `Events.cpp`).

Read the flyers trace **§0 (Shared infrastructure)** first — every field, helper,
and flag op referenced below (`NewEvent_4A050`, `struct_byte_0xc &= 0xF7`,
`AddEventToMap_57D70`, `CopyMaxLifeToLife_49A20`, `SetEntityIndexAndRot_49CD0`,
`SetEntityShiftRot_49EA0`, the `str_D7BD6[N]` behavior-row table, and "no RNG in
any creator") is defined there and applies identically here.

---

## str91 subtype→creator map (low band), EF:1567

The str91 table row format is `{0x002A5C44, subtype, creator_addr, 1}`. Low-band
rows (EF:1568–1581):

| Subtype | str91 addr | Creator fn | EF line | action | model | row (str_D7BD6[N]) | sprite |
|--------|-----------|-----------|--------|-------|------|-----|-------|
| 0x00 | 0x0022E2E0 | `SummonFireball_4D2E0` | 34729 | 0  | 0  | 64 | 340 (+light) |
| 0x01 | 0x0022E3B0 | `SummonManaPosession_4D3B0` | 34764 | 1  | 1  | 61 | 209 (+ShiftRot) |
| 0x02 | 0x0022E470 | `sub_4D470` | 34788 | 2  | 2  | 60 | 211 |
| 0x03 | 0x0022E500 | `sub_4D500` | 34810 | 3  | 3  | 60 | 76  |
| 0x04 | 0x0022E590 | `sub_4D590` | 34832 | 4  | 4  | 60 | 210 |
| 0x05 | 0x0022E620 | `sub_4D620` | 34854 | 5  | 5  | 60 | 211 |
| 0x06 | 0x0022E6B0 | `sub_4D6B0` | 34876 | 6  | 6  | 60 | 212 |
| 0x07 | 0x0022E740 | `sub_4D740` | 34898 | 7  | 7  | 60 | 213 |
| 0x08 | 0x0022E7D0 | `sub_4D7D0` | 34920 | 8  | 8  | 63 | 214 |
| 0x09 | 0x0022E860 | `sub_4D860` | 34942 | 9  | 9  | 63 | 216 (+light) |
| 0x0A | 0x0022E900 | `sub_4D900` | 34965 | 10 | 10 | 60 | 18  |
| 0x0B | 0x0022E990 | `sub_4D990` | 34987 | 11 | 11 | 60 | 281 |
| 0x0C | 0x0022EA20 | `sub_4DA20` | 35009 | 12 | 12 | 60 | 216 |
| 0x0D | 0x0022EAB0 | `AddEvent09_0D_4DAB0` | 35031 | 13 | 13 | (none) | 195 (via `sub_49E10`) |

**Every low-band creator resolves to a real function body in remc2 — there are
NO `return NULL` stubs in this band** (contrast the flyers trace, where 0x10/0x12/0x13
are stubbed). action==model==subtype for the entire band (except 0x00 vs 0x1C — see note).

**Note on subtype 0x00 / 0x1C sharing `SummonFireball_4D2E0`:** str91[0x00] = 0x0022E2E0
points directly at `SummonFireball_4D2E0` (yields action 0 / model 0). The
high-band subtype 0x1C (str91 addr 0x0022E380 = `sub_4D380`, EF:34752) *wraps*
`SummonFireball_4D2E0` and then overrides `actionIndex=29; model=28`. So the raw
fireball creator is the 0x00 entry; 0x1C is the overridden re-skin. (Documented in
the flyers trace §Part-1 0x1C; repeated here for the shared body.)

---

## PART 1 — Creators (subtypes 0x00–0x0D)

Every creator follows the standard shape: `NewEvent_4A050()`; if non-null set
fields; `struct_byte_0xc_12_15.byte[0] &= 0xF7` (clear bit 3 / 0x08);
`AddEventToMap_57D70(event,position)`; `CopyMaxLifeToLife_49A20`; sprite via
`SetEntityIndexAndRot_49CD0` (or `sub_49E10` for 0x0D). Launch yaw/pitch are NOT
set by the creator (set by the launcher afterward). **RNG draws: 0 in every
creator** (verified — no `rand_0x14_20` / `9377*` term anywhere in EF:34729–35048).

### Subtype 0x00 — `SummonFireball_4D2E0` (EF:34729)
```
actionIndex = 0;   class = 9;   model = 0;
actSpeed = 384;    minSpeed = 384;    mana = 50;
maxLife = 0x2000 / actSpeed  (= 8192/384 = 21);
dword_0xA0_160x = &str_D7BD6[64];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 340);
AddEvent2_847D0(event, 128, 1, 0);        // trailing light/particle
```
Behavior **row 64**, sprite **340**, mana 50. **Extra write:** trailing
`AddEvent2_847D0(...,128,1,0)` (light) at EF:34746. No ShiftRot. **RNG: 0.**
(EF:34734–34746.)

### Subtype 0x01 — `SummonManaPosession_4D3B0` (EF:34764)
```
actionIndex = 1;   class = 9;   model = 1;
actSpeed = 384;    minSpeed = 384;    mana = 50;
dword_0xA0_160x = &str_D7BD6[61];
maxLife = 4096 / actSpeed  (= 10);
xtype_0x41_65 = 10;                        // *** extra: target-class filter preset to 10 ***
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 209);
SetEntityShiftRot_49EA0(event, 2*array.pitch, 5*array.fov/2);
```
Behavior **row 61**, sprite **209**, mana 50. **Extra writes:** `xtype_0x41_65 = 10`
(EF:34777 — this creator pre-seeds the victim-probe class filter, unusual for the
band), and `SetEntityShiftRot_49EA0(event, 2*array_0x52_82.pitch, 5*array_0x52_82.fov/2)`
(EF:34782 — note the pitch/roll extent = `2×array.pitch`, fov extent = `5×array.fov/2`).
**RNG: 0.** (EF:34769–34782.)

### Subtype 0x02 — `sub_4D470` (EF:34788)
```
actionIndex = 2;  class = 9;  model = 2;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 211);
```
Behavior **row 60**, sprite **211**, mana 50. No ShiftRot, no extra writes. **RNG: 0.**
(EF:34793–34804.)

### Subtype 0x03 — `sub_4D500` (EF:34810)
```
actionIndex = 3;  class = 9;  model = 3;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 76);
```
Behavior **row 60**, sprite **76**, mana 50. No ShiftRot, no extra writes. **RNG: 0.**
(EF:34815–34826.)

### Subtype 0x04 — `sub_4D590` (EF:34832)
```
actionIndex = 4;  class = 9;  model = 4;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 210);
```
Behavior **row 60**, sprite **210**, mana 50. No extras. **RNG: 0.** (EF:34837–34848.)

### Subtype 0x05 — `sub_4D620` (EF:34854)
```
actionIndex = 5;  class = 9;  model = 5;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 211);
```
Behavior **row 60**, sprite **211**, mana 50. No extras. **RNG: 0.** (EF:34859–34870.)

### Subtype 0x06 — `sub_4D6B0` (EF:34876)
```
actionIndex = 6;  class = 9;  model = 6;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 212);
```
Behavior **row 60**, sprite **212**, mana 50. No extras. **RNG: 0.** (EF:34881–34892.)

### Subtype 0x07 — `sub_4D740` (EF:34898)
```
actionIndex = 7;  class = 9;  model = 7;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 213);
```
Behavior **row 60**, sprite **213**, mana 50. No extras. **RNG: 0.** (EF:34903–34914.)

### Subtype 0x08 — `sub_4D7D0` (EF:34920)
```
actionIndex = 8;  class = 9;  model = 8;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[63];     // *** row 63 (not 60) ***
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 214);
```
Behavior **row 63**, sprite **214**, mana 50. No extras. **RNG: 0.** (EF:34925–34936.)

### Subtype 0x09 — `sub_4D860` (EF:34942)
```
actionIndex = 9;  class = 9;  model = 9;
actSpeed = 384;   minSpeed = 384;   mana = 50;
maxLife = 3584 / actSpeed  (= 9);     // *** 3584 (0xE00), not 0x2000 ***
dword_0xA0_160x = &str_D7BD6[63];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 216);
AddEvent2_847D0(event, 128, 9, 0);    // *** trailing light, param 9 (not 1) ***
```
Behavior **row 63**, sprite **216**, mana 50. **Extra write:** trailing
`AddEvent2_847D0(event,128,9,0)` at EF:34959 (note middle param `9`, vs `1` in the
0x00 fireball). Shorter maxLife divisor (3584). **RNG: 0.** (EF:34947–34959.)

### Subtype 0x0A — `sub_4D900` (EF:34965)
```
actionIndex = 10;  class = 9;  model = 10;
actSpeed = 384;    minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 18);
```
Behavior **row 60**, sprite **18**, mana 50. No extras. **RNG: 0.** (EF:34970–34981.)

### Subtype 0x0B — `sub_4D990` (EF:34987)
```
actionIndex = 11;  class = 9;  model = 11;
actSpeed = 384;    minSpeed = 384;   mana = 50;
maxLife = 0x2000 / actSpeed  (= 21);
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 281);
```
Behavior **row 60**, sprite **281**, mana 50. No extras. **RNG: 0.** (EF:34992–35003.)

### Subtype 0x0C — `sub_4DA20` (EF:35009)
```
actionIndex = 12;  class = 9;  model = 12;
actSpeed = 384;    minSpeed = 384;   mana = 50;
maxLife = 2048 / actSpeed  (= 5);     // *** 2048 (0x800), not 0x2000 ***
dword_0xA0_160x = &str_D7BD6[60];
byte[0] &= 0xF7;  AddToMap;  CopyLife;
SetEntityIndexAndRot_49CD0(event, 216);
```
Behavior **row 60**, sprite **216**, mana 50. Shorter maxLife divisor (2048).
No extras. **RNG: 0.** (EF:35014–35025.)

### Subtype 0x0D — `AddEvent09_0D_4DAB0` (EF:35031)
```
actionIndex = 0xD (13);  class = 9;  model = 0xD (13);
actSpeed = 384;   minSpeed = 384;
maxLife = 5120 / actSpeed  (= 13);    // *** 5120 (0x1400) ***
// *** NO mana write (mana left at NewEvent default) ***
// *** NO dword_0xA0_160x behavior-row write ***
byte[0] &= 0xF7u;  AddToMap;  CopyLife;
sub_49E10(event, 195);                // *** sub_49E10 (the ×2 rot variant), NOT SetEntityIndexAndRot_49CD0 ***
```
**Distinct from the rest of the band:** no `mana_0x90_144` set, no
`dword_0xA0_160x` behavior row set, and the sprite is applied via
**`sub_49E10(event, 195)`** (EF:35045 — the "×2 on pitch/roll/fov" variant per
flyers §0.3) instead of `SetEntityIndexAndRot_49CD0`. Sprite **195**, maxLife
divisor 5120. No ShiftRot, no trailing light. **RNG: 0.** (EF:35036–35045.)

---

## Cross-band observations

- **Uniform kinematics:** every low-band flyer is `actSpeed = minSpeed = 384`,
  `class = 9`, `mana = 50` (except 0x0D which sets no mana). Only the `maxLife`
  numerator varies: 0x2000 (8192) for most, 3584 (0x09), 2048 (0x0C), 5120 (0x0D),
  4096 (0x01). Divisor is always `actSpeed` (384).
- **Behavior rows:** default is **row 60** (`str_D7BD6[60]`, the standard flyer
  row, = flyers-trace §0.2 index 60 @ 0x7f8). Exceptions: **row 61** (0x01, the
  possession homer — same row as high-band 0x11/0x19), **row 64** (0x00 fireball —
  same row as high-band 0x1C), **row 63** (0x08 and 0x09), and **no row** (0x0D).
- **Trailing lights (`AddEvent2_847D0`):** only 0x00 (`128,1,0`) and 0x09
  (`128,9,0`). All others have none.
- **ShiftRot (`SetEntityShiftRot_49EA0`):** only 0x01
  (`2*array.pitch, 5*array.fov/2`). All others rely on the default rot set by
  `SetEntityIndexAndRot_49CD0` (or `sub_49E10` for 0x0D).
- **`xtype_0x41_65` preset:** only 0x01 sets it (=10) in the creator; the rest
  leave the victim-class filter to be armed by the launcher.
- **Flag op:** `byte[0] &= 0xF7` (clear bit 3 / 0x08) is present in ALL 14 — same
  as the flyers band.

---

## COMPACT TABLE (subtype → action / model / speed / life / row / sprite)

| Subtype | Creator (EF) | action | model | actSpeed=minSpeed | maxLife (num/384) | row | sprite | extras |
|--------|-------------|-------|------|------|------|-----|-------|-------|
| 0x00 | SummonFireball_4D2E0 (34729) | 0  | 0  | 384 | 0x2000→21 | 64 | 340 | +AddEvent2(128,1,0) |
| 0x01 | SummonManaPosession_4D3B0 (34764) | 1  | 1  | 384 | 4096→10 | 61 | 209 | xtype=10; ShiftRot(2·pitch, 5·fov/2) |
| 0x02 | sub_4D470 (34788) | 2  | 2  | 384 | 0x2000→21 | 60 | 211 | — |
| 0x03 | sub_4D500 (34810) | 3  | 3  | 384 | 0x2000→21 | 60 | 76  | — |
| 0x04 | sub_4D590 (34832) | 4  | 4  | 384 | 0x2000→21 | 60 | 210 | — |
| 0x05 | sub_4D620 (34854) | 5  | 5  | 384 | 0x2000→21 | 60 | 211 | — |
| 0x06 | sub_4D6B0 (34876) | 6  | 6  | 384 | 0x2000→21 | 60 | 212 | — |
| 0x07 | sub_4D740 (34898) | 7  | 7  | 384 | 0x2000→21 | 60 | 213 | — |
| 0x08 | sub_4D7D0 (34920) | 8  | 8  | 384 | 0x2000→21 | 63 | 214 | — |
| 0x09 | sub_4D860 (34942) | 9  | 9  | 384 | 3584→9  | 63 | 216 | +AddEvent2(128,9,0) |
| 0x0A | sub_4D900 (34965) | 10 | 10 | 384 | 0x2000→21 | 60 | 18  | — |
| 0x0B | sub_4D990 (34987) | 11 | 11 | 384 | 0x2000→21 | 60 | 281 | — |
| 0x0C | sub_4DA20 (35009) | 12 | 12 | 384 | 2048→5  | 60 | 216 | — |
| 0x0D | AddEvent09_0D_4DAB0 (35031) | 13 | 13 | 384 | 5120→13 | (none) | 195 | no mana; sprite via sub_49E10 |

## OPEN ITEMS
1. **None in the creators themselves** — all 14 low-band creators (0x00–0x0D)
   resolve to full, non-stubbed function bodies in remc2 (EF:34729–35048). Every
   field is recoverable verbatim.
2. **Behavior-row contents** (`str_D7BD6[60/61/63/64]`) and **sprite/particle
   table** entries (`particlesParameters_D951C[18/76/195/209/210/211/212/213/214/216/281/340]`)
   are indices only — their data lives in separate tables not read here (same
   caveat as flyers-trace OPEN #5). Extract separately if the port needs the
   turn caps / model geometry.
3. **Launchers** for the low band were out of scope for this task (Part-1 creators
   only). Flyers-trace §3.3 already documents one: thunk `sub_1D260` →
   spawns **(9,9)** subtype 0x09 (arms byte_0x43/44 = 10/23, subSpellIndex = 4000).
   The remaining low-band launch sites are not yet mapped — flag for a follow-up
   caller sweep if needed.
