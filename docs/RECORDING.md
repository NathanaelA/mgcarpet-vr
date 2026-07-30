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
cycles). The `capture` object also carries `tear_gate` (bool: emit-time
inter-tick gating ran) and, for a tick-patched exe,
`window_gated: true` with `exe_patch: {mailbox_guest, spin_period_counts}`
(see "Tick-patched capture") — where each `t` is the stub's
authoritative sub-step counter.

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
conformance verdicts. MC2 has no per-entity clock; its equivalent
gate is open work (Turn + LCG-step parity at minimum).

The FIRST record has no pair to gate it, so recorders MUST NOT write
it unvetted (a mid-tick anchor rejects every later pair against it and
starves the stream): hold the candidate and flush it only once the
first clean pair vouches for it, replacing the anchor with the newer
read whenever a bootstrap pair is rejected.

## Tick-patched capture (windowed)

The tear gate is a *reconstruction* — it infers, after the fact,
whether a frozen snapshot happened to land between ticks. The exe
tick-patch (`tools/mc_exe_tickpatch.py`) removes the inference by
making the game cooperate. It installs a ~167-byte wrapper stub around
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
   counter (measured live at ~480 Hz) until one period (default 5 counts)
   has elapsed since the last release. The default game speed runs the
   tick fn 4× per rendered frame, so `fps = 480 / (4 × period)` ≈ **24 fps**
   at period 5 — the authentic Magic Carpet rate — regardless of how high
   DOSBox `cycles` is set; the excess cycles are burned in the spin. (Both
   obj1's cave and obj3's mailbox must be page-aligned via their `vsize`
   fields, or the tail is outside the segment limit — the code cave won't
   execute and the mailbox writes won't persist.) This is the frame cap
   retail never had; it
   is a *presentation* throttle only. MC1's sim is wall-clock
   independent (its lockstep multiplayer proves it: the PIT counter
   feeds render/animation timing, never sim state), so pacing changes
   *when* sub-steps run, never *what* they compute — the recorded tick
   sequence is byte-identical to an unpaced run.

2. **Publishes a mailbox** in obj3's committed tail (guest-linear
   `0x132c40`, same address in both builds; the stub derives obj3's real
   runtime base from the game's own relocated struct pointer so its writes
   stay in obj3 and never corrupt game memory): an 8-byte magic
   (`MGCTTIK1`), a monotonic sub-step counter (`+8`), and an
   `in_window` flag (`+0xC`) raised for the whole spin. The spin *is*
   the quiescent window — the world struct is fully settled from the
   previous sub-step and the current one's LCG draw has not begun — and
   it is proportional to the spare cycle budget, so on a fast host it is
   ~7 ms wide on *every* sub-step, bursts included.

A recorder that finds the magic switches to **windowed capture**: take
the struct only while `in_window==1`, require the counter and struct to
stay put across the consensus reads, and use the counter's delta as
continuity. This is strictly stronger than the tear gate (a
between-tick window is guaranteed by construction, not inferred) and
`t` is the stub's authoritative sub-step index, not a `+63`-mode
estimate. Such recordings stamp `capture.window_gated: true` and
`capture.exe_patch: {mailbox_guest, spin_period_counts}`; consumers may
treat window-gated snapshots as tear-free without re-running
`pair_clean`. The tear gate remains the path for unpatched exes and for
MC2 (which is already `Turn`-throttled and gap-free).

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
- **The fixture runner** — `mgc-conform` (crates/mgc-conform):
  - `check-decode` (any recording): re-decode every tick's raw
    `state` through the Rust decoders (`mgc_formats::mgcr`) and
    demand value equality with the stored `obs` channel — pins the
    Rust decode against the recorder's.
  - `verify-deltas` (retail; MC1/HW wired, MC2 open): for each
    adjacent tear-gate-clean pair, import the raw `state` at N onto a
    pristine-built world (`World::retail_import_mc1` — pool
    slot-for-slot incl. hidden state, the LIVE free-stack order,
    globals, the human column routed outside the pool), tick once
    with **pin-the-human** (the recorded carpet pose drives
    `World::tick`, so world fidelity verifies with zero dependence on
    input reconstruction), and diff the port's obs projection
    (`World::obs_project_mc1`) against the recorded `obs` at N+1.
    Reports: fixture-grade vs torn pair counts, per-tick LCG
    draw-count histogram, the +63 phase-clock table, entity-set
    events by (class, model), and per-field mismatch counters with
    examples. `--pin-pose n|n1`, `--input-delay k` (cast
    reconstruction from the raw input channel), `--dump t`.
    A deviations allowlist keyed to DEVIATIONS.md entries is still
    open work.
  - `extract` / `fixtures` — the FIXTURE SUITE (docs/CONFORMANCE.md):
    lift triaged pairs into a committed manifest
    (`conformance/*.json`, expected status per pair) and replay them
    as an automated expected-status test on every `cargo test`
    (crates/mgc-conform/tests/suite.rs; skips when the recording or
    baked tree is absent).
  - `verify-replay` (port): init from header, feed inputs, compare
    the hash at every tick — not built yet.

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
