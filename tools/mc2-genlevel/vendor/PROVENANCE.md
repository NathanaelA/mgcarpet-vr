# Vendored code provenance

The files in this directory are copied **verbatim** from the remc2 /
Magic Carpet 2 HD project (GPL-3.0):

- Source: https://github.com/thobbsinteractive/magic-carpet-2-hd
- Commit: `3000646247f43e357f608b389877ec9d3ad5bcf0` (local checkout,
  development branch)
- Files: `remc2/engine/Terrain.cpp`, `remc2/engine/Unk_D47E0.{cpp,h}`,
  `remc2/engine/Unk_D4A30.{cpp,h}`

`Terrain.cpp` contains the reverse-engineered MC2 terrain generation
(originally decompiled from MC2.EXE by Tomáš Veselý and contributors).
Do not edit these files: refresh them from upstream instead, and keep
this commit reference current. The shim header `../Terrain.h` adapts
them to build standalone.
