# MC1/HW campaign / save / menu / launcher trace (2026-07-18)

Decompile recon for campaign stitching. Citations = `reference/remc1/sub_main.cpp`
unless prefixed; HW deltas cross-checked in `reference/remc1hw`. Companion:
`mc2-campaign-save-menu.md`; HW inventory: `docs/SURVEY-MC1HW.md`.

## Key structs (`engine/Basic.h`)

- `str_AE408_AE3F8` = persistent/config struct (`Basic.h:584-628`):
  `var_u16_17` @17 = **current campaign level**; `var_u8_29[32]` @29 +
  `var_u8_61[32]` @61 = the two campaign snapshot buffers the menu save
  persists (`var_u8_61` = 12-byte custom starting-spell loadout,
  `var_u8_29[0]` = use-custom flag; applied at level init `:48521-48531`,
  `:48570-48574`, default table `off_99B68`).
- `str_AE400_AE3F0` = the big world struct (full snapshot target).

## Campaign structure — strictly linear, no level select

- Advance: `TopProcedure_340B0` (`:41487`); after `GameLoop_34610`, exit-status
  `str_13323[..].var_u16_13325` bit 2 (won) → `var_u16_17++` (`:41595-41608`).
  Status `&6==4` = replay/movie arm (same level again).
- **Skip table {8,17,28,33,39}**: `sub_34070` (`:41456-41473`) — exact-match
  `++var_u16_17` again; called per level iteration (`:41543-41544`) gated
  `if(!IsHiddenWord)` — **HW has NO skips**.
- **Length: MC1 = 50 (indices 0–49, ~45 played), HW = 25 (0–24).**
  `sub_4E5B0_4E8F0` (`:59905`): `someVar = 50; if(IsHiddenWord) someVar = 25`
  (`:59939-59941`); `var_u16_17 == someVar` → frontend selector 10 = outro
  (`:60147`).
- Campaign/custom boundary = index 50: the PMULTI custom picker
  `sub_4D680_4D9C0` sets `var_u16_17 = word_12CBC2 + 50` (`:59530-59531`).
- New game: `var_u16_17 = 0` (`:60698-60702`); `byte_9687C` = loaded-save flag.
- `var_916` (`:54908`) = per-player live-world spell-flag array, NOT the
  campaign carry.

## Save systems (TWO, independent)

### A. Menu save `save/carpdd%02X.gam`, 6 slots — pure progression, ~130 B

Slot-name list `sub_51A10_51D50` (`:61982-62004`, `off_96864[6][21]`, "--" =
empty). Write `sub_51C90_51FD0` (`:62052-62081`), read `sub_51AF0_51E30`
(`:62007-62041`). Record, in write order: magic `4` (4 B) · slot name (20 B) ·
config `var_u8_29` (32 B) · config `var_u8_61` (32 B) · world+8597 settings
(12 B) · encoded level `4*(var_u16_17 + byte_12CBD0 + byte_12CBD1)` (4 B) ·
world+15318 (24 B) · `byte_12CBD0` name-rotation counter (1 B) ·
`byte_12CBD1` player-count (1 B) · world+8597 again (12 B).
Level decode on load: `var_u16_17 = stored/4 − byte_12CBD0 − byte_12CBD1`
(`:62036`). Frontend-menu only (between levels). **HW byte-identical**
(`hw:45629`).

### B. In-level quicksave `gam%05d.dat` + `map%05d.dat`, slot 199 — full snapshot

Raw RAM image, no serialisation. **Not portable and not readable by us** —
absolute 32-bit pointers throughout. Documented for behaviour, not for
interop; the port's own design is `docs/archive/DESIGN-SAVES.md`.

**Binding.** ALT+S / ALT+L, no menu path. `ProcessKeys_16B00` (`:19931`),
reached per frame via `sub_17C20` (`:20292`, called `:20336`) from
`DrawAndEventsInGame_34530_348F0` (`:41667-41668`). Alt test =
`pressedKeys_12EEF0_12EEE0[56]` (`:19976`, scancode 0x38 = LEFT Alt);
`case 0x1f` = S → save (`:19979`), `case 0x26` = L → load (`:19995`). Both
arms zero `lastPressedKey` (`:19993`, `:20014`), which makes the second
Alt+S arm at `:20379-20381` (`MakeControlCommand_188A0(10,0)`) **dead** —
control commands 10/11 (`:48600-48609`) fire only from a demo stream.

**Slot.** Format strings `"c:/CARPET.CD/%s/gam%05d.dat"` (`:49528`) and
`map%05d.dat` (`:49576`), but ALT+S/L pass the literal **199**
(`:19985-19986`, `:19998-19999`). ONE global slot — not per-level, not
per-campaign-slot, no rotation. Hard-coded absolute DOS path with no
fallback (unlike the level loader, `:49289-49302`). `"movie/gam%05d.dat"`
(`:49348`, `:49416`, `:49571`) = demo recording: same code, same layout,
different directory; snapshots at frame 0 (`:49651-49654`) and restores on
playback (`:49609-49612`). ALT+S/L never touch it.

**No sharing with §A.** Disjoint filenames, disjoint code; an in-level save
writes no progression bytes. Only incidental overlap: the 24-byte persistent
spell block §A stores lives *inside* the dump, so it rides along.

#### `gam00199.dat` — one `fwrite`, 232,713 B

`sub_3E750_3EA90` (`:49525-49530`) → `sub_62E60_63370` (`:74409`) = create/
truncate + single `fwrite` + close. Size = `sizeof(Type_str_AE400_AE3F0)`
(`Basic.h:535` comments `//size 232713`); HW's IDA output renders it as the
address symbol `byte_38D09` = 0x38D09 = 232713 (`hw:45668`). `#pragma pack(1)`
(`Basic.h:26`), 32-bit DOS4GW ⇒ 4-byte pointers.

| off | size | field | contents |
|---|---|---|---|
| 0 | 4 | `var_0` | mode/pause block — **discarded on load** |
| 4 | 4 | `rand_4` | **global RNG** (LCG `x = 9377x + 9439`, `:52223`) |
| 8 | 4 | | local player index (2) + player count (2) |
| 12 | 28 | `str_12`, `var_u32_32/36/38` | opaque |
| 40 | 4 | | free-list top |
| 44 | 532 | `var_u8_44[]` | **sprite/texture bank residency** (529 used, 0/1/2; `sub_593B0_598C0` `:66933-66948`) |
| 576 | 17 | | music/ambient id, palette/fade, stub |
| 593 | 4000 | | free-entity **raw pointer stack** — rebuilt on load |
| 4593 | 4004 | | recycle top + recycle **pointer stack** — rebuilt |
| 8597 | 36 | `set` | graphics settings — **discarded on load** |
| 8633 | 544 | | opaque |
| 9177 | 64 | | 8 × (`axis_3d` + u16) |
| 9241 | 2033 | | opaque |
| 11274 | 2049 | `str_11274` | scratch/template player record |
| **13323** | **16392** | `str_13323[8]` | **per-wizard records, 2049 B each** |
| 29715 | 80 | | per-player pending control-command slot |
| **29795** | **164000** | `str_29795[1000]` | **entity pool, 164 B each** |
| **193795** | **38812** | `str_193795` | decompressed `levels.dat` record for the current level (`:49319`, `:51535`) |
| 232607 | 106 | `str_232607` | thing count (4) + 96 B per-type census (`:43933-43954`) + **level number @232707** (2) + **tick counter @232709** (4) |
| | **232713** | | |

Wizard record `TypeStrAE400_13323` = 2049 B (`Basic.h:250-300`): `playIndex`
@+10 (index into the entity pool), 8×68 B message slots @+28, 33×14 B camera
keyframe ring @+572, 12 B name @+1046, 48 B chat @+1094, UI-mode byte @+1098,
then `Type_160 str_1103` (946 B, `Basic.h:159-243`) = the spell/mana column:
`var_14958_1635_532[24]` (i32 spell ids, −1 = empty), `var_676` (24×u16),
`var_15198_1875_772[24]`, `var_15222_1899_796[48]`,
`var_15318_1995_892[24]` (the block §A persists), selected-spell indices
@+940/+944 (255 = none).

Entity `Type_AE400_29795` = 164 B (`Basic.h:341-412`): `next` **pointer** @0,
per-entity `rand` @4, maxLife @8, actLife @12, flags @16, id @24, **class
byte @64** (0 = free slot), sClass/sModel @66/67, position `axis_3d` @72,
velocity @78, speeds @126/128, **model-table pointer @156**, **owner-record
pointer @160**. Castles, mounds, creatures, spell effects and players all
live in this one pool.

#### `map00199.dat` — six `fwrite`s, 398,018 B

`sub_3E8C0_3EC00` (`:49566-49579`), in order; loads mirror exactly
(`:49545-49563`).

| # | size | global | meaning |
|---|---|---|---|
| 1 | 65536 | `mapTerrainType_CC1E0` | 256×256 tile/texture id (`:14951`) |
| 2 | 65536 | `mapHeightmap_DC1E0` | height (`:14952`) |
| 3 | 65536 | `mapShading_EC1E0` | shading/lighting (`:14953`) |
| 4 | 65536 | `mapAngle_FC1E0` | tile angle (low 3 bits = corner class, bit 7 = locked) (`:14955`) |
| 5 | 131072 | `mapEntityIndex_10C1E0` | **`int16`** per-tile entity index (`:14957`) |
| 6 | 4802 | `byte_B5D40` | `char[2401][2]` auto-tile LUT, 7⁴ corner tuples → (type, angle) (`:14863`, used `:41233-41236`) |

Block 6 is **derived**, rebuilt at level gen by `sub_32560` (`:39947`, fill
`:40151-40178`) — saved anyway.

**Total per save: 232,713 + 398,018 = 630,731 B.**

#### Sequences

Save (`:19979-19994`): snapshot sprite banks `sub_593B0_598C0` (`:19982`) ·
stash level number into `+232707` (`:19983`) and tick counter
`dword_AC5D4` into `+232709` (`:19984`) — both live in the *other*, unsaved
global · write gam (`:19985`) · write map (`:19986`) · rebuild free/recycle
stacks (`:19987-19988`) · toast 100 ticks (`:19989-19991`).

Load (`:19995-20014`): `sub_3E690_3E9D0(199)` (`:49459`) = existence probe
(`:49462`) → stash `var_0` + `set` (`:49466-49469`) → read the whole file
**straight over the live struct** (`:49492`; `RncUnpack_62B60_63070` no-ops
unless the first 4 bytes are `RNC\x01`, `:73665-73667`) → restore `var_0` +
`set` (`:49493-49494`) → pointer fixups `sub_416B0` (`:49508`) → rebuild
stacks (`:49515-49516`) · read map (`:19999`) · restore level number
(`:20000`) and tick (`:20001`) · re-sync sprite banks `sub_59420_59930`
(`:20002`, `:66959`) · reset input idle baseline `FlvInitSet_356E0_35AA0`
(`:20003`) · toast (`:20004-20006`).

Snapshot point is a clean **inter-tick boundary**: input runs at `:41668`,
the sim tick at `:41677-41688`.

#### Fixups and rebuilds

`sub_416B0` (`:52152-52188`; HW `sub_419F0` `hw:48208-48240`) is a genuine
pointer-rebasing pass, two loops:
1. Owner relink, per player: `str_29795[str_13323[i].playIndex].+160 =
   &str_13323[i].str_1103` (`:52173`, `hw:48222`).
2. Model-table rebase, every entity with class byte ≠ 0: delta between the
   *expected* and *saved* `+156` of the local player's carpet, applied to
   all. HW shows it unobscured (`hw:48232-48237`); anchor `unk_99018 ==
   &unk_98F38[7]` (`Type_156` = 32 B, `Basic.h:314-330`), the canonical `+156`
   for a player carpet (`:44190`). Delta is 0 on a same-build reload; the
   pass exists to survive a moved table.

`sub_37220_375E0` (`:43825-43859`) walks entities 999→1 and **regenerates
both 4000-byte pointer stacks**, so the 8 KB of pointers in the file is dead
weight. The caller then sets the recycle top to −1 (`:49516`, `:19988`),
discarding what it just built — the same idiom level init uses
(`:51566-51571`). Entity `next` pointers are stale but every list is rebuilt
each tick at the top of `sub_41780_41AC0` (`:52216-52260`).
`mapEntityIndex` holds **indices**, not pointers — no fixup needed.

Not saved at all: the whole second master struct `str_AE408_AE3F8` (only the
level number is smuggled through `+232707`), the terrain-modification RNG
`pseudoRand_12C1E0` (`:41238`), sprite/texture bank *contents*, palettes,
sound/MIDI, mouse. Saved-but-discarded: `var_0` and the 36-byte `set` — so
reflections/shadows/sky/blur survive a load.

#### No validation — and a retail bug

**No magic, no version, no length field, no checksum in either file, and
nothing is checked.** The only "validation" is a bare open+close probe
(`:49535`, `:49586`). `sub_3EEA0_3F1E0` reads `FileLengthBytes` bytes with no
cap (`:49738-49740`); its return is ignored at `:49492`, and *that* return is
ignored at the call site `:19998`. Map reads are unchecked too — a truncated
file silently leaves the previous map's tail in memory. Contrast §A, which
does validate its magic (`:62021-62023`).

⇒ **ALT+L with no save present still prints "Game loaded."** and executes the
restore steps anyway, copying the level-start-zeroed `str_232607` (`:51487`)
over the live level number and tick counter — **silently setting the current
level to 0**. Identical in HW (`hw:18196-18199`).

#### Gating

Blocked: network game (`var_u8_0 & 0x10`, `:19980`, `:19996`) — but the key
is still swallowed, so it is a **silent** no-op with no message; demo
playback (`var_u8_0 & 4` skips all key handling, `:41667`); a control command
already queued this tick (`:19974`, `:20329`); UI mode 3 = text entry (modes
0/4 `:20326-20336`, 1 `:20635`, 2 `:20645`, 3 routes to text `:20743`);
AI-driven local player (`:20316-20321`).

Allowed: **while paused** — `sub_17C20` is deliberately NOT gated on the
pause bit at `:41667`, while `:41666` and `:41670` are. (This is the retail
basis for pause-and-rearrange; see `docs/archive/DESIGN-SAVES.md`.) Also allowed
**while dead** — no life check anywhere in the path.

No confirmation, no slot picker, no overwrite prompt. Feedback is a 100-tick
toast with hard-coded English literals, **not** from the localisable
`dword_AE238_AE228[]` table the F1–F9 messages use (`:20090`, `:20135`) — so
both strings are untranslated in retail.

#### HW delta and port artefacts

**No functional differences.** Verified across binding, gate, slot, paths,
struct size, map block sizes, preserved fields, fixup algorithm, level/tick
stash offsets, stack rebuild, bank snapshot/restore, input reset and toast
text. Port artefacts (not retail): HW's `sub_3E750_3EA90` body is commented
out `//fix!!!` (`hw:45668`) so the HW port cannot save; remc1's `movie/`
variant passes `sizeof(pointer)` instead of the struct size (`:49393`), the
`save/` variant at `:49529` is correct; `Basic.h:290` comments
`Type_AE400_9177` as "len 48" when it is 8 (only 8 closes the offsets) —
trust the offset-encoded names, not the comments.

Unresolved (would need retail runs): whether right Alt (E0 38) also lands on
index 56; behaviour when only one of the two files exists; whether a
cross-level ALT+L (which restores the embedded 38,812 B level record and
resets `var_u16_17`) really teleports you back into the saved level.

Save dir created at first run `CreateGameDir_3EC90('C',"\carpet.cd","save")`
(`:51471`); GOG DOSBox overlays to `cloud_saves/`.

## Launcher — two EXEs + chooser

ISO `CARPET.CD`: `CARPET/CARPET.EXE` (MC1), `CARPET/HIDDEN.EXE` (HW),
`CARPET/SELECT.EXE` (graphical chooser, art `data/sel0-0.dat`/`selp0-0.dat`
MC1 vs `sel1-0.dat`/`selp1-0.dat` HW 640×480), root stub `CARPET.EXE` runs
SELECT, reads choice from `C:\CARPET.CD\CP.DAT` (retail = `02 00`), then
`DOS4GW CARPET|HIDDEN`. Compile-time `IsHiddenWord` == which EXE.

## Frontend state machine `sub_4AB20_4AE60` (`:57863-57925`)

Selector `mainMenuSelector_12CBCE_12CBBE`: 9=logo intro (`intro\logo.dat`),
0=intro2, 8=title art (MC1 `title-01/02`, HW `title-03/04`, `:60284-60308`),
6=language select (persists `language.inf`, `:60347-60357`; boot selector,
`:57840`), 1=sound/joystick, 2=main menu, 4=PMULTI custom picker, 5=play,
7=perf config, 10=outro. `intro.pld` marker = intro seen → skip
(`:57843-57858`).

- Main menu (`:58943-59000`): `screens/mainmenu.dat`+`.pal` (320×200; all
  SCREENS bgs are 64000 B mode-13h + 768 B .PAL; hi-res VESA path exists via
  `typeResolution_12F02E & 1`), MMSPR sprites, GLOBE.DAT + TIMER.DAT anims,
  MMMASK.DAT hotspots + button table `dword_4A12C_4A46C[]`, 6 save-slot names.
- Intros: `PlayInfoFmv_107C0`, custom Bullfrog FMV (`INTRO.DAT` = 79 MB);
  chain logo.dat → intel.dat → intro.dat; scroll.dat on load-slot preview.

## Level flow

boot (language→logo→title) → menu → New Game (level 0) or Load → play:
skip-check (MC1 only) → `LoadLevel` (RNC `LEVELS/LEVELS.DAT`+`.TAB` MC1 /
`DDLEVELS.*` HW, fixed 38812-B struct) → `GameLoop_34610` → completion FMV:
win alternates `intro\levelw1.dat`/`levelw2.dat` by clock parity
(`:59969-59980`), loss `intro\levelose.dat` (`:59982-59990`) → `var_u16_17++`
→ repeat → outro `intro\outro.dat` (`:59364`) at 50/25, which writes no save
(durable record = whatever the player saves via menu).

Caveat: HW's `if(IsHiddenWord) int someVar = 25;` is a decompiler shadowed
local; real binary assigns (value genuinely 25). Same at the 20→10 map-grid
rows.

## Main menu + save/load UI (recon 2, 2026-07-18)

CORRECTIONS to the survey above: selector 7 = the `intro/intel.dat`
logo stage (sub_4EFC0 → sel 9), NOT perf config — no perf-config menu
exists (pperf.dat is the post-level SCORE screen inside selector 5).
Selector 1 = the SNDSETUP sound-CARD wizard (7 pages, writes
sndsetup.inf/.dat; skipped at boot when sndsetup.dat exists); no
joystick options there.

- **Hotspots**: MMMASK.DAT = full-frame 8bpp bitmap, pixel value ==
  button id under it; mouse probed at (x>>1, y>>1) in 320×200 space
  (:58539). TAB/Shift+TAB cycle ids 1-11 skipping invalid; Enter/click
  fires. Validity (:58861): Multiplayer needs network; ids 7-10 only
  in a submode; Continue (11) only when a game is underway
  (`!byte_9687C`; init 1, cleared by New-Game click and save-load).
- **Buttons** (dword_4A12C_4A46C): 1 = New Game/Play (byte_9687C set →
  confirm + var_u16_17=0, else resume current), 2 = name/loadout entry
  over scroll.dat (two fields → var_u8_29 (30 ch) + var_u8_61 (8 ch),
  prompts dword_AE238[34]/[35]), 3 = Multiplayer → sel 4, 4 = Quit
  (confirm), 5 = LOAD submode, 6 = SAVE submode; in a submode ids 5-10
  = the six slots (var 1-6); 11 = Continue (entry past the
  reconstructed array — resumes var_u16_17; confirm in playtest).
- **Highlight law**: NOT an overlay sprite — per-pixel palette
  BRIGHTEN of the hovered mask region: sub_51E84 remaps pixels whose
  mask==id through a 256-LUT built at menu open = palette × 1.30
  clamped 63 (:62134, :60837).
- **MMSPR draw sites**: 1 = LOAD label (358,10) / load-box (24,8);
  2 = SAVE label (336,86) / name-box (24,8); 5 = OK (136,210);
  6 = Cancel (480,210); 7 = quit-confirm art (centered 65,75,189,44).
  Labels are pre-rendered art (localized), not font-drawn.
- **GLOBE.DAT / TIMER.DAT**: mini FMV streams stepped by sub_1002D on
  odd frames while sel==2; globe frames 1..30 wrap; timer 1..3 wrap,
  animates ONLY when a game is underway. Frame blit x,y live in the
  frame headers (decode needed).
- **Slot UI**: 6 slots, names off_96864[6][21], "--" empty; slot text
  clip rects (320-space): (170,0,150,36) (161,37,159,35) (191,73,
  129,32) (172,104,148,27) (186,133,134,31) (173,165,147,28). SAVE:
  scroll.dat parchment FMV, OK x68-81 y106-116, Cancel x240-250
  y105-115, name field x110-230 y85-95 → editor max 20 chars (fresh
  slot auto-opens editor). LOAD: mmspr[1] + slot name confirm in
  viewport (65,75,189,44).
- **Between-level flow**: win → levelw1/levelw2 FMV (clock parity) or
  levelose → PPERF score screen (pperf.dat, rows reveal on a
  timeline, auto-dismiss 1320 ticks) → MAIN MENU (sel 2; sel 10 outro
  at 50/25). The menu IS the retail transition beat; Save there
  persists the already-incremented var_u16_17; Continue resumes.
- **Menu sound**: music/ambient cue ids 0xD + 4 at menu open
  (:58945-92); no hover click; PPERF row-tick sample via
  sub_65F10(0,3).
- **HW**: same code/tables/asset names; art differs per game dir;
  title-03/04; no skip table; length 25.

OPEN: MMMASK region geometry (decode the file), Continue's concrete
table entry, mmspr full census, GLOBE/TIMER frame-header decode,
menu music id 0xD → track mapping.

## MMMASK layout + MMSPR identification (recon 3, 2026-07-18 —
## decoded from the retail files, ASCII-verified)

Hotspot regions (id: bbox, 320×200):
1 = (0,12)-(62,118)   the GLOBE arch      → New Game/Play
2 = (20,129)-(93,197) bottom-left         → name/loadout entry
3 = (103,80)-(164,156) center             → Multiplayer
4 = (105,159)-(158,193) bottom-center C:\ → Quit
5-10 = the SIX right-side BOOKS, top to bottom: (171,1)-(319,35),
  (161,37)-(319,70), (192,72)-(319,104), (172,102)-(319,132),
  (186,134)-(319,162), (172,165)-(319,194) — top book = LOAD,
  second = SAVE; in a submode all six are the slots (= SLOT_RECTS).
11 = (96,24)-(144,74) the HOURGLASS       → Continue

THE HOURGLASS IS NOT IN MAINMENU.DAT — bare wall art there. The
TIMER.DAT keyframe carries the whole hourglass on its stand; retail
shows it (and its Continue click point) only while a game is
underway. Bake law: timer visible set = sand inter-frame deltas ∪
keyframe-vs-bg VISUAL diff inside the id-11 region.

MMSPR sprites (ASCII-identified; draw-site labels in recon 2 were
partly wrong): 1/2 = "C:\ + arrow" exit icons (30/28×22 — NOT
load/save labels), 3 = 41×33 (unidentified, unused), 4 = 1×1 pad,
5 = checkmark 19×11 (OK), 6 = X 13×12 (Cancel), 7 = C:\-on-a-scroll
quit icon 32×20 (centered in the quit-confirm viewport
(65,75,189,44)). SCROLL.DAT plays over a CLEARED screen — the
save/load dialog is scroll-on-black, not an overlay.

## Recon 4 — the menu MUSIC + SFX-bank calls (2026-07-18, opus agent)

The banked "menu music cue 0xD" from `sub_5D070_5D580(0xDu)` at
:58945 was a RED HERRING — that function is the SFX SAMPLE-BANK
loader, not music (body :69682: `sprintf("data/snds%d-%d.dat", a1,
byte_939EC)` → `snds13-<tier>.dat/.tab`, feeding the 32-voice mixer;
`byte_939EC` = a free-RAM quality tier chosen at :51955-52020 — 1 =
high, 0 = mid, 3 = low). Function names lie; read the body.

The menu-open sequence :58945-92 in full:
1. `sub_5D070_5D580(0xD)` — load the MENU SFX bank (snds13: button
   clicks etc.). NOT YET PORTED — our menu clicks are silent; a bake
   member + bank selector is the hook.
2. `sub_5CEF0_5D400(0)` — load MUSIC BANK 0 (body :69619:
   `sprintf("data/music%d-%d.dat", a1, byte_CBFEE)`; fills the song
   table `dword_CBF60`). `byte_CBFEE` = the sound-DRIVER variant
   (:54026-98): 0 = AdLib/OPL FM (`inst.bnk`/`drum.bnk`), 2 =
   wavetable/General-MIDI — the two arrangements our bake carries.
3. `sub_5D290_5D7A0(4)` — PLAY song id 4 of the loaded bank =
   **CSETUP.HMP** (bank 0 = ids 1-4 = cgame1/2/3 + csetup). This is
   the dedicated menu MIDI — the same SETUP law as MC2's
   StartMusic(4).

Menu→level: :59992-94 stops sfx (`sub_5D010_5D520`) + music
(`sub_20E60_20E60`) before the launch; level entry (:41550-56)
reloads snds0 + music0 and the in-level loop starts a random song 1-3
(:41578-82 `pseudoRand%3 + 1` = the cgame cycle we already ship).
Multiplayer screen (:60004-06) plays song 1 (cgame1); returning to it
replays 4 (:59803).

Ported 2026-07-18 (frontend/gameplay separation): `frontend_music()`
plays `csetup` (already baked, FM + GM) on every MC1/HW frontend
entry; level music starts with the session install; the teardown
stops sfx + speech like the retail transition. The snds13 menu-SFX
bank remains the open hook.
