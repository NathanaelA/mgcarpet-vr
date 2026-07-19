# Roadmap

Working plan, updated as phases complete. History: phase 1 = bootstrap +
RNC + DAT/TAB (committed), phase 2 = level parsing (MC1/HW/MC2) + .mgcl
format + bake (committed), phase 3 = MC2 terrain oracle, byte-validated
(this commit).

## FRONTEND/GAMEPLAY SEPARATION — LANDED 2026-07-18 (playtest owed)

The player-directed architecture session ("think of the menu more as a
loader for multiple instances of the gameplay, separate from them"):
the frontend and gameplay were siamese twins — the app pre-loaded a
level at boot and the menu/map screens rode on top of the frozen sim,
so level ambient (wind etc.) kept sounding under the menus and MC1's
menu inherited whatever music the loaded level was playing.

**The split (mgc-app/src/main.rs):**
- `Session` = one running gameplay instance (LoadedLevel + Simulation
  + flyer/pose interpolation state), held as `Option<Box<Session>>`.
  `Screen { Level, Menu, Map }` replaces the `menu_screen`/
  `map_screen` bool pair. Invariant: frontend screens hold NO session.
- **The frontend is the loader**: a campaign boots LEVEL-LESS into
  its main menu (`main()` skips `load_level` entirely; App::new takes
  `Option<LoadedLevel>`); the session is constructed by
  `install_level` when a portal click / Continue launches, and torn
  down (`teardown_session`) on every exit to the hub (Esc-abandon,
  won-edge routing, map Exit). Single-level mode still boots straight
  into its session; the headless instruments (screenshot/map/probe)
  load a level as before.
- **Teardown** drops the sim wholesale and cuts every level sound:
  new `Audio::stop_sounds` → `FaithfulMixer::reset` (stops all 32
  channels, wipes request slots + ambient wishes — the retail
  frontend transition stops the whole SFX system, remc1 :59992-94
  `sub_5D010_5D520`), plus `stop_speech`. New `Renderer::clear_level`
  drops the terrain buffers + billboards/bars/lights/sky so nothing
  of the world can render under the frontend. Direct level→level
  chains (the demon-mouth dive) cut the old level's sounds inside
  `install_level` the same way.
- **Per-mode music**: the frontend owns its menu set —
  `frontend_music()` plays MC1 `csetup` / MC2 `mc2-menu`; the level
  music starts with the session install and dies with the teardown.
- **MC1 MENU MUSIC TRACED** (opus decompile agent): the banked "cue
  0xD" was a RED HERRING — `sub_5D070_5D580(0xD)` at remc1 :58945
  loads the menu's SFX SAMPLE bank (`data/snds13-N.dat`; the function
  is a sample loader, its name lies). The actual menu music start is
  :58992 `sub_5D290_5D7A0(4)` = song id 4 of music bank 0
  (`sub_5CEF0_5D400(0)` → `data/music0-N.dat`) = **CSETUP.HMP** —
  exactly MC2's SETUP-track law (StartMusic(4)). Menu→level switches
  song index only (4 → random 1-3 = cgame1-3, :41578-82). `csetup`
  was ALREADY BAKED in mc1-audio (FM + GM renders) — no bake change,
  no epoch bump. (Also mapped: variant suffixes — music0-0 = FM/OPL,
  music0-2 = GM; snds13 suffix = free-RAM quality tier.)
- **Atlas juggling retired**: `map_atlas_live`/`menu_atlas_live` and
  the swap dance replaced by one `ui_atlas: UiAtlas` owner tag
  (Level/MapScreen/MenuMc2/MenuMc1/FrontendUi); every screen
  re-uploads only when it isn't the owner. The P options menu over a
  frontend screen draws on black with a frontend-owned `UiAssets`
  copy (harvested from the torn-down session, or lazily from the
  game's variant bundle) — it no longer needs a frozen level's atlas.
- Frame loop: `RedrawRequested` dispatches exclusively — frontend
  screens tick/draw via `frontend_frame` (no sim, no accumulator,
  default camera over the cleared renderer); the gameplay body runs
  only with a live session. The quickselect/pool telemetry, camera,
  HUD and won-edge blocks all moved onto the session accessors.

Presentation change (deliberate): the P preferences menu over the MC2
map now draws on black (there is no frozen level to show beneath) —
the map freezes while it is up, as before. MC1's Continue at boot no
longer double-installs the level (the old harmless quirk is gone —
the menu launch is now the ONLY install).

Verified: workspace tests green (195 sim + goldens strict under
MGC_REQUIRE_GOLDENS=1 — no sim changes), clippy clean on the three
touched crates, headless screenshot path intact, and all three
campaigns (`--campaign mc1|mc1hw|mc2`) boot level-less into their
menus with the right music and no errors (6-second smoke runs).

**Round 2 (player, same day) LANDED — pointer-capture policy + MC1
name prefill**: (1) The capture ladder is now per-surface (player
directive): MENUS run a FREE pointer (MC2 temple: unconfined, OS
cursor hidden over the window since the screen draws the retail
cursor sprite — new `free_menu_pointer`; MC1 globe: free + OS cursor,
as before), the MAP alone confines (edge-pixel scrolling needs the
boundary), the GAME alone locks. Esc map→menu releases the
confinement automatically (`enter_main_menu` asserts the menu
state); the CursorMoved clamp/warp now fires ONLY on `Screen::Map`.
P-pause re-asserts the surface's pointer state on close (in-level:
re-lock if flight held it; map: re-confine — this also fixes the old
visible-OS-cursor-over-the-map leak after a P round trip; MC2 menu:
re-free/hide). This resolves the report "Esc does not release the
pointer from the MC2 menu" — the menu simply never captures now.
(2) MC1's rename dialog pre-fills the current save name for EDITING
(MC2's already did; `Mc1Menu.player_name`, refreshed with the entry
slot scan and after every rename commit) instead of asking from
scratch.

**Round 3 (player, same day) LANDED — the missing MAP AMBIENCE**: the
retail map's ambient bursts (Cymmerian screams 38/23, volcano/meteor
whoosh 5, the falling-star loops 46-58) had gone silent. ROOT: the
map REQUESTS its sounds into the mixer (`wm.take_sounds()` →
`a.event`), but the mixer only emits on `Audio::tick` — the 24 Hz
flush that lived exclusively inside the SIM tick loop, which the map
screen freezes (the drain arm silently starved every frontend mixer
request when `map_screen` joined it; music/narration survived because
they bypass the mixer). FIX: the frontend owns a wall-time 24 Hz
mixer pump (`frontend_audio_accum` in `frontend_frame`) — map bursts
and MC2 menu clicks flush again, fade ramps run, and the narration
DUCK now recovers on the map (menu music no longer stays at 1/3
volume after a briefing line — a latent bug the same starvation
hid). Plus: `teardown_session` clears the danger-music wish
(`set_danger(false)`) so a mid-combat exit can't leave the danger
ramp armed under the frontend. LESSON: a request/flush split dies
silently when a mode gates the flush — every surface that REQUESTS
sounds must also own a path to the FLUSH.

**Round 4 (player, same day) LANDED — the in-level ABANDON-CONFIRM
dialog** (player: an accidental Esc after an hour of grinding must
never toss the level). Retail law traced by an opus agent (remc2):
in-flight Esc (PlayerInput.cpp:587) → `sub_18B30` → MenuState 13
("in-game abandon game yes/no dialog", GameUI.cpp:637-39) — retail
MC2 DOES confirm; the second confirming action queues command 29
(Banished) which ends the level to the map. The dialog
(`DrawOkCancelMenu_30A60`, GameUI.cpp:4591-4636): prompt =
langindexbuffer[2] + code-appended "?" → **"Abandon level?"** in the
IN-GAME font, over the LIVE view (no panel, no separate screen),
above a centered contiguous OK/Cancel sprite pair — **MSPR 257/258
from the in-level HUD bank** (GameBitmapIndexes.h SPELL_BUTTON_OK1/
CANCEL1; the same widget serves Load=2/Save=3 prompts via
SelectedMenuItem). Confirm = Enter (0x1c) or the OK sprite; cancel =
Esc or the Cancel sprite (ReadOkayCancelButtonEvents_19E00,
PI:2356-2438 — no Y/N keys). **CRITICAL modality fact: retail does
NOT pause** — GAME_PAUSED is a separate toggle; the world keeps
simulating and sounding under the dialog (EF:31796); the dialog only
replaces the input read (steering decays, speed persists — the
map/book law). **Everything draws from in-level assets** — no
frontend bundle involved (the player's hope, confirmed).
**Retail MC1 has NO confirm** (remc1 :20539-43 fires the abandon
27/29 pair on the FIRST Esc, and no OkCancel code exists in remc1) —
extending MC2's dialog to MC1/HW/single-level is a deliberate
player-directed improvement.
SHIPPED faithful: Esc(ungrabbed) → dialog; Enter/OK-click confirms
(MC2 → map, MC1 → menu, single-level → app exit); Esc/Cancel-click
stays; world keeps running beneath (tick_input treats the modal like
the book — no movement, no fire; all other input swallowed); MC2
draws the real 257/258 sprites (50×32, contiguous centered pair =
retail GetOkayCancelButtonPositions geometry), MC1 falls back to
labeled slab buttons in the same geometry (its bank has no OK/Cancel
art); prompt "Abandon level?" on a soft readability slab (jar-marker
idiom — presentational; retail's palette font self-contrasted).
OPEN: byte-verify langindexbuffer[2]'s English against the packed
language DAT; retail OK2/CANCEL2 pressed-state sprites unexplored.

**Adversarial review round (opus agent, same day) — 3 findings, all
FIXED**: (1) P-pause held across the won-fade teardown left the audio
output SUSPENDED under the frontend with the options menu stuck open
(teardown cleared `paused` without the matching `set_paused(false)` /
`menu = None`); (2) same across the demon-mouth direct chain — the
next level booted pre-paused (`install_level` now normalizes
paused/menu/audio-suspend too); (3) `enter_main_menu`'s asset-load
failure path bailed before committing `Screen::Menu`, stranding a
dead Level screen with no session (mode now commits before the
fallible loads — failures land in the menu frame's own direct-launch
fallback). Review also positively cleared: no `sess!` panic holes
(the fade-routing mid-frame teardown tail is Option-aware), boot
combinations, atlas-owner ordering, mixer-reset/tick interplay.

PLAYTEST OWED, watch: (a) launch → Esc → relaunch cycling (session
construct/teardown churn — load times, music handoff, no sound
carry-over), (b) the won-edge beats (MC1 → menu with csetup, MC2 →
map, mouth → direct secret chain), (c) P menu over the map (black
backdrop, fonts from the frontend UI bank, close restores the map
atlas), (d) MC1 menu csetup arrangement (FM vs GM follows
`audio.arrangement` as gameplay music does), (e) resume-from-slot
parking still right after the level-less boot, (f) P-pause across a
won-fade (the review-fixed desync — confirm one press now closes and
sound survives into the map/menu). DEFERRED with hooks:
MC1 menu click SAMPLES (the snds13 bank the 0xD call actually loads —
needs a bake member + bank selector; menu is silent-clicks today),
per-mode SFX bank switching generally (retail loads snds13 for the
frontend and snds0 for levels; we keep the gameplay bank loaded).

## CAMPAIGN MENUS + MAP OVERLAY — LANDED 2026-07-18 (playtest owed)

The player-requested five-feature session (decompile recon banked in
the two campaign trace docs, "recon 2/3" sections; BAKE_EPOCH 18):

1. **Gameplay/map separation**: the stage-goal-marker leak into the
   MC2 world map fixed at the ROOT — `sync_world` re-sent the level's
   screen-space map decorations every frame, overwriting the map
   entry's one-time clear one frame later (the stamp pass draws OVER
   the UI quads). The setters are now gated on the frontend screens.
2. **MC2 map pointer law** (retail-exact, MI:3132-75): cursor
   confined to the 640×480 screen (Confined grab + clamp/warp,
   4:3-area edges); scroll fires on the exact boundary pixel
   (0/638/478), X/Y independent (corner = diagonal), accelerating
   4→24 px/frame at the 70 Hz retail clock, reset on release.
3. **MC2 map border overlay** (worldmap.rs): the ornate 640×480 frame
   (quadrant-RLE @0x141E85, decoder `hscreen::border_frame` ports
   sub_85CC3 verbatim) + the four ALWAYS-ON corner buttons (Save
   top-left 250/251, Load top-right 252/253, New Game bottom-left
   248/249, Exit-to-menu bottom-right 246/247; grey idle/gold hover,
   click sample 14) + the parchment scroll dialogs (strip 254, OK
   257/258, Cancel 255/256, 16-px-step opening, 8 slots at 16-px
   pitch, save labels editable — filter space/0-9/letters max 15,
   "_" caret) + the level DESCRIPTION text (strings[23+level] at
   x 130 w 380, y 280/60 by portal half, FONT1, shadowed; dismissed
   by a portal trip, re-arms per visit with the narrative/secret
   suppression law). "Start next level" = the open portal itself
   (retail law). Esc = dialog-close, else exit to menu.
4. **MC2 main menu** (frontend.rs): the HSCREEN0 case-4 temple screen
   (own palette; sequential chunk table pinned — palette@0, bg@768,
   111-sprite bank, fonts), idle fires/incense anims, hover art,
   cursor 39; wired: New Game (enters the map — the reset lives on
   the map corner button, retail law), Set Name
   (12-char uppercase filtered entry → player_name), Save/Load slot
   dialogs (shared parchment law, "Empty" slots), Exit confirm;
   Multiplayer/Keys/Language/Joystick present but report inert.
   Campaign boots into the menu; map Exit/Esc returns there. English
   L2.TXT strings baked (471 entries; L1 is FRENCH — GOG langIndex=2).
5. **MC1/HW main menu** (frontend_mc1.rs + `assets/mc1-ui` bundle):
   retail 320×200 screen composed CPU-side exactly like the VGA path
   — MMMASK hotspot bitmap (pixel value = button id), highlight =
   ×1.30 palette-brighten of the masked region (no overlay sprites),
   GLOBE 30-frame + TIMER 3-frame FMV deltas (new `fmv.rs` decoder:
   Bullfrog 12-byte header + FLIC BRUN/LC/SS2/COLOR chunks; frame
   sizes in the retail files LIE — chunk-sum walking), timer runs
   only mid-campaign; six-slot save/load submodes (mask regions 5-10
   become slots, labels centered in the parchment rects, "--" =
   empty), OK/Cancel at the retail hit rects. Wired: Continue (=
   next level), New Game confirm, Save (label edit, max 20), Load
   confirm, name change, quit confirm. **The between-level beat**:
   an MC1/HW win now returns to the MENU (win/lose FMVs + PPERF
   score screen deferred with the FMV track), Continue launches the
   next level. Preferences: P opens the options menu OVER the MC2
   map (atlas swap dance; the frozen level shows beneath like the
   in-level pause).

Round 2 (same day, player screenshot + report) LANDED:
- **Scroll dialogs redrawn to the real retail law** (DrawScrollDialog2
  MI:5488, banked as trace recon 4): ONE unrolled scroll — roller bar
  at the top and the animated bottom edge only, solid parchment fill
  ((0x2A,0x24,0x1D)) with vertical edge lines between, title in dim
  ink UNDER the top bar, OK at x1+15 / Cancel right-aligned to
  x1+barW−12 resting on the bottom roller, slot rows from y1+32.
  (The first cut tiled the bar sprite — the "stacked scrolls".)
- **Esc never quits**: menus close-modal only (Quit/Exit stays a
  button); in-level Esc = release pointer first, abandon to the hub
  second (MC1 → menu, MC2 → map). App exit only via the menu.
- **Map exit silences the map**: new `mgc-audio stop_speech()` cuts
  the narration mid-clip when leaving for the menu.
- **MC1 menu corrected to the delta law**: retail steps ONLY the
  GLOBE/TIMER FLIC deltas — the keyframes never draw (they differ
  from MAINMENU.DAT on 58k px incl. a black globe surround; the menu
  art already holds the globe and hourglass at rest). Baked
  touched-pixel MASK crops (globe = 3024 px), masked blits; the
  hourglass draws only mid-campaign. Save/Load labels at their
  retail base positions (179,5)/(168,43); dialogs now sit on
  SCROLL.DAT's fully-unrolled last frame (that movie retail DOES
  play whole — the unroll animation itself stays deferred).

Rounds 3-4 (player playtest, same day) LANDED: MC1 menu decoded
from the MMMASK/MMSPR files themselves (hotspot bboxes + sprite
ASCII): the globe = New Game, bottom C:\ = Quit, the six right-side
BOOKS = load (top) / save (second) / the slot list in submode, the
HOURGLASS = Continue — it exists ONLY mid-campaign (MAINMENU.DAT
has bare wall; the whole hourglass lives in TIMER.DAT's keyframe;
baked as visual-diff mask within the id-11 region). Dialogs =
SCROLL.DAT's unrolled screen over BLACK; disk icons (MMSPR 1/2) on
the top two books in the toplevel menu only; C:\-scroll icon (7)
centered in the quit confirm with "Quit to DOS"; "New Game?
Yes/No" (etext 36) confirm gated on actual progress; name dialog =
"What is your name" (retail's name/call-name pair collapsed to one
name, MC2-style — player directive); movie palettes remapped to
MAINMENU.PAL at bake (fmv.rs captures COLOR chunks; index 0 stays
transparent — the black-speck fix).

NEXT SESSION (player-directed): **frontend/gameplay separation** —
the menus/map still ride on a frozen pre-loaded level (ambient
sound bleeds into the hub; MC1 menu lacks its dedicated MIDI, cue
0xD untraced). Target: the frontend as a LOADER of gameplay
instances — a level is constructed when the portal/Continue is
clicked and torn down on exit, with per-mode audio ownership. See
memory frontend-gameplay-separation.

Deferred with hooks: the scroll.dat unroll animation, PPERF score
screen, win/lose FMVs, MC1 menu music (cue 0xD mapping untraced),
MC2 attract mode, SetKeys/Language/Joystick screens. OPEN
(playtest): temple hover sprite polarity (grey/gold tables inverted
between map and menu — flip if buttons render dark), sprite 66
permanent blit vs the Joystick button, edge-scroll feel.

## CAMPAIGN STITCHING — CORE + MC2 WORLD MAP LANDED 2026-07-18
## (playtest owed)

Scope agreed with the player 2026-07-18: campaign core + the MC2 world
map now; intros/FMV, cutscenes and graphical menus deferred (hook
points print). Entry = `--campaign <mc1|mc1hw|mc2>` (+ `--slot N`
1-based, `--new-game`) per player directive — the retail SELECT.EXE
launcher is replaced by the CLI. Decompile ground truth banked in
docs/traces/mc1-campaign-save-menu.md + mc2-campaign-save-menu.md
(campaign structure, BOTH games' save formats, menu/intro flow, map
screen, HSCREEN0 chunk table, exit-marker draw law).

- **Campaign law** (`mgc-app campaign.rs`): MC1 linear 0-49 with skip
  table {8,17,28,33,39}, HW 0-24 no skips; MC2 mains 0-24 (portal
  index = level number, 24 = finale) + secrets 30-34 off mains
  4/7/11/17/19, portal tables verbatim from
  Type_MapScreenPortals_E17CC / Type_SecretMapScreenPortals_E2970.
  Exit routing: demon mouth (endseq target model 4) always jumps
  straight into the attached secret; checkpoint X returns to the map
  unless the secret is revealed-uncompleted (traced 38545&0x10 arm —
  VERIFY in playtest). Sim: new hash-neutral `mc2_exit_model()`.
- **Retail-format saves** (`saves.rs`), read AND write, per-game dirs
  `saves/{mc1,mc1hw,mc2}/`: MC1/HW `carpddNN.gam` 142 B × 6 slots
  (obfuscated level word; blob24 = the var_15318 collected-spell
  flags — the campaign spell memory, now typed); MC2 `SAVEn.GAM`
  1319 B × 8 slots with the 505-byte str_611 player block fully
  mapped (banked/volatile XP, learned set, tiers, sel, hand binds at
  exact offsets; rest carried opaquely byte-exact). Old GOG saves
  drop into saves/<game>/ and load; ours load back into retail.
- **Driver** (main.rs): won() edge → fold completion into the slot
  record (MC2 opens the portal prefix + secret states + serializes
  the live book; MC1 commits collected flags), persist, fade, then
  in-place level switch (`install_level`: sim rebuild + carry
  re-grant + renderer re-upload + music). Carry law: MC1 grant =
  collected ∩ level availability mask (retail :49226-33); MC2 =
  `mc2_grant_plausible` with banked XP (the sub_549A0 carry).
  Castle-less-death restart re-grants. Plausible-spellbook
  instrument auto-disables in campaign mode.
- **MC2 world-map screen** (`worldmap.rs` + `assets/mc2-ui` bundle,
  BAKE_EPOCH 16): HSCREEN0.DAT case-6 chunks (1280×960 bg, 6-bit
  palette, 313-sprite bank — HSPR signed-RLE, empirically pinned;
  the recon agent's "raw 8bpp" claim was WRONG). Boot/between-levels
  hub: scrolling crop anchored to the next portal's authored
  viewport, move-keys pan, portals animate per state (flag 37-43,
  closed 70, secret 270-272/305-311), bank-sprite-39 cursor, click =
  launch, completed = replay. Sim frozen while up; UI atlas swapped.
- **In-level X/O exit markers**: MC2 (14,3)/(14,4) live entities →
  HUD-bank sprites 83/84 centered via the map_stamps path
  (GameUI.cpp:2049-53 law; hidden dis-gated markers appear once
  revealed).

### Map-screen polish round (same day, player playtest feedback)

Player round 1: cursor showed a flag, no pop-in/travel/ambience, HUD
quest-marker leak. All re-traced from MenusAndIntros.cpp and LANDED:
- **Cursor = bank sprite 239** (MI:986 — the map chunk's own; "39"
  was the MAIN-MENU chunk's cursor, the two case-4/case-6 sprite
  chunks share the runtime array).
- **Portal materialization** (MI:2790-2827): completed = flag 37-43;
  the NEXT portal pops in once per session (sound 41, 70→83) then
  idles as the OPEN portal 33-35 (not the closed 70 swirl); later
  portals not drawn (loop breaks). Secrets pop the same way before
  270-272 / 305-311.
- **Travel carpet** (SetAnimationVariables_7DA70 / sub_80D40 /
  MoveAnimObject_7E9D0): 8 heading families ×4 frames = sprites
  1-32, ~6 px/frame Bresenham, trail dot sprite 139 stamped every
  >8 px (the dotted route; session-persistent like retail's bg
  buffer), travel sound 19; entry leg flies last-completed → newly
  revealed portal and the pop-in waits for arrival; a CLICKED portal
  plays the leg first, launches on arrival. Heading-family compass
  orientation is a best-guess — VERIFY carpet facing in playtest.
- **Ambient set dressing** (x_BYTE_E26C8_str MI:199-216 +
  DrawAnimSprite_81CA0 EF:46934): 14 rows ported — loop rows always
  drawn, burst rows INVISIBLE during their delay then one cycle
  (frames first..last-1, 12.5 fps); the frame-85/86 cluster vanishes
  once the finale portal opens. Ambient sound hooks skipped.
- **HUD leak fixed**: entering the map clears the renderer's
  screen-space map overlays (stamps/path/objective marks) — the
  frozen level's quest marker was bleeding through (player report).
- Anim cadences retail-derived: 100 Hz clock → portals/ambients
  12.5 fps, carpet frames 6.25 fps.

Player round-1 notes also settled scope: MC1/HW menus = NEXT SESSION
(the menu pause is the retail level-transition beat; direct stitching
is too seamless), and the MC2 map's 4-button edge overlay
(save/load/next/exit — mostly redundant with portals + auto-persist)
rides the same menus session.

### Map round 3 — sounds, narrative, Esc-to-map + a progression-law
### fix (same day, player feedback)

- **Next-level narrative**: `PresentLevelDescription_80C30` (MI:3569-
  3601) = description text ETEXT[23+level] + speech row=level seg 0,
  once per map visit (`IsPlayingCDTrack` latch), SUPPRESSED while the
  pending level's secret portal is non-hidden. Ported: speech fires
  after the portal pop-in completes; the description TEXT rides the
  deferred map-overlay/font work (the map chunk's own glyphs at 254+).
- **Ambient sounds**: the burst rows with `time4 != -1` fire samples
  on the wait→anim edge (EF:46999-47009): the two head-poke screams
  (sample 38 — the leviathan/cymmerian calls), the (545,54) burst
  (23), and the per-cycle meteor whoosh (5 — the burst twin of the
  (831,245) loop). Loop-row phase offsets restored (the authored
  start frames de-sync the falling stars). The frames-46-58 loops ARE
  the falling-meteor streaks — one sits by portal 3's region, the
  "find the star that fell" level (player's remembered pre-level-4
  meteor).
- **Esc in an MC2 campaign level → back to the world map** (retail:
  leaving a level lands on the map, not out of the game); nothing is
  recorded, the level replays from its portal.
- **PROGRESSION-LAW FIX (the important one)**: the demon-mouth exit
  does NOT complete the parent — portal promotion only happens in
  PortalsUpdate on a map visit, and the mouth path skips the map;
  the SECRET's completion promotes both (its PortalsUpdate arm
  force-activates the parent). A failed secret leaves the parent
  pending + secret revealed → narrative suppressed + the X-exit
  routing arm sends a parent replay back into the secret. Driver
  updated (mouth/X-with-pending-secret don't advance; secret
  completion advances the parent). See the DERIVED law note in
  docs/traces/mc2-campaign-save-menu.md.

### Map round 4 — first completed-level playtest fixes (2026-07-18)

Player completed mc2:00; three findings, all LANDED:
- **Save/complete LOOP (the bad one)**: the won-edge gate was
  `quit_fade.is_none()` — the map screen consumes the fade, so the
  finished level (still rendering beneath) refired the edge every
  frame: "completed" + a slot write + re-entry sounds per frame (the
  "corrupted sound" = the CD narration/portal samples stuttering
  under the loop; our save itself plays NO sound). Fixed with a
  `won_handled` latch (reset on install/restart). NOTE (player,
  retail observation): the GOG build's own save-confirm blip is
  corrupted — if we ever add the retail save sound, mute/skip it.
- **Map music**: the map now plays the frontend track (`mc2-menu`
  render; retail StartMusic(4) menu set carries across the map);
  level launch restores level music.
- **Exit X/O timing**: the map marker must show from level START
  (it marks where the exit will be) — the dis-gated (14,3)/(14,4)
  entities spawn HIDDEN and never reached the pose list. New
  read-only sim getter `mc2_exit_marker_poses` (hash-neutral) feeds
  dedicated stamps.
  ROUND-2 CORRECTION (player): the marker sits on the **TRIGGER**,
  not the fly-to portal — the (11,12)/(11,31) ENDING TRIP SWITCHES
  (= retail runtime class 0x0B models 0x0C/0x1F in GameUI.cpp:2049-
  53; the agent's untraced editor→runtime remap resolved: they're
  simply the class-11 switch THINGs, distinct entities from the
  class-14 portals). Getter now plots the switches, excluding
  tripped (0x400) ones — retail's trip clears exactly the map-icon
  bit (:54701), so the X vanishes once you've hit it and the endseq
  flies you to the separate (14,x) portal.
- **Entry-leg direction + parked flyer** (player): the carpet flies
  the leg you JUST completed (previous flag → new flag) and PARKS on
  the completed portal (default family 13, MI:956-58 init); the new
  portal pops + narrative fires on arrival. Between legs the carpet
  now rests visibly on the last flag; a click-leg departs from it.

### Map round 5 — travel-sound ruling + parked-flyer law (2026-07-18)

- **Narrative crackle → INVESTIGATED + MUTED (BAKE_EPOCH 17)**.
  Forensics on the GOG rip (player waveforms + our analysis of
  level-01-seg-0): the bursts are DATA-AS-PCM, not sound — flat
  spectrum, no carrier (not modulated), ZCR≈0.5, stereo channels
  UNCORRELATED (r=0.02), 18% 0xFF bytes, entropy 7.04 b/B, no known
  magic/ASCII; the bytes self-match inside the audio track (NOT
  data-track leakage), and burst edges do NOT align to the image's
  2352 B sector grid (≈4- and 12-sector lengths at arbitrary
  offsets) — so not whole-sector rip corruption either; the garbage
  is in whatever source GOG mastered from. PLAYER NOTE: the voice
  continues UNDERNEATH ("Be" of "Before Vissuluth's rule" is drowned
  by the first burst) — unrecoverable by trimming; a clean-source
  rip (community search, player running) could restore it someday.
  FIX SHIPPED: `redbook::mute_leading_junk` — burst = ≥4 consecutive
  sectors RMS≥5000 & ZCR≥0.38 strictly before the first sustained
  voice run; MUTES (not cuts — durations/trigger timing preserved)
  the pre-voice head with a 3 ms onset fade. Speech fricatives
  never reach the burst signature in ≥4-sector runs (measured across
  the library); clean clips byte-identical. Mid-clip bursts (rare,
  interleaved with voice) left alone — indistinguishable risk.
  FUTURE: accept a clean-audio override dir at bake if the community
  surfaces an uncorrupted pressing.
  ROUND-3 TIGHTENING (player caught over-reach — clean in-level
  hints were being clipped, some multi-second): library audit showed
  confirmed junk heads at |corr| 0.04-0.17 vs wrongly-eaten content
  at 0.3-0.6. New law: SEGMENT 0 ONLY (the corruption lives at track
  heads = map narratives; hints never affected), junk sectors must
  ALSO be |corr|<0.25, and the mute stops at the END of the last
  junk run (never extends to the detected voice onset). Final:
  16 clips, 10.8 s — hints verified restored, crackle clips still
  clean. FLAG-COLOR CHECK (player query): main flags 37-43 = WHITE
  (224,224,232); completed-secret marker 305-311 = RED (252,36,36)
  straight from the bank — the red "capture flag" on a conquered
  hidden world is FAITHFUL (the parent main flies the white flag).
  ROUND-2 REFINEMENTS (player sample-level inspection): (a) byte-
  insertion/misframing hypothesis TESTED AND DEAD — the burst core
  is white at all 4 byte framings (no realignment recovery); (b) the
  player's outlier-delete+stack-left filter REFUTED — the plausible-
  looking low samples in the corrupt region are ALSO white (survivor
  ZCR 0.52 vs voice 0.14), the stream was replaced, not laced;
  (c) NEW ONSET DETECTOR: per-sector STEREO CORRELATION (junk r≈0
  σ0.04, voice r≈0.87 — 0.6 is 15σ, un-fakeable by noise) joins ZCR:
  voicey = RMS≥1500 && (ZCR<0.30 || corr≥0.60). This preserves
  syllable onsets still buried under fading junk (level-1's
  "Be[fore]": sector 38 r=+0.69 now survives) and un-mutes 8 clips
  whose "junk" turned out to sit after real correlated audio.
  Final: 46 of 138 clips, 58 s silenced.
- **Travel sound (player ruling)**: sample 19 plays IMMEDIATELY with
  the portal click (retail starts it late — its 640×480 map was
  sluggish and the dynamic differed), but only when the flyer
  actually travels (retail's own leg-length gate, MI:3786; a click
  on the portal the carpet rests on stays silent).
- **Parked-flyer law**: the synthetic entry leg is GONE — on map
  entry the carpet parks on the portal of the level JUST PLAYED
  (`run.current`: completed, failed via Esc, or a replayed older
  level — player-described), the new portal pops on sight +
  narrative after; travel legs exist only for clicks and depart
  from the parked spot. Save/load: the .GAM stores NO position —
  retail itself resets to the last activated portal on load, which
  is what `run.current` resolves to on resume.

### Map round 7 — the dotted-route law (player, 2026-07-18)

Player correction: the dotted line is NOT a travel record — it is
the FIXED main-line path (identical on every load); the only per-
segment question is drawn-or-not. LANDED as: segments between
completed portals + the frontier segment stamp on every map entry —
EXCEPT the frontier segment in the visit immediately after the
completion that revealed its portal, which stays blank until the
carpet actually flies the canonical leg (last-completed flag →
pending portal); any other flight (off-route origin or destination,
replays, secrets) draws nothing and the segment appears on the next
entry instead. Loads always show the full trail (session-transient
"just revealed" state only). Finale done = whole route (retail
MapMenuPortalsDraw_81760). Freeform live-stamp trail deleted; dots
= sprite 139 at ~12 px spacing along portal-center lines, the
in-flight canonical leg drawing itself up to the carpet.
(Also banked: player will try to obtain the ORIGINAL CD audio
tracks — a clean-vs-GOG diff could drive a reconstruction filter
for the narration corruption, feeding the bake override hook.)

DEFERRED (hook points in place): intro/cutscene FMVs (Bullfrog .DAT
decoders located: remc2 Animation.cpp:41-77, remc1 PlayInfoFmv_107C0;
MC2 cutscene table CUT1-6 after mains 4/8/12/16/23/24), MC1 win/lose
FMVs + outro, graphical menus + save-slot UI (CLI slots only; incl.
the MC2 map edge-overlay buttons), level-description text + map
music + finale route-draw (MapMenuPortalsDraw_81760), ambient/travel
sound variants, retail right-click-only replay nuance. OPEN: does
retail let you click mains other than next/completed; the
`x_WORD_17DB8A == -1` trail-stamp gate (we stamp on every leg);
editor (14,3)→runtime (11,12) remap untraced (cosmetic); post-finale
map (completed slot currently refuses resume without --new-game).

## BANKED: game-manual naming reconciliation (cleanup final phase)

Player directive 2026-07-16. Our creature/entity/spell names accreted
from decompile spelunking and survey nicknames, and they drift from what
the devs actually called things — the MC2 m27 went "multipart
tree/kraken" in our docs until the player identified it as the **HYDRA**
(5 fireball heads that retract/regrow; body attackable only with all
heads down). In a final cleanup phase, reconcile ALL names against the
GAME MANUALS (MC1 + MC2; the GOG installs ship them) as the canonical
source: sweep docs/SURVEY-*, docs/traces/*, code comments/identifiers,
and the census tools so every (class,model) and spell uses the manual
name, keeping old nicknames as greppable aliases where they already
permeate trace docs. Scope is naming/docs only — no behavior. Do it
LATE (after the remaining port/review sessions) so it doesn't churn
active work.

## MC2 spell-verification track — SESSION 4 (2026-07-14)

Fool's Mana (22), Magic Mine (23), G4 spell-names, and the Metamorph
(4)+Summon-Army (19) creature pair all LANDED. 14 channel tests + full
12/12 suites green, MC1+MC2 goldens hold, no bake-epoch bump. The
creature pair's "blocked on control-routing" verdict was overturned — an
Opus trace proved Metamorph is a cosmetic pose-puppet (no control rebind;
the out-of-pool `human_pose` model fits cleanly). Full ledger + citations
in `docs/spell-audit/00-PLAN.md` (Session 4 block) and the
`mc2-spell-verification-track` memory. Remaining spell work = the rival
track (Duel 14 tether, Alliance 24, Rebound 8 tiering, Beyond-Sight T2);
deferred presentation = metamorph carpet-hide + the spell-name level-up
banner.

## Phase 4 — the carpet flyer (LANDED 2026-07-04, MVP)

Flying over every baked level (MC1 + HW + MC2) with authentic terrain,
MC1 palette colors, seamless torus wraparound, and a headless
`--screenshot` mode for autonomous verification.

What shipped:
1. **Assets** (importer): MC1 `DATA/PAL{0,1}-0.DAT` → 8-bit RGB
   `baked/mc1/assets/palette-{day,night}.bin`, plus the tile-type →
   palette-index map (256 bytes at +0x14000 of decompressed TABLES.DAT,
   the exact lookup the engine's map view uses — remc2 GameUI.cpp) →
   `tile-colors.bin`. NOTE (resolved 2026-07-05): MC2's `DATA/` tree
   lives on the CD image inside the GOG install; the importer now reads
   `game.gog` directly (`mgc_import::iso` + `gamedata` source layers),
   so the full MC2 catalog — `DATA/`, `SOUND/`, `INTRO/`, the redbook
   soundtrack — is available for baking. MC2 levels still render with
   the MC1 day palette as a stand-in until MC2 bundles are baked
   (geometry is already correct).
2. **mgc-render**: wgpu terrain pass — 256x256 grid mesh, engine-authentic
   alternating tile diagonals (`(tx+tz)&1`, sub_B5C60) and vertical scale
   (1 height byte = 1/8 tile; engine computes `32 * h`); terrain-type
   byte + color LUT resolved in the fragment shader (palette-as-LUT per
   README); per-vertex hillshade; distance fog; 3x3 instanced wrap
   copies for the toroidal horizon. Offscreen target + PNG readback for
   `--screenshot`.
3. **mgc-app / mgc-sim**: fixed-timestep loop (30 Hz placeholder tick,
   render interpolation with wrap-aware lerp), WASD + mouse-grab flight
   with inertia, ground clamp, world wrap. Flight constants are
   placeholder feel — eyeball against remc2 side-by-side before habits
   form (still open).
4. Level picker: `mgcarpet --level baked/<game>/level-NNN.mgcl`;
   screenshot mode `--screenshot out.png [--camera x,y,z,yaw°,pitch°]`.

Landed after the DOSBox map-screen comparison (user snapshot of MC1
level 1 — coastline shape confirmed): `terrain/shading.bin` baked from
the oracle (+0x20000) plus `shade-lut.bin` (TABLES.DAT +0x0000, 64x256),
and the renderer now resolves colors through the engine's full path —
`palette[shade_lut[shade][tile_colors[type]]]` — giving the quantized
per-tile light and dithered ocean of the original. IMPORTANT finding
from that comparison: MC1's fullscreen map draws actual TMAPS textures
top-down (green land, forest speckles), NOT flat table colors — flat
table colors (brown/tan for level 1) are MC2-map-style and our current
stand-in. The green look arrives with the TMAPS texture track. Also:
MC1's map view doesn't fit all 256 tiles on screen (bits wrap beyond
the edges) — remember when building the map UI.

Phase 4 follow-ups: ~~MC2 CD data extraction + per-environment
palettes/TABLES{D,N,C}~~ (DONE 2026-07-05, see "MC2 environment
bundles" below), feel-tuning pass.
DONE 2026-07-05: the sim spawns at the class-3 m4 player-start marker
(start #0 of 8; original stores position only — spawn faces engine
yaw 0 = north, altitude = ground + hover; `entities::player_start`,
also the default `--screenshot` camera). The exact hover height and
the yaw-0 = north reading are to be confirmed in DOSBox comparisons.

### Textured terrain (LANDED 2026-07-04)

MC1 terrain now renders with the original's textures. Key findings,
all confirmed against remc2 + retail data analysis:

- **TMAPS is NOT the terrain texture set** (roadmap previously assumed
  so). Terrain ground tiles come from the BLK/BLOCK atlases; TMAPS
  holds sprite/billboard/animated textures (water surfaces, overlays).
- **MC1's `{0,1}` file suffix pairs are world tilesets, not day/night**:
  0 = temperate, 1 = arctic (snow, pines, stone-brick buildings) —
  spanning PAL, BLK, TMAPS, BUILD, sprites. `DTABLES.DAT` (stored
  uncompressed) is the arctic counterpart of `TABLES.DAT`. The old
  `palette-{day,night}.bin` assets are renamed to per-set
  `palette-{0,1}.bin` etc. MC1 apparently has no night variants at all.
- Importer bakes per set N: `palette-N.bin`, `tile-colors-N.bin`,
  `shade-lut-N.bin`, `terrain-atlas-N.bin` (from `BLKN-1.DAT`, the
  32x32-cell high-detail atlas; 256 px wide, 8 cells/row, 152 cells;
  `BLKN-0.DAT` is the 16px low-detail variant, not baked).
- **The terrain-type byte IS the atlas cell index** (identity, exactly
  like remc2's MC2 path: `textIndex = mapTerrainType[cell]`). Verified
  by matching per-cell average colors against the map-view tile-colors
  table — near-exact on every common type, i.e. the +0x14000 table was
  itself generated as texture averages. Type 0 = the water texture;
  village/castle building textures are terrain types ~8-40 (buildings
  are painted terrain in this engine).
- Renderer: atlas texel resolved in the fragment shader through the
  same engine path as before, minus the flat-color hop:
  `palette[shade_lut[shade][texel]]` (remc2 GameRenderOriginal mode-7
  inner loop). Flat tile colors remain the fallback for levels without
  a baked atlas (none since MC2's bundles landed 2026-07-05).
  `--tileset 0|1` selects the MC1 set at launch.
- TMAPS archives fully parse (`tmaps.rs`): 10-byte TAB entries
  `{u32 unpacked_size, u32 offset, u16 group}`, per-entry RNC, payload
  `{u16 flags, u16 w, u16 h}` + 8bpp pixels. Groups = animation frame
  runs. Baking/rendering them is the sprite/water track.

UV orientation (LANDED 2026-07-04, player-reported 180°-rotated shore
tiles): the generator's angle plane (+0x30000 of the oracle block,
previously discarded) now bakes as `terrain/angle.bin` (additive
member, FORMAT.md updated); bits 4-6 select the tile texture's
orientation. remc2's `UVTable_D4350[32]` decodes as
`row = 4*orientation + camera_base`; at base 0 (world space, ours) the
8 orientations are dihedral: bit4 = flip x, bit5 = flip y, bit6 = swap
axes. Fixes directional transition tiles (shorelines) AND kills the
tiled-pattern repetition — Bullfrog's textures are edge-symmetric and
the varied per-tile orientation is what makes them tile organically.
The `camera_base` term (remc2 `v248x[32]`) is assumed to be the
original renderer's view-quadrant compensation and intentionally not
replicated.

Overhead map / book screen (LANDED 2026-07-04): Enter toggles the
original's book screen, laid out per the player's DOSBox capture — map
pane LEFT (~60%), live world viewport top-right, spell list
bottom-right (dark placeholder until spells land). Map = flat-color
overhead, one pixel per tile via the engine's map path
`palette[shade_lut[shade][tile_colors[type]]]` (`mgc_render::map_pixels`)
plus a player marker; axis-aligned and static (the original
floats/rotates with heading — deliberately not replicated; ours is the
cleaner comparison instrument). Headless: `--map out.png [--map-scale
N]` writes the pure map PNG (no marker) for 1:1 diffing against DOSBox
map screenshots; `--screenshot ... --map-view` renders the book screen.
The original's map pane additionally shows entities (villages, trees,
the player's balloon as sprites/dots) — ours is terrain-only until the
entity track; its map also zooms (doesn't fit all 256 tiles).

Textured-terrain follow-ups:
- Arctic tileset selection: RESOLVED (2026-07-04, remc1 clone — see
  "MC1 reference generator"). The tileset is per level BUNDLE, not per
  level: the engine's only switch is the Hidden Worlds mode flag
  (remc1 `IsHiddenWord`, sub_main.cpp:49842), which selects
  `tmaps1-0` + `DDLEVELS` vs `tmaps0-0` + `LEVELS` (and by extension
  the whole `{0,1}` asset family). So: bake/render Hidden Worlds
  levels with tileset 1, campaign levels with tileset 0; `--tileset`
  stays as an override. The earlier snlin<200 hypothesis is dead as a
  selector — snlin is never read by the engine at all (vestigial
  editor data); the snlin<200 campaign levels (10, 17-18, 34-47, 50,
  63-66) would render temperate like everything else in LEVELS.
  Cross-check against play memory that base-campaign MC1 indeed has
  no snow worlds (remc1's own HW support is stubbed: IsHiddenWord is
  never set true, and dtables/blk1/pal1 are unreferenced in the
  decompile).
- Water/lava animation: RESOLVED twice over — the payload mystery fell
  with the FLC decode (see "Billboarded sprites"), and the animated
  ocean turned out not to involve TMAPS at all (see "Terrain water
  animation", LANDED 2026-07-05: per-vertex sine displacement, the
  texture cell is static).
- MC2 textured terrain: DONE 2026-07-05 ("MC2 environment bundles"
  below) — the render path was already environment-agnostic.

## MC2 environment bundles (LANDED 2026-07-05)

Parity step after the gamedata-from-CD-images work (same day): MC2's
four environment graphics sets bake as bundles `mc2-day`, `mc2-night`,
`mc2-night-fog`, `mc2-cave`, and MC2 levels render textured with their
own palettes. All facts traced in remc2 (agent report, session-local):

- **Environment selection** (remc2 ReadAndDecompress.cpp:21-170,
  Level.cpp:878-906): suffix letters D/N/C = day/night/cave; day's
  terrain atlas is the *un-suffixed* `BLOCK32.DAT`; `F` = a second
  night set ("fog"), selected on night levels when the header's
  gfx_type (our level.json field, remc2 byte_0x2FED2) has bit 1 set —
  fog swaps atlas (BL32F) + palette (PALF), keeps night tables/TMAPS.
  5 baked night levels carry the fog bit (campaign 024 among them).
  TMAPS digit = MapType ordinal (0/1/2 = D/N/C).
- **TABLES{D,N,C}.DAT layout differs from MC1**: same 0x14600 total,
  tile-type→map-color still at +0x14000, but the 64x256 shade LUT sits
  at **+0x4000** (a pixel-remap/transparency table occupies +0x0000;
  remc2 Basic.cpp:121-123). Bake slices per-game offsets; baked MC2
  shading maxes at 47, inside the 64 rows.
- **Terrain texturing = identity**, as MC1: the per-tile type byte is
  the atlas cell index (remc2 GameRenderHD.cpp:854), same geometry
  (256 wide, 32px cells). BLOCK16 = the 16px low-detail set (unused by
  us); GTDEF*/FTDEF* ship on the CD but nothing in remc2 reads them;
  GTD2.DAT = a scripted Day-map table swap (event case 5), not a
  per-frame input.
- **Sprites**: TMAPS decode + FLC machinery carried over; MC2 streams
  add palette sub-chunks (4 COLOR_256 / 11 COLOR_64, skipped — frames
  stay 8bpp) and 1-byte placeholder slots (kept as frame-less entries,
  ids stay dense). Sprite atlas width now doubles from 1024 as needed
  to stay under the 8192 texture-dimension baseline (MC2 packs ~9.4k
  rows at 1024). MC1 bundle output byte-identical to pre-refactor.
- **Deferred for later tracks**: MC2 entity billboards (needs the MC2
  (class,model)→sprite mapping — mobs/entities track); SKY{D,N}0-0.DAT
  sky bitmaps (no sky pass yet; cave has none); CLRD-0.DAT 4096-entry
  minimap LUT (remc2 loads only the D file, for the MC2 map-view
  port); per-environment keyColor1/keyColor2 transparency keys
  (ReadAndDecompress.cpp:153-168, matters for the sky pass);
  build/search members (MC2 terrain-feature pass = its own port).
- Previews for player validation: `baked/preview-mc2/` (level-001 day,
  000 night, 024 night-fog, 003 cave + their `--map` renders).

## The authenticity matrix (design, consolidated 2026-07-05)

Player directive, generalizing the scattered per-track notes (sprite
pose backend, palette-as-LUT, smooth_shading, extended controls,
savepoints, enhanced map markers, de-homogenized waves): nearly every
player-perceivable subsystem can be improved without changing the
gameplay — fireball sprites → particle streams, 8-way billboards →
3D models, texture swaps, added GFX, richer audio mixing, extra
controls — and EVERY such improvement is an opt-in alternate beside
an always-available faithful implementation. The in-game options menu
LANDED 2026-07-16: pause (P) opens it — one tab per domain, every
registry option as a row (toggles / choice cycles / 0.1-step sliders /
the fog stop-bar), hover explanations, grey-at-default vs
white-when-changed ink, FONT1 typeface; menu changes apply live and
persist into the sparse `mgcarpet.json` overlay (runtime key toggles
stay session-only). Very few enhancements get implemented soon; the
point is to never build a hurdle against them.

Architecture rules (discipline, not framework — no pre-abstraction):

1. **Sim state is full-fidelity and semantic; presentation resolves
   late.** The sprite-pose constraint is the template: sim carries
   continuous pose, the renderer's backend decides billboard-snap vs
   mesh. Same seam for everything: projectiles are sim entities
   (sprite vs particles is a renderer choice); AUDIO: the sim exposes
   EMITTERS (water tiles, villages, wind-over-relief), the authentic
   mixer implements the original's rule — ambient governed by the
   tile under the player — while an enhanced mixer distance-weights
   all emitters in range ("in a village but hearing distant waves");
   both consume the same emitter data, the original rule is the
   degenerate mix. Don't hard-wire tile→sound when the audio track
   starts. Input already resolves to `FlightInput` before the sim —
   keep that seam.
2. **Every toggle is classed P or G.** P = presentation-only: cannot
   change sim outcomes, freely combinable, replay-neutral. G =
   gameplay-affecting (extended lift, savepoints, difficulty): must
   be RECORDED IN REPLAY/SAVE HEADERS (a replay taped under enhanced
   controls is not a valid quirk-regression fixture for authentic
   physics — the replay suite depends on this), and must never bypass
   authenticity-critical rules (walls block lift, castle-respawn
   semantics).
3. **Options are an open registry** in `config.rs`: id, category
   (graphics/audio/controls/gameplay), class P/G, faithful/extended
   label pair. The menu renders the registry; JSON + CLI flags derive
   from it. Options are ENUMS, not booleans, even while two-valued
   (`sprites: billboard | mesh` leaves room for named alternates like
   community model packs).
4. **Selection happens once, at subsystem boundaries** (renderer
   backend, audio mixer, input mapper, sim rule flags) — never
   scattered `if enhanced` branches inside ported routines. Ported
   routines stay pure, oblivious, and carry their decompile citation
   in the doc comment (the existing de-facto standard = the "direct
   import" label); enhanced alternates get labeled as deliberate
   deviations.

Config layering (player directive, 2026-07-05, IMPLEMENTED same day):
two files, same structure — `mgcarpet.json.defaults` = the faithful
baseline with EVERY option spelled out at its authentic value,
GENERATED by the app when missing (delete to refresh after upgrades;
doubles as the always-current option reference), and `mgcarpet.json` =
optional sparse overrides deep-merged over it. CLI flags override both
per run. `config.rs` `Config::load` does the JSON-level merge; both
files gitignored.

First shipped genuinely-unfaithful enhancement (2026-07-05, player
request, doubles as the event-system debugging instrument):
**`map_trigger_areas`** — tinted circles on the overhead map for live
trigger volumes and portals (amber = fly-into proximity, red =
kill-watchers, cyan = collected-item triggers, violet = portals; light
fill + stronger rim, toroidal wrap). The original never reveals
trigger areas — the player notes it leaves you second-guessing intent
(MC2 "fixes" this with the guiding voiceover), so this is sanctioned
deviation territory AND the practical way to test what fires where.
Off by default; V toggles at runtime; `--map-triggers` for headless
`--map`/`--screenshot` renders. Plumbing: `World::active_volumes()` →
app color mapping → `MapArea` overlay in `map_pixels` (drawn between
terrain and entity dots; direct RGB, deliberately outside the palette
— this layer is explicitly non-faithful). Overlay refreshes when
triggers spawn/fire/expire.

ENTITY RENDER INTERPOLATION (design settled 2026-07-06, player-
initiated; LANDED 2026-07-16 as `render.enhancement.smooth_motion`,
PLAYER-CERTIFIED the same evening — "the feel is a million times
better, and the delay is practically irrelevant"; the one-tick
latency question is closed by play. Implementation
exactly as designed below: hash-silent per-slot generation counter
(`SlotGens`, bumped in `new_event`; `LivePose` carries slot +
generation), `entities::lerp_poses` (torus + yaw short-arc, 8-tile
teleport snap ceiling, unpaired poses draw at cur),
`apply_smooth_motion` re-sets billboards/health-bars/lights per
frame at the camera's own accumulator alpha; map layers stay
per-tick. P-class default-ON per the classification directive;
goldens untouched — the counter hashes to nothing unconditionally):
- Problem: sim entities move in whole per-tick strides while the
  renderer runs at frame rate — from the side, a rapid-fireball
  stream reads as motionless fireballs hanging in a row; "animation"
  currently exists only by retinal blur.
- Solution: interpolate entity positions between the LAST TWO tick
  snapshots (render at tick N-ε with the app's existing fixed-step
  accumulator alpha) — the same scheme the carpet has used since
  phase 4. NOT forward prediction: precomputing next positions would
  either burn per-entity LCG draws early (movement handlers consume
  RNG — determinism poison) or demand dry-run copies of every
  handler (divergence factory). Past-interp needs ZERO sim changes
  and, drawn correctly, an impacting projectile glides exactly into
  its real impact point (no overshoot case exists).
- Mechanics: stable pose identity across snapshots (pool slots are
  LIFO-reused same-tick — needs a runtime-only per-slot generation
  counter or slot+class+model match), torus-aware shortest-path
  lerp, snap on spawn/despawn/teleport (distance ceiling a few
  tiles). Sprite FRAMES and the 8-rotation snap stay discrete —
  the stepping look of objects is authentic; only world-space
  motion smooths. Entities render ≤1 tick (33ms) behind the
  interpolated camera — imperceptible, and input latency untouched.
- CLASSIFICATION (player directive, 2026-07-06, emphatic): the
  original's tick↔frame LOCK is a defect, not a virtue — "the
  original gets unplayable when the hardware is too slow or fast,
  both for a different reason... one of those things where we
  absolutely do NOT want to be faithful, because it sucked and
  played no role in the game's success." The fixed-timestep
  decoupling is the project's founding deviation; entity interp
  completes it. Default ON (P-class, replay-neutral); a discrete
  "render-locked feel" toggle may exist for curiosity, not as the
  default.

Note on `smooth_shading` (player, 2026-07-05): possibly redundant now
— the tile-edge artifacts that motivated it were mostly the UV
ROTATION bug (fixed 2026-07-04; the original textures are designed
edge-seamless). Clarification: the toggle interpolates the SHADE/light
level across tile centers, not texture seams — logically distinct,
but whether the per-tile shade snap is still visible enough to matter
post-textures awaits the player's eyeball. Keep as the exemplar
toggle either way.

## Phase 5 — flight fidelity (FAITHFUL MC1 FLIGHT LANDED 2026-07-07)

### The Phase-5 port (LANDED 2026-07-07; playtest owed)

`mgc_sim::flight` = the verbatim human-carpet port (sub_455D0 :55110
+ sub_46840 :55760 command integration + the sub_45410 z-floor), in
integer engine units on the existing SIN/COS 16.16 tables; selected
once at the `Simulation::step` boundary by the two G-class enums
(`ThrustModel::{Mc1,Enhanced}`, `AltitudeModel::{Faithful,
ExtendedLift}`), config `flight.{thrust,altitude,bindings,
mouse_sensitivity}` + CLI `--thrust/--altitude/--bindings`; faithful
values default everywhere; classic bindings = mouse + Up/Down
accel/decel + Left/Right strafe. 127 workspace tests green incl. the
WALL-CLIMB ACCEPTANCE TEST (ride a 25-tile cliff on the z-floor →
dash away level, altitude holds → counter-impulse standstill →
8/tick sink) and 9 flight unit tests. Delete
`mgcarpet.json.defaults` to regenerate with the new options.

VERBATIM TRACE FACTS (full agent report distilled; corrections to
the older banked summary marked ⚠):
- Input: mouse = ABSOLUTE cursor offset from screen center, ±127
  both axes (:19904-16); no keyboard aim exists. ⚠ ROLL is a RATE
  (yaw += filtered/8 per tick, :55146; max ≈31/2048/tick, full
  circle ≈65 ticks) while PITCH is ABSOLUTE (the filtered value IS
  the published 11-bit aim, ±254 ≈ ±44.6°, :55158-60). Both run the
  same low-pass `s += (2·input − s)/4` (:49017-20 + :55143-44),
  converging on 2× the raw input; the truncating decay PARKS at
  |s| ≤ 3 — below the /8 turn threshold, so turning stops dead.
  ⚠ remc1 declares the filter fields u16 — transcription bug (the
  sign-division idioms prove movsx); ported signed i16.
  Mouse-forward = positive pitch = DIVE (stick convention); Y-flip
  is a future bindings-tier option. Modern mapping: relative motion
  integrates a VIRTUAL STICK (STICK_PER_PIXEL 0.4 × sensitivity;
  original ≈0.8/px on the 640-space cursor).
- Speed: target (Type_160 v_12) steps ±16/TICK HELD, clamp ±80,
  HOLDS on release (no stop key — the authentic quantum-hunting
  standstill); actual (+126) chases in pure ±16 sign steps; base
  +128 re-pinned to 80 every tick (:55343). Strafe: own ±80 speed
  at yaw+512, ±16/tick held, −4/tick decay released with sign-flip
  snap (:55782-821).
- ⚠ The move is a TRUE POLAR ROTATION (sub_41EC0 :52523):
  horizontal = s·cos(eff_pitch), z −= s·sin(eff_pitch) — the
  player's "always level plane" holds within cos(44.6°) ≈ 29% max
  loss. DIVES pass the raw aim; CLIMB is scaled by authority
  (−v5)/256 where v5 = clamp(z − ground − 1024, ±256): full below
  ground+768, zero at the ground+1024 soft ceiling, INVERTED above
  (saturating −1× at ground+1280) — lasting altitude comes only
  from terrain rising underneath. Level flight (pitch 0) holds ANY
  altitude; speed-0 sink 8/tick only above the soft ceiling; hover
  below it is EXACT. ⚠ eff_pitch is PERSISTENT state — the
  s==0/v6==0 branch leaves it stale and the step consumes it
  (:55163-95); ported as state.
- Commit: wall gate (already ported) then the UNCONDITIONAL z-floor
  ground+128 at the final candidate (:55103-05); a fully blocked
  move commits NOTHING (even the sink is lost). No hard ceiling
  anywhere. Knock (v_22/v_24) applies after forward+strafe, before
  the gate.
- Accelerate spell (:65131-200): writes BOTH v_12 and actSpeed
  (3×80 held / 2×80 released) — the chase is fully bypassed; ⚠ on
  expiry it resets v_12 = act = +80 MAX FORWARD even out of
  backward flight (ported via edge detection at the sim boundary);
  ⚠ ANY Up/Down press cancels (the v_14 speed-touched flag) — the
  mc1 tier cancels on any thrust key; enhanced keeps the
  playtest-settled resisting-input-only cancel.
- ⚠ Camera renders HALF the aim pitch (pitch_8 = u16_329/2,
  :52434) — app halves under mc1; casts aim along the FULL
  published +32. The camera also ROLLS with the raw stick
  (roll_10 = u16_327, :52432) — NOT rendered yet (CameraView has
  no roll) = banked presentation item ("view banks into turns").
- ⚠ Map/book "fixes orientation, not velocity" is EMERGENT: map
  modes write no input → filter targets 0 → ×0.75/tick decay to
  center over ~8-12 ticks while v_12 persists (:49044, :20635-744).
  Ported the same way: the book zeroes the tick's inputs, filters
  decay in-sim; the virtual stick recenters on toggle.
- The every-64th-tick FLUTTER roll (entity-private LCG 9377/9439,
  r%11==0 → sound 46, :55294-99) — the move's ONE RNG draw — now
  wired (`Mc1Moved::flutter` → player-anchored sound 46).
- Behavior row 7 recorded in flight.rs docs: v_10=1024 soft
  ceiling, v_12=128 floor, v_20=0xFFFFFEFF (wall bit cleared).
- remc1 maintainer-edit flags from this trace: FIX_MOUSE cursor pin
  (:19896), MODIFY_SETTINGS key injector (:19933), dead
  `locret_455C0` indirect call in the accel handler (:65166 —
  original likely called something real there), `//fix` at :65116,
  plus the u16 signedness bug above.

ENHANCED-TIER CHANGES (same session, player directives): thrust +
the Accelerate override now act in the yaw GROUND PLANE (aim pitch
never bleeds mobility — the view-ray thrust bug is dead); explicit
lift exists only under `extended-lift`, capped at the LIVE highest
terrain tile + the 4-tile band (rising-only — the cap never yanks
down altitude already held; wall-climb gains are legitimate); the
old hard 40-tile ceiling is gone; enhanced inherits the faithful
speed-0 settle (8 units/tick above ground+4) so altitude is never a
one-way trap without lift keys.

OPEN (Phase-5 leftovers, priority order):
- PLAYTEST the faithful model (the player is the oracle): stick
  feel/sensitivity default, turn rate, the quantum-hunt standstill,
  the wall-climb move on a real level, camera half-pitch feel,
  mouse-forward=dive polarity (Y-flip binding if it grates),
  book-screen decay, accel-spell cancel semantics under mc1.
- Camera ROLL (view banks with the stick) — needs a roll term in
  CameraView; small renderer change, big feel item.
- `mc2` thrust tier: normalize key (backspace?) zeroing velocity +
  heading — trace from remc2 config/code when wanted.
- TORSO-AIM enhanced aiming (design banked above) — the intended
  eventual enhanced default; carpet-edge visual with it.
- Spawn-grace/mortality fields (u8_326 = 100-tick grace, regen
  divisors, death state 2 fall) traced in the same report — feed
  the mortality track.

### Flight playtest + spot-fix round (2026-07-07, same day)

- **CONTROLS PLAYER-CERTIFIED**: "as frustrating and useless as I
  remember them to be" — the faithful MC1 model passes its oracle.
- **DRAWABLE-GAP FIX (systemic)**: `world::drawable()` predated the
  spell-fidelity sessions — five sprite-carrying effects existed in
  the sim but never drew: (10,6) standing fire [the missing
  burning-tree flame the player reported — and retroactively the real
  cause of playtest-3's "wall of fire didn't even show"], (10,16)
  lava bombs, (10,19) volcano plume, (10,38) storm cloud, (10,43)
  castle upgrade token. All added. (10,53) napalm cloud stays
  invisible correctly — it's a driver; its (10,6) sheets are the
  visible part. Tree-burn regression test now asserts the flame
  draws at trunk height. RE-VERIFY IN PLAY: burning trees, wall of
  fire, volcano bombs+plume, storm cloud, the upgrade token ball.
- **SETTLER MAP DOTS = PURPLE** (player retail ground truth, senior
  over the decompile's villager color LUT[16] — which decodes dark
  green; LUT index 0x101, one transcription digit away, is exactly a
  dark purple and the likely original). Both dot paths updated;
  shade approximate (vga 7,3,7) — calibrate against retail if it
  reads wrong.
- **NEW OPTIONS**: `enhancements.map_owned_buildings` (MC2's
  claimed-dwelling map highlight brought to MC1, P-class, default
  off — player proposal); `flight.invert_y` (P-class binding,
  default false = the authentic mouse-forward-dives polarity; the
  originals shipped the same option).
- **MATRIX CLARIFICATION (player directive)**: options are
  MULTI-COLUMN, not binary — `MC1 | MC2 | improved` (or more) per
  subsystem; MC2 behaviors are legitimate faithful alternates for
  MC1 play, not just enhancements. The flight tier enums already
  follow this shape; future options should too.

### Map-fidelity pass (2026-07-07, from the player's retail
### screenshot + spot checks; LANDED, PLAYER-CERTIFIED 2026-07-09)

The full sub_48710_48A50 marker pass, replacing the minimal dot
switch (player report: no castle/balloon/projectile markers, owned
buildings missing, low refresh):
- **Refresh fixed**: the map texture recomposed every 8th tick (the
  reported low rate) — now a dedicated `Renderer::update_map`
  recomposes EVERY frame (the original redraws per frame; the blink
  and marching-ants phases live in the pixels). `update_terrain`
  (plane re-uploads) stays terrain-dirty-only.
- **Verbatim dot switch** (:57184-:57292, unit-tested): player-owned
  class-9/10 things = team violet `byte_99B58[0]=0xB7`; wild = the
  RGB-LUT[3856] blue-violet — the LUT decodes BLUE-major, settled by
  the retail screenshot's blue-violet village speckles, which are
  HOUSE dots: **the original does dot houses** (m45 falls through
  the owner-color rule; the earlier "settlers are purple" report =
  these). Villagers = LUT[16] dark green (reverted from purple);
  wild creatures LUT[1] near-black; owned creatures 0x71 (odd team
  entry, :57252); wild mana balls = raw 232; claimed balls BLINK
  0xB7↔0x71 on a global phase (~4 Hz); portals = the 2x2 grown dot
  (v60=2); charred trees vanish (v29 stays 0); class-2 m1/m3 =
  near-black. `map_owned_buildings` (enhancement) now = a 2x2 grown
  dot for owned dwellings (the MC2-style legibility bump over the
  original's barely-distinct 1px).
- **Icon stamps**: own castle = UI sprite 58+team, own balloons =
  66+team (:57230/:57234), cropped from the composited HSPR atlas
  (`UiAssets::map_stamp`) and blitted onto the map (`MapStamp`).
  ICON IDS 58/66 ARE TRACE-ONLY — verify in the first playtest
  (wrong sprite = one-constant fix). Rival markers need the reveal
  flag (v59) — no rivals exist yet.
- **The guide path**: player → own castle, a mark every 4 units
  from a phase offset cycling 0..3 per frame (:57161-82) — the
  marching-ants crawl; plotted as a brighten over the terrain (the
  original goes through the blend LUT). Target = the (3,2) pose;
  drawn only while a castle stands. Endpoint VERIFIED in retail
  play (player, 2026-07-09): points at the castle as ported —
  no re-trace needed.
- LivePose gained `player_owned` (owner +24 / claim +144 vs
  PLAYER_TARGET) — the team-color rule's input.
- NOT ported yet: the advertised-trigger X markers as map stamps
  (icons 83/84 — infrastructure now exists, needs class-11 poses
  exposed), rival-wizard reveal, the class-2 m1/m3 settings gate
  (str_93.var_u8[2] — always-on for us), invisible drivers' dots
  (quake walker etc. — original dots them, ours only dots drawn
  poses), jar dot color re-check under the BLUE-major LUT reading
  (ours keeps red pending retail comparison).
- FIDELITY NOTE for rival reveal (found in the 3-commit review vs
  sub_48710 :57238-56): the class-5 building dot in map_dots_from_poses
  gates the team-color branch on `!player_owned`, but the decompile's
  actual test is `owner == self` (unowned building). These differ ONLY
  for a RIVAL-owned building — retail gives it that rival's team color
  `byte_99B58[1+2*owner]`, ours falls through to villager-green. Inert
  today (single-wizard, no rivals), but when rival reveal lands this
  gate must become a proper owner-vs-self check keyed on the actual
  owner id, not the binary player_owned flag.

## MORTALITY + THE CASTLE WEAPON — LANDED 2026-07-07
## (PLAYER-CERTIFIED 2026-07-09: "completely faithful")

The whole six-item agenda below shipped in one session (mgc_sim
world/features/combat/flight boundary + app wiring). Trace sources:
three deep decompile passes banked in this section; every port cites
sub_main.cpp lines in code comments.

**TRACE CORRECTIONS to earlier session notes (important):**
- `u8_326` is NOT the spawn grace — it is the fireball CHARGE counter
  (increments toward 200 per alive tick, consumed at cast :65072).
  The spawn grace is **`Type_160.u16_331`**: while > 0 the whole
  6-channel mailbox is memset to zero each tick — total immunity,
  steal/grip included, danger music stays calm (:55367-71). Respawn
  arms 100; the at-castle redirect re-arms 2 (unconditional write —
  sitting home under fire authentically shortens a fresh grace).
- Health regen is **/250 home, /2000 afield** (the /200-vs-/2000 pair
  banked earlier is the MANA rates). Max life 10000 flat; skill does
  NOT scale it (the +26 respawn-wait formula paces AI only — the
  HUMAN respawns on Space immediately, the timer is computed and
  never enforced, :55627/:20081/:48620).
- Castle HP ladder levels 6/7 = **80000** (decompiler-mangled
  `loc_13880` = 0x13880; our earlier 60000 carry corrected). Ladder:
  20000/40000/40000/60000/60000/80000/80000 for levels 1-7. No castle
  HP regen exists — upgrading is the only heal; on any level change
  a NEGATIVE life (overkill) re-deducts from the new max capped at
  half (sub_47BD0).

**What landed (sim):**
1. PLAYER MORTALITY (world.rs `LifeState` + `apply_player_damage`,
   sub_46540 verbatim): ch0 shield quartering (amount/4, the quarter
   also paid from mana), knockback v_24/v_22 = source→victim bearing
   + amount/10 clamp 80 through the already-ported knock machinery,
   hit sound 17 / red flash / 16-tick regen stall on every processed
   hit; ch3 mana steal; ch4 grip side-effects (the tether itself =
   the duel track). At the own castle, pending ch0 FORWARDS into the
   castle's mailbox — the castle tanks for you (:55353-62). Death:
   state-2 fall (gravity −2/tick² clamp −256, sub_455D0 still
   drifting, (10,1) fire trail, sound 16), touchdown at ground+128 →
   the 24-slot jar scatter (models remembered, manifestation
   entities become DECAYING world jars, 200-289 ticks, tick70 = 3)
   + the (10,40) GRAVE inheriting every player-owned loose ball
   (possess the grave to reclaim the bank — sub_275C0 ported), then
   the grey dead-wait: death camera turns toward the killer, Space
   respawns at the castle with life/mana full + grace 100 + spells
   re-instantiated; castle-less = lost + LEVEL RESTART (app rebuilds
   the pristine world). The old invincibility survives as the
   `invincible` config/CLI toggle (G-class dev instrument, default
   OFF — the player is mortal now).
2. CASTLE HP/DAMAGE/DOWNGRADE (castle_tick + castle_downgrade,
   sub_47EC0/sub_47A70/sub_470E0): ch0 castle pre-pass added to the
   area writers (castles of OTHER teams take area mail — mob-death
   fire cells at 400/hit are how flocks fell castles; ~50 net hits
   kill a level-1), the 127E0 blast variant also arms the 30-tick
   shake → damage REPAINT (painter without the kill bit — kills
   nothing, verbatim). Lethal total = ONE LEVEL DOWN per event:
   sound 30, 10% capacity haircut + the OVERFLOW EJECTOR (sub_47130
   — also now running every other established tick, closing that
   deferred item: spill = stored − capacity when houses+stored
   exceed it, thrown as 1-32 owner-tagged balls 15-35 tiles out with
   the (1024−flag_height)/8 pop, plus 4 (10,54) mana MAGNETS — their
   ch4 ball-writes land but the pull is inert until the banked
   magnet chain is traced), footprint un-stamp through the collapse
   walker's zeroed fake event (synchronous, as retail), ladder reset
   with the overkill carry, repaint after 5 ticks. Level 1 lethal =
   TOTAL DESTRUCTION: balloons released, the whole bank scatters
   wild, entity freed, player castle-less. "Castle under attack" UI
   flash (+391) exposed to the HUD.
3. THE CASTLE WEAPON (tick_castle_painter → build_footprint_kill,
   sub_40E20): every paint tick of the BUILD/UPGRADE painter (the
   +18&1 kill bit — set only by the upgrade commit :56492) executes
   the footprint: class-2 scenery deleted outright, class-5
   creatures life = −1 at any HP with kill credit + corpse drops to
   the castle owner, EXCEPT models 6/8/16 (boss exemptions) and
   anything owner-owned (broader than caster immunity — your
   skeletons survive). Wizards/balloons/castles/projectiles immune
   structurally. Player-side caster immunity for terrain spells was
   already write-side (owner id on every effect) — verified, no
   intake-side gate exists in retail.
4. DEMOLISH: Shift+L (app) → PlayerCommand.demolish → own castle
   life = −1 (:55846-50), one downgrade level per press through the
   exact damage path (sub_47EC0's first line catches the mail-less
   kill — that check is load-bearing for demolish). BONUS TRACE:
   Shift+K is authentic wizard SUICIDE (unported; parked).
5. TWO-CAST RULE — NARROWED (player, 2026-07-09), mechanism still
   open, RETAIL TEST PENDING (player will run a specific case):
   the observed two-step is strictly tied to INTERFERING HUMAN
   DWELLINGS — appears at any castle level, not just level 0;
   castles on clear ground are single-cast. Two candidate
   mechanisms, deliberately NOT adjudicated yet: (a) an actual
   raze-then-build gate — first cast clears the dwellings, second
   builds (would make the decompile's `!+26` short-circuit at
   :56055, which skips the space gate at level 0, a transcription
   error); or (b) EMERGENT — the build lands in one cast, but the
   damaged dwelling's explosion feeds back and downgrades the fresh
   castle one level, reading as "cast twice" in play (would leave
   the decompile correct and our port already faithful). Don't get
   hung up on interpretation (a); wait for the retail case before
   touching the port. No mana spills from the pre-clear itself:
   collapsing houses evacuate up to 4 villagers — who are NOT
   exempt from the castle kill, and the corpse drops are the
   "spilled mana" of player lore.

**App:** E/Q = extended-lift float (player directive — Space freed
for respawn/level-continue, Shift freed for chords), Space = respawn
confirm, Shift+L = demolish, level-restart rebuild on castle-less
death (pristine WorldInit stored at load), functional-first vitals
HUD (life bar + grace shimmer + castle-alert strip + red hit flash +
fall red-out + dead grey-out with blinking Space prompt — palette
rows 2/7 in retail; the faithful presentation is the UI/UX track),
`invincible` in config/CLI, help text updated.

**Tests:** 4 new mortality/castle units (grace→damage→knock→death→
grave→restart; castle respawn + re-grant; footprint execution with
owner/boss exemptions + kill credit; downgrade eject/overkill-carry
+ demolish razing) — 64 lib + all integration green; the wall-climb
acceptance test untouched. Combat tests pin `invincible` (they were
written against the dev player).

**Known deviations / owed:**
- Jar-scatter LCG runs on the world stream (the original rolls the
  dying wizard's private Type_160 stream — not modeled outside
  flight); same constants and draw count.
- Grave-spawn failure on a full pool proceeds graveless (original
  retries the landing next tick with slots already converted — a
  retail quirk not worth the state).
- Mobs keep mauling the landed corpse (original hides the wizard,
  flag 0x20 → deaggro); cosmetic while mail is discarded.
- Dead player's mana keeps regenerating (refilled at respawn anyway).
- (10,54) magnet ball-pull inert pending the (9,17)→(10,54) chain
  trace; unify with the spell-side state-21 APPROX puller then.
- PLAYTEST CLOSED (player, 2026-07-09): mortality + castle feel
  "completely faithful", no complaints after the fix rounds.

### PLAYTEST-6 FIX ROUND (same day, mortality track feedback)

1. **Orphaned-tower / castle-flow interruptions — ROOT-CAUSED, a
   fidelity bug of ours**: the original processes castle damage ONLY
   from the standing state (sub_47EC0 runs from +70=4 alone; during
   any painter/leveler transformation the mailbox and pending
   lethals just ACCRUE). Our f59=4 conflated "waiting for the
   painter" with "established", so lethals (incl. Shift+L) processed
   mid-transformation — the downgrade collapse ran under a live
   painter, which then repainted castle terrain with no castle
   behind it (the "central tower left behind"). Fixed: waits moved
   to the original's pure-wait sub-state 1; damage/demolish/upgrade/
   ejector/balloons all gate on established. This also RESTORES the
   authentic between-transformations window (the dragon-squat
   upgrade-instead-of-death trick the player described) — deferred
   mail processes the instant the castle re-establishes, and a
   pending upgrade can land first. Regression test:
   `demolish_during_the_build_defers_until_established`. REMAINING
   DISCUSSION (player: "the original was buggy — perhaps debug the
   flow independently"): retail's own mid-air quirks (the
   "indefinite transformation midpoint" soft-lock) may live in state
   interleavings we now structurally avoid; if retail-faithful
   glitch reproduction is ever wanted for these, it needs a
   dedicated trace of the +48/+50/+70 interleaving. Current stance:
   keep the robust shape (it matches the decompile's state gating).
2. **Castle self-destruct depth**: ours = one level per Shift+L
   press (the decompile shape: life=−1 → sub_47A70 = one level).
   PLAYER BELIEF: retail razes the whole castle end-to-end — RETAIL
   CHECK OWED; if confirmed, make demolish loop the ladder (trivial).
3. **Castle ball aim FIXED (playtest-6 "ignores up/down")**: the
   launch now inherits the wizard's aim PITCH too (:65913-14 copies
   +30/+32) and the flight is the original's EASED steer (sub_53B50
   via sub_422A0, behavior-row-0 caps yaw 56 / pitch 22 per tick)
   instead of our snap-steer — the aim shapes the early arc. The
   ease exposed a missing landing rule, also ported: the with-castle
   (upgrade) ball lands on OVERLAP with the linked castle and snaps
   onto it (:63484-88 — the 0x4000 z-extent makes overflight count).
   Spawn z = carpet + the wizard half-extent (+84), as retail; the
   "originates above the head" impression is partly the half-pitch
   camera (authentic). Watch in play whether other non-homing
   spells need the same pitch-inheritance sweep.
4. **Worm-vs-castle damage VERIFIED WORKING in-sim** (the playtest
   doubt was a visibility problem): building a level-1 castle under
   the 17-section worm kills the chain and the corpse fire cells
   deal ~9,600 into the castle (20,000 max) — it survives ONE worm
   at the decompile's constants (fire cell = ONE 400 ch0 write per
   cell lifetime, spreader ≈50% spawn odds per 2x2 cell; both
   verbatim). Player memory says retail "usually destroys" it —
   RETAIL CHECK OWED on magnitude; if retail is deadlier the suspect
   is fire-cell count per corpse burst, not the per-hit damage.
   Meanwhile castle HP is now VISIBLE: a thin green/red strip above
   the castle panel (functional-first; retail shows no HP meter).
5. **Extended-lift auto-decline** (player directive): with the
   hover keys idle the carpet now settles 8/tick toward the
   terrain-follow floor at any speed — ground-contact pickups (spell
   jars) work again under the alternate scheme. Both movers.
6. **Villager map dots**: retail's LUT[16] decodes to (r0,g1,b0) — a
   green so dark our nearest-palette match landed on BLACK (the
   report). Retargeted to a legible mid-green per the map-legibility
   ruling. VERIFY the shade in play; if retail's actual pixel is
   known from a screenshot, match it exactly.

### PLAYTEST-7 FIX: the never-landing corpse (same day)

The death fall deadlocked mid-air under the ENHANCED thrust model:
the fall integration clamped against ground at the INTEGER CARPET's
x/y — which the enhanced mover never updates after spawn (only the
mc1 mover keeps it live). Die anywhere the local ground sits lower
than the level-start spawn's and the corpse floored on a phantom
altitude above the real terrain; the ground+128 landing check never
fired, so the whole landing chain (scatter/grave/dead-wait) never
ran and the fall effects looped. Fixed: the fall integrates in FLYER
space at the FLYER's position (exact-roundtrip ground+0.5 tiles =
the engine's ground+128), mirrored into the carpet for the mc1
model; the enhanced mover's living hover clearance (0.75 tiles)
also now drops to the 0.5-tile touchdown floor while falling — it
sat ABOVE the landing threshold and would have held the corpse off
the ground even at the right position. Boundary regressions:
`death_fall_lands_under_mc1` +
`death_fall_lands_under_enhanced_far_from_spawn` (the exact
deadlock shape); the world-only mortality tests drove poses by hand
and could never catch this class — the sim-boundary harness
(`flat_world` in lib.rs tests) is the pattern for future mortality
flows.

## NEXT TRACK (agreed 2026-07-07): MORTALITY + THE CASTLE WEAPON

The player's pick for the next 1-3 sessions, with a body of new
ground truth (player briefing 2026-07-07 — most of it trace-
verifiable; trace before porting):

1. **Player mortality**: real health/damage intake (drop the
   permanent spawn grace — the flight trace already pinned the
   fields: grace u8_326 = 100 ticks, regen divisors /200 at castle
   vs /2000 afield, death = state 2 fall −2/tick², land ground+128,
   respawn wait 32·((255−skill)>>3)+32), death → CASTLE RESPAWN
   (the original's only checkpoint; no castle = level restart),
   death drops (24-ball inventory scatter + the m40 grave inheriting
   owned balls, :55519-65), hit/death sounds 17/16, hit knockback
   through the already-ported v_22/v_24 machinery (grace currently
   discards it). Unblocks Heal/Shield/Rebound/Invisible validation
   and the danger-music arm loses its dev shim.
2. **Castle HP / damage / downgrade** (sub_47A70, called from the
   castle tick :56145; health ladder already ported): mob-death
   explosions damage the castle — enough kills on the footprint can
   knock it DOWN A LEVEL (player ground truth; the downgrade is the
   balance for item 3).
3. **TERRAIN-TRANSFORMATION DAMAGE — the castle IS a weapon**
   (player briefing, the load-bearing part): the castle build/
   upgrade transformation INSTANTLY KILLS weaker monsters on the
   footprint — "one of the most effective mass-destruction weapons
   in the game". Canonical strategy: fly into a vulture flock
   (near-certain death with fireball alone), build a castle under
   your feet — kills a swath, grants safety + regeneration,
   possess the spilled mana, rebuild bigger. A few casts clear a
   whole cloud + a starting boost. REPLAY-FIXTURE CANDIDATE once
   replays exist. Interlocks: (a) mob explosions may downgrade the
   fresh castle (item 2); (b) WIZARDS take NO castle-transformation
   damage, only mobs (player memory — verify in trace); (c) our
   load-time feature pass deliberately omitted damage broadcasts —
   the runtime build path must NOT (the m41/m42 chain's ch0
   writes, sub_127E0 family).
4. **Caster immunity rule** (crater/quake/volcano already ported
   owner-immune for mobs): enemy-cast terrain spells DO damage the
   player; your OWN never do — incl. (player belief) your own
   volcano's post-effects. Port the player-side intake with the
   same owner gate.
5. **The demolish key** — MC1 Shift+L (player memory; MC2 same or
   similar): razes/levels your own castle, enabling indefinite
   castle-as-attack-spell use at the cost of your respawn point
   (die castle-less = restart level). Trace the key handler +
   demolish path (the collapse walker's "castle demolish zeroed
   fake event" is already ported — this is its trigger).
6. **Castle vs dwellings — the two-cast rule** (player ground
   truth): towers that would pop inside existing dwellings make the
   FIRST cast destroy those dwellings WITHOUT building; the SECOND
   cast builds. Our ported upgrade arm (sub_12C50 house pre-clear +
   sub_12D10 space gate with silent bounce) matches the shape —
   verify the same two-step applies to the INITIAL build, and that
   the pre-clear kill is what feeds item 3's mana spill.

## FAITHFUL UI/UX — SUBSTANTIALLY LANDED 2026-07-07 (session 2, playtest owed)

Picked up the banked UI/UX track (below). Two cores landed, both
player-verified against matched-resolution DOSBox side-by-sides:

**Core A — rotated player-centered maps** (mgc-render map.wgsl +
`Renderer::minimap_rect`/`map_stamp_quads`; port of remc1
`DrawMinimap_49300` :57491). The map was a static axis-aligned grid;
now BOTH surfaces are player-centered, yaw-rotated, toroidally
wrapping (shader-side affine sample; entity dots/path bake into the
world texture and rotate for free):
- In-flight **round radar** (new) — corner-anchored (0,0), edge-
  touching, disc = full 128 native px, `+`/`-` runtime zoom (MC2/MC1
  feature). Faithful 128-tile span.
- **Book map** — full 256-tile world (our no-clip improvement; the
  original's ~251-tile framing clips edges — a rotation+no-AA
  sub-sampling artifact we're free to beat).
- **Upright icon stamps** — castle/balloon must stay screen-upright
  under rotation, so they're NOT baked into the rotated texture:
  `map_stamp_quads` projects world→screen (inverse rotation) and
  blits upright from the UI atlas. Per-range anchor (sub_48710
  :57344-64): castle 58-65 = bottom-LEFT, balloon 66-73 =
  bottom-CENTER. `MapStamp{uv, anchor}`.

**Core B — the HUD top strip** (mgc-app ui.rs `hud_quads` + new
`UiAssets::{sprite_quad,sprite_quad_tint,sprite_dims}`; port of
sub_22E50 wizard strip + sub_23D40 equipped spells). Replaced the
functional-first bottom bars with the real begSprTab sprites at the
decompiled coordinates:
- **Geometry 1:1** (player pixel-measured): six tiles packed from
  x=2 with 0px gaps — native origins 2,126,254,382,510,574 = [40]
  radar-frame (124) | 3× [41] wizard sub-panels (128) | 2× spell
  frames [1]/[2] (64). `HUD_SECTION=128`.
- **Wizard panel** (slot A = own castle, the FIRST sub-panel per
  `var_50` :27214): bg [41]/alert [55]/empty [54], castle-level
  glyph [43+lvl] (emblem+heart+orb+"1" all baked in), divider [42],
  life bar (red 0x7B), capacity bar (amber) + collected-mana bar
  (WHITE, v29). All content GATED on castle level>0 (else bare
  marble [54]). Ally slots B/C empty until multi-wizard.
- **Equipped-spell panels** (x=510/574): frame [1] idle / [2]
  cast-in-progress (on the +48 burst counter, NOT "equipped"), icon
  [spell+6] at NATIVE 62×34 (not stretched), availability meter at
  +36 = GREY progress bar (partial mana toward next cast, v26) +
  WHITE single-pixel dots (whole casts affordable, sub_61594).
- **Panel brightness**: base UI atlas now composites RAW palette
  (was blend-over-black, darkening panels ~30%). Icons/glyphs draw
  raw; only the slot-tile luminous ramps keep the blend.
- **HUD transparency = a toggle** (`enhancements.hud_transparency:
  mc1 | opaque`): MC1 always-on transparency (the whole HUD —
  panels AND radar — blends over the sky via shared
  `HUD_PANEL_ALPHA`); MC2/opaque = solid, for readability (the
  radar especially — player: transparency "makes the map less
  useful"). Multi-column matrix option; `MGC_HUD_OPAQUE=1` headless
  override.

### Wizard strip finalized — three sub-panels (2026-07-07, session 3)

The three sub-panels of sub_22E50 are NOW correctly semantic (player
ground truth reconciled with the :27214/:27334/:27374 trace — the
earlier "castle | ally | ally" reading was WRONG):
- **Slot A (v22, `var_50`) = the LINKED CASTLE**: castle-level glyph
  [43+lvl], divider [42], life bar = the **CASTLE's HP**
  (`castle_hp`, not player life — the earlier code drew player life
  here, a suspected-and-CONFIRMED bug the player flagged), mana
  capacity+banked bars, win tick. Gated on castle && level>0 (else
  bare marble [54]).
- **Slot B (v23, `var_52[]`) = the player's MANA BALLOONS**: the
  roster is 1/2/3 balloons wide by castle level (1-3 / 4-5 / 6-7,
  the :27296-314 switch); glyph [50+count], divider [42], then per
  balloon a THIN 2px HP bar (red, stacked y=12+2i) + cargo bar
  (white, y=30+2i). Empty [54] with no castle. Sim exposes it via
  `World::player_balloons()` → `LoadoutView.balloons:
  Vec<(hp_frac, cargo_frac)>` reading player-owned class-3/model-3
  entities capped at the roster size. (This resolves the "balloon
  status" ask — the strips ARE the balloons, per player.)
- **Slot C (v24, `a1x` = self) = the player's OWN wizard**: base
  glyph [43], divider [42], **player** life bar (this is where
  player health belongs, :27375), self mana capacity (mana_max,
  amber) + current (mana, white) over the world total, win tick.
  Drawn UNCONDITIONALLY (the wizard is always present).

Unit test `loadout_surfaces_the_balloon_roster_by_castle_level`
(roster size by level + per-balloon fractions + collapse clears it).
Balloon glyph sprites [50-53] confirmed present in the UI atlas
(34×44). ALL trace-cited coords; the balloon glyph art [50+count] is
TRACE-ONLY — eyeball in the first playtest. Build+tests green.

### Book/map screen topology — faithful layout (2026-07-07, session 3)

The Enter book/map screen rebuilt to the decompile's actual topology
(sub_20E60 case 4 :26776 + the spellbook grid :26915-72), replacing
the invented 0.6/0.42 split-pane. KEY FINDING — the original book
screen is NOT a clean split; it's the **live world as background**
with three overlays, all at native 640×480 (scaled by w/640, h/480):
- **Map pane** = `DrawMinimap(0,0, 382,378, ...)` — a 382×378
  rotated player-centered map at the TOP-LEFT corner (renderer
  `BOOK_MAP_*` consts; map-globals + stamp projection share the same
  pixel rect; sampler aspect = 382/378, so `map_stamp_quads` gained
  an `aspect` param that stretches the longer axis to match the
  shader's `mode.y`).
- **Spellbook grid** = 24 spells in DISPLAY_ORDER, TIGHTLY packed
  from (384,162), cell = the slot-slab [3] = 64×37, `locMouseX +=
  64` wrapping at 640 back to 384 (`locMouseY += 37`) → 4 cols × 6
  rows filling bottom-right (`ui.rs book_cell`, native coords; two
  geometry unit tests). The earlier 4×6 grid with padding/gaps was
  makeshift — now gapless per player ground truth.
- **World viewport** = the top-right L-remainder (map's right edge
  → screen edge, y 0 → 162), scissored.
- **Bottom strip** (native y≥378, left of the spellbook) = the
  multiplayer MESSAGE/EVENT LOG (player: the "empty" strip; also
  where sub_22880's hover scoreboard draws) — STUBBED as a
  delimited dark panel; real content needs the DrawText/font path.

**Spellbook = a faithful SPELL PICKER** (player corrected my read):
left/right-click a spell binds it to that mouse button and exits to
flight — the only way to reach the 24 spells beyond the 10
quick-select keys. Click-to-equip was already wired (`self.hovered`
→ `pending_equip`); kept intact. The **3-state grid** landed here:
owned+castable = full icon, owned+unaffordable (cost > mana) =
DIMMED, not-owned = GHOST silhouette (tint stand-in for the
original's blend rows); only castable cells hover/bind (the
:26926 affordability gate). Build + workspace tests green, clippy
clean (the 2 combat.rs `&& false` errors are pre-existing).

**Playtest-fix round (same session, player screenshot):**
- **MAP PANE WAS BLANK — fixed**: the book layout's `sx/sy`
  resolution-scale locals were SHADOWED by the camera basis's
  `let (sy, cy) = cam.yaw.sin_cos()` a few lines below; at yaw 0,
  `sin=0` zeroed the map pane's NDC half-height → a degenerate
  (invisible) quad. Renamed to `res_x/res_y`. Map now renders full
  (terrain, coastlines, castle+balloon icons, player marker, entity
  dots) — player-screenshot-confirmed via headless `--map-view`.
  The radar was unaffected (its own globals block). LESSON: keep
  layout scale names distinct from the camera trig locals.
- **Equipped-hand highlight REMOVED** (player: retail CANNOT show
  which spells are on LMB/RMB — the highlighted slab was our
  invention and read oddly). `variant` pinned to 0 (plain slab). It
  returns LATER as a clearly-labeled UNFAITHFUL option with a better
  affordance for expressing the binding.
- **QUICKSELECT NUMBER BADGES added** (retail DOES stamp these,
  sub_24230 :27857): a spell bound to a number key shows its digit
  glyph `[30+slot]` (10×14; slot 0 = key "1" = [30] … slot 9 = key
  "0" = [39]) in the cell corner. `book_quads` now takes
  `&quick_binds` (already tracked app-side); the faithful expression
  of which spells are hotkeyed.

**Second playtest-fix round (same session):**
- **World viewport gap fixed**: its left edge moved from the map's
  right (382) to the SPELLBOOK's left (384), so the world column
  sits flush above the spellbook and the 2px map→spellbook seam
  stays black instead of leaking a world sliver ("live view a few
  pixels too large").
- **Bottom = pure BLACK** (player: retail's bottom below map +
  spellbook is simply black/empty, NOT a colored log panel): the
  book-mode clear is now (0,0,0) and the app's LOG_STRIP fill/rule
  quads are removed. The message log still lands there later via the
  DrawText path — with no panel tint.
- **Map extended flush with the spellbook** (player: "gap between map
  and the rest — should map be bigger?"): the decompile's 382×378 is
  the DrawMinimap SAMPLE size, not the on-screen pane.

**Proportions RE-CALIBRATED from the player's HI-RES retail
screenshot (senior over the decompile coords):** the earlier
162/384 guesses were wrong. MEASURED native 640×480 geometry:
- **map** (0,0) **384 × 416**
- **world viewport** x 384..640, y 0..**194**
- **spellbook** x 384..640, y **194..416**, 4 cols × 6 rows of 64×37
- **bottom bar** y 416..480 (~64px, BLACK)
The lo-res book has a smaller bottom bar than hi-res (player note);
we target hi-res. Consts: BOOK_MAP_H=416, BOOK_SPELL_Y=194 (renderer
+ ui `BOOK_GRID_Y`); grid tests updated (bottom = 416).

- **Spell-icon STRETCH fixed** (player: "icon smaller than the box,
  stretches downwards" — same bug the HUD equipped panels had): the
  book drew the pre-composited icon-on-slab TILE stretched into the
  cell, so at non-4:3 windows the icon distorted (and even at 4:3 the
  cell scale differed per axis). Now the slab [3] stretches to the
  cell (a texture — invisible) while the ICON draws SEPARATELY at its
  native 62×34, UNIFORM-scaled and centered (`sprite_quad_rect_tint`)
  — undistorted at any window aspect. Verified at both 4:3 and 16:9.
  The old `slot_quad`/`slot_uv` composite path is parked
  (`#[allow(dead_code)]`) for the future unfaithful binding indicator.
- **Icon STATES corrected** (took THREE tries, player-guided to the
  answer): OWNED spells (:26932 sub_24230) = raw full-COLOUR
  DrawBitmap. UNOWNED (:26972 sub_23CF0 → sub_23AE0) = the icon's
  outer SHAPE used as a coverage mask, inside which the STONE-SLAB
  TEXTURE shows through DARKENED — a dark relief cut into the tile.
  sub_23AE0 writes `blend[0xA6 | dest]`: the sprite bytes are COVERAGE
  ONLY (icon colours dropped) and the written colour darkens the DEST
  (the slab beneath), which is why the tile texture bleeds through the
  silhouette. (Wrong tries en route: colour-dimming, then flat
  silhouette, then greyscale — the player's own theory nailed it:
  "silhouette … outer shape of the sprite, but the texture of the tile
  exactly".) Impl: ui.wgsl mode 2 = MASK-DARKEN (fill the icon
  coverage with a dark TRANSLUCENT tint that alpha-blends over the
  already-drawn slab), signalled by a NEGATIVE uv width;
  `UiAssets::sprite_quad_rect_mask` + `UNOWNED_MASK`
  ([.05,.04,.03,.74], tuned darker per player). Data was correct all
  along (only spells 0/3 owned at L1); the made-up "unaffordable =
  dimmed" middle state is GONE.
- **Slab COLOUR warmed** (player side-by-side: retail slabs are warm
  dark BROWN, ours were cool blue-grey): our raw [3] sprite is
  ~(158,165,198) blue-grey; the original blends it through the LUT
  over the book bg, warming + darkening. A neutral darkening kept it
  blue, so `SLAB_DIM` warms to [.58,.46,.32] (boost red-relative, cut
  blue, darken). Verified side-by-side: brown slabs + dark-relief
  silhouettes + 2 colour icons = retail match.
- **2px "T" GAP between the panes** (player: retail has a black
  demarcation; ours was packed too tight — take it from the map +
  live view, NOT the spellbook which is 1:1): `BOOK_GAP=2`; the map's
  right edge recedes to 384−2 and the world viewport's bottom to
  194−2, so a 2px black gap forms the T; the spellbook origin stays
  fixed at (384,194).

**NEXT:** the DrawText/bitmap-font path (shared dependency: the
message-log/scoreboard text, PAUSED text, book hover stats). Then
the deferred cosmetic polish (dot aliasing via an AA/post pass,
exact transparency alpha, a thin black outline atop some panels),
and the equipped-hand-binding affordance as an unfaithful option.
Playtest owed: click-to-bind + quickselect digits end-to-end; the
wizard-strip castle-HP/balloons.
Full blow-by-blow (every correction + oracle citation) is in the
`hud-uiux-parity-track` memory.

## UI/UX FINAL-PASS — blend-LUT fidelity (banked 2026-07-07, session 3)

The HUD + book/map screens are player-verified against retail
side-by-sides and functionally faithful, BUT the presentation rests on
HAND-TUNED RGB constants where the original uses ONE mechanism: its
`blend[src | dest<<8]` palette LUT plus a few colour-index tables. The
honest completion of "presentation resolves late, faithfully" is to
bake and run that machinery in-shader and DELETE the magic numbers.
This is the long tail; none of it blocks play. Tiers, most-impactful
first:

**TIER 1 — replace hand-tuned constants with the real LUT/tables.**
Bake `blend` (the 2D `strPal.byte_BB934`), the `byte_99B58` team-colour
ramp, the `byte_AD167` text-colour table, and CLRD's 4096-entry minimap
LUT; resolve UI/map colours through them in-shader. Each eyeballed
constant then vanishes and becomes automatically correct across ALL
FOUR environment palettes (currently every value is temperate-only
eyeballed). The constant → oracle map (checklist for the pass):
  - `ui.rs SLAB_DIM [.58,.46,.32]` → the book slab = sub_23940 blends
    [3] through the LUT over the book bg (warm brown, not our raw
    blue-grey).
  - `ui.rs UNOWNED_MASK [.05,.04,.03,.74]` → `blend[0xA6 | dest]` — the
    mask-darken over the slab (sub_23AE0). ui.wgsl mode 2 approximates
    it with a translucent dark fill.
  - `mgc_render HUD_PANEL_ALPHA 0.62` + `ui.rs PANEL_TINT` → sub_23940
    panel backgrounds blend over the live framebuffer (transparency).
  - `ui.rs DIGIT_INK` (black) → quickselect digit = `byte_AD167[1]`.
  - `ui.rs LIFE_RED / CAP_AMBER / MANA_WHITE / METER_GREY` →
    `byte_99B58[2*owner (+1)]` team-ramp entries (0x7B etc.).
  - map dots (`map_pixels`): owned 0xB7 / wild LUT[3856] / villager
    LUT[16] / etc. → the CLRD minimap LUT, not our nearest-palette
    picks; re-check settler/jar dot colours under the BLUE-major read.

**TIER 2 — trace-only IDs, never pixel-diffed against retail.**
Map stamp icons 58 (castle) / 66 (balloon) + their anchors; wizard-
strip balloon glyph [50+count] and castle-level glyph [43+lvl] internal
art; the guide-path endpoint (Type_160+50). All decompile-cited, none
visually confirmed — a wrong id is a one-constant fix once seen.

**TIER 3 — deferred features with a known home.**
  - The DrawText/bitmap-font path — unblocks the book message-log
    strip, the sub_22880 hover scoreboard (player names + captured-mana
    %03d, sprites [85]/[86]), and PAUSED text (132,50). Shared dep.
  - Spell affordability marks (sub_247C0 diagonal stripes over
    unaffordable owned cells) — dropped with the fake "unaffordable =
    dimmed" state; the real overlay isn't drawn yet.
  - Equipped-hand LMB/RMB indicator as a LABELED unfaithful option
    (parked `slot_quad`/`slot_uv` composite path, #[allow(dead_code)]).
  - Book-map bilinear-smoothing toggle (jagged rotated coastline;
    matrix opt-in, default crisp).
  - Camera ROLL in the flight view (banked from Phase 5).

**TIER 4 — data-fidelity confirmations (live play / screenshot).**
Wizard-panel castle-HP-in-slot-A vs player-HP-in-slot-C with an
established castle; the lo-res (320×200) book layout (we target hi-res
only — its coords + smaller bottom bar are untouched); dot-aliasing
AA/post pass; a thin black panel outline seen on some HUD panels.

METHOD note for the pass: the LUT bake is the ONE change that closes
most of Tier 1 at once and future-proofs the arctic/MC2 palettes;
Tiers 2–4 are then discrete visual confirmations, not fidelity risks.

## HUD/MAP REVIEW-FIX SESSION — LANDED (2026-07-08, Fable review pass)

A 10-angle review of the HUD + book/map commits against the decompile
found and fixed these (all divergences verified against
reference/remc1 line-by-line before changing code). Build + workspace
tests + headless HUD/book/opaque/transparent smoke renders green.
PLAYTEST CLOSED (player, 2026-07-09): HUD and map faithful, no
complaints after fixes. One NEW banked item spun out: aspect
stretching at non-4:3 resolutions — its own investigation later
(see PLAYTEST-10 certification sweep).

**Fidelity fixes (decompile-verified):**
- **Book bind gate was WRONG** (:26926): the real gate is the spell's
  `castle_req` (+132, ctor a8) vs the CASTLE's stored mana (+140) —
  `req==0` spells (Fireball/Claim/Heal/Castle) are ALWAYS bindable;
  it is NOT possess_mana vs player mana (that lockout blocked
  re-equipping when poor and bypassed the unlock ladder). Sim now
  exposes `LoadoutView.bindable[24]` (all-true under dev_spells);
  unit test `book_bind_gate_is_the_castle_stored_unlock_ladder`.
- **Hover treatment** (:26933-86): the ring (sub_24DA0/24D20, ink
  byte_AE167) draws on ANY hovered cell — locked and unowned included;
  the hovered BINDABLE cell instead gets a full OPAQUE equipped-panel
  redraw after the grid loop (`sub_23D40(x,y,spell,1)` — a4=1 = raw
  DrawBitmap frame [1]/[2] by burst + icon + availability meter,
  overdrawing neighbours with the 64×44 frame).
- **bar() drew an invented dark track**: sub_22810 (:26991) draws ONLY
  the clamped fill (skip <2px) straight on the marble — and our track
  was ALSO covering the amber capacity fill under the white stored
  overlay (the capacity ladder never rendered). Track deleted; if
  bars read poorly on the transparent marble over our darker sky, an
  opt-in track returns as a labeled improved-column option.
- **Alert flashes = THREE independent counters** (u8_391 castle :56705
  / u8_392 player :55679/:55692/:55723 / u8_393 balloon :56826, each
  =4 on a processed hit): sim gained player_alert/balloon_alert
  (castle_alert existed), set in the ch0/ch3/ch4 player intakes and
  the balloon damage inbox; slots A/B/C flash their own counters,
  blink-gated at tick parity (retail alternates per frame and
  decrements per flash — approximation noted in code).
- **Balloon panel semantics** (:27278-344): glyph = [50+ROSTER] where
  roster width comes from castle LEVEL and survives balloon deaths
  (dead slots just draw no bars, :27335-40); marble [54] only when NO
  castle (:27281). `LoadoutView.balloons` is now the roster
  (`Vec<Option<(hp,cargo)>>`); test extended.
- **Win tick** (:27267-74/:27380-87): TWO 2×2 marks at y=26 AND y=38
  at `+58 + (pct<<6)/100`, colour alternating between the two
  team-ramp entries per blink (white/grey stand-ins); the green
  completed recolour stays as our LABELED helper (retail has none).
- **Player marker** (:57449-69): the ~2px white square was wrong —
  retail draws a four-arm fading CROSS, arm = surface_width/12, 1px
  thick at ANY surface res, fading through the fog ramp 0x2C00→0x2400
  (linear white-mix approximation until the LUT bake). Now in
  map.wgsl (fwidth hoisted before the round-mask discard for
  uniformity).
- **Guide ants** (:57155-82): retail steps 4 MAP-SURFACE pixels along
  the PROJECTED line (start (tick&3)+4, break at the surface edge) —
  ours stepped 4 world TILES baked into the texture (sparser on the
  book map, stretched with radar zoom). Now screen-space
  (`project_guide_path`) beside the stamps; `MapOverlay` lost
  `path`/`stamps` (both draw at render time now).
- **vitals_quads stale alert strip**: the amber castle-alert bar still
  flashed at the OLD bottom-right panel coords (panel moved to the
  top strip) — deleted; slot A's [55] is the cue.
- **Rival castles** (:57330-66): retail stamps EVERY castle with
  [58+team] unconditionally (only balloons check reveal v59); our
  player_owned gate stays ONLY because MapIcons bakes just the
  player-team sprites — corrective comment in entities.rs, lift when
  team icons land.

**Stamp geometry (mgc-render):**
- **Wrap-after-rotation**: stamps now project ALL wrapped images
  (±tiles per axis) through the exact shader transform — the old
  per-axis-wrap-then-cull lost edge duplicates; KEY INSIGHT from the
  yaw-sweep test: at diagonal headings the 256-tile book span
  GENUINELY hides far-corner tiles (rotated window misses every
  lattice image — retail's "edge weirdness" relative), so the correct
  invariant is stamps ≡ shader visibility, image-for-image, which the
  new `project_map_stamps` guarantees; tests: full-yaw sweep inside
  the inscribed disc + edge-stamp wrap DUPLICATE (pane y-span 279 >
  256 world shows edge tiles twice).
- **Stamps now SCALE with the surface** (book: res_x; radar:
  disc/128) — they were raw native px (a speck at HD; retail hi-res
  doubles positions but never ran >640 wide, so scaling is the
  proportionality reading, noted in code).
- **Stamps CLIP to the pane/disc-bounding-square** (uv trimmed
  proportionally, `clip_quad_to`) — anchor-point cull stays (retail
  only marks entities whose map position is inside the window);
  per-pixel disc rim mask deferred to the LUT-bake pass.

**Hygiene/perf:**
- MGC_HUD_OPAQUE now honours its VALUE (=0 forces transparent) and
  run_screenshot reads `hud_transparency` from config like live play
  (env var = A/B override only). NOTE: the player's live
  mgcarpet.json runs "opaque" by choice (transparent marble over our
  darker-than-retail sky kills radar contrast — the reason the
  option exists).
- Book layout constants: ui.rs consumes mgc-render's pub
  BOOK_SPELL_X/Y (one source, no cross-crate drift); MAP_TILES
  replaces hardcoded 256 in both projection fns and reaches map.wgsl
  via mode.w; hud_quads' duplicate push closure → push_opt;
  minimap_on dead API removed (radar draws once a level loads).
- Perf: update_map (256×256 recompose + full upload) now runs per SIM
  TICK not per display frame (all baked content is tick-derived; ants
  march per frame anyway being screen-space); the per-frame ui_quads
  clone → two-region write_buffer into one vertex buffer; loadout()
  castle scan shared across castle/castle_hp/balloons/bindable;
  vitals() computed once per frame.

**Playtest-fix round (same day, player report):**
- **PAUSE landed** (P key): sim clock freezes + accumulator drains (no
  catch-up burst), renderer/UI/book stay live; ‖ glyph at retail's
  PAUSED spot (132,50) until DrawText. Book bindings now apply
  IMMEDIATELY while paused (`World::equip_hands` extracted pub; the
  tick would otherwise consume `pending_equip` only after unpause —
  the stale-HUD report).
- **Locked spells = the fog WASH, not stripes**: sub_247C0 is NOT
  diagonal lines — it remaps the whole cell rect through FOG ROW 0x30
  (`fog[256·a6 + dest]`). Both sub_24230 (:27860, book cell) and
  sub_23D40 (:27767, equipped panel) draw it when castle_req (+132,
  read as _DWORD — NO u16 truncation) > castle stored (+140) or no
  castle. This is the missing visual for the unlock ladder — the
  player's "unusable spells on 024" were the FAITHFUL gate with no
  tell. LOCKED_WASH lands on book cells + equipped panels; polarity
  guessed PALE (fog rows read bright-high per the cross ramp 0x2C→
  0x24) — RETAIL CHECK OWED, one constant to flip.
- **sub_24230 decoded properly** (the earlier port paraphrased it):
  icon drawn `DrawBitmap(a1, a2)` = CELL ORIGIN top-left NATIVE size
  (2px right + 3px bottom slack — the player's "icons too low,
  castle touches bottom" was our center+fit-to-cell), slab swaps
  [3]→[4] while the burst counter runs (the cooldown veil was our
  invention — deleted), digit badge at the origin and gated on a
  per-spell +844 COUNTDOWN decremented per draw (retail badges FLASH
  after assignment; ours stay on in the book as the readable
  reading), and sub_23D40's redraw RE-STAMPS the digit (:27749-67)
  — the "digit hidden while hovering" report; now re-stamped.
- **Availability dots = ONE native pixel each** (SETTLED against the
  decompile after a player screenshot round: sub_615D4 hi-res /
  sub_61594 lo-res each write a single byte — the "2×2 shaded dot"
  in DOSBox captures is its upscaler smearing that pixel across the
  2-px spacing grid). `meter_dots` shared by HUD + book-hover redraw.
- **LOCKED_WASH polarity = DARK** (player retail book screenshot:
  locked cells are clearly darkened) → fog rows run DARK-high, which
  CONTRADICTS the white map-marker cross fade (rows 0x24-0x2C) —
  cross polarity re-check owed in play; the mix target flips to
  black if retail's cross reads dark (comment in map.wgsl).
- **Pixel snapping**: all UI quads snap to the integer pixel grid
  EDGE-CONSISTENTLY (round left/right independently — tiling
  preserved); kills the row-vs-row rasterization mismatch (dots) and
  placement shimmer at fractional scales (1.5 at 720p). REMAINING
  in-sprite aliasing at fractional scales is structural: the real fix
  = compose the UI at NATIVE 640×480 offscreen and upscale ONCE with
  a chosen filter (integer-snap/linear/etc.) — BANKED as the
  ui-native-layer work item (also the road to clean non-4:3).
- **Old bottom-center life bar deleted** (redundant with slot C;
  player). Grace shimmer + death overlays stay — the shimmer is the
  white 100%→0 drain at spawn (unfaithful invulnerability cue;
  player-sanctioned TEMPORARY, delete once mortality playtesting is
  done).
- **Quick keys: one spell ↔ one digit** (player: retail unassigns the
  spell's previous key; two slots on one spell fought over the digit
  badge) — assignment now clears the spell's other slots.
- **Unowned-relief shade** (player side-by-side: ours very close,
  retail a touch darker/browner): the EXACT result is computable
  today — the bundle already ships the blend LUT, so the unowned
  relief can be pre-baked CPU-side as icon-coverage → blend[0xA6 |
  slab<<8] tiles (the parked slot_uv composite machinery does exactly
  this transform); fold into the LUT-bake pass.
- sub_23D40 ALSO has a second meter (+61/+62, ink byte_AD167[241],
  55px ruler) drawn over the availability bar — charges/uses of the
  equipped spell? NOT PORTED yet; fields unmapped. Banked.

**Still parked** (unchanged): slot_quad/slot_uv composite atlas rows
(the future unfaithful equipped-hand indicator), the +844 badge
countdown + +14421 flag semantics, the sub_23D40 +61/+62 second
meter, DrawText path, LUT bake (Tier 1 — the ring ink, ant ink,
cross fade, alert blink cadence, SLAB_DIM, LOCKED_WASH row 0x30,
blend[0xA6|dest] mask factor etc. all cite it).

## PLAYTEST-8 REPORT (player, 2026-07-08) — split into two clusters

Agreed split: Cluster B (mechanical fixes) THIS session; Cluster A
(mob-AI fidelity) banked as the NEXT dedicated trace session.

### Cluster A — mob-AI fidelity (LANDED 2026-07-08, dedicated trace
### session — 4-agent decompile fan-out + fixes; PLAYTEST CLOSED
### 2026-07-09 [wyvern + griffon thorough, bee lunge, genie via the
### level-010 win-trigger spawn] except: autoaim held open pending
### the planned crosshair target-predictor dev feature — observed
### working, but the predictor is the tool to really see it)

Player observations, verbatim substance:
- **Aggression classes are wrong**: wyverns should aggro on SIGHT —
  and not just of the player: peaceful until they see ANYTHING
  (player-corrected 2026-07-08). They happily wreck human dwellings;
  MOST mobs attack humans, with per-model ferocity levels. Once a
  wyvern sees you nearby it picks up your trail, follows and shoots
  (very powerful rapid fire, kills in 1-2 attacks). Ours only aggro
  when provoked — which is the GRYPHON's model (peaceful until
  attacked). Needs a real disassembly pass on target
  acquisition/ferocity per model — the wander/awake trace skimmed it.
- **Mob special abilities almost completely absent** (kraken duel/
  tether was implemented specially; the rest never trigger — likely
  gated on attack-mode states we never enter):
  - Gryphons: effectively PERMANENT REBOUND once in attack mode —
    first meteor lands, after that fireballs/meteors deflect off
    them; lightning is the counter.
  - Bees: ACCELERATE on your trail — practically no escape in
    retail; ours are slow and mostly harmless.
  - Genies: MANA STEAL (unverified in ours yet — player needs a
    genie level; check in trace anyway).
- **Spell autoaim/target-snap is off**: retail generally SNAPS to
  target. Our meteor aimed at a bee cluster can fly straight through
  it (retail: picks one, explodes, kills the cluster). Crows
  following the player are near-impossible to hit (fast movers).
  Player suspects data-driven aim assist — trace the projectile
  launch / target acquisition path.

#### THE SESSION RECORD (2026-07-08) — all four items traced against
#### sub_main.cpp and landed; tests green (6 new); playtest owed

**1. AGGRESSION — the engine has NO per-model aggro list.** The
shared WANDER sub_19D70 runs, for EVERY awake creature, Scan A over
the class-3 wizard list `36462` (range v_28² + cone v_30 +
invisibility +16&0x20) → +146/CHASE, then Scan B over the same-owner
creature list `str_36382x[+65]` (+52 gate, no invis check) → PACK.
Our hardcoded whitelist `matches!(m, 1|2|3|6|7|10)` was the
invention — deleted; all (m,1) models scan. IDLE's pack scan is NOT
awake-gated (verbatim). The awake pre-pass wake test is 3D
(sub_42410 :64353) — ours was 2D, fixed. Behavior row = per-SPAWN
constant, not model-indexed (agent report has the full 17-row
v_26/v_28/v_30 table; wyvern row 25 = 40/0x1200/0x200, griffon row
20 = 40/0x1900/0x200).

**2. WYVERN m16 (states 96-102).** Its wander sub_20710 calls the
SHARED sub_19D70 first (⇒ full sight-aggro inherited — our port had
scan=false, wyverns could never aggro at all) then layers the HOUSE
HUNT (:26033-58): nearest class-10 m45 within v_28² every v_26+1
ticks, NO cone / NO invisibility / NO awake gate → +146, chase.
Custom chase sub_207E0 (:26062) ported verbatim as wyvern_chase:
bearing every 8th tick and only when target is class 3 or beyond
0x200 3D (no orbiting the house it burns), dead/expired → hunt,
burst +26 = one homing (9,0) fireball PER TICK (row unk_98F38[2]
turn 0x71, +44=3000, +140=60000, 4x launch height, filter = the
wyvern's OWN +66/+67 — we had literals), every v_26 a SQUARED 2D
range drop-out (the shared chase's is un-squared 3D), roar sound 39
in the RE-ARM block at 2*v_26 (we had it in the burst), cone 0xE3 →
+26=15. Spawn life 100000. Hit-in-chase retargets without state
change (generic inbox role-2 arm ✓).

**3. GRIFFON m8 — the rebound + the retaliation.** State 50
sub_1CE30 (:23552) is the ONLY creature code that raises the
deflection bit (+17|=0x80 = our flags 0x8000, the same bit our
combat already checks): re-set EVERY chase tick, cleared by NOTHING.
Fireballs/meteors full-deflect off it (sub_52B30 :62849: mana debit
+140/4, trajectory reversed, retargeted at the shooter); beams
(lightning) never full-deflect — at most a quarter-nerf gated on
class-3 targets (:63435-47) — so lightning stays the counter, by
construction. "First meteor lands" CONFIRMED by code order: the
provoking hit is applied in IDLE before the state-50 transition sets
the bit. Retaliation: IDLE sub_1CA50 (:23455-58) promotes a
hit-by-wizard griffon straight to chase — our inbox EXCLUDED m8 from
retaliation (fixed: only m12/13/14 mark-without-chasing); griffon
death stamps the killer's +528=200 (Dead arm now includes m8); the
proximity scan stays +528-wanted-gated (:23500) = player_aggro, so
unprovoked griffons hold fire. griffon_chase: full speed while +26
runs, screech 38 every v_26, attack thunk stamps the wanted timer.

**4. BEE m2 — the lunge, and the catch-up mis-fix RE-CONFIRMED.**
sub_1B3C0 (:22335): the sting cooldown +26 counts down BEFORE the
shared chase and on the expiry tick the bee LUNGES at 3x maxSpeed
(:22346-47 — 210 vs base 70); leaving the chase state resets to
maxSpeed (:22363-66). Both were unported (bees "slow and harmless");
the thunk's cooldown gate we'd invented is deleted (the original
stings whenever the cadence lands it in range — the recoil physics
is the real cooldown). THE CATCH-UP ARBITRATION: this session's
trace agent read :21814's `+126 += leader.+130` as authentic and we
flipped it — the asleep-crowd regression test IMMEDIATELY caught the
runaway; re-reading the dead decompiler temp settled it for good:
`//v10 = v3x->+130 + v3x->+126` reads BOTH operands from the LEADER
(a dead temp of the += form would read the member's +126), so the
original binary computed `member = leader.speed + leader.accel`
(bounded) and the += is remc1's maintainer mis-fix — exactly the
2026-07-05 session's call. The retail "no escape" is the 3x lunge.

**5. GENIE m11 — the full state machine, previously two nops.**
IDLE sub_1DE40 = the BLINK CYCLE: 12-puff (10,1) sparkle ring on a
3x4 grid of 40-unit cells → +26=1 → next expiry sound 21 and the
phase bit (+16 byte0 bit0, ours flags 0x2000) picks random TELEPORT
(((rand%0x3C)<<8)+12800 per axis, toroidal — 50..109 tiles NE) into
WANDER vs straight into CHASE. WANDER sub_1DFE0 = self-heal
(+maxLife>>6 clamp, every v_26), awake+quarter-life-gated wizard
scan → AMBUSH, else EAT A MANA BALL (sub_1E810: nearest m39 in
v_28², absorb + destroy + (10,0) puff + sound 11 — the OTHER half of
"genies steal mana"), yaw jitter, and above 3/4 life the MANA HUNT
(:24523-46): the FIRST wizard holding ANY mana, NO range/cone gate →
ambush. AMBUSH sub_1E770 = teleport to actSpeed<<6 = 15 tiles AHEAD
of the target ALONG THE TARGET'S OWN HEADING at the target's z, then
idle (the ring/blink cycle re-enters chase). CHASE sub_1E380:
below-half break-off and out-of-range/target-dead ends all BLINK
HOME (sub_1E720, sound 11) — not walk; chatter 11 every 8*v_26;
seeker every v_26 (the +26 alternation is vestigial — the decompiled
branches are identical, +69=25 always). Steal chain re-confirmed:
seeker (9,8) carries +44=3000 → impact spawns (10,25) → ch3 area
write → victim loses the mana UNCONDITIONALLY, stealer gains it ONLY
if class 3 (:55689-91) — the class-5 genie DESTROYS it (our port
already right). Hit-retaliation ambush-blinks at the attacker.
Deviation noted in code: the engine falls through after a home-blink
and fires one stray seeker with the CLEARED (slot-0) target — we
return instead.

**6. AUTOAIM — the snap is real, but it lives IN FLIGHT, not at
launch.** Player launches NEVER write +146 (all sub_57xxx arms:
carpet yaw/pitch + the +150 point 0x4000 ahead). The snap = every
visible class-9 flight handler calls the acquire scanner sub_54520
(:63943) while +146 is invalid — and 0 IS invalid, so player bolts
re-scan EVERY TICK. The scanner is keyed on SUBTYPE +65 (a code
constant — the player's "data-driven" suspicion, nearly): cases
{0,3,4} = wizards + all other-team awake creatures (±0x71 yaw AND
pitch cone, 3D ≤ 5120, min weighted score), {1} = possess
(balls/houses), {7,8,11,12} = wizard-only, {9} = the beam pick,
default = none. So fireball m0, METEOR m3 and volcano m4
self-acquire; crater m5 / magnet m6 / quake m2 authentically don't;
duel m7 / steal m8 / undead m11 acquire only wizards (no-op until AI
wizards). PORT: m0 one-shot acquire → per-tick; meteor's generic
flight + the payload path (m4) now acquire per tick; possess m1
per-tick. Homing turn caps stay authentic (fireball row
unk_98F38[5]: v_2 = 5/tick yaw, v_6 = 22/tick pitch) — a bolt
launched wide of a fast lateral mover STILL misses (= retail "crows
near-impossible"), and the cluster kill is the acquired bee's
detonation splash. Impact stays pure AABB overlap (sub_11980 grid +
extents — no proximity-to-target test). Corrections to the trace
brief: sub_46B00 is a damage-application routine, NOT the cast arm;
sub_580A0 (:66235) is the (9,18) Global-Death fuse spawn. STILL
UNSEEN (table truncation): class-9 state handlers ≥ 14 (the real
retail fireball state-19 flight if the player fireball is (9,18)-
adjacent — ours flies state 0, consistent with everything visible)
and the (10,55)/(10,17) blast radii (ported earlier from their own
traces).

**Tests** (all green, workspace): wyvern_aggros_the_player_on_sight,
wyvern_hunts_and_burns_houses (asleep hunt + chase + house damage),
griffon_peaceful_until_hit_then_rebounds_and_retaliates,
bee_lunges_at_triple_speed_after_the_sting,
genie_blinks_ambushes_and_steals_mana (blink displacement + (10,25)
flash), fireball_snaps_to_offaxis_targets (4°-wide kill on the
stationary militia; the asleep-crowd runaway regression re-proved
its worth against the catch-up flip).

**PLAYTEST**: PLAYER-VALIDATED same day (map 024, mid-session):
wyvern sight-aggro + lethality ("died before I could say wyvern")
and the griffon rebound (a second death) both present. GENIES
VERIFIED 2026-07-09. AUTOAIM VALIDATED 2026-07-09: vultures
("crows"; the manual's name) noticeably easier to hit — the
per-tick re-acquire was the missing snap. BEE LUNGE + METEOR
SNAP/CLUSTER KILL VERIFIED 2026-07-09. WATCH ITEM (player, no
specific claim yet): METEOR DAMAGE feels suspiciously LOW — player
will run a detailed retail comparison; our chain = the m3 bolt's
row +44 = 10000 copied on the generic path (:62759-72) into the
(10,17) growing blast ring's 10-tick broadcast (sub_25CE0) — if the
report firms up, re-trace the ring's per-tick damage split vs
retail (one 10000 mailbox write vs the retail total over the ring
growth is the likely divergence axis). WYVERN HOUSE RAZING VERIFIED 2026-07-09 (visible in the first
seconds of level 024). GRIFFON full behavior VERIFIED same day:
peaceful until attacked, raises the rebound on retaliation ("they
launch the rebound spell" — the player reads the bit going up as a
cast), and the FIRST STRIKE (e.g. a meteor) still lands before the
rebound is up — player-certified "correct and faithful to retail".
STILL OWED: the lightning counter on a rebounding griffon;
volcano-lob homing (traced case 4 — verify it reads right in play).
Awake-gated damage intake means a sleeping (>24 tiles) creature
banks hits until you close — verbatim, but worth a feel check.

**BANKED FEATURE (player, 2026-07-09): the predictive AUTOAIM
CROSSHAIR** — a per-spell projectile-behavior predictor/debug
instrument more than a combat aid: a read-only per-frame acquire
query from the muzzle pose (per hand, keyed by the equipped spell's
projectile subtype), drawn LOCKED vs UNLOCKED (color; unlocked = the
neutral aim point) so acquisition state is visible without
fly-and-shoot testing — the player wants it to diagnose
suspected-off shooting distances (no specific claim yet). Feasible:
needs a pure no-mutation variant of aim_assist (no
f146/f34/f36/player_danger writes, no LCG draws); opt-in "improved"
column per the authenticity matrix. Caveats: acquire ≠ hit under the
authentic 5/tick turn cap; first-tick prediction undersells later
in-flight snaps; bonus: camera = HALF aim pitch, so the true aim
point sits off screen-center — the crosshair fixes real control
readability too.

### Cast-cadence fidelity — CLOSED as correct (player, 2026-07-09)
### + a standing DESIGN NOTE on tick rate

In play the cadence seems correct — closed. But the deeper finding
(player): there is NO SUCH THING as a strictly faithful cadence.
Retail's cadence was tied to tick rate and time rate, and tick rate
was VARIABLE because the original locked ticks to fps — raising the
resolution literally slowed the game down, to the point that rapid
clicking could reach the same fireball cadence as the hold-to-repeat
rapid fireball. That was always wrong in retail; we must pick our
own canonical constants. Player's guess: the most faithful tick-rate
/ time-rate constants will come from MC2 — take them from there when
the timing pass happens. (Original suspicion about the burst counter
[recast lockout vs emission cadence, sub_46B00 :55851 + LABEL_32
:55892] kept for reference but no longer drives a session.)

### Cluster B — mechanical fixes (LANDED same day; broad play since
### surfaced no complaints — 2026-07-09 certification sweep)

All five fixed against the decompile, workspace tests green:

- **Death "archers" = sprite-less scattered jars.** The class-12 ctor
  sub_3BF70 (:47979) gives EVERY jar sprite type 77 + a 4x extent
  override; our `grant_spell` manifestations never got a sprite, so
  the death scatter dropped type-0 billboards (the "archers"; they
  "vanished" because scattered jars decay in 200-289 ticks).
  grant_spell + the pre-placed spawn arm now both apply 77 + 4x
  extents (the generous pickup vacuum).
- **Global Death — settled in THREE passes (final = the real
  handler, player-playtest-driven).** Decompiled facts: state 0x42
  (class-12 states run 3xmodel) = sub_580A0 :66235 spawns the (9,18)
  at the wizard (ctor sub_3A390 :46392: fireball-shaped boilerplate
  — speed 384 + carpet speed, life 0x2000/384 = 21, row [5], sprite
  42; +26 = the ACCUMULATED CHARGE byte 326 zeroed into it on
  release, role unknown; +150 = the aim point 0x4000 ahead),
  detonation = (10,55). State 19's flight handler is PAST remc1's
  truncated class-9 table (MC2 engine: no analog) → reconstructed
  from PLAYER GROUND TRUTH (fire once, wait, blast lands around the
  caster; intro meditation = likely artistic license): the fuse
  RIDES THE CASTER for its 21-tick life, then raises the field in
  place. THE FIELD (10,55): pass 2 ported the WRONG HANDLER — the
  class-10 table (str_255998) is keyed by STATE not model (verified:
  napalm state 58 → sub_29780 ✓); the model-key landed on state 55's
  terrain-raising volcano riser = the player's "it does a volcano"
  report. The REAL state-60 handler sub_299D0 (:31263), now ported
  verbatim: +26 (32) priming ticks each playing SOUND 43 (the
  tick-tock), then ONE full-pool sweep — enemy entities within 0xA00
  = 10 TILES by PURE 2D DISTANCE (sub_423D0 is x/y only: the
  infinite vertical KILL CYLINDER = the "shoots very well up/down"
  flat plane): class 2/5 die INSTANTLY (life = -1, no kill credit,
  no effect spawned — "the monsters simply explode"), class 3 take
  the 7000 on ch0, own team skipped; finish = sound 44 at the field
  AND the owner + the sub_44BE0(owner, 3) SCREEN FLASH, free. NO
  terrain change, NO drift, NO visuals — authentically invisible;
  total cast→boom ≈ 21 + 32 = 53 ticks ≈ 2s (matches playtest-3's
  observed prime). The ctor's life-19/speed/heading/extents are dead
  weight (kept verbatim). The old timer-55 + 3D-proximity pulse (and
  its sound 30) deleted. FOLLOW-UP (same day, playtest round 2): the
  fuse's ctor sprite 42 DREW while riding the carpet — a 1-2s
  rapid-fireball explosion series above the player's head; retail
  shows NO prime visual (the draw gate lives in the missing state-19
  handler → the player observation rules) — (9,18) is now excluded
  from drawable(). ROUND 3 (same day): overlapping charges RETAIL-CONFIRMED
  in play ("launch multiple and make a flyby — nothing survives, a
  tail of unclaimed mana") → the spell-22 recast now RESETS the
  burst instead of gating on it (charges stack, each fuse
  independent; test extended); the huge-sprite aiming feel ("looks
  like you're there when you aren't") confirmed authentic. RETAIL
  CHECK STILL OWED: blast site tracks the carpet vs parks. UNMODELED, banked: +26 charge-byte semantics, the 101x742
  mana drain (ours debits at cast), the sub_44BE0 screen flash (the
  GENERAL flash mechanism — player: retail flashes "on many
  occasions", untraced). LESSONS: class-10 dispatch tables key by
  STATE; when the decompile is truncated, reconstruct from player
  ground truth, not structural similarity.
- **Balloons come home.** The dispatcher (:56376) DEFAULTS every
  state-9 balloon's target to the CASTLE each pass, overriding to the
  nearest free claimed ball only when the balloon has cargo room and
  the castle census is below capacity — no ball → castle stays the
  target (return, offload, hover home). Our none→idle parked them at
  the last pickup AND skipped the altitude servo (idle = early
  return). Fixed; test extended (returns + hovers the castle
  neighborhood). ALTITUDE: verified faithful already — the row-9
  servo (sub_42000, band [g+512, g+1536], DESCEND-only -16/-4 with a
  hard floor snap-UP) ratchets balloons high over rolling terrain
  (hills snap them up, valleys drain at only 4/tick); the tethered
  ball rises 128/tick to the balloon for the pickup dip. MC2's
  different profile = a future authenticity-matrix alternate.
  Untraced nicety: retail staggers retargeting by castle+63 % fleet.
- **Castle final destruction: the tower was only ON SCREEN.** The sim
  un-stamp (the zeroed fake collapse, verified cell-for-cell the
  inverse of the painter's sculpt) already flattened the footprint —
  but castle_tick has no dirty-returning dispatch arm, and the final
  destruction spawns no follow-up painter, so the renderer never
  re-uploaded terrain: flag gone, tower ghost stays. Gen now carries
  a terrain_dirty accumulator (set in castle_downgrade), merged by
  World::tick. Real-BUILD.DAT integration test: barren square, no
  stump, no lingering protection bits.
- **Audio batch.** The DING = sound 14 = sub_3DC90 (:49072), the
  SCREEN-MODE switch — played at EVERY mode change: level start
  (World::new), map/book enter AND exit (app toggle), respawn (case
  0xF → sub_3DC90(0) :48640; player independently recalled this one),
  and the book equip chime (:48721/:48729, equip_hands). Jar pickup
  = sound 18 at the wizard (:64848, try_pickup). PAUSE now suspends
  ALL sound: output-thread Suspend command — channels + music FREEZE
  positions and stream silence, resuming where they left off; a map
  toggle while paused leaves its ding queued in the mixer and it
  flushes on the first unpaused tick = the retail deferred-ding
  quirk (our per-id request slot plays it once even if retail would
  overlap two — noted).

## HOSTILE WIZARDS (RIVAL AI) — TRACE BANK (2026-07-09, 5-agent
## decompile fan-out; the durable record — agent reports are
## session-local)

Scope: everything class-3 model-1 (the AI wizard), from level data to
map labels. Headline: the AI is a DECISION LAYER on top of engines we
already ported — it casts through the same class-12 spell entities,
takes damage through the same sub_46540, and obeys the same mana
economy (census/costs/castle-stored thresholds). All cites remc1
sub_main.cpp.

### Level data (importer work — the one data gap)

The 38812-byte level record tail (str_193795):
- u16 @38800 (map-screen coord math :27268), u16 @38802 = PLAYER
  COUNT (var_u16_10 ← :51537), u8[8] @38804 = per-player STARTING
  CASTLE level (0 = none, N = spawn castle at level N−1). remc1's
  `byte_38C97` static initializer is DATA-SEGMENT GARBAGE and its
  :54972 read indexes off base+player (wild) — never trust remc1
  here; read the level record.
- str_230867_37072[8] @37072, 216 bytes/player: +4 u16 → Type_160
  u16_522 AGGRESSION (hate rise rate, war thresholds, opportunism
  margins), +12 u16 → u16_524 ACCURACY (aim cone `((255-p)/4+20)°`,
  rebound-notice probability), +8 u16 → u16_526 TEMPO (decision
  period `64-p/4` ticks, turn agility `/(8+(255-p)/16)`, burst pause
  `(255-p)/8+1`, respawn delay `32*((255-p)/8)+32`), +16 u8[24] =
  pre-granted spells (var_230883), +116 u8[24] = allowed/availability
  mask (var_230983 — the same mask the human grant intersects).
  +0..3/+140..215 unread anywhere. AI grant at level init =
  var_230883 && var_230983 (:49222); the allowed mask also loads the
  AI's learn-eligibility list Type_160+796[24].
- Player start markers = class-3 MODELS 4..11 placements (models
  4+slot): spawn NO entity, they write str_9177[slot] (:44068-107).
  Wizard spawns at marker, z = ground + 256 (:54845-49).

### Spawn / init / respawn (sub_44D30_45070 :54802-55062)

Callers: (1) command dispatcher sub_3C9D0 :48633 — command 1 posted
for all slots at level init by sub_3DD50 :49151 (slots ≥ player
count inert), command 0xF = human Space-respawn (castle-less single
player → 13325 |= 0xC restart), command 3 = unused; (2) the state-3
dead tick :55616 = AI AUTO-RESPAWN. Fresh spawn = sub_373F0(pos, 3,
is_ai) → AI is class 3 MODEL 1, STATE 1 (class-3 dispatch is by
STATE +70, table str_254ADC :4668: 0=human tick sub_45C90, 1=AI tick
sub_13170, 2=death fall, 3=dead/respawn-wait, 4/5/6=castle, 9=
balloon). Respawn reuses the entity, teleports to castle. Both:
grace u16_331=100, mint the 24 carried spell-jar entities (+532),
sprite by PLAYER SLOT (0→44, 1..7→273..279 — seven DISTINCT 8-frame
rival art sets, draw type 0x11 = 16 views via mirror, NO anim
frames, world height 200 like the player; :54927-55). AI branch:
personality u16_522/524/526 from the level record; STARTING CASTLE
if owns spell 16 && byte_38C97[p]>0: castle (3,2) at wizard pos
(even-parity tile snap), level = byte_38C97[p]−1, the build painter
sub_279D0 replayed per level 0..N−1 via a scratch record in slot 0
(terrain stamped to match), extents sub_37150, capacity ladder
sub_47C60, castle spawns FULL (stored = capacity, cap 320000), sound
30 (:54963-55005). NOT the balloon spawner: balloons come from the
CASTLE TICK roster sub_47400 :56264 — (balloons, workers) by castle
level 1..7 = 1/0, 1/0, 1/4, 2/6, 2/14, 3/18, 3/34; missing balloons
respawn at the castle (3,3), owner = castle owner, excess destroyed;
workers = class-5 m15 spawns on 16-tick cooldown into Type_160+84.
Every OTHER wizard's hate row toward the (re)spawned player is set
to −24609 (40927 = elevated-but-decaying) = post-respawn truce
(:55037-41). Model-1 extras: AI state +415 = 0, own hate rows =
24607 neutral, castle-spell cooldown slot (+724[16], the field remc1
shows as "+756") = 4*player — a per-player build stagger. Rival
NAMES: off_99B68[8] by slot = Zanzamar, Vodor, Gryshnak, Mahmoud,
Syed, Raschid, Alhabbal, Scheherazade (:5741, copied :49158).
Human at 1,000,000 life/mana-base if named unk_AE89E (dev cheat,
:55019-28); normal = 10000 life, mana base u32_322 = 1000.

### The AI brain (sub_13170 :17842; housekeeping sub_132B0 :17903)

Per tick, always: burst-lockout recovery (+404<0 → ++); decrement 24
AI re-attempt cooldowns +724[]; HATE DECAY toward 24607 (below: +=
u522+1; above: −=(256−u522) only while war flag +462 clear); book
rebuild sub_45C10; at-own-castle (AABB) → grace=2 and the mailbox is
DISCARDED (human's forwards to the castle — asymmetry, retail-check
owed); else sub_46540 damage intake (shared: shield quarter+mana,
knockback dmg/10, kill credit +38, sound 17); MOVEMENT sub_14EB0;
fireball charge u8_326→200; REGEN (at castle: mana max/200 min 1000,
life max/200; afield: mana max/2000 min 100, life max/500 — AI heals
4× the human's afield /2000, 1.25× at home — deliberate-looking,
cheap retail A/B); spell learning sub_15EC0; every `64−u526/4` ticks
(keyed on entity age byte +63): incoming-projectile scan sub_16800
(nearest class-9 with target +146 = me within 5120) → lateral JINK
v_16=80 (decay 4/tick) + reactive cast (incoming model {0,3,16} →
0xE rebound, {4,9} → 4 shield) + heal (1) if hurt; altitude hard
clamp z ∈ [ground+stat12, ground+stat10].

MOVEMENT (sub_14EB0 :18780): band-settle toward the type's altitude
band (above band: +stat14 sink; upper band: stat14*25/100; below:
floor clamp), forward step ALWAYS LEVEL (pitch 0) at speed +126,
lateral dodge step at yaw+0x200 by v_16, accel ±16/tick toward
target speed v_12, turn rate = angdiff/(8+(255−u526)/16) clamped to
type min/max. NO wall gate (crosses walls — consistent with flying
mobs), NO drag/knock fields (those live in sub_455D0 only), no
pitch. This IS the "emergent clamp" carpet lore, now exact.

DECISION CASCADE (sub_136C0 :18048, run every tick after the state
handler; state 0 runs it twice): (1) castle-less && owns 16 &&
maxmana ≥ cost → scan map as 4×4 supercells from own cell, 2
candidates/cell, site OK if nearest foreign castle > 12288 → site in
+150, STATE 3 (fly out, plant); (2) life < max/2 && castle → STATE
11 flee home; then only on decision ticks: (3) upgrade: castle
state 4 + spell-16 idle + cooldown 0 + sub_12D10 room-to-grow +
maxmana ≥ cost → STATE 1 (fly home, cast 0x10); (4) raid castle
(needs an offense spell of {0,7,8,15,17,20}): hate[owner] >
50000−ownerWealth/10*u522/255 && owner > 7680 away, OR castle poorer
than mine by 640*(255−u522) → STATE 7; (5) attack wizard: skip
invisible (spell-12 active) targets; war flag → immediate; hate
threshold as above; or bully castle-less richer targets → STATE 8;
(6) intercept balloon carrying > 10*(275−u522) away from home →
STATE 9; (7) possess balls (owns 3; if owns 16 only while maxmana ≤
castle cost): wild balls by distance, enemy balls if at-war or
unguarded → STATE 6; (8) hunt any mana-holding creature (all 20
class-5 lists, no range cap) → STATE 13; (9) else full-life or
castle-less → STATE 12 cruise / STATE 11 home. Target = +146 +
signature +148 (team+model+class<<7), revalidated per tick. States
2/4/5/10 have handlers but no setter (cut content). Approach bands
per state (arrive/boost): castle 512/2048, site 2048/3072, ball
1024/3072 (claim: cast 3 while facing within ~5° writes ball+144 =
own id), raid 2048/3584, wizard/balloon/creature 3072/4096; beyond
the boost radius the AI casts 2 (speed-up). State 11 far-from-home
tries spell 0x13 (19, magnet) — DEAD CODE (executor has no case);
also cloaks (0xC) while fleeing.

HATE LEDGER (str_456[8] per player: +4 hate, +6 war flag; feed
sub_16540 :19643 from the sim loop, one-shot per projectile via
byte17 0x20): hit on a CASTLE → victim's hate[shooter] += 5000
(models 3/4/11/16) or 1000, war flag when hate > 50000−wealth-scaled
threshold; hit on a WIZARD → +3000/+500 (read keyed on +63 in remc1
— suspect, likely +65); fire (10,1) burning a claimed ball → ball
owner's hate += ballMana/4. Landing a cast on a wizard clears OWN
war flag toward them (:18338).

CAST EXECUTOR (sub_155F0 :19096; readiness sub_15A00 :19219): AI
"actions" ARE spell indices; both gates check owned + cooldown
+724[s]==0 + mana ≥ cost; aimed groups add an aim cone (commit 30°
for 0/0xF, 40° for 3/7/8/0xB/0xD/0x11/0x14; readiness pre-gate
(255−u524)/4+20 degrees) and set ABSOLUTE aim pitch to the target;
fireball/lightning (0/0xF) also run the 8-shot BURST counter +404
(at 8 → negative lockout (255−u526)/8+1 ticks); 0x10 with castle =
the normal (9,10)→(10,43) upgrade chain, CASTLE-LESS = direct
sub_373F0(site,3,2) spawn — the AI's first castle is FREE and
instant (mana gated only by readiness). Spells 18/19/21/22/23 hit
the default case = AI can NEVER cast (global death, mana magnet,
second speed-up, armageddon-class, rapid fireball). AI re-attempt
cooldowns word_90034[24] = {2,1,32,10,1,0,0,4,400,0,1,0,1,0,1,1,40,
600,0,1,4,2,3,4}. ATTACK PICKER (sub_16030 :19459; castle variant
sub_16310 = same minus rebound branch): poverty latch +406 (mana <
max/4 → no attack casts until > max/4+6000 or max/2); priority 17 →
8 → (target rebounding && rand%255 < u524 → 15) → 7 → 20 → 0 →
fallback 15; per spell: castable now → cast; affordable-by-ceiling
(maxmana ≥ cost, sub_15E90) but cooling/poor → WAIT (save up);
else next. AI aims at CURRENT position (no lead); its projectiles
use the same per-tick homing re-acquire.

SPELL LEARNING (no jar pickup for AI): ground jars only touch-
collect for model-0 wizards; but any jar in the world arms EVERY AI
wizard's timer +628[s]=200 if unowned + allowed (+796[s]); the AI
tick counts down and CONJURES ITS OWN COPY at 0 (off_987DE ctor,
:19381-443) — gated on a human wizard existing. So the AI "learns"
any spell 200 ticks after a jar of it exists anywhere.

### Mortality / property / win

DEATH = the shared path: intake returns 2 → state 2 death fall
(gravity −2 min −256, (10,1) fire trail sprite 41/tick), impact:
kill credit (killer wizard's Type_160+30+2*victimTeam tally — read
ONLY by the book roster), death message (viewer message slot,
"%name% <str 54>", period 100), JAR SCATTER = all 24 carried spell
jars detach (+70++) at corpse ± LCG&0x1FF−256, life = LCG%90+200
(DECAY — free copies; the wizard keeps its spells for respawn),
grave (10,40) sprite 65, in-flight balls re-pointed at the grave,
entity hidden (flag 0x20), respawn timer +26 = 32*((255−u526)/8)+32.
State-3 wait: AI with castle → count down, full re-init sub_44D30
(respawn AT castle, grace 100); castle-less (checked EVERY tick
while dead — killing the castle during the wait counts) →
byte_13329_6 = 0 ELIMINATED (win check :52119 and HUD roster :27100
skip; property NOT torn down: castle would keep working, balloons
keep ferrying, claims persist — but elimination only happens
castle-less; an eliminated hoard keeps depressing the human's share
until re-possessed). Human death unchanged (ported).

WIN CHECK (sub_415C0 :52100, per frame, skipped when AE408+0 &
0x110): for each ACTIVE player WITH a castle: share = 100*(u32_308
[house-stored] + castle stored +140)/world_total; share > required
(level header u16 @38803-ish AE400+0x38C93) for 16 CONSECUTIVE
checks → 13325 |= 2 (won). RIVALS NEED NOT DIE. An AI reaching the
threshold sets its own bit 2 — nothing consumes it in single player
(AI cannot end the level). Lose = own castle-less death only.
world_total (u32_188, census :56867-919) = SEED 1000 (the caller's
u32_322) + the pool walk's f140 over class 5, class 3 MODELS 2/3
ONLY (wizards skipped — carried mana never counts), balls m39,
houses m45 (houses also credit owner u32_308); the agent report's
"incl. wizards" was wrong (caught by playtest-9's mana-accounting
report).

DUEL GRIP: banked trace CONFIRMED end-to-end (victim inbox writes
the ATTACKER's pull u16_314/316/318; flyer pulls the CASTER toward
the victim; release at counter 1000 / dist ≥ 5120 / victim dead).
The AI NEVER casts duel (no selector emits 11) — human-initiated
only; AI victims process ch4 through the same shared intake.

### Presentation

- Billboards: rival = own 8-frame art set by slot (rows 273-279,
  draw type 0x11, mirror views, no anim). Own carpet never drawn
  (flag bit 0 + 0x21 skip). TRACE-ONLY: verify the seven robes
  visually. Row var_11 = 0xFF (44/273/274) vs 0x01 (275-279) —
  meaning unknown.
- TEAM COLOR = NO runtime palette remap anywhere. Mechanisms:
  distinct art (wizards); sprite += team (balloons 169-176, castle
  balls 177-184 on claim :30809, mana balls base 105+8*team by team
  /52 wild, tier by dword_900A4 size ladder :29574-633); the color
  PAIR table byte_99B58[16] = {B7,71, 7D,7A, 9D,9A, 07,5A, 1D,1B,
  DD,DA, 3C,39, 10,0E} (map dots, HUD fills, name text).
- MAP: wizards get NO dot ever (case 3 stamps only models 2/3).
  Castle stamps 58+team for EVERY castle unconditionally (our
  MapIcons must bake all 8 team icons and lift the player_owned
  gate). Balloon stamps 66+team gated own || v59. v59 = BEYOND
  SIGHT remaining ticks (spell 5's jar +48; cast sets duration,
  toggle tick drains mana upkeep). v59 also gates the rival NAME
  LABEL pass :57413-48: every active living rival's NAME drawn at
  its live wizard position in team color byte_99B58[1+2*team] — a
  moving label, not a dot. Owned-creature dots byte_99B58[1+2*team]
  (generalizes our TEAM0 pair), wizard-owned class-9/10 spell dots
  byte_99B58[2*team], claimed balls blink the team pair.
- HUD strip sub_22E50 carries NO rival info (A=castle, B=own
  BALLOONS with HP+cargo bars from +52 roster, C=self) — matches
  what we built. Rival vitals live on the BOOK ROSTER sub_22880
  (:27009-165, hover bottom strip y≥382 of the fullscreen map):
  per active player a row = frame icon 85 team-color filled, NAME,
  current MANA %d (+136... entity mana ceiling), and 8 kill-tally
  cells (icon 86) colored by column player = Type_160+30 table.
- AUDIO: wizard death scream 16 (any wizard, positional); claim
  chime 4 at the claiming wizard (rivals too — ours is player-
  gated, LIFT the gate with positional attenuation); AI fireball
  spawn sound 40 at the projectile :65233; ambient wizard vocal:
  every 64 ticks 1-in-11 per wizard, sound 46 (the same id as the
  wing flutter — TRACE-ONLY, ear-check); danger music v_46=100 armed
  by hits AND by projectile target-acquisition of the human
  (:64013/:64095, confirmed); proximity alarm loops 5/31 via
  u32_396/400 meters (< 1536 starts, fed from :28215/:30831 —
  human-side alert, separate small feature, unported).

### HOSTILE WIZARDS — LANDED 2026-07-09 (same session as the trace
### bank; all workspace tests green incl. 5 new lifecycle tests;
### PLAYER-CERTIFIED same day: level 010 played and WON — rivals
### aggro as remembered, handle themselves through the whole game;
### corpse death un-owns all carried mana and anyone possessing the
### corpse claims it; "everything is faithful". Residual deviations
### = later spottings during the full campaign playthrough.)

Everything below is in `mgc_sim::rivals` (+ world/combat/features
wiring, importer, app) unless flagged deferred.

- LEVEL DATA: the .mgcl layout bug FIXED — the "2095-slot entity
  table" really ends at 37072 (1999 slots); the tail 96 pseudo-slots
  were the 8x216 WIZARD RECORDS misparsed as things (DDLEVELS 198's
  phantom "markers" = its 8-player arena config), and the "footer"
  decodes as (map-word, PLAYER COUNT, castle levels[8]) — the
  parking-lot footer mystery CLOSED (footer[1] = the long-observed
  player count). Format v2: MC1 packages bake wizards.json
  (aggression/accuracy/tempo + castle_level + the two 24-spell
  masks + player_count); FORMAT.md updated; all levels re-baked.
  Campaign survey: rivals from idx 002 on (Vodor, agg 116); up to 8
  players; rival castles to level 7 (idx 015); slot-0 HUMAN castle
  levels nonzero on idx 005/011/015/018/035 — the init gate is
  AI-only in the decompile. RETAIL CHECK ANSWERED (player,
  2026-07-09): some levels DO preplant the human a castle of
  varying level → **WIRING OWED** (extend the starting-castle init
  to slot 0 on those five levels). Memory-vs-data check on idx 048
  (pre-final; player recalled Vodor + Gryshnak both starting at
  level 7): baked wizards.json says player_count 3, rival slot 1 =
  castle_level 7 (agg/acc/tempo 255), rival slot 2 = castle_level 0
  (agg 254, tempo 252), human slot 0 = castle_level 0. So: two
  rivals confirmed, but only ONE preplants at level 7 — the other
  starts castle-less (its free+instant first castle plausibly reads
  as "both started castled" in memory), and 048 preplants nothing
  for the human.
- SIM: rivals are pool class-3 model-1 entities (slot art rows
  273-279) + a Rival extension record (the Type_160 subset). Landed
  brain: full housekeeping (cooldowns word_90034, hate decay,
  at-castle grace+mailbox discard, shared damage intake w/ shield
  quarter, movement sub_14EB0 verbatim [band settle, always-level,
  jink, accel 16, tempo turn], AI regen rates, spell learning
  200-tick timers, think-gated projectile defense [jink 80 +
  reactive rebound/shield] + heal, altitude clamp), the 9-step
  selector cascade (site scout, flee, upgrade, raid castle, attack
  wizard, intercept balloon, possess [castle-ladder-cost gate — the
  economy loop], mana hunt, home/cruise), state handlers with
  approach bands + speed-up boosts + possess claim (ball f144 +
  chime 4 positional), the cast executor (aim cones, 8-shot burst +
  tempo lockout, debit riding the regen delta, castle-less FREE
  castle plant + ch5-mail upgrades through the castle tick),
  attack picker (poverty latch, 17→8→[anti-rebound 15]→7→20→0→15,
  save-up wait). Mortality: death fall + fire trail + scream 16,
  jar SCATTER (decaying DROPPED_JAR free copies — the corpse-spells
  source for the human), grave + ball re-point, kill tally, hidden
  unhittable husk, castled auto-respawn (tempo-scaled timer, grace
  100, book re-mint, hate truce), castle-less ELIMINATION. Census
  generalized per-owner (rival ceilings; NOTE 2026-07-09: the world
  total does NOT count wizard-carried mana — the census pool walk
  skips class-3 models 0/1 (:56875-78) and only SEEDS u32_188 with
  the caller's intrinsic 1000 (:56867); our brief carried-mana
  addition was the "all HUD bars breathe with my pool" bug, reverted
  same day). Starting castles: pre-leveled, FULL,
  terrain stamped instantly (stamp_castle_terrain), balloons arrive
  via the castle tick's roster (owner-generic — it always was).
- COMBAT: aim_assist targets rival wizards (cloak/hidden 0x20
  respected); m7/m11 wizard-only acquire live (aim_assist_wizards);
  DUEL CHAIN LANDED: (9,7) → (10,26) tether (ctor sprite row 284,
  life 8, follows victim, ch4 200/tick) → victim intake latches the
  CASTER pull (traced formula; APPROX transport: the knock channel,
  clamped 80/tick) until 1000 ticks / 5120 / victim death.
- APP: wizards.json → RivalConfigs → World::set_wizards (restart
  rebuilds rivals); full byte_99B58 TEAM_COLORS[8] map pairs (dots
  for owned creatures/balls/spells by team); castle stamps 58+team
  for EVERY castle (the retail unconditional rule — icons baked all
  8 teams), balloon stamps 66+team gated own || Beyond Sight; v59
  = beyond_sight() now CONSUMED. RivalView API (name/pos/mana/
  life/kills/eliminated) + kill tally + death drain.
- INTERIMS (all flagged in code): rival name labels on the map =
  2x2 team dots under Beyond Sight (retail draws moving NAMES —
  DrawText/font track, banked with the book ROSTER sub_22880 [data
  exposed via rival_views] and the death-message ticker [console
  line for now]); hate feed runs at damage-intake/acquisition
  instead of the per-projectile ledger scan (same inputs, later
  timing; wizard-hit model split folded to +3000); AI castle
  upgrade skips the cosmetic (9,10) ball ride (ch5 token mail
  direct); creature scans still target the HUMAN only (mobs don't
  aggro rivals yet — the wizard-list widening is the next fidelity
  follow-up); AI never re-taught pitch aim vs moving targets beyond
  the commit snapshot (as traced — no lead).
- SMOKE-VALIDATED headless on real bakes (examples/rivalprobe.rs):
  level 002 Vodor roams, claims balls (ceiling 2000→20536), opens
  fire; level 010 both castled rivals hold ~30k economies with
  sustained combat. PLAYTEST CLOSED (player, 2026-07-09, level 010
  won): faithful across the board; anything residual gets spotted
  in the full campaign playthrough.

### PLAYTEST-9 FIRST FIXES (player, 2026-07-09 — same day; LANDED,
### tests green)

Player report: AI aggression "very much correct" (first-contact
validation). Fixes landed:
- RIVAL MANA-BALL COLOR: claimed balls looked unclaimed in 3D (map
  color was right). ball_resize only knew PLAYER_TARGET; now
  Gen::rival_ents[8] (slot→wizard entity, maintained by rival
  spawn/respawn) resolves any wizard owner → family base 105+8·team
  (:29627-32). Claims of an eliminated wizard keep their color
  (property persists).
- RIVAL CASTLE FLAG was white (sprite 177 flat): rival castles now
  wear 177+team (:30809-10 family), both the starting castle and
  the free plant; balloons likewise 169+team (the castle
  dispatcher's `+86 += var_48`, :56347 — was flat 169).
- DEBUG HEALTH BARS (G-instrument) extended to the wizard family:
  rival carpets, castles, balloons all publish life_frac (player
  request — enemy castle HP especially). The human has no billboard
  (first person) — a self bar joins with multiplayer.
- KRAKEN DIES ON LAND — player-suspected missing rule CONFIRMED +
  landed: the mover's same-tile shortcut is FIRST-CANDIDATE-ONLY
  (:21225-30); the ±60°/reverse retries test the v_20 terrain mask
  unconditionally (:21252-91), so a kraken (row 18 = the ONLY
  water-only mask, 0x1) standing on raised ground fails all four at
  the next tile crossing → life = −1. Our move_probe had applied
  the shortcut to every candidate — beached krakens bounced forever
  inside the land tile. Regression test added (beached dies ~200
  ticks, swimming lives); the bare-creature test fixture is now an
  ocean. NOTE the m15 walker has a STRONGER standing check (state
  94 on forbidden ground every 8th tick, sub_20480 :25935-40 —
  already ported in grid_walk); the castle weapon's m6/8/16
  exemption now reads as "the terrain rule gets the kraken anyway".

### PLAYTEST-9 ROUND 2 (player, 2026-07-09 — LANDED, tests green)

- BALLOONS WERE INVULNERABLE (enemy AND own): the castle-tick
  balloon spawner (sub_47400's respawn arm) missed BOTH the ch0
  vulnerability bit (+28 = 1, ctor :44283) — so area writes skipped
  balloons entirely — and the spawn-time link (sub_41CF0 :44284) —
  so a balloon that never left its home tile was invisible to the
  direct-hit cell scans too. Both fixed. ALSO re-traced the balloon
  tick's control flow (sub_47F90 :56717-58): the damage INBOX runs
  at the tick's END, after movement/delivery — and the castle
  delivery pass (:56800-01) heals act_life to FULL on every pass,
  cargo or not. So a balloon parked in its castle's delivery ring
  is AUTHENTICALLY near-invulnerable to chip damage (per-tick full
  heal); they die in flight or to a single lethal burst. Our tick
  reordered to match (inbox last); regression test hits a flying
  balloon.
- THE CYAN VOLUME ON 010 = the class-11 STATE-4 trigger — decoded
  (sub_59B80 :67293-315): NOT an inventory trigger. It is the WIN
  TRIGGER: waits for the human's completion latch (13325 bit 2,
  castle held), fires its disposition, despawns, CONSUMES THE WIN
  (13325 &= 0xFD), sound 41. Campaign levels script the goal with
  it: reaching the share spawns the next stage instead of ending
  the level — 010's disposition unleashes a GENIE (5,11) at (5,68)
  to steal your banked mana back; only re-holding the share with no
  armed win trigger left ends the level. (Also kills the "genie
  needs a ~40+ level" assumption — genies hide behind win
  triggers.) PORTED: trigger_tick state 4 + VolumeKind renamed
  Inventory → WinTrigger; test: the trigger eats the win and
  spawns its stage. Player note ("effectively finished 010"): with
  the trigger live, 010's completion now requires beating the
  genie's theft — re-playtest.

### PLAYTEST-9 ROUND 3 — bind gate correction (player, 2026-07-09)

Retail lets you ASSIGN quickselect/hands to owned spells whose
castle_req isn't met (quickbar = campaign state, routinely bound to
not-yet-castable spells — player retail memory, which is senior; and
the equip command :48717-31 checks ownership only). Our book UI had
promoted the :26926 castle-stored check from a VISUAL (the LOCKED
fog wash + equipped-panel wash, both kept) into an input gate —
hovered cells only became bind candidates when castle-qualified.
FIXED: bind candidacy = ownership; the cast keeps fizzling sim-side
(buzz 29) until the castle stores enough. LoadoutView.bindable now
feeds visuals only.

### PLAYTEST-9 ROUND 4 — HUD castle-mana bar (player; pre-existing,
### LANDED)

- "Castle mana permanently full" = a DISPLAY double-count: slot A's
  white fill drew `stored + banked` where banked ALREADY = houses
  u32_308 + castle stored (:27240-66 verbatim: capacity bar =
  castle +136 / world in v27, fill bar = BANKED / world in v29 —
  nothing adds stored twice). Sim-side stored was always honest
  (fresh castle 0, grows by absorb/balloon delivery, ejects
  overflow). Also ported the missing full-state: banked == capacity
  draws ONE bar blinking between the pair's colors (:27242-53).
- "Empty bars yellow-ish, should be grey-ish": the capacity bars'
  CAP_AMBER was our functional-first invention — retail v27 =
  byte_99B58[1+2·team], the SAME index as the spell meter's v26 the
  player already certified GREY (2026-07-07). Capacity bars (slots
  A and C) now use the meter grey; CAP_AMBER deleted.

### PLAYTEST-9 BANKED ITEMS (player, 2026-07-09 — verified where
### cheap, all UNIMPLEMENTED pending a future session)

1. CRAB REGEN — CHECKED, AUTHENTIC, no action: m5's wander AND chase
   tails regen `maxLife >> 7` PER TICK (:22959-65/:22976-82, ours
   verbatim in m5_regen) — unlike everyone else's v_26-gated
   `maxLife >> 6`. A grown crab (max 40000) regens ~312/tick;
   "exceptionally hard to kill without rapid fireball or meteor" is
   the designed experience.
2. CASTLE-SPELL METER COST: the equipped-panel progress bar for
   spell 16 uses the ctor's 1000 — retail rewrites the spell
   entity's +136 to the CAPACITY LADDER at the castle's current
   level on every init/level-up (sub_47DD0 :56617), so the meter
   tracks the NEXT upgrade's cost, ballooning per level; at level 7
   the cost is 30,000,000 → the bar reads forever-empty (mind the
   div-by-0 when porting — expose the dynamic cost through
   LoadoutView like the cast path already computes it).
3. "ENEMY LIGHTNING DETONATES MY METEOR AND TURNS IT HOSTILE"
   (player memory, game-of-origin uncertain): MC1 findings — class-9
   projectiles are NOT mail-damageable (every ctor clears +16 bit 8,
   :45925-46150 family), so no lightning-damages-meteor path exists
   in this engine. MC1's only projectile-conversion machinery = the
   DEFLECTION blocks (:62735-49/:62870-86/:63138-50): owner +24 →
   deflector's team, retarget +146 = deflector, life refill,
   jittered return heading — and GRIFFONS natively carry the
   rebound bit (0x8000, the playtest-8 trace), krakens' beams
   trigger deflectors too. So the memory is either the griffon/
   kraken deflection read as "lightning blew it up mid-flight"
   (deflected meteors DO come back hostile), or an MC2 behavior —
   check reference/remc2 when its combat track opens. STATUS
   2026-07-09: stays banked; player will attempt a retail MC1
   repro to settle which.

### Deliberate-looking AI asymmetries (retail-check register —
### deferred by agreement 2026-07-09 to the full-campaign testing
### pass, not a standalone check)

AI heals 4× afield/1.25× home; AI at-castle discards damage (no
castle forward); AI first castle free+instant; AI ignores walls,
drag and knockback; AI omniscient scans (LOS only as tactical
filter); AI learns spells by timer instead of pickup; AI auto-
respawns while castled. remc1 SUSPECTS: hate write keyed +63 vs
+65; the win-gate 0x100 bit; spell-22 castle-req constant corrupted
((int)&loc_30D40 — needs binary); word_90034[8]=400 alignment;
sub_16990's cached human pointer unread (dead).

## PLAYTEST-10 — CERTIFICATION SWEEP (player, 2026-07-09)

One session closed most of the owed playtests and answered several
retail-check questions. The one-line ledger (details edited into
each owning section):

**Certified faithful (closed):** rival wizards (level 010 WON;
aggro, brains, corpse mana un-owning + possess-to-claim — full
campaign playthrough is the residual-deviation net); wyvern +
griffon (thorough); bee triple lunge; mortality + castle feel;
HUD + map; marching-ants endpoint (castle, as ported); audio ("as
good as it can be without unfaithful changes"); cast cadence (with
the tick-rate design note below).

**Held open deliberately:** autoaim — observed working, but final
judgment waits for the planned dev feature of CROSSHAIR TARGETING
PREDICTORS to really see the behavior; genie (needs a ~40+ level);
lightning-detonates-meteor (player will attempt retail repro —
might be MC2-only); AI asymmetry register (folded into the
full-campaign testing pass).

**Answers that changed the work queue:**
1. TWO-CAST CASTLE — narrowed to interfering-dwelling cases at ANY
   level, but the mechanism (explicit raze gate vs emergent
   dwelling-explosion downgrading the fresh castle) is deliberately
   unadjudicated; player will provide a specific retail-tested case.
   No port change until then (mortality section item 5).
2. HUMAN STARTING CASTLES — retail DOES preplant them on some
   levels → wiring owed for slot 0 on idx 005/011/015/018/035.
   048 data check: two rivals, only slot 1 preplants (level 7);
   slot 2 and the human start castle-less (hostile-wizards section).
3. TICK RATE IS A DESIGN DECISION, NOT A PORT: retail locked ticks
   to fps, so cadence/time rate varied with resolution (higher res
   = slower game; rapid clicks could match rapid-fireball cadence).
   No faithful target exists — pick canonical constants, likely
   from MC2, when the timing pass happens.

**New banked item:** NON-4:3 ASPECT STRETCHING — presentation
stretches at non-4:3 resolutions; its own dedicated investigation
later (render/UI track, not a sim issue).

## HOUSEKEEPING TRACK — LANDED 2026-07-09 (same day as PLAYTEST-10)

Three infrastructure chores toward opening playtesting up (player
directive; all landed, workspace green, 160 tests):

1. **Bake epoch + auto-bake**: `mgc_formats::BAKE_EPOCH` (content
   epoch, orthogonal to the schema versions — bump on output changes
   under an unchanged schema) stamped into every `meta.json` and
   `bundle.json`; pre-epoch artifacts deserialize as 0 = always
   stale. The game shell (`mgc-app/src/bakecheck.rs`) checks the
   requested level + all bundle stamps at startup and reruns the
   FULL bake when anything is missing/stale — `mgc-import` is now
   linked into the game (embedded importer, user-chosen), and the
   orchestration moved from the CLI into
   `mgc_import::bake::bake_all` (one shared path; the CLI is a thin
   wrapper). Gamedata located via config `gamedata` → `MGC_GAMEDATA`
   → `gamedata/`. Tested end-to-end: stale epoch-0 tree
   auto-rebaked (415 packages) and loaded; second run no-op.
   FORMAT.md updated in lockstep (rule 3 = the epoch rule).
2. **Release pipeline**: `.github/workflows/release.yml` — tag `v*`
   (or manual dry run) → linux x86_64 (ubuntu-22.04 for old glibc) +
   windows msvc builds of `mgcarpet` + `mgc-import`, archived with
   README/LICENSE, attached to a GitHub Release
   (softprops/action-gh-release). Repo will be PRIVATE first;
   the user creates/pushes the remote. ci.yml + release.yml both
   gained the Linux ALSA-headers step (cpal → alsa-sys needs
   libasound2-dev; the old ci.yml would have failed its first real
   ubuntu run). README rewritten: playtester quickstart
   (release binary + GOG data + auto-bake), build deps, honest
   Status. Also fixed pre-existing CI-gate breakers: cargo fmt
   drift across recent sessions + ~70 clippy findings; deleted the
   accidentally-committed stray duplicate
   `crates/mgc-app/src/crates/` (was a mis-pathed rivalprobe.rs
   copy from the rival session — deletion left for the user's git).
   NEW LINT POLICY (root Cargo.toml `[workspace.lints.clippy]`, all
   crates inherit): the "rewrite into idiomatic Rust" style lints
   that would erase trace-shape correspondence are allowed
   workspace-wide (collapsible_if, manual_is_multiple_of,
   unnecessary_cast, manual_range_patterns/contains,
   nonminimal_bool, needless_range_loop, int_plus_one,
   type_complexity, blocks_in_conditions, too_many_arguments) —
   keeping ported code auditable against the decompile is a
   feature; everything else stays deny-in-CI and was fixed. Two
   real finds while clearing: combat.rs aim-scan had a dead
   `(score == bs && false)` tie-break arm (simplified, semantics
   identical — strictly-less = earlier slot wins on ties, the
   original's scan order), and spell_castle.rs's clear-spot scan
   had a latent label bug (`continue 'outer` skipped the whole ROW,
   so only the first column of candidates was ever tested — worked
   by luck; proper two-level scan restored).
1b. **Level shorthand** (player follow-up, same day): `--level
   <game>:<index>` (`mc1:32`, `mc1hw:7`, `mc2:100`) resolves to
   `baked/<game>/level-NNN.mgcl` — dissolves the first-launch
   catch-22 (can't tab-complete a level file that only exists AFTER
   the launch bakes it). Unknown tag + numeric index fails fast
   (typos must not trigger a wasted full bake); anything else falls
   through as a path (Windows `C:` prefixes safe). Raw paths remain
   for dev use; long-term the flag demotes to dev/debug once
   campaign progression + the level browser exist (player: "you
   should not be launching specific levels selectively").
   ROOT-INFERENCE GUARD (player-caught, same day): the pre-fix
   `mc3:5` test run inferred baked root `.` from the bare path and
   SPRAYED A FULL BAKE INTO THE REPO ROOT (mc1/ mc1hw/ mc2/ assets/
   manifest.sha256 — 325MB, cleaned). ensure_baked now only infers
   a root when the level's parent dir is named mc1|mc1hw|mc2; a
   nonconventional path that EXISTS loads as-is (custom packages,
   no auto-bake), a missing one errors with a pointer to the
   shorthand instead of baking into a guessed directory.
3. **docs/FIDELITY.md started** — the porting record (what is true
   now) vs the roadmap (how we got here): entry format = Original /
   Port / Verified / Options / Deviations-&-interims + a 5-grade
   verification ladder (decompile-traced → oracle-diffed →
   player-validated → player-certified → retail-verified; recorded
   original gameplay outranks the decompile) + the P/G option-class
   legend. First two entries written for format review: terrain
   generation, player flight. Remaining subsystems queued in the
   file's tail note; fill in follow-up sessions.

## AUTOAIM CROSSHAIR — LANDED 2026-07-09 (the playtest-8 banked
## predictor instrument; design agreed with the player same day;
## PLAYTEST OWED — this is the tool gating the autoaim closure)

P-class `enhancements.crosshair` (config + `--crosshair` +
runtime C toggle, default OFF; the original shows no aim UI).
Agreed design = ONE neutral cross + per-hand lock markers (the
player's two-full-crosshairs sketch refined: both hands share the
aim vector, so idle glyphs would just stack):

- **Neutral cross**: white-edged black `+` at the TRUE aim point —
  the faithful camera pitches at HALF the aim pitch, so aim is never
  screen center (drawn at 20 tiles = the 5120 acquire range along
  the full-pitch ray from the eye; unaffected by the kraken-drag
  camera kick, which is the instrument working).
- **Lock markers**: on the target each hand's EQUIPPED spell would
  acquire this instant — LEFT = upright `+`, RIGHT = diagonal `×`
  (shape-coded, not shade-coded: survives blink + busy backdrops,
  and the shapes compose to an 8-point star when both hands lock the
  same target), cores gently blinking red (sin over sim ticks).
- **The pure scan**: `Gen::aim_preview_scan` (combat.rs) = read-only
  twin of the aim_assist family — identical filters/cone(±0x71)/
  range(≤5120)/min-score, NO writes, NO player_danger arming, NO LCG
  (purity compiler-guaranteed by &self). `World::aim_preview(pose)`
  keys per hand: {0,23 fireballs, 7 meteor, 8 volcano, 15 lightning}
  → creature set; {3 possess} → balls/houses; {11 duel, 13 steal,
  17 undead} → wizard-only; others → None. Runs from the real
  muzzle pose incl. the volcano's +0x60 down-arc pitch bias.
- **Plumbing**: `mgc_render::world_to_screen` (pub — the same
  wrap+matrix as the world pass; also the future name-label hook),
  `ui::crosshair_quads` glyphs (UiQuad solids; × = chunky diagonal
  squares, axis-aligned quads only). Not wired into the
  --screenshot path (windowed play only).
- Tests: sim hand-keying + cone octant sweep; render projection
  (center/behind/wrap seam). HONEST-INSTRUMENT CAVEAT for the
  playtest: acquisition ≠ hit — homing yaw is capped 5/tick, the
  marker shows what the shot will CHASE (fast crossers still
  evade); and the first-tick lock undersells later in-flight
  re-acquires (fireball/meteor/volcano re-scan per tick in flight).

## MULTI-GAME ARCHITECTURE — AGREED DESIGN + PLAN (2026-07-09)

Resolves the NEXT-SESSION AGENDA below (kept for provenance).
Decided in discussion with the player; work runs on a separate mc2
branch (player handles git) so there is a clean way out on failure.

**Core decision: ONE sim, not parallel sims.** remc2 is the same
codebase evolved, and both engines are already table-dispatched
(`dword_96902` class dispatch, `off_97D12` spawn creators,
`off_987DE` spell thunks, `str_256038` trigger states) — the plugin
seam is the original's own architecture, promoted, not an invented
abstraction. MC1 and MC2 conjoin over a shared chassis; HW is a
delta inside MC1 content (retail's own sibling-binary model), not a
plugin.

**The five-tier divergence taxonomy** (what "a feature" is):

1. **Chassis parameters** — pool sizes, rand constants, tick
   constants. Each game defines a pristine set; overridable =
   the LIMIT-REMOVING option category (G-class matrix rows;
   "faithful" resolves per profile). Must be runtime values, not
   consts, from day one.
2. **Tuning tables** — behavior rows, sprite stats, spell stats,
   economy ladders, LUTs. Extracted from the BINARIES via tooling
   (extract-remc1-tables.py; extend to remc2), NOT from game data
   files. Freely mixable in principle.
3. **Dispatch wiring** — (class, model, state) → handler tables.
   Tabular by nature; THE mixing surface.
4. **Dispatched handlers** — per-model spawn/AI/effect routines;
   the bulk of the code mass (~90% of sim lines; pure tables ≈ 8%),
   but swappable per-row through tier 3. RULE: no handler ever
   contains `if mc2` — a routine that differs is TWO handlers, each
   a verbatim port auditable 1:1 against its decompile; variation
   lives exclusively in the wiring.
5. **Global engine verbs** — NOT row-dispatched, woven into the
   tick: tick orchestration/ordering, targeting/autoaim scan,
   awake/sight/perception, damage application (sub_46B00) +
   mailbox protocol, mana census/regen/debit loop, LOS/height
   sampling, wall gate + movement commit, the movement-core state
   interpreter, player flight. Swap WHOLESALE as family columns
   (flight.rs's enum-selected models = the existing template).
   The genuinely hard set; the ones MC2 actually rewrote.

**Cross-imports** (MC2 features in MC1, "improved" replacements) =
authenticity-matrix columns over tiers 3-5. Prerequisite: unified
ID registries (spell/sound/model superset enums with per-game
subsets — MC1 spell 5 ≠ MC2 spell 5). Families interact through
chassis "currencies" (damage, mana, pose, ownership); combinations
that reach deeper may be declared UNSUPPORTED, deliberately.

**Graceful degradation is a requirement of the seam:** unknown
(class, model) → inert placeholder billboard + log line, never a
crash, never silence. Every MC2/HW level stays loadable from day
one; porting progress is visible as placeholders become creatures.

**Verification:** state-hash fixtures per profile (world-state hash
over N ticks; the replay-fixture stand-in until replays exist). The
MC1-faithful profile must remain bit-identical through every phase.

**Limit-removing register** (chassis-parameter deviations, the
source-port classic): pool exhaustion already bites — map 032
trigger starvation (kill-shortfall kept slots full), map 039 walls
exceed the pool at load. Path: (a) instrument the faithful pool
with exhaustion telemetry FIRST (zero deviation, builds the
catalogue in playtests), (b) CHECK retail's exhaustion behavior —
fail-open silent drop is likely, and 039 "fails to load" in OUR
engine may be our error path where retail shrugged (the faithful
fix), (c) then the bumped pool as the opt-in improvement. Caveat
traced: dropped spawns carry mana, so lifting the cap MOVES the
win-check %-goal on affected levels — bigger pool = same world plus
retail's dropped entities, bit-identical up to first exhaustion.

**Phases** (0+1 targeted for the 2026-07-09 session):

- **Phase 0 — remc2 survey** (read-only, falsifiable). Deliver:
  (a) the ChassisParams diff (pool/allocator, rand, entity struct,
  mailboxes, tick ordering), (b) the tier-5 verb inventory with a
  verdict per verb: identical | parameterized | rewritten.
  KILL CRITERION: chassis diff doesn't fit a page, or most verbs
  rewritten-with-different-structure → superset bet is wrong, fall
  back to parallel sims. Survey sources: reference/remc2/remc2/
  engine/ (Events.cpp = pool/dispatch, EventsFunctions.cpp 63k =
  handler mass, Basic*/engine_support = math/LOS).
  **LANDED 2026-07-09 — KILL CRITERION PASSED DECISIVELY** (full
  report: docs/SURVEY-MC2.md). MC2 demonstrably reuses MC1's
  chassis: same LCG constants (9377/9439), same two-stack allocator
  (both originals have the reclaim second stack our port omits —
  now a chassis item to grow once), byte-identical 6-channel
  mailbox protocol, identical tile chains and disposition/trigger
  machinery incl. the 10-tick debounce; ~16 chassis params, half
  evaporating in native Rust. Of ten verbs, ONE is genuinely
  REWRITTEN (the player movement-commit gate: MC1 type-8 walls vs
  MC2 water/blocked-flag/cave-steer — and MC2's gate also ZEROES
  target speed on block, which MC1 never did); the rest IDENTICAL
  or PARAMETERIZED, often to the constant (flight filter/rates/
  speed ±16/80 all match; behavior-row schema byte-identical +2
  trailing flag bytes; MC2 model-0 row ≡ our BEHAVIOR[0] exactly).
  Gifts: MC2's CHASE takes the attack thunk as a function-pointer
  PARAMETER (Bullfrog themselves moved toward our tier-3/4 design);
  the spell-XP system = decorators on events our combat already
  emits (+1 in the damage intake for shield, per-hit for fireball,
  tiles-altered for terrain spells); PACK-FOLLOW retains retail
  MC1's catch-up line verbatim (:9482 — the 2026-07-05 mis-fix
  ruling RE-CONFIRMED a third time, now cross-engine).
  SURVEY BYCATCH, FIXED same day: our awake pre-pass used a 3D
  distance — sub_42410 (:52748, SYNCHRONIZED) is 2D (x/y only,
  altitude never gates waking); remc2's EuclideanDistXY agrees.
  Fixed in mobs.rs; state-hash goldens unchanged (fixture flies
  low). Retail-check register additions: MC2 homing-cap table
  values (data, not code), the shield full-absorb flag transition,
  MC2 cast-cost scaling per spell level.
- **Phase 1 — MC1 verb extraction**, zero behavior change: land
  the state-hash fixture test BEFORE touching anything, then carve
  tier-5 verbs out of world.rs/mobs.rs/combat.rs into a verbs
  module behind the Phase-0-shaped boundary; the mgc_sim::mc1::*
  namespacing move rides along (fixes the misleading unprefixed
  names). Existing tests + new fixtures stay green with identical
  hashes.
  **LANDED 2026-07-09** (same session as Phase 0), with one scope
  decision: the PHYSICAL verb carving folds into Phase 2 — the
  survey showed most verbs are shared-skeleton-plus-params, so
  moving their code twice (once into an MC1-only module, again
  behind the Phase-2 interface) would be waste; Phase 2 defines the
  interface from both inventories and carves once. What landed:
  - **State-hash fixture** (mgc-sim/tests/state_hash.rs):
    World::state_hash() digests the FULL persistent state (pool
    internals, LCG streams, mailboxes; stable FNV-1a; full
    destructures make new fields a compile error), 6 golden
    checkpoints over a scripted level-005 run with rivals +
    two-hand fireball combat; determinism double-run asserted.
    Goldens pin the PORT's behavior (re-pin only on deliberate
    change, say so in the commit).
  - **ChassisParams** (mgc-sim/src/chassis.rs, player-endorsed
    mid-session): the tier-1 table as code — level_table_slots,
    pool_slots, ent_rand_width (U32|U16), bucket_models,
    bucket_excluded_states, win_streak_ticks — with pristine MC1 /
    MC2 (survey values) sets; Gen stores a set at construction,
    World::new = MC1, World::new_with_chassis = explicit; every
    POOL/TABLE_SLOTS const use became len()-driven. Test-proven:
    bumped pool 2000 = observably identical on 005 (the
    limit-removing transparency property).
  - **Pool-exhaustion telemetry**: new_event still fails open like
    retail but counts drops (Gen.exhausted → World::
    take_pool_exhausted → app stdout ERROR line + per-level running
    total). The 032/039 catalogue builds itself from playtests.
    Retail-check still owed: what retail 039 actually looks like
    (fail-open holes?).
    CATALOGUE ENTRY #1 (player, same day): **level 039 drops 680
    allocations at load** — the authored feature set alone blows the
    1000-slot pool, confirming the fail-open theory. `--pool-slots N`
    dev CLI flag added (G-class limit-removing override, threads a
    deviating ChassisParams through WorldInit; announces itself on
    stdout; 2..=60000, slots are u16).
    PLAYER-VERIFIED same day: **039 comes up UNCRIPPLED under
    `--pool-slots 2000`** — the first limit-removing win, plausibly
    the first time the level has ever existed in full. Follow-ups
    banked: retail 039 comparison (where are retail's fail-open
    holes? → FIDELITY.md entry), promote pool_slots to a proper
    config option in the limit-removing register once the catalogue
    settles, note slots ≥1000 have no retail-faithful f63 rhythm
    (idx-as-u8 truncation — benign, documented).
  - **AWAKE 2D FIX** (survey bycatch, see Phase 0 entry).
  - **mgc_sim::mc1::\*** namespacing: world/mobs/combat/rivals/
    spells/features/tables + behavior/entities/sprite_stats/corners
    (ex mc1_*) now live under mc1/; chassis + flight (the seam
    template) stay top-level; all consumers updated. Goldens came
    through UNCHANGED — the move proven behavior-transparent.
- **Phase 2 — superset interface**, defined from BOTH inventories:
  enum-dispatched static impls (replay-recordable, flight.rs
  precedent), minimal unified registries. EXIT CHECKPOINT: the
  frankenstein smoke test — MC1 content pushed through the seam at
  an MC2 level; expect mostly placeholders + a few misfit spawns,
  no crash. Disposable scaffolding proving the wiring.
  **LANDED 2026-07-09** (same day as Phases 0-1); the frankenstein
  checkpoint PASSES on real data. What landed:
  - **mgc_sim::verbs** (the tier-3 wiring surface): one enum per
    tier-5 verb — Awake/Movement (the whole class-5 handler family)/
    Targeting/Damage/Objective/Corpse/CommitGate/Flight — bundled in
    a `VerbSet` Gen takes at construction (like ChassisParams) and
    never rebranches on outside the dispatch seams. Every MC2 arm is
    DECLARED-PENDING: it serves the MC1 implementation and notes the
    fallback (`World::verb_fallbacks`, once per verb). Phase 3 lands
    real arms by editing exactly these matches. Two inventory rows
    carry no enum by design: tick orchestration (variance = chassis
    data + a future pre-pass hook list) and LOS/height (variance =
    DATA — the cave ceiling plane joins Planes when the MC2 cave
    slice lands). The dispatch sites: awake pre-pass + class-5 arm +
    player intake + objective check in World::tick, the win check
    carved out of tick into `objective_mc1` (the ObjectiveVerb
    seam), aim_assist/_wizards/_possess + corpse_drop dispatchers in
    combat.rs (bodies renamed `*_mc1`), the wall-gate pair in
    World (commit-gate; fallback noted at the sim boundary in
    lib.rs, where &mut exists, alongside FlightVerb).
  - **mgc_sim::ids**: `GameId {Mc1, Mc1Hw, Mc2}` + pristine profile
    selection (`chassis()`, `verbs()`, `known_thing()`), the
    `From<mgc_formats::Game>` boundary map, and THE KEYING RULE:
    anything crossing games keys by (GameId, local id) — MC1 spell 5
    ≠ MC2 spell 5. `World::new_for_game` = the profile constructor
    (new/new_with_chassis stay, MC1 semantics).
  - **The spawn seam's graceful degradation**: `mc1::known_thing` =
    the MC1 registry column (derived from the spawn guards;
    authentic no-spawns like start markers and null creators are
    KNOWN non-entities); `spawn_from_thing` consults it — unknown
    (class, model) → misfit ledger (`World::misfits`, app logs WARN
    once per entry) + optional marker-stone placeholder billboard
    (`set_placeholders`, OFF by default — faithful worlds drop
    unknowns silently like retail).
  - **Chassis-truth fixes the frankenstein flushed out**: the
    load-time feature scan and disposition scan were hard-coded
    `1..2000` (OOB panic on MC2's 1200-slot table; also meant MC1's
    2096-slot community headroom never fired) — now table-len
    driven; walk_chain's `% TABLE_SLOTS` const → table len (const
    deleted). REAL CRASH FOUND: cyclic parent/child links (MC2
    reuses those fields as context params) livelocked walk_chain —
    4 billion failed allocations overflowed the exhaustion counter.
    Fixed with a table-len hop cap (unreachable on well-formed
    data — a malformed community MC1 level would hang RETAIL the
    same way) + saturating telemetry counter.
  - **tests/frankenstein.rs** (the exit checkpoint): real
    baked/mc2/level-000 (311 things, full oracle terrain) under the
    MC2 profile with mc1-temperate stand-in assets. Sweeps + a
    census pass (debug_fire_disposition instrument) → deterministic
    state stream, 129 live things (MC1 misfit-spawns + placeholder
    stones), misfit ledger truthful ((5,19)x6, (14,3/5), (15,2/3),
    (11,32)x4, the (10,29) chain-cycle note), five verb seams noting
    MC1 fallbacks. Self-skips without baked mc2 data.
    KNOWN COLLISION banked for Phase 3: MC2's class-0 Conditional
    Spawn collides with the MC1 table's class-0 EMPTY-SLOT SENTINEL
    — invisible to the disposition scan; the MC2 spawn column must
    key emptiness differently.
  - Goldens: ONE deliberate re-pin (VerbSet/GameId/telemetry joined
    the hashed layout; fields unread by handlers), then every carve
    landed with bit-identical hashes.
- **Phase 3 — MC2 vertical slice: a living level-000.**
  SPEC SHARPENED 2026-07-09 by an Opus adversarial review against
  the remc2 source (all citations = reference/remc2 engine files);
  the review's data checks are already done: level-000 is map_type
  NIGHT (cave machinery genuinely deferrable) and its checkpoint
  table is [type 5 fly-to (115,212), type 5 fly-to (194,213),
  type 7 kill-THING slot 103, type 0 mana ≥ 15%, type 7 kill-THING
  slot 189] — kill objectives present, so the objective arm is NOT
  stubbable (see 3.4).
  GOAL: mc2/level-000 runs as a live World — MC2-native THING
  decode, three slice creature models acting through MC2 arms, the
  real MC2 commit gate, a real single-stage objective, its own
  golden fixture. Placeholders stay legitimate for everything
  unported. NON-GOALS (Phase 4): spell-XP, multi-stage progression
  + class-0 conditional-spawn machinery, cave levels, full roster,
  multipart chains, MC2 flight model, rivals on MC2, MC2 HUD.
  Standing rules: no `if mc2` in a handler; MC1 goldens NEVER move;
  faithful default + FIDELITY.md for deviations; BAKE_EPOCH on any
  bake change.
  Work items, landing order:
  - **3.1 MC2 THING decode.** Superset Rec + an mc2 table build
    honoring MC2 field semantics (swi_id = stage tag, parent/child
    = context params, par3). THREE record states, not one (review
    Q1/Q2, remc2 :32982/:32991, Events.cpp:164-275): class==0 at
    runtime = CONSUMED sentinel (but class-0 ON DISK = Conditional
    Spawn content — the MC2 column keys emptiness on its own
    consumed bit, never class); DisId==-1 = spawn-at-load;
    DisId>=0 = disposition-gated (the consumer IS the MC1-shaped
    disposition scan, sub_4A1E0 :32950, widened by a stage-var
    pre-pass the slice models don't invoke).
  - **3.2 MC2 data extraction** (hard gate for 3.3/3.4 — the awake
    AND movement arms read the same behavior-row flags byte; one
    shared extracted table or silently inconsistent creatures).
    Behavior rows for the slice models incl. the +2 trailing flag
    bytes (bit 1 die-on-water :8855, bit 4 pack-disable :9022,
    bit 8 flee/alt-chase :9003); sprite stats/rows for billboards;
    homing caps as data (retail-check closes here). ASSET
    EXTRACTION (review Q3 — these exist ONLY inside game.gog, which
    iso.rs already reads): SEARCH.DAT (same 1024-byte ring format
    as MC1), BLDGPRM.DAT (77 x 4-byte records — a NEW format, not
    MC1's BUILD/TAB; loader :38319), BLOCK16/32.DAT; enable the
    bundle.rs search/build emit path for MC2 variants + a new
    bldgprm member; FORMAT.md + BAKE_EPOCH bump.
  - **3.3 The slice creatures = (5,1) Vulture + (5,4) Archers +
    (5,13) Villager** (review Q4, ctors :33720/:33878/:34037):
    single-part, fixed actionIndex, population-dominant (33+28 of
    ~75 authored). The draft's (5,3) and (5,19) are CUT as traps —
    (5,3) is a 16-segment multipart worm in bucket-excluded state
    0xE8 (:33836), (5,19)'s ctor spawns a CLASS-9 flyer (:34882);
    both violate the kill criterion below. FLEE (sub_1C980 :9572)
    is a shared primitive gated per-model by flags bit 8 — port
    only if a slice row sets it, else Phase 4.
  - **3.4 Real MC2 verb arms**: awake (flag-byte keying) + movement
    core (widened row, thunk-as-param) for the slice models;
    targeting (extended subtype key + homing caps; model-78
    pre-acquire/buildings source deferred unless a slice model
    needs them); commit gate (water/blocked-flag + ZERO target
    speed on block :59602; cave-steer inert — level-000 is night);
    corpse = creature death → mana spheres via the shared pipeline
    (spell-token scatter is WIZARD-death material :sub_5E310 —
    Phase 4); **objective = the REAL minimal single-stage engine**
    (NOT a stub — level-000 has kill objectives): InitStages
    checkpoint load (:40567), the sub_58DA0 per-spawn stage binding
    (:40650 — needs an explicit hook at the spawn seam, no `if
    mc2`), win check for types 0/5/7 (:40693-40812). FLIGHT STAYS
    PENDING (review Q7/Q8: MC2 climb law = row-driven linear ramp
    :59645, different formula, + player-extension fields MC1 lacks
    :59610-99 — a real port, moved to Phase 4; the player flies the
    MC1 arm over the slice, which is fine).
  - **3.5 App wiring**: lift the `!= Game::MagicCarpet2` world
    gate when terrain + 3.2 members exist; MC2 billboards/map dots;
    placeholders ON by default for MC2 until the roster closes.
  - **3.6 Fixture + EXIT CHECKPOINT** (tightened per review — every
    criterion must be EXERCISED, not vacuously green):
    state_hash_mc2 goldens over a scripted level-000 run +
    double-run determinism; MC1 goldens untouched; and on the
    scripted run: (a) zero crashes; (b) each ported seam POSITIVELY
    served — fire at a slice creature (targeting ran), fly into
    water/blocked (assert target speed zeroes next tick), kill a
    slice creature (assert the mana-sphere drop entity appears) —
    ledger-empty alone is necessary, not sufficient; (c) no misfit
    entries for the slice models; (d) per-model behavior asserts —
    Vulture wakes/chases/attacks/dies, Archers wakes + fires
    STATIONARY (maxSpeed 0 — do not assert chase), Villager wakes +
    wanders/flees (townsfolk, excluded from kill-count — do not
    assert attack); (e) the objective arm latches a type-5 fly-to
    at (115,212) and resolves the type-7 kill targets (slots
    103/189) through the spawn binding; (f) a playtest build boots
    level-000 with the slice creatures visibly acting.
    KILL CRITERION (scope alarm): a slice model dragging in >2
    unported subsystems gets SWAPPED for a simpler single-part
    model, never widens the slice.
  **SIM CORE LANDED 2026-07-09** (same day as Phases 0-2; 3.5 app
  wiring + exit item (f) = the next session's opener). Ledger:
  - 3.1/3.2 as specced: bundles carry search.bin + bldgprm.bin
    (BAKE_EPOCH 2, rebaked, MC1 goldens survived);
    tools/extract-remc2-tables.py → mgc_sim::mc2::{behavior (157
    rows, ABSOLUTE indexing), sprite_params (347)}; anchor tests pin
    MC2[59] ≡ MC1[0] and the slice rows' flags. FLEE resolved
    IN-scope (Vulture + Villager rows set bit 8; Archers don't).
    TABLE-BASE FIX: MC1's 1999-record file = engine slots 1..=1999
    (+1), MC2's 1200-record file IS the engine table (base 0 — the
    type-7 checkpoint targets index it directly).
  - 3.3 mgc_sim::mc2::mobs (~1100 lines, verbatim-cited): the three
    ctors (RNG draw order preserved incl. the villager's %9 sprite
    pick), shared primitives (move core w/ the verbatim retry-yaw
    byte quirks + die-on-water suicide, patrol/idle two-scan,
    pack-follow w/ leader hand-off, FLEE w/ the HIBYTE+4 flip,
    chase-attack, prekill/kill w/ the model-{9,12,13,14,15}
    kill-credit exclusion, mana-sphere transform), the archer brain
    (wanted-timer gate → player_aggro, shrine consumption) + arrow
    fire (danger → player_danger), the townie brain, the (9,13)
    arrow ctor+flight, the MC2 awake pass (propagate-then-decrement
    + f59 wake delay). APPROX register (module doc): slot-order pool
    scans for retail's slot-ordered lists; arrow probe = the MC1
    victim scan; arrow impact = ch0 area-write pending sub_10C80;
    spheres fly via the MC1 (10,39) ball ctor; sub_20130 missing
    from the decompile (unreachable: archers never flee).
  - 3.4 verb arms LIVE: awake/movement/objective/commit-effective;
    class-9 targeting = arrow native + MC1 spells through the
    fallback (deliberate: the player keeps fireballs until MC2
    spells land). Objective = Mc2Stages (InitStages registration w/
    the type-7 MODEL resolution, per-tick types 0/5/7, cursor
    advance, IsLevelEnd → completed; hashed only when populated so
    MC1 goldens held with ZERO re-pins this whole phase). Init =
    the GenerateEvents passes (A-E verbatim filters, F+G merged
    until bldgprm threads through) + shared disposition 0.
  - CROSS-COLUMN DAMAGE CONTRACT (the fixture flushed it out): MC2
    carries NO per-channel vulnerability mask — its one damage gate
    byte[0]&8 ≡ MC1 flags&8 (same NewEvent default bit, chassis
    again); MC1 writers additionally demand the +28 channel mask, so
    MC2 ctors set f28=1 at the seam. Hitboxes: MC2 puts extents on
    PROJECTILE quads (every class-9 ctor ShiftRots); creatures like
    the vulture are faithful ZERO-extent targets — area/fire damage
    is the kill path, direct hits need the projectile's own quad.
    Arrows clear their hittable bit (:35038).
  - 3.6 fixture GREEN (tests/mc2_slice.rs): deterministic goldens
    (pinned) + positive per-model asserts — vulture wakes→FLEES(14),
    archer killed by fireball → kill credit + mana sphere, villager
    kills EXCLUDED from credit, archer FIRES an arrow at the wanted
    wizard + danger music arms, type-5 fly-to latches at (115,212) +
    cursor advances + no premature win. Frankenstein updated to the
    shrunken-fallback reality (only damage + player-spell targeting
    fall back). KNOWN GAP: level-000's villagers are authored on
    tiles whose walkable paint comes from the unported BUILDING
    creators → terrain-imprisoned (wander asserted via heading);
    unblocks with (10,45). BYCATCH: MC2 night maps are archipelagos
    — type 0 IS water (MC1 fireballs splash on it; the villager
    prison + die-on-water mask corroborate).
  - **3.5 APP WIRING LANDED 2026-07-09** (exit (f) boots; player
    playtest owed). Ledger:
    - World gate lifted: `load_level` builds every game through the
      game-aware `WorldInit` (new field set: `game`, `stages`,
      `placeholders`; `World::new_full` made pub — explicit chassis
      + game, so `--pool-slots` composes with MC2). MC1/HW path
      byte-identical (HW now truthfully passes `Mc1Hw`; the sim
      treats Mc1|Mc1Hw identically everywhere — verified). MC2:
      `set_placeholders(true)` (roster open) + `set_mc2_stages` from
      the package checkpoints; feature assets = the mc1-temperate
      stand-in (the MC2 arms never touch them — the slice fixture's
      arrangement); win_pct/wizards stay MC1-column. Boot telemetry:
      verb-fallback list + misfit ledger + billboard/pose counts.
    - THE MC2 BILLBOARD SIZE LAW (research agent vs remc2, then
      source-verified): `rot_speed_8` IS the world height in engine
      units (256/tile); width re-derives from the frame's pixel
      aspect each draw (GameRenderOriginal.cpp:2192-98 — our
      renderer's world_w already does exactly this); `word_2`/
      `word_4` are DEAD fields (never read); a 0 height cross-fills
      from `speed_6` × aspect (loader :44895-903). DRAW TYPE: the
      table's byte_12 is OVERWRITTEN at load from the TMAPS entry
      header byte = payload[1] = the flags HIGH byte (:44906), which
      our bake already preserves in SpriteEntry.flags — so
      `draw_type = flags >> 8`, NO REBAKE. Draw-type semantics =
      MC1's switch (17-20 rotational, 0/1 static, 21 no-reorient)
      plus the 22-36 animated band (LABEL_26) — renderer arm
      widened `2..=16 | 22..=36`. `byte_11` = sprite-list span tag
      (255 = rotational family), consumed at level setup, not draw.
      App: `entities::resolve_pose_sprite(game, type_index, dims)`
      dispatches MC1 stats vs MC2 params; `sprite_dims` closures now
      return (w, h, flags). INDEXING NOTE: our extracted
      SPRITE_PARAMS is exactly remc2's 0-based table (wizard 0x52
      row = index 43 both sides; the research report's "row 44" was
      a line-number miscount — re-verified against
      Type_WORD_D951C.cpp raw initializer).
    - Player start: MC2's GenerateEvents start = the (10,0x52)
      record, parent=player number → class-3 m0 wizard at terrain
      alt, yaw 0, NO spawn hover (hover is flight physics)
      (Events.cpp:162-170, AddPlayer_4A920 :33317). BUT campaign
      level-000 authors NO (10,0x52) — it uses the MC1-shaped (3,4)
      marker (slot 1, tile 77,222): `player_start` MC2 arm tries
      (10,0x52) then falls back to (3,4). The (3,4) record misfits
      through the spawn seam (placeholder stone at the start point —
      truthful, cosmetic).
    - Map dots INTERIM: MC2 poses run the MC1 sub_48710 switch (the
      slice keys line up: villager (5,13) ∈ 12..=14 green, spheres
      (10,39)). The real MC2 map law is TRACED AND BANKED for the
      map pass: DrawMinimapEntities_B_61A00 (GameUI.cpp:951,
      switch :1141-1411) — 12-bit colorPalette_var28 map (CIVILIANS
      15, MARKER_STONE 0x88, SPELLS 3840, CREATURE 4095...),
      playersColors_E88E0x team colors, MSPRD bitmap stamps (83/84
      cases), possession icons via sub_885E0, castle rope line
      :1089-1130, map-type center colors :1043-63.
    - Exit (f) BOOT GREEN: level-000 renders headless, 61/61 live
      poses resolve to billboards, start honored, misfit ledger
      truthful ((10,45)x47 houses, (11,*) triggers, (10,60)x9,
      (10,1)x8, (10,29)x2, (2,2)x3, (3,4)x1). Tests: app unit tests
      pin the size law + level-000 start; 171 workspace tests green;
      MC1 goldens + mc2_slice goldens UNMOVED (zero re-pins).
      PLAYTEST OWED: fly level-000 — slice creatures visibly
      acting, MC2 sprite sizes/facings vs retail memory, placeholder
      stones legible, map dot legibility on night terrain.
  - **MC2 PLAYTEST-1 + BUILDING CREATOR — LANDED 2026-07-09** (same
    day as 3.5; player ran level-000 against retail):
    - PLAYTEST REPORT: map terrain plausibly accurate; map dot
      colors off (known interim; the REAL MC2 map law is traced
      below); buildings absent (fixed this session); the narrator
      route "underwater and made of settlers" where retail shows
      obelisks→spire; narration voice tracks loop instead of firing
      at trigger points; verdict: don't over-disambiguate against
      the unported mass — proper verification when implementation
      is complete.
    - LEVEL-000 DATA DECODE (probe): the "settlers" = a PERFECT LINE
      of (5,13) villagers, slots 23-51, y=212, x=118..145, ALL on
      water, dis 0 — starting at the first fly-to (115,212); the
      retail-visible route = the (10,1) chain (42 records,
      SEQUENTIAL dis 13..32, fired progressively by the (11,0)x21
      trigger switches) + 81 (10,45) buildings total (47 at-load =
      the western village, the rest disposition-gated = the drowned
      EASTERN village that builds up as the route fires); the type-7
      kill objectives (slots 103/189) ARE the disposition-3 archers
      in that drowned village. It's one coherent authored sequence.
    - REMC2 RESEARCH (agent, source-verified; citations in the
      module docs): (10,45) = AddTerrainModification_50250 :36677 +
      sub_49A30 :32753; BUILD0-0.DAT = the footprint bank (6-byte
      TAB rows ≡ MC1; cells = TWO bytes {paint code, pad height});
      BLDGPRM = {u16 production rate, u8 flags [0x10 pass F/G split,
      8 no-mana, 4 no-cave-raise, 1 enterable], u8 chain} — NO
      footprints (bundle.rs doc corrected); the 30-tick action
      ApplyTerrainModification_37240 :27181 = footprint kill
      sub_57390 :39746 (class 2 removed, class 5 KILLED except
      models {6,8,10,16,22,23,27} + 25-in-action-200) → per-tick
      height LERP toward pad+base → every-5th walkable paint
      (angle&0xF0|1, type 1) → final park state 52 with life =
      1000*rate; MC2 BUILDINGS ARE BILLBOARD SPRITES — remc2 has NO
      polygon draw path at all (DrawSprites_3E360 GameRenderNG:2731;
      sprite row 177; SetShiftByCastle_49EC0 quad from footprint);
      ARROWS carry a TARGET-CLASS FILTER (xtype=3/xsubtype=-1,
      sub_200F0 :11955; sub_10780 skips other classes :3766) — no
      friendly fire; NARRATION = ONE CD track per level sliced into
      SEGMENTS (CdTracks_DB080[28], Type_DB080_CdTrack.h: segment 0
      at level start MenusAndIntros:3599, segment objective+1 per
      stage advance :41038; PlayCDTrackSegmentNumber_86EB0 :47987);
      class-11 models 0/1 = INVISIBLE switch entities that fire
      sub_4A1E0(id,1) (the disposition activator :44504/:44516) +
      WAV 41 for models>3 only; WATER: getTerrainAlt = the FLOOR
      (mapHeightmap); night maps have NO occluding water surface
      (sub_43B40 second-heightmap build is CAVE-ONLY, Terrain.cpp:52
      — night runs sub_43D50 which only flags open sea; the
      rasterizer draws every tile at 32*mapHeightmap + wave dip
      :817-27), sea level = const 44 off-cave (caves read header
      byte_0x2FED3); die-on-water: ALL slice behavior rows set flag
      bit 1 and block water in v_20 (villager 0xfffffefe, archer
      0xfff080fe) — a boxed-in walker on water dies on its first
      all-blocked move (:8855), same-tile moves commit unbraked
      (verbatim both sides).
    - LANDED: the (10,45) BUILDING CREATOR (mgc_sim::mc2::mobs
      mc2_spawn_building + mc2_building_tick; known_thing + spawn
      arm + game-keyed class-10 51/52 dispatch arms; GenerateEvents
      pass F/G split live off bldgprm flag 0x10); FeatureAssets
      gains bldgprm (HASH-TRANSPARENT when empty — MC1 golden stream
      unchanged, verified) + with_bldgprm; mc2 bundles now carry
      build.tab/build.dat (BUILD0-0, one bank all environments —
      Basic.cpp:271 fixed path; BAKE_EPOCH 3, rebaked); app + both
      fixtures feed the mc2-night bundle's OWN search/build/bldgprm
      (mc1-temperate stand-in = fallback only); ARROW FRIENDLY-FIRE
      FIX (the class-3 target filter — pre-fix, archer volleys
      whittled their own pack); live_poses: unclaimed (10,45) hidden
      on MC1 (house = painted terrain, flag-when-claimed) but DRAWN
      on MC2 (the building IS the sprite). APPROX register (module
      doc): build carousel (IsNextEvent0A_2A one-at-a-time) — all
      raise concurrently; per-cell sub_462A0 retile → one MC1-shaped
      retile_and_shade on the final tick; sub_45DC0 texture-band
      paint (codes >= 8) + sub_48A20 edge rings UNPORTED — the
      type-1 village paint stands in (= the player's "missing
      textures", parked until the texture pass); cave second
      heightmap waits for caves. FIXTURE: mc2_slice re-pinned
      (DELIBERATE: buildings at init + native assets + arrow
      filter); new asserts — >=10 at-load buildings, state-52 park,
      type-1 paint under the pad, villagers FREED (imprisonment gap
      CLOSED; free walkers may die authentically — survival not
      asserted, zero kill-credit is), archer watch tracks the
      walking archer AND stands in its FOV cone (the wizard scan is
      cone-gated :9152-95 — buildings woke the archer walk states);
      frankenstein: buildings must NOT be ledgered + must be live.
      171 workspace tests green; MC1 goldens NEVER moved. PLAYER
      CONFIRMED in-session: "buildings are actually present".
    - ~~RETAIL CHECK banked: fly retail level-000 to (118..145, 212)
      at level start — does the drowned settler line show/flash/die?~~
      CLOSED by MC2 PLAYTEST-2 (player, 2026-07-10): retail has a
      NARROW STRAIGHT PATH across the water there — the walkers
      cross it; our port never loads/raises it (suspect the class-14
      m1 terrain riser). See "MC2 PLAYTEST-2 — LEVEL-000 MISFIT
      IDENTIFICATION".
    - ~~NEXT (the route experience): switch triggers + texture pass
      + map law~~ LANDED (next block); narration segments (extract
      CdTracks_DB080 → slice the baked redbook audio → segment 0 at
      start, objective+1 on stage advance — kills the track loop)
      and building production (state 52 mana) still open.
  - **MC2 VISUALS SESSION — LANDED 2026-07-09** (same day; four
    Opus research traces + four ports; PLAYTEST OWED — fly level-000:
    the route firing switch-by-switch with visible explosion
    clusters, building ground textures, MC2 map dot colors, trigger
    areas + green objective circles on the map overlay):
    - TEXTURE-BAND PASS (mgc_sim::mc2::terrain_paint, all verbatim
      from Terrain.cpp): `unk_D4A30` = the 144x2 texture-band bank
      (embedded, extracted programmatically); sub_45BE0 slope
      classifier; sub_45DC0 paint-code interpreter (codes ≥ 8 =
      banded texture + angle-nibble rotation + the 0x80 lock, < 8 =
      blend-class nibble + retile); sub_462A0 = the MC2-NATIVE
      retile (village fill + base-7 blend + shade with the NON-DAY
      INVERSION `64 - s`, :2030-33; NightShade flag on Gen is
      hash-transparent when off — MC1 golden STREAM unchanged, app
      sets it from the level header, fixtures match). KEY FIND:
      MC2's blend table `building_F2CD0x` is GENERATED at level
      setup (sub_44580 over `unk_D47E0` — Level.cpp:231/364 is
      savegame I/O, NOT a load) by the SAME 8-dihedral algorithm as
      MC1's byte_B5D40 (all 8 permutation keys + orientation codes
      match corners::arrangements line-for-line) — so
      `Gen::retile` simply swaps to `retile_table_for(MC2 corner
      classes)` at construction; NO bake change. Building tick now
      runs the retail cadence: per-lerp-tick sub_462A0, every-5th
      village pre-paint + sub_45DC0 overpaint, final-tick per-cell
      sub_462A0 sweep + the sub_48A20 pad-edge height rings (3x3
      smoothing over non-building-textured cells, offsets kept
      verbatim including the half-extent asymmetry). This closed
      the "missing textures" playtest report. APPROX: cave
      second-heightmap arms skipped (Phase 4.5); `x_BYTE_D41D8`
      per-texture anim flags undecoded (Maths.cpp — check when
      terrain anim fidelity comes up).
    - CLASS-11 SWITCHES ((11,0..=3) known + native): spawn = the
      shared trigger chassis (id24 = the record's stageTag_12 =
      the disposition it FIRES; extents word_10<<8 x 4096 — the
      (11,_) post-init was already shape-identical to remc2
      :33198-207); tick = mc2_switch_tick (models 0/1 =
      enter/leave one-shot CONSUMING fire ≡ sub_4A1E0(id,1), 2/3 =
      repeating with the 10-count rearm, non-consuming); probe =
      the &7 phase gate + ground re-snap + a 2D-ONLY box test
      (CompareAxisWithShift_106F0 :3726 — NO z term; switches
      trigger at any altitude) against the player, whose
      half-extents = sprite-params row 44 speed_6/2 = 0
      (AddPlayer_4A920 → SetEntityIndexAndRot 44 — the player
      entity is faithfully zero-extent; row 43 is the VISIBLE
      wizard billboard). Switches are never map-linked and carry
      no sprite — invisible and non-colliding by construction,
      the placeholder stones at (11,*) positions are GONE.
      active_volumes() now feeds the map-triggers overlay from the
      real switch boxes AND plots still-active stage checkpoints
      (VolumeKind::Objective, green, radius = the 3-tile fly-to
      latch) — the player's route-troubleshooting request.
      Models 4 (level-end), 5/6 (flag timer, WAV 41), 17, 32
      (stage-gated) stay misfits until their couplings land.
    - ROUTE EXPLOSIONS + SCENERY native: (10,1) "Big explosion"
      (NewAdd0A01 :35354: life 1, sprite row 41, sound 3;
      tick AddQuickfair0A_01 :22768: SEARCH-ring-0 sweep, ~50%
      per-cell draw, children at pos-96+192*cell±64) seeding
      (10,0) ground fires (NewAdd0A00 :35332 / sub_30D50 :22692:
      fuse &3, one-shot 400 ch0 area damage [sub_10C80 ≡ our
      area_write under the cross-column mask], worn-path repaints
      26/10/11 → sub_45DC0 codes 0x14-0x16 — THE SAME painter the
      buildings use — else the scorch dig, flicker z-drift, sound
      3); (2,0/1/2) tree/stone/dolmen (AddTree/AddStone/AddDolmen
      :33433-502 — tree = 4 LCG draws + ±32 jitter + sprite 83/84
      pick, burnable f56=1; stone/dolmen non-collidable, dolmen
      ShiftRot 1024). SEAM RULE (regression caught in-session):
      on the MC2 column `spawn_effect` models 0/1 resolve into
      the NATIVE MC2 ctors — the MC1-fallback fireball otherwise
      spawned an MC1-shaped fire that the game-keyed dispatch fed
      to the MC2 handler (damage-field mismatch = silent fires).
      MC1 spawn_effect 0/1 and MC2's are the SAME entity lineage
      (life 8/1, damage 400, sprites 7/41, extents 128). MC2
      class-2 tick column = inert hold (tree lifespan/burn joins
      the Phase-4 roster). Boot: 119/119 poses drawn (was 61);
      misfits left: (10,60) smoke x9 (traced: an emitter→puff
      particle chain, SetParticleSmoke3C_4EA20 + the rising-puff
      ticks :23576-23650 — port when decoration matters),
      (10,29) x2 (OPEN), (11,4), (3,4) (authentic).
    - MC2 MINIMAP LAW (mgc-app entities::mc2_map_dots, verbatim
      DrawMinimapEntities_B_61A00 GameUI.cpp:1134-1411 on our
      full-map view): 12-bit MapColourIndexs codes resolve through
      the level palette by nearest match — CLRD-0.DAT is retail's
      PRECOMPUTED copy of exactly that quantization (no bake
      needed; RGB nibble order settled by convention alignment:
      SPELLS 0xF00 red / CIVILIANS 0x00F blue / CREATURE white);
      playersColors_E88E0x team pairs embedded per environment
      (day/night/cave, :32180-262; cave = night + wizard-0
      override); map-type colours v90/v91/v92 (:1043-63); blink
      phases colorIndex_121[k] = (Turn/k)&1 (:37563-66). Rules:
      civilians CIVILIANS-blue, wild creatures = the map-type fill
      (night: white), owned units = team DARK, buildings/
      projectiles = owner-bright else 0xF0F, owned dwellings blink
      the pair, (10,0x12) + (10,0x56/57) skipped, portal 2x2,
      switch X-models 0x0C/0x1F = 2x2 white INTERIM for MSPRD
      stamps 83/84, spells red, class-14 m5 red/white blinker.
      Unit test pins the switch. BANKED: bake the MSPRD bank (the
      real X/castle-flag/balloon stamps + possession icons via
      sub_885E0), the castle ROPE line (:1089-1130, translucent
      white via the F6EE0+0x4000 blend LUT — joins the guide-path
      machinery when MC2 castles land), Beyond-Sight enemy-wizard
      reveal + name labels (:1492-1529, DrawText track), the
      centre-cross colours (:1531-78) if the rotating-radar
      projection ever ports.
    - Tests: mc2_slice re-pinned per landing (DELIBERATE, documented
      in the golden block) + new asserts (switches spawned/invisible/
      unledgered/overlay-fed, objective circles, building ground ≥
      type 8); the archer kill probe rebuilt as overhead volleys
      (MC2 creatures are zero-extent — the kill path is the fire ON
      the cell, whose area write fires ONCE per fire; live fires
      also CAPTURE follow-up fireballs via the victim scan and
      drift out of the z band, so volley-and-wait is the
      deterministic shape). MC1 goldens NEVER moved (the NightShade
      field hashes to nothing when off). 24 suites green, clippy
      clean.
    - PROGRESSION GATES (same day, after the player's mid-session
      run: "switches live, monsters missing"): level-000's FULL
      chain decoded from data (examples/switchprobe.rs in
      mgc-formats): the (11,1) LEAVE-trigger (74,212, box 64)
      releases dis 1 on leaving the start region → stage-gated
      (11,32) switches chain the campaign — par1 names the AUTHORED
      stage row; row-0 fly-to (115,212) done → dis 2, row-1 fly-to
      (194,213) done → dis 3 = THE FOUR (5,4) KILL-TARGET ARCHERS
      (the player's "missing monsters" — they hung off the unported
      model), kill objective (row 2, slot 103) done → dis 5 → the
      row-3 mana-goal gate → dis 6 = the (5,19) flyer wave, and
      (11,4) par1 5 = the level-end release (dis 4: the victory
      cluster + the (11,12) X-marker). PORTED: models 32
      (AddSwitch0B_20_6F1C0 :54353 — Mc2Stage carries `row`, the
      switch fires when its row hits state 2; par1 stored in f71
      per :33200-01, NO extents) and 4 (level-end = our `completed`
      latch, :54329). The slice now unlocks the archers through the
      AUTHORED chain (debug disposition fire removed) and asserts
      both checkpoints latched + cursor past them. KNOWN LIMIT: the
      dis-6 (5,19) flyer wave and the (5,3) multipart are Phase-4.3
      roster (placeholder stones); the final kill objective (row 4,
      slot 189) targets them, so the level currently stalls at the
      LAST stage until 4.3 — everything before it plays.
- **Phase 4 — MC2 flagship systems + breadth** (first written
  2026-07-09, ordered per the review; RE-ORDERED same day by PLAYER
  DIRECTIVE: "port all creatures rather than the ones from
  level-000... finish all of the tabular data [next session or two],
  then grind through levels and work out the deviations" — 4.3 goes
  FIRST as the FULL-ROSTER SWEEP, then a new LEVEL-GRIND phase where
  the misfit ledger + verb-fallback telemetry ARE the deviation
  worklist per level; three Opus inventory agents dispatched
  2026-07-09 to bank the complete creator/action-table survey →
  docs/SURVEY-MC2-ROSTER.md before the sweep session):
  - 4.3 FULL ROSTER (FIRST): every class-5 creature over the shared
    primitives, incl. the MULTIPART-CHAIN subsystem (states
    0xB4/0xE8/0xEA; unblocks (5,3)) and the class-9 flyer family
    (unblocks (5,19) and level-000's FINAL stage); the class-10
    sweep in waves (visual effects on existing machinery first,
    castle/spell/stage-coupled later); remaining class-11 models
    (5/6 flag timers, the 7..=0x2C middle band incl. 12/16/17);
    classes 12/14/15; the class-2 tick column (tree lifespan/burn);
    placeholder default flips OFF when the roster closes.
    - **WAVE A LANDED 2026-07-10** (nine background research agents
      → the verbatim trace bank docs/traces/mc2-*.md, keep + reuse;
      plus the AUTHORED-RECORD CENSUS, SURVEY-MC2-ROSTER.md §4 =
      examples/rostercensus.rs — the sweep-priority signal):
      - CLASS-5 (mgc_sim::mc2::roster, 14 new models): 2 (day-only
        pack hunter, melee 200 + recoil lunge, snd 12/13), 9 (the
        hive imp — the campaign's most-authored creature x4577:
        materialize countdown, m2-prey seek, generic cone scan,
        arrow volleys via sub_1CDA0, CONSUME-AND-SPLIT of models
        {4,12,13} within 0x600), 12 (builder — places real (10,45)s
        via the global-LCG template pick then RETIRES into the
        villager brain, action 105), 14 (trader — docks into far
        bldgprm-flagged buildings), 16 (boss 60000: wide building
        sweep + 15-bolt homing burst, (9,0) w/ subSpell 1600 + mana
        50000, snd 39), 17 (dive-bomber: (9,20) lobs ≥0x700, row-87
        3x-speed dive + melee 350, snd 58), 18 (slow tank: 5-shot
        (9,0) fan subSpell 800, watch/roam timers), 19 (the firebug
        flyer — level-000's FINAL WAVE UNBLOCKED: flank point 2048
        ahead of target yaw, RNG hover altitude + ±64 bob,
        strafe-bolt 500, dive-melee 300 w/ snd 43/44), 20 ((9,21)
        arcs then 2x-speed melee rush, snd 32/tick), 21 (floating
        caster x2804: hover-bob physics + pose sprites 305-312,
        (9,0) bolts, snd 42 roll), 23 (the mana leviathan — the
        only ctor-flyer, z=0x2000: hunts (10,39) spheres, grabs +
        SWALLOWS them (flags 0x40 grip), (9,9) subSpell-4000
        retaliation, snd 59), 24 (cave brute — ctor cave-gated =
        authentic no-spawn until 4.5; handlers ready, melee 1536
        dmg 1500 snd 7), 25 (swarm splitter: subSpell-300 LIFETIME
        countdown, castle-gnaw 60/tick via the attacker's-castle
        hunt, death SPLITS into 3 water-striding minis + (10,1)
        burst, snd 37), 26 (mana leech: drain manaRegen+14 — human
        side banked in Gen::mc2_player_drain until the MC2 mana
        ledger, %63 spell-hijack roll consumed/effect pending
        class-15, snd 62), 28 (fastest melee brute: one-striker
        pack gate, saved-yaw ±56 swing arcs, melee 768 dmg 2000,
        snd 38). Per-model spawn ordinals (array_0x10[m]++) live in
        Gen::mc2_spawn_ord (hash-transparent; the three SLICE
        creatures still use alloc-slot f63 — banked fidelity pass).
        NOT ported: 10 (doomsday pyramid — helpers OPEN in trace),
        15 (never authored, no launch site), multipart 0/3/22/27
        (traces BANKED, next session).
      - CLASS-9 (mgc_sim::mc2::proj): the shared flyer core
        sub_65820 (per-tick homing w/ ROW caps v_2/v_6 — row 64 = 5
        = the MC1-matching fireball cap, row 65 = 0 = straight
        creature bolts; ±2 speed ramp; xtype/xsubtype victim
        filter; terrain SKIM not detonate; water despawn exempt
        models {4,22,24,26}; life expiry; impact spawns (f68,f69)
        carrying subSpell), creators (9,0) bolt / (9,9) / (9,20) /
        (9,21), the attack thunks sub_1CC20/1CDA0/1CE80/1CED0/
        1CF20/1D0E0/1D1A0/1D260/1D460, and F_MC2PROJ (flags bit 29)
        keying native-vs-fallback dispatch so MC1 player spells
        keep falling back safely. Impacts whose effects are
        unported ((10,65)/(10,66)/(10,23)) apply their damage
        directly as ch0 area + count the misfit — damage lands, the
        visual gap stays ledgered. Shield ricochet/homing
        (sub_68740/68940/68AC0) skipped until (10,78) exists.
      - CLASS-2 (mgc_sim::mc2::scenery): models 3-8 ctors (6 =
        cave bee, cave-gated) + the TREE BURN LADDER (state 0 hit
        intake → flame spawn + 130..189 re-seed → state 1 burning →
        state 2 charred w/ sprite swap 83→226/84→227) + falling
        props (2,7)/(2,8) w/ gravity −24/tick clamp ±192 and
        RNG-bounce on damage. KEY CONTRACT FIX: burnable class-2
        now sets f28=1 (MC2's burn gate IS `(1<<ch) & byte_0x38_56`
        — without the +28 admit, MC1's area writer never reached
        trees). The tree flame = the (10,0) fire element standing
        in for retail's (10,6) (APPROX, same sprite family).
      - CLASS-11: the slot-condition band (sub_6F300 verbatim —
        model→slot map 13..=29→0..=16 / 33..=44→0x11..=0x1C, model
        30 = ANY-slot; occupied = live class-5 of the watched model
        excl. segment states; empty → 16-tick countdown → snd 41 +
        fire_disposition + despawn) + X-markers 12/31 (player
        proximity APPROX box test; 31 latches `completed`, both
        hide their linked class-14 map graphic). KNOWN LIMIT: a
        slot switch watching an UNPORTED model (the multipart
        family) fires ~17 ticks in (its slot never populates) —
        progression runs AHEAD instead of stalling; closes when
        multipart lands. Models 5..=11 stay misfits (trace OPEN).
      - CLASS-14 (world.rs): models 0/3/4/5 spawn + tick (X marker
        sprite 338 / end marker 339 terrain-pinned; scroll 280 w/
        768x1280 pickup box → snd 63 + Gen::mc2_scrolls, the 4-XP
        grant BANKED for the 4.2 spell-XP system); model 1 terrain
        riser spawns + holds inert (sub_59F60 ~1240 lines, trace
        OPEN) + misfit-counted; model 2 cave-only. drawable() +=
        class 14.
      - (10,1) corpse burst: mc2_kill's misfit note replaced by the
        native big-explosion spawn (KillEntity's class-10 subtype-1
        per the multipart trace) — corpses now pop authentically.
      - Tests: mc2_slice goldens re-pinned (DELIBERATE; the AT-LOAD
        hash is UNCHANGED — divergence starts when dis 6 releases
        the previously-misfit firebug wave); frankenstein contract
        updated (only multipart/(5,10)/(5,15) may be class-5
        misfits, (5,3) asserted PRESENT); 4 new unit tests (m18
        fan = exactly 5 bolts + 800-subSpell impacts, m21 bolt
        launch, tree burn ladder end-to-end, slot-switch
        chain-fire); examples/rosterprobe.rs smoke on level-000
        (firebug wave lives, dives w/ snd 43/44, melee-hits the
        player [intake snd 17]; slot switches fired snd 41). MC1
        goldens NEVER moved (Mc2Ord/Mc2Quiet hash-transparent).
        24 workspace suites green, clippy clean.
      - Misfit ledger after (level-000 frankenstein): (5,3) x4
        multipart, (10,5) splash x31, (10,29)/(10,59)/(10,60)
        effects, (15,2)/(15,3) tokens, (3,4) authentic start
        marker.
      - APPROX register (all flagged in module docs): m17 dive
        z-curve reconstructed from the trace's shape; m18
        sub_253B0 duration map partially pinned; m21 hover ported
        to summary; m12 site-jitter/clear scans shaped; the
        missing +6 handlers hold inert (retail's dispatch would
        CRASH — they are unreachable, rows' flee bit clear); m20's
        human mobilize-counter gate → thunk-result (4.4); m2
        vertical homing vs the human uses carpet z (no
        half-height).
    - **MULTIPART SUBSYSTEM LANDED 2026-07-10** (same day as wave
      A; three parallel Opus trace agents closed the gaps first →
      docs/traces/mc2-m27-branch-machine.md +
      mc2-m22-worm-helpers.md + mc2-m0-m3-gaps.md join the bank;
      `mgc_sim::mc2::multipart` = class-5 models 0/3/22/27; all 24
      workspace suites green, clippy+fmt clean, MC1 goldens
      UNTOUCHED, mc2_slice re-pinned once — ONLY checkpoint E
      moved, at-load through D identical):
      - m0 worm/hydra + m3 multipart flyer (ctors sub_4B240/
        sub_4B6F0): 16 state-0xE8 children byte-copy the head
        (keeping its id — owner immunity spans the chain; the m0
        head-mana>>5 quirk is bug-compatible), sprite rows 19+i/
        89+i, m3 link metrics = 65% particle values w/ first link
        125%; child tick sub_1B6B0 = rigid 3D follow at -f56 +
        own damage intake ("tail-shot kills the worm" rides
        mc2_state_head's min-life walk, already ported); head
        states = thin primitive wrappers + the m0 bob (gravity −5,
        floor bounce +150, cave ceiling = 4.5). KEY TRACE FINDS:
        states 0x06/0x1E are REAL enabled binary functions the
        decompiler NEVER LIFTED (remc2 silently no-ops them —
        held inert, structural guess = the flee slot, retail-check
        banked); the sub_1F0C0 projectile LASSO is authentically
        DORMANT (its fontTypeIndex gate is never armed by any
        recovered writer); PreKillEntity_1C890 IS chain-aware
        (cascades kill states over f54 — our mc2_prekill already
        had it) and the f63-per-tick increment means every child
        converts within its 8-tick stagger (the "only indices
        0/8" reading was pre-increment).
      - m22 segmented worm (the castle-mana thief): map par1 =
        tail length → sub_4CB60 ring pairs (signed offsets
        ±1..±7), spiral follow sub_271D0 (sub_273C0 chirality from
        the writhe-phase bits), owner-colorize suite sub_27590/
        27610/278F0 over the x_BYTE_D400C ramp (wild=52 base,
        verbatim non-monotone row 5), head move sub_26FF0
        (whole-chain terrain ceiling +384 w/ the f38 rise budget),
        writhe anim sub_272C0 (snd 48) + spin decay, grow cycle
        sub_27880 (1024 ticks: +2 tail to 15, +1000 mana cap
        50000), resize sub_27720 (grow/shrink PAIRS), the full
        0xB1→0xB2→0xB3 castle-drain chain (sweep-recolor inward,
        every-32nd castle acquire ≤0x100, deposit = self-consume
        by tail shrinking; castle lookup = None until MC2 castles
        land — retail's own castle-less revert arm serves), chain
        kill 0xB5 = whole-chain mana spheres. CRITICAL TRACE FIND
        (flagged OPEN, retail-check banked): the m22 head is
        DAMAGE-IMMUNE through its entire traced suite — sub_26F10
        consumes hits for accelerate/turn-away only, never
        subtracts life; melee only ENRAGES the worm. Ported
        faithfully (an external life writer → 0xB5 still works).
        Sound-42 mis-attribution corrected: sub_265A0 = the
        m20/21 hover core, NOT m22.
      - m27 3-tier tree kraken: 51-entity chain (body 0xD9 + 5
        branches 0xE9 + 45 segments 0xEA — the latter two are
        NULL dispatch entries, driven by the body via sub_29A90;
        the world loop skips their f63 so the branch machine's
        manual increment is the only clock), str_D404C[5] +
        xx_DWORD_D40BC[17] spline tables DUMPED from the binary
        statics and hardcoded, the FULL 16-way branch state
        machine (whip windup/forward strike at a target/back
        swing/segment-extend/detach-regrow cycle) w/ byte-faithful
        draws #A-#D, the 9-segment drooping-arc spline sub_2AA90
        (96-unit steps, symmetric pitch-bend {0,−c0,−c1,+c1,+c0,
        +c0,+c1,−c1,−c0}), branch bolts sub_2A7F0 ((9,0) low /
        (9,9) high on the 33% regen roll, subSpell 850, snd 15/23
        at the body, re-fire only at regen 2 — the easy-to-misport
        v37==2 case), whip snd 17, teleport re-plant 0xD8 (2
        draws + 128-probe walk, snd 22). LIFE MODEL: branches =
        REGENERATING LIMBS (initial ladder 460k+920 = 1380..3220,
        damage capped 76/hit, death → retract/detach → regrow at
        rand%0x398+920), body = 1000000 HP and untouchable until
        the f50 gauge (live branches) hits 0 — "clear the
        tentacles to expose the body"; body death = fraction
        scatter (16 spheres of the 20000 pool) + the 0xDD cascade
        popping every member. Sounds 37/59/7/62 re-attributed to
        m24/m25 (NOT m27).
      - Seams: spawn column +4 models ((5,22) passes par1),
        known_thing → (5, 0..=4|9|12..=14|16..=28), dispatch arms
        0..=7/24..=31/176..=183/216..=223/232/233|234, awake pass
        + every scan already carried the 0xB4/0xE8/0xEA
        exclusions from wave A. Field homes in the module doc
        (word_0x24_36→f38, byte_0x3B_59→f50, subSpell spiral→f46,
        playerEntityIndex→dest_x — no new Ent fields, hashes
        safe).
      - SHARED-CODE FIX with MC1 goldens held: ball_tick mutual-
        merge annihilation (two coincident mana balls could each
        absorb the other in one tick and the mana VANISHED — an
        our-port ordering artifact; retail's merged ball is
        display-disabled and can't re-merge). Entry guard on
        flags 0x400; every MC1 state-hash golden unchanged.
      - Tests: 4 new unit probes (m3 17-chain topology + awake
        trail, m0 head-death cascade + sphere drops [player moved
        AWAY — the certified collection loop hoovers drops near],
        m22 tail topology/colorize + chain kill, m27 51-topology +
        76-cap + gauge-0 exposure + 20000-mana scatter);
        frankenstein contract TIGHTENED (only (5,10)/(5,15) may be
        class-5 misfits, (5,3) asserted LIVE); examples/
        multipartprobe.rs smoke on level-027 (authors ALL FOUR:
        6 m0 + 12 m3 + 5 m22 + 3 m27 = 534 chain poses stable
        over 1200 ticks, branch machine whipping snd 17 x117 +
        both bolt tiers flying, worms writhing snd 48, misfit
        ledger multipart-CLEAN).
      - APPROX register (module doc): sub_2A7F0's `+= setting_30`
        LCG perturb unmodeled (a game-loop counter — branch
        stream diverges after the first bolt roll); m27
        sub_2A940's x_DWORD_E9BA8 freeze gate reads 0 (writer
        untraced, likely pause); the m27 emerge/teleport folds
        sub_102D0 mask 4 into the shared blocked test; draw-group
        byte[2]/byte[3] markers + the sub_49D50 palette-shade
        byte unmodeled (renderer-side); m27 show/hide = flags
        0x20 (the live_poses suppress bit).
      - RETAIL-CHECK BANK (new): does the m22 worm die to
        fireballs in retail, or only drain/expire (the head-
        immunity finding)? do m0/m3 ever flee (the unrecovered
        0x06/0x1E slots)? does a killed m0 worm's corpse-fire
        burn its own mana drops?
      - NEXT in 4.3 [RESOLVED 2026-07-10 — riser/spheres/tokens
        LANDED + the research fan-out; see the dated session block
        below]: the (14,1) TERRAIN RISER + its (10,63)/(10,64)
        raise/lower triggers — the FULL VERBATIM SPEC is banked in
        docs/traces/mc2-class14-m1-riser.md (3-phase machine, both
        orientations, every array write; 724 trigger records
        campaign-wide, levels 002/003/005/007/008 walls+pillars;
        NOT needed by level-000). Then the rest of the class-10
        band by census ((10,39) x3822 spheres!, (10,58) x1509 —
        note: creator 58 yields a model-39 sphere with mana 2560,
        the "returns-0" was an identity-check artifact, see
        mc2-class10-m59-m60.md §8; (10,28) x1424), then class-15
        tokens (trace banked), then (5,10) doomsday pyramid
        (helpers OPEN — needs a re-trace).
    - **MC2 PLAYTEST-2 — LEVEL-000 MISFIT IDENTIFICATION (player,
      2026-07-10; retail comparison via dosbox was blocked, these
      are from-memory + in-port observations; BANKED as next
      session's worklist — each maps a ledger entry to its retail
      look and may explain "behaviour not happening when it
      should"):**
      1. **(10,29) x2 = the QUEST BEACON — a smoke column**:
         continuous stream of transparent smoke clouds rising
         from the ground and vanishing; the SAME effect serves
         volcano smoke and quest beacons across the campaign
         (x2145 — the census priority is confirmed gameplay-
         critical). Level-000 authors exactly two: the first
         stage trigger and the spire center (two homing beacons
         in this level's stage goals). In the port they render as
         the misfit PLACEHOLDER (the "miniaturized stationary
         wizard glued to the floor" = placeholder art, not a real
         spawn).
      2. **(10,45) BUILDING TEMPLATE WRONG** (a real bug in the
         ported path, not a misfit): every building in the small
         starting city should be the ONE-BLOCK kind; the port
         renders the spire+outer-ring model (much bigger) for
         every one. Same wrong buildings inside the spire ("a bit
         of a mess"). Suspect the BUILD00/BLDGPRM template
         selection — the `building_fixup(par1 + 16)` id mapping
         or the template-index decode.
      3. **The rising structure near the spire is the WRONG
         one**: retail raises INDESTRUCTIBLE TALL SPIKES (between
         the first and second stage triggers, when flying around/
         arriving at the spire); the port raises something
         "indescribable but definitely wrong". Candidates:
         (10,60) x9 (count fits a spike row — placeholder art
         today) and/or the class-14 m1 terrain riser machinery.
         Player offers a screenshot if needed.
      4. **The MISSING WATER PATH** (CLOSES the Playtest-1
         "settlers imprisoned in water" retail check): retail has
         a NARROW STRAIGHT PATH from the land toward the spire —
         the humans between the stage triggers walk it; in the
         port they hang suspended in water because it never
         loads. No misplaced object plausibly IS the path →
         suspect TERRAIN deformation, i.e. the class-14 model-1
         riser (sub_59F60, ~1240 lines, held inert +
         misfit-noted) raising a causeway, possibly stage-driven.
         The (10,5) x9 splash misfits are consistent with the
         drowned walkers.
      5. Console ledger for the record — at load: (10,29) x2,
         (3,4) x1 [the authentic start marker], (10,60) x9;
         runtime adds (10,59) x1, (15,2) x1, (10,5) x9. Worklist
         order next session: (10,29) beacon + (10,60) spikes +
         the (14,1) riser trace (one fix likely closes items 3
         AND 4), then the (10,45) template bug, then the rest of
         the class-10 band.
      **RESOLVED (2026-07-10, all four items LANDED + traced —
      docs/traces/mc2-class10-m29-m5-m13.md, mc2-class10-m59-m60.md,
      mc2-class14-m1-riser.md; identifications largely CORRECTED
      by the decompile):**
      1. (10,29) is NOT a smoke column — it is an INVISIBLE
         one-tick stage/quest marker (sub_4FA00/action 0x1F); the
         placeholder statue was our misfit stand-in. The REAL
         smoke columns are the (10,59)/(10,60) emitters (remc2
         "in quest point") — one (10,13)/(10,14) rising cloud per
         tick for 800..899 ticks, invisible untargetable emitters.
         BOTH families ported (mc2::effects).
      2. (10,45) template bug: the shared post-init applied MC1's
         building_fixup(par1+16) to MC2 — MC2's id is RAW par1
         (EF:33089) + par2→xtype/f66 (= the on-death disposition,
         fired by the collapse). FIXED + player-confirmed.
         FOLLOW-UP (player report, same day): all buildings flew
         a player flag — the billboard IS the owner flag (sprite
         177), drawn claimed-only in retail; landed the state-52
         house tick (possess claim via the f56-bit-1 delivery
         gate — stone templates bldgprm flags&8 never set it —
         + militia pop + damage) and the state-53 teardown
         (occupant evacuation, terrain restore, chain rebuild,
         on-death disposition). Building destructibility now live
         (ctor f56=33).
      3. The "wrong riser" near the spire: no (14,1)/(10,63)/
         (10,64) exist in level-000 AT ALL — the rising structures
         there are (10,45) BUILDINGS (dis-fired construction
         anim) with indestructible/unclaimable templates + the
         (10,60) smoke ring dressing (9 authored in a ring at
         ~(77,220)). The riser machinery serves levels 002+.
      4. The MISSING WATER PATH is the (10,29) WAYPOINT CHAIN
         (slots 62↔63 par-linked): GenerateEvents stamps each leg
         (angle nibble := 1 clears the deep-water bit + retile) —
         cells (114..174, y=212), exactly the drowned-villager
         row. Ported as mc2_waypoint_chain (synchronous stamp).
      Frankenstein misfit ledger after the batch: (3,4) start
      marker [authentic no-spawn] + (15,2)/(15,3) tokens only.
      MC1 goldens untouched; MC2 slice re-pinned (DELIBERATE).
      **Player follow-ups banked (2026-07-10):** (a) possess-claim
      verification BLOCKED until MC2 spells exist (Phase 4.2) —
      the claim column is in but untestable; (b) retail smoke
      beacons are TRANSPARENT — texture-level translucency, not
      an alpha mask (the entities carry byte[2]|=2, retail's
      transparent-effect draw list; trace OPEN-4) — a renderer
      item for the 4.9 presentation track [RESOLVED same day —
      see MC2 PLAYTEST-3: the byte[2] reading was WRONG, the real
      mechanism is per-sprite raster modes; render-level alpha
      LANDED + player-endorsed, docs/traces/
      mc2-transparency-drawlist.md]; (c) remaining placeholder
      billboards = the class-15 spell-jar tokens.
    - **MC2 PLAYTEST-3 BANKED (player, 2026-07-10, same-day
      session — the landed half of this playtest is recorded in
      the session ledger below):**
      - **CASTLE BUILDER WRONG (bank for next session — TRACE
        FIRST):** castle levels 1-2 build effectively NO visuals —
        only a white flag planted in the ground (or water); level
        3 then erupts into an amalgamation of water/rock/
        impenetrable-wall textures in a strange spiky arrangement
        (player screenshot on file: tall thin wedges of blue
        plank-ish texture rising from water in a rough ring).
        Suspects, by analogy with the (10,45) template bug: the
        MC2 castle build path reading MC1's castle footprint
        law — wrong BUILD-bank entry or the MC2 2-byte cell
        {paint code, pad height} decode misapplied to the castle
        pad, and/or MC1 castle-level geometry tables used where
        MC2 has its own (the MC2 castle global id-68 is a noted
        APPROX skip in mc2_spawn_building). Trace the retail MC2
        castle build/upgrade chain (spell cast → terrain writes →
        wall/keep paint) before porting fixes.
      - Retail's rotated-map dot offsets (mana blobs embedded in
        walls, dots drifting when a balloon carries spheres) are
        a retail BUG we deliberately do NOT reproduce — our dots
        bake at true world positions into the rotated texture
        (player directive 2026-07-10).
    - **MC2 PLAYTEST-3 SESSION LEDGER (2026-07-10 — the landed
      half; presentation-fidelity batch, all trace-backed):**
      1. **SHADE-LUT MIS-CARVE FIXED (BAKE_EPOCH 5):** all four
         MC2 bundles carved shade-lut.bin at TABLES +0x4000 —
         that slice is the 256x256 sprite BLEND matrix
         (`nearest_palette(⅓·src + ⅔·dst)`), NOT the shade LUT;
         both games shade from +0x0000 (row 32 ≈ identity, row 0
         = fog/sky color, row 63 = black). Found by the
         transparency trace, verified byte-for-byte
         (shade-lut.bin == blend-lut[:0x4000]). PLAYER-CERTIFIED:
         map colors now match retail ("perfectly"); night sea =
         same noise, far darker. FORMAT.md corrected.
      2. **TRANSLUCENCY LANDED (render-level, player-endorsed
         over LUT-exact — "wouldn't even try to reproduce the
         faithful mechanism"):** LivePose/Billboard carry retail
         raster mode (0/2/3); a second billboard pipeline draws
         modes 2/3 with alpha 1/3 / 2/3, back-to-front,
         depth-test-only. Smoke (10,13)/(10,14) tagged from the
         static particle descriptors; entity-flag overrides bit
         23 → mode 2 / bit 24 → mode 3 wired (bit 23 is
         DUAL-PURPOSE by retail design: also the m26 wraith's
         full-speed wake marker — the ghost look IS the state;
         reconciliation in the trace doc §6.2).
      3. **ENV-DRIVEN SKY:** MC2 sky/fog = the bundle shade-LUT
         row-0 mode through the palette (night/cave black,
         night-fog near-black, day pale blue) — clear, distance
         fog and map-screen fill all follow; verified black on
         level-000. MC1/HW keep the certified constant until the
         sky trace lands.
      4. **UNCLAIMED-BUILDING MAP DOTS:** retail's map pass never
         skips on the claim bit — unclaimed buildings draw 0xF0F
         UNPOSSESSED_BUILDING2 (GameUI.cpp:1276-95), the same
         pink as all unowned class-10 (incl. smoke). Pose export
         now emits them as map_only (dot yes, flag billboard no).
         The sub_885E0 possession-icon stamps (kind 21 =
         productive unclaimed) stay banked with the map-icons
         track.
      5. **MC2 MAP-SCREEN LIVE VIEW UN-SQUASHED:** aspect-true to
         the viewport rect with the flight fov_y = the middle
         slice of the normal view (player retail observation,
         senior over the EF:21864 full-frame-squeeze reading).
      6. **BUILDING POSSESSION FIXED (the PLAYTEST-3 "possess
         claims balls but not buildings" report):** traced
         end-to-end (docs/traces/mc2-possession-delivery.md) —
         retail casts spawn a (10,12) claim pulse (9 ticks,
         sub_112D0 ch1 mail to every byte_0x38_56&2 overlap);
         our mc2_spawn_building populated only the faithful
         mirror f56 and never f28, and the SHARED writer gate
         (area_write, mc1/combat.rs:126) tests f28 — the claim
         mail (and ch0 area damage!) never reached the house
         tick. Fix: f28 = 1 (+|=2 productive) beside the f56
         writes — the cross-column damage contract extended to
         the claim channel. MC2 slice goldens re-pinned
         (DELIBERATE, every checkpoint — 47 at-load buildings
         hash the new field); MC1 goldens untouched. PLAYER
         VERIFY OWED: possess a dwelling (chime 4, flag +
         militia); NOTE buildings now also take area damage
         (retail-correct destructibility, watch for it in
         playtest). Banked in the trace: the stone-template
         probe skip (projectile flies through stone), the
         level-2 forced/steal tier, rival-claim chime anchor.
      7. **WALKER/WANDER AI FIXED (the PLAYTEST-3 sheep-dispersal
         + brownian-settlers report; docs/traces/
         mc2-walker-wander-ai.md):** the "sheep" are (5,1) = remc2's
         GOAT (our Vulture label was a survey misnomer — renamed
         MC2-side only; MC1's (5,1) really is the Vulture). Retail
         needs NO flocking magic: fast yaw slew + leader-follow
         chains + binary slope refusal keep herds milling; settlers
         are NEVER in free wander — they permanently march at the
         nearest ENTERABLE building (the causeway files = that scan
         + water flanks + slope walls, the playtest's guess
         confirmed). Landed D1-D4: (D1) move-commit turn clamp
         v_4→v_2 (goats 5→45/tick — THE dispersal bug; sub_58350's
         v_4 arg is dead in retail), (D2) the passability probe's
         one-extra-point false block (the brownian spins), (D3) the
         townie building scan's bldgprm byte_2&1 enterable gate,
         (D4) the +0 patrol reversed-cone quirk. Goat-flee probe
         reworked to retail law (only wander/patrol scan wizards;
         followers copy their leader) + a new follow-chain
         assertion; MC2 goldens re-pinned (DELIBERATE), MC1
         untouched. PLAYER VERIFY OWED: herd stays put south of
         start; settlers file along the causeway. Banked retail
         checks in the trace (herd radius, settler split point).
      7b. **WALKER RESIDUAL — BANKED (player, 2026-07-10: "a gap
         in the movement direction, tomorrow's problem"). After
         D1-D4, goats/settlers move and function but their
         LONG-RANGE character still deviates. RETAIL EVIDENCE BANK
         (player dosbox session, screenshots on file):
         (i) goats = "mountain goats": authored spread converges
         into ONE tight cluster on a mountain peak ringed by
         valley moats; they wander the (visibly steep) flanks and
         essentially never leave — 1-2 individuals escape over a
         long session; they DO tick and move throughout, just
         little; (ii) settlers read as a PURE RANDOM WALK (no
         dwelling-seeking; claiming every village building changed
         NOTHING) drifting slowly across the causeway — the
         crossing is emergent from water flanks + hill aversion;
         (iii) both groups' effective mobility is far below ours.
         **RESOLVED BY THE TRACE FOLLOW-UP (same day —
         mc2-walker-wander-ai.md §FU.0-FU.6, superseding note at
         doc head): OUR LAWS ARE CORRECT; THE GAP IS TICK
         ECONOMICS.** No time-slicing/culling/freeze exists
         (UpdateEntities_57730 EF:40116-80 = plain full-pool loop
         per tick); no pen exists (flood-fill on oracle-exact
         terrain: 15,920 tiles reachable under goat rules; v_22 =
         dead pad, no anchor/uphill bias, no +0 traffic); a
         reference simulation of the exact retail law reproduces
         OUR dispersal (median ~19 tiles @500 ticks, ~70 @4000).
         The townie march is real and UNGATED (only static
         bldgprm byte_2&1 — why claiming changed nothing); it
         reads brownian in retail via one steer per 40 ticks
         drowned by causeway blocks + survivor bias (marchers
         vanish into houses). The 8/28 settler drownings are the
         law working (flag-1 dies on ANY all-four-blocked tick).
         RECONCILIATION: retail ticks ONCE PER RENDERED FRAME
         (EF:31800-15, ~15-20 fps period hardware) and freezes
         the world under pause/menus (EF:40093); our port ticks a
         fixed 30 Hz and does NOT pause in map view → 2-4x sim
         time per wall-clock minute. BANKED FIXES: F1 calibrate
         the MC2 tick rate (+ authentic x4/x8 game-speed option),
         F2 retail pause semantics (map/menu freeze — verify
         retail's map-pause first), F3 nothing else (D1-D4
         stand). FALSIFIER for the next retail session: a
         3-minute UNPAUSED close-range herd timelapse — the law
         predicts 20-50 tiles drift; a few-tile result would
         force a from-binary retrace (GOG NETHERW.EXE layout
         differs from remc2's IDA base — direct verification
         banked). TICK-RATE CALIBRATION (player's smoke-clock
         protocol, 2026-07-10): time one smoke-column cloud
         base→top in retail — clouds live EXACTLY 32 ticks
         (emitter overwrites life, mc2-class10-m59-m60.md §3), so
         tick rate = 32 / T seconds (ours: 30 Hz → ~1.07 s;
         1.6 s → 20 Hz; 2.1 s → 15 Hz; 3.2 s → 10 Hz). Player
         priors noted; SUPERSEDED same day by the TIMING VERDICT
         (§FU.7 + the player's decisive retail observation): MC2
         is FRAME-LOCKED FREE-RUNNING — the in-game loop is a
         bare while(1) with no wait/timer (EF:31621-59); the
         engine's only clock is the 120 Hz AIL service timer
         (EF:43027-29; PIT divisor 10022 fallback) feeding input/
         fades/FLC only, never the world pass; the x4/x8 speed
         key is a plain loop multiplier. CONFIRMED from gameplay:
         retail smoke rises SLOWER at higher resolution — tick
         rate = achieved fps. So retail has NO nominal tick rate;
         period hardware ran ~15-25 fps at 320x200. remc2's own
         calibration: 30 fps cap (maxGameFps default,
         read_config.cpp:24) = OUR EXACT 30 Hz. F1 RESOLVED: our
         fixed 30 Hz + interpolation IS the correct ceiling
         (nothing to change by default); optional "period
         hardware" tick-cap knob (15-18 Hz) as an authenticity-
         matrix option for retail-slow walkers. F2 (pause
         semantics: retail freezes the world under menus — verify
         the map screen; our map view does NOT pause) remains the
         actionable fix. Smoke clear-time protocol if still
         wanted: emitter life = rand%100+800 ticks + 32 for the
         last cloud → 832-931 ticks total (30 Hz: ~28-31 s;
         20 Hz: ~42-47 s; 15 Hz: ~55-62 s; 10 Hz: ~83-93 s);
         MEASURED (player, 2026-07-10): retail ~44 s → ~19.7
         ticks/s (the player's dosbox+resolution delivers ~20
         fps); our sim ~27 s → ~810 ticks at 30 Hz, validating
         our clock AND the emitter law. Verdict: we tick 1.5x
         faster than retail-as-played — the "period hardware"
         authenticity knob should offer ~20 Hz (measured), and
         the remaining herd-containment residual, if any, rests
         on the F2 pause fraction + the banked 3-minute unpaused
         herd timelapse falsifier (the law predicts clear drift
         even at 20 Hz).
      7c. **ARCHER DEATH SOUND — RESOLVED AS MISIDENTIFIED
         (player, 2026-07-10 session-5 playtest):** the scream
         was NOT the archer's death sound — it was the PLAYER
         DAMAGE sound (an arrow hitting the player), coinciding
         with the kill. No archer-sound bug; the per-game MC2
         sound map remains the general Phase 4.6 item.
      8. **NIGHT ENVIRONMENT TRACE LANDED (docs/traces/
         mc2-night-environment.md)** — confirms the session's fixes
         and banks the rest: classification = LEVELS.DAT header
         byte 6 (level 000 = Night, gfx 0; campaign tally 77 day /
         36 night / 5 fog / 47 cave); retail sky = a wrap-tiled
         CLOUD-PLANE bitmap (SKYD/SKYN0-0.DAT, DrawSky_40950 —
         roll/yaw/pitch tracked; no stars/moon), flat keyColor1
         fill for cave/sky-off — our row-0 sky is numerically the
         exact base color (SKYD 74% idx 254, SKYN 94.5% idx 64);
         BANKED for 4.9: bake the SKY bitmaps for the cloud plane +
         the retail fog law (linear in d², onset 3840 / full 4864 /
         cutoff 5120, terrain fogs by shade ROW toward row 0 — and
         the LUT brightness POLARITY INVERTS at night: row 0 black,
         row 63 brightest; scorch/shadow/dynamic-light code depends
         on it). Big-map path confirmed retail-verbatim. Also
         banked: the 50-slot night/cave DYNAMIC LIGHT system,
         per-env CLRN/CLRC map-color files, per-env sound/music
         programs, day/night spell deltas (spells 4/19 life 2↔19,
         Cave-In cave-only, (5,2) day-only spawn).
    - **FULL-CAMPAIGN UNPORTED CENSUS (2026-07-10, 165 baked
      levels, records/levels) — the tabular remainder before
      gameplay/spells/end-to-end:**
      - CLASS 15 spell tokens, models 0..=25 (~890 records,
        50 levels) — THE SPELL JARS, prerequisite for 4.2;
        trace banked (mc2-class15-spell-tokens.md).
        [LANDED 2026-07-10 — pickup layer; cast machinery = 4.2]
      - (10,39) mana spheres x3822/81 + (10,58) sphere-2560
        variant x1509/66 — the ground mana economy.
        [LANDED 2026-07-10 via the shared ball machinery]
      - The UNIDENTIFIED class-10 high band: (10,80) x3033/45,
        (10,83) x1696/36, (10,84) x1000/38, (10,85) x854/35,
        (10,82) x333/42, (10,86) x7/1 — ~7000 records with no
        identification yet; needs a creator/action trace sweep
        like the m59/m60 doc.
        [IDENTIFIED 2026-07-10: the CAVE TERRAIN GENERATOR —
        mc2-class10-high-band.md; defers to Phase 4.5 wholesale]
      - The class-10 middle band: (10,11) x1265/61, (10,31)
        x1073/28, (10,28) x1424/35, (10,6) standing fire x973/43
        (the (10,0) stand-in APPROX in scenery.rs), (10,50)
        x695/15 [a sub_49090 chain family like the waypoints!],
        (10,9) x286/47 [generate pass-1, special par writes],
        (10,76) x172/25, (10,54) x157/37, (10,25) x104/3,
        (10,22) x102/16, (10,57) x89/11 [the third puff variant
        — ALREADY TRACED, m29-m5-m13 doc §4.3], (10,17) x69/15,
        (10,15) x52/14, (10,67) x49/9, (10,71) x21/6, (10,8)
        x11/2, (10,23) x10/2, (10,52) x6/2.
      - (10,34) MC2 portal x312/45 — the MC1 portal arm exists;
        the MC2 spawn column doesn't admit it yet.
        [TRACED 2026-07-10: a self-contained player-only warp,
        NOT the MC1 pairing — mc2-class10-m50-chains-and-tail.md]
      - (10,63)/(10,64) riser triggers x724/44 + (14,1) — spec
        banked, the next-session opener.
        [LANDED 2026-07-10 — mgc_sim::mc2::riser, full lifecycle]
      - (5,10) doomsday pyramid x41/6 — helpers OPEN, re-trace.
        [RE-TRACED 2026-07-10, all helpers CLOSED, port-ready —
        mc2-class5-m10-doomsday.md]
      - Class-3 start markers (3,4..=11) x510 — already
        functionally consumed (start_markers); ledger-noise only,
        admit as known no-spawns when convenient.
      - **THE LEVEL-ENDING PORTAL SEQUENCE (player, 2026-07-10 —
        critical-path for end-to-end): NOT a THING — it's the
        class-3 PLAYER actions 11/12, `sub_5E8C0_endGameSeq`
        (EF:60313, str30[0xB/0xC] = 0x23F8C0).** The final stage
        trigger / (11,31)-family switch sets the player's
        actionIndex = 11 (already documented in
        mc2-class11-switches-class14.md §3): the handler's
        byte_0x46_70 phase machine TAKES CONTROL (case 0: zeroes
        the movement channels, chime 41, sub_5C800(a1x,6),
        targets the class-14 model-4 level-end marker via
        word_0x36DFC — the MAP MARKER we already spawn), case 1
        decelerates, case 3 rotates toward the marker, later
        cases spawn the portal visual (the "demon mouth"), fly
        the player in with MOTION BLUR (the renderer gate
        `str_0x21AE.xxxx_0x21B1`, GameRenderHD:227) and force
        level termination. Retail look: big demon-mouth portal,
        forced flight, motion blur. **DEFERRED (player,
        2026-07-10): the cinematic is the fancy stuff at the end
        — do NOT trace/analyze it now. Awareness-only anchor; a
        plain level-termination on player-action-11 suffices as
        the stand-in until the presentation pass. Eventually
        needs a FlightVerb takeover seam + renderer blur.**
  - **2026-07-10 SESSION — RISER + ECONOMY + TOKENS LANDED, 4-AGENT
    RESEARCH FAN-OUT COMPLETE (all workspace tests green; MC1
    goldens untouched; MC2 slice re-pinned DELIBERATE — the runtime
    (15,2) jar spawns live from window C on):**
    - LANDED (14,1) TERRAIN RISER — `mgc_sim::mc2::riser`, the full
      sub_59F60 three-phase machine from the banked spec (instant
      build / animated raise / animated lower+restore, both
      orientations, every non-cave array write verbatim incl. the
      OPEN-1 symmetric-dirty fix; cave ceiling arms = 4.5) + the
      (10,63)/(10,64) one-shot triggers (sub_5B070 same-cell lookup)
      + the LABEL_49 par wiring (par1=orientation, par2=length) +
      loop sound 47. Riser is INVISIBLE now (no sprite — the old
      placeholder-77 misfit note removed). 3 lifecycle tests
      (instant X/Y, lower→restore→re-raise).
    - LANDED (10,39)/(10,58) GROUND MANA ECONOMY — authored spheres
      spawn through the shared ball machinery
      (CreateManaSphere512/2560 → model-39 ball, 512/2560 mana,
      unowned family 52); the MC2 action-0x29 tick column stays the
      module-doc APPROX (MC1 ball tick flies/claims). Test banked.
    - LANDED CLASS-15 SPELL TOKENS — `mgc_sim::mc2::tokens` (shared
      AddSpellXX ctor: sprite 77, box 768/768/1280, state 3M) + the
      swi_id state bump (0=inert cast slot, 1=pickup, 2=self-
      replenishing pickup, >=3=junk 253 — the shared class-12/15
      spawn case EF:33209) + the sub_68FF0 pickup tick (fall/clamp,
      f63&3 stagger, AABB scan, sound 18, replacement drop). Grants
      BANK into `Gen::mc2_spell_tokens` (bitmask, hash-transparent)
      until Phase 4.2 — APPROX: we despawn the collected jar (retail
      converts it into the wizard's live spell object; 4.2 restores
      the slot economy). SetSpell_6D5E0 data wiring + death scatter
      + word_0x2E_46 cast gate = 4.2. Pickup/replenish/junk test.
    - RESEARCH FAN-OUT (4 Opus agents, all reports in docs/traces/):
      1. **mc2-class10-high-band.md** — the whole unidentified band
         is the CAVE TERRAIN GENERATOR: cave-only, invisible,
         one-shot heightmap sculptors ticked to completion inside
         the ApplyEvents settle loop ((10,82) 6x6 room carve,
         (10,83) cosine dome, (10,84)/(10,85) cosine pit/hill pair
         via par3+word_10, (10,86) ambient drip — ALSO runtime-
         spawned ahead of the player every 8th turn, (10,81) swept
         ridge carver). DEFERS to Phase 4.5 caves wholesale. OPEN:
         (10,80) x3033 is a code-inert stub (creator+action write
         nothing) — needs a THING-data/baked-heightmap comparison.
         Porting caves must reproduce settle order + per-entity RNG
         for the baked-geometry goldens.
      2. **mc2-class10-m6-m9-m11-m28-m31.md** — (10,6) is the REAL
         standing fire (sprite 228, life 240, per-tick ch0 area
         heat via sub_11400, night light source; the (10,0)
         stand-in APPROX in scenery.rs is wrong on damage/light/
         life — port this creator to close it); (10,9) = the
         raise-land/apocalypse cosine dome (endgame machinery, par1
         = subspell selector consumed at load — importer must carry
         it); (10,11) IS (10,19) (ctor remaps model+action; ground-
         fire-spray singleton); (10,28)/(10,31) are LOAD-TIME
         terrain-authoring markers (road lines spawning (10,27)
         walkers / river strokes dropping (10,50)-family widths) —
         generate-pass machinery like the waypoint chains, not
         runtime entities. OPEN: (10,9) dome geometry helpers.
      3. **mc2-class10-m50-chains-and-tail.md** — (10,50) = a
         sub_49090 CHAIN GENERATOR (stageTag-gated walk → one
         (10,51) traveling damage-beam per leg, settle-ticked);
         (10,34) = the MC2 self-contained player-only teleporter
         (sprite-223 pad, facing-cone warp to par1/par2 tile,
         sounds 21/22/20 — DIFFERS from MC1's paired-portal arm:
         needs its own creator, not the MC1 arm); tail identified
         one-line each ((10,54) damage aura, (10,22) whirlwind,
         (10,17) meteor impact, (10,15) fire trail, (10,67) flood/
         quake, (10,71) fissure, (10,25)/(10,23) AoE blasts, (10,8)
         DEAD creator, (10,52) permanent castle anchor = the
         building fallback). OPEN: (10,76) fire-sphere ring needs a
         dedicated read (highest tail count).
      4. **mc2-class5-m10-doomsday.md** — (5,10) DOOMSDAY PYRAMID
         fully CLOSED (all 8 previously-OPEN helpers traced,
         port-ready): stationary scripted endgame boss, 16-phase
         state machine — summon table, projectile bursts, player
         tractor beam, the expanding crater carve (sub_56F10 disc
         sink) escalating to per-tick KillAllCreatures, climax
         scatters (10,17)/(10,9) + global extinction flag; damage-
         mailbox read but life clamped >= 8 (script-immortal);
         spawn gate = the doom-palette level bit. 6 retail checks
         banked in the doc.
    - ~~NEXT in 4.3 (research-informed order): (10,6) real standing
      fire (closes a wrong-damage APPROX on 973 records), (10,34)
      MC2 teleporter, (10,28)/(10,31)+(10,50)/(10,51) generate-pass
      chain families, the tail effects by count, (5,10) doomsday
      port (trace now port-ready), (10,9) after its dome-geometry
      helper pass; class-15 cast machinery + scatter land WITH 4.2;
      the cave band ((10,80..86) + (14,2) pillar) lands WITH 4.5.~~
      [ALL BUT THREE LANDED 2026-07-10 session 4 — see below]
  - **2026-07-10 SESSION 4 — THE CLASS-10 TABULAR REMAINDER LANDED
    (all workspace tests green, 118 passing; MC1 goldens untouched;
    clippy/fmt clean):**
    - LANDED (10,6) REAL STANDING FIRE — `mc2_spawn_fire6`/`
      mc2_fire6_tick` (effects.rs): sprite 228, life 240, per-tick
      ch0 area heat 50 (trees take a tenth — `building_tenth`), the
      6-step grow/shrink sprite machine, ~1/7 (10,14) shrink puffs,
      water extinguish, one last pulse on despawn. CLOSES the
      scenery.rs (10,0) stand-in APPROX (973 records); the tree
      flame now spawns the real creator. Dynamic light = 4.9.
    - LANDED (10,34) MC2 TELEPORTER — `mc2_spawn_portal` +
      `World::mc2_portal_tick`: sprite-223 pad, par1/par2
      destination tile (the shared (10,34) post-init already had the
      tile-center math), facing-cone (< 0xAA) warp via
      pending_teleport, sounds 21/22/20, persistent at maxLife 0.
      APPROX banked: rival warp (needs a pad near a rival), warp-out
      altitude row (trace OPEN-2), the sub_5C800(6) blue flash (4.9).
    - LANDED THE GENERATE-PASS CHAIN FAMILIES — mc2_author_chain
      generalizes the waypoint walk over {0x1C road, 0x1D waterpath,
      0x1F river, 0x32 fence}: (10,50)→(10,51) fence = one traveling
      RIDGE/damage beam per leg (raises a radius-3 disc +10..24/tick
      via the shared dig_cell chassis, ch0 damage 100 + sound 10
      every tick, settle-run inline at load); (10,28) road =
      sub_48400's coarse-Bresenham staircase of (10,27) strip
      walkers COLLAPSED to synchronous stamps (the new
      `mc2_ridge_stamp` = sub_46180: type-8 2x2 + 3x3 NW-SE reshade
      + night inversion; the sub_33F70 raise guard verbatim incl.
      its vacuous center-read quirk); (10,31) river = FAITHFULLY
      INERT (retail's carve consumer is a self-destruct stub —
      mc2-terrain-author-painters.md §3.4/OPEN-1; geometry rides the
      level header); subtype 0x50 = the (10,80) CAVE tube carver
      (identified: spawns (10,81), lowers floor + raises ceiling) —
      defers WITH 4.5.
    - LANDED THE TAIL-EFFECT BAND — `mgc_sim::mc2::tail`: (10,52)
      anchor (sprite 205, immortal, empty EV case), (10,8) known
      no-spawn, (10,25)/(10,23) one-shot blasts (type 3 par-amount /
      type 0 amount 25 + sound 24), (10,17) meteor (300/tick, the
      damage-suppressed (10,0) ring visuals, ring cycle (f26+2)%11,
      sound 30), (10,15) fire trail (wander, 8-water-tick death,
      drops (10,11→19) sprays), the (10,19) ground-fire-spray tick
      (the 11→19 ctor remap honored; smoke rings on odd life; ch0
      200/tick; word_0x33 singleton APPROX), (10,54) aura (squared
      range 0xC40000, mail[4] first-come tag, magnitude
      min(dist,42)), (10,22) WHIRLWIND (11-node sprite-stacked tail,
      the wander+drag pass, the lift/spin/fling grab machine with
      F_GRABBED + F_STOP, 1000 mail per airborne tick, the 8th-tick
      castle-shake contact pass, teardown; PLAYER LIFT arm banked on
      the FlightVerb takeover seam — damage-only overlap interim),
      (10,71) fissure (±1 heightmap jitter disc ramping 0→15→0,
      664 beat every 4th tick + sound 10), (10,76) FIRE-SPHERE ORB
      (hub + 25 sprite-340 satellites in the 5-ring lattice, 5
      slot-0 damage carriers at 70, breathe 192..480 step 18, tumble
      +22/+16, collapse → (10,0) fire + chain teardown).
    - area_write now RETURNS THE HIT COUNT (retail sub_10C80/116A0
      contract) — the spellbook reports (4.2) and the (10,9)
      earthquake gate consume it; MC1 callers ignore it.
    - **THE HEADLESS-STATE BUG (player-assisted gdb hunt):** class
      10 has a catch-all dispatch arm (`10 => Gen::tick`) that
      DESPAWNS unknown states — the whirlwind tail (82) and orb
      satellites (84) died on tick 0, their heads kept dragging the
      freed slots via f54, `move_relink` re-linked corpses into the
      map grid, and slot REUSE (the orb's collapse fire) forged a
      next20 CYCLE — an infinite area_write chain walk (100% CPU).
      Fix: the MC2-gated no-op arm for states 82/84/0x38 (retail's
      strA0 NULL entries / empty EV case). LESSON for every future
      headless-entity port: retail "no handler" ≠ our fall-through —
      the class-10 catch-all eats unlisted states; give every
      chained/headless state an explicit no-op arm.
    - RESEARCH BANKED (4 more Opus traces in docs/traces/):
      mc2-class10-m76-fire-spheres.md (CLOSED the m50 OPEN-3),
      mc2-class10-m9-dome-geometry.md (dome radius FIXED at init —
      only height eases; SPELLS.DAT row 18 = model 9's par1 subspell
      table {120/240/480 dmg, 7/9/11 maxLife}, stride 80/26/offsets
      +0/+24 — THE IMPORTER MUST CARRY SPELLS.DAT; apocalypse-latch
      order confirmed), mc2-class10-tail-helper-closure.md ((10,67)
      = a terrain-morph flood w/ lava conversion + action-74 restore
      finisher; (10,71) stamp closed; whirlwind passes closed;
      sub_5C800 = palette flash, code 6 = blue),
      mc2-terrain-author-painters.md (roads/rivers/cave carver, the
      river-inert finding, (10,80) puzzle resolved).
    - ~~NEXT in 4.3 (order agreed with the player 2026-07-10, end of
      session 5): SPELLS.DAT IMPORT FIRST → (10,9) → (10,67) →
      (5,10) → (10,57)~~ LANDED 2026-07-10 (session 6), all but the
      (10,67) port:
      - **SPELLS.DAT IMPORT** — bundles carry `spells.bin` (26x80
        verbatim, BAKE_EPOCH 6, FORMAT.md row), parsed into
        `mgc_sim::mc2::spells` + FeatureAssets (hash-when-present;
        MC1 goldens untouched, MC2 slice re-pinned DELIBERATE).
        **KEY FINDING: the retail CD's SPELLS.DAT DIFFERS from the
        decompile's Spells.cpp fallback** (row 18 subSpell
        {400,800,1200} vs {120,240,480}; rows 16/17 likewise — life
        values match). The Opus authority trace CONFIRMED the CD
        file wins at runtime (ReadFileAndDecompress over the baked
        table, EF:42903; SetDefaultSpells never rewrites
        subSpell/life) — docs/traces/mc2-class10-m9-dome-open-
        closure.md §4. Also there: the rows-4/19 LevelInit patch is
        keyed to MapType (Day vs non-Day), NOT difficulty — tier-0
        life+hintText only, unported (4.2). The par1 overrides
        (EV:387-390, case list 9/0xB/0xF ONLY — (10,17) has NO
        authored override) landed in the spawn-seam post-init:
        subSpell always, model 9 → maxLife, 11/15 → life; test
        mc2_par1_spells_overrides pins the CD values end to end.
      - **(10,9) APOCALYPSE DOME** (`mgc_sim::mc2::morph`): the full
        three-phase machine — perimeter-MIN base, fixed radius
        (maxLife|1), raised-cosine grow with 1/life easing,
        life==3 summit cap + child beat, finalize plateau
        (summit-24) + 2x2 cap + despawn. The sin table
        (Maths::sin_DB750[2560]) extracted verbatim into
        mc2::sin_lut by extract-remc2-tables.py; the Heron isqrt +
        its seed LUT ported exactly (the disc test is 2-D — the
        open-closure trace corrected the parent doc: NO z term, and
        sub_6D8B0 is wizard spell-XP credit, NOT an earthquake
        event). Per-cell writes ride sub_570F0 semantics
        (auto-flat sub_57450 set re-derived + tested, water-seal
        walk) into `mc2_add_building_region` =
        AddBuildingToTerrain_46570 (the unconditional-seed twin of
        mc2_retile_region — shading formula confirmed byte-shared).
        The apocalypse latch = World::mc2_apocalypse
        (hash-transparent while clear); the (10,18)/(10,91) summit
        eruption child is UNPORTED — note_misfit ledgered (the
        (10,5)-splash precedent). Test mc2_dome_raises_and_finalizes
        (plateau 140 / cap 124 / footprint / ledger). NOTE the
        center-tile trap: retail `(pos+128)>>8` on a tile-centered
        THING = authored tile + 1 — the cap sits at (tile..tile+1)².
      - **(5,10) DOOMSDAY PYRAMID** (`mgc_sim::mc2::doomsday`, an
        `impl World` — the machine drives world globals): the
        16-state script (sprites 341-345), footprint wipe (reuses
        mc2_building_clear_tile = the same sub_57390), the
        flatten-crater bit chain (sub_56F10 stamps via ring_cells +
        add-building recompute), kill-all + global life-140 reset,
        proximity devour, the damage mailbox with the IMMORTAL
        life-8 clamp, weighted summons (m0/m21/m25/m19 all ported;
        (9,0)/(9,9) bolts armed at the player; (9,3)/(9,26)
        misfit-ledgered), the tractor beam via the player_knock
        channel, and the CLIMAX: state 0xF spawns the (10,9) dome
        life=32/maxLife=11 and SETS mc2_apocalypse — the doomsday
        pyramid IS the endgame apocalypse spawner (sub_21030 case
        0xF from the dome trace). The doom-palette gate
        (byte_0x2FED2 & 2 = the night-fog gfx bit) = World::
        mc2_doom_level, set by the app from gfx_type & 2; the gate
        runs on the machine's FIRST tick (dis-0 spawns precede the
        setter — the spawn-seam ordering note). World also gains
        mc2_doom_meter (the HUD doom meter, banked for 4.9). Tests:
        mc2_doomsday_pyramid_extinction_script (integration: gate,
        crater, unkillable, rock ring) + mc2_doomsday_death_script
        (in-crate: clamp → state 3 → 12..15 → extinction + dome +
        latch). APPROXes cited in the module doc (anim-length
        timers, palette flashes, the spell-slot-8 death test = 4.2).
      - **(10,87) THIRD PUFF** (0x57 — the roadmap's "(10,57)" was
        the hex): m13's life roll + sprite 67 under its own action
        0x5E, smoke-family wrapper + dispatch + registry arms.
    - LANDED 2026-07-11: **(10,67) FLOOD/QUAKE PORT**
      (`mgc_sim::mc2::flood`) — the full three-action machine from
      the two traces (mc2-class10-tail-helper-closure.md §1 +
      mc2-class10-m67-flood-helpers.md, all three corrections
      followed): ctor sub_51730 (life 120 / subSpell 20000 /
      ±17-tile AABB / NO maxLife — deliberate), the sub_39E40 probe
      (≥225 open cells or a neighbour quake/under-construction
      building in 54x54 → despawn), phase 1 = the 18x18 4-CORNER
      MEAN sample (z = mean−64, ref = 32·(mean−80) — the mixed-unit
      f44 max quirk ported verbatim), phase 2 = the 30x30 sin-LUT
      crater morph (outer rim blend + inner dip, 1/countdown ease,
      burnable→lava via sub_57450, inline NW−SE+32 shading IN scan
      order, the 2x2 center h−h/cd drop → punches to 0 = the flood),
      phase 3 = the lava/AddBuildingToTerrain commit → action 73 =
      the shove hold (~most of the 120 life), action 74 = the
      restore finisher (settle toward the rim cosine + rand&3
      jitter, sub_439A0 mean-of-8 snap in the last 2 steps + the
      full phase-2 settle, castle release, despawn; life sits at 0
      so the `life & 3` gate fires every tick — 16 settle ticks).
      Helpers: sub_39FA0 filter ladder, sub_39B60 shove (26x26
      window, dist² < 3328² disc, force (3328−d)·128/3328 clamp
      [4,128] cap d, z pull, ground clamp), sub_3A200 (F_TOSSED +
      grab tag, source-RNG %7 → victim life+1 mail = the near-kill),
      sub_3A090 damage pass. KEY IDENTIFICATIONS beyond the traces
      (list-builder EF:39964-40075): retail `dword_38519` = the
      CLASS-3 list ⇒ the "model-2 object grab" = CASTLES (grab bit +
      word_0x30_48=30 = our f50 castle SHAKE + owner f40 + the
      20000 mail, NO owner immunity — your own castle quakes too);
      `dword_38527` = the class-10 MODEL-45 list ⇒ the "effect kill"
      = the quake ERASES village buildings (life=−1). ⚠ BANKED
      FIDELITY CHECK: mc2::tail's whirlwind CONTACT pass ported
      38527 as "generic class-10 effects" — per the builder it
      should be model-45 BUILDINGS (and 38519 castles it has right);
      re-trace sub_33710 on the next tail touch. Flag homes:
      F_TOSSED = flags bit 31 (retail byte[0] bit0 = the tossed
      latch — aliases our "active" bit, can't share), grab = the
      F_NO_CORPSE bit (authentic byte[2]&0x10 alias: quake kills
      leave no corpse). CompareAxisWithShift is XY-ONLY — the
      generic ent_overlap z-term would have voided the castle grab
      (flood z rides in HEIGHTMAP units); flood_overlap is the
      faithful 2-D test. Spawn seams: spawn column + known_thing +
      the TRIGGER-ONLY par1 seam (post-init gated on dis_id != 0 —
      dis-0 generate keeps ctor defaults per EV:387, a fired
      disposition consumes SPELLS row 20, correction #3). Player
      arm = the whirlwind precedent (knock-channel pull, 32000
      kill-scale mail on the close roll; z-pull/spin bank on the
      FlightVerb takeover seam) — APPROXes cited in the module doc.
      Tests: mc2_flood_quake_craters_shoves_and_restores (crater→0,
      castle grab/shake/mail/release, goat tossed, restore >40,
      despawn), mc2_flood_par1_trigger_seam (dis-0 vs fired), the
      burn_flags 0x7F0000 table pin. MC1 goldens untouched
      (sim-only, no bake).
    - LANDED 2026-07-11 (session 2): **THE MISFIT LEDGER SWEEP —
      CLOSED TO ZERO.** Instruments: examples/mc2census.rs (static
      THING census vs known_thing) + examples/mc2sweep.rs (boot all
      165 levels, fire every disposition, 64 ticks, harvest
      w.misfits() + catch panics). END STATE: **census 100.0%
      (69381/69381 records admitted), runtime misfit union EMPTY,
      0 panicking levels.** Three parallel Opus traces banked
      (docs/traces/): mc2-class10-m57.md, mc2-class10-m18-m91-
      summit.md, mc2-class9-m3-m26.md. Landed:
      - **(10,57) = the RANDOM-VALUE mana sphere** (sub_50130,
        action 0x3E; the old census note "third puff, already
        traced" was the hex/decimal confusion — 87=0x57 was the
        puff; DECIMAL 57 is a sphere-family member, list 38523 with
        0x27/0x28): mana = own-stream `% 0x7D0` = 0..1999; rides
        the shared ball machinery like 39/58 (action-0x3E physics +
        the AI-avoidance gate word_0x244_580 = the same shared-ball
        APPROX).
      - **(10,65)/(10,66) = one-tick WIZARD-DEBUFF STAMPS**
        (sub_50780/507C0 ctors; sub_38E70/38F70, actions 0x46/0x47)
        — the (9,20)/(9,21) lob impact payloads (in-session
        identification): backward kick (moveBoost −80) + grunt
        54..57 on the target wizard; 66 additionally mails its
        subSpell. Player kick rides player_knock; stagger-ramp/
        stun/tint channels bank on the FlightVerb seam (cited).
        The impact seam now routes (10, 17|22|23|65|66) to the real
        ctors (was: misfit + direct-damage APPROX).
      - **(9,3)/(9,26) = the METEOR SHOT / WHIRLWIND SEED**
        (sub_4D500/sub_4E180, sprite 76/320, row 60, the shared
        flyer core): the doomsday pyramid's case-9/8 summons now
        launch them (pos+640 fwd, z+768, aimed at the avatar,
        sound 15; impacts (10,17) subSpell 6000 fuse 10 / (10,22)
        subSpell 20 fuse 3). (9,3)'s action-3 wrapper lays the
        damage-suppressed (10,0) spark trail (2 draws/tick). The
        4.2 cast column reuses both (spells 9/21).
      - **THE DOME SUMMIT CHILDREN** (the (10,9) life==3 beat, was
        note_misfit): (10,18) = the ground-vortex eruption
        controller (sub_32A70 — pulse machine emitting (10,16)
        TORNADOES; tick-0 seizes the vortex/plume singletons
        word_0x31/word_0x33 = OUR MC1 VOLCANO erupting/plume
        REGISTERS, raises the persistent (10,19) fire column +
        one visual (9,0) bolt pitched −386 with impact (10,17));
        **(10,16) tornado-drag runs retail's sub_33110 = THE
        (10,22) WHIRLWIND DRIVER under action 16** — one new ctor
        (sprite 210, life 100..199, subSpell 200), zero new
        machine. (10,91) = the apocalypse MANA RAIN (sub_32CF0):
        3 thrown (10,39) spheres/tick with the exact 5-draw arming
        order, riding ball_tick's native dest_x/dest_y throw
        deltas; GATED on a 200-slot free cushion (cited deviation:
        retail expires rain spheres via byte[1]|=0x20 + life 140,
        a decay channel the shared ball APPROX lacks — ungated it
        would exhaust the pool). The 26-row XP flood = 4.2. Both
        runtime-only (never authored).
      - Ledger hygiene: the (10,80..=86) CAVE-GENERATOR band
        admitted as known no-spawn records (identified + deferred
        wholesale to 4.5 — the honesty signal is the 4.5 tracker,
        ~6.9k rows stop reading as misfits); the whirlwind CONTACT
        pass re-traced and FIXED (sub_33710: dword_38527 = model-45
        BUILDINGS not "generic effects", castles get the owner
        stamp f40, overlap = the shared XY-only mc2_overlap_xy).
      - Tests: mc2_summit_vortex_erupts, mc2_summit91_rains_mana,
        mc2_debuff_stamps_mail_wizards + the dome slice test
        updated (ledger-clean + the persisting fire column).
        Workspace 214 green, clippy/fmt clean, MC1 goldens
        untouched.
    - RESEARCH BANKED 2026-07-11 (session 2, player-directed:
      research only, PORT NEXT SESSION — just before the 4.5 cave
      levels): **THE MC2 CASTLE COLUMN** (today: the MC1 castle
      object serves MC2 worlds — the known cross-column stand-in).
      Two Opus traces: docs/traces/**mc2-castle-builder.md** +
      **mc2-castle-runtime.md**. Delta headlines:
      - Core = class-3 model-2 SAME as MC1 ✓, field homes match our
        column (level→f26, build sub-state word_0x2E_46→f59,
        capacity→f136, BUILD00 row byte_0x46_70); the SAME m41/m42
        leveler/painter pair — but MC1's f59 one-handler machine is
        MC2's THREE actionIndices: 4 = standing tick
        (EndOfCastleProjectile_5F8F0 EF:61055), 5 = build/repaint
        SM (BeginOfCastleCreation_5FA70 EF:61123, f59 cases 0..6),
        6 = destroy-one-level (sub_5FCA0 → sub_605E0).
      - Ladder sub_60810 (EF:61695): CAP {5000,8500,18000,38800,
        78600,158200,317400,3e8} — DIFFERS from MC1 at every level
        ≥ 1; HP = MC1's shape pre-scale × the Life-personality
        factor ((L×((factor<<8)+256))>>8, Life 256/factor 0 = MC1
        flat).
      - Intake sub_609E0 (EF:61733): ONE mail channel, straight
        subtract (no /10, no shield), lethal → action 6; the
        word_0x80_128 self-id channel = one-level-down; NO blast
        shake — on MC2 castles word_0x30_48 is a PROJECTILE TIMER;
        sounds: 30 downgrade / 10 upgrade only, no hit grunt.
      - NO balloons: castle mana = standing-tick absorb of
        overlapping owned model-39 spheres + the sub_60F00 census
        crediting (10,45) possessed buildings into dword_0x13C_316;
        overflow above cap EJECTS scattered spheres (sub_5FD00).
        Type-0 objective law confirmed identical (castle-gated, no
        debounce) — our numerator already matches.
      - Stage pieces = (10,79) chained on word_0x34_52 (f52), one
        per BUILD00 sub-part (sub_613D0 EF:62233);
        RemoveCastleStage_385C0 (EF:28071) = the TERRAIN RESTORE +
        mana scatter, not the piece remover; the (10,52) anchor is
        NOT part of the castle chain. Guard (5,15) spawns INSIDE
        the intake driver sub_5FF50 (EF:61488, per allowance slot,
        16-tick cooldown) — our ported guard rewires there.
      - Creation sites: cast = spell verb 2 (sub_15730 case 2
        EF:6820; RE-CAST on an owned castle = THE UPGRADE TRIGGER,
        sub_60480: level++, +1 castle XP, ladder, piece rebuild),
        level-load EF:43779 (stamps levels 0..N-1 via sub_36FC0),
        the terrain-authored "Bad Stone" variant sub_4AA40
        (EF:33362, maxLife 40000), and a (10,32) castle SEED
        (sub_4FA60 EF:36292, par1 → BUILD00 row on trigger-spawn)
        — none authored in the 165-level census (runtime paths).
        Downgrade sub_605E0: one level + 10% stored-mana haircut
        SCATTERED as (10,39) spheres + occupant ejection + terrain
        restore; level 0 → unbind + despawn. Guard allowance by
        level = {2,2,2,2,2,3,3}.
      - ⚠ RECONCILE AT PORT TIME: the fresh quake/whirlwind castle
        writes (f50=30 + ch0 mail per the flood trace VERBATIM) —
        f50=30 is retail-faithful but on MC2 castles it means the
        turret/projectile timer (return-fire suppression?), not
        MC1's shake; and the runtime trace claims the grab should
        ride the word_0x80_128 level-down channel — CROSS-CHECK
        against the flood helpers doc §4 before changing anything.
    - LANDED 2026-07-11 (session 9): **THE MC2 CASTLE COLUMN PORT**
      (`mgc_sim::mc2::castle`, dispatch game-keyed in mc1/world.rs).
      - Research first (player-directed Opus fan-out): TWO new
        traces — docs/traces/**mc2-castle-open-items.md** (the §9
        closure, with THREE CORRECTIONS to the banked castle
        traces) + **mc2-castle-data-tables.md**; correction banners
        added to both older castle docs.
      - THE TRACE CORRECTIONS (all decompile-verified in-session):
        (1) `word_0x80_128` = the **UPGRADE-request** channel, not
        a downgrade — its writer is the delivered castle cast
        `sub_389F0` (EF:28240) which also writes `word_0x7C_124 =
        10`: the EXACT MC1 ch5 `(10, owner)` token protocol, so
        the MC1 (10,43) token serves both columns verbatim (our
        mail[5] home). (2) `dword_38519` = the CLASS-3 live list
        (EF:39975) ⇒ the flood grab DOES hit castles — **the flood
        port stands unchanged**; the grab's `f50=30` is consumed by
        the standing tick as the settle timer (branch A skips
        intake — the "mailbox accrues during the shake" MC1 shape;
        holds at 1 while grabbed, repaint on release). (3) MC2 HAS
        BALLOONS: class-3 action 9 = `AddBallon_60AB0` (EF:61763),
        fleet quota `sub_60400` = MC1's table verbatim ((1,0)..
        (3,34)); the runtime trace's "no balloons"/"townsfolk" and
        the builder's "{2,2,..} guard allowance" were misreads.
        (4) The (10,32) seed = a class-10 PROJECTILE (`sub_344A0`),
        never authored — dead path, not ported. (5) `sub_5F890` =
        the HUD build-ghost sync (no occupant ejection exists).
        (6) The (10,79) stage piece = `sub_508E0_castle_defend_
        create` → action 0x56 `sub_3AF00` — the DEFENDER LAUNCHER.
      - PORTED (mc2/castle.rs, ~700 lines): the THREE actionIndices
        (4 = `EndOfCastleProjectile_5F8F0` standing tick w/ settle
        branch + even-tick heavy work; 5 = `BeginOfCastleCreation`
        f59 SM — state 5 leveler = proven dead code; 6 =
        `sub_5FCA0` destroy w/ free-slot gate + 5-tick settle);
        `sub_609E0` straight-subtract intake + upgrade channel;
        `sub_60480` upgrade (+1 castle XP banked to 4.2);
        `sub_605E0` downgrade (10% haircut scatter + RemoveCastle-
        Stage model-0 unstamp + ladder + death/unbind at 0);
        `sub_60810` ladder (CAP {5000,8500,18000,38800,78600,
        158200,317400,300M}, HP shape ×Life-factor — identity 256/0
        until the factor table source lands); `sub_5FD00` overflow
        ejector vs (owner 13C bank + stored); one-sphere-per-even-
        tick absorb; `sub_5FF50` roster (balloon fleet w/ stagger
        retarget + `sub_5F810` chooser, dead/over-quota → mana
        spheres; (5,15) guard slots, 16-tick cooldown f44,
        courtyard +128/+640); `AddBallon_60AB0` balloon tick
        (tether 1024, castle ring delivery, `sub_60EA0` intake,
        behavior row 68 = base+9); the (10,42) painter
        `AddTerrainMod0A_2A_37BC0` (19-tick progressive cumulative
        rise rows 1..=lvl, `sub_45DC0(7,..)` paint every 7th tick,
        settle −1/−25 by f59, bit3→bit7 promote, parent f59=2 +
        self-despawn); the (10,79) piece ctor + dwell tick (launch
        AI banked to 4.2; geometry pending the DB038 dump); MC2
        (3,2) ctor site-datum delta (32×corner-mean over row 1).
      - Field homes: f50 settle timer, f59 build sub-state, f44
        guard cooldown, mail[5] upgrade channel, F_UPGRADE_ARMED =
        flags 0x40, painter parent = f40, piece back-link = f146.
      - Tests: **mc2_castle** (tests/mc2_castle.rs) drives the full
        cycle on level-000 — cast → MC2 painter stamps → level 1
        **cap 8500** (the ladder discriminator vs MC1's 10000) →
        1 balloon → token recast → level 2 cap 18000 → demolish ×2
        → castle gone, footprint unprotected. Workspace 214 green,
        MC1 goldens untouched, census 100.0% (69381/165), runtime
        sweep EMPTY, 0 panics, clippy/fmt clean.
      - DATA TABLES LANDED (same session, the second Opus trace =
        docs/traces/**mc2-castle-data-tables.md**): `x_BYTE_DB038`
        = a STATIC 52-byte array (EF:2594), dumped + decoded —
        MC2_STAGE_PARTS const in castle.rs with a decode-proof
        unit test (L1: 1 centred piece; L2/3: 4 corners of 21²;
        L4/5: 4 of 35²; L6/7: 8 of 48²). **KEY FINDING: the
        (10,79) pieces are RESEARCH-GATED** — `array_0x24E_590` is
        filled LAZILY by the castle-research child `sub_69AB0`
        (EF:56120-21) from SPELLS.DAT (`[lvl]` = HP factor ←
        subSpellIndex_2, `[9+lvl]` = part-type ← life_0x1A); a
        fresh castle is all-zero ⇒ HP factor 1.0× and NO pieces —
        so our pre-4.2 piece-less castle is RETAIL-FAITHFUL. The
        sub_613D0 walk-down + part-type plumbing (piece f67) is
        wired; 4.2 only needs to fill `mc2_castle_part_type`.
        Life personality confirmed: human ALWAYS 256 (EF:43720);
        AI from map-header `WizardMapSettings.Life_0x3612F`
        (EF:43768).
      - BANKED: (a) authored MC2 starting castles —
        `player_0x2FED9[8]` = per-color castle LEVEL in the level
        header (BasicTerrain.h:77, consumer EF:43777/43789);
        needs the importer to carry the header field (rides the
        4.3b level grind with the MC2 rival records). (b) AI
        castle-HP Life scaling — same rival-record wiring. (c) The
        L7 extent quirk: BUILD00 row 7 = 1×1 (degenerate), retail
        reads it unclamped → a 3-tile extent at level 7; ported
        as-is, needs a level-7 golden. (d) The (10,79) defender
        launch AI (sub_3AF00 states 3..8 + sub_6DCA0 SPELLS casts)
        + castle research children = 4.2.
    - PLAYTEST-11 (2026-07-11, level-000 fresh run): **CASTLE
      BUILDER PLAYER-CERTIFIED** ("castle builder correct").
      Worklist + resolutions:
      1. **Own castle guards attacked the castle** (ramparts,
         shooting the flag; the flag took no damage — that part was
         the owner-immunity rule working). ROOT CAUSE: the (5,15)
         acquire scan ported as "no ownership filter — verbatim",
         but retail gates the class-3 walk on `id_0x1A_26 != own`
         (EF:15031). FIXED in m15_scan (+ the player-target arm
         gates on own != PLAYER_TARGET). The old trace note was a
         misread.
      2. **"Worms" condensed in a small area**: level-000 authors
         NO worm models — the census is goats(5,1)×33, flyers
         (5,3)×4, archers(5,4)×4, villagers(5,13)×28, firebugs
         (5,19)×6; the "worms" = the SEGMENTED (5,3) MULTIPART
         FLYERS. New Opus trace docs/traces/
         **mc2-m22-worm-steering.md**: the true worm family (5,22)
         has NO steering law at all in retail — heads cruise ONE
         fixed random spawn heading forever (ctor seed EF:34392;
         idle never touches roll; the serpentine spin is body-only)
         — and our port + per-entity heading seeds are faithful.
         REPEAT-PLAYTHROUGH CONFIRMATION (player screenshot: one
         worm fully stacked on coastal shallows): the (5,3)s ARE
         the worms — dis-probe: one on dis=4 (the final stage) +
         THREE on dis=34 ringed "around the mainland" at (90,37)/
         (51,32)/(53,147). Segments carrying INDEPENDENT health =
         authentic retail (children are byte-copies with own
         damage intake, EF:8710-19); the multi-dot map + stacked
         bars were OUR presentation bug — FIXED: the pose
         `segment` flag is now game-keyed (MC2 = actions
         0xB4/0xE8..0xEA, retail's own map-plot skip verbatim
         GameUI.cpp:1220; MC1 keeps 120 — which is MC2's (5,15)
         guard brain state, so the old shared test also wrongly
         hid idle guards on MC2 worlds). The HEAD-NEVER-MOVES part
         → RESOLVED by the Opus trace docs/traces/
         **mc2-m3-flyer-movement.md**: the (5,3) "flyer" is a
         misnomer — row 74 makes it a TERRAIN-GROUNDED WALKER
         (v_14=-64 sink, v_12=0 hover, no aerial arm; v_20 water
         refusal + die-on-water when boxed); the head DOES wander
         (30-tick cadence, mag 85..340) and children string out
         only while AWAKE (player within ~24 tiles) — asleep they
         snap onto the head every 4th phase = the condensed stack.
         OUR PORT IS FAITHFUL end-to-end (explicit warning banked:
         do NOT add a flyer arm / remove water refusal / force
         movement). CHECK-A run in-session: all four spawn tiles
         are passable land (nothing in the v_20 blocked mask) —
         not a doomed-data case. VERIFY NEXT PLAYTEST: watch a
         worm from close range — awake children should trail out
         behind the wandering head; if they stay stacked when
         close, revisit CHECK-B (the D2 block probe under the m3
         head's multi-step reach).
      3. **HP bars missing on structures**: LivePose life_frac now
         also exported for destructible structures — (10,45)
         dwellings, (10,52) anchors, (10,79) castle pieces (debug
         overlay H, both games).
      4. **The level-000 "spire + 4 granite blocks"** — REPEAT
         PLAYTHROUGH RESOLVED the identity split: the five (14,5)
         X-cluster objects at (214-218,211-215) are the SCROLL
         PICKUPS (player: "the scrolls live just past the spire",
         spawned by the final stage with a firefly trigger; retail
         UpdateScroll_59C80 EF:41158 = collect-by-touch, no damage
         mail). The spire + granite blocks themselves = (10,45)
         BUILDINGS (same structure, different footprints — player
         read). Their "bar never moves but they eventually die"
         = the PRODUCTION-COUNTDOWN-AS-LIFE quirk: the parked
         building's act_life = 1000 x rate (mc2_building_tick
         final frame; retail CompareEvent08_38B00 drains the SAME
         field on damage — authentic dual-use). FIXED: the debug
         bar now denominates a parked MC2 dwelling against its
         parked value (1000 x f140), so damage visibly eats it.
      5. **Settler nearest-building bias — DEFERRED EXPLORATION
         (player directive, repeat playthrough)**: the pick +
         cadence re-verified retail-verbatim (sub_23020 EF:14395,
         `% period / 2` EF:14291), and the 7b adjudication stands
         — but the player's RETAIL OBSERVATION still disagrees
         ("what you say clearly disagrees with what I can see in
         the retail game, and there has to be an answer as to
         why"). NOT a priority now. BANKED: a thorough end-phase
         exploration; the player may also consult the remc2
         disassembly authors. Candidate reconciliation hypotheses
         to test then: F1 tick economics (2-4x), the one-steer-
         per-40-ticks drowning, retail pause semantics (F2), and
         whether some retail build variant gates the march.
    - PLAYTEST-11 ROUND 3 (player clarifications + screenshot of
      the high-level castle):
      6. **THE WORM BLOB root-caused + fixed (APPROX, cited)**: the
         head moves fine — the 16 chain children ride EXACTLY on
         it because their link length (`word_0x36_54` = 65% of
         `particlesParameters_D951C[89+ci].speed_6`, EF:33846-58)
         is ZERO: the speed_6 column is zero for every chain-child
         row in the vendored table AND in the PRISTINE EXE inside
         game.gog (binary-verified in-session at image offset
         ~0x16d06c). Retail on-screen worms visibly trail (senior
         source) — the true spacing source is OPEN (banked with
         the settler question for the disassembly authors).
         Until resolved: zero links keep the head ctor's authored
         96 (a 16-segment worm strings over ~6 tiles). The
         previously-vacuous m3 chain-stretch unit test now bites;
         MC2 slice golden E re-pinned (DELIBERATE — the chain in
         its window carries the new f56).
      7. **THE CASTLE WALKWAY OFFSET root-caused + fixed
         (VERBATIM)**: retail's painter places each accumulated
         row at `center - (dim >> 1)` with scratch offset
         `D/2 - d/2` (EF:27798) — our `(D - d)/2` loses ONE TILE
         whenever the frame is even and the row odd, i.e. exactly
         the 48x48 stage over its 35/21-tile interior rings: every
         inner ring sat one tile toward -x/-y of the outer ring =
         the circled offset walkways, the squashed center tower,
         AND the archer respawn loop (guards spawned into
         wall-blocked cells; the all-four-blocked walker law
         killed them). Proven by an oracle diff (our law vs the
         verbatim retail loop on the real BUILD00 data: L1-5
         identical, L6 = 902 cells off). Painter frame also
         widened to the largest accumulated row (retail's L7
         repaint memory-stomps its scratch — the 1x1 row 7 —
         which we cannot reproduce; documented APPROX). The
         mc2_castle test now climbs to L6 and asserts guards
         SURVIVE on the walkways (discrimination-checked: the old
         formula fails it). Also verified verbatim while tracing:
         retail's accumulation consumes rows with w/h swapped
         (EF:27795-27810) — invisible for the square castle rows;
         noted for any future non-square use.
      8. Confirmed by the player: spire/blocks take visible damage
         now; the respawning "spikes" = faithful; scrolls placed
         correctly. BANKED (player will retail-verify in the
         post-porting playtests): own-possessed buildings take a
         small amount of damage when the FLAG is hit directly —
         check against retail whether owner immunity should cover
         the building's ch0 entirely.
    - **PLAYTEST-11 CLOSED — PLAYER-CERTIFIED 2026-07-11** ("worms
      fixed, or at least looking retail-like and behaving
      correctly, castle fixed, no complaints"): the castle column,
      the walkway fix, the worm spacing floor, the guard fix, the
      structure bars — all certified across three rounds.
    - NEXT: **4.3b LEVEL GRIND** (+ the class-15 cast machinery
      with 4.2 unlocking castle research/pieces/XP; F1 tick-rate
      calibration for the walker/flyer long-range character).
      Standing open-questions bucket for the disassembly authors:
      settler long-range behavior + the worm segment spacing
      source (both with exact divergence citations in the
      PLAYTEST-11 ledger).
      Class-15 cast machinery WITH 4.2 (the MC1 cast bridge = the
      other big "wrong table" family until then — the flood is
      bridged to MC1's Earthquake in the CTRL pane today); a sound
      stand-in id sweep (e.g. the objective chime 41) rides the 4.6
      per-game sound map; the cave band WITH 4.5.
    - LANDED 2026-07-10 (session 5): THE LEVEL-000 MISSION-CHAIN
      BATCH (player report: the script stalled after the archer-kill
      spell jar — the castle stage never armed and the firefly wave
      never came). Root causes + fixes, all retail-traced:
      (1) the type-7 kill objective lacked retail's CURRENT-cursor
      gate (:40827) — rows latched vacuously at load before their
      dis-gated targets spawned (row 4 = kill-the-fireflies
      pre-latched, so completing the castle row would have ended the
      level, SKIPPING the wave); (2) the m32 stage-gated switch now
      sets retail's ObjectiveDone_2 = 1 pause as it fires (:54371) —
      the skipped pass bridges the one-tick latch→cascade-spawn gap
      (new World field mc2_objective_pause, hashed with the stages);
      (3) the MC2 census seeds the world total at 1 per retail
      sub_61F50 (was MC1's intrinsic 1000 — skews the type-0 15%
      share); (4) (3,4)..(3,11) = the 8 wizard start-position
      markers (sub_4A820.., array_0x2362 writes, spawn nothing) —
      known no-spawn seam arms, misfit warn gone; (5) (5,15) = the
      CASTLE GUARD archer, PORTED from the m2-9-12-14-15 trace
      (ctor sub_4C1E0, own wander sub_24190 w/ 4-heading RNG probe,
      class-3 acquire scan, stationary (9,13) volleys w/ template
      subSpell + self-inherited xtype) — the castle guard respawn
      now routes per column (retail EF:61488; the MC1 creature under
      MC2 dispatch was the (5,15) misfit-despawn warn); (6)
      Mc2Stage.force = retail's external force-complete bit
      (:40737) + debug_complete_mc2_stage hook. Level-000 chain
      NOTE: row 3 is authored as type 0 = OWN CASTLE + banked share
      >= 15% of the world total (numerator = possessed (10,45) mana
      + castle stored — our banked already matches retail's 13C +
      castle.mana) — "build a castle" alone does NOT fire it; the
      village possession/building-mana economy (4.6) is how retail
      play crosses 15%. New regression test
      mc2_level000_mission_chain (mc2_slice.rs) drives the whole
      script: checkpoints → archers → jar → forced row 3 → 5
      fireflies + row 4 armed-not-latched → hunt → completed, zero
      misfits. MC2 slice goldens re-pinned (DELIBERATE).
      **PLAYER-CERTIFIED same day: full level-000 playthrough "a
      largely a success"** — castle stage fired (player banked
      mana into the castle; RETAIL MEMORY CONFIRMED the mana
      gate: their original run built the castle ON TOP of loose
      mana, meeting 15% instantly), fireflies appeared + died,
      the final stage released the DEMON-MOUTH EXIT + trigger
      point, and hitting it finished the level (both objects
      despawn — correct). Follow-ups from the run: (10,5) splash
      misfit x1 = the MC2 class-9 flyer water-despawn still
      carried the pre-effects-band "pending" note — now spawns
      the real mc2_spawn_splash (id-inherited, EF:62955-65);
      castle still builds with the MC1 object (KNOWN — the MC2
      castle item; "a really strange creation at level 3").
      **WORM SEGMENT SPACING — BANKED (player, 2026-07-10):**
      the (5,22) worms travel with their segments packed so
      tight the chain reads as a SINGLE POINT from a distance
      (a real but VERY short gap up close). Retail worms trail
      visibly spaced segments. Suspect the tail-follow spacing
      law in the m22 chain (mgc_sim::mc2::multipart /
      docs/traces/mc2-m22-worm-helpers.md — the follow step's
      distance constant or the per-segment catch-up cadence);
      re-trace the retail segment-gap law when picked up.
  - **4.2 SPELL COLUMN CORE — LANDED 2026-07-11 (PLAYTEST OWED).**
    Research: FOUR Opus traces banked same-day —
    docs/traces/mc2-player-cast-path.md (the full input→sub_5F660
    gate→sub_5F7B0 arm→effect-state→sub_6DCA0 chain, ALL 26 dispatch
    arms: 10 projectile spells, 16 direct-effect; tier selection =
    array_0x437 → SetSpell → byte_0x46_70), mc2-spell-xp.md
    (sub_6D8B0 award + the FULL award table; sub_6D9C0 SP level law
    = highest tier whose xpos1 ≤ banked+volatile XP; castle
    RESEARCH-GATE CLOSED: pieces/HP/CAP key on the castle ENTITY
    level dword_0x10_16 — one step per re-cast — spell XP idx 2 is a
    7-capped shadow), mc2-stage-engine-completion.md (the full
    objective-type switch 0..9, the UNPORTED StageVars layer = the
    4.1 gap, two port corrections), and
    mc2-class9-low-band-creators.md (subtypes 0x00-0x0D verbatim, no
    stubs). PORTED (mgc_sim::mc2::cast + world wiring, all tests
    green, mc2sweep clean):
    - Mc2Spellbook (str_611 subset) on World: volatile+banked XP,
      levels, selected tiers, quick-slots; hash-transparent while
      pristine (MC1 goldens untouched).
    - Class-15 manifestation economy: the collected jar IS the
      spell object (state 3M, hidden from draw views, slot KEPT —
      the bank-and-despawn interim closed); death scatter
      sub_5E310 implemented, wired at 4.6.
    - **SetDefaultSpells_5C0A0: every MC2 level starts with
      FIREBALL + POSSESS at 0 XP (player-retail note 2026-07-11),
      bound left/right** — the sim invariant at world init.
    - The cast chain verbatim: per-model re-arm/retrigger switch,
      the mana gate (fail sound 29), the armed cast window
      (word_0x2E_46 = word_0x18 duration; fireball tier-0 = 5 ticks
      = MC1's known fireball cadence), first-tick spawn + full-cost
      commit via the negative-delta stamp, pending-tier apply at
      expiry (sub_6D880), per-spell cooldowns.
    - sub_6DCA0: all 10 projectile arms (charged fireball 28/(10,76)
      + charged thunder 12/(9,9) on tier life; the 0x15/0x19
      payload/charge division) + creators for every player subtype
      (one parameterized ctor, low-band + flyers-band params);
      cast sounds v6 (9/23/15) + per-spell direct sounds.
    - Direct-effect spells on the Player channels: shield/rebound/
      invisible/beyond-sight/heal/speed-up armed-window semantics,
      teleport via the MC1 return-pair (APPROX), posses = (9,17)
      with a ch1-claim impact arm, summon (9,24) / mine (9,29) /
      alliance (9,25) / fools-mana (10,57) direct spawns; castle →
      cast_castle under the MC2 mana LADDER (GetSpellManaCost
      L:1729-55). metamorph/steal_mana/duel = gate+mana only,
      misfit-counted (visible gap).
    - XP: impact awards ride a drained Gen mailbox → sub_6D8B0
      (class-3 model-0 gate = the human until MC2 rivals);
      sub_6D9C0 level law + castle clamp 7 + tier clamp + sound-61
      notify; Mc2BookView (owned/levels/sel/xp/cost/binds) feeds
      the pane.
    - APP: the CTRL pane commits through PlayerCommand.mc2_select
      (retail action 0x1F/0x20); pane tiles/costs/tier-ceilings/
      hand binds read the NATIVE book; **MC2_CAST_BRIDGE RETIRED**
      (main.rs + ui.rs).
    - Tests: mc2_cast_column_laws (gate/SetSpell/ladder/pending-
      tier), pickup test = the manifestation economy, mission
      chain on the new debug_smite instrument (the old firehose
      kill loop was a marginal fight — strays → village offense →
      model-4 MILITIA flood the type-7 extinction predicate;
      authentic, separately owned). MC2 slice goldens re-pinned
      (DELIBERATE: the fireball+possess seed + jar slot economy).
    OPEN/banked from the traces: auto-aim sub_67CB0 scoring (player
    projectiles fly straight without a lock — the crosshair-lock
    feed); the charge-accumulation site byte_0x154_340; the
    multi-shot charged loops (twin fireball / ±113 lightning fork,
    EF:56604-56); metamorph/steal_mana/duel effect bodies; the
    castle sub_69AB0 build-queue cast; select-time hint text (4.9);
    banked-XP fold call site (sub_6DB50 a4=1); MP xpos2 ladder
    (with MC2 rivals). Stage-engine trace corrections applied:
    objective chime = sound 61 gated current-row/level-end,
    suppressed at cursor 0 (was a 41 stand-in); objective text =
    langindex[IndexLevelText + cursor] (4.9). The 4.1 stage-var/
    class-0 layer is trace-complete and unported.
    - **PLAYTEST-12 ROUND 1 (player, 2026-07-11) — 5 items, all
      LANDED same day** (2 new Opus traces: mc2-cast-input.md +
      mc2-autoaim.md; tests green, goldens re-pinned DELIBERATE,
      sweep clean; re-playtest owed):
      (1) initial fireball+possess CONFIRMED correct;
      (2) hand-side launch — the firing button recorded at arm,
      launch from that hand's muzzle; verbatim-CONFIRMED by
      `sub_68E50` (EF:55595: 256-unit step at yaw∓512 keyed on the
      byte[1]&1/&2 button bits, terrain-clip revert — exactly the
      MC1 law already in `World::muzzle`); the render-side hand
      animation mapping is a flagged 4.9 item;
      (3) AUTO-AIM PORTED (`sub_67CB0` + the `sub_68490/685D0`
      scorer, docs/traces/mc2-autoaim.md): one-shot acquisition on
      the projectile's first flight tick — model-keyed list scans
      (offensive: wizards→creatures→worm fallback; POSSESSION:
      spheres→BUILDINGS→worms — that's how the pulse finds
      dwellings; lightning: pitch cone 0x200, range speed·maxLife;
      0x10 yaw 0x100; cave-in grounded-only), cones 0x71, hard
      range 5120, score = on-axis² + (4·sin err)² (alignment ×16
      over distance), zero RNG; lock → f146 + snap, then the
      existing row-cap homing curves it. KEY RETAIL FACT: there is
      NO HUD reticle in MC2 — the aim feedback IS the projectile
      curving (the local-player sprite 42); a screen reticle would
      be an opt-in enhancement, not a faithful feature. APPROX
      register on the method (owner range = wizard row 59 v_28
      4096; awake gate f58; worm bucket; EF:54788 self-self
      distance = flagged decompile artifact, two-point form used);
      (4) HUD hand panels wired: the pane's pre-composited MC2
      icon tile + the availability meter (partial bar + affordable
      dots) off the native book's tier cost + the armed-window
      frame highlight (functional-first; faithful MC2 HUD = 4.9);
      (5) CAST CADENCE verbatim (docs/traces/mc2-cast-input.md):
      fire bits are EDGE per press (`HandleMouseButtons_18F80`);
      **the rapid-fire flag is `byte_0x3B_59 = (fontType_0x1B & 1)
      == 0` (L:1519)** — held re-fires only when the tier is RAPID
      and its window is live; in the CD table ONLY fireball tier 1
      ("Repeat Fireball") and lightning tier 0 are rapid — exactly
      the player's report. Gate corrections from the primary
      source: castle re-cast buzzes 29, lightning tier 1+ refuses
      while armed, possess re-press = the banked byte_0x3C_60
      release signal. BANKED: `byte_0x154_340` = a free-running
      time-since-last-cast counter (cap 200, EF:59991) copied to
      each projectile's dword_0x10_16 — consumer unknown, not
      modeled (would be pure hash churn); regression test
      mc2_cast_cadence_and_autoaim pins click-vs-rapid + the goat
      lock.
    - **PLAYTEST-12 ROUND 2 (player, 2026-07-11) — 2 items, LANDED
      same day** (1 new Opus trace: mc2-hud-hand-icons.md; suite +
      sweep green, goldens re-pinned DELIBERATE; re-playtest owed):
      (1) HUD hand icons — retail's hand panels use a DEDICATED
      26-icon BIG run at sprite 123+spell (`DrawSpellIcon_2E260`,
      GameUI.cpp:374, `SPELL_FIREBALL_BIG=123`), NOT the CTRL
      grid's small 97+ run; same MSPR/HSPR bank, lowres-vs-hires =
      the VGA flag picking the M vs H file pair per map type
      (LoadSpr_47160 L:1019-63). Wired via sprite_quad(123+spell);
      retail also has NO flight-view center reticle (POINTERS.DAT
      cursor is menu-only).
      (2) fireball flying THROUGH targets — TWO root causes, both
      closed: (a) our probe tested overlap only at the tick's end
      position where retail's `sub_10780` ray-marches the map cells
      along the flight — the chord is now marched in ≤128-unit
      sub-steps (movement itself unchanged); (b) **THE BIG FIND:
      the particle-param table's speed_6 column is legitimately
      ZERO in the shipped EXE — retail DERIVES it at load from
      each sprite bitmap's aspect (`speed_6 = width·rotSpeed_8/
      height`, the init pass EF:44870-44910, 255×255 fallback for
      broken bitmaps). This CLOSES the PLAYTEST-11 worm-spacing
      "provenance OPEN"** (the worm link length reads the derived
      speed_6, not the table 0). Ported as
      mgc_sim::mc2::derive_sprite_extents fed with the baked
      sprite-index dims (FeatureAssets.mc2_sprite_ext,
      hash-when-present; app + every MC2 fixture/example wired);
      mc2_set_sprite + the multipart 65% metrics consult the
      derived pair; the 96 worm floor stays only for dims-less
      unit fixtures. The player's diagnosis ("possession's aiming
      on the fireball") was disconfirmed against the trace —
      terrain-skim is shared retail flight law (EF:62947-53);
      the miss was the probe + the zero boxes. Regression: the
      cadence/autoaim test now pins the actual STRIKE (impact XP)
      under synthetic dims.
    - **PLAYTEST-12 ROUND 3 (player, 2026-07-11) — BANKED for the
      next session** (context-limited; leads pre-analyzed):
      (1) hand icons CERTIFIED correct.
      (2) aiming still distinctly off + the aiming crosshair shows
      nothing though aiming clearly exists. LEAD: the cast-input
      trace found `nextEntity_0x18_24`/`entityIndex2_0x1A_26` =
      the MOUSE AIM OFFSET (x_DWORD_180590/180594 → EF:38065-66)
      added to the launch angles — retail MC2 free-aims via the
      mouse offset from screen center; our port launches on the
      pose facing only and ignores the offset seam. Wire the aim
      offset + the crosshair feedback (the app's MC1 predictor
      instrument shows nothing for MC2 because the lock is
      projectile-side; either simulate the would-be acquisition
      per tick for the instrument, or draw the free-aim cursor).
      Also compare the fireball state's 34-capped initial turn
      (EF:63108-13) vs our full snap.
      (3) possession CERTIFIED correct vs retail. Fireball: (a)
      WRONG SPRITE — round MC1-looking instead of MC2's
      star-shape. LEAD: we set type86 = the PARAM row index (340);
      the actual bitmap is `param.word_0` (row 340 → 0x1CF = 463)
      — check how the app's billboard layer resolves MC2 type86
      (if it treats it as a direct sprite id, EVERY mc2_set_sprite
      consumer draws the wrong art; goats may be coincidentally
      close). (b) fireball still terrain-follows; PLAYER-RETAIL
      CERTAIN: only POSSESSION skims terrain — the fireball
      explodes on ground contact. LEAD: re-read `sub_65C20`
      (EF:63057) — the round-2 reading took its terrain arm as a
      clamp (the generic sub_65820 creature-bolt law), but the
      player-observed retail law says the fireball state
      detonates on terrain contact; the terrain arm is likely
      per-state (v20=1 impact vs clamp). Port per-state terrain
      law.
      (4) BOTH projectile spells fail absolutely over water:
      instant splash sound at cast, no projectile visible,
      regardless of player altitude. LEAD (near-certain BUG in
      proj.rs mc2_flyer_tick): our water arm tests
      `cap_bit(x,y) == 1` UNCONDITIONALLY — retail's splash
      (EF:62955-65) sits INSIDE the terrain-contact branch (only
      a projectile flying AT the water surface splashes; z is
      never consulted in ours, so any flight OVER water despawns
      on its first tick). Gate the splash on the z-clamp arm.
      "They aim into the water" likely = the same instant-despawn
      reading as aim.
    - **PLAYTEST-12 ROUND 3 — LANDED 2026-07-11 (same day), 2 new
      Opus traces; suite 126 green, goldens UNCHANGED (MC1 + MC2
      slice), sweep clean; PLAYER-CERTIFIED mid-session ("sprite
      correct, behaviour correct, killed goats"):**
      (2) AIM: the mouse-offset lead was DISCONFIRMED by the trace
      (docs/traces/mc2-mouse-aim.md) — `x_DWORD_180590/594` are
      written ONLY by peripheral devices (VR head-tracker/VFX
      puck/joystick, EF:49687-88); the plain mouse is device 7 with
      NO case in the read switch (EF:49653), so the offsets stay 0
      and pose-only launch is CORRECT. The real retail terms: the
      fireball's first flight tick turns YAW ONLY, capped at 34
      units (~6°), pitch SNAPPED (EF:63106-19) — ported in
      mc2_flyer_tick (actions 0/29; every other state keeps the
      full snap per EF:62907-13). Channel-A note banked for 4.4:
      retail mouse-X is a lean-to-turn yaw RATE off the ABSOLUTE
      screen-center offset, pitch absolute (EF:59610-52) — check
      the port's pose integration when the MC2 flight model lands.
      CROSSHAIR: retail draws NO reticle and NO moving cursor —
      the aim feedback IS the local-player projectile (sprite 42)
      curving; the app's P-class crosshair instrument now routes
      MC2-bound hands through `World::mc2_aim_preview` (the pure
      `mc2_aim_scan` twin of sub_67CB0 — the acquisition scan
      split into scan + mutating lock).
      (3a) SPRITE: the billboard resolution was already correct
      (param.word_0); the actual root cause: retail swaps the
      LOCAL player's FIREBALL to the star-shaped muzzle/aim sprite
      42 (`SetEntityIndex_49C90(v17x, 42)`, gated local && spell 0
      — EF:30291, index+frame only, extents stay row-340) — our
      mc2_launch doc promised it but never applied it. Ported.
      (3b)+(4) TERRAIN/WATER: full verbatim re-trace
      (docs/traces/mc2-projectile-terrain-water.md) OVERTURNS the
      round-2 reading — EVERY ballistic state DETONATES on terrain
      contact (the contact clamp only places the burst,
      EF:62954/63139: clamp → v14/v20=1 → impact); POSSESSION
      alone runs a PRE-move ground-raise (EF:63262-64) and skims,
      and has NO water arm at all; the water splash is NESTED
      inside the terrain-contact branch (EF:62956/63141) — flight
      OVER water never runs it (that WAS the round-3 (4) bug).
      mc2_flyer_tick now: possession pre-clamp before the chord
      probe, per-state contact → splash-or-detonate, life
      countdown only on no-contact. The two cast-column tests
      flew at z=612 UNDER the fixture's 3200 ground (masked by the
      old clamp law) — poses corrected to 3712; the strike pin's
      goat moved onto the muzzle axis (an off-axis goat now
      correctly grazes past under cap-34 + 5/tick row-64 homing).
      RE-CERTIFY NEXT PLAYTEST: water splash only on descent into
      the surface; possession skim overwater; the 34-cap launch
      feel; the C-toggle crosshair instrument on MC2 hands.
    - **PLAYTEST-13 (player, 2026-07-11 — round-3 fixes CERTIFIED:
      "can't spot any obvious mistakes with the visuals") — 3 new
      items, (1)+(2) LANDED same day, 2 new Opus traces; suite 126
      green + the strike test now pins the payload; sweep clean:**
      (1) FIREBALL DAMAGE ~0.4× retail (player calibration: settler
      11 hits vs retail 5, goat 7 vs 2) — ROOT CAUSE
      (docs/traces/mc2-fireball-damage.md): the fireball dispatch
      arm carried `payload: false`, so the projectile kept the
      new_event default f44=100 instead of the tier's
      subSpellIndex 250; retail copies the payload UNCONDITIONALLY
      in every effect-state skeleton (fireball sub_693F0
      EF:55864). Fix: DispatchArm.payload REMOVED, mc2_launch
      copies always. Trace also banked the full damage law: NO
      direct-hit write (all damage = the (10,0) fire's ONE-SHOT
      flat ch0 area write, byte[0]&2 latch, sub_10C80 →
      dword_0x5E_94 → life, no scaling); settler (5,13)
      maxLife=1000, goat (5,1) 600, no regen; 250→ settler 5 ✓,
      goat computes 3 vs player-reported 2 (small goat-only
      residue flagged OPEN in trace §6). PLAYER-CERTIFIED
      2026-07-11 (follow-up playthrough): damage rate verified
      correct — the bug is squashed.
      (2) SPELL-XP BAR in the CTRL pane flyout LANDED — verbatim
      law EF:22633-71: each UNLOCKED level box except the third
      draws 54×2 at (+6,+28); bg CLRD 0, fill CLRD 3840 (0xF00
      red); in-progress level fills (xp−xpos1[l])/(xpos1[l+1]−
      xpos1[l]) with xp = banked+volatile, passed levels full bar;
      SP xpos1 / MP xpos2. Mc2BookView grew the xpos ladder;
      SelectorView carries xp/xpos; solid-quad overlay on the
      pre-composited tiles.
      (3) WORM MOVEMENT — BANKED (player call) but RESEARCH DONE
      (docs/traces/mc2-worm-path-follow.md): the breadcrumb/
      path-history hypothesis is FALSE — no history buffer exists
      anywhere in retail. MC2 has TWO segmented worms: m22 = a
      rigid head-anchored spiral COIL rebuilt every tick
      (sub_271D0 — never traces the head's path), m0/m3 = an
      immediate-parent follow-the-leader chain (sub_1B6B0) whose
      snake look is emergent and whose partial-freeze is the f58
      awake gate (asleep = every-4th child snaps onto its leader,
      EF:8729). BOTH port arms + the f54 chain propagation
      (sub_68C70) diff FAITHFUL arm-for-arm. OPEN: which worm the
      player fought (ASK: trailing segmented worm vs spinning
      coil?); if the freeze reproduces, prime suspects = the awake
      re-arm cadence in live play or the m22 coil-spacing sprite
      dims — NOT the follow law. Do NOT add a breadcrumb.
    - **PLAYTEST-13 ROUND 2 (player, 2026-07-11 — items 1+2 above
      landed; "otherwise a satisfactory session") — THE GHOST
      FIREBALL, LANDED same day:** every non-basic MC2 cast (heal,
      shield, invisibility, ...) also launched a fireball. ROOT
      CAUSE: the G dev toggle's `set_dev_spells` ran the MC1
      grant loop, and MC1's `grant_spell` AUTO-FILLS the MC1 hands
      (player.left = MC1 fireball) — and the MC1 hand-cast arm
      still ran on the MC2 column, so every fire press cast BOTH
      columns (the basics looked fine because the MC2 fireball
      visually coincides with its ghost). TWO GATES:
      `grant_spell` no-ops on GameId::Mc2 (no MC1 class-12
      manifestations on the MC2 column — dev/plausible instruments
      grant through mc2_dev_grant), and the MC1 hand-cast arm is
      column-gated (since 4.2 ALL MC2 casts ride mc2_cast_input).
      Regression: mc2_dev_spells_cast_no_mc1_ghost. Two bridge-era
      integration tests ported to the NATIVE cast (mc2_castle now
      casts book spell 2 dev-granted; the mc2_slice D/E combat
      legs pulse the seeded LEFT fireball's click-cadence edge —
      slice D/E goldens re-pinned DELIBERATE, post-init..C
      unchanged; MC1 goldens untouched). BANKED (player call): the
      full per-spell testthrough — support past the projectile
      band is thin (metamorph/steal_mana/duel are known misfit
      no-ops; "meteor shows no effect" to re-check now that the
      ghost is gone).
    misfit ledger + fallback telemetry as the checklist, fix
    deviations from expected behavior level by level (the
    goldens/probe methodology per fix).
  - 4.1 Stage-engine completion: multi-stage progression +
    checkpoint chaining, the stage-var reaction pass (:4961), the
    class-0 Conditional Spawn machinery, objective messages. (The
    single-stage core + the (11,32)/(11,4) stage-gates landed in
    Phase 3.5.)
  - 4.2 Spell-XP system (the flagship): 3 levels/spell as
    decorators on events combat.rs already emits; spell-collapse
    ladders (Fireball → Repeat → FIRESTORM); cast-cost scaling per
    level (retail-check).
  - 4.4 MC2 FLIGHT MODEL (deferred from 3.4): the row-driven climb
    law + extended player state (strafe/boost/slow/mobilize
    channels, full-stop) as the real FlightVerb::Mc2 arm.
    **CORE LANDED 2026-07-12 (workspace green, MC1+MC2 goldens
    UNTOUCHED, clippy/fmt clean, sweep 165 levels 0 panics 0
    misfits; PLAYTEST OWED — the faithful arm runs under the mc1
    thrust-model config on MC2 levels; the player's default enhanced
    model got the debuff channels only). Trace =
    docs/traces/mc2-flight-model.md (Opus, same day).**
    - TRACE CORRECTIONS (headline): (1) the carpet's tuning row is
      **66 open / 104 cave, NOT 59** — AddPlayer_4A920 (EF:33329-32)
      overwrites the generic default; real constants: climb band
      1024/3072, clearance 256, buoyancy −16/−8 (survey line ~448
      corrected in place; the old "row0xa = 1792" was row 59). (2)
      `moveTest_5D0A0` is the PLAYER FLIGHT gate, not the walker
      steer gate (mc2-cave-ceiling-sim §4a mislabel). (3) The
      speed-zero + spell-cancel on refusal (EF:59599-605) runs on
      CAVE refusals only — open-level water blocks return early. (4)
      The cancelled spell is `SpellEnabled[3]` = MC2 SPEED-UP (the
      accelerate channel), not possess — hitting a cave wall kills
      your speed boost. (5) MC1 floor clearance 128 vs MC2 **256**
      (row 0xc).
    - LANDED: `flight::mc2_move` (sub_5D530 statement order — pose
      with (4−moveSpeed) slow-scale, ±16 chase, the row-0xa climb
      ramp, forward/strafe/knock polar steps, displacement mailbox,
      debuff decay walk, the two-part vertical law: mobilize −51
      settle / row-0xe buoyancy sink / floor ground+256 / cave roof
      ceiling−384 with the round-2 floor-max guard) + `Mc2Row`
      (66/104) + `Mc2Ext` (slow/stun/mailbox/water/nudge channels) +
      7 unit tests; `Gen::mc2_flight_gate` (moveTest_5D0A0 verbatim:
      deep-water two-cardinal slide via radix_tan/dist3/arc_err,
      cave free-commit at headroom < ceiling−576 unsealed, the
      6-probe ±512 widening steer-search radii 16/32/64/112/176/256
      with the ±(17·i)/6 yaw assist, final seal check) +
      `mc2_flight_stuck` + the sub_5DD50 un-gated 128-unit nudge;
      World hooks (player_mc2_gate/stuck, mc2_carpet_row,
      mc2_cancel_accel, take_mc2_debuffs); lib.rs `move_mc2` arm
      (game-keyed dispatch via verbs.flight, accel-expiry edge,
      extended-lift = lift-key only — the faithful row-0xe buoyancy
      IS the idle settle on this arm); the (10,65)/(10,66) debuff
      stamps now QUEUE slow/paralyze hits (`Gen::mc2_debuffs`,
      hash-only-when-pending so every pre-existing golden stands)
      and the ENHANCED mover services the web channels too (scale
      (4−ms)/4, full-stop + −51 settle). Carpet fov = 100 (params
      row 44 rotSpeed/2 — OPEN-3 closed); moveBoost = the existing
      player_knock channel (identical cap-128/−4/snap-4 law).
    - BANKED: the sub_5DE30 possess/tornado leash (worklist 7 —
      needs the grab spells targeting the player); the slow-web red
      tint + paralyze web overlay (presentation — app reads
      `carpet_mc2.move_speed`); OPEN-4 RNG note (MC2's tick rolls a
      cave-ambient/water-loop LCG, EF:59802 — presentation-side,
      unported; no world golden touches it).
  - 4.5 Cave levels: ceiling plane on Planes (the enum-less
    LOS/height variance), cave-steer commit-gate arm (:59566-81),
    cave bundles + BLDGPRM cave-presence bits.
    **RESEARCH COMPLETE 2026-07-11 — 3 Opus traces banked:
    docs/traces/mc2-cave-terrain-foundation.md +
    mc2-cave-ceiling-sim.md + mc2-cave-roster.md. Headlines:**
    - CEILING = second heightmap, built at load by MIRRORING the
      floor about `MapBasicHeight` (header byte_0x2FED3):
      `ceiling = MBH − min(floor, MBH)`; then ±3 jitter with the
      FIXED LCG seed 37487429 (open cells, row-major — all
      deterministic, goldens-safe). INVARIANT re-run by every
      terrain writer: ceiling>floor = OPEN (bit3 clear) else
      ceiling=floor−1 SEALED (mapAngle|=8 — OPPOSITE polarity to
      the non-cave open-sea bit). Sealed collides like water
      (EF:59861).
    - COLLISION margins: player flight CLAMPS at ceiling−384 (no
      bounce/damage); m0-bob BOUNCES at ceiling−256 vel −150;
      projectiles BOTH detonate-on-ceiling AND glide-clamp
      (per-state, like the floor law); walker cave-steer = free
      if headroom < ceiling−576 && !bit3, else steer to the
      clearer unsealed diagonal, else refuse+stop (EF:59515-606).
    - ROSTER: cave-only (2,6) bee = passive ground-pinned silent
      sprite (only reacts to being shot), (5,24) brute aggros the
      class-3 BUILDING list not the player, (14,2) pillar =
      floor↑+ceiling↓ column until sealed (GenerateEvents pass
      4), (10,86) drip runtime spawner = 2560 ahead every 8th
      turn, Cave-In spell 25 = cave-only (9,30)→(10,89) ceiling
      slam (terrain is the weapon, no direct HP write; 0 authored
      records); cave-EXCLUDED: (2,7)/(2,8), (5,27), (5,2)
      day-only; (11,40) = the one cave-only switch (behavior
      OPEN). Events.cpp has ZERO isCaveLevel branches — the cave
      roster is entirely ctor-gated.
    - CENSUS: 47/165 levels are caves (full list in the
      foundation trace §9; instrument tmp_cavecensus.rs kept);
      level 003 is already a cave; level-106 = 2-THING stub.
      BLDGPRM flag 4 confirmed = no-cave-raise (clear ⇒ ceiling
      +80 headroom bubble over the footprint).
    - PORT ORDER (agreed shape): (1) Planes grows `ceiling`
      (manual Hash impl, hash-only-when-non-empty — MC1 goldens
      untouched; ~56 construction sites) + map_type/MBH plumbing
      into World; (2) cave terrain init (mirror+jitter+invariant)
      in the feature pass; (3) the sculptor band (10,80..86) +
      (14,2) + the (10,81) tube carver in the settle loop, exact
      order+RNG; (4) wire the ~25 deferred "4.5" arms
      (riser/flood/morph/terrain_paint/castle) through ONE shared
      invariant helper; (5) movement/collision (flight clamp,
      bob bounce, projectile ceiling laws, cave-steer, balloon
      walk, absorb 2048); (6) roster gates live (bee/brute/drip/
      pillar) + Cave-In; (7) app ceiling render pass (fifth plane
      texture, fixed atlas tile 1, no sky) + mc2-cave boot; (8)
      ACCEPTANCE = a cave-level state-hash golden after settle
      (foundation OPEN-1) + a playtest.
    **SESSION 1 (2026-07-11) — STEPS 1-3 LANDED (suite 25/25
    green, MC1+MC2 goldens UNTOUCHED, clippy/fmt clean, sweep 165
    levels 0 panics 0 misfits):**
    - ORACLE FIX: mc2-genlevel never seeded `MapBasicHeight` from
      the header — every pre-8 cave bake mirrored the ceiling
      about the weak default 44 AND baked a wrong sealed-bit mask.
      Now `level[0x05]` on caves (level-003 = 114, verified). The
      mysterious header `unk05` IS the cave basic height —
      renamed `basic_height` everywhere (serde alias for old
      bakes). BAKE_EPOCH 8: cave packages carry
      `terrain/ceiling.bin` (oracle plane +0x40000; omitted
      off-cave — all-zero there, sub_43D50 never writes it).
    - PLANES: fifth plane `ceiling` (manual Hash,
      hash-only-when-non-empty — the FeatureAssets pattern; MC1 +
      MC2 non-cave streams unchanged, verified). `Rec` gained
      `par3` (the THING's third param) with a manual Hash
      EXCLUDING it (static input; keeping it hashed moved every
      MC2 golden — the session's one golden scare, root-caused
      via bisect). `World::is_cave` = ceiling presence (the
      enum-less DATA variance verbs.rs planned).
    - `mgc_sim::mc2::cave` LANDED: `cave_seal_fixup` (THE
      invariant), `cave_box_jitter` (sub_43C60 — fresh fixed-seed
      LCG per call), `cave_wall_ring` (sub_34B00), the perimeter
      min/max floor/ceiling quads, ctors + verbatim ticks for the
      whole band — (10,80) inert marker/0x57, (10,81) tube carver
      /0x58 (32-sample rolling midline, eased nibble radii, torus
      wrap), (10,82) box mesa/0x59, (10,83) cosine dome/0x5A,
      (10,84)/(10,85) pit-hill pair/0x5B-0x5C (par3-or-RNG depth),
      (10,86) drip ctor+tick/0x5D (runtime spawner = roster step).
    - WIRING: spawn arms + sub_4A310 post-init (dome radius =
      word_10; pit/hill radius + par3 depth + the −128 recentre) +
      the (10,80) chain-author case (packed prev|next par3 nibble
      radii) + `mc2_settle_cave_band` after EVERY generate pass
      (slot-order rounds ⇒ phase-0 samples before same-pass
      writes, exactly ApplyEvents EV:410-526; drips = the settle
      DISABLE band — despawn unticked; dead slots reaped so later
      passes reuse them) + runtime dispatch arms 0x57..=0x5D for
      disposition-fired records. ids.rs: the band is no longer
      "known no-spawn" — it spawns.
    - VERIFIED: tmp_caveprobe (kept) — levels 030/003/073 carve
      4.3k-8.5k ceiling cells at boot; the BAKED foundation has 0
      invariant violations; post-settle violations (1.5k-4.7k) are
      the KNOWN missing load-time cave arms — the building raise's
      +80 headroom bubble (bldgprm flag 4), the road/path ridge
      stamps' ceiling bump, the sub_45DC0 retile enforcement
      (TR:1875-1912) — i.e. exactly worklist step 4, now ordered
      FIRST among the deferred arms.
    - ~~NEXT (order): (a) the load-time deferred arms above → probe
      violations to 0, (b) the (14,2) pillar (sub_5B100, pass 3),
      (c) movement/collision ceiling laws, (d) roster + Cave-In,
      (e) app ceiling render, (f) the cave golden + playtest.~~
    **SESSION 2 (2026-07-11) — STEP (a) LANDED: probe violations 0
    on ALL 47 cave levels (suite 25/25 green, clippy/fmt clean,
    sweep 165 levels 0 panics 0 misfits, MC1 goldens untouched):**
    - The load-time cave arms, all verbatim: (1) sub_45DC0 paint's
      2×2 quad invariant re-assert (TR:1875-1912 — the early return
      on the 4th cell's seal path is the same fixup); (2) the
      sub_462A0/46570 shading-pass arm (TR:2034-2042) in
      `mc2_blend_shade_passes`; (3) sub_46180 ridge stamp
      (EF:31061-71); (4) SetHeightmapByBuilding_48B90
      (EF:32531-42) in `mc2_smooth_pad_edge`; (5) the BUILDING
      HEADROOM BUBBLE in `mc2_building_tick` — on caves, unless
      bldgprm flags & 4 (no-cave-raise), EVERY footprint cell (pad
      or not) lerps ceiling toward min(max(floor, base)+80, 255)
      per tick + invariant (:27349-73). The instant sibling
      (sub_36FC0, :27114-37) has no ported caller (sub_5C950 stage
      machinery — unported, noted in the mobs.rs register).
    - THE HIDDEN 4TH ARM (found via level-020, the one level with
      209 residual violations — no tunnel chains, ten (10,50)
      ridge-fence chains): retail's `sub_56F10` = the shared
      chassis `dig_cell` carries its own cave arm (EF:39534-43) —
      the ceiling COUNTER-SHIFTS by the raw delta (dig down = roof
      up, saturate 255 high, u8-truncate below), invariant via the
      tail recompute. Ported into `dig_cell` (is_cave-gated, MC1
      untouched).
    - SUPERSET retile_and_shade: the MC1 chassis recompute
      (dig_cell's tail) gained MC2's twin DATA-variant arms — the
      non-Day shade INVERSION (retail sub_56F10 resolves through
      sub_462A0/46570 which invert on night/cave; ours didn't) and
      the cave invariant instead of the blind bit3 clear. Both
      no-ops on MC1 (flag false / ceiling empty). mc2_slice
      goldens re-pinned (DELIBERATE — level-000 is a night level;
      A..E move, post-init unchanged): every MC2 dig now writes
      retail-correct night shades.
    - tmp_caveprobe widened to sweep ALL cave levels (47 found,
      total violations 0; level-106 = the known 2-THING stub).
    - ~~NEXT (order): (b) the (14,2) pillar (sub_5B100, pass 3),
      (c) movement/collision ceiling laws (flight clamp −384,
      m0-bob bounce −256 vel −150, projectile ceiling laws,
      cave-steer :59515-606, balloon walk, absorb 2048), (d) the
      runtime deferred arms (riser/flood/morph/castle ceiling
      eases), (e) roster gates (bee/brute/drip/pillar) + Cave-In,
      (f) app ceiling render (fifth plane, fixed atlas tile 1, no
      sky) + mc2-cave boot, (g) the cave golden + playtest.~~
    **SESSION 3 (2026-07-11) — STEPS (b)..(g) ALL LANDED (suite
    green incl. the NEW mc2_cave golden, probe 0 violations on all
    47 caves, sweep 165 levels 0 panics 0 misfits, clippy/fmt
    clean, MC1 + mc2_slice goldens untouched). AWAITING PLAYTEST.**
    - (b) THE (14,2) PILLAR: `Gen::mc2_pillar_tick` (sub_5B100
      verbatim — measure/grow/retract on life 0/1/2, koef2 =
      2·par3+4, grow rate koef2/4, snd 47, the grow/retract bit3
      sync WITHOUT the ceiling pin); ctor via mc2_spawn_class14
      model-2 arm (cave-gated, life 0, maxLife left at the NewEvent
      default like retail); THING wiring par1→f44 orient,
      par3→f146 half-width (sub_4A310 case-0xE model-2); the
      MEASURE tick runs inside mc2_settle_cave_band (ApplyEvents
      brackets); the existing (10,63)/(10,64) triggers already
      matched model 2. 32 pillars measure at load on level-014.
    - (c) CEILING LAWS: `Gen::ceiling_z` (sub_10C60 — ground_z's
      exact bilinear ×32 sampler over the ceiling plane, callers
      cave-gated) + `cave_poke` (sub_11E70 margin 0) +
      `cave_collide` (sub_11E20 margin 384, hover resolved by the
      CALLER — row156 indexing differs per family; banked
      #[allow(dead_code)] for the 4.4 commit gate). Wired: player
      flight clamp ceiling−384 at the sim boundary (lib.rs, after
      extended lift so the deviation can't pierce the roof;
      World::player_cave_ceiling); MC1 wall-gate cave arm (sealed
      bit3 blocks like walls + the cardinal slide standing in for
      the 4.4 steer-search — documented); mc2_path_blocked cave arm
      (bit3 OR poke, EF:3674-83); m0-bob ceiling bounce −150 above
      ceiling−256; projectile DETONATE-on-ceiling (the comma arm at
      EF:62951/63136/63281 — ceiling−fov, floor wins when both) +
      possession's pre-move ceiling GLIDE clamp (EF:63265-70, its
      post-move contact only fires across a sealed gap); the 4c
      generic clamps — m21 hover (+f44 reset), (10,0) fire tick,
      whirlwind lift-and-throw victims, orb hub (margin = RADIUS
      f44 not fov); castle: balloon sphere-tether 1024→2048
      (EF:61793-96), the balloon CEILING WALK (sub_60D50: flags
      bit0 latch, attach on sealed/poke, 96 walk / 48 fly, snd 22
      behind a 32-tick f71 cooldown, ceiling−fov clamp while
      FLYING only), castle space-check bit3 arm (sub_11C80), (3,3)
      cave SetEntityShiftRot(256,768). NO PORTED HOME (noted): the
      MC2 sphere bounce arms (EF:26192/26479 — the 0x29/0x3E tick
      columns ride the MC1 ball APPROX), sub_3A8B0's clamp (the
      unported (10,78) shield), sub_5DD50's nudge (4.4 mover).
    - (d) RUNTIME MUTATOR ARMS, all verbatim: riser — instant-build
      4×N invariant blocks (EF:41535/41686), raise-tick invariant
      re-walks (EF:41957/42012), stamp clears gated !cave
      (EF:41838/41897), restore-strip invariant walks k in
      3..L−4 × [1,0,−1,−2] (EF:42253-42450); flood — the +64
      ceiling eases in the dome sweep (EF:28604-14) and the
      finisher settle (EF:28954-61) + flood_shade_cell's bit3
      SYNC-only arm (no pin, EF:28647/28985); (10,9) dome — the
      ceiling ease UP toward floor+64 (EF:23366-79, the roof keeps
      clearance ahead of the rising dome) + the every-box-cell
      bit3 sync (EF:23381-87). Terraform brush sub_377F0 = the
      DEAD-CODE (10,41) leveler (no port needed); sub_48D20
      ceiling smoother = zero callers in retail. The (3,0) wizard
      cave behavior row 104-vs-66 rides the MC1 rivals chassis —
      banked for 4.4/4.3b.
    - (e) ROSTER: (2,6) cave bee ctor+ticks (passive ground-pinned
      silent damage-target; 4 ent-stream draws life 100..179 /
      ±32 scatter / sprite 324..327; live tick sub_651B0 —
      byte[2]|=2 → flags 0x2_0000, mailbox death → corpse action
      19 + sprite+4 + ONE (10,13) puff; corpse sub_65240 = floor
      snap + water despawn); (5,24) cave brute ctor live
      (sub_4CCF0 verbatim — row 102, melee 1500, snd 7, aggros the
      class-3 BUILDING list via the shared idle scan; handlers
      were already ported); cave-EXCLUDED gates: (2,7)/(2,8)
      falling props + (5,27) m27 return None on caves; (5,2)'s
      night_shade gate already covers caves (the app folds Cave
      into night_shade). The (10,86) DRIP RUNTIME SPAWNER
      (sub_58630) at the tick head: every 8th turn (NEW World::
      mc2_turn counter — HASH-EXCLUDED, Rec.par3 precedent), 2560
      ahead of facing, 20×20 window from two GLOBAL-stream draws,
      cols step 11, the col offset ZEROES after the first row
      (retail v17=0), first empty non-sealed tile. CAVE-IN (spell
      25): the cast column already carried the arm (30, (10,89),
      charge) — landed the (10,89) ctor (sub_50A20, cave-only,
      action 0x60, life 40) + the sub_67910 post-impact fixup in
      mc2_proj_impact (maxLife = tier charge → ring base 3/5/7,
      phase reset) + `Gen::mc2_cave_in_tick` (sub_311E0 VERBATIM:
      6 rings +2 tiles each, sin_DB750 wave profile on f44
      227→1024 +22/tick, floor rubble-raise + ceiling drop,
      invariant with the ceiling pinned to the FLOOR — this
      variant's quirk; ~74 (10,13) rocks flung at wave 455, z from
      retail's stale last-swept cell; TRACE CORRECTION: the
      EF:23003-37 "burial" branch actually carves spherical
      SURVIVAL POCKETS around class-3 model-0 WIZARDS — floor
      down/ceiling up, not onto them) + the cave-only cast gate
      (EF:43883). Cave-In is an UPKEEP spell (maxManaLimit
      100k/150k/250k = castle-pool gate, duration 31/41/51): the
      dev-spells instrument now mirrors retail's OWN cheat flag
      (`OptionsSettingFlag_24 & 0x20`, L:1531-35 — f136=0,
      per-tick mana 1) so dev casts clear the gate authentically.
    - (f) APP CEILING RENDER: LevelView.ceiling (Option) → a
      second terrain draw with the ceiling heightmap in the height
      slot and twin globals (atlas.w = 1): fixed WALL texture
      (atlas cell 1 — the sculptors stamp tile_type 1 on carved
      walls), same shade/colormap pipeline, water animation off,
      painter plan-depth composites it like retail's ceiling
      raster; ceiling texture re-uploaded on terrain_dirty
      (World::ceiling_plane). mc2-cave bundle/boot was already
      wired (variant select + Mc2MapEnv::Cave + night_shade).
    - (g) GOLDEN: tests/mc2_cave.rs on level-014 (the
      roster-richest cave: 32 pillars, 61 brutes, 92 bees — the
      brutes/bees are ALL dis-gated, materialized by the storm
      like retail's switch column). Checkpoints: post-init /
      idle+drips / native Cave-In collapse / disposition storm;
      probes: whole-map invariant 0 post-settle AND post-collapse,
      pillars measured, cave-only spawns live, cave-excluded
      empty, drip cadence fired, collapse moved the ceiling.
      LESSON: most of a cave map is SEALED ROCK — park fixtures at
      an open_spot() (3×3 unsealed, headroom > 40) or casts
      detonate on the spot sub-tick (authentic).
    **PLAYTEST-CAVE round 1 (player, level 003 — "overall the
    impression is good") + the fix pass (suite green, clippy 0,
    goldens unmoved):**
    - (1) HOVER THROUGH THE CEILING — the clamp had landed only on
      the faithful `mc1_move` path; the player flies the ENHANCED
      thrust model (lib.rs float mover). FIXED: the cave roof is a
      HARD clamp on that path too (ceiling−384, vy zeroed upward —
      no altitude grandfathering under rock).
    - (2) MAP MISSING PITCH-BLACK COLLAPSED AREAS — retail's cave
      map variant (GameUI:2414-2443) draws SEALED (bit3) pixels as
      palette index 0. FIXED in map_pixels (cave = LevelView has a
      ceiling; sealed → palette[0], open → normal terrain color).
    - (3) CORRIDOR DOORS "don't trigger" — HEADLESS-VERIFIED
      WORKING (examples/tmp_doorprobe.rs, level 003 door at tile
      182,43): the level authors SIX dis-0 (10,64) RAISE triggers
      = the doors CLOSE during the first ~35 ticks of play (69/69
      sealed); the proximity switch (swi 34, box 5 tiles) then
      fires (10,63) RETRACT → door OPENS (31/100); the re-closer
      (11,1) leave-switch is chain-gated behind ANOTHER door's
      disposition (dis 40 → swi 42) — not every door re-closes,
      authentic. DIAGNOSIS: pre-fix the player could fly OVER the
      level (bug 1) — closed doors weren't on the flight path and
      trigger boxes were crossed at altitude/not at all; with the
      roof clamp + the sealed-bit3 wall arm the corridors are the
      only route. RE-TEST with this build; if the doors still read
      wrong in-app, the player's planned RETAIL COMPARISON anchors
      expected feel (door close/open cadence, switch box sizes).
    **PLAYTEST-CAVE round 2 (player, level 003 — pool exhaustion +
    door way-back + through-the-floor + sealed-area spiders) + the
    fix pass (suite green, clippy 0, sweep clean; mc2_cave golden
    DELIBERATELY re-pinned):**
    - POOL EXHAUSTION / "fireflies" — ROOT-CAUSED + FIXED, a TRACE
      CORRECTION: the m6-doc §0 claim "a (10,11) THING IS a (10,19)
      entity" is WRONG. Retail's creator row 0xB = NewAdd0A0B_4E840
      (EF:1715 → :35553) = the (10,11) SCORCH RING: action 11
      (sub_31FB0 EF:23490), maxLife 40, radius +1 every 3rd frame,
      area burn (full subSpell first tick, /25 after), the disc
      digs −3 per tick (sub_31F00 ≡ the MC1 chassis
      dig_disc_minus3 — exact same template walk), snd 10,
      invisible, one-shot. Our port had been routing all authored
      (10,11)s (level 003: 37) into PERMANENT (10,19) fire sprays —
      each pumping 4-puff (10,14) rings → ~790 live dust particles,
      pool saturated at t≈20 (reproduced via examples/tmp_poolprobe
      — KEPT). The (10,19) spray ctor stays for its REAL spawners
      (dome summit + vortex machinery). mc2_par1_spells_overrides
      updated (model 11 now carries the row-16 tier life itself);
      the volcano spell's (10,11) impact arm added to
      mc2_proj_impact (it had no arm at all — would have misfit).
    - THROUGH-THE-FLOOR at the door-edge slopes — FIXED: the new
      cave roof clamp could PIN the player under the terrain where
      headroom < 384+clearance (ceiling−384 below floor+128 —
      exactly the door slopes). Retail's sub_5D530 branch order
      only ceiling-clamps ABOVE the floor band; both flight paths
      now clamp to max(ceiling−384, floor+clearance).
    - DOOR WAY-BACK ("trapped in a side alley") — BANKED for the
      player's RETAIL RUN: the engine chain verifies headless
      (close at load → open on approach → re-closers chain-gated
      behind OTHER doors' dispositions per the authored data);
      whether retail re-opens a closed-behind-you door (repeating
      switches? a reverse-side switch we mis-consume?) needs the
      retail recording before touching the one-shot semantics.
    - SEALED-AREA SPIDERS ("trapped under the ceiling in the black
      area") — likely the god-view artifact (billboards visible
      over sealed-region meshes while flying above the roof,
      impossible after the clamp) — RE-TEST; if still visible from
      legal positions, the fix is presentation-side (billboard
      culling inside sealed cells), banked pending retail.
    **PLAYTEST-CAVE round 3 (player, level 003) — CERTIFIED: "this
    actually fixed all of the issues, inaccessible areas etc...
    I was able to get everywhere, trigger everything, kill
    everything. This was a major success."** The round-2 trapped/
    spider/way-back reports resolved with the scorch-ring + clamp
    fixes — the "systemic" root was the (10,11) mis-port distorting
    the level's whole economy/roster flow, not the door chains.
    - RESIDUAL (minor, banked): "still able to peek through the
      ceiling in some circumstances" — likely the camera near-plane
      clipping the low ceiling mesh at steep roof gradients (the
      eye sits legally at ceiling−384 but the frustum's near plane
      pokes the polygon), or an extended-lift edge. Presentation-
      side; candidates: near-plane-aware headroom margin, or a
      cave-only camera pitch clamp. Pair with the player's retail
      comparison when it happens.
    - ~~NEXT: 4.3b LEVEL GRIND / 4.4 MC2 flight+commit gate
      (cave_collide + the steer-search EF:59515-606 banked ready);
      the player's retail-comparison notes fold in as they arrive.~~
      [4.4 CORE LANDED 2026-07-12 — see the 4.4 entry above; the
      cave playtest across MULTIPLE cave levels is still owed
      (player, 2026-07-12: proceed with grind/flight meanwhile).]
    **SESSION 2026-07-12 — the 4.3b/4.4 trace-bank day (4 Opus
    fan-out traces, all banked same day):**
    - docs/traces/**mc2-flight-model.md** → the 4.4 port (LANDED,
      above).
    - docs/traces/**mc2-rivals-spawn-mortality.md** — MC2 wizard
      LIFECYCLE: sub_53160 (EF:38088) initialises ALL 8 colors (no
      per-slot active flag; human = color == LevelIndex_0xc claim —
      VERIFY at port time; NumberOfPlayers bounds only the input
      pump); entity spawn/respawn = sub_5C950 (EF:43600, (3,
      IsAiPlayer) — model 0 human / 1 AI); start positions =
      array_0x2362 from the (3,4)..(3,11) markers; personality
      words 578/580/582 + Life word_0x24A (scales maxLife) AI-only;
      book + starting spell-XP tiers = InitialiseSpells_54A50
      (EF:38650, third mask byte_0x360FBx[26] = per-spell level
      0..2); death chain action 2 = sub_5E310 (kill credit, 26
      spell-token scatter, (10,40) grave, sphere re-point, timer
      1200) / action 3 = sub_5E7C0 (castle respawn; castle-less AI
      = banished, feeds objective case 3). CORRECTIONS: the
      at-castle damage redirect is HUMAN-only in MC2 (AI keeps
      damage on itself — differs from MC1's discard note); respawn
      timer flat 1200 (vs MC1's tempo formula); heal rates human
      /250 home /2000 afield, AI /200 /500.
    - docs/traces/**mc2-rivals-brain.md** — the MC2 rival brain is
      the MC1 brain FUNCTION-FOR-FUNCTION (sub_12910 sandwich =
      housekeeping/selector/handlers; state byte_0x1C1_449; hate
      ledger array_0x1FC_508 neutral 0x601F; casting through the
      shared sub_5F660 router; burst gun + poverty latch verbatim
      vs our rivals.rs). PORT HAZARDS: the SPELL-ID REMAP (heal
      1→5, speed-up 2→3, possess 3→1, cloak 12→0xB, castle 16→2;
      offense set {0,7,9,0x10,0x12,0x13,0x14,0x15}); recast +
      attack-priority tables differ wholesale (x_WORD_D3F4C,
      unk_D3F80x/89x/91x — transcribed); NEW sub_16580 water/
      obstacle steer runs after every state handler (MC1 AI ignores
      walls — must ADD); learn-timer GONE (physical token pickup
      sub_15CB0 + Perception scroll roll; AI XP via the shared
      sub_6D8B0); free-instant runtime AI castle GONE (load-time
      authored only); brain cadence = own byte % (64−reflexes/4)
      throttle inside the 1/4/8 UpdateEntities bands; the "(3,0)
      cave row 104-vs-66" anchor = the flight tuning row (4.4),
      NOT a brain gate. OPEN: sub_169C0/16730 yaw tables, the
      pickup chain transcription, sub_583B0 scout metric.
    - docs/traces/**mc2-music-law.md** — in-game MC2 music = ONE
      LOOPING XMI from SOUND/MUSIC.DAT selected BY MAP TYPE
      (Night=1/Day=2/Cave=3 → maptypeMusic_0x235, EF:31441-49;
      struct offset 0x235 — the old "+576" note was imprecise);
      driver sections G/R/F/W, 6 tracks/bank, XMI (not HMP),
      possibly RNC-compressed. The 28 redbook tracks = the
      per-level OBJECTIVE VOICEOVER: one continuous track per
      level, sliced by the compiled segment table CdTracks_DB080
      [28]×10 (start,length in CD frames ×13.33 → ms); objective
      box plays (track=level number, segment=ObjectiveText+1).
      "Script cases 12/25" DO NOT EXIST as music opcodes —
      superseded. Our redbook-loop interim is wrong on both source
      and selection. AGREED PORT SHAPE (player, 2026-07-12): bake
      the voiceover as a TABLE OF SPEECH SNIPPETS (slice the CD
      tracks by CdTracks_DB080 at import into per-objective audio
      members) — do NOT replicate the segment-seek mechanism at
      runtime; music = XMI→SMF→the existing GM/fluidsynth pipeline,
      3 tracks by map type.
    - IDENTIFICATIONS closed same session (data cross-checked over
      the baked campaign, docs landed in mgc-import + mgc-formats;
      serde key renames deferred to the next BAKE_EPOCH):
      (1) header `unk09` (`word_0x2FED7`) = **NumberOfPlayers** —
      the REAL activation law: the lifecycle trace's "all 8 colors
      spawn" is gated by the input pump bound (EF:37567), so colors
      0..n-1 spawn; the human = color 0 in single player
      (LevelIndex_0xc = 0, EF:43127 — it's the LOCAL PLAYER slot,
      not a level number). Data: level-000 n=1 (no rivals),
      level-004 n=3, level-016 n=6, level-022 n=8 (five authored
      rival castles), level-067 n=8 with a HUMAN level-7 starting
      castle (players[0]=7). Authored castle levels on colors ≥ n
      (e.g. level-003 [0,2,..] n=1) = dead data, never spawns.
      (2) wizard-block `unknown_spells` (`byte_0x360FBx`) =
      per-spell STARTING XP LEVEL 0..2 — the rivals' spell-XP seed
      (EF:38693). (3) header `unk07` (`word_0x2FED5`) = runtime
      objective scratch, no load consumer.
    - ~~NEXT: the MC2 RIVALS PORT (4.3b groundwork — the two traces
      above onto the MC1 chassis; spawn colors 1..n-1 as rivals +
      the authored starting castles incl. the human's on 067); the
      MC2 music/speech bake; the multi-level cave playtest + the
      4.4 faithful-arm playtest; the player's retail-comparison
      notes fold in as they arrive.~~
    **THE MC2 RIVAL COLUMN — PORTED (2026-07-12, playtest owed):**
    `mgc_sim::mc2::rivals` (colors 1..n-1 from header unk09 =
    NumberOfPlayers; app wires `Mc2RivalConfig` from wizards.json +
    header players[]). LANDED:
    - LIFECYCLE: (3,1) carpets at the (3,4+color) markers (missing
      marker = origin, retail law), personality words + the Life
      scalar (wizard maxLife AND castle-HP factor — `Gen::
      mc2_life_scale`, hash-transparent at default), the book =
      per-rival [`Mc2Spellbook`] of class-15 manifestations granted
      LOAD-TIME ONLY (start && !blocked at the authored tier 0..2 —
      spellIndex_D94FF is identity, open-closure §4); AUTHORED
      STARTING CASTLES (AI-only — open-closure §7 settled the
      human gets NONE, level-067's players[0]=7 is inert; synchronous
      painter settle + extents + Life-scaled ladder + stage pieces +
      full mana clamp 320000).
    - BRAIN: the sub_12910 sandwich on MC2 ids (heal 5, speed 3,
      possess 1, cloak 0xB, castle 2), MC2 recast table x_WORD_D3F4C,
      MC2 attack-priority walks (unk_D3F80x/89x/91x) with the
      anti-buff branch (target-holds-8 → 7 at Perception%), the
      selector with DEFENSE as cascade step 7, the poverty latch,
      burst gun (cones 0xAA/0xE3, lockout (refl−255)/8−1), the
      combat WEAVE (3·minSpeed·refl/255) and the NEW WATER STEER
      (sub_16580 + the verbatim yaw/step tables, 40-step detour
      march, Bresenham LOS, the 1118/1119 micro-FSM); the REACTIVE
      ANTI-PROJECTILE DEFENSE (sub_15CB0 chain — the open-closure
      RETRACTION of the "learn-by-pickup" reading; no runtime spell
      acquisition exists); the Chebyshev scout gate (sub_583B0 =
      max(|dx|,|dy|) > 12288 — corrects the MC1-inherited 12288²).
    - CAST ARM: readiness (ceiling+cost+cone) → executor → the
      shared MC2 class-9 spawners (mc2_spawn_cast_proj, owner-tagged
      — homing/damage/impact-XP all serve unchanged); castle cast =
      the mail[5] upgrade token protocol / the direct (3,2) build
      spawn (NO free MC1-style plant); rivals earn spell XP through
      mc2_award_xp (rival arm added) and relevel with the authored
      tier as floor.
    - MORTALITY: death fall → class-15 SPELL-TOKEN scatter
      (re-collectible, rand%90+200) → (10,40) grave (inert census
      anchor, model-keyed no-op arm; retail action OPEN) → sphere
      re-point → FLAT 1200 respawn timer; castle-less = BANISHED →
      the objective engine's kill-player cases 3/8 (type-3 payload
      is 1-BASED color — InitStages' default arm `stage_1 - 1`).
    - REGRESSION: tests/mc2_rivals.rs (level-004 spawn/determinism/
      elimination→objective-completion end-to-end incl. rivals
      BUILDING castles and respawning; level-022 seven authored
      castles stand); mc2sweep now wires rivals on all levels — 0
      panics at 600 ticks; all goldens hold (hash-gated column).
    - BUG FOUND BY THE SWEEP (pre-existing, latent): `F_GRABBED`
      (whirlwind grab) and `F_MC2PROJ` both claimed flag bit 29 —
      the whirlwind TEARDOWN clears its bit over a radius-12 disc on
      EVERY entity, stripping the MC2-column marker off any passing
      projectile (fell to the MC1 handler; rival rows 61+ made it
      panic). F_GRABBED moved to bit 22.
    - OPEN/APPROX (flagged inline): the DEFENSE state body
      (sub_161A0/sub_15FC0 untranscribed — weave + reactive-table
      approximation), the cruise scroll-grab (spell 0x16, rides the
      class-14 pickup surface), heal channel rate (maxLife/20
      MC1-certified stand-in), the grave's retail action, steal-mana
      emission (no ported caster casts 0xD yet — open-closure §5
      transcribed for 4.6), the projectile hate-feed timing (intake-
      time, the MC1-column position), creatures still aggro only the
      human (the wizard-list widening carries over).
    - ~~NEXT: the 4.3b RIVAL PLAYTEST (a wizard-duel level, e.g. 004/
      022/067) + the MC2 music/speech bake; the multi-level cave
      playtest + the 4.4 faithful-arm playtest; the player's
      retail-comparison notes fold in as they arrive.~~
    **THE MC2 AUDIO COLUMN — LANDED (2026-07-12, BAKE EPOCH 9;
    ear-test/playtest owed):** music + voiceover, closing the last
    MC2 port column. Two same-day Opus traces + one in-session
    duration-fit proof:
    - docs/traces/**mc2-voiceover-triggers.md** — the complete CD
      speech law: `CdTracks_DB080[28]` dumped verbatim (int16 pairs,
      CD frames ×1000/75 → ms, truncating); exactly THREE call
      sites (map hover seg 0 / objective box seg `ObjectiveText+1`
      or 9 at level end / secret rows 25-26); the IN-GAME trigger =
      `byte_0x36E02` ramp (level load + current-row advance +
      type-31 beacon → ~7-tick delay, chime 41 at step 7, speech +
      chime 61 at step 8, quiet tail to 0xC8); music+sfx DUCK to
      1/3 during a line, fade back up; per-track rips need NO TOC
      correction (remc2's own SDL backend ignores TrackOffsets).
      POST-TRACE CORRECTION (duration-fit, 27/27 vs 11 violations):
      TrackIdx counts AUDIO tracks — table row r slices rip track
      r+2, row 27 = dead data (no disc track behind it).
    - docs/traces/**mc2-music-dat-xmi.md** — MUSIC.DAT ground truth
      (G/R/F/W × 2 banks; gameplay = G bank 1 "C1" set; the region =
      SIX CONCATENATED single-song `FORM XDIR…CAT XMID` containers,
      header slot data offsets shifted one slot = the retail ±1
      play skew; names in-memory order, NOT the decompile's shadow
      struct; no RNC in retail), the XMI→SMF law (summed-run
      deltas, embedded note durations → synthesized offs, strip
      cc110-119, division 60 + tempo pass-through, every sub-song =
      one whole-song cc116/117 infinite loop → loop the FLAC), and
      the DANGER-MUSIC VERDICT: MC2 HAS one — cc119-TAGGED channels
      (6/7/8) are WAR STEMS expression-zeroed in peace and ramped
      ±1 at 90 Hz on the `word_0x36_54=100` projectile-contact
      countdown (the MC1 v_46 law's twin; our `mc2_danger_poke` was
      already the exact arm). remc2's danger.ogg = non-canonical.
    - IMPORT: `mgc-import::xmi` (parser + mix-aware SMF encoder:
      Full/Ambient/WarStem by cc119 tag), `::mc2_music` (container
      walk, split on FORM XDIR magics — self-validating),
      `::cdtracks` (the verbatim table, header-generated);
      `bake_mc2_audio` now emits `music/mc2-{night,day,cave,menu}
      .flac` (+ `-danger` stems; GAME1/2/3=Night/Day/Cave by the
      maptype law, SETUP=menu; fluidsynth GM, shared-peak
      normalization) and `speech/level-RR-seg-S.flac` + speech.json
      (138 clips); the interim full redbook track-NN music members
      are GONE. Byte-verified: slice lengths match the table to the
      ms; container 0 opens at 120 BPM = GAME1.
    - RUNTIME: mgc-audio gains the SPEECH LANE (one-shot i16 stream,
      duck-exempt) + the duck (instant 1/3, ~0.7 s ramp-up APPROX)
      + per-game danger ramp rates (MC2 ±3/tick); the sim ports the
      `byte_0x36E02` ramp verbatim (`speech_ramp_mc2`, hashed with
      the stage board — MC2 goldens re-pinned, MC1 pins untouched);
      AudioFrame carries the segment cue; the app maps row=level
      (specials 30-34 → row 0/10 verbatim), config `audio.speech`.
      MapType music selection replaced the `%27` interim.
    - EPOCH 9 also carries the deferred renames: level.json `unk09 →
      number_of_players`, wizards.json `unknown_spells →
      starting_spell_levels` (serde aliases keep old bakes loading).
    - OPEN (all flagged inline): the type-31 beacon speech variant
      (`byte_0x36E0B` — waits on the beacon switch port; secret
      rows 25/26 unreachable until then), the palette flash at
      speech onset (presentation), the retail fade-up step rate
      (APPROX 0.7 s), the F-section FM render as a future
      faithful-alternate arrangement, INTRO/CUTS sub-songs unbaked
      (no cutscene track yet), MC1's own song-command trace (its
      `%3` interim still stands).
    - PLAYTEST CHECKLIST (the player is the ear oracle): maptype
      track identity vs retail memory (night/day/cave), the war-stem
      swell under fire + decay after, briefing voiceover at level
      start + per-objective lines ~7 ticks after the advance chime,
      the duck depth, level-complete line (seg 9), speech clip
      cut points (any word clipped = the truncation law needs the
      ceil variant), menu music when a menu exists.
  - 4.6 Corpse/economy + damage completion: sphere split/merge,
    wizard-death spell-token scatter, new intake channels, per-game
    sound map.
  - 4.7 Cross-import columns: tested MC2 arms become authenticity-
    matrix options in MC1 contexts ((GameId, local-id) keying).
  - 4.8 HW delta pass: Hidden Worlds deviations as a small override
    set on the MC1 profile — needs the per-game known_thing
    override point (ids.rs note).
  - 4.9 MC2 presentation fidelity (HUD/book/map) — joins the
    banked faithful-UI/UX track.
  EXIT: an MC2 campaign level completable start-to-finish with the
  spell-XP loop live, player-certified; MC1 goldens untouched;
  FIDELITY.md rows per ported system.

## NEXT-SESSION AGENDA (player, 2026-07-09): MULTI-GAME
## ARCHITECTURE — the variant/plugin system
## [RESOLVED — see "MULTI-GAME ARCHITECTURE — AGREED DESIGN +
## PLAN" above]

Context: the private playtest release is OUT (selected testers,
2026-07-09); the game "looks fantastic and works more or less
end-to-end" — the remaining MC1 work is quirk-hunting from feedback.
The next structural conversation: how to manage THREE GAMES in one
engine, with swappable feature families — "a plugin system where
many of these aspects can be swapped out as a whole and you get a
different game."

The three candidates (player's framing):
- **MC1** — almost complete.
- **HIDDEN WORLDS** — "almost precisely MC1, emphasis on almost":
  minor spell deviations, possibly new textures; rabbit-hole depth
  unknown. Retail ships it as its OWN BINARY, multiplexed at startup
  (the launcher chooses MC1 or HW) — so retail itself treats it as a
  sibling build, not a data pack.
- **MC2** — changed almost EVERYTHING: every texture, every sprite,
  every AI routine; the structure and meaning are the same but the
  systems are reworked. Flagship example: the SPELL SYSTEM — an
  EXPERIENCE system where each spell has 3 LEVELS and levels itself
  up as you use it; several MC1 spells collapse into one MC2 spell
  ladder (Fireball L1 → Repeat Fireball L2 → FIRESTORM L3 — no
  firestorm exists in MC1). "I could go on."

Design connections to carry into the discussion:
- The [authenticity matrix] already stipulates MULTI-COLUMN options
  (mc1 | mc2 | improved) with MC2 behaviors as faithful alternates
  importable into MC1 contexts — the plugin system is that idea
  promoted from per-option columns to whole FEATURE FAMILIES
  (spell system, AI routine set, sprite/texture set, economy).
- Existing seams that already point this way: unified asset bundles
  (variants, one schema), the enum-not-bool option registry, the
  sim's semantic/presentation split, wizards.json per-game decode,
  per-game level dirs (mc1/mc1hw/mc2) + the `--level game:index`
  addressing, campaign::is_campaign_level (MC1-specific — would be
  per-profile).
- Open design questions for the session: profile = a top-level
  selection that PINS a coherent set of family choices (with the
  matrix allowing deliberate cross-imports)? How do G-class replay
  semantics interact with profiles? Where do HW's deviations live —
  a delta on the MC1 profile (retail's own sibling-binary model
  suggests: same engine, small override set)? What's the FIRST MC2
  gameplay slice (spell-experience system?) and does its port live
  behind the same seams?

## SPELL SELECTOR — the MC2 CTRL pane as a CORE MECHANIC (LANDED
## 2026-07-10, UI + selection round; effects/economy = Phase 4.2)

Player interjection during the level-0 MC2 playthrough: MC2 casting
works cross-column (lmb/rmb + dev spells) but SELECTION had no
surface — and MC2's selector is the better mechanism, so it became a
matrix option rather than an MC2-only screen. Verbatim trace (Opus
agent, full citations): **docs/traces/mc2-spell-selector-ui.md** —
CTRL (0x1D) hold-to-open/release-to-close, 2×13 grid in `spell_t`
identity order, icons `97+spell`, flyout icons `179+3·spell+level`,
THE persistent per-spell level (`array_0x437[spell]`, reused by every
selection route — binding is per-spell L/R, never per-key-per-level),
grey-not-disable gating, and the map screen's true geometry (minimap
0..382, live view 384..640 × 0..400 rendered with the FLIGHT fov —
the non-aspect squeeze is projection reuse, not a stretch blit).

What shipped:
- **`spell_selector` option** (config `enhancements.spell_selector` +
  `--spell-selector`): `auto` (default = each game's faithful
  surface) | `mc1` | `mc2` | `mc1+mc2` (both surfaces at once, MC1
  only). MC2 always coerces to the pane (no 26-spell in-map grid is
  invented); resolution → `config::SelectorSurfaces { map_book,
  ctrl_pane }`.
- **The pane widget** (`ui::SelectorPane`/`selector_quads`): game-
  parametric (MC2 = 2×13/26 spells/3-level flyout with MC2 art; MC1 =
  2×12/24 in DISPLAY_ORDER, book art, no flyout). Hold CTRL → pointer
  hijack (grab off), hover flyout, click-drag-release commits level +
  hand binding, SHIFT+click fast-binds, hovered box shows the live
  shot meter. Works over flight AND the map screen (same overlay, as
  retail).
- **Pre-composited pane tiles** (player direction, same session):
  every grid/flyout state bakes at atlas-build time through the REAL
  blend LUT — opaque = raw copy, unaffordable ghost = the
  DrawTransparentBitmap blend, unowned relief = the colourize-0xA6
  row — so the draw path is one textured quad per box and the
  treatment choice lives in the composite, not the renderer.
- **MC2 UI sprites baked** (BAKE_EPOCH 4): HSPR{D,N,C}0-0 per
  environment (night-fog shares night), same TAB/DAT signed-RLE as
  MC1 (`bitmap_pos_struct2_t`); MC2's blend LUT confirmed at the same
  TABLES +0x4000..+0x14000 slice (GameUI.cpp:525/1105). 262 entries,
  every trace-cited index verified in the bake (89/90/91 boxes 48×36,
  edge 8×72 = exactly two rows, 13·48+2·8 = 640 native ✓).
- **Map screen topology** (`mgc_render::MapScreenLayout`): `Mc1Book`
  (unchanged) vs `Mc2Split` — minimap 0..382 × 400, live view
  384..640 × 0..400 with the flight-aspect projection (authentic
  horizontal squeeze), bottom 80px black (the pane's zone). Driven by
  `map_book` absence, so MC1 + `spell_selector=mc2` gets it too;
  screenshots follow the game's faithful topology.
- **Per-spell selected level** (`App::spell_levels[26]`, app-side
  this round) + the **cross-column cast bridge**
  (`ui::MC2_CAST_BRIDGE`): MC2 pane spell → MC1 stand-in
  manifestation (fireball→Fireball … summon_army→Undead Army; both
  MC2 quakes share MC1 Earthquake; 7 spells have no analogue —
  selectable, noted to console, equip deferred).

DEFERRED to Phase 4.2 (the MC2 spell column): the level actually
changing the cast (`subspell[level]` tuples + per-level mana costs —
data-driven `SPELLS_BEGIN_BUFFER_str`, NOT source literals), the XP
unlock ladder (`SpellLevels_0x41D`; until then all 3 tiers
selectable as a stand-in), the flyout XP bar, SHIFT+LMB/RMB spell
cycling, per-spell mana pools/`SpellEnabled` possession (today =
bridged MC1 ownership), sim-side persistence of `array_0x437`, the
top-HUD active-spell tile with the Roman-numeral level
(DrawSpellIcon_2E260 — the HUD parity track), and cave_in's
cave-level gate. OPEN from the trace: MC2 has NO numeric spell
hotbar (cmd 43 = name slots) — our digit quick-keys stay an MC1
book enhancement only.

BANKED for the spell-level round (player, 2026-07-10): the flyout
FOCUS MODEL. The hover flyout must not close when the pointer
travels from the grid box up INTO the flyout — the path inevitably
crosses other grid boxes (or leaves the box's rect), so once a
flyout is open, focus TRANSFERS to it: it stays anchored to its
spell until the pointer moves completely outside the combined
bounds — and retail keeps the menu open even when the pointer
leaves the selection pane entirely (exact rules TBD; the player
will provide a fuller description). Our current click-drag commit
masks this (the drag pins the anchor slot), but hover-driven level
inspection/selection needs the real focus model. Likely trace
anchors: `spellOnCursor_50` persistence + `SelectSpellCategory_6D420`
bounds vs the `byte_0x457` sub-state (the submenu-open state ALREADY
routes input to `SelectSpell_6D4F0` only — the original's own focus
transfer for the drag case, PI:806-929).

Player playtest follow-ups (2026-07-10, same session): (a) icons
misaligned → FIXED same day (retail draws the grid icon at the BOX
ORIGIN, EF:22543 — centring was ours; tiles rebaked at origin);
(b) undiscovered spells showed the grey relief → retail draws EMPTY
boxes unless the learn flags (0x3E9/0x403) are set, which we don't
model — relief commented out, tile still baked as a future opt-in;
(c) G/dev-spells now lights all 26 MC2 boxes (the 7 bridge-less ones
selectable, equip deferred); (d) corner tags corrected to
hovered-only transparent blits (EF:22546-53), right tag at
+boxW−tagLeftW (EF:22452); shot meter to the verbatim (+6,+28)/36px
geometry (EF:22516-29); (e) the MC1-layout HUD's equipped-spell
icon/meter/wash are SUPPRESSED in MC2 (wrong atlas ids) — the real
MC2 top tile (big icon 123+spell + Roman-numeral level + mana pool,
DrawSpellIcon_2E260/UI:341) lands with 4.2 + DrawText; (f) PLAYER
REPORT for the MC2 gameplay track: the possession spell does not
possess — expected, claims run the weak fallback until the MC2
spell column (mc2/mobs.rs:1965 note); bank it in the 4.2 worklist.

## BANKED TRACK: faithful UI/UX (player, 2026-07-07)

The original's in-flight presentation as its own work item, after
the mortality cluster: the HUD's full information set (health/mana
bar positions and visuals, the castle panel sub_22E50 with level
digit 43+lvl / capacity+banked bars / alert flash 55 — data already
live in LoadoutView), the fullscreen map/book layout against
BOOKBKG.DAT, spell-panel placement, and every HUD readout the
original shows. HSPR bake groundwork exists (spellbook/HUD quads,
map icon stamps); this track makes the layout authentic rather than
functional-first.

### Original Phase-5 notes (pre-port; kept for context)

Player-observed divergences from the original (2026-07-04, first
hands-on session with the flyer). Expected — the MVP flight model was
invented, not ported — but these are the things to get right next,
and they set the pattern for every behavior to come: **original
handling is the baseline; modern conveniences are opt-in flags**
(same stance as the renderer's palette-authentic baseline).

The flips now persist in `mgcarpet.json` (working dir or `--config`;
schema in `crates/mgc-app/src/config.rs`, one `enhancements` field per
option, absent = authentic, CLI flags override per run) — the
authenticity matrix's home until a real in-game options screen exists.
First entry: `smooth_shading` (terrain shade interpolated across tile
centers vs the original per-tile snap; T toggles at runtime). The
extended-controls and savepoint options below land there too.

1. **Controls.** The original scheme is mouse for aiming plus
   up/down arrows for accelerate/decelerate and left/right arrows for
   strafe — NOT WASD. Implement that as the default; keep the current
   WASD scheme as an alternate binding behind a config/flag. (Long
   term this is remc2-style configurable bindings; their config.json
   "Classic" vs "Modern" keyboard profiles are the reference.)

2. **Altitude model.** The original carpet has NO explicit up/down
   control at all. It terrain-follows: drifts slowly downward toward
   a hover height, and rises swiftly when the ground rises sharply
   under it. Port that behavior (the banked sub_455D0 trace has the
   real response curves) and make it the default. The current
   Space/Shift explicit lift is a genuinely nice addition — keep it,
   but behind an "extended controls" option flag, off by default.
   Getting the drift/rise response right matters more than any
   constant: it's the single most recognizable part of carpet
   handling.
   PLAYER FIELD NOTE (2026-07-06, original MC1PLUS replay): the
   authentic altitude-gain SKILL MOVE — find a tall vertical wall,
   ride the ground-follow up its face, then dash away level and pray
   the altitude holds — is core play and "mind-bogglingly
   frustrating" by design. The banked trace explains it exactly:
   the hard floor (ground+128) carries you up the wall; away from
   the wall you're above the soft ceiling (ground+1024) where climb
   authority INVERTS (pitching up sinks you), level flight roughly
   holds, speed-0 hover bleeds 8/tick. The Phase-5 port must
   reproduce this move verbatim — it's the acceptance test for the
   altitude model — and it is the strongest argument yet banked for
   the extended-lift G-toggle as the sanctioned mercy option.

3. **Walls block traversal absolutely** (player, 2026-07-05, after
   the feature pass landed). The type-8 walls GenerateFeatures raises
   CANNOT be flown over at any altitude — the original hard-blocks
   crossing them, and that rule is load-bearing difficulty design
   (mazes stay mazes; without it several levels become trivial).
   Port the exact blocking rule with the movement code (mechanism to
   confirm from remc1's player movement — candidates: the angle 0x80
   protection bit vs the wall height itself). Related, MUST NOT be
   designed away: walls are indestructible directly, but can be
   breached INDIRECTLY — building a castle close enough repeatedly
   can tear a traversable hole (a known player strategy for escaping
   mazes), and some spells can be thrown over walls. Plausible
   mechanism already visible in the ported feature code: the building
   flatten pass writes heights UNCONDITIONALLY over its footprint
   (digs honor the 0x80 protection bit; construction flattening does
   not), so a castle footprint overlapping wall tiles pulls them
   toward the castle base height. The counter-constraint gating the
   exploit: castle placement appears to require a 0x80-free area
   (8x8-tile protection-bit scan, remc1 sub_main.cpp:17825) — port
   placement validation with the same scan so "close enough" stays
   exactly as hard as the original. The extended-controls lift option
   must NOT bypass wall blocking.

3. **Graphics parity judgement still waits on sprites + sky.** MC1
   terrain textures landed (2026-07-04, same day); remaining noise
   sources before fidelity comparisons make sense: billboarded
   entities, sky, water animation, UV rotation variants.

Method note: divergences get logged here as they're noticed in play
against memory/remc2, then knocked out in fidelity passes — playtest
feedback is the test suite for feel.

### Flight-control tiers (design, banked 2026-07-06 from player notes)

The player's verdict on original MC1 handling: it "sucks, really
really hard" compared to the MVP model — but per the matrix it stays
the faithful baseline. The dreaded branching collapses into THREE
ORTHOGONAL registry enums, not one blob:

1. **Bindings** (P-class, input-mapping ahead of the `FlightInput`
   seam): arrows-vs-WASD profiles, Y-axis flip, mouse sensitivity.
   Y-flip is a binding, NOT a flight-model property. Freely
   combinable with any tier below.
2. **Thrust model** (G-class — physics, recorded in replays):
   `mc1 | mc2 | enhanced`.
   - `mc1`: up/down keys are accelerate/decelerate IMPULSES —
     asymmetric accel vs decel, no stop key; precise standstill
     takes multiple back-and-forth quantum-hunting passes
     (authentic frustration; 1994 UX).
   - `mc2`: same impulse scheme + the stop key (backspace, per
     player memory — confirm exact key from remc2 config). Likely
     mc1 + stop + different constants, not a separate model.
   - `enhanced`: the current MVP feel — hold-to-fly, automatic
     deceleration on release. Player: "better by default in every
     way." Principled framing: STRAFE ALREADY WORKS HOLD-TO-MOVE
     IN BOTH ORIGINALS — enhanced generalizes the original's own
     strafe behavior to the forward axis.
   - Port faithful `mc1` FIRST (physics reference, quirk-replay
     substrate, the wall-climb acceptance test), then layer the
     others; enhanced constants get re-tuned as a deviation from
     the ported model, not the other way around.
   - The ACCELERATE SPELLS (types 2/21) sit OUTSIDE the tier: while
     active they replace the thrust model entirely (the original
     writes carpet speed directly; ours does the equivalent
     override), with brake/opposite-thrust input as the cancel in
     every tier (2026-07-06 spell-playtest design resolution — see
     "Spell repertoire"). Any thrust-model port must preserve this
     seam untouched.
3. **Altitude model** (G-class, already banked): terrain-follow
   (sub_455D0 trace) vs extended lift. Explicit hover up/down has
   NO original equivalent (player: its absence in the original is
   "a huge mistake") — it lives here as part of extended lift,
   never bypassing wall blocking. Orthogonal to the thrust tier:
   the wall-climb move must reproduce under faithful altitude
   regardless of thrust setting.

PLAYER DESIGN NOTES (2026-07-07, session-opening brief for the
Phase-5 port — ground truth, folds into the tiers above):

- **The faithful model is RATE-BASED, deliberately physical**: the
  original simulates flying a physical carpet, "usually to a clumsy
  effect". Mouse offset works like an AIRPLANE STICK — the tilt sets
  the RATE of turn, not the target heading; moving the mouse left
  starts a turn that continues until you equalize back right. Same
  family as accel/decel: an input SETS a state that persists until
  the opposite input unsets it. "Incredibly painful" and overdue for
  unfaithful replacements soon after the faithful port — but the
  faithful port comes first and stays the baseline. (Matches the
  decompile: screen-center offset → low-pass → yaw += s>>3.)
- **THE CARPET IS ALWAYS LEVEL — thrust is a flat horizontal plane
  regardless of aim pitch** (the cardinal correction to the MVP
  placeholder, which thrusts along the 3D view ray). Aiming up/down
  exists for SHOOTING, not movement: in the original, looking
  straight down costs you NO horizontal mobility. This is
  load-bearing combat design — most mobs must be aimed at
  up/down while you dodge in the ground plane; a view-ray thrust
  model bleeds dodge ability exactly when you need it. The faithful
  move = pure yaw-plane polar step at full speed + a separate small
  vertical term (climb scaled by the soft-ceiling authority, dives
  raw). KEEP this rule in every tier, including enhanced.
- **Vertical motion in the faithful model is passive**: the carpet
  floats UP along physical objects (terrain, penetrable walls — the
  ground-follow floor) and settles DOWN by itself; there is no
  fly-up control at all.
- **Extended lift (float up/down keys) is clearly unfaithful** —
  config/CLI flag, G-class, off by default. NEW CONSTRAINT: cap
  explicit float-up at the LEVEL'S HIGHEST TERRAIN TILE (queried at
  load or live) so it can never become a god's-eye map view. Never
  bypasses wall blocking.
- **MC2-tier facts to fold in when that tier lands**: the normalize
  key (backspace, per player memory — confirm from remc2 config)
  zeroes ALL control state — velocity AND heading/turn rate. Also:
  entering/exiting the fullscreen map fixes your ORIENTATION (roll/
  pitch anchor) in MC1 (and likely MC2) but NOT the velocity —
  which is otherwise hard to stop. Find the reset site in the
  decompile; it doubles as documentation of what "orientation
  state" exists to reset.
- **TORSO-AIM (the player's own enhanced-tier design, banked for
  after the faithful port)**: decouple aim from carpet heading
  within a cone — like a person standing on a carpet whose feet are
  planted but whose torso turns freely a limited amount. Mouse aims
  the torso (free aim, absolute, painless); the CARPET gradually
  re-points toward the torso's direction with its own turn rate. A
  hard 180 is deliberately multiple swipes + waiting for the carpet
  to come around underneath you — keeps the original's "the carpet
  is a physical vehicle" spirit while eliminating the stick pain.
  Eventual presentation hook: draw the carpet's front edge at the
  bottom of the screen visibly re-aligning under the aim. This is
  the intended eventual DEFAULT enhanced aiming model; the current
  MVP mouse-look stays as the plain alternate.

## Audio — SOUND + MUSIC (LANDED 2026-07-06; EAR TEST PASSED
## 2026-07-09: "as good as it can be without unfaithful changes")

The step-aside-to-sound session. Import + runtime + sim wiring in one
pass; everything below is in-repo and green (imports byte-probed,
mixer behavior unit-tested, workspace tests pass).

FORMATS DECODED (probed byte-exact against retail; parsing in
mgc-import):
- MC1 `SNDS<bank>-<q>.DAT/.TAB`: whole-file RNC; TAB = 32-byte records
  `{name[18], u32 off @+0x12, u32 len @+0x1A, u16 90 @+0x1E}`, record
  0 = pseudo-header (size field = alloc hint, NOT the DAT length); DAT
  = raw unsigned 8-bit mono PCM. `q` = free-RAM quality tier (remc1
  :51973); per-sample sizes are exactly 2.00x between tiers → rate
  halvings of one master; we bake `-1` = 22050 Hz only. Bank = the
  per-level/screen sound set (level command case 4/36 → sub_5D070);
  bank 0 = the 47-sound gameplay bank whose TAB INDEX = THE ENGINE
  SOUND ID (flush loop 0..47, sub_55100) — the name table confirmed
  every previously traced id (9=FIREBAL1, 15=FIREBAL2, 19=SPEEDUP,
  22=PORTUSE, 23=LITNING, 25=HEAL, 29=CANTUSE, 40=POSSHOT6, 42=RUBBER)
  and named the ambient set: 1=WAVES2, 2=WHB (wind), 5=FIRE,
  31=MARKET. Banks 1-13 = aux sets (intro VOCs, win/fail, menu
  right/wrong).
- MC1 `MUSIC<bank>-<d>.DAT/.TAB`: same TAB; payload = HMP songs
  ("HMIMIDIP", old variant: track count @0x30, BPM=tick-rate @0x38
  (120), 12-byte track chunks @0x308 `{index, len, channel}`, delta =
  LE 7-bit VLQ TERMINATED by the high bit, no running status,
  note-off = vel 0). `d` = DRIVER arrangement (remc1 :54030: 0xA002
  AdLib loads INST/DRUM.BNK → `-0`; `-1`/`-2` other cards). Bank 0 =
  cgame1-3 + csetup, bank 1 = cintro4-6. Song selection = the level
  command stream (case 12/44 play once, 25/57 play looped; the
  level's song id sits in the level struct +576) — UNTRACED source
  data, so runtime song choice is INTERIM (level index % 3).
- `INST.BNK`/`DRUM.BNK`: classic Ad Lib banks ("ADLIB-"), GM-ordered
  (entry 0 = piano1; DRUM entry N = percussion patch for MIDI note N
  — 35 bdc1, 38 snare, 42/46 hats). Field order cross-checked against
  libADLMIDI gen_adldata load_bnk.h.
- MC2 `SOUND/SOUND.DAT`: u32 @EOF-4 → directory `{i16 counts[6]}` +
  per-bank 96-byte records of six 16-byte tier slots `{index_off,
  data_off, index_size, data_size}` (-1 = absent; retail GOG ships
  only the 8-bit tiers, best = 822 = 8-bit 22050). Tier index = the
  SAME 32-byte record table as MC1's TAB (pseudo-entry 0 included);
  samples are full RIFF WAVs (stripped at bake). Ids match MC1's low
  range exactly (OCEAN=1, FIREBAL1=9, MARKET=31 — remc2
  WavIndexes.h), 10 banks.
- MC2 redbook: 27 audio tracks (~19 min, avg ~45 s — sparse ambient
  cues; track 28 = a 6 s sting) inside game.gog per the game.ins cue;
  MC1's image is a cooked ISO, NO redbook — so OPL-vs-redbook was
  never a choice: MC1 faithful music = OPL3 render, MC2 faithful =
  the redbook rip (its XMI/AIL arrangement = future faithful-
  alternate for no-CD parity; MC2 also runs a peace/danger music
  crossfade — remc2 GAME_music_war — untraced, future).

IMPORT (all pure Rust, no FFI anywhere): per-GAME audio bundles
`baked/assets/{mc1,mc2}-audio/` (audio is game-scoped, not
graphics-variant-scoped — bank digits are level selectors, not
tileset pairs). `sounds.bin` = deduped raw pcm8 blob + `sounds.json`
(engine-id-keyed banks); `music/*.flac` + `music.json`. MC1 music
rendered at import: mgc-import::hmp (parser) + ::adlib (BNK parse +
OPL sequencer on the nuked-opl3 crate; 18 2-op channels, oldest-
steal, drums = DRUM.BNK patch at note pitch) → 44100 mono →
flacenc (pure-Rust FLAC). PLAYER-VALIDATED same session (cgame1.wav
probe: "completely fine and expected"; peak-touching = chip-accurate
saturation, 0.0005% of samples, not normalization). INTERIM in the
renderer, accepted at the 2026-07-09 ear test (any change here would
be unfaithful): velocity + CC7 IGNORED (retail songs carry
CC7=0 and vels 1..9 on busy channels in EVERY arrangement — only
renders sensibly if the era driver used raw patch levels), pitch-bend
range assumed ±2 semitones. Redbook ripped losslessly → stereo FLAC
(track boundaries verified by activity profile). Dev aid:
`cargo run -p mgc-import --release --example music_probe`.

RUNTIME (crate mgc-audio; matrix seam = output backend is dumb,
MIXING POLICY is swappable): cpal output stream (pure Rust; silent
stub when no device — headless safe) rendering 32 channels + music;
FLAC decode via claxon (whole-track, tens of ms). FaithfulMixer =
the ported original (unit-tested):
- Request phase sub_55370 (:64444): one pending slot per sound id
  per tick, loudest-wins within 8 (sub_55870 `new-old >= -8`);
  linear falloff over range 12288 units ahead shrinking to 9216
  BEHIND the listener (range = 12288·(1024-off/2)/1024), cull past
  12288² dist², drop below vol 512; PAN only beyond 320 units
  (closer = center); torus-wrapped i16 deltas, engine 2048-space
  yaw. Per-id policy table: mode 1 restart {3,9,15,16,18-28,30,40,
  43-45}, mode 3 don't-interrupt {7,8,10-13,32-39,41},
  player-gated {4,14,29 / 17}, ambient loops {1,2,5,31}, default
  DROP (RUBBER 42 authentically unreachable through this path!).
- Flush sub_55100: 32 driver channels, free-channel alloc, NO
  stealing (full = drop, like sub_48570).
- Ambient loops sub_520F0/52120/52400 + fade pumps sub_51FC0/522E0:
  waves XOR wind by water-under-carpet (:55254-65), fire/market by
  proximity, targets 70/70/120/85 (<<8), fade +2048/-2048 per tick,
  cut below 4096. INTERIM: proximity = direct 8-tile scan over live
  fires (c10 m0/m6) and houses (m45); the original refreshes
  per-player countdown fields from the emitters' own handlers —
  exact hysteresis owed with that trace.
The enhanced distance-weighted emitter mixer (authenticity matrix)
lands later as a sibling policy over the same Cmd stream; the
options enum joins `enhancements` then.

SIM EMISSION (mgc_sim::features::SoundEvent {id, pos, tag=slot,
player}; Gen::snd/snd_player; World::take_audio drains + computes
the ambient inputs): wired at the traced sub_55370 call sites —
attack thunks m0/m3→8 (:22182/:22406), m1→7 (:22294), m2→13
(:22358), m5→32 (:22975), m8→38 (:23555), m11 dart→9 (:24700) +
kraken m6 growl 37 (:23240)/buffet 42 (:23223), dragon m16 roar 39
(:26186, +63 % 2·v_26 throttle), settler house-build gong 10
(:24983), bolt first-tick LCG 33+(rand&3) (:63795 — the draw was
already ported, now feeds the id), standing-fire crackle 3 (:28118),
explosion 30, mana-ball claim 4 (player-gated by the mixer),
rebound deflect 28 (:62880); player-side: per-spell launch sounds
(fireball/rapid 9, meteor/volcano 15, accelerate 19, teleport 22,
lightning 23, heal 25, possess 40 — untraced spells stay silent) and
cast-blocked buzz 29 on the mana gates. Player hit 17/death 16 wait
on mortality (grace discards the inbox before the sound in the
original too). NOT YET WIRED (sites known, next audio pass): m4/m10
dart sound (roadmap's old "sound 195" was garble — no 55370 call
found in that thunk region; verify), m12 castle-idle 21 (:24335),
m11 eruption cadence 11 (:24663)/state entry 11, splash 27 (:28292),
mystery 10 (:28328), win/fail jingles, portal make/gone 20/21,
switch 41, smartbomb warn/blast 43/44.

APP: mgc-app opens the device (skipped for --screenshot/--map),
loads the game's audio bundle, drains World::take_audio each sim
tick into the mixer (per-tick flush = authentic fade timing), F1/F2
= the original's sound/music toggles (remc1 :20086/:20100), config
`audio` section {sound, music, sfx_volume, music_volume} (delete
mgcarpet.json.defaults to regenerate with it). Music: MC1 level →
cgame{1+idx%3} looped, MC2 → track-{02+idx%27} (both INTERIM until
the song-command/redbook-selection traces).

EAR-TEST CHECKLIST for the next playtest (the player is the oracle):
sample pitch (22050 assumption), music tempo (120 tick/s) + timbre
(velocity/CC7-ignored assumption), pan left/right polarity on a
strafe pass, ambient waves↔wind switchover + fire/market radius
feel, fireball launch/explosion timing against DOSBox memory.

FIRST AUDIO PLAYTEST (player, 2026-07-06 — "as far as the
implementation goes, it's successful"):
- LOOP-SEAM CRACKS (wind worst, market double-crack, water blip):
  ROOT-CAUSED + FIXED same day — every MC1 bank entry carries a
  16-BYTE TAIL PAD the driver never plays (sub_48570/sub_52120 pass
  `size - 16`; wind's pad = garbage noise, others = 0x00 runs =
  full-negative PCM pops). Importer now trims it for SAMPLE banks
  (NOT music: CGAME2's last HMP track owns part of those bytes —
  per-song slack varies). Rebaked; player re-check owed.
- PLAYER HAS BEEN PLAYING THE ORIGINAL UNDER GENERAL MIDI, not
  AdLib — their ear reference = the MUSIC*-2 GM arrangement on a
  wavetable bank, NOT our OPL3 render of MUSIC*-0. Banked option:
  render the -2 arrangement through a GM soundfont at import as the
  matrix's music alternates (music_source: adlib | general-midi;
  which one is "faithful" is genuinely ambiguous — both are original
  arrangements; DEFAULT stays adlib for now).
- POLYPHONY STACKING (round 2 refined: NOT clipping — a hollow
  comb-filter echo from many copies of the SAME sample at small
  offsets, e.g. meteor's explosion swarm): the mixer has NO hidden
  throttle — word_12CD28 (set to 2 after every flush play) is
  WRITE-ONLY in the whole decompile, vestigial; the original stacks
  exactly like our port (per-id slot = max 1 new start/tick,
  accumulating across ticks to the 32-channel cap). So the echo is
  (a) mostly a SYMPTOM of our meteor emitting more explosion
  entities over more ticks than retail — the already-banked m17
  blast-ring trace (square→circle) should fix count and sound
  together; recheck audio after that spell-fidelity pass; and
  (b) residually authentic-ish sample stacking. Enhanced-mixer
  backlog (P-class): same-id retrigger MERGE (restart-or-boost
  instead of layering) + soft limiter.
- AMBIENT MIXING (player directive): current same-time mixing of
  the ambient loops = their suggested enhancement, KEEP; extend the
  fire/market audible radius further AND put the whole
  distance-mixed-ambience behavior behind a non-faithful flag
  (faithful = the original's tile/proximity switching only). I.e.
  the enhanced ambient mixer graduates from backlog to requested.
- SFX correctness: "some are off, tied to the spells' own effect
  deviations" — player compiling a proper list; fix with the spell
  fidelity items, not as audio bugs.

DANGER MUSIC DECODED + LANDED (2026-07-06, from the player's
observation that the original runs ambient/danger mixes per track and
our static picks sometimes played "the danger track"): the danger mix
is NOT a separate song — every MC1 in-game song keeps its combat
layers on MIDI CHANNELS 3/4/5, initialized at CC7 0 (the "CC7=0 on
busy channels" mystery from the first render session, now solved),
and the engine fades them with runtime CC7 ramps: sub_20BD0 sends
`Bn 07 <level>` to 0xB3/0xB4/0xB5, step 2 over 0..126, callback rate
0x3C/s fading IN (~1.05 s) and 0x14/s fading OUT (~3.15 s);
sub_20D00 = the mode switch. TRIGGER: the wizard's v_46 countdown —
armed to 100 (~3.3 s) by every processed hit (sub_46540's
damage/grip/steal blocks → sub_46520 :55637) AND by any projectile
ACQUIRING the human as its homing target (:64013/:64095 — being shot
at counts before being hit); the player tick decrements it and picks
the mode on v_46 > 0 (:55282-92). PORT: the bake renders each
layered song twice — ambient mix (ch3/4/5 pinned silent) +
sample-aligned danger stem (ch3/4/5 solo at the 126 ceiling) — into
`file` + `danger_file` (auto-detected: notes on a danger channel
whose first CC7 is 0; cgame1-3 get stems, csetup/cintro correctly
don't); CC7 is now honored as channel volume generally (applied at
note-on; the old ignore-CC7 call was wrong for exactly these
channels); the output mixes the stem as a position-locked overlay
whose gain runs the original's ramp at sim-tick granularity; sim
arms `player_danger` from the player mail inbox + aim-assist
acquisition and reports it in AudioFrame. Side effect: ambient base
mixes got much leaner (cgame2 FLAC halved) — the danger layers were
the density the player was hearing. INTERIM: live CC7 ramps within a
song are not re-applied to already-sounding notes (retail songs only
set CC7 up front); the invincible dev player arms danger from
discarded mail so the mode is audible in playtests; bonus correction
from this trace — the every-64-tick wing-flap roll plays sound 46
(FLUTTER), not 3.

### General MIDI music arrangement (LANDED 2026-07-11, user request)

MC1's CD carries all three per-sound-card arrangements of every song:
`MUSIC<bank>-<d>` with d = 0 AdLib FM, 1 Roland MT-32, 2 General MIDI
(remc1 :54029-30: 0xA001 GENERAL → digit 2, 0xA004 ROLAND → 1, 0xA002
ADLIB → 0; same HMP container, per-driver patches/mix). The bake now
also renders the `-2` GM arrangement through **fluidsynth + a GM
soundfont** (discovery: `MGC_FLUIDSYNTH`/`MGC_SOUNDFONT` env overrides,
then PATH + distro soundfont paths; `mgc_import::fluid`), via a new
HMP→SMF type-0 encoder (`mgc_import::smf` — HMP ticks/second ⇒
division=tick_rate + 1s/quarter tempo, MixSpec channel pins carried
over). Same ambient/danger-stem contract as FM: the GM arrangement
keeps its danger layers on ch3/4/5 at CC7 0 (probed), so
`has_danger_layer` transfers unchanged; float-WAV capture (FluidR3
peaks 2.3× FS at unity gain) with ONE normalization factor per song
over the ambient+stem SUM (the runtime overlay cannot clip). New
bundle members `music/*-gm[-danger].flac` (44100 STEREO — pan is real
in the GM data) + `gm_file`/`gm_danger_file` in music.json; BAKE_EPOCH
6→7; FORMAT.md updated. Runtime: `audio.arrangement` config
(`auto`|`fm`|`gm`, authenticity-matrix multi-column — all three
arrangements shipped retail; auto = GM when baked, FM fallback),
`Audio::set_prefer_gm`. Hosts without fluidsynth bake FM-only bundles
and play exactly as before (Windows release = FM until a bundled
renderer is decided). ~~MC2 unaffected (redbook IS its retail music;
its AIL XMI fallback stays a future faithful-alternate).~~ [CORRECTED
2026-07-12 by docs/traces/mc2-music-law.md: MC2's in-game music is the
XMI (MUSIC.DAT, by map type); the redbook tracks are the per-level
objective VOICEOVER, segment-indexed — see the 4.3b/4.4 session record
for the agreed speech-snippet bake shape.] PLAYER-CERTIFIED 2026-07-11
("tested everything and it's working great").

## Spell repertoire (track started 2026-07-06; CORE LANDED same day)

### BLUE spell jars — disassembly-VERIFIED + LANDED (2026-07-11)

Player report: late-campaign levels (past ~25) re-deliver masked-away
spells as BLUE jars that cast with no castle/mana requirement —
castle-less survival in maze levels. Traced (background agent, full
citations in docs/traces/mc1-blue-jars.md) and landed in mgc-sim:
- Data: the class-12 THING's `data_12` ∈ {0,1,2} = red, {3,4,5} =
  blue (−3 recovers the same sub-state); blue sets `+18 byte[2]|=4`,
  sprite/type 280 (red 77) and — the semantics — the manifestation's
  requirement `+132 = 0` (:44043-54, :64845). Our THING post-init
  already decoded this ("village-owned variant" guess now corrected:
  it's the UNRESTRICTED jar, `world::BLUE_SPELL`).
- KEY CORRECTION to our port: the bind gate (:26924) and the
  cast-unavailable gate (:27860-64) read the OWNED MANIFESTATION's
  `+132`, not the spell table — `+132` is ONE threshold encoding both
  the castle-presence/level requirement and the min-mana bar, so blue
  = zeroed = both gates open. Port: `spell_castle_req(id)` (owned +
  BLUE_SPELL → 0, else table) now feeds `spell_gate` + `loadout`
  bindable.
- Persistence: the death jar-scatter banks blue per spell (`var_916`,
  :55531-35) and the respawn re-grant restores marker + sprite + zero
  req (:54908-12) → `Player::death_owned_blue`, hashed only when
  armed (the mc2_apocalypse pattern — pre-blue goldens hold).
- Also settled: model65 = spell id via `off_987DE[+65]` dispatch
  (:64884/:48853) — the old try_pickup TODO is closed.
- Test: `blue_jar_unrestricts_its_spell_and_survives_death`.
- PLAYER-CERTIFIED 2026-07-11, exercised on level 039 with the
  expose-jar-spells debug markers. Player context banked: 039 is one
  of the levels EXCLUDED from the retail campaign because it is
  broken — THING slot space for one, and generally wonky/unfinished.
- OPEN (flagged UNCERTAIN by the trace): no path found that seeds
  blue at level load without a physical blue THING jar; if a playtest
  shows a castle-less spell with no blue jar on the level, revisit.
  Cross-LEVEL blue carry (var_916 in the campaign snapshot) is also
  untraced — bank with the campaign-carry track below.
- Debug options landed alongside (2026-07-11, player-certified):
  `enhancements.expose_jar_spells` (jar spell-icon markers, map +
  main view; MC1 class-12 jars incl. blue/scattered + MC2 class-15
  tokens) and `enhancements.grace_meter` (the unfaithful spawn-grace
  strip, now DEFAULT OFF — faithful shows nothing for grace). Both
  have CLI overrides.

### Campaign spell progression — disassembly-VERIFIED (2026-07-07)

Traced to settle whether levels can be playtested in isolation.
Answer: NO — the human's per-level spellbook = (that level's
availability mask) ∩ (spells collected in prior levels). The system
is active from level 1, NOT a mid-game switch. Sources: level-init
loop sub_main.cpp:49141-49256, spellbook rebuild sub_45C10_45F50
:55304-20, new-game reset :60700.

Three pieces of state:
- `var_676[24]` — LIVE spellbook on the wizard entity, indexed by
  spell type. Rebuilt each wizard init: memset 0 then repopulate from
  the working list (:55310-18).
- `var_14958_1635_532[24]` — the WORKING spell list for the current
  level (entity ids of held spell objects). Wiped to -1 at level
  start (:49193); filled by grant logic + jar pickups (:48871).
- `var_15318_1995_892[24]` — PERSISTENT collected-spell flags (24
  bytes, one per spell). The campaign memory.

Carry-over: at each level load the prior player record is snapshotted
into `str_11274` (:49145) and the 24 collected flags are copied out
of it into the fresh record (:49148). New game zeroes them (:60700).
So collected spells PERSIST — but the game does NOT auto-grant all
previous spells. Miss a jar → flag never set → never receive it.

Per-spell grant decision at every level start (:49218-49241),
switched by `var_u8_13332_9`:
- HUMAN branch (else, :49226): grant spell v14 iff
  `str_230867_37072[level].var_230983[v14] == 1` (per-level
  availability mask) AND `var_15318_1995_892[v14]` (collected flag)
  is set (:49229 + :49233). The intersection is what STRIPS spells at
  the start of restrictive levels.
- AI-WIZARD branch (`var_u8_13332_9==1`, set for every non-human at
  :49153-54 where `var_u16_8` = human index): grant straight from the
  level table (`var_230883 && var_230983`), no collected requirement.
- CHEAT (`var_u8_0 & 0x10`, :49231/:49239): human bypasses the
  collected-flag check → gets every spell the LEVEL table permits.
  This is the game's own "access all spells" (:48904).

Per-level tables `str_230867_37072[level]`: `var_230983` =
availability (==1 for human eligibility), `var_230883` = second grant
mask (the AI/cheat path). NOT yet traced: red-vs-blue jar object types
(blue = castle-level-requirement bypass), where the collected flag
gets SET on red-jar pickup (the class-12 grant only writes the live
list here; the persistent commit is elsewhere — likely level
completion), and save/load of the flags (+15318 copied at :58756/
:58995).

NOTE (manual-confirmed + disassembly): levels 1-25 set every
`var_230983` entry to 1, so the availability mask is a no-op there —
you have a spell iff you COLLECTED it. So even the all-spells cheat
over-grants for those levels; faithful state needs the collected set.

PLAYTEST IMPLICATION: three paths. (1) Faithful campaign mode —
thread the 24 collected flags through transitions (str_11274 snapshot
→ copy-in) + bake each level's two 24-byte masks; small, correct
long-term. (2) Blanket all-spells toggle — reuses the game's
`var_u8_0 & 0x10` cheat; simplest, but over-grants on levels 1-25 (all
masks are 1 there, so it hands you spells the campaign hadn't
delivered). (3) THEORETICAL-MAX MASK (player idea 2026-07-07, best for
faithful playtest) — statically infer, from the campaign level
sequence, the set of spells a diligent player COULD legitimately hold
entering level N, and seed the collected flags with it.

LANDED 2026-07-07 (path 3, `plausible_spellbook`): mgc-app
`campaign.rs` builds the cumulative jar-union from the sibling
`level-NNN.mgcl` files (`jar_spells_in` = class-12 Entity placements,
model = spell id; `plausible_spellbook` unions levels `0..N` excluding
N and blacklisted/lost levels), granted into the world at level start
via `World::grant_spells` (reuses the normal grant/auto-equip path).
Toggle: `enhancements.plausible_spellbook` + `--plausible-spellbook`/
`--no-` (mirrors `invincible`); G-class, TEST-ONLY, logs the exact
levels scanned + spell names (no silent claim). VALIDATED on real
baked mc1: index 000 grants {Fireball, Possess, Castle} (the "first
three"), the union grows ~1-few/level, and reaches all 24 with Global
Death's first jar at index 024 — i.e. LEVEL 25 (1-indexed), the last
level before index 025 begins stealing spells via the availability
mask. INDEXING (settled with player): "levels 25 and before" = indices
0..=024; the mask regime starts at index 025. Post-025 levels carry
huge jar sets (the availability-mask regime, all masks 1 ≤ idx 024).
PLAYER-CONFIRMED live: launching the instrument on index 024 grants 23
of 24 — everything except Global Death, whose only jar is 024's own,
which you collect DURING that level (the `0..N` exclusive-of-N rule
working end-to-end). Blacklist WIRED: `MC1_BLACKLIST =
{8, 17, 28, 33, 39}` (the code-confirmed campaign skip table, remc1
sub_34070; see "MC1 CAMPAIGN SKIP TABLE" below); lost levels 50+
excluded by the `< 50` gate. So the union scans exactly the 45 played
campaign levels before the target — no over-grant from skipped worlds.

MASK FIX (player-caught 2026-07-09, LANDED same day): the instrument
granted the raw jar union and SKIPPED the availability-mask
intersection — the retail grant is availability(:49229) AND
collected(:49233), so from index 025 on the union must be filtered by
the target level's mask exactly like a real campaign arrival (the
player's framing: past 25 the collected set is simply "all 24", and
the game plays on via SELECTIVE masking for challenge). The mask =
the level tail's human slot-0 `allowed_spells` (var_230983), already
baked in wizards.json. `campaign::apply_level_mask` now intersects
(stripped spells logged — "level mask strips: …" — never silent);
maskless packages (MC2/old bakes) no-op. Verified on real bakes:
idx 030 = 17/24 granted (strips Quake/Crater/Lightning/Castle/
Undead/Wall of Fire/Rapid Fireball), idx 035 = the brutal trio
{Fireball, Possess, Create Castle}, ≤024 unchanged (all-1 masks).
Unit test pins the intersection + the no-op path.

PREGRANT HACK REMOVED (player-caught on level 032, 2026-07-09):
`grant_starting_spells` hardcoded Fireball+Possess into every fresh
World — a pre-jar-era playability hack that leaked past both masks
(the 032 report: "fireball, mana(=possess), beyond sight" =
hack {0,3} + plausible∩allowed {5}). The faithful fresh-world book
is EMPTY: the retail human grant is availability ∩ collected, and
idx 000's start row is empty too — retail level 1 begins spell-less
and the first three spells are its JARS, collected in play (bare
level-000 launch is now the authentic first-level experience). Data
survey (human slot 0): idx 000 start={} allowed=all; 001 {0,3,16};
010 {12 ids}; 025 start=all; 030/035 start==allowed EXACTLY; 032
start={0,3} allowed={5}. In-level jars authentically bypass the
availability mask (032's meteor-alcove jar granted in play — the
mask gates level-START grants only).

RETAIL-CONSISTENT (player correction, same day): retail 032 does
start with Beyond Sight — the initial "no spells" report was the
spell being useless on that level and easy to forget. That is
exactly plausible∩allowed = {5}: the mask-fixed instrument
reproduces the 032 arrival loadout verbatim. (032 cannot
discriminate the start-row hypothesis below — start∩allowed = ∅
there, both models agree.)

BANKED RETAIL CHECK — the START-ROW HYPOTHESIS (player, same day):
maybe slot-0 `starting_spells` (var_230883) means "granted even if
not collected" — a pregrant floor. The decompile's human branch
never reads it (:49226-33), but start==allowed on 030/035 fits the
floor reading (post-mask levels guaranteeing a loadout), and the
two models agree for completionist runs (collected∩allowed ==
allowed) — they diverge ONLY when jars were missed. Retail test:
fresh game + level-skip cheat to idx 030 with zero collected — a
17-spell book confirms the floor, an empty book confirms the
decompile. Either way 032 grants at most {5}: its start {0,3} is
DISJOINT from allowed {5} — start spells that can never arrive, a
FIFTH authoring defect in 032's tail (after the stall, the alcove,
the repeating obelisk, the empty dis 19).

STATUS: this is the SETTLED INTERIM for individual-level playtesting,
standing in until full campaign mode (path 1) lands a stateful store
of collected spells threaded across level transitions (str_11274
snapshot → copy-in + the two per-level availability masks). Until
then, `plausible_spellbook` is the faithful way to enter any non-wizard
level with the spellbook a diligent player could legitimately hold —
deadline-safe (every spell collectable in a played level before idx
025, verified) so it never grants an impossible spell. Corpse spells
(the other legit source, from rival wizard levels) remain the one
un-modeled input; add when the enemy-wizard AI lands (same per-level
table via the `var_u8_13332_9` AI branch).

Path (3) is DISASSEMBLY-CONFIRMED sound: jars are STATIC placement
records `str_1072[]` (NOT conditionally spawned) — level load iterates
them and, for each `data_0==12` (class-12 jar), increments a per-spell
census `var_u8_232611[data_2]` where `data_2` = the jar's spell id
(:43952-55). The granted spell = the entity's `var_u8_29860_65`
(offset +65), the same field that addresses `var_676` (:64794,
:55318) — statically known from the placement. So per level: union
the spell-ids of all class-12 placement records; cumulative union
through level N = the collectable upper bound entering N. Caveats
(honest bounds, don't break it): it's an UPPER bound (a real run may
miss jars — which is what we want for playtest: "everything obtainable
here"); and it OMITS enemy-wizard-corpse spells (the other legit
source) — true max = jar-union ∪ rival loadouts from prior wizard
levels (rival spells come from the same per-level table via the AI
branch, traceable). For early/non-wizard levels jar-union is complete;
add corpse spells after wizard levels. Better than (2) because it
never tests a level with a spell the campaign couldn't have delivered,
so bugs found are real. TEST-ONLY, not campaign state.

LANDED (2026-07-06, pending playtest): the full 24-spell player
repertoire in mgc-sim (`spells.rs` static table + world.rs runtime:
Player state, class-12 manifestation entities in the pool [slot
economy honest], per-hand cast dispatch off the traced sub_46B00
gates, burst counters as refire pacing, mana pool 100k INTERIM w/
carpet-rule regen, per-shot deduction implemented [remc1 ships it
commented out — maintainer mis-fix pattern], jar pickup converts the
jar entity in place + auto-equips LEFT :64855, no duplicate grants,
starting spells INTERIM = Fireball+Possess after dis-0), the HSPR UI
pipeline (mgc-render screen-space quads; mgc-app/ui.rs composites
icons through blend-lut EXACTLY like the original's 2D blit — icons
bake pre-composited on their slot slabs, equip highlights as tile
variants [the luminous fireball/possess ramps only read right over
the slab pixels; slab centers are index-0 holes where the original
shows the book page — DATA/BOOKBKG.DAT 320x200 exists for the later
authentic-layout pass]), book-screen spellbook grid in display order
w/ hover + LMB/RMB equip + cooldown veils + mana bars, in-flight HUD
(two equipped slots + mana bar), quick keys 1-9,0 (our enhancement;
the original's only digit path is a Ctrl+]+digit chord :20340-56),
and the dev_spells toggle. Headless: --screenshot --map-view renders
the book w/ UI; MGC_DUMP_UI_ATLAS=path dumps the composited atlas.
Sim tests cover cast/deduct/gate, jar pickup slot-retention, accel
exclusivity, dev-spells grant.

FIRST SPELL PLAYTEST (player, 2026-07-06, fixes LANDED same day —
93 tests green; edge-triggered casts [autofire = Rapid Fireball
only, Lightning Bolt hold-stream], accel thrust-override, Meteor
m17 blast ring, Earthquake canyon-walker trench, Volcano m18-marker
eruptions, Global Death invisible-prime point-blank pulse; Castle
NOT reproducible as broken — spell_castle.rs proves the build on
clear dry ground; player's casts likely hit protected/water tiles =
the authentic silent skip; TODO app-side "cannot place" cue. Book
click now equips AND closes, per original UX.): visuals have expected deviations; behavior findings (player
ground truth, several beyond any trace we have): (1) hold-autofire
must be EDGE-TRIGGERED per cast — auto-repeat is Rapid Fireball's
whole identity (and Lightning Bolt = hold-stream per manual);
Possess autofiring at 1/tick was wrong; (2) Accelerate must propel
with NO thrust key held (our boost multiplied thrust input = x0
bug); REFINED SEMANTICS (design resolution of the accelerate-spell
vs hold-W-thrust-model conflict): while active the spell REPLACES
the thrust model — propelled at 3x (button held) / 2x (released)
along facing until the ~8s burst drains, thrust input ignored, the
ONE live control = brake/opposite-thrust input which CANCELS the
spell (the manual's down-cursor cancel, generalized per binding
set). Deliberately tier-independent: the original also bypasses
its own control scheme here, so the same override runs unchanged
under the future faithful MC1 thrust model (Phase 5 note updated); (3) Meteor = projectile + MASSIVE-radius explosion on impact;
(4) EARTHQUAKE = an ongoing crater that TRAVELS FORWARD from the
impact point (= the canyon-walker shape, not the expanding m11
bowl); (5) Volcano needs the secondary magma/eruption damage, cone
alone is half the spell; (6) GLOBAL DEATH authentic behavior: NO
visible effect — primes on cast, waits ~2s, then at expiry a single
lightning-blast SOUND + damage in a TINY radius around the carpet
("you have to be straight below a dragon to affect it" — the small
AoE is the balance); our ground explosion was wrong in every
particular; (7) Create Castle did nothing in play (diagnosis
dispatched). Heal/Shield/Rebound/Invisible untestable until the
player is mortal (life/death housekeeping).

SECOND SPELL PLAYTEST (player, 2026-07-06 — round-1 fixes verified):
Accelerate CONFIRMED, edge-triggered casting CONFIRMED, book
click-equip-and-close CONFIRMED, Earthquake/Volcano visuals+models
CONFIRMED. Open fix items banked from this round:
- Meteor blast is SQUARE, should read circular (area size right).
  Suspect our area_write box/Chebyshev metric — DO NOT blindly
  change it, it's shared with the ported mob combat; trace the m17
  ring's damage test first (the tick-growing ring may be naturally
  radial in the original).
- TERRAIN SPELLS ARE KILL ZONES (player ground truth): Earthquake,
  Volcano AND Crater must deal massive damage to anything in the
  effect area while running — "volcano right in front of a wizard
  not suspended high = almost guaranteed kill" (acceptance test).
  This is the features-track "damage broadcasts deliberately
  omitted" debt coming due.
- Global Death effect "shockingly small if any" — barely scratched
  a vulture group at point blank. Prime suspect: the damage write's
  vertical/z window excluding airborne targets near the carpet;
  investigate with the vulture repro.
- Castle: fires once + lockout = matches the original's surface
  behavior; the rest (mana thresholds per castle level, balloons,
  upgrades) DEFERRED to the mana-collection/housekeeping cluster by
  agreement.
Player verdict: "a worthy result for a first attempt." NEXT SESSION
(player directive): STEP ASIDE TO SOUND — music, sound mixing,
spell/combat effects — designed so the spell repertoire needs no
rework later. Head start already banked: the emission handlers'
original SOUND IDS are traced (fireball 9, meteor/volcano 15,
accelerate 19, teleport 22, lightning 23, heal 25, claim-mana 40,
cast-blocked message 29; sub_55370_558A0 call sites) — design =
sim emits (sound_id, position) events at the original call sites,
mixers consume (authentic tile-rule vs enhanced distance-weighted,
per the banked emitter architecture). SOUND.DAT via Moburma's
BullfrogSoundExtractor format, XMI→OGG via libADLMIDI at import.

THIRD SPELL PLAYTEST (player, 2026-07-06 — first 12 spells, post-audio;
fix session same day, this section updated in place as items land):
- Fireball: discharge + explosion CORRECT. Bug: max-range midair
  expiry explodes at GROUND level below the flight point, however
  high the fireball was.
- Possess: effects + sounds correct; claiming inert (mana track, as
  expected). Bug: the auto-aim ACTIVELY steers shots away from free
  mana balls toward creatures — balls are invisible to our
  aim_assist (class-5 + wizard scan only).
- Accelerate fwd/back: CONFIRMED perfect (sound + behavior).
- Create Castle: no sound, no projectile, no visible building — yet
  the market ambient starts and Teleport anchors to the site, so the
  m45 build event lives. DIAGNOSED same session: cast_castle passes
  build-table row 16, which retail data shows is a 1x1 stub; the
  real castle footprints are rows 1-7 (8x8, 21x21 x2, 35x35 x2,
  48x48 x2 — castle levels; the 8x8 protection scan matches row 1).
- Heal: sound correct; healing untestable until health lands.
- Rebound: CONFIRMED (sound + return fire, all applicable
  projectile spells incl. meteor).
- Shield: no effects at all; player recalls no audio in the
  original either — untestable until damage; leave.
- Invisible: CONFIRMED (attackers keep attacking; moving while
  cloaking throws pursuers off the trail).
- Earthquake/Crater: missing DISCHARGE sound (should resemble
  volcano/castle's) and missing EFFECT loop (low-key earth
  rumble/crackle while the deformation runs) — both spells.
- Meteor: discharge sound correct. Bugs: (a) a ground-level
  explosion trail paints the path between caster and impact —
  never seen in the original (suspect shared root with the
  fireball-expiry bug: our fire effects snap z to ground every
  tick, so the authentic in-air decorative trail (10,1) falls to
  the terrain); (b) impact blast reads SQUARE (player screenshot),
  should be round.
- Volcano: discharge sound correct. Bugs: effect sound missing
  (same terrain-change rumble as earthquake/crater); eruptions are
  fireballs in random directions from nowhere — original has a top
  crater area launching BALLISTIC lava bombs (player re-checking
  the original for the exact look); activity must be FINITE, ours
  reads indefinite.
- Lightning Bolt: trajectory + the chaining sound CONFIRMED
  correct. The standard explosion at the bolt end is probably not
  authentic, and damage seemed low — notably LESS than a rapid
  fireball, which reads wrong.
- Lightning Storm: complete gap — our 8-bolt fan showed nothing.
  Authentic: a storm forms around/above the target and projects
  multiple bolts over a period of time (kills most enemies alone);
  vs a building it launches way up high above and bolts fly
  everywhere.
- Undead Army: summons apes + the rock-thrower mob, not
  skeletons; they attack the CASTER (must not) and drop mana on
  death (must not). There's an upper limit on concurrently active
  skeleton groups (value unknown to the player).
- Mana Magnet: inert pending mana collection (expected); missing
  the soft-projectile discharge sound (same as Possess).
- Steal Mana: projectile + trajectory correct; missing the same
  soft-projectile discharge sound as Possess. Effect testable
  later via genie casts (levels ~40+).
- Beyond Sight: no wizards exist to test the effect, but the cast
  lacks its projectile; sound unknown to the player.
- Duel: projectile correct (krakens use the tether effect —
  verified from their side); missing a discharge sound.
- Teleport: PERFECT, exact sound. Related gap: PORTAL use should
  play the same sound (22) and doesn't.
- Wall of Fire: nothing at all in play (our 5-fire line didn't
  even show). Authentic: a fireball-like projectile; at the target
  a unique "napalm from the sky" sprite effect.
- Global Death: priming correctly silent; the expiry blast needs a
  real explosion sound. (Vertical damage-window suspicion already
  banked from round 2.)
- Rapid Fireball: projectiles + cadence CONFIRMED; missing the
  per-shot discharge sound — the original is a distinct fireball
  machine gun, ours only sounds the first shot.

PLAYTEST-3 FIX SESSION (same day, 8 remc1 trace agents + port pass;
112 workspace tests green — playtest verification owed):
- FIRE Z RULE (sub_42000_42340 :52576-601): fires NEVER snap down to
  terrain — above ground they drift by the fixed flicker delta, below
  they clamp UP. Fixes the fireball's ground-level max-range
  explosion (midair expiry explodes at the projectile's live z,
  :62890-932) AND the meteor's ground trail (the (10,1) seeders ride
  the flight altitude — an in-air fiery tail, :63029-38, :28171).
- METEOR BLAST IS ROUND IN DATA: sub_25CE0 (:28671) places fires on
  the SEARCH.DAT ring annuli (160-unit pitch, ±64 jitter, -96 2x2-
  center recenter) — our Chebyshev box placeholder in ring_cells_pub
  was the square; now delegates to the real parsed rings (and the
  same fix un-scattered the u8-wrapped negative deltas). The DAMAGE
  box (192·ring half-extents, sub_120B0 AABB) is authentically
  square and untouched. Extent floor removed (:28696-97).
- TERRAIN KILL ZONES LANDED (the round-2 debt): volcano cone
  2000/tick ch0 while growing (:28327), crater 200 first tick then
  /25 (:28396-400), ridge full f44 per raise (:29163) — owner-immune
  (the caster is safe, bystanders die); the loop-10 RUMBLE at all
  three sites (:28328/:28421/:29164) = the missing earthquake/
  crater/volcano effect sound. Runtime-only (load fixpoint passes
  ctx=None — the original broadcasts into the half-built pool,
  nothing observable).
- EARTHQUAKE = the real (10,15) crevice walker (sub_3ABE0 :46946 +
  sub_25990 :28534): life 128, random start heading, ±45 wander,
  256/step, a 10-tick m11 digger per step (extents+owner copied),
  >8 net water ticks kill it. Replaces the canyon-head APPROX.
- VOLCANO = the real m18 driver (sub_25EC0 :28731): counter machine
  (tick-0 start, 1..126 at p=1/5 skipping every 16th, clean death
  only via an activation at exactly 127 — else the global erupting
  register stays latched, authentically blocking ALL re-arms),
  global one-eruption-at-a-time + (10,19) plume swap, BALLISTIC
  (10,16) lava bombs (sub_3ACC0/sub_25A60: life 100-199, vz 256 up,
  gravity -28 clamp [-384,256], bounce -vz/4, rest → 30-tick 3x
  standing fire, downhill roll ×250/256 [gradient APPROX]), the
  eruption-start blast fireball (pitch -386, life 1 → (10,17)
  field), dormancy re-arm p=1/100 past counter 2500. FINITE, as the
  player demanded.
- POSSESS INVERTED AS TRACED (sub_54520 case 1 :64040-77 +
  sub_11AC0 :17033): the lob targets ONLY mana balls/houses (never
  creatures — the exact opposite of our old aim assist), flight z
  clamps up to terrain (:62975-77), detonation = the (10,12) ch1
  claim flash (sub_3AA10, 512 extents, 8 ticks); balls (:29439-45)
  and built houses (:30801-14, sprite 177 swap) claim from the ch1
  SENDER + chime 4. The -1536 mana-drain amount + claim
  presentation = mana track.
- CREATE CASTLE = the real chain (the m45 house event was the WRONG
  PRIMITIVE — build row 16 is a 1x1 stub): cast (sound 15 :65914) →
  c9 m10 castle ball (sprite 18, ground target 4096 = 16 tiles
  ahead :65894-902) → sub_12F70 scans (launch fail = silent abort;
  landing fail = flip 180 + one step back, build anyway; the 8x8
  window is (tx-8..tx-1) ASYMMETRIC, ported verbatim) → class-3 m2
  castle entity (parity-snapped, sprite 177, state-5 machine) →
  level-up (gong 10 :56474) → m42 CASTLE painter (sub_285C0: 20
  ticks, CUMULATIVE build rows 1..=level — rows 1-8 are the castle
  levels, 8x8 at level 1 — paint every 7th tick, protection stamp)
  → m41 ground leveler (10 ticks toward the clamped-220 perimeter
  mean [APPROX arithmetic mean]). Teleport now anchors to the real
  castle. Deferred: upgrades (m43 token), balloons, mana capacity
  per level (sub_47DD0 list), respawn.
- LIGHTNING STORM = the real chain (:65988 → sub_53DC0 :63628 →
  sub_3B460/sub_26D20 :29279): ONE c9 m12 carrier (sound 9, life
  5, wizard-homing — flies straight for us until rivals exist) →
  the (10,38) STORM CLOUD (state 40, sprite 272): climbs 64/tick to
  ground+1024 (the "launches way up high above a castle" = this
  servo over mound terrain), then 2 bolts/tick for 33 ticks in
  opposite random directions (pitch 56 down, life/3, the spell's
  2000 each, thunder 23/tick) ≈ 66 bolts. Replaces the 8-bolt fan.
- WALL OF FIRE = the real chain (:66110 → c9 m16 [fireball sprite,
  state past remc1's truncated table — sub_53B50-shaped straight
  flight, no +44 copy] → (10,53) NAPALM cloud, state 58 sub_29780
  :31140): 15 waves of standing flames on SEARCH rings 0..1
  (112-unit pitch), wave 0 = a 14-tick ground-fire patch, waves
  1..14 = 1-tick sheets climbing 128/wave — the rising fire
  curtain, 100/tick per flame. The row's 24464 confirmed dead
  weight. Launch sound 9.
- UNDEAD ARMY = the real spawner semantics (sub_26E90 :29353): up
  to 8 class-5 MODEL-9 skeletons (NOT m7 apes — m9 is the
  materialize-riser, sprite 220) on a 512 ring facing radial+180°,
  owner on BOTH +24 and +144 (remc1 writes only +144 — suspected
  slip beside its :29366 pool hardcode; +24 is what the
  do-not-attack-owner gates read, :24242), f140=0 → NO corpse mana
  (:29672 gate), 64-per-owner cap. player_in_aggro_range now honors
  the owner gate. Deferred: the human→skeleton conversion arm, the
  mana-scaled army pool (remc1 hardcodes 10000 anyway).
- LIGHTNING BOLT endpoint verified AUTHENTIC as-shipped: the (10,23)
  flash is sprite 7 (the standard explosion look the player
  doubted), 200x200, ONE ch0 write of the projectile's +44 — player
  bolt = 500 (vs the fireball fire's 400; the beam copies +44 with
  the shielded-quarter exception, :63445-46). Added the missing
  thunder-crack 24 (:28911).
- SOUND MAP CORRECTED/EXTENDED (all trace-cited): earthquake 9 (NOT
  15), crater 15, castle 15, duel 9, steal mana 9 (:65764 — the
  player's possess-soft memory loses to the trace), storm 9, undead
  9, wall of fire 9, mana magnet 40 (:66097); rapid fireball now
  thunks 9 PER SHOT (the machine gun); portal use plays 22 (the
  teleport sound, player-confirmed gap); Global Death expiry plays
  the real explosion 30; Beyond Sight is authentically SILENT and
  projectile-less in remc1 (sub_56730 :65292 = mana gate only — the
  player's "should have a projectile" memory has no substrate in
  this decompile; flagged, not invented).
- New tests: castle chain over baked data (rewritten), volcano
  finite-eruption, possess claim, storm rain, napalm curtain,
  owned-skeleton ring.

BUILDING/CASTLE VISUAL POLISH (banked 2026-07-06, player report):
BOTH ITEMS RESOLVED 2026-07-06f (see the fix-session block below):
- Destroyed/damaged buildings' strange texture = our fire burn
  conversion wrote the PAINT CODES (0x14/0x15/0x16) as tile TYPES
  (20/21/22 — unrelated families) instead of routing them through
  sub_33800 to the damage-stage types 10/11/12.
- The castle flatten = our m41 leveler APPROX (flatten-to-mean);
  the original is a uniform ADDITIVE translation of the whole
  footprint — the tower rides along by construction.
- METHOD NOTE (player directive): the player is re-validating all
  spells; for unknown/contentious/undocumented behaviors they will
  extract ground truth from ORIGINAL GAMEPLAY (DOSBox) rather than
  decompile-only reasoning — treat recorded original play as the
  senior source when remc1 is truncated/suspect.

PLAYTEST-4 PICKUP (session-close record 2026-07-06 — read this + the
fix-session block above before the next session; the player re-runs
the full spell validation next).

A. VERIFICATION CHECKLIST (what changed, what to look for):
- Fireball: max-range midair expiry now explodes AT altitude
  (drifting ±32/tick flicker, never dropping to ground).
- Meteor: trail = in-air fiery tail along the flight path (fires
  only ground where the meteor flew low); impact blast ROUND
  (concentric even-then-odd ring waves, ~6.25 tiles max), damage
  footprint still box-shaped (authentic).
- Possess: shots now curve TOWARD mana balls/houses (never toward
  creatures), skim rising terrain, and claiming chimes (sound 4);
  claimed balls/houses only re-chime on owner CHANGE. Claim has no
  further visible effect yet (mana track).
- Earthquake: discharge sound 9; the crevice wanders from a RANDOM
  initial heading (not the aim!) — check against original play;
  rumble loop 10 while digging; it's now a kill zone.
- Volcano/Crater: discharge 15; rumble 10; kill zones (bystanders;
  the caster is immune). Volcano: finite eruption (~4s window of
  ballistic bombs from the crater + plume), possible rare
  re-eruptions, and the one-per-map-at-a-time register.
- Create Castle: sound 15; ball flies 16 tiles, castle rises over
  ~1s, gong 10. [KNOWN ISSUE resolved 2026-07-06f: the leveler
  flatten was our APPROX — see the fix-session block.] Lockout
  while the ball/token lives; the RECAST on a standing castle is
  now the UPGRADE. Teleport anchors to the built castle.
- Lightning Bolt: endpoint = the standard explosion look + damage
  500 — TRACE-VERIFIED AUTHENTIC, calibrate perception; new
  thunder-crack 24 at the endpoint.
- Lightning Storm: one carrier at the aim → cloud climbs to 1024
  above terrain → 2 bolts/tick x33 at 2000 + thunder per tick.
- Undead Army: 8 skeletons rising in a ring, never attacking the
  caster, no mana drops. (Conversion of humans into more skeletons
  NOT ported yet.)
- Wall of Fire: fireball-like bolt → 15-wave rising flame curtain.
  Authentically SILENT at the effect in single player.
- Rapid Fireball: per-shot machine-gun discharge. Steal Mana:
  discharge 9 (trace beat memory — verify vs original play!).
  Mana Magnet: discharge 40. Duel: discharge 9. Portals: teleport
  sound 22 on use. Global Death: explosion 30 at expiry.
- Ear checklist: rumble 10 during all terrain deformation; lava-bomb
  rest-fires crackling; claim chime 4 gated to the player.

B. TRACE BANK — findings traced this session but NOT implemented
(the agent reports are session-local; this is their durable home):
- DUEL GRIP CHAIN (mortality/AI-wizard track): (9,7) → (10,26)
  effect (ctor :47116: +44=200, life 8, sprite 284); tick sub_263C0
  :28949 broadcasts ch4 200/tick; the VICTIM's ch4 inbox
  (:55663-77) writes the ATTACKER's Type_160 u16_314=victim,
  u16_316=200, u32_318=clamp(dist,1024,3072); the flyer (:55228-48)
  then physically PULLS the attacker toward the victim until the
  counter reaches 1000 (~800 ticks), dist ≥ 5120, or the victim
  dies. This is also the kraken-drag machinery already ported as
  the buffet — reconcile when wizards exist.
- MANA MAGNET REAL CHAIN (mana track — ours is an APPROX puller):
  spell 19 = (9,17) → (10,54) effect; sub_29920 (:31234-57) writes
  ch4 onto mana balls (class-10 m39) = the attraction; ball ch4
  consumption is the same protocol as the duel grip. Replace our
  (10,40)-state-21 magnet when the ball-motion/collection semantics
  land.
- POSSESS LEFTOVERS [CLOSED 2026-07-06f: the -1536 is INERT in the
  original — all ch1 readers discard the value; ownership flip IS
  the drain via the census]: model-40 GROUP-TRANSFER entity (sub_275C0
  :29636): on ch1 claim by a class-3 wizard it reassigns +144 of
  EVERY entity owned by it, then dies; ball ctor mana 512 (2500 on
  a flag, :47460-62); houses gain the ch1 accept bit only when
  BUILT (+28 |= 2, :43735 — ours matches); entity list heads
  gamedata+36462 (class-3), +36466 (m39/40 balls), +36470 (m45
  houses), +36474 (class 9), +36382 (20 per-model class-5 lists),
  rebuilt every tick :52246-320 — our pool scans replace them.
- CASTLE HOUSEKEEPING (banked ladder) [LANDED 2026-07-06f except
  the downgrade path + AI-wizard castles — see the fix-session
  block]: mana capacity by level =
  sub_47C60 :56572 / list sub_47DD0 :56617 (5000/10000/20000/
  40000/...); RECAST on own castle = (9,10) with +68/69=(10,43)
  UPGRADE TOKEN homing at the castle (+146=castle idx, :65904-08);
  castle case-0 upgrade gate = sub_12D10 space check; DOWNGRADE
  path sub_47A70 :56513 (sound 30, collapse steps); AI wizards
  cast castles via case 0x10 :19202; LEVEL-INIT castles for
  wizards w/ starting castles :54974 (sound 30; BALLOONS spawned
  per level :54985-94); byte_38C97[player] = initial castle
  levels; settler house build row = (rand&7)+25 :24886, authored
  = data_14+16 :43143; sub_37150 also writes the odd +78=0xE000
  z-marker (we skip it — would z-orphan AABBs; find its real
  consumer someday).
- UNDEAD LEFTOVERS: skeleton CONVERSION arm (roam state cycles
  class-5 m4/m12/m13 lists; human within 0x600 → deleted, a new
  (5,9) spawns there, owner inherited — :23837-923, dormant
  variant :24040-115); melee = (9,13) bolt, damage 600 when +144
  set / 400 wild (:21953-56); expiry = anim 245 bone pile,
  dormant, re-rises after 50 ticks (:24017-27); army-pool +44
  semantics from the cast (:65967-72: 1 normally, spell +136 vs
  mana-rich targets) vs remc1's :29366 hardcode 10000 (suspected
  maintainer patch — original may size +140 shares from it).
- COMBAT-ENGINE FACTS: sub_127E0 (ch0 writer used by the terrain
  deformers) ALSO stamps wizard +50=30 unconditionally incl. the
  CASTER (:17523) — a ride-the-deforming-ground timer (class-3
  state-4 pins z to terrain while it counts, :55978-99); wizard
  damage bypasses +28 masks / +16&8 / +66-67 filters entirely
  (dedicated model-2 list loop; the cell scan skips c3 m2) — our
  player probe applies filter_admits, semantic diff, harmless at
  -1/-1; mailbox accumulate rule confirmed (:17301-05); bolt
  quarter-damage vs shielded wizards (+17 sign bit && +140 ≥
  proj+140/4 → effect +44>>2, :63434-41); projectile rebound
  bounces only explosion models 1/17/53 (:62713-22); water hits
  splash, never blast (:62690-98).
- SOUND-ENGINE FACTS: id 10 = mode-3 LOOPING group {7,8,10-13,
  32-39,41} (:64576-97); 3/9/15/24/30 = mode-1 one-shots; 24 =
  the (10,23) flash one-shot (:28911); (10,14) walker exists
  (ctor sub_3AB40: state 14, filter 10/14, speed rand%0x35+51,
  life rand%0x21+28) — purpose unknown, no caller traced.
- CLASS-9 STATE TABLE TRUNCATION: remc1's table ends at state 13
  (:4853); the original spans ~22 entries (0x255870-0x25573C).
  Model→state: 0-13→0-13, 14→15, 15→16, 16→17, 17→18. Surviving
  candidate handlers for the gap: sub_54290 :63830 (decay),
  sub_542B0 :63841 (homing, (10,12)+explosion), sub_54480 :63928
  (instant transform, copies +44), sub_53B50 :63525 (steer-to-
  +150). Wall-of-fire state 17 = sub_53B50-shaped (our port);
  ORIGINAL GAMEPLAY footage is the arbiter for these.

C. KNOWN APPROX/RISK REGISTER in the new code (all cited in code):
- m41 castle leveler tower-flatten: FIXED 2026-07-06f (uniform
  translation, exact trace — see the fix-session block below).
- (10,19) plume tick untraced (ours: countdown + anim only).
- Lava-bomb downhill roll uses a central-difference gradient
  (sub_41F50's exact table unported); bomb ch0 contact damage
  unverified (fires carry the damage).
- Storm carrier flies straight for us (case-7/8/B/C acquisition
  scans the class-3 wizard list — empty until AI wizards); case
  0xC castle preference (sub_54BD0 cost) unported.
- Castle ball steering = per-tick snap (original eases with type-
  record turn rates); castle sub-state 6 = our invented wait state
  (original's flow between painter and leveler unclear).
- Quake-walker digger f82 copy is part of the +80 dword (ours
  copies both — verify no independent f82 semantics).
- Napalm cloud extents swap (1024 ctor → 512/2048 in tick) ported
  verbatim; its ch0 write is 0 by integer division (verbatim).
- Load-time feature ticks pass ctx=None (no damage/sound at load;
  original broadcasts into the half-built pool — nothing
  observable known to survive, but level-032-style authored
  effects deserve a regression eye).
- Eruption globals (erupting/plume) = Gen fields with slot-reuse
  guards the original lacks (it dangles indices — quirk parity
  only matters if a replay ever crosses it).

PLAYTEST-4 FIX SESSION (2026-07-06f — castle flatten + damaged-wall
textures + THE MANA ECONOMY; 3 remc1 trace agents + port pass; all
workspace tests green incl. new castle-upgrade + balloon-collection
integration tests):
- CASTLE LEVELER FIXED (sub_28200 :30284 exact): the m41 pass is a
  uniform ADDITIVE TRANSLATION — every footprint tile gets the SAME
  signed step (target-current)/counter each tick; the tower rides
  along by construction. Target = 4-corner OUTSIDE average
  sub_361C0(x0-1, y0-1, h+2, w+2) clamp 220; scalar current starts
  at castle site z>>5; counter 10..1 add, then the protection dance
  (0x80→0x08 on the last add, 9 idle ticks, restore at -1); finish
  writes castle sub-state 2 + site z = 32*final + depth-3 perimeter
  smooth. Our invented castle wait state 6 = the ORIGINAL's state 6
  (:56132) — vindicated. Painter finish now PROMOTES only bit-0x08
  tiles (:30697-707), not the whole rect.
- DAMAGED-WALL TEXTURES FIXED (fire cell sub_24F60 :28080-105 +
  sub_33800 full decode): the burn ladder repaints via PAINT CODES
  0x14/0x15/0x16 → sub_33800 → unk_909BCx rows = tile TYPES
  10/11/12 (white-wall damage stages; pristine = 26 edge / 27
  corner [corner tiles IMMUNE]; type 12 terminal). Our fire
  conversion wrote the codes AS types (20/21/22 = alien families) —
  the player's "texture never seen before". Wall paint code 0x10
  never repaints 10/11/12 (rebuilds keep damage, :41040-44 — ours
  already matched). One damage stage per fire cell landing,
  entirely independent of building health (sub_28DC0 writes NO
  tiles).
- COLLAPSE WALKER VERBATIM (sub_28FE0 :30835): per-cell semantics
  by hi nibble — 0 = unprotect only (texture kept!), 3 = unprotect
  + knock-down (sub-code 1 drops BOTH -12 AND -16 — decompile
  fall-through, ported verbatim) + per-tile sub_33E10 retexture,
  walls = corner code 1 + retexture BEFORE the height drop (height
  ≤ 4·(lo-1) → 0 exactly); evacuee z drops every 8th STREAM byte
  (control bytes count), spawn at tile corner (no +128); base z =
  footprint avg4 when the event has a model (castle demolish's
  zeroed fake event → z>>5); finish = the full-rect 3x3 vertex
  smoother sub_36080 (not a retexture).
- SPELL TABLE REINTERPRETED (sub_55DD0 :64909 gate + sub_55E80
  :64936 debit): ctor a4 (+136) = the spell's TOTAL MANA COST
  (gated + debited per cast); a8 (+132) = REQUIRED CASTLE STORED
  MANA — the spell-unlock ladder (teleport/magnet 10000, wall-of-
  fire 12000, duel 16000, steal 20000, bolt 25000, invis/rapid
  50000, storm 90000, meteor/crater 100000, quake 120000, undead
  150000, volcano 180000; magic bomb = the frozen 199488 artifact).
  The debit rides the wizard's regen delta (+132) — remc1 ships it
  COMMENTED OUT (maintainer mis-fix #3); we restore the original.
- MANA ECONOMY LANDED (sub_48230 :56839 census, :52327 call): the
  wizard ceiling (+136) = intrinsic 1000 + Σ +140 of every CLAIMED
  entity (class 5, castle, balloons, m39 balls, m45 houses; m40
  totems excluded); houses also feed the banked tally (u32_308);
  world total tracked. Regen (+132): max/200 floor 1000 touching
  the own castle, max/2000 floor 100 afield; pool clamps [0, max].
  Regen runs BEFORE cast handling (original wizard-tick order) so
  debits land next tick. LoadoutView exposes banked/world/castle
  (stored, cap, level) — the HUD castle panel (sub_22E50 :27172,
  level digit 43+lvl, capacity+banked bars, alert flash 55) is a
  UI-track item with data now live.
- CLAIMED BALLS RECOLOR (sub_274D0 :29572): sprite = owner-color
  row base (105 + 8*color; wild = 52) + size class over
  {256,512,1024,2048,4096,9192,18384,36768}; our sole wizard =
  color 0.
- CASTLE LADDER + UPGRADE CHAIN (sub_47DD0 :56617 caps 5000/10000/
  20000/40000/80000/160000/320000/30M by level 0-7): recast on the
  own castle = the (9,10) ball flying AT the castle, morphing into
  the (10,43) token (sprite 41, life 8, +44 = -1536 dead weight)
  which mails castle ch5 {10, owner} on touch (:31033-34); castle
  case-4 intake (owner-only, max level 7) → the level-up arm with
  sub_12C50 house pre-clear (footprint+256 kill) + sub_12D10 space
  gate (no castle overlap, new-footprint edge ring free of the
  protection bit; reject = silent bounce to established). Level-up
  sets capacity from the ladder. Spell-built castles start at 0
  stored (the level-INIT full-castle rule is the wizard-castle
  track).
- BALLOONS + COLLECTION LANDED (ctor sub_37A00 :44266: life 10000,
  speed 48, cargo cap 10000, behavior row 9, sprite 169; dispatcher
  sub_47400 :56264 every other castle tick, fleet (balloons,guards)
  = L1(1,0) L2(1,0) L3(1,4) L4(2,6) L5(2,14) L6(3,18) L7(3,34),
  guards = class-5 m15 HP 512; tick sub_47F90 :56716): targeting =
  castle when full else nearest own claimed ball unclaimed by
  siblings; ball tether flag 0x40 (>1024 clears, near sets + ball
  homes); touch absorbs cargo + refreshes life; delivery ring =
  level*speed at the altitude-band floor; death drops cargo as a
  claimed ball; row-9 altitude servo everywhere. Direct castle
  absorption of owned touching balls while below cap (:57023-32).
- POSSESS DRAIN VERDICT: the -1536 is INERT in the original — every
  ch1 reader discards the value; "drain" = the ownership flip
  moving +140 between wizards' ceilings at the next census. Ported
  as dead weight; the trace-bank entry is CLOSED.
- PLAYTEST-5 IMMEDIATE FIXES (same day, player report on the first
  economy build):
  1. CASTLE UPGRADE THRESHOLD FOUND (the player's doubling-cost
     memory was RIGHT; the trace agents missed it): wizext +708 =
     var_676.var_u16[16] = the owner's SPELL-16 MANIFESTATION slot
     (NOT "partner wizard" — glossary correction). sub_47C60/
     sub_47DD0, called at castle init/level-up/downgrade, REWRITE
     the castle spell's cost (+136) to the capacity-ladder value at
     the castle's CURRENT level (level 1 → next cast costs 10000,
     2 → 20000, …; the fresh no-castle cast keeps the ctor 1000) —
     capacity and next-upgrade cost are the same doubling number.
     The model-16 trigger arm (:55901-11) gates SILENTLY on wizard
     +140 ≥ that cost and FIZZLES (29) while the manifestation's
     charge is pinned through the build (sub_46D20 = the charge
     pin, not "balloon reset" — second glossary correction). Ported
     as a dynamic cost in the cast arm + lockout buzz.
  2. CASTLE FLAG (site-z split): the original keeps entity z (+76,
     refreshed to live ground EVERY tick — idle :56014 + wait cases
     1/4/6 — so the flag rides the painted tower top) separate from
     the build-site datum (+154, the painter/leveler target). We
     had conflated them — flag sat buried at base height. Castle
     f28 now = site z; entity z refreshes per tick.
  3. GUARD DEATH CYCLE: guards spawn at (x+128, y+640) ON THE
     GROUND (the courtyard — not the tower center/slope where ours
     died), facing 512, throttled by the castle's +46 cooldown
     (ours f46): at most ONE guard per dispatch pass, 16 passes
     between spawns (:56412-47). Ctor life kept (the earlier "HP
     512" note was the facing value misread).
  4. Castle HEALTH ladder ported (sub_47C60 a3: L1 20000, L2/3
     40000, L4+ 60000; L6/7 decompile-garbage consts → 60000;
     damage carry-over min(deficit, new/2) = castle-HP track).
  5. Absorption moved inside the every-other-tick block (:57023).
  6. POSSESS FEEDBACK (playtest-5 round 2 — capture "failed"
     because it was invisible+silent; the claim machinery itself
     was proven working by a new house-claim test): (a) the claim
     chime anchors at the CLAIMANT (:29444/:30806
     sub_55370(claimant, -1, 4)) — ours anchored at the ball/house,
     and the player-gated id-4 policy then dropped it = silence
     (sound 4 = "selectsp"; the player's "high pitched ahh" —
     ear-check vs retail owed; sound 11 "possess2" is only played
     by GENIE attack states :24663/29/96, and the earlier "(10,12)
     ctor sound 41" note was a misread of sub_36FA0 = the SPRITE
     setter); (b) the (10,12) flash carries its ctor sprite row 41
     but draws NOTHING in retail (player-confirmed after a brief
     wrongly-visible round — excluded from drawables; the ctor's
     +16 & 0xF6 bit-3 clear is likely the real draw gate, untraced); (c) m45 house entities were excluded from
     the drawable list entirely — the claimed flag (sprite 177 +
     owner color) never rendered; now drawn when CLAIMED only
     (APPROX: the neutral-state draw gate is untraced — the claim
     clears +16 bit 0, meaning unknown; models 12/40 added to
     drawables too); (d) captured buildings immune to their OWNER's
     damage = PLAYER GROUND TRUTH implemented as an intake guard —
     NO substrate in the decompile (ch0 writer sub_120B0 and intake
     :31070 have no owner check) — DOSBox verification owed.
  7. Castle site-z de-aliased: f28 briefly doubled as site z ON THE
     CASTLE while area_write reads f28 as the mailbox channel mask
     on every entity — site z bits could have opened phantom mail
     channels. Site z moved to a dedicated Ent field (+154); castle
     f28 = the authentic mask 33 (ch0+ch5, sub_37920 :44247).
  8. WORLD-RELATIVE MANA + THE WIN THRESHOLD DECODED (player
     directive: the original never shows absolute mana): the win
     check's byte_38C93[gamedata] (:52128) = gamedata+232595 =
     level block 193795 + 38800 = **the first u16 of the MC1 level
     FOOTER** — the required banked percentage of world mana
     (data-confirmed: level 000=35%, 001=40%, ramping to 010=90%;
     footer[1] = the known max-players field). Ported: World
     win_pct (wired from GenParams.footer[0] in the app), the
     verbatim completion check (castle-owning wizard, banked share
     STRICTLY over the goal for 16 consecutive ticks → latched
     flag, sub_415C0 :52100-40), LoadoutView win_pct/completed, and
     the HUD's world-relative castle-panel pair (capacity/world +
     banked/world bars with the goal tick at win_pct%, green once
     latched — sub_22E50 :27268-74 semantics; sprite-frame HSPR
     panel = UI-track polish). Completion → campaign progression =
     the campaign track.
- NEW APPROX/DEBTS (all cited in code): the m42 painter's per-cell
  DELTA-ARRAY mechanism + its counter-2/1 bit-3 dance (:30545-93)
  unported — ours re-decodes goals per tick, converging to the
  same heights (timing-only delta); upgrade space-gate edge walk
  ported from trace description (not verbatim code); ball tether =
  vertical-only homing; balloon slots = census scan (no wizext
  +52/+84 slot arrays); OVERFLOW EJECTOR (sub_47130 :56160 — 32
  scattered claimed balls + 4 markers past cap) UNPORTED; castle
  HP/damage/downgrade (sub_47A70, sound 30) unported; win check
  (banked% > byte_38C93[level] 16 ticks) not wired (data live);
  balloon-death slot cleanup drops cargo at the balloon (ours) vs
  the dispatcher (original) — same observable.

INTERIM/APPROX inventory (fidelity passes owed, all marked in code):
per-spell emission approximations (earthquake uses the m11 digger —
the authentic c10 m15 crevice walker is unported; undead army spawns
3 fixed skeletons vs the mana-scaled m36 spawner; lightning storm =
8-bolt fan vs chained m12; wall of fire = 5 standing fires; mana
magnet = 30-tick puller; steal mana keeps m8's wizard-only
detonation), castle spell = placement scan + build event only (no
balloon/levels/respawn — housekeeping), teleport's no-castle branch
= 64-tile LCG hop, duel latches nothing (no rival wizards), beyond
sight = flag only (our map is all-seeing pending map-authenticity),
mana economy pool/recharge semantics simplified (the original's
+308-recompute/possess-lock/per-spell +132 gates await the mana
collection track), starting-spell level data (Type_37072 runtime
struct traced, but its LEVEL-FILE source is NOT the runtime offsets
— the 1042-byte reserved block at 0x30 is the candidate, undecoded),
jar spell-id-from-model65 unverified vs retail.

THE SPELL ID MAPPING (player-confirmed 2026-07-06 from the book-order
naming + byte_99B88 permutation — internally consistent with stats,
icons, and every prior trace; treat as ground truth):
internal type → spell (MANUAL-OFFICIAL names, player-supplied with
full manual descriptions same day): 0 Fireball, 1 Heal (refills to
max while mana lasts), 2 Accelerate (hold = max speed; DOWN CURSOR
cancels), 3 Possess (claim buildings/mana), 4 Shield (absorbs 3/4
of spell energy), 5 Beyond Sight, 6 Earthquake (no effect on
water), 7 Meteor, 8 Volcano (periodic re-eruptions), 9 Crater,
10 Teleport (to castle; recast returns to the cast site),
11 Duel to the Death (locks two players; only Accelerate escapes —
player calls it tether), 12 Invisible (casting breaks the cloak),
13 Steal Mana ("useless for players, heavily used by genies"),
14 Rebound (deflects incoming FIRE spells; deflection-bit owner),
15 Lightning Bolt (hold = continuous stream, locks onto target),
16 Create Castle (one-active lockout; each cast launches a mana
balloon; recasts on the site upgrade it), 17 Undead Army
(red-cloaked skeletons attack wizards/castles/balloons),
18 Lightning Storm (radiates all directions), 19 Mana Magnet
(gathers local mana into one ball for balloon pickup), 20 Wall of
Fire (player: "fire storm, pretty useless"), 21 Accelerate
Backwards, 22 Global Death — yes, officially (player's "magic
bomb": 75000 possess, one-shots anything at point blank; the 032
"no magic-bomb on this level" spell; the old "behavior 66 =
possession" trace reading was WRONG), 23 Rapid Fireball (the dev
fireball's donor: refire window 3, f44=50).
Types 2↔21 are the traced mutually-exclusive toggle pair =
forward/backward thrust. "Damage 100" on utility rows = vestigial
constructor filler (shared 9-arg ctor sub_3BF70). Spellbook display
order = byte_99B88 = {0,3,2,16,1,14,4,12,6,9,7,8,15,18,17,19,13,
5,11,10,20,21,22,23}; icon = begSprTab[type+6]. MC1 shows NO spell
names in-game (ETEXT = 80 intro/menu strings only; ETEXT.DAT is RNC
despite the old roadmap note). MC2 note (player): MC2 names all
spells in data + 3 experience tiers per spell; reuse for naming/UI
when its track lands.

CAMPAIGN MODE (player note 2026-07-06, banked for after the spell
track): with spells in, "the entire campaign mode simply becomes a
sequence of levels with saves in between" — level progression =
campaign index order minus the skip table {8,17,28,33,39} (decoded,
see parking lot), inter-level save = the deterministic sim snapshot
plus the persistent spell-carry flags (var_892 semantics, ported in
the spell track) and mana/score totals; briefings from ETEXT
(strings 23-47, objectives 48+ — Text track). Completion detection
= the mana/possession threshold + trigger machinery already live.

DEV-SPELLS TOGGLE (player directive 2026-07-06, for early spell
playtesting): `enhancements.dev_spells` / `--dev-spells` / G at
runtime = all 24 spells granted + infinite mana. G-class (replays
taped with it on are not faithful fixtures). Authentic-adjacent:
the original ships "access all spells" / "more mana" debug commands
(remc1 :48836 cheat menu).

HSPR/MSPR FORMAT DECODED (2026-07-06): whole-file RNC (except
HSPR0-1, stored raw w/ minor deltas — DTABLES pattern); TAB = 6-byte
entries {u32 offset, u8 w, u8 h}, 87 sprites; payload = signed-RLE
rows (n>0 copy n pixels, n<0 skip -n, 0 = next row; index 0
transparent). HSPR = the 640x480 UI set (icons 62x34), MSPR = exact
half-size 320x200 twin (31x17). Spell icons = entries 6..29 keyed by
internal type; 1/2 = LMB/RMB highlights, 3/4 slot backgrounds, 40
HUD panel, 41/42 mana-bar frames, 43-52 level pips, 83/84 = the map
trigger X-markers. DATA/BOOK.PAL (RNC, 768B VGA 6-bit) = the book
screen palette; icon color path still unresolved (icons index a
~120-126 ramp that reads wrong under both PAL0-0 and BOOK.PAL vs
the real red heal heart — a blit-time remap is suspected, trace
pending).

## MC1 terrain oracle

### MC1 reference generator found (2026-07-04, remc1 clone)

`reference/remc1` (gitignored local clone) is a remc2-style decompilation of MC1 and contains
the actual MC1 terrain generator, complete. Entry:
`sub_31AA0_31AE0` (sub_main.cpp:39289), called from level load
(:51544) with the raw level buffer — i.e. GEN_MAP at offset 0, params
read by byte offset. This settles every open classifier question; the
plan changes from "fit the MC2 oracle" to "port the MC1 generator
natively" (it is ~15 small passes, all in hand).

Pipeline (offsets are GEN_MAP fields; PRNG is the same LCG as MC2,
`x = 9377*x + 9439`, seeded from seed(+4) — the TYPE LAYER IS
SEEDED-RANDOM, so a faithful port must consume PRNG draws in exactly
this order):
 1. Fractal `sub_725C8(seed, off(+8), raise(+12))` — diamond-square
    on the 256x256 torus into an i16 scratch field; `off` is the seed
    CELL INDEX, `raise` the value planted there. The amplitude-clamp
    input is gnarl(+16): remc1's decompile loses it in a frozen
    stack-reconstruction array (savedregs[5]=0 — reconstruction gap),
    but remc2's byte-identical fractal takes (seed, offset, raise,
    gnarl) explicitly, and our oracle validation already proved gnarl
    feeds MC1's fractal. Quirk: if `save/scanned.rmd` (0x10000 bytes)
    exists, it replaces fractal+normalize as the raw heightmap.
 2. `sub_32A50` — normalize: scale by 12845056/max (>>16), clamp to
    0..196.
 3. `sub_32AE0(river(+20), sourc(+24))` — class-map init (height!=0
    -> class 5, else 0=water), then carve `river` channels: up to
    1000 PRNG probes for a land cell with height > sourc;
    `sub_32B90` walks steepest-descent (8-nbhd min), clamping heights
    monotonically non-increasing along the path and marking it water
    (class 0, heights RETAINED — elevated river water). So RIVER =
    channel count, SOURC = minimum source altitude: the Bullfrog
    names finally parse. MC1 rivers are real 1-tile downhill
    channels, NOT the MC2-lriver=0 ponds we currently bake.
 4. `sub_33500` — flatten water shores to fixpoint (quads with 4
    water-adjacent corners take the local min height).
 5. `sub_320A0(snflt(+32))` — vegetation: class-5 tiles with 2x2-quad
    height diff < snflt -> class 3; == snflt -> class 4. Then grows
    class 4 wherever 3 and 5 meet without 2.
 6. `sub_32D00(bhlin(+36), bhflt(+40))` — low flat land (8-nbhd max
    < bhlin, diff <= bhflt) reverts to class 5.
 7. `sub_32300` — boundary dirt: where {3,5}, {3,water}, {water,land}
    meet in a quad -> class 4 ring.
 8. `sub_32EB0(bhlin, bhflt)` — interior of low flat class-5 regions
    (all 8 neighbors in {5,2}) -> class 2.
 9. `sub_33180(rkste(+44))` — quad diff >= rkste -> class 6; then
    class-6 tiles whose neighborhood mixes in 3, 2, or (5 and 4) ->
    class 1. rkste is UNSCALED — our 1.5x cliff-cut hack was
    compensating for MC2's different pass; drop it with the port.
10. `sub_31FA0` — majority smoothing: ALL 8 neighbors same non-water
    class -> adopt it (fills single-cell holes).
11. `sub_31BB0` — land height smoothing (8-nbhd average, thresholds
    4/10 on local relief).
12. `sub_31EC0` — sea-level shore flattening: quads touching class 4
    and height-0 water get zeroed heights.
13. `sub_32560` — texture selection. `unk_9075C`
    (sub_main.cpp:2648) is MC1's corner-class table, the exact analog
    of MC2's `unk_D47E0`: 148 textures x 4 corner classes; 0-6 pure,
    7-34 = 0xFF (building slots, never generator-picked), 35+ =
    transitions. Builds 7^4 buckets of (texture, orientation)
    candidates over all 8 dihedral corner arrangements (orientation
    codes 0x00..0x70 = exactly our angle bits 4-6), then per tile:
    key = quad corner classes (343a+49b+7c+d), PRNG picks among up to
    12 candidates — quirk: `rand % (n+1)` with overflow mapped to
    candidate 0, doubling its weight — no match -> texture 1. Writes
    tile type + orientation bits. (A flat 2401-entry {texture,orient}
    fallback table `byte_B5D40` is also built — likely what repaints
    terrain after deformation in-game.)
14. `sub_31D40` — deep-water flag: water tiles fully surrounded by
    water with no typed land nearby -> angle bit 3.
15. `sub_329C0` — shading: `shade = h[x-1,y-1] - h[x+1,y+1] + 32`;
    flat (==32) -> PRNG dither 28..36; clamp <28 -> 28..31 (&3),
    >40 -> 40..47 (&7). pseudoRand is reset to 0 first.

MC1 class semantics (texture N = pure class N; colors from our baked
tile-colors-0): 0=water, 1=dark basalt (85,77,73), 2=sand
(186,150,101), 3=vegetation olive (101,89,20), 4=dirt brown
(154,109,65), 5=sand-variant (same map color as 2), 6=brown rock
(125,89,60). snlin(+28) is read by NOTHING — there is no snow pass in
MC1; snow worlds are purely the arctic tileset's textures (see the
tileset-selection note under textured-terrain follow-ups).

This RESOLVES the barren-world "dirt vs sand" residual: MC1's steep
terrain is class 6 (warm brown rock, reads as dirt) with dark basalt
only as class-1 transition accents, plus pervasive class-4 boundary
dirt on rough worlds — a structure MC2's classifier cannot produce.
The interim 5->4 reclass idea is dead; the fix is the native port.

PORTED (2026-07-04, same day): `mgc-import/src/mc1_terrain.rs` is the
native Rust port of the full pipeline; `mc1_oracle_payload`, the
rkste*1.5 hack, and the 5->4 reclass (`terrain_classes.rs`) are gone,
and MC1/HW bakes need no external tool (MC2 keeps its oracle).
Validation: heightmaps byte-match the MC2 oracle on river-free levels
(000, 050, 069: 100.0%; others 99.5-99.9%, residue = MC1's extra
smoothing/shore/river passes); water fraction matches within 0.7pp
worst-case over all 143 levels; the entity-coherence canary holds
(level 001: same wet-count as the oracle). Levels with many class-2
entities "in water" (069: 322 standing stones, 067, hw water worlds)
are IDENTICAL under oracle and native — by-design placements, not
divergence. PORT LESSON: the fractal's 4-corner sum accumulates in
16-bit registers (`int16_t sumEnt`, remc1's `__int16` chains) and the
overflow is LOAD-BEARING — deep-water worlds (raise=-10000) wrap the
sum positive, and widening it to i32 collapses them into flat all-land
plateaus (uniform height -> flat -> all-vegetation = the "flat green
plain" the player caught mid-session from an intermediate bake).
Level-020 rivers now render as winding channels into the twin-lake
basin. Confirmed from live params: level 000 carries gnarl=0 and
remc1's frozen `savedregs` = level 000's exact (seed, off, raise,
gnarl) — the savedregs[5] slot is gnarl beyond doubt. Type layer
still awaits an eyeball pass against the player's DOSBox captures.
Dev aid: `cargo run -p mgc-import --example mc1gen_probe` (stats from
level params, raw or from an archive). remc1 licensing: same GPL3
assumption as remc2.

### 1:1 validation pass — COMPLETE (player, 2026-07-04)

All MC1 levels validated side-by-side against DOSBox: "terrain is
just solid and perfect." The MC1 terrain track is CLOSED (remaining
visual deltas are the known entity-mod/building/sprite gaps below).
Byproduct findings: the campaign skip table (see parking lot), rivers
player-verified on level 010, and level 039 confirming the generator's
degenerate collapse is authentic (see below). Method notes kept for
reference:

Our side pre-rendered: `baked/maps/mc1/level-NNN.png` and
`baked/maps/mc1hw/` (512x512, HW in arctic tileset colors) — regenerate
with `mgcarpet --level ... --map ... --map-scale 2 [--tileset 1]`.
EXPECTED deviations from DOSBox (do not chase as generator bugs):
- Canyons/craters/walls/building-flattening/painted buildings: LANDED
  2026-07-05 (see "Static terrain features") — no longer expected
  deviations; `baked/maps/` regenerated with them applied.
- Entity dots: LANDED 2026-07-05 (see "Map entity dots") — no longer
  an expected deviation; remaining map-only gaps are the balloon
  sprite icons and rival name labels.
- The original map pane floats/rotates with heading and doesn't fit
  all 256 tiles; ours is the full axis-aligned torus.
- MC1's fullscreen map draws top-down TEXTURES; ours draws the flat
  map-color table — compare shapes and color classes, not texel
  detail.
REAL signal worth reporting: coastline/lake/river SHAPE differences,
land-class differences (dirt where grass should be etc.), missing or
extra river channels, and any level that looks degenerate (flat,
single-color) — those are generator-port bugs.

### Prior MC2-oracle work (historical)

RESULT (2026-07-04): hypothesis CONFIRMED for heightmap/water — MC2's
generator on MC1 seed params (mapped 1:1, `lriver=0`) reproduces MC1
terrain shape. Evidence (no DOSBox ground truth needed for this
verdict): entity-placement coherence over 10 sampled levels —
1733/1734 land entities land on generated land (level 001 is 82%
water yet places 227/228; level 002 is 91% water with 124/124);
krakens hit water 46/51 on maps that are <35% water; village/forest
clusters follow shorelines and valleys exactly. The few misses are
all near-shore tiles (shoreline micro-divergence or authentic shoals
— undecidable without ground truth). Level 050's apparent outlier
was a misclassified amphibian (crabs, on a raise=-10000 water
world). `raise` survives u16 truncation because the generator reads
it as signed __int16. Regression canary:
crates/mgc-import/tests/bake.rs `mc1_terrain_generation_is_coherent`.

Caveats:
- Tile-type layer, RESOLVED (2026-07-04, textured-terrain session,
  player-validated against a DOSBox map capture: "can't really see a
  difference in terrain"). The wrong types were our param-mapping bug:
  phase 3 mapped MC1 GEN_MAP params to MC2 payload slots POSITIONALLY,
  trusting remc2's MC2 field names — but those names don't describe
  what the generator passes do. The Bullfrog names map onto the passes
  SEMANTICALLY (mc1_oracle_payload doc has the full table):
    snlin/snflt → snow pass sub_454F0 (above altitude snlin, flatter
      than snflt → class 6; snlin=200 > max height = "no snow", the
      value on all temperate MC1 levels);
    bhlin/bhflt → beach pass sub_45060/45210 (below altitude bhlin,
      flatter than bhflt → class 2);
    rkste → cliff pass sub_45600 (local diff >= rkste → class 1);
    +0x33 vegetation pass (slopes flatter than the cut → veg) → snflt
      AGAIN, the same param feeding both snow and vegetation passes
      (established by the player's level-003 comparison: snflt=7 =
      barren rock world with green pockets, snflt=50 = lush; level-000
      output byte-identical under either 255 or snflt=50). High snflt
      = lush, low = barren. `sourc` feeds no generator slot.
  BONUS: `snlin < 200` marks the snow levels — MC1 campaign 10, 17-18,
  34-47, 50, 63-66 + nearly all Hidden Worlds — which is almost
  certainly the arctic-TILESET selector too (level-010 renders as a
  proper snow world under tileset 1). Confirm against play memory
  before auto-selecting tileset by it.
  The type-class machinery is documented by `unk_D47E0` (per-texture
  table of 4 corner classes: types 0-6 = pure classes, 35+ =
  transitions; `building_F2CD0x` is built by inverting it).
- RESOLVED by the remc1 reference generator (see above) — kept for
  the data points: on barren worlds
  the original's ground is DIRT (class 4, warm brown 154,109,65); our
  oracle produces SAND (class 5, pale — same color as beach) + basalt
  above rkste, which the player read as "too much rocky basalt". MC2's
  generator structurally cannot blanket-produce class 4 (no dirt pass)
  — first genuine MC1≠MC2 classifier divergence. Candidate interim
  fix: post-bake reclass 5→4 for MC1 (transitions remapped via
  unk_D47E0 corner substitution) — NOT applied, pending data points
  from the player's level-by-level run: levels 004/005/009 (same
  barren params — same brown ground?) and whether inland pale sand
  exists anywhere in MC1 (a true desert level would veto the blanket
  rule). Cliff cut (rkste vs ~1.5x) to be fitted jointly then; note
  level 000 has only 4 basalt tiles, so earlier validation never
  constrained the cliff pass. Definitive answer = MC1 reference
  classifier.
- The river question: RESOLVED by remc1 (rivers = carved downhill
  channels, see above) and PLAYER-VERIFIED 2026-07-04 — side-by-side
  DOSBox map vs our render on level 010 shows matching winding
  channels. CLOSED. Prior reasoning kept below: player recalls MC1
  having
  no rivers as terrain features (MC2-only). Measured: with lriver=0,
  MC2's river pass on MC1 params produces scattered PONDS, not
  channels (consistent with rivers = `river` count x `lriver` length;
  zero length = pond) — so both can be true. MC1's RIVER/SOURC params
  are Bullfrog-named and meaningfully varied in the data, so MC1's
  generator consumed them somehow; whether it made these same ponds
  is unverified (krakens sit in base-fractal pools either way, so the
  entity test is blind). DECISIVE TEST: DOSBox map screenshot of
  level index 20 (21st campaign level, river=58) — pass-on render is
  peppered with ponds + a beach-ringed twin-lake basin, pass-off has
  two bare lakes. One glance settles it. If MC1 shows no ponds, zero
  `river` in mc1_oracle_payload (`sourc` no longer feeds the oracle at
  all — it turned out to hit MC2's highland-line field, see the
  tile-type caveat below).

Experiment script: tools/mc1-oracle-test.py — synthesizes a fake MC2
level buffer with MC1 params at the oracle's offsets, scores entity
coherence, renders overlay PNGs.

## Asset tracks (each independently optional, like game data)

- **Textures**: terrain atlases DONE for MC1 (see Phase 4 "Textured
  terrain") and MC2 (2026-07-05, "MC2 environment bundles" below).
- **Sprites**: LANDED 2026-07-05 for MC1/HW (see "Billboarded sprites"
  below). MC2's sprites live on its CD image (blocked with the rest of
  its DATA tree); its TMAPS variants bake into the same bundle schema
  when they land. DESIGN CONSTRAINT (player, 2026-07-04): keep the
  door open for
  replacing sprites with real 3D models later (an "unfaithful remake"
  visual mode — the 8-rotation billboard snap is period-authentic but
  not a virtue). Concretely: the sim carries continuous entity pose
  (position + real-valued yaw + animation state) and never quantizes;
  the renderer's entity layer maps (class/model, pose) → visual via a
  representation backend — billboard-sprite (authentic default: pick
  nearest of the 8 views at draw time) or mesh (opt-in via the
  authenticity matrix, model assets community-supplied). Rotation
  snapping is a sprite-path rendering detail, same stance as
  palette-as-LUT vs enhanced color.
### Billboarded sprites + static entities (LANDED 2026-07-05, MC1+HW)

The world is inhabited: every drawable level entity stands at its spawn
position with its authentic sprite, size, and rotation-view behavior.
No behavior/AI/animation ticking yet. What shipped, and the decoded
architecture (full byte-level notes in the agent traces; key facts
here):

- **Unified asset bundles** (user directive 2026-07-05, schema in
  FORMAT.md "Asset bundles", types + loader `mgc_formats::bundle`):
  one modern-engine-friendly format spanning all three games; variants
  `mc1-temperate` / `mc1-arctic` and (2026-07-05) `mc2-day` /
  `mc2-night` / `mc2-night-fog` / `mc2-cave`. Replaced the flat
  `baked/mc1/assets/*-{0,1}.bin` scheme (bake removes it). Levels
  auto-resolve: mc1 → temperate, mc1hw → arctic (`--tileset`
  overrides); mc2 → by header map_type + the gfx_type fog bit.
- **TMAPS = the world billboards** in both games (HSPR/MSPR turned out
  to be the 2D UI sprite library — spell icons/panels, MSPR = low-res
  variant; a UI-track concern). MC1 TMAPS payloads are RAW 8bpp pixels
  (index 0 transparent), per-entry RNC; TAB group field = rotation/
  animation family.
- **Animated TMAPS entries decoded** (the roadmap's old "extended
  payload" mystery): after the base image sits a real Autodesk
  Animator FLI/FLC stream — `{u16 frames, u32 len}` then 0xF1FA FRAME
  chunks of DELTA_FLC (type 7) / FLI_COPY (type 16) deltas. Decoder
  `mgc-import/src/flc.rs`; all 428 animated entries across both
  tilesets decode byte-exactly. Bundles store fully pre-decoded frames.
- Retail data bug: TMAPS1-0 entries 153/156 ship corrupt headers
  (1x122 with a neighbor's payload) — baked as frame-less
  placeholders, ids stay dense.
- **Sprite selection architecture** (remc1): entity → type index
  (offset 86) → `unk_99BA0x[286]` stats row {sprite_base, world
  width/height in 1/256-tile units, shade, draw_type}. Extracted to
  `mgc-sim/src/mc1_sprite_stats.rs` by tools/extract-remc1-tables.py.
  EXTRACTION HAZARD: the decompiler zeroes var_6/var_12 in the braced
  initializer; the true values live in the overlapping alias array
  `word_99BA6` (same memory +6 bytes, sub_main.cpp:6049) — the script
  merges both with continuity cross-checks. The engine also refreshes
  draw_type at sprite-load from the sprite's OWN flags high byte
  (:66708), so bundle `flags >> 8` is equivalent — and world width or
  height may be 0, derived from the other via the sprite's pixel
  aspect at load (:66699).
- **Draw types** (view selection; DrawSprite3D :37552): 0/1/21 single
  view (21 additionally skips the anchor half-width shift), 2..=16
  animation (entity anim byte added to sprite id), 17 = 8 views +
  mirrored back half, 18 = 16 views, 19/20 = 5-/3-view folds via
  byte_906E8/906F8. View sector = `((entityYaw - camYaw) >> 3 & 0xF0)
  >> 4` on 11-bit angles = floor(rel/128) of 2048. On-screen size =
  `fovDist * height / depth`, aspect from the current view's pixels;
  anchor = bottom-center at terrain altitude; index-0 pixels skipped;
  fog = distance-banded shade LUT (same family as terrain).
- **(class,model) → type index** is per-model spawn-function logic (no
  flat table); traced for all classes → `mgc-sim/src/mc1_entities.rs`.
  Census-relevant: class 2 scenery (m0 tree = LCG-random 83/84 +
  ±32-unit position jitter; m1 palm 79, m2 dolmen 39, m3 270, m4/5
  48), class 3 m0/1 = the wizard-carpet balloon 44, m2 castle 177, m3
  169, m4-11 = the 8 player-start MARKERS (no entity — they fill the
  start-position globals), class 5 creatures (m0/m3/m6 multi-part
  worms → heads 40/88/49; m7 alternates 199/85 by spawn-count parity;
  m13 = 217 (4/7) / 218 (3/7); rest constants), class 7/11 = spawner
  volumes/logic (non-drawable), class 12 = mana ball 77, raw-
  overwritten to 280 (village-owned) when the THING's swi_id >= 3.
  Class 10 m45 buildings select multi-tile structures from BUILD
  footprints (begBuildTab), not billboards — entity track.
- Renderer: `mgc_render::Billboard` + instanced screen-aligned quads
  (billboard.wgsl), same palette/shade-LUT color path and fog as
  terrain, chunky integer texel sampling, torus wrap-nearest
  placement. App resolves entities via `mgc-app/src/entities.rs`.
- Deliberate approximations, for the fidelity pass: per-slot-seeded
  spawn LCG instead of the original's strict slot-order global stream
  (tree variant mix + jitter are stable but not byte-identical);
  entity yaw is per-slot LCG (records carry none; original assigns at
  spawn); animated sprites show frame 0 (no entity ticking); sprite
  fog is the renderer's exponential blend at neutral shade, not the
  original's distance bands; growth creatures (class 5 m5) show their
  base form.
- Multi-part creatures render with full bodies (2026-07-05, after the
  player caught head-only worms on lost level 033): the original
  spawns the 16 segments STACKED on the head (state 120 — hidden from
  the map and behavior lists) and movement strings them out from the
  first tick, so we settle them in a trailing line behind the head
  (0.35-tile spacing — an invented resting pose; movement will own
  segment positions). Segment types 19-34 all share sprite base 112
  (rock ball) with a world-size ramp 150->210->60 = the bulge-and-
  taper body. Bestiary note (player-confirmed on campaign level 4):
  class 5 m0 = the FLYING worm (black winged head, sprite 54) — in
  play it should be airborne, our grounded pose is tick-0-ish; m3 =
  the ground worm (crescent head 62); m6 = the kraken (finned hump
  22, parts base 119). The player had never seen m0's head sprite
  because 017/033 (where he first met it in the flyer) are skip-table
  lost levels.

Sprite depth semantics = PAINTER-ORDER DEPTH (2026-07-05g,
player-reported: sprites near vertical walls clipped, ground portals
vanished under pitch — two rounds): the original renderer draws
tiles back-to-front and blits each tile's queued sprite RIGHT AFTER
that tile's own triangles (remc1 :33673-74 — DrawSprite3D inside the
tile walk); occlusion is pure painter order at TILE granularity, no
depth comparison exists. A first fix (per-sprite anchor depth vs
true ray depth) was wrong in both directions — a wall's upper
reaches are genuinely nearer than a portal at its base (still
clipped), and a glancing wall face can be farther than a jar hidden
behind it (showed through). The real emulation: the 3D depth channel
now carries HORIZONTAL (plan) camera distance for every pass —
terrain writes `length(world.xz - cam.xz)/768` as frag_depth (on a
heightfield, plan distance orders identically to ray depth along
every view ray, so terrain-vs-terrain occlusion is unchanged);
billboards and health bars write their anchor TILE CENTER's plan
distance minus half a tile (the "after its own tile" rule) as a flat
varying. Sprites are never clipped by walls they stand against
(farther tiles), always hidden by tiles in front, ridge silhouettes
still occlude partially, and same-tile sprites resolve by draw
order — the original's exact compositing, including its
tile-granular popping when a mover crosses a tile boundary behind a
wall edge. Cost: frag_depth disables early-Z (trivial at our scene
complexity).

Sprite-track follow-ups: ~~water/lava animated surface tiles~~ (dead
premise — terrain tiles never render from TMAPS; the animated ocean is
vertex displacement, see "Terrain water animation"); ~~mana-ball and
creature idle animation~~ (DONE 2026-07-05, same section: animated
sprites now cycle their FLC frames one per turn); the balloon at the
class-3 player start; HSPR/MSPR UI sprites when the HUD/spell UI
track starts (MC2's are per-environment: HSPR{D,N,C}0-0).

Map entity dots (LANDED 2026-07-05, same session): the overhead map
plots entities exactly like the original's overlay (remc1
sub_48710_48A50, :57050 — the same code draws the F8 corner map and
the book map): 1px per entity, switching on LIVE class. Trees/scenery
(live class 2, model 0, spawn state 0 — verified drawn) = raw palette
index 28; wild creatures = near-black via the engine's 16^3
RGB->palette LUT (byte_AD167_AD157, nearest-palette-match of
RGB(3+4r,3+4g,3+4b) 6-bit — reproduced at load against the bundle
palette), wizard-OWNED creatures = the owner's team color
(byte_99B58[1+2*team], player = blue 0x71 — likely the player-recalled
"blue-ish humans", along with villagers m12-14 = dark green [16]);
pre-placed pickup jars (class 12, sprite 116 = the red jar) = bright
red [3841] — the vital red dots. Castle markers are NOT dots: the
original blits team-colored UI-SPRITE ICONS (begSprTab 58+team castle,
66+team for model 3; LABEL_66 dispatches v29>1 to DrawBitmap) —
pending the HSPR/UI bake we draw a team-blue dot stand-in. `--map`
PNGs now include the dots — DOSBox map comparisons cover entities.

ADVERTISED TRIGGERS (found 2026-07-06, player observed "large X"
marks on the original's map at SOME trigger spots — theory "a hidden
bit we never decoded" was nearly right): sub_48710's case 0xB
(:57386-57401, missed in the first read) — CLASS-11 triggers with
MODELS 9-12 blit UI sprite 83 (the X mark) center-anchored on both
maps; model 31 blits sprite 84; all other trigger models draw
NOTHING. The trigger model doubles as the advertise flag: Bullfrog
marks the intended flight path, not the secrets. Level 032's data
matches the player's sighting exactly — the only model-9 triggers
are dis-13's landing trigger and the dis-14 POOL-STALL trigger (the
"official path" waypoints); every jar-adjacent trigger is model 0 =
quiet. CORRECTIONS bundled with this find: (1) the earlier "begSprTab
83/84 = balloon icons" attribution was a misread of this very case —
83/84 are the trigger marker icons; (2) the old trigger-trace line
"models 5-16 = kill triggers" is imprecise — 032 plays (and our port
runs) models 9-12 as PROXIMITY variants (state = model; kill triggers
start at 13), now corroborated by the original advertising 9-12 as
fly-here marks. TODO with the HSPR/UI bake: draw the authentic X
icons for model-9-12/31 triggers on the faithful map (they are part
of the original's map language, unlike the sanctioned
map_trigger_areas overlay which reveals the QUIET ones too).

MANA DOT SEMANTICS (player, 2026-07-05): loose mana balls are colored
by their controlling player — unclaimed = orange, claimed = blinking
colored dots. Those are RUNTIME entities (live class 2 models 1/3 =
the blink branch of case 2, spawned by kills/possession, not level
records) — they arrive with mana mechanics, at which point the blink
phase and owner tint must be ported (the case-2 code draws the blink
half-phase near-black; the orange base likely = palette 28 rendered
under the map's palette handling — verify in the runtime port).
Player also notes MC2's map object drawing is DISTINCTLY IMPROVED
over MC1's — when the MC2 track lands, port its map overlay
separately rather than reusing MC1's (remc2 GameUI is the reference).

ENHANCED MAP MARKERS (design note, player 2026-07-05, to build in a
UI pass): the original's 1px dots are genuinely bad design, not a
quirk to preserve unconditionally — clusters are unreadable at
320x200, and at 640x480 the original doubled terrain pixels but NOT
the dots, leaving them nearly invisible — while the map is the
critical instrument for spotting vital pickups (red spell dots).
Faithful 1px stays the authentic baseline (and the DOSBox comparison
instrument); add an opt-in `enhancements` mode (config, like
smooth_shading) with resolution-independent markers — per-category
glyphs/icons sized in screen space, legible when clustered. Design
freely; this is explicitly sanctioned deviation territory.

- **Sound + Music**: LANDED 2026-07-06 — see "Audio (SOUND + MUSIC
  track)" below. (The old plan's assumptions died well: MC1 has no
  SOUND.DAT — its banks are SNDS<bank>-<q>.DAT; libADLMIDI wasn't
  needed — pure-Rust nuked-opl3 + our own HMP parser render the
  music at import.)
- **Text**: ETEXT.DAT (uncompressed, null-terminated strings; objective
  text = sequential blocks from string 48, briefings 23-47) → JSON.
  Localized variants (GTEXT/FTEXT/...) same format.

### Terrain water animation + sprite frame cycling (LANDED 2026-07-05)

The roadmap's long-standing assumption ("cycle the animated water/lava
TMAPS groups over terrain") was WRONG — traced in both decompiles
(agent report, session-local; key citations here):

- **Terrain surfaces never render from TMAPS and no pixels are ever
  swapped.** The animated ocean is a per-grid-corner SINE DISPLACEMENT
  + shade shimmer in the tile projector; the texture stays the static
  BLK/BLOCK32 atlas cell. Per corner:
  `sinprod = (sin[(y<<7 + turn<<S) & 0x7FF] >> 8) *
  (sin[(x<<7 + turn<<S) & 0x7FF] >> 8)` on the 2048-entry 16.16 sine
  table — amplitude 65536, wavelength 16 tiles (so the 256-tile torus
  carries exactly 16 periods and wraps seamlessly), phase `turn<<S` of
  2048 per turn. The gates differ per game:
  - MC1 (remc1 sub_main.cpp:33955, non-reflection variant :33354):
    S=6 (period 32 turns); DEEP-WATER corners only (angle bit 3, the
    generator's open-sea flag): `alt -= sinprod >> 10` (±64 alt units
    = ±1/4 tile swell) and shade `pnt5 += 8*sinprod` (±8 LUT rows of
    shimmer — exactly the generator's 28..36 flat-water dither range).
    Shallow/shore water is still.
  - MC2 (remc2 GameRenderOriginal.cpp:1054): S=5 (period 64 turns);
    EVERY water corner (type 0): `alt -= sinprod >> 13` (±1/32 tile
    ripple) + the same ±8-row shimmer, shimmer skipped where the
    corner's shade >= 56; angle bit 3 only flags the triangle 0x80 for
    the reflection pass. (MC2's `alt2`/`inverse_alt_8` reflection
    plane — fed by hmap2, the +0x40000 mystery plane, mystery now
    solved — wobbles multiplicatively; reflections are not ported, no
    reflection pass yet.)
  - The turn counter is a single global incremented once per logic
    turn (remc1 :48548, remc2 EventsFunctions.cpp:37555), which the
    original runs per rendered frame (variable rate on DOS hardware) —
    no canonical Hz exists; our sim tick (30 Hz) is the turn. Pacing
    to be eyeballed against DOSBox.
- **Lava/magma is STATIC terrain** in the 3D view — no wobble, no
  texture animation. Types 8/9's per-type flag (`byte_90168` →
  triangle flag 0x100000) selects a flat-fill raster mode with
  averaged corner shade (a LOD/fallback path), not an effect. Lava's
  in-game motion is gameplay (heightmap rise) + runtime effect
  sprites — future tracks. Per-type tables for later: MC1
  `byte_900C4/90168/9020C` = MC2 `x_BYTE_D41D8[328]` (textAtyp +
  reflection-eligibility halves).
- **Animated TMAPS entries are a sprite-only subsystem**: a per-frame
  driver (remc1 sub_590D0_595E0, remc2 sub_715B0 with an extra
  grouped-run branch) steps every animated entry's FLC stream one
  frame per unpaused frame, forward loop, all entries in lockstep.
  No palette cycling anywhere in either engine's water/lava path.

Our port (same session): `WaveMode` (Off/Mc1/Mc2, selected by game
tag) on `LevelView`; the wave runs in terrain.wgsl's vertex stage
(gates evaluated per corner exactly like the original, so shared mesh
vertices displace consistently), shimmer interpolated to the fragment
stage like the original's per-corner `pnt5_32` and added to the
per-tile shade snap (rounded, as the original's +128 bias truncation
does). Renderer `set_anim_turn(turn)` = the clock, fed from the sim
tick + render-interpolation alpha (wrapped mod 4096 for f32
precision); billboards with sprite flags bit 0 now draw frame
`turn % frames.len()` (the FLC cycle — mana balls, torches, etc.).
Headless: `--screenshot ... --anim-turn N` (default 0, deterministic).
Deliberate approximations: float `sin()` vs the integer table
(sub-quantum), shimmer not attenuated by the original's fog-band
scaling (our fog blends final color), per-tile shade snap retained
under the interpolated shimmer (the original gouraud-interpolates
pnt5 whole). Verified by screenshot pixel-diff: MC1 level 001 turn 0
vs 8 = 33% of pixels differ, vs 32 (one period) = 0.06% (FP noise,
±1 color step); MC2 level 001: 48% at quarter period, 0.12% at 64.
PLAYER-VALIDATED 2026-07-05: "works as intended … looks like the
original." Noted caveat for the enhancements backlog: the wave field
is a single homogeneous, unidirectional 16-tile sine grid — the
original's heavy fog never lets you see a large water body whole, so
the repeating moiré pattern it forms was invisible by design; our
lighter fog exposes it over open ocean. Sanctioned future OPT-IN
enhancement (config `enhancements` family, like smooth_shading):
de-homogenized water — e.g. phase-jittered/multi-octave wave field —
while the faithful grid stays the baseline. Related: fog
density/banding fidelity itself is still on the Phase 5 graphics
parity list.

## Sim core (after the flyer)

Substrate vs. game personality split (see "spiritual successors" note):
- Substrate: fixed-tick scheduler, entity model, terrain state +
  deformation ops, flight physics, collision, mana/castle mechanics
  shared by both games.
- Personalities: MC1 rules (kill-all + exit) vs MC2 rules (stage
  checkpoints, objective opcodes 0/1/2/3/5/7/8/9, conditional spawns,
  secret exits). Data already baked in `stages.json`.
- First gameplay slice — STATIC TERRAIN FEATURES: LANDED 2026-07-05
  for MC1/HW (see "Static terrain features" below; MC2's variant is a
  separate remc2 port, still open). Billboarded entities: LANDED
  2026-07-05 (see "Billboarded sprites").
- NEXT (agreed 2026-07-05): ~~(1) terrain animations~~ (LANDED same
  day — see "Terrain water animation"; the TMAPS-cycling premise was
  wrong, the mechanism is vertex displacement). (2) TRIGGERS & EVENTS
  — the "destructible world"
  slice, deliberately BEFORE spells-as-combat and before mob AI:
  places that cause earthquakes cause them, portals actually move the
  player, mobs/objects get spawned by triggers (stationary — no AI
  yet), spell collection (which is itself trigger/pickup driven).
  Rationale: get world mutation + event plumbing right on its own
  before layering behavior on top. Reference trace DONE (2026-07-05,
  agent report distilled below in "Triggers & events — remc1 runtime
  trace") — implementation is the next session's work.

### Triggers & events — LANDED 2026-07-05 (destructible-world slice, MC1/HW)

Implemented the same session as the trace below (which remains the
reference index). `mgc_sim::world::World` = the living level; shipped:

- **The engine is shared**: `features::Gen` was made lifetime-free
  (owns `Planes` + assets) and now serves both the load-time fixpoint
  pass (byte-identical — feature-heavy maps 005/025/034/039 verified
  against `baked/maps` before wiring) and the runtime tick, exactly
  like the original's single pool/dispatch. Runtime tick = one pass
  per sim turn (30 Hz tick = the game turn): global LCG draw, class-5
  bucket counts, per-event state dispatch, the per-tick `f63`
  increment (:52406) that makes the same crater handlers run
  "continuously" (digger radius growth gates on `f63 % 3` at runtime
  vs alloc-slot at load — same field, both behaviors emerge).
- **Dispositions**: level init fires disposition 0; THINGs with
  `dis_id != 0` are LATENT until a trigger fires their disposition
  (one-shot fires consume the records). This CORRECTS our previous
  over-population — across the mc1 campaign 13670 of ~21k creatures
  are trigger-gated and no longer spawn at load. Map dots/billboards
  now show the live set (map diffs vs baked/maps are dot-only;
  terrain pixels byte-stable). NOTE for the next DOSBox pass: levels
  are correctly sparser at start, and dis-0 class-10 things (2814
  across the campaign) now run their terrain events in the first
  runtime moments — authentic dynamic terrain from level start.
- **Class-11 trigger volumes**: AABB (radius `swi_sz` tiles, ±4096
  alt units), states 0-12 = balloon-proximity one-shot/repeating
  variants (probe throttled to every 8th tick via `f63 & 7`;
  repeating = 10-tick rearm that waits for the player to leave),
  states 13-30 = kill-triggers (class-5 model bucket empty 16
  consecutive ticks; they correctly cannot fire until combat exists —
  buckets stay populated), state 4 = collected-item trigger (stub
  until inventory). The balloon list is the player alone until AI
  wizards land.
- **Spawns**: `sub_37560_37920` post-init per class (id24 = `swi_id`
  = the disposition a trigger fires; extents from `swi_sz`; class-12
  `byte70 += swi_id` with the >=3 village-jar branch; class-10 m45
  building fixup). Drawables (classes 2/3/5/12) spawn as inert pool
  events (no AI — they stand, render, count in kill buckets) mirrored
  to the app for billboards/map dots; class-5 body segments render
  via the existing multipart path.
- **Renderer**: terrain heights moved from the vertex buffer into an
  R8Uint texture read in the vertex stage (with the water wave), so
  runtime mutation is `Renderer::update_terrain` = four 64 KB texture
  writes + a map recompose, driven by the world's dirty flags. The
  flyer's ground clamp follows the live height plane.
- Validated on level 005's authored cascade (integration test
  `world_level005.rs`, self-skips without baked data): fly to
  (99,115) → disposition 1 → the chain-terminating crater digs
  continuously near (95,108) + spawns a follow-up trigger → flying
  there fires disposition 2 → the 8-creature ambush; both triggers
  one-shot; 232-tick runs deterministic.
- **Deliberate deviations** (fidelity-pass fodder): player AABB is a
  point (original balloon has a small extent); creatures skip their
  real spawn handlers (no per-event AI state, no original pool-slot
  consumption for segment events — slot-order LCG dithers of
  runtime-spawned craters shift accordingly); class-12 PICKUP/mana
  transfer NOT ported (jars/mana spawn and render; the original's
  collection routes through owner blocks + class-9 carrier effects —
  that's the mana track); class-10 models 39/40 spatial lists and
  earthquake state 0x3A unported (quake spawning is spell-domain;
  ROADMAP flag: which class-10 MODEL maps to state 0x3A is still
  unresolved); damage broadcasts/sounds omitted as at load time.
- PORTALS: LANDED same day after all (the trace's "no portal entity
  mechanism" was WRONG — player caught level 032 starting void; its
  data disproved it). **Class 10 model 34 = the portal vortex**:
  spawn sub_3B300 (:47329) — state 36, sprite row 223 (the 8-view
  animated vortex family 220), 1-tile extents, spawns 640 alt units
  up then follows the ground; the THING post-init (:44024) writes the
  authored destination into +150/+152 from child/parent (level 032's
  entry portal at (11,253) → (5.5, 230.5), the maze entrance). Tick
  sub_26A60 (:29170): player AABB overlap AND a FACING GATE — heading
  within 170/2048 (~30°) of the bearing to the portal, i.e. you must
  fly INTO the vortex — then the player entity moves to the
  destination; actLife>0 = timed portal (authored ones carry 0 =
  permanent). Ported: World::portal_tick + pending-teleport handoff
  consumed by Simulation::step (altitude clamps above dest ground;
  velocity carries). PlayerPose now carries the 11-bit heading
  (engine angle = flyer yaw · 2048/τ — advance() confirms identical
  axis conventions). Portal renders as a billboard ((10,34) → type
  223 in mc1_entities). Integration test world_level032.rs: portal
  spawns at init, facing-away doesn't fire, flying in teleports to
  (5.5, 230.5), never expires. Level 032's remaining "void" is
  correct: its population is authored behind the disposition chain
  (triggers 1→2→3→18/4… as breadcrumbs through the maze sections).
  PLAYER-VALIDATED in play (2026-07-05): portal works; flying over
  the maze's jars trips the adjacent trigger → the bee pack spawns —
  the chain runs end to end. Fine-tune later: portal ENTRY semantics
  feel slightly off (one suspect FIXED 2026-07-05 with the mobs slice:
  the player now carries sprite-44 extents in the AABB test — re-check
  in play; remaining suspect: the facing cone); further progression
  waits on combat (expected); jars inert (mana track). Level 032 =
  the standing event-chain testbed.

### Mobs & movement — LANDED 2026-07-05 (spawn + movement slice, MC1/HW)

Implemented same day from three fresh agent traces (the banked trace
below stays as the reference index, with corrections). What shipped
(`mgc_sim::mobs`, ~700 lines on the existing event pool):

- **Real spawn handlers for classes 2/3/5** replace the inert spawns:
  per-model constants (life/speeds/accel/behavior row/sprite type/
  extents/mana fields) ported verbatim from str_254D48/str_254B84/
  str_255478 dispatch; multipart worms (m0/m3 16 segments, m6 kraken
  2) spawn as byte-copies of the head at state 120, chained via
  +52/+54. All spawn randomness rides the PER-EVENT LCG (+4, seeded
  `slot + global` without advancing global) — tree variant/jitter and
  creature facing draws are now byte-faithful, retiring the app's
  per-slot approximation. `unk_98F38` behavior rows extracted to
  `mc1_behavior.rs` (extractor extended; 31 rows).
- **Movement core sub_196E0**: altitude clamp toward the row band
  (sub_42000, quarter-step inside), polar step on the 16.16 tables
  (sub_41EC0), the creature wall rule (terrain-capability mask
  sub_11810 vs row v_20 + roughness sub_19650 vs v_16; same-tile steps
  free), retry headings, all-blocked → life = -1; commit via
  move_relink + rate-limited turn (sub_422A0, cap v_2).
- **The six state primitives** (IDLE/WANDER/CHASE/PACK/DEATH/CORPSE,
  6·model family blocks) + the model-0 flyer bob (z += f26; f26 -= 5;
  re-arm 150 below ground+256), the model-15 grid-walker (sub_20480),
  segment follow (sub_19550: awake segments hang at +56 units behind
  the leader along the exact 3D bearing), and the **awake system**
  (sub_54F80 pre-pass: +58 countdown mirrored into segments, re-armed
  16/18 while the player is within 24 tiles; asleep creatures skip
  scans and collapse their segments every 4th tick).
- **Live poses**: `World::live_poses()` exports continuous pose
  (8.8 position, altitude, real yaw, sprite type + anim frame) straight
  from the pool; the app's billboard/map-dot layers consume it
  (billboards refresh per tick, map recompose throttled to every 8th
  tick). `Billboard.frame` + renderer support for the 2..=16 animation
  draw types landed with it. Segment entities render but stay out of
  entity lists/map dots (state-120 exclusion) — the invented
  trailing-line resting pose is gone, movement owns segment positions.
- **Player extents fixed** (the portal-entry suspect): the AABB test
  sub_118C0 sums BOTH entities' extents and centers each z by its
  half-height (+78); the player carpet now carries sprite 44's stats
  halves (119/119/100). Level-005's second trigger correspondingly
  fires from ~0.5 tiles farther out (test choreography adjusted).
- **THING post-init corrected** (dormant bug): the original gives
  id24/extents/refill ONLY to class 11 (and class-10 m4); our old
  `c <= 11` arm was clobbering creature extents with swi_sz. Also
  (10,45) gets ONLY the building fixup, and the class-12 jar branch
  writes +86 = 280 directly.
- CORRECTIONS to the banked trace (below), from the fresh traces:
  spawn states m9 → **54** and m11 → **66** (not 55/67 — the 6n+1
  pattern breaks twice); the movement retry yaw is **±341 (~60°)**,
  not ±85 (the +85 in the source is 341's low byte); behavior-row
  columns v_4/v_6/v_8/v_22 are DEAD (never read); CHASE's range-drop
  compares v_28 UNSQUARED against true 3D distance while scans use
  v_28² vs 2D dist² (asymmetric, verbatim); the water rule sub_42090
  is NOT part of creature movement (sole caller = the corpse-fall
  handler); IDLE neither moves nor animates.
- Deliberate deviations (all AI/combat/mana-track fodder): attack
  calls are no-ops (chasers close in and shadow the player); damage
  mailboxes (+90/+94) unread; custom family behaviors beyond movement
  stubbed stationary (m4 disguise, m5 growth/mana hunt, m9 buried
  eat/emerge detail, m11 teleport caster, m12-14 house life, m16 house
  destroyer — their movement states use the standard primitives);
  m4/m8 aggro is possession-gated in the original → no aggro yet;
  corpses despawn without the mana-ball/bones drop; pack/separation
  scans run in pool order (original: per-tick rebuilt linked lists —
  head-insertion order approximated); m15's 4-entry direction-vote
  weight table lives at a code/data alias (`*(_DWORD*)sub_1FF40`) the
  decompile can't express — uniform weights stand in (draw count
  identical, streams align; extract from the retail EXE someday).
- Tests: worm segments trail the head; an awake creature wanders; a
  villager on a 1-tile island is CONTAINED by water (the wall rule);
  level-005 cascade + determinism (now includes pose snapshots) and
  level-032 portal still green.

First playtest fixes (player on level 032, same day):
- **Runaway worm/bee speed** (packs gradually accelerating without
  bound). Two compounding causes, both fixed: (1) WANDER's scans are
  entirely awake-gated in the original (:21514) — wizard scan first,
  pack scan only as its fallback; the agent trace read it as
  "awake→wizard ELSE pack", so every distant asleep crowd packed up.
  (2) The pack catch-up at :21814 reads `+126 += leader.+130` in
  remc1's source — but the decompiler's RAW line preserved in a
  comment above it (`v10 = v3x->+130 + v3x->+126`) shows the original
  computes member speed = LEADER's speed + accel, a bounded "fly
  slightly faster than the leader"; the += is a remc1 maintainer
  MIS-FIX and porting it verbatim is the runaway. DECOMPILE-TRUST
  LESSON: when a ported formula misbehaves, check the commented-out
  raw decompiler lines against the maintainer's hand-fix. Also: PACK's
  join-chase and default cases RETURN before separation+accel.
  Regression test: an 8-bee crowd asleep for 3000 ticks stays under a
  tile/tick.
- **Burrowers stuck as the blue flame** (m9, 123 on level 032): the
  spawn state 54 materialize sequence is now ported (sub_1CFF0: flame
  form 220 → 16-frame transform anim 237 stepping every other tick →
  type-201 lurking mound at state 55) plus state 55's visible slice
  (sub_1D060: surface timer re-armed while the player is near; bury
  as type 245 when it expires; burrow-walk + jitter; CASTLE hunt —
  nearest class-3 m2 within extent+v_28 → chase, popping up as the
  warrior form 202 via sub_1DCD0). Buried mode's villager hunting
  (sub_1D6D0) stays an AI-track stub (buried mounds sit still).
  Regression test asserts the 220→237→201 sequence.
- **Portal drawn floating at +640** (level 032 entry portal; the
  teleport region was correctly at ground). The pose-consumer switch
  exposed it: the sim's portal re-grounds on its first tick, but the
  per-tick `entities_dirty` only fired when creatures existed — and
  032's population is disposition-gated (zero at start), so the app
  kept the load-time pose snapshot at the spawn altitude. Fixed:
  `portal_tick` flags the entity set dirty whenever the portal's
  altitude actually changes (first-tick drop + terrain re-dug under a
  portal later). Regression assertion added to world_level032.
- NOT bugs: m7 alternating skeleton (type 85, odd ordinals) / type-199
  look (even) is authentic dual-variant spawning (different life too:
  4000 vs 2000); chasers stacking exactly on the player = the no-op
  attack call (combat track) — in the original you'd be dead.

### Combat & the dev fireball — LANDED 2026-07-05 (combat slice, MC1/HW)

Implemented same day from the two banked traces below (+ a follow-up
correction pass). What shipped (`mgc_sim::combat`, ~900 lines, plus
inbox/thunk/death integration in `mgc_sim::mobs` and the cast/input
plumbing in `world`/`lib`/mgc-app):

- **Damage mailboxes**: the six-channel {u32 amount, u16 source} array
  (+90..+124) on every event plus an out-of-pool inbox for the player;
  the shared write protocol (accumulate while pending, overwrite
  stale); area writers (sub_120B0 with owner/damageable/vulnerability/
  filter gates, ch0's class-3-m2 exclusion, sub_124F0's building/10)
  and the direct write — with the area protocol, NOT sub_12B50's
  inverted transcription (maintainer-suspect).
- **The creature inbox block** opens creature_tick for roles 0-3
  exactly as the original opens every state handler: awake-gated
  damage apply, segment-chain weakest-life inheritance (shooting the
  tail kills the worm), attacker/killer latches, per-role dispatch
  (IDLE/WANDER aggro→CHASE on class-3 attackers only, CHASE retarget,
  PACK leader+member retarget, m8/12/13/14 never hit-aggro). Segments
  take their own damage in state 120.
- **Attack thunks** for every fighting model: m0/m3 fireballs (500),
  m1/m2 melee ≤1024 (m2 recoil+cooldown), m4/m10 bolts (250), m5's
  mana-scaled multishot (1 LCG; fireball spread / zigzags / the
  8000-payload blast bolt), m6's 5-bolt spit bursts, m7's slow bolt
  (780), m8's 4000 zigzag with narrowed filter, m9's bolt (600/400,
  aimed at the TARGET — the transcription self-aim is a decompile
  bug), m11's wizard-seeker (3000 → ch3 mana steal), m15's default-
  damage bolt (100 — NewEvent's +44), m16's 15-fireball facing-gated
  bursts (3000, strong homing row [2], mana 60000).
- **Class-9 projectiles**: m0 fireball (one-time aim assist within a
  ±0x71 cone ≤5120 units, then row-capped homing; deflection LCG
  ported, nothing sets the rebound bit yet), m3 trail bolt (decorative
  no-damage fire trail), m8 wizard-only detonator, m9 zigzag
  (simplified: one visual segment + 2 draws per tick), m13 straight
  bolt (direct one-shot ch0 broadcast, no explosion event), m14 =
  m13-interim (its state 15 is past remc1's truncated table).
- **Class-10 combat effects**: m0 fire (state 0: the REAL fireball
  damage — one 400 ch0 broadcast; burn conversions 26/10/11 →
  0x14/15/16; the 1-LCG scorch crater on flat low dry ground; flicker
  draw), m1 fire-spreader/corpse flame (ring at +26, 3 draws/cell),
  m5 water splash, m17 growing blast ring (+44/maxLife per tick +
  fire rings), m23 hit-flash (one-shot ch0), m25 mana-steal flash
  (one-shot ch3), m39 mana ball state 41 (launch-arc physics,
  gravity/bounce/friction, ch1 claim intake, overlap merge).
  spawn_creator now delegates models 0/1/5/39 to the real inits.
- **DEATH/CORPSE completed**: segment chain → corpses with killer
  propagation, the human-player kill counter (+359 semantics, models
  9/12-15 excluded), m13/m14 castle-absorb silent despawn; CORPSE on
  the 8th-phase tick drops the mana ball (1 unused corpse draw + 2
  ball launch draws, +140/+144 transfer, sprite 52 unowned) and the
  death-flame puff, then despawns. Every worm segment drops.
- **The dev repeat-fireball** (player side): PlayerCommand{fire} in
  the tick input; hold = re-arm every tick = one fireball per game
  tick (spell 23's true gate — mana-limited in the original, infinite
  here since sub_55DD0/sub_55E80 are the bypassed cheat); muzzle 256
  units left at yaw-512 with the terrain revert; launch z + carpet
  half-height; heading/pitch/speed from the pose (PlayerPose gained
  pitch + speed; the app converts flyer pitch sign and passes tiles/
  tick). App: hold left mouse (while grabbed) fires.
- **Invincible player** = the original's spawn grace made permanent
  (:55367-71): all six channels discarded every tick, ch0 totaled
  (`World::player_damage_taken()`); stats via `combat_stats()`.
- Deliberate deviations (flagged in combat.rs / the banked trace):
  aim-assist scoring = angular miss (Δyaw²+Δpitch²) pending the exact
  sub_54A90 metric; m9 zigzag path simplified; m14 interim flight;
  ring-cell ordering synthesized (LCG parity vs original unproven);
  m2 lunge-restore skipped; m4 mimic/possession, m6 camera buffet,
  m11 idle burn/teleport cycles, m12 castle building, m13/14 castle
  feeding still stubbed (AI track); sounds omitted; ball size
  thresholds (dword_900A4) + owner palettes pending extraction; map
  dots skip projectiles/effects.
- Tests (all green, 40 workspace-wide): fireball kill → mana-ball
  drop + kill credit; 3-shot aggro → invincible player mauled (total
  recorded); worm chain dies from the head, segments drop; scripted-
  fire determinism; level-005 cascade/determinism + level-032 portal
  unchanged.
- NOTE for playtesting: the crater-rim wall-death is real — chasers
  crossing a fresh crater rim can die to the all-blocked rule (the
  first aggro test caught it). Authentic behavior, worth watching.
Second playtest fixes (player, level 032, same day):
- **"Flying human archers with crow mechanics"** = the m4 MIMICS.
  Probe (examples/dbg032fly.rs — keep: per-model altitude/spread
  triage): m7 archers grounded ✓, m1 crows flying 2-7.5 tiles ✓
  (authentic — row 13 floor ground+512), but c5m4 at up to 28.6
  tiles, dispersed map-wide, sprite type 0 (a human look), firing
  dart bolts. Cause: m4's custom ambusher handlers (disguise, speed
  0) were stubbed onto the GENERIC wander/pack primitives → they
  roamed; row 0's band (1792) + feather-fall (v_14 = -4/tick) means
  walking off a canyon rim leaves them hanging in the sky for ~2000
  ticks. Fix: m4 fully stationary (idle/wander/pack no-ops) with a
  stand-and-shoot CHASE (face target, dart thunk every v_26, no
  movement) — interim until the real m4 family trace. LESSON: a
  "harmless" movement stub can turn a custom-handler family into a
  wrong-looking hybrid; when a family's handlers are custom in the
  original, stub STATIONARY, not generic.
- **Authored c10m17 fire traps got no init**: level 032 authors
  blast-ring records behind dispositions (6 in the dis-7 obelisk
  room, 5 dis-13, 8 dis-16, 6 dis-4); spawn_creator's generic arm
  gave them NewEvent defaults (life 300 → a 300-tick zero-damage
  firestorm digging scorch holes). spawn_creator now delegates
  models 17/23/25 to the real combat inits — they erupt as the
  authentic 10-tick blast (and dis-4's traps correctly cook a few
  of its own mimics).
- Player-validated same session: fireball, auto-aim and cadence
  "very much akin to the original… couldn't tell the difference";
  full 032 run — killing everything before the dis-14 trigger
  avoids the pool starvation (chain completes), dragon + crab kill
  trigger fires dis 19 = the authored no-op, confirming the level's
  endgame is data-buggy, not us. Game speed (tick rate vs the
  original's frame-locked turns) = still the open Phase-5 question.
Third playtest round (player, same day): crab mana-eating + ball rest
fix:
- **m5 crab family LANDED** (player: "crabs consume mana balls, grow,
  use higher spells — what makes maze crabs dangerous"): real WANDER
  sub_1BF60 (:22775 — custom handler, NO yaw-jitter draws; every
  v_26: wizard scan → CHASE, else steer to the targeted ball
  [within maxSpeed<<7 → EAT, +26=15], else acquire nearest ball +
  lay a class-10 m52 egg at 500-over-max mana [1 LCG,
  +26 = 10·(rand%10)+100, cost 500]); EAT state 0x21 sub_1C170
  (:22986 — think period +26 15→3 inside 20·maxSpeed, absorb within
  5·maxSpeed: mana += ball +140, destroy, back to wander, GROW);
  GROW sub_38820 (:44943): size = clamp(mana/(maxmana>>3), 0, 7) →
  sprite 185+size, size-up adds +5000 max life; regen tail
  maxlife>>7/tick in wander+chase. Growth auto-upgrades the already-
  scaled multishot (7·mana/maxmana tier). NOTE from decompile: m5's
  scans are NOT awake-gated (unlike the generic wander) — ported
  verbatim. Level-032 probe: crabs hit the authored 49-ball grid
  immediately, one at size 2 within the probe window — dis 4's grid
  is authored crab food. DEVIATION: the m52 egg despawns unhatch ed
  (spawn+draw+cost kept for stream shape; hatch handler = AI track).
  Test crab_eats_the_mana_grid_and_grows green.
- **Mana-ball perpetual jiggle fixed** (player: balls "oscillating…
  as if attracted by tile centers" — the grid itself is AUTHENTIC,
  dis 4 authors 49 balls at tile centers): ball_tick applied gravity
  at rest, cycling z between ground and ground-16 every 2 ticks.
  Now gravity only while airborne/launched; settled balls are fully
  static (and skip the relink). The one-hop on spawn stays (+46 =
  128 launch in the original's init).
- **Ball size classes LANDED** (player: "dragon usually yields a
  massive ball… here barely noticeable" — amounts were right, every
  ball wore the hardcoded type 52): sub_274D0 (:29574) ported —
  thresholds dword_900A4 = {256, 512, 1024, 2048, 4096, 9192, 18384}
  (:2215), sprite = family + class (8 classes, types 52..59: 61×50
  up to the 492×400 boulder), nonzero classes halve extents
  (sub_370E0 :43781), re-derived EVERY tick (:29569 — merged balls
  visibly grow). Owner palette families (105+8·slot) stay the mana-
  collection track. Dragon drop (50000) = the class-7 boulder.
- **Monster health bars** (unfaithful debug enhancement, same day):
  classic red-on-black bars above class-5 chain heads —
  `LivePose.life_frac` (sim) → `entities::health_bars_from_poses` →
  a solid-color instanced bar pass (mgc-render bar.wgsl, billboard
  camera basis, depth-tested, no fog). Opt-in via
  `enhancements.health_bars` / `--health-bars` / the H key (delete
  mgcarpet.json.defaults to regenerate it with the new option).
  Segments carry no bar — their damage propagates to the head's bar
  through the inbox chain walk within a tick.

Follow-up-trace corrections to the banked combat spec below (from the
same-day class-9/class-10 pass — the durable versions):
- NewEvent defaults +44 = 100 and +68 = 10 (:43873/:43879) — "unset"
  projectile damage is 100, unset explosion = fire.
- Class-9 model→flight: m12 (state 12, sub_53DC0) is the one that
  force-explodes into class-10 m38 (storm cloud, state 40, rains
  bolt-pairs); m13 (state 13, sub_54180) is the straight bolt with
  the direct ch0 write; m14 spawns at state 15 which is PAST remc1's
  transcribed 14-entry table — m7's real bolt flight needs the
  retail binary's table. Class-9 inits: all speed 384/life 21 except
  m9 (life 9), m12 (5), m13 (13), m14 (speed 128, life 32); rows
  [5]/[1]/[4]/[4]/[1]/none/none; sprites 42/76/214/216/216/195/196.
- CONFIRMED: sub_52B30 copies only +24/+30/+32 to the explosion —
  fireball damage = the fire's 400 for both fireballs; the spell-row
  +44 (125/50) is dead weight in this transcription.
- m8's bolt explodes ONLY on class-3 model ≤1 victims (everything
  else = silent despawn), and its +69=25 explosion is a ch3 MANA
  STEAL, not damage — m11 drains 3000 mana, wizards only.
- Class-10 effect constants: m0 fire state 0 (life 8, +44 400,
  sprite 7, 128³), m1 spreader state 1 (life 1, sprite 41, ring at
  +26; damage-suppression = +18 bit 0 on the FIRE, inherited from
  the seeder's 0x10000 bit), m5 splash state 5 (life 8, sprite 244,
  grounded), m6 ground wave state 6 = sub_252D0 (per-tick 124F0,
  +44 50, life 240, sprite 228), m17 blast state 17 (life 10, +44
  3000 default, invisible, +44/maxLife per tick), m23 flash state 23
  (life 8, +44 25 default, sprite 7, 200³), m25 steal state 25
  (life 8, +44 2000 default, sprite 283, 512³), m38 storm state 40
  (life 32, sprite 272, 2 bolts/tick + 1 LCG heading draw).

### Combat polish checklist (player, 2026-07-05 — LANDED same day,
### fourth session)

All four items from the third playtest landed (details inline below;
41 workspace tests green). Session-wide notes: knockback-from-damage
resolved as authentically absent under the invincibility grace (see
item 1); the ch4 grip is a wizard SPELL, parked for the spell track;
flying mobs legitimately cross walls (see item 2's FINDING); NEXT
candidates: playtest the beam/buffet/villages on level 032 + a real
village level (008?), then mob AI customs (possession/mimic-restore/
m11 idle cycles/castles), Phase-5 flight, mana collection.

1. ~~**Kraken tractor beam**~~ LANDED (this session) — with a
   HEADLINE CORRECTION from the fresh ch4 trace: the kraken's escape
   restriction is NOT the ch4 mailbox at all. Two mechanisms were
   conflated:
   - **ch4 "grip" = a WIZARD SPELL** (spell entry 0x21/triple 11,
     Duel-style): its class-9 m7 bolt spawns a class-10 m26 gripper
     ONLY on wizard hits (:63188); the ch4 consumer (:55663-82)
     writes tether data into the CASTER's Type_160 (+314 anchor,
     +316 timer→1000, +318 ring 1024-3072) and the sole reader is
     the human move (:55228-48): radial spring (dist−lock)/8 capped
     ±120 + forced yaw ≤130/tick, 5120 break. It ring-locks the
     CASTER to the victim ("you can't flee the fight you started")
     — spell-domain, SKIPPED until the spell track (noted: only ever
     binds the human as caster; irrelevant to kraken encounters).
   - **The kraken drag = the BUFFET**: the m6 attack cluster
     (:23215-31) writes the VICTIM's Type_160 v_22/v_24 DIRECTLY
     (not a mailbox — spawn grace does NOT shield it; even our
     invincible dev player is dragged, authentically): every ON tick
     of the 41-on/91-off +26 duty cycle, v_24 = bearing victim→
     kraken, v_22 = 80; the human move applies v_22 units along
     v_24 per tick (clamp 128), decays 4/tick, snaps <4 (:55204-18).
     PORTED: `Gen::player_knock` + `World::take_knock_step` +
     `Simulation::step` applies the displacement INSIDE the move,
     before the wall gate (the drag cannot pull you through walls);
     mgc-app adds the camera pitch kick (−v_22/8 engine units,
     :52433-37). Deviation: the kraken writes during world.tick →
     the pull lands one flyer-tick later (invisible at 41-tick
     phases). Sound 42 omitted (audio track).
   - **Knockback from damage** (the session's note-and-skip item):
     the generic hit kick writes the SAME v_22/v_24 fields via the
     ch0 inbox (v_22 = clamp(dmg/10,0,80) AWAY from the attacker,
     :55710-26) — but grace memsets the mailboxes first (:55367-71),
     so its absence for the invincible player is AUTHENTIC, and the
     ported knock machinery is exactly what it will feed through
     when player mortality lands. Nothing further to do now. (v_26
     is written by both writers and read by NOTHING — dead field.)
2. ~~**Solid wall impassable for the PLAYER**~~ LANDED (this session;
   cannot be crossed at any altitude; the burn-to-breach proxy
   conversion already works in our build — player-verified). The
   human commit gate sub_45410 (:55065) is ported into the flyer
   step: `Gen::player_wall_gate` (mobs.rs) — proposed tile with
   capability mask == 0x100 (exactly = type-8 wall) rejects the
   move, retried along the two adjacent cardinals (floor/ceil
   multiple of 512) stepped FROM THE CURRENT position scaled by
   angular proximity `dist·(512-Δ)>>9`; both blocked → whole move
   discarded. Emergent authentic detail: a blocked approach from
   afar re-lands SHORT of the wall via its own cardinal (approach
   softening); head-on cardinal hits void in place (zero-length
   ceil slide). `World::player_wall_gate` wraps it in tile units;
   `Simulation::step` applies it between integration and the ground
   clamp (velocity is kept — pushing against the wall decays via
   drag). The routine's unconditional z-floor (ground + row v_12 =
   128) stays with the flyer's own clamp until Phase 5. Tests:
   slide/void/corner-discard/pass-through unit cases + a 600-tick
   sim flight into a wall at altitude 30 (never crosses).
   FINDING re "mobs too, incl. flyers": in the original, FLYING
   creatures CAN cross walls — every flying behavior row's v_20
   terrain mask is 0xffffffff (wall bit allowed); only the human
   row 7 (0xfffffeff) and the ground rows (0xfff080fe) clear bit
   0x100. Our creature movement already reproduces this. So "walls
   block flying mobs" is NOT an original rule — flag for the player
   before anyone "fixes" it.
3. ~~**Human buildings: destruction + settlers building**~~ LANDED
   (this session — the full village slice, from a fresh trace with
   several briefing corrections):
   - **Class-2 m0 is the TREE, not a building** — the sub_124F0 /10
     discount (:17465) is a TREE rule (keeps area spells from
     vaporizing forests). Tree life = 300 (the spawn's rolled
     2500-7500 goes into +12 and RefillLife clobbers it — verified
     against RefillLife :43701; the trace agent misread this).
     PORTED tree burn chain (states 0/1/2, :57662-90): a lethal hit
     sparks a class-10 m6 STANDING FIRE (sub_252D0 :28199, also
     ported: flame-size sprite walk up 7/down 12, ground+f46 ride,
     50 ch0 per tick through the /10 writer — forests chain-burn),
     burn timer rand%60+130, then the charred sprite (83→226,
     84→227). Deviation: the shrink-phase (10,13) smoke puffs are
     skipped (draw kept for stream parity).
   - **Village houses = class-10 m45**, full ch0 damage (2000 life
     post-construction = 5 fireballs — "hard" confirmed). PORTED
     state 52 (sub_28DC0): damage intake (sub_29640, u32), each
     non-lethal hit pops ONE militiaman out while occupants > 2 and
     sets the attacker wizard's +528 aggro; every 40 ticks f140 =
     occupants<<8 + the full-house 1/16 emission (sub_28D10 mix:
     2/12 militia, 2/12 migrant, 5/12 villager, 3/12 settler).
     PORTED state 53 collapse (sub_28FE0): footprint walk, occupant
     EVACUATION (last one out = a settler m12; ≥4 remaining draw
     the emission mix; else militia — village defense IS the
     evacuation, there is no separate defender spawn), rubble
     (protection cleared, angle nibble 1, tower cells −12/−16,
     raised cells LCG-knocked), region retexture. No mana spill.
     REGRESSION fixed on the way: runtime state-52 buildings used
     to die in the load-loop's default self-kill arm — villages now
     persist as damageable entities. Deviations: ch1 possession
     re-owner skipped (spell track); castle-crush direct kill wired
     (life<0 → 53) but castle expansion itself is the castle track.
   - **The +528 "wanted" timer** (the archer-hostility gate) →
     `Gen::player_aggro`: set 200 by building hits, villager-family
     hits AND kills (m8/12/13/14 inbox marks, m4+villager death
     latches), −1/tick; `World::player_aggro()` exposes it.
   - **m4 = the VILLAGE MILITIA** (the "mimic" reading was half the
     story — sprite 0 is the unarmed human look). PORTED real
     handlers: IDLE (sub_1B5D0) with the acquisition ladder —
     wizard ONLY when +528 ≠ 0, burrowers (m9) unconditionally,
     otherwise walk back into a house within 0x1000 (the death slot
     +26=1 silent-absorb, occupants++); CHASE (sub_1BB20) arms the
     206-or-1 sprite (1 LCG, 11/20), speed 0, target-copied filter,
     stand-and-shoot darts + aggro refresh; break = the state-24
     disarm slot. Idle pair-up pack (27) still stubbed. NOTE for
     level 032: maze "mimics" now sit passive until provoked (hit-
     aggro or wanted timer) — matches the original's gate; watch it
     in play.
   - **Settlers DO build** (confirmed): the full m12 chain PORTED —
     73 wander (+26 countdown) → 75 seek (nearest m45; NO house on
     the map = wander forever, villages only grow around existing
     buildings) → 74 approach (patience countdown, 0xA00 arrival)
     → 72 BUILD (sub_1EA40 verbatim: one attempt/tick, attempt # =
     side E/W/S/N, 3 settler-LCG draws — type (rand&7)+25, gap,
     jitter; sub_1E9B0 inflated halves (dim<<8)/2+768; water abort;
     sub_1E920 4-corner flatness 15/16; overlap scan vs every house
     AND castle) — success spawns the (10,45) site at state 51 (the
     SAME 30-tick construction the features pass runs) and the
     settler retires to state 79: model stays 12, dispatch is
     state-based, the original's own trick. m13/m14 feeders PORTED
     (steer in beyond 0x800, drop full houses, walk in the door;
     m14 only migrates to villages beyond 0xE100000 dist², wrapping
     32-bit math verbatim).
   - Castle-guard m15 arrows (state 92, class-9 m13) were already
     in; the castle's guard-slot respawn loop (:56425-62) = castle
     track. Tests: house persists → militia pop under fire → aggro
     flags → collapse evacuates (settler last) + rubble nibble;
     tree chars, fire burns out; a settler builds a second house
     and settles (1200-tick integration).
4. ~~**Kraken lightning looks like a projectile, should be a
   FLASH**~~ LANDED (this session) — sub_535E0 (:63272) fully
   re-traced (cross-checked against remc2's byte-identical
   sub_66750) and ported as the real ONE-TICK BEAM: the flight
   walks to termination inside its single tick (384-unit steps,
   life 9 counts STEPS; victim snap to exact position — no +78; NO
   water splash, NO deflection), then lays 8·steps+1 state-14
   segment entities (sprite 216, real pool slots — slot pressure is
   authentic; slot-order life 0/−1 under a pre-decrement kill test
   = exactly one rendered frame each) along a ±1 random walk:
   amplitude pinches clamp(remaining/2,0,8), draws CONDITIONAL, and
   the CONFIRMED-in-both-decompiles quirk that only the FIRST walk
   (v32) displaces — in BOTH z and the yaw+0x200 perpendicular
   (a diagonal zigzag plane, ±96 units max) — while the second
   walk's draws only advance the RNG. Endpoint always explodes at
   the SEGMENT-WALK end (m23 flash carries the damage; shielded
   class-3 victims with mana ≥ +140/4 quarter it — 800→200, no
   drain/deflect). Thunk corrections landed with it: m6 AND m8
   (:23261-64, :22156-60) copy the TARGET's OWN +66/+67 fields
   (player → −1/−1 hit-anything), both set beam row [6] (inert in
   flight — the beam never homes; aim assist only for unowned
   beams). Deviation: the explosion's +146 stamps hit-or-0 where
   the original writes garbage on a miss (remc2 guards the same
   spot). m5's tier-1/2 zigzags and m8's 4000 bolt inherit the beam
   automatically. Kraken burst driver unchanged (5 beams per burst,
   every v_26=40 in range 4096).

### Player carpet — remc1 trace (Phase 5 groundwork, 2026-07-05)

Fresh full trace (supersedes the "carpet wall rule is emergent"
summary below — that story is HALF right). **The roadmap's premise was
wrong: class 3 model 0 = the HUMAN player** (spawn sub_37820 :44180,
tick sub_45C90 :55323, move sub_455D0 :55110, behavior row 7);
**model 1 = the AI wizard** (spawn sub_378A0, tick sub_13170 :17842,
move sub_14EB0 :18781, row 8). Proof: respawn sub_44D30 passes
`model = is_computer_player` (:54852). Both are now spawned as
entities by mgc_sim::mobs (no tick). Key facts for the flight port:

- **Human movement (sub_455D0)**: yaw += filtered roll >> 3; actSpeed
  chases target ±16/tick (range ±80 units/tick); pitch angle =
  filtered input, but CLIMB authority scales with distance below the
  soft ceiling ground+1024 (`(pitch · -(z-ground-1024 clamped ±256))
  >> 8` — full below, zero at, INVERTED above; dives pass raw); at
  speed 0 above the ceiling: sink 8/tick; strafe = second polar step
  at yaw+512, ±80, decay 4/tick on release; then the COMMIT GATE
  sub_45410 (:55065): **type-8 (wall) tiles are horizontally
  impassable for the human** — blocked moves retry along the two
  adjacent cardinals scaled by angular proximity (wall SLIDING), both
  solid → whole move discarded; finally z hard-floored to ground+128.
  NO hard ceiling. Walls block doubly: the horizontal type-8 gate AND
  the +48-height floor ride. Row 7's mask has bit 0x100 (wall)
  cleared; row 8 (AI) allows everything.
- **AI movement (sub_14EB0)** commits UNCONDITIONALLY (the banked
  emergent-clamp story applies to the AI only): altitude approach
  sub_42000 (drift -4/tick above band, -1 inside), forward + strafe
  steps at pitch 0, accel ±16 toward Type_160 target speed, turn
  `angdiff/(((255-skill)>>4)+8)` clamped [v_4,v_2]=[5,256] toward +34.
  AI band = hard clamp [ground+128, ground+768] each tick (sub_132B0
  :18035).
- **Input pipeline** (for mapping modern input): mouse POSITION from
  screen center → roll/pitch ±127 (:19889); per tick the pair
  low-passes `s += (2·input - s)/4`; yaw rate = s>>3 (max ~5.45°/tick);
  arrows Up/Down = target speed ±16 step, Left/Right = strafe. Spawn
  z = ground + 256; spawn grace u16_331 = 100 ticks (damage discarded).
  Human hover holds altitude exactly (polar step no-ops at speed 0).
- Human tick order (:55323): owned-creature census, balloon-touch
  mana transfer, input apply, damage inbox (or grace), MOVE, mana/life
  regen (combat vs idle divisors), death → state 2 (fall -2/tick²,
  land at ground+128, state 3 respawn wait `32·((255-skill)>>3)+32`).
  Only PRNG in flight: every 64th tick, flap-sound roll (1 draw).
- The dead-wizard fall is where sub_42090's water rule lives (corpse
  sinks to -768 over water-angle tiles and despawns).

### Mobs & movement — remc1 trace (banked reference, 2026-07-05)

Original agent trace (kept as the reference index; see the LANDED
section above for corrections). Key facts, all citations remc1
sub_main.cpp:

- **Tables**: class-5 tick table str_254DCC (:4687, 0x79 states),
  spawn table str_255478 (:4421, models 0..16 ONLY — 17..19 hit the
  null terminator, no spawn). Class 3: str_254ADC/:4668,
  str_254B84/:4367; ~~the player carpet = class 3 MODEL 1 (spawn
  sub_378A0 :44201, tick sub_13170 :17842)~~ WRONG — model 1 is the
  AI wizard; the human is model 0 (see "Player carpet" above). Per-model behavior rows
  `unk_98F38[]` (Type_156, Basic.h:333 — EXTRACTION TARGET): confirmed
  columns v_2/v_4 turn caps, v_10/v_12 altitude band, v_14 alt step,
  v_16/v_18 slope thresholds, v_20 terrain-permission mask, v_26 tick
  period, v_28 aggro r², v_30 facing cone; v_6/v_8/v_22 unconfirmed.
- **Spawn handlers** (full per-model constant table in the agent
  report, session-local; re-derive from the spawners :44570-:45640):
  common shape = NewEvent + state/model + maxSpeed(+128)/accel(+130)/
  actSpeed(+126)/life(+8) + mana=life>>1 (+140, sub_36F90 :43741) +
  behavior-row ptr (+156) + LCG facing draw (`+34=+30=(x&0x7FF)-1`) +
  place + RefillLife + sprite/extents via sub_36FA0 (:43741 family —
  extents = sprite stats halves). Spawn states are 6·family blocks
  (m0→1, m1→7, m2→13, m3→19 … m16→97). Multipart (m0/m3/m6): 16 (2
  for m6) segment clones at state 120 = sub_19550 (:21107), a rigid
  follow chain via +52/+54 links, seg sprites 19+i / 89+i / 50+193.
- **State machine** = 6 shared primitives per family block (base +
  0..5): IDLE sub_19B10 (:21311, damage/death + aggro scan every v_26
  ticks), WANDER sub_19D70 (:21421, move + 2-LCG-draw yaw jitter
  :21506 — draw order load-bearing), CHASE sub_1A120 (:21580, bearing
  update every 4th tick), PACK sub_1A390, DEATH sub_1A6C0 (walks the
  segment chain), CORPSE sub_1A800 (spawns a pickup, despawns).
  Flyers add an altitude oscillator (sub_1B120 :22206: z += f26; f26
  -= 5; below ground+256 → f26 = 150 — the wing-flap bob).
- **Movement core = sub_196E0 (:21182)**: altitude clamp toward the
  behavior row's band (sub_42000 :52576) → polar step
  (sub_41EC0 :52523, x += speed·sin[yaw]>>16, y -= speed·cos[yaw]>>16)
  → **the creature wall rule**: sub_11640 (:16821) blocks the step if
  the target tile's terrain-capability bit (sub_11810 :16879; type 8
  wall → bit 0x100) is not in the creature's v_20 mask, or the local
  slope (sub_19650 :21149) exceeds v_16; blocked → retry yaw ±85,
  then reversed, 4 candidates, all blocked → life = -1 (die). Commit
  via move_relink; then rate-limited turn toward +34
  (sub_422A0 :52689, cap v_2).
- **THE CARPET WALL RULE IS EMERGENT** (Phase 5 keystone): the player
  move (sub_14EB0 :18781) commits horizontal motion UNCONDITIONALLY —
  no terrain test. Walls block because the wall-builder raises the
  heightmap +48 on type-8 tiles (:28999/:29030) and the carpet's
  altitude clamp (sub_132B0 :18035: z forced into [ground+128,
  ground+1024]) shoves the carpet up the wall face — blocking comes
  from the ground-height clamp + the steep lip, not a horizontal
  gate. Port the clamp faithfully and walls Just Work (and the castle
  breach exploit stays intact). Carpet specifics: accel approaches
  target speed in steps of 16/tick (:18828); turn rate
  angdiff/(((255-t)>>4)+8) clamped to [v_4, v_2]=[0,256] (:18832); a
  90°-offset strafe term decaying 4/tick; behavior row =
  unk_98F38[8] = {8, 0x100, 0, 0x100, 0, 0x400, 0x80, -4, 0x100,
  0x200, ~0x100 (mask: walls forbidden), 0x1800, 40, 0x2000, 0x200}.
  Player extents come from sprite 44's stats halves — REPLACE the
  point-extent stub in world.rs trigger/portal overlap when mobs
  land.
- **Yaw**: current +30, target +34 (11-bit, high byte = pitch);
  bearing via atan LUT word_9374C (sub_40F87 :51818). **Animation**:
  +86 sprite base, +88 frame, +89 frame count
  (byte_90AD8[word_99BA6[3+7·sprite]]); advance = sub_42510 (:52763)
  once per state-tick, returns "finished" driving transitions.
- **Flyer water rule**: sub_42090 (:52605) — over water-angle tiles
  the altitude floor drops to -768 (descend/crash); creatures
  standing on forbidden terrain flee/die via an every-8-tick check
  (:25936).

### Triggers & events — remc1 runtime trace (groundwork, 2026-07-05)

Full agent report was session-local; the implementation-relevant facts
(all citations remc1 sub_main.cpp unless noted; event struct =
Type_AE400_29795, 164 bytes, engine/Basic.h:368):

- **Runtime tick = `sub_41780_41AC0` (:52197)**, the per-turn sim.
  Called 1x/4x/16x per frame by DrawAndEventsInGame's gameSpeed
  switch (:41672). Per call, in order: (1) global LCG draw ONCE
  (`rand = 9377*rand + 9439`, :52223 — before any handler,
  load-bearing); (2) prepass over flagged events; (3) rebuild spatial
  collision lists by class (creatures list 0, class-5 by model,
  class-9 list 3, class-10 walls/buildings lists 1/2; heads
  str_36382x); (4) mana accounting `sub_48230_48570` (:56839);
  (5) main tick: slots 1..999 dispatch
  `dword_96902[class].str_0[state].data6(event)` — per-STATE (byte
  70) tick handlers with a state-id sanity guard, same pool/dispatch
  shape as the ported load-time loop (`sub_36620_369E0` is load-time
  ONLY). Dispatch tables to extract: `dword_96902` (:5041, 18-byte
  Type_96902 per class: tick table + spawn table), flat spawn tables
  `off_97D12` (:5075), `off_987DE` (:5167), per-class sub-tables
  str_254*/255*/256* (14-byte entries {marker, state-id, fn, enabled,
  _}).
- **Dispositions are the trigger mechanism.** THING_INIT (Type_1090,
  9 u16): data_8 = dis_id MEMBERSHIP (0xFFFF = load-time),
  data_12 = dis_id TO FIRE (for triggers; doubles as event field
  +24), data_10 = radius, data_14/16 = per-class params. Firing
  disId X (`sub_37440_37800(disId, oneShot)`, :43924) scans ALL 1999
  THING slots and spawns every thing whose data_8 == X via
  `sub_37560_37920` (:43988); oneShot=1 zeroes the record (consumed).
  disId 0 = full level (re)init (mana recount). That's how level-005's
  dis_id=1 crater fires. Spawn post-init per class: most classes get
  id_24=data_12 + radius extent (`sub_37130_374F0`); class-12 mana
  gets byte70 += data_12 with the >=3 "village jar" branch (280) we
  already know.
- **Class 11 = trigger volumes** (tick table str_256038 :4921, 32
  model handlers :67223+). Condition primitives: creature-AABB test
  `sub_5A090_5A5A0` and player-AABB test `sub_5A120_5A630` (3D AABB
  overlap via pos+72/extent+78, `sub_11950`/`sub_118C0` :16963).
  Handler flavors: model 0/1 = creature inside/outside, one-shot
  (frees itself); models 2/3 = periodic re-check (10-tick countdown
  in field +26), fires REPEATING (oneShot=0); model 4 = fires when a
  player inventory/spell bit (+13325 & 2) is set ("collect X to
  open"); models 5-16 = fire when a spatial list bucket is EMPTY
  (all creatures of a kind dead, 16-tick debounce, one-shot) — the
  kill-to-clear trigger. Trigger-spawned mobs are just the fired
  disposition's THINGs at their authored positions — placement only,
  AI later, exactly the slice boundary we wanted.
- **Earthquake = class-10 state 0x3A handler `sub_29780`** (:31140):
  ~15-tick life, expanding Bresenham ring 1 tile/tick, per ring cell
  2 per-event-LCG draws (x jitter then y jitter, rand%0x81-64) +
  child effect events (class 10 model 6); AoE entity shake/damage via
  `sub_120B0`; terrain damage comes from the CHILD crater events, not
  the quake itself.
- **World mutation core**: single-tile writer `sub_40A10_40D50`
  (:51621) — clamp height 0..200, honor the 0x80 protection bit (by
  mode), floor-to-water conversion when all 8 neighbors are water,
  and PER-TILE inline retile+reshade (`sub_33B90_33F80` /
  `sub_33E10_34200`); ring dig `sub_40D30_41070` (:51693); crater
  ring -3/pass `sub_255D0` (:28353); the runtime expanding crater =
  state 11 `sub_25670` (:28379, radius grows every 3rd tick via
  byte63%3, stops on water bit or center clamp). States 8/9/10 =
  free/impact-with-aftershock/radial-dig. Terrain queries:
  `sub_11760` = angle-nibble bitmask (craters stop on water bit),
  `sub_11810` = terrain-TYPE capability mask (water 0x100, deep
  0x200, lava 0x100000+ — the drowning/lava classification).
- **Spell/mana pickups (class 12)**: init `sub_3BF70` (:47981) sets
  total mana (+136), per-tick mana (+140 = total/tickCount), spell
  grant (+132); collection is NOT instantaneous — `sub_48230_48570`
  accrues +140 per tick by proximity/ownership into the collector
  (+136 drains); castle/building classes feed mana the same way. No
  respawn (one-shot when drained).
- **Portals: NOT an entity-class mechanism.** In-level teleport is
  the teleport SPELL (10) — spell-domain, deliberately out of this
  slice. Teleport-pad-looking level content would be class-11
  triggers + fired dispositions. Drop "portals" from the slice's
  scope; revisit with spells.
- **Open ends flagged by the trace** (verify during implementation):
  swi_id's exact THING field (likely data_16; only runtime consumers
  found are class-10 m34/m45 post-init params); class 7's exact role
  (read str_255620/str_2555CC if class-7 THINGs appear in target
  levels); the spell-grant write in the class-12 tick handler
  (SpellEnabled index); which class-10 MODEL spawns the earthquake
  state (spell-cast side).
- PRNG discipline for the port: global LCG 1 draw/tick at loop top;
  per-event LCGs seeded/drawn inside handlers (crater :28312/:28339
  1x, quake 2x per ring cell); u16 pseudoRand is a separate stream in
  some effect handlers — check per handler.

### Combat, damage, death & corpses — remc1 trace (banked 2026-07-05)

Fresh agent trace (session-local report distilled here). Citations
remc1 sub_main.cpp; event = Type_AE400_29795 (Basic.h:368). Class-5
tick table str_254DCC (:4687) is STATE-indexed (6·model blocks), same
for class-3 (str_254ADC :4668) and class-10 (str_255998 :4856 — mana
ball is model 39 but state 41 → sub_27030, set at spawn :47453).

- **Damage mailboxes = SIX 6-byte channels** at +90: `{u32 amount,
  u16 source_id}` × 6 (+90/94, +96/100, +102/106, +108/112, +114/118,
  +120/124; cleared `memset(+90,0,36)` :17977/:55369). ch0 = physical
  damage; ch1 = mana-ball collection claim (ball +144 = src, :29439);
  ch3 = mana steal (victim +140 → attacker +140, :55683); ch4 =
  grip/attract (humans: tether +314.. :55663; balls: velocity toward
  src :29451); ch5 = balloon recall (:56707); ch2 unobserved.
- **Write protocol** (all area writers): `if (+94) +90 += amt; else
  +90 = amt; +94 = attacker_id` (:17301-05 et al) — accumulate while
  pending, overwrite stale (readers clear +94 but NOT +90).
  **`sub_12B50` (:17600, melee/direct, no gates) has this INVERTED —
  almost certainly a transcription swap; port the area-writer
  protocol** (same risk class as the :21814 mis-fix).
- **Writers**: `sub_120B0(ev,ch,amt)` :17235 area write (radius =
  (+80+255)>>8 cells; gates: not owner (+24 equality), target flag
  +16&8, vulnerability `+28 & (1<<ch)` :17294, attacker's +66/+67
  class/model filter (-1 wildcards); ch0 also full-dmg pass over
  class-3 m2 :17325 and grid loop excludes them :17372).
  `sub_124F0` :17399 = ch0 variant, class-2 m0 buildings take
  amt/10 (:17465). `sub_127E0` :17502 = + balloon wobble +50=30.
  Direct life: building crush life=-1 :17635, kill-all sub_194F0
  :21098, armageddon (class2/5 life=-1; class3 via 12B50) :31290.
- **Creature inbox block** (opens EVERY class-5 state handler — IDLE
  :21330-67, WANDER :21450, CHASE :21597, PACK :21703, customs):
  (1) if awake (+58≠0) and +94≠0: `life -= +90` (u32, NO scaling),
  +94=0 (+90 stays stale), hitflag=1, +40=attacker (:21333-40);
  (2) awake: walk +54 segment chain — any segment's life < own →
  own life = segment life, +40 = seg's +40, hitflag=1 (:21345-61 —
  shooting the tail kills the worm); (3) always: life<0 → hitflag=2,
  +38=+40 (killer latch, :21363-67); (4) hitflag=1 → if attacker
  class 3: +146=attacker, state→base+2 CHASE (:21370-76; non-class-3
  attackers ignored for aggro; CHASE just retargets :21636; PACK
  retargets leader+self :21742); hitflag=2 → state→base+4 DEATH.
  Runs BEFORE movement. No creature armor. Family customs on hit:
  m6 buffets the victim's camera (+24/+26/+22, sound 42 :23223);
  m8/12/13/14 set victim wizard +528=200 "under attack" instead of
  chasing (:25057, :25147, :25270, :25534).
- **Human inbox** (sub_45C90 :55323): grace `u16_331≠0` → memset all
  six channels + decrement (:55367-71; spawn=100; **permanent
  invincibility = grace that never decrements**). Else sub_46540
  :55641: ch4 grip → ch3 steal → ch0: shield flag byte[1]&0x40 →
  dmg>>=2, mana-=dmg/4, bit cleared (one hit per arm :55700-07);
  life-=dmg; camera kick +24/+26=bearing-to-attacker, +22 =
  clamp(dmg/10,0,80); hit anim, flash +392=4, **regen stall
  +383=16**, sound 17 (:55710-26); life<0 → +38=attacker, state 2,
  sound 16. Regen: mana += +132/tick always; life += u16_341 only
  when +383==0 (:55385-90), rates max/2000 (min 100) & max/2000,
  pad-boosted /200 (min 1000) & /250 (:55407-21). AI wizard: same
  blocks (:17975-84), unconditional life regen (:17991).
- **Attack thunks** (CHASE :21665-72: every `+63 % v_26 == 0`, dist3d
  ≥ v_28 → WANDER, else thunk; target lost → WANDER :21658).
  Projectile fields: +66/+67 target filter (creatures fire 3/-1),
  +146 homing target, +156 homing row (unk_98F38: [6]=straight,
  [2]=turn 0x71, [3]=0x11), +24 owner (immune), z += attacker +84,
  +68/+69 explosion class/model. Per model (dmg on projectile +44):
  m0/m3 `sub_1A8E0` :21874 class9 m0 dmg 500, expl 10/0, row [6],
  sound 8. m1 melee `sub_1AB10` :21962: dist3d<1024 → 12B50(ch0,
  own +44), sound 7. m2 same thunk sound 13 + z-homing chase, hit →
  recoil speed=-accel, cooldown +26=3·v_26, lunge 3·base on expiry
  (:22335-62). m4/m10 `sub_1A990` :21907 class9 m13 dmg 250, sound
  195 (+ m4 mimic: 1 LCG draw, sprite 206 if rand%20≤10 else 1, copy
  target class/model :22744; restore :22766). m5 `sub_1AB70` :21976
  mana-scaled: v2=7·mana/maxmana, **1 LCG iff v2≠0** (rand%(100·v2)
  /100), count clamp(v2,1,5); case 0: n×class9 m0 dmg 400 rows
  [6-i]; 1-2: (n-1)×m9 dmg 800 expl 23; 3-6: 1×m3 dmg 8000 expl 17
  row [3]; sound 32; regen maxlife>>7/tick; eats mana balls (:22986,
  absorb +140); lays class10 m52 at mana>max+500 (1 LCG,
  +26=10·(rand%10)+100, cost 500). m6 :23120: speed pinned 30,
  counter +26 (++, >40 → -90); +26>0 → buffet; every v_26 in range
  sound 37, +71=5; each tick +71>0: class9 m9 dmg 800 expl 23 row
  [6], +66/67 := target's. m7 `sub_1AE30` :22101 class9 m14 dmg 780
  (bolt sub_54180 :63789: straight, 1 LCG sound 33+(rand&3), direct
  120B0(ch0,+44) on hit); anim 85↔198, +26=30 (:23319-56). m8
  `sub_1AEE0` :22134 class9 m9 dmg 4000 expl 23, filter := target's;
  flag byte[1]|=0x80/tick, sound 38, +528=200. m9 :24120 →
  `sub_1AA40` :21935 class9 m13, dmg 600 if segments else 400, sound
  203; range = v_28 + target x-extent for balloons (:24198); **aim
  bug: :21947-48 passes own pos twice — atan2(0,0); use the target
  (cf. sub_1A990 :21920)**. m11 :24554: breaks off below maxlife/2;
  class9 m8 dmg 3000 +26=20 expl 25 per v_26, sounds 9/11 (:24661;
  duplicated-branch suspect :24669); idle alternates 11×class10 m1
  fire grid / teleport (2 LCG, ±(12800+(rand%60)<<8)) :24336-84;
  eats balls (:24751). m12: no attack; builds castles state 0x49
  (1 LCG type=(rand&7)+25, ≤4 attempts × 2 LCG, class10 m45 state
  51, sound 10 :24980). m13/m14: siege feeders — join class10 m45
  castle (castle +26++ vs +128 capacity), death state with +26≠0 =
  silent absorb, no corpse (:25447, :25623). m15: class9 m13 per
  v_26, NO +44 override (spawn default), no row (:25846); hostile
  terrain → state 94 death (:25934). m16 dragon :26062: in v_28 AND
  facing within 0xE3 → +26=15; each tick +26>0: class9 m0 dmg 3000
  **+140=60000** row [2] (turn 0x71), z += 4·(+84); sound 39.
- **Projectile→damage**: flight handlers home via sub_52550 :62534,
  acquire via sub_54520 :63943 (models 0/3/4); mana-shield reflect
  (victim byte17&0x80, cost proj +140/4): 1 LCG scatter (fireball
  %0x5B-45 :62875; others %0x2D-22 :63141), owner := victim, life
  refill. Explode → spawn class +68/model +69; generic sub_52770
  copies +44 AND +146 to the explosion (:62759-72); m13 always →
  class10 m38 (:63767). The EXPLOSION does the mailbox writes (fire
  sub_24F60 ch0 once on landing :28073; wave sub_252D0 ch0 EVERY
  tick via 124F0 :28255). Accuracy stats +343/+347 (:62585-613).
- **DEATH sub_1A6C0** :21820 — ONE tick: walk +54 chain, all
  segments → state base+5 CORPSE, propagate nonzero segment +38 to
  head (:21828-39); kill credit; own state → base+5. No fall physics
  — death visuals are the per-state sprite tables. sub_42090 :52605
  (water sink to -768, despawn) belongs to the dropped-pickup ticker
  sub_55A40 :64729, not creatures.
- **CORPSE sub_1A800** :21855 — acts on `+63 & 7 == 0` (≤7-tick
  linger): (MP: +140=5000); `sub_27690` :29663 iff +140>0: **1 LCG
  draw on corpse seed (result unused — keep the draw)**, spawn
  class10 m39 (state 41) mana ball: ball +140 = corpse +140, +144 =
  corpse +144, then 2 LCG on BALL's seed: heading = corpse heading
  + rand%0x71 - 56, speed = rand%0x30 + 16; vert vel +46 =
  (1024 - height_above_ground)>>3 (:29675-93); corpse +144=0. Then
  class10 m1 death-flame puff (+24 = corpse id; life 1, +44=400,
  sprite 41, radius +26=0 → no spread; :21866, spawn :46482, tick
  sub_25130 :28127, 3 LCG/cell only when radius>0). Self-destroy.
  Segments each drop their own ball+puff. Ball defaults (:47443):
  +140=512, +46=128, +28=3, +58=0x80, sprite by size sub_274D0
  :29574 (thresholds dword_900A4, owner palette 105+8·slot, 52
  unowned). Ball tick sub_27030 :29416: ch1 claim/ch4 attract;
  vel clamp ±64, gravity 16 (floor -128), quarter-bounce, friction
  250/256, merge on overlap sub_277D0 :29700 (sum +140).
- **Kill credit** :21840-50: killer class 3 MODEL 0 (human only) &&
  head && model ∉ {9,12,13,14,15} → `++player+359` kills counter.
  No mana/XP — the reward is the ball. End-level %: 100·kills/census
  (:54691); census = load-time class-5 count excl. 9,12-15 (:43933).
  Player death drops: 24 inventory scatters (2 LCG + life rand%0x5A
  +200 each :55519-49), grave m40 inherits owned balls (:55550-65).
- **Decompile suspects** (beyond 12B50): m7 regen :23305-11 is
  garbage (port min(life+max>>6, max), cf. :24472); m9 aim (above);
  m11 dup branches :24669; corpse unused LCG draw :29674 (keep);
  class-9 table :4838 shows states 0..13 but sub_535E0 :63349 sets
  +70=14 — table transcription may be truncated.

### Fireball / repeat fireball — remc1 trace (banked 2026-07-05)

Fresh agent trace (session-local report distilled). Citations remc1
sub_main.cpp. Player-confirmed: fireball and repeat fireball are the
SAME missile — only the input gate (and cost) differ.

- **Classes**: class 12 = spells (model = spell id 0..23, state =
  3·id + phase; tick table str_2563D8 :4957, creators str_2567D8
  :4586). class 9 = projectiles (fireball = m0 state 0; tick table
  str_25573C :4838, inits str_255870 :4463). class 10 = effects
  (impact = m0, splash = m5). UI slot→spell map byte_99B88 :5752:
  slot 0 = spell 0 fireball; spell 23 = repeat fireball. (Spell 3 =
  class9 m1, NOT a fireball :65205.)
- **Spell row** `sub_3BF70(ev,id,3·id,A,B,C,D,E,F)` :47981: +44=F,
  +50=B refire window, +62=D charge max, +60=C mode (1=click-edge,
  0=hold-autofire), +140=A/B (projectile mana), +132=E castle gate,
  +136=A per-shot cost, sprite 77, +42=caster (:54906). Fireball
  `sub_3C090` :48032 = (200, 5, 1, 0, 0, 125): cost 200, window 5,
  click mode, +44=125. Repeat `sub_3C480` :48068 = (600, 3, 0, 0,
  50000, 50): cost 600, window 3, HOLD mode, castle-mana gate 50000
  (compared only, never deducted :64919/:26924), +44=50.
- **Input→cast**: mouse edge/held :15444-63; per-frame UI :20588-635
  issues MakeControlCommand(6,16/32) (L/R) — mode 1 on edge only,
  mode 0 on edge OR held while +48>0 (:20627-30) ⇒ re-arms every
  tick ⇒ 1 projectile/game tick while held, mana-limited. Command
  bits → Type_160.dw_0 (:49021; 16=L, 32=R). Player MOVE tail calls
  sub_46B00(player, spell, 0x100/0x200, 16/32) :55825-30 → :55851:
  mana gate (player +140 ≥ spell +136 :55875), **ARM: spell +48 =
  +50** :55893, caster flags |= 0x100/0x200 (muzzle side) :55894.
- **Cast tick** (class 12 state 0 = sub_56090 :65039; repeat state
  69 = sub_58240 :66295, byte-identical): +48≤0 → nop. Gate
  sub_55DD0 :64908 (mana/life ≥ 0, castle gate, mana ≥ cost at
  burst start +48==+50); fail → sound 29, +48=1. **Spawn only when
  +48 == +50**: proj = NewEvent(caster pos, class 9 m0) :65058;
  `+126 += caster +126` (inherits carpet speed onto base 384)
  :65060; **muzzle offset** sub_55EF0 :64965: 256 units at yaw∓512
  by flag 0x100/0x200, reverted if muzzle terrain above caster z
  :64981; +68/+69 = 10/0 :65064; +24 = caster +24 (owner) :65066;
  +44 = 125 (vestigial, see below); +140 = 40 (deflect cost basis);
  z += caster +84 :65069; +30/+32 = caster yaw/pitch :65070;
  +26 = u8_326 ticks-since-last-cast (then zeroed; clamped ≤16 in
  sub_54520, else unused) :65072; +150.. = pos + 0x4000 ahead
  (vestigial for m0) :65074; **launch sound 9** :65079. Then
  `--+48`/tick :65086 — click mode: 1 shot + 4-tick tail; refire =
  click rate. **Mana deduction sub_55E80 :64937 is remc1-DISABLED
  (`//fix` :64944-50): original = spawn tick `caster +132 =
  -(cost)` (or -= if already negative), mid-burst clamp positive
  +132 to 0 — +132 is the SIGNED regen accumulator (Basic.h has it
  u16 — port as i16), added to mana every tick (:55387) and
  recomputed from maxmana each tick. Same pattern :65115.**
- **Projectile init sub_39A10** :45861: speed +126=+128=384, life
  +8=+12=0x2000/384=21 ticks, +140=50 (cast overwrites 40), row
  +156 = unk_98F38[**5**] (player cast does NOT override; mob
  dragon sets [2] :26165), +16 &= ~8 (not hittable), sprite 42,
  extents = sprite halves; NewEvent defaults +66/67=-1/-1 (hits
  anything), seed = slot + global rand.
- **Flight sub_52B30** :62779 per tick, in order: (1) +146==0 →
  one-time (flag +16|=2) **aim assist** sub_54520 :63943: scan
  wizard list (range = CASTER row v_28) + all 20 class-5 model
  lists (awake only, any range) :63991-64007; candidate within
  ±0x71 yaw AND pitch cone and dist3d ≤ 5120 (sub_54A90 :64191,
  lowest squared miss wins); hit → +146 = target, +34/+36 = angles
  (z-centered), notify human target :64013; else +34/36 = +30/32.
  Turn: yaw ≤34/tick toward +34, pitch snaps :62815-25. +146≠0 →
  home every tick sub_52550 :62534 (caps row v_2/v_6: row[5]=5,
  row[2]=0x71, row[6]=0). (2) move: polar step yaw/pitch/speed
  :62842. (3) hit scan sub_11980 :16988 (cell spiral (+80+255)>>8;
  gates: +16&8, +66/67 filter, **+24 ≠ proj +24** — owner-equality
  immunity is the ONLY friendly-fire rule); hit non-rebound →
  teleport onto target, explode — **projectile writes NO mailbox**;
  rebound (+17 bit7, mana≥10): victim +140 -= 10, sound 28,
  reverse + 1 LCG scatter (%0x5B - 45), retarget shooter, owner :=
  deflector, life refilled :62858-90. (4) no hit: ground ≤ z →
  --life, <0 → midair explode :62903-10; ground > z → un-move;
  water (sub_11810 type 0) → class10 m5 splash + delete (NO
  explosion/damage/crater) :62916-21; else explode. (5) explode:
  spawn class 10 m0 at pos (owner/yaw/pitch copied; **+44 NOT
  copied** — sub_52B30 omits the word[22] copy that generic
  sub_52770 does :62771 ⇒ effective damage = the fire's constant
  400 for BOTH fireballs; spell-row 125/50 dead — flagged, verify
  vs retail someday); accuracy stats sub_526C0; delete. LCG: 0
  draws/tick in flight, 1 on deflect.
- **Explosion class 10 m0** (init sub_3A490 :46454: life 8,
  **+44=400 = the real fireball damage**, +28=0, not-hittable,
  +18|=2, sprite 7, extents forced 128³ :46476; tick sub_24F60
  :28047): first active tick (+26&3 gate, 0 for fireball): if +18
  bit0 clear → **sub_120B0(ch0, 400)** :28080 (victims need +28
  bit0, +16&8, owner ≠, not class-3 m2; attacker id = original
  caster ⇒ kill credit flows); terrain conversions 26→0x14, 10→
  0x15, 11→0x16 :28088-97; else if type ∉ [6..0x22], slope nibble
  ≠1, z-ground ≤ 128, not water: **1 LCG draw → crater sub_40D30
  (depth -(rand%7))** :28100-07 (the ported trigger-crater fn);
  2nd LCG: +46 = rand%65 - 32 drift; sound 3 :28110-18. Every
  tick: sub_42000 motion + frame advance; ~10 ticks, delete.
- **Tick-order notes**: global rand steps once at :52222; spatial
  lists rebuilt (class-9 list at 36474 :52286 — balloon-vs-
  projectile sub_11AC0 scans it); commands applied :49021; slot-
  order dispatch. Spell events tick every frame even unselected
  (idempotent at +48==0). Class 9/10 NOT in population counters
  (:43977) — no disposition interference. Rebound changes owner ⇒
  a deflected fireball can kill its caster.
- **Port bypass for the dev infinite-fireball**: skip sub_55DD0
  gates + sub_55E80 deduction only; everything else verbatim.

### Static terrain features (LANDED 2026-07-05, MC1 + HW)

`mgc_sim::features` is a full port of remc1's
`GenerateFeatures_36430_367F0` (sub_main.cpp:43043), applied by
mgc-app at level load to the pristine baked planes (`--no-terrain-
features` renders the raw generator output for comparison). What the
pass does, per the four-agent decompile trace (scratch reports; key
findings inlined here):

- Level entities with `class 10 && dis_id == 0xFFFF` are consumed in
  slot order 1..1999. Chained models (28 walls, 29 tracks, 31 canyons,
  50 ridges; `swi_id != 0` = pending flag, parent/child = 1-BASED
  slot links, our slot + 1) run per-segment terrain functions; other
  models spawn events into the original's 1000-slot pool, which an
  event loop then ticks to fixpoint. Dispatch is by the event's
  byte-70 tick index, NOT its model; the creator table is indexed by
  model directly (model 39 ≠ trees — it and most models spawn events
  that are purged unticked; only 9/10/11 craters, 30 track pieces,
  32 canyon heads, 45 buildings, 51 ridge heads do terrain work).
- Craters: model 9 = growing hill + final -40 pit; 10 = one-shot dish
  -(rand%7) honoring building protection; 11 = expanding -3 bowl whose
  radius growth is gated on `pool_slot % 3` — slot allocation order is
  load-bearing and reproduced exactly (free stack 999→1, LIFO reuse).
- Canyons walk 1 tile/tick along an atan-LUT heading spawning 3-tick
  diggers; ridges (model 50) raise +(rand%15+10) discs every 4 tiles;
  walls (28) decompose into staircase strips of +48-height type-8
  tiles with 0x80-protected borders; tracks (29) stamp angle-nibble-1
  lines. Buildings (45): footprint RLE from `BUILD?-0.{TAB,DAT}`
  (baked as `build-N.{tab,dat}.bin`), 30 ticks of progressive flatten
  toward the 4-corner average (+4·k/+12/+16 cell offsets), painting
  every 5th tick via the `unk_909xx` pair tables (texture ids 8..34),
  protection bit 0x80, final perimeter smoothing — the event then
  survives as the persistent castle/building entity (entity track
  will pick those up; we currently drop them after the terrain
  effect).
- Dig cells clamp height to 0..200, convert floor-hits to water when
  no neighbor blocks it, and retile through `byte_B5D40` (the flat
  2401-entry first-candidate table — now derived in
  `mgc_sim::mc1_tables::retile_table` from the shared corner-class
  buckets; mc1_terrain.rs imports the same machinery). Retiling draws
  the u16 pseudoRand stream; its post-generation state is replayed
  from the pristine height plane (the generator's shading pass resets
  it to 0 — `features::post_generation_pseudo_rand`). The global u32
  rand = the GEN_MAP seed, advanced once at event-loop entry;
  per-event LCGs seed from `slot + global`.
- Ring iteration order comes from `SEARCH.DAT` (baked `search.bin`,
  32x32 ring grid; ring 0 is a 2x2 block) — including the original's
  off-by-one that drops the last cell of a dig's outermost ring.
  Static LUTs (sin/cos/atan/bitSqrt/paint pairs) are extracted
  verbatim by `tools/extract-remc1-tables.py` → mgc-sim `tables.rs`
  (formula reconstruction does NOT match; keep the extractor).
- Deliberately omitted (terrain-neutral at load): damage broadcasts
  (they pre-damage persistent building entities — revisit in the
  entity track), sounds, and the class-10 events' transient
  mapEntityIndex links' gameplay side effects.

Validation: level 005 now shows the scar valley (the five-node canyon
chain (147,202)→(99,113)) and its village; the chain-terminating
crater (model 11 at (95,108)) has `dis_id = 1` — it is a
disposition-TRIGGERED runtime crater, not a load-time carve, contrary
to the earlier roadmap note. Level 039's degenerate plateau carries
its authored 329-segment wall maze; level 034 is a four-corner
labyrinth + keep (likely the portal-maze quirk level); level 025's
ridge chains render. All 143 MC1+HW maps regenerate without error
(`baked/maps/` now includes features — the DOSBox "EXPECTED
deviations" list shrinks to entities/sprites only). PLAYER-VALIDATED
2026-07-05: side-by-side DOSBox comparison during the session found
no deviations — the MC1 static-feature track is CLOSED. MC2's
feature phase is an INDEPENDENT implementation in the original;
porting it from remc2 is a separate piece of work (the mgc-sim
scaffolding — planes, event pool, loop shape — should carry over,
but the tables/formats are MC1-specific).
- Verification: remc2's deterministic replay + memimage approach as
  template; their fixtures already validated our terrain.

Gameplay checklist after the fidelity pass (updated 2026-07-05:
triggers & events LANDED, then mobs spawn+movement LANDED — see their
sections): ~~mobs (real spawn handlers + movement)~~ → NEXT: mob AI +
combat (attack thunks spawn class-9 projectiles, damage mailboxes
+90/+94, the custom family behaviors, possession), the Phase-5 carpet
flight model (human sub_455D0 trace banked above), spells as combat,
mana/spell collection (unblocks class-12 pickups, corpse mana drops +
kill-triggers firing in anger).

### The quirk problem (design note, 2026-07-04)

The biggest overall hurdle will be *quirks* — much more so for MC1
than MC2. MC2 is polished: handholding, sane goals, stage scripting.
MC1 after roughly the first 25 levels switches to "challenge the hell
out of the player", and its late campaign is built on emergent,
borderline-exploit strategies that the tiniest details and rounding
errors can make or break. Two player-attested examples to preserve:

- A level that spawns you with nothing but a fireball spell against
  several level-7 enemy castles and a huge wyvern flock. The intended
  (only?) strategy is emergent: lure the wyverns into chasing you,
  kite them into destroying the enemy castle, steal the spilled mana,
  build your own castle — with heavy save/load execution.
- A portal maze level (no castle allowed inside, progressively harder
  sections, a final section whose crabs eat loose mana, grow huge and
  invincible, and kill you). The known completion is a quirk: in the
  second-to-last section, one corner has a razor-thin line of sight
  out of the maze, and a castle spell threaded through it collects
  your claimed mana and triggers the completion threshold before the
  final section is ever entered.

Implications, so we don't design them away later:
- These strategies are the content. "Fixing" them breaks the game.
  Line-of-sight margins, projectile collision widths, mana accounting
  and thresholds, aggro/flocking behavior — all must reproduce the
  original's math, including its rounding. Expect the gameplay sim to
  need the original's fixed-point integer arithmetic (16-bit axes,
  the 8.8 tile fraction) rather than floats; f32 in mgc-sim today is
  fine for the flyer but not a foundation for combat/mana rules.
- The original games have NO savestates. The "save" mechanic is the
  castle: the player respawns at their castle (any level of it); with
  the castle wrecked or castle-building forbidden, death restarts the
  level from scratch. That absence is itself load-bearing design —
  the maze level bans castles precisely to take your checkpoint away
  — so castle-respawn semantics must be authentic. But per the player
  this is a classic hardcore-era mechanic not worth missing: hours of
  nitpicking a hard level lost to a death at the very end. So plan a
  real in-level savepoint feature as a modern, clearly-non-original
  convenience (same opt-in stance as extended controls). The
  deterministic sim gives exact, cheap snapshots for free, and the
  same machinery doubles as the dev/debug/replay tool regardless of
  what the player-facing option is set to.
- Once a quirk strategy works, record it as a deterministic replay —
  the replay suite becomes the regression test that tuning never
  silently kills a required exploit.
- Playtesting-against-memory is the only oracle for most of this;
  budget for it (the fun kind of testing, per the player).

## Spiritual successors (design stance, not work)

If this becomes "the ScummVM of carpet flyers" (I of the Dragon et al.),
the protections are cheap and already mostly structural:
- `.mgcl` `game` tag is an open set; members are additive. A new title
  is a new tag + new members, never a schema rewrite.
- One importer module per title; nothing shared by force.
- Keep the sim substrate/personality boundary honest (forced anyway by
  MC1 vs MC2 rule differences).
- Do NOT pre-abstract beyond evidence: successors share feel, not data
  shapes. Generalize when a second concrete target exists.

## Open questions / parking lot

- Original-engine curiosity (player, 2026-07-04, during validation
  runs): on some levels the letter 'u' doesn't render — "PAUSED!"
  draws as "Pased!", chat cheat "quick" displays "qick" yet matches.
  Diagnosed: shipped FONT0/1/2 glyphs are all intact (verified from
  retail data), and DrawText advances by the glyph TAB entry's own
  width — so a runtime, level-dependent overwrite zeroes that one
  font TAB entry (classic asset-arena clobber; remc1's data segment
  even contains ASCII "re" fossilized into a numeric table,
  word_968B4). NOT a quirk to preserve: heap-layout corruption, not
  gameplay math; our baked-asset paths can't reproduce it.

- `hmap2` (second heightmap, dump region +0x40000): SEMANTICS SOLVED
  (2026-07-05, water-animation trace) — it feeds the tile projector's
  `inverse_alt_8`/`alt2_8` mirrored vertex (remc2
  GameRenderOriginal.cpp:1052: `x_BYTE_14B4E0_second_heightmap << 15
  >> 10`), i.e. the WATER-REFLECTION plane, sine-wobbled per frame.
  Rebuilt post-load, so not baked; derive it when a reflection pass
  lands (MC1 gates the whole thing on its `reflections` detail
  setting).
- GEN_MAP pre-header + class-0 marker semantics still open. The MC1
  "footer" mystery CLOSED 2026-07-09 (hostile-wizards level-tail
  decode): it's (map-word, PLAYER COUNT, castle levels[8]) — the
  2026-07-04 footer[1]≈player-start-count observation was that
  player count all along.
- MC1 CAMPAIGN SKIP TABLE (found 2026-07-04, player-discovered then
  code-confirmed, player-verified for 008 and 017): the single-player
  campaign HARDCODES skipping level indices {8, 17, 28, 33, 39} —
  remc1 `sub_34070` (sub_main.cpp:41456) bumps the level counter past
  them. The picture after the full validation pass:
  - Campaign = indices 0-49 (displayed as levels 1-50, index 049 = the
    finale), minus the five skips = 45 played levels.
  - Indices 050-069 = the multiplayer/netherworld map pool (footer[1]
    is max players — 8 there, with 8-14 player starts placed).
  - Skipped 008/017/028/033 are complete, fully-populated worlds
    (008: 683 entities) with footer[1] of 2-6 — most plausibly
    multiplayer maps parked inside the campaign index range; browser
    content ("lost levels") alongside MC2's 18 dev levels.
  - Skipped 039 is a BROKEN level: its GEN_MAP params (raise=-7720,
    gnarl=106, seed=58704) hit the generator's authentic degenerate
    collapse — the fractal field stays all-negative and the 16-bit
    normalize clamps the whole map to a flat featureless plateau
    (player-confirmed in DOSBox; our port reproduces it exactly via
    the load-bearing i16 corner-sum wrap). 1083 entities were placed,
    so the level was authored and its terrain roll later broke;
    likely the original REASON the skip table exists.
  - NO SPELL IS UNIQUE TO A SKIP (checked 2026-07-07 against baked jar
    data): 008 has zero spell jars; 017/028/033/039's jars are all
    obtainable in played levels too (039 is the richest of the five at
    12 spell types — its economy was designed but never shipped). So
    the campaign is self-consistent: the reachable 45 levels grant the
    full 24-spell repertoire, the skips cost the player nothing, and
    `plausible_spellbook` loses no spell by honoring the blacklist.
    No dev curveball.
  - STRONGER (deadline-safe): every one of the 24 spells has a PLAYED
    (non-skip) source BEFORE level 25 — the point where the per-level
    availability mask engages and strips anything uncollected. 24/24,
    no skip-only pre-25 dependency. First-played sources span level 0
    (Fireball/Possess/Castle) to level 24 (Global Death, the tightest
    case — the last spell lands on the last level before the mask).
    Reads as intentional tuning: the skip table is arranged to never
    make a spell unreachable ahead of the deadline.
  Engine implication: campaign progression must replicate the skip
  table; the level browser should expose the skipped five explicitly.
- MC2's 18 extended-format dev levels (~39 KB, version != 2): parse
  someday for the "lost levels" browser.
- LEVEL-032 TRIGGER CHAIN STALL — CONFIRMED pool exhaustion
  (player-reported + probe-verified 2026-07-05; fix deferred by
  agreement): disposition 14 (the room before the final section)
  authors 532 pool slots of content — 30 ground worms (17 slots each)
  + 21 mimics + LAST IN SLOT ORDER the class-11 trigger that fires
  disposition 15. With 421 slots free at fire time, 24 worms + 13
  mimics spawn and the dis-15 trigger silently dies on the empty pool
  — the chain severs exactly as the player sees ("only worms, no
  portal, no triggers left"). AUTHENTIC Bullfrog fragility: the
  original runs the identical slot-order spawn loop; the chain
  survives only when enough kills have freed the pool (kill-all
  playstyle), which is why the player hit the same stall in the real
  game once on a sneakier run and why our combat-less port reproduces
  it deterministically. The player also flags a second authoring error
  in the same room: a portal leading to an unreachable meteor-spell
  alcove (the known completion strategy is the castle-respawn
  force-through) — the "official" trigger sequence is likely
  mis-authored. Standing instruments: `cargo run -p mgc-sim --example
  dbg032chain` walks the whole chain headless and logs free slots /
  spawn outcomes per disposition (World::debug_pool); `dbg032dis`
  dumps the authored per-disposition records. Revisit after combat
  lands (corpse cleanup restores the intended slot economy).
  AGREED FIX DESIGN (player, 2026-07-05, for the fix iteration): an
  engine-level G-class toggle `disposition_spawns: faithful |
  progression-first` — the fixed rule sorts each disposition's
  records so class 11/10 (triggers, portals) spawn BEFORE class 5/2
  creatures, so pool starvation eats trailing monsters, never the
  chain (generalizes to any level with the same latent fragility; a
  032-only data patch was rejected — bake stays faithful). MUST be
  G-class: spawn order shifts pool slots → per-event RNG seeds
  (slot+global) → facings/jitter/m7 parity all differ; replays taped
  under the fixed rule are not faithful fixtures. Rationale: upstream
  has verified defects in the completed game; an unfirable chain is
  beyond quirk protection.

  LEVEL-032 AUTHORED CHAIN (player-recorded walkthrough 2026-07-05 +
  data correspondence; the fix iteration's reference):
  1. Entry portal (11,253) → maze start (5.5,230.5); the dis-3-spawned
     trigger at (4,229) fires dis 18 = a courtesy portal (12,234) →
     (18.5,234.5).
  2. First corridor: 4 one-shot triggers up the west wall — dis 1
     (4,221) → 2 (4,213) → 3 (4,205) → 4 (16,183) — each spawning
     monsters + a spell jar; dis 4 is huge (319 slots: bees, worms,
     wraith/skeletons, burrowers, 49 mana balls) and spawns the
     progression portal (19,182)→(18.5,176.5) PLUS the meteor alcove's
     RETURN portal (241,217)→(4.5,220.5).
  3. Next rooms: dis 4's trigger (4,148) → dis 5 = portal
     (252,167)→(254.5,145.5); its trigger (254,144) → dis 6 = 136
     slots of monsters + portal (253,229)→(237.5,234.5).
  4. THE METEOR ALCOVE BUG (player theory CONFIRMED from data): the
     alcove ~(239-241, 210-217) holds the spell jar (dis 3, (239,210))
     and the return portal to the start (dis 4) — but NO portal in the
     level has a destination inside it; the intended entrance was
     never authored (should have been a second portal / different
     destination). Known player strategy: castle outside, die, force
     through. Genuine dev mistake.
  5. Dis 6's trigger (237,235) → dis 7: obelisk row. AUTHORING SLIP
     confirmed: dis 7's trigger (236,230) is MODEL 2 = REPEATING
     (10-tick rearm; every other chain trigger is one-shot model 0) —
     the player-observed "first obelisk trigger never disappears, can
     re-fire the second post forever" (harmless; re-fires spawn a
     duplicate obelisk + trigger). Chain: dis 7-10 each spawn an
     obelisk post (c2 m1) + the next trigger; dis 11 = the MEDUSA post
     (c2 m3); dis 12 = 85 slots of monsters + portal
     (215,232)→(206.5,218.5).
  6. That portal lands ON the dis-13 trigger (206,219) → spawns the
     dis-14 trigger just south (204,231) = THE POOL-STALL trigger (532
     slots wanted; see above). Denied continuation: dis 15 = portal
     (188,243)→(214.5,250.5) beside the failed trigger; its landing
     trigger (212,250) → dis 16 = the DRAGON room (11 burrowers + the
     c5 m16 dragon — extremely tough per the player; no magic-bomb
     spell exists on this level) + trigger (230,245) → dis 17 = final
     portal (228,253)→(8.5,225.5) — which LOOPS BACK to the first
     room (authored; there is no way out of the maze) + a 14-tile
     kill trigger at (238,11).
  7. THE ENDGAME IS A NO-OP (verified): the kill trigger is state 18 =
     bucket 5 → it watches the five CRABS (c5 m5, spawned dis 4/6),
     NOT the dragon (m16 = bucket 16 = state 29; nothing watches it —
     corrects the earlier session note "kill trigger watching a
     dragon"); and dis 19 has ZERO records of ANY kind — firing it
     spawns nothing. Consistent with MC1's completion mechanic:
     the level is won by the mana/possession THRESHOLD (the known
     castle-through-the-sightline quirk strategy), not by escaping.
     Fourth authoring defect of this level's tail (stall, alcove,
     repeating obelisk, empty dis 19) — Bullfrog never tested the
     "official" path to the end.
  8. VERIFIED AGAINST THE ORIGINAL IN PLAY (player, 2026-07-06,
     MC1PLUS with a level skip to 032): "as accurate as it possibly
     can be" — the never-disappearing obelisk trigger and the no-op
     final kill trigger both reproduce in the RETAIL game, upgrading
     this whole analysis from data-reading to original-verified. And
     the analysis paid out forward: armed with the alcove finding,
     the player retrieved the practically-unreachable METEOR SPELL in
     the original and completed level 032 with it — possibly as once
     intended, and by the player's account for the first time ever.
     When the replay suite exists, tape that strategy as a fixture
     (it exercises the alcove force-through + the completion
     threshold end to end).
  9. DIFFICULTY CALCULUS CONFIRMED on a second original replay
     (player, 2026-07-06): without a lucky wall-dent by the meteor
     jar, the crab stage is a hard wall — the crabs eat the loose
     mana, outgrow the fireball's damage economy entirely (rebound
     deflection helps but does not break even), and an hour of
     skilled attempts could not pass; WITH the meteor spell the
     same stage is trivial. So 032's balance genuinely hinges on
     the mis-authored alcove: the intended difficulty valve was the
     meteor pickup, and the missing portal destination is what
     turned the level infamous. Preservation stance unchanged (the
     bake stays faithful; the crabs' mana-scaling is exactly the
     m5 machinery we ported) — but this is prime material for the
     eventual sanctioned-mercy catalog (savepoints, extended lift),
     and the replay suite should tape BOTH the meteor route and a
     crab-wall failure run as fixtures.
- Model-15 (grid-walker) direction-vote weights: the original reads a
  4-entry word table through a code/data alias (`*(_DWORD*)sub_1FF40`,
  sub_main.cpp:25931) the decompile can't express; our port uses
  uniform weights (same draw count, streams align). Extract the 8
  bytes at that VA from the retail MC1 executable someday.
- TAB shared-slot alias groups (25-27, 28-30…): how the game's
  secret-exit redirect actually resolves indices.
- Objective text ↔ stage checkpoint pairing (string 48+ sequential
  scheme) — needed for mission UI.
- remc2 upstream license ambiguity (MIT vs GPL3, their issue #190) —
  we assume GPL3 for all vendored/ported code.
