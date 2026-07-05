# Game data (bring your own)

The engine requires original game data from legally owned copies. Nothing
in this directory except this README is ever committed to git.

Expected layout: **1:1 copies of the GOG install directories**, kept
exactly as the installer laid them down (cruft, DOSBox, manuals and all —
that makes them trivially reproducible for anyone who owns the games):

```
gamedata/
├── Magic Carpet Plus/    ← GOG "Magic Carpet Plus" install, verbatim
│   └── CARPET.CD/game.gog     (CD image; MC1 + Hidden Worlds data)
└── Magic Carpet 2/       ← GOG "Magic Carpet 2" install, verbatim
    ├── game.gog               (CD image; most MC2 data + soundtrack)
    └── GAME/NETHERW/          (hard-disk portion)
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
