# MC2 campaign / save / menu / map-screen trace (2026-07-18)

Decompile recon for campaign stitching. Citations relative to `reference/remc2/`.
Companion: `mc1-campaign-save-menu.md`. Upstream objective-completion half:
`mc2-stage-engine-completion.md` (IsLevelEnd_0).

## Campaign structure

**The two portal tables ARE the campaign definition** (no table in game data —
it lives in code):

- Main portals: `engine/Type_MapScreenPortals_E17CC.cpp:3` — `mapScreenPortals_E17CC[26]`,
  22 B/entry, 25 real + terminator. Fields: `time_0`, `viewPortPosX_4/Y_6`
  (map-scroll anchor), `word_8/10` (hit-box 0x28×0x28), `portalPosX_12/Y_14`,
  `spriteIndex_16`, `activated_18` (**2=hidden, 1=activated**), `byte_19`, `word_20`.
  **Portal index == level number** (`MenusAndIntros.cpp:1542, 3346`;
  `EventsFunctions.cpp:31383`). Index 24 = finale: click sets
  `setting_38545 |= 0x20` (`MenusAndIntros.cpp:3352-3353`), completion with that
  bit → ending `sub_6E0D0()` (`EventsFunctions.cpp:31505-31506`).
- Secret portals: `engine/Type_SecretMapScreenPortals_E2970.cpp:3` — `[6]`,
  17 B/entry, 5 real. Fields: `time_0`, `index_4` (parent main level),
  `levelNumber_6`, `posX_8/Y_10`, `activated_12` (**3=hidden, 2=revealed,
  1=completed**), `spriteIndex_14`, `byte_16`. Init values:

  | secret level | parent main (`index_4`) | map pos |
  |---|---|---|
  | 30 | 4  | 287,656 |
  | 31 | 7  | 879,614 |
  | 32 | 11 | 854,400 |
  | 33 | 17 | 395,114 |
  | 34 | 19 | 365,504 |

- **Campaign set = mains 0–24 + secrets 30–34 = 30 levels.** Confirmed by
  `CLEVELS/LEVELS.TAB` (1000-entry uint32 offset table, level number indexes it
  directly): slots 25–29 and 35+ are repeated-offset zero-length gaps. Loader
  `EventsFunctions.cpp:38229-38242`. `isSecretLevel` = `levelnumber > 24 && < 50`
  (`EventsFunctions.cpp:31407`; portal gate `MenusAndIntros.cpp:2224`).

## Progression + exit routing

- Complete-bit: `sub_57570` (`EventsFunctions.cpp:39887-39888`):
  `if (IsLevelEnd_0 && !(byte[2] & 0x10)) byte[2] |= 2` on
  `dw_w_b_0_2BDE_11230`.
- `PortalsUpdate_7DD70` (`MenusAndIntros.cpp:2208`): complete-bit + levelnumber
  match promotes main portal `activated_18 = 2→1` (sprite 37), reveals next
  hidden portal. Secret arm (`2226-2245`): match `levelNumber_6`, set
  `activated_12 = 1` (sprite 305), force-activate parent portal.
- **Exit variants** (exit state machine `EventsFunctions.cpp:60534-60544`, case 0xC):
  both set `byte[2] = 0x0A` (advance|reload). Secret exit (action 11, demon
  mouth (14,4)): also clears `setting_38545` bit 0x04 and **always sets
  `byte[2] |= 0x10`**. Normal exit (action 12, checkpoint X (14,3)): sets 0x10
  only if `setting_38545 & 0x10` (level has a revealed-uncompleted secret
  portal; bit set at click time `MenusAndIntros.cpp:3350-3351`).
- Loop consumption (`EventsFunctions.cpp:31510-31548`): `byte[2] & 0x10` +
  advance + `actLevel < 24` → `GetSecretAndActivedPortal_824B0(actLevel)`
  (`:46992`, matches `index_4`), set `levelnumber_43w = levelNumber_6`,
  **VGA_Resize(320,200)** (secret levels run 320×200; mains/menus 640×480),
  `LevelInitGame_56A30` — straight into the secret level, no map return.
  Without 0x10 → break to map screen, linear advance.
- **DERIVED completion law (player-confirmed semantics, 2026-07-18): the
  mouth exit does NOT complete the parent.** Portal promotion happens only
  in `PortalsUpdate` ON A MAP VISIT — the mouth path jumps straight into
  the secret, so the parent's complete-bit never reaches the map;
  completing the SECRET promotes both (the secret arm force-activates the
  parent, MI:2226-45). A failed/abandoned secret therefore leaves the
  parent PENDING with its secret portal revealed — which is exactly what
  the two suppression/routing arms serve: `PresentLevelDescription`
  suppresses the parent's narrative (`:3583-89`, already heard), and the
  checkpoint-X-with-revealed-secret arm (`setting_38545 & 0x10`) routes a
  parent replay back INTO the pending secret. After the secret completes,
  the map shows parent-completed + next revealed + next narrative — "as if
  the hidden level didn't happen" (player).

## Cutscene table

`cutScene_E16E0[7]` (`MenusAndIntros.cpp:189`): plays after completing level
index N when `levelnumber_43w + 1 == levelNumber_4`, once (`overplayed_5`
latch, `:4064-4076`; secrets map through parent `index_4 + 1`, `:4082-4097`):
CUT1 after idx 4, CUT2 after 8, CUT3 after 12, CUT4 after 16, CUT5 after 23,
CUT6 after 24 (finale). Files `INTRO/CUT%d.DAT`.

## Save systems (TWO, independent)

### A. Campaign save `SAVE/SAVE%d.GAM`, slots 1–8 — pure progression, 1319 B

Path `portability/port_filesystem.cpp:549`. Write `MenusAndIntros.cpp:2529-2618`
(order at `2606-2616`), read `1445-1518`. Map-screen only.

| off | size | field |
|---|---|---|
| 0 | 4 | signature `0xFFFFFFF7` |
| 4 | 20 | slot label (`xx_BYTE_17DF14[slot]`) |
| 24 | 32 | `player_name_57ar` |
| 56 | 32 | `savestring_89` |
| 88 | 102 | `secretMapScreenPortals_E2970` (6×17 B; load takes `activated_12` at +12, `:1505-1506`) |
| 190 | 16 | `m_GameSettings` |
| 206 | 4 | `numLevelsCompleted` (`:2599-2604`) |
| 210 | 4 | current-level `byte[2]` complete-flags |
| 214 | 505 | `str_611` — player spell/XP/mana block (`type_str_611`) |
| 719 | 500 | per-MAIN-level stats 25×5 int (`x_DWORD_17DBC8x`) |
| 1219 | 100 | per-SECRET-level stats 5×5 int (`x_DWORD_17DDBCx`; spells%/accuracy%/kills%/mana%/time, `sub_82AB0` `EventsFunctions.cpp:47027-47033`) |

On load, main-portal states are **reconstructed from `numLevelsCompleted`**
(first N activated) and `levelnumber_43w` = last activated
(`MenusAndIntros.cpp:1521-1544`). No `.GAM` ships with GOG (player-created).

### B. Mid-level snapshot `SLEV%d/SMAP%d/SVER%d.DAT` — full world dump

`Level.cpp:172-405`. SMAP = 5×64 KB terrain planes + 128 KB entityIndex +
4802 B buildings = **463,554 B** uncompressed; SLEV = whole
`type_shadow_D41A0_BYTESTR_0` entity/game blob, compressed, pointer fix-ups on
load (`sub_57680_FixPointersAfterLoad`); SVER = 8 B {version=15, level}.
GOG install ships slot 2 (`GAME/NETHERW/SAVE/SMAP2.DAT` = exact 463,554 B).
NOT a documentable subset — retail RAM layout.

## Menu / intro / map screen

- State machine `MenusAndIntros_76930` (`:578`), `nextMenu_E29D8`:
  SetToIntro → Intros → MainMenu (`:616-624`).
- Intro `Intros_76D10` (`:736`): HSCREEN0.DAT world-map bg (`:742-744`),
  Bullfrog frog `ShowWelcomeScreen_83850` (`:772`), `INTRO/INTRO.DAT` +
  `INTRO2.DAT` via `PlayInfoFmv` (`:774-797`). **FMV = proprietary Bullfrog
  .DAT**: 12-B header (frameCount/height/width) + keyframe/delta frames,
  decoder `ReadFrame_75DB0`/`DrawFrame_75E70` (`Animation.cpp:41-77`).
- Main menu `MainMenu_76FA0` (`:816`): 640×480 HSCREEN0 bg, logo sprite 66 at
  (185,232), music track 4, attract replay after 60 s idle (`:860-866`).
  New Game `sub_7E640` (`:1367`) resets portals to hidden.
## Map-screen ASSETS + in-level exit markers (second recon pass)

- `DATA/SCREENS/HSCREEN0.DAT` (1,557,528 B, inside game.gog ISO) = ONE flat
  blob addressed by hard offsets; every chunk via
  `sub_7AA70_load_and_decompres_dat_file(path, dest, position, length)`
  (EF:46290) → seek/read/`RNC\x01`-or-raw. World-map screen = `case 6` of
  `sub_7A110_load_hscreen` (EF:46108-180):
  - bg @0xB2C47 len 0x87D83 RNC → 1,228,800 = **1280×960 8bpp**
  - palette @0x13A9CA, 768 raw = 256×RGB **6-bit** (<<2)
  - sprite pool @0x783BD RNC → 301,787 B 8bpp pixels
  - sprite index @0x91856 RNC → 1878 B = **313 × 6-byte records**
  - `case 4` = main menu bg, same file. No other SCREENS files exist.
- 6-byte TAB record (`bitmap_pos_struct2_t`, bitmap_pos_struct.h:27):
  `{u32 le offset-into-pool, u8 w, u8 h}`; pixels raw 8bpp row-major,
  index 0 transparent. A THIRD tab variant (importer has 4-byte dattab +
  10-byte tmaps; this one needs a new parser).
- Blit law: 1280-wide source scrolled into the 640×480 viewport
  (`DrawNetGameMapBackground_85C8B` MI:4699); sprites drawn at
  `(portalPos − scroll)` by `DrawFrameAnim_7E5A0` (EF:46471).
- **In-level map exit markers**: drawer `DrawMinimapEntites_61880`
  (EF:62388) → GameUI.cpp:1591/951; class-11 arm: model 12 → sprite **83**
  (red X, normal exit), model 31 → sprite **84** (O, secret exit), from the
  in-game HUD bank `DATA/MSPRD0-0.DAT/.TAB` (MSPRN night / MSPRC cave;
  TAB = 262 × the same 6-byte records). Drawn CENTERED, colour baked into
  the sprite, visibility = map-circle clip ONLY (no discovery gate). The O
  overdraws the co-located X. Exit-list corroboration EF:40077,
  GameUI.cpp:3071/3086. OPEN: the editor THING (14,3)/(14,4) → runtime
  (11,12)/(11,31) remap was not traced (spawn-table side).

- Map screen: animated every frame (`DrawAnimTextsAndPlaySounds_7D400` `:2774`).
  **Portal draw law (round-2 re-read `:2790-2827`)**: activated==1 (completed)
  = flag anim 37→43; the FIRST activated==2 portal = the NEXT level — plays
  open sound 41 (`:2820`), pop-in anim 70→83 ONCE (per-session `byte_19`
  latch), then idles as the OPEN portal 33→35; the draw loop `break`s after
  it, so later portals never draw. Secrets (`:2828-2879`): completed 305→311;
  revealed = same pop-in latch (`byte_16`) then 270→272. Frame step: every
  ≥8 ticks of the 100 Hz clock = 12.5 fps (`DrawFrameAnim_7E5A0` EF:46471).
- **Map cursor = bank sprite 239** (`:986`, set on map-screen entry). The
  "cursor 39" at `:845`/`:1845` belongs to the MAIN-MENU sprite chunk
  (`case 4`) — the case-4/case-6 chunks load into the SAME runtime array
  `xy_DWORD_17DED4_spritestr`, so indices only mean anything per-chunk
  (in the map chunk, 37-43 are the flag frames).
- **Travel carpet**: `SetAnimationVariables_7DA70` (`:2277`) picks one of 8
  heading families ×4 frames = sprites 1-32 from the 0..2048 angle
  (cardinals: 0/2048→17, 512→9, 1024→1, 1536→25; sectors between → 5/13/
  21/29); `sub_80D40` (`:3604`) moves 3×2 px Bresenham steps per frame
  (`CreateAnimObject_7E8D0`/`MoveAnimObject_7E9D0` EF:46528-46637, count
  2,2), carpet frames step every ≥16 clock ticks = 6.25 fps
  (`MoveAnimIndex_81260` EF:46640), start sound 19 (`:3788`). Trail: dot
  **sprite 139** stamped INTO the 1280×960 map background buffer whenever
  the carpet moved >8 px (`DrawMapObject_812D0` EF:46655, gate
  `x_WORD_17DB8A == -1` `:3728` — gate semantics untraced). Entry after a
  completion sets mode 4 (`:961-964`), flying last-completed → newly
  revealed; the pop-in waits for arrival (the `a4==3||5` gate). At the
  finale, `MapMenuPortalsDraw_81760` (`:3795`) stamps the whole route
  statically into the bg.
- **Ambient set dressing**: `x_BYTE_E26C8_str[16]` (`:199-216`, struct
  EventsFunctions.h:253-270 `{time1,time2,x,y,first,last,frameIndex,
  delay_s,state,burst,t4,sample,mode,pan}`), drawn by `DrawAnimSprite_81CA0`
  (EF:46934): state 2 = WAIT (invisible) until `(now-t0)/100 > delay_s`,
  then anim frames first..last-1 at 12.5 fps; burst rows (byte_21==1)
  return to WAIT after one cycle, loop rows repeat forever. The
  firstFrame-85/86 cluster (4 spots: static 85 + burst 86-92 overlays) is
  suppressed once portal 24 activates (`:2786`).
  Hit-test `InRegion_7B200` (`:3335-3344`); left-click
  = launch + travel animation (`sub_80D40`, carpet walks portal→portal) +
  objective text `PresentLevelDescription_80C30` (`:3304-3319`);
  **right-click on a completed (activated_18==1) portal replays it**
  (`:3385-3405`, launch state 5).

## Map border overlay + pointer law (recon 3, 2026-07-18)

Driver = `NewGameDialog_77350` (MI:939, name lies — it IS the map screen);
per-frame `NewGameDraw_7EAE0` (MI:2996). Esc/Exit-button → main menu
(`endAction=2`); portal-arrival → launch (`endAction=1`).

- **Ornate frame**: `sub_85CC3_draw_round_frame` (EF:47713) RLE-blits the
  640×480 border art every frame. Chunk @0x141E85 len 13195 (EF:46150) →
  24523 B stream: i16 tokens for the TOP-LEFT QUADRANT (240 rows;
  >0 = literal run mirrored into all 4 quadrants, <0 = transparent skip,
  0 = row end; rows 0-18 (`a2>221`) duplicate one trailing byte). Art
  only — no hit-testing.
- **Corner buttons `mapMenuButtons_E23E0[4]`** (MI:321-326), ALWAYS
  visible/clickable (no edge-reveal). grey=idle byte_21, gold=hover
  byte_20; hit-box = the sprite's own dims (MI:2408-9); click = sample
  14 + open dialog (MI:2410-2414):
  | fn | corner | pos | grey/gold |
  |---|---|---|---|
  | Exit → menu (`sub_7E620`=return 2) | bottom-right | 581,427 | 246/247 |
  | New Game (`sub_7E640` confirm+reset, stays on map) | bottom-left | 0,427 | 248/249 |
  | Save (`SaveGameDialog_78730`) | top-left | 0,0 | 250/251 |
  | Load (`LoadGameDialog_780F0`) | top-right | 581,0 | 252/253 |
- Dialogs = parchment scroll (`DrawScrollDialog_7BF20` MI:5402: opens in
  16-px steps to str_26 height; 1=OK 2=Cancel; frame sprites
  x_WORD_17DF06..0E = case-6 254-258). Save anchor (29,60) h=200 title
  422; Load (510,60) h=200 title 421; NewGame confirm (37,348) h=60
  title 467. 8 slots, row pitch 16 px at (x1+20, y1+16+16k), hit 90×16,
  "Empty" = lang[414], selected = pal(3F,3F,3F) dim = (16,10,09); Save
  slots have label edit (sub_7F6A0: filter sub_7C200 space/0-9/A-Z/a-z,
  max 15, "_" caret; Esc(1)/Enter(28)/Backspace(14)).
- F1 tooltips: textBoxStr_E2516 rows, lang 466/405/406/467.
- **Description text** `PresentLevelDescription_80C30` (MI:3569-3601):
  text = langindexbuffer[23 + level]; draw x=130 w=380, y=280 if the
  target portal y<478 else 60 (MI:3313-14); font 1 (= DATA/FONT1),
  white (3F,3F,3F), bordered word-wrap `sub_7FCB0`; state 1→2 after
  1.5 s (0xF ticks of 100 Hz); keypad 111/79 toggle; secret-revealed
  suppression (MI:3582-89). Narration rides the same call.
- **Pointer law** (MI:3132-3175 + port_sdl_vga_mouse.cpp:206-226):
  cursor CONFINED to the 640×480 screen; scroll triggers on the EXACT
  edge pixel (x==0 / x>=638=MOUSE_MAX_X / y==0 / y>=478=MOUSE_MAX_Y);
  X and Y independent (corner = diagonal); accel `shift_step += 4`
  per moving frame capped 24 px/frame, reset to 0 on release
  (MI:3167-73); scroll clamp 0..638 / 0..478 (MI:3163-4). No cursor
  sprite change at edges. Edge-scroll suspended mid carpet-flight.

## Main menu (recon 3, 2026-07-18) — HSCREEN case 4

State machine MenusAndIntros_76930 → MainMenu_76FA0 (MI:816). Case-4
chunks are SEQUENTIAL from 0 (loader advances `17DEDC = pos+len`):
palette @0 (768 raw), bg @768 len 168081 (RNC → 640×480), sprite pool
@168849 len 102213 (RNC → 111 sprites), tab @271062 len 411, font
"4b" pool @0x13ACCA len 1226 + tab @0x13B194 len 548 (272 glyphs 7×7).
Case-6 also loads two font banks: A @0x1641FC/0x1646BA (273 × 8×14,
index array shifted +1 at MI-load) and B @0x164907/0x164DAE (271 ×
7×7), + TcolNext @0x13B3B8 (16384) + blob @0x13CE20 (20581 raw).

- Music track 4 vol 127; cursor = case-4 sprite 39; version text at
  (10,465) red; attract = intros 1/2 alternating after 60 s idle.
- Idle anims: fires (17,159) spr 1-8 / (531,156) 9-16; incense
  (154,308) 17-25 / (482,308) 26-34; step every 4 ticks.
- 9 buttons (str_E1BAC, MI:334-344) — pos / box / spr byte_20/21 /
  handler: NewGame (206,67) 80×80 59/51 → map screen; SetName (281,65)
  80×80 60/52; Multiplayer (362,72) 80×80 61/53; Save (200,157) 80×80
  62/54; SetKeys (405,231) 60×44 106/106; Load (391,158) 80×80 63/55;
  Exit (294,25) 52×44 64/56 (Esc auto-selects); Language (289,155)
  60×44 65/57; Joystick (185,232) 60×44 66/58. Hover draws byte_21 (the
  INVERSE of the map table — resolve visually); click sample 14.
  Sprite 66 also blitted permanently at (185,232) (MI:897).
- Name entry `SetPlayerNameDialog_78E00` (MI:4799): chars
  space/0-9/A-Z/a-z, `_strupr`'d, MAX 12, blinking "_" caret; OK →
  player_name_57ar (+ wizard name); Cancel → empty. Save/Load dialogs
  from the menu = same 8-slot scroll UI as the map buttons.
- Load OK → portals reconstructed from numLevelsCompleted, then into
  the map screen. Exit = scroll confirm → quit.
- Glyph law (sub_6F940 Basic.cpp:1914): glyph sprite = char + 1;
  glyphs are MASKS drawn in the text color (`_setcolor` blit); 0x0A =
  newline; space width = glyph[33].width. GetFont(1) = DATA/FONT1
  (E9B20 = {FONT0, FONT1, HFONT3, FONT1}).

Language file L%d.TXT: skip 4785-byte header, then NUL-separated
strings, 471 entries (sub_5B870 EF:42829). Level descriptions =
entries 23+level; "Empty" = 414; save/load titles 422/421; new-game
confirm 467.

## DrawScrollDialog2 composition (recon 4, 2026-07-18 — player round 2)

`DrawScrollDialog2_7B660` (MI:5488-5688), the ONE-unrolled-scroll law
(the earlier strip-tiling read was wrong — player screenshot):
- roller bar sprite (`17DF06`: case-6 254 / case-4 72, 114×12) drawn
  at (x1,y1) AND again at (x1, y1+open) — top and bottom edges only;
- between them a SOLID parchment fill: (x1+10, y1+barH−2, barW−22,
  open), color = palette nearest 6-bit (0x2A,0x24,0x1D); one vertical
  edge line each side at x1+10 and x1+barW−12, color (0x25,0x1F,0x19);
- title = langindex[word_38_6] in ink (0x16,0x10,0x09), centered
  between x1+10 and x1+10+barW−22 at y = y1+barH+2, shown once
  open > letterHeight+10;
- mode 3 buttons once fully open: OK (`17DF0C`/hover `0E`) at
  (x1+15, y1+height−okH); Cancel (`17DF08`/hover `0A`) right-aligned
  to x1+barW−12, same bottom line — both resting ON the bottom
  roller; keyboard Enter(28)=OK, Esc(1)=Cancel; click sample 14;
- slot rows (Save/LoadGameDialog) at (x1+20, y1+16*(k+1)), k 1-based
  → first row y1+32, under the title line.

## MC1 menu movie law (recon 4): GLOBE/TIMER step ONLY the FLIC
deltas over the live screen — the full-canvas BRUN keyframe never
draws (it differs from MAINMENU.DAT on 58k px incl. a black globe
surround; the menu art already holds globe+hourglass at rest). The
animation = the inter-frame touched set (globe 3024 px). SCROLL.DAT
by contrast IS played whole (PlayInfoFmv) — its last frame = the
save/load dialog backdrop. Movie palettes byte-match MAINMENU.PAL.

## Recon — the IN-LEVEL abandon-confirm dialog (2026-07-18, opus agent)

Retail MC2's in-flight Esc does NOT abandon directly — the law
(citations = remc2/engine/):

- Esc (scancode 0x01) in every in-level input sub-mode →
  `sub_18B30()` (PlayerInput.cpp:587-89, 940-43, 1037-39, 2300-06).
- `sub_18B30` (PI:1946-63): guarded by the finale flag (byte[2]&0x20);
  first call queues `HandleButtonClick_191B0(20, 13)` + sets
  `SelectedMenuItem_38546 = 1`; the command-20 consumer
  (EventsFunctions.cpp:37715-37) calls `SetMenuCursorPosition_52E90`
  → `MenuState 13` = "show in-game abandon game yes/no dialog"
  (GameUI.cpp:637-39; 14 = the same dialog from a map-side submenu).
  A SECOND confirming call (MenuState already 13/14) queues 29+27 —
  29 = Banished (EF:37773-87) ends the level → world map.
- The dialog `DrawOkCancelMenu_30A60` (GameUI.cpp:4591-4636): prompt
  `sprintf("%s?", langindexbuffer[2])` = "Abandon level?" (decompiler
  comment English; byte-verify vs the packed language DAT = OPEN),
  drawn with the IN-GAME font over the LIVE view inside
  DrawGameFrame (alive: at (132,50), EF:21779-81; dead/map variant
  at (6,6)); buttons = MSPR sprites 257 (OK) / 258 (CANCEL) from
  DATA/MSPRD0-0.DAT — the in-level HUD/spell bank, 50×32 each,
  contiguous pair centered on screen
  (GetOkayCancelButtonPositions_30BE0). The same widget serves
  Load (SelectedMenuItem 2, string 423) and Save (3, 424).
- Inputs (ReadOkayCancelButtonEvents_19E00, PI:2356-2438): confirm =
  Enter 0x1c or the OK sprite; cancel = Esc or the Cancel sprite
  (restores the pre-dialog MenuState saved in byte_38544). No Y/N.
- MODALITY: retail does NOT pause — GAME_PAUSED is a separate
  toggle; PlayerEvents + UpdateEntities run every frame regardless
  (EF:31796, 31804-17); only sprite anim + entity-physics tail +
  entity sounds gate on GAME_PAUSED. The world runs and sounds under
  "Abandon level?"; the dialog merely replaces the movement read.
- ASSETS: everything is in-level (in-game font + language table +
  MSPR bank) — the frontend HSCREEN bundle is never touched.
- MC1: NO confirm at all — remc1 :20539-43 fires MakeControlCommand
  27+29 on the FIRST Esc; no OkCancel code exists in remc1.

Ported 2026-07-18 (mgc-app main.rs `exit_confirm` + ui.rs
`exit_confirm_quads`): faithful prompt/sprites/inputs/no-pause; the
dialog extended to MC1/HW/single-level by player directive (retail
left them unguarded). Deviations: soft readability slab behind the
prompt + hover tint (presentational); Load/Save prompt reuse not
ported (no in-level load/save in the remake).
