# MC1 castle terrain datum — the re-averaging law (adjudicated 2026-07-22)

Player report: a castle built on a peak sinks to ambient ground as it
upgrades; the player initially believed retail froze the height datum at
first build. Decompile trace (opus agent, remc1 + remc1hw corroboration)
REFUTED the frozen datum, and the player then reproduced the sink in
retail (GOG install, peak build, every upgrade stepped lower) —
**the sink is faithful retail behavior; the port is correct; not-a-bug.**

## The law

On EVERY transform (first build and each upgrade), the class-3 m2
castle runs painter → leveler:

- **Painter** `sub_285C0` (:30445, byte70 44): composites build rows
  1..=level into a scratch buffer sized exactly `buildTab[level].w×h`
  and ramps the footprint to `datum + cell` over ~19 ticks. Writes are
  datum-relative absolute; the loop covers the UN-grown footprint —
  **no apron/skirt** (:30537-30583).
- **Leveler** `sub_28200` (:30284, byte70 43): `current = site z/32`;
  `target = sub_361C0(x0-1, y0-1, h+2, w+2)` (:30434) — the FLOOR-MEAN
  of the four corners of the footprint grown by one tile per side,
  clamped 220. Those corners are strictly outside the painted rect and
  always hold ambient terrain. 9 ticks of uniform translation of the
  whole footprint, then finish (:30419-27): castle state 2,
  **castle+154 = 32 × target** — :30424 is the ONLY writer of +154 in
  the entire binary — then perimeter smooth `sub_35F30` depth 3.
- The ctor seeds +154 = raw center ground (:44255-56); the L1 leveler
  immediately replaces it with the L1 grown-corner mean ("the tiles
  just below the L1 tower" — the true first datum).
- The damage REPAINT also re-enters the leveler (painter finish :30708
  sets state 5 unconditionally), so even an un-upgraded castle under
  repeated bombardment re-averages toward local terrain.

## Why the sink is chunky, not gradual

buildTab footprints (full-res): L1 8×8, L2-3 21×21, L4-5 35×35,
L6-7 48×48 — three jumps of ~6-7 tiles per side. Each jump samples
virgin terrain far beyond the previous smooth (depth 3), so the datum
drops at L2/L4/L6 and barely moves at L3/L5/L7. Corners reaching water
(height 0) drag the mean hard toward 0. The `sub_360C0` smoother
cannot prop the corners up: it excludes building-textured cells
(types 6..0x22) from its 3×3 average.

## MC2 contrast (the sequel's fix)

MC2 computes its datum ONCE at the castle ctor (`sub_4AA40` EF:33399):
32 × perimeter-MIN over the BUILD00 row-1 footprint, stored in
axis+154 and reused by every upgrade/repaint painter; the growing
absolute `datum + cell` stamp terraforms surroundings toward the
frozen datum. MC2 does not sink — retail or port. (See
mc2-castle-*.md traces.)

## Port status

- `tick_castle_leveler` (engine/features.rs), `avg4`, the painter, and
  the state chain are line-faithful; verified in both directions.
- Regression pin: `crates/mgc-sim/tests/mc1_castle_datum.rs` — peak
  build + upgrades to 7; asserts each post-transform datum equals the
  grown-corner floor-mean sampled pre-cast, and that the peak castle
  sinks. A deliberate frozen-datum deviation (if ever ruled in as an
  opt-in alternate) must consciously update that test.
