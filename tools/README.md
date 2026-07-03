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

## MC1 oracle (planned)

Reference dumps for MC1 terrain generation, via instrumented DOSBox
running the original binary (the dosbox-x-remc2 methodology), until/
unless the generator can be carved out of the dormant remc1
decompilation — or MC2's generator proves compatible with MC1 seeds,
which should be tested against the DOSBox dumps first.
