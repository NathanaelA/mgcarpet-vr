# MC1 entity state-table registration map (remc1 decompile)

Source: `reference/remc1/sub_main.cpp` (113,017 lines) + `reference/remc1/engine/Basic.h`.
Dispatch loop `sub_41780_41AC0` at `sub_main.cpp:52197`; per-entity tick at `:52354-52406`.

All line numbers below are `sub_main.cpp` unless noted.

---

## 1. Row struct + `data10` semantics

Two 14-byte row structs, both declared at `sub_main.cpp:4304-4325`:

### `Type_254A34x` — the per-TICK handler row (used by `str_0`)
```c
#pragma pack (1)
typedef struct {              // size 14
    uint32_t data0;           // @0  = 0x002548F4 filler / string-ptr sentinel ("is-populated" marker)
    uint16_t data4;           // @4  = STATE ID (must equal the row's array index to be valid)
    void (*data6)(Type_AE400_29795*);   // @6  = per-tick handler (entity ptr)
    uint16_t data10;          // @10 = TICK-ENABLE FLAG   <-- the gate
    uint16_t data12;          // @12 = unused (always 0)
} Type_254A34x;
```

### `Type_254A34` — the CONSTRUCTOR/spawn row (used by `str_4`)
```c
typedef struct {              // size 14
    uint32_t data0;           // @0
    uint16_t data4;           // @4  = state/model id echo
    Type_AE400_29795* (*data6)(axis_3d*);   // @6 = spawn ctor (returns new entity)
    uint16_t data10;          // @10 = CONSTRUCTOR-ENABLED flag
    uint16_t data12;          // @12
} Type_254A34;
```

### Top-level per-class descriptor `Type_96902` (`:4327-4334`, array `:5041`)
```c
typedef struct {              // size 18
    Type_254A34x* str_0;      // @0  -> per-state TICK table   (indexed by state70)
    Type_254A34*  str_4;      // @4  -> per-state CTOR table    (indexed by state/model)
    uint32_t data4x; uint16_t data8x; uint32_t data14x;
} Type_96902;
Type_96902 dword_96902[/*14*/] = { ... };   // indexed by entity CLASS = var_u8_29859_64 (offset 64)
```

**`data10` width & meaning:** `uint16_t` (2 bytes). It is the "this (class,state) row is LIVE" gate.
- In the **tick** dispatch (`:52356`): `if (dword_96902[class].str_0[state].data10) { data6(ent); ent.+63++; }`.
  So a nonzero `data10` is *the* condition that both calls the per-tick handler **and** steps the `+63` phase clock (`var_u8_29858_63`, `:52406`).
- In the **spawn** dispatch (`sub_373F0_377B0` `:43916`, `sub_37560_37920` `:44001`): `data10` gates whether the constructor `str_4[...].data6` runs at all.
- Observed values are **only `0x0001` (enabled) or `0x0000` (disabled)** — it is used as a boolean, never a count.

Note the dispatch also requires `data4 == state70` (`:52354`). When a row's `data4` does not equal its array index (interior "gap" rows whose `data4==0`), the entity hits the `"STATE-ID does not match. CLASS %d, MODEL %d, STATE %d"` error branch (`:52411`) and is NOT ticked (it is routed to `sub_41E80_421C0`).

---

## 2. Where rows are REGISTERED — it is STATIC, there is no runtime helper

The task's hypothesized `register(class,state,fn,flags)` helper **does not exist**. Registration is entirely **static C array initializers**, resolved at compile/link time:

- Per-class TICK arrays `Type_254A34x str_254xxxx[]` at **`:4646-5039`** (one array per class).
- Per-class CTOR arrays `Type_254A34 str_254xxxx[]` at **`:4344-4614`** (one array per class).
- Top-level binding `Type_96902 dword_96902[] = {...}` at **`:5041-5056`** maps each class index to its `(str_0, str_4)` pair.

Initializer field order is `{ data0, data4(state), data6(handler), data10, data12 }`, so **the 4th initializer literal is `data10`** — e.g. `{ 0x002548F4, 0x0000, sub_49890_49BD0, 0x0001, 0x0000 }` registers class-2 state-0 with `data10 = 1`.

Class -> `str_0` tick array (from `dword_96902[]`, `:5042-5055`):

| class | str_0 tick array | ctor array (str_4) | tick array def line |
|------|------|------|------|
| 0 | `nullptr` (never dispatched; loop guards `if (class)`) | `nullptr` | — |
| 1 | `str_254A08` | `str_254A34` | 4646 |
| 2 | `str_254C3C` | `str_254D48` | 4650 |
| 3 | `str_254ADC` | `str_254B84` | 4668 |
| 4 | `str_254DAC` | `str_254DBC` | 4683 |
| 5 | `str_254DCC` | `str_255478` | 4687 |
| 6 | `str_255574` | `str_2555A0` | 4813 |
| 7 | `str_2555CC` | `str_255620` | 4819 |
| 8 | `str_255674` | `str_2556D8` | 4828 |
| 9 | `str_25573C` | `str_255870` | 4838 |
| 10 | `str_255998` | `str_255D0C` | 4856 |
| 11 | `str_256038` | `str_256208` | 4921 |
| 12 | `str_2563D8` | `str_2567D8` | 4957 |
| 13 | `str_256938` | `str_256980` | 5033 |

(The `off_97D12[]` table at `:5075` is a *third*, separate spawn table keyed by feature id, used by level-gen `sub_36480_36840` `:43074`; not part of the per-tick dispatch.)

---

## 4. Is `data10` mutated at runtime? NO — it is per-row static.

Exhaustive search of `sub_main.cpp`:
- No assignment to `.data10`, `.data6`, or `.data4` anywhere (grep `\.data(10|6|4)\s*=[^=]` -> 0 hits).
- No `dword_96902[...]` appears on the left side of an assignment (0 hits).
- No `memcpy`/`memset` targets these arrays.

Every reference to `.data10` is a **read** in a dispatch guard (`:43219, 43231, 43246, 43259, 43307, 43320, 43337, 43350, 43916, 44001, 52356`). `data10` is fixed at program load and never changes. So the tick/no-tick status of a `(class,state)` row is 100% static — an entity is "asleep" purely as a function of its `(class,state)` coordinate, never a runtime toggle.

---

## 5. The NO-TICK (inert) set

Total tick-table rows parsed: **356**; rows that actually tick (`data10!=0 && data4==pos && handler!=null`): **320**. The inert rows:

**Whole classes that never tick:**
- **class 0** — `str_0 == nullptr`; the loop's `if (v17)` (`:52352`) skips class 0 entirely.
- **class 1** (`str_254A08`) — single row `[1][0]`: handler `nullptr`, `data10=0` (`:4647`). Inert.
- **class 4** (`str_254DAC`) — single row `[4][0]`: handler `nullptr`, `data0=0`, `data10=0` (`:4684`). Inert (no live state).

**Individual disabled rows (`data10 == 0`, handler PRESENT but tick suppressed):**
| class | state | handler | line |
|------|------|------|------|
| 5 | 0x66-0x6B (102-107) | `sub_20B90` | 4790-4795 |
| 5 | 0x6C-0x71 (108-113) | `sub_20BA0` | 4796-4801 |
| 5 | 0x72-0x77 (114-119) | `sub_20BB0` | 4802-4807 |
| 10 | 0x2E (46) | `sub_29540` | 4903 |

**Gap rows (`data4 == 0` != array index -> STATE-ID-mismatch error path, not ticked):**
| class | array pos | line |
|------|------|------|
| 10 | 0x27 (39) | 4896 |
| 10 | 0x2F (47) | 4904 |
| 10 | 0x31 (49) | 4906 |
| 10 | 0x32 (50) | 4907 |

(Plus one terminal sentinel `{0,0,null,0,0}` closing every array — never reached for a valid state.)

### Confirm / refute of the specific rows asked

All numbers are from the tables in section 3. **Every one of the queried rows is `data10 = 0x0001` = TICKED**, so each "asleep/inert" hypothesis is **REFUTED**:

| queried row | actual | verdict | line |
|------|------|------|------|
| class 2 state 0 ("trees") | `sub_49890_49BD0`, data10=**1** | **REFUTE** — ticks every logic step | 4651 |
| class 5 state 120 (0x78, "multipart") | `sub_19550`, data10=**1** | **REFUTE** — state 120 DOES tick | 4808 |
| class 5 model 13 state 79 (0x4F) | `sub_1F640`, data10=**1** | **REFUTE** — ticks (model is not part of the key) | 4767 |
| class 10 state 52 (0x34, "buildings m45") | `sub_28DC0`, data10=**1** | **REFUTE** — ticks | 4909 |
| class 12 state 1 | `sub_56250`, data10=**1** | **REFUTE** | 4959 |
| class 12 state 9 | `sub_56510_56A40`, data10=**1** | **REFUTE** | 4967 |
| class 12 state 48 (0x30) | `sub_57610_57B40`, data10=**1** | **REFUTE** | 5006 |
| class 12 state 49 (0x31) | `sub_56250`, data10=**1** | **REFUTE** | 5007 |
| class 9 state 1 | `sub_52ED0_53210`, data10=**1** | **REFUTE** | 4840 |
| class 3 state 4 | `sub_46DB0_470F0`, data10=**1** | **REFUTE** | 4673 |

Important nuances that likely explain the "asleep" intuition:
- **"class 5 multipart segments" is the 0x66-0x77 band, NOT state 120.** States 102-119 have `data10=0` (child segments driven by their parent). State 120 (`sub_19550`) is the ticked parent and is *also* specially excluded from the awake linked-list build at `:52266` (`var_u8_29865_70 != 120`), but it still runs in the dispatch loop.
- **"class 10 buildings" inert state is 0x2E (46), not 0x34 (52).** 0x34 ticks; 0x2E is `data10=0`.
- A `data10=1` row whose handler is a **stub** (`sub_49AC0/49B40/49B70` for class 2 `:4617-4619`; `sub_20B90/A0/B0`; `sub_1F630/1F9F0/...`) is still "ticked": the (no-op) handler is called AND `+63` is incremented. So such entities are **byte-stable but NOT asleep** — the `+63` clock keeps advancing. That distinction ("byte-stable" vs "+63-frozen") is the crux of the LCG puzzle below.

---

## 6. The head-LCG call graph (LCG advance vs `+63` advance)

### Call graph (line-cited)
```
GameLoop_34610_349D0            :41730
  while(1) { if(exit) break;    :41767-41771
    DrawAndEventsInGame_34530   :41772 }
      DrawAndEventsInGame_34530_348F0            :41656
        switch (str_AE408_AE3F8->gameSpeed_150)  :41672
          case 1:  for i in 0..3   -> sub_41780_41AC0()   :41677  (4x)
          case 2:  for j in 0..15  -> sub_41780_41AC0()   :41683  (16x)
          default: sub_41780_41AC0()                      :41688  (1x)
```
So `sub_41780_41AC0` runs **1 / 4 / 16 sub-steps per rendered frame** depending on game speed. It has exactly **3 call sites: `:41677, :41683, :41688`** — all inside `DrawAndEventsInGame`. No other caller.

### Inside `sub_41780_41AC0` (`:52197`)
```
:52223   rand_4 = 9377*rand_4 + 9439;              // GLOBAL LCG draw — UNCONDITIONAL, first statement
:52224   if (str_AE400_AE3F0->var_0.var_u8_2 & 1)  goto LABEL_52;   // pause early-out -> skip everything below
:52226   awake-list build loop (calls sub_41E90_421D0 for byte[1]&4 entities)
:52328   dispatch loop over entities 1..999:
:52354     if (data4 == state70)
:52356        if (data10) {
:52405            data6(&ent);                      // per-tick handler
:52406            ent.var_u8_29858_63++;            // +63 phase clock  <-- ONLY increment site in the whole file
:52411     else  "STATE-ID does not match" error
```

### Every OTHER site that steps the two quantities
- **`+63` (`var_u8_29858_63`) increment:** grep across the whole file finds `var_u8_29858_63++` at **exactly one site — `:52406`**. Every other reference is a modulo READ (`% v_26`, `& 3`, `& 7`, `& 0x3F`, …) used by handlers for animation phase. The only other WRITES are `:43882` and `:43907` inside `NewEvent_372C0_37680` (entity allocation), which *set* `+63 = (slot - base)` = the slot index at spawn — not a per-tick step.
- **GLOBAL `rand_4` (`str_AE400_AE3F0->rand_4`) writes:** exactly four sites —
  - `:52223` `sub_41780_41AC0` head — the per-substep draw (above).
  - `:43185` `sub_36620_369E0` head — a *second* dispatch, but it is called **only from `GenerateFeatures_36430_367F0` `:43061`** (level/feature generation), and it does **not** step `+63`. Its `do{...}while(runAgain)` re-scans all 1000 entities for class-10 buildings but draws `rand_4` **once** at the head regardless of iteration count.
  - `:43276` `sub_36620_369E0_new` head — a rewrite variant with **no callers** (dead).
  - `:39292` `sub_31AA0_31AE0` — `rand_4 = *(a1+4)` **reseed from a saved value** at level load (not an LCG step).
  (All the other ~130 `9377*...+9439` hits step per-entity `rand_29799_4` (entity `+4`) or the separate global `pseudoRand_12C1E0_12C1D0`, not the header LCG.)

### Finding on the 12.5% anomaly
In the remc1 code, **the head LCG draw (`:52223`) and the `+63` step (`:52406`) are welded inside the same function**, with the draw as the unconditional first statement and `+63` strictly downstream. The only branch between them is the pause early-out (`:52224`), which *skips* the dispatch (and thus `+63`) while the draw has *already happened*. There is **no path anywhere that reaches `+63++` without first executing the `rand_4` draw**, and `+63++` exists at only one site. The LCG `x' = 9377x + 9439 (mod 2^32)` can never yield `x'==x` (9376·x is always even, 9439 odd), so every call to `sub_41780_41AC0` provably changes `rand_4`.

Therefore, **the observed "global LCG static on ~12.5% of ticks while `+63` still advances" cannot be produced by the remc1 code as written** — that combination is impossible here. The only decouplings present are the *inverse* (LCG advances while `+63` does not):
1. **pause early-out** `:52224` — draw happens, dispatch/`+63` skipped;
2. **`sub_36620_369E0`** `:43185` (feature-gen) — draw happens, no `+63`;
3. **level-load reseed** `:39292` — `rand_4` set, no `+63`.

Conclusion (code-grounded, not speculation): the `+63`-without-LCG behaviour seen in live retail memory is **not reproducible from this decompile's control flow**, which strongly implies the retail binary positions/gates the header draw differently from remc1's unconditional hoist to the top of `sub_41780_41AC0` — i.e. retail's header LCG is **draw-driven / conditionally reached**, whereas remc1 emits it unconditionally per sub-step. The remake and retail diverge precisely at the placement of `:52223`. (The multi-sub-step `gameSpeed` fan-out at `:41677/41683/41688` is real but does not explain the anomaly: each sub-step draws `rand_4` once and steps `+63` once.)

---

## 3. FULL registration table

(Generated by parsing `str_254xxxx[]` at `:4646-5039`. `pos` = array index = the state used to index `str_0[state70]`; `state(data4)` = the row's echoed state id; a row ticks iff `data10!=0 && data4==pos && handler!=null`.)

### class 1  (dword_96902[1].str_0 = str_254A08, def line 4645)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | nullptr | 0x0000 | no | 4647 | data10=0 DISABLED |

### class 2  (dword_96902[2].str_0 = str_254C3C, def line 4649)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_49890_49BD0 | 0x0001 | YES | 4651 |  |
| 0x1 | 0x1 | sub_499C0_49D00 | 0x0001 | YES | 4652 |  |
| 0x2 | 0x2 | sub_49A50_49D90 | 0x0001 | YES | 4653 |  |
| 0x3 | 0x3 | sub_49AA0_49DE0 | 0x0001 | YES | 4654 |  |
| 0x4 | 0x4 | sub_49AC0 | 0x0001 | YES | 4655 |  |
| 0x5 | 0x5 | sub_49AC0 | 0x0001 | YES | 4656 |  |
| 0x6 | 0x6 | sub_49AD0_49E10 | 0x0001 | YES | 4657 |  |
| 0x7 | 0x7 | sub_49B40 | 0x0001 | YES | 4658 |  |
| 0x8 | 0x8 | sub_49B40 | 0x0001 | YES | 4659 |  |
| 0x9 | 0x9 | sub_49B50_49E90 | 0x0001 | YES | 4660 |  |
| 0xA | 0xA | sub_49B70 | 0x0001 | YES | 4661 |  |
| 0xB | 0xB | sub_49B70 | 0x0001 | YES | 4662 |  |
| 0xC | 0xC | sub_49B70 | 0x0001 | YES | 4663 |  |
| 0xD | 0xD | sub_49B70 | 0x0001 | YES | 4664 |  |
| 0xE | 0x0 | nullptr | 0x0000 | no | 4665 | GAP data4=0x0!=pos => STATE-ID mismatch |

### class 3  (dword_96902[3].str_0 = str_254ADC, def line 4667)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_45C90_45FD0 | 0x0001 | YES | 4669 |  |
| 0x1 | 0x1 | sub_13170 | 0x0001 | YES | 4670 |  |
| 0x2 | 0x2 | sub_45FC0_46300 | 0x0001 | YES | 4671 |  |
| 0x3 | 0x3 | sub_46480_467C0 | 0x0001 | YES | 4672 |  |
| 0x4 | 0x4 | sub_46DB0_470F0 | 0x0001 | YES | 4673 |  |
| 0x5 | 0x5 | sub_46F10_47250 | 0x0001 | YES | 4674 |  |
| 0x6 | 0x6 | sub_470E0 | 0x0001 | YES | 4675 |  |
| 0x7 | 0x7 | sub_47F80 | 0x0001 | YES | 4676 |  |
| 0x8 | 0x8 | sub_47F80 | 0x0001 | YES | 4677 |  |
| 0x9 | 0x9 | sub_47F90_482D0 | 0x0001 | YES | 4678 |  |
| 0xA | 0xA | sub_481C0 | 0x0001 | YES | 4679 |  |
| 0xB | 0x0 | nullptr | 0x0000 | no | 4680 | terminal sentinel |

### class 4  (dword_96902[4].str_0 = str_254DAC, def line 4682)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | nullptr | 0x0000 | no | 4684 | terminal sentinel |

### class 5  (dword_96902[5].str_0 = str_254DCC, def line 4686)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_1B060 | 0x0001 | YES | 4688 |  |
| 0x1 | 0x1 | sub_1B070 | 0x0001 | YES | 4689 |  |
| 0x2 | 0x2 | sub_1B090 | 0x0001 | YES | 4690 |  |
| 0x3 | 0x3 | sub_1B0E0 | 0x0001 | YES | 4691 |  |
| 0x4 | 0x4 | sub_1B100 | 0x0001 | YES | 4692 |  |
| 0x5 | 0x5 | sub_1B110 | 0x0001 | YES | 4693 |  |
| 0x6 | 0x6 | sub_1B160 | 0x0001 | YES | 4694 |  |
| 0x7 | 0x7 | sub_1B200 | 0x0001 | YES | 4695 |  |
| 0x8 | 0x8 | sub_1B2D0 | 0x0001 | YES | 4696 |  |
| 0x9 | 0x9 | sub_1B320 | 0x0001 | YES | 4697 |  |
| 0xA | 0xA | sub_1B330 | 0x0001 | YES | 4698 |  |
| 0xB | 0xB | sub_1B340 | 0x0001 | YES | 4699 |  |
| 0xC | 0xC | sub_1B350 | 0x0001 | YES | 4700 |  |
| 0xD | 0xD | sub_1B370 | 0x0001 | YES | 4701 |  |
| 0xE | 0xE | sub_1B3C0 | 0x0001 | YES | 4702 |  |
| 0xF | 0xF | sub_1B4C0 | 0x0001 | YES | 4703 |  |
| 0x10 | 0x10 | sub_1B4E0 | 0x0001 | YES | 4704 |  |
| 0x11 | 0x11 | sub_1B4F0 | 0x0001 | YES | 4705 |  |
| 0x12 | 0x12 | sub_1B500 | 0x0001 | YES | 4706 |  |
| 0x13 | 0x13 | sub_1B510 | 0x0001 | YES | 4707 |  |
| 0x14 | 0x14 | sub_1B520 | 0x0001 | YES | 4708 |  |
| 0x15 | 0x15 | sub_1B570 | 0x0001 | YES | 4709 |  |
| 0x16 | 0x16 | sub_1B580 | 0x0001 | YES | 4710 |  |
| 0x17 | 0x17 | sub_1B590 | 0x0001 | YES | 4711 |  |
| 0x18 | 0x18 | sub_1B5A0 | 0x0001 | YES | 4712 |  |
| 0x19 | 0x19 | sub_1B5D0 | 0x0001 | YES | 4713 |  |
| 0x1A | 0x1A | sub_1BB20 | 0x0001 | YES | 4714 |  |
| 0x1B | 0x1B | sub_1BBE0 | 0x0001 | YES | 4715 |  |
| 0x1C | 0x1C | sub_1BC10 | 0x0001 | YES | 4716 |  |
| 0x1D | 0x1D | sub_1BC40 | 0x0001 | YES | 4717 |  |
| 0x1E | 0x1E | sub_1BD10 | 0x0001 | YES | 4718 |  |
| 0x1F | 0x1F | sub_1BD20 | 0x0001 | YES | 4719 |  |
| 0x20 | 0x20 | sub_1C110 | 0x0001 | YES | 4720 |  |
| 0x21 | 0x21 | sub_1C170 | 0x0001 | YES | 4721 |  |
| 0x22 | 0x22 | sub_1C3C0 | 0x0001 | YES | 4722 |  |
| 0x23 | 0x23 | sub_1C3D0 | 0x0001 | YES | 4723 |  |
| 0x24 | 0x24 | sub_1C490 | 0x0001 | YES | 4724 |  |
| 0x25 | 0x25 | sub_1C4A0 | 0x0001 | YES | 4725 |  |
| 0x26 | 0x26 | sub_1C4F0 | 0x0001 | YES | 4726 |  |
| 0x27 | 0x27 | sub_1C880 | 0x0001 | YES | 4727 |  |
| 0x28 | 0x28 | sub_1C8D0 | 0x0001 | YES | 4728 |  |
| 0x29 | 0x29 | sub_1C8E0 | 0x0001 | YES | 4729 |  |
| 0x2A | 0x2A | sub_1C8F0 | 0x0001 | YES | 4730 |  |
| 0x2B | 0x2B | sub_1C900 | 0x0001 | YES | 4731 |  |
| 0x2C | 0x2C | sub_1C960 | 0x0001 | YES | 4732 |  |
| 0x2D | 0x2D | sub_1CA00 | 0x0001 | YES | 4733 |  |
| 0x2E | 0x2E | sub_1CA20 | 0x0001 | YES | 4734 |  |
| 0x2F | 0x2F | sub_1CA30 | 0x0001 | YES | 4735 |  |
| 0x30 | 0x30 | sub_1CA40 | 0x0001 | YES | 4736 |  |
| 0x31 | 0x31 | sub_1CA50 | 0x0001 | YES | 4737 |  |
| 0x32 | 0x32 | sub_1CE30 | 0x0001 | YES | 4738 |  |
| 0x33 | 0x33 | sub_1CF50 | 0x0001 | YES | 4739 |  |
| 0x34 | 0x34 | sub_1CF60 | 0x0001 | YES | 4740 |  |
| 0x35 | 0x35 | sub_1CFE0 | 0x0001 | YES | 4741 |  |
| 0x36 | 0x36 | sub_1CFF0 | 0x0001 | YES | 4742 |  |
| 0x37 | 0x37 | sub_1D060 | 0x0001 | YES | 4743 |  |
| 0x38 | 0x38 | sub_1DA60 | 0x0001 | YES | 4744 |  |
| 0x39 | 0x39 | sub_1DC80 | 0x0001 | YES | 4745 |  |
| 0x3A | 0x3A | sub_1DCB0 | 0x0001 | YES | 4746 |  |
| 0x3B | 0x3B | sub_1DCC0 | 0x0001 | YES | 4747 |  |
| 0x3C | 0x3C | sub_1DDD0 | 0x0001 | YES | 4748 |  |
| 0x3D | 0x3D | sub_1DDE0 | 0x0001 | YES | 4749 |  |
| 0x3E | 0x3E | sub_1DDF0 | 0x0001 | YES | 4750 |  |
| 0x3F | 0x3F | sub_1DE10 | 0x0001 | YES | 4751 |  |
| 0x40 | 0x40 | sub_1DE20 | 0x0001 | YES | 4752 |  |
| 0x41 | 0x41 | sub_1DE30 | 0x0001 | YES | 4753 |  |
| 0x42 | 0x42 | sub_1DE40 | 0x0001 | YES | 4754 |  |
| 0x43 | 0x43 | sub_1DFE0 | 0x0001 | YES | 4755 |  |
| 0x44 | 0x44 | sub_1E380 | 0x0001 | YES | 4756 |  |
| 0x45 | 0x45 | sub_1E6F0 | 0x0001 | YES | 4757 |  |
| 0x46 | 0x46 | sub_1E700 | 0x0001 | YES | 4758 |  |
| 0x47 | 0x47 | sub_1E710 | 0x0001 | YES | 4759 |  |
| 0x48 | 0x48 | sub_1EA40 | 0x0001 | YES | 4760 |  |
| 0x49 | 0x49 | sub_1EED0 | 0x0001 | YES | 4761 |  |
| 0x4A | 0x4A | sub_1F120 | 0x0001 | YES | 4762 |  |
| 0x4B | 0x4B | sub_1F390 | 0x0001 | YES | 4763 |  |
| 0x4C | 0x4C | sub_1F5A0 | 0x0001 | YES | 4764 |  |
| 0x4D | 0x4D | sub_1F5B0 | 0x0001 | YES | 4765 |  |
| 0x4E | 0x4E | sub_1F630 | 0x0001 | YES | 4766 |  |
| 0x4F | 0x4F | sub_1F640 | 0x0001 | YES | 4767 |  |
| 0x50 | 0x50 | sub_1F9F0 | 0x0001 | YES | 4768 |  |
| 0x51 | 0x51 | sub_1F9F0 | 0x0001 | YES | 4769 |  |
| 0x52 | 0x52 | sub_1FA00 | 0x0001 | YES | 4770 |  |
| 0x53 | 0x53 | sub_1FAA0 | 0x0001 | YES | 4771 |  |
| 0x54 | 0x54 | sub_1FAB0 | 0x0001 | YES | 4772 |  |
| 0x55 | 0x55 | sub_1FAC0 | 0x0001 | YES | 4773 |  |
| 0x56 | 0x56 | sub_1FE80 | 0x0001 | YES | 4774 |  |
| 0x57 | 0x57 | sub_1FE80 | 0x0001 | YES | 4775 |  |
| 0x58 | 0x58 | sub_1FE90 | 0x0001 | YES | 4776 |  |
| 0x59 | 0x59 | sub_1FEC0 | 0x0001 | YES | 4777 |  |
| 0x5A | 0x5A | sub_1FF50 | 0x0001 | YES | 4778 |  |
| 0x5B | 0x5B | sub_1FF60 | 0x0001 | YES | 4779 |  |
| 0x5C | 0x5C | sub_201D0 | 0x0001 | YES | 4780 |  |
| 0x5D | 0x5D | sub_203E0 | 0x0001 | YES | 4781 |  |
| 0x5E | 0x5E | sub_203F0 | 0x0001 | YES | 4782 |  |
| 0x5F | 0x5F | sub_20400 | 0x0001 | YES | 4783 |  |
| 0x60 | 0x60 | sub_20700 | 0x0001 | YES | 4784 |  |
| 0x61 | 0x61 | sub_20710 | 0x0001 | YES | 4785 |  |
| 0x62 | 0x62 | sub_207E0 | 0x0001 | YES | 4786 |  |
| 0x63 | 0x63 | sub_20B60 | 0x0001 | YES | 4787 |  |
| 0x64 | 0x64 | sub_20B70 | 0x0001 | YES | 4788 |  |
| 0x65 | 0x65 | sub_20B80 | 0x0001 | YES | 4789 |  |
| 0x66 | 0x66 | sub_20B90 | 0x0000 | no | 4790 | data10=0 DISABLED |
| 0x67 | 0x67 | sub_20B90 | 0x0000 | no | 4791 | data10=0 DISABLED |
| 0x68 | 0x68 | sub_20B90 | 0x0000 | no | 4792 | data10=0 DISABLED |
| 0x69 | 0x69 | sub_20B90 | 0x0000 | no | 4793 | data10=0 DISABLED |
| 0x6A | 0x6A | sub_20B90 | 0x0000 | no | 4794 | data10=0 DISABLED |
| 0x6B | 0x6B | sub_20B90 | 0x0000 | no | 4795 | data10=0 DISABLED |
| 0x6C | 0x6C | sub_20BA0 | 0x0000 | no | 4796 | data10=0 DISABLED |
| 0x6D | 0x6D | sub_20BA0 | 0x0000 | no | 4797 | data10=0 DISABLED |
| 0x6E | 0x6E | sub_20BA0 | 0x0000 | no | 4798 | data10=0 DISABLED |
| 0x6F | 0x6F | sub_20BA0 | 0x0000 | no | 4799 | data10=0 DISABLED |
| 0x70 | 0x70 | sub_20BA0 | 0x0000 | no | 4800 | data10=0 DISABLED |
| 0x71 | 0x71 | sub_20BA0 | 0x0000 | no | 4801 | data10=0 DISABLED |
| 0x72 | 0x72 | sub_20BB0 | 0x0000 | no | 4802 | data10=0 DISABLED |
| 0x73 | 0x73 | sub_20BB0 | 0x0000 | no | 4803 | data10=0 DISABLED |
| 0x74 | 0x74 | sub_20BB0 | 0x0000 | no | 4804 | data10=0 DISABLED |
| 0x75 | 0x75 | sub_20BB0 | 0x0000 | no | 4805 | data10=0 DISABLED |
| 0x76 | 0x76 | sub_20BB0 | 0x0000 | no | 4806 | data10=0 DISABLED |
| 0x77 | 0x77 | sub_20BB0 | 0x0000 | no | 4807 | data10=0 DISABLED |
| 0x78 | 0x78 | sub_19550 | 0x0001 | YES | 4808 |  |
| 0x79 | 0x0 | nullptr | 0x0000 | no | 4809 | terminal sentinel |

### class 6  (dword_96902[6].str_0 = str_255574, def line 4812)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_31A90 | 0x0001 | YES | 4814 |  |
| 0x1 | 0x1 | sub_31A90 | 0x0001 | YES | 4815 |  |
| 0x2 | 0x0 | nullptr | 0x0000 | no | 4816 | terminal sentinel |

### class 7  (dword_96902[7].str_0 = str_2555CC, def line 4818)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_5B5D0 | 0x0001 | YES | 4820 |  |
| 0x1 | 0x1 | sub_5B5D0 | 0x0001 | YES | 4821 |  |
| 0x2 | 0x2 | sub_5B5D0 | 0x0001 | YES | 4822 |  |
| 0x3 | 0x3 | sub_5B5D0 | 0x0001 | YES | 4823 |  |
| 0x4 | 0x4 | sub_5B5D0 | 0x0001 | YES | 4824 |  |
| 0x5 | 0x0 | nullptr | 0x0000 | no | 4825 | terminal sentinel |

### class 8  (dword_96902[8].str_0 = str_255674, def line 4827)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_48650 | 0x0001 | YES | 4829 |  |
| 0x1 | 0x1 | sub_48650 | 0x0001 | YES | 4830 |  |
| 0x2 | 0x2 | sub_48650 | 0x0001 | YES | 4831 |  |
| 0x3 | 0x3 | sub_48650 | 0x0001 | YES | 4832 |  |
| 0x4 | 0x4 | sub_48650 | 0x0001 | YES | 4833 |  |
| 0x5 | 0x5 | sub_48650 | 0x0001 | YES | 4834 |  |
| 0x6 | 0x0 | nullptr | 0x0000 | no | 4835 | terminal sentinel |

### class 9  (dword_96902[9].str_0 = str_25573C, def line 4837)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_52B30_52E70 | 0x0001 | YES | 4839 |  |
| 0x1 | 0x1 | sub_52ED0_53210 | 0x0001 | YES | 4840 |  |
| 0x2 | 0x2 | sub_53060 | 0x0001 | YES | 4841 |  |
| 0x3 | 0x3 | sub_53070_533B0 | 0x0001 | YES | 4842 |  |
| 0x4 | 0x4 | sub_53060 | 0x0001 | YES | 4843 |  |
| 0x5 | 0x5 | sub_53060 | 0x0001 | YES | 4844 |  |
| 0x6 | 0x6 | sub_53060 | 0x0001 | YES | 4845 |  |
| 0x7 | 0x7 | sub_530B0 | 0x0001 | YES | 4846 |  |
| 0x8 | 0x8 | sub_530C0_53400 | 0x0001 | YES | 4847 |  |
| 0x9 | 0x9 | sub_535E0_53920 | 0x0001 | YES | 4848 |  |
| 0xA | 0xA | sub_53980_53CC0 | 0x0001 | YES | 4849 |  |
| 0xB | 0xB | sub_53060 | 0x0001 | YES | 4850 |  |
| 0xC | 0xC | sub_53DC0_54100 | 0x0001 | YES | 4851 |  |
| 0xD | 0xD | sub_54180_544D0 | 0x0001 | YES | 4852 |  |
| 0xE | 0x0 | nullptr | 0x0000 | no | 4853 | GAP data4=0x0!=pos => STATE-ID mismatch |

### class 10  (dword_96902[10].str_0 = str_255998, def line 4855)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_24F60 | 0x0001 | YES | 4857 |  |
| 0x1 | 0x1 | sub_25130 | 0x0001 | YES | 4858 |  |
| 0x2 | 0x2 | sub_252B0 | 0x0001 | YES | 4859 |  |
| 0x3 | 0x3 | sub_253F0 | 0x0001 | YES | 4860 |  |
| 0x4 | 0x4 | sub_25980 | 0x0001 | YES | 4861 |  |
| 0x5 | 0x5 | sub_25410 | 0x0001 | YES | 4862 |  |
| 0x6 | 0x6 | sub_252D0 | 0x0001 | YES | 4863 |  |
| 0x7 | 0x7 | sub_253E0 | 0x0001 | YES | 4864 |  |
| 0x8 | 0x8 | sub_253E0 | 0x0001 | YES | 4865 |  |
| 0x9 | 0x9 | sub_25470 | 0x0001 | YES | 4866 |  |
| 0xA | 0xA | sub_25570 | 0x0001 | YES | 4867 |  |
| 0xB | 0xB | sub_25670 | 0x0001 | YES | 4868 |  |
| 0xC | 0xC | sub_25760 | 0x0001 | YES | 4869 |  |
| 0xD | 0xD | sub_257B0 | 0x0001 | YES | 4870 |  |
| 0xE | 0xE | sub_258A0 | 0x0001 | YES | 4871 |  |
| 0xF | 0xF | sub_25990 | 0x0001 | YES | 4872 |  |
| 0x10 | 0x10 | sub_25A60 | 0x0001 | YES | 4873 |  |
| 0x11 | 0x11 | sub_25CE0 | 0x0001 | YES | 4874 |  |
| 0x12 | 0x12 | sub_25EC0 | 0x0001 | YES | 4875 |  |
| 0x13 | 0x13 | sub_26140 | 0x0001 | YES | 4876 |  |
| 0x14 | 0x14 | sub_262C0 | 0x0001 | YES | 4877 |  |
| 0x15 | 0x15 | sub_262C0 | 0x0001 | YES | 4878 |  |
| 0x16 | 0x16 | sub_262C0 | 0x0001 | YES | 4879 |  |
| 0x17 | 0x17 | sub_262D0 | 0x0001 | YES | 4880 |  |
| 0x18 | 0x18 | sub_26350 | 0x0001 | YES | 4881 |  |
| 0x19 | 0x19 | sub_26360 | 0x0001 | YES | 4882 |  |
| 0x1A | 0x1A | sub_263C0 | 0x0001 | YES | 4883 |  |
| 0x1B | 0x1B | sub_26670 | 0x0001 | YES | 4884 |  |
| 0x1C | 0x1C | sub_26560 | 0x0001 | YES | 4885 |  |
| 0x1D | 0x1D | sub_26760 | 0x0001 | YES | 4886 |  |
| 0x1E | 0x1E | sub_253E0 | 0x0001 | YES | 4887 |  |
| 0x1F | 0x1F | sub_253E0 | 0x0001 | YES | 4888 |  |
| 0x20 | 0x20 | sub_26890 | 0x0001 | YES | 4889 |  |
| 0x21 | 0x21 | sub_253E0 | 0x0001 | YES | 4890 |  |
| 0x22 | 0x22 | sub_26920 | 0x0001 | YES | 4891 |  |
| 0x23 | 0x23 | sub_26CE0 | 0x0001 | YES | 4892 |  |
| 0x24 | 0x24 | sub_26A60 | 0x0001 | YES | 4893 |  |
| 0x25 | 0x25 | sub_26C00 | 0x0001 | YES | 4894 |  |
| 0x26 | 0x26 | sub_26E90 | 0x0001 | YES | 4895 |  |
| 0x27 | 0x0 | nullptr | 0x0000 | no | 4896 | GAP data4=0x0!=pos => STATE-ID mismatch |
| 0x28 | 0x28 | sub_26D20 | 0x0001 | YES | 4897 |  |
| 0x29 | 0x29 | sub_27030 | 0x0001 | YES | 4898 |  |
| 0x2A | 0x2A | sub_275C0 | 0x0001 | YES | 4899 |  |
| 0x2B | 0x2B | sub_28200 | 0x0001 | YES | 4900 |  |
| 0x2C | 0x2C | sub_285C0 | 0x0001 | YES | 4901 |  |
| 0x2D | 0x2D | sub_293D0 | 0x0001 | YES | 4902 |  |
| 0x2E | 0x2E | sub_29540 | 0x0000 | no | 4903 | data10=0 DISABLED |
| 0x2F | 0x0 | nullptr | 0x0000 | no | 4904 | GAP data4=0x0!=pos => STATE-ID mismatch |
| 0x30 | 0x30 | sub_27D30 | 0x0001 | YES | 4905 |  |
| 0x31 | 0x0 | nullptr | 0x0000 | no | 4906 | GAP data4=0x0!=pos => STATE-ID mismatch |
| 0x32 | 0x0 | nullptr | 0x0000 | no | 4907 | GAP data4=0x0!=pos => STATE-ID mismatch |
| 0x33 | 0x33 | sub_27D30 | 0x0001 | YES | 4908 |  |
| 0x34 | 0x34 | sub_28DC0 | 0x0001 | YES | 4909 |  |
| 0x35 | 0x35 | sub_28FE0 | 0x0001 | YES | 4910 |  |
| 0x36 | 0x36 | sub_253E0 | 0x0001 | YES | 4911 |  |
| 0x37 | 0x37 | sub_269A0 | 0x0001 | YES | 4912 |  |
| 0x38 | 0x38 | sub_296A0 | 0x0001 | YES | 4913 |  |
| 0x39 | 0x39 | sub_29700 | 0x0001 | YES | 4914 |  |
| 0x3A | 0x3A | sub_29780 | 0x0001 | YES | 4915 |  |
| 0x3B | 0x3B | sub_29920_29960 | 0x0001 | YES | 4916 |  |
| 0x3C | 0x3C | sub_299D0_29A10 | 0x0001 | YES | 4917 |  |
| 0x3D | 0x3D | sub_29B70 | 0x0001 | YES | 4918 |  |
| 0x3E | 0x0 | nullptr | 0x0000 | no | 4919 | terminal sentinel |

### class 11  (dword_96902[11].str_0 = str_256038, def line 4920)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_59A80_59F90 | 0x0001 | YES | 4922 |  |
| 0x1 | 0x1 | sub_59AB0_59FC0 | 0x0001 | YES | 4923 |  |
| 0x2 | 0x2 | sub_59AE0_59FF0 | 0x0001 | YES | 4924 |  |
| 0x3 | 0x3 | sub_59B30_5A040 | 0x0001 | YES | 4925 |  |
| 0x4 | 0x4 | sub_59B80_5A090 | 0x0001 | YES | 4926 |  |
| 0x5 | 0x5 | sub_59C40_5A150 | 0x0001 | YES | 4927 |  |
| 0x6 | 0x6 | sub_59C70_5A180 | 0x0001 | YES | 4928 |  |
| 0x7 | 0x7 | sub_59CA0_5A1B0 | 0x0001 | YES | 4929 |  |
| 0x8 | 0x8 | sub_59CF0_5A200 | 0x0001 | YES | 4930 |  |
| 0x9 | 0x9 | sub_59D40_5A250 | 0x0001 | YES | 4931 |  |
| 0xA | 0xA | sub_59D70_5A280 | 0x0001 | YES | 4932 |  |
| 0xB | 0xB | sub_59DA0_5A2B0 | 0x0001 | YES | 4933 |  |
| 0xC | 0xC | sub_59DF0_5A300 | 0x0001 | YES | 4934 |  |
| 0xD | 0xD | sub_59F60_5A470 | 0x0001 | YES | 4935 |  |
| 0xE | 0xE | sub_59F70_5A480 | 0x0001 | YES | 4936 |  |
| 0xF | 0xF | sub_59F80_5A490 | 0x0001 | YES | 4937 |  |
| 0x10 | 0x10 | sub_59F90_5A4A0 | 0x0001 | YES | 4938 |  |
| 0x11 | 0x11 | sub_59FA0_5A4B0 | 0x0001 | YES | 4939 |  |
| 0x12 | 0x12 | sub_59FB0_5A4C0 | 0x0001 | YES | 4940 |  |
| 0x13 | 0x13 | sub_59FC0_5A4D0 | 0x0001 | YES | 4941 |  |
| 0x14 | 0x14 | sub_59FD0_5A4E0 | 0x0001 | YES | 4942 |  |
| 0x15 | 0x15 | sub_59FE0_5A4F0 | 0x0001 | YES | 4943 |  |
| 0x16 | 0x16 | sub_59FF0_5A500 | 0x0001 | YES | 4944 |  |
| 0x17 | 0x17 | sub_5A000_5A510 | 0x0001 | YES | 4945 |  |
| 0x18 | 0x18 | sub_5A010_5A520 | 0x0001 | YES | 4946 |  |
| 0x19 | 0x19 | sub_5A020_5A530 | 0x0001 | YES | 4947 |  |
| 0x1A | 0x1A | sub_5A030_5A540 | 0x0001 | YES | 4948 |  |
| 0x1B | 0x1B | sub_5A040_5A550 | 0x0001 | YES | 4949 |  |
| 0x1C | 0x1C | sub_5A050_5A560 | 0x0001 | YES | 4950 |  |
| 0x1D | 0x1D | sub_5A060_5A570 | 0x0001 | YES | 4951 |  |
| 0x1E | 0x1E | sub_5A070_5A580 | 0x0001 | YES | 4952 |  |
| 0x1F | 0x1F | sub_5A080 | 0x0001 | YES | 4953 |  |
| 0x20 | 0x0 | nullptr | 0x0000 | no | 4954 | terminal sentinel |

### class 12  (dword_96902[12].str_0 = str_2563D8, def line 4956)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_56090_565C0 | 0x0001 | YES | 4958 |  |
| 0x1 | 0x1 | sub_56250 | 0x0001 | YES | 4959 |  |
| 0x2 | 0x2 | sub_56260 | 0x0001 | YES | 4960 |  |
| 0x3 | 0x3 | sub_56270_567A0 | 0x0001 | YES | 4961 |  |
| 0x4 | 0x4 | sub_56250 | 0x0001 | YES | 4962 |  |
| 0x5 | 0x5 | sub_56260 | 0x0001 | YES | 4963 |  |
| 0x6 | 0x6 | sub_56380_568B0 | 0x0001 | YES | 4964 |  |
| 0x7 | 0x7 | sub_56250 | 0x0001 | YES | 4965 |  |
| 0x8 | 0x8 | sub_56260 | 0x0001 | YES | 4966 |  |
| 0x9 | 0x9 | sub_56510_56A40 | 0x0001 | YES | 4967 |  |
| 0xA | 0xA | sub_56250 | 0x0001 | YES | 4968 |  |
| 0xB | 0xB | sub_56260 | 0x0001 | YES | 4969 |  |
| 0xC | 0xC | sub_566C0 | 0x0001 | YES | 4970 |  |
| 0xD | 0xD | sub_56250 | 0x0001 | YES | 4971 |  |
| 0xE | 0xE | sub_56260 | 0x0001 | YES | 4972 |  |
| 0xF | 0xF | sub_56730 | 0x0001 | YES | 4973 |  |
| 0x10 | 0x10 | sub_56250 | 0x0001 | YES | 4974 |  |
| 0x11 | 0x11 | sub_56260 | 0x0001 | YES | 4975 |  |
| 0x12 | 0x12 | sub_567A0_56CD0 | 0x0001 | YES | 4976 |  |
| 0x13 | 0x13 | sub_56250 | 0x0001 | YES | 4977 |  |
| 0x14 | 0x14 | sub_56260 | 0x0001 | YES | 4978 |  |
| 0x15 | 0x15 | sub_56950_56E80 | 0x0001 | YES | 4979 |  |
| 0x16 | 0x16 | sub_56250 | 0x0001 | YES | 4980 |  |
| 0x17 | 0x17 | sub_56260 | 0x0001 | YES | 4981 |  |
| 0x18 | 0x18 | sub_56AF0_57020 | 0x0001 | YES | 4982 |  |
| 0x19 | 0x19 | sub_56250 | 0x0001 | YES | 4983 |  |
| 0x1A | 0x1A | sub_56260 | 0x0001 | YES | 4984 |  |
| 0x1B | 0x1B | sub_56CA0_571D0 | 0x0001 | YES | 4985 |  |
| 0x1C | 0x1C | sub_56250 | 0x0001 | YES | 4986 |  |
| 0x1D | 0x1D | sub_56260 | 0x0001 | YES | 4987 |  |
| 0x1E | 0x1E | sub_56E50_57380 | 0x0001 | YES | 4988 |  |
| 0x1F | 0x1F | sub_56250 | 0x0001 | YES | 4989 |  |
| 0x20 | 0x20 | sub_56260 | 0x0001 | YES | 4990 |  |
| 0x21 | 0x21 | sub_57040_57570 | 0x0001 | YES | 4991 |  |
| 0x22 | 0x22 | sub_56250 | 0x0001 | YES | 4992 |  |
| 0x23 | 0x23 | sub_56260 | 0x0001 | YES | 4993 |  |
| 0x24 | 0x24 | sub_571B0_576E0 | 0x0001 | YES | 4994 |  |
| 0x25 | 0x25 | sub_56250 | 0x0001 | YES | 4995 |  |
| 0x26 | 0x26 | sub_56260 | 0x0001 | YES | 4996 |  |
| 0x27 | 0x27 | sub_57250_57780 | 0x0001 | YES | 4997 |  |
| 0x28 | 0x28 | sub_56250 | 0x0001 | YES | 4998 |  |
| 0x29 | 0x29 | sub_56260 | 0x0001 | YES | 4999 |  |
| 0x2A | 0x2A | sub_573F0_57920 | 0x0001 | YES | 5000 |  |
| 0x2B | 0x2B | sub_56250 | 0x0001 | YES | 5001 |  |
| 0x2C | 0x2C | sub_56260 | 0x0001 | YES | 5002 |  |
| 0x2D | 0x2D | sub_57470_579A0 | 0x0001 | YES | 5003 |  |
| 0x2E | 0x2E | sub_56250 | 0x0001 | YES | 5004 |  |
| 0x2F | 0x2F | sub_56260 | 0x0001 | YES | 5005 |  |
| 0x30 | 0x30 | sub_57610_57B40 | 0x0001 | YES | 5006 |  |
| 0x31 | 0x31 | sub_56250 | 0x0001 | YES | 5007 |  |
| 0x32 | 0x32 | sub_56260 | 0x0001 | YES | 5008 |  |
| 0x33 | 0x33 | sub_57800_57D30 | 0x0001 | YES | 5009 |  |
| 0x34 | 0x34 | sub_56250 | 0x0001 | YES | 5010 |  |
| 0x35 | 0x35 | sub_56260 | 0x0001 | YES | 5011 |  |
| 0x36 | 0x36 | sub_579D0_57F00 | 0x0001 | YES | 5012 |  |
| 0x37 | 0x37 | sub_56250 | 0x0001 | YES | 5013 |  |
| 0x38 | 0x38 | sub_56260 | 0x0001 | YES | 5014 |  |
| 0x39 | 0x39 | sub_57B80_580B0 | 0x0001 | YES | 5015 |  |
| 0x3A | 0x3A | sub_56250 | 0x0001 | YES | 5016 |  |
| 0x3B | 0x3B | sub_56260 | 0x0001 | YES | 5017 |  |
| 0x3C | 0x3C | sub_57D40_58270 | 0x0001 | YES | 5018 |  |
| 0x3D | 0x3D | sub_56250 | 0x0001 | YES | 5019 |  |
| 0x3E | 0x3E | sub_56260 | 0x0001 | YES | 5020 |  |
| 0x3F | 0x3F | sub_57F00_58410 | 0x0001 | YES | 5021 |  |
| 0x40 | 0x40 | sub_56250 | 0x0001 | YES | 5022 |  |
| 0x41 | 0x41 | sub_56260 | 0x0001 | YES | 5023 |  |
| 0x42 | 0x42 | sub_580A0_585B0 | 0x0001 | YES | 5024 |  |
| 0x43 | 0x43 | sub_56250 | 0x0001 | YES | 5025 |  |
| 0x44 | 0x44 | sub_56260 | 0x0001 | YES | 5026 |  |
| 0x45 | 0x45 | sub_58240_58750 | 0x0001 | YES | 5027 |  |
| 0x46 | 0x46 | sub_56250 | 0x0001 | YES | 5028 |  |
| 0x47 | 0x47 | sub_56260 | 0x0001 | YES | 5029 |  |
| 0x48 | 0x0 | nullptr | 0x0000 | no | 5030 | terminal sentinel |

### class 13  (dword_96902[13].str_0 = str_256938, def line 5032)

| pos | state(data4) | handler(data6) | data10 | ticked? | line | note |
|----|----|----|----|----|----|----|
| 0x0 | 0x0 | sub_43AF0 | 0x0001 | YES | 5034 |  |
| 0x1 | 0x1 | sub_43AF0 | 0x0001 | YES | 5035 |  |
| 0x2 | 0x2 | sub_43AF0 | 0x0001 | YES | 5036 |  |
| 0x3 | 0x3 | sub_43AF0 | 0x0001 | YES | 5037 |  |
| 0x4 | 0x0 | nullptr | 0x0000 | no | 5038 | terminal sentinel |