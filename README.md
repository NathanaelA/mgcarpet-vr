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
mgc-app     ── winit shell: window, input, fixed-timestep game loop
```

Verification strategy: original-engine output is the oracle. Expanded
terrain and parsed data are validated byte-for-byte against reference
dumps produced by the original code ([remc2] for MC2, instrumented DOSBox
for MC1). Fixtures live outside git (derived from copyrighted data); their
SHA-256 hashes are committed as pins.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

Rust 1.85+ (edition 2024). No system dependencies.

## Game data

Copy your installed GOG game directories into `gamedata/` — see
[gamedata/README.md](gamedata/README.md). Then:

```sh
cargo run -p mgc-import -- scan gamedata
```

which finds and integrity-checks every RNC-compressed file in the data.

## Status

Bootstrap. RNC decompression works; DAT/TAB parsing, level import, and the
first rendering milestone (the "carpet flyer") are next.

## Related projects

- [remc2 / Magic Carpet 2 HD](https://github.com/thobbsinteractive/magic-carpet-2-hd) —
  the reverse-engineered MC2 port this project uses as its behavioral
  reference. If you want to *play* MC2 today, go there.
- [Moburma's Magic Carpet tools](https://github.com/Moburma) — level
  extractors and format research.
- [MagicCarpetFileFormat](https://github.com/michaelhoward/MagicCarpetFileFormat) —
  MC1 level format specification.

## License

GPL-3.0-or-later. The RNC decompressor is ported from remc2's
implementation (itself derived from the decompiled original ProPack).

[remc2]: https://github.com/thobbsinteractive/magic-carpet-2-hd
