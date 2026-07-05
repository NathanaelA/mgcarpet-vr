# The `.mgcl` level package format

Version 1 — this document is normative. Any tool that reads or writes
`.mgcl` files should treat this file as the specification; changes to the
format must land in the same commit as changes to this document.

## Design goals

- One self-contained file per level, cheap to load, trivial to inspect.
- A **superset of both games**: Magic Carpet 1, Hidden Worlds, and Magic
  Carpet 2 levels use one schema; earlier games simply omit the features
  they don't have.
- **Expanded, not seeded**: everything the original games generate from
  seeds at runtime (terrain, most importantly) is stored pre-generated.
  Engines and editors never deal with seeds, RNG sequencing, or original
  Bullfrog formats.
- Community-friendly: any language with a zip library and a JSON parser
  can read and write levels. Original *game-derived* packages must not be
  redistributed (they are derived from copyrighted data), but
  community-authored levels in this format are original works.

## Container

A `.mgcl` file is a **ZIP archive with all members stored uncompressed**
(compression method 0, "stored").

- Writers MUST NOT compress members and MUST write deterministic output
  (fixed timestamps), so identical content yields identical bytes.
- Readers MAY reject members using any compression method other than
  stored.
- Member names are case-sensitive and use `/` as the path separator.
- Tools MUST preserve members they do not understand when rewriting a
  package (read-modify-write, not regenerate-from-model).

## Members

| Member | Required | Format | Content |
|---|---|---|---|
| `meta.json` | yes | JSON | Format version, game, level identity, provenance |
| `things.json` | yes | JSON | Entity and marker records |
| `genparams.json` | no | JSON | Original terrain-generation parameters (provenance) |
| `level.json` | no | JSON | MC2 level header: environment, player slots (absent on MC1) |
| `wizards.json` | no | JSON | MC2 per-wizard configuration: AI stats, spell loadouts |
| `stages.json` | no | JSON | MC2 mission script: stage checkpoints and variables |
| `terrain/height.bin` | no | binary | Expanded heightmap (see below) |
| `terrain/type.bin` | no | binary | Expanded terrain-type map |
| `terrain/shading.bin` | no | binary | Per-tile light level (see below) |
| `terrain/angle.bin` | no | binary | Per-tile texture orientation/flags (see below) |

MC1/Hidden Worlds terrain is always present (the importer generates it
natively); MC2 terrain is present when the importer had the MC2 oracle
tool available. Readers must tolerate absence, and the terrain members
always appear together.

### `meta.json`

```json
{
  "format_version": 1,
  "game": "mc1",
  "level": 0,
  "source": {
    "archive": "LEVELS.DAT",
    "entry": 0,
    "entry_sha256": "<hex sha-256 of the raw archive entry>"
  },
  "importer": { "name": "mgc-import", "version": "0.1.0" }
}
```

- `format_version` (integer): this document's version. Breaking changes
  bump it; additive changes (new members, new optional JSON fields) do
  not.
- `game`: `"mc1"`, `"mc1hw"` (Hidden Worlds), or `"mc2"`.
- `level`: index of the level in its source archive (campaign order for
  retail levels).
- `source`: provenance of game-derived packages — enough to regenerate
  and to verify the input was the expected retail data. Absent on
  community-authored levels.
- `importer`: tool and version that produced the package.

### `things.json`

All placed objects, in a single array ordered by original slot index:

```json
{
  "things": [
    { "slot": 0, "kind": "entity", "class": 2, "model": 0,
      "x": 45, "y": 144, "dis_id": 65535, "swi_sz": 0, "swi_id": 65535,
      "parent": 0, "child": 0 }
  ]
}
```

- `slot`: index in the original entity table. `parent`/`child` fields
  reference these indices (linked lists: paths, wizard groups), so slots
  are preserved verbatim from the source data.
- `kind`: `"entity"` (a real placed thing) or `"marker"` (MC1-only:
  class-0 records observed in retail data as structured coordinate
  chains, probably terrain-feature nodes; semantics not yet confirmed).
  In **MC2**, class 0 is Conditional Spawn — a real gameplay entity that
  materializes when its trigger group fires — and is `"entity"`.
  Garbage slots present in retail files (uninitialized editor memory)
  are **not** baked.
- `class`/`model`: original Bullfrog type IDs, preserved exactly so any
  package can be diffed against reference-engine behavior. The known
  class/model → name mapping is documentation, not data. Note the IDs
  are per-game namespaces (e.g. spell pickups are class 12 in MC1,
  class 15 in MC2).
- Coordinates are on the game's 256×256 logical grid.
- `dis_id`, `swi_sz`, `swi_id`: disposition and switch linkage as in the
  original format; `65535` (0xFFFF) means "none".
- **MC2 field mapping**: values are byte-order-normalized from the
  on-disk representation (little-endian in GOG retail data). MC2 semantics
  occupy the shared fields as follows: `swi_sz` holds MC2's unnamed
  word at record offset 0x0A; `swi_id` holds the **stage tag** (MC1's
  switch-target ID repurposed for the mission system — still trigger
  linkage); `parent`/`child` hold context parameters 1/2 (building
  type, teleport destination X/Y, or path parent/child links depending
  on entity type); `par3` (MC2 only, absent on MC1 records) holds
  context parameter 3.

### `genparams.json`

The original `GEN_MAP` block, kept for provenance and for editors that
want to "reroll" terrain via original-engine oracles. Engines MUST NOT
require it: the authoritative terrain is the expanded `terrain/*` data.

```json
{
  "pre_header": 135538,
  "seed": 1921, "off": 41339, "raise": 2834, "gnarl": 0,
  "river": 1, "sourc": 0, "snlin": 200, "snflt": 50,
  "bhlin": 30, "bhflt": 16, "rkste": 18,
  "footer": [35, 1, 0, 0, 0, 0]
}
```

Field names are Bullfrog's own (recovered from their GAM save-game text
format). `raise` is signed. Game-specific fields: `pre_header` and
`footer` are MC1-only; `lriver` (river length/count) is MC2-only. Each
is omitted where not applicable.

### `level.json` (MC2 only)

```json
{
  "level_id": 3, "gfx_type": 0, "map_type": "day",
  "players": [1, 1, 0, 0, 0, 0, 0, 0],
  "unk05": 0, "unk07": 10, "unk09": 0
}
```

- `map_type`: `"day"`, `"night"`, or `"cave"` — selects the entire asset
  set the original engine loads (sprites, sky, palette, tables, blocks).
- `players`: activation flags for the 8 wizard slots.
- `unk*`: unexplained original header fields, preserved verbatim.

### `wizards.json` (MC2 only)

Exactly 8 blocks; slot 0 is the human player, 1–7 the AI wizards.

```json
{
  "wizards": [
    { "aggression": 128, "reflexes": 100, "perception": 80, "life": 500,
      "starting_spells": [1, 0, ...26 values...],
      "unknown_spells":  [0, ...],
      "blocked_spells":  [0, ...] }
  ]
}
```

Spell arrays have 26 entries indexed by MC2 spell ID (0 = Fireball …
25 = Cave In); `starting_spells` values are upgrade tiers 0–3.
`unknown_spells` mirrors an unexplained original array, preserved
verbatim.

### `stages.json` (MC2 only)

The mission script, preserved verbatim from the level's stage tables
(unused 0xFF-filled slots are omitted):

```json
{
  "checkpoints": [ { "index": 0, "stage": 1, "x": 115, "y": 212 } ],
  "variables":   [ { "index": 0, "stage": 1, "x": 10, "y": 20, "data": 0 } ]
}
```

Checkpoints and variables together encode the objective sequence.
Observed decoding of checkpoint records (confirmed against the spec's
gameplay-verified tutorial script — e.g. MC2 level 0 bakes to
`(5, 0, 115, 212)` = fly-to (115,212), `(7, 103, 0, 0)` = kill creatures
linked to entity slot 103): `index` holds the **objective opcode**,
`stage` the opcode's parameter (entity slot, mana threshold, or wizard
id), `x`/`y` the target coordinates. Opcodes per remc2's BasicTerrain.h:
0 = collect mana, 1/7 = kill creature, 2 = destroy structure,
3 = kill enemy wizard, 5 = release point / fly-to, 8 = kill all players,
9 = destroy building. Field names stay as the originals pending full
confirmation; the data is the raw source of truth. Objective
display text lives in the game's `ETEXT.DAT` and is not part of the
package.

### `terrain/height.bin`, `terrain/type.bin`, `terrain/shading.bin`, `terrain/angle.bin`

65,536 bytes each: one byte per tile on the 256x256 grid, row-major,
index `y * 256 + x`, matching the original engine's in-memory layout
(height in `height.bin`, per-tile terrain/texture type in `type.bin`,
light level in `shading.bin`, texture orientation in `angle.bin`).
`shading.bin` and `angle.bin` are optional (additive members).

`shading.bin` indexes the shade dimension of the game's color-remap
tables — the engine resolves a tile's displayed color as
`palette[tables[shading * 256 + tables[0x14000 + type]]]` (both table
slices are baked as assets: `shade-lut-N.bin`, `tile-colors-N.bin`).

`angle.bin` is the generator's per-tile angle/flags byte. Bits 4-6
select one of 8 UV orientations for the tile's terrain texture
(dihedral: bit 4 = flip x, bit 5 = flip y, bit 6 = swap axes —
decoded from the engine's `UVTable_D4350`, world-space/base-0 rows;
critical for one-directional transition tiles like shorelines). The
other bits carry generator flags (terrain class in bits 0-2, deep
water — MC1 — or cave ceiling — MC2 — in bit 3, visibility in bit 7)
and are preserved verbatim.

The content is the **pristine output of the original generation
algorithm**. MC2: the algorithm carved verbatim out of remc2 into
`tools/mc2-genlevel`, run over the level's seed parameters and
validated byte-for-byte against remc2's DOSBox-derived regression
fixtures. MC1/Hidden Worlds: the importer's native Rust port of MC1's
own generator (`mc1_terrain.rs`, from the remc1 decompilation —
docs/ROADMAP.md "MC1 reference generator found"), whose heightmap
reproduces the previously-validated oracle output near-byte-exactly
and whose type layer is MC1's real classifier. Entity-driven terrain
modification (walls, canyons, building flattening) is deliberately NOT
baked in: engines apply those at load time from `things.json`, exactly
as the original engine does after generation (implemented for MC1/HW
in `mgc_sim::features` — the GenerateFeatures port; it additionally
consumes the `search.bin` ring table and `build.{tab,dat}.bin`
building footprints from the level's asset bundle). Vertical-scale
and water-level semantics will be documented as the renderer work
firms them up.

## Versioning and evolution

1. New members and new optional JSON fields are additive — no version
   bump; old readers ignore them.
2. Changing the meaning or layout of an existing member bumps
   `format_version`; readers reject versions they don't know.
3. This file is updated in the same change as the code.

# Asset bundles

The second engine-facing format (Rust types + loader:
`mgc_formats::bundle`): everything translated from one game's asset
catalogs, as a directory of uniformly-named members. Where `.mgcl`
carries what is *level*-scoped, a bundle carries what is *world*-scoped
— palettes, color LUTs, terrain textures, sprites, terrain-feature
data; sounds, music, and text will join additively.

One schema, many *variants*: game and environment differences are
expressed as bundle instances, never as layout differences. Current
variants: `mc1-temperate`, `mc1-arctic` (MC1's two complete world
tilesets; Hidden Worlds levels use the arctic one). Pending original
data from the MC2 CD image: `mc2-day`, `mc2-night`, `mc2-cave`. The
engine resolves a variant id (`baked/assets/<variant>/`); Bullfrog
catalog names, RNC, sprite RLE, and FLC animation encodings all die in
the importer.

All integers little-endian; all pixel data 8-bit palette indices
(palette-as-LUT is the engine's authenticity baseline — RGBA is a
render-time resolve, and index 0 is the sprite-transparent index).

| member | contents |
|---|---|
| `bundle.json` | manifest: `format_version`, `variant`, `game`, importer, source catalog files + raw-file sha256 |
| `palette.bin` | 256 x RGBA8, VGA 6-bit expanded (`v<<2\|v>>4`); index 0 has alpha 0 |
| `shade-lut.bin` | 64 rows x 256: `shade level x palette index -> final palette index` (the light/fog remap; `TABLES.DAT` first 0x4000 bytes) |
| `tile-colors.bin` | 256: terrain type -> flat map color index (`TABLES.DAT` +0x14000) |
| `terrain-atlas.bin` + `.json` | terrain texture atlas, square cells; the json gives `{cell, width, cells}`; the terrain-type byte indexes cells row-major |
| `sprites.bin` + `sprites.json` | one 8bpp atlas of all world-sprite frames + its index (below) |
| `search.bin` | 32x32 ring-order grid (terrain-feature digs) |
| `build.tab.bin`, `build.dat.bin` | building footprint RLE maps (terrain-feature building pass) |

`sprites.json` (`mgc_formats::bundle::SpriteIndex`): `atlas_width`,
`atlas_height`, and one entry per original sprite id (dense — ids are
the original engine's world-sprite numbering; known-corrupt retail
entries stay as frame-less placeholders). Each entry: `id`, `group`
(first id of its rotation/animation family, from the TMAPS TAB group
field), `width`/`height` (all frames share one size), original
`flags`, and `frames[]` of atlas `{x, y}` positions — frame 0 is the
base image, further frames are the pre-decoded FLC animation. The
flags high byte is the engine's *draw type* (how facing/animation pick
a family member — see `mgc_sim::mc1_sprite_stats`); flags bit 0 marks
animated entries.

Provenance per source (MC1): `PAL{N}-0.DAT`, `TABLES.DAT` /
`DTABLES.DAT`, `BLK{N}-1.DAT`, `TMAPS{N}-0.DAT/.TAB`,
`BUILD{N}-0.TAB/.DAT`, `SEARCH.DAT` for N = 0 (temperate) / 1
(arctic). The versioning/evolution rules above apply unchanged
(`bundle.json` `format_version`).
