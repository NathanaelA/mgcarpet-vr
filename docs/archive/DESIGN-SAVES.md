# Saves and the in-game menu — design

Living design document for the port's save/load mechanism and the in-level
pause menu that fronts it.

**Status: IMPLEMENTED** (playtest owed). The code is:

- `crates/mgc-sim/src/snapshot.rs` — the sim payload codec, plus `Snap`
  impls beside each type they serialize.
- `crates/mgc-formats/src/mgcs.rs` — the `.mgcs` container.
- `crates/mgc-app/src/saves.rs` — the slot model (native-first, retail
  export alongside).
- `crates/mgc-app/src/minimenu.rs` — the pause mini-menu.
- `App::{save_slot, load_slot, mini_click}` in `crates/mgc-app/src/main.rs`.

Tests: `crates/mgc-sim/tests/snapshot.rs` (acceptance, both games),
`engine::world::tests::snapshot_*` (pool internals the digests cannot see),
`mgcs::tests::*` (container), `saves::tests::*` (slot precedence).

The "Deviations from this document" section at the end records where the
implementation knowingly departs from the sketch above it.

Retail research backing every claim here lives in
`traces/mc1-campaign-save-menu.md` §B and `traces/mc2-campaign-save-menu.md`
§B (offset maps, call sequences, gating).

## The premise correction

`archive/ROADMAP-2026-07-19-full.md:8939` states "The original games have NO
savestates" and frames in-level saving as a modern convenience. **That is
wrong.** All three retail games ship full mid-level snapshot machinery:

- **MC1 Plus / Hidden Worlds** — `Alt+S` / `Alt+L`, no menu path. ONE global
  slot, hard-coded 199: `save/gam00199.dat` (232,713 B, a single `fwrite` of
  the master game struct) + `map00199.dat` (398,018 B — 4x64 KB planes +
  128 KB `int16` entity index + 4,802 B auto-tile LUT). No magic, no version,
  no checksum, nothing validated.
- **MC2** — pause (`P`) menu Save/Load icons behind an OK/Cancel confirm, no
  hotkey. `SLEV%d/SMAP%d/SVER%d.DAT`, TWO slots: index 0 is the player save,
  index 1 is an automatic level-start checkpoint the engine restores on death.
  Validated by version==15 + campaign level + level ID + exact sizes. The GOG
  install ships slot 2 — the checkpoint, not a player save.

Neither game's mid-level save shares storage with its campaign save. Both are
raw RAM images carrying absolute 32-bit pointers, so **no retail interop is
possible in either direction** for mid-level state; the existing ruling stands.

The two engines dump near-isomorphic structures, and that shape already
matches our `Gen`: terrain planes, per-tile entity index, a ~1000-entry
entity pool, the THING table, and an RNG word.

## Rulings

Decided; recorded so they are not relitigated.

1. **Faithful backend, one shared UI.** All three games get the same save
   mechanism and the same menu. We do not reproduce MC2's icon set for MC2
   and Alt-keys for MC1.
2. **No level-start checkpoint.** Retail MC2's slot 1 exists because
   regenerating a level was slow. Ours is near-instant; the existing restart
   path stays.
3. **No imitation of retail's spell-XP carry.** Retail MC2 deliberately does
   not rewind spell XP or research on load (`sub_549A0`). Ours restores what
   it saved.
4. **Our format is authoritative.** A slot is one native file holding the
   campaign/player state as of level entry plus, optionally, the live
   in-level state. The port reads only this.
5. **Retail-format export is one-way and best-effort.** The retail record is
   written alongside for players who want to carry progress into retail. The
   port never prefers it: a retail/imported file is read only when no native
   file exists for that slot, and is otherwise overwritten. Round-tripping
   back from retail is explicitly not supported.
6. **Slot counts stay 6 (MC1/HW) and 8 (MC2)** — the only reason those
   numbers exist is that the export target has them.
7. **Original menu art.** MC2's icons cannot be reused for MC1/HW: retail art
   is not redistributable and bundles are per-variant. New icons live in
   `assets/static/`, with the MC2 set as visual reference only.
8. **Esc never dismisses the mini-menu**, per the standing law. Unpause is
   the only exit.

## Slot model

Per game, `saves/<tag>/`:

- `<retail-stem>.mgcs` — the native save. Always written.
- `<retail-stem>.gam` — the retail export. Written alongside; never read
  unless no `.mgcs` exists for that slot.

The native file always contains the campaign/player record as of level entry,
so every slot is loadable from the main menu without first entering a level.
It additionally contains the world payload when the save was taken mid-level;
loading such a slot resolves the level, loads it, then applies the payload.
A slot therefore knows on its own whether it resumes at the hub or drops
straight into play.

Lifecycle: mid-level save writes campaign + payload; completing the level
rewrites the slot with campaign state only. No stale payload survives its
level.

### Where a load lands

**A slot's own contents decide, not where you loaded it from.** Both entry
points — the frontend slot pickers and the in-level mini-menu — go through
the same `App::resume_slot`:

| slot holds | loaded from the menu/map | loaded in a level |
|---|---|---|
| a world payload | resumes INTO that level at the saved position | resumes into that level |
| campaign only | stays on the menu/map | leaves the level for the hub |

The two diagonals are the point. Picking a mid-level slot from the main menu
used to adopt its campaign record and leave the player on the menu, so
entering the level replayed it from the start — a load that silently
discarded exactly the state it was meant to preserve. And loading a hub save
from inside a level drops back to the hub, because that is where such a save
was taken; launching some level fresh instead would be a restart wearing a
load's clothes.

### How a slot reads

Every save sits at a level, so every slot names one. A slot that resumes
into that level adds the mana percentage the run had reached — the HUD's own
numerator over its denominator:

- `3 WIZARD  L3` — the campaign parked in front of level 3.
- `3 WIZARD  L3 15%` — a run fifteen percent of the way through level 3.

One shape, and the suffix says which. The percentage doubles as a rough
how-far-in marker, which a bare "in level" flag would not give. `Some(0)` is
a resume at 0% mana, NOT a hub save — a fresh run must not collapse into the
hub shape.

The figure is `World::player_mana_share_pct`: what the player POSSESSES,
world-relative — the HUD's SELF panel. Not `Player::banked`, which is the
CASTLE panel's numerator (`(10,45)` houses + `(3,2)` castle-stored) and so
reads 0 under MC2 until a castle stands, however much has been collected.
It also subtracts the intrinsic 1000 every wizard is born with, so a fresh
level reads 0%, and clamps, because MC2 seeds its world total at 1 rather
than at that base.

The level lives on the header, not inside the resume block: every save
carries one, and a single copy cannot disagree with itself. The frontend
lists and the mini-menu render from the same `saves::SlotInfo`, so the two
never diverge.

**Slot names are DERIVED, never authored.** The stored label is the player
name and nothing else; the level and progress are composed at draw time. Per-slot
naming is gone from all three frontends — MC1's `Modal::EditLabel`, and the
select-to-edit buffer in the MC2 menu and world-map dialogs — and the
`SaveTo` actions carry no label at all.

That is a correctness rule, not a simplification. Every one of those editors
seeded itself from the RENDERED row (`slots[k].0`) and wrote the result back
as the name, so each save appended another `L3 15%` to the stored string.
Naming a save is also just not interesting: the player name plus where they
are says everything the row needs to. The remaining name dialogs
(`SetName`) edit the PLAYER name, which is the one thing worth authoring, and
they are the only writers of a stored label.

Slot-row text is **letters, digits, spaces and `%` only**. The messaging font
is the game's own FONT1 bank addressed by `glyph = byte + 1`, so a byte
renders as its ASCII character only where the bank happens to hold that
character: `*` draws as a lightning flash, and non-ASCII is worse — an em
dash is three bytes and draws as three unrelated glyphs. (`%` is confirmed
good; it appears in other shipped text.)

## Format

House pattern (`FORMAT.md`, `bundle.rs`): a ZIP with a JSON index and raw
little-endian binary members. No new serialisation dependency.

- `save.json` — header and all scalar state. Floats stored as `to_bits` u32;
  decimal text does not round-trip and `Player::speed_boost` is hashed by bit
  pattern.
  Header carries: game, level, bundle variant, `format_version`,
  `bake_epoch`, the level package's `entry_sha256`, `ChassisParams`,
  `VerbSet`, thrust/altitude model, G-class option flags, display name,
  timestamp.
- `campaign.bin` — the retail record verbatim (142 B MC1/HW, 1,319 B MC2),
  including the opaque blobs the native format does not otherwise need. They
  are carried solely so the export stays byte-exact.
- `world/*.bin` — the bulk arrays: terrain planes (plus MC2 ceiling),
  `map_entity`, `ent`, `table`, `free`, `slot_gen`.
- `sim.bin` — the `Simulation` layer (`tick`, `flyer`, `carpet`,
  `carpet_mc2`) and the small app-side set (`quick_binds`, `pane_bound`,
  `spell_levels`, `prev_owned`, the virtual stick).

Excluded and re-supplied from the reloaded bundle: `Gen::assets`,
`Gen::retile`, and `ChassisParams`'s `&'static [u8]`.

Unlike `.mgcl`, saves have no committed-hash pinning requirement, so the
"all members stored uncompressed" rule does not carry over — deflate takes
the ~590 KiB payload well under 100 KiB. `zip` is currently pulled with
`default-features = false`; this needs the deflate feature.

**Version gating:** the container version is read through a minimal probe
struct and checked BEFORE the full header is deserialized. A version bump is
exactly the case where the rest of the schema changed shape, so parsing the
whole header first fails on some unrelated field and buries the one error
that would have explained it — the v1→v2 bump moved `level` from an object to
a number, and an old save reported `invalid type: map, expected u32` instead
of `save version 1, this build knows 2`. Same reasoning applies to the
payload: `SNAPSHOT_VERSION` is checked before any field is applied.

**Salvage across versions.** A container version this build cannot apply does
NOT cost the player their progress. `campaign.bin` is retail's byte layout —
pinned by the decompile and by `Mc1Save`/`Mc2Save`, not by our schema — so it
is readable at any version by construction, and `mgcs::recover` lifts it out
through a probe carrying only shape-stable fields (`level` is deliberately
absent from that probe: its type is exactly what changed). The world payload
is not salvageable, because its field order is `SNAPSHOT_VERSION`'s business.

So a version bump costs the resume and nothing else — and that loss is
SURFACED, never silent: such a slot lists amber with an `old` suffix
(`SlotInfo::stale`), because a slot that quietly stopped resuming reads as
healthy right up until the level restarts under the player. Writing the slot
again heals it.

**Rejection policy:** reject on level-package `entry_sha256` or chassis
mismatch, and surface the slot as incompatible rather than loading it.
Deliberately NOT on `BAKE_EPOCH` alone — the epoch bumps for unrelated
reasons (audio re-renders, UI assets) and gating on it would invalidate every
save for no reason.

`free` order is load-bearing (pool economy) — saved verbatim, never rebuilt
from occupancy. Pool-slot handles are safe only because the whole pool is
saved verbatim; slots are never renumbered.

## Apply path

Reuse the castle-less-death restart at `crates/mgc-app/src/main.rs`: rebuild
world, set both dirty flags, preserve thrust/altitude models across the swap,
reset the flyer, clear `pose_prev`/`pose_cur` (a stale snapshot could
coincidentally pair `(slot, generation)`). Loading is that path with a
payload apply spliced in after `install_level`.

Music restarts from the top; the danger ramp re-converges from
`player_danger` within a second or two. Retail did no better.

## The in-game menu

### Why it changes

Retail MC1 keeps the entire input path live during pause: `sub_17C20`
(`remc1/sub_main.cpp:41667`) is deliberately NOT gated on the pause bit while
the calls immediately above and below it are. Pausing to rearrange spells and
then unpausing into a prepared volley is engineered behaviour, and on MC1's
harder levels it is close to essential. Our full-screen pause menu regressed
it. Restoring it is a fidelity fix, not a UI preference.

### The flow

Pause raises a small always-visible **mini-menu** — save, load, options —
anchored clear of the HUD. While it is up:

- The sim is frozen, but input stays live: spell selection, the big map, and
  the normal HUD interactions all work and none of them dismiss the menu or
  change pause state.
- The mini-menu consumes clicks only within its own rect, so it cannot take
  clicks meant for the spell selector or map overlays.
- **The cursor stays free for the whole pause.** `App::set_grab` refuses to
  re-grab while `paused` — several ordinary paths try to (closing the big
  map is the obvious one), and re-grabbing left the panel clickable with an
  invisible pointer.
- Save and load act as described above. Options opens the existing full
  options menu; **the mini-menu hides while it is up** (two panels at once
  read as clutter, and the options menu is modal anyway). **Esc** closes just
  that layer and returns to the mini-menu, still paused and with the live
  input that implies; **unpause closes both**.
- Unpause is the only way out of the mini-menu itself.
- Results — saved, refused, slot empty — go to the in-game **toast** line,
  not into the panel. The panel is narrow by design and a message long
  enough to be useful ran off the screen edge. The toast is the surface
  built for it, and the one option changes already use. Its timer runs on
  the stopped sim clock, so the message stays up for as long as the player
  is in the menu and decays once play resumes.
- **The retail PAUSED indicator stays** (`ui::pause_quads`) and the panel
  carries no "PAUSED" text of its own: that banner is the pause STATE, the
  panel is the MENU. The slot list keeps a heading ("SAVE TO" / "LOAD FROM")
  because it is the only thing distinguishing two identical lists.

Sound/music icons are dropped — the options menu already owns volume; retail
MC2 only surfaced them because it had no other route.

Saving is offered only from the mini-menu. Paused is an inter-tick boundary
by construction, which is exactly where both retail engines snapshot, so this
needs no extra machinery.

Pause stays disabled in multiplayer, as in both retail games.

## Open decisions

- ~~**Equip latency.**~~ **Already solved before this work started** —
  `App::flush_equip_if_paused` (`main.rs`) applies hand bindings and the MC2
  tier select directly to the world while paused, on the grounds that
  binding is UI state, not simulation. The mini-menu inherits it; no
  UI-only tick is needed.
- ~~**Anchor placement.**~~ **Decided: upper right, INSET.** Not jammed into
  the corner — far enough from the edge to sit clear of the HUD's top strip
  (`minimenu::{MARGIN, TOP, WIDTH}`). The panel is deliberately small: it
  must not cover more than the live-view part of the big (ENTER) map, and
  the MC1 spell selector stays visible under it. **Playtest owed** against
  both HUDs and both map screens; the three constants are the dial.
- **Mid-level option gating — STILL OPEN.** Options changed from the
  mini-menu's Options layer are not yet restricted or recorded. Some cannot
  change mid-level at all: `entity_pool_size` sizes the pool and feeds the
  hash, so changing it is a different world, not a setting — and the
  snapshot's identity fingerprint now REFUSES a slot whose pool size
  differs, which turns this from a silent corruption into a refusal but does
  not yet stop the player from causing it. The settings registry still needs
  a "mid-level changeable" axis; the rest should grey out.

## Deviations from the sketch above

Recorded so the difference reads as deliberate rather than as drift.

1. **One payload member, not `world/*.bin` + `sim.bin`.** `mgc-formats`
   cannot see `mgc-sim`'s types — it is the dependency, not the dependent —
   so splitting the stream there would have put the field layout in two
   places. `mgc_sim::snapshot` emits one versioned stream and the container
   stores it as `snapshot.bin`.
2. **`save.json` holds a header, not "all scalar state".** The ~70 `Gen` and
   `World` scalars ride in the binary stream with everything else, for the
   same reason. The header carries only what the slot LIST needs plus the
   rejection keys, so listing never touches the ~570 KiB payload.
3. **`ChassisParams`/`VerbSet` are identity-CHECKED, not restored.**
   `bucket_excluded_states` is a `&'static [u8]` with nowhere to land, and
   both are fixed at construction anyway. The stream fingerprints them (with
   the pool/table/terrain geometry) and refuses a mismatch before writing a
   single field.
4. **The two slot files coexist** rather than one replacing the other, per
   the doc's own ruling 5 — the memory-banked "writing one deletes the
   other" is superseded. Every slot therefore stays retail-exportable.
5. **The three hash-excluded `Player` latches are now hashed WHEN ARMED**
   rather than left excluded (prerequisite 2 below). Transparent at pristine,
   so no golden moved.
6. **The mini-menu is TEXT, not icons** — ruling 7 anticipated original art
   in `assets/static/`, and none was drawn. Text rows carry what icons
   cannot (the slot label, its level, how far into it the run got) in a
   panel narrow enough to leave the HUD and the map's live view usable,
   which was the stronger constraint. Ruling 7 still stands if the panel
   ever grows an icon row: MC2's art is not reusable.

## Prerequisites

All closed. Kept because the reasoning is load-bearing for the tests.

1. ~~**`Simulation` has zero state-hash coverage**~~ — **LANDED.**
   `Simulation::state_hash` (`crates/mgc-sim/src/lib.rs`) folds
   `World::state_hash` together with the flight tier (`tick`, `flyer` by
   float BIT PATTERN, both carpet structs, `accel_was_active`,
   `terrain_height`, and the two G-class model selectors), behind the same
   exhaustive-destructure discipline. Purely additive — every world golden
   is unmoved. Fixture `crates/mgc-sim/tests/sim_state_hash.rs`: goldens for
   both thrust models plus `hash_sees_*` coverage proofs asserting that
   carpet speed, aim, float velocity, `-0.0` vs `0.0`, the tick counter and
   both selectors each reach the digest.
2. ~~**`Player::{accel_mc2_factor, invis_strength, metamorph}`**~~ —
   **LANDED.** They were hash-excluded, and the recorded reason for
   `accel_mc2_factor` ("the derived `speed_boost` already reflects it") was
   WRONG: `speed_boost` is recomputed from scratch each tick, so a running
   sim only diverges a tick later, and the latch independently gates
   `World::{thrust_cancel, full_stop_cancel_accel}` before `speed_boost` is
   read. At rest — where a snapshot is compared — all three were invisible.
   Now hashed only when armed (the `death_owned_blue` precedent), so every
   golden is unmoved; coverage proof in
   `engine::world::tests::hash_sees_the_mc2_cast_latches_only_when_armed`,
   including that the latch VALUE reaches the digest, not just its armedness.
3. ~~**`App::spell_levels` shadows `Mc2Spellbook::sel`**~~ — **LANDED**, and
   it was a live bug, not only a save hazard. `pane_commit` wrote the
   REQUESTED tier while `mc2_select_spell` clamps to the earned cap and
   rejects an unlearned spell outright; the mirror existed but ran only in
   the pane's DRAW path, so between a commit and the next pane frame the
   displayed tier name and the shift-commit tier came from a value the sim
   had refused. `spell_levels` is now a read-only mirror refreshed in
   `sync_world` ahead of its early returns, and `pane_commit` no longer
   writes it under MC2.

`mgc-sim` has no serde and `World`/`Gen` are not `Clone`. Resolved by
keeping the crate dependency-free and hand-writing the codec: every struct
goes out through an exhaustive DESTRUCTURE and comes back through an
exhaustive struct LITERAL, so a new field breaks the build in both
directions. No `..Default::default()` on the read side — that is the one
construct that would let a forgotten field compile silently. Restore is an
APPLY onto an already-built world, so nothing needs to be `Clone`.

**Acceptance test:** snapshot, restore, assert `state_hash` and
`observable_digest` unchanged, then tick both copies 600 steps and assert
they stay equal — `crates/mgc-sim/tests/snapshot.rs`, on MC1 level-005 and
MC2 level-001 (the MC2 arm wired with stages, so the stage engine and spell
book are actually exercised).

**Two traps the acceptance test alone does not catch**, both covered by
separate assertions in `engine::world::tests`:

- **`SlotGens` hashes to NOTHING** (`features.rs`), so `slot_gen` must be
  asserted directly. Verified by sabotage: dropping it from the codec PASSES
  the `state_hash` round trip. (`free`, by contrast, IS hashed — `Gen`
  derives `Hash` — so the digest does catch that one.)
- **`free.len()` is the wrong probe for "did play disturb the pool".** The
  expanding crater allocates and then frees itself, so the stack ends the
  same SIZE with its top permuted. That is precisely why `free` is saved
  verbatim and never rebuilt from occupancy: a rebuild would match on length
  and on set, and still hand out different slots from then on.

Payload measures 570 KiB (MC1) / 554 KiB (MC2) uncompressed, so the deflate
call was right — a slot lands well under 100 KiB.

## Banked

- **Death legibility.** Castle respawn and level restart are visually
  indistinguishable because our restart is instant where retail's was a
  visible reload. Independent of this feature.
- Retail MC1 reuses its snapshot code verbatim for demo recording
  (`movie/gam%05d.dat`). If a replay system is ever built, the same
  machinery serves it — as it did in retail.
