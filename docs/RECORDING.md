# The `.mgcr` gameplay recording format

Version 1 — this document is normative. Any tool that reads or writes
`.mgcr` recordings should treat this file as the specification; changes
to the format must land in the same commit as changes to this document.
(Pre-1.0: no backward compatibility is owed to any earlier sample
recordings — player ruling 2026-07-29.)

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
Line 1 MUST be the header record; every following line is a tick
record, in strictly increasing tick order.

Writers MUST serialize floats so they round-trip bit-exactly
(serde_json's shortest-round-trip encoding does). 64-bit hashes are
hex **strings** — JSON numbers are doubles and cannot carry a u64.

## The header record

Common fields:

```json
{"type":"header","format":1,
 "game":"mc1|mc1hw|mc2","level":3,
 "source":"retail|port",
 "tick_hz":24,
 "channels":{"input":"exact|raw|none","obs":true,"state":true,"hash":false},
 "tool":{"name":"mc_dosbox_recorder","git":"<rev>"},
 "created":"2026-07-29T12:00:00Z"}
```

`source:"retail"` adds: `"build":"A|B"` (CARPET.EXE / HIDDEN.EXE
address half), plus free-form capture provenance (DOSBox version,
cycles).

`source:"port"` adds `"sim"`, the **sim-config closure** — everything
that feeds the state hash, pinned so `--replay` can refuse (or
force-apply) a mismatched environment:

- `thrust_model`, `altitude_model` — the flight tiers are **sim
  physics**, not presentation (`Simulation::thrust_model` doc:
  *"fixed per run; replay headers must record them once replays
  exist"*; DEVIATIONS.md "enhanced flight": *"Selected once at the sim
  boundary; replays record it"*).
- `snapshot_version`, pool sizes (the pool size feeds the hash).
- every sim-reaching option from the options registry, including
  sim-affecting dev instruments (e.g. `dev.lift_unclamped`);
  presentation-only options are excluded and never recorded.
- RNG seed(s) and level/campaign provenance; for mid-level starts an
  embedded start snapshot (`"start_mgcs_b64"`), otherwise the pristine
  level is the tick-0 state.

## Tick records

```json
{"t":N, "input":…, "obs":…, "state":…, "hash":…, "wallclock":…}
```

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
  the tick boundary — held scancodes, mouse cursor, held buttons
  (MC1/HW), or the persistent steering state (MC2, which exposes no
  raw input registers). This is an **approximation**: the in-struct
  10-byte control command is consumed and zeroed mid-tick, so a click
  shorter than one tick can be missed or land ±1 tick. Raw input is
  advisory; retail recordings are validated by state, never by
  replaying input.

Retail's own multiplayer lockstep puts exactly the per-tick 10-byte
control commands on the wire — the "consumed command per player per
tick" unit is retail's own canon, and this channel is deliberately
shaped like it.

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

### `hash` — the port verification channel (port only)

The golden state hash at tick N, as a hex string. Inputs + hashes is
full byte-exact determinism verification at a few dozen bytes per
tick — and it is the desync checksum retail's lockstep never had, so a
future multiplayer inherits it unchanged.

## Gaps

Recorders SHOULD emit gap-free streams (lower DOSBox cycles until they
do). A jump of k>1 in `t` is legal but breaks the fixture pairing
across it; runners count and report pair coverage.

## Consumers

- **`--replay <file>`** (the game): port recordings only
  (`source:"port"`, `input:"exact"`). Pins the header's sim closure —
  mismatch is a refusal, not a warning. Feeds the input channel,
  optionally asserts the hash channel live; a mid-demo desync is
  surfaced on screen, never silently absorbed. Playback speed is a
  viewer control (presentation); per-tick semantics are invariant.
- **Puppet playback** (any recording, retail included): drive the
  recorded poses through the renderer with **no sim** — watch the
  actual retail run inside the port. Presentation styling is free
  here: e.g. enhanced-style banking is a pure function of turn rate ×
  forward speed, both recoverable from the pose stream, so a retail
  run can be *shown* banking into its curves without touching physics.
- **The fixture runner** (dedicated test bin), two modes:
  - `verify-replay` (port): init from header, feed inputs, compare the
    hash at every tick.
  - `verify-deltas` (retail): for each adjacent pair, import the raw
    `state` at N into a `Session`, tick once, diff the `obs` at N+1 —
    with a deviations allowlist keyed to DEVIATIONS.md entries, and a
    **pin-the-human** mode that forces the human carpet's recorded
    state each tick so world fidelity (AI, monsters, projectiles,
    regen) verifies with zero dependence on input reconstruction.

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
