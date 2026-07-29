# tools

Pinned, run-once oracle tools that are not part of the engine.

## mc2-genlevel

Standalone MC2 terrain generation: the original algorithm (diamond-square
fractal + rivers + surface typing), carved **verbatim** out of remc2
(`vendor/`, see `mc2-genlevel/vendor/PROVENANCE.md`) with a thin shim
header and CLI around it. `mgc-import bake` invokes it per level to
produce the `terrain/*.bin` package members; the engine itself never
links or runs it.

Build:

```sh
make -C tools/mc2-genlevel
```

`bake` finds the binary at its default build location, or via the
`MGC_GENLEVEL` environment variable; without it, packages bake without
terrain members.

Validation: output is byte-identical to remc2's DOSBox-verified
regression memimages (all four generated arrays, confirmed on levels
whose fixtures contain no post-generation entity edits; the test
`baked_terrain_matches_remc2_fixture` re-checks this when a remc2
checkout is present, override its location with `MGC_REMC2`).

## rip-mc2-cdaudio.py

Pulls the 27 redbook soundtrack tracks out of the GOG MC2 install's
`game.gog` CD image into FLAC files (needs ffmpeg), for the engine's
future music support. Game *data* files are never extracted this way —
the importer reads them straight from the image (`mgc_import::iso`).

## mc_dosbox_recorder.py

Records a retail playthrough from a running DOSBox into a `.mgcr`
recording — zstd-compressed JSONL, one record per game tick;
**`docs/RECORDING.md` is the normative format spec**. Line 1 is the
header (game, level, build, channel declaration); each tick record
carries the decoded observable projection (`obs`), the RAW master-struct
image plus MC1's external input registers (`state` — the full
mutable-state closure, the fixture-initialization source; retail's own
in-level save writes this exact struct with a single `fwrite`), and the
persistent raw input at the tick boundary (`input`, approximate by
nature — retail recordings verify by state, never by replaying input).
`--no-state` drops the closure for lightweight scouting runs; an output
path ending in `.jsonl` (or `-` for stdout) writes plain JSONL. Writing
`.mgcr` needs the python `zstandard` package. Expect very roughly
~20 KB/tick compressed with the state channel (see
`docs/traces/mc1-campaign-save-menu.md` for the struct map it reads, and
the memory note "retail-conformance-recorder" for the design).

It launches DOSBox as a **child** (so reading its memory usually needs no
root under the default ptrace policy), then locates the master world
struct by CONTENT — scanning guest RAM for the loaded level's pristine
embedded record (so it needs `--level <n>`, the level you're in). DOS4GW
doesn't map guest addresses to host memory affinely, so fixed addresses
don't work; the globals (wall clock, raw input) live in a *separate*
static frame found by its own landmark. It waits patiently through
menus/FMVs, then waits for the sim to actually be ticking before
recording. It takes N consecutive byte-identical reads as proof of a
non-torn, between-ticks snapshot (CONSENSUS), and — since retail has no
global logic-tick counter — counts elapsed ticks from the mode of the
per-entity `+63` increment across persistent entities. Consensus only
proves the guest was frozen; the inter-tick tear gate then rejects
mid-pass parks (cursor clock bands, LCG parity, and the early-cursor
park whose only tell is a moved LCG under a zero +63 mode). When the
sim saturates the emulated CPU (level-start spawn storms, heavy
combat) every park is mid-tick and ticks are unrecoverable — the
recorder reports that loss live per pending tick and folds the streak
breakdown into the gap line, rather than leaving a silent `t` jump.
The first record is only written once the first clean pair vouches
for it (an unvetted mid-tick anchor would starve the whole stream).

```sh
# locate + decode ONE clean snapshot, print a sanity census
./tools/mc_dosbox_recorder.py --game mc1 --level 0 --once -- dosbox -conf … CARPET.EXE

# record ~200 ticks (park the player somewhere safe first)
./tools/mc_dosbox_recorder.py --game mc1 --level 3 --out run.mgcr --max-ticks 200 \
    -- dosbox -conf … CARPET.EXE
#   --pid <n>          attach to an already-running dosbox instead
#   --no-wait-live     don't wait for gameplay to be ticking
#   --no-state         omit the raw closure (fixtures NEED it)

# inspect a recording
zstdcat run.mgcr | head -1 | jq .        # the header
zstdcat run.mgcr | jq -c 'select(.t==5) | .obs.player'
```

Start the game and load the level you named with `--level`; the tool
waits for it. Games:

* `mc1` — validated against a live dump.
* `mc1hw` — shares MC1's engine + struct; reads DDLEVELS. Core +
  externals both verified, build auto-detected (CARPET.EXE=A,
  HIDDEN.EXE=B).
* `mc2` — the D41A0_0 engine (a different struct). Field map verified
  against two live struct dumps (levels 0 and 4): the human decodes to
  class 3 model 0 with the right life/mana and the level-record needle
  locates exactly one struct. The differences from MC1 are handled
  internally by `family == "mc2"` — a 168-byte pool record (facing is the
  world-space yaw at +0x1C, verified live), a 2124-byte per-player block
  whose per-frame `Turn` counter drives continuity (MC2 has no per-entity
  tick byte) and whose flight column holds the persistent steering command
  (`cmd_speed`), and a structural pool-census locate filter. MC2 keeps no
  separate static frame and exposes no usable raw input register, so
  steering intent is read from that persistent state (heading + cmd_speed)
  rather than a mouse/key register. It reads CLEVELS, so `--level` extracts
  the matching MC2 level record via `mgc-import`.

```sh
# MC2: record a level-0 playthrough
./tools/mc_dosbox_recorder.py --game mc2 --level 0 --out mc2run.mgcr \
    -- dosbox -conf … MC2.EXE
```

If capture reports missed-tick gaps, lower DOSBox `cycles` (or raise the
resolution) so the sim runs slow enough to snapshot every tick.

## MC1 oracle (planned)

Reference dumps for MC1 terrain generation, via instrumented DOSBox
running the original binary (the dosbox-x-remc2 methodology), until/
unless the generator can be carved out of the dormant remc1
decompilation — or MC2's generator proves compatible with MC1 seeds,
which should be tested against the DOSBox dumps first.
