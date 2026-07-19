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

### MC2 fidelity debts
- Jar re-collect / double-manifestation side bug — mask desync lets a
  carried spell's jar re-collect; root fix = set the SpellEnabled mask bit
  in `mc2_adopt_manifestation` (hashed path — needs golden verification).
- Human MC2 death does not scatter its spellbook — `mc2_scatter_spells`
  (cast.rs) is uncalled; wire into the human-death path + re-mint from a
  `known` mask on respawn.
- Rivals: DEFENSE state body untranscribed (weave+reactive approximated);
  disguise VISUAL unported; steal-mana/cruise-scroll-grab casts unwired;
  heal rate = MC1 stand-in; per-projectile hate-feed timing; creatures
  aggro only the human. Rival-spell tail: Duel tether, Beyond-Sight T2,
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
- Building type 68 projectile-devouring structure (`sub_21F60`) unported;
  Global Death 0x12/0x13 m18+m19 homing reconstruction banked.
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

### Refactors (agreed shortlist — see retrospective §4)
- S1 split `World::tick()` / `spawn_from_thing()` per class (pure code
  motion, goldens must not move); S2 rename shared engine out of `mc1::`;
  S3 split `live_poses()` per game; A1 `CampaignRun` enum; A2 GM-normalize
  dedupe. Deferred: per-model dispatch table (after S1); `WizardConfig`
  enum (with next FORMAT_VERSION bump). LATE: game-manual naming
  reconciliation sweep.

### Later tracks
- FMV/cutscenes (decoders located): intros, MC1 win/lose + outro, MC2
  CUT1-6 + attract mode, PPERF score screen, ScrollDialog unroll.
- Feature-family plugin promotion (authenticity-matrix columns → whole
  swappable families) — design agreed, mostly folded into existing seams.
- Flight feel-tuning pass vs remc2; custom level designs (wyvern-kite,
  portal-maze ideas in the archive ledger).
- FIDELITY.md subsystem write-ups ("entries to come" backlog).
