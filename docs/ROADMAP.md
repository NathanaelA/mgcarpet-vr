# mgcarpet ROADMAP

A faithful Rust remake of Magic Carpet 1, Hidden Worlds, and Magic Carpet 2
on one superset engine, running against pristine GOG game data. Both
campaigns are playable end-to-end (MC2 through its finale; MC1/HW hub +
levels); the full MC2 roster, spell set, rivals, stage engine, castle, and
campaign menus are ported; MC1 retail bit-identity is pinned by state-hash
goldens throughout.

This file is the brief successor to the 9,160-line development ledger, which
is preserved at `archive/ROADMAP-2026-07-19-full.md`. It states what stands
and lists what remains; history lives in the archive and git.

## Document map

- `ROADMAP.md` (this) — status + remaining work.
- `RETROSPECTIVE-2026-07-19.md` — the project wrap: architecture verdict,
  seam-deviation history, error classes, refactor shortlist.
- `DEVIATIONS.md` — the canonical register of deliberate departures from
  retail (code sites marked `(deliberate)`). Check BEFORE "fixing" toward
  retail.
- `FIDELITY.md` — fail-open fidelity gaps and approximations still owed.
- `FORMAT.md` — the baked bundle/level format spec (lockstep with code).
- `archive/DESIGN-SAVES.md` — save/load and the in-game menu: retail
  findings and the settled rulings. IMPLEMENTED; kept for the rulings and
  the reasoning behind them, which the code comments cite.
- `traces/`, `spell-audit/` — the decompile research bank; cited by code
  comments; consult before re-porting anything.
- `archive/` — completed working documents (surveys, reviews, audits,
  playtest banks, the full ledger). Open items were extracted into this
  file; the archive is history only.

## Done (the arc, in one screen)

- MC1 core port: sim, terrain, spells, mobs, rivals, HUD/map, GM music —
  player-certified across many playtest rounds.
- Multi-game architecture (2026-07-09): ONE superset sim, five-tier
  divergence taxonomy, VerbSet wiring, ChassisParams, state-hash goldens.
  Kill criterion passed decisively; see the retrospective.
- MC2 full port (07-09 → 07-18): roster + multipart + class-9 flyers, spell
  column with XP, castle column, cave column, stage/objective engine (all
  shipped types), rivals, doomsday endgame, campaign stitching, menus + map
  overlay, audio column (music, narration, SFX policy). 165 levels load
  with a 100.0% THING census; campaign playtested through the finale.
- Hidden Worlds: core delta landed (chassis shared wholesale, one verb arm,
  spells-table data delta).
- Presentation: enhanced renderer (fog/sky/reflections/dynamic lights),
  smooth motion, shore/fire/lightning effect tracks, options registry.
- 2026-07-19: comment sweep (histology out, `DEVIATIONS.md` born), docs
  restructure, retrospective.

## Remaining work

### Player reports 2026-07-21 (round 2) — ALL FIXED + PLAYER-CONFIRMED

All three closed and confirmed by the player in-game the same session.
Every one turned out to be a TRANSCRIPTION defect, not a design gap.

**1. MC1 castle-as-weapon took HALVED damage — FIXED.**
- Root cause: `spreader_tick` (the `(10,1)` corpse flame, retail
  `sub_25130` :28142-58) ran its fire-ring spawn ONCE. Retail runs it on
  EVERY tick of the puff's life, and the puff's life is 1, so retail
  spawns the ring TWICE. Two independent off-by-ones caused this:
  (a) retail's life test reads the PRE-decrement value, ours read
  post-decrement; (b) retail's `& 2` latch guards ONLY the one-shot
  sound (`sub_55370_558A0(.., -1, 3)`), while our port had hoisted the
  whole body under it and returned early.
- Every creature death in MC1 was therefore delivering half its fire
  damage — the castle crush was just where the player could see it.
- MEASURED on a 17-part worm crushed under a fresh level-1 castle:
  **10,400 before → 20,400 after**, against a 20,000 ladder. That is
  retail's reported "destroys the castle outright, or leaves the bar at
  0 so any scratch finishes it", to the unit.
- RULED OUT and documented so it is not re-opened: the ~50% per-cell
  spawn gate `2 * (rand % 0x9D / 79) - 1 > 0` is FAITHFUL. `rand` is a
  self-contained LCG (`9377*s + 9439`), the idiom appears 16× in remc1
  as the engine's RandomSign, and remc2's INDEPENDENT decompile of a
  DIFFERENT binary has the identical gate in the identical loop
  (`engine/EventsFunctions.cpp:22793`). The numeric fit that made it
  look guilty (51 × 400 = 20,400) is explained by the two-pass law with
  the gate intact. Intake, ring size, part count, one-shot latch and
  mail accumulation were all verified faithful and lossless.

**2. MC1 militia never descended — FIXED (one wrong byte).**
- Root cause: the m4 constructor pointed at BEHAVIOR row 0 instead of
  row 16. remc1's `sub_386DE` could not resolve the row symbol and
  substituted `unk_98F38[0]`; the unresolved declaration survives in the
  file, commented out as `//int unk_99138;//fix` (:44891) directly above
  that constructor, and `unk_99138` (:5328) self-identifies as `0x0010`
  = row 16. Every other single-body ctor maps model n → row 12+n
  (12,13,14,15,**0**,17,…), and row 16 is referenced by NO ctor anywhere.
- Row 0 is the FLYER row (`v_14=-4`, `v_20=0xFFFFFFFF`); row 16 is the
  ground-walker row (`v_14=-128`, `v_20=0xFFF080FE`).
- This ALSO closed the separate "archers walk out over the sea like a
  flyer" report: row 16's terrain mask excludes water, and its descent
  gives ground-glue WITH the flying leeway the player described.
- The previous "arithmetically faithful GIVEN the roam" analysis was
  right about the function BODY and wrong about the question.

**3. mc1:000 "extraneous tower" — NOT a spawn bug; m12 settler fixed.**
- The building is settler-built ~44 ticks in (reads as "at init" from
  the cockpit), not a THING row. The player re-checked and confirmed
  retail builds there too — the ORIGINAL premise was withdrawn. No
  load-path admission test differs; the whole THING chain is faithful.
- But three genuine transcription defects were found in the m12 chain
  and fixed, and the player then confirmed the settlers now travel to
  and build at the shore location retail uses:
  - `m12_wander` (`sub_1EED0` :25077-84): pre-decrement `+26` test —
    retail spends THREE wander think-ticks from the ctor's 2, we spent
    two, leaving our `ent_rand` phase 2 draws ahead at BUILD.
  - `m12_approach` (`sub_1F120` :25165): C precedence makes the think
    gate `(f63 % v_26) / 2`, not `f63 % (v_26 / 2)`.
  - `m12_approach` (:25168-70): the same pre-decrement `+26` test.
- STILL OPEN (banked, needs care): our `m12_approach` returns early on
  patience-out and does a top-of-function target-validity check; retail
  has neither — its validity test lives INSIDE the think gate and it
  FALLS THROUGH, so it can still promote to BUILD the same tick.

**SYSTEMATIC LEAD — the pre/post-decrement class.** Three of the five
defects above are the same shape: retail does
`v = field; field = v - 1; if (v <op>)`, testing the PRE-decrement
value, and our port wrote `field -= 1; if (field <op>)`. Confirmed
present in `sub_25130` (corpse flame), `sub_1EED0`/`sub_1F120` (settler)
and — UNFIXED — `sub_25CE0` (`blast_ring_tick`, :28684-86), which
therefore runs one ring pass fewer than retail. A sweep of every
per-tick life decrement in the effects/creature families is owed; for a
short-lived entity this halves its whole output, which is exactly how
the castle bug hid.

### Saves + in-game menu — LANDED 2026-07-21 (playtest owed)
- Mid-level save/load and the pause mini-menu, per `archive/DESIGN-SAVES.md`
  (which now records status, deviations and the remaining open item).
- Sim payload codec `mgc_sim::snapshot` — dependency-free, hand-written,
  exhaustive destructure out / exhaustive struct literal back, so a new
  field is a compile error in both directions. Restore APPLIES onto an
  already-built world (the level package supplies `Gen::assets`/`retile`
  and the `&'static` chassis slice); an identity fingerprint refuses a
  foreign world before writing anything.
- `.mgcs` container (`mgc_formats::mgcs`): ZIP + `save.json` header +
  `campaign.bin` + `snapshot.bin`, DEFLATEd (unlike `.mgcl`, which stays
  Stored for its committed hashes). Header alone drives the slot list.
- Slot model: `<stem>.mgcs` native + `<stem>.gam` retail export beside it;
  native always wins, the `.gam` is read only when no native file exists.
- **OPEN — mid-level option gating.** `entity_pool_size` (and anything else
  that resizes the pool) still has no "mid-level changeable" axis in the
  settings registry, so it can be changed from the in-level Options layer.
  The snapshot identity check turns the result into a REFUSED load rather
  than a corrupt one, but the option should grey out instead.
- UI round 1 (player review) landed: the panel's own "PAUSED" title
  dropped (the retail banner stays — banner = state, panel = menu),
  results moved to the toast line (they overflowed a
  narrow panel), cursor stays free for the whole pause (`set_grab` refuses
  to re-grab while paused — closing the big map was re-capturing it), Esc
  from Options returns to the mini-menu instead of unpausing, the two
  panels are mutually exclusive on screen, panel background darkened for
  contrast over sky and desert.
- UI round 2 (player review): loading is now decided by the SLOT, not by
  where you loaded from — a mid-level slot resumes into its level from the
  main menu / world map too (it used to adopt the record and leave you on
  the menu, so entering replayed the level from the start), and a
  campaign-only slot loaded in-level exits to the hub. Frontend slot lists
  route through `saves::scan_slot` and show `L<n>` on a resuming slot.
  Slot-row text is letters/digits/spaces/`%` ONLY: the messaging font is
  the game's FONT1 bank at `glyph = byte + 1`, so `*` drew as a lightning
  flash and an em dash drew as three junk glyphs.
- UI round 3: EVERY slot names its level (`L3`), and a resuming slot adds
  the mana percentage the run had reached (`L3 15%`) — one shape, the
  suffix says which, and the number doubles as a how-far-in marker.
  `level` was promoted onto the save header (both kinds of save carry one,
  and one copy cannot disagree with itself); `InLevel` gained `mana_pct`
  and lost its duplicate `index`. `SAVE_VERSION` 1 -> 2.
- Rebase hazard, seen once already: the exhaustive destructure makes any
  commit that adds a `Gen`/`World`/`Player`/rival field FAIL THE BUILD
  until the field is added to the codec (`Gen::pal_flash` from the
  purple-flash commit did exactly this). That is the design working. Judge
  separately whether the addition also needs a `SNAPSHOT_VERSION` bump: it
  does whenever the byte layout shifts, which is essentially always,
  because the identity fingerprint is written AHEAD of the payload and so
  cannot catch the misalignment.
- The mini-menu is TEXT rows, not the icon set `archive/DESIGN-SAVES.md` ruling 7
  anticipated: text carries the label, level and progress that icons
  cannot, in a panel narrow enough to leave the HUD and the map's live
  view usable. No `assets/static/` art is owed unless the panel grows an
  icon row.
- Version gates now read the version through a minimal PROBE struct before
  deserializing the rest. A bump is precisely when the schema changed
  shape, so a full parse fails on an unrelated field and buries the
  explanation (v1 saves reported "invalid type: map, expected u32"). Same
  law on the payload side. Regression test carries a verbatim v1 header.
- Cross-version SALVAGE: a `.mgcs` this build cannot apply still gives up
  its campaign record (`mgcs::recover`) — that record is RETAIL's byte
  layout, so it survives any version of ours; only the resume, whose field
  order is `SNAPSHOT_VERSION`'s, is lost. Such a slot lists amber + `old`
  (`SlotInfo::stale`) so the loss is visible before it bites; re-saving
  heals it. Verified against real v1 saves.
- Per-slot save NAMING removed (all three frontends). Every editor seeded
  itself from the RENDERED slot row and wrote it back as the name, so the
  `L<n> <pct>%` suffix accumulated on each save. Slot names are now derived
  — stored label = player name, level/progress composed at draw time — and
  `SaveTo` carries no label. The `SetName` dialogs (player name) stay and
  are the only writers of a stored label.
- Playtest round 1 fixes: MC2 save rows read 0% because the figure came
  from `Player::banked` — the CASTLE panel's numerator (`(10,45)` houses +
  `(3,2)` castle stored), which stays 0 under MC2 until a castle stands.
  Now `World::player_mana_share_pct`: what the player POSSESSES, minus the
  intrinsic 1000 every wizard is born with (so a fresh level reads 0%) and
  clamped, because MC2 seeds its world total at 1 rather than at that base.
- Also fixed (PRE-EXISTING, surfaced by loading): `install_level` cut sfx
  and speech only when an outgoing session existed, so a launch FROM a
  frontend never cut the world map's narration of the upcoming level and it
  played on over the level. Now unconditional, matching retail
  (remc1 :59992-94) and the observed behaviour that entering early cuts the
  map line and the level plays its own, different narration.
- **PLAYTEST OWED**: mini-menu placement against both HUDs and both map
  screens (`minimenu::{MARGIN, TOP, WIDTH}` are the dial); the load
  round trip in all four menu/level x resume/hub combinations; the MC2
  tier-name fix from prereq 3.

### Player reports 2026-07-21 — FIXED 2026-07-21 (playtest owed)
- **FIXED 2026-07-21 (round 2) — MC1 militia/"archers" roam unbounded
  and hover over water.** Same single root cause as the "never descend"
  report: the m4 ctor used BEHAVIOR row 0 (the FLYER row, terrain mask
  `0xFFFFFFFF`) instead of row 16 (ground-walker, mask `0xFFF080FE`,
  which excludes water). See the round-2 entry above. The analysis
  below is kept because its REPRODUCTION is still the right probe, but
  its "faithful given the roam" conclusion is superseded.
  Player report (mc1:02, level-independent): an archer walking
  perfectly horizontally out over the sea, "like a flyer". Player is
  certain retail militia never do this — they get flying LEEWAY (so a
  collapsing building can pop them into the air without killing them)
  but stay near their building. Most creatures are visibly glued to
  the ground.
  - **REPRODUCED headlessly** (probe, not kept): an m4 spawned on
    height-22 coastal land walks off the coast and is left 713 engine
    units above the seabed, shedding 1 unit/tick — ~700 ticks of
    horizontal flight. It then crossed 100+ tiles and the map seam.
  - The hover is arithmetically faithful GIVEN the roam: in-band
    descent is 25% of `v_14` = 1/tick, and the militiaman crosses a
    tile in 8.5 ticks, so it can shed only ~0.27 height units per
    tile. On ordinary slopes that lag is sub-height-unit (invisible =
    "glued"); at a coastline it is 20+ height units at once.
  - **RULED OUT — all verified byte-exact vs the decompile**: the
    altitude clamp (`sub_42000`, and `sub_196E0` calls it, NOT the
    water-aware `sub_42090`); the ground reference (`sub_11F50` →
    `sub_724C0`, bilinear ×32); the move permit (`sub_11640` mode 1)
    and behavior row 0 (`v_20 = 0xFFFFFFFF` really does permit water);
    the roughness probe (`sub_19650`); the position commit
    (`sub_41C70`, pure list maintenance, no ground snap); the m4 ctor
    (`sub_386DE`, speed 30); the wander draws; the acquisition ladder;
    and the runtime dispatch loop (`sub_41780_41AC0` walks all 1000
    slots every step — NO per-entity stagger, hypothesis refuted).
  - **RULED OUT — the house leash.** `sub_1B5D0`'s steer-home branch
    keys on `+146` holding a (10,45) house, but every writer of `+146`
    on the m4 path is gated on `class == 3`, and the ladder explicitly
    EXCLUDES (10,45). Dead code for model 4. The two lists it scans
    are `+36462` = wizards and `+36418` = `str_36382x[9]` = the m9
    burrowers — neither can yield a house.
  - **So nothing in the transcribed decompile bounds the roam.** Every
    piece verified faithful while the aggregate is visibly wrong —
    which in remc1 has twice meant incomplete transcription (the
    truncated class-9 state table; `sub_41C70` "SYNCHRONIZED" with a
    missing body) rather than a mis-port.
  - NEXT (measurements, not theories): (1) compare our militia
    POPULATION over time against retail's emit law — if villages
    over-produce, the rare wanderer becomes a common sight, which fits
    "archers are generally not constrained anymore" better than any
    single-creature explanation; (2) check for an m4 lifetime/despawn
    we have dropped. Player is gathering more data.
- **OPEN — MC1 tick rate is an unverified ESTIMATE.** Retail advances
  the sim once per RENDERED FRAME (`DrawAndEventsInGame` :41672; the
  F3 speeds are 4×/16× of it, which our `game_speed` models
  correctly). MC1 ran uncapped and hardware-bound, so `TICK_RATE_HZ =
  24` is borrowed from MC2's 24 FPS limiter (documented as such in
  mgc-sim/src/lib.rs). This scales every MC1 motion in wall-clock
  terms. Cannot by itself explain the militia roam (a wrong constant
  makes the ocean crossing sooner or later, never impossible).
- **OPEN — two PROVEN MC1 militia deviations** (independent of the
  roam, both from the `sub_1B5D0` trace): (1) our idle ladder has a
  port-invented "nearest building within 0x1000 → walk in" step and
  routes it through `mob_death` — retail's idle has NO house step at
  all (its walk-in lives in the dead leash branch and is a SILENT
  absorb), so ours leaves militia corpses at houses; (2) militia aggro
  reads a single global `player_aggro` flag instead of retail's
  per-wizard `+528` wanted timer, so MC1 RIVAL wizards never draw
  militia fire. Also minor: retail's idle zeroes `+26` every tick
  (:22482), ours does not.
- **The castle-transformation kill was too weak in BOTH columns**
  (player report: "castle building works as a destruction spell, but a
  lot less than it should"). The mechanism is NOT a movement lockup —
  it is an explicit model-keyed execution over the footprint, and
  immunity is by MODEL, not by flight (hence wyvern/griffon immune,
  dragon/worm/bee/vulture not).
  - **MC1: the lethal area was under 40% of retail's.** `sub_40E20`
    fires for EVERY cell of every positive RLE run, over rows
    1..=level, BEFORE the cell byte is read (:30634 precedes :30635) —
    an EMPTY footprint cell kills exactly like a masonry one. Our
    `build_footprint_kill` gated on `byte != 0`, shrinking a level-7
    castle's sweep from 2304 tiles to 899, and swept only the top row.
    Both fixed; test `castle_kill_sweeps_empty_footprint_cells_too`.
    MC1's exemption list {6, 8, 16} = Kraken/Griffon/Wyvern and the
    owner-spare were already faithful.
  - **MC2: the castle path never purged at all** (below).

### Player reports 2026-07-21 — FIXED 2026-07-21 (playtest owed)
- **MC2 castles were not lethal to what they rise over.** Retail's
  (10,42) castle painter runs `sub_57390` over EVERY cell of the
  cumulative footprint on EVERY tick of the 19-tick rise (EF:27826-27),
  gated on the painter's kill bit (`byte[2] & 1`) which only the
  level-UP spawn sets (`sub_60480` EF:61602) — never the damage repaint
  (`sub_5FBD0`). Our MC2 painter never purged at all, so castles only
  killed incidentally (creatures the terrain lift happened to strand);
  the player's report was MC2 fireflies (model 19, unprotected)
  surviving builds they should not. MC1's column already had the arm
  (`build_footprint_kill` under `+18 & 1`, :56492). Now ported as
  `F_BUILD_KILL`. Also fixed in `mc2_building_clear_tile`: the skip
  test is retail's OWNER compare (`victim.id24 != owner`), not a slot
  compare — a wizard's own creatures walk through their own
  construction — and the victim's killer/attacker pair
  (`word_0x24_36`/`+38`) now credits the builder. The slot compare was
  indistinguishable on the village path (an unowned building's `id24`
  defaults to its own slot) but wrong for an owned castle. Test
  `a_rising_castle_executes_what_stands_under_it`.
- **Destroying a castle left a flagless "tower" standing** (both games,
  site-dependent; player-confirmed fixed on the mc2:06 ocean site,
  rival Belix). The remnant was never an entity — the (3,2) castle
  entity CARRIES the flag and despawned correctly; what stood was
  painted TERRAIN. Three independent causes, all now closed:
  1. **MC2 datum was the corner MEAN, retail's is the perimeter MIN**
     (`sub_4AA40` EF:33399 → `sub_48E60`/`sub_48F20`, init 250). The
     stamp writes `datum + cell` absolutely, the demolish only
     subtracts `cell` back, and nothing saves the original ground —
     the min datum is exactly what makes that asymmetry land flush.
     The mean sat above the low side of any slope and left
     `mean - ground` of stone mesa. Flat sites hid it → the
     site-dependence. `mc2_castle_site_z` now uses the existing
     verbatim `mc2_perimeter_min`. Test
     `a_castle_on_a_slope_leaves_no_mesa_behind` (18-unit mesa before,
     0 after).
  2. **Level-0 castles stamped BUILD row 1.** Retail's build row IS
     the level, unclamped, and row 0 is EMPTY (w = h = 0) — a level-0
     castle is a bare flag owning no terrain, which is why the destroy
     path never un-stamps it. Both columns clamped the row up to 1
     (MC2 `mc2_spawn_castle_painter`; MC1 `spawn_starting_castle`
     passing `lvl + 1`, plus the painter/repaint clamps), raising a
     tower nothing would ever remove. MC1's demolish also lacked
     retail's `if (level > 0)` guard (:56506), so a level-0 death
     demolished a row-1 footprint that was never built. Test
     `an_authored_castle_owns_only_its_own_levels_terrain`.
  3. **MC1's un-stamp could silently not run at all.** Retail builds
     the fake collapse event in the SCRATCH slot (entity 0, :56517-24)
     and never allocates; ours took a pool slot with no else-arm,
     right after `castle_eject` can spend up to 36 — on a
     pool-pressured level the whole demolish was skipped and the full
     tower stayed with its flag gone. Now uses `SCRATCH`.
  - Level 005's goldens re-pinned (authored rival castles stamp one
    ring less at load): all six layout hashes move, the OBSERVABLE
    projection moves at post-init ONLY and holds A-E — the evidence
    that footprints changed and play did not.
  - NOT a bug, deliberate: the leftover flatten pad itself. Retail has
    ONE heightmap, no backup, and the demolish is a relative
    subtraction — a destroyed castle genuinely leaves its levelled pad
    (plus up to +19 of byte-wrapping LCG rubble jitter, faithful in
    both columns). Only the EXCESS above the datum was ours.
- **MC1 Global Death had no player-visible effect.** Retail's only
  sighting of the spell is a full-screen palette flash at the
  detonation — `sub_44BE0(owner, 3)` → `Type_160+152`, painted by the
  frame tail (:41813 case 3: red +48, blue saturated, green untouched
  = a violet wash, then the case-1 `FadeInOut(pal, 4, 1)` ramp home).
  The field handler had it commented as OPEN/unported. Now ported as
  `PalFlash` (Gen, hash-silent presentation channel) → `PlayerVitals
  .pal_flash` → the ui.rs overlay, armed only when the field's owner is
  the local player (retail gates on the slot compare). Test asserts the
  row-3 arm inside `global_death_fuses_at_the_caster_into_the_flat_plane_field`.
- OPEN, same channel: **row 6** (`sub_44BE0(v4, 6)` at :29215 — the
  warm R+48/G+32/B+32 wash when a creature lands a charge on the
  player) is still unported; the `PalFlash` channel is now there to
  carry it. Rows 2 and 7 are already drawn (hit flash, death grey-out).

### Player reports 2026-07-20 — FIXED 2026-07-20 (playtest owed)
- **Collapse-evacuee militia FLOAT** (level 04 "floating archers"):
  fixed in the dormant-arm BONUS below (restored the militia movement
  core + wander).
- **Player-death camera slid along the terrain** instead of pinning at
  the corpse. The dead-state handler (mgc-sim/lib.rs) zeroed only the
  MC1/MC2 carpet speeds; under the Enhanced mover the camera rides the
  float velocity (`flyer.v*`), which kept drifting. Fix = zero BOTH the
  carpet speeds AND the enhanced float velocity BEFORE the move whenever
  dead (a true pin); FALLING keeps its faithful glide; the existing
  turn-toward-killer (killer_pos → yaw) completes the retail behavior.
  Test `dead_wizard_pins_at_the_grave_under_enhanced`.

### Player reports 2026-07-19 — FIXED 2026-07-19
- Rival castle sink PLAYER-CERTIFIED; firefly damage accepted as
  FAITHFUL (byte-verified; the opt-in "stronger firefly" lever would
  be the `f63 % 35` shooter throttle). Playtest still owed on: worm
  possession color, thrust-model switch hand-off, and the
  enhanced-thrust×Faithful-altitude vertical (decline-crosstalk
  ruling: the altitude AXIS owns vertical on both thrust models —
  see DEVIATIONS "move_enhanced (level-plane thrust)").
- MC2 big map coverage FIXED 2026-07-19 (playtest owed): map-screen
  pane now spans the faithful 318.75 tiles vertically (retail
  DrawMinimap scaling 204, EF:21840-49) instead of the bare 256-tile
  world; retail's 4.6% terrain-vs-entity horizontal misalignment
  deliberately not reproduced (DEVIATIONS mgc-render).

### Player reports 2026-07-19 round 2 (traced 2026-07-19)
- **MC1 level 04 trigger altitude-gated — TRACED FAITHFUL, no fix.**
  The retail probe is the same 3-axis AABB (sub_118C0 :16963 has a z
  arm; the 2-D suspicion is refuted — :58490 is unrelated UI code):
  class-11 volumes get authored horizontal extents but a FIXED 4096
  vertical half-extent (:44038 → sub_37130 :43790), probe the
  sprite-44 wizard at flight altitude, and resnap z to the CURRENT
  (dug) ground on quiet probes (:67632). Retail's speed-0 sink is
  the same 8 units/tick (:55171). RESIDUAL suspect if the player
  still finds it worse than retail: our hole dug DEEPER than retail
  at the trigger cell (terrain bake / crater depth — the same
  memimage-compare class as the MC2 foundation angle-nibble check).
- **MC1 level 04 trigger-spawned skeletons passive — FIXED
  2026-07-19, TWO stacked gaps** (playtest owed). Skeleton = the
  (5,9) burrower mound. Gap 1: the state-55 handler `m9_hidden` was
  missing retail sub_1D060's awake-gated WIZARD scan (:23796-23833)
  — the only path that ever targets the player from state 55. Gap 2
  (the player-visible one — reported again post-fix-1 as "attack my
  castle but never me"): mounds bury 400 ticks after emerging with
  no player near, and the buried arm was a stub with NO way back up
  — retail sub_1D6D0 (:24016-28) arms a −50 countdown when the
  wizard enters the 24-tile wake gate and the mound RISES again
  (sub_1DDB0 :24273); the level-04 army had buried itself before
  the player ever arrived (the castle worked because building it
  nearby kept nearby mounds awake → never buried → unbounded-radius
  castle hunt). Both ported; tests
  `m9_mound_scans_the_wizard_when_awake` +
  `m9_buried_mound_rises_near_the_wizard`; live-level verify:
  dis-2 army buries player-far, then 16 risers chasing within 400
  ticks of hovering it. Goldens unmoved. STILL OPEN (AI track):
  the roam/convert self-spawn (surfaced :23834-23920 tile-dis-3
  gated, buried :24030-118 unconditional owner stamp — the
  undead-army growth that consumes villagers/creatures within
  0x600 and mints new (5,9)s).

### MC2 fidelity debts
- `mc2_seed_default_spells` unconditionally seeds `{0,1}` at EVERY level
  init — a floor retail does not have. Spells are HOARDED across levels
  in both games; retail's `InitialiseSpells_54A50` (EF:38721-62) grants
  the carried book alone on campaign levels > 0, and falls back to the
  level's authored `starting_spells` row only when there is no carry
  (level 0, or a direct `--level N` = `LEVEL_LOADED_FROM_ARG`). Two
  consequences: (a) a spell permanently lost to the wraith steal would be
  handed back by us and not by retail; (b) direct `--level N` playtest
  launches should get the authored row (8 spells on mc2:003) instead of
  2 — the row is imported and in the bundle but consumed only by rivals.
  The campaign CARRY itself is already correct (`apply_campaign_book`),
  as is the HAND binding (`mc2_rebind_hands_canonical`).
- Jar re-collect / double-manifestation side bug — mask desync lets a
  carried spell's jar re-collect; root fix = set the SpellEnabled mask bit
  in `mc2_adopt_manifestation` (hashed path — needs golden verification).
- Human MC2 death does not scatter its spellbook — `mc2_scatter_spells`
  (cast.rs) is uncalled; wire into the human-death path + re-mint from a
  `known` mask on respawn.
- Rivals: DEFENSE disguise VISUAL unported (the state machine, tier
  pick, shadowing and speed law ARE faithful — sub_15FC0/sub_161A0);
  heal rate = MC1 stand-in; per-projectile hate-feed timing; creatures
  aggro only the human. (Scroll-grab cast IS wired at rivals.rs:1968;
  steal-mana is absent from retail's rival rotation — neither a gap.)
  Rival-spell tail: Duel tether, Beyond-Sight T2,
  rival rebound-window mirror.
- Model helpers: doomsday (5,10) x41/6 helpers; m22 worm head + link-length
  provenance (floor 96 APPROX); (10,76) fire-sphere own creator; (10,9)
  dome geometry helpers; (10,54) magnet ball-pull pending (9,17) chain;
  misfit class-11 models 5..=11 switch handlers (ids.rs).
- WATCH: (10,83) dome LOAD anchor corner-vs-center (load path may sit a
  tile off; cave levels certified, so subtle if wrong).
- Stage engine: objective type 4 (escort) unported — 0 shipped levels,
  completeness only; type 6 stays `_ => false` (dead in retail);
  per-model phase-7 held-wrapper ambient tails (sound rolls, ground
  re-snap) partly swept; kind-3/4/5 `&2` handle-tracking branches dormant.
- Class-5 stale-slot deaths — banked root fix = hash-excluded per-slot
  generation counter (REVIEW PR-3 class).
- Global Death 0x12/0x13 m18+m19 homing reconstruction banked.
  (Type-68 `sub_21F60` line removed 2026-07-20 — audit found it ported
  as the doomsday devour pass, doomsday.rs:466.)
- Metamorph carpet-hide + spell-name level-up banner (presentation).

### MC1 fidelity debts (the INTERIM inventory)
- Per-spell emission approximations: earthquake m11 digger (vs c10 m15
  crevice walker), undead army 3 fixed skeletons, lightning-storm 8-bolt
  fan, wall-of-fire 5 standing fires, mana-magnet 30-tick puller,
  steal-mana wizard-only.
- Castle housekeeping cluster: balloons/levels/respawn, overflow ejector,
  castle HP/damage/downgrade, per-level win threshold `byte_38C93`, m42
  painter delta-array. (The deferred mana-collection/castle economy home.)
- Spell-audit gaps: Possession tier→(10,54) magnet-child link MISSING
  (tier-1 attracts no mana); Lightning `sub_66FD0` L1/L2 burst unported;
  spell 19 blocked on (10,72); placed Magic-Mine variant (spell 23);
  meteor charge-tiered fuse (proj.rs TODO); quake subtype-23 wrapper
  unverified; mana-regen mid-burst suppression branch; steal-mana
  wizard-gate decision; fools-mana OPEN-1/2; base-MC1 spell-20 multi-bolt
  spray.
- Starting-spell level-file source undecoded (1042-byte reserved block
  @0x30); jar spell-id-from-model65 unverified; blue-seed cross-level
  carry (var_916) untraced.
- MC1 song-command source untraced (runtime song = level%3 interim); FM
  renderer ignores velocity/pitch-bend (accepted interim).
- Genies MANA STEAL unverified (the mobs.rs mana-track concern).
- Shift+K wizard suicide parked; GEN_MAP pre-header semantics open.

### Hidden Worlds
- PLAYTEST OWED — spell-20 homing/rebalance + napalm fork uncertified.
- TMAPS-156 arctic tree: blank vs neighbor-155 pixels — trace before
  touching the frame-less skip.
- Spell-20 visuals trace + `mc1-arctic` bitmap verify; napalm↔spell-20
  relationship trace; world-map grid 20→10 (optional UI).
- Mana-shield model-53 reflect gate (latent until wizard shields ship).

### App / frontend
- MC1 menu click samples (snds13 bake member + per-mode SFX bank switch).
- Inert menu screens: Multiplayer / SetKeys / Language / Joystick (both
  games' equivalents).
- Post-finale MC2 map refuses resume without `--new-game`; trail-stamp
  gate + editor (14,3)→(11,12) marker remap untraced.
- MC2 map 4-button edge overlay (save/load/next/exit); retail right-click
  replay nuance.
- WATCH: temple hover sprite polarity may be inverted; langindexbuffer[2]
  byte-verify; OK2/CANCEL2 pressed-state sprites.
- Non-4:3 aspect handling.

### Render
- TMAPS textured fullscreen map (the "green look" for MC1's book map).
- SKY bitmap cloud-plane bake + the 50-slot night/cave dynamic-light
  system; `hmap2` water-reflection plane.
- Camera ROLL render term (faithful bank), ending blur/fov-dolly, palette
  flashes; MC2 map entity billboards (needs MC2 sprite bake).
- Tremor SHAKING presentation (trace what retail drives it with).

### Audio
- REVIEW audio column: D1 per-id request modes vs remc2 dispatch (ids
  <47); D2 (owner,id) channel key — VERIFY FIRST, likely already landed
  (stale checkbox); D3 missing cue sheet must not drop sounds/music; D4
  stale bank-1 docs; D5 war-stem cc11 expression curve; D6 remaining hard
  cuts; D7 misc cleanups.
- Type-31 beacon speech variant (secret rows unreachable until wired);
  speech-onset palette flash; INTRO/CUTS sub-songs unbaked; F-section FM
  render (faithful alternate); unwired MC2 sound sites (m4/m10 …).
- Clean-CD narration reconstruction if an uncorrupted pressing surfaces
  (bake override hook exists; GOG track heads corrupt — see memory/tools).

### Playtests owed
- HW delta; MC2 stage-hold levels (esp. level-014's dormant model-18);
  Nyphur rival engagement (hate-feed timing feel); MC1 24Hz feel
  re-certification; `castle_lock_active` window feel; duel tether with a
  creature nearby.

### Retail checks owed
- Castle Shift+L self-destruct depth; worm-vs-castle fire-cell magnitude;
  meteor/castle blast-site tracking; retail level-039 fail-open look; MC2
  spell-carry start-row hypothesis; the AI-asymmetry register.

### Banked opt-ins (enhancement/alternate features)
- Torso-aim enhanced aiming (intended eventual enhanced default);
  predictive autoaim closure (no-mutation aim_assist variant).
- Exclude creatures from pyramid damage; slot-16 summon-corpse death
  animation; legible map markers; MC2 XMI/AIL no-CD faithful-alternate
  arrangement.

### Refactors (see retrospective §4)
- LANDED 2026-07-19 (goldens unmoved): S1 tick()/spawn_from_thing() arm
  extraction, S2 shared engine → `mgc_sim::engine::{world, features}`,
  S3 live_poses() per-game split, A1 `CampaignSave` enum, A2
  GM-normalize dedupe.
- Deferred: declared per-model dispatch table (only if provably
  order-equivalent); `WizardConfig` enum (with next FORMAT_VERSION
  bump). LATE: game-manual naming reconciliation sweep.

### Later tracks
- FMV/cutscenes (decoders located): intros, MC1 win/lose + outro, MC2
  CUT1-6 + attract mode, PPERF score screen, ScrollDialog unroll.
- Feature-family plugin promotion (authenticity-matrix columns → whole
  swappable families) — design agreed, mostly folded into existing seams.
- Flight feel-tuning pass vs remc2; custom level designs (wyvern-kite,
  portal-maze ideas in the archive ledger).
- FIDELITY.md subsystem write-ups ("entries to come" backlog).
