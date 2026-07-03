# Game data (bring your own)

The engine requires original game data from legally owned copies. Nothing
in this directory except this README is ever committed to git.

Expected layout:

```
gamedata/
├── mc1/    ← your installed "Magic Carpet Plus" GOG directory, copied
│             wholesale (contains the MC1 + Hidden Worlds data files)
└── mc2/    ← your installed "Magic Carpet 2" GOG directory, copied
              wholesale (contains the NETHERW directory)
```

The GOG releases ship as installers; install them (native or via Wine),
then copy the resulting game directories here. The importer only reads
these files — it never modifies them — and bakes engine packages from
them into `baked/` (also git-ignored).

Quick check that the data is readable:

```sh
cargo run -p mgc-import -- scan gamedata
```
