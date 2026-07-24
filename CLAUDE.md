# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
# Build
cargo build --workspace

# Test (most tests skip silently without baked data)
cargo test --workspace

# Run a single test
cargo test -p mgc-sim state_hash

# Import/bake game data (run once per machine)
cargo run -p mgc-import -- scan gamedata
cargo run -p mgc-import -- bake gamedata baked

# Run the game
cargo run -p mgc-app -- --level mc1:1
cargo run -p mgc-app -- --campaign mc1 --slot 0

# Headless screenshot (no display needed)
cargo run -p mgc-app -- --level mc1:1 --screenshot out.png

# Build the MC2 terrain oracle (needed to bake MC2 terrain)
make -C tools/mc2-genlevel
```

Linux audio requires `libasound2-dev`. Rust 1.85+ (edition 2024) is required. `opt-level = 1` is set in dev profile because RNC decompression is too slow at level 0.

## Architecture

The pipeline flows in one direction:

```
gamedata/  (GOG installs, never committed)
    ↓
mgc-import  — the ONLY code that understands Bullfrog formats (RNC, DAT/TAB,
    |          XMI, seeded terrain generation). Runs once per machine; bakes
    |          everything into .mgcl packages. The engine never sees a seed.
    ↓
mgc-formats — the baked package format (.mgcl = stored ZIP + JSON + raw bins).
    |          This is the sole data contract between import and the engine.
    ↓
mgc-sim     — pure, headless, deterministic simulation core (24 Hz fixed tick)
mgc-render  — wgpu renderer (reads sim state, interpolates between ticks)
mgc-audio   — cpal output + ported original mixer + FLAC music
mgc-app     — winit shell: window, input, fixed-timestep game loop
```

**mgc-sim** is the heart of the engine. It is strictly I/O-free, single-threaded, and deterministic: given the same `.mgcl` package and input sequence, output is bit-identical everywhere. The sim advances only via `Simulation::step`; rendering interpolates between the last two ticks and never influences state. World coordinates: 1.0 = one terrain tile (256 fixed-point units in the original); map is 256×256 tiles, wraps toroidally; altitude = `height_byte / 8`.

The sim is split into:
- `engine/` — the shared MC1/MC2 core (`world.rs`, `features.rs`, `combat.rs`, `rivals.rs`)
- `mc1/` — MC1-specific modules (spells, mobs, rivals, flight)
- `mc2/` — MC2-specific modules (roster, multipart, castle, cast, stagevars, flood, doomsday, tail, proj)
- `flight.rs`, `chassis.rs`, `ids.rs`, `verbs.rs` — shared flight model, pool params, game identity, dispatch

**mgc-app** owns the game shell: it loads `.mgcl` packages, runs the fixed-timestep loop (`TICK_RATE_HZ = 24`), maps winit input into `FlightInput`/`PlayerCommand`, and interpolates between the last two sim ticks for rendering. Submodules: `config.rs` (options registry), `settings.rs` (runtime registry, `mgcarpet.json`), `frontend.rs`/`frontend_mc1.rs` (campaign menus), `ui.rs` (HUD/book/map quads), `saves.rs`, `worldmap.rs`, `entities.rs` (minimap dots/billboards), `campaign.rs`.

**G-class vs P-class options.** The authenticity matrix distinguishes two option kinds: **G-class** (gameplay) options change simulation state or RNG consumption and are recorded into replays — a run under a non-faithful G option is not a faithful fixture. **P-class** (presentation) options resolve at render/input time and never affect simulation outcomes; they can be changed freely. The config and DEVIATIONS.md documents every option with its class and faithful default.

## Fidelity and deviation conventions

This is a porting project. **The faithful original behavior is always the default and always available.** Every deviation from retail is deliberate and opt-in. The relevant docs:

- `docs/DEVIATIONS.md` — the canonical register of every intentional departure from retail behavior, with code site markers `(deliberate)`. **Check this before "fixing" anything toward retail.**
- `docs/FIDELITY.md` — the porting record: what each subsystem implements, verification grade, and known gaps.
- `docs/traces/` and `docs/spell-audit/` — the decompile research bank; cited by code comments.

Verification grades (weakest to strongest): decompile-traced → oracle-diffed → player-validated → player-certified → retail-verified. **Recorded original gameplay outranks the decompile** — remc1/remc2 are machine reconstructions with known transcription errors; when retail play contradicts them, retail wins.

## Lint policy

The workspace `Cargo.toml` allows several Clippy lints that would normally be idiomatic improvements: `collapsible_if`, `needless_range_loop`, `unnecessary_cast`, etc. **This is intentional.** Sim, audio, and importer code is traced from the original engine decompilations, and keeping the original's shape — including its nesting, arithmetic, indexed loops, and boolean shapes — is a feature: it is what makes the port auditable against the trace. Do not refactor these lints away.

## Testing model

Tests in `mgc-sim` require baked game data (placed at `baked/` relative to the workspace root) and self-skip when it is absent. The golden state-hash tests (`state_hash.rs`) pin the CURRENT port's behavior, not retail's — they are refactoring invariants. When a deliberate behavior change lands, regenerate them by running the test with `--nocapture` and copying the printed array; say so in the commit. Do not re-pin goldens for unintentional divergences.

The `state_hash` test maintains two separate golden arrays: `GOLDEN` (full internal state including pool layout) and `OBSERVABLE` (terrain + population + poses only). A layout-only re-pin moves `GOLDEN` but leaves `OBSERVABLE` unchanged; if `OBSERVABLE` moves, behavior changed.

## `.mgcl` package format

`.mgcl` files are stored (uncompressed) ZIP archives. The format is documented in `docs/FORMAT.md`, which is normative and evolves in lockstep with the code. Community-authored levels are freely shareable; packages baked from copyrighted GOG data must not be redistributed.
