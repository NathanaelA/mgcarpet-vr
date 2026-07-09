# mgcarpet

A modern, cross-platform engine for Bullfrog's **Magic Carpet** (1994) and
**Magic Carpet 2: The Netherworlds** (1995).

This is an engine-only reimplementation: it ships no game content and
requires original game data from a legally owned copy (both games are sold
on [GOG](https://www.gog.com/en/game/magic_carpet) —
[MC2 here](https://www.gog.com/en/game/magic_carpet_2_the_netherworlds)).
The goal is one engine that plays both games — with fixed-timestep
simulation (game speed independent of framerate), GPU rendering with real
draw distance, save-anywhere, and modern platform support — while treating
the original gameplay as the specification: behavioral fidelity by
default, deviations deliberate and opt-in.

## Architecture

```
gamedata/  (your GOG installs — never committed)
    │
    ▼
mgc-import  ── the only code that understands Bullfrog formats (RNC,
    │          DAT/TAB, XMI, seeded terrain generation). Runs once per
    │          machine; expands everything into baked packages. All
    │          procedural/seeded data is expanded here — the engine never
    │          sees a seed.
    ▼
mgc-formats ── the baked package format: the sole data contract
    │
    ▼
mgc-sim     ── pure, headless, deterministic simulation core
mgc-render  ── wgpu renderer (reads sim state, interpolates between ticks)
mgc-audio   ── cpal output + the ported original mixer + FLAC music
mgc-app     ── winit shell: window, input, fixed-timestep game loop
```

Verification strategy: original-engine output is the oracle. Expanded
terrain and parsed data are validated byte-for-byte against reference
dumps produced by the original code ([remc2] for MC2, instrumented DOSBox
for MC1). Fixtures live outside git (derived from copyrighted data); their
SHA-256 hashes are committed as pins.

## Quickstart (playtesting)

1. Get the `mgcarpet` binary: a
   [release](../../releases) archive for Linux/Windows, or build it
   yourself (below).
2. Copy your installed GOG game directories into a `gamedata/` folder
   next to the binary — see [gamedata/README.md](gamedata/README.md)
   for the expected layout. Any subset works (MC1 only is fine).
3. Run it:

   ```sh
   ./mgcarpet                                  # MC1 campaign level 1
   ./mgcarpet --level baked/mc1/level-009.mgcl # a specific level
   ```

   On first run the game finds no baked data and **bakes it from your
   GOG installs automatically** (once per machine; also after an
   upgrade that changes the bake — the data carries an epoch stamp).
   To point elsewhere than `gamedata/`, set `MGC_GAMEDATA` or
   `"gamedata"` in `mgcarpet.json`.

Options live in `mgcarpet.json` (sparse overrides) next to the
generated `mgcarpet.json.defaults`, which documents every option with
its faithful-authentic default. `--help` lists the CLI flags.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

Rust 1.85+ (edition 2024). Linux needs the ALSA headers for audio
(`libasound2-dev` on Debian/Ubuntu, `alsa-lib` headers elsewhere);
Windows and macOS need no system dependencies.

## Game data

The engine consumes *baked* packages, generated from your GOG installs
under `gamedata/` — the game shell does this by itself on first run
(see Quickstart), and `mgc-import` exposes the same importer as a
standalone tool:

```sh
cargo run -p mgc-import -- scan gamedata   # integrity-check the data
cargo run -p mgc-import -- bake gamedata baked   # bake everything
```

## The level package format

Levels bake to `.mgcl` files — stored (uncompressed) ZIP containers with
JSON for structured data and raw binary for bulk grids, one schema
covering both games. The format is explicitly documented in
**[docs/FORMAT.md](docs/FORMAT.md)**; that document is normative and
evolves in lockstep with the code. Community-authored levels in this
format are original works and freely shareable (unlike packages baked
from the copyrighted game data, which stay on your machine).

## Status

MC1 is playable at near-parity and player-certified faithful across
the core game: flight model, terrain and villages, monster AI, combat,
the full 24-spell repertoire, mana economy, castles, player mortality,
and rival (AI) wizards — with sound effects and the original music.
The porting record — what each subsystem does, how it was verified,
and where it deliberately deviates — is being assembled in
docs/FIDELITY.md; docs/ROADMAP.md is the working log. MC2 levels parse
and render (environment bundles, terrain via the remc2-carved oracle);
its gameplay port comes after MC1.

## Credits and prior art

This project stands on years of community reverse engineering. It would
not exist without:

- **Tomáš Veselý (turican0)** — the original
  [remc2](https://github.com/turican0/remc2), the assembly→C++
  reconstruction of Magic Carpet 2 that made the engine family
  understandable at all.
- **Tim Hobbs (thobbsinteractive) and contributors** —
  [Magic Carpet 2 HD](https://github.com/thobbsinteractive/magic-carpet-2-hd),
  the actively maintained continuation (play MC2 today: go there), whose
  codebase serves as this project's behavioral reference, and whose
  `DataFileRNC` implementation our RNC decoder is ported from.
- **Michael Howard** —
  [MagicCarpetFileFormat](https://github.com/michaelhoward/MagicCarpetFileFormat),
  the MC1 level format specification our parser is built against.
- **Moburma** — [Magic Carpet tooling](https://github.com/Moburma)
  (MCDatExtractor, MCLevelReader, MCLevelEdit, BullfrogSoundExtractor)
  and the extensive unused-content documentation on
  [The Cutting Room Floor](https://tcrf.net/Magic_Carpet_(DOS)).
- **lab313ru** —
  [rnc_propack_source](https://github.com/lab313ru/rnc_propack_source),
  the decompiled original RNC ProPack.
- **Bullfrog Productions** — for the 1994 original that was so far ahead
  of its time we're still catching up to it.

## License

GPL-3.0-or-later. The RNC decompressor is ported from remc2's
implementation (itself derived from the decompiled original ProPack).

[remc2]: https://github.com/thobbsinteractive/magic-carpet-2-hd
