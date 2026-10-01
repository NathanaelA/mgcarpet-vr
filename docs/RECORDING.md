# The `.mgcr` gameplay recording format

Version 2 — this document is normative. Any tool that reads or writes
`.mgcr` recordings should treat this file as the specification; changes
to the format must land in the same commit as changes to this document.
(Pre-1.0: no backward compatibility is owed to any earlier sample
recordings — player ruling 2026-07-29.)

Format 2 is a strict superset of format 1: it adds the optional
**terrain channel**. Readers accept both; format-1 takes simply carry
no terrain. Writers stamp `"format":2` when (and only when) the header
declares the terrain channel.

## Design goals

One format, three roles:

1. **Conformance ground truth** — a retail playthrough captured tick by
   tick from DOSBox ("what would retail do"), carrying the full raw
   state closure so it can be re-decoded forever as the field maps
   improve. A human has to play these; nothing may be lost at record
   time.
2. **Single-tick fixtures** — every adjacent tick pair (N, N+1) is an
   independent test: initialize the sim from the recorded state at N,
   apply the tick-N input, tick once, diff against N+1. Divergence at
   one tick never invalidates the rest of the run.
3. **Demos** — input-only recordings made by the port, replayed on the
   deterministic sim (`--replay`). Retail precedent: MC1's attract mode
   (`MOVIE/MVI00000.DAT`) is exactly this — an input recording replayed
   on the game's own deterministic engine. Tiny (no state channel);
   verified by the per-tick golden hash.

Channels are optional per recording; the header declares what is
present. Retail recordings verify by **state**; port recordings replay
by **input** and verify by **hash**.

## Container

A `.mgcr` file is a zstd-compressed stream of UTF-8 JSON lines
(inspect with `zstdcat`). Tools also accept the uncompressed `.jsonl`.
Line 1 MUST be the header record. Line 2 MAY be the **init record**
(`"type":"init"`, declared by `channels.init`; see "The init record");
every following line is a tick record, in strictly increasing tick
order.

Writers MUST serialize floats so they round-trip bit-exactly
(serde_json's shortest-round-trip encoding does). 64-bit hashes are
hex **strings** — JSON numbers are doubles and cannot carry a u64.

## The header record

Common fields:

```json
{"type":"header","format":2,
 "game":"mc1|mc1hw|mc2","level":3,
 "source":"retail|port",
 "tick_hz":24,
 "channels":{"input":"exact|raw|none","obs":true,"state":true,"hash":false,
             "terrain":{"planes":["type","height","shading","angle"],"dims":[256,256]}},
 "tool":{"name":"mc_dosbox_recorder","git":"<rev>"},
 "created":"2026-07-29T12:00:00Z"}
```

`channels.terrain` (format 2, optional) declares the measured terrain
planes: `planes` names them in the order they appear in every terrain
blob; `dims` is `[width, height]`, shared by all of them. Absent =
the recording carries no terrain channel.

`channels.init` (optional, default false) declares an init record on
line 2. Readers that do not use it MUST skip it; `mgc_formats::mgcr::
Recording::open` takes it into `Recording::init` before any tick
reader runs.

`source:"retail"` adds: `"build":"A|B"` (CARPET.EXE / HIDDEN.EXE
address half), plus free-form capture provenance (DOSBox version,
cycles). The `capture` object also carries `tear_gate` (bool: emit-time
inter-tick gating ran) and, for a tick-patched exe,
`window_gated: true` with `exe_patch: {mailbox_guest, spin_period_counts,
first_counter, live_counter}` (see "Tick-patched capture") — where each
`t` is the stub's authoritative sub-step counter, relative to
`first_counter` (the raw counter behind t=0). A take with an init
record adds `exe_patch.init_park: {counter, released}` (the stub
counter the park held at — 0 on a fresh process — and `"host"`).

`source:"port"` adds `"sim"`, the **sim-config closure** — everything
that feeds the state hash, pinned so `--replay` can refuse (or
force-apply) a mismatched environment:

- `thrust_model`, `altitude_model` — the flight tiers are **sim
  physics**, not presentation (`Simulation::thrust_model` doc:
  *"fixed per run; replay headers must record them once replays
  exist"*; DEVIATIONS.md "enhanced flight": *"Selected once at the sim
  boundary; replays record it"*).
- `snapshot_version`, pool sizes (the pool size feeds the hash) —
  `entity_pool_size` and `awake_range`, present only when the take was
  recorded with the override. Chassis geometry is decided BEFORE the
  world exists, so the replay has to read them from the header; by the
  time the start snapshot's identity block would catch a mismatch it
  can only refuse.
- `patches` — the retail-bug patch policy (`gameplay · patches`,
  DEVIATIONS.md "Patch options"). `--record` forces every patch to its
  RETAIL arm for the whole session and stamps `"patches": "retail"`;
  `--replay` pins to the recorded policy. A port header WITHOUT the
  key predates the option class (2026-08-08) and replays under the
  legacy hard-wired set (`GameplayPatches::legacy()` — the sim those
  takes were recorded against). Retail-source takes always pin the
  retail arms.
- every sim-reaching option from the options registry, including
  sim-affecting cheats (e.g. `gameplay.cheat.weightless`);
  presentation-only options are excluded and never recorded.
- RNG seed(s) and level/campaign provenance; for mid-level starts an
  embedded start snapshot (`"start_mgcs_b64"`), otherwise the pristine
  level is the tick-0 state.

## Tick records

```json
{"t":N, "input":…, "obs":…, "state":…, "hash":…, "terrain":…, "wallclock":…, "set":…, "terrain_rand":…}
```

`terrain_rand` (retail, from 2026-09-27; optional) is the u16
terrain-painting LCG — MC1 `pseudoRand_12C1E0`, MC2 `rand2_17B4E0`,
the stream the retile/blend passes draw — read with the terrain planes
inside the same capture window. It sits right after the entity index
in both engines' terrain block (+0x60000 / +0x70000).

**Phase convention:** the state-bearing channels (`obs`, `state`,
`hash`) describe the world **at** tick N (t=0 = the initial state);
`input` is the input **consumed by the tick that advances N to N+1**.
Replay therefore reads record N, applies its input, steps once, and
checks against record N+1.

### `input` — the sim-boundary input, per player

- Port (`channels.input:"exact"`): the serialized `FlightInput`
  superset — **both** encodings (the classic virtual stick
  `stick_x`/`stick_y` and the enhanced float axes
  `thrust`/`strafe`/`lift`/`yaw_delta`/`pitch_delta`), casts, equips,
  `full_stop`, and any other sim-reaching verbs. The Rust type is
  normative by reference; its serialization is versioned by `format`.
  Recording both encodings keeps the stream *mechanically* feedable to
  either thrust model (see Cross-model replay below).
- Retail (`channels.input:"raw"`): the persistent externals sampled at
  the tick boundary — held scancodes, mouse cursor and held buttons.
  On MC1/HW they live in the separate static frame; on MC2 they are
  the `ReadGameUserInputs` register block in the struct's own data
  image (held state = the "2" registers @0x18074C/0x18074A, press
  LATCHES @0x180746/0x180744, live cursor @0xE3760, the game's own
  cursor-at-press snapshot @0xE375C, `pressedKeys` ×128 — remc2 named
  VAs; the runtime frame sits at −0xB0E98 from them, anchored on the
  located struct and validated by the control-mode word + keybind
  table; see the recorder's `MC2_BUILDS`). MC2's `ext` adds
  `latch_b64` and `press_b64` to the four standard register blobs.
  This is an **approximation**: the in-struct 10-byte control command
  is consumed and zeroed mid-tick, so a click shorter than one tick
  can be missed or land ±1 tick — MC2's press latch narrows this (it
  is set at the press edge and survives to the release), and the
  press-position pair records the aim the cast actually used. Raw
  input is advisory; retail recordings are validated by state, never
  by replaying input. (MC2 takes recorded before 2026-07-30 predate
  the MC2 register map and carry `input:"none"`.)

  **The consumed move/fire byte outranks the externals.** Both games
  also RECORD the consumed per-tick command byte inside the state
  closure (`Type_160`/`Type_str_164 dw_0`: bits 1/2 speed, 4/8
  strafe, 0x10 left fire, 0x20 right fire) — the byte retail's own
  tick acted on, stamped by the consume loop (MC1 post-pass, read at
  record N; MC2 in PlayerEvents, read at N+1). Corpus-measured
  against retail's own arms: MC1 2,368/2,368 single-shot casts at
  exactly byte-record +2, MC2 560/560 arms on the same record — so
  consumers that need exact input (the pose channel, `replay`)
  read the byte and the ±1 caveat below applies only to the raw
  externals.

  **MC2 phase caveat — the snapshot straddles retail's poll, and the
  latch resolves it.** The recorder parks in the settled tail of frame
  `r` (after the entity pass, before frame `r+1`'s `PlayerEvents`), so
  a press visible at record `r` may have been consumed by frame `r`'s
  own poll *or* still be pending for frame `r+1`. The press LATCH is
  cleared the instant `HandleMouseButtons_18F80` consumes it, so a
  latch still up at the snapshot means "not yet polled": the input
  frame `r` actually consumed is
  `held(r) && !latch(r) || latch(r-1)`, and THAT is the stream a
  consumer must feed to the tick that advances `r-1` → `r`. Measured
  against retail's own arm ticks over the whole MC2 corpus: 4,814 of
  4,815 right-hand casts land on its rising edge with zero offset
  (`mgc-conform`'s `verify_mc2::align_cmd_mc2`). Consumers of the MC1
  `input` channel have no latch register and keep the ±1 caveat.

Retail's own multiplayer lockstep puts exactly the per-tick 10-byte
control commands on the wire — the "consumed command per player per
tick" unit is retail's own canon, and this channel is deliberately
shaped like it.

**Cheats: the witness is the toast, not the key.** Both engines expose
a cheat menu on control opcode 30 (`0x1E`, `param1` = sub-code 1..7 on
MC1, 1..10 on MC2; ALT+F-key in both). It is the one recorded verb
that MUTATES the world instead of steering it, so a free-running
consumer must apply it or diverge permanently from that tick. It
cannot be read off the control slot — retail memsets the 10-byte
command in the same event pass (remc1 :49044), before any capture
window opens, and opcode 30 appears **zero** times in either cheat
take. The raw key channel does see the F-key but carries the ±1-tick
caveat above and cannot separate a held key from a repeat.

The handler's OWN on-screen message settles both: it names the cheat
and it re-arms a lifetime counter that otherwise only counts down, and
it lands in the per-player block INSIDE the state closure (MC1 text at
wizard `+28 + 68·i`, counter at `+64`; MC2 text at block `+0x1C`,
counter at `+0x4D`). A cheat fired iff the counter INCREASED across
the pair and the text matches a handler string — repeats are dated by
the counter alone, since the text does not change between them.
Measured: mc1l0-test 23/23 fires and mc2l0-test 103/103, each matching
a key press edge 1:1, zero misses and zero false positives
(`mgc_formats::recover::Cheat`).

PHASE is per-game and both arms are corpus-dated
(`engine::world::cheats`): MC1's handler runs in `DrawAndEventsInGame`'s
command pass AHEAD of the tick function whose stub holds the capture
window, so its writes are visible at `t=N` UN-TICKED — the port applies
it at the TAIL of the pair tick. MC2's rides `PlayerEvents` INSIDE the
frame the recorder samples the tail of, so its mints do tick that
frame — the port applies it at the tick TOP, beside the MC2 respawn.
Port takes carry the sub-code in `PortInput.cheat`, so a `--replay
--record` transcode of a cheated take still reproduces itself.

### `obs` — the shared observable projection

The decoded, human-greppable view: RNG word, wizards/players, control
slots, active entities with their gameplay fields — the same schema
whether decoded from retail memory or emitted by the port, so one
comparator serves retail-vs-port and port-vs-port. All values are
exact integers or exactly-round-tripping floats; comparison is
equality, never tolerance.

### `state` — the raw retail closure (retail only)

The full master-struct image, base64 (`"struct_b64"`; ~227 KB MC1/HW,
~220 KB MC2 — includes the pool, the per-wizard/per-player AI columns,
the control array, the RNG word, and the embedded pristine level
record; retail's own in-level save writes this exact MC1 struct with a
single `fwrite`, so the image is the game's own idea of its closure),
plus on MC1/HW the external input registers from the static frame
(`"ext"`: `keys_b64` pressed-scancode array, `cursor_b64` mouse cursor,
`lbtn_b64`/`rbtn_b64` held buttons — raw register bytes). The static
frame sits outside the consensus window, so `ext` carries the same
±1-tick attribution caveat as the `input` channel.
Consecutive images are nearly identical, so zstd collapses the channel;
no delta scheme is needed. This channel is the fixture-initialization
source and the licence to improve field maps after the fact. The
closure is *believed* complete; a delta-verify failure that survives
triage is the detector for state living outside it.

`wallclock` (retail): the free-running ~120 Hz PIT clock — a liveness/
ordering signal only, never part of the closure.

### `terrain` — the measured terrain channel (format 2)

The live terrain planes, read from guest memory in the same settled
window as `state` and recorded **relative to the previous record**:

```json
{"terrain":{"base_b64":"…"}}          // the take's FIRST record only
{"terrain":{"delta_b64":"…"}}         // any later record with edits
```

- **`base_b64`** — the full plane set at the first recorded tick: the
  declared planes concatenated in header order, each `width × height`
  bytes, verbatim guest-linear layout (cell index = the plane's linear
  byte offset). This is the t≈0 image — it doubles as the stock-bake
  validator (diff against the port's generated level terrain).
- **`delta_b64`** — per declared plane, in order: a `u32` LE count,
  then `count × (u16 LE cell, u8 value)` — the cells that changed
  since the **previous record** and their new values. An absent
  `terrain` key = empty delta (nothing changed). A record after a `t`
  gap simply carries everything the gap changed — the channel is
  self-healing by construction, which is WHY deltas are
  record-relative and not game-event-incremental: recorder stalls
  can never lose a terraform. Decoders MUST reject truncated blobs,
  trailing bytes and out-of-range cells outright (a torn blob never
  half-applies).

The channel describes the world **at** tick N (same phase convention
as `obs`/`state`). A streaming consumer maintains the running image in
O(delta) per record (`mgc_formats::mgcr::TerrainImage`); a consumer
that starts mid-stream without the base may still accumulate deltas
but must not treat the planes as absolute (`TerrainImage::based`).
Torn/excluded pairs keep their terrain deltas — planes are stable
mid-entity-pass except for the active edit, and the next record's
delta re-syncs regardless.

**Plane sources (both engines, decompile-verified 2026-08-05):** the
planes are CONTIGUOUS static arrays in guest memory, captured in
their guest order `type | height | shading | angle` at block offsets
+0/+0x10000/+0x20000/+0x30000. MC1/HW: base `mapTerrainType` guest
`0xCC1E0` (build A) / `0xCC1D0` (build B, dual-suffixed), reached via
the recorder's byte_99B58 static frame; MC1 shading is hard-clamped
to [28,47] by every retail writer — the recorder's alignment AND
level-generated gate — and is NOT derivable from height (flat cells
take an LCG roll at bake), which is why it must be captured. MC2:
base `mapTerrainType_10B4E0` through the struct-anchored data frame
(named VA − 0xB0E98), plus the cave-only `ceiling` plane
(`x_BYTE_14B4E0`, +0x40000) appended to the declared list **only when
the level's MapType (struct+0x2FED4) is Cave** — retail never writes
it on Day/Night levels, so off-cave it holds BSS residue, not
terrain. Cell = `tile_y*256 + tile_x`; world z = `height[cell] × 32`
(floor and ceiling alike).

**The entity index (retail, from 2026-09-27).** The per-tile
`mapEntityIndex` (i16 per cell: the pool slot heading that tile's
entity chain, 0 = none; MC1 block +0x40000, MC2 +0x50000 after the
ceiling) rides the channel as two BYTE planes appended after the
terrain planes: `entity_index_lo` then `entity_index_hi` (value =
`lo | hi << 8`). It is fully derivable from the pool — it is captured
as ground truth for exactly that reason (the remc2 V03 save carries
it, and a disagreement with the reconstruction is itself a finding).
Consumers that grade terrain skip planes they do not model
(`terrain-check` reports them as skipped). A moving entity changes a
handful of cells per tick; measured on mc2l0, ≤334 delta bytes/tick.
Read by `mgc-conform lane-check` and the `MGC_INDEX` replay switch
(docs/CONFORMANCE.md) since round 166. ⚠ Retail's table is NOT always
derivable from the pool: on four MC1/HW takes a wizard's death links a
FREE record into a chain (ledger 166-4).

Size: empty deltas are 4 bytes per plane before compression;
terraform windows tens of cells; volcano/doomsday storms hundreds —
negligible next to `state`.

## The init record

```json
{"type":"init","counter":0,"obs":…,"input":…,"state":…,"terrain_rand":…,
 "terrain_b64":"…","building_f2cd0_b64":"…"}
```

The world **as level initialisation left it, before frame 1** — line 2
of a take whose header declares `channels.init`. It is taken by the
recorder inside the tick-patched exe's **init park** (see "The init
park"), with the guest held until the recorder releases it, so it is
race-free by construction. Fields:

- the tick channels (`obs`, `input`, `state`, `terrain_rand`) exactly as
  in a tick record, minus `t`;
- `counter` — the stub counter the park held at (0 on a fresh launch);
- `terrain_b64` — the FULL plane image, header plane order (the
  terrain planes and the entity-index byte planes), concatenated like a
  `base_b64`. Deliberately NOT a `terrain` channel: record 0 still
  carries its own `base_b64`, and no base/delta chain ever folds the
  init image;
- `building_f2cd0_b64` (MC2) — `building_F2CD0x`, the 4802-byte
  building-shape table the V03 save's SMAP part ends with
  (level-independent; captured to verify, not assumed).

What "before frame 1" means, per engine (decompile-verified 2026-09-27):

- **MC2**: the park is in the frame-driver stub, before `call
  DrawAndEventsInGame_47560` on the first frame. Between the end of
  `LevelInitGame_56A30` (remc2's `RecordingLevelSave` point, the V03
  save instant) and there, retail only makes idempotent overwrites
  (`LoadSpr_47160`'s paths, `maptypeMusic_0x235` from MapType,
  `paletteMod_51 = 0`, `dw_w_b_0_2BDE_11230.word[1] = 0`). No carpet
  exists yet (it is spawned inside frame 1); Turn is 0.
- **MC1/HW**: the park is on `GameLoop_34610`'s `call
  DrawAndEventsInGame_34530_348F0`. Every wizard's command slot
  (`+29715 + 10·p`) still holds the JOIN command (1) that
  `sub_3DD50_3E090` leaves; frame 1's `sub_3C9D0_3CD10` then spawns
  each wizard's carpet, spell tokens and (computer wizards) starting
  castle, clears the slot, and bumps `+13341` — all BEFORE the first
  tick call, and without drawing `rand_4` (NewEvent only reads it).
  So the init record is pre-spawn, and record 0 (`first_counter: 1`,
  the tick stub's first window) is post-spawn, pre-tick: the pair
  grades the join prologue by itself. (Also written there, not world
  state: `+581` 1→0 in the palette load; `u32_379` = the wall clock.)

Old takes carry no init record; `record0_settle` keeps serving them.

### `hash` — the port verification channel (port only)

The golden state hash at tick N, as a hex string. Inputs + hashes is
full byte-exact determinism verification at a few dozen bytes per
tick — and it is the desync checksum retail's lockstep never had, so a
future multiplayer inherits it unchanged.

### `set` — live-option events (port only, optional)

`"set":{"invincible":true}` — options the player applied from the
running app between the previous row and this row's hash point. These
are app-side sim writes the `input` channel never carries (a cheat
toggle from the options menu forked test.mgcr's whole course invisibly
— 2026-08-27), so the recorder emits them as row events and a replayer
MUST apply them before grading the row's hash, through the same
setters the options menu uses. Current keys: `invincible`,
`dev_spells`, `ghost`, `inert`, `weightless` (bools; `lift_unclamped` is
the pre-2026-09-15 spelling of `weightless` and still replays), `thrust_model`,
`altitude_model` (`"classic"`/`"enhanced"`). A key the replaying build
does not implement is a REFUSAL, not a warning — the take's course
depends on it.

These are port constructs and deliberately not the retail cheat lane
(`input.cheat`): that lane replays retail's own cheat opcodes with
retail's semantics, which differ from the port options' at the edges.

## Gaps

Recorders SHOULD emit gap-free streams (lower DOSBox cycles until they
do). A jump of k>1 in `t` is legal but breaks the fixture pairing
across it; runners count and report pair coverage.

The known gap mechanism on a tear-gated recorder is a SIM-DOMINATED
stretch: whenever the guest's cycles are spent inside the entity pass,
every DOSBox park lands mid-tick and no clean boundary is exposed —
those ticks are unrecoverable by sampling, whatever the poll rate. Two
flavors:
- LOAD-shaped: sim logic swells (ambient spawn storms, heavy combat)
  or host stalls (audio buffer pressure) eat the budget. Mitigations:
  raise cycles until the game reaches its frame cap, raise the GAME's
  render load (its SVGA mode — render cycles never touch the sim
  struct, so render-bound frames are wide capture windows), bigger
  mixer buffers or sound off.
- STRUCTURAL, fixed-length: full-screen flash/fade sequences (big
  explosions, the level-start fade) draw almost nothing for ~9-10
  frames, the frame collapses to sim+flip, the game momentarily runs
  FAST, and the renderer capture window vanishes — a deterministic
  ~9-tick gap that no cycles/render/sound setting can remove. These
  are exactly the transition-dense ticks fixtures want. The structural
  fix is the tick-patched exe (below): it makes the game pace itself,
  so a quiescent window exists every sub-step regardless of render
  load, closing both gap flavors at once.
The recorder must classify mid-tick parks (including the early-cursor
case, where the tick-top LCG has drawn but the +63 mode still reads
0 — indistinguishable from "same tick" without the RNG check) and
report the loss LIVE, per pending tick, not only as a bare `t` jump
discovered afterwards. (A tick-patched exe removes the guesswork —
see "Tick-patched capture".)

## Capture tearing (the inter-tick gate)

Read-consensus (N byte-identical reads of the volatile ranges) proves
only that the guest was FROZEN — DOSBox regularly parks
**mid-entity-loop**, so a consensus image can be a mid-tick state:
entities below the loop cursor already stepped, entities above not,
and the global LCG possibly not yet drawn. On the first recorded
corpus ~75% of MC1 snapshots were mid-pass; the artifacts masqueraded
as sim findings (a "12.5% RNG stall", an "asleep set" of
+63-frozen entities) until the fixture runner proved the stepped
set always formed one contiguous slot band — the loop cursor.

The MC1/HW law: a snapshot pair is a true inter-tick pair iff every
persisted entity's `+63` clock advanced by exactly `dv` (retail's
dispatch table is static; every live state row ticks) AND the global
LCG advanced exactly `dv` steps (one draw per sub-step). Recorders
MUST enforce this at emit time (`pair_clean`). Deviant
discrimination: only steps of exactly `dv±1` count as tear suspects
(the cursor-band signature — one pass short or long); arbitrary-step
deviants are ambient spawn CHURN (slot re-use overwrites `+63` with
the spawn ordinal — constant on HW's weather families, and a flat
deviant cap starves the recorder there). Headers stamp
`capture.tear_gate: true`; recordings
without the stamp carry torn states, and fixture runners MUST
re-classify their pairs with the same test and exclude torn ones from
conformance verdicts.

The MC2 law (measured on the mc2l0 corpus, 2026-07-30) is different:
neither Turn continuity nor LCG-step parity discriminates. Retail's
frame order is `PlayerEvents` (Turn++) → `UpdateEntities` (one
unconditional LCG top-draw, then the slot-order dispatch), so a
DOSBox park between the two yields a snapshot whose Turn has advanced
but whose entities have not — Turn delta is +1 on EVERY adjacent
pair, torn or not, and the draw count per tick is activity-dependent
(0..16+, mode 1) with most frozen pairs still showing one draw. The
working discriminator is the per-entity phase byte `byte_0x3E_62`
(incremented once per handler run, per entity, per pass): a true
inter-tick pair is **step-1 dominant** over the entities live at both
ends (`d1 ≥ max(d0, d2)`, deltas taken mod 256 with values outside
{0,1,2} ignored as animation wraps). A Turn-side park produces an
all-0 pair (positions frozen; measured moved-fraction 0.04) followed
by an all-2 pair — ~30% of mc2l0's pairs. The runner applies this
gate from the raw states (`mgc-conform`'s `capture_clean_mc2`);
recorder-side emit gating for MC2 is still open work.

### The PER-SLOT gate's cadence law (round 149)

`capture_clean_mc2` gates whole PAIRS; `verify_mc2::torn_slots` then
drops individual slots inside an accepted pair. Until round 149 the
per-slot test was the bare `phase3e delta == 1`, which contradicted
the pair-level test above in two ways and hid **82%** of mc2l24's
exclusions on ONE species:

1. **A null-dispatch row never bumps.** `UpdateEntities_57730`
   (NETHERW.EXE file 0x7BF30) bumps `byte_0x3E_62` only inside the arm
   that ran a handler (`cmp %cx,%dx` on `row.word_4`, `cmpl $0,0xa(%eax)`
   on `row.dword_10`, then `call *0x6(%eax)` and `inc`). A record parked
   in a row with `address_6 == 0 && dword_10 == 0` HOLDS its phase byte
   for life. Measured on mc2l24: `(5,27)` act 0xEA held on 1,591,335 of
   1,591,470 consecutive-tick pairs, `(10,75)` act 0x52 on 28,006 of
   28,006, `(10,77)` act 0x54 on 49,875 of 49,883.
2. **A birth re-seeds it.** `byte_0x3E_62 = D41A0_0.array_0x10[model]++`
   (the per-model spawn ordinal — it is in the recording, as
   `RetailMc2::spawn_ord`). This is the same "ambient spawn CHURN" the
   MC1/HW paragraph above already names; the MC2 per-slot gate simply
   never got it. A capture tear shifts a slot by at most one dispatch
   per side, so `{-1, 0, +1, +2}` is the whole tear-reachable set;
   mc2l24's `(10,0)` `-10 x51,753 / -9 x15,966` family is a re-seed.
   The DIRECT witness of a re-allocation is retail's own `life_0x8`:
   it counts DOWN under the handler (class 9 action 0x0E is
   `sub_67410`//248410, literally `life--; if (old < 0) retire`), so a
   life that ROSE cannot be the same instance. Cross-tabulated on the
   whole mc2l24 take for the `(9,9)` spark: all 278,523 `+1` pairs
   have life falling, and 179,457 of the 180,248 non-`+1` pairs have
   life rising.

Both narrowings are on by default. `MGC_TEAR_LEGACY=1` restores the
bare `!= 1` test, `MGC_TEAR_NO_RESEED=1` disarms (2) alone.

Measured round 149 (whole takes, raw shadow):

| take   | arm                | tear-gate hidden | RAW SHADOW           | graded |
|--------|--------------------|------------------|----------------------|--------|
| mc2l24 | `MGC_TEAR_LEGACY`  |        1,936,794 | 980 (4 lanes)        | 10 seg |
| mc2l24 | cadence law        |        **3,309** | 980 (same 4 lanes)   | 10 seg |
| mc2l22 | `MGC_TEAR_LEGACY`  |        2,298,158 | 1,307 (3 lanes)      |  4 seg |
| mc2l22 | cadence law        |       **12,472** | 1,308 (+1, old lane) |  4 seg |

`--segmented --brief` is byte-identical on both takes in both arms.

The FIRST record has no pair to gate it, so recorders MUST NOT write
it unvetted (a mid-tick anchor rejects every later pair against it and
starves the stream): hold the candidate and flush it only once the
first clean pair vouches for it, replacing the anchor with the newer
read whenever a bootstrap pair is rejected.

## Tick-patched capture (windowed)

The tear gate is a *reconstruction* — it infers, after the fact,
whether a frozen snapshot happened to land between ticks. The exe
tick-patch (`tools/mc_exe_tickpatch.py`) removes the inference by
making the game cooperate. It installs a 249-byte wrapper stub around
the per-sub-step tick function (remc1 `sub_41780_41AC0`) of a COPY of the
binary — `CARPET_REC.EXE` / `HIDDEN_REC.EXE`, never the pristine
gamedata — by redirecting the tick fn's callers (rewriting each
gameSpeed-fanout `call`'s 4-byte rel32) so they enter the stub, which
paces, then `call`s the original untouched tick fn and `ret`s. The
function entry stays byte-for-byte intact (an earlier version overwrote
the entry with a detour, which decoded as a wild `add eax,[eax]` when
the dynamic recompiler picked the region up misaligned). Every sub-step
the stub does two things:

1. **Paces to a wall-clock deadline.** It spins on the game's own PIT
   counter (measured live at ~120 Hz) until one period (default 5 counts)
   has elapsed since the last release, so `fps = 120 / period` ≈ **24 fps**
   at period 5 — the authentic Magic Carpet rate — regardless of how high
   DOSBox `cycles` is set; the excess cycles are burned in the spin. Exactly
   **one sub-step per rendered frame** is paced (the first, detected via the
   gameSpeed fan-out's live loop index in `EBX`), so the F3 game-speed feature
   still speeds the *sim* up 4×/16× while the frame rate holds; at the default
   speed of one sub-step per frame every sub-step is the first. (Both
   obj1's cave and obj3's mailbox must be page-aligned via their `vsize`
   fields, or the tail is outside the segment limit — the code cave won't
   execute and the mailbox writes won't persist.) This is the frame cap
   retail never had; it
   is a *presentation* throttle only. MC1's sim is wall-clock
   independent (its lockstep multiplayer proves it: the PIT counter
   feeds render/animation timing, never sim state), so pacing changes
   *when* sub-steps run, never *what* they compute — the recorded tick
   sequence is byte-identical to an unpaced run.

   **Floors the window** (`--floor N`, default 2 counts, `0` disables).
   A deadline is only as good as the compute fitting inside it: a sub-step
   heavy enough to overrun its period — deaths, several meteors at once —
   arrives with the deadline already passed, the spin falls straight
   through, and `in_window` is raised and cleared within a handful of
   instructions. That zero-width window is unlandable, so the recorder
   drops the frame and the delta across it tears; and because the release
   path tolerates up to 30 counts of backlog with no wait at all, one
   heavy sub-step is followed by a burst of free-running ones. The floor
   clamps `deadline = max(deadline, now + N)` before the spin, making the
   window's width independent of load and deleting the catch-up burst with
   it. It is a **no-op while the game keeps up** (steady-state waits are
   unchanged) and is charged only to the sub-steps that already overran —
   unlike lowering `--period`, which taxes every frame. The PIT counter is
   integral, so `N=1` guarantees nothing and `N=2` is the smallest value
   guaranteeing a full count (≥8.3 ms); the tool rejects `1`.

2. **Publishes a mailbox** in obj3's committed tail (guest-linear
   `0x132c40`, same address in both builds; the stub derives obj3's real
   runtime base from the game's own relocated struct pointer so its writes
   stay in obj3 and never corrupt game memory): an 8-byte magic
   (`MGCTTIK1`), a monotonic sub-step counter (`+8`), and an
   `in_window` flag (`+0xC`) raised for the whole spin. The spin *is*
   the quiescent window — the world struct is fully settled from the
   previous sub-step and the current one's LCG draw has not begun — and
   it is proportional to the spare cycle budget, so on a fast host it is
   ~7 ms wide on a typical sub-step. On a sub-step with *no* spare budget
   the width is whatever `--floor` guarantees (≥8.3 ms at the default 2)
   rather than zero, which is what makes "every sub-step, bursts included"
   true rather than aspirational.

A recorder that finds the magic switches to **windowed capture**: take
the struct only while `in_window==1`, require the counter and struct to
stay put across the consensus reads, and use the counter's delta as
continuity. This is strictly stronger than the tear gate (a
between-tick window is guaranteed by construction, not inferred) and
`t` is the stub's authoritative sub-step index, not a `+63`-mode
estimate. Such recordings stamp `capture.window_gated: true` and
`capture.exe_patch: {mailbox_guest, spin_period_counts, first_counter,
live_counter}`; consumers may treat window-gated snapshots as tear-free
without re-running `pair_clean`. `first_counter` is the raw stub counter
of record 0 and `live_counter` the value at which the recorder saw the
counter start moving; they differ only when the recorder missed windows
at the start. The counter is process-lifetime (the stub initialises it
once, on its first call), so for a fresh launch it dates record 0
absolutely: the MC1 stub opens its window BEFORE the tick call, so
`first_counter: 1` is the pre-tick world of frame 1 — ⚠ AFTER frame 1's
join/spawn prologue, not post-init (see "The init record") — (t=0 =
sub-step 0, and generally record 0 is the state after `first_counter − 1`
ticks); the MC2 stub opens it AFTER the frame driver, so `first_counter:
1` is the state after frame 1. The pre-frame-1 world of both is the init
record. Recordings without these fields predate the
mailbox go-live (2026-09-23) and start a latency-random 1..24 ticks in
(the port reads that phase from record 0 — `record0_settle`). To avoid re-scanning a window it has already captured, the
recorder reads only the 8-byte mailbox first and pulls the full struct
only when `in_window==1` **and** the counter has advanced past the last
emitted frame.

### The init park (both arms, 2026-09-27)

`--init-park N` (default 500 timer counts; `0` = no park code) adds a
one-shot hold on the call that starts a level's frame 1, so the
recorder can take the init record without a race:

- **Where.** MC2: inside the signal stub, before its `call
  DrawAndEventsInGame_47560`. MC1/HW: a small stub of its own on
  `GameLoop_34610`'s sole `call DrawAndEventsInGame_34530_348F0`
  (`pushad`, park, `popad`, tail-`jmp` to the untouched driver; the
  call's rel32 is the only game byte changed, and it carries no LE
  fixup). The MC1 tick stub is byte-identical with or without it.
- **Mailbox words** (MC1 base `+0x1C/+0x20/+0x24`, MC2 `+0x10/+0x14/
  +0x18`): `ARM`, `PARK`, and the park magic `MGCP`. A fresh process
  (no `MGCP` yet) parks its first level unconditionally and writes
  `MGCP`; afterwards a park happens only when the host writes `ARM =
  1` (the stub clears it as it parks). `PARK = 1` while held; the
  host writes `2` to release; the stub leaves `3` (released by the
  host) or `4` (timed out). The tick counter and `in_window` do not
  move during the park — on MC1 the tick stub has not even run, so its
  `MGCTTIK1` magic appears only after it.
- **Timeout.** N counts of the game's own timer (~4.2 s on MC1, 5 s on
  MC2 at the default), so the exe run without a recorder only pauses at
  level start. A read-only recorder cannot release; the timeout does.
  `wait_for_mailbox_tick` releases any park it finds uncaptured, so a
  caller that ignores the init record never sits the timeout out.
- **Locating the struct while parked.** Neither engine has a carpet at
  the park, so the recorder's class-3 census cannot pass. MC2 accepts a
  needle hit whose own mailbox (struct − 0xD41A0 + the MC2 frame) shows
  a live park. MC1 takes the static frame whose mailbox shows a live
  park (the build from `--game`: CARPET = A, HIDDEN = B), the one
  needle hit that looks freshly initialised, and re-verifies the frame
  against the owner chain once frame 1 has spawned the carpets
  (dropping the init record if they disagree).
- Behaviour-neutral like the rest of the stub: a wait, no writes to
  game memory.

### MC2 / NETHERW arm (signal-only)

MC2 already frame-limits itself — `InGameLoop_47320` runs the whole frame
(`DrawAndEventsInGame_47560`: `PlayerEvents`→ entity pass → draw) then
spins `while (before+5 > GameTimerTurn)` until 5 timer ticks elapse. So
MC2 takes are gap-free, but ~33 % are **torn**: DOSBox can park the guest
between `PlayerEvents` (`Turn++`) and the entity pass — a settled-looking
but mid-frame state (the phase-byte law above). The `NETHERW_REC.EXE` arm
therefore adds **no pacer**; it only *signals* the true boundary. It
redirects the loop's sole `call DrawAndEventsInGame_47560` to a wrapper
that clears `in_window` (the frame is about to mutate), calls the original
frame driver, then bumps a monotonic **per-frame** counter and raises
`in_window`. The flag is thus up from just after the draw, across MC2's
native limiter spin, until the next frame's `Turn++` — a settled window,
so the `Turn++`-park tear is unobservable by construction.

MC2's native budget is **absolute** (`turn_sampled_before_the_frame + 5`
ticks of the 100 Hz PIT), so a frame heavy enough to overrun it leaves no
spin at all and the window collapses to nothing — the same failure the MC1
pacer has, arriving by the same route. The MC2 stub therefore carries the
same **`--floor N`** (default 2 counts, `0` disables): after the counter
bump and the `in_window` raise, it spins on `GameTimerTurn` until N counts
have passed, so the total window is `floor + max(0, native spin)`. Placing
it in the tail — *after* the counter bump — means the window the recorder
sees announced as fresh is the same one being held open. `--pace N` (which
widens the absolute budget) remains available but is now the second-line
knob: it taxes every frame and still cannot guarantee a window on a frame
that blows the wider budget too. The mailbox
(magic `MGCTTIK2`, counter `+8`, `in_window` `+0xC`; **no period field**)
sits in obj3's committed BSS tail (guest `0x1842c0`); the stub derives
obj3's real base by reading the game's own fixed-up `GameTimerTurn` disp,
and both `vsize`s are page-aligned (same segment-limit requirement as
MC1). Continuity is the counter's delta — **never** the per-player `Turn`,
which advances mid-frame inside `PlayerEvents` and so cannot gate the
tear. The recorder also goes live off this mailbox: it spins on the
counter (a 16-byte read at the fixed address, no scan) and starts on the
first window it sees move, instead of polling the gameplay RNG — which
could only ever notice a tick after it had happened, ≥1 tick plus up to
150 ms late. Window-gated MC2 recordings stamp `spin_period_counts: null`. Old
tear-gated MC2 takes are unaffected; only RE-RECORDED takes get the
window (retiring the per-entity torn-slot exclusion). The tear gate
remains the path for any unpatched exe.

The same mailbox drives **retail re-recording** (`tools/mc_dosbox_retake.py`,
MC2 only): the driver boots retail with the take's level (the conf's
`-level N` is rewritten), turn-aligns to the take's t=0, transplants the
take's t=0 struct image AND terrain planes over the parked guest (the anchor:
heap pointers re-based, a few session-volatile fields kept native, the
entityIndex plane rebuilt from the pool's LINKED chain heads), then at every
park writes the raw input registers the next frame's fold will read and
captures the window exactly as the recorder does. Nothing is fed from a park
sample where the take's own state can say what the frame consumed: the
cursor is synthesized from the consumed roll/pitch (the fold's arithmetic
inverted from the flight column, remc2's `solveEnvelope`; equal to the v03
exporter's bytes on all 14734 mc2l0 turns), click edges come from the press
latch, key edges are attributed by the state they step (arrows: `cmd_speed`
and the strafe register, a table learned from the take's unambiguous frames;
Ctrl/Enter: the MenuState byte), a button RELEASE seen at a park while a
cast is still charged for that frame — or while the level-1 fireball token
shows that frame re-armed it (fired again, or starved with the charge
counter zeroed) — stays held, and every non-arrow,
non-modifier press also sets retail's `LastPressedKey` latch (value = the
scancode). The spell-bar menus (MenuState 5/8) are the one place the take's
raw input cannot be turned into the frame's input: the game warps the
cursor at open and restores it at close (so the close frame gets the
previous park's cursor, every other menu frame the next park's), the
flight roll is re-derived every frame from the cursor SNAPSHOT taken at
open (a synthesized cursor matches the consumed roll, not the pixel), and
the category is committed at the press frame from a poll position no park
sample carries. For those three the driver transplants the source's own
state cells each menu window — the backup position (player +0x3FA) and the
selected category/tier (str_611 +0x458/+0x459); both had been on the
anchor's keep-native list, inherited from remc2's replay, which is why a
retake picked the neighbouring spell. The retake's `input` channel is what
the driver wrote, not evidence. Validate with `tools/compare_mgcr.py A B
--retake --continue`. The comparator additionally DEMOTES the null-victim
ghost: on any pool record retail stamps `word_0x96` with
`-(EntitiesBase/168)&0xFFFF` when a lightning impact has no victim
(`mgc_sim::mc2::proj::mc2_null_victim_ghost`) — a session-layout constant
(0xAE02 on the old sources, 0xB41B on the tickpatched-exe retakes) that
PERSISTS as slot residue after the (10,23)/(10,38) blast dies, so the
demote gate is the value, not the species: both sides >= 0x8000 (a live
leader index is < 1000). Demoted bytes are counted on their own summary
line. Measured on mc2l0: pose exact over 2500 ticks
including three spell-bar menus and the map. Shift+L (remove castle stage)
is attributed by the castle's life sign (its press witness; releases are
inert), and a button release during a level-1 fireball auto-fire by the
token's charge counter (re-armed = still held, even when the cast starves).
Shift itself is only a modifier and has no witness of its own.

**The lockstep (2026-09-27; `--no-lockstep` disables it).** A timed
capture window is only as wide as the stub's floor (>= 8 ms) plus whatever
limiter spin the frame left, and the feed, three stable struct reads and
the record all have to land inside it — so any host stall longer than the
window loses a frame, the guest folds stale registers, and the take forks
(the gap class; it is load-driven: six parallel retakes of mc2l1-new under
48 CPU burners lost 3..12 windows EACH and all six diverged, the same six
on a calm host were clean). So the driver asks the stub to hold every
frame (**the frame hold**, below): it writes the magic into `HOLD` while
the guest still stands in its init park, and from then on every frame ends
in the stub's tail with `FPARK = 1`, its window open, until the driver —
done feeding and capturing that window — writes `PARK_RELEASE`. While held
the mailbox is unchanged, so the capture and every window-keyed check work
as they are; the driver releases as soon as the record's live reads are
done and serializes while the guest runs. The window is as long as the
driver needs, up to the stub's patience (`--frame-hold`, ~4 s). A hold that
times out, or a window the guest opened before the hold was engaged
(`--pid`), is a fresh window with nothing behind it: the driver takes it as
a timed one and says so. An exe without the hold runs timed, as before,
with a warning. The driver's `done:` report carries a `lockstep:` line
(windows held / timed, hold timeouts, the longest hold).

⭐ **Why the hold is in the stub's TAIL.** The first lockstep re-armed the
INIT park every frame — an ENTRY park, at the top of the stub. Between
frame N's tail and the next entry the guest runs its own loop control, and
on the level's last frame that is the whole teardown and the next level's
load, after which the stub parks over a DIFFERENT world with the mailbox
unchanged. Every take that runs to its level's end came out bit-identical
up to the last tick and 17-21k bytes off ON it (Turn 0, a fresh terrain:
the reloaded level's init state), while the same driver timed was
IDENTICAL. ⚠ A truncated take never shows this: test an end-of-take change
on a take that really ends its level. The driver keeps one guard from that
round: a captured Turn that FELL BACK is another level, and the take stops
there (`! the level ended in the guest after tick N`).

**The feed plan (`--plan FILE`).** Every feed rule reads the SOURCE take
and nothing else — the one exception is the stale-`L` latch clear, which
looks at the guest's own `LastPressedKey`. The driver is built on that:
`scan_take` makes one pass over the take and keeps, per tick, the raw
registers and the state fields the oracles read (a `Park`); `KeyOracle`
dates every input edge from them; `FeedPlanner` turns them into a `Plan`
per window (the register feed, the `LastPressedKey` write, the state cells
to transplant); and the drive loop only writes the plan, captures and
emits — it prepares the next window's plan while the guest runs its frame,
so the window itself is spent on writes and reads. `--plan FILE` prints
the whole plan, one line per window, without launching anything:

    t=1899 keys=[72,77] cursor=331,204 held=10 latch=00 press=318,199 struct+0x36dec=...

Run it with two versions of the driver and `diff` the files to see exactly
which frames of which takes a rule change moves — before spending a
retake on it (the "measure the flip set before choosing a rule" lesson,
now a command). `stale-L?` marks a window where the guest's latch is
cleared IF it still reads 38.

**The spellbook snipe** (`--no-snipe` disables it). A campaign take's
spellbook is not just data: level init copies the player block's learned
flags, spell levels and experience into the campaign record and back
(sub_53160 → sub_549A0), enables every learned spell, and the carpet spawn
allocates one token entity per enabled spell, in spell order. A fresh
`-level N` boot has two starting spells, so a campaign take's ten extra
tokens shift every later allocation by ten pool slots — and the slot is
gameplay (the allocator seeds each entity's private RNG with slot + master
RNG and starts its think cadence at the slot index). Reconciling the
spellbook at the first window is too late for that; the driver therefore
writes the take's five arrays into the player block BEFORE the level
loads: once BYTEARRAY_4's pointer global reads its calibrated address
(process init is past; on this machine ~2.7 s after launch, the level
loads at ~5 s) and while the level needle is still absent. The guest is
reached by content (DOSBox's BIOS date at linear 0xFFFF5) plus the linear
addresses a located run writes to `~/.cache/mgcarpet/retake-snipe-mc2.json`
(so the first run on a machine only calibrates). The file holds a LIST of
boot variants (the heap does not always land in the same place — the
pool-base variant below): an unsniped run ADDS its boot's addresses,
atomically, and never overwrites a known one, so one odd boot cannot
poison the runs after it, and a boot that matches no variant is given up
on as soon as the stub's mailbox magic appears (the level is loaded)
instead of waiting the timeout out. Measured on mc2l0
without the anchor: 260/261 entities byte-identical in the same slot at
t=0 (the tokens at 153..164, the free stack identical), the whole run
within 6 bytes (the wizard name and one display setting) — the lead-in
pitch converges by t=20. A retake is then input + spellbook only.
Measured on mc2l1 without the anchor (2026-09-24): IDENTICAL across all
17,317 ticks (struct + terrain), the two instrument gaps the first run
exposed both fixed in the driver:
1. **The release oracle's firing test was fooled by the other hand.**
   `held_through` held a release whenever mana decreased at all — but
   the right-hand fireball's -100 kept the delta negative for a frame
   after the -250/tick beam's button was released (t=12213: the source
   consumed release+press in ONE frame, the retake held the beam one
   extra frame and the pool wandered for 296 ticks). The test now
   requires the drain to CONTINUE AT ITS RUNNING RATE
   (`m2-m1 <= m1-m0`), which the other hand's smaller shot cannot
   fake. One decision flips in the whole take — the bug itself.
2. **Auto-fire repeat pulses are invisible in the park registers.**
   Retail's input layer re-emits the fire click on a ~4-frame repeat
   timer while a button is held (HandleMouseButtons_18F80's latch arm):
   the consumed fire bit (flight column dw_0, 0x10/0x20) pulses on the
   grid, but the press latch only shows the physical press, so the
   composed edge stream missed every repeat. One missed pulse stalls
   the token re-arm and the auto-fire — at t=12246 the skipped
   fireball shifted kills/mana by +3100 within 3 ticks and the take
   NEVER resynced (the first run's "transient" was luck, not a law).
   The driver now feeds the latch on exactly the frames the take's own
   consumed byte (`dw_0`, series[14]) says a click was consumed —
   mechanism-independent. TWO guards make the latch feed safe: never
   when the fold's arm-b (held && the hand token's charge > 0 at the
   previous park) clicks unaided — a fed latch there is NOT idempotent,
   because two latch bits in one frame are retail's both-buttons
   MAP-OPEN (commits PlayerAction 20, drops BOTH fire clicks; mc2l2
   t=18864 forked permanently on a beam pulse fed beside the fireball
   press edge) — and never when the composed edge already carries it.
3. **Sub-frame key taps are invisible in the park registers too.**
   A press+release inside one inter-park window folds into the guest
   but never shows in any pressedKeys sample — mc2l3 t=14025 consumed
   a speed-up notch (dw_0 0x01, cmd_speed 0->16) with no key in any
   park, and the missing notch forked the trajectory permanently. The
   consumed byte is again the authority: the fed image's four arrow
   cells are set from `dw_0 & 0x0F` (the movement bits), the KeyOracle
   tables staying on as the corroborating fallback for everything
   else. mc2l2 t=25986 added the full-stop key (Backspace, opcode
   0x27): cmd_speed := 0 is an absolute write no step law produces,
   so RESET_KEYS dates its press by the register hitting 0.
The comparator demotes the session-constant fields (wizard name,
game-settings block 0x218A..0x219A incl. shadows/flat — SESSION_CONSTANT)
so a freerun take's real windows are not buried under per-tick benign
noise. Since 2026-09-25 it also demotes `array_0x39[508]` (the TMAP
usage table — read only by the load/save asset manager; a campaign
session's table carries residue a fresh boot can't match: a constant
61-123 bytes on mc2l31/32/33) and the null-victim ghost (entity
`word_0x96`, both sides >= 0x8000 — see above). Since 2026-09-26 it
also demotes the hit-flash counters (str_611 +0x195..+0x197, all 8
players): their ARMING is closure (a survivable hit — mc2l22-new
t=19421 armed identically both sides), but their DECREMENT is gated
on BYTEARRAY_4's palette phase (GameUI.cpp:163/238/295,
colorIndex_121[1]) — out-of-closure session state, so the countdown
legitimately desyncs (mc2l22-new t=19427/t=19857: the source's
counter stuck at 1, the retake's reached 0).
Real damage forks still surface through the life fields. Residual
known-benign residue on freerun takes: the lead-in
attitude envelope converges within ~50 ticks (the anchor exists to
erase it), and a fast spell-bar flick can leave the committed category
byte off by one for a few frames until the next menu re-transplants it
(mc2l2 t=17407, self-heals, never propagates — the poll-position commit
is the known-hard menu case).

**The 2026-09-25 attribution round (the batch re-grade's forks):**
4. **The `-level` grant.** `InitialiseSpells_54A50` reads
   `LEVEL_LOADED_FROM_ARG` (BYTEARRAY_4+24 bit 0x80, its ONLY reader;
   ⚠ the remc2 field suffix is DECIMAL): a `-level N` boot grants the
   map's StartingSpells ON TOP of the carried book, a campaign
   transition doesn't — six campaign takes (l12/13/14/16/17/30-new) had
   up to 18 spells fewer than their boots, and the extra tokens shifted
   every allocation. The snipe holds the bit clear until the pool
   fills; the take's own t=0 book is authoritative for both source
   kinds. (Takes with pre-t=0 pool-residue damage beyond the book —
   free-stack order, stale recycle cells — still want the anchor;
   `NewEvent_4A050` memsets the 168-byte record at spawn, so dead-slot
   residue itself never propagates.)
5. **A record-0 menu.** A take whose record 0 sits inside a spell-bar
   menu (mc2l7/mc2l19 — Ctrl HELD from t=0, the press edge pre-record)
   needs the guest's menu open from the lead-in: the driver holds Ctrl
   through the skipped windows (a press+release lets the first fed
   Ctrl-held image toggle it shut again) and transplants the backup
   snapshot/category at the FIRST window too (the per-window assist
   skips it on its prev_ctr gate). Without it the fed menu-warp cursor
   spends two frames as FLIGHT input and the envelope never unwinds.
6. **The composed click edge can be one frame EARLY.** A press landing
   mid-frame (after the poll) raises `held` at park k but is consumed
   by frame k+1 — the latch is set and consumed inside k+1, invisible
   at every park (mc2l22 t=9 cast at 9 vs 10; the token charge + mana
   prove it). When the take's consumed byte lacks the fire bit at the
   composed frame but carries it at the next, the driver drops the fed
   edge (the pulse supplement re-adds it a frame later). ⚠ Only ever
   SHIFT, never drop: the byte doesn't record every click class (mc2l0
   has 22 composed edges it never carries — pairs ~14 frames apart).
7. **Spell-cycle clicks.** Shift/Alt+click cycles the hand's spell
   (sub_18DA0) — a UI consume dw_0 never carries. The witness is the
   selection index (str_611 +0x451/+0x453) changing (+ the spell-name
   toast re-arming +0x4D): a sub-frame tap is synthesized from the
   witness (mc2l7 t=1153 "Meteor"), and a visible press whose click was
   consumed a frame late is deferred to it (mc2l19 t=129). Requires the
   modifier in the fed keys image (else it would be a FIRE click);
   never inside a menu window.
8. **The respawn key.** Space's press revives the dead player at the
   castle; PRESS_EFFECT_KEYS dates it by the carpet's life sign flip
   (mc2l20 t=15922: the source respawned at frame 15923, the retake at
   15922 — the respawn's 26 token allocations and RNG draws never
   resynced).
9. **A modifier press can delete a movement frame.** Shift+arrow is NOT
   a movement command, so a Shift edge dated one frame early eats a
   strafe/speed frame the source consumed (mc2l6 t=3377: the register
   left the 80-clamp a frame early; mc2l17 t=9340 same shape). The
   witness: dw_0's movement bits stop while the parks still show the
   arrows held — the oracle dates the modifier's press to the next
   frame.
10. **A one-park held dip composes a phantom re-press.** The park
    sampler can catch held=0 for one or two parks while the button never
    left the fold's view; the C-law turns the rise into a fresh edge and
    the retake casts a spell the source never had (mc2l9 t=2683, mc2l34
    t=3581, mc2l19 t=16025). Kill the edge when the rise frame's dw_0
    lacks the click AND the rise came out of a ≤2-park dip. mc2l0's 22
    byte-invisible clicks rise from sustained zeros (real presses from
    rest — dry clicks: no spell ready, nothing happens, feed them).
11. **The menu commit transplant.** The per-window backup assist also
    writes the take's committed category/tier at the first window AFTER
    a menu closes — the pick commits at the press frame from a poll
    position no park carries (mc2l30-new t=7186: the last menu left
    +0x458 1-off for 3.2k ticks).

**The 2026-09-26 attribution round (the wave-3 re-grade's forks):**
12. **Item 10's byte test is blind inside menus.** A spell-bar
    double-click (release at park k, re-press at k+1) is a REAL
    re-click — the source re-arms byte_0x457 and the toast — but dw_0
    never carries menu clicks, so the dip patch's byte test "proved"
    every menu re-press phantom and ate it (mc2l0 t=2256, mc2l3 t=848,
    mc2l6 t=558, mc2l11 t=20, mc2l10-secondtake t=4531). In a menu the
    commit witness arbitrates instead: the toast RE-ARMING at the dip
    frame (the counter jumps up: idle/-1 -> 19, a countdown 11 -> 13 —
    countdown and expiry never jump up) means the release was real and
    the edge stays. Validated: mc2l0/mc2l3/mc2l6 retakes byte-IDENTICAL
    end-to-end; mc2l11's fork moved to t=11164.
13. **A one-park dip inside a menu can be wholly phantom** (the mirror
    of 12): the release half never entered the source's frame stream
    (a physical bounce inside one frame), and the fed release committed
    a bar pick the source never had (mc2l34 t=6755: byte_0x457 dropped,
    the pick changed the equipped spell). Same witness, opposite sign:
    NO commit at the dip frame -> the dip never happened — feed held=1
    through it AND kill the re-press edge at the next window.
14. **The release oracle's mana witness reads unrelated upkeep.** A
    steady drain that has nothing to do with the held button makes
    `firing` true and holds the release a frame too long. In MENUS the
    buttons are UI consumes, so only the toast witness votes (mc2l5
    t=2458: a -14/tick upkeep held a menu release late; mc2l11 t=21:
    the old ==/!= byte test also went blind while a previous toast was
    still counting down — the re-arm test dates it). In FLIGHT the
    hand's dw_0 fire bit is decisive: any consume of the button at
    frame f sets it (mc2l0 t=1899, bit set at f = still held,
    corroborated), and a per-frame consumer's bit STOPPING at f is the
    release, whatever the mana rate says (mc2l14 t=960: the beam's
    drain lands on the TARGET entity, the carpet's -14/tick upkeep
    faked the rate, the retake beamed one extra tick and the death
    spawn forked the pool). The mana rate survives only for the
    grid-gap case (bit clear at f-1 too — the byte is blind there).
15. **The modifier/click same-window race, both directions.** A
    Shift/Alt PRESS edge and a click edge in one fed frame compose a
    Shift+click = the spell CYCLE — but dw_0 carrying the click's fire
    bit proves the source consumed the click as a FIRE before the
    modifier landed (mc2l9 t=3229: RShift rose at the press's park, the
    source fired dry, the retake cycled and armed the spell-name
    toast). Withhold the rising modifier for that frame. The inverse: a
    Shift/Alt RELEASE dated a frame early beside a CYCLE the take does
    show (the selection indexes move with no menu open — the modifier
    was still live at f's poll): hold the release to f+1 (mc2l34
    t=16736: the retake fired a 160-entity volley where the source
    cycled spells). The same release ambiguity beside a CHEAT: a fire
    the toast counter dates to frame f proves Alt was live at f's poll
    (the handler's gate is pressedKeys[ALT] + LastPressedKey), so Alt's
    release holds to f+1 too (mc2l6-rival t=207: the park shows Alt up
    at the cheat's frame; fed early, the +100k mana grant never came).
16. **The sub-frame Shift+L demolish tap.** A press+release inside one
    inter-park window leaves no L in any park, but the castle's life
    jumping from healthy to EXACTLY -1 in one frame is its signature —
    combat deaths crawl through small per-frame hits (mc2l13 t=1748:
    18150 -> -1; mc2l23 t=2213: 40000 -> -1). The Shift dispatch reads
    ONLY LastPressedKey, so feeding the latch with Shift in the image
    reproduces it (series[21] carries the castle's raw life for the
    jump-size test; the threshold is 5000).
17. **The pause key's press witnesses.** P centers the cursor and backs
    it up: pos_backup := (320,200) in the consuming frame (mc2l24
    t=7966: the fallback fed P a frame early and the whole pause ran a
    frame ahead). The UNPAUSE press writes nothing new — but the world
    itself is the witness: while paused the roll/pitch deltas are
    FROZEN, and the first frame they move is the frame the press was
    consumed (mc2l24 t=7978: P held parks 7978-7979, the source thawed
    at 7980). Feed no P while frozen; a pending press edge lands at
    the thaw frame.
18. **Shift+K is suicide** (PlayerInput case 0x25: carpet life := -1).
    The witness is the carpet's alive flag flipping off — RESET_KEYS
    with the alive series slot (mc2l5 t=17379: the K edge showed at
    park 17379 but the carpet died at frame 17380; the retake killed it
    a frame early).
19. **A non-bar menu's open frame snapshots the live cursor pixel**
    into pos_backup — mid-poll, no park carries it, and the synthesized
    cursor lands a few px off (mc2l19-taketwo t=10942 menu 10:
    (268,222) vs (270,222); mc2l6-rival t=69 menu 3: (326,38) vs
    (324,198)). pos_backup is the cursor-RESTORE source at the menu's
    close, so the wrong snapshot forks the pose at close — the take's
    own row carries the real snapshot from the open frame on;
    transplant it at the open edge.
20. **A dead Ctrl/Enter tap must not open the menu.** A sub-park tap
    that the source's polls never saw leaves the menu byte flat AND
    pos_backup unchanged (a real one-frame menu would still snap the
    cursor at its open) — feeding it opens the retake's menu for a
    frame the source never had (mc2l24 t=34867: one-park Ctrl tap, the
    retake's menu opened at 34867 and posbak forked for 19k ticks).
    The oracle drops the tap when the witness stays flat; a real hold
    self-corrects through the next frame's park image.
21. **Modal dialogs take the real cursor, not the pose synth.** The
    modal MenuStates (9, 10-14 — options, volume, okay/cancel: the
    flight mover never runs there, the take's roll/pitch deltas are 0)
    give the cursor ONE job: the dialog's buttons. The synthesized
    pixel reproduces the frozen pose, not the button (mc2l22-new
    t=10570: the dialog-close click landed at (313,204) instead of
    (326,380) and menu 9 never closed). Feed the source's real cursor
    there exactly as in the bar menus — EXCEPT at the close frame,
    which is a flight frame again and takes the exact consumed pose
    (mc2l19-taketwo t=10947: the real cursor's park pixel folded to
    (-6,-6) where the source consumed (0,0)).

**The 2026-09-27 attribution round (the wave-6/7 re-grade's forks):**
22. **CLICK-cadence hands: dw_0 drives the latch outright.** The arm-b
    model (held && charge > 0 clicks unaided) holds ONLY for RAPID
    spells (token byte_0x3B_59 == 0: the fold's else branch fires on
    `edge || (held && word_0x2E_46 > 0)`, PlayerInput.cpp:2051/2071 —
    verified instruction-identical at NETHERW.EXE 0x3D845/0x3D8CF). A
    CLICK-cadence hand (byte_0x3B_59 == 1) NEVER clicks without an
    edge, and the release oracle's held-through erases exactly the
    0->1 transition the guest's own UpdateMouseEventData would have
    turned into an edge (mc2l10 t=522: the source consumed the
    mid-frame re-press, the retake's held continuation folded nothing,
    the fire cycle walked a frame off for good; the hand was fireball
    t0, steps 5 — the "steps 5 == RAPID" reading was wrong, the
    cadence byte is the authority). scan_take now carries both hands'
    cadence bytes (token +0x3B, series[22]/[23]) and the supplement
    feeds the latch for CLICK hands wherever dw_0 says the click was
    consumed — never beside the other hand's composed latch (two
    latch bits in one frame are the both-buttons MAP-OPEN,
    PlayerInput.cpp:597). The "auto-fire grid" the open list hunted
    is NOT a fold timer (HandleMouseButtons has no counter — retail
    bytes verified): it is the token's word_0x2E_46 cooldown ticking
    in the entity pass, and whether a tap at charge == 1 is accepted
    depends on the carpet-vs-token pool-slot dispatch order. Validated:
    mc2l10/mc2l13/mc2l22 byte-IDENTICAL over their full takes.
23. **The modal CLOSE frame splits on whether it consumes a click.**
    "Closing" for a modal must test the MODAL set — 9..14 are not in
    SPELL_BAR_MENUS, so the item-21 bar test read "closing" on EVERY
    modal frame and the dialog got the pose cursor instead of its
    buttons' real cursor (mc2l22-new t=10570, the close click landing
    at (313,204), menu 9 never closed — the late-09-26 driver
    regression behind the wave-6 -b/-c flip). With the test fixed the
    rule refines item 21: a close frame that consumes a click edge
    needs the REAL pixel (the dialog's button; the mover doesn't fold
    that frame — the source consumed (0,0) at mc2l22-new t=10570 with
    the click at (326,380)); a key-closed dialog's close frame is a
    flight frame again and takes the exact consumed pose
    (mc2l19-taketwo t=10947).
24. **The bar menu's close LAW.** Menu 5/8 closes at the first frame
    whose fold sees Ctrl AND both buttons up (MBS 0x10/4/8 all clear —
    PlayerInput.cpp's menu-5/8 LABEL_122; Ctrl opens the bar from
    flight via MBS bit4 -> HandleButtonClick(20,5)). So a frame the
    take's menu byte SURVIVES (bar at both records) provably was NOT
    all-up, and a composed all-up frame inside a surviving window
    means the Ctrl release was dated a frame early — its usual
    witness (the menu byte) can't move while a held button keeps the
    bar open (mc2l15 t=34977: the source's bar rode a bar-pick press
    to t=34984; the retake closed at 34977 and the restore-frame poll
    swung the pose). The driver holds Ctrl for the offending frame —
    inside 5/8 bit4 votes only on the close law.
25. **The sub-frame modifier dip.** With Shift (or Alt) held the fold
    takes the modifier branch and the arrows emit nothing — yet dw_0
    can still carry a movement notch (mc2l20 t=20284: Shift+arrows
    held from ~20278, dw_0 flat through 20283, then ONE notch 0x0a
    (cmd_speed 80->64, strafe 0->16) with Shift still held at every
    park). The modifier blinked for exactly that frame — a sub-frame
    release+repress no park carries (the mirror of the sub-frame
    arrow tap; a full plain-branch frame would notch every later
    frame too, the arrows stay held). Drop the modifier for the
    notch frame — flight frames only (the menu-5/8 path emits arrows
    modifier-blind), never Ctrl (bit4's flight job is opening the
    bar), never Alt beside a dated cheat (the handler's gate reads
    pressedKeys[ALT]).
26. **The demolish no-op outcome filter.** Retail gates opcode 42 on
    a mid-frame queue/rebuild state the parks can't see; a press
    landing inside the gate is silently dropped (mc2l12 t=19598: a
    repeat 2 frames into the rebuild; mc2l5 t=39979: the castle at
    full 20000; mc2l11 t=11164: plain L at 11163, Shift at 11164 —
    the source's Shift dispatch DID consume the latch (cleared it),
    the demolish was gated downstream). The take's castle life is the
    outcome oracle: any consumed demolish lands it at exactly -1
    within the consume window — no -1, withhold the LastPressedKey
    write. The latch-lifecycle form (plain-L rise, the Shift frame
    arriving inside the same hold, typematic re-arming the source's
    latch) scans ahead to the first Shift frame; no opinion when
    another latch-worthy key rises first.
27. **byte_0x457 rides the menu-window transplant.** A bar pick can
    fail to fire in the guest although every fed register is
    verifiably correct (mc2l8 t=917, mc2l50 t=401: MBS edge consumed,
    the commit byte stays 0, the whole equip chain forks — confirming
    the earlier probe's "deciding state out-of-closure"). The byte is
    IN the take's closure; the per-window bar assist (already writing
    +0x3FA/+0x458/+0x459) now writes it too, and the guest's own
    commit path (v16 -> opcode 41/31/32 next frame) replays it from
    the take's value. Validated: mc2l8 byte-IDENTICAL over its full
    take.
28. **The full stop's idle dating.** RESET_KEYS dates Backspace by
    cmd_speed hitting 0 — blind when cmd_speed is already 0 (idle),
    so the fallback dated the press a frame early (mc2l16 t=27063:
    the retake zeroed the input a frame early, delta (10,-4) vs (4,0);
    the source fired at 27064, where the human's pixel happened to
    sit near center anyway). The fire frame's signature rides the
    take's own envelope: inputs zeroed means delta == _g(-now) on
    BOTH axes (PlayerEvents' envelope law, EF:38352: delta =
    _g(2*input - now_prev)) — a recentering pixel produces the same
    numbers, so redating is effect-neutral when it matches. The
    driver defers the Backspace latch to the signature frame.
29. **The fly-assistant's watch cells ride a tail transplant.** The
    48-frame idle trigger (sub_1A7A0: press-pos unchanged &&
    PlayerAction == 0 && byte5 == 0 -> D41A0+0x36DF0++, fire opcode
    39 past 0x30) watches cells in the struct TAIL (>= 0x36DEA —
    recorded but never graded, "informational"), so they drift
    silently and the full stop fires a frame off (mc2l24-crazy
    t=18698). The driver transplants 0x36DEC..0x36DF2 per window from
    the take's record. (mc2l16 t=27063 looked like this but was item
    28 — the assistant is DISABLED in that take; its watch cells sit
    at 0.)

Open classes (mapped, not yet fixed), after the 2026-09-27 round: the
cursor-PIXEL aim (a spray cast reads the pixel, not the consumed
roll/pitch; mc2l50 t=771; mc2l24 t=35669 — the beam's aim entity
integrates the live cursor pixel between pulses: dw_0 and the button
stream match, the aim accumulator doesn't); takes with SOURCE-side
recording gaps (l18 — 13 divergent ticks astride the gap at t=51846,
l0-galore — freerun can't cross; wants a mid-take re-anchor or a
re-record); takes RECORDED on the boot-variant pool base (mc2l21: the
source itself carries base 0x31cf6e-side init-settle state — the
anchor transplants t=0 exactly, t=1 forks on the hand tokens' charge
fields 0x0f vs 0x09; the mana canary can't see it; wants a serial
re-run or a fresh take); and the known-unfixable old-binary takes
(mc2l4/mc2l4-new, below). Retired by the round: the auto-fire grid
(item 22), the modal close (23), the bar close (24), the Shift+arrow
suppression gap (25), the Shift+L gates incl. the LastPressedKey
lifecycle (26), the out-of-closure menu pick (27), the idle full stop
(28), the fly-assistant trigger (29), and the toast typing cadence
(the "WINDY" lane — a display-phase slip, now demoted compare-side
with the hit-flash counters: names_81 + byte_0x3E2, see
compare_mgcr.py's VOLATILE_KEEP_NATIVE). The BOOT-VARIANT init-settle
race (t=1 forks with pool base 0x31cf6e instead of 0x31cec6 — the
guest's level init hasn't finished its deferred steps when the anchor
lands: the carpet's mana is re-zeroed at frame 1 and the fly-assistant
idle counter freezes — mc2l50 wave3, mc2l6 wave5; intermittent,
host-timing-dependent, a serial re-run boots clean); the retake-side
capture gap (a missed mailbox window starves the guest of one fed frame
— the stream shifts a frame out of phase PERMANENTLY from the gap tick;
wave3's 4-way parallel runs produced 7 takes so tainted (l15/l16/l18/
l20/l22/l22-new/l31, first divergence == first gap tick) — re-run
serially; the compare's gap bridging aligns the t's but the guest's
world missed a fed frame, so the tail is garbage by construction).
Likely contributor: the /tmp partition pressure — each retake streams
~20 KB/tick to /tmp/retake-fix, and a nearly-full partition stalls the
driver loop past the capture window (wave3's gapped takes all came
from the 4-way parallel run; serial runs on the cleaned partition are
gap-free so far). A
canary now guards the boot-variant class: an anchored run whose
carpet mana at t=1..3 doesn't match the take's is aborted loud
("boot variant ... re-run") — the deferred init's mana re-zero is
detected at frame 1 instead of after a 20k-tick garbage compare.

**The three unclean takes (2026-09-27 night) — the EMULATOR BUILD is
part of a take.** The MC2 corpus was recorded under dosbox-staging
0.82.2; the retake rig ran DOSBox 0.74-3. The same exe, the same state
and the same feed replay differently on the two, through two retail
reads of memory the game never wrote. Both "open classes" above are
superseded by this: mc2l21 is NOT a boot-variant take and mc2l24 is NOT
a cursor-pixel aim.

- **mc2l21, t=1 — the severed StageVar watch reads DOS/4GW's code.**
  StageVar row 5 (kind 5, `&2` clear) holds the 67 goats at slots
  78..146; its union is the autosave-severed offset 0x6078 (147 x 168).
  `sub_12780` dereferences it unguarded: FIRED when the dword at linear
  0x6080 is negative or byte 0x6085 has bit 2 set. That address is the
  real-mode half of the DOS extender, and the image loads ONE PARAGRAPH
  higher under staging (PSP 0x193, against 0x192 under 0.74-3), so the
  same code bytes sit 16 bytes further on:

  | build | dword @0x6080 | byte @0x6085 | row 5 |
  |---|---|---|---|
  | dosbox-staging 0.82.2 | 0x0FC08ED8 | 0xE0 | never fires (the source) |
  | DOSBox 0.74-3 | 0x38E6830F | 0x36 | fires in the lead-in |

  Under 0.74-3 the guest's own row (struct tail, kept native by the
  anchor) is FIRED before t=0, and the transplanted goats are released
  by frame 1. Row 6 reads the interrupt table (0x158: F000:1060,
  negative) and fires on both. mc2l4's row 2 (0x7230) is the mirror
  case — fires under staging, not under 0.74-3 — and still retakes
  clean on 0.74-3 because the source's row had fired BEFORE its t=0:
  the release rides in with the anchor and nothing is held any more.
  The retake's tail still carries the wrong flag there.
- **mc2l24, t=35669 — the hydra's `v34`.** `sub_29A90` reads
  `[ebp-0x10]` uninitialised (the `mc2l24-hydra-v34-parity`
  deviation). Measured in the running guest: the game loop's esp is
  linear 0x355244 under staging (0x315244 under 0.74-3), the slot is
  232 bytes below it, and at the frame hold it carries what the frame's
  tail left — odd and above 4 on 81..89% of frames under staging, which
  is the source's "skip the wander draw" at all nine ticks. Two staging
  runs carry the SAME value there on every one of 4,001 frames
  (t=34600..38600); under 0.74-3 another call tree writes that depth on
  most frames and the first of the nine reads even. Interrupt frames
  DO land on the game's stack (DOS/4GW at ring 0, no stack switch:
  EFLAGS at esp-4 of the hold spin on ~10% of the samples), so a
  residue read stays exposed to an interrupt between the frame's start
  and the read. `--release-phase MS` (experimental) releases each frame
  at a fixed phase of the guest's 120 Hz timer, which pins where the
  timer interrupt falls inside the frame: 7 of 7 runs passed the nine
  ticks with it (phase 0, 7 and 8 ms), against 3 of 4 without. That is
  too few runs to name the timer as the cause of the one miss (the
  sound card's interrupt is the untested candidate); a retake of a
  hydra level is verified against its source and re-run on a miss.
- **mc2l24-crazy, t=74206 — a bar pick fed a frame early.** The menu
  commit witness was "the toast counter jumps UP". A spell-xp toast was
  still counting at 124 when the pick re-armed the counter to 19: a
  step DOWN, the witness stayed blind and the release went at window
  74205 instead of 74206. The witness is now "any step other than the
  countdown's -1 onto a positive value". Census: 23 such down-steps in
  the corpus, 3 inside a bar menu, and this is the only feed frame the
  rule moves (plans of the three takes diffed before and after).
- **mc2l24-crazy, "stall after 79,000" — the driver waited for a frame
  that never comes.** The guest had run the take's last frame
  (counter 79070 = t 79064, rng equal to the source's) and was
  released; the drive loop only noticed the end of the take at the
  NEXT window. After a level's last frame the guest loads the next
  level and opens one — except on level 24, the campaign's last, which
  goes to the outro. The loop now stops once the take's last park is
  captured. (mc2l24 only ever finished under 0.74-3 because its forked
  world never completed the level.)

Verdicts, dosbox-staging 0.82.2, driver with both fixes, against the
source: mc2l21 IDENTICAL (22,171 ticks), mc2l24-crazy IDENTICAL (79,065),
mc2l24 clean on 10 of 11 runs through the nine hydra ticks, two of
them run to the end and IDENTICAL over 54,058 ticks (one run
forked at t=38523, the last of the nine, with every struct byte equal
to a clean run's up to t=38522). Under 0.74-3 mc2l24-crazy is IDENTICAL
too; mc2l24 forks at t=35669 on every run, and two runs of the same
feed fork from EACH OTHER at t=36293. Staging needs a real SDL video
driver (the dummy driver aborts on the OpenGL probe): run it on an Xvfb
display with `output = texture`, audio on the dummy driver; its snipe
calibration is its own (struct @linear 0x356038, pool base 0x35CEC6 —
the corpus's base).

**A retake starts at the level's first frame (2026-09-28).** The
corpus's takes start 2..11 turns into their level (the recorder's old
go-live probe). A retake captures the windows before its source's first
record too, from turn 1, with the level-init record in front, and the
result is A WHOLE TAKE IN ITS OWN RIGHT: numbered from its first frame,
every record with its `input` / `state.ext` and its `feed`. For the
first frames there is no source record to hand the input lanes on from,
so they are written down as the recorder would have sampled them — the
registers as they stood when the frame ran — and the `feed` says what
the driver wrote for the next one. `--no-lead-in` restores the old
shape.

A retake of such a take does not know there ever was a gap. It reads
the take as it stands, anchors on its first record (turn 1) and replays
the feed from there — straight through the frame the first retake was
anchored at, with nothing written over it. So stage 2 = stage 3 proves
the take has no seam, in every lane.

The header keeps the provenance: `capture.lead_in` (`t_shift`,
`windows`, `dropped`, `first_turn`, `source_t0_turn`, `cursors_b64`,
`recipe` where the take has one), handed on to later generations
(`handed_on`). `t_shift` is how far the
take's tick numbers sit ahead of the ORIGINAL recording's, and it is
used for one thing only: holding a take against the original.
`compare_mgcr.py` and `lane_census.py` take it off (the first frames
read as negative t): A MAY BE A LATER ANCHOR OF B — B's records before
A's first are skipped with a note — and two takes that both carry the
first frames are compared over them as well. ⚠ The port's reader does
not know `t_shift` yet: a stage file put in `recordings/` as it stands
would move every tick reference (fixtures, known-deviations, the brief
baseline) by its shift.

The lead-in frames are the guest's own world, run from level init; the
anchor still lands on the source's first record. Whether the two meet
is measured at every retake: the guest's frame at the anchor window is
read BEFORE the transplant and held against the source record, judged
like a `--retake` compare plus the terrain planes, and stamped as
`capture.sync` (`identical`, `struct_bytes`, `terrain_cells`, `where`,
`first`). Where the verdict is not `identical` the take has a seam at
the anchor: the lead-in shows the level's first frames as a fresh boot
plays them, not the frames the recording session had. (Six takes of
the MC2 corpus had one; their recipes close it — below.)

What the driver writes in the lead-in, and why:

- **The attitude path.** The human's mouse before the first record is
  unknown, but the fold keeps no memory of it other than the applied
  roll/pitch (`applied += trunc((2 * consumed - applied) / 4)`, player
  `+0x53B/+0x53D`; the step at `+0x3EA/+0x3EC`; `+0x399/+0x39C` are the
  previous applied values halved; the carpet's `+0x1E` is the pitch).
  The source's first record gives the applied value and the last step,
  the guest's own park gives where it stands, and `lead_in_path`
  solves, per axis, the consumed values in between; the cursors are
  synthesized from them and land in the first frames' `feed` and
  `input` (the header's `capture.lead_in.cursors_b64` repeats them,
  window turn -> cursor).
  The path may not leave the span between its ends: the applied roll
  TURNS the carpet from |8| on (`yaw -= roll / 8` per frame — a first
  solver that swung to -18 and back left mc2l2-new's yaw at 2044 for
  0).
- **A take that starts inside the spell bar** (mc2l7, mc2l19: Ctrl was
  pressed before the first record). The frame that opens the bar
  snapshots the live cursor (`+0x3FA`): it is fed the source's
  snapshot. The frames after it read the live cursor for the hover:
  they are fed the source's own cursor. The category (`+0x83E`) is the
  session's — the last one used, levels ago — and rides in at the first
  window.
- **What is the session's goes in at the first window**: the
  SESSION_CONSTANT fields (the profile's wizard name, the settings
  block, the TMAP table), as cells of that window's feed. They are
  constants of the recording session; with them in from turn 1 the
  frame the first retake was anchored at changes nothing in them.
- **The spellbook snipe must have landed.** With the anchor alone a
  missed snipe is repaired at t=0 and nobody notices; the lead-in in
  front of it would be a fresh spellbook's world (one spell token
  fewer, no xp). The driver aborts ("snipe TOO LATE", retriable). Seen
  on 3 of ~120 short boots run five at a time, never in the 78 full
  retakes of batch 2.

- **The take's lead-in recipe** (`--lead-in-recipe JSON`, or
  `$MGC_LEAD_IN_RECIPES/<take>.json`; the six recipes of the MC2 corpus
  are in `tools/lead-in-recipes/`). What the attitude path cannot
  bring: the state of the recording SESSION that a fresh `-level N`
  boot has not, and input the player gave before the first record. See
  "The lead-in recipe" below.

Survey of the guest's own sync frame, 39 takes, staging 0.82.2
(terrain and the RNG equal on all 39, no lead-in window dropped):

| | settle cursor only | with the path | with the recipes |
|---|---|---|---|
| identical to the source's first record | 19 | 33 | 39 |
| flight attitude, 1..6 bytes (12 takes) and the bar-menu start (2) | 14 | 0 | 0 |
| session state: recycle cells, the rivals' learned-spell flags (player `+0x7CF..`), the level-start autosave (mc2l30-new, mc2l31, mc2l32, mc2l33, mc2l4-new) | 5 | 5 | 0 |
| input before the first record (mc2l24-crazy: a fireball) | 1 | 1 | 0 |

### The lead-in recipe (2026-09-28)

Six MC2 takes do not meet their source's first record on the attitude
path alone. What separates them was measured on the guest's own first
frames (a short retake with `MGC_SYNC_DUMP`), and it is three things.

**Session residue.** Bytes the level never touches once it runs:

- the RECYCLE CELLS (`0x11EA`, 1000 pointers). `sub_49F90` writes them
  — it rebuilds BOTH entity stacks by scanning the pool from slot 999
  down: free slots onto the free stack, live entities with flags byte 2
  `& 2` onto the recycle stack — and every caller but level init sets
  the top back to -1 right after. Nothing reads a cell above the top,
  so the cells are a fossil of the last rebuild, and under them of the
  levels the session played before;
- the rivals' learned-spell flags (`str_611 array_0x3E9`, player
  `+0x7CF`, 26 bytes): carried from level to level.

Both are written at the first window, the recycle cells as slot numbers
(the pointers are made for the guest's pool base).

**The level-start autosave.** `sub_57640` saves the level once
(`SaveLevel_55080(1, …)`, gated by `setting_38545 & 0x80`, BYTEARRAY_4
`+0x9691`); a fresh boot runs it in frame 3. The save is not inert:
`sub_55100` turns the struct's pointers into offsets and back (that is
what severs a StageVar row — mc2l21, mc2l4), and `sub_49F90` rebuilds
the stacks: THE FREE STACK COMES OUT SORTED BY SLOT, the recycle cells
are rewritten from the pool of turn 2. Whether the recording session
saved is read off the take:

| take | witness | the session |
|---|---|---|
| mc2l30-new | free slots 58, 59, 60 lie in the order they were freed; the guest's save sorted them | did not save in frame 3 |
| mc2l32, mc2l33 | the recycle cells still start with the level-init list (11 and 9 entities); the guest's save wrote the 40 and 127 of turn 2 over it | did not save in frame 3 |
| mc2l4-new | StageVar row 2 (tail `0x36605..`) holds the severed offset `0x7230`, as the guest's tail does from turn 3 on | saved |
| mc2l31 | the TMAP table only (below): no flagged entity, the free stack sorted either way, no pointer row in the tail | the guest's save is not the session's |

Where the session did not save in frame 3 the recipe sets the bit at
the first window (`ba4_or`), and the guest does not save either.

**The save rebuilds the TMAP table** (`sub_71930`: `array_0x39`, one
byte per sprite the asset manager holds). A fresh boot holds the
level's sprites; the session of a hidden level held those of the
levels before it too, and its table shows them (22 / 123 / 61 / 114
bytes off a fresh boot's on mc2l30-new / mc2l31 / mc2l32 / mc2l33;
none on mc2l4-new and mc2l24-crazy). The table is a session constant
— written at the first window, left out of the world compare — so a
guest that saves in frame 3 carries the fresh table from there up to
the old sync frame, and a retake of that retake carries it for good:
mc2l31, run with the save left on, met its source in the world and
still differed from its own retake in `struct:session` on every tick
from the old sync frame on. Its save is off like its sisters'.

**The save's stamp.** Every source carries `dword_0x36DF6 =
&str_D7BD6[59]` (tail `0x36DF6`, `ac 93 2a 00` under staging) — the
save writes it (`Level.cpp:184`), and the port and remc2 read it as the
row base of the type table. The takes whose save is off carry it too:
their session carried the stamp in from the level before (or saved
before the level's first frame ran — the stacks of a pool still in its
init state rebuild to themselves). A guest whose save is off never writes it,
and the tail is the guest's own in every retake (the anchor stops at
the pool's end) — the first three-stage run of these takes differed
from its source in `struct:tail` on every tick. So the stamp goes in
at the first window with the rest, found by a second probe made with
the save off (`seam_recipe.py --nosave-probe`). ⚠ A byte of the tail
that moves on its own (the per-turn counter at `0x36E02`) is not a
mark of the save: a mark changes in the save's frame and in no other.

**Input before the first record** (mc2l24-crazy). The source's first
record (turn 6) shows a fireball of the player's left hand in slot 139:
life 16 of 21 and 5 x 383 away from the carpet's own y — launched by
the frame of turn 2 (a projectile is moved in the frame that launches
it: an in-take cast shows life 20 there); 100 mana spent; the press
register still at the cursor's start (320,200). The click is fed at the
first window. The fireball's pitch (`+0x1E` = 4) is the carpet's
applied pitch of the launching frame, so the attitude path is PINNED
there (`pins`, `lead_in_path`). How long the button was held leaves no
trace in the state — 1, 2 and 3 frames reach the same sync frame; one
frame is fed.

A take that carries its first frames is judged at its own first
window: `capture.sync` there holds the guest's frame WITH what the
record says that window wrote (the session's constants, a recipe's
residue) — those bytes are the frame as recorded, not a seam.

A recipe is data, made once per take by `seam_recipe.py SOURCE PROBE
PROBE.sync.bin` (`tools/retake-rig/`) — it sorts every
differing byte of the sync frame into recycle / stack / learned, reads
the autosave's witnesses, and REFUSES a take with bytes of any other
kind (mc2l24-crazy's recipe is hand-made). The format is in the
driver's comment at `load_recipe`. What a recipe writes lands in the
first frames' `feed` like every other write, so a retake of the retake
replays it and knows nothing of the recipe; the header keeps it as
`capture.lead_in.recipe`. A recipe whose window never came (a late
lead-in) aborts the run.

The recipes are a one-off of the corpus conversion: a take recorded
from the level's first frame has no lead-in to fill.

**The whole MC2 corpus as whole takes, 2026-09-28** (the 39 takes of
the batch; mc2l24 was run apart, below; staging 0.82.2, five guests at
a time;
the takes in `re-recordings/`, the board and logs in `tools/retake-rig/2026-09-28-mc2/`): stage 1 vs 2,
2 vs 3 and 1 vs 3 IDENTICAL on 39 of 39, the sync frame IDENTICAL at
both stages on all 39, and the lane census names `struct:native` alone
on every pair — 1v2 the kept-native fields, 2v3 a dozen bytes of them
on the first frames (the clock). THE CENSUS IS THE JUDGE OF A SEAM,
not the world compare: the session and tail lanes are outside the
compare, and both of this round's mistakes (the stamp, mc2l31's table)
passed all three compares. 79 retake runs, no rig failure, ONE world
miss: mc2l18's first stage 3 forked from its stage 2 at t=5619, where a
many-segment creature ((5,22), slots 647..) is born with every
segment's z 192 apart from the other run's — the hydra's family (a
spawn that reads what the frame left behind); the pipeline's one
re-run was IDENTICAL.

**mc2l24** (the hydra: an uninitialised stack slot read at nine ticks)
meets its source's first record on the attitude path and passed the
three stages at the first go. Retaken on from its own product to
generation 7: generations 2, 3, 4 and 6 IDENTICAL to the source,
generation 5 forked at t=37955 and generation 7 at t=38523 (hydra
ticks both). A MISS IS NOT INHERITED — generation 6 was made from the
missed generation 5 and is the source's world again: a retake replays
the feed and computes the world anew, so every run rolls for itself,
and the compare against the source says which run is the take. Under
staging the roll has missed 3 times in 17 runs.

**The port's grade of the 40 whole takes** (`replay --segmented
--brief`; `tools/retake-rig/brief_board.py`, `trim_lead_in.py`). From
the source's first record on, 80 of 80 rows (stages 2 and 3) are the
baseline's, byte for byte. From turn 1, 29 takes are the baseline's
with the shift taken off, mc2l24 differs in its roster alone (the nine
rules are keyed by tick), and ten takes show ONE deviation each and
are clean behind it: THE PORT DOES NOT MODEL THE LEVEL-START AUTOSAVE.
Six take another slot at the first allocation behind the sorted free
stack (mc2l3, mc2l3-new, mc2l5, mc2l7, mc2l23, mc2l30), three deviate
in the save's own frame on the severed StageVar row (mc2l21, mc2l4,
mc2l4-new), mc2l14 at t=5938 is not dug. The four takes whose session
did not save are clean from turn 1.

`--frame-rate FPS` (retake-only speedup): rewrites InGameLoop's native
frame-budget byte (`add esi,5` — the ~120 Hz PIT count budget) in the
RUNNING guest before the level loads — the exe on disk is never
touched, and patching pre-load means the dynamic recompiler only ever
translates the patched bytes. Under the lockstep the guest waits for
the driver at every frame, so any rate is safe and the stub's floor spin
is dead time: the driver rewrites the floor to 0 in the running guest
(`--fast-floor N` overrides). Measured 2026-09-27: `--frame-rate 120`
sustains ~75 fps, against the native 24. Without the lockstep
the floor bounds the window (2 = >= 8.3 ms; 1 risks near-zero windows; 0
is refused) and ~40 fps is the floor-2 ceiling. The floor signature is
the MC2 stub's own (`8b 82 disp32 / 05
imm32`: a register-indirect GameTimerTurn read and an imm32 add — until
2026-09-27 the driver matched the MC1 encoding, never found the site, and
reported it as "no limiter site found"); the limiter and the floor are
now reported separately. `--abort-on-gap` aborts loud at the first missed
capture window — a missed feed leaves the guest folding on stale
registers and the tail is garbage by construction; with the lockstep a
gap needs a stall longer than the hold's timeout.

**The barrel-roll abort assist.** sub_55C60's abort check (at phase >=
4: `|mouse.x - byteindex_220| > 16` -> phase 8 -> finish) compares the
LIVE cursor — a mid-frame poll no park sample carries — against the
arm-time snapshot in BYTEARRAY_4 (outside the closure). The synthesized
cursor reproduces the consumed roll, not the pixel, and the park cursor
during a roll is game-written, so the check can flip either way
(mc2l3-new t=9944: the human moved the mouse since the arm and the
source's roll aborted 3->8->0; the retake's fed pixel sat within 16 of
its own arm snapshot, the roll continued, and the pose forked for
good). The driver watches the take's own phase byte (player +0x846,
series[17]) and writes byteindex_220 (BA4+0xDC, a signed word) ahead of
each live-roll frame: the fed x to suppress a spurious abort, fed x +
32 to force the one the source took.

**Old-binary takes can't freerun the fixed exe.** mc2l4/mc2l4-new were
both recorded before 8b9c8a5 (the volcano/stagevar exe guard): their
t=0 already differs from a fixed-binary boot (the stagevar pointer
corruption reads low memory as booleans at level init — the take's
pool has 97 recycled slots and divergent spawn flags while the RNG
matches). No instrument fix bridges undefined behaviour; such takes
must be re-recorded with the fixed binary.

### The whole takes are the corpus: ticks from 0 (2026-09-28)

⚖ Ruled 2026-09-28: the whole takes ARE the MC2 corpus, each one its own
take. Record 0 is game turn 1, and no reader learns a shift — the
`t_shift` in `capture.lead_in` is what the retake driver needed to sync
on its source, and provenance after that.

What follows a take's numbering, and what does not:

| what | numbering |
|---|---|
| `conformance/known-deviations.json` | THE TAKE'S. A tick-keyed rule scopes a boundary of the take as it stands, so `mc2l24-hydra-v34-parity`'s nine ticks moved by the take's shift (+5), once |
| `conformance/brief-baseline.txt` | the take's: the 40 MC2 rows are a sweep of the whole takes |
| `conformance/fixtures/mc2*/`, `conformance/mc2*.json` | THEIR OWN. ⚖ Ruled 2026-09-28: a fixture is the evidence — a pair of retail states for a LEVEL — and the recording it was cut from is a source of more evidence, a disposable one. Its `t` is the key between the manifest and its file and nothing else. NOT MOVED |

The per-take shifts (1..10 records) are tabled in
`tools/retake-rig/2026-09-28-mc2/t_shift.txt`. Where a fixture sits in
today's take is a lookup, not a property of the fixture:
`tools/retake-rig/verify_fixture_shift.py` finds **326 of the 347 MC2
fixtures in their whole take at `t + shift`, equal on all their
records** outside the retake's kept-native and session-constant lanes
(`compare_mgcr.py`), obs and input lanes included. The other 21 are the
`mc2l0` fixtures, cut from an earlier, 22,695-tick take of that name —
the ruling's own case: they are in no take of the corpus and guard
their laws all the same.

⚠ PROSE FOLLOWS THE FIXTURES. An MC2 tick cited in a comment, a fixture
note, a roster note or a ledger entry written before 2026-09-28 is in
the ORIGINAL take's numbering: add the take's shift to find it in the
corpus.

**The save's frame is in the take.** A whole take carries the level's
first frames, and with them retail's level-start checkpoint autosave
(`sub_57640`, from `PaletteChanges_47760`'s third step — the first call
of frame 3). Its witness inside the closure is the save's stamp,
`dword_0x36DF6`: zero in records 0 and 1, `&str_D7BD6[59]` from record 2
on, in all 36 takes whose session saved; set from record 0 in the four
whose session had saved before the level began (mc2l30-new, mc2l31,
mc2l32, mc2l33 — their recipe writes it at the first window). Every
retail driver reads the save off that edge
(`mgcr::mc2_autosave_lands`) and runs the port's own save at the top of
the tick into the stamped record
(`World::mc2_replay_checkpoint_autosave`; docs/CONFORMANCE.md,
"the level-start autosave").

### MC1 / Hidden Worlds retakes (2026-09-28)

`tools/mc_dosbox_retake.py` hands an MC1 / HW take to its MC1 arm,
`tools/mc1_dosbox_retake.py` (same command line;
`tools/retake-rig/retake_staging.sh` picks the game directory and the
conf by the take's header). The rig is the same — init park, lockstep,
lead-in, sync verdict, feed lane, whole take from the level's first
frame. What is MC1's own:

**The frame is built the other way round.** MC2: fold, consume, tick,
draw, window. MC1: fold, consume, WINDOW, tick, draw. A record shows
the frame's input folded and consumed and its tick not yet run. A
register written at window k is read by the fold of frame k+1, and
the outcome is in record k+1 — the MC2 driver's window numbering.

**The sample of record f is what frame f folded.** The recorder
samples the registers at the start of the window, right behind the
fold. Measured on mc1l0 / mc1l1: the consumed move bits equal the
arrow keys of the SAME record on 99.9 % of the samples (the previous
record's: 97.8 %), a consumed fire bit finds its button held in the
same record on 1,674 of 1,676, the same record's cursor reproduces
the frame's roll / pitch step on 94-96 %. On MC2 an edge usually lands
BEHIND the sample; on MC1 it does not.

**The closure dates the input itself.** MC1 stamps what it consumed,
every frame, and the feed of window p is planned from record p+1:

| witness (local wizard) | names |
|---|---|
| Type_160 `+0` | the consumed move / fire byte: arrows 1/2/4/8, fire 0x10/0x20; exactly 48 = Shift+L (demolish) |
| Type_160 `+4/+6` with `+327/+329` | the frame's roll / pitch step and the applied value it stepped from: the stick, inverted exactly, fed as the cursor the fold maps to it |
| wizard `+1098` | the input mode (0 flight, 2 the spell menu): Enter |
| Type_160 `+940/+944` | the hands: a pick in the menu (a press latch over the hovered spell), a digit in flight |
| Type_160 `+772` | the digit bindings |
| wizard `+2`, the carpet's life | Space (leave a won level, revive), Esc (quit), Shift+K |
| every entity's `+63` and class | the pause: a paused tick draws the RNG and nothing else (P) |
| the message counter and text | the cheats (Alt+F1..F7) |

A key no witness covers is fed on the record that first shows it
down. The driver's `--check` runs the whole plan through a model of
the fold and holds the outcome against these witnesses (no guest);
`--plan FILE` prints the feed.

**Two registers the MC1 recorder does not sample** are written at
every window (zero when nothing is planned): the press latches
(`mouseLeftButton_12EFDE` / `mouseRightButton_12EFDC`, 6 bytes below
the held registers in both builds) and `lastPressedKey` (keys +
0x80). A retake's `feed.regs` carries the latches as `latch_b64`
(MC2's member) and the key as `feed.lastkey`.

**The spell menu's hover is the draw's.** The draw of frame k finds
the cursor inside a cell of the 4 x 6 grid (64 x 38 cells from
(384,162), `byte_99B88`'s order) and the fold of frame k+1 picks what
it found. The pass that closes the menu centres the cursor, so a
pick's cursor is the sample BEFORE the closing record; where that
sample is not inside the picked spell's cell the cell's centre is fed
(2 of 94 picks on mc1hwl0). The row height is measured (126 picks),
not read.

**`-custom` is not a menu session.** `-custom -level N` is the only
way into a level without the menu, and it sets the launch bit (bit 0
of `str_AE408 +1`). RETAIL'S WIN CHECK (`sub_415C0`, `(word & 0x110)
== 0`) skips a custom launch: the level is never won. The first full
retake of mc1l0 was the source's world byte for byte on all 7,098
ticks but for the win countdown (wizard `+0`) and the won flag
(`+2`), from t=3389 on. The driver clears the bit in the parked world,
before frame 1 (`--keep-launch-bit` leaves it). Its other readers are
the boot's, the menu loop's and a debug printf.

**`-custom` loads the French text table** (`data/ftext.dat`,
hardcoded in the same branch of TopProcedure; a menu session loads its
language setting's). The game's messages are copied into the closure,
so mc1l2's retake forked from its source when a rival died: "has
died." against "est mort(e)." (t=8298, the message slot alone). The
driver writes the take's language into the parked guest — the 80
strings and the pointer table, as `sub_44700` builds it — and reads
which language that is off the take's own messages (`--language`
overrides; English when the take has none).

**The carried spell book is written at the init park.** The per-level
seat init (`sub_3DD50`) keeps the wizard's book (Type_160 `+892`)
across levels and lists the spells the level allows; frame 1's join
spawns a token for each. A `-custom` boot has an empty book, so a
level above the first starts with no spells and every pool slot
behind them shifted (mc1l1: 1,015 bytes at the source's first frame).
The driver writes the seat init's OUTCOME for the source's book into
the parked world — the book, the spell list (`+532`), the hands, the
digit bindings — and frame 1 spawns the source's tokens into the
source's slots itself. Header `capture.init_feed.cells`; a retake of
the retake writes them as they stand. The init record is taken before
the write: it stays the guest's own level init.

**The lead-in and the sync verdict** are MC2's. MC1's Turn is the
per-frame counter at wizard `+18` (bumped by every command pass): a
fresh boot's first window reads 1, the corpus takes start at 9..24.
The record shows the applied attitude BEFORE the frame's step, so the
path runs from the guest's applied + step to the source's applied and
its last frame makes the source's step. At the first window what is
the session's is written — the seat init's scratch block at 11274 and
the wizard names (masked by `compare_mgcr.py --retake`), and the
player's display settings at 8597 (compared: the F-keys change them
inside a take; mc1hwl0's player flew with shadows off) — cells of that
window's feed. `capture.sync` carries `frame` where MC2's carries
`turn`.

**Pointers are not re-based.** The MC1 closure is full of guest
pointers, and every take of the corpus and every fresh boot under
dosbox-staging 0.82.2 has the struct at the same guest address
(0x1DE40). `compare_mgcr.py` compares them raw; the driver's anchor
(the transplant of what differs at the sync frame) refuses a take whose
struct sits elsewhere.

**Kept native** (`compare_mgcr.py`, `MC1_KEEP_NATIVE`): the level's
music track (struct `+576`; a `-custom` boot has no music), the
wall-clock stamp of the carpet's spawn (Type_160 `+379`; written,
never read) and the spell icons' blink counters (Type_160 `+844`, 24
bytes: the draw's, 2026-09-29). Three bytes per record on level 0.

First results (2026-09-28, staging, lockstep, ~75 fps):

| take | ticks | 1v2 | 2v3 | 1v3 | guest's own first frame |
|---|---|---|---|---|---|
| mc1l0 | 7,098 (+16 lead-in) | IDENTICAL | IDENTICAL | IDENTICAL | identical |
| mc1l1 | 10,710 (+18) | IDENTICAL | IDENTICAL | IDENTICAL | identical (the book written at the park) |
| mc1l2 | 10,589 (+12) | IDENTICAL | IDENTICAL | IDENTICAL | identical |
| mc1hwl0 | 53,054 (+8) | terrain forks at t=17754 (2026-09-29, the sound block written: IDENTICAL) | IDENTICAL | as 1v2 | identical |
| mc1hwl1 | 53,496 (+13) | terrain forks at t=368 (2026-09-29: IDENTICAL) | IDENTICAL | as 1v2 | identical (menu seed, frame 13) |

Compares are `--retake --obs`; the census of every pair names
`struct:native` and `wallclock` alone on the three MC1 takes. Logs:
`tools/retake-rig/2026-09-28-mc1/`; the three whole takes:
`re-recordings/`.

**A take that starts in the spell menu** (mc1hwl1: Enter is still down
in its first samples) opened it before its first record. The lead-in
feeds Enter for the opening frame — the LATEST frame from which the
source's attitude and heading can still be reached, since no frame
behind it folds a stick (`--lead-in-menu-frame` names it outright).
The roll's path has a turn to make as well: every tick adds applied
roll / 8 (truncated) to the heading, and a source that banked before
its first record has turned (mc1hwl1: by -5). `lead_path` solves the
attitude, the turn and the menu's stickless frames together.

**`-custom` boots with the sound off, and the row-0 gate reads the
sound driver** (2026-09-29; the two Hidden Worlds forks of 2026-09-28).
The branch of TopProcedure that loads the French text also clears the
sound and music flags (`byte_939E4` / `byte_939CC`, remc1 :41508): the
boot reads `sndsetup.inf`, never initialises the driver, never loads
the sound table. No tick hears a sound — but `sub_360C0`, the smoother,
gates a cell on `mapTerrainType[t - 257]` and `[t - 256]` WITHOUT the
u16 wrap: for a cell of the map's first row the two reads land in the
257 bytes below the type plane (`CC0DF..CC1DF`; Hidden Worlds 16 lower,
with the plane), the sound driver's globals, and a byte there that
reads as a building type (6..0x22) keeps the cell from being smoothed.
Cell (x, 0) reads bytes x and x + 1.

The port models those bytes one collapse at a time
(`Gen::OOB_TYPE_SHIM`, 23 building-classed bytes inferred from the
corpus). Measured on a guest whose sound init ran (the four flag stores
turned into `mov bl,1`; CARPET.EXE file 0x4C914, HIDDEN.EXE 0x4CED4):
the block holds 20 of the 23, no other, the same in both executables,
on every level, 360 ticks in, with and without a music driver
configured. The other three are the menu session's heap pointers:

| byte | global | what |
|---|---|---|
| 117 (119: its third byte) | `dword_CC154` | the sound table's BEGIN — loaded by the menu loop (`sub_5D070`), never by a `-custom` boot |
| 225 (227) | `dword_CC1C0` | the sound table's END |
| 101 | `dword_CC144` | the handle of the last sample started: follows the live audio, changes inside a take |

The driver writes the block into the parked world as a menu session's
sound init leaves it (cell frame `sound`, offsets from the block's
first byte = type plane - 257; header `capture.init_feed.cells`), and
the guest's sound STAYS OFF: every reader of those globals but the gate
tests the sound flags first. The two table pointers are written as
representatives of their CLASS (plain / building — all the gate tells
apart; third byte 0x1A, a building, second byte plain, as every session
of the corpus reads), never as the session's addresses, which no take
carries. Their low bytes are 0xC0 apart (the table unpacks to 0x5C0
bytes into a 16-aligned block), so begin and end are never both
building-classed.

What is the SESSION's — the two pointers' classes, the handle's from
which tick — is in the take's recipe (`tools/lead-in-recipes/<take>.json`,
`session.sound_table` = `{"begin": c, "end": c}`, `session.sound_handle`
= `[[t, c], ...]`, c = `plain` / `build`; default begin plain, end
building: the corpus's majority). The driver LEARNS it: it holds every
record it captures against the source's (the world lane as
`compare_mgcr.py --retake` judges it, a digest per record; the terrain
planes cell for cell), and where the first record that is not the
source's differs on the map's first row in the columns that read a
session's byte (100 / 101 the handle, 116 / 117 begin, 224 / 225 end)
it turns that class, writes the recipe and starts the guest over
(`relaunch`; at most 12 boots, `--no-learn` = one; the take's scan is
handed on, a boot costs the guest's start). A fork of any other
kind is reported (`FORK: t=…`, `watch: …` at the end) and the take goes
on (`--abort-on-fork` stops it).

**The generator's row 0.** The level generator smooths too, and it ran
before the init park, under the `-custom` boot's block: a handful of
cells on the map's first rows stand smoothed in the fresh boot and kept
in the source from its first frame on (mc1hwl2: (54,0) 2 | 7, (75,0)
7 | 11, (54,1) 2 | 3 — bytes 55 and 76; mc1l20: 7 cells). They show at
the sync verdict (the parked planes differ from the source's first
record by what the first frames do, too): where the planes differ
there on rows 248..8 alone the guest is started over and the source's
cells are written into the parked world with the block (cell frame
`terrain:<plane>`, offset = the cell). mc1l13: 68 cells of height and
28 of shading on rows 252..5; the four entities the generator stood on
them find their height by themselves.

**The lead-in's hand.** A player who flew before the first record left
the flight column as the witness: the forward speed (Type_160 `+12`,
16 a tick from an arrow up to 80, kept), `+14` (1 while the last tick
changed it), the side speed (`+16`, 16 a tick, back 4 a tick; both
side arrows down: the right one wins), the carpet's place. `lead_solve`
takes the stick of an axis as still but for ONE MOVE OR TWO, or a
glide from the centre (the
fold's filter, applied += (2 * stick - applied) / 4, makes the applied
value a clock of a move until it settles; the roll's also turns the
carpet, applied / 8 a tick) and the arrows of an axis as at most three
runs, keeps what leaves the source's attitude, heading and both speeds
EXACTLY, and FLIES the nearest of them through the mover itself
(`tools/mc1_flight.py`: remc1 `sub_46840` + `sub_455D0` after the
port's `mc1_move_duel_with`, on the port's sine tables — the next
record's place, predicted from a record's state, is the take's on
2,861 of 2,898 flight ticks of mc1l3; the rest are knocks and lifts).
The candidate that lands in the source's place — x, y, z, the pitch
flown — is the hand; where none does the nearest are NUDGED into it
(`lead_polish`: the filter forgets, so the stick of an early frame
moves the carpet's way and leaves the first record's attitude alone —
mc1l47, 3 units of y and 1 of z off under `pitch 44 from frame 7`,
lands in place with a pitch of 7 at frame 2; mc1l37 likewise). THE
GUEST'S OWN FRAME IS THE PROOF, and the judge where the mover cannot
be (what else hangs on the hand) — the guest is started over with the
next candidate where the sync verdict differs:

- in the carpet's place alone: where the guest stands as flown, no
hand the driver knows does better — the nearest is the take's and
the anchor has the rest (mc1l29: 2, 8 and 4 units of x, y, z — a
carpet that flew backwards and sideways through a turn). mc1l27,
mc1l37, mc1l43, mc1l47, mc1hwl8: identical at the first boot;
- in more than that: another ROLL history — what a lead-in's attitude
placed. mc1hwl7's player stood still and looked around: the
creatures of the level are placed by the heading of frame 13, 413
bytes of 156 entities under the shortest path to the source's
attitude, none under `roll 10 from frame 5, -12 from frame 11` (the
fifth boot).

What the boots found is kept in the take's recipe (`lead_in`: the
consumed roll, pitch and arrow bits of the frames from `from_frame`
on; `generator_cells`), and the next retake of the take starts with
it, in one boot.

**The spell menu's age.** Every draw of an open menu counts the blink
counters of the listed spells down (Type_160 `+844`, by spell id), so
a source that starts in the menu says how many frames drew it: the
opening frame is `frame - age` (mc1hwl4: 5 draws, Enter at frame 13,
where the latest reachable frame was 17). The counters themselves are
KEPT NATIVE in the comparison: the draw's own, and which icons a draw
paints hangs on the cursor at the draw.

**A message being typed** (input mode 3: I opens it, Enter sends it) is
fed by LastPressedKey alone, and the message slot is the witness: the
character it grew by names the key (the K of QUICK is Shift+K's key,
the I no witness's — fed by their samples they came a frame early or
not at all). A source that starts inside a message gets I and its
characters in the lead-in's last frames. QUICK, sent, turns the
session's cheats on (`str_AE408 +1` bit 7 — the launch bit's byte): a
take that cheats without sending it first has the bit from an earlier
level, and the driver sets it at the init park (`tool.args.cheats_on`).

**A source that starts paused.** Every live entity counts its ticks
(`+63`), and the carpet's says how many of the frames in front of the
source's first record ran one. mc1l6's player hit P as the level came
up: the carpet is 12 ticks old at frame 15 where the fresh boot's is
14, every creature of the level two ticks behind (2,529 bytes at the
sync frame). A paused tick draws the RNG and leaves, and an entity born
takes its private seed from the RNG, so WHERE the pause stood shows in
the seeds: a source whose first record stands paused has it at the
lead-in's end (P folded by the frame of the first tick that did not
run); one that had paused and gone on is given the pause in its first
frames — its place is not in the take.

**The recycle stack's stale cells are not compared.** Struct `+4597`
is a stack of 1,000 entity pointers, top index at `+4593` (-1 =
empty): the entities the allocator may take back when the pool runs
full. Level init resets the top and leaves the cells, so above the top
they hold what the SESSION's last full pool pushed (mc1l6's source:
338 pointers of an earlier level; a fresh boot: zeros). Nothing reads
above the top: `compare_mgcr.py` (`mc1_canonical`), the census, the
driver's sync verdict and its watch zero those cells in both images.

**The census of 2026-09-29** — every MC1 take and mc1hwl0..8, stage 2
and its compare (`pipeline_staging.py --census-only`,
`tools/retake-rig/2026-09-29-mc1/`, `census_board.py pass4 pass5`):

| | takes |
|---|---|
| IDENTICAL to the source, every record (the watch) and the compare | 58 of 62: all nine of mc1hwl0..8, 49 of MC1's 53 |
| … of them in one boot | 49 |
| … with the generator's row-0 cells learned | 5 (mc1hwl2, mc1l12, mc1l13, mc1l20, mc1l34) |
| … with a session's sound class learned | mc1l34 (end plain; handle 1657 / 7466), mc1l43 (end plain; handle 16126), mc1l47 (handle 14943 / 15386 / 19357 / 29670) — the port's roster rows, found again by the guest |
| … with the lead-in's hand searched by the guest | mc1hwl0, mc1hwl7 (and mc1l6, up to its fork) |
| … a source under way, its hand solved or nudged in the first boot | mc1hwl8, mc1l27, mc1l35, mc1l37, mc1l43, mc1l47 |
| … a source in a menu, a message, a pause | mc1hwl1, mc1hwl4, mc1l11, mc1l12, mc1l16, mc1l18, mc1l37-new, mc1l49; mc1l0-bigcastle; mc1l6 |
| … with a seam left to the anchor | mc1l29 (6 bytes: the carpet 2, 8, 4 off) |
| forked on THE SESSION'S BUILD DATA (below) | mc1l6 t=11733, mc1l26-froze t=31586, mc1l45-froze t=28114, mc1l48-nodeath t=9716 |
| not retaken | mc1l48 (starts over five times: below), mc1l32-terrainless (15 recording gaps) |

The driver of 2026-09-28 (`pass1/`): 14 IDENTICAL of 40 compared, 26
forks on the row-0 gate, 12 retakes lost. The third stage (the retake
of the retake, from its feed lane and its header's cells) was run on
seven of the 58 — mc1hwl2, mc1hwl4, mc1hwl8, mc1l0-bigcastle,
mc1l0-spells-galore, mc1l20, mc1l43: 2v3 and 1v3 IDENTICAL. The 58
whole takes: `re-recordings/`.

⚠ **OPEN — THE SESSION'S BUILD DATA.** Four takes leave their source
where a castle is built or levelled, and the fresh boot does there
what the PORT does: mc1l6 forks at the rival castle build the port's
open row names (t=11781, slot 529 `z`; the terrain 48 ticks ahead of
it); mc1l48-nodeath at the top wall of the registered "reload painter"
wound (the guest paints types 26 / 27 at (145..149, 60..61), the
source none); mc1l45-froze where a castle leveler (10,41) takes its
goal from the footprint's four corners (`+44`: 157 in the source, 87
in the guest, the planes equal); mc1l26-froze 330 ticks before its
end. The sources ran on build data the session had damaged (the
port's ruling of 2026-09-08: retail-side heap damage, not derivable
from any asset) — and a `-custom` boot with its sound off has none of
it. Not the driver's to write: no take carries those bytes.

**A level that starts over inside a take** (mc1l48, five times). A
dead wizard with no castle and Space LOSES the level (command 15:
flags |= 0xC, remc1 :48620) and retail loads it again, in the same
process, the frame counter from 1. The driver follows (the take's own
frame numbers are the expected ones; Space is fed where the level is
lost — Esc, command 29, quits to the menu, where nothing moves a
guest on), and the guest starts the level over at the same window.
⚠ OPEN: the level it starts over is a FRESH one, and the source's is
not — mc1l48's retake is the source's on its first 8,591 ticks, and at
frame 1 of the second run the source has 15 cells of building types
(39, 76, 78), 9 of height, 17 of shading and 1,458 of angle that a
fresh level does not have. The reload's own inputs are the session's
(the build data in memory, the port's "reload painter" wound): not in
the take, and not written by the driver.

**One record missing.** A take whose recorder lost a single window is
bridged (`bridge_gap`): the missing frame's stick is what takes the
predecessor's applied attitude + step to the successor's applied, its
arrows what the flight column's laws need between the two, a fire one
both neighbours consumed; the watch judges the successor. A longer gap
is refused (mc1l32-terrainless: 15 gaps, 74 records, one of 28).

**The lockstep's stale release** (2026-09-29, `wait_fresh_window`).
The driver released "a hold standing in the window already handled" on
two reads, the mailbox and the hold's state; a driver the host took
the CPU from between them found the guest a frame on, held in a FRESH
window, and released that one unhandled — `window N is followed by
N+2`, six takes of the first census (eight guests and their compares
on the 40 cores), at windows 3,169 .. 35,756. A held guest does not
move: the mailbox is read again behind the state, and the hold is
released only where it still stands in the handled window. Both arms.
The MC1 arm also stretches the frame hold's own timeout in the running
guest (~4.2 s as patched; `--hold-timeout`, 60 s): the longest hold of
a take that made it read 2.7 s.

**The anchor moves entities, and the tile index with them.** Where the
guest's own first frame has entities elsewhere than the source's
(mc1hwl7: 413 bytes of 156 entities — creatures that are placed by the
player's heading at frame 13, which the lead-in's attitude path does
not reproduce) the transplant of the pool alone leaves the tile chains'
heads (`mapEntityIndex`) the guest's, and the guest hangs in its next
tick. The driver writes the index the source's image implies — a live
entity with `+16` bit 2 and no predecessor (`+22`) heads the tile of
its place — after the guest's own plane proved that law on the guest's
own image.

**The volcano-guarded takes (2026-09-29, mc1hwl9 .. mc1hwl24).** The
takes of Hidden Worlds level 9 and up were recorded on the
volcano-guarded binary (mc1hwl9-crashed alone was not: it is the
retail crash), and no source header says so. Their retakes run
`reference/carpet/HIDDENV.EXE` — the guard and the frame hold, the 8.3
name of `HIDDEN_RECVG.EXE` — chosen by the batch:
`MGC_MC1HW_EXE=HIDDENV`. The driver holds the exe on disk against its
name (the kick store at 0x25FBE, `has_volcano_guard`), prints
`exe: HIDDENV.EXE, volcano guard ON` and keeps both in the retake's
header (`capture.exe`); a retake of the retake runs the exe its header
names and refuses the other. Stage 2 and its compare
(`tools/retake-rig/2026-09-29-mc1hw-vg/`, `board.txt`):

| | takes |
|---|---|
| IDENTICAL to the source, the watch and the compare | 18 of 19: 17 of the 18 guarded takes, and mc1hwl9-crashed on `HIDDEN.EXE` (4,643 ticks) |
| … of them in one boot | 9 |
| … with the generator's row-0 cells learned | mc1hwl9, mc1hwl9-lightningbug, mc1hwl10, mc1hwl13, mc1hwl14, mc1hwl14-badcastletoken, mc1hwl20, mc1hwl23, mc1hwl24 |
| forked | mc1hwl12 (below: solved, and left as it is) |

The 18 whole takes: `re-recordings/`.

**The spell menu's rows are 36 high** (`MENU_H`; it was 38, "measured"
on mc1hwl0's 126 picks, which fit 36, 37 and 38 alike). mc1hwl15's
first retake left its source for 36 ticks in ONE byte, the left hand
(`Type_160 +940`: slot 6 in the take, 10 in the guest, until the next
pick): the pick's sampled cursor, y = 270, was kept as hovering the
take's spell in a row of 238..275, and is the next row's (270..305).
The source's hand had moved on behind the sample. Measured on every
pick of the corpus by the cursor sampled before the closing record,
the hand standing still: 28 HW takes, 2,714 picks — 1 / 6 / 20 outside
a row of 36 / 37 / 38; 53 MC1 takes, 3,270 picks — 0 / 7 / 30. None of
the 58 takes of the census had a pick in the two or so pixels that
tell the heights apart and a hand that stood there.

**mc1hwl12 — the build table is read past its end, and what lies
behind it is the session's** (dug 2026-09-29;
`tools/retake-rig/2026-09-29-mc1hw-vg/dig/`). 32 ticks of
t=17531..25130 differ in the first retake, in runs, the world the
source's before, between and behind them: `+80 / +82` of whatever
entity holds pool slot 877, and `WIZ[7].T160 +331` on two of them. The
values are `sub_37150`'s — `+78 = 0xE000`, `+80 / +82 =
((dim << 8) + 1280) >> 1` of `begBuildTab[a2].dim.x / .y` (halved at
`typeResolution == 1`), `+84 = 0x4000` — called by the castle's
routines with the entity's own `+26` (`sub_46DB0` on every even tick;
`sub_12C50`, `sub_12D10` with `+26 + 1`). The slot was sized with
rows 101..115 and one row above 200 of a table of 78 (`BUILD1-0.TAB`,
468 bytes; MC1's `BUILD0-0.TAB` has 69): the read lands in the heap
block behind the table.

A `-custom` boot loads the resource table into an empty heap, back to
back in the table's order: SearchD 0x5F9D0, the build dat, the build
tab, `FONT0.DAT` at the tab's end rounded up to 16 (tab + 480 in HW,
tab + 416 in MC1), `FONT0.TAB`, `FONT1.DAT`, … — the guest's memory
behind the tab is the unpacked `FONT0.DAT`, byte for byte. A session
that came through the menu (every take of the corpus: run, load the
campaign, start the level) loads the same table into a heap the menu
has used, and the blocks fall elsewhere — measured on a guest driven
through the menu by X events (`dig/menu-tools/`):

MC1, CARPET.EXE   build dat 0x5F9D0 … SearchD 0x96600, build tab
                  0x97600, FONT1.DAT 0x977A0 (tab + 416), …,
                  FONT0.DAT 0x9CD10
HW, HIDDENV.EXE   build dat 0x5F9D0 … SearchD 0x96D80, build tab
                  0x97D80, FONT1.DAT 0x97F60 (tab + 480), …,
                  FONT0.DAT 0x9D490

`FONT1.DAT` behind the table is what the source read: a guest with the
file's 3,952 bytes written there (`--poke-ptr
1:-0x1DDA0:480:<hex>`) retakes mc1hwl12 IDENTICAL, 43,967 ticks; and
mc1hwl15 retaken through the menu, with every `-custom` repair off
(`--keep-conf --keep-launch-bit --no-init-cells --no-sound-block`),
is IDENTICAL too, 23,695 ticks. Sound is not part of it (a sound-on
guest holds the `-custom` boot's bytes).

⚠ **KNOWN LIMITATION — FIVE TAKES ARE NOT RETAKEN, AND STAY AS THEY
ARE** (player's ruling, 2026-09-29: the long tail of this work; the
corpus's whole takes are the 76 of `re-recordings/`.
mc1l32-terrainless is out of the campaign by the same ruling — an
early recording; mc1l48, which starts over five times, was not retaken
in the census and is not now). A retake boots `-custom -level N`
into an empty heap; what retail reads or writes out of bounds lands in
another session's memory than the source's.

| take | leaves its source at | of | what it is |
|---|---|---|---|
| mc1hwl12 | t=17531 (32 ticks, in runs to 25130) | 43,967 | the build table read past its end — SOLVED above, not written by the driver |
| mc1l26-froze | t=31586 | 31,915, froze | the level-250 castle of the last eruption kick (t=31424): 162 ticks behind the kick, 329 before the freeze. With `FONT1.DAT` behind the table the fork moves to t=31637. The two earlier kicks (t=25646, t=30711) retake clean |
| mc1l45-froze | t=28114 | 28,506, froze | the same: 384 ticks behind the kick of t=27730, 392 before the freeze; the font block changes nothing |
| mc1l48-nodeath | t=9716 | 30,424 | the registered painter wound (`mc1l48-reload-painter-dat-damage-*`): the source's build dat was damaged in memory. NOT REPRODUCED by any guest — the build dat and the search block of a `-custom` guest AND of a menu-path guest are the shipped files at t=1, 9700 and 11900 |
| mc1l6 | t=11733 | 39,755 | the registered `mc1l6-build-row5-dat-damage-ground-reads`; no volcano and no level-250 castle in front of it |

What writes the source's build dat and search block is not known
(untried: the interrupts of a real keyboard and mouse — the rig writes
the registers and none fires; keys no record's sample caught; real-time
pacing against the lockstep). The port's roster rows for these takes
stand as they are.

**The three stages of the 76 whole takes (2026-09-29,
`tools/retake-rig/2026-09-29-stage3/`, `board.txt`).** Stage 2 = the
whole takes of `re-recordings/` (49 MC1, 27 HW), stage 3 = their
retake from the feed lane and the header's cells, the exe the header
names (`capture.exe`; none: CARPET / HIDDEN), eight guests, 2 h 04.

| | takes |
|---|---|
| 1v2, 2v3 and 1v3 IDENTICAL | 75 of 76 |
| census 1v2 and 1v3: `struct:native, wallclock` alone | 75 |
| census 2v3: `struct:native, wallclock` | 60 |
| census 2v3: `wallclock` alone | 15 |
| stage 3 not made | mc1l29 |
| retakes lost to the rig | 1 of 77 runs (mc1hwl0: the emulator left at its start; the second run is the take) |
| world misses (a stage that diverged and was run again) | 0 |

⚠ **mc1l29's whole take does not retake.** Its lead-in ends with the
carpet +2, +8, -4 off the source's place (the census's one seam), and
the driver's anchor transplanted the six bytes at the source's first
frame (emitted t=15). The transplant is not in the feed lane: the
stage 3 guest flies the lead-in as fed, keeps its own place, and is
not the take's from t=15 on (`FORK: t=15`; it loses the level at
t=10042). From the source's first record on the take IS the source
(1v2 IDENTICAL, 28,151 ticks); what is not reproducible is the step
between its records 14 and 15. Left as it is, with the five above.

**The swap (2026-09-29, the player's).** The 75 certified stage-3
takes were copied over their sources in `recordings/`: an MC1 / HW take
there is now a whole take from the level's first frame, carrying its
feed lane (a retake of it replays the feed). The seven takes without a
stable whole take keep their ORIGINAL recording under the name
`<take>.torn.mgcr` — mc1hwl12, mc1l26-froze, mc1l29, mc1l45-froze,
mc1l48, mc1l48-nodeath, mc1l6 — a reminder that a retake is owed.
`re-recordings/` and every take file of the rig's run directories were
deleted the same day (the mid-products); where the text above says
`re-recordings/`, read `recordings/`. The run directories `tools/retake-rig/<date>-*/`
(logs, boards, results, the dig's dumps and the menu scripts) were
deleted too, once the 75 copies in `recordings/` had been checked
(each decompresses, and holds the record count of its certified
stage 3): the paths the text above cites are gone, the numbers are
what they measured. Kept beside the rig: `t_shift.txt` (every whole
take's shift, regenerated from the headers) and
`brief-baseline.pre-swap.txt` (the baseline as it stood before the
MC1 / HW swap; the MC2 rows of round 163 were not kept). The roster and
the brief baseline followed the swap the same day (round 165 in
docs/CONFORMANCE-FINDINGS.md): `mgc-conform` reads `<take>.torn.mgcr`
as the take `<take>`, so a torn take's rules and baseline row stand
under its old name and its old ticks; the 13 tick-keyed rules of the
swapped takes moved by their take's `t_shift`; the 75 baseline rows
were re-keyed. Fixtures, manifests and prose keep the original
numbering.

## Consumers

- **`--replay <file>`** (the game; LANDED): SOURCE-AGNOSTIC — one
  flag plays both arms (player-ruled):
  - **Retail takes** (`source:"retail"`, state channel required):
    inline input recovery (the shared laws in
    `mgc_formats::recover` — the consumed move/fire byte fed to the
    movers verbatim via `FlightInput::mc1_move_byte`, the inverted
    stick filter, hand equips/rebinds, the respawn witness, the cheat
    toast), world seeded by `retail_import_*` at the first closure,
    gaps re-anchor fresh segments. PURE replay: divergence is graded
    at every capture-clean boundary (the pose channel's lane set) and
    reported, never corrected. A recorded cheat with no port handler
    is REPORTED as such — it guarantees divergence from that tick, and
    an unexplained wall is worse than a named one.
  - **Port recordings** (`source:"port"`, `input:"exact"`): pins the
    header's sim closure (tier tags applied; a foreign
    `snapshot_version` is a refusal, not a warning), restores the
    embedded `start_mgcs_b64`, feeds the input channel and asserts
    the hash channel live.
  Either way the HUD carries a bit-exact / "diverged since t=N"
  counter — a mid-demo desync is surfaced on screen, never silently
  absorbed. Playback speed is a viewer control (F3; presentation
  only); per-tick semantics are invariant.

  **THE VIEWPOINT (player-ruled 2026-09-20).** A replay is watched one
  of two ways, and both are presentation: no sim state, no graded
  lane, nothing hashed.
  - **`--firstperson`** (the DEFAULT, so the flagless behaviour is
    unchanged): through the carpet's own eye, exactly as the take was
    flown. NOTHING is drawn at the player — the translucent GHOST
    billboard that used to ride at the recorded pose is now OFF. It
    sat under the viewport obscuring the picture, and the divergence
    it advertised is in the take's data and the HUD counter anyway.
  - **`--thirdperson`**: the eye swings onto a BOOM behind and above
    the carpet (`mgc-app/src/camera.rs`), looking along the flyer's
    own view direction — parallel, never toed in, so the horizon sits
    where first person puts it. The player's own carpet is drawn
    SOLID as the subject; retail draws it nowhere and the port has no
    human entity to unhide (`World::tick` takes a `PlayerPose`), so it
    is minted at draw time from the interpolated flyer, exactly like
    the ghost — an instrument, never a pool entity.
    ⭐ **THE BOOM FRAMES AT A FIXED ANGLE, NOT A FIXED LENGTH**: it
    sits on a cone at `atan(tan(fov_y/2)/3)` off the reversed view
    axis, which puts the subject on the two-thirds mark of the screen
    at EVERY length — and the length is not constant, because the
    boom shortens against terrain (floor via `ground_height_tiles`,
    cave roof via `player_cave_ceiling`, both `&self` reads) so the
    camera never ends up inside rock or through a cave ceiling. A
    framing that drifted as the boom shortened would pump the subject
    up and down the screen at every hill.
    ⭐ **IT IS A CHASE CAMERA, NOT A RIGID BOOM** (player-set
    2026-09-23, the Gothic model — `camera::ChaseCam`): the camera
    heading follows the carpet's with a ~0.3 s lag, so a turn shows
    the carpet's flank against the frame for a beat; the eye trails
    its boom target elastically (~0.12 s), so accelerations read as
    the carpet pulling away; the boom is four tiles and the axis is
    tilted 12° below the carpet's, watching it from a little above.
    The framing law holds THROUGH the lag because the camera looks AT
    the subject and lifts its axis by the cone angle — the subject is
    on the two-thirds mark on every frame, measured mid-lag by the
    test. The subject sprite BANKS through its turns
    (`Billboard::roll`, the one sprite that ever rolls): the enhanced
    mover's bank law re-derived from the recorded motion
    (`camera::motion_bank` → `mgc_sim::enhanced_bank`), smoothed. The
    world itself never rolls. `--firstperson` remains the correctness
    view; every chase number is a feel knob in `camera.rs`.
  - **`MGC_REPLAY_GHOST=1`** puts the ghost back in either view, for
    a take that HAS diverged: retail's pose translucent over the
    port's solid one is the A/B picture.
  `--thirdperson` is replay-only — it breaks aiming, so live play
  keeps retail's eye.
  **`--film <dir> [--film-from T] [--film-to T] [--film-rate N]`**
  (LANDED 2026-09-30) captures the session as numbered PNG frames
  on a FILM CLOCK: the frame loop stops reading wall time and every
  rendered frame advances exactly `1/N` of a turn (`N` frames per
  turn, so `N × 24` fps), nothing dropped, nothing paced by the
  display. Before `--film-from` the session SEEKS (bursts of 240
  turns per frame, no capture); it exits at `--film-to` or when the
  take ends. The frame is the ordinary live frame drawn a second
  time into a capture texture (`Renderer::render_capture`, the
  `render_texture` seam), so every effect, the HUD, the chase cam
  and the smooth-motion interpolation are in it; the wall-time FPS
  counter is the one thing suppressed. Verified bit-identical across
  runs (48 frames of mc2l7, two runs, every PNG equal). Audio is
  muted for the run. `tools/film.py` turns a SHOT LIST
  (`docs/media/shots.json`: take, turn window, camera, width) into
  animated WebP / GIF / MP4 / stills via ffmpeg — a README clip is
  a replay of a certified take, re-rendered after any visual change
  by re-running the script.
  **`--replay-check <file>`** is the headless twin: whole take, drift
  summary on stdout, exit 0 only on zero divergence. Its retail
  results are certified against `mgc-conform replay`'s (identical
  first-divergence boundaries on mc1l0 t=563 / mc2l3 t=244,
  2026-08-07).
- **`--record <out.mgcr>`** (the game; LANDED): write the running
  session as a port recording — `source:"port"`, `input:"exact"`
  (`mgc_formats::mgcr::PortInput`, the serialization mirror of the
  sim's `FlightInput`), hash channel on, and the start state embedded
  as `start_mgcs_b64` (a pristine level boot is just the t=0 special
  case, so mid-level and campaign starts replay exactly). Writer =
  `mgc_formats::mgcr::RecordingWriter` (zstd JSONL, `.jsonl` stays
  plain). Recording ends with the session (level switch or exit
  finalizes the stream).
  The header also carries the OFFLINE chassis overrides the session
  ran with — `entity_pool_size` and `awake_range`, written only when
  overridden — and `--replay` builds its world from those rather than
  from the replaying run's own CLI/config. They cannot be recovered
  any later: the start snapshot's identity block opens on
  `chassis.pool_slots` / `chassis.awake_gate_sq`, so a world built at
  any other size REFUSES the snapshot ("snapshot is for a different
  world"), which is what made a `--pool-slots N` take unreplayable by
  every invocation including the one that recorded it. A take from
  before these keys reads as "not overridden", i.e. the faithful
  default it was recorded under.
- **`--replay <in.mgcr> --record <out.mgcr>`** (LANDED) — re-record a
  take as an INPUT-ONLY port take, for sharing. The combination used to
  be refused. A retail take is ~500 KB/tick and nearly all of it is the
  two channels the replay never hands to the sim: `state` (61%) and
  `obs` (39%); the tick's player input is 0.0% of the file. It is NOT
  the `input` channel either — that holds the raw DOSBox externals
  (`keys_down`/`mouse`) that retail's consume loop filters and latches
  before the mover sees them, so the crop cannot be a channel filter.
  But a replay ALREADY recovers the exact input every tick and
  `--record` already writes exactly that format, so letting them meet
  is the whole feature. Measured 104-264x smaller. `--replay-check
  <in> --record <out>` is the headless twin; `tools/strip-recordings`
  batches it and VERIFIES each output by replaying it.
  Two rules make the result reproduce, both in `begin_replay_recording`:
  1. **Start after the anchor.** A retail take seeds the world from its
     first closure inside the driver's first `next`, so a snapshot taken
     at session install captures a pre-seed world and the take desyncs
     on tick one.
  2. **Carry the import pin** (`World::import_pin`). The seeding is an
     import, and an imported world holds config/state the snapshot
     deliberately skips — `strict_retail`, `measured_terrain`, the
     carpet slot, `castle_reg`, and the rest of the residue. Missing
     one shows up as a take that cannot reproduce its own hash channel
     (mc1l0 without it: 17 ticks; with `strict_retail` only: 59; with
     the pin: all 7,097). Grow the list as more turn up — the recipe is
     to read the divergence tick's INPUT record, which names the event
     that first touched the missing lane: mc1l4 desynced at t=105 and
     t=104 is its first `fire_left`, which is `wiz_charge` (stamped
     onto the manifestation's `f26` at spawn, so invisible until a
     spell is actually cast). Note that enumerating the importer's
     writes needs indexed and nested assignments too, not just
     `self.field =` — `wiz_charge` is written as `self.wiz_charge[i]`.
     STILL OPEN: mc1l1 desyncs at t=2850, which is a `demolish`
     (`mc1_move_byte: 48`) — so the lane is something
     `World::player_castle` resolves through, the `wizext+50`/
     `castle_reg` side. `castle_reg` itself is already pinned and reads
     all-zero there, so it is one hop further out.
  A re-recorded take is NOT a conformance fixture: the retail channels
  ARE the oracle, and an input-only take has nothing to grade. Keep the
  originals; `mgc-conform` reads those.
  Multi-segment takes are out of scope by ruling — a re-anchor is a
  capture gap that input alone cannot cross, so the recording stops
  there and says so (all takes should be single-segment; fix the rig,
  not the consumer).
- **Puppet playback** (any recording, retail included): drive the
  recorded poses through the renderer with **no sim** — watch the
  actual retail run inside the port. Presentation styling is free
  here: e.g. enhanced-style banking is a pure function of turn rate ×
  forward speed, both recoverable from the pose stream, so a retail
  run can be *shown* banking into its curves without touching physics.
- **The fixture runner** — `mgc-conform` (crates/mgc-conform):
  - `check-decode` (any recording): re-decode every tick's raw
    `state` through the Rust decoders (`mgc_formats::mgcr`) and
    demand value equality with the stored `obs` channel — pins the
    Rust decode against the recorder's.
  - `verify-deltas` (retail; MC1/HW and MC2 wired): for each
    adjacent tear-gate-clean pair, import the raw `state` at N onto a
    pristine-built world (`World::retail_import_mc1` /
    `World::retail_import_mc2` — pool slot-for-slot incl. hidden
    state, the LIVE free-stack order, globals, the human column
    routed outside the pool), tick once with **pin-the-human** (the
    recorded carpet pose drives `World::tick`, so world fidelity
    verifies with zero dependence on input reconstruction), and diff
    the port's obs projection (`World::obs_project_mc1` /
    `obs_project_mc2`) against the recorded `obs` at N+1. The MC2
    arm additionally excludes per-entity-torn slots (phase-byte
    delta ≠ 1 inside an accepted pair) from field comparison.
    Reports: fixture-grade vs torn pair counts, per-tick LCG
    draw-count histogram, the +63 phase-clock table, entity-set
    events by (class, model), and per-field mismatch counters with
    examples. `--pin-pose n|n1`, `--input-delay k` (cast
    reconstruction from the raw input channel — MC1 only; the MC2
    arm derives the cast phase from the press latch instead, see
    below), `--dump t`.
    A deviations allowlist keyed to DEVIATIONS.md entries is still
    open work.
  - `extract` / `fixtures` — the FIXTURE SUITE (docs/CONFORMANCE.md):
    lift triaged pairs into a committed manifest
    (`conformance/*.json`, expected status per pair) and replay them
    as an automated expected-status test on every `cargo test`
    (crates/mgc-conform/tests/suite.rs; skips when the recording or
    baked tree is absent).
  - `replay` (retail): PURE INPUT REPLAY (docs/CONFORMANCE.md "The
    replay verifier") — seed the world ONCE from the first closure,
    then free-run feeding only the input stream recovered from the
    recording (the consumed move/fire byte, the inverted stick
    filter, hand equips, the respawn key); divergence is reported at
    every recorded boundary and never corrected. A `t` gap re-anchors
    a fresh segment, so gap-free takes replay as one unbroken chain —
    recorders should keep striving for gap-free streams.
  - `verify-replay` (port): init from header, feed inputs, compare
    the hash at every tick — LANDED as the game's own
    `--replay-check` (the port arm of the source-agnostic `--replay`
    above; the round-trip is pinned by
    `mgc-app replay::tests::port_record_replay_roundtrip`).

## Cross-model replay (sandbox, not replay)

The input channel carries both encodings, so a recording *can* be fed
to a sim configured with a different `thrust_model`/`altitude_model`.
This is a **sandbox**, not a reproduction: the flight tiers are
different in-sim physics (chase-the-pointer steering vs. the retail
stick law; hold-to-fly drag vs. the speed-target chase; crosshair-lead
casting vs. hull-heading casting), and the pilot flew closed-loop
against one of them. Expect the trajectory to diverge within seconds
and compound. Tools MUST void the hash channel and mark the session as
non-verifying when the header's models are overridden.

## Size expectations (non-normative)

Input-only demo: tens of bytes/tick — minutes of gameplay in tens of
KB. Full retail capture: ~230 KB/tick raw before compression; the
between-tick redundancy lets container-level zstd absorb the channel
(measured ~20 KB/tick under an adversarial synthetic worst case —
incompressible base image, fully random 2 KB/tick churn; real structs
are mostly sparse and churn is clustered, so expect better). The
decoded `obs` channel is ~170 KB/tick uncompressed JSON; it exists for
greppability and comparison, not economy.
