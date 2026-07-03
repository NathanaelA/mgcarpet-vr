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

The terrain pair is present when the importer had a terrain oracle
available (currently MC2 only); readers must tolerate its absence, and
the two members always appear together.

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

### `terrain/height.bin` and `terrain/type.bin`

65,536 bytes each: one byte per tile on the 256x256 grid, row-major,
index `y * 256 + x`, matching the original engine's in-memory layout
(height in `height.bin`, per-tile terrain/texture type in `type.bin`).

The content is the **pristine output of the original generation
algorithm** — produced by running the algorithm itself (carved verbatim
out of remc2 into `tools/mc2-genlevel`) over the level's seed
parameters, and validated byte-for-byte against remc2's DOSBox-derived
regression fixtures. Entity-driven terrain modification (walls, canyons,
building flattening) is deliberately NOT baked in: engines apply those
at load time from `things.json`, exactly as the original engine does
after generation. Vertical-scale and water-level semantics will be
documented as the renderer work firms them up.

## Versioning and evolution

1. New members and new optional JSON fields are additive — no version
   bump; old readers ignore them.
2. Changing the meaning or layout of an existing member bumps
   `format_version`; readers reject versions they don't know.
3. This file is updated in the same change as the code.
