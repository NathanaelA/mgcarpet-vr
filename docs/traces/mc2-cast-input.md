# MC2 CAST INPUT SEMANTICS — Verbatim Trace (mouse buttons → casts)

All `file:line` citations relative to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
Files: `EventsFunctions.cpp` (EF), `Events.cpp` (E), `PlayerInput.cpp` (PI), `Level.cpp` (L),
`sub_main_mouse.h`, `global_types.h`.

Cross-refs (do NOT re-derive):
- `docs/traces/mc2-player-cast-path.md` — the cast GATE `sub_5F660` (EF:60874), the ARM
  `sub_5F7B0` (EF:60973), the per-tick affordability/drain `sub_68D50`/`sub_68DE0`, and the
  effect-state spawn skeleton (fireball `sub_693F0` EF:55832 etc.) that reads the hand offsets.
  THIS doc traces the layer UPSTREAM of that: raw mouse → the button bitfield + the cadence flag.
- `crates/mgc-sim/src/mc2/spells.rs` — SPELLS.DAT layout (26 rows × 80 bytes, 3 tiers × 26 bytes);
  `font_type = tier byte @ offset 25`.

---

## 0. TL;DR — the four laws

1. **Press vs hold.** The raw mouse layer produces TWO signals per button: an **EDGE** word
   (`x_WORD_180746_mouse_left_button` / `…180744_right`) set only on the *first cycle after press*,
   and a **HELD** word (`x_WORD_18074C_mouse_left2_button` / `…18074A_right2`) set every cycle the
   button is down. These fold into `MouseButtonState_18059C`: bit1=left-edge, bit2=right-edge,
   bit4=left-held, bit8=right-held (EF:49676-49683). `HandleMouseButtons_18F80` (PI:2027) turns these
   into the player-record fire bits `entityIndex_0x0` **0x10** (left) / **0x20** (right) / **0x40**
   (quick-slot). The whole `entityIndex_0x0` byte is EDGE-CONSUMED per frame: it is re-zeroed
   at PI:504 each input pass, `|=`'d with the fire bit for that pass, copied to the player entity at
   EF:38064, then the input packet is `memset` to 0 at EF:38083.

2. **Cadence (click vs rapid).** Per spell-tier, encoded in SPELLS.DAT **`fontType_0x1B` bit 0**:
   `byte_0x3B_59 = (subspell[tier].fontType_0x1B & 1) == 0` (L:1519).
   - `byte_0x3B_59 == 1` (font bit0 CLEAR) → **CLICK-to-fire**: `HandleMouseButtons` fires the bit only
     on the EDGE state (PI:2045-2049 / 2065-2069) and immediately clears the state bit; one cast per
     physical press.
   - `byte_0x3B_59 == 0` (font bit0 SET) → **RAPID**: fires on EDGE **or** (HELD `& 4/8` **and** the
     spell entity is mid-cast `word_0x2E_46 > 0`) (PI:2051-2055 / 2071-2075); re-fires while held.
   Fireball font = `[0,1,0]` → tier0 CLICK, **tier1 RAPID ("Repeat Fireball")**, tier2 CLICK — this is
   exactly the player-observed behavior. Lightning font = `[1,0,0]` → tier0 RAPID, tiers1/2 CLICK. All
   other 24 spells (all tiers) = CLICK. (values §2.3, dumped from `baked/assets/mc2-day/spells.bin`.)

3. **Charge.** `byte_0x154_340` is a **free-running per-tick counter** on the player context
   (`dword_0xA4_164x`), `++` every player tick, clamped to 200 (EF:59991-59992, mirror EF:5424-5425).
   It is NOT gated on holding a button. On each projectile spawn the effect state copies it to the
   projectile's `dword_0x10_16` and **zeroes it** (e.g. fireball EF:55869-55870). So it is a
   *time-since-last-cast* value (cadence/aging), NOT a hold-to-charge accumulator. No spell has
   hold-to-charge-release-to-fire. (§3.)

4. **Hand launch offset.** The projectile yaw/pitch are `caster.yaw + dword_0xA4_164x->nextEntity_0x18_24`
   and `caster.pitch + …entityIndex2_0x1A_26` (EF:55867-55868). Those two fields are the **crosshair /
   aim offset**, written from the mouse position via `x_DWORD_180590` / `x_DWORD_180594`
   (PI:2139-2140, copied to the player at EF:38065-38066). **They do NOT depend on which button fired** —
   left and right cast along the identical aim vector. The observed left-side/right-side muzzle is the
   RENDER-side hand animation driven by the recorded button bit (256/512 → caster
   `struct_byte_0xc_12_15` bits, `sub_5F7B0` EF:60977-60978); the simulation launch point is
   `caster.position` + z-lift `array_0x52_82.fov` (EF:55866) for both buttons. (§4 — the sim-side
   left/right launch-side is a render concern, flagged OPEN.)

---

## 1. INPUT → BUTTON-BIT LAYER

### 1.1 Raw mouse → the two-word (edge/held) representation — `UpdateMouseEventData_8CB3A` (EF:51417)

The platform entry `MouseEvents(buttons,x,y)` (EF:51403) forwards to `UpdateMouseEventData_8CB3A`,
which reads the incoming button bitmask `x_DWORD_180710_mouse_buttons_states` (`= mouse_states`,
EF:51438) and produces the four button words. VERBATIM (EF:51464-51501):
```c
if (x_DWORD_180710_mouse_buttons_states & 2)   // left button pressed
{
    if (!x_WORD_18074C_mouse_left2_button) { … }
    if (!x_WORD_18074C_mouse_left2_button && !x_WORD_180746_mouse_left_button)  // first cycle after press
    {
        x_WORD_180746_mouse_left_button = 1;         // EDGE word: set once
        x_WORD_E375C_mouse_position_x = mouse_posx;  // latch cursor at press
        x_WORD_E375E_mouse_position_y = mouse_posy;
    }
    x_WORD_18074C_mouse_left2_button = 1;            // HELD word: set every cycle down
}
if (x_DWORD_180710_mouse_buttons_states & 4)         // left button released
    x_WORD_18074C_mouse_left2_button = 0;            // clear HELD on release
if (x_DWORD_180710_mouse_buttons_states & 8)         // right button pressed
{
    if (!x_WORD_18074A_mouse_right2_button && !x_WORD_180744_mouse_right_button)  // first cycle
    {
        x_WORD_180744_mouse_right_button = 1;        // EDGE word (right)
        x_WORD_E375C_mouse_position_x = mouse_posx;
        x_WORD_E375E_mouse_position_y = mouse_posy;
    }
    x_WORD_18074A_mouse_right2_button = 1;           // HELD word (right)
}
if (x_DWORD_180710_mouse_buttons_states & 0x10)      // right button released
    x_WORD_18074A_mouse_right2_button = 0;
```
So: **`…_button` (180746/180744) = EDGE** (set only on the press transition, because the `!left2`
gate means it can only fire while HELD is still 0, i.e. the very first frame). **`…2_button`
(18074C/18074A) = HELD** (set while down, cleared on the release event). The incoming bitmask packs
press=bit for-each-button + separate release bits (2/4 left, 8/0x10 right). The EDGE word stays 1
until a *consumer* clears it (see §1.3); it is NOT auto-cleared each frame. The HELD word tracks the
physical button.

The same "first cycle after press" idiom appears in the debug/replay injector at EF:31631-31640
(comment `//first cycle after press and ...`) confirming the reading.

### 1.2 The two words → `MouseButtonState_18059C` bits — `ProcessInput` snapshot (EF:49675-49685)

Each input tick the four words are OR-folded into the frame state byte. VERBATIM:
```c
unk_18058Cstr.MouseButtonState_18059C = 0;
if (x_WORD_180746_mouse_left_button)   unk_18058Cstr.MouseButtonState_18059C  = 1;   // bit0 left EDGE
if (x_WORD_180744_mouse_right_button)  unk_18058Cstr.MouseButtonState_18059C |= 2;   // bit1 right EDGE
if (x_WORD_18074C_mouse_left2_button)  unk_18058Cstr.MouseButtonState_18059C |= 4;   // bit2 left HELD
if (x_WORD_18074A_mouse_right2_button) unk_18058Cstr.MouseButtonState_18059C |= 8;   // bit3 right HELD
if (pressedKeys_180664[x_BYTE_EB39E_keys[5]]) …                                |= 0x10;// bit4 CTRL
```
(bits: **1=left click-edge, 2=right click-edge, 4=left held, 8=right held, 0x10=CTRL**.)

### 1.3 `MouseButtonState` → the player fire bits — `HandleMouseButtons_18F80` (PI:2027)

Called from the in-game input loop (PI:635, PI:985; and E:889 for the recorded-input path). VERBATIM
(PI:2037-2076):
```c
if (a1x->…str_611.SpellIndexLeft_0x451_1105 == -1)          // no spell bound left
    unk_18058Cstr.MouseButtonState_18059C &= 0xFE;          //   swallow left
else {
    if (Entities_EA3E4[ …SpellEnabled[SpellIndexLeft] ]->byte_0x3B_59 == 1)   // CLICK spell
    {
        if (unk_18058Cstr.MouseButtonState_18059C & 1)      // only the EDGE bit
        {
            HandleButtonClick_191B0(6, 16);                 // set player fire bit 0x10
            unk_18058Cstr.MouseButtonState_18059C &= 0xFE;  // consume the edge → 1 cast/press
        }
    }
    else if (unk_18058Cstr.MouseButtonState_18059C & 1      // RAPID spell: EDGE …
          || unk_18058Cstr.MouseButtonState_18059C & 4      //   … OR HELD …
             && Entities_EA3E4[ …SpellEnabled[SpellIndexLeft] ]->word_0x2E_46 > 0)  // … while cast active
    {
        HandleButtonClick_191B0(6, 16);
        unk_18058Cstr.MouseButtonState_18059C &= 0xFE;
    }
}
// … right side identical with SpellIndexRight, bit 2 (edge) / bit 8 (held), fire bit 32 (0x20) …
```
`HandleButtonClick_191B0(6, loSetting)` (PI:1056, case 5/6) sets the fire bit on the input packet,
VERBATIM (PI:1079-1085):
```c
case 5:
case 6://set Player movement
    if (…PlayerAction_byte0 != hiSetting && …PlayerAction_byte0) return;   // don't clobber a pending action
    D41A0_0.playerInputs_0x6E3E[LevelIndex_0xc].PlayerAction_byte0 = hiSetting;   // = 6
    D41A0_0.playerInputs_0x6E3E[LevelIndex_0xc].entityIndex_0x6E3E_byte5 |= loSetting;   // |= 0x10 / 0x20
    return;
```
The quick-slot (0x40) fire comes from `HandleButtonClick_191B0(6, 64)` at PI:882 (both-buttons /
CTRL combo). Arrow-key "fire" is `(6, 128)` at PI:2087 (keyboard). The movement bits 1/2/4/8 come from
`HandleButtonClick_191B0(6, 1..8)` at PI:2030-2036 (arrow keys via `x_WORD_1805C0_arrow_keys`).

### 1.4 Input packet → player entity, and the EDGE-CONSUME reset — `sub_5DFB0`/dispatcher (EF:38064)

VERBATIM (EF:38064-38066, EF:38083):
```c
actEvent->dword_0xA4_164x->entityIndex_0x0     = playerInputs_0x6E3E[i].entityIndex_0x6E3E_byte5;  // fire bits
actEvent->dword_0xA4_164x->nextEntity_0x18_24  = playerInputs_0x6E3E[i].nextEntity_0x6E3E_word6;   // aim yaw off
actEvent->dword_0xA4_164x->entityIndex2_0x1A_26= playerInputs_0x6E3E[i].entityIndex2_0x6E3E_word8; // aim pitch off
…
memset(&D41A0_0.playerInputs_0x6E3E[i], 0, 10);   // WHOLE packet cleared each dispatch
```
and the packet's fire byte is separately zeroed at the top of the per-player input pass
(PI:504 `…entityIndex_0x6E3E_byte5 = 0;`).

**Verdict — the fire bits (`entityIndex_0x0` 0x10/0x20/0x40) are EDGE / per-frame:** they are rebuilt
from scratch every input pass (zeroed → OR'd with whatever `HandleMouseButtons` decides *this* pass →
consumed → packet memset). They are NOT a latched held-state. HOLD is represented only indirectly, and
only for RAPID spells, via the `MouseButtonState & 4/8` (held word) branch that keeps re-emitting the
edge bit while `word_0x2E_46 > 0`.

### 1.5 The physical-release clears (the CLICK re-arm requirement)

The EDGE words are also force-cleared when the physical button is up, `ProcessKeyboardPresses` tail
(PI:1049-1052), VERBATIM:
```c
if (!(unk_18058Cstr.MouseButtonState_18059C & 1)) x_WORD_180746_mouse_left_button  = 0;
if (!(unk_18058Cstr.MouseButtonState_18059C & 2)) x_WORD_180744_mouse_right_button = 0;
```
and `sub_7A060` (EF:46060-46063) zeroes all four words after a menu poll. Because the EDGE word can
only re-arm when BOTH it and the HELD word are 0 (§1.1), a CLICK spell requires a genuine
release-then-press to fire again.

---

## 2. THE REPEAT / CADENCE LAW

### 2.1 What actually gates re-fire

Two independent gates in series:

1. **Input gate — `HandleMouseButtons_18F80` (§1.3)**: `byte_0x3B_59` selects EDGE-only (CLICK) vs
   EDGE|HELD (RAPID). For RAPID the HELD branch is additionally conditioned on the spell entity being
   mid-cast (`word_0x2E_46 > 0`), so auto-repeat only continues while the previous cast is still
   running — it re-arms the moment the cast advances.

2. **Sim gate — `sub_5F660` per-model switch (EF:60893-60952)**: even when a fire bit arrives, the gate
   refuses to re-arm most spells while a cast is already in flight. Reconciled per model below.

### 2.2 `sub_5F660` per-model precondition switch (EF:60893-60952) — reconciled

`sub_5F380` (EF:60748) dispatches the fire bit to the gate (EF:60851-60862):
```c
if (…entityIndex_0x0 & 0x10) sub_5F660(player, Entities[…SpellEnabled[SpellIndexLeft ]], 256);
if (…entityIndex_0x0 & 0x20) sub_5F660(player, Entities[…SpellEnabled[SpellIndexRight]], 512);
if (…entityIndex_0x0 & 0x40) sub_5F660(player, Entities[…SpellEnabled[quickSlot]], 256);
```
`sub_5F660(caster a1x, spellEntity a2x, bit a3)`; model-1 caster forces `v3=0,v5=1` first
(EF:60888-60892). Switch on `a2x->model_0x40_64`:

| model(s) | switch behavior (EF) | meaning |
|---|---|---|
| **0 fireball** | `if (byte_0x46_70 < 2) break; else LABEL_16` (60895-60898) | tier<2 → fall to mana gate & (re)arm; tier==2 → LABEL_16 (arm only if idle) |
| **1 posses** | `if (word_0x2E_46<=0) break; else {byte_0x3C_60=1; …dword\|=v3; sub_5F7E0; v7=1; goto LABEL_23}` (60899-60907) | if a cast is active, just re-stamp the button bit (no re-arm); else break→arm |
| **2 castle** | `if (word_0x2E_46<=0) break; else {fail-sound 29; goto LABEL_23}` (60908-60913) | can't recast castle while active |
| **4,6,8,0xB,0xC,0xE** | `if (model!=0 goto LABEL_16); if (word_0x2E_46<=0) break; else word_0x2E_46 = (model==4?7:1); goto LABEL_23` (60914-60928) | non-wizard caster→LABEL_16; wizard→RETRIGGER active cast (reset timer) instead of re-arm |
| **7 lightning** | `if (byte_0x46_70 < 1 \|\| !word_0x2E_46) break; else goto LABEL_23` (60929-60932) | tier0 → break→arm; tier≥1 while active → LABEL_23 (no re-arm) |
| **9,0xA,0xD,0xF,0x10..0x18** LABEL_16 | `if (word_0x2E_46) goto LABEL_23; else break` (60946-60948) | arm ONLY if idle; refuse while cast active |
| default | break (60950) | fall to mana gate |

Then the mana gate (EF:60953-60961): `if (mana < maxMana) v6=1; else { sub_5F7B0(arm); v7=1; }`.
`break` reaches the gate (arms if affordable); `goto LABEL_23` skips it (no arm).

### 2.3 The complete per-spell cadence table

`byte_0x3B_59` (the input-gate flag) = `(fontType_0x1B & 1)==0`. Values below dumped verbatim from
`baked/assets/mc2-day/spells.bin` (identical across cave/night bundles for these fields):

| model | spell | fontType [t0,t1,t2] | byte_0x3B_59 (1=CLICK) | ⇒ cadence per tier | word_0x18 (cast dur/divisor) [t0,t1,t2] |
|---|---|---|---|---|---|
| 0 | **fireball** | [0,1,0] | [1,0,1] | **t0 CLICK · t1 RAPID (Repeat Fireball) · t2 CLICK** | [5,11,15] |
| 1 | posses | [0,0,0] | [1,1,1] | CLICK all | [3,41,51] |
| 2 | castle | [0,0,0] | [1,1,1] | CLICK all | [101,101,101] |
| 3 | speed_up | [0,0,0] | [1,1,1] | CLICK all | [301,451,501] |
| 4 | metamorph | [0,0,0] | [1,1,1] | CLICK all | [201,301,455] |
| 5 | heal | [0,0,0] | [1,1,1] | CLICK all | [11,21,41] |
| 6 | shield | [0,0,0] | [1,1,1] | CLICK all | [101,201,301] |
| 7 | **lightning** | [1,0,0] | [0,1,1] | **t0 RAPID · t1 CLICK · t2 CLICK** | [5,35,45] |
| 8 | rebound | [0,0,0] | [1,1,1] | CLICK all | [125,251,125] |
| 9 | meteor | [0,0,0] | [1,1,1] | CLICK all | [3,7,11] |
| 10 | teleport | [0,0,0] | [1,1,1] | CLICK all | [11,11,11] |
| 11 | invisible | [0,0,0] | [1,1,1] | CLICK all | [181,183,183] |
| 12 | beyond_sight | [0,0,0] | [1,1,1] | CLICK all | [151,261,361] |
| 13 | steal_mana | [0,0,0] | [1,1,1] | CLICK all | [5,7,21] |
| 14 | duel | [0,0,0] | [1,1,1] | CLICK all | [195,395,603] |
| 15 | tremor | [0,0,0] | [1,1,1] | CLICK all | [31,41,61] |
| 16 | crater | [0,0,0] | [1,1,1] | CLICK all | [31,41,51] |
| 17 | earthquake | [0,0,0] | [1,1,1] | CLICK all | [21,31,41] |
| 18 | volcano | [0,0,0] | [1,1,1] | CLICK all | [23,33,43] |
| 19 | summon_army | [0,0,0] | [1,1,1] | CLICK all | [299,399,499] |
| 20 | gravity_well | [0,0,0] | [1,1,1] | CLICK all | [11,21,27] |
| 21 | whirlwind | [0,0,0] | [1,1,1] | CLICK all | [7,11,21] |
| 22 | fools_mana | [0,0,0] | [1,1,1] | CLICK all | [25,25,25] |
| 23 | magic_mine | [0,0,0] | [1,1,1] | CLICK all | [47,67,87] |
| 24 | alliance | [0,0,0] | [1,1,1] | CLICK all | [23,29,63] |
| 25 | cave_in | [0,0,0] | [1,1,1] | CLICK all | [31,41,51] |

**Only 2 spells have any RAPID tier: fireball tier1 and lightning tier0.** Everything else is
click-to-fire on every tier — matching the player's "each spell is either rapid-fire or not, and it's a
property of specific tiers" observation. (For fireball the fast tier is the *middle* tier — the
project's "Repeat Fireball" — with the tier2 upgrade reverting to a slower/stronger single-shot.)

### 2.4 Why fireball t0 is single but t1 rapid — the exact mechanism

Both the input gate AND the sim gate cooperate:
- **Input gate:** t0 has `byte_0x3B_59==1` → `HandleMouseButtons` fires only on the EDGE bit and clears
  it (PI:2045-2049). Holding does nothing until release+re-press. t1 has `byte_0x3B_59==0` → the HELD
  branch (PI:2051) re-emits the fire bit each frame while `word_0x2E_46>0`.
- **Sim gate:** fireball `case 0` is `if (byte_0x46_70<2) break` (EF:60896) → for t0 and t1 it *breaks*
  to the mana gate and re-arms `word_0x2E_46 = word_0x30_48` (EF:60976) whenever a fire bit arrives.
  So the sim gate does NOT block fireball re-arm at t0/t1 — the click-vs-rapid difference is entirely
  the **input gate `byte_0x3B_59`**. (For t2, `byte_0x46_70==2` → LABEL_16 → arms only when idle;
  combined with CLICK input that is one shot per press.)
- Cast length: t0 `word_0x18=5` ticks, t1 `=11`. RAPID at t1 re-fires roughly every `word_0x18` ticks
  (the earliest the HELD branch can re-arm is once the previous cast advances / mana allows), giving the
  visibly fast stream. `word_0x36_54` (a separate cooldown counter, decremented each effect tick,
  EF:55895) is NOT the fireball repeat gate — it is not tested by `sub_5F660`.

### 2.5 The `word_0x2E_46` window and `word_0x36_54`

- `word_0x2E_46` = cast-in-progress timer, armed to `word_0x30_48` (= `subspell.word_0x18`) by
  `sub_5F7B0` (EF:60976); the effect state spawns only on the first tick (`word_0x2E_46 == word_0x30_48`)
  and decrements each tick (EF:55889). It is BOTH the "cast active" flag the RAPID HELD branch and the
  gate test against.
- `word_0x36_54` = a per-spell cooldown counter decremented at every effect-state tail (EF:55895 etc.),
  set to 64 on pickup. It gates certain effect-state internals but is NOT consulted by the input gate or
  `sub_5F660`; it is not the re-fire throttle. (Confirmed: no `word_0x36_54` read in PI:2027 or
  EF:60874-60969.)

---

## 3. THE CHARGE MECHANIC — `byte_0x154_340`

**Every writer of `byte_0x154_340`** (all on `dword_0xA4_164x`, the player context):
- **Accumulate:** EF:59991-59992 (in `AddPlayer03_00_5E010`, the per-tick player update) and the mirror
  EF:5424-5425 (`sub_146F0`-adjacent player tick). VERBATIM:
  ```c
  if (a1x->dword_0xA4_164x->byte_0x154_340 < 200)
      a1x->dword_0xA4_164x->byte_0x154_340++;
  ```
  Runs **unconditionally every non-paused player tick**, clamped at 200. **NOT gated on any button.**
- **Consume+zero on spawn:** every projectile effect state copies it to the projectile's
  `dword_0x10_16` then zeroes it — fireball EF:55869-55870, castle-adjacent EF:55975, meteor
  EF:56814-56815, lightning EF:56620-56621 / 56673-56674, and the class-10 impact effects at
  EF:56058, 57211-57212, 57380-57381, 57453-57454, 57525-57526, 57595-57596, 57671-57672,
  57748-57749, 57825-57826, 57994-57995, 58075-58076, 58159-58160. VERBATIM (fireball):
  ```c
  v6x->dword_0x10_16 = v1x->dword_0xA4_164x->byte_0x154_340;
  v1x->dword_0xA4_164x->byte_0x154_340 = 0;
  ```
  (Possess writes `dword_0x10_16` from a squared subspell payload instead and just zeroes the counter,
  EF:55974-55975 — it does not carry the charge value.)

**Interpretation.** `byte_0x154_340` is a **free-running tick counter = time since the last cast**
(reset to 0 at each spawn, grows while not casting, capped 200). It is copied onto the projectile as
`dword_0x10_16`. It is **not** a hold-to-charge accumulator: holding a button does not feed it, and no
spell has hold-to-charge-release-to-fire. What `dword_0x10_16` does on the projectile is out of scope
here (a per-projectile age/scale field — see class-9 trace); the input-side law is: **charge = tick age
of the caster, auto-reset per cast, button-independent.**

---

## 4. THE HAND / LAUNCH OFFSETS

### 4.1 What writes the offsets

The projectile launch angle is `caster facing + hand offset` (fireball EF:55867-55868):
```c
v6x->yaw_0x1C_28   = v1x->dword_0xA4_164x->nextEntity_0x18_24  + v1x->yaw_0x1C_28;
v6x->pitch_0x1E_30 = v1x->dword_0xA4_164x->entityIndex2_0x1A_26 + v1x->pitch_0x1E_30;
```
`nextEntity_0x18_24` / `entityIndex2_0x1A_26` are written on the player entity at EF:38065-38066 from the
input packet fields `nextEntity_0x6E3E_word6` / `entityIndex2_0x6E3E_word8`, which
`ComputeMousePlayerMovement_17060` fills from the crosshair globals (PI:2139-2140):
```c
D41A0_0.playerInputs_0x6E3E[LevelIndex_0xc].nextEntity_0x6E3E_word6   = unk_18058Cstr.x_DWORD_180590;
D41A0_0.playerInputs_0x6E3E[LevelIndex_0xc].entityIndex2_0x6E3E_word8 = unk_18058Cstr.x_DWORD_180594;
```
`x_DWORD_180590` (yaw) / `x_DWORD_180594` (pitch) are the **aim / crosshair offset**, derived from the
mouse/joystick sample (EF:49688 `x_DWORD_180590 = (x_DWORD_180590 << 11) / 360`, and the head-tracker
path EF:50563-50589). They encode where the crosshair points relative to straight-ahead — **the same
value regardless of which mouse button fired.** The cursor position is latched at press time
(EF:51470-51471 / 51494-51495 write `x_WORD_E375C/E375E` on the first press cycle).

### 4.2 Muzzle z-lift

`v6x->position_0x4C_76.z += v1x->array_0x52_82.fov;` (EF:55866) — the projectile spawns at the caster's
position raised by `array_0x52_82.fov` (`axis_4d array_0x52_82`, global_types.h:372). Same for both
buttons.

### 4.3 Left vs right launch SIDE — where it actually comes from

`sub_5F7B0` records which button armed the cast on the CASTER (EF:60977-60978):
```c
a2x->struct_byte_0xc_12_15.byte[1] &= 0xFCu;   // clear low 2 bits of byte[1]
a2x->struct_byte_0xc_12_15.dword |= a3;        // a3 = 256 (left, 0x100) or 512 (right, 0x200)
```
256 = byte[1] bit0, 512 = byte[1] bit1. This flag is consumed **render-side** (the wizard's
hand/wand animation and the muzzle-flash draw), not by the class-15 effect state — the effect state's
yaw/pitch (§4.1) has NO button-dependent term and no `byte[1]&0x100/0x200` read. The local-player
muzzle sprite is `SetEntityIndex_49C90(proj, 42)` (EF:55877-55878) regardless of side.

**Conclusion:** in the SIMULATION, left and right casts launch from the same point along the same aim
vector; the player-observed left-side/right-side projectile is a **presentation** effect keyed off the
recorded 256/512 button bit (the animating hand). **OPEN:** the exact render mapping of
`struct_byte_0xc_12_15` bits0/1 → left/right hand frame + on-screen muzzle x-offset was not located in
the class-15/sim files (it lives in the GameRender* / Animation wand-draw path); the sim-side port
should record the button bit onto the caster and mirror the offsets symmetrically, deferring the
visible side to the renderer. For a faithful *feel*, if a sim-side launch x-offset is desired, it must
be derived from the 256/512 bit — there is no such offset in the traced effect-state math.

---

## 5. SPELLS.DAT cadence fields

| tier field (offset in 26-byte tier) | name | role in cadence |
|---|---|---|
| +22 `word_0x18` (i16) | cast duration / mana divisor | `word_0x30_48`; # of ticks the cast runs; also `mana = maxMana / word_0x18`. Governs *how often* a RAPID spell can re-fire (earliest re-arm ≈ each cast completion). |
| +25 `fontType_0x1B` (u8) | **the cadence flag** | bit0 → `byte_0x3B_59 = (fontType&1)==0` (L:1519) → CLICK(1) vs RAPID(0) in `HandleMouseButtons`. **This is the field that encodes rapid-fire.** Remaining bits of `fontType_0x1B` unused by the cast path (name suggests a UI font index; only bit0 is read by the sim — OPEN whether other bits matter to rendering). |
| +24 `life_0x1A` (i8) | charge/tier level | carried to projectile (`byte_0x46_70`), drives twin-shot / charged-subtype selection in `sub_6DCA0` (cast-path trace §2); NOT a cadence field. |
| +0 `byte_0` (row, not tier) | tier count | `SetSpell` clamps chosen tier to `byte_0-1` (L:1508); all rows = 3. Not cadence. |
| +20 `hintText_0x16` (i16) | hint string id | shown at tier-SELECT, not at fire; no cadence role (cast-path trace §4). |

No other SPELLS.DAT field feeds re-fire rate. Confirmed: `HandleMouseButtons` reads only
`byte_0x3B_59`; `sub_5F660` reads only `byte_0x46_70` (tier) and `word_0x2E_46`; the effect states read
`word_0x30_48` (= `word_0x18`).

---

## 6. OPEN / uncertain

- **Render-side left/right hand + muzzle x-offset** (§4.3): the `struct_byte_0xc_12_15` bit0/1 → hand
  animation frame + on-screen muzzle mapping was not located in the sim/effect files. The sim launch is
  side-independent; the observed left/right side is presentation. Locate in GameRender*/Animation before
  claiming a sim-side x-offset.
- **`x_DWORD_180590/180594` full derivation for plain mouse**: traced through the joystick/head-tracker
  branch (EF:49688, EF:50563-50589) and the crosshair latch; the exact plain-2D-mouse yaw/pitch scaling
  (vs the `<<11 / 360` and `*4` seen) may differ per input device (`x_WORD_1805C2_joystick` case). The
  crosshair→offset scale for the default mouse device is device-branch dependent — pin the active branch
  when porting aim.
- **`fontType_0x1B` upper bits**: only bit0 is read by the cast path; whether bits 1..7 select a UI/font
  variant is unconfirmed (name is a guess).
- **`dword_0x10_16` on the projectile** (what the charge value does downstream): out of scope here; see
  class-9 projectile trace.
- **`word_0x36_54`** semantics beyond "decrement each effect tick" (its 64-on-pickup init and which
  effect-internal branch reads it) not fully transcribed; confirmed it is NOT the input/gate re-fire
  throttle.
