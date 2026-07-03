# tools

Home for pinned, run-once oracle tools that are not part of the engine:

- `mc2-genlevel` (planned): MC2's terrain/level generation carved out of
  remc2 (`~/projects/remc2`, GPL) into a standalone CLI. Invoked by
  `mgc-import` to expand seeded level data; the engine itself never links
  or runs it. Candidate sources: `remc2/engine/Terrain.cpp`
  (`GenerateLevelMap_43830`), the `rand2` LCG, and the terrain
  decompression routines, plus their immediate dependencies.

- MC1 oracle (planned): reference dumps for MC1 terrain generation,
  produced via instrumented DOSBox running the original binary (the
  dosbox-x-remc2 methodology), until/unless the generator can be carved
  out of the dormant remc1 decompilation.
