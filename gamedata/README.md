# Game data (bring your own)

The engine requires original game data from legally owned copies. Nothing
in this directory except this README is ever committed to git.

Expected layout: **1:1 copies of the GOG install directories**, kept
exactly as the installer laid them down (cruft, DOSBox, manuals and all —
that makes them trivially reproducible for anyone who owns the games),
plus an optional `overlay/` directory for community modifications (see
"Community overlay" below):

```
gamedata/
├── Magic Carpet Plus/    ← GOG "Magic Carpet Plus" install, verbatim
│   └── CARPET.CD/game.gog     (CD image; MC1 + Hidden Worlds data)
├── Magic Carpet 2/       ← GOG "Magic Carpet 2" install, verbatim
│   ├── game.gog               (CD image; most MC2 data + soundtrack)
│   └── GAME/NETHERW/          (hard-disk portion)
└── overlay/              ← OPTIONAL: community mods (may be a git clone)
```

Directory names don't matter — the importer detects layouts by content
(`mgc_import::gamedata`) and reads game files straight out of the
`game.gog` CD images (`mgc_import::iso`); nothing is ever extracted to
disk. The legacy fully-installed flat layouts (`mc1/` with `DATA/` +
`LEVELS/` on disk, `mc2/GAME/NETHERW/` without a CD image) are still
recognized; GOG installs win when both are present.

To obtain the installs: run the GOG installer (Windows, or Wine on
Linux) and copy the resulting directory here — or skip Wine entirely
with [innoextract](https://constexpr.org/innoextract/), which unpacks
GOG's Inno Setup installers directly on Linux and yields this same
layout.

**Any subset is fine.** Each game (MC1, Hidden Worlds, MC2) is detected
independently: `bake` processes whatever is present and skips the rest
with a note, and the test suite likewise self-skips per missing game.
Hidden Worlds data ships inside the Magic Carpet Plus install, so it
normally arrives together with MC1.

Quick check that the data is readable (also verifies every RNC
container inside the CD images):

```sh
cargo run -p mgc-import -- scan gamedata
```

## Community overlay (mods)

`gamedata/overlay/` holds community-modified or community-authored
replacement files, consumed when the game data is baked. It is entirely
optional, and it is designed to BE a git checkout — clone a community
mod repository (e.g. `mgc-community`) as this directory and its
`README`/`LICENSE`/`.git` are ignored. Full spec (naming rules, what a
modded bake means, how to extend the structure with new categories):
[docs/MODDING.md](../docs/MODDING.md).

Layout — one directory per game (`mc1`, `mc1hw`, `mc2`), one directory
per category; `levels/` is the only category so far:

```
gamedata/overlay/
├── mc1/levels/LEV00032.DAT     ← replaces MC1  LEVELS.DAT   member 32
├── mc1hw/levels/LEV00007.DAT   ← replaces HW   DDLEVELS.DAT member 7
└── mc2/levels/LEV00003.DAT     ← replaces MC2  LEVELS.DAT   member 3
```

Level files are decompressed archive-member payloads named
`LEVnnnnn.DAT` (`LEV` + zero-padded 5-digit member index + `.DAT`,
case-insensitive) — the naming the community's extraction tools
produce. Anything the bake cannot apply is reported by name, never
silently ignored.

Two rules to know:

- **Overlay changes are NOT auto-detected.** The rebake-on-stale check
  only tracks importer versions. After adding, changing, or removing
  overlay files, **delete `baked/`** — the next game start (or
  `cargo run -p mgc-import -- bake gamedata baked`) rebakes with the
  overlay applied.
- **An overlay bake is a MODDED bake, not a faithful one.** Level data
  feeds the simulation, so a modded bake self-identifies everywhere: a
  `MODDED` marker file at the baked root lists every substitution, the
  game prints an `OVERLAY` line when loading a modded level, saves are
  keyed to the modded data (they refuse to load against a pristine
  bake, and vice versa), and the conformance/golden tooling refuses
  the tree outright. To return to a faithful setup: remove (or move
  aside) `gamedata/overlay/`, delete `baked/`, rebake.
