# MC2 HUD HAND-PANEL / EQUIPPED-SPELL ICONS — Verbatim Trace (remc2)

All `file:line` citations relative to
`/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
Files: `EventsFunctions.cpp` (EF), `GameUI.cpp` (UI), `Level.cpp` (L), `Basic.cpp` (B),
`GameBitmapIndexes.h` (GBI).

Companion trace: `docs/traces/mc2-spell-selector-ui.md` (the CTRL selector; its box/icon ids
were HSPR 87..91 / 97+ / 179+). **This trace corrects the mental model**: the CTRL-pane grid
icons and the equipped-hand-panel icons come from the **same bank** but are **two different
icon runs** — SMALL (97+) for the grid, BIG (123+) for the hands.

---

## TL;DR (the four answers)

- **Draw site**: the two equipped-spell hand panels are drawn by
  `DrawSpellIcon_2E260` (UI:341), called TWICE per frame from `DrawGameFrame_2BE30`
  (EF:21750 LEFT, EF:21758 RIGHT) at fixed x positions `spellLeftPosX` / `spellRightPosX`,
  `y = 2*scale`. It is gated by the "top bar" display option, `m_Display.m_wTopBar` (EF:21746).
  A second copy for the MAP screen is at EF:21977 / EF:21984.
- **Icon id formula** (UI:374, verbatim):
  `(*filearray_2aa18c[filearrayindex_MSPRD00DATTAB].posistruct)[playerEvent->model_0x40_64 + SPELL_FIREBALL_BIG]`
  i.e. **`sprite_id = SPELL_FIREBALL_BIG + spell_model_index` = `123 + model`** (GBI:29 `SPELL_FIREBALL_BIG=123`).
  The CTRL-pane grid uses `SPELL_FIREBALL_SMALL + idx` = `97 + idx` (EF:22542, GBI:28) — a DIFFERENT run.
- **Asset file**: filearray index 4 = `filearrayindex_MSPRD00DATTAB` (B:148). Its buffer is
  swapped between the **lowres MSPR** bank and the **hires HSPR** bank at map load
  (`LoadSpr_47160`, L:1017). Per-map-type names: day `MSPRD0-0`/`HSPRD0-0`, night `MSPRN0-0`/
  `HSPRN0-0`, cave `MSPRC0-0`/`HSPRC0-0` (L:1019-1039).
- **Lo/hi selection law** (L:1046): `if (x_WORD_180660_VGA_type_resolution == 1)` → load the
  **M**SPR (lowres/320-wide) bank into index 4 (L:1048-1049); **else** load the **H**SPR
  (hires) bank into index 4 (L:1058-1063). So the "two flavours" the player sees are the
  `M…`(lowres) vs `H…`(hires) DAT/TAB pair, chosen purely by the VGA resolution flag.
- **Crosshair**: NONE. The flight view draws no center reticle sprite. Only a mouse pointer,
  from `DATA/POINTERS.DAT` (filearray index 0), and only while a menu is open. (see §5)

---

## 1. The draw site — `DrawGameFrame_2BE30` (EF), flight branch (case 5)

`DrawGameFrame_2BE30` at EF:21496. Per-frame X positions computed at entry (EF:21498-21514):

```c
int16_t spellLeftPosX = 510;      // EF:21498  (default 640-wide layout)
int16_t spellRightPosX = 574;     // EF:21499
uint8_t scale = 1;                // EF:21500
...
if (x_WORD_180660_VGA_type_resolution != 1) {       // EF:21507  (not the 320x200 mode)
    if (!DefaultResolutions()) {                     // EF:21509  (non-standard = scaled UI)
        scale = gameUiScale;                          // EF:21511
        spellLeftPosX  = screenWidth_18062C - (130 * scale);  // EF:21512  right-anchored
        spellRightPosX = screenWidth_18062C - (66  * scale);  // EF:21513
    }
}
```

So in the canonical 640x480 layout the LEFT box sits at x=510, the RIGHT at x=574 (64px apart =
one panel width); in a scaled/wide window both are anchored to the right edge
(`screenWidth − 130*scale` and `screenWidth − 66*scale`). Both boxes are at `y = 2*scale`.

The flight-view draw calls, gated by the top-bar option (EF:21746 `if (m_Display.m_wTopBar)`):

```c
//Left                                                  // EF:21749-21755
DrawSpellIcon_2E260(
    spellLeftPosX, 2 * scale,
    Entities_EA3E4[playerEntity->dword_0xA4_164x->str_611.SpellsEnabled_0x333_819x.SpellEnabled[
        playerEntity->dword_0xA4_164x->str_611.SpellIndexLeft_0x451_1105]],
    false, scale);
//Right                                                 // EF:21757-21763
DrawSpellIcon_2E260(
    spellRightPosX, 2 * scale,
    Entities_EA3E4[…SpellEnabled[…SpellIndexRight_0x453_1107]],
    false, scale);
DrawTopStatusBar_2D710(playerEntity, scale);            // EF:21765  (mini-map + castle/player HP+mana)
```

The 3rd arg is the **live class-15 spell entity** for the LEFT/RIGHT-bound spell: it dereferences
`SpellEnabled[SpellIndexLeft]` (the possessed-entity index for the spell bound to LMB) — see the
selector trace §0 for `SpellIndexLeft_0x451_1105` / `SpellIndexRight_0x453_1107` and `SpellEnabled[]`.
`altDrawFunction=false`. If the bound spell is `-1`/unpossessed the entity resolves to
`Entities_EA3E4[0]` and `DrawSpellIcon_2E260` early-outs (UI:343 `if (playerEvent > Entities_EA3E4[0])`).

**Map-screen copy** (EF:21976-21989, MenuState 6/7/8 branch): identical calls at
`spellLeftPosX,2` / `spellRightPosX,2`, each guarded by
`x_D41A0_BYTEARRAY_4_struct.leftSpellPlayerIndex_38400` / `…rightSpellPlayerIndex_38401`.

---

## 2. `DrawSpellIcon_2E260` — the panel painter (UI:341-407)

This draws ONE equipped-spell box: frame bar + big icon + level numeral + mana meter + a
"cannot afford / regen" tint. Structure:

### 2a. The two draw modes (UI:351-354 gate)
The top of the function branches on whether the spell is "active/flashing":
```c
if (!(SPELLS_BEGIN_BUFFER_str[playerEvent->model_0x40_64].isEnabled_1 & 4)   // UI:351
    || playerEvent->word_0x2E_46 <= 0 || playerEvent->word_0x2E_46 >= 32     // cast timer out of (0,32)
    || !x_D41A0_BYTEARRAY_4_struct.colorIndex_121[1])                        // not a flash frame
{ … normal draw … }
```
When that condition is FALSE (spell mid-cast AND in a flash frame) the whole body is skipped —
that is the **cast-in-progress highlight**: the box blinks by *omitting* its own redraw on
alternating frames (`colorIndex_121[1]` is the global flash toggle), leaving the previous frame /
background showing. (No dedicated "casting" sprite; the effect is the skipped-frame blink.)

### 2b. Frame bar (UI:356-373)
```c
if (playerEvent->word_0x2E_46)                                 // cast timer nonzero → glow frame
    bitmap = posistruct[SPELL_TOPTILE_BAR_GLOW];               // GBI:9  id 2
else
    bitmap = posistruct[SPELL_TOPTILE_BAR];                    // GBI:8  id 1
// altDrawFunction picks DrawBitmap_2BB40 vs ptrDrawBitmap_F01E8 (transparency-aware); flight passes false
```
So the box frame is **SPELL_TOPTILE_BAR (id 1)**, or **…_GLOW (id 2)** while the cast timer
`word_0x2E_46` is running.

### 2c. The BIG spell icon (UI:374) — the load-bearing line
```c
DrawBitmap_2BB40(posX, posY,
    (*filearray_2aa18c[filearrayindex_MSPRD00DATTAB].posistruct)[playerEvent->model_0x40_64 + SPELL_FIREBALL_BIG],
    scale);
```
**`icon_id = SPELL_FIREBALL_BIG + model = 123 + spell_model_index`.** `model_0x40_64` is the
spell's `spell_t`/model index (fireball=0 → 123). GBI:29 `SPELL_FIREBALL_BIG = 123`; the next
constant is `SPELL_TOP_LEFT_CORNER = 149` (GBI:30), so the BIG run is ids **123..148 = 26 icons**,
one per spell. This is a **different, larger icon set** than the CTRL grid's `SPELL_FIREBALL_SMALL`
run (97..122, GBI:28) — which is exactly the player's "bigger, MC1-like" observation.

### 2d. Level (tier) numeral (UI:375-379) — per-tier variation
```c
DrawText_2BC10((char*)SpellLevelText_DB06C[playerEvent->byte_0x46_70],
    posistruct[SPELL_TOPTILE_BAR].width_4 * scale + posX
        - (8*scale)*strlen(SpellLevelText_DB06C[playerEvent->byte_0x46_70]) - (2*scale),
    posY, (*xadataclrd0dat.colorPalette_var28)[0], scale);
```
The hand icon itself does **not** change art per tier — the tier is drawn as a right-aligned TEXT
label (Roman numeral / level string `SpellLevelText_DB06C[byte_0x46_70]`), `byte_0x46_70` = the
spell entity's active level (selector trace §0). So: one BIG bitmap per spell, plus an overlaid
level string; no per-level bitmap swap.

### 2e. Mana / availability meter (UI:380-403) — geometry
Only when `playerEvent->maxMana_0x8C_140` (the spell has a mana cost):
```c
// horizontal fine bar (fractional mana toward next cast), UI:382-387
DrawLine_2BC80(posX + 4*scale, posY + 36*scale,
    (56*scale) * (parent->mana_0x90_144 % spell->maxMana_0x8C_140) / spell->maxMana_0x8C_140,
    4*scale, color1);
// stacked 2x2 pips = whole casts affordable, UI:388-396
int manaScaled = parent->mana_0x90_144 / spell->maxMana_0x8C_140;
for (i=0; i<27 && manaScaled; i++)
  for (j=0; j<2 && manaScaled; j++) {
    DrawLine_2BC80(posX + 2*(i+2)*scale, posY + 2*(j+18)*scale, 2*scale, 2*scale, color0);
    manaScaled--; }
```
`color0`/`color1` are the owning wizard's two palette colours (UI:349-350). A up-to-27×2 grid of
2×2 pips = number of casts the player can currently afford, plus a thin fractional bar at
`posY+36`. Bar/pips are **drawn primitives (DrawLine_2BC80), not sprites**.

### 2f. "Can't sustain / regen" tint (UI:398-403)
If the spell has an upkeep (`manaRegen_0x88_136`) the castle can't cover, the whole box is
colour-washed via `DrawSquareByColor_2E850` over `SPELL_TOPTILE_BAR`'s width/height, colour 16
(non-Day maps) or 48 (Day map). This is a tint pass, not a sprite.

---

## 3. The asset file & the lowres/hires law

### 3a. Which bank (`filearrayindex_MSPRD00DATTAB` = index 4)
`int filearrayindex_MSPRD00DATTAB = 4;` (B:148). Default binding (B:232):
`{ &MSPRD00TAB_BEGIN_BUFFER, &MSPRD00TAB_END_BUFFER, &MSPRD00DAT_BEGIN_BUFFER, &posistruct5 }`.

Retail filenames (B:314-322):
```
DATA/MSPRD0-0.DAT / .TAB   (xadatamsprd00dat / …tab)   ← lowres bank
DATA/HSPRD0-0.DAT / .TAB   (xadatahsprd00dat / …tab)   ← hires bank
```

### 3b. Per-map-type path patch — `LoadSpr_47160` (L:1017-1044)
Both bank names are rewritten by `MapType` before load:
```
Day   → MSPRD0-0 / HSPRD0-0        (L:1021-1024)
Night → MSPRN0-0 / HSPRN0-0        (L:1028-1031)
Cave  → MSPRC0-0 / HSPRC0-0        (L:1035-1038)
```
(the 5th char D/N/C = day/night/cave; note the C++ var names keep the `…D00…` spelling regardless.)

### 3c. The lo/hi swap into index 4 — `LoadSpr_47160` (L:1046-1074)
```c
if (x_WORD_180660_VGA_type_resolution == 1) {                     // L:1046  — 320x200 lowres
    DataFileIO::LoadFileArray_84250(psxadatamsprd00dat);          // L:1048  load M-bank
    filearray_2aa18c[filearrayindex_MSPRD00DATTAB] =
        { &MSPRD00TAB_BEGIN_BUFFER,&MSPRD00TAB_END_BUFFER,&MSPRD00DAT_BEGIN_BUFFER,&posistruct5 }; // L:1049
    help_VGA_type_resolution = 1;                                  // L:1054
} else {                                                           // L:1056  — hires
    DataFileIO::LoadFileArray_84250(psxadatahsprd00dat);          // L:1058  load H-bank
    if (fixedMenuGraphics && !(…&1)) LoadFixedMenuGraphics();     // L:1060  (optional PNG override, remc2-only)
    filearray_2aa18c[filearrayindex_MSPRD00DATTAB] =
        { &HSPRD00TAB_BEGIN_BUFFER,&HSPRD00TAB_END_BUFFER,&HSPRD00DAT_BEGIN_BUFFER,&posistruct5 }; // L:1063
    help_VGA_type_resolution = 8;                                  // L:1069
}
CreateIndexes_6EB90(&filearray_2aa18c[filearrayindex_MSPRD00DATTAB]);   // L:1074
```
**So index 4 physically holds either the M-bank or the H-bank**; every HUD draw
(`…[filearrayindex_MSPRD00DATTAB].posistruct[…]`) transparently reads whichever was loaded.
The lowres path also applies at the `x_WORD_180660_VGA_type_resolution & 1` (odd = 320-wide)
tests seen throughout the draw code (e.g. EF:21799, UI:481, UI:514). **The lo/hi choice is by the
VGA resolution flag only** — `==1` ⇒ M-bank; otherwise ⇒ H-bank.

The same-indexed reads mean the BIG icon id `123 + model` and the frame id `1/2` are identical
between banks; only the underlying bitmap resolution differs. So porting only needs ONE id table;
pick the M vs H asset set by the active resolution.

> NOTE (remc2 extension, not retail): `fixedMenuGraphics` / `LoadFixedMenuGraphics` (L:940-1014)
> is an OPTIONAL PNG hot-patch layer that overwrites individual HSPR sprite indices from
> `…/<maptype>/HSPRD00_<index>.png` files. Retail has no such path; ignore for fidelity, but it
> confirms the sprite-index numbering (`HSPRD00_<n>.png` = posistruct index n).

---

## 4. Icon-run map in the MSPR/HSPR bank (index 4) — for the port

| run | base const (GBI) | id | span | used by |
|---|---|---|---|---|
| equipped-hand / top-bar frame | `SPELL_TOPTILE_BAR` | 1 | 1..2 (+GLOW) | UI:360-371 |
| **BIG spell icons (HANDS)** | `SPELL_FIREBALL_BIG` | **123** | 123..148 (26) | **UI:374** |
| SMALL spell icons (CTRL grid) | `SPELL_FIREBALL_SMALL` | 97 | 97..122 (26) | EF:22542/22544/22560 |
| CTRL pane frames/boxes | `SPELL_TILE_BAR`..`SPELL_ICON_PANEL2` | 87..91 | — | selector trace |
| flyout sub-icons | `SPELL_SUB_FIREBALL1_SMALL` | 179 | 179+ | selector trace |
| mini-map/castle/player HUD panels | `MINI_MAP_PANEL`..`HEALTH_PANEL_HIT` | 40..55 | — | UI:123 (DrawTopStatusBar) |

**Our port bug per the task**: we currently draw the CTRL grid tiles (97-run / pane frames) in the
hand panels. Retail draws the **123-run BIG icons** in a **SPELL_TOPTILE_BAR (id 1)** frame. Fix =
switch the hand-panel icon source to `123 + model` in the id-1/id-2 frame, level as a text numeral,
mana as DrawLine pips — all from the SAME index-4 bank (M or H by resolution).

---

## 5. Crosshair / flight-view cursor

**No flight crosshair/reticle sprite exists.** Searched `DrawGameFrame_2BE30` and the GameRender
DrawWorld path — nothing draws a center-screen aim sprite in flight (autoaim is invisible in
retail). The only pointer is the OS/software **mouse cursor** from `DATA/POINTERS.DAT` /
`DATA/POINTERS.TAB` (B:265-267), filearray index 0 (`filearrayindex_POINTERSDATTAB`, B:144),
shown via `SetCursor_8CD27(posistruct[CURSOR_SPRITE_INDEX_D419E])` (UI:660, EF:8594) — and only
while a menu/selector is open; in pure flight it is set to the null sprite `posistruct[0]`
(EF:42976, EF:8658 `//Set cursor to Null (Don't Draw)`). The POINTERS bank is a SEPARATE file from
the MSPR/HSPR HUD bank, so the crosshair question does not affect the hand-icon file choice.

---

## 6. OPEN / caveats

- **Exact bitmap dimensions** of the BIG (123-run) vs SMALL (97-run) icons are in the TAB
  `width_4`/`height_5` fields (read at runtime, e.g. UI:376 uses `SPELL_TOPTILE_BAR.width_4`); not
  a compile-time constant — confirm against the pristine GOG `MSPRD0-0.TAB` / `HSPRD0-0.TAB` when
  baking. OPEN until measured from the real TAB.
- The `word_0x2E_46` (cast timer) → GLOW-frame and the `colorIndex_121[1]` flash-skip together are
  the entire "casting" visual; verify our port's cast-highlight matches (glow frame while timer
  running + blink) rather than inventing a highlight sprite.
